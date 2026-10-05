//! The simulated reasoner: its law, its cost, its draws, and what its wrong answers look like.

use super::*;
use crate::kinds::Tier;
use crate::oracle::{self, ObsLabel};
use crate::rng::Gen;
use crate::{
    Diagnosis, ObsRef, Question, StreamAction, StreamEvent, StreamHypothesis, StreamKind,
    StreamOutcome, StreamSimulator,
};
use gordian_core::Instant;
use gordian_world::ServiceId;
use std::collections::BTreeMap;

/// The logistic function, with the standard library's `exp`, independent of the crate's own.
fn sigma(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn open_params(seed: u64, abc: (f64, f64, f64)) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.mean_gap_ns = 8_000_000_000;
    p.reasoner.a = abc.0;
    p.reasoner.b = abc.1;
    p.reasoner.c = abc.2;
    p.budget.reasoner_ns = u64::MAX / 2;
    p
}

/// Deliver everything, ask `rounds` questions about every incident with varying amounts of its
/// decisive evidence and some noise in the context, collect every answer. Returns, per
/// `(incident, k)`, the number of calls, the number right (judged by comparing the answer with
/// the truth, not by the simulator's own flag), and the expected probability computed here.
fn run_law(params: &StreamParams, rounds: usize) -> (usize, Vec<(u32, u64, u64, f64)>) {
    let (stream, truth) = with_truth(params);
    let mut sim = StreamSimulator::new(stream);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let mut rng = Gen::keyed(&[params.seed, 0xFEED]);
    let n_obs = truth.labels.len() as u64;
    // (incident, k) -> [calls, right, p]
    let mut groups: BTreeMap<(u32, usize), (u64, u64, f64)> = BTreeMap::new();
    let mut asked: BTreeMap<u32, (u32, usize)> = BTreeMap::new(); // call -> (incident, k)
    let mut total = 0;
    for inc in &truth.incidents {
        let m = inc.decisive.len();
        for round in 0..rounds {
            let k = round % (m + 1);
            let mut context: Vec<ObsRef> = inc.decisive[..k]
                .iter()
                .map(|o| ObsRef::Passive(*o))
                .collect();
            // Noise: observations that are not decisive evidence of this incident.
            for _ in 0..(rng.below(6)) {
                let o = crate::ObsId(rng.below(n_obs) as u32);
                let decisive_here = matches!(
                    truth.labels[o.0 as usize],
                    ObsLabel::Incident { id, role: crate::labels::EvidenceRole::Decisive } if id == inc.id
                );
                let r = ObsRef::Passive(o);
                if !decisive_here && !context.contains(&r) {
                    context.push(r);
                }
            }
            let focus = inc.observations[0];
            let out = sim.apply(
                StreamAction::Escalate {
                    context,
                    question: Question::Diagnose { focus },
                },
                end,
            );
            let StreamOutcome::Escalated { call, .. } = out else {
                panic!("refused: {out:?}")
            };
            asked.insert(call, (inc.id, k));
            let q = k as f64 / m as f64;
            let p = sigma(
                params.reasoner.a + params.reasoner.b * q - params.reasoner.c * inc.difficulty,
            );
            groups.entry((inc.id, k)).or_insert((0, 0, p)).0 += 1;
            total += 1;
        }
    }
    // Collect the answers once their latency has passed.
    for e in sim.observe_until(Instant(params.duration_ns + 3_600_000_000_000)) {
        if let StreamEvent::Answered { call, answer, .. } = e {
            let (inc, k) = asked[&call];
            let right = answer.diagnosis == truth.incidents[inc as usize].truth;
            groups.get_mut(&(inc, k)).unwrap().1 += right as u64;
        }
    }
    // The simulator's own record agrees with the independent judgement.
    let traced = oracle::calls(&sim);
    assert_eq!(traced.len(), total);
    let own_right: u64 = traced.iter().filter(|c| c.correct).count() as u64;
    let judged_right: u64 = groups.values().map(|g| g.1).sum();
    assert_eq!(own_right, judged_right, "trace and answers disagree");
    (
        total,
        groups
            .into_iter()
            .map(|((inc, _), (n, r, p))| (inc, n, r, p))
            .collect(),
    )
}

