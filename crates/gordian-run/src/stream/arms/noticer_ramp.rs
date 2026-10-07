//! The ramp noticer (work item B3): a per-node trend detector on counter readings, benign ones
//! included, composed with a base noticer.
//!
//! # The failure it answers
//!
//! B1 and B2 measured that no noticer that triggers on the public rules' "abnormal" verdict can
//! anchor the slow leak: its early readings are benign (a counter below `HIGH`), so the first
//! abnormal observation is seconds after the incident's first observation, and the notice, when
//! there is one, is anchored late. R10's ceiling puts the value of noticing the leak at its first
//! reading at 0.75 of quality. The question this noticer puts is whether a *public* rule that
//! reads the values, not the verdicts, can notice a leak within the background budget, and at what
//! price in false notices. It is the status quo's chance at the leak, built before the medium is
//! asked to claim it.
//!
//! # What it is, in one paragraph
//!
//! For each counter at each service (a *key*) the noticer follows **chains**: runs of readings that
//! continue each other smoothly. A reading continues a chain when it follows the chain's last
//! accepted reading within `gap_ns` and its value is no more than `max_step` above and no more than
//! `max_drop` below the chain's level (the last accepted value). A chain with at least
//! `min_readings` readings whose level is at least `min_rise` above its first reading's value is a
//! **ramp**, and a ramp is noticed once, anchored on the chain's first reading (the earliest
//! observation of the rise), about the service of the key. It composes with a base noticer
//! ([`super::noticer_rung::RungBased`]): the base notices as it always does, and the anomalies this
//! noticer opens join the base's set, so the base's attach rule, score, retirement and the rung's
//! downstream (working state, rule, declaring, context) treat them as any other.
//!
//! # The readings, stated before any tuning
//!
//! Every word of the rule is a reading of "notices when a node's reading has risen monotonically,
//! or by more than a slope threshold, over a window, anchored at the earliest observation of the
//! rise". Each is a parameter or is fixed here.
//!
//! - **What is read.** Counter readings only, every one the rung holds, whatever the public rules'
//!   verdict on it: the rise of a benign counter is the signal, so the verdict is not consulted.
//!   Messages, snapshots, probe results and corrections are not read. The reading's value is the
//!   counter's `u64`. Readings are processed in delivery order (observation id); the instant used is
//!   the reading's own, never the instant of the step, so the result does not depend on how often
//!   the harness calls the noticer, except that a notice is stamped with the instant of the step
//!   that processes the reading that completes the ramp.
//! - **Key.** The pair (service, counter name). A rise at one counter of one node is not joined
//!   with another's.
//! - **Monotone, or a slope.** Neither word alone. A chain tolerates a reading's falling by up to
//!   `max_drop` (`max_drop = 0` is non-decreasing: a reading equal to the level continues the
//!   chain) and its rising by at most `max_step`, per reading. The cap on a step is what makes a
//!   smooth rise a ramp and a jump a different thing: the public rules' own abnormal readings in a
//!   burst jump by tens. The net rise `min_rise` over at least `min_readings` readings is the
//!   slope threshold: it is a rise per number of readings, and the readings' spacing is bounded by
//!   `gap_ns`, so it is a slope in units per second only to within that bound.
//! - **Window.** There is no fixed window. A chain lives while its readings keep arriving within
//!   `gap_ns` of each other, and ends when one does not (the next reading at the key, if any,
//!   starts afresh). `gap_ns` is the one time parameter. A chain dropped for silence is dropped the
//!   next time a reading at its key arrives.
//! - **Outliers.** A reading that does not continue any live chain at its key does not end the
//!   chains: it starts a chain of its own, and the chains it did not continue wait for the next
//!   reading. So a single stray reading in the middle of a ramp (the key's other readings, from
//!   whatever else touches that counter) leaves the ramp's chain as it was, and the stray's own
//!   chain dies for want of followers. At most [`MAX_CHAINS_PER_KEY`] chains are live at a key;
//!   when a fifth would start, the one dropped is an un-noticed chain with the fewest readings, the
//!   oldest last reading breaking a tie.
//! - **Which chain a reading continues.** When it continues several, the longest (most readings),
//!   the earliest started on a tie.
//! - **Earliest observation of the rise.** The first reading of the chain that crosses. A stray
//!   that a ramp's first reading continues (within `gap_ns`, a step within bounds) is the chain's
//!   first reading and so the anchor; this is a limit of the reading, not hidden: it moves the
//!   anchor earlier by at most `gap_ns`.
//! - **Noticed once per chain.** At the step that processes the reading that makes the chain
//!   satisfy `min_readings` and `min_rise`, the chain's readings so far are attached to a new
//!   anomaly anchored on the first, with the service of the key as its site, noticed at that step.
//!   Readings that continue the chain afterwards are attached to the anomaly while it is tracked, so
//!   that it stays alive for as long as the ramp lasts (the rung retires an anomaly after a quiet
//!   time without an attached observation) and the attach rule sees the site as speaking.
//!   Readings that do not continue the chain are not attached. A chain that has crossed does not
//!   cross again; a later chain at the same key may.
//! - **Adoption.** An un-noticed candidate of the base at the same service whose every attached
//!   observation is among the ramp anomaly's is removed when the ramp anomaly is opened: it was
//!   the base's partial view of the same thing and would otherwise be noticed later as a second
//!   anomaly about it. A candidate with any other attached observation stays.
//! - **Retirement and score** are the base's: the anomaly is in the base's set and is quiet when no
//!   observation has been attached to it for the rung's quiet time.
//! - **What a ramp is not.** A rise that begins above `HIGH` is still a ramp if its steps are
//!   smooth; a ramp that is flat and then rises is a chain from the flat part, and its anchor is
//!   the flat part's first reading if every step is within bounds. A ramp too slow for `min_rise`
//!   over the chain's life is never noticed, and a chain longer than [`MAX_CHAIN_READINGS`]
//!   readings without crossing is dropped.
//!
//! # How the readings were chosen
//!
//! Written after looking at the public readings of the slow leaks of the tuning streams
//! (10000-10099: the instants of the incidents' first observations come from the evaluator's output on
//! those streams, and the readings of the dumped public stream around them): a leak shows as a run of
//! readings of one counter at one service, a second or so apart, rising by a few units a reading with
//! some noise, with an occasional unrelated reading of the same key among them. The chain rule
//! (smooth steps, tolerated dips, a gap, an outlier that does not end the run) is a description of that
//! picture in terms of values and instants, with its numbers left as parameters. Nothing was read from
//! `HIDDEN-DESIGN.md`. The held-out streams were not looked at before the parameters were chosen.
//!
//! # What it is not
//!
//!
//! It is not a model of any incident family. It is a threshold on the shape of a counter's recent
//! readings; whether it notices a given incident depends on the incident's readings having that
//! shape, which is an empirical matter the evaluator measures. Its parameters were chosen on the
//! tuning streams against the evaluator's measures, and the choice and the sensitivity are in the
//! B3 report. It reads no label, no incident, no tier and nothing about hidden structure: the
//! observations as delivered, the public rules' verdict on each (which it does not use), the
//! public graph (through the base), and the instant.

