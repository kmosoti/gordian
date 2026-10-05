//! The cheap rung every stream arm shares.
//!
//! EXP-101's intervention is *when and what an arm escalates* and nothing else (charter section
//! 6). Everything that happens before an escalation, and everything that happens to a declaration,
//! is therefore one piece of code, this file, and an arm is [`super::StreamArm`] plus an
//! [`super::EscalationRule`]. What the rung is, in one paragraph:
//!
//! It watches the passive observations for **anomalies** with public statistics only: an
//! abnormal observation is one the first world's public rules call abnormal (a counter at or above
//! `HIGH`, a catalogue message other than `CheckHealth`, a changed snapshot). Abnormal
//! observations are grouped by the public graph into candidate anomalies (same service; or a
//! dependent of the anomaly's site within a short burst window, which is what propagation looks
//! like), each scored by a z-score of its recent abnormal count against a baseline the rung learns
//! from its own history of the stream ([`RungConfig`]). A candidate whose score crosses the notice
//! threshold becomes a **noticed anomaly**. For a noticed anomaly the rung keeps a first-world
//! [`WorkingState`] over the observations at its site and its dependents, runs the first world's
//! heuristic, estimator and verifier on it (the memory lookup is never read by the shared rule and
//! is not run), and lets the shared rule ([`crate::policy::decide::Decider`]) decide: declare,
//! buy a probe, or wait. That is "what the cheap rung concludes".
//!
//! # What the rung does not know
//!
//! Only the first world's public rules (through the components and the rule) and what the stream
//! shows. It knows nothing of the hidden rules of the stream's hard incidents. A hard incident is
//! noticed and concluded about exactly like any other, and its conclusion is wrong when the hidden
//! rules make it wrong; that is the escalation problem. The only arm that knows more is
//! `ablation_hidden_rules` (`ablation.rs`), by construction and by name.
//!
//! # Declarations
//!
//! One procedure for every arm. The cheap rung declares at most once per anomaly (its first
//! conclusion; a lone "no fault" is treated as a dismissal of the anchor), unless the escalation
//! rule holds the declaration back because a reasoner call about the anomaly is in flight. A
//! reasoner answer is declared when it arrives, anchored on the observation it was asked about,
//! and *outranks* the cheap rung: once an answer has been declared for an anomaly the cheap rung
//! never declares for it. An answer identical to the last declaration with the same anchor is not
//! declared again. At the end of the stream every anomaly still undecided gets the rule's final
//! call. Declaring is free in the stream's bill, as in the first world's.

use super::context::{self, ContextBuilder, PublicView};
use super::{Applied, Proposed, Source};
use crate::policy::decide::{DecideConfig, Decider};
use crate::stream::meter::{Meter, RuleCall};
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ComponentOutput, ConsistencyVerifier, CountEstimator, RuleHeuristic, VERIFIER_ID,
    WorkingState,
};
use gordian_core::{ComponentId, Instant};
use gordian_stream::{
    Diagnosis, ObsId, ObsRef, StreamAction, StreamEvent, StreamHypothesis, StreamKind, StreamPublic,
};
use gordian_world::episode::PublicInfo;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{HIGH, SignalText, SymptomTag, signature};
use gordian_world::{Action, Observation, Probe, ProbeResult, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The parameters of the shared cheap rung. One value per run, in the manifest, for every arm: a
/// per-arm value would be a difference in the rung, which EXP-101 holds fixed.
///
/// None of these was tuned against any result: they are placeholders chosen by reading the
/// public rates of the stream (`stream/mod.rs`, "Placeholders").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RungConfig {
    /// The shared decision rule's patience, in nanoseconds **after the anomaly is noticed**. The
    /// first world's rule waits this long from the start of an episode, when its first symptoms
    /// arrive within a second; a noticed anomaly is already seconds old, so measuring from its
    /// anchor would have the rule declare at its deadline the moment it is first looked at, and
    /// never wait for the evidence or the probe that would settle it.
    pub patience_ns: u64,
    /// Capacity of an anomaly's working state, in observations.
    pub window: usize,
    /// Fewest nanoseconds between two reviews (component runs plus a rule call) of one anomaly.
    pub review_ns: u64,
    /// The z-score at which a candidate anomaly is noticed.
    pub notice_z: f64,
    /// The window over which a candidate's abnormal observations are counted for its score.
    pub score_window_ns: u64,
    /// How long after a burst begins at an anomaly's site an abnormal observation at one of the
    /// site's dependents is read as propagation and attached to it; also the span of the cluster
    /// the anchor is chosen from.
    pub burst_ns: u64,
    /// How long a site must have been silent for its next abnormal observation to begin a new
    /// burst. A site's later heartbeats are not propagation events and open no window.
    pub burst_gap_ns: u64,
    /// An anomaly with no abnormal observation for this long is quiet and retires.
    pub quiet_ns: u64,
    /// How long the rung keeps observations it may be asked to put in a context.
    pub retain_ns: u64,
    /// The prior rate of abnormal observations per service, in milli-hertz, before the rung has
    /// seen enough of the stream to estimate it.
    pub prior_mhz: u64,
    /// The weight of that prior, as nanoseconds of stream.
    pub prior_ns: u64,
    /// How far before an anomaly's anchor a context may reach, nanoseconds.
    pub context_lookback_ns: u64,
    /// Most references in a context built by the rung. Past this the rung keeps the first quarter
    /// (the burst) and the most recent rest.
    pub context_max_refs: u32,
    /// How the context of an escalation is built (work item R6). The default is the rung's own,
    /// which is not written to a manifest; an arm may name another in the manifest
    /// ([`crate::stream::manifest::StreamArmSpec::context`]), which takes the place of this one
    /// for that arm.
    #[serde(default, skip_serializing_if = "ContextBuilder::is_rung")]
    pub context: ContextBuilder,
}

impl Default for RungConfig {
    /// Placeholders. See the type's documentation.
    fn default() -> Self {
        Self {
            patience_ns: 3_000_000_000,
            window: 256,
            review_ns: 500_000_000,
            notice_z: 3.0,
            score_window_ns: 8_000_000_000,
            burst_ns: 400_000_000,
            burst_gap_ns: 2_000_000_000,
            quiet_ns: 6_000_000_000,
            retain_ns: 120_000_000_000,
            prior_mhz: 100,
            prior_ns: 30_000_000_000,
            context_lookback_ns: 2_000_000_000,
            context_max_refs: 128,
            context: ContextBuilder::Rung,
        }
    }
}

