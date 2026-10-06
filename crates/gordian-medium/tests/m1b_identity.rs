//! The all-off identity with M1b (work item M3): with sub-tick support pruning off (every cell's
//! sub-tick lookback 0, the default), the medium produces the bytes M1b produced for the same spec
//! and events, oscillome elements included.
//!
//! The digests below were produced by M1b's own code: this file was committed first on the
//! `subtick-support` branch, on top of the merged M1b and M2 crates and before any M3 change, run
//! there, and its digests written in (commit "Pin M1b's bytes as digests before sub-tick pruning
//! changes anything"). Checking out that commit and running this test reproduces them. The
//! hashing is `tests/m1_identity.rs`'s (FNV-1a 64 over, for every tick of every case: the
//! persisted bytes after the tick, the tick's summary and truncation record, every proposal, and
//! the debug text of every trace item; per case, the ledger's totals), extended with the cycle
//! summaries a tick completes and the plasticity port's record.
//!
//! The cases use every oscillome element at each tick length of the sweep (100 ms, 500 ms, 2 s):
//! the oscillome spec of `tests/common` under dense and sparse events and tight limits, and 300
//! generated specs mixing M1's forms with ordered and binned coincidences, retiring latches,
//! oscillators, phase gates and quantities in time. M1's own identity is `tests/m1_identity.rs`,
//! unchanged.

#![allow(clippy::field_reassign_with_default)]

mod common;

use common::{Rig, TestGen, open_field, oscillome_spec, random_events_in};
use gordian_medium::{
    Address, Archetype, CellId, CellSpec, Event, Field, Gate, Limits, Medium, MediumSpec,
    Oscillome, P, Pattern, SynapseSpec, Tag, TickSummary, TimeTarget, Timed,
};

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.0 ^= u64::from(*x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
}

fn hash_summary(h: &mut Fnv, s: &TickSummary) {
    let c = s.counts;
    for v in [
        s.tick,
        c.cell_updates,
        c.synapse_traversals,
        c.event_routings,
        c.field_reads,
        c.proposals,
        u64::from(s.passes),
        s.active_cells,
        s.carried,
        s.refs_dropped,
        s.unanchored,
    ] {
        h.u64(v);
    }
    match s.truncation {
        None => h.u64(0),
        Some(t) => {
            h.u64(1);
            for v in [
                u64::from(t.op_limit),
                u64::from(t.proposal_limit),
                u64::from(t.pass),
                t.events_unrouted,
                t.cells_not_run,
                t.messages_not_sent,
                t.proposals_dropped,
            ] {
                h.u64(v);
            }
        }
    }
    h.bytes(format!("{:?}", s.completed).as_bytes());
}

/// Run `ticks` ticks of `spec` on `events` from `start` on a clock of `tick_len_ns` with `field`,
/// hashing everything. Returns (built, proposals, truncated ticks).
fn digest_run(
    h: &mut Fnv,
    spec: &MediumSpec,
    events: Vec<Event>,
    (start, ticks, tick_len_ns): (u64, u64, u64),
    field: Field,
) -> (u64, u64, u64) {
    let Ok(mut m) = Medium::from_spec(spec) else {
        h.u64(0xdead);
        return (0, 0, 0);
    };
    h.bytes(&m.to_bytes());
    let mut rig = Rig::with_tick(start, tick_len_ns, events);
    rig.field.0 = field;
    rig.plasticity.scale = Some(0.9);
    let mut seen_props = 0;
    let mut seen_traces = 0;
    for _ in 0..ticks {
        let Ok(s) = rig.step(&mut m) else {
            h.u64(0xbad);
            break;
        };
        hash_summary(h, &s);
        h.bytes(&m.to_bytes());
        for (t, p) in &rig.effector.proposals[seen_props..] {
            h.u64(*t);
            h.u64(u64::from(p.kind));
            for r in std::iter::once(&p.anchor).chain(p.refs.iter()) {
                h.u64(r.tick);
                h.u64(u64::from(r.offset_ns));
                h.u64(u64::from(r.seq));
            }
            h.u64(u64::from(p.strength.to_bits()));
            h.u64(u64::from(p.cell.0));
        }
        seen_props = rig.effector.proposals.len();
        for trace in &rig.trace.traces[seen_traces..] {
            h.u64(trace.tick);
            for item in &trace.items {
                h.bytes(format!("{item:?}").as_bytes());
            }
        }
        seen_traces = rig.trace.traces.len();
    }
    let t = rig.ledger.totals;
    for v in [
        rig.ledger.ticks,
        t.cell_updates,
        t.synapse_traversals,
        t.event_routings,
        t.field_reads,
        t.proposals,
        rig.ledger.truncations.len() as u64,
    ] {
        h.u64(v);
    }
    h.bytes(format!("{:?}{:?}", rig.plasticity.ticks, rig.plasticity.cycles).as_bytes());
    (
        1,
        rig.effector.proposals.len() as u64,
        rig.ledger.truncations.len() as u64,
    )
}

