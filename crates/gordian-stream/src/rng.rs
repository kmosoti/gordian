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

/// Horner's rule: the polynomial with `coeffs` from the highest power down, at `x`.
fn horner(coeffs: &[f64], x: f64) -> f64 {
    coeffs.iter().fold(0.0, |acc, c| acc * x + c)
}

/// The inverse of the standard normal distribution function, `z` with `Phi(z) = p`, for `p` in
/// `(0, 1)`. Wichura's algorithm AS 241 (PPND16), about 1e-16 relative accuracy, using only basic
/// operations, `det_ln` and `sqrt` (which IEEE makes correctly rounded). `p` at or below 0 gives
/// negative infinity and at or above 1 positive infinity.
// The coefficients are the published ones of AS 241; a few carry one digit more than an f64 holds.
#[allow(clippy::excessive_precision)]
pub(crate) fn det_norm_inv(p: f64) -> f64 {
    const A: [f64; 8] = [
        2.509_080_928_730_122_7e3,
        3.343_057_558_358_812_8e4,
        6.726_577_092_700_87e4,
        4.592_195_393_154_987e4,
        1.373_169_376_550_946e4,
        1.971_590_950_306_551_4e3,
        1.331_416_678_917_843_8e2,
        3.387_132_872_796_366_5,
    ];
    const B: [f64; 8] = [
        5.226_495_278_852_854_6e3,
        2.872_908_573_572_194_3e4,
        3.930_789_580_009_271e4,
        2.121_379_430_158_659_6e4,
        5.394_196_021_424_751e3,
        6.871_870_074_920_579e2,
        4.231_333_070_160_091e1,
        1.0,
    ];
    const C: [f64; 8] = [
        7.745_450_142_783_414e-4,
        2.272_384_498_926_918_5e-2,
        2.417_807_251_774_506e-1,
        1.270_458_252_452_368_4,
        3.647_848_324_763_204_6,
        5.769_497_221_460_691,
        4.630_337_846_156_546,
        1.423_437_110_749_683_5,
    ];
    const D: [f64; 8] = [
        1.050_750_071_644_416_9e-9,
        5.475_938_084_995_345e-4,
        1.519_866_656_361_645_7e-2,
        1.481_039_764_274_800_8e-1,
        6.897_673_349_851e-1,
        1.676_384_830_183_803_8,
        2.053_191_626_637_759,
        1.0,
    ];
    const E: [f64; 8] = [
        2.010_334_399_292_288e-7,
        2.711_555_568_743_487_6e-5,
        1.242_660_947_388_078_4e-3,
        2.653_218_952_657_612_4e-2,
        2.965_605_718_285_048_7e-1,
        1.784_826_539_917_291_3,
        5.463_784_911_164_114,
        6.657_904_643_501_104,
    ];
    const F: [f64; 8] = [
        2.044_263_103_389_939_7e-15,
        1.421_511_758_316_445_9e-7,
        1.846_318_317_510_054_7e-5,
        7.868_691_311_456_133e-4,
        1.487_536_129_085_061_5e-2,
        1.369_298_809_227_358e-1,
        5.998_322_065_558_88e-1,
        1.0,
    ];
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180625 - q * q;
        return q * horner(&A, r) / horner(&B, r);
    }
    let tail = if q < 0.0 { p } else { 1.0 - p };
    let r = (-det_ln(tail)).sqrt();
    let val = if r <= 5.0 {
        let r = r - 1.6;
        horner(&C, r) / horner(&D, r)
    } else {
        let r = r - 5.0;
        horner(&E, r) / horner(&F, r)
    };
    if q < 0.0 { -val } else { val }
}

#[cfg(test)]
pub(crate) mod tests {
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
    fn the_series_return_pinned_bits() {
        // Bit-exact values: a platform whose basic operations differed would fail here, and
        // generation would no longer replay across platforms.
        assert_eq!(det_exp(1.0).to_bits(), 0x4005bf0a8b145768);
        assert_eq!(det_exp(-3.7).to_bits(), 0x3f99511fc6871045);
        assert_eq!(det_ln(0.3).to_bits(), 0xbff34378fcbda720);
        assert_eq!(det_ln(1234.5).to_bits(), 0x401c79436f818745);
    }

    /// The standard normal distribution function, from the Taylor series of erf, which is
    /// accurate to about 1e-10 on the range used here. Independent of the crate's inverse.
    pub(crate) fn phi(x: f64) -> f64 {
        // Beyond six standard deviations the series loses its precision and the answer is 0 or 1
        // to better than 1e-9.
        if x > 6.0 {
            return 1.0;
        }
        if x < -6.0 {
            return 0.0;
        }
        let t = x / std::f64::consts::SQRT_2;
        let (mut term, mut sum) = (t, t);
        for n in 1..200 {
            term *= -t * t / n as f64;
            sum += term / (2 * n + 1) as f64;
        }
        0.5 + 0.5 * (2.0 / std::f64::consts::PI.sqrt()) * sum
    }

    #[test]
    fn the_inverse_normal_inverts_the_normal() {
        let mut p = 1e-7;
        while p < 1.0 - 1e-7 {
            let z = det_norm_inv(p);
            assert!(
                (phi(z) - p).abs() < 1e-10 + 1e-9 * p,
                "p {p}: z {z} -> {}",
                phi(z)
            );
            p = if p < 0.5 {
                p * 1.9
            } else {
                1.0 - (1.0 - p) / 1.9
            };
        }
        assert!(det_norm_inv(0.5).abs() < 1e-15);
        assert!((det_norm_inv(0.975) - 1.959963984540054).abs() < 1e-12);
        assert!((det_norm_inv(0.025) + 1.959963984540054).abs() < 1e-12);
        assert_eq!(det_norm_inv(0.0), f64::NEG_INFINITY);
        assert_eq!(det_norm_inv(1.0), f64::INFINITY);
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
