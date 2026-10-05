//! `always_escalate`: every anomaly the cheap rung notices is escalated, `delay_ns` after it is noticed.
//!
//! The charter's "quality ceiling at full cost; not assumed optimal". One escalation per noticed
//! anomaly, with the context the shared rung builds from what it holds near the focus at that
//! moment. With the default delay of zero that moment is the step the anomaly is noticed, and
//! evidence that arrives after the question is asked (a hard incident's second phase, a decoy's
//! recovery) is not in the context, which is what "at once" costs. The delay is a timing knob and
//! nothing more: `delay_ns` after notice the anomaly is escalated, if the rung still holds it, with
//! whatever the rung holds by then (public observations only). It is tuned per reasoner setting
//! before any comparison (charter section 7) and a delay of zero is the arm as R3 built it. The
//! arm spends the reasoner budget on noise anomalies as readily as on incidents; when the budget
//! is spent the harness refuses further calls and nothing is charged.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "always_escalate";

/// The default delay after notice: none, the arm as R3 built it.
pub const DEFAULT_DELAY_NS: u64 = 0;

/// The rule: escalate each noticed anomaly once, `delay_ns` after it is noticed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Always {
    delay_ns: u64,
}

impl Always {
    /// A rule that escalates each anomaly `delay_ns` after it is noticed (0: when it is noticed).
    pub fn new(delay_ns: u64) -> Self {
        Self { delay_ns }
    }
}

impl EscalationRule for Always {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| {
                v.attempts == 0
                    && (self.delay_ns == 0 || now.0 >= v.noticed_at.0.saturating_add(self.delay_ns))
            })
            .map(|v| v.id)
            .collect()
    }
}
