//! Work item A6d: the shared rule narrows each stored hypothesis set once, not once per step.
//!
//! The acceptance of the item is that nothing the rule decides changes, and only what it costs
//! does. `verdicts_at_the_default_budget_are_those_the_rule_gave_before_incremental_narrowing`
//! checks that end to end through the recorder against a record written before the change; the
//! tests below it check the mechanism and its accounting.

mod common;

use common::*;
use gordian_run::recorder::execute;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/verdicts-before-incremental-narrowing.csv"
);

/// Every arm at the default 20 ms compute budget, 20 seeds by 11 classes, must give for each
/// episode the verdict columns the rule gave before work item A6d. The fixture was written by this
/// test at the last commit before the rule changed, by running it with
/// `GORDIAN_WRITE_A6D_VERDICT_FIXTURE=1`; it is a record of the old behaviour, so it is never
/// regenerated from the new code. At 20 ms no arm is near its limit, so a verdict that moved would
/// mean the rule's decisions depended on how often it narrowed.
#[test]
fn verdicts_at_the_default_budget_are_those_the_rule_gave_before_incremental_narrowing() {
    let arms = b1_arms();
    let m = grid_manifest("a6d-verdicts", arms.clone(), 20, 20_000_000);
    let dir = scratch("a6d-verdicts");
    execute(&m, &dir).unwrap();
    let now = verdicts(&dir, &arms);
    assert_eq!(now.len(), arms.len() * 11 * 20);

    if std::env::var_os("GORDIAN_WRITE_A6D_VERDICT_FIXTURE").is_some() {
        let mut text = format!("arm,seed,class,{}\n", VERDICT_COLUMNS.join(","));
        for ((arm, seed, class), v) in &now {
            text.push_str(&format!("{arm},{seed},{class},{v}\n"));
        }
        fs::create_dir_all(Path::new(FIXTURE).parent().unwrap()).unwrap();
        fs::write(FIXTURE, text).unwrap();
        return;
    }

    let recorded = fs::read_to_string(FIXTURE).expect("the pre-A6d verdict fixture");
    let mut before = BTreeMap::new();
    for line in recorded.lines().skip(1) {
        let f: Vec<&str> = line.splitn(4, ',').collect();
        before.insert(
            (
                f[0].to_owned(),
                f[1].parse::<u64>().unwrap(),
                f[2].to_owned(),
            ),
            f[3].to_owned(),
        );
    }
    assert_eq!(before.len(), now.len(), "the fixture covers every episode");
    let moved: Vec<String> = now
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, v)| format!("{k:?}: before {:?}, now {v:?}", before.get(k)))
        .collect();
    assert!(
        moved.is_empty(),
        "{} of {} verdicts changed, first: {:#?}",
        moved.len(),
        now.len(),
        &moved[..moved.len().min(10)]
    );
}

// ---- the mechanism and its accounting ----

use gordian_components::{ComponentOutput, ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Instant};
use gordian_run::policy::decide::{DecideConfig, Decider, Remaining, RuleOps};
use gordian_run::policy::{
    self, Built, Policy, PolicyId, PolicySpec, fixed_pipeline, random_matched,
};
use gordian_run::standard_components;
use gordian_world::{
    Action, EpisodeClass, FaultKind, Observation, Probe, ProbeKind, ProbeResult, ServiceId,
};
use std::cell::Cell;
use std::rc::Rc;

fn verifier_set() -> ComponentOutput {
    candidates(
        "verifier",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 1),
            fault(FaultKind::ConfigDrift, 2),
        ],
    )
}

