//! Counted operations in the harness and the shared rule (work item A8b).
//!
//! The units of each component are tested next to the component (`gordian-components`,
//! `tests/counting.rs`) and the checker's next to the checker. This file tests what the harness
//! and the rule add: that every scheduling call is counted exactly once, that components count
//! what ran and not what was merely selected, that privileged arms count nothing, that a count
//! is deterministic and part of the replayed row, that the weighted count in the row is the
//! weighted sum of the counts, and that no decision can depend on a count.

mod common;

use common::*;
use gordian_components::{
    Component, ComponentOutput, ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID, WorkingState,
    ops,
};
use gordian_core::Instant;
use gordian_run::harness::{EpisodeRecord, Limits, StopReason, run_episode_privileged};
use gordian_run::policy::decide::{DecideConfig, Decider, RULE_UNITS, RuleOps};
use gordian_run::policy::privileged::{OracleFactory, Variant};
use gordian_run::policy::{self, Built, Policy, PolicySpec};
use gordian_run::results::{RESULTS_HEADER, results_row};
use gordian_run::standard_components;
use gordian_world::{ComponentMode, EpisodeClass, generate};
use std::fs;
use std::path::Path;

fn arm(spec: &PolicySpec, seed: u64) -> Box<dyn Policy> {
    match policy::build(spec, &DecideConfig::default(), &spec.id().0, seed) {
        Built::Public(p) => p,
        Built::Privileged(_) => panic!("{:?} is privileged", spec.id()),
    }
}

fn play_arm(spec: &PolicySpec, seed: u64, class: EpisodeClass, l: &Limits) -> EpisodeRecord {
    let mut policy = arm(spec, seed);
    let mut components = standard_components();
    play(seed, class, policy.as_mut(), &mut components, l).unwrap()
}

fn tight() -> Limits {
    let mut l = limits();
    l.compute = 60_000;
    l
}

fn unit_count(ops: &gordian_components::Ops, unit: &str) -> u64 {
    let at = ops::units(ops.component())
        .iter()
        .position(|u| u.name == unit)
        .unwrap();
    ops.counts()[at]
}

fn rule_count(ops: &RuleOps, unit: &str) -> u64 {
    let at = RULE_UNITS.iter().position(|u| u.name == unit).unwrap();
    ops.counts()[at]
}

#[test]
fn a_run_counts_the_same_work_every_time() {
    for spec in [PolicySpec::HeuristicOnly, PolicySpec::AllComponents] {
        for class in EpisodeClass::ALL {
            for seed in 0..3u64 {
                let a = play_arm(&spec, seed, class, &limits());
                let b = play_arm(&spec, seed, class, &limits());
                assert_eq!(a.ops, b.ops, "{:?} {class:?} {seed}", spec.id());
                assert_eq!(results_row("r", &a), results_row("r", &b));
            }
        }
    }
}

#[test]
fn only_the_components_that_ran_are_counted() {
    for class in EpisodeClass::ALL {
        for seed in 0..3u64 {
            let h = play_arm(&PolicySpec::HeuristicOnly, seed, class, &limits());
            for o in &h.ops.components {
                if o.component() == HEURISTIC_ID {
                    assert!(o.total() > 0, "{class:?} {seed}");
                    // One call per step the heuristic was selected and ran. `components_run`
                    // also counts a call a `Fail` directive stopped (charged, not run), which
                    // only the ComponentTimeout class makes.
                    let calls = unit_count(o, "calls");
                    if class == EpisodeClass::ComponentTimeout {
                        assert!(calls <= u64::from(h.components_run), "{class:?} {seed}");
                    } else {
                        assert_eq!(calls, u64::from(h.components_run), "{class:?} {seed}");
                    }
                } else {
                    assert_eq!(o.total(), 0, "{class:?} {seed}: {:?}", o.component());
                }
            }
            let all = play_arm(&PolicySpec::AllComponents, seed, class, &limits());
            let calls: u64 = all
                .ops
                .components
                .iter()
                .map(|o| unit_count(o, "calls"))
                .sum();
            if class == EpisodeClass::ComponentTimeout {
                assert!(calls <= u64::from(all.components_run), "{class:?} {seed}");
            } else {
                assert_eq!(calls, u64::from(all.components_run), "{class:?} {seed}");
            }
            for id in [HEURISTIC_ID, ESTIMATOR_ID, MEMORY_ID, VERIFIER_ID] {
                let o = all.ops.components.iter().find(|o| o.component() == id);
                // (A ComponentTimeout episode may make one of them fail, and a failed one counts
                // nothing: the next test.)
                if class != EpisodeClass::ComponentTimeout {
                    assert!(o.is_some_and(|o| o.total() > 0), "{class:?} {seed} {id:?}");
                }
            }
        }
    }
}

