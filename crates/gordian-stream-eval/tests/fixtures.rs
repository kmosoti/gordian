//! The hand-written cases in `fixtures/stream-cases.json`, and checks on the fixtures themselves.
//!
//! The cases describe truth, trajectory and calls directly, and give each expected verdict as a
//! literal worked out by hand from `RULES.md`. Nothing here calls the stream's generator.

mod common;

use common::CompactTruth;
use gordian_stream::{StreamAction, StreamOutcome, Tier};
use gordian_stream_eval::{CallSummary, StreamEvalError, StreamStep, StreamVerdict, score_stream};
use serde::Deserialize;
use std::collections::BTreeSet;

const FIXTURES: &str = include_str!("../fixtures/stream-cases.json");
const RULES_MD: &str = include_str!("../RULES.md");

#[derive(Debug, Deserialize)]
enum Expected {
    Verdict(StreamVerdict),
    Error(StreamEvalError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    rules: Vec<String>,
    // Prose for a human reader; the test does not use it.
    #[allow(dead_code)]
    note: String,
    truth: CompactTruth,
    trajectory: Vec<StreamStep>,
    calls: Vec<CallSummary>,
    expected: Expected,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURES).expect("fixtures/stream-cases.json parses")
}

/// Ids of the rules in RULES.md: the first cell of every table row that starts with `| S`.
fn documented_rules() -> BTreeSet<String> {
    RULES_MD
        .lines()
        .filter_map(|l| l.strip_prefix("| S"))
        .filter_map(|rest| rest.split('|').next())
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| format!("S{id}"))
        .collect()
}

