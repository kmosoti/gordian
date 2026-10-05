//! The checker's operation counts (work item A8b).
//!
//! The equivalence tests (`equivalence.rs`) already show, over about the same inputs as the A5b
//! work, that the counted checker returns what the plain checker and the reference return, and
//! that the counts do not change between calls. This file tests the counts themselves: that
//! they have the structure the units claim, that they follow the content of the evidence and
//! not only its length (which is the reason they exist), and that the counted dependents mask
//! is the mask the reference uses.

use super::*;
use crate::episode::PublicInfo;
use crate::graph::{dependents_mask, dependents_mask_counted};
use crate::physics::{
    CheckerOps, HIGH, SignalText, consistent_hypotheses_counted, consistent_worlds_counted,
};
use crate::sense::{CounterName, Severity};

fn ctr(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

#[test]
fn counted_mask_equals_the_mask_the_reference_uses() {
    for class in EpisodeClass::ALL {
        for seed in 0..40 {
            let ep = episode(class, seed);
            let services = &ep.public_info().services;
            // Include a site outside the graph.
            for site in 0..=services.len() as u32 {
                let (counted, steps) = dependents_mask_counted(services, ServiceId(site));
                assert_eq!(counted, dependents_mask(services, ServiceId(site)));
                if (site as usize) < services.len() {
                    // One step per visited service, at least.
                    assert!(steps >= (services.len() - site as usize - 1) as u64);
                } else {
                    assert_eq!(steps, 0);
                }
            }
        }
    }
}

#[test]
fn counts_have_the_structure_of_the_units() {
    for class in EpisodeClass::ALL {
        for seed in 0..40 {
            let ep = episode(class, seed);
            let public = ep.public_info();
            let s = public.services.len() as u64;
            let stream = ep.stream();
            for cut in [0, stream.len() / 2, stream.len()] {
                let evidence = &stream[..cut];
                let (_, ops) = consistent_worlds_counted(&public, evidence);
                assert_eq!(ops.scanned, cut as u64, "{class:?} {seed}");
                // `no fault`, and for each site five kinds, two of them under two bit settings.
                assert_eq!(ops.worlds_tried, 1 + 7 * s, "{class:?} {seed}");
                assert!(ops.mask_steps >= s, "{class:?} {seed}");
                // Every world looks at one observation at least when there is any informative
                // one, and never at more than all of them.
                let informative = evidence
                    .iter()
                    .filter(|(_, o)| is_abnormal(&ep, o))
                    .count() as u64;
                assert!(ops.evals + ops.probe_evals <= ops.worlds_tried * informative);
                if informative > 0 {
                    assert!(ops.evals + ops.probe_evals >= ops.worlds_tried);
                } else {
                    assert_eq!(ops.evals + ops.probe_evals, 0);
                }
                assert_eq!(ops.probe_evals, 0, "no probe results in a passive stream");
            }
        }
    }
}

#[test]
fn probe_results_are_counted_as_probe_evaluations() {
    let ep = episode(EpisodeClass::Ambiguous, 3);
    let public = ep.public_info();
    let mut evidence: Evidence = ep.stream().to_vec();
    let before = consistent_worlds_counted(&public, &evidence).1;
    for kind in ProbeKind::ALL {
        evidence.push(probe_obs(&ep, kind, ServiceId(0)));
    }
    let after = consistent_worlds_counted(&public, &evidence).1;
    assert!(after.probe_evals > 0);
    assert_eq!(before.probe_evals, 0);
    // The passive observations are evaluated at least as often as before; a world a probe
    // contradicts stops there, but never before it has seen the passive ones.
    assert!(after.evals >= before.evals);
    assert_eq!(after.scanned, before.scanned + ProbeKind::ALL.len() as u64);
}

/// The reason the counters exist: two windows of the same length and the same graph that cost
/// different amounts of work are counted differently. A refuted-early window against one
/// every surviving world must read to the end.
#[test]
fn counts_follow_content_not_length() {
    let public: PublicInfo = episode(EpisodeClass::Ambiguous, 3).public_info();
    let site = public
        .services
        .iter()
        .find(|s| s.depends_on.is_empty())
        .map(|s| s.id)
        .expect("a root service");
    let n = 64;
    // Every observation is an `ErrorRate` alarm at one site: every world with that site
    // survives to the end, every other world is contradicted at the first observation.
    let focused: Evidence = (0..n)
        .map(|k| (Instant(k as u64), ctr(site, CounterName::ErrorRate, HIGH)))
        .collect();
    // The same length, but the alarms are spread over every service in turn, so almost every
    // world is contradicted early (a world is contradicted by an alarm at a service that it is
    // neither at nor upstream of).
    let spread: Evidence = (0..n)
        .map(|k| {
            let service = ServiceId((k % public.services.len()) as u32);
            (Instant(k as u64), ctr(service, CounterName::ErrorRate, HIGH))
        })
        .collect();
    // Same length and nothing informative: no world is evaluated at all.
    let quiet: Evidence = (0..n)
        .map(|k| (Instant(k as u64), ctr(site, CounterName::ErrorRate, HIGH - 1)))
        .collect();
    let ops_focused = consistent_worlds_counted(&public, &focused).1;
    let ops_spread = consistent_worlds_counted(&public, &spread).1;
    let ops_quiet = consistent_worlds_counted(&public, &quiet).1;
    assert_eq!(ops_focused.scanned, ops_spread.scanned);
    assert_eq!(ops_focused.scanned, ops_quiet.scanned);
    assert_eq!(ops_quiet.evals, 0);
    assert!(
        ops_focused.evals > ops_spread.evals,
        "{ops_focused:?} against {ops_spread:?}"
    );
    assert!(ops_spread.evals > 0);
}

#[test]
fn a_dangling_reference_stops_the_scan_and_the_count() {
    let ep = episode(EpisodeClass::NoiseFlood, 1);
    let public = ep.public_info();
    let mut evidence: Evidence = ep.stream().to_vec();
    let at = evidence.len() / 2;
    evidence.insert(
        at,
        (
            Instant(1),
            Observation::Message {
                service: ServiceId(public.services.len() as u32 + 5),
                text_id: SignalText::OutOfResource.text_id(),
                severity: Severity::Low,
            },
        ),
    );
    let (worlds, ops) = consistent_worlds_counted(&public, &evidence);
    assert!(worlds.is_empty());
    assert_eq!(ops.scanned, at as u64 + 1);
    assert_eq!(ops.worlds_tried, 0);
    assert_eq!(ops.evals + ops.probe_evals + ops.mask_steps, 0);
    let (hyps, ops_h) = consistent_hypotheses_counted(&public, &evidence);
    assert!(hyps.is_empty());
    assert_eq!(ops_h, ops);
}

#[test]
fn the_total_is_the_sum_of_the_fields() {
    let ops = CheckerOps {
        scanned: 1,
        mask_steps: 2,
        worlds_tried: 3,
        evals: 4,
        probe_evals: 5,
    };
    assert_eq!(ops.total(), 15);
}
