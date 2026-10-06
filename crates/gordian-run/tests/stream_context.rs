//! The context builders of work item R6 (`rung`, `window`, `cooccur`, `neighbourhood`), the
//! selection oracle's use of them, and the check that the default builder changed nothing.
//!
//! What is pinned, and by what:
//!
//! - each builder does what its definition says, on observations and graphs written by hand;
//! - each builder is deterministic, respects its cap, never repeats a reference and never refers
//!   to an observation the rung does not hold;
//! - each builder uses no hidden state: its module is one of the files the textual guard covers,
//!   it takes a [`PublicView`] and nothing else, and the contexts an arm builds do not change when
//!   only the reasoner's hidden parameters do;
//! - the selection oracle with any public builder asks exactly where `always_escalate` asks about
//!   hard incidents, with the same contexts: its only privilege is which anomalies are hard;
//! - the supplementary `oracle_selection_context` asks only about hard anomalies, once each, with
//!   a subset of the incident's decisive evidence;
//! - the `rung` builder, spelled or defaulted, writes the `results.csv` and `incidents.csv` that
//!   the binary of commit `1b7b0c7` (before R6) wrote, byte for byte, and a non-default builder
//!   does not.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::context::{
    ContextBuilder, PublicView, cap_head_tail, cooccur, neighbourhood, window, within_hops,
};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store};
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::results::results_row;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_run::stream::{SegmentRecord, execute_stream};
use gordian_stream::generate;
use gordian_stream::{ObsId, ObsRef, Question, StreamAction, StreamOutcome, StreamParams, Tier};
use gordian_stream_eval::truth_from_stream;
use gordian_world::{CounterName, Observation, ResourceKind, Service, ServiceId};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use stream_common::*;

const NS: u64 = 1_000_000_000;
const MS: u64 = 1_000_000;

// ---- Hand-built inputs

/// Services 0..5: 1 depends on 0, 2 on 1, 3 on 0; 4 is alone; 5 depends on 4.
fn graph() -> Vec<Service> {
    let depends: [&[u32]; 6] = [&[], &[0], &[1], &[0], &[], &[4]];
    depends
        .iter()
        .enumerate()
        .map(|(i, d)| Service {
            id: ServiceId(i as u32),
            depends_on: d.iter().map(|x| ServiceId(*x)).collect(),
            resource: ResourceKind::Cpu,
            config_hash: 0,
            unreliable_health: false,
        })
        .collect()
}

fn held(id: u32, at_ms: u64, service: u32, abnormal: bool) -> Held {
    Held {
        id: ObsId(id),
        at: Instant(at_ms * MS),
        obs: Observation::Counter {
            service: ServiceId(service),
            name: CounterName::ErrorRate,
            value: if abnormal { 90 } else { 1 },
        },
        abnormal,
    }
}

fn ids(refs: &[ObsRef]) -> Vec<u32> {
    refs.iter()
        .map(|r| match r {
            ObsRef::Passive(o) => o.0,
            ObsRef::Probe(_) => panic!("a builder returned a probe reference"),
        })
        .collect()
}

fn view<'a>(
    store: &'a Store,
    services: &'a [Service],
    now_ms: u64,
    anchor_ms: u64,
    site: u32,
) -> PublicView<'a> {
    PublicView {
        store,
        services,
        now: Instant(now_ms * MS),
        anchor_at: Instant(anchor_ms * MS),
        site: ServiceId(site),
        burst_gap_ns: 2 * NS,
        lookback_ns: 2 * NS,
    }
}

// ---- Each builder, by its definition

