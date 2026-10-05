//! The stream evaluator inside the stream harness (work item R3b): that a real segment is scored
//! by it, that its totals agree with independent readings of the same trajectory, that the
//! per-incident file and the per-stream file say the same thing, that the hard-fault family is
//! written only where evaluator output goes, and that the call record's focus is cross-checked.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::results::{INCIDENTS_HEADER, RESULTS_HEADER};
use gordian_run::stream::score::{StreamEvalError, StreamStep};
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_run::stream::{StreamHarnessError, execute_stream};
use gordian_stream::{
    ObsId, Question, StreamAction, StreamOutcome, StreamParams, StreamSimulator, generate,
};
use gordian_stream_eval::{calls_from_sim, score_stream, truth_from_stream};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use stream_common::*;

/// A stream with plenty of hard incidents, so that every tier and family appears.
fn hard_heavy(seed: u64) -> StreamParams {
    let mut p = params(seed, 300);
    p.mix.plain_permille = 300;
    p.mix.hard_permille = 500;
    p
}

fn run(m: &StreamManifest, name: &str) -> PathBuf {
    let out = scratch(name).join("run");
    execute_stream(m, &out).unwrap_or_else(|e| panic!("{e}"));
    out
}

// ---- The totals agree with independent readings

#[test]
fn a_segment_is_scored_by_the_evaluator_and_its_totals_agree_with_the_trajectory_and_the_bill() {
    let specs = [
        StreamPolicySpec::Never,
        StreamPolicySpec::Always,
        StreamPolicySpec::from_id("threshold_score").unwrap(),
        StreamPolicySpec::Oracle,
    ];
    let mut tiers_seen = [0u32; 3];
    for seed in 0..4 {
        let p = hard_heavy(seed);
        let l = limits(&p);
        for spec in &specs {
            let record = play(&p, spec, &l).unwrap();
            let v = &record.verdict;
            let t = &v.totals;
            let who = format!("{} seed {seed}", spec.id().0);

            // One verdict per incident of the truth, and the tier counts are over them.
            let n = u32::try_from(v.per_incident.len()).unwrap();
            assert_eq!(
                t.incidents.plain + t.incidents.hard + t.incidents.decoy,
                n,
                "{who}"
            );
            assert_eq!(
                record.incident_families.len(),
                v.per_incident.len(),
                "{who}"
            );
            tiers_seen[0] += t.incidents.plain;
            tiers_seen[1] += t.incidents.hard;
            tiers_seen[2] += t.incidents.decoy;

            // Two readings of the same trajectory agree: the evaluator's reasoner usage, the
            // trajectory counts, the bill's communication and the manifest's exchange rate.
            assert_eq!(
                t.reasoner.calls,
                u64::from(record.trajectory_counts.reasoner_calls)
            );
            assert_eq!(
                t.reasoner.tokens,
                record.bill.total(gordian_core::Resource::Communication)
            );
            assert_eq!(t.reasoner.modelled_ns, t.reasoner.tokens * 250_000, "{who}");
            assert_eq!(
                record.reasoner_cost_ns,
                t.reasoner.tokens * 250_000,
                "{who}"
            );

            // Every accepted escalation is classified exactly once (S23), and by incident (S13).
            let e = &t.escalations;
            assert_eq!(
                u64::from(e.needed + e.unneeded + e.background),
                t.reasoner.calls,
                "{who}"
            );
            let about_incidents: u32 = v.per_incident.iter().map(|i| i.escalations).sum();
            assert_eq!(
                about_incidents + e.background,
                e.needed + e.unneeded + e.background
            );
            assert!(e.informed >= v.per_incident.iter().map(|i| i.informed_escalations).sum());
            assert!(e.correct >= v.per_incident.iter().map(|i| i.correct_escalations).sum());

            // Plain and hard incidents are correct or missed, never both and never neither (S9).
            assert_eq!(t.correct.plain + t.missed.plain, t.incidents.plain, "{who}");
            assert_eq!(t.correct.hard + t.missed.hard, t.incidents.hard, "{who}");
            assert!(t.decoys_silent <= t.incidents.decoy, "{who}");

            // The totals are sums of the per-incident rows.
            let wrong: u32 = v
                .per_incident
                .iter()
                .filter(|i| i.tier != gordian_stream::Tier::Decoy)
                .map(|i| i.wrong_declarations)
                .sum();
            assert_eq!(wrong, t.wrong_declarations, "{who}");

            match spec {
                StreamPolicySpec::Never => {
                    assert_eq!(t.reasoner.calls, 0, "{who}");
                    assert_eq!(t.reasoner.tokens, 0, "{who}");
                    assert_eq!(e.hard_incidents_escalated + e.other_incidents_escalated, 0);
                }
                StreamPolicySpec::Always => {
                    assert!(t.reasoner.calls > 0, "{who}");
                    assert!(
                        e.unneeded + e.background > 0,
                        "{who}: it escalates what is not hard"
                    );
                }
                StreamPolicySpec::Oracle => {
                    // Exactly the hard incidents, nothing else (the evaluator's reading of the
                    // trajectory, independent of the arm's own plan).
                    assert_eq!(u64::from(e.needed), t.reasoner.calls, "{who}");
                    assert_eq!(e.unneeded + e.background, 0, "{who}");
                    assert_eq!(e.other_incidents_escalated, 0, "{who}");
                }
                _ => {}
            }
        }
    }
    assert!(
        tiers_seen.iter().all(|c| *c > 0),
        "every tier was exercised: {tiers_seen:?}"
    );
}

