//! The nine ports (docs/medium-ports.md, section 6) and their trivial adapters.
//!
//! Ports are traits here; world adapters live outside the crate. Anchoring and routing are not
//! ports: they are what the medium does, and a port for either would let an adapter supply the
//! answer. The adapters in this module know no world: they script, collect, count or discard.
//!
//! Departure from section 6: the first trace adapter there is "file, sampled". The crate does no
//! I/O, so [`SamplingTrace`] keeps its samples in memory, capped; writing them to a file belongs
//! to the harness.

use std::collections::BTreeMap;

use gordian_core::Instant;

use crate::medium::{Medium, TickSummary, TickTrace, Truncation};
use crate::oscillome::CycleSummary;
use crate::types::{Event, EventRef, Field, OpCounts, Proposal};

/// Names the tick and knows its length (in).
pub trait Clock {
    /// The tick to run now. The medium refuses a tick that is not the one after the last.
    fn now(&self) -> u64;
    /// The tick length, in nanoseconds of the world's logical clock. At least 1.
    fn tick_len_ns(&self) -> u64;
    /// The tick that contains `at`.
    fn tick_of(&self, at: Instant) -> u64 {
        at.0 / self.tick_len_ns().max(1)
    }
    /// `at`'s position inside its tick, in nanoseconds, saturated at `u32::MAX` (a tick longer
    /// than about 4.29 s loses resolution at its end; see DESIGN.md).
    fn offset_in_tick(&self, at: Instant) -> u32 {
        u32::try_from(at.0 % self.tick_len_ns().max(1)).unwrap_or(u32::MAX)
    }
}

/// This tick's events (in).
pub trait Sense {
    /// Every event of `tick`, in any order. Each must have `tick` as its tick, a finite value and
    /// a `seq` unique within the tick.
    fn events(&mut self, tick: u64) -> Vec<Event>;
}

/// This tick's field (in).
pub trait FieldSource {
    /// The field for `tick`. Its scalars must be finite.
    fn field(&mut self, tick: u64) -> Field;
}

/// Consumes proposals (out).
pub trait Effector {
    /// The proposals of `tick`, in the order the emitters produced them (pass, then cell id).
    fn consume(&mut self, tick: u64, proposals: Vec<Proposal>);
}

/// A typed call to an expensive resource, with its declared cost. Not used by M1 or M2: no
/// archetype issues one, and the answer would return as an [`Event`] through [`Sense`].
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceCall {
    /// What is asked; its meaning belongs to the adapter.
    pub kind: u16,
    /// The events the call cites.
    pub refs: Vec<EventRef>,
    /// Declared cost, in modelled picoseconds.
    pub declared_cost_ps: u64,
}

/// The adapter's answer to a [`ResourceCall`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceReply {
    /// Accepted; the answer arrives later as an event that carries `ticket` in its tags or value,
    /// as the adapter declares.
    Accepted {
        /// Identifies the call.
        ticket: u32,
    },
    /// Refused, for example over budget.
    Refused,
}

/// Calls out to an expensive resource (out, then in).
pub trait Resource {
    /// Place a call at `tick`.
    fn call(&mut self, tick: u64, call: &ResourceCall) -> ResourceReply;
}

/// Receives the operation counts of every tick (out).
pub trait Ledger {
    /// The counts of `tick` and, if the tick hit a limit, its truncation record.
    fn record(&mut self, tick: u64, counts: &OpCounts, truncation: Option<&Truncation>);
}

/// Stores and returns the medium's persisted bytes (both).
pub trait Persist {
    /// Keep `bytes`, the persisted medium after `tick`.
    fn store(&mut self, tick: Option<u64>, bytes: Vec<u8>);
    /// The most recently stored bytes.
    fn load(&self) -> Option<&[u8]>;
}

/// Receives sampled per-tick activity (out).
pub trait Trace {
    /// Whether `tick` is sampled. Asked before the tick runs, so an unsampled tick builds no
    /// trace.
    fn wants(&mut self, tick: u64) -> bool;
    /// The trace of a sampled tick.
    fn record(&mut self, trace: TickTrace);
}

/// Runs once at the end of every tick with the tick's summary, or, when the oscillome names a
/// plasticity rhythm, once at the end of each of that rhythm's boundary ticks with the summary of
/// the cycle just completed (in).
pub trait Plasticity {
    /// May change the weights of plastic synapses through [`Medium::set_weight`].
    fn end_of_tick(&mut self, medium: &mut Medium, summary: &TickSummary);
    /// The same, at a cycle boundary of the plasticity rhythm (M1b decision 6). Does nothing
    /// unless the adapter says otherwise.
    fn end_of_cycle(&mut self, _medium: &mut Medium, _cycle: &CycleSummary) {}
}

/// The ports one tick uses. The resource and persistence ports are not called during a tick.
pub struct Ports<'a> {
    /// Names the tick.
    pub clock: &'a mut dyn Clock,
    /// Supplies the events.
    pub sense: &'a mut dyn Sense,
    /// Supplies the field.
    pub field: &'a mut dyn FieldSource,
    /// Consumes proposals.
    pub effector: &'a mut dyn Effector,
    /// Receives counts.
    pub ledger: &'a mut dyn Ledger,
    /// Receives sampled traces.
    pub trace: &'a mut dyn Trace,
    /// Runs at tick end.
    pub plasticity: &'a mut dyn Plasticity,
}

