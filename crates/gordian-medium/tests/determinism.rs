//! The determinism, sparsity, counting and limit tests of docs/medium-ports.md, section 10.

mod common;

use common::{Rig, at, ev, open_field, random_events, rich_spec};
use gordian_medium::{
    Event, InMemoryPersist, Limits, Medium, MediumBuilder, MediumSpec, OpCounts, Proposal,
    SenseMode, StepError, TraceItem,
};

const START: u64 = 1_000;
const TICKS: u64 = 120;

fn run_collecting(spec: &MediumSpec, events: Vec<Event>) -> (Vec<Vec<u8>>, Vec<(u64, Proposal)>) {
    let mut m = Medium::from_spec(spec).unwrap();
    let mut rig = Rig::new(START, events);
    rig.field.0 = open_field();
    let mut bytes = Vec::new();
    for _ in 0..TICKS {
        rig.step(&mut m).unwrap();
        bytes.push(m.to_bytes());
    }
    (bytes, rig.effector.proposals)
}

#[test]
fn same_spec_and_events_give_identical_bytes_after_every_tick() {
    let spec = rich_spec(Limits::default());
    for seed in [1u64, 2, 3] {
        let events = random_events(seed, START, TICKS, 6);
        let (a, pa) = run_collecting(&spec, events.clone());
        let (b, pb) = run_collecting(&spec, events);
        assert_eq!(a, b, "seed {seed}");
        assert_eq!(pa, pb, "seed {seed}");
        // Not vacuous: the run proposed, and the medium's state moved.
        assert!(!pa.is_empty(), "seed {seed}: no proposals");
        assert_ne!(a.first(), a.last());
    }
}

#[test]
fn snapshot_restore_and_continue_matches_the_uninterrupted_run() {
    let spec = rich_spec(Limits::default());
    let events = random_events(7, START, TICKS, 6);
    let (reference, ref_props) = run_collecting(&spec, events.clone());
    for k in [0u64, 1, 17, 60, TICKS - 1] {
        let mut m = Medium::from_spec(&spec).unwrap();
        let mut rig = Rig::new(START, events.clone());
        rig.field.0 = open_field();
        for _ in 0..k {
            rig.step(&mut m).unwrap();
        }
        let mut store = InMemoryPersist::default();
        m.persist(&mut store);
        drop(m);
        let mut restored = Medium::restore(&store).unwrap();
        assert_eq!(store.load_bytes(), restored.to_bytes().as_slice());
        for t in k..TICKS {
            rig.step(&mut restored).unwrap();
            assert_eq!(
                restored.to_bytes(),
                reference[t as usize],
                "k {k}, tick {t}"
            );
        }
        let after: Vec<_> = ref_props
            .iter()
            .filter(|(t, _)| *t >= START + k)
            .cloned()
            .collect();
        let mine: Vec<_> = rig
            .effector
            .proposals
            .iter()
            .filter(|(t, _)| *t >= START + k)
            .cloned()
            .collect();
        assert_eq!(mine, after, "k {k}");
    }
}

trait LoadBytes {
    fn load_bytes(&self) -> &[u8];
}

impl LoadBytes for InMemoryPersist {
    fn load_bytes(&self) -> &[u8] {
        self.bytes.as_deref().unwrap()
    }
}

#[test]
fn events_in_a_different_order_within_a_tick_give_the_same_result() {
    let spec = rich_spec(Limits::default());
    // Dense ticks, so that sense cells get three or more events at once: with two, IEEE addition
    // is commutative and a missing sort would go unseen (a mutation check found this).
    let events = random_events(11, START, TICKS, 40);
    let (reference, ref_props) = run_collecting(&spec, events.clone());
    // Reverse each tick's events, and separately rotate them, keeping ticks in order.
    let reorder = |rotate: bool| -> Vec<Event> {
        let mut out = Vec::new();
        for t in START..START + TICKS {
            let mut tick: Vec<Event> = events.iter().filter(|e| e.tick == t).cloned().collect();
            if rotate {
                let n = tick.len();
                if n > 1 {
                    tick.rotate_left(n / 2);
                }
            } else {
                tick.reverse();
            }
            out.extend(tick);
        }
        out
    };
    for rotate in [false, true] {
        let (bytes, props) = run_collecting(&spec, reorder(rotate));
        assert_eq!(bytes, reference, "rotate {rotate}");
        assert_eq!(props, ref_props, "rotate {rotate}");
    }
}

