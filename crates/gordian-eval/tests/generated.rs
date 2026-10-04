//! `score` against episodes the world generates, with `Truth::from_episode` as the only bridge.
//!
//! The expectations are stated from the plan's class definitions, not read back from the code
//! under test: which classes have a fault, and which classes are critical.

use gordian_core::Instant;
use gordian_eval::{EvalError, Step, Truth, score};
use gordian_world::episode::BudgetSpec;
use gordian_world::{
    Action, Episode, EpisodeClass, EpisodeSpec, FaultKind, Outcome, ProbeKind, Refusal, ServiceId,
    Simulator, generate,
};

const SEEDS: u64 = 20;
const MS: u64 = 1_000_000;

fn episodes() -> impl Iterator<Item = (u64, EpisodeClass, Episode)> {
    (0..SEEDS).flat_map(|seed| {
        EpisodeClass::ALL
            .into_iter()
            .map(move |class| (seed, class, generate(&EpisodeSpec::new(seed, class))))
    })
}

/// The plan's table: `CriticalFault` is critical, and DESIGN.md makes `QuietUrgent` critical.
fn class_is_critical(class: EpisodeClass) -> bool {
    matches!(
        class,
        EpisodeClass::CriticalFault | EpisodeClass::QuietUrgent
    )
}

fn step(at: u64, action: Action, outcome: Outcome) -> Step {
    Step {
        at: Instant(at),
        action,
        outcome,
    }
}

/// Run `actions` through a real simulator at the given instants and record the trajectory.
fn run(episode: &Episode, actions: &[(u64, Action)]) -> Vec<Step> {
    let mut sim = Simulator::new(episode.clone());
    actions
        .iter()
        .map(|(at, action)| step(*at, *action, sim.apply(*action, Instant(*at))))
        .collect()
}

#[test]
fn the_truth_matches_the_class() {
    let mut count = 0;
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        assert_eq!(truth.class, class, "seed {seed}");
        assert_eq!(
            truth.faults.is_empty(),
            class == EpisodeClass::NoFault,
            "{class:?} seed {seed}"
        );
        for fault in &truth.faults {
            assert_eq!(
                fault.critical,
                class_is_critical(class),
                "{class:?} seed {seed}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 220, "20 seeds x 11 classes");
}

#[test]
fn declaring_the_truth_succeeds_with_no_critical_miss() {
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        let hypothesis = truth.faults.first().map(|f| (f.kind, f.site));
        let declare = Action::Declare { fault: hypothesis };

        // Written by hand, as the fixtures are.
        let by_hand = [step(0, declare, Outcome::Declared)];
        let v = score(&truth, &by_hand).unwrap();
        assert!(v.success, "{class:?} seed {seed}");
        assert!(!v.critical_miss, "{class:?} seed {seed}");
        assert!(!v.false_alarm, "{class:?} seed {seed}");
        assert!(!v.abstained && !v.undecided, "{class:?} seed {seed}");
        assert_eq!((v.probes_used, v.corrections), (0, 0));
        assert_eq!(v.decision_at, Some(Instant(0)));

        // Recorded from the real simulator: its `Declared` is what the scorer must accept.
        let recorded = run(&episode, &[(5 * MS, declare)]);
        assert_eq!(recorded[0].outcome, Outcome::Declared);
        let v = score(&truth, &recorded).unwrap();
        assert!(v.success && !v.critical_miss, "{class:?} seed {seed}");
        assert_eq!(v.decision_at, Some(Instant(5 * MS)));
    }
}

#[test]
fn abstaining_succeeds_only_when_there_is_no_fault() {
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        let recorded = run(&episode, &[(MS, Action::Abstain)]);
        assert_eq!(recorded[0].outcome, Outcome::Abstained);
        let v = score(&truth, &recorded).unwrap();
        let no_fault = class == EpisodeClass::NoFault;
        assert_eq!(v.success, no_fault, "{class:?} seed {seed}");
        assert!(v.abstained, "{class:?} seed {seed}");
        assert!(!v.undecided && !v.false_alarm, "{class:?} seed {seed}");
        assert_eq!(
            v.critical_miss,
            class_is_critical(class),
            "{class:?} seed {seed}"
        );
    }
}

#[test]
fn declaring_no_fault_succeeds_only_when_there_is_no_fault() {
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        let v = score(
            &truth,
            &run(&episode, &[(MS, Action::Declare { fault: None })]),
        )
        .unwrap();
        assert_eq!(
            v.success,
            class == EpisodeClass::NoFault,
            "{class:?} seed {seed}"
        );
        assert!(!v.abstained && !v.false_alarm, "{class:?} seed {seed}");
        assert_eq!(
            v.critical_miss,
            class_is_critical(class),
            "{class:?} seed {seed}"
        );
    }
}

#[test]
fn a_wrong_kind_or_a_wrong_site_fails_on_every_generated_episode() {
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        let n = u32::try_from(episode.world().len()).unwrap();
        let Some(fault) = truth.faults.first() else {
            // NoFault: any declared fault is a false alarm.
            let v = score(
                &truth,
                &run(
                    &episode,
                    &[(
                        MS,
                        Action::Declare {
                            fault: Some((FaultKind::ConfigDrift, ServiceId(0))),
                        },
                    )],
                ),
            )
            .unwrap();
            assert!(
                v.false_alarm && !v.success && !v.critical_miss,
                "seed {seed}"
            );
            continue;
        };
        let other_site = ServiceId((fault.site.0 + 1) % n);
        assert_ne!(other_site, fault.site);
        let other_kind = FaultKind::ALL
            .into_iter()
            .find(|k| *k != fault.kind)
            .unwrap();
        for wrong in [(fault.kind, other_site), (other_kind, fault.site)] {
            let recorded = run(&episode, &[(MS, Action::Declare { fault: Some(wrong) })]);
            assert_eq!(recorded[0].outcome, Outcome::Declared, "world accepts it");
            let v = score(&truth, &recorded).unwrap();
            assert!(!v.success, "{class:?} seed {seed} {wrong:?}");
            assert_eq!(v.critical_miss, fault.critical, "{class:?} seed {seed}");
            assert!(!v.false_alarm, "{class:?} seed {seed}");
        }
    }
}

