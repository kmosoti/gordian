//! [`LearnedNoticer`]: M2's medium behind B1's `Noticer` seam, with three of its constants learned.
//!
//! The effector, the sense and clock adapters, the attaching rule, the ledger and the hard limits
//! are M2's (`arms/medium/noticing.rs`), copied here because that file builds its medium itself
//! and gives the plasticity port nothing to run; a test pins that with the learned values set to
//! M2's frozen constants and learning off this arm yields the same notices, retirements and bill
//! as `MediumNoticer`. What differs:
//!
//! - **The graph.** [`crate::stream::arms::medium::graph::spec`] with the learned values in its
//!   parameters, built with every quantity in time already converted
//!   (`oscillome.seconds` empty, see [`super::splice`]), and the oscillome's 10 s rhythm with
//!   cycle summaries and the plasticity port at its boundaries.
//! - **The plasticity adapter** ([`Hook`]). At the end of each boundary tick of the 10 s rhythm
//!   the medium calls it with the cycle's summary. It absorbs the cycle's observations into the
//!   learner's evidence and applies the update rule ([`super::learner`]). The port cannot change
//!   a cell's parameters, so the adapter records that they changed, and the noticer swaps in the
//!   same graph with the new values, state intact ([`super::splice::transplant`]), before the
//!   next tick.
//! - **The ledger.** The medium's counts and ticks as M2, plus the learner's own counted
//!   operations (an operation per bin updated, per replayed reading and per bin scanned at a
//!   boundary) at the price of one synapse traversal each ([`LEARNER_OP_NS`]).

use super::carry;
use super::learner::{Learned, Session};
use super::params::{LearnedParams, round_window};
use super::splice::transplant;
use crate::stream::arms::medium::adapters::{DeliveredSense, TickClock, TickLedger, encode};
use crate::stream::arms::medium::graph::{KIND_NOTICE, KIND_RETIRE, Layout, spec};
use crate::stream::arms::noticer::{Notice, Noticer, NoticerCost, Scorer, Tracked, attach_target};
use crate::stream::arms::rung::{Held, RungConfig, Store, service_of};
use gordian_core::{ComponentId, Instant};
use gordian_medium::{
    CollectingEffector, ConstantField, CycleSummary, Field, Medium, MediumSpec, Plasticity, Ports,
    Prices, TickSummary,
};
use gordian_stream::ObsId;
use gordian_world::Service;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The noticer's id, as the run output writes it.
pub const LEARNED_ID: &str = "learned";

/// The id the learned noticer's charges are attributed to on the bill (the medium's is 16).
pub const LEARNED_COMPONENT: ComponentId = ComponentId(17);

/// The period of the rhythm the plasticity adapter runs at, nanoseconds.
pub const RHYTHM_NS: u64 = 10_000_000_000;

/// The modelled price of one counted learner operation, nanoseconds: a synapse traversal's.
pub const LEARNER_OP_NS: u64 = 25;

/// How long abnormal observations that joined no anomaly are kept to be offered again.
const REOFFER_NS: u64 = 4_000_000_000;

/// What the noticer counted over the segment, for diagnostics and tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LearnedStats {
    /// Proposals of kind notice.
    pub notice_proposals: u64,
    /// Of those, the ones whose anchor was a live anomaly's anchor.
    pub duplicate_notices: u64,
    /// Of those, the ones whose anchor was no longer held.
    pub lost_anchors: u64,
    /// Of the notice proposals, the ones made by a ramp emitter.
    pub ramp_proposals: u64,
    /// Proposals of kind retire.
    pub retire_proposals: u64,
    /// Ticks the medium refused (a defect: should be none).
    pub step_errors: u64,
    /// Boundaries the plasticity adapter ran at.
    pub boundaries: u64,
    /// Times the graph was rebuilt with new values and the state transplanted.
    pub swaps: u64,
    /// Swaps that failed their check (a defect: should be none).
    pub swap_errors: u64,
}

