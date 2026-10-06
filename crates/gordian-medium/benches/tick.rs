//! Cost micro-benchmark of one tick (work items M1 and M1b): measured time per tick at 10, 100
//! and 1,000 active cells, set beside the declared prices of docs/medium-ports.md section 7.
//!
//! M1's five workloads, each in a steady state where every tick does the same work, with every
//! oscillome element off (M1b reruns them at the calibrated prices):
//!
//! - `mixed`: the headline. N/2 sense cells, one event each per tick; each drives its own
//!   integrator (delay 0, fires every tick); each integrator has one outgoing synapse gated by a
//!   field scalar that is zero (traversed and read, never carried). Per tick: N/2 routings, N
//!   cell updates (N active cells), N traversals (N/2 sense to integrator, N/2 gated), N/2 field
//!   reads.
//! - `unmatched`: N events that match no cell. Per tick: N routings, nothing else.
//! - `sense`: N events to N sense cells without synapses. N routings, N updates.
//! - `fanout`: `sense` plus four outgoing synapses per sense cell, gated by a cell that never
//!   runs. N routings, N updates, 4N traversals.
//! - `fieldgate`: `sense` plus four outgoing synapses per sense cell, gated by a zero field
//!   scalar. N routings, N updates, 4N traversals, 4N field reads.
//!
//! Differences between the isolating workloads give per-kind estimates (`analysis` in the
//! report): routing from `unmatched`, update from `sense - unmatched`, traversal from
//! `fanout - sense`, field read from `fieldgate - fanout`.
//!
//! M1b adds five workloads with the oscillome on (100 ms tick, rhythms of 10 s and 100 s, cycle
//! summaries, plasticity at the 10 s boundaries with a plasticity adapter that does nothing):
//!
//! - `mixed_osc`: `mixed` with the integrators' synapses phase-gated (a window that is never open
//!   at these ticks) instead of field-gated. The same counts as `mixed`; the difference is what
//!   the engine and the phase gate cost beyond the model.
//! - `phasegate`: `fieldgate` with phase gates. N routings, N updates, 4N traversals, 4N field
//!   reads.
//! - `oscillator`: N oscillators of period one tick, started by one event each in the first
//!   untimed tick, then running on their own messages: per tick N updates and N self-synapse
//!   traversals, no routing.
//! - `idle` and `idle_osc`: N quiet sense cells and no events, with the oscillome off and on:
//!   no counted operation; the measured time is the tick's fixed cost, and the difference is the
//!   engine's per-tick work (phases and cycle summaries), which section 7 does not price.
//!
//! Each tick's events are built in the untimed setup of `iter_batched`; the timed routine is
//! `Medium::step` with trivial adapters.
//! The per-tick operation counts of each workload are printed to stderr once, as
//! `counts <id> <cell_updates> <synapse_traversals> <event_routings> <field_reads>`.
//!
//! Run through the runner on cores 0-2:
//! `scripts/cgroup-run.sh --name medium-tick-bench --cpus 0-2 --cpu-quota 300 --memory 2G --report <file> -- cargo bench -p gordian-medium --bench tick`

use std::cell::Cell;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use gordian_medium::{
    Address, CellId, ConstantField, CountingLedger, DiscardingEffector, Event, Field, Gate, Medium,
    MediumBuilder, NoPlasticity, NoTrace, Oscillome, Pattern, Ports, Sense, SenseMode, StepClock,
    SynapseSpec,
};

const SIZES: [u32; 3] = [10, 100, 1_000];
const START: u64 = 1;
const TICK_NS: u64 = 100_000_000;

/// The oscillome of the M1b workloads.
fn oscillome() -> Oscillome {
    Oscillome {
        tick_len_ns: TICK_NS,
        periods_ns: vec![10_000_000_000, 100_000_000_000],
        cycle_summary: true,
        plasticity_rhythm: Some(0),
        ..Oscillome::default()
    }
}

/// A phase gate on the 100 s rhythm that is read and never carries: on 100 ms ticks the phases
/// at tick starts are multiples of 0.001, so the window [0.9999, 1) never opens.
const CLOSED_PHASE: Gate = Gate::Phase {
    rhythm: 1,
    from: 0.999_9,
    to: 1.0,
};

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(60)
}

