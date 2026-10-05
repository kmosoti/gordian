//! `contradiction_escalation`: escalate an anomaly once the cheap rung's evidence about it has
//! contradicted every hypothesis for a while.
//!
//! The charter's cascade (learning-to-defer) row, built from public information only: the cheap
//! rung asks the first world's consistency checker which hypotheses no attached observation
//! contradicts, and when the answer is *none*, the evidence breaks the public rules. That is the
//! first world's own word for "contradictory", and it says nothing about what the hypothesis
//! should be. Whether the empty set is a statement about the world or about a damaged window is not
//! something a cheap rung can tell (`gordian-components/src/verifier.rs`), so the rule asks for
//! the contradiction to persist.
//!
//! The rule escalates an anomaly once, when both hold:
//!
//! - the checker has found no consistent hypothesis in every check for at least `persist_ns`
//!   ([`AnomalyView::contradicted_since`], kept by the rung when the rule asks:
//!   [`EscalationRule::monitors`]); a check that finds a consistent hypothesis restarts the
//!   count; and
//! - `delay_ns` has passed since the anomaly was noticed (the same meaning as `always_escalate`'s
//!   delay: more evidence arrives, and the context is built from what the rung holds then).
//!
//! `persist_ns = 0` escalates at the first contradictory check after the delay. The context is the
//! rung's context ([`super::rung::Rung::context`]), the same function every arm uses, and the
//! declaring procedure is the shared one. The only thing this rule adds to the rung is that the
//! rung keeps the verifier's verdict current for it, charged through the meter like any component
//! run. The rule reads no hidden state: its inputs are the [`AnomalyView`] and the clock.
//!
//! What it cannot do, by construction: escalate a hard incident the checker never contradicts (a
//! mimic whose phase 2 is not in the anomaly's evidence, or the slow leak, which the checker reads
//! as plain resource exhaustion), and it escalates any plain incident or decoy whose attached
//! evidence the checker finds contradictory (another incident's observations mis-attached).

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "contradiction_escalation";

/// The default delay after notice: none.
pub const DEFAULT_DELAY_NS: u64 = 0;

/// The default persistence: none.
pub const DEFAULT_PERSIST_NS: u64 = 0;

/// The rule: escalate an anomaly once, when the consistency checker has found no hypothesis for
/// `persist_ns` and `delay_ns` have passed since the anomaly was noticed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Contradiction {
    delay_ns: u64,
    persist_ns: u64,
}

impl Contradiction {
    /// A rule with the given delay after notice and persistence of the contradiction.
    pub fn new(delay_ns: u64, persist_ns: u64) -> Self {
        Self {
            delay_ns,
            persist_ns,
        }
    }
}

impl EscalationRule for Contradiction {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn monitors(&self) -> bool {
        true
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| {
                v.attempts == 0
                    && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)
                    && v.contradicted_since
                        .is_some_and(|since| now.0 >= since.0.saturating_add(self.persist_ns))
            })
            .map(|v| v.id)
            .collect()
    }
}
