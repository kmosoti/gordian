//! The ramp noticer (work item B3): the chain rule as a pure function of readings, every boundary of
//! the readings the module documentation states, the composed noticer through the rung, its identity
//! with the base when it cannot cross, and the files.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`scripts/b3_gate.py`); the tests here pin the same property on small streams.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::noticer::{BaseSpec, NoticeKind, NoticerSpec};
use gordian_run::stream::arms::noticer_ramp::{
    Fed, MAX_CHAIN_READINGS, MAX_CHAINS_PER_KEY, RampDetector, RampSpec, continues,
};
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::noticer_split::SplitSpec;
use gordian_run::stream::arms::rung::{Held, Rung, RungConfig};
use gordian_run::stream::execute_stream;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamEvent, StreamPublic};
use gordian_world::physics::SignalText;
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use serde_json::{Value, json};
use stream_common::*;

const MS: u64 = 1_000_000;
const S: u64 = 1_000;

fn spec() -> RampSpec {
    RampSpec {
        gap_ns: 2_000 * MS,
        max_step: 12,
        max_drop: 2,
        min_readings: 4,
        min_rise: 10,
    }
}

fn composed(ramp: Option<RampSpec>, split: Option<SplitSpec>) -> NoticerSpec {
    NoticerSpec::Composed {
        base: BaseSpec::Rung { notice_z: None },
        ramp,
        split,
    }
}

fn reading(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

fn held(id: u32, ms: u64, obs: Observation) -> Held {
    let abnormal = matches!(obs, Observation::Counter { value, .. } if value >= 50);
    Held {
        id: ObsId(id),
        at: at(ms),
        obs,
        abnormal,
    }
}

/// Feed `(ms, value)` readings of Saturation at service 0 to a fresh detector, ids from 0.
fn feed(det: &mut RampDetector, series: &[(u64, u64)]) -> Vec<Fed> {
    series
        .iter()
        .enumerate()
        .map(|(i, (ms, v))| {
            det.feed(&held(
                i as u32,
                *ms,
                reading(0, CounterName::Saturation, *v),
            ))
        })
        .collect()
}

/// Readings every 1.2 s from `t0` ms with `values`.
fn series(t0: u64, values: &[u64]) -> Vec<(u64, u64)> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| (t0 + 1_200 * i as u64, *v))
        .collect()
}

const LEAK: [u64; 14] = [28, 29, 33, 39, 44, 48, 50, 55, 60, 64, 68, 67, 73, 74];

fn crossed(f: &Fed) -> bool {
    matches!(f, Fed::Crossed { .. })
}

// ---- the spec and the manifest

#[test]
fn the_composed_noticer_is_written_and_read_with_every_parameter() {
    let value = json!({
        "noticer": "composed",
        "base": {"noticer": "rung"},
        "ramp": {"gap_ns": 2_000_000_000u64, "max_step": 12, "max_drop": 2, "min_readings": 4, "min_rise": 10}
    });
    let parsed: NoticerSpec = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed, composed(Some(spec()), None));
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    assert_eq!(parsed.id(), "ramp");
    assert!(!parsed.is_default());
    // The re-anchor as a base, both pieces: every parameter is written and read back.
    let both = json!({
        "noticer": "composed",
        "base": {"noticer": "reanchor", "notice_z": 2.0, "gap_ns": 20_000_000u64, "min_burst": 2, "isolation": "site"},
        "ramp": {"gap_ns": 2_000_000_000u64, "max_step": 12, "max_drop": 2, "min_readings": 4, "min_rise": 10},
        "split": {"gap_ns": 3_000_000_000u64, "min_burst": 3}
    });
    let parsed: NoticerSpec = serde_json::from_value(both.clone()).unwrap();
    assert_eq!(
        parsed,
        NoticerSpec::Composed {
            base: BaseSpec::Reanchor {
                notice_z: Some(2.0),
                gap_ns: 20 * MS,
                min_burst: 2,
                isolation: Isolation::Site
            },
            ramp: Some(spec()),
            split: Some(SplitSpec {
                gap_ns: 3_000 * MS,
                min_burst: 3
            }),
        }
    );
    assert_eq!(serde_json::to_value(parsed).unwrap(), both);
    assert_eq!(parsed.id(), "ramp_split_reanchor");
    // A parameter the noticer does not have, a missing one, and an unknown base are errors.
    for bad in [
        json!({"noticer": "composed", "base": {"noticer": "rung"}, "ramp": {"gap_ns": 1, "max_step": 1, "max_drop": 0, "min_readings": 2, "min_rise": 1, "q": 1}}),
        json!({"noticer": "composed", "base": {"noticer": "rung"}, "ramp": {"gap_ns": 1, "max_step": 1, "max_drop": 0, "min_readings": 2}}),
        json!({"noticer": "composed", "base": {"noticer": "change_triggered", "quiet_ns": 1}, "ramp": {"gap_ns": 1, "max_step": 1, "max_drop": 0, "min_readings": 2, "min_rise": 1}}),
        json!({"noticer": "composed", "ramp": {"gap_ns": 1, "max_step": 1, "max_drop": 0, "min_readings": 2, "min_rise": 1}}),
    ] {
        assert!(serde_json::from_value::<NoticerSpec>(bad).is_err());
    }
}