fn pattern(node: u16) -> Pattern {
    Pattern {
        domain: Some(0),
        node: Some(node),
        channel: Some(0),
        tag: None,
    }
}

fn event(node: u16, seq: u32, domain: u16) -> Event {
    Event {
        tick: 0,
        offset_ns: seq.wrapping_mul(7919) % 100_000_000,
        source: Address {
            domain,
            node,
            channel: 0,
        },
        tags: Vec::new(),
        value: 1.0,
        seq,
    }
}

/// A medium, the events every tick of it receives, and, if not empty, the events of the first
/// untimed tick instead.
struct Workload {
    medium: Medium,
    events: Vec<Event>,
    first: Vec<Event>,
}

fn mixed(n: u32) -> Workload {
    mixed_with(n, false)
}

fn mixed_osc(n: u32) -> Workload {
    mixed_with(n, true)
}

fn mixed_with(n: u32, osc: bool) -> Workload {
    let half = n / 2;
    let mut b = if osc {
        MediumBuilder::new().oscillome(oscillome())
    } else {
        MediumBuilder::new()
    };
    let senses: Vec<CellId> = (0..half)
        .map(|i| b.sense(pattern(i as u16), SenseMode::Sum))
        .collect();
    let integ: Vec<CellId> = (0..half).map(|_| b.integrator(0.9, 0.5, true, 4)).collect();
    for i in 0..half as usize {
        b.synapse(senses[i], integ[i], 1.0, 0);
        b.synapse_with(SynapseSpec {
            from: integ[i],
            to: integ[(i + 1) % half as usize],
            weight: 1.0,
            delay_ticks: 1,
            gate: if osc { CLOSED_PHASE } else { Gate::Field(2) },
            plastic: false,
        });
    }
    Workload {
        medium: b.build().unwrap(),
        events: (0..half).map(|i| event(i as u16, i, 0)).collect(),
        first: Vec::new(),
    }
}

fn unmatched(n: u32) -> Workload {
    let mut b = MediumBuilder::new();
    b.sense(pattern(0), SenseMode::Presence);
    Workload {
        medium: b.build().unwrap(),
        // Domain 1: no cell listens there.
        events: (0..n).map(|i| event(i as u16, i, 1)).collect(),
        first: Vec::new(),
    }
}

fn sense_with(n: u32, fan: u32, gate: Option<Gate>) -> Workload {
    let mut b = if matches!(gate, Some(Gate::Phase { .. })) {
        MediumBuilder::new().oscillome(oscillome())
    } else {
        MediumBuilder::new()
    };
    let senses: Vec<CellId> = (0..n)
        .map(|i| b.sense(pattern(i as u16), SenseMode::Presence))
        .collect();
    if let Some(gate) = gate {
        // Targets that never receive anything (every synapse into them is closed).
        let sinks: Vec<CellId> = (0..fan).map(|_| b.latch(0.5, 0)).collect();
        let gate = match gate {
            Gate::Cell(_) => Gate::Cell(sinks[0]),
            g => g,
        };
        for &s in &senses {
            for &t in &sinks {
                b.synapse_with(SynapseSpec {
                    from: s,
                    to: t,
                    weight: 1.0,
                    delay_ticks: 0,
                    gate,
                    plastic: false,
                });
            }
        }
    }
    Workload {
        medium: b.build().unwrap(),
        events: (0..n).map(|i| event(i as u16, i, 0)).collect(),
        first: Vec::new(),
    }
}

fn oscillators(n: u32) -> Workload {
    let mut b = MediumBuilder::new().oscillome(oscillome());
    for i in 0..n {
        let s = b.sense(pattern(i as u16), SenseMode::Presence);
        // Decay 0.999999 per cycle with a floor of 1e-30: it stops, but not within the
        // benchmark's few million cycles.
        let (osc, _) = b.oscillator(1.0, 1.0e-30, 0, 0.999_999, 1);
        b.synapse(s, osc, 1.0, 0);
    }
    Workload {
        medium: b.build().unwrap(),
        events: Vec::new(),
        first: (0..n).map(|i| event(i as u16, i, 0)).collect(),
    }
}

