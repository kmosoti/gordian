//! Structural invariants of the stream, the simulator's interface, and the checks that nothing
//! hidden is reachable through the public types.

use super::*;
use crate::kinds::{HardKind, Tier};
use crate::oracle::{self, ObsLabel};
use crate::{
    ObsRef, Question, StreamAction, StreamEvent, StreamOutcome, StreamRefusal, StreamSimulator,
};
use gordian_core::Instant;
use gordian_world::graph::dependents_mask;
use gordian_world::{Observation, ProbeKind, ServiceId};
use std::collections::BTreeSet;

fn service_of(o: &Observation) -> ServiceId {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => *service,
        Observation::Probed { probe, .. } => probe.target,
        Observation::Correction { site, .. } => *site,
    }
}

#[test]
fn the_stream_is_sorted_in_range_and_names_real_services() {
    for seed in 0..20 {
        let (s, t) = with_truth(&StreamParams::new(seed));
        let n = t.services.len();
        assert!((6..=12).contains(&n));
        let mut prev = Instant::ZERO;
        for (at, o) in s.events() {
            assert!(*at >= prev, "seed {seed}: unsorted");
            assert!(at.0 <= t.duration_ns);
            assert!(service_of(o).index() < n);
            prev = *at;
        }
        assert_eq!(t.labels.len(), s.events().len());
    }
}

#[test]
fn the_graph_is_a_dag_with_unconnected_pairs() {
    let mut with_pairs = 0;
    for seed in 0..40 {
        let s = generate(&StreamParams::new(seed));
        let info = s.public_info();
        for sv in &info.services {
            for d in &sv.depends_on {
                assert!(d.index() < sv.id.index(), "edges point to lower ids");
            }
        }
        let n = info.services.len();
        let mut pairs = 0;
        for a in 0..n {
            for b in (a + 1)..n {
                if crate::regime::incomparable(
                    &info.services,
                    ServiceId(a as u32),
                    ServiceId(b as u32),
                ) {
                    pairs += 1;
                }
            }
        }
        if pairs >= 2 {
            with_pairs += 1;
        }
    }
    assert_eq!(
        with_pairs, 40,
        "the generator retries until a cascade is possible"
    );
}

#[test]
fn incidents_arrive_before_the_tail_and_services_are_never_shared() {
    for seed in 0..30 {
        let (_, t) = with_truth(&StreamParams::new(seed));
        let last = t.duration_ns - 60_000_000_000;
        for (i, a) in t.incidents.iter().enumerate() {
            assert!(a.onset_ns <= last);
            assert_eq!(a.id as usize, i);
            for b in &t.incidents[i + 1..] {
                let shared = a.occupies.iter().any(|s| b.occupies.contains(s));
                if shared {
                    assert!(
                        b.onset_ns >= a.busy_until_ns,
                        "seed {seed}: incidents {} and {} overlap on a service",
                        a.id,
                        b.id
                    );
                }
            }
        }
    }
}

#[test]
fn tier_mix_matches_the_parameters() {
    let (mut plain, mut hard, mut decoy) = (0u32, 0u32, 0u32);
    for seed in 0..120 {
        let mut p = StreamParams::new(seed);
        p.recurrence_permille = 0;
        let (_, t) = with_truth(&p);
        for i in &t.incidents {
            match i.tier {
                Tier::Plain => plain += 1,
                Tier::Hard => hard += 1,
                Tier::Decoy => decoy += 1,
            }
        }
    }
    let n = (plain + hard + decoy) as f64;
    let check = |count: u32, p: f64, what: &str| {
        let sd = (n * p * (1.0 - p)).sqrt();
        assert!(
            (count as f64 - n * p).abs() < 4.5 * sd,
            "{what}: {count} of {n}, expected {}",
            n * p
        );
    };
    check(plain, 0.8, "plain");
    check(hard, 0.1, "hard");
    check(decoy, 0.1, "decoy");
}

