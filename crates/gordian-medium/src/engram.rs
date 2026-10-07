//! Engrams: bind on the plasticity port, recall by coincidence (work item A1a; the design is in
//! `DESIGN.md`, "The engram", written before this code).
//!
//! An **engram** is a small piece of ordinary medium that a bind builds: one `Coincidence` cell
//! per node its key's site ranges over (`key[e, n]`), fed by shared presence `Sense` cells, one
//! per (node, feature tag) (`feat[t, n]`); a `Latch` that any of them fires (`latch[e]`, the
//! engram's own cell); and an `Emit` (`emit[e]`) behind a plastic synapse whose weight is the
//! engram's **strength**. The engram table ([`Engrams`]) keeps, per engram, its key, its outcome,
//! its strength and its counts, and maps the emitter to the engram, so that a recall proposal
//! resolves to an outcome. The crate knows tags and nodes only; what a tag means (a kind of
//! observation, a diagnosis) belongs to the world adapter.
//!
//! The names label a mechanism; they claim nothing about what it achieves.
//!
//! # The cells of one engram with a key of `k` live features
//!
//! | Cell | Archetype | Parameters |
//! |---|---|---|
//! | `feat[t, n]` | `Sense`, presence | pattern `(domain, n, any channel, t)` |
//! | `key[e, n]` | `Coincidence`, sliding | `n` = `k`, window `w`, consumed, lookback `w` |
//! | `latch[e]` | `Latch` | threshold `k`, hold 0 |
//! | `emit[e]` | `Emit` | threshold `k * theta`, kind the recall kind, lookback `w`, refractory `r` |
//!
//! Synapses: `feat[t, n] -> key[e, n]` weight 1, plastic (a feature dropped by generalisation is
//! set to weight 0: its messages are no longer positive, so its slot never counts); `key[e, n] ->
//! latch[e]` weight 1, fixed; `latch[e] -> emit[e]` weight `s`, plastic. All without delay. The
//! emitter's input is `k * s` when a key cell fires, so a recall needs `k * s >= k * theta` in
//! `f32`.
//!
//! # Determinism and cost
//!
//! Engrams are kept and searched in id order; the only maps are `BTreeMap`s. The plasticity work
//! (bind, contradiction, generalisation, decay) is counted in [`Engrams::take_work`] as one cell
//! update per cell created or re-parameterised and one synapse traversal per synapse created or
//! re-weighted (an analogy, stated in `DESIGN.md`), for the adapter to price.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::archetype::Archetype;
use crate::medium::{Medium, TickSummary};
use crate::oscillome::Oscillome;
use crate::persist::{DecodeError, Reader, Writer};
use crate::ports::{Persist, Plasticity};
use crate::spec::{CellSpec, MediumSpec, SpecError, SynapseSpec};
use crate::types::{CellId, EventRef, Gate, Limits, OpCounts, P, Pattern, Prices, Proposal, S};
use crate::types::{SynapseId, Tag};

/// Most features in a key: the slots of a sliding coincidence.
pub const MAX_FEATURES: usize = S;

/// The parameters of an engram store. Fixed before a run; nothing here is learned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngramParams {
    /// The domain of every feature pattern.
    pub domain: u16,
    /// The coincidence window and the support lookback, in ticks.
    pub window_ticks: u32,
    /// Fewest features a key must have to be bound, and that generalisation keeps (1 to 8).
    pub min_features: u8,
    /// What a bind adds to a strength.
    pub gain: f32,
    /// The most a strength may be.
    pub max_strength: f32,
    /// The strength a recall needs.
    pub threshold: f32,
    /// What a contradiction takes from a strength.
    pub penalty: f32,
    /// The factor a strength is multiplied by at each boundary of the decay rhythm, in `(0, 1]`.
    pub decay: f32,
    /// The index of the decay rhythm in the medium's oscillome.
    pub decay_rhythm: u8,
    /// Ticks after a recall before the same engram may recall again.
    pub refractory_ticks: u32,
    /// Whether a bind that matches no engram exactly may narrow the closest engram with the same
    /// outcome to the features both keys share (generalisation by intersection).
    pub generalise: bool,
    /// The kind of a recall proposal.
    pub kind: u16,
}

