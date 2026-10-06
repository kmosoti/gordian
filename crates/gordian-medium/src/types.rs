//! The medium's vocabulary (docs/medium-ports.md, section 3): addresses, tags, events, ids, the
//! field, proposals, operation counts, prices and limits.

use gordian_core::{Charge, Resource as CoreResource};
use serde::{Deserialize, Serialize};

/// Number of parameters per cell.
pub const P: usize = 8;
/// Number of state scalars per cell.
pub const S: usize = 8;
/// Number of field scalars.
pub const F: usize = 4;

/// Public structure of the world: where an event came from. The medium attaches no meaning to the
/// three numbers; an adapter does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Address {
    /// A coarse partition of the world (for example a service graph).
    pub domain: u16,
    /// A place inside the domain (for example a service).
    pub node: u16,
    /// A kind of signal at the node (for example a counter name).
    pub channel: u16,
}

/// A public symbol attached to an event by the sense adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tag(pub u32);

/// A stable handle to an event.
///
/// Departure from docs/medium-ports.md section 3: the handle carries `offset_ns` beside
/// `(tick, seq)`. The anchoring rule orders events by `(tick, offset_ns)`, and a proposal's
/// consumer must be able to apply that order to the references alone. The derived order is
/// `(tick, offset_ns, seq)`; `seq` breaks ties between simultaneous events. `(tick, seq)` alone
/// still identifies the event (the medium refuses a tick whose events repeat a `seq`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventRef {
    /// The tick the event belongs to.
    pub tick: u64,
    /// Its position inside the tick, in nanoseconds.
    pub offset_ns: u32,
    /// Its sequence number, unique within the tick, assigned by the sense adapter.
    pub seq: u32,
}

/// One external event, as the sense adapter delivers it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The tick the event belongs to. Must equal the tick being run.
    pub tick: u64,
    /// Its position inside the tick, so that a long tick keeps fine timing.
    pub offset_ns: u32,
    /// Where it came from.
    pub source: Address,
    /// Public symbols attached to it.
    pub tags: Vec<Tag>,
    /// A scalar reading. Must be finite.
    pub value: f32,
    /// Sequence number, unique within the tick.
    pub seq: u32,
}

impl Event {
    /// The event's handle.
    pub fn reference(&self) -> EventRef {
        EventRef {
            tick: self.tick,
            offset_ns: self.offset_ns,
            seq: self.seq,
        }
    }
}

/// Dense id of a cell: its position in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellId(pub u32);

/// Dense id of a synapse: its position in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SynapseId(pub u32);

/// What must be active for a synapse to carry.
///
/// A cell gate is active when the gate cell ran in the current tick and its latest activation in
/// the tick is greater than zero; a field gate is active when the field scalar is greater than
/// zero. The gate is read when the synapse is traversed, after the pass in which its source ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gate {
    /// Always carries.
    None,
    /// Carries while this cell is active.
    Cell(CellId),
    /// Carries while this field scalar is positive. Reading it counts as one field read.
    Field(u8),
}

/// The broadcast scalars, supplied once per tick by the field adapter and read-only within it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Field {
    /// The scalars. Must be finite.
    pub scalars: [f32; F],
}

/// What an emitter cell proposes.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// The kind, from the emitter's parameters; its meaning belongs to the effector adapter.
    pub kind: u16,
    /// The earliest contributing event (section 5's anchoring rule).
    pub anchor: EventRef,
    /// The contributing events within the emitter's lookback, in [`EventRef`] order.
    pub refs: Vec<EventRef>,
    /// The emitter's input when it crossed its threshold.
    pub strength: f32,
    /// The emitter.
    pub cell: CellId,
}

/// Operations done in a tick (or summed over ticks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpCounts {
    /// Archetype functions run. A cell that runs twice in a tick (once per pass) counts twice.
    pub cell_updates: u64,
    /// Outgoing synapses of active cells examined, whether or not their gate let them carry.
    pub synapse_traversals: u64,
    /// One per (event, matching sense cell) delivery, and one per event that matched no cell.
    pub event_routings: u64,
    /// Field scalars read by archetypes and by field gates.
    pub field_reads: u64,
    /// Proposals handed to the effector.
    pub proposals: u64,
}

impl OpCounts {
    /// The operations that count against the per-tick limit: everything except proposals, which
    /// have their own limit.
    pub fn ops(&self) -> u64 {
        self.cell_updates
            .saturating_add(self.synapse_traversals)
            .saturating_add(self.event_routings)
            .saturating_add(self.field_reads)
    }

    /// Add `other` to these counts, saturating.
    pub fn accumulate(&mut self, other: &OpCounts) {
        self.cell_updates = self.cell_updates.saturating_add(other.cell_updates);
        self.synapse_traversals = self
            .synapse_traversals
            .saturating_add(other.synapse_traversals);
        self.event_routings = self.event_routings.saturating_add(other.event_routings);
        self.field_reads = self.field_reads.saturating_add(other.field_reads);
        self.proposals = self.proposals.saturating_add(other.proposals);
    }

    /// Modelled cost in picoseconds at `prices`: the weighted sum of the counts, saturating.
    pub fn modelled_ps(&self, prices: &Prices) -> u64 {
        [
            (self.cell_updates, prices.cell_update_ps),
            (self.synapse_traversals, prices.synapse_traversal_ps),
            (self.event_routings, prices.event_routing_ps),
            (self.field_reads, prices.field_read_ps),
            (self.proposals, prices.proposal_ps),
        ]
        .iter()
        .fold(0u64, |sum, (n, price)| {
            sum.saturating_add(n.saturating_mul(*price))
        })
    }

