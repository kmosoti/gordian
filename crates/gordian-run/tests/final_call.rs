//! The final call (plan item A6b): when the loop finds that no affordable work is left, or that
//! the horizon was reached, the policy gets one more `decide`, and a declaration made there is a
//! scored terminal step with `stop_reason = final_declaration`.
//!
//! The expectations come from `HARNESS.md` section 1 and the evaluator's rules, not from reading
//! the code under test. The comparison tests do not compare with a stored copy of an old output:
//! they run the same arm twice in the same build, once as built and once wrapped so that it makes
//! no final declaration ([`NoFinal`], which is exactly the loop before A6b as far as `results.csv`
//! goes), and compare the rows.

mod common;

use common::*;
use gordian_components::{ComponentOutput, VERIFIER_ID, WorkingState};
use gordian_core::{
    Bill, Charge, ComponentId, EntryKind, Instant, Phase, Resource, decode_accounting,
};
use gordian_eval::Truth;
use gordian_run::harness::{EpisodeRecord, Limits, StopReason, run_episode_privileged};
use gordian_run::policy::decide::{DecideConfig, RuleOps};
use gordian_run::policy::privileged::{OracleFactory, Variant};
use gordian_run::policy::scripted::{ScriptedPolicy, ScriptedStep};
use gordian_run::policy::{self, Built, Policy, PolicyId, PolicySpec, fixed_pipeline, zero_cost};
use gordian_run::results::results_row;
use gordian_run::standard_components;
use gordian_world::{
    Action, EpisodeClass, Observation, Outcome, ProbeKind, Refusal, ServiceId, Simulator, generate,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const MS: u64 = 1_000_000;

fn declare_truth(seed: u64, class: EpisodeClass, l: &Limits) -> Action {
    let (kind, site) = truth_fault(&spec(seed, class, l));
    Action::Declare {
        fault: Some((kind, site)),
    }
}

/// A scripted policy that records what it is shown at the final call, and how often `decide` and
/// `decide_final` were called.
type SeenLog = Rc<RefCell<Vec<Seen>>>;
type Calls = Rc<Cell<u32>>;

struct FinalSpy {
    inner: ScriptedPolicy,
    finals: SeenLog,
    decides: Calls,
}

impl FinalSpy {
    fn new(inner: ScriptedPolicy) -> (Self, SeenLog, Calls) {
        let finals = Rc::new(RefCell::new(Vec::new()));
        let decides = Rc::new(Cell::new(0));
        (
            Self {
                inner,
                finals: finals.clone(),
                decides: decides.clone(),
            },
            finals,
            decides,
        )
    }
}

impl Policy for FinalSpy {
    fn id(&self) -> PolicyId {
        PolicyId::new("final-spy")
    }
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_select_cost(state)
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.inner.select(state, bill)
    }
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        self.decides.set(self.decides.get() + 1);
        self.inner.decide(state, outputs)
    }
    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_final_cost(state)
    }
    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        self.finals.borrow_mut().push(Seen {
            now: state.now,
            evidence: state.evidence().iter().cloned().collect(),
            outputs: Vec::new(),
            hypotheses: state.hypotheses.len(),
            pending: Vec::new(),
            probes_remaining: None,
        });
        self.inner.decide_final(state)
    }
    fn take_ops(&mut self) -> RuleOps {
        self.inner.take_ops()
    }
}

fn is_probed(o: &Observation) -> bool {
    matches!(o, Observation::Probed { .. })
}

/// A scripted policy that spends the two probe units of `l.probes = 2` on step one, which leaves
/// nothing affordable, with `final_action` as its final call.
fn out_of_probes(final_action: Option<Action>) -> (EpisodeRecord, SeenLog, Calls) {
    let mut l = limits();
    l.probes = 2;
    let site = truth_fault(&spec(3, EpisodeClass::CriticalFault, &l)).1;
    let mut script = ScriptedPolicy::actions(vec![Action::Probe {
        kind: ProbeKind::LatencySample,
        target: site,
    }]);
    if let Some(action) = final_action {
        script = script.with_final(action);
    }
    let (mut spy, finals, decides) = FinalSpy::new(script);
    let record = play(3, EpisodeClass::CriticalFault, &mut spy, &mut [], &l).unwrap();
    (record, finals, decides)
}

// ---- when the call happens ----

