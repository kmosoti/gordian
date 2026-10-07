//! [`MediumNoticer`]: the medium behind B1's `Noticer` seam (work item M2, build items 1 to 3).
//!
//! # What happens at a step
//!
//! The rung calls [`Noticer::observe`] for each delivered abnormal observation, then
//! [`Noticer::notice`] once. In `notice`:
//!
//! 1. **Sense.** Every observation delivered since the last call (the store holds them, in id
//!    order, benign ones included) is encoded ([`super::adapters::encode`]) and queued. With
//!    `abnormal_only` (the control) only the abnormal ones are.
//! 2. **Clock.** Every tick that is complete at the step's instant is run, in order, empty ones
//!    included ([`super::adapters::TickClock`]).
//! 3. **Effector.** The proposals of those ticks become notices and retirements, in the order
//!    the medium made them:
//!    - a proposal of kind notice is a new anomaly: anchor the proposal's anchor, site the
//!      service the anchor is about, attached the anchor and the proposal's references in
//!      delivery order. A proposal whose anchor is the anchor of a live anomaly is the same
//!      anomaly and makes no second notice. (An anchor that a live anomaly merely *holds*, as
//!      evidence attached to it by the rung's rule, does not stop a notice: that is how the rung's
//!      own mis-anchoring would come back, a stray's anomaly absorbing the incident after it;
//!      the observation then belongs to both anomalies, as with B1's `EarliestAnchor`);
//!    - a proposal of kind retire, from the latch that watches service `n`, makes every live
//!      anomaly sited at `n`, or made by `n`'s emitters, ready to retire ([`Noticer::retirable`]);
//!      the rung retires it when nothing is pending on it.
//! 4. **Ledger.** The ticks' operation counts and the ticks themselves are summed for the arm to
//!    charge ([`Noticer::take_cost`]).
//!
//! # Attaching later evidence
//!
//! After a notice, an abnormal observation joins a live anomaly by the rung's own rule
//! ([`attach_target`]: the speaking anomaly at its site, else propagation from the burst that
//! began at its upstream, else the site's anomaly), so that the downstream rung sees the same
//! kind of evidence whichever noticer it runs under. One that joins none is kept for a few
//! seconds and offered again to an anomaly noticed later whose anchor precedes it (the medium
//! notices a tick or more after the evidence, so the rung offers it before the anomaly exists).
//!
//! # The engram layer (work item A1a)
//!
//! With `engram` in the parameters the noticer also holds an engram layer
//! ([`super::engram`]): every delivered abnormal observation and message is taken into it as well,
//! its ticks run after the noticing graph's at each step, and its recalls are resolved to the
//! anomalies noticed so far and handed to the arm ([`Noticer::recalls`]). A reasoner's answer
//! ([`Noticer::answered`]) is bound with the key built from what this noticer holds about the
//! anomaly that owns the answer's focus. Without `engram`, nothing of this exists and the noticer
//! is M2's and M3's, byte for byte.
//!
//! # The recall gate and the two-site key (work item A1c)
//!
//! With the layer's `gate` on, a resolved recall is not handed over at once: it waits in the
//! noticer until the arm shows the noticed anomalies after the step's consistency checks
//! ([`Noticer::gated_recalls`]), and is handed over only if [`super::gate`] admits it (within one
//! review period of the rung), else dropped. With `two_site`, the layer is given the public graph
//! and the rung's burst constants, an answer's key is the two-site key when the anomaly has an
//! unconnected partner, and a pair engram's recall is resolved through the pair the crate names
//! ([`super::engram`], "A1c").
//!
//! # The anticipation layer (work item A2)
//!
//! With `anticipation` in the parameters the noticer also holds an anticipation layer
//! ([`super::anticipation`]): every delivered observation is taken into it at the step that
//! delivers it, its pair-cell medium ticks after the engram layer at each step, and its
//! predictions go to its trace only. With its `attach` switch on, the noticer attaches an abnormal
//! observation through [`MediumNoticer`]'s `attach_index`: the rung's rule, then, where that rule
//! gives the observation to no anomaly or only to its own service's stale one, the layer's learned
//! edges. Without `anticipation`, or with `attach` off, the noticer attaches exactly as before.
//!
//! # Hard limits
//!
//! The medium's own (operations and proposals per tick, cells, synapses, references) are in its
//! spec and always on. The arm's bill is charged for every tick; when it refuses, the medium
//! stops: no more ticks, no more notices in the segment ([`Noticer::refused`]).

