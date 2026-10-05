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
//! for the privileged `oracle.rs`.
//!
//! # One decision rule, many selectors
//!
//! Every non-privileged policy is an [`Arm`]: a [`Selector`], which says which components run
//! each step, and the shared [`decide::Decider`], which says what to do with their outputs. A
//! selector has no access to `decide`, and `Arm` is the only non-privileged `Policy`
//! implementation, so two arms cannot differ in their decision rule. `POLICIES.md` states the
//! rule and the policies.
//!
//! # Costs
//!
//! A policy declares its own scheduling cost through [`Policy::declared_select_cost`]; the
//! harness charges it under `Phase::Scheduling` once per step, before `select`. An [`Arm`]
//! declares its selector's cost plus the shared rule's cost, so every arm pays for the same
//! rule. A policy that declares nothing must say so with [`zero_cost`], so that a zero is a
//! recorded statement rather than an omission. The harness also *times* `select` and `decide`
//! and records the timing in the ledger, but a policy never sees a timing, so a timing cannot
//! influence a decision.
//!
//! # The privileged arms
//!
//! `oracle_immediate` and `oracle_evidence` need the episode's truth. The [`Policy`] trait does
//! not carry it. They are built by [`privileged::OracleFactory`], which the harness calls with
//! the truth through `run_episode_privileged`; see that module.

pub mod all_components;
pub mod decide;
pub mod fixed_pipeline;
pub mod heuristic_only;
#[path = "oracle.rs"]
pub mod privileged;
pub mod random_matched;
pub mod scripted;

use decide::{DecideConfig, Decider, Remaining};
use gordian_components::{ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Resource};
use gordian_world::Action;
use serde::{Deserialize, Serialize};

/// The identity of a policy, as written in the ledger and in results.
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

    /// The name of the rule this policy uses to turn component outputs into actions. Every
    /// non-privileged arm reports [`decide::RULE`]; a test asserts it. The default says the
    /// policy makes no claim.
    fn decision_rule(&self) -> &'static str {
        "unspecified"
    }

    /// What one step of this policy's own selection and decision work is declared to cost.
    ///
    /// Charged under `Phase::Scheduling` once per step, before [`Policy::select`]. A function of
    /// `state` and the policy's own fields only, with no side effects: the harness may call it
    /// more than once per step (it also asks whether any work is still affordable). Return
    /// [`zero_cost`] to declare a free policy.
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

    /// What the final call is declared to cost.
    ///
    /// The harness makes one final call when it has found that no affordable work is left or that
    /// the horizon was reached (`HARNESS.md`, section 1). It charges this under
    /// `Phase::Scheduling` before [`Policy::decide_final`], and calls `decide_final` whether or
    /// not the charge is accepted, because declaring is free. It is the cost of the decision work
    /// alone: no selection happens, so a selector's cost is not part of it. A function of `state`
    /// and the policy's own fields, with no side effects. [`zero_cost`] declares a free call.
    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge>;

    /// The final call: the policy's last chance to act, with no component output because none ran.
    ///
    /// A policy should declare or abstain here, as it would at its own deadline. Only `Declare`
    /// and `Abstain` are carried out; any other action is recorded in the ledger and ignored,
    /// since nothing could follow it. `None` leaves the episode undecided.
    fn decide_final(&mut self, state: &WorkingState) -> Option<Action>;
}

/// The explicit declaration that a policy's selection costs nothing: one `Compute` charge of
/// zero. It is recorded in the ledger at every step.
pub fn zero_cost() -> Vec<Charge> {
    vec![Charge::new(Resource::Compute, 0)]
}

/// The part of a non-privileged policy that is allowed to differ between arms: which components
/// run at a step.
pub trait Selector {
    /// Stable identity of the arm.
    fn id(&self) -> PolicyId;

    /// What the selection itself is declared to cost at `state`, not counting the shared
    /// decision rule, which [`Arm`] adds. [`zero_cost`] for a selection that costs nothing worth
    /// declaring.
    fn select_cost(&self, state: &WorkingState) -> Vec<Charge>;

    /// The components to run this step, in order.
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId>;
}

/// A selector together with the shared decision rule. The only non-privileged policy.
#[derive(Debug, Clone)]
pub struct Arm<S: Selector> {
    selector: S,
    decider: Decider,
}

