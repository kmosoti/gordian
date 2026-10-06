//! The learner: the public summaries it keeps and the rule that turns them into parameters.
//!
//! The rule is stated in full in [`super`]'s module documentation; this file is its code. Nothing
//! here reads anything but delivered observations (their service, kind, value, instant and the
//! public rules' verdict) and the manifest's constants.

use crate::stream::arms::rung::{Held, service_of};
use gordian_world::{CounterName, Observation};
use std::collections::VecDeque;

/// Upper edges of the gap bins, seconds: the gap from an abnormal observation back to the nearest
/// earlier abnormal observation of another kind at the same service.
pub const GAP_EDGES_S: [f64; NB] = [
    0.001, 0.002, 0.003, 0.005, 0.008, 0.012, 0.020, 0.030, 0.050, 0.075, 0.100, 0.150, 0.200,
    0.300, 0.400, 0.600, 0.800, 1.000,
];
/// The same edges in nanoseconds, for comparing gaps exactly.
pub const GAP_EDGES_NS: [u64; NB] = [
    1_000_000,
    2_000_000,
    3_000_000,
    5_000_000,
    8_000_000,
    12_000_000,
    20_000_000,
    30_000_000,
    50_000_000,
    75_000_000,
    100_000_000,
    150_000_000,
    200_000_000,
    300_000_000,
    400_000_000,
    600_000_000,
    800_000_000,
    1_000_000_000,
];
/// Number of gap bins.
pub const NB: usize = 18;
/// The candidate ramp thresholds, in readings, lowest first.
pub const RAMP_CANDIDATES: [f64; NC] = [1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0];
/// Number of candidate thresholds.
pub const NC: usize = 8;
/// Number of bins of the step distribution of readings far apart in time (whole units 0 to 100).
pub const FAR_BINS: usize = 101;
/// Two readings of one counter at one service at least this far apart (seconds) are read as
/// independent draws.
pub const FAR_GAP_S: f64 = 6.0;
/// Least cumulative events with a predecessor within the horizon before the window moves.
pub const MIN_GAP_EVIDENCE: f64 = 200.0;
/// Fewest observed plus expected events in a bin for its test to be decisive.
pub const MIN_GAP_BIN: f64 = 20.0;
/// Least firings (observed plus surrogate) at the lowest candidate before the threshold moves.
pub const MIN_RAMP_EVIDENCE: f64 = 50.0;
/// Fewest observed plus surrogate firings in a candidate's band for its test to be decisive.
pub const MIN_RAMP_BIN: f64 = 10.0;
/// The floor of the replayed integrator's level.
const LEVEL_FLOOR: f64 = -50.0;
/// The golden ratio's fractional part, the step of the low-discrepancy sequence that draws the
/// surrogate steps without a random number generator.
const PHI: f64 = 0.618_033_988_749_894_9;

/// The four kinds of abnormal observation the burst cell tells apart: error rate, latency, a
/// message, and the rest.
pub fn slot_of(obs: &Observation) -> usize {
    match obs {
        Observation::Counter {
            name: CounterName::ErrorRate,
            ..
        } => 0,
        Observation::Counter {
            name: CounterName::Latency,
            ..
        } => 1,
        Observation::Message { .. } => 2,
        _ => 3,
    }
}

/// What is learned and carried from one segment to the next: the two parameters and the evidence
/// they were set from. Everything in it is a function of delivered observations.
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    /// The coincidence window, nanoseconds.
    pub window_ns: f64,
    /// The ramp threshold, in readings.
    pub ramp_threshold: f64,
    /// Cumulative: abnormal observations with an earlier abnormal observation of another kind at
    /// the same service within `GAP_EDGES_S[b]`.
    pub gap_obs: [f64; NB],
    /// The same count expected if kinds arrived independently at the observed rates.
    pub gap_nul: [f64; NB],
    /// Abnormal observations per kind, over everything seen.
    pub slot_events: [f64; 4],
    /// Service-seconds over everything seen.
    pub node_seconds: f64,
    /// Step sizes between consecutive readings of one counter at one service at least
    /// `FAR_GAP_S` apart.
    pub far: [f64; FAR_BINS],
    /// Firings of a replayed ramp integrator with threshold `RAMP_CANDIDATES[c]`, reset on firing.
    pub fire_obs: [f64; NC],
    /// The same on surrogate steps drawn from `far`.
    pub fire_nul: [f64; NC],
    /// Boundaries seen.
    pub boundaries: u64,
    /// Segments begun.
    pub segments: u64,
    /// Boundaries at which the window moved.
    pub window_updates: u64,
    /// Boundaries at which the threshold moved.
    pub threshold_updates: u64,
}

