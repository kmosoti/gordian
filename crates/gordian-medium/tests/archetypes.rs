//! Each archetype against a hand-computed example (docs/medium-ports.md, section 10), and the
//! anchoring rule of section 5. Every expected number below was worked by hand from the
//! parameter table in `src/archetype.rs`; the comments show the working.

mod common;

use common::{Rig, at, ev};
use gordian_medium::{
    CellId, EventRef, Field, Gate, Limits, Medium, MediumBuilder, Pattern, SenseMode, SynapseSpec,
    Tag, TraceItem,
};

/// Feed `target` from one Sum-mode sense cell per node (nodes `0..inputs`), weight 1, delay 0,
/// so that the target runs in pass 2 of the tick its input arrives, with the event values as
/// input.
fn harness(inputs: u16, add_target: impl FnOnce(&mut MediumBuilder) -> CellId) -> (Medium, CellId) {
    let mut b = MediumBuilder::new();
    let senses: Vec<_> = (0..inputs)
        .map(|n| b.sense(at(n, 0), SenseMode::Sum))
        .collect();
    let target = add_target(&mut b);
    for s in senses {
        b.synapse(s, target, 1.0, 0);
    }
    (b.build().unwrap(), target)
}

/// Run ticks `0..script.len()`, with `script[t]` the (node, value) events of tick `t`; return the
/// target's activation in each tick and its state after each tick.
fn drive(m: &mut Medium, target: CellId, script: &[&[(u16, f32)]]) -> (Vec<f32>, Vec<[f32; 8]>) {
    drive_with(m, target, script, Field::default())
}

fn drive_with(
    m: &mut Medium,
    target: CellId,
    script: &[&[(u16, f32)]],
    field: Field,
) -> (Vec<f32>, Vec<[f32; 8]>) {
    let mut events = Vec::new();
    for (t, tick) in script.iter().enumerate() {
        for (seq, (node, value)) in tick.iter().enumerate() {
            events.push(ev(t as u64, 0, *node, 0, *value, seq as u32));
        }
    }
    let mut rig = Rig::new(0, events);
    rig.field.0 = field;
    let mut acts = Vec::new();
    let mut states = Vec::new();
    for t in 0..script.len() as u64 {
        rig.step(m).unwrap();
        let c = &m.cells()[target.0 as usize];
        acts.push(c.activation_at(t));
        states.push(c.state);
    }
    (acts, states)
}

#[test]
fn sense_presence_sum_and_count() {
    let mut b = MediumBuilder::new();
    let p = b.sense(at(0, 0), SenseMode::Presence);
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let c = b.sense(at(0, 0), SenseMode::Count);
    let other = b.sense(at(0, 1), SenseMode::Count);
    let tagged = b.sense(
        Pattern {
            tag: Some(Tag(5)),
            ..Pattern::default()
        },
        SenseMode::Count,
    );
    let mut m = b.build().unwrap();
    let mut e2 = ev(0, 7, 0, 0, 3.0, 1);
    e2.tags.push(Tag(5));
    let mut rig = Rig::new(0, vec![ev(0, 3, 0, 0, 2.0, 0), e2]);
    let summary = rig.step(&mut m).unwrap();
    let act = |id: CellId| m.cells()[id.0 as usize].activation_at(0);
    assert_eq!(act(p), 1.0);
    assert_eq!(act(s), 5.0);
    assert_eq!(act(c), 2.0);
    assert_eq!(act(other), 0.0);
    assert_eq!(act(tagged), 1.0);
    // Routings: event 0 to p, s, c; event 1 to p, s, c and the tagged cell.
    assert_eq!(summary.counts.event_routings, 7);
    assert_eq!(summary.counts.cell_updates, 4);
}

