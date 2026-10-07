//! The engram layer of the medium noticer (work item A1a, Lab 1): the arm's own answered
//! incidents bound into engrams ([`gordian_medium::engram`]), recalled by coincidence, and
//! declared without asking the reasoner.
//!
//! Everything in this documentation, the key definition and the confirmation policy above all,
//! was written and committed before any run of the arm. A change after a run is recorded in
//! `experiments/exploration/a1a-engram.md` with its reason, never edited in here silently.
//!
//! **Amendment W2 (before any run of the arm; the first form was committed at `715a700`).** The
//! chief relayed four findings of W2 (Lab 3, the world's laws measured from the hidden side): ids
//! of services and free-form messages are regenerated per stream, so they recur only inside one;
//! the law that reaches across streams is the family's, in stream-invariant features; a key made
//! of an incident's first phase collides with plain incidents and decoys; the reasoner is right
//! about half the time on hard incidents at this setting, so one answer is not a memory. The key,
//! the strength rule, the carry and the first values below are the amended ones; what changed is
//! listed in the report (`a1a-engram.md`, "Amendment W2"). No arm had run under the first form.
//!
//! # What the layer is
//!
//! A second medium beside the noticing graph, held by the same noticer
//! ([`super::noticing::MediumNoticer`]), on the same tick, fed from the same delivered
//! observations, with its own sense, clock, effector and ledger ports, and the engram table as its
//! plasticity port. It starts empty: no cell exists until a bind builds one. Its counted
//! operations, its ticks and its plasticity work are priced at the declared prices (200 / 25 /
//! 40 / 2 ns, and 200 ns per tick) and charged to the arm's bill with the noticing graph's, as a
//! component call. Off (`engram` absent from the noticer's parameters), the medium noticer is
//! what it was, byte for byte.
//!
//! # What crosses its sense port
//!
//! Every delivered observation that the public rules call abnormal, and every message, benign
//! ones included (a free-form message is benign under the public rules; its id is a public byte
//! once delivered). Benign counter readings and benign snapshots do not cross: no feature is made
//! of them (below), and a reading that matches no cell would cost a routing and carry nothing.
//! Each becomes the event [`super::adapters::encode`] makes, and an abnormal counter reading also
//! carries its **band** tag ([`band_tag`]).
//!
//! # The key (fixed before any run)
//!
//! For an answer about observation `focus`, the anomaly is the live anomaly of this noticer that
//! owns `focus` (its anchor or an attached observation). If there is none (it retired, or never
//! existed here), nothing is bound. Otherwise, with `s` its site and `t0` its anchor's instant,
//! the key's features are, over the observations this noticer holds that are about `s` and were
//! emitted in `[t0, t0 + key_span_ns]`, in delivery order:
//!
//! - an abnormal observation: its **kind** (which of the five counters, a message, a snapshot;
//!   [`super::adapters::abnormal_kind_tag`]), and, for a counter, its **band** (the shape of the
//!   counters: band 0 below `2 * HIGH`, band 1 below `4 * HIGH`, band 2 from `4 * HIGH`, with
//!   `HIGH` the public alarm line, per counter name);
//! - a catalogue message, abnormal or not: its **message id** (a catalogue id is the first
//!   world's public vocabulary, the same in every stream);
//! - a free-form message: its id ([`super::adapters::message_tag`], folded to 31 bits as the sense
//!   adapter folds it), **in a site-keyed key only**. A free-form id is regenerated per stream
//!   (W2), so it may enter only a key that does not leave the stream (below, "Carry").
//!
//! A feature is **late** when its first occurrence in the span is more than `onset_ns` after
//! `t0`: evidence that arrived after the incident's first phase. The first phase is read
//! publicly as the first burst at the site (`onset_ns`, the rung's `burst_gap_ns`); what makes a
//! hard incident hard has, by the brief's own description of a decoy, not arrived by then.
//! **A key with no late feature is not bound**: a key of first-phase evidence alone is what a
//! plain incident or a decoy shows too (W2: about 1.9 wrong recalls per stream). Repeats are
//! dropped and the first 8 distinct features are kept (the slots of a coincidence), with the
//! first late feature kept in place of the eighth when none of the first eight is late. A key of
//! fewer than `min_features` features is not bound. The **site** is a variable (`site: family`:
//! the features must recur together at one service, whichever) or fixed (`site: site`: at service
//! `s`). Nothing else enters the key: no tier, no label, no incident, no evaluator measure; only
//! bytes this noticer was delivered and the public alarm line.
//!
//! **What a recall waits for.** A recall is the coincidence of every live feature of its key at
//! one service within `key_span_ns`, and every bound key holds a late feature, so a recall waits
//! until evidence beyond the first phase has arrived; with generalisation on, an engram is never
//! narrowed to a key without one of its late features (`Key::with_marks`, `DESIGN.md`).
//!
//! **Not in the key, and why** (W2 names them as invariant): a feature at another service (the
//! partner of a cascade, the second primary of a split brain, and the timing between a site's
//! alarm and its partner's) needs a coincidence across two services, and the engram's key cells
//! bind one service each; probe answers do not cross the noticing seam (the rung admits them to
//! the anomaly that bought them). Both are what a relational key would add, and are not built.
//!
//! # The outcome
//!
//! The answer's diagnosis as a tag ([`outcome_tag`]: 0 for "not an incident", 1 to 5 the known
//! kinds, 6 to 9 the hard kinds, in their declaration order) and a site: none for "not an
//! incident"; the recall's support (substituted at recall) when the diagnosed site is the
//! anomaly's site `s`; otherwise that service, fixed, and **only in a site-keyed key**: a service
//! number means nothing in another stream (W2), so a family-keyed answer naming another site than
//! the anomaly's is not bound (counted).
//!
//! # Recall and declaration
//!
//! A recall is an engram's emitter firing. Its anchor and references are delivered observations;
//! it concerns the noticed anomaly that owns its anchor, else the first that owns one of its
//! references (in the order the noticer created them). A recall that concerns no anomaly yet is
//! kept for 4 s (the noticer's re-offer time) and tried again at each step, then dropped. Of two
//! recalls concerning one anomaly at one step, the stronger is taken (then the lower engram id); an
//! anomaly is recalled at most once. The diagnosis is the outcome's kind at the outcome's site: a
//! support site is the service the recall's anchor observation is about.
//!
//! The arm ([`crate::stream::arms::StreamArm`]) then, for an anomaly that has had no escalation
//! and no answer: **declares** the diagnosis at once (source `recall`; the shared rule no longer
//! declares for it, as for a recogniser), and **never escalates** that anomaly afterwards,
//! whatever its rule asks: a recall yields a `Declare` and no `Escalate`. Unless the recall is
//! confirmed (below).
//!
//! # The confirmation policy (fixed before any run)
//!
//! `confirm` in the parameters, one of:
//!
//! - `never`: no recall is confirmed.
//! - `every` with `k`: the arm's recalls are counted in stream order across segments (the count
//!   is carried with the engrams); the `k`-th, `2k`-th, ... are confirmed.
//! - `on_contradiction`: a recall is confirmed when its engram has been contradicted at least once
//!   (a later answer disagreed with a pattern it keys), or when another engram recalled a
//!   different outcome for the same anomaly at the same step.
//!
//! A confirmed recall is **not declared by the memory**: the arm escalates the anomaly at once,
//! with the rung's context, as any escalation; the answer is declared when it arrives (it is a
//! reasoner's) and is bound like every answer, so it strengthens the engram that recalled or
//! contradicts it. Confirming means asking instead of guessing, not guessing and then asking.
//!
//! # Parameters (fixed before any run, with the reason for each value)
//!
//! | Parameter | First value | Why |
//! |---|---|---|
//! | `onset_ns` | 2 s | the first phase: the rung's `burst_gap_ns`, how long a site must be silent before its next alarm begins a new burst, so 2 s from the anchor is the incident's first burst at its site |
//! | `key_span_ns` | 10 s | half the shortest public hard deadline (20 s, a critical hard incident): a recall that waits for the whole key still leaves half the window for the declaration to land in time |
//! | coincidence window | `key_span_ns` in ticks, rounded up | a recall needs the features within the span they were collected over |
//! | `min_features` | 2 | one feature (one alarm kind) recurs at every incident of that kind; a pattern needs two |
//! | `gain`, `threshold` | 1, 1.5 | the strength is a net count of answers (agreeing minus disagreeing, decayed): one answer is not a memory when the reasoner is right about half the time on hard incidents (W2); a recall needs two more agreeing than disagreeing |
//! | `max_strength` | 4 | four net agreeing answers |
//! | `penalty` | 1 | one disagreeing answer cancels one agreeing one (the vote) |
//! | `decay`, `decay_period_ns` | 0.99 per 100 s | the slow rhythm (section 4b); the family law holds across streams (W2), so forgetting is for a regime change, whose rate is unknown: a strength halves in about 69 cycles (about 11 streams of 600 s) |
//! | `refractory_ns` | 6 s | the rung's `quiet_ns`: one recall per engram per anomaly's life |
//! | `generalise` | on | the count must aggregate over binds of one pattern although its keys differ by background features; intersection is how they aggregate, and it never drops the last late feature |
//! | `carry_site` | off | below |
//!
//! None was chosen against a measure; A1a tunes nothing. A1b may tune them, on its tuning seeds,
//! under its criterion.
//!
//! # Carry
//!
//! The harness plays one fresh arm per segment, segments in stream order. The layer's persisted
//! bytes (the persist port's: the engram medium and its table) and its recall count are kept in a
//! process-wide store keyed by `state_key` (L1's precedent, [`carry`]), written whenever they
//! change and read when a segment begins. At a segment's start the medium is rebuilt from its
//! structure and weights: the engrams carry, their activity does not. With `carry` off the layer
//! starts empty every segment; with `bind` off it never binds, so it never recalls (the control
//! with the layer's cost and no memory).
//!
//! **A site-keyed layer starts empty at every segment** (a segment is one stream, and service
//! numbers and free-form message ids are regenerated per stream, W2: a site-keyed engram carried
//! into the next stream is wrong by construction). `carry_site: true` carries it anyway; that
//! form is a labelled control, never the default. A family-keyed layer carries (its keys hold
//! stream-invariant features only, and its outcome sites are relative).
//!
//! Variable sites range over services 0 to 11 (`MAX_SERVICES`, the public bound on a stream's
//! graph), so an engram built on a stream of 8 services still recalls on a stream of 12.
//!
//! # What this is not
//!
//! It is not told whether a recall was right: the stream never says. A memory-made error is
//! scored by the evaluator as a declaration like any other, and A1b's measures separate it. The
//! arm learns only from answers it asked for, so what it can remember depends on what its rule
//! asks about.

