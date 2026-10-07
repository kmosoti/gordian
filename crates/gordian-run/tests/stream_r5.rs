//! The arms of work item R5 (`contradiction_escalation`, `oracle_selection_privileged`,
//! `oracle_decoy_privileged`) and the check that adding them changed no existing result.
//!
//! What is pinned, and by what:
//!
//! - the three arms are deterministic (same stream, same rows and trajectory);
//! - the selection oracle makes exactly the calls `always_escalate` makes at the same delay about
//!   hard incidents, with identical contexts: so it escalates only hard incidents, every hard
//!   anomaly, and uses the rung's own context (an independent arm's, not a copy of it);
//! - the decoy oracle escalates nothing, and the only dismissals it adds to `never_escalate`'s are
//!   about decoys;
//! - the contradiction arm is a public arm: built from public information, in a file the textual
//!   guard covers, and the verdicts the rung keeps for it change what is *paid* but not what is
//!   noticed or declared;
//! - every arm that existed before R5 writes the same `results.csv` and `incidents.csv` it wrote
//!   before the work (fixtures written by the binary of commit `22ca528`).

mod stream_common;

use gordian_core::Instant;
use gordian_run::policy::PolicyId;
use gordian_run::stream::arms::contradiction::Contradiction;
use gordian_run::stream::arms::rung::{AnomalyView, RungConfig};
use gordian_run::stream::arms::{ArmRole, EscalationRule, StreamArm, StreamPolicy};
use gordian_run::stream::manifest::{Exchange, StreamManifest};
use gordian_run::stream::results::results_row;
use gordian_run::stream::spec::{StreamPolicySpec, build_public, privileged_factory};
use gordian_run::stream::{SegmentRecord, execute_stream, run_segment};
use gordian_stream::{
    ObsId, ObsRef, Question, StreamAction, StreamOutcome, StreamParams, StreamPublic, Tier,
    generate,
};
use gordian_stream_eval::truth_from_stream;
use gordian_world::ServiceId;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use stream_common::*;

const NS: u64 = 1_000_000_000;

fn secs(s: u64) -> Instant {
    Instant(s * NS)
}

fn view(id: u32) -> AnomalyView {
    AnomalyView {
        id,
        site: ServiceId(0),
        anchor: ObsId(id),
        anchor_at: Instant(0),
        noticed_at: secs(1),
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
        services: 2,
    }
}

fn mix(seed: u64, plain: u32, hard: u32) -> StreamParams {
    let mut p = params(seed, 300);
    p.mix.plain_permille = plain;
    p.mix.hard_permille = hard;
    p
}

fn hard_heavy(seed: u64) -> StreamParams {
    mix(seed, 300, 500)
}

fn decoy_heavy(seed: u64) -> StreamParams {
    mix(seed, 300, 100)
}

/// The actions of a trajectory, in order, without the instants and outcomes: the logical clock
/// advances by the busy time of every component run, so an arm that is charged for more checks
/// acts a few microseconds later, which is cost and not a difference in what it does.
fn actions(record: &SegmentRecord) -> Vec<StreamAction> {
    record.trajectory.iter().map(|s| s.action.clone()).collect()
}

/// Which incidents were declared correctly by their deadline: the verdict without its instants.
fn correct_flags(record: &SegmentRecord) -> Vec<(u32, bool, u32, u32)> {
    record
        .verdict
        .per_incident
        .iter()
        .map(|i| {
            (
                i.id,
                i.correct_by_deadline,
                i.correct_declarations,
                i.wrong_declarations,
            )
        })
        .collect()
}