/// Narrowed once and scored once, then neither until the stored set or the probes bought change.
///
/// Three faults and no "no fault": the rule can buy a probe, so it narrows the set and scores
/// every probe. The same call again (nothing arrives, the window is the same) does neither and
/// decides the same; the same output arriving again does neither; a probe result in the window
/// narrows and scores again, once; a different output narrows and scores again, once. The
/// reference ([`Decider::without_cache`]) does both at every call and decides the same.
#[test]
fn a_stored_set_is_narrowed_and_scored_again_only_when_it_or_the_probes_change() {
    let mut state = fresh_state();
    let set = verifier_set();
    let mut rule = Decider::default();
    let mut reference = Decider::without_cache(DecideConfig::default());
    let units = |ops: &RuleOps| (count(ops, "worlds"), count(ops, "probe_evals"));
    let step = |rule: &mut Decider,
                reference: &mut Decider,
                state: &WorkingState,
                outputs: &[(ComponentId, ComponentOutput)]| {
        let got = rule.decide(state, outputs);
        let want = reference.decide(state, outputs);
        assert_eq!(got, want, "the rule decides as the reference");
        (got, rule.take_ops(), reference.take_ops())
    };

    // First call: the set arrives, is narrowed (3 worlds) and its probes scored.
    let (action, ops, reference_ops) = step(
        &mut rule,
        &mut reference,
        &state,
        &[(VERIFIER_ID, set.clone())],
    );
    assert!(matches!(action, Some(Action::Probe { .. })), "{action:?}");
    let (worlds, evals) = units(&ops);
    assert!(worlds >= 3 && evals > 0, "{worlds} worlds, {evals} evals");
    assert_eq!(
        units(&ops),
        units(&reference_ops),
        "the first call is the same work"
    );
    let f = rule.cost_features(&state);
    assert_eq!(f.narrowed_worlds, 3, "three worlds narrowed");
    assert_eq!(f.narrowed_worlds + f.scored_worlds, worlds);
    assert_eq!(f.probe_evals, evals);

    // The next call, nothing new: no narrowing, no scoring, the same action.
    let (again, ops, reference_ops) = step(&mut rule, &mut reference, &state, &[]);
    assert_eq!(again, action);
    assert_eq!(
        units(&ops),
        (0, 0),
        "nothing changed, nothing narrowed or scored"
    );
    assert!(units(&reference_ops).0 >= 3 && units(&reference_ops).1 > 0);
    let f = rule.cost_features(&state);
    assert_eq!(
        (f.narrowed_worlds, f.scored_worlds, f.probe_evals),
        (0, 0, 0)
    );
    // Its declared cost is the base and the scan alone: a fresh rule's. The reference declares the
    // narrowing and the scoring of the set it holds, again.
    assert_eq!(
        rule.declared_cost(&state),
        Decider::default().declared_cost(&state)
    );
    assert!(reference.declared_cost(&state).amount > rule.declared_cost(&state).amount);

    // The same output arriving again is compared, not decoded, and the view is kept.
    let (again, ops, _) = step(
        &mut rule,
        &mut reference,
        &state,
        &[(VERIFIER_ID, set.clone())],
    );
    assert_eq!(again, action);
    assert_eq!(units(&ops), (0, 0));
    assert_eq!(count(&ops, "decoded_outputs"), 0);
    assert!(count(&ops, "compared_bytes") > 0);

    // A probe result in the window changes what the set is narrowed against: once.
    state.admit(
        Instant(1_000_000),
        Observation::Probed {
            probe: Probe {
                kind: ProbeKind::ConfigSnapshot,
                target: ServiceId(1),
            },
            result: ProbeResult::ConfigHash(0),
        },
    );
    let (_, ops, reference_ops) = step(&mut rule, &mut reference, &state, &[]);
    assert!(units(&ops).0 > 0, "a probe result narrows again");
    assert_eq!(
        count(&ops, "decoded_outputs"),
        0,
        "without decoding anything"
    );
    assert_eq!(units(&ops).0, units(&reference_ops).0);
    let (_, ops, _) = step(&mut rule, &mut reference, &state, &[]);
    assert_eq!(units(&ops), (0, 0), "and not again");

    // A different output replaces the set and the view kept with it: once.
    let other = candidates(
        "verifier",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 2),
        ],
    );
    let (_, ops, _) = step(
        &mut rule,
        &mut reference,
        &state,
        &[(VERIFIER_ID, other.clone())],
    );
    assert_eq!(count(&ops, "decoded_outputs"), 1);
    assert!(units(&ops).0 > 0, "a new set is narrowed");
    let (_, ops, _) = step(&mut rule, &mut reference, &state, &[(VERIFIER_ID, other)]);
    assert_eq!(units(&ops), (0, 0), "and kept");
    // The first set coming back after the other is a different set: narrowed once more.
    let (_, ops, _) = step(&mut rule, &mut reference, &state, &[(VERIFIER_ID, set)]);
    assert!(units(&ops).0 > 0);
}

/// A stored set whose narrowing leaves nothing hands the decision to the next stored set, and the
/// empty narrowing is kept as well: the next call does not narrow the first set again.
#[test]
fn an_emptied_set_is_remembered_and_the_next_source_acts() {
    let mut state = fresh_state();
    let verifier = candidates("verifier", vec![fault(FaultKind::ConfigDrift, 1)]);
    let heuristic = candidates(
        "heuristic",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 2),
        ],
    );
    // A correction that says site 1 was not the fault empties the verifier's single world.
    state.admit(
        Instant(1_000_000),
        Observation::Correction {
            site: ServiceId(1),
            resolved: false,
        },
    );
    let mut rule = Decider::default();
    let mut reference = Decider::without_cache(DecideConfig::default());
    let outputs = [(VERIFIER_ID, verifier), (HEURISTIC_ID, heuristic)];
    let got = rule.decide(&state, &outputs);
    assert_eq!(got, reference.decide(&state, &outputs));
    assert!(got.is_some(), "the heuristic's set acts: {got:?}");
    let first = rule.take_ops();
    let again = rule.decide(&state, &[]);
    assert_eq!(again, reference.decide(&state, &[]));
    assert_eq!(again, got);
    let second = rule.take_ops();
    assert!(count(&first, "worlds") > 0);
    assert_eq!(count(&second, "worlds"), 0, "both narrowings are kept");
    assert!(reference.take_ops().total() > 0);
}

