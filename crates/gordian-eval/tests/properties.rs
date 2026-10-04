//! Properties of `score` over generated trajectories. The trajectories are built here from the
//! world's types; the world's generator is not involved.

use gordian_core::Instant;
use gordian_eval::{EvalError, Step, Truth, score};
use gordian_world::sense::{Probe, ProbeKind, ProbeResult};
use gordian_world::step::CostSummary;
use gordian_world::{
    Action, EpisodeClass, Fault, FaultKind, Observation, Outcome, Refusal, ServiceId,
};
use proptest::prelude::*;

const SERVICES: u32 = 12;

fn arb_service() -> impl Strategy<Value = ServiceId> {
    (0..SERVICES).prop_map(ServiceId)
}

fn arb_kind() -> impl Strategy<Value = FaultKind> {
    (0..FaultKind::ALL.len()).prop_map(|i| FaultKind::ALL[i])
}

fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
    (0..ProbeKind::ALL.len()).prop_map(|i| ProbeKind::ALL[i])
}

fn arb_class() -> impl Strategy<Value = EpisodeClass> {
    (0..EpisodeClass::ALL.len()).prop_map(|i| EpisodeClass::ALL[i])
}

fn arb_faulted_class() -> impl Strategy<Value = EpisodeClass> {
    arb_class().prop_filter("a faulted class", |c| *c != EpisodeClass::NoFault)
}

fn arb_fault() -> impl Strategy<Value = Fault> {
    (
        arb_kind(),
        arb_service(),
        0..10_000_000_000u64,
        any::<bool>(),
    )
        .prop_map(|(kind, site, onset, critical)| Fault {
            kind,
            site,
            onset: Instant(onset),
            critical,
        })
}

/// A truth that obeys R18: `NoFault` has no fault, other classes have one.
fn arb_truth() -> impl Strategy<Value = Truth> {
    prop_oneof![
        Just(Truth {
            class: EpisodeClass::NoFault,
            faults: vec![]
        }),
        (arb_faulted_class(), arb_fault()).prop_map(|(class, f)| Truth {
            class,
            faults: vec![f]
        }),
    ]
}

fn probed(kind: ProbeKind, target: ServiceId, at: u64) -> Outcome {
    Outcome::Probed {
        observation: Observation::Probed {
            probe: Probe { kind, target },
            result: ProbeResult::Negative,
        },
        ready_at: Instant(at),
        cost: CostSummary {
            probes: 1,
            time_ns: 1,
        },
    }
}

fn corrected(site: ServiceId, resolved: bool, at: u64) -> Outcome {
    Outcome::Corrected {
        observation: Observation::Correction { site, resolved },
        ready_at: Instant(at),
        cost: CostSummary {
            probes: 3,
            time_ns: 50,
        },
    }
}

/// What a non-terminal, coherent step is, before it is given a time.
#[derive(Debug, Clone)]
enum Kind {
    Probe(ProbeKind, ServiceId),
    Correct(ServiceId, bool),
    RefusedProbe(ServiceId),
    RefusedCorrect(ServiceId),
    RefusedDeclare(ServiceId),
}

fn arb_kind_of_step() -> impl Strategy<Value = Kind> {
    prop_oneof![
        (arb_probe_kind(), arb_service()).prop_map(|(k, s)| Kind::Probe(k, s)),
        (arb_service(), any::<bool>()).prop_map(|(s, r)| Kind::Correct(s, r)),
        arb_service().prop_map(Kind::RefusedProbe),
        arb_service().prop_map(Kind::RefusedCorrect),
        arb_service().prop_map(Kind::RefusedDeclare),
    ]
}

