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
//!
//! # One arm or several
//!
//! A manifest has either `arm` and `policy` (one arm, the layout every earlier manifest has) or
//! `arms`, a list of `{arm, policy}` (an interleaved run, work item A8): every arm shares the
//! seeds, classes, limits, episode parameters and decision rule, and each episode is played once
//! per arm, back to back, in an order drawn per episode from `run_seed` ([`crate::interleave`]).
//! Giving both forms is an error. In memory `arm` and `policy` always hold the first arm, so code
//! that reads them sees a valid arm either way; [`Manifest::arm_specs`] is the list to iterate.

use crate::harness::Limits;
use crate::policy::PolicySpec;
use crate::policy::decide::DecideConfig;
use gordian_core::Instant;
use gordian_world::{EpisodeClass, EpisodeSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

/// What the arm name of a privileged policy must contain, so that every output that carries the
/// arm says the arm used hidden state.
pub const PRIVILEGED: &str = "privileged";

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

/// Names an arm of an interleaved run may not take: files the run directory holds beside the arm
/// directories.
pub const RESERVED_ARM_NAMES: [&str; 3] = ["manifest.json", "drift.csv", "usage.json"];

/// The default number of episodes between two drift-control workloads (plan A8).
pub const DEFAULT_DRIFT_BLOCK: u32 = 50;

/// One arm of an interleaved run: its name and the policy it plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmSpec {
    /// The arm's name: the treatment this arm is. In an interleaved run it is also the name of
    /// the arm's subdirectory, so it uses the characters of a `run_id`.
    pub arm: String,
    /// The arm's policy and its parameters.
    pub policy: PolicySpec,
}

fn default_drift_block() -> u32 {
    DEFAULT_DRIFT_BLOCK
}

/// Everything that determines a run.
///
/// Serialized through a private form in which `arm` and `policy` are the one-arm spelling and
/// `arms` the several-arm one (see the module documentation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawManifest", into = "RawManifest")]
pub struct Manifest {
    /// Names the run directory and the cgroup. Letters, digits, `.`, `_` and `-` only.
    pub run_id: String,
    /// The experiment this run belongs to, for example `EXP-001` or `exploration`.
    pub experiment: String,
    /// The arm: which treatment this run is. In an interleaved run, the first arm of `arms`.
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
    /// The policy and its parameters. A bare id (`"heuristic_only"`) is the policy with its
    /// defaults, so manifests written before the baselines existed still parse; see
    /// [`PolicySpec`].
    pub policy: PolicySpec,
    /// The arms of an interleaved run, in the order listed (the order of play is drawn per
    /// episode, not this one). Empty for a one-arm manifest. When not empty, `arm` and `policy`
    /// equal the first entry; [`Manifest::validate`] checks it.
    pub arms: Vec<ArmSpec>,
    /// Seeds the per-episode draw of the arm order, together with the episode's seed and class
    /// ([`crate::interleave::arm_order`]). Unused by a one-arm manifest.
    pub run_seed: u64,
    /// The drift-control workload runs before the first episode, then before every
    /// `drift_block`-th episode after that, and once after the last ([`crate::drift`]). Counts
    /// `(seed, class)` units, not arm-episodes. At least 1.
    pub drift_block: u32,
    /// The parameters of the decision rule every non-privileged arm shares. Not part of the
    /// policy: arms may differ only in selection. Absent in older manifests, which get the
    /// defaults they were run with.
    #[serde(default)]
    pub decide: DecideConfig,
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

/// The serialized form of a [`Manifest`].
#[derive(Serialize, Deserialize)]
struct RawManifest {
    run_id: String,
    experiment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy: Option<PolicySpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arms: Vec<ArmSpec>,
    #[serde(default)]
    run_seed: u64,
    #[serde(default = "default_drift_block")]
    drift_block: u32,
    source_revision: String,
    lockfile_sha256: String,
    toolchain: String,
    cpu_flags: Vec<String>,
    isolation: IsolationSpec,
    seeds: Vec<u64>,
    episode_classes: Vec<(EpisodeClass, u32)>,
    #[serde(default)]
    decide: DecideConfig,
    limits: Limits,
    episode_params: EpisodeParams,
    trace_sample_rate: f64,
    internal_external_ratio: Option<RatioTolerance>,
}

impl From<Manifest> for RawManifest {
    fn from(m: Manifest) -> Self {
        let (arm, policy, arms) = if m.arms.is_empty() {
            (Some(m.arm), Some(m.policy), Vec::new())
        } else {
            (None, None, m.arms)
        };
        RawManifest {
            run_id: m.run_id,
            experiment: m.experiment,
            arm,
            policy,
            arms,
            run_seed: m.run_seed,
            drift_block: m.drift_block,
            source_revision: m.source_revision,
            lockfile_sha256: m.lockfile_sha256,
            toolchain: m.toolchain,
            cpu_flags: m.cpu_flags,
            isolation: m.isolation,
            seeds: m.seeds,
            episode_classes: m.episode_classes,
            decide: m.decide,
            limits: m.limits,
            episode_params: m.episode_params,
            trace_sample_rate: m.trace_sample_rate,
            internal_external_ratio: m.internal_external_ratio,
        }
    }
}

impl TryFrom<RawManifest> for Manifest {
    type Error = String;

