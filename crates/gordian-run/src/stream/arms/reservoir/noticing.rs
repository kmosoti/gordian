//! [`ReservoirNoticer`]: the echo state network behind B1's `Noticer` seam.
//!
//! The rule, the inputs and the priors are in the module documentation of [`super`]. This file is
//! the machinery: the tick clock, the per-node cells, the reservoir and readout per tick, the
//! residual runs, and the seam's bookkeeping, which is the learned noticer's (the attaching rule,
//! the re-offer of observations that joined no anomaly, the score, the retirement test) so that
//! everything downstream of a notice is shared.

use super::carry;
use super::esn::{
    N_COUNTERS, N_IN, N_KINDS, N_OUT, Readout, Weights, fill_regressor, regressor_len,
};
use super::params::ReservoirParams;
use crate::stream::arms::noticer::{
    Notice, Noticer, NoticerCost, Scorer, Tracked, attach_target, quiet_ids,
};
use crate::stream::arms::rung::{Held, RungConfig, Store, service_of};
use gordian_core::{ComponentId, Instant};
use gordian_stream::ObsId;
use gordian_world::{CounterName, Observation, Service};
use std::collections::VecDeque;

/// The noticer's id, as the run output writes it.
pub const RESERVOIR_ID: &str = "reservoir";

/// The id the noticer's charges are attributed to on the bill (the medium's is 16, the learned
/// noticer's 17, the dataflow noticer's 18).
pub const RESERVOIR_COMPONENT: ComponentId = ComponentId(19);

/// The declared price of one counted operation (a multiply-add, or one of the seven cheap
/// operations of a unit's update), nanoseconds. One nanosecond is an assumption about a dense
/// scalar loop on this machine; the run's own measured time per segment is reported beside the
/// modelled one so that the assumption can be read against a measurement.
pub const ESN_OP_NS: u64 = 1;

/// The most of one kind of abnormal observation counted in one node's tick.
pub const ABNORMAL_CAP: u32 = 4;
/// The most of other nodes' abnormal observations counted in a tick.
pub const POPULATION_CAP: u32 = 8;
/// A counter reading is the reading over this, capped at [`VALUE_CAP`].
pub const VALUE_UNIT: f64 = 100.0;
/// The largest value input.
pub const VALUE_CAP: f64 = 2.0;

/// How long abnormal observations that joined no anomaly are kept to be offered again.
const REOFFER_NS: u64 = 4_000_000_000;

/// What the noticer counted over the segment, for diagnostics and tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReservoirStats {
    /// Ticks processed.
    pub ticks: u64,
    /// Node-ticks whose residual energy exceeded the threshold.
    pub hot: u64,
    /// Of those, the ones that began a run (the previous tick at the node was not hot).
    pub runs: u64,
    /// Runs whose anchor already belonged to a live anomaly, so that no notice was made.
    pub suppressed: u64,
    /// Notices made.
    pub notices: u64,
    /// Readout updates made in the segment.
    pub updates: u64,
    /// Observations that arrived for a tick already processed (counted in the current tick).
    pub late: u64,
}

/// One node's observations in one tick, reduced to the input.
#[derive(Debug, Clone, Default)]
struct Cell {
    abnormal: [u32; N_KINDS],
    values: [f64; N_COUNTERS],
    abnormal_total: u32,
    first_abnormal: Option<usize>,
    first_any: Option<usize>,
}

/// The index of an observation's kind among the seven the input tells apart.
fn kind_of(obs: &Observation) -> usize {
    match obs {
        Observation::Counter { name, .. } => counter_index(*name),
        Observation::Message { .. } => N_COUNTERS,
        _ => N_COUNTERS + 1,
    }
}

fn counter_index(name: CounterName) -> usize {
    match name {
        CounterName::ErrorRate => 0,
        CounterName::Latency => 1,
        CounterName::Saturation => 2,
        CounterName::AuthFailures => 3,
        CounterName::Restarts => 4,
    }
}

/// The input of one node for one tick from its cell and the abnormal observations at all other
/// nodes in the tick, and the indices of its non-zero entries (ascending).
///
/// Entries `0..7`: abnormal observations of each kind, at most [`ABNORMAL_CAP`], over two.
/// Entries `7..12`: the last reading of each counter in the tick over [`VALUE_UNIT`], at most
/// [`VALUE_CAP`], whether or not the public rules call it abnormal. Entry `12`: abnormal
/// observations at the other nodes, at most [`POPULATION_CAP`], over four.
pub fn input_of(cell_abnormal: &[u32; N_KINDS], cell_values: &[f64; N_COUNTERS], others: u32) -> ([f64; N_IN], Vec<usize>) {
    let mut u = [0.0; N_IN];
    for (k, slot) in u.iter_mut().take(N_KINDS).enumerate() {
        *slot = f64::from(cell_abnormal[k].min(ABNORMAL_CAP)) / 2.0;
    }
    for c in 0..N_COUNTERS {
        u[N_KINDS + c] = cell_values[c];
    }
    u[N_OUT] = f64::from(others.min(POPULATION_CAP)) / 4.0;
    let nz = (0..N_IN).filter(|&i| u[i] != 0.0).collect();
    (u, nz)
}

