//! The run manifest: everything that determines a run, and the environment it ran in.
//!
//! The charter (section 11) requires every experiment to record source revision, toolchain,
//! dependency lock, environment, hardware capabilities, data split, seeds, scheduler policy,
//! resource limits and result artifacts. The manifest is the input of `gordian-run` and is
//! copied canonically into the run directory as `manifest.json`.
//!
//! The environment fields (revision, lockfile hash, toolchain, CPU flags) are written *into* the
//! manifest before the run. `scripts/run-driver.sh` refuses a manifest whose revision is not the
//! current clean `HEAD`; the other three are recorded, not checked.

use crate::harness::Limits;
use crate::policy::PolicyId;
use gordian_core::Instant;
use gordian_world::{EpisodeClass, EpisodeSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

/// How the driver isolates the run: the arguments it passes to `scripts/cgroup-run.sh`, and the
/// wall-clock backstop it passes to `timeout`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolationSpec {
    /// cpuset, for example `0-2`.
    pub cpus: String,
    /// CFS quota in percent of one core, for example `300`.
    pub cpu_quota_percent: u32,
    /// Hard memory limit in bytes.
    pub memory_bytes: u64,
    /// Wall-clock backstop for `timeout(1)`, in seconds. The arm's own limits are what bound an
    /// episode; this only stops a run that has gone wrong.
    pub timeout_secs: u64,
}

impl Default for IsolationSpec {
    /// Cores 0-2, 300% CPU, 2 GB (the plan's one-arm ceiling), one hour.
    fn default() -> Self {
        Self {
            cpus: "0-2".to_owned(),
            cpu_quota_percent: 300,
            memory_bytes: 2 * 1024 * 1024 * 1024,
            timeout_secs: 3600,
        }
    }
}

/// The tolerance on `internal_external_ratio`, the measured nanoseconds the harness recorded
/// divided by the CPU nanoseconds the runner's controller reported.
///
/// `internal_external_ratio` in the manifest is `None` while the instrument is uncalibrated; the
/// first measured value is the starting point of the tolerance (`docs/local-test-plan.md`, A4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RatioTolerance {
    /// Smallest acceptable ratio.
    pub min: f64,
    /// Largest acceptable ratio.
    pub max: f64,
}

/// The episode-generator parameters the run fixes. The probe and time budget are not here: they
/// come from the limits (`Limits::world_budget`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeParams {
    /// End of every episode, nanoseconds of logical time.
    pub horizon_ns: u64,
    /// Irrelevant observations per true observation.
    pub noise_rate: u32,
    /// Fewest services in a world.
    pub min_services: u8,
    /// Most services in a world.
    pub max_services: u8,
    /// For `DelayedConfigChange`: observations between the snapshot and the first symptom.
    pub delay_k: u32,
}

impl Default for EpisodeParams {
    /// The world's own defaults (`EpisodeSpec::new`), written out so the manifest records them.
    fn default() -> Self {
        let base = EpisodeSpec::new(0, EpisodeClass::Ambiguous);
        Self {
            horizon_ns: base.horizon.0,
            noise_rate: base.noise_rate,
            min_services: base.min_services,
            max_services: base.max_services,
            delay_k: base.delay_k,
        }
    }
}

/// Everything that determines a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Names the run directory and the cgroup. Letters, digits, `.`, `_` and `-` only.
    pub run_id: String,
    /// The experiment this run belongs to, for example `EXP-001` or `exploration`.
    pub experiment: String,
    /// The arm: which treatment this run is.
    pub arm: String,
    /// `git rev-parse HEAD` at manifest time.
    pub source_revision: String,
    /// SHA-256 of `Cargo.lock`.
    pub lockfile_sha256: String,
    /// `rustc -V`.
    pub toolchain: String,
    /// The `flags` line of the first processor in `/proc/cpuinfo`.
    pub cpu_flags: Vec<String>,
    /// How the driver isolates the run.
    pub isolation: IsolationSpec,
    /// The seeds, in order. Class `c` with count `n` runs the first `n` seeds.
    pub seeds: Vec<u64>,
    /// The classes and how many of the seeds each runs, in the order the run executes them.
    pub episode_classes: Vec<(EpisodeClass, u32)>,
    /// The policy.
    pub policy: PolicyId,
    /// Hard limits and loop parameters.
    pub limits: Limits,
    /// Episode-generator parameters.
    pub episode_params: EpisodeParams,
    /// Fraction of episodes whose ledger is kept in `events-sample.jsonl`, chosen by a hash of
    /// (seed, class) alone, so every arm of an experiment samples the same episodes.
    pub trace_sample_rate: f64,
    /// Tolerance on `internal_external_ratio`, or `None` while uncalibrated.
    pub internal_external_ratio: Option<RatioTolerance>,
}