impl RungConfig {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.window == 0 {
            return Err("rung.window must be at least 1".to_owned());
        }
        if self.review_ns == 0 || self.score_window_ns == 0 || self.quiet_ns == 0 {
            return Err("rung.review_ns, score_window_ns and quiet_ns must be positive".to_owned());
        }
        if !self.notice_z.is_finite() {
            return Err("rung.notice_z must be finite".to_owned());
        }
        if self.context_max_refs == 0 {
            return Err("rung.context_max_refs must be at least 1".to_owned());
        }
        if self.prior_ns == 0 {
            return Err("rung.prior_ns must be positive".to_owned());
        }
        Ok(())
    }
}

/// One passive observation the rung holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Its id, the position in the stream.
    pub id: ObsId,
    /// When it was emitted.
    pub at: Instant,
    /// What it is.
    pub obs: Observation,
    /// Whether the first world's public rules call it abnormal.
    pub abnormal: bool,
}

/// The observations the rung holds: everything delivered in the last `retain_ns`, in id order.
#[derive(Debug, Clone, Default)]
pub struct Store {
    items: VecDeque<Held>,
}

impl Store {
    /// A store holding `items`, oldest first. The rung fills its own as the stream delivers; this
    /// is for tests and tools that need a store of known content.
    pub fn with(items: impl IntoIterator<Item = Held>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// The held observations, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Held> {
        self.items.iter()
    }

    /// How many are held.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether none is held.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn push(&mut self, held: Held) {
        self.items.push_back(held);
    }

    fn prune(&mut self, now: Instant, retain_ns: u64) {
        while self
            .items
            .front()
            .is_some_and(|h| h.at.0.saturating_add(retain_ns) < now.0)
        {
            self.items.pop_front();
        }
    }
}

/// The service a passive observation is about, if it is about one.
pub fn service_of(obs: &Observation) -> Option<ServiceId> {
    match obs {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => Some(*service),
        Observation::Probed { .. } | Observation::Correction { .. } => None,
    }
}

/// Whether the first world's public rules call `obs` abnormal: a counter at or above `HIGH`, a
/// catalogue message other than `CheckHealth`, or a snapshot whose hash differs from the public
/// graph's. Everything else is permitted under every hypothesis and carries no information.
pub fn is_abnormal(obs: &Observation, services: &[gordian_world::Service]) -> bool {
    match obs {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => {
            matches!(SignalText::from_text_id(*text_id), Some(t) if t != SignalText::CheckHealth)
        }
        Observation::Snapshot {
            service,
            config_hash,
        } => services
            .get(service.index())
            .is_some_and(|s| s.config_hash != *config_hash),
        Observation::Probed { .. } | Observation::Correction { .. } => false,
    }
}

/// What an escalation rule is shown of one noticed anomaly. Public statistics and the rung's own
/// bookkeeping; nothing hidden.
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyView {
    /// Dense from zero in order of creation (candidates that never crossed the threshold take a
    /// number too).
    pub id: u32,
    /// The service the anomaly is anchored at.
    pub site: ServiceId,
    /// The first abnormal observation attached to it.
    pub anchor: ObsId,
    /// When the anchor was emitted.
    pub anchor_at: Instant,
    /// When its score first crossed the notice threshold.
    pub noticed_at: Instant,
    /// Its score now: the z-score of its abnormal count over the last score window.
    pub score: f64,
    /// The highest score it has had since it was noticed.
    pub peak_score: f64,
    /// A digest of the evidence about it: the symptom tags of its abnormal observations and the
    /// number of services they are about. Changes only when a new kind of symptom, or a new
    /// service, appears; a repeated heartbeat does not change it.
    pub digest: u64,
    /// Escalations proposed for it so far (accepted or not).
    pub attempts: u32,
    /// Proposed escalations not yet answered or refused.
    pub pending: u32,
    /// Answers received for it.
    pub answered: u32,
    /// When the last escalation for it was proposed.
    pub last_attempt_at: Option<Instant>,
    /// Its digest when the last escalation for it was proposed.
    pub last_attempt_digest: Option<u64>,
    /// Whether the cheap rung has declared for it.
    pub cheap_declared: bool,
    /// Observations delivered so far in the whole stream.
    pub delivered: u32,
    /// Since when every check of the public consistency checker on the evidence attached to this
    /// anomaly has found no hypothesis consistent with it (an empty set: the first world's own
    /// word for "contradictory"): the instant of the first check of the current run of empty
    /// results. `None` when the latest check found a consistent hypothesis, or none has been made.
    /// The rung makes these checks only for an arm whose rule asks ([`Rung::set_monitor`]), so for
    /// every other arm it is always `None`. Public information only: the checker reads the
    /// attached observations and the probes bought, under the first world's public rules.
    pub contradicted_since: Option<Instant>,
}

/// What one review of an anomaly concluded.
#[derive(Debug, Clone, PartialEq)]
pub enum Conclusion {
    /// The rule declares this (`None`: the anchor is not an incident).
    Declare(Diagnosis),
    /// The rule has nothing to declare and gives up.
    Abstain,
    /// The rule wants this probe bought first.
    Probe(Probe),
}

struct Cheap {
    state: WorkingState,
    decider: Decider,
    components: Vec<Box<dyn Component>>,
}

