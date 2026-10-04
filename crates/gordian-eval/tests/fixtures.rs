//! The hand-written cases in `fixtures/tiny-cases.json`, and checks on the fixtures themselves.
//!
//! The cases describe truth and trajectory directly. Nothing here calls the world's generator.

use gordian_eval::{EvalError, Step, Truth, Verdict, score};
use gordian_world::{Action, EpisodeClass, Outcome, Refusal};
use serde::Deserialize;
use std::collections::BTreeSet;

const FIXTURES: &str = include_str!("../fixtures/tiny-cases.json");
const RULES_MD: &str = include_str!("../RULES.md");

#[derive(Debug, Deserialize)]
enum Expected {
    Verdict(Verdict),
    Error(EvalError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    rules: Vec<String>,
    // Prose for a human reader; the test does not use it.
    #[allow(dead_code)]
    note: String,
    truth: Truth,
    trajectory: Vec<Step>,
    expected: Expected,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURES).expect("fixtures/tiny-cases.json parses")
}

/// Ids of the rules in RULES.md: the first cell of every table row that starts with `| R`.
fn documented_rules() -> BTreeSet<String> {
    RULES_MD
        .lines()
        .filter_map(|l| l.strip_prefix("| R"))
        .filter_map(|rest| rest.split('|').next())
        .map(|id| format!("R{}", id.trim()))
        .collect()
}

