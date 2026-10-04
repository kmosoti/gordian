//! One test per episode class, checking the generator guarantee by inspecting the episode.
//!
//! Each test loops over many seeds and also asserts that the interesting branches were actually
//! taken, so a guarantee cannot hold vacuously.

use super::*;
use crate::episode::StreamLabel;
use crate::fault::FaultKind;
use crate::oracle::reveal;
use crate::physics::{
    CATALOGUE_LIMIT, ENTANGLED, SignalText, SymptomTag, consistent_worlds, signature,
};
use crate::sense::{CounterName, Severity};
use std::collections::BTreeMap;

fn kind_of(ep: &Episode) -> FaultKind {
    reveal(ep).faults[0].kind
}

#[test]
fn ambiguous_has_three_or_more_kinds_and_exactly_one_resolving_probe() {
    let mut kinds_seen = std::collections::BTreeSet::new();
    for seed in SEEDS {
        let ep = episode(EpisodeClass::Ambiguous, seed);
        let t = truth(&ep).unwrap();
        let after_stream = consistent(&ep, ep.stream());
        assert!(after_stream.contains(&Some(t)));
        assert!(
            after_stream
                .iter()
                .all(|h| h.is_some_and(|(_, s)| s == t.1)),
            "site must be identified"
        );
        assert!(
            after_stream.len() >= 3,
            "seed {seed}: only {after_stream:?}"
        );
        assert_eq!(
            after_stream.len(),
            5,
            "every kind fits the shared first symptom"
        );
        kinds_seen.insert(t.0);

        let resolving: Vec<ProbeKind> = ProbeKind::ALL
            .into_iter()
            .filter(|k| {
                let ev = with_probe(&ep, ep.stream(), *k, t.1);
                consistent(&ep, &ev) == vec![Some(t)]
            })
            .collect();
        let expected = match t.0 {
            FaultKind::ResourceExhausted => ProbeKind::ResourceUsage,
            FaultKind::ConfigDrift => ProbeKind::ConfigSnapshot,
            FaultKind::CredentialExpired => ProbeKind::CredentialCheck,
            other => panic!("unexpected truth {other:?}"),
        };
        assert_eq!(resolving, vec![expected], "seed {seed}");

        // A probe at any other service changes nothing.
        for target in (0..ep.world().len() as u32)
            .map(ServiceId)
            .filter(|s| *s != t.1)
        {
            for k in ProbeKind::ALL {
                let ev = with_probe(&ep, ep.stream(), k, target);
                assert_eq!(consistent(&ep, &ev), after_stream);
            }
        }
    }
    // Truths are limited to the kinds that have a dedicated resolving probe (see DESIGN.md).
    assert_eq!(kinds_seen.len(), 3, "{kinds_seen:?}");
}

#[test]
fn delayed_config_change_snapshot_precedes_first_symptom_by_at_least_k() {
    for k in [0u32, 1, 5, 20, 200] {
        for seed in 0..25 {
            let mut spec = EpisodeSpec::new(seed, EpisodeClass::DelayedConfigChange);
            spec.delay_k = k;
            let ep = generate(&spec);
            let t = truth(&ep).unwrap();
            assert_eq!(t.0, FaultKind::ConfigDrift);
            let stream = ep.stream();
            let snap = stream
                .iter()
                .position(|(_, o)| matches!(o, Observation::Snapshot { service, config_hash }
                    if *service == t.1 && *config_hash != ep.world().services[t.1.index()].config_hash))
                .expect("changed snapshot present");
            let symptom = stream
                .iter()
                .position(|(_, o)| matches!(o, Observation::Counter { value, .. } if *value >= crate::physics::HIGH))
                .expect("symptom present");
            assert!(symptom > snap, "snapshot must come first");
            assert!(
                symptom - snap > k as usize,
                "k={k} seed={seed}: {} between",
                symptom - snap - 1
            );
            // Nothing abnormal lies between them.
            assert!(
                stream[snap + 1..symptom]
                    .iter()
                    .all(|(_, o)| !is_abnormal(&ep, o))
            );

            // With the snapshot the fault is identified; without it, it is not.
            assert_eq!(consistent(&ep, stream), vec![Some(t)]);
            let without: Evidence = stream
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != snap)
                .map(|(_, e)| e.clone())
                .collect();
            assert!(consistent(&ep, &without).len() >= 3);
            // ... and a ConfigSnapshot probe at the site recovers it.
            let ev = with_probe(&ep, &without, ProbeKind::ConfigSnapshot, t.1);
            assert_eq!(consistent(&ep, &ev), vec![Some(t)]);
        }
    }
}