use super::adapters::{
    DeliveredSense, TickClock, TickLedger, abnormal_kind_tag, encode, message_tag,
};
use crate::stream::arms::rung::Held;
use gordian_core::Instant;
use gordian_medium::engram::{pair_bytes, restart, restore_pair};
use gordian_medium::{
    CollectingEffector, ConstantField, EngramParams, Engrams, Field, Key, KeySite, Limits, Medium,
    NoTrace, OpCounts, Outcome, OutcomeSite, Ports, Prices, Recall, Tag,
};
use gordian_stream::{Diagnosis, HardKind, StreamHypothesis, StreamKind};
use gordian_world::physics::{CATALOGUE_LIMIT, HIGH, MAX_SERVICES};
use gordian_world::{CounterName, FaultKind, Observation, ServiceId};
use serde::{Deserialize, Serialize};

/// The kind of a recall proposal in the engram medium.
pub const KIND_RECALL: u16 = 3;

/// The first band tag: `BAND + 4 * counter + band`.
pub const BAND: u32 = 0x200;

/// Where a key's features must recur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteMode {
    /// At one service, whichever (the site is a variable).
    Family,
    /// At the anomaly's own service.
    Site,
}

/// Which recalls the arm asks the reasoner about instead of declaring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfirmPolicy {
    /// None.
    Never,
    /// The `k`-th, `2k`-th, ... recall in stream order.
    Every {
        /// The period, at least 1.
        k: u32,
    },
    /// A recall by an engram that has been contradicted, or that another engram disagrees with.
    OnContradiction,
}

