//! A policy driven by a fixed list, for testing the harness.
//!
//! It looks at nothing: step `k` of the episode selects `script[k].select` and decides
//! `script[k].action`. Once the script is exhausted it selects nothing and waits forever, which
//! is how the tests build a policy that never decides. At the harness's final call it does what
//! [`ScriptedPolicy::with_final`] said, and by default nothing, so a script that never decides
//! stays undecided.
//!
//! It is a test instrument, not a baseline, and is not registered in [`super::build`].

use super::{Policy, PolicyId, zero_cost};
use gordian_components::{ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId};
use gordian_world::Action;

/// One step of a script.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptedStep {
    /// Components to select at this step.
    pub select: Vec<ComponentId>,
    /// The action to decide at this step; `None` waits.
    pub action: Option<Action>,
}

/// A policy that replays a fixed script.
#[derive(Debug, Clone)]
pub struct ScriptedPolicy {
    id: PolicyId,
    script: Vec<ScriptedStep>,
    select_cost: Vec<Charge>,
    final_action: Option<Action>,
    final_cost: Vec<Charge>,
    step: usize,
}

impl ScriptedPolicy {
    /// A policy that plays `script`, declaring a zero scheduling cost.
    pub fn new(script: Vec<ScriptedStep>) -> Self {
        Self {
            id: PolicyId::new("scripted"),
            script,
            select_cost: zero_cost(),
            final_action: None,
            final_cost: zero_cost(),
            step: 0,
        }
    }

    /// A policy whose step `k` takes `actions[k]` and selects no component.
    pub fn actions(actions: Vec<Action>) -> Self {
        Self::new(
            actions
                .into_iter()
                .map(|action| ScriptedStep {
                    select: Vec::new(),
                    action: Some(action),
                })
                .collect(),
        )
    }

    /// A policy that never selects, acts or decides.
    pub fn never_decides() -> Self {
        Self::new(Vec::new())
    }

    /// The same policy declaring `cost` as its scheduling cost at every step.
    pub fn with_select_cost(mut self, cost: Vec<Charge>) -> Self {
        self.select_cost = cost;
        self
    }

    /// The same policy taking `action` at the harness's final call.
    pub fn with_final(mut self, action: Action) -> Self {
        self.final_action = Some(action);
        self
    }

    /// The same policy declaring `cost` for the final call.
    pub fn with_final_cost(mut self, cost: Vec<Charge>) -> Self {
        self.final_cost = cost;
        self
    }

    fn current(&self) -> Option<&ScriptedStep> {
        self.script.get(self.step)
    }
}

impl Policy for ScriptedPolicy {
    fn id(&self) -> PolicyId {
        self.id.clone()
    }

    fn declared_select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        self.select_cost.clone()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        self.current().map(|s| s.select.clone()).unwrap_or_default()
    }

    fn decide(
        &mut self,
        _state: &WorkingState,
        _outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        let action = self.current().and_then(|s| s.action);
        self.step += 1;
        action
    }

    fn declared_final_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        self.final_cost.clone()
    }

    fn decide_final(&mut self, _state: &WorkingState) -> Option<Action> {
        self.final_action
    }
}
