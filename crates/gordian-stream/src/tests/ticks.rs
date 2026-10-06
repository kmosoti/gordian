//! W1: the tick statistics of `examples/ticks`.
//!
//! The example keeps its logic in `examples/ticks/stats.rs`; this module compiles that file
//! into the crate's tests, so the same code is tested on hand-written input and on generated
//! streams. The example's own `main` only formats what `stats` returns.

// `stats.rs` names this crate as `gordian_stream`, as an example does, so it is included
// (not declared with `#[path]`) into a module that declares that name.
#[allow(dead_code)]
mod stats {
    extern crate self as gordian_stream;
    include!("../../examples/ticks/stats.rs");
}

use super::*;
use crate::HardKind;
use crate::oracle::{NoiseKind, ObsLabel};
use stats::*;
use std::collections::BTreeMap;

const SEEDS: std::ops::Range<u64> = 33_000..33_030;

fn ev(at_ms: u64, node: u32, abnormal: bool) -> Ev {
    Ev {
        at_ns: at_ms * MS,
        node: Some(node),
        abnormal,
    }
}

fn io(at_ms: u64, node: u32, abnormal: bool, decisive: bool) -> IncObs {
    IncObs {
        at_ns: at_ms * MS,
        node: Some(node),
        abnormal,
        decisive,
    }
}

#[test]
fn histograms_count_every_event_and_every_empty_cell() {
    // 1 s, two nodes, 300 ms ticks: ceil(1000 / 300) = 4 ticks and 8 cells.
    let events = [
        ev(10, 0, false),
        ev(20, 0, true),
        ev(250, 1, true),
        ev(310, 0, false),
        ev(999, 1, false),
    ];
    let h = tick_hists(&events, 1_000 * MS, 2, 300 * MS);
    // Ticks 0..4 hold 3, 1, 0, 1 events (the instant 999 ms is in tick 3).
    assert_eq!(h.all.0, BTreeMap::from([(0, 1), (1, 2), (3, 1)]));
    assert_eq!(h.all.n(), 4);
    assert_eq!(h.all.total(), 5);
    // Abnormal: tick 0 holds two, the others none.
    assert_eq!(h.abnormal.0, BTreeMap::from([(0, 3), (2, 1)]));
    assert_eq!(h.abnormal.total(), 2);
    // Cells: (0,t0)=2, (1,t0)=1, (0,t1)=1, (1,t3)=1; four more empty.
    assert_eq!(h.node_all.0, BTreeMap::from([(0, 4), (1, 3), (2, 1)]));
    assert_eq!(h.node_all.n(), 8);
    assert_eq!(h.node_all.total(), 5);
    assert_eq!(h.node_abnormal.0, BTreeMap::from([(0, 6), (1, 2)]));
}

#[test]
fn an_instant_on_a_boundary_belongs_to_the_later_tick() {
    assert_eq!(tick_of(99_999_999, 100 * MS), 0);
    assert_eq!(tick_of(100 * MS, 100 * MS), 1);
    let h = tick_hists(&[ev(100, 0, false)], 200 * MS, 1, 100 * MS);
    assert_eq!(h.all.0, BTreeMap::from([(0, 1), (1, 1)]));
    // An event past the nominal duration extends the tick count rather than being dropped.
    let late = tick_hists(&[ev(450, 0, false)], 200 * MS, 1, 100 * MS);
    assert_eq!(late.all.n(), 5);
    assert_eq!(late.all.total(), 1);
}

#[test]
fn histogram_summaries() {
    let mut h = Hist::default();
    for (v, n) in [(0, 6), (1, 2), (4, 2)] {
        h.add(v, n);
    }
    assert_eq!(h.n(), 10);
    assert!((h.mean() - 1.0).abs() < 1e-12);
    assert!((h.variance() - 2.4).abs() < 1e-12); // (6 * 1 + 2 * 0 + 2 * 9) / 10
    assert_eq!(h.quantile(0.5), 0);
    assert_eq!(h.quantile(0.7), 1);
    assert_eq!(h.quantile(0.9), 4);
    assert_eq!(h.max(), 4);
    assert!((h.share_at_most(0) - 0.6).abs() < 1e-12);
    assert!((h.share_at_most(3) - 0.8).abs() < 1e-12);
}

