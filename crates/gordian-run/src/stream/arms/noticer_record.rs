//! The record rung (work item E1, Lab 2): the re-anchor noticer plus a table from a key to the
//! diagnosis last obtained from the reasoner for that key, consulted before the arm escalates.
//! It is the public comparator for the engram of work item A1: what a careful engineer would build
//! to keep records, in two key forms, three confirmation policies and a reset at the boundary of a
//! stream.
//!
//! Everything in this documentation (the keys, the policies, the gate, the tuning rule) was
//! written and committed before any run of the arm. A change after a run is recorded in
//! `experiments/exploration/e1-memory-measures.md` with its reason, never edited in here silently.
//! The author of the arm did not read `crates/gordian-stream/HIDDEN-DESIGN.md`; the arm uses the
//! public rules (`PUBLIC.md`, the first world's rules in `gordian-world`) and what it learns from
//! its own answers, and nothing that exists only on the hidden side of a world.
//!
//! # What it is
//!
//! A [`noticer::Noticer`] that wraps a base noticer (the later re-anchor, by default the
//! configuration B2 selected) and delegates every noticing decision to it, so what is noticed and
//! where it is anchored is exactly the base's. It adds a memory behind the seam the engram of work
//! item A1a uses: the reasoner's answers reach it ([`noticer::Noticer::answered`]) and it yields
//! recalls ([`noticer::MemoryRecall`]) that the arm declares without escalating, or, when the
//! confirmation policy says so, asks about instead.
//!
//! # The gate: it waits for evidence the public rules cannot explain
//!
//! A recall may be made only for an anomaly on which the rung's own consistency checker has found
//! no hypothesis consistent with the evidence attached to it
//! ([`super::rung::AnomalyView::contradicted_since`], the first world's own word for
//! "contradictory"; the public meaning of rule-breaking evidence). The rung keeps that verdict
//! only when asked ([`noticer::Noticer::needs_verdicts`], which this noticer answers yes), runs
//! the consistency verifier through the meter (charged and counted like every component call) and
//! hands the verdicts to [`noticer::Noticer::gated_recalls_in`] after each step's checks. An anomaly
//! the public rules can explain is never recalled for, which is why a recall displaces no
//! declaration the shared rule would have made on such an anomaly (a plain incident the public rules
//! settle), and why a key built from the first seconds of evidence alone, which a plain incident
//! and a decoy also show, never fires. What the gate does not exclude is a decoy or a plain
//! incident whose attached evidence another incident's symptoms made contradictory: those recalls
//! are collisions, and the evaluator counts them (`RULES.md` K6).
//!
//! The gate also decides what is learned. The key of an anomaly is read **once**, when the gate has
//! been open for `settle_ns` (the *snapshot*), and the answer the reasoner later gives for that
//! anomaly is bound to that same key. A key read at the time of the answer instead would hold
//! evidence that arrived after the time a recall can act (the arm asks 16 s after the notice, a
//! recall is made seconds after the first contradiction) and would not match the key at recall
//! time for the same family of incident. An answer about an anomaly that has no snapshot (the gate
//! never opened before it was asked, or it was retired and forgotten) is not bound; it is counted.
//!
//! # The key (fixed before any run)
//!
//! Read from the base noticer's tracked anomaly and the observations the rung holds, at the
//! snapshot, as a sorted list of 64-bit features. `s` is the anomaly's site and `t0` its anchor's
//! instant. Every feature is a function of public bytes the arm was delivered, the public graph
//! and the public alarm line `HIGH`; nothing else enters.
//!
//! The **family-keyed** key holds stream-invariant features only (no service id and no message id,
//! both of which are regenerated per stream); the **site-keyed** key holds the same features and
//! the site's id besides. The features come in three cumulative levels, `level` in the parameters:
//!
//! | Level | Features added |
//! |---|---|
//! | `kinds` | the set of abnormal kinds at `s` (which of the five counters read at or above `HIGH`, which catalogue messages other than `CheckHealth`, whether a configuration snapshot differs from the public graph's), and the kind of the anchor observation |
//! | `bands` | for each abnormal counter at `s`, the band of its highest reading in the evidence: below 2 `HIGH`, below 4 `HIGH`, or above |
//! | `timing` | the band of the gate delay (from `t0` to the first contradicted check): under 1 s, 3 s, 6 s, 10 s, or more |
//!
//! Evidence means the abnormal observations attached to the anomaly by the base noticer that are
//! about `s`, emitted by the snapshot instant. Observations at other services (dependents, a
//! hidden partner) are not in the key: a concurrent incident at an unconnected service would put a
//! partner in the key of one in three recurrences by chance (the stream has an incident about every
//! 22 s and the evidence window is seconds), and a key of exact features must not carry chance.
//! Free-form message ids are benign under the public rules and appear at a site as background
//! too; they enter a site-keyed key only when `msg_ids` is on (off by default, a labelled
//! sensitivity row): the free-form ids at `s` between `t0` and the snapshot. Probe answers do not
//! cross the noticing seam (the rung admits them to the anomaly that bought them) and are not in
//! the key.
//!
//! # The table, and what a recall says
//!
//! A table from key to entry. An entry holds the diagnosis last obtained for the key (the answer
//! as the reasoner gave it, with the site it named), the observation the answer was about (the
//! **source**, handed to the harness for every recall: [`noticer::Noticer::recall_source`]), how
//! many answers have been bound to the key, and whether the entry is *disputed*: set when an answer
//! differs from the one stored, cleared when one agrees. The newest answer replaces the stored one;
//! there is no vote, which is what "the diagnosis last obtained" means. At a recall the table is
//! read with the anomaly's snapshot key:
//!
//! - **Site-keyed**: the stored diagnosis is declared as it is.
//! - **Family-keyed**: the stored answer is bound only if it names no site other than the anomaly's
//!   (or says "not an incident"); a service number means nothing in another stream. The declaration
//!   is the stored kind at the anomaly's own site.
//!
//! A recall is offered at most once per anomaly and only to an anomaly with no escalation and no
//! answer; the arm then declares it at once (source `recall`) and never escalates that anomaly,
//! whatever the selection rule asks, unless the policy confirms it.
//!
//! # The confirmation policies (fixed before any run)
//!
//! `confirm` in the parameters, as the engram's:
//!
//! - `never`: a recall is declared.
//! - `every` with `k`: the recalls of the arm, counted in stream order across segments (the count
//!   is carried even by an arm that resets its table), confirm at the `k`-th, `2k`-th, ...; a
//!   confirmed recall is not declared, the arm asks the reasoner about the anomaly at once, and the
//!   answer is bound like every answer.
//! - `on_contradiction`: a recall is confirmed when its entry is disputed.
//!
//! # Reset at the boundary of a stream
//!
//! `reset` in the parameters. On, the table is empty at the start of every segment: nothing about
//! services, message ids or the graph survives into a stream where they are regenerated. Off, the
//! table is carried to the next segment in a process-wide store keyed by `state_key` (L1's
//! precedent, [`carry`]); a site-keyed table carried is wrong by construction (W2: a site number
//! means another service in the next stream), a labelled control and reported with and without the
//! reset, never tuned away. The store also holds the number of segments the arm has played, and
//! every entry the segment it was bound in, so that a recall can say how many segments ago its
//! source was bound (`age`): the harness then judges the source against the truth of its incident
//! in the stream it was asked in (observation numbers are per stream), which it has kept.
//!
//! # Parameters and the tuning rule (fixed before any run)
//!
//! | Parameter | Value | Why |
//! |---|---|---|
//! | base | the later re-anchor, B2's selection | the memoryless comparator is the same noticer; what differs is the memory |
//! | `settle_ns` | 1 s, not tuned | two checks' worth (the rung's `review_ns` is 0.5 s): more of the first rule-breaking evidence is in the key, and a recall still has most of a deadline of at least 20 s |
//! | `level` | `kinds`, `bands`, `timing`, tuned | the only free parameter of the key |
//! | `k` of `every` | 2, 4, 8, tuned | the only free parameter of a policy |
//! | `msg_ids` | off | below the key |
//!
//! Tuned on seeds 10000 to 10099, in stream order, under W2's stale-error bound for the memory's
//! own errors: of the recalls whose stored answer was right for its own incident (a recall with a
//! right source, `RULES.md` K6), at most 0.20 are wrong, and at most 0.25 such wrong recalls per
//! stream on average. Stage 1: for each form and each reset setting, the three levels under the
//! policy `never`; the level kept is the one with the most hard incidents unasked correct among those
//! meeting the bound (ties to the richer level; a stage-1 configuration with no recall of a right
//! source meets the bound vacuously, and if the best satisfying configuration has no unasked
//! correct hard incident the level kept is `timing`, the finest, and the cell is reported as one
//! the bound leaves empty). Stage 2: at that level, `every` at `k` of 2, 4 and 8 and
//! `on_contradiction`; for `every`, the `k` kept is the one with the most hard incidents unasked
//! correct among those meeting the bound (ties to the smaller `k`), or the smallest collision share
//! when none does. The held-out table (seeds 40000 to 40199, in stream order) runs every cell once
//! with its tuned values, beside the memoryless re-anchor.
//!
//! **Two cases the rule above leaves open, decided before any run** (the script that applies the
//! rule, `experiments/exploration/scripts/e1_select.py`, carries these words): when no level meets
//! the bound in stage 1, the level kept is `timing` and the cell is reported as failing the bound;
//! when no `k` meets it in stage 2, the `k` kept is the one with the smallest collision share, ties
//! to the larger `k`. The statistics the rule reads are `memory.csv`'s: the collision share is
//! `recalls_wrong_source_right` over `recalls_correct_source_right + recalls_wrong_source_right`,
//! pooled over the 100 streams; the count per stream is `recalls_wrong_source_right` summed over
//! the streams and divided by their number; the hard incidents unasked correct are
//! `unasked_correct_hard` summed. Each cell's `never` and `on_contradiction` arms have no free
//! parameter beyond the stage 1 level.
//!
//! # What this is not
//!
//! It is not told whether a recall was right: the stream never says. It learns only from answers it
//! asked for, so what it can remember depends on what its selection rule asks about; under the
//! selection oracle every answer is about a hard incident and the table holds nothing about plain
//! incidents or decoys, and a wrong recall on one is never corrected. Its table lookups and binds
//! are bookkeeping and are not priced in the modelled bill (no calibrated weights); the
//! consistency checks it asks the rung for are, as for every arm that monitors.

