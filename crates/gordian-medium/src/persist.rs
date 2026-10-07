//! Deterministic bytes of the whole medium (the persistence port's payload).
//!
//! # Encoding, versions 1 and 2
//!
//! Little-endian, no padding. `f32` values are written as their bit patterns, so a restore is
//! exact. Tags are explicit and never derived from enum order.
//!
//! Version 1 is M1's encoding, unchanged, and is written for every medium that uses no element of
//! the oscillome ([`MediumSpec::uses_oscillome`]): that is what makes the all-off medium M1's,
//! byte for byte (M1b decision 9). Version 2 is written for every other medium: version 1's
//! layout, in which an archetype tag may be 7 (`Oscillator`) and a gate tag 3 (phase), followed
//! by the oscillome section. Each version is refused for the other kind of medium, so a medium has
//! one encoding.
//!
//! ```text
//! magic "GMED", version u8 (1 or 2)
//! limits:  max_cells u32, max_synapses u32, max_ops_per_tick u64, max_proposals_per_tick u32,
//!          max_refs u16, max_passes u8
//! prices:  cell_update, synapse_traversal, event_routing, field_read, proposal (u64 ps each)
//! last_tick: opt u64           (opt = flag u8 0/1, then the value if 1)
//! totals:  cell_updates, synapse_traversals, event_routings, field_reads, proposals (u64 each)
//! cells:   count u32, then per cell:
//!            archetype tag u8, params [u32 bits; 8], state [u32 bits; 8], activation u32 bits,
//!            last_active opt u64, pattern opt (domain opt u16, node opt u16, channel opt u16,
//!            tag opt u32), support refs
//! synapses: count u32, then per synapse:
//!            from u32, to u32, weight u32 bits, delay u8, gate (tag u8: 0 none, 1 cell + u32,
//!            2 field + u8, 3 phase + rhythm u8 + from u32 bits + to u32 bits), plastic u8
//! pending: count of due ticks u32, then per due tick (ascending): tick u64, count u32, then per
//!            message: target u32, synapse u32, sent_tick u64, sent_pass u8, value u32 bits, refs
//! wake_next: count u32, cell ids u32 (ascending)
//! version 2 only, the oscillome:
//!          tick_len_ns u64, periods (count u8, then u64 each), seconds (count u32, then per entry:
//!          target (tag u8: 0 param + cell u32 + index u8, 1 delay + synapse u32), ns u64),
//!          cycle_summary u8, plasticity_rhythm opt u8, trace_rhythm opt u8,
//!          then, if cycle_summary, per rhythm in order: cycle u64, first_tick opt u64, ticks u64,
//!          counts (5 u64), passes, active_cell_ticks, truncated_ticks, carried, refs_dropped,
//!          unanchored (u64 each)
//! refs = count u16, then per ref: tick u64, offset_ns u32, seq u32
//! ```
//!
//! Decoding rejects a wrong magic or version, an unknown tag, a flag other than 0 or 1,
//! truncated input, trailing bytes, a structure the spec validation refuses, and dynamic state
//! that names cells or synapses that do not exist. It never panics.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::archetype::Archetype;
use crate::medium::{Medium, Message};
use crate::oscillome::{CycleSummary, Oscillome, TimeTarget, Timed};
use crate::spec::{CellSpec, MediumSpec, SpecError, SynapseSpec};
use crate::types::{
    CellId, EventRef, Gate, Limits, OpCounts, P, Pattern, Prices, S, SynapseId, Tag,
};

const MAGIC: &[u8; 4] = b"GMED";
/// M1's encoding: a medium that uses nothing of the oscillome.
const VERSION_M1: u8 = 1;
/// A medium that uses the oscillome.
const VERSION_OSCILLOME: u8 = 2;

/// Why persisted bytes could not be decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// The input ended early (or there was none).
    Truncated,
    /// Not a medium.
    BadMagic,
    /// A version this code does not read.
    UnsupportedVersion(u8),
    /// An unknown tag or flag.
    BadTag(u8),
    /// Bytes after the end.
    TrailingBytes,
    /// The structure is not a valid spec.
    Spec(SpecError),
    /// Dynamic state names a cell or synapse that does not exist, or is out of order.
    Inconsistent(&'static str),
}

