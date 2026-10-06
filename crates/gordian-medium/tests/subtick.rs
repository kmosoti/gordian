//! Sub-tick support (work item M3): the sub-tick lookback that cuts a firing cell's support by
//! event time inside the tick, and the ordered coincidence's arrivals at event resolution. Each
//! has a hand-worked example; both are off by default, and with them off the bytes are M1b's
//! (`tests/m1b_identity.rs`).

mod common;

use common::{Rig, TestGen, at, ev};
use gordian_medium::{
    Archetype, CellId, Event, Limits, Medium, MediumBuilder, MediumSpec, Oscillome, SenseMode,
    SpecError, TimeKind, TimeTarget, secs,
};

/// Three passes: sense, then the accumulating cell, then the emitter, in one tick.
fn builder(len: u64) -> MediumBuilder {
    MediumBuilder::new()
        .limits(Limits {
            max_passes: 3,
            ..Limits::default()
        })
        .oscillome(Oscillome {
            tick_len_ns: len,
            ..Oscillome::default()
        })
}

const MS: u32 = 1_000_000;

/// The burst graph: three sense cells (nodes 0, 1, 2 on channel 0, counting) into an ordered
/// coincidence of two within `window_us`, consumed, with a tick lookback of `lookback` ticks,
/// a sub-tick lookback of `sub_us` (0: off) and arrivals at event resolution when `every`; then
/// an emitter that cites everything it is sent. On a tick of `len` ns.
fn burst_spec(len: u64, window_us: u32, lookback: u32, sub_us: u32, every: bool) -> MediumSpec {
    let mut b = builder(len);
    let senses: Vec<CellId> = (0..3)
        .map(|node| b.sense(at(node, 0), SenseMode::Count))
        .collect();
    let c = b.coincidence_ordered(2, window_us, false, true, lookback);
    b.set_param(c, 7, sub_us as f32);
    b.set_param(c, 6, if every { 1.0 } else { 0.0 });
    let e = b.emit(1.0, 1, 1_000, 0);
    for s in senses {
        b.synapse(s, c, 1.0, 0);
    }
    b.synapse(c, e, 1.0, 0);
    b.into_spec()
}

/// (tick, anchor seq, refs' seqs) of every proposal over `ticks` ticks.
fn proposals(
    spec: &MediumSpec,
    len: u64,
    events: Vec<Event>,
    ticks: u64,
) -> Vec<(u64, u32, Vec<u32>)> {
    let mut m = Medium::from_spec(spec).unwrap();
    let mut rig = Rig::with_tick(0, len, events);
    for _ in 0..ticks {
        rig.step(&mut m).unwrap();
    }
    rig.effector
        .proposals
        .iter()
        .map(|(t, p)| (*t, p.anchor.seq, p.refs.iter().map(|r| r.seq).collect()))
        .collect()
}

#[test]
fn a_sub_tick_lookback_keeps_a_stray_that_shares_the_bursts_tick_out_of_its_support() {
    let len = secs(2.0);
    // A stray at node 2 at 100 ms; the burst, nodes 0 and 1, at 1,500 and 1,510 ms; all in tick 0.
    let events = vec![
        ev(0, 100 * MS, 2, 0, 1.0, 0),
        ev(0, 1_500 * MS, 0, 0, 1.0, 1),
        ev(0, 1_510 * MS, 1, 0, 1.0, 2),
    ];
    // Off (M1b): the support is the whole tick, and the stray is the anchor.
    let off = proposals(
        &burst_spec(len, 20_000, 0, 0, false),
        len,
        events.clone(),
        2,
    );
    assert_eq!(off, vec![(0, 0, vec![0, 1, 2])]);
    // A sub-tick lookback of 20 ms before the firing instant (1,510 ms): the burst only.
    let on = proposals(
        &burst_spec(len, 20_000, 0, 20_000, false),
        len,
        events.clone(),
        2,
    );
    assert_eq!(on, vec![(0, 1, vec![1, 2])]);
    // 10 ms reaches the burst's first event exactly (1,510 - 10 = 1,500, inclusive).
    let edge = proposals(
        &burst_spec(len, 20_000, 0, 10_000, false),
        len,
        events.clone(),
        2,
    );
    assert_eq!(edge, vec![(0, 1, vec![1, 2])]);
    // 9,999 us does not: the anchor moves to the second event.
    let short = proposals(
        &burst_spec(len, 20_000, 0, 9_999, false),
        len,
        events.clone(),
        2,
    );
    assert_eq!(short, vec![(0, 2, vec![2])]);
    // A lookback reaching back to the stray (1,410 ms) keeps it.
    let long = proposals(
        &burst_spec(len, 20_000, 0, 1_410_000, false),
        len,
        events,
        2,
    );
    assert_eq!(long, vec![(0, 0, vec![0, 1, 2])]);
}

