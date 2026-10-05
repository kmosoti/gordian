//! The one door to hidden stream state, for the stream harness and its privileged arm.
//!
//! `gordian-stream` keeps its hidden labels (tiers, decisive-evidence labels, true hypotheses,
//! deadlines, the reasoner's accuracy draws) behind the cargo feature `reveal-hidden-state`. This
//! crate switches that feature on and offers two functions and the types they return:
//!
//! - [`truth_of`]: the [`StreamTruth`] of a generated stream. The harness builds it at the start
//!   of a stream segment and holds it aside, as the episode harness holds `gordian_eval::Truth`.
//! - [`call_records`]: every reasoner call a simulator accepted, with the hidden facts behind its
//!   answer. The harness reads them once, after the segment, and holds them aside the same way.
//!
//! **Only the stream harness (`gordian-run`'s `src/stream/harness.rs`), the privileged oracle arm
//! (`src/stream/oracle.rs`) and the stream evaluator may call anything here.** A baseline, the
//! substrate or any other policy that names this crate fails review: `scripts/check-no-oracle.sh`
//! bans the word `reveal` (and the types below) in every policy file, and allows `oracle::` only
//! in `gordian-stream`, the evaluators and this crate. Cargo unifies features across a build, so
//! the feature is a guard and not a proof; the textual check is the second guard and review the
//! third.

#![forbid(unsafe_code)]

pub use gordian_stream::oracle::{
    CallTrace, EvidenceRole, IncidentTruth, ObsLabel, ShapeTruth, StreamTruth, TruthRegime,
};
use gordian_stream::{Stream, StreamSimulator};

/// The hidden truth of `stream`.
pub fn truth_of(stream: &Stream) -> StreamTruth {
    gordian_stream::oracle::reveal(stream)
}

/// Every reasoner call `sim` accepted, in order, with the hidden facts behind each answer.
pub fn call_records(sim: &StreamSimulator) -> Vec<CallTrace> {
    gordian_stream::oracle::calls(sim)
}