/// The engram layer's parameters, in the medium noticer's `engram` field. See the module
/// documentation for each value and its reason.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngramConfig {
    /// The key under which the layer's state is carried across segments.
    pub state_key: u64,
    /// Whether answers are bound (off: the layer runs and never binds).
    #[serde(default = "yes")]
    pub bind: bool,
    /// Whether the layer's state is carried across segments (a site-keyed layer's only with
    /// `carry_site`).
    #[serde(default = "yes")]
    pub carry: bool,
    /// Carry a site-keyed layer across segments too: a labelled control (module documentation,
    /// "Carry"), never the default.
    #[serde(default)]
    pub carry_site: bool,
    /// Family-keyed or site-keyed.
    pub site: SiteMode,
    /// Generalisation by intersection.
    #[serde(default)]
    pub generalise: bool,
    /// The confirmation policy.
    pub confirm: ConfirmPolicy,
    /// The first phase: a feature first seen more than this after the anchor is late, and a key
    /// needs a late feature, nanoseconds.
    pub onset_ns: u64,
    /// How long after the anchor the key's features are collected, nanoseconds; also the
    /// coincidence window (in ticks, rounded up).
    pub key_span_ns: u64,
    /// Fewest features a key must have.
    pub min_features: u8,
    /// What a bind adds to a strength.
    pub gain: f32,
    /// The strength a recall needs.
    pub threshold: f32,
    /// The most a strength may be.
    pub max_strength: f32,
    /// What a contradiction takes from a strength.
    pub penalty: f32,
    /// The factor a strength is multiplied by at each decay boundary.
    pub decay: f32,
    /// The decay rhythm's period, nanoseconds.
    pub decay_period_ns: u64,
    /// The least time between two recalls of one engram, nanoseconds.
    pub refractory_ns: u64,
}

