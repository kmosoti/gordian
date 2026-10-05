//! Counted operations of the four components (work item A8b).
//!
//! What is tested here is the structure and the honesty of the counts, not their weights: that a
//! count is deterministic and changes nothing the component outputs, that each unit counts what
//! its definition says on windows where the answer can be worked out by hand or from the world's
//! own counted checker, that the counts follow content (the reason they exist), and that a count
//! cannot be built, summed across components or priced by anything but the code that counted.
//! Whether the weighted counts track wall time is a calibration result, not a unit test
//! (`CALIBRATION.md`, section 9).

mod common;

use common::*;
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, Ops,
    PriorRecordLookup, RuleHeuristic, VERIFIER_ID, WorkingState, ops,
};
use gordian_core::{ComponentId, Instant};
use gordian_world::episode::PriorRecord;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{
    HIGH, SignalText, consistent_hypotheses_counted, consistent_hypotheses_reference,
};
use gordian_world::{
    Action, CounterName, EpisodeClass, EpisodeSpec, FaultKind, Observation, Outcome, ProbeKind,
    ServiceId, Severity, Simulator, generate,
};
use proptest::prelude::*;

/// One per call: the `calls` unit, or for the verifier, whose fixed cost is in the envelope of the
/// entry it emits, the one of `candidates` and `damaged` that applies.
fn calls_of(ops: &Ops) -> u64 {
    if ops::units(ops.component())
        .iter()
        .any(|u| u.name == "calls")
    {
        count_of(ops, "calls")
    } else {
        count_of(ops, "candidates") + count_of(ops, "damaged")
    }
}

fn count_of(ops: &Ops, unit: &str) -> u64 {
    let at = ops::units(ops.component())
        .iter()
        .position(|u| u.name == unit)
        .unwrap_or_else(|| panic!("{:?} has no unit {unit}", ops.component()));
    ops.counts()[at]
}

