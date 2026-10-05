//! The stream manifest: everything that determines a stream run, and the environment it ran in.
//!
//! It is the stream's counterpart of [`crate::manifest::Manifest`] and follows the same rules
//! (charter section 11: source revision, toolchain, lockfile, environment, hardware capabilities,
//! seeds, arms, resource limits, result artifacts). Differences, all forced by the world:
//!
//! - the unit of replication is a **stream segment** ([`StreamManifest::seeds`], one segment per
//!   seed), not an `(seed, class)` episode. Its length is the stream's `duration_ns`; see
//!   `stream/mod.rs`, "Segments";
//! - the generator's parameters are a whole [`gordian_stream::StreamParams`] (with the seed
//!   replaced per segment), because the tier mix, the regime schedule and the reasoner's
//!   `(a, b, c)` are what a sweep varies and a run must record them;
//! - the limits ([`StreamLimits`]) are the stream's: substrate and rule compute, probes, probe
//!   time and reasoner tokens, with the reasoner's budget in tokens (its own unit) and the
//!   conversion to total cost in [`Exchange`];
//! - every arm is in `arms` (an interleaved run is the only kind), each carrying its role.
//!
//! The binary tells a stream manifest from an episode manifest by its `stream_params` key.

use super::arms::ArmRole;
use super::arms::rung::RungConfig;
use super::spec::StreamPolicySpec;
use crate::manifest::{Environment, IsolationSpec, RatioTolerance};
use gordian_core::{Budget, Resource};
use gordian_stream::{ReasonerCostSpec, StreamBudgetSpec, StreamParams};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// What the arm name of a privileged arm must contain.
pub const PRIVILEGED: &str = "privileged";

/// What the arm name of an ablation must contain.
pub const ABLATION: &str = "ablation";

/// Names an arm may not take: files the run directory holds beside the arm directories.
pub const RESERVED_ARM_NAMES: [&str; 3] = ["manifest.json", "drift.csv", "usage.json"];

/// The default number of segments between drift-control workloads (plan A8).
pub const DEFAULT_DRIFT_BLOCK: u32 = 50;

/// How the reasoner's cost, in its own unit (tokens), becomes total cost, in modelled
/// nanoseconds (charter sections 4 and 12). A preregistered parameter of the experiment, not a
/// property of the world: the stream declares a price per token, and the exchange rate used for
/// total cost is this one, so a sweep over rates does not touch the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exchange {
    /// Modelled nanoseconds of total cost per reasoner token.
    pub reasoner_ns_per_token: u64,
}

impl Default for Exchange {
    /// The stream's own declared price (250,000 ns a token, `DESIGN.md` section 11), so that by
    /// default the converted reasoner cost equals the world's declared one.
    fn default() -> Self {
        Self {
            reasoner_ns_per_token: 250_000,
        }
    }
}

/// The hard limits and loop parameters of one stream segment.
///
/// `probes` and `probe_time_ns` are also the stream's own budget and `reasoner_tokens` times the
/// stream's price per token is its reasoner budget; the harness refuses a stream whose budget
/// differs (`StreamHarnessError::BudgetMismatch`) and does not overwrite it silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamLimits {
    /// `Resource::Compute`: declared substrate and rule cost over the segment, nanoseconds
    /// (components, the shared rule, and the arm's declared bookkeeping). Not the reasoner's.
    pub compute: u64,
    /// `Resource::Memory` limit, bytes. Declared so a charge can be refused; nobody charges it.
    pub memory: u64,
    /// Probe units over the segment.
    pub probes: u64,
    /// Probe time over the segment, nanoseconds (`Resource::Time`).
    pub probe_time_ns: u64,
    /// The reasoner's budget over the segment in its own unit, tokens (`Resource::Communication`
    /// in the bill: tokens exchanged with the reasoner).
    pub reasoner_tokens: u64,
    /// Logical time one step takes at least, nanoseconds. At least 1.
    pub step_ns: u64,
    /// The runaway guard: the segment stops after this many steps.
    pub max_steps: u32,
}

impl StreamLimits {
    /// Limits that match `params`' own budget, with a substrate allowance of `compute`
    /// nanoseconds, steps of `step_ns`, and a step cap of twice the steps that fit in the
    /// stream plus slack.
    pub fn matching(params: &StreamParams, compute: u64, step_ns: u64) -> Self {
        let p = params.normalized();
        let step_ns = step_ns.max(1);
        let fit = p.duration_ns / step_ns + 1;
        Self {
            compute,
            memory: 1 << 30,
            probes: p.budget.probes,
            probe_time_ns: p.budget.probe_time_ns,
            reasoner_tokens: p
                .budget
                .reasoner_ns
                .checked_div(p.reasoner.cost.ns_per_token)
                .unwrap_or(0),
            step_ns,
            max_steps: u32::try_from(fit.saturating_mul(2).saturating_add(16)).unwrap_or(u32::MAX),
        }
    }

