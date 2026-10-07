//! The hand-written cases in `fixtures/memory-cases.json` (work item E1), and checks on the
//! fixtures themselves.
//!
//! The cases describe the truth, the trajectory and the record of recalls directly, and give each
//! expected verdict as a literal worked out by hand from `RULES.md`, section "Memory". Nothing
//! here calls the stream's generator.

mod common;

use common::CompactTruth;
use gordian_stream::{ObsId, Tier};
use gordian_stream_eval::{
    MemoryError, MemoryVerdict, RecallEntry, SourceClass, StreamStep, score_memory,
};
use serde::Deserialize;
use std::collections::BTreeSet;

const FIXTURES: &str = include_str!("../fixtures/memory-cases.json");
const RULES_MD: &str = include_str!("../RULES.md");

#[derive(Debug, Deserialize)]
enum Expected {
    Verdict(MemoryVerdict),
    Error(MemoryError),
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
    recalls: Vec<RecallEntry>,
    expected: Expected,
}

impl Case {
    fn score(&self) -> Result<MemoryVerdict, MemoryError> {
        score_memory(&self.truth.build(), &self.trajectory, &self.recalls)
    }
}

fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURES).expect("fixtures/memory-cases.json parses")
}

/// Ids of the memory rules in RULES.md: the first cell of every table row that starts with `| K`.
fn documented_rules() -> BTreeSet<String> {
    RULES_MD
        .lines()
        .filter_map(|l| l.strip_prefix("| K"))
        .filter_map(|rest| rest.split('|').next())
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| format!("K{id}"))
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
    assert!(cases.len() >= 20, "only {} cases", cases.len());
}

#[test]
fn every_documented_rule_is_pinned_and_every_cited_rule_exists() {
    let documented = documented_rules();
    assert_eq!(documented.len(), 10, "RULES.md should define K1 to K10");
    for n in 1..=10 {
        assert!(documented.contains(&format!("K{n}")), "K{n} is missing");
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
                MemoryError::StepOutOfRange { .. } => "StepOutOfRange",
                MemoryError::NotADeclaration { .. } => "NotADeclaration",
                MemoryError::StepRepeated { .. } => "StepRepeated",
                MemoryError::UnknownObservation { .. } => "UnknownObservation",
            });
            assert!(e.to_string().len() > 10, "{e}");
        }
    }
    assert_eq!(seen.len(), 4, "error variants exercised: {seen:?}");
    let text = MemoryError::StepRepeated { step: 7 }.to_string();
    assert!(text.contains('7') && text.contains("recalls"), "{text}");
    let text = MemoryError::UnknownObservation {
        step: 2,
        obs: ObsId(9),
    }
    .to_string();
    assert!(text.contains('9') && text.contains('2'), "{text}");
}

#[test]
fn every_cell_and_every_anchor_kind_appears() {
    let mut cells = BTreeSet::new();
    let (mut plain, mut hard, mut decoy, mut background) = (false, false, false, false);
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        let t = &v.totals;
        for (name, n) in [
            ("correct_source_right", t.recalls.correct_source_right),
            ("correct_source_wrong", t.recalls.correct_source_wrong),
            ("correct_source_unknown", t.recalls.correct_source_unknown),
            ("wrong_source_right", t.recalls.wrong_source_right),
            ("wrong_source_wrong", t.recalls.wrong_source_wrong),
            ("wrong_source_unknown", t.recalls.wrong_source_unknown),
        ] {
            if n > 0 {
                cells.insert(name);
            }
        }
        plain |= t.recalls_on_plain.total() > 0;
        hard |= t.recalls_on_hard.total() > 0;
        decoy |= t.recalls_on_decoy.total() > 0;
        background |= t.recalls_on_background.total() > 0;
    }
    assert_eq!(cells.len(), 6, "{cells:?}");
    assert!(plain && hard && decoy && background);
}

#[test]
fn every_tier_has_an_unasked_correct_and_an_unasked_wrong_case() {
    let (mut uc, mut uw) = (BTreeSet::new(), BTreeSet::new());
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        for i in &v.per_incident {
            if i.unasked_correct {
                uc.insert(format!("{:?}", i.tier));
            }
            if i.unasked_wrong {
                uw.insert(format!("{:?}", i.tier));
            }
        }
    }
    assert_eq!(uc.len(), 3, "{uc:?}");
    assert_eq!(uw.len(), 3, "{uw:?}");
}

