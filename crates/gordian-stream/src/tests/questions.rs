//! R8's question records: what they hold, and that the pool is exactly what the plan says.

use super::*;
use crate::kinds::Tier;
use crate::oracle::ObsLabel;
use crate::questions::{POOL_WINDOW_NS, QuestionRecord, questions};
use gordian_world::{Observation, ServiceId};
use std::collections::BTreeSet;

fn params(seed: u64) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.regimes.clear();
    p
}

fn service_of(o: &Observation) -> Option<ServiceId> {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => Some(*service),
        _ => None,
    }
}

fn all(seeds: std::ops::Range<u64>) -> Vec<(Stream, Vec<QuestionRecord>)> {
    seeds
        .map(|s| {
            let st = generate(&params(s));
            let qs = questions(&st, POOL_WINDOW_NS, true, true);
            (st, qs)
        })
        .collect()
}

#[test]
fn questions_are_a_pure_function_of_the_stream() {
    let st = generate(&params(30_001));
    let a = questions(&st, POOL_WINDOW_NS, true, true);
    assert_eq!(a, questions(&st, POOL_WINDOW_NS, true, true));
    let again = questions(&generate(&params(30_001)), POOL_WINDOW_NS, true, true);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&again).unwrap()
    );
}

#[test]
fn only_plain_and_non_leak_hard_incidents_are_questions() {
    let mut hard = 0;
    let mut plain = 0;
    let mut leaks_seen = 0;
    for seed in 30_000..30_040 {
        let st = generate(&params(seed));
        let truth = reveal(&st);
        leaks_seen += truth
            .incidents
            .iter()
            .filter(|i| {
                i.shape.hard_kind == Some(crate::HardKind::SlowLeak) && i.tier == Tier::Hard
            })
            .count();
        let ids: BTreeSet<u32> = questions(&st, POOL_WINDOW_NS, true, true)
            .iter()
            .map(|q| q.incident)
            .collect();
        for inc in &truth.incidents {
            let expect = match inc.tier {
                Tier::Plain => true,
                Tier::Hard => inc.shape.hard_kind != Some(crate::HardKind::SlowLeak),
                Tier::Decoy => false,
            };
            assert_eq!(
                ids.contains(&inc.id),
                expect,
                "seed {seed} incident {}",
                inc.id
            );
            if expect {
                match inc.tier {
                    Tier::Plain => plain += 1,
                    _ => hard += 1,
                }
            }
        }
        // The tier selectors are independent.
        for q in questions(&st, POOL_WINDOW_NS, false, true) {
            assert_eq!(q.tier, Tier::Hard);
        }
        for q in questions(&st, POOL_WINDOW_NS, true, false) {
            assert_eq!(q.tier, Tier::Plain);
        }
        assert!(questions(&st, POOL_WINDOW_NS, false, false).is_empty());
    }
    assert!(hard > 30 && plain > 400, "hard {hard}, plain {plain}");
    assert!(
        leaks_seen > 0,
        "the test must see leaks to show they are skipped"
    );
}

#[test]
fn the_focus_is_the_incidents_first_observation_and_sits_at_the_truth_site() {
    let mut checked = 0;
    for (st, qs) in all(30_000..30_040) {
        let truth = reveal(&st);
        for q in qs {
            let inc = &truth.incidents[q.incident as usize];
            assert_eq!(q.focus.id, inc.observations[0].0);
            assert!(inc.observations.iter().all(|o| o.0 >= q.focus.id));
            assert_eq!(truth.incident_of(ObsId(q.focus.id)), Some(q.incident));
            assert_eq!(q.focus.obs, st.events()[q.focus.id as usize].1);
            // The prompt relies on this: the site of the truth is the focus's service.
            let h = q.truth.expect("a plain or hard incident has a truth");
            assert_eq!(service_of(&q.focus.obs), Some(h.site), "seed {}", q.seed);
            // Decisive evidence, in full and in stream order.
            let ids: Vec<u32> = q.decisive.iter().map(|d| d.id).collect();
            let expect: Vec<u32> = inc.decisive.iter().map(|d| d.0).collect();
            assert_eq!(ids, expect);
            assert!(!ids.is_empty());
            assert!(ids.windows(2).all(|w| w[0] < w[1]));
            checked += 1;
        }
    }
    assert!(checked > 400);
}

