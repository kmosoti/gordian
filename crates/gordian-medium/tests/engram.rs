//! Engrams (work item A1a): bind, recall, strength, contradiction, decay, generalisation,
//! limits, persistence and the two structural operations, each against a hand-worked example.

use gordian_medium::engram::{pair_bytes, restart, restore_pair};
use gordian_medium::{
    Address, Archetype, BindResult, CellSpec, CollectingEffector, ConstantField, CountingLedger,
    Engrams, Event, Field, InMemoryPersist, Key, KeySite, Limits, Medium, MediumBuilder,
    NoTrace, Outcome, OutcomeSite, Persist, Ports, Prices, Recall, ScriptedSense, SenseMode,
    SpecError, StepClock, SynapseSpec, Tag, TimeTarget,
};
use gordian_medium::{EngramParams, Gate, Pattern};

const TICK: u64 = 100_000_000;
const DECAY_PERIOD: u64 = 10_000_000_000; // 100 ticks
const KIND: u16 = 7;

fn params() -> EngramParams {
    EngramParams {
        domain: 0,
        window_ticks: 5,
        min_features: 2,
        gain: 1.0,
        max_strength: 4.0,
        threshold: 0.5,
        penalty: 1.0,
        decay: 0.5,
        decay_rhythm: 0,
        refractory_ticks: 10,
        generalise: false,
        kind: KIND,
    }
}

fn fresh(p: EngramParams, limits: Limits) -> (Medium, Engrams) {
    let spec = Engrams::medium_spec(TICK, DECAY_PERIOD, limits, Prices::DECLARED);
    let medium = Medium::from_spec(&spec).unwrap();
    let engrams = Engrams::new(p, &[0, 1, 2]).unwrap();
    (medium, engrams)
}

fn ev(tick: u64, offset_ms: u32, node: u16, tag: u32, seq: u32) -> Event {
    Event {
        tick,
        offset_ns: offset_ms * 1_000_000,
        source: Address {
            domain: 0,
            node,
            channel: 0,
        },
        tags: vec![Tag(tag)],
        value: 1.0,
        seq,
    }
}

/// Run ticks `from..to` with `events`, the store as the plasticity port; the recalls made.
fn run(
    medium: &mut Medium,
    engrams: &mut Engrams,
    from: u64,
    to: u64,
    events: Vec<Event>,
) -> Vec<(u64, Recall)> {
    let mut sense = ScriptedSense::from_events(events);
    let mut field = ConstantField(Field::default());
    let mut effector = CollectingEffector::default();
    let mut ledger = CountingLedger::default();
    let mut trace = NoTrace;
    for t in from..to {
        let mut clock = StepClock::new(t, TICK);
        let mut ports = Ports {
            clock: &mut clock,
            sense: &mut sense,
            field: &mut field,
            effector: &mut effector,
            ledger: &mut ledger,
            trace: &mut trace,
            plasticity: engrams,
        };
        medium.step(&mut ports).unwrap();
    }
    effector
        .proposals
        .iter()
        .filter_map(|(t, p)| engrams.recall(p).map(|r| (*t, r)))
        .collect()
}

fn key(tags: &[u32], site: KeySite) -> Key {
    Key::new(tags.iter().map(|t| Tag(*t)), site)
}

const OUT_A: Outcome = Outcome {
    tag: Tag(1),
    site: OutcomeSite::Support,
};
const OUT_B: Outcome = Outcome {
    tag: Tag(2),
    site: OutcomeSite::Support,
};

