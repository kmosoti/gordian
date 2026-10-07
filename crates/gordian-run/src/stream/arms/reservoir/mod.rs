//! The reservoir noticer (work item L2, Lab 3): an echo state network per node whose residual
//! notices, with no hand-designed graph.
//!
//! The learned comparator for the learning claim. L1 asked whether the medium's constants can be
//! learned online from public history; this arm asks whether a generic cheap temporal learner does
//! as well with nothing hand-designed at all. The rule, the inputs, the priors, the residual rule
//! and the retirement below were written before any run of the arm;
//! `experiments/exploration/l2-reservoir.md` records every change made after one.
//!
//! # What is built
//!
//! One reservoir per node of the public graph, all of them the same network (the same fixed
//! random weights, drawn once from the manifest's seed) with their own activations; one linear
//! readout shared by every node, trained online by recursive least squares to predict a node's
//! readings in the next tick from the reservoir's state and the tick's input. A node's noticing
//! signal is the prediction residual. Nothing in the network, the input or the rule names a
//! family of incident, a vocabulary of the hidden side, a timing of a burst or the public graph's
//! edges (the graph is used only by the seam's shared attaching rule, below).
//!
//! # The tick, the input, the target
//!
//! The arm runs on a tick of `tick_ns` (the criterion's is 100 ms). A tick is complete when the
//! harness's instant has passed its end; the arm processes complete ticks in order, one noticing
//! step at a time, as the learned noticer runs its medium's ticks.
//!
//! For node `n` and tick `t`, the input `u(n, t)` has 13 entries, in fixed units:
//!
//! | Entries | Content |
//! |---|---|
//! | 0 to 6 | for each kind (error rate, latency, saturation, authentication failures, restarts, messages, snapshots): the number of abnormal observations of that kind at `n` in the tick, at most 4, over 2 |
//! | 7 to 11 | for each counter: its last reading at `n` in the tick over 100, at most 2, whether or not the public rules call it abnormal (0 if none) |
//! | 12 | abnormal observations at the other nodes in the tick, at most 8, over 4 |
//!
//! The abnormal-kind tags are the public rules' verdict and the observation's kind; the values are
//! the observations' own. Entries 0 to 11 are the node's own readings; entry 12 is a population
//! channel that carries no graph. The target of the prediction made after tick `t` is entries 0
//! to 11 of `u(n, t + 1)`. No tier, label, incident, deadline, reasoner answer or notice score
//! reaches the arm, and neither does the evaluator's measure of anything it has done.
//!
//! # The reservoir
//!
//! `size` units per node with state `x`, updated each tick by the leaky rule
//!
//! ```text
//! x' = (1 - a) x + a softsign(W x + W_in u + b)        softsign(v) = v / (1 + |v|)
//! ```
//!
//! `W` is sparse: each unit takes `max(3, size / 8)` distinct random sources, each with weight
//! `+-radius / sqrt(fan_in)` (the nominal spectral radius), `W_in` is dense and uniform in
//! `[-input_scale, input_scale]`, `b` uniform in `[-0.2, 0.2]`. All are drawn once, by a SplitMix64
//! sequence keyed by the manifest's seed and the size ([`rng`]). The leak `a`, the radius and
//! the size are tuned (below). Activations start at zero every segment. Every operation is a basic
//! IEEE one (the softsign is chosen for that), so a replay gives the same bits.
//!
//! # The update rule
//!
//! The regressor at node `n` after tick `t` is `z = [x, u, 1]`, of length `size + 14`. The
//! readout `W_out` (12 by `size + 14`) predicts `y_hat = W_out z`. Recursive least squares, with no
//! forgetting, trains it as follows. When tick `t + 1` has been taken in, for each node in order
//! of the graph, the a priori error is `err = u(n, t + 1)[0..12] - y_hat`; the update is
//!
//! ```text
//! v = P z      s = 1 / (1 + z . v)      W_out += err (s v)^T      P -= (s v) v^T
//! ```
//!
//! for the regressor `z` at which `y_hat` was made, and only then does the state move on. The
//! updates are prequential: the residual that is thresholded is the one before the readout has
//! seen the tick. The inverse-covariance estimate `P` is symmetric by construction. No forgetting
//! factor is used because sparse, mostly idle regressors make forgetting blow `P` up along the
//! directions nothing excites; the cost is that the readout averages over every regime the
//! stream has shown it.
//!
//! # The priors
//!
//! - **Readout:** all zero. The arm predicts nothing: before it has learned, its residual is the
//!   input itself. This is the least informative prior, and it is the learning-off control's
//!   readout for the whole run.
//! - **Inverse covariance:** `P = I / ridge`, ridge 0.1 (a weak ridge: after a few hundred ticks
//!   of activity the data outweighs it).
//! - **Weights:** random, fixed, a function of the seed. They are not tuned per segment.
//! - The reservoir's four free quantities (`size`, `leak`, `spectral_radius`) and the residual
//!   threshold are the tuned ones; their grid and the rule that picks from it are fixed in
//!   `scripts/l2_common.py` before the tuning run.
//!
//! # The residual-to-notice rule
//!
//! The residual energy of node `n` at tick `t + 1` is `e = sum over the 12 readings of err^2`,
//! in the units above. A node-tick is **hot** when `e > threshold`. A **run** is a maximal
//! sequence of consecutive hot ticks at one node; a run **begins** at its earliest tick. A run
//! that begins makes a notice, anchored at the first abnormal observation of the node in that
//! earliest tick (the first observation of the node in the tick if it has no abnormal one), with
//! the node as the site, **unless** that observation already belongs to a live anomaly, in which
//! case nothing is noticed (the shared attaching rule has already assigned it). Runs that begin in
//! the same tick are noticed in the order of their anchors' delivery. A run of one tick is a run:
//! there is no minimum length. The notice is made at the harness step at which the
//! tick completes; the anchor is the tick's observation, not the step.
//!
//! What is attached to a new anomaly is the shared seam's: abnormal observations after the anchor
//! at the site while it is speaking, and at the site's dependents within the rung's burst window
//! of the site's burst, by [`crate::stream::arms::noticer::attach_target`], with the observations that
//! joined no anomaly offered again as the learned noticer offers them. That rule reads the public
//! graph; it is shared by every noticer that is not the medium's, and is not part of this arm's
//! signal.
//!
//! # Retirement
//!
//! The rung's own quiet rule, shared with the rung's noticers: a noticed anomaly with no abnormal
//! observation attached to it for the rung's `quiet_ns` (6 s, a public constant) is retirable. The
//! arm has no latch of its own.
//!
//! # Carry, and the controls
//!
//! The harness plays one fresh arm per segment, so the trained readout (`W_out` and `P`) is kept
//! in a process-wide store keyed by `state_key` ([`carry`]) and loaded when a segment begins:
//! that is what lets experience accumulate across segments in stream order. With `carry` off
//! the arm learns within a segment only. With `learning` off the readout stays at its zero prior,
//! keeps no evidence and is never carried: that is the learning-off control, which has the same
//! reservoir, the same input and the same threshold.
//!
//! # Cost
//!
//! Every function that does arithmetic returns the operations it executed, by the formulas in
//! [`esn`]; the noticer reports them through the seam's cost hook at the declared price of
//! [`noticing::ESN_OP_NS`] per operation, charged to the arm's bill under component 19 like the
//! medium's operations. The segment's compute limit (2 s of modelled compute) is a hard limit
//! like every other: a noticer whose charge is refused stops noticing for the rest of the segment.
//! The readout's update is quadratic in `size + 14` per node per tick, and is by far the largest
//! term.
//!
//! # What this is not
//!
//! The arm is told nothing about which clusters are incidents. Its notices are the ticks at
//! which a learned expectation of the node's readings fails by more than a threshold; they are
//! anchored where the failure begins, which may or may not be where the evaluator says an incident
//! begins, and the report measures that. Nothing here is "attention" or "memory"; it is a fixed
//! random network, a matrix of 12 by `size + 14` weights and the matrix that updates them.

pub mod carry;
pub mod esn;
pub mod noticing;
pub mod params;
pub mod rng;

pub use noticing::{
    ESN_OP_NS, RESERVOIR_COMPONENT, RESERVOIR_ID, ReservoirNoticer, ReservoirStats,
};
pub use params::ReservoirParams;

use super::noticer::Noticer;
use super::rung::RungConfig;
use gordian_world::Service;

/// The reservoir noticer `params` names, for the public graph `services`, with the rung's
/// parameters `cfg`.
///
/// # Panics
///
/// If the parameters do not validate. [`ReservoirParams::validate`] is checked when the manifest
/// is, so this is a defect, not a result.
pub fn build(params: &ReservoirParams, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    match ReservoirNoticer::new(*params, cfg.clone(), services) {
        Ok(n) => Box::new(n),
        Err(e) => panic!("the reservoir noticer does not build: {e}"),
    }
}
