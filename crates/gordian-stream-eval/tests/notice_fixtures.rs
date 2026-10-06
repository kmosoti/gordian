//! The hand-written cases in `fixtures/notice-cases.json` (work item B1), and checks on the
//! fixtures themselves.
//!
//! The cases describe the truth, the observation instants and the record of notices directly, and
//! give each expected verdict as a literal worked out by hand from `RULES.md`, section "Notices".
//! Nothing here calls the stream's generator.

mod common;

use common::CompactTruth;
use gordian_core::Instant;
use gordian_stream::Tier;
use gordian_stream_eval::{
    ANCHOR_WINDOW_NS, NoticeEvalError, NoticeTrace, NoticeVerdict, score_notices,
};
use serde::Deserialize;
use std::collections::BTreeSet;

const FIXTURES: &str = include_str!("../fixtures/notice-cases.json");
const RULES_MD: &str = include_str!("../RULES.md");

#[derive(Debug, Deserialize)]
enum Expected {
    Verdict(NoticeVerdict),
    Error(NoticeEvalError),
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
    obs_at: Vec<u64>,
    trace: NoticeTrace,
    expected: Expected,
}

impl Case {
    fn score(&self) -> Result<NoticeVerdict, NoticeEvalError> {
        let at: Vec<Instant> = self.obs_at.iter().map(|n| Instant(*n)).collect();
        score_notices(&self.truth.build(), &at, &self.trace)
    }
}

fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURES).expect("fixtures/notice-cases.json parses")
}

/// Ids of the notice rules in RULES.md: the first cell of every table row that starts with `| N`.
fn documented_rules() -> BTreeSet<String> {
    RULES_MD
        .lines()
        .filter_map(|l| l.strip_prefix("| N"))
        .filter_map(|rest| rest.split('|').next())
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| format!("N{id}"))
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
fn there_are_at_least_twenty_five_distinct_cases() {
    let cases = cases();
    let names: BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "case names must be unique");
    assert!(cases.len() >= 25, "only {} cases", cases.len());
}

#[test]
fn every_documented_rule_is_pinned_and_every_cited_rule_exists() {
    let documented = documented_rules();
    assert_eq!(documented.len(), 12, "RULES.md should define N1 to N12");
    for n in 1..=12 {
        assert!(documented.contains(&format!("N{n}")), "N{n} is missing");
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
                NoticeEvalError::ObservationInstantsMismatch { .. } => {
                    "ObservationInstantsMismatch"
                }
                NoticeEvalError::IncidentIdMismatch { .. } => "IncidentIdMismatch",
                NoticeEvalError::LabelOfUnknownIncident { .. } => "LabelOfUnknownIncident",
                NoticeEvalError::UnknownAnchor { .. } => "UnknownAnchor",
                NoticeEvalError::NoticeBeforeAnchor { .. } => "NoticeBeforeAnchor",
                NoticeEvalError::NoticeTimeWentBackwards { .. } => "NoticeTimeWentBackwards",
                NoticeEvalError::AnomalyNoticedTwice { .. } => "AnomalyNoticedTwice",
                NoticeEvalError::RetirementTimeWentBackwards { .. } => {
                    "RetirementTimeWentBackwards"
                }
                NoticeEvalError::RetirementWithoutNotice { .. } => "RetirementWithoutNotice",
                NoticeEvalError::RetirementBeforeNotice { .. } => "RetirementBeforeNotice",
            });
        }
    }
    assert_eq!(seen.len(), 10, "error variants exercised: {seen:?}");
}

#[test]
fn every_error_says_what_it_is_and_where() {
    let mut said = BTreeSet::new();
    for case in cases() {
        if let Expected::Error(e) = &case.expected {
            let text = e.to_string();
            assert!(text.len() > 10, "{text}");
            said.insert(text);
        }
    }
    // The ten variants say ten different things (the fixtures use each with distinct numbers).
    assert!(said.len() >= 10, "{said:?}");
    let text = NoticeEvalError::RetirementWithoutNotice {
        index: 4,
        anomaly: 7,
    }
    .to_string();
    assert!(
        text.contains('4') && text.contains('7') && text.contains("retirement"),
        "{text}"
    );
}

