//! Scoring what an arm declared from memory against hidden truth (work item E1).
//!
//! The rules are in `RULES.md`, section "Memory", one numbered row each (K1 to K10); the comments
//! below cite them. [`score_memory`] is a pure function of its arguments, like
//! [`crate::score_stream`], [`crate::score_notices`] and [`crate::score_selection`]. It reads the
//! trajectory the harness recorded (the same one `score_stream` scores), the record of which
//! accepted declarations were made from memory and from which observation the memory was bound
//! (the *source*), and the truth. It adds no number to `score_stream`'s verdict and moves none.
//!
//! What it separates, and why: a declaration with no escalation about its incident is **unasked**.
//! An unasked correct declaration on a hard incident is what a memory is for (the cheap rung
//! never makes one). An unasked wrong declaration counts every error the arm makes without
//! asking, the cheap rung's own included (W2: 17% of plain incidents, 73% of decoys, with no
//! memory at all), so a memory's own errors are counted over the declarations the harness marks
//! as recalls, and each wrong recall is classed by the source it was bound at: the stored answer
//! was right for its own incident (the key collided with another truth, or the physics had
//! changed) or it was wrong (the memory inherited the reasoner's error).

use crate::step::StreamStep;
use crate::verdict::TierCounts;
use gordian_stream::oracle::{IncidentTruth, StreamTruth};
use gordian_stream::{Diagnosis, HardKind, ObsId, StreamAction, StreamOutcome, Tier};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Where the truth of a memory's source is read from (K5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceTruth {
    /// The source observation is one of this stream's: its incident's truth is in the truth given.
    #[default]
    Here,
    /// The memory was carried from an earlier stream, whose observation numbers mean nothing in
    /// this one: the harness read the truth of the source's incident there, and gives it.
    Earlier {
        /// What the earlier stream's truth says about the incident the source observation belongs
        /// to (`None`, "not an incident", for background and for a decoy).
        truth: Diagnosis,
    },
}

/// What a memory was bound at (K5): the observation the answer was about and the answer, as the
/// memory stored it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallSource {
    /// The observation the reasoner was asked about (the focus of the answer the memory kept), in
    /// the stream it was asked in.
    pub obs: ObsId,
    /// The diagnosis the memory stored from that answer, with the site it named.
    pub diagnosis: Diagnosis,
    /// Whose truth judges it.
    #[serde(default)]
    pub truth: SourceTruth,
}

/// One declaration made from memory, as the harness records it (K5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallEntry {
    /// The index in the trajectory of the accepted declaration it is.
    pub step: usize,
    /// What the memory was bound at; `None` when the memory does not say.
    #[serde(default)]
    pub source: Option<RecallSource>,
}

/// Whether the stored answer was right for the incident it was about (K5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceClass {
    /// The stored diagnosis equals the truth of the incident the source observation belongs to
    /// (`None` for background and for a decoy).
    Right,
    /// It does not.
    Wrong,
    /// The memory recorded no source.
    Unknown,
}

/// The six cells a recall falls in: its own outcome against its source's class (K6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallCells {
    /// Correct recall, source right.
    pub correct_source_right: u32,
    /// Correct recall, source wrong.
    pub correct_source_wrong: u32,
    /// Correct recall, no source recorded.
    pub correct_source_unknown: u32,
    /// Wrong recall, source right: a collision or staleness.
    pub wrong_source_right: u32,
    /// Wrong recall, source wrong: inherited from the reasoner's error.
    pub wrong_source_wrong: u32,
    /// Wrong recall, no source recorded.
    pub wrong_source_unknown: u32,
}

impl RecallCells {
    fn add(&mut self, correct: bool, source: SourceClass) {
        let cell = match (correct, source) {
            (true, SourceClass::Right) => &mut self.correct_source_right,
            (true, SourceClass::Wrong) => &mut self.correct_source_wrong,
            (true, SourceClass::Unknown) => &mut self.correct_source_unknown,
            (false, SourceClass::Right) => &mut self.wrong_source_right,
            (false, SourceClass::Wrong) => &mut self.wrong_source_wrong,
            (false, SourceClass::Unknown) => &mut self.wrong_source_unknown,
        };
        *cell += 1;
    }

    /// Every recall in the cells.
    pub fn total(&self) -> u32 {
        self.correct() + self.wrong()
    }

    /// Correct recalls.
    pub fn correct(&self) -> u32 {
        self.correct_source_right + self.correct_source_wrong + self.correct_source_unknown
    }

    /// Wrong recalls.
    pub fn wrong(&self) -> u32 {
        self.wrong_source_right + self.wrong_source_wrong + self.wrong_source_unknown
    }