fn yes() -> bool {
    true
}

impl Default for EngramConfig {
    /// The first values (module documentation, as amended), family-keyed, generalising, never
    /// confirming.
    fn default() -> Self {
        Self {
            state_key: 0,
            bind: true,
            carry: true,
            carry_site: false,
            site: SiteMode::Family,
            generalise: true,
            confirm: ConfirmPolicy::Never,
            onset_ns: 2_000_000_000,
            key_span_ns: 10_000_000_000,
            min_features: 2,
            gain: 1.0,
            threshold: 1.5,
            max_strength: 4.0,
            penalty: 1.0,
            decay: 0.99,
            decay_period_ns: 100_000_000_000,
            refractory_ns: 6_000_000_000,
        }
    }
}

impl EngramConfig {
    /// Whether the layer's state carries across segments: `carry`, and for a site-keyed layer
    /// also `carry_site`.
    pub fn carries(&self) -> bool {
        self.carry && (self.site == SiteMode::Family || self.carry_site)
    }

    /// The crate's parameters for a tick of `tick_ns`.
    pub fn params(&self, tick_ns: u64) -> EngramParams {
        let tick = tick_ns.max(1);
        let ticks = |ns: u64| u32::try_from(ns.div_ceil(tick)).unwrap_or(u32::MAX);
        EngramParams {
            domain: super::adapters::DOMAIN,
            window_ticks: ticks(self.key_span_ns),
            min_features: self.min_features,
            gain: self.gain,
            max_strength: self.max_strength,
            threshold: self.threshold,
            penalty: self.penalty,
            decay: self.decay,
            decay_rhythm: 0,
            refractory_ticks: ticks(self.refractory_ns),
            generalise: self.generalise,
            kind: KIND_RECALL,
        }
    }