fn probe_result(ep: &gordian_world::Episode, kind: ProbeKind, target: ServiceId) -> Observation {
    let mut sim = Simulator::new(ep.clone());
    match sim.apply(Action::Probe { kind, target }, Instant::ZERO) {
        Outcome::Probed { observation, .. } => observation,
        other => panic!("probe refused: {other:?}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// A count is a function of the input: the same instance twice and a fresh instance give the
    /// same output and the same count, and the output is the output `run` gives.
    #[test]
    fn counts_are_deterministic_and_run_is_the_counted_run(
        spec in spec_strategy(),
        cap in 0usize..120,
        upto in 0usize..400,
    ) {
        let ep = generate(&spec);
        let w = state_of(&ep, cap, upto);
        for (mut a, mut b) in all_components().into_iter().zip(all_components()) {
            let (out1, ops1) = a.run_counted(&w);
            let (out2, ops2) = a.run_counted(&w);
            let (out3, ops3) = b.run_counted(&w);
            prop_assert_eq!(&out1, &out2);
            prop_assert_eq!(&out1, &out3);
            prop_assert_eq!(ops1, ops2);
            prop_assert_eq!(ops1, ops3);
            prop_assert_eq!(&out1, &a.run(&w));
            prop_assert_eq!(ops1.component(), a.id());
            prop_assert_eq!(ops1.counts().len(), ops::units(a.id()).len());
            prop_assert_eq!(calls_of(&ops1), 1);
        }
    }

    /// Every component scans the window once (the verifier twice: the copy into the checker's
    /// slice and the checker's first pass), and the sizes the units are defined on are the sizes
    /// of the input.
    #[test]
    fn scan_and_size_units_are_the_input_sizes(
        spec in spec_strategy(),
        cap in 0usize..120,
        upto in 0usize..400,
    ) {
        let ep = generate(&spec);
        let w = state_of(&ep, cap, upto);
        let n = w.size() as u64;
        let s = w.public.services.len() as u64;
        let (_, h) = RuleHeuristic::new().run_counted(&w);
        prop_assert_eq!(count_of(&h, "scanned"), n);
        prop_assert!(count_of(&h, "rules") <= 15);
        let (_, e) = CountEstimator::new().run_counted(&w);
        prop_assert_eq!(count_of(&e, "scanned"), n);
        prop_assert_eq!(count_of(&e, "hypotheses"), 1 + 5 * s);
        let (_, m) = PriorRecordLookup::new().run_counted(&w);
        prop_assert_eq!(count_of(&m, "scanned"), n);
        // The lookup reads every record when the window shows a symptom and none otherwise.
        let read = count_of(&m, "records");
        prop_assert!(read == 0 || read == w.public.prior_records.len() as u64);
        let (_, v) = ConsistencyVerifier::new().run_counted(&w);
        prop_assert_eq!(count_of(&v, "scanned"), 2 * n);
        prop_assert_eq!(count_of(&v, "worlds"), 1 + 7 * s);
    }

    /// The verifier's counts are the world checker's own counts, and what it emits is what the
    /// reference checker returns: counting changed neither.
    #[test]
    fn the_verifier_counts_what_the_checker_counts_and_emits_what_the_reference_returns(
        spec in spec_strategy(),
        upto in 0usize..400,
    ) {
        let ep = generate(&spec);
        let w = state_of(&ep, 4096, upto);
        let evidence: Vec<_> = w.evidence().iter().cloned().collect();
        let (set, checked) = consistent_hypotheses_counted(&w.public, &evidence);
        let (out, v) = ConsistencyVerifier::new().run_counted(&w);
        prop_assert_eq!(count_of(&v, "worlds"), checked.worlds_tried);
        prop_assert_eq!(count_of(&v, "evals"), checked.evals);
        prop_assert_eq!(count_of(&v, "probe_evals"), checked.probe_evals);
        prop_assert_eq!(count_of(&v, "scanned"), evidence.len() as u64 + checked.scanned);
        let reference = consistent_hypotheses_reference(&w.public, &evidence);
        prop_assert_eq!(&set, &reference);
        match decode(&out.entries[0].1).unwrap() {
            HypothesisEntry::Candidates { ranked, .. } => {
                prop_assert!(!reference.is_empty());
                let got: Vec<_> = ranked.iter().map(|r| r.hypothesis).collect();
                prop_assert_eq!(got, reference.clone());
                prop_assert_eq!(count_of(&v, "ranked"), reference.len() as u64);
            }
            HypothesisEntry::EvidenceDamaged { .. } => {
                prop_assert!(reference.is_empty());
                prop_assert_eq!(count_of(&v, "ranked"), 0);
            }
        }
    }
}

#[test]
fn the_lookup_counts_the_records_it_reads_and_stops_at_its_read_limit() {
    let ep = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous));
    let mut w = state_of(&ep, 4096, ep.stream().len());
    let own = w.public.prior_records.len();
    for extra in [0usize, 5, 200] {
        let mut padded = w.clone();
        for _ in 0..extra {
            padded.public.prior_records.push(PriorRecord {
                signature: vec![gordian_world::physics::SymptomTag::Text(
                    SignalText::CheckHealth,
                )],
                resolution: FaultKind::ConfigDrift,
            });
        }
        let (_, all) = PriorRecordLookup::new().run_counted(&padded);
        assert_eq!(count_of(&all, "records"), (own + extra) as u64);
        let (_, limited) = PriorRecordLookup::with_read_limit(4).run_counted(&padded);
        assert_eq!(count_of(&limited, "records"), 4.min((own + extra) as u64));
    }
    // A window with no symptom reads no record and emits nothing.
    w = state_of(&ep, 4096, 0);
    let (out, silent) = PriorRecordLookup::new().run_counted(&w);
    assert!(out.entries.is_empty());
    assert_eq!(count_of(&silent, "records"), 0);
    assert_eq!(count_of(&silent, "entries"), 0);
}

#[test]
fn an_empty_window_costs_a_call_and_nothing_else() {
    let ep = generate(&EpisodeSpec::new(5, EpisodeClass::Ambiguous));
    let w = state_of(&ep, 64, 0);
    let (out, h) = RuleHeuristic::new().run_counted(&w);
    assert!(out.entries.is_empty());
    assert_eq!(h.counts().iter().sum::<u64>(), 1, "{h:?}");
    let (_, m) = PriorRecordLookup::new().run_counted(&w);
    assert_eq!(m.counts().iter().sum::<u64>(), 1);
}

