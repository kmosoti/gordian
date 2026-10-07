//! The reservoir noticer's manifest parameters.

use serde::{Deserialize, Serialize};

/// The reservoir noticer, as a manifest writes it (`{"noticer": "reservoir", ...}`).
///
/// The four quantities of the brief that are tuned on the tuning seeds are `size`, `leak`,
/// `spectral_radius` and `threshold`. The rest are fixed by the module documentation before any
/// run and are written in every manifest so that a manifest says what ran.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservoirParams {
    /// The seed of the fixed random weights (recurrent, input and bias). Every segment of every
    /// arm that names the same seed and sizes draws the same network.
    pub seed: u64,
    /// The number of reservoir units per node.
    pub size: u32,
    /// The leak rate `a` in `(0, 1]`: a unit moves this share of the way to its new activation
    /// each tick. The state's memory is about `1 / a` ticks.
    pub leak: f64,
    /// The nominal spectral radius of the recurrent weights: entries are `+-radius / sqrt(fan_in)`
    /// on `fan_in` randomly chosen sources per unit, whose realized radius is close to this for
    /// the sizes in range (a test measures it).
    pub spectral_radius: f64,
    /// The scale of the input weights, which are uniform in `[-scale, scale]`.
    pub input_scale: f64,
    /// The residual threshold: a tick is hot at a node when the squared prediction residual,
    /// summed over the node's twelve readings, exceeds this (the readings are in the fixed
    /// units of the module documentation).
    pub threshold: f64,
    /// The tick length, nanoseconds. The criterion's is 100 ms.
    pub tick_ns: u64,
    /// Whether the readout is trained. Off: it stays at its initial weights, all zero (the
    /// learning-off control), and nothing is carried.
    pub learning: bool,
    /// Whether the readout is carried from one segment to the next. Off: every segment starts
    /// again from the initial readout.
    pub carry: bool,
    /// The key of the carried readout. Two learning arms of one manifest need their own.
    pub state_key: u64,
    /// The ridge of the prior on the readout: the inverse-covariance estimate starts at
    /// `I / ridge`.
    pub ridge: f64,
}

/// The smallest reservoir the arm builds.
pub const MIN_SIZE: u32 = 4;
/// The largest. Past this the readout's update alone exceeds the stream's compute limit at the
/// declared price (`noticing::ESN_OP_NS`): a size of 128 costs about 1.8 s of the 2 s a segment
/// is allowed, and the bill would refuse it.
pub const MAX_SIZE: u32 = 128;

impl ReservoirParams {
    /// The parameters as the module documentation fixes them before tuning: 32 units, leak 0.3,
    /// spectral radius 0.9, input scale 1, threshold 1.0, 100 ms ticks, learning on and carried,
    /// ridge 0.1. The four tuned quantities are placeholders until the tuning run replaces them.
    pub fn standard(seed: u64, state_key: u64) -> Self {
        Self {
            seed,
            size: 32,
            leak: 0.3,
            spectral_radius: 0.9,
            input_scale: 1.0,
            threshold: 1.0,
            tick_ns: 100_000_000,
            learning: true,
            carry: true,
            state_key,
            ridge: 0.1,
        }
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_SIZE..=MAX_SIZE).contains(&self.size) {
            return Err(format!(
                "noticer reservoir: size must be in {MIN_SIZE}..={MAX_SIZE}"
            ));
        }
        if !(self.leak.is_finite() && self.leak > 0.0 && self.leak <= 1.0) {
            return Err("noticer reservoir: leak must be in (0, 1]".to_owned());
        }
        if !(self.spectral_radius.is_finite()
            && self.spectral_radius > 0.0
            && self.spectral_radius <= 2.0)
        {
            return Err("noticer reservoir: spectral_radius must be in (0, 2]".to_owned());
        }
        if !(self.input_scale.is_finite() && self.input_scale > 0.0 && self.input_scale <= 10.0) {
            return Err("noticer reservoir: input_scale must be in (0, 10]".to_owned());
        }
        if !(self.threshold.is_finite() && self.threshold > 0.0) {
            return Err("noticer reservoir: threshold must be positive".to_owned());
        }
        if !(1_000_000..=10_000_000_000).contains(&self.tick_ns) {
            return Err("noticer reservoir: tick_ns must be 1 ms to 10 s".to_owned());
        }
        if !(self.ridge.is_finite() && self.ridge > 0.0) {
            return Err("noticer reservoir: ridge must be positive".to_owned());
        }
        Ok(())
    }
}
