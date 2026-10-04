//! Hand-built cases: what each component says about evidence small enough to check by eye, and
//! the places where each is known to be wrong or blind.

mod common;

use common::all_components;
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID,
    PriorRecordLookup, RuleHeuristic, VERIFIER_ID, WorkingState,
};
use gordian_core::{Instant, Resource};
use gordian_world::episode::PriorRecord;
use gordian_world::physics::{HIGH, SignalText, SymptomTag};
use gordian_world::{
    CounterName, EpisodeClass, EpisodeSpec, FaultKind, Hypothesis, Observation, Probe, ProbeKind,
    ProbeResult, PublicInfo, ServiceId, Severity, generate,
};

/// Public info of a generated world, with `records` as its prior records, and a pair
/// `(dependent, site)` such that `dependent` depends on `site`.
fn world(records: Vec<PriorRecord>) -> (PublicInfo, ServiceId, ServiceId) {
    let mut public = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous)).public_info();
    public.prior_records = records;
    let dependent = public
        .services
        .iter()
        .find(|s| !s.depends_on.is_empty())
        .expect("a world with at least one edge");
    (public.clone(), dependent.id, dependent.depends_on[0])
}

fn error_rate(service: ServiceId) -> Observation {
    Observation::Counter {
        service,
        name: CounterName::ErrorRate,
        value: HIGH + 10,
    }
}

fn message(service: ServiceId, text: SignalText) -> Observation {
    Observation::Message {
        service,
        text_id: text.text_id(),
        severity: Severity::Medium,
    }
}

fn window(public: &PublicInfo, capacity: usize, evidence: &[Observation]) -> WorkingState {
    let mut w = WorkingState::new(public.clone(), capacity);
    for (i, o) in evidence.iter().enumerate() {
        w.admit(Instant(i as u64 + 1), o.clone());
    }
    w
}

fn only_entry(out: &gordian_components::ComponentOutput) -> HypothesisEntry {
    assert_eq!(out.entries.len(), 1);
    decode(&out.entries[0].1).unwrap()
}

fn ranked_of(entry: &HypothesisEntry) -> Vec<Hypothesis> {
    match entry {
        HypothesisEntry::Candidates { ranked, .. } => ranked.iter().map(|r| r.hypothesis).collect(),
        HypothesisEntry::EvidenceDamaged { .. } => panic!("expected candidates"),
    }
}

#[test]
fn components_have_distinct_ids_and_work_as_trait_objects() {
    let ids: Vec<_> = all_components().iter().map(|c| c.id()).collect();
    assert_eq!(
        ids,
        vec![HEURISTIC_ID, ESTIMATOR_ID, MEMORY_ID, VERIFIER_ID]
    );
}

#[test]
fn heuristic_reads_a_characteristic_message_as_a_unique_proposal() {
    let (public, _, site) = world(vec![]);
    let w = window(&public, 8, &[message(site, SignalText::Unauthorized)]);
    let out = RuleHeuristic::new().run(&w);
    assert_eq!(
        out.proposal,
        Some(Some((FaultKind::CredentialExpired, site)))
    );
    assert_eq!(out.requests.len(), 1);
    assert_eq!(out.requests[0].component, VERIFIER_ID);
}

#[test]
fn heuristic_on_a_bare_error_rate_lists_every_kind_and_proposes_nothing() {
    let (public, _, site) = world(vec![]);
    let w = window(&public, 8, &[error_rate(site)]);
    let out = RuleHeuristic::new().run(&w);
    assert_eq!(out.proposal, None);
    assert!(out.requests.is_empty());
    let entry = only_entry(&out);
    let expected: Vec<Hypothesis> = FaultKind::ALL.iter().map(|k| Some((*k, site))).collect();
    assert_eq!(ranked_of(&entry), expected);
    let HypothesisEntry::Candidates { tied_at_top, .. } = entry else {
        unreachable!()
    };
    assert_eq!(tied_at_top, 5);
}

