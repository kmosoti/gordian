//! The call-budgeted public selector (work item B5): what its features are, which anomalies it asks
//! about and in what order, how the budget binds, how it is spelled in a manifest, that the new
//! `AnomalyView::services` is the count of distinct services of the evidence, and what it does in
//! the harness (a budget that never binds is always-escalate's question set; a budget of `k` with a
//! flat score asks about the first `k` anomalies to become ready).
//!
//! The byte-identity gate against R6's recorded hashes is a run (`experiments/exploration/scripts/b5_gate.py`), not a test.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::EscalationRule;
use gordian_run::stream::arms::noticer::{self, Noticer, NoticerSpec};
use gordian_run::stream::arms::public_budgeted::{
    AGE_CAP_NS, EVIDENCE_CAP, FEATURE_NAMES, Features, PublicBudgeted, SERVICES_CAP, Score,
};
use gordian_run::stream::arms::rung::{AnomalyView, Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::execute_stream;
use gordian_run::stream::spec::{KNOWN, StreamPolicySpec};
use gordian_stream::{ObsId, StreamParams, generate};
use gordian_world::ServiceId;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
        cheap_declared: true,
        delivered: 100,
        evidence: 0,
        contradicted_since: None,
        services: 0,
    }
}

fn flat(threshold: f64) -> Score {
    Score {
        threshold,
        ..Score::default()
    }
}

fn with_evidence(id: u32, evidence: u32) -> AnomalyView {
    AnomalyView {
        evidence,
        ..view(id)
    }
}

const EVIDENCE_ONLY: Score = Score {
    threshold: 0.0,
    contradiction: 0.0,
    silence: 0.0,
    evidence: 1.0,
    services: 0.0,
    age: 0.0,
};

// ---- the features, by their definitions

#[test]
fn each_feature_reads_one_field_of_the_view_as_the_module_documentation_says() {
    let now = secs(40);
    let base = Features::of(&view(1), now);
    assert_eq!(
        base.to_array()[..4],
        [0.0, 0.0, 0.0, 0.0],
        "declared, not contradicted, no evidence, no services"
    );
    assert_eq!(FEATURE_NAMES.len(), base.to_array().len());

    // contradiction: set when the checker's latest verdict is empty, whatever its start
    let mut v = view(1);
    v.contradicted_since = Some(secs(39));
    assert_eq!(Features::of(&v, now).contradiction, 1.0);
    v.contradicted_since = Some(secs(0));
    assert_eq!(Features::of(&v, now).contradiction, 1.0);
    // silence: the rung has declared nothing
    let mut v = view(1);
    v.cheap_declared = false;
    assert_eq!(Features::of(&v, now).silence, 1.0);
    // evidence: ln(1 + n) over ln(1 + cap), clipped
    assert_eq!(Features::of(&with_evidence(1, 0), now).evidence, 0.0);
    let at_cap = Features::of(&with_evidence(1, EVIDENCE_CAP as u32), now).evidence;
    assert!((at_cap - 1.0).abs() < 1e-12, "{at_cap}");
    assert_eq!(
        Features::of(&with_evidence(1, 10 * EVIDENCE_CAP as u32), now).evidence,
        1.0
    );
    let one = Features::of(&with_evidence(1, 1), now).evidence;
    assert!((one - 2f64.ln() / (1.0 + EVIDENCE_CAP).ln()).abs() < 1e-12);
    assert!(
        Features::of(&with_evidence(1, 4), now).evidence > one,
        "monotone"
    );
    // services: distinct services over the cap, clipped
    let mut v = view(1);
    v.services = 2;
    assert_eq!(Features::of(&v, now).services, 2.0 / SERVICES_CAP);
    v.services = 100;
    assert_eq!(Features::of(&v, now).services, 1.0);
    // age: from the anchor to the step, over the cap, clipped; measured from the anchor, not the notice
    let mut v = view(1);
    v.anchor_at = secs(10);
    v.noticed_at = secs(30);
    let f = Features::of(&v, secs(70));
    assert!((f.age - 60e9 / AGE_CAP_NS).abs() < 1e-12, "{}", f.age);
    v.noticed_at = secs(10);
    assert_eq!(
        Features::of(&v, secs(70)).age,
        f.age,
        "the notice's instant is not read"
    );
    assert_eq!(Features::of(&v, secs(10 + 1000)).age, 1.0);
    // an anchor in the future of the step (a clock that has not caught up) is age zero, not a wrap
    assert_eq!(Features::of(&v, secs(5)).age, 0.0);
}

