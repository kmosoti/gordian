//! Helpers shared by the integration tests.
#![allow(dead_code)]

use gordian_components::{Component, ConsistencyVerifier, CountEstimator, PriorRecordLookup};
use gordian_components::{RuleHeuristic, WorkingState};
use gordian_core::Instant;
use gordian_world::episode::BudgetSpec;
use gordian_world::{Episode, EpisodeClass, EpisodeSpec, generate};
use proptest::prelude::*;

/// Specs over every class and the full range of generator parameters.
pub fn spec_strategy() -> impl Strategy<Value = EpisodeSpec> {
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

/// A spec of one fixed class.
pub fn class_spec(class: EpisodeClass) -> impl Strategy<Value = EpisodeSpec> {
    spec_strategy().prop_map(move |mut s| {
        s.class = class;
        s
    })
}

/// A working state holding the first `upto` observations of the episode's stream, with room for
/// `capacity` of them.
pub fn state_of(ep: &Episode, capacity: usize, upto: usize) -> WorkingState {
    let mut w = WorkingState::new(ep.public_info(), capacity);
    for (t, o) in ep.stream().iter().take(upto) {
        w.admit(*t, o.clone());
    }
    w
}

/// A working state whose window holds the whole stream.
pub fn full_window(spec: &EpisodeSpec) -> (Episode, WorkingState) {
    let ep = generate(spec);
    let n = ep.stream().len();
    let w = state_of(&ep, n, n);
    (ep, w)
}

/// One fresh instance of every component, with the lookup reading every record.
pub fn all_components() -> Vec<Box<dyn Component>> {
    vec![
        Box::new(RuleHeuristic::new()),
        Box::new(CountEstimator::new()),
        Box::new(PriorRecordLookup::new()),
        Box::new(ConsistencyVerifier::new()),
    ]
}
