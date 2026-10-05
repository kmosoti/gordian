//! Work item A6c: the shared rule decodes each component output once.
//!
//! The acceptance of the item is that nothing the rule decides changes, and only what it costs
//! does. `verdicts_at_the_default_budget_are_those_the_rule_gave_before_decode_once` checks that
//! end to end through the recorder; the tests below it check the mechanism and its accounting.

mod common;

use common::*;
use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID};
use gordian_run::manifest::{ArmSpec, EpisodeParams, IsolationSpec, Manifest, PRIVILEGED};
use gordian_run::policy::decide::DecideConfig;
use gordian_run::policy::{PolicySpec, fixed_pipeline, random_matched};
use gordian_run::recorder::execute;
use gordian_world::EpisodeClass;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The ten arms of the B1 grid (`experiments/exploration/scripts/mkmanifest.py`), by name.
fn b1_arms() -> Vec<ArmSpec> {
    let pipeline = |components: Vec<gordian_core::ComponentId>, every: u32| {
        PolicySpec::FixedPipeline(fixed_pipeline::Config { components, every })
    };
    let arm = |name: &str, policy: PolicySpec| ArmSpec {
        arm: name.to_owned(),
        policy,
    };
    vec![
        arm("heuristic_only", PolicySpec::HeuristicOnly),
        arm("all_components", PolicySpec::AllComponents),
        arm(
            "random_p025",
            PolicySpec::RandomMatched(random_matched::Config { p: 0.25 }),
        ),
        arm(
            "random_p050",
            PolicySpec::RandomMatched(random_matched::Config { p: 0.5 }),
        ),
        arm("fixed_verifier_only", pipeline(vec![VERIFIER_ID], 1)),
        arm("fixed_estimator_only", pipeline(vec![ESTIMATOR_ID], 1)),
        arm("fixed_heuristic_every2", pipeline(vec![HEURISTIC_ID], 2)),
        arm("fixed_heuristic_every4", pipeline(vec![HEURISTIC_ID], 4)),
        arm(
            &format!("oracle_evidence_{PRIVILEGED}"),
            PolicySpec::OracleEvidence,
        ),
        arm(
            &format!("oracle_immediate_{PRIVILEGED}"),
            PolicySpec::OracleImmediate,
        ),
    ]
}

/// An interleaved manifest of `arms` over `seeds` seeds of every class, at `compute` nanoseconds.
fn grid_manifest(run_id: &str, arms: Vec<ArmSpec>, seeds: u64, compute: u64) -> Manifest {
    let mut limits = limits();
    limits.compute = compute;
    Manifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arm: arms[0].arm.clone(),
        source_revision: "0".repeat(40),
        lockfile_sha256: "0".repeat(64),
        toolchain: "rustc test".to_owned(),
        cpu_flags: vec!["avx2".to_owned()],
        cpu_model: Some("test cpu".to_owned()),
        cpu_mhz: Some(2100.0),
        isolation: IsolationSpec::default(),
        seeds: (0..seeds).collect(),
        episode_classes: EpisodeClass::ALL
            .iter()
            .map(|c| (*c, seeds as u32))
            .collect(),
        policy: arms[0].policy.clone(),
        arms,
        run_seed: 0,
        drift_block: 50,
        decide: DecideConfig::default(),
        limits,
        episode_params: EpisodeParams::default(),
        trace_sample_rate: 0.0,
        internal_external_ratio: None,
    }
}

/// The columns of `results.csv` that say what the arm concluded, as opposed to what it cost or
/// when. These are the verdict columns of work item A6c.
const VERDICT_COLUMNS: [&str; 7] = [
    "success",
    "critical_miss",
    "false_alarm",
    "abstained",
    "undecided",
    "probes_used",
    "corrections",
];

/// `(arm, seed, class)` to the verdict columns joined by commas, over every arm of a run.
fn verdicts(dir: &Path, arms: &[ArmSpec]) -> BTreeMap<(String, u64, String), String> {
    let mut out = BTreeMap::new();
    for arm in arms {
        let csv = fs::read_to_string(dir.join(&arm.arm).join("results.csv")).unwrap();
        let mut lines = csv.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let at = |name: &str| header.iter().position(|h| *h == name).unwrap();
        let (seed, class) = (at("seed"), at("class"));
        let columns: Vec<usize> = VERDICT_COLUMNS.iter().map(|c| at(c)).collect();
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            let key = (
                arm.arm.clone(),
                f[seed].parse().unwrap(),
                f[class].to_owned(),
            );
            let value = columns.iter().map(|c| f[*c]).collect::<Vec<_>>().join(",");
            assert!(out.insert(key, value).is_none(), "duplicate episode");
        }
    }
    out
}

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/verdicts-before-decode-once.csv"
);

/// Every arm at the default 20 ms compute budget, 20 seeds by 11 classes, must give for each
/// episode the verdict columns the rule gave before work item A6c. The fixture was written by this
/// test at commit da73030 (the last commit before the change), by running it with
/// `GORDIAN_WRITE_VERDICT_FIXTURE=1`; it is a record of the old behaviour, so it is never
/// regenerated from the new code. At 20 ms no arm is near its limit, so a verdict that moved would
/// mean the rule's decisions had depended on how often it decoded.
#[test]
fn verdicts_at_the_default_budget_are_those_the_rule_gave_before_decode_once() {
    let arms = b1_arms();
    let m = grid_manifest("a6c-verdicts", arms.clone(), 20, 20_000_000);
    let dir = scratch("a6c-verdicts");
    execute(&m, &dir).unwrap();
    let now = verdicts(&dir, &arms);
    assert_eq!(now.len(), arms.len() * 11 * 20);

    if std::env::var_os("GORDIAN_WRITE_VERDICT_FIXTURE").is_some() {
        let mut text = format!("arm,seed,class,{}\n", VERDICT_COLUMNS.join(","));
        for ((arm, seed, class), v) in &now {
            text.push_str(&format!("{arm},{seed},{class},{v}\n"));
        }
        fs::create_dir_all(Path::new(FIXTURE).parent().unwrap()).unwrap();
        fs::write(FIXTURE, text).unwrap();
        return;
    }

    let recorded = fs::read_to_string(FIXTURE).expect("the pre-A6c verdict fixture");
    let mut before = BTreeMap::new();
    for line in recorded.lines().skip(1) {
        let f: Vec<&str> = line.splitn(4, ',').collect();
        before.insert(
            (
                f[0].to_owned(),
                f[1].parse::<u64>().unwrap(),
                f[2].to_owned(),
            ),
            f[3].to_owned(),
        );
    }
    assert_eq!(before.len(), now.len(), "the fixture covers every episode");
    let moved: Vec<String> = now
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, v)| format!("{k:?}: before {:?}, now {v:?}", before.get(k)))
        .collect();
    assert!(
        moved.is_empty(),
        "{} of {} verdicts changed, first: {:#?}",
        moved.len(),
        now.len(),
        &moved[..moved.len().min(10)]
    );
}
