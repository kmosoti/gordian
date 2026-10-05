//! What the harness records: the trajectory's steps and the reasoner's call summaries.

use crate::timeserde;
use gordian_core::Instant;
use gordian_stream::{ObsId, StreamAction, StreamOutcome};
use serde::{Deserialize, Serialize};

/// One recorded step: when the policy acted, what it tried, and what the stream answered.
///
/// The stream's own types, unchanged. The outcome says what happened; the action says what was
/// tried. Reasoner answers are not steps: they arrive through `observe_until`, and the scorer
/// never reads them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamStep {
    /// The instant passed to `StreamSimulator::apply`.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
    /// What the policy tried.
    pub action: StreamAction,
    /// What `StreamSimulator::apply` returned.
    pub outcome: StreamOutcome,
}

/// The hidden side of one accepted reasoner call, in the order the calls were accepted.
///
/// `calls[n]` describes the escalation whose outcome carried `call == n`. Everything the scorer
/// needs from the simulator's private record is here: whether the call was informed and whether
/// its answer was right. The rest (when, how many references, which observation) repeats the
/// trajectory and is cross-checked against it (`RULES.md`, S35), so a harness that pairs the
/// wrong simulator with a trajectory fails loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSummary {
    /// When the call was made.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
    /// When its answer was delivered.
    #[serde(with = "timeserde::instant")]
    pub ready_at: Instant,
    /// The observation the question was about, when the source knows it. [`crate::calls_from_sim`]
    /// fills it from the stream's `CallTrace`; a source that does not know it leaves it `None`,
    /// and the trajectory's own `Escalate` step is then the only source. When it is present it
    /// must equal the trajectory's (S35).
    pub focus: Option<ObsId>,
    /// References in the context.
    pub refs: u32,
    /// The call was informed: it answered from decisive evidence in its context, not by guessing.
    pub informed: bool,
    /// The answer equalled the truth (an informed call, or a lucky guess).
    pub correct: bool,
}
