//! The medium's state and the tick (docs/medium-ports.md, section 4).
//!
//! # The tick, in its total order
//!
//! For tick `T`:
//!
//! 1. **Clock and field.** `clock.now()` names `T`, which must be the tick after the last one
//!    run (the first tick may be any value); when the oscillome gives a tick length, the clock's
//!    must equal it. `field.field(T)` supplies the field; every scalar must be finite. The
//!    medium writes the oscillome's phases at `T` into the field (zeros when it has no rhythm).
//! 2. **Sense and routing.** `sense.events(T)` supplies the events. Each must belong to `T`, have
//!    a finite value and a `seq` not repeated within `T`; otherwise the tick is refused with a
//!    [`StepError`] and the medium is unchanged. The events are sorted by
//!    `(offset_ns, source, seq)` and routed in that order; each event goes to every sense cell
//!    whose pattern matches it, in cell id order. One routing per delivery, and one for an event
//!    that matches no cell.
//! 3. **Delivery.** Messages due at `T` (sent earlier with a delay, or carried from `T - 1`'s
//!    last pass) are delivered, and every cell that asked to be woken at `T` gets a wake.
//! 4. **Passes.** Pass 1 runs every cell with input, in id order. A cell's inputs are first put
//!    in canonical order (wakes; events in routing order; messages by `(sent tick, sent pass,
//!    synapse id)`). A cell runs at most once per pass. Cells without input do not run and are
//!    not counted.
//! 5. **Propagation**, after every cell of the pass has run: each cell whose activation is not
//!    zero traverses its outgoing synapses, cells in id order and each cell's synapses in id
//!    order. A synapse whose gate is active sends `weight * activation` with the cell's support
//!    as references: with delay `d >= 1` to tick `T + d`; with delay 0 to the next pass, or, if
//!    this was pass `max_passes`, to tick `T + 1`. If any zero-delay message went to the next
//!    pass, that pass runs (step 4 again) and propagates (step 5 again).
//! 6. **Proposals.** Emitters that fired during the passes made proposals, in (pass, cell id)
//!    order; the effector receives them.
//! 7. **Cycles and plasticity.** When the oscillome keeps cycle summaries, the tick's summary is
//!    added to every rhythm's cycle, and the cycles whose boundary `T` is are completed (in
//!    [`TickSummary::completed`]). Plasticity then runs: once with the tick's summary, or, when
//!    the oscillome names a plasticity rhythm, only at that rhythm's boundaries, with its
//!    completed cycle's summary.
//! 8. **Ledger and trace.** The ledger receives the counts and the truncation record, if any;
//!    the trace receives the tick's trace if it asked for this tick (when the oscillome names a
//!    trace rhythm, it is asked only on that rhythm's boundary ticks).
//!
//! # Limits
//!
//! Before each unit of work (one routing; one cell run with its field reads; one synapse
//! traversal with its field read) the tick checks that the work would keep its operations at or
//! under `max_ops_per_tick`. The first unit that would not is not done, and from then on nothing
//! more is routed, run or sent in this tick: the events not fully routed, the cells with input
//! that did not run and the synapses not traversed are counted in a [`Truncation`], which the
//! ledger receives. Proposals past `max_proposals_per_tick` are dropped and counted the same way.
//! Steps 6 to 8 still run. Nothing panics.

use std::collections::BTreeMap;
use std::mem;
use std::sync::Arc;

use crate::archetype::{self, Archetype, Ctx, Emission, Input, Origin, Refs, SupportRule, sane};
use crate::oscillome::{CycleSummary, Oscillome, OscillomeEngine};
use crate::ports::{Persist, Ports};
use crate::spec::{CellSpec, MediumSpec, SpecError, SynapseSpec};
use crate::types::{
    CellId, Event, EventRef, F, Field, Gate, Limits, OpCounts, P, Pattern, Prices, Proposal, R, S,
    SynapseId,
};

/// The sub-tick lookback (M3): keep the events of `support` (ascending) that are no earlier
/// than `sub_us` microseconds before the run's instant, in event time. The instant is `at_us`
/// (microseconds from the start of `tick`) when the archetype gives one, else the newest event in
/// the support. An event's time is `(its tick - tick) * tick_len_ns + offset_ns`, in integers, so
/// order inside a tick is read from `offset_ns` and the cut does not depend on where the tick's
/// edges fall. Events after the instant are kept: this is a lower edge, as the tick lookback is.
fn cut_below_tick(
    support: &mut Vec<EventRef>,
    tick: u64,
    tick_len_ns: u64,
    at_us: Option<i64>,
    sub_us: u64,
) {
    let len = i128::from(tick_len_ns);
    let time =
        |r: &EventRef| (i128::from(r.tick) - i128::from(tick)) * len + i128::from(r.offset_ns);
    let at = match at_us {
        Some(us) => i128::from(us) * 1_000,
        None => match support.last() {
            Some(r) => time(r),
            None => return,
        },
    };
    let floor = at - i128::from(sub_us) * 1_000;
    support.retain(|r| time(r) >= floor);
}