    /// Check the parameters for a tick of `tick_ns`, and that an empty engram medium builds.
    pub fn validate(&self, tick_ns: u64) -> Result<(), String> {
        let err = |e: &str| Err(format!("noticer medium engram: {e}"));
        if let ConfirmPolicy::Every { k: 0 } = self.confirm {
            return err("confirm every k needs k >= 1");
        }
        if self.key_span_ns == 0 || self.onset_ns >= self.key_span_ns {
            return err("key_span_ns must be positive and longer than onset_ns");
        }
        if self.decay_period_ns <= tick_ns {
            return err("decay_period_ns must be longer than the tick");
        }
        if let Err(e) = self.params(tick_ns).validate() {
            return err(e);
        }
        let spec = Engrams::medium_spec(
            tick_ns,
            self.decay_period_ns,
            Limits::default(),
            Prices::DECLARED,
        );
        Medium::from_spec(&spec).map_err(|e| format!("noticer medium engram: {e:?}"))?;
        Ok(())
    }
}

/// The band tag of an abnormal counter reading (the shape of the counters), or `None` for any
/// other observation.
pub fn band_tag(obs: &Observation) -> Option<Tag> {
    let Observation::Counter { name, value, .. } = obs else {
        return None;
    };
    if *value < HIGH {
        return None;
    }
    let band = if *value < 2 * HIGH {
        0
    } else if *value < 4 * HIGH {
        1
    } else {
        2
    };
    let c = CounterName::ALL.iter().position(|n| n == name).unwrap_or(0) as u32;
    Some(Tag(BAND + 4 * c + band))
}

/// The tag of a diagnosis: 0 not an incident, 1 to 5 the known kinds, 6 to 9 the hard kinds.
pub fn outcome_tag(diagnosis: &Diagnosis) -> Tag {
    match diagnosis {
        None => Tag(0),
        Some(h) => match h.kind {
            StreamKind::Known(k) => {
                Tag(1 + FaultKind::ALL.iter().position(|x| *x == k).unwrap_or(0) as u32)
            }
            StreamKind::Hard(k) => {
                Tag(6 + HardKind::ALL.iter().position(|x| *x == k).unwrap_or(0) as u32)
            }
        },
    }
}

/// The diagnosis an outcome tag names at `site`; `None` for a tag that names nothing, or a kind
/// with no site.
pub fn diagnosis_of(tag: Tag, site: Option<ServiceId>) -> Option<Diagnosis> {
    let kind = match tag.0 {
        0 => return Some(None),
        t @ 1..=5 => StreamKind::Known(FaultKind::ALL[(t - 1) as usize]),
        t @ 6..=9 => StreamKind::Hard(HardKind::ALL[(t - 6) as usize]),
        _ => return None,
    };
    site.map(|site| Some(StreamHypothesis { kind, site }))
}

/// The outcome of `diagnosis` for an anomaly at `site`.
pub fn outcome_of(diagnosis: &Diagnosis, site: ServiceId) -> Outcome {
    let osite = match diagnosis {
        None => OutcomeSite::None,
        Some(h) if h.site == site => OutcomeSite::Support,
        Some(h) => OutcomeSite::Fixed(u16::try_from(h.site.0).unwrap_or(u16::MAX)),
    };
    Outcome {
        tag: outcome_tag(diagnosis),
        site: osite,
    }
}

/// Whether an observation crosses the engram layer's sense port.
pub fn senses(held: &Held) -> bool {
    held.abnormal || matches!(held.obs, Observation::Message { .. })
}