#[test]
fn a_component_that_was_charged_but_failed_counted_nothing() {
    // ComponentTimeout episodes mark a component to fail: charged, no output, no work.
    let l = limits();
    let mut checked = 0;
    for seed in 0..40u64 {
        let directives = generate(&spec(seed, EpisodeClass::ComponentTimeout, &l))
            .harness_directives()
            .to_vec();
        let failed: Vec<u32> = directives
            .iter()
            .filter(|d| d.mode == ComponentMode::Fail && d.component < 4)
            .map(|d| d.component)
            .collect();
        if failed.is_empty() {
            continue;
        }
        let record = play_arm(
            &PolicySpec::AllComponents,
            seed,
            EpisodeClass::ComponentTimeout,
            &l,
        );
        for id in failed {
            let o = record
                .ops
                .components
                .iter()
                .find(|o| o.component().0 == id)
                .unwrap();
            assert_eq!(o.total(), 0, "seed {seed}: component {id} failed");
            checked += 1;
        }
        // The others ran and counted.
        assert!(record.ops.ops_component() > 0);
    }
    assert!(checked >= 3, "only {checked} failed components were seen");
}

#[test]
fn privileged_arms_count_nothing() {
    for variant in [Variant::Immediate, Variant::Evidence] {
        for class in EpisodeClass::ALL {
            let factory = OracleFactory::new(variant, DecideConfig::default());
            let l = limits();
            let mut components = standard_components();
            let record =
                run_episode_privileged(&spec(2, class, &l), &factory, &mut components, &l).unwrap();
            assert_eq!(record.ops.ops_component(), 0, "{variant:?} {class:?}");
            assert_eq!(record.ops.ops_sched(), 0, "{variant:?} {class:?}");
            assert_eq!(record.ops.modelled_component_ns(), 0);
            assert_eq!(record.ops.modelled_sched_ns(), 0);
        }
    }
}

/// Every scheduling call is counted once: one `calls` per `decide` the loop made, which is one
/// per step, and one more for the final call when the loop made one.
#[test]
fn the_rule_counts_one_call_per_step_and_one_for_the_final_call() {
    let mut finals = 0;
    let mut plain = 0;
    for l in [limits(), tight()] {
        for spec in [PolicySpec::HeuristicOnly, PolicySpec::AllComponents] {
            for class in EpisodeClass::ALL {
                for seed in 0..4u64 {
                    let r = play_arm(&spec, seed, class, &l);
                    let final_call = matches!(
                        r.stop,
                        StopReason::FinalDeclaration
                            | StopReason::BudgetExhausted
                            | StopReason::Horizon
                    );
                    let expected = u64::from(r.steps) + u64::from(final_call);
                    assert_eq!(
                        rule_count(&r.ops.sched, "calls"),
                        expected,
                        "{:?} {class:?} {seed} {:?}",
                        spec.id(),
                        r.stop
                    );
                    if final_call {
                        finals += 1;
                    } else {
                        plain += 1;
                    }
                }
            }
        }
    }
    assert!(
        finals > 20 && plain > 20,
        "{finals} final calls, {plain} without"
    );
}

#[test]
fn the_weighted_count_in_the_row_is_the_weighted_sum_of_the_counts() {
    for spec in [
        PolicySpec::HeuristicOnly,
        PolicySpec::AllComponents,
        PolicySpec::FixedPipeline(Default::default()),
    ] {
        for class in EpisodeClass::ALL {
            let r = play_arm(&spec, 1, class, &limits());
            // Worked out here from the unit tables, not read back from `modelled_ps`.
            let mut component_ps = 0u64;
            for o in &r.ops.components {
                for (unit, count) in ops::units(o.component()).iter().zip(o.counts()) {
                    component_ps += unit.weight_ps * count;
                }
            }
            let sched_ps: u64 = RULE_UNITS
                .iter()
                .zip(r.ops.sched.counts())
                .map(|(u, c)| u.weight_ps * c)
                .sum();
            assert_eq!(r.ops.modelled_component_ns(), (component_ps + 500) / 1000);
            assert_eq!(r.ops.modelled_sched_ns(), (sched_ps + 500) / 1000);
            // The row carries them, last, in the order the header names them.
            let row = results_row("r", &r);
            let cells: Vec<&str> = row.split(',').collect();
            let names: Vec<&str> = RESULTS_HEADER.split(',').collect();
            assert_eq!(cells.len(), names.len());
            let at = |name: &str| cells[names.iter().position(|n| *n == name).unwrap()];
            assert_eq!(at("ops_component"), r.ops.ops_component().to_string());
            assert_eq!(at("ops_sched"), r.ops.ops_sched().to_string());
            assert_eq!(
                at("modelled_component_ns"),
                r.ops.modelled_component_ns().to_string()
            );
            assert_eq!(
                at("modelled_sched_ns"),
                r.ops.modelled_sched_ns().to_string()
            );
        }
    }
}

#[test]
fn the_new_columns_are_the_last_four_of_the_header() {
    let names: Vec<&str> = RESULTS_HEADER.split(',').collect();
    assert_eq!(
        &names[names.len() - 4..],
        &[
            "ops_component",
            "ops_sched",
            "modelled_component_ns",
            "modelled_sched_ns"
        ]
    );
}