/// A cell: one instance of an archetype.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Its id.
    pub id: CellId,
    /// Its archetype.
    pub archetype: Archetype,
    /// Its parameters.
    pub params: [f32; P],
    /// Its state.
    pub state: [f32; S],
    /// Its activation at its latest run.
    pub activation: f32,
    /// The tick of its latest run; `None` before the first. (Departure: section 3 has a plain
    /// `u64`; `None` keeps "never ran" distinct from "ran at tick 0".)
    pub last_active: Option<u64>,
    /// The address pattern of a sense cell.
    pub pattern: Option<Pattern>,
    /// The events the cell currently cites, ascending, at most `max_refs`. (Departure: section 3
    /// has no such field; the anchoring rule of section 5 needs the events that contributed to an
    /// activation, and they cannot live in `[f32; S]`.)
    pub support: Vec<EventRef>,
}

impl Cell {
    /// The cell's activation as seen during `tick`: its latest activation if it ran in `tick`,
    /// otherwise zero. A cell that did not run is silent.
    pub fn activation_at(&self, tick: u64) -> f32 {
        if self.last_active == Some(tick) {
            self.activation
        } else {
            0.0
        }
    }
}

/// A synapse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Synapse {
    /// Its id.
    pub id: SynapseId,
    /// Source cell.
    pub from: CellId,
    /// Target cell.
    pub to: CellId,
    /// Weight.
    pub weight: f32,
    /// Delay in ticks.
    pub delay_ticks: u8,
    /// Gate.
    pub gate: Gate,
    /// Whether plasticity may change the weight.
    pub plastic: bool,
    /// Position among the target's incoming synapses, by id (derived; a coincidence cell's slot).
    pub(crate) slot: u32,
}

/// A message in flight.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Message {
    pub(crate) target: CellId,
    pub(crate) synapse: SynapseId,
    pub(crate) sent_tick: u64,
    pub(crate) sent_pass: u8,
    pub(crate) value: f32,
    pub(crate) refs: Arc<[EventRef]>,
}

/// What a tick that hit a limit did not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Truncation {
    /// The operation limit was reached.
    pub op_limit: bool,
    /// The proposal limit was reached.
    pub proposal_limit: bool,
    /// The pass in which the operation limit was reached (0: during routing).
    pub pass: u8,
    /// Events not routed to every matching cell.
    pub events_unrouted: u64,
    /// Cells with input that did not run.
    pub cells_not_run: u64,
    /// Synapses of active cells not traversed.
    pub messages_not_sent: u64,
    /// Proposals made but not handed to the effector.
    pub proposals_dropped: u64,
}

/// What a tick did.
#[derive(Debug, Clone, PartialEq)]
pub struct TickSummary {
    /// The tick.
    pub tick: u64,
    /// Its operations.
    pub counts: OpCounts,
    /// What it did not do, if it hit a limit.
    pub truncation: Option<Truncation>,
    /// Passes run (0 if no cell had input).
    pub passes: u8,
    /// Distinct cells that ran.
    pub active_cells: u64,
    /// Zero-delay messages of the last pass carried to the next tick.
    pub carried: u64,
    /// Event references dropped because a support exceeded `max_refs` (the earliest are kept).
    pub refs_dropped: u64,
    /// Emitter firings that made no proposal because they cited no event.
    pub unanchored: u64,
    /// The cycles completed at this tick, one per rhythm whose boundary it is, in rhythm order,
    /// when the oscillome keeps cycle summaries (M1b decision 6); else empty.
    pub completed: Vec<CycleSummary>,
}

/// One item of a tick's trace.
#[derive(Debug, Clone, PartialEq)]
pub enum TraceItem {
    /// An event delivered to a sense cell.
    Routed {
        /// The event.
        event: EventRef,
        /// The cell.
        cell: CellId,
    },
    /// An event that matched no sense cell.
    Unmatched {
        /// The event.
        event: EventRef,
    },
    /// A cell ran.
    Ran {
        /// The cell.
        cell: CellId,
        /// The pass.
        pass: u8,
        /// Its inputs, wakes included.
        inputs: u32,
        /// Field scalars it read.
        field_reads: u64,
        /// Its activation.
        activation: f32,
    },
    /// A cell with input did not run (truncation).
    NotRun {
        /// The cell.
        cell: CellId,
        /// The pass.
        pass: u8,
    },
    /// A synapse was traversed.
    Traversed {
        /// The synapse.
        synapse: SynapseId,
        /// The pass.
        pass: u8,
        /// Whether the gate was a field read.
        field_read: bool,
        /// Whether it carried (its gate was active).
        carried: bool,
    },
    /// A proposal reached the effector.
    Proposed {
        /// The emitter.
        cell: CellId,
        /// Its anchor.
        anchor: EventRef,
    },
    /// A proposal was dropped at the proposal limit.
    ProposalDropped {
        /// The emitter.
        cell: CellId,
    },
}