/// The key features of the observations `held` (already those about the site, in the span, in
/// delivery order), each with whether it is late (first seen after `onset_end`): kinds, bands,
/// catalogue message ids, and free-form message ids when `free_form` (a site-keyed key). Not
/// deduplicated or capped: [`Key::with_marks`] does that.
pub fn features<'a>(
    held: impl IntoIterator<Item = &'a Held>,
    onset_end: Instant,
    free_form: bool,
) -> Vec<(Tag, bool)> {
    let mut out = Vec::new();
    for h in held {
        let late = h.at.0 > onset_end.0;
        if h.abnormal {
            out.push((abnormal_kind_tag(&h.obs), late));
            if let Some(b) = band_tag(&h.obs) {
                out.push((b, late));
            }
        }
        if let Observation::Message { text_id, .. } = h.obs
            && (free_form || text_id < CATALOGUE_LIMIT)
        {
            out.push((message_tag(text_id), late));
        }
    }
    // A feature is late only if it was not seen in the first phase: its first occurrence counts.
    let mut first: Vec<(Tag, bool)> = Vec::new();
    for (t, late) in out {
        if !first.iter().any(|(x, _)| *x == t) {
            first.push((t, late));
        }
    }
    first
}

/// What the layer counted over the segment, for diagnostics and tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EngramLayerStats {
    /// Answers that reached the layer.
    pub answers: u64,
    /// Of those, the ones whose anomaly this noticer no longer held (nothing bound).
    pub answers_unheld: u64,
    /// Binds made (whatever their result).
    pub binds: u64,
    /// Answers not bound: the key had no late feature.
    pub no_late: u64,
    /// Answers not bound: a family-keyed answer naming another site than the anomaly's.
    pub elsewhere: u64,
    /// Recalls the engram medium made.
    pub recalls: u64,
    /// Recalls handed to the arm.
    pub recalls_matched: u64,
    /// Recalls dropped: no anomaly owned them within the re-offer time.
    pub recalls_unmatched: u64,
    /// Recalls for an anomaly already recalled, or beaten by a stronger one at the same step.
    pub recalls_redundant: u64,
    /// Recalls handed to the arm as confirmed.
    pub confirmed: u64,
    /// The carried state could not be read (a defect: should be none); the layer started empty.
    pub carry_errors: u64,
    /// Ticks the engram medium refused (a defect: should be none).
    pub step_errors: u64,
}

/// The engram layer. See the module documentation.
pub struct EngramLayer {
    cfg: EngramConfig,
    tick_ns: u64,
    medium: Medium,
    engrams: Engrams,
    sense: DeliveredSense,
    ledger: TickLedger,
    effector: CollectingEffector,
    prices: Prices,
    next_tick: u64,
    /// Recalls acted on (matched to an anomaly), in stream order across segments: carried.
    recall_count: u64,
    stopped: bool,
    dirty: bool,
    stats: EngramLayerStats,
}

impl std::fmt::Debug for EngramLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngramLayer")
            .field("cfg", &self.cfg)
            .field("engrams", &self.engrams.engrams().len())
            .field("recall_count", &self.recall_count)
            .finish()
    }
}

impl EngramLayer {
    /// A layer for a tick of `tick_ns`: the carried state under `cfg.state_key` if `cfg.carry`
    /// and there is one, else empty.
    pub fn new(cfg: EngramConfig, tick_ns: u64) -> Result<Self, String> {
        cfg.validate(tick_ns)?;
        let params = cfg.params(tick_ns);
        let nodes: Vec<u16> = (0..u16::from(MAX_SERVICES)).collect();
        let empty = || -> Result<(Medium, Engrams), String> {
            let spec = Engrams::medium_spec(
                tick_ns,
                cfg.decay_period_ns,
                Limits::default(),
                Prices::DECLARED,
            );
            let medium = Medium::from_spec(&spec).map_err(|e| format!("{e:?}"))?;
            Ok((medium, Engrams::new(params, &nodes)?))
        };
        let mut stats = EngramLayerStats::default();
        let mut recall_count = 0;
        let (medium, engrams) = match cfg.carries().then(|| carry::load(cfg.state_key)).flatten() {
            None => empty()?,
            Some((count, bytes)) => match restore_pair(&bytes)
                .ok()
                .and_then(|(m, e)| Some((restart(&m).ok()?, e)))
                .filter(|(_, e)| *e.params() == params && e.nodes() == nodes.as_slice())
            {
                Some(pair) => {
                    recall_count = count;
                    pair
                }
                None => {
                    stats.carry_errors += 1;
                    empty()?
                }
            },
        };
        Ok(Self {
            prices: *medium.prices(),
            cfg,
            tick_ns,
            medium,
            engrams,
            sense: DeliveredSense::default(),
            ledger: TickLedger::default(),
            effector: CollectingEffector::default(),
            next_tick: 0,
            recall_count,
            stopped: false,
            dirty: false,
            stats,
        })
    }

