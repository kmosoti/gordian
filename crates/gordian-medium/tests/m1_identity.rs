//! The all-off identity with M1 (work item M1b, decision 9): with every oscillome element off,
//! the medium produces the bytes M1 produced for the same spec and events.
//!
//! The digests below were produced by M1's own code: this file was committed first on the
//! `oscillome` branch, on top of the merged M1 crate and before any M1b change, run there, and
//! its digests written in (commit "Pin M1's bytes as digests before the oscillome changes
//! anything"). Checking out that commit and running this test reproduces them. Each digest is
//! FNV-1a 64 over, for every tick of every case: the persisted bytes after the tick, the tick's
//! counts, passes, active cells, carried messages, dropped references, unanchored firings and
//! truncation record, every proposal (kind, anchor, references, strength bits, cell), and the
//! debug text of every trace item; and, per case, the ledger's totals.
//!
//! The specs use only what M1 has, and state their prices explicitly as M1's declared prices
//! (20 / 5 / 10 / 2 ns), because decision 8 changes `Prices::DECLARED`: the same spec means the
//! same prices. Specs are built by mutating `MediumSpec::default()` and fields by mutating
//! `Field::default()`, so this file compiles unchanged against both versions of the crate.

#![allow(clippy::field_reassign_with_default)]

mod common;

use common::{Rig, TestGen, open_field, random_events, rich_spec};
use gordian_medium::{
    Address, Archetype, CellId, CellSpec, Event, Field, Gate, Limits, Medium, MediumSpec, P,
    Pattern, Prices, SynapseSpec, Tag, TickSummary,
};

/// M1's declared prices, written out (decision 8 moves `Prices::DECLARED`).
const M1_PRICES: Prices = Prices {
    cell_update_ps: 20_000,
    synapse_traversal_ps: 5_000,
    event_routing_ps: 10_000,
    field_read_ps: 2_000,
    proposal_ps: 0,
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
}