// ---- The call record's focus

/// A short stream, one escalation at its end about the first incident's first observation, and
/// the evaluator's check of the call record.
#[test]
fn the_call_records_focus_is_the_traces_and_the_evaluator_checks_it_against_the_trajectory() {
    let p = hard_heavy(2);
    let stream = generate(&p);
    let truth = truth_from_stream(&stream);
    let end = Instant(truth.duration_ns);
    let first_incident = &truth.incidents[0];
    let focus = first_incident.observations[0];
    let mut sim = StreamSimulator::new(stream);
    sim.observe_until(end);
    let action = StreamAction::Escalate {
        context: vec![],
        question: Question::Diagnose { focus },
    };
    let outcome = sim.apply(action.clone(), end);
    assert!(
        matches!(outcome, StreamOutcome::Escalated { .. }),
        "{outcome:?}"
    );
    let steps = vec![StreamStep {
        at: end,
        action,
        outcome,
    }];

    // The bridge fills the focus from the stream's own record, and that agrees with the step.
    let calls = calls_from_sim(&sim);
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].focus,
        Some(focus),
        "the trace carries the escalation's focus"
    );
    assert!(score_stream(&truth, &steps, &calls).is_ok());

    // A record whose focus is some other observation is refused, and nothing else was changed:
    // the evaluator compares the focus with the trajectory's (S35).
    let other = ObsId(if focus.0 == 0 { 1 } else { 0 });
    assert_ne!(other, focus);
    let mut tampered = calls.clone();
    tampered[0].focus = Some(other);
    assert_eq!(
        score_stream(&truth, &steps, &tampered).unwrap_err(),
        StreamEvalError::CallRecordMismatch { index: 0, call: 0 }
    );
    // A source that does not know the focus is still accepted: the trajectory is the source then.
    let mut unknown = calls;
    unknown[0].focus = None;
    assert!(score_stream(&truth, &steps, &unknown).is_ok());
}

// ---- An evaluator error is a defect, not a row

#[test]
fn an_evaluator_error_is_a_harness_defect_and_the_harness_propagates_it() {
    // A trajectory with an accepted escalation and no call record is one no correct harness
    // could have recorded.
    let p = hard_heavy(3);
    let stream = generate(&p);
    let truth = truth_from_stream(&stream);
    let end = Instant(truth.duration_ns);
    let focus = truth.incidents[0].observations[0];
    let mut sim = StreamSimulator::new(stream);
    sim.observe_until(end);
    let action = StreamAction::Escalate {
        context: vec![],
        question: Question::Diagnose { focus },
    };
    let outcome = sim.apply(action.clone(), end);
    let steps = vec![StreamStep {
        at: end,
        action,
        outcome,
    }];
    let error = score_stream(&truth, &steps, &[]).unwrap_err();
    // It converts to the harness's error type, which a run reports and stops on.
    let defect = StreamHarnessError::from(error);
    assert!(matches!(defect, StreamHarnessError::Eval(_)), "{defect:?}");
    assert!(defect.to_string().contains("evaluator refused"), "{defect}");

    // And the harness has no path that turns the error into a row: the one call is propagated,
    // and neither the results writer nor the recorder mentions the scorer.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stream");
    let harness = fs::read_to_string(src.join("harness.rs")).unwrap();
    assert_eq!(harness.matches("score_stream(").count(), 1);
    assert!(harness.contains("score_stream(&truth, &st.trajectory, &calls)?;"));
    for file in ["results.rs", "recorder.rs", "mod.rs"] {
        let text = fs::read_to_string(src.join(file)).unwrap();
        assert!(!text.contains("score_stream("), "{file}");
    }
}

