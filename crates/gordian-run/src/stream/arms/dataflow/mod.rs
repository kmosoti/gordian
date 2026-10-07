//! The incremental-dataflow noticer (work item C1, Lab 2): B3's ramp and split over the later
//! re-anchor, written as relations kept up to date by deltas, on a small general incremental
//! engine, with every operation the engine does counted and priced.
//!
//! The unit's question is not whether this noticer is better than the hand-written ones (it is
//! built to give the same answers) but what the same public rules cost, in operations and in lines,
//! when they are expressed on a general incremental engine instead of as hand-written bookkeeping
//! (B3's `noticer_*.rs`) or as a graph of cells (M2's medium). The criterion fixed for the unit is
//! reproduction and cost.
//!
//! # The engine choice, recorded before anything was built
//!
//! The dependency rule asks for the requirement, the simpler alternative considered and the cost.
//!
//! **Requirement.** Incremental maintenance of joins and windows over persistent facts: an
//! observation arrives, and only the relations it touches are brought up to date (the anomaly it
//! attaches to, the chains at its counter, the window counts of one anomaly), with every step
//! counted so that the work can be priced as the medium's is.
//!
//! **Candidates.**
//!
//! 1. *Timely / differential dataflow.* The general-purpose engines. Not used. Neither is in
//!    `Cargo.lock` or in the local registry cache (a new dependency tree, fetched and compiled in
//!    the 3-core, 3 GB build envelope); the determinism obligations would be ours to discharge
//!    (single worker, no hash-ordered output, consolidated output sorted) for code we did not
//!    write; the rules to be reproduced are order-dependent folds (the attach rule reads the
//!    anomalies as the previous observation left them, a chain tie is broken by arrival order, an
//!    anchor move resets a timestamp a rebuild would not), which differential dataflow expresses
//!    through iterative scopes or custom stateful operators, where the engine contributes nothing
//!    and exact reproduction is harder to argue; and the operations inside its operators cannot
//!    be counted from outside, so a cost "priced like the medium's" would not be available. That
//!    is a judgement from the libraries' documented model, not a measurement: nothing was built
//!    on them here.
//! 2. *The rung's existing hand-written incrementality* (the simpler alternative: B3's
//!    `RampDetector`, `attach_target`, `split_picks`, `Tracked`). It is the thing the unit is
//!    compared against, so it is not a candidate for the build; it is the oracle the tests hold
//!    this noticer to (AGENTS.md: keep a simple reference implementation as an oracle).
//! 3. *A hand-written incremental relational core* ([`engine`]): ordered tables with a delta log
//!    each, an incremental projection operator, point probes, counted range scans. **Chosen.**
//!
//! **Cost of the choice.** The engine is ours: about 250 lines, no dependency, deterministic by
//! construction (`BTreeMap` only; no hash, no thread, no clock, no randomness inside it). Its
//! generality is what its interface knows (it names no observation, anomaly or counter) and no
//! more; a measurement of it is a measurement of this engine, not of a production one, and no
//! claim is made about timely or differential dataflow. Where a general engine's strength would
//! show (joins over large, churning relations) these rules do not exercise it: the relations are
//! tens of rows.
//!
//! # What is built and what is not
//!
//! Built, in this directory:
//!
//! | File | What |
//! |---|---|
//! | `engine.rs` | the general core: [`engine::Table`] (ordered relation, counted probes, writes and scanned rows, optional delta log), [`engine::project`] (an incremental projection or filter of one table's deltas into another), the operation counts and the declared price table |
//! | `rules.rs` | the public rules as pure functions of rows: attach choice, chain continuation and eviction, the densest burst, the isolated anchor and the burst after it, the split and the instant it becomes due, the score |
//! | `program.rs` | the relations and how the rules are evaluated incrementally: facts in, derived relations kept by deltas (an index of anomalies by site, the not-yet-noticed set, a window index by time, an index by observation), the schedule of one step, timers, notices and retirements out |
//! | `noticing.rs` | the seam: [`noticing::DataflowNoticer`] implements `Noticer`, keeps the `Tracked` view the rung reads, reports its counted work for billing |
//! | `bench.rs` | the criterion micro-benchmark behind the declared prices (a `[[bench]]` of this crate whose source lives here) |
//!
//! The noticer is **B3's ramp and split over the later re-anchor over the rung's candidates**,
//! the arm `ramp_split_over_re2` of B3's table, and every combination of its optional pieces (the
//! base the rung or the re-anchor; the ramp and the split each present or absent). The rung's own
//! candidates (group an abnormal observation by the public graph, score a candidate by the z of its
//! recent abnormal count, notice on the threshold, re-anchor on the densest burst, forget the
//! stale, retire the quiet) are relations here too, because the B3 row is the rung's noticing with
//! three rules added, and an exact reproduction needs all of it. Not built: B4's follow-up rule
//! (`RampSpec::follow` must be absent), a retirement rule of its own (the rung's quiet time is
//! the noticer's, as in every B1 to B3 noticer), anything that reads a reasoner answer.
//!
//! # What it must reproduce, and how that is checked
//!
//! For every call the rung makes, the same answers as the hand-written composition, whose rules
//! this noticer re-expresses but whose code it does not call: the anomaly an observation attaches
//! to, the notices of a step in order, the retirable set, the score, and the anomalies the rung
//! reads back (anchor, site, attached observations in order, noticed instant, peak score, evidence
//! digest). `tests/stream_dataflow.rs` runs both through the rung's call sequence on generated
//! streams and compares at every call; the driver run compares the notice record per arm.
//!
//! # What is counted and what is priced
//!
//! Four kinds of operation ([`engine::OpCounts`]): a **probe** (a point read of a table), a
//! **write** (a row inserted, replaced or removed, a delta logged), a **scan** (one row visited by
//! a range scan) and a **fire** (one rule or operator invocation: a delta handled, an observation
//! fed to a rule, a pass of a step). The noticer reports the counts it did since the last report;
//! the arm bills them at [`engine::Prices::DECLARED`], which the benchmark in `bench.rs` calibrated
//! (the report of C1 gives the measured values beside the declared ones), as a component call, as
//! the medium's are. **Not counted:** the seam adapter (the `Tracked` view the rung reads, the
//! construction of the notice records) and the store the rung holds; the adapter's work is in the
//! measured wall time and not in the modelled cost. The medium's bill, for its part, does not
//! include the rung's attach rule, the score or the adapters; this noticer's bill includes the attach rule and the
//! score because they are its relations. The two bills are therefore of different scope, and the
//! report says so beside every cost.
//!
//! # Determinism
//!
//! A pure function of the public observations and the instants of its calls: no thread, no clock,
//! no randomness, no hashing; every table is a `BTreeMap`; every iteration is in key order; every
//! tie is broken by an id. Floating point is used only for the score, with the same expressions
//! as the rung's scorer so that it replays bit for bit.
//!
//! Public information only: the observations as delivered, the public rules' verdict on each, the
//! public graph. It reads no label, no incident, no tier and nothing about hidden structure.

