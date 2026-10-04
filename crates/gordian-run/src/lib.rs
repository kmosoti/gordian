//! The run harness, recorder and driver binary (work item A4 in `docs/local-test-plan.md`).
//!
//! Built here:
//!
//! - [`harness::run_episode`]: the one function that plays one episode, charging a
//!   [`gordian_core::Bill`], recording a [`gordian_core::Ledger`], timing every component and
//!   scheduling call at the boundary, and scoring the trajectory with `gordian_eval`.
//! - [`policy::Policy`], a scripted test policy and one trivial real policy (`heuristic_only`).
//!   The other baselines are item A6.
//! - [`manifest::Manifest`], [`results`] (`results.csv`, `measured.csv`) and [`recorder`]
//!   (`manifest.json`, `events-sample.jsonl`), and the `gordian-run` binary.
//!
//! Not built here: the baselines of A6, the oracle policy, the analysis of any run. The driver
//! script `scripts/run-driver.sh` pins, isolates and launches the binary; this crate does not.
//!
//! `HARNESS.md` next to this crate's `Cargo.toml` states the loop's rules, the decision on
//! which budget enforces which resource, and every departure from the plan.

#![forbid(unsafe_code)]

pub mod harness;
pub mod manifest;
pub mod policy;
pub mod recorder;
pub mod results;

pub use harness::{
    EpisodeRecord, HarnessError, Limits, Measured, StopReason, run_episode, run_episode_privileged,
    standard_components,
};
pub use manifest::{EpisodeParams, IsolationSpec, Manifest, RatioTolerance};
pub use policy::{Policy, PolicyId, PolicySpec};
