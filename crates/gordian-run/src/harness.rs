//! The episode loop.
//!
//! [`run_episode`] plays one episode: generate it, hand it to a simulator, then repeat
//! sense, schedule, compute, decide, act, until the episode closes or a limit stops it; finally
//! score the recorded trajectory. The rules of the loop, and the decision on which budget
//! enforces which resource, are in `HARNESS.md`. This file is the implementation of them.
//!
//! # The wall clock
//!
//! This is the one place in the system that reads a wall clock, because it is a boundary:
//! [`std::time::Instant`] brackets each component call and each scheduling call, and each
//! duration is appended to the ledger as a `Measurement` from producer `harness/timer` (never as
//! `Accounting`, which `Bill::replay` decodes). Nothing the loop decides reads a timing.
//! Logical time is a [`ManualClock`] that moves only by declared costs and by the step quantum.

use crate::policy::Policy;
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ComponentOutput, ConsistencyVerifier, CountEstimator, PriorRecordLookup,
    RuleHeuristic, WorkingState,
};
use gordian_core::{
    Bill, Budget, Charge, ComponentId, EntryId, EntryKind, Instant, Ledger, LedgerError,
    ManualClock, Phase, Provenance, RecordedChargeError, Resource,
};
use gordian_eval::{EvalError, Step, Truth, Verdict, score};
use gordian_world::episode::BudgetSpec;
use gordian_world::step::CostSummary;
use gordian_world::{
    Action, ComponentMode, EpisodeClass, EpisodeSpec, Hypothesis, Observation, Outcome, ProbeKind,
    PublicInfo, Refusal, ServiceId, Simulator, generate,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Instant as Wall;

/// Producer version written to every ledger entry the harness itself appends.
const HARNESS_VERSION: &str = concat!("gordian-run/", env!("CARGO_PKG_VERSION"));

/// How many scored hypotheses the harness keeps in `WorkingState::hypotheses`.
const MAX_HYPOTHESES: usize = 8;

/// The hard limits and loop parameters of one episode.
///
/// `compute`, `memory`, `time_ns`, `probes` and `communication` are the declared limits of the
/// episode's [`Bill`]. `probes` and `time_ns` are also the world's own budget for the episode
/// (`EpisodeSpec::budget`); the harness refuses a spec whose budget differs. See `HARNESS.md`,
/// section 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// `Resource::Compute` limit: declared component and scheduling cost, nanoseconds.
    pub compute: u64,
    /// `Resource::Memory` limit, bytes.
    pub memory: u64,
    /// `Resource::Time` limit, nanoseconds of busy logical time: declared component and
    /// scheduling time, and the latency of probes and corrections. Idle waiting is not billed.
    pub time_ns: u64,
    /// `Resource::Probes` limit, probe units.
    pub probes: u64,
    /// `Resource::Communication` limit.
    pub communication: u64,
    /// Capacity of the bounded working state, in observations.
    pub window: usize,
    /// Logical time one step takes at least, nanoseconds. A step takes the larger of this and
    /// the busy time spent in it. Must be at least 1 so that time always advances.
    pub step_ns: u64,
    /// The runaway guard: the episode stops undecided after this many steps.
    pub max_steps: u32,
}

impl Default for Limits {
    /// Provisional defaults for exploration runs, not a preregistered budget. The world's own
    /// defaults for probes and time (12 probes, 250 ms); a 20 ms compute allowance, under what
    /// running every component at every step would use; 50 ms steps (200 over the default
    /// horizon); a step cap of 1000.
    fn default() -> Self {
        let world = BudgetSpec::default();
        Self {
            compute: 20_000_000,
            memory: 1 << 30,
            time_ns: world.time_ns,
            probes: world.probes,
            communication: 1_000_000,
            window: 256,
            step_ns: 50_000_000,
            max_steps: 1000,
        }
    }
}

