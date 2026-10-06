//! Deterministic bytes of the whole medium (the persistence port's payload).
//!
//! # Encoding, version 1
//!
//! Little-endian, no padding. `f32` values are written as their bit patterns, so a restore is
//! exact. Tags are explicit and never derived from enum order.
//!
//! ```text
//! magic "GMED", version u8 = 1
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
//!            2 field + u8), plastic u8
//! pending: count of due ticks u32, then per due tick (ascending): tick u64, count u32, then per
//!            message: target u32, synapse u32, sent_tick u64, sent_pass u8, value u32 bits, refs
//! wake_next: count u32, cell ids u32 (ascending)
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
use crate::spec::{CellSpec, MediumSpec, SpecError, SynapseSpec};
use crate::types::{
    CellId, EventRef, Gate, Limits, OpCounts, P, Pattern, Prices, S, SynapseId, Tag,
};

const MAGIC: &[u8; 4] = b"GMED";
const VERSION: u8 = 1;

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

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn len32(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u64(x);
            }
        }
    }
    fn opt_u16(&mut self, v: Option<u16>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u16(x);
            }
        }
    }
    fn refs(&mut self, refs: &[EventRef]) {
        // A support never exceeds max_refs (a u16), so the count fits.
        self.u16(u16::try_from(refs.len()).unwrap_or(u16::MAX));
        for r in refs.iter().take(usize::from(u16::MAX)) {
            self.u64(r.tick);
            self.u32(r.offset_ns);
            self.u32(r.seq);
        }
    }
    fn counts(&mut self, c: &OpCounts) {
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

struct Reader<'a> {
    rest: &'a [u8],
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let (head, tail) = self
            .rest
            .split_first_chunk::<N>()
            .ok_or(DecodeError::Truncated)?;
        self.rest = tail;
        Ok(*head)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn flag(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            t => Err(DecodeError::BadTag(t)),
        }
    }
    fn opt_u64(&mut self) -> Result<Option<u64>, DecodeError> {
        Ok(if self.flag()? {
            Some(self.u64()?)
        } else {
            None
        })
    }
    fn opt_u16(&mut self) -> Result<Option<u16>, DecodeError> {
        Ok(if self.flag()? {
            Some(self.u16()?)
        } else {
            None
        })
    }
    /// A count of items each at least `min_len` bytes long, checked against the bytes left so a
    /// hostile count cannot demand a huge allocation.
    fn count(&mut self, min_len: usize) -> Result<usize, DecodeError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_len) > self.rest.len() {
            return Err(DecodeError::Truncated);
        }
        Ok(n)
    }
    fn refs(&mut self) -> Result<Vec<EventRef>, DecodeError> {
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
    fn counts(&mut self) -> Result<OpCounts, DecodeError> {
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
        w.0.extend_from_slice(MAGIC);
        w.u8(VERSION);
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
        w.0
    }

    /// Rebuild a medium from [`Medium::to_bytes`]. Never panics on malformed input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Medium, DecodeError> {
        let mut r = Reader { rest: bytes };
        if &r.take::<4>()? != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let version = r.u8()?;
        if version != VERSION {
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
        let spec = MediumSpec {
            cells,
            synapses,
            limits,
            prices,
        };
        spec.validate().map_err(DecodeError::Spec)?;

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
                let ok = spec
                    .synapses
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
            if c.0 as usize >= spec.cells.len() || wake_next.last().is_some_and(|l| *l >= c) {
                return Err(DecodeError::Inconsistent("wake list"));
            }
            wake_next.push(c);
        }
        if !r.rest.is_empty() {
            return Err(DecodeError::TrailingBytes);
        }
        Medium::from_parts(&spec, dynamic, pending, wake_next, last_tick, totals)
            .map_err(DecodeError::Spec)
    }
}
