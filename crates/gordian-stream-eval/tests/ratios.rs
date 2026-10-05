//! The two derived ratios on `StreamTotals` (RULES.md, "Derived ratios"), and the `Default`
//! totals. The counts they read are pinned by the fixtures; here the arithmetic is pinned.

use gordian_stream_eval::{EscalationCounts, StreamTotals, TierCounts};

fn totals(
    hard: u32,
    needed: u32,
    unneeded: u32,
    background: u32,
    hard_escalated: u32,
) -> StreamTotals {
    StreamTotals {
        incidents: TierCounts {
            plain: 7,
            hard,
            decoy: 2,
        },
        escalations: EscalationCounts {
            needed,
            unneeded,
            background,
            hard_incidents_escalated: hard_escalated,
            ..EscalationCounts::default()
        },
        ..StreamTotals::default()
    }
}

#[test]
fn precision_is_needed_over_every_escalation_background_included() {
    assert_eq!(totals(3, 2, 2, 1, 1).escalation_precision(), Some(0.4));
    assert_eq!(totals(3, 4, 0, 0, 1).escalation_precision(), Some(1.0));
    assert_eq!(totals(3, 0, 3, 0, 0).escalation_precision(), Some(0.0));
    assert_eq!(totals(3, 0, 0, 5, 0).escalation_precision(), Some(0.0));
    assert_eq!(totals(3, 1, 0, 3, 1).escalation_precision(), Some(0.25));
    // 3 / 4: the unneeded count is in the denominator.
    assert_eq!(totals(3, 3, 1, 0, 1).escalation_precision(), Some(0.75));
}

#[test]
fn precision_is_undefined_without_an_escalation() {
    assert_eq!(totals(3, 0, 0, 0, 0).escalation_precision(), None);
    assert_eq!(StreamTotals::default().escalation_precision(), None);
}

#[test]
fn recall_is_hard_incidents_escalated_over_hard_incidents() {
    assert_eq!(totals(4, 9, 0, 0, 1).escalation_recall(), Some(0.25));
    assert_eq!(totals(2, 2, 0, 0, 2).escalation_recall(), Some(1.0));
    assert_eq!(totals(5, 0, 6, 0, 0).escalation_recall(), Some(0.0));
    // Repeated calls about one hard incident do not raise it.
    assert_eq!(totals(2, 10, 0, 0, 1).escalation_recall(), Some(0.5));
}

#[test]
fn recall_is_undefined_without_a_hard_incident() {
    assert_eq!(totals(0, 0, 4, 0, 0).escalation_recall(), None);
    assert_eq!(StreamTotals::default().escalation_recall(), None);
}

#[test]
fn the_default_totals_are_all_zero() {
    let json = serde_json::to_value(StreamTotals::default()).unwrap();
    fn all_zero(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Number(n) => n.as_u64() == Some(0),
            serde_json::Value::Object(m) => m.values().all(all_zero),
            _ => false,
        }
    }
    assert!(all_zero(&json), "{json}");
}
