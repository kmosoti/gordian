//! Property tests over generated specs: determinism, ordering, structure, refusal, soundness of
//! the public checker against the generator, and headroom.

use super::*;
use crate::episode::BudgetSpec;
use crate::fault::FaultKind;
use crate::oracle::reveal;
use crate::physics::MAX_SERVICES;
use crate::sense::{Probe, ProbeResult};
use crate::step::Refusal;
use proptest::prelude::*;
use std::collections::BTreeSet;

fn spec_strategy() -> impl Strategy<Value = EpisodeSpec> {
    (
        any::<u64>(),
        0usize..11,
        0u64..30_000_000_000,
        0u32..8,
        (0u64..20, 0u64..500_000_000),
        (0u8..15, 0u8..15),
        0u32..30,
    )
        .prop_map(
            |(seed, class, horizon, noise_rate, (probes, time_ns), (lo, hi), delay_k)| {
                EpisodeSpec {
                    seed,
                    class: EpisodeClass::ALL[class],
                    horizon: Instant(horizon),
                    noise_rate,
                    budget: BudgetSpec { probes, time_ns },
                    min_services: lo,
                    max_services: hi,
                    delay_k,
                }
            },
        )
}

fn config() -> ProptestConfig {
    ProptestConfig::with_cases(64)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn generation_is_deterministic_and_normalization_is_idempotent(spec in spec_strategy()) {
        let a = generate(&spec);
        let b = generate(&spec);
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(&a, &generate(&spec.normalized()));
        prop_assert_eq!(a.spec(), &spec.normalized());
    }

    #[test]
    fn stream_is_sorted_and_inside_the_horizon(spec in spec_strategy()) {
        let ep = generate(&spec);
        let s = ep.stream();
        prop_assert!(s.windows(2).all(|w| w[0].0 <= w[1].0));
        prop_assert!(s.iter().all(|(t, _)| *t <= ep.spec().horizon));
        prop_assert_eq!(reveal(&ep).labels.len(), s.len());
    }

    #[test]
    fn world_is_a_dag_of_four_to_twelve_and_fault_sites_are_services(spec in spec_strategy()) {
        let ep = generate(&spec);
        let n = ep.world().len();
        prop_assert!((4..=MAX_SERVICES as usize).contains(&n));
        for (i, s) in ep.world().services.iter().enumerate() {
            prop_assert_eq!(s.id.index(), i);
            prop_assert!(s.depends_on.iter().all(|d| d.index() < i), "edges go high to low");
            prop_assert!(s.depends_on.windows(2).all(|w| w[0] < w[1]));
            if i > 0 {
                prop_assert!(!s.depends_on.is_empty());
            }
        }
        for f in reveal(&ep).faults {
            prop_assert!(ep.world().service(f.site).is_some());
        }
        // Every observation refers to a service in the world.
        for (_, o) in ep.stream() {
            let id = match o {
                Observation::Counter { service, .. }
                | Observation::Message { service, .. }
                | Observation::Snapshot { service, .. } => *service,
                _ => continue,
            };
            prop_assert!(id.index() < n);
        }
    }

    #[test]
    fn actions_naming_a_nonexistent_service_are_refused_and_charge_nothing(
        spec in spec_strategy(), extra in 0u32..1000, k in 0usize..6
    ) {
        let ep = generate(&spec);
        let bad = ServiceId(ep.world().len() as u32 + extra);
        let mut sim = Simulator::new(ep);
        let before = sim.remaining();
        for action in [
            Action::Probe { kind: ProbeKind::ALL[k], target: bad },
            Action::Correct { site: bad },
            Action::Declare { fault: Some((FaultKind::ALL[k % 5], bad)) },
        ] {
            prop_assert_eq!(sim.apply(action, Instant::ZERO), Outcome::Refused(Refusal::UnknownService(bad)));
        }
        prop_assert_eq!(sim.remaining(), before);
        prop_assert!(!sim.is_over(), "a refused declaration must not close the episode");
    }

    #[test]
    fn soundness_over_every_prefix_of_the_stream(spec in spec_strategy()) {
        let ep = generate(&spec);
        let t = truth(&ep);
        let s = ep.stream();
        let mut cuts: BTreeSet<usize> = (0..=s.len().min(40)).collect();
        cuts.extend((0..=20).map(|i| s.len() * i / 20));
        for cut in cuts {
            let c = consistent(&ep, &s[..cut]);
            prop_assert!(c.contains(&t), "cut {} of {}: truth {:?} missing from {:?}", cut, s.len(), t, c);
        }
    }

    #[test]
    fn soundness_with_arbitrary_probes_and_corrections(
        spec in spec_strategy(),
        cut in 0usize..=100,
        actions in proptest::collection::vec((0usize..7, 0u32..16, any::<bool>()), 0..10),
    ) {
        // Default budget: with a random tiny budget nearly every probe would be refused.
        let ep = generate(&EpisodeSpec { budget: BudgetSpec::default(), ..spec });
        let t = truth(&ep);
        let s = ep.stream();
        let mut evidence: Evidence = s[..s.len() * cut / 100].to_vec();
        let mut sim = Simulator::new(ep.clone());
        let n = ep.world().len() as u32;
        for (a, target, at_site) in actions {
            // Half the actions aim at the true site, where probes carry information.
            let target = match (at_site, t) {
                (true, Some((_, site))) => site,
                _ => ServiceId(target % n),
            };
            let action = if a < 6 {
                Action::Probe { kind: ProbeKind::ALL[a], target }
            } else {
                Action::Correct { site: target }
            };
            match sim.apply(action, Instant::ZERO) {
                Outcome::Probed { observation, ready_at, .. } | Outcome::Corrected { observation, ready_at, .. } => {
                    evidence.push((ready_at, observation));
                }
                Outcome::Refused(_) => {}
                other => prop_assert!(false, "unexpected {other:?}"),
            }
            let c = consistent(&ep, &evidence);
            prop_assert!(c.contains(&t), "truth {:?} missing from {:?} after {:?}", t, c, action);
        }
    }

    #[test]
    fn headroom_a_probe_sequence_within_budget_reaches_exactly_the_truth(spec in spec_strategy()) {
        prop_assume!(spec.class != EpisodeClass::NoFault);
        // The generated budget is irrelevant here: headroom is a claim about the default budget.
        let ep = generate(&EpisodeSpec { budget: BudgetSpec::default(), ..spec });
        let found = min_probes(&ep, 4);
        prop_assert!(found.is_some(), "class {:?}: no resolving sequence within budget", ep.spec().class);
    }
}

