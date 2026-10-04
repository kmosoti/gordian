//! Episode specification, the generated episode, and what a policy may know at its start.

use crate::fault::{Fault, FaultKind};
use crate::graph::{Service, World};
use crate::physics::{MAX_SERVICES, MIN_SERVICES, SymptomTag};
use crate::sense::Observation;
use gordian_core::{Budget, Instant, Resource};
use serde::{Deserialize, Serialize};

/// Smallest horizon the generator accepts: 2 seconds. Smaller horizons are raised to this so
/// that every symptom chain fits inside the episode.
pub const MIN_HORIZON: Instant = Instant(2_000_000_000);

/// Largest `noise_rate` the generator accepts.
pub const MAX_NOISE_RATE: u32 = 50;

/// Largest `delay_k` the generator accepts.
pub const MAX_DELAY_K: u32 = 200;

/// The eleven episode classes. What each guarantees is stated on the variant and tested in
/// `tests/classes.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EpisodeClass {
    /// The first symptoms fit at least three fault kinds at the true site; exactly one probe
    /// kind on the site settles which.
    Ambiguous,
    /// A changed-config snapshot arrives at least `delay_k` observations before the first
    /// symptom, and fixes the fault. Without it, a probe is needed.
    DelayedConfigChange,
    /// At least 80% of observations are irrelevant messages with high-entropy text ids.
    NoiseFlood,
    /// Two hidden bits, each revealed by its own probe; the fault is `ENTANGLED.0` when they are
    /// equal and `ENTANGLED.1` otherwise. One probe alone changes nothing.
    JointlyDecisive,
    /// No fault. The only correct actions are `Abstain` and `Declare { fault: None }`.
    NoFault,
    /// A non-quiet fault with `critical = true`.
    CriticalFault,
    /// A noise flood whose only true signal is one low-severity message. The fault is critical.
    QuietUrgent,
    /// Each true observation appears 2 to 5 times with identical content.
    Duplicates,
    /// A decoy service whose `HealthCheck` is inconclusive and suggests running itself again.
    FeedbackBait,
    /// A prior record matches the current symptoms and names the wrong kind.
    StaleMemory,
    /// The harness carries directives making components fail or run slow. Policies never see
    /// them.
    ComponentTimeout,
}

impl EpisodeClass {
    /// Every class.
    pub const ALL: [EpisodeClass; 11] = [
        EpisodeClass::Ambiguous,
        EpisodeClass::DelayedConfigChange,
        EpisodeClass::NoiseFlood,
        EpisodeClass::JointlyDecisive,
        EpisodeClass::NoFault,
        EpisodeClass::CriticalFault,
        EpisodeClass::QuietUrgent,
        EpisodeClass::Duplicates,
        EpisodeClass::FeedbackBait,
        EpisodeClass::StaleMemory,
        EpisodeClass::ComponentTimeout,
    ];
}

/// Serializable stand-in for the two hard limits the world charges: probes and time.
///
/// `gordian_core::Budget` is not serializable and core is not this crate's to change, so the
/// spec carries the numbers and builds the `Budget` on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetSpec {
    /// Limit on `Resource::Probes`.
    pub probes: u64,
    /// Limit on `Resource::Time`, in nanoseconds of probe time.
    pub time_ns: u64,
}

impl BudgetSpec {
    /// The `Budget` this spec describes.
    pub fn to_budget(&self) -> Budget {
        Budget::new()
            .with_limit(Resource::Probes, self.probes)
            .with_limit(Resource::Time, self.time_ns)
    }
}

impl Default for BudgetSpec {
    fn default() -> Self {
        Self {
            probes: 12,
            time_ns: 250_000_000,
        }
    }
}

/// Everything that determines an episode. `generate` is a pure function of this value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeSpec {
    /// Seed of the ChaCha8 stream.
    pub seed: u64,
    /// Which class of episode to build.
    pub class: EpisodeClass,
    /// End of the episode. Actions after it are refused. Raised to [`MIN_HORIZON`] if smaller.
    #[serde(with = "crate::timeserde::instant")]
    pub horizon: Instant,
    /// Irrelevant observations injected per true observation, counting at least 8 true
    /// observations so that an empty signal still gets noise. Flood classes raise the effective
    /// rate until at least 80% of observations are irrelevant messages. Capped at
    /// [`MAX_NOISE_RATE`].
    pub noise_rate: u32,
    /// Hard limits on probes and probe time.
    pub budget: BudgetSpec,
    /// Fewest services in the world (clamped to 4..=12).
    pub min_services: u8,
    /// Most services in the world (clamped to `min_services..=12`).
    pub max_services: u8,
    /// For `DelayedConfigChange`: observations that must separate the snapshot from the first
    /// symptom. Capped at [`MAX_DELAY_K`].
    pub delay_k: u32,
}