use super::noticer::{Notice, Noticer, RetireCause, Tracked};
use super::noticer_follow::Follower;
use super::noticer_rung::{RungBased, RungNoticer};
use super::rung::{Held, Store, service_of};
use gordian_core::Instant;
use gordian_world::{CounterName, Observation, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The most chains alive at one key at a time.
pub const MAX_CHAINS_PER_KEY: usize = 4;

/// A chain with more readings than this that has not crossed is dropped.
pub const MAX_CHAIN_READINGS: u32 = 256;

/// The parameters of the ramp noticer. Every one is written to a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampSpec {
    /// The longest silence, nanoseconds, between two readings of a chain: a reading later than this
    /// after the chain's last does not continue it.
    pub gap_ns: u64,
    /// The most a reading may be above the chain's level and continue it.
    pub max_step: u32,
    /// The most a reading may be below the chain's level and continue it. Zero: non-decreasing.
    pub max_drop: u32,
    /// The fewest readings, the first included, of a chain that is a ramp. At least 2.
    pub min_readings: u32,
    /// The least amount the chain's level must be above its first reading's value for it to be a
    /// ramp. At least 1.
    pub min_rise: u32,
    /// The follow-up rule on a ramp-noticed anomaly (work item B4), if the noticer has one. Not
    /// written when absent, so a manifest written before B4 is the same text as one written now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<super::noticer_follow::FollowSpec>,
}

