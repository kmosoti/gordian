//! `never_escalate`: the cheap rung only.
//!
//! The charter's first baseline: whether the expensive reasoner is needed at all. The first
//! world's components and the shared rule, adapted to the stream's observations by the shared
//! rung, and nothing else. It declares what the cheap rung concludes, which for a plain incident
//! is the answer and for a hard one is whatever the first world's public rules make of evidence
//! they do not cover.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;

/// The id this arm is registered under.
pub const ID: &str = "never_escalate";

/// The rule: escalate nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct Never;

impl EscalationRule for Never {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }
}