impl<S: Selector> Arm<S> {
    /// `selector` with the shared rule under `config`.
    pub fn with(selector: S, config: DecideConfig) -> Self {
        Self {
            selector,
            decider: Decider::new(config),
        }
    }

    /// The selector.
    pub fn selector(&self) -> &S {
        &self.selector
    }
}

impl<S: Selector> Policy for Arm<S> {
    fn id(&self) -> PolicyId {
        self.selector.id()
    }

    fn decision_rule(&self) -> &'static str {
        decide::RULE
    }

    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        let rule = self.decider.declared_cost(state);
        let mut compute = rule.amount;
        let mut charges = Vec::new();
        for charge in self.selector.select_cost(state) {
            if charge.resource == Resource::Compute {
                compute = compute.saturating_add(charge.amount);
            } else {
                charges.push(charge);
            }
        }
        charges.insert(0, Charge::new(Resource::Compute, compute));
        charges
    }

    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.decider.note_remaining(Remaining::of(bill));
        self.selector.select(state, bill)
    }

    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        self.decider.decide(state, outputs)
    }

    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge> {
        vec![self.decider.declared_final_cost(state)]
    }

    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        self.decider.decide_final(state)
    }
}

/// A policy and its configuration, as the manifest writes it.
///
/// In JSON a policy with nothing to configure is its id, `"heuristic_only"`, which is also what
/// every manifest written before the baselines existed holds. A policy with parameters is an
/// object with a `policy` field and its parameters, for example
/// `{"policy": "random_matched", "p": 0.3}`. The decision rule's own parameter is not here: it
/// is the same for every arm, so it is the manifest's `decide`.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicySpec {
    /// `heuristic_only`.
    HeuristicOnly,
    /// `fixed_pipeline` with its components and period.
    FixedPipeline(fixed_pipeline::Config),
    /// `all_components`.
    AllComponents,
    /// `random_matched` with its selection probability.
    RandomMatched(random_matched::Config),
    /// `oracle_immediate`: privileged.
    OracleImmediate,
    /// `oracle_evidence`: privileged.
    OracleEvidence,
}

/// Ids of the policies [`build`] knows.
pub const KNOWN: &[&str] = &[
    heuristic_only::ID,
    fixed_pipeline::ID,
    all_components::ID,
    random_matched::ID,
    privileged::ID_IMMEDIATE,
    privileged::ID_EVIDENCE,
];

impl PolicySpec {
    /// The policy with `id` and its default configuration, or why there is none.
    pub fn from_id(id: &str) -> Result<PolicySpec, String> {
        Self::from_parts(id, None, None, None)
    }

    /// The policy `id` with the given parameters. A parameter the policy does not have is an
    /// error, not ignored.
    pub fn from_parts(
        id: &str,
        components: Option<Vec<String>>,
        every: Option<u32>,
        p: Option<f64>,
    ) -> Result<PolicySpec, String> {
        let stray = |name: &str| format!("policy {id:?} has no parameter {name:?}");
        let spec = match id {
            heuristic_only::ID
            | all_components::ID
            | privileged::ID_IMMEDIATE
            | privileged::ID_EVIDENCE => {
                if components.is_some() {
                    return Err(stray("components"));
                }
                if every.is_some() {
                    return Err(stray("every"));
                }
                if p.is_some() {
                    return Err(stray("p"));
                }
                match id {
                    heuristic_only::ID => PolicySpec::HeuristicOnly,
                    all_components::ID => PolicySpec::AllComponents,
                    privileged::ID_IMMEDIATE => PolicySpec::OracleImmediate,
                    _ => PolicySpec::OracleEvidence,
                }
            }
            fixed_pipeline::ID => {
                if p.is_some() {
                    return Err(stray("p"));
                }
                let default = fixed_pipeline::Config::default();
                let components = match components {
                    Some(names) => fixed_pipeline::parse_components(&names)?,
                    None => default.components,
                };
                PolicySpec::FixedPipeline(fixed_pipeline::Config {
                    components,
                    every: every.unwrap_or(default.every),
                })
            }
            random_matched::ID => {
                if components.is_some() {
                    return Err(stray("components"));
                }
                if every.is_some() {
                    return Err(stray("every"));
                }
                PolicySpec::RandomMatched(random_matched::Config {
                    p: p.unwrap_or(random_matched::Config::default().p),
                })
            }
            other => {
                return Err(format!("unknown policy {other:?}; known: {KNOWN:?}"));
            }
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            PolicySpec::FixedPipeline(c) => c.validate(),
            PolicySpec::RandomMatched(c) => c.validate(),
            _ => Ok(()),
        }
    }

