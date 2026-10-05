//! Running a stream manifest: protocol replay per arm, interleaving, the files, the columns, the
//! labels that say what an arm is, the events sample and the command line.

mod stream_common;

use gordian_run::stream::manifest::{StreamArmSpec, StreamManifest};
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_run::stream::{StreamRunError, execute_stream};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use stream_common::*;

fn run(m: &StreamManifest, name: &str) -> std::path::PathBuf {
    let out = scratch(name);
    let out = out.join("run");
    execute_stream(m, &out).unwrap_or_else(|e| panic!("{e}"));
    out
}

#[test]
fn each_arms_results_are_byte_identical_on_replay() {
    let m = all_arms("replay", 3, 200, 1);
    let a = run(&m, "replay-a");
    let b = run(&m, "replay-b");
    for arm in &m.arms {
        let (ra, rb) = (
            read(&a.join(&arm.arm), "results.csv"),
            read(&b.join(&arm.arm), "results.csv"),
        );
        assert_eq!(ra, rb, "{} differs on replay", arm.arm);
        assert_eq!(ra.lines().count(), 1 + 3, "one row per segment");
    }
    // The manifest is written canonically, and the measurements are the only thing that varies.
    assert_eq!(read(&a, "manifest.json"), read(&b, "manifest.json"));
    assert_eq!(read(&a, "manifest.json"), m.canonical_json());
}

#[test]
fn an_interleaved_run_gives_each_arm_what_a_run_of_that_arm_alone_gives() {
    let m = all_arms("inter", 3, 200, 7);
    let together = run(&m, "inter-together");
    for (index, arm) in m.arms.iter().enumerate() {
        let single = m.single_arm(index);
        let alone = run(&single, &format!("inter-alone-{index}"));
        assert_eq!(
            read(&together.join(&arm.arm), "results.csv"),
            read(&alone.join(&arm.arm), "results.csv"),
            "{} differs between an interleaved run and a run alone",
            arm.arm
        );
        // The arm's own manifest is the one-arm manifest.
        let own: StreamManifest =
            serde_json::from_str(&read(&together.join(&arm.arm), "manifest.json")).unwrap();
        assert_eq!(own.arms.len(), 1);
        assert_eq!(own.arms[0], *arm);
    }
}

#[test]
fn results_do_not_depend_on_the_order_the_arms_play_in_or_on_the_drift_workload() {
    let base = all_arms("order", 3, 200, 1);
    let reference = run(&base, "order-ref");
    let mut other = base.clone();
    other.run_seed = 987_654;
    other.drift_block = 1;
    let moved = run(&other, "order-moved");
    for arm in &base.arms {
        assert_eq!(
            read(&reference.join(&arm.arm), "results.csv"),
            read(&moved.join(&arm.arm), "results.csv"),
            "{}",
            arm.arm
        );
    }
    // The order itself did change: some arm played first in a different set of segments.
    let positions = |dir: &Path, arm: &str| -> Vec<String> {
        (0..3)
            .map(|i| cell(&read(&dir.join(arm), "measured.csv"), i, "arm_position"))
            .collect()
    };
    let moved_somewhere = base
        .arms
        .iter()
        .any(|a| positions(&reference, &a.arm) != positions(&moved, &a.arm));
    assert!(
        moved_somewhere,
        "the run seed did not change any arm's position"
    );
    // And the drift workload ran more often.
    let blocks = |dir: &Path| read(dir, "drift.csv").lines().count();
    assert!(blocks(&moved) > blocks(&reference));
}