/// One anomaly: a candidate until it is noticed.
struct Anomaly {
    id: u32,
    site: ServiceId,
    region: Vec<bool>,
    anchor: ObsId,
    anchor_at: Instant,
    /// Abnormal observations attached to it: instant, id, service.
    attached: Vec<(Instant, ObsId, ServiceId)>,
    /// For each attached observation, its symptom tags and whether it is a changed snapshot,
    /// so that the evidence can be rebuilt when the anchor moves.
    sigs: Vec<(Vec<SymptomTag>, bool)>,
    /// The symptom tags (counter names and catalogue messages, service forgotten) of what is
    /// attached, whether a changed snapshot is, and the services it is about: the evidence
    /// digest's content.
    tags: BTreeSet<SymptomTag>,
    snapshot_changed: bool,
    services: BTreeSet<ServiceId>,
    last_abnormal_at: Instant,
    /// When the last abnormal observation at the anomaly's own site was attached.
    last_site_at: Instant,
    /// When the current burst began at the site: its first abnormal observation after a silence
    /// of `burst_gap_ns`. Propagation to dependents is measured from here.
    burst_open_at: Instant,
    noticed_at: Option<Instant>,
    peak_score: f64,
    cheap: Option<Cheap>,
    dirty: bool,
    patience_reviewed: bool,
    /// The rule's patience in the anomaly's own time (from its anchor): the time between anchor
    /// and notice plus the configured patience.
    patience_rel: u64,
    last_review: Option<Instant>,
    not_before: Instant,
    awaiting: BTreeSet<Probe>,
    attempts: u32,
    pending: u32,
    answered: u32,
    last_attempt_at: Option<Instant>,
    last_attempt_digest: Option<u64>,
    /// The cheap rung's conclusion, kept while a hold stops it being declared.
    deferred: Option<Diagnosis>,
    cheap_done: bool,
    cheap_declared: bool,
    reasoner_declared: bool,
    /// Diagnoses a recognizer has declared (an arm that revises its conclusion).
    recognized: Vec<Diagnosis>,
    /// Evidence arrived since the last consistency check (only read when the rung monitors).
    monitor_dirty: bool,
    /// When the consistency checker last ran on this anomaly (only set when the rung monitors).
    last_check: Option<Instant>,
    /// See [`AnomalyView::contradicted_since`].
    contradicted_since: Option<Instant>,
}

impl Anomaly {
    /// Whether a (benign or abnormal) observation about `service` at `at` belongs in a context
    /// built for this anomaly: anything at its site, and at a dependent of the site only within
    /// the burst window after the anchor. Propagation is immediate (public rule 1: a dependent's
    /// symptom is permitted only after an `ErrorRate` at the site), so a dependent's observation
    /// long after the burst is more plausibly another incident's or background than this one's.
    fn admits(&self, service: ServiceId, at: Instant, burst_ns: u64) -> bool {
        service == self.site
            || (self.region.get(service.index()).copied().unwrap_or(false)
                && at.0 <= self.anchor_at.0.saturating_add(burst_ns))
    }

    fn rel(&self, at: Instant) -> Instant {
        Instant(at.0.saturating_sub(self.anchor_at.0))
    }

    fn owns(&self, id: ObsId) -> bool {
        id == self.anchor || self.attached.iter().any(|(_, o, _)| *o == id)
    }

    fn note_attached(&mut self, held: &Held, service: ServiceId, burst_gap_ns: u64) {
        let snapshot = matches!(held.obs, Observation::Snapshot { .. });
        let tags = if snapshot {
            Vec::new()
        } else {
            signature(&[(held.at, held.obs.clone())])
        };
        self.attached.push((held.at, held.id, service));
        self.last_abnormal_at = held.at;
        if service == self.site {
            if held.at.0 >= self.last_site_at.0.saturating_add(burst_gap_ns)
                || self.attached.len() == 1
            {
                self.burst_open_at = held.at;
            }
            self.last_site_at = held.at;
        }
        self.services.insert(service);
        self.snapshot_changed |= snapshot;
        self.tags.extend(tags.iter().copied());
        self.sigs.push((tags, snapshot));
    }

    /// Move the anchor to the start of the densest burst among the attached observations: the
    /// attached observation with the most attached observations in the `cluster_ns` from it (the
    /// earliest on a tie), and make its service the anomaly's site.
    ///
    /// An anomaly's first attached observation may be background (an isolated blip or stray at
    /// the same service, or at a service the site depends on, a moment before the incident
    /// began). The anchor is what a declaration and a question are *about*, and the site is what
    /// the region is built from, so both must belong to the thing noticed and not to whatever
    /// happened to come first. A real incident begins with a burst (several abnormal observations
    /// within a few hundred milliseconds); a stray is alone. Observations before the new anchor
    /// are dropped from the anomaly's evidence. Public statistics only.
    fn reanchor(&mut self, cluster_ns: u64, services: &[gordian_world::Service]) {
        let density = |k: usize| {
            let from = self.attached[k].0.0;
            self.attached[k..]
                .iter()
                .take_while(|(t, _, _)| t.0 <= from.saturating_add(cluster_ns))
                .count()
        };
        let mut chosen = 0;
        let mut best = 0;
        for k in 0..self.attached.len() {
            let d = density(k);
            if d > best {
                best = d;
                chosen = k;
            }
        }
        if chosen == 0 {
            return;
        }
        self.attached.drain(..chosen);
        self.sigs.drain(..chosen);
        self.tags = self
            .sigs
            .iter()
            .flat_map(|(t, _)| t.iter().copied())
            .collect();
        self.snapshot_changed = self.sigs.iter().any(|(_, snap)| *snap);
        self.services = self.attached.iter().map(|(_, _, s)| *s).collect();
        self.anchor = self.attached[0].1;
        self.anchor_at = self.attached[0].0;
        self.site = self.attached[0].2;
        self.region = dependents_mask(services, self.site);
        self.last_site_at = self
            .attached
            .iter()
            .rev()
            .find(|(_, _, s)| *s == self.site)
            .map_or(self.anchor_at, |(t, _, _)| *t);
        self.burst_open_at = self.anchor_at;
    }

    /// The observation a declaration of `diagnosis` is anchored on: the first attached abnormal
    /// observation at the diagnosed site if there is one, else the anomaly's anchor. The rule
    /// names the site from the evidence, and the evidence at that site is what the declaration
    /// is about.
    fn anchor_for(&self, diagnosis: &Diagnosis) -> ObsId {
        diagnosis
            .and_then(|h| {
                self.attached
                    .iter()
                    .find(|(_, _, s)| *s == h.site)
                    .map(|(_, o, _)| *o)
            })
            .unwrap_or(self.anchor)
    }