#[test]
fn window_is_the_most_recent_observations_of_the_last_w_seconds_across_services_most_recent_first()
{
    let services = graph();
    // Twenty observations, one a second, rotating over the six services, none abnormal.
    let store = Store::with((0..20).map(|k| held(k, u64::from(k) * 1000, k % 6, false)));
    let v = view(&store, &services, 19_000, 5_000, 1);
    // The last 5 s from 19 s reach back to 14 s inclusive: ids 14..=19, most recent first.
    assert_eq!(ids(&window(&v, 5 * NS, 100)), vec![19, 18, 17, 16, 15, 14]);
    // The cap keeps the most recent.
    assert_eq!(ids(&window(&v, 5 * NS, 4)), vec![19, 18, 17, 16]);
    // A window longer than what is held is everything held.
    assert_eq!(window(&v, 100 * NS, 100).len(), 20);
    // It reads the instant of the call and not the anomaly: the anchor is irrelevant to it.
    let other_anchor = view(&store, &services, 19_000, 12_000, 4);
    assert_eq!(window(&v, 5 * NS, 100), window(&other_anchor, 5 * NS, 100));
    // Services are not filtered: benign and abnormal readings at every service are in it.
    let services_in: BTreeSet<u32> = ids(&window(&v, 6 * NS, 100))
        .iter()
        .map(|i| i % 6)
        .collect();
    assert_eq!(services_in.len(), 6);
}

#[test]
fn cooccur_is_what_happens_at_services_whose_abnormal_readings_began_with_the_anomalys() {
    let services = graph();
    // The anomaly is anchored at service 1 at t = 10 s. Abnormal readings:
    //   service 0 at 10.2 s (began 0.2 s after the anchor);
    //   service 2 at 8.5 s and again at 10.2 s (its run began 1.5 s before the anchor, and the
    //     10.2 s reading is not a beginning: it follows another within the 2 s burst gap);
    //   service 3 at 14 s (began 4 s after);
    //   service 4 at 8 s (began 2 s before);
    //   service 5 benign only.
    let store = Store::with([
        held(0, 7_000, 0, false),
        held(1, 8_000, 4, true),
        held(2, 8_500, 5, false),
        held(3, 8_500, 2, true),
        held(4, 10_000, 1, true),
        held(5, 10_200, 0, true),
        held(6, 10_200, 2, true),
        held(7, 11_000, 5, false),
        held(8, 12_000, 1, false),
        held(9, 14_000, 3, true),
        held(10, 14_500, 0, false),
    ]);
    let v = view(&store, &services, 15_000, 10_000, 1);
    // Observations are taken from 8 s (the anchor minus the 2 s lookback) to now.
    // Delta zero: the site alone (its beginning is the anchor).
    assert_eq!(ids(&cooccur(&v, 0, 100)), vec![4, 8]);
    // Delta 0.5 s adds service 0 (began 0.2 s after). Service 2's 10.2 s reading is within the
    // delta but is not a beginning.
    assert_eq!(ids(&cooccur(&v, NS / 2, 100)), vec![4, 5, 8, 10]);
    assert_eq!(ids(&cooccur(&v, NS, 100)), vec![4, 5, 8, 10]);
    // Delta 2 s adds service 4 (began 2 s before) and service 2 (began 1.5 s before). Service 5,
    // benign only, never co-occurs.
    assert_eq!(ids(&cooccur(&v, 2 * NS, 100)), vec![1, 3, 4, 5, 6, 8, 10]);
    // Delta 5 s adds service 3 (began 4 s after).
    assert_eq!(
        ids(&cooccur(&v, 5 * NS, 100)),
        vec![1, 3, 4, 5, 6, 8, 9, 10]
    );
    // Older observations at a member service are not in it (id 0, at 7 s, service 0).
    assert!(!ids(&cooccur(&v, 5 * NS, 100)).contains(&0));
}