#[test]
fn every_hard_family_and_both_modes_occur_and_decoys_are_never_critical() {
    let mut seen = BTreeSet::new();
    for seed in 0..60 {
        let (_, t) = with_truth(&StreamParams::new(seed));
        for i in &t.incidents {
            if i.tier == Tier::Decoy {
                assert!(!i.critical);
                assert!(i.deadline_ns.is_none());
                assert!(i.truth.is_none());
            }
            if i.tier != Tier::Plain {
                seen.insert((
                    i.shape.hard_kind.unwrap(),
                    i.shape.contradicts_early,
                    i.tier,
                ));
            }
        }
    }
    for hk in HardKind::ALL {
        for tier in [Tier::Hard, Tier::Decoy] {
            if hk == HardKind::SlowLeak {
                assert!(seen.contains(&(hk, None, tier)), "{hk:?} {tier:?}");
            } else {
                for early in [false, true] {
                    assert!(
                        seen.contains(&(hk, Some(early), tier)),
                        "{hk:?} {early} {tier:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn observation_ids_are_dense_and_delivered_once() {
    let s = stream_for(7);
    let total = s.events().len();
    let mut sim = StreamSimulator::new(s);
    let mut next = 0u32;
    let mut now = 0u64;
    while now <= 600_000_000_000 {
        for e in sim.observe_until(Instant(now)) {
            if let StreamEvent::Observed { id, at, .. } = e {
                assert_eq!(id.0, next);
                assert!(at.0 <= now);
                next += 1;
            }
        }
        now += 7_300_000_000;
    }
    for e in sim.observe_until(Instant(600_000_000_000)) {
        if let StreamEvent::Observed { id, .. } = e {
            assert_eq!(id.0, next);
            next += 1;
        }
    }
    assert_eq!(next as usize, total);
    assert!(sim.observe_until(Instant(600_000_000_000)).is_empty());
}

fn stream_for(seed: u64) -> crate::Stream {
    generate(&StreamParams::new(seed))
}

fn sim_with_everything_delivered(params: &StreamParams) -> (StreamSimulator, u32) {
    let mut sim = StreamSimulator::new(generate(params));
    let events = sim.observe_until(Instant(params.duration_ns));
    let n = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Observed { .. }))
        .count() as u32;
    (sim, n)
}

#[test]
fn actions_are_refused_without_charge_when_they_are_malformed() {
    let p = StreamParams::new(9);
    let (mut sim, n) = sim_with_everything_delivered(&p);
    let before = sim.remaining();
    let end = Instant(p.duration_ns);
    let obs = |i: u32| ObsRef::Passive(crate::ObsId(i));
    let q = |i: u32| Question::Diagnose {
        focus: crate::ObsId(i),
    };

    assert_eq!(
        sim.apply(
            StreamAction::Probe {
                kind: ProbeKind::HealthCheck,
                target: ServiceId(99)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::UnknownService(ServiceId(99)))
    );
    assert_eq!(
        sim.apply(
            StreamAction::Escalate {
                context: vec![obs(0), obs(0)],
                question: q(0)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::DuplicateRef(obs(0)))
    );
    assert_eq!(
        sim.apply(
            StreamAction::Escalate {
                context: vec![obs(n)],
                question: q(0)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::UnknownRef(obs(n)))
    );
    assert_eq!(
        sim.apply(
            StreamAction::Escalate {
                context: vec![ObsRef::Probe(0)],
                question: q(0)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::UnknownRef(ObsRef::Probe(0)))
    );
    assert_eq!(
        sim.apply(
            StreamAction::Escalate {
                context: vec![],
                question: q(n)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::UnknownRef(obs(n)))
    );
    assert_eq!(
        sim.apply(
            StreamAction::Declare {
                anchor: crate::ObsId(n),
                diagnosis: None
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::UnknownRef(obs(n)))
    );
    let too_many: Vec<ObsRef> = (0..=p.max_context).map(|i| obs(i.min(n - 1))).collect();
    assert!(matches!(
        sim.apply(
            StreamAction::Escalate {
                context: too_many,
                question: q(0)
            },
            end
        ),
        StreamOutcome::Refused(StreamRefusal::ContextTooLarge { .. })
    ));
    assert_eq!(
        sim.apply(
            StreamAction::Declare {
                anchor: crate::ObsId(0),
                diagnosis: None
            },
            Instant(end.0 + 1)
        ),
        StreamOutcome::Refused(StreamRefusal::PastDuration)
    );
    assert_eq!(sim.remaining(), before, "a refusal charged something");
    assert!(oracle::calls(&sim).is_empty());
}

#[test]
fn time_may_not_go_backwards() {
    let p = StreamParams::new(9);
    let mut sim = StreamSimulator::new(generate(&p));
    sim.observe_until(Instant(50_000_000_000));
    assert_eq!(
        sim.apply(
            StreamAction::Probe {
                kind: ProbeKind::HealthCheck,
                target: ServiceId(0)
            },
            Instant(10_000_000_000)
        ),
        StreamOutcome::Refused(StreamRefusal::TimeWentBackwards)
    );
}

#[test]
fn the_probe_and_reasoner_budgets_are_hard_limits() {
    let mut p = StreamParams::new(2);
    p.budget.probes = 3;
    p.budget.reasoner_ns =
        p.reasoner.cost.cost(0).modelled_ns + p.reasoner.cost.cost(1).modelled_ns;
    let (mut sim, _) = sim_with_everything_delivered(&p);
    let end = Instant(p.duration_ns);
    let probe = |sim: &mut StreamSimulator, kind| {
        sim.apply(
            StreamAction::Probe {
                kind,
                target: ServiceId(0),
            },
            end,
        )
    };
    // HealthCheck costs one probe unit.
    for _ in 0..3 {
        assert!(matches!(
            probe(&mut sim, ProbeKind::HealthCheck),
            StreamOutcome::Probed { .. }
        ));
    }
    assert!(matches!(
        probe(&mut sim, ProbeKind::HealthCheck),
        StreamOutcome::Refused(StreamRefusal::ProbeBudgetExceeded { .. })
    ));
    assert_eq!(sim.remaining().probes, 0);

    let focus = crate::ObsId(0);
    let ask = |sim: &mut StreamSimulator, refs: usize| {
        sim.apply(
            StreamAction::Escalate {
                context: (0..refs as u32)
                    .map(|i| ObsRef::Passive(crate::ObsId(i)))
                    .collect(),
                question: Question::Diagnose { focus },
            },
            end,
        )
    };
    assert!(matches!(ask(&mut sim, 0), StreamOutcome::Escalated { .. }));
    let left = sim.remaining().reasoner_ns;
    assert_eq!(left, p.reasoner.cost.cost(1).modelled_ns);
    // Two references cost more than is left.
    assert!(matches!(
        ask(&mut sim, 2),
        StreamOutcome::Refused(StreamRefusal::ReasonerBudgetExceeded { .. })
    ));
    assert_eq!(
        sim.remaining().reasoner_ns,
        left,
        "a refusal charged something"
    );
    assert!(matches!(ask(&mut sim, 1), StreamOutcome::Escalated { .. }));
    assert_eq!(sim.remaining().reasoner_ns, 0);
}

#[test]
fn probe_cost_and_ready_at_depend_on_the_probe_alone() {
    let p = StreamParams::new(4);
    let (mut sim, _) = sim_with_everything_delivered(&p);
    let (_, t) = with_truth(&p);
    let mut seen: std::collections::BTreeMap<ProbeKind, (gordian_world::step::CostSummary, u64)> =
        Default::default();
    // Probe every service at the onset of every incident and at an idle instant.
    let mut times: Vec<u64> = t.incidents.iter().map(|i| i.onset_ns + 1).collect();
    times.push(p.duration_ns);
    times.sort();
    let mut last = 0;
    for at in times {
        for kind in ProbeKind::ALL {
            // Probes cost budget; stay inside it by refreshing the simulator.
            let now = Instant(at.max(last));
            last = now.0;
            let mut fresh = sim.clone();
            if let StreamOutcome::Probed { cost, ready_at, .. } = fresh.apply(
                StreamAction::Probe {
                    kind,
                    target: ServiceId(0),
                },
                now,
            ) {
                let entry = (cost, ready_at.0 - now.0);
                assert_eq!(*seen.entry(kind).or_insert(entry), entry, "{kind:?}");
            }
        }
    }
    let _ = &mut sim;
    assert_eq!(seen.len(), 6);
}

#[test]
fn a_reasoner_answer_is_not_delivered_before_it_is_ready() {
    let p = StreamParams::new(6);
    let mut sim = StreamSimulator::new(generate(&p));
    let at = Instant(100_000_000_000);
    let events = sim.observe_until(at);
    let focus = match events.last().unwrap() {
        StreamEvent::Observed { id, .. } => *id,
        e => panic!("{e:?}"),
    };
    let StreamOutcome::Escalated {
        call,
        ready_at,
        cost,
    } = sim.apply(
        StreamAction::Escalate {
            context: vec![ObsRef::Passive(focus)],
            question: Question::Diagnose { focus },
        },
        at,
    )
    else {
        panic!("refused")
    };
    assert_eq!(cost, p.reasoner.cost.cost(1));
    assert_eq!(ready_at.0 - at.0, cost.latency_ns);
    let early = sim.observe_until(Instant(ready_at.0 - 1));
    assert!(
        early
            .iter()
            .all(|e| matches!(e, StreamEvent::Observed { .. })),
        "answer delivered early"
    );
    let on_time = sim.observe_until(ready_at);
    let answers: Vec<_> = on_time
        .iter()
        .filter(|e| matches!(e, StreamEvent::Answered { .. }))
        .collect();
    assert_eq!(answers.len(), 1);
    match answers[0] {
        StreamEvent::Answered {
            call: c,
            at,
            answer,
        } => {
            assert_eq!(*c, call);
            assert_eq!(*at, ready_at);
            assert_eq!(answer.focus, focus);
        }
        _ => unreachable!(),
    }
    assert!(
        sim.observe_until(Instant(ready_at.0 + 1_000_000_000))
            .iter()
            .all(|e| matches!(e, StreamEvent::Observed { .. })),
        "answer delivered twice"
    );
}

// ---- Hidden state is not reachable through the public types.

/// `true` when `T` implements `Serialize`, decided at compile time by inherent-method priority.
macro_rules! implements_serialize {
    ($t:ty) => {{
        struct Probe<T>(std::marker::PhantomData<T>);
        #[allow(dead_code)]
        trait Fallback {
            fn is_serialize(&self) -> bool {
                false
            }
        }
        impl<T> Fallback for Probe<T> {}
        impl<T: serde::Serialize> Probe<T> {
            #[allow(dead_code)]
            fn is_serialize(&self) -> bool {
                true
            }
        }
        Probe::<$t>(std::marker::PhantomData).is_serialize()
    }};
}

#[test]
fn the_generated_stream_and_the_simulator_are_not_serializable() {
    // The probe sees through to a type that is serializable, so the macro is not vacuous.
    assert!(implements_serialize!(crate::StreamParams));
    assert!(implements_serialize!(crate::StreamPublic));
    assert!(!implements_serialize!(crate::Stream));
    assert!(!implements_serialize!(crate::StreamSimulator));
}

#[test]
fn debug_output_of_the_hidden_types_carries_no_hidden_state() {
    let s = stream_for(3);
    let text = format!("{s:?}");
    assert!(text.len() < 100, "{text}");
    for word in [
        "Plain",
        "Hard",
        "Decoy",
        "deadline",
        "difficulty",
        "Decisive",
        "truth",
    ] {
        assert!(!text.contains(word), "{word} in {text}");
    }
    let sim = StreamSimulator::new(s);
    let text = format!("{sim:?}");
    assert!(text.len() < 200, "{text}");
    for word in [
        "Plain",
        "Hard",
        "Decoy",
        "deadline",
        "difficulty",
        "Decisive",
    ] {
        assert!(!text.contains(word), "{word} in {text}");
    }
}

#[test]
fn the_public_information_has_exactly_the_documented_fields() {
    let info = stream_for(3).public_info();
    let value = serde_json::to_value(&info).unwrap();
    let keys: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    let expected: BTreeSet<&str> = [
        "services",
        "duration_ns",
        "deadlines",
        "reasoner_cost",
        "max_context",
        "budget",
    ]
    .into_iter()
    .collect();
    assert_eq!(keys, expected);
    // Neither the reasoner's coefficients nor the mix appear anywhere in the serialization.
    let text = value.to_string();
    for word in [
        "permille",
        "\"a\"",
        "\"b\"",
        "\"c\"",
        "regime",
        "tier",
        "recurrence",
        "distractor",
        "penalty",
    ] {
        assert!(
            !text.contains(word),
            "{word} leaked into the public information"
        );
    }
}

#[test]
fn public_information_does_not_depend_on_any_hidden_parameter() {
    let base = StreamParams::new(21);
    let reference = generate(&base).public_info();
    let mut p = base.clone();
    p.mix.hard_permille = 500;
    p.mix.plain_permille = 100;
    p.recurrence_permille = 900;
    p.reasoner.a = 3.0;
    p.reasoner.b = 0.5;
    p.reasoner.c = 9.0;
    p.reasoner.distractor_penalty = 0.4;
    p.regimes.clear();
    p.noise.catalogue_mhz = 0;
    p.difficulty.hard.lo = 0.0;
    p.critical.hard_permille = 1000;
    p.mean_gap_ns = 3_000_000_000;
    assert_eq!(generate(&p).public_info(), reference);
}

#[test]
fn nothing_announces_a_regime_change() {
    let with = StreamParams::new(31);
    let mut without = with.clone();
    without.regimes.clear();
    let first_change = with.regimes.iter().map(|r| r.at_ns).min().unwrap();
    let a = generate(&with);
    let b = generate(&without);
    assert_eq!(a.public_info(), b.public_info());
    let before = |s: &crate::Stream| {
        s.events()
            .iter()
            .filter(|(at, _)| at.0 < first_change)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        before(&a),
        before(&b),
        "the stream differs before the first change"
    );
    assert_ne!(
        a.events(),
        b.events(),
        "the change must change something later"
    );
}

#[test]
fn pulses_and_decisive_evidence_use_the_roles_the_documentation_gives() {
    // Every decisive observation of a decoy is a benign counter at its site after its
    // resolution; every decisive observation of a hard incident arrives at or after T0.
    let t0 = crate::timing::T0_NS;
    for seed in 0..20 {
        let (s, t) = with_truth(&StreamParams::new(seed));
        for inc in &t.incidents {
            for id in &inc.decisive {
                let (at, o) = &s.events()[id.0 as usize];
                match inc.tier {
                    Tier::Decoy => {
                        assert!(at.0 >= inc.onset_ns + t0);
                        assert!(
                            matches!(o, Observation::Counter { value, .. } if *value < 50),
                            "{o:?}"
                        );
                    }
                    Tier::Hard => assert!(at.0 >= inc.onset_ns + t0, "{o:?}"),
                    Tier::Plain => assert!(at.0 <= inc.onset_ns + 100_000_000),
                }
            }
            assert!(!inc.decisive.is_empty());
        }
    }
}

#[test]
fn every_observation_label_matches_the_oracle_view() {
    let (s, t) = with_truth(&StreamParams::new(8));
    let mut counted = 0;
    for (i, l) in t.labels.iter().enumerate() {
        if let ObsLabel::Incident { id, .. } = l {
            assert!(
                t.incidents[*id as usize]
                    .observations
                    .contains(&crate::ObsId(i as u32))
            );
            counted += 1;
        }
    }
    let total: usize = t.incidents.iter().map(|i| i.observations.len()).sum();
    assert_eq!(counted, total);
    // The graph the oracle reports is the one the public information reports.
    assert_eq!(t.services, s.public_info().services);
}

#[test]
fn dependents_are_those_of_the_graph_in_force() {
    // A plain Identified incident's dependents' observations are exactly the dependents of its
    // site in the graph of the regime in force at its onset (checked here before any regime).
    let mut p = StreamParams::new(14);
    p.regimes.clear();
    p.mix.plain_permille = 1000;
    p.mix.hard_permille = 0;
    let (s, t) = with_truth(&p);
    for inc in &t.incidents {
        let site = inc.occupies[0];
        let deps: Vec<usize> = dependents_mask(&t.services, site)
            .iter()
            .enumerate()
            .filter(|(_, h)| **h)
            .map(|(i, _)| i)
            .collect();
        if inc.shape.duo {
            continue;
        }
        for id in &inc.decisive {
            let svc = service_of(&s.events()[id.0 as usize].1);
            assert!(svc == site || deps.contains(&svc.index()), "{inc:?}");
        }
    }
}