#[test]
fn running_out_of_affordable_work_gives_one_final_call_whose_declaration_is_scored() {
    let l0 = {
        let mut l = limits();
        l.probes = 2;
        l
    };
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l0);
    let (record, finals, decides) = out_of_probes(Some(truth));
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert_eq!(record.stop.as_str(), "final_declaration");
    // The final call is not a step; the one step was the probe.
    assert_eq!(record.steps, 1);
    assert_eq!(decides.get(), 1);
    assert_eq!(finals.borrow().len(), 1);
    // A real terminal step in the scored trajectory, scored by the ordinary rules.
    let v = &record.verdict;
    assert!(v.success && !v.undecided && !v.abstained && !v.critical_miss);
    assert_eq!(record.trajectory.len(), 2);
    let last = record.trajectory.last().unwrap();
    assert_eq!(last.action, truth);
    assert_eq!(last.outcome, Outcome::Declared);
    // The step ended at 50 ms; the call is made there, and the verdict carries that instant.
    assert_eq!(last.at, Instant(50 * MS));
    assert_eq!(v.decision_at, Some(Instant(50 * MS)));
}

#[test]
fn the_final_call_sees_the_result_of_the_probe_bought_in_the_last_step() {
    let l0 = {
        let mut l = limits();
        l.probes = 2;
        l
    };
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l0);
    let (_, finals, _) = out_of_probes(Some(truth));
    let finals = finals.borrow();
    // The probe's result (ready 8 ms after the step began) reached the working state before the
    // call, so the money spent on it was not wasted on a policy that could not look.
    assert_eq!(finals[0].now, Instant(50 * MS));
    assert!(finals[0].evidence.iter().any(|(_, o)| is_probed(o)));
}

#[test]
fn the_horizon_gives_a_final_call_made_at_the_horizon() {
    let l = limits();
    let truth = declare_truth(3, EpisodeClass::Ambiguous, &l);
    let (mut spy, finals, decides) =
        FinalSpy::new(ScriptedPolicy::never_decides().with_final(truth));
    let record = play(3, EpisodeClass::Ambiguous, &mut spy, &mut [], &l).unwrap();
    // 201 steps; the clock is at 10.05 s when the loop stops, past the horizon, which the world
    // refuses. The call is made at the horizon.
    assert_eq!(record.steps, 201);
    assert_eq!(decides.get(), 201);
    assert_eq!(finals.borrow().len(), 1);
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert_eq!(record.verdict.decision_at, Some(Instant(10_000 * MS)));
    assert!(record.verdict.success);
}

#[test]
fn a_result_that_is_ready_only_after_the_horizon_is_not_shown_to_the_final_call() {
    // Step 200 starts at the horizon and buys a probe whose result is ready 8 ms later, after it.
    let l = limits();
    let site = truth_fault(&spec(3, EpisodeClass::Ambiguous, &l)).1;
    let mut script: Vec<ScriptedStep> = (0..200).map(|_| step(&[], None)).collect();
    script.push(step(
        &[],
        Some(Action::Probe {
            kind: ProbeKind::LatencySample,
            target: site,
        }),
    ));
    let (mut spy, finals, _) = FinalSpy::new(
        ScriptedPolicy::new(script).with_final(declare_truth(3, EpisodeClass::Ambiguous, &l)),
    );
    let record = play(3, EpisodeClass::Ambiguous, &mut spy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert_eq!(record.verdict.probes_used, 1);
    let finals = finals.borrow();
    assert!(finals[0].evidence.iter().all(|(_, o)| !is_probed(o)));
    // The probe was carried out at the horizon, and the declaration is at the horizon too.
    let at: Vec<Instant> = record.trajectory.iter().map(|s| s.at).collect();
    assert_eq!(at, vec![Instant(10_000 * MS), Instant(10_000 * MS)]);
}

#[test]
fn the_step_cap_is_a_runaway_guard_and_gets_no_final_call() {
    let mut l = limits();
    l.max_steps = 7;
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l);
    let (mut spy, finals, _) = FinalSpy::new(ScriptedPolicy::never_decides().with_final(truth));
    let record = play(3, EpisodeClass::CriticalFault, &mut spy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::StepCap);
    assert!(finals.borrow().is_empty());
    assert!(record.verdict.undecided);
    assert!(record.trajectory.is_empty());
}

#[test]
fn an_ordinary_declaration_gets_no_final_call() {
    let l = limits();
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l);
    let (mut spy, finals, _) =
        FinalSpy::new(ScriptedPolicy::actions(vec![truth]).with_final(Action::Abstain));
    let record = play(3, EpisodeClass::CriticalFault, &mut spy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::Terminal);
    assert!(finals.borrow().is_empty());
    assert!(record.verdict.success);
}

