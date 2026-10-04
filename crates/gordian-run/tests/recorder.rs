//! The recorder: manifests, result files, the events sample, protocol replay, the smoke run.

mod common;

use common::*;
use gordian_core::{ComponentId, Resource};
use gordian_run::manifest::{EpisodeParams, IsolationSpec, Manifest};
use gordian_run::policy::scripted::{ScriptedPolicy, ScriptedStep};
use gordian_run::policy::{Policy, PolicyId};
use gordian_run::recorder::{RunError, execute, execute_with, sampled};
use gordian_world::{Action, EpisodeClass, ProbeKind, ServiceId};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// A manifest with a fixed, fake environment, so that tests do not depend on git or rustc.
fn manifest(run_id: &str, policy: &str, seeds: u64, rate: f64) -> Manifest {
    Manifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arm: policy.to_owned(),
        source_revision: "0".repeat(40),
        lockfile_sha256: "0".repeat(64),
        toolchain: "rustc test".to_owned(),
        cpu_flags: vec!["avx2".to_owned()],
        isolation: IsolationSpec::default(),
        seeds: (0..seeds).collect(),
        episode_classes: EpisodeClass::ALL
            .iter()
            .map(|c| (*c, seeds as u32))
            .collect(),
        policy: PolicyId::new(policy),
        limits: limits(),
        episode_params: EpisodeParams::default(),
        trace_sample_rate: rate,
        internal_external_ratio: None,
    }
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

/// The plan's `results.csv` columns, in the plan's order (`docs/local-test-plan.md`, A4).
const PLAN_COLUMNS: [&str; 19] = [
    "run_id",
    "seed",
    "class",
    "success",
    "critical_miss",
    "false_alarm",
    "abstained",
    "undecided",
    "probes_used",
    "corrections",
    "decision_at_ns",
    "bill_compute",
    "bill_memory",
    "bill_time",
    "bill_probes",
    "bill_comm",
    "bill_storage",
    "components_run",
    "components_skipped",
];

/// A scripted policy that probes, corrects and declares, so that every kind of outcome line
/// reaches the events sample.
fn busy_policy() -> Box<dyn Policy> {
    Box::new(ScriptedPolicy::new(vec![
        ScriptedStep {
            select: vec![
                ComponentId(0),
                ComponentId(1),
                ComponentId(2),
                ComponentId(3),
            ],
            action: Some(Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: ServiceId(0),
            }),
        },
        ScriptedStep {
            select: vec![ComponentId(3)],
            action: Some(Action::Probe {
                kind: ProbeKind::ConfigSnapshot,
                target: ServiceId(1),
            }),
        },
        ScriptedStep {
            select: vec![],
            action: Some(Action::Correct { site: ServiceId(0) }),
        },
        ScriptedStep {
            select: vec![ComponentId(0)],
            action: Some(Action::Abstain),
        },
    ]))
}

fn busy(_: &PolicyId) -> Option<Box<dyn Policy>> {
    Some(busy_policy())
}

// ---- protocol replay ----

#[test]
fn the_same_manifest_run_twice_gives_byte_identical_results() {
    let m = manifest("replay", "heuristic_only", 6, 0.5);
    let (a, b) = (scratch("replay-a"), scratch("replay-b"));
    let sa = execute(&m, &a).unwrap();
    let sb = execute(&m, &b).unwrap();
    assert_eq!(sa, sb);
    assert_eq!(read(&a, "results.csv"), read(&b, "results.csv"));
    assert_eq!(read(&a, "manifest.json"), read(&b, "manifest.json"));
    assert_eq!(read(&a, "manifest.json"), m.canonical_json());

    // measured.csv has the same keys in the same order; its values are timings and may differ.
    let keys = |dir: &Path| -> Vec<(String, String, String)> {
        rows(&read(dir, "measured.csv"))
            .into_iter()
            .map(|r| (r[0].clone(), r[1].clone(), r[2].clone()))
            .collect()
    };
    assert_eq!(keys(&a), keys(&b));
    assert_eq!(keys(&a).len(), 66);

    // The events sample is the same episodes with the same entries; only timer payloads differ.
    let strip = |dir: &Path| -> Vec<Value> {
        read(dir, "events-sample.jsonl")
            .lines()
            .map(|l| {
                let mut v: Value = serde_json::from_str(l).unwrap();
                if v["producer"] == "harness/timer" {
                    v["payload"] = Value::Null;
                }
                v
            })
            .collect()
    };
    let (ea, eb) = (strip(&a), strip(&b));
    assert!(!ea.is_empty());
    assert_eq!(ea, eb);
}