#[test]
fn heuristic_calls_silence_no_fault_and_an_empty_window_nothing() {
    let (public, _, site) = world(vec![]);
    let benign = Observation::Counter {
        service: site,
        name: CounterName::ErrorRate,
        value: HIGH - 1,
    };
    let w = window(&public, 8, &[benign]);
    assert_eq!(RuleHeuristic::new().run(&w).proposal, Some(None));
    let empty = window(&public, 8, &[]);
    let out = RuleHeuristic::new().run(&empty);
    assert!(out.entries.is_empty() && out.proposal.is_none());
}

#[test]
fn heuristic_locates_an_upstream_site_through_the_error_rate_and_fails_without_it() {
    let (public, dependent, site) = world(vec![]);
    let evidence = [
        error_rate(site),
        message(dependent, SignalText::UpstreamUnreachable),
    ];
    let full = window(&public, 8, &evidence);
    assert_eq!(
        RuleHeuristic::new().run(&full).proposal,
        Some(Some((FaultKind::DependencyDown, site)))
    );
    // The recency window drops the anchor: the heuristic can say nothing about the site.
    let tight = window(&public, 1, &evidence);
    let out = RuleHeuristic::new().run(&tight);
    assert!(out.entries.is_empty() && out.proposal.is_none());
}

#[test]
fn heuristic_site_follows_whichever_anchor_the_window_kept() {
    // The site's own ErrorRate comes first, then the dependent's, then a latency alarm at the
    // dependent. The heuristic takes the earliest high ErrorRate in the window as the site. With
    // the whole stream that is the true upstream service; once the recency window has dropped
    // the anchor it is the dependent, and the dependent is a candidate the window alone cannot
    // exclude. This is the "may be wrong" of the rule table, shown without any hidden state.
    let (public, dependent, site) = world(vec![]);
    let latency = Observation::Counter {
        service: dependent,
        name: CounterName::Latency,
        value: HIGH + 5,
    };
    let evidence = [error_rate(site), error_rate(dependent), latency];
    let sites = |w: &WorkingState| -> Vec<Option<ServiceId>> {
        ranked_of(&only_entry(&RuleHeuristic::new().run(w)))
            .iter()
            .map(|h| h.map(|(_, s)| s))
            .collect()
    };
    let full = window(&public, 8, &evidence);
    assert!(sites(&full).iter().all(|s| *s == Some(site)));
    let tight = window(&public, 2, &evidence);
    assert!(sites(&tight).iter().all(|s| *s == Some(dependent)));
    let still_consistent = ranked_of(&only_entry(&ConsistencyVerifier::new().run(&tight)));
    assert!(
        still_consistent
            .iter()
            .any(|h| h.map(|(_, s)| s) == Some(dependent)),
        "the checker cannot rule the heuristic's guess out from the tight window"
    );
}

#[test]
fn verifier_reports_damaged_evidence_not_an_empty_world() {
    let (public, dependent, site) = world(vec![]);
    let evidence = [
        error_rate(site),
        message(dependent, SignalText::UpstreamUnreachable),
    ];
    let full = window(&public, 8, &evidence);
    let out = ConsistencyVerifier::new().run(&full);
    assert_eq!(
        ranked_of(&only_entry(&out)),
        vec![Some((FaultKind::DependencyDown, site))]
    );
    assert_eq!(out.proposal, Some(Some((FaultKind::DependencyDown, site))));

    let tight = window(&public, 1, &evidence);
    let out = ConsistencyVerifier::new().run(&tight);
    assert_eq!(out.proposal, None);
    assert_eq!(
        only_entry(&out),
        HypothesisEntry::EvidenceDamaged {
            source: "verifier".to_string(),
            window: 1
        }
    );
    assert_eq!(out.entries[0].0, gordian_core::EntryKind::Hypothesis);
}

#[test]
fn estimator_does_not_invent_a_winner_when_the_verifier_has_none() {
    let (public, dependent, site) = world(vec![]);
    let evidence = [
        error_rate(site),
        message(dependent, SignalText::UpstreamUnreachable),
    ];
    let tight = window(&public, 1, &evidence);
    assert!(
        ConsistencyVerifier::new().run(&tight).proposal.is_none(),
        "the verifier has nothing"
    );
    // The estimator still ranks: every hypothesis forbids the lone message, so all tie at the
    // same lowest level and there is no proposal. It does not invent a winner.
    let out = CountEstimator::new().run(&tight);
    assert_eq!(out.proposal, None);
}

