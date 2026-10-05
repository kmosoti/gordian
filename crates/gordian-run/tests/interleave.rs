//! Interleaved arms and the drift control (plan item A8).

mod common;

use common::*;
use gordian_run::drift::{DRIFT_HEADER, REPS};
use gordian_run::interleave::arm_order;
use gordian_run::manifest::{ArmSpec, EpisodeParams, IsolationSpec, Manifest};
use gordian_run::policy::decide::DecideConfig;
use gordian_run::policy::{self, Built, Policy, PolicyId, PolicySpec};
use gordian_run::recorder::{RunError, execute, execute_report, execute_with};
use gordian_world::EpisodeClass;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::fs;
use std::path::Path;

fn arm(name: &str, policy: &str) -> ArmSpec {
    ArmSpec {
        arm: name.to_owned(),
        policy: PolicySpec::from_id(policy).unwrap(),
    }
}

/// A manifest with a fixed, fake environment over `seeds` seeds of every class.
fn manifest(run_id: &str, arms: Vec<ArmSpec>, seeds: u64, run_seed: u64) -> Manifest {
    Manifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arm: arms[0].arm.clone(),
        policy: arms[0].policy.clone(),
        arms,
        run_seed,
        drift_block: 50,
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
        decide: DecideConfig::default(),
        limits: limits(),
        episode_params: EpisodeParams::default(),
        trace_sample_rate: 0.0,
        internal_external_ratio: None,
    }
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()))
}

fn rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

/// Three arms of three kinds, one of them seeded by its name.
fn three_arms() -> Vec<ArmSpec> {
    let mut random = arm("random-arm", "random_matched");
    random.policy = PolicySpec::from_parts("random_matched", None, None, Some(0.4)).unwrap();
    vec![
        arm("heur", "heuristic_only"),
        random,
        arm("all", "all_components"),
    ]
}

#[test]
fn a_multi_arm_run_writes_one_directory_per_arm_and_a_drift_file() {
    let m = manifest("layout", three_arms(), 2, 5);
    let dir = scratch("il-layout");
    let report = execute_report(&m, &dir).unwrap();
    assert_eq!(report.total.episodes, 3 * 22);
    assert_eq!(
        report
            .arms
            .iter()
            .map(|(a, s)| (a.as_str(), s.episodes))
            .collect::<Vec<_>>(),
        vec![("heur", 22), ("random-arm", 22), ("all", 22)]
    );
    assert!(dir.join("manifest.json").is_file());
    assert!(dir.join("drift.csv").is_file());
    // The arms' files are in their directories and not in the run directory.
    assert!(!dir.join("results.csv").exists());
    assert!(!dir.join("measured.csv").exists());
    for (index, spec) in m.arms.iter().enumerate() {
        let arm_dir = dir.join(&spec.arm);
        for file in ["results.csv", "measured.csv", "manifest.json"] {
            assert!(arm_dir.join(file).is_file(), "{}/{file}", spec.arm);
        }
        // The arm's manifest is its one-arm manifest, and its rows carry that run id.
        assert_eq!(
            read(&arm_dir, "manifest.json"),
            m.single_arm(index).canonical_json()
        );
        let results = rows(&read(&arm_dir, "results.csv"));
        assert_eq!(results.len(), 22);
        assert!(
            results
                .iter()
                .all(|r| r[0] == format!("layout.{}", spec.arm))
        );
        let measured = read(&arm_dir, "measured.csv");
        assert!(measured.lines().next().unwrap().ends_with(",arm_position"));
        assert!(
            rows(&measured)
                .iter()
                .all(|r| r[0] == format!("layout.{}", spec.arm))
        );
    }
    // The whole manifest in the run directory is the manifest that ran.
    let back: Manifest = serde_json::from_str(&read(&dir, "manifest.json")).unwrap();
    assert_eq!(back, m);
}

#[test]
fn the_positions_of_an_episode_are_a_permutation_of_the_arms_and_the_drawn_one() {
    let m = manifest("positions", three_arms(), 3, 77);
    let dir = scratch("il-positions");
    execute(&m, &dir).unwrap();
    let measured: Vec<Vec<Vec<String>>> = m
        .arms
        .iter()
        .map(|a| rows(&read(&dir.join(&a.arm), "measured.csv")))
        .collect();
    for (row, (seed, class)) in m.episodes().into_iter().enumerate() {
        let expected = arm_order(77, seed, class, 3);
        let mut positions = Vec::new();
        for (index, arm_rows) in measured.iter().enumerate() {
            assert_eq!(arm_rows[row][1], seed.to_string());
            assert_eq!(arm_rows[row][2], format!("{class:?}"));
            let position: usize = arm_rows[row][6].parse().unwrap();
            assert_eq!(expected[position], index, "seed {seed} class {class:?}");
            positions.push(position);
        }
        positions.sort_unstable();
        assert_eq!(positions, vec![0, 1, 2]);
    }
    // With 33 episodes and 3 arms, every arm is first at least once (probability of failing
    // for a fair draw: 3 * (2/3)^33, about 4e-6).
    for arm_rows in &measured {
        assert!(arm_rows.iter().any(|r| r[6] == "0"));
    }
}