fn add(a: &mut (u64, u64, u64), b: (u64, u64, u64)) {
    a.0 += b.0;
    a.1 += b.1;
    a.2 += b.2;
}

const TICKS_NS: [u64; 3] = [100_000_000, 500_000_000, 2_000_000_000];

/// The oscillome spec at each tick length: sparse and dense events over 250 ticks (25 s to
/// 500 s, crossing 10 s boundaries at every length and 100 s ones at the longer two), then tight
/// limits.
fn oscillome_digest() -> (u64, (u64, u64, u64)) {
    let mut h = Fnv::new();
    let mut seen = (0, 0, 0);
    for tick in TICKS_NS {
        let spec = oscillome_spec(Limits::default(), tick);
        for (seed, per_tick) in [(1u64, 6u64), (2, 6), (11, 40)] {
            let events = random_events_in(seed, 0, 250, per_tick, tick);
            add(
                &mut seen,
                digest_run(&mut h, &spec, events, (0, 250, tick), open_field()),
            );
        }
        for seed in 0..4u64 {
            let limits = Limits {
                max_ops_per_tick: 2 + seed * 5,
                max_proposals_per_tick: (seed % 3) as u32,
                ..Limits::default()
            };
            let spec = oscillome_spec(limits, tick);
            let events = random_events_in(seed, 0, 120, 10, tick);
            add(
                &mut seen,
                digest_run(&mut h, &spec, events, (0, 120, tick), open_field()),
            );
        }
    }
    (h.0, seen)
}

/// Parameters for a generated cell of `archetype`; `form` picks an oscillome form where the
/// archetype has one (coincidence: 0 sliding, 1 binned, 2 ordered; latch: retiring or not).
fn params_for(archetype: Archetype, form: u64, g: &mut TestGen) -> [f32; P] {
    let mut raw = [0u8; P];
    for r in raw.iter_mut() {
        *r = g.below(256) as u8;
    }
    let f = |i: usize| f32::from(raw[i]);
    let mut p = [0.0f32; P];
    match archetype {
        Archetype::Sense => p[0] = f32::from(raw[0] % 3),
        Archetype::Integrator => {
            p[0] = f(0) / 255.0;
            p[1] = f(1) / 16.0 - 2.0;
            p[2] = f32::from(raw[2] % 2);
            p[3] = f32::from(raw[3] % 20);
        }
        Archetype::Novelty => {
            p[0] = (f(0) + 1.0) / 256.0;
            p[1] = f(1) / 32.0;
            p[2] = f(2) / 64.0;
            p[3] = f32::from(raw[3] % 5);
            p[4] = f32::from(raw[4] % 2);
        }
        Archetype::Coincidence => match form % 3 {
            0 => {
                p[0] = f32::from(1 + raw[0] % 8);
                p[1] = f32::from(raw[1] % 6);
                p[2] = f32::from(raw[2] % 2);
                p[3] = f32::from(raw[3] % 20);
            }
            1 => {
                p[0] = f32::from(1 + raw[0] % 8);
                p[1] = f32::from(raw[1] % 3);
                p[2] = f32::from(raw[2] % 2);
                p[3] = f32::from(raw[3] % 20);
                p[4] = 1.0;
                p[5] = f32::from(raw[5] % 2);
                p[6] = f32::from(1 + raw[6] % 5);
            }
            _ => {
                p[0] = f32::from(1 + raw[0] % 4);
                // Windows from 0 to 1.5 s in microseconds, so that they cross tick edges.
                p[1] = f32::from(raw[1]) * 6_000.0;
                p[2] = f32::from(raw[2] % 2);
                p[3] = f32::from(raw[3] % 4);
                p[4] = 2.0;
                p[5] = f32::from(raw[5] % 2);
            }
        },
        Archetype::Gate => {
            p[0] = f32::from(raw[0] % 4);
            p[1] = f(1) / 64.0 - 1.0;
            p[2] = f32::from(raw[2] % 2);
        }
        Archetype::Latch => {
            p[0] = f(0) / 32.0 - 1.0;
            p[1] = f32::from(raw[1] % 6);
            if form % 2 == 1 {
                p[2] = 1.0;
                p[3] = f32::from(raw[3]);
            }
        }
        Archetype::Emit => {
            p[0] = f(0) / 32.0 - 1.0;
            p[1] = f(1);
            p[2] = f32::from(raw[2] % 20);
            p[3] = f32::from(raw[3] % 4);
        }
        Archetype::Oscillator => {
            p[0] = 0.5 + f(0) / 64.0;
            p[1] = f(1) / 256.0;
            p[2] = f32::from(1 + raw[2] % 4);
        }
    }
    p
}