use super::adapters::{DeliveredSense, TickClock, TickLedger, encode};
use super::anticipation::Layer as AnticipationLayer;
use super::engram::{EngramLayer, diagnosis_of, features, pair_features};
use super::gate::Reading;
use super::graph::{KIND_NOTICE, KIND_RETIRE, Layout, MediumParams, spec};
use super::trace::{BindNote, NONE, TraceKind};
use crate::stream::arms::noticer::{
    MemoryRecall, Notice, Noticer, NoticerCost, Scorer, Tracked, attach_target,
};
use crate::stream::arms::rung::{AnomalyView, Held, RungConfig, Store, service_of};
use gordian_core::{ComponentId, Instant};
use gordian_medium::{
    CollectingEffector, ConstantField, Field, Medium, NoPlasticity, NoTrace, OutcomeSite, Ports,
    Prices, Recall, pair_of,
};
use gordian_stream::{Diagnosis, ObsId};
use gordian_world::{Service, ServiceId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The noticer's id, as the run output writes it.
pub const MEDIUM_ID: &str = "medium";

/// The id the medium's charges are attributed to on the bill (`Phase::Component`). Not a
/// component of `gordian-components`: the four of those are 0 to 3.
pub const MEDIUM_COMPONENT: ComponentId = ComponentId(16);

/// How long abnormal observations that joined no anomaly are kept to be offered again.
const REOFFER_NS: u64 = 4_000_000_000;

/// What the noticer counted over the segment, for diagnostics and tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediumStats {
    /// Proposals of kind notice.
    pub notice_proposals: u64,
    /// Of those, the ones whose anchor was a live anomaly's anchor.
    pub duplicate_notices: u64,
    /// Of those, the ones whose anchor was no longer held.
    pub lost_anchors: u64,
    /// Proposals of kind retire.
    pub retire_proposals: u64,
    /// Ticks the medium refused (a defect: should be none).
    pub step_errors: u64,
}

/// The medium as a noticer. See the module documentation.
pub struct MediumNoticer {
    params: MediumParams,
    cfg: RungConfig,
    services: Vec<Service>,
    layout: Layout,
    medium: Medium,
    prices: Prices,
    sense: DeliveredSense,
    ledger: TickLedger,
    effector: CollectingEffector,
    next_tick: u64,
    /// The id of the next observation to take from the store.
    cursor: u32,
    /// Observations taken in, in id order, for as long as the rung keeps them.
    recent: VecDeque<Held>,
    /// Abnormal observations that joined no anomaly, to offer to a later notice.
    unattached: VecDeque<Held>,
    scorer: Scorer,
    anomalies: Vec<Tracked>,
    next_id: u32,
    retiring: BTreeSet<u32>,
    /// The service whose emitter made each live anomaly, by id.
    origin: BTreeMap<u32, u32>,
    stopped: bool,
    stats: MediumStats,
    /// The engram layer (A1a), when the parameters name one.
    engram: Option<EngramLayer>,
    /// Recalls not yet owned by a noticed anomaly, with the instant they were made.
    pending_recalls: VecDeque<(Instant, Recall)>,
    /// Anomalies recalled, by id: each is recalled at most once.
    recalled: BTreeSet<u32>,
    /// Recalls resolved and not yet taken by the arm.
    recalls_out: Vec<MemoryRecall>,
    /// Recalls waiting for the gate (A1c), in the order they were resolved.
    gate_pending: Vec<GatedRecall>,
    /// The noticed anomalies that carry a standing declaration, as the rung last said (A1d).
    standing: BTreeSet<u32>,
    /// The anticipation layer (A2), when the parameters name one.
    anticipation: Option<AnticipationLayer>,
}