#[test]
fn the_ids_say_which_pieces_a_composed_noticer_has() {
    let ids = |base: BaseSpec, r: bool, s: bool| {
        NoticerSpec::Composed {
            base,
            ramp: r.then(spec),
            split: s.then_some(SplitSpec {
                gap_ns: S * MS,
                min_burst: 2,
            }),
        }
        .id()
    };
    let rung = BaseSpec::Rung { notice_z: None };
    let re = BaseSpec::Reanchor {
        notice_z: None,
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    };
    assert_eq!(ids(rung, true, false), "ramp");
    assert_eq!(ids(rung, false, true), "split");
    assert_eq!(ids(rung, true, true), "ramp_split");
    assert_eq!(ids(re, true, false), "ramp_reanchor");
    assert_eq!(ids(re, false, true), "split_reanchor");
    assert_eq!(ids(re, true, true), "ramp_split_reanchor");
    // None of them is written with a comma: the run output is unquoted.
    for id in [
        ids(rung, true, true),
        ids(re, true, true),
        ids(re, false, true),
    ] {
        assert!(!id.contains(',') && !id.contains(' '));
    }
}

#[test]
fn the_ramp_parameters_are_checked() {
    assert!(spec().validate().is_ok());
    let bad = |f: &dyn Fn(&mut RampSpec)| {
        let mut s = spec();
        f(&mut s);
        s.validate().unwrap_err()
    };
    assert!(bad(&|s| s.gap_ns = 0).contains("gap_ns"));
    assert!(bad(&|s| s.min_readings = 1).contains("min_readings"));
    assert!(bad(&|s| s.min_rise = 0).contains("min_rise"));
    // The extreme legal values are legal.
    let mut s = spec();
    s.min_readings = 2;
    s.min_rise = 1;
    s.max_drop = 0;
    s.max_step = 0;
    assert!(s.validate().is_ok());
    // A composed noticer needs a piece, and its pieces' parameters are checked with it.
    assert!(
        composed(None, None)
            .validate()
            .unwrap_err()
            .contains("ramp, a split or both")
    );
    let mut b = spec();
    b.min_rise = 0;
    assert!(composed(Some(b), None).validate().is_err());
    let mut m = manifest(
        "ramp-bad",
        &[("never_escalate", "never_escalate")],
        1,
        150,
        0,
    );
    m.noticers
        .insert("never_escalate".to_owned(), composed(None, None));
    assert!(m.validate().is_err());
    m.noticers
        .insert("never_escalate".to_owned(), composed(Some(spec()), None));
    assert!(m.validate().is_ok());
}

// ---- the value rule

#[test]
fn a_reading_continues_a_chain_within_max_step_above_and_max_drop_below_its_level() {
    // Level 40, max_step 12, max_drop 2.
    assert!(continues(40, 40, 2, 12), "equal continues");
    assert!(continues(40, 52, 2, 12), "exactly max_step above");
    assert!(!continues(40, 53, 2, 12), "one more");
    assert!(continues(40, 38, 2, 12), "exactly max_drop below");
    assert!(!continues(40, 37, 2, 12), "one more");
    // max_drop 0 is non-decreasing.
    assert!(continues(40, 40, 0, 12));
    assert!(!continues(40, 39, 0, 12));
    // Near zero and at the top of the range, in unsigned arithmetic.
    assert!(continues(0, 0, 2, 12));
    assert!(continues(1, 0, 2, 12));
    assert!(!continues(0, 13, 2, 12));
    assert!(continues(u64::MAX - 1, u64::MAX, 0, 1));
    assert!(!continues(u64::MAX, 0, 2, 12));
}