#[test]
fn lags_are_counted_on_the_grid_and_missing_instants_are_not_dropped() {
    // A cascade: first observation at 95 ms, the root's first alarm at 105 ms, the partner's
    // alarm at 230 ms; a benign reading and a non-abnormal reading at the partner do not count.
    let obs = [
        io(95, 0, false, false),
        io(96, 0, false, true),
        io(105, 0, true, true),
        io(150, 1, false, false),
        io(230, 1, true, false),
        io(6_000, 1, true, true),
    ];
    let i = incident_instants(&obs, Some(1));
    assert_eq!(i.first_ns, 95 * MS);
    assert_eq!(i.first_abnormal_ns, Some(105 * MS));
    assert_eq!(i.partner_alarm_ns, Some(230 * MS));

    let mut s100 = LagSamples::default();
    add_incident(&mut s100, &obs, Some(1), 100 * MS);
    assert_eq!(s100.to_abnormal.0, BTreeMap::from([(1, 1)]));
    assert_eq!(s100.to_partner.0, BTreeMap::from([(2, 1)]));
    assert_eq!(s100.abnormal_to_partner.0, BTreeMap::from([(1, 1)]));
    // Decisive: 96 ms, 105 ms and 6 s; the first observation's tick is [0, 100): one of three.
    assert_eq!(
        (s100.decisive_any, s100.decisive_all, s100.with_decisive),
        (1, 0, 1)
    );
    assert!((s100.decisive_share_sum - 1.0 / 3.0).abs() < 1e-12);

    let mut s500 = LagSamples::default();
    add_incident(&mut s500, &obs, Some(1), 500 * MS);
    assert_eq!(s500.to_abnormal.0, BTreeMap::from([(0, 1)]));
    assert_eq!(s500.to_partner.0, BTreeMap::from([(0, 1)]));
    assert_eq!(s500.abnormal_to_partner.0, BTreeMap::from([(0, 1)]));
    assert!((s500.decisive_share_sum - 2.0 / 3.0).abs() < 1e-12);

    // No alarm at the partner: counted as missing at every tick length.
    let silent = [io(0, 0, true, true), io(40, 1, false, false)];
    let mut s = LagSamples::default();
    add_incident(&mut s, &silent, Some(1), 100 * MS);
    assert_eq!(
        (s.no_partner_alarm, s.to_partner.n(), s.incidents),
        (1, 0, 1)
    );
    assert_eq!(s.decisive_all, 1);
    // No abnormal observation at all, and no partner in the family: only `no_abnormal` moves.
    let quiet = [io(0, 0, false, false), io(900, 0, false, true)];
    let mut q = LagSamples::default();
    add_incident(&mut q, &quiet, None, 100 * MS);
    assert_eq!(
        (q.no_abnormal, q.no_partner_alarm, q.to_abnormal.n()),
        (1, 0, 0)
    );
    assert_eq!(q.decisive_any, 0);
}

#[test]
fn the_same_lags_in_milliseconds_do_not_depend_on_a_grid() {
    let obs = [
        io(95, 0, false, false),
        io(105, 0, true, false),
        io(230, 1, true, false),
    ];
    let mut ms = MsSamples::default();
    add_incident_ms(&mut ms, &obs, Some(1));
    assert_eq!(ms.to_abnormal.0, BTreeMap::from([(10, 1)]));
    assert_eq!(ms.to_partner.0, BTreeMap::from([(135, 1)]));
    assert_eq!(ms.abnormal_to_partner.0, BTreeMap::from([(125, 1)]));
}

fn totals(seeds: std::ops::Range<u64>) -> Totals {
    let mut t = Totals::new(&TICK_NS);
    for seed in seeds {
        t.add_stream(&generate(&StreamParams::new(seed)), &TICK_NS);
    }
    t
}