impl Manifest {
    /// Check that the manifest can be run. Does not check that the policy is known; the recorder
    /// does.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.run_id.is_empty()
            && self.run_id.len() <= 100
            && self
                .run_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            && self.run_id != "."
            && self.run_id != "..";
        if !id_ok {
            return Err(format!(
                "run_id {:?} must be 1 to 100 of letters, digits, '.', '_', '-'",
                self.run_id
            ));
        }
        if self.seeds.is_empty() {
            return Err("seeds is empty".to_owned());
        }
        let unique: BTreeSet<u64> = self.seeds.iter().copied().collect();
        if unique.len() != self.seeds.len() {
            return Err("seeds contains a duplicate".to_owned());
        }
        if self.episode_classes.is_empty() {
            return Err("episode_classes is empty".to_owned());
        }
        let classes: BTreeSet<EpisodeClass> = self.episode_classes.iter().map(|c| c.0).collect();
        if classes.len() != self.episode_classes.len() {
            return Err("episode_classes names a class twice".to_owned());
        }
        for (class, count) in &self.episode_classes {
            if *count == 0 || *count as usize > self.seeds.len() {
                return Err(format!(
                    "class {class:?} count {count} must be between 1 and the {} seeds",
                    self.seeds.len()
                ));
            }
        }
        if !(self.trace_sample_rate.is_finite() && (0.0..=1.0).contains(&self.trace_sample_rate)) {
            return Err("trace_sample_rate must be in [0, 1]".to_owned());
        }
        if let Some(t) = &self.internal_external_ratio
            && !(t.min.is_finite() && t.max.is_finite() && 0.0 <= t.min && t.min <= t.max)
        {
            return Err("internal_external_ratio needs finite 0 <= min <= max".to_owned());
        }
        let iso = &self.isolation;
        if iso.cpus.is_empty()
            || !iso
                .cpus
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ',' | '-'))
        {
            return Err(format!(
                "isolation.cpus {:?} is not a cpuset list",
                iso.cpus
            ));
        }
        if iso.cpu_quota_percent == 0 || iso.memory_bytes == 0 || iso.timeout_secs == 0 {
            return Err("isolation needs a positive quota, memory and timeout".to_owned());
        }
        self.limits.validate().map_err(|e| e.to_string())
    }

    /// The episode spec for `(seed, class)`: the manifest's generator parameters, with the
    /// world budget taken from the limits.
    pub fn spec_for(&self, seed: u64, class: EpisodeClass) -> EpisodeSpec {
        let p = &self.episode_params;
        let mut spec = EpisodeSpec::new(seed, class);
        spec.horizon = Instant(p.horizon_ns);
        spec.noise_rate = p.noise_rate;
        spec.min_services = p.min_services;
        spec.max_services = p.max_services;
        spec.delay_k = p.delay_k;
        spec.budget = self.limits.world_budget();
        spec
    }

    /// Every `(seed, class)` of the run, in execution order: classes as listed, each over the
    /// first `count` seeds as listed. This is also the row order of the results files.
    pub fn episodes(&self) -> Vec<(u64, EpisodeClass)> {
        self.episode_classes
            .iter()
            .flat_map(|(class, count)| {
                self.seeds
                    .iter()
                    .take(*count as usize)
                    .map(move |seed| (*seed, *class))
            })
            .collect()
    }

    /// The canonical text of the manifest: pretty JSON and a final newline. Deterministic.
    pub fn canonical_json(&self) -> String {
        let mut text =
            serde_json::to_string_pretty(self).expect("a manifest serializes: no map keys, no NaN");
        text.push('\n');
        text
    }

    /// A manifest for `policy` over `seed_count` seeds starting at `seed_start`, running every
    /// episode class on all of them, with the environment captured from the current directory.
    pub fn for_current_environment(
        run_id: &str,
        experiment: &str,
        arm: &str,
        policy: &PolicyId,
        seed_start: u64,
        seed_count: u32,
        trace_sample_rate: f64,
    ) -> Result<Manifest, String> {
        let env = Environment::capture(Path::new("."))?;
        let manifest = Manifest {
            run_id: run_id.to_owned(),
            experiment: experiment.to_owned(),
            arm: arm.to_owned(),
            source_revision: env.source_revision,
            lockfile_sha256: env.lockfile_sha256,
            toolchain: env.toolchain,
            cpu_flags: env.cpu_flags,
            isolation: IsolationSpec::default(),
            seeds: (seed_start..seed_start + u64::from(seed_count)).collect(),
            episode_classes: EpisodeClass::ALL
                .iter()
                .map(|class| (*class, seed_count))
                .collect(),
            policy: policy.clone(),
            limits: Limits::default(),
            episode_params: EpisodeParams::default(),
            trace_sample_rate,
            internal_external_ratio: None,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

/// The parts of a manifest that describe the machine and the source, captured at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// `git rev-parse HEAD`.
    pub source_revision: String,
    /// SHA-256 of `Cargo.lock`, from `sha256sum`.
    pub lockfile_sha256: String,
    /// `rustc -V`.
    pub toolchain: String,
    /// CPU feature flags from `/proc/cpuinfo`.
    pub cpu_flags: Vec<String>,
}

fn command_output(program: &str, args: &[&str], dir: &Path) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!("{program} {args:?} failed: {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

impl Environment {
    /// Capture the environment of the repository at `dir`. Reads `git`, `sha256sum`, `rustc` and
    /// `/proc/cpuinfo`; these are boundary effects, recorded once in the manifest.
    pub fn capture(dir: &Path) -> Result<Environment, String> {
        let source_revision = command_output("git", &["rev-parse", "HEAD"], dir)?;
        let sum = command_output("sha256sum", &["Cargo.lock"], dir)?;
        let lockfile_sha256 = sum
            .split_whitespace()
            .next()
            .ok_or("sha256sum printed nothing")?
            .to_owned();
        let toolchain = command_output("rustc", &["-V"], dir)?;
        let cpuinfo =
            fs::read_to_string("/proc/cpuinfo").map_err(|e| format!("/proc/cpuinfo: {e}"))?;
        let cpu_flags = cpuinfo
            .lines()
            .find(|l| l.starts_with("flags"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, flags)| flags.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default();
        Ok(Environment {
            source_revision,
            lockfile_sha256,
            toolchain,
            cpu_flags,
        })
    }
}