#[test]
fn the_features_read_nothing_else_of_the_view() {
    // Everything the score is not allowed to depend on is changed; the features do not move.
    let now = secs(40);
    let a = view(1);
    let b = AnomalyView {
        id: 99,
        site: ServiceId(7),
        anchor: ObsId(500),
        noticed_at: secs(3),
        score: 123.0,
        peak_score: 456.0,
        digest: 0xDEAD,
        attempts: 0,
        pending: 5,
        answered: 5,
        last_attempt_at: Some(secs(2)),
        last_attempt_digest: Some(1),
        delivered: 9999,
        ..view(1)
    };
    assert_eq!(Features::of(&a, now), Features::of(&b, now));
}

#[test]
fn the_score_is_the_weighted_sum_and_a_weight_may_be_negative() {
    let f = Features {
        contradiction: 1.0,
        silence: 0.0,
        evidence: 0.5,
        services: 0.25,
        age: 1.0,
    };
    let s = Score {
        threshold: 0.0,
        contradiction: 2.0,
        silence: 100.0,
        evidence: 4.0,
        services: -4.0,
        age: 0.5,
    };
    assert_eq!(s.of(&f), 2.0 + 0.0 + 2.0 - 1.0 + 0.5);
}

// ---- which anomalies, in what order, under which budget

#[test]
fn an_anomaly_is_asked_about_at_notice_plus_delay_and_only_once() {
    let mut rule = PublicBudgeted::new(16 * S, 8, flat(0.0));
    assert!(
        rule.monitors(),
        "the checker's runs are billed for every budgeted arm"
    );
    let v = view(1);
    assert!(
        rule.targets(secs(25), std::slice::from_ref(&v)).is_empty(),
        "one step before notice + delay (26 s)"
    );
    assert_eq!(rule.targets(secs(26), std::slice::from_ref(&v)), vec![1]);
    assert!(
        rule.targets(secs(27), std::slice::from_ref(&v)).is_empty(),
        "once"
    );
    assert_eq!(rule.spent(), 1);
    // An anomaly already asked about (by anyone) is not asked about.
    let mut asked = view(2);
    asked.attempts = 1;
    assert!(rule.targets(secs(60), &[asked]).is_empty());
}

#[test]
fn the_ready_step_is_the_only_look_an_anomaly_gets() {
    // Below the threshold at its ready step, an anomaly is never asked about, though its score
    // rises past the threshold at a later step.
    let mut rule = PublicBudgeted::new(
        16 * S,
        8,
        Score {
            threshold: 0.5,
            ..EVIDENCE_ONLY
        },
    );
    let low = with_evidence(1, 1);
    assert!(
        rule.targets(secs(26), std::slice::from_ref(&low))
            .is_empty()
    );
    let grown = with_evidence(1, 60);
    assert!(
        rule.targets(secs(30), std::slice::from_ref(&grown))
            .is_empty(),
        "passed over at 26 s; not reconsidered"
    );
    assert_eq!(rule.spent(), 0, "a passed-over anomaly costs nothing");
    // An anomaly that is not yet ready is not looked at: it is scored when it becomes ready.
    let mut rule = PublicBudgeted::new(
        16 * S,
        8,
        Score {
            threshold: 0.5,
            ..EVIDENCE_ONLY
        },
    );
    assert!(rule.targets(secs(20), &[with_evidence(1, 1)]).is_empty());
    assert_eq!(
        rule.targets(secs(26), &[with_evidence(1, 60)]),
        vec![1],
        "the evidence at 26 s counts, not the evidence at 20 s"
    );
}

