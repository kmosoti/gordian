//! The public selectors and the selection files (work item B4): what `public_threshold` and
//! `public_change` select, their spelling in a manifest, and what the harness writes about what an
//! arm escalated, notice by notice.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::EscalationRule;
use gordian_run::stream::arms::public_change::PublicChange;
use gordian_run::stream::arms::public_threshold::PublicThreshold;
use gordian_run::stream::arms::rung::AnomalyView;
use gordian_run::stream::execute_stream;
use gordian_run::stream::results::{SELECTION_HEADER, SELECTION_NOTICES_HEADER};
use gordian_run::stream::spec::{KNOWN, StreamPolicySpec};
use gordian_stream::ObsId;
use gordian_world::ServiceId;
use serde_json::json;
use stream_common::*;

const S: u64 = 1_000_000_000;

fn secs(n: u64) -> Instant {
    Instant(n * S)
}

fn view(id: u32) -> AnomalyView {
    AnomalyView {
        id,
        site: ServiceId(0),
        anchor: ObsId(id),
        anchor_at: Instant(0),
        noticed_at: secs(10),
        score: 4.0,
        peak_score: 4.0,
        digest: 11,
        attempts: 0,
        pending: 0,
        answered: 0,
        last_attempt_at: None,
        last_attempt_digest: None,
        cheap_declared: false,
        delivered: 100,
        evidence: 6,
        contradicted_since: None,
    }
}

// ---- public_threshold

#[test]
fn the_threshold_selector_waits_for_the_delay_and_asks_about_a_silent_rung() {
    let mut rule = PublicThreshold::new(16 * S, 2 * S);
    assert!(
        rule.monitors(),
        "it reads the checker, so the rung keeps it current"
    );
    let silent = view(1);
    assert!(
        rule.targets(secs(25), std::slice::from_ref(&silent))
            .is_empty(),
        "before notice + delay (26 s)"
    );
    assert_eq!(
        rule.targets(secs(26), std::slice::from_ref(&silent)),
        vec![1],
        "at notice + delay, with no declaration"
    );
    let mut declared = view(2);
    declared.cheap_declared = true;
    assert!(
        rule.targets(secs(40), &[declared]).is_empty(),
        "declared and not contradicted: resolved"
    );
}

#[test]
fn the_threshold_selector_asks_about_a_contradiction_that_has_lasted_the_persistence() {
    let mut rule = PublicThreshold::new(16 * S, 4 * S);
    let mut v = view(1);
    v.cheap_declared = true;
    v.contradicted_since = Some(secs(20));
    // Delay passed (26 s), contradiction only 3 s old at 23 s: not asked. The delay is the later.
    assert!(rule.targets(secs(23), std::slice::from_ref(&v)).is_empty());
    // At 26 s the contradiction is 6 s old: asked.
    assert_eq!(rule.targets(secs(26), std::slice::from_ref(&v)), vec![1]);
    // A contradiction that began at 25 s is 1 s old at 26 s: not asked until 29 s.
    v.contradicted_since = Some(secs(25));
    assert!(rule.targets(secs(26), std::slice::from_ref(&v)).is_empty());
    assert!(rule.targets(secs(28), std::slice::from_ref(&v)).is_empty());
    assert_eq!(rule.targets(secs(29), std::slice::from_ref(&v)), vec![1]);
}

#[test]
fn the_threshold_selector_asks_once_and_a_silent_rung_is_silent_for_the_persistence_after_notice() {
    // Persistence longer than the delay: a silent rung is asked about at notice + persistence.
    let mut rule = PublicThreshold::new(2 * S, 8 * S);
    let v = view(1);
    assert!(rule.targets(secs(17), std::slice::from_ref(&v)).is_empty());
    assert_eq!(rule.targets(secs(18), std::slice::from_ref(&v)), vec![1]);
    // Once: never again, and never while a call is in flight.
    let mut asked = view(1);
    asked.attempts = 1;
    assert!(rule.targets(secs(60), &[asked]).is_empty());
    // Zero persistence and zero delay ask at notice.
    let mut now = PublicThreshold::new(0, 0);
    assert_eq!(now.targets(secs(10), &[view(1)]), vec![1]);
    // The rung's contradiction and silence are the only things read: score, digest and evidence do
    // not matter.
    let mut other = view(3);
    other.score = -5.0;
    other.digest = 99;
    other.evidence = 0;
    assert_eq!(now.targets(secs(10), &[other]), vec![3]);
}