// ---- results.csv and incidents.csv

fn table(dir: &Path, name: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let text = read(dir, name);
    let mut it = rows(&text).into_iter();
    let header = it.next().unwrap();
    (header, it.collect())
}

fn col(header: &[String], name: &str) -> usize {
    header
        .iter()
        .position(|c| c == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

#[test]
fn incidents_csv_is_the_per_incident_side_of_results_csv_and_carries_the_family() {
    let mut m = all_arms("incidents", 3, 300, 0);
    m.stream_params.mix.plain_permille = 300;
    m.stream_params.mix.hard_permille = 500;
    m.limits = limits(&m.stream_params);
    let out = run(&m, "incidents");
    let families = ["compound", "cascade", "split_brain", "slow_leak"];

    // What the truth says about each seed's incidents, which no arm changes.
    let mut truth_by_seed: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let mut family_seen = BTreeMap::<String, u32>::new();
    for arm in &m.arms {
        let dir = out.join(&arm.arm);
        let (rh, results) = table(&dir, "results.csv");
        let (ih, incidents) = table(&dir, "incidents.csv");
        assert_eq!(rh.join(","), RESULTS_HEADER);
        assert_eq!(ih.join(","), INCIDENTS_HEADER);
        assert!(incidents.iter().all(|r| r.len() == ih.len()));
        assert!(
            !rh.iter().any(|c| c == "family"),
            "the family is not in results.csv"
        );
        let get = |row: &Vec<String>, h: &[String], name: &str| row[col(h, name)].clone();

        for row in &results {
            let seed = get(row, &rh, "seed");
            let mine: Vec<&Vec<String>> = incidents
                .iter()
                .filter(|r| get(r, &ih, "seed") == seed)
                .collect();
            let num = |name: &str| get(row, &rh, name).parse::<u64>().unwrap();
            // Ids are dense from zero, in order, and there is one row per incident of the stream.
            let count = num("incidents_plain") + num("incidents_hard") + num("incidents_decoy");
            assert_eq!(mine.len() as u64, count, "{} seed {seed}", arm.arm);
            for (k, r) in mine.iter().enumerate() {
                assert_eq!(get(r, &ih, "incident"), k.to_string());
                assert_eq!(get(r, &ih, "arm_role"), get(row, &rh, "arm_role"));
                assert_eq!(get(r, &ih, "run_id"), get(row, &rh, "run_id"));
            }

            // The family is a hard incident's, and only a hard incident's.
            for r in &mine {
                let tier = get(r, &ih, "tier");
                let family = get(r, &ih, "family");
                if tier == "hard" {
                    assert!(families.contains(&family.as_str()), "{family:?}");
                    *family_seen.entry(family).or_default() += 1;
                } else {
                    assert_eq!(family, "", "a {tier} incident has no family");
                }
            }

            // Every total of results.csv is the sum of the rows of incidents.csv it names.
            let sum = |tier: &[&str], name: &str, only: &dyn Fn(&Vec<String>) -> bool| -> u64 {
                mine.iter()
                    .filter(|r| tier.contains(&get(r, &ih, "tier").as_str()) && only(r))
                    .map(|r| get(r, &ih, name).parse::<u64>().unwrap_or(0))
                    .sum()
            };
            let flag = |name: &'static str| {
                let ih = ih.clone();
                move |r: &Vec<String>| r[col(&ih, name)] == "true"
            };
            let any = |_: &Vec<String>| true;
            for tier in ["plain", "hard"] {
                let tc = |name: &str| num(&format!("{name}_{tier}"));
                let count = |only: &dyn Fn(&Vec<String>) -> bool| {
                    mine.iter()
                        .filter(|r| get(r, &ih, "tier") == tier && only(r))
                        .count() as u64
                };
                assert_eq!(count(&any), num(&format!("incidents_{tier}")));
                assert_eq!(count(&flag("correct_by_deadline")), tc("correct"));
                assert_eq!(count(&flag("missed")), tc("missed"));
                assert_eq!(count(&flag("critical_miss")), tc("critical_missed"));
            }
            assert_eq!(
                sum(&["plain", "hard"], "wrong_declarations", &any),
                num("wrong_declarations")
            );
            let decoys = |only: &dyn Fn(&Vec<String>) -> bool| {
                mine.iter()
                    .filter(|r| get(r, &ih, "tier") == "decoy" && only(r))
                    .count() as u64
            };
            let positive = |name: &'static str| {
                let ih = ih.clone();
                move |r: &Vec<String>| r[col(&ih, name)].parse::<u64>().unwrap() > 0
            };
            assert_eq!(
                decoys(&positive("correct_declarations")),
                num("decoys_dismissed")
            );
            assert_eq!(
                decoys(&positive("wrong_declarations")),
                num("decoys_alarmed")
            );
            let silent = |r: &Vec<String>| {
                r[col(&ih, "correct_declarations")] == "0"
                    && r[col(&ih, "wrong_declarations")] == "0"
            };
            assert_eq!(decoys(&silent), num("decoys_silent"));
            assert_eq!(
                sum(&["hard"], "escalations", &any),
                num("escalations_needed")
            );
            assert_eq!(
                sum(&["plain", "decoy"], "escalations", &any),
                num("escalations_unneeded")
            );
            let escalated = |r: &Vec<String>| r[col(&ih, "escalations")] != "0";
            let hard_escalated = mine
                .iter()
                .filter(|r| get(r, &ih, "tier") == "hard" && escalated(r))
                .count() as u64;
            let other_escalated = mine
                .iter()
                .filter(|r| get(r, &ih, "tier") != "hard" && escalated(r))
                .count() as u64;
            assert_eq!(hard_escalated, num("hard_incidents_escalated"));
            assert_eq!(other_escalated, num("other_incidents_escalated"));
            // Calls about background are in no incident's row.
            let inc_informed: u64 = mine
                .iter()
                .map(|r| get(r, &ih, "informed_escalations").parse::<u64>().unwrap())
                .sum();
            assert!(inc_informed <= num("calls_informed"));
            // Totals against the segment's own cost columns.
            assert_eq!(
                num("reasoner_calls"),
                num("escalations_needed")
                    + num("escalations_unneeded")
                    + num("escalations_background")
            );
            assert_eq!(num("bill_comm"), num("reasoner_tokens"));
            assert_eq!(
                num("reasoner_modelled_ns"),
                num("reasoner_tokens") * 250_000
            );

            // The incidents and their families do not depend on the arm.
            let key: Vec<(String, String, String)> = mine
                .iter()
                .map(|r| {
                    (
                        get(r, &ih, "tier"),
                        get(r, &ih, "family"),
                        get(r, &ih, "critical"),
                    )
                })
                .collect();
            match truth_by_seed.get(&seed) {
                Some(first) => assert_eq!(first, &key, "{} seed {seed}", arm.arm),
                None => {
                    truth_by_seed.insert(seed, key);
                }
            }
        }
    }
    assert_eq!(truth_by_seed.len(), 3);
    assert!(
        family_seen.len() >= 2,
        "the test exercised {family_seen:?}; more than one family should appear"
    );

    // The family is evaluator output: it is in no other file of the run.
    for arm in &m.arms {
        let dir = out.join(&arm.arm);
        for name in ["results.csv", "measured.csv", "manifest.json"] {
            let text = read(&dir, name);
            for family in families {
                assert!(
                    !text.contains(family),
                    "{} {name} mentions {family}",
                    arm.arm
                );
            }
        }
    }
}

#[test]
fn incidents_csv_is_byte_identical_on_replay_and_the_privileged_arm_says_so() {
    let m = all_arms("incidents-replay", 2, 200, 3);
    let a = run(&m, "incidents-replay-a");
    let b = run(&m, "incidents-replay-b");
    for arm in &m.arms {
        for name in ["incidents.csv", "results.csv"] {
            assert_eq!(
                read(&a.join(&arm.arm), name),
                read(&b.join(&arm.arm), name),
                "{} {name}",
                arm.arm
            );
        }
    }
    let (ih, rows_p) = table(&a.join("oracle_escalation_privileged"), "incidents.csv");
    assert!(!rows_p.is_empty());
    assert!(
        rows_p
            .iter()
            .all(|r| r[col(&ih, "arm_role")] == "privileged")
    );
    let (_, rows_a) = table(&a.join("ablation_hidden_rules"), "incidents.csv");
    assert!(rows_a.iter().all(|r| r[col(&ih, "arm_role")] == "ablation"));
}