// ---- what is recorded ----

#[test]
fn final_declaration_means_the_terminal_step_came_from_the_final_call() {
    // A final call that does not close the episode leaves the reason the loop stopped.
    let (record, finals, _) = out_of_probes(None);
    assert_eq!(finals.borrow().len(), 1, "the call was made");
    assert_eq!(record.stop, StopReason::BudgetExhausted);
    assert!(record.verdict.undecided && record.verdict.decision_at.is_none());

    let l = limits();
    let (mut spy, finals, _) = FinalSpy::new(ScriptedPolicy::never_decides());
    let record = play(3, EpisodeClass::Ambiguous, &mut spy, &mut [], &l).unwrap();
    assert_eq!(finals.borrow().len(), 1);
    assert_eq!(record.stop, StopReason::Horizon);
    assert!(record.verdict.undecided);

    // Abstaining at the final call is a final declaration too: the arm answered "no answer".
    let (record, _, _) = out_of_probes(Some(Action::Abstain));
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert!(record.verdict.abstained && !record.verdict.undecided);
    assert!(
        record.verdict.critical_miss,
        "an abstention is still a miss on a critical fault"
    );
}

#[test]
fn only_a_declaration_or_an_abstention_is_carried_out_at_the_final_call() {
    let (record, finals, _) = out_of_probes(Some(Action::Probe {
        kind: ProbeKind::HealthCheck,
        target: ServiceId(0),
    }));
    assert_eq!(finals.borrow().len(), 1);
    assert_eq!(record.stop, StopReason::BudgetExhausted);
    assert!(record.verdict.undecided);
    // The world was not asked: the trajectory is the one probe of step one.
    assert_eq!(record.trajectory.len(), 1);
    assert_eq!(record.verdict.probes_used, 1);
    // The ledger says what happened to it.
    let decisions = record
        .ledger
        .iter()
        .filter(|e| e.kind == EntryKind::Decision)
        .count();
    assert_eq!(decisions, 2);
    let ignored: Vec<_> = record
        .ledger
        .iter()
        .filter(|e| e.kind == EntryKind::Outcome && e.provenance.producer == "harness/final")
        .collect();
    assert_eq!(ignored.len(), 1);
}

#[test]
fn the_final_call_is_timed_like_any_other_decide() {
    let l0 = {
        let mut l = limits();
        l.probes = 2;
        l
    };
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l0);
    let (record, _, _) = out_of_probes(Some(truth));
    let mut selects = 0;
    let mut decides = 0;
    let mut sched = 0u64;
    for e in record
        .ledger
        .iter()
        .filter(|e| e.provenance.producer == "harness/timer")
    {
        assert_eq!(e.kind, EntryKind::Measurement);
        let v: serde_json::Value = serde_json::from_slice(&e.payload).unwrap();
        match v["what"].as_str().unwrap() {
            "select" => selects += 1,
            "decide" => decides += 1,
            other => panic!("unexpected timing {other}"),
        }
        sched += v["ns"].as_u64().unwrap();
    }
    assert_eq!((selects, decides), (1, 2), "one step, and the final call");
    assert_eq!(sched, record.measured.sched_ns);
}

// ---- what it costs ----

#[test]
fn the_final_call_is_charged_under_scheduling_when_the_bill_can_pay() {
    let mut l = limits();
    l.compute = 1_000;
    let truth = declare_truth(3, EpisodeClass::Ambiguous, &l);
    let mut policy = ScriptedPolicy::never_decides()
        .with_final(truth)
        .with_final_cost(vec![Charge::new(Resource::Compute, 700)]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert_eq!(record.bill.total(Resource::Compute), 700);
    assert_eq!(
        record.bill.by_phase(Resource::Compute).collect::<Vec<_>>(),
        vec![(Phase::Scheduling, 700)]
    );
}

#[test]
fn a_final_call_the_bill_cannot_pay_for_still_declares() {
    let mut l = limits();
    l.compute = 500;
    let truth = declare_truth(3, EpisodeClass::Ambiguous, &l);
    let mut policy = ScriptedPolicy::never_decides()
        .with_final(truth)
        .with_final_cost(vec![Charge::new(Resource::Compute, 700)]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    assert!(record.verdict.success);
    // Nothing was debited, and the refusal is in the ledger.
    assert_eq!(record.bill.total(Resource::Compute), 0);
    let refused: Vec<_> = record
        .ledger
        .iter()
        .filter(|e| e.kind == EntryKind::Accounting)
        .map(|e| decode_accounting(&e.payload).unwrap())
        .filter(|a| !a.accepted)
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].phase, Phase::Scheduling);
    assert_eq!(
        refused[0].charges,
        vec![Charge::new(Resource::Compute, 700)]
    );
}

