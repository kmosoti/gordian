//! Scoring what a noticer noticed against hidden truth (work item B1).
//!
//! The rules are in `RULES.md`, section "Notices", one numbered row each (N1 to N12); the
//! comments below cite them. [`score_notices`] is a pure function of its arguments, as
//! [`crate::score_stream`] is. It scores the **record of notices and retirements** a harness kept,
//! not a trajectory: a notice is not an action the stream answers, so it has no place in a
//! [`crate::StreamStep`], and noticing is measured without replaying a ledger.
//!
//! The truth carries no instant for an individual observation (`RULES.md`, judgement 8), but the
//! stream's passive observations are public and the harness holds their instants, so they are an
//! input here: `obs_at`, parallel to the truth's labels.

use crate::timeserde;
use gordian_core::Instant;
use gordian_stream::oracle::{ObsLabel, StreamTruth};
use gordian_stream::{ObsId, Tier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// "Within 1 s" of the incident's first observation, nanoseconds (N5). The bound is inclusive.
pub const ANCHOR_WINDOW_NS: u64 = 1_000_000_000;

/// One notice, as a harness records it: which anomaly, anchored on which observation, at which
/// instant (the instant of the step that yielded it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeEntry {
    /// The anomaly's id, unique within a noticer's record.
    pub anomaly: u32,
    /// The observation it is anchored on.
    pub anchor: ObsId,
    /// When it was noticed.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
}

/// One retirement: the anomaly the harness stopped working on, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetireEntry {
    /// The anomaly's id.
    pub anomaly: u32,
    /// When it was retired.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
}

/// Everything a noticer recorded over one stream, each list in the order recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeTrace {
    /// The notices.
    pub notices: Vec<NoticeEntry>,
    /// The retirements.
    pub retirements: Vec<RetireEntry>,
}

/// The score of one incident (N1 to N6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentNotices {
    /// The incident's id.
    pub id: u32,
    /// Its tier.
    pub tier: Tier,
    /// When its first observation was emitted, if it has one (N4).
    #[serde(with = "timeserde::option_instant")]
    pub first_observation_at: Option<Instant>,
    /// Notices whose anchor belongs to it (N1, N6).
    pub notices: u32,
    /// At least one notice (N2).
    pub noticed: bool,
    /// The instant of the earliest notice about it (N3).
    #[serde(with = "timeserde::option_instant")]
    pub first_notice_at: Option<Instant>,
    /// `first_notice_at` minus `first_observation_at`, nanoseconds (N4).
    pub notice_latency_ns: Option<u64>,
    /// A notice about it whose anchor is within [`ANCHOR_WINDOW_NS`] of its first observation (N5).
    pub anchor_correct: bool,
}

/// The score of one notice (N7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeScore {
    /// The incident the anchor belongs to, `None` for background (N1).
    pub incident: Option<u32>,
    /// That incident's tier.
    pub tier: Option<Tier>,
    /// The anchor's instant and the incident's first observation's differ by this many
    /// nanoseconds; `None` for background or an incident with no observation (N7).
    pub anchor_offset_ns: Option<u64>,
    /// The anchor belongs to an incident and is within [`ANCHOR_WINDOW_NS`] of its first
    /// observation (N5, N7).
    pub anchor_correct: bool,
}

pub use crate::verdict::TierCounts;

/// The totals of one stream's notices (N8 to N10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeTotals {
    /// Notices recorded.
    pub notices: u32,
    /// Notices anchored on background (N8).
    pub on_background: u32,
    /// Notices anchored on a plain incident (N8).
    pub on_plain: u32,
    /// Notices anchored on a hard incident (N8).
    pub on_hard: u32,
    /// Notices anchored on a decoy (N8).
    pub on_decoy: u32,
    /// Incidents with at least one notice, by tier (N9).
    pub noticed: TierCounts,
    /// Incidents with an anchor-correct notice, by tier (N9).
    pub anchor_correct: TierCounts,
    /// Retirements recorded (N10).
    pub retirements: u32,
}

/// The score of one stream's notices: every incident, every notice, and the totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeVerdict {
    /// One entry per incident of the truth, in id order.
    pub per_incident: Vec<IncidentNotices>,
    /// One entry per notice, in the order recorded.
    pub per_notice: Vec<NoticeScore>,
    /// Totals over the stream.
    pub totals: NoticeTotals,
}