#[test]
fn results_have_the_plans_columns_in_order_then_the_additions_one_row_per_episode() {
    let m = manifest("shape", "heuristic_only", 3, 0.0);
    let dir = scratch("shape");
    execute(&m, &dir).unwrap();
    let results = read(&dir, "results.csv");
    let header: Vec<&str> = results.lines().next().unwrap().split(',').collect();
    assert_eq!(&header[..19], &PLAN_COLUMNS);
    assert_eq!(&header[19..], &["directives_ignored", "stop_reason"]);
    let r = rows(&results);
    assert_eq!(r.len(), m.episodes().len());
    assert_eq!(r.len(), 33);
    assert!(r.iter().all(|row| row.len() == header.len()));
    // Row order is the manifest's execution order, keyed by (seed, class).
    let keys: Vec<(u64, String)> = r
        .iter()
        .map(|row| (row[1].parse().unwrap(), row[2].clone()))
        .collect();
    let expected: Vec<(u64, String)> = m
        .episodes()
        .iter()
        .map(|(s, c)| (*s, format!("{c:?}")))
        .collect();
    assert_eq!(keys, expected);
    // measured.csv: one row per results row, same keys, three timing columns.
    let measured = read(&dir, "measured.csv");
    assert_eq!(
        measured.lines().next().unwrap(),
        "run_id,seed,class,measured_component_ns,measured_sched_ns,measured_harness_ns"
    );
    let mr = rows(&measured);
    assert_eq!(mr.len(), r.len());
    for (a, b) in r.iter().zip(&mr) {
        assert_eq!(a[..3], b[..3]);
        assert!(b[3..].iter().all(|v| v.parse::<u64>().is_ok()));
    }
    // No events file when the sample rate is zero.
    assert!(!dir.join("events-sample.jsonl").exists());
}

#[test]
fn a_manifest_names_classes_in_order_and_runs_the_first_count_seeds_of_each() {
    let mut m = manifest("order", "heuristic_only", 5, 0.0);
    m.seeds = vec![40, 10, 30, 20, 50];
    m.episode_classes = vec![(EpisodeClass::NoFault, 2), (EpisodeClass::Ambiguous, 3)];
    assert_eq!(
        m.episodes(),
        vec![
            (40, EpisodeClass::NoFault),
            (10, EpisodeClass::NoFault),
            (40, EpisodeClass::Ambiguous),
            (10, EpisodeClass::Ambiguous),
            (30, EpisodeClass::Ambiguous),
        ]
    );
    let back: Manifest = serde_json::from_str(&m.canonical_json()).unwrap();
    assert_eq!(back, m);
}

// ---- the smoke run ----

#[test]
fn heuristic_only_completes_20_seeds_by_11_classes_and_every_episode_is_scored() {
    let m = manifest("smoke", "heuristic_only", 20, 0.0);
    let dir = scratch("smoke");
    let summary = execute(&m, &dir).unwrap();
    assert_eq!(summary.episodes, 220);
    let r = rows(&read(&dir, "results.csv"));
    assert_eq!(r.len(), 220);
    let header: Vec<String> = read(&dir, "results.csv")
        .lines()
        .next()
        .unwrap()
        .split(',')
        .map(str::to_owned)
        .collect();
    let col = |name: &str| header.iter().position(|h| h == name).unwrap();
    let mut per_class: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let (mut compute, mut time) = (0u64, 0u64);
    for row in &r {
        for name in [
            "success",
            "critical_miss",
            "false_alarm",
            "abstained",
            "undecided",
        ] {
            assert!(
                matches!(row[col(name)].as_str(), "true" | "false"),
                "{name} in {row:?}"
            );
        }
        // Every episode is scored: a decided episode has a decision time, an undecided one has
        // none, and the two agree.
        let undecided = row[col("undecided")] == "true";
        assert_eq!(row[col("decision_at_ns")].is_empty(), undecided, "{row:?}");
        assert!(!row[col("stop_reason")].is_empty());
        let e = per_class.entry(row[2].clone()).or_default();
        e.0 += 1;
        e.1 += u32::from(row[col("success")] == "true");
        compute += row[col("bill_compute")].parse::<u64>().unwrap();
        time += row[col("bill_time")].parse::<u64>().unwrap();
    }
    assert_eq!(per_class.len(), 11);
    assert!(per_class.values().all(|(n, _)| *n == 20));
    // Printed for the report; run with --nocapture to see it.
    for (class, (n, ok)) in &per_class {
        eprintln!("heuristic_only {class}: {ok}/{n}");
    }
    eprintln!("heuristic_only mean bill_compute {}", compute / 220);
    assert!(compute > 0, "the heuristic was charged");
    assert_eq!(time, 0, "heuristic_only uses no probe and no declared time");
    // A smoke test of the instrument, not of the policy: some episodes are solved, some not.
    let ok: u32 = per_class.values().map(|(_, ok)| ok).sum();
    assert!(ok > 0 && ok < 220, "{ok}");
}