/// The reservoir noticer. See the module documentation of [`super`].
pub struct ReservoirNoticer {
    params: ReservoirParams,
    cfg: RungConfig,
    services: Vec<Service>,
    weights: Weights,
    readout: Readout,
    nodes: usize,
    // Per node, flattened: reservoir state, the regressor the standing prediction was made at,
    // and the standing prediction.
    state: Vec<f64>,
    scratch_state: Vec<f64>,
    regressor: Vec<f64>,
    prediction: Vec<[f64; N_OUT]>,
    hot_before: Vec<bool>,
    update_scratch: Vec<f64>,
    // The clock and the intake.
    pending: VecDeque<Held>,
    cursor: u32,
    next_tick: u64,
    // The seam's bookkeeping.
    unattached: VecDeque<Held>,
    scorer: Scorer,
    anomalies: Vec<Tracked>,
    next_id: u32,
    stopped: bool,
    ops: u64,
    ops_total: u64,
    stats: ReservoirStats,
    energies: Option<Vec<(u64, u16, f64)>>,
}

impl std::fmt::Debug for ReservoirNoticer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReservoirNoticer")
            .field("params", &self.params)
            .field("next_tick", &self.next_tick)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl ReservoirNoticer {
    /// A noticer for the public graph `services`, with the rung's parameters `cfg`. Starts from
    /// the readout carried under `params.state_key` if the arm learns and carries and there is
    /// one of the right size, else from the initial readout.
    pub fn new(params: ReservoirParams, cfg: RungConfig, services: &[Service]) -> Result<Self, String> {
        params.validate()?;
        let n = params.size as usize;
        let nodes = services.len();
        let weights = Weights::new(params.seed, n, params.spectral_radius, params.input_scale);
        let d = regressor_len(n);
        let readout = if params.learning && params.carry {
            carry::load(params.state_key).filter(|r| r.dim() == d)
        } else {
            None
        }
        .unwrap_or_else(|| Readout::new(d, params.ridge));
        let mut regressor = vec![0.0; nodes * d];
        let mut prediction = vec![[0.0; N_OUT]; nodes];
        let zero_input = [0.0; N_IN];
        let zero_state = vec![0.0; n];
        let mut ops = 0;
        for i in 0..nodes {
            fill_regressor(&mut regressor[i * d..(i + 1) * d], &zero_state, &zero_input);
            ops += readout.predict(&regressor[i * d..(i + 1) * d], &mut prediction[i]);
        }
        Ok(Self {
            scorer: Scorer::new(&cfg, nodes),
            params,
            cfg,
            services: services.to_vec(),
            weights,
            readout,
            nodes,
            state: vec![0.0; nodes * n],
            scratch_state: vec![0.0; n],
            regressor,
            prediction,
            hot_before: vec![false; nodes],
            update_scratch: vec![0.0; 2 * d],
            pending: VecDeque::new(),
            cursor: 0,
            next_tick: 0,
            unattached: VecDeque::new(),
            anomalies: Vec::new(),
            next_id: 0,
            stopped: false,
            ops,
            ops_total: 0,
            stats: ReservoirStats::default(),
            energies: None,
        })
    }

    /// Record every node-tick's residual energy (tick, node, energy) from now on. For tests and
    /// diagnostics; a run does not.
    pub fn log_energies(&mut self) {
        self.energies = Some(Vec::new());
    }

    /// The recorded energies, if [`ReservoirNoticer::log_energies`] was called.
    pub fn energies(&self) -> &[(u64, u16, f64)] {
        self.energies.as_deref().unwrap_or(&[])
    }

    /// The readout now.
    pub fn readout(&self) -> &Readout {
        &self.readout
    }

    /// The fixed weights.
    pub fn weights(&self) -> &Weights {
        &self.weights
    }

    /// What the noticer counted.
    pub fn stats(&self) -> &ReservoirStats {
        &self.stats
    }

    /// The operations counted over the segment.
    pub fn ops_total(&self) -> u64 {
        self.ops_total + self.ops
    }

    /// The modelled cost of the segment so far, nanoseconds.
    pub fn total_ns(&self) -> u64 {
        self.ops_total() * ESN_OP_NS
    }

    fn ingest(&mut self, store: &Store) {
        let fresh: Vec<&Held> = store
            .iter()
            .rev()
            .take_while(|h| h.id.0 >= self.cursor)
            .collect();
        for h in fresh.into_iter().rev() {
            self.cursor = h.id.0.saturating_add(1);
            if service_of(&h.obs).is_some() {
                self.pending.push_back(h.clone());
            }
        }
    }

    /// Process every complete tick before `now`, in order, making the notices their hot runs give.
    /// Returns the indices (into `anomalies`) of the anomalies created, in order.
    fn run_ticks(&mut self, now: Instant) -> Vec<usize> {
        let tick_ns = self.params.tick_ns;
        let complete = now.0 / tick_ns;
        let mut created = Vec::new();
        let before = self.next_tick;
        while self.next_tick < complete && !self.stopped {
            let tick = self.next_tick;
            self.process_tick(tick, now, &mut created);
            self.next_tick += 1;
        }
        // Kept once per step that trained, so that a segment's end leaves its readout behind.
        if self.next_tick > before && self.params.learning && self.params.carry {
            carry::store(self.params.state_key, &self.readout);
        }
        created
    }

    fn process_tick(&mut self, tick: u64, now: Instant, created: &mut Vec<usize>) {
        let tick_ns = self.params.tick_ns;
        let n = self.params.size as usize;
        let d = regressor_len(n);
        // The tick's observations, in delivery order.
        let mut bucket: Vec<Held> = Vec::new();
        while let Some(front) = self.pending.front() {
            let t = front.at.0 / tick_ns;
            if t > tick {
                break;
            }
            if t < tick {
                self.stats.late += 1;
            }
            if let Some(h) = self.pending.pop_front() {
                bucket.push(h);
            }
        }
        let mut cells = vec![Cell::default(); self.nodes];
        let mut abnormal_all = 0u32;
        for (i, h) in bucket.iter().enumerate() {
            let Some(service) = service_of(&h.obs) else {
                continue;
            };
            let Some(cell) = cells.get_mut(service.index()) else {
                continue;
            };
            if cell.first_any.is_none() {
                cell.first_any = Some(i);
            }
            let kind = kind_of(&h.obs);
            if h.abnormal {
                cell.abnormal[kind] = cell.abnormal[kind].saturating_add(1);
                cell.abnormal_total += 1;
                abnormal_all += 1;
                if cell.first_abnormal.is_none() {
                    cell.first_abnormal = Some(i);
                }
            }
            if let Observation::Counter { name, value, .. } = &h.obs {
                cell.values[counter_index(*name)] = (*value as f64 / VALUE_UNIT).min(VALUE_CAP);
            }
        }
        let learning = self.params.learning;
        let threshold = self.params.threshold;
        let mut candidates: Vec<Held> = Vec::new();
        for node in 0..self.nodes {
            let cell = &cells[node];
            let others = abnormal_all - cell.abnormal_total;
            let (u, nz) = input_of(&cell.abnormal, &cell.values, others);
            // The residual of the standing prediction against what this tick brought.
            let mut err = [0.0; N_OUT];
            let mut energy = 0.0;
            for o in 0..N_OUT {
                err[o] = u[o] - self.prediction[node][o];
                energy += err[o] * err[o];
            }
            self.ops += N_OUT as u64 * 3;
            if learning {
                let z = &self.regressor[node * d..(node + 1) * d];
                let ops = self.readout.update(z, &err, &mut self.update_scratch);
                if ops > 0 {
                    self.stats.updates += 1;
                }
                self.ops += ops;
            }
            // The state moves on, and a new prediction is made for the next tick.
            let x = &mut self.state[node * n..(node + 1) * n];
            self.ops += self.weights.step(x, &u, &nz, self.params.leak, &mut self.scratch_state);
            x.copy_from_slice(&self.scratch_state);
            fill_regressor(&mut self.regressor[node * d..(node + 1) * d], x, &u);
            self.ops += self.readout.predict(
                &self.regressor[node * d..(node + 1) * d],
                &mut self.prediction[node],
            );
            if let Some(log) = self.energies.as_mut() {
                log.push((tick, node as u16, energy));
            }
            let hot = energy > threshold;
            if hot {
                self.stats.hot += 1;
                if !self.hot_before[node] {
                    self.stats.runs += 1;
                    if let Some(i) = cell.first_abnormal.or(cell.first_any) {
                        candidates.push(bucket[i].clone());
                    }
                }
            }
            self.hot_before[node] = hot;
        }
        self.stats.ticks += 1;
        // In the order the anchors were delivered, which within a tick is the order in time: the
        // node that began first is noticed first, so that the observations of a node that began
        // a moment later (a dependent) are attached to its anomaly and not made another.
        candidates.sort_by_key(|h| h.id);
        for held in candidates {
            if let Some(i) = self.open(&held, now) {
                self.reoffer(&[i]);
                created.push(i);
            }
        }
    }

    /// Open a noticed anomaly anchored on `held`, unless the anchor already belongs to one.
    fn open(&mut self, held: &Held, now: Instant) -> Option<usize> {
        if self.anomalies.iter().any(|a| a.owns(held.id)) {
            self.stats.suppressed += 1;
            return None;
        }
        let site = service_of(&held.obs)?;
        let mut a = Tracked::new(
            self.next_id,
            held,
            site,
            &self.services,
            self.cfg.burst_gap_ns,
        );
        a.noticed_at = Some(now);
        self.next_id += 1;
        self.anomalies.push(a);
        self.stats.notices += 1;
        Some(self.anomalies.len() - 1)
    }

    /// Offer the abnormal observations that joined no anomaly to the new ones, by the shared
    /// attaching rule (the learned noticer's, unchanged).
    fn reoffer(&mut self, created: &[usize]) {
        if created.is_empty() {
            return;
        }
        let first_anchor = created
            .iter()
            .map(|&i| self.anomalies[i].anchor)
            .min()
            .unwrap_or(ObsId(u32::MAX));
        let kept = std::mem::take(&mut self.unattached);
        for h in kept {
            let joined = if h.id > first_anchor && !self.anomalies.iter().any(|a| a.owns(h.id)) {
                service_of(&h.obs).and_then(|service| {
                    attach_target(
                        &self.anomalies,
                        service,
                        h.at,
                        self.cfg.burst_ns,
                        self.cfg.burst_gap_ns,
                    )
                    .filter(|i| created.contains(i) && self.anomalies[*i].anchor < h.id)
                    .map(|i| (i, service))
                })
            } else {
                None
            };
            match joined {
                Some((i, service)) => {
                    self.anomalies[i].note_attached(&h, service, self.cfg.burst_gap_ns);
                }
                None => self.unattached.push_back(h),
            }
        }
    }

    fn prune(&mut self, now: Instant) {
        while self
            .unattached
            .front()
            .is_some_and(|h| h.at.0.saturating_add(REOFFER_NS) < now.0)
        {
            self.unattached.pop_front();
        }
    }
}