#[test]
fn neighbourhood_is_what_happens_within_k_hops_of_the_site_in_the_public_graph() {
    let services = graph();
    let set = |site: u32, hops: u32| -> Vec<u32> {
        within_hops(&services, ServiceId(site), hops)
            .iter()
            .enumerate()
            .filter(|(_, m)| **m)
            .map(|(i, _)| i as u32)
            .collect()
    };
    assert_eq!(set(1, 0), vec![1]);
    assert_eq!(set(1, 1), vec![0, 1, 2]);
    assert_eq!(set(1, 2), vec![0, 1, 2, 3]);
    assert_eq!(set(1, 9), vec![0, 1, 2, 3]);
    assert_eq!(set(4, 1), vec![4, 5]);
    assert_eq!(set(2, 1), vec![1, 2]);
    assert_eq!(set(2, 3), vec![0, 1, 2, 3]);
    // A site outside the graph reaches nothing.
    assert!(set(40, 3).is_empty());

    let store = Store::with([
        held(0, 7_000, 1, false), // older than the lookback
        held(1, 8_000, 0, false),
        held(2, 9_000, 4, true),
        held(3, 11_000, 2, false),
        held(4, 12_000, 5, true),
        held(5, 14_000, 3, true),
        held(6, 14_500, 1, true),
    ]);
    let v = view(&store, &services, 15_000, 10_000, 1);
    assert_eq!(ids(&neighbourhood(&v, 0, 100)), vec![6]);
    assert_eq!(ids(&neighbourhood(&v, 1, 100)), vec![1, 3, 6]);
    assert_eq!(ids(&neighbourhood(&v, 2, 100)), vec![1, 3, 5, 6]);
    // No hop count reaches the unconnected pair.
    assert_eq!(ids(&neighbourhood(&v, 9, 100)), vec![1, 3, 5, 6]);
}

#[test]
fn the_service_and_hop_builders_cap_by_keeping_the_first_quarter_and_the_most_recent_rest() {
    let services = graph();
    let store = Store::with((0..40).map(|k| held(k, 8_000 + u64::from(k) * 100, 1, true)));
    let v = view(&store, &services, 15_000, 8_000, 1);
    let kept = ids(&neighbourhood(&v, 1, 8));
    // head = 8 / 4 = 2, tail = 6.
    assert_eq!(kept, vec![0, 1, 34, 35, 36, 37, 38, 39]);
    assert_eq!(ids(&cooccur(&v, NS, 8)), kept);
    let refs: Vec<ObsRef> = (0..10).map(|k| ObsRef::Passive(ObsId(k))).collect();
    assert_eq!(cap_head_tail(refs.clone(), 10), refs);
    assert_eq!(ids(&cap_head_tail(refs, 3)), vec![7, 8, 9]);
}

#[test]
fn builders_are_pure_functions_of_their_view() {
    let services = graph();
    let store = Store::with((0..30).map(|k| held(k, u64::from(k) * 500, k % 6, k % 3 == 0)));
    let v = view(&store, &services, 15_000, 6_000, 1);
    for builder in [
        ContextBuilder::Window {
            window_ns: 6 * NS,
            max_refs: 9,
        },
        ContextBuilder::Cooccur {
            delta_ns: NS,
            max_refs: 9,
        },
        ContextBuilder::Neighbourhood {
            hops: 2,
            max_refs: 9,
        },
    ] {
        let a = gordian_run::stream::arms::context::build(&builder, &v).unwrap();
        let b = gordian_run::stream::arms::context::build(&builder, &v).unwrap();
        assert_eq!(a, b, "{builder:?}");
        assert!(a.len() <= 9, "{builder:?}");
        let unique: BTreeSet<_> = a.iter().collect();
        assert_eq!(unique.len(), a.len(), "{builder:?} repeats a reference");
    }
    // The rung's own builder needs the anomaly's region, which only the rung holds.
    assert!(gordian_run::stream::arms::context::build(&ContextBuilder::Rung, &v).is_none());
}

// ---- The manifest spelling