fn check_law(groups: &[(u32, u64, u64, f64)], total: usize, label: &str) {
    let mut excess = 0.0;
    let mut var = 0.0;
    let mut chi2 = 0.0;
    let mut used = 0.0;
    for (_, n, r, p) in groups {
        let (n, r) = (*n as f64, *r as f64);
        excess += r - n * p;
        let v = n * p * (1.0 - p);
        var += v;
        if v >= 1.0 {
            chi2 += (r - n * p).powi(2) / v;
            used += 1.0;
        }
    }
    let z = excess / var.sqrt();
    println!(
        "{label}: {total} calls in {} cells, z = {z:.2}, chi2 = {chi2:.1} on {used} cells",
        groups.len()
    );
    assert!(total >= 10_000, "{label}: only {total} calls");
    assert!(z.abs() < 4.0, "{label}: total frequency off by z = {z}");
    assert!(
        (chi2 - used).abs() < 5.0 * (2.0 * used).sqrt(),
        "{label}: cells are not calibrated, chi2 {chi2} on {used}"
    );
    // Calibration by decile of the expected probability.
    let mut bins = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); 10]; // n, right, sum p, sum var
    for (_, n, r, p) in groups {
        let b = ((p * 10.0) as usize).min(9);
        bins[b].0 += *n as f64;
        bins[b].1 += *r as f64;
        bins[b].2 += *n as f64 * p;
        bins[b].3 += *n as f64 * p * (1.0 - p);
    }
    for (i, (n, r, ep, v)) in bins.iter().enumerate() {
        if *n < 200.0 {
            continue;
        }
        assert!(
            (r - ep).abs() < 4.5 * v.sqrt().max(1.0),
            "{label}: decile {i}: {r} right of {n}, expected {ep}"
        );
    }
}

#[test]
fn the_reasoners_accuracy_follows_the_law_over_ten_thousand_calls() {
    let (n, g) = run_law(&open_params(11, (-1.0, 5.0, 2.0)), 250);
    check_law(&g, n, "default (a, b, c) = (-1, 5, 2)");
}

#[test]
fn the_law_holds_across_other_settings_of_a_b_and_c() {
    for (i, abc) in [(0.0, 3.0, 1.0), (-2.0, 6.0, 4.0), (1.5, 1.0, 0.5)]
        .into_iter()
        .enumerate()
    {
        let (n, g) = run_law(&open_params(20 + i as u64, abc), 260);
        check_law(&g, n, &format!("(a, b, c) = {abc:?}"));
    }
}

#[test]
fn the_law_covers_a_wide_range_of_q_and_d() {
    // The test above is only as good as the cells it fills. Check the spread of q and d.
    let params = open_params(11, (-1.0, 5.0, 2.0));
    let (_, t) = with_truth(&params);
    let mut qs = std::collections::BTreeSet::new();
    let (mut dmin, mut dmax) = (1.0f64, 0.0f64);
    for inc in &t.incidents {
        for k in 0..=inc.decisive.len() {
            qs.insert((100.0 * k as f64 / inc.decisive.len() as f64) as u32 / 10);
        }
        dmin = dmin.min(inc.difficulty);
        dmax = dmax.max(inc.difficulty);
    }
    assert!(qs.len() >= 9, "q covers {qs:?}");
    assert!(dmin < 0.15 && dmax > 0.85, "d covers [{dmin}, {dmax}]");
}

#[test]
fn a_question_about_background_has_q_one_difficulty_background_and_the_answer_none() {
    let params = open_params(5, (-1.0, 5.0, 2.0));
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let focus = t
        .labels
        .iter()
        .position(|l| matches!(l, ObsLabel::Background(_)))
        .map(|i| crate::ObsId(i as u32))
        .unwrap();
    let p = sigma(-1.0 + 5.0 - 2.0 * 0.3);
    let (mut right, n) = (0u32, 3000u32);
    // Each call is about background, so the call index (per incident, here "none") counts them.
    for _ in 0..n {
        let StreamOutcome::Escalated { .. } = sim.apply(
            StreamAction::Escalate {
                context: vec![],
                question: Question::Diagnose { focus },
            },
            end,
        ) else {
            panic!()
        };
    }
    for e in sim.observe_until(Instant(params.duration_ns + 3_600_000_000_000)) {
        if let StreamEvent::Answered { answer, .. } = e {
            right += answer.diagnosis.is_none() as u32;
        }
    }
    let sd = (n as f64 * p * (1.0 - p)).sqrt();
    assert!(
        (right as f64 - n as f64 * p).abs() < 4.5 * sd,
        "{right}/{n} vs {p}"
    );
    for c in oracle::calls(&sim) {
        assert_eq!(c.incident, None);
        assert_eq!(c.q, 1.0);
        assert_eq!(c.d, 0.3);
    }
}