/// A recall waiting for the gate (A1c).
#[derive(Debug, Clone, Copy)]
struct GatedRecall {
    /// When it was resolved.
    at: Instant,
    anomaly: u32,
    diagnosis: Diagnosis,
    engram: usize,
    disagreed: bool,
    /// The recall's anchor and outcome tag, for the trace (A1d).
    anchor: u32,
    tag: gordian_medium::Tag,
    /// What the gate read at the latest step it read this recall (A1d).
    last: Reading,
}

impl std::fmt::Debug for MediumNoticer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediumNoticer")
            .field("params", &self.params)
            .field("next_tick", &self.next_tick)
            .field("anomalies", &self.anomalies.len())
            .finish()
    }
}

impl MediumNoticer {
    /// A medium noticer for the public graph `services`, with the rung's parameters `cfg` for
    /// what the rung's rules need (attaching, the score). Fails if the graph does not build.
    pub fn new(
        params: MediumParams,
        cfg: RungConfig,
        services: &[Service],
    ) -> Result<Self, String> {
        let (spec, layout) = spec(&params, services)?;
        let medium = Medium::from_spec(&spec).map_err(|e| format!("{e:?}"))?;
        let engram = match params.engram {
            Some(ecfg) => {
                let mut layer = EngramLayer::new(ecfg, params.tick_ns)?;
                layer.set_public(services, cfg.burst_ns, cfg.burst_gap_ns, cfg.retain_ns);
                Some(layer)
            }
            None => None,
        };
        let anticipation = match params.anticipation {
            Some(acfg) => Some(AnticipationLayer::new(
                acfg,
                params.tick_ns,
                services,
                &cfg,
            )?),
            None => None,
        };
        Ok(Self {
            scorer: Scorer::new(&cfg, services.len()),
            prices: *medium.prices(),
            params,
            cfg,
            services: services.to_vec(),
            layout,
            medium,
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
            stats: MediumStats::default(),
            engram,
            pending_recalls: VecDeque::new(),
            recalled: BTreeSet::new(),
            recalls_out: Vec::new(),
            gate_pending: Vec::new(),
            standing: BTreeSet::new(),
            anticipation,
        })
    }

    /// The anticipation layer, if the parameters name one (A2).
    pub fn anticipation(&self) -> Option<&AnticipationLayer> {
        self.anticipation.as_ref()
    }

    /// Where an abnormal observation about `service` at `at` attaches: the rung's rule
    /// ([`attach_target`]); with the anticipation layer's `attach` switch on (A2), where that rule
    /// gives it to no anomaly or only by its last rule (its own service's stale anomaly), the
    /// anomaly a learned edge gives it to, if any. Without the switch, exactly the rung's rule.
    fn attach_index(&mut self, service: ServiceId, at: Instant) -> Option<usize> {
        let base = attach_target(
            &self.anomalies,
            service,
            at,
            self.cfg.burst_ns,
            self.cfg.burst_gap_ns,
        );
        let Some(layer) = self.anticipation.as_mut().filter(|l| l.config().attach) else {
            return base;
        };
        if let Some(i) = base {
            let a = &self.anomalies[i];
            let speaking = at.0
                <= a.last_site_at()
                    .0
                    .saturating_add(self.cfg.burst_gap_ns.saturating_mul(2));
            if a.site != service || speaking {
                return base;
            }
        }
        layer.attach_target(&self.anomalies, service, at).or(base)
    }

    /// The engram layer, if the parameters name one (A1a).
    pub fn engram(&self) -> Option<&EngramLayer> {
        self.engram.as_ref()
    }

    /// The medium.
    pub fn medium(&self) -> &Medium {
        &self.medium
    }