fn noise_fraction(ep: &Episode) -> f64 {
    let labels = reveal(ep).labels;
    let noise_msgs = ep
        .stream()
        .iter()
        .zip(&labels)
        .filter(|((_, o), l)| {
            **l == StreamLabel::Noise
                && matches!(o, Observation::Message { text_id, .. } if *text_id >= CATALOGUE_LIMIT)
        })
        .count();
    noise_msgs as f64 / ep.stream().len() as f64
}

#[test]
fn noise_flood_is_at_least_80_percent_irrelevant_high_entropy_messages() {
    let mut ids = std::collections::BTreeSet::new();
    for seed in SEEDS {
        let ep = episode(EpisodeClass::NoiseFlood, seed);
        assert!(
            noise_fraction(&ep) >= 0.8,
            "seed {seed}: {}",
            noise_fraction(&ep)
        );
        for (_, o) in ep.stream() {
            if let Observation::Message { text_id, .. } = o
                && *text_id >= CATALOGUE_LIMIT
            {
                ids.insert(*text_id);
            }
        }
        // The true signal is still decisive on its own.
        assert_eq!(consistent(&ep, ep.stream()), vec![truth(&ep)]);
    }
    // "High entropy": noise ids are essentially all distinct.
    let total: usize = SEEDS
        .map(|s| episode(EpisodeClass::NoiseFlood, s).stream().len())
        .sum();
    assert!(
        ids.len() * 100 >= total * 70,
        "{} distinct of {total}",
        ids.len()
    );
}

#[test]
fn jointly_decisive_probes_are_individually_uninformative_and_jointly_decisive() {
    let (mut dd, mut it, mut bit1) = (0, 0, 0);
    for seed in 0..80 {
        let ep = episode(EpisodeClass::JointlyDecisive, seed);
        let t = truth(&ep).unwrap();
        let hidden = reveal(&ep);
        assert!(t.0 == ENTANGLED.0 || t.0 == ENTANGLED.1);
        assert_eq!(
            hidden.bits.0 == hidden.bits.1,
            t.0 == ENTANGLED.0,
            "parity rule"
        );
        if t.0 == ENTANGLED.0 {
            dd += 1
        } else {
            it += 1
        }
        if hidden.bits.0 {
            bit1 += 1
        }

        let pair = vec![Some((ENTANGLED.0, t.1)), Some((ENTANGLED.1, t.1))];
        assert_eq!(
            consistent(&ep, ep.stream()),
            pair,
            "stream must leave exactly the two kinds"
        );
        let kind_counts = |ev: &[(Instant, Observation)]| {
            let mut m: BTreeMap<FaultKind, usize> = BTreeMap::new();
            for (h, _) in consistent_worlds(&ep.public_info(), ev) {
                *m.entry(h.unwrap().0).or_default() += 1;
            }
            m
        };
        let before = kind_counts(ep.stream());
        assert_eq!(before.values().copied().collect::<Vec<_>>(), vec![2, 2]);
        for k in [ProbeKind::LatencySample, ProbeKind::ErrorSample] {
            let ev = with_probe(&ep, ep.stream(), k, t.1);
            assert_eq!(consistent(&ep, &ev), pair, "one sample must not decide");
            let after = kind_counts(&ev);
            assert_eq!(
                after.values().copied().collect::<Vec<_>>(),
                vec![1, 1],
                "posterior over kinds unchanged"
            );
        }
        let both = with_probe(
            &ep,
            &with_probe(&ep, ep.stream(), ProbeKind::LatencySample, t.1),
            ProbeKind::ErrorSample,
            t.1,
        );
        assert_eq!(consistent(&ep, &both), vec![Some(t)]);
        // No other single probe at the site decides either.
        for k in [
            ProbeKind::HealthCheck,
            ProbeKind::ResourceUsage,
            ProbeKind::ConfigSnapshot,
            ProbeKind::CredentialCheck,
        ] {
            let ev = with_probe(&ep, ep.stream(), k, t.1);
            assert_eq!(consistent(&ep, &ev), pair, "{k:?} must not decide");
        }
    }
    assert!(dd > 20 && it > 20, "both kinds occur: {dd} {it}");
    assert!(bit1 > 20 && bit1 < 60, "bit 1 is balanced: {bit1}");
}