impl EngramParams {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), &'static str> {
        let finite = [
            self.gain,
            self.max_strength,
            self.threshold,
            self.penalty,
            self.decay,
        ];
        if finite.iter().any(|x| !x.is_finite()) {
            return Err("engram parameters must be finite");
        }
        if !(1..=MAX_FEATURES as u8).contains(&self.min_features) {
            return Err("min_features must be 1 to 8");
        }
        if self.gain <= 0.0 || self.threshold <= 0.0 || self.max_strength < self.gain {
            return Err("gain and threshold must be positive, max_strength at least gain");
        }
        if self.penalty < 0.0 || !(self.decay > 0.0 && self.decay <= 1.0) {
            return Err("penalty must be at least 0, decay in (0, 1]");
        }
        let int = 16_777_216u32;
        if self.window_ticks > int || self.refractory_ticks > int {
            return Err("window and refractory must be at most 2^24 ticks");
        }
        Ok(())
    }
}

/// Where a key's features must occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum KeySite {
    /// At one node, whichever (family-keyed: the site is a variable).
    Variable,
    /// At this node (site-keyed).
    Fixed(u16),
}

impl KeySite {
    /// Whether an engram with site `self` would recall on a pattern with site `other`.
    pub fn compatible(self, other: KeySite) -> bool {
        match (self, other) {
            (KeySite::Fixed(a), KeySite::Fixed(b)) => a == b,
            _ => true,
        }
    }
}

/// A key: distinct feature tags, at most [`MAX_FEATURES`], in the order given, and a site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    features: Vec<Tag>,
    /// Where the features must occur.
    pub site: KeySite,
}

impl Key {
    /// A key from `features` (repeats dropped, the first [`MAX_FEATURES`] distinct kept, in the
    /// order given) at `site`.
    pub fn new(features: impl IntoIterator<Item = Tag>, site: KeySite) -> Key {
        let mut out: Vec<Tag> = Vec::new();
        for t in features {
            if out.len() == MAX_FEATURES {
                break;
            }
            if !out.contains(&t) {
                out.push(t);
            }
        }
        Key {
            features: out,
            site,
        }
    }

    /// The features, in order.
    pub fn features(&self) -> &[Tag] {
        &self.features
    }
}

/// The site an outcome names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OutcomeSite {
    /// None (for example "not an incident").
    None,
    /// The node of the event that anchors the recall, substituted at recall.
    Support,
    /// This node.
    Fixed(u16),
}

/// What an engram recalls: a tag whose meaning is the adapter's, and a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Outcome {
    /// The outcome's tag.
    pub tag: Tag,
    /// Its site.
    pub site: OutcomeSite,
}

/// One engram: its key, outcome, strength, counts and cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Engram {
    /// The key as created; `live` says which features remain.
    pub key: Key,
    /// Per feature of `key`: still in the key (generalisation drops features).
    pub live: Vec<bool>,
    /// What it recalls.
    pub outcome: Outcome,
    /// Its strength (the weight of `latch[e] -> emit[e]`).
    pub strength: f32,
    /// Binds that created or strengthened it.
    pub binds: u32,
    /// Contradictions that weakened it.
    pub contradictions: u32,
    /// Recalls it made.
    pub recalls: u32,
    /// Its key cells, with the node each watches, ascending by node.
    pub coincidences: Vec<(u16, CellId)>,
    /// The input synapses of its key cells: `inputs[c * k + f]` is feature `f` into key cell `c`,
    /// with `k` the number of features of `key`.
    pub inputs: Vec<SynapseId>,
    /// Its latch.
    pub latch: CellId,
    /// Its emitter.
    pub emit: CellId,
    /// The synapse whose weight is its strength.
    pub strength_synapse: SynapseId,
}

impl Engram {
    /// The features still in the key, in order.
    pub fn live_features(&self) -> Vec<Tag> {
        self.key
            .features
            .iter()
            .zip(&self.live)
            .filter(|(_, l)| **l)
            .map(|(t, _)| *t)
            .collect()
    }

    fn n_live(&self) -> usize {
        self.live.iter().filter(|l| **l).count()
    }
}

/// What a bind did to the engram it concerns.
#[derive(Debug, Clone, PartialEq)]
pub enum BindResult {
    /// A new engram (its index).
    Created(usize),
    /// An engram with exactly this key and outcome was strengthened.
    Strengthened(usize),
    /// An engram with this outcome was narrowed to the shared features and strengthened.
    Generalised(usize),
    /// The key had fewer than `min_features` features; nothing was bound.
    TooFewFeatures,
    /// The cells or synapses would exceed the medium's limits (or the medium refused them).
    Refused(SpecError),
}

