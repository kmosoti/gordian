//! The public checker on hand-built evidence, and JSON round trips.

use super::*;
use crate::episode::{BudgetSpec, PublicInfo};
use crate::fault::FaultKind::{self, *};
use crate::graph::{ResourceKind, Service};
use crate::physics::SignalText;
use crate::sense::{CounterName, Probe, ProbeResult, Severity};
use crate::step::Refusal;

/// A chain 0 <- 1 <- 2 <- 3 (each depends on the one before) plus service 4 depending on 0.
fn public() -> PublicInfo {
    let dep = |i: u32, ds: &[u32]| Service {
        id: ServiceId(i),
        depends_on: ds.iter().copied().map(ServiceId).collect(),
        resource: ResourceKind::Cpu,
        config_hash: 1000 + i as u64,
        unreliable_health: i == 3,
    };
    PublicInfo {
        services: vec![
            dep(0, &[]),
            dep(1, &[0]),
            dep(2, &[1]),
            dep(3, &[2]),
            dep(4, &[0]),
        ],
        prior_records: vec![],
        horizon: Instant(10_000_000_000),
        budget: BudgetSpec::default(),
    }
}

fn counter(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

fn at(obs: Vec<Observation>) -> Evidence {
    obs.into_iter()
        .enumerate()
        .map(|(i, o)| (Instant(i as u64 + 1), o))
        .collect()
}

fn hyps(ev: &Evidence) -> Vec<crate::Hypothesis> {
    crate::physics::consistent_hypotheses(&public(), ev)
}

fn s(i: u32) -> ServiceId {
    ServiceId(i)
}

#[test]
fn no_evidence_contradicts_nothing() {
    assert_eq!(hyps(&vec![]).len(), 1 + 5 * 5);
}

#[test]
fn benign_observations_contradict_nothing() {
    let ev = at(vec![
        counter(2, CounterName::ErrorRate, 49),
        Observation::Message {
            service: s(1),
            text_id: 0xDEAD_BEEF_0000,
            severity: Severity::Critical,
        },
        Observation::Message {
            service: s(1),
            text_id: SignalText::CheckHealth.text_id(),
            severity: Severity::High,
        },
        Observation::Snapshot {
            service: s(2),
            config_hash: 1002,
        },
    ]);
    assert_eq!(hyps(&ev).len(), 26);
}

#[test]
fn an_abnormal_counter_excludes_no_fault_and_pins_the_site() {
    let ev = at(vec![counter(1, CounterName::ErrorRate, 90)]);
    let h = hyps(&ev);
    assert_eq!(
        h,
        FaultKind::ALL
            .iter()
            .map(|k| Some((*k, s(1))))
            .collect::<Vec<_>>()
    );
}

#[test]
fn dependent_symptoms_narrow_kinds_and_need_an_earlier_anchor() {
    let ev = at(vec![
        counter(1, CounterName::ErrorRate, 90),
        counter(2, CounterName::Latency, 80),
    ]);
    assert_eq!(
        hyps(&ev),
        vec![
            Some((ResourceExhausted, s(1))),
            Some((DependencyDown, s(1))),
            Some((Intermittent, s(1)))
        ]
    );
    // Same two observations in the other order: the dependent symptom has no cause yet.
    let rev: Evidence = vec![ev[1].clone(), ev[0].clone()];
    assert!(hyps(&rev).is_empty());
    // A message that only a *dependent* can emit, with no cause before it, is contradicted by
    // every hypothesis. (A bare latency counter is not: it fits a fault at that very service.)
    let orphan = Observation::Message {
        service: s(2),
        text_id: SignalText::UpstreamUnreachable.text_id(),
        severity: Severity::Low,
    };
    assert!(hyps(&at(vec![orphan])).is_empty());
}

#[test]
fn symptoms_do_not_flow_upstream() {
    // ErrorRate at 0 and at 4 (which depends on 0): site 0 only. Site 4 is not an ancestor of 0.
    let ev = at(vec![
        counter(0, CounterName::ErrorRate, 90),
        counter(4, CounterName::ErrorRate, 90),
    ]);
    let sites: std::collections::BTreeSet<_> =
        hyps(&ev).into_iter().flatten().map(|(_, s)| s).collect();
    assert_eq!(sites, [s(0)].into_iter().collect());
    // ErrorRate at 4 and at 2 (unrelated branches): nothing explains both.
    let ev = at(vec![
        counter(4, CounterName::ErrorRate, 90),
        counter(2, CounterName::ErrorRate, 90),
    ]);
    assert!(hyps(&ev).is_empty());
}

#[test]
fn characteristic_messages_decide_kind_and_site() {
    let m = |service, t: SignalText| Observation::Message {
        service: s(service),
        text_id: t.text_id(),
        severity: Severity::Low,
    };
    assert_eq!(
        hyps(&at(vec![m(2, SignalText::Unauthorized)])),
        vec![Some((CredentialExpired, s(2)))]
    );
    // UpstreamUnreachable at 2 needs an anchor at its upstream site.
    let ev = at(vec![
        counter(1, CounterName::ErrorRate, 90),
        m(2, SignalText::UpstreamUnreachable),
    ]);
    assert_eq!(hyps(&ev), vec![Some((DependencyDown, s(1)))]);
    assert_eq!(
        hyps(&at(vec![m(2, SignalText::MixedSignals)])),
        vec![Some((DependencyDown, s(2))), Some((Intermittent, s(2)))]
    );
}

#[test]
fn changed_snapshot_means_config_drift_at_that_service_and_only_one_new_value() {
    let snap = |service: u32, h: u64| Observation::Snapshot {
        service: s(service),
        config_hash: h,
    };
    assert_eq!(hyps(&at(vec![snap(3, 7)])), vec![Some((ConfigDrift, s(3)))]);
    assert!(
        hyps(&at(vec![snap(3, 7), snap(3, 8)])).is_empty(),
        "one drifted value per service"
    );
    assert_eq!(
        hyps(&at(vec![snap(3, 7), snap(3, 7)])),
        vec![Some((ConfigDrift, s(3)))]
    );
}

#[test]
fn probe_results_must_match_the_rules() {
    let probed = |kind, target: u32, result| Observation::Probed {
        probe: Probe {
            kind,
            target: s(target),
        },
        result,
    };
    let base = counter(1, CounterName::ErrorRate, 90);
    let ev = at(vec![
        base.clone(),
        probed(ProbeKind::ResourceUsage, 1, ProbeResult::Positive),
    ]);
    assert_eq!(hyps(&ev), vec![Some((ResourceExhausted, s(1)))]);
    let ev = at(vec![
        base.clone(),
        probed(ProbeKind::ConfigSnapshot, 1, ProbeResult::ConfigHash(1001)),
    ]);
    assert!(!hyps(&ev).contains(&Some((ConfigDrift, s(1)))));
    let ev = at(vec![
        base.clone(),
        probed(ProbeKind::ConfigSnapshot, 1, ProbeResult::ConfigHash(555)),
    ]);
    assert_eq!(hyps(&ev), vec![Some((ConfigDrift, s(1)))]);
    // A wrong-typed result is a contradiction, not a shrug.
    let ev = at(vec![
        base.clone(),
        probed(ProbeKind::ResourceUsage, 1, ProbeResult::ConfigHash(1)),
    ]);
    assert!(hyps(&ev).is_empty());
    // Unreliable health endpoint: only the self-suggesting inconclusive answer is admissible.
    let me = ProbeResult::Inconclusive {
        suggest: Some(Probe {
            kind: ProbeKind::HealthCheck,
            target: s(3),
        }),
    };
    assert_eq!(
        hyps(&at(vec![probed(ProbeKind::HealthCheck, 3, me)])).len(),
        26
    );
    assert!(
        hyps(&at(vec![probed(
            ProbeKind::HealthCheck,
            3,
            ProbeResult::Negative
        )]))
        .is_empty()
    );
    assert!(hyps(&at(vec![probed(ProbeKind::HealthCheck, 2, me)])).is_empty());
    // Healthy everywhere is "no fault" or a fault elsewhere.
    let ev = at(vec![probed(
        ProbeKind::HealthCheck,
        2,
        ProbeResult::Negative,
    )]);
    assert!(hyps(&ev).contains(&None) && !hyps(&ev).contains(&Some((ConfigDrift, s(2)))));
}

#[test]
fn the_parity_rule_needs_both_bits() {
    let probed = |kind, result| Observation::Probed {
        probe: Probe { kind, target: s(1) },
        result,
    };
    let mixed = Observation::Message {
        service: s(1),
        text_id: SignalText::MixedSignals.text_id(),
        severity: Severity::Low,
    };
    let pair = vec![Some((DependencyDown, s(1))), Some((Intermittent, s(1)))];
    for r1 in [ProbeResult::Positive, ProbeResult::Negative] {
        let one = at(vec![mixed.clone(), probed(ProbeKind::LatencySample, r1)]);
        assert_eq!(hyps(&one), pair);
        for r2 in [ProbeResult::Positive, ProbeResult::Negative] {
            let two = at(vec![
                mixed.clone(),
                probed(ProbeKind::LatencySample, r1),
                probed(ProbeKind::ErrorSample, r2),
            ]);
            let expect = if r1 == r2 {
                DependencyDown
            } else {
                Intermittent
            };
            assert_eq!(hyps(&two), vec![Some((expect, s(1)))]);
        }
    }
}

#[test]
fn correction_effects_are_evidence_about_the_site() {
    let ev = at(vec![
        counter(1, CounterName::ErrorRate, 90),
        Observation::Correction {
            site: s(1),
            resolved: true,
        },
    ]);
    assert_eq!(hyps(&ev).len(), 5);
    let ev = at(vec![Observation::Correction {
        site: s(1),
        resolved: false,
    }]);
    assert!(hyps(&ev).contains(&None) && hyps(&ev).iter().flatten().all(|(_, site)| *site != s(1)));
}

#[test]
fn dangling_references_contradict_everything() {
    assert!(hyps(&at(vec![counter(9, CounterName::ErrorRate, 1)])).is_empty());
}

#[test]
fn json_round_trips() {
    fn rt<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) {
        let text = serde_json::to_string(v).unwrap();
        let back: T = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, v, "{text}");
    }
    for class in EpisodeClass::ALL {
        let ep = episode(class, 11);
        rt(&ep.spec().clone());
        rt(&ep.public_info());
        rt(&ep);
        rt(&crate::oracle::reveal(&ep));
        for (_, o) in ep.stream().iter().take(20) {
            rt(o);
        }
    }
    for a in [
        Action::Abstain,
        Action::Correct { site: s(2) },
        Action::Declare { fault: None },
        Action::Declare {
            fault: Some((Intermittent, s(1))),
        },
        Action::Probe {
            kind: ProbeKind::ErrorSample,
            target: s(0),
        },
    ] {
        rt(&a);
    }
    for o in [
        Outcome::Declared,
        Outcome::Abstained,
        Outcome::Refused(Refusal::EpisodeOver),
        Outcome::Refused(Refusal::BudgetExceeded {
            cost: Default::default(),
        }),
        Outcome::Probed {
            observation: Observation::Correction {
                site: s(1),
                resolved: true,
            },
            ready_at: Instant(5),
            cost: Default::default(),
        },
    ] {
        rt(&o);
    }
}

#[test]
fn spec_json_uses_plain_nanoseconds() {
    let spec = EpisodeSpec::new(7, EpisodeClass::Ambiguous);
    let v: serde_json::Value = serde_json::to_value(&spec).unwrap();
    assert_eq!(v["horizon"], 10_000_000_000u64);
    assert_eq!(v["class"], "Ambiguous");
}