#[test]
fn a_bind_builds_an_engram_that_recalls_on_the_whole_key_at_one_node() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    let b = e.bind(&mut m, &key(&[10, 11, 12], KeySite::Variable), OUT_A);
    assert_eq!(b.result, BindResult::Created(0));
    let g = &e.engrams()[0];
    // Three nodes, three features: 9 sense cells, 3 key cells, a latch, an emitter.
    assert_eq!(m.cells().len(), 9 + 3 + 2);
    assert_eq!(m.synapses().len(), 9 + 3 + 1);
    assert_eq!(g.coincidences.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(g.strength, 1.0);

    // All three features at node 1 within the window: one recall, anchored on the earliest.
    let events = vec![
        ev(3, 10, 1, 11, 0),
        ev(4, 20, 1, 10, 1),
        ev(6, 5, 1, 12, 2),
        ev(6, 6, 1, 99, 3),
    ];
    let r = run(&mut m, &mut e, 0, 10, events);
    assert_eq!(r.len(), 1);
    let (tick, rec) = &r[0];
    assert_eq!(*tick, 6, "four passes: the recall comes in the tick of the last feature");
    assert_eq!(rec.engram, 0);
    assert_eq!(rec.outcome, OUT_A);
    assert_eq!(rec.anchor.seq, 0);
    assert_eq!(
        rec.refs.iter().map(|r| r.seq).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(e.engrams()[0].recalls, 1);
}

#[test]
fn no_recall_when_a_feature_is_missing_late_or_at_another_node() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    // Missing feature: nothing.
    assert!(run(&mut m, &mut e, 0, 10, vec![ev(1, 0, 0, 10, 0)]).is_empty());
    // Late: 10 at tick 10, 11 at tick 16 (window 5 ticks): nothing.
    let late = vec![ev(10, 0, 0, 10, 0), ev(16, 0, 0, 11, 1)];
    assert!(run(&mut m, &mut e, 10, 20, late).is_empty());
    // Split across nodes: nothing (the site is one node, whichever).
    let split = vec![ev(30, 0, 0, 10, 0), ev(30, 1, 1, 11, 1)];
    assert!(run(&mut m, &mut e, 20, 40, split).is_empty());
    // Both at node 2: a recall.
    let both = vec![ev(41, 0, 2, 10, 0), ev(42, 1, 2, 11, 1)];
    assert_eq!(run(&mut m, &mut e, 40, 50, both).len(), 1);
}

#[test]
fn a_fixed_site_recalls_only_at_its_node() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Fixed(2)), OUT_A);
    assert_eq!(e.engrams()[0].coincidences.len(), 1);
    let at0 = vec![ev(1, 0, 0, 10, 0), ev(1, 1, 0, 11, 1)];
    assert!(run(&mut m, &mut e, 0, 10, at0).is_empty());
    let at2 = vec![ev(11, 0, 2, 10, 0), ev(11, 1, 2, 11, 1)];
    assert_eq!(run(&mut m, &mut e, 10, 20, at2).len(), 1);
}

#[test]
fn strength_grows_on_repeated_binds_up_to_the_cap() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    let k = key(&[10, 11], KeySite::Variable);
    e.bind(&mut m, &k, OUT_A);
    let cells = m.cells().len();
    for expect in [2.0, 3.0, 4.0, 4.0] {
        let b = e.bind(&mut m, &k, OUT_A);
        assert_eq!(b.result, BindResult::Strengthened(0));
        assert_eq!(e.engrams()[0].strength, expect);
        let s = e.engrams()[0].strength_synapse;
        assert_eq!(m.synapses()[s.0 as usize].weight, expect);
    }
    // The same key in another order is the same key; nothing new is built.
    let again = e.bind(&mut m, &key(&[11, 10], KeySite::Variable), OUT_A);
    assert_eq!(again.result, BindResult::Strengthened(0));
    assert_eq!(m.cells().len(), cells);
    assert_eq!(e.engrams()[0].binds, 6);
}

