//! `heuristic_only`: select the rule heuristic at every step, and nothing else.
//!
//! The arm is the shared decision rule ([`super::decide`]) fed by the heuristic alone. It is the
//! charter's "simple task-specific heuristic" baseline: whether the task needs more than a rule
//! table. Because the rule is shared, it probes when the heuristic's candidates are ambiguous and
//! declares what the heuristic proposes; what it cannot do is rule candidates out by symptoms,
//! which is the estimator's and the verifier's work and which it never selects.
//!
//! Until work item A6 this arm had its own rule (declare a unique heuristic fault, otherwise wait
//! for a patience and guess, never probe). That rule is gone: it made this arm differ from the
//! others in `decide`, which EXP-001 forbids. The id is unchanged.

pub use super::decide::DEFAULT_PATIENCE;
use super::decide::DecideConfig;
use super::{Arm, PolicyId, Selector, zero_cost};
use gordian_components::{HEURISTIC_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Instant};

/// The id this policy is registered under.
pub const ID: &str = "heuristic_only";

/// The selector: the heuristic, every step.
#[derive(Debug, Clone, Copy, Default)]
pub struct Heuristic;

impl Selector for Heuristic {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        vec![HEURISTIC_ID]
    }
}

/// The `heuristic_only` policy.
pub type HeuristicOnly = Arm<Heuristic>;

impl Default for Arm<Heuristic> {
    fn default() -> Self {
        Self::new()
    }
}

impl Arm<Heuristic> {
    /// The policy with the default patience.
    pub fn new() -> Self {
        Arm::with(Heuristic, DecideConfig::default())
    }

    /// The policy with the shared rule declaring its fallback at `patience`.
    pub fn with_patience(patience: Instant) -> Self {
        Arm::with(
            Heuristic,
            DecideConfig {
                patience_ns: patience.0,
            },
        )
    }
}