#[test]
fn a_cell_with_no_input_is_not_updated_and_not_counted() {
    let mut b = MediumBuilder::new();
    let fed = b.sense(at(0, 0), SenseMode::Sum);
    let starved = b.sense(at(1, 0), SenseMode::Sum);
    let integ = b.integrator(0.5, 100.0, true, 5);
    b.synapse(starved, integ, 1.0, 0);
    let mut m = b.build().unwrap();
    let before = m.cells()[starved.0 as usize].clone();
    let before_integ = m.cells()[integ.0 as usize].clone();
    let events = (0..10).map(|t| ev(t, 0, 0, 0, 1.0, 0)).collect();
    let mut rig = Rig::new(0, events);
    for _ in 0..10 {
        let s = rig.step(&mut m).unwrap();
        assert_eq!(s.counts.cell_updates, 1);
        assert_eq!(
            s.counts.synapse_traversals, 0,
            "the fed cell has no synapses"
        );
        assert_eq!(s.active_cells, 1);
    }
    assert_eq!(m.cells()[starved.0 as usize], before);
    assert_eq!(m.cells()[integ.0 as usize], before_integ);
    assert_eq!(m.cells()[fed.0 as usize].last_active, Some(9));
    for trace in &rig.trace.traces {
        for item in &trace.items {
            if let TraceItem::Ran { cell, .. } = item {
                assert_eq!(*cell, fed);
            }
        }
    }
    assert_eq!(m.totals().cell_updates, 10);
}

/// Recount every operation kind from the trace items, independently of the medium's counters.
fn recount(items: &[TraceItem]) -> OpCounts {
    let mut c = OpCounts::default();
    for item in items {
        match item {
            TraceItem::Routed { .. } | TraceItem::Unmatched { .. } => c.event_routings += 1,
            TraceItem::Ran {
                field_reads,
                inputs,
                ..
            } => {
                assert!(*inputs > 0, "a cell ran without input");
                c.cell_updates += 1;
                c.field_reads += field_reads;
            }
            TraceItem::Traversed { field_read, .. } => {
                c.synapse_traversals += 1;
                c.field_reads += u64::from(*field_read);
            }
            TraceItem::Proposed { .. } => c.proposals += 1,
            TraceItem::NotRun { .. } | TraceItem::ProposalDropped { .. } => {}
        }
    }
    c
}

#[test]
fn operation_counts_equal_an_independent_recount_from_the_trace() {
    for (seed, max_ops) in [(3u64, 1_000_000u64), (4, 40), (5, 7)] {
        let limits = Limits {
            max_ops_per_tick: max_ops,
            max_proposals_per_tick: 2,
            ..Limits::default()
        };
        let mut m = Medium::from_spec(&rich_spec(limits)).unwrap();
        let mut rig = Rig::new(START, random_events(seed, START, TICKS, 8));
        rig.field.0 = open_field();
        let mut summed = OpCounts::default();
        let mut summaries = Vec::new();
        for _ in 0..TICKS {
            let s = rig.step(&mut m).unwrap();
            summed.accumulate(&s.counts);
            summaries.push(s);
        }
        assert_eq!(rig.trace.traces.len() as u64, TICKS);
        for (trace, s) in rig.trace.traces.iter().zip(&summaries) {
            assert_eq!(trace.tick, s.tick);
            assert_eq!(
                recount(&trace.items),
                s.counts,
                "seed {seed}, tick {}",
                s.tick
            );
            // Every traversal's source ran in that pass.
            for item in &trace.items {
                if let TraceItem::Traversed { synapse, pass, .. } = item {
                    let from = m.synapses()[synapse.0 as usize].from;
                    assert!(trace.items.iter().any(|i| matches!(i,
                        TraceItem::Ran { cell, pass: p, .. } if *cell == from && p == pass)));
                }
            }
        }
        assert_eq!(&summed, m.totals());
        assert_eq!(rig.ledger.totals, summed);
        assert_eq!(rig.ledger.ticks, TICKS);
        assert_eq!(
            rig.effector.proposals.len() as u64,
            summed.proposals,
            "seed {seed}"
        );
    }
}