#[test]
fn episodes_are_played_back_to_back_in_the_drawn_order_in_one_process() {
    let m = manifest(
        "backtoback",
        vec![
            arm("one", "heuristic_only"),
            arm("two", "all_components"),
            arm("three", "fixed_pipeline"),
        ],
        2,
        9,
    );
    let ids = ["heuristic_only", "all_components", "fixed_pipeline"];
    let log: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let make = |id: &PolicyId| -> Option<Box<dyn Policy>> {
        log.borrow_mut().push(id.0.clone());
        let spec = PolicySpec::from_id(&id.0).ok()?;
        match policy::build(&spec, &DecideConfig::default(), &id.0, 0) {
            Built::Public(policy) => Some(policy),
            Built::Privileged(_) => None,
        }
    };
    execute_with(&m, &scratch("il-backtoback"), &make).unwrap();
    let log = log.into_inner();
    // The factory is asked once per arm before the run (a check that every policy exists), then
    // once per arm-episode, in the order of play.
    let (_, played) = log.split_at(3);
    let mut expected = Vec::new();
    for (seed, class) in m.episodes() {
        for index in arm_order(9, seed, class, 3) {
            expected.push(ids[index].to_owned());
        }
    }
    assert_eq!(played, expected.as_slice());
}

#[test]
fn every_arms_results_equal_a_single_arm_run_of_that_arm_byte_for_byte() {
    let m = manifest("replay", three_arms(), 3, 1234);
    let dir = scratch("il-replay");
    execute(&m, &dir).unwrap();
    for (index, spec) in m.arms.iter().enumerate() {
        let alone = scratch(&format!("il-replay-alone-{}", spec.arm));
        let single = m.single_arm(index);
        assert!(!single.is_interleaved());
        execute(&single, &alone).unwrap();
        assert_eq!(
            read(&dir.join(&spec.arm), "results.csv"),
            read(&alone, "results.csv"),
            "arm {}",
            spec.arm
        );
        // The arm directory's own manifest is that same one-arm manifest, so it can be rerun
        // from the file.
        let from_file: Manifest =
            serde_json::from_str(&read(&dir.join(&spec.arm), "manifest.json")).unwrap();
        assert_eq!(from_file, single);
        let again = scratch(&format!("il-replay-file-{}", spec.arm));
        execute(&from_file, &again).unwrap();
        assert_eq!(
            read(&dir.join(&spec.arm), "results.csv"),
            read(&again, "results.csv")
        );
    }
}

#[test]
fn the_order_of_play_and_the_drift_workload_do_not_change_any_arms_results() {
    let base = manifest("indep", three_arms(), 3, 1);
    let mut other_order = base.clone();
    other_order.run_seed = 987_654_321;
    let mut drifty = base.clone();
    drifty.drift_block = 1;
    let mut calm = base.clone();
    calm.drift_block = 100_000;

    let dirs: Vec<_> = [
        ("base", &base),
        ("order", &other_order),
        ("drifty", &drifty),
        ("calm", &calm),
    ]
    .into_iter()
    .map(|(name, m)| {
        let dir = scratch(&format!("il-indep-{name}"));
        execute(m, &dir).unwrap();
        dir
    })
    .collect();
    // The orders differ between the first two runs (otherwise the comparison says nothing).
    let positions = |dir: &Path| -> Vec<String> {
        rows(&read(&dir.join("heur"), "measured.csv"))
            .into_iter()
            .map(|r| r[6].clone())
            .collect()
    };
    assert_ne!(positions(&dirs[0]), positions(&dirs[1]));
    for spec in &base.arms {
        let reference = read(&dirs[0].join(&spec.arm), "results.csv");
        for dir in &dirs[1..] {
            assert_eq!(
                reference,
                read(&dir.join(&spec.arm), "results.csv"),
                "{}",
                spec.arm
            );
        }
    }
    // The run with a workload at every episode has one block per episode and the closing one.
    assert_eq!(rows(&read(&dirs[2], "drift.csv")).len(), 33 + 1);
    assert_eq!(rows(&read(&dirs[3], "drift.csv")).len(), 1 + 1);
}