/// Totals over a set of episodes, kept in cells so that a policy handed to the harness by value
/// can still report them.
#[derive(Clone, Default)]
struct Totals {
    calls: Rc<Cell<u64>>,
    probing_calls: Rc<Cell<u64>>,
    finals: Rc<Cell<u64>>,
    worlds_arm: Rc<Cell<u64>>,
    worlds_reference: Rc<Cell<u64>>,
    evals_arm: Rc<Cell<u64>>,
    evals_reference: Rc<Cell<u64>>,
    declared_arm: Rc<Cell<u64>>,
    declared_reference: Rc<Cell<u64>>,
    idle_steps: Rc<Cell<u64>>,
    busy_steps: Rc<Cell<u64>>,
}

fn add(cell: &Rc<Cell<u64>>, n: u64) {
    cell.set(cell.get() + n);
}

/// Wraps a policy and feeds every call's inputs to two more rules: `reference`, the rule as it was
/// after A6c ([`Decider::without_cache`]), and `shadow`, a second copy of the rule under test, so
/// that its cost features can be read next to its count. The arm and the reference must decide
/// the same at every call, on the inputs the arm's own run produces. What they cost may differ,
/// and that is the point, so the work each did and what each declared are added up for the test.
struct AgainstReference {
    inner: Box<dyn Policy>,
    reference: Decider,
    shadow: Decider,
    /// What the shadow counted since the harness last took the arm's count.
    shadow_ops: RuleOps,
    totals: Totals,
}

impl AgainstReference {
    /// What the shadow rule's features say about the call it just made must be what its count
    /// says: the declared cost carries exactly the work done. Its count is kept to be compared
    /// with the arm's when the harness takes that.
    fn check_features(&mut self, state: &WorkingState) {
        let ops = self.shadow.take_ops();
        self.shadow_ops.accumulate(&ops);
        let f = self.shadow.cost_features(state);
        assert_eq!(f.decoded_outputs, count(&ops, "decoded_outputs"));
        // The declared cost counts the hypotheses of the leading tie, the count all those listed.
        assert!(f.decoded_hypotheses <= count(&ops, "decoded_ranked"));
        assert_eq!(
            f.decoded_hypotheses == 0,
            count(&ops, "decoded_ranked") == 0
        );
        assert_eq!(f.compared_bytes, count(&ops, "compared_bytes"));
        assert_eq!(f.narrowed_worlds + f.scored_worlds, count(&ops, "worlds"));
        assert_eq!(f.probe_evals, count(&ops, "probe_evals"));
    }
}

impl Policy for AgainstReference {
    fn id(&self) -> PolicyId {
        self.inner.id()
    }
    fn decision_rule(&self) -> &'static str {
        self.inner.decision_rule()
    }
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        // The selectors of the arms tested declare nothing of their own, so the arm's declared
        // cost is the rule's. A step whose previous call did no decoding, comparing, narrowing
        // or scoring is declared the base and the scan alone, which is what a fresh rule
        // declares at this state; a step after any of that is declared more.
        let got = self.inner.declared_select_cost(state);
        let total: u64 = got.iter().map(|c| c.amount).sum();
        assert_eq!(total, self.shadow.declared_cost(state).amount);
        let f = self.shadow.cost_features(state);
        let idle = f.decoded_outputs + f.compared_bytes + f.narrowed_worlds + f.probe_evals == 0;
        let floor = Decider::default().declared_cost(state).amount;
        if idle {
            assert_eq!(total, floor, "no work last call, no work declared");
            add(&self.totals.idle_steps, 1);
        } else {
            assert!(total > floor, "work last call is declared");
            add(&self.totals.busy_steps, 1);
        }
        add(&self.totals.declared_arm, total);
        add(
            &self.totals.declared_reference,
            self.reference.declared_cost(state).amount,
        );
        got
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.reference.note_remaining(Remaining::of(bill));
        self.shadow.note_remaining(Remaining::of(bill));
        self.inner.select(state, bill)
    }
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        let got = self.inner.decide(state, outputs);
        let want = self.reference.decide(state, outputs);
        assert_eq!(got, want, "decide differs from the reference rule");
        assert_eq!(self.shadow.decide(state, outputs), got);
        self.check_features(state);
        add(&self.totals.calls, 1);
        if matches!(got, Some(Action::Probe { .. })) {
            add(&self.totals.probing_calls, 1);
        }
        got
    }
    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge> {
        let got = self.inner.declared_final_cost(state);
        assert_eq!(got, vec![self.shadow.declared_final_cost(state)]);
        got
    }
    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        let got = self.inner.decide_final(state);
        let want = self.reference.decide_final(state);
        assert_eq!(got, want, "decide_final differs from the reference rule");
        assert_eq!(self.shadow.decide_final(state), got);
        self.check_features(state);
        add(&self.totals.finals, 1);
        got
    }
    fn take_ops(&mut self) -> RuleOps {
        let want = self.reference.take_ops();
        let got = self.inner.take_ops();
        let shadow = std::mem::take(&mut self.shadow_ops);
        assert_eq!(got, shadow, "the arm counts what the shared rule counts");
        let t = &self.totals;
        // Nothing the cache touches differs: one call per call, and what is decoded and compared.
        for unit in [
            "calls",
            "decoded_outputs",
            "decoded_ranked",
            "compared_bytes",
        ] {
            assert_eq!(count(&got, unit), count(&want, unit), "{unit}");
        }
        // What it does touch can only fall, call by call.
        for unit in ["worlds", "probe_evals"] {
            assert!(count(&got, unit) <= count(&want, unit), "{unit}");
        }
        add(&t.worlds_arm, count(&got, "worlds"));
        add(&t.worlds_reference, count(&want, "worlds"));
        add(&t.evals_arm, count(&got, "probe_evals"));
        add(&t.evals_reference, count(&want, "probe_evals"));
        got
    }
}

