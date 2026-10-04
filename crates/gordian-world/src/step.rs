//! The policy-facing interface: a [`Simulator`] that delivers observations and applies actions.
//!
//! A policy never holds an `Episode`. It gets passive observations from
//! [`Simulator::observe_until`] and acts through [`Simulator::apply`]. Probe results and
//! correction effects come back inside the [`Outcome`] and are observations derived from hidden
//! state, paid for from the budget.

use crate::episode::{Episode, PublicInfo};
use crate::fault::FaultKind;
use crate::graph::ServiceId;
use crate::physics::{correct_cost, probe_cost, probe_result};
use crate::sense::{Observation, Probe, ProbeKind};
use gordian_core::{Budget, Charge, Instant, Resource};
use serde::{Deserialize, Serialize};

/// What a policy can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Run a probe.
    Probe {
        /// What to measure.
        kind: ProbeKind,
        /// Where.
        target: ServiceId,
    },
    /// Attempt a correction at a service.
    Correct {
        /// Where.
        site: ServiceId,
    },
    /// Close the episode with a diagnosis. `None` declares that there is no fault.
    Declare {
        /// The diagnosis.
        fault: Option<(FaultKind, ServiceId)>,
    },
    /// Close the episode without a diagnosis.
    Abstain,
}

impl Action {
    /// The declared cost of the action. Declaring and abstaining are free.
    pub fn cost(&self) -> Vec<Charge> {
        match self {
            Action::Probe { kind, .. } => probe_cost(*kind),
            Action::Correct { .. } => correct_cost(),
            Action::Declare { .. } | Action::Abstain => Vec::new(),
        }
    }
}

/// A cost or a remainder in the two resources the world charges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CostSummary {
    /// `Resource::Probes`.
    pub probes: u64,
    /// `Resource::Time`, nanoseconds.
    pub time_ns: u64,
}

impl CostSummary {
    /// Fold a list of charges. Charges to other resources are ignored; the world charges only
    /// probes and time.
    pub fn from_charges(charges: &[Charge]) -> Self {
        let mut out = Self::default();
        for c in charges {
            if c.resource == Resource::Probes {
                out.probes += c.amount;
            } else if c.resource == Resource::Time {
                out.time_ns += c.amount;
            }
        }
        out
    }
}

/// Why an action was refused. A refusal charges nothing and, except `EpisodeOver`, leaves the
/// episode open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The episode was already closed by `Declare` or `Abstain`.
    EpisodeOver,
    /// The action names a service that is not in the world.
    UnknownService(ServiceId),
    /// The declared cost does not fit in the remaining budget.
    BudgetExceeded {
        /// The cost that was refused.
        cost: CostSummary,
    },
    /// `now` is earlier than a time already passed to this simulator.
    TimeWentBackwards,
    /// `now` is after the horizon.
    PastHorizon,
}

/// The result of an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// A probe ran. `observation` is `Observation::Probed`.
    Probed {
        /// The result, as an observation.
        observation: Observation,
        /// When the result is available: `now` plus the probe's time cost.
        #[serde(with = "crate::timeserde::instant")]
        ready_at: Instant,
        /// What the probe cost.
        cost: CostSummary,
    },
    /// A correction was attempted. `observation` is `Observation::Correction`.
    Corrected {
        /// The effect, as an observation.
        observation: Observation,
        /// When the effect is visible.
        #[serde(with = "crate::timeserde::instant")]
        ready_at: Instant,
        /// What the correction cost.
        cost: CostSummary,
    },
    /// The episode closed with a diagnosis. Correctness is not reported to the policy.
    Declared,
    /// The episode closed without a diagnosis.
    Abstained,
    /// The action was not carried out.
    Refused(Refusal),
}

/// Delivers an episode to a policy.
///
/// The simulator never reveals whether a declaration was right; the evaluator scores the
/// recorded trajectory against hidden state.
#[derive(Debug, Clone)]
pub struct Simulator {
    episode: Episode,
    budget: Budget,
    cursor: usize,
    last_now: Instant,
    over: bool,
}

impl Simulator {
    /// A simulator at the start of `episode`, with the episode's budget.
    pub fn new(episode: Episode) -> Self {
        let budget = episode.spec.budget.to_budget();
        Self {
            episode,
            budget,
            cursor: 0,
            last_now: Instant::ZERO,
            over: false,
        }
    }

    /// What a policy may know at episode start.
    pub fn public_info(&self) -> PublicInfo {
        self.episode.public_info()
    }

    /// True once `Declare` or `Abstain` has been accepted.
    pub fn is_over(&self) -> bool {
        self.over
    }

    /// Budget still available.
    pub fn remaining(&self) -> CostSummary {
        CostSummary {
            probes: self.budget.remaining(Resource::Probes).unwrap_or(0),
            time_ns: self.budget.remaining(Resource::Time).unwrap_or(0),
        }
    }

    /// Passive observations not yet delivered with instant at or before `now`, in stream order.
    /// Each observation is delivered once. Returns nothing once the episode is over.
    pub fn observe_until(&mut self, now: Instant) -> Vec<(Instant, Observation)> {
        if self.over {
            return Vec::new();
        }
        self.last_now = self.last_now.max(now);
        let stream = &self.episode.stream;
        let end = self.cursor + stream[self.cursor..].partition_point(|(at, _)| *at <= now);
        let out = stream[self.cursor..end].to_vec();
        self.cursor = end;
        out
    }

    /// Apply an action at `now`.
    pub fn apply(&mut self, action: Action, now: Instant) -> Outcome {
        if self.over {
            return Outcome::Refused(Refusal::EpisodeOver);
        }
        if now < self.last_now {
            return Outcome::Refused(Refusal::TimeWentBackwards);
        }
        if now > self.episode.spec.horizon {
            return Outcome::Refused(Refusal::PastHorizon);
        }
        let named = match action {
            Action::Probe { target, .. } => Some(target),
            Action::Correct { site } => Some(site),
            Action::Declare {
                fault: Some((_, site)),
            } => Some(site),
            Action::Declare { fault: None } | Action::Abstain => None,
        };
        if let Some(id) = named
            && id.index() >= self.episode.world.len()
        {
            return Outcome::Refused(Refusal::UnknownService(id));
        }
        let charges = action.cost();
        if self.budget.charge_all(&charges).is_err() {
            return Outcome::Refused(Refusal::BudgetExceeded {
                cost: CostSummary::from_charges(&charges),
            });
        }
        self.last_now = now;
        let cost = CostSummary::from_charges(&charges);
        let ready_at = Instant(now.0.saturating_add(cost.time_ns));
        let hidden = &self.episode.hidden;
        let truth = hidden.faults.first();
        match action {
            Action::Probe { kind, target } => {
                let probe = Probe { kind, target };
                let result = probe_result(
                    &self.episode.world.services,
                    truth.and_then(|f| f.hypothesis()),
                    hidden.bits,
                    hidden.drift_hash.unwrap_or(0),
                    probe,
                );
                Outcome::Probed {
                    observation: Observation::Probed { probe, result },
                    ready_at,
                    cost,
                }
            }
            Action::Correct { site } => Outcome::Corrected {
                observation: Observation::Correction {
                    site,
                    resolved: truth.is_some_and(|f| f.site == site),
                },
                ready_at,
                cost,
            },
            Action::Declare { .. } => {
                self.over = true;
                Outcome::Declared
            }
            Action::Abstain => {
                self.over = true;
                Outcome::Abstained
            }
        }
    }
}