/// What one bind did.
#[derive(Debug, Clone, PartialEq)]
pub struct Bind {
    /// To the engram of this key and outcome.
    pub result: BindResult,
    /// The engrams it contradicted (weakened), by index.
    pub contradicted: Vec<usize>,
}

/// A recall, resolved from an emitter's proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct Recall {
    /// The engram, by index.
    pub engram: usize,
    /// Its outcome (a `Support` site is the anchor event's node, which the adapter resolves).
    pub outcome: Outcome,
    /// Its strength at the recall.
    pub strength: f32,
    /// The earliest event of the coincidence that fired.
    pub anchor: EventRef,
    /// Its events within the window.
    pub refs: Vec<EventRef>,
}

/// Counts over the store's life (carried with it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngramStats {
    /// Binds asked for.
    pub binds: u64,
    /// Engrams created.
    pub created: u64,
    /// Exact strengthenings.
    pub strengthened: u64,
    /// Generalisations.
    pub generalised: u64,
    /// Engrams weakened by contradiction (one per engram per bind).
    pub contradictions: u64,
    /// Binds refused for too few features.
    pub too_few: u64,
    /// Binds refused by the limits.
    pub refused: u64,
    /// Decay boundaries applied.
    pub decays: u64,
    /// Recalls resolved.
    pub recalls: u64,
}

/// The engram table and the plasticity adapter that builds, strengthens, weakens and decays
/// engrams in a medium. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Engrams {
    params: EngramParams,
    nodes: Vec<u16>,
    engrams: Vec<Engram>,
    senses: BTreeMap<(u16, Tag), CellId>,
    by_emit: BTreeMap<CellId, usize>,
    stats: EngramStats,
    work: OpCounts,
}

fn params8(values: &[f32]) -> [f32; P] {
    let mut p = [0.0; P];
    p[..values.len()].copy_from_slice(values);
    p
}