impl RampSpec {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(f) = &self.follow {
            f.validate()?;
        }
        if self.gap_ns == 0 {
            return Err("noticer ramp: gap_ns must be at least 1".to_owned());
        }
        if self.min_readings < 2 {
            return Err("noticer ramp: min_readings must be at least 2".to_owned());
        }
        if self.min_rise < 1 {
            return Err("noticer ramp: min_rise must be at least 1".to_owned());
        }
        Ok(())
    }
}

/// Whether a reading of `value` continues a chain whose level is `level`: at most `max_step`
/// above it and at most `max_drop` below it, an equal value included. The value part of the rule
/// ([`RampDetector`] adds the time part).
pub fn continues(level: u64, value: u64, max_drop: u32, max_step: u32) -> bool {
    if value >= level {
        value - level <= u64::from(max_step)
    } else {
        level - value <= u64::from(max_drop)
    }
}

/// Where a chain is: the pair of a service and a counter.
pub type Key = (ServiceId, CounterName);

/// What feeding one reading to the detector came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Fed {
    /// Nothing the caller must do: the reading started a chain, continued one that has not
    /// crossed, or continued none that matters.
    Nothing,
    /// The reading continued a chain that has crossed and been linked to this anomaly: attach it.
    Extends(u32),
    /// The reading made a chain cross. The caller opens the anomaly and links it.
    Crossed {
        /// The chain's identity, for [`RampDetector::link`].
        uid: u64,
        /// The chain's key.
        key: Key,
        /// The chain's readings, the first (the anchor) first.
        readings: Vec<Held>,
    },
}

/// A chain not yet linked to an anomaly holds its readings; a linked one holds none.
#[derive(Debug, Clone)]
struct Chain {
    uid: u64,
    first_value: u64,
    level: u64,
    count: u32,
    last_at: Instant,
    readings: Vec<Held>,
    anomaly: Option<u32>,
}

/// The chains at every key, and the arithmetic of [`RampSpec`]. Pure: a function of the readings
/// fed to it, in order.
#[derive(Debug, Clone)]
pub struct RampDetector {
    spec: RampSpec,
    chains: BTreeMap<Key, Vec<Chain>>,
    next_uid: u64,
    readings: u64,
    comparisons: u64,
}

/// The anomaly id a crossed chain holds between [`RampDetector::feed`] and [`RampDetector::link`].
const PENDING: u32 = u32::MAX;

impl RampDetector {
    /// A detector with no chains.
    pub fn new(spec: RampSpec) -> Self {
        Self {
            spec,
            chains: BTreeMap::new(),
            next_uid: 0,
            readings: 0,
            comparisons: 0,
        }
    }

    /// Counter readings fed so far.
    pub fn readings_seen(&self) -> u64 {
        self.readings
    }

    /// Chain comparisons made so far (a reading against a live chain at its key).
    pub fn comparisons(&self) -> u64 {
        self.comparisons
    }

    /// Chains alive now, over every key.
    pub fn live_chains(&self) -> usize {
        self.chains.values().map(Vec::len).sum()
    }

