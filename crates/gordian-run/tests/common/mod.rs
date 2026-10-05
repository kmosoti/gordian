//! Helpers shared by the integration tests.
//!
//! Tests may read hidden truth through `gordian_eval::Truth` (the evaluator's own type) to write
//! the declaration a scripted policy should make. Policies never can.

#![allow(dead_code, missing_docs)]

use gordian_components::{Component, ComponentOutput, Ops, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, EntryKind, Instant, Resource};
use gordian_eval::Truth;
use gordian_run::harness::{EpisodeRecord, HarnessError, Limits, run_episode};
use gordian_run::policy::decide::RuleOps;
use gordian_run::policy::scripted::{ScriptedPolicy, ScriptedStep};
use gordian_run::policy::{Policy, PolicyId};
use gordian_world::{
    Action, EpisodeClass, EpisodeSpec, FaultKind, Observation, ServiceId, generate,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

/// Limits with the loop parameters the hand-checked tests assume: 50 ms steps, default budgets.
pub fn limits() -> Limits {
    Limits::default()
}

/// The spec for `(seed, class)` whose world budget matches `limits`.
pub fn spec(seed: u64, class: EpisodeClass, limits: &Limits) -> EpisodeSpec {
    let mut spec = EpisodeSpec::new(seed, class);
    spec.budget = limits.world_budget();
    spec
}

/// The true `(kind, site)` of a faulted episode, read through the evaluator's truth type.
pub fn truth_fault(spec: &EpisodeSpec) -> (FaultKind, ServiceId) {
    let truth = Truth::from_episode(&generate(spec));
    let fault = truth.faults.first().expect("a faulted class has a fault");
    (fault.kind, fault.site)
}

/// Number of services in the episode's world.
pub fn service_count(spec: &EpisodeSpec) -> u32 {
    generate(spec).world().len() as u32
}

/// Run `policy` with `components` on `(seed, class)` under `limits`.
pub fn play(
    seed: u64,
    class: EpisodeClass,
    policy: &mut dyn Policy,
    components: &mut [Box<dyn Component>],
    limits: &Limits,
) -> Result<EpisodeRecord, HarnessError> {
    run_episode(&spec(seed, class, limits), policy, components, limits)
}

/// Run a script with no components.
pub fn play_script(
    seed: u64,
    class: EpisodeClass,
    actions: Vec<Action>,
    limits: &Limits,
) -> EpisodeRecord {
    let mut policy = ScriptedPolicy::actions(actions);
    play(seed, class, &mut policy, &mut [], limits).expect("episode runs")
}

/// A shared count of how often a component's `run` was called.
pub type Counter = Rc<Cell<u32>>;

/// A component with a fixed declared cost whose runs are counted.
pub struct Fake {
    pub id: ComponentId,
    pub cost: Vec<Charge>,
    pub runs: Rc<Cell<u32>>,
    pub entries: Vec<(EntryKind, Vec<u8>)>,
    /// Wall time `run` takes, so that measured durations are visibly nonzero.
    pub sleep: std::time::Duration,
}

impl Fake {
    pub fn new(id: u32, cost: Vec<Charge>) -> (Self, Rc<Cell<u32>>) {
        let runs = Rc::new(Cell::new(0));
        (
            Self {
                id: ComponentId(id),
                cost,
                runs: Rc::clone(&runs),
                entries: Vec::new(),
                sleep: std::time::Duration::ZERO,
            },
            runs,
        )
    }
}

impl Component for Fake {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn declared_cost(&self, _input: &WorkingState) -> Vec<Charge> {
        self.cost.clone()
    }

    fn run_counted(&mut self, _input: &WorkingState) -> (ComponentOutput, Ops) {
        self.runs.set(self.runs.get() + 1);
        if !self.sleep.is_zero() {
            std::thread::sleep(self.sleep);
        }
        let output = ComponentOutput {
            entries: self.entries.clone(),
            ..ComponentOutput::default()
        };
        // A test double has no units: it counts nothing.
        (output, Ops::zero(self.id))
    }
}

/// `n` fakes with ids `0..n`, each declaring `compute` Compute and `time_ns` Time.
pub fn fakes(n: u32, compute: u64, time_ns: u64) -> (Vec<Box<dyn Component>>, Vec<Counter>) {
    let mut components: Vec<Box<dyn Component>> = Vec::new();
    let mut counters = Vec::new();
    for id in 0..n {
        let (fake, runs) = Fake::new(
            id,
            vec![
                Charge::new(Resource::Compute, compute),
                Charge::new(Resource::Time, time_ns),
            ],
        );
        components.push(Box::new(fake));
        counters.push(runs);
    }
    (components, counters)
}

/// What a spy policy saw at one step.
#[derive(Debug, Clone)]
pub struct Seen {
    pub now: Instant,
    pub evidence: Vec<(Instant, Observation)>,
    pub outputs: Vec<ComponentId>,
    pub hypotheses: usize,
    pub pending: Vec<ComponentId>,
    pub probes_remaining: Option<u64>,
}

/// A scripted policy that records what it is shown at each `decide`.
pub struct Spy {
    pub inner: ScriptedPolicy,
    pub log: Rc<RefCell<Vec<Seen>>>,
    /// Wall time `decide` takes.
    pub sleep: std::time::Duration,
    probes_remaining: Option<u64>,
}

impl Spy {
    pub fn new(inner: ScriptedPolicy) -> (Self, Rc<RefCell<Vec<Seen>>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                inner,
                log: Rc::clone(&log),
                sleep: std::time::Duration::ZERO,
                probes_remaining: None,
            },
            log,
        )
    }
}