/// Run `ticks` ticks of `spec` on `events` from `start` with `field`, hashing everything.
/// Returns (built, proposals, truncated ticks), so the test can show the cases are not vacuous.
fn digest_run(
    h: &mut Fnv,
    spec: &MediumSpec,
    events: Vec<Event>,
    start: u64,
    ticks: u64,
    field: Field,
) -> (u64, u64, u64) {
    let Ok(mut m) = Medium::from_spec(spec) else {
        h.u64(0xdead);
        return (0, 0, 0);
    };
    h.bytes(&m.to_bytes());
    let mut rig = Rig::new(start, events);
    rig.field.0 = field;
    let mut seen_props = 0;
    let mut seen_traces = 0;
    for _ in 0..ticks {
        let s = rig.step(&mut m).unwrap();
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

fn with_m1_prices(mut spec: MediumSpec) -> MediumSpec {
    spec.prices = M1_PRICES;
    spec
}

/// Valid M1 parameters for `archetype` from generated bytes (as `tests/properties.rs` does).
fn params_for(archetype: Archetype, g: &mut TestGen) -> [f32; P] {
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
        Archetype::Coincidence => {
            p[0] = f32::from(1 + raw[0] % 8);
            p[1] = f32::from(raw[1] % 6);
            p[2] = f32::from(raw[2] % 2);
            p[3] = f32::from(raw[3] % 20);
        }
        Archetype::Gate => {
            p[0] = f32::from(raw[0] % 4);
            p[1] = f(1) / 64.0 - 1.0;
            p[2] = f32::from(raw[2] % 2);
        }
        Archetype::Latch => {
            p[0] = f(0) / 32.0 - 1.0;
            p[1] = f32::from(raw[1] % 6);
        }
        Archetype::Emit => {
            p[0] = f(0) / 32.0 - 1.0;
            p[1] = f(1);
            p[2] = f32::from(raw[2] % 20);
            p[3] = f32::from(raw[3] % 4);
        }
        #[allow(unreachable_patterns)]
        _ => unreachable!("only M1's seven archetypes are generated"),
    }
    p
}

/// The seven archetypes M1 has, in M1's tag order.
const M1_ARCHETYPES: [Archetype; 7] = [
    Archetype::Sense,
    Archetype::Integrator,
    Archetype::Novelty,
    Archetype::Coincidence,
    Archetype::Gate,
    Archetype::Latch,
    Archetype::Emit,
];

/// A generated M1 spec: 1 to 13 cells of any archetype, up to 39 synapses with every gate kind
/// and delays 0 to 2, tight limits.
fn generated_spec(g: &mut TestGen) -> MediumSpec {
    let n = 1 + g.below(13) as usize;
    let mut spec = MediumSpec::default();
    for _ in 0..n {
        let archetype = M1_ARCHETYPES[g.below(7) as usize];
        let params = params_for(archetype, g);
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
    let non_sense: Vec<u32> = (0..n as u32)
        .filter(|i| spec.cells[*i as usize].archetype != Archetype::Sense)
        .collect();
    if !non_sense.is_empty() {
        for _ in 0..g.below(40) {
            let from = g.below(n as u64) as u32;
            let to = non_sense[g.below(non_sense.len() as u64) as usize];
            let gate = match g.below(4) {
                0 | 1 => Gate::None,
                2 => Gate::Cell(CellId(g.below(n as u64) as u32)),
                _ => Gate::Field(g.below(4) as u8),
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
    spec.limits = Limits {
        max_ops_per_tick: 1 + g.below(300),
        max_proposals_per_tick: g.below(4) as u32,
        max_refs: 1 + g.below(5) as u16,
        max_passes: 1 + g.below(3) as u8,
        ..Limits::default()
    };
    spec.prices = M1_PRICES;
    spec
}

const VALUES: [f32; 6] = [0.0, 0.5, 1.0, 2.0, -1.0, 1.0e30];

fn generated_events(g: &mut TestGen, start: u64, ticks: u64) -> Vec<Event> {
    let mut out = Vec::new();
    for t in start..start + ticks {
        for seq in 0..g.below(6) {
            out.push(Event {
                tick: t,
                offset_ns: g.below(1000) as u32,
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

fn field_of(scalars: [f32; 4]) -> Field {
    let mut f = Field::default();
    f.scalars = scalars;
    f
}

/// The rich spec of `tests/common` (every archetype, every gate kind, feedback), three seeds of
/// sparse events and one of dense events, 120 ticks each.
fn rich_digest() -> (u64, (u64, u64, u64)) {
    let mut h = Fnv::new();
    let mut seen = (0, 0, 0);
    let spec = with_m1_prices(rich_spec(Limits::default()));
    for (seed, per_tick) in [(1u64, 6u64), (2, 6), (3, 6), (11, 40)] {
        let events = random_events(seed, 1_000, 120, per_tick);
        add(
            &mut seen,
            digest_run(&mut h, &spec, events, 1_000, 120, open_field()),
        );
    }
    // The rich spec under tight limits: truncation paths.
    for seed in 0..6u64 {
        let limits = Limits {
            max_ops_per_tick: 1 + seed * 3,
            max_proposals_per_tick: (seed % 3) as u32,
            ..Limits::default()
        };
        let spec = with_m1_prices(rich_spec(limits));
        let events = random_events(seed, 1_000, 60, 10);
        add(
            &mut seen,
            digest_run(&mut h, &spec, events, 1_000, 60, open_field()),
        );
    }
    (h.0, seen)
}

/// 300 generated specs, 30 ticks each, with a generated field.
fn generated_digest() -> (u64, (u64, u64, u64)) {
    let mut h = Fnv::new();
    let mut seen = (0, 0, 0);
    let mut g = TestGen(0x0123_4567_89ab_cdef);
    for _ in 0..300 {
        let spec = generated_spec(&mut g);
        let events = generated_events(&mut g, 50, 22);
        let mut scalars = [0.0f32; 4];
        for s in scalars.iter_mut() {
            *s = (g.below(400) as f32) / 100.0 - 2.0;
        }
        add(
            &mut seen,
            digest_run(&mut h, &spec, events, 50, 30, field_of(scalars)),
        );
    }
    (h.0, seen)
}

/// The digests M1's code produced (see the module documentation).
const RICH_DIGEST: u64 = 0x2b3e_314d_5e15_8d68;
const GENERATED_DIGEST: u64 = 0xa156_72f4_d8b4_f8c8;

#[test]
fn with_every_oscillome_element_off_the_bytes_are_m1s() {
    let (rich, rich_seen) = rich_digest();
    let (generated, gen_seen) = generated_digest();
    eprintln!("rich digest {rich:#018x} (built, proposals, truncated ticks) {rich_seen:?}");
    eprintln!(
        "generated digest {generated:#018x} (built, proposals, truncated ticks) {gen_seen:?}"
    );
    // Not vacuous: the cases build, propose and truncate.
    assert_eq!(rich_seen.0, 10);
    assert!(rich_seen.1 > 0 && rich_seen.2 > 0);
    assert!(gen_seen.0 > 200 && gen_seen.1 > 0 && gen_seen.2 > 0);
    assert_eq!(rich, RICH_DIGEST, "rich spec");
    assert_eq!(generated, GENERATED_DIGEST, "generated specs");
}
