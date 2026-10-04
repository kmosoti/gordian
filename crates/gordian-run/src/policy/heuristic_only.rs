//! `heuristic_only`: run the rule heuristic and declare what it ranks first.
//!
//! This is the one real policy built in work item A4, as an end-to-end smoke test of the
//! harness. Item A6 builds the baselines proper and may replace it.
//!
//! # Rule
//!
//! Every step it selects the heuristic component only. It declares as soon as the heuristic has
//! a unique best *fault* (a proposal that names a fault). Until then it waits, so that the
//! symptoms of a fault that has not started yet are not mistaken for silence. Once the logical
//! clock reaches `patience` it declares the heuristic's first-ranked candidate whether or not
//! the ranking is tied, and abstains if the heuristic returned no candidate at all.
//!
//! A heuristic proposal of "no fault" is treated like any other non-fault answer: the policy
//! waits for `patience` and then declares it. That is what a symptom-free episode looks like to
//! the rule table, and it is the reason `NoFault` success must be read together with the
//! critical-miss rate on faulted classes (`docs/review-log.md`, A5).
//!
//! It uses no probe and no other component, and declares its own scheduling cost as zero.

use super::{Policy, PolicyId, zero_cost};
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{ComponentOutput, HEURISTIC_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Instant};
use gordian_world::{Action, Hypothesis};

/// The id this policy is registered under.
pub const ID: &str = "heuristic_only";

/// Default patience: 3 s of logical time. Every generated symptom is emitted within about half a
/// second of an onset that is at most a quarter of the default horizon (`gordian-world`'s
/// builder), so by 3 s the heuristic has seen all there is to see.
pub const DEFAULT_PATIENCE: Instant = Instant(3_000_000_000);

/// The policy.
#[derive(Debug, Clone)]
pub struct HeuristicOnly {
    patience: Instant,
}

impl Default for HeuristicOnly {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicOnly {
    /// The policy with [`DEFAULT_PATIENCE`].
    pub fn new() -> Self {
        Self {
            patience: DEFAULT_PATIENCE,
        }
    }

    /// The policy declaring a fallback answer once the clock reaches `patience`.
    pub fn with_patience(patience: Instant) -> Self {
        Self { patience }
    }
}

/// The first-ranked hypothesis of the first candidate list in `output`, if any.
fn top_candidate(output: &ComponentOutput) -> Option<Hypothesis> {
    output
        .entries
        .iter()
        .find_map(|(_, bytes)| match decode(bytes).ok()? {
            HypothesisEntry::Candidates { ranked, .. } => ranked.first().map(|r| r.hypothesis),
            HypothesisEntry::EvidenceDamaged { .. } => None,
        })
}

impl Policy for HeuristicOnly {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn declared_select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        vec![HEURISTIC_ID]
    }

    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        let output = outputs
            .iter()
            .find(|(id, _)| *id == HEURISTIC_ID)
            .map(|(_, o)| o);
        if let Some(Some(Some(fault))) = output.map(|o| o.proposal) {
            return Some(Action::Declare { fault: Some(fault) });
        }
        if state.now < self.patience {
            return None;
        }
        match output.and_then(top_candidate) {
            Some(hypothesis) => Some(Action::Declare { fault: hypothesis }),
            None => Some(Action::Abstain),
        }
    }
}