/// A sampled tick's activity.
#[derive(Debug, Clone, PartialEq)]
pub struct TickTrace {
    /// The tick.
    pub tick: u64,
    /// What happened, in the order it happened.
    pub items: Vec<TraceItem>,
}

/// Why a tick was refused. A refused tick changes nothing in the medium.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepError {
    /// The clock named a tick other than the one after the last.
    TickOutOfOrder {
        /// The tick expected.
        expected: Option<u64>,
        /// The tick named.
        got: u64,
    },
    /// An event's tick is not the tick being run.
    EventTickMismatch {
        /// The tick being run.
        tick: u64,
        /// The event.
        event: EventRef,
    },
    /// An event's value is NaN or infinite.
    NonFiniteEventValue(EventRef),
    /// Two events of the tick share a `seq`.
    DuplicateSeq {
        /// The tick.
        tick: u64,
        /// The repeated `seq`.
        seq: u32,
    },
    /// A field scalar is NaN or infinite.
    NonFiniteField {
        /// Its index.
        index: usize,
    },
    /// The clock's tick length is not the one the oscillome was built for.
    TickLength {
        /// The oscillome's.
        spec: u64,
        /// The clock's.
        clock: u64,
    },
}

/// Why [`Medium::set_weight`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightError {
    /// No such synapse.
    Unknown(SynapseId),
    /// The synapse is not plastic.
    NotPlastic(SynapseId),
    /// The weight is NaN or infinite.
    NonFinite,
}

/// Routing index key: a sense cell's pattern without its tag.
type IndexKey = (Option<u16>, Option<u16>, Option<u16>);

/// A cell that ran with a non-zero activation and has outgoing synapses: its id, activation and
/// the support its messages carry.
type Fired = (CellId, f32, Arc<[EventRef]>);

/// The dynamic part of a decoded cell: last run, state, activation, support.
pub(crate) type CellDynamic = (Option<u64>, [f32; S], f32, Vec<EventRef>);

/// The medium.
#[derive(Debug, Clone)]
pub struct Medium {
    pub(crate) cells: Vec<Cell>,
    pub(crate) synapses: Vec<Synapse>,
    pub(crate) limits: Limits,
    pub(crate) prices: Prices,
    /// Messages by the tick they are due.
    pub(crate) pending: BTreeMap<u64, Vec<Message>>,
    /// Cells to wake at the next tick, ascending.
    pub(crate) wake_next: Vec<CellId>,
    pub(crate) last_tick: Option<u64>,
    pub(crate) totals: OpCounts,
    /// The oscillome's spec (M1b), as given (with its quantities in time).
    pub(crate) oscillome: Oscillome,
    /// The Oscillome Engine; its cycle summaries are persisted.
    pub(crate) engine: OscillomeEngine,
    // Derived from cells and synapses; rebuilt on restore, never persisted.
    /// Per cell: an oscillator's period (its self-synapse's delay), else 0.
    period: Vec<u64>,
    out_edges: Vec<Vec<SynapseId>>,
    index: BTreeMap<IndexKey, Vec<CellId>>,
    // Scratch; empty between ticks.
    inbox: Vec<Vec<Input>>,
}

impl PartialEq for Medium {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

/// Bookkeeping of one tick in progress.
struct TickRun {
    tick: u64,
    max_ops: u64,
    ops: u64,
    counts: OpCounts,
    truncation: Truncation,
    stopped: bool,
    trace: Option<Vec<TraceItem>>,
    refs_dropped: u64,
    unanchored: u64,
    carried: u64,
    active_cells: u64,
}

impl TickRun {
    /// Whether `ops` more operations may be done; marks the tick truncated if not.
    fn allow(&mut self, ops: u64, pass: u8) -> bool {
        if self.stopped {
            return false;
        }
        if self.ops.saturating_add(ops) > self.max_ops {
            self.stopped = true;
            self.truncation.op_limit = true;
            self.truncation.pass = pass;
            return false;
        }
        self.ops += ops;
        true
    }

    fn trace(&mut self, item: impl FnOnce() -> TraceItem) {
        if let Some(t) = self.trace.as_mut() {
            t.push(item());
        }
    }
}

fn sorted_events(events: &mut [Event]) {
    events.sort_by_key(|e| (e.offset_ns, e.source, e.seq));
}

impl Medium {
    /// Build a medium from a spec, after validating it and converting its quantities in time
    /// ([`MediumSpec::resolved`]).
    pub fn from_spec(spec: &MediumSpec) -> Result<Medium, SpecError> {
        let (spec, _) = spec.resolved()?;
        let spec = &spec;
        let cells = spec
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| Cell {
                id: CellId(i as u32),
                archetype: c.archetype,
                params: c.params,
                state: c.archetype.initial_state(),
                activation: 0.0,
                last_active: None,
                pattern: c.pattern,
                support: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut incoming = vec![0u32; cells.len()];
        let synapses = spec
            .synapses
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let slot = incoming[s.to.0 as usize];
                incoming[s.to.0 as usize] += 1;
                Synapse {
                    id: SynapseId(i as u32),
                    from: s.from,
                    to: s.to,
                    weight: s.weight,
                    delay_ticks: s.delay_ticks,
                    gate: s.gate,
                    plastic: s.plastic,
                    slot,
                }
            })
            .collect();
        let mut medium = Medium {
            cells,
            synapses,
            limits: spec.limits,
            prices: spec.prices,
            pending: BTreeMap::new(),
            wake_next: Vec::new(),
            last_tick: None,
            totals: OpCounts::default(),
            oscillome: spec.oscillome.clone(),
            engine: OscillomeEngine::new(&spec.oscillome),
            period: Vec::new(),
            out_edges: Vec::new(),
            index: BTreeMap::new(),
            inbox: Vec::new(),
        };
        medium.derive();
        Ok(medium)
    }