impl Learned {
    /// The state before any experience: the priors and no evidence.
    pub fn prior(window_ns: u64, ramp_threshold: f32) -> Self {
        Self {
            window_ns: window_ns as f64,
            ramp_threshold: f64::from(ramp_threshold),
            gap_obs: [0.0; NB],
            gap_nul: [0.0; NB],
            slot_events: [0.0; 4],
            node_seconds: 0.0,
            far: [0.0; FAR_BINS],
            fire_obs: [0.0; NC],
            fire_nul: [0.0; NC],
            boundaries: 0,
            segments: 0,
            window_updates: 0,
            threshold_updates: 0,
        }
    }

    /// The window the evidence supports, seconds: the upper edge of the last gap bin before the
    /// first bin in which, with enough events to say, more than `1 - min_precision` of the events
    /// are what independent kinds would give. `None` before enough evidence.
    pub fn window_estimate(&self, min_precision: f64) -> Option<f64> {
        if self.gap_obs[NB - 1] < MIN_GAP_EVIDENCE {
            return None;
        }
        let (mut prev_o, mut prev_n) = (0.0, 0.0);
        let mut last = 0;
        for b in 0..NB {
            let o = self.gap_obs[b] - prev_o;
            let n = self.gap_nul[b] - prev_n;
            prev_o = self.gap_obs[b];
            prev_n = self.gap_nul[b];
            if o + n >= MIN_GAP_BIN && n > (1.0 - min_precision) * o {
                break;
            }
            last = b;
        }
        Some(GAP_EDGES_S[last])
    }

    /// The ramp threshold the evidence supports. The firings at candidate `c` are those of a
    /// replayed integrator with that threshold; lowering the threshold from candidate `c + 1` to
    /// `c` adds the band's firings, `fire[c] - fire[c + 1]` (the top candidate's band is all its
    /// firings). The estimate is the lowest candidate such that its band and every band above it,
    /// with enough firings to say, has more than `min_precision` of its firings beyond what
    /// surrogate steps give. `None` before enough evidence.
    pub fn threshold_estimate(&self, min_precision: f64) -> Option<f64> {
        if self.fire_obs[0] + self.fire_nul[0] < MIN_RAMP_EVIDENCE {
            return None;
        }
        let mut th = RAMP_CANDIDATES[NC - 1];
        for c in (0..NC).rev() {
            let (mut o, mut n) = (self.fire_obs[c], self.fire_nul[c]);
            if c + 1 < NC {
                o -= self.fire_obs[c + 1];
                n -= self.fire_nul[c + 1];
            }
            if o + n >= MIN_RAMP_BIN && n > (1.0 - min_precision) * o {
                break;
            }
            th = RAMP_CANDIDATES[c];
        }
        Some(th)
    }

    /// The update rule, at a boundary: each parameter moves `step` of the way to the value its
    /// evidence supports (the window in log space), when there is evidence. Returns whether
    /// either moved.
    pub fn update(&mut self, step: f64, min_precision: f64) -> bool {
        self.boundaries += 1;
        let mut moved = false;
        if let Some(w) = self.window_estimate(min_precision) {
            let target = (w * 1e9).clamp(1_000.0, 1e9);
            let ln = (1.0 - step) * self.window_ns.ln() + step * target.ln();
            let new = ln.exp().clamp(1_000.0, 1e9);
            moved |= (new - self.window_ns).abs() > 0.0;
            self.window_ns = new;
            self.window_updates += 1;
        }
        if let Some(t) = self.threshold_estimate(min_precision) {
            let new = (1.0 - step) * self.ramp_threshold + step * t;
            moved |= (new - self.ramp_threshold).abs() > 0.0;
            self.ramp_threshold = new;
            self.threshold_updates += 1;
        }
        moved
    }
}

