//! The learned noticer (work item L1, Lab 3): M2's medium graph with three of its tuned constants
//! replaced by values adjusted online from the public history of the run.
//!
//! This is the first reading of the charter's aim proxy 2 (improvement per unit experience). The
//! rule below was written before any run of the arm; `experiments/exploration/l1-learned-noticer.md`
//! records every change made after one.
//!
//! # What is learned, and what is not
//!
//! The graph is M2's frozen 100 ms graph (`structure` in [`LearnedParams`]), unchanged except for
//! these constants, which are parameters of the learner:
//!
//! | Quantity | Where it is in the graph | Prior |
//! |---|---|---|
//! | the coincidence window `w` | the window of the burst cells (two kinds at one service) and of the three-kind cells, one value | the rung's `burst_ns`, 400 ms: the status quo's own public constant for "the span of a burst" |
//! | the emitter's lookback | the burst path's lookback, **derived** from `w` (see below) | the window's prior rounded down to ticks, 400 ms |
//! | the ramp threshold `θ` | the threshold of every ramp integrator, in readings | 2: the least a trend can be made of |
//!
//! The onset integrator of the brief's list is switched off in M2's frozen 100 ms graph, so it has
//! no constants to replace; nothing stands in for them. Everything else (the confirmation gate and
//! its hold and delay, the refractory period, the ramp integrator's time constant, jump band and
//! penalty and lookback, the retiring latch's hold) is M2's frozen value and is not learned.
//!
//! # What the learner may read
//!
//! Delivered observations: the service, kind, value and instant of each, and the public rules'
//! verdict on it (abnormal or benign); the rung's public constants in the manifest (`burst_ns`,
//! `prior_mhz`, `prior_ns`); and what it has itself computed from these. No tier, label, incident,
//! deadline, reasoner answer or notice score. Nothing the evaluator measures is an input: whether
//! a notice was right is never known to it.
//!
//! # The update rule
//!
//! The plasticity adapter runs at each boundary of the 10 s rhythm. At a boundary it takes in the
//! cycle's observations and then moves each parameter toward the value the accumulated evidence
//! supports. The evidence is a comparison of what was seen against what independence would give.
//!
//! **Kinds.** An abnormal observation has one of four kinds, the four inputs of the burst cell:
//! error rate, latency, a message, and the rest (the other counters and snapshots).
//!
//! **The window.** For each abnormal observation of kind `k` at service `n` at time `t`:
//!
//! 1. *Observed.* Find the nearest earlier abnormal observation at `n` of a kind other than `k`,
//!    within 1 s, at gap `g`. For each bin edge `e` in 1, 2, 3, 5, 8, 12, 20, 30, 50, 75, 100,
//!    150, 200, 300, 400, 600, 800, 1000 ms with `g <= e`, add 1 to `obs[e]`.
//! 2. *Expected.* Estimate the rate `r(n, k')` of abnormal observations of each kind `k'` at `n` as
//!    `(c + T0 * m) / (t + T0)`, where `c` is the count of that kind at `n` earlier in this
//!    segment, `m` the pooled rate per service of that kind over everything seen (the rung's
//!    `prior_mhz` split over the four kinds before anything has been seen), and `T0` the rung's
//!    `prior_ns` (30 s). With `R = sum of r(n, k') over k' != k`, add `1 - exp(-e * R)` to
//!    `nul[e]` for every bin edge. This is the chance that some other-kind observation at `n`
//!    falls in the `e` before `t` if the kinds arrived independently at those rates.
//! 3. *Estimate.* Take the bins as bands `(e_{i-1}, e_i]`: `dobs`, `dnul` are the increments of
//!    `obs`, `nul` across the band. A band is *chance-dominated* if `dobs + dnul >= 20` (enough
//!    to say) and `dnul > (1 - p) * dobs`, where `p` is `min_precision` (1/2): fewer than `p` of
//!    the band's events are beyond chance. The estimate `w*` is the upper edge of the last band
//!    before the first chance-dominated one (the first edge, 1 ms, if the first band is). No
//!    estimate before 200 events with a predecessor within 1 s have been seen.
//! 4. *Move.* `ln w <- (1 - s) ln w + s ln w*`, with step `s` = 1/4, at each boundary with an
//!    estimate; `w` is kept between 1 us and 1 s and rounded to whole microseconds.
//!
//! The lookback is not a separate estimate: it is `w` rounded down to whole ticks. The anchor is
//! the earliest event in the cell's support, and a support that reaches further back than the
//! cluster's own extent includes strays that precede it. That the cluster's extent is where the
//! support should end is a structural choice taken from M2's lookback table (a design reading,
//! not learned); the learned part is the extent.
//!
//! **The ramp threshold.** The learner replays the graph's ramp integrator (its time constant,
//! jump band and penalty are M2's) on every counter reading, benign ones included, per counter
//! per service, as the cells take it: the level decays, the reading adds one, and if that carries
//! it across the threshold it fires and resets to zero; one pass later the penalty for a step
//! above the jump band is subtracted. It replays eight integrators at once, with thresholds 1.5,
//! 2, 2.5, 3, 3.5, 4, 5 and 6, and counts each one's firings: the notices the ramp path would
//! make at that threshold. It replays them again on the same arrival times with *surrogate*
//! steps: the step to each reading is drawn from the distribution of steps between readings of
//! one counter at one service at least 6 s apart (a stand-in for independent readings), at the
//! quantile given by a fixed hash of the series and the draw's index (pseudo-random, and a pure
//! function of its inputs, so a replay draws the same; no clock or seed is read). The firings above what
//! the surrogate gives are the dense, smooth readings. Lowering the threshold from one candidate
//! to the next adds that band's firings; a band is chance-dominated as above (`p`, with at least
//! 10 firings, observed plus surrogate, to say). The estimate `θ*` is the lowest candidate such
//! that its band and every band above it are not chance-dominated. No estimate before 50
//! firings (observed plus surrogate) at the lowest candidate. `θ <- (1 - s) θ + s θ*`.
//!
//! (Amendment A1, after the first development run: the first form of this statistic counted
//! excursions of the level by peak, not firings, and its estimate fell to the lowest candidate
//! while the cells fired several times per excursion; the form above counts what the cells emit.
//! Amendment A2, after the second development run: the surrogate steps were first drawn at the
//! points of the golden-ratio sequence, whose consecutive draws are anti-correlated, so the
//! surrogate never gave a run of small steps and its firings were nil above the lowest
//! thresholds; the hash draws above replace it, and a step is the bin's whole number, as the
//! counters are integers, not the bin's middle.)
//!
//! # Carry, and the controls
//!
//! The harness plays one fresh arm per segment, so the learned values and their evidence are kept
//! in a process-wide store keyed by `state_key` ([`carry`]) and loaded when a segment begins:
//! that is what lets experience accumulate across segments in stream order. With `carry` off the
//! arm learns within a segment only. With `learning` off it stays at its priors and keeps no
//! evidence; that is the learning-off control.
//!
//! # What this is not
//!
//! The adapter chooses the window and the threshold from the stream's own chance structure. It is
//! not told which clusters are incidents, so a cluster that is real but not an incident is as
//! good as one that is; it can be wrong about what the evaluator calls right, and the report
//! measures that. Nothing here is "attention" or "memory"; it is two numbers and the histograms
//! they were set from.
//!
//! # Parameter changes and the plasticity port
//!
//! The medium's plasticity port can only change plastic synapse weights. The learned quantities
//! are cell parameters, so the adapter flags a change and the noticer rebuilds the same graph
//! with the new values and transplants the old state into it ([`splice`]); the medium crate is
//! unchanged. The adapter's own work is counted and charged ([`noticing::LEARNER_OP_NS`]).

pub mod carry;
pub mod learner;
pub mod noticing;
pub mod params;
pub mod splice;

pub use learner::Learned;
pub use noticing::{LEARNED_COMPONENT, LEARNED_ID, LearnedNoticer, LearnedStats};
pub use params::LearnedParams;

use super::noticer::Noticer;
use super::rung::RungConfig;
use gordian_world::Service;

/// The learned noticer `params` names, for the public graph `services`, with the rung's
/// parameters `cfg`.
///
/// # Panics
///
/// If the graph does not build for this public graph. [`LearnedParams::validate`] builds it on a
/// small graph when the manifest is checked, so this is a defect, not a result.
pub fn build(params: &LearnedParams, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    match LearnedNoticer::new(*params, cfg.clone(), services) {
        Ok(n) => Box::new(n),
        Err(e) => panic!("the learned noticer's graph does not build: {e}"),
    }
}
