//! The policy interface and the policies built so far.
//!
//! A policy decides, at each step of an episode, which components to run and what to do next.
//! It is the only thing that differs between the arms of an experiment; components, weights,
//! inputs and evaluator stay fixed.
//!
//! # What a policy may see
//!
//! Only a [`WorkingState`] (public information and a bounded window of observations the policy
//! has been shown), the [`Bill`] (limits and spend so far), and the outputs of the components it
//! selected in the current step. It never sees hidden simulator state, the episode class, the
//! harness directives, the evaluator, or measured wall-clock time. Files in this directory must
//! not name the hidden-state accessor, the evaluator crate, the evaluator's truth type, the
//! generated-episode type or the simulator; `scripts/check-no-oracle.sh` checks that, except
//! for the privileged `oracle.rs` (item A6, not built).
//!
//! # Costs
//!
//! A policy declares its own scheduling cost through [`Policy::declared_select_cost`]; the
//! harness charges it under `Phase::Scheduling` once per step, before `select`. A policy that
//! declares nothing must say so with [`zero_cost`], so that a zero is a recorded statement
//! rather than an omission. The harness also *times* `select` and `decide` and records the
//! timing in the ledger, but a policy never sees a timing, so a timing cannot influence a
//! decision.

pub mod heuristic_only;
pub mod scripted;

use gordian_components::{ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Resource};
use gordian_world::Action;
use serde::{Deserialize, Serialize};

/// The identity of a policy, as written in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyId(pub String);

impl PolicyId {
    /// A policy id from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// A scheduling policy.
///
/// One instance plays one episode: the recorder builds a fresh policy per episode with
/// [`build`], so no state leaks between episodes.
pub trait Policy {
    /// Stable identity, written to the ledger and the manifest.
    fn id(&self) -> PolicyId;

    /// What one step of this policy's own selection and decision work is declared to cost.
    ///
    /// Charged under `Phase::Scheduling` once per step, before [`Policy::select`]. A function of
    /// `state` only, with no side effects: the harness may call it more than once per step (it
    /// also asks whether any work is still affordable). Return [`zero_cost`] to declare a free
    /// policy.
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge>;

    /// The components to run this step, in order. Duplicates run once. An id that names no
    /// component of the run is a harness error. Not called when the scheduling charge was
    /// refused.
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId>;

    /// What to do after this step's components have run. `outputs` holds the output of each
    /// selected component that ran and did not fail, in the order they ran. `None` waits for
    /// more observations.
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action>;
}

/// The explicit declaration that a policy's selection costs nothing: one `Compute` charge of
/// zero. It is recorded in the ledger at every step.
pub fn zero_cost() -> Vec<Charge> {
    vec![Charge::new(Resource::Compute, 0)]
}

/// Ids of the policies [`build`] knows.
pub const KNOWN: &[&str] = &[heuristic_only::ID];

/// A fresh policy for `id`, or `None` when no policy has that id. Item A6 adds the other
/// baselines here; the [`Policy`] trait does not change.
pub fn build(id: &PolicyId) -> Option<Box<dyn Policy>> {
    match id.0.as_str() {
        heuristic_only::ID => Some(Box::new(heuristic_only::HeuristicOnly::new())),
        _ => None,
    }
}