#[test]
fn estimator_penalizes_what_a_hypothesis_forbids() {
    let (public, _, site) = world(vec![]);
    let latency = Observation::Counter {
        service: site,
        name: CounterName::Latency,
        value: HIGH + 1,
    };
    let w = window(&public, 8, &[error_rate(site), latency]);
    let scores = CountEstimator::new().scores(&w);
    let score_of = |h: Hypothesis| scores.iter().find(|(x, _)| *x == h).unwrap().1;
    let permitted = score_of(Some((FaultKind::ResourceExhausted, site)));
    let forbidden = score_of(Some((FaultKind::ConfigDrift, site)));
    assert_eq!(permitted, CountEstimator::BASE + 2);
    assert_eq!(
        forbidden,
        CountEstimator::BASE + 1 - CountEstimator::PENALTY
    );
    assert_eq!(
        score_of(None),
        CountEstimator::BASE - 2 * CountEstimator::PENALTY
    );
    // Three kinds tie at the top: no proposal.
    let out = CountEstimator::new().run(&w);
    assert_eq!(out.proposal, None);
    let HypothesisEntry::Candidates {
        tied_at_top,
        ranked,
        ..
    } = only_entry(&out)
    else {
        unreachable!()
    };
    assert_eq!(tied_at_top, 3);
    assert_eq!(ranked.len(), CountEstimator::TOP_K);
}

#[test]
fn estimator_scores_cover_every_hypothesis_in_the_checkers_order() {
    let (public, _, _) = world(vec![]);
    let w = window(&public, 8, &[]);
    let scores = CountEstimator::new().scores(&w);
    assert_eq!(scores.len(), 1 + 5 * public.services.len());
    assert_eq!(scores[0].0, None);
    assert_eq!(
        scores[1].0,
        Some((FaultKind::ResourceExhausted, ServiceId(0)))
    );
    assert_eq!(
        scores[6].0,
        Some((FaultKind::ResourceExhausted, ServiceId(1)))
    );
    assert!(scores.iter().all(|(_, s)| *s == CountEstimator::BASE));
}

fn record(tags: &[SymptomTag], resolution: FaultKind) -> PriorRecord {
    PriorRecord {
        signature: tags.to_vec(),
        resolution,
    }
}

#[test]
fn lookup_is_fooled_by_a_stale_record_that_the_verifier_refutes() {
    let plain = [SymptomTag::Counter(CounterName::ErrorRate)];
    let (public, _, site) = world(vec![record(&plain, FaultKind::CredentialExpired)]);
    let negative = Observation::Probed {
        probe: Probe {
            kind: ProbeKind::CredentialCheck,
            target: site,
        },
        result: ProbeResult::Negative,
    };
    let w = window(&public, 8, &[error_rate(site), negative]);
    let stale = Some((FaultKind::CredentialExpired, site));

    let out = PriorRecordLookup::new().run(&w);
    assert_eq!(out.proposal, Some(stale), "the lookup follows the record");

    let verified = ranked_of(&only_entry(&ConsistencyVerifier::new().run(&w)));
    assert!(
        !verified.contains(&stale),
        "the evidence refutes the record"
    );
    let ranked = CountEstimator::new().scores(&w);
    let credential = ranked.iter().find(|(h, _)| *h == stale).unwrap().1;
    let resource = ranked
        .iter()
        .find(|(h, _)| *h == Some((FaultKind::ResourceExhausted, site)))
        .unwrap()
        .1;
    assert!(credential < resource);
}

