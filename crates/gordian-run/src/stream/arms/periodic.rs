//! `periodic_escalation`: live anomalies are reviewed every `period_ns`.
//!
//! The charter's "does timing need to be state-dependent". On a global schedule (the first tick
//! one period after the stream starts), every noticed anomaly that is still live and has no call
//! in flight is escalated, with the context the shared rung holds at that tick. An anomaly that
//! retires between ticks is never escalated by this arm unless a tick found it live. The period
//! is a manifest parameter, tuned later per budget; the default is a placeholder that nothing
//! was tuned to.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "periodic_escalation";

/// The placeholder period: 10 s, about the time a decoy takes to show itself.
pub const DEFAULT_PERIOD_NS: u64 = 10_000_000_000;

/// The rule.
#[derive(Debug, Clone, Copy)]
pub struct Periodic {
    period_ns: u64,
    next_tick: u64,
}

impl Periodic {
    /// A rule that reviews every `period_ns` (at least 1).
    pub fn new(period_ns: u64) -> Self {
        let period_ns = period_ns.max(1);
        Self {
            period_ns,
            next_tick: period_ns,
        }
    }
}

impl EscalationRule for Periodic {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        if now.0 < self.next_tick {
            return Vec::new();
        }
        // The next tick after `now`, so a step that overshoots several ticks reviews once.
        self.next_tick = (now.0 / self.period_ns + 1).saturating_mul(self.period_ns);
        views
            .iter()
            .filter(|v| v.pending == 0)
            .map(|v| v.id)
            .collect()
    }
}