/// The plasticity adapter: runs at the 10 s rhythm's boundaries.
struct Hook<'a> {
    state: &'a mut Learned,
    session: &'a mut Session,
    learning: bool,
    step: f64,
    min_precision: f64,
    boundaries: &'a mut u64,
    ran: bool,
    changed: bool,
}

impl Plasticity for Hook<'_> {
    fn end_of_tick(&mut self, _medium: &mut Medium, _summary: &TickSummary) {}

    fn end_of_cycle(&mut self, _medium: &mut Medium, cycle: &CycleSummary) {
        *self.boundaries += 1;
        self.ran = true;
        if !self.learning {
            return;
        }
        let upto_s = (cycle.cycle + 1) as f64 * (RHYTHM_NS as f64 / 1e9);
        self.session.process_until(self.state, upto_s);
        if self.state.update(self.step, self.min_precision) {
            self.changed = true;
        }
    }
}

/// The medium's spec for `params` and the learned values: M2's graph with the conversions
/// applied, and the 10 s rhythm with its summaries and plasticity.
pub fn learned_spec(
    params: &LearnedParams,
    state: &Learned,
    services: &[Service],
) -> Result<(MediumSpec, Layout), String> {
    let mp = params.medium_params(round_window(state.window_ns), state.ramp_threshold as f32);
    let (mut spec, layout) = spec(&mp, services)?;
    spec.oscillome.seconds.clear();
    spec.oscillome.periods_ns = vec![RHYTHM_NS];
    spec.oscillome.cycle_summary = true;
    spec.oscillome.plasticity_rhythm = Some(0);
    Ok((spec, layout))
}

/// The learned noticer. See the module documentation.
pub struct LearnedNoticer {
    params: LearnedParams,
    cfg: RungConfig,
    services: Vec<Service>,
    layout: Layout,
    medium: Medium,
    prices: Prices,
    state: Learned,
    session: Session,
    boundaries: u64,
    sense: DeliveredSense,
    ledger: TickLedger,
    effector: CollectingEffector,
    next_tick: u64,
    cursor: u32,
    recent: VecDeque<Held>,
    unattached: VecDeque<Held>,
    scorer: Scorer,
    anomalies: Vec<Tracked>,
    next_id: u32,
    retiring: BTreeSet<u32>,
    origin: BTreeMap<u32, u32>,
    stopped: bool,
    learner_ops: u64,
    learner_ops_total: u64,
    stats: LearnedStats,
}

impl std::fmt::Debug for LearnedNoticer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LearnedNoticer")
            .field("params", &self.params)
            .field("next_tick", &self.next_tick)
            .field("window_ns", &self.state.window_ns)
            .field("ramp_threshold", &self.state.ramp_threshold)
            .finish()
    }
}

impl LearnedNoticer {
    /// A learned noticer for the public graph `services`, with the rung's parameters `cfg`.
    /// Starts from the carried state under `params.state_key` if the arm learns and carries and
    /// there is one, else from the priors.
    pub fn new(
        params: LearnedParams,
        cfg: RungConfig,
        services: &[Service],
    ) -> Result<Self, String> {
        let mut state = if params.learning && params.carry {
            carry::load(params.state_key)
        } else {
            None
        }
        .unwrap_or_else(|| {
            Learned::prior(
                params.prior_window(cfg.burst_ns),
                params.prior_ramp_threshold,
            )
        });
        if params.learning {
            state.segments += 1;
        }
        let (spec, layout) = learned_spec(&params, &state, services)?;
        let medium = Medium::from_spec(&spec).map_err(|e| format!("{e:?}"))?;
        let s = &params.structure;
        let session = Session::new(
            services.len(),
            s.ramp_tau_ns as f64 / 1e9,
            f64::from(s.ramp_jump),
            f64::from(s.ramp_penalty),
            cfg.prior_mhz,
            cfg.prior_ns as f64 / 1e9,
        );
        Ok(Self {
            scorer: Scorer::new(&cfg, services.len()),
            prices: *medium.prices(),
            params,
            cfg,
            services: services.to_vec(),
            layout,
            medium,
            state,
            session,
            boundaries: 0,
            sense: DeliveredSense::default(),
            ledger: TickLedger::default(),
            effector: CollectingEffector::default(),
            next_tick: 0,
            cursor: 0,
            recent: VecDeque::new(),
            unattached: VecDeque::new(),
            anomalies: Vec::new(),
            next_id: 0,
            retiring: BTreeSet::new(),
            origin: BTreeMap::new(),
            stopped: false,
            learner_ops: 0,
            learner_ops_total: 0,
            stats: LearnedStats::default(),
        })
    }