    /// The ledger port: ticks and counts over the segment.
    pub fn ledger(&self) -> &TickLedger {
        &self.ledger
    }

    /// What the noticer counted.
    pub fn stats(&self) -> &MediumStats {
        &self.stats
    }

    /// The modelled cost of the segment so far, nanoseconds: counts at the declared prices plus
    /// the price per tick (the engram layer's ticks included when there is one; its plasticity
    /// work is charged through [`Noticer::take_cost`] only).
    pub fn total_ns(&self) -> u64 {
        self.ledger.total_ns(&self.prices)
            + self.engram.as_ref().map_or(0, EngramLayer::total_ns)
            + self
                .anticipation
                .as_ref()
                .map_or(0, AnticipationLayer::total_ns)
    }

    /// Take in what the store holds past the cursor, at the step of instant `now`.
    fn ingest(&mut self, store: &Store, now: Instant) {
        let fresh: Vec<&Held> = store
            .iter()
            .rev()
            .take_while(|h| h.id.0 >= self.cursor)
            .collect();
        for h in fresh.into_iter().rev() {
            self.cursor = h.id.0.saturating_add(1);
            if (h.abnormal || !self.params.abnormal_only)
                && let Some(event) = encode(h, self.params.tick_ns)
            {
                self.sense.push(event);
            }
            if let Some(layer) = self.engram.as_mut() {
                layer.take(h);
            }
            if let Some(layer) = self.anticipation.as_mut() {
                let owner = self.anomalies.iter().find(|a| a.owns(h.id)).map(|a| a.id);
                layer.take(h, now, owner);
            }
            self.recent.push_back(h.clone());
        }
    }

    /// Run every tick complete at `now`.
    fn run_ticks(&mut self, now: Instant) {
        let complete = TickClock::complete_before(self.params.tick_ns, now);
        let mut field = ConstantField(Field::default());
        let mut trace = NoTrace;
        let mut plasticity = NoPlasticity;
        while self.next_tick < complete && !self.stopped {
            let mut clock = TickClock {
                tick: self.next_tick,
                tick_ns: self.params.tick_ns,
            };
            let mut ports = Ports {
                clock: &mut clock,
                sense: &mut self.sense,
                field: &mut field,
                effector: &mut self.effector,
                ledger: &mut self.ledger,
                trace: &mut trace,
                plasticity: &mut plasticity,
            };
            if self.medium.step(&mut ports).is_err() {
                // A defect, never expected: the medium refused its input. Stop rather than
                // feed it a broken sequence.
                self.stats.step_errors += 1;
                self.stopped = true;
            }
            self.next_tick += 1;
        }
    }

    fn held(&self, id: u32) -> Option<&Held> {
        let i = self.recent.partition_point(|h| h.id.0 < id);
        self.recent.get(i).filter(|h| h.id.0 == id)
    }

