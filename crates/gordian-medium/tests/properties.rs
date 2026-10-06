//! Property tests of docs/medium-ports.md, section 10: no panic on any spec within the limits;
//! counts monotone in events delivered. Plus a worked counterexample showing which reading of
//! "monotone" holds (DESIGN.md, departure on monotonicity).

mod common;

use common::{Rig, at, ev};
use gordian_medium::{
    Address, Archetype, CellId, CellSpec, Event, Field, Gate, Limits, Medium, MediumBuilder,
    MediumSpec, P, Pattern, Prices, SenseMode, SynapseSpec, Tag, TraceItem,
};
use proptest::prelude::*;

/// Valid parameters for `archetype`, drawn from small generated numbers.
fn params_for(archetype: Archetype, raw: [u8; P]) -> [f32; P] {
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
    }
    p
}

fn arb_cell() -> impl Strategy<Value = CellSpec> {
    (0usize..7, any::<[u8; P]>(), 0u16..3, 0u16..2, any::<bool>()).prop_map(
        |(a, raw, node, channel, tagged)| {
            let archetype = Archetype::ALL[a];
            CellSpec {
                archetype,
                params: params_for(archetype, raw),
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
            }
        },
    )
}

fn arb_spec() -> impl Strategy<Value = MediumSpec> {
    (
        prop::collection::vec(arb_cell(), 1..14),
        prop::collection::vec(
            (
                any::<u16>(),
                any::<u16>(),
                -3.0f32..3.0,
                0u8..3,
                0u8..4,
                any::<bool>(),
            ),
            0..40,
        ),
        (1u64..300, 0u32..4, 1u16..6, 1u8..4),
    )
        .prop_map(
            |(cells, raw_syn, (max_ops, max_props, max_refs, max_passes))| {
                let n = cells.len() as u16;
                let non_sense: Vec<u32> = cells
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.archetype != Archetype::Sense)
                    .map(|(i, _)| i as u32)
                    .collect();
                let mut synapses = Vec::new();
                if !non_sense.is_empty() {
                    for (from, to, weight, delay, gate, plastic) in raw_syn {
                        let to = non_sense[usize::from(to) % non_sense.len()];
                        let gate = match gate {
                            0 | 1 => Gate::None,
                            2 => Gate::Cell(CellId(u32::from(from.wrapping_mul(7) % n))),
                            _ => Gate::Field((from % 4) as u8),
                        };
                        synapses.push(SynapseSpec {
                            from: CellId(u32::from(from % n)),
                            to: CellId(to),
                            weight,
                            delay_ticks: delay,
                            gate,
                            plastic,
                        });
                    }
                }
                MediumSpec {
                    cells,
                    synapses,
                    limits: Limits {
                        max_ops_per_tick: max_ops,
                        max_proposals_per_tick: max_props,
                        max_refs,
                        max_passes,
                        ..Limits::default()
                    },
                    prices: Prices::DECLARED,
                }
            },
        )
}

/// One scripted event: (offset, node, channel, value index, tagged).
type ScriptEvent = (u32, u16, u16, u8, bool);

/// Per tick, a list of scripted events.
fn arb_script() -> impl Strategy<Value = Vec<Vec<ScriptEvent>>> {
    prop::collection::vec(
        prop::collection::vec((0u32..1000, 0u16..4, 0u16..2, 0u8..6, any::<bool>()), 0..6),
        1..25,
    )
}

const VALUES: [f32; 6] = [0.0, 0.5, 1.0, 2.0, -1.0, 1.0e30];