/// Every accepted escalation of a trajectory: instant, focus, context.
fn calls(record: &SegmentRecord) -> BTreeSet<(u64, u32, Vec<ObsRef>)> {
    record
        .trajectory
        .iter()
        .filter_map(|step| match (&step.action, &step.outcome) {
            (StreamAction::Escalate { context, question }, StreamOutcome::Escalated { .. }) => {
                let Question::Diagnose { focus } = question;
                Some((step.at.0, focus.0, context.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The anchors of every accepted declaration that names no incident.
fn dismissals(record: &SegmentRecord) -> BTreeSet<u32> {
    record
        .trajectory
        .iter()
        .filter_map(|step| match (&step.action, &step.outcome) {
            (
                StreamAction::Declare {
                    anchor,
                    diagnosis: None,
                },
                StreamOutcome::Declared { .. },
            ) => Some(anchor.0),
            _ => None,
        })
        .collect()
}

// ---- The manifest spelling

#[test]
fn the_new_arms_are_written_as_their_ids_or_with_their_parameters() {
    let from = |text: &str| serde_json::from_str::<StreamPolicySpec>(text);
    let to = |spec: &StreamPolicySpec| serde_json::to_string(spec).unwrap();
    assert_eq!(
        from("\"contradiction_escalation\"").unwrap(),
        StreamPolicySpec::Contradiction {
            delay_ns: 0,
            persist_ns: 0
        }
    );
    let full = from(
        r#"{"policy": "contradiction_escalation", "delay_ns": 8000000000, "persist_ns": 2000000000}"#,
    )
    .unwrap();
    assert_eq!(
        full,
        StreamPolicySpec::Contradiction {
            delay_ns: 8 * NS,
            persist_ns: 2 * NS
        }
    );
    assert_eq!(
        to(&full),
        r#"{"policy":"contradiction_escalation","delay_ns":8000000000,"persist_ns":2000000000}"#
    );
    // A zero is not written; a bare id is the all-zero arm.
    assert_eq!(
        to(&StreamPolicySpec::Contradiction {
            delay_ns: 0,
            persist_ns: 0
        }),
        "\"contradiction_escalation\""
    );
    assert_eq!(
        to(&StreamPolicySpec::Contradiction {
            delay_ns: 0,
            persist_ns: 4 * NS
        }),
        r#"{"policy":"contradiction_escalation","persist_ns":4000000000}"#
    );
    assert_eq!(
        from(r#"{"policy": "oracle_selection", "delay_ns": 6000000000}"#).unwrap(),
        StreamPolicySpec::OracleSelection {
            delay_ns: 6 * NS,
            hold_until_asked: false
        }
    );
    assert_eq!(
        to(&StreamPolicySpec::OracleSelection {
            delay_ns: 0,
            hold_until_asked: false
        }),
        "\"oracle_selection\""
    );
    assert_eq!(to(&StreamPolicySpec::OracleDecoy), "\"oracle_decoy\"");
    // Strays are errors.
    for bad in [
        r#"{"policy": "oracle_decoy", "delay_ns": 1}"#,
        r#"{"policy": "oracle_selection", "persist_ns": 1}"#,
        r#"{"policy": "always_escalate", "persist_ns": 1}"#,
        r#"{"policy": "contradiction_escalation", "p": 0.5}"#,
        r#"{"policy": "oracle_escalation", "delay_ns": 1}"#,
    ] {
        assert!(from(bad).is_err(), "{bad}");
    }
    // Roles, and the names a manifest demands.
    assert_eq!(
        StreamPolicySpec::from_id("oracle_selection")
            .unwrap()
            .role(),
        ArmRole::Privileged
    );
    assert_eq!(
        StreamPolicySpec::from_id("oracle_decoy").unwrap().role(),
        ArmRole::Privileged
    );
    assert_eq!(
        StreamPolicySpec::from_id("contradiction_escalation")
            .unwrap()
            .role(),
        ArmRole::Comparison
    );
    let m = manifest("names", &[("never_escalate", "never_escalate")], 1, 150, 0);
    let named = |name: &str, policy: &str| {
        let mut m = m.clone();
        m.arms[0].arm = name.to_owned();
        m.arms[0].policy = StreamPolicySpec::from_id(policy).unwrap();
        m.validate()
    };
    assert!(named("oracle_selection", "oracle_selection").is_err());
    assert!(named("oracle_selection_privileged", "oracle_selection").is_ok());
    assert!(named("oracle_decoy", "oracle_decoy").is_err());
    assert!(named("oracle_decoy_privileged", "oracle_decoy").is_ok());
    assert!(named("contradiction_privileged", "contradiction_escalation").is_err());
    assert!(named("contradiction_escalation", "contradiction_escalation").is_ok());
}

#[test]
fn only_the_contradiction_arm_is_built_from_public_information() {
    let p = params(1, 150);
    let public = public_of(&p);
    let rung = RungConfig::default();
    let contradiction = StreamPolicySpec::from_id("contradiction_escalation").unwrap();
    let arm = build_public(
        &contradiction,
        &rung,
        &public,
        "contradiction_escalation",
        1,
    )
    .unwrap();
    assert_eq!(arm.role(), ArmRole::Comparison);
    assert_eq!(arm.decision_rule(), gordian_run::policy::decide::RULE);
    assert!(privileged_factory(&contradiction, &rung).is_none());
    for id in ["oracle_selection", "oracle_decoy"] {
        let spec = StreamPolicySpec::from_id(id).unwrap();
        assert!(
            build_public(&spec, &rung, &public, &format!("{id}_privileged"), 1).is_none(),
            "{id} is not a public arm"
        );
        assert!(privileged_factory(&spec, &rung).is_some(), "{id}");
    }
}

// ---- Determinism

#[test]
fn every_new_arm_is_deterministic() {
    let specs = [
        StreamPolicySpec::Contradiction {
            delay_ns: 4 * NS,
            persist_ns: 2 * NS,
        },
        StreamPolicySpec::Contradiction {
            delay_ns: 0,
            persist_ns: 0,
        },
        StreamPolicySpec::OracleSelection {
            delay_ns: 6 * NS,
            hold_until_asked: false,
        },
        StreamPolicySpec::OracleDecoy,
    ];
    for seed in [3, 8] {
        let p = hard_heavy(seed);
        let l = limits(&p);
        for spec in &specs {
            let a = play(&p, spec, &l).unwrap();
            let b = play(&p, spec, &l).unwrap();
            assert_eq!(
                results_row("x", &a),
                results_row("x", &b),
                "{spec:?} {seed}"
            );
            assert_eq!(a.trajectory, b.trajectory, "{spec:?} {seed}");
            assert_eq!(a.verdict, b.verdict, "{spec:?} {seed}");
        }
    }
}

// ---- The contradiction rule

#[test]
fn the_contradiction_rule_escalates_once_after_the_delay_and_the_persistence() {
    let mut rule = Contradiction::new(4 * NS, 2 * NS);
    assert!(rule.monitors());
    // Noticed at 1 s. Never contradicted: never escalated.
    let quiet = view(1);
    assert!(rule.targets(secs(100), &[quiet]).is_empty());
    // Contradicted since 3 s: persistence is met at 5 s, the delay at 5 s (1 + 4).
    let mut v = view(2);
    v.contradicted_since = Some(secs(3));
    assert!(rule.targets(secs(4), std::slice::from_ref(&v)).is_empty());
    assert_eq!(rule.targets(secs(5), std::slice::from_ref(&v)), vec![2]);
    assert_eq!(rule.targets(secs(50), std::slice::from_ref(&v)), vec![2]);
    // Persistence not yet met although the delay has passed.
    let mut late = view(3);
    late.contradicted_since = Some(secs(20));
    assert!(
        rule.targets(secs(21), std::slice::from_ref(&late))
            .is_empty()
    );
    assert_eq!(rule.targets(secs(22), std::slice::from_ref(&late)), vec![3]);
    // Delay not yet met although the contradiction has persisted.
    let mut early = view(4);
    early.noticed_at = secs(10);
    early.contradicted_since = Some(secs(0));
    assert!(
        rule.targets(secs(13), std::slice::from_ref(&early))
            .is_empty()
    );
    assert_eq!(
        rule.targets(secs(14), std::slice::from_ref(&early)),
        vec![4]
    );
    // Once escalated, never again.
    v.attempts = 1;
    assert!(rule.targets(secs(50), &[v]).is_empty());
    // Zero and zero is the first contradictory check after notice.
    let mut zero = Contradiction::new(0, 0);
    let mut c = view(5);
    c.contradicted_since = Some(secs(2));
    assert_eq!(zero.targets(secs(2), &[c]), vec![5]);
    // Every other rule leaves the rung alone.
    assert!(!gordian_run::stream::arms::always::Always::default().monitors());
    assert!(!gordian_run::stream::arms::never::Never.monitors());
    assert!(!gordian_run::stream::arms::change::Change.monitors());
}

/// A rule that escalates nothing and records what it is shown, with the monitor on or off.
struct Recorder {
    seen: Rc<RefCell<Vec<AnomalyView>>>,
    monitor: bool,
}

impl EscalationRule for Recorder {
    fn id(&self) -> PolicyId {
        PolicyId::new("never_escalate")
    }

    fn monitors(&self) -> bool {
        self.monitor
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        self.seen.borrow_mut().extend_from_slice(views);
        Vec::new()
    }
}

fn run_recorder(p: &StreamParams, monitor: bool) -> (SegmentRecord, Vec<AnomalyView>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let handle = Rc::clone(&seen);
    let record = run_segment(
        p,
        &move |public: &StreamPublic| -> Box<dyn StreamPolicy> {
            Box::new(StreamArm::with(
                Recorder {
                    seen: Rc::clone(&handle),
                    monitor,
                },
                public,
                RungConfig::default(),
            ))
        },
        &limits(p),
        &Exchange::default(),
    )
    .unwrap();
    let views = seen.borrow().clone();
    (record, views)
}

#[test]
fn the_rung_keeps_a_contradiction_verdict_only_when_the_rule_asks_and_it_costs_but_changes_nothing()
{
    let mut contradicted = 0usize;
    let mut views_total = 0usize;
    for seed in [3, 8, 11] {
        let p = hard_heavy(seed);
        let (off, off_views) = run_recorder(&p, false);
        let (on, on_views) = run_recorder(&p, true);
        // Off: the verdict is never kept, so nothing about the rung differs from before.
        assert!(
            off_views.iter().all(|v| v.contradicted_since.is_none()),
            "a rule that does not ask is shown a verdict (seed {seed})"
        );
        // On: the same anomalies, the same declarations, the same verdicts of the evaluator.
        assert_eq!(actions(&off), actions(&on), "seed {seed}");
        assert_eq!(correct_flags(&off), correct_flags(&on), "seed {seed}");
        assert_eq!(off.verdict.totals, on.verdict.totals, "seed {seed}");
        assert_eq!(off.anomalies_noticed, on.anomalies_noticed, "seed {seed}");
        let strip = |views: &[AnomalyView]| -> Vec<AnomalyView> {
            views
                .iter()
                .map(|v| AnomalyView {
                    contradicted_since: None,
                    ..v.clone()
                })
                .collect()
        };
        assert_eq!(strip(&off_views), strip(&on_views), "seed {seed}");
        // The checks are paid for: more component runs, so more modelled cost.
        assert!(on.components_run > off.components_run, "seed {seed}");
        assert!(on.substrate_ns() > off.substrate_ns(), "seed {seed}");
        // The verdict is a start time at or after notice.
        for v in &on_views {
            if let Some(since) = v.contradicted_since {
                assert!(since >= v.noticed_at, "a verdict before the notice");
                contradicted += 1;
            }
        }
        views_total += on_views.len();
    }
    assert!(views_total > 0);
    assert!(
        contradicted > 0,
        "the checker found a contradiction in none of {views_total} views of hard-heavy streams"
    );
}

#[test]
fn a_contradiction_arm_that_never_fires_is_never_escalate_with_the_checks_paid_for() {
    let p = hard_heavy(5);
    let l = limits(&p);
    let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    let silent = play(
        &p,
        &StreamPolicySpec::Contradiction {
            delay_ns: 1_000_000 * NS,
            persist_ns: 0,
        },
        &l,
    )
    .unwrap();
    assert_eq!(actions(&never), actions(&silent));
    assert_eq!(correct_flags(&never), correct_flags(&silent));
    assert_eq!(never.verdict.totals, silent.verdict.totals);
    assert_eq!(silent.verdict.totals.reasoner.calls, 0);
    assert!(silent.substrate_ns() > never.substrate_ns());
}

#[test]
fn the_contradiction_arm_asks_about_what_always_asks_about_no_earlier_and_with_the_rungs_context() {
    // Both arms escalate an anomaly once, `delay` after it is noticed at the earliest, and build the
    // context with the rung's one function. So a call of the contradiction arm is a call of
    // `always_escalate` at the same delay about the same focus, no earlier; when the two are at
    // the same instant the contexts are identical. (The contradiction arm is later when the
    // checker finds the contradiction after the delay.)
    let (mut same_instant, mut later, mut calls_total) = (0usize, 0usize, 0usize);
    for seed in [3, 8, 11] {
        let p = hard_heavy(seed);
        let l = limits(&p);
        let delay_ns = 6 * NS;
        let always = play(&p, &StreamPolicySpec::Always { delay_ns }, &l).unwrap();
        let contra = play(
            &p,
            &StreamPolicySpec::Contradiction {
                delay_ns,
                persist_ns: 0,
            },
            &l,
        )
        .unwrap();
        assert_eq!(always.counts.escalations_refused, 0);
        assert_eq!(contra.counts.escalations_refused, 0);
        let a = calls(&always);
        for (at, focus, context) in calls(&contra) {
            calls_total += 1;
            let matching: Vec<_> = a.iter().filter(|(_, f, _)| *f == focus).collect();
            assert_eq!(
                matching.len(),
                1,
                "seed {seed}: always makes one call about focus {focus}"
            );
            let (a_at, _, a_context) = matching[0];
            assert!(
                at >= *a_at,
                "seed {seed}: the contradiction arm asked first"
            );
            if at == *a_at {
                assert_eq!(&context, a_context, "seed {seed} focus {focus}");
                same_instant += 1;
            } else {
                later += 1;
            }
        }
    }
    assert!(
        calls_total > 0,
        "the contradiction arm made no call on hard-heavy streams"
    );
    assert!(
        same_instant > 0,
        "no call at the same instant as always ({later} later)"
    );
}

// ---- The selection oracle

#[test]
fn the_selection_oracle_is_always_escalate_restricted_to_hard_incidents_with_the_same_contexts() {
    let mut hard_calls = 0usize;
    let mut skipped_other = 0usize;
    for seed in 0..5 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let l = limits(&p);
        for delay in [0, 4 * NS] {
            let always = play(&p, &StreamPolicySpec::Always { delay_ns: delay }, &l).unwrap();
            let selection = play(
                &p,
                &StreamPolicySpec::OracleSelection {
                    delay_ns: delay,
                    hold_until_asked: false,
                },
                &l,
            )
            .unwrap();
            // The comparison is only valid while nothing is refused for lack of budget.
            assert_eq!(always.counts.escalations_refused, 0, "seed {seed}");
            assert_eq!(selection.counts.escalations_refused, 0, "seed {seed}");
            let hard_of = |focus: u32| {
                truth
                    .incident_of(ObsId(focus))
                    .is_some_and(|i| truth.incidents[i as usize].tier == Tier::Hard)
            };
            let expected: BTreeSet<_> = calls(&always)
                .into_iter()
                .filter(|(_, focus, _)| hard_of(*focus))
                .collect();
            skipped_other += calls(&always).len() - expected.len();
            let got = calls(&selection);
            // Only hard incidents are asked about...
            assert!(
                got.iter().all(|(_, focus, _)| hard_of(*focus)),
                "seed {seed}"
            );
            // ...every hard anomaly is, at the same instant and with the rung's own context.
            assert_eq!(got, expected, "seed {seed} delay {delay}");
            hard_calls += got.len();
            assert_eq!(selection.role, ArmRole::Privileged);
            assert_eq!(selection.arm_id, "oracle_selection");
        }
    }
    assert!(
        hard_calls >= 10,
        "the test exercised {hard_calls} hard calls"
    );
    assert!(
        skipped_other > 0,
        "always escalated nothing but hard incidents"
    );
}

#[test]
fn the_selection_oracle_does_not_build_a_context_of_its_own() {
    // The arm's contexts are not the decisive sets the other oracle sends: on hard-heavy streams
    // at least one of its calls has a context that differs from every decisive set.
    let mut differs = 0usize;
    for seed in 0..4 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let selection = play(
            &p,
            &StreamPolicySpec::OracleSelection {
                delay_ns: 4 * NS,
                hold_until_asked: false,
            },
            &limits(&p),
        )
        .unwrap();
        let decisive: Vec<BTreeSet<ObsRef>> = truth
            .incidents
            .iter()
            .filter(|i| i.tier == Tier::Hard)
            .map(|i| i.decisive.iter().map(|o| ObsRef::Passive(*o)).collect())
            .collect();
        for (_, _, context) in calls(&selection) {
            let set: BTreeSet<ObsRef> = context.iter().copied().collect();
            if !decisive.contains(&set) {
                differs += 1;
            }
        }
    }
    assert!(differs > 0);
}

// ---- The decoy oracle

#[test]
fn the_decoy_oracle_dismisses_decoys_and_escalates_nothing() {
    let (mut added, mut dismissed_dec, mut dismissed_never) = (0usize, 0u32, 0u32);
    let (mut alarmed_dec, mut alarmed_never) = (0u32, 0u32);
    for seed in 0..5 {
        let p = decoy_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let l = limits(&p);
        let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
        let decoy = play(&p, &StreamPolicySpec::OracleDecoy, &l).unwrap();
        assert_eq!(decoy.role, ArmRole::Privileged);
        assert_eq!(decoy.arm_id, "oracle_decoy");
        // Escalates nothing.
        assert!(calls(&decoy).is_empty());
        assert_eq!(decoy.verdict.totals.reasoner.calls, 0);
        assert_eq!(decoy.verdict.totals.escalations.needed, 0);
        // The dismissals it adds to never_escalate's are all about decoys.
        let base = dismissals(&never);
        for anchor in dismissals(&decoy).difference(&base) {
            let incident = truth
                .incident_of(ObsId(*anchor))
                .unwrap_or_else(|| panic!("seed {seed}: dismissed background {anchor}"));
            assert_eq!(
                truth.incidents[incident as usize].tier,
                Tier::Decoy,
                "seed {seed}: dismissed a non-decoy"
            );
            added += 1;
        }
        // Plain and hard outcomes are never_escalate's: it touches nothing else.
        assert_eq!(
            decoy.verdict.totals.correct, never.verdict.totals.correct,
            "seed {seed}"
        );
        assert_eq!(
            decoy.verdict.totals.missed, never.verdict.totals.missed,
            "seed {seed}"
        );
        dismissed_dec += decoy.verdict.totals.decoys_dismissed;
        dismissed_never += never.verdict.totals.decoys_dismissed;
        alarmed_dec += decoy.verdict.totals.decoys_alarmed;
        alarmed_never += never.verdict.totals.decoys_alarmed;
    }
    assert!(added > 0, "the decoy oracle dismissed nothing new");
    assert!(dismissed_dec > dismissed_never);
    assert!(alarmed_dec < alarmed_never);
}

#[test]
fn the_decoy_oracle_dismisses_nothing_on_a_stream_without_decoys() {
    let mut p = params(3, 200);
    p.mix.plain_permille = 700;
    p.mix.hard_permille = 300;
    let l = limits(&p);
    let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    let decoy = play(&p, &StreamPolicySpec::OracleDecoy, &l).unwrap();
    assert_eq!(never.trajectory, decoy.trajectory);
    assert_eq!(never.verdict, decoy.verdict);
}

// ---- Nothing that existed changed

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stream-r5-regression")
}

#[test]
fn every_arm_that_existed_before_r5_writes_the_results_it_wrote_before() {
    let text = fs::read_to_string(fixture_dir().join("manifest.json")).unwrap();
    let m: StreamManifest = serde_json::from_str(&text).expect("the pre-R5 manifest parses");
    assert_eq!(m.arms.len(), 8);
    let out = scratch("r5-regression").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    for arm in &m.arms {
        for file in ["results.csv", "incidents.csv"] {
            let expected =
                fs::read_to_string(fixture_dir().join(format!("{}.{file}", arm.arm))).unwrap();
            let got = fs::read_to_string(out.join(&arm.arm).join(file)).unwrap();
            assert!(
                expected == got,
                "{} {file} differs from the fixture written before R5",
                arm.arm
            );
        }
    }
}
