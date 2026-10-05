//! `random_escalation`: each noticed anomaly is escalated with probability `p`.
//!
//! The charter's "random escalation at matched cost": whether selection matters, rather than
//! merely escalating less. `p` is a manifest parameter and not a match; the headroom check tunes
//! it so that this arm's total cost equals a target arm's. The draw is made once per noticed
//! anomaly, the step it is noticed, from a ChaCha8 stream seeded from the stream's seed and the
//! arm's name by the caller (`random_matched::rng_seed`): the arm is handed 32 bytes and never
//! sees the seed. Exactly one `u64` is drawn per noticed anomaly, in order, whether or not it is
//! escalated and whatever happened to earlier calls, so the stream of draws does not depend on
//! outcomes. An escalated anomaly gets the same context as in every other arm.
//!
//! `delay_ns` is a timing knob: an anomaly the draw selected is escalated `delay_ns` after it was
//! noticed (default 0: at notice, the arm as R3 built it), if the rung still holds it then. The
//! draw itself stays at notice, so the draws, and which anomalies are selected, do not depend on
//! the delay.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use std::collections::BTreeSet;

/// The id this arm is registered under.
pub const ID: &str = "random_escalation";

/// The placeholder probability.
pub const DEFAULT_P: f64 = 0.5;

/// The default delay after notice: none, the arm as R3 built it.
pub const DEFAULT_DELAY_NS: u64 = 0;

/// The rule.
#[derive(Debug, Clone)]
pub struct Random {
    p: f64,
    delay_ns: u64,
    rng: ChaCha8Rng,
    drawn: BTreeSet<u32>,
    /// Selected by the draw and not yet escalated.
    selected: BTreeSet<u32>,
}

impl Random {
    /// A rule with probability `p` and a generator seeded from `seed`.
    pub fn new(p: f64, seed: [u8; 32]) -> Self {
        Self::with_delay(p, 0, seed)
    }

    /// A rule with probability `p` that escalates a selected anomaly `delay_ns` after it is
    /// noticed, and a generator seeded from `seed`.
    pub fn with_delay(p: f64, delay_ns: u64, seed: [u8; 32]) -> Self {
        Self {
            p,
            delay_ns,
            rng: ChaCha8Rng::from_seed(seed),
            drawn: BTreeSet::new(),
            selected: BTreeSet::new(),
        }
    }
}

impl EscalationRule for Random {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        for v in views {
            if !self.drawn.insert(v.id) {
                continue;
            }
            // The top 53 bits as a fraction in [0, 1): `p = 0` never passes, `p = 1` always does.
            let u = (self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            if u < self.p {
                self.selected.insert(v.id);
            }
        }
        // Escalate the selected anomalies that are due, in the order the rung lists them. With no
        // delay that is exactly the ones drawn at this step.
        let mut chosen = Vec::new();
        for v in views {
            let due = self.delay_ns == 0 || now.0 >= v.noticed_at.0.saturating_add(self.delay_ns);
            if due && self.selected.remove(&v.id) {
                chosen.push(v.id);
            }
        }
        chosen
    }
}