    /// A fresh budget with every limit declared and nothing spent. Build a new one for every
    /// segment: a bill over a budget that already has spend attributes none of it to a phase.
    pub fn budget(&self) -> Budget {
        Budget::new()
            .with_limit(Resource::Compute, self.compute)
            .with_limit(Resource::Memory, self.memory)
            .with_limit(Resource::Time, self.probe_time_ns)
            .with_limit(Resource::Probes, self.probes)
            .with_limit(Resource::Communication, self.reasoner_tokens)
    }

    /// The part of the limits the stream enforces itself, given the stream's declared price.
    pub fn world_budget(&self, cost: &ReasonerCostSpec) -> StreamBudgetSpec {
        StreamBudgetSpec {
            probes: self.probes,
            probe_time_ns: self.probe_time_ns,
            reasoner_ns: self.reasoner_tokens.saturating_mul(cost.ns_per_token),
        }
    }

    /// Check the parameters the loop relies on.
    pub fn validate(&self) -> Result<(), String> {
        if self.step_ns == 0 {
            return Err("limits.step_ns must be at least 1".to_owned());
        }
        if self.max_steps == 0 {
            return Err("limits.max_steps must be at least 1".to_owned());
        }
        Ok(())
    }
}

/// One arm of a stream run: its name and the policy it plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamArmSpec {
    /// The arm's name: the treatment this arm is. Also the name of its subdirectory.
    pub arm: String,
    /// The arm's policy and its parameters.
    pub policy: StreamPolicySpec,
}

/// Everything that determines a stream run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamManifest {
    /// Names the run directory and the cgroup. Letters, digits, `.`, `_` and `-` only.
    pub run_id: String,
    /// The experiment this run belongs to, for example `EXP-101` or `exploration`.
    pub experiment: String,
    /// The arms, in the order listed (the order of play is drawn per segment).
    pub arms: Vec<StreamArmSpec>,
    /// Seeds the per-segment draw of the arm order.
    pub run_seed: u64,
    /// The drift-control workload runs before the first segment, then before every
    /// `drift_block`-th one, and once after the last.
    pub drift_block: u32,
    /// `git rev-parse HEAD` at manifest time.
    pub source_revision: String,
    /// SHA-256 of `Cargo.lock`.
    pub lockfile_sha256: String,
    /// `rustc -V`.
    pub toolchain: String,
    /// The `flags` line of the first processor in `/proc/cpuinfo`.
    pub cpu_flags: Vec<String>,
    /// The `model name` line of the first processor, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,
    /// The `cpu MHz` line of the first processor, read once at manifest time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_mhz: Option<f64>,
    /// How the driver isolates the run.
    pub isolation: IsolationSpec,
    /// The stream seeds, one segment each, in the order the run plays them.
    pub seeds: Vec<u64>,
    /// The generator's parameters. `seed` is replaced by each segment's.
    pub stream_params: StreamParams,
    /// The cheap rung every arm shares.
    pub rung: RungConfig,
    /// Hard limits and loop parameters.
    pub limits: StreamLimits,
    /// How the reasoner's cost becomes total cost.
    pub exchange: Exchange,
    /// Fraction of segments whose ledger is kept in `events-sample.jsonl`, chosen by a hash of
    /// the seed alone, so every arm of an experiment samples the same segments.
    pub trace_sample_rate: f64,
    /// Tolerance on `internal_external_ratio`, or `None` while uncalibrated.
    pub internal_external_ratio: Option<RatioTolerance>,
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

/// An arm's policy must be valid, and a privileged or ablation one must say so in its name.
fn validate_arm(arm: &str, policy: &StreamPolicySpec) -> Result<(), String> {
    policy.validate()?;
    match policy.role() {
        ArmRole::Privileged if !arm.contains(PRIVILEGED) => Err(format!(
            "arm {arm:?} runs the privileged policy {:?}; its arm name must contain {PRIVILEGED:?}",
            policy.id().0
        )),
        ArmRole::Ablation if !arm.contains(ABLATION) => Err(format!(
            "arm {arm:?} runs the ablation {:?}, which encodes the stream's hidden rules and is \
             never a comparison arm; its arm name must contain {ABLATION:?}",
            policy.id().0
        )),
        ArmRole::Comparison if arm.contains(PRIVILEGED) || arm.contains(ABLATION) => Err(format!(
            "arm {arm:?} is a comparison arm and may not be named as a privileged or ablation arm"
        )),
        _ => Ok(()),
    }
}

