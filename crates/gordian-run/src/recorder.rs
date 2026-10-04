//! Running a manifest and writing its record.
//!
//! [`execute`] runs every `(seed, class)` of a manifest, one episode at a time, and writes into
//! the run directory:
//!
//! - `manifest.json`, the canonical copy of the manifest;
//! - `results.csv` and `measured.csv` (see [`crate::results`]);
//! - `events-sample.jsonl`, when `trace_sample_rate` is above zero.
//!
//! `usage.json` is not written here. The runner (`scripts/cgroup-run.sh`) writes it after this
//! process exits, and the driver adds `internal_external_ratio` to it.
//!
//! # The events sample
//!
//! The episodes whose ledgers are kept are chosen by [`sampled`], a hash of `(seed, class)` and
//! the rate alone. The choice does not depend on the policy or on anything that happened, so
//! every arm of an experiment keeps the same episodes. For each sampled episode the file holds
//! one `episode` line (key and the public information the policy was given), one `public_stream`
//! line (the passive observation stream), and one `entry` line per ledger entry. It holds no
//! verdict and no hidden state: the verdict is in `results.csv`, and the hidden state is never
//! in anything the harness serializes. The `class` on the lines is the join key with the CSV
//! files, not a policy input; a pipeline that trains on the file must drop it.
//!
//! `harness/timer` entries in the sample are wall-clock timings and differ between runs.

use crate::harness::{
    EpisodeRecord, HarnessError, run_episode, run_episode_privileged, standard_components,
};
use crate::manifest::Manifest;
use crate::policy::{self, Built, Policy, PolicyId};
use crate::results::{MEASURED_HEADER, RESULTS_HEADER, class_name, measured_row, results_row};
use gordian_core::{Entry, EntryKind, Phase, decode_accounting};
use gordian_world::EpisodeClass;
use serde_json::{Value, json};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Why a run did not complete. A run that fails writes no results files.
#[derive(Debug)]
pub enum RunError {
    /// The manifest cannot be run.
    Manifest(String),
    /// No policy has the manifest's policy id.
    UnknownPolicy(PolicyId),
    /// Reading or writing a file failed, or a run file already exists.
    Io(String),
    /// The harness reported a defect while playing an episode. The run stops: a defect is not a
    /// result.
    Harness {
        /// The episode's seed.
        seed: u64,
        /// The episode's class.
        class: EpisodeClass,
        /// What went wrong.
        error: HarnessError,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Manifest(why) => write!(f, "invalid manifest: {why}"),
            RunError::UnknownPolicy(id) => {
                write!(f, "unknown policy {:?}; known: {:?}", id.0, policy::KNOWN)
            }
            RunError::Io(why) => write!(f, "{why}"),
            RunError::Harness { seed, class, error } => {
                write!(f, "harness defect in seed {seed} class {class:?}: {error}")
            }
        }
    }
}

impl std::error::Error for RunError {}

fn io_error(what: &str, path: &Path, e: std::io::Error) -> RunError {
    RunError::Io(format!("{what} {}: {e}", path.display()))
}

/// Counts of what a run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    /// Episodes played.
    pub episodes: usize,
    /// Episodes whose ledger went to the events sample.
    pub sampled: usize,
    /// Episodes the arm never decided.
    pub undecided: usize,
    /// Episodes scored as successes.
    pub successes: usize,
}

/// splitmix64 finalizer.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// FNV-1a over the class's name, so the hash does not depend on enum order.
fn class_hash(class: EpisodeClass) -> u64 {
    class_name(class)
        .bytes()
        .fold(0xCBF2_9CE4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
        })
}

