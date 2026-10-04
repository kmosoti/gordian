//! `gordian-run`: execute one manifest of episodes and record the result.
//!
//! ```text
//! gordian-run --manifest FILE --out DIR
//! gordian-run init --run-id ID --policy POLICY --seed-start N --seed-count N --out FILE
//!                  [--experiment NAME] [--arm NAME] [--trace-sample-rate R]
//!                  [--components NAMES] [--every K] [--p P] [--patience-ns N]
//! ```
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
//! Exit status: 0 on success, 1 when the run failed, 2 on a usage error.

use gordian_run::manifest::{Manifest, PRIVILEGED};
use gordian_run::policy::PolicySpec;
use gordian_run::policy::decide::DecideConfig;
use gordian_run::recorder::execute;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage:
  gordian-run --manifest FILE --out DIR
  gordian-run init --run-id ID --policy POLICY --seed-start N --seed-count N --out FILE
                   [--experiment NAME] [--arm NAME] [--trace-sample-rate R]
                   [--components NAMES] [--every K] [--p P] [--patience-ns N]";

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
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let summary = execute(&manifest, &out).map_err(|e| e.to_string())?;
    eprintln!(
        "gordian-run: {} episodes, {} successes, {} undecided, {} sampled",
        summary.episodes, summary.successes, summary.undecided, summary.sampled
    );
    Ok(())
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
        ],
    )?;
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
    let manifest = Manifest::for_current_environment(
        required(&flags, "run-id")?,
        flags
            .get("experiment")
            .map_or("exploration", String::as_str),
        flags
            .get("arm")
            .map_or(default_arm.as_str(), String::as_str),
        &policy,
        decide,
        number(&flags, "seed-start")?,
        number(&flags, "seed-count")?,
        rate,
    )?;
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