impl StreamManifest {
    /// Check that the manifest can be run.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = safe_name(&self.run_id, 100);
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
            return Err("arms is empty".to_owned());
        }
        let mut names = BTreeSet::new();
        for spec in &self.arms {
            if !safe_name(&spec.arm, 50) || self.run_id.len() + 1 + spec.arm.len() > 100 {
                return Err(format!(
                    "arm name {:?} must be 1 to 50 of letters, digits, '.', '_', '-' and short \
                     enough that `<run_id>.<arm>` is at most 100: it names a directory and a run id",
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
        if self.seeds.is_empty() {
            return Err("seeds is empty".to_owned());
        }
        let unique: BTreeSet<u64> = self.seeds.iter().copied().collect();
        if unique.len() != self.seeds.len() {
            return Err("seeds contains a duplicate".to_owned());
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
        self.rung.validate()?;
        self.limits.validate()?;
        let expected = self
            .limits
            .world_budget(&self.stream_params.normalized().reasoner.cost);
        if self.stream_params.normalized().budget != expected {
            return Err(format!(
                "stream_params.budget {:?} differs from the limits' budget {expected:?}",
                self.stream_params.normalized().budget
            ));
        }
        Ok(())
    }

    /// The role of arm `index`.
    pub fn role_of(&self, index: usize) -> ArmRole {
        self.arms[index].policy.role()
    }

    /// The names of the comparison arms: neither privileged nor ablation. What an analysis of an
    /// experiment compares.
    pub fn comparison_arms(&self) -> Vec<&str> {
        self.arms
            .iter()
            .filter(|a| a.policy.role() == ArmRole::Comparison)
            .map(|a| a.arm.as_str())
            .collect()
    }

    /// The one-arm manifest of arm `index`: the same run with only that arm, so that running it
    /// alone gives the arm's `results.csv` byte for byte (protocol replay is per arm). Its
    /// `run_id` is `<run_id>.<arm>`, which is what the arm's rows carry. A manifest that already
    /// has one arm is returned as it is, so that running an arm's own `manifest.json` reproduces
    /// its rows, run id included.
    ///
    /// # Panics
    ///
    /// If `index` is not an arm of the manifest.
    pub fn single_arm(&self, index: usize) -> StreamManifest {
        if self.arms.len() == 1 {
            assert_eq!(index, 0, "a one-arm manifest has only arm 0");
            return self.clone();
        }
        let spec = self.arms[index].clone();
        StreamManifest {
            run_id: format!("{}.{}", self.run_id, spec.arm),
            arms: vec![spec],
            ..self.clone()
        }
    }

    /// The stream parameters of the segment with `seed`.
    pub fn params_for(&self, seed: u64) -> StreamParams {
        StreamParams {
            seed,
            ..self.stream_params.clone()
        }
    }

    /// The canonical text of the manifest: pretty JSON and a final newline. Deterministic.
    pub fn canonical_json(&self) -> String {
        let mut text =
            serde_json::to_string_pretty(self).expect("a manifest serializes: no map keys, no NaN");
        text.push('\n');
        text
    }

    /// A manifest for `arms` over `seed_count` seeds starting at `seed_start`, with the default
    /// stream, rung, limits and exchange, and the environment captured from the current directory.
    #[allow(clippy::too_many_arguments)]
    pub fn for_current_environment(
        run_id: &str,
        experiment: &str,
        arms: Vec<StreamArmSpec>,
        seed_start: u64,
        seed_count: u32,
        trace_sample_rate: f64,
    ) -> Result<StreamManifest, String> {
        let env = Environment::capture(Path::new("."))?;
        let stream_params = StreamParams::new(0);
        let limits = StreamLimits::matching(&stream_params, DEFAULT_COMPUTE_NS, DEFAULT_STEP_NS);
        let manifest = StreamManifest {
            run_id: run_id.to_owned(),
            experiment: experiment.to_owned(),
            arms,
            run_seed: 0,
            drift_block: DEFAULT_DRIFT_BLOCK,
            source_revision: env.source_revision,
            lockfile_sha256: env.lockfile_sha256,
            toolchain: env.toolchain,
            cpu_flags: env.cpu_flags,
            cpu_model: env.cpu_model,
            cpu_mhz: env.cpu_mhz,
            isolation: IsolationSpec::default(),
            seeds: (seed_start..seed_start + u64::from(seed_count)).collect(),
            stream_params,
            rung: RungConfig::default(),
            limits,
            exchange: Exchange::default(),
            trace_sample_rate,
            internal_external_ratio: None,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

/// The provisional substrate allowance of a segment: 2 s of declared compute, far above what any
/// arm declares at the default rung (`stream/mod.rs`, "Placeholders"), so that it does not bind
/// unless an experiment sets it to. Not a preregistered budget.
pub const DEFAULT_COMPUTE_NS: u64 = 2_000_000_000;

/// The provisional step: 500 ms, so that a burst (under 300 ms) is complete when it is first
/// looked at, as the first world's cheap-rung tests looked every half second.
pub const DEFAULT_STEP_NS: u64 = 500_000_000;