/// The acceptance in the small: on the inputs real episodes give it, at the default budget and at
/// two binding ones (where arms run out of affordable work and the final call is made), the rule
/// that keeps its narrowed views decides exactly as the rule that narrows at every call. It
/// narrows and scores less, declares less, and every other counted unit is identical.
#[test]
fn the_rule_decides_as_the_reference_that_narrows_at_every_call_and_does_less_work() {
    let decide = DecideConfig::default();
    let totals = Totals::default();
    let specs = [
        PolicySpec::HeuristicOnly,
        PolicySpec::AllComponents,
        PolicySpec::RandomMatched(random_matched::Config { p: 0.5 }),
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![VERIFIER_ID],
            every: 1,
        }),
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![ESTIMATOR_ID],
            every: 1,
        }),
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![HEURISTIC_ID],
            every: 4,
        }),
    ];
    for compute in [None, Some(250_000), Some(60_000)] {
        let mut l = limits();
        if let Some(compute) = compute {
            l.compute = compute;
        }
        for spec in &specs {
            for class in EpisodeClass::ALL {
                for seed in 0..4u64 {
                    let Built::Public(inner) = policy::build(spec, &decide, &spec.id().0, seed)
                    else {
                        panic!("{:?} is not public", spec.id());
                    };
                    let mut mirror = AgainstReference {
                        inner,
                        reference: Decider::without_cache(decide),
                        shadow: Decider::new(decide),
                        shadow_ops: RuleOps::ZERO,
                        totals: totals.clone(),
                    };
                    let mut components = standard_components();
                    play(seed, class, &mut mirror, &mut components, &l).unwrap();
                }
            }
        }
    }
    let t = &totals;
    eprintln!(
        "incremental-narrowing totals over {} calls ({} bought a probe, {} final): worlds {} against {}, probe evaluations {} against {}, declared {} against {} ns, {} idle and {} busy steps",
        t.calls.get(),
        t.probing_calls.get(),
        t.finals.get(),
        t.worlds_arm.get(),
        t.worlds_reference.get(),
        t.evals_arm.get(),
        t.evals_reference.get(),
        t.declared_arm.get(),
        t.declared_reference.get(),
        t.idle_steps.get(),
        t.busy_steps.get()
    );
    // Not vacuous: many calls, probes bought, final calls made, and both kinds of step seen.
    assert!(t.calls.get() > 1_000, "{} calls", t.calls.get());
    assert!(
        t.probing_calls.get() > 50,
        "{} probes",
        t.probing_calls.get()
    );
    assert!(t.finals.get() > 50, "{} final calls", t.finals.get());
    assert!(t.idle_steps.get() > 100 && t.busy_steps.get() > 100);
    // And a real saving in both of the repeated terms, and in what is declared.
    assert!(t.worlds_arm.get() < t.worlds_reference.get());
    assert!(t.evals_arm.get() < t.evals_reference.get());
    assert!(t.declared_arm.get() < t.declared_reference.get());
    // The work is still done: not skipped wholesale.
    assert!(t.worlds_arm.get() > t.calls.get() / 20);
}