#[test]
fn a_declared_time_for_the_final_call_moves_the_clock_before_it_decides() {
    let mut l = limits();
    l.probes = 2;
    let truth = declare_truth(3, EpisodeClass::CriticalFault, &l);
    let site = truth_fault(&spec(3, EpisodeClass::CriticalFault, &l)).1;
    let mut policy = ScriptedPolicy::actions(vec![Action::Probe {
        kind: ProbeKind::LatencySample,
        target: site,
    }])
    .with_final(truth)
    .with_final_cost(vec![Charge::new(Resource::Time, 30 * MS)]);
    let record = play(3, EpisodeClass::CriticalFault, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::FinalDeclaration);
    // 50 ms at the end of the step, 30 ms for the call. LatencySample's 8 ms is billed too.
    assert_eq!(record.verdict.decision_at, Some(Instant(80 * MS)));
    assert_eq!(record.bill.total(Resource::Time), 38 * MS);
}

#[test]
fn a_final_declaration_the_world_refuses_leaves_the_episode_undecided() {
    // A component's busy time carries the last step past the horizon, so its probe is refused as
    // past the horizon; the final call then cannot be earlier than that step and is refused too.
    // The episode stays undecided and nothing is a harness error.
    let l = limits();
    let site = truth_fault(&spec(3, EpisodeClass::Ambiguous, &l)).1;
    let mut script: Vec<ScriptedStep> = (0..200).map(|_| step(&[], None)).collect();
    script.push(step(
        &[0],
        Some(Action::Probe {
            kind: ProbeKind::HealthCheck,
            target: site,
        }),
    ));
    let mut policy = ScriptedPolicy::new(script).with_final(Action::Abstain);
    let (mut components, _) = fakes(1, 10, 100 * MS);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    assert_eq!(record.stop, StopReason::Horizon);
    assert!(record.verdict.undecided);
    assert_eq!(record.trajectory.len(), 2);
    assert!(
        record
            .trajectory
            .iter()
            .all(|s| s.outcome == Outcome::Refused(Refusal::PastHorizon))
    );
}

// ---- the shared rule ----

fn arms() -> Vec<PolicySpec> {
    vec![
        PolicySpec::HeuristicOnly,
        PolicySpec::AllComponents,
        PolicySpec::RandomMatched(policy::random_matched::Config { p: 0.5 }),
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![VERIFIER_ID],
            every: 1,
        }),
    ]
}

fn limits_with_compute(compute: Option<u64>) -> Limits {
    let mut l = limits();
    if let Some(compute) = compute {
        l.compute = compute;
    }
    l
}

fn build_public(spec: &PolicySpec, seed: u64) -> Box<dyn Policy> {
    match policy::build(spec, &DecideConfig::default(), &spec.id().0, seed) {
        Built::Public(p) => p,
        Built::Privileged(_) => panic!("{:?} is privileged", spec.id()),
    }
}

/// A policy that behaves as the wrapped one does until the final call, and then does nothing, for
/// free. The loop with this wrapper is the loop before the final call existed, as far as the
/// rows of `results.csv` go: a zero charge leaves the bill alone, and the call closes nothing.
struct NoFinal(Box<dyn Policy>);

impl Policy for NoFinal {
    fn id(&self) -> PolicyId {
        self.0.id()
    }
    fn decision_rule(&self) -> &'static str {
        self.0.decision_rule()
    }
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.0.declared_select_cost(state)
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.0.select(state, bill)
    }
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        self.0.decide(state, outputs)
    }
    fn declared_final_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }
    fn decide_final(&mut self, _state: &WorkingState) -> Option<Action> {
        None
    }
    fn take_ops(&mut self) -> RuleOps {
        self.0.take_ops()
    }
}

fn play_arm(
    spec: &PolicySpec,
    seed: u64,
    class: EpisodeClass,
    l: &Limits,
    final_call: bool,
) -> EpisodeRecord {
    let inner = build_public(spec, seed);
    let mut policy: Box<dyn Policy> = if final_call {
        inner
    } else {
        Box::new(NoFinal(inner))
    };
    let mut components = standard_components();
    play(seed, class, policy.as_mut(), &mut components, l).unwrap()
}

