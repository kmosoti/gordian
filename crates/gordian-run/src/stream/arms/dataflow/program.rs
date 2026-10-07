//! The relations and how the rules are evaluated incrementally over them.
//!
//! # The relations
//!
//! Facts come in as observations (one delta per delivered abnormal observation, one per counter
//! reading the rung's store holds past a cursor) and as the instant of a step. Everything else is
//! derived, and kept up to date by deltas rather than recomputed:
//!
//! | Relation | Key | Row | How it is kept |
//! |---|---|---|---|
//! | `anomaly` | anomaly id | site, anchor, timing, noticed instant, peak score | the rules write it; logged |
//! | `member` | (anomaly, order taken) | the attached observation: id, instant, service, payload | the rules write it; logged |
//! | `by_site` | (site, anomaly) | | [`project`] of `anomaly`'s deltas |
//! | `pending` | anomaly | | [`project`] of `anomaly`'s deltas, filtered to not noticed |
//! | `by_time` | (anomaly, instant, order) | | [`project`] of `member`'s deltas: the window index of the score |
//! | `by_obs` | (anomaly, observation) | order taken | [`project`] of `member`'s deltas: the index "owns" reads |
//! | `chain` | (service, counter, chain id) | first value, level, readings, last instant, anomaly | the ramp rule writes it |
//! | `chain_readings` | (chain id, observation) | the reading | until the chain crosses |
//! | `dirty` | anomaly | | marked from `member`'s deltas, consumed by the split |
//! | `timers`, `wake_of` | (instant, anomaly), anomaly | | the split's re-examination instants |
//! | `upstream` | (service, site) | | the public graph, loaded once |
//! | `agg` | counter | the number of abnormal observations delivered | the observe rule |
//!
//! # The schedule of a step
//!
//! [`Program::observe`] runs once per delivered abnormal observation: the attach rule reads
//! `by_site` and `upstream`, and the observation is added to the anomaly it chose or opens one.
//! [`Program::notice`] runs once per step, in this order, each pass followed by [`Program::settle`]
//! (the derived relations catch up with what the pass wrote): the ramp rule over the counter
//! readings the store has past its cursor, then adoption of the candidates the new ramps subsume;
//! the split over the anomalies whose rows changed or whose timer is due; the crossing of the
//! candidates that `pending` holds, each re-anchored on its densest burst and, if the base is the
//! later re-anchor, on the burst after an isolated anchor; the forgetting of stale candidates.
//! Notices are read back out of the relations, the rung's crossings first, then the ramps'.
//!
//! The order is the order the hand-written composition runs its pieces in, because the composed
//! answers depend on it (a ramp's anomaly is in the set before the base's score is read; a split
//! runs before the base notices). It is a requirement of reproduction, not a property the rules
//! have by themselves.
//!
//! # What is incremental here and what is not
//!
//! Incremental: the attach rule reads one site's anomalies and the upstream sites', not all of
//! them; the score reads the rows of one anomaly inside the window through `by_time`, not all its
//! rows; the split examines an anomaly only when its rows changed or the instant its answer
//! depends on has passed; the derived indexes are touched only when their projection changes.
//! Not: crossing evaluates every pending candidate at every step (its score moves with the
//! instant and with the global count); retirability and the peak refresh scan the live anomalies;
//! an examination of one anomaly reads all its rows.

use super::DataflowSpec;
use super::engine::{Delta, OpCounts, Table, project};
use super::rules::{self, ChainRow, Row, ScoreParams, Timing};
use crate::stream::arms::noticer::{BaseSpec, Notice};
use crate::stream::arms::noticer_ramp::{MAX_CHAIN_READINGS, MAX_CHAINS_PER_KEY, RampSpec};
use crate::stream::arms::noticer_reanchor::Isolation;
use crate::stream::arms::noticer_split::SplitSpec;
use crate::stream::arms::rung::{Held, RungConfig, Store, service_of};
use gordian_core::Instant;
use gordian_stream::ObsId;
use gordian_world::graph::dependents_mask;
use gordian_world::{CounterName, Observation, Service, ServiceId};
use std::cmp::Reverse;