#[test]
fn every_event_is_counted_once_at_every_tick_length() {
    let mut events = 0u64;
    let mut abnormal = 0u64;
    let mut services = 0u64;
    let mut duration = 0u64;
    for seed in SEEDS {
        let s = generate(&StreamParams::new(seed));
        events += s.events().len() as u64;
        abnormal += stream_events(&s).iter().filter(|e| e.abnormal).count() as u64;
        services += s.public_info().services.len() as u64;
        duration += s.public_info().duration_ns;
    }
    let t = totals(SEEDS);
    assert_eq!(
        (t.events, t.abnormal_events, t.duration_ns),
        (events, abnormal, duration)
    );
    assert!(events > 0 && abnormal > 0 && abnormal < events);
    for (k, &tick_ns) in TICK_NS.iter().enumerate() {
        let h = &t.hists[k];
        let ticks = duration.div_ceil(tick_ns);
        assert_eq!(h.all.n(), ticks, "tick {tick_ns}");
        assert_eq!(h.abnormal.n(), ticks);
        assert_eq!(h.all.total(), events);
        assert_eq!(h.abnormal.total(), abnormal);
        // Every stream has the same duration, so cells = ticks per stream * services.
        let per_stream = duration / SEEDS.count() as u64;
        let cells = per_stream.div_ceil(tick_ns) * services;
        assert_eq!(h.node_all.n(), cells);
        assert_eq!(h.node_abnormal.n(), cells);
        assert_eq!(h.node_all.total(), events);
        assert_eq!(h.node_abnormal.total(), abnormal);
        // A coarser tick merges finer ones: fewer ticks, so a larger mean, and no mass is lost.
        if k > 0 {
            assert!(h.all.mean() > t.hists[k - 1].all.mean());
            assert!(h.all.share_at_most(0) <= t.hists[k - 1].all.share_at_most(0));
        }
    }
}

#[test]
fn the_abnormal_flag_agrees_with_what_the_world_says_each_noise_process_is() {
    let (mut blips, mut benign, mut free, mut snaps) = (0, 0, 0, 0);
    for seed in SEEDS {
        let (s, t) = with_truth(&StreamParams::new(seed));
        let evs = stream_events(&s);
        for (i, l) in t.labels.iter().enumerate() {
            let ObsLabel::Background(k) = l else { continue };
            match k {
                NoiseKind::Blip => {
                    blips += 1;
                    assert!(evs[i].abnormal, "a blip is an abnormal counter");
                }
                NoiseKind::Benign => {
                    benign += 1;
                    assert!(!evs[i].abnormal, "a benign counter is below the threshold");
                }
                NoiseKind::FreeForm => {
                    free += 1;
                    assert!(!evs[i].abnormal, "free-form text has no public meaning");
                }
                NoiseKind::Snapshot => {
                    snaps += 1;
                    assert!(!evs[i].abnormal, "an unchanged snapshot");
                }
                NoiseKind::CatalogueStray | NoiseKind::MiniBurst => {}
            }
        }
    }
    assert!(blips > 0 && benign > 0 && free > 0 && snaps > 0);
}

/// One hard incident's instants, its onset, and whether its partner is a dependent of its site
/// (so that the site's own burst alarms it).
type HardRow = (IncidentInstants, u64, bool);

fn hard_lags(seeds: std::ops::Range<u64>) -> BTreeMap<(HardKind, Option<bool>), Vec<HardRow>> {
    let mut out: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for seed in seeds {
        let (s, t) = with_truth(&StreamParams::new(seed));
        let evs = stream_events(&s);
        for inc in &t.incidents {
            let Some(g) = group_of(inc) else { continue };
            let obs = incident_obs(&evs, &t, inc);
            let partner = inc.shape.other.map(|p| p.index() as u32);
            // The first observation is the incident's first by stream order and by instant.
            assert_eq!(obs[0].at_ns, obs.iter().map(|o| o.at_ns).min().unwrap());
            assert_eq!(obs[0].at_ns, evs[inc.observations[0].0 as usize].at_ns);
            let site = inc.occupies[0];
            let dependent = inc
                .shape
                .other
                .is_some_and(|p| crate::incident::dependents(&t.services, site).contains(&p));
            out.entry((g.family, g.contradict)).or_default().push((
                incident_instants(&obs, partner),
                inc.onset_ns,
                dependent,
            ));
        }
    }
    out
}