    /// The parameters.
    pub fn config(&self) -> &EngramConfig {
        &self.cfg
    }

    /// The engram medium.
    pub fn medium(&self) -> &Medium {
        &self.medium
    }

    /// The engram table.
    pub fn engrams(&self) -> &Engrams {
        &self.engrams
    }

    /// What the layer counted over the segment.
    pub fn stats(&self) -> &EngramLayerStats {
        &self.stats
    }

    /// The ledger port: the engram medium's ticks and counts.
    pub fn ledger(&self) -> &TickLedger {
        &self.ledger
    }

    /// Recalls acted on so far, carried across segments.
    pub fn recall_count(&self) -> u64 {
        self.recall_count
    }

    /// Take in one delivered observation, if it crosses the sense port.
    pub fn take(&mut self, held: &Held) {
        if !senses(held) {
            return;
        }
        if let Some(mut event) = encode(held, self.tick_ns) {
            if let Some(b) = band_tag(&held.obs) {
                event.tags.push(b);
            }
            self.sense.push(event);
        }
    }

    /// Run every tick complete at `now`; the recalls they made.
    pub fn run_ticks(&mut self, now: Instant) -> Vec<Recall> {
        let complete = TickClock::complete_before(self.tick_ns, now);
        let mut field = ConstantField(Field::default());
        let mut trace = NoTrace;
        let decays = self.engrams.stats().decays;
        while self.next_tick < complete && !self.stopped {
            let mut clock = TickClock {
                tick: self.next_tick,
                tick_ns: self.tick_ns,
            };
            let mut ports = Ports {
                clock: &mut clock,
                sense: &mut self.sense,
                field: &mut field,
                effector: &mut self.effector,
                ledger: &mut self.ledger,
                trace: &mut trace,
                plasticity: &mut self.engrams,
            };
            if self.medium.step(&mut ports).is_err() {
                self.stats.step_errors += 1;
                self.stopped = true;
            }
            self.next_tick += 1;
        }
        let mut out = Vec::new();
        for (_, p) in std::mem::take(&mut self.effector.proposals) {
            if let Some(r) = self.engrams.recall(&p) {
                self.stats.recalls += 1;
                out.push(r);
            }
        }
        if !out.is_empty() || self.engrams.stats().decays != decays {
            self.dirty = true;
        }
        self.save_if_dirty();
        out
    }

    /// Bind `features` (with their late marks) at `site` to `diagnosis` (an answer about an
    /// anomaly at `site`). Nothing when binding is off or the layer has stopped, when no feature
    /// is late, or, family-keyed, when the diagnosis names another site.
    pub fn bind(&mut self, features: Vec<(Tag, bool)>, site: ServiceId, diagnosis: &Diagnosis) {
        self.stats.answers += 1;
        if !self.cfg.bind || self.stopped {
            return;
        }
        if !features.iter().any(|(_, late)| *late) {
            self.stats.no_late += 1;
            return;
        }
        if self.cfg.site == SiteMode::Family && diagnosis.is_some_and(|h| h.site != site) {
            self.stats.elsewhere += 1;
            return;
        }
        let key_site = match self.cfg.site {
            SiteMode::Family => KeySite::Variable,
            SiteMode::Site => KeySite::Fixed(u16::try_from(site.0).unwrap_or(u16::MAX)),
        };
        let key = Key::with_marks(features, key_site);
        self.engrams
            .bind(&mut self.medium, &key, outcome_of(diagnosis, site));
        self.stats.binds += 1;
        self.dirty = true;
        self.save_if_dirty();
    }

