//! The seeded generator that draws the reservoir's fixed random weights.
//!
//! A SplitMix64 sequence keyed by a list of words, in basic integer operations only (wrapping
//! multiplication, xor, shifts), following the pattern of `gordian-stream`'s generator: no library
//! seed expansion, no platform math, no clock. The weights are a pure function of the seed in the
//! manifest and the sizes, so a replay draws the same network. This is the whole of the randomness
//! in the arm, and it is spent once, at construction.

/// One step of SplitMix64.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seeded generator with the few helpers the weights need.
#[derive(Debug, Clone)]
pub struct Draws {
    state: u64,
}

impl Draws {
    /// A generator keyed by `words`. Every word is mixed in sequence, so `[a, b]` and `[b, a]`
    /// give different sequences.
    pub fn keyed(words: &[u64]) -> Self {
        let mut state = 0x5245_5345_5256_4F49u64;
        for w in words {
            let mut s = state ^ *w;
            state = splitmix(&mut s);
        }
        Self { state }
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        splitmix(&mut self.state)
    }

    /// Uniform in `[0, 1)` with 53 bits of resolution.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `[-1, 1)`.
    pub fn signed(&mut self) -> f64 {
        2.0 * self.unit() - 1.0
    }

    /// Uniform in `0..n` by rejection sampling. `n` must be positive.
    pub fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.next_u64();
            if x >= threshold {
                return x % n;
            }
        }
    }

    /// `true` or `false` with equal probability.
    pub fn coin(&mut self) -> bool {
        self.next_u64() >> 63 == 1
    }
}