    /// Rebuild the derived structures (outgoing lists, routing index, scratch) from the cells and
    /// synapses, in id order.
    fn derive(&mut self) {
        self.out_edges = vec![Vec::new(); self.cells.len()];
        self.period = vec![0; self.cells.len()];
        for s in &self.synapses {
            self.out_edges[s.from.0 as usize].push(s.id);
            if s.from == s.to && self.cells[s.from.0 as usize].archetype == Archetype::Oscillator {
                self.period[s.from.0 as usize] = u64::from(s.delay_ticks);
            }
        }
        self.index = BTreeMap::new();
        for c in &self.cells {
            if let Some(p) = c.pattern {
                self.index
                    .entry((p.domain, p.node, p.channel))
                    .or_default()
                    .push(c.id);
            }
        }
        self.inbox = vec![Vec::new(); self.cells.len()];
    }

    /// The current structure as a spec (weights as they are now).
    pub fn spec(&self) -> MediumSpec {
        MediumSpec {
            cells: self
                .cells
                .iter()
                .map(|c| CellSpec {
                    archetype: c.archetype,
                    params: c.params,
                    pattern: c.pattern,
                })
                .collect(),
            synapses: self
                .synapses
                .iter()
                .map(|s| SynapseSpec {
                    from: s.from,
                    to: s.to,
                    weight: s.weight,
                    delay_ticks: s.delay_ticks,
                    gate: s.gate,
                    plastic: s.plastic,
                })
                .collect(),
            limits: self.limits,
            prices: self.prices,
            oscillome: self.oscillome.clone(),
        }
    }

    /// The Oscillome Engine (phases, cycles, boundaries, and the cycle summaries in progress).
    pub fn engine(&self) -> &OscillomeEngine {
        &self.engine
    }

    /// Whether the medium uses any element of the oscillome (as [`MediumSpec::uses_oscillome`]).
    pub fn uses_oscillome(&self) -> bool {
        !self.oscillome.is_off()
            || self
                .cells
                .iter()
                .any(|c| c.archetype.uses_oscillome(&c.params))
            || self
                .synapses
                .iter()
                .any(|s| matches!(s.gate, Gate::Phase { .. }))
    }

    /// The cells, in id order.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The synapses, in id order.
    pub fn synapses(&self) -> &[Synapse] {
        &self.synapses
    }

    /// The limits.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The declared prices.
    pub fn prices(&self) -> &Prices {
        &self.prices
    }

    /// The last tick run.
    pub fn last_tick(&self) -> Option<u64> {
        self.last_tick
    }

    /// Operations summed over every tick run.
    pub fn totals(&self) -> &OpCounts {
        &self.totals
    }

    /// Messages in flight.
    pub fn pending_messages(&self) -> usize {
        self.pending.values().map(Vec::len).sum()
    }

    /// Change the weight of a plastic synapse.
    pub fn set_weight(&mut self, id: SynapseId, weight: f32) -> Result<(), WeightError> {
        if !weight.is_finite() {
            return Err(WeightError::NonFinite);
        }
        let s = self
            .synapses
            .get_mut(id.0 as usize)
            .ok_or(WeightError::Unknown(id))?;
        if !s.plastic {
            return Err(WeightError::NotPlastic(id));
        }
        s.weight = weight;
        Ok(())
    }