/// A record that cannot be right. There is no verdict: it would hide a harness bug inside a
/// results table. `index` is the position in the list of the first offending entry; the checks
/// run in the order of N11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoticeEvalError {
    /// `obs_at` does not have one instant per label (N11).
    ObservationInstantsMismatch {
        /// Labels in the truth.
        labels: usize,
        /// Instants given.
        instants: usize,
    },
    /// An incident's `id` is not its position in the truth's incidents (N11).
    IncidentIdMismatch {
        /// Position in `incidents`.
        index: usize,
        /// The id found there.
        id: u32,
    },
    /// A label names an incident that is not in the truth (N11).
    LabelOfUnknownIncident {
        /// The observation.
        obs: ObsId,
        /// The incident id it names.
        incident: u32,
    },
    /// A notice's anchor is not an observation of the stream (N11).
    UnknownAnchor {
        /// Index of the notice.
        index: usize,
        /// The anchor named.
        anchor: ObsId,
    },
    /// A notice's instant is earlier than its anchor's (N11).
    NoticeBeforeAnchor {
        /// Index of the notice.
        index: usize,
    },
    /// A notice's instant is earlier than the notice before it (N11).
    NoticeTimeWentBackwards {
        /// Index of the notice.
        index: usize,
    },
    /// An anomaly is noticed twice (N11).
    AnomalyNoticedTwice {
        /// Index of the second notice.
        index: usize,
        /// The anomaly.
        anomaly: u32,
    },
    /// A retirement's instant is earlier than the retirement before it (N11).
    RetirementTimeWentBackwards {
        /// Index of the retirement.
        index: usize,
    },
    /// A retirement of an anomaly that was never noticed, or was retired already (N11).
    RetirementWithoutNotice {
        /// Index of the retirement.
        index: usize,
        /// The anomaly.
        anomaly: u32,
    },
    /// A retirement earlier than the notice of the same anomaly (N11).
    RetirementBeforeNotice {
        /// Index of the retirement.
        index: usize,
    },
}

impl fmt::Display for NoticeEvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservationInstantsMismatch { labels, instants } => {
                write!(f, "{instants} observation instants for {labels} labels")
            }
            Self::IncidentIdMismatch { index, id } => {
                write!(f, "incident at position {index} has id {id}")
            }
            Self::LabelOfUnknownIncident { obs, incident } => {
                write!(f, "observation {} names unknown incident {incident}", obs.0)
            }
            Self::UnknownAnchor { index, anchor } => {
                write!(
                    f,
                    "notice {index}: anchor {} is not an observation",
                    anchor.0
                )
            }
            Self::NoticeBeforeAnchor { index } => {
                write!(f, "notice {index} is earlier than its anchor")
            }
            Self::NoticeTimeWentBackwards { index } => {
                write!(f, "notice {index} is earlier than the notice before it")
            }
            Self::AnomalyNoticedTwice { index, anomaly } => {
                write!(f, "notice {index}: anomaly {anomaly} was noticed already")
            }
            Self::RetirementTimeWentBackwards { index } => {
                write!(
                    f,
                    "retirement {index} is earlier than the retirement before it"
                )
            }
            Self::RetirementWithoutNotice { index, anomaly } => {
                write!(f, "retirement {index}: anomaly {anomaly} is not live")
            }
            Self::RetirementBeforeNotice { index } => {
                write!(f, "retirement {index} is earlier than its notice")
            }
        }
    }
}

impl std::error::Error for NoticeEvalError {}

fn bump(counts: &mut TierCounts, tier: Tier) {
    match tier {
        Tier::Plain => counts.plain += 1,
        Tier::Hard => counts.hard += 1,
        Tier::Decoy => counts.decoy += 1,
    }
}

