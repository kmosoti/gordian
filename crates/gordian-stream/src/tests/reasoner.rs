//! The simulated reasoner: its law, its invariant, its cost, its draws.
//!
//! Two properties matter most and each has its own tests:
//!
//! 1. *No information from no evidence.* The answer depends on the truth only through the decisive
//!    evidence in the context. At `q = 0` the answer is a guess computed from the context and the
//!    public rules, and carries no information about the truth beyond what the context's public
//!    content does.
//! 2. *Repeating a question buys nothing.* An identical context gets an identical answer; different
//!    contexts about one incident are correlated through a Gaussian copula.

use super::*;
use crate::kinds::Tier;
use crate::oracle::{self, IncidentTruth, ObsLabel};
use crate::rng::Gen;
use crate::rng::tests::phi;
use crate::stream::{Forced, generate_forced};
use crate::{
    Diagnosis, ObsId, ObsRef, Question, StreamAction, StreamEvent, StreamKind, StreamOutcome,
    StreamSimulator,
};
use gordian_core::Instant;
use std::collections::BTreeMap;

/// The logistic function, with the standard library's `exp`, independent of the crate's own.
fn sigma(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// The probability that a call is informed, computed here from the definition.
fn h_std(abc: (f64, f64, f64), q: f64, d: f64) -> f64 {
    let base = sigma(abc.0 - abc.2 * d);
    let full = sigma(abc.0 + abc.1 * q - abc.2 * d);
    ((full - base) / (1.0 - base)).clamp(0.0, 1.0)
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

const LATER: u64 = 3_600_000_000_000;

fn ask(sim: &mut StreamSimulator, focus: ObsId, context: Vec<ObsRef>, now: Instant) -> u32 {
    match sim.apply(
        StreamAction::Escalate {
            context,
            question: Question::Diagnose { focus },
        },
        now,
    ) {
        StreamOutcome::Escalated { call, .. } => call,
        o => panic!("refused: {o:?}"),
    }
}

/// Every answer delivered by `until`, by call.
fn answers(sim: &mut StreamSimulator, until: Instant) -> BTreeMap<u32, Diagnosis> {
    sim.observe_until(until)
        .into_iter()
        .filter_map(|e| match e {
            StreamEvent::Answered { call, answer, .. } => Some((call, answer.diagnosis)),
            _ => None,
        })
        .collect()
}

/// A cluster-robust z statistic for `sum (observed - expected)` over independent clusters: each
/// cluster's total excess is one draw, and the variance is estimated from them. Valid when the
/// calls inside a cluster are dependent (as calls about one incident are), given enough clusters.
fn cluster_z(clusters: &BTreeMap<u32, f64>) -> f64 {
    let sum: f64 = clusters.values().sum();
    let ss: f64 = clusters.values().map(|e| e * e).sum();
    if ss == 0.0 { 0.0 } else { sum / ss.sqrt() }
}

// ---- The law

#[test]
fn the_informed_probability_is_zero_without_evidence_and_rises_with_it() {
    for abc in [
        (-1.0, 5.0, 2.0),
        (0.0, 3.0, 1.0),
        (-2.0, 6.0, 4.0),
        (1.5, 1.0, 0.5),
    ] {
        let mut spec = StreamParams::new(0).reasoner;
        (spec.a, spec.b, spec.c) = abc;
        for di in 0..=10 {
            let d = di as f64 / 10.0;
            assert_eq!(
                crate::reasoner::informed_probability(&spec, 0.0, d),
                0.0,
                "{abc:?} d {d}"
            );
            let mut prev = 0.0;
            for qi in 1..=10 {
                let q = qi as f64 / 10.0;
                let h = crate::reasoner::informed_probability(&spec, q, d);
                assert!(
                    h > prev,
                    "{abc:?}: h is not increasing in q at q {q}, d {d}"
                );
                assert!((h - h_std(abc, q, d)).abs() < 1e-12, "{abc:?} q {q} d {d}");
                prev = h;
            }
            assert!(prev <= 1.0);
        }
    }
}

struct Row {
    incident: u32,
    k: usize,
    d: f64,
    q: f64,
    h: f64,
    p0: f64,
    informed: bool,
    correct: bool,
}

/// Deliver everything, ask `rounds` questions about every incident with varying amounts of its
/// decisive evidence in the context and some noise, collect every answer. Correctness is judged by
/// comparing the *delivered answer* with the truth, not by the simulator's own flag.
fn run_law(params: &StreamParams, rounds: usize) -> Vec<Row> {
    let (stream, truth) = with_truth(params);
    let mut sim = StreamSimulator::new(stream);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    let mut rng = Gen::keyed(&[params.seed, 0xFEED]);
    let n_obs = truth.labels.len() as u64;
    let mut asked: BTreeMap<u32, (u32, usize)> = BTreeMap::new();
    for inc in &truth.incidents {
        let m = inc.decisive.len();
        for round in 0..rounds {
            let k = round % (m + 1);
            let mut context: Vec<ObsRef> = inc.decisive[..k]
                .iter()
                .map(|o| ObsRef::Passive(*o))
                .collect();
            for _ in 0..rng.below(6) {
                let o = ObsId(rng.below(n_obs) as u32);
                let decisive_here = matches!(
                    truth.labels[o.0 as usize],
                    ObsLabel::Incident { id, role: crate::labels::EvidenceRole::Decisive } if id == inc.id
                );
                let r = ObsRef::Passive(o);
                if !decisive_here && !context.contains(&r) {
                    context.push(r);
                }
            }
            let call = ask(&mut sim, inc.observations[0], context, end);
            asked.insert(call, (inc.id, k));
        }
    }
    let delivered = answers(&mut sim, Instant(params.duration_ns + LATER));
    let traced = oracle::calls(&sim);
    assert_eq!(traced.len(), asked.len());
    let mut rows = Vec::new();
    for (call, (inc_id, k)) in asked {
        let inc = &truth.incidents[inc_id as usize];
        let t = &traced[call as usize];
        let q = k as f64 / inc.decisive.len() as f64;
        let correct = delivered[&call] == inc.truth;
        assert_eq!(correct, t.correct, "trace and answer disagree");
        assert!((t.q - q).abs() < 1e-12 && (t.d - inc.difficulty).abs() < 1e-12);
        rows.push(Row {
            incident: inc_id,
            k,
            d: inc.difficulty,
            q,
            h: t.h,
            p0: t.p0,
            informed: t.informed,
            correct,
        });
    }
    rows
}

fn check_law(rows: &[Row], abc: (f64, f64, f64), label: &str) {
    assert!(rows.len() >= 10_000, "{label}: only {} calls", rows.len());
    // The crate's informed probability is the definition's, computed here with std's exp.
    for r in rows {
        assert!((r.h - h_std(abc, r.q, r.d)).abs() < 1e-9, "{label}");
        assert!(
            !r.informed || r.q > 0.0,
            "{label}: informed without evidence"
        );
    }
    let expected = |r: &Row| r.p0 + (1.0 - r.p0) * h_std(abc, r.q, r.d);
    let z_of = |keep: &dyn Fn(&Row) -> bool| -> (f64, usize) {
        let mut clusters: BTreeMap<u32, f64> = BTreeMap::new();
        let mut n = 0;
        for r in rows.iter().filter(|r| keep(r)) {
            *clusters.entry(r.incident).or_insert(0.0) += (r.correct as u8 as f64) - expected(r);
            n += 1;
        }
        (cluster_z(&clusters), n)
    };
    let mut report = Vec::new();
    for (name, keep) in [
        ("all", Box::new(|_: &Row| true) as Box<dyn Fn(&Row) -> bool>),
        ("q = 0", Box::new(|r: &Row| r.q == 0.0)),
        ("0 < q < 1", Box::new(|r: &Row| r.q > 0.0 && r.q < 1.0)),
        ("q = 1", Box::new(|r: &Row| r.q == 1.0)),
        ("d < 0.4", Box::new(|r: &Row| r.d < 0.4)),
        (
            "0.4 <= d < 0.7",
            Box::new(|r: &Row| r.d >= 0.4 && r.d < 0.7),
        ),
        ("d >= 0.7", Box::new(|r: &Row| r.d >= 0.7)),
        ("expected p < 0.5", Box::new(|r: &Row| expected(r) < 0.5)),
        ("expected p >= 0.5", Box::new(|r: &Row| expected(r) >= 0.5)),
    ] {
        let (z, n) = z_of(&*keep);
        report.push(format!("{name}: {n} calls z {z:.2}"));
        if n >= 300 {
            assert!(
                z.abs() < 4.5,
                "{label}, {name}: cluster-robust z = {z} over {n} calls"
            );
        }
    }
    println!("{label}: {} calls; {}", rows.len(), report.join("; "));
    // Without evidence the accuracy is exactly the guess's: informed is never true there.
    let zero: Vec<&Row> = rows.iter().filter(|r| r.k == 0).collect();
    assert!(!zero.is_empty() && zero.iter().all(|r| !r.informed && r.h == 0.0));
}

fn law_rows(seeds: std::ops::Range<u64>, abc: (f64, f64, f64), rho: f64) -> Vec<Row> {
    let mut rows = Vec::new();
    for seed in seeds {
        let mut p = open_params(seed, abc);
        p.reasoner.rho = rho;
        for mut r in run_law(&p, 30) {
            // Clusters must not collide across streams.
            r.incident += (seed as u32) * 10_000;
            rows.push(r);
        }
    }
    rows
}

#[test]
fn the_reasoners_accuracy_follows_the_law_at_the_defaults() {
    let abc = (-1.0, 5.0, 2.0);
    check_law(
        &law_rows(11..19, abc, 0.7),
        abc,
        "default (a, b, c) = (-1, 5, 2), rho 0.7",
    );
}

#[test]
fn the_law_holds_across_other_settings_and_correlations() {
    for (i, (abc, rho)) in [
        ((0.0, 3.0, 1.0), 0.0),
        ((-2.0, 6.0, 4.0), 0.7),
        ((1.5, 1.0, 0.5), 0.95),
        ((-1.0, 5.0, 2.0), 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let rows = law_rows(30 + 10 * i as u64..38 + 10 * i as u64, abc, rho);
        check_law(&rows, abc, &format!("(a, b, c) = {abc:?}, rho {rho}"));
    }
}

#[test]
fn the_law_covers_a_wide_range_of_q_and_d() {
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

// ---- Invariant 1: no information from no evidence

fn category(d: &Diagnosis) -> usize {
    match d {
        None => 0,
        Some(h) => match h.kind {
            StreamKind::Known(k) => 1 + k as usize,
            StreamKind::Hard(k) => 6 + k as usize,
        },
    }
}

fn mi(xs: &[usize], ys: &[usize]) -> f64 {
    let n = xs.len() as f64;
    let mut joint: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let mut px: BTreeMap<usize, f64> = BTreeMap::new();
    let mut py: BTreeMap<usize, f64> = BTreeMap::new();
    for (x, y) in xs.iter().zip(ys) {
        *joint.entry((*x, *y)).or_insert(0.0) += 1.0;
        *px.entry(*x).or_insert(0.0) += 1.0;
        *py.entry(*y).or_insert(0.0) += 1.0;
    }
    joint
        .iter()
        .map(|((x, y), c)| {
            let pxy = c / n;
            pxy * (pxy / ((px[x] / n) * (py[y] / n))).ln()
        })
        .sum()
}

fn permute(ys: &[usize], g: &mut Gen) -> Vec<usize> {
    let mut v = ys.to_vec();
    for i in (1..v.len()).rev() {
        v.swap(i, g.below(i as u64 + 1) as usize);
    }
    v
}

/// The mutual information of `xs` and `ys` and the largest of 100 permutations of `ys`.
fn mi_against_baseline(xs: &[usize], ys: &[usize], g: &mut Gen) -> (f64, f64) {
    let m = mi(xs, ys);
    let max = (0..100)
        .map(|_| mi(xs, &permute(ys, g)))
        .fold(0.0, f64::max);
    (m, max)
}

#[test]
fn without_decisive_evidence_the_answer_carries_no_information_about_the_truth() {
    // Over many incidents of every tier, ask with no decisive evidence in the context, in two
    // ways: an empty context, and a context of random background observations. The answer's
    // mutual information with the truth stays within its permutation baseline. Then ask with all
    // the decisive evidence: the information is there, so the test could have failed.
    let (mut truths, mut empty, mut noisy, mut full) = (vec![], vec![], vec![], vec![]);
    for seed in 0..30 {
        let mut p = open_params(seed, (-1.0, 5.0, 2.0));
        p.mix.plain_permille = 400;
        p.mix.hard_permille = 300;
        p.recurrence_permille = 0;
        let (s, t) = with_truth(&p);
        let end = Instant(p.duration_ns);
        let mut sim = StreamSimulator::new(s);
        sim.observe_until(end);
        let background: Vec<ObsRef> = t
            .labels
            .iter()
            .enumerate()
            .filter(|(_, l)| matches!(l, ObsLabel::Background(_)))
            .map(|(i, _)| ObsRef::Passive(ObsId(i as u32)))
            .collect();
        let mut rng = Gen::keyed(&[seed, 0xB6]);
        let mut calls: Vec<(u32, u32, u32, usize)> = Vec::new();
        for inc in &t.incidents {
            let focus = inc.observations[0];
            let c0 = ask(&mut sim, focus, vec![], end);
            let mut ctx = Vec::new();
            while ctx.len() < 8 {
                let r = background[rng.below(background.len() as u64) as usize];
                if !ctx.contains(&r) {
                    ctx.push(r);
                }
            }
            let c1 = ask(&mut sim, focus, ctx, end);
            let c2 = ask(
                &mut sim,
                focus,
                inc.decisive.iter().map(|o| ObsRef::Passive(*o)).collect(),
                end,
            );
            calls.push((c0, c1, c2, category(&inc.truth)));
        }
        let got = answers(&mut sim, Instant(p.duration_ns + LATER));
        for (c0, c1, c2, truth) in calls {
            truths.push(truth);
            empty.push(category(&got[&c0]));
            noisy.push(category(&got[&c1]));
            full.push(category(&got[&c2]));
        }
        for c in oracle::calls(&sim).iter().filter(|c| c.q == 0.0) {
            assert!(!c.informed);
        }
    }
    let mut g = Gen::keyed(&[0x1F0]);
    let (m_empty, b_empty) = mi_against_baseline(&empty, &truths, &mut g);
    let (m_noisy, b_noisy) = mi_against_baseline(&noisy, &truths, &mut g);
    let (m_full, b_full) = mi_against_baseline(&full, &truths, &mut g);
    println!(
        "{} incidents; MI(answer, truth): empty context {m_empty:.4} (baseline max {b_empty:.4}), \
         background context {m_noisy:.4} ({b_noisy:.4}), all decisive evidence {m_full:.4} ({b_full:.4})",
        truths.len()
    );
    assert!(m_empty <= b_empty, "{m_empty} > {b_empty}");
    assert!(m_noisy <= b_noisy, "{m_noisy} > {b_noisy}");
    assert!(
        m_full > 10.0 * b_full,
        "the control does not separate: {m_full} vs {b_full}"
    );
    // An uninformed answer never names a hard kind: its vocabulary is the public physics.
    assert!(empty.iter().chain(&noisy).all(|c| *c <= 5));
}

#[test]
fn an_answer_with_no_decisive_evidence_is_the_public_readings_guess_and_nothing_more() {
    // Context: the incident's own first-moments evidence, which is public and not decisive for a
    // hard incident or a decoy. A mimic of a compound or cascade reads as its imitated kind at its
    // site to the public checker, and that is what the reasoner answers without evidence.
    let (mut n, mut imitated) = (0, 0);
    for seed in 0..12 {
        let mut p = open_params(seed, (-1.0, 5.0, 2.0));
        p.mix.plain_permille = 0;
        p.mix.hard_permille = 500;
        p.recurrence_permille = 0;
        // After a regime change the public rules are stale and the guess would be a stale
        // reading; that is the rules' staleness, not what this test is about.
        p.regimes.clear();
        let (s, t) = with_truth(&p);
        let end = Instant(p.duration_ns);
        let mut sim = StreamSimulator::new(s.clone());
        sim.observe_until(end);
        let mut asked = Vec::new();
        for inc in &t.incidents {
            use crate::HardKind::{Cascade, Compound};
            if !(matches!(inc.shape.hard_kind, Some(Compound) | Some(Cascade))
                && inc.shape.contradicts_early == Some(false))
            {
                continue;
            }
            let burst: Vec<ObsRef> = inc
                .observations
                .iter()
                .filter(|o| s.events()[o.0 as usize].0.0 < inc.onset_ns + 300_000_000)
                .filter(|o| !inc.decisive.contains(o))
                .map(|o| ObsRef::Passive(*o))
                .collect();
            let call = ask(&mut sim, inc.observations[0], burst, end);
            asked.push((call, inc.clone()));
        }
        let got = answers(&mut sim, Instant(p.duration_ns + LATER));
        for (call, inc) in asked {
            n += 1;
            let want = inc.shape.mimics.map(|k| crate::StreamHypothesis {
                kind: StreamKind::Known(k),
                site: inc.occupies[0],
            });
            imitated += (got[&call] == want) as u32;
        }
    }
    println!("{n} mimics asked with their first moments");
    assert!(n > 60, "{n}");
    assert_eq!(
        imitated, n,
        "the guess from the public rules is the imitated kind"
    );
}

#[test]
fn a_hard_incident_and_a_decoy_are_answered_alike_when_no_public_evidence_separates_them() {
    // (a) By construction: swap the tier of an incident after every draw is made; ask about it
    //     with its first six seconds of evidence in the context (not decisive for either tier).
    //     The two simulators give the same answer, because nothing the reasoner reads and no draw
    //     it uses depends on the tier until decisive evidence is in the context.
    let t0 = crate::timing::T0_NS;
    let mut tested = 0;
    for seed in 0..10 {
        let mut p = open_params(seed, (-1.0, 5.0, 2.0));
        p.mix.plain_permille = 0;
        p.mix.hard_permille = 500;
        p.recurrence_permille = 0;
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            let to = if inc.tier == Tier::Hard {
                Tier::Decoy
            } else {
                Tier::Hard
            };
            let forced = generate_forced(
                &p,
                Forced {
                    arrival: inc.arrival,
                    tier: Some(to),
                    duo_kind: None,
                },
            );
            let now = Instant(inc.onset_ns + t0);
            let phase1: Vec<ObsId> = inc
                .observations
                .iter()
                .copied()
                .filter(|o| s.events()[o.0 as usize].0 < now)
                .collect();
            let mut results = Vec::new();
            for stream in [s.clone(), forced] {
                let mut sim = StreamSimulator::new(stream);
                sim.observe_until(now);
                let ctx = phase1.iter().map(|o| ObsRef::Passive(*o)).collect();
                let call = ask(&mut sim, phase1[0], ctx, now);
                let got = answers(&mut sim, Instant(now.0 + LATER));
                let trace = oracle::calls(&sim);
                assert_eq!(trace[0].q, 0.0, "phase 1 must hold no decisive evidence");
                assert!(!trace[0].informed);
                results.push(got[&call]);
            }
            assert_eq!(
                results[0], results[1],
                "seed {seed} incident {}: the answer depends on the tier",
                inc.id
            );
            tested += 1;
        }
    }
    println!("{tested} incidents swapped and asked");
    assert!(tested > 150, "{tested}");

    // (b) Statistically, on unswapped incidents: the answer's category against the tier.
    let (mut tiers, mut at_zero, mut at_full) = (vec![], vec![], vec![]);
    for seed in 0..40 {
        let mut p = open_params(seed, (-1.0, 5.0, 2.0));
        p.mix.plain_permille = 0;
        p.mix.hard_permille = 500;
        p.recurrence_permille = 0;
        let (s, t) = with_truth(&p);
        let mut sim = StreamSimulator::new(s.clone());
        let end = Instant(p.duration_ns);
        sim.observe_until(end);
        let mut asked = Vec::new();
        for inc in &t.incidents {
            let cut = Instant(inc.onset_ns + t0);
            let phase1: Vec<ObsRef> = inc
                .observations
                .iter()
                .filter(|o| s.events()[o.0 as usize].0 < cut)
                .map(|o| ObsRef::Passive(*o))
                .collect();
            let c0 = ask(&mut sim, inc.observations[0], phase1.clone(), end);
            let mut all = phase1;
            all.extend(inc.decisive.iter().map(|o| ObsRef::Passive(*o)));
            all.sort();
            all.dedup();
            let c1 = ask(&mut sim, inc.observations[0], all, end);
            asked.push((c0, c1, (inc.tier == Tier::Hard) as usize));
        }
        let got = answers(&mut sim, Instant(p.duration_ns + LATER));
        for (c0, c1, tier) in asked {
            tiers.push(tier);
            at_zero.push(category(&got[&c0]));
            at_full.push(category(&got[&c1]));
        }
    }
    let mut g = Gen::keyed(&[0xDEC0]);
    let (m0, b0) = mi_against_baseline(&at_zero, &tiers, &mut g);
    let (m1, b1) = mi_against_baseline(&at_full, &tiers, &mut g);
    println!(
        "{} hard and decoy incidents; MI(answer, tier): phase-1 context {m0:.4} (baseline max \
         {b0:.4}); with decisive evidence {m1:.4} ({b1:.4})",
        tiers.len()
    );
    assert!(m0 <= b0, "{m0} > {b0}");
    assert!(
        m1 > 10.0 * b1,
        "the control does not separate: {m1} vs {b1}"
    );
}

// ---- Cost

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
    let try_ask = |sim: &mut StreamSimulator, n: usize| {
        sim.apply(
            StreamAction::Escalate {
                context: ctx(n),
                question: Question::Diagnose { focus },
            },
            end,
        )
    };
    let before = sim.remaining().reasoner_ns;
    assert!(matches!(try_ask(&mut sim, 5), StreamOutcome::Refused(_)));
    assert_eq!(sim.remaining().reasoner_ns, before);
    assert!(oracle::calls(&sim).is_empty());
    assert!(
        sim.clone()
            .observe_until(Instant(end.0 + 1_000_000_000_000))
            .is_empty()
    );

    let StreamOutcome::Escalated { ready_at, cost, .. } = try_ask(&mut sim, 3) else {
        panic!()
    };
    assert_eq!(sim.remaining().reasoner_ns, before - cost.modelled_ns);
    assert_eq!(sim.remaining().reasoner_ns, 0);
    assert!(sim.observe_until(Instant(ready_at.0 - 1)).is_empty());

    let StreamOutcome::Escalated { .. } = try_ask(&mut fresh, 3) else {
        panic!()
    };
    let a = sim.observe_until(ready_at);
    let b = fresh.observe_until(ready_at);
    assert_eq!(a, b);
    assert!(matches!(a[0], StreamEvent::Answered { .. }));
}

// ---- Invariant 2: repeating a question buys nothing

#[test]
fn the_same_question_gets_the_same_answer_however_often_and_in_whatever_order() {
    let params = open_params(9, (-1.0, 3.0, 1.0));
    let (s, t) = with_truth(&params);
    let end = Instant(params.duration_ns);
    let mut sim = StreamSimulator::new(s);
    sim.observe_until(end);
    let mut rng = Gen::keyed(&[9, 0xAA]);
    let mut plan = Vec::new();
    for inc in t.incidents.iter().take(40) {
        let focus = inc.observations[0];
        let half = inc.decisive.len() / 2;
        let base: Vec<ObsRef> = inc.decisive[..half]
            .iter()
            .map(|o| ObsRef::Passive(*o))
            .collect();
        let mut shuffled = base.clone();
        shuffled.reverse();
        let before = sim.remaining().reasoner_ns;
        let calls: Vec<u32> = (0..3)
            .map(|i| {
                ask(
                    &mut sim,
                    focus,
                    if i == 1 {
                        shuffled.clone()
                    } else {
                        base.clone()
                    },
                    end,
                )
            })
            .collect();
        // Every repeat is paid for.
        assert!(sim.remaining().reasoner_ns < before);
        // Different contexts about the same incident: forty of them.
        let others: Vec<u32> = (0..40)
            .map(|_| {
                let mut c = base.clone();
                c.push(ObsRef::Passive(ObsId(
                    rng.below(t.labels.len() as u64) as u32
                )));
                c.sort();
                c.dedup();
                ask(&mut sim, focus, c, end)
            })
            .collect();
        plan.push((calls, others));
    }
    let got = answers(&mut sim, Instant(params.duration_ns + LATER));
    let mut differing_contexts = 0;
    for (calls, others) in &plan {
        assert_eq!(
            got[&calls[0]], got[&calls[1]],
            "order of references changed the answer"
        );
        assert_eq!(
            got[&calls[0]], got[&calls[2]],
            "repeating the question changed the answer"
        );
        let distinct: std::collections::BTreeSet<_> = others.iter().map(|c| got[c]).collect();
        differing_contexts += (distinct.len() > 1) as u32;
    }
    assert_eq!(plan.len(), 40);
    assert!(
        differing_contexts >= 10,
        "different contexts never differ: {differing_contexts}"
    );
}

#[test]
fn draws_are_keyed_by_incident_and_context_and_not_by_what_else_was_asked() {
    let params = open_params(9, (-1.0, 3.0, 1.0));
    let (s, t) = with_truth(&params);
    let end = Instant(params.duration_ns);
    let (x, y) = (&t.incidents[0], &t.incidents[1]);
    let refs = |inc: &IncidentTruth, n: usize| -> Vec<ObsRef> {
        inc.decisive[..n.min(inc.decisive.len())]
            .iter()
            .map(|o| ObsRef::Passive(*o))
            .collect()
    };
    let mut a = StreamSimulator::new(s.clone());
    a.observe_until(end);
    let ca = ask(&mut a, x.observations[0], refs(x, 1), end);
    let ans_a = answers(&mut a, Instant(params.duration_ns + LATER))[&ca];
    let mut b = StreamSimulator::new(s);
    b.observe_until(end);
    for n in 0..6 {
        ask(&mut b, y.observations[0], refs(y, n), end);
    }
    let cb = ask(&mut b, x.observations[0], refs(x, 1), end);
    let ans_b = answers(&mut b, Instant(params.duration_ns + LATER))[&cb];
    assert_eq!(
        ans_a, ans_b,
        "the answer about x depended on what else was asked"
    );
    let calls = oracle::calls(&b);
    assert_eq!(
        calls[cb as usize].fingerprint,
        oracle::calls(&a)[ca as usize].fingerprint
    );
}

/// The probability that at least two of three calls about one incident are informed, when each is
/// informed with probability `h` and their informed-ness is a Gaussian copula with correlation
/// `rho`: the integral over the incident's normal draw of the conditional majority probability.
fn majority_of_three(h: f64, rho: f64) -> f64 {
    if h <= 0.0 {
        return 0.0;
    }
    if h >= 1.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (-8.0f64, 8.0f64);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if phi(mid) < h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    let (mut sum, step) = (0.0f64, 0.01f64);
    let mut z = -8.0f64;
    while z <= 8.0 {
        let dens = (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let p = if rho >= 1.0 {
            if z < t { 1.0 } else { 0.0 }
        } else {
            phi((t - rho.sqrt() * z) / (1.0 - rho).sqrt())
        };
        sum += dens * (3.0 * p * p * (1.0 - p) + p * p * p) * step;
        z += step;
    }
    sum
}

/// Per hard incident, `triples` groups of three different contexts, each holding all the decisive
/// evidence plus one distinct background observation. Returns, per incident, the difficulty, the
/// fraction of triples whose majority was right, and the fraction of single calls right.
fn majority_runs(rho: f64, streams: u64, triples: usize) -> Vec<(f64, f64, f64)> {
    let abc = (-1.0, 3.5, 2.0);
    let mut out = Vec::new();
    for seed in 0..streams {
        let mut p = open_params(seed, abc);
        p.reasoner.rho = rho;
        p.mean_gap_ns = 4_000_000_000;
        p.mix.plain_permille = 0;
        p.mix.hard_permille = 1000;
        p.recurrence_permille = 0;
        let (s, t) = with_truth(&p);
        let end = Instant(p.duration_ns);
        let mut sim = StreamSimulator::new(s);
        sim.observe_until(end);
        let background: Vec<ObsId> = t
            .labels
            .iter()
            .enumerate()
            .filter(|(_, l)| matches!(l, ObsLabel::Background(_)))
            .map(|(i, _)| ObsId(i as u32))
            .collect();
        let mut rng = Gen::keyed(&[seed, 0xC0]);
        let mut plan = Vec::new();
        for inc in &t.incidents {
            let mut groups = Vec::new();
            for _ in 0..triples {
                let mut three = [0u32; 3];
                let mut used = std::collections::BTreeSet::new();
                for slot in three.iter_mut() {
                    let extra = loop {
                        let o = background[rng.below(background.len() as u64) as usize];
                        if used.insert(o) {
                            break o;
                        }
                    };
                    let mut ctx: Vec<ObsRef> =
                        inc.decisive.iter().map(|o| ObsRef::Passive(*o)).collect();
                    ctx.push(ObsRef::Passive(extra));
                    *slot = ask(&mut sim, inc.observations[0], ctx, end);
                }
                groups.push(three);
            }
            plan.push((inc.difficulty, inc.truth, groups));
        }
        let got = answers(&mut sim, Instant(p.duration_ns + LATER));
        for (d, truth, groups) in plan {
            let mut maj = 0.0;
            let mut single = 0.0;
            for g in &groups {
                let right: Vec<bool> = g.iter().map(|c| got[c] == truth).collect();
                maj += (right.iter().filter(|r| **r).count() >= 2) as u8 as f64;
                single += right.iter().filter(|r| **r).count() as f64 / 3.0;
            }
            out.push((d, maj / groups.len() as f64, single / groups.len() as f64));
        }
    }
    out
}

#[test]
fn majority_of_three_different_contexts_follows_the_copula_and_is_worse_than_independence() {
    let abc = (-1.0, 3.5, 2.0);
    for rho in [0.0f64, 0.7, 0.95] {
        let runs = majority_runs(rho, 40, 4);
        let n = runs.len();
        assert!(n >= 800, "rho {rho}: only {n} incidents");
        // Hard incidents: a guess never equals the truth, so a call is right iff it is informed.
        let mut vs_copula = BTreeMap::new();
        let mut vs_independent = BTreeMap::new();
        let mut marginal = BTreeMap::new();
        let (mut obs_sum, mut copula_sum, mut indep_sum) = (0.0, 0.0, 0.0);
        for (i, (d, maj, single)) in runs.iter().enumerate() {
            let h = h_std(abc, 1.0, *d);
            let m_copula = majority_of_three(h, rho);
            let m_indep = 3.0 * h * h - 2.0 * h * h * h;
            vs_copula.insert(i as u32, maj - m_copula);
            vs_independent.insert(i as u32, maj - m_indep);
            marginal.insert(i as u32, single - h);
            obs_sum += maj;
            copula_sum += m_copula;
            indep_sum += m_indep;
        }
        let (zc, zi, zm) = (
            cluster_z(&vs_copula),
            cluster_z(&vs_independent),
            cluster_z(&marginal),
        );
        println!(
            "rho {rho}: {n} incidents; majority-of-three right {:.4}; copula predicts {:.4} (z {zc:.2}); \
             independence predicts {:.4} (z {zi:.2}); single-call accuracy z {zm:.2}",
            obs_sum / n as f64,
            copula_sum / n as f64,
            indep_sum / n as f64
        );
        assert!(
            zc.abs() < 4.0,
            "rho {rho}: majority off the copula's prediction, z {zc}"
        );
        assert!(
            zm.abs() < 4.0,
            "rho {rho}: marginal accuracy off the law, z {zm}"
        );
        if rho == 0.0 {
            assert!(zi.abs() < 4.0, "rho 0 must be independence, z {zi}");
        } else {
            // Positive correlation lowers the majority's accuracy when single calls are right
            // more than half the time, which they are here.
            assert!(zi < -4.0, "rho {rho}: not worse than independence, z {zi}");
            assert!(copula_sum < indep_sum);
        }
    }
}

// ---- Background, graph, and the rest

#[test]
fn a_question_about_background_has_q_one_the_background_difficulty_and_the_answer_none() {
    let abc = (-1.0, 5.0, 2.0);
    let params = open_params(5, abc);
    let (s, t) = with_truth(&params);
    let end = Instant(params.duration_ns);
    let mut sim = StreamSimulator::new(s.clone());
    sim.observe_until(end);
    let foci: Vec<ObsId> = t
        .labels
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l, ObsLabel::Background(_)))
        .map(|(i, _)| ObsId(i as u32))
        .take(3000)
        .collect();
    let calls: Vec<u32> = foci
        .iter()
        .map(|f| ask(&mut sim, *f, vec![], end))
        .collect();
    let got = answers(&mut sim, Instant(params.duration_ns + LATER));
    let traces = oracle::calls(&sim);
    let services = s.public_info().services;
    let h = h_std(abc, 1.0, 0.3);
    let (mut right, mut expect, mut var) = (0.0, 0.0, 0.0);
    for (f, c) in foci.iter().zip(&calls) {
        let tr = &traces[*c as usize];
        assert_eq!((tr.incident, tr.q, tr.d), (None, 1.0, 0.3));
        assert!((tr.h - h).abs() < 1e-9);
        // With an empty context the guess is uniform over "not an incident" and every kind at
        // the focus's service or upstream of it.
        let site = crate::reasoner::service_of(&s.events()[f.0 as usize].1);
        let mut up = std::collections::BTreeSet::new();
        let mut stack = vec![site];
        while let Some(x) = stack.pop() {
            for d in &services[x.index()].depends_on {
                if up.insert(*d) {
                    stack.push(*d);
                }
            }
        }
        let size = 1 + 5 * (1 + up.len());
        assert!(
            (tr.p0 - 1.0 / size as f64).abs() < 1e-12,
            "p0 {} vs 1/{size}",
            tr.p0
        );
        let p = tr.p0 + (1.0 - tr.p0) * h;
        assert!((tr.p - p).abs() < 1e-12);
        right += got[c].is_none() as u8 as f64;
        expect += p;
        var += p * (1.0 - p);
    }
    assert!(
        (right - expect).abs() < 4.5 * var.sqrt(),
        "{right} vs {expect} (sd {})",
        var.sqrt()
    );
}

#[test]
fn no_incident_is_ever_answered_with_a_service_outside_the_graph() {
    let params = open_params(3, (-2.0, 1.0, 1.0));
    let (s, t) = with_truth(&params);
    let mut sim = StreamSimulator::new(s);
    let end = Instant(params.duration_ns);
    sim.observe_until(end);
    for inc in &t.incidents {
        for n in 0..30 {
            ask(
                &mut sim,
                inc.observations[0],
                inc.observations
                    .iter()
                    .take(n % 7)
                    .map(|o| ObsRef::Passive(*o))
                    .collect(),
                end,
            );
        }
    }
    for (_, d) in answers(&mut sim, Instant(params.duration_ns + LATER)) {
        if let Some(h) = d {
            assert!(h.site.index() < t.services.len());
        }
    }
}
