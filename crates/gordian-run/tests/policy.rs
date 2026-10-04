//! The registry, the scripted test policy, and `heuristic_only` under the shared decision rule.
//!
//! The other baselines, the shared rule, the privileged arms and the manifest's policy
//! configuration are in `tests/baselines.rs`.

mod common;

use common::*;
use gordian_core::{Charge, Instant, Resource};
use gordian_run::policy::decide::DEFAULT_PATIENCE;
use gordian_run::policy::heuristic_only::HeuristicOnly;
use gordian_run::policy::scripted::ScriptedPolicy;
use gordian_run::policy::{self, Built, Policy, PolicyId, PolicySpec, zero_cost};
use gordian_run::{StopReason, standard_components};
use gordian_world::{Action, ComponentMode, EpisodeClass, generate};

#[test]
fn the_registry_builds_the_known_policies_and_nothing_else() {
    let decide = policy::decide::DecideConfig::default();
    for id in policy::KNOWN {
        let spec = PolicySpec::from_id(id).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(spec.id(), PolicyId::new(*id));
        match policy::build(&spec, &decide, "arm", 1) {
            Built::Public(policy) => {
                assert!(!spec.is_privileged(), "{id}");
                assert_eq!(policy.id(), PolicyId::new(*id));
            }
            Built::Privileged(_) => assert!(spec.is_privileged(), "{id}"),
        }
    }
    assert!(PolicySpec::from_id("oracle").is_err());
    assert!(PolicySpec::from_id("").is_err());
}

#[test]
fn a_policy_declares_a_zero_cost_explicitly() {
    assert_eq!(zero_cost(), vec![Charge::new(Resource::Compute, 0)]);
    let state = gordian_components::WorkingState::new(
        gordian_world::generate(&spec(1, EpisodeClass::NoFault, &limits())).public_info(),
        8,
    );
    assert_eq!(
        ScriptedPolicy::never_decides().declared_select_cost(&state),
        zero_cost()
    );
    // A shared-rule arm declares the rule's cost, and it is not zero: the rule is not free.
    let cost = HeuristicOnly::new().declared_select_cost(&state);
    assert_eq!(cost.len(), 1);
    assert_eq!(cost[0].resource, Resource::Compute);
    assert!(cost[0].amount > 0);
}

#[test]
fn heuristic_only_never_probes_and_waits_for_its_patience_on_a_symptom_free_stream() {
    // On NoFault the rule table proposes "no fault" from the first non-empty window, but a lone
    // "no fault" is not acted on early: the rule waits until the clock reaches its patience, then
    // declares it. Steps start at multiples of 50 ms, and the heuristic's declared compute
    // nanoseconds are its time, so the declaration is a few microseconds after 3 s.
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
        let at = v.decision_at.expect("decided");
        assert!(
            at >= DEFAULT_PATIENCE && at.0 < DEFAULT_PATIENCE.0 + 1_000_000,
            "seed {seed}: {at:?}"
        );
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
        assert_eq!(record.verdict.probes_used, 0, "seed {seed}");
    }
    assert!(
        early > 0,
        "no episode was decided before the patience ran out"
    );
}

#[test]
fn heuristic_only_abstains_at_its_patience_when_the_heuristic_never_produced_output() {
    // A Fail directive on the heuristic (component 0) makes it produce nothing, so the shared
    // rule has no candidates; probes are no use without a set, and at the patience the rule
    // abstains. The other seeds of the class, where the heuristic ran, are decided.
    let l = limits();
    let failing = |seed: u64| {
        generate(&spec(seed, EpisodeClass::ComponentTimeout, &l))
            .harness_directives()
            .iter()
            .any(|d| d.component == 0 && d.mode == ComponentMode::Fail)
    };
    let seed = (0..300u64)
        .find(|s| failing(*s))
        .expect("such a seed exists in 300");
    let mut policy = HeuristicOnly::new();
    let mut components = standard_components();
    let record = play(
        seed,
        EpisodeClass::ComponentTimeout,
        &mut policy,
        &mut components,
        &l,
    )
    .unwrap();
    assert_eq!(record.stop, StopReason::Terminal);
    assert!(record.verdict.abstained);
    // The failed heuristic was still charged at every step, and its compute nanoseconds moved
    // the clock, so the abstention is a few microseconds after the patience.
    let at = record.verdict.decision_at.expect("decided");
    assert!(at >= DEFAULT_PATIENCE && at.0 < DEFAULT_PATIENCE.0 + 1_000_000);
    assert_eq!(record.components_run, 61);
}

#[test]
fn heuristic_only_probes_when_the_candidates_are_ambiguous_and_declares_what_remains() {
    // Ambiguous leaves five kinds at the site. The shared rule buys the probe with the smallest
    // expected remaining set until one hypothesis is left, and declares it before the patience.
    let l = limits();
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
        let v = &record.verdict;
        assert!(v.success, "seed {seed}");
        assert!(v.probes_used >= 1, "seed {seed}");
        assert!(v.decision_at.unwrap() < DEFAULT_PATIENCE, "seed {seed}");
    }
}

#[test]
fn heuristic_only_with_no_patience_has_no_candidate_on_an_empty_window_and_abstains() {
    let l = limits();
    let mut policy = HeuristicOnly::with_patience(Instant(0));
    let mut components = standard_components();
    let record = play(2, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    // At t = 0 nothing has arrived, so the heuristic returns no candidate. It ran once, and its
    // compute nanoseconds are its time, so the abstention is a microsecond or so after 0.
    assert!(record.verdict.abstained);
    assert!(record.verdict.decision_at.unwrap().0 < 1_000_000);
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
