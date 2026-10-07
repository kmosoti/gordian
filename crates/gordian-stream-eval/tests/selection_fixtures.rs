//! The hand-written cases in `fixtures/selection-cases.json` (work item B4), and checks on the
//! fixtures themselves.
//!
//! The cases describe the truth and the record of notices, retirements and escalations directly,
//! and give each expected verdict as a literal worked out by hand from `RULES.md`, section
//! "Selection accounting". Nothing here calls the stream's generator.

mod common;

use common::CompactTruth;
use gordian_stream_eval::{
    IncidentClass, SelectionError, SelectionTrace, SelectionVerdict, score_selection,
};
use serde::Deserialize;
use std::collections::BTreeSet;

const FIXTURES: &str = include_str!("../fixtures/selection-cases.json");
const RULES_MD: &str = include_str!("../RULES.md");

// A few dozen of these are read once per test run; the size difference between the variants costs nothing, and
// boxing the verdict would only make the comparison below compare a box with a reference.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Deserialize)]
enum Expected {
    Verdict(SelectionVerdict),
    Error(SelectionError),
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
    trace: SelectionTrace,
    expected: Expected,
}

impl Case {
    fn score(&self) -> Result<SelectionVerdict, SelectionError> {
        score_selection(&self.truth.build(), &self.trace)
    }
}

fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURES).expect("fixtures/selection-cases.json parses")
}

/// Ids of the selection rules in RULES.md: the first cell of every table row that starts with `| E`.
fn documented_rules() -> BTreeSet<String> {
    RULES_MD
        .lines()
        .filter_map(|l| l.strip_prefix("| E"))
        .filter_map(|rest| rest.split('|').next())
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| format!("E{id}"))
        .collect()
}

