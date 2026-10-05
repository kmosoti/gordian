//! Recurrence and regime change occur as specified.

use super::*;
use crate::kinds::{HardKind, Tier};
use crate::oracle::{IncidentTruth, RegimeDetail, ShapeTruth, StreamTruth};
use crate::{RegimeKind, RegimeSchedule, StreamKind};
use gordian_core::Instant;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{SignalText, characteristic_message, consistent_hypotheses};
use gordian_world::{FaultKind, Observation, ServiceId};

fn no_regimes(seed: u64, r: u32) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.regimes.clear();
    p.recurrence_permille = r;
    p
}

/// For incident `i`, whether some earlier incident could have been repeated: all of its services
/// free at `i`'s onset.
fn has_eligible_template(t: &StreamTruth, i: usize) -> bool {
    let onset = t.incidents[i].onset_ns;
    let busy = |s: &ServiceId| {
        t.incidents[..i]
            .iter()
            .any(|k| k.occupies.contains(s) && k.busy_until_ns > onset)
    };
    t.incidents[..i]
        .iter()
        .any(|j| j.occupies.iter().all(|s| !busy(s)))
}

#[test]
fn recurrence_frequency_matches_the_parameter() {
    for r in [250u32, 600] {
        let (mut eligible, mut recurring, mut recurring_without_template) = (0u64, 0u64, 0u64);
        let streams = if r == 250 { 160 } else { 60 };
        for seed in 0..streams {
            let (_, t) = with_truth(&no_regimes(seed, r));
            for (i, inc) in t.incidents.iter().enumerate() {
                let elig = has_eligible_template(&t, i);
                eligible += elig as u64;
                if inc.recurrence_of.is_some() {
                    recurring += 1;
                    if !elig {
                        recurring_without_template += 1;
                    }
                }
            }
        }
        assert_eq!(recurring_without_template, 0, "r = {r}");
        let p = r as f64 / 1000.0;
        let n = eligible as f64;
        let freq = recurring as f64 / n;
        // The few arrivals dropped for want of a free service never recur, which can only raise
        // the share among those that were kept; allow for it.
        let sd = (p * (1.0 - p) / n).sqrt();
        println!("r = {p}: {recurring}/{eligible} = {freq:.4} (sd {sd:.4})");
        assert!(
            (freq - p).abs() < 4.5 * sd + 0.01,
            "r = {p}: {freq} over {n}"
        );
    }
}

#[test]
fn a_recurrence_repeats_kind_and_site_and_criticality_but_not_the_noise() {
    let (mut n, mut identical_evidence) = (0, 0);
    for seed in 0..60 {
        let (s, t) = with_truth(&no_regimes(seed, 500));
        for inc in &t.incidents {
            let Some(from) = inc.recurrence_of else {
                continue;
            };
            let old = &t.incidents[from as usize];
            n += 1;
            assert!(old.busy_until_ns <= inc.onset_ns);
            assert_eq!(inc.tier, old.tier);
            assert_eq!(inc.truth, old.truth, "same kind and site");
            assert_eq!(inc.occupies, old.occupies);
            assert_eq!(inc.critical, old.critical);
            assert_eq!(inc.difficulty, old.difficulty);
            let same_shape = |a: &ShapeTruth, b: &ShapeTruth| a == b;
            assert!(same_shape(&inc.shape, &old.shape));
            // Different noise: the observations, relative to onset, are not the same.
            let rel = |i: &IncidentTruth| -> Vec<(u64, Observation)> {
                i.observations
                    .iter()
                    .map(|o| {
                        let (at, ob) = &s.events()[o.0 as usize];
                        (at.0 - i.onset_ns, ob.clone())
                    })
                    .collect()
            };
            if rel(inc) == rel(old) {
                identical_evidence += 1;
            }
        }
    }
    assert!(n > 200, "{n}");
    assert_eq!(
        identical_evidence, 0,
        "a recurrence replays the old evidence"
    );
}

