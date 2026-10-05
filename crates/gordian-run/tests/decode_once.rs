//! Work item A6c: the shared rule decodes each component output once.
//!
//! The acceptance of the item is that nothing the rule decides changes, and only what it costs
//! does. `verdicts_at_the_default_budget_are_those_the_rule_gave_before_decode_once` checks that
//! end to end through the recorder; the tests below it check the mechanism and its accounting.

mod common;

use common::*;
use gordian_components::VERIFIER_ID;
use gordian_run::policy::decide::DecideConfig;
use gordian_run::policy::{PolicySpec, fixed_pipeline, random_matched};
use gordian_run::recorder::execute;
use gordian_world::EpisodeClass;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/verdicts-before-decode-once.csv"
);

/// Every arm at the default 20 ms compute budget, 20 seeds by 11 classes, must give for each
/// episode the verdict columns the rule gave before work item A6c. The fixture was written by this
/// test at commit da73030 (the last commit before the change), by running it with
/// `GORDIAN_WRITE_VERDICT_FIXTURE=1`; it is a record of the old behaviour, so it is never
/// regenerated from the new code. At 20 ms no arm is near its limit, so a verdict that moved would
/// mean the rule's decisions had depended on how often it decoded.
#[test]
fn verdicts_at_the_default_budget_are_those_the_rule_gave_before_decode_once() {
    let arms = b1_arms();
    let m = grid_manifest("a6c-verdicts", arms.clone(), 20, 20_000_000);
    let dir = scratch("a6c-verdicts");
    execute(&m, &dir).unwrap();
    let now = verdicts(&dir, &arms);
    assert_eq!(now.len(), arms.len() * 11 * 20);

    if std::env::var_os("GORDIAN_WRITE_VERDICT_FIXTURE").is_some() {
        let mut text = format!("arm,seed,class,{}\n", VERDICT_COLUMNS.join(","));
        for ((arm, seed, class), v) in &now {
            text.push_str(&format!("{arm},{seed},{class},{v}\n"));
        }
        fs::create_dir_all(Path::new(FIXTURE).parent().unwrap()).unwrap();
        fs::write(FIXTURE, text).unwrap();
        return;
    }

    let recorded = fs::read_to_string(FIXTURE).expect("the pre-A6c verdict fixture");
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

use gordian_components::{ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId};
use gordian_run::policy::decide::{Decider, Remaining, RuleOps};
use gordian_run::policy::{self, Built, Policy, PolicyId};
use gordian_run::standard_components;
use gordian_world::{Action, FaultKind};
use std::cell::Cell;
use std::rc::Rc;

/// The first time an output arrives it is decoded and counted; the same output arriving again is
/// recognised, not decoded, not counted as decoded and not declared; an output that differs is
/// decoded again, and so is the first one when it comes back after another.
#[test]
fn an_output_is_decoded_when_it_differs_from_the_one_held_and_not_when_it_equals_it() {
    let state = fresh_state();
    let a = candidates(
        "verifier",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 1),
        ],
    );
    let b = candidates(
        "verifier",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 1),
            fault(FaultKind::ConfigDrift, 2),
        ],
    );
    let mut rule = Decider::default();
    let step = |rule: &mut Decider, out: &ComponentOutput| {
        rule.decide(&state, &[(VERIFIER_ID, out.clone())]);
        let ops = rule.take_ops();
        (
            count(&ops, "decoded_outputs"),
            count(&ops, "decoded_ranked"),
            count(&ops, "compared_bytes"),
            rule.cost_features(&state),
        )
    };
    let (decoded, ranked, compared, f) = step(&mut rule, &a);
    assert_eq!((decoded, ranked, compared), (1, 2, 0), "first arrival");
    assert_eq!((f.decoded_outputs, f.decoded_hypotheses), (1, 2));

    let (decoded, ranked, compared, f) = step(&mut rule, &a);
    assert_eq!((decoded, ranked), (0, 0), "the same output again");
    assert!(compared > 0, "the comparison is counted");
    assert_eq!((f.decoded_outputs, f.decoded_hypotheses), (0, 0));
    assert_eq!(
        f.compared_bytes, compared,
        "and carried into the declared cost"
    );

    let (decoded, ranked, _, _) = step(&mut rule, &b);
    assert_eq!((decoded, ranked), (1, 3), "a different output");
    let (decoded, ranked, _, _) = step(&mut rule, &a);
    assert_eq!((decoded, ranked), (1, 2), "the first again, after another");
    let (decoded, _, _, _) = step(&mut rule, &a);
    assert_eq!(decoded, 0);
}

/// What is held follows what arrived last: after a different output the rule acts on it, and an
/// equal repeat leaves its decision where it was.
#[test]
fn the_decision_follows_the_latest_output_and_a_repeat_changes_nothing() {
    let state = fresh_state();
    let wide = candidates(
        "verifier",
        vec![
            fault(FaultKind::ResourceExhausted, 0),
            fault(FaultKind::ConfigDrift, 1),
        ],
    );
    let one = candidates("verifier", vec![fault(FaultKind::ConfigDrift, 1)]);
    let mut rule = Decider::default();
    let first = rule.decide(&state, &[(VERIFIER_ID, wide.clone())]);
    let repeat = rule.decide(&state, &[(VERIFIER_ID, wide)]);
    assert_eq!(first, repeat);
    assert_eq!(
        rule.decide(&state, &[(VERIFIER_ID, one)]),
        Some(Action::Declare {
            fault: fault(FaultKind::ConfigDrift, 1)
        })
    );
}

