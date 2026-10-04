//! Property tests over generated episodes: bounded window, determinism, entry kinds, and the
//! relations between the verifier, the estimator, and the world's checker.
//!
//! Nothing here reads hidden state. Where the plan asks for soundness against the truth, that is
//! the world's own test; these tests compare components with the public checker.

mod common;

use common::{all_components, class_spec, full_window, spec_strategy, state_of};
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, RuleHeuristic, VERIFIER_ID, WorkingState,
};
use gordian_core::{EntryKind, Instant};
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{
    Action, EpisodeClass, Hypothesis, Observation, ProbeKind, Simulator, generate,
};
use proptest::prelude::*;

fn candidates(payload: &[u8]) -> Vec<Hypothesis> {
    match decode(payload).expect("payload decodes") {
        HypothesisEntry::Candidates { ranked, .. } => {
            ranked.into_iter().map(|r| r.hypothesis).collect()
        }
        HypothesisEntry::EvidenceDamaged { .. } => Vec::new(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// `size()` never exceeds the capacity after any admit sequence, and the window is exactly
    /// the last `capacity` admitted observations in admission order (the documented rule).
    #[test]
    fn window_is_bounded_and_keeps_the_most_recent(
        spec in spec_strategy(),
        capacity in 0usize..48,
        repeats in 1usize..3,
    ) {
        let ep = generate(&spec);
        let mut w = WorkingState::new(ep.public_info(), capacity);
        let mut admitted: Vec<(Instant, Observation)> = Vec::new();
        for _ in 0..repeats {
            for (t, o) in ep.stream() {
                w.admit(*t, o.clone());
                admitted.push((*t, o.clone()));
                prop_assert!(w.size() <= capacity);
                prop_assert_eq!(w.size(), admitted.len().min(capacity));
            }
        }
        let keep = admitted.len().saturating_sub(capacity);
        let expected: Vec<_> = admitted[keep..].to_vec();
        let held: Vec<_> = w.evidence().iter().cloned().collect();
        prop_assert_eq!(held, expected);
        prop_assert_eq!(w.capacity(), capacity);
    }

    /// Each component's `run` is a function of the input: the same instance twice, and a fresh
    /// instance, give identical output, for windows of any size over any prefix.
    #[test]
    fn run_is_deterministic(spec in spec_strategy(), cap in 0usize..80, upto in 0usize..400) {
        let ep = generate(&spec);
        let w = state_of(&ep, cap, upto);
        for mut component in all_components() {
            let first = component.run(&w);
            let second = component.run(&w);
            prop_assert_eq!(&first, &second, "component {:?}", component.id());
        }
        for (mut a, mut b) in all_components().into_iter().zip(all_components()) {
            prop_assert_eq!(a.run(&w), b.run(&w));
        }
    }

    /// Every entry a component emits is a hypothesis whose payload decodes, and requests obey
    /// the acyclic rule: never to itself, and the verifier asks for nothing.
    #[test]
    fn entries_are_hypotheses_and_requests_are_acyclic(
        spec in spec_strategy(),
        cap in 0usize..80,
        upto in 0usize..400,
    ) {
        let ep = generate(&spec);
        let w = state_of(&ep, cap, upto);
        for mut component in all_components() {
            let out = component.run(&w);
            for (kind, payload) in &out.entries {
                prop_assert_eq!(*kind, EntryKind::Hypothesis);
                prop_assert!(decode(payload).is_ok());
            }
            for request in &out.requests {
                prop_assert_ne!(request.component, component.id());
                prop_assert!(!request.reason.is_empty() && request.reason.len() <= 64);
            }
            if component.id() == VERIFIER_ID {
                prop_assert!(out.requests.is_empty());
            }
        }
    }

    /// With room for the whole stream the verifier returns exactly the world checker's set on
    /// the full stream, and that set is not empty.
    #[test]
    fn verifier_equals_the_checker_on_the_full_stream(spec in spec_strategy()) {
        let (ep, w) = full_window(&spec);
        let expected = consistent_hypotheses(&w.public, ep.stream());
        prop_assert!(!expected.is_empty());
        let out = ConsistencyVerifier::new().run(&w);
        prop_assert_eq!(out.entries.len(), 1);
        prop_assert_eq!(candidates(&out.entries[0].1), expected.clone());
        prop_assert_eq!(out.proposal, if expected.len() == 1 { Some(expected[0]) } else { None });
    }

    /// With room for the whole stream, the hypotheses the estimator scores highest are exactly
    /// the checker's set, so its top hypothesis is in the set. (For `CriticalFault` this is the
    /// property named by the plan; it is checked over every class.)
    #[test]
    fn estimator_top_set_is_the_consistent_set_on_the_full_stream(spec in spec_strategy()) {
        let (ep, w) = full_window(&spec);
        let consistent = consistent_hypotheses(&w.public, ep.stream());
        prop_assert!(!consistent.is_empty());
        let scores = CountEstimator::new().scores(&w);
        let best = scores.iter().map(|(_, s)| *s).max().unwrap();
        let top: Vec<Hypothesis> = scores.iter().filter(|(_, s)| *s == best).map(|(h, _)| *h).collect();
        prop_assert_eq!(top, consistent.clone());
        let out = CountEstimator::new().run(&w);
        let ranked = candidates(&out.entries[0].1);
        prop_assert!(consistent.contains(&ranked[0]));
    }

    /// With the whole stream in the window, every candidate the heuristic lists is in the
    /// checker's set. The heuristic's rules are written from the same physics, so it is wrong,
    /// relative to the checker, only when the window has lost evidence; "wrong" relative to the
    /// hidden truth is a statement about ambiguity, which this crate cannot test.
    #[test]
    fn heuristic_candidates_are_consistent_on_the_full_stream(spec in spec_strategy()) {
        let (ep, w) = full_window(&spec);
        let set = consistent_hypotheses(&w.public, ep.stream());
        let out = RuleHeuristic::new().run(&w);
        for (_, payload) in &out.entries {
            for h in candidates(payload) {
                prop_assert!(set.contains(&h), "{h:?} not in {set:?}");
            }
        }
    }

    /// The named case: on a `CriticalFault` stream the estimator's top hypothesis is in the
    /// verifier's consistent set.
    #[test]
    fn estimator_top_is_verifier_consistent_for_critical_faults(
        spec in class_spec(EpisodeClass::CriticalFault),
    ) {
        let (_, w) = full_window(&spec);
        let verifier = ConsistencyVerifier::new().run(&w);
        let set = candidates(&verifier.entries[0].1);
        let estimator = CountEstimator::new().run(&w);
        let top = candidates(&estimator.entries[0].1)[0];
        prop_assert!(set.contains(&top));
    }

    /// The same relation holds after one paid probe result joins the stream (one probe cannot
    /// separate the estimator's per-observation view from the verifier's joint view).
    #[test]
    fn estimator_top_set_matches_the_checker_after_one_probe(
        spec in spec_strategy(),
        kind in 0usize..6,
        target in 0usize..12,
    ) {
        let ep = generate(&spec);
        let n_services = ep.world().len();
        let mut sim = Simulator::new(ep.clone());
        let outcome = sim.apply(
            Action::Probe {
                kind: ProbeKind::ALL[kind],
                target: gordian_world::ServiceId((target % n_services) as u32),
            },
            Instant::ZERO,
        );
        let gordian_world::Outcome::Probed { observation, .. } = outcome else {
            return Err(TestCaseError::reject("probe refused by the budget"));
        };
        let mut evidence = ep.stream().to_vec();
        evidence.push((ep.spec().horizon, observation));
        let mut w = WorkingState::new(ep.public_info(), evidence.len());
        for (t, o) in &evidence {
            w.admit(*t, o.clone());
        }
        let consistent = consistent_hypotheses(&w.public, &evidence);
        prop_assume!(!consistent.is_empty());
        let scores = CountEstimator::new().scores(&w);
        let best = scores.iter().map(|(_, s)| *s).max().unwrap();
        let top: Vec<Hypothesis> = scores.iter().filter(|(_, s)| *s == best).map(|(h, _)| *h).collect();
        prop_assert_eq!(top, consistent);
    }

    /// A declared cost is a function of sizes only. Two windows of the same length over the
    /// same services cost the same whatever they contain, and the cost never decreases as the
    /// window grows.
    #[test]
    fn declared_cost_depends_on_sizes_only(spec in spec_strategy(), n in 0usize..64) {
        let ep = generate(&spec);
        prop_assume!(!ep.stream().is_empty());
        let mut a = WorkingState::new(ep.public_info(), n);
        let mut b = WorkingState::new(ep.public_info(), n);
        for (i, (t, o)) in ep.stream().iter().cycle().take(n).enumerate() {
            a.admit(*t, o.clone());
            // Same size, different content: the same stream read from another offset.
            let (t2, o2) = &ep.stream()[(i + 1) % ep.stream().len()];
            b.admit(*t2, o2.clone());
        }
        let mut grown = a.clone();
        if let Some((t, o)) = ep.stream().first() {
            // A larger window over the same services (capacity allows one more).
            grown = WorkingState::new(ep.public_info(), n + 1);
            for (t0, o0) in a.evidence() {
                grown.admit(*t0, o0.clone());
            }
            grown.admit(*t, o.clone());
        }
        for component in all_components() {
            prop_assert_eq!(component.declared_cost(&a), component.declared_cost(&b));
            let small: u64 = component.declared_cost(&a).iter().map(|c| c.amount).sum();
            let big: u64 = component.declared_cost(&grown).iter().map(|c| c.amount).sum();
            prop_assert!(big >= small);
        }
    }
}