#[test]
fn integrator_with_reset() {
    // leak 0.5, threshold 1.5, reset to zero.
    // t0: 0 + 1 = 1.0, below.  t1: 0.5 + 1 = 1.5, crosses (0.5 < 1.5 <= 1.5): fires 1.5, resets.
    // t2: no input, does not run.  t3: 0 * 0.25 + 1 = 1.0.  t4: 0.5 + 2 = 2.5, fires 2.5.
    let (mut m, x) = harness(1, |b| b.integrator(0.5, 1.5, true, 10));
    let (acts, states) = drive(
        &mut m,
        x,
        &[&[(0, 1.0)], &[(0, 1.0)], &[], &[(0, 1.0)], &[(0, 2.0)]],
    );
    assert_eq!(acts, vec![0.0, 1.5, 0.0, 0.0, 2.5]);
    assert_eq!(
        states.iter().map(|s| s[0]).collect::<Vec<_>>(),
        vec![1.0, 0.0, 0.0, 1.0, 0.0]
    );
}

#[test]
fn integrator_without_reset_fires_on_the_rising_edge_only() {
    // leak 0.5, threshold 1.5, keep.
    // t0: 1.0.  t1: 0.5 + 1 = 1.5, fires 1.5.  t2: 0.75 + 1 = 1.75; 0.75 < 1.5 so it crosses
    // again and fires 1.75.  t3: 0.875 + 1 = 1.875, fires.  With leak 1 instead, the level
    // would stay above and fire once (below).
    let (mut m, x) = harness(1, |b| b.integrator(0.5, 1.5, false, 10));
    let (acts, _) = drive(
        &mut m,
        x,
        &[&[(0, 1.0)], &[(0, 1.0)], &[(0, 1.0)], &[(0, 1.0)]],
    );
    assert_eq!(acts, vec![0.0, 1.5, 1.75, 1.875]);

    // leak 1: 1, 2 (fires), 3 (already above: quiet), 4 (quiet).
    let (mut m, x) = harness(1, |b| b.integrator(1.0, 1.5, false, 10));
    let (acts, states) = drive(
        &mut m,
        x,
        &[&[(0, 1.0)], &[(0, 1.0)], &[(0, 1.0)], &[(0, 1.0)]],
    );
    assert_eq!(acts, vec![0.0, 2.0, 0.0, 0.0]);
    assert_eq!(states[3][0], 4.0);
}

#[test]
fn novelty_with_warmup() {
    // rate 0.5, k 2, floor 0.1, warm-up 2, gaps ignored. Inputs 1, 1, 1, 5.
    // run 1: d = |1 - 0| = 1, runs 0 < 2. mean 0.5, dev 0.5, runs 1.
    // run 2: d = 0.5, runs 1 < 2. mean 0.75, dev 0.5, runs 2.
    // run 3: d = 0.25, band 2 * 0.5 + 0.1 = 1.1, quiet. mean 0.875, dev 0.375.
    // run 4: d = 4.125 > band 0.85: fires 4.125. mean 2.9375, dev 2.25.
    let (mut m, x) = harness(1, |b| b.novelty(0.5, 2.0, 0.1, 2, false));
    let (acts, states) = drive(
        &mut m,
        x,
        &[&[(0, 1.0)], &[(0, 1.0)], &[(0, 1.0)], &[(0, 5.0)]],
    );
    assert_eq!(acts, vec![0.0, 0.0, 0.0, 4.125]);
    assert_eq!(states[3][0], 2.9375);
    assert_eq!(states[3][1], 2.25);
}