#[test]
fn every_fixture_produces_its_expected_result() {
    let mut failures = Vec::new();
    for case in cases() {
        let got = score(&case.truth, &case.trajectory);
        let ok = match (&case.expected, &got) {
            (Expected::Verdict(want), Ok(have)) => want == have,
            (Expected::Error(want), Err(have)) => want == have,
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "case `{}`: expected {:?}, got {:?}",
                case.name, case.expected, got
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn there_are_at_least_thirty_distinct_cases() {
    let cases = cases();
    let names: BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "case names must be unique");
    assert!(cases.len() >= 30, "only {} cases", cases.len());
}

#[test]
fn every_episode_class_appears_in_a_fixture() {
    let seen: BTreeSet<EpisodeClass> = cases().iter().map(|c| c.truth.class).collect();
    for class in EpisodeClass::ALL {
        assert!(seen.contains(&class), "no fixture has class {class:?}");
    }
}

#[test]
fn every_documented_rule_is_pinned_and_every_cited_rule_exists() {
    let documented = documented_rules();
    assert_eq!(documented.len(), 19, "RULES.md should define R1 to R19");
    let mut cited = BTreeSet::new();
    for case in cases() {
        assert!(!case.rules.is_empty(), "case `{}` cites no rule", case.name);
        for rule in case.rules {
            assert!(
                documented.contains(&rule),
                "case `{}` cites unknown rule {rule}",
                case.name
            );
            cited.insert(rule);
        }
    }
    let unpinned: Vec<_> = documented.difference(&cited).collect();
    assert!(
        unpinned.is_empty(),
        "rules pinned by no fixture: {unpinned:?}"
    );
}

#[test]
fn every_error_variant_and_refusal_kind_is_exercised() {
    let cases = cases();
    let mut errors = BTreeSet::new();
    let mut refusals = BTreeSet::new();
    for case in &cases {
        if let Expected::Error(e) = &case.expected {
            errors.insert(match e {
                EvalError::ActionAfterTerminal { .. } => "ActionAfterTerminal",
                EvalError::TimeWentBackwards { .. } => "TimeWentBackwards",
                EvalError::OutcomeMismatch { .. } => "OutcomeMismatch",
                EvalError::EpisodeOverWithoutTerminal { .. } => "EpisodeOverWithoutTerminal",
                EvalError::InvalidTruth { .. } => "InvalidTruth",
            });
        }
        for step in &case.trajectory {
            if let Outcome::Refused(r) = &step.outcome {
                refusals.insert(match r {
                    Refusal::EpisodeOver => "EpisodeOver",
                    Refusal::UnknownService(_) => "UnknownService",
                    Refusal::BudgetExceeded { .. } => "BudgetExceeded",
                    Refusal::TimeWentBackwards => "TimeWentBackwards",
                    Refusal::PastHorizon => "PastHorizon",
                });
            }
        }
    }
    assert_eq!(errors.len(), 5, "error variants exercised: {errors:?}");
    // TimeWentBackwards as a *refusal* is deliberately not in a fixture: a trajectory holding it
    // at a decreasing instant is rejected (R15), and at a non-decreasing instant it is an
    // ordinary refusal covered by the other refusal kinds. See the dedicated test below.
    for kind in [
        "EpisodeOver",
        "UnknownService",
        "BudgetExceeded",
        "PastHorizon",
    ] {
        assert!(refusals.contains(kind), "no fixture refuses with {kind}");
    }
}

#[test]
fn a_time_went_backwards_refusal_at_a_nondecreasing_instant_is_an_ordinary_refusal() {
    // The simulator can refuse with TimeWentBackwards against an `observe_until` time that the
    // trajectory does not contain (RULES.md, "How the world's types were read").
    let case = cases()
        .into_iter()
        .find(|c| c.name == "ambiguous/probe-then-right-declaration")
        .expect("case exists");
    let mut traj = case.trajectory.clone();
    traj.insert(
        1,
        Step {
            at: traj[0].at,
            action: Action::Abstain,
            outcome: Outcome::Refused(Refusal::TimeWentBackwards),
        },
    );
    let with = score(&case.truth, &traj).expect("scores");
    let without = score(&case.truth, &case.trajectory).expect("scores");
    assert_eq!(with, without);
}

#[test]
fn fixtures_survive_a_serialization_round_trip() {
    for case in cases() {
        let truth: Truth =
            serde_json::from_str(&serde_json::to_string(&case.truth).unwrap()).unwrap();
        assert_eq!(truth, case.truth, "truth of `{}`", case.name);
        for step in &case.trajectory {
            let back: Step = serde_json::from_str(&serde_json::to_string(step).unwrap()).unwrap();
            assert_eq!(&back, step, "a step of `{}`", case.name);
        }
        if let Expected::Verdict(v) = &case.expected {
            let json = serde_json::to_string(v).unwrap();
            let back: Verdict = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, v, "verdict of `{}`", case.name);
        }
        if let Expected::Error(e) = &case.expected {
            let back: EvalError = serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap();
            assert_eq!(&back, e, "error of `{}`", case.name);
        }
    }
}

#[test]
fn instants_are_written_as_plain_nanoseconds() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "ambiguous/probe-then-right-declaration")
        .expect("case exists");
    let step = serde_json::to_value(&case.trajectory[1]).unwrap();
    assert_eq!(step["at"], serde_json::json!(4_000_000));
    let Expected::Verdict(v) = case.expected else {
        panic!("expected a verdict");
    };
    let verdict = serde_json::to_value(&v).unwrap();
    assert_eq!(verdict["decision_at"], serde_json::json!(4_000_000));
    let undecided = score(&case.truth, &[]).unwrap();
    let undecided = serde_json::to_value(&undecided).unwrap();
    assert_eq!(undecided["decision_at"], serde_json::Value::Null);
}

#[test]
fn error_messages_name_the_offending_step() {
    let cases = [
        (
            EvalError::ActionAfterTerminal { index: 7 },
            "step 7 follows",
        ),
        (
            EvalError::TimeWentBackwards { index: 8 },
            "step 8 is earlier",
        ),
        (
            EvalError::OutcomeMismatch { index: 9 },
            "step 9 has an outcome",
        ),
        (
            EvalError::EpisodeOverWithoutTerminal { index: 10 },
            "step 10 was refused",
        ),
        (
            EvalError::InvalidTruth {
                class: EpisodeClass::NoFault,
                fault_count: 3,
            },
            "3 fault(s)",
        ),
    ];
    for (error, needle) in cases {
        let text = error.to_string();
        assert!(text.contains(needle), "`{text}` lacks `{needle}`");
        let as_dyn: &dyn std::error::Error = &error;
        assert_eq!(as_dyn.to_string(), text);
    }
    let text = EvalError::InvalidTruth {
        class: EpisodeClass::NoFault,
        fault_count: 3,
    }
    .to_string();
    assert!(text.contains("NoFault"));
}