#[test]
fn ready_anomalies_are_taken_best_first_and_ties_by_id() {
    let mut rule = PublicBudgeted::new(0, 3, EVIDENCE_ONLY);
    let views = [
        with_evidence(5, 3),
        with_evidence(2, 40),
        with_evidence(9, 10),
        with_evidence(1, 10),
        with_evidence(7, 0),
    ];
    // Best first: 2 (40), then the tie of 10 by id (1, then 9); the budget of 3 leaves out 5 and 7.
    assert_eq!(rule.targets(secs(10), &views), vec![2, 1, 9]);
    assert_eq!(rule.spent(), 3);
}

#[test]
fn the_budget_binds_across_steps_and_a_later_better_anomaly_finds_it_spent() {
    let mut rule = PublicBudgeted::new(0, 2, flat(0.0));
    assert_eq!(rule.targets(secs(10), &[view(1)]), vec![1]);
    assert_eq!(
        rule.targets(secs(11), &[view(1), view(2)]),
        vec![2],
        "the second is the last"
    );
    assert!(
        rule.targets(secs(12), &[view(1), view(2), with_evidence(3, 64)])
            .is_empty(),
        "spent: even a better anomaly is not asked about"
    );
    assert_eq!(rule.spent(), 2);
}

#[test]
fn the_threshold_keeps_budget_for_a_later_anomaly() {
    // k = 1; a poor anomaly first, a good one later. Flat: the poor one takes the budget. With a
    // threshold that the poor one does not reach: the good one gets it.
    let poor = with_evidence(1, 0);
    let good = with_evidence(2, 64);
    let mut flat_rule = PublicBudgeted::new(0, 1, flat(0.0));
    assert_eq!(
        flat_rule.targets(secs(10), std::slice::from_ref(&poor)),
        vec![1]
    );
    assert!(
        flat_rule
            .targets(secs(11), &[poor.clone(), good.clone()])
            .is_empty()
    );
    let mut keeping = PublicBudgeted::new(
        0,
        1,
        Score {
            threshold: 0.5,
            ..EVIDENCE_ONLY
        },
    );
    assert!(
        keeping
            .targets(secs(10), std::slice::from_ref(&poor))
            .is_empty()
    );
    assert_eq!(keeping.targets(secs(11), &[poor, good]), vec![2]);
}

#[test]
fn the_threshold_is_inclusive_and_a_score_below_it_is_not_asked() {
    // evidence 64 scores exactly 1.0 (up to rounding); threshold at the score asks, above does not.
    let v = with_evidence(1, EVIDENCE_CAP as u32);
    let score = EVIDENCE_ONLY.of(&Features::of(&v, secs(10)));
    let mut at = PublicBudgeted::new(
        0,
        4,
        Score {
            threshold: score,
            ..EVIDENCE_ONLY
        },
    );
    assert_eq!(at.targets(secs(10), std::slice::from_ref(&v)), vec![1]);
    let mut above = PublicBudgeted::new(
        0,
        4,
        Score {
            threshold: score + 1e-9,
            ..EVIDENCE_ONLY
        },
    );
    assert!(above.targets(secs(10), &[v]).is_empty());
}

#[test]
fn a_flat_score_with_a_zero_threshold_is_the_first_k_to_become_ready() {
    // Anomalies become ready at 10, 12, 12, 15 s (notice + 0 delay); k = 3. With every weight zero
    // the three earliest are asked about whatever their evidence is, and ties at one step by id.
    let mut rule = PublicBudgeted::new(0, 3, flat(0.0));
    let mut asked = Vec::new();
    let mk = |id: u32, at: u64, ev: u32| AnomalyView {
        noticed_at: secs(at),
        evidence: ev,
        ..view(id)
    };
    let all = [mk(1, 10, 0), mk(2, 12, 50), mk(3, 12, 1), mk(4, 15, 60)];
    for now in [10, 12, 15] {
        let live: Vec<AnomalyView> = all
            .iter()
            .filter(|v| v.noticed_at <= secs(now))
            .cloned()
            .collect();
        asked.extend(rule.targets(secs(now), &live));
    }
    assert_eq!(asked, vec![1, 2, 3]);
}