impl Engrams {
    /// An empty store. `nodes` are the nodes a variable site ranges over (sorted and deduplicated
    /// here).
    pub fn new(params: EngramParams, nodes: &[u16]) -> Result<Engrams, &'static str> {
        params.validate()?;
        let mut nodes = nodes.to_vec();
        nodes.sort_unstable();
        nodes.dedup();
        if nodes.is_empty() {
            return Err("an engram store needs at least one node");
        }
        Ok(Engrams {
            params,
            nodes,
            engrams: Vec::new(),
            senses: BTreeMap::new(),
            by_emit: BTreeMap::new(),
            stats: EngramStats::default(),
            work: OpCounts::default(),
        })
    }

    /// The spec of an empty engram medium: no cells, the given limits and prices (with at least
    /// four passes, so that a feature reaches the emitter within its tick), and the oscillome on
    /// with the tick length, one rhythm (the decay rhythm, rhythm 0) and cycle summaries.
    pub fn medium_spec(
        tick_len_ns: u64,
        decay_period_ns: u64,
        limits: Limits,
        prices: Prices,
    ) -> MediumSpec {
        MediumSpec {
            cells: Vec::new(),
            synapses: Vec::new(),
            limits: Limits {
                max_passes: limits.max_passes.max(4),
                ..limits
            },
            prices,
            oscillome: Oscillome {
                tick_len_ns,
                periods_ns: vec![decay_period_ns],
                cycle_summary: true,
                ..Oscillome::default()
            },
        }
    }

    /// The parameters.
    pub fn params(&self) -> &EngramParams {
        &self.params
    }

    /// The nodes a variable site ranges over.
    pub fn nodes(&self) -> &[u16] {
        &self.nodes
    }

    /// The engrams, in creation order.
    pub fn engrams(&self) -> &[Engram] {
        &self.engrams
    }

    /// Counts over the store's life.
    pub fn stats(&self) -> &EngramStats {
        &self.stats
    }

    /// The plasticity work since the last call, and forget it.
    pub fn take_work(&mut self) -> OpCounts {
        std::mem::take(&mut self.work)
    }

    fn set_strength(&mut self, medium: &mut Medium, i: usize, s: f32) {
        let e = &mut self.engrams[i];
        e.strength = s.clamp(0.0, self.params.max_strength);
        // The synapse is the engram's own, plastic, and the value is finite: a refusal is a
        // defect of the table, and the table keeps its own value either way.
        let _ = medium.set_weight(e.strength_synapse, e.strength);
        self.work.synapse_traversals += 1;
    }

    /// Bind `key` to `outcome` in `medium`, between ticks: contradict, then strengthen an exact
    /// match, else generalise (if on), else create. See `DESIGN.md`, "Bind".
    pub fn bind(&mut self, medium: &mut Medium, key: &Key, outcome: Outcome) -> Bind {
        self.stats.binds += 1;
        let feats = key.features.clone();
        if feats.len() < usize::from(self.params.min_features) {
            self.stats.too_few += 1;
            return Bind {
                result: BindResult::TooFewFeatures,
                contradicted: Vec::new(),
            };
        }
        // 1. Contradiction: every engram that would recall on this pattern with another outcome.
        let mut contradicted = Vec::new();
        for i in 0..self.engrams.len() {
            let e = &self.engrams[i];
            if e.outcome != outcome
                && e.key.site.compatible(key.site)
                && e.n_live() > 0
                && e.live_features().iter().all(|t| feats.contains(t))
            {
                let s = (e.strength - self.params.penalty).max(0.0);
                self.engrams[i].contradictions += 1;
                self.set_strength(medium, i, s);
                self.stats.contradictions += 1;
                contradicted.push(i);
            }
        }
        // 2. An engram with exactly this key (live features, as a set) and outcome.
        let same = |e: &Engram| {
            let live = e.live_features();
            e.outcome == outcome
                && e.key.site == key.site
                && live.len() == feats.len()
                && live.iter().all(|t| feats.contains(t))
        };
        if let Some(i) = self.engrams.iter().position(same) {
            self.strengthen(medium, i);
            self.stats.strengthened += 1;
            return Bind {
                result: BindResult::Strengthened(i),
                contradicted,
            };
        }
        // 3. Generalisation: the engram with this outcome sharing the most features.
        if self.params.generalise {
            let min = usize::from(self.params.min_features);
            let mut best: Option<(usize, usize)> = None;
            for (i, e) in self.engrams.iter().enumerate() {
                if e.outcome != outcome || e.key.site != key.site {
                    continue;
                }
                let shared = e
                    .live_features()
                    .iter()
                    .filter(|t| feats.contains(t))
                    .count();
                if shared >= min && best.is_none_or(|(_, b)| shared > b) {
                    best = Some((i, shared));
                }
            }
            if let Some((i, _)) = best
                && self.narrow(medium, i, &feats).is_ok()
            {
                self.strengthen(medium, i);
                self.stats.generalised += 1;
                return Bind {
                    result: BindResult::Generalised(i),
                    contradicted,
                };
            }
        }
        // 4. A new engram.
        match self.create(medium, &feats, key.site, outcome) {
            Ok(i) => {
                self.stats.created += 1;
                Bind {
                    result: BindResult::Created(i),
                    contradicted,
                }
            }
            Err(e) => {
                self.stats.refused += 1;
                Bind {
                    result: BindResult::Refused(e),
                    contradicted,
                }
            }
        }
    }

    fn strengthen(&mut self, medium: &mut Medium, i: usize) {
        let s = self.engrams[i].strength + self.params.gain;
        self.engrams[i].binds += 1;
        self.set_strength(medium, i, s);
    }

    /// Keep only the live features of engram `i` that are in `feats`: their input synapses go to
    /// weight 0, and the key cells', latch's and emitter's thresholds follow the new count.
    fn narrow(&mut self, medium: &mut Medium, i: usize, feats: &[Tag]) -> Result<(), SpecError> {
        let e = &self.engrams[i];
        let k = e.key.features.len();
        let drop: Vec<usize> = (0..k)
            .filter(|&f| e.live[f] && !feats.contains(&e.key.features[f]))
            .collect();
        if drop.is_empty() {
            return Ok(());
        }
        let n = (e.n_live() - drop.len()) as f32;
        let mut changes: Vec<(CellId, usize, f32)> = e
            .coincidences
            .iter()
            .map(|&(_, c)| (c, 0usize, n))
            .collect();
        changes.push((e.latch, 0, n));
        changes.push((e.emit, 0, n * self.params.threshold));
        medium.set_params(&changes)?;
        self.work.cell_updates += changes.len() as u64;
        let e = &mut self.engrams[i];
        for c in 0..e.coincidences.len() {
            for &f in &drop {
                let _ = medium.set_weight(e.inputs[c * k + f], 0.0);
                self.work.synapse_traversals += 1;
            }
        }
        for &f in &drop {
            e.live[f] = false;
        }
        Ok(())
    }

    /// Build a new engram for `feats` at `site` with `outcome`, strength `gain`.
    fn create(
        &mut self,
        medium: &mut Medium,
        feats: &[Tag],
        site: KeySite,
        outcome: Outcome,
    ) -> Result<usize, SpecError> {
        let p = self.params;
        let nodes: Vec<u16> = match site {
            KeySite::Fixed(n) => vec![n],
            KeySite::Variable => self.nodes.clone(),
        };
        let k = feats.len();
        let base = medium.cells().len() as u32;
        let mut cells: Vec<CellSpec> = Vec::new();
        let mut new_senses: BTreeMap<(u16, Tag), CellId> = BTreeMap::new();
        for &n in &nodes {
            for &t in feats {
                if !self.senses.contains_key(&(n, t)) && !new_senses.contains_key(&(n, t)) {
                    new_senses.insert((n, t), CellId(base + cells.len() as u32));
                    cells.push(CellSpec {
                        archetype: Archetype::Sense,
                        params: params8(&[0.0]),
                        pattern: Some(Pattern {
                            domain: Some(p.domain),
                            node: Some(n),
                            channel: None,
                            tag: Some(t),
                        }),
                    });
                }
            }
        }
        let w = p.window_ticks as f32;
        let kf = k as f32;
        let mut coincidences = Vec::with_capacity(nodes.len());
        for &n in &nodes {
            coincidences.push((n, CellId(base + cells.len() as u32)));
            cells.push(CellSpec {
                archetype: Archetype::Coincidence,
                params: params8(&[kf, w, 1.0, w]),
                pattern: None,
            });
        }
        let latch = CellId(base + cells.len() as u32);
        cells.push(CellSpec {
            archetype: Archetype::Latch,
            params: params8(&[kf, 0.0]),
            pattern: None,
        });
        let emit = CellId(base + cells.len() as u32);
        cells.push(CellSpec {
            archetype: Archetype::Emit,
            params: params8(&[
                kf * p.threshold,
                f32::from(p.kind),
                w,
                p.refractory_ticks as f32,
            ]),
            pattern: None,
        });
        let sense_of = |n: u16, t: Tag| -> CellId {
            self.senses
                .get(&(n, t))
                .or_else(|| new_senses.get(&(n, t)))
                .copied()
                .unwrap_or(CellId(u32::MAX))
        };
        let syn = |from: CellId, to: CellId, weight: f32, plastic: bool| SynapseSpec {
            from,
            to,
            weight,
            delay_ticks: 0,
            gate: Gate::None,
            plastic,
        };
        let sbase = medium.synapses().len() as u32;
        let mut synapses: Vec<SynapseSpec> = Vec::new();
        let mut inputs = Vec::with_capacity(nodes.len() * k);
        for &(n, c) in &coincidences {
            for &t in feats {
                inputs.push(SynapseId(sbase + synapses.len() as u32));
                synapses.push(syn(sense_of(n, t), c, 1.0, true));
            }
        }
        for &(_, c) in &coincidences {
            synapses.push(syn(c, latch, 1.0, false));
        }
        let strength_synapse = SynapseId(sbase + synapses.len() as u32);
        let strength = p.gain.min(p.max_strength);
        synapses.push(syn(latch, emit, strength, true));
        medium.grow(&cells, &synapses)?;
        self.work.cell_updates += cells.len() as u64;
        self.work.synapse_traversals += synapses.len() as u64;
        self.senses.extend(new_senses);
        let i = self.engrams.len();
        self.by_emit.insert(emit, i);
        self.engrams.push(Engram {
            key: Key {
                features: feats.to_vec(),
                site,
            },
            live: vec![true; k],
            outcome,
            strength,
            binds: 1,
            contradictions: 0,
            recalls: 0,
            coincidences,
            inputs,
            latch,
            emit,
            strength_synapse,
        });
        Ok(i)
    }

    /// Every engram's strength times the decay factor (one boundary of the decay rhythm).
    pub fn decay(&mut self, medium: &mut Medium) {
        self.stats.decays += 1;
        for i in 0..self.engrams.len() {
            let s = self.engrams[i].strength;
            if s > 0.0 {
                self.set_strength(medium, i, s * self.params.decay);
            }
        }
    }

    /// The recall a proposal is, if it is one of an engram's emitters'.
    pub fn recall(&mut self, proposal: &Proposal) -> Option<Recall> {
        if proposal.kind != self.params.kind {
            return None;
        }
        let i = *self.by_emit.get(&proposal.cell)?;
        let e = &mut self.engrams[i];
        e.recalls += 1;
        self.stats.recalls += 1;
        Some(Recall {
            engram: i,
            outcome: e.outcome,
            strength: e.strength,
            anchor: proposal.anchor,
            refs: proposal.refs.clone(),
        })
    }
}

