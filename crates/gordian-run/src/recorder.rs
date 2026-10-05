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
//! # Interleaved runs (plan A8)
//!
//! A manifest with `arms` runs every arm in this one process. For each `(seed, class)` the arms
//! play the episode one after another in the order [`crate::interleave::arm_order`] draws, and
//! every arm writes its own files into its own subdirectory `<out>/<arm>/`: `results.csv`,
//! `measured.csv` (with `arm_position`), `events-sample.jsonl`, and a `manifest.json` that is the
//! arm's one-arm manifest ([`Manifest::single_arm`]), so each arm directory is a one-arm run
//! directory and running that manifest alone reproduces the arm's `results.csv` byte for byte.
//! `<out>/manifest.json` is the whole manifest. A one-arm manifest writes its files straight into
//! `<out>` as before.
//!
//! Either way `<out>/drift.csv` holds the drift-control timings ([`crate::drift`]): the workload
//! runs before the first episode, before every `drift_block`-th one, and once after the last.
//! It touches no arm and no arm is charged for it.
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

use crate::drift::{DRIFT_HEADER, Workload, drift_row};
use crate::harness::{
    EpisodeRecord, HarnessError, run_episode, run_episode_privileged, standard_components,
};
use crate::interleave::arm_order;
use crate::manifest::{ArmSpec, Manifest};
use crate::policy::{self, Built, Policy, PolicyId};
use crate::results::{MEASURED_HEADER, RESULTS_HEADER, class_name, measured_row, results_row};
use gordian_core::{Entry, EntryKind, Phase, decode_accounting};
use gordian_world::EpisodeClass;
use serde_json::{Value, json};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

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
        /// The arm that was playing.
        arm: String,
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
            RunError::Harness {
                arm,
                seed,
                class,
                error,
            } => {
                write!(
                    f,
                    "harness defect in arm {arm:?}, seed {seed} class {class:?}: {error}"
                )
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

impl RunSummary {
    const ZERO: RunSummary = RunSummary {
        episodes: 0,
        sampled: 0,
        undecided: 0,
        successes: 0,
    };

    fn add(&mut self, other: &RunSummary) {
        self.episodes += other.episodes;
        self.sampled += other.sampled;
        self.undecided += other.undecided;
        self.successes += other.successes;
    }
}

/// What a run did: the totals over every arm, each arm's own counts in the manifest's order, and
/// how many drift blocks ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// The sum over arms: `episodes` counts arm-episodes.
    pub total: RunSummary,
    /// Each arm's name and counts, in the order the manifest lists the arms.
    pub arms: Vec<(String, RunSummary)>,
    /// Rows written to `drift.csv`.
    pub drift_blocks: usize,
}

/// splitmix64 finalizer.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// FNV-1a over the class's name, so the hash does not depend on enum order.
pub(crate) fn class_hash(class: EpisodeClass) -> u64 {
    class_name(class)
        .bytes()
        .fold(0xCBF2_9CE4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
        })
}

/// Whether the ledger of `(seed, class)` is kept at `rate`. A pure function of its arguments:
/// the same episodes are kept in every arm. A rate of 0 keeps none and 1 keeps all.
pub fn sampled(seed: u64, class: EpisodeClass, rate: f64) -> bool {
    sampled_keyed(seed, class_hash(class), rate)
}