    /// Structural plasticity (work item A1a, DESIGN.md "The engram"): append `cells` (in their
    /// initial state) and `synapses` to the medium. The result is validated as one spec, existing
    /// cells and synapses included, against the limits, the archetypes' rules and the oscillome,
    /// so a plasticity adapter cannot bypass the hard limits on cells and synapses; on refusal the
    /// medium is unchanged. Ids of existing cells and synapses, their state, their supports, the
    /// messages in flight and the wakes are unchanged; a new synapse into an existing cell takes
    /// the next slot of that cell. The new cells' parameters are taken as given: quantities in time
    /// are not converted for them (give them in ticks). Returns the ids of the first new cell and
    /// the first new synapse.
    pub fn grow(
        &mut self,
        cells: &[CellSpec],
        synapses: &[SynapseSpec],
    ) -> Result<(CellId, SynapseId), SpecError> {
        let first_cell = CellId(self.cells.len() as u32);
        let first_synapse = SynapseId(self.synapses.len() as u32);
        let mut spec = self.spec();
        spec.cells.extend_from_slice(cells);
        spec.synapses.extend_from_slice(synapses);
        spec.validate()?;
        let mut incoming = vec![0u32; spec.cells.len()];
        for s in &self.synapses {
            incoming[s.to.0 as usize] += 1;
        }
        for (k, c) in cells.iter().enumerate() {
            self.cells.push(Cell {
                id: CellId(first_cell.0 + k as u32),
                archetype: c.archetype,
                params: c.params,
                state: c.archetype.initial_state(),
                activation: 0.0,
                last_active: None,
                pattern: c.pattern,
                support: Vec::new(),
            });
        }
        for (k, s) in synapses.iter().enumerate() {
            let slot = incoming[s.to.0 as usize];
            incoming[s.to.0 as usize] += 1;
            self.synapses.push(Synapse {
                id: SynapseId(first_synapse.0 + k as u32),
                from: s.from,
                to: s.to,
                weight: s.weight,
                delay_ticks: s.delay_ticks,
                gate: s.gate,
                plastic: s.plastic,
                slot,
            });
        }
        self.derive();
        Ok((first_cell, first_synapse))
    }

    /// Structural plasticity (A1a): set parameters of existing cells, `(cell, index, value)`, in
    /// order. Validated as one spec with every change applied; on refusal nothing changes. A
    /// parameter that a quantity in time sets (`oscillome.seconds`) is refused: its value is the
    /// conversion's, and a restore would put the conversion back. State, support and activation
    /// are unchanged.
    pub fn set_params(&mut self, changes: &[(CellId, usize, f32)]) -> Result<(), SpecError> {
        let mut spec = self.spec();
        for &(cell, index, value) in changes {
            let c = spec
                .cells
                .get_mut(cell.0 as usize)
                .ok_or(SpecError::Structure("no such cell"))?;
            let p = c
                .params
                .get_mut(index)
                .ok_or(SpecError::Structure("no such parameter"))?;
            *p = value;
            let timed = self.oscillome.seconds.iter().any(|t| {
                t.target
                    == crate::oscillome::TimeTarget::Param {
                        cell,
                        index: index as u8,
                    }
            });
            if timed {
                return Err(SpecError::Structure(
                    "the parameter is set by a quantity in time",
                ));
            }
        }
        spec.validate()?;
        for &(cell, index, value) in changes {
            self.cells[cell.0 as usize].params[index] = value;
        }
        Ok(())
    }

    /// Store the persisted bytes through `port`.
    pub fn persist(&self, port: &mut dyn Persist) {
        port.store(self.last_tick, self.to_bytes());
    }

    /// Restore from the bytes `port` holds.
    pub fn restore(port: &dyn Persist) -> Result<Medium, crate::persist::DecodeError> {
        let bytes = port.load().ok_or(crate::persist::DecodeError::Truncated)?;
        Medium::from_bytes(bytes)
    }

    /// Rebuild a medium from decoded parts; the derived structures are recomputed.
    pub(crate) fn from_parts(
        spec: &MediumSpec,
        cells_dynamic: Vec<CellDynamic>,
        pending: BTreeMap<u64, Vec<Message>>,
        wake_next: Vec<CellId>,
        last_tick: Option<u64>,
        totals: OpCounts,
        summaries: Option<Vec<CycleSummary>>,
    ) -> Result<Medium, SpecError> {
        let mut m = Medium::from_spec(spec)?;
        m.engine.summaries = summaries;
        for (cell, (last_active, state, activation, support)) in
            m.cells.iter_mut().zip(cells_dynamic)
        {
            cell.last_active = last_active;
            cell.state = state;
            cell.activation = activation;
            cell.support = support;
        }
        m.pending = pending;
        m.wake_next = wake_next;
        m.last_tick = last_tick;
        m.totals = totals;
        Ok(m)
    }

    /// The sense cells `event` is delivered to, in id order.
    fn route(&self, event: &Event, out: &mut Vec<CellId>) {
        out.clear();
        let s = event.source;
        for d in [Some(s.domain), None] {
            for n in [Some(s.node), None] {
                for c in [Some(s.channel), None] {
                    if let Some(ids) = self.index.get(&(d, n, c)) {
                        for id in ids {
                            let tag = self.cells[id.0 as usize].pattern.and_then(|p| p.tag);
                            if tag.is_none_or(|t| event.tags.contains(&t)) {
                                out.push(*id);
                            }
                        }
                    }
                }
            }
        }
        out.sort_unstable();
    }