    /// Recalls whose source was right, correct or not: the denominator of the collision share.
    pub fn source_right(&self) -> u32 {
        self.correct_source_right + self.wrong_source_right
    }

    fn plus(&mut self, other: &Self) {
        self.correct_source_right += other.correct_source_right;
        self.correct_source_wrong += other.correct_source_wrong;
        self.correct_source_unknown += other.correct_source_unknown;
        self.wrong_source_right += other.wrong_source_right;
        self.wrong_source_wrong += other.wrong_source_wrong;
        self.wrong_source_unknown += other.wrong_source_unknown;
    }
}

/// The score of one recall (K5, K6, K7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallScore {
    /// The trajectory step of the recall.
    pub step: usize,
    /// The incident its anchor belongs to; `None` for background.
    pub incident: Option<u32>,
    /// That incident's tier.
    pub tier: Option<Tier>,
    /// Whether it equals the truth of what its anchor belongs to (K5).
    pub correct: bool,
    /// Whether its stored answer was right for the incident it was about (K5).
    pub source: SourceClass,
    /// The incident the source observation belongs to in this stream; `None` for background, for
    /// no source and for a source in an earlier stream.
    pub source_incident: Option<u32>,
}

/// The memory score of one incident (K1 to K7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentMemory {
    /// The incident's id.
    pub id: u32,
    /// Its tier.
    pub tier: Tier,
    /// The incident it repeats, from the truth (K1).
    pub recurrence_of: Option<u32>,
    /// A hard incident with an earlier hard incident of the same family and mode at another site
    /// (K2).
    pub same_family_earlier: bool,
    /// Accepted declarations anchored on it that equal its truth (S4, S5).
    pub correct_declarations: u32,
    /// Accepted declarations anchored on it that do not (S11).
    pub wrong_declarations: u32,
    /// Accepted escalations whose focus belongs to it (S13).
    pub escalations: u32,
    /// At least one correct declaration and no escalation (K3).
    pub unasked_correct: bool,
    /// At least one wrong declaration and no escalation, whoever made it (K4).
    pub unasked_wrong: bool,
    /// At least one wrong declaration made from memory, and no escalation (K6).
    pub stale_wrong: bool,
    /// The declarations made from memory that are anchored on it, in the six cells (K5, K6).
    pub recalls: RecallCells,
}

/// The totals of one stream (K7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryTotals {
    /// Declarations made from memory, in the six cells, over every anchor.
    pub recalls: RecallCells,
    /// Of those, the ones anchored on a plain incident.
    pub recalls_on_plain: RecallCells,
    /// Anchored on a hard incident.
    pub recalls_on_hard: RecallCells,
    /// Anchored on a decoy.
    pub recalls_on_decoy: RecallCells,
    /// Anchored on background.
    pub recalls_on_background: RecallCells,
    /// Incidents with an unasked correct declaration, by tier (K3).
    pub unasked_correct: TierCounts,
    /// Incidents with an unasked wrong declaration, by tier (K4).
    pub unasked_wrong: TierCounts,
    /// Incidents with a wrong recall and no escalation, by tier (K6).
    pub stale_wrong: TierCounts,
    /// Hard incidents with `recurrence_of` set (K1).
    pub hard_recurrences: u32,
    /// Of those, with an unasked correct declaration.
    pub hard_recurrences_unasked_correct: u32,
    /// Hard incidents with no `recurrence_of` and `same_family_earlier` (K2).
    pub hard_elsewhere: u32,
    /// Of those, with an unasked correct declaration.
    pub hard_elsewhere_unasked_correct: u32,
    /// Hard incidents with either (a recurrence, or the same family elsewhere).
    pub hard_reachable: u32,
    /// Of those, with an unasked correct declaration.
    pub hard_reachable_unasked_correct: u32,
}

/// The memory score of a stream: every incident, and the totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryVerdict {
    /// One entry per incident of the truth, in id order.
    pub per_incident: Vec<IncidentMemory>,
    /// One entry per recall, in the order recorded.
    pub per_recall: Vec<RecallScore>,
    /// Totals over the stream.
    pub totals: MemoryTotals,
}