impl Noticer for ReservoirNoticer {
    fn id(&self) -> &'static str {
        RESERVOIR_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        if !held.abnormal {
            return None;
        }
        self.scorer.saw_abnormal();
        let service = service_of(&held.obs)?;
        match attach_target(
            &self.anomalies,
            service,
            held.at,
            self.cfg.burst_ns,
            self.cfg.burst_gap_ns,
        ) {
            Some(i) => {
                self.anomalies[i].note_attached(held, service, self.cfg.burst_gap_ns);
                Some(self.anomalies[i].id)
            }
            None => {
                self.unattached.push_back(held.clone());
                None
            }
        }
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        let mut out = Vec::new();
        if !self.stopped {
            self.ingest(store);
            let created = self.run_ticks(now);
            for i in created {
                let a = &self.anomalies[i];
                out.push(Notice {
                    id: a.id,
                    anchor: a.anchor,
                    anchor_at: a.anchor_at,
                    site: a.site,
                    attached: a.attached.iter().map(|(_, o, _)| *o).collect(),
                });
            }
        }
        self.prune(now);
        out
    }

    fn anomalies(&self) -> &[Tracked] {
        &self.anomalies
    }

    fn score(&self, id: u32, now: Instant) -> f64 {
        self.tracked(id)
            .map_or(f64::NEG_INFINITY, |a| self.scorer.score(a, now))
    }

    fn refresh(&mut self, now: Instant) {
        for a in &mut self.anomalies {
            if a.noticed_at.is_some() {
                a.peak_score = a.peak_score.max(self.scorer.score(a, now));
            }
        }
    }

    fn retirable(&self, now: Instant) -> Vec<u32> {
        quiet_ids(&self.anomalies, now, self.cfg.quiet_ns)
    }

    fn retire(&mut self, id: u32) {
        self.anomalies.retain(|a| a.id != id);
    }

    fn take_cost(&mut self) -> Option<NoticerCost> {
        let ops = std::mem::take(&mut self.ops);
        if ops == 0 {
            return None;
        }
        self.ops_total += ops;
        Some(NoticerCost {
            component: RESERVOIR_COMPONENT,
            compute_ns: ops * ESN_OP_NS,
        })
    }

    fn refused(&mut self) {
        self.stopped = true;
    }
}