#[test]
fn a_disagreeing_answer_weakens_the_engram_and_binds_its_own() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    let k = key(&[10, 11], KeySite::Variable);
    e.bind(&mut m, &k, OUT_A);
    e.bind(&mut m, &k, OUT_A); // strength 2
    // A wider pattern holding the key, with another outcome: the engram would have recalled on
    // it, so it is contradicted.
    let wide = key(&[10, 11, 12], KeySite::Variable);
    let b = e.bind(&mut m, &wide, OUT_B);
    assert_eq!(b.contradicted, vec![0]);
    assert_eq!(b.result, BindResult::Created(1));
    assert_eq!(e.engrams()[0].strength, 1.0);
    assert_eq!(e.engrams()[0].contradictions, 1);
    // A pattern that does not hold the key does not contradict it.
    let other = e.bind(&mut m, &key(&[10, 13], KeySite::Variable), OUT_B);
    assert!(other.contradicted.is_empty());
    // Twice more: strength 0, and the engram no longer recalls.
    e.bind(&mut m, &k, OUT_B);
    assert_eq!(e.engrams()[0].strength, 0.0);
    let events = vec![ev(1, 0, 0, 10, 0), ev(1, 1, 0, 11, 1)];
    let r = run(&mut m, &mut e, 0, 10, events);
    assert!(r.iter().all(|(_, rec)| rec.engram != 0));
    // ... while the engram of the key with the new outcome does.
    assert!(r.iter().any(|(_, rec)| rec.outcome == OUT_B));
}

#[test]
fn strength_decays_at_each_boundary_of_the_decay_rhythm() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A); // 2.0
    // Ticks 0..99 hold the first cycle; its boundary is tick 99.
    run(&mut m, &mut e, 0, 99, Vec::new());
    assert_eq!(e.engrams()[0].strength, 2.0);
    run(&mut m, &mut e, 99, 100, Vec::new());
    assert_eq!(e.engrams()[0].strength, 1.0);
    run(&mut m, &mut e, 100, 300, Vec::new());
    assert_eq!(e.engrams()[0].strength, 0.25);
    assert_eq!(e.stats().decays, 3);
    // Below the threshold (0.5): the whole key no longer recalls.
    let events = vec![ev(301, 0, 0, 10, 0), ev(301, 1, 0, 11, 1)];
    assert!(run(&mut m, &mut e, 300, 310, events).is_empty());
}

#[test]
fn generalisation_keeps_the_features_that_recur_with_the_outcome() {
    let mut p = params();
    p.generalise = true;
    let (mut m, mut e) = fresh(p, Limits::default());
    e.bind(&mut m, &key(&[10, 11, 12], KeySite::Variable), OUT_A);
    let b = e.bind(&mut m, &key(&[10, 11, 13], KeySite::Variable), OUT_A);
    assert_eq!(b.result, BindResult::Generalised(0));
    assert_eq!(e.engrams()[0].live_features(), vec![Tag(10), Tag(11)]);
    assert_eq!(e.engrams()[0].strength, 2.0);
    // The key cells now need two features, and feature 12 no longer counts.
    let c = e.engrams()[0].coincidences[0].1;
    assert_eq!(m.cells()[c.0 as usize].params[0], 2.0);
    let events = vec![ev(1, 0, 0, 10, 0), ev(1, 1, 0, 11, 1)];
    assert_eq!(run(&mut m, &mut e, 0, 10, events).len(), 1);
    // Sharing fewer than min_features: a new engram.
    let b = e.bind(&mut m, &key(&[10, 14, 15], KeySite::Variable), OUT_A);
    assert_eq!(b.result, BindResult::Created(1));
    // Without generalisation the second bind would have been a new engram.
    let (mut m2, mut e2) = fresh(params(), Limits::default());
    e2.bind(&mut m2, &key(&[10, 11, 12], KeySite::Variable), OUT_A);
    let b = e2.bind(&mut m2, &key(&[10, 11, 13], KeySite::Variable), OUT_A);
    assert_eq!(b.result, BindResult::Created(1));
}

#[test]
fn a_key_too_short_is_not_bound_and_repeats_are_dropped() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    let b = e.bind(&mut m, &key(&[10, 10, 10], KeySite::Variable), OUT_A);
    assert_eq!(b.result, BindResult::TooFewFeatures);
    assert!(e.engrams().is_empty());
    assert!(m.cells().is_empty());
    let long = key(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], KeySite::Variable);
    assert_eq!(long.features().len(), 8);
}