/// A decision cannot depend on a count. Two copies of the rule see the same states and outputs;
/// one has its count taken after every call, the other never does. They act identically.
#[test]
fn the_rule_acts_the_same_whether_or_not_its_count_is_taken() {
    let l = limits();
    let mut compared = 0;
    for class in EpisodeClass::ALL {
        for seed in 0..4u64 {
            let ep = generate(&spec(seed, class, &l));
            let stream = ep.stream().to_vec();
            let mut drained = Decider::new(DecideConfig::default());
            let mut hoarding = Decider::new(DecideConfig::default());
            let mut state = WorkingState::new(ep.public_info(), l.window);
            let mut components = standard_components();
            for (i, (at, obs)) in stream.iter().enumerate() {
                state.admit(*at, obs.clone());
                if i % 3 != 0 {
                    continue;
                }
                let outputs: Vec<_> = components
                    .iter_mut()
                    .map(|c| (c.id(), c.run(&state)))
                    .collect();
                let a = drained.decide(&state, &outputs);
                drained.take_ops();
                let b = hoarding.decide(&state, &outputs);
                assert_eq!(a, b, "{class:?} {seed} step {i}");
                compared += 1;
            }
            let mut late = state.clone();
            late.now = Instant(DecideConfig::default().patience_ns);
            assert_eq!(drained.decide(&late, &[]), hoarding.decide(&late, &[]));
            assert_eq!(drained.decide_final(&late), hoarding.decide_final(&late));
        }
    }
    assert!(compared > 100, "{compared} comparisons");
}

/// The rule's count is taken, and resets: what `take_ops` returns is what was done since the
/// last call, and a second call returns nothing.
#[test]
fn taking_the_count_resets_it() {
    let l = limits();
    let ep = generate(&spec(1, EpisodeClass::Ambiguous, &l));
    let mut state = WorkingState::new(ep.public_info(), l.window);
    for (at, obs) in ep.stream() {
        state.admit(*at, obs.clone());
    }
    let mut components = standard_components();
    let outputs: Vec<_> = components
        .iter_mut()
        .map(|c| (c.id(), c.run(&state)))
        .collect();
    let mut rule = Decider::new(DecideConfig::default());
    assert_eq!(rule.take_ops(), RuleOps::ZERO);
    rule.decide(&state, &outputs);
    let first = rule.take_ops();
    assert_eq!(rule_count(&first, "calls"), 1);
    assert_eq!(rule_count(&first, "scanned"), state.size() as u64);
    assert_eq!(
        rule_count(&first, "decoded_outputs"),
        3,
        "verifier, estimator, heuristic"
    );
    assert!(rule_count(&first, "decoded_ranked") > 0);
    assert_eq!(rule.take_ops(), RuleOps::ZERO);
    // Decoding happens when outputs arrive, not on a call without them.
    rule.decide(&state, &[]);
    let second = rule.take_ops();
    assert_eq!(rule_count(&second, "decoded_outputs"), 0);
    assert_eq!(rule_count(&second, "calls"), 1);
}

// ---- no policy can set or see a count ----

/// A policy is handed a `ComponentOutput`. If it ever grew a field for a count, the pattern below
/// would stop compiling: the destructuring names every field and has no `..`.
#[test]
fn a_component_output_has_no_place_for_a_count() {
    let ep = generate(&spec(1, EpisodeClass::Ambiguous, &limits()));
    let mut state = WorkingState::new(ep.public_info(), 64);
    for (at, obs) in ep.stream() {
        state.admit(*at, obs.clone());
    }
    let output: ComponentOutput = gordian_components::RuleHeuristic::new().run(&state);
    let ComponentOutput {
        entries: _,
        requests: _,
        proposal: _,
    } = output;
}

#[test]
fn only_the_shared_rule_makes_counts_and_policies_never_read_them() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/policy");
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).unwrap();
        // Nothing but the rule builds a `RuleOps` from counts; others use `RuleOps::ZERO` or pass
        // the rule's through.
        if name != "decide.rs" {
            assert!(
                !text.contains("counts:") && !text.contains(".counts()"),
                "{name} builds or reads a count"
            );
            assert!(
                !text.contains("modelled_ps") && !text.contains("weight_ps"),
                "{name} prices a count"
            );
        }
        // Selectors (and the shared wrapper) never name the components' count type.
        if !matches!(name.as_str(), "mod.rs" | "decide.rs") {
            assert!(
                !text.contains("Ops") || name == "oracle.rs" || name == "scripted.rs",
                "{name} names a count"
            );
        }
    }
    // Inside the rule, the count is written by the rule's own steps and read only by the code
    // that hands it over (`take_ops`) and merges a call's count (`decide_at`).
    let rule = fs::read_to_string(dir.join("decide.rs")).unwrap();
    let reads: Vec<&str> = rule.lines().filter(|l| l.contains("self.ops")).collect();
    assert_eq!(
        reads.len(),
        2,
        "self.ops is touched outside take_ops and decide_at: {reads:?}"
    );
}