#[test]
fn within_a_stream_each_hard_kind_has_a_small_vocabulary_of_message_ids_and_no_two_share() {
    use std::collections::{BTreeMap, BTreeSet};
    let mut kinds_seen = BTreeSet::new();
    for seed in 0..30 {
        let (s, t) = with_truth(&no_regimes(seed, 250));
        let mut vocab: BTreeMap<HardKind, BTreeSet<u64>> = Default::default();
        for inc in &t.incidents {
            if inc.tier != Tier::Hard {
                continue;
            }
            for id in &inc.decisive {
                if let Observation::Message { text_id, .. } = &s.events()[id.0 as usize].1
                    && SignalText::from_text_id(*text_id).is_none()
                {
                    vocab
                        .entry(inc.shape.hard_kind.unwrap())
                        .or_default()
                        .insert(*text_id);
                }
            }
        }
        let mut all = BTreeSet::new();
        for (k, ids) in &vocab {
            assert!(ids.len() <= 4, "seed {seed}: {k:?} uses {} ids", ids.len());
            for id in ids {
                assert!(
                    all.insert(*id),
                    "seed {seed}: {k:?}: id shared with another kind"
                );
            }
            kinds_seen.insert(*k);
        }
    }
    assert_eq!(kinds_seen.len(), 4);
}

// ---- Regime changes

fn regime_params(seed: u64) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.mix.plain_permille = 1000;
    p.mix.hard_permille = 0;
    p.recurrence_permille = 0;
    p.mean_gap_ns = 8_000_000_000;
    p.regimes = vec![
        RegimeSchedule {
            at_ns: 150_000_000_000,
            change: RegimeKind::SignatureShift,
        },
        RegimeSchedule {
            at_ns: 300_000_000_000,
            change: RegimeKind::EdgeAdd,
        },
    ];
    p
}

#[test]
fn regime_changes_happen_at_the_scheduled_times_and_say_what_they_changed() {
    for seed in 0..40 {
        let (_, t) = with_truth(&regime_params(seed));
        assert_eq!(t.regimes.len(), 2);
        assert_eq!(t.regimes[0].at_ns, 150_000_000_000);
        assert_eq!(t.regimes[1].at_ns, 300_000_000_000);
        match t.regimes[0].detail {
            RegimeDetail::SignatureShift { kind, now_emits } => assert_ne!(kind, now_emits),
            d => panic!("{d:?}"),
        }
        match t.regimes[1].detail {
            RegimeDetail::EdgeAdd {
                dependent,
                dependency,
            } => {
                assert!(dependency.index() < dependent.index());
                let svc = &t.services;
                assert!(!svc[dependent.index()].depends_on.contains(&dependency));
                assert!(
                    !dependents_mask(svc, dependency)[dependent.index()],
                    "the new edge adds nothing: already a dependent"
                );
            }
            d => panic!("{d:?}"),
        }
    }
}