    /// The medium.
    pub fn medium(&self) -> &Medium {
        &self.medium
    }

    /// The learned state now.
    pub fn learned(&self) -> &Learned {
        &self.state
    }

    /// The ledger port: ticks and counts over the segment.
    pub fn ledger(&self) -> &TickLedger {
        &self.ledger
    }

    /// What the noticer counted.
    pub fn stats(&self) -> &LearnedStats {
        &self.stats
    }

    /// The learner's counted operations over the segment.
    pub fn learner_ops_total(&self) -> u64 {
        self.learner_ops_total
    }

    /// The modelled cost of the segment so far, nanoseconds: the medium's counts at the declared
    /// prices plus the price per tick, plus the learner's operations.
    pub fn total_ns(&self) -> u64 {
        self.ledger.total_ns(&self.prices) + self.learner_ops_total * LEARNER_OP_NS
    }

    fn ingest(&mut self, store: &Store) {
        let fresh: Vec<&Held> = store
            .iter()
            .rev()
            .take_while(|h| h.id.0 >= self.cursor)
            .collect();
        for h in fresh.into_iter().rev() {
            self.cursor = h.id.0.saturating_add(1);
            if (h.abnormal || !self.params.structure.abnormal_only)
                && let Some(event) = encode(h, self.params.structure.tick_ns)
            {
                self.sense.push(event);
            }
            if self.params.learning {
                self.session.push(h);
            }
            self.recent.push_back(h.clone());
        }
    }

    /// Rebuild the graph with the learned values now and carry the state over.
    fn swap(&mut self) {
        let built = learned_spec(&self.params, &self.state, &self.services)
            .and_then(|(spec, _)| Medium::from_spec(&spec).map_err(|e| format!("{e:?}")))
            .and_then(|fresh| transplant(&self.medium, &fresh));
        match built {
            Ok(m) => {
                self.medium = m;
                self.stats.swaps += 1;
            }
            Err(_) => self.stats.swap_errors += 1,
        }
    }

    fn run_ticks(&mut self, now: Instant) {
        let tick_ns = self.params.structure.tick_ns;
        let complete = TickClock::complete_before(tick_ns, now);
        let mut field = ConstantField(Field::default());
        let mut trace = gordian_medium::NoTrace;
        while self.next_tick < complete && !self.stopped {
            let mut clock = TickClock {
                tick: self.next_tick,
                tick_ns,
            };
            let mut hook = Hook {
                state: &mut self.state,
                session: &mut self.session,
                learning: self.params.learning,
                step: f64::from(self.params.step),
                min_precision: f64::from(self.params.min_precision),
                boundaries: &mut self.boundaries,
                ran: false,
                changed: false,
            };
            let mut ports = Ports {
                clock: &mut clock,
                sense: &mut self.sense,
                field: &mut field,
                effector: &mut self.effector,
                ledger: &mut self.ledger,
                trace: &mut trace,
                plasticity: &mut hook,
            };
            let stepped = self.medium.step(&mut ports);
            let (ran, changed) = (hook.ran, hook.changed);
            if stepped.is_err() {
                self.stats.step_errors += 1;
                self.stopped = true;
            }
            self.stats.boundaries = self.boundaries;
            self.next_tick += 1;
            if changed {
                self.swap();
            }
            if self.params.learning {
                self.learner_ops += self.session.take_ops();
                if self.params.carry && ran {
                    carry::store(self.params.state_key, &self.state);
                }
            }
        }
    }

