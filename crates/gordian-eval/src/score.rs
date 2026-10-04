//! Scoring a recorded trajectory against hidden truth.
//!
//! The rules are in `RULES.md` next to this crate's `Cargo.toml`, one numbered row each; the
//! comments below cite them as R1, R2, and so on. [`score`] is a pure function of its arguments:
//! it reads no clock, draws no randomness, and keeps no state between calls.
//!
//! # How a trajectory is read
//!
//! A [`Step`] is one action the arm tried and what the world answered. The world's own types are
//! used unchanged: [`Action`] and [`Outcome`]. What happened is read from the *outcome*: a step
//! counts as a probe only if the world carried the probe out (`Outcome::Probed`), and as a
//! terminal action only if the world accepted the close (`Outcome::Declared` or
//! `Outcome::Abstained`). A refused step changed nothing and counts toward nothing (R12).
//!
//! The scorer does not run the world. It does not check that probe results are what the world
//! would have returned, or that a refusal was justified; that is the world's tests' job. It does
//! check that the trajectory is internally coherent (R14 to R17), because an incoherent
//! trajectory means the harness recorded something the world could not have produced.

use crate::timeserde;
use crate::truth::Truth;
use gordian_core::Instant;
use gordian_world::{Action, Outcome, Refusal};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One recorded step: when the arm acted, what it tried, and what the world answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The instant passed to `Simulator::apply`.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
    /// What the arm tried.
    pub action: Action,
    /// What `Simulator::apply` returned.
    pub outcome: Outcome,
}

/// The score of one episode. Every field is defined by a row of `RULES.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// The first terminal action was correct (R2, R5).
    pub success: bool,
    /// A critical fault existed and the arm did not succeed (R7).
    pub critical_miss: bool,
    /// The episode had no fault and the arm declared one or carried out a correction (R6).
    pub false_alarm: bool,
    /// The terminal action was `Abstain` (R8). Coverage is computed from this downstream.
    pub abstained: bool,
    /// There was no terminal action (R9).
    pub undecided: bool,
    /// Probes the world carried out, counted as actions, not as resource units (R10).
    pub probes_used: u32,
    /// Corrections the world carried out (R11).
    pub corrections: u32,
    /// When the terminal action was taken, if there was one (R13).
    #[serde(with = "timeserde::option_instant")]
    pub decision_at: Option<Instant>,
}

/// A trajectory or truth that no correct harness could have produced. Scoring stops: there is no
/// verdict, because a verdict would hide a harness bug inside a result table.
///
/// `index` is the position in the trajectory of the first offending step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalError {
    /// A step follows the first terminal action, refused or not (R14).
    ActionAfterTerminal {
        /// Index of the first step after the terminal action.
        index: usize,
    },
    /// A step's instant is earlier than the step before it (R15).
    TimeWentBackwards {
        /// Index of the step that went back.
        index: usize,
    },
    /// A step's outcome is not one its action can produce (R16).
    OutcomeMismatch {
        /// Index of the step.
        index: usize,
    },
    /// A step was refused with `EpisodeOver` although no terminal action preceded it (R17).
    EpisodeOverWithoutTerminal {
        /// Index of the step.
        index: usize,
    },
    /// The truth is inconsistent with its class (R18).
    InvalidTruth {
        /// The class given.
        class: gordian_world::EpisodeClass,
        /// How many faults were given.
        fault_count: usize,
    },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::ActionAfterTerminal { index } => {
                write!(f, "step {index} follows the terminal action")
            }
            EvalError::TimeWentBackwards { index } => {
                write!(f, "step {index} is earlier than the step before it")
            }
            EvalError::OutcomeMismatch { index } => {
                write!(f, "step {index} has an outcome its action cannot produce")
            }
            EvalError::EpisodeOverWithoutTerminal { index } => {
                write!(
                    f,
                    "step {index} was refused as episode-over but nothing closed the episode"
                )
            }
            EvalError::InvalidTruth { class, fault_count } => {
                write!(
                    f,
                    "truth has {fault_count} fault(s), inconsistent with class {class:?}"
                )
            }
        }
    }
}

impl std::error::Error for EvalError {}

/// Score `trajectory` against `truth`.
///
/// Steps are read in order. For each step the first failing check decides the error: after a
/// terminal action (R14), then time going backwards (R15), then the outcome check (R16, R17).
/// The truth is checked before any step (R18). Equal timestamps are allowed.
pub fn score(truth: &Truth, trajectory: &[Step]) -> Result<Verdict, EvalError> {
    if !truth.is_consistent() {
        return Err(EvalError::InvalidTruth {
            class: truth.class,
            fault_count: truth.faults.len(),
        });
    }

    let mut probes_used: u32 = 0;
    let mut corrections: u32 = 0;
    // The first terminal action and when it was accepted (R1).
    let mut terminal: Option<(Instant, Action)> = None;
    let mut previous: Option<Instant> = None;

    for (index, step) in trajectory.iter().enumerate() {
        if terminal.is_some() {
            return Err(EvalError::ActionAfterTerminal { index });
        }
        if previous.is_some_and(|p| step.at < p) {
            return Err(EvalError::TimeWentBackwards { index });
        }
        previous = Some(step.at);

        match (&step.action, &step.outcome) {
            (_, Outcome::Refused(Refusal::EpisodeOver)) => {
                return Err(EvalError::EpisodeOverWithoutTerminal { index });
            }
            // A refusal changed nothing: not a probe, a correction, or a terminal (R1, R12).
            (_, Outcome::Refused(_)) => {}
            (Action::Probe { .. }, Outcome::Probed { .. }) => {
                probes_used = probes_used.saturating_add(1);
            }
            (Action::Correct { .. }, Outcome::Corrected { .. }) => {
                corrections = corrections.saturating_add(1);
            }
            (Action::Declare { .. }, Outcome::Declared) | (Action::Abstain, Outcome::Abstained) => {
                terminal = Some((step.at, step.action));
            }
            _ => return Err(EvalError::OutcomeMismatch { index }),
        }
    }

    // R2, R3, R4, R5: success is decided by the first terminal action alone.
    let success = match terminal {
        Some((_, Action::Declare { fault })) => {
            if truth.is_no_fault() {
                fault.is_none()
            } else {
                fault.is_some_and(|(kind, site)| truth.has_fault(kind, site))
            }
        }
        Some((_, Action::Abstain)) => truth.is_no_fault(),
        _ => false,
    };
    let declared_a_fault = matches!(terminal, Some((_, Action::Declare { fault: Some(_) })));
    let abstained = matches!(terminal, Some((_, Action::Abstain)));

    Ok(Verdict {
        success,
        // R7
        critical_miss: !success && truth.has_critical_fault(),
        // R6
        false_alarm: truth.is_no_fault() && (declared_a_fault || corrections > 0),
        // R8
        abstained,
        // R9
        undecided: terminal.is_none(),
        probes_used,
        corrections,
        // R13
        decision_at: terminal.map(|(at, _)| at),
    })
}