#[test]
fn the_sub_tick_lookback_reaches_across_a_tick_edge_by_event_time() {
    let len = secs(0.5);
    // Node 0 at 495 ms (tick 0), node 1 at 505 ms (tick 1, offset 5 ms): 10 ms apart.
    let events = vec![ev(0, 495 * MS, 0, 0, 1.0, 0), ev(1, 5 * MS, 1, 0, 1.0, 0)];
    // Tick lookback 0 (M2's frozen value): the support is tick 1's only.
    let ticks0 = proposals(
        &burst_spec(len, 20_000, 0, 0, false),
        len,
        events.clone(),
        3,
    );
    assert_eq!(ticks0, vec![(1, 0, vec![0])]);
    // Tick lookback 1 and a sub-tick lookback of 20 ms: the event in tick 0 is 10 ms before
    // the firing instant and is kept; the floor is no longer the tick.
    let spec = burst_spec(len, 20_000, 1, 20_000, false);
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::with_tick(0, len, events.clone());
    for _ in 0..3 {
        rig.step(&mut m).unwrap();
    }
    let p = &rig.effector.proposals;
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].1.anchor.tick, p[0].1.anchor.offset_ns), (0, 495 * MS));
    // A sub-tick lookback of 5 ms cuts it.
    let mut m = Medium::from_spec(&burst_spec(len, 20_000, 1, 5_000, false)).unwrap();
    let mut rig = Rig::with_tick(0, len, events);
    for _ in 0..3 {
        rig.step(&mut m).unwrap();
    }
    let p = &rig.effector.proposals;
    assert_eq!((p[0].1.anchor.tick, p[0].1.anchor.offset_ns), (1, 5 * MS));
}

#[test]
fn an_integrator_cuts_its_support_before_its_newest_event_when_it_fires() {
    let len = secs(2.0);
    let spec = |sub_us: u32| {
        let mut b = builder(len);
        let s = b.sense(at(0, 0), SenseMode::Count);
        let i = b.integrator(1.0, 3.0, true, 0);
        b.set_param(i, 7, sub_us as f32);
        let e = b.emit(1.0, 1, 1_000, 0);
        b.synapse(s, i, 1.0, 0);
        b.synapse(i, e, 1.0, 0);
        b.into_spec()
    };
    let events = vec![
        ev(0, 0, 0, 0, 1.0, 0),
        ev(0, 900 * MS, 0, 0, 1.0, 1),
        ev(0, 950 * MS, 0, 0, 1.0, 2),
        ev(0, 960 * MS, 0, 0, 1.0, 3),
    ];
    assert_eq!(
        proposals(&spec(0), len, events.clone(), 1),
        vec![(0, 0, vec![0, 1, 2, 3])]
    );
    // 100 ms before the newest (960 ms): from 860 ms.
    assert_eq!(
        proposals(&spec(100_000), len, events.clone(), 1),
        vec![(0, 1, vec![1, 2, 3])]
    );
    // A run that does not fire keeps its support uncut: the first two events (tick 0) are cited
    // when the third, in tick 1, fires it with a tick lookback of 1.
    let spec1 = {
        let mut s = spec(10_000);
        s.cells[1].params[3] = 1.0;
        s
    };
    let split = vec![
        ev(0, 1_000 * MS, 0, 0, 1.0, 0),
        ev(0, 1_995 * MS, 0, 0, 1.0, 1),
        ev(1, 3 * MS, 0, 0, 1.0, 0),
    ];
    // Fired at tick 1, 3 ms; the floor is 10 ms before, 1,993 ms of tick 0: the event at 1,000
    // ms is cut, the one at 1,995 ms kept.
    let p = proposals(&spec1, len, split, 2);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].0, 1);
    assert_eq!(p[0].2.len(), 2, "{p:?}");
}