/// The estimator does work for a probe result that it does not do for a passive stream, and the
/// count says so; the sort the ranking needs is cheap when every score is equal and dearer when
/// the evidence separates the hypotheses.
#[test]
fn estimator_counts_probe_results_and_the_work_of_separating_scores() {
    let ep = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous));
    let services = ep.world().len() as u64;
    let quiet = state_of(&ep, 64, 0);
    let (_, none) = CountEstimator::new().run_counted(&quiet);
    assert_eq!(count_of(&none, "probe_evals"), 0);
    assert_eq!(count_of(&none, "permit_scans"), 0);
    let all_equal_sort = count_of(&none, "sort_cmps");

    let mut w = state_of(&ep, 64, ep.stream().len());
    let (_, passive) = CountEstimator::new().run_counted(&w);
    assert!(count_of(&passive, "permit_scans") > 0);
    assert_eq!(count_of(&passive, "probe_evals"), 0);
    assert!(
        count_of(&passive, "sort_cmps") > all_equal_sort,
        "separated scores must make the sort work: {} against {all_equal_sort}",
        count_of(&passive, "sort_cmps")
    );
    for kind in ProbeKind::ALL {
        w.admit(Instant(w.now.0 + 1), probe_result(&ep, kind, ServiceId(0)));
    }
    let (_, probed) = CountEstimator::new().run_counted(&w);
    // Each probe result is judged against every hypothesis, once or four times for the entangled
    // pair.
    let hypotheses = 1 + 5 * services;
    assert!(count_of(&probed, "probe_evals") >= 6 * hypotheses.min(2 * services));
    assert!(count_of(&probed, "probe_evals") > 0);
}

/// The reason the counts exist: windows of the same size and the same graph that make a
/// component do different amounts of work are counted differently.
#[test]
fn counts_follow_content_not_length() {
    let ep = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous));
    let public = ep.public_info();
    let site = public
        .services
        .iter()
        .find(|s| s.depends_on.is_empty())
        .map(|s| s.id)
        .unwrap();
    let n = 48;
    let build = |obs: &dyn Fn(usize) -> Observation| {
        let mut w = WorkingState::new(public.clone(), n);
        for k in 0..n {
            w.admit(Instant(k as u64), obs(k));
        }
        w
    };
    let alarms = |k: usize| Observation::Counter {
        service: site,
        name: CounterName::ErrorRate,
        value: HIGH + (k as u64 % 3),
    };
    let benign = |_k: usize| Observation::Counter {
        service: site,
        name: CounterName::ErrorRate,
        value: HIGH - 1,
    };
    let loud = build(&alarms);
    let quiet = build(&benign);
    for (name, mut component) in [
        (
            "estimator",
            Box::new(CountEstimator::new()) as Box<dyn Component>,
        ),
        ("verifier", Box::new(ConsistencyVerifier::new())),
        ("heuristic", Box::new(RuleHeuristic::new())),
    ] {
        let (_, a) = component.run_counted(&loud);
        let (_, b) = component.run_counted(&quiet);
        // Same size, same graph, same scan; different work.
        assert_eq!(count_of(&a, "scanned"), count_of(&b, "scanned"), "{name}");
        assert_ne!(a, b, "{name}: the same count for different content");
        assert!(a.total() > b.total(), "{name}: {a:?} against {b:?}");
    }
}

