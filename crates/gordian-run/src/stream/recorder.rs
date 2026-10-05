//! Running a stream manifest and writing its record.
//!
//! [`execute_stream`] plays every seed of a [`StreamManifest`], one segment at a time, each
//! segment once per arm, back to back, in an order drawn per segment
//! ([`crate::interleave::arm_order_keyed`], as plan A8 does for episodes), so that machine drift
//! falls on every arm alike. Each arm writes its own subdirectory `<out>/<arm>/` holding its
//! one-arm `manifest.json`, `results.csv`, `measured.csv` (with `arm_position`) and
//! `events-sample.jsonl`; `<out>/manifest.json` is the whole manifest and `<out>/drift.csv` holds
//! the drift-control timings ([`crate::drift`], reused unchanged: the workload runs before the
//! first segment, before every `drift_block`-th one, and once after the last, touches no arm and
//! charges none). The layout is the interleaved episode layout, so `scripts/run-driver.sh` runs it
//! unchanged.
//!
//! Refuses to start if a results file already exists, so a recorded run is never overwritten. A
//! harness defect aborts the run and no results file is written. Every segment produces a row for
//! every arm, including step-capped ones; nothing is excluded.
//!
//! # The events sample
//!
//! The segments whose ledgers are kept are chosen by a hash of the seed and `trace_sample_rate`
//! alone, so every arm keeps the same ones. For each sampled segment the file holds one `segment`
//! line (the seed and the public information the arm was given), one `public_stream` line (the
//! passive observations) and one `entry` line per ledger entry. It holds no verdict, no truth and
//! no call record; a test checks that. Ledger entries are not the scorer's input and say nothing
//! about whether any declaration was right.

use super::harness::{StreamHarnessError, run_segment, run_segment_privileged};
use super::manifest::StreamManifest;
use super::results::{MEASURED_HEADER, measured_row, results_header, results_row};
use super::score::StreamScorer;
use super::spec::{build_public, privileged_factory};
use crate::drift::{DRIFT_HEADER, Workload, drift_row};
use crate::interleave::arm_order_keyed;
use crate::recorder::{hex, phase_json, sampled_keyed};
use gordian_core::{Entry, EntryKind, decode_accounting};
use serde_json::{Value, json};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// The key that separates the stream harness's arm-order and sampling draws from every episode
/// harness draw: the ASCII of `stream`.
pub const STREAM_KEY: u64 = 0x0000_7374_7265_616d;

/// Why a stream run did not complete. A run that fails writes no results files.
#[derive(Debug)]
pub enum StreamRunError {
    /// The manifest cannot be run.
    Manifest(String),
    /// Reading or writing a file failed, or a run file already exists.
    Io(String),
    /// The harness reported a defect while playing a segment. The run stops: a defect is not a
    /// result.
    Harness {
        /// The arm that was playing.
        arm: String,
        /// The stream's seed.
        seed: u64,
        /// What went wrong.
        error: StreamHarnessError,
    },
}

impl fmt::Display for StreamRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamRunError::Manifest(why) => write!(f, "invalid manifest: {why}"),
            StreamRunError::Io(why) => write!(f, "{why}"),
            StreamRunError::Harness { arm, seed, error } => {
                write!(f, "harness defect in arm {arm:?}, seed {seed}: {error}")
            }
        }
    }
}

impl std::error::Error for StreamRunError {}

fn io_error(what: &str, path: &Path, e: std::io::Error) -> StreamRunError {
    StreamRunError::Io(format!("{what} {}: {e}", path.display()))
}

/// Counts of what one arm of a run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamRunSummary {
    /// Segments played.
    pub segments: usize,
    /// Segments whose ledger went to the events sample.
    pub sampled: usize,
    /// Segments the step cap stopped.
    pub step_capped: usize,
    /// Reasoner calls accepted, over all segments.
    pub reasoner_calls: u64,
    /// Declarations recorded, over all segments.
    pub declarations: u64,
}

impl StreamRunSummary {
    fn add(&mut self, other: &StreamRunSummary) {
        self.segments += other.segments;
        self.sampled += other.sampled;
        self.step_capped += other.step_capped;
        self.reasoner_calls += other.reasoner_calls;
        self.declarations += other.declarations;
    }
}

/// What a stream run did: the totals over arms, each arm's own counts in the manifest's order, and
/// how many drift blocks ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRunReport {
    /// The sum over arms: `segments` counts arm-segments.
    pub total: StreamRunSummary,
    /// Each arm's name and counts.
    pub arms: Vec<(String, StreamRunSummary)>,
    /// Rows written to `drift.csv`.
    pub drift_blocks: usize,
}

/// An entry as one JSON line.
fn entry_line(seed: u64, entry: &Entry) -> Value {
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
        "seed": seed,
        "id": entry.id.0,
        "at_ns": entry.at.0,
        "kind": format!("{:?}", entry.kind),
        "producer": entry.provenance.producer,
        "producer_version": entry.provenance.producer_version,
        "inputs": entry.provenance.inputs.iter().map(|i| i.0).collect::<Vec<_>>(),
        "payload": body,
    })
}

/// Write the sample lines of one segment.
fn write_sample(
    out: &mut impl Write,
    run_id: &str,
    record: &super::harness::SegmentRecord,
) -> Result<(), String> {
    let mut put = |value: Value| -> Result<(), String> {
        serde_json::to_writer(&mut *out, &value).map_err(|e| e.to_string())?;
        out.write_all(b"\n").map_err(|e| e.to_string())
    };
    put(json!({
        "record": "segment",
        "run_id": run_id,
        "seed": record.seed,
        "public_info": serde_json::to_value(&record.public).map_err(|e| e.to_string())?,
    }))?;
    put(json!({
        "record": "public_stream",
        "seed": record.seed,
        "observations": record.public_stream.iter().map(|(at, observation)| json!({
            "at_ns": at.0,
            "observation": observation,
        })).collect::<Vec<_>>(),
    }))?;
    for entry in record.ledger.iter() {
        put(entry_line(record.seed, entry))?;
    }
    Ok(())
}