    /// Feed one delivered observation. Anything but a counter reading is [`Fed::Nothing`].
    pub fn feed(&mut self, held: &Held) -> Fed {
        let Observation::Counter {
            service,
            name,
            value,
        } = held.obs
        else {
            return Fed::Nothing;
        };
        self.readings += 1;
        let key = (service, name);
        let gap = self.spec.gap_ns;
        let chains = self.chains.entry(key).or_default();
        chains.retain(|c| held.at.0 <= c.last_at.0.saturating_add(gap));
        let mut pick: Option<usize> = None;
        for (i, c) in chains.iter().enumerate() {
            self.comparisons += 1;
            if continues(c.level, value, self.spec.max_drop, self.spec.max_step)
                && pick.is_none_or(|p| c.count > chains[p].count)
            {
                pick = Some(i);
            }
        }
        let Some(i) = pick else {
            if chains.len() >= MAX_CHAINS_PER_KEY
                && let Some(drop) = (0..chains.len()).min_by_key(|&j| {
                    let c = &chains[j];
                    (c.anomaly.is_some(), c.count, c.last_at)
                })
            {
                chains.remove(drop);
            }
            let uid = self.next_uid;
            self.next_uid += 1;
            chains.push(Chain {
                uid,
                first_value: value,
                level: value,
                count: 1,
                last_at: held.at,
                readings: vec![held.clone()],
                anomaly: None,
            });
            return Fed::Nothing;
        };
        let c = &mut chains[i];
        c.count += 1;
        c.level = value;
        c.last_at = held.at;
        if let Some(anomaly) = c.anomaly {
            return if anomaly == PENDING {
                Fed::Nothing
            } else {
                Fed::Extends(anomaly)
            };
        }
        c.readings.push(held.clone());
        let rise = i128::from(c.level) - i128::from(c.first_value);
        if c.count >= self.spec.min_readings && rise >= i128::from(self.spec.min_rise) {
            c.anomaly = Some(PENDING);
            return Fed::Crossed {
                uid: c.uid,
                key,
                readings: std::mem::take(&mut c.readings),
            };
        }
        if c.count > MAX_CHAIN_READINGS {
            chains.remove(i);
        }
        Fed::Nothing
    }

    /// Tell the detector which anomaly the chain `uid` at `key` was opened as.
    pub fn link(&mut self, key: Key, uid: u64, anomaly: u32) {
        if let Some(c) = self
            .chains
            .get_mut(&key)
            .and_then(|cs| cs.iter_mut().find(|c| c.uid == uid))
        {
            c.anomaly = Some(anomaly);
        }
    }
}

/// A base noticer with the ramp noticer composed over it. See the module documentation.
#[derive(Debug, Clone)]
pub struct RampNoticer<B> {
    inner: B,
    detector: RampDetector,
    seen_through: Option<u32>,
    id: &'static str,
    /// The follow-up rule on the anomalies this noticer opens (work item B4), if the spec has one.
    follow: Option<Follower>,
}

impl<B: RungBased> RampNoticer<B> {
    /// `inner` with ramps noticed as `spec` says; `id` is the id the run output writes.
    pub fn new(inner: B, spec: RampSpec, id: &'static str) -> Self {
        Self {
            inner,
            detector: RampDetector::new(spec),
            seen_through: None,
            id,
            follow: spec.follow.map(Follower::new),
        }
    }

    /// The follow-up rule's state, for a test or a diagnostic (`None` without a follow-up rule).
    pub fn follower(&self) -> Option<&Follower> {
        self.follow.as_ref()
    }

    /// The detector, for a test or a diagnostic.
    pub fn detector(&self) -> &RampDetector {
        &self.detector
    }

    /// Open the anomaly of a chain that crossed: anchored on its first reading, attached its
    /// readings, noticed at `now`, in the base's set. Returns its id.
    fn open(&mut self, readings: &[Held], now: Instant) -> Option<u32> {
        let first = readings.first()?;
        let service = service_of(&first.obs)?;
        let rung = self.inner.rung_mut();
        let gap = rung.config().burst_gap_ns;
        let id = rung.take_id();
        let mut anomaly = Tracked::new(id, first, service, rung.services(), gap);
        for h in &readings[1..] {
            anomaly.note_attached(h, service, gap);
        }
        anomaly.noticed_at = Some(now);
        rung.push(anomaly);
        Some(id)
    }

    /// Remove the base's un-noticed candidates at the service of anomaly `id` whose every attached
    /// observation the anomaly holds (see the module documentation, "Adoption"). Done after every
    /// reading of the step has been processed, so that readings delivered in one step with the one
    /// that completed the ramp are in the anomaly before the candidates are compared with it.
    fn adopt(&mut self, id: u32) {
        let rung = self.inner.rung_mut();
        let list = rung.anomalies_vec_mut();
        let Some(mine) = list.iter().position(|a| a.id == id) else {
            return;
        };
        let (site, owned) = (list[mine].site, list[mine].clone());
        list.retain(|a| {
            !(a.id != id
                && a.noticed_at.is_none()
                && a.site == site
                && a.attached.iter().all(|(_, o, _)| owned.owns(*o)))
        });
    }

