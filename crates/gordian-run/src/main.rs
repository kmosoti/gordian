//! `gordian-run`: execute one manifest of episodes and record the result.
//!
//! ```text
//! gordian-run --manifest FILE --out DIR
//! gordian-run init --run-id ID --policy POLICY --seed-start N --seed-count N --out FILE
//!                  [--experiment NAME] [--arm NAME] [--trace-sample-rate R]
//!                  [--components NAMES] [--every K] [--p P] [--patience-ns N]
//!                  [--arms ARMS] [--run-seed N] [--drift-block N]
//! ```
//!
//! `--arms` makes the manifest an interleaved one (plan A8): a comma-separated list of arm names,
//! each either `NAME`, which plays the `--policy` with its parameters, or `NAME=POLICY`, which
//! plays that policy with its defaults. It replaces `--arm`. `--run-seed` seeds the per-episode
//! arm order and `--drift-block` is the number of episodes between drift-control workloads
//! (default 50). For anything else, edit the manifest.
//!
//! `--policy` is one of `heuristic_only`, `fixed_pipeline`, `all_components`, `random_matched`,
//! `oracle_immediate`, `oracle_evidence`. `--components` (comma-separated names from `heuristic`,
//! `estimator`, `memory`, `verifier`) and `--every` configure `fixed_pipeline`; `--p` configures
//! `random_matched`; `--patience-ns` is the shared decision rule's, the same for every arm. A
//! parameter a policy does not have is an error. An oracle arm's default `--arm` ends in
//! `_privileged`, and the manifest refuses an oracle arm whose name lacks `privileged`.
//!
//! The first form runs the manifest and writes the run directory. It is normally launched by
//! `scripts/run-driver.sh`, which isolates and times it. The second writes a manifest for the
//! current checkout, with every episode class on the given seeds and the environment captured
//! from the machine; edit the file for anything else before running it.
//!
//! # Stream runs (work item R3)
//!
//! ```text
//! gordian-run --manifest FILE --out DIR          (a manifest with a `stream_params` key)
//! gordian-run init-stream --run-id ID --arms ARMS --seed-start N --seed-count N --out FILE
//!                         [--experiment NAME] [--trace-sample-rate R] [--run-seed N]
//!                         [--drift-block N] [--duration-ns N]
//! ```
//!
//! The first form tells a stream manifest from an episode manifest by its `stream_params` key and
//! plays every seed once per arm, interleaved, scoring each stream with the stream evaluator
//! (`gordian-stream-eval`; an evaluator error stops the run). `--arms` is a comma-separated list of `NAME=POLICY`
//! (policy defaults; edit the manifest for parameters); `POLICY` is one of `never_escalate`,
//! `always_escalate`, `periodic_escalation`, `change_triggered`, `threshold_score`,
//! `random_escalation`, `oracle_escalation` (privileged: its name must contain `privileged`) and
//! `ablation_hidden_rules` (never a comparison arm: its name must contain `ablation`). A bare
//! `POLICY` names the arm after it, with `_privileged` appended for the oracle.
//! `--duration-ns` shortens or lengthens the streams and rewrites the limits to match.
//!
//! Exit status: 0 on success, 1 when the run failed, 2 on a usage error.

use gordian_run::manifest::{ArmSpec, Manifest, PRIVILEGED};
use gordian_run::policy::PolicySpec;
use gordian_run::policy::decide::DecideConfig;
use gordian_run::recorder::execute_report;
use gordian_run::stream::manifest::{StreamArmSpec, StreamLimits, StreamManifest};
use gordian_run::stream::{StreamPolicySpec, execute_stream};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage:
  gordian-run --manifest FILE --out DIR
  gordian-run init --run-id ID --policy POLICY --seed-start N --seed-count N --out FILE
                   [--experiment NAME] [--arm NAME] [--trace-sample-rate R]
                   [--components NAMES] [--every K] [--p P] [--patience-ns N]
                   [--arms ARMS] [--run-seed N] [--drift-block N]
  gordian-run init-stream --run-id ID --arms ARMS --seed-start N --seed-count N --out FILE
                          [--experiment NAME] [--trace-sample-rate R] [--run-seed N]
                          [--drift-block N] [--duration-ns N]";

