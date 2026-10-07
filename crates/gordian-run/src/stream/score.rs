//! The stream harness's use of the stream evaluator (work items R2 and R3b).
//!
//! # What the harness does with it
//!
//! A segment is scored once, after the last step, by
//! `gordian_stream_eval::score_stream`, called with `(&truth, &trajectory, &calls)`:
//!
//! - `truth` is built by `truth_from_stream` immediately after the stream is generated;
//! - `trajectory` is the list of [`StreamStep`]s the harness recorded (every action the stream
//!   answered, with the instant it was applied);
//! - `calls` is built by `calls_from_sim` from the simulator after the last step.
//!
//! The harness holds the truth and the call records aside exactly as the episode harness holds
//! `gordian_eval::Truth`: they are locals of `harness.rs`'s `play`, handed to the scorer and to
//! nothing else, and no arm interface has a place for either. **This file names neither**: the
//! harness reads the truth by inference (it binds what `truth_from_stream` returns and never
//! writes its type), so no file of this crate writes the truth's type or the oracle's path, and
//! the only allowlisted places in the repository that do are `gordian-stream` and
//! `gordian-stream-eval` (`scripts/check-no-oracle.sh`).
//!
//! An `Err` from the evaluator is a defect in the harness (a trajectory no correct harness could
//! have recorded, or call records from another simulator). It becomes
//! [`StreamHarnessError::Eval`](super::harness::StreamHarnessError::Eval), the run stops and no
//! row is written; it is never a result.
//!
//! # What is here
//!
//! The evaluator's types, re-exported under the names the harness uses; the one count the
//! evaluator does not make, [`TrajectoryCounts`] (probes bought and the reasoner's declared
//! latency, read from the trajectory alone, no truth); and [`family_name`], the label of a hard
//! incident's family for `incidents.csv`.
//!
//! The evaluator's verdict says what a trajectory did against a stream. It does not say whether an
//! arm is good: `RULES.md` of the evaluator lists the judgements (a late correct declaration, a
//! wrong declaration beside a correct one, a late dismissal) that an experiment's preregistration
//! must settle.

use gordian_stream::{HardKind, StreamAction, StreamOutcome};

pub use gordian_stream_eval::{
    ByClass, CallSummary, EscalationCounts, EscalationEntry, EscalationTotals, IncidentClass,
    IncidentMemory, IncidentNotices, IncidentVerdict, MemoryError, MemoryTotals, MemoryVerdict,
    NoticeEntry, NoticeEvalError, NoticeFates, NoticeOutcome, NoticeScore, NoticeTotals,
    NoticeTrace, NoticeVerdict, ReasonerUsage, RecallCells, RecallEntry, RecallScore,
    RecallSource, RetireEntry, ScoredCounts, SelectionError, SelectionRetire, SelectionTrace,
    SelectionVerdict, SourceClass, StreamEvalError, StreamStep, StreamTotals, StreamVerdict,
    TierCounts,
};

/// Counts the evaluator does not make, read from the trajectory alone.
///
/// None of them needs the truth. `declarations` and `reasoner_calls`-like totals that the evaluator
/// also makes are cross-checked against its own in the harness's tests, so two independent
/// readings of the same trajectory must agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrajectoryCounts {
    /// Probes the stream carried out.
    pub probes_used: u32,
    /// Declarations the stream recorded.
    pub declarations: u32,
    /// Declarations that named a diagnosis.
    pub declared_incident: u32,
    /// Declarations that said the anchor is not an incident.
    pub declared_dismissal: u32,
    /// Reasoner calls the stream accepted.
    pub reasoner_calls: u32,
    /// Declared latency of those calls, summed, nanoseconds (the reasoner's own unit).
    pub reasoner_latency_ns: u64,
}

impl TrajectoryCounts {
    /// Count `trajectory`. Refused actions are not counted; an action answered with the wrong kind
    /// of outcome is the evaluator's error (S31), not this function's, so it is ignored here.
    pub fn of(trajectory: &[StreamStep]) -> Self {
        let mut c = Self::default();
        for step in trajectory {
            match (&step.action, &step.outcome) {
                (StreamAction::Probe { .. }, StreamOutcome::Probed { .. }) => c.probes_used += 1,
                (StreamAction::Escalate { .. }, StreamOutcome::Escalated { cost, .. }) => {
                    c.reasoner_calls += 1;
                    c.reasoner_latency_ns = c.reasoner_latency_ns.saturating_add(cost.latency_ns);
                }
                (StreamAction::Declare { diagnosis, .. }, StreamOutcome::Declared { .. }) => {
                    c.declarations += 1;
                    if diagnosis.is_some() {
                        c.declared_incident += 1;
                    } else {
                        c.declared_dismissal += 1;
                    }
                }
                _ => {}
            }
        }
        c
    }
}

/// The label a hard incident's family has in `incidents.csv`: the name of its hard kind. Hidden
/// state, written only into evaluator output (`HARNESS.md`, section 11).
pub fn family_name(kind: HardKind) -> &'static str {
    match kind {
        HardKind::Compound => "compound",
        HardKind::Cascade => "cascade",
        HardKind::SplitBrain => "split_brain",
        HardKind::SlowLeak => "slow_leak",
    }
}