/// The characteristic message of `kind` among an incident's observations at its site.
fn site_char_messages(s: &crate::Stream, inc: &IncidentTruth) -> Vec<SignalText> {
    let site = inc.occupies[0];
    inc.observations
        .iter()
        .filter_map(|o| match &s.events()[o.0 as usize].1 {
            Observation::Message {
                service, text_id, ..
            } if *service == site => SignalText::from_text_id(*text_id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_signature_shift_changes_one_kinds_characteristic_message_from_its_instant_on() {
    let (mut before, mut after, mut other_kinds_after) = (0, 0, 0);
    for seed in 0..120 {
        let (s, t) = with_truth(&regime_params(seed));
        let RegimeDetail::SignatureShift { kind, now_emits } = t.regimes[0].detail else {
            panic!()
        };
        let at = t.regimes[0].at_ns;
        for inc in &t.incidents {
            let Some(k) = inc.shape.known_kind else {
                continue;
            };
            if inc.shape.duo {
                continue; // a duo carries no characteristic message
            }
            let msgs = site_char_messages(&s, inc);
            assert_eq!(msgs.len(), 1, "{inc:?}");
            let own = characteristic_message(k);
            if k != kind || inc.onset_ns < at {
                assert_eq!(msgs[0], own, "kind {k:?} onset {}", inc.onset_ns);
                if k == kind {
                    before += 1;
                } else if inc.onset_ns >= at {
                    other_kinds_after += 1;
                }
            } else {
                assert_eq!(msgs[0], characteristic_message(now_emits));
                assert_ne!(msgs[0], own);
                after += 1;
            }
        }
    }
    assert!(
        before > 20 && after > 20 && other_kinds_after > 100,
        "{before} {after} {other_kinds_after}"
    );
}

#[test]
fn an_edge_add_makes_faults_upstream_of_it_alarm_a_service_the_public_graph_says_is_unaffected() {
    let (mut before, mut after) = (0, 0);
    for seed in 0..300 {
        let (s, t) = with_truth(&regime_params(seed));
        let RegimeDetail::EdgeAdd {
            dependent,
            dependency,
        } = t.regimes[1].detail
        else {
            panic!()
        };
        let at = t.regimes[1].at_ns;
        for inc in &t.incidents {
            if inc.occupies[0] != dependency || inc.shape.duo {
                continue;
            }
            let alarms_dependent = inc.observations.iter().any(|o| {
                matches!(&s.events()[o.0 as usize].1,
                    Observation::Counter { service, .. } if *service == dependent)
            });
            if inc.onset_ns >= at {
                assert!(
                    alarms_dependent,
                    "seed {seed}: the new edge carried nothing"
                );
                after += 1;
            } else {
                assert!(
                    !alarms_dependent,
                    "seed {seed}: alarmed before the edge existed"
                );
                before += 1;
            }
        }
    }
    assert!(before > 10 && after > 10, "{before} {after}");
}

#[test]
fn a_regime_change_makes_the_cheap_rung_wrong_exactly_where_it_claims() {
    // The first world's checker keeps the first world's rules and the time-zero graph. Affected
    // incidents are the ones whose own evidence then excludes their true hypothesis.
    let (mut shift_wrong, mut shift_total, mut edge_wrong, mut edge_total, mut other_wrong) =
        (0, 0, 0, 0, 0);
    for seed in 0..200 {
        let (s, t) = with_truth(&regime_params(seed));
        let public = s.public_info().world_public_info();
        let RegimeDetail::SignatureShift { kind, .. } = t.regimes[0].detail else {
            panic!()
        };
        for inc in &t.incidents {
            let ev = evidence_of(&s, &t, inc.id);
            let h = inc.truth.unwrap();
            let StreamKind::Known(k) = h.kind else {
                panic!()
            };
            let truth = Some((k, h.site));
            let sound = consistent_hypotheses(&public, &ev).contains(&truth);
            let after_shift = inc.onset_ns >= t.regimes[0].at_ns;
            let after_edge = inc.onset_ns >= t.regimes[1].at_ns;
            let shifted = after_shift && k == kind && !inc.shape.duo;
            // Affected by the new edge: some observation of the incident is at a service the
            // time-zero graph does not make a dependent of its site.
            let initial = dependents_mask(&t.services, h.site);
            let edge_hit = after_edge
                && inc.observations.iter().any(|o| {
                    let sv = match &s.events()[o.0 as usize].1 {
                        Observation::Counter { service, .. }
                        | Observation::Message { service, .. } => *service,
                        _ => return false,
                    };
                    sv != h.site && !initial[sv.index()]
                });
            if shifted {
                shift_total += 1;
                shift_wrong += (!sound) as u32;
            } else if edge_hit {
                edge_total += 1;
                edge_wrong += (!sound) as u32;
            } else if !sound {
                other_wrong += 1;
            }
        }
    }
    assert!(
        shift_total > 20 && edge_total > 10,
        "{shift_total} {edge_total}"
    );
    assert_eq!(
        shift_wrong, shift_total,
        "a shifted signature must exclude the truth"
    );
    assert_eq!(edge_wrong, edge_total, "a new edge must exclude the truth");
    assert_eq!(other_wrong, 0, "an unaffected incident is still sound");
}

#[test]
fn probe_answers_do_not_change_with_the_regime() {
    use super::cheap::probing_sim;
    use crate::{StreamAction, StreamOutcome};
    use gordian_world::physics::probe_result;
    use gordian_world::{Probe, ProbeKind};
    let p = regime_params(3);
    let (s, t) = with_truth(&p);
    let template = probing_sim(&p);
    let services = s.public_info().services;
    let mut after = 0;
    for inc in t.incidents.iter().filter(|i| i.onset_ns > 320_000_000_000) {
        let site = inc.occupies[0];
        let kind = inc.shape.known_kind.unwrap();
        let mut sim = template.clone();
        let StreamOutcome::Probed {
            observation: Observation::Probed { result, .. },
            ..
        } = sim.apply(
            StreamAction::Probe {
                kind: ProbeKind::ResourceUsage,
                target: site,
            },
            Instant(inc.onset_ns + 500_000_000),
        )
        else {
            panic!()
        };
        let want = probe_result(
            &services,
            Some((kind, site)),
            (false, false),
            0,
            Probe {
                kind: ProbeKind::ResourceUsage,
                target: site,
            },
        );
        assert_eq!(result, want);
        after += 1;
    }
    assert!(after > 5);
    let _ = FaultKind::ALL;
}