// ---- the chain rule

#[test]
fn a_ramp_crosses_at_the_reading_that_gives_it_min_readings_and_min_rise() {
    let mut det = RampDetector::new(spec());
    let s = series(10_000, &LEAK);
    let fed = feed(&mut det, &s);
    // 28, 29, 33, 39: four readings, rise 11 >= 10, at the fourth.
    let idx: Vec<usize> = fed
        .iter()
        .enumerate()
        .filter(|(_, f)| crossed(f))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(idx, vec![3]);
    let Fed::Crossed { readings, key, .. } = &fed[3] else {
        unreachable!()
    };
    assert_eq!(*key, (ServiceId(0), CounterName::Saturation));
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![0, 1, 2, 3],
        "the chain's readings, the first (the anchor) first"
    );
    assert_eq!(readings[0].at, at(10_000));
    // Nothing else crosses: a chain crosses once. Before the link the readings that continue it say
    // nothing to do.
    assert!(fed[4..].iter().all(|f| *f == Fed::Nothing));
}

#[test]
fn the_rise_must_be_min_rise_and_the_chain_min_readings_long() {
    // Rise 9 over the readings: not a ramp at min_rise 10, a ramp at 9.
    let flat = series(0, &[28, 30, 33, 35, 37, 37, 37]);
    let mut det = RampDetector::new(spec());
    assert!(!feed(&mut det, &flat).iter().any(crossed));
    let mut s9 = spec();
    s9.min_rise = 9;
    let mut det = RampDetector::new(s9);
    let idx: Vec<_> = feed(&mut det, &flat).iter().map(crossed).collect();
    assert_eq!(
        idx.iter().position(|c| *c),
        Some(4),
        "37 - 28 = 9 at the fifth"
    );
    // Three readings with a large rise are not four.
    let short = series(0, &[20, 30, 42]);
    let mut det = RampDetector::new(spec());
    assert!(!feed(&mut det, &short).iter().any(crossed));
    // A falling chain has no rise: signed arithmetic, a rise below zero does not count.
    let down = series(0, &[60, 59, 58, 57, 56, 55]);
    let mut det = RampDetector::new(spec());
    assert!(!feed(&mut det, &down).iter().any(crossed));
}

#[test]
fn a_silence_longer_than_the_gap_ends_a_chain_and_one_of_exactly_the_gap_does_not() {
    let values = [28u64, 31, 35, 40, 44, 49];
    let mut exact = Vec::new();
    for (i, v) in values.iter().enumerate() {
        exact.push((i as u64 * 2_000, *v));
    }
    let mut det = RampDetector::new(spec());
    assert!(
        feed(&mut det, &exact).iter().any(crossed),
        "2 s between readings is the gap, inclusive"
    );
    // One more millisecond between the second and the third reading: the chain ends there, and the
    // later readings (35, 40, 44, 49) make a chain of their own, which crosses at the fourth of them.
    let mut late: Vec<(u64, u64)> = exact.clone();
    for p in late.iter_mut().skip(2) {
        p.0 += 1;
    }
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &late);
    assert_eq!(fed.iter().position(crossed), Some(5));
    let Fed::Crossed { readings, .. } = &fed[5] else {
        unreachable!()
    };
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
}

#[test]
fn a_chain_that_restarts_after_a_silence_is_a_new_chain_with_its_own_first_reading() {
    // 28, 31, then ten seconds of nothing, then a ramp: the ramp's anchor is its own first reading.
    let mut s = vec![(0, 28), (1_200, 31)];
    s.extend(series(12_000, &[26, 30, 36, 41]));
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &s);
    let Fed::Crossed { readings, .. } = fed.last().unwrap() else {
        panic!("{fed:?}")
    };
    assert_eq!(readings[0].id.0, 2);
    assert_eq!(readings.len(), 4);
}

