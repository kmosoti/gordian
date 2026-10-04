//! `fixed_pipeline`: a configured ordered subset of components, run every `k` steps.
//!
//! The charter's "tuned fixed pipeline" baseline. The configuration is in the manifest:
//!
//! ```json
//! {"policy": "fixed_pipeline", "components": ["heuristic", "estimator", "verifier"], "every": 2}
//! ```
//!
//! The defaults are all four components in id order and `every = 1`, which with the shared
//! decision rule is the same arm as `all_components` (the components read the working state and
//! not each other's output, so their order inside a step changes nothing but which one a bill
//! refusal would drop first). Tuning the order, the subset and the period is exploration run B1's
//! job, on exploration data; nothing here is tuned.
//!
//! The pipeline runs at its first step and then at every `k`-th step, counted in calls to
//! `select`. The harness calls `select` once per step except when it refuses the scheduling
//! charge, so the count is the step count unless the arm has run out of compute.

use super::{PolicyId, Selector, zero_cost};
use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId};
use std::collections::BTreeSet;

/// The id this policy is registered under.
pub const ID: &str = "fixed_pipeline";

/// The names a manifest uses for the four components, in id order.
pub const NAMES: [(&str, ComponentId); 4] = [
    ("heuristic", HEURISTIC_ID),
    ("estimator", ESTIMATOR_ID),
    ("memory", MEMORY_ID),
    ("verifier", VERIFIER_ID),
];

/// The id of the component a manifest calls `name`.
pub fn component_by_name(name: &str) -> Option<ComponentId> {
    NAMES.iter().find(|(n, _)| *n == name).map(|(_, id)| *id)
}

/// The manifest name of component `id`.
pub fn component_name(id: ComponentId) -> Option<&'static str> {
    NAMES.iter().find(|(_, i)| *i == id).map(|(n, _)| *n)
}

/// Component ids from manifest names, in the order given.
pub fn parse_components(names: &[String]) -> Result<Vec<ComponentId>, String> {
    names
        .iter()
        .map(|name| {
            component_by_name(name).ok_or_else(|| {
                format!(
                    "unknown component {name:?}; known: {:?}",
                    NAMES.map(|(n, _)| n)
                )
            })
        })
        .collect()
}

/// Manifest names of `components`. A component that has no name is written as its number.
pub fn component_names(components: &[ComponentId]) -> Vec<String> {
    components
        .iter()
        .map(|id| component_name(*id).map_or_else(|| id.0.to_string(), str::to_owned))
        .collect()
}

/// The configuration: which components, in which order, and how often.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The components to run, in order. Not empty, no repeats.
    pub components: Vec<ComponentId>,
    /// Run at the first step and then at every `every`-th. At least 1.
    pub every: u32,
}

impl Default for Config {
    /// All four components in id order, every step.
    fn default() -> Self {
        Self {
            components: NAMES.iter().map(|(_, id)| *id).collect(),
            every: 1,
        }
    }
}

impl Config {
    /// Check the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.components.is_empty() {
            return Err("fixed_pipeline needs at least one component".to_owned());
        }
        let unique: BTreeSet<ComponentId> = self.components.iter().copied().collect();
        if unique.len() != self.components.len() {
            return Err("fixed_pipeline lists a component twice".to_owned());
        }
        if self.every == 0 {
            return Err("fixed_pipeline needs every >= 1".to_owned());
        }
        Ok(())
    }
}

/// The selector.
#[derive(Debug, Clone)]
pub struct Pipeline {
    config: Config,
    calls: u64,
}

impl Pipeline {
    /// A pipeline over `config`.
    pub fn new(config: Config) -> Self {
        Self { config, calls: 0 }
    }
}

impl Selector for Pipeline {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        let due = self.calls % u64::from(self.config.every) == 0;
        self.calls += 1;
        if due {
            self.config.components.clone()
        } else {
            Vec::new()
        }
    }
}