#[test]
fn arrivals_at_event_resolution_see_a_burst_behind_a_stray_of_the_same_kind() {
    let len = secs(2.0);
    // A stray at node 0 at 100 ms; the burst at nodes 0 and 1, at 1,500 and 1,505 ms.
    let events = vec![
        ev(0, 100 * MS, 0, 0, 1.0, 0),
        ev(0, 1_500 * MS, 0, 0, 1.0, 1),
        ev(0, 1_505 * MS, 1, 0, 1.0, 2),
    ];
    // M1b's arrivals: node 0's message stands at its earliest event (100 ms), 1,405 ms before
    // node 1's: no burst.
    assert!(
        proposals(
            &burst_spec(len, 20_000, 0, 20_000, false),
            len,
            events.clone(),
            2
        )
        .is_empty()
    );
    // Every event cited is an arrival: the burst fires at 1,505 ms, cut to the burst.
    assert_eq!(
        proposals(
            &burst_spec(len, 20_000, 0, 20_000, true),
            len,
            events.clone(),
            2
        ),
        vec![(0, 1, vec![1, 2])]
    );
    // Without the sub-tick lookback the stray is still cited (and is the anchor): the two
    // mechanisms are separate switches.
    assert_eq!(
        proposals(&burst_spec(len, 20_000, 0, 0, true), len, events, 2),
        vec![(0, 0, vec![0, 1, 2])]
    );
}

#[test]
fn arrivals_at_event_resolution_see_a_burst_that_a_later_event_would_hide() {
    // In one 100 ms tick: nodes 0 and 1 at 10 and 20 ms, node 2 at 90 ms. M1b's rule counts the
    // slots within the window of the newest (90 ms) only: one. The scan finds the window ending
    // at 20 ms.
    let len = secs(0.1);
    let events = vec![
        ev(0, 10 * MS, 0, 0, 1.0, 0),
        ev(0, 20 * MS, 1, 0, 1.0, 1),
        ev(0, 90 * MS, 2, 0, 1.0, 2),
    ];
    assert!(
        proposals(
            &burst_spec(len, 20_000, 0, 0, false),
            len,
            events.clone(),
            2
        )
        .is_empty()
    );
    let p = proposals(&burst_spec(len, 20_000, 0, 20_000, true), len, events, 2);
    // Fired at 20 ms; the floor is 0 ms: events 0 and 1, and the later event 2 is kept (the cut
    // is a lower edge).
    assert_eq!(p, vec![(0, 0, vec![0, 1, 2])]);
    // A sustained source does not hide its own burst either: node 0 at 10 and 900 ms, node 1 at
    // 20 ms, in a 2 s tick.
    let len = secs(2.0);
    let sustained = vec![
        ev(0, 10 * MS, 0, 0, 1.0, 0),
        ev(0, 20 * MS, 1, 0, 1.0, 1),
        ev(0, 900 * MS, 0, 0, 1.0, 2),
    ];
    for every in [false, true] {
        let p = proposals(
            &burst_spec(len, 20_000, 0, 20_000, every),
            len,
            sustained.clone(),
            1,
        );
        assert_eq!(p.len(), 1, "every {every}");
        assert_eq!(p[0].1, 0, "every {every}");
    }
}