/// One arm's output while the run is in progress.
struct ArmOut {
    name: String,
    run_id: String,
    results_path: PathBuf,
    measured_path: PathBuf,
    events_path: PathBuf,
    results: String,
    measured: String,
    events: Option<BufWriter<File>>,
    summary: StreamRunSummary,
}

/// Write `manifest.json` into `dir`, or check that the one there is the same manifest.
fn write_manifest(dir: &Path, manifest: &StreamManifest) -> Result<(), StreamRunError> {
    let path = dir.join("manifest.json");
    match fs::read_to_string(&path) {
        Ok(existing) => match serde_json::from_str::<StreamManifest>(&existing) {
            Ok(m) if &m == manifest => Ok(()),
            _ => Err(StreamRunError::Io(format!(
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

/// Run every segment of `manifest` and write the run directory `out`, scoring each with `scorer`.
///
/// The harness's counterpart of [`crate::recorder::execute_report`]. Each segment is played once
/// per arm in the order [`arm_order_keyed`] draws, each time with a fresh arm, budget, bill and
/// ledger.
pub fn execute_stream(
    manifest: &StreamManifest,
    out: &Path,
    scorer: &dyn StreamScorer,
) -> Result<StreamRunReport, StreamRunError> {
    manifest.validate().map_err(StreamRunError::Manifest)?;
    fs::create_dir_all(out).map_err(|e| io_error("cannot create", out, e))?;
    let drift_path = out.join("drift.csv");
    if drift_path.exists() {
        return Err(StreamRunError::Io(format!(
            "{} already exists; a recorded run is never overwritten",
            drift_path.display()
        )));
    }

    let mut arms: Vec<ArmOut> = Vec::with_capacity(manifest.arms.len());
    for (index, spec) in manifest.arms.iter().enumerate() {
        let single = manifest.single_arm(index);
        let dir = out.join(&spec.arm);
        fs::create_dir_all(&dir).map_err(|e| io_error("cannot create", &dir, e))?;
        let arm = ArmOut {
            name: spec.arm.clone(),
            run_id: single.run_id.clone(),
            results_path: dir.join("results.csv"),
            measured_path: dir.join("measured.csv"),
            events_path: dir.join("events-sample.jsonl"),
            results: format!("{}\n", results_header()),
            measured: format!("{MEASURED_HEADER}\n"),
            events: None,
            summary: StreamRunSummary::default(),
        };
        for path in [&arm.results_path, &arm.measured_path, &arm.events_path] {
            if path.exists() {
                return Err(StreamRunError::Io(format!(
                    "{} already exists; a recorded run is never overwritten",
                    path.display()
                )));
            }
        }
        write_manifest(&dir, &single)?;
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

    for (done, seed) in manifest.seeds.iter().copied().enumerate() {
        if (done as u64).is_multiple_of(block_every) {
            let sample = workload.run_block(drift_blocks, done as u64);
            drift.push_str(&drift_row(&manifest.run_id, &sample));
            drift.push('\n');
            drift_blocks += 1;
        }
        let params = manifest.params_for(seed);
        let order = arm_order_keyed(manifest.run_seed, seed, STREAM_KEY, arms.len());
        for (position, index) in order.iter().copied().enumerate() {
            let spec = &manifest.arms[index];
            let arm = &mut arms[index];
            let record = match privileged_factory(&spec.policy, &manifest.rung) {
                Some(factory) => run_segment_privileged(
                    &params,
                    &factory,
                    &manifest.limits,
                    &manifest.exchange,
                    scorer,
                ),
                None => run_segment(
                    &params,
                    &|public| {
                        build_public(&spec.policy, &manifest.rung, public, &spec.arm, seed)
                            .expect("a policy that is not privileged builds a public arm")
                    },
                    &manifest.limits,
                    &manifest.exchange,
                    scorer,
                ),
            }
            .map_err(|error| StreamRunError::Harness {
                arm: arm.name.clone(),
                seed,
                error,
            })?;
            arm.results.push_str(&results_row(&arm.run_id, &record));
            arm.results.push('\n');
            arm.measured
                .push_str(&measured_row(&arm.run_id, &record, position));
            arm.measured.push('\n');
            arm.summary.segments += 1;
            arm.summary.step_capped +=
                usize::from(record.stop == super::harness::StreamStop::StepCap);
            arm.summary.reasoner_calls += u64::from(record.verdict.reasoner_calls);
            arm.summary.declarations += u64::from(record.verdict.declarations);
            if let Some(events) = arm.events.as_mut()
                && sampled_keyed(seed, STREAM_KEY, manifest.trace_sample_rate)
            {
                write_sample(events, &arm.run_id, &record).map_err(|e| {
                    StreamRunError::Io(format!("{}: {e}", arm.events_path.display()))
                })?;
                arm.summary.sampled += 1;
            }
        }
    }
    // The closing block, so that the last timing is taken after the last segment.
    let sample = workload.run_block(drift_blocks, manifest.seeds.len() as u64);
    drift.push_str(&drift_row(&manifest.run_id, &sample));
    drift.push('\n');
    drift_blocks += 1;

    let mut total = StreamRunSummary::default();
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
    Ok(StreamRunReport {
        total,
        arms: summaries,
        drift_blocks: drift_blocks as usize,
    })
}
