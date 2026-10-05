//! Helpers shared by the stream harness's integration tests.

#![allow(dead_code, missing_docs)]

use gordian_core::Instant;
use gordian_run::manifest::IsolationSpec;
use gordian_run::stream::arms::rung::RungConfig;
use gordian_run::stream::manifest::{
    DEFAULT_COMPUTE_NS, DEFAULT_STEP_NS, Exchange, StreamArmSpec, StreamLimits, StreamManifest,
};
use gordian_run::stream::spec::{StreamPolicySpec, build_public, privileged_factory};
use gordian_run::stream::{SegmentRecord, StreamHarnessError, run_segment, run_segment_privileged};
use gordian_stream::{StreamParams, StreamPublic, generate};
use std::fs;
use std::path::{Path, PathBuf};

/// Stream parameters that keep a test fast: a short stream (still longer than the tail every
/// incident must fit in), seeded with `seed`. Regime changes are scheduled inside it.
pub fn params(seed: u64, duration_s: u64) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.duration_ns = duration_s * 1_000_000_000;
    p.regimes = vec![gordian_stream::RegimeSchedule {
        at_ns: duration_s * 500_000_000,
        change: gordian_stream::RegimeKind::SignatureShift,
    }];
    p
}

/// Limits matching `params`, with the default substrate allowance and step.
pub fn limits(params: &StreamParams) -> StreamLimits {
    StreamLimits::matching(params, DEFAULT_COMPUTE_NS, DEFAULT_STEP_NS)
}

/// A manifest with a fixed, fake environment over `seeds` seeds of streams `duration_s` long.
pub fn manifest(
    run_id: &str,
    arms: &[(&str, &str)],
    seeds: u64,
    duration_s: u64,
    run_seed: u64,
) -> StreamManifest {
    let arms: Vec<StreamArmSpec> = arms
        .iter()
        .map(|(name, policy)| StreamArmSpec {
            arm: (*name).to_owned(),
            policy: StreamPolicySpec::from_id(policy).unwrap(),
        })
        .collect();
    let stream_params = params(0, duration_s);
    StreamManifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arms,
        run_seed,
        drift_block: 50,
        source_revision: "0".repeat(40),
        lockfile_sha256: "0".repeat(64),
        toolchain: "rustc test".to_owned(),
        cpu_flags: vec!["avx2".to_owned()],
        cpu_model: Some("test cpu".to_owned()),
        cpu_mhz: Some(2100.0),
        isolation: IsolationSpec::default(),
        seeds: (0..seeds).collect(),
        limits: limits(&stream_params),
        stream_params,
        rung: RungConfig::default(),
        exchange: Exchange::default(),
        trace_sample_rate: 0.0,
        internal_external_ratio: None,
    }
}

/// Every arm of the registry, by id.
pub const ALL_ARMS: [&str; 11] = [
    "never_escalate",
    "always_escalate",
    "periodic_escalation",
    "change_triggered",
    "threshold_score",
    "random_escalation",
    "contradiction_escalation",
    "oracle_escalation",
    "oracle_selection",
    "oracle_decoy",
    "ablation_hidden_rules",
];

/// The arm name a manifest must give `policy`.
pub fn arm_name(policy: &str) -> String {
    match policy {
        "oracle_escalation" | "oracle_selection" | "oracle_decoy" => {
            format!("{policy}_privileged")
        }
        other => other.to_owned(),
    }
}

/// A manifest running every arm.
pub fn all_arms(run_id: &str, seeds: u64, duration_s: u64, run_seed: u64) -> StreamManifest {
    let pairs: Vec<(String, &str)> = ALL_ARMS.iter().map(|p| (arm_name(p), *p)).collect();
    let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(n, p)| (n.as_str(), *p)).collect();
    manifest(run_id, &borrowed, seeds, duration_s, run_seed)
}

/// Play one segment of `spec` on `params`, scored by the stream evaluator.
pub fn play(
    params: &StreamParams,
    spec: &StreamPolicySpec,
    limits: &StreamLimits,
) -> Result<SegmentRecord, StreamHarnessError> {
    let rung = RungConfig::default();
    let exchange = Exchange::default();
    let seed = params.seed;
    match privileged_factory(spec, &rung) {
        Some(factory) => run_segment_privileged(params, &factory, limits, &exchange),
        None => run_segment(
            params,
            &|public: &StreamPublic| {
                build_public(spec, &rung, public, &spec.id().0, seed).expect("a public arm")
            },
            limits,
            &exchange,
        ),
    }
}

/// The public information of the stream `params` describes.
pub fn public_of(params: &StreamParams) -> StreamPublic {
    generate(params).public_info()
}

/// A scratch directory under the target directory, emptied first.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/stream-test-runs")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()))
}

/// `csv` as rows of fields (no quoting is used in these files).
pub fn rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

/// The value of column `name` in row `row` (a data row, not the header) of `csv`.
pub fn cell(csv: &str, row: usize, name: &str) -> String {
    let table = rows(csv);
    let col = table[0]
        .iter()
        .position(|c| c == name)
        .unwrap_or_else(|| panic!("no column {name}"));
    table[row + 1][col].clone()
}

/// Every data row of `csv` without its first column (`run_id`), so that two runs that differ
/// only in their run id compare equal.
pub fn without_run_id(csv: &str) -> Vec<String> {
    csv.lines()
        .skip(1)
        .map(|l| l.split_once(',').unwrap().1.to_owned())
        .collect()
}

/// A tiny instant helper.
pub fn at(ms: u64) -> Instant {
    Instant(ms * 1_000_000)
}