    /// Turn the proposals collected so far into notices and retirement marks.
    fn effect(&mut self, now: Instant) -> Vec<usize> {
        let mut created = Vec::new();
        for (_, p) in std::mem::take(&mut self.effector.proposals) {
            match p.kind {
                KIND_NOTICE => {
                    self.stats.notice_proposals += 1;
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

    /// Offer the kept unattached observations to the anomalies just created.
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
            let mut joined = None;
            if h.id > first_anchor
                && !self.anomalies.iter().any(|a| a.owns(h.id))
                && let Some(service) = service_of(&h.obs)
                && let Some(i) = self.attach_index(service, h.at)
                && created.contains(&i)
                && self.anomalies[i].anchor < h.id
            {
                joined = Some((i, service));
            }
            match joined {
                Some((i, service)) => {
                    self.anomalies[i].note_attached(&h, service, self.cfg.burst_gap_ns);
                }
                None => self.unattached.push_back(h),
            }
        }
    }

    /// The anomaly a recall concerns: the noticed anomaly that owns its anchor, else the first
    /// that owns one of its references.
    ///
    /// A pair engram's recall (A1c) concerns the noticed anomaly sited at the pair's first node
    /// that owns its anchor or one of its references, else the first noticed anomaly that owns
    /// one of its observations about that node.
    fn owner_of(&self, r: &Recall) -> Option<u32> {
        let noticed = || self.anomalies.iter().filter(|a| a.noticed_at.is_some());
        if let Some((first, _)) = r.fired.and_then(pair_of) {
            let site = ServiceId(u32::from(first));
            let cited = || std::iter::once(&r.anchor).chain(r.refs.iter());
            return noticed()
                .filter(|a| a.site == site)
                .find(|a| cited().any(|x| a.owns(ObsId(x.seq))))
                .or_else(|| {
                    noticed().find(|a| {
                        cited().any(|x| {
                            a.owns(ObsId(x.seq))
                                && self
                                    .held(x.seq)
                                    .is_some_and(|h| service_of(&h.obs) == Some(site))
                        })
                    })
                })
                .map(|a| a.id);
        }
        noticed()
            .find(|a| a.owns(ObsId(r.anchor.seq)))
            .or_else(|| noticed().find(|a| r.refs.iter().any(|x| a.owns(ObsId(x.seq)))))
            .map(|a| a.id)
    }

    /// Run the engram layer's ticks complete at `now` and resolve its recalls (new and kept) to
    /// the anomalies noticed so far. See [`super::engram`], "Recall and declaration".
    fn engram_step(&mut self, now: Instant) {
        let Some(mut layer) = self.engram.take() else {
            return;
        };
        for r in layer.run_ticks(now) {
            self.pending_recalls.push_back((now, r));
        }
        let mut by_anomaly: BTreeMap<u32, Vec<Recall>> = BTreeMap::new();
        for (at, r) in std::mem::take(&mut self.pending_recalls) {
            match self.owner_of(&r) {
                Some(id) => by_anomaly.entry(id).or_default().push(r),
                None if now.0 <= at.0.saturating_add(REOFFER_NS) => {
                    self.pending_recalls.push_back((at, r));
                }
                None => {
                    layer.note_unmatched();
                    layer.mark_recall(
                        TraceKind::Unmatched,
                        NONE,
                        r.anchor.seq,
                        r.outcome.tag,
                        r.engram,
                    );
                }
            }
        }
        let gated = layer.config().gate.needs_verdicts();
        for (id, mut rs) in by_anomaly {
            if self.recalled.contains(&id) || self.gate_pending.iter().any(|g| g.anomaly == id) {
                for r in &rs {
                    layer.note_redundant();
                    layer.mark_recall(
                        TraceKind::Redundant,
                        id,
                        r.anchor.seq,
                        r.outcome.tag,
                        r.engram,
                    );
                }
                continue;
            }
            rs.sort_by(|a, b| {
                b.strength
                    .total_cmp(&a.strength)
                    .then(a.engram.cmp(&b.engram))
            });
            let best = rs.swap_remove(0);
            let disagreed = rs.iter().any(|r| r.outcome != best.outcome);
            for r in &rs {
                layer.note_redundant();
                layer.mark_recall(
                    TraceKind::Redundant,
                    id,
                    r.anchor.seq,
                    r.outcome.tag,
                    r.engram,
                );
            }
            let site = match (best.outcome.site, best.fired.and_then(pair_of)) {
                (OutcomeSite::None, _) => None,
                // A pair engram's support site is the pair's first node (A1c).
                (OutcomeSite::Support, Some((first, _))) => Some(ServiceId(u32::from(first))),
                (OutcomeSite::Support, None) => {
                    self.held(best.anchor.seq).and_then(|h| service_of(&h.obs))
                }
                (OutcomeSite::Fixed(n), _) => Some(ServiceId(u32::from(n))),
            };
            let (anchor, tag) = (best.anchor.seq, best.outcome.tag);
            let Some(diagnosis) = diagnosis_of(tag, site) else {
                layer.note_unmatched();
                layer.mark_recall(TraceKind::Unmatched, id, anchor, tag, best.engram);
                continue;
            };
            layer.mark_recall(TraceKind::Offered, id, anchor, tag, best.engram);
            if gated {
                // Waits for the arm's views after the step's checks (A1c).
                layer.note_gate_offered();
                self.gate_pending.push(GatedRecall {
                    at: now,
                    anomaly: id,
                    diagnosis,
                    engram: best.engram,
                    disagreed,
                    anchor,
                    tag,
                    last: Reading::Consistent,
                });
                continue;
            }
            layer.mark_recall(TraceKind::Admitted, id, anchor, tag, best.engram);
            let confirm = layer.acted_on(best.engram, disagreed);
            if confirm {
                layer.mark_recall(TraceKind::Confirmed, id, anchor, tag, best.engram);
            }
            self.recalled.insert(id);
            self.recalls_out.push(MemoryRecall {
                anomaly: id,
                diagnosis,
                confirm,
            });
        }
        self.engram = Some(layer);
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

impl Noticer for MediumNoticer {
    fn id(&self) -> &'static str {
        MEDIUM_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        if !held.abnormal {
            return None;
        }
        self.scorer.saw_abnormal();
        let service = service_of(&held.obs)?;
        match self.attach_index(service, held.at) {
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
            self.ingest(store, now);
            self.run_ticks(now);
            let created = self.effect(now);
            self.reoffer(&created);
            self.engram_step(now);
            if let Some(layer) = self.anticipation.as_mut() {
                layer.run_ticks(now);
            }
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
        let own = self.ledger.take_ns(&self.prices);
        let memory = self.engram.as_mut().and_then(EngramLayer::take_ns);
        let pairs = self
            .anticipation
            .as_mut()
            .and_then(AnticipationLayer::take_ns);
        if own.is_none() && memory.is_none() && pairs.is_none() {
            return None;
        }
        Some(NoticerCost {
            component: MEDIUM_COMPONENT,
            compute_ns: own.unwrap_or(0) + memory.unwrap_or(0) + pairs.unwrap_or(0),
        })
    }

    fn refused(&mut self) {
        self.stopped = true;
        if let Some(layer) = self.engram.as_mut() {
            layer.refused();
        }
        if let Some(layer) = self.anticipation.as_mut() {
            layer.refused();
        }
    }

    /// Bind the answer (A1a): the key is built from what this noticer holds about the anomaly
    /// that owns `focus`, over the key span from its anchor at its site ([`super::engram`], "The
    /// key"). Nothing without an engram layer.
    fn answered(&mut self, focus: ObsId, diagnosis: Diagnosis) {
        let Some(mut layer) = self.engram.take() else {
            return;
        };
        match self.anomalies.iter().find(|a| a.owns(focus)) {
            None => {
                layer.unheld();
                layer.mark_answer(NONE, focus.0, &diagnosis, BindNote::Unheld);
            }
            Some(a) => {
                let anomaly = a.id;
                let (from, site) = (a.anchor_at.0, a.site);
                let cfg = *layer.config();
                let to = from.saturating_add(cfg.key_span_ns);
                let onset_end = Instant(from.saturating_add(cfg.onset_ns));
                let free_form = cfg.site == super::engram::SiteMode::Site;
                // The two-site key, when the anomaly has an unconnected partner (A1c).
                if let Some((t_a, partner, t_b, relation)) = cfg
                    .two_site
                    .then(|| layer.partner(site, a.anchor_at))
                    .flatten()
                {
                    let lo = t_a.0.min(t_b.0);
                    let hi = lo.saturating_add(cfg.key_span_ns);
                    let onset_end = Instant(lo.saturating_add(cfg.onset_ns));
                    let relation_late = t_a.0.max(t_b.0) > onset_end.0;
                    let feats = pair_features(
                        self.recent.iter().filter(|h| {
                            h.at.0 >= lo
                                && h.at.0 <= hi
                                && service_of(&h.obs).is_some_and(|s| s == site || s == partner)
                        }),
                        site,
                        relation,
                        relation_late,
                        onset_end,
                    );
                    let note = layer.bind_roles(feats, site, &diagnosis);
                    layer.mark_answer(anomaly, focus.0, &diagnosis, note);
                    self.engram = Some(layer);
                    return;
                }
                let feats = features(
                    self.recent.iter().filter(|h| {
                        h.at.0 >= from && h.at.0 <= to && service_of(&h.obs) == Some(site)
                    }),
                    onset_end,
                    free_form,
                );
                let note = layer.bind(feats, site, &diagnosis);
                layer.mark_answer(anomaly, focus.0, &diagnosis, note);
            }
        }
        self.engram = Some(layer);
    }

    fn recalls(&mut self) -> Vec<MemoryRecall> {
        std::mem::take(&mut self.recalls_out)
    }

    fn needs_verdicts(&self) -> bool {
        self.engram
            .as_ref()
            .is_some_and(|l| l.config().gate.needs_verdicts())
    }

    /// The recall gate (A1c; [`super::gate`]): each waiting recall is dropped if its anomaly was
    /// asked about, answered or retired, or if more than one review period of the rung has passed
    /// since it was resolved; otherwise handed over if the gate admits the anomaly's view, and kept
    /// if not.
    fn gated_recalls(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<MemoryRecall> {
        let Some(mut layer) = self.engram.take() else {
            return self.recalls();
        };
        let gate = layer.config().gate;
        let wait = self.cfg.review_ns;
        let mut keep = Vec::new();
        for mut g in std::mem::take(&mut self.gate_pending) {
            let mark = |layer: &mut EngramLayer, kind: TraceKind, g: &GatedRecall| {
                layer.mark_recall(kind, g.anomaly, g.anchor, g.tag, g.engram);
            };
            let view = views.iter().find(|v| v.id == g.anomaly);
            match view {
                None => {
                    layer.note_gate_overtaken();
                    mark(&mut layer, TraceKind::Overtaken, &g);
                }
                Some(v) if v.attempts > 0 || v.answered > 0 => {
                    layer.note_gate_overtaken();
                    mark(&mut layer, TraceKind::Overtaken, &g);
                }
                Some(_) if now.0 > g.at.0.saturating_add(wait) => {
                    layer.note_gate_closed();
                    let kind = match g.last {
                        Reading::Standing => TraceKind::GatedStanding,
                        _ => TraceKind::GatedConsistent,
                    };
                    mark(&mut layer, kind, &g);
                }
                Some(v) => match gate.reading(v, self.standing.contains(&g.anomaly)) {
                    Reading::Admit => {
                        layer.note_gate_admitted();
                        mark(&mut layer, TraceKind::Admitted, &g);
                        let confirm = layer.acted_on(g.engram, g.disagreed);
                        if confirm {
                            mark(&mut layer, TraceKind::Confirmed, &g);
                        }
                        self.recalled.insert(g.anomaly);
                        self.recalls_out.push(MemoryRecall {
                            anomaly: g.anomaly,
                            diagnosis: g.diagnosis,
                            confirm,
                        });
                    }
                    reading => {
                        g.last = reading;
                        keep.push(g);
                    }
                },
            }
        }
        self.gate_pending = keep;
        self.engram = Some(layer);
        std::mem::take(&mut self.recalls_out)
    }

    /// The rung's standing declarations, read by the `stale` gate (A1d).
    fn standing_declarations(&mut self, ids: &[u32]) {
        if self
            .engram
            .as_ref()
            .is_some_and(|l| l.config().gate.reads_declarations())
        {
            self.standing = ids.iter().copied().collect();
        }
    }

    /// The arm's decision on a recall, for the trace (A1d).
    fn recall_declared(&mut self, now: Instant, anomaly: u32, declared: bool) {
        if let Some(layer) = self.engram.as_mut() {
            layer.mark_declared(now, anomaly, declared);
        }
    }
}