/// One delivered observation, as the learner keeps it until its cycle completes.
#[derive(Debug, Clone, Copy)]
struct Item {
    at_ns: u64,
    at_s: f64,
    node: usize,
    /// The counter's index in `CounterName::ALL` and its reading, for a counter reading.
    reading: Option<(usize, f64)>,
    /// The kind, for an abnormal observation.
    abnormal_slot: Option<usize>,
}

/// One counter at one service: the last reading and the replayed ramp integrators (one per
/// candidate threshold, and the same on surrogate steps).
#[derive(Debug, Clone, Copy, Default)]
struct Series {
    seen: bool,
    last_t: f64,
    last_v: f64,
    level: [f64; NC],
    s_level: [f64; NC],
    s_idx: u64,
}

/// What the learner keeps for one segment: the observations of the cycle in progress, the recent
/// abnormal observations per service, the counts that set the expected rates, the replayed
/// series. Dropped at the segment's end; only [`Learned`] is carried.
#[derive(Debug, Clone)]
pub struct Session {
    n_services: usize,
    tau_s: f64,
    jump: f64,
    penalty_per_unit: f64,
    prior_rate: f64,
    prior_s: f64,
    pending: VecDeque<Item>,
    recent: Vec<VecDeque<(u64, usize)>>,
    counts: Vec<[f64; 4]>,
    series: Vec<[Series; 5]>,
    last_t: f64,
    cdf: Vec<f64>,
    ops: u64,
}

impl Session {
    /// A session for `n_services` services. The ramp integrator's constants are the graph's
    /// (`tau_s`, `jump`, `penalty`); the rate prior is the rung's (`prior_mhz`, `prior_s`).
    pub fn new(
        n_services: usize,
        tau_s: f64,
        jump: f64,
        penalty: f64,
        prior_mhz: u64,
        prior_s: f64,
    ) -> Self {
        Self {
            n_services,
            tau_s: tau_s.max(1e-3),
            jump,
            penalty_per_unit: penalty / jump.max(1.0),
            prior_rate: prior_mhz as f64 / 1000.0,
            prior_s,
            pending: VecDeque::new(),
            recent: vec![VecDeque::new(); n_services],
            counts: vec![[0.0; 4]; n_services],
            series: vec![[Series::default(); 5]; n_services],
            last_t: 0.0,
            cdf: Vec::new(),
            ops: 0,
        }
    }

    /// Keep one delivered observation until its cycle completes.
    pub fn push(&mut self, held: &Held) {
        let Some(service) = service_of(&held.obs) else {
            return;
        };
        let node = service.0 as usize;
        if node >= self.n_services {
            return;
        }
        let reading = match &held.obs {
            Observation::Counter { name, value, .. } => CounterName::ALL
                .iter()
                .position(|n| n == name)
                .map(|i| (i, *value as f64)),
            _ => None,
        };
        self.pending.push_back(Item {
            at_ns: held.at.0,
            at_s: held.at.0 as f64 / 1e9,
            node,
            reading,
            abnormal_slot: held.abnormal.then(|| slot_of(&held.obs)),
        });
    }

    /// The counted operations since the last call.
    pub fn take_ops(&mut self) -> u64 {
        std::mem::take(&mut self.ops)
    }

    /// Absorb every kept observation before `upto_s` seconds into `learned`, in order.
    pub fn process_until(&mut self, learned: &mut Learned, upto_s: f64) {
        self.refresh_cdf(learned);
        while self.pending.front().is_some_and(|i| i.at_s < upto_s) {
            if let Some(item) = self.pending.pop_front() {
                self.absorb(learned, item);
            }
        }
        self.ops += (NB + NC + FAR_BINS) as u64;
    }

    fn refresh_cdf(&mut self, learned: &Learned) {
        let total: f64 = learned.far.iter().map(|x| x + 1.0).sum();
        let mut acc = 0.0;
        self.cdf = learned
            .far
            .iter()
            .map(|x| {
                acc += (x + 1.0) / total;
                acc
            })
            .collect();
    }

    /// The `idx`th surrogate step: the step distribution's quantile at the golden-ratio sequence.
    fn surrogate(&self, idx: u64) -> f64 {
        let u = ((idx + 1) as f64 * PHI).fract();
        let bin = self.cdf.partition_point(|c| *c < u).min(FAR_BINS - 1);
        bin as f64 + 0.5
    }