#[test]
fn with_a_tiny_compute_budget_every_arm_declares_instead_of_ending_undecided() {
    // 60,000 ns was a tiny budget for every arm when this test was written. Since work item A6d
    // (the rule no longer re-narrows at every step) the cheapest arm, `heuristic_only`, declares
    // before it runs out of means at that budget, so the test also runs a budget (10,000 ns) at
    // which every arm does run out. The claim, that no arm ends undecided, is checked at both.
    for spec in arms() {
        let mut final_declarations = 0;
        for compute in [60_000, 10_000] {
            let l = limits_with_compute(Some(compute));
            let mut classes_bound = 0;
            for class in EpisodeClass::ALL {
                let mut in_class = 0;
                for seed in 0..5u64 {
                    let record = play_arm(&spec, seed, class, &l, true);
                    assert!(
                        !record.verdict.undecided,
                        "{:?} {class:?} {seed} compute {compute}: {:?}",
                        spec.id(),
                        record.stop
                    );
                    assert!(record.stop.is_decided());
                    assert!(record.verdict.decision_at.is_some());
                    if record.stop == StopReason::FinalDeclaration {
                        in_class += 1;
                    }
                }
                final_declarations += in_class;
                if in_class >= 3 {
                    classes_bound += 1;
                }
            }
            // `all_components` runs out of means in every class but `DelayedConfigChange`, where
            // it declares early (the review log's probe: 10 of 11 classes, 202 of 220 episodes).
            // The claim is shown on several classes, not on one.
            if spec == PolicySpec::AllComponents {
                assert!(classes_bound >= 9, "{classes_bound} classes at {compute}");
            }
        }
        assert!(
            final_declarations > 0,
            "{:?} never made a final declaration",
            spec.id()
        );
    }
}

#[test]
fn the_final_call_changes_only_the_rows_that_would_have_ended_undecided() {
    let mut differing = 0;
    let mut same_at_default = 0;
    for compute in [None, Some(250_000), Some(60_000)] {
        let l = limits_with_compute(compute);
        for spec in arms() {
            for class in EpisodeClass::ALL {
                for seed in 0..4u64 {
                    let with = play_arm(&spec, seed, class, &l, true);
                    let without = play_arm(&spec, seed, class, &l, false);
                    let (a, b) = (results_row("r", &with), results_row("r", &without));
                    let context = format!("{:?} {class:?} {seed} compute {compute:?}", spec.id());
                    if without.stop.is_decided() {
                        // Decided before the loop stopped: the final call was never made.
                        assert_eq!(a, b, "{context}");
                        assert_eq!(with.stop, without.stop, "{context}");
                        if compute.is_none() {
                            same_at_default += 1;
                        }
                    } else if with.stop == StopReason::FinalDeclaration {
                        differing += 1;
                        assert!(
                            matches!(
                                without.stop,
                                StopReason::BudgetExhausted | StopReason::Horizon
                            ),
                            "{context}: {:?}",
                            without.stop
                        );
                        assert!(without.verdict.undecided, "{context}");
                        // What the final call may change: the verdict, the decision time, the
                        // stop reason and the compute it cost. Nothing the arm did before it.
                        assert_eq!(with.steps, without.steps, "{context}");
                        assert_eq!(with.components_run, without.components_run, "{context}");
                        assert_eq!(
                            with.components_skipped, without.components_skipped,
                            "{context}"
                        );
                        assert_eq!(
                            with.verdict.probes_used, without.verdict.probes_used,
                            "{context}"
                        );
                        assert_eq!(
                            with.verdict.corrections, without.verdict.corrections,
                            "{context}"
                        );
                        for r in [Resource::Probes, Resource::Time, Resource::Memory] {
                            assert_eq!(with.bill.total(r), without.bill.total(r), "{context}");
                        }
                        assert!(
                            with.bill.total(Resource::Compute)
                                >= without.bill.total(Resource::Compute),
                            "{context}"
                        );
                        assert!(
                            with.bill.total(Resource::Compute)
                                - without.bill.total(Resource::Compute)
                                < 1_000_000,
                            "{context}: a final call costs microseconds"
                        );
                        // The trajectory is the old one plus one terminal step.
                        assert_eq!(
                            with.trajectory.len(),
                            without.trajectory.len() + 1,
                            "{context}"
                        );
                        assert_eq!(
                            &with.trajectory[..without.trajectory.len()],
                            &without.trajectory[..],
                            "{context}"
                        );
                        assert_ne!(a, b);
                    } else {
                        panic!(
                            "{context}: undecided without the final call, {:?} with it",
                            with.stop
                        );
                    }
                }
            }
        }
    }
    assert!(same_at_default > 100, "{same_at_default}");
    assert!(differing > 100, "{differing}");
}