fn make_step(kind: &Kind, at: u64) -> Step {
    let budget = Outcome::Refused(Refusal::BudgetExceeded {
        cost: CostSummary {
            probes: 1,
            time_ns: 1,
        },
    });
    let (action, outcome) = match *kind {
        Kind::Probe(k, t) => (Action::Probe { kind: k, target: t }, probed(k, t, at + 1)),
        Kind::Correct(s, r) => (Action::Correct { site: s }, corrected(s, r, at + 50)),
        Kind::RefusedProbe(t) => (
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: t,
            },
            budget,
        ),
        Kind::RefusedCorrect(s) => (Action::Correct { site: s }, budget),
        Kind::RefusedDeclare(s) => (
            Action::Declare {
                fault: Some((FaultKind::ConfigDrift, s)),
            },
            Outcome::Refused(Refusal::UnknownService(s)),
        ),
    };
    Step {
        at: Instant(at),
        action,
        outcome,
    }
}

/// A prefix of coherent non-terminal steps, with non-decreasing times.
fn arb_prefix() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec((arb_kind_of_step(), 0..1_000u64), 0..12).prop_map(|items| {
        let mut at = 0;
        items
            .iter()
            .map(|(kind, dt)| {
                at += dt;
                make_step(kind, at)
            })
            .collect()
    })
}

fn declare_step(prefix: &[Step], fault: Option<(FaultKind, ServiceId)>) -> Step {
    Step {
        at: prefix.last().map_or(Instant::ZERO, |s| s.at),
        action: Action::Declare { fault },
        outcome: Outcome::Declared,
    }
}

