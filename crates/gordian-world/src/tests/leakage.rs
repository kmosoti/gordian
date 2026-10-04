//! Adversarial checks that hidden state does not leak through things that should be
//! uninformative: the stream's content, order and length, probe cost and timing, and the hidden
//! bits' marginals.
//!
//! The strongest of these rebuilds the *same* seed with a different true kind drawn from the same
//! ambiguity set. Every random draw is taken before the kind is applied, so if the public output
//! differs at all, the generator has leaked the kind.

use super::*;
use crate::builder::build_forced;
use crate::fault::FaultKind;
use crate::oracle::reveal;
use crate::physics::ENTANGLED;
use crate::step::Outcome;

fn ambiguity_set(class: EpisodeClass, seed: u64) -> Vec<FaultKind> {
    let ep = episode(class, seed);
    consistent(&ep, ep.stream())
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
        .collect()
}

fn assert_indistinguishable(
    class: EpisodeClass,
    seeds: std::ops::Range<u64>,
    expect_sizes: &[usize],
) {
    let mut sizes = std::collections::BTreeSet::new();
    for seed in seeds {
        let spec = EpisodeSpec::new(seed, class);
        let set = ambiguity_set(class, seed);
        if !expect_sizes.contains(&set.len()) {
            continue; // an Identified draw in CriticalFault: decided by design, nothing to hide
        }
        sizes.insert(set.len());
        let reference = build_forced(&spec, set[0]);
        for kind in &set {
            let ep = build_forced(&spec, *kind);
            assert_eq!(reveal(&ep).faults[0].kind, *kind);
            assert_eq!(
                ep.stream(),
                reference.stream(),
                "{class:?} seed {seed}: stream depends on kind {kind:?}"
            );
            assert_eq!(ep.world(), reference.world());
            assert_eq!(
                ep.public_info(),
                reference.public_info(),
                "public info depends on kind"
            );
            assert_eq!(ep.harness_directives(), reference.harness_directives());
            // And the checker agrees that nothing public separates them.
            assert_eq!(
                consistent(&ep, ep.stream()),
                consistent(&reference, reference.stream())
            );
        }
    }
    assert_eq!(
        sizes.into_iter().collect::<Vec<_>>(),
        expect_sizes.to_vec(),
        "{class:?}: expected set sizes not all exercised"
    );
}

#[test]
fn ambiguous_streams_are_identical_across_the_ambiguity_set() {
    assert_indistinguishable(EpisodeClass::Ambiguous, 0..80, &[5]);
}

#[test]
fn jointly_decisive_streams_are_identical_for_both_kinds() {
    assert_indistinguishable(EpisodeClass::JointlyDecisive, 0..80, &[2]);
}

#[test]
fn feedback_bait_and_critical_plain_streams_are_identical_across_the_set() {
    assert_indistinguishable(EpisodeClass::FeedbackBait, 0..40, &[5]);
    assert_indistinguishable(EpisodeClass::CriticalFault, 0..60, &[5]);
}

#[test]
fn stale_memory_stream_is_identical_across_kinds_even_though_the_stale_record_is_not() {
    for seed in 0..40 {
        let spec = EpisodeSpec::new(seed, EpisodeClass::StaleMemory);
        let a = build_forced(&spec, FaultKind::ALL[0]);
        let b = build_forced(&spec, FaultKind::ALL[4]);
        assert_eq!(a.stream(), b.stream());
    }
    // Known, intended residual: the stale record is wrong by construction, so its resolution is
    // weakly informative ("truth is not this kind"). It is what the class is. Recorded in DESIGN.md.
}

