//! Simulator behaviour: delivery, refusal, budget, termination.

use super::*;
use crate::fault::FaultKind;
use crate::oracle::reveal;
use crate::physics::{correct_cost, probe_cost};
use crate::sense::ProbeResult;
use crate::step::{CostSummary, Refusal};
use gordian_core::Resource;

fn sim(class: EpisodeClass, seed: u64) -> (Episode, Simulator) {
    let ep = episode(class, seed);
    (ep.clone(), Simulator::new(ep))
}

#[test]
fn observe_until_delivers_each_observation_once_in_order() {
    let (ep, mut sim) = sim(EpisodeClass::NoiseFlood, 3);
    let horizon = ep.spec().horizon.0;
    let mut got: Evidence = Vec::new();
    for step in 1..=50u64 {
        let now = Instant(horizon * step / 50);
        let batch = sim.observe_until(now);
        assert!(batch.iter().all(|(t, _)| *t <= now));
        got.extend(batch);
    }
    assert_eq!(got, ep.stream());
    assert!(
        sim.observe_until(Instant(horizon)).is_empty(),
        "nothing is delivered twice"
    );
}

#[test]
fn observe_until_never_goes_back() {
    let (ep, mut sim) = sim(EpisodeClass::Ambiguous, 1);
    let first_symptom = ep.stream().iter().map(|(t, _)| *t).max().unwrap();
    let all = sim.observe_until(first_symptom);
    assert_eq!(all.len(), ep.stream().len());
    assert!(sim.observe_until(Instant::ZERO).is_empty());
}

#[test]
fn declare_and_abstain_close_the_episode() {
    for closing in [Action::Abstain, Action::Declare { fault: None }] {
        let (_, mut sim) = sim(EpisodeClass::NoFault, 0);
        assert!(!sim.is_over());
        let out = sim.apply(closing, Instant::ZERO);
        assert!(matches!(out, Outcome::Abstained | Outcome::Declared));
        assert!(sim.is_over());
        let before = sim.remaining();
        for action in [
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: ServiceId(0),
            },
            Action::Correct { site: ServiceId(0) },
            Action::Abstain,
            Action::Declare { fault: None },
        ] {
            assert_eq!(
                sim.apply(action, Instant(1)),
                Outcome::Refused(Refusal::EpisodeOver)
            );
        }
        assert_eq!(sim.remaining(), before);
        assert!(sim.observe_until(Instant(u64::MAX)).is_empty());
    }
}

#[test]
fn declaring_never_reveals_correctness() {
    let (ep, mut sim) = sim(EpisodeClass::Ambiguous, 2);
    let t = truth(&ep).unwrap();
    let wrong = FaultKind::ALL.into_iter().find(|k| *k != t.0).unwrap();
    let a = Simulator::new(ep.clone()).apply(Action::Declare { fault: Some(t) }, Instant::ZERO);
    let b = sim.apply(
        Action::Declare {
            fault: Some((wrong, t.1)),
        },
        Instant::ZERO,
    );
    assert_eq!(
        a, b,
        "right and wrong declarations are indistinguishable to the policy"
    );
}

