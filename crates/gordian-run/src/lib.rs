//! The run harness, recorder and driver binary, and the baseline policies (work items A4 and A6
//! in `docs/local-test-plan.md`).
//!
//! Built here:
//!
//! - [`harness::run_episode`]: the one function that plays one episode, charging a
//!   [`gordian_core::Bill`], recording a [`gordian_core::Ledger`], timing every component and
//!   scheduling call at the boundary, and scoring the trajectory with `gordian_eval`.
//! - [`policy::Policy`], a scripted test policy, and the baselines of item A6: four arms that
//!   share one decision rule and differ only in which components they select, and two
//!   privileged oracle arms. `POLICIES.md` states them.
//! - [`harness::run_episode_privileged`], the one way a policy is built from the episode's truth.
//! - [`manifest::Manifest`], [`results`] (`results.csv`, `measured.csv`) and [`recorder`]
//!   (`manifest.json`, `events-sample.jsonl`), and the `gordian-run` binary.
//! - [`interleave`] and [`drift`] (item A8): a manifest may list several arms, each episode is
//!   played once per arm in an order drawn per episode, and a fixed reference workload is timed
//!   between episodes into `drift.csv`.
//!
//! Not built here: the analysis of any run. The driver script `scripts/run-driver.sh` pins,
//! isolates and launches the binary; this crate does not.
//!
//! `HARNESS.md` next to this crate's `Cargo.toml` states the loop's rules, the decision on
//! which budget enforces which resource, and every departure from the plan. `POLICIES.md` states
//! the shared decision rule, each policy, the cost of the rule, and the privileged path.

#![forbid(unsafe_code)]

pub mod drift;
pub mod harness;
pub mod interleave;
pub mod manifest;
pub mod policy;
pub mod recorder;
pub mod results;

pub use harness::{
    EpisodeOps, EpisodeRecord, HarnessError, Limits, Measured, StopReason, run_episode,
    run_episode_privileged, standard_components,
};
pub use manifest::{ArmSpec, EpisodeParams, IsolationSpec, Manifest, RatioTolerance};
pub use policy::{Policy, PolicyId, PolicySpec};
