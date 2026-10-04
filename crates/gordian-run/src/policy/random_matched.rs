//! `random_matched`: each component is selected independently with probability `p` at each step.
//!
//! The charter's "random activation at matched compute" baseline: it asks whether selection
//! matters, rather than merely doing less. `p` is in the manifest:
//!
//! ```json
//! {"policy": "random_matched", "p": 0.3}
//! ```
//!
//! `p` is a parameter, not a matched quantity. Exploration run B1 tunes it so that the *measured*
//! compute of this arm matches a target arm's (plan section 5, A4: measured cost is the primary
//! cost); this module only runs whatever `p` it is given. The default, 0.5, is a placeholder
//! that nothing was tuned to.
//!
//! # Randomness
//!
//! A ChaCha8 generator seeded from the episode seed and the arm name ([`rng_seed`]), so a run is
//! reproducible and two arms of an experiment do not share a stream. The seed is derived outside
//! the policy; the policy is handed 32 bytes and never sees the episode seed. Each step draws
//! exactly one `u64` per component, in id order, whether or not the draw selects it, so the
//! stream a step consumes does not depend on earlier outcomes. `p = 0` never selects and `p = 1`
//! always does.
//!
//! # Cost
//!
//! The draws are four generator calls, a few nanoseconds each; the selector declares none of it
//! (`zero_cost`), and the measured scheduling time in `measured.csv` includes it. The shared
//! decision rule's cost is added by [`super::Arm`] as for every arm.

use super::{PolicyId, Selector, zero_cost};
use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// The id this policy is registered under.
pub const ID: &str = "random_matched";

/// The components the arm chooses among, in the order the draws are made.
const COMPONENTS: [ComponentId; 4] = [HEURISTIC_ID, ESTIMATOR_ID, MEMORY_ID, VERIFIER_ID];

/// The configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// Probability that a component is selected at a step, in `[0, 1]`.
    pub p: f64,
}

impl Default for Config {
    /// The placeholder 0.5.
    fn default() -> Self {
        Self { p: 0.5 }
    }
}

impl Config {
    /// Check the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.p.is_finite() && (0.0..=1.0).contains(&self.p) {
            Ok(())
        } else {
            Err(format!("random_matched needs p in [0, 1], got {}", self.p))
        }
    }
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The generator seed for episode `episode_seed` of arm `arm`: the arm name folded with FNV-1a,
/// combined with the seed, and expanded to 32 bytes with splitmix64 (not with a library's seed
/// expansion, so that no dependency's choice is a hidden input).
pub fn rng_seed(episode_seed: u64, arm: &str) -> [u8; 32] {
    let name = arm.bytes().fold(0xCBF2_9CE4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
    });
    let mut from_seed = episode_seed;
    let mut state = splitmix64(&mut from_seed) ^ name.rotate_left(17);
    let mut bytes = [0u8; 32];
    for chunk in bytes.as_chunks_mut::<8>().0 {
        *chunk = splitmix64(&mut state).to_le_bytes();
    }
    bytes
}

/// The selector.
#[derive(Debug, Clone)]
pub struct RandomSubset {
    p: f64,
    rng: ChaCha8Rng,
}

impl RandomSubset {
    /// A selector with probability `p` and a generator seeded from `seed`.
    pub fn new(p: f64, seed: [u8; 32]) -> Self {
        Self {
            p,
            rng: ChaCha8Rng::from_seed(seed),
        }
    }
}

impl Selector for RandomSubset {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, _bill: &Bill) -> Vec<ComponentId> {
        let mut chosen = Vec::new();
        for id in COMPONENTS {
            // The top 53 bits as a fraction in [0, 1): `p = 0` never passes, `p = 1` always does.
            let u = (self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            if u < self.p {
                chosen.push(id);
            }
        }
        chosen
    }
}
