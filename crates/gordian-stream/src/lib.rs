//! The stream world: one long-running service graph with incidents, deadlines, recurrence,
//! regime changes and a simulated expensive reasoner (work item R1 in `docs/local-test-plan.md`).
//!
//! # What is built
//!
//! - [`generate`]: a pure function of [`StreamParams`] that builds a [`Stream`]: a service graph,
//!   background noise, and incidents arriving as a Poisson process. Every incident has a hidden
//!   [`Tier`]: **plain** (obeys the first world's public physics), **hard** (obeys rules the cheap
//!   rung does not have) or **decoy** (imitates a hard incident for its first seconds and then
//!   resolves by itself). Incidents have hidden deadlines; some repeat an earlier incident; at
//!   scheduled instants part of the public physics changes without announcement.
//! - [`StreamSimulator`]: the policy-facing interface, `observe_until(now)` and
//!   `apply(action, now)`, with the actions `Probe`, `Escalate` and `Declare`.
//! - The simulated reasoner (its law is stated in `reasoner.rs` and in `DESIGN.md`): informed with
//!   a probability that is zero without decisive evidence in the context, otherwise a guess from
//!   the context and the public rules; paid for before it answers, answering after a declared
//!   latency.
//! - `oracle` (feature `reveal-hidden-state`): the only way to read hidden state.
//!
//! # What is not built
//!
//! The stream evaluator (R2), the stream harness and baselines (R3), and the headroom check (R4).
//! Nothing here claims that a policy can or cannot do well; `DESIGN.md` states what each part of
//! the construction guarantees and how each guarantee is tested.
//!
//! Design choices and every way a policy might infer hidden state are in `DESIGN.md`.

#![forbid(unsafe_code)]

pub mod kinds;
pub mod params;
pub mod sim;
pub mod stream;

mod incident;
mod labels;
mod noise;
mod present;
mod probe;
mod reasoner;
mod regime;
mod rng;
mod timeserde;

#[cfg(any(test, feature = "reveal-hidden-state"))]
pub mod oracle;

#[cfg(test)]
mod tests;

pub use kinds::{Diagnosis, HardKind, ObsId, ObsRef, Question, StreamHypothesis, StreamKind, Tier};
pub use params::{
    CriticalSpec, DeadlineSpec, DifficultySpec, NoiseSpec, ReasonerCost, ReasonerCostSpec,
    ReasonerSpec, RegimeKind, RegimeSchedule, StreamBudgetSpec, StreamParams, StreamPublic,
    TierMix, UnitRange, Window, timing,
};
pub use sim::{
    Answer, StreamAction, StreamEvent, StreamOutcome, StreamRefusal, StreamRemaining,
    StreamSimulator,
};
pub use stream::{Stream, generate};