fn parse_flags(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut flags = BTreeMap::new();
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected argument {flag:?}"))?;
        if !allowed.contains(&name) {
            return Err(format!("unknown option --{name}"));
        }
        let value = iter
            .next()
            .ok_or_else(|| format!("--{name} needs a value"))?;
        if flags.insert(name.to_owned(), value.clone()).is_some() {
            return Err(format!("--{name} given twice"));
        }
    }
    Ok(flags)
}

fn required<'a>(flags: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    flags
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("--{name} is required"))
}

fn number<T: std::str::FromStr>(flags: &BTreeMap<String, String>, name: &str) -> Result<T, String> {
    required(flags, name)?
        .parse()
        .map_err(|_| format!("--{name} is not a valid number"))
}

fn run(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args, &["manifest", "out"])?;
    let manifest_path = PathBuf::from(required(&flags, "manifest")?);
    let out = PathBuf::from(required(&flags, "out")?);
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
    let probe: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    if probe.get("stream_params").is_some() {
        return run_stream(&text, &manifest_path, &out);
    }
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let report = execute_report(&manifest, &out).map_err(|e| e.to_string())?;
    let summary = report.total;
    eprintln!(
        "gordian-run: {} episodes, {} successes, {} undecided, {} sampled, {} drift blocks",
        summary.episodes,
        summary.successes,
        summary.undecided,
        summary.sampled,
        report.drift_blocks
    );
    if manifest.is_interleaved() {
        for (arm, s) in &report.arms {
            eprintln!(
                "gordian-run:   arm {arm}: {} episodes, {} successes, {} undecided",
                s.episodes, s.successes, s.undecided
            );
        }
    }
    Ok(())
}