#[test]
fn a_stray_in_the_middle_of_a_ramp_neither_ends_it_nor_joins_it() {
    // The ramp's readings every 1.2 s: 28, 29, 33, 39, 44; a stray of 70 between the second and the
    // third. The stray fits no chain (a step of 41 above 29) and starts its own; the ramp continues
    // from 29 with 33 and 39, and crosses at 39: four accepted readings, a rise of 11.
    let s = vec![
        (10_000, 28),
        (11_200, 29),
        (11_800, 70),
        (12_400, 33),
        (13_600, 39),
        (14_800, 44),
    ];
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &s);
    let (i, Fed::Crossed { readings, .. }) = fed
        .iter()
        .enumerate()
        .find(|(_, f)| crossed(f))
        .expect("the ramp crosses")
    else {
        unreachable!()
    };
    assert_eq!(i, 4);
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![0, 1, 3, 4],
        "the stray (id 2) is not in the chain"
    );
    // A dip of more than max_drop is a stray too (20, nine below 29).
    let mut s = s;
    s[2].1 = 20;
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &s);
    let (i, Fed::Crossed { readings, .. }) = fed
        .iter()
        .enumerate()
        .find(|(_, f)| crossed(f))
        .expect("the ramp crosses")
    else {
        unreachable!()
    };
    assert_eq!(i, 4);
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![0, 1, 3, 4]
    );
    // A dip of exactly max_drop is not a stray: 27 after 29 continues the chain, which then has five
    // readings (rise 11 at 39) when it crosses.
    s[2].1 = 27;
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &s);
    let (_, Fed::Crossed { readings, .. }) = fed
        .iter()
        .enumerate()
        .find(|(_, f)| crossed(f))
        .expect("crosses")
    else {
        unreachable!()
    };
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
}

#[test]
fn a_stray_before_the_first_reading_is_not_the_anchor_unless_the_first_reading_continues_it() {
    // 70 then the ramp: 28 does not continue 70, so the ramp's anchor is 28.
    let s = series(9_000, &[70, 28, 29, 33, 39]);
    let mut det = RampDetector::new(spec());
    let Fed::Crossed { readings, .. } = feed(&mut det, &s).pop().unwrap() else {
        panic!()
    };
    assert_eq!(readings[0].id.0, 1);
    // 25 then the ramp: 28 continues 25 (a step of 3), so the stray is the first reading and the
    // anchor, one spacing earlier. The limit the module documentation states.
    let s = series(9_000, &[25, 28, 29, 33, 39]);
    let mut det = RampDetector::new(spec());
    let Fed::Crossed { readings, .. } = feed(&mut det, &s).pop().unwrap() else {
        panic!()
    };
    assert_eq!(readings[0].id.0, 0);
}

#[test]
fn a_burst_of_jumping_readings_is_not_a_ramp() {
    // The public rules' abnormal readings in a burst jump by tens.
    let s = series(0, &[88, 20, 91, 14, 75, 99, 12, 80, 60, 95, 8, 77]);
    let mut det = RampDetector::new(spec());
    assert!(!feed(&mut det, &s).iter().any(crossed));
}

#[test]
fn the_public_rules_verdict_is_not_consulted_and_only_counters_are_read() {
    let mk = |abnormal: bool| {
        let mut det = RampDetector::new(spec());
        let mut out = Vec::new();
        for (i, (ms, v)) in series(0, &LEAK).iter().enumerate() {
            let mut h = held(i as u32, *ms, reading(0, CounterName::Saturation, *v));
            h.abnormal = abnormal;
            out.push(match det.feed(&h) {
                Fed::Nothing => (0, Vec::new()),
                Fed::Extends(a) => (1, vec![a]),
                Fed::Crossed { readings, .. } => {
                    (2, readings.iter().map(|r| r.id.0).collect::<Vec<_>>())
                }
            });
        }
        out
    };
    assert_eq!(mk(true), mk(false));
    assert!(mk(false).iter().any(|(k, _)| *k == 2));
    // A message, a snapshot and a probe result are nothing to the detector.
    let mut det = RampDetector::new(spec());
    let msg = Observation::Message {
        service: ServiceId(0),
        text_id: SignalText::OutOfResource.text_id(),
        severity: Severity::High,
    };
    assert_eq!(det.feed(&held(0, 0, msg)), Fed::Nothing);
    assert_eq!(det.readings_seen(), 0);
    assert_eq!(det.live_chains(), 0);
    assert_eq!(
        det.feed(&held(1, 10, reading(0, CounterName::Saturation, 5))),
        Fed::Nothing
    );
    assert_eq!(det.readings_seen(), 1);
    assert_eq!(det.live_chains(), 1);
}