impl EpisodeSpec {
    /// A spec with default parameters: 10 s horizon, noise rate 3, 4 to 12 services, `k = 5`.
    pub fn new(seed: u64, class: EpisodeClass) -> Self {
        Self {
            seed,
            class,
            horizon: Instant(10_000_000_000),
            noise_rate: 3,
            budget: BudgetSpec::default(),
            min_services: MIN_SERVICES,
            max_services: MAX_SERVICES,
            delay_k: 5,
        }
    }

    /// The spec with out-of-range parameters clamped. `generate` stores and uses this form.
    pub fn normalized(&self) -> Self {
        let min_services = self.min_services.clamp(MIN_SERVICES, MAX_SERVICES);
        let max_services = self.max_services.clamp(min_services, MAX_SERVICES);
        Self {
            horizon: self.horizon.max(MIN_HORIZON),
            noise_rate: self.noise_rate.min(MAX_NOISE_RATE),
            min_services,
            max_services,
            delay_k: self.delay_k.min(MAX_DELAY_K),
            ..self.clone()
        }
    }
}

/// A resolution recorded in an earlier episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorRecord {
    /// Sorted symptom tags seen in the earlier episode (see `physics::signature`).
    pub signature: Vec<SymptomTag>,
    /// The fault kind that earlier episode turned out to have.
    pub resolution: FaultKind,
}

/// What a policy may know at episode start.
///
/// The episode class, the fault, the hidden bits and the harness directives are not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicInfo {
    /// The service graph with configurations at start.
    pub services: Vec<Service>,
    /// Records from earlier episodes. Advisory: a record may be stale.
    pub prior_records: Vec<PriorRecord>,
    /// End of the episode.
    #[serde(with = "crate::timeserde::instant")]
    pub horizon: Instant,
    /// The hard limits.
    pub budget: BudgetSpec,
}

/// How a harness degrades a component in this episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentMode {
    /// The component fails when run.
    Fail,
    /// The component's time cost is multiplied by `factor`.
    Slow {
        /// Multiplier on the declared time cost, at least 2.
        factor: u32,
    },
}

/// An instruction to the harness about one component. Policies never see these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDirective {
    /// Index of the component. Independent of any component registry.
    pub component: u32,
    /// What to do to it.
    pub mode: ComponentMode,
}

/// What a stream entry is. Hidden state, used only by tests and the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamLabel {
    /// A true observation. Copies of one observation share `group`.
    Signal {
        /// Index of the original observation.
        group: u32,
    },
    /// A decoy that suggests a check which cannot help.
    Bait,
    /// Irrelevant to the fault.
    Noise,
}

/// Hidden simulator state. Fields are crate-private; see `oracle::reveal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hidden {
    pub(crate) faults: Vec<Fault>,
    pub(crate) bits: (bool, bool),
    pub(crate) drift_hash: Option<u64>,
    pub(crate) labels: Vec<StreamLabel>,
}

/// A generated episode: public world and stream, hidden instance.
///
/// Policies never hold one. They hold a `Simulator` built from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub(crate) spec: EpisodeSpec,
    pub(crate) world: World,
    #[serde(with = "crate::timeserde::timed")]
    pub(crate) stream: Vec<(Instant, Observation)>,
    pub(crate) directives: Vec<ComponentDirective>,
    pub(crate) prior: Vec<PriorRecord>,
    pub(crate) hidden: Hidden,
}

impl Episode {
    /// The normalized spec the episode was generated from.
    pub fn spec(&self) -> &EpisodeSpec {
        &self.spec
    }

    /// The public world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The passive observation stream, sorted by instant. Public by construction: it is what
    /// the simulator delivers to a policy.
    pub fn stream(&self) -> &[(Instant, Observation)] {
        &self.stream
    }

    /// Harness directives. The harness applies them; policies never see them.
    pub fn harness_directives(&self) -> &[ComponentDirective] {
        &self.directives
    }

    /// What a policy may know at episode start.
    pub fn public_info(&self) -> PublicInfo {
        PublicInfo {
            services: self.world.services.clone(),
            prior_records: self.prior.clone(),
            horizon: self.spec.horizon,
            budget: self.spec.budget,
        }
    }
}

/// Generate an episode. A pure function of `spec`: no clock, no global state, no hash-order
/// dependence. The spec is normalized first.
pub fn generate(spec: &EpisodeSpec) -> Episode {
    crate::builder::build(spec)
}