    fn validate_inputs(
        &self,
        tick: u64,
        tick_len_ns: u64,
        field: &Field,
        events: &[Event],
    ) -> Result<(), StepError> {
        let expected = match self.last_tick {
            None => None,
            Some(t) => Some(t.checked_add(1).ok_or(StepError::TickOutOfOrder {
                expected: None,
                got: tick,
            })?),
        };
        if expected.is_some_and(|e| e != tick) {
            return Err(StepError::TickOutOfOrder {
                expected,
                got: tick,
            });
        }
        let spec_len = self.oscillome.tick_len_ns;
        if spec_len != 0 && spec_len != tick_len_ns {
            return Err(StepError::TickLength {
                spec: spec_len,
                clock: tick_len_ns,
            });
        }
        for (index, x) in field.scalars.iter().enumerate() {
            if !x.is_finite() {
                return Err(StepError::NonFiniteField { index });
            }
        }
        let mut seqs = Vec::with_capacity(events.len());
        for e in events {
            if e.tick != tick {
                return Err(StepError::EventTickMismatch {
                    tick,
                    event: e.reference(),
                });
            }
            if !e.value.is_finite() {
                return Err(StepError::NonFiniteEventValue(e.reference()));
            }
            seqs.push(e.seq);
        }
        seqs.sort_unstable();
        if let Some(w) = seqs.windows(2).find(|w| w[0] == w[1]) {
            return Err(StepError::DuplicateSeq { tick, seq: w[0] });
        }
        Ok(())
    }

    fn deliver(&mut self, active: &mut Vec<CellId>, target: CellId, input: Input) {
        let inbox = &mut self.inbox[target.0 as usize];
        if inbox.is_empty() {
            active.push(target);
        }
        inbox.push(input);
    }

    /// Run one tick through `ports`. See the module documentation for the order.
    pub fn step(&mut self, ports: &mut Ports<'_>) -> Result<TickSummary, StepError> {
        // 1. Clock and field. 2. Events, validated before anything changes.
        let tick = ports.clock.now();
        let mut field = ports.field.field(tick);
        let mut events = ports.sense.events(tick);
        let tick_len_ns = if self.oscillome.tick_len_ns == 0 {
            0
        } else {
            ports.clock.tick_len_ns()
        };
        self.validate_inputs(tick, tick_len_ns, &field, &events)?;
        sorted_events(&mut events);
        self.last_tick = Some(tick);
        let traced = self.begin_rhythms(ports, tick, &mut field);

        let mut run = TickRun {
            tick,
            max_ops: self.limits.max_ops_per_tick,
            ops: 0,
            counts: OpCounts::default(),
            truncation: Truncation::default(),
            stopped: false,
            trace: traced.then(Vec::new),
            refs_dropped: 0,
            unanchored: 0,
            carried: 0,
            active_cells: 0,
        };
        let mut active: Vec<CellId> = Vec::new();

        // 2. Routing.
        let mut targets = Vec::new();
        for (order, event) in events.iter().enumerate() {
            let r = event.reference();
            self.route(event, &mut targets);
            if targets.is_empty() {
                if run.allow(1, 0) {
                    run.counts.event_routings += 1;
                    run.trace(|| TraceItem::Unmatched { event: r });
                } else {
                    run.truncation.events_unrouted += 1;
                }
                continue;
            }
            let mut complete = true;
            for &cell in &targets {
                if !run.allow(1, 0) {
                    complete = false;
                    break;
                }
                run.counts.event_routings += 1;
                run.trace(|| TraceItem::Routed { event: r, cell });
                let input = Input {
                    value: event.value,
                    origin: Origin::Event {
                        order: order as u32,
                    },
                    refs: Refs::One(r),
                };
                self.deliver(&mut active, cell, input);
            }
            if !complete {
                run.truncation.events_unrouted += 1;
            }
        }

        // 3. Delivery of messages due now, and wakes.
        if let Some(messages) = self.pending.remove(&tick) {
            for m in messages {
                let syn = &self.synapses[m.synapse.0 as usize];
                let input = Input {
                    value: m.value,
                    origin: Origin::Synapse {
                        sent_tick: m.sent_tick,
                        sent_pass: m.sent_pass,
                        synapse: m.synapse,
                        slot: syn.slot,
                        self_loop: syn.from == syn.to,
                    },
                    refs: Refs::Many(m.refs),
                };
                self.deliver(&mut active, m.target, input);
            }
        }
        for cell in mem::take(&mut self.wake_next) {
            let input = Input {
                value: 0.0,
                origin: Origin::Wake,
                refs: Refs::None,
            };
            self.deliver(&mut active, cell, input);
        }

        // 4 and 5. Passes.
        let mut proposals: Vec<Proposal> = Vec::new();
        let mut pass: u8 = 0;
        while !active.is_empty() {
            pass += 1;
            let mut this_pass = mem::take(&mut active);
            this_pass.sort_unstable();
            this_pass.dedup();
            let fired = self.run_pass(&mut run, &field, pass, &this_pass, &mut proposals);
            self.propagate(&mut run, &field, pass, fired, &mut active);
        }

        // 6. Proposals.
        ports.effector.consume(tick, proposals);

        let truncation = (run.truncation != Truncation::default()).then_some(run.truncation);
        let mut summary = TickSummary {
            tick,
            counts: run.counts,
            truncation,
            passes: pass,
            active_cells: run.active_cells,
            carried: run.carried,
            refs_dropped: run.refs_dropped,
            unanchored: run.unanchored,
            completed: Vec::new(),
        };
        self.totals.accumulate(&run.counts);

        // 7. Cycles, then plasticity.
        self.end_rhythms(ports, &mut summary);

        // 8. Ledger and trace.
        ports
            .ledger
            .record(tick, &summary.counts, summary.truncation.as_ref());
        if let Some(items) = run.trace.take() {
            ports.trace.record(TickTrace { tick, items });
        }
        Ok(summary)
    }