impl Plasticity for Engrams {
    /// At a boundary of the decay rhythm, decay every strength.
    fn end_of_tick(&mut self, medium: &mut Medium, summary: &TickSummary) {
        if summary
            .completed
            .iter()
            .any(|c| c.rhythm == self.params.decay_rhythm)
        {
            self.decay(medium);
        }
    }
}

// ---- persistence ------------------------------------------------------------------------------

const TABLE_MAGIC: &[u8; 4] = b"GENG";
const PAIR_MAGIC: &[u8; 4] = b"GEMB";
const TABLE_VERSION: u8 = 1;

fn write_site(w: &mut Writer, site: KeySite) {
    match site {
        KeySite::Variable => w.u8(0),
        KeySite::Fixed(n) => {
            w.u8(1);
            w.u16(n);
        }
    }
}

fn read_site(r: &mut Reader<'_>) -> Result<KeySite, DecodeError> {
    match r.u8()? {
        0 => Ok(KeySite::Variable),
        1 => Ok(KeySite::Fixed(r.u16()?)),
        t => Err(DecodeError::BadTag(t)),
    }
}

impl Engrams {
    /// The table's bytes (the medium's are separate). Little-endian, no padding, `f32` as bits:
    ///
    /// ```text
    /// magic "GENG", version u8 (1)
    /// params: domain u16, window u32, min_features u8, gain, max_strength, threshold, penalty,
    ///         decay (f32 bits each), decay_rhythm u8, refractory u32, generalise u8, kind u16
    /// nodes: count u32, u16 each
    /// stats: binds, created, strengthened, generalised, contradictions, too_few, refused,
    ///        decays, recalls (u64 each)
    /// senses: count u32, then node u16, tag u32, cell u32 (ascending by (node, tag))
    /// engrams: count u32, then per engram: site (u8 0 variable | 1 fixed + u16), features
    ///          (count u8, tag u32 each), live (u8 each), outcome tag u32, outcome site (u8 0
    ///          none | 1 support | 2 fixed + u16), strength f32 bits, binds, contradictions,
    ///          recalls (u32 each), coincidences (count u32, node u16 + cell u32 each), inputs
    ///          (count u32, u32 each), latch u32, emit u32, strength synapse u32
    /// ```
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.0.extend_from_slice(TABLE_MAGIC);
        w.u8(TABLE_VERSION);
        let p = &self.params;
        w.u16(p.domain);
        w.u32(p.window_ticks);
        w.u8(p.min_features);
        for x in [p.gain, p.max_strength, p.threshold, p.penalty, p.decay] {
            w.f32(x);
        }
        w.u8(p.decay_rhythm);
        w.u32(p.refractory_ticks);
        w.u8(u8::from(p.generalise));
        w.u16(p.kind);
        w.len32(self.nodes.len());
        for n in &self.nodes {
            w.u16(*n);
        }
        let s = &self.stats;
        for v in [
            s.binds,
            s.created,
            s.strengthened,
            s.generalised,
            s.contradictions,
            s.too_few,
            s.refused,
            s.decays,
            s.recalls,
        ] {
            w.u64(v);
        }
        w.len32(self.senses.len());
        for ((n, t), c) in &self.senses {
            w.u16(*n);
            w.u32(t.0);
            w.u32(c.0);
        }
        w.len32(self.engrams.len());
        for e in &self.engrams {
            write_site(&mut w, e.key.site);
            w.u8(e.key.features.len() as u8);
            for t in &e.key.features {
                w.u32(t.0);
            }
            for l in &e.live {
                w.u8(u8::from(*l));
            }
            w.u32(e.outcome.tag.0);
            match e.outcome.site {
                OutcomeSite::None => w.u8(0),
                OutcomeSite::Support => w.u8(1),
                OutcomeSite::Fixed(n) => {
                    w.u8(2);
                    w.u16(n);
                }
            }
            w.f32(e.strength);
            w.u32(e.binds);
            w.u32(e.contradictions);
            w.u32(e.recalls);
            w.len32(e.coincidences.len());
            for (n, c) in &e.coincidences {
                w.u16(*n);
                w.u32(c.0);
            }
            w.len32(e.inputs.len());
            for s in &e.inputs {
                w.u32(s.0);
            }
            w.u32(e.latch.0);
            w.u32(e.emit.0);
            w.u32(e.strength_synapse.0);
        }
        w.0
    }

    /// Decode a table and check it against `medium`: every cell and synapse it names exists and
    /// has the archetype, pattern and wiring the table says, every strength equals its synapse's
    /// weight, and re-encoding gives the same bytes. Never panics.
    pub fn from_bytes(bytes: &[u8], medium: &Medium) -> Result<Engrams, DecodeError> {
        let mut r = Reader { rest: bytes };
        if r.take::<4>()? != *TABLE_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let v = r.u8()?;
        if v != TABLE_VERSION {
            return Err(DecodeError::UnsupportedVersion(v));
        }
        let params = EngramParams {
            domain: r.u16()?,
            window_ticks: r.u32()?,
            min_features: r.u8()?,
            gain: r.f32()?,
            max_strength: r.f32()?,
            threshold: r.f32()?,
            penalty: r.f32()?,
            decay: r.f32()?,
            decay_rhythm: r.u8()?,
            refractory_ticks: r.u32()?,
            generalise: r.flag()?,
            kind: r.u16()?,
        };
        let n = r.count(2)?;
        let nodes: Vec<u16> = (0..n).map(|_| r.u16()).collect::<Result<_, _>>()?;
        let mut store = Engrams::new(params, &nodes)
            .map_err(|_| DecodeError::Inconsistent("engram parameters or nodes"))?;
        if store.nodes != nodes {
            return Err(DecodeError::Inconsistent(
                "engram nodes not sorted and distinct",
            ));
        }
        store.stats = EngramStats {
            binds: r.u64()?,
            created: r.u64()?,
            strengthened: r.u64()?,
            generalised: r.u64()?,
            contradictions: r.u64()?,
            too_few: r.u64()?,
            refused: r.u64()?,
            decays: r.u64()?,
            recalls: r.u64()?,
        };
        let cells = medium.cells();
        let synapses = medium.synapses();
        let cell = |id: u32, a: Archetype| -> Result<CellId, DecodeError> {
            match cells.get(id as usize) {
                Some(c) if c.archetype == a => Ok(CellId(id)),
                _ => Err(DecodeError::Inconsistent("engram cell")),
            }
        };
        let n = r.count(10)?;
        for _ in 0..n {
            let (node, tag) = (r.u16()?, Tag(r.u32()?));
            let c = cell(r.u32()?, Archetype::Sense)?;
            let want = Pattern {
                domain: Some(params.domain),
                node: Some(node),
                channel: None,
                tag: Some(tag),
            };
            if cells[c.0 as usize].pattern != Some(want)
                || store.senses.insert((node, tag), c).is_some()
            {
                return Err(DecodeError::Inconsistent("engram feature cell"));
            }
        }
        let n = r.count(1)?;
        for _ in 0..n {
            let site = read_site(&mut r)?;
            let k = usize::from(r.u8()?);
            if k > MAX_FEATURES {
                return Err(DecodeError::Inconsistent("engram key too long"));
            }
            let features: Vec<Tag> = (0..k).map(|_| r.u32().map(Tag)).collect::<Result<_, _>>()?;
            let live: Vec<bool> = (0..k).map(|_| r.flag()).collect::<Result<_, _>>()?;
            let tag = Tag(r.u32()?);
            let osite = match r.u8()? {
                0 => OutcomeSite::None,
                1 => OutcomeSite::Support,
                2 => OutcomeSite::Fixed(r.u16()?),
                t => return Err(DecodeError::BadTag(t)),
            };
            let strength = r.f32()?;
            let (binds, contradictions, recalls) = (r.u32()?, r.u32()?, r.u32()?);
            let m = r.count(6)?;
            let mut coincidences = Vec::with_capacity(m);
            for _ in 0..m {
                let node = r.u16()?;
                coincidences.push((node, cell(r.u32()?, Archetype::Coincidence)?));
            }
            let m = r.count(4)?;
            let inputs: Vec<SynapseId> = (0..m)
                .map(|_| r.u32().map(SynapseId))
                .collect::<Result<_, _>>()?;
            let latch = cell(r.u32()?, Archetype::Latch)?;
            let emit = cell(r.u32()?, Archetype::Emit)?;
            let strength_synapse = SynapseId(r.u32()?);
            let e = Engram {
                key: Key { features, site },
                live,
                outcome: Outcome { tag, site: osite },
                strength,
                binds,
                contradictions,
                recalls,
                coincidences,
                inputs,
                latch,
                emit,
                strength_synapse,
            };
            store.check(&e, synapses)?;
            if store.by_emit.insert(emit, store.engrams.len()).is_some() {
                return Err(DecodeError::Inconsistent("two engrams share an emitter"));
            }
            store.engrams.push(e);
        }
        if !r.rest.is_empty() {
            return Err(DecodeError::TrailingBytes);
        }
        if store.to_bytes() != bytes {
            return Err(DecodeError::Inconsistent("engram table does not re-encode"));
        }
        Ok(store)
    }

    /// The wiring an engram's table entry claims, against the medium's synapses.
    fn check(&self, e: &Engram, synapses: &[crate::medium::Synapse]) -> Result<(), DecodeError> {
        let bad = Err(DecodeError::Inconsistent("engram wiring"));
        let k = e.key.features.len();
        let sites_ok = match e.key.site {
            KeySite::Fixed(n) => e.coincidences.len() == 1 && e.coincidences[0].0 == n,
            KeySite::Variable => {
                e.coincidences.iter().map(|(n, _)| *n).collect::<Vec<_>>() == self.nodes
            }
        };
        if !sites_ok || e.inputs.len() != e.coincidences.len() * k || !e.strength.is_finite() {
            return bad;
        }
        let syn = |id: SynapseId| synapses.get(id.0 as usize);
        for (c, &(n, cell)) in e.coincidences.iter().enumerate() {
            for f in 0..k {
                let Some(s) = syn(e.inputs[c * k + f]) else {
                    return bad;
                };
                let from = self.senses.get(&(n, e.key.features[f]));
                if s.to != cell || Some(&s.from) != from || !s.plastic {
                    return bad;
                }
            }
        }
        match syn(e.strength_synapse) {
            Some(s)
                if s.from == e.latch
                    && s.to == e.emit
                    && s.plastic
                    && s.weight.to_bits() == e.strength.to_bits() => {}
            _ => return bad,
        }
        Ok(())
    }
}