    /// A digest of the evidence about the anomaly: its symptom tags and the number of services
    /// they are about. FNV-1a over their text, so it is stable. A repeated heartbeat does not
    /// change it; a new kind of symptom, or a new service, does.
    fn digest(&self) -> u64 {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3);
            }
        };
        for t in &self.tags {
            feed(format!("{t:?};").as_bytes());
        }
        feed(
            format!(
                "snapshot={};services={}",
                self.snapshot_changed,
                self.services.len()
            )
            .as_bytes(),
        );
        h
    }
}

/// The shared cheap rung. See the module documentation.
pub struct Rung {
    cfg: RungConfig,
    public: StreamPublic,
    world: PublicInfo,
    store: Store,
    delivered: u32,
    /// The instant of the latest step taken in: the instant of a call, for a context builder.
    now: Instant,
    abnormal_seen: u64,
    anomalies: Vec<Anomaly>,
    next_id: u32,
    noticed_total: u32,
    /// The last diagnosis declared for each anchor, so an identical answer is not declared twice.
    declared: BTreeMap<ObsId, Diagnosis>,
    /// Whether the rung keeps the consistency checker's verdict on each anomaly up to date, for a
    /// rule that escalates on it. False for every arm but `contradiction_escalation`, for which
    /// the rung then does exactly what it did before the field existed.
    monitor: bool,
}

fn map_hypothesis(h: gordian_world::Hypothesis) -> Diagnosis {
    h.map(|(kind, site)| StreamHypothesis {
        kind: StreamKind::Known(kind),
        site,
    })
}

/// Whether the verifier's output says no hypothesis is consistent with the evidence: an
/// `EvidenceDamaged` entry, or a candidate list that is empty. An output with no entry, or one the
/// rung cannot read, says nothing and counts as a consistent set.
fn verdict_is_empty(output: &ComponentOutput) -> bool {
    !output.entries.is_empty()
        && output.entries.iter().all(|(_, bytes)| match decode(bytes) {
            Ok(HypothesisEntry::EvidenceDamaged { .. }) => true,
            Ok(HypothesisEntry::Candidates { ranked, .. }) => ranked.is_empty(),
            Err(_) => false,
        })
}

/// Record a verdict: the start of a run of empty verdicts is remembered, and a verdict with a
/// hypothesis ends the run.
fn record_verdict(
    empty: bool,
    now: Instant,
    dirty: &mut bool,
    last_check: &mut Option<Instant>,
    since: &mut Option<Instant>,
) {
    *dirty = false;
    *last_check = Some(now);
    if empty {
        since.get_or_insert(now);
    } else {
        *since = None;
    }
}

/// The ids of the components the cheap rung runs, in the order it runs them: the heuristic, the
/// estimator and the verifier. The memory lookup is not run: the shared rule never reads it.
pub fn cheap_component_ids() -> Vec<ComponentId> {
    vec![
        gordian_components::HEURISTIC_ID,
        gordian_components::ESTIMATOR_ID,
        gordian_components::VERIFIER_ID,
    ]
}

impl Rung {
    /// A rung over the stream's public information.
    pub fn new(public: &StreamPublic, cfg: RungConfig) -> Self {
        Self {
            world: public.world_public_info(),
            public: public.clone(),
            cfg,
            store: Store::default(),
            delivered: 0,
            now: Instant(0),
            abnormal_seen: 0,
            anomalies: Vec::new(),
            next_id: 0,
            noticed_total: 0,
            declared: BTreeMap::new(),
            monitor: false,
        }
    }

    /// Whether to keep each noticed anomaly's consistency verdict ([`AnomalyView::contradicted_since`])
    /// up to date. Off by default; set once, before the first step, by an arm whose rule reads it.
    /// When on, the rung runs the consistency verifier (through the meter, so charged, counted and
    /// recorded like every component run) on a noticed anomaly that received evidence since its
    /// last check and that no escalation has been proposed for, at most once per `review_ns`,
    /// including after the shared rule has concluded and stopped reviewing it.
    pub fn set_monitor(&mut self, on: bool) {
        self.monitor = on;
    }

    /// The configuration.
    pub fn config(&self) -> &RungConfig {
        &self.cfg
    }

    /// The observations held.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Observations delivered so far.
    pub fn delivered(&self) -> u32 {
        self.delivered
    }

    /// Anomalies that crossed the notice threshold so far.
    pub fn noticed_total(&self) -> u32 {
        self.noticed_total
    }

    /// Anomalies currently noticed and not retired.
    pub fn live(&self) -> usize {
        self.anomalies
            .iter()
            .filter(|a| a.noticed_at.is_some())
            .count()
    }

    /// The expected abnormal observations per service per second, learned from the stream so
    /// far with the configured prior. Basic IEEE arithmetic only, so it replays bit for bit.
    fn baseline_hz(&self, now: Instant) -> f64 {
        let services = self.public.services.len().max(1) as f64;
        let prior_s = self.cfg.prior_ns as f64 / 1e9;
        let prior_hz = self.cfg.prior_mhz as f64 / 1000.0;
        let t_s = now.0 as f64 / 1e9;
        (self.abnormal_seen as f64 + prior_hz * services * prior_s) / (services * (t_s + prior_s))
    }

    fn score_of(&self, a: &Anomaly, now: Instant) -> f64 {
        let window = self.cfg.score_window_ns;
        let from = now.0.saturating_sub(window);
        let n = a.attached.iter().filter(|(at, _, _)| at.0 > from).count() as f64;
        let mu = self.baseline_hz(now) * (window as f64 / 1e9);
        (n - mu) / (mu + 1.0).sqrt()
    }