#[test]
fn the_operation_limit_truncates_and_records_without_panicking() {
    // Five events to five sense cells, a limit of three operations: three routings, then
    // nothing; the two events not routed and the three cells that got input but did not run are
    // recorded.
    let mut b = MediumBuilder::new().limits(Limits {
        max_ops_per_tick: 3,
        ..Limits::default()
    });
    let cells: Vec<_> = (0..5)
        .map(|n| b.sense(at(n, 0), SenseMode::Presence))
        .collect();
    let mut m = b.build().unwrap();
    let events = (0..5u16)
        .map(|n| ev(0, u32::from(n), n, 0, 1.0, u32::from(n)))
        .collect();
    let mut rig = Rig::new(0, events);
    let s = rig.step(&mut m).unwrap();
    assert_eq!(s.counts.event_routings, 3);
    assert_eq!(s.counts.cell_updates, 0);
    let t = s.truncation.expect("truncated");
    assert!(t.op_limit && !t.proposal_limit);
    assert_eq!(
        t.pass, 0,
        "the limit was hit while routing the fourth event"
    );
    assert_eq!(t.events_unrouted, 2);
    assert_eq!(t.cells_not_run, 3);
    assert_eq!(rig.ledger.truncations, vec![(0, t)]);
    for c in &cells {
        assert_eq!(m.cells()[c.0 as usize].last_active, None);
    }
    // The next tick starts with a fresh allowance.
    let s = rig.step(&mut m).unwrap();
    assert_eq!(s.truncation, None);

    // The rich spec under tight limits, many seeds: never over the limit, never a panic, and
    // every truncation reaches the ledger.
    for seed in 0..20u64 {
        let limits = Limits {
            max_ops_per_tick: 1 + seed * 3,
            max_proposals_per_tick: (seed % 3) as u32,
            ..Limits::default()
        };
        let mut m = Medium::from_spec(&rich_spec(limits)).unwrap();
        let mut rig = Rig::new(START, random_events(seed, START, 60, 10));
        rig.field.0 = open_field();
        let mut truncated = 0;
        for _ in 0..60 {
            let s = rig.step(&mut m).unwrap();
            assert!(s.counts.ops() <= limits.max_ops_per_tick);
            assert!(s.counts.proposals <= u64::from(limits.max_proposals_per_tick));
            truncated += u64::from(s.truncation.is_some());
        }
        assert_eq!(rig.ledger.truncations.len() as u64, truncated);
        assert!(truncated > 0, "seed {seed}: the limits were never reached");
    }
}

