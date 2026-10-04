//! The small world: a controlled digital environment with public rules and a hidden instance.
//!
//! This crate is built (A1 in `docs/local-test-plan.md`): a dependency graph of 4 to 12
//! services, five fault kinds, passive counters, messages and snapshots, paid probes, eleven
//! episode classes, a deterministic generator, a policy-facing [`step::Simulator`], and a public
//! consistency checker [`physics::consistent_hypotheses`].
//!
//! Layout: public rules and costs in [`physics`]; the hidden instance behind [`episode::Episode`]
//! and, under feature `reveal-hidden-state`, `oracle::reveal`.
//!
//! Design choices and every departure from the plan are in `DESIGN.md`.

#![forbid(unsafe_code)]

pub mod episode;
pub mod fault;
pub mod graph;
pub mod physics;
pub mod sense;
pub mod step;

mod builder;
mod timeserde;

#[cfg(any(test, feature = "reveal-hidden-state"))]
pub mod oracle;

#[cfg(test)]
mod tests;

pub use episode::{
    ComponentDirective, ComponentMode, Episode, EpisodeClass, EpisodeSpec, PublicInfo, generate,
};
pub use fault::{Fault, FaultKind, Hypothesis};
pub use graph::{ResourceKind, Service, ServiceId, World};
pub use sense::{CounterName, Observation, Probe, ProbeKind, ProbeResult, Severity};
pub use step::{Action, Outcome, Refusal, Simulator};