    /// The modelled cost as a compute charge in picoseconds, for a ledger adapter that bills it
    /// on a `gordian_core::Bill` (the A8b convention: counted operations priced by declared
    /// weights).
    pub fn compute_charge_ps(&self, prices: &Prices) -> Charge {
        Charge::new(CoreResource::Compute, self.modelled_ps(prices))
    }
}

/// Declared prices, in modelled picoseconds per operation (docs/medium-ports.md, section 7).
///
/// Picoseconds, as in `gordian-components`' operation weights, so that a calibrated price of a
/// fraction of a nanosecond keeps its precision in integer arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prices {
    /// Per cell update.
    pub cell_update_ps: u64,
    /// Per synapse traversal.
    pub synapse_traversal_ps: u64,
    /// Per event routing.
    pub event_routing_ps: u64,
    /// Per field read.
    pub field_read_ps: u64,
    /// Per proposal. Section 7 declares no price for proposals; the first value is zero.
    pub proposal_ps: u64,
}

impl Prices {
    /// The first guess of section 7: 20 ns per cell update, 5 ns per synapse traversal, 10 ns per
    /// event routing, 2 ns per field read, nothing per proposal.
    pub const DECLARED: Prices = Prices {
        cell_update_ps: 20_000,
        synapse_traversal_ps: 5_000,
        event_routing_ps: 10_000,
        field_read_ps: 2_000,
        proposal_ps: 0,
    };
}

impl Default for Prices {
    fn default() -> Self {
        Self::DECLARED
    }
}

/// Hard limits, always on (docs/medium-ports.md, sections 2 and 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Most cells a medium may have.
    pub max_cells: u32,
    /// Most synapses a medium may have.
    pub max_synapses: u32,
    /// Most operations ([`OpCounts::ops`]) in one tick. Work past it is not done, and the tick
    /// records a [`crate::Truncation`].
    pub max_ops_per_tick: u64,
    /// Most proposals handed to the effector in one tick. Further proposals are dropped and
    /// counted in the tick's [`crate::Truncation`].
    pub max_proposals_per_tick: u32,
    /// Most event references a cell keeps as its support, and a message carries. The earliest are
    /// kept; the number dropped is reported per tick.
    pub max_refs: u16,
    /// Passes per tick, the first included (section 4, step 5: "at most `max_passes` times",
    /// first value 2). Zero-delay messages sent in the last pass are carried to the next tick.
    pub max_passes: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_cells: 65_536,
            max_synapses: 1_048_576,
            max_ops_per_tick: 1_000_000,
            max_proposals_per_tick: 64,
            max_refs: 32,
            max_passes: 2,
        }
    }
}

/// Which external events a sense cell receives. `None` matches anything; an event matches when
/// every present field equals the event's and, if a tag is given, the event carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    /// Required domain.
    pub domain: Option<u16>,
    /// Required node.
    pub node: Option<u16>,
    /// Required channel.
    pub channel: Option<u16>,
    /// Required tag.
    pub tag: Option<Tag>,
}

impl Pattern {
    /// Whether `event` matches.
    pub fn matches(&self, event: &Event) -> bool {
        self.domain.is_none_or(|d| d == event.source.domain)
            && self.node.is_none_or(|n| n == event.source.node)
            && self.channel.is_none_or(|c| c == event.source.channel)
            && self.tag.is_none_or(|t| event.tags.contains(&t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modelled_cost_is_the_weighted_sum() {
        let counts = OpCounts {
            cell_updates: 3,
            synapse_traversals: 4,
            event_routings: 5,
            field_reads: 6,
            proposals: 7,
        };
        assert_eq!(counts.ops(), 18);
        assert_eq!(
            counts.modelled_ps(&Prices::DECLARED),
            3 * 20_000 + 4 * 5_000 + 5 * 10_000 + 6 * 2_000
        );
        assert_eq!(
            counts.compute_charge_ps(&Prices::DECLARED),
            Charge::new(CoreResource::Compute, 142_000)
        );
    }

    #[test]
    fn event_refs_order_by_tick_then_offset_then_seq() {
        let a = EventRef {
            tick: 1,
            offset_ns: 900,
            seq: 0,
        };
        let b = EventRef {
            tick: 2,
            offset_ns: 0,
            seq: 0,
        };
        let c = EventRef {
            tick: 2,
            offset_ns: 0,
            seq: 1,
        };
        assert!(a < b && b < c);
    }

    #[test]
    fn pattern_wildcards_and_tags() {
        let e = Event {
            tick: 0,
            offset_ns: 0,
            source: Address {
                domain: 1,
                node: 2,
                channel: 3,
            },
            tags: vec![Tag(9)],
            value: 1.0,
            seq: 0,
        };
        assert!(Pattern::default().matches(&e));
        let p = Pattern {
            node: Some(2),
            tag: Some(Tag(9)),
            ..Pattern::default()
        };
        assert!(p.matches(&e));
        assert!(
            !Pattern {
                tag: Some(Tag(8)),
                ..p
            }
            .matches(&e)
        );
        assert!(
            !Pattern {
                channel: Some(4),
                ..p
            }
            .matches(&e)
        );
    }
}