#[test]
fn the_rule_is_a_function_of_the_views_and_the_instant_alone_and_replays_exactly() {
    let views: Vec<AnomalyView> = (0..12)
        .map(|i| AnomalyView {
            noticed_at: secs(u64::from(i % 4)),
            evidence: i * 3,
            services: i % 5,
            cheap_declared: i % 2 == 0,
            contradicted_since: (i % 3 == 0).then_some(secs(1)),
            ..view(i)
        })
        .collect();
    let score = Score {
        threshold: 0.7,
        contradiction: 1.0,
        silence: 0.5,
        evidence: 1.0,
        services: 0.5,
        age: -0.25,
    };
    let play = || {
        let mut rule = PublicBudgeted::new(2 * S, 5, score);
        (3..8)
            .flat_map(|t| rule.targets(secs(t), &views))
            .collect::<Vec<u32>>()
    };
    assert_eq!(play(), play());
    assert!(!play().is_empty());
    assert!(play().len() <= 5);
}

// ---- the spelling in a manifest

#[test]
fn the_selector_is_written_and_read_with_its_budget_and_score() {
    let spelled = json!({
        "policy": "public_budgeted", "delay_ns": 16_000_000_000u64, "k": 8,
        "score": {"threshold": 0.5, "contradiction": 0.25, "silence": -1.0, "evidence": 1.0,
                  "services": 2.0, "age": 0.0}
    });
    let spec: StreamPolicySpec = serde_json::from_value(spelled.clone()).unwrap();
    assert_eq!(
        spec,
        StreamPolicySpec::PublicBudgeted {
            delay_ns: 16 * S,
            k: 8,
            score: Score {
                threshold: 0.5,
                contradiction: 0.25,
                silence: -1.0,
                evidence: 1.0,
                services: 2.0,
                age: 0.0
            }
        }
    );
    assert_eq!(serde_json::to_value(&spec).unwrap(), spelled);
    assert_eq!(spec.id().0, "public_budgeted");
    assert_eq!(spec.role(), gordian_run::stream::arms::ArmRole::Comparison);
    assert!(KNOWN.contains(&"public_budgeted"));
    // An absent score is all zeros, and the score is always written, so what ran is recorded.
    let bare: StreamPolicySpec =
        serde_json::from_value(json!({"policy": "public_budgeted", "k": 3})).unwrap();
    assert_eq!(
        bare,
        StreamPolicySpec::PublicBudgeted {
            delay_ns: 0,
            k: 3,
            score: Score::default()
        }
    );
    let written = serde_json::to_value(&bare).unwrap();
    assert_eq!(written["score"]["threshold"], json!(0.0));
    assert!(
        written.get("delay_ns").is_none(),
        "a zero delay is not written"
    );
    // What the manifest refuses.
    for bad in [
        json!({"policy": "public_budgeted"}),
        json!({"policy": "public_budgeted", "k": 0}),
        json!({"policy": "public_budgeted", "k": 2, "persist_ns": 1}),
        json!({"policy": "public_budgeted", "k": 2, "score": {"threshold": 0.0, "bogus": 1.0}}),
        json!({"policy": "public_budgeted", "k": 2, "score": {"threshold": 0.0, "evidence": "x"}}),
        json!({"policy": "public_change", "k": 2, "score": {"threshold": 0.0}}),
        json!({"policy": "never_escalate", "score": {"threshold": 0.0}}),
    ] {
        assert!(
            serde_json::from_value::<StreamPolicySpec>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(StreamPolicySpec::from_id("public_budgeted").is_err());
    // A score is written whole: a score with a field missing is refused (what ran is recorded in
    // full), and a non-finite number cannot be written in JSON at all.
    let partial: Result<StreamPolicySpec, _> = serde_json::from_value(
        json!({"policy": "public_budgeted", "k": 2, "score": {"threshold": 0.5, "evidence": 1.0}}),
    );
    assert!(partial.is_err());
    let s = StreamPolicySpec::PublicBudgeted {
        delay_ns: 0,
        k: 1,
        score: Score {
            threshold: f64::NAN,
            ..Score::default()
        },
    };
    assert!(s.validate().is_err(), "a non-finite threshold is refused");
}

// ---- the new view field: distinct services of the evidence, for every noticer

fn exploration(file: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../experiments/exploration")
        .join(file);
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")),
    )
    .unwrap()
}

/// The noticers of the table that a run can spell from the committed selections, by name.
fn noticers() -> Vec<(&'static str, NoticerSpec)> {
    let sel = exploration("b3-selected.json");
    let m2 = exploration("m2-selected.json");
    let r = &sel["ramp"]["chosen"]["params"];
    let sp = &sel["split_re2"]["chosen"]["params"];
    let ms = 1_000_000u64;
    let n = |v: &Value| v.as_u64().unwrap();
    let base = json!({"noticer": "reanchor", "notice_z": 2.0, "gap_ns": 20 * ms, "min_burst": 2,
                      "isolation": "site"});
    let ramp = json!({"gap_ns": n(&r["gap_ms"]) * ms, "max_step": n(&r["max_step"]),
                      "max_drop": n(&r["max_drop"]), "min_readings": n(&r["min_readings"]),
                      "min_rise": n(&r["min_rise"])});
    let split = json!({"gap_ns": n(&sp["gap_ms"]) * ms, "min_burst": n(&sp["min_burst"])});
    let composed = json!({"noticer": "composed", "base": base, "ramp": ramp, "split": split});
    let dataflow = json!({"noticer": "dataflow", "base": base, "ramp": ramp, "split": split});
    let parse = |v: Value| serde_json::from_value::<NoticerSpec>(v).unwrap();
    vec![
        ("rung", parse(json!({"noticer": "rung"}))),
        ("reanchor", parse(base.clone())),
        ("ramp+split over re-anchor", parse(composed)),
        ("dataflow", parse(dataflow)),
        ("medium", parse(m2["ticks"]["100"]["noticer"].clone())),
    ]
}

#[test]
fn the_service_count_of_a_view_is_the_distinct_services_of_the_attached_evidence_for_every_noticer()
{
    let mut sp = StreamParams::new(10_003);
    sp.duration_ns = 150 * S;
    let stream = generate(&sp);
    let public = stream.public_info();
    let held: Vec<Held> = stream
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
    let mut total_checked = 0u64;
    let mut multi = 0u64;
    for (name, spec) in noticers() {
        let cfg = RungConfig::default();
        let mut n: Box<dyn Noticer> = noticer::build(&spec, &cfg, &public.services);
        let mut window: VecDeque<Held> = VecDeque::new();
        let (mut next, mut now) = (0usize, 0u64);
        while now < sp.duration_ns {
            while next < held.len() && held[next].at.0 <= now {
                if held[next].abnormal {
                    n.observe(&held[next]);
                }
                window.push_back(held[next].clone());
                next += 1;
            }
            while window
                .front()
                .is_some_and(|h| h.at.0.saturating_add(10 * S) < now)
            {
                window.pop_front();
            }
            n.notice(Instant(now), &Store::with(window.iter().cloned()));
            n.refresh(Instant(now));
            for t in n.anomalies() {
                let distinct: BTreeSet<ServiceId> = t.attached.iter().map(|(_, _, s)| *s).collect();
                assert_eq!(
                    t.service_count(),
                    distinct.len(),
                    "{name}: anomaly {} at {now}",
                    t.id
                );
                total_checked += 1;
                multi += u64::from(distinct.len() > 1);
            }
            for id in n.retirable(Instant(now)) {
                n.retire(id);
            }
            now += 500_000_000;
        }
    }
    assert!(
        total_checked > 1000,
        "{total_checked} anomaly-steps checked"
    );
    assert!(multi > 0, "some anomaly spans more than one service");
}

// ---- in the harness

/// Per notice of an arm: (seed, anomaly) -> (calls about it, first call's instant).
fn asked(out: &std::path::Path, arm: &str) -> BTreeMap<(u64, u64), (u64, Option<u64>)> {
    let t = rows(&read(&out.join(arm), "selection_notices.csv"));
    let col = |name: &str| t[0].iter().position(|c| c == name).unwrap();
    t[1..]
        .iter()
        .map(|r| {
            (
                (
                    r[col("seed")].parse().unwrap(),
                    r[col("anomaly")].parse().unwrap(),
                ),
                (
                    r[col("escalations")].parse().unwrap(),
                    r[col("first_escalation_at_ns")].parse().ok(),
                ),
            )
        })
        .collect()
}

fn budgeted(k: u32, score: Score, delay_ns: u64) -> StreamPolicySpec {
    StreamPolicySpec::PublicBudgeted { delay_ns, k, score }
}

fn calls_per_seed(out: &std::path::Path, arm: &str) -> Vec<u64> {
    let t = rows(&read(&out.join(arm), "results.csv"));
    let c = t[0].iter().position(|c| c == "reasoner_calls").unwrap();
    t[1..].iter().map(|r| r[c].parse().unwrap()).collect()
}

#[test]
fn a_budget_that_never_binds_asks_about_what_always_escalate_asks_about_and_a_budget_of_k_about_the_first_k()
 {
    let mut m = manifest(
        "b5-harness",
        &[
            ("always", "always_escalate"),
            ("open", "never_escalate"),
            ("first3", "never_escalate"),
            ("never", "never_escalate"),
        ],
        4,
        300,
        7,
    );
    m.arms[0].policy = StreamPolicySpec::Always { delay_ns: 4 * S };
    m.arms[1].policy = budgeted(1_000_000, flat(-1.0), 4 * S);
    m.arms[2].policy = budgeted(3, flat(0.0), 4 * S);
    m.validate().unwrap();
    let out = scratch("b5-harness").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));

    let always = asked(&out, "always");
    let open = asked(&out, "open");
    let first3 = asked(&out, "first3");
    let calls_always: u64 = always.values().map(|(c, _)| c).sum();
    assert!(
        calls_always > 12,
        "{calls_always} calls: enough to bind a budget of 3 per stream"
    );
    // Never binding: the same notices are asked about, once each. The checker's billing moves the
    // logical clock by microseconds, so the notice record is compared, not the instants.
    let asked_set = |m: &BTreeMap<(u64, u64), (u64, Option<u64>)>| -> BTreeSet<(u64, u64)> {
        m.iter()
            .filter(|(_, (c, _))| *c > 0)
            .map(|(k, _)| *k)
            .collect()
    };
    assert_eq!(asked_set(&open), asked_set(&always));
    assert!(open.values().all(|(c, _)| *c <= 1) && always.values().all(|(c, _)| *c <= 1));
    // A budget of 3 and a flat score: at most 3 calls per segment, exactly the earliest 3 that
    // always-escalate makes in each segment.
    for calls in calls_per_seed(&out, "first3") {
        assert!(calls <= 3, "{calls}");
    }
    let mut by_seed: BTreeMap<u64, Vec<(u64, u64)>> = BTreeMap::new();
    for ((seed, anomaly), (c, at)) in &always {
        if *c > 0 {
            by_seed
                .entry(*seed)
                .or_default()
                .push((at.unwrap(), *anomaly));
        }
    }
    let mut spent = 0;
    for (seed, mut v) in by_seed {
        v.sort();
        let expected: BTreeSet<(u64, u64)> = v.iter().take(3).map(|(_, a)| (seed, *a)).collect();
        let got: BTreeSet<(u64, u64)> = first3
            .iter()
            .filter(|((s, _), (c, _))| *s == seed && *c > 0)
            .map(|(k, _)| *k)
            .collect();
        assert_eq!(got, expected, "seed {seed}");
        spent += got.len();
    }
    assert!(spent >= 6, "{spent}");
    // The arm that never asks is the floor.
    assert!(calls_per_seed(&out, "never").iter().all(|c| *c == 0));
}

