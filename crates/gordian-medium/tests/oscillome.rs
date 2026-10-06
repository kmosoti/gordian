//! The oscillome (work item M1b, docs/medium-ports.md section 4b): its acceptance tests and a
//! hand-computed example for each element. Every expected number below was worked by hand; the
//! comments show the working.

mod common;

use common::{
    ABNORMAL, NOTICE, ORDERED, RETIRE, RHYTHMS_NS, Rig, ev, oscillome_spec, random_events_in,
    rich_spec,
};
use gordian_medium::oscillome::{
    bin_index, cycle_index, delay_ticks, phase_of, span_ticks, time_kind,
};
use gordian_medium::{
    Archetype, CellId, Event, EventRef, Field, Gate, InMemoryPersist, Limits, Medium,
    MediumBuilder, MediumSpec, OpCounts, Oscillome, Pattern, Prices, SenseMode, StepError,
    SynapseSpec, TimeKind, TimeTarget, TraceItem, secs,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;

fn at(node: u16, channel: u16) -> Pattern {
    common::at(node, channel)
}

fn r(tick: u64, offset_ns: u32, seq: u32) -> EventRef {
    EventRef {
        tick,
        offset_ns,
        seq,
    }
}

// ---------------------------------------------------------------------------------------------
// Decision 3: phases are a pure function of the tick index and the tick length.

/// An integer reference written independently of the crate: the remainder by modular
/// multiplication, the 24 fraction bits by binary long division, the boundary by whether adding
/// one tick crosses the period.
fn reference(tick: u64, len: u64, period: u64) -> (f32, bool) {
    let p = u128::from(period);
    let rem = (u128::from(tick) % p) * (u128::from(len) % p) % p;
    let mut q: u32 = 0;
    let mut x = rem;
    for _ in 0..24 {
        x *= 2;
        q <<= 1;
        if x >= p {
            x -= p;
            q |= 1;
        }
    }
    let phase = f32::from_bits(0) + (q as f32) * (1.0 / 16_777_216.0);
    (phase, rem + u128::from(len) >= p)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    /// The engine's phase and boundary at any tick equal the integer reference's, the phase lies
    /// in [0, 1) within 2^-24 of the exact fraction, and neither depends on any history: the
    /// engine has no state they read.
    #[test]
    fn phases_are_a_pure_function_of_tick_index_and_tick_length(
        tick in any::<u64>(),
        len in 1u64..=u64::from(u32::MAX),
        extra in 1u64..1_000_000_000_000,
    ) {
        let period = len + extra;
        let o = Oscillome {
            tick_len_ns: len,
            periods_ns: vec![period],
            ..Oscillome::default()
        };
        let engine = gordian_medium::OscillomeEngine::new(&o);
        let (phase, boundary) = reference(tick, len, period);
        prop_assert_eq!(engine.phase(0, tick).to_bits(), phase.to_bits());
        prop_assert_eq!(phase_of(tick, len, period).to_bits(), phase.to_bits());
        prop_assert_eq!(engine.is_boundary(0, tick), boundary);
        prop_assert!((0.0..1.0).contains(&phase));
        let exact = ((u128::from(tick) * u128::from(len)) % u128::from(period)) as f64
            / period as f64;
        prop_assert!(exact - f64::from(phase) >= 0.0);
        prop_assert!(exact - f64::from(phase) < 1.0 / 16_777_216.0 + 1e-12);
        // The cycle index moves by one exactly at a boundary.
        let next = cycle_index(tick.saturating_add(1), len, period);
        if tick < u64::MAX {
            prop_assert_eq!(next - cycle_index(tick, len, period), u64::from(boundary));
        }
    }
}

/// The phases the medium uses are the engine's, whatever the field adapter supplied, and are the
/// same whether the medium started at tick 0 or later. Observed through two phase gates (one per
/// rhythm, window [0.65, 0.75)) on 300 ms ticks, while the field adapter claims every phase is
/// 0.7: a gate carries exactly when the reference phase is in its window.
#[test]
fn the_field_carries_the_engines_phases_from_any_start() {
    let len = 300 * MS;
    let mut b = MediumBuilder::new().oscillome(Oscillome {
        tick_len_ns: len,
        periods_ns: RHYTHMS_NS.to_vec(),
        ..Oscillome::default()
    });
    let s = b.sense(at(0, 0), SenseMode::Presence);
    let latches: Vec<CellId> = (0..2u8)
        .map(|rhythm| {
            let l = b.latch(0.5, 0);
            b.synapse_with(SynapseSpec {
                from: s,
                to: l,
                weight: 1.0,
                delay_ticks: 0,
                gate: Gate::Phase {
                    rhythm,
                    from: 0.65,
                    to: 0.75,
                },
                plastic: false,
            });
            l
        })
        .collect();
    let spec = b.into_spec();
    for start in [0u64, 500] {
        let mut m = Medium::from_spec(&spec).unwrap();
        let events = (start..start + 700)
            .map(|t| ev(t, 0, 0, 0, 1.0, 0))
            .collect();
        let mut rig = Rig::with_tick(start, len, events);
        rig.field.0 = Field {
            phases: [0.7, 0.7, 0.7],
            ..Field::default()
        };
        let mut carried = [0u32; 2];
        for t in start..start + 700 {
            rig.step(&mut m).unwrap();
            for (k, p) in RHYTHMS_NS.iter().enumerate() {
                let phase = reference(t, len, *p).0;
                assert_eq!(m.engine().phase(k, t), phase);
                let open = (0.65..0.75).contains(&phase);
                let active = m.cells()[latches[k].0 as usize].activation_at(t) > 0.0;
                assert_eq!(active, open, "start {start}, tick {t}, rhythm {k}");
                carried[k] += u32::from(active);
            }
        }
        assert!(
            carried[0] > 0 && carried[1] > 0,
            "start {start}: {carried:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Decision 3, unevenness, and phase gates.

#[test]
fn a_phase_gate_carries_only_inside_its_window_and_counts_a_field_read() {
    // A 10 s rhythm on 1 s ticks: phases 0, 0.1, ..., 0.9. Window [0, 0.5): ticks 0-4 carry,
    // 5-9 do not, 10-14 carry. A sense cell at node 0 fires every tick into a latch (hold 0)
    // through the gate.
    let mut b = MediumBuilder::new().oscillome(Oscillome {
        tick_len_ns: secs(1.0),
        periods_ns: vec![secs(10.0)],
        ..Oscillome::default()
    });
    let s = b.sense(at(0, 0), SenseMode::Presence);
    let l = b.latch(0.5, 0);
    b.synapse_with(SynapseSpec {
        from: s,
        to: l,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::Phase {
            rhythm: 0,
            from: 0.0,
            to: 0.5,
        },
        plastic: false,
    });
    let mut m = b.build().unwrap();
    let events = (0..15).map(|t| ev(t, 0, 0, 0, 1.0, 0)).collect();
    let mut rig = Rig::with_tick(0, secs(1.0), events);
    let mut carried = Vec::new();
    for t in 0..15 {
        let summary = rig.step(&mut m).unwrap();
        carried.push(m.cells()[l.0 as usize].activation_at(t) > 0.0);
        assert_eq!(
            summary.counts.field_reads, 1,
            "one phase read per traversal"
        );
    }
    let expected: Vec<bool> = (0..15).map(|t| t % 10 < 5).collect();
    assert_eq!(carried, expected);
    // The trace marks the traversal as a field read.
    assert!(rig.trace.traces[7].items.iter().any(|i| matches!(
        i,
        TraceItem::Traversed {
            field_read: true,
            carried: false,
            ..
        }
    )));
}

// ---------------------------------------------------------------------------------------------
// Decision 4: binding by phase and the sliding window are both modes of `Coincidence`.

/// Two sources one tick apart, straddling a bin edge, coincide in the window form and not in
/// the binned form; two sources almost a bin apart inside one bin coincide in the binned form
/// and not in a short window. 10 s rhythm in 10 bins of 1 s, on 500 ms ticks (2 ticks per bin).
#[test]
fn bins_and_windows_disagree_at_the_edges() {
    let o = Oscillome {
        tick_len_ns: 500 * MS,
        periods_ns: vec![secs(10.0)],
        ..Oscillome::default()
    };
    let run = |binned: bool, script: &[(u64, u16)]| -> Vec<f32> {
        let mut b = MediumBuilder::new().oscillome(o.clone());
        let s0 = b.sense(at(0, 0), SenseMode::Presence);
        let s1 = b.sense(at(1, 0), SenseMode::Presence);
        let c = if binned {
            b.coincidence_binned(2, 0, 10, 0, true, 10)
        } else {
            b.coincidence(2, 1, true, 10)
        };
        b.synapse(s0, c, 1.0, 0);
        b.synapse(s1, c, 1.0, 0);
        let mut m = b.build().unwrap();
        let events = script
            .iter()
            .map(|&(t, node)| ev(t, 0, node, 0, 1.0, 0))
            .collect();
        let mut rig = Rig::with_tick(0, 500 * MS, events);
        (0..6)
            .map(|t| {
                rig.step(&mut m).unwrap();
                m.cells()[c.0 as usize].activation_at(t)
            })
            .collect()
    };
    // Ticks 1 and 2 straddle the edge between bin 0 (ticks 0, 1) and bin 1 (ticks 2, 3).
    let straddle = [(1, 0), (2, 1)];
    assert_eq!(run(false, &straddle), vec![0.0, 0.0, 2.0, 0.0, 0.0, 0.0]);
    assert_eq!(run(true, &straddle), vec![0.0; 6]);
    // Ticks 2 and 3 share bin 1.
    let inside = [(2, 0), (3, 1)];
    assert_eq!(run(true, &inside), vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    // A binned coincidence reads its rhythm's phase: one field read per run.
    let mut b = MediumBuilder::new().oscillome(o.clone());
    let s0 = b.sense(at(0, 0), SenseMode::Presence);
    let c = b.coincidence_binned(1, 0, 10, 0, false, 10);
    b.synapse(s0, c, 1.0, 0);
    let mut m = b.build().unwrap();
    let mut rig = Rig::with_tick(0, 500 * MS, vec![ev(0, 0, 0, 0, 1.0, 0)]);
    let s = rig.step(&mut m).unwrap();
    assert_eq!((s.counts.cell_updates, s.counts.field_reads), (2, 1));
}

/// Decision 1: at a 2 s tick a burst's order lives in `offset_ns`, and the ordered coincidence
/// reads it. Node 0 then node 1 within 150 ms fires; node 1 first does not (the lead); 400 ms
/// apart does not (the window); across a tick edge, 1.95 s and 2.05 s (tick 0 at 1.95 s, tick 1
/// at 0.05 s) fires, because the window is in event time, not ticks.
#[test]
fn the_ordered_coincidence_reads_order_inside_a_long_tick() {
    let len = secs(2.0);
    let run = |events: Vec<Event>| -> Vec<f32> {
        let mut b = MediumBuilder::new().oscillome(Oscillome {
            tick_len_ns: len,
            ..Oscillome::default()
        });
        let s0 = b.sense(at(0, 0), SenseMode::Presence);
        let s1 = b.sense(at(1, 0), SenseMode::Presence);
        let c = b.coincidence_ordered(2, 150_000, true, true, 10);
        b.synapse(s0, c, 1.0, 0);
        b.synapse(s1, c, 1.0, 0);
        let mut m = b.build().unwrap();
        let mut rig = Rig::with_tick(0, len, events);
        (0..3)
            .map(|t| {
                rig.step(&mut m).unwrap();
                m.cells()[c.0 as usize].activation_at(t)
            })
            .collect()
    };
    let ms = |x: u32| x * 1_000_000;
    assert_eq!(
        run(vec![
            ev(0, ms(100), 0, 0, 1.0, 0),
            ev(0, ms(180), 1, 0, 1.0, 1)
        ]),
        vec![2.0, 0.0, 0.0]
    );
    assert_eq!(
        run(vec![
            ev(0, ms(180), 0, 0, 1.0, 0),
            ev(0, ms(100), 1, 0, 1.0, 1)
        ]),
        vec![0.0, 0.0, 0.0],
        "node 1 first: no lead"
    );
    assert_eq!(
        run(vec![
            ev(0, ms(100), 0, 0, 1.0, 0),
            ev(0, ms(500), 1, 0, 1.0, 1)
        ]),
        vec![0.0, 0.0, 0.0],
        "400 ms apart"
    );
    assert_eq!(
        run(vec![
            ev(0, ms(1950), 0, 0, 1.0, 0),
            ev(1, ms(50), 1, 0, 1.0, 0)
        ]),
        vec![0.0, 2.0, 0.0],
        "100 ms apart across a tick edge"
    );
    // The sliding window cannot tell the first two apart: both orders fire.
    let window = |events: Vec<Event>| -> f32 {
        let mut b = MediumBuilder::new();
        let s0 = b.sense(at(0, 0), SenseMode::Presence);
        let s1 = b.sense(at(1, 0), SenseMode::Presence);
        let c = b.coincidence(2, 0, true, 10);
        b.synapse(s0, c, 1.0, 0);
        b.synapse(s1, c, 1.0, 0);
        let mut m = b.build().unwrap();
        let mut rig = Rig::with_tick(0, len, events);
        rig.step(&mut m).unwrap();
        m.cells()[c.0 as usize].activation_at(0)
    };
    assert_eq!(
        window(vec![
            ev(0, ms(180), 0, 0, 1.0, 0),
            ev(0, ms(100), 1, 0, 1.0, 1)
        ]),
        2.0
    );
}

// ---------------------------------------------------------------------------------------------
// Decision 5: the oscillator wakes itself once per cycle, by a delayed self-message.

#[test]
fn the_oscillator_runs_once_per_cycle_and_decays() {
    // Threshold 1, floor 0.2, no cycle limit, decay 0.5, period 3. One event at t0 (value 1):
    // fires 1 at t0, 0.5 at t3, 0.25 at t6; at t9 the amplitude 0.125 is below the floor: it
    // stops. Runs: t0, t3, t6, t9 (4 updates over 12 ticks); self traversals at t0, t3, t6.
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let (osc, _) = b.oscillator(1.0, 0.2, 0, 0.5, 3);
    b.synapse(s, osc, 1.0, 0);
    let mut m = b.build().unwrap();
    let mut rig = Rig::new(0, vec![ev(0, 0, 0, 0, 1.0, 0)]);
    let mut acts = Vec::new();
    let mut osc_runs = 0;
    for t in 0..12 {
        rig.step(&mut m).unwrap();
        acts.push(m.cells()[osc.0 as usize].activation_at(t));
        osc_runs += rig.trace.traces[t as usize]
            .items
            .iter()
            .filter(|i| matches!(i, TraceItem::Ran { cell, .. } if *cell == osc))
            .count();
    }
    assert_eq!(
        acts,
        vec![1.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(osc_runs, 4, "per cycle, not per tick");
    // Sense (1 run, 1 traversal) + oscillator (4 runs, 3 self traversals).
    assert_eq!(m.totals().cell_updates, 5);
    assert_eq!(m.totals().synapse_traversals, 4);
    assert_eq!(m.cells()[osc.0 as usize].state[1], -1.0, "stopped");
    assert_eq!(m.pending_messages(), 0);
}

#[test]
fn a_reset_mid_cycle_restarts_the_phase_and_ends_the_old_chain() {
    // Period 3, decay 1 (no decay), 3 cycles at most. Events at t0 (1.0) and t4 (2.0).
    // t0: reset, fires 1.  t3: cycle 1, fires 1.  t4: reset (age 0), fires 2; the t3 message
    // to itself is due at t6.  t6: age 2, not a whole period: ignored, that chain ends.
    // t7: cycle 1 of the new phase, fires 2.  t10: cycle 2.  t13: cycle 3.  t16: cycle 4 > 3:
    // stops. The proposals of a downstream emitter cite the reset's event as anchor.
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let (osc, _) = b.oscillator(1.0, 0.0, 3, 1.0, 3);
    let e = b.emit(0.5, 4, 100, 0);
    b.synapse(s, osc, 1.0, 0);
    b.synapse(osc, e, 1.0, 1);
    let mut m = b.build().unwrap();
    let mut rig = Rig::new(0, vec![ev(0, 5, 0, 0, 1.0, 0), ev(4, 9, 0, 0, 2.0, 0)]);
    let mut acts = Vec::new();
    for t in 0..18 {
        rig.step(&mut m).unwrap();
        acts.push(m.cells()[osc.0 as usize].activation_at(t));
    }
    let fired: Vec<(usize, f32)> = acts
        .iter()
        .enumerate()
        .filter(|(_, a)| **a != 0.0)
        .map(|(t, a)| (t, *a))
        .collect();
    assert_eq!(
        fired,
        vec![(0, 1.0), (3, 1.0), (4, 2.0), (7, 2.0), (10, 2.0), (13, 2.0)]
    );
    let anchors: Vec<(u64, EventRef)> = rig
        .effector
        .proposals
        .iter()
        .map(|(t, p)| (*t, p.anchor))
        .collect();
    assert_eq!(
        anchors,
        vec![
            (1, r(0, 5, 0)),
            (4, r(0, 5, 0)),
            (5, r(4, 9, 0)),
            (8, r(4, 9, 0)),
            (11, r(4, 9, 0)),
            (14, r(4, 9, 0)),
        ]
    );
}

/// A reset in the pass its own message arrives: the old phase is absorbed, and the new phase
/// cites only the event that reset it, not the events of the old phase its message carried. The
/// sense-to-oscillator synapse has a delay of one tick, so both arrive in pass 1. Period 3, one
/// cycle. e0 (t0, 1.0) resets it at t1 (fires 1.0, below the emitter's 1.5); its message is due at
/// t4, when e3 (t3, 2.0) arrives too: e3 resets it (fires 2.0, citing e3 only); cycle 1 at t7
/// (2.0, still citing e3); stops at t10. The emitter, one tick behind, proposes at t5 and t8.
/// (When the two arrive in different passes, the oscillator runs in both: the old cycle first,
/// then the reset, and downstream sees both, as for any cell; DESIGN.md departure 44.)
#[test]
fn a_reset_with_its_own_message_cites_only_the_new_event() {
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let (osc, _) = b.oscillator(1.0, 0.0, 1, 1.0, 3);
    let e = b.emit(1.5, 4, 100, 0);
    b.synapse(s, osc, 1.0, 1);
    b.synapse(osc, e, 1.0, 1);
    let mut m = b.build().unwrap();
    let mut rig = Rig::new(0, vec![ev(0, 5, 0, 0, 1.0, 0), ev(3, 9, 0, 0, 2.0, 0)]);
    for _ in 0..12 {
        rig.step(&mut m).unwrap();
    }
    let got: Vec<(u64, EventRef, Vec<EventRef>)> = rig
        .effector
        .proposals
        .iter()
        .map(|(t, p)| (*t, p.anchor, p.refs.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (5, r(3, 9, 0), vec![r(3, 9, 0)]),
            (8, r(3, 9, 0), vec![r(3, 9, 0)])
        ]
    );
    assert_eq!(m.cells()[osc.0 as usize].state[1], -1.0, "stopped");
}

// ---------------------------------------------------------------------------------------------
// Decision 7: a latch whose hold expires proposes `retire`, citing its anchor.

#[test]
fn a_retiring_latch_proposes_retire_when_its_hold_expires() {
    // Threshold 1, hold 2, retire kind 9. t0: event e0 (1.5) fires it: held t0, t1, t2; it wakes
    // on t1, t2 and t3; at t3 the hold has expired: it proposes retire, anchor e0, strength 1.5.
    // Then t6: e6 fires it again; t7: e7 re-fires within the hold (merged: anchor stays e6);
    // held to t9; at t10, retire with anchor e6 and refs [e6, e7].
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let l = b.latch_retiring(1.0, 2, 9);
    b.synapse(s, l, 1.0, 0);
    let mut m = b.build().unwrap();
    let events = vec![
        ev(0, 3, 0, 0, 1.5, 0),
        ev(6, 4, 0, 0, 2.0, 0),
        ev(7, 1, 0, 0, 3.0, 0),
    ];
    let mut rig = Rig::new(0, events);
    let mut latch_runs = 0;
    for t in 0..12 {
        rig.step(&mut m).unwrap();
        latch_runs += rig.trace.traces[t as usize]
            .items
            .iter()
            .filter(|i| matches!(i, TraceItem::Ran { cell, .. } if *cell == l))
            .count();
    }
    let got: Vec<_> = rig
        .effector
        .proposals
        .iter()
        .map(|(t, p)| (*t, p.kind, p.anchor, p.refs.clone(), p.strength))
        .collect();
    assert_eq!(
        got,
        vec![
            (3, 9, r(0, 3, 0), vec![r(0, 3, 0)], 1.5),
            (10, 9, r(6, 4, 0), vec![r(6, 4, 0), r(7, 1, 0)], 3.0),
        ]
    );
    // t0..t3 (4 runs) and t6..t10 (6 runs: at t7 the wake arrives in pass 1 and the sense
    // message in pass 2, so the latch runs twice, as M1's latch does): one run more per hold
    // than M1's latch, to see it end.
    assert_eq!(latch_runs, 10);
    assert!(m.cells()[l.0 as usize].support.is_empty());
}

// ---------------------------------------------------------------------------------------------
// Decision 6 and the schedules: cycle summaries, plasticity and trace at boundaries.

#[test]
fn cycle_summaries_reach_plasticity_at_the_named_boundaries_only() {
    // 500 ms ticks; rhythms 10 s (20 ticks) and 100 s (200 ticks); plasticity on the 10 s
    // rhythm. 450 ticks from tick 0: boundaries of the 10 s rhythm at 19, 39, ..., 439 (22).
    let len = 500 * MS;
    let spec = oscillome_spec(Limits::default(), len);
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::with_tick(0, len, random_events_in(3, 0, 450, 6, len));
    rig.plasticity.scale = Some(0.5);
    let mut completed = Vec::new();
    let mut totals_since = OpCounts::default();
    for _ in 0..450 {
        let s = rig.step(&mut m).unwrap();
        totals_since.accumulate(&s.counts);
        for c in &s.completed {
            if c.rhythm == 0 {
                // A cycle's counts are the sum of its ticks' counts.
                assert_eq!(c.counts, totals_since);
                totals_since = OpCounts::default();
            }
        }
        completed.extend(s.completed);
    }
    assert!(rig.plasticity.ticks.is_empty(), "never every tick");
    let at: Vec<(u64, u64)> = rig
        .plasticity
        .cycles
        .iter()
        .map(|c| (c.cycle, c.first_tick.unwrap()))
        .collect();
    assert_eq!(at, (0..22).map(|c| (c, 20 * c)).collect::<Vec<_>>());
    assert!(rig.plasticity.cycles.iter().all(|c| c.ticks == 20));
    let slow: Vec<_> = completed.iter().filter(|c| c.rhythm == 1).collect();
    assert_eq!(slow.len(), 2);
    assert_eq!((slow[0].first_tick, slow[0].ticks), (Some(0), 200));
    // Plasticity changed the plastic weights (0.5 per boundary, 22 times).
    let w = m.synapses().iter().find(|s| s.plastic).unwrap().weight;
    assert_eq!(w, 0.5f32.powi(22));
    // The run was not vacuous: every proposal kind of the spec appeared.
    let kinds: std::collections::BTreeSet<u16> =
        rig.effector.proposals.iter().map(|(_, p)| p.kind).collect();
    assert!(
        kinds.contains(&NOTICE) && kinds.contains(&RETIRE),
        "{kinds:?}"
    );
}

#[test]
fn the_trace_rhythm_samples_only_boundary_ticks() {
    let len = 2_000 * MS;
    let mut spec = oscillome_spec(Limits::default(), len);
    spec.oscillome.trace_rhythm = Some(0);
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::with_tick(7, len, random_events_in(5, 7, 40, 4, len));
    for _ in 0..40 {
        rig.step(&mut m).unwrap();
    }
    // 10 s on 2 s ticks: boundaries 9, 14, ..., 44 (tick t ends a cycle when t % 5 == 4).
    let traced: Vec<u64> = rig.trace.traces.iter().map(|t| t.tick).collect();
    assert_eq!(traced, vec![9, 14, 19, 24, 29, 34, 39, 44]);
}

// ---------------------------------------------------------------------------------------------
// Acceptance: identical bytes after snapshot and restore, at a boundary and mid-cycle.

fn run_bytes(spec: &MediumSpec, len: u64, events: &[Event], ticks: u64) -> Vec<Vec<u8>> {
    let mut m = Medium::from_spec(spec).unwrap();
    let mut rig = Rig::with_tick(0, len, events.to_vec());
    rig.plasticity.scale = Some(0.9);
    (0..ticks)
        .map(|_| {
            rig.step(&mut m).unwrap();
            m.to_bytes()
        })
        .collect()
}

#[test]
fn snapshot_and_restore_at_a_boundary_and_mid_cycle_give_identical_bytes() {
    for len in [100 * MS, 500 * MS, 2_000 * MS] {
        let spec = oscillome_spec(Limits::default(), len);
        let ticks = 2 * (RHYTHMS_NS[1] / len) + 37;
        let events = random_events_in(17, 0, ticks, 6, len);
        let reference = run_bytes(&spec, len, &events, ticks);
        let per10 = RHYTHMS_NS[0] / len;
        let per100 = RHYTHMS_NS[1] / len;
        // After a 10 s boundary, after a tick that is both, and in the middle of both cycles.
        for k in [per10, per100, per10 + per10 / 2, per100 + 3, 1] {
            let mut m = Medium::from_spec(&spec).unwrap();
            let mut rig = Rig::with_tick(0, len, events.clone());
            rig.plasticity.scale = Some(0.9);
            for _ in 0..k {
                rig.step(&mut m).unwrap();
            }
            let mut store = InMemoryPersist::default();
            m.persist(&mut store);
            let mid = m.engine().summaries().unwrap().iter().any(|c| c.ticks > 0);
            let mut restored = Medium::restore(&store).unwrap();
            assert_eq!(restored.to_bytes(), m.to_bytes());
            assert_eq!(restored.to_bytes()[4], 2, "the oscillome's encoding");
            // k = per100: tick per100 - 1 ended a cycle of both rhythms, so nothing is in
            // progress (a snapshot exactly at a boundary); every other k is mid-way in at least
            // the 100 s cycle, whose summary in progress is persisted.
            assert_eq!(mid, k != per100, "len {len}, k {k}");
            drop(m);
            for t in k..ticks {
                rig.step(&mut restored).unwrap();
                assert_eq!(
                    restored.to_bytes(),
                    reference[t as usize],
                    "len {len}, k {k}, tick {t}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Decision 9: every element switchable off; all off is M1.

/// With every element off the medium persists in M1's encoding (version 1); switching on any
/// one element moves it to version 2; switching it off again restores M1's bytes for the run.
#[test]
fn each_element_switches_on_and_off_independently() {
    let base = rich_spec(Limits::default());
    let len = 100 * MS;
    let events = common::random_events(4, 0, 60, 6);
    let run = |spec: &MediumSpec| -> Vec<Vec<u8>> {
        let mut m = Medium::from_spec(spec).unwrap();
        let mut rig = Rig::with_tick(0, len, events.clone());
        rig.field.0 = common::open_field();
        (0..60)
            .map(|_| {
                rig.step(&mut m).unwrap();
                m.to_bytes()
            })
            .collect()
    };
    let m1 = run(&base);
    assert!(!base.uses_oscillome());
    assert!(m1.iter().all(|b| b[4] == 1));
    type Switch<'a> = Box<dyn Fn(&mut MediumSpec) + 'a>;
    let switches: Vec<(&str, Switch<'_>)> = vec![
        (
            "tick length",
            Box::new(|s: &mut MediumSpec| s.oscillome.tick_len_ns = len),
        ),
        (
            "rhythms",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.periods_ns = vec![secs(10.0)];
            }),
        ),
        (
            "cycle summary and plasticity schedule",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.periods_ns = vec![secs(10.0)];
                s.oscillome.cycle_summary = true;
                s.oscillome.plasticity_rhythm = Some(0);
            }),
        ),
        (
            "trace schedule",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.periods_ns = vec![secs(10.0)];
                s.oscillome.trace_rhythm = Some(0);
            }),
        ),
        (
            "a quantity in time",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.seconds.push(gordian_medium::Timed {
                    target: TimeTarget::Delay {
                        synapse: gordian_medium::SynapseId(0),
                    },
                    ns: 0,
                });
            }),
        ),
        (
            "a phase gate",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.periods_ns = vec![secs(10.0)];
                s.synapses[0].gate = Gate::Phase {
                    rhythm: 0,
                    from: 0.0,
                    to: 1.0,
                };
            }),
        ),
        (
            "a retiring latch",
            Box::new(|s: &mut MediumSpec| {
                let l = s
                    .cells
                    .iter_mut()
                    .find(|c| c.archetype == Archetype::Latch)
                    .unwrap();
                l.params[2] = 1.0;
            }),
        ),
        (
            "a binned coincidence",
            Box::new(|s: &mut MediumSpec| {
                s.oscillome.tick_len_ns = len;
                s.oscillome.periods_ns = vec![secs(10.0)];
                let c = s
                    .cells
                    .iter_mut()
                    .find(|c| c.archetype == Archetype::Coincidence)
                    .unwrap();
                c.params[4] = 1.0;
                c.params[6] = 10.0;
            }),
        ),
        (
            "an oscillator",
            Box::new(|s: &mut MediumSpec| {
                let id = CellId(s.cells.len() as u32);
                s.cells.push(gordian_medium::CellSpec {
                    archetype: Archetype::Oscillator,
                    params: [1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    pattern: None,
                });
                s.synapses.push(SynapseSpec {
                    from: id,
                    to: id,
                    weight: 1.0,
                    delay_ticks: 3,
                    gate: Gate::None,
                    plastic: false,
                });
            }),
        ),
    ];
    for (name, switch) in &switches {
        let mut on = base.clone();
        switch(&mut on);
        assert!(on.uses_oscillome(), "{name}");
        on.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let bytes = run(&on);
        assert!(bytes.iter().all(|b| b[4] == 2), "{name}");
    }
    // All off again: M1's bytes.
    let mut off = base.clone();
    off.oscillome = Oscillome::default();
    assert_eq!(run(&off), m1);
}

/// An M1 spec's JSON has no `oscillome` key and reads back to the same spec; an oscillome spec
/// round-trips through JSON and through the medium's `spec()`, and resolving it twice changes
/// nothing.
#[test]
fn specs_round_trip_with_and_without_the_oscillome() {
    let m1 = rich_spec(Limits::default());
    let json = serde_json::to_string(&m1).unwrap();
    assert!(!json.contains("oscillome"));
    assert_eq!(serde_json::from_str::<MediumSpec>(&json).unwrap(), m1);

    let spec = oscillome_spec(Limits::default(), 500 * MS);
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(serde_json::from_str::<MediumSpec>(&json).unwrap(), spec);
    let (resolved, conversions) = spec.resolved().unwrap();
    assert_eq!(resolved.resolved().unwrap().0, resolved);
    assert_eq!(conversions.len(), spec.oscillome.seconds.len());
    let built = Medium::from_spec(&spec).unwrap();
    assert_eq!(built.spec(), resolved);
    assert_eq!(
        Medium::from_spec(&built.spec()).unwrap().to_bytes(),
        built.to_bytes()
    );
}

#[test]
fn a_clock_of_another_tick_length_is_refused_and_changes_nothing() {
    let spec = oscillome_spec(Limits::default(), 500 * MS);
    let mut m = Medium::from_spec(&spec).unwrap();
    let before = m.to_bytes();
    let mut rig = Rig::with_tick(0, 100 * MS, vec![ev(0, 0, 0, 0, 1.0, 0)]);
    assert_eq!(
        rig.step(&mut m),
        Err(StepError::TickLength {
            spec: 500 * MS,
            clock: 100 * MS
        })
    );
    assert_eq!(m.to_bytes(), before);
}

#[test]
fn malformed_oscillome_specs_are_refused() {
    let ok = oscillome_spec(Limits::default(), 500 * MS);
    ok.validate().unwrap();
    let refuse = |f: &dyn Fn(&mut MediumSpec)| {
        let mut s = ok.clone();
        f(&mut s);
        assert!(s.validate().is_err());
        assert!(Medium::from_spec(&s).is_err());
    };
    refuse(&|s| s.oscillome.periods_ns[0] = 500 * MS); // not slower than the tick
    refuse(&|s| s.oscillome.periods_ns = vec![secs(1.0); 4]); // more than R
    refuse(&|s| s.oscillome.tick_len_ns = 0); // rhythms without a tick
    refuse(&|s| s.oscillome.tick_len_ns = u64::from(u32::MAX) + 1);
    refuse(&|s| s.oscillome.cycle_summary = false); // plasticity needs it
    refuse(&|s| s.oscillome.trace_rhythm = Some(2));
    refuse(&|s| {
        let t = s.oscillome.seconds[0];
        s.oscillome.seconds.push(t); // a target named twice
    });
    refuse(&|s| {
        s.oscillome.seconds.push(gordian_medium::Timed {
            target: TimeTarget::Param {
                cell: CellId(0),
                index: 0,
            },
            ns: 1,
        }) // a sense cell's parameter is not a time
    });
    refuse(&|s| {
        s.oscillome.seconds.push(gordian_medium::Timed {
            target: TimeTarget::Delay {
                synapse: gordian_medium::SynapseId(0),
            },
            ns: secs(200.0), // 400 ticks
        })
    });
    // A phase gate on a missing rhythm, or with an empty window.
    refuse(&|s| {
        let g = s
            .synapses
            .iter_mut()
            .find(|x| matches!(x.gate, Gate::Phase { .. }));
        g.unwrap().gate = Gate::Phase {
            rhythm: 2,
            from: 0.0,
            to: 0.5,
        };
    });
    refuse(&|s| {
        let g = s
            .synapses
            .iter_mut()
            .find(|x| matches!(x.gate, Gate::Phase { .. }));
        g.unwrap().gate = Gate::Phase {
            rhythm: 0,
            from: 0.5,
            to: 0.5,
        };
    });
    // An oscillator without its self-synapse, or one that never stops.
    refuse(&|s| {
        let osc = s
            .cells
            .iter()
            .position(|c| c.archetype == Archetype::Oscillator)
            .unwrap() as u32;
        s.synapses
            .retain(|x| !(x.from == CellId(osc) && x.to == CellId(osc)));
        s.oscillome
            .seconds
            .retain(|t| !matches!(t.target, TimeTarget::Delay { .. }));
    });
    refuse(&|s| {
        let osc = s
            .cells
            .iter_mut()
            .find(|c| c.archetype == Archetype::Oscillator)
            .unwrap();
        osc.params[2] = 0.0; // no cycle limit
        osc.params[1] = 0.0; // and no floor
    });
    // A binned coincidence with bins shorter than the tick.
    refuse(&|s| {
        let c = s
            .cells
            .iter_mut()
            .find(|c| c.archetype == Archetype::Coincidence && c.params[4] == 1.0)
            .unwrap();
        c.params[6] = 1_000.0; // 100 ms bins on a 500 ms tick
    });
    // An ordered coincidence on a tick that is not a whole number of microseconds.
    refuse(&|s| {
        s.oscillome.tick_len_ns = 500 * MS + 1;
        s.oscillome.seconds.clear();
    });
}

// ---------------------------------------------------------------------------------------------
// Decision 2: the conversion table per tick length.

/// The reference durations, from W1 (`experiments/exploration/w1-tick-and-measures.md`): a
/// burst partner's alarm (20, 60, 150 ms), a tick-scale delay (0.5 s), the incident horizon (6,
/// 10, 16 s), and retention (100, 200 s). Each as a synapse delay (nearest, at least one tick),
/// a lookback (up), and a time constant (per-tick decay).
#[test]
fn the_conversion_table_per_tick_length() {
    let durations_ms: [u64; 9] = [20, 60, 150, 500, 6_000, 10_000, 16_000, 100_000, 200_000];
    // (tick ms, delays in ticks, lookbacks in ticks). A synapse delay is a u8: 1000 and 2000
    // ticks, and 400 ticks, do not fit, and the spec is refused (checked below).
    let expected: [(u64, [u64; 9], [u64; 9]); 3] = [
        (
            100,
            [1, 1, 2, 5, 60, 100, 160, 1000, 2000],
            [1, 1, 2, 5, 60, 100, 160, 1000, 2000],
        ),
        (
            500,
            [1, 1, 1, 1, 12, 20, 32, 200, 400],
            [1, 1, 1, 1, 12, 20, 32, 200, 400],
        ),
        (
            2_000,
            [1, 1, 1, 1, 3, 5, 8, 50, 100],
            [1, 1, 1, 1, 3, 5, 8, 50, 100],
        ),
    ];
    for (tick_ms, delays, lookbacks) in expected {
        let len = tick_ms * MS;
        let mut b = MediumBuilder::new().oscillome(Oscillome {
            tick_len_ns: len,
            ..Oscillome::default()
        });
        let s = b.sense(at(0, 0), SenseMode::Presence);
        let mut rows = Vec::new();
        let mut too_long = None;
        for &d in &durations_ms {
            let i = b.integrator(0.5, 1.0, true, 0);
            let syn = b.synapse(s, i, 1.0, 1);
            if delay_ticks(d * MS, len) <= 255 {
                b.timed(TimeTarget::Delay { synapse: syn }, d * MS);
            } else {
                too_long.get_or_insert(syn);
            }
            b.timed(TimeTarget::Param { cell: i, index: 3 }, d * MS);
            b.timed(TimeTarget::Param { cell: i, index: 0 }, d * MS);
            rows.push(d);
        }
        let spec = b.into_spec();
        if let Some(syn) = too_long {
            let mut refused = spec.clone();
            refused.oscillome.seconds.push(gordian_medium::Timed {
                target: TimeTarget::Delay { synapse: syn },
                ns: 200_000 * MS,
            });
            assert!(matches!(
                refused.validate(),
                Err(gordian_medium::SpecError::BadTimed {
                    reason: "delay over 255 ticks",
                    ..
                })
            ));
        }
        let conversions = spec.conversions().unwrap();
        let mut conversions = conversions.into_iter();
        let mut got_delays = Vec::new();
        let mut got_lookbacks = Vec::new();
        let mut collapsed = Vec::new();
        eprintln!("tick {tick_ms} ms");
        for d in &rows {
            let delay = if delay_ticks(d * MS, len) <= 255 {
                let c = conversions.next().unwrap();
                assert_eq!(c.kind, TimeKind::Delay);
                if c.collapsed_to_one_tick() {
                    collapsed.push(*d);
                }
                Some(c.value as u64)
            } else {
                None
            };
            let (lookback, decay) = (conversions.next().unwrap(), conversions.next().unwrap());
            assert_eq!(
                (lookback.kind, decay.kind),
                (TimeKind::Span, TimeKind::Decay)
            );
            got_delays.push(delay.unwrap_or(delay_ticks(d * MS, len)));
            got_lookbacks.push(lookback.value as u64);
            eprintln!(
                "  {d} ms: exact {:.3} ticks, delay {}, lookback {} ticks, decay per tick for \
                 tau = {d} ms: {:e} (bits {:#010x})",
                lookback.exact_ticks(),
                delay.map_or("over 255 ticks: refused".to_string(), |x| format!(
                    "{x} ticks"
                )),
                lookback.value,
                decay.value,
                decay.value.to_bits()
            );
        }
        assert_eq!(got_delays, delays, "tick {tick_ms} ms");
        assert_eq!(got_lookbacks, lookbacks, "tick {tick_ms} ms");
        let expected_collapsed: Vec<u64> = durations_ms
            .iter()
            .copied()
            .filter(|d| 2 * d * MS < 3 * len)
            .collect();
        assert_eq!(collapsed, expected_collapsed, "tick {tick_ms} ms");
        eprintln!("  delays that collapse to one tick: {collapsed:?} ms");
        // The built medium carries the converted values.
        let m = Medium::from_spec(&spec).unwrap();
        assert_eq!(m.synapses()[0].delay_ticks as u64, delays[0]);
    }
    // The rounding rules at their edges.
    assert_eq!(delay_ticks(149 * MS, 100 * MS), 1);
    assert_eq!(delay_ticks(150 * MS, 100 * MS), 2);
    assert_eq!(span_ticks(101 * MS, 100 * MS), 2);
    // The rhythms per tick length: 10 s and 100 s are whole multiples of every swept tick.
    for (tick_ms, per) in [(100u64, (100, 1000)), (500, (20, 200)), (2_000, (5, 50))] {
        let e = gordian_medium::OscillomeEngine::new(&Oscillome {
            tick_len_ns: tick_ms * MS,
            periods_ns: RHYTHMS_NS.to_vec(),
            ..Oscillome::default()
        });
        assert_eq!(e.ticks_per_cycle(0), (per.0, per.0));
        assert_eq!(e.ticks_per_cycle(1), (per.1, per.1));
        eprintln!(
            "tick {tick_ms} ms: 10 s = {} ticks, 100 s = {} ticks per cycle",
            per.0, per.1
        );
    }
}

#[test]
fn time_kinds_follow_the_parameter_and_the_mode() {
    let mut p = [0.0f32; 8];
    assert_eq!(
        time_kind(Archetype::Coincidence, 1, &p),
        Some(TimeKind::Span)
    );
    p[4] = 1.0;
    assert_eq!(time_kind(Archetype::Coincidence, 1, &p), None, "bins");
    p[4] = 2.0;
    assert_eq!(
        time_kind(Archetype::Coincidence, 1, &p),
        Some(TimeKind::Micros)
    );
    assert_eq!(
        time_kind(Archetype::Novelty, 0, &p),
        None,
        "gaps ignored: per run"
    );
    let mut g = [0.0f32; 8];
    g[4] = 1.0;
    assert_eq!(time_kind(Archetype::Novelty, 0, &g), Some(TimeKind::Rate));
    assert_eq!(time_kind(Archetype::Latch, 1, &p), Some(TimeKind::Span));
    assert_eq!(time_kind(Archetype::Oscillator, 1, &p), None);
    assert_eq!(time_kind(Archetype::Sense, 0, &p), None);
    // Bins are counted from tick 0 of the world's clock, like phases.
    assert_eq!(bin_index(19, 500 * MS, secs(10.0), 10), 9);
}

// ---------------------------------------------------------------------------------------------
// Determinism and counting with the oscillome on (M1's section 10 tests, repeated).

#[test]
fn oscillome_runs_are_deterministic_order_independent_and_counted() {
    for len in [100 * MS, 500 * MS, 2_000 * MS] {
        let spec = oscillome_spec(Limits::default(), len);
        let events = random_events_in(29, 0, 300, 30, len);
        let a = run_bytes(&spec, len, &events, 300);
        assert_eq!(a, run_bytes(&spec, len, &events, 300));
        // Each tick's events reversed: the same bytes.
        let mut reversed = Vec::new();
        for t in 0..300 {
            let mut tick: Vec<Event> = events.iter().filter(|e| e.tick == t).cloned().collect();
            tick.reverse();
            reversed.extend(tick);
        }
        assert_eq!(run_bytes(&spec, len, &reversed, 300), a, "len {len}");
        // Counts equal a recount from the trace, with phase reads and binned reads.
        let mut m = Medium::from_spec(&spec).unwrap();
        let mut rig = Rig::with_tick(0, len, events.clone());
        let mut field_reads = 0;
        for _ in 0..300 {
            let s = rig.step(&mut m).unwrap();
            let trace = rig.trace.traces.last().unwrap();
            let mut c = OpCounts::default();
            for item in &trace.items {
                match item {
                    TraceItem::Routed { .. } | TraceItem::Unmatched { .. } => c.event_routings += 1,
                    TraceItem::Ran { field_reads, .. } => {
                        c.cell_updates += 1;
                        c.field_reads += field_reads;
                    }
                    TraceItem::Traversed { field_read, .. } => {
                        c.synapse_traversals += 1;
                        c.field_reads += u64::from(*field_read);
                    }
                    TraceItem::Proposed { .. } => c.proposals += 1,
                    _ => {}
                }
            }
            assert_eq!(c, s.counts, "len {len}, tick {}", s.tick);
            field_reads += s.counts.field_reads;
        }
        assert!(field_reads > 0, "len {len}: phase reads happened");
        let kinds: std::collections::BTreeSet<u16> =
            rig.effector.proposals.iter().map(|(_, p)| p.kind).collect();
        eprintln!("len {len}: proposal kinds {kinds:?}");
        assert!(kinds.contains(&NOTICE), "len {len}: {kinds:?}");
        let _ = (ORDERED, ABNORMAL);
    }
}

#[test]
fn the_ordered_spec_kind_appears_at_every_tick_length() {
    // A scripted burst: node 0 channel 0 at 10 ms, node 1 channel 0 at 60 ms after it, in the
    // same tick at 500 ms and 2 s and in the next tick at... 100 ms only when it crosses; here
    // both inside tick 3.
    for len in [100 * MS, 500 * MS, 2_000 * MS] {
        let spec = oscillome_spec(Limits::default(), len);
        let mut m = Medium::from_spec(&spec).unwrap();
        let events = vec![
            ev(3, 10_000_000, 0, 0, 1.0, 0),
            ev(3, 70_000_000, 1, 0, 1.0, 1),
        ];
        let mut rig = Rig::with_tick(0, len, events);
        for _ in 0..6 {
            rig.step(&mut m).unwrap();
        }
        let ordered: Vec<_> = rig
            .effector
            .proposals
            .iter()
            .filter(|(_, p)| p.kind == ORDERED)
            .map(|(_, p)| p.anchor)
            .collect();
        assert_eq!(ordered, vec![r(3, 10_000_000, 0)], "len {len}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any oscillome spec within the limits runs any events without panicking, within its
    /// limits, and its bytes restore exactly mid-run.
    #[test]
    fn no_panic_with_the_oscillome_and_bytes_restore(
        tick_ms in prop::sample::select(vec![100u64, 300, 500, 2_000]),
        max_ops in 1u64..400,
        max_props in 0u32..4,
        seed in any::<u64>(),
        k in 0u64..60,
    ) {
        let len = tick_ms * MS;
        let limits = Limits {
            max_ops_per_tick: max_ops,
            max_proposals_per_tick: max_props,
            ..Limits::default()
        };
        let spec = oscillome_spec(limits, len);
        let events = random_events_in(seed, 0, 80, 12, len);
        let mut m = Medium::from_spec(&spec).unwrap();
        let mut rig = Rig::with_tick(0, len, events);
        rig.plasticity.scale = Some(0.99);
        for t in 0..80 {
            if t == k {
                let restored = Medium::from_bytes(&m.to_bytes()).unwrap();
                prop_assert_eq!(restored.to_bytes(), m.to_bytes());
                m = restored;
            }
            let s = rig.step(&mut m).unwrap();
            prop_assert!(s.counts.ops() <= max_ops);
            prop_assert!(s.counts.proposals <= u64::from(max_props));
            for c in m.cells() {
                prop_assert!(c.activation.is_finite());
                prop_assert!(c.state.iter().all(|x| x.is_finite()));
            }
        }
    }
}

/// The version-2 bytes refuse corruption without panicking, and a stored conversion that no
/// longer matches its quantity in time is refused, not repaired.
#[test]
fn oscillome_bytes_reject_corruption_without_panicking() {
    let len = 500 * MS;
    let spec = oscillome_spec(Limits::default(), len);
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::with_tick(0, len, random_events_in(9, 0, 50, 6, len));
    for _ in 0..50 {
        rig.step(&mut m).unwrap();
    }
    let bytes = m.to_bytes();
    assert_eq!(Medium::from_bytes(&bytes).unwrap().to_bytes(), bytes);
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        if let Ok(decoded) = Medium::from_bytes(&b) {
            // Whatever decodes re-encodes to exactly the bytes it came from.
            assert_eq!(decoded.to_bytes(), b, "byte {i}");
        }
    }
    // The oscillator's period is given as 6 s (12 ticks). Claim 7 s instead: the stored delay of
    // 12 ticks is no longer the conversion (14), and the bytes are refused.
    let six = secs(6.0).to_le_bytes();
    let at = bytes
        .windows(8)
        .rposition(|w| w == six)
        .expect("the 6 s entry is in the oscillome section");
    let mut b = bytes.clone();
    b[at..at + 8].copy_from_slice(&secs(7.0).to_le_bytes());
    assert!(matches!(
        Medium::from_bytes(&b),
        Err(gordian_medium::DecodeError::Inconsistent(
            "a stored value differs from its conversion"
        ))
    ));
    // Version 1 with an oscillome section, or version 2 without one, is refused.
    let mut b = bytes.clone();
    b[4] = 1;
    assert!(Medium::from_bytes(&b).is_err());
    let m1 = Medium::from_spec(&rich_spec(Limits::default())).unwrap();
    let mut b = m1.to_bytes();
    b[4] = 2;
    assert!(Medium::from_bytes(&b).is_err());
}

#[test]
fn prices_in_specs_are_the_declared_ones_unless_stated() {
    assert_eq!(MediumBuilder::new().into_spec().prices, Prices::DECLARED);
    assert_eq!(Prices::DECLARED.cell_update_ps, 200_000);
}
