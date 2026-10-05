//! Errors: a trajectory, a call record or a truth that no correct harness could have produced.

use gordian_stream::ObsId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A recording that cannot be right. Scoring stops: there is no verdict, because a verdict would
/// hide a harness bug inside a results table.
///
/// `index` is the position in the trajectory of the first offending step. The checks run in the
/// order of `RULES.md` S37.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEvalError {
    /// An incident's `id` is not its position in `incidents` (S27).
    IncidentIdMismatch {
        /// Position in `incidents`.
        index: usize,
        /// The id found there.
        id: u32,
    },
    /// An incident's tier contradicts its truth, deadline or criticality (S28).
    TierContradictsTruth {
        /// The incident's id.
        incident: u32,
    },
    /// A label names an incident that is not in the truth (S29).
    LabelOfUnknownIncident {
        /// The observation.
        obs: ObsId,
        /// The incident id it names.
        incident: u32,
    },
    /// A step's instant is earlier than the step before it (S30).
    TimeWentBackwards {
        /// Index of the step that went back.
        index: usize,
    },
    /// A step's outcome is not one its action can produce (S31).
    OutcomeMismatch {
        /// Index of the step.
        index: usize,
    },
    /// An accepted step is after the stream's end (S32).
    ActionAfterEnd {
        /// Index of the step.
        index: usize,
    },
    /// An accepted declaration's anchor or escalation's focus is not an observation of the
    /// stream (S33).
    UnknownObservation {
        /// Index of the step.
        index: usize,
        /// The observation named.
        obs: ObsId,
    },
    /// An accepted declaration's anchor or escalation's focus belongs to an incident that had
    /// not begun at the step's instant (S34).
    ObservationNotYetEmitted {
        /// Index of the step.
        index: usize,
        /// The observation named.
        obs: ObsId,
    },
    /// An accepted escalation has no call summary, or its summary disagrees with it (S35).
    CallRecordMismatch {
        /// Index of the step.
        index: usize,
        /// The call index the outcome carried.
        call: u32,
    },
    /// The call summaries are not exactly the accepted escalations (S36).
    CallCountMismatch {
        /// Accepted escalations in the trajectory.
        escalations: usize,
        /// Call summaries given.
        calls: usize,
    },
}

impl fmt::Display for StreamEvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamEvalError::IncidentIdMismatch { index, id } => {
                write!(f, "incident at position {index} has id {id}")
            }
            StreamEvalError::TierContradictsTruth { incident } => {
                write!(f, "incident {incident}: tier contradicts truth or deadline")
            }
            StreamEvalError::LabelOfUnknownIncident { obs, incident } => {
                write!(
                    f,
                    "observation {} is labelled with unknown incident {incident}",
                    obs.0
                )
            }
            StreamEvalError::TimeWentBackwards { index } => {
                write!(f, "step {index} is earlier than the step before it")
            }
            StreamEvalError::OutcomeMismatch { index } => {
                write!(f, "step {index} has an outcome its action cannot produce")
            }
            StreamEvalError::ActionAfterEnd { index } => {
                write!(f, "step {index} was accepted after the end of the stream")
            }
            StreamEvalError::UnknownObservation { index, obs } => {
                write!(
                    f,
                    "step {index} names observation {}, which does not exist",
                    obs.0
                )
            }
            StreamEvalError::ObservationNotYetEmitted { index, obs } => {
                write!(
                    f,
                    "step {index} names observation {}, whose incident had not begun",
                    obs.0
                )
            }
            StreamEvalError::CallRecordMismatch { index, call } => {
                write!(f, "step {index} (call {call}) has no matching call summary")
            }
            StreamEvalError::CallCountMismatch { escalations, calls } => {
                write!(
                    f,
                    "{escalations} accepted escalation(s) but {calls} call summary(ies)"
                )
            }
        }
    }
}

impl std::error::Error for StreamEvalError {}