#[test]
fn arrivals_at_event_resolution_respect_the_lead_and_keep_the_latest_candidate() {
    let len = secs(2.0);
    let spec = |lead: bool| {
        let mut b = builder(len);
        let s0 = b.sense(at(0, 0), SenseMode::Count);
        let s1 = b.sense(at(1, 0), SenseMode::Count);
        let c = b.coincidence_ordered(2, 50_000, lead, true, 1);
        b.set_param(c, 6, 1.0);
        let e = b.emit(1.0, 1, 1_000, 0);
        b.synapse(s0, c, 1.0, 0);
        b.synapse(s1, c, 1.0, 0);
        b.synapse(c, e, 1.0, 0);
        b.into_spec()
    };
    // Node 1 at 100 ms, then node 0 at 120 ms: within 50 ms, but node 0 is not first.
    let reversed = vec![ev(0, 100 * MS, 1, 0, 1.0, 0), ev(0, 120 * MS, 0, 0, 1.0, 1)];
    assert!(proposals(&spec(true), len, reversed.clone(), 2).is_empty());
    assert_eq!(proposals(&spec(false), len, reversed, 2).len(), 1);
    // Node 0 at 100 ms, node 1 at 120 ms and node 0 again at 130 ms: node 0's earliest in the
    // window ending at 120 ms is 100 ms, first: fires.
    let led = vec![
        ev(0, 100 * MS, 0, 0, 1.0, 0),
        ev(0, 120 * MS, 1, 0, 1.0, 1),
        ev(0, 130 * MS, 0, 0, 1.0, 2),
    ];
    assert_eq!(proposals(&spec(true), len, led, 2).len(), 1);
    // A slot keeps its latest candidate across a tick: node 0 at 100 ms and 1,980 ms in tick 0,
    // node 1 at 10 ms of tick 1 (30 ms after node 0's latest).
    let carried = vec![
        ev(0, 100 * MS, 0, 0, 1.0, 0),
        ev(0, 1_980 * MS, 0, 0, 1.0, 1),
        ev(1, 10 * MS, 1, 0, 1.0, 0),
    ];
    let p = proposals(&spec(true), len, carried, 3);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].0, 1);
}

#[test]
fn sub_tick_specs_are_validated_and_given_in_time() {
    let len = secs(0.5);
    // Without a tick length, a sub-tick lookback is refused (event times need it).
    let mut b = MediumBuilder::new();
    let i = b.integrator(0.5, 1.0, true, 0);
    b.set_param(i, 7, 10_000.0);
    assert_eq!(
        b.spec().validate(),
        Err(SpecError::BadForm {
            cell: i,
            reason: "a sub-tick lookback needs the oscillome's tick length"
        })
    );
    b.set_param(i, 7, 0.0);
    b.spec().validate().unwrap();
    // Not an integer; arrivals flag not 0 or 1.
    let mut s = burst_spec(len, 20_000, 0, 0, false);
    s.cells[3].params[7] = 0.5;
    assert!(matches!(s.validate(), Err(SpecError::BadParam { .. })));
    let mut s = burst_spec(len, 20_000, 0, 0, false);
    s.cells[3].params[6] = 2.0;
    assert!(matches!(s.validate(), Err(SpecError::BadParam { .. })));
    // An integrator's parameter 7 is checked too.
    let mut b = MediumBuilder::new().oscillome(Oscillome {
        tick_len_ns: len,
        ..Oscillome::default()
    });
    let i = b.integrator(0.5, 1.0, true, 0);
    b.set_param(i, 7, -1.0);
    assert!(b.build().is_err());
    // Given in time: microseconds, rounded up, the same at every tick length.
    for tick in [secs(0.1), secs(0.5), secs(2.0)] {
        let mut b = MediumBuilder::new().oscillome(Oscillome {
            tick_len_ns: tick,
            ..Oscillome::default()
        });
        let c = b.coincidence_ordered(2, 20_000, false, true, 0);
        let i = b.integrator(0.5, 1.0, true, 0);
        b.timed(TimeTarget::Param { cell: c, index: 7 }, 20_000_500);
        b.timed(TimeTarget::Param { cell: i, index: 7 }, secs(0.25));
        let (spec, conv) = b.into_spec().resolved().unwrap();
        assert_eq!(spec.cells[0].params[7], 20_001.0);
        assert_eq!(spec.cells[1].params[7], 250_000.0);
        assert!(conv.iter().all(|c| c.kind == TimeKind::Micros));
    }
    // A sliding coincidence takes one too, and then uses the oscillome (version 2 bytes).
    let mut b = MediumBuilder::new().oscillome(Oscillome {
        tick_len_ns: len,
        ..Oscillome::default()
    });
    let c = b.coincidence(2, 1, true, 1);
    b.set_param(c, 7, 30_000.0);
    let spec = b.into_spec();
    assert!(spec.uses_oscillome());
    assert_eq!(spec.cells[0].archetype, Archetype::Coincidence);
    spec.validate().unwrap();
}