// ---------------------------------------------------------------------------------------------
// Trivial adapters.

/// A clock the driver advances one tick at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepClock {
    /// The tick [`Clock::now`] returns.
    pub tick: u64,
    /// The tick length in nanoseconds.
    pub tick_len_ns: u64,
}

impl StepClock {
    /// A clock at `start`.
    pub fn new(start: u64, tick_len_ns: u64) -> Self {
        Self {
            tick: start,
            tick_len_ns,
        }
    }

    /// Move to the next tick.
    pub fn advance(&mut self) {
        self.tick = self.tick.saturating_add(1);
    }
}

impl Clock for StepClock {
    fn now(&self) -> u64 {
        self.tick
    }
    fn tick_len_ns(&self) -> u64 {
        self.tick_len_ns
    }
}

/// Events scripted per tick; each tick's events are handed out once.
#[derive(Debug, Clone, Default)]
pub struct ScriptedSense {
    /// The script.
    pub by_tick: BTreeMap<u64, Vec<Event>>,
}

impl ScriptedSense {
    /// From a list of events, grouped by their tick, keeping their order within a tick.
    pub fn from_events(events: impl IntoIterator<Item = Event>) -> Self {
        let mut by_tick: BTreeMap<u64, Vec<Event>> = BTreeMap::new();
        for e in events {
            by_tick.entry(e.tick).or_default().push(e);
        }
        Self { by_tick }
    }
}

impl Sense for ScriptedSense {
    fn events(&mut self, tick: u64) -> Vec<Event> {
        self.by_tick.remove(&tick).unwrap_or_default()
    }
}

/// The same field every tick.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ConstantField(pub Field);

impl FieldSource for ConstantField {
    fn field(&mut self, _tick: u64) -> Field {
        self.0
    }
}

/// Keeps every proposal with its tick.
#[derive(Debug, Clone, Default)]
pub struct CollectingEffector {
    /// Proposals in arrival order.
    pub proposals: Vec<(u64, Proposal)>,
}

impl Effector for CollectingEffector {
    fn consume(&mut self, tick: u64, proposals: Vec<Proposal>) {
        self.proposals
            .extend(proposals.into_iter().map(|p| (tick, p)));
    }
}

/// Counts proposals and drops them.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscardingEffector {
    /// Proposals seen.
    pub seen: u64,
}

impl Effector for DiscardingEffector {
    fn consume(&mut self, _tick: u64, proposals: Vec<Proposal>) {
        self.seen += proposals.len() as u64;
    }
}

/// Refuses every call.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoResource;

impl Resource for NoResource {
    fn call(&mut self, _tick: u64, _call: &ResourceCall) -> ResourceReply {
        ResourceReply::Refused
    }
}

/// Sums the counts and keeps every truncation record.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CountingLedger {
    /// Ticks recorded.
    pub ticks: u64,
    /// Counts summed over the ticks.
    pub totals: OpCounts,
    /// Every truncation, with its tick.
    pub truncations: Vec<(u64, Truncation)>,
}

impl Ledger for CountingLedger {
    fn record(&mut self, tick: u64, counts: &OpCounts, truncation: Option<&Truncation>) {
        self.ticks += 1;
        self.totals.accumulate(counts);
        if let Some(t) = truncation {
            self.truncations.push((tick, *t));
        }
    }
}

/// Keeps the latest persisted bytes in memory.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPersist {
    /// The tick after which the bytes were stored.
    pub tick: Option<u64>,
    /// The bytes.
    pub bytes: Option<Vec<u8>>,
}

impl Persist for InMemoryPersist {
    fn store(&mut self, tick: Option<u64>, bytes: Vec<u8>) {
        self.tick = tick;
        self.bytes = Some(bytes);
    }
    fn load(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Samples every `every`-th tick (those with `tick % every == phase`), keeping at most `cap`
/// traces; later samples are counted in `overflow`, not kept.
#[derive(Debug, Clone, Default)]
pub struct SamplingTrace {
    /// Sampling period in ticks; 0 samples nothing.
    pub every: u64,
    /// Which residue is sampled.
    pub phase: u64,
    /// Most traces kept.
    pub cap: usize,
    /// The kept traces.
    pub traces: Vec<TickTrace>,
    /// Sampled ticks not kept because the cap was reached.
    pub overflow: u64,
}

impl SamplingTrace {
    /// Sample every `every`-th tick, keeping at most `cap`.
    pub fn new(every: u64, cap: usize) -> Self {
        Self {
            every,
            cap,
            ..Self::default()
        }
    }
}

impl Trace for SamplingTrace {
    fn wants(&mut self, tick: u64) -> bool {
        self.every > 0 && tick % self.every == self.phase % self.every
    }
    fn record(&mut self, trace: TickTrace) {
        if self.traces.len() < self.cap {
            self.traces.push(trace);
        } else {
            self.overflow += 1;
        }
    }
}

/// Samples nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTrace;

impl Trace for NoTrace {
    fn wants(&mut self, _tick: u64) -> bool {
        false
    }
    fn record(&mut self, _trace: TickTrace) {}
}

/// Changes nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPlasticity;

impl Plasticity for NoPlasticity {
    fn end_of_tick(&mut self, _medium: &mut Medium, _summary: &TickSummary) {}
}