impl Limits {
    /// A fresh budget with every limit declared and nothing spent. Build a new one for every
    /// episode: a bill over a budget that already has spend attributes none of it to a phase.
    pub fn budget(&self) -> Budget {
        Budget::new()
            .with_limit(Resource::Compute, self.compute)
            .with_limit(Resource::Memory, self.memory)
            .with_limit(Resource::Time, self.time_ns)
            .with_limit(Resource::Probes, self.probes)
            .with_limit(Resource::Communication, self.communication)
    }

    /// The part of the limits the simulator enforces from the episode spec.
    pub fn world_budget(&self) -> BudgetSpec {
        BudgetSpec {
            probes: self.probes,
            time_ns: self.time_ns,
        }
    }

    /// Check the parameters the loop relies on.
    pub fn validate(&self) -> Result<(), HarnessError> {
        if self.step_ns == 0 {
            return Err(HarnessError::InvalidLimits("step_ns must be at least 1"));
        }
        if self.max_steps == 0 {
            return Err(HarnessError::InvalidLimits("max_steps must be at least 1"));
        }
        Ok(())
    }
}

/// Why an episode stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The simulator accepted a `Declare` or `Abstain`.
    Terminal,
    /// Undecided, and nothing the arm could still do is affordable: no scheduling step plus
    /// component, and no probe or correction, fits the bill.
    BudgetExhausted,
    /// Undecided, and the logical clock passed the episode horizon.
    Horizon,
    /// Undecided, and the step cap was reached.
    StepCap,
}

impl StopReason {
    /// The value written to the `stop_reason` column of `results.csv`.
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::Terminal => "terminal",
            StopReason::BudgetExhausted => "budget_exhausted",
            StopReason::Horizon => "horizon",
            StopReason::StepCap => "step_cap",
        }
    }
}

/// Wall-clock durations for one episode, from `std::time::Instant` at the boundary.
///
/// Nondeterministic. They are the charter's cost `C` (see `HARNESS.md`) and go to
/// `measured.csv`, never to `results.csv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Measured {
    /// Sum of the durations of `Component::run` calls that ran.
    pub component_ns: u64,
    /// Sum of the durations of the policy's `declared_select_cost`, `select` and `decide` calls,
    /// exactly the `select` and `decide` entries of `harness/timer` in the ledger.
    pub sched_ns: u64,
    /// Everything else inside the episode: generation, simulator, charging, ledger, scoring,
    /// and the affordability check that decides whether the arm is out of means.
    pub harness_ns: u64,
}

/// Everything one episode produced.
#[derive(Debug, Clone)]
pub struct EpisodeRecord {
    /// The episode's seed.
    pub seed: u64,
    /// The episode's class. The evaluator's key, not something a policy sees.
    pub class: EpisodeClass,
    /// The score, from `gordian_eval::score`.
    pub verdict: Verdict,
    /// The live bill at the end of the episode.
    pub bill: Bill,
    /// The ledger: observations, accounting, decisions, outcomes, timings.
    pub ledger: Ledger,
    /// The steps the simulator answered, in order, up to and including the terminal one.
    pub trajectory: Vec<Step>,
    /// Steps the loop took.
    pub steps: u32,
    /// Selected components that were charged and then executed or failed.
    pub components_run: u32,
    /// Selected components whose charge the bill refused, so they did not run.
    pub components_skipped: u32,
    /// Harness directives that named no real component and were ignored.
    pub directives_ignored: u32,
    /// Why the loop stopped.
    pub stop: StopReason,
    /// Wall-clock durations.
    pub measured: Measured,
    /// What the policy was told at the start. Public by construction.
    pub public_info: PublicInfo,
    /// The episode's passive observation stream in full, including what was never delivered.
    /// Public by construction (`Episode::stream`).
    pub public_stream: Vec<(Instant, Observation)>,
}