// ---- Cost

#[test]
fn a_call_costs_at_least_ten_thousand_typical_component_calls() {
    use gordian_components::WorkingState;
    use gordian_core::Resource;
    let p = StreamParams::new(0);
    let (s, _) = with_truth(&p);
    let public = s.public_info().world_public_info();
    let mut costs: Vec<u64> = Vec::new();
    for window in [16usize, 64, 256] {
        let mut state = WorkingState::new(public.clone(), window);
        for (at, o) in s.events().iter().take(window) {
            state.admit(*at, o.clone());
        }
        for c in gordian_run::standard_components() {
            let ns: u64 = c
                .declared_cost(&state)
                .iter()
                .filter(|ch| ch.resource == Resource::Compute)
                .map(|ch| ch.amount)
                .sum();
            costs.push(ns);
        }
    }
    costs.sort();
    let typical = costs[costs.len() / 2];
    let base = p.reasoner.cost.cost(0).modelled_ns;
    println!(
        "median component call {typical} ns, reasoner base {base} ns, ratio {}",
        base / typical
    );
    assert!(typical > 0);
    assert!(base >= 10_000 * typical, "base {base} vs typical {typical}");
    // The most expensive component call at the largest window is still far below one call.
    assert!(base >= 1_000 * *costs.last().unwrap());
    // The per-reference term is real: a context of 100 costs several bases.
    let c100 = p.reasoner.cost.cost(100);
    assert!(c100.modelled_ns > 3 * base && c100.tokens == 400 + 100 * 20);
    assert_eq!(c100.calls, 1);
}

#[test]
fn cost_is_charged_before_the_answer_exists_and_a_refused_call_leaves_no_trace() {
    let mut params = open_params(7, (-1.0, 5.0, 2.0));
    params.budget.reasoner_ns = params.reasoner.cost.cost(3).modelled_ns;
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s.clone());
    let mut fresh = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    fresh.observe_until(end);
    let inc = &t.incidents[0];
    let focus = inc.observations[0];
    let ctx = |n: usize| -> Vec<ObsRef> {
        inc.observations[..n]
            .iter()
            .map(|o| ObsRef::Passive(*o))
            .collect()
    };
    let ask = |sim: &mut StreamSimulator, n: usize| {
        sim.apply(
            StreamAction::Escalate {
                context: ctx(n),
                question: Question::Diagnose { focus },
            },
            end,
        )
    };
    let before = sim.remaining().reasoner_ns;
    // Too many references for what is left: refused, nothing charged, no answer queued.
    assert!(matches!(ask(&mut sim, 5), StreamOutcome::Refused(_)));
    assert_eq!(sim.remaining().reasoner_ns, before);
    assert!(oracle::calls(&sim).is_empty());
    assert!(
        sim.clone()
            .observe_until(Instant(end.0 + 1_000_000_000_000))
            .is_empty()
    );

    // The call that fits: the whole price is gone from the budget at the moment `apply`
    // returns, before any answer has been delivered.
    let StreamOutcome::Escalated { ready_at, cost, .. } = ask(&mut sim, 3) else {
        panic!()
    };
    assert_eq!(sim.remaining().reasoner_ns, before - cost.modelled_ns);
    assert_eq!(sim.remaining().reasoner_ns, 0);
    assert!(sim.observe_until(Instant(ready_at.0 - 1)).is_empty());

    // The refused call consumed no draw: the same call on a simulator that never saw the
    // refusal gets the same answer.
    let StreamOutcome::Escalated { .. } = ask(&mut fresh, 3) else {
        panic!()
    };
    let a = sim.observe_until(ready_at);
    let b = fresh.observe_until(ready_at);
    assert_eq!(a, b);
    assert!(matches!(a[0], StreamEvent::Answered { .. }));
}

// ---- Draws