    fn held(&self, id: u32) -> Option<&Held> {
        let i = self.recent.partition_point(|h| h.id.0 < id);
        self.recent.get(i).filter(|h| h.id.0 == id)
    }

    fn effect(&mut self, now: Instant) -> Vec<usize> {
        let mut created = Vec::new();
        for (_, p) in std::mem::take(&mut self.effector.proposals) {
            match p.kind {
                KIND_NOTICE => {
                    self.stats.notice_proposals += 1;
                    if self.layout.ramp_notice.contains(&p.cell) {
                        self.stats.ramp_proposals += 1;
                    }
                    let anchor = ObsId(p.anchor.seq);
                    if self.anomalies.iter().any(|a| a.anchor == anchor) {
                        self.stats.duplicate_notices += 1;
                        continue;
                    }
                    let Some(first) = self.held(anchor.0).cloned() else {
                        self.stats.lost_anchors += 1;
                        continue;
                    };
                    let Some(site) = service_of(&first.obs) else {
                        self.stats.lost_anchors += 1;
                        continue;
                    };
                    let mut a = Tracked::new(
                        self.next_id,
                        &first,
                        site,
                        &self.services,
                        self.cfg.burst_gap_ns,
                    );
                    let ids: BTreeSet<u32> = p
                        .refs
                        .iter()
                        .map(|r| r.seq)
                        .filter(|id| *id > anchor.0)
                        .collect();
                    for id in ids {
                        if let Some(h) = self.held(id).cloned()
                            && let Some(s) = service_of(&h.obs)
                        {
                            a.note_attached(&h, s, self.cfg.burst_gap_ns);
                        }
                    }
                    a.noticed_at = Some(now);
                    if let Some(node) = self.layout.emitter_node(p.cell) {
                        self.origin.insert(a.id, node);
                    }
                    self.next_id += 1;
                    self.anomalies.push(a);
                    created.push(self.anomalies.len() - 1);
                }
                KIND_RETIRE => {
                    self.stats.retire_proposals += 1;
                    if let Some(&node) = self.layout.latches.get(&p.cell) {
                        for a in &self.anomalies {
                            if a.site.0 == node || self.origin.get(&a.id) == Some(&node) {
                                self.retiring.insert(a.id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        created
    }

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
        let keep = self.cfg.retain_ns;
        while self
            .recent
            .front()
            .is_some_and(|h| h.at.0.saturating_add(keep) < now.0)
        {
            self.recent.pop_front();
        }
        while self
            .unattached
            .front()
            .is_some_and(|h| h.at.0.saturating_add(REOFFER_NS) < now.0)
        {
            self.unattached.pop_front();
        }
    }
}

impl Noticer for LearnedNoticer {
    fn id(&self) -> &'static str {
        LEARNED_ID
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
            self.run_ticks(now);
            let created = self.effect(now);
            self.reoffer(&created);
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

    fn retirable(&self, _now: Instant) -> Vec<u32> {
        self.anomalies
            .iter()
            .filter(|a| self.retiring.contains(&a.id))
            .map(|a| a.id)
            .collect()
    }

    fn retire(&mut self, id: u32) {
        self.anomalies.retain(|a| a.id != id);
        self.retiring.remove(&id);
        self.origin.remove(&id);
    }

    fn take_cost(&mut self) -> Option<NoticerCost> {
        let ops = std::mem::take(&mut self.learner_ops);
        self.learner_ops_total += ops;
        let medium = self.ledger.take_ns(&self.prices);
        if medium.is_none() && ops == 0 {
            return None;
        }
        Some(NoticerCost {
            component: LEARNED_COMPONENT,
            compute_ns: medium.unwrap_or(0) + ops * LEARNER_OP_NS,
        })
    }

    fn refused(&mut self) {
        self.stopped = true;
    }
}