/// The anomaly id a crossed chain holds between the crossing and the link.
const PENDING: u32 = u32::MAX;

/// One anomaly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Anomaly {
    pub site: u32,
    pub anchor: u32,
    pub anchor_at: u64,
    pub timing: Timing,
    pub noticed: Option<u64>,
    pub peak: f64,
}

/// One attached observation.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Member {
    pub obs: u32,
    pub at: u64,
    pub svc: u32,
    pub abnormal: bool,
    pub payload: Observation,
}

impl Member {
    fn of(held: &Held, svc: u32) -> Self {
        Self {
            obs: held.id.0,
            at: held.at.0,
            svc,
            abnormal: held.abnormal,
            payload: held.obs.clone(),
        }
    }

    fn row(&self) -> Row {
        Row {
            at: self.at,
            svc: self.svc,
        }
    }

    /// The observation as the rung holds it.
    pub fn held(&self) -> Held {
        Held {
            id: ObsId(self.obs),
            at: Instant(self.at),
            obs: self.payload.clone(),
            abnormal: self.abnormal,
        }
    }
}

type MemberKey = (u32, u64);
type ChainKey = (ServiceId, CounterName, u64);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Chain {
    first_value: u64,
    level: u64,
    count: u32,
    last_at: u64,
    anomaly: Option<u32>,
}

impl Chain {
    fn row(&self) -> ChainRow {
        ChainRow {
            first_value: self.first_value,
            level: self.level,
            count: self.count,
            last_at: self.last_at,
            crossed: self.anomaly.is_some(),
        }
    }
}

/// What feeding one counter reading to the ramp rule came to.
enum Fed {
    Nothing,
    Extends(u32),
    Crossed {
        uid: u64,
        key: (ServiceId, CounterName),
        readings: Vec<Held>,
    },
}

/// What the relations changed since the adapter last read them: the deltas of `anomaly` and
/// `member`.
#[derive(Debug, Default)]
pub(super) struct Changes {
    pub anomalies: Vec<Delta<u32, Anomaly>>,
    pub members: Vec<Delta<MemberKey, Member>>,
}

/// The later re-anchor's parameters.
#[derive(Debug, Clone, Copy)]
struct Reanchor {
    gap_ns: u64,
    min_burst: u32,
    isolation: Isolation,
}

/// The program: the relations, the parameters of the rules and the counters.
pub struct Program {
    notice_z: f64,
    burst_ns: u64,
    burst_gap_ns: u64,
    quiet_ns: u64,
    score: ScoreParams,
    reanchor: Option<Reanchor>,
    ramp: Option<RampSpec>,
    split: Option<SplitSpec>,
    anomaly: Table<u32, Anomaly>,
    member: Table<MemberKey, Member>,
    by_site: Table<(u32, u32), ()>,
    pending: Table<u32, ()>,
    by_time: Table<(u32, u64, u64), ()>,
    by_obs: Table<(u32, u32), u64>,
    chain: Table<ChainKey, Chain>,
    chain_readings: Table<(u64, u32), Held>,
    dirty: Table<u32, ()>,
    timers: Table<(u64, u32), ()>,
    wake_of: Table<u32, u64>,
    upstream: Table<(u32, u32), ()>,
    agg: Table<u8, u64>,
    next_id: u32,
    next_seq: u64,
    next_uid: u64,
    seen_through: Option<u32>,
    fires: u64,
    setup: OpCounts,
    changes: Changes,
}