#[test]
fn every_tier_and_both_anchor_outcomes_appear() {
    let (mut correct, mut late, mut missed, mut background) = (false, false, false, false);
    let mut tiers = BTreeSet::new();
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        for i in &v.per_incident {
            correct |= i.anchor_correct;
            late |= i.noticed && !i.anchor_correct;
            missed |= !i.noticed;
            if i.noticed {
                tiers.insert(format!("{:?}", i.tier));
            }
        }
        background |= v.totals.on_background > 0;
    }
    assert!(correct && late && missed && background);
    assert_eq!(tiers.len(), 3, "tiers noticed: {tiers:?}");
}

/// An expected verdict that contradicts itself would mean the fixture, not the scorer, is wrong.
/// These identities are stated from `RULES.md`, not read out of `score_notices`.
#[test]
fn expected_verdicts_are_internally_consistent() {
    for case in cases() {
        let Expected::Verdict(v) = &case.expected else {
            continue;
        };
        let name = &case.name;
        let t = &v.totals;
        // N8: the four by-anchor counts sum to the notices, and per-notice rows are the notices.
        assert_eq!(
            t.on_background + t.on_plain + t.on_hard + t.on_decoy,
            t.notices,
            "{name}"
        );
        assert_eq!(v.per_notice.len() as u32, t.notices, "{name}");
        assert_eq!(t.notices as usize, case.trace.notices.len(), "{name}");
        assert_eq!(
            t.retirements as usize,
            case.trace.retirements.len(),
            "{name}"
        );
        // N6: the per-incident counts add to the notices anchored on incidents.
        let on_incidents: u32 = v.per_incident.iter().map(|i| i.notices).sum();
        assert_eq!(on_incidents, t.notices - t.on_background, "{name}");
        // N9: totals by tier count the incidents flagged.
        let by = |tier: Tier, f: &dyn Fn(&gordian_stream_eval::IncidentNotices) -> bool| {
            v.per_incident
                .iter()
                .filter(|i| i.tier == tier && f(i))
                .count() as u32
        };
        assert_eq!(t.noticed.plain, by(Tier::Plain, &|i| i.noticed), "{name}");
        assert_eq!(t.noticed.hard, by(Tier::Hard, &|i| i.noticed), "{name}");
        assert_eq!(t.noticed.decoy, by(Tier::Decoy, &|i| i.noticed), "{name}");
        assert_eq!(
            t.anchor_correct.hard,
            by(Tier::Hard, &|i| i.anchor_correct),
            "{name}"
        );
        for i in &v.per_incident {
            // N2, N3, N4: noticed, first notice and latency come together.
            assert_eq!(i.noticed, i.notices > 0, "{name}");
            assert_eq!(i.noticed, i.first_notice_at.is_some(), "{name}");
            assert_eq!(
                i.notice_latency_ns.is_some(),
                i.noticed && i.first_observation_at.is_some(),
                "{name}"
            );
            // N5: anchor-correct implies noticed.
            assert!(!i.anchor_correct || i.noticed, "{name}");
        }
        // N5, N7: a per-notice flag is the 1 s window on the offset.
        for n in &v.per_notice {
            assert_eq!(
                n.anchor_correct,
                n.anchor_offset_ns.is_some_and(|o| o <= ANCHOR_WINDOW_NS),
                "{name}"
            );
        }
    }
}

#[test]
fn scoring_is_a_function_of_its_inputs_and_survives_a_serialization_round_trip() {
    for case in cases() {
        let first = case.score();
        assert_eq!(first, case.score(), "{}", case.name);
        if let Ok(verdict) = first {
            let text = serde_json::to_string(&verdict).unwrap();
            let back: NoticeVerdict = serde_json::from_str(&text).unwrap();
            assert_eq!(verdict, back, "{}", case.name);
        }
    }
}

#[test]
fn the_verdict_does_not_depend_on_the_order_the_notices_of_one_instant_are_recorded_in() {
    // Notices at equal instants may be recorded in either order: the per-incident fields and the
    // totals are the same, and only the per-notice rows are in the order recorded.
    let case = cases()
        .into_iter()
        .find(|c| c.name.starts_with("streams/two-hard"))
        .unwrap();
    let mut swapped = case.trace.clone();
    let n = swapped.notices.len();
    swapped.notices.swap(n - 2, n - 1);
    let at: Vec<Instant> = case.obs_at.iter().map(|n| Instant(*n)).collect();
    let truth = case.truth.build();
    let a = score_notices(&truth, &at, &case.trace).unwrap();
    let b = score_notices(&truth, &at, &swapped).unwrap();
    assert_eq!(a.per_incident, b.per_incident);
    assert_eq!(a.totals, b.totals);
}