#[test]
fn the_limits_refuse_a_bind_and_change_nothing() {
    let limits = Limits {
        max_cells: 10,
        ..Limits::default()
    };
    let (mut m, mut e) = fresh(params(), limits);
    let before = pair_bytes(&m, &e);
    let b = e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    assert!(matches!(
        b.result,
        BindResult::Refused(SpecError::TooManyCells { .. })
    ));
    assert_eq!(e.stats().refused, 1);
    // The stats moved (a refusal is counted); the medium did not.
    assert!(m.cells().is_empty());
    assert_eq!(e.engrams().len(), 0);
    assert_ne!(pair_bytes(&m, &e), before);
    // A site-keyed engram fits (2 sense cells, a key cell, a latch, an emitter).
    let b = e.bind(&mut m, &key(&[10, 11], KeySite::Fixed(0)), OUT_A);
    assert_eq!(b.result, BindResult::Created(0));
}

#[test]
fn the_same_binds_and_events_give_the_same_bytes_and_a_restore_continues_identically() {
    let script = |m: &mut Medium, e: &mut Engrams| {
        e.bind(m, &key(&[10, 11], KeySite::Variable), OUT_A);
        let a = run(m, e, 0, 50, vec![ev(3, 0, 1, 10, 0), ev(4, 0, 1, 11, 1)]);
        e.bind(m, &key(&[12, 13], KeySite::Fixed(1)), OUT_B);
        let b = run(m, e, 50, 150, vec![ev(60, 0, 1, 12, 0), ev(61, 0, 1, 13, 1)]);
        (a, b)
    };
    let (mut m1, mut e1) = fresh(params(), Limits::default());
    let (mut m2, mut e2) = fresh(params(), Limits::default());
    let r1 = script(&mut m1, &mut e1);
    let r2 = script(&mut m2, &mut e2);
    assert_eq!(r1, r2);
    assert_eq!((r1.0.len(), r1.1.len()), (1, 1));
    assert_eq!(pair_bytes(&m1, &e1), pair_bytes(&m2, &e2));

    // Persist, restore, continue: identical to the uninterrupted run.
    let mut port = InMemoryPersist::default();
    gordian_medium::engram::persist(&m1, &e1, &mut port);
    let (mut m3, mut e3) = restore_pair(port.load().unwrap()).unwrap();
    assert_eq!(pair_bytes(&m3, &e3), pair_bytes(&m1, &e1));
    let more = vec![ev(151, 0, 2, 10, 0), ev(152, 0, 2, 11, 1)];
    let a = run(&mut m1, &mut e1, 150, 250, more.clone());
    let b = run(&mut m3, &mut e3, 150, 250, more);
    assert_eq!(a, b);
    assert_eq!(a.len(), 1);
    assert_eq!(pair_bytes(&m1, &e1), pair_bytes(&m3, &e3));
}

#[test]
fn a_restart_keeps_the_engrams_and_forgets_the_activity() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    // Half a pattern at the end of one segment...
    run(&mut m, &mut e, 0, 10, vec![ev(9, 0, 0, 10, 0)]);
    let mut m2 = restart(&m).unwrap();
    assert_eq!(m2.last_tick(), None);
    assert_eq!(m2.synapses(), m.synapses());
    // ... and the other half at the start of the next: no recall across the restart.
    assert!(run(&mut m2, &mut e, 0, 10, vec![ev(0, 0, 0, 11, 0)]).is_empty());
    // The whole pattern in the new segment recalls.
    let both = vec![ev(20, 0, 0, 10, 1), ev(20, 1, 0, 11, 2)];
    assert_eq!(run(&mut m2, &mut e, 10, 30, both).len(), 1);
}