    fn absorb(&mut self, st: &mut Learned, item: Item) {
        let t = item.at_s;
        if t > self.last_t {
            st.node_seconds += (t - self.last_t) * self.n_services as f64;
            self.last_t = t;
        }
        if let Some((c, v)) = item.reading {
            self.ramp(st, item.node, c, t, v);
        }
        if let Some(slot) = item.abnormal_slot {
            self.gap(st, item.node, slot, item.at_ns);
        }
    }

    fn ramp(&mut self, st: &mut Learned, node: usize, c: usize, t: f64, v: f64) {
        self.ops += (4 * NC) as u64;
        let (tau, jump, pen) = (self.tau_s, self.jump, self.penalty_per_unit);
        let mut s = self.series[node][c];
        let (decay, dev, sdev) = if s.seen {
            let dt = t - s.last_t;
            let dev = (v - s.last_v).abs();
            if dt >= FAR_GAP_S {
                st.far[(dev as usize).min(FAR_BINS - 1)] += 1.0;
            }
            let sdev = self.surrogate(s.s_idx);
            s.s_idx += 1;
            ((-dt / tau).exp(), dev, sdev)
        } else {
            (0.0, 0.0, 0.0)
        };
        step_levels(&mut s.level, &mut st.fire_obs, decay, dev, jump, pen);
        step_levels(&mut s.s_level, &mut st.fire_nul, decay, sdev, jump, pen);
        s.seen = true;
        s.last_t = t;
        s.last_v = v;
        self.series[node][c] = s;
    }

    fn gap(&mut self, st: &mut Learned, node: usize, slot: usize, at_ns: u64) {
        self.ops += (NB + 6) as u64;
        let t = at_ns as f64 / 1e9;
        // The rate of the other kinds at this service, from this segment's counts so far shrunk
        // toward the pooled rate (or the rung's prior rate before any is pooled) with the
        // rung's prior weight.
        let elapsed = t.max(0.0);
        let mut rho = 0.0;
        for k in 0..4 {
            if k == slot {
                continue;
            }
            let pooled = if st.node_seconds > 1.0 {
                st.slot_events[k] / st.node_seconds
            } else {
                self.prior_rate / 4.0
            };
            rho += (self.counts[node][k] + self.prior_s * pooled) / (elapsed + self.prior_s);
        }
        for (b, edge) in GAP_EDGES_S.iter().enumerate() {
            st.gap_nul[b] += -(-edge * rho).exp_m1();
        }
        let recent = &mut self.recent[node];
        let horizon = GAP_EDGES_NS[NB - 1];
        while recent
            .front()
            .is_some_and(|(x, _)| at_ns.saturating_sub(*x) > horizon)
        {
            recent.pop_front();
        }
        if let Some((x, _)) = recent.iter().rev().find(|(_, k)| *k != slot) {
            let g = at_ns.saturating_sub(*x);
            for (b, edge) in GAP_EDGES_NS.iter().enumerate() {
                if g <= *edge {
                    st.gap_obs[b] += 1.0;
                }
            }
        }
        recent.push_back((at_ns, slot));
        self.counts[node][slot] += 1.0;
        st.slot_events[slot] += 1.0;
    }
}

/// One reading of the replayed integrators, as the graph's cells take it: the level decays, the
/// reading adds one and the integrator fires if that carries it across its threshold (and then
/// resets to zero), and only afterwards, one pass later in the graph, the penalty for a large
/// step is subtracted.
fn step_levels(
    levels: &mut [f64; NC],
    fires: &mut [f64; NC],
    decay: f64,
    dev: f64,
    jump: f64,
    penalty_per_unit: f64,
) {
    for (c, level) in levels.iter_mut().enumerate() {
        let decayed = *level * decay;
        let raised = decayed + 1.0;
        *level = if decayed < RAMP_CANDIDATES[c] && raised >= RAMP_CANDIDATES[c] {
            fires[c] += 1.0;
            0.0
        } else {
            raised
        };
        if dev > jump {
            *level = (*level - penalty_per_unit * dev).max(LEVEL_FLOOR);
        }
    }
}
