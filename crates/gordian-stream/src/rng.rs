//! Seeded generation helpers: a ChaCha8 stream keyed by a list of words, and the three
//! transcendental functions the stream needs, computed with basic IEEE operations only.
//!
//! Why not `f64::exp` and `f64::ln`: the standard library forwards them to the platform's
//! math library, which is allowed to differ in the last bit between platforms. A single bit
//! can flip a comparison between a draw and a probability, and the stream's contract is that
//! generation and replay are pure functions of the seed and the parameters. Addition,
//! subtraction, multiplication and division are correctly rounded everywhere, and Rust does not
//! contract them into fused operations, so the series below return the same bits on every
//! target.

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// One step of splitmix64.
pub(crate) fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Domain tags, so that two purposes keyed by the same numbers never share a stream.
pub(crate) mod domain {
    /// The service graph and the regime changes.
    pub(crate) const WORLD: u64 = 1;
    /// The arrival process and tier mix.
    pub(crate) const ARRIVAL: u64 = 2;
    /// An incident's shape: kind, mode, partner, bits.
    pub(crate) const SHAPE: u64 = 3;
    /// An incident's phase-1 burst.
    pub(crate) const BURST: u64 = 4;
    /// An incident's heartbeat series.
    pub(crate) const PULSE: u64 = 5;
    /// An incident's phase-2 evidence.
    pub(crate) const PHASE2: u64 = 6;
    /// An incident's remaining draws: difficulty, deadline, recovery time, criticality.
    pub(crate) const MISC: u64 = 7;
    /// Background noise, one sub-stream per noise type.
    pub(crate) const NOISE: u64 = 8;
    /// The pool of free-form message ids.
    pub(crate) const POOL: u64 = 9;
    /// The simulated reasoner.
    pub(crate) const REASONER: u64 = 10;
}

/// A seeded generator with the few helpers the generator needs.
#[derive(Debug, Clone)]
pub(crate) struct Gen(ChaCha8Rng);

impl Gen {
    /// A generator keyed by `words`. The 32-byte ChaCha seed is built with splitmix64 here, not
    /// with `SeedableRng::seed_from_u64`, so that no library's seed expansion is a hidden input.
    /// Every word is mixed in sequence, so `[a, b]` and `[b, a]` give different streams.
    pub(crate) fn keyed(words: &[u64]) -> Self {
        let mut state = 0x243F_6A88_85A3_08D3u64;
        for w in words {
            let mut s = state ^ *w;
            state = splitmix(&mut s);
        }
        let mut bytes = [0u8; 32];
        for chunk in bytes.chunks_mut(8) {
            chunk.copy_from_slice(&splitmix(&mut state).to_le_bytes());
        }
        Gen(ChaCha8Rng::from_seed(bytes))
    }

    pub(crate) fn u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// Uniform in `0..n` by rejection sampling. `n` must be positive.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.u64();
            if x >= threshold {
                return x % n;
            }
        }
    }

    /// Uniform in `lo..=hi`.
    pub(crate) fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    /// True with probability `permille / 1000`.
    pub(crate) fn permille(&mut self, permille: u32) -> bool {
        self.below(1000) < permille as u64
    }

    /// Uniform in `[0, 1)` with 53 bits of resolution.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `(0, 1]`, for taking a logarithm.
    pub(crate) fn unit_open(&mut self) -> f64 {
        ((self.u64() >> 11) + 1) as f64 / (1u64 << 53) as f64
    }

    /// An exponential gap with the given mean, in nanoseconds.
    pub(crate) fn exp_gap_ns(&mut self, mean_ns: u64) -> u64 {
        let u = self.unit_open();
        let gap = -det_ln(u) * mean_ns as f64;
        if gap >= u64::MAX as f64 {
            u64::MAX
        } else {
            gap as u64
        }
    }
}

const LN2: f64 = std::f64::consts::LN_2;

/// Natural logarithm of `x` in `(0, inf)`, from basic operations only.
///
/// `x = m * 2^e` with `m` in `[sqrt(1/2), sqrt(2))`, then `ln m = 2 atanh((m-1)/(m+1))` by its
/// power series, whose ratio is at most `(0.1716)^2` here, so 24 terms are far below one ulp.
pub(crate) fn det_ln(x: f64) -> f64 {
    debug_assert!(x > 0.0 && x.is_finite());
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | (1023u64 << 52));
    if m > std::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let z = (m - 1.0) / (m + 1.0);
    let z2 = z * z;
    let mut term = z;
    let mut sum = 0.0;
    let mut k = 1.0;
    for _ in 0..24 {
        sum += term / k;
        term *= z2;
        k += 2.0;
    }
    2.0 * sum + e as f64 * LN2
}

/// `exp(x)` from basic operations only. `x` is clamped to `[-700, 700]`.
pub(crate) fn det_exp(x: f64) -> f64 {
    let x = x.clamp(-700.0, 700.0);
    let k = (x / LN2).round();
    let r = x - k * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for i in 1..30 {
        term *= r / i as f64;
        sum += term;
    }
    let scale = f64::from_bits(((k as i64 + 1023) as u64) << 52);
    sum * scale
}

/// The logistic function, `1 / (1 + exp(-x))`.
pub(crate) fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + det_exp(-x.clamp(-40.0, 40.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_and_exp_agree_with_std_to_a_few_ulps() {
        let mut x = 1e-12;
        while x < 1e12 {
            let a = det_ln(x);
            let b = x.ln();
            assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "ln {x}: {a} {b}");
            x *= 1.37;
        }
        let mut y = -30.0;
        while y < 30.0 {
            let a = det_exp(y);
            let b = y.exp();
            assert!(((a - b) / b).abs() < 1e-13, "exp {y}: {a} {b}");
            y += 0.173;
        }
    }

    #[test]
    fn sigmoid_is_symmetric_and_bounded() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-15);
        for x in [-9.0, -2.0, -0.3, 0.7, 4.0] {
            assert!((sigmoid(x) + sigmoid(-x) - 1.0).abs() < 1e-14);
        }
        assert!(sigmoid(1000.0) <= 1.0 && sigmoid(-1000.0) >= 0.0);
    }

    #[test]
    fn keyed_streams_depend_on_every_word_and_their_order() {
        let a = Gen::keyed(&[1, 2, 3]).u64();
        assert_eq!(a, Gen::keyed(&[1, 2, 3]).u64());
        assert_ne!(a, Gen::keyed(&[1, 2, 4]).u64());
        assert_ne!(a, Gen::keyed(&[3, 2, 1]).u64());
        assert_ne!(a, Gen::keyed(&[1, 2]).u64());
    }

    #[test]
    fn exponential_gap_has_the_requested_mean() {
        let mut g = Gen::keyed(&[9]);
        let n = 200_000u64;
        let total: u64 = (0..n).map(|_| g.exp_gap_ns(1_000_000)).sum();
        let mean = total as f64 / n as f64;
        assert!((mean - 1_000_000.0).abs() < 10_000.0, "mean {mean}");
    }
}