pub(crate) struct Writer(pub(crate) Vec<u8>);

impl Writer {
    pub(crate) fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub(crate) fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub(crate) fn len32(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    pub(crate) fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u64(x);
            }
        }
    }
    pub(crate) fn opt_u16(&mut self, v: Option<u16>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u16(x);
            }
        }
    }
    pub(crate) fn opt_u8(&mut self, v: Option<u8>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u8(x);
            }
        }
    }
    pub(crate) fn oscillome(&mut self, o: &Oscillome) {
        self.u64(o.tick_len_ns);
        self.u8(u8::try_from(o.periods_ns.len()).unwrap_or(u8::MAX));
        for p in &o.periods_ns {
            self.u64(*p);
        }
        self.len32(o.seconds.len());
        for t in &o.seconds {
            match t.target {
                TimeTarget::Param { cell, index } => {
                    self.u8(0);
                    self.u32(cell.0);
                    self.u8(index);
                }
                TimeTarget::Delay { synapse } => {
                    self.u8(1);
                    self.u32(synapse.0);
                }
            }
            self.u64(t.ns);
        }
        self.u8(u8::from(o.cycle_summary));
        self.opt_u8(o.plasticity_rhythm);
        self.opt_u8(o.trace_rhythm);
    }
    pub(crate) fn cycle(&mut self, c: &CycleSummary) {
        self.u64(c.cycle);
        self.opt_u64(c.first_tick);
        self.u64(c.ticks);
        self.counts(&c.counts);
        for v in [
            c.passes,
            c.active_cell_ticks,
            c.truncated_ticks,
            c.carried,
            c.refs_dropped,
            c.unanchored,
        ] {
            self.u64(v);
        }
    }
    pub(crate) fn refs(&mut self, refs: &[EventRef]) {
        // A support never exceeds max_refs (a u16), so the count fits.
        self.u16(u16::try_from(refs.len()).unwrap_or(u16::MAX));
        for r in refs.iter().take(usize::from(u16::MAX)) {
            self.u64(r.tick);
            self.u32(r.offset_ns);
            self.u32(r.seq);
        }
    }
    pub(crate) fn counts(&mut self, c: &OpCounts) {
        for v in [
            c.cell_updates,
            c.synapse_traversals,
            c.event_routings,
            c.field_reads,
            c.proposals,
        ] {
            self.u64(v);
        }
    }
}

pub(crate) struct Reader<'a> {
    pub(crate) rest: &'a [u8],
}