    /// An answer reached the layer whose anomaly the noticer no longer held.
    pub fn unheld(&mut self) {
        self.stats.answers += 1;
        self.stats.answers_unheld += 1;
    }

    /// Whether the recall of engram `engram` is confirmed under the policy, given whether another
    /// engram disagreed at the same step; counts the recall.
    pub fn acted_on(&mut self, engram: usize, disagreed: bool) -> bool {
        self.recall_count += 1;
        self.stats.recalls_matched += 1;
        self.dirty = true;
        let confirm = match self.cfg.confirm {
            ConfirmPolicy::Never => false,
            ConfirmPolicy::Every { k } => self.recall_count.is_multiple_of(u64::from(k.max(1))),
            ConfirmPolicy::OnContradiction => {
                disagreed
                    || self
                        .engrams
                        .engrams()
                        .get(engram)
                        .is_some_and(|e| e.contradictions > 0)
            }
        };
        if confirm {
            self.stats.confirmed += 1;
        }
        self.save_if_dirty();
        confirm
    }

    /// Count a recall that was not acted on.
    pub fn note_unmatched(&mut self) {
        self.stats.recalls_unmatched += 1;
    }

    /// Count a recall made redundant.
    pub fn note_redundant(&mut self) {
        self.stats.recalls_redundant += 1;
    }

    /// The bill refused: no more ticks and no more binds in the segment.
    pub fn refused(&mut self) {
        self.stopped = true;
    }

    /// The modelled cost of the work since the last call, nanoseconds, and forget it: the
    /// engram medium's counts and ticks (as the noticing graph's) plus the plasticity work, at
    /// the declared prices. `None` when there was none.
    pub fn take_ns(&mut self) -> Option<u64> {
        let work: OpCounts = self.engrams.take_work();
        let ticks = self.ledger.take_ns(&self.prices);
        let plastic = work.modelled_ps(&self.prices) / 1_000;
        match (ticks, plastic) {
            (None, 0) => None,
            (t, p) => Some(t.unwrap_or(0) + p),
        }
    }

    /// The modelled cost of the layer's ticks over the segment, nanoseconds (plasticity work not
    /// included).
    pub fn total_ns(&self) -> u64 {
        self.ledger.total_ns(&self.prices)
    }

    fn save_if_dirty(&mut self) {
        if self.dirty && self.cfg.carries() {
            carry::store(
                self.cfg.state_key,
                self.recall_count,
                pair_bytes(&self.medium, &self.engrams),
            );
        }
        self.dirty = false;
    }
}

pub mod carry {
    //! What the engram layer carries from one segment to the next: its persisted bytes (the
    //! engram medium and its table, `gordian_medium::engram::pair_bytes`) and its recall count,
    //! in a process-wide store keyed by `state_key`, with one writer and one reader, the engram
    //! layer of one key (L1's `learned::carry` is the precedent, and its caveats hold: a replay of
    //! a manifest in a new process reproduces it exactly; two runs in one process under one key do
    //! not, which is why every run names its keys and tests reset them).

    use std::collections::BTreeMap;
    use std::sync::{Mutex, PoisonError};

    type Carried = (u64, Vec<u8>);

    static STORE: Mutex<BTreeMap<u64, Carried>> = Mutex::new(BTreeMap::new());

    /// The recall count and bytes carried under `key`, if any.
    pub fn load(key: u64) -> Option<Carried> {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned()
    }

    /// Keep `count` and `bytes` under `key`.
    pub fn store(key: u64, count: u64, bytes: Vec<u8>) {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, (count, bytes));
    }

    /// Forget what is carried under `key`.
    pub fn reset(key: u64) {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key);
    }
}
