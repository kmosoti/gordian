//! Dump the hidden labels of a manifest's streams as JSON lines, for the R5 trace diagnostic
//! (`experiments/exploration/scripts/r5_trace.py`).
//!
//! ```text
//! cargo run --release -p gordian-run --example r5_labels -- --manifest FILE --first SEED --count N
//! ```
//!
//! One line per seed: the incidents of the stream (tier, hard family, criticality, onset,
//! deadline, every observation labelled with the incident and the decisive ones). It reads the
//! truth the way the stream evaluator does (`truth_from_stream`), which is the only reason this
//! file may: it is analysis tooling that runs after a run, never linked into an arm or the
//! harness, and its output is read by one analysis script and by nothing that plays a stream.
//! `scripts/check-no-oracle.sh` does not ban it (it names neither the oracle's path nor a
//! policy file), and the arm files' guard (`tests/stream_arms.rs`) covers `src/stream/arms/` only.

use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::score::family_name;
use gordian_stream::{Tier, generate};
use gordian_stream_eval::truth_from_stream;
use serde_json::json;
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: r5_labels --manifest FILE --first SEED --count N");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let (mut manifest, mut first, mut count) = (None, None, None);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--manifest" => manifest = Some(value),
            "--first" => first = value.parse::<u64>().ok(),
            "--count" => count = value.parse::<u64>().ok(),
            _ => return usage(),
        }
    }
    let (Some(manifest), Some(first), Some(count)) = (manifest, first, count) else {
        return usage();
    };
    let text = match std::fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("r5_labels: cannot read {manifest}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let manifest: StreamManifest = match serde_json::from_str(&text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("r5_labels: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for seed in first..first + count {
        let truth = truth_from_stream(&generate(&manifest.params_for(seed)));
        let incidents: Vec<_> = truth
            .incidents
            .iter()
            .map(|i| {
                let tier = match i.tier {
                    Tier::Plain => "plain",
                    Tier::Hard => "hard",
                    Tier::Decoy => "decoy",
                };
                let family = i.shape.hard_kind.map_or("", family_name);
                json!({
                    "id": i.id,
                    "tier": tier,
                    "family": family,
                    "critical": i.critical,
                    "onset_ns": i.onset_ns,
                    "deadline_ns": i.deadline_ns,
                    "decisive": i.decisive.iter().map(|o| o.0).collect::<Vec<_>>(),
                    "observations": i.observations.iter().map(|o| o.0).collect::<Vec<_>>(),
                })
            })
            .collect();
        let line = json!({ "seed": seed, "incidents": incidents });
        if writeln!(out, "{line}").is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
