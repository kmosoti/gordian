//! The learned noticer (work item L1): the update rule on hand-made evidence, the transplant of a
//! running medium's state under new parameters, the equivalence with M2's medium when nothing is
//! learned, the carry across segments, and the manifest.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test (`scripts/l1_gate.py`).

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::learned::carry;
use gordian_run::stream::arms::learned::learner::{
    FAR_BINS, GAP_EDGES_S, Learned, NB, NP, PEAK_EDGES, Session, slot_of,
};
use gordian_run::stream::arms::learned::noticing::{LearnedNoticer, learned_spec};
use gordian_run::stream::arms::learned::params::round_window;
use gordian_run::stream::arms::learned::splice::transplant;
use gordian_run::stream::arms::learned::{LearnedParams, build};
use gordian_run::stream::arms::medium::{MediumNoticer, MediumParams};
use gordian_run::stream::arms::noticer::{Notice, Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_stream::{ObsId, generate};
use gordian_world::{CounterName, Observation, ServiceId};
use serde_json::Value;
use stream_common::*;

const MS: u64 = 1_000_000;

/// M2's frozen 100 ms graph, from the committed selection.
fn frozen() -> MediumParams {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/exploration/m2-selected.json"
    );
    let sel: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let NoticerSpec::Medium(p) =
        serde_json::from_value(sel["ticks"]["100"]["noticer"].clone()).unwrap()
    else {
        panic!("not a medium");
    };
    p
}

/// The learned arm at M2's frozen constants with learning off: priors equal the frozen values.
fn learned_at_frozen() -> LearnedParams {
    let f = frozen();
    LearnedParams {
        learning: false,
        carry: false,
        prior_window_ns: f.burst_window_ns,
        prior_ramp_threshold: f.ramp_threshold,
        ..LearnedParams::with_structure(f, 1)
    }
}

/// Play the public observations of `seed` (a stream of `duration_s`) to a noticer as the rung
/// does, a step every 500 ms, and return the notices (with the step's instant), the retirable
/// sets, and the noticer's cost charges.
fn replay<N: Noticer>(
    n: &mut N,
    seed: u64,
    duration_s: u64,
) -> (Vec<(u64, Notice)>, Vec<(u64, Vec<u32>)>, Vec<u64>) {
    replay_params(n, &params(seed, duration_s))
}

fn replay_params<N: Noticer>(
    n: &mut N,
    p: &gordian_stream::StreamParams,
) -> (Vec<(u64, Notice)>, Vec<(u64, Vec<u32>)>, Vec<u64>) {
    let stream = generate(p);
    let public = stream.public_info();
    let all: Vec<Held> = stream
        .events()
        .iter()
        .enumerate()
        .map(|(i, (at, o))| Held {
            id: ObsId(i as u32),
            at: *at,
            abnormal: is_abnormal(o, &public.services),
            obs: o.clone(),
        })
        .collect();
    let (mut items, mut next, mut now) = (Vec::new(), 0, 0u64);
    let (mut notices, mut retirable, mut costs) = (Vec::new(), Vec::new(), Vec::new());
    while now < p.duration_ns {
        while next < all.len() && all[next].at.0 <= now {
            if all[next].abnormal {
                n.observe(&all[next]);
            }
            items.push(all[next].clone());
            next += 1;
        }
        for x in n.notice(Instant(now), &Store::with(items.clone())) {
            notices.push((now, x));
        }
        if let Some(c) = n.take_cost() {
            costs.push(c.compute_ns);
        }
        let r = n.retirable(Instant(now));
        for id in &r {
            n.retire(*id);
        }
        if !r.is_empty() {
            retirable.push((now, r));
        }
        now += 500 * MS;
    }
    (notices, retirable, costs)
}

fn services_of(seed: u64) -> Vec<gordian_world::Service> {
    public_of(&params(seed, 150)).services
}

// ---- the update rule on hand-made evidence

/// Evidence with `excess` events beyond chance in the first `edge` bins and chance only after.
fn gap_evidence(edge_index: usize, excess: f64) -> Learned {
    let mut l = Learned::prior(400_000_000, 2.0);
    let mut obs = 0.0;
    let mut nul = 0.0;
    for b in 0..NB {
        // Chance: 100 events per bin; beyond chance: `excess` per bin up to and including edge_index.
        nul += 100.0;
        obs += 100.0 + if b <= edge_index { excess } else { 0.0 };
        l.gap_obs[b] = obs;
        l.gap_nul[b] = nul;
    }
    l
}