    /// The policy's id.
    pub fn id(&self) -> PolicyId {
        PolicyId::new(self.id_str())
    }

    fn id_str(&self) -> &'static str {
        match self {
            PolicySpec::HeuristicOnly => heuristic_only::ID,
            PolicySpec::FixedPipeline(_) => fixed_pipeline::ID,
            PolicySpec::AllComponents => all_components::ID,
            PolicySpec::RandomMatched(_) => random_matched::ID,
            PolicySpec::OracleImmediate => privileged::ID_IMMEDIATE,
            PolicySpec::OracleEvidence => privileged::ID_EVIDENCE,
        }
    }

    /// Whether the policy is built from the episode's truth. Its arm name must say so.
    pub fn is_privileged(&self) -> bool {
        matches!(
            self,
            PolicySpec::OracleImmediate | PolicySpec::OracleEvidence
        )
    }
}

/// The JSON shape of a policy with parameters. Parameters a policy does not have are absent.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tagged {
    policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    components: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    every: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    p: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Repr {
    Id(String),
    Full(Tagged),
}

impl Serialize for PolicySpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            PolicySpec::FixedPipeline(c) => Tagged {
                policy: fixed_pipeline::ID.to_owned(),
                components: Some(fixed_pipeline::component_names(&c.components)),
                every: Some(c.every),
                p: None,
            }
            .serialize(serializer),
            PolicySpec::RandomMatched(c) => Tagged {
                policy: random_matched::ID.to_owned(),
                components: None,
                every: None,
                p: Some(c.p),
            }
            .serialize(serializer),
            other => serializer.serialize_str(other.id_str()),
        }
    }
}

impl<'de> Deserialize<'de> for PolicySpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let result = match Repr::deserialize(deserializer)? {
            Repr::Id(id) => PolicySpec::from_id(&id),
            Repr::Full(t) => PolicySpec::from_parts(&t.policy, t.components, t.every, t.p),
        };
        result.map_err(serde::de::Error::custom)
    }
}

/// What [`build`] makes for one episode.
pub enum Built {
    /// A policy that is never shown the truth.
    Public(Box<dyn Policy>),
    /// A privileged arm, built from the truth by the harness
    /// ([`crate::harness::run_episode_privileged`]).
    Privileged(privileged::OracleFactory),
}

/// A fresh policy for one episode of the arm named `arm`, whose episode seed is `episode_seed`.
///
/// `decide` is the shared rule's parameters, the same for every arm of a run. The seed is used
/// only to derive the random arm's generator, together with the arm name; nothing else sees it.
pub fn build(spec: &PolicySpec, decide: &DecideConfig, arm: &str, episode_seed: u64) -> Built {
    let public = |policy: Box<dyn Policy>| Built::Public(policy);
    match spec {
        PolicySpec::HeuristicOnly => public(Box::new(Arm::with(
            heuristic_only::Heuristic,
            decide.to_owned(),
        ))),
        PolicySpec::FixedPipeline(config) => public(Box::new(Arm::with(
            fixed_pipeline::Pipeline::new(config.clone()),
            decide.to_owned(),
        ))),
        PolicySpec::AllComponents => public(Box::new(Arm::with(
            all_components::AllComponents,
            decide.to_owned(),
        ))),
        PolicySpec::RandomMatched(config) => public(Box::new(Arm::with(
            random_matched::RandomSubset::new(
                config.p,
                random_matched::rng_seed(episode_seed, arm),
            ),
            decide.to_owned(),
        ))),
        PolicySpec::OracleImmediate => Built::Privileged(privileged::OracleFactory::new(
            privileged::Variant::Immediate,
            *decide,
        )),
        PolicySpec::OracleEvidence => Built::Privileged(privileged::OracleFactory::new(
            privileged::Variant::Evidence,
            *decide,
        )),
    }
}