impl Reader<'_> {
    pub(crate) fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let (head, tail) = self
            .rest
            .split_first_chunk::<N>()
            .ok_or(DecodeError::Truncated)?;
        self.rest = tail;
        Ok(*head)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    pub(crate) fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub(crate) fn flag(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            t => Err(DecodeError::BadTag(t)),
        }
    }
    pub(crate) fn opt_u64(&mut self) -> Result<Option<u64>, DecodeError> {
        Ok(if self.flag()? {
            Some(self.u64()?)
        } else {
            None
        })
    }
    pub(crate) fn opt_u16(&mut self) -> Result<Option<u16>, DecodeError> {
        Ok(if self.flag()? {
            Some(self.u16()?)
        } else {
            None
        })
    }
    pub(crate) fn opt_u8(&mut self) -> Result<Option<u8>, DecodeError> {
        Ok(if self.flag()? { Some(self.u8()?) } else { None })
    }
    pub(crate) fn oscillome(&mut self) -> Result<Oscillome, DecodeError> {
        let tick_len_ns = self.u64()?;
        let n = usize::from(self.u8()?);
        if n.saturating_mul(8) > self.rest.len() {
            return Err(DecodeError::Truncated);
        }
        let periods_ns = (0..n).map(|_| self.u64()).collect::<Result<_, _>>()?;
        let n = self.count(1 + 4 + 8)?;
        let mut seconds = Vec::with_capacity(n);
        for _ in 0..n {
            let target = match self.u8()? {
                0 => TimeTarget::Param {
                    cell: CellId(self.u32()?),
                    index: self.u8()?,
                },
                1 => TimeTarget::Delay {
                    synapse: SynapseId(self.u32()?),
                },
                t => return Err(DecodeError::BadTag(t)),
            };
            seconds.push(Timed {
                target,
                ns: self.u64()?,
            });
        }
        Ok(Oscillome {
            tick_len_ns,
            periods_ns,
            seconds,
            cycle_summary: self.flag()?,
            plasticity_rhythm: self.opt_u8()?,
            trace_rhythm: self.opt_u8()?,
        })
    }
    pub(crate) fn cycle(&mut self, rhythm: u8) -> Result<CycleSummary, DecodeError> {
        let cycle = self.u64()?;
        let first_tick = self.opt_u64()?;
        let ticks = self.u64()?;
        let counts = self.counts()?;
        Ok(CycleSummary {
            rhythm,
            cycle,
            first_tick,
            ticks,
            counts,
            passes: self.u64()?,
            active_cell_ticks: self.u64()?,
            truncated_ticks: self.u64()?,
            carried: self.u64()?,
            refs_dropped: self.u64()?,
            unanchored: self.u64()?,
        })
    }
    /// A count of items each at least `min_len` bytes long, checked against the bytes left so a
    /// hostile count cannot demand a huge allocation.
    pub(crate) fn count(&mut self, min_len: usize) -> Result<usize, DecodeError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_len) > self.rest.len() {
            return Err(DecodeError::Truncated);
        }
        Ok(n)
    }
    pub(crate) fn refs(&mut self) -> Result<Vec<EventRef>, DecodeError> {
        let n = usize::from(self.u16()?);
        if n.saturating_mul(16) > self.rest.len() {
            return Err(DecodeError::Truncated);
        }
        (0..n)
            .map(|_| {
                Ok(EventRef {
                    tick: self.u64()?,
                    offset_ns: self.u32()?,
                    seq: self.u32()?,
                })
            })
            .collect()
    }
    pub(crate) fn counts(&mut self) -> Result<OpCounts, DecodeError> {
        Ok(OpCounts {
            cell_updates: self.u64()?,
            synapse_traversals: self.u64()?,
            event_routings: self.u64()?,
            field_reads: self.u64()?,
            proposals: self.u64()?,
        })
    }
}

