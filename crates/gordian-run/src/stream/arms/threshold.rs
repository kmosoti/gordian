//! `threshold_score`: escalate above a scalar anomaly score, wait and see below.
//!
//! The charter's "whether a single scalar statistic suffices". The score is the rung's: the
//! z-score of an anomaly's abnormal count over the last score window against a baseline rate
//! learned from the stream so far, from public observations only. The arm escalates an anomaly
//! once, the first step its score is at or above `tau` within `wait_ns` of the anomaly being
//! noticed; an anomaly still below `tau` when the wait ends is never escalated by this arm. `tau`
//! and `wait_ns` are manifest parameters, tuned later per budget; the defaults are placeholders
//! that nothing was tuned to.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "threshold_score";

/// The placeholder threshold, above the rung's notice score of 2.
pub const DEFAULT_TAU: f64 = 3.5;

/// The placeholder wait: 6 s, the rung's quiet time.
pub const DEFAULT_WAIT_NS: u64 = 6_000_000_000;

/// The rule.
#[derive(Debug, Clone, Copy)]
pub struct Threshold {
    tau: f64,
    wait_ns: u64,
}

impl Threshold {
    /// A rule with threshold `tau` and wait `wait_ns`.
    pub fn new(tau: f64, wait_ns: u64) -> Self {
        Self { tau, wait_ns }
    }
}

impl EscalationRule for Threshold {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| {
                v.attempts == 0
                    && v.score >= self.tau
                    && now.0 <= v.noticed_at.0.saturating_add(self.wait_ns)
            })
            .map(|v| v.id)
            .collect()
    }
}