    /// Take in what the step delivered: observations (stored, attached to anomalies, admitted to
    /// the working states of noticed ones), reasoner answers (returned), probe results (admitted
    /// to the anomaly that bought them).
    pub fn absorb(
        &mut self,
        events: &[StreamEvent],
        probe_results: &[(u32, Instant, Observation)],
        now: Instant,
    ) -> Vec<(ObsId, Diagnosis)> {
        self.now = now;
        let mut answers = Vec::new();
        for event in events {
            match event {
                StreamEvent::Observed { id, at, obs } => {
                    let abnormal = is_abnormal(obs, &self.public.services);
                    let held = Held {
                        id: *id,
                        at: *at,
                        obs: obs.clone(),
                        abnormal,
                    };
                    self.delivered = self.delivered.max(id.0.saturating_add(1));
                    if abnormal {
                        self.abnormal_seen += 1;
                        // The evidence about an anomaly is what is attached to it: every
                        // abnormal observation is assigned to one anomaly, so that another
                        // incident's symptoms are not handed to this one's rule as evidence (the
                        // first world's checker reads them as a contradiction). Benign
                        // observations carry no information under the public rules.
                        if let Some(i) = self.attach(&held) {
                            let a = &mut self.anomalies[i];
                            let rel = Instant(at.0.saturating_sub(a.anchor_at.0));
                            if let Some(cheap) = a.cheap.as_mut() {
                                cheap.state.admit(rel, obs.clone());
                                a.dirty = true;
                                a.monitor_dirty = true;
                            }
                        }
                    }
                    self.store.push(held);
                }
                StreamEvent::Answered { answer, .. } => {
                    answers.push((answer.focus, answer.diagnosis));
                }
            }
        }
        for (_, ready, obs) in probe_results {
            if let Observation::Probed { probe, .. } = obs {
                for a in &mut self.anomalies {
                    if a.awaiting.remove(probe) {
                        if let Some(cheap) = a.cheap.as_mut() {
                            let rel = Instant(ready.0.saturating_sub(a.anchor_at.0));
                            cheap.state.admit(rel, obs.clone());
                        }
                        a.dirty = true;
                        a.monitor_dirty = true;
                        break;
                    }
                }
            }
        }
        self.store.prune(now, self.cfg.retain_ns);
        answers
    }