#[test]
fn no_fault_has_no_symptoms_and_probing_every_service_clears_it() {
    for seed in SEEDS {
        let ep = episode(EpisodeClass::NoFault, seed);
        assert!(reveal(&ep).faults.is_empty());
        assert!(!ep.stream().is_empty());
        assert!(ep.stream().iter().all(|(_, o)| !is_abnormal(&ep, o)));
        assert!(consistent(&ep, ep.stream()).contains(&None));
        // The stream alone cannot exclude faults (absence is not evidence); health checks can.
        let mut ev = ep.stream().to_vec();
        for s in 0..ep.world().len() as u32 {
            ev = with_probe(&ep, &ev, ProbeKind::HealthCheck, ServiceId(s));
        }
        assert_eq!(consistent(&ep, &ev), vec![None]);
    }
}

#[test]
fn critical_flag_is_set_for_exactly_two_classes() {
    for class in EpisodeClass::ALL {
        for seed in 0..20 {
            let ep = episode(class, seed);
            let faults = reveal(&ep).faults;
            match class {
                EpisodeClass::NoFault => assert!(faults.is_empty()),
                EpisodeClass::CriticalFault | EpisodeClass::QuietUrgent => {
                    assert_eq!(faults.len(), 1);
                    assert!(faults[0].critical, "{class:?}");
                }
                _ => {
                    assert_eq!(faults.len(), 1);
                    assert!(!faults[0].critical, "{class:?}");
                }
            }
        }
    }
}

#[test]
fn quiet_urgent_has_one_low_severity_true_signal_in_a_flood() {
    let mut low_noise_seen = false;
    for seed in SEEDS {
        let ep = episode(EpisodeClass::QuietUrgent, seed);
        let labels = reveal(&ep).labels;
        let signals: Vec<_> = ep
            .stream()
            .iter()
            .zip(&labels)
            .filter(|(_, l)| matches!(l, StreamLabel::Signal { .. }))
            .collect();
        assert_eq!(signals.len(), 1, "exactly one true observation");
        match &signals[0].0.1 {
            Observation::Message {
                severity, text_id, ..
            } => {
                assert_eq!(*severity, Severity::Low);
                assert!(*text_id < CATALOGUE_LIMIT);
            }
            other => panic!("signal must be a message, got {other:?}"),
        }
        assert!(noise_fraction(&ep) >= 0.8);
        // The one message decides kind and site.
        assert_eq!(consistent(&ep, ep.stream()), vec![truth(&ep)]);
        // Low severity is not a tell: noise also contains Low messages.
        low_noise_seen |= ep.stream().iter().zip(&labels).any(|((_, o), l)| {
            *l == StreamLabel::Noise
                && matches!(
                    o,
                    Observation::Message {
                        severity: Severity::Low,
                        ..
                    }
                )
        });
    }
    assert!(low_noise_seen);
}

#[test]
fn duplicates_repeat_each_true_observation_two_to_five_times() {
    for seed in SEEDS {
        let ep = episode(EpisodeClass::Duplicates, seed);
        let labels = reveal(&ep).labels;
        let mut groups: BTreeMap<u32, Vec<&Observation>> = BTreeMap::new();
        for ((_, o), l) in ep.stream().iter().zip(&labels) {
            if let StreamLabel::Signal { group } = l {
                groups.entry(*group).or_default().push(o);
            }
        }
        assert!(groups.len() >= 2);
        for (g, copies) in &groups {
            assert!(
                (2..=5).contains(&copies.len()),
                "group {g} has {}",
                copies.len()
            );
            assert!(copies.iter().all(|o| *o == copies[0]), "identical content");
        }
        assert_eq!(consistent(&ep, ep.stream()), vec![truth(&ep)]);
    }
}