const ARCHETYPES: [Archetype; 8] = Archetype::ALL;

/// A generated M1b spec on a tick of `tick_len_ns`: 2 to 14 cells of any archetype and form,
/// rhythms of 10 s and 100 s with cycle summaries and plasticity on the 10 s rhythm, up to 39
/// synapses with every gate kind (phase gates included) and delays 0 to 2, an oscillator's clock
/// for each oscillator, some quantities in time, tight limits. Some specs are refused (an
/// ordered coincidence with more than four inputs, a coincidence with more than eight): the
/// refusal is hashed too.
fn generated_spec(g: &mut TestGen, tick_len_ns: u64) -> MediumSpec {
    let n = 2 + g.below(13) as usize;
    let mut spec = MediumSpec::default();
    spec.oscillome = Oscillome {
        tick_len_ns,
        periods_ns: vec![10_000_000_000, 100_000_000_000],
        cycle_summary: true,
        plasticity_rhythm: Some(0),
        ..Oscillome::default()
    };
    for _ in 0..n {
        let archetype = ARCHETYPES[g.below(8) as usize];
        let form = g.below(3);
        let params = params_for(archetype, form, g);
        let node = g.below(3) as u16;
        let channel = g.below(2) as u16;
        let tagged = g.below(2) == 1;
        spec.cells.push(CellSpec {
            archetype,
            params,
            pattern: (archetype == Archetype::Sense).then_some(Pattern {
                domain: None,
                node: Some(node),
                channel: if channel == 0 {
                    None
                } else {
                    Some(channel - 1)
                },
                tag: tagged.then_some(Tag(1)),
            }),
        });
    }
    // Each oscillator's clock first, so that the random synapses after it never make a second
    // self-synapse (they skip self-loops onto oscillators).
    for i in 0..n {
        if spec.cells[i].archetype == Archetype::Oscillator {
            spec.synapses.push(SynapseSpec {
                from: CellId(i as u32),
                to: CellId(i as u32),
                weight: 0.5 + (g.below(50) as f32) / 100.0,
                delay_ticks: 1 + g.below(5) as u8,
                gate: Gate::None,
                plastic: false,
            });
        }
    }
    let targets: Vec<u32> = (0..n as u32)
        .filter(|i| spec.cells[*i as usize].archetype != Archetype::Sense)
        .collect();
    if !targets.is_empty() {
        for _ in 0..g.below(40) {
            let from = g.below(n as u64) as u32;
            let to = targets[g.below(targets.len() as u64) as usize];
            if from == to && spec.cells[to as usize].archetype == Archetype::Oscillator {
                continue;
            }
            let gate = match g.below(5) {
                0 | 1 => Gate::None,
                2 => Gate::Cell(CellId(g.below(n as u64) as u32)),
                3 => Gate::Field(g.below(4) as u8),
                _ => Gate::Phase {
                    rhythm: g.below(2) as u8,
                    from: (g.below(5) as f32) / 5.0,
                    to: (1 + g.below(4) as u32) as f32 / 5.0 + 0.1,
                },
            };
            spec.synapses.push(SynapseSpec {
                from: CellId(from),
                to: CellId(to),
                weight: (g.below(600) as f32) / 100.0 - 3.0,
                delay_ticks: g.below(3) as u8,
                gate,
                plastic: g.below(2) == 1,
            });
        }
    }
    // Some quantities in time: integrator leaks and lookbacks, emitter lookbacks.
    for i in 0..n {
        let cell = CellId(i as u32);
        match spec.cells[i].archetype {
            Archetype::Integrator if g.below(2) == 1 => {
                spec.oscillome.seconds.push(Timed {
                    target: TimeTarget::Param { cell, index: 0 },
                    ns: 100_000_000 * (1 + g.below(40)),
                });
                spec.oscillome.seconds.push(Timed {
                    target: TimeTarget::Param { cell, index: 3 },
                    ns: 50_000_000 * g.below(60),
                });
            }
            Archetype::Emit if g.below(2) == 1 => {
                spec.oscillome.seconds.push(Timed {
                    target: TimeTarget::Param { cell, index: 2 },
                    ns: 100_000_000 * g.below(50),
                });
            }
            _ => {}
        }
    }
    spec.limits = Limits {
        max_ops_per_tick: 1 + g.below(300),
        max_proposals_per_tick: g.below(4) as u32,
        max_refs: 1 + g.below(5) as u16,
        max_passes: 1 + g.below(3) as u8,
        ..Limits::default()
    };
    spec
}