/// A defect in the harness or in how it was called. Never a result: a run that hits one fails.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessError {
    /// The evaluator refused the trajectory: a bug in the loop (`RULES.md`, R14 to R18).
    Eval(EvalError),
    /// The ledger refused an append.
    Ledger(LedgerError),
    /// The limits are unusable.
    InvalidLimits(&'static str),
    /// The episode spec's world budget differs from the limits' probes and time.
    BudgetMismatch {
        /// What the spec carries.
        spec: BudgetSpec,
        /// What the limits require.
        limits: BudgetSpec,
    },
    /// The policy selected a component that is not part of the run.
    UnknownComponent(ComponentId),
    /// Two components of the run share an id.
    DuplicateComponent(ComponentId),
    /// The bill and the simulator disagree about an action's cost or acceptance.
    BillDisagreement(String),
    /// The loop and the evaluator disagree about whether the episode closed.
    Inconsistent(String),
    /// A ledger payload could not be encoded.
    Payload(String),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarnessError::Eval(e) => write!(f, "evaluator refused the trajectory: {e}"),
            HarnessError::Ledger(e) => write!(f, "ledger refused an append: {e:?}"),
            HarnessError::InvalidLimits(why) => write!(f, "invalid limits: {why}"),
            HarnessError::BudgetMismatch { spec, limits } => write!(
                f,
                "episode spec budget {spec:?} differs from the limits' world budget {limits:?}"
            ),
            HarnessError::UnknownComponent(id) => {
                write!(f, "policy selected unknown component {}", id.0)
            }
            HarnessError::DuplicateComponent(id) => {
                write!(f, "two components share id {}", id.0)
            }
            HarnessError::BillDisagreement(why) => write!(f, "bill and simulator disagree: {why}"),
            HarnessError::Inconsistent(why) => write!(f, "inconsistent episode: {why}"),
            HarnessError::Payload(why) => write!(f, "ledger payload: {why}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<EvalError> for HarnessError {
    fn from(e: EvalError) -> Self {
        HarnessError::Eval(e)
    }
}

impl From<LedgerError> for HarnessError {
    fn from(e: LedgerError) -> Self {
        HarnessError::Ledger(e)
    }
}

/// One fresh instance of each fixed component of `gordian-components`, in id order. The lookup
/// reads every record.
pub fn standard_components() -> Vec<Box<dyn Component>> {
    vec![
        Box::new(RuleHeuristic::new()),
        Box::new(CountEstimator::new()),
        Box::new(PriorRecordLookup::new()),
        Box::new(ConsistencyVerifier::new()),
    ]
}

fn elapsed_ns(since: Wall) -> u64 {
    u64::try_from(since.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let started = Wall::now();
    let value = f();
    (value, elapsed_ns(started))
}

fn provenance(producer: String, inputs: Vec<EntryId>) -> Provenance {
    Provenance {
        producer,
        producer_version: HARNESS_VERSION.to_owned(),
        inputs,
    }
}

fn payload(value: &impl Serialize) -> Result<Vec<u8>, HarnessError> {
    serde_json::to_vec(value).map_err(|e| HarnessError::Payload(e.to_string()))
}

/// Declared `Resource::Time` in `charges`, saturating.
fn time_of(charges: &[Charge]) -> u64 {
    charges
        .iter()
        .filter(|c| c.resource == Resource::Time)
        .fold(0u64, |sum, c| sum.saturating_add(c.amount))
}

/// `charges` with every `Resource::Time` amount multiplied by `factor` (a `Slow` directive).
fn slowed(charges: Vec<Charge>, factor: u32) -> Vec<Charge> {
    charges
        .into_iter()
        .map(|c| {
            if c.resource == Resource::Time {
                Charge::new(c.resource, c.amount.saturating_mul(u64::from(factor)))
            } else {
                c
            }
        })
        .collect()
}

/// Whether `charges` would be accepted by `budget`. Changes nothing.
fn affordable(budget: &Budget, charges: &[Charge]) -> bool {
    budget.clone().charge_all(charges).is_ok()
}

/// What happened to a recorded charge.
enum Charged {
    /// Accepted; the bill is debited. The id is the accounting entry.
    Accepted(EntryId),
    /// Refused by the bill; the bill is unchanged. The id is the accounting entry that records
    /// the refusal.
    Refused(EntryId),
}

fn charge(
    bill: &mut Bill,
    ledger: &mut Ledger,
    at: Instant,
    producer: &str,
    phase: Phase,
    charges: &[Charge],
) -> Result<Charged, HarnessError> {
    match bill.charge_recorded(
        ledger,
        at,
        provenance(producer.to_owned(), Vec::new()),
        phase,
        charges,
    ) {
        Ok(entry) => Ok(Charged::Accepted(entry)),
        Err(RecordedChargeError::Refused { entry, .. }) => Ok(Charged::Refused(entry)),
        Err(RecordedChargeError::Ledger(e)) => Err(HarnessError::Ledger(e)),
    }
}

fn append(
    ledger: &mut Ledger,
    at: Instant,
    kind: EntryKind,
    producer: String,
    inputs: Vec<EntryId>,
    body: Vec<u8>,
) -> Result<EntryId, HarnessError> {
    Ok(ledger.append(at, kind, provenance(producer, inputs), body)?)
}

/// Record one timing as a `Measurement` from `harness/timer`.
fn record_timing(
    ledger: &mut Ledger,
    at: Instant,
    what: &str,
    component: Option<ComponentId>,
    ns: u64,
    inputs: Vec<EntryId>,
) -> Result<(), HarnessError> {
    let body = payload(&json!({
        "what": what,
        "component": component.map(|c| c.0),
        "ns": ns,
    }))?;
    append(
        ledger,
        at,
        EntryKind::Measurement,
        "harness/timer".to_owned(),
        inputs,
        body,
    )?;
    Ok(())
}

/// The scored hypotheses of the first output that carries any, at most [`MAX_HYPOTHESES`].
fn scored_hypotheses(outputs: &[(ComponentId, ComponentOutput)]) -> Vec<(Hypothesis, u32)> {
    for (_, output) in outputs.iter().rev() {
        for (_, bytes) in &output.entries {
            if let Ok(HypothesisEntry::Candidates { ranked, .. }) = decode(bytes) {
                let scored: Vec<(Hypothesis, u32)> = ranked
                    .iter()
                    .filter_map(|r| r.score.map(|s| (r.hypothesis, s)))
                    .take(MAX_HYPOTHESES)
                    .collect();
                if !scored.is_empty() {
                    return scored;
                }
            }
        }
    }
    Vec::new()
}

/// Whether the arm could still do anything: pay for a scheduling step and then a component, or
/// for a probe or a correction. Declaring and abstaining are free and do not count; an arm that
/// can no longer compute or sense is out of means, and the loop stops it.
fn work_affordable(
    bill: &Bill,
    policy: &dyn Policy,
    state: &WorkingState,
    components: &[Box<dyn Component>],
    slow: &BTreeMap<ComponentId, u32>,
) -> bool {
    let mut budget = bill.budget().clone();
    if budget
        .charge_all(&policy.declared_select_cost(state))
        .is_err()
    {
        return false;
    }
    let component_fits = components.iter().any(|c| {
        let mut cost = c.declared_cost(state);
        if let Some(factor) = slow.get(&c.id()) {
            cost = slowed(cost, *factor);
        }
        affordable(&budget, &cost)
    });
    if component_fits {
        return true;
    }
    let target = ServiceId(0);
    ProbeKind::ALL
        .iter()
        .map(|kind| Action::Probe {
            kind: *kind,
            target,
        })
        .chain(std::iter::once(Action::Correct { site: target }))
        .any(|action| affordable(&budget, &action.cost()))
}

/// Play one episode and score it.
///
/// `spec` must carry the world budget that `limits` requires (`Limits::world_budget`), because
/// the simulator enforces it independently of the bill. `policy` and `components` should be
/// fresh for the episode; the recorder builds them so.
///
/// The loop, per step: deliver the observations that arrived (and the result of the previous
/// probe) and admit them into the working state; charge the policy's declared scheduling cost and
/// let it select components; charge and run each selected component; let the policy decide;
/// apply the action. It stops at the first of: a terminal outcome, the logical clock passing the
/// horizon, no affordable work left, or the step cap. `HARNESS.md` gives the rest.
///
/// # Errors
///
/// A [`HarnessError`] is a defect in the harness or in the call, never a result. In particular
/// an [`EvalError`] from the evaluator means the loop recorded a trajectory no correct harness
/// could have produced.
pub fn run_episode(
    spec: &EpisodeSpec,
    policy: &mut dyn Policy,
    components: &mut [Box<dyn Component>],
    limits: &Limits,
) -> Result<EpisodeRecord, HarnessError> {
    let started = Wall::now();
    limits.validate()?;
    if spec.budget != limits.world_budget() {
        return Err(HarnessError::BudgetMismatch {
            spec: spec.budget,
            limits: limits.world_budget(),
        });
    }
    let mut ids: Vec<ComponentId> = components.iter().map(|c| c.id()).collect();
    ids.sort();
    if let Some(pair) = ids.windows(2).find(|w| w[0] == w[1]) {
        return Err(HarnessError::DuplicateComponent(pair[0]));
    }

    // The episode exists in this block only. Truth is built from it immediately, what the
    // harness needs from it is copied out, and then it moves into the simulator. After this
    // block nothing but the simulator holds it.
    let episode = generate(spec);
    let truth = Truth::from_episode(&episode);
    let directives = episode.harness_directives().to_vec();
    let public_stream = episode.stream().to_vec();
    let horizon = episode.spec().horizon;
    let (seed, class) = (episode.spec().seed, episode.spec().class);
    let mut sim = Simulator::new(episode);
    let public_info = sim.public_info();

    // Harness directives name components by index; a real component has that id. The others
    // are ignored and counted.
    let mut fail: BTreeSet<ComponentId> = BTreeSet::new();
    let mut slow: BTreeMap<ComponentId, u32> = BTreeMap::new();
    let mut directives_ignored = 0u32;
    for d in &directives {
        let id = ComponentId(d.component);
        if !ids.contains(&id) {
            directives_ignored += 1;
            continue;
        }
        match d.mode {
            ComponentMode::Fail => {
                fail.insert(id);
            }
            ComponentMode::Slow { factor } => {
                slow.insert(id, factor);
            }
        }
    }

    // One fresh budget, one bill over it, one ledger, for this episode only.
    let mut bill = Bill::new(limits.budget());
    let mut ledger = Ledger::new();
    let mut clock = ManualClock::new();
    let mut state = WorkingState::new(public_info.clone(), limits.window);
    let policy_producer = format!("policy/{}", policy.id().0);

    let mut trajectory: Vec<Step> = Vec::new();
    let mut ready_results: Vec<(Instant, Observation)> = Vec::new();
    let mut measured = Measured::default();
    let (mut components_run, mut components_skipped) = (0u32, 0u32);
    let mut steps = 0u32;

    let stop = loop {
        steps += 1;
        let step_start = clock.now();

        // Sense: passive observations that have arrived, and the result of the last probe.
        // Admitted in instant order; the measurement entry is stamped with the admission time
        // and carries the observation's own instant in its payload.
        let mut arrived: Vec<(Instant, Observation)> = sim.observe_until(step_start);
        arrived.append(&mut ready_results);
        arrived.sort_by_key(|(at, _)| *at);
        for (at, observation) in arrived {
            let body = payload(&json!({ "at_ns": at.0, "observation": observation }))?;
            append(
                &mut ledger,
                step_start,
                EntryKind::Measurement,
                "harness/sensor".to_owned(),
                Vec::new(),
                body,
            )?;
            state.admit(at, observation);
        }
        state.now = step_start;

        // Schedule: charge the declared selection cost, then select. One timer entry covers the
        // policy's declared-cost call and its select call.
        let (select_cost, cost_ns) = timed(|| policy.declared_select_cost(&state));
        let scheduled = charge(
            &mut bill,
            &mut ledger,
            clock.now(),
            &policy_producer,
            Phase::Scheduling,
            &select_cost,
        )?;
        let (selected, select_ns, accounting) = match scheduled {
            Charged::Accepted(accounting) => {
                clock.advance(time_of(&select_cost));
                let (selected, ns) = timed(|| policy.select(&state, &bill));
                (selected, ns, accounting)
            }
            Charged::Refused(refusal) => (Vec::new(), 0, refusal),
        };
        let ns = cost_ns.saturating_add(select_ns);
        measured.sched_ns = measured.sched_ns.saturating_add(ns);
        record_timing(
            &mut ledger,
            clock.now(),
            "select",
            None,
            ns,
            vec![accounting],
        )?;

        // Compute: charge, then run, each selected component once.
        let mut outputs: Vec<(ComponentId, ComponentOutput)> = Vec::new();
        let mut ran: Vec<ComponentId> = Vec::new();
        let mut seen: Vec<ComponentId> = Vec::new();
        for id in selected {
            if !ids.contains(&id) {
                return Err(HarnessError::UnknownComponent(id));
            }
            if seen.contains(&id) {
                components_skipped += 1;
                continue;
            }
            seen.push(id);
            let Some(component) = components.iter_mut().find(|c| c.id() == id) else {
                return Err(HarnessError::UnknownComponent(id));
            };
            let mut cost = component.declared_cost(&state);
            if let Some(factor) = slow.get(&id) {
                cost = slowed(cost, *factor);
            }
            let producer = format!("component/{}", id.0);
            let accounting = match charge(
                &mut bill,
                &mut ledger,
                clock.now(),
                &producer,
                Phase::Component(id),
                &cost,
            )? {
                Charged::Accepted(accounting) => accounting,
                Charged::Refused(_) => {
                    components_skipped += 1;
                    continue;
                }
            };
            components_run += 1;
            ran.push(id);
            clock.advance(time_of(&cost));
            if fail.contains(&id) {
                // Charged, but produced nothing. The ledger says only that nothing came out.
                let body = payload(&json!({ "component": id.0, "output": "none" }))?;
                append(
                    &mut ledger,
                    clock.now(),
                    EntryKind::ComputationResult,
                    producer,
                    vec![accounting],
                    body,
                )?;
                continue;
            }
            let (output, ns) = timed(|| component.run(&state));
            measured.component_ns = measured.component_ns.saturating_add(ns);
            record_timing(
                &mut ledger,
                clock.now(),
                "component",
                Some(id),
                ns,
                vec![accounting],
            )?;
            for (kind, body) in &output.entries {
                append(
                    &mut ledger,
                    clock.now(),
                    *kind,
                    producer.clone(),
                    vec![accounting],
                    body.clone(),
                )?;
            }
            for request in &output.requests {
                let body = payload(&json!({
                    "component": request.component.0,
                    "reason": request.reason,
                }))?;
                append(
                    &mut ledger,
                    clock.now(),
                    EntryKind::ComputationRequest,
                    producer.clone(),
                    vec![accounting],
                    body,
                )?;
            }
            outputs.push((id, output));
        }

        // Decide, and act.
        let (action, ns) = timed(|| policy.decide(&state, &outputs));
        measured.sched_ns = measured.sched_ns.saturating_add(ns);
        record_timing(&mut ledger, clock.now(), "decide", None, ns, Vec::new())?;
        let mut terminal = false;
        if let Some(action) = action {
            let decision = append(
                &mut ledger,
                clock.now(),
                EntryKind::Decision,
                policy_producer.clone(),
                Vec::new(),
                payload(&action)?,
            )?;
            let cost = action.cost();
            let at = clock.now();
            if !affordable(bill.budget(), &cost) {
                // The bill is the authority and is never looser than the simulator, so the
                // world is not asked. The refusal is recorded through the bill, which appends a
                // refused accounting entry; the trajectory holds only what the world answered.
                match charge(
                    &mut bill,
                    &mut ledger,
                    at,
                    "harness/sensor",
                    Phase::Sensing,
                    &cost,
                )? {
                    Charged::Refused(_) => {}
                    Charged::Accepted(_) => {
                        return Err(HarnessError::BillDisagreement(
                            "a charge that a dry run refused was accepted".to_owned(),
                        ));
                    }
                }
                let body = payload(&json!({ "refused": "bill" }))?;
                append(
                    &mut ledger,
                    at,
                    EntryKind::Outcome,
                    "harness/bill".to_owned(),
                    vec![decision],
                    body,
                )?;
            } else {
                let outcome = sim.apply(action, at);
                append(
                    &mut ledger,
                    at,
                    EntryKind::Outcome,
                    "harness/simulator".to_owned(),
                    vec![decision],
                    payload(&outcome)?,
                )?;
                trajectory.push(Step {
                    at,
                    action,
                    outcome: outcome.clone(),
                });
                match outcome {
                    Outcome::Probed {
                        observation,
                        ready_at,
                        cost: world_cost,
                    }
                    | Outcome::Corrected {
                        observation,
                        ready_at,
                        cost: world_cost,
                    } => {
                        if world_cost != CostSummary::from_charges(&cost) {
                            return Err(HarnessError::BillDisagreement(format!(
                                "world charged {world_cost:?} for an action declared as {cost:?}"
                            )));
                        }
                        match charge(
                            &mut bill,
                            &mut ledger,
                            at,
                            "harness/sensor",
                            Phase::Sensing,
                            &cost,
                        )? {
                            Charged::Accepted(_) => {}
                            Charged::Refused(_) => {
                                return Err(HarnessError::BillDisagreement(
                                    "the bill refused a charge a dry run accepted".to_owned(),
                                ));
                            }
                        }
                        // Busy for the action's latency; the result is delivered at the next
                        // step, in instant order with whatever arrived meanwhile.
                        if let Some(wait) = ready_at.since(clock.now()) {
                            clock.advance(wait);
                        }
                        ready_results.push((ready_at, observation));
                    }
                    Outcome::Declared | Outcome::Abstained => terminal = true,
                    Outcome::Refused(Refusal::BudgetExceeded { cost }) => {
                        return Err(HarnessError::BillDisagreement(format!(
                            "the world refused {cost:?} that the bill had accepted"
                        )));
                    }
                    Outcome::Refused(_) => {}
                }
            }
        }
        if terminal {
            break StopReason::Terminal;
        }

        // Keep the working state's harness-owned parts: scored hypotheses and pending requests.
        let scored = scored_hypotheses(&outputs);
        if !scored.is_empty() {
            state.hypotheses = scored;
        }
        state.pending.retain(|p| !ran.contains(&p.component));
        for (requester, output) in &outputs {
            for request in &output.requests {
                if request.component != *requester
                    && !state
                        .pending
                        .iter()
                        .any(|p| p.component == request.component)
                {
                    state.pending.push(request.clone());
                }
            }
        }

        // A step takes at least the quantum; idle time is not billed.
        let step_end = Instant(step_start.0.saturating_add(limits.step_ns));
        if let Some(rest) = step_end.since(clock.now()) {
            clock.advance(rest);
        }

        if clock.now() > horizon {
            break StopReason::Horizon;
        }
        if !work_affordable(&bill, &*policy, &state, components, &slow) {
            break StopReason::BudgetExhausted;
        }
        if steps >= limits.max_steps {
            break StopReason::StepCap;
        }
    };

    // Score. The trajectory ends at the terminal step if there was one; the loop stops right
    // after it, so no attempt after the close is ever recorded. An evaluator error is a harness
    // bug and fails the run.
    let verdict = score(&truth, &trajectory)?;
    if verdict.undecided != (stop != StopReason::Terminal) {
        return Err(HarnessError::Inconsistent(format!(
            "loop stopped with {stop:?} but the evaluator says undecided = {}",
            verdict.undecided
        )));
    }
    let total = elapsed_ns(started);
    measured.harness_ns = total
        .saturating_sub(measured.component_ns)
        .saturating_sub(measured.sched_ns);

    Ok(EpisodeRecord {
        seed,
        class,
        verdict,
        bill,
        ledger,
        trajectory,
        steps,
        components_run,
        components_skipped,
        directives_ignored,
        stop,
        measured,
        public_info,
        public_stream,
    })
}