#[test]
fn feedback_bait_decoy_health_check_loops_until_the_budget_stops_it() {
    for seed in SEEDS {
        let ep = episode(EpisodeClass::FeedbackBait, seed);
        let t = truth(&ep).unwrap();
        let decoys: Vec<_> = ep
            .world()
            .services
            .iter()
            .filter(|s| s.unreliable_health)
            .collect();
        assert_eq!(decoys.len(), 1);
        let decoy = decoys[0].id;
        assert_ne!(decoy, t.1);
        let labels = reveal(&ep).labels;
        let bait: Vec<_> = ep
            .stream()
            .iter()
            .zip(&labels)
            .filter(|(_, l)| **l == StreamLabel::Bait)
            .collect();
        assert!(bait.len() >= 2);
        assert!(bait.iter().all(|((_, o), _)| matches!(o,
            Observation::Message { service, text_id, .. }
                if *service == decoy && *text_id == SignalText::CheckHealth.text_id())));

        // Follow the suggestions: every answer asks for the same probe again.
        let mut sim = Simulator::new(ep.clone());
        let mut action = Action::Probe {
            kind: ProbeKind::HealthCheck,
            target: decoy,
        };
        let mut evidence = ep.stream().to_vec();
        let mut steps = 0;
        loop {
            match sim.apply(action, Instant::ZERO) {
                Outcome::Probed { observation, .. } => {
                    steps += 1;
                    let Observation::Probed { probe, result } = &observation else {
                        panic!()
                    };
                    let crate::sense::ProbeResult::Inconclusive {
                        suggest: Some(next),
                    } = result
                    else {
                        panic!("decoy must be inconclusive, got {result:?}")
                    };
                    assert_eq!(*next, *probe, "suggestion repeats the same probe");
                    action = Action::Probe {
                        kind: next.kind,
                        target: next.target,
                    };
                    evidence.push((Instant::ZERO, observation));
                }
                Outcome::Refused(crate::step::Refusal::BudgetExceeded { .. }) => break,
                other => panic!("{other:?}"),
            }
        }
        // Only the budget stopped the loop: every probe cost 1 and all of them were spent.
        assert_eq!(steps, ep.spec().budget.probes);
        assert_eq!(sim.remaining().probes, 0);
        // Following the bait taught the policy nothing.
        assert_eq!(consistent(&ep, &evidence), consistent(&ep, ep.stream()));
    }
}

#[test]
fn stale_memory_has_a_matching_record_that_is_wrong_and_not_excluded_by_the_rules() {
    for seed in SEEDS {
        let ep = episode(EpisodeClass::StaleMemory, seed);
        let t = truth(&ep).unwrap();
        let sig = signature(ep.stream());
        assert_eq!(sig, vec![SymptomTag::Counter(CounterName::ErrorRate)]);
        let matching: Vec<_> = ep
            .public_info()
            .prior_records
            .into_iter()
            .filter(|r| r.signature == sig)
            .collect();
        assert_eq!(matching.len(), 1);
        assert_ne!(matching[0].resolution, t.0, "record must be wrong now");
        // The rules cannot rule the stale answer out: only a probe can.
        assert!(consistent(&ep, ep.stream()).contains(&Some((matching[0].resolution, t.1))));
    }
}

#[test]
fn prior_records_for_identified_signatures_are_correct() {
    for class in [
        EpisodeClass::NoiseFlood,
        EpisodeClass::Duplicates,
        EpisodeClass::QuietUrgent,
        EpisodeClass::ComponentTimeout,
    ] {
        for seed in SEEDS {
            let ep = episode(class, seed);
            let k = kind_of(&ep);
            let sig = signature(ep.stream());
            let matching: Vec<_> = ep
                .public_info()
                .prior_records
                .into_iter()
                .filter(|r| r.signature == sig)
                .collect();
            assert!(
                !matching.is_empty(),
                "{class:?} seed {seed}: no record for {sig:?}"
            );
            assert!(matching.iter().all(|r| r.resolution == k));
        }
    }
}

#[test]
fn component_timeout_carries_harness_directives_and_other_classes_do_not() {
    let mut modes = (false, false);
    for class in EpisodeClass::ALL {
        for seed in 0..30 {
            let ep = episode(class, seed);
            let d = ep.harness_directives();
            if class == EpisodeClass::ComponentTimeout {
                assert!((1..=3).contains(&d.len()));
                let mut ids: Vec<u32> = d.iter().map(|x| x.component).collect();
                ids.sort();
                ids.dedup();
                assert_eq!(ids.len(), d.len(), "distinct components");
                for x in d {
                    match x.mode {
                        crate::ComponentMode::Fail => modes.0 = true,
                        crate::ComponentMode::Slow { factor } => {
                            assert!(factor >= 2);
                            modes.1 = true;
                        }
                    }
                }
            } else {
                assert!(d.is_empty(), "{class:?}");
            }
        }
    }
    assert!(modes.0 && modes.1);
}

#[test]
fn critical_fault_covers_both_plain_and_identified_signals() {
    let (mut plain, mut identified) = (0, 0);
    for seed in SEEDS {
        let ep = episode(EpisodeClass::CriticalFault, seed);
        if consistent(&ep, ep.stream()).len() > 1 {
            plain += 1
        } else {
            identified += 1
        }
    }
    assert!(plain > 5 && identified > 5, "{plain} {identified}");
}