/// A window the heuristic's upstream-site rule decides on (no generated window is one: the
/// site's own message arrives with the dependent's and decides first). It builds a dependents mask
/// for every earlier alarm before it finds the right site, and counts the steps.
#[test]
fn the_upstream_rule_counts_the_masks_it_builds() {
    // A graph with a subject that depends on something, and room for three alarms elsewhere.
    let (ep, subject_id, site, decoys) = (0u64..60)
        .find_map(|seed| {
            let ep = generate(&EpisodeSpec::new(seed, EpisodeClass::Ambiguous));
            let public = ep.public_info();
            let subject = public
                .services
                .iter()
                .rev()
                .find(|s| !s.depends_on.is_empty())?;
            let (subject_id, site) = (subject.id, subject.depends_on[0]);
            let decoys: Vec<ServiceId> = public
                .services
                .iter()
                .map(|s| s.id)
                .filter(|id| *id != subject_id && *id != site)
                .filter(|id| !dependents_mask(&public.services, *id)[subject_id.index()])
                .take(3)
                .collect();
            (decoys.len() == 3).then_some((ep, subject_id, site, decoys))
        })
        .expect("some generated graph has room for three decoys");
    let public = ep.public_info();
    let mut steps = Vec::new();
    for k in 0..=3 {
        let mut w = WorkingState::new(public.clone(), 16);
        let mut at = 1u64;
        for id in decoys.iter().take(k).chain(std::iter::once(&site)) {
            w.admit(
                Instant(at),
                Observation::Counter {
                    service: *id,
                    name: CounterName::ErrorRate,
                    value: HIGH,
                },
            );
            at += 1;
        }
        w.admit(
            Instant(at),
            Observation::Message {
                service: subject_id,
                text_id: SignalText::UpstreamUnreachable.text_id(),
                severity: Severity::Low,
            },
        );
        let (out, h) = RuleHeuristic::new().run_counted(&w);
        assert_eq!(
            out.proposal,
            Some(Some((FaultKind::DependencyDown, site))),
            "k = {k}"
        );
        steps.push(count_of(&h, "mask_steps"));
    }
    assert!(steps[0] > 0);
    assert!(steps.windows(2).all(|p| p[0] < p[1]), "{steps:?}");
}

#[test]
fn units_are_named_once_and_every_known_component_has_some() {
    for id in [HEURISTIC_ID, ESTIMATOR_ID, MEMORY_ID, VERIFIER_ID] {
        let units = ops::units(id);
        assert!(!units.is_empty() && units.len() <= ops::MAX_UNITS);
        // The per-call fixed cost is an explicit unit, never an intercept: `calls`, or for the
        // verifier the envelope of the entry it emits.
        assert!(
            units.iter().any(|u| u.name == "calls")
                || (units.iter().any(|u| u.name == "candidates")
                    && units.iter().any(|u| u.name == "damaged")),
            "{id:?}: no explicit per-call unit"
        );
        let mut names: Vec<_> = units.iter().map(|u| u.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), units.len(), "{id:?} repeats a unit name");
    }
    assert!(ops::units(ComponentId(99)).is_empty());
    assert!(Ops::zero(ComponentId(99)).counts().is_empty());
}

#[test]
fn zero_work_prices_at_zero_and_a_priced_count_is_the_weighted_sum() {
    assert_eq!(Ops::zero(VERIFIER_ID).modelled_ps(), 0);
    let ep = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous));
    let w = state_of(&ep, 256, ep.stream().len());
    for mut component in all_components() {
        let (_, counted) = component.run_counted(&w);
        let expected: u64 = ops::units(component.id())
            .iter()
            .zip(counted.counts())
            .map(|(u, c)| u.weight_ps * c)
            .sum();
        assert_eq!(counted.modelled_ps(), expected, "{:?}", component.id());
    }
}

#[test]
fn counts_of_one_component_add_and_counts_of_two_do_not() {
    let ep = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous));
    let w = state_of(&ep, 256, ep.stream().len());
    let (_, a) = RuleHeuristic::new().run_counted(&w);
    let mut sum = Ops::zero(HEURISTIC_ID);
    sum.accumulate(&a);
    sum.accumulate(&a);
    assert_eq!(sum.total(), 2 * a.total());
    assert_eq!(sum.modelled_ps(), 2 * a.modelled_ps());
}

#[test]
#[should_panic(expected = "different components")]
fn summing_counts_of_different_components_is_refused() {
    let mut a = Ops::zero(HEURISTIC_ID);
    a.accumulate(&Ops::zero(VERIFIER_ID));
}