#[test]
fn a_score_that_prefers_evidence_asks_about_different_anomalies_than_a_flat_one() {
    let mut m = manifest(
        "b5-weights",
        &[("flat", "never_escalate"), ("evidence", "never_escalate")],
        4,
        300,
        7,
    );
    // k = 4 and a threshold on the evidence feature that some ready anomalies do not reach: with
    // the threshold at zero the score would only order the anomalies of one step.
    m.arms[0].policy = budgeted(4, flat(0.0), 4 * S);
    m.arms[1].policy = budgeted(
        4,
        Score {
            threshold: 0.6,
            ..EVIDENCE_ONLY
        },
        4 * S,
    );
    m.validate().unwrap();
    let out = scratch("b5-weights").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    let set = |arm: &str| -> BTreeSet<(u64, u64)> {
        asked(&out, arm)
            .into_iter()
            .filter(|(_, (c, _))| *c > 0)
            .map(|(k, _)| k)
            .collect()
    };
    assert_ne!(set("flat"), set("evidence"));
    for arm in ["flat", "evidence"] {
        assert!(calls_per_seed(&out, arm).iter().all(|c| *c <= 4), "{arm}");
    }
}

#[test]
fn a_budgeted_run_replays_byte_for_byte() {
    let mut m = manifest("b5-replay", &[("a", "never_escalate")], 2, 200, 4);
    m.arms[0].policy = budgeted(
        4,
        Score {
            threshold: 0.3,
            contradiction: 1.0,
            silence: 0.5,
            evidence: 1.0,
            services: 0.5,
            age: 0.1,
        },
        4 * S,
    );
    let run = |name: &str| {
        let out = scratch(name).join("run");
        execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
        out
    };
    let (x, y) = (run("b5-replay-a"), run("b5-replay-b"));
    for file in [
        "results.csv",
        "incidents.csv",
        "selection.csv",
        "selection_notices.csv",
    ] {
        assert_eq!(read(&x.join("a"), file), read(&y.join("a"), file), "{file}");
    }
}

