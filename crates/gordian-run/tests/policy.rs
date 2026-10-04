//! The policies built in this unit: the scripted test policy and `heuristic_only`.

mod common;

use common::*;
use gordian_core::{Charge, Instant, Resource};
use gordian_run::policy::heuristic_only::{DEFAULT_PATIENCE, HeuristicOnly};
use gordian_run::policy::scripted::ScriptedPolicy;
use gordian_run::policy::{self, Policy, PolicyId, zero_cost};
use gordian_run::{StopReason, standard_components};
use gordian_world::{Action, EpisodeClass};

#[test]
fn the_registry_builds_the_known_policies_and_nothing_else() {
    for id in policy::KNOWN {
        let policy = policy::build(&PolicyId::new(*id)).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(policy.id(), PolicyId::new(*id));
    }
    assert!(policy::build(&PolicyId::new("oracle")).is_none());
    assert!(policy::build(&PolicyId::new("")).is_none());
}

#[test]
fn a_policy_declares_a_zero_cost_explicitly() {
    assert_eq!(zero_cost(), vec![Charge::new(Resource::Compute, 0)]);
    let state = gordian_components::WorkingState::new(
        gordian_world::generate(&spec(1, EpisodeClass::NoFault, &limits())).public_info(),
        8,
    );
    assert_eq!(
        HeuristicOnly::new().declared_select_cost(&state),
        zero_cost()
    );
    assert_eq!(
        ScriptedPolicy::never_decides().declared_select_cost(&state),
        zero_cost()
    );
}

#[test]
fn heuristic_only_never_probes_and_waits_for_its_patience_on_a_symptom_free_stream() {
    // On NoFault the rule table proposes "no fault" from the first non-empty window, but a
    // proposal of no fault is not acted on early: the policy waits until the clock reaches its
    // patience, then declares it. Steps start at multiples of 50 ms, so that is exactly 3 s.
    let l = limits();
    for seed in 0..10u64 {
        let mut policy = HeuristicOnly::new();
        let mut components = standard_components();
        let record = play(
            seed,
            EpisodeClass::NoFault,
            &mut policy,
            &mut components,
            &l,
        )
        .unwrap();
        let v = &record.verdict;
        assert!(v.success && !v.abstained && !v.false_alarm, "seed {seed}");
        assert_eq!(v.decision_at, Some(DEFAULT_PATIENCE), "seed {seed}");
        assert_eq!(v.probes_used, 0);
        assert_eq!(record.stop, StopReason::Terminal);
        // 61 steps (0 ms to 3,000 ms) each ran the heuristic once.
        assert_eq!(record.components_run, 61);
        assert_eq!(record.bill.total(Resource::Probes), 0);
    }
}

#[test]
fn heuristic_only_declares_early_when_the_rule_table_names_a_unique_fault() {
    // DelayedConfigChange carries a changed-configuration snapshot, which the rule table maps to
    // exactly one kind at a known site.
    let l = limits();
    let mut early = 0;
    for seed in 0..10u64 {
        let mut policy = HeuristicOnly::new();
        let mut components = standard_components();
        let record = play(
            seed,
            EpisodeClass::DelayedConfigChange,
            &mut policy,
            &mut components,
            &l,
        )
        .unwrap();
        let at = record.verdict.decision_at.expect("decided");
        if at < DEFAULT_PATIENCE {
            early += 1;
        }
        assert!(record.verdict.success, "seed {seed}");
    }
    assert!(
        early > 0,
        "no episode was decided before the patience ran out"
    );
}

#[test]
fn heuristic_only_abstains_at_its_patience_when_the_heuristic_never_ran() {
    // With no compute the bill refuses the heuristic at every step, so the policy never sees an
    // output. Probes are still affordable, so the arm is not out of means; at its patience the
    // policy has no candidate and abstains.
    let mut l = limits();
    l.compute = 0;
    let mut policy = HeuristicOnly::new();
    let mut components = standard_components();
    let record = play(1, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    assert_eq!(record.stop, StopReason::Terminal);
    assert!(record.verdict.abstained);
    assert_eq!(record.verdict.decision_at, Some(DEFAULT_PATIENCE));
    assert_eq!((record.components_run, record.components_skipped), (0, 61));
}

#[test]
fn heuristic_only_declares_its_first_ranked_candidate_at_its_patience_when_the_ranking_is_tied() {
    // Ambiguous leaves several kinds tied at the site, so the rule table makes no unique
    // proposal. At its patience the policy declares the first-ranked of the tie (the table lists
    // ties in fault-kind order), which is right for some seeds and wrong for others.
    let l = limits();
    let mut right = 0;
    for seed in 0..10u64 {
        let mut policy = HeuristicOnly::new();
        let mut components = standard_components();
        let record = play(
            seed,
            EpisodeClass::Ambiguous,
            &mut policy,
            &mut components,
            &l,
        )
        .unwrap();
        assert_eq!(
            record.verdict.decision_at,
            Some(DEFAULT_PATIENCE),
            "seed {seed}"
        );
        assert!(!record.verdict.abstained, "seed {seed}");
        right += u32::from(record.verdict.success);
    }
    assert!((1..10).contains(&right), "{right} of 10");
}

#[test]
fn heuristic_only_with_no_patience_has_no_candidate_on_an_empty_window_and_abstains() {
    let l = limits();
    let mut policy = HeuristicOnly::with_patience(Instant(0));
    let mut components = standard_components();
    let record = play(2, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    // At t = 0 nothing has arrived, so the heuristic returns no candidate.
    assert!(record.verdict.abstained);
    assert_eq!(record.verdict.decision_at, Some(Instant(0)));
}

#[test]
fn a_scripted_policy_waits_forever_once_its_script_is_spent() {
    let l = limits();
    let mut policy = ScriptedPolicy::actions(vec![Action::Probe {
        kind: gordian_world::ProbeKind::HealthCheck,
        target: gordian_world::ServiceId(0),
    }]);
    let record = play(1, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::Horizon);
    assert_eq!(record.trajectory.len(), 1);
}