pub mod engine;
pub mod noticing;
pub mod program;
pub mod rules;

pub use noticing::{DATAFLOW_COMPONENT, DATAFLOW_ID, DataflowNoticer};

use super::noticer::{BaseSpec, Noticer};
use super::noticer_ramp::RampSpec;
use super::noticer_split::SplitSpec;
use super::rung::RungConfig;
use gordian_world::Service;
use serde::{Deserialize, Serialize};

fn billed_default() -> bool {
    true
}

fn is_billed(billed: &bool) -> bool {
    *billed
}

/// The parameters of the dataflow noticer, written to a manifest as the fields of a noticer
/// tagged `dataflow`. The spelling is [`super::noticer::NoticerSpec::Composed`]'s with the id
/// changed and one more switch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataflowSpec {
    /// The base: the rung's own noticing, or the later re-anchor over it.
    pub base: BaseSpec,
    /// The ramp rule, if it is part of this noticer. Without a follow-up rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<RampSpec>,
    /// The split rule, if it is part of this noticer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<SplitSpec>,
    /// Whether the noticer's counted work is charged to the arm's bill. On by default; off is a
    /// labelled control (the notice record is then independent of the bill's clock).
    #[serde(default = "billed_default", skip_serializing_if = "is_billed")]
    pub billed: bool,
}

impl DataflowSpec {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        self.base.validate()?;
        if let Some(r) = &self.ramp {
            r.validate()?;
            if r.follow.is_some() {
                return Err("noticer dataflow: the follow-up rule is not expressed".to_owned());
            }
        }
        if let Some(s) = &self.split {
            s.validate()?;
        }
        Ok(())
    }
}

/// The dataflow noticer `spec` names, for the public graph `services`, under the rung's
/// parameters `cfg`.
pub fn build(spec: &DataflowSpec, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    Box::new(DataflowNoticer::new(*spec, cfg.clone(), services))
}