#[test]
fn real_probe_and_correction_outcomes_are_counted() {
    for (seed, class, episode) in episodes() {
        let truth = Truth::from_episode(&episode);
        let site = truth.faults.first().map_or(ServiceId(0), |f| f.site);
        let hypothesis = truth.faults.first().map(|f| (f.kind, f.site));
        let recorded = run(
            &episode,
            &[
                (
                    MS,
                    Action::Probe {
                        kind: ProbeKind::HealthCheck,
                        target: site,
                    },
                ),
                (3 * MS, Action::Correct { site }),
                (60 * MS, Action::Declare { fault: hypothesis }),
            ],
        );
        assert!(matches!(recorded[0].outcome, Outcome::Probed { .. }));
        assert!(matches!(recorded[1].outcome, Outcome::Corrected { .. }));
        let v = score(&truth, &recorded).unwrap();
        assert_eq!(
            (v.probes_used, v.corrections),
            (1, 1),
            "{class:?} seed {seed}"
        );
        assert!(v.success, "{class:?} seed {seed}");
        // A correction on a NoFault episode is a false alarm even though the close is right.
        assert_eq!(
            v.false_alarm,
            class == EpisodeClass::NoFault,
            "{class:?} seed {seed}"
        );
    }
}

#[test]
fn the_worlds_refusals_are_not_counted() {
    let mut spec = EpisodeSpec::new(3, EpisodeClass::Ambiguous);
    spec.budget = BudgetSpec {
        probes: 1,
        time_ns: 250 * MS,
    };
    let episode = generate(&spec);
    let truth = Truth::from_episode(&episode);
    let site = truth.faults[0].site;
    let health = Action::Probe {
        kind: ProbeKind::HealthCheck,
        target: site,
    };
    let recorded = run(
        &episode,
        &[
            (
                0,
                Action::Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(999),
                },
            ),
            (MS, health),
            (2 * MS, health),
            (3 * MS, Action::Correct { site }),
        ],
    );
    assert!(matches!(
        recorded[0].outcome,
        Outcome::Refused(Refusal::UnknownService(_))
    ));
    assert!(matches!(recorded[1].outcome, Outcome::Probed { .. }));
    assert!(matches!(
        recorded[2].outcome,
        Outcome::Refused(Refusal::BudgetExceeded { .. })
    ));
    assert!(matches!(
        recorded[3].outcome,
        Outcome::Refused(Refusal::BudgetExceeded { .. })
    ));
    // Budget ran out, nothing closed the episode: undecided, and the miss is not critical here.
    let v = score(&truth, &recorded).unwrap();
    assert!(v.undecided && !v.success && !v.abstained);
    assert_eq!((v.probes_used, v.corrections), (1, 0));
    assert_eq!(v.decision_at, None);
}

#[test]
fn a_declaration_past_the_horizon_is_refused_and_leaves_the_episode_undecided() {
    let episode = generate(&EpisodeSpec::new(4, EpisodeClass::CriticalFault));
    let truth = Truth::from_episode(&episode);
    let hypothesis = truth.faults.first().map(|f| (f.kind, f.site));
    let late = episode.spec().horizon.0 + 1;
    let recorded = run(&episode, &[(late, Action::Declare { fault: hypothesis })]);
    assert_eq!(recorded[0].outcome, Outcome::Refused(Refusal::PastHorizon));
    let v = score(&truth, &recorded).unwrap();
    assert!(v.undecided && !v.success);
    assert!(v.critical_miss, "undecided on a critical episode is a miss");
}

#[test]
fn a_step_the_world_refuses_after_the_close_is_a_harness_error() {
    let episode = generate(&EpisodeSpec::new(5, EpisodeClass::Ambiguous));
    let truth = Truth::from_episode(&episode);
    let recorded = run(
        &episode,
        &[
            (MS, Action::Abstain),
            (
                2 * MS,
                Action::Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(0),
                },
            ),
        ],
    );
    assert_eq!(recorded[1].outcome, Outcome::Refused(Refusal::EpisodeOver));
    assert_eq!(
        score(&truth, &recorded),
        Err(EvalError::ActionAfterTerminal { index: 1 })
    );
}

#[test]
fn a_step_stamped_earlier_than_the_one_before_is_a_harness_error() {
    let episode = generate(&EpisodeSpec::new(6, EpisodeClass::Ambiguous));
    let truth = Truth::from_episode(&episode);
    let recorded = run(
        &episode,
        &[
            (
                5 * MS,
                Action::Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(0),
                },
            ),
            (
                3 * MS,
                Action::Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(0),
                },
            ),
        ],
    );
    assert_eq!(
        recorded[1].outcome,
        Outcome::Refused(Refusal::TimeWentBackwards)
    );
    assert_eq!(
        score(&truth, &recorded),
        Err(EvalError::TimeWentBackwards { index: 1 })
    );
}