#[test]
fn draws_are_keyed_by_incident_and_call_index_and_not_by_what_else_was_asked() {
    let params = open_params(9, (-1.0, 5.0, 2.0));
    let (s, t) = with_truth(&params);
    let end = Instant(params.duration_ns);
    let (x, y) = (&t.incidents[0], &t.incidents[1]);
    let ask = |sim: &mut StreamSimulator, inc: &crate::oracle::IncidentTruth, k: usize| {
        let ctx: Vec<ObsRef> = inc.decisive[..k.min(inc.decisive.len())]
            .iter()
            .map(|o| ObsRef::Passive(*o))
            .collect();
        sim.apply(
            StreamAction::Escalate {
                context: ctx,
                question: Question::Diagnose {
                    focus: inc.observations[0],
                },
            },
            end,
        )
    };
    let later = Instant(params.duration_ns + 3_600_000_000_000);
    let answers_about = |sim: &mut StreamSimulator| -> Vec<(u32, Diagnosis)> {
        let mut v: Vec<(u32, Diagnosis)> = sim
            .observe_until(later)
            .into_iter()
            .filter_map(|e| match e {
                StreamEvent::Answered { call, answer, .. } => Some((call, answer.diagnosis)),
                _ => None,
            })
            .collect();
        v.sort_by_key(|(call, _)| *call);
        v
    };
    // A: about x, three times. B: about y five times, then about x three times.
    let mut a = StreamSimulator::new(s.clone());
    a.observe_until(end);
    for _ in 0..3 {
        ask(&mut a, x, 1);
    }
    let ans_a = answers_about(&mut a);
    let mut b = StreamSimulator::new(s);
    b.observe_until(end);
    for _ in 0..5 {
        ask(&mut b, y, 2);
    }
    for _ in 0..3 {
        ask(&mut b, x, 1);
    }
    let ans_b: Vec<_> = answers_about(&mut b).into_iter().skip(5).collect();
    assert_eq!(
        ans_a.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
        ans_b.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
        "the answers about x depended on what else was asked"
    );
    // And the trace says the call index counts calls about that incident.
    let calls_b = oracle::calls(&b);
    let about_x: Vec<u32> = calls_b
        .iter()
        .filter(|c| c.incident == Some(x.id))
        .map(|c| c.call_index)
        .collect();
    assert_eq!(about_x, vec![0, 1, 2]);
}

#[test]
fn repeated_calls_with_the_same_context_are_independent_draws() {
    // An answer with probability near one half: asking again gives both outcomes.
    let params = open_params(9, (0.0, 0.0, 0.0));
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let inc = &t.incidents[0];
    for _ in 0..200 {
        sim.apply(
            StreamAction::Escalate {
                context: vec![],
                question: Question::Diagnose {
                    focus: inc.observations[0],
                },
            },
            end,
        );
    }
    let right = oracle::calls(&sim).iter().filter(|c| c.correct).count();
    assert!((60..=140).contains(&right), "{right} of 200 at p = 1/2");
}

// ---- Wrong answers are plausible

fn neighbours(services: &[gordian_world::Service], site: ServiceId) -> Vec<ServiceId> {
    let mut out: Vec<ServiceId> = services
        .iter()
        .filter(|s| {
            s.id != site
                && (s.depends_on.contains(&site)
                    || services[site.index()].depends_on.contains(&s.id))
        })
        .map(|s| s.id)
        .collect();
    if out.is_empty() {
        out.push(ServiceId(((site.index() + 1) % services.len()) as u32));
    }
    out
}

