//! The learned noticer's manifest parameters, and how a learned state becomes the medium's.

use crate::stream::arms::medium::{CoincidenceForm, MediumParams};
use serde::{Deserialize, Serialize};

/// The longest coincidence window the learner will set, nanoseconds.
pub const MAX_WINDOW_NS: u64 = 1_000_000_000;
/// The shortest, nanoseconds (the ordered coincidence's window is a whole number of
/// microseconds, at least one).
pub const MIN_WINDOW_NS: u64 = 1_000;

/// The learned noticer, as a manifest writes it (`{"noticer": "learned", ...}`).
///
/// `structure` is M2's graph: every constant the learner does not touch, as M2 froze it at 100 ms.
/// Four quantities of it are replaced by learned values and are overwritten: the burst window
/// (`burst_window_ns`), the three-kind window (`burst3_window_ns`, tied to the burst window), the
/// burst lookback (`burst_lookback_ns`, derived from the window) and the ramp threshold
/// (`ramp_threshold`). The values they have in `structure` are what the hand design chose; the
/// learned arm starts from the priors below and never reads them (a test pins that).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnedParams {
    /// The graph's structure and the constants that are not learned.
    pub structure: MediumParams,
    /// Whether the plasticity adapter updates the parameters. Off: the arm stays at its priors
    /// (the learning-off control).
    pub learning: bool,
    /// Whether what is learned is carried from one segment to the next. Off: every segment
    /// starts again from the priors (the within-segment ablation).
    pub carry: bool,
    /// The key of the carried state. Two arms with the same key share it, so every learning arm
    /// of a manifest needs its own.
    pub state_key: u64,
    /// The share of the way (in log space for the window) the parameter moves toward the value
    /// the evidence supports at each boundary, in `(0, 1]`.
    pub step: f32,
    /// The least share of a bin's observed events that must be beyond chance for the bin to be
    /// kept, in `(0, 1)`: the marginal precision. 0.5 is "more likely than not".
    pub min_precision: f32,
    /// The prior coincidence window, nanoseconds; 0 means the shared rung's `burst_ns`, the
    /// public constant of the status quo.
    pub prior_window_ns: u64,
    /// The prior ramp threshold, in readings: two, the least a trend can be made of.
    pub prior_ramp_threshold: f32,
}

impl LearnedParams {
    /// The parameters of the learned arm as fixed before any run: M2's frozen 100 ms graph as
    /// the structure, learning on, carried across segments, a step of 1/4, marginal precision 1/2,
    /// the rung's `burst_ns` as the prior window and two readings as the prior ramp threshold.
    pub fn with_structure(structure: MediumParams, state_key: u64) -> Self {
        Self {
            structure,
            learning: true,
            carry: true,
            state_key,
            step: 0.25,
            min_precision: 0.5,
            prior_window_ns: 0,
            prior_ramp_threshold: 2.0,
        }
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        let s = &self.structure;
        if !s.burst || s.coincidence == CoincidenceForm::Off {
            return Err("noticer learned: the structure needs the burst cells".to_owned());
        }
        if !s.ramp {
            return Err("noticer learned: the structure needs the ramp cells".to_owned());
        }
        if !(self.step > 0.0 && self.step <= 1.0) {
            return Err("noticer learned: step must be in (0, 1]".to_owned());
        }
        if !(self.min_precision > 0.0 && self.min_precision < 1.0) {
            return Err("noticer learned: min_precision must be in (0, 1)".to_owned());
        }
        if self.prior_window_ns != 0
            && !(MIN_WINDOW_NS..=MAX_WINDOW_NS).contains(&self.prior_window_ns)
        {
            return Err("noticer learned: prior_window_ns out of range".to_owned());
        }
        if !(self.prior_ramp_threshold.is_finite() && self.prior_ramp_threshold > 1.0) {
            return Err("noticer learned: prior_ramp_threshold must be above 1".to_owned());
        }
        self.medium_params(self.prior_window(400_000_000), self.prior_ramp_threshold)
            .validate()
    }

    /// The prior window, nanoseconds, given the rung's `burst_ns`.
    pub fn prior_window(&self, rung_burst_ns: u64) -> u64 {
        let w = if self.prior_window_ns == 0 {
            rung_burst_ns
        } else {
            self.prior_window_ns
        };
        round_window(w as f64)
    }

    /// The medium's parameters for a learned window `window_ns` and ramp threshold: the burst
    /// window and the three-kind window are the window, rounded to whole microseconds; the burst
    /// lookback is the window rounded down to whole ticks (the cluster's own extent: the anchor
    /// is the earliest event of the support, and the support should reach no further back than
    /// the cluster); the ramp threshold is the threshold.
    pub fn medium_params(&self, window_ns: u64, ramp_threshold: f32) -> MediumParams {
        let mut p = self.structure;
        let w = window_ns.clamp(MIN_WINDOW_NS, MAX_WINDOW_NS);
        let w = w - w % 1_000;
        p.burst_window_ns = w;
        if p.burst3_window_ns > 0 {
            p.burst3_window_ns = w;
        }
        p.burst_lookback_ns = (w / p.tick_ns) * p.tick_ns;
        p.ramp_threshold = ramp_threshold;
        p
    }
}

/// A window in nanoseconds as a whole number of microseconds in range.
pub fn round_window(ns: f64) -> u64 {
    let us = (ns / 1_000.0).round().max(1.0) as u64;
    (us * 1_000).clamp(MIN_WINDOW_NS, MAX_WINDOW_NS)
}