fn events_of(script: &[Vec<ScriptEvent>], start: u64) -> Vec<Event> {
    let mut out = Vec::new();
    for (t, tick) in script.iter().enumerate() {
        for (seq, &(offset, node, channel, v, tagged)) in tick.iter().enumerate() {
            out.push(Event {
                tick: start + t as u64,
                offset_ns: offset,
                source: Address {
                    domain: 0,
                    node,
                    channel,
                },
                tags: if tagged { vec![Tag(1)] } else { vec![] },
                value: VALUES[usize::from(v)],
                seq: seq as u32,
            });
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any spec within the limits builds or is refused, and then runs any events without
    /// panicking, without exceeding its limits, with consistent totals and persisted bytes that
    /// restore exactly.
    #[test]
    fn no_panic_on_any_spec_within_the_limits(
        spec in arb_spec(),
        script in arb_script(),
        field in prop::array::uniform4(-2.0f32..2.0),
    ) {
        let Ok(mut m) = Medium::from_spec(&spec) else {
            // Refused (for example a coincidence cell with more than eight inputs): no panic.
            return Ok(());
        };
        let mut rig = Rig::new(50, events_of(&script, 50));
        rig.field.0 = Field { scalars: field };
        let mut previous = *m.totals();
        for _ in 0..script.len() + 8 {
            let s = rig.step(&mut m).unwrap();
            prop_assert!(s.counts.ops() <= spec.limits.max_ops_per_tick);
            prop_assert!(s.counts.proposals <= u64::from(spec.limits.max_proposals_per_tick));
            prop_assert!(s.passes <= spec.limits.max_passes);
            // Cumulative counts never go down as events are delivered.
            let now = *m.totals();
            prop_assert!(now.cell_updates >= previous.cell_updates);
            prop_assert!(now.event_routings >= previous.event_routings);
            prop_assert!(now.synapse_traversals >= previous.synapse_traversals);
            previous = now;
            for c in m.cells() {
                prop_assert!(c.activation.is_finite());
                prop_assert!(c.state.iter().all(|x| x.is_finite()));
                prop_assert!(c.support.len() <= usize::from(spec.limits.max_refs));
            }
        }
        prop_assert_eq!(&rig.ledger.totals, m.totals());
        let bytes = m.to_bytes();
        prop_assert_eq!(Medium::from_bytes(&bytes).unwrap().to_bytes(), bytes);
    }

    /// From the same state, delivering a superset of a tick's events never decreases that tick's
    /// event routings or the cells run in its first pass (when the tick is not truncated).
    #[test]
    fn counts_are_monotone_in_events_delivered(
        spec in arb_spec(),
        script in arb_script(),
        extra in prop::collection::vec((0u32..1000, 0u16..4, 0u16..2, 0u8..6, any::<bool>()), 1..5),
    ) {
        let spec = MediumSpec {
            limits: Limits { max_ops_per_tick: 1_000_000, max_proposals_per_tick: 1_000, ..spec.limits },
            ..spec
        };
        let Ok(mut m) = Medium::from_spec(&spec) else { return Ok(()); };
        // Bring the medium to some state.
        let mut rig = Rig::new(50, events_of(&script, 50));
        for _ in 0..script.len() {
            rig.step(&mut m).unwrap();
        }
        let tick = 50 + script.len() as u64;
        let base = events_of(&script[..1], tick);
        let mut more_script = script[..1].to_vec();
        more_script[0].extend(extra);
        let more = events_of(&more_script, tick);
        let first_pass = |events: Vec<Event>| {
            let mut copy = m.clone();
            let mut r = Rig::new(tick, events);
            let s = r.step(&mut copy).unwrap();
            let pass1 = r.trace.traces[0]
                .items
                .iter()
                .filter(|i| matches!(i, TraceItem::Ran { pass: 1, .. }))
                .count();
            (s.counts.event_routings, pass1, s.truncation)
        };
        let (routings_a, pass1_a, ta) = first_pass(base);
        let (routings_b, pass1_b, tb) = first_pass(more);
        prop_assert!(ta.is_none() && tb.is_none());
        prop_assert!(routings_b >= routings_a);
        prop_assert!(pass1_b >= pass1_a);
    }

    /// Arbitrary bytes never make decoding panic.
    #[test]
    fn decoding_arbitrary_bytes_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..400)) {
        let mut with_magic = b"GMED\x01".to_vec();
        with_magic.extend_from_slice(&bytes);
        let _ = Medium::from_bytes(&bytes);
        let _ = Medium::from_bytes(&with_magic);
    }
}

/// The stronger reading of "counts are monotone in events delivered" (a superset of events never
/// gives fewer operations over the tick or the run) is false for this design, and should be:
/// inhibition exists. Worked by hand: an integrator (leak 1, threshold 1, reset) gets +1 from a
/// sense cell at node 0 and -1 from one at node 1, and drives a latch holding 3 ticks.
/// - Event at node 0 only: sense 1 run, integrator fires (1 run), latch runs t0..t3 (4 runs):
///   6 cell updates.
/// - The same plus an event at node 1 in the same tick: 2 sense runs, the integrator sums 0 and
///   stays quiet (1 run), the latch never runs: 3 cell updates.
#[test]
fn whole_tick_counts_are_not_monotone_in_events() {
    let mut b = MediumBuilder::new();
    let excite = b.sense(at(0, 0), SenseMode::Presence);
    let inhibit = b.sense(at(1, 0), SenseMode::Presence);
    let i = b.integrator(1.0, 1.0, true, 10);
    let l = b.latch(0.5, 3);
    b.synapse(excite, i, 1.0, 0);
    b.synapse(inhibit, i, -1.0, 0);
    b.synapse(i, l, 1.0, 1);
    let spec = b.into_spec();
    let run = |events: Vec<Event>| {
        let mut m = Medium::from_spec(&spec).unwrap();
        let mut rig = Rig::new(0, events);
        for _ in 0..6 {
            rig.step(&mut m).unwrap();
        }
        m.totals().cell_updates
    };
    assert_eq!(run(vec![ev(0, 0, 0, 0, 1.0, 0)]), 6);
    assert_eq!(run(vec![ev(0, 0, 0, 0, 1.0, 0), ev(0, 5, 1, 0, 1.0, 1)]), 3);
}