#[test]
fn chains_are_per_service_and_counter() {
    // The ramp's readings alternate with another counter at the same service and the same counter at
    // another service, each with values that would break it if they were joined.
    let mut det = RampDetector::new(spec());
    let mut crossings = Vec::new();
    for (i, v) in LEAK.iter().take(6).enumerate() {
        let t = 10_000 + 1_200 * i as u64;
        let id = (3 * i) as u32;
        let f = det.feed(&held(id, t, reading(0, CounterName::Saturation, *v)));
        det.feed(&held(
            id + 1,
            t + 100,
            reading(0, CounterName::Latency, 90 - 7 * i as u64),
        ));
        det.feed(&held(
            id + 2,
            t + 200,
            reading(1, CounterName::Saturation, 10 + (i as u64 % 2) * 60),
        ));
        if let Fed::Crossed { key, .. } = f {
            crossings.push((i, key));
        }
    }
    assert_eq!(
        crossings,
        vec![(3, (ServiceId(0), CounterName::Saturation))]
    );
}

#[test]
fn the_longest_chain_takes_a_reading_that_fits_two_and_the_earlier_one_a_tie() {
    // Chain A is 28, 29, 30 (level 30). 43 is a step of 13 above it and starts chain B. 42 is within
    // a step of 12 of A and within a drop of 2 of B: it fits both, A is longer and takes it.
    let mut det = RampDetector::new(spec());
    for (i, v) in [28u64, 29, 30, 43].iter().enumerate() {
        det.feed(&held(
            i as u32,
            100 * i as u64,
            reading(0, CounterName::Saturation, *v),
        ));
    }
    assert_eq!(det.live_chains(), 2);
    let fed = det.feed(&held(4, 400, reading(0, CounterName::Saturation, 42)));
    let Fed::Crossed { readings, .. } = fed else {
        panic!("{fed:?}")
    };
    assert_eq!(
        readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
        vec![0, 1, 2, 4],
        "chain A took 42 (rise 14 in four readings); B (43) is not in it"
    );
    // A tie goes to the earlier chain. Two chains of one reading each, 10 and 23 (a step of 13); 21
    // fits both (11 above 10, 2 below 23), and the first takes it. Then 25 and 30 continue it and
    // it crosses, which the second chain (23, 21, 25, 30: a rise of 7) would not have.
    let mut det = RampDetector::new(spec());
    for (i, v) in [10u64, 23, 21, 25, 30].iter().enumerate() {
        let fed = det.feed(&held(
            i as u32,
            100 * i as u64,
            reading(0, CounterName::Saturation, *v),
        ));
        if i == 4 {
            let Fed::Crossed { readings, .. } = fed else {
                panic!("{fed:?}")
            };
            assert_eq!(
                readings.iter().map(|h| h.id.0).collect::<Vec<_>>(),
                vec![0, 2, 3, 4]
            );
        } else {
            assert_eq!(fed, Fed::Nothing, "reading {i}");
        }
    }
}

#[test]
fn at_most_four_chains_live_at_a_key_and_a_noticed_chain_is_kept() {
    let mut det = RampDetector::new(spec());
    // A ramp that crosses and is linked, then many unrelated readings at the same key.
    let s = series(0, &LEAK[..5]);
    let fed = feed(&mut det, &s);
    let Fed::Crossed { uid, key, .. } = &fed[3] else {
        panic!()
    };
    det.link(*key, *uid, 7);
    for (i, v) in [90u64, 5, 70, 15, 95, 3, 60].iter().enumerate() {
        det.feed(&held(
            100 + i as u32,
            5_000 + 10 * i as u64,
            reading(0, CounterName::Saturation, *v),
        ));
        assert!(det.live_chains() <= MAX_CHAINS_PER_KEY);
    }
    assert_eq!(det.live_chains(), MAX_CHAINS_PER_KEY);
    // The linked chain survived the evictions: a reading that continues it says so.
    let last = 44; // the chain's level
    let fed = det.feed(&held(
        200,
        5_500,
        reading(0, CounterName::Saturation, last + 3),
    ));
    assert_eq!(fed, Fed::Extends(7));
}

#[test]
fn once_linked_a_chain_reports_the_anomaly_for_each_reading_that_continues_it() {
    let mut det = RampDetector::new(spec());
    let s = series(0, &LEAK);
    let mut link = None;
    let mut extends = 0;
    for (i, (ms, v)) in s.iter().enumerate() {
        match det.feed(&held(
            i as u32,
            *ms,
            reading(0, CounterName::Saturation, *v),
        )) {
            Fed::Crossed { uid, key, .. } => link = Some((key, uid)),
            Fed::Extends(a) => {
                assert_eq!(a, 3);
                extends += 1;
            }
            Fed::Nothing => {}
        }
        if let Some((key, uid)) = link.take() {
            det.link(key, uid, 3);
        }
    }
    // Readings 4..=13 continue the chain (67 after 68 is a dip of one, within max_drop 2).
    assert_eq!(extends, 10);
    // Between the crossing and the link a reading is nothing to do: a chain that has crossed and
    // is not linked yet does not cross again either.
    let mut det = RampDetector::new(spec());
    let fed = feed(&mut det, &series(0, &LEAK[..6]));
    assert!(crossed(&fed[3]));
    assert_eq!(fed[4], Fed::Nothing);
    assert_eq!(fed[5], Fed::Nothing);
}

