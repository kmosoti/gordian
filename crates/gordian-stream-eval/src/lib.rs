//! The stream evaluator: scores a recorded stream trajectory against the hidden truth of its
//! stream (work item R2 in `docs/local-test-plan.md`).
//!
//! Built: [`score_stream`], its inputs [`StreamStep`] and [`CallSummary`], its outputs
//! [`StreamVerdict`] (per-incident [`IncidentVerdict`] and [`StreamTotals`]), its errors
//! [`StreamEvalError`], and the two helpers that read the stream's oracle, [`truth_from_stream`]
//! and [`calls_from_sim`]. The rules are numbered in `RULES.md`; hand-written cases that pin each
//! rule are in `fixtures/stream-cases.json`.
//!
//! Memory (work item E1): [`score_memory`] scores what an arm declared from memory, with the
//! observation each memory was bound at, rules K1 onward in `RULES.md`.
//!
//! Not built, and not claimed: a verdict says what a trajectory did against a stream, not
//! whether a policy is good. The evaluator never judges an outcome with a model.
//!
//! This crate depends on `gordian-core`, on `gordian-stream` with its oracle feature, and on no
//! policy or harness crate. [`score_stream`] takes a [`StreamTruth`](gordian_stream::oracle::StreamTruth)
//! rather than a `Stream`, so fixtures need not run the generator; `truth_from_stream` and
//! `calls_from_sim` are the only calls into the oracle.

#![forbid(unsafe_code)]

mod bridge;
mod error;
mod memory;
mod notice;
mod score;
mod select;
mod step;
mod timeserde;
mod verdict;

pub use bridge::{calls_from_sim, truth_from_stream};
pub use error::StreamEvalError;
pub use memory::{
    IncidentMemory, MemoryError, MemoryTotals, MemoryVerdict, RecallCells, RecallEntry,
    RecallScore, RecallSource, SourceClass, SourceTruth, score_memory,
};
pub use notice::{
    ANCHOR_WINDOW_NS, IncidentNotices, NoticeEntry, NoticeEvalError, NoticeScore, NoticeTotals,
    NoticeTrace, NoticeVerdict, RetireEntry, score_notices,
};
pub use score::score_stream;
pub use select::{
    ByClass, EscalationEntry, EscalationTotals, IncidentClass, NoticeFates, NoticeOutcome,
    SelectionError, SelectionRetire, SelectionTrace, SelectionVerdict, score_selection,
};
pub use step::{CallSummary, StreamStep};
pub use verdict::{
    EscalationCounts, IncidentVerdict, ReasonerUsage, ScoredCounts, StreamTotals, StreamVerdict,
    TierCounts,
};