/// Random bursts (two or three events within 40 ms) and strays at three nodes on a tick of
/// `len`.
fn random_burst_events(seed: u64, ticks: u64, len: u64) -> Vec<Event> {
    let mut g = TestGen(seed);
    let mut out = Vec::new();
    for t in 0..ticks {
        let mut offs: Vec<u64> = Vec::new();
        for _ in 0..g.below(3) {
            let centre = g.below(len);
            for _ in 0..1 + g.below(3) {
                offs.push((centre + g.below(40_000_000)).min(len - 1));
            }
        }
        offs.sort_unstable();
        for (seq, off) in offs.into_iter().enumerate() {
            out.push(ev(t, off as u32, g.below(3) as u16, 0, 1.0, seq as u32));
        }
    }
    out
}

#[test]
fn sub_tick_runs_are_deterministic_restorable_and_order_independent() {
    for len in [secs(0.1), secs(0.5), secs(2.0)] {
        for every in [false, true] {
            let spec = burst_spec(len, 25_000, 2, 30_000, every);
            let events = random_burst_events(len ^ u64::from(every), 200, len);
            let run = |events: Vec<Event>, restore_at: Option<u64>| {
                let mut m = Medium::from_spec(&spec).unwrap();
                let mut rig = Rig::with_tick(0, len, events);
                let mut bytes = Vec::new();
                for t in 0..200 {
                    rig.step(&mut m).unwrap();
                    if restore_at == Some(t) {
                        m = Medium::from_bytes(&m.to_bytes()).unwrap();
                    }
                    bytes.push(m.to_bytes());
                }
                (
                    bytes,
                    format!("{:?}", rig.effector.proposals),
                    rig.effector.proposals.len(),
                )
            };
            let a = run(events.clone(), None);
            assert!(a.2 > 10, "proposals made: {}", a.2);
            assert_eq!(a, run(events.clone(), None));
            assert_eq!(a, run(events.clone(), Some(77)), "restored mid-run");
            // The order in which the sense port hands a tick's events over does not matter.
            let mut shuffled = events;
            shuffled.reverse();
            shuffled.sort_by_key(|e| e.tick);
            assert_eq!(a, run(shuffled, None));
        }
    }
}

#[test]
fn the_sub_tick_cut_adds_no_counted_operation() {
    let len = secs(2.0);
    let events = random_burst_events(9, 100, len);
    let counts = |sub: u32| {
        let mut m = Medium::from_spec(&burst_spec(len, 25_000, 1, sub, false)).unwrap();
        let mut rig = Rig::with_tick(0, len, events.clone());
        for _ in 0..100 {
            rig.step(&mut m).unwrap();
        }
        (rig.ledger.totals, rig.effector.proposals.len())
    };
    let (off, n_off) = counts(0);
    let (on, n_on) = counts(10_000);
    // The same runs happen; the cut changes what is cited, not what is done.
    assert_eq!(off, on);
    assert_eq!(n_off, n_on);
    assert!(n_on > 0);
}
