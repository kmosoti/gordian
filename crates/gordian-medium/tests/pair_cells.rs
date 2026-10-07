//! Pair cells (work item A2): the cells a store builds, the level as a decayed net count of the
//! evidence events, the held edge and its narrowest band, the trials' bookkeeping, the miss
//! weight, and determinism.

use gordian_medium::oscillome::decay_per_tick;
use gordian_medium::{
    Archetype, CollectingEffector, ConstantField, CountingLedger, EV_CHANNEL, Event, Field, Limits,
    Medium, NoPlasticity, NoTrace, PairCells, PairParams, Ports, Prices, ScriptedSense, StepClock,
    Trials, chance, pair_node,
};

const TICK: u64 = 100_000_000;
const S: u64 = 1_000_000_000;

fn params() -> PairParams {
    PairParams {
        domain: 0,
        bands_ns: vec![400_000_000, 2 * S, 10 * S],
        gain: 1.0,
        threshold: 2.0,
        tau_ns: 150 * S,
    }
}

fn store(pairs: &[(u16, u16)]) -> (Medium, PairCells) {
    PairCells::build(params(), pairs, TICK, Limits::default(), Prices::DECLARED).unwrap()
}

/// Run ticks `from..to` with `events`.
fn run(m: &mut Medium, from: u64, to: u64, events: Vec<Event>) {
    let mut sense = ScriptedSense::from_events(events);
    let mut field = ConstantField(Field::default());
    let mut ledger = CountingLedger::default();
    let mut trace = NoTrace;
    let mut plasticity = NoPlasticity;
    let mut effector = CollectingEffector::default();
    for t in from..to {
        let mut clock = StepClock::new(t, TICK);
        let mut ports = Ports {
            clock: &mut clock,
            sense: &mut sense,
            field: &mut field,
            effector: &mut effector,
            ledger: &mut ledger,
            trace: &mut trace,
            plasticity: &mut plasticity,
        };
        m.step(&mut ports).unwrap();
    }
}

fn ev(p: &PairCells, a: u16, b: u16, k: usize, value: f32, at_ns: u64, seq: u32) -> Event {
    p.evidence_event((a, b, k), value, at_ns, 0, seq).unwrap()
}

#[test]
fn a_store_builds_a_sense_cell_and_a_decaying_integrator_per_pair_and_band() {
    let (m, p) = store(&[(0, 1), (1, 0), (2, 5)]);
    assert_eq!(m.cells().len(), 3 * 3 * 2);
    assert_eq!(m.synapses().len(), 3 * 3);
    let leak = decay_per_tick(150 * S, TICK);
    for &(a, b) in p.pairs() {
        for k in 0..3 {
            let c = &m.cells()[p.counter(a, b, k).unwrap().0 as usize];
            assert_eq!(c.archetype, Archetype::Integrator);
            assert_eq!(c.params[0], leak, "leak from tau");
            assert_eq!(c.params[1], 2.0, "threshold");
            assert_eq!(c.params[2], 1.0, "never reset");
            let s = &m.cells()[c.id.0 as usize - 1];
            assert_eq!(s.archetype, Archetype::Sense);
            let pat = s.pattern.unwrap();
            assert_eq!(pat.node, Some(pair_node(a, b)));
            assert_eq!(pat.channel, Some(EV_CHANNEL + k as u16));
        }
    }
    assert!(p.has(2, 5) && !p.has(5, 2));
    assert!(p.counter(0, 1, 3).is_none());
}

#[test]
fn a_store_refuses_bad_pairs_and_bad_parameters() {
    let build = |pairs: &[(u16, u16)], params: PairParams| {
        PairCells::build(params, pairs, TICK, Limits::default(), Prices::DECLARED).is_err()
    };
    assert!(build(&[(1, 1)], params()));
    assert!(build(&[(0, 1), (0, 1)], params()));
    assert!(build(&[(0, 200)], params()));
    let mut q = params();
    q.bands_ns = vec![2 * S, S];
    assert!(build(&[(0, 1)], q));
    let mut q = params();
    q.bands_ns = vec![];
    assert!(build(&[(0, 1)], q));
    let mut q = params();
    q.tau_ns = 0;
    assert!(build(&[(0, 1)], q));
    let mut q = params();
    q.gain = 0.0;
    assert!(build(&[(0, 1)], q));
}