const VALUES: [f32; 6] = [0.0, 0.5, 1.0, 2.0, -1.0, 1.0e30];

fn generated_events(g: &mut TestGen, start: u64, ticks: u64, tick_len_ns: u64) -> Vec<Event> {
    let mut out = Vec::new();
    for t in start..start + ticks {
        for seq in 0..g.below(6) {
            out.push(Event {
                tick: t,
                offset_ns: g.below(tick_len_ns) as u32,
                source: Address {
                    domain: 0,
                    node: g.below(4) as u16,
                    channel: g.below(2) as u16,
                },
                tags: if g.below(2) == 1 {
                    vec![Tag(1)]
                } else {
                    vec![]
                },
                value: VALUES[g.below(6) as usize],
                seq: seq as u32,
            });
        }
    }
    out
}

/// 300 generated specs, 100 per tick length, 60 ticks each, with a generated field.
fn generated_digest() -> (u64, (u64, u64, u64)) {
    let mut h = Fnv::new();
    let mut seen = (0, 0, 0);
    let mut g = TestGen(0x5eed_0f_3b1d_0001);
    for k in 0..300 {
        let tick = TICKS_NS[k % 3];
        let spec = generated_spec(&mut g, tick);
        let events = generated_events(&mut g, 90, 50, tick);
        let mut field = Field::default();
        for s in field.scalars.iter_mut() {
            *s = (g.below(400) as f32) / 100.0 - 2.0;
        }
        add(
            &mut seen,
            digest_run(&mut h, &spec, events, (90, 60, tick), field),
        );
    }
    (h.0, seen)
}

/// The digests M1b's code produced (see the module documentation).
const OSCILLOME_DIGEST: u64 = 0x9424_e69e_ba20_e0cc;
const GENERATED_DIGEST: u64 = 0x13a2_0c69_f195_42b7;

#[test]
fn with_sub_tick_pruning_off_the_bytes_are_m1bs() {
    let (osc, osc_seen) = oscillome_digest();
    let (generated, gen_seen) = generated_digest();
    eprintln!("oscillome digest {osc:#018x} (built, proposals, truncated ticks) {osc_seen:?}");
    eprintln!(
        "generated digest {generated:#018x} (built, proposals, truncated ticks) {gen_seen:?}"
    );
    // Not vacuous: the cases build, propose and truncate.
    assert_eq!(osc_seen.0, 21);
    assert!(osc_seen.1 > 0 && osc_seen.2 > 0);
    assert!(gen_seen.0 > 150 && gen_seen.1 > 0 && gen_seen.2 > 0);
    assert_eq!(osc, OSCILLOME_DIGEST, "oscillome spec");
    assert_eq!(generated, GENERATED_DIGEST, "generated specs");
}