#[test]
fn every_fixture_produces_its_expected_result() {
    let mut failures = Vec::new();
    for case in cases() {
        let got = case.score();
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
fn case_names_are_unique_and_there_are_enough_of_them() {
    let cases = cases();
    let names: BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "case names must be unique");
    assert!(cases.len() >= 10, "only {} cases", cases.len());
}

#[test]
fn every_documented_rule_is_pinned_and_every_cited_rule_exists() {
    let documented = documented_rules();
    assert_eq!(documented.len(), 8, "RULES.md should define E1 to E8");
    for n in 1..=8 {
        assert!(documented.contains(&format!("E{n}")), "E{n} is missing");
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
fn every_error_variant_is_exercised_and_says_what_it_is() {
    let mut seen = BTreeSet::new();
    for case in cases() {
        if let Expected::Error(e) = &case.expected {
            seen.insert(match e {
                SelectionError::UnknownObservation { .. } => "UnknownObservation",
                SelectionError::RetirementWithoutNotice { .. } => "RetirementWithoutNotice",
            });
            let text = e.to_string();
            assert!(text.len() > 10, "{text}");
        }
    }
    assert_eq!(seen.len(), 2, "error variants exercised: {seen:?}");
    let text = SelectionError::RetirementWithoutNotice { anomaly: 7 }.to_string();
    assert!(text.contains('7') && text.contains("retirement"), "{text}");
    let text = SelectionError::UnknownObservation {
        obs: gordian_stream::ObsId(9),
    }
    .to_string();
    assert!(text.contains('9'), "{text}");
}

#[test]
fn every_class_appears_and_both_ways_of_retiring_do() {
    let mut classes = BTreeSet::new();
    let (mut followup, mut quiet, mut before, mut after_call) = (false, false, false, false);
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        for o in &v.per_notice {
            classes.insert(format!("{:?}", o.class));
            followup |= o.followup;
            quiet |= o.retired_at.is_some() && !o.followup;
            before |= o.retired_before_escalation;
            after_call |= o.retired_at.is_some() && o.escalations > 0;
        }
    }
    assert_eq!(classes.len(), IncidentClass::ALL.len(), "{classes:?}");
    assert!(followup && quiet && before && after_call);
}

/// An expected verdict that contradicts itself would mean the fixture, not the scorer, is wrong.
/// These identities are stated from `RULES.md`, not read out of `score_selection`.
#[test]
fn expected_verdicts_are_internally_consistent() {
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        let name = &case.name;
        // E3: one outcome per notice recorded; E7: the notices by class add to them.
        assert_eq!(v.per_notice.len(), case.trace.notices.len(), "{name}");
        let n = &v.notices;
        for class in IncidentClass::ALL {
            let in_class: Vec<_> = v.per_notice.iter().filter(|o| o.class == class).collect();
            assert_eq!(
                n.notices.get(class) as usize,
                in_class.len(),
                "{name} {class:?}"
            );
            assert_eq!(
                n.escalated.get(class) as usize,
                in_class.iter().filter(|o| o.escalations > 0).count(),
                "{name} {class:?}"
            );
            assert_eq!(
                n.retired_before_escalation.get(class) as usize,
                in_class
                    .iter()
                    .filter(|o| o.retired_before_escalation)
                    .count(),
                "{name} {class:?}"
            );
            assert_eq!(
                n.followup_retired.get(class) as usize,
                in_class.iter().filter(|o| o.followup).count(),
                "{name} {class:?}"
            );
            assert_eq!(
                n.followup_before_escalation.get(class) as usize,
                in_class
                    .iter()
                    .filter(|o| o.followup && o.retired_before_escalation)
                    .count(),
                "{name} {class:?}"
            );
            // E4: retired before escalation is retired and not asked about.
            for o in in_class {
                assert_eq!(
                    o.retired_before_escalation,
                    o.retired_at.is_some() && o.escalations == 0,
                    "{name}"
                );
                // E5: a retirement by the rule is a retirement.
                assert!(!o.followup || o.retired_at.is_some(), "{name}");
                // The first escalation exists exactly when there is one.
                assert_eq!(o.first_escalation_at.is_some(), o.escalations > 0, "{name}");
            }
        }
        // E2: the calls by class are the escalations recorded; the cost by class is their sum.
        let e = &v.escalations;
        let calls: u32 = IncidentClass::ALL.iter().map(|c| e.calls.get(*c)).sum();
        assert_eq!(calls as usize, case.trace.escalations.len(), "{name}");
        let tokens: u64 = IncidentClass::ALL.iter().map(|c| e.tokens.get(*c)).sum();
        assert_eq!(
            tokens,
            case.trace.escalations.iter().map(|x| x.tokens).sum::<u64>(),
            "{name}"
        );
        let ns: u64 = IncidentClass::ALL
            .iter()
            .map(|c| e.modelled_ns.get(*c))
            .sum();
        assert_eq!(
            ns,
            case.trace
                .escalations
                .iter()
                .map(|x| x.modelled_ns)
                .sum::<u64>(),
            "{name}"
        );
        // E1: the escalations about a notice and the unattributed ones are all the escalations.
        let about: u32 = v.per_notice.iter().map(|o| o.escalations).sum();
        assert_eq!(about + e.unattributed, calls, "{name}");
    }
}

#[test]
fn fixtures_survive_a_serialization_round_trip() {
    for case in cases() {
        if let Expected::Verdict(v) = &case.expected {
            let text = serde_json::to_string(v).unwrap();
            let back: SelectionVerdict = serde_json::from_str(&text).unwrap();
            assert_eq!(&back, v, "{}", case.name);
        }
        let text = serde_json::to_string(&case.trace).unwrap();
        let back: SelectionTrace = serde_json::from_str(&text).unwrap();
        assert_eq!(back, case.trace, "{}", case.name);
    }
}

#[test]
fn the_class_words_are_the_ones_the_run_output_writes_in_the_order_the_counts_are() {
    // `selection_notices.csv` writes a notice's class with these words, and the analysis reads them.
    let words: Vec<&str> = IncidentClass::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(words, ["background", "plain", "hard", "leak", "decoy"]);
}
