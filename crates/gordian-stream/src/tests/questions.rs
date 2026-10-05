//! R8's question records: what they hold, and that the pool is exactly what the plan says.

use super::*;
use crate::kinds::Tier;
use crate::oracle::ObsLabel;
use crate::questions::{POOL_WINDOW_NS, Pool, QuestionRecord, questions, questions_with_pool};
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

// ---------------------------------------------------------------------------------------------
// R9's pool option: the focus incident's own non-decisive observations are in the pool.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_pool_is_the_old_pool() {
    // `questions` and the explicit default give the same bytes, and there is no `pool_roles`.
    for seed in 30_000..30_012 {
        let st = generate(&params(seed));
        let a = questions(&st, POOL_WINDOW_NS, true, true);
        let b = questions_with_pool(&st, POOL_WINDOW_NS, true, true, Pool::default());
        assert_eq!(Pool::default(), Pool::Others);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        assert!(a.iter().all(|q| q.pool_roles.is_none()));
        assert!(
            a.iter()
                .all(|q| !serde_json::to_string(q).unwrap().contains("pool_roles"))
        );
    }
}

#[test]
fn the_own_pool_is_everything_in_the_window_except_the_focus_and_the_decisive_evidence() {
    let mut own_seen = 0usize;
    let mut roles_seen = BTreeSet::new();
    for seed in 30_000..30_025 {
        let st = generate(&params(seed));
        let truth = reveal(&st);
        for q in questions_with_pool(&st, POOL_WINDOW_NS, true, true, Pool::IncludingOwn) {
            let (lo, hi) = (
                q.focus.at_ns.saturating_sub(POOL_WINDOW_NS),
                q.focus.at_ns + POOL_WINDOW_NS,
            );
            let decisive: BTreeSet<u32> = q.decisive.iter().map(|d| d.id).collect();
            // Independent recomputation: every event in the window that is not the focus and not
            // one of the incident's decisive observations.
            let expect: Vec<u32> = (0..st.events().len() as u32)
                .filter(|i| {
                    let at = st.events()[*i as usize].0.0;
                    at >= lo && at <= hi && *i != q.focus.id && !decisive.contains(i)
                })
                .collect();
            let got: Vec<u32> = q.pool.iter().map(|p| p.id).collect();
            assert_eq!(got, expect, "seed {} incident {}", q.seed, q.incident);
            assert_eq!(q.pool_size, got.len());
            // The roles line up with the pool and with the labels.
            let roles = q.pool_roles.as_ref().expect("roles with the own pool");
            assert_eq!(roles.len(), q.pool.len());
            for (p, role) in q.pool.iter().zip(roles) {
                assert_eq!(p.obs, st.events()[p.id as usize].1);
                let own = truth.incident_of(ObsId(p.id)) == Some(q.incident);
                assert_eq!(role.starts_with("own:"), own, "{role}");
                assert!(!own || !role.ends_with("Decisive"), "{role}");
                own_seen += usize::from(own);
                roles_seen.insert(role.split(':').next().unwrap().to_string());
            }
            // The focus and the decisive evidence are never in it.
            let pool: BTreeSet<u32> = got.into_iter().collect();
            assert!(!pool.contains(&q.focus.id));
            assert!(decisive.iter().all(|d| !pool.contains(d)));
        }
    }
    // The test has power: the own observations are really there, and all three kinds of role occur.
    assert!(own_seen > 1_000, "own observations seen: {own_seen}");
    assert_eq!(roles_seen.len(), 3, "{roles_seen:?}");
}

#[test]
fn the_own_pool_is_the_default_pool_plus_the_incidents_own_non_decisive_observations() {
    // The two pools differ by exactly the focus incident's own non-decisive, non-focus
    // observations in the window: neither pool lost or gained anything else, and the default
    // pool is a subset.
    let mut extra_total = 0usize;
    for seed in 30_000..30_020 {
        let st = generate(&params(seed));
        let truth = reveal(&st);
        let a = questions_with_pool(&st, POOL_WINDOW_NS, true, true, Pool::Others);
        let b = questions_with_pool(&st, POOL_WINDOW_NS, true, true, Pool::IncludingOwn);
        assert_eq!(a.len(), b.len());
        for (qa, qb) in a.iter().zip(&b) {
            assert_eq!((qa.seed, qa.incident), (qb.seed, qb.incident));
            assert_eq!(qa.focus, qb.focus);
            assert_eq!(qa.decisive, qb.decisive);
            let pa: BTreeSet<u32> = qa.pool.iter().map(|p| p.id).collect();
            let pb: BTreeSet<u32> = qb.pool.iter().map(|p| p.id).collect();
            assert!(pa.is_subset(&pb));
            let diff: BTreeSet<u32> = pb.difference(&pa).copied().collect();
            assert!(diff.iter().all(|i| {
                truth.incident_of(ObsId(*i)) == Some(qa.incident)
                    && *i != qa.focus.id
                    && !qa.decisive.iter().any(|d| d.id == *i)
            }));
            extra_total += diff.len();
            // Both pools stay in stream order.
            assert!(qb.pool.windows(2).all(|w| w[0].id < w[1].id));
        }
    }
    assert!(extra_total > 1_000, "extra observations: {extra_total}");
}

#[test]
fn the_own_pool_record_has_the_roles_key_and_nothing_else_new() {
    let st = generate(&params(30_000));
    let qs = questions_with_pool(&st, POOL_WINDOW_NS, true, true, Pool::IncludingOwn);
    let v = serde_json::to_value(qs.first().expect("a question")).unwrap();
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
            "pool_roles",
            "pool_size",
            "recurrence_of",
            "seed",
            "services",
            "tier",
            "truth"
        ]
    );
}