/// [`sampled`] with the class replaced by any 64-bit key, for a world with no episode classes
/// (the stream harness).
pub fn sampled_keyed(seed: u64, key: u64, rate: f64) -> bool {
    if rate <= 0.0 {
        return false;
    }
    if rate >= 1.0 {
        return true;
    }
    let h = mix(mix(seed) ^ key);
    // The top 53 bits as a fraction of 2^53.
    ((h >> 11) as f64) / ((1u64 << 53) as f64) < rate
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn phase_json(phase: Phase) -> Value {
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
/// Refuses to start if `out` (or, for an interleaved manifest, an arm's directory) already holds
/// a results file, so a recorded run is never overwritten. Episodes run sequentially, in
/// `Manifest::episodes` order; each is played once per arm, in the order
/// [`crate::interleave::arm_order`] draws, each time with a fresh policy, fresh components, a
/// fresh budget, bill and ledger. Every episode produces a row for every arm, including
/// undecided ones; nothing is excluded. A [`HarnessError`] aborts the run and no results file is
/// written. The returned summary is the total over arms; [`execute_report`] has each arm's.
pub fn execute(manifest: &Manifest, out: &Path) -> Result<RunSummary, RunError> {
    run_manifest(manifest, out, &Source::Registry).map(|report| report.total)
}

/// [`execute`], returning each arm's counts and the number of drift blocks as well.
pub fn execute_report(manifest: &Manifest, out: &Path) -> Result<RunReport, RunError> {
    run_manifest(manifest, out, &Source::Registry)
}

/// [`execute`] with the policies supplied by `make_policy` instead of the registry
/// ([`policy::build`]). For tests and for arms that are not registered. `make_policy` is called
/// once per episode and arm, with the arm's policy id, and must return a fresh policy each time.
/// It cannot supply a privileged arm.
pub fn execute_with(
    manifest: &Manifest,
    out: &Path,
    make_policy: &dyn Fn(&PolicyId) -> Option<Box<dyn Policy>>,
) -> Result<RunSummary, RunError> {
    run_manifest(manifest, out, &Source::Custom(make_policy)).map(|report| report.total)
}

/// Where a run's policies come from.
enum Source<'a> {
    /// [`policy::build`], from the manifest's policy, decision rule and arm.
    Registry,
    /// The caller's function.
    Custom(&'a dyn Fn(&PolicyId) -> Option<Box<dyn Policy>>),
}

/// Write `manifest.json` into `dir`, or check that the one there is the same manifest.
fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), RunError> {
    let path = dir.join("manifest.json");
    match fs::read_to_string(&path) {
        Ok(existing) => match serde_json::from_str::<Manifest>(&existing) {
            Ok(m) if &m == manifest => Ok(()),
            _ => Err(RunError::Io(format!(
                "{} exists and differs from the manifest being run",
                path.display()
            ))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, manifest.canonical_json())
                .map_err(|e| io_error("cannot write", &path, e))
        }
        Err(e) => Err(io_error("cannot read", &path, e)),
    }
}

/// One arm's output while the run is in progress.
struct ArmOut {
    name: String,
    spec: ArmSpec,
    /// The run id its rows carry.
    run_id: String,
    results_path: PathBuf,
    measured_path: PathBuf,
    events_path: PathBuf,
    results: String,
    measured: String,
    events: Option<BufWriter<File>>,
    summary: RunSummary,
}

fn run_manifest(
    manifest: &Manifest,
    out: &Path,
    source: &Source<'_>,
) -> Result<RunReport, RunError> {
    manifest.validate().map_err(RunError::Manifest)?;
    let specs = manifest.arm_specs();
    if let Source::Custom(make) = source {
        for spec in &specs {
            if make(&spec.policy.id()).is_none() {
                return Err(RunError::UnknownPolicy(spec.policy.id()));
            }
        }
    }
    fs::create_dir_all(out).map_err(|e| io_error("cannot create", out, e))?;
    let drift_path = out.join("drift.csv");
    if drift_path.exists() {
        return Err(RunError::Io(format!(
            "{} already exists; a recorded run is never overwritten",
            drift_path.display()
        )));
    }

    // One directory per arm in an interleaved run; the run directory itself for one arm.
    let mut arms: Vec<ArmOut> = Vec::with_capacity(specs.len());
    for (index, spec) in specs.iter().enumerate() {
        let single = manifest.single_arm(index);
        let dir = if manifest.is_interleaved() {
            out.join(&spec.arm)
        } else {
            out.to_path_buf()
        };
        fs::create_dir_all(&dir).map_err(|e| io_error("cannot create", &dir, e))?;
        let arm = ArmOut {
            name: spec.arm.clone(),
            spec: spec.clone(),
            run_id: single.run_id.clone(),
            results_path: dir.join("results.csv"),
            measured_path: dir.join("measured.csv"),
            events_path: dir.join("events-sample.jsonl"),
            results: format!("{RESULTS_HEADER}\n"),
            measured: format!("{MEASURED_HEADER}\n"),
            events: None,
            summary: RunSummary::ZERO,
        };
        for path in [&arm.results_path, &arm.measured_path, &arm.events_path] {
            if path.exists() {
                return Err(RunError::Io(format!(
                    "{} already exists; a recorded run is never overwritten",
                    path.display()
                )));
            }
        }
        if manifest.is_interleaved() {
            write_manifest(&dir, &single)?;
        }
        arms.push(arm);
    }
    write_manifest(out, manifest)?;
    if manifest.trace_sample_rate > 0.0 {
        for arm in &mut arms {
            let file = File::create(&arm.events_path)
                .map_err(|e| io_error("cannot create", &arm.events_path, e))?;
            arm.events = Some(BufWriter::new(file));
        }
    }

    let mut drift = format!("{DRIFT_HEADER}\n");
    let mut drift_blocks = 0u32;
    let mut workload = Workload::new();
    let block_every = u64::from(manifest.drift_block);

    let units = manifest.episodes();
    for (done, (seed, class)) in units.iter().copied().enumerate() {
        if (done as u64).is_multiple_of(block_every) {
            let sample = workload.run_block(drift_blocks, done as u64);
            drift.push_str(&drift_row(&manifest.run_id, &sample));
            drift.push('\n');
            drift_blocks += 1;
        }
        let spec = manifest.spec_for(seed, class);
        let order = arm_order(manifest.run_seed, seed, class, arms.len());
        for (position, index) in order.iter().copied().enumerate() {
            let arm = &mut arms[index];
            let built = match source {
                Source::Registry => {
                    policy::build(&arm.spec.policy, &manifest.decide, &arm.spec.arm, seed)
                }
                Source::Custom(make) => match make(&arm.spec.policy.id()) {
                    Some(policy) => Built::Public(policy),
                    None => return Err(RunError::UnknownPolicy(arm.spec.policy.id())),
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
            .map_err(|error| RunError::Harness {
                arm: arm.name.clone(),
                seed,
                class,
                error,
            })?;
            arm.results.push_str(&results_row(&arm.run_id, &record));
            arm.results.push('\n');
            arm.measured
                .push_str(&measured_row(&arm.run_id, &record, position));
            arm.measured.push('\n');
            arm.summary.episodes += 1;
            arm.summary.undecided += usize::from(record.verdict.undecided);
            arm.summary.successes += usize::from(record.verdict.success);
            if let Some(events) = arm.events.as_mut()
                && sampled(seed, class, manifest.trace_sample_rate)
            {
                write_sample(events, &arm.run_id, &record)
                    .map_err(|e| RunError::Io(format!("{}: {e}", arm.events_path.display())))?;
                arm.summary.sampled += 1;
            }
        }
    }
    // The closing block, so that the last timing is taken after the last episode.
    let sample = workload.run_block(drift_blocks, units.len() as u64);
    drift.push_str(&drift_row(&manifest.run_id, &sample));
    drift.push('\n');
    drift_blocks += 1;

    let mut total = RunSummary::ZERO;
    let mut summaries = Vec::with_capacity(arms.len());
    for mut arm in arms {
        if let Some(mut events) = arm.events.take() {
            events
                .flush()
                .map_err(|e| io_error("cannot write", &arm.events_path, e))?;
        }
        fs::write(&arm.results_path, &arm.results)
            .map_err(|e| io_error("cannot write", &arm.results_path, e))?;
        fs::write(&arm.measured_path, &arm.measured)
            .map_err(|e| io_error("cannot write", &arm.measured_path, e))?;
        total.add(&arm.summary);
        summaries.push((arm.name, arm.summary));
    }
    fs::write(&drift_path, drift).map_err(|e| io_error("cannot write", &drift_path, e))?;
    Ok(RunReport {
        total,
        arms: summaries,
        drift_blocks: drift_blocks as usize,
    })
}