/// An output with no entries (a component that ran and found nothing) replaces what was held, as
/// it always did, and repeating it is recognised too.
#[test]
fn an_empty_output_still_replaces_what_was_held() {
    let state = fresh_state();
    let held = candidates("verifier", vec![fault(FaultKind::ConfigDrift, 1)]);
    let mut rule = Decider::default();
    assert!(rule.decide(&state, &[(VERIFIER_ID, held)]).is_some());
    rule.take_ops();
    let nothing = ComponentOutput::default();
    assert_eq!(rule.decide(&state, &[(VERIFIER_ID, nothing.clone())]), None);
    assert_eq!(rule.decide(&state, &[(VERIFIER_ID, nothing)]), None);
    assert_eq!(count(&rule.take_ops(), "decoded_outputs"), 0);
}

/// Totals over a set of episodes, kept in cells so that a policy handed to the harness by value
/// can still report them.
#[derive(Clone, Default)]
struct Totals {
    calls: Rc<Cell<u64>>,
    decoded_arm: Rc<Cell<u64>>,
    decoded_reference: Rc<Cell<u64>>,
    ranked_arm: Rc<Cell<u64>>,
    ranked_reference: Rc<Cell<u64>>,
    declared_arm: Rc<Cell<u64>>,
    declared_reference: Rc<Cell<u64>>,
}

fn add(cell: &Rc<Cell<u64>>, n: u64) {
    cell.set(cell.get() + n);
}

/// Wraps a policy and feeds every call's inputs to the reference rule, which decodes everything it
/// is shown ([`Decider::without_reuse`]). The two must decide the same at every call, on the
/// inputs the arm's own run produces. What they cost may differ, and that is the point, so the
/// work each did, and what each declared, is added up for the test to compare.
struct AgainstReference {
    inner: Box<dyn Policy>,
    reference: Decider,
    totals: Totals,
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
        // cost is the rule's, and the reference rule's is what the old rule would have declared
        // at the same step from the same outputs.
        let got = self.inner.declared_select_cost(state);
        add(
            &self.totals.declared_arm,
            got.iter().map(|c| c.amount).sum(),
        );
        add(
            &self.totals.declared_reference,
            self.reference.declared_cost(state).amount,
        );
        got
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.reference.note_remaining(Remaining::of(bill));
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
        add(&self.totals.calls, 1);
        got
    }
    fn declared_final_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_final_cost(state)
    }
    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        let got = self.inner.decide_final(state);
        let want = self.reference.decide_final(state);
        assert_eq!(got, want, "decide_final differs from the reference rule");
        got
    }
    fn take_ops(&mut self) -> RuleOps {
        let want = self.reference.take_ops();
        let got = self.inner.take_ops();
        let t = &self.totals;
        add(&t.decoded_arm, count(&got, "decoded_outputs"));
        add(&t.decoded_reference, count(&want, "decoded_outputs"));
        add(&t.ranked_arm, count(&got, "decoded_ranked"));
        add(&t.ranked_reference, count(&want, "decoded_ranked"));
        // One call per call. The reference narrows and scores at every call, which the arm no
        // longer does when nothing it narrows against has changed (work item A6d), so the arm's
        // count of those can only be lower; `incremental_narrowing.rs` holds it to the rule as
        // it was after A6c, which does equal work in everything but that.
        assert_eq!(count(&got, "calls"), count(&want, "calls"), "calls");
        for unit in ["worlds", "probe_evals"] {
            assert!(count(&got, unit) <= count(&want, unit), "{unit}");
        }
        assert_eq!(
            count(&want, "compared_bytes"),
            0,
            "the reference compares nothing"
        );
        got
    }
}

/// The acceptance in the small: on the inputs real episodes give it, at the default budget and at
/// two binding ones (where arms run out of affordable work and the final call is made), the rule
/// that recognises a repeated output decides exactly as the rule that decodes everything. It
/// decodes fewer outputs, declares less, and every other counted unit is identical.
#[test]
fn the_rule_decides_as_the_reference_that_decodes_every_output_and_does_less_work() {
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
                        reference: Decider::without_reuse(decide),
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
        "decode-once totals over {} calls: outputs decoded {} against {}, candidates decoded {} against {}, declared {} against {} ns",
        t.calls.get(),
        t.decoded_arm.get(),
        t.decoded_reference.get(),
        t.ranked_arm.get(),
        t.ranked_reference.get(),
        t.declared_arm.get(),
        t.declared_reference.get()
    );
    assert!(t.calls.get() > 1_000, "{} calls", t.calls.get());
    // Not vacuous, and a real saving: outputs repeat, and the old rule decoded them again.
    assert!(
        t.decoded_arm.get() < t.decoded_reference.get()
            && t.ranked_arm.get() < t.ranked_reference.get(),
        "decoded {} against {} outputs, {} against {} candidates",
        t.decoded_arm.get(),
        t.decoded_reference.get(),
        t.ranked_arm.get(),
        t.ranked_reference.get()
    );
    assert!(
        t.declared_arm.get() < t.declared_reference.get(),
        "declared {} against {} ns",
        t.declared_arm.get(),
        t.declared_reference.get()
    );
    // And the decoding still happens: it is not skipped wholesale.
    assert!(
        t.decoded_arm.get() > t.calls.get() / 20,
        "{} decodes",
        t.decoded_arm.get()
    );
}
