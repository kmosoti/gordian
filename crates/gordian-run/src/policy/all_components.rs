//! `all_components`: every component at every step.
//!
//! The charter's "all-component execution" baseline: what selection misses, and not assumed to be
//! an accuracy ceiling. All four components run at every step, in id order. They read the
//! working state and not each other's output, so the order inside a step does not matter to what
//! they produce.
//!
//! With the default configuration `fixed_pipeline` selects exactly the same components, so the
//! two arms are the same function until B1 tunes the pipeline.

use super::{PolicyId, Selector, zero_cost};
use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId};

/// The id this policy is registered under.
pub const ID: &str = "all_components";

/// The selector.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllComponents;

impl Selector for AllComponents {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        vec![HEURISTIC_ID, ESTIMATOR_ID, MEMORY_ID, VERIFIER_ID]
    }
}