#[test]
fn every_fixture_produces_its_expected_result() {
    let mut failures = Vec::new();
    for case in cases() {
        let truth = case.truth.build();
        let got = score_stream(&truth, &case.trajectory, &case.calls);
        let ok = match (&case.expected, &got) {
            (Expected::Verdict(want), Ok(have)) => want == have,
            (Expected::Error(want), Err(have)) => want == have,
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "case `{}`:\n  expected {:?}\n  got      {:?}",
                case.name, case.expected, got
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn there_are_at_least_forty_distinct_cases() {
    let cases = cases();
    let names: BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "case names must be unique");
    assert!(cases.len() >= 40, "only {} cases", cases.len());
}

#[test]
fn every_documented_rule_is_pinned_and_every_cited_rule_exists() {
    let documented = documented_rules();
    assert_eq!(documented.len(), 37, "RULES.md should define S1 to S37");
    for n in 1..=37 {
        assert!(documented.contains(&format!("S{n}")), "S{n} is missing");
    }
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
fn every_error_variant_is_exercised() {
    let mut seen = BTreeSet::new();
    for case in cases() {
        if let Expected::Error(e) = &case.expected {
            seen.insert(match e {
                StreamEvalError::IncidentIdMismatch { .. } => "IncidentIdMismatch",
                StreamEvalError::TierContradictsTruth { .. } => "TierContradictsTruth",
                StreamEvalError::LabelOfUnknownIncident { .. } => "LabelOfUnknownIncident",
                StreamEvalError::TimeWentBackwards { .. } => "TimeWentBackwards",
                StreamEvalError::OutcomeMismatch { .. } => "OutcomeMismatch",
                StreamEvalError::ActionAfterEnd { .. } => "ActionAfterEnd",
                StreamEvalError::UnknownObservation { .. } => "UnknownObservation",
                StreamEvalError::ObservationNotYetEmitted { .. } => "ObservationNotYetEmitted",
                StreamEvalError::CallRecordMismatch { .. } => "CallRecordMismatch",
                StreamEvalError::CallCountMismatch { .. } => "CallCountMismatch",
            });
        }
    }
    assert_eq!(seen.len(), 10, "error variants exercised: {seen:?}");
}

#[test]
fn every_tier_appears_in_a_case_in_each_outcome_that_applies_to_it() {
    let mut plain_correct = false;
    let mut plain_missed = false;
    let mut hard_correct = false;
    let mut hard_missed = false;
    let mut critical_missed = BTreeSet::new();
    let mut decoy_dismissed = false;
    let mut decoy_alarmed = false;
    let mut decoy_silent = false;
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        for i in &v.per_incident {
            match i.tier {
                Tier::Plain => {
                    plain_correct |= i.correct_by_deadline;
                    plain_missed |= i.missed;
                }
                Tier::Hard => {
                    hard_correct |= i.correct_by_deadline;
                    hard_missed |= i.missed;
                }
                Tier::Decoy => {
                    decoy_dismissed |= i.correct_declarations > 0;
                    decoy_alarmed |= i.wrong_declarations > 0;
                    decoy_silent |= i.correct_declarations + i.wrong_declarations == 0;
                }
            }
            if i.critical_miss {
                critical_missed.insert(i.tier);
            }
        }
    }
    assert!(plain_correct && plain_missed, "plain outcomes not covered");
    assert!(hard_correct && hard_missed, "hard outcomes not covered");
    assert!(
        critical_missed.contains(&Tier::Plain) && critical_missed.contains(&Tier::Hard),
        "critical misses not covered for both tiers"
    );
    assert!(
        decoy_dismissed && decoy_alarmed && decoy_silent,
        "decoy outcomes not covered"
    );
}

#[test]
fn the_cases_cover_the_late_informed_and_escalation_situations_the_plan_names() {
    let names: Vec<String> = cases().into_iter().map(|c| c.name).collect();
    for needle in [
        "late",                    // late correct declarations
        "declared-as-an-incident", // a decoy declared as an incident
        "dismissed",               // a decoy declared "not an incident"
        "needed",                  // a needed escalation
        "unneeded",                // an unneeded escalation
        "informed-and-uninformed", // an informed and an uninformed call
    ] {
        assert!(
            names.iter().any(|n| n.contains(needle)),
            "no case name mentions `{needle}`"
        );
    }
}

#[test]
fn accepted_probe_escalate_and_declare_steps_all_appear() {
    let mut probes = 0;
    let mut escalations = 0;
    let mut declarations = 0;
    let mut refusals = 0;
    for case in cases() {
        for s in &case.trajectory {
            match (&s.action, &s.outcome) {
                (_, StreamOutcome::Refused(_)) => refusals += 1,
                (StreamAction::Probe { .. }, StreamOutcome::Probed { .. }) => probes += 1,
                (StreamAction::Escalate { .. }, StreamOutcome::Escalated { .. }) => {
                    escalations += 1
                }
                (StreamAction::Declare { .. }, StreamOutcome::Declared { .. }) => declarations += 1,
                _ => {}
            }
        }
    }
    assert!(probes > 0 && escalations > 0 && declarations > 0 && refusals > 0);
}

/// An expected verdict that contradicts itself would mean the fixture, not the scorer, is wrong.
/// These identities are stated from `RULES.md`, not read out of `score_stream`.
#[test]
fn expected_verdicts_are_internally_consistent() {
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        let t = &v.totals;
        let name = &case.name;
        let tier = |x: Tier| v.per_incident.iter().filter(move |i| i.tier == x).count() as u32;
        assert_eq!(t.incidents.plain, tier(Tier::Plain), "{name}");
        assert_eq!(t.incidents.hard, tier(Tier::Hard), "{name}");
        assert_eq!(t.incidents.decoy, tier(Tier::Decoy), "{name}");
        assert_eq!(
            t.critical_incidents,
            v.per_incident.iter().filter(|i| i.critical).count() as u32,
            "{name}"
        );
        assert_eq!(v.per_incident.len(), case.truth.incidents.len(), "{name}");
        // S23: every escalation has exactly one class.
        let e = &t.escalations;
        assert_eq!(
            u64::from(e.needed + e.unneeded + e.background),
            t.reasoner.calls,
            "{name}"
        );
        assert_eq!(t.reasoner.calls as usize, case.calls.len(), "{name}");
        // Escalations by incident plus background make all calls.
        let per: u32 = v.per_incident.iter().map(|i| i.escalations).sum();
        assert_eq!(
            per + e.background,
            e.needed + e.unneeded + e.background,
            "{name}"
        );
        // S25: totals count every call, so they are at least the per-incident sums.
        assert!(
            v.per_incident
                .iter()
                .map(|i| i.informed_escalations)
                .sum::<u32>()
                <= e.informed,
            "{name}"
        );
        assert_eq!(
            e.informed as usize,
            case.calls.iter().filter(|c| c.informed).count(),
            "{name}"
        );
        assert_eq!(
            e.correct as usize,
            case.calls.iter().filter(|c| c.correct).count(),
            "{name}"
        );
        // S9, S10, S16: only plain and hard incidents are missed; decoys never.
        for i in &v.per_incident {
            assert_eq!(
                i.missed,
                i.tier != Tier::Decoy && !i.correct_by_deadline,
                "{name}"
            );
            assert_eq!(i.critical_miss, i.critical && i.missed, "{name}");
            assert!(!(i.correct_by_deadline && i.tier == Tier::Decoy), "{name}");
            assert_eq!(
                i.first_correct_at.is_some(),
                i.correct_declarations > 0,
                "{name}"
            );
            assert_eq!(
                i.first_correct_at.is_some(),
                i.time_to_first_correct_ns.is_some(),
                "{name}"
            );
        }
        // S20.
        let sum = |f: &dyn Fn(&gordian_stream_eval::IncidentVerdict) -> bool, tier: Tier| {
            v.per_incident
                .iter()
                .filter(|i| i.tier == tier && f(i))
                .count() as u32
        };
        assert_eq!(
            t.correct.plain,
            sum(&|i| i.correct_by_deadline, Tier::Plain),
            "{name}"
        );
        assert_eq!(
            t.correct.hard,
            sum(&|i| i.correct_by_deadline, Tier::Hard),
            "{name}"
        );
        assert_eq!(t.missed.plain, sum(&|i| i.missed, Tier::Plain), "{name}");
        assert_eq!(t.missed.hard, sum(&|i| i.missed, Tier::Hard), "{name}");
        assert_eq!(
            t.critical_missed.plain,
            sum(&|i| i.critical_miss, Tier::Plain),
            "{name}"
        );
        assert_eq!(
            t.critical_missed.hard,
            sum(&|i| i.critical_miss, Tier::Hard),
            "{name}"
        );
        assert_eq!(
            t.wrong_declarations,
            v.per_incident
                .iter()
                .filter(|i| i.tier != Tier::Decoy)
                .map(|i| i.wrong_declarations)
                .sum::<u32>(),
            "{name}"
        );
        // S21, S22.
        let decoys = || v.per_incident.iter().filter(|i| i.tier == Tier::Decoy);
        assert_eq!(
            t.decoys_dismissed,
            decoys().filter(|i| i.correct_declarations > 0).count() as u32,
            "{name}"
        );
        assert_eq!(
            t.decoys_alarmed,
            decoys().filter(|i| i.wrong_declarations > 0).count() as u32,
            "{name}"
        );
        assert_eq!(
            t.decoys_silent,
            decoys()
                .filter(|i| i.correct_declarations + i.wrong_declarations == 0)
                .count() as u32,
            "{name}"
        );
        assert_eq!(
            t.false_alarms,
            decoys().map(|i| i.wrong_declarations).sum::<u32>() + t.false_alarms_on_background,
            "{name}"
        );
    }
}

#[test]
fn fixtures_survive_a_serialization_round_trip() {
    for case in cases() {
        for step in &case.trajectory {
            let back: StreamStep =
                serde_json::from_str(&serde_json::to_string(step).unwrap()).unwrap();
            assert_eq!(&back, step, "a step of `{}`", case.name);
        }
        for call in &case.calls {
            let back: CallSummary =
                serde_json::from_str(&serde_json::to_string(call).unwrap()).unwrap();
            assert_eq!(&back, call, "a call of `{}`", case.name);
        }
        match &case.expected {
            Expected::Verdict(v) => {
                let back: StreamVerdict =
                    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap();
                assert_eq!(&back, v, "verdict of `{}`", case.name);
            }
            Expected::Error(e) => {
                let back: StreamEvalError =
                    serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap();
                assert_eq!(&back, e, "error of `{}`", case.name);
            }
        }
    }
}

#[test]
fn instants_are_written_as_plain_nanoseconds() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "plain/correct-on-time")
        .expect("case exists");
    let step = serde_json::to_value(&case.trajectory[0]).unwrap();
    assert_eq!(step["at"], serde_json::json!(12_000_000_000u64));
    let Expected::Verdict(v) = case.expected else {
        panic!("expected a verdict");
    };
    let verdict = serde_json::to_value(&v).unwrap();
    assert_eq!(
        verdict["per_incident"][0]["first_correct_at"],
        serde_json::json!(12_000_000_000u64)
    );
    let silent = cases()
        .into_iter()
        .find(|c| c.name == "plain/silent-is-missed")
        .expect("case exists");
    let verdict = score_stream(&silent.truth.build(), &[], &[]).unwrap();
    let verdict = serde_json::to_value(&verdict).unwrap();
    assert_eq!(
        verdict["per_incident"][0]["first_correct_at"],
        serde_json::Value::Null
    );
    let with_call = cases()
        .into_iter()
        .find(|c| c.name == "escalation/needed-hard")
        .expect("case exists");
    let call = serde_json::to_value(with_call.calls[0]).unwrap();
    assert_eq!(call["at"], serde_json::json!(20_000_000_000u64));
    assert_eq!(call["ready_at"], serde_json::json!(22_006_000_000u64));
}