// ---- public_change

#[test]
fn the_change_selector_asks_when_the_evidence_has_grown_by_k_since_notice() {
    let mut rule = PublicChange::new(16 * S, 3);
    let mut v = view(1); // evidence 6 at notice (first sight)
    assert!(rule.targets(secs(10), std::slice::from_ref(&v)).is_empty());
    v.evidence = 8; // grown by 2
    assert!(rule.targets(secs(30), std::slice::from_ref(&v)).is_empty());
    v.evidence = 9; // grown by 3, delay passed
    assert_eq!(rule.targets(secs(30), std::slice::from_ref(&v)), vec![1]);
    // Growth that is complete before the delay still counts at the delay, not before it.
    let mut rule = PublicChange::new(16 * S, 3);
    let mut w = view(2);
    assert!(rule.targets(secs(10), std::slice::from_ref(&w)).is_empty());
    w.evidence = 12;
    assert!(
        rule.targets(secs(20), std::slice::from_ref(&w)).is_empty(),
        "before the delay"
    );
    assert_eq!(
        rule.targets(secs(26), std::slice::from_ref(&w)),
        vec![2],
        "at the delay"
    );
}

#[test]
fn the_change_selector_reads_the_baseline_once_and_a_fall_is_not_growth() {
    let mut rule = PublicChange::new(0, 2);
    let mut v = view(1);
    assert!(rule.targets(secs(10), std::slice::from_ref(&v)).is_empty()); // baseline 6
    // The count falls (observations moved out of the anomaly), then rises to 7: grown by 1 over
    // the baseline, not 2.
    v.evidence = 3;
    assert!(rule.targets(secs(11), std::slice::from_ref(&v)).is_empty());
    v.evidence = 7;
    assert!(rule.targets(secs(12), std::slice::from_ref(&v)).is_empty());
    v.evidence = 8;
    assert_eq!(rule.targets(secs(13), std::slice::from_ref(&v)), vec![1]);
    // Once.
    v.attempts = 1;
    assert!(rule.targets(secs(14), std::slice::from_ref(&v)).is_empty());
    // Anomalies have their own baselines.
    let mut a = view(5);
    a.evidence = 20;
    let mut b = view(6);
    b.evidence = 2;
    assert!(rule.targets(secs(15), &[a.clone(), b.clone()]).is_empty());
    a.evidence = 22;
    b.evidence = 3;
    assert_eq!(rule.targets(secs(16), &[a, b]), vec![5]);
    // k of zero is one: a rule that asks about every anomaly at notice is `always_escalate`.
    let mut one = PublicChange::new(0, 0);
    let mut c = view(9);
    assert!(one.targets(secs(10), std::slice::from_ref(&c)).is_empty());
    c.evidence += 1;
    assert_eq!(one.targets(secs(10), std::slice::from_ref(&c)), vec![9]);
}

// ---- the spelling