use super::noticer::{
    BaseSpec, MemoryRecall, Notice, Noticer, NoticerCost, RecallSource, RetireCause, Tracked,
};
use super::noticer_reanchor::ReanchorNoticer;
use super::noticer_rung::RungNoticer;
use super::rung::{AnomalyView, Held, RungConfig, Store};
use gordian_core::Instant;
use gordian_stream::{Diagnosis, ObsId, StreamHypothesis};
use gordian_world::physics::{CATALOGUE_LIMIT, HIGH, signature};
use gordian_world::{CounterName, Observation, Service, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The id of the site-keyed form, as the run output writes it.
pub const RECORD_SITE_ID: &str = "record_site";
/// The id of the family-keyed form.
pub const RECORD_FAMILY_ID: &str = "record_family";

/// What a key holds besides the evidence at the anomaly's site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyForm {
    /// The site's own id besides (ids are regenerated per stream: it cannot leave one).
    Site,
    /// Features that hold in every stream only.
    Family,
}

/// The richness of the key's features (the module documentation's table), cumulative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyLevel {
    /// The abnormal kinds at the site and the anchor's kind.
    Kinds,
    /// And the band of each abnormal counter's highest reading.
    Bands,
    /// And the band of the gate delay.
    Timing,
}

/// Which recalls are asked about instead of declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordConfirm {
    /// None.
    Never,
    /// The `k`-th, `2k`-th, ... recall of the arm, counted across segments.
    Every {
        /// The period; at least 2.
        k: u32,
    },
    /// A recall whose entry is disputed.
    OnContradiction,
}