// ---- the events sample ----

/// Names that appear in hidden simulator state, in the harness directives, or in the evaluator's
/// truth: the fields of `Hidden`, `Fault`, `ComponentDirective`, `StreamLabel`, and the labels.
/// Quoted, so that `"onset"` matches a key and not a word inside a class name.
const HIDDEN_KEYS: [&str; 17] = [
    "faults",
    "bits",
    "drift_hash",
    "labels",
    "onset",
    "critical",
    "directives",
    "hidden",
    "mode",
    "factor",
    "Fail",
    "Slow",
    "Signal",
    "Bait",
    "Noise",
    "group",
    "truth",
];

fn all_keys(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                into.insert(k.clone());
                all_keys(v, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| all_keys(v, into)),
        _ => {}
    }
}

#[test]
fn the_events_sample_holds_no_hidden_state() {
    // Every class, a policy that exercises probes and corrections, and a policy that runs
    // components, all sampled in full.
    let m = manifest("leakcheck", "busy", 4, 1.0);
    let busy_dir = scratch("hidden-busy");
    execute_with(&m, &busy_dir, &busy).unwrap();
    let heur = manifest("leakcheck-h", "heuristic_only", 4, 1.0);
    let heur_dir = scratch("hidden-heuristic");
    execute(&heur, &heur_dir).unwrap();

    // The keys the file may contain: the recorder's own, the public information, observations,
    // actions, outcomes, accounting, the components' hypothesis payloads, and the timer's.
    let allowed: BTreeSet<&str> = [
        // recorder
        "record",
        "run_id",
        "seed",
        "class",
        "public_info",
        "observations",
        "at_ns",
        "observation",
        "id",
        "kind",
        "producer",
        "producer_version",
        "inputs",
        "payload",
        // public information
        "services",
        "prior_records",
        "horizon",
        "budget",
        "probes",
        "time_ns",
        "depends_on",
        "resource",
        "config_hash",
        "unreliable_health",
        "signature",
        "resolution",
        // observations, actions, outcomes
        "Counter",
        "Message",
        "Snapshot",
        "Probed",
        "Correction",
        "Probe",
        "Correct",
        "Declare",
        "Abstain",
        "service",
        "name",
        "value",
        "text_id",
        "severity",
        "config_hash",
        "target",
        "probe",
        "result",
        "site",
        "resolved",
        "fault",
        "ready_at",
        "cost",
        "Refused",
        "Declared",
        "Abstained",
        "Corrected",
        "Inconclusive",
        "suggest",
        "ConfigHash",
        "Positive",
        "Negative",
        "BudgetExceeded",
        "UnknownService",
        "EpisodeOver",
        "PastHorizon",
        "TimeWentBackwards",
        "Text",
        // accounting
        "phase",
        "accepted",
        "charges",
        "resource",
        "amount",
        "component",
        "hex",
        // components and the timer
        "source",
        "basis",
        "ranked",
        "hypothesis",
        "score",
        "tied_at_top",
        "window",
        "output",
        "reason",
        "what",
        "ns",
        "refused",
        "Candidates",
        "EvidenceDamaged",
    ]
    .into_iter()
    .collect();

    for dir in [&busy_dir, &heur_dir] {
        let text = read(dir, "events-sample.jsonl");
        assert!(text.lines().count() > 100, "the sample is not empty");
        for name in HIDDEN_KEYS {
            let quoted = format!("\"{name}\"");
            assert!(
                !text.contains(&quoted),
                "{name} appears in {}",
                dir.display()
            );
        }
        assert!(
            !text.contains("drift"),
            "drift appears in {}",
            dir.display()
        );
        let mut keys = BTreeSet::new();
        for line in text.lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            all_keys(&v, &mut keys);
        }
        let unexpected: Vec<_> = keys
            .iter()
            .filter(|k| !allowed.contains(k.as_str()))
            .collect();
        assert!(
            unexpected.is_empty(),
            "keys outside the allowlist: {unexpected:?}"
        );
    }
    // The busy policy's file does contain probe results and correction effects, so the allowlist
    // above was tested against them.
    let text = read(&busy_dir, "events-sample.jsonl");
    assert!(text.contains("\"Probed\"") && text.contains("\"Correction\""));
    assert!(text.contains("\"Corrected\""));
}

