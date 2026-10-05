//! `always_escalate`: every anomaly the cheap rung notices is escalated at once.
//!
//! The charter's "quality ceiling at full cost; not assumed optimal". One escalation per noticed
//! anomaly, the step it is noticed, with the context the shared rung builds from what it holds
//! near the focus at that moment. Evidence that arrives after the question is asked (a hard
//! incident's second phase, a decoy's recovery) is not in the context, which is what "at once"
//! costs. It spends the reasoner budget on noise anomalies as readily as on incidents; when the
//! budget is spent the harness refuses further calls and nothing is charged.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "always_escalate";

/// The rule: escalate each noticed anomaly once, when it is noticed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Always;

impl EscalationRule for Always {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| v.attempts == 0)
            .map(|v| v.id)
            .collect()
    }
}