    /// Step 1's oscillome part: write the phases of `tick` into `field` (a pure function of the
    /// tick, section 4b), and say whether the tick is traced (only on the trace rhythm's boundary
    /// ticks when the oscillome names one). Kept out of `step` (not inlined) so that the M1 path
    /// through `step` keeps its shape; see DESIGN.md, the M1b benchmark.
    #[inline(never)]
    fn begin_rhythms(&self, ports: &mut Ports<'_>, tick: u64, field: &mut Field) -> bool {
        field.phases = if self.engine.rhythms() == 0 {
            [0.0; R]
        } else {
            self.engine.phases(tick)
        };
        match self.oscillome.trace_rhythm {
            None => ports.trace.wants(tick),
            Some(r) => self.engine.is_boundary(usize::from(r), tick) && ports.trace.wants(tick),
        }
    }

    /// Step 7: add the tick to every rhythm's cycle, then run plasticity, every tick or at the
    /// boundaries of the plasticity rhythm with that rhythm's completed cycle. Not inlined, as
    /// `begin_rhythms`.
    #[inline(never)]
    fn end_rhythms(&mut self, ports: &mut Ports<'_>, summary: &mut TickSummary) {
        summary.completed = self.engine.record(summary.tick, summary);
        match self.oscillome.plasticity_rhythm {
            None => ports.plasticity.end_of_tick(self, summary),
            Some(r) => {
                if let Some(cycle) = summary.completed.iter().find(|c| c.rhythm == r) {
                    ports.plasticity.end_of_cycle(self, cycle);
                }
            }
        }
    }

    /// Step 4: run every cell of `cells` (ascending) once. Returns the cells whose activation is
    /// not zero and that have outgoing synapses, with the support their messages carry.
    fn run_pass(
        &mut self,
        run: &mut TickRun,
        field: &Field,
        pass: u8,
        cells: &[CellId],
        proposals: &mut Vec<Proposal>,
    ) -> Vec<Fired> {
        let tick = run.tick;
        let max_refs = usize::from(self.limits.max_refs);
        let mut fired = Vec::new();
        for &id in cells {
            let i = id.0 as usize;
            let mut inputs = mem::take(&mut self.inbox[i]);
            if inputs.is_empty() {
                continue;
            }
            let ctx = self.ctx(id, tick);
            let cell = &mut self.cells[i];
            let field_reads = cell.archetype.field_reads(&cell.params);
            if !run.allow(1 + field_reads, pass) {
                run.truncation.cells_not_run += 1;
                run.trace(|| TraceItem::NotRun { cell: id, pass });
                continue;
            }
            run.counts.cell_updates += 1;
            run.counts.field_reads += field_reads;
            if cell.last_active != Some(tick) {
                run.active_cells += 1;
            }
            inputs.sort_by_key(Input::order_key);
            let out = archetype::run(
                cell.archetype,
                &cell.params,
                &mut cell.state,
                &inputs,
                field,
                &ctx,
            );
            cell.activation = out.activation;
            cell.last_active = Some(tick);

            // The support this run cites.
            let mut support: Vec<EventRef> = match out.support {
                SupportRule::Keep => mem::take(&mut cell.support),
                SupportRule::Replace | SupportRule::Merge | SupportRule::ReplaceExternal => {
                    let mut s = if out.support == SupportRule::Merge {
                        mem::take(&mut cell.support)
                    } else {
                        Vec::new()
                    };
                    for input in &inputs {
                        if out.support == SupportRule::ReplaceExternal
                            && matches!(
                                input.origin,
                                Origin::Synapse {
                                    self_loop: true,
                                    ..
                                }
                            )
                        {
                            continue;
                        }
                        s.extend_from_slice(input.refs.as_slice());
                    }
                    s.sort_unstable();
                    s.dedup();
                    if let Some(lookback) = cell.archetype.support_lookback(&cell.params) {
                        s.retain(|r| tick.saturating_sub(r.tick) <= lookback);
                    }
                    if out.activation != 0.0
                        && let Some(sub_us) = cell.archetype.sub_tick_lookback_us(&cell.params)
                    {
                        cut_below_tick(&mut s, tick, self.engine.tick_len_ns(), out.at_us, sub_us);
                    }
                    s
                }
            };
            if support.len() > max_refs {
                run.refs_dropped += (support.len() - max_refs) as u64;
                support.truncate(max_refs);
            }

            run.trace(|| TraceItem::Ran {
                cell: id,
                pass,
                inputs: inputs.len() as u32,
                field_reads,
                activation: out.activation,
            });

            if let Some(emission) = out.emit {
                if let Some(&anchor) = support.first() {
                    if proposals.len() >= self.limits.max_proposals_per_tick as usize {
                        run.truncation.proposal_limit = true;
                        run.truncation.proposals_dropped += 1;
                        run.trace(|| TraceItem::ProposalDropped { cell: id });
                    } else {
                        let (kind, refs, strength) = match emission {
                            Emission::Notice => {
                                let lookback = cell.params[2] as u64;
                                let refs = support
                                    .iter()
                                    .copied()
                                    .filter(|r| tick.saturating_sub(r.tick) <= lookback)
                                    .collect();
                                (cell.params[1] as u16, refs, out.activation)
                            }
                            // A retirement cites the whole support the latch held.
                            Emission::Retire { kind, strength } => {
                                (kind, support.clone(), strength)
                            }
                        };
                        run.counts.proposals += 1;
                        run.trace(|| TraceItem::Proposed { cell: id, anchor });
                        proposals.push(Proposal {
                            kind,
                            anchor,
                            refs,
                            strength,
                            cell: id,
                        });
                    }
                } else {
                    run.unanchored += 1;
                }
            }
            if out.wake && self.wake_next.last() != Some(&id) {
                // Cells run in ascending order within a pass, but a cell may ask in two passes.
                if let Err(pos) = self.wake_next.binary_search(&id) {
                    self.wake_next.insert(pos, id);
                }
            }
            if out.activation != 0.0 && !self.out_edges[i].is_empty() {
                fired.push((id, out.activation, Arc::from(support.as_slice())));
            }
            cell.support = if out.clear_support {
                Vec::new()
            } else {
                support
            };
        }
        fired
    }

