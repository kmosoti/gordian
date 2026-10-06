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
    assert_eq!(documented.len(), 16, "RULES.md should define N1 to N16");
    for n in 1..=16 {
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
            // N15: the conjunction, and neither flag on a notice about no incident (N14).
            assert_eq!(
                n.anchor_site_correct,
                n.anchor_correct && n.site_correct,
                "{name}"
            );
            assert!(n.incident.is_some() || !n.site_correct, "{name}");
        }
        // N14, N15: the counts of notices are the flags of the per-notice rows, and the
        // incidents' flags are "some notice about it".
        assert_eq!(
            t.notices_site_correct as usize,
            v.per_notice.iter().filter(|n| n.site_correct).count(),
            "{name}"
        );
        assert_eq!(
            t.notices_anchor_site_correct as usize,
            v.per_notice
                .iter()
                .filter(|n| n.anchor_site_correct)
                .count(),
            "{name}"
        );
        for (tier, want_site, want_both) in [
            (
                Tier::Plain,
                t.site_correct.plain,
                t.anchor_site_correct.plain,
            ),
            (Tier::Hard, t.site_correct.hard, t.anchor_site_correct.hard),
            (
                Tier::Decoy,
                t.site_correct.decoy,
                t.anchor_site_correct.decoy,
            ),
        ] {
            assert_eq!(want_site, by(tier, &|i| i.site_correct), "{name}");
            assert_eq!(want_both, by(tier, &|i| i.anchor_site_correct), "{name}");
        }
        for i in &v.per_incident {
            assert!(
                !i.anchor_site_correct || (i.anchor_correct && i.site_correct),
                "{name}"
            );
            assert!(!i.site_correct || i.noticed, "{name}");
        }
        // N16: the tiers' precisions add to the precision.
        let tiers: f64 = [Tier::Plain, Tier::Hard, Tier::Decoy]
            .iter()
            .filter_map(|tier| t.precision_in(*tier))
            .sum();
        assert_eq!(
            t.precision().map(|p| (p * 1e9).round()),
            t.precision().map(|_| (tiers * 1e9).round()),
            "{name}"
        );
    }
}

/// The ratios of N16 on the stream the fixture was written for, worked out by hand: six notices,
/// four on incidents (one plain, two hard, one decoy), two of them anchor-and-site-correct.
#[test]
fn notice_precision_is_the_share_of_notices_on_incidents_worked_by_hand() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "precision/notices-on-incidents-over-all-notices")
        .unwrap();
    let verdict = case.score().unwrap();
    let t = &verdict.totals;
    assert_eq!(t.on_incidents(), 4);
    assert_eq!(t.precision(), Some(4.0 / 6.0));
    assert_eq!(t.precision_in(Tier::Plain), Some(1.0 / 6.0));
    assert_eq!(t.precision_in(Tier::Hard), Some(2.0 / 6.0));
    assert_eq!(t.precision_in(Tier::Decoy), Some(1.0 / 6.0));
    assert_eq!(t.strict_precision(), Some(2.0 / 6.0));
    // A stream with no notice has no precision, however many incidents it has.
    let empty = cases()
        .into_iter()
        .find(|c| c.name == "incident/never-noticed")
        .unwrap()
        .score()
        .unwrap();
    assert_eq!(empty.totals.notices, 0);
    assert_eq!(empty.totals.precision(), None);
    assert_eq!(empty.totals.precision_in(Tier::Hard), None);
    assert_eq!(empty.totals.strict_precision(), None);
    // A stream whose every notice is on background has precision zero, not none.
    let background = cases()
        .into_iter()
        .find(|c| c.name == "site/a-notice-on-background-is-never-site-correct")
        .unwrap()
        .score()
        .unwrap();
    assert_eq!(background.totals.precision(), Some(0.0));
    assert_eq!(background.totals.strict_precision(), Some(0.0));
}

/// N13 to N16 add inputs and counts and change none of the earlier measures: whatever site the
/// record gives (or none), every field of N1 to N12 is the same.
#[test]
fn the_site_does_not_change_the_measures_of_n1_to_n12() {
    for case in cases() {
        let Expected::Verdict(_) = &case.expected else {
            continue;
        };
        let base = case.score().unwrap();
        for variant in 0..3u32 {
            let mut trace = case.trace.clone();
            for n in &mut trace.notices {
                n.site = match variant {
                    0 => None,
                    1 => Some(n.site.map_or(0, |s| s + 1)),
                    _ => Some(900 + n.anomaly),
                };
            }
            let at: Vec<Instant> = case.obs_at.iter().map(|n| Instant(*n)).collect();
            let other = score_notices(&case.truth.build(), &at, &trace).unwrap();
            assert_eq!(base.totals.notices, other.totals.notices, "{}", case.name);
            assert_eq!(base.totals.noticed, other.totals.noticed, "{}", case.name);
            assert_eq!(
                base.totals.anchor_correct, other.totals.anchor_correct,
                "{}",
                case.name
            );
            assert_eq!(base.totals.on_hard, other.totals.on_hard, "{}", case.name);
            assert_eq!(
                base.totals.on_background, other.totals.on_background,
                "{}",
                case.name
            );
            assert_eq!(base.per_incident.len(), other.per_incident.len());
            for (a, b) in base.per_incident.iter().zip(&other.per_incident) {
                assert_eq!(
                    (
                        a.notices,
                        a.noticed,
                        a.first_notice_at,
                        a.notice_latency_ns,
                        a.anchor_correct
                    ),
                    (
                        b.notices,
                        b.noticed,
                        b.first_notice_at,
                        b.notice_latency_ns,
                        b.anchor_correct
                    ),
                    "{}",
                    case.name
                );
            }
            for (a, b) in base.per_notice.iter().zip(&other.per_notice) {
                assert_eq!(
                    (a.incident, a.tier, a.anchor_offset_ns, a.anchor_correct),
                    (b.incident, b.tier, b.anchor_offset_ns, b.anchor_correct),
                    "{}",
                    case.name
                );
            }
        }
    }
}

/// The fixtures of work item B1 (N1 to N12) are the first 29, unchanged, and none of them records a
/// site: they still pass as written, which is what "N13 to N16 change none of N1 to N12" means.
#[test]
fn the_fixtures_of_n1_to_n12_are_unchanged_and_record_no_site() {
    let all = cases();
    for case in all.iter().take(29) {
        assert!(
            case.rules.iter().all(|r| {
                let n: u32 = r[1..].parse().unwrap();
                n <= 12
            }),
            "{}",
            case.name
        );
        assert!(
            case.trace.notices.iter().all(|n| n.site.is_none()),
            "{}",
            case.name
        );
    }
    assert!(all.len() > 29);
    assert!(all.iter().skip(29).all(|c| c.rules.iter().any(|r| {
        let n: u32 = r[1..].parse().unwrap();
        n >= 13
    })));
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