#[test]
fn the_sample_is_chosen_by_seed_and_class_alone() {
    // Same episodes whatever the policy.
    let sampled_set = |dir: &Path| -> BTreeSet<(u64, String)> {
        read(dir, "events-sample.jsonl")
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|v| v["record"] == "episode")
            .map(|v| {
                (
                    v["seed"].as_u64().unwrap(),
                    v["class"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let (a, b) = (scratch("sample-a"), scratch("sample-b"));
    execute(&manifest("sa", "heuristic_only", 30, 0.25), &a).unwrap();
    execute_with(&manifest("sb", "busy", 30, 0.25), &b, &busy).unwrap();
    let (sa, sb) = (sampled_set(&a), sampled_set(&b));
    assert!(!sa.is_empty());
    assert_eq!(sa, sb);
    // And it is the function `sampled` that chose them.
    for (seed, class) in EpisodeClass::ALL
        .iter()
        .flat_map(|c| (0..30u64).map(move |s| (s, *c)))
    {
        assert_eq!(
            sa.contains(&(seed, format!("{class:?}"))),
            sampled(seed, class, 0.25),
            "{seed} {class:?}"
        );
    }
}

#[test]
fn sampling_keeps_none_at_zero_all_at_one_and_about_the_rate_between() {
    let pairs: Vec<(u64, EpisodeClass)> = EpisodeClass::ALL
        .iter()
        .flat_map(|c| (0..1000u64).map(move |s| (s, *c)))
        .collect();
    assert!(pairs.iter().all(|(s, c)| !sampled(*s, *c, 0.0)));
    assert!(pairs.iter().all(|(s, c)| sampled(*s, *c, 1.0)));
    let kept = pairs.iter().filter(|(s, c)| sampled(*s, *c, 0.1)).count();
    let fraction = kept as f64 / pairs.len() as f64;
    assert!((0.08..0.12).contains(&fraction), "{fraction}");
    // Deterministic.
    let again = pairs.iter().filter(|(s, c)| sampled(*s, *c, 0.1)).count();
    assert_eq!(kept, again);
}

#[test]
fn the_events_sample_ledger_lines_replay_to_the_results_bill() {
    // The accounting in a sampled ledger, read back from the file, sums to the bill columns of
    // the row with the same key: the file and the table describe the same episode.
    let m = manifest("sum", "busy", 3, 1.0);
    let dir = scratch("sum");
    execute_with(&m, &dir, &busy).unwrap();
    let results = read(&dir, "results.csv");
    let header: Vec<&str> = results.lines().next().unwrap().split(',').collect();
    let col = |n: &str| header.iter().position(|h| *h == n).unwrap();
    let mut spent: BTreeMap<(u64, String, String), u64> = BTreeMap::new();
    for line in read(&dir, "events-sample.jsonl").lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        if v["record"] == "entry" && v["kind"] == "Accounting" && v["payload"]["accepted"] == true {
            for c in v["payload"]["charges"].as_array().unwrap() {
                *spent
                    .entry((
                        v["seed"].as_u64().unwrap(),
                        v["class"].as_str().unwrap().to_owned(),
                        c["resource"].as_str().unwrap().to_owned(),
                    ))
                    .or_default() += c["amount"].as_u64().unwrap();
            }
        }
    }
    for row in rows(&results) {
        let key = |r: &str| (row[1].parse::<u64>().unwrap(), row[2].clone(), r.to_owned());
        for (resource, name) in [
            (Resource::Compute, "bill_compute"),
            (Resource::Time, "bill_time"),
            (Resource::Probes, "bill_probes"),
        ] {
            let from_file = spent
                .get(&key(&format!("{resource:?}")))
                .copied()
                .unwrap_or(0);
            assert_eq!(from_file.to_string(), row[col(name)], "{row:?} {name}");
        }
    }
}

#[test]
fn measured_columns_are_the_sums_of_the_timer_entries_in_the_sample() {
    let m = manifest("timers", "heuristic_only", 3, 1.0);
    let dir = scratch("timers");
    execute(&m, &dir).unwrap();
    let mut component: BTreeMap<(u64, String), u64> = BTreeMap::new();
    let mut sched: BTreeMap<(u64, String), u64> = BTreeMap::new();
    for line in read(&dir, "events-sample.jsonl").lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        if v["record"] == "entry" && v["producer"] == "harness/timer" {
            let key = (
                v["seed"].as_u64().unwrap(),
                v["class"].as_str().unwrap().to_owned(),
            );
            let ns = v["payload"]["ns"].as_u64().unwrap();
            match v["payload"]["what"].as_str().unwrap() {
                "component" => *component.entry(key).or_default() += ns,
                "select" | "decide" => *sched.entry(key).or_default() += ns,
                other => panic!("unknown timer entry {other}"),
            }
        }
    }
    let measured = rows(&read(&dir, "measured.csv"));
    assert_eq!(measured.len(), 33);
    for row in &measured {
        let key = (row[1].parse::<u64>().unwrap(), row[2].clone());
        assert_eq!(row[3].parse::<u64>().unwrap(), component[&key], "{row:?}");
        assert_eq!(row[4].parse::<u64>().unwrap(), sched[&key], "{row:?}");
        // What is left of the episode's wall time is the harness's share.
        assert!(row[5].parse::<u64>().unwrap() > 0, "{row:?}");
    }
}

// ---- failure modes ----

#[test]
fn a_recorded_run_is_never_overwritten() {
    let m = manifest("again", "heuristic_only", 2, 0.0);
    let dir = scratch("again");
    execute(&m, &dir).unwrap();
    let err = execute(&m, &dir).unwrap_err();
    assert!(matches!(err, RunError::Io(_)), "{err}");
    // A different manifest into a directory holding another manifest.json is refused too.
    let other = scratch("other");
    fs::write(
        other.join("manifest.json"),
        manifest("x", "heuristic_only", 3, 0.0).canonical_json(),
    )
    .unwrap();
    let err = execute(&m, &other).unwrap_err();
    assert!(matches!(err, RunError::Io(_)), "{err}");
    assert!(!other.join("results.csv").exists());
}

#[test]
fn an_existing_identical_manifest_json_is_accepted() {
    let m = manifest("pre", "heuristic_only", 2, 0.0);
    let dir = scratch("pre");
    fs::write(dir.join("manifest.json"), m.canonical_json()).unwrap();
    execute(&m, &dir).unwrap();
}

#[test]
fn unusable_manifests_and_policies_are_refused_before_anything_runs() {
    let dir = scratch("refused");
    let mut unknown = manifest("u", "no_such_policy", 2, 0.0);
    assert!(matches!(
        execute(&unknown, &dir).unwrap_err(),
        RunError::UnknownPolicy(_)
    ));
    unknown.policy = PolicyId::new("heuristic_only");
    let bad = |edit: &dyn Fn(&mut Manifest)| {
        let mut m = manifest("b", "heuristic_only", 3, 0.0);
        edit(&mut m);
        assert!(m.validate().is_err());
        assert!(matches!(
            execute(&m, &scratch("refused-bad")).unwrap_err(),
            RunError::Manifest(_)
        ));
    };
    bad(&|m| m.run_id = "has/slash".to_owned());
    bad(&|m| m.run_id = String::new());
    bad(&|m| m.seeds = vec![1, 1, 2]);
    bad(&|m| m.seeds.clear());
    bad(&|m| m.episode_classes = vec![(EpisodeClass::NoFault, 4)]);
    bad(&|m| m.episode_classes = vec![(EpisodeClass::NoFault, 0)]);
    bad(&|m| m.episode_classes = vec![(EpisodeClass::NoFault, 1), (EpisodeClass::NoFault, 2)]);
    bad(&|m| m.trace_sample_rate = 1.5);
    bad(&|m| m.trace_sample_rate = f64::NAN);
    bad(&|m| m.limits.step_ns = 0);
    bad(&|m| m.isolation.cpus = "0-2; rm".to_owned());
    bad(&|m| m.isolation.timeout_secs = 0);
}

#[test]
fn a_harness_defect_aborts_the_run_and_writes_no_results() {
    let m = manifest("defect", "bad", 2, 0.0);
    let dir = scratch("defect");
    let bad = |_: &PolicyId| -> Option<Box<dyn Policy>> {
        Some(Box::new(ScriptedPolicy::new(vec![ScriptedStep {
            select: vec![ComponentId(77)],
            action: None,
        }])))
    };
    let err = execute_with(&m, &dir, &bad).unwrap_err();
    assert!(matches!(err, RunError::Harness { .. }), "{err}");
    assert!(!dir.join("results.csv").exists());
    assert!(!dir.join("measured.csv").exists());
}

#[test]
fn undecided_episodes_stay_in_the_table_with_no_decision_time() {
    let mut m = manifest("undecided", "idle", 3, 0.0);
    m.limits.max_steps = 4;
    let dir = scratch("undecided");
    let idle = |_: &PolicyId| -> Option<Box<dyn Policy>> {
        Some(Box::new(ScriptedPolicy::never_decides()))
    };
    let summary = execute_with(&m, &dir, &idle).unwrap();
    assert_eq!(
        (summary.episodes, summary.undecided, summary.successes),
        (33, 33, 0)
    );
    let results = read(&dir, "results.csv");
    let header: Vec<&str> = results.lines().next().unwrap().split(',').collect();
    let col = |n: &str| header.iter().position(|h| *h == n).unwrap();
    for row in rows(&results) {
        assert_eq!(row[col("undecided")], "true");
        assert_eq!(row[col("success")], "false");
        assert_eq!(row[col("decision_at_ns")], "");
        assert_eq!(row[col("stop_reason")], "step_cap");
    }
    // NoFault is undecided too: not a success, and not a false alarm either.
    let nofault: Vec<_> = rows(&results)
        .into_iter()
        .filter(|r| r[2] == "NoFault")
        .collect();
    assert_eq!(nofault.len(), 3);
    assert!(nofault.iter().all(|r| r[col("false_alarm")] == "false"));
    // Critical classes count as critical misses.
    assert!(
        rows(&results)
            .iter()
            .filter(|r| r[2] == "CriticalFault" || r[2] == "QuietUrgent")
            .all(|r| r[col("critical_miss")] == "true")
    );
}

// ---- the binary ----

#[test]
fn the_binary_initializes_a_manifest_and_runs_it() {
    let bin = env!("CARGO_BIN_EXE_gordian-run");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = scratch("binary");
    let manifest_path = dir.join("m.json");
    let status = std::process::Command::new(bin)
        .current_dir(&root)
        .args(["init", "--run-id", "bin-test", "--policy", "heuristic_only"])
        .args([
            "--seed-start",
            "100",
            "--seed-count",
            "2",
            "--trace-sample-rate",
            "0",
        ])
        .arg("--out")
        .arg(&manifest_path)
        .status()
        .unwrap();
    assert!(status.success());
    let m: Manifest = serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let head = std::process::Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        m.source_revision,
        String::from_utf8_lossy(&head.stdout).trim()
    );
    assert_eq!(m.lockfile_sha256.len(), 64);
    assert!(m.toolchain.starts_with("rustc "));
    assert_eq!(m.seeds, vec![100, 101]);
    assert_eq!(m.episode_classes.len(), 11);

    let run_dir = dir.join("run");
    let status = std::process::Command::new(bin)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--out")
        .arg(&run_dir)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(rows(&read(&run_dir, "results.csv")).len(), 22);
    assert!(run_dir.join("measured.csv").exists());
    assert!(run_dir.join("manifest.json").exists());

    // A usage error exits 2, and a missing manifest exits 1.
    let usage = std::process::Command::new(bin)
        .arg("--bogus")
        .status()
        .unwrap();
    assert_eq!(usage.code(), Some(2));
    let missing = std::process::Command::new(bin)
        .args(["--manifest", "/nonexistent/m.json", "--out"])
        .arg(dir.join("never"))
        .status()
        .unwrap();
    assert_eq!(missing.code(), Some(1));
}