    /// What a run of `id` at `tick` knows besides its inputs (before the run changes the cell).
    fn ctx(&self, id: CellId, tick: u64) -> Ctx {
        let cell = &self.cells[id.0 as usize];
        let dt = cell.last_active.map_or(0, |t| tick.saturating_sub(t));
        let mut ctx = Ctx {
            tick,
            dt,
            tick_len_ns: self.engine.tick_len_ns(),
            bins_elapsed: 0,
            period: self.period[id.0 as usize],
        };
        if cell.archetype == Archetype::Coincidence && cell.params[4] == 1.0 {
            let (r, bins) = (cell.params[5] as usize, cell.params[6] as u64);
            ctx.bins_elapsed = cell.last_active.map_or(0, |t| {
                self.engine
                    .bin(r, bins, tick)
                    .saturating_sub(self.engine.bin(r, bins, t))
            });
        }
        ctx
    }

    /// Step 5: traverse the outgoing synapses of the cells that fired in `pass`.
    fn propagate(
        &mut self,
        run: &mut TickRun,
        field: &Field,
        pass: u8,
        fired: Vec<Fired>,
        active: &mut Vec<CellId>,
    ) {
        let tick = run.tick;
        for (id, activation, refs) in fired {
            for k in 0..self.out_edges[id.0 as usize].len() {
                let sid = self.out_edges[id.0 as usize][k];
                let syn = self.synapses[sid.0 as usize];
                let field_read = matches!(syn.gate, Gate::Field(_) | Gate::Phase { .. });
                if !run.allow(1 + u64::from(field_read), pass) {
                    run.truncation.messages_not_sent += 1;
                    continue;
                }
                run.counts.synapse_traversals += 1;
                run.counts.field_reads += u64::from(field_read);
                let open = match syn.gate {
                    Gate::None => true,
                    Gate::Cell(c) => self.cells[c.0 as usize].activation_at(tick) > 0.0,
                    Gate::Field(k) => field.scalars[usize::from(k).min(F - 1)] > 0.0,
                    Gate::Phase { rhythm, from, to } => Gate::phase_window_contains(
                        from,
                        to,
                        field.phases[usize::from(rhythm).min(R - 1)],
                    ),
                };
                run.trace(|| TraceItem::Traversed {
                    synapse: sid,
                    pass,
                    field_read,
                    carried: open,
                });
                if !open {
                    continue;
                }
                let message = Message {
                    target: syn.to,
                    synapse: sid,
                    sent_tick: tick,
                    sent_pass: pass,
                    value: sane(syn.weight * activation),
                    refs: Arc::clone(&refs),
                };
                if syn.delay_ticks == 0 && pass < self.limits.max_passes {
                    let input = Input {
                        value: message.value,
                        origin: Origin::Synapse {
                            sent_tick: tick,
                            sent_pass: pass,
                            synapse: sid,
                            slot: syn.slot,
                            self_loop: syn.from == syn.to,
                        },
                        refs: Refs::Many(message.refs),
                    };
                    self.deliver(active, syn.to, input);
                } else {
                    if syn.delay_ticks == 0 {
                        run.carried += 1;
                    }
                    let due = tick.saturating_add(u64::from(syn.delay_ticks.max(1)));
                    self.pending.entry(due).or_default().push(message);
                }
            }
        }
    }
}
