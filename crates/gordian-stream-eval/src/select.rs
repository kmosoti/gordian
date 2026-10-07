//! Accounting for what a selector escalates and what a follow-up rule retires (work item B4).
//!
//! The rules are in `RULES.md`, section "Selection accounting", one numbered row each (E1 to E8);
//! the comments below cite them. [`score_selection`] is a pure function of its arguments, like
//! [`crate::score_stream`] and [`crate::score_notices`]. It reads three records the harness keeps,
//! none of them a policy input: the notices and retirements of the arm's noticer (with the cause of
//! each retirement), the escalations the stream accepted (with the cost each declared), and the
//! truth that says what each anchor and focus belongs to. It adds no number to `score_stream`'s
//! verdict and moves none: it attributes escalations to the notices they are about, so that the
//! cost of a notice anchored on a decoy or on a late plain incident (which the selection oracle
//! never asks about, and which both precision measures count as correct) can be read.

use crate::notice::NoticeEntry;
use crate::timeserde;
use gordian_core::Instant;
use gordian_stream::oracle::StreamTruth;
use gordian_stream::{HardKind, ObsId, Tier};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What an anchor or a focus belongs to (E2). The slow leak is a hard incident, kept apart from the
/// other hard incidents because B3's ramp noticer acts on it and the follow-up rule's error
/// ("a leak wrongly retired") is about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentClass {
    /// An observation of no incident.
    Background,
    /// A plain incident.
    Plain,
    /// A hard incident outside the slow-leak family.
    Hard,
    /// A slow leak.
    Leak,
    /// A decoy.
    Decoy,
}

impl IncidentClass {
    /// Every class, in the order the counts are written.
    pub const ALL: [IncidentClass; 5] = [
        Self::Background,
        Self::Plain,
        Self::Hard,
        Self::Leak,
        Self::Decoy,
    ];

    /// The word written to the run output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Plain => "plain",
            Self::Hard => "hard",
            Self::Leak => "leak",
            Self::Decoy => "decoy",
        }
    }
}

/// One value per [`IncidentClass`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByClass<T> {
    /// Background.
    pub background: T,
    /// Plain incidents.
    pub plain: T,
    /// Hard incidents outside the slow-leak family.
    pub hard: T,
    /// Slow leaks.
    pub leak: T,
    /// Decoys.
    pub decoy: T,
}

impl<T: Copy> ByClass<T> {
    /// The value of `class`.
    pub fn get(&self, class: IncidentClass) -> T {
        match class {
            IncidentClass::Background => self.background,
            IncidentClass::Plain => self.plain,
            IncidentClass::Hard => self.hard,
            IncidentClass::Leak => self.leak,
            IncidentClass::Decoy => self.decoy,
        }
    }
}

impl<T> ByClass<T> {
    fn slot(&mut self, class: IncidentClass) -> &mut T {
        match class {
            IncidentClass::Background => &mut self.background,
            IncidentClass::Plain => &mut self.plain,
            IncidentClass::Hard => &mut self.hard,
            IncidentClass::Leak => &mut self.leak,
            IncidentClass::Decoy => &mut self.decoy,
        }
    }
}

/// One accepted escalation, as the harness records it: when, about which observation, and the cost
/// the stream declared for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationEntry {
    /// The instant of the step that made the call.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
    /// The observation the question is about.
    pub focus: ObsId,
    /// Tokens the call cost, as the outcome declared them.
    pub tokens: u64,
    /// Modelled nanoseconds the call cost, as the outcome declared them.
    pub modelled_ns: u64,
}

/// One retirement, with why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRetire {
    /// The anomaly.
    pub anomaly: u32,
    /// When it was retired.
    #[serde(with = "timeserde::instant")]
    pub at: Instant,
    /// Whether a follow-up rule retired it (as against its going quiet).
    pub followup: bool,
}

/// Everything [`score_selection`] reads besides the truth, each list in the order recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionTrace {
    /// The notices.
    pub notices: Vec<NoticeEntry>,
    /// The retirements.
    pub retirements: Vec<SelectionRetire>,
    /// The accepted escalations.
    pub escalations: Vec<EscalationEntry>,
}

/// What became of one notice (E3 to E5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeOutcome {
    /// The anomaly.
    pub anomaly: u32,
    /// The incident its anchor belongs to, `None` for background (N1).
    pub incident: Option<u32>,
    /// What the anchor belongs to.
    pub class: IncidentClass,
    /// Escalations that are about this notice (E1).
    pub escalations: u32,
    /// The instant of the earliest of them.
    #[serde(with = "timeserde::option_instant")]
    pub first_escalation_at: Option<Instant>,
    /// When the anomaly was retired, `None` if it was not by the end of the record.
    #[serde(with = "timeserde::option_instant")]
    pub retired_at: Option<Instant>,
    /// It was retired by a follow-up rule.
    pub followup: bool,
    /// It was retired and no escalation is about it (E4).
    pub retired_before_escalation: bool,
}

/// Escalations and what they cost, by what their focus belongs to (E2, E6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationTotals {
    /// Accepted escalations.
    pub calls: ByClass<u32>,
    /// Tokens the calls cost.
    pub tokens: ByClass<u64>,
    /// Modelled nanoseconds the calls cost.
    pub modelled_ns: ByClass<u64>,
    /// Escalations that are about no notice (E1): a call a rule makes directly.
    pub unattributed: u32,
}