fn accepted(prefix: &[Step], want_probe: bool) -> u32 {
    let n = prefix
        .iter()
        .filter(|s| match &s.outcome {
            Outcome::Probed { .. } => want_probe,
            Outcome::Corrected { .. } => !want_probe,
            _ => false,
        })
        .count();
    u32::try_from(n).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// `score` is a pure function: equal inputs, equal results, including equal errors, over
    /// trajectories that may be incoherent.
    #[test]
    fn score_is_deterministic(
        truth in arb_truth(),
        prefix in arb_prefix(),
        tail in prop::collection::vec(
            (prop::option::of((arb_kind(), arb_service())), 0..3u8, 0..1_000u64), 0..4),
    ) {
        let mut traj = prefix;
        for (fault, shape, at) in tail {
            // Incoherent shapes on purpose: wrong outcomes, steps after the close.
            let (action, outcome) = match shape {
                0 => (Action::Declare { fault }, Outcome::Declared),
                1 => (Action::Abstain, Outcome::Abstained),
                _ => (Action::Abstain, Outcome::Declared),
            };
            traj.push(Step { at: Instant(at), action, outcome });
        }
        let first = score(&truth, &traj);
        let second = score(&truth.clone(), &traj.clone());
        prop_assert_eq!(&first, &second);
        let third = score(&truth, &traj);
        prop_assert_eq!(first, third);
    }

    /// A right declaration after any coherent prefix succeeds, and the counters are the
    /// accepted steps of the prefix (R2, R10, R11, R12).
    #[test]
    fn a_right_declaration_succeeds_and_counts_accepted_steps(
        class in arb_faulted_class(),
        fault in arb_fault(),
        prefix in arb_prefix(),
    ) {
        let truth = Truth { class, faults: vec![fault.clone()] };
        let mut traj = prefix.clone();
        traj.push(declare_step(&prefix, fault.hypothesis()));
        let verdict = score(&truth, &traj).unwrap();
        prop_assert!(verdict.success);
        prop_assert!(!verdict.critical_miss);
        prop_assert!(!verdict.false_alarm);
        prop_assert!(!verdict.undecided);
        prop_assert_eq!(verdict.probes_used, accepted(&prefix, true));
        prop_assert_eq!(verdict.corrections, accepted(&prefix, false));
        prop_assert_eq!(verdict.decision_at, Some(traj.last().unwrap().at));
    }

    /// Changing the declared site of a successful trajectory to any other service makes it fail.
    #[test]
    fn a_wrong_site_is_never_a_success(
        class in arb_faulted_class(),
        fault in arb_fault(),
        prefix in arb_prefix(),
        offset in 1..SERVICES,
    ) {
        let truth = Truth { class, faults: vec![fault.clone()] };
        let mut traj = prefix.clone();
        traj.push(declare_step(&prefix, fault.hypothesis()));
        prop_assert!(score(&truth, &traj).unwrap().success);

        let other = ServiceId((fault.site.0 + offset) % SERVICES);
        prop_assert_ne!(other, fault.site);
        let mut wrong = prefix.clone();
        wrong.push(declare_step(&prefix, Some((fault.kind, other))));
        let verdict = score(&truth, &wrong).unwrap();
        prop_assert!(!verdict.success);
        prop_assert_eq!(verdict.critical_miss, fault.critical);
        prop_assert!(!verdict.false_alarm);
    }

    /// Changing the declared kind of a successful trajectory to any other kind makes it fail.
    #[test]
    fn a_wrong_kind_is_never_a_success(
        class in arb_faulted_class(),
        fault in arb_fault(),
        prefix in arb_prefix(),
        offset in 1..FaultKind::ALL.len(),
    ) {
        let truth = Truth { class, faults: vec![fault.clone()] };
        let index = FaultKind::ALL.iter().position(|k| *k == fault.kind).unwrap();
        let other = FaultKind::ALL[(index + offset) % FaultKind::ALL.len()];
        prop_assert_ne!(other, fault.kind);
        let mut wrong = prefix.clone();
        wrong.push(declare_step(&prefix, Some((other, fault.site))));
        let verdict = score(&truth, &wrong).unwrap();
        prop_assert!(!verdict.success);
        prop_assert_eq!(verdict.critical_miss, fault.critical);
    }

    /// Inserting refused steps anywhere before the close leaves the verdict unchanged (R12).
    #[test]
    fn refusals_change_nothing(
        truth in arb_truth(),
        prefix in arb_prefix(),
        close in prop_oneof![
            Just(None),
            Just(Some(None)),
            (arb_kind(), arb_service()).prop_map(|(k, s)| Some(Some((k, s)))),
        ],
        insert_at in any::<prop::sample::Index>(),
        refusal in prop_oneof![
            arb_service().prop_map(Refusal::UnknownService),
            Just(Refusal::PastHorizon),
            Just(Refusal::BudgetExceeded { cost: CostSummary { probes: 1, time_ns: 1 } }),
        ],
        refused_action in prop_oneof![
            arb_service().prop_map(|t| Action::Probe { kind: ProbeKind::HealthCheck, target: t }),
            arb_service().prop_map(|s| Action::Correct { site: s }),
            Just(Action::Abstain),
            Just(Action::Declare { fault: None }),
        ],
    ) {
        let mut base = prefix.clone();
        match close {
            None => {}
            Some(None) => base.push(Step {
                at: prefix.last().map_or(Instant::ZERO, |s| s.at),
                action: Action::Abstain,
                outcome: Outcome::Abstained,
            }),
            Some(Some(f)) => base.push(declare_step(&prefix, Some(f))),
        }
        let want = score(&truth, &base).unwrap();

        // Insert before the close (or anywhere, if there is none), at the previous step's time.
        let slots = if close.is_some() { base.len() } else { base.len() + 1 };
        let position = insert_at.index(slots);
        let at = if position == 0 { Instant::ZERO } else { base[position - 1].at };
        let mut with = base;
        with.insert(position, Step { at, action: refused_action, outcome: Outcome::Refused(refusal) });
        prop_assert_eq!(score(&truth, &with).unwrap(), want);
    }

    /// Any step after the close is an error naming that step, never a verdict (R14).
    #[test]
    fn a_step_after_the_close_is_an_error(
        truth in arb_truth(),
        prefix in arb_prefix(),
        close_fault in prop::option::of((arb_kind(), arb_service())),
        extra in arb_kind_of_step(),
    ) {
        let mut traj = prefix.clone();
        traj.push(declare_step(&prefix, close_fault));
        let close_index = traj.len() - 1;
        let at = traj[close_index].at.0;
        traj.push(make_step(&extra, at));
        prop_assert_eq!(
            score(&truth, &traj),
            Err(EvalError::ActionAfterTerminal { index: close_index + 1 })
        );
    }
}
