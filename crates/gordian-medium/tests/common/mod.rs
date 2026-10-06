//! Shared test rig: the trivial adapters wired into `Ports`, event helpers, a deterministic
//! test-only generator and a spec that uses every archetype and every gate kind.

#![allow(dead_code)]

use gordian_medium::{
    Address, CollectingEffector, ConstantField, CountingLedger, Event, Field, Gate, Limits, Medium,
    MediumBuilder, MediumSpec, NoPlasticity, Pattern, Ports, SamplingTrace, ScriptedSense,
    SenseMode, StepClock, StepError, SynapseSpec, Tag, TickSummary,
};

/// The adapters one test drives a medium with.
pub struct Rig {
    pub clock: StepClock,
    pub sense: ScriptedSense,
    pub field: ConstantField,
    pub effector: CollectingEffector,
    pub ledger: CountingLedger,
    pub trace: SamplingTrace,
    pub plasticity: NoPlasticity,
}

impl Rig {
    /// Start at `start`, with `events` scripted, a zero field, and every tick traced.
    pub fn new(start: u64, events: Vec<Event>) -> Self {
        Rig {
            clock: StepClock::new(start, 100_000_000),
            sense: ScriptedSense::from_events(events),
            field: ConstantField(Field::default()),
            effector: CollectingEffector::default(),
            ledger: CountingLedger::default(),
            trace: SamplingTrace::new(1, usize::MAX),
            plasticity: NoPlasticity,
        }
    }

    /// Run one tick and, if it was accepted, advance the clock.
    pub fn step(&mut self, medium: &mut Medium) -> Result<TickSummary, StepError> {
        let mut ports = Ports {
            clock: &mut self.clock,
            sense: &mut self.sense,
            field: &mut self.field,
            effector: &mut self.effector,
            ledger: &mut self.ledger,
            trace: &mut self.trace,
            plasticity: &mut self.plasticity,
        };
        let result = medium.step(&mut ports);
        if result.is_ok() {
            self.clock.advance();
        }
        result
    }
}

/// An event at `(tick, offset)` from node `node`, channel `channel` of domain 0.
pub fn ev(tick: u64, offset_ns: u32, node: u16, channel: u16, value: f32, seq: u32) -> Event {
    Event {
        tick,
        offset_ns,
        source: Address {
            domain: 0,
            node,
            channel,
        },
        tags: Vec::new(),
        value,
        seq,
    }
}

/// A pattern for one (node, channel).
pub fn at(node: u16, channel: u16) -> Pattern {
    Pattern {
        domain: Some(0),
        node: Some(node),
        channel: Some(channel),
        tag: None,
    }
}

/// splitmix64, for test inputs only (the crate itself has no randomness).
pub struct TestGen(pub u64);

impl TestGen {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

pub const NODES: u16 = 4;
pub const CHANNELS: u16 = 2;
pub const ABNORMAL: Tag = Tag(1);

/// A spec that uses every archetype, every gate kind, zero and non-zero delays and a feedback
/// loop: per node, sense cells per channel and one for abnormal-tagged events, an integrator, a
/// novelty cell (gaps as zeros), a latch, a field-gated relay, then a coincidence over
/// neighbouring nodes and an emitter per node.
pub fn rich_spec(limits: Limits) -> MediumSpec {
    let mut b = MediumBuilder::new().limits(limits);
    let mut emitters = Vec::new();
    let mut integrators = Vec::new();
    for node in 0..NODES {
        let senses: Vec<_> = (0..CHANNELS)
            .map(|ch| {
                b.sense(
                    at(node, ch),
                    if ch == 0 {
                        SenseMode::Sum
                    } else {
                        SenseMode::Count
                    },
                )
            })
            .collect();
        let abnormal = b.sense(
            Pattern {
                node: Some(node),
                tag: Some(ABNORMAL),
                ..Pattern::default()
            },
            SenseMode::Presence,
        );
        let integ = b.integrator(0.8, 2.5, true, 20);
        let nov = b.novelty(0.2, 3.0, 0.5, 3, true);
        let latch = b.latch(1.0, 3);
        let gate = b.gate(0, 0.5, true);
        let emit = b.emit(1.0, 1, 10, 2);
        for &s in &senses {
            b.synapse(s, integ, 1.0, 0);
            b.synapse(s, nov, 1.0, 1);
        }
        b.synapse(abnormal, integ, 2.0, 0);
        b.synapse(integ, latch, 1.0, 0);
        b.synapse(nov, gate, 1.0, 0);
        b.synapse(gate, emit, 1.0, 1);
        b.synapse_with(SynapseSpec {
            from: latch,
            to: emit,
            weight: 1.0,
            delay_ticks: 0,
            gate: Gate::Field(1),
            plastic: true,
        });
        // Feedback: the latch re-excites the integrator a tick later, while the novelty cell is
        // active in that tick.
        b.synapse_with(SynapseSpec {
            from: latch,
            to: integ,
            weight: 0.5,
            delay_ticks: 1,
            gate: Gate::Cell(nov),
            plastic: false,
        });
        integrators.push(integ);
        emitters.push(emit);
    }
    let coinc_emit = b.emit(2.0, 2, 30, 0);
    for node in 0..NODES {
        let c = b.coincidence(2, 3, true, 30);
        let left = integrators[node as usize];
        let right = integrators[((node + 1) % NODES) as usize];
        b.synapse(left, c, 1.0, 0);
        b.synapse(right, c, 1.0, 2);
        b.synapse(c, coinc_emit, 1.0, 0);
    }
    b.into_spec()
}

/// A field that opens the gates and the field-gated synapses.
pub fn open_field() -> Field {
    Field {
        scalars: [1.0, 1.0, 0.0, 0.0],
    }
}

/// `ticks` ticks of random events for the rich spec, from `seed`.
pub fn random_events(seed: u64, start: u64, ticks: u64, max_per_tick: u64) -> Vec<Event> {
    let mut g = TestGen(seed);
    let mut out = Vec::new();
    for t in start..start + ticks {
        let n = g.below(max_per_tick + 1);
        for seq in 0..n {
            let mut e = ev(
                t,
                g.below(100_000_000) as u32,
                g.below(u64::from(NODES) + 1) as u16, // one node past the graph: unmatched
                g.below(u64::from(CHANNELS)) as u16,
                // Values whose f32 sums depend on the order of addition, so that a test of
                // order independence can see a missing sort (a mutation check found that
                // multiples of 0.5 could not).
                if g.below(10) == 0 {
                    1.0e7
                } else {
                    (g.below(2000) as f32) * 0.0137
                },
                seq as u32,
            );
            if g.below(3) == 0 {
                e.tags.push(ABNORMAL);
            }
            out.push(e);
        }
    }
    out
}