    /// Attach a reading that continued a chain that has crossed to the anomaly, if it is still
    /// tracked and does not hold it.
    fn extend(&mut self, anomaly: u32, held: &Held) {
        let Some(service) = service_of(&held.obs) else {
            return;
        };
        let rung = self.inner.rung_mut();
        let gap = rung.config().burst_gap_ns;
        if let Some(a) = rung.anomalies_mut().iter_mut().find(|a| a.id == anomaly)
            && !a.owns(held.id)
        {
            a.note_attached(held, service, gap);
        }
    }
}

impl<B: RungBased> Noticer for RampNoticer<B> {
    fn id(&self) -> &'static str {
        self.id
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        self.inner.observe(held)
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        let mut fresh: Vec<&Held> = store
            .iter()
            .rev()
            .take_while(|h| self.seen_through.is_none_or(|s| h.id.0 > s))
            .collect();
        fresh.reverse();
        let mut opened = Vec::new();
        for held in &fresh {
            // The follow-up rule reads every counter reading at a watched key, whether or not it
            // continues the chain, before the detector sees it: a reading is a follow-up reading
            // of the watches opened by earlier ones, never of the one it completes.
            if let (
                Some(follow),
                Observation::Counter {
                    service,
                    name,
                    value,
                },
            ) = (self.follow.as_mut(), &held.obs)
            {
                follow.feed((*service, *name), *value);
            }
            match self.detector.feed(held) {
                Fed::Nothing => {}
                Fed::Extends(anomaly) => self.extend(anomaly, held),
                Fed::Crossed { uid, key, readings } => {
                    if let Some(id) = self.open(&readings, now) {
                        self.detector.link(key, uid, id);
                        opened.push(id);
                        if let (Some(follow), Observation::Counter { value, .. }) =
                            (self.follow.as_mut(), &held.obs)
                        {
                            follow.open(id, key, *value, held.at);
                        }
                    }
                }
            }
        }
        if let Some(follow) = self.follow.as_mut() {
            follow.expire(now);
        }
        if let Some(last) = fresh.last() {
            self.seen_through = Some(last.id.0);
        }
        for &id in &opened {
            self.adopt(id);
        }
        // Forget the watches and withdrawals of anomalies no longer tracked (adopted into another,
        // or retired since the last step).
        if let Some(follow) = self.follow.as_mut() {
            let inner = &self.inner;
            follow.retain(|a| inner.rung().tracked(a).is_some());
        }
        let mut out = self.inner.notice(now, store);
        for id in opened {
            if let Some(a) = self.inner.rung().tracked(id) {
                out.push(RungNoticer::notice_of(a));
            }
        }
        out
    }

    fn anomalies(&self) -> &[Tracked] {
        self.inner.anomalies()
    }

    fn score(&self, id: u32, now: Instant) -> f64 {
        self.inner.score(id, now)
    }

    fn refresh(&mut self, now: Instant) {
        self.inner.refresh(now);
    }

    fn retirable(&self, now: Instant) -> Vec<u32> {
        let mut out = self.inner.retirable(now);
        if let Some(follow) = &self.follow {
            for id in follow.withdrawn() {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    fn retire(&mut self, id: u32) {
        self.inner.retire(id);
    }

    fn retire_cause(&self, id: u32) -> RetireCause {
        if self.follow.as_ref().is_some_and(|f| f.is_withdrawn(id)) {
            RetireCause::Followup
        } else {
            self.inner.retire_cause(id)
        }
    }
}

impl<B: RungBased> RungBased for RampNoticer<B> {
    fn rung(&self) -> &RungNoticer {
        self.inner.rung()
    }

    fn rung_mut(&mut self) -> &mut RungNoticer {
        self.inner.rung_mut()
    }
}