#[test]
fn every_view_in_a_run_has_at_least_one_service_and_no_more_than_its_evidence() {
    // A recording rule over real segments: asks about nothing, writes down every view it is shown.
    use gordian_run::stream::arms::StreamArm;
    use gordian_run::stream::run_segment;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Recorder(Rc<RefCell<Vec<AnomalyView>>>);
    impl EscalationRule for Recorder {
        fn id(&self) -> gordian_run::policy::PolicyId {
            gordian_run::policy::PolicyId::new("recorder")
        }
        fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
            self.0.borrow_mut().extend_from_slice(views);
            Vec::new()
        }
    }
    let params = params(10_001, 200);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);
    run_segment(
        &params,
        &|public: &gordian_stream::StreamPublic| {
            Box::new(StreamArm::with(
                Recorder(Rc::clone(&sink)),
                public,
                RungConfig::default(),
            ))
        },
        &limits(&params),
        &gordian_run::stream::manifest::Exchange::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let views = seen.borrow();
    assert!(views.len() > 20, "{} views", views.len());
    for v in views.iter() {
        assert!(v.services >= 1, "an anomaly with evidence spans a service");
        assert!(v.services <= v.evidence.max(1), "{v:?}");
    }
    // The count is of distinct services, not of observations: some anomaly holds several observations
    // at one service (fewer services than evidence), and some spans more than one service.
    assert!(views.iter().any(|v| v.services < v.evidence));
    assert!(views.iter().any(|v| v.services > 1));
    assert!(views.iter().any(|v| v.services == 1));
}