#[test]
fn novelty_counts_silent_ticks_as_zeros_in_closed_form() {
    // rate 0.5, k 1, floor 0, warm-up 1, gaps as zeros.
    // t0: x 4, d 4, runs 0 >= 1? no. mean 2, dev 2, runs 1.
    // t1, t2 silent: two zero inputs, step by step: d = 2, dev 2, mean 1; d = 1, dev 1.5,
    // mean 0.5. The closed form gives dev = 0.25 * 2 + 2 * 0.5 * 2 * 0.5 = 1.5, mean 0.5.
    // t3: x 4, d 3.5 > band 1.5: fires 3.5. mean 2.25, dev 2.5.
    let (mut m, x) = harness(1, |b| b.novelty(0.5, 1.0, 0.0, 1, true));
    let (acts, states) = drive(&mut m, x, &[&[(0, 4.0)], &[], &[], &[(0, 4.0)]]);
    assert_eq!(acts, vec![0.0, 0.0, 0.0, 3.5]);
    assert_eq!(states[3][0], 2.25);
    assert_eq!(states[3][1], 2.5);

    // Gaps ignored: t3 sees mean 2, dev 2: d 2, band 2, not above: quiet.
    let (mut m, x) = harness(1, |b| b.novelty(0.5, 1.0, 0.0, 1, false));
    let (acts, _) = drive(&mut m, x, &[&[(0, 4.0)], &[], &[], &[(0, 4.0)]]);
    assert_eq!(acts, vec![0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn coincidence_within_a_window() {
    // n 2, window 2, consume. Slots: node 0 -> 0, node 1 -> 1, node 2 -> 2.
    // t0: slot 0 at age 0; count 1.  t2: slot 0 age 2 <= 2, slot 1 age 0: count 2, fires 2,
    // consumed.  t3: slot 2; count 1.  t6: slot 2 age 3 > 2 expires; slot 0 new: count 1.
    // t7: slot 0 age 1, slot 1 new: fires 2.
    let (mut m, x) = harness(3, |b| b.coincidence(2, 2, true, 10));
    let (acts, _) = drive(
        &mut m,
        x,
        &[
            &[(0, 1.0)],
            &[],
            &[(1, 1.0)],
            &[(2, 1.0)],
            &[],
            &[],
            &[(0, 1.0)],
            &[(1, 1.0)],
        ],
    );
    assert_eq!(acts, vec![0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0]);

    // Without consume, three sources in one tick fire 3, and the next arrival within the window
    // fires again; a non-positive arrival does not count.
    let (mut m, x) = harness(3, |b| b.coincidence(2, 2, false, 10));
    let (acts, states) = drive(
        &mut m,
        x,
        &[&[(0, 1.0), (1, 1.0), (2, 1.0)], &[(0, 1.0)], &[(1, -1.0)]],
    );
    assert_eq!(acts, vec![3.0, 3.0, 3.0]);
    assert_eq!(&states[2][..3], &[1.0, 2.0, 2.0]);
}

#[test]
fn gate_follows_its_field_scalar() {
    // Field scalar 0 at 1.0, threshold 0.5, open above: passes the input sum. At 0.0: closed.
    let (mut m, x) = harness(1, |b| b.gate(0, 0.5, true));
    let open = Field {
        scalars: [1.0, 0.0, 0.0, 0.0],
    };
    let (acts, _) = drive_with(&mut m, x, &[&[(0, 2.0)], &[(0, 3.0)]], open);
    assert_eq!(acts, vec![2.0, 3.0]);
    let (mut m, x) = harness(1, |b| b.gate(0, 0.5, true));
    let (acts, _) = drive_with(&mut m, x, &[&[(0, 2.0)]], Field::default());
    assert_eq!(acts, vec![0.0]);
    // Open below: the reverse.
    let (mut m, x) = harness(1, |b| b.gate(0, 0.5, false));
    let (acts, _) = drive_with(&mut m, x, &[&[(0, 2.0)]], Field::default());
    assert_eq!(acts, vec![2.0]);
    // One field read per run.
    assert_eq!(m.totals().field_reads, 1);
}

#[test]
fn latch_holds_for_h_ticks_by_waking_itself() {
    // threshold 1, hold 2. t0: 1.5 fires, held 1.5, two ticks left: wakes. t1: woken, 1 left,
    // active 1.5, wakes. t2: woken, 0 left, active, no wake. t3: does not run. t4: 0.5 is below
    // the threshold: runs, quiet. t5: 2.0 fires again.
    let (mut m, x) = harness(1, |b| b.latch(1.0, 2));
    let (acts, states) = drive(
        &mut m,
        x,
        &[&[(0, 1.5)], &[], &[], &[], &[(0, 0.5)], &[(0, 2.0)]],
    );
    assert_eq!(acts, vec![1.5, 1.5, 1.5, 0.0, 0.0, 2.0]);
    assert_eq!(states[5][1], 2.0);
    // Runs: the sense cell on t0, t4, t5; the latch on t0, t1, t2, t4, t5.
    assert_eq!(m.totals().cell_updates, 8);
    assert_eq!(m.cells()[x.0 as usize].last_active, Some(5));
}

#[test]
fn emit_threshold_kind_and_refractory() {
    // threshold 1, kind 7, lookback 5, refractory 1. t0: 2 proposes. t1: 2, one tick since:
    // refractory. t2: two ticks since: proposes. t3: 0.5 is below the threshold.
    let (mut m, x) = harness(1, |b| b.emit(1.0, 7, 5, 1));
    let script: &[&[(u16, f32)]] = &[&[(0, 2.0)], &[(0, 2.0)], &[(0, 2.0)], &[(0, 0.5)]];
    let mut events = Vec::new();
    for (t, tick) in script.iter().enumerate() {
        for (seq, (node, value)) in tick.iter().enumerate() {
            events.push(ev(t as u64, 10, *node, 0, *value, seq as u32));
        }
    }
    let mut rig = Rig::new(0, events);
    for _ in 0..4 {
        rig.step(&mut m).unwrap();
    }
    let got: Vec<_> = rig
        .effector
        .proposals
        .iter()
        .map(|(t, p)| (*t, p.kind, p.strength, p.cell, p.anchor.tick))
        .collect();
    assert_eq!(got, vec![(0, 7, 2.0, x, 0), (2, 7, 2.0, x, 2)]);
}

fn r(tick: u64, offset_ns: u32, seq: u32) -> EventRef {
    EventRef {
        tick,
        offset_ns,
        seq,
    }
}

#[test]
fn the_anchor_is_the_earliest_contributing_event_and_refs_respect_the_lookback() {
    // Integrator (leak 1, threshold 3, reset, lookback 10) accumulates three events over three
    // ticks and crosses at t2; the emitter (lookback 1) cites the integrator's support. The
    // anchor is the earliest contributing event (t0); the refs are those within one tick of t2.
    // Inside t2 two events arrive out of offset order; sorting by offset is the medium's job.
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let i = b.integrator(1.0, 3.0, true, 10);
    let e = b.emit(1.0, 3, 1, 0);
    b.synapse(s, i, 1.0, 0);
    b.synapse(i, e, 1.0, 1);
    let mut m = b.build().unwrap();
    let events = vec![
        ev(0, 500, 0, 0, 1.0, 0),
        ev(1, 200, 0, 0, 1.0, 0),
        ev(2, 900, 0, 0, 0.5, 0),
        ev(2, 100, 0, 0, 0.5, 1),
    ];
    let mut rig = Rig::new(0, events);
    for _ in 0..4 {
        rig.step(&mut m).unwrap();
    }
    assert_eq!(rig.effector.proposals.len(), 1);
    let (tick, p) = &rig.effector.proposals[0];
    assert_eq!(
        *tick, 3,
        "the integrator fires at t2, the emitter one tick later"
    );
    assert_eq!(p.anchor, r(0, 500, 0));
    assert_eq!(
        p.refs,
        vec![r(2, 100, 1), r(2, 900, 0)],
        "t0 and t1 are outside the lookback"
    );
    assert_eq!(p.strength, 3.0);
    // The integrator reset, so its support is empty again.
    assert!(m.cells()[i.0 as usize].support.is_empty());
}

/// An accumulating cell's lookback bounds what it cites, not what it sums: with leak 1 the t0
/// event still contributes to the level that crosses at t2, but with lookback 1 the integrator no
/// longer cites it, so the anchor is the t1 event. The level and the support have different
/// memories (DESIGN.md records this as a weakness of the first anchoring rule).
#[test]
fn an_accumulating_cell_prunes_its_support_to_its_lookback() {
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let i = b.integrator(1.0, 3.0, true, 1);
    let e = b.emit(1.0, 3, 100, 0);
    b.synapse(s, i, 1.0, 0);
    b.synapse(i, e, 1.0, 0);
    let mut m = b.build().unwrap();
    let events = vec![
        ev(0, 0, 0, 0, 1.0, 0),
        ev(1, 0, 0, 0, 1.0, 0),
        ev(2, 0, 0, 0, 1.0, 0),
    ];
    let mut rig = Rig::new(0, events);
    for _ in 0..4 {
        rig.step(&mut m).unwrap();
    }
    // Sense (pass 1) and integrator (pass 2) at t2; the emitter would need a third pass, so its
    // zero-delay message is carried and it runs at t3.
    let (tick, p) = &rig.effector.proposals[0];
    assert_eq!(*tick, 3);
    assert_eq!(p.strength, 3.0);
    assert_eq!(p.anchor, r(1, 0, 0));
    assert_eq!(p.refs, vec![r(1, 0, 0), r(2, 0, 0)]);
}

#[test]
fn refs_beyond_max_refs_keep_the_earliest_and_are_counted() {
    let mut b = MediumBuilder::new().limits(Limits {
        max_refs: 2,
        ..Limits::default()
    });
    let s = b.sense(at(0, 0), SenseMode::Count);
    let e = b.emit(1.0, 1, 100, 0);
    b.synapse(s, e, 1.0, 0);
    let mut m = b.build().unwrap();
    let events = (0..5).map(|k| ev(0, 50 - k, 0, 0, 1.0, k)).collect();
    let mut rig = Rig::new(0, events);
    let summary = rig.step(&mut m).unwrap();
    // The sense cell keeps 2 of 5 (3 dropped); the emitter receives those 2.
    assert_eq!(summary.refs_dropped, 3);
    let p = &rig.effector.proposals[0].1;
    assert_eq!(p.anchor, r(0, 46, 4));
    assert_eq!(p.refs, vec![r(0, 46, 4), r(0, 47, 3)]);
}

#[test]
fn gates_on_synapses_and_the_pass_limit() {
    // s -> a (delay 0) -> b (delay 0) -> c (delay 0): with two passes, s runs in pass 1, a in
    // pass 2, and a's message to b is carried to the next tick, where b runs in pass 1 and c in
    // pass 2.
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Presence);
    let a = b.latch(0.5, 0);
    let bb = b.latch(0.5, 0);
    let c = b.latch(0.5, 0);
    b.synapse(s, a, 1.0, 0);
    b.synapse(a, bb, 1.0, 0);
    b.synapse(bb, c, 1.0, 0);
    // A field-gated and a cell-gated synapse from s: the field one is closed (scalar 0 is 0),
    // the cell one is gated by `a`, which runs only in pass 2, after s's traversals: closed.
    let d = b.latch(0.5, 0);
    b.synapse_with(SynapseSpec {
        from: s,
        to: d,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::Field(0),
        plastic: false,
    });
    b.synapse_with(SynapseSpec {
        from: s,
        to: d,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::Cell(a),
        plastic: false,
    });
    // And one gated by s itself, which is active when s's synapses are traversed: open.
    let g = b.latch(0.5, 0);
    b.synapse_with(SynapseSpec {
        from: s,
        to: g,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::Cell(s),
        plastic: false,
    });
    let mut m = b.build().unwrap();
    let mut rig = Rig::new(0, vec![ev(0, 0, 0, 0, 1.0, 0)]);
    let s0 = rig.step(&mut m).unwrap();
    assert_eq!(s0.passes, 2);
    assert_eq!(s0.carried, 1);
    let act = |m: &Medium, id: CellId, t| m.cells()[id.0 as usize].activation_at(t);
    assert_eq!(act(&m, a, 0), 1.0);
    assert_eq!(act(&m, bb, 0), 0.0);
    assert_eq!(act(&m, d, 0), 0.0);
    assert_eq!(act(&m, g, 0), 1.0);
    // Traversals: s's four synapses (one field read) and a's one.
    assert_eq!(s0.counts.synapse_traversals, 5);
    assert_eq!(s0.counts.field_reads, 1);
    let s1 = rig.step(&mut m).unwrap();
    assert_eq!(act(&m, bb, 1), 1.0);
    assert_eq!(act(&m, c, 1), 1.0);
    assert_eq!(s1.carried, 0);
    let ran: Vec<_> = rig.trace.traces[1]
        .items
        .iter()
        .filter_map(|i| match i {
            TraceItem::Ran { cell, pass, .. } => Some((cell.0, *pass)),
            _ => None,
        })
        .collect();
    assert_eq!(ran, vec![(bb.0, 1), (c.0, 2)]);
}

#[test]
fn plastic_weights_only() {
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Presence);
    let e = b.emit(1.0, 1, 0, 0);
    let fixed = b.synapse(s, e, 1.0, 0);
    let plastic = b.synapse_with(SynapseSpec {
        from: s,
        to: e,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::None,
        plastic: true,
    });
    let mut m = b.build().unwrap();
    assert!(m.set_weight(fixed, 2.0).is_err());
    assert!(m.set_weight(plastic, f32::NAN).is_err());
    assert!(m.set_weight(plastic, 0.25).is_ok());
    assert_eq!(m.synapses()[plastic.0 as usize].weight, 0.25);
    // The change is persisted.
    assert_eq!(
        Medium::from_bytes(&m.to_bytes()).unwrap().synapses()[plastic.0 as usize].weight,
        0.25
    );
}

/// The documented order is observable through f32 rounding: `1e8 + 1 - 1e8` is 0 in that order
/// and 1 in the order `-1e8 + 1e8 + 1`.
#[test]
fn events_are_summed_in_offset_order() {
    let mut b = MediumBuilder::new();
    let s = b.sense(at(0, 0), SenseMode::Sum);
    let mut m = b.build().unwrap();
    // Supplied as 1e8 (offset 10), 1 (offset 20), -1e8 (offset 5). Sorted by offset:
    // -1e8, 1e8, 1: the sum is 1.
    let mut rig = Rig::new(
        0,
        vec![
            ev(0, 10, 0, 0, 1.0e8, 0),
            ev(0, 20, 0, 0, 1.0, 1),
            ev(0, 5, 0, 0, -1.0e8, 2),
        ],
    );
    rig.step(&mut m).unwrap();
    assert_eq!(m.cells()[s.0 as usize].activation_at(0), 1.0);
}

/// Messages are summed by synapse id, not by sender: synapse 0 comes from the third sense cell
/// (-1e8), synapse 1 from the first (1e8), synapse 2 from the second (1). By synapse id the sum
/// is -1e8 + 1e8 + 1 = 1; in sender order it would be 1e8 + 1 - 1e8 = 0. Checked for zero-delay
/// messages (next pass) and one-tick messages (the pending queue).
#[test]
fn messages_are_summed_in_synapse_id_order() {
    for delay in [0u8, 1] {
        let mut b = MediumBuilder::new();
        let s0 = b.sense(at(0, 0), SenseMode::Sum);
        let s1 = b.sense(at(1, 0), SenseMode::Sum);
        let s2 = b.sense(at(2, 0), SenseMode::Sum);
        let t = b.integrator(1.0, 1.0e30, true, 0);
        b.synapse(s2, t, 1.0, delay);
        b.synapse(s0, t, 1.0, delay);
        b.synapse(s1, t, 1.0, delay);
        let mut m = b.build().unwrap();
        let mut rig = Rig::new(
            0,
            vec![
                ev(0, 0, 0, 0, 1.0e8, 0),
                ev(0, 0, 1, 0, 1.0, 1),
                ev(0, 0, 2, 0, -1.0e8, 2),
            ],
        );
        rig.step(&mut m).unwrap();
        rig.step(&mut m).unwrap();
        assert_eq!(m.cells()[t.0 as usize].state[0], 1.0, "delay {delay}");
    }
}