#[test]
fn a_chain_that_is_never_a_ramp_is_dropped_after_max_chain_readings() {
    let mut det = RampDetector::new(spec());
    let n = MAX_CHAIN_READINGS as usize + 1;
    let flat: Vec<(u64, u64)> = (0..n).map(|i| (i as u64 * 1_000, 30)).collect();
    let fed = feed(&mut det, &flat);
    assert!(fed.iter().all(|f| *f == Fed::Nothing));
    assert_eq!(
        det.live_chains(),
        0,
        "dropped at the reading that passes the cap"
    );
}

// ---- through the rung

fn rung_with(noticer: NoticerSpec) -> RungConfig {
    RungConfig {
        noticer,
        ..RungConfig::default()
    }
}

/// A rung fed the observations of each step, with ids running on across steps.
struct Run {
    rung: Rung,
    next: u32,
}

impl Run {
    fn new(public: &StreamPublic, noticer: NoticerSpec) -> Self {
        Self {
            rung: Rung::new(public, rung_with(noticer)),
            next: 0,
        }
    }

    fn step(&mut self, now_ms: u64, observations: &[(u64, Observation)]) -> Vec<ObsId> {
        let events: Vec<StreamEvent> = observations
            .iter()
            .map(|(ms, obs)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: obs.clone(),
                }
            })
            .collect();
        let ids = events
            .iter()
            .map(|e| match e {
                StreamEvent::Observed { id, .. } => *id,
                StreamEvent::Answered { .. } => unreachable!(),
            })
            .collect();
        self.rung.absorb(&events, &[], at(now_ms));
        self.rung.notice(at(now_ms));
        ids
    }

    fn notices(&self) -> Vec<(ObsId, Instant, ServiceId, Instant)> {
        self.rung
            .notice_log()
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .map(|e| (e.anchor, e.anchor_at, e.site, e.at))
            .collect()
    }
}

fn leak_observations(service: ServiceId, t0: u64, values: &[u64]) -> Vec<(u64, Observation)> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                t0 + 1_200 * i as u64,
                reading(service.0, CounterName::Saturation, *v),
            )
        })
        .collect()
}

/// Feed `obs` one step per 500 ms, delivering what falls in each step, as the harness does.
fn play(run: &mut Run, obs: &[(u64, Observation)], until_ms: u64) {
    let mut now = 500;
    let mut i = 0;
    while now <= until_ms {
        let mut batch = Vec::new();
        while i < obs.len() && obs[i].0 <= now {
            batch.push(obs[i].clone());
            i += 1;
        }
        run.step(now, &batch);
        now += 500;
    }
}

#[test]
fn a_leak_is_noticed_on_its_first_reading_and_the_base_notices_it_later_if_at_all() {
    let public = public_of(&params(0, 150));
    let site = ServiceId(0);
    let obs = leak_observations(site, 10_000, &LEAK);
    // The rung alone: its anomalies are made of abnormal observations, readings at or above 50.
    let mut rung = Run::new(&public, NoticerSpec::default());
    play(&mut rung, &obs, 40 * S);
    for (anchor, anchor_at, _, _) in rung.notices() {
        assert!(
            anchor_at > at(10_000 + 1_200 * 5),
            "the rung's anchors are at readings of 50 and over: {anchor:?} at {anchor_at:?}"
        );
    }
    // The ramp noticer over the rung: one notice, anchored on the first reading, about its service,
    // stamped at the step that processes the fourth reading (at 13.6 s: the step at 14 s).
    let mut ramp = Run::new(&public, composed(Some(spec()), None));
    play(&mut ramp, &obs, 40 * S);
    let notices = ramp.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(
        notices[0],
        (ObsId(0), at(10_000), site, at(14_000)),
        "anchor, anchor instant, site, notice instant"
    );
    assert_eq!(ramp.rung.noticer_id(), "ramp");
    assert_eq!(ramp.rung.noticed_total(), 1);
}