fn idle_with(n: u32, osc: bool) -> Workload {
    let mut b = if osc {
        MediumBuilder::new().oscillome(oscillome())
    } else {
        MediumBuilder::new()
    };
    for i in 0..n {
        b.sense(pattern(i as u16), SenseMode::Presence);
    }
    Workload {
        medium: b.build().unwrap(),
        events: Vec::new(),
        first: Vec::new(),
    }
}

/// Hands the routine the events its setup prepared.
struct Handoff(Vec<Event>);

impl Sense for Handoff {
    fn events(&mut self, _tick: u64) -> Vec<Event> {
        std::mem::take(&mut self.0)
    }
}

fn bench_workload(c: &mut Criterion, name: &str, n: u32, build: fn(u32) -> Workload) {
    let Workload {
        mut medium,
        events,
        first,
    } = build(n);
    let field = ConstantField(Field::default());
    let mut clock = StepClock::new(START, TICK_NS);
    let mut effector = DiscardingEffector::default();
    let mut ledger = CountingLedger::default();
    let mut trace = NoTrace;
    let mut plasticity = NoPlasticity;

    // Two untimed ticks reach the steady state; print the second one's counts.
    for k in 0..2 {
        let script = if k == 0 && !first.is_empty() {
            &first
        } else {
            &events
        };
        let mut sense = Handoff(
            script
                .iter()
                .cloned()
                .map(|mut e| {
                    e.tick = clock.tick;
                    e
                })
                .collect(),
        );
        let mut f = field;
        let mut ports = Ports {
            clock: &mut clock,
            sense: &mut sense,
            field: &mut f,
            effector: &mut effector,
            ledger: &mut ledger,
            trace: &mut trace,
            plasticity: &mut plasticity,
        };
        let s = medium.step(&mut ports).unwrap();
        clock.advance();
        if s.tick == START + 1 {
            let k = s.counts;
            eprintln!(
                "counts {name}/{n} {} {} {} {} active {}",
                k.cell_updates,
                k.synapse_traversals,
                k.event_routings,
                k.field_reads,
                s.active_cells
            );
            assert!(s.truncation.is_none());
        }
    }

    let next_setup = Cell::new(clock.tick);
    let clock = Cell::new(clock);
    c.benchmark_group(name)
        .bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_batched(
                || {
                    let tick = next_setup.get();
                    next_setup.set(tick + 1);
                    events
                        .iter()
                        .cloned()
                        .map(|mut e| {
                            e.tick = tick;
                            e
                        })
                        .collect::<Vec<_>>()
                },
                |events| {
                    let mut ck = clock.get();
                    let mut sense = Handoff(events);
                    let mut f = field;
                    let mut ports = Ports {
                        clock: &mut ck,
                        sense: &mut sense,
                        field: &mut f,
                        effector: &mut effector,
                        ledger: &mut ledger,
                        trace: &mut trace,
                        plasticity: &mut plasticity,
                    };
                    let s = medium.step(&mut ports);
                    ck.advance();
                    clock.set(ck);
                    black_box(s.unwrap())
                },
                BatchSize::SmallInput,
            )
        });
}

fn ticks(c: &mut Criterion) {
    for n in SIZES {
        bench_workload(c, "mixed", n, mixed);
        bench_workload(c, "unmatched", n, unmatched);
        bench_workload(c, "sense", n, |n| sense_with(n, 0, None));
        bench_workload(c, "fanout", n, |n| {
            sense_with(n, 4, Some(Gate::Cell(CellId(0))))
        });
        bench_workload(c, "fieldgate", n, |n| {
            sense_with(n, 4, Some(Gate::Field(2)))
        });
        bench_workload(c, "mixed_osc", n, mixed_osc);
        bench_workload(c, "phasegate", n, |n| sense_with(n, 4, Some(CLOSED_PHASE)));
        bench_workload(c, "oscillator", n, oscillators);
        bench_workload(c, "idle", n, |n| idle_with(n, false));
        bench_workload(c, "idle_osc", n, |n| idle_with(n, true));
    }
}

criterion_group! {
    name = benches;
    config = config();
    targets = ticks
}
criterion_main!(benches);