#[test]
fn drift_csv_has_a_block_every_drift_block_episodes_and_a_closing_one() {
    let mut m = manifest("drift", vec![arm("heur", "heuristic_only")], 1, 0);
    m.drift_block = 4;
    let dir = scratch("il-drift");
    let report = execute_report(&m, &dir).unwrap();
    let text = read(&dir, "drift.csv");
    assert_eq!(text.lines().next().unwrap(), DRIFT_HEADER);
    let r = rows(&text);
    // 11 episodes: before episodes 0, 4 and 8, then after the last.
    let blocks: Vec<(String, String)> = r.iter().map(|x| (x[1].clone(), x[2].clone())).collect();
    let want: Vec<(String, String)> = [(0, 0), (1, 4), (2, 8), (3, 11)]
        .iter()
        .map(|(b, u)| (b.to_string(), u.to_string()))
        .collect();
    assert_eq!(blocks, want);
    assert_eq!(report.drift_blocks, 4);
    for row in &r {
        assert_eq!(row[0], "drift");
        assert_eq!(row[3], REPS.to_string());
        let (ns, min_ns): (u64, u64) = (row[4].parse().unwrap(), row[5].parse().unwrap());
        assert!(min_ns > 0 && ns >= min_ns * u64::from(REPS), "{row:?}");
    }
}

#[test]
fn the_drift_workload_is_no_arms_measured_time_and_the_intervals_are_disjoint() {
    // Every interval the harness records is a stretch of the caller's own wall time, and the
    // stretches do not overlap: an episode's three measured sums (generation to scoring) and a
    // drift block are different spans of the same call. So the sum of every arm's measured
    // columns and every drift timing is at most the wall time of `execute`. A drift block that
    // were also inside an arm's episode would be counted twice and push the sum over the wall
    // time once the blocks are a large share of it (here a block before every episode).
    let mut m = manifest("disjoint", three_arms(), 2, 0);
    m.drift_block = 1;
    let dir = scratch("il-disjoint");
    let started = std::time::Instant::now();
    execute(&m, &dir).unwrap();
    let wall = started.elapsed().as_nanos() as u64;
    let drift_ns: u64 = rows(&read(&dir, "drift.csv"))
        .iter()
        .map(|r| r[4].parse::<u64>().unwrap())
        .sum();
    let measured_ns: u64 = m
        .arms
        .iter()
        .flat_map(|a| rows(&read(&dir.join(&a.arm), "measured.csv")))
        .map(|r| {
            r[3..6]
                .iter()
                .map(|v| v.parse::<u64>().unwrap())
                .sum::<u64>()
        })
        .sum();
    assert!(drift_ns > 0 && measured_ns > 0);
    assert!(
        measured_ns + drift_ns <= wall,
        "arms {measured_ns} ns + drift {drift_ns} ns exceed the wall time {wall} ns"
    );
    eprintln!("arms {measured_ns} ns, drift {drift_ns} ns, wall {wall} ns");
}

#[test]
fn an_oracle_arm_may_share_a_run_but_must_say_so_in_its_name() {
    let mut ok = manifest(
        "priv",
        vec![
            arm("heur", "heuristic_only"),
            arm("oracle_immediate_privileged", "oracle_immediate"),
        ],
        2,
        3,
    );
    let dir = scratch("il-priv");
    execute(&ok, &dir).unwrap();
    // The privileged arm's results are what it gives alone.
    let alone = scratch("il-priv-alone");
    execute(&ok.single_arm(1), &alone).unwrap();
    assert_eq!(
        read(&dir.join("oracle_immediate_privileged"), "results.csv"),
        read(&alone, "results.csv")
    );
    // The public arm is unaffected by sharing the process with an arm that sees the truth.
    let heur_alone = scratch("il-priv-heur");
    execute(&ok.single_arm(0), &heur_alone).unwrap();
    assert_eq!(
        read(&dir.join("heur"), "results.csv"),
        read(&heur_alone, "results.csv")
    );
    // An oracle arm whose name does not say so is refused.
    ok.arms[1].arm = "oracle".to_owned();
    assert!(matches!(
        execute(&ok, &scratch("il-priv-bad")).unwrap_err(),
        RunError::Manifest(_)
    ));
}