#[test]
fn the_proposal_limit_drops_and_records() {
    let mut b = MediumBuilder::new().limits(Limits {
        max_proposals_per_tick: 2,
        ..Limits::default()
    });
    for n in 0..4 {
        let s = b.sense(at(n, 0), SenseMode::Presence);
        let e = b.emit(1.0, 9, 0, 0);
        b.synapse(s, e, 1.0, 0);
    }
    let mut m = b.build().unwrap();
    let events = (0..4u16)
        .map(|n| ev(5, 0, n, 0, 1.0, u32::from(n)))
        .collect();
    let mut rig = Rig::new(5, events);
    let s = rig.step(&mut m).unwrap();
    assert_eq!(s.counts.proposals, 2);
    let t = s.truncation.unwrap();
    assert!(t.proposal_limit && !t.op_limit);
    assert_eq!(t.proposals_dropped, 2);
    assert_eq!(rig.effector.proposals.len(), 2);
    // The two kept are the lowest cell ids.
    let cells: Vec<_> = rig
        .effector
        .proposals
        .iter()
        .map(|(_, p)| p.cell.0)
        .collect();
    assert_eq!(cells, vec![1, 3]);
}

#[test]
fn malformed_ticks_are_refused_and_change_nothing() {
    let spec = rich_spec(Limits::default());
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::new(10, vec![ev(10, 0, 0, 0, 1.0, 0)]);
    rig.step(&mut m).unwrap();
    let before = m.to_bytes();

    // A duplicate seq.
    rig.sense
        .by_tick
        .insert(11, vec![ev(11, 0, 0, 0, 1.0, 4), ev(11, 9, 1, 0, 1.0, 4)]);
    assert_eq!(
        rig.step(&mut m),
        Err(StepError::DuplicateSeq { tick: 11, seq: 4 })
    );
    assert_eq!(m.to_bytes(), before);
    // A NaN value.
    rig.sense
        .by_tick
        .insert(11, vec![ev(11, 0, 0, 0, f32::NAN, 0)]);
    assert!(matches!(
        rig.step(&mut m),
        Err(StepError::NonFiniteEventValue(_))
    ));
    // An event of another tick.
    rig.sense.by_tick.insert(11, vec![ev(12, 0, 0, 0, 1.0, 0)]);
    assert!(matches!(
        rig.step(&mut m),
        Err(StepError::EventTickMismatch { .. })
    ));
    // A non-finite field.
    rig.field.0.scalars[2] = f32::INFINITY;
    assert_eq!(
        rig.step(&mut m),
        Err(StepError::NonFiniteField { index: 2 })
    );
    rig.field.0.scalars[2] = 0.0;
    // A skipped tick.
    rig.clock.tick = 13;
    assert_eq!(
        rig.step(&mut m),
        Err(StepError::TickOutOfOrder {
            expected: Some(11),
            got: 13
        })
    );
    assert_eq!(m.to_bytes(), before);
    rig.clock.tick = 11;
    assert!(rig.step(&mut m).is_ok());
}

#[test]
fn the_spec_round_trips_through_json_exactly() {
    let spec = rich_spec(Limits::default());
    let json = serde_json::to_string(&spec).unwrap();
    let back: MediumSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(back, spec);
    assert_eq!(
        Medium::from_spec(&back).unwrap().to_bytes(),
        Medium::from_spec(&spec).unwrap().to_bytes()
    );
    // And the built medium gives its spec back.
    assert_eq!(Medium::from_spec(&spec).unwrap().spec(), spec);
}

#[test]
fn persisted_bytes_reject_corruption_without_panicking() {
    let spec = rich_spec(Limits::default());
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut rig = Rig::new(START, random_events(9, START, 30, 6));
    rig.field.0 = open_field();
    for _ in 0..30 {
        rig.step(&mut m).unwrap();
    }
    let bytes = m.to_bytes();
    assert!(m.pending_messages() > 0 || !bytes.is_empty());
    assert_eq!(Medium::from_bytes(&bytes).unwrap().to_bytes(), bytes);
    for cut in [0, 3, 5, 40, bytes.len() / 2, bytes.len() - 1] {
        assert!(Medium::from_bytes(&bytes[..cut]).is_err(), "cut {cut}");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(Medium::from_bytes(&longer).is_err());
    // Flip every byte in turn: decoding either fails or yields a medium; it never panics.
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        let _ = Medium::from_bytes(&b);
    }
}