#[test]
fn the_readings_that_continue_the_ramp_stay_with_its_anomaly_and_it_retires_when_they_stop() {
    let public = public_of(&params(0, 150));
    let site = ServiceId(0);
    let obs = leak_observations(site, 10_000, &LEAK);
    let mut run = Run::new(&public, composed(Some(spec()), None));
    play(&mut run, &obs, 28 * S);
    let views = run.rung.views(at(28 * S));
    assert_eq!(
        views.len(),
        1,
        "one anomaly, whatever the base did with the readings over 50"
    );
    let id = views[0].id;
    let held = run.rung.attached(id);
    // The first fourteen readings are all attached (those over 50 reached it through the attach
    // rule's own site-speaking branch, and the ramp noticer's own for the rest): every one once.
    let ids: Vec<u32> = held.iter().map(|(_, o, _)| o.0).collect();
    assert_eq!(ids, (0..14).collect::<Vec<_>>());
    assert_eq!(held[0].0, at(10_000));
    // After the last reading (at 25.6 s) it is quiet for the rung's 6 s and then due to retire.
    assert!(run.rung.quiet(at(28 * S)).is_empty());
    let last = 10_000 + 1_200 * 13;
    assert_eq!(run.rung.quiet(at(last + 6_000)), vec![id]);
    assert!(run.rung.quiet(at(last + 5_999)).is_empty());
}

#[test]
fn a_candidate_that_is_a_part_of_the_ramp_is_adopted_and_one_with_more_is_kept() {
    let public = public_of(&params(0, 150));
    let site = ServiceId(0);
    // A ramp that begins just under the alarm line, so that readings over 50 reach the base as a
    // candidate in the same step the ramp is noticed: 46, 48, 52, 55, 58 (five readings, rise 12).
    let ramp_obs = leak_observations(site, 10_000, &[46, 48, 52, 55, 58]);
    let mut run = Run::new(&public, composed(Some(spec()), None));
    run.step(16 * S, &ramp_obs);
    assert_eq!(run.rung.noticed_total(), 1);
    assert_eq!(
        run.rung.debug_anomalies().len(),
        1,
        "the candidate of the readings over 50 was part of the ramp and is gone: {:?}",
        run.rung.debug_anomalies()
    );
    // Without the ramp noticer the base has its candidate.
    let mut base = Run::new(&public, NoticerSpec::default());
    base.step(16 * S, &ramp_obs);
    assert_eq!(base.rung.debug_anomalies().len(), 1);
    assert_eq!(base.rung.noticed_total(), 0);
    // The same with a catalogue message at the site attached to the candidate: it holds something
    // the ramp's anomaly does not, so it stays (a candidate that has not been noticed).
    let mut with_message = ramp_obs.clone();
    with_message.push((
        13_000,
        Observation::Message {
            service: site,
            text_id: SignalText::OutOfResource.text_id(),
            severity: Severity::High,
        },
    ));
    with_message.sort_by_key(|(t, _)| *t);
    let mut run = Run::new(&public, composed(Some(spec()), None));
    run.step(16 * S, &with_message);
    assert_eq!(run.rung.noticed_total(), 1);
    assert_eq!(run.rung.noticer_id(), "ramp");
    assert_eq!(
        run.rung.debug_anomalies().len(),
        2,
        "the candidate holds the message, which the ramp's anomaly does not: {:?}",
        run.rung.debug_anomalies()
    );
}

#[test]
fn a_ramp_anomaly_is_the_same_whatever_the_cadence_of_the_steps() {
    let public = public_of(&params(0, 150));
    let site = ServiceId(0);
    let obs = leak_observations(site, 10_000, &LEAK);
    let mut once = Run::new(&public, composed(Some(spec()), None));
    once.step(30 * S, &obs);
    let mut fine = Run::new(&public, composed(Some(spec()), None));
    play(&mut fine, &obs, 30 * S);
    let (a, b) = (once.notices(), fine.notices());
    assert_eq!(a.len(), 1);
    assert_eq!((a[0].0, a[0].1, a[0].2), (b[0].0, b[0].1, b[0].2));
    assert_ne!(a[0].3, b[0].3, "only the instant of the notice differs");
    let (ida, idb) = (
        once.rung.views(at(30 * S))[0].id,
        fine.rung.views(at(30 * S))[0].id,
    );
    assert_eq!(
        once.rung.attached(ida).len(),
        fine.rung.attached(idb).len(),
        "and what is attached"
    );
    assert_eq!(once.rung.attached(ida).len(), 14);
}