fn run_stream(
    text: &str,
    manifest_path: &std::path::Path,
    out: &std::path::Path,
) -> Result<(), String> {
    let manifest: StreamManifest =
        serde_json::from_str(text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let report = execute_stream(&manifest, out).map_err(|e| e.to_string())?;
    eprintln!(
        "gordian-run: {} segments, {} reasoner calls, {} declarations, {} sampled, {} drift blocks",
        report.total.segments,
        report.total.reasoner_calls,
        report.total.declarations,
        report.total.sampled,
        report.drift_blocks
    );
    for (arm, s) in &report.arms {
        eprintln!(
            "gordian-run:   arm {arm}: {} segments, {} reasoner calls, {} declarations, {} step-capped",
            s.segments, s.reasoner_calls, s.declarations, s.step_capped
        );
    }
    Ok(())
}

/// The arms of `--arms` of `init-stream`: `NAME=POLICY` or a bare `POLICY`, with defaults.
fn parse_stream_arms(list: &str) -> Result<Vec<StreamArmSpec>, String> {
    let mut arms = Vec::new();
    for entry in list.split(',') {
        let entry = entry.trim();
        let (name, id) = match entry.split_once('=') {
            Some((name, id)) => (name.trim().to_owned(), id.trim()),
            None => (String::new(), entry),
        };
        let policy = StreamPolicySpec::from_id(id)?;
        let name = if name.is_empty() {
            match policy.role() {
                gordian_run::stream::arms::ArmRole::Privileged => format!("{id}_privileged"),
                _ => id.to_owned(),
            }
        } else {
            name
        };
        arms.push(StreamArmSpec {
            arm: name,
            policy,
            context: None,
        });
    }
    Ok(arms)
}

fn init_stream(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(
        args,
        &[
            "run-id",
            "arms",
            "seed-start",
            "seed-count",
            "out",
            "experiment",
            "trace-sample-rate",
            "run-seed",
            "drift-block",
            "duration-ns",
        ],
    )?;
    let rate = match flags.get("trace-sample-rate") {
        Some(_) => number::<f64>(&flags, "trace-sample-rate")?,
        None => 0.01,
    };
    let mut manifest = StreamManifest::for_current_environment(
        required(&flags, "run-id")?,
        flags
            .get("experiment")
            .map_or("exploration", String::as_str),
        parse_stream_arms(required(&flags, "arms")?)?,
        number(&flags, "seed-start")?,
        number(&flags, "seed-count")?,
        rate,
    )?;
    if flags.contains_key("duration-ns") {
        manifest.stream_params.duration_ns = number(&flags, "duration-ns")?;
        manifest.limits = StreamLimits::matching(
            &manifest.stream_params,
            manifest.limits.compute,
            manifest.limits.step_ns,
        );
    }
    if flags.contains_key("run-seed") {
        manifest.run_seed = number(&flags, "run-seed")?;
    }
    if flags.contains_key("drift-block") {
        manifest.drift_block = number(&flags, "drift-block")?;
    }
    manifest.validate()?;
    let out = PathBuf::from(required(&flags, "out")?);
    if out.exists() {
        return Err(format!("{} already exists", out.display()));
    }
    fs::write(&out, manifest.canonical_json())
        .map_err(|e| format!("cannot write {}: {e}", out.display()))
}

/// The arms of `--arms`: `NAME` plays `default`, `NAME=POLICY` plays that policy's defaults.
fn parse_arms(list: &str, default: &PolicySpec) -> Result<Vec<ArmSpec>, String> {
    let mut arms = Vec::new();
    for entry in list.split(',') {
        let entry = entry.trim();
        let (name, policy) = match entry.split_once('=') {
            Some((name, id)) => (name.trim(), PolicySpec::from_id(id.trim())?),
            None => (entry, default.clone()),
        };
        if name.is_empty() {
            return Err(format!("--arms entry {entry:?} has no arm name"));
        }
        arms.push(ArmSpec {
            arm: name.to_owned(),
            policy,
        });
    }
    Ok(arms)
}

fn init(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(
        args,
        &[
            "run-id",
            "policy",
            "seed-start",
            "seed-count",
            "out",
            "experiment",
            "arm",
            "trace-sample-rate",
            "components",
            "every",
            "p",
            "patience-ns",
            "arms",
            "run-seed",
            "drift-block",
        ],
    )?;
    if flags.contains_key("arms") && flags.contains_key("arm") {
        return Err("--arms replaces --arm; give one of them".to_owned());
    }
    let components = flags.get("components").map(|list| {
        list.split(',')
            .map(|name| name.trim().to_owned())
            .collect::<Vec<_>>()
    });
    let policy = PolicySpec::from_parts(
        required(&flags, "policy")?,
        components,
        flags
            .get("every")
            .map(|_| number(&flags, "every"))
            .transpose()?,
        flags.get("p").map(|_| number(&flags, "p")).transpose()?,
    )?;
    let decide = match flags.get("patience-ns") {
        Some(_) => DecideConfig {
            patience_ns: number(&flags, "patience-ns")?,
        },
        None => DecideConfig::default(),
    };
    let rate = match flags.get("trace-sample-rate") {
        Some(_) => number::<f64>(&flags, "trace-sample-rate")?,
        None => 0.01,
    };
    // An oracle arm's default name says so, because the manifest refuses one that does not.
    let default_arm = if policy.is_privileged() {
        format!("{}_{PRIVILEGED}", policy.id().0)
    } else {
        policy.id().0
    };
    let arms = match flags.get("arms") {
        Some(list) => Some(parse_arms(list, &policy)?),
        None => None,
    };
    // The one-arm spelling of the first arm: `for_current_environment` validates it, and an
    // interleaved manifest keeps `arm` and `policy` equal to the first of its `arms`.
    let (first_arm, first_policy) = match &arms {
        Some(arms) => (arms[0].arm.as_str(), &arms[0].policy),
        None => (
            flags
                .get("arm")
                .map_or(default_arm.as_str(), String::as_str),
            &policy,
        ),
    };
    let mut manifest = Manifest::for_current_environment(
        required(&flags, "run-id")?,
        flags
            .get("experiment")
            .map_or("exploration", String::as_str),
        first_arm,
        first_policy,
        decide,
        number(&flags, "seed-start")?,
        number(&flags, "seed-count")?,
        rate,
    )?;
    if let Some(arms) = arms {
        manifest.arms = arms;
    }
    if flags.contains_key("run-seed") {
        manifest.run_seed = number(&flags, "run-seed")?;
    }
    if flags.contains_key("drift-block") {
        manifest.drift_block = number(&flags, "drift-block")?;
    }
    manifest.validate()?;
    let out = PathBuf::from(required(&flags, "out")?);
    if out.exists() {
        return Err(format!("{} already exists", out.display()));
    }
    fs::write(&out, manifest.canonical_json())
        .map_err(|e| format!("cannot write {}: {e}", out.display()))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        Some("init") => init(&args[1..]),
        Some("init-stream") => init_stream(&args[1..]),
        Some(_) => run(&args),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("gordian-run: {why}");
            if why.starts_with("unknown option")
                || why.contains("is required")
                || why.starts_with("unexpected argument")
                || why.contains("needs a value")
            {
                eprintln!("{USAGE}");
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