/// Fewest probes after which the checker returns exactly the truth, searched by iterative
/// deepening over every probe that can shrink the consistent set, through the real simulator
/// (so the budget is enforced and results come from hidden state). `None` if none exists within
/// `max_depth` probes.
pub(crate) fn min_probes(ep: &Episode, max_depth: usize) -> Option<usize> {
    let t = truth(ep);
    (0..=max_depth).find(|d| search(&Simulator::new(ep.clone()), ep.stream().to_vec(), *d, ep, t))
}

fn search(
    sim: &Simulator,
    evidence: Evidence,
    depth: usize,
    ep: &Episode,
    t: crate::Hypothesis,
) -> bool {
    let current = consistent(ep, &evidence);
    if current == vec![t] {
        return true;
    }
    // Progress is measured in consistent *worlds* (hypothesis plus hidden bits), not hypotheses:
    // one sample of a jointly decisive pair removes a world without removing a hypothesis.
    let worlds = |ev: &Evidence| crate::physics::consistent_worlds(&ep.public_info(), ev).len();
    let current_worlds = worlds(&evidence);
    if depth == 0 {
        return false;
    }
    // A probe at a service that is nobody's candidate site returns the healthy answer, which no
    // remaining hypothesis contradicts. So only candidate sites need trying.
    let targets: BTreeSet<ServiceId> = if current.contains(&None) {
        (0..ep.world().len() as u32).map(ServiceId).collect()
    } else {
        current.iter().flatten().map(|(_, s)| *s).collect()
    };
    for target in targets {
        for kind in ProbeKind::ALL {
            let mut next = sim.clone();
            if let Outcome::Probed {
                observation,
                ready_at,
                ..
            } = next.apply(Action::Probe { kind, target }, Instant::ZERO)
            {
                let mut ev = evidence.clone();
                ev.push((ready_at, observation));
                if worlds(&ev) < current_worlds && search(&next, ev, depth - 1, ep, t) {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn headroom_needs_exactly_the_probes_each_class_is_built_to_need() {
    // The minimum number of probes per class, so that headroom is not vacuous: classes that are
    // built to need a probe must need one, and decisive streams must need none.
    for class in EpisodeClass::ALL {
        if class == EpisodeClass::NoFault {
            continue;
        }
        let mut seen = BTreeSet::new();
        for seed in SEEDS {
            let ep = episode(class, seed);
            seen.insert(
                min_probes(&ep, 4).unwrap_or_else(|| panic!("{class:?} seed {seed}: no sequence")),
            );
        }
        let expected: &[usize] = match class {
            EpisodeClass::Ambiguous | EpisodeClass::FeedbackBait | EpisodeClass::StaleMemory => {
                &[1]
            }
            EpisodeClass::JointlyDecisive => &[2],
            EpisodeClass::CriticalFault => &[0, 1],
            _ => &[0],
        };
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), expected, "{class:?}");
    }
}

#[test]
fn probe_results_are_inconclusive_only_on_unreliable_services() {
    for class in EpisodeClass::ALL {
        for seed in 0..10 {
            let ep = episode(class, seed);
            for s in &ep.world().services {
                let (_, obs) = probe_obs(&ep, ProbeKind::HealthCheck, s.id);
                let Observation::Probed { probe, result } = obs else {
                    panic!()
                };
                assert_eq!(
                    probe,
                    Probe {
                        kind: ProbeKind::HealthCheck,
                        target: s.id
                    }
                );
                assert_eq!(
                    matches!(result, ProbeResult::Inconclusive { .. }),
                    s.unreliable_health
                );
            }
        }
    }
}

#[test]
fn soundness_after_every_probe_kind_at_the_site_for_every_class_and_seed() {
    // Deterministic companion to the proptest above: the interesting probes are the ones at the
    // fault site, and random targets rarely hit it twice.
    for class in EpisodeClass::ALL {
        for seed in SEEDS {
            let ep = episode(class, seed);
            let t = truth(&ep);
            let site = t.map_or(ServiceId(0), |(_, s)| s);
            let mut sim = Simulator::new(ep.clone());
            let mut evidence = ep.stream().to_vec();
            // All six once cost 8 of the 12 probes. Reversed so the two samples go first.
            for kind in ProbeKind::ALL.into_iter().rev() {
                match sim.apply(Action::Probe { kind, target: site }, Instant::ZERO) {
                    Outcome::Probed {
                        observation,
                        ready_at,
                        ..
                    } => evidence.push((ready_at, observation)),
                    other => panic!("{class:?} seed {seed}: {kind:?} refused: {other:?}"),
                }
                let c = consistent(&ep, &evidence);
                assert!(
                    c.contains(&t),
                    "{class:?} seed {seed} after {kind:?}: {t:?} not in {c:?}"
                );
            }
            if let Outcome::Corrected {
                observation,
                ready_at,
                ..
            } = sim.apply(Action::Correct { site }, Instant::ZERO)
            {
                evidence.push((ready_at, observation));
                assert!(consistent(&ep, &evidence).contains(&t));
            }
        }
    }
}
