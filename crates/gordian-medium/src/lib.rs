//! The medium: a sparse, persistent collection of cells joined by synapses, bathed in a field of
//! a few broadcast scalars, advancing in ticks (work item M1; design in `docs/medium-ports.md`,
//! departures in `DESIGN.md` next to this crate's manifest).
//!
//! # What is built
//!
//! - The types of section 3 ([`types`]), the seven archetypes of section 5 ([`archetype`]), the
//!   tick of section 4 with its total order ([`medium`]), the nine port traits of section 6 with
//!   trivial adapters ([`ports`]), operation counting with hard limits and [`Truncation`]
//!   records, declared prices (section 7), [`MediumSpec`] and [`MediumBuilder`] ([`spec`]), and
//!   deterministic persisted bytes ([`persist`]).
//! - The oscillome of section 4b (work item M1b, [`oscillome`]): slower rhythms whose phases are
//!   broadcast in the field, phase gates, coincidence binned by a rhythm or ordered by event
//!   time, the `Oscillator` archetype, latch retirement, quantities given in time and converted
//!   at build, per-cycle summaries, and plasticity and trace sampling at rhythm boundaries. Every
//!   element is switchable off, and with all off the medium is M1's, byte for byte.
//!
//! # What is not built
//!
//! No world adapter, no noticer, no learning (M2 and later). Nothing here claims that the
//! archetypes notice or anchor anything in a world; that is what M2's experiment is for.
//!
//! # Determinism
//!
//! Cells and synapses are stored and iterated in id order; the only maps are `BTreeMap`s; there
//! is no I/O, no clock and no randomness. Scalars are `f32`, computed with `+ - * /`, comparison,
//! `min`, `max` and `abs` only. Same spec and same events give the same persisted bytes, on one
//! CPU with one set of flags.

#![forbid(unsafe_code)]

pub mod archetype;
pub mod medium;
pub mod oscillome;
pub mod persist;
pub mod ports;
pub mod spec;
pub mod types;

pub use archetype::{Archetype, ParamError};
pub use medium::{
    Cell, Medium, StepError, Synapse, TickSummary, TickTrace, TraceItem, Truncation, WeightError,
};
pub use oscillome::{
    Conversion, CycleSummary, Oscillome, OscillomeEngine, TimeKind, TimeTarget, Timed, secs,
};
pub use persist::DecodeError;
pub use ports::{
    Clock, CollectingEffector, ConstantField, CountingLedger, DiscardingEffector, Effector,
    FieldSource, InMemoryPersist, Ledger, NoPlasticity, NoResource, NoTrace, Persist, Plasticity,
    Ports, Resource, ResourceCall, ResourceReply, SamplingTrace, ScriptedSense, Sense, StepClock,
    Trace,
};
pub use spec::{CellSpec, MediumBuilder, MediumSpec, SenseMode, SpecError, SynapseSpec};
pub use types::{
    Address, CellId, Event, EventRef, F, Field, Gate, Limits, OpCounts, P, Pattern, Prices,
    Proposal, R, S, SynapseId, Tag,
};