/// A record that cannot be right (K8). Scoring stops: there is no verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryError {
    /// A recall names a trajectory step that does not exist.
    StepOutOfRange {
        /// The step index named.
        step: usize,
    },
    /// A recall names a step that is not an accepted declaration.
    NotADeclaration {
        /// The step index named.
        step: usize,
    },
    /// Two recalls name the same step.
    StepRepeated {
        /// The step index named twice.
        step: usize,
    },
    /// An accepted declaration, or a recall's source, names an observation that does not exist.
    UnknownObservation {
        /// The step index (for a source, the step of its recall).
        step: usize,
        /// The observation named.
        obs: ObsId,
    },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::StepOutOfRange { step } => {
                write!(f, "a recall names step {step}, which does not exist")
            }
            MemoryError::NotADeclaration { step } => {
                write!(
                    f,
                    "a recall names step {step}, which is not an accepted declaration"
                )
            }
            MemoryError::StepRepeated { step } => {
                write!(f, "two recalls name step {step}")
            }
            MemoryError::UnknownObservation { step, obs } => {
                write!(
                    f,
                    "step {step} names observation {}, which does not exist",
                    obs.0
                )
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// The class of a hard incident: its hard kind and its mode (K2). `None` for an incident that is
/// not hard or has no hard kind.
fn class_of(inc: &IncidentTruth) -> Option<(HardKind, Option<bool>)> {
    if inc.tier != Tier::Hard {
        return None;
    }
    inc.shape
        .hard_kind
        .map(|k| (k, inc.shape.contradicts_early))
}

/// The incident's site: the first service it occupies (N13), if it occupies one.
fn site_of(inc: &IncidentTruth) -> Option<u32> {
    inc.occupies.first().map(|s| s.0)
}

/// K2 for every incident: an earlier hard incident of the same class at another site exists.
fn same_family_earlier(truth: &StreamTruth) -> Vec<bool> {
    truth
        .incidents
        .iter()
        .enumerate()
        .map(|(i, inc)| {
            let (Some(class), Some(site)) = (class_of(inc), site_of(inc)) else {
                return false;
            };
            truth.incidents[..i].iter().any(|earlier| {
                class_of(earlier) == Some(class) && site_of(earlier).is_some_and(|s| s != site)
            })
        })
        .collect()
}

/// The truth of what an observation belongs to: the incident's true diagnosis, or `None` (not an
/// incident) for background (K5). A decoy's truth is `None` too.
fn truth_of_obs(truth: &StreamTruth, obs: ObsId) -> Option<Diagnosis> {
    if (obs.0 as usize) >= truth.labels.len() {
        return None;
    }
    Some(match truth.incident_of(obs) {
        Some(id) => truth.incidents[id as usize].truth,
        None => None,
    })
}

#[derive(Default, Clone)]
struct Acc {
    correct: u32,
    wrong: u32,
    escalations: u32,
    cells: RecallCells,
}

/// Score `trajectory` and the record of recalls `recalls` against `truth`.
///
/// Steps that the stream refused are not read (S3). The trajectory must be one `score_stream`
/// accepts; this function checks only what it reads (K8).
pub fn score_memory(
    truth: &StreamTruth,
    trajectory: &[StreamStep],
    recalls: &[RecallEntry],
) -> Result<MemoryVerdict, MemoryError> {
    let mut acc = vec![Acc::default(); truth.incidents.len()];
    let mut totals = MemoryTotals::default();

    // K8: the recall record names accepted declarations, each once.
    let mut named = vec![false; trajectory.len()];
    for r in recalls {
        let Some(step) = trajectory.get(r.step) else {
            return Err(MemoryError::StepOutOfRange { step: r.step });
        };
        let accepted_declaration = matches!(
            (&step.action, &step.outcome),
            (StreamAction::Declare { .. }, StreamOutcome::Declared { .. })
        );
        if !accepted_declaration {
            return Err(MemoryError::NotADeclaration { step: r.step });
        }
        if std::mem::replace(&mut named[r.step], true) {
            return Err(MemoryError::StepRepeated { step: r.step });
        }
    }

    let outside = |obs: ObsId| (obs.0 as usize) >= truth.labels.len();

    // K3, K4, K6: the declarations and escalations about each incident.
    for (index, step) in trajectory.iter().enumerate() {
        match (&step.action, &step.outcome) {
            (StreamAction::Declare { anchor, diagnosis }, StreamOutcome::Declared { .. }) => {
                if outside(*anchor) {
                    return Err(MemoryError::UnknownObservation {
                        step: index,
                        obs: *anchor,
                    });
                }
                let incident = truth.incident_of(*anchor);
                let correct = match incident {
                    Some(id) => *diagnosis == truth.incidents[id as usize].truth,
                    None => diagnosis.is_none(),
                };
                if let Some(id) = incident {
                    let a = &mut acc[id as usize];
                    if correct {
                        a.correct += 1;
                    } else {
                        a.wrong += 1;
                    }
                }
            }
            (StreamAction::Escalate { question, .. }, StreamOutcome::Escalated { .. }) => {
                let gordian_stream::Question::Diagnose { focus } = question;
                if outside(*focus) {
                    return Err(MemoryError::UnknownObservation {
                        step: index,
                        obs: *focus,
                    });
                }
                if let Some(id) = truth.incident_of(*focus) {
                    acc[id as usize].escalations += 1;
                }
            }
            _ => {}
        }
    }

    // K5, K6: each recall, in the order recorded.
    let mut per_recall = Vec::with_capacity(recalls.len());
    for r in recalls {
        let step = &trajectory[r.step];
        let StreamAction::Declare { anchor, diagnosis } = &step.action else {
            unreachable!("checked above: a recall names an accepted declaration");
        };
        let incident = truth.incident_of(*anchor);
        let correct = match incident {
            Some(id) => *diagnosis == truth.incidents[id as usize].truth,
            None => diagnosis.is_none(),
        };
        let class = match r.source {
            None => SourceClass::Unknown,
            Some(source) => {
                let stored_truth = match source.truth {
                    SourceTruth::Earlier { truth } => truth,
                    SourceTruth::Here => {
                        let Some(here) = truth_of_obs(truth, source.obs) else {
                            return Err(MemoryError::UnknownObservation {
                                step: r.step,
                                obs: source.obs,
                            });
                        };
                        here
                    }
                };
                if source.diagnosis == stored_truth {
                    SourceClass::Right
                } else {
                    SourceClass::Wrong
                }
            }
        };
        per_recall.push(RecallScore {
            step: r.step,
            incident,
            tier: incident.map(|id| truth.incidents[id as usize].tier),
            correct,
            source: class,
            source_incident: r.source.and_then(|source| match source.truth {
                SourceTruth::Here => truth.incident_of(source.obs),
                SourceTruth::Earlier { .. } => None,
            }),
        });
        totals.recalls.add(correct, class);
        match incident.map(|id| truth.incidents[id as usize].tier) {
            Some(Tier::Plain) => totals.recalls_on_plain.add(correct, class),
            Some(Tier::Hard) => totals.recalls_on_hard.add(correct, class),
            Some(Tier::Decoy) => totals.recalls_on_decoy.add(correct, class),
            None => totals.recalls_on_background.add(correct, class),
        }
        if let Some(id) = incident {
            acc[id as usize].cells.add(correct, class);
        }
    }

    let earlier = same_family_earlier(truth);
    let mut per_incident = Vec::with_capacity(truth.incidents.len());
    for ((inc, a), same_family) in truth.incidents.iter().zip(&acc).zip(&earlier) {
        let unasked_correct = a.correct > 0 && a.escalations == 0;
        let unasked_wrong = a.wrong > 0 && a.escalations == 0;
        let stale_wrong = a.cells.wrong() > 0 && a.escalations == 0;
        let tier_counts = |counts: &mut TierCounts| match inc.tier {
            Tier::Plain => counts.plain += 1,
            Tier::Hard => counts.hard += 1,
            Tier::Decoy => counts.decoy += 1,
        };
        if unasked_correct {
            tier_counts(&mut totals.unasked_correct);
        }
        if unasked_wrong {
            tier_counts(&mut totals.unasked_wrong);
        }
        if stale_wrong {
            tier_counts(&mut totals.stale_wrong);
        }
        if inc.tier == Tier::Hard {
            let recurrence = inc.recurrence_of.is_some();
            let elsewhere = !recurrence && *same_family;
            totals.hard_recurrences += u32::from(recurrence);
            totals.hard_recurrences_unasked_correct += u32::from(recurrence && unasked_correct);
            totals.hard_elsewhere += u32::from(elsewhere);
            totals.hard_elsewhere_unasked_correct += u32::from(elsewhere && unasked_correct);
            let reachable = recurrence || *same_family;
            totals.hard_reachable += u32::from(reachable);
            totals.hard_reachable_unasked_correct += u32::from(reachable && unasked_correct);
        }
        per_incident.push(IncidentMemory {
            id: inc.id,
            tier: inc.tier,
            recurrence_of: inc.recurrence_of,
            same_family_earlier: *same_family,
            correct_declarations: a.correct,
            wrong_declarations: a.wrong,
            escalations: a.escalations,
            unasked_correct,
            unasked_wrong,
            stale_wrong,
            recalls: a.cells,
        });
    }
    // The per-tier recall cells add up to the total (K7): checked in debug builds.
    debug_assert_eq!(totals.recalls.total() as usize, recalls.len());
    let mut by_anchor = RecallCells::default();
    for cells in [
        &totals.recalls_on_plain,
        &totals.recalls_on_hard,
        &totals.recalls_on_decoy,
        &totals.recalls_on_background,
    ] {
        by_anchor.plus(cells);
    }
    debug_assert_eq!(by_anchor, totals.recalls);

    Ok(MemoryVerdict {
        per_incident,
        per_recall,
        totals,
    })
}