// ---- the identity with the base

fn log_of(noticer: NoticerSpec, seed: u64) -> (Vec<String>, f64) {
    let spec = StreamPolicySpec::from_id("never_escalate").unwrap();
    let p = params(seed, 200);
    let l = limits(&p);
    let rec = play_with_rung(&p, &spec, &l, &rung_with(noticer)).unwrap();
    assert!(rec.notice_log.iter().all(|e| e.anchor_at <= e.at));
    (
        rec.notice_log
            .iter()
            .map(|e| {
                format!(
                    "{:?} {} {:?} {:?} {:?} {:?}",
                    e.kind, e.anomaly, e.anchor, e.site, e.anchor_at, e.at
                )
            })
            .collect(),
        rec.notices.totals.notices as f64,
    )
}

#[test]
fn a_ramp_noticer_that_cannot_cross_is_its_base_in_whole_segments() {
    let never = RampSpec {
        min_rise: u32::MAX,
        ..spec()
    };
    let reanchor = |ramp: Option<RampSpec>| NoticerSpec::Composed {
        base: BaseSpec::Reanchor {
            notice_z: Some(2.0),
            gap_ns: 20 * MS,
            min_burst: 2,
            isolation: Isolation::Site,
        },
        ramp,
        split: None,
    };
    let plain_reanchor = NoticerSpec::Reanchor {
        notice_z: Some(2.0),
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    };
    for seed in [11, 12] {
        let (base, n) = log_of(NoticerSpec::default(), seed);
        assert!(n > 0.0, "the segment has notices");
        assert_eq!(
            log_of(composed(Some(never), None), seed).0,
            base,
            "seed {seed}"
        );
        assert_eq!(
            log_of(reanchor(Some(never)), seed).0,
            log_of(plain_reanchor, seed).0,
            "seed {seed}, over the re-anchor"
        );
    }
}

#[test]
fn a_ramp_noticer_that_can_cross_adds_notices_and_is_scored_by_the_evaluator() {
    // A permissive ramp on whole segments: the evaluator accepts the record (an error would be a
    // violation of its rules for a notice list), and the notices are the base's plus the ramp's.
    let loose = RampSpec {
        gap_ns: 2_000 * MS,
        max_step: 15,
        max_drop: 4,
        min_readings: 3,
        min_rise: 6,
    };
    let mut more = 0;
    for seed in [11, 12, 13] {
        let (base, _) = log_of(NoticerSpec::default(), seed);
        let (with, _) = log_of(composed(Some(loose), None), seed);
        more += usize::from(with.len() > base.len());
    }
    assert!(
        more > 0,
        "a loose ramp finds a ramp somewhere in three segments"
    );
}

// ---- the files

#[test]
fn the_run_writes_the_composed_noticers_record_and_replays_it_byte_for_byte() {
    let mut m = manifest(
        "ramp-files",
        &[("a", "never_escalate"), ("b", "never_escalate")],
        2,
        200,
        9,
    );
    m.noticers.insert(
        "b".to_owned(),
        composed(
            Some(RampSpec {
                gap_ns: 2_000 * MS,
                max_step: 15,
                max_drop: 4,
                min_readings: 3,
                min_rise: 6,
            }),
            Some(SplitSpec {
                gap_ns: 2_000 * MS,
                min_burst: 2,
            }),
        ),
    );
    m.validate().unwrap();
    let text = m.canonical_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["noticers"]["b"]["noticer"], "composed");
    assert_eq!(value["noticers"]["b"]["ramp"]["min_rise"], 6);
    assert_eq!(value["noticers"]["b"]["split"]["min_burst"], 2);
    let back: StreamManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    let run = |name: &str| {
        let out = scratch(name).join("run");
        execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
        out
    };
    let (x, y) = (run("ramp-replay-a"), run("ramp-replay-b"));
    for arm in ["a", "b"] {
        for file in [
            "notices.csv",
            "notice_incidents.csv",
            "notice_events.csv",
            "results.csv",
        ] {
            assert_eq!(
                read(&x.join(arm), file),
                read(&y.join(arm), file),
                "{arm}/{file}"
            );
        }
    }
    assert_eq!(
        cell(&read(&x.join("b"), "notices.csv"), 0, "noticer"),
        "ramp_split"
    );
    assert_eq!(
        cell(&read(&x.join("a"), "notices.csv"), 0, "noticer"),
        "rung"
    );
}
