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

/// The rule.
#[derive(Debug, Clone)]
pub struct Random {
    p: f64,
    rng: ChaCha8Rng,
    drawn: BTreeSet<u32>,
}

impl Random {
    /// A rule with probability `p` and a generator seeded from `seed`.
    pub fn new(p: f64, seed: [u8; 32]) -> Self {
        Self {
            p,
            rng: ChaCha8Rng::from_seed(seed),
            drawn: BTreeSet::new(),
        }
    }
}

impl EscalationRule for Random {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut chosen = Vec::new();
        for v in views {
            if !self.drawn.insert(v.id) {
                continue;
            }
            // The top 53 bits as a fraction in [0, 1): `p = 0` never passes, `p = 1` always does.
            let u = (self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            if u < self.p {
                chosen.push(v.id);
            }
        }
        chosen
    }
}