impl Policy for Spy {
    fn id(&self) -> PolicyId {
        PolicyId::new("spy")
    }

    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_select_cost(state)
    }

    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.probes_remaining = bill.budget().remaining(Resource::Probes);
        self.inner.select(state, bill)
    }

    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        if !self.sleep.is_zero() {
            std::thread::sleep(self.sleep);
        }
        self.log.borrow_mut().push(Seen {
            now: state.now,
            evidence: state.evidence().iter().cloned().collect(),
            outputs: outputs.iter().map(|(id, _)| *id).collect(),
            hypotheses: state.hypotheses.len(),
            pending: state.pending.iter().map(|p| p.component).collect(),
            probes_remaining: self.probes_remaining,
        });
        self.inner.decide(state, outputs)
    }

    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_final_cost(state)
    }

    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        self.inner.decide_final(state)
    }

    fn take_ops(&mut self) -> RuleOps {
        self.inner.take_ops()
    }
}

/// A scripted step selecting `select` and taking `action`.
pub fn step(select: &[u32], action: Option<Action>) -> ScriptedStep {
    ScriptedStep {
        select: select.iter().map(|id| ComponentId(*id)).collect(),
        action,
    }
}

/// A unique scratch directory under cargo's per-target tmp dir.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

// ---- the B1 grid, shared by the tests that compare verdicts against a recorded fixture ----

use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID};
use gordian_run::manifest::{ArmSpec, EpisodeParams, IsolationSpec, Manifest, PRIVILEGED};
use gordian_run::policy::decide::DecideConfig;
use gordian_run::policy::{PolicySpec, fixed_pipeline, random_matched};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The ten arms of the B1 grid (`experiments/exploration/scripts/mkmanifest.py`), by name.
pub fn b1_arms() -> Vec<ArmSpec> {
    let pipeline = |components: Vec<gordian_core::ComponentId>, every: u32| {
        PolicySpec::FixedPipeline(fixed_pipeline::Config { components, every })
    };
    let arm = |name: &str, policy: PolicySpec| ArmSpec {
        arm: name.to_owned(),
        policy,
    };
    vec![
        arm("heuristic_only", PolicySpec::HeuristicOnly),
        arm("all_components", PolicySpec::AllComponents),
        arm(
            "random_p025",
            PolicySpec::RandomMatched(random_matched::Config { p: 0.25 }),
        ),
        arm(
            "random_p050",
            PolicySpec::RandomMatched(random_matched::Config { p: 0.5 }),
        ),
        arm("fixed_verifier_only", pipeline(vec![VERIFIER_ID], 1)),
        arm("fixed_estimator_only", pipeline(vec![ESTIMATOR_ID], 1)),
        arm("fixed_heuristic_every2", pipeline(vec![HEURISTIC_ID], 2)),
        arm("fixed_heuristic_every4", pipeline(vec![HEURISTIC_ID], 4)),
        arm(
            &format!("oracle_evidence_{PRIVILEGED}"),
            PolicySpec::OracleEvidence,
        ),
        arm(
            &format!("oracle_immediate_{PRIVILEGED}"),
            PolicySpec::OracleImmediate,
        ),
    ]
}

/// An interleaved manifest of `arms` over `seeds` seeds of every class, at `compute` nanoseconds.
pub fn grid_manifest(run_id: &str, arms: Vec<ArmSpec>, seeds: u64, compute: u64) -> Manifest {
    let mut limits = limits();
    limits.compute = compute;
    Manifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arm: arms[0].arm.clone(),
        source_revision: "0".repeat(40),
        lockfile_sha256: "0".repeat(64),
        toolchain: "rustc test".to_owned(),
        cpu_flags: vec!["avx2".to_owned()],
        cpu_model: Some("test cpu".to_owned()),
        cpu_mhz: Some(2100.0),
        isolation: IsolationSpec::default(),
        seeds: (0..seeds).collect(),
        episode_classes: EpisodeClass::ALL
            .iter()
            .map(|c| (*c, seeds as u32))
            .collect(),
        policy: arms[0].policy.clone(),
        arms,
        run_seed: 0,
        drift_block: 50,
        decide: DecideConfig::default(),
        limits,
        episode_params: EpisodeParams::default(),
        trace_sample_rate: 0.0,
        internal_external_ratio: None,
    }
}

/// The columns of `results.csv` that say what the arm concluded, as opposed to what it cost or
/// when. These are the verdict columns of work item A6c.
pub const VERDICT_COLUMNS: [&str; 7] = [
    "success",
    "critical_miss",
    "false_alarm",
    "abstained",
    "undecided",
    "probes_used",
    "corrections",
];

/// `(arm, seed, class)` to the verdict columns joined by commas, over every arm of a run.
pub fn verdicts(dir: &Path, arms: &[ArmSpec]) -> BTreeMap<(String, u64, String), String> {
    let mut out = BTreeMap::new();
    for arm in arms {
        let csv = fs::read_to_string(dir.join(&arm.arm).join("results.csv")).unwrap();
        let mut lines = csv.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let at = |name: &str| header.iter().position(|h| *h == name).unwrap();
        let (seed, class) = (at("seed"), at("class"));
        let columns: Vec<usize> = VERDICT_COLUMNS.iter().map(|c| at(c)).collect();
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            let key = (
                arm.arm.clone(),
                f[seed].parse().unwrap(),
                f[class].to_owned(),
            );
            let value = columns.iter().map(|c| f[*c]).collect::<Vec<_>>().join(",");
            assert!(out.insert(key, value).is_none(), "duplicate episode");
        }
    }
    out
}