impl Program {
    /// A program with no facts, for the public graph `services`, under the rung's parameters.
    pub fn new(spec: &DataflowSpec, cfg: &RungConfig, services: &[Service]) -> Self {
        let (z, reanchor) = match spec.base {
            BaseSpec::Rung { notice_z } => (notice_z, None),
            BaseSpec::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            } => (
                notice_z,
                Some(Reanchor {
                    gap_ns,
                    min_burst,
                    isolation,
                }),
            ),
        };
        let mut upstream = Table::new();
        for site in 0..services.len() {
            let mask = dependents_mask(services, ServiceId(site as u32));
            for (svc, hit) in mask.iter().enumerate() {
                if *hit {
                    upstream.put((svc as u32, site as u32), ());
                }
            }
        }
        let mut p = Self {
            notice_z: z.unwrap_or(cfg.notice_z),
            burst_ns: cfg.burst_ns,
            burst_gap_ns: cfg.burst_gap_ns,
            quiet_ns: cfg.quiet_ns,
            score: ScoreParams {
                services: services.len(),
                prior_ns: cfg.prior_ns,
                prior_mhz: cfg.prior_mhz,
                window_ns: cfg.score_window_ns,
            },
            reanchor,
            ramp: spec.ramp,
            split: spec.split,
            anomaly: Table::logged(),
            member: Table::logged(),
            by_site: Table::new(),
            pending: Table::new(),
            by_time: Table::new(),
            by_obs: Table::new(),
            chain: Table::new(),
            chain_readings: Table::new(),
            dirty: Table::new(),
            timers: Table::new(),
            wake_of: Table::new(),
            upstream,
            agg: Table::new(),
            next_id: 0,
            next_seq: 0,
            next_uid: 0,
            seen_through: None,
            fires: 0,
            setup: OpCounts::default(),
            changes: Changes::default(),
        };
        p.setup = p.all_counts();
        p
    }

    fn all_counts(&self) -> OpCounts {
        [
            self.anomaly.counts(),
            self.member.counts(),
            self.by_site.counts(),
            self.pending.counts(),
            self.by_time.counts(),
            self.by_obs.counts(),
            self.chain.counts(),
            self.chain_readings.counts(),
            self.dirty.counts(),
            self.timers.counts(),
            self.wake_of.counts(),
            self.upstream.counts(),
            self.agg.counts(),
        ]
        .into_iter()
        .fold(
            OpCounts {
                fires: self.fires,
                ..OpCounts::default()
            },
            OpCounts::plus,
        )
    }

    /// Everything counted since construction (the loading of the public graph excluded).
    pub fn counts(&self) -> OpCounts {
        self.all_counts().since(self.setup)
    }

    /// The changes to `anomaly` and `member` since the last call.
    pub(super) fn take_changes(&mut self) -> Changes {
        std::mem::take(&mut self.changes)
    }

    /// The anomaly at `id`, not counted: for the adapter.
    pub(super) fn peek_anomaly(&self, id: u32) -> Option<&Anomaly> {
        self.anomaly.peek(&id)
    }

    /// An anomaly's attached observations in the order it took them, not counted: for the adapter.
    pub(super) fn peek_members(&self, id: u32) -> Vec<Member> {
        self.member
            .scan_free((id, 0)..=(id, u64::MAX))
            .map(|(_, m)| m.clone())
            .collect()
    }

    /// The anomalies currently held, by id, not counted: for the adapter and the tests.
    pub fn live_ids(&self) -> Vec<u32> {
        self.anomaly.scan_free(..).map(|(k, _)| *k).collect()
    }

    // ---- settling ---------------------------------------------------------------------------

    /// Bring the derived relations up to date with what the rules wrote, and hand the deltas on to
    /// the adapter.
    pub(super) fn settle(&mut self) {
        let ad = self.anomaly.drain();
        let md = self.member.drain();
        if ad.is_empty() && md.is_empty() {
            return;
        }
        self.fires += project(&ad, &mut self.by_site, |id, a| Some(((a.site, *id), ())));
        self.fires += project(&ad, &mut self.pending, |id, a| {
            a.noticed.is_none().then_some((*id, ()))
        });
        self.fires += project(&md, &mut self.by_time, |k, m| Some(((k.0, m.at, k.1), ())));
        if self.ramp.is_some() {
            self.fires += project(&md, &mut self.by_obs, |k, m| Some(((k.0, m.obs), k.1)));
        }
        if self.split.is_some() {
            for d in &md {
                self.dirty.put(d.key.0, ());
            }
        }
        self.changes.anomalies.extend(ad);
        self.changes.members.extend(md);
    }

    // ---- the observe rule -------------------------------------------------------------------

    /// One delivered abnormal observation: attach it to an anomaly or open one. Returns the
    /// anomaly's id.
    pub fn observe(&mut self, held: &Held) -> Option<u32> {
        if !held.abnormal {
            return None;
        }
        self.fires += 1;
        let seen = self.agg.get(&0).copied().unwrap_or(0) + 1;
        self.agg.put(0, seen);
        let svc = service_of(&held.obs)?.0;
        let id = match self.attach_target(svc, held.at.0) {
            Some(id) => {
                self.attach(id, held, svc);
                id
            }
            None => self.open_candidate(held, svc),
        };
        self.settle();
        Some(id)
    }

    /// Which anomaly an abnormal observation about `svc` at `at` belongs to: the highest-numbered
    /// anomaly sited at `svc` that is still speaking; else the anomaly at an upstream site whose
    /// burst window holds the instant, the most recent burst winning, the lower id on a tie; else
    /// the highest-numbered anomaly sited at `svc`.
    fn attach_target(&self, svc: u32, at: u64) -> Option<u32> {
        let mut highest_here: Option<u32> = None;
        for ((_, id), _) in self.by_site.range((svc, 0)..=(svc, u32::MAX)).rev() {
            let Some(a) = self.anomaly.get(id) else {
                continue;
            };
            highest_here.get_or_insert(*id);
            if rules::speaking(at, a.timing.last_site, self.burst_gap_ns) {
                return Some(*id);
            }
        }
        let mut best: Option<((u64, Reverse<u32>), u32)> = None;
        for ((_, site), _) in self.upstream.range((svc, 0)..=(svc, u32::MAX)) {
            for ((_, id), _) in self.by_site.range((*site, 0)..=(*site, u32::MAX)) {
                let Some(a) = self.anomaly.get(id) else {
                    continue;
                };
                if rules::propagating(at, a.timing.burst_open, self.burst_ns) {
                    let rank = rules::propagation_rank(a.timing.burst_open, *id);
                    if best.as_ref().is_none_or(|(r, _)| rank > *r) {
                        best = Some((rank, *id));
                    }
                }
            }
        }
        best.map(|(_, id)| id).or(highest_here)
    }

    fn put_member(&mut self, id: u32, m: Member) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.member.put((id, seq), m);
    }

    fn attach(&mut self, id: u32, held: &Held, svc: u32) {
        let Some(mut a) = self.anomaly.get(&id).copied() else {
            return;
        };
        a.timing = a.timing.attach(held.at.0, svc, a.site, self.burst_gap_ns);
        self.put_member(id, Member::of(held, svc));
        self.anomaly.put(id, a);
    }

    fn open_candidate(&mut self, held: &Held, svc: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.anomaly.put(
            id,
            Anomaly {
                site: svc,
                anchor: held.id.0,
                anchor_at: held.at.0,
                timing: Timing::first(held.at.0),
                noticed: None,
                peak: f64::NEG_INFINITY,
            },
        );
        self.put_member(id, Member::of(held, svc));
        id
    }

    // ---- the step ---------------------------------------------------------------------------

    /// One step at `now` over the store: the ramp, the split, the crossing and the forgetting, in
    /// that order. Returns the notices of the step.
    pub fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        let opened = if self.ramp.is_some() {
            self.ramp_pass(now, store)
        } else {
            Vec::new()
        };
        if self.split.is_some() {
            self.split_pass(now);
        }
        let crossed = self.cross_pass(now);
        let mut out: Vec<Notice> = crossed
            .iter()
            .filter_map(|&id| self.notice_of(id))
            .collect();
        self.forget_stale(now);
        out.extend(opened.iter().filter_map(|&id| self.notice_of(id)));
        self.settle();
        out
    }

    fn notice_of(&self, id: u32) -> Option<Notice> {
        let a = self.anomaly.get(&id)?;
        Some(Notice {
            id,
            anchor: ObsId(a.anchor),
            anchor_at: Instant(a.anchor_at),
            site: ServiceId(a.site),
            attached: self
                .member
                .range((id, 0)..=(id, u64::MAX))
                .map(|(_, m)| ObsId(m.obs))
                .collect(),
        })
    }

    /// An anomaly's rows with their keys, in the order it took them. Counted.
    fn members(&self, id: u32) -> Vec<(MemberKey, Member)> {
        self.member
            .range((id, 0)..=(id, u64::MAX))
            .map(|(k, m)| (*k, m.clone()))
            .collect()
    }

    fn remove_anomaly(&mut self, id: u32) {
        let keys: Vec<MemberKey> = self
            .member
            .range((id, 0)..=(id, u64::MAX))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            self.member.del(&k);
        }
        self.anomaly.del(&id);
    }

    /// Move the anchor of anomaly `id` to its row `k` (0, or past the end, moves nothing): the rows
    /// before it leave, its service becomes the site, the timing follows.
    fn move_anchor_to(&mut self, id: u32, members: &[(MemberKey, Member)], k: usize) {
        if k == 0 || k >= members.len() {
            return;
        }
        for (key, _) in &members[..k] {
            self.member.del(key);
        }
        let Some(mut a) = self.anomaly.get(&id).copied() else {
            return;
        };
        let rows: Vec<Row> = members[k..].iter().map(|(_, m)| m.row()).collect();
        let first = &members[k].1;
        a.anchor = first.obs;
        a.anchor_at = first.at;
        a.site = first.svc;
        a.timing = a.timing.after_move(&rows, a.site, a.anchor_at);
        self.anomaly.put(id, a);
    }

    // ---- the ramp rule ----------------------------------------------------------------------

    fn ramp_pass(&mut self, now: Instant, store: &Store) -> Vec<u32> {
        let Some(spec) = self.ramp else {
            return Vec::new();
        };
        self.fires += 1;
        let seen = self.seen_through;
        let mut fresh: Vec<&Held> = store
            .iter()
            .rev()
            .take_while(|h| seen.is_none_or(|s| h.id.0 > s))
            .collect();
        fresh.reverse();
        let mut opened = Vec::new();
        for held in &fresh {
            match self.feed(held, &spec) {
                Fed::Nothing => {}
                Fed::Extends(id) => self.extend(id, held),
                Fed::Crossed { uid, key, readings } => {
                    if let Some(id) = self.open_ramp(&readings, now) {
                        self.link(key, uid, id);
                        opened.push(id);
                    }
                }
            }
            self.settle();
        }
        if let Some(last) = fresh.last() {
            self.seen_through = Some(last.id.0);
        }
        for &id in &opened {
            self.adopt(id);
        }
        self.settle();
        opened
    }

    fn drop_chain(&mut self, key: ChainKey) {
        self.chain.del(&key);
        let uid = key.2;
        let readings: Vec<(u64, u32)> = self
            .chain_readings
            .range((uid, 0)..=(uid, u32::MAX))
            .map(|(k, _)| *k)
            .collect();
        for k in readings {
            self.chain_readings.del(&k);
        }
    }

    fn feed(&mut self, held: &Held, spec: &RampSpec) -> Fed {
        let Observation::Counter {
            service,
            name,
            value,
        } = held.obs
        else {
            return Fed::Nothing;
        };
        self.fires += 1;
        let at = held.at.0;
        let mut alive: Vec<(ChainKey, Chain)> = Vec::new();
        let all: Vec<(ChainKey, Chain)> = self
            .chain
            .range((service, name, 0)..=(service, name, u64::MAX))
            .map(|(k, c)| (*k, *c))
            .collect();
        for (k, c) in all {
            if rules::expired(&c.row(), at, spec.gap_ns) {
                self.drop_chain(k);
            } else {
                alive.push((k, c));
            }
        }
        let rows: Vec<ChainRow> = alive.iter().map(|(_, c)| c.row()).collect();
        let Some(i) = rules::continued_chain(&rows, value, spec.max_drop, spec.max_step) else {
            if let Some(v) = rules::eviction_victim(&rows, MAX_CHAINS_PER_KEY) {
                self.drop_chain(alive[v].0);
            }
            let uid = self.next_uid;
            self.next_uid += 1;
            self.chain.put(
                (service, name, uid),
                Chain {
                    first_value: value,
                    level: value,
                    count: 1,
                    last_at: at,
                    anomaly: None,
                },
            );
            self.chain_readings.put((uid, held.id.0), held.clone());
            return Fed::Nothing;
        };
        let (key, mut c) = alive[i];
        c.count += 1;
        c.level = value;
        c.last_at = at;
        if let Some(anomaly) = c.anomaly {
            self.chain.put(key, c);
            return if anomaly == PENDING {
                Fed::Nothing
            } else {
                Fed::Extends(anomaly)
            };
        }
        self.chain_readings.put((key.2, held.id.0), held.clone());
        if rules::is_ramp(&c.row(), spec.min_readings, spec.min_rise) {
            c.anomaly = Some(PENDING);
            self.chain.put(key, c);
            let uid = key.2;
            let taken: Vec<((u64, u32), Held)> = self
                .chain_readings
                .range((uid, 0)..=(uid, u32::MAX))
                .map(|(k, h)| (*k, h.clone()))
                .collect();
            for (k, _) in &taken {
                self.chain_readings.del(k);
            }
            return Fed::Crossed {
                uid,
                key: (service, name),
                readings: taken.into_iter().map(|(_, h)| h).collect(),
            };
        }
        if c.count > MAX_CHAIN_READINGS {
            self.drop_chain(key);
        } else {
            self.chain.put(key, c);
        }
        Fed::Nothing
    }

    fn link(&mut self, key: (ServiceId, CounterName), uid: u64, id: u32) {
        let k = (key.0, key.1, uid);
        if let Some(mut c) = self.chain.get(&k).copied() {
            c.anomaly = Some(id);
            self.chain.put(k, c);
        }
    }

    /// Open the anomaly of a chain that crossed: anchored on its first reading, holding all its
    /// readings, noticed now.
    fn open_ramp(&mut self, readings: &[Held], now: Instant) -> Option<u32> {
        self.fires += 1;
        let first = readings.first()?;
        let svc = service_of(&first.obs)?.0;
        let id = self.next_id;
        self.next_id += 1;
        let mut a = Anomaly {
            site: svc,
            anchor: first.id.0,
            anchor_at: first.at.0,
            timing: Timing::first(first.at.0),
            noticed: None,
            peak: f64::NEG_INFINITY,
        };
        self.put_member(id, Member::of(first, svc));
        for h in &readings[1..] {
            a.timing = a.timing.attach(h.at.0, svc, a.site, self.burst_gap_ns);
            self.put_member(id, Member::of(h, svc));
        }
        a.noticed = Some(now.0);
        self.anomaly.put(id, a);
        Some(id)
    }

    /// Attach a reading that continued a chain that has crossed to its anomaly, if the anomaly is
    /// still held and does not hold the reading.
    fn extend(&mut self, id: u32, held: &Held) {
        let Some(svc) = service_of(&held.obs) else {
            return;
        };
        let Some(mut a) = self.anomaly.get(&id).copied() else {
            return;
        };
        if self.by_obs.contains(&(id, held.id.0)) {
            return;
        }
        a.timing = a.timing.attach(held.at.0, svc.0, a.site, self.burst_gap_ns);
        self.put_member(id, Member::of(held, svc.0));
        self.anomaly.put(id, a);
    }

    /// Remove the candidates at the site of ramp anomaly `id` that are not noticed and whose every
    /// row the ramp anomaly holds: they were a partial view of the same thing.
    fn adopt(&mut self, id: u32) {
        self.fires += 1;
        let Some(site) = self.anomaly.get(&id).map(|a| a.site) else {
            return;
        };
        let here: Vec<u32> = self
            .by_site
            .range((site, 0)..=(site, u32::MAX))
            .map(|((_, other), _)| *other)
            .collect();
        for other in here {
            if other == id {
                continue;
            }
            let Some(a) = self.anomaly.get(&other).copied() else {
                continue;
            };
            if a.noticed.is_some() {
                continue;
            }
            let subsumed = self
                .member
                .range((other, 0)..=(other, u64::MAX))
                .all(|(_, m)| self.by_obs.contains(&(id, m.obs)));
            if subsumed {
                self.remove_anomaly(other);
            }
        }
    }

    // ---- the split --------------------------------------------------------------------------

    fn split_pass(&mut self, now: Instant) {
        let Some(spec) = self.split else {
            return;
        };
        self.fires += 1;
        let due: Vec<(u64, u32)> = self.timers.range(..(now.0, 0)).map(|(k, _)| *k).collect();
        for (w, id) in due {
            self.timers.del(&(w, id));
            self.wake_of.del(&id);
            self.dirty.put(id, ());
        }
        while let Some(id) = self.dirty.first_key() {
            self.dirty.del(&id);
            self.examine(id, now, &spec);
            self.settle();
        }
    }

    fn examine(&mut self, id: u32, now: Instant, spec: &SplitSpec) {
        self.fires += 1;
        if let Some(w) = self.wake_of.get(&id).copied() {
            self.timers.del(&(w, id));
            self.wake_of.del(&id);
        }
        let Some(site) = self.anomaly.get(&id).map(|a| a.site) else {
            return;
        };
        let members = self.members(id);
        let rows: Vec<Row> = members.iter().map(|(_, m)| m.row()).collect();
        let (picks, due) = rules::split_rule(
            &rows,
            site,
            spec.gap_ns,
            spec.min_burst,
            self.burst_ns,
            now.0,
        );
        if let Some(w) = due {
            self.timers.put((w, id), ());
            self.wake_of.put(id, w);
        }
        if let Some(picks) = picks {
            self.split_off(id, &members, &picks);
        }
    }

    /// Move the rows at `picks` out of anomaly `id` into a new anomaly anchored on the first of
    /// them; both anomalies' timing is recomputed from the rows each now holds.
    fn split_off(&mut self, id: u32, members: &[(MemberKey, Member)], picks: &[usize]) {
        let new_id = self.next_id;
        self.next_id += 1;
        let mut take = vec![false; members.len()];
        for &p in picks {
            take[p] = true;
        }
        let mut moved: Vec<Member> = Vec::new();
        let mut kept: Vec<Row> = Vec::new();
        for (i, (key, m)) in members.iter().enumerate() {
            if take[i] {
                self.member.del(key);
                moved.push(m.clone());
            } else {
                kept.push(m.row());
            }
        }
        let Some(mut a) = self.anomaly.get(&id).copied() else {
            return;
        };
        a.timing = Timing::retime(&kept, a.site, a.anchor_at, self.burst_gap_ns);
        self.anomaly.put(id, a);
        let first = moved[0].clone();
        let rows: Vec<Row> = moved.iter().map(Member::row).collect();
        self.anomaly.put(
            new_id,
            Anomaly {
                site: first.svc,
                anchor: first.obs,
                anchor_at: first.at,
                timing: Timing::retime(&rows, first.svc, first.at, self.burst_gap_ns),
                noticed: None,
                peak: f64::NEG_INFINITY,
            },
        );
        for m in moved {
            self.put_member(new_id, m);
        }
    }

    // ---- crossing, the re-anchors and forgetting --------------------------------------------

    fn cross_pass(&mut self, now: Instant) -> Vec<u32> {
        self.fires += 1;
        let candidates: Vec<u32> = self.pending.range(..).map(|(k, _)| *k).collect();
        let crossed: Vec<u32> = candidates
            .into_iter()
            .filter(|&id| self.score_of(id, now.0) >= self.notice_z)
            .collect();
        for &id in &crossed {
            let members = self.members(id);
            let rows: Vec<Row> = members.iter().map(|(_, m)| m.row()).collect();
            let k = rules::densest_start(&rows, self.burst_ns);
            self.move_anchor_to(id, &members, k);
            if let Some(mut a) = self.anomaly.get(&id).copied() {
                a.noticed = Some(now.0);
                self.anomaly.put(id, a);
            }
            if let Some(re) = self.reanchor {
                let members = self.members(id);
                let rows: Vec<Row> = members.iter().map(|(_, m)| m.row()).collect();
                if let Some(k) = rules::later_burst_start(
                    &rows,
                    re.gap_ns,
                    re.min_burst,
                    self.burst_ns,
                    re.isolation,
                ) {
                    self.move_anchor_to(id, &members, k);
                }
            }
        }
        self.settle();
        crossed
    }

    fn forget_stale(&mut self, now: Instant) {
        let candidates: Vec<u32> = self.pending.range(..).map(|(k, _)| *k).collect();
        for id in candidates {
            let stale = self.anomaly.get(&id).is_some_and(|a| {
                now.0 > a.timing.last_abnormal.saturating_add(self.score.window_ns)
            });
            if stale {
                self.remove_anomaly(id);
            }
        }
    }

    /// The score of anomaly `id` at `now`: the rung's z of the rows inside the window, read through
    /// the window index. `NEG_INFINITY` for an anomaly that is not held.
    fn score_of(&self, id: u32, now: u64) -> f64 {
        if !self.anomaly.contains(&id) {
            return f64::NEG_INFINITY;
        }
        let from = now.saturating_sub(self.score.window_ns).saturating_add(1);
        let n = self
            .by_time
            .range((id, from, 0)..=(id, u64::MAX, u64::MAX))
            .count() as u64;
        let seen = self.agg.get(&0).copied().unwrap_or(0);
        rules::z_score(&self.score, seen, now, n)
    }

    // ---- what the seam reads ----------------------------------------------------------------

    /// The score of anomaly `id` at `now`.
    pub fn score(&self, id: u32, now: Instant) -> f64 {
        self.score_of(id, now.0)
    }

    /// Bring every noticed anomaly's peak score up to date with `now`.
    pub fn refresh(&mut self, now: Instant) {
        self.fires += 1;
        let noticed: Vec<(u32, f64)> = self
            .anomaly
            .range(..)
            .filter(|(_, a)| a.noticed.is_some())
            .map(|(k, a)| (*k, a.peak))
            .collect();
        for (id, peak) in noticed {
            let np = peak.max(self.score_of(id, now.0));
            if np != peak
                && let Some(mut a) = self.anomaly.get(&id).copied()
            {
                a.peak = np;
                self.anomaly.put(id, a);
            }
        }
        self.settle();
    }

    /// The noticed anomalies with no abnormal observation for the quiet time, by id.
    pub fn retirable(&self, now: Instant) -> Vec<u32> {
        self.anomaly
            .range(..)
            .filter(|(_, a)| {
                a.noticed.is_some() && now.0 >= a.timing.last_abnormal.saturating_add(self.quiet_ns)
            })
            .map(|(k, _)| *k)
            .collect()
    }

    /// Forget anomaly `id`.
    pub fn retire(&mut self, id: u32) {
        self.fires += 1;
        self.remove_anomaly(id);
        self.settle();
    }
}