impl Medium {
    /// The persisted bytes of the whole medium (structure, state, messages in flight, wakes,
    /// the last tick and the running totals). Equal media give equal bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        let oscillome = self.uses_oscillome();
        w.0.extend_from_slice(MAGIC);
        w.u8(if oscillome {
            VERSION_OSCILLOME
        } else {
            VERSION_M1
        });
        let l = &self.limits;
        w.u32(l.max_cells);
        w.u32(l.max_synapses);
        w.u64(l.max_ops_per_tick);
        w.u32(l.max_proposals_per_tick);
        w.u16(l.max_refs);
        w.u8(l.max_passes);
        let p = &self.prices;
        for v in [
            p.cell_update_ps,
            p.synapse_traversal_ps,
            p.event_routing_ps,
            p.field_read_ps,
            p.proposal_ps,
        ] {
            w.u64(v);
        }
        w.opt_u64(self.last_tick);
        w.counts(&self.totals);
        w.len32(self.cells.len());
        for c in &self.cells {
            w.u8(c.archetype.tag());
            for x in c.params {
                w.f32(x);
            }
            for x in c.state {
                w.f32(x);
            }
            w.f32(c.activation);
            w.opt_u64(c.last_active);
            match c.pattern {
                None => w.u8(0),
                Some(p) => {
                    w.u8(1);
                    w.opt_u16(p.domain);
                    w.opt_u16(p.node);
                    w.opt_u16(p.channel);
                    match p.tag {
                        None => w.u8(0),
                        Some(Tag(t)) => {
                            w.u8(1);
                            w.u32(t);
                        }
                    }
                }
            }
            w.refs(&c.support);
        }
        w.len32(self.synapses.len());
        for s in &self.synapses {
            w.u32(s.from.0);
            w.u32(s.to.0);
            w.f32(s.weight);
            w.u8(s.delay_ticks);
            match s.gate {
                Gate::None => w.u8(0),
                Gate::Cell(c) => {
                    w.u8(1);
                    w.u32(c.0);
                }
                Gate::Field(k) => {
                    w.u8(2);
                    w.u8(k);
                }
                Gate::Phase { rhythm, from, to } => {
                    w.u8(3);
                    w.u8(rhythm);
                    w.f32(from);
                    w.f32(to);
                }
            }
            w.u8(u8::from(s.plastic));
        }
        w.len32(self.pending.len());
        for (due, messages) in &self.pending {
            w.u64(*due);
            w.len32(messages.len());
            for m in messages {
                w.u32(m.target.0);
                w.u32(m.synapse.0);
                w.u64(m.sent_tick);
                w.u8(m.sent_pass);
                w.f32(m.value);
                w.refs(&m.refs);
            }
        }
        w.len32(self.wake_next.len());
        for c in &self.wake_next {
            w.u32(c.0);
        }
        if oscillome {
            w.oscillome(&self.oscillome);
            for c in self.engine.summaries().unwrap_or(&[]) {
                w.cycle(c);
            }
        }
        w.0
    }

    /// Rebuild a medium from [`Medium::to_bytes`]. Never panics on malformed input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Medium, DecodeError> {
        let mut r = Reader { rest: bytes };
        if &r.take::<4>()? != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let version = r.u8()?;
        if version != VERSION_M1 && version != VERSION_OSCILLOME {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let limits = Limits {
            max_cells: r.u32()?,
            max_synapses: r.u32()?,
            max_ops_per_tick: r.u64()?,
            max_proposals_per_tick: r.u32()?,
            max_refs: r.u16()?,
            max_passes: r.u8()?,
        };
        let prices = Prices {
            cell_update_ps: r.u64()?,
            synapse_traversal_ps: r.u64()?,
            event_routing_ps: r.u64()?,
            field_read_ps: r.u64()?,
            proposal_ps: r.u64()?,
        };
        let last_tick = r.opt_u64()?;
        let totals = r.counts()?;
        let n_cells = r.count(1 + 4 * (P + S + 1) + 1 + 1 + 2)?;
        let mut cells = Vec::with_capacity(n_cells);
        let mut dynamic = Vec::with_capacity(n_cells);
        for _ in 0..n_cells {
            let tag = r.u8()?;
            let archetype = Archetype::from_tag(tag).ok_or(DecodeError::BadTag(tag))?;
            let mut params = [0.0f32; P];
            for x in params.iter_mut() {
                *x = r.f32()?;
            }
            let mut state = [0.0f32; S];
            for x in state.iter_mut() {
                *x = r.f32()?;
            }
            let activation = r.f32()?;
            let last_active = r.opt_u64()?;
            let pattern = if r.flag()? {
                Some(Pattern {
                    domain: r.opt_u16()?,
                    node: r.opt_u16()?,
                    channel: r.opt_u16()?,
                    tag: if r.flag()? { Some(Tag(r.u32()?)) } else { None },
                })
            } else {
                None
            };
            let support = r.refs()?;
            if support.windows(2).any(|w| w[0] >= w[1]) {
                return Err(DecodeError::Inconsistent("support not ascending"));
            }
            if support.len() > usize::from(limits.max_refs) {
                return Err(DecodeError::Inconsistent("support over max_refs"));
            }
            if [activation].iter().chain(state.iter()).any(|x| x.is_nan()) {
                return Err(DecodeError::Inconsistent("NaN in state"));
            }
            cells.push(CellSpec {
                archetype,
                params,
                pattern,
            });
            dynamic.push((last_active, state, activation, support));
        }
        let n_syn = r.count(4 + 4 + 4 + 1 + 1 + 1)?;
        let mut synapses = Vec::with_capacity(n_syn);
        for _ in 0..n_syn {
            let from = CellId(r.u32()?);
            let to = CellId(r.u32()?);
            let weight = r.f32()?;
            let delay_ticks = r.u8()?;
            let gate = match r.u8()? {
                0 => Gate::None,
                1 => Gate::Cell(CellId(r.u32()?)),
                2 => Gate::Field(r.u8()?),
                3 => Gate::Phase {
                    rhythm: r.u8()?,
                    from: r.f32()?,
                    to: r.f32()?,
                },
                t => return Err(DecodeError::BadTag(t)),
            };
            let plastic = r.flag()?;
            synapses.push(SynapseSpec {
                from,
                to,
                weight,
                delay_ticks,
                gate,
                plastic,
            });
        }
        // The structure is complete only after the oscillome section (version 2), at the end;
        // the dynamic state between is read first and checked against the structure afterwards.
        let n_due = r.count(12)?;
        let mut pending: BTreeMap<u64, Vec<Message>> = BTreeMap::new();
        let mut previous_due = None;
        for _ in 0..n_due {
            let due = r.u64()?;
            if previous_due.is_some_and(|p| p >= due) || last_tick.is_some_and(|t| due <= t) {
                return Err(DecodeError::Inconsistent("due ticks"));
            }
            previous_due = Some(due);
            let n = r.count(4 + 4 + 8 + 1 + 4 + 2)?;
            let mut messages = Vec::with_capacity(n);
            for _ in 0..n {
                let target = CellId(r.u32()?);
                let synapse = SynapseId(r.u32()?);
                let sent_tick = r.u64()?;
                let sent_pass = r.u8()?;
                let value = r.f32()?;
                let refs = r.refs()?;
                let ok = synapses
                    .get(synapse.0 as usize)
                    .is_some_and(|s| s.to == target);
                if !ok || value.is_nan() {
                    return Err(DecodeError::Inconsistent("message"));
                }
                messages.push(Message {
                    target,
                    synapse,
                    sent_tick,
                    sent_pass,
                    value,
                    refs: Arc::from(refs),
                });
            }
            pending.insert(due, messages);
        }
        let n_wake = r.count(4)?;
        let mut wake_next = Vec::with_capacity(n_wake);
        for _ in 0..n_wake {
            let c = CellId(r.u32()?);
            if c.0 as usize >= cells.len() || wake_next.last().is_some_and(|l| *l >= c) {
                return Err(DecodeError::Inconsistent("wake list"));
            }
            wake_next.push(c);
        }
        let (oscillome, summaries) = if version == VERSION_OSCILLOME {
            let o = r.oscillome()?;
            let summaries = if o.cycle_summary {
                let mut sums = Vec::with_capacity(o.periods_ns.len());
                for rhythm in 0..o.periods_ns.len() {
                    let c = r.cycle(rhythm as u8)?;
                    let consistent = c.first_tick.is_some() == (c.ticks > 0)
                        && c.first_tick
                            .is_none_or(|f| last_tick.is_some_and(|l| f <= l));
                    if !consistent {
                        return Err(DecodeError::Inconsistent("cycle summary"));
                    }
                    sums.push(c);
                }
                Some(sums)
            } else {
                None
            };
            (o, summaries)
        } else {
            (Oscillome::default(), None)
        };
        if !r.rest.is_empty() {
            return Err(DecodeError::TrailingBytes);
        }
        let spec = MediumSpec {
            cells,
            synapses,
            limits,
            prices,
            oscillome,
        };
        let (resolved, _) = spec.resolved().map_err(DecodeError::Spec)?;
        if spec.uses_oscillome() != (version == VERSION_OSCILLOME) {
            return Err(DecodeError::Inconsistent("encoding version"));
        }
        // A medium stores the converted values of its quantities in time; bytes whose stored
        // value differs from the conversion are refused, not repaired (compared by bits).
        let same_params = spec.cells.iter().zip(&resolved.cells).all(|(a, b)| {
            a.params
                .iter()
                .zip(&b.params)
                .all(|(x, y)| x.to_bits() == y.to_bits())
        });
        let same_delays = spec
            .synapses
            .iter()
            .zip(&resolved.synapses)
            .all(|(a, b)| a.delay_ticks == b.delay_ticks);
        if !(same_params && same_delays) {
            return Err(DecodeError::Inconsistent(
                "a stored value differs from its conversion",
            ));
        }
        Medium::from_parts(
            &spec, dynamic, pending, wake_next, last_tick, totals, summaries,
        )
        .map_err(DecodeError::Spec)
    }
}