#[test]
fn the_files_and_columns_are_what_the_documentation_says() {
    let m = all_arms("cols", 2, 200, 0);
    let out = run(&m, "cols");
    for name in ["manifest.json", "drift.csv"] {
        assert!(out.join(name).exists(), "{name}");
    }
    let header = gordian_run::stream::results::results_header();
    let columns: Vec<&str> = header.split(',').collect();
    for (index, arm) in m.arms.iter().enumerate() {
        let dir = out.join(&arm.arm);
        for name in [
            "manifest.json",
            "results.csv",
            "measured.csv",
            "events-sample.jsonl",
        ] {
            assert!(
                dir.join(name).exists() || name == "events-sample.jsonl",
                "{name}"
            );
        }
        let results = read(&dir, "results.csv");
        let table = rows(&results);
        assert_eq!(table[0], columns);
        assert!(table.iter().all(|r| r.len() == columns.len()));
        assert_eq!(table.len(), 3);
        for (seed, row) in table.iter().skip(1).enumerate() {
            assert_eq!(row[0], format!("cols.{}", arm.arm));
            assert_eq!(row[1], m.role_of(index).as_str());
            assert_eq!(row[2], seed.to_string());
        }
        // Total cost is the substrate plus the reasoner's, converted, and the reasoner's tokens are
        // the bill's communication.
        for r in 0..2 {
            let get = |c: &str| cell(&results, r, c).parse::<u64>().unwrap();
            assert_eq!(
                get("total_cost_ns"),
                get("substrate_ns") + get("reasoner_cost_ns")
            );
            assert_eq!(
                get("substrate_ns"),
                get("modelled_component_ns") + get("modelled_sched_ns")
            );
            assert_eq!(get("bill_comm"), get("reasoner_tokens"));
            assert_eq!(get("reasoner_cost_ns"), get("reasoner_tokens") * 250_000);
            assert_eq!(
                get("declarations"),
                get("cheap_declarations") + get("reasoner_declarations")
            );
            assert_eq!(
                get("declarations"),
                get("declared_incident") + get("declared_dismissal")
            );
        }
        let measured = read(&dir, "measured.csv");
        let mtable = rows(&measured);
        assert_eq!(
            mtable[0],
            [
                "run_id",
                "seed",
                "measured_component_ns",
                "measured_sched_ns",
                "measured_harness_ns",
                "arm_position"
            ]
        );
        assert_eq!(
            mtable.len(),
            table.len(),
            "one measured row per results row"
        );
    }
    // Arm positions are a permutation of the arms in every segment.
    for seed in 0..2usize {
        let positions: BTreeSet<String> = m
            .arms
            .iter()
            .map(|a| {
                cell(
                    &read(&out.join(&a.arm), "measured.csv"),
                    seed,
                    "arm_position",
                )
            })
            .collect();
        assert_eq!(positions.len(), m.arms.len());
    }
}

#[test]
fn the_privileged_and_ablation_arms_say_so_on_every_row_and_in_their_names() {
    let m = all_arms("roles", 2, 200, 0);
    let out = run(&m, "roles");
    let role_of = |arm: &str| cell(&read(&out.join(arm), "results.csv"), 0, "arm_role");
    assert_eq!(role_of("oracle_escalation_privileged"), "privileged");
    assert_eq!(role_of("ablation_hidden_rules"), "ablation");
    assert_eq!(role_of("never_escalate"), "comparison");
    assert_eq!(
        m.comparison_arms(),
        vec![
            "never_escalate",
            "always_escalate",
            "periodic_escalation",
            "change_triggered",
            "threshold_score",
            "random_escalation"
        ]
    );

    // The manifest refuses an arm that hides what it is.
    let bad = |name: &str, policy: &str| {
        let mut m = m.clone();
        m.arms = vec![StreamArmSpec {
            arm: name.to_owned(),
            policy: StreamPolicySpec::from_id(policy).unwrap(),
        }];
        m.validate()
    };
    assert!(
        bad("oracle_escalation", "oracle_escalation")
            .unwrap_err()
            .contains("privileged")
    );
    assert!(
        bad("hidden_rules", "ablation_hidden_rules")
            .unwrap_err()
            .contains("ablation")
    );
    assert!(bad("oracle_escalation_privileged", "oracle_escalation").is_ok());
    assert!(bad("ablation_hidden_rules", "ablation_hidden_rules").is_ok());
    // A comparison arm may not borrow the labels either.
    assert!(bad("never_privileged", "never_escalate").is_err());
    assert!(bad("never_ablation", "never_escalate").is_err());
    // Names are unique, safe and not the files of the run directory.
    let mut dup = m.clone();
    dup.arms[1].arm = dup.arms[0].arm.clone();
    assert!(dup.validate().unwrap_err().contains("twice"));
    let mut reserved = m.clone();
    reserved.arms[0].arm = "drift.csv".to_owned();
    assert!(reserved.validate().is_err());
    let mut slash = m.clone();
    slash.arms[0].arm = "a/b".to_owned();
    assert!(slash.validate().is_err());
}

#[test]
fn a_manifest_names_the_stream_the_limits_and_the_exchange_and_validates_them() {
    let m = all_arms("manifest", 2, 200, 0);
    // Round trip, and the key the binary uses to tell a stream manifest from an episode one.
    let text = m.canonical_json();
    let back: StreamManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    let value: Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("stream_params").is_some());
    assert!(value.get("exchange").is_some());
    assert!(value.get("limits").is_some());
    assert!(value.get("episode_classes").is_none());

    let mut bad = m.clone();
    bad.limits.probes += 1;
    assert!(bad.validate().unwrap_err().contains("budget"));
    let mut bad = m.clone();
    bad.seeds = vec![1, 1];
    assert!(bad.validate().unwrap_err().contains("duplicate"));
    let mut bad = m.clone();
    bad.drift_block = 0;
    assert!(bad.validate().is_err());
    let mut bad = m.clone();
    bad.rung.window = 0;
    assert!(bad.validate().is_err());
    let mut bad = m.clone();
    bad.trace_sample_rate = 2.0;
    assert!(bad.validate().is_err());
    assert!(m.validate().is_ok());
}

