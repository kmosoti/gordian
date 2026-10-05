//! The stream scorer's seam: the types the harness hands a scorer, and the scorer that needs no
//! hidden truth.
//!
//! # The interface
//!
//! The stream evaluator (work item R2, `gordian-stream-eval`) is built in parallel and scores a
//! segment with one call:
//!
//! ```text
//! score_stream(truth: &StreamTruth, trajectory: &[StreamStep], calls: &[CallSummary])
//!     -> Result<StreamVerdict, StreamEvalError>
//! ```
//!
//! [`StreamScorer`] is that call as a trait. The harness builds the truth and the call records
//! at the start and end of a segment, holds them aside exactly as the episode harness holds
//! `gordian_eval::Truth`, and hands them to the scorer after the last step; no policy is ever
//! given either (the arm interface has no place for them, and `harness.rs` and `oracle.rs` are
//! the only files that may name them).
//!
//! # What is here and what R2 replaces
//!
//! The five names `StreamTruth`, `StreamStep`, `CallSummary`, `StreamVerdict` and
//! `StreamEvalError` are R2's. This file defines the ones that must exist for the harness to build
//! and test without R2: [`StreamStep`], [`StreamVerdict`] and [`StreamEvalError`] are minimal
//! definitions, and `StreamTruth` and [`CallSummary`] are the stream crate's own types, reached
//! through `gordian-stream-reveal`. **When R2 lands, replace the definitions here with R2's
//! re-exports and write one adapter in the scorer's `impl`**; `results.rs` takes its verdict
//! columns from [`StreamVerdict::HEADER`] and [`StreamVerdict::row`], so a richer verdict adds
//! columns in this one file.
//!
//! The [`CountScorer`] is the only scorer built here. It counts what the trajectory contains
//! (probes bought, reasoner calls and their cost in the calls' own units, declarations by kind)
//! and reads no truth: it says how much an arm did and what it paid, never whether it was right.
//! Whether a declaration was right, when, and for which tier is R2's.

use gordian_core::Instant;
use gordian_stream::{StreamAction, StreamOutcome};
use std::fmt;

pub use gordian_stream_reveal::CallTrace as CallSummary;
pub use gordian_stream_reveal::StreamTruth;

/// One action the stream simulator answered, in order, with the instant it was applied.
///
/// The scored trajectory is exactly the actions the simulator answered: an action the bill
/// refused is not in it (the simulator was not asked), and the ledger holds the refusal. It
/// excludes nothing an evaluator needs: probes with their results and costs, reasoner calls with
/// their full contexts, foci and costs, declarations with their anchors and diagnoses.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamStep {
    /// When the action was applied.
    pub at: Instant,
    /// What was done.
    pub action: StreamAction,
    /// What the simulator answered.
    pub outcome: StreamOutcome,
}

/// Why a trajectory could not be scored. A defect in the harness or the scorer, never a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvalError {
    /// The trajectory is not one a correct harness could have recorded.
    Malformed(String),
}

impl fmt::Display for StreamEvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamEvalError::Malformed(why) => write!(f, "malformed trajectory: {why}"),
        }
    }
}

impl std::error::Error for StreamEvalError {}

/// What a scorer reports for one segment, as the columns of `results.csv`.
///
/// This is the count-only verdict. R2's verdict is richer (per incident: whether and when a
/// correct declaration was made, critical misses, wrong declarations, decoys declared, escalations
/// by tier) and replaces it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamVerdict {
    /// Probes the simulator carried out.
    pub probes_used: u32,
    /// Declarations the simulator recorded.
    pub declarations: u32,
    /// Declarations that named a diagnosis.
    pub declared_incident: u32,
    /// Declarations that said the anchor is not an incident.
    pub declared_dismissal: u32,
    /// Reasoner calls the simulator accepted.
    pub reasoner_calls: u32,
    /// Tokens of those calls (their own unit).
    pub reasoner_tokens: u64,
    /// Declared latency of those calls, summed, nanoseconds (their own unit).
    pub reasoner_latency_ns: u64,
    /// The world's declared price of those calls, modelled nanoseconds. The manifest's exchange
    /// rate, not this, converts tokens into total cost.
    pub reasoner_declared_ns: u64,
}

impl StreamVerdict {
    /// The verdict columns of `results.csv`, in order.
    pub const HEADER: &'static str = "probes_used,declarations,declared_incident,declared_dismissal,reasoner_calls,reasoner_tokens,reasoner_latency_ns,reasoner_declared_ns";

    /// The verdict's values for the columns of [`StreamVerdict::HEADER`], comma separated.
    pub fn row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{}",
            self.probes_used,
            self.declarations,
            self.declared_incident,
            self.declared_dismissal,
            self.reasoner_calls,
            self.reasoner_tokens,
            self.reasoner_latency_ns,
            self.reasoner_declared_ns,
        )
    }
}

/// Scores one stream segment.
///
/// The harness calls it once, after the last step, with the truth it built at the start and the
/// call records it read at the end. The signature is R2's fixed interface.
pub trait StreamScorer {
    /// Score `trajectory` against `truth`, with the reasoner's `calls` as recorded on the hidden
    /// side.
    ///
    /// # Errors
    ///
    /// A [`StreamEvalError`] when the trajectory is not one a correct harness could have recorded.
    fn score_stream(
        &self,
        truth: &StreamTruth,
        trajectory: &[StreamStep],
        calls: &[CallSummary],
    ) -> Result<StreamVerdict, StreamEvalError>;
}

/// The scorer that needs no truth: it counts the trajectory.
///
/// It ignores `truth` and `calls` by design (a test shows that a different truth gives the same
/// verdict), so that the harness can be run, tested and smoke-tested before the evaluator is
/// wired in, and so that a cost-and-count comparison never depends on hidden state.
#[derive(Debug, Clone, Copy, Default)]
pub struct CountScorer;

impl StreamScorer for CountScorer {
    fn score_stream(
        &self,
        _truth: &StreamTruth,
        trajectory: &[StreamStep],
        _calls: &[CallSummary],
    ) -> Result<StreamVerdict, StreamEvalError> {
        let mut v = StreamVerdict::default();
        let mut last = Instant::ZERO;
        for step in trajectory {
            if step.at < last {
                return Err(StreamEvalError::Malformed(format!(
                    "step at {} follows step at {}",
                    step.at.0, last.0
                )));
            }
            last = step.at;
            match (&step.action, &step.outcome) {
                (StreamAction::Probe { .. }, StreamOutcome::Probed { .. }) => v.probes_used += 1,
                (StreamAction::Escalate { .. }, StreamOutcome::Escalated { cost, .. }) => {
                    v.reasoner_calls += 1;
                    v.reasoner_tokens = v.reasoner_tokens.saturating_add(cost.tokens);
                    v.reasoner_latency_ns = v.reasoner_latency_ns.saturating_add(cost.latency_ns);
                    v.reasoner_declared_ns =
                        v.reasoner_declared_ns.saturating_add(cost.modelled_ns);
                }
                (StreamAction::Declare { diagnosis, .. }, StreamOutcome::Declared { .. }) => {
                    v.declarations += 1;
                    if diagnosis.is_some() {
                        v.declared_incident += 1;
                    } else {
                        v.declared_dismissal += 1;
                    }
                }
                (_, StreamOutcome::Refused(_)) => {}
                (action, outcome) => {
                    return Err(StreamEvalError::Malformed(format!(
                        "action {action:?} was answered with {outcome:?}"
                    )));
                }
            }
        }
        Ok(v)
    }
}