#[test]
fn probe_cost_and_timing_do_not_depend_on_the_truth() {
    for class in [EpisodeClass::Ambiguous, EpisodeClass::JointlyDecisive] {
        for seed in 0..30 {
            let spec = EpisodeSpec::new(seed, class);
            let set = ambiguity_set(class, seed);
            let site = truth(&episode(class, seed)).unwrap().1;
            let mut shapes = Vec::new();
            for kind in &set {
                let mut sim = Simulator::new(build_forced(&spec, *kind));
                let mut shape = Vec::new();
                for k in ProbeKind::ALL {
                    for rep in 0..3 {
                        match sim.apply(
                            Action::Probe {
                                kind: k,
                                target: site,
                            },
                            Instant(rep * 7),
                        ) {
                            Outcome::Probed { ready_at, cost, .. } => {
                                shape.push(Some((ready_at, cost)))
                            }
                            Outcome::Refused(r) => {
                                shape.push(None);
                                let _ = r;
                            }
                            other => panic!("{other:?}"),
                        }
                    }
                }
                shapes.push(shape);
            }
            assert!(
                shapes.windows(2).all(|w| w[0] == w[1]),
                "cost/timing/refusal pattern depends on kind"
            );
        }
    }
}

#[test]
fn hidden_bits_are_balanced_within_each_kind() {
    // A single bit must tell nothing about the kind: P(bit = 1 | kind) is 1/2 for both.
    let mut counts = std::collections::BTreeMap::<(FaultKind, usize), (u32, u32)>::new();
    for seed in 0..600 {
        let ep = episode(EpisodeClass::JointlyDecisive, seed);
        let h = reveal(&ep);
        let k = h.faults[0].kind;
        for (i, b) in [h.bits.0, h.bits.1].into_iter().enumerate() {
            let e = counts.entry((k, i)).or_default();
            e.0 += b as u32;
            e.1 += 1;
        }
    }
    assert_eq!(counts.len(), 4);
    for ((k, i), (ones, n)) in counts {
        assert!(n > 200, "{k:?} bit {i}: only {n} samples");
        let p = ones as f64 / n as f64;
        assert!((0.38..=0.62).contains(&p), "{k:?} bit {i}: P(1) = {p}");
    }
    let _ = ENTANGLED;
}

#[test]
fn periodic_snapshots_do_not_avoid_the_fault_site() {
    // Absence of snapshots at one service would point at the fault before its first symptom.
    let (mut at_site, mut elsewhere, mut other_services) = (0u32, 0u32, 0u32);
    for seed in 0..200 {
        let mut spec = EpisodeSpec::new(seed, EpisodeClass::Ambiguous);
        spec.noise_rate = 20;
        let ep = generate(&spec);
        let site = truth(&ep).unwrap().1;
        for (_, o) in ep.stream() {
            if let Observation::Snapshot { service, .. } = o {
                if *service == site {
                    at_site += 1;
                } else {
                    elsewhere += 1;
                }
            }
        }
        other_services += ep.world().len() as u32 - 1;
    }
    let per_site = at_site as f64 / 200.0;
    let per_other = elsewhere as f64 / other_services as f64;
    assert!(
        per_site > 0.7 * per_other && per_site < 1.3 * per_other,
        "{per_site} vs {per_other}"
    );
}

#[test]
fn the_volatile_prior_record_is_independent_of_the_truth_outside_stale_memory() {
    // In StaleMemory the matching record is wrong by definition. Everywhere else it must be
    // right about as often as chance (1/5), or it would leak that class and the truth.
    let (mut right, mut n) = (0u32, 0u32);
    for seed in 0..500 {
        let ep = episode(EpisodeClass::Ambiguous, seed);
        let kind = truth(&ep).unwrap().0;
        let sig = vec![crate::physics::SymptomTag::Counter(
            crate::CounterName::ErrorRate,
        )];
        let rec = ep
            .public_info()
            .prior_records
            .into_iter()
            .find(|r| r.signature == sig)
            .unwrap();
        right += (rec.resolution == kind) as u32;
        n += 1;
    }
    let p = right as f64 / n as f64;
    assert!((0.14..=0.26).contains(&p), "P(record right) = {p}");
}