#[test]
fn error_messages_name_the_offending_step() {
    use gordian_stream::ObsId;
    let cases = [
        (
            StreamEvalError::IncidentIdMismatch { index: 3, id: 9 },
            "position 3 has id 9",
        ),
        (
            StreamEvalError::TierContradictsTruth { incident: 4 },
            "incident 4",
        ),
        (
            StreamEvalError::LabelOfUnknownIncident {
                obs: ObsId(5),
                incident: 8,
            },
            "observation 5 is labelled with unknown incident 8",
        ),
        (
            StreamEvalError::TimeWentBackwards { index: 7 },
            "step 7 is earlier",
        ),
        (
            StreamEvalError::OutcomeMismatch { index: 8 },
            "step 8 has an outcome",
        ),
        (
            StreamEvalError::ActionAfterEnd { index: 9 },
            "step 9 was accepted after the end",
        ),
        (
            StreamEvalError::UnknownObservation {
                index: 10,
                obs: ObsId(11),
            },
            "step 10 names observation 11, which does not exist",
        ),
        (
            StreamEvalError::ObservationNotYetEmitted {
                index: 12,
                obs: ObsId(13),
            },
            "step 12 names observation 13, whose incident had not begun",
        ),
        (
            StreamEvalError::CallRecordMismatch { index: 14, call: 2 },
            "step 14 (call 2)",
        ),
        (
            StreamEvalError::CallCountMismatch {
                escalations: 3,
                calls: 5,
            },
            "3 accepted escalation(s) but 5 call summary",
        ),
    ];
    for (error, needle) in cases {
        let text = error.to_string();
        assert!(text.contains(needle), "`{text}` lacks `{needle}`");
        let as_dyn: &dyn std::error::Error = &error;
        assert_eq!(as_dyn.to_string(), text);
    }
}
