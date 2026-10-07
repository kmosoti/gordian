//! `public_threshold`: escalate an anomaly once, at R5's delay after it is noticed, if the cheap
//! rung has not resolved it by public information alone (work item B4).
//!
//! This is the non-privileged counterpart of the selection oracle for one of the two selectors B4
//! asks for: *"a public threshold rule: escalate when the rung's conclusion is contradictory or
//! silent for `t` seconds after notice"*. It selects, among the anomalies any noticer opens, the
//! ones the reasoner is asked about. It does not change the noticer, the context (the rung's) or
//! the instant of the question (`delay_ns` after notice, R5's 16 s, the instant the selection
//! oracle asks), so the only thing that differs from `oracle_selection` is which anomalies are
//! chosen. Status: built, with the readings below stated before any tuning; its parameter `t` is
//! tuned on seeds 10000-10099 (`scripts/b4_*.py`).
//!
//! # Readings of the brief's words (fixed before any run)
//!
//! - **"The rung's conclusion."** What the shared cheap rung has done with the anomaly: it has
//!   *declared* for it ([`AnomalyView::cheap_declared`]) or it has not, and the public consistency
//!   checker ([`AnomalyView::contradicted_since`], which the rung keeps current for a rule that asks
//!   by [`EscalationRule::monitors`], as `contradiction_escalation` does) finds a hypothesis
//!   consistent with the evidence attached to the anomaly, or none. Nothing else is read: not the
//!   score, not the digest, not the evidence count, no hidden state.
//! - **"Contradictory."** The checker has found no consistent hypothesis in every check since an
//!   instant at least `persist_ns` ago (`contradicted_since <= now - persist_ns`): the same
//!   reading `contradiction_escalation` has of "contradictory for a while", with the same
//!   caveat that an empty set can be about a damaged window and not about the world.
//! - **"Silent."** The rung has made no declaration about the anomaly at all (it has not concluded,
//!   or concluded to wait or to abstain), and `persist_ns` has passed since the anomaly was
//!   noticed. The alternative reading, that the *anomaly* is silent (no new abnormal observation
//!   for `t` seconds, as a resolved decoy is), is **not** adopted: the rung already retires an
//!   anomaly that has been quiet for 6 s, so that reading would escalate exactly the anomalies the
//!   rung is about to retire; it is listed so that a reader who meant it can say so.
//! - **"For `t` seconds after notice."** `persist_ns` is `t`. The rule asks at the first step at
//!   or after `noticed_at + max(delay_ns, persist_ns)` at which the condition holds, and only
//!   while the anomaly is live (an anomaly the rung has retired is gone, which is the same
//!   retirement every public arm is subject to). It asks once per anomaly (`attempts == 0`): a
//!   second call about the same anomaly is never made, and an answer in flight is not repeated.
//! - **"Or."** Either condition suffices. An anomaly the rung has declared for and the checker
//!   finds consistent is not escalated.
//! - **R5's delay.** `delay_ns` is a manifest parameter and is not tuned: the table fixes it at
//!   16 s, the delay R5 fixed for the selection oracle at the primary setting, so that selection
//!   is the only variable between this arm and the oracle.
//!
//! # What it cannot do, by construction
//!
//! It has no way to tell a hard incident from a plain one except through the checker's verdict,
//! which R5 measured as non-selective (the checker contradicts 97% of plain anomalies at some point
//! in their life), and it has no way to tell a decoy or a late plain notice from an incident. The
//! table of work item B4 reports what that costs on every noticer's anomalies; it is a baseline,
//! not a design.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "public_threshold";

/// The rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct PublicThreshold {
    delay_ns: u64,
    persist_ns: u64,
}

impl PublicThreshold {
    /// A rule that asks `delay_ns` after notice about an anomaly the rung has been unable to
    /// resolve for `persist_ns`.
    pub fn new(delay_ns: u64, persist_ns: u64) -> Self {
        Self {
            delay_ns,
            persist_ns,
        }
    }

    /// Whether the rung's conclusion about `view` is contradictory or silent as of `now`, for at
    /// least `persist_ns` (the module documentation's two readings).
    pub fn unresolved(&self, view: &AnomalyView, now: Instant) -> bool {
        let contradictory = view
            .contradicted_since
            .is_some_and(|since| now.0 >= since.0.saturating_add(self.persist_ns));
        let silent =
            !view.cheap_declared && now.0 >= view.noticed_at.0.saturating_add(self.persist_ns);
        contradictory || silent
    }
}

impl EscalationRule for PublicThreshold {
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
                    && self.unresolved(v, now)
            })
            .map(|v| v.id)
            .collect()
    }
}