#[test]
fn lookup_matches_equal_signatures_only() {
    let plain = [SymptomTag::Counter(CounterName::ErrorRate)];
    let both = [
        SymptomTag::Counter(CounterName::ErrorRate),
        SymptomTag::Counter(CounterName::Latency),
    ];
    let wider = [
        SymptomTag::Counter(CounterName::ErrorRate),
        SymptomTag::Counter(CounterName::Latency),
        SymptomTag::Counter(CounterName::Saturation),
    ];
    let latency = |service| Observation::Counter {
        service,
        name: CounterName::Latency,
        value: HIGH + 1,
    };
    let (public, _, site) = world(vec![
        record(&plain, FaultKind::ConfigDrift),
        record(&both, FaultKind::Intermittent),
        record(&wider, FaultKind::ResourceExhausted),
    ]);
    // The window shows ErrorRate and Latency. One record is a strict subset of that, one equal,
    // one a strict superset; only the equal one matches.
    let w = window(&public, 8, &[error_rate(site), latency(site)]);
    let out = PriorRecordLookup::new().run(&w);
    assert_eq!(out.proposal, Some(Some((FaultKind::Intermittent, site))));
    let HypothesisEntry::Candidates { ranked, .. } = only_entry(&out) else {
        unreachable!()
    };
    assert_eq!(ranked.len(), 1);
    // No symptoms: nothing matches, nothing is emitted.
    let silent = window(&public, 8, &[]);
    assert!(PriorRecordLookup::new().run(&silent).entries.is_empty());
}

#[test]
fn lookup_votes_and_ties_give_no_proposal() {
    let plain = [SymptomTag::Counter(CounterName::ErrorRate)];
    let (public, _, site) = world(vec![
        record(&plain, FaultKind::ConfigDrift),
        record(&plain, FaultKind::ConfigDrift),
        record(&plain, FaultKind::Intermittent),
    ]);
    let w = window(&public, 8, &[error_rate(site)]);
    let out = PriorRecordLookup::new().run(&w);
    assert_eq!(out.proposal, Some(Some((FaultKind::ConfigDrift, site))));

    let (public, _, site) = world(vec![
        record(&plain, FaultKind::ConfigDrift),
        record(&plain, FaultKind::Intermittent),
    ]);
    let w = window(&public, 8, &[error_rate(site)]);
    assert_eq!(PriorRecordLookup::new().run(&w).proposal, None);
}

#[test]
fn lookup_read_limit_bounds_both_the_reading_and_the_declared_cost() {
    let plain = [SymptomTag::Counter(CounterName::ErrorRate)];
    let records: Vec<_> = (0..10)
        .map(|_| record(&plain, FaultKind::ConfigDrift))
        .collect();
    let (public, _, site) = world(records);
    let w = window(&public, 8, &[error_rate(site)]);

    let all = PriorRecordLookup::new();
    let few = PriorRecordLookup::with_read_limit(3);
    let none = PriorRecordLookup::with_read_limit(0);
    // Record reads are charged in Compute only; the lookup declares no other resource.
    assert!(
        all.declared_cost(&w)
            .iter()
            .all(|ch| ch.resource == Resource::Compute)
    );
    let compute_ns = |c: &PriorRecordLookup| -> u64 {
        c.declared_cost(&w)
            .iter()
            .filter(|ch| ch.resource == Resource::Compute)
            .map(|ch| ch.amount)
            .sum()
    };
    assert!(compute_ns(&all) > compute_ns(&few) && compute_ns(&few) > compute_ns(&none));

    // With a limit of zero nothing is read, so nothing can match.
    let mut none = none;
    assert!(none.run(&w).entries.is_empty());
}

#[test]
fn window_evicts_oldest_first_and_loses_the_anchor() {
    let (public, dependent, site) = world(vec![]);
    let mut w = WorkingState::new(public, 2);
    w.admit(Instant(1), error_rate(site));
    w.admit(Instant(2), message(dependent, SignalText::CheckHealth));
    w.admit(
        Instant(3),
        message(dependent, SignalText::UpstreamUnreachable),
    );
    assert_eq!(w.size(), 2);
    assert_eq!(w.now, Instant(3));
    let held: Vec<Instant> = w.evidence().iter().map(|(t, _)| *t).collect();
    assert_eq!(held, vec![Instant(2), Instant(3)]);
    assert!(
        !w.evidence().iter().any(|(_, o)| *o == error_rate(site)),
        "the anchor was the first thing dropped"
    );
}

#[test]
fn zero_capacity_admits_nothing_but_still_moves_the_clock() {
    let (public, _, site) = world(vec![]);
    let mut w = WorkingState::new(public, 0);
    w.admit(Instant(9), error_rate(site));
    assert_eq!(w.size(), 0);
    assert_eq!(w.now, Instant(9));
    for mut component in all_components() {
        // No component fails on an empty window.
        let _ = component.run(&w);
    }
}
