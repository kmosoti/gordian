//! `change_triggered`: recompute and escalate only when the evidence about an anomaly changed.
//!
//! The charter's "whether salience adds anything beyond skipping unchanged inputs". The evidence
//! about an anomaly is summarised by the rung's digest: the symptom tags of its abnormal
//! observations and the number of services they are about. The arm escalates when an anomaly has
//! no call in flight and its digest differs from the digest at its last escalation (or it has
//! none), so the first escalation is at notice and later ones only when a new kind of symptom or
//! a new service appears. A repeated heartbeat changes nothing; a decoy's silence changes nothing
//! either, so this arm never learns that a decoy resolved. Those are the properties of the
//! baseline, not defects of its build.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "change_triggered";

/// The rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct Change;

impl EscalationRule for Change {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| v.pending == 0 && v.last_attempt_digest != Some(v.digest))
            .map(|v| v.id)
            .collect()
    }
}