#[test]
fn budget_is_a_hard_limit_and_refusal_changes_nothing() {
    let (ep, mut sim) = sim(EpisodeClass::NoFault, 5);
    let limit = ep.spec().budget;
    let mut spent = CostSummary::default();
    let target = ServiceId(0);
    loop {
        let before = sim.remaining();
        match sim.apply(
            Action::Probe {
                kind: ProbeKind::LatencySample,
                target,
            },
            Instant::ZERO,
        ) {
            Outcome::Probed { cost, .. } => {
                spent.probes += cost.probes;
                spent.time_ns += cost.time_ns;
            }
            Outcome::Refused(Refusal::BudgetExceeded { cost }) => {
                assert_eq!(sim.remaining(), before, "refusal charges nothing");
                assert!(cost.probes > before.probes || cost.time_ns > before.time_ns);
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(spent.probes <= limit.probes && spent.time_ns <= limit.time_ns);
    assert_eq!(sim.remaining().probes, limit.probes - spent.probes);
    // The cheapest probe may still fit; the budget is per-resource, not per-action.
    assert!(!sim.is_over());
}

#[test]
fn time_must_not_go_backwards_and_the_horizon_is_a_deadline() {
    let (ep, mut sim) = sim(EpisodeClass::NoFault, 1);
    let action = Action::Probe {
        kind: ProbeKind::HealthCheck,
        target: ServiceId(0),
    };
    assert!(matches!(
        sim.apply(action, Instant(1000)),
        Outcome::Probed { .. }
    ));
    assert_eq!(
        sim.apply(action, Instant(999)),
        Outcome::Refused(Refusal::TimeWentBackwards)
    );
    sim.observe_until(Instant(5000));
    assert_eq!(
        sim.apply(action, Instant(4999)),
        Outcome::Refused(Refusal::TimeWentBackwards)
    );
    let late = Instant(ep.spec().horizon.0 + 1);
    assert_eq!(
        sim.apply(action, late),
        Outcome::Refused(Refusal::PastHorizon)
    );
    assert_eq!(
        sim.apply(Action::Abstain, late),
        Outcome::Refused(Refusal::PastHorizon)
    );
    assert!(matches!(
        sim.apply(action, ep.spec().horizon),
        Outcome::Probed { .. }
    ));
}

#[test]
fn ready_at_is_now_plus_declared_time_cost() {
    let (_, mut sim) = sim(EpisodeClass::Ambiguous, 0);
    let now = Instant(123_456);
    let Outcome::Probed { ready_at, cost, .. } = sim.apply(
        Action::Probe {
            kind: ProbeKind::ConfigSnapshot,
            target: ServiceId(0),
        },
        now,
    ) else {
        panic!()
    };
    assert_eq!(
        cost,
        CostSummary::from_charges(&probe_cost(ProbeKind::ConfigSnapshot))
    );
    assert_eq!(ready_at, Instant(now.0 + cost.time_ns));
}

#[test]
fn every_probe_and_the_correction_have_a_declared_cost() {
    for k in ProbeKind::ALL {
        let c = probe_cost(k);
        let sum = CostSummary::from_charges(&c);
        assert!(sum.probes >= 1, "{k:?}");
        assert!(sum.time_ns > 0, "{k:?}");
        assert_eq!(c.len(), 2);
        assert!(
            c.iter()
                .all(|x| x.resource == Resource::Probes || x.resource == Resource::Time)
        );
    }
    let c = CostSummary::from_charges(&correct_cost());
    assert!(c.probes >= 1 && c.time_ns > 0);
}

#[test]
fn correction_resolves_only_at_the_fault_site() {
    for seed in 0..20 {
        let ep = episode(EpisodeClass::Ambiguous, seed);
        let t = truth(&ep).unwrap();
        for s in 0..ep.world().len() as u32 {
            let mut sim = Simulator::new(ep.clone());
            let Outcome::Corrected { observation, .. } =
                sim.apply(Action::Correct { site: ServiceId(s) }, Instant::ZERO)
            else {
                panic!()
            };
            assert_eq!(
                observation,
                Observation::Correction {
                    site: ServiceId(s),
                    resolved: ServiceId(s) == t.1
                }
            );
        }
    }
    let ep = episode(EpisodeClass::NoFault, 0);
    let mut sim = Simulator::new(ep);
    let Outcome::Corrected { observation, .. } =
        sim.apply(Action::Correct { site: ServiceId(0) }, Instant::ZERO)
    else {
        panic!()
    };
    assert_eq!(
        observation,
        Observation::Correction {
            site: ServiceId(0),
            resolved: false
        }
    );
}

#[test]
fn probe_results_follow_the_public_table_on_the_true_site() {
    let mut saw = [false; 3];
    for seed in 0..60 {
        let ep = episode(EpisodeClass::Ambiguous, seed);
        let t = truth(&ep).unwrap();
        let hidden = reveal(&ep);
        let result = |k| match probe_obs(&ep, k, t.1).1 {
            Observation::Probed { result, .. } => result,
            _ => unreachable!(),
        };
        assert_eq!(result(ProbeKind::HealthCheck), ProbeResult::Positive);
        assert_eq!(
            result(ProbeKind::ResourceUsage) == ProbeResult::Positive,
            t.0 == FaultKind::ResourceExhausted
        );
        assert_eq!(
            result(ProbeKind::CredentialCheck) == ProbeResult::Positive,
            t.0 == FaultKind::CredentialExpired
        );
        let start = ep.world().services[t.1.index()].config_hash;
        match (t.0, result(ProbeKind::ConfigSnapshot)) {
            (FaultKind::ConfigDrift, ProbeResult::ConfigHash(h)) => {
                assert_ne!(h, start);
                assert_eq!(Some(h), hidden.drift_hash);
                saw[0] = true;
            }
            (_, ProbeResult::ConfigHash(h)) => {
                assert_eq!(h, start);
                saw[1] = true;
            }
            other => panic!("{other:?}"),
        }
        // The samples are negative for kinds outside the parity pair.
        assert_eq!(result(ProbeKind::LatencySample), ProbeResult::Negative);
        saw[2] = true;
    }
    assert!(saw.iter().all(|x| *x));
}