#[test]
fn the_selectors_are_written_and_read_with_their_parameters() {
    let t: StreamPolicySpec = serde_json::from_value(
        json!({"policy": "public_threshold", "delay_ns": 16_000_000_000u64, "persist_ns": 2_000_000_000u64}),
    )
    .unwrap();
    assert_eq!(
        t,
        StreamPolicySpec::PublicThreshold {
            delay_ns: 16 * S,
            persist_ns: 2 * S
        }
    );
    assert_eq!(
        serde_json::to_value(&t).unwrap(),
        json!({"policy": "public_threshold", "delay_ns": 16_000_000_000u64, "persist_ns": 2_000_000_000u64})
    );
    let c: StreamPolicySpec = serde_json::from_value(
        json!({"policy": "public_change", "delay_ns": 16_000_000_000u64, "k": 5}),
    )
    .unwrap();
    assert_eq!(
        c,
        StreamPolicySpec::PublicChange {
            delay_ns: 16 * S,
            k: 5
        }
    );
    assert_eq!(
        serde_json::to_value(&c).unwrap(),
        json!({"policy": "public_change", "delay_ns": 16_000_000_000u64, "k": 5})
    );
    // Zeros are not written for the threshold; `k` always is.
    assert_eq!(
        serde_json::to_value(StreamPolicySpec::PublicThreshold {
            delay_ns: 0,
            persist_ns: 0
        })
        .unwrap(),
        json!("public_threshold")
    );
    assert_eq!(
        serde_json::to_value(StreamPolicySpec::PublicChange { delay_ns: 0, k: 2 }).unwrap(),
        json!({"policy": "public_change", "k": 2})
    );
    assert_eq!(c.id().0, "public_change");
    assert_eq!(t.id().0, "public_threshold");
    // Both are comparison arms: neither is privileged, so neither may carry the name of one.
    assert_eq!(t.role(), gordian_run::stream::arms::ArmRole::Comparison);
    assert_eq!(c.role(), gordian_run::stream::arms::ArmRole::Comparison);
    assert!(KNOWN.contains(&"public_threshold") && KNOWN.contains(&"public_change"));
    // The threshold has no `k`, the change rule needs one that is at least 1, and neither takes the
    // other's parameters.
    for bad in [
        json!({"policy": "public_threshold", "k": 1}),
        json!({"policy": "public_change"}),
        json!({"policy": "public_change", "k": 0}),
        json!({"policy": "public_change", "k": 1, "persist_ns": 1}),
        json!({"policy": "never_escalate", "k": 1}),
    ] {
        assert!(
            serde_json::from_value::<StreamPolicySpec>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(StreamPolicySpec::from_id("public_change").is_err());
    assert!(StreamPolicySpec::from_id("public_threshold").is_ok());
}

// ---- the selection files

fn parse(csv: &str) -> Vec<Vec<String>> {
    rows(csv)
}

fn col(table: &[Vec<String>], name: &str) -> usize {
    table[0].iter().position(|c| c == name).unwrap()
}

fn sum(table: &[Vec<String>], prefix: &str) -> u64 {
    let columns: Vec<usize> = table[0]
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.starts_with(prefix)
                && ["background", "plain", "hard", "leak", "decoy"]
                    .iter()
                    .any(|k| c.ends_with(k))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(columns.len(), 5, "{prefix}");
    table[1..]
        .iter()
        .map(|r| {
            columns
                .iter()
                .map(|&i| r[i].parse::<u64>().unwrap())
                .sum::<u64>()
        })
        .sum()
}

#[test]
fn the_selection_files_are_written_for_every_arm_and_add_up_to_the_results() {
    let mut m = manifest(
        "b4-files",
        &[
            ("never", "never_escalate"),
            ("change", "never_escalate"),
            ("threshold", "never_escalate"),
            ("always", "always_escalate"),
            ("sel_privileged", "oracle_selection"),
        ],
        3,
        300,
        5,
    );
    m.arms[1].policy = StreamPolicySpec::PublicChange {
        delay_ns: 4 * S,
        k: 1,
    };
    m.arms[2].policy = StreamPolicySpec::PublicThreshold {
        delay_ns: 4 * S,
        persist_ns: 0,
    };
    m.validate().unwrap();
    let out = scratch("b4-files").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    let mut asked_somewhere = false;
    for arm in ["never", "change", "threshold", "always", "sel_privileged"] {
        let dir = out.join(arm);
        let sel_text = read(&dir, "selection.csv");
        let notices_text = read(&dir, "selection_notices.csv");
        assert_eq!(sel_text.lines().next().unwrap(), SELECTION_HEADER);
        assert_eq!(
            notices_text.lines().next().unwrap(),
            SELECTION_NOTICES_HEADER
        );
        let sel = parse(&sel_text);
        let res = parse(&read(&dir, "results.csv"));
        let notices = parse(&read(&dir, "notices.csv"));
        let per_notice = parse(&notices_text);
        // E2: the classes' calls, tokens and modelled nanoseconds are S26's totals.
        for (field, column) in [
            ("calls_", "reasoner_calls"),
            ("tokens_", "reasoner_tokens"),
            ("modelled_ns_", "reasoner_modelled_ns"),
        ] {
            let total: u64 = res[1..]
                .iter()
                .map(|r| r[col(&res, column)].parse::<u64>().unwrap())
                .sum();
            assert_eq!(sum(&sel, field), total, "{arm} {field}");
        }
        // N8: the notices by class are the notice counts; E3: one row per notice.
        let n_total: u64 = notices[1..]
            .iter()
            .map(|r| r[col(&notices, "notices")].parse::<u64>().unwrap())
            .sum();
        assert_eq!(sum(&sel, "notices_"), n_total, "{arm}");
        assert_eq!(per_notice.len() - 1, n_total as usize, "{arm}");
        // E1: every call of an arm that asks about the anomalies it has noticed is about one.
        let unattributed: u64 = sel[1..]
            .iter()
            .map(|r| {
                r[col(&sel, "escalations_unattributed")]
                    .parse::<u64>()
                    .unwrap()
            })
            .sum();
        assert_eq!(unattributed, 0, "{arm}: calls about no notice");
        let about: u64 = per_notice[1..]
            .iter()
            .map(|r| r[col(&per_notice, "escalations")].parse::<u64>().unwrap())
            .sum();
        assert_eq!(about, sum(&sel, "calls_"), "{arm}");
        // E4: retired before escalation is retired and never asked about; no rule retires by
        // follow-up in these arms.
        for r in &per_notice[1..] {
            let retired = !r[col(&per_notice, "retired_at_ns")].is_empty();
            let asked = r[col(&per_notice, "escalations")] != "0";
            assert_eq!(
                r[col(&per_notice, "retired_before_escalation")] == "true",
                retired && !asked,
                "{arm}"
            );
            assert_ne!(r[col(&per_notice, "retire_cause")], "followup", "{arm}");
        }
        assert_eq!(sum(&sel, "followup_retired_"), 0, "{arm}");
        asked_somewhere |= about > 0;
    }
    assert!(
        asked_somewhere,
        "some arm asked about some notice in three segments"
    );
    // The arm that never escalates has no calls; the one that always does asks about at least what
    // the others do.
    let never = parse(&read(&out.join("never"), "selection.csv"));
    assert_eq!(sum(&never, "calls_"), 0);
    let calls = |arm: &str| sum(&parse(&read(&out.join(arm), "selection.csv")), "calls_");
    assert!(calls("always") >= calls("change"));
    assert!(calls("always") >= calls("threshold"));
}

#[test]
fn the_selection_files_replay_byte_for_byte() {
    let mut m = manifest("b4-replay", &[("a", "never_escalate")], 2, 200, 4);
    m.arms[0].policy = StreamPolicySpec::PublicThreshold {
        delay_ns: 2 * S,
        persist_ns: S,
    };
    let run = |name: &str| {
        let out = scratch(name).join("run");
        execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
        out
    };
    let (x, y) = (run("b4-replay-a"), run("b4-replay-b"));
    for file in ["selection.csv", "selection_notices.csv", "results.csv"] {
        assert_eq!(read(&x.join("a"), file), read(&y.join("a"), file), "{file}");
    }
}