/// The record rung's parameters. Every one is written to the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordParams {
    /// The noticer underneath (the later re-anchor B2 selected, in every arm of work item E1).
    pub base: BaseSpec,
    /// The key's form.
    pub form: KeyForm,
    /// The key's level.
    pub level: KeyLevel,
    /// The confirmation policy.
    pub confirm: RecordConfirm,
    /// Reset the table at the start of every segment.
    pub reset: bool,
    /// How long the gate must have been open before the key is read, nanoseconds.
    pub settle_ns: u64,
    /// Add the free-form message ids at the site to a site-keyed key (a labelled sensitivity row).
    #[serde(default)]
    pub msg_ids: bool,
    /// The key under which a table (and the recall count) is carried across segments in the
    /// process-wide store. Distinct per arm.
    pub state_key: u64,
}

impl RecordParams {
    /// The noticer's id: `record_site` or `record_family`.
    pub fn id(&self) -> &'static str {
        match self.form {
            KeyForm::Site => RECORD_SITE_ID,
            KeyForm::Family => RECORD_FAMILY_ID,
        }
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        self.base.validate()?;
        if let RecordConfirm::Every { k } = self.confirm
            && k < 2
        {
            return Err("noticer record: confirm every needs k of at least 2".to_owned());
        }
        if self.msg_ids && self.form != KeyForm::Site {
            return Err("noticer record: msg_ids belongs to a site-keyed key only".to_owned());
        }
        Ok(())
    }
}