/// Whether the ledger of `(seed, class)` is kept at `rate`. A pure function of its arguments:
/// the same episodes are kept in every arm. A rate of 0 keeps none and 1 keeps all.
pub fn sampled(seed: u64, class: EpisodeClass, rate: f64) -> bool {
    if rate <= 0.0 {
        return false;
    }
    if rate >= 1.0 {
        return true;
    }
    let h = mix(mix(seed) ^ class_hash(class));
    // The top 53 bits as a fraction of 2^53.
    ((h >> 11) as f64) / ((1u64 << 53) as f64) < rate
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn phase_json(phase: Phase) -> Value {
    match phase {
        Phase::Component(id) => json!({ "phase": "Component", "component": id.0 }),
        other => json!({ "phase": format!("{other:?}") }),
    }
}

/// An entry as one JSON line. Accounting payloads are decoded, JSON payloads embedded, anything
/// else written as hex.
fn entry_line(record: &EpisodeRecord, entry: &Entry) -> Value {
    let body = if entry.kind == EntryKind::Accounting {
        match decode_accounting(&entry.payload) {
            Ok(a) => json!({
                "phase": phase_json(a.phase),
                "accepted": a.accepted,
                "charges": a.charges.iter().map(|c| json!({
                    "resource": format!("{:?}", c.resource),
                    "amount": c.amount,
                })).collect::<Vec<_>>(),
            }),
            Err(_) => json!({ "hex": hex(&entry.payload) }),
        }
    } else {
        serde_json::from_slice::<Value>(&entry.payload)
            .unwrap_or_else(|_| json!({ "hex": hex(&entry.payload) }))
    };
    json!({
        "record": "entry",
        "seed": record.seed,
        "class": class_name(record.class),
        "id": entry.id.0,
        "at_ns": entry.at.0,
        "kind": format!("{:?}", entry.kind),
        "producer": entry.provenance.producer,
        "producer_version": entry.provenance.producer_version,
        "inputs": entry.provenance.inputs.iter().map(|i| i.0).collect::<Vec<_>>(),
        "payload": body,
    })
}

/// Write the sample lines of one episode.
fn write_sample(out: &mut impl Write, run_id: &str, record: &EpisodeRecord) -> Result<(), String> {
    let mut put = |value: Value| -> Result<(), String> {
        serde_json::to_writer(&mut *out, &value).map_err(|e| e.to_string())?;
        out.write_all(b"\n").map_err(|e| e.to_string())
    };
    put(json!({
        "record": "episode",
        "run_id": run_id,
        "seed": record.seed,
        "class": class_name(record.class),
        "public_info": serde_json::to_value(&record.public_info).map_err(|e| e.to_string())?,
    }))?;
    put(json!({
        "record": "public_stream",
        "seed": record.seed,
        "class": class_name(record.class),
        "observations": record.public_stream.iter().map(|(at, observation)| json!({
            "at_ns": at.0,
            "observation": observation,
        })).collect::<Vec<_>>(),
    }))?;
    for entry in record.ledger.iter() {
        put(entry_line(record, entry))?;
    }
    Ok(())
}

/// Run every episode of `manifest` and write the run directory `out`.
///
/// Refuses to start if `out` already holds a results file, so a recorded run is never
/// overwritten. Episodes run sequentially, in `Manifest::episodes` order, each with a fresh
/// policy, fresh components, a fresh budget, bill and ledger. Every episode produces a row,
/// including undecided ones; nothing is excluded. A [`HarnessError`] aborts the run and no
/// results file is written.
pub fn execute(manifest: &Manifest, out: &Path) -> Result<RunSummary, RunError> {
    run_manifest(manifest, out, &Source::Registry)
}

/// [`execute`] with the policies supplied by `make_policy` instead of the registry
/// ([`policy::build`]). For tests and for arms that are not registered. `make_policy` is called
/// once per episode and must return a fresh policy each time. It cannot supply a privileged arm.
pub fn execute_with(
    manifest: &Manifest,
    out: &Path,
    make_policy: &dyn Fn(&PolicyId) -> Option<Box<dyn Policy>>,
) -> Result<RunSummary, RunError> {
    run_manifest(manifest, out, &Source::Custom(make_policy))
}

/// Where a run's policies come from.
enum Source<'a> {
    /// [`policy::build`], from the manifest's policy, decision rule and arm.
    Registry,
    /// The caller's function.
    Custom(&'a dyn Fn(&PolicyId) -> Option<Box<dyn Policy>>),
}

fn run_manifest(
    manifest: &Manifest,
    out: &Path,
    source: &Source<'_>,
) -> Result<RunSummary, RunError> {
    manifest.validate().map_err(RunError::Manifest)?;
    if let Source::Custom(make) = source
        && make(&manifest.policy.id()).is_none()
    {
        return Err(RunError::UnknownPolicy(manifest.policy.id()));
    }
    fs::create_dir_all(out).map_err(|e| io_error("cannot create", out, e))?;
    let results_path = out.join("results.csv");
    let measured_path = out.join("measured.csv");
    let events_path = out.join("events-sample.jsonl");
    for path in [&results_path, &measured_path, &events_path] {
        if path.exists() {
            return Err(RunError::Io(format!(
                "{} already exists; a recorded run is never overwritten",
                path.display()
            )));
        }
    }
    let manifest_path = out.join("manifest.json");
    match fs::read_to_string(&manifest_path) {
        Ok(existing) => match serde_json::from_str::<Manifest>(&existing) {
            Ok(m) if &m == manifest => {}
            _ => {
                return Err(RunError::Io(format!(
                    "{} exists and differs from the manifest being run",
                    manifest_path.display()
                )));
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&manifest_path, manifest.canonical_json())
                .map_err(|e| io_error("cannot write", &manifest_path, e))?;
        }
        Err(e) => return Err(io_error("cannot read", &manifest_path, e)),
    }

    let mut events = if manifest.trace_sample_rate > 0.0 {
        let file =
            File::create(&events_path).map_err(|e| io_error("cannot create", &events_path, e))?;
        Some(BufWriter::new(file))
    } else {
        None
    };

    let mut results = String::from(RESULTS_HEADER);
    results.push('\n');
    let mut measured = String::from(MEASURED_HEADER);
    measured.push('\n');
    let mut summary = RunSummary {
        episodes: 0,
        sampled: 0,
        undecided: 0,
        successes: 0,
    };

    for (seed, class) in manifest.episodes() {
        let spec = manifest.spec_for(seed, class);
        let built = match source {
            Source::Registry => {
                policy::build(&manifest.policy, &manifest.decide, &manifest.arm, seed)
            }
            Source::Custom(make) => match make(&manifest.policy.id()) {
                Some(policy) => Built::Public(policy),
                None => return Err(RunError::UnknownPolicy(manifest.policy.id())),
            },
        };
        let mut components = standard_components();
        let record = match built {
            Built::Public(mut policy) => {
                run_episode(&spec, policy.as_mut(), &mut components, &manifest.limits)
            }
            Built::Privileged(factory) => {
                run_episode_privileged(&spec, &factory, &mut components, &manifest.limits)
            }
        }
        .map_err(|error| RunError::Harness { seed, class, error })?;
        results.push_str(&results_row(&manifest.run_id, &record));
        results.push('\n');
        measured.push_str(&measured_row(&manifest.run_id, &record));
        measured.push('\n');
        summary.episodes += 1;
        summary.undecided += usize::from(record.verdict.undecided);
        summary.successes += usize::from(record.verdict.success);
        if let Some(events) = events.as_mut()
            && sampled(seed, class, manifest.trace_sample_rate)
        {
            write_sample(events, &manifest.run_id, &record)
                .map_err(|e| RunError::Io(format!("events-sample.jsonl: {e}")))?;
            summary.sampled += 1;
        }
    }

    if let Some(mut events) = events {
        events
            .flush()
            .map_err(|e| io_error("cannot write", &events_path, e))?;
    }
    fs::write(&results_path, results).map_err(|e| io_error("cannot write", &results_path, e))?;
    fs::write(&measured_path, measured).map_err(|e| io_error("cannot write", &measured_path, e))?;
    Ok(summary)
}