#[test]
fn the_pool_is_every_other_observation_within_the_window_and_nothing_else() {
    // The plan's window: 40 s either side of the focus.
    assert_eq!(POOL_WINDOW_NS, 40_000_000_000);
    for (st, qs) in all(30_000..30_025) {
        let truth = reveal(&st);
        for q in qs {
            let (lo, hi) = (
                q.focus.at_ns.saturating_sub(POOL_WINDOW_NS),
                q.focus.at_ns + POOL_WINDOW_NS,
            );
            let expect: Vec<u32> = (0..st.events().len() as u32)
                .filter(|i| {
                    let at = st.events()[*i as usize].0.0;
                    at >= lo && at <= hi && truth.incident_of(ObsId(*i)) != Some(q.incident)
                })
                .collect();
            let got: Vec<u32> = q.pool.iter().map(|p| p.id).collect();
            assert_eq!(got, expect, "seed {} incident {}", q.seed, q.incident);
            assert_eq!(q.pool_size, got.len());
            for p in &q.pool {
                assert_eq!(p.obs, st.events()[p.id as usize].1);
                assert_eq!(p.at_ns, st.events()[p.id as usize].0.0);
                // From the background or from another incident: never the focus incident,
                // whatever its role.
                match truth.labels[p.id as usize] {
                    ObsLabel::Background(_) => {}
                    ObsLabel::Incident { id, .. } => assert_ne!(id, q.incident),
                }
            }
            // Decisive evidence and the focus are never in the pool.
            let pool: BTreeSet<u32> = got.into_iter().collect();
            assert!(q.decisive.iter().all(|d| !pool.contains(&d.id)));
            assert!(!pool.contains(&q.focus.id));
        }
    }
}

#[test]
fn most_incidents_have_a_pool_larger_than_the_largest_level() {
    // The design excludes incidents whose pool is smaller than 400; this shows the exclusion
    // is the exception, so the questions are not drawn from a narrow slice of the stream.
    let (mut big, mut total) = (0, 0);
    for (_, qs) in all(30_000..30_030) {
        for q in qs {
            total += 1;
            if q.pool_size >= 400 {
                big += 1;
            }
        }
    }
    assert!(total > 300);
    assert!(big * 10 > total * 8, "{big} of {total}");
}

#[test]
fn clearing_the_regimes_changes_nothing_before_the_first_one() {
    let st = generate(&params(30_002));
    assert!(reveal(&st).regimes.is_empty());
    let with = generate(&StreamParams::new(30_002));
    assert!(!reveal(&with).regimes.is_empty());
    // Before the first change (200 s) the two public streams agree (HIDDEN-DESIGN.md, section 8).
    let cut = 200_000_000_000u64;
    let a: Vec<_> = st.events().iter().filter(|(t, _)| t.0 < cut).collect();
    let b: Vec<_> = with.events().iter().filter(|(t, _)| t.0 < cut).collect();
    assert_eq!(a, b);
}

#[test]
fn a_record_serializes_with_the_documented_keys() {
    let (_, qs) = all(30_000..30_001).remove(0);
    let q = qs.first().expect("the stream has a question");
    let v: serde_json::Value = serde_json::to_value(q).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "decisive",
            "duo",
            "family",
            "focus",
            "incident",
            "mode",
            "onset_ns",
            "other",
            "pool",
            "pool_size",
            "recurrence_of",
            "seed",
            "services",
            "tier",
            "truth"
        ]
    );
}