/// Store a medium and its engram table through `port`, as one payload:
/// `"GEMB"`, the medium's byte length (u32), the medium's bytes, the table's bytes.
pub fn persist(medium: &Medium, engrams: &Engrams, port: &mut dyn Persist) {
    port.store(medium.last_tick(), pair_bytes(medium, engrams));
}

/// The bytes [`persist`] stores.
pub fn pair_bytes(medium: &Medium, engrams: &Engrams) -> Vec<u8> {
    let m = medium.to_bytes();
    let mut out = Vec::with_capacity(8 + m.len());
    out.extend_from_slice(PAIR_MAGIC);
    out.extend_from_slice(&u32::try_from(m.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&m);
    out.extend_from_slice(&engrams.to_bytes());
    out
}

/// Decode what [`persist`] stored. Never panics.
pub fn restore_pair(bytes: &[u8]) -> Result<(Medium, Engrams), DecodeError> {
    let mut r = Reader { rest: bytes };
    if r.take::<4>()? != *PAIR_MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let n = r.u32()? as usize;
    if n > r.rest.len() {
        return Err(DecodeError::Truncated);
    }
    let (m, t) = r.rest.split_at(n);
    let medium = Medium::from_bytes(m)?;
    let engrams = Engrams::from_bytes(t, &medium)?;
    Ok((medium, engrams))
}

/// The same medium with its activity forgotten: its structure and weights as they are now
/// (`Medium::spec`), every cell in its initial state, nothing in flight, no tick run. What carries
/// from one segment to the next.
pub fn restart(medium: &Medium) -> Result<Medium, SpecError> {
    Medium::from_spec(&medium.spec())
}