#[test]
fn a_wrong_answer_is_a_plausible_hypothesis_and_never_the_truth() {
    // a = b = c = 0 gives p = 1/2: half the answers are wrong, whatever the context.
    let params = open_params(13, (-3.0, 0.0, 0.0));
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let mut asked: BTreeMap<u32, u32> = BTreeMap::new();
    for inc in &t.incidents {
        for _ in 0..60 {
            let StreamOutcome::Escalated { call, .. } = sim.apply(
                StreamAction::Escalate {
                    context: vec![],
                    question: Question::Diagnose {
                        focus: inc.observations[0],
                    },
                },
                end,
            ) else {
                panic!()
            };
            asked.insert(call, inc.id);
        }
    }
    let mut seen: BTreeMap<Tier, std::collections::BTreeSet<Diagnosis>> = BTreeMap::new();
    let mut wrong = 0;
    for e in sim.observe_until(Instant(params.duration_ns + 3_600_000_000_000)) {
        let StreamEvent::Answered { call, answer, .. } = e else {
            continue;
        };
        let inc = &t.incidents[asked[&call] as usize];
        if answer.diagnosis == inc.truth {
            continue;
        }
        wrong += 1;
        let site = inc.occupies[0];
        if let Some(h) = answer.diagnosis {
            assert!(h.site.index() < t.services.len());
        }
        seen.entry(inc.tier).or_default().insert(answer.diagnosis);
        let near = neighbours(&t.services, site);
        match inc.tier {
            Tier::Plain => {
                let k = inc.shape.known_kind.unwrap();
                match answer.diagnosis {
                    None => {}
                    Some(StreamHypothesis {
                        kind: StreamKind::Known(w),
                        site: at,
                    }) => {
                        assert!(
                            (at == site && w != k) || (w == k && near.contains(&at)),
                            "plain {k:?}@{site:?}: {:?}",
                            answer.diagnosis
                        );
                    }
                    other => panic!("plain incident answered {other:?}"),
                }
            }
            Tier::Hard => {
                let hk = inc.shape.hard_kind.unwrap();
                match answer.diagnosis {
                    None => {}
                    Some(StreamHypothesis {
                        kind: StreamKind::Known(w),
                        site: at,
                    }) => {
                        assert_eq!((Some(w), at), (inc.shape.mimics, site));
                    }
                    Some(StreamHypothesis {
                        kind: StreamKind::Hard(w),
                        site: at,
                    }) => {
                        assert!(
                            (at == site && w != hk) || (w == hk && near.contains(&at)),
                            "hard {hk:?}@{site:?}: {:?}",
                            answer.diagnosis
                        );
                    }
                }
            }
            Tier::Decoy => {
                let hk = inc.shape.hard_kind.unwrap();
                let h = answer
                    .diagnosis
                    .expect("a wrong answer about a decoy names an incident");
                assert_eq!(h.site, site);
                match h.kind {
                    StreamKind::Hard(_) => {}
                    StreamKind::Known(w) => assert_eq!(Some(w), inc.shape.mimics, "{hk:?}"),
                }
            }
        }
    }
    assert!(wrong > 1000, "{wrong}");
    // The wrong answers are varied, not one constant.
    for tier in [Tier::Plain, Tier::Hard, Tier::Decoy] {
        assert!(seen[&tier].len() >= 3, "{tier:?}: {:?}", seen[&tier]);
    }
}

#[test]
fn the_most_common_wrong_answer_about_a_hard_incident_is_the_kind_the_cheap_rung_is_led_to() {
    // p is almost zero, so almost every answer is wrong.
    let mut params = open_params(14, (-6.0, 0.0, 0.0));
    params.mix.plain_permille = 0;
    params.mix.hard_permille = 1000;
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let mut asked = BTreeMap::new();
    for inc in &t.incidents {
        for _ in 0..80 {
            if let StreamOutcome::Escalated { call, .. } = sim.apply(
                StreamAction::Escalate {
                    context: vec![],
                    question: Question::Diagnose {
                        focus: inc.observations[0],
                    },
                },
                end,
            ) {
                asked.insert(call, inc.id);
            }
        }
    }
    let (mut mimic, mut total) = (0u32, 0u32);
    for e in sim.observe_until(Instant(params.duration_ns + 3_600_000_000_000)) {
        if let StreamEvent::Answered { call, answer, .. } = e {
            let inc = &t.incidents[asked[&call] as usize];
            total += 1;
            if let Some(h) = answer.diagnosis
                && matches!(h.kind, StreamKind::Known(k) if Some(k) == inc.shape.mimics)
            {
                mimic += 1;
            }
        }
    }
    // Weight 4 of a pool of total weight about 11 to 12.
    let share = mimic as f64 / total as f64;
    assert!((0.28..0.45).contains(&share), "{share}");
}

#[test]
fn no_incident_is_ever_answered_with_a_service_outside_the_graph() {
    let params = open_params(3, (-2.0, 1.0, 1.0));
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    for inc in &t.incidents {
        for _ in 0..30 {
            sim.apply(
                StreamAction::Escalate {
                    context: vec![],
                    question: Question::Diagnose {
                        focus: inc.observations[0],
                    },
                },
                end,
            );
        }
    }
    for e in sim.observe_until(Instant(params.duration_ns + 3_600_000_000_000)) {
        if let StreamEvent::Answered { answer, .. } = e
            && let Some(h) = answer.diagnosis
        {
            assert!(h.site.index() < t.services.len());
        }
    }
}