/// The feature kinds, in the top byte of a feature.
const F_KIND: u64 = 1;
const F_ANCHOR: u64 = 2;
const F_BAND: u64 = 3;
const F_GATE: u64 = 4;
const F_SNAPSHOT: u64 = 5;
const F_SITE: u64 = 6;
const F_MSG: u64 = 7;

fn feature(kind: u64, payload: u64) -> u64 {
    (kind << 56) | (payload & ((1 << 56) - 1))
}

/// FNV-1a over `text`: a stable code for a symptom tag, whatever order the enum is declared in.
fn code(text: &str) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for b in text.as_bytes() {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn counter_code(name: CounterName) -> u64 {
    match name {
        CounterName::ErrorRate => 0,
        CounterName::Latency => 1,
        CounterName::Saturation => 2,
        CounterName::AuthFailures => 3,
        CounterName::Restarts => 4,
    }
}

/// The band of an abnormal counter reading: below twice the public alarm line, below four times,
/// or above.
pub fn band_of(value: u64) -> u64 {
    if value < 2 * HIGH {
        0
    } else if value < 4 * HIGH {
        1
    } else {
        2
    }
}

/// The band of the gate delay: under 1 s, 3 s, 6 s, 10 s, or more.
pub fn gate_band(delay_ns: u64) -> u64 {
    const S: u64 = 1_000_000_000;
    match delay_ns {
        d if d < S => 0,
        d if d < 3 * S => 1,
        d if d < 6 * S => 2,
        d if d < 10 * S => 3,
        _ => 4,
    }
}

/// The key of an anomaly, as the module documentation defines it: the sorted features of the
/// evidence at `site` held in `evidence` (the abnormal observations attached to it, in delivery
/// order, the anchor first), the gate delay, and for a site-keyed key the site's id.
pub fn key_of(
    params: &RecordParams,
    site: ServiceId,
    evidence: &[(&Held, ServiceId)],
    gate_delay_ns: u64,
    store: &Store,
    anchor_at: Instant,
    now: Instant,
) -> Vec<u64> {
    let mut keys: BTreeSet<u64> = BTreeSet::new();
    let mut bands: BTreeMap<u64, u64> = BTreeMap::new();
    for (i, (held, service)) in evidence.iter().enumerate() {
        if *service != site {
            continue;
        }
        let tags = signature(&[(held.at, held.obs.clone())]);
        if i == 0 {
            for t in &tags {
                keys.insert(feature(F_ANCHOR, code(&format!("{t:?}"))));
            }
        }
        for t in &tags {
            keys.insert(feature(F_KIND, code(&format!("{t:?}"))));
        }
        match &held.obs {
            Observation::Snapshot { .. } => {
                keys.insert(feature(F_SNAPSHOT, 1));
                if i == 0 {
                    keys.insert(feature(F_ANCHOR, 0xFFFF));
                }
            }
            Observation::Counter { name, value, .. } => {
                let best = bands.entry(counter_code(*name)).or_insert(0);
                *best = (*best).max(band_of(*value));
            }
            _ => {}
        }
    }
    if params.level >= KeyLevel::Bands {
        for (counter, band) in bands {
            keys.insert(feature(F_BAND, (counter << 8) | band));
        }
    }
    if params.level >= KeyLevel::Timing {
        keys.insert(feature(F_GATE, gate_band(gate_delay_ns)));
    }
    if params.form == KeyForm::Site {
        keys.insert(feature(F_SITE, u64::from(site.0)));
        if params.msg_ids {
            for h in store.iter() {
                if let Observation::Message {
                    service, text_id, ..
                } = &h.obs
                    && *service == site
                    && *text_id >= CATALOGUE_LIMIT
                    && h.at >= anchor_at
                    && h.at <= now
                {
                    keys.insert(feature(F_MSG, *text_id));
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// What the table holds for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The diagnosis last obtained, as the reasoner gave it (with the site it named).
    pub stored: Diagnosis,
    /// The observation the answer was about.
    pub source: ObsId,
    /// The segment the answer was bound in: the arm's count of the segments it had played before
    /// it (a segment is one stream).
    pub segment: u64,
    /// Answers bound to the key.
    pub answers: u32,
    /// The last answer disagreed with the one before it.
    pub disputed: bool,
}

type Table = BTreeMap<Vec<u64>, Entry>;

/// What the arm counts about its own memory, for tests and reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecordStats {
    /// Answers bound.
    pub binds: u32,
    /// Answers not bound: the anomaly had no snapshot.
    pub unbound_no_snapshot: u32,
    /// Answers not bound: a family-keyed answer naming a site other than the anomaly's.
    pub unbound_other_site: u32,
    /// Snapshots taken.
    pub snapshots: u32,
    /// Lookups made (an anomaly past its settle time, each step until it hits or is asked).
    pub lookups: u32,
    /// Recalls offered (declared or confirmed).
    pub recalls: u32,
    /// Of those, confirmed.
    pub confirmed: u32,
}

struct Snapshot {
    key: Vec<u64>,
    site: ServiceId,
}

/// The record rung's noticer: a base noticer with a table behind the memory seam.
pub struct RecordNoticer<B: Noticer> {
    inner: B,
    params: RecordParams,
    table: Table,
    /// The recalls the arm has made, across segments (the count `every` reads).
    recall_count: u64,
    /// How many segments the arm played before this one: its place in the sequence of streams,
    /// from its own history and nothing else.
    segment: u64,
    snapshots: Vec<Snapshot>,
    by_anomaly: BTreeMap<u32, usize>,
    by_obs: BTreeMap<ObsId, usize>,
    offered: BTreeSet<u32>,
    sources: BTreeMap<u32, RecallSource>,
    stats: RecordStats,
}

impl<B: Noticer> RecordNoticer<B> {
    /// A record noticer over `inner`, starting from the carried table (and recall count) under
    /// `params.state_key` when the arm does not reset.
    pub fn new(inner: B, params: RecordParams) -> Self {
        let (recall_count, segment, carried) =
            carry::load(params.state_key).unwrap_or((0, 0, Table::new()));
        let this = Self {
            inner,
            params,
            table: if params.reset { Table::new() } else { carried },
            recall_count,
            segment,
            snapshots: Vec::new(),
            by_anomaly: BTreeMap::new(),
            by_obs: BTreeMap::new(),
            offered: BTreeSet::new(),
            sources: BTreeMap::new(),
            stats: RecordStats::default(),
        };
        this.persist();
        this
    }

    /// What the memory has done so far.
    pub fn stats(&self) -> RecordStats {
        self.stats
    }

    /// The table as it stands.
    pub fn table(&self) -> &BTreeMap<Vec<u64>, Entry> {
        &self.table
    }

    fn persist(&self) {
        let kept = if self.params.reset {
            Table::new()
        } else {
            self.table.clone()
        };
        carry::store(
            self.params.state_key,
            self.recall_count,
            self.segment + 1,
            kept,
        );
    }

    /// Read the key of anomaly `id` now, the gate open since `since`.
    fn snapshot(&mut self, id: u32, since: Instant, now: Instant, store: &Store) -> Option<usize> {
        let tracked = self.inner.tracked(id)?;
        let ids: BTreeSet<ObsId> = tracked.attached.iter().map(|(_, o, _)| *o).collect();
        let held: BTreeMap<ObsId, &Held> = store
            .iter()
            .filter(|h| ids.contains(&h.id))
            .map(|h| (h.id, h))
            .collect();
        let evidence: Vec<(&Held, ServiceId)> = tracked
            .attached
            .iter()
            .filter_map(|(_, o, s)| held.get(o).map(|h| (*h, *s)))
            .collect();
        let key = key_of(
            &self.params,
            tracked.site,
            &evidence,
            since.0.saturating_sub(tracked.anchor_at.0),
            store,
            tracked.anchor_at,
            now,
        );
        let site = tracked.site;
        let attached: Vec<ObsId> = ids.into_iter().collect();
        let index = self.snapshots.len();
        self.snapshots.push(Snapshot { key, site });
        self.by_anomaly.insert(id, index);
        for o in attached {
            self.by_obs.entry(o).or_insert(index);
        }
        self.stats.snapshots += 1;
        Some(index)
    }

    /// The diagnosis a recall of `entry` declares for an anomaly at `site`.
    fn declared(&self, entry: &Entry, site: ServiceId) -> Diagnosis {
        match self.params.form {
            KeyForm::Site => entry.stored,
            KeyForm::Family => entry
                .stored
                .map(|h| StreamHypothesis { kind: h.kind, site }),
        }
    }
}

impl<B: Noticer> Noticer for RecordNoticer<B> {
    fn id(&self) -> &'static str {
        self.params.id()
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        self.inner.observe(held)
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        self.inner.notice(now, store)
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
        self.inner.retirable(now)
    }

    fn retire(&mut self, id: u32) {
        self.inner.retire(id);
    }

    fn retire_cause(&self, id: u32) -> RetireCause {
        self.inner.retire_cause(id)
    }

    fn take_cost(&mut self) -> Option<NoticerCost> {
        self.inner.take_cost()
    }

    fn refused(&mut self) {
        self.inner.refused();
    }

    fn answered(&mut self, focus: ObsId, diagnosis: Diagnosis) {
        self.inner.answered(focus, diagnosis);
        let Some(&index) = self.by_obs.get(&focus) else {
            self.stats.unbound_no_snapshot += 1;
            return;
        };
        let snap = &self.snapshots[index];
        if self.params.form == KeyForm::Family
            && let Some(h) = diagnosis
            && h.site != snap.site
        {
            self.stats.unbound_other_site += 1;
            return;
        }
        let key = snap.key.clone();
        match self.table.get_mut(&key) {
            Some(entry) => {
                entry.disputed = entry.stored != diagnosis;
                entry.stored = diagnosis;
                entry.source = focus;
                entry.segment = self.segment;
                entry.answers += 1;
            }
            None => {
                self.table.insert(
                    key,
                    Entry {
                        stored: diagnosis,
                        source: focus,
                        segment: self.segment,
                        answers: 1,
                        disputed: false,
                    },
                );
            }
        }
        self.stats.binds += 1;
        self.persist();
    }

    fn recall_source(&self, anomaly: u32) -> Option<RecallSource> {
        self.sources.get(&anomaly).copied()
    }

    fn needs_verdicts(&self) -> bool {
        true
    }

    fn gated_recalls_in(
        &mut self,
        now: Instant,
        store: &Store,
        views: &[AnomalyView],
    ) -> Vec<MemoryRecall> {
        let contradicted: Vec<(u32, Instant)> = views
            .iter()
            .filter_map(|v| v.contradicted_since.map(|since| (v.id, since)))
            .collect();
        self.gated_by(now, store, &contradicted)
    }
}

impl<B: Noticer> RecordNoticer<B> {
    /// The recalls at `now` for the noticed anomalies named in `contradicted` (id and the time
    /// the checker first found no consistent hypothesis, [`AnomalyView::contradicted_since`]).
    pub fn gated_by(
        &mut self,
        now: Instant,
        store: &Store,
        contradicted: &[(u32, Instant)],
    ) -> Vec<MemoryRecall> {
        let mut out = Vec::new();
        for &(id, since) in contradicted {
            if self.offered.contains(&id)
                || now.0 < since.0.saturating_add(self.params.settle_ns)
                || self
                    .inner
                    .tracked(id)
                    .is_none_or(|t| t.noticed_at.is_none())
            {
                continue;
            }
            let index = match self.by_anomaly.get(&id) {
                Some(&i) => i,
                None => match self.snapshot(id, since, now, store) {
                    Some(i) => i,
                    None => continue,
                },
            };
            self.stats.lookups += 1;
            let Some(entry) = self.table.get(&self.snapshots[index].key).copied() else {
                continue;
            };
            let confirm = match self.params.confirm {
                RecordConfirm::Never => false,
                RecordConfirm::Every { k } => {
                    self.recall_count += 1;
                    self.recall_count.is_multiple_of(u64::from(k))
                }
                RecordConfirm::OnContradiction => entry.disputed,
            };
            self.offered.insert(id);
            self.stats.recalls += 1;
            if confirm {
                self.stats.confirmed += 1;
            } else {
                self.sources.insert(
                    id,
                    RecallSource {
                        obs: entry.source,
                        diagnosis: entry.stored,
                        age: u32::try_from(self.segment - entry.segment).unwrap_or(u32::MAX),
                    },
                );
            }
            out.push(MemoryRecall {
                anomaly: id,
                diagnosis: self.declared(&entry, self.snapshots[index].site),
                confirm,
            });
            if matches!(self.params.confirm, RecordConfirm::Every { .. }) {
                self.persist();
            }
        }
        out
    }
}

/// The noticer `params` names, for a stream with the public graph `services`, under the rung's
/// parameters `cfg`.
pub fn build(params: &RecordParams, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    match params.base {
        BaseSpec::Rung { notice_z } => {
            let mut cfg = cfg.clone();
            if let Some(z) = notice_z {
                cfg.notice_z = z;
            }
            Box::new(RecordNoticer::new(RungNoticer::new(cfg, services), *params))
        }
        BaseSpec::Reanchor {
            notice_z,
            gap_ns,
            min_burst,
            isolation,
        } => {
            let mut cfg = cfg.clone();
            if let Some(z) = notice_z {
                cfg.notice_z = z;
            }
            Box::new(RecordNoticer::new(
                ReanchorNoticer::new(cfg, services, gap_ns, min_burst, isolation),
                *params,
            ))
        }
    }
}

/// What the record rung carries from one segment to the next: its recall count, the number of
/// segments it has played (so that a recall can say how many segments ago its source was bound),
/// and, for an arm that does not reset, its table, in a process-wide store keyed by `state_key` with one writer and
/// one reader, the noticer of one arm (L1's `learned::carry` is the precedent, and its caveats
/// hold: a replay of a manifest in a new process reproduces it exactly; two runs in one process
/// under one key do not, which is why every run names its keys and tests reset them).
pub mod carry {
    use super::Table;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, PoisonError};

    type Carried = (u64, u64, Table);

    static STORE: Mutex<BTreeMap<u64, Carried>> = Mutex::new(BTreeMap::new());

    /// The recall count, the segments played and the table carried under `key`, if any.
    pub fn load(key: u64) -> Option<Carried> {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned()
    }

    /// Keep `count`, `segments` (the segments played, this one included) and `table` under `key`.
    pub fn store(key: u64, count: u64, segments: u64, table: Table) {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, (count, segments, table));
    }

    /// Forget what is carried under `key`.
    pub fn reset(key: u64) {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key);
    }
}