#[test]
fn the_window_estimate_is_the_last_band_that_is_beyond_chance() {
    // Bands 0..=7 (up to 30 ms) hold 400 events each against 100 expected; later bands hold
    // only what chance gives. A band is chance-dominated if expected > half the observed.
    let l = gap_evidence(7, 300.0);
    assert_eq!(l.window_estimate(0.5), Some(GAP_EDGES_S[7]));
    // With a higher precision bar the same bands are 400 against 100: 0.75 is met, 0.8 is not.
    assert_eq!(l.window_estimate(0.7), Some(GAP_EDGES_S[7]));
    assert_eq!(l.window_estimate(0.8), Some(GAP_EDGES_S[0]));
    // Thin excess: 150 against 100 is below one half beyond chance.
    let l = gap_evidence(7, 50.0);
    assert_eq!(l.window_estimate(0.5), Some(GAP_EDGES_S[0]));
}

#[test]
fn no_window_estimate_before_enough_events() {
    let mut l = gap_evidence(7, 300.0);
    for b in 0..NB {
        l.gap_obs[b] /= 100.0;
        l.gap_nul[b] /= 100.0;
    }
    assert!(l.gap_obs[NB - 1] < 200.0);
    assert_eq!(l.window_estimate(0.5), None);
}

#[test]
fn the_threshold_estimate_is_the_lowest_edge_with_every_bin_above_it_beyond_chance() {
    let mut l = Learned::prior(400_000_000, 2.0);
    // Bins 1 (1.5), 2 (2.0): chance only; bins 3 (2.5) and up: structure.
    l.peak_obs = [900.0, 40.0, 30.0, 60.0, 50.0, 40.0, 30.0, 20.0, 10.0];
    l.peak_nul = [800.0, 38.0, 28.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    assert_eq!(l.threshold_estimate(0.5), Some(PEAK_EDGES[3]));
    // Evidence in bin 1 and 2 beyond chance as well: the threshold falls to the lowest candidate.
    l.peak_nul[1] = 5.0;
    l.peak_nul[2] = 5.0;
    assert_eq!(l.threshold_estimate(0.5), Some(PEAK_EDGES[1]));
    // A chance-dominated bin high up stops the scan there.
    l.peak_nul[5] = 40.0;
    l.peak_obs[5] = 40.0;
    assert_eq!(l.threshold_estimate(0.5), Some(PEAK_EDGES[6]));
    // Too little evidence: no estimate.
    let mut thin = Learned::prior(400_000_000, 2.0);
    thin.peak_obs[3] = 20.0;
    assert_eq!(thin.threshold_estimate(0.5), None);
}

#[test]
fn an_update_moves_a_share_of_the_way_in_log_space_for_the_window_and_linearly_for_the_threshold() {
    let mut l = gap_evidence(7, 300.0);
    l.peak_obs = [900.0, 40.0, 30.0, 60.0, 50.0, 40.0, 30.0, 20.0, 10.0];
    l.peak_nul = [800.0, 38.0, 28.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let (w0, t0) = (l.window_ns, l.ramp_threshold);
    assert!(l.update(0.25, 0.5));
    let w_star = GAP_EDGES_S[7] * 1e9;
    let want_w = ((0.75 * w0.ln()) + 0.25 * w_star.ln()).exp();
    assert!((l.window_ns - want_w).abs() < 1e-6 * want_w);
    let want_t = 0.75 * t0 + 0.25 * PEAK_EDGES[3];
    assert!((l.ramp_threshold - want_t).abs() < 1e-12);
    // Repeated updates converge to the supported values and stop moving them.
    for _ in 0..200 {
        l.update(0.25, 0.5);
    }
    assert!((l.window_ns - w_star).abs() < 1.0);
    assert!((l.ramp_threshold - PEAK_EDGES[3]).abs() < 1e-9);
    assert_eq!(l.boundaries, 201);
    // A step of 1 jumps.
    let mut j = gap_evidence(7, 300.0);
    j.update(1.0, 0.5);
    assert!((j.window_ns - w_star).abs() < 1e-3);
}

fn held_obs(id: u32, at_ms: u64, obs: Observation, services: &[gordian_world::Service]) -> Held {
    Held {
        id: ObsId(id),
        at: Instant(at_ms * MS),
        abnormal: is_abnormal(&obs, services),
        obs,
    }
}

fn reading(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

#[test]
fn the_gap_evidence_counts_the_nearest_other_kind_and_expects_chance_from_the_rates() {
    let services = services_of(0);
    let mut s = Session::new(services.len(), 4.0, 10.0, 3.0, 100, 30.0);
    let mut l = Learned::prior(400_000_000, 2.0);
    // At service 1: error rate (abnormal) at 1000 ms, latency (abnormal) at 1007 ms, latency again
    // at 1020 ms: the second event's nearest other-kind predecessor is 7 ms back; the third's is
    // the error rate, 20 ms back (its own kind's 1007 is not another kind).
    let obs = [
        (0, 1000, reading(1, CounterName::ErrorRate, 90)),
        (1, 1007, reading(1, CounterName::Latency, 80)),
        (2, 1020, reading(1, CounterName::Latency, 85)),
    ];
    for (id, ms, o) in obs {
        s.push(&held_obs(id, ms, o, &services));
    }
    s.process_until(&mut l, 2.0);
    // The 5 ms edge holds none, the 8 ms edge holds the 7 ms gap, the 20 ms edge both.
    assert_eq!(l.gap_obs[3], 0.0);
    assert_eq!(l.gap_obs[4], 1.0);
    assert_eq!(l.gap_obs[5], 1.0);
    assert_eq!(l.gap_obs[6], 2.0);
    assert_eq!(l.gap_obs[NB - 1], 2.0);
    // The expectation is positive, non-decreasing in the edge, and below one per event.
    assert!(l.gap_nul[0] > 0.0);
    assert!(l.gap_nul.windows(2).all(|w| w[0] <= w[1]));
    assert!(l.gap_nul[NB - 1] < 3.0);
    // Three events were counted, by kind: error rate, latency, latency.
    assert_eq!(l.slot_events, [1.0, 2.0, 0.0, 0.0]);
    assert_eq!(slot_of(&reading(1, CounterName::Saturation, 90)), 3);
}

#[test]
fn nothing_after_the_boundary_is_absorbed_before_it() {
    let services = services_of(0);
    let mut s = Session::new(services.len(), 4.0, 10.0, 3.0, 100, 30.0);
    let mut l = Learned::prior(400_000_000, 2.0);
    s.push(&held_obs(
        0,
        9_990,
        reading(1, CounterName::ErrorRate, 90),
        &services,
    ));
    s.push(&held_obs(
        1,
        10_005,
        reading(1, CounterName::Latency, 90),
        &services,
    ));
    s.process_until(&mut l, 10.0);
    assert_eq!(l.slot_events.iter().sum::<f64>(), 1.0);
    s.process_until(&mut l, 20.0);
    assert_eq!(l.slot_events.iter().sum::<f64>(), 2.0);
}

#[test]
fn smooth_dense_readings_peak_above_what_surrogate_steps_give() {
    let services = services_of(0);
    let mut s = Session::new(services.len(), 4.0, 10.0, 3.0, 100, 30.0);
    let mut l = Learned::prior(400_000_000, 2.0);
    // Far-apart readings of broad steps first, so that the surrogate has a distribution.
    let mut id = 0;
    for k in 0..300u64 {
        let v = (k * 37) % 100;
        s.push(&held_obs(
            id,
            1_000 + k * 20_000,
            reading(2, CounterName::Latency, v),
            &services,
        ));
        id += 1;
    }
    // A ramp at service 3: a reading a second for 12 s, each 4 higher than the last.
    let t0 = 7_000_000;
    for k in 0..12u64 {
        s.push(&held_obs(
            id,
            t0 + k * 1_000,
            reading(3, CounterName::Saturation, 10 + 4 * k),
            &services,
        ));
        id += 1;
    }
    // A closing reading far later so that the excursion is recorded.
    s.push(&held_obs(
        id,
        t0 + 200_000,
        reading(3, CounterName::Saturation, 5),
        &services,
    ));
    s.process_until(&mut l, 1.0e4);
    assert!(l.far.iter().sum::<f64>() > 100.0, "far steps were counted");
    assert_eq!(l.far.len(), FAR_BINS);
    // The ramp's excursion peaked at about 4 readings (tau 4 s, a reading a second).
    let high: f64 = l.peak_obs[3..].iter().sum();
    assert!(high >= 1.0, "peaks {:?}", l.peak_obs);
    let surrogate_high: f64 = l.peak_nul[3..].iter().sum();
    assert_eq!(surrogate_high, 0.0, "surrogate {:?}", l.peak_nul);
    assert_eq!(l.peak_obs.len(), NP);
}

// ---- the transplant

#[test]
fn a_transplant_keeps_the_state_and_takes_the_parameters_of_the_fresh_medium() {
    let seed = 10_004;
    let services = services_of(seed);
    let a = learned_at_frozen();
    let sa = Learned::prior(20_000_000, 3.0);
    let mut sb = Learned::prior(37_000_000, 2.5);
    sb.segments = 9;
    // Play a stream through a noticer built from the first state, then transplant the second's
    // parameters into its medium.
    let mut n = LearnedNoticer::new(a, RungConfig::default(), &services).unwrap();
    replay(&mut n, seed, 60);
    let old = n.medium().clone_for_test();
    let (spec_b, _) = learned_spec(&a, &sb, &services).unwrap();
    let fresh = gordian_medium::Medium::from_spec(&spec_b).unwrap();
    let new = transplant(&old, &fresh).unwrap();
    let _ = sa;
    let changed = old
        .cells()
        .iter()
        .zip(new.cells())
        .filter(|(o, n)| o.params != n.params)
        .count();
    assert!(changed > 0, "the parameters differ in some cells");
    for (o, n) in old.cells().iter().zip(new.cells()) {
        assert_eq!(o.state, n.state);
        assert_eq!(o.support, n.support);
        assert_eq!(o.last_active, n.last_active);
    }
    assert_eq!(old.last_tick(), new.last_tick());
    // Same graph, so the same bytes except in the parameter words: round trip is stable.
    let again = transplant(&new, &fresh).unwrap();
    assert_eq!(again.to_bytes(), new.to_bytes());
}

/// `Medium` is `Clone`-free; a copy through its bytes stands in for one in these tests.
trait CloneForTest {
    fn clone_for_test(&self) -> gordian_medium::Medium;
}

impl CloneForTest for gordian_medium::Medium {
    fn clone_for_test(&self) -> gordian_medium::Medium {
        gordian_medium::Medium::from_bytes(&self.to_bytes()).unwrap()
    }
}

#[test]
fn a_noticer_that_learns_swaps_its_graph_at_boundaries_and_every_swap_checks() {
    let seed = 10_005;
    let services = services_of(seed);
    let key = 9_001;
    carry::reset(key);
    let p = LearnedParams {
        carry: true,
        learning: true,
        ..LearnedParams::with_structure(frozen(), key)
    };
    let mut n = LearnedNoticer::new(p, RungConfig::default(), &services).unwrap();
    replay(&mut n, seed, 600);
    let st = n.stats();
    assert_eq!(st.swap_errors, 0);
    assert_eq!(st.step_errors, 0);
    assert_eq!(
        st.boundaries, 59,
        "600 s is 59 complete 10 s cycles at the last step"
    );
    assert!(st.swaps > 0, "the evidence gate was reached in one stream");
    assert!(
        n.learned().window_ns < 400_000_000.0,
        "the window moved from its prior"
    );
    assert!(n.learner_ops_total() > 0);
    // What is carried is what the noticer holds.
    let held = carry::load(key).unwrap();
    assert_eq!(held.boundaries, n.learned().boundaries);
    assert_eq!(held.segments, 1);
    carry::reset(key);
}

// ---- equivalence with M2's medium when nothing is learned

#[test]
fn at_the_frozen_constants_with_learning_off_it_is_m2s_medium() {
    for seed in [10_001u64, 10_007] {
        let services = services_of(seed);
        let mut m = MediumNoticer::new(frozen(), RungConfig::default(), &services).unwrap();
        let mut l =
            LearnedNoticer::new(learned_at_frozen(), RungConfig::default(), &services).unwrap();
        let (mn, mr, mc) = replay(&mut m, seed, 150);
        let (ln, lr, lc) = replay(&mut l, seed, 150);
        assert!(!mn.is_empty());
        assert_eq!(mn, ln, "seed {seed}: notices");
        assert_eq!(mr, lr, "seed {seed}: retirements");
        assert_eq!(mc, lc, "seed {seed}: the bill");
        assert_eq!(l.stats().swaps, 0);
        assert_eq!(m.total_ns(), l.total_ns());
    }
}

#[test]
fn the_structures_own_values_for_the_learned_constants_are_never_read() {
    let seed = 10_002;
    let services = services_of(seed);
    let a = learned_at_frozen();
    let mut b = a;
    b.structure.burst_window_ns = 77 * MS;
    b.structure.burst3_window_ns = 55 * MS;
    b.structure.burst_lookback_ns = 900 * MS;
    b.structure.ramp_threshold = 9.0;
    let mut na = LearnedNoticer::new(a, RungConfig::default(), &services).unwrap();
    let mut nb = LearnedNoticer::new(b, RungConfig::default(), &services).unwrap();
    assert_eq!(replay(&mut na, seed, 100).0, replay(&mut nb, seed, 100).0);
}

// ---- carry and determinism

#[test]
fn what_is_learned_is_carried_to_the_next_segment_and_reproduced_by_a_replay() {
    let run = |key: u64| -> (Learned, Learned) {
        carry::reset(key);
        let p = LearnedParams::with_structure(frozen(), key);
        let mut end = Vec::new();
        for seed in [10_010u64, 10_011] {
            let services = services_of(seed);
            let mut n = LearnedNoticer::new(p, RungConfig::default(), &services).unwrap();
            if !end.is_empty() {
                // The second segment begins where the first ended.
                let first: &Learned = &end[0];
                assert_eq!(n.learned().boundaries, first.boundaries);
                assert_eq!(n.learned().window_ns, first.window_ns);
            }
            replay(&mut n, seed, 100);
            end.push(n.learned().clone());
        }
        carry::reset(key);
        (end[0].clone(), end[1].clone())
    };
    let a = run(9_101);
    let b = run(9_102);
    assert_eq!(a, b, "a replay reproduces the carried state exactly");
    assert!(a.1.boundaries > a.0.boundaries);
    assert_eq!(a.1.segments, 2);
}

#[test]
fn without_carry_every_segment_starts_from_the_priors() {
    let key = 9_201;
    carry::reset(key);
    let p = LearnedParams {
        carry: false,
        ..LearnedParams::with_structure(frozen(), key)
    };
    for seed in [10_010u64, 10_011] {
        let services = services_of(seed);
        let n = LearnedNoticer::new(p, RungConfig::default(), &services).unwrap();
        assert_eq!(n.learned().boundaries, 0);
        assert_eq!(n.learned().window_ns, 400_000_000.0);
    }
    assert!(carry::load(key).is_none());
}

#[test]
fn learning_off_keeps_the_priors_and_no_evidence() {
    let seed = 10_012;
    let services = services_of(seed);
    let p = LearnedParams {
        learning: false,
        ..LearnedParams::with_structure(frozen(), 9_301)
    };
    let mut n = LearnedNoticer::new(p, RungConfig::default(), &services).unwrap();
    replay(&mut n, seed, 100);
    assert_eq!(n.learned(), &Learned::prior(400_000_000, 2.0));
    assert_eq!(n.stats().swaps, 0);
    assert_eq!(n.learner_ops_total(), 0);
}

#[test]
fn the_prior_window_is_the_rungs_burst_constant_unless_the_manifest_names_one() {
    let p = LearnedParams::with_structure(frozen(), 1);
    assert_eq!(p.prior_window(400 * MS), 400 * MS);
    assert_eq!(p.prior_window(123_456_789), 123_457_000);
    let q = LearnedParams {
        prior_window_ns: 50 * MS,
        ..p
    };
    assert_eq!(q.prior_window(400 * MS), 50 * MS);
    assert_eq!(round_window(0.0), 1_000);
    assert_eq!(round_window(5e12), 1_000_000_000);
    // The lookback is the window rounded down to whole ticks.
    assert_eq!(p.medium_params(450 * MS, 2.0).burst_lookback_ns, 400 * MS);
    assert_eq!(p.medium_params(30 * MS, 2.0).burst_lookback_ns, 0);
    assert_eq!(p.medium_params(30 * MS, 2.0).burst3_window_ns, 30 * MS);
}

// ---- the manifest

#[test]
fn the_learned_noticer_is_written_read_and_validated() {
    let p = LearnedParams::with_structure(frozen(), 77);
    let spec = NoticerSpec::Learned(p);
    spec.validate().unwrap();
    let text = serde_json::to_string(&spec).unwrap();
    assert!(text.contains("\"noticer\":\"learned\""));
    let back: NoticerSpec = serde_json::from_str(&text).unwrap();
    assert_eq!(back, spec);
    assert_eq!(spec.id(), "learned");
    for bad in [
        LearnedParams { step: 0.0, ..p },
        LearnedParams { step: 1.5, ..p },
        LearnedParams {
            min_precision: 1.0,
            ..p
        },
        LearnedParams {
            prior_ramp_threshold: 1.0,
            ..p
        },
        LearnedParams {
            prior_window_ns: 5,
            ..p
        },
    ] {
        assert!(NoticerSpec::Learned(bad).validate().is_err());
    }
    let services = services_of(1);
    let rung = RungConfig::default();
    let n = build(&p, &rung, &services);
    assert_eq!(n.id(), "learned");
}

// ---- a tool: the learner's trajectory over a run's seeds

/// Replays the learned noticer named by `L1_NOTICER` (the manifest's JSON for it) over the
/// streams `L1_SEEDS` (`first:count`, in order, the arm carrying what it learns) and writes one row
/// per stream to `L1_OUT`: the notices the arm made, the learned values at the stream's end, and
/// the learner's evidence. The learner reads delivered observations only, so this replay's state
/// is the run's; the notice counts are for checking it against the run's `notices.csv`.
#[test]
#[ignore = "a tool for the PI: writes a file"]
fn learned_trajectory() {
    use std::fmt::Write as _;
    let seeds = std::env::var("L1_SEEDS").unwrap_or_else(|_| "10000:3".to_owned());
    let out = std::env::var("L1_OUT").unwrap_or_else(|_| "l1-trajectory.csv".to_owned());
    let json = std::env::var("L1_NOTICER").expect("L1_NOTICER");
    let (first, count) = seeds.split_once(':').expect("first:count");
    let (first, count): (u64, u64) = (first.parse().unwrap(), count.parse().unwrap());
    let NoticerSpec::Learned(p) = serde_json::from_str::<NoticerSpec>(&json).unwrap() else {
        panic!("not a learned noticer");
    };
    carry::reset(p.state_key);
    let mut text = String::from(
        "seed,notices,window_ms,ramp_threshold,boundaries,window_updates,threshold_updates,swaps,swap_errors,learner_ops,gap_obs_top,gap_nul_top,peak_obs_hi,peak_nul_hi,window_estimate_ms,threshold_estimate\n",
    );
    for seed in first..first + count {
        let sp = gordian_stream::StreamParams::new(seed);
        let services = generate(&sp).public_info().services;
        let mut n = LearnedNoticer::new(p, RungConfig::default(), &services).unwrap();
        let (notices, _, _) = replay_params(&mut n, &sp);
        let l = n.learned();
        let st = n.stats();
        writeln!(
            text,
            "{seed},{},{:.4},{:.4},{},{},{},{},{},{},{},{:.1},{},{:.1},{},{}",
            notices.len(),
            l.window_ns / 1e6,
            l.ramp_threshold,
            l.boundaries,
            l.window_updates,
            l.threshold_updates,
            st.swaps,
            st.swap_errors,
            n.learner_ops_total(),
            l.gap_obs[NB - 1],
            l.gap_nul[NB - 1],
            l.peak_obs[3..].iter().sum::<f64>(),
            l.peak_nul[3..].iter().sum::<f64>(),
            l.window_estimate(f64::from(p.min_precision))
                .map_or(String::new(), |w| format!("{:.1}", w * 1e3)),
            l.threshold_estimate(f64::from(p.min_precision))
                .map_or(String::new(), |t| format!("{t}")),
        )
        .unwrap();
    }
    carry::reset(p.state_key);
    std::fs::write(&out, text).expect("write the trajectory");
}
