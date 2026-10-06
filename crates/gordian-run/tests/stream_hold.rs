//! The selection oracle's option `hold_until_asked` (work item B2): off by default and then not
//! written, and when on, an anomaly the oracle will ask about is not retired before it has been
//! asked.
//!
//! The byte-identity gate against R6's recorded hashes (the option is off there) is a run, not a
//! test (`experiments/exploration/scripts/b1_gate.py`).

mod stream_common;

use gordian_run::stream::arms::noticer::NoticeKind;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::Tier;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use stream_common::*;

const NS: u64 = 1_000_000_000;

fn selection(delay_s: u64, hold: bool) -> StreamPolicySpec {
    StreamPolicySpec::OracleSelection {
        delay_ns: delay_s * NS,
        hold_until_asked: hold,
    }
}

#[test]
fn the_option_is_off_by_default_and_not_written_when_off() {
    let bare = StreamPolicySpec::from_id("oracle_selection").unwrap();
    assert_eq!(bare, selection(0, false));
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        json!("oracle_selection")
    );
    // A delay with the option off is written as before the option existed.
    assert_eq!(
        serde_json::to_value(selection(16, false)).unwrap(),
        json!({"policy": "oracle_selection", "delay_ns": 16 * NS})
    );
    let old: StreamPolicySpec =
        serde_json::from_value(json!({"policy": "oracle_selection", "delay_ns": 16 * NS})).unwrap();
    assert_eq!(old, selection(16, false));
    // On, it is written with the delay (when there is one) and read back.
    let on = json!({"policy": "oracle_selection", "delay_ns": 16 * NS, "hold_until_asked": true});
    assert_eq!(serde_json::to_value(selection(16, true)).unwrap(), on);
    assert_eq!(
        serde_json::from_value::<StreamPolicySpec>(on).unwrap(),
        selection(16, true)
    );
    let no_delay = json!({"policy": "oracle_selection", "hold_until_asked": true});
    assert_eq!(serde_json::to_value(selection(0, true)).unwrap(), no_delay);
    assert_eq!(
        serde_json::from_value::<StreamPolicySpec>(no_delay).unwrap(),
        selection(0, true)
    );
    // An explicit false is the default.
    let off: StreamPolicySpec = serde_json::from_value(
        json!({"policy": "oracle_selection", "delay_ns": 16 * NS, "hold_until_asked": false}),
    )
    .unwrap();
    assert_eq!(off, selection(16, false));
    assert_eq!(
        serde_json::to_value(off).unwrap().get("hold_until_asked"),
        None
    );
}

#[test]
fn only_the_selection_oracle_has_the_option() {
    for policy in [
        "never_escalate",
        "always_escalate",
        "oracle_notice",
        "oracle_selection_context",
        "oracle_escalation",
        "random_escalation",
    ] {
        let err =
            StreamPolicySpec::from_parts(policy, None, None, None, None, None, None, Some(true))
                .unwrap_err();
        assert!(err.contains("hold_until_asked"), "{policy}: {err}");
    }
    let err = serde_json::from_value::<StreamPolicySpec>(
        json!({"policy": "always_escalate", "hold_until_asked": true}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("hold_until_asked"), "{err}");
    assert!(
        StreamPolicySpec::from_parts(
            "oracle_selection",
            None,
            None,
            None,
            None,
            Some(NS),
            None,
            Some(true)
        )
        .is_ok()
    );
}

/// For each notice of the record, in order: (anomaly, instant noticed, anchored on a hard
/// incident), and for each retirement: (anomaly, instant retired).
fn lives(
    record: &gordian_run::stream::SegmentRecord,
) -> (BTreeMap<u32, (u64, bool)>, BTreeMap<u32, u64>) {
    let mut noticed = BTreeMap::new();
    let mut retired = BTreeMap::new();
    let mut scores = record.notices.per_notice.iter();
    for e in &record.notice_log {
        match e.kind {
            NoticeKind::Notice => {
                let s = scores.next().expect("a score per notice");
                noticed.insert(e.anomaly, (e.at.0, s.tier == Some(Tier::Hard)));
            }
            NoticeKind::Retire => {
                retired.insert(e.anomaly, e.at.0);
            }
        }
    }
    (noticed, retired)
}

#[test]
fn a_hard_anomaly_is_not_retired_before_it_is_asked_and_the_others_are() {
    let delay = 30;
    let (mut early_without, mut early_other_with, mut hard_with) = (0, 0, 0);
    let (mut needed_without, mut needed_with) = (0, 0);
    for seed in 11..35 {
        let p = params(seed, 200);
        let l = limits(&p);
        let without = play(&p, &selection(delay, false), &l).unwrap();
        let with = play(&p, &selection(delay, true), &l).unwrap();
        let (n0, r0) = lives(&without);
        let (n1, r1) = lives(&with);
        // Without the option, a hard anomaly can be retired before the delay has passed: it is then
        // never asked about.
        for (a, (at, hard)) in &n0 {
            if let Some(r) = r0.get(a) {
                early_without += i32::from(*hard && *r < at + delay * NS);
            }
        }
        // With it, a hard-anchored anomaly is retired only at or after notice + delay, once asked.
        for (a, (at, hard)) in &n1 {
            if let Some(r) = r1.get(a) {
                if *hard {
                    hard_with += 1;
                    assert!(
                        *r >= at + delay * NS,
                        "seed {seed}: anomaly {a} retired {} ms before it could be asked",
                        (at + delay * NS - r) / 1_000_000
                    );
                } else {
                    early_other_with += i32::from(*r < at + delay * NS);
                }
            }
        }
        needed_without += without.verdict.totals.escalations.needed;
        needed_with += with.verdict.totals.escalations.needed;
        // Nothing else about the arm's kind changes: it still asks only about hard anomalies.
        assert_eq!(with.verdict.totals.escalations.unneeded, 0, "seed {seed}");
        assert_eq!(with.verdict.totals.escalations.background, 0, "seed {seed}");
        // The noticer and the stream are the same.
        assert_eq!(with.observations, without.observations);
        assert_eq!(with.noticer, without.noticer);
    }
    assert!(
        early_without > 0,
        "the streams hold a hard anomaly retired before 30 s"
    );
    assert!(
        hard_with > 0,
        "and a hard anomaly that is retired once asked"
    );
    assert!(
        early_other_with > 0,
        "other anomalies retire at their quiet time as ever"
    );
    assert!(
        needed_with >= needed_without,
        "keeping an anomaly live cannot make the oracle ask about fewer hard incidents: \
         {needed_with} against {needed_without}"
    );
}

#[test]
fn with_the_option_off_the_arm_is_the_arm_it_was() {
    // An explicit false, a delay and the bare id: the same record as the arm built without the
    // option (these are the same value, so this pins that the factory does not read anything else).
    let p = params(11, 200);
    let l = limits(&p);
    let a = play(&p, &selection(16, false), &l).unwrap();
    let b = play(
        &p,
        &serde_json::from_value::<StreamPolicySpec>(
            json!({"policy": "oracle_selection", "delay_ns": 16 * NS}),
        )
        .unwrap(),
        &l,
    )
    .unwrap();
    assert_eq!(a.verdict, b.verdict);
    assert_eq!(a.notices, b.notices);
    assert_eq!(a.notice_log, b.notice_log);
    // The option on changes retirements, and only what follows from them.
    let c = play(&p, &selection(16, true), &l).unwrap();
    assert_eq!(c.observations, a.observations);
    let v: Value = serde_json::to_value(selection(16, true)).unwrap();
    assert_eq!(v["hold_until_asked"], true);
}