#[test]
fn at_the_default_limits_no_arm_makes_a_final_declaration() {
    let l = limits();
    for spec in arms() {
        for class in EpisodeClass::ALL {
            for seed in 0..4u64 {
                let record = play_arm(&spec, seed, class, &l, true);
                assert_eq!(
                    record.stop,
                    StopReason::Terminal,
                    "{:?} {class:?} {seed}",
                    spec.id()
                );
            }
        }
    }
}

// ---- the oracles ----

fn fresh_state(seed: u64, class: EpisodeClass, l: &Limits) -> (WorkingState, Truth) {
    let s = spec(seed, class, l);
    let episode = generate(&s);
    let truth = Truth::from_episode(&episode);
    let sim = Simulator::new(episode);
    (WorkingState::new(sim.public_info(), l.window), truth)
}

#[test]
fn the_oracles_at_the_final_call_are_their_own_deadline() {
    let l = limits();
    let decide = DecideConfig::default();
    for class in EpisodeClass::ALL {
        let (state, truth) = fresh_state(2, class, &l);
        let hypothesis = truth.faults.first().map(|f| (f.kind, f.site));
        // `oracle_immediate` declares the truth at every call, the final one included.
        let mut immediate = OracleFactory::new(Variant::Immediate, decide).build(&truth);
        assert_eq!(immediate.declared_final_cost(&state), zero_cost());
        assert_eq!(
            immediate.decide_final(&state),
            Some(Action::Declare { fault: hypothesis }),
            "{class:?}"
        );
        // `oracle_evidence` has seen nothing, so the evidence identifies nothing: it abstains,
        // as at its patience deadline. It does not read the truth to declare, and does not probe.
        let mut evidence = OracleFactory::new(Variant::Evidence, decide).build(&truth);
        assert_eq!(evidence.declared_final_cost(&state), zero_cost());
        assert_eq!(
            evidence.decide_final(&state),
            Some(Action::Abstain),
            "{class:?}"
        );
    }
}

fn play_oracle(variant: Variant, seed: u64, class: EpisodeClass, l: &Limits) -> EpisodeRecord {
    let factory = OracleFactory::new(variant, DecideConfig::default());
    let mut components = standard_components();
    run_episode_privileged(&spec(seed, class, l), &factory, &mut components, l).unwrap()
}

#[test]
fn the_oracles_are_unaffected_at_the_default_limits() {
    let l = limits();
    for variant in [Variant::Immediate, Variant::Evidence] {
        for class in EpisodeClass::ALL {
            for seed in 0..5u64 {
                let record = play_oracle(variant, seed, class, &l);
                assert_eq!(
                    record.stop,
                    StopReason::Terminal,
                    "{variant:?} {class:?} {seed}"
                );
                assert!(record.verdict.success, "{variant:?} {class:?} {seed}");
            }
        }
    }
}

#[test]
fn the_evidence_oracle_never_declares_what_the_evidence_has_not_identified_even_at_the_final_call()
{
    // A compute allowance too small for any component, and few probes: the ideal observer runs
    // out of affordable work before it has identified the fault.
    let mut finals = 0;
    for probes in [0, 1, 2] {
        let mut l = limits();
        l.compute = 1;
        l.probes = probes;
        for class in EpisodeClass::ALL {
            for seed in 0..6u64 {
                let record = play_oracle(Variant::Evidence, seed, class, &l);
                let v = &record.verdict;
                assert!(!v.undecided, "{class:?} {seed}: {:?}", record.stop);
                // A decided episode is either right or an abstention: never a wrong declaration.
                assert!(v.success || v.abstained, "{class:?} {seed} probes {probes}");
                if record.stop == StopReason::FinalDeclaration {
                    finals += 1;
                    match record.trajectory.last().unwrap().action {
                        Action::Abstain => {}
                        Action::Declare { .. } => assert!(v.success, "{class:?} {seed}"),
                        other => panic!("{other:?}"),
                    }
                }
            }
        }
    }
    assert!(finals > 5, "{finals} final calls: the budget did not bind");
}
