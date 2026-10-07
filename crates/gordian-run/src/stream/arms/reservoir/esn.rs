//! The echo state network: fixed random weights, the leaky state update, and the linear readout
//! trained by recursive least squares.
//!
//! Basic IEEE operations only (addition, subtraction, multiplication, division, square root, and
//! comparison), in a fixed order, so a replay gives the same bits. The activation is the softsign
//! `x / (1 + |x|)`, a bounded odd squashing function with no platform math in it.
//!
//! Every function that does arithmetic returns the number of multiply-adds (or equivalent
//! operations) it executed, by the formulas stated at each, so that the arm's cost is counted from
//! what ran and priced by the noticer.

// Stopped mid-unit (chief, 2026-10-07): the lint below flags index loops that walk two arrays
// in lockstep in the readout's recursive least squares; left for the PI to rewrite when L2
// resumes, so that the stopped unit merges with its numerics unchanged.
#![allow(clippy::needless_range_loop)]
use super::rng::Draws;

/// The kinds of abnormal observation the input tells apart: the five counters (error rate,
/// latency, saturation, authentication failures, restarts), messages, and snapshots.
pub const N_KINDS: usize = 7;
/// The counters whose last reading in a tick is an input.
pub const N_COUNTERS: usize = 5;
/// The node's own readings: what the readout predicts and what the residual is taken over.
pub const N_OUT: usize = N_KINDS + N_COUNTERS;
/// The inputs: the node's own readings and one population channel (abnormal observations at
/// other nodes in the tick).
pub const N_IN: usize = N_OUT + 1;

/// The scale of the fixed reservoir bias, uniform in `[-BIAS_SCALE, BIAS_SCALE]`: it breaks the
/// symmetry of the odd activation, so that the units are not sign-flipped copies of each other.
pub const BIAS_SCALE: f64 = 0.2;

/// The softsign activation.
pub fn softsign(x: f64) -> f64 {
    x / (1.0 + x.abs())
}

/// The recurrent fan-in per unit for a reservoir of `n` units: a tenth of the units, at least
/// three. Sparse, as the brief says.
pub fn fan_in_for(n: usize) -> usize {
    (n / 8).max(3)
}

/// The fixed random weights of the reservoir.
#[derive(Debug, Clone)]
pub struct Weights {
    n: usize,
    fan_in: usize,
    /// Source unit of each recurrent entry: `fan_in` per unit, unit-major.
    rec_idx: Vec<u32>,
    /// Value of each recurrent entry.
    rec_val: Vec<f64>,
    /// Input weights, `n x N_IN`, row-major.
    w_in: Vec<f64>,
    /// Bias per unit.
    bias: Vec<f64>,
}

impl Weights {
    /// The network of `n` units drawn from `seed`: each unit takes `fan_in_for(n)` distinct
    /// sources chosen uniformly (itself allowed), each with weight `+-radius / sqrt(fan_in)` with
    /// equal chance of either sign; input weights uniform in `[-input_scale, input_scale]`; bias
    /// uniform in `[-BIAS_SCALE, BIAS_SCALE]`. Draw order: for each unit its sources and signs,
    /// then the input weights unit by unit, then the biases.
    pub fn new(seed: u64, n: usize, radius: f64, input_scale: f64) -> Self {
        let fan_in = fan_in_for(n).min(n);
        let mut d = Draws::keyed(&[seed, n as u64]);
        let mag = radius / (fan_in as f64).sqrt();
        let mut rec_idx = Vec::with_capacity(n * fan_in);
        let mut rec_val = Vec::with_capacity(n * fan_in);
        for _ in 0..n {
            let mut chosen: Vec<u32> = Vec::with_capacity(fan_in);
            while chosen.len() < fan_in {
                let s = d.below(n as u64) as u32;
                if !chosen.contains(&s) {
                    chosen.push(s);
                }
            }
            for s in chosen {
                rec_idx.push(s);
                rec_val.push(if d.coin() { mag } else { -mag });
            }
        }
        let mut w_in = Vec::with_capacity(n * N_IN);
        for _ in 0..n * N_IN {
            w_in.push(d.signed() * input_scale);
        }
        let bias = (0..n).map(|_| d.signed() * BIAS_SCALE).collect();
        Self {
            n,
            fan_in,
            rec_idx,
            rec_val,
            w_in,
            bias,
        }
    }

    /// The number of units.
    pub fn size(&self) -> usize {
        self.n
    }

    /// The recurrent fan-in.
    pub fn fan_in(&self) -> usize {
        self.fan_in
    }

    /// The recurrent weights as a dense `n x n` matrix, row-major (for tests and the reference).
    pub fn dense_recurrent(&self) -> Vec<f64> {
        let mut m = vec![0.0; self.n * self.n];
        for i in 0..self.n {
            for t in 0..self.fan_in {
                m[i * self.n + self.rec_idx[i * self.fan_in + t] as usize] +=
                    self.rec_val[i * self.fan_in + t];
            }
        }
        m
    }

    /// The input weights, `n x N_IN`, row-major.
    pub fn input_weights(&self) -> &[f64] {
        &self.w_in
    }

    /// The biases.
    pub fn biases(&self) -> &[f64] {
        &self.bias
    }