#[test]
fn a_one_arm_manifest_keeps_its_layout_and_gains_position_zero_and_a_drift_file() {
    let mut flat = manifest("onearm", vec![arm("heur", "heuristic_only")], 2, 0);
    flat.arms.clear();
    assert!(!flat.is_interleaved());
    let dir = scratch("il-onearm");
    execute(&flat, &dir).unwrap();
    for file in ["results.csv", "measured.csv", "manifest.json", "drift.csv"] {
        assert!(dir.join(file).is_file(), "{file}");
    }
    assert!(!dir.join("heur").exists());
    assert!(
        rows(&read(&dir, "measured.csv"))
            .iter()
            .all(|r| r[6] == "0")
    );
    assert!(
        rows(&read(&dir, "results.csv"))
            .iter()
            .all(|r| r[0] == "onearm")
    );
    // A one-arm manifest's JSON has `arm` and `policy` and no `arms`.
    let v: Value = serde_json::from_str(&flat.canonical_json()).unwrap();
    assert_eq!(v["arm"], "heur");
    assert_eq!(v["policy"], "heuristic_only");
    assert!(v.get("arms").is_none());
}

// ---- the manifest's two spellings ----

fn json_of(m: &Manifest) -> Value {
    serde_json::from_str(&m.canonical_json()).unwrap()
}

#[test]
fn an_interleaved_manifest_spells_its_arms_and_not_arm_and_policy() {
    let m = manifest("spell", three_arms(), 2, 42);
    let v = json_of(&m);
    assert!(v.get("arm").is_none() && v.get("policy").is_none());
    assert_eq!(v["run_seed"], 42);
    assert_eq!(v["drift_block"], 50);
    assert_eq!(
        v["arms"][0],
        json!({"arm": "heur", "policy": "heuristic_only"})
    );
    assert_eq!(
        v["arms"][1],
        json!({"arm": "random-arm", "policy": {"policy": "random_matched", "p": 0.4}})
    );
    let back: Manifest = serde_json::from_value(v).unwrap();
    assert_eq!(back, m);
    assert_eq!(
        back.arm, "heur",
        "arm and policy hold the first arm in memory"
    );
}

#[test]
fn a_manifest_written_before_a8_still_parses_with_the_defaults() {
    let mut v = json_of(&manifest("old", vec![arm("heur", "heuristic_only")], 2, 0));
    let m = v.as_object_mut().unwrap();
    m.remove("arms");
    m.remove("run_seed");
    m.remove("drift_block");
    m.insert("arm".to_owned(), json!("heur"));
    m.insert("policy".to_owned(), json!("heuristic_only"));
    let parsed: Manifest = serde_json::from_value(v).unwrap();
    assert!(!parsed.is_interleaved());
    assert_eq!((parsed.run_seed, parsed.drift_block), (0, 50));
    assert_eq!(parsed.arm_specs().len(), 1);
    parsed.validate().unwrap();
}

#[test]
fn the_two_spellings_do_not_mix_and_one_is_required() {
    let both = {
        let mut v = json_of(&manifest("both", three_arms(), 2, 0));
        v["arm"] = json!("heur");
        v["policy"] = json!("heuristic_only");
        v
    };
    let err = serde_json::from_value::<Manifest>(both)
        .unwrap_err()
        .to_string();
    assert!(err.contains("not both"), "{err}");
    let neither = {
        let mut v = json_of(&manifest("neither", three_arms(), 2, 0));
        v.as_object_mut().unwrap().remove("arms");
        v
    };
    let err = serde_json::from_value::<Manifest>(neither)
        .unwrap_err()
        .to_string();
    assert!(err.contains("`arm` and `policy`"), "{err}");
}

#[test]
fn arm_names_must_be_distinct_safe_and_not_a_run_file() {
    let check = |names: &[&str]| -> Result<(), String> {
        let arms = names.iter().map(|n| arm(n, "heuristic_only")).collect();
        manifest("names", arms, 2, 0).validate()
    };
    check(&["a1", "a2"]).unwrap();
    assert!(check(&["a", "a"]).unwrap_err().contains("twice"));
    for bad in [
        "",
        "..",
        ".",
        "a/b",
        "a b",
        "drift.csv",
        "manifest.json",
        "usage.json",
    ] {
        assert!(check(&["ok", bad]).is_err(), "{bad:?}");
    }
    assert!(check(&[&"x".repeat(51)]).is_err());
    let mut m = manifest("names", vec![arm("a", "heuristic_only")], 2, 0);
    m.drift_block = 0;
    assert!(m.validate().unwrap_err().contains("drift_block"));
    m.drift_block = 1;
    m.arms[0].arm = "b".to_owned();
    assert!(m.validate().unwrap_err().contains("first of `arms`"));
}