#[test]
fn corrupted_bytes_are_refused_without_panicking() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    e.bind(&mut m, &key(&[12, 13], KeySite::Fixed(2)), OUT_B);
    run(&mut m, &mut e, 0, 20, vec![ev(3, 0, 1, 10, 0)]);
    let bytes = pair_bytes(&m, &e);
    assert!(restore_pair(&bytes).is_ok());
    let mut refused = 0;
    for i in 0..bytes.len() {
        for flip in [0x01u8, 0x80] {
            let mut b = bytes.clone();
            b[i] ^= flip;
            match restore_pair(&b) {
                Ok((m2, e2)) => assert_eq!(pair_bytes(&m2, &e2), b),
                Err(_) => refused += 1,
            }
        }
    }
    assert!(refused > 0);
    for n in 0..bytes.len() {
        assert!(restore_pair(&bytes[..n]).is_err());
    }
}

#[test]
fn plasticity_work_is_counted() {
    let (mut m, mut e) = fresh(params(), Limits::default());
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    let w = e.take_work();
    assert_eq!(w.cell_updates, m.cells().len() as u64);
    assert_eq!(w.synapse_traversals, m.synapses().len() as u64);
    e.bind(&mut m, &key(&[10, 11], KeySite::Variable), OUT_A);
    assert_eq!(e.take_work().synapse_traversals, 1);
    run(&mut m, &mut e, 0, 100, Vec::new());
    assert_eq!(e.take_work().synapse_traversals, 1, "one decay of one engram");
    assert_eq!(e.take_work(), Default::default());
}

#[test]
fn grow_appends_and_keeps_existing_state_and_set_params_validates() {
    let mut b = MediumBuilder::new();
    let s = b.sense(Pattern::default(), SenseMode::Presence);
    let i = b.integrator(0.5, 2.0, true, 0);
    b.timed(TimeTarget::Param { cell: i, index: 0 }, 1_000_000_000);
    let mut spec = b.into_spec();
    spec.synapses.push(SynapseSpec {
        from: s,
        to: i,
        weight: 1.0,
        delay_ticks: 0,
        gate: Gate::None,
        plastic: false,
    });
    spec.oscillome.tick_len_ns = TICK;
    let mut m = Medium::from_spec(&spec).unwrap();
    let mut events = ScriptedSense::from_events(vec![ev(0, 0, 0, 1, 0)]);
    let mut clock = StepClock::new(0, TICK);
    let mut ports = Ports {
        clock: &mut clock,
        sense: &mut events,
        field: &mut ConstantField(Field::default()),
        effector: &mut CollectingEffector::default(),
        ledger: &mut CountingLedger::default(),
        trace: &mut NoTrace,
        plasticity: &mut gordian_medium::NoPlasticity,
    };
    m.step(&mut ports).unwrap();
    let level = m.cells()[i.0 as usize].state[0];
    assert_eq!(level, 1.0);

    let latch = CellSpec {
        archetype: Archetype::Latch,
        params: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        pattern: None,
    };
    let into = SynapseSpec {
        from: s,
        to: i,
        weight: 2.0,
        delay_ticks: 0,
        gate: Gate::None,
        plastic: true,
    };
    let (c, syn) = m.grow(&[latch], &[into]).unwrap();
    assert_eq!((c.0, syn.0), (2, 1));
    assert_eq!(m.cells()[i.0 as usize].state[0], level, "state kept");
    assert_eq!(m.last_tick(), Some(0));
    // Refusals change nothing.
    let before = m.to_bytes();
    let bad = SynapseSpec { to: s, ..into };
    assert!(m.grow(&[], &[bad]).is_err());
    assert!(m.set_params(&[(i, 1, f32::NAN)]).is_err());
    assert!(m.set_params(&[(i, 0, 0.9)]).is_err(), "set by a quantity in time");
    assert!(m.set_params(&[(gordian_medium::CellId(99), 0, 1.0)]).is_err());
    assert_eq!(m.to_bytes(), before);
    m.set_params(&[(i, 1, 3.0)]).unwrap();
    assert_eq!(m.cells()[i.0 as usize].params[1], 3.0);
    // The grown medium persists and restores.
    assert_eq!(Medium::from_bytes(&m.to_bytes()).unwrap().to_bytes(), m.to_bytes());
}