    /// One tick of the leaky state: `x' = (1 - a) x + a softsign(W x + W_in u + b)`, written to
    /// `out`. `nz` lists, ascending, the indices of the non-zero entries of `u`; the others are
    /// not multiplied. Returns the operations executed: `n * (fan_in + nz + 7)` (a multiply-add
    /// for each recurrent and each non-zero input entry, and seven for the bias, the leak and the
    /// activation).
    pub fn step(
        &self,
        x: &[f64],
        u: &[f64; N_IN],
        nz: &[usize],
        leak: f64,
        out: &mut [f64],
    ) -> u64 {
        let keep = 1.0 - leak;
        for i in 0..self.n {
            let mut acc = self.bias[i];
            let base = i * self.fan_in;
            for t in 0..self.fan_in {
                acc += self.rec_val[base + t] * x[self.rec_idx[base + t] as usize];
            }
            let row = i * N_IN;
            for &c in nz {
                acc += self.w_in[row + c] * u[c];
            }
            out[i] = keep * x[i] + leak * softsign(acc);
        }
        (self.n * (self.fan_in + nz.len() + 7)) as u64
    }
}

/// The linear readout and its recursive-least-squares state.
///
/// `w` is `N_OUT x d` (row-major), the map from the regressor `z` to the predicted readings;
/// `p` is `d x d`, the running inverse of the regressors' second-moment matrix plus the ridge.
/// The initial readout is all zero ("predict nothing") and `p = I / ridge`. The update is the
/// standard RLS with no forgetting (`lambda = 1`): forgetting with sparse, mostly idle regressors
/// makes `p` grow without bound along the directions nothing excites.
#[derive(Debug, Clone, PartialEq)]
pub struct Readout {
    d: usize,
    w: Vec<f64>,
    p: Vec<f64>,
    updates: u64,
}

impl Readout {
    /// The initial readout for regressors of length `d` under `ridge`.
    pub fn new(d: usize, ridge: f64) -> Self {
        let mut p = vec![0.0; d * d];
        for i in 0..d {
            p[i * d + i] = 1.0 / ridge;
        }
        Self {
            d,
            w: vec![0.0; N_OUT * d],
            p,
            updates: 0,
        }
    }

    /// The regressor length.
    pub fn dim(&self) -> usize {
        self.d
    }

    /// The weights, `N_OUT x d`, row-major.
    pub fn weights(&self) -> &[f64] {
        &self.w
    }

    /// The inverse-covariance estimate, `d x d`, row-major.
    pub fn covariance(&self) -> &[f64] {
        &self.p
    }

    /// How many updates have been made (over every segment the readout was carried through).
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// The predicted readings `W z` into `out`. Returns `N_OUT * d` operations.
    pub fn predict(&self, z: &[f64], out: &mut [f64; N_OUT]) -> u64 {
        for (o, slot) in out.iter_mut().enumerate() {
            let row = &self.w[o * self.d..(o + 1) * self.d];
            let mut s = 0.0;
            for j in 0..self.d {
                s += row[j] * z[j];
            }
            *slot = s;
        }
        (N_OUT * self.d) as u64
    }

    /// One RLS update at regressor `z` with a priori error `err = y - W z` (the caller has just
    /// predicted at `z`): `v = P z`, `s = 1 / (1 + z.v)`, `k = s v`, `W += err k^T`,
    /// `P -= k v^T` (computed once for the upper triangle and mirrored, so `P` stays exactly
    /// symmetric). `scratch` has length at least `2 d` (it holds `v` and `k`). An update whose denominator is not a finite
    /// number above one is skipped (it cannot happen while `P` is positive definite). Returns the
    /// operations executed: `d^2 + d + 1 + d + N_OUT d + d (d + 1) / 2`, or 0 if skipped.
    pub fn update(&mut self, z: &[f64], err: &[f64; N_OUT], scratch: &mut [f64]) -> u64 {
        let d = self.d;
        let (v, k) = scratch.split_at_mut(d);
        for i in 0..d {
            let row = &self.p[i * d..(i + 1) * d];
            let mut s = 0.0;
            for j in 0..d {
                s += row[j] * z[j];
            }
            v[i] = s;
        }
        let mut q = 0.0;
        for i in 0..d {
            q += z[i] * v[i];
        }
        let denom = 1.0 + q;
        if !(denom.is_finite() && denom > 1.0) {
            return 0;
        }
        let s = 1.0 / denom;
        for j in 0..d {
            k[j] = s * v[j];
        }
        for o in 0..N_OUT {
            let e = err[o];
            let row = &mut self.w[o * d..(o + 1) * d];
            for j in 0..d {
                row[j] += e * k[j];
            }
        }
        for i in 0..d {
            let ki = k[i];
            for j in i..d {
                let pij = self.p[i * d + j] - ki * v[j];
                self.p[i * d + j] = pij;
                self.p[j * d + i] = pij;
            }
        }
        self.updates += 1;
        (d * d + d + 1 + d + N_OUT * d + d * (d + 1) / 2) as u64
    }
}

/// The regressor length for a reservoir of `n` units: the state, the input and a constant one.
pub fn regressor_len(n: usize) -> usize {
    n + N_IN + 1
}

/// Fill `z` with the regressor `[x, u, 1]`.
pub fn fill_regressor(z: &mut [f64], x: &[f64], u: &[f64; N_IN]) {
    let n = x.len();
    z[..n].copy_from_slice(x);
    z[n..n + N_IN].copy_from_slice(u);
    z[n + N_IN] = 1.0;
}