/// An expected verdict that contradicts itself would mean the fixture, not the scorer, is wrong.
/// These identities are stated from `RULES.md`, not read out of `score_memory`.
#[test]
fn expected_verdicts_are_internally_consistent() {
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        let name = &case.name;
        assert_eq!(v.per_incident.len(), case.truth.incidents.len(), "{name}");
        let t = &v.totals;
        // K7: one row per recall in the cells, and the anchors' cells add to the total.
        assert_eq!(t.recalls.total() as usize, case.recalls.len(), "{name}");
        let anchored = t.recalls_on_plain.total()
            + t.recalls_on_hard.total()
            + t.recalls_on_decoy.total()
            + t.recalls_on_background.total();
        assert_eq!(anchored, t.recalls.total(), "{name}");
        // K5: one score per recall, in the order recorded, and its flags are the cells' content.
        assert_eq!(v.per_recall.len(), case.recalls.len(), "{name}");
        for (score, entry) in v.per_recall.iter().zip(&case.recalls) {
            assert_eq!(score.step, entry.step, "{name}");
            assert_eq!(score.tier.is_some(), score.incident.is_some(), "{name}");
            assert_eq!(
                score.source == SourceClass::Unknown,
                entry.source.is_none(),
                "{name}"
            );
        }
        let correct = v.per_recall.iter().filter(|s| s.correct).count() as u32;
        assert_eq!(correct, t.recalls.correct(), "{name}");
        let right = v
            .per_recall
            .iter()
            .filter(|s| s.source == SourceClass::Right)
            .count() as u32;
        assert_eq!(right, t.recalls.source_right(), "{name}");
        // K6: the per-incident cells add to the total minus what is anchored on background.
        let per_incident: u32 = v.per_incident.iter().map(|i| i.recalls.total()).sum();
        assert_eq!(
            per_incident + t.recalls_on_background.total(),
            t.recalls.total(),
            "{name}"
        );
        for i in &v.per_incident {
            // K3, K4, K6 as a function of the row's own counts.
            assert_eq!(
                i.unasked_correct,
                i.correct_declarations > 0 && i.escalations == 0
            );
            assert_eq!(
                i.unasked_wrong,
                i.wrong_declarations > 0 && i.escalations == 0
            );
            assert_eq!(i.stale_wrong, i.recalls.wrong() > 0 && i.escalations == 0);
            // A recall is a declaration: the cells never exceed the declarations.
            assert!(i.recalls.correct() <= i.correct_declarations, "{name}");
            assert!(i.recalls.wrong() <= i.wrong_declarations, "{name}");
            // K2 only for a hard incident.
            assert!(!i.same_family_earlier || i.tier == Tier::Hard, "{name}");
        }
        // K7: the tier counts are the rows' flags counted.
        let count = |f: &dyn Fn(&gordian_stream_eval::IncidentMemory) -> bool, tier: Tier| {
            v.per_incident
                .iter()
                .filter(|i| i.tier == tier && f(i))
                .count() as u32
        };
        assert_eq!(
            t.unasked_correct.plain,
            count(&|i| i.unasked_correct, Tier::Plain),
            "{name}"
        );
        assert_eq!(
            t.unasked_correct.hard,
            count(&|i| i.unasked_correct, Tier::Hard),
            "{name}"
        );
        assert_eq!(
            t.unasked_correct.decoy,
            count(&|i| i.unasked_correct, Tier::Decoy),
            "{name}"
        );
        assert_eq!(
            t.unasked_wrong.plain,
            count(&|i| i.unasked_wrong, Tier::Plain),
            "{name}"
        );
        assert_eq!(
            t.unasked_wrong.hard,
            count(&|i| i.unasked_wrong, Tier::Hard),
            "{name}"
        );
        assert_eq!(
            t.unasked_wrong.decoy,
            count(&|i| i.unasked_wrong, Tier::Decoy),
            "{name}"
        );
        assert_eq!(
            t.stale_wrong.plain,
            count(&|i| i.stale_wrong, Tier::Plain),
            "{name}"
        );
        assert_eq!(
            t.stale_wrong.hard,
            count(&|i| i.stale_wrong, Tier::Hard),
            "{name}"
        );
        assert_eq!(
            t.stale_wrong.decoy,
            count(&|i| i.stale_wrong, Tier::Decoy),
            "{name}"
        );
        // K7's three populations, from the rows.
        let hard = |f: &dyn Fn(&gordian_stream_eval::IncidentMemory) -> bool| {
            v.per_incident
                .iter()
                .filter(|i| i.tier == Tier::Hard && f(i))
                .count() as u32
        };
        assert_eq!(
            t.hard_recurrences,
            hard(&|i| i.recurrence_of.is_some()),
            "{name}"
        );
        assert_eq!(
            t.hard_recurrences_unasked_correct,
            hard(&|i| i.recurrence_of.is_some() && i.unasked_correct),
            "{name}"
        );
        assert_eq!(
            t.hard_elsewhere,
            hard(&|i| i.recurrence_of.is_none() && i.same_family_earlier),
            "{name}"
        );
        assert_eq!(
            t.hard_elsewhere_unasked_correct,
            hard(&|i| i.recurrence_of.is_none() && i.same_family_earlier && i.unasked_correct),
            "{name}"
        );
        assert_eq!(
            t.hard_reachable,
            hard(&|i| i.recurrence_of.is_some() || i.same_family_earlier),
            "{name}"
        );
        assert_eq!(
            t.hard_reachable_unasked_correct,
            hard(&|i| (i.recurrence_of.is_some() || i.same_family_earlier) && i.unasked_correct),
            "{name}"
        );
    }
}

#[test]
fn the_source_class_names_round_trip() {
    for (class, text) in [
        (SourceClass::Right, "\"Right\""),
        (SourceClass::Wrong, "\"Wrong\""),
        (SourceClass::Unknown, "\"Unknown\""),
    ] {
        assert_eq!(serde_json::to_string(&class).expect("serializes"), text);
    }
}

#[test]
fn a_verdict_survives_a_serialization_round_trip() {
    for case in cases() {
        let Ok(v) = case.score() else { continue };
        let text = serde_json::to_string(&v).expect("serializes");
        let back: MemoryVerdict = serde_json::from_str(&text).expect("parses");
        assert_eq!(v, back, "{}", case.name);
    }
}
