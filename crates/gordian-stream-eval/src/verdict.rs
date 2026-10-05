//! The score of a stream. Every field is defined by a row of `RULES.md`.

use crate::timeserde;
use gordian_core::Instant;
use gordian_stream::Tier;
use serde::{Deserialize, Serialize};

/// The score of one incident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentVerdict {
    /// The incident's id (dense from zero, in order of arrival).
    pub id: u32,
    /// Its tier.
    pub tier: Tier,
    /// Whether a miss counts under the critical bound.
    pub critical: bool,
    /// Accepted declarations anchored on this incident that equal its truth (S4, S5). For a
    /// decoy these are the dismissals (S15).
    pub correct_declarations: u32,
    /// Accepted declarations anchored on this incident that do not equal its truth (S11). For a
    /// decoy these are the false alarms (S14).
    pub wrong_declarations: u32,
    /// When the first correct declaration was made, whatever the deadline (S7, S8).
    #[serde(with = "timeserde::option_instant")]
    pub first_correct_at: Option<Instant>,
    /// `first_correct_at` minus the incident's onset, nanoseconds (S8).
    pub time_to_first_correct_ns: Option<u64>,
    /// A plain or hard incident with a correct declaration at or before its deadline (S6).
    /// Always false for a decoy, which has no deadline.
    pub correct_by_deadline: bool,
    /// A plain or hard incident that is not `correct_by_deadline` (S9). Always false for a decoy.
    pub missed: bool,
    /// `critical` and `missed` (S10).
    pub critical_miss: bool,
    /// Accepted escalations whose focus anchors in this incident (S13).
    pub escalations: u32,
    /// Of those, the calls that were informed (S13).
    pub informed_escalations: u32,
    /// Of those, the calls whose answer was right (S13).
    pub correct_escalations: u32,
}

/// Incident counts by tier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierCounts {
    /// Plain incidents.
    pub plain: u32,
    /// Hard incidents.
    pub hard: u32,
    /// Decoys.
    pub decoy: u32,
}

/// Counts for the two tiers that have a deadline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoredCounts {
    /// Plain incidents.
    pub plain: u32,
    /// Hard incidents.
    pub hard: u32,
}

/// Escalations, classified by what they were about (S23 to S25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationCounts {
    /// Accepted escalations about a hard incident.
    pub needed: u32,
    /// Accepted escalations about a plain incident or a decoy.
    pub unneeded: u32,
    /// Accepted escalations whose focus belongs to no incident.
    pub background: u32,
    /// Hard incidents with at least one accepted escalation (S24).
    pub hard_incidents_escalated: u32,
    /// Plain incidents and decoys with at least one accepted escalation (S24).
    pub other_incidents_escalated: u32,
    /// Accepted escalations that were informed, whatever they were about (S25).
    pub informed: u32,
    /// Accepted escalations whose answer was right, whatever they were about (S25).
    pub correct: u32,
}

/// What the reasoner cost, in its own units (S26).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonerUsage {
    /// Accepted calls.
    pub calls: u64,
    /// References in their contexts, summed.
    pub refs: u64,
    /// Tokens, as the outcomes declared them.
    pub tokens: u64,
    /// Modelled nanoseconds, as the outcomes declared them.
    pub modelled_ns: u64,
}

/// The score of a whole stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamTotals {
    /// Incidents by tier (S19).
    pub incidents: TierCounts,
    /// Incidents with `critical` set (S19).
    pub critical_incidents: u32,
    /// Plain and hard incidents correct by their deadline (S20).
    pub correct: ScoredCounts,
    /// Plain and hard incidents missed (S20).
    pub missed: ScoredCounts,
    /// Plain and hard incidents critically missed (S20).
    pub critical_missed: ScoredCounts,
    /// Wrong declarations about plain and hard incidents (S20).
    pub wrong_declarations: u32,
    /// Decoys with at least one correct dismissal (S21).
    pub decoys_dismissed: u32,
    /// Decoys with at least one false alarm (S21).
    pub decoys_alarmed: u32,
    /// Decoys nothing was declared about (S21).
    pub decoys_silent: u32,
    /// Declarations of an incident that were about a decoy or about nothing (S22).
    pub false_alarms: u32,
    /// Of those, the ones anchored on background (S18, S22).
    pub false_alarms_on_background: u32,
    /// Escalations (S23 to S25).
    pub escalations: EscalationCounts,
    /// Reasoner cost (S26).
    pub reasoner: ReasonerUsage,
}

impl StreamTotals {
    /// Share of accepted escalations that were about a hard incident: `needed` over all
    /// escalations, background included. `None` when nothing was escalated. A call-level ratio:
    /// ten escalations about one hard incident score ten needed ones.
    pub fn escalation_precision(&self) -> Option<f64> {
        let e = &self.escalations;
        let all = u64::from(e.needed) + u64::from(e.unneeded) + u64::from(e.background);
        if all == 0 {
            None
        } else {
            Some(f64::from(e.needed) / all as f64)
        }
    }

    /// Share of hard incidents that were escalated at least once. `None` when the stream has no
    /// hard incident. An incident-level ratio.
    pub fn escalation_recall(&self) -> Option<f64> {
        if self.incidents.hard == 0 {
            None
        } else {
            Some(
                f64::from(self.escalations.hard_incidents_escalated)
                    / f64::from(self.incidents.hard),
            )
        }
    }
}

/// The score of one stream: every incident, and the totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamVerdict {
    /// One entry per incident of the truth, in id order.
    pub per_incident: Vec<IncidentVerdict>,
    /// Totals over the stream.
    pub totals: StreamTotals,
}