/// Score a record of notices against the truth of its stream.
///
/// `obs_at[i]` is the instant observation `i` was emitted (the stream's public observation
/// list), parallel to `truth.labels`.
///
/// # Errors
///
/// A [`NoticeEvalError`] when the record cannot be right (N11).
pub fn score_notices(
    truth: &StreamTruth,
    obs_at: &[Instant],
    trace: &NoticeTrace,
) -> Result<NoticeVerdict, NoticeEvalError> {
    // N11, first check: one instant per label.
    if obs_at.len() != truth.labels.len() {
        return Err(NoticeEvalError::ObservationInstantsMismatch {
            labels: truth.labels.len(),
            instants: obs_at.len(),
        });
    }
    for (index, inc) in truth.incidents.iter().enumerate() {
        if inc.id as usize != index {
            return Err(NoticeEvalError::IncidentIdMismatch { index, id: inc.id });
        }
    }
    for (i, label) in truth.labels.iter().enumerate() {
        if let ObsLabel::Incident { id, .. } = label
            && (*id as usize) >= truth.incidents.len()
        {
            return Err(NoticeEvalError::LabelOfUnknownIncident {
                obs: ObsId(i as u32),
                incident: *id,
            });
        }
    }
    // N11, in order: for each notice, the anchor exists (a), the notice is not before it (b), the
    // notices do not go back in time (c), the anomaly is new (d).
    let mut noticed_at: BTreeMap<u32, Instant> = BTreeMap::new();
    let mut previous: Option<Instant> = None;
    for (index, n) in trace.notices.iter().enumerate() {
        let Some(anchor_at) = obs_at.get(n.anchor.0 as usize).copied() else {
            return Err(NoticeEvalError::UnknownAnchor {
                index,
                anchor: n.anchor,
            });
        };
        if n.at < anchor_at {
            return Err(NoticeEvalError::NoticeBeforeAnchor { index });
        }
        if previous.is_some_and(|p| n.at < p) {
            return Err(NoticeEvalError::NoticeTimeWentBackwards { index });
        }
        previous = Some(n.at);
        if noticed_at.insert(n.anomaly, n.at).is_some() {
            return Err(NoticeEvalError::AnomalyNoticedTwice {
                index,
                anomaly: n.anomaly,
            });
        }
    }
    let mut live = noticed_at.clone();
    let mut previous: Option<Instant> = None;
    for (index, r) in trace.retirements.iter().enumerate() {
        if previous.is_some_and(|p| r.at < p) {
            return Err(NoticeEvalError::RetirementTimeWentBackwards { index });
        }
        previous = Some(r.at);
        let Some(at) = live.remove(&r.anomaly) else {
            return Err(NoticeEvalError::RetirementWithoutNotice {
                index,
                anomaly: r.anomaly,
            });
        };
        if r.at < at {
            return Err(NoticeEvalError::RetirementBeforeNotice { index });
        }
    }

    // Per incident: the first observation and its instant (N4).
    let first_at: Vec<Option<Instant>> = truth
        .incidents
        .iter()
        .map(|i| {
            i.observations
                .first()
                .and_then(|o| obs_at.get(o.0 as usize).copied())
        })
        .collect();
    let mut per_incident: Vec<IncidentNotices> = truth
        .incidents
        .iter()
        .zip(&first_at)
        .map(|(i, first)| IncidentNotices {
            id: i.id,
            tier: i.tier,
            first_observation_at: *first,
            notices: 0,
            noticed: false,
            first_notice_at: None,
            notice_latency_ns: None,
            anchor_correct: false,
        })
        .collect();

    let mut totals = NoticeTotals {
        retirements: trace.retirements.len() as u32,
        ..NoticeTotals::default()
    };
    let mut per_notice = Vec::with_capacity(trace.notices.len());
    for n in &trace.notices {
        totals.notices += 1;
        // N1: the anchor's label names the incident, or none.
        let owner = truth.incident_of(n.anchor);
        let Some(id) = owner else {
            totals.on_background += 1;
            per_notice.push(NoticeScore {
                incident: None,
                tier: None,
                anchor_offset_ns: None,
                anchor_correct: false,
            });
            continue;
        };
        let inc = &mut per_incident[id as usize];
        match inc.tier {
            Tier::Plain => totals.on_plain += 1,
            Tier::Hard => totals.on_hard += 1,
            Tier::Decoy => totals.on_decoy += 1,
        }
        inc.notices += 1;
        inc.noticed = true;
        if inc.first_notice_at.is_none_or(|t| n.at < t) {
            inc.first_notice_at = Some(n.at);
        }
        let anchor_at = obs_at[n.anchor.0 as usize];
        let offset = inc
            .first_observation_at
            .map(|first| anchor_at.0.abs_diff(first.0));
        let correct = offset.is_some_and(|o| o <= ANCHOR_WINDOW_NS);
        inc.anchor_correct |= correct;
        per_notice.push(NoticeScore {
            incident: Some(id),
            tier: Some(inc.tier),
            anchor_offset_ns: offset,
            anchor_correct: correct,
        });
    }
    for inc in &mut per_incident {
        inc.notice_latency_ns = match (inc.first_notice_at, inc.first_observation_at) {
            (Some(notice), Some(first)) => Some(notice.0.saturating_sub(first.0)),
            _ => None,
        };
        if inc.noticed {
            bump(&mut totals.noticed, inc.tier);
        }
        if inc.anchor_correct {
            bump(&mut totals.anchor_correct, inc.tier);
        }
    }
    Ok(NoticeVerdict {
        per_incident,
        per_notice,
        totals,
    })
}