#[test]
fn the_default_builder_is_not_written_and_every_builder_round_trips() {
    let rung = serde_json::to_string(&RungConfig::default()).unwrap();
    assert!(!rung.contains("\"context\""), "{rung}");
    let default_spec = gordian_run::stream::manifest::StreamArmSpec {
        arm: "never_escalate".to_owned(),
        policy: StreamPolicySpec::Never,
        context: None,
    };
    assert!(
        !serde_json::to_string(&default_spec)
            .unwrap()
            .contains("\"context\"")
    );
    for builder in [
        ContextBuilder::Rung,
        ContextBuilder::Window {
            window_ns: 20 * NS,
            max_refs: 64,
        },
        ContextBuilder::Cooccur {
            delta_ns: NS / 2,
            max_refs: 128,
        },
        ContextBuilder::Neighbourhood {
            hops: 3,
            max_refs: 128,
        },
    ] {
        let spec = gordian_run::stream::manifest::StreamArmSpec {
            context: Some(builder),
            ..default_spec.clone()
        };
        let text = serde_json::to_string(&spec).unwrap();
        assert!(text.contains(builder.name()), "{text}");
        let back: gordian_run::stream::manifest::StreamArmSpec =
            serde_json::from_str(&text).unwrap();
        assert_eq!(back, spec);
        let cfg = RungConfig {
            context: builder,
            ..RungConfig::default()
        };
        let back: RungConfig = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back, cfg);
    }
    // A manifest written before builders existed parses to the default.
    let old: RungConfig = serde_json::from_str(&rung).unwrap();
    assert_eq!(old.context, ContextBuilder::Rung);
    // A parameter the builder does not have, and an unknown builder, are errors.
    let bad = |text: &str| serde_json::from_str::<ContextBuilder>(text).is_err();
    assert!(bad(
        r#"{"builder": "window", "window_ns": 1, "max_refs": 2, "hops": 1}"#
    ));
    assert!(bad(r#"{"builder": "no_such_builder"}"#));
    assert!(bad(r#"{"builder": "window", "window_ns": 1}"#));
}

#[test]
fn a_manifest_refuses_a_builder_it_cannot_run() {
    let mut m = manifest("ctx", &[("never_escalate", "never_escalate")], 1, 150, 0);
    let limit = m.stream_params.normalized().max_context;
    let ok = ContextBuilder::Window {
        window_ns: NS,
        max_refs: 8,
    };
    m.arms[0].context = Some(ok);
    assert!(m.validate().is_ok(), "{:?}", m.validate());
    for bad in [
        ContextBuilder::Window {
            window_ns: NS,
            max_refs: 0,
        },
        ContextBuilder::Window {
            window_ns: 0,
            max_refs: 8,
        },
        ContextBuilder::Cooccur {
            delta_ns: NS,
            max_refs: limit + 1,
        },
        ContextBuilder::Neighbourhood {
            hops: 1,
            max_refs: 0,
        },
    ] {
        m.arms[0].context = Some(bad);
        assert!(m.validate().is_err(), "{bad:?} was accepted");
    }
    m.arms[0].context = None;
    m.rung.context = ContextBuilder::Cooccur {
        delta_ns: NS,
        max_refs: 0,
    };
    assert!(m.validate().is_err());
    // An arm's own builder takes the place of the manifest's, and only for that arm.
    m.rung.context = ContextBuilder::Neighbourhood {
        hops: 1,
        max_refs: 8,
    };
    m.arms[0].context = Some(ok);
    assert_eq!(m.rung_for(&m.arms[0]).context, ok);
    m.arms[0].context = None;
    assert_eq!(m.rung_for(&m.arms[0]).context, m.rung.context);
}

// ---- On real streams

fn hard_heavy(seed: u64) -> StreamParams {
    let mut p = params(seed, 300);
    p.mix.plain_permille = 300;
    p.mix.hard_permille = 500;
    p
}

fn rung_with(context: ContextBuilder) -> RungConfig {
    RungConfig {
        context,
        ..RungConfig::default()
    }
}

fn builders(cap: u32) -> Vec<ContextBuilder> {
    vec![
        ContextBuilder::Window {
            window_ns: 20 * NS,
            max_refs: cap,
        },
        ContextBuilder::Cooccur {
            delta_ns: 2 * NS,
            max_refs: cap,
        },
        ContextBuilder::Neighbourhood {
            hops: 2,
            max_refs: cap,
        },
    ]
}

/// Every accepted escalation of a trajectory: instant, focus, context.
fn calls(record: &SegmentRecord) -> BTreeSet<(u64, u32, Vec<ObsRef>)> {
    record
        .trajectory
        .iter()
        .filter_map(|step| match (&step.action, &step.outcome) {
            (StreamAction::Escalate { context, question }, StreamOutcome::Escalated { .. }) => {
                let Question::Diagnose { focus } = question;
                Some((step.at.0, focus.0, context.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_builder_is_deterministic_on_real_streams_for_a_public_and_a_privileged_arm() {
    let always = StreamPolicySpec::Always { delay_ns: 6 * NS };
    let selection = StreamPolicySpec::OracleSelection {
        delay_ns: 6 * NS,
        hold_until_asked: false,
    };
    let mut total = 0usize;
    for seed in [3, 8] {
        let p = hard_heavy(seed);
        let l = limits(&p);
        for builder in builders(64) {
            let rung = rung_with(builder);
            for spec in [&always, &selection] {
                let a = play_with_rung(&p, spec, &l, &rung).unwrap();
                let b = play_with_rung(&p, spec, &l, &rung).unwrap();
                assert_eq!(results_row("x", &a), results_row("x", &b), "{builder:?}");
                assert_eq!(a.trajectory, b.trajectory, "{builder:?} seed {seed}");
                assert_eq!(a.verdict, b.verdict, "{builder:?} seed {seed}");
                total += calls(&a).len();
            }
        }
    }
    assert!(total > 20, "only {total} calls were made");
}

#[test]
fn every_builder_respects_its_cap_repeats_nothing_and_asks_about_held_observations_only() {
    let always = StreamPolicySpec::Always { delay_ns: 6 * NS };
    for cap in [5u32, 16] {
        for builder in builders(cap) {
            let rung = rung_with(builder);
            let (mut seen, mut at_cap) = (0usize, 0usize);
            for seed in [3, 8, 11] {
                let p = hard_heavy(seed);
                let record = play_with_rung(&p, &always, &limits(&p), &rung).unwrap();
                // A reference to an observation the rung does not hold is refused by the stream;
                // none is, and nothing is refused for budget either, so every call is counted.
                assert_eq!(
                    record.counts.escalations_refused, 0,
                    "{builder:?} seed {seed}"
                );
                for (_, _, context) in calls(&record) {
                    seen += 1;
                    assert!(
                        context.len() <= cap as usize,
                        "{builder:?}: {}",
                        context.len()
                    );
                    at_cap += usize::from(context.len() == cap as usize);
                    let unique: BTreeSet<_> = context.iter().collect();
                    assert_eq!(
                        unique.len(),
                        context.len(),
                        "{builder:?} repeats a reference"
                    );
                }
            }
            assert!(seen >= 10, "{builder:?}: {seen} calls");
            // The cap binds somewhere, so the check above has power.
            assert!(
                at_cap > 0,
                "{builder:?} cap {cap} never bound in {seen} calls"
            );
        }
    }
}

#[test]
fn contexts_do_not_depend_on_the_reasoners_hidden_parameters() {
    // The reasoner's `(a, b, c)` and `rho` change what its answers are, never what an arm
    // observed before asking. An arm that asks once per anomaly after a fixed delay therefore asks
    // the same questions with the same contexts under any of them, whatever its builder.
    let always = StreamPolicySpec::Always { delay_ns: 6 * NS };
    let mut compared = 0usize;
    for builder in builders(64) {
        let rung = rung_with(builder);
        let p1 = hard_heavy(3);
        let mut p2 = p1.clone();
        p2.reasoner.b = 2.5;
        p2.reasoner.rho = 0.0;
        let mut p3 = p1.clone();
        p3.reasoner.b = 8.0;
        p3.reasoner.rho = 0.95;
        let a = play_with_rung(&p1, &always, &limits(&p1), &rung).unwrap();
        let b = play_with_rung(&p2, &always, &limits(&p2), &rung).unwrap();
        let c = play_with_rung(&p3, &always, &limits(&p3), &rung).unwrap();
        assert_eq!(calls(&a), calls(&b), "{builder:?}");
        assert_eq!(calls(&a), calls(&c), "{builder:?}");
        compared += calls(&a).len();
    }
    assert!(compared > 20);
}

#[test]
fn different_builders_build_different_contexts() {
    // So that the equalities above are not vacuous.
    let always = StreamPolicySpec::Always { delay_ns: 6 * NS };
    let p = hard_heavy(3);
    let l = limits(&p);
    let default = calls(&play(&p, &always, &l).unwrap());
    assert!(!default.is_empty());
    for builder in builders(64) {
        let other = calls(&play_with_rung(&p, &always, &l, &rung_with(builder)).unwrap());
        assert_ne!(default, other, "{builder:?} built the rung's contexts");
    }
}

#[test]
fn the_selection_oracle_with_any_public_builder_asks_where_always_asks_about_hard_incidents() {
    let mut hard_calls = 0usize;
    for seed in 0..4 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let l = limits(&p);
        let hard_of = |focus: u32| {
            truth
                .incident_of(ObsId(focus))
                .is_some_and(|i| truth.incidents[i as usize].tier == Tier::Hard)
        };
        for builder in builders(32) {
            let rung = rung_with(builder);
            let delay_ns = 4 * NS;
            let always =
                play_with_rung(&p, &StreamPolicySpec::Always { delay_ns }, &l, &rung).unwrap();
            let selection = play_with_rung(
                &p,
                &StreamPolicySpec::OracleSelection {
                    delay_ns,
                    hold_until_asked: false,
                },
                &l,
                &rung,
            )
            .unwrap();
            assert_eq!(always.counts.escalations_refused, 0);
            assert_eq!(selection.counts.escalations_refused, 0);
            let expected: BTreeSet<_> = calls(&always)
                .into_iter()
                .filter(|(_, focus, _)| hard_of(*focus))
                .collect();
            let got = calls(&selection);
            assert_eq!(got, expected, "seed {seed} {builder:?}");
            hard_calls += got.len();
        }
    }
    assert!(hard_calls >= 10, "{hard_calls} hard calls");
}

#[test]
fn the_context_only_selection_oracle_asks_once_about_each_hard_anomaly_with_decisive_evidence() {
    let (mut calls_total, mut with_evidence) = (0usize, 0usize);
    for seed in 0..4 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let l = limits(&p);
        let delay_ns = 12 * NS;
        let selection = play(
            &p,
            &StreamPolicySpec::OracleSelection {
                delay_ns,
                hold_until_asked: false,
            },
            &l,
        )
        .unwrap();
        let context_only = play(
            &p,
            &StreamPolicySpec::OracleSelectionContext { delay_ns },
            &l,
        )
        .unwrap();
        assert_eq!(context_only.counts.escalations_refused, 0);
        // The same anomalies, at the same instants: only the context differs.
        let key = |c: &BTreeSet<(u64, u32, Vec<ObsRef>)>| -> BTreeSet<(u64, u32)> {
            c.iter().map(|(at, f, _)| (*at, *f)).collect()
        };
        let a = calls(&selection);
        let b = calls(&context_only);
        assert_eq!(key(&a), key(&b), "seed {seed}");
        assert_eq!(
            b.len(),
            calls_with_distinct_focus(&b),
            "one call per anomaly"
        );
        for (_, focus, context) in &b {
            calls_total += 1;
            let incident = truth.incident_of(ObsId(*focus)).expect("a hard incident");
            let inc = &truth.incidents[incident as usize];
            assert_eq!(inc.tier, Tier::Hard);
            let decisive: BTreeSet<_> = inc.decisive.iter().map(|o| ObsRef::Passive(*o)).collect();
            assert!(
                context.iter().all(|r| decisive.contains(r)),
                "seed {seed}: a reference that is not decisive evidence"
            );
            with_evidence += usize::from(!context.is_empty());
        }
    }
    assert!(calls_total >= 8, "{calls_total} calls");
    assert!(
        with_evidence * 2 >= calls_total,
        "{with_evidence} of {calls_total} held evidence"
    );
}

fn calls_with_distinct_focus(c: &BTreeSet<(u64, u32, Vec<ObsRef>)>) -> usize {
    c.iter().map(|(_, f, _)| *f).collect::<BTreeSet<_>>().len()
}

// ---- The guard covers the builder module

#[test]
fn the_builder_module_is_a_policy_file_and_names_nothing_a_policy_may_not_see() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stream/arms");
    let path = dir.join("context.rs");
    let text = fs::read_to_string(&path).expect("the builder module is under src/stream/arms");
    // `tests/stream_arms.rs` bans these words from every file of that directory, and
    // `scripts/check-no-oracle.sh` bans them from the same directory; repeated here for the
    // builder file by name so that moving it out of the directory fails this test.
    for word in [
        "StreamTruth",
        "CallSummary",
        "CallTrace",
        "IncidentTruth",
        "StreamVerdict",
        "StreamParams",
        "StreamSimulator",
        "OraclePlan",
        "PlanIncident",
        "gordian_stream_eval",
        "truth_from_stream",
        "calls_from_sim",
        "reveal",
        ["oracle", "::"].concat().as_str(),
    ] {
        assert!(!text.contains(word), "context.rs contains {word}");
    }
    // The builder entry points take a view of public information and parameters, nothing else.
    for signature in [
        "pub fn window(view: &PublicView<'_>, window_ns: u64, max_refs: u32)",
        "pub fn cooccur(view: &PublicView<'_>, delta_ns: u64, max_refs: u32)",
        "pub fn neighbourhood(view: &PublicView<'_>, hops: u32, max_refs: u32)",
        "pub fn build(builder: &ContextBuilder, view: &PublicView<'_>)",
    ] {
        assert!(text.contains(signature), "{signature}");
    }
    // The view's fields are public information the rung holds: observations, the graph, the
    // instant of the call, the anchor and the site.
    let view = text
        .split("pub struct PublicView")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("the view is defined in the module");
    let fields: Vec<&str> = view
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .collect();
    assert_eq!(
        fields,
        [
            "store",
            "services",
            "now",
            "anchor_at",
            "site",
            "burst_gap_ns",
            "lookback_ns"
        ]
    );
}

// ---- `rung` changed nothing

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stream-r6-regression")
}

fn fixture_manifest() -> StreamManifest {
    let text = fs::read_to_string(fixture_dir().join("manifest.json")).unwrap();
    serde_json::from_str(&text).expect("the pre-R6 manifest parses")
}

fn assert_matches_fixtures(m: &StreamManifest, name: &str) {
    let out = scratch(name).join("run");
    execute_stream(m, &out).unwrap_or_else(|e| panic!("{e}"));
    for arm in &m.arms {
        for file in ["results.csv", "incidents.csv"] {
            let expected =
                fs::read_to_string(fixture_dir().join(format!("{}.{file}", arm.arm))).unwrap();
            let got = fs::read_to_string(out.join(&arm.arm).join(file)).unwrap();
            assert!(
                expected == got,
                "{} {file} differs from the fixture written before R6",
                arm.arm
            );
        }
    }
}

#[test]
fn the_rung_builder_writes_the_results_it_wrote_before_whether_defaulted_or_spelled() {
    let m = fixture_manifest();
    assert_eq!(m.arms.len(), 4);
    // The manifest as it was: nothing names a builder.
    assert!(m.arms.iter().all(|a| a.context.is_none()));
    assert_matches_fixtures(&m, "r6-regression-default");
    // The same arms with the builder named, per arm and for the whole rung.
    let mut per_arm = m.clone();
    for arm in &mut per_arm.arms {
        arm.context = Some(ContextBuilder::Rung);
    }
    assert_matches_fixtures(&per_arm, "r6-regression-per-arm");
    let mut whole = m.clone();
    whole.rung.context = ContextBuilder::Rung;
    assert_matches_fixtures(&whole, "r6-regression-rung");
}

#[test]
fn a_manifest_written_before_builders_serializes_to_the_same_text() {
    let path = fixture_dir().join("manifest.json");
    let text = fs::read_to_string(path).unwrap();
    let m: StreamManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(m.canonical_json(), text);
}

#[test]
fn a_non_default_builder_does_change_the_results_so_the_check_above_has_power() {
    let m = fixture_manifest();
    let mut changed = m.clone();
    for arm in &mut changed.arms {
        arm.context = Some(ContextBuilder::Window {
            window_ns: 20 * NS,
            max_refs: 64,
        });
    }
    let out = scratch("r6-regression-window").join("run");
    execute_stream(&changed, &out).unwrap_or_else(|e| panic!("{e}"));
    let mut differing = 0;
    for arm in &changed.arms {
        let expected =
            fs::read_to_string(fixture_dir().join(format!("{}.results.csv", arm.arm))).unwrap();
        let got = fs::read_to_string(out.join(&arm.arm).join("results.csv")).unwrap();
        differing += usize::from(expected != got);
    }
    // Three arms escalate (always, contradiction, selection); the decoy oracle never does.
    assert_eq!(differing, 3);
}