    fn try_from(r: RawManifest) -> Result<Self, String> {
        let (arm, policy) = if let Some(first) = r.arms.first() {
            if r.arm.is_some() || r.policy.is_some() {
                return Err(
                    "a manifest gives either `arm` and `policy` or `arms`, not both".to_owned(),
                );
            }
            (first.arm.clone(), first.policy.clone())
        } else {
            match (r.arm, r.policy) {
                (Some(arm), Some(policy)) => (arm, policy),
                _ => {
                    return Err(
                        "a manifest needs `arm` and `policy`, or a non-empty `arms`".to_owned()
                    );
                }
            }
        };
        Ok(Manifest {
            run_id: r.run_id,
            experiment: r.experiment,
            arm,
            policy,
            arms: r.arms,
            run_seed: r.run_seed,
            drift_block: r.drift_block,
            source_revision: r.source_revision,
            lockfile_sha256: r.lockfile_sha256,
            toolchain: r.toolchain,
            cpu_flags: r.cpu_flags,
            isolation: r.isolation,
            seeds: r.seeds,
            episode_classes: r.episode_classes,
            decide: r.decide,
            limits: r.limits,
            episode_params: r.episode_params,
            trace_sample_rate: r.trace_sample_rate,
            internal_external_ratio: r.internal_external_ratio,
        })
    }
}

/// Whether `name` is a safe single path component made of a `run_id`'s characters.
fn safe_name(name: &str, max: usize) -> bool {
    !name.is_empty()
        && name.len() <= max
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && name != "."
        && name != ".."
}

/// An arm's policy must be valid, and a privileged one must say so in its name.
fn validate_arm(arm: &str, policy: &PolicySpec) -> Result<(), String> {
    policy.validate()?;
    if policy.is_privileged() && !arm.contains(PRIVILEGED) {
        return Err(format!(
            "arm {arm:?} runs the privileged policy {:?}; its arm name must contain {PRIVILEGED:?}",
            policy.id().0
        ));
    }
    Ok(())
}

impl Manifest {
    /// Whether the manifest lists its arms (an interleaved run, with one subdirectory per arm).
    pub fn is_interleaved(&self) -> bool {
        !self.arms.is_empty()
    }

    /// The arms in the order the manifest lists them: `arms`, or the one `arm` and `policy`.
    pub fn arm_specs(&self) -> Vec<ArmSpec> {
        if self.arms.is_empty() {
            vec![ArmSpec {
                arm: self.arm.clone(),
                policy: self.policy.clone(),
            }]
        } else {
            self.arms.clone()
        }
    }

    /// The one-arm manifest of arm `index`: the same run with only that arm, so that running it
    /// alone gives the arm's `results.csv` byte for byte (protocol replay is per arm). For an
    /// interleaved manifest its `run_id` is `<run_id>.<arm>`, which is what the arm's rows carry
    /// and what the arm's own `manifest.json` says. For a one-arm manifest, `index` must be 0
    /// and the manifest is returned as it is.
    ///
    /// # Panics
    ///
    /// If `index` is not an arm of the manifest.
    pub fn single_arm(&self, index: usize) -> Manifest {
        if self.arms.is_empty() {
            assert_eq!(index, 0, "a one-arm manifest has only arm 0");
            return self.clone();
        }
        let spec = &self.arms[index];
        Manifest {
            run_id: format!("{}.{}", self.run_id, spec.arm),
            arm: spec.arm.clone(),
            policy: spec.policy.clone(),
            arms: Vec::new(),
            ..self.clone()
        }
    }

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
        if self.drift_block == 0 {
            return Err("drift_block must be at least 1".to_owned());
        }
        if self.arms.is_empty() {
            validate_arm(&self.arm, &self.policy)?;
        } else {
            if self.arm != self.arms[0].arm || self.policy != self.arms[0].policy {
                return Err("`arm` and `policy` must equal the first of `arms`".to_owned());
            }
            let mut names = BTreeSet::new();
            for spec in &self.arms {
                // The derived run id `<run_id>.<arm>` must itself be a valid run id.
                if !safe_name(&spec.arm, 50) || self.run_id.len() + 1 + spec.arm.len() > 100 {
                    return Err(format!(
                        "arm name {:?} must be 1 to 50 of letters, digits, '.', '_', '-' and short enough that `<run_id>.<arm>` is at most 100: it names a directory and a run id",
                        spec.arm
                    ));
                }
                if RESERVED_ARM_NAMES.contains(&spec.arm.as_str()) {
                    return Err(format!(
                        "arm name {:?} is the name of a file in the run directory",
                        spec.arm
                    ));
                }
                if !names.insert(spec.arm.as_str()) {
                    return Err(format!("arm name {:?} is used twice", spec.arm));
                }
                validate_arm(&spec.arm, &spec.policy)?;
            }
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
    #[allow(clippy::too_many_arguments)]
    pub fn for_current_environment(
        run_id: &str,
        experiment: &str,
        arm: &str,
        policy: &PolicySpec,
        decide: DecideConfig,
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
            arms: Vec::new(),
            run_seed: 0,
            drift_block: DEFAULT_DRIFT_BLOCK,
            decide,
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