/// Notices and what became of them, by what their anchor belongs to (E3 to E5, E7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeFates {
    /// Notices.
    pub notices: ByClass<u32>,
    /// Notices at least one escalation is about.
    pub escalated: ByClass<u32>,
    /// Notices retired before any escalation (E4).
    pub retired_before_escalation: ByClass<u32>,
    /// Notices a follow-up rule retired (E5).
    pub followup_retired: ByClass<u32>,
    /// Notices a follow-up rule retired before any escalation (E5).
    pub followup_before_escalation: ByClass<u32>,
}

/// The score of one stream's selection: every notice and the totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionVerdict {
    /// One entry per notice, in the order recorded.
    pub per_notice: Vec<NoticeOutcome>,
    /// Escalations by class (E2, E6).
    pub escalations: EscalationTotals,
    /// Notices by class (E7).
    pub notices: NoticeFates,
}

/// A record that cannot be right (E8). There is no verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionError {
    /// A notice's anchor, or an escalation's focus, is not an observation of the stream.
    UnknownObservation {
        /// The observation named.
        obs: ObsId,
    },
    /// A retirement of an anomaly that was never noticed, or was retired already.
    RetirementWithoutNotice {
        /// The anomaly.
        anomaly: u32,
    },
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownObservation { obs } => {
                write!(
                    f,
                    "observation {} is not an observation of the stream",
                    obs.0
                )
            }
            Self::RetirementWithoutNotice { anomaly } => {
                write!(f, "retirement of anomaly {anomaly}, which is not live")
            }
        }
    }
}

impl std::error::Error for SelectionError {}

/// The class of the observation `obs` (E2): by the incident its label names, background when none.
fn class_of(
    truth: &StreamTruth,
    obs: ObsId,
) -> Result<(Option<u32>, IncidentClass), SelectionError> {
    if (obs.0 as usize) >= truth.labels.len() {
        return Err(SelectionError::UnknownObservation { obs });
    }
    let Some(id) = truth.incident_of(obs) else {
        return Ok((None, IncidentClass::Background));
    };
    let inc = &truth.incidents[id as usize];
    let class = match inc.tier {
        Tier::Plain => IncidentClass::Plain,
        Tier::Decoy => IncidentClass::Decoy,
        Tier::Hard if inc.shape.hard_kind == Some(HardKind::SlowLeak) => IncidentClass::Leak,
        Tier::Hard => IncidentClass::Hard,
    };
    Ok((Some(id), class))
}

/// Score a record of selection against the truth of its stream.
///
/// # Errors
///
/// A [`SelectionError`] when the record cannot be right (E8).
pub fn score_selection(
    truth: &StreamTruth,
    trace: &SelectionTrace,
) -> Result<SelectionVerdict, SelectionError> {
    // E3: one outcome per notice, in the order recorded, with the class of its anchor (N1).
    let mut per_notice = Vec::with_capacity(trace.notices.len());
    for n in &trace.notices {
        let (incident, class) = class_of(truth, n.anchor)?;
        per_notice.push(NoticeOutcome {
            anomaly: n.anomaly,
            incident,
            class,
            escalations: 0,
            first_escalation_at: None,
            retired_at: None,
            followup: false,
            retired_before_escalation: false,
        });
    }
    // E8, then the retirement of each notice: the first retirement of an anomaly; a second one, or
    // one of an anomaly never noticed, is an error. An anomaly id is unique among notices (N11), so
    // the position found is the notice.
    for r in &trace.retirements {
        let Some(pos) = trace.notices.iter().position(|n| n.anomaly == r.anomaly) else {
            return Err(SelectionError::RetirementWithoutNotice { anomaly: r.anomaly });
        };
        if per_notice[pos].retired_at.is_some() {
            return Err(SelectionError::RetirementWithoutNotice { anomaly: r.anomaly });
        }
        per_notice[pos].retired_at = Some(r.at);
        per_notice[pos].followup = r.followup;
    }

    let mut escalations = EscalationTotals::default();
    for e in &trace.escalations {
        // E2: the call is about the incident its focus belongs to (S2), and costs what it declared.
        let (_, class) = class_of(truth, e.focus)?;
        *escalations.calls.slot(class) += 1;
        *escalations.tokens.slot(class) += e.tokens;
        *escalations.modelled_ns.slot(class) += e.modelled_ns;
        // E1: it is about the latest notice anchored on its focus that was made at or before the
        // call and had not been retired before it (a retirement at the call's own instant is after
        // it: the rung asks before it retires within a step).
        let target = trace
            .notices
            .iter()
            .enumerate()
            .rev()
            .find(|(i, n)| {
                n.anchor == e.focus
                    && n.at <= e.at
                    && per_notice[*i].retired_at.is_none_or(|r| r >= e.at)
            })
            .map(|(i, _)| i);
        match target {
            Some(i) => {
                let o = &mut per_notice[i];
                o.escalations += 1;
                o.first_escalation_at = Some(o.first_escalation_at.map_or(e.at, |f| f.min(e.at)));
            }
            None => escalations.unattributed += 1,
        }
    }

    let mut notices = NoticeFates::default();
    for o in &mut per_notice {
        // E4: retired, and no escalation is about it.
        o.retired_before_escalation = o.retired_at.is_some() && o.escalations == 0;
        // E7.
        *notices.notices.slot(o.class) += 1;
        if o.escalations > 0 {
            *notices.escalated.slot(o.class) += 1;
        }
        if o.retired_before_escalation {
            *notices.retired_before_escalation.slot(o.class) += 1;
        }
        // E5.
        if o.followup {
            *notices.followup_retired.slot(o.class) += 1;
            if o.retired_before_escalation {
                *notices.followup_before_escalation.slot(o.class) += 1;
            }
        }
    }
    Ok(SelectionVerdict {
        per_notice,
        escalations,
        notices,
    })
}