#[test]
fn partner_alarms_follow_the_documented_timing() {
    // HIDDEN-DESIGN.md section 4.2: a Contradict cascade or split brain alarms its partner
    // 20 to 230 ms after the first alarm (the table gives that range for the cascade; the split
    // brain's peer is read against the same instants); a Mimic one shows the partner only in
    // phase 2, 6 s or more after onset. Every observation carries a sub-millisecond jitter, so
    // the bounds are given a millisecond either way.
    let lags = hard_lags(SEEDS);
    let mut seen = 0;
    for (&(family, contradict), rows) in &lags {
        match (family, contradict) {
            (HardKind::Cascade, Some(true)) => {
                for (i, _, _) in rows {
                    let d = i.partner_alarm_ns.unwrap() - i.first_abnormal_ns.unwrap();
                    assert!(
                        (19 * MS..=231 * MS).contains(&d),
                        "contradict cascade partner after {} ms",
                        d / MS
                    );
                    seen += 1;
                }
            }
            (HardKind::Cascade | HardKind::SplitBrain, Some(false)) => {
                for (i, onset, dependent) in rows {
                    let at = i
                        .partner_alarm_ns
                        .expect("a mimic's partner alarms in phase 2");
                    let early = at < onset + 6_000 * MS - MS;
                    if early {
                        // A split brain's peer can be a dependent of the site, and the site's
                        // own burst alarms up to three dependents (a `Latency` reading 21 to
                        // 60 ms in). A cascade's partner is not connected to the site, so
                        // never.
                        assert!(
                            *dependent,
                            "{family:?} mimic partner early, not a dependent"
                        );
                        assert_eq!(family, HardKind::SplitBrain);
                        assert!(at <= onset + 61 * MS, "dependent peer alarms in the burst");
                    }
                    seen += 1;
                }
            }
            (HardKind::SplitBrain, Some(true)) => {
                for (i, _, _) in rows {
                    assert!(i.partner_alarm_ns.is_some());
                    seen += 1;
                }
            }
            (HardKind::Compound | HardKind::SlowLeak, _) => {
                for (i, _, _) in rows {
                    assert_eq!(i.partner_alarm_ns, None, "{family:?} has no partner");
                }
            }
            (f, c) => panic!("unexpected group {f:?} {c:?}"),
        }
    }
    assert!(
        seen > 10,
        "seen {seen}: the test must meet each partner family"
    );
}

#[test]
fn a_slow_leak_has_no_early_abnormal_observation() {
    // The leak's phase 1 is benign readings only; the first abnormal observation is the
    // threshold crossing, 6 to 14 s after onset by the design record (a second of slack).
    let lags = hard_lags(SEEDS);
    let rows = &lags[&(HardKind::SlowLeak, None)];
    assert!(!rows.is_empty());
    for (i, onset, _) in rows {
        if let Some(a) = i.first_abnormal_ns {
            assert!(
                a >= onset + 5 * 1_000 * MS,
                "leak alarm {} ms after onset",
                (a - onset) / MS
            );
        }
    }
}

#[test]
fn totals_are_a_pure_function_of_the_seeds() {
    assert_eq!(totals(33_000..33_004), totals(33_000..33_004));
    // And a stream seen at three tick lengths is the same stream: the millisecond table does not
    // depend on which tick lengths were asked for.
    let mut one = Totals::new(&TICK_NS[..1]);
    let mut three = Totals::new(&TICK_NS);
    for seed in 33_000..33_004 {
        let s = generate(&StreamParams::new(seed));
        one.add_stream(&s, &TICK_NS[..1]);
        three.add_stream(&s, &TICK_NS);
    }
    assert_eq!(one.ms, three.ms);
    assert_eq!(one.hists[0], three.hists[0]);
    assert_eq!(one.lags[0], three.lags[0]);
}