#[test]
fn a_policy_is_written_as_its_id_or_with_its_parameters_and_stray_parameters_are_errors() {
    let from = |text: &str| serde_json::from_str::<StreamPolicySpec>(text);
    assert_eq!(from("\"never_escalate\"").unwrap(), StreamPolicySpec::Never);
    let periodic = from(r#"{"policy": "periodic_escalation", "period_ns": 5000000000}"#).unwrap();
    assert_eq!(
        periodic,
        StreamPolicySpec::Periodic {
            period_ns: 5_000_000_000
        }
    );
    // Written back with every parameter, so a manifest records what ran.
    assert_eq!(
        serde_json::to_string(&StreamPolicySpec::from_id("threshold_score").unwrap()).unwrap(),
        r#"{"policy":"threshold_score","tau":3.5,"wait_ns":6000000000}"#
    );
    // The delay after notice: its default is zero and a zero is not written, so a manifest written
    // before the parameter existed is the text a manifest of the same arm has now.
    let delayed = from(r#"{"policy": "always_escalate", "delay_ns": 8000000000}"#).unwrap();
    assert_eq!(
        delayed,
        StreamPolicySpec::Always {
            delay_ns: 8_000_000_000
        }
    );
    assert_eq!(
        serde_json::to_string(&delayed).unwrap(),
        r#"{"policy":"always_escalate","delay_ns":8000000000}"#
    );
    assert_eq!(
        from(r#"{"policy": "random_escalation", "p": 0.3, "delay_ns": 4000000000}"#).unwrap(),
        StreamPolicySpec::Random {
            p: 0.3,
            delay_ns: 4_000_000_000
        }
    );
    assert_eq!(
        from(r#"{"policy": "random_escalation", "p": 0.3}"#).unwrap(),
        StreamPolicySpec::Random {
            p: 0.3,
            delay_ns: 0
        }
    );
    assert_eq!(
        serde_json::to_string(&StreamPolicySpec::Random {
            p: 0.3,
            delay_ns: 0
        })
        .unwrap(),
        r#"{"policy":"random_escalation","p":0.3}"#
    );
    assert_eq!(
        serde_json::to_string(&StreamPolicySpec::Always { delay_ns: 0 }).unwrap(),
        "\"always_escalate\""
    );
    for stray in [
        "periodic_escalation",
        "threshold_score",
        "never_escalate",
        "change_triggered",
    ] {
        assert!(
            from(&format!(r#"{{"policy": "{stray}", "delay_ns": 1}}"#)).is_err(),
            "{stray} has no delay"
        );
    }
    assert!(from(r#"{"policy": "never_escalate", "p": 0.5}"#).is_err());
    assert!(from(r#"{"policy": "random_escalation", "p": 1.5}"#).is_err());
    assert!(from(r#"{"policy": "periodic_escalation", "period_ns": 0}"#).is_err());
    assert!(from(r#"{"policy": "random_escalation", "q": 0.5}"#).is_err());
    assert!(from("\"no_such_arm\"").is_err());
    for id in ALL_ARMS {
        let spec = StreamPolicySpec::from_id(id).unwrap();
        let back: StreamPolicySpec =
            serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(back, spec);
    }
}

#[test]
fn a_recorded_run_is_never_overwritten() {
    let m = manifest("keep", &[("never_escalate", "never_escalate")], 1, 150, 0);
    let out = run(&m, "keep");
    match execute_stream(&m, &out) {
        Err(StreamRunError::Io(why)) => assert!(why.contains("already exists"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_events_sample_holds_the_public_record_and_no_hidden_state() {
    let mut m = manifest(
        "events",
        &[
            ("always_escalate", "always_escalate"),
            ("oracle_escalation_privileged", "oracle_escalation"),
        ],
        1,
        200,
        0,
    );
    m.trace_sample_rate = 1.0;
    let out = run(&m, "events");
    // Names of the hidden side's fields: the tier, criticality and deadline of an incident, the
    // decisive-evidence labels, the reasoner's accuracy draws, the stream's own parameters.
    let hidden = [
        "tier",
        "critical",
        "deadline",
        "deadline_ns",
        "difficulty",
        "decisive",
        "informed",
        "correct",
        "fingerprint",
        "p0",
        "mix",
        "regimes",
        "recurrence_permille",
        "plain_permille",
        "hard_permille",
        "StreamLabel",
        "Decisive",
        "occupies",
        // The evaluator's output: the hard-fault family (`incidents.csv`) and its names.
        "family",
        "compound",
        "cascade",
        "split_brain",
        "slow_leak",
    ];
    for arm in &m.arms {
        let text = read(&out.join(&arm.arm), "events-sample.jsonl");
        let mut records = BTreeMap::<String, usize>::new();
        let mut keys = BTreeSet::new();
        fn walk(value: &Value, keys: &mut BTreeSet<String>) {
            match value {
                Value::Object(map) => {
                    for (k, v) in map {
                        keys.insert(k.clone());
                        walk(v, keys);
                    }
                }
                Value::Array(items) => items.iter().for_each(|v| walk(v, keys)),
                _ => {}
            }
        }
        for line in text.lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            *records
                .entry(v["record"].as_str().unwrap().to_owned())
                .or_default() += 1;
            walk(&v, &mut keys);
        }
        assert_eq!(records["segment"], 1);
        assert_eq!(records["public_stream"], 1);
        assert!(records["entry"] > 100);
        for name in hidden {
            assert!(
                !keys.contains(name),
                "{} sample has the key {name}",
                arm.arm
            );
            assert!(
                !text.contains(&format!("\"{name}\"")),
                "{} sample has {name}",
                arm.arm
            );
        }
        // The public information is the six documented fields.
        let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        let info = first["public_info"].as_object().unwrap();
        let fields: BTreeSet<&str> = info.keys().map(String::as_str).collect();
        assert_eq!(
            fields,
            BTreeSet::from([
                "services",
                "duration_ns",
                "deadlines",
                "reasoner_cost",
                "max_context",
                "budget"
            ])
        );
    }
    // Every arm samples the same segments.
    let cheap = read(&out.join("always_escalate"), "events-sample.jsonl");
    let privileged = read(
        &out.join("oracle_escalation_privileged"),
        "events-sample.jsonl",
    );
    assert_eq!(
        cheap
            .lines()
            .next()
            .map(|l| l.replace("always_escalate", "x")),
        privileged
            .lines()
            .next()
            .map(|l| l.replace("oracle_escalation_privileged", "x"))
    );
}

#[test]
fn the_command_line_writes_and_runs_a_stream_manifest() {
    let bin = env!("CARGO_BIN_EXE_gordian-run");
    // `init-stream` captures the environment of the current directory: the repository root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = scratch("cli");
    let manifest_path = dir.join("m.json");
    let status = Command::new(bin)
        .current_dir(&root)
        .args([
            "init-stream",
            "--run-id",
            "cli-test",
            "--arms",
            "n=never_escalate,a=always_escalate,oracle_escalation,h=ablation_hidden_rules",
            "--seed-start",
            "0",
            "--seed-count",
            "2",
            "--duration-ns",
            "150000000000",
            "--trace-sample-rate",
            "0",
            "--out",
        ])
        .arg(&manifest_path)
        .status()
        .unwrap();
    // `h` lacks the word `ablation`: the manifest refuses it, and nothing is written.
    assert!(!status.success());
    assert!(!manifest_path.exists());
    let status = Command::new(bin)
        .current_dir(&root)
        .args([
            "init-stream",
            "--run-id",
            "cli-test",
            "--arms",
            "n=never_escalate,a=always_escalate,oracle_escalation,ablation_hidden_rules",
            "--seed-start",
            "0",
            "--seed-count",
            "2",
            "--duration-ns",
            "150000000000",
            "--trace-sample-rate",
            "0",
            "--out",
        ])
        .arg(&manifest_path)
        .status()
        .unwrap();
    assert!(status.success());
    let m: StreamManifest =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(
        m.arms.iter().map(|a| a.arm.as_str()).collect::<Vec<_>>(),
        [
            "n",
            "a",
            "oracle_escalation_privileged",
            "ablation_hidden_rules"
        ]
    );
    assert_eq!(m.stream_params.duration_ns, 150_000_000_000);
    assert!(m.validate().is_ok());
    let out = dir.join("out");
    let status = Command::new(bin)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    for arm in [
        "n",
        "a",
        "oracle_escalation_privileged",
        "ablation_hidden_rules",
    ] {
        assert_eq!(
            read(&out.join(arm), "results.csv").lines().count(),
            3,
            "{arm}"
        );
    }
    // A manifest that is neither kind is a usage failure, not a panic.
    let junk = dir.join("junk.json");
    fs::write(&junk, "{}").unwrap();
    let status = Command::new(bin)
        .arg("--manifest")
        .arg(&junk)
        .arg("--out")
        .arg(dir.join("junk-out"))
        .status()
        .unwrap();
    assert!(!status.success());
}