#[test]
fn the_level_is_the_evidence_decayed_by_the_integrators_own_rule() {
    let (mut m, mut p) = store(&[(0, 1)]);
    let leak = decay_per_tick(150 * S, TICK);
    // +1 at tick 3, nothing else.
    run(&mut m, 0, 11, vec![ev(&p, 0, 1, 2, 1.0, 3 * TICK + 5, 0)]);
    let l10 = p.level(&m, 0, 1, 2, 10).unwrap();
    let mut want = 1.0f32;
    for _ in 0..7 {
        want *= leak;
    }
    assert!((l10 - want).abs() < 1e-6, "{l10} {want}");
    // Other bands untouched.
    assert_eq!(p.level(&m, 0, 1, 0, 10), Some(0.0));
    // A miss of 0.5 at tick 11: the cell's state at its run equals the read level before it, minus
    // 0.5; the next read decays it once more.
    let before = p.level(&m, 0, 1, 2, 11).unwrap();
    run(&mut m, 11, 13, vec![ev(&p, 0, 1, 2, -0.5, 11 * TICK, 1)]);
    let c = &m.cells()[p.counter(0, 1, 2).unwrap().0 as usize];
    assert_eq!(
        c.state[0],
        before - 0.5,
        "the read is the level the cell runs from"
    );
    assert_eq!(p.level(&m, 0, 1, 2, 12).unwrap(), (before - 0.5) * leak);
    // Reads are counted, one cell update each.
    assert_eq!(p.take_work().cell_updates, 4);
    assert_eq!(p.take_work().cell_updates, 0);
}

#[test]
fn there_is_no_floor_misses_take_the_level_below_zero() {
    let (mut m, mut p) = store(&[(0, 1)]);
    let evs = (0..4)
        .map(|i| ev(&p, 0, 1, 0, -1.0, (2 + i) * TICK, i as u32))
        .collect();
    run(&mut m, 0, 8, evs);
    let l = p.level(&m, 0, 1, 0, 8).unwrap();
    assert!(l < -3.9 && l > -4.0, "{l}");
}

#[test]
fn an_edge_is_held_from_the_threshold_on_and_named_by_its_narrowest_band() {
    let (mut m, mut p) = store(&[(0, 1), (1, 0)]);
    // One follow in every band: not held.
    let one: Vec<Event> = (0..3)
        .map(|k| ev(&p, 0, 1, k, 1.0, 2 * TICK, k as u32))
        .collect();
    run(&mut m, 0, 3, one);
    assert_eq!(p.held(&m, 0, 1, 3), None);
    // A second follow in bands 1 and 2, a miss in band 0: held, narrowest band 1.
    let two = vec![
        ev(&p, 0, 1, 0, -0.1, 3 * TICK, 10),
        ev(&p, 0, 1, 1, 1.0, 3 * TICK, 11),
        ev(&p, 0, 1, 2, 1.0, 3 * TICK, 12),
    ];
    run(&mut m, 3, 4, two);
    // Decay below 2 at once: two follows decayed for one tick are just under 2.
    let l = p.level(&m, 0, 1, 1, 4).unwrap();
    assert!(l < 2.0 && l > 1.99, "{l}");
    assert_eq!(p.held(&m, 0, 1, 4), None);
    // A third follow in band 1 only.
    run(&mut m, 4, 5, vec![ev(&p, 0, 1, 1, 1.0, 4 * TICK, 20)]);
    assert_eq!(p.held(&m, 0, 1, 5), Some(1));
    assert_eq!(
        p.held(&m, 1, 0, 5),
        None,
        "the reverse pair is another cell"
    );
    // Long after, decay has taken it below the threshold again.
    run(&mut m, 5, 6, vec![]);
    assert_eq!(p.held(&m, 0, 1, 5 + 600 * 10), None);
}

#[test]
fn an_evidence_event_for_a_tick_already_run_goes_to_the_next_tick_at_offset_zero() {
    let (_, p) = store(&[(0, 1)]);
    let e = p
        .evidence_event((0, 1, 1), -0.5, 3 * TICK + 7, 2, 4)
        .unwrap();
    assert_eq!((e.tick, e.offset_ns), (3, 7));
    let e = p
        .evidence_event((0, 1, 1), -0.5, 3 * TICK + 7, 5, 4)
        .unwrap();
    assert_eq!((e.tick, e.offset_ns), (5, 0));
    assert_eq!(e.value, -0.5);
    assert!(p.evidence_event((1, 0, 1), 1.0, 0, 0, 0).is_none());
}