    fn attach(&mut self, held: &Held) -> Option<usize> {
        let service = service_of(&held.obs)?;
        let burst = self.cfg.burst_ns;
        let gap = self.cfg.burst_gap_ns;
        // An observation about the site of an anomaly that is still speaking (its last
        // observation at its site within two burst gaps) is that anomaly's, even if it falls in
        // another site's burst window: a site's heartbeat is not propagation.
        // Otherwise, propagation: an observation about a dependent of an anomaly's site, within
        // the burst window of the burst that began at that site, belongs to that burst; when
        // several anomalies qualify, the one whose burst began most recently. Last, the same
        // site's stale anomaly. The orders matter: letting a stale isolated observation at the
        // dependent swallow the dependent's share of someone else's burst, measuring the window
        // from an anomaly's first observation, or letting another incident's burst window take a
        // site's own heartbeat, each fragments incidents and lands declarations on background.
        let speaking = gap.saturating_mul(2);
        let found = self
            .anomalies
            .iter()
            .rposition(|a| {
                a.site == service && held.at.0 <= a.last_site_at.0.saturating_add(speaking)
            })
            .or_else(|| {
                self.anomalies
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| {
                        a.site != service
                            && a.region.get(service.index()).copied().unwrap_or(false)
                            && held.at.0 >= a.burst_open_at.0
                            && held.at.0 <= a.burst_open_at.0.saturating_add(burst)
                    })
                    .max_by_key(|(i, a)| (a.burst_open_at, std::cmp::Reverse(*i)))
                    .map(|(i, _)| i)
            })
            .or_else(|| self.anomalies.iter().rposition(|a| a.site == service));
        match found {
            Some(i) => {
                self.anomalies[i].note_attached(held, service, gap);
                Some(i)
            }
            None => {
                let id = self.next_id;
                self.next_id += 1;
                let region = dependents_mask(&self.public.services, service);
                let mut anomaly = Anomaly {
                    id,
                    site: service,
                    region,
                    anchor: held.id,
                    anchor_at: held.at,
                    attached: Vec::new(),
                    sigs: Vec::new(),
                    tags: BTreeSet::new(),
                    snapshot_changed: false,
                    services: BTreeSet::new(),
                    last_abnormal_at: held.at,
                    last_site_at: held.at,
                    burst_open_at: held.at,
                    noticed_at: None,
                    peak_score: f64::NEG_INFINITY,
                    cheap: None,
                    dirty: false,
                    patience_reviewed: false,
                    patience_rel: 0,
                    last_review: None,
                    not_before: Instant::ZERO,
                    awaiting: BTreeSet::new(),
                    attempts: 0,
                    pending: 0,
                    answered: 0,
                    last_attempt_at: None,
                    last_attempt_digest: None,
                    deferred: None,
                    cheap_done: false,
                    cheap_declared: false,
                    reasoner_declared: false,
                    recognized: Vec::new(),
                    monitor_dirty: false,
                    last_check: None,
                    contradicted_since: None,
                };
                anomaly.note_attached(held, service, gap);
                self.anomalies.push(anomaly);
                Some(self.anomalies.len() - 1)
            }
        }
    }

    /// Notice the candidates whose score has crossed the threshold, and forget the ones that
    /// never will.
    pub fn notice(&mut self, now: Instant) {
        let mut crossed = Vec::new();
        for (i, a) in self.anomalies.iter().enumerate() {
            if a.noticed_at.is_none() && self.score_of(a, now) >= self.cfg.notice_z {
                crossed.push(i);
            }
        }
        for i in crossed {
            self.anomalies[i].reanchor(self.cfg.burst_ns, &self.public.services);
            let mut state = WorkingState::new(self.world.clone(), self.cfg.window);
            let anchor_at = self.anomalies[i].anchor_at;
            let ids: BTreeSet<ObsId> = self.anomalies[i]
                .attached
                .iter()
                .map(|(_, o, _)| *o)
                .collect();
            for h in self.store.iter().filter(|h| ids.contains(&h.id)) {
                state.admit(Instant(h.at.0.saturating_sub(anchor_at.0)), h.obs.clone());
            }
            let a = &mut self.anomalies[i];
            let patience_rel = now.0.saturating_sub(a.anchor_at.0) + self.cfg.patience_ns;
            a.patience_rel = patience_rel;
            a.cheap = Some(Cheap {
                state,
                decider: Decider::new(DecideConfig {
                    patience_ns: patience_rel,
                }),
                components: vec![
                    Box::new(RuleHeuristic::new()),
                    Box::new(CountEstimator::new()),
                    Box::new(ConsistencyVerifier::new()),
                ],
            });
            a.noticed_at = Some(now);
            a.dirty = true;
            a.monitor_dirty = true;
            self.noticed_total += 1;
        }
        let ttl = self.cfg.score_window_ns;
        self.anomalies.retain(|a| {
            a.noticed_at.is_some() || now.0 <= a.last_abnormal_at.0.saturating_add(ttl)
        });
    }

    /// The noticed anomalies, in the order they were noticed (by id), as an escalation rule sees
    /// them. Updates each one's peak score.
    pub fn views(&mut self, now: Instant) -> Vec<AnomalyView> {
        let scores: Vec<f64> = self
            .anomalies
            .iter()
            .map(|a| self.score_of(a, now))
            .collect();
        let mut out = Vec::new();
        for (i, a) in self.anomalies.iter_mut().enumerate() {
            if a.noticed_at.is_some() {
                a.peak_score = a.peak_score.max(scores[i]);
            }
        }
        for (i, a) in self.anomalies.iter().enumerate() {
            let Some(noticed_at) = a.noticed_at else {
                continue;
            };
            out.push(AnomalyView {
                id: a.id,
                site: a.site,
                anchor: a.anchor,
                anchor_at: a.anchor_at,
                noticed_at,
                score: scores[i],
                peak_score: a.peak_score,
                digest: a.digest(),
                attempts: a.attempts,
                pending: a.pending,
                answered: a.answered,
                last_attempt_at: a.last_attempt_at,
                last_attempt_digest: a.last_attempt_digest,
                cheap_declared: a.cheap_declared,
                delivered: self.delivered,
                contradicted_since: a.contradicted_since,
            });
        }
        out
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.anomalies.iter().position(|a| a.id == id)
    }

    /// The context for a question about anomaly `id`: references to the held observations that
    /// are evidence about it (at its site, and at its dependents within the burst) from
    /// `context_lookback_ns` before its anchor to now, benign and free-form ones included (the rung has no cheap way to tell which matter), and to the
    /// probes bought for it. Capped as [`RungConfig::context_max_refs`] says.
    ///
    /// That is the `rung` builder, the default. With another builder in [`RungConfig::context`]
    /// the references are that builder's ([`super::context`]), a function of public information
    /// only: the observations held, the public graph, the instant of the latest step and the
    /// anomaly's site and anchor.
    pub fn context(&self, id: u32) -> Vec<ObsRef> {
        let Some(i) = self.index_of(id) else {
            return Vec::new();
        };
        let a = &self.anomalies[i];
        if !self.cfg.context.is_rung() {
            let view = PublicView {
                store: &self.store,
                services: &self.public.services,
                now: self.now,
                anchor_at: a.anchor_at,
                site: a.site,
                burst_gap_ns: self.cfg.burst_gap_ns,
                lookback_ns: self.cfg.context_lookback_ns,
            };
            if let Some(refs) = context::build(&self.cfg.context, &view) {
                return refs;
            }
        }
        let from = a.anchor_at.0.saturating_sub(self.cfg.context_lookback_ns);
        let mut refs: Vec<ObsRef> = self
            .store
            .iter()
            .filter(|h| {
                h.at.0 >= from
                    && service_of(&h.obs).is_some_and(|s| a.admits(s, h.at, self.cfg.burst_ns))
            })
            .map(|h| ObsRef::Passive(h.id))
            .collect();
        let cap = self.cfg.context_max_refs as usize;
        if refs.len() > cap {
            let head = cap / 4;
            let tail = cap - head;
            let mut kept: Vec<ObsRef> = refs[..head].to_vec();
            kept.extend_from_slice(&refs[refs.len() - tail..]);
            refs = kept;
        }
        refs
    }

    /// Record that an escalation was proposed for anomaly `id` at `now`.
    pub fn note_attempt(&mut self, id: u32, now: Instant, digest: u64) {
        if let Some(i) = self.index_of(id) {
            let a = &mut self.anomalies[i];
            a.attempts += 1;
            a.pending += 1;
            a.last_attempt_at = Some(now);
            a.last_attempt_digest = Some(digest);
        }
    }

    /// An escalation of anomaly `id` was refused: it is no longer pending.
    pub fn note_refused(&mut self, id: u32) {
        if let Some(i) = self.index_of(id) {
            let a = &mut self.anomalies[i];
            a.pending = a.pending.saturating_sub(1);
        }
    }

    /// A probe proposed for anomaly `id` was refused.
    pub fn note_probe_refused(&mut self, id: u32, probe: Probe, now: Instant) {
        if let Some(i) = self.index_of(id) {
            let review = self.cfg.review_ns;
            let a = &mut self.anomalies[i];
            a.awaiting.remove(&probe);
            a.dirty = true;
            a.not_before = Instant(now.0.saturating_add(review));
        }
    }

    /// Handle an answer about `focus`: the anomaly that owns it has the answer. Returns whether
    /// `diagnosis` should be declared (it differs from the last declaration anchored there), and
    /// marks the anomaly as answered.
    pub fn take_answer(&mut self, focus: ObsId, diagnosis: Diagnosis) -> bool {
        if let Some(a) = self.anomalies.iter_mut().find(|a| a.owns(focus)) {
            a.pending = a.pending.saturating_sub(1);
            a.answered += 1;
            a.reasoner_declared = true;
            a.deferred = None;
        }
        let fresh = self.declared.get(&focus) != Some(&diagnosis);
        if fresh {
            self.declared.insert(focus, diagnosis);
        }
        fresh
    }

    /// Whether the anomaly `id` is held back from declaring (see [`Rung::review`]) is decided by
    /// the caller; this reports its cheap conclusion if one is waiting and the reasoner has not
    /// declared for it.
    pub fn deferred(&self, id: u32) -> Option<Diagnosis> {
        let a = &self.anomalies[self.index_of(id)?];
        if a.reasoner_declared {
            None
        } else {
            a.deferred
        }
    }

    /// The deferred conclusion of anomaly `id` is declared now.
    pub fn declare_deferred(&mut self, id: u32) -> Option<Proposed> {
        let i = self.index_of(id)?;
        let diagnosis = self.anomalies[i].deferred.take()?;
        self.declare_cheap(i, diagnosis, 0)
    }

    fn declare_cheap(&mut self, i: usize, diagnosis: Diagnosis, tag: u64) -> Option<Proposed> {
        let a = &mut self.anomalies[i];
        a.cheap_done = true;
        // A reasoner's answer outranks the shared rule, and so does a recogniser's conclusion
        // (an arm that revises): once either has declared for this anomaly the rule does not.
        if a.reasoner_declared || !a.recognized.is_empty() {
            return None;
        }
        a.cheap_declared = true;
        let anchor = a.anchor_for(&diagnosis);
        if self.declared.get(&anchor) == Some(&diagnosis) {
            return None;
        }
        self.declared.insert(anchor, diagnosis);
        Some(Proposed {
            tag,
            action: StreamAction::Declare { anchor, diagnosis },
            source: Source::CheapRung,
        })
    }

    /// Defer a conclusion: the cheap rung has one, an escalation rule holds it back.
    fn defer(&mut self, i: usize, diagnosis: Diagnosis) {
        let a = &mut self.anomalies[i];
        a.cheap_done = true;
        a.deferred = Some(diagnosis);
    }

    /// A conclusion a recognizer reached, declared at once and possibly after the rule's own: at
    /// most once per distinct diagnosis. For an arm that revises (`ablation_hidden_rules`).
    pub fn declare_recognized(&mut self, id: u32, diagnosis: Diagnosis) -> Option<Proposed> {
        let i = self.index_of(id)?;
        let a = &mut self.anomalies[i];
        if a.reasoner_declared || a.recognized.contains(&diagnosis) {
            return None;
        }
        a.recognized.push(diagnosis);
        a.cheap_done = true;
        a.cheap_declared = true;
        let anchor = a.anchor_for(&diagnosis);
        if self.declared.get(&anchor) == Some(&diagnosis) {
            return None;
        }
        self.declared.insert(anchor, diagnosis);
        Some(Proposed {
            tag: 0,
            action: StreamAction::Declare { anchor, diagnosis },
            source: Source::CheapRung,
        })
    }

    /// Dismiss anomaly `id`: declare "not an incident" at its anchor, now, and never let the cheap
    /// rung declare for it. The declaration is the cheap rung's by source (it is not a reasoner
    /// answer), and the anomaly is no longer reviewed. For a rule that dismisses
    /// ([`super::EscalationRule::dismissals`]); only the privileged decoy arm does.
    pub fn dismiss(&mut self, id: u32) -> Option<Proposed> {
        self.declare_recognized(id, None)
    }

    /// The anomalies due a review, by id: noticed, not finished with the rule, not waiting for a
    /// probe, not in cool-down, and with something new to look at (or the
    /// patience newly passed).
    pub fn due(&self, now: Instant) -> Vec<u32> {
        let mut out = Vec::new();
        for a in &self.anomalies {
            if a.cheap.is_none() {
                continue;
            }
            if a.cheap_done || !a.awaiting.is_empty() || now < a.not_before {
                continue;
            }
            if a.last_review
                .is_some_and(|t| now.0 < t.0.saturating_add(self.cfg.review_ns))
            {
                continue;
            }
            let rel_now = a.rel(now).0;
            let patience_due = rel_now >= a.patience_rel && !a.patience_reviewed;
            if a.dirty || patience_due {
                out.push(a.id);
            }
        }
        out
    }

    /// Review anomaly `id`: run the cheap components on its working state and let the shared rule
    /// decide, both through the meter. Returns the rule's conclusion, if it reached one.
    pub fn review(&mut self, id: u32, now: Instant, meter: &mut Meter<'_>) -> Option<Conclusion> {
        let i = self.index_of(id)?;
        let monitor = self.monitor;
        let a = &mut self.anomalies[i];
        let patience = a.patience_rel;
        let rel_now = Instant(now.0.saturating_sub(a.anchor_at.0));
        let cheap = a.cheap.as_mut()?;
        cheap.state.now = cheap.state.now.max(rel_now);
        let mut outputs: Vec<(ComponentId, ComponentOutput)> = Vec::new();
        for component in &mut cheap.components {
            if let Some(output) = meter.run_component(component.as_mut(), &cheap.state) {
                outputs.push((component.id(), output));
            }
        }
        if monitor && let Some((_, output)) = outputs.iter().find(|(c, _)| *c == VERIFIER_ID) {
            record_verdict(
                verdict_is_empty(output),
                now,
                &mut a.monitor_dirty,
                &mut a.last_check,
                &mut a.contradicted_since,
            );
        }
        let call = meter.rule_call(&mut cheap.decider, &cheap.state, &outputs, false);
        let RuleCall::Ran(action) = call else {
            return None;
        };
        a.dirty = false;
        a.last_review = Some(now);
        if rel_now.0 >= patience {
            a.patience_reviewed = true;
        }
        match action {
            Some(Action::Declare { fault }) => Some(Conclusion::Declare(map_hypothesis(fault))),
            Some(Action::Abstain) => Some(Conclusion::Abstain),
            Some(Action::Probe { kind, target }) => {
                let probe = Probe { kind, target };
                a.awaiting.insert(probe);
                Some(Conclusion::Probe(probe))
            }
            Some(Action::Correct { .. }) | None => None,
        }
    }

    /// The noticed anomalies whose consistency check is due: the rung monitors, evidence arrived
    /// since the last check, no escalation was proposed for it, and at least `review_ns` has
    /// passed since the last check. By id.
    pub fn due_checks(&self, now: Instant) -> Vec<u32> {
        if !self.monitor {
            return Vec::new();
        }
        self.anomalies
            .iter()
            .filter(|a| {
                a.cheap.is_some()
                    && a.attempts == 0
                    && a.monitor_dirty
                    && a.last_check
                        .is_none_or(|t| now.0 >= t.0.saturating_add(self.cfg.review_ns))
            })
            .map(|a| a.id)
            .collect()
    }

    /// Check anomaly `id` against the public rules: run the consistency verifier on its working
    /// state through the meter and keep whether it left a hypothesis
    /// ([`AnomalyView::contradicted_since`]). Only the verifier runs, not the other components
    /// and not the shared rule, and nothing is declared: this is how a rule that escalates on
    /// contradiction sees evidence that arrived after the shared rule concluded. A refusal by the
    /// bill leaves the previous verdict in place.
    pub fn check(&mut self, id: u32, now: Instant, meter: &mut Meter<'_>) {
        let Some(i) = self.index_of(id) else {
            return;
        };
        let a = &mut self.anomalies[i];
        let rel_now = Instant(now.0.saturating_sub(a.anchor_at.0));
        let Some(cheap) = a.cheap.as_mut() else {
            return;
        };
        cheap.state.now = cheap.state.now.max(rel_now);
        let mut verifier = ConsistencyVerifier::new();
        if let Some(output) = meter.run_component(&mut verifier, &cheap.state) {
            record_verdict(
                verdict_is_empty(&output),
                now,
                &mut a.monitor_dirty,
                &mut a.last_check,
                &mut a.contradicted_since,
            );
        }
    }

    /// Apply a review's conclusion for anomaly `id`: returns the proposals it leads to.
    ///
    /// A declaration is proposed unless `held`, in which case it is kept for later
    /// ([`Rung::deferred`]). A probe becomes a probe proposal tagged with the anomaly.
    pub fn conclude(
        &mut self,
        id: u32,
        conclusion: Conclusion,
        held: bool,
        tag: u64,
    ) -> Option<Proposed> {
        let i = self.index_of(id)?;
        // The rule concludes once per anomaly: a later conclusion is not acted on.
        if self.anomalies[i].cheap_done {
            return None;
        }
        match conclusion {
            Conclusion::Declare(diagnosis) => {
                if held {
                    self.defer(i, diagnosis);
                    None
                } else {
                    self.declare_cheap(i, diagnosis, tag)
                }
            }
            Conclusion::Abstain => {
                self.anomalies[i].cheap_done = true;
                None
            }
            Conclusion::Probe(probe) => Some(Proposed {
                tag,
                action: StreamAction::Probe {
                    kind: probe.kind,
                    target: probe.target,
                },
                source: Source::Probe,
            }),
        }
    }

    /// The final call for anomaly `id`: the rule at its deadline, whatever the clock says.
    /// Declares what it has unless an answer was declared or the rule already declared.
    pub fn finish_one(&mut self, id: u32, meter: &mut Meter<'_>, tag: u64) -> Option<Proposed> {
        let i = self.index_of(id)?;
        if let Some(diagnosis) = self.anomalies[i].deferred.take() {
            return self.declare_cheap(i, diagnosis, tag);
        }
        if self.anomalies[i].cheap_done {
            return None;
        }
        let a = &mut self.anomalies[i];
        let cheap = a.cheap.as_mut()?;
        let call = meter.rule_call(&mut cheap.decider, &cheap.state, &[], true);
        match call {
            RuleCall::Ran(Some(Action::Declare { fault })) => {
                self.declare_cheap(i, map_hypothesis(fault), tag)
            }
            _ => {
                self.anomalies[i].cheap_done = true;
                None
            }
        }
    }

    /// Ids of the noticed anomalies still undecided by the cheap rung.
    pub fn undecided(&self) -> Vec<u32> {
        self.anomalies
            .iter()
            .filter(|a| a.noticed_at.is_some() && (!a.cheap_done || a.deferred.is_some()))
            .map(|a| a.id)
            .collect()
    }

    /// The quiet anomalies that are ready to retire: noticed, no abnormal observation for
    /// `quiet_ns`, nothing pending.
    pub fn quiet(&self, now: Instant) -> Vec<u32> {
        self.anomalies
            .iter()
            .filter(|a| {
                a.noticed_at.is_some()
                    && a.pending == 0
                    && a.awaiting.is_empty()
                    && now.0 >= a.last_abnormal_at.0.saturating_add(self.cfg.quiet_ns)
            })
            .map(|a| a.id)
            .collect()
    }

    /// Retire anomaly `id`: it is forgotten.
    pub fn retire(&mut self, id: u32) {
        self.anomalies.retain(|a| a.id != id);
    }

    /// The attached abnormal observations of anomaly `id`, with their services.
    pub fn attached(&self, id: u32) -> Vec<(Instant, ObsId, ServiceId)> {
        self.index_of(id)
            .map(|i| self.anomalies[i].attached.clone())
            .unwrap_or_default()
    }

    /// Whether anomaly `id` owns observation `obs`.
    pub fn owns(&self, id: u32, obs: ObsId) -> bool {
        self.index_of(id)
            .is_some_and(|i| self.anomalies[i].owns(obs))
    }

    /// Feed back what happened to the proposals of the previous step: a refused probe or
    /// escalation frees the anomaly that was waiting on it. `outstanding` maps tags to what was
    /// proposed.
    pub fn applied(
        &mut self,
        applied: &[Applied],
        outstanding: &mut BTreeMap<u64, Outstanding>,
        now: Instant,
    ) {
        for result in applied {
            let Some(what) = outstanding.remove(&result.tag) else {
                continue;
            };
            if result.accepted {
                continue;
            }
            match what {
                Outstanding::Escalation { anomaly } => self.note_refused(anomaly),
                Outstanding::Probe { anomaly, probe } => {
                    self.note_probe_refused(anomaly, probe, now);
                }
            }
        }
    }
}

/// A proposal the rung is waiting to hear about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outstanding {
    /// An escalation for an anomaly.
    Escalation {
        /// The anomaly.
        anomaly: u32,
    },
    /// A probe bought for an anomaly.
    Probe {
        /// The anomaly.
        anomaly: u32,
        /// The probe.
        probe: Probe,
    },
}

/// Whether a probe result is positive, for recognizers that read bought probes.
pub fn is_positive(obs: &Observation) -> bool {
    matches!(
        obs,
        Observation::Probed {
            result: ProbeResult::Positive,
            ..
        }
    )
}

impl Rung {
    /// A one-line summary of every anomaly the rung holds, for diagnostics.
    pub fn debug_anomalies(&self) -> Vec<String> {
        self.anomalies
            .iter()
            .map(|a| {
                format!(
                    "anomaly {} site {} anchor {} at {} ms attached {} noticed {:?}",
                    a.id,
                    a.site.0,
                    a.anchor.0,
                    a.anchor_at.0 / 1_000_000,
                    a.attached.len(),
                    a.noticed_at.map(|t| t.0 / 1_000_000)
                )
            })
            .collect()
    }
}
