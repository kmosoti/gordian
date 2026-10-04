//! Helpers shared by the integration tests.
//!
//! Tests may read hidden truth through `gordian_eval::Truth` (the evaluator's own type) to write
//! the declaration a scripted policy should make. Policies never can.

#![allow(dead_code, missing_docs)]

use gordian_components::{Component, ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, EntryKind, Instant, Resource};
use gordian_eval::Truth;
use gordian_run::harness::{EpisodeRecord, HarnessError, Limits, run_episode};
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

    fn run(&mut self, _input: &WorkingState) -> ComponentOutput {
        self.runs.set(self.runs.get() + 1);
        if !self.sleep.is_zero() {
            std::thread::sleep(self.sleep);
        }
        ComponentOutput {
            entries: self.entries.clone(),
            ..ComponentOutput::default()
        }
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