#[test]
fn a_follow_credits_the_latest_open_trial_per_partner_and_band_once() {
    let mut t = Trials::new(vec![400, 2_000, 10_000]);
    // a = 0 at 1000 and at 4000 (both waiting for b = 1); a = 2 at 3500 waiting for b = 1. The
    // weights are (follow, miss) per band.
    let w0 = [(0.9, 0.1), (0.8, 0.2), (0.7, 0.3)];
    t.open(0, 1, 1_000, 7, &w0);
    t.open(0, 1, 4_000, 8, &w0);
    t.open(2, 1, 3_500, 9, &[(0.6, 0.4), (0.5, 0.5), (0.4, 0.6)]);
    assert_eq!(t.open_count(), 9);
    // b = 1 at 4300: for a = 0 every band's latest trial (opened at 4000) is credited; for a = 2
    // the 2 s and 10 s trials (gap 800).
    let f = t.follow(1, 4_300, 77);
    let got: Vec<(u16, usize, u64, u32, f32)> = f
        .iter()
        .map(|e| {
            (
                e.trial.a,
                e.trial.band,
                e.trial.at_ns,
                e.trial.token,
                e.value,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (0, 0, 4_000, 8, 0.9),
            (0, 1, 4_000, 8, 0.8),
            (0, 2, 4_000, 8, 0.7),
            (2, 1, 3_500, 9, 0.5),
            (2, 2, 3_500, 9, 0.4),
        ]
    );
    assert!(f.iter().all(|e| e.follow && e.by == 77 && e.at_ns == 4_300));
    // A second instant at 1 at 4350 credits only what is left with it in its window: a = 0's
    // 10 s trial opened at 1000 (gap 3350); a = 2's 0.4 s trial is 850 behind, out of its band.
    let f = t.follow(1, 4_350, 78);
    let got: Vec<(u16, usize, u64)> = f
        .iter()
        .map(|e| (e.trial.a, e.trial.band, e.trial.at_ns))
        .collect();
    assert_eq!(got, vec![(0, 2, 1_000)]);
    // An instant at another node, or at the trial's own instant, credits nothing.
    assert!(t.follow(3, 4_400, 0).is_empty());
    // What is left expires as misses at the deadlines, in deadline order.
    let m = t.expire(u64::MAX);
    let got: Vec<(u16, usize, u64, f32)> = m
        .iter()
        .map(|e| (e.trial.a, e.trial.band, e.at_ns, e.value))
        .collect();
    assert_eq!(
        got,
        vec![
            (0, 0, 1_400, -0.1),
            (0, 1, 3_000, -0.2),
            (2, 0, 3_900, -0.4)
        ]
    );
    assert!(m.iter().all(|e| !e.follow));
    assert_eq!(t.open_count(), 0);
}

#[test]
fn a_trial_expires_only_once_its_deadline_is_past_and_a_follow_at_the_deadline_counts() {
    let mut t = Trials::new(vec![400]);
    t.open(0, 1, 1_000, 1, &[(0.5, 0.5)]);
    assert!(
        t.expire(1_400).is_empty(),
        "deadline 1400 is not before 1400"
    );
    assert_eq!(
        t.follow(1, 1_400, 2).len(),
        1,
        "a gap equal to the band is inside it"
    );
    t.open(0, 1, 2_000, 3, &[(0.5, 0.5)]);
    assert!(
        t.follow(1, 2_000, 4).is_empty(),
        "the same instant is not after it"
    );
    assert_eq!(t.expire(2_401).len(), 1);
}

#[test]
fn the_chance_is_that_of_a_poisson_instant_in_the_window_and_gives_zero_drift() {
    assert_eq!(chance(0.0, 10 * S), 0.0);
    assert_eq!(chance(-1.0, 10 * S), 0.0);
    assert_eq!(chance(f64::NAN, 10 * S), 0.0);
    let q = chance(0.1, 10 * S);
    assert!((q - (1.0 - (-1.0f64).exp()) as f32).abs() < 1e-6, "{q}");
    let q = chance(0.1, 400_000_000);
    assert!((q - (1.0 - (-0.04f64).exp()) as f32).abs() < 1e-7, "{q}");
    // Zero drift at chance: q (1 - q) - (1 - q) q = 0; and a follow is worth less the likelier
    // it was.
    assert!(chance(0.25, 400_000_000) < chance(0.25, 2 * S));
    assert!(chance(0.25, 2 * S) < chance(0.25, 10 * S));
    assert!(chance(1.0e6, 10 * S) <= 1.0);
}

#[test]
fn the_same_evidence_gives_the_same_levels_bit_for_bit() {
    let go = || {
        let (mut m, mut p) = store(&[(0, 1), (3, 2)]);
        let mut evs = Vec::new();
        for i in 0..40u32 {
            let v = if i % 3 == 0 { 1.0 } else { -0.37 };
            evs.push(ev(
                &p,
                0,
                1,
                (i % 3) as usize,
                v,
                u64::from(i) * 7 * TICK,
                i,
            ));
            evs.push(ev(&p, 3, 2, 1, -v, u64::from(i) * 5 * TICK + 3, 1000 + i));
        }
        evs.sort_by_key(|e| e.tick);
        run(&mut m, 0, 400, evs);
        let mut bits = Vec::new();
        for &(a, b) in &[(0u16, 1u16), (3, 2)] {
            for k in 0..3 {
                bits.push(p.level(&m, a, b, k, 400).unwrap().to_bits());
            }
        }
        (bits, *m.totals())
    };
    assert_eq!(go(), go());
}
