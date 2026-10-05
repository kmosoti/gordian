//! The A6 baselines: the shared decision rule, each selector, the privileged arms, and the
//! manifest's policy configuration.
//!
//! Expected values are derived from the world's public rules and the specification, not read
//! back from the code under test. The tests may read hidden truth through the evaluator's own
//! type (`Truth`) to check the oracles and to count what a policy should have found; policies
//! never can.

mod common;

use common::*;
use gordian_components::payload::{HypothesisEntry, Ranked, hypothesis_entry};
use gordian_components::{
    ComponentOutput, ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID, WorkingState,
};
use gordian_core::{Bill, Charge, ComponentId, EntryKind, Instant, Phase, Resource};
use gordian_eval::Truth;
use gordian_run::manifest::{EpisodeParams, IsolationSpec, Manifest, PRIVILEGED};
use gordian_run::policy::decide::{
    Bought, DEFAULT_PATIENCE, DecideConfig, Decider, RULE, Remaining, probe_units, score_probes,
    worlds_of,
};
use gordian_run::policy::privileged::{OracleFactory, Variant};
use gordian_run::policy::{
    self, Built, Policy, PolicyId, PolicySpec, fixed_pipeline, random_matched,
};
use gordian_run::recorder::execute;
use gordian_run::{EpisodeRecord, run_episode_privileged, standard_components};
use gordian_world::physics::{consistent_hypotheses, consistent_worlds, probe_result};
use gordian_world::{
    Action, EpisodeClass, FaultKind, Hypothesis, Observation, Probe, ProbeKind, ProbeResult,
    ServiceId, Simulator, generate,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

// ---- helpers ----

fn manifest(run_id: &str, arm: &str, policy: PolicySpec, seeds: u64) -> Manifest {
    Manifest {
        run_id: run_id.to_owned(),
        experiment: "test".to_owned(),
        arm: arm.to_owned(),
        source_revision: "0".repeat(40),
        lockfile_sha256: "0".repeat(64),
        toolchain: "rustc test".to_owned(),
        cpu_flags: vec!["avx2".to_owned()],
        isolation: IsolationSpec::default(),
        seeds: (0..seeds).collect(),
        episode_classes: EpisodeClass::ALL
            .iter()
            .map(|c| (*c, seeds as u32))
            .collect(),
        policy,
        arms: Vec::new(),
        run_seed: 0,
        drift_block: 50,
        decide: DecideConfig::default(),
        limits: limits(),
        episode_params: EpisodeParams::default(),
        trace_sample_rate: 0.0,
        internal_external_ratio: None,
    }
}

fn arm_name(spec: &PolicySpec) -> String {
    if spec.is_privileged() {
        format!("{}_{PRIVILEGED}", spec.id().0)
    } else {
        spec.id().0
    }
}

/// One of every registered policy, `random_matched` and `fixed_pipeline` configured.
fn all_specs() -> Vec<PolicySpec> {
    vec![
        PolicySpec::HeuristicOnly,
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![HEURISTIC_ID, VERIFIER_ID],
            every: 3,
        }),
        PolicySpec::AllComponents,
        PolicySpec::RandomMatched(random_matched::Config { p: 0.4 }),
        PolicySpec::OracleImmediate,
        PolicySpec::OracleEvidence,
    ]
}

fn non_privileged_specs() -> Vec<PolicySpec> {
    all_specs()
        .into_iter()
        .filter(|s| !s.is_privileged())
        .collect()
}

/// Run `spec` on `seeds` seeds of every class through the recorder and return `results.csv`.
fn run_results(name: &str, arm: &str, spec: PolicySpec, seeds: u64) -> String {
    let m = manifest(name, arm, spec, seeds);
    let dir = scratch(name);
    execute(&m, &dir).unwrap_or_else(|e| panic!("{name}: {e}"));
    fs::read_to_string(dir.join("results.csv")).unwrap()
}

fn column(csv: &str, name: &str) -> Vec<String> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = header.iter().position(|h| *h == name).unwrap();
    lines
        .map(|l| l.split(',').nth(col).unwrap().to_owned())
        .collect()
}

fn truth_of(s: &gordian_world::EpisodeSpec) -> Hypothesis {
    Truth::from_episode(&generate(s))
        .faults
        .first()
        .map(|f| (f.kind, f.site))
}

/// One episode of a non-privileged arm built the way the recorder builds it.
fn play_built(
    spec: &PolicySpec,
    seed: u64,
    class: EpisodeClass,
    decide: &DecideConfig,
) -> EpisodeRecord {
    let l = limits();
    let mut components = standard_components();
    match policy::build(spec, decide, &arm_name(spec), seed) {
        Built::Public(mut p) => play(seed, class, p.as_mut(), &mut components, &l).unwrap(),
        Built::Privileged(factory) => run_episode_privileged(
            &common::spec(seed, class, &l),
            &factory,
            &mut components,
            &l,
        )
        .unwrap(),
    }
}

// ---- the shared rule ----

/// Wraps a policy and feeds every `decide` call's inputs to a fresh reference [`Decider`], so
/// that the wrapped policy's `decide` can be compared with the shared function directly.
struct Mirror {
    inner: Box<dyn Policy>,
    reference: Decider,
    calls: std::rc::Rc<std::cell::Cell<u32>>,
    probes: std::rc::Rc<std::cell::Cell<u32>>,
}

impl Policy for Mirror {
    fn id(&self) -> PolicyId {
        self.inner.id()
    }
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<Charge> {
        self.inner.declared_select_cost(state)
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.reference.note_remaining(Remaining::of(bill));
        self.inner.select(state, bill)
    }
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        let got = self.inner.decide(state, outputs);
        let want = self.reference.decide(state, outputs);
        assert_eq!(got, want, "decide differs from the shared rule");
        self.calls.set(self.calls.get() + 1);
        if matches!(got, Some(Action::Probe { .. })) {
            self.probes.set(self.probes.get() + 1);
        }
        got
    }
}

#[test]
fn every_non_privileged_arm_reports_the_shared_rule_and_decides_exactly_as_it_does() {
    let l = limits();
    let decide = DecideConfig::default();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let probes = std::rc::Rc::new(std::cell::Cell::new(0));
    for spec in non_privileged_specs() {
        for class in EpisodeClass::ALL {
            for seed in 0..4u64 {
                let Built::Public(inner) = policy::build(&spec, &decide, &arm_name(&spec), seed)
                else {
                    panic!("{:?} is not public", spec.id());
                };
                assert_eq!(inner.decision_rule(), RULE, "{:?}", spec.id());
                let mut mirror = Mirror {
                    inner,
                    reference: Decider::new(decide),
                    calls: calls.clone(),
                    probes: probes.clone(),
                };
                let mut components = standard_components();
                play(seed, class, &mut mirror, &mut components, &l).unwrap();
            }
        }
    }
    // Not vacuous: the rule was called many times, and it bought probes.
    assert!(calls.get() > 1_000, "{} calls", calls.get());
    assert!(probes.get() > 50, "{} probes", probes.get());
}

#[test]
fn only_the_arm_wrapper_the_test_policy_and_the_oracle_implement_policy() {
    // The structural half of the claim: a policy that does not go through `Arm` has its own
    // `decide`. Outside the test instrument and the privileged file there must be exactly one
    // implementation, the wrapper's, and it delegates to `Decider`.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/policy");
    let mut implementers = BTreeSet::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if text.contains("impl Policy for") || text.contains("impl<S: Selector> Policy for") {
            implementers.insert(name.clone());
        }
        // Only the rule itself, the test instrument and the privileged file define a `decide`.
        if !matches!(
            name.as_str(),
            "mod.rs" | "decide.rs" | "scripted.rs" | "oracle.rs"
        ) {
            assert!(
                !text.contains("fn decide("),
                "{name} defines its own decide"
            );
        }
    }
    assert_eq!(
        implementers.into_iter().collect::<Vec<_>>(),
        vec!["mod.rs", "oracle.rs", "scripted.rs"]
    );
    let wrapper = fs::read_to_string(dir.join("mod.rs")).unwrap();
    assert!(wrapper.contains("self.decider.decide(state, outputs)"));
}

#[test]
fn the_default_fixed_pipeline_and_all_components_are_the_same_arm() {
    // Same selection (all four components, every step), same rule: identical results. B1 is
    // what makes the pipeline differ, by tuning it.
    let a = run_results("same-a", "x", PolicySpec::AllComponents, 3);
    let b = run_results(
        "same-b",
        "x",
        PolicySpec::from_id("fixed_pipeline").unwrap(),
        3,
    );
    // run_id is the first column; give both runs the same id by comparing without it.
    let strip = |s: &str| {
        s.lines()
            .map(|l| l.split_once(',').unwrap().1.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&a), strip(&b));
}

#[test]
fn a_fixed_pipeline_of_the_heuristic_alone_is_heuristic_only() {
    // The selector is the only difference between arms, so selecting the same components under
    // the same rule is the same arm.
    let a = run_results("hp-a", "x", PolicySpec::HeuristicOnly, 3);
    let b = run_results(
        "hp-b",
        "x",
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![HEURISTIC_ID],
            every: 1,
        }),
        3,
    );
    let strip = |s: &str| {
        s.lines()
            .map(|l| l.split_once(',').unwrap().1.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&a), strip(&b));
}

fn output(source: &str, ranked: Vec<Hypothesis>, scored: bool) -> ComponentOutput {
    let n = ranked.len() as u32;
    let entry = HypothesisEntry::Candidates {
        source: source.to_owned(),
        basis: "test".to_owned(),
        ranked: ranked
            .into_iter()
            .map(|hypothesis| Ranked {
                hypothesis,
                score: scored.then_some(1),
            })
            .collect(),
        tied_at_top: n,
    };
    ComponentOutput {
        entries: vec![hypothesis_entry(&entry)],
        ..ComponentOutput::default()
    }
}

fn damaged() -> ComponentOutput {
    let entry = HypothesisEntry::EvidenceDamaged {
        source: "verifier".to_owned(),
        window: 0,
    };
    ComponentOutput {
        entries: vec![hypothesis_entry(&entry)],
        ..ComponentOutput::default()
    }
}

fn fresh_state() -> WorkingState {
    let l = limits();
    WorkingState::new(
        generate(&spec(3, EpisodeClass::Ambiguous, &l)).public_info(),
        l.window,
    )
}

const S0: ServiceId = ServiceId(0);
const S1: ServiceId = ServiceId(1);

fn at(kind: FaultKind, site: ServiceId) -> Hypothesis {
    Some((kind, site))
}

fn five_kinds(site: ServiceId) -> Vec<Hypothesis> {
    FaultKind::ALL.into_iter().map(|k| at(k, site)).collect()
}

#[test]
fn the_rule_declares_a_lone_fault_at_once_and_a_lone_no_fault_only_at_the_patience() {
    let mut rule = Decider::default();
    let mut state = fresh_state();
    let one = at(FaultKind::ResourceExhausted, S0);
    let out = vec![(HEURISTIC_ID, output("heuristic", vec![one], false))];
    assert_eq!(
        rule.decide(&state, &out),
        Some(Action::Declare { fault: one })
    );

    let mut rule = Decider::default();
    let out = vec![(HEURISTIC_ID, output("heuristic", vec![None], false))];
    assert_eq!(rule.decide(&state, &out), None);
    state.now = DEFAULT_PATIENCE;
    assert_eq!(
        rule.decide(&state, &[]),
        Some(Action::Declare { fault: None })
    );
}

#[test]
fn the_rule_waits_while_no_fault_is_still_a_candidate_and_then_declares_the_first() {
    let mut rule = Decider::default();
    let mut state = fresh_state();
    let set = vec![
        None,
        at(FaultKind::ConfigDrift, S0),
        at(FaultKind::Intermittent, S0),
    ];
    let out = vec![(VERIFIER_ID, output("verifier", set, false))];
    assert_eq!(rule.decide(&state, &out), None, "no probe while silent");
    state.now = DEFAULT_PATIENCE;
    assert_eq!(
        rule.decide(&state, &[]),
        Some(Action::Declare { fault: None })
    );
}

#[test]
fn the_rule_abstains_at_the_patience_when_it_never_had_a_candidate() {
    let mut rule = Decider::default();
    let mut state = fresh_state();
    assert_eq!(rule.decide(&state, &[]), None);
    state.now = DEFAULT_PATIENCE;
    assert_eq!(rule.decide(&state, &[]), Some(Action::Abstain));
}

#[test]
fn the_rule_buys_the_probe_with_the_smallest_expected_remaining_set() {
    let mut rule = Decider::default();
    let state = fresh_state();
    let set = five_kinds(S0);
    let out = vec![(VERIFIER_ID, output("verifier", set.clone(), false))];
    let Some(Action::Probe { kind, target }) = rule.decide(&state, &out) else {
        panic!("expected a probe");
    };
    let services = &state.public.services;
    let worlds = worlds_of(services, &set, &Bought::default());
    let scores = score_probes(services, &worlds, &Bought::default());
    let best = scores.iter().map(|s| s.numerator).min().unwrap();
    let chosen = scores
        .iter()
        .find(|s| s.probe == Probe { kind, target })
        .unwrap();
    assert_eq!(chosen.numerator, best);
    // Seven worlds (two each for the parity pair, one for each other kind), five hypotheses. A
    // dedicated probe splits one hypothesis off: (1 * 1 + 6 * 4) / 7 < 5. The samples split the
    // pair's worlds without removing a hypothesis from either side: worse.
    assert_eq!(worlds.len(), 7);
    assert!(best < 5 * 7);
    assert_eq!(best, 25);
    assert!(matches!(
        kind,
        ProbeKind::ResourceUsage | ProbeKind::ConfigSnapshot | ProbeKind::CredentialCheck
    ));
    // Ties go to the cheaper probe, then the earlier kind: of the three one-unit dedicated
    // probes, ResourceUsage (2 ms) beats ConfigSnapshot (3 ms) and equals CredentialCheck (2 ms),
    // which comes later in kind order.
    assert_eq!((kind, target), (ProbeKind::ResourceUsage, S0));
}

#[test]
fn the_rule_narrows_its_candidates_by_the_probes_in_the_window_and_declares_what_remains() {
    let mut rule = Decider::default();
    let mut state = fresh_state();
    let set = vec![
        at(FaultKind::ResourceExhausted, S0),
        at(FaultKind::ConfigDrift, S0),
    ];
    let out = vec![(HEURISTIC_ID, output("heuristic", set, false))];
    assert!(matches!(
        rule.decide(&state, &out),
        Some(Action::Probe { .. })
    ));
    // A negative resource probe at the site rules out exhaustion; the heuristic's stored output
    // is unchanged and the rule still declares what is left.
    state.admit(
        Instant(1),
        Observation::Probed {
            probe: Probe {
                kind: ProbeKind::ResourceUsage,
                target: S0,
            },
            result: ProbeResult::Negative,
        },
    );
    assert_eq!(
        rule.decide(&state, &[]),
        Some(Action::Declare {
            fault: at(FaultKind::ConfigDrift, S0)
        })
    );
}

#[test]
fn a_skipped_component_leaves_its_last_output_in_place_and_the_verifier_outranks_the_rest() {
    let mut rule = Decider::default();
    let state = fresh_state();
    let verifier = vec![
        at(FaultKind::ResourceExhausted, S0),
        at(FaultKind::ConfigDrift, S0),
    ];
    let first = vec![(VERIFIER_ID, output("verifier", verifier, false))];
    let a = rule.decide(&state, &first);
    assert!(matches!(a, Some(Action::Probe { .. })));
    // Later only the heuristic runs, and it is sure of something else. The verifier's stored
    // set is stale but still the best available one, so the rule keeps probing it.
    let heuristic = vec![(
        HEURISTIC_ID,
        output("heuristic", vec![at(FaultKind::Intermittent, S1)], false),
    )];
    assert_eq!(rule.decide(&state, &heuristic), a);
    // A verifier that reports a damaged window offers nothing: the heuristic's answer is used.
    let damaged = vec![(VERIFIER_ID, damaged())];
    assert_eq!(
        rule.decide(&state, &damaged),
        Some(Action::Declare {
            fault: at(FaultKind::Intermittent, S1)
        })
    );
}

#[test]
fn the_rule_does_not_buy_a_probe_it_cannot_afford() {
    let mut rule = Decider::default();
    let state = fresh_state();
    let out = vec![(VERIFIER_ID, output("verifier", five_kinds(S0), false))];
    rule.note_remaining(Remaining {
        probes: Some(0),
        time_ns: Some(u64::MAX),
    });
    assert_eq!(rule.decide(&state, &out), None);
    // One unit left: the rule buys a one-unit probe and never one that costs two.
    rule.note_remaining(Remaining {
        probes: Some(1),
        time_ns: Some(u64::MAX),
    });
    let Some(Action::Probe { kind, .. }) = rule.decide(&state, &[]) else {
        panic!("expected a probe");
    };
    assert_eq!(probe_units(kind).0, 1);
}

#[test]
fn the_rule_stalls_on_jointly_decisive_and_never_buys_a_parity_sample() {
    // The pair DependencyDown / Intermittent at one site: each sample splits the worlds (two and
    // two) but leaves both hypotheses on each side, so the expected number of hypotheses left is
    // exactly the number there are now. The rule needs a strict gain and never buys one. This is
    // the reported behaviour of the class: one-step expected-set-size is defeated by it.
    let state = fresh_state();
    let services = &state.public.services;
    let pair = vec![
        at(FaultKind::DependencyDown, S0),
        at(FaultKind::Intermittent, S0),
    ];
    let worlds = worlds_of(services, &pair, &Bought::default());
    assert_eq!(worlds.len(), 4);
    let scores = score_probes(services, &worlds, &Bought::default());
    let bound = 2 * 4;
    for s in &scores {
        assert!(s.numerator >= bound, "{:?} scores {}", s.probe, s.numerator);
    }
    let sample = |kind| scores.iter().find(|s| s.probe.kind == kind).unwrap();
    assert_eq!(sample(ProbeKind::LatencySample).numerator, bound);
    assert_eq!(sample(ProbeKind::ErrorSample).numerator, bound);
    let mut rule = Decider::default();
    let out = vec![(VERIFIER_ID, output("verifier", pair, false))];
    assert_eq!(rule.decide(&state, &out), None);

    // And in the episodes: no non-privileged arm ever buys a parity sample on that class.
    let decide = DecideConfig::default();
    let mut decided_by_guess = 0;
    for spec in non_privileged_specs() {
        for seed in 0..12u64 {
            let record = play_built(&spec, seed, EpisodeClass::JointlyDecisive, &decide);
            for step in &record.trajectory {
                if let Action::Probe { kind, .. } = step.action {
                    assert!(
                        !matches!(kind, ProbeKind::LatencySample | ProbeKind::ErrorSample),
                        "{:?} seed {seed}",
                        spec.id()
                    );
                }
            }
            decided_by_guess += u32::from(record.verdict.decision_at.unwrap() >= DEFAULT_PATIENCE);
        }
    }
    assert!(decided_by_guess > 0, "the fallback guess was never reached");
}

#[test]
fn on_jointly_decisive_the_fallback_names_dependency_down_and_is_right_exactly_then() {
    // The rule stalls (previous test), so every arm ends in the patience fallback, which
    // declares the first of the tied pair: `DependencyDown`, because ties keep fault-kind order.
    // It is therefore right on exactly the episodes whose truth is `DependencyDown`, a share set
    // by the generator and not by any arm.
    let l = limits();
    let decide = DecideConfig::default();
    let (mut down, mut flapping) = (0, 0);
    for seed in 0..20u64 {
        let truth_kind = truth_of(&spec(seed, EpisodeClass::JointlyDecisive, &l))
            .expect("a fault")
            .0;
        for policy in [PolicySpec::AllComponents, PolicySpec::HeuristicOnly] {
            let record = play_built(&policy, seed, EpisodeClass::JointlyDecisive, &decide);
            assert!(
                record.verdict.decision_at.unwrap() >= DEFAULT_PATIENCE,
                "seed {seed}"
            );
            assert_eq!(
                record.verdict.success,
                truth_kind == FaultKind::DependencyDown,
                "seed {seed} {:?}",
                policy.id()
            );
        }
        match truth_kind {
            FaultKind::DependencyDown => down += 1,
            FaultKind::Intermittent => flapping += 1,
            other => panic!("{other:?} in JointlyDecisive"),
        }
    }
    assert!(down > 0 && flapping > 0, "{down} {flapping}");
}

/// The hypothetical drift hash the rule assumes at `target`, restated from the checker's own
/// convention for the reference computation.
fn assumed_drift(
    public: &gordian_world::PublicInfo,
    evidence: &[(Instant, Observation)],
    target: ServiceId,
) -> u64 {
    let start = public.services[target.index()].config_hash;
    evidence
        .iter()
        .find_map(|(_, o)| match o {
            Observation::Probed {
                probe,
                result: ProbeResult::ConfigHash(h),
            } if probe.kind == ProbeKind::ConfigSnapshot
                && probe.target == target
                && *h != start =>
            {
                Some(*h)
            }
            // A passive snapshot that already showed the drifted hash fixes it too. The rule
            // does not read passive snapshots; the partition of worlds by result is the same
            // whichever value a drifted world is assumed to return.
            Observation::Snapshot {
                service,
                config_hash,
            } if *service == target && *config_hash != start => Some(*config_hash),
            _ => None,
        })
        .unwrap_or(start.wrapping_add(1))
}

#[test]
fn expected_set_size_agrees_with_the_checker_over_hypothetical_outcomes() {
    // The rule scores a probe from the worlds that give each result, without calling the checker
    // again. The reference does what the specification says: for every possible result of the
    // probe, add it to the evidence and count what `consistent_hypotheses` returns, weighted by
    // the share of consistent worlds that give that result. They must agree, as integers, for
    // every probe, on prefixes of generated streams and after probes were bought.
    let l = limits();
    let mut compared = 0u64;
    for class in EpisodeClass::ALL {
        for seed in 0..4u64 {
            let ep = generate(&spec(seed, class, &l));
            let public = ep.public_info();
            let stream = ep.stream().to_vec();
            let mut sim = Simulator::new(ep.clone());
            for cut in [stream.len() / 3, stream.len()] {
                let mut evidence = stream[..cut].to_vec();
                // Buy up to two probes at the first service and the second, so that bought
                // evidence is exercised: one dedicated, one parity sample.
                for round in 0..3 {
                    if round > 0 {
                        let kind = [ProbeKind::ResourceUsage, ProbeKind::LatencySample][round - 1];
                        let site = consistent_hypotheses(&public, &evidence)
                            .into_iter()
                            .flatten()
                            .map(|(_, s)| s)
                            .next()
                            .unwrap_or(S0);
                        if let gordian_world::Outcome::Probed {
                            observation,
                            ready_at,
                            ..
                        } = sim.apply(Action::Probe { kind, target: site }, Instant::ZERO)
                        {
                            evidence.push((ready_at, observation));
                        }
                    }
                    let hyps = consistent_hypotheses(&public, &evidence);
                    if hyps.is_empty() {
                        continue;
                    }
                    let bought = Bought::from_evidence(evidence.iter());
                    let worlds = worlds_of(&public.services, &hyps, &bought);
                    let reference_worlds = consistent_worlds(&public, &evidence);
                    assert_eq!(worlds, reference_worlds, "{class:?} seed {seed}");
                    for score in score_probes(&public.services, &worlds, &bought) {
                        let drift = assumed_drift(&public, &evidence, score.probe.target);
                        let mut by_result: Vec<(ProbeResult, u64)> = Vec::new();
                        for (h, bits) in &reference_worlds {
                            let r = probe_result(&public.services, *h, *bits, drift, score.probe);
                            match by_result.iter_mut().find(|(x, _)| *x == r) {
                                Some(slot) => slot.1 += 1,
                                None => by_result.push((r, 1)),
                            }
                        }
                        let mut reference = 0u64;
                        for (result, count) in by_result {
                            let mut after = evidence.clone();
                            after.push((
                                Instant(u64::MAX / 2),
                                Observation::Probed {
                                    probe: score.probe,
                                    result,
                                },
                            ));
                            let left = consistent_hypotheses(&public, &after).len() as u64;
                            reference += count * left;
                        }
                        assert_eq!(
                            score.numerator, reference,
                            "{class:?} seed {seed} cut {cut} round {round} {:?}",
                            score.probe
                        );
                        compared += 1;
                    }
                }
            }
        }
    }
    assert!(compared > 2_000, "only {compared} probes compared");
}

#[test]
fn every_non_privileged_arm_pays_for_the_shared_rule_under_scheduling() {
    let decide = DecideConfig::default();
    for spec in non_privileged_specs() {
        let record = play_built(&spec, 4, EpisodeClass::NoFault, &decide);
        let scheduling: u64 = record
            .bill
            .by_phase(Resource::Compute)
            .filter(|(p, _)| *p == Phase::Scheduling)
            .map(|(_, a)| a)
            .sum();
        assert!(scheduling > 0, "{:?}", spec.id());
    }
    // The privileged arms are a ceiling, not a mechanism: they declare no scheduling cost.
    for spec in [PolicySpec::OracleImmediate, PolicySpec::OracleEvidence] {
        let record = play_built(&spec, 4, EpisodeClass::NoFault, &decide);
        assert_eq!(record.bill.total(Resource::Compute), 0, "{:?}", spec.id());
    }
}

#[test]
fn the_patience_is_the_shared_rules_and_moves_every_arms_fallback() {
    let early = DecideConfig {
        patience_ns: 1_000_000_000,
    };
    for spec in non_privileged_specs() {
        let record = play_built(&spec, 4, EpisodeClass::NoFault, &early);
        let at = record.verdict.decision_at.expect("decided");
        assert!(
            at.0 >= 1_000_000_000 && at.0 < 1_100_000_000,
            "{:?}: {at:?}",
            spec.id()
        );
    }
}

// ---- determinism and the selectors ----

#[test]
fn each_policy_run_twice_on_the_same_manifest_gives_a_byte_identical_results_file() {
    for spec in all_specs() {
        let arm = arm_name(&spec);
        let a = run_results(&format!("det-a-{arm}"), &arm, spec.clone(), 3);
        let b = run_results(&format!("det-b-{arm}"), &arm, spec.clone(), 3);
        // Different run ids would differ in the first column; replace it.
        let normalize = |s: &str| {
            s.replace(&format!("det-a-{arm}"), "id")
                .replace(&format!("det-b-{arm}"), "id")
        };
        assert_eq!(normalize(&a), normalize(&b), "{arm}");
        assert_eq!(a.lines().count(), 1 + 33, "{arm}");
    }
    // The same manifest through the same run id, byte for byte.
    let spec = PolicySpec::RandomMatched(random_matched::Config { p: 0.4 });
    let a = run_results("det-same", "random_matched", spec.clone(), 3);
    let b = run_results("det-same", "random_matched", spec, 3);
    assert_eq!(a, b);
}

#[test]
fn random_matched_with_p_zero_runs_nothing_and_with_p_one_is_all_components() {
    let none = run_results(
        "rm-0",
        "random_matched",
        PolicySpec::RandomMatched(random_matched::Config { p: 0.0 }),
        3,
    );
    assert!(column(&none, "components_run").iter().all(|n| n == "0"));
    assert!(column(&none, "components_skipped").iter().all(|n| n == "0"));
    // No components, so no candidates: probes are never bought, and every episode ends in the
    // fallback, which abstains.
    assert!(column(&none, "probes_used").iter().all(|n| n == "0"));
    assert!(column(&none, "abstained").iter().all(|n| n == "true"));

    // p = 1 selects the same components in the same order as all_components. Both declare a
    // zero selection cost and the same shared rule, so everything in the file is equal, the
    // bill included. (If random_matched ever declares a cost for its draws, only the scheduling
    // part of the bill may differ.)
    let all = run_results("rm-1", "x", PolicySpec::AllComponents, 3);
    let one = run_results(
        "rm-1",
        "x",
        PolicySpec::RandomMatched(random_matched::Config { p: 1.0 }),
        3,
    );
    assert_eq!(
        column(&one, "components_run"),
        column(&all, "components_run")
    );
    assert_eq!(one, all);
}

#[test]
fn random_matched_is_a_function_of_the_episode_seed_and_the_arm_name() {
    assert_eq!(
        random_matched::rng_seed(7, "a"),
        random_matched::rng_seed(7, "a")
    );
    assert_ne!(
        random_matched::rng_seed(7, "a"),
        random_matched::rng_seed(8, "a")
    );
    assert_ne!(
        random_matched::rng_seed(7, "a"),
        random_matched::rng_seed(7, "b")
    );
    let spec = PolicySpec::RandomMatched(random_matched::Config { p: 0.5 });
    let a = run_results("rm-a", "arm-a", spec.clone(), 3);
    let b = run_results("rm-a", "arm-b", spec, 3);
    assert_ne!(
        column(&a, "components_run"),
        column(&b, "components_run"),
        "two arms drew the same stream"
    );
    // About half of the 4 components per step, over the steps taken.
    let ran: u64 = column(&a, "components_run")
        .iter()
        .map(|n| n.parse::<u64>().unwrap())
        .sum();
    assert!(ran > 0);
}

#[test]
fn fixed_pipeline_runs_its_components_at_the_first_step_and_every_kth_after() {
    // NoFault: the shared rule waits until the patience, 61 steps (0 ms to 3,000 ms). With the
    // heuristic and verifier every 3 steps the pipeline runs at steps 0, 3, ..., 60: 21 times.
    let spec = PolicySpec::FixedPipeline(fixed_pipeline::Config {
        components: vec![HEURISTIC_ID, VERIFIER_ID],
        every: 3,
    });
    let record = play_built(&spec, 2, EpisodeClass::NoFault, &DecideConfig::default());
    assert_eq!(record.components_run, 2 * 21);
    assert!(record.verdict.success);
}

// ---- the privileged arms ----

fn evidence_of(record: &EpisodeRecord, window: usize) -> WorkingState {
    let mut state = WorkingState::new(record.public_info.clone(), window);
    for entry in record.ledger.iter() {
        if entry.kind == EntryKind::Measurement && entry.provenance.producer == "harness/sensor" {
            let v: serde_json::Value = serde_json::from_slice(&entry.payload).unwrap();
            let obs: Observation = serde_json::from_value(v["observation"].clone()).unwrap();
            state.admit(Instant(v["at_ns"].as_u64().unwrap()), obs);
        }
    }
    state
}

fn run_oracle(variant: Variant, seed: u64, class: EpisodeClass) -> EpisodeRecord {
    let l = limits();
    let factory = OracleFactory::new(variant, DecideConfig::default());
    let mut components = standard_components();
    run_episode_privileged(&spec(seed, class, &l), &factory, &mut components, &l).unwrap()
}

#[test]
fn oracle_immediate_succeeds_on_every_episode_at_the_first_step() {
    for class in EpisodeClass::ALL {
        for seed in 0..30u64 {
            let record = run_oracle(Variant::Immediate, seed, class);
            assert!(record.verdict.success, "{class:?} seed {seed}");
            assert!(!record.verdict.critical_miss && !record.verdict.false_alarm);
            assert_eq!(record.verdict.decision_at, Some(Instant::ZERO));
            assert_eq!(record.verdict.probes_used, 0);
        }
    }
}

#[test]
fn oracle_evidence_is_nearly_always_right_and_never_declares_before_the_evidence_identifies() {
    let l = limits();
    let mut declarations = 0;
    let mut probing = 0;
    for class in EpisodeClass::ALL {
        let (mut ok, mut n) = (0, 0);
        for seed in 0..30u64 {
            let record = run_oracle(Variant::Evidence, seed, class);
            n += 1;
            ok += u32::from(record.verdict.success);
            if class == EpisodeClass::NoFault {
                assert!(!record.verdict.false_alarm, "seed {seed}");
            }
            probing += u32::from(record.verdict.probes_used > 0);
            // The property that makes it an ideal observer and not a second oracle_immediate:
            // at the step it declared, the public evidence in its working state, read with the
            // world's rules, leaves exactly the true hypothesis. Recomputed here from the ledger.
            if let Some(last) = record.trajectory.last()
                && let Action::Declare { fault } = last.action
            {
                let s = spec(seed, class, &l);
                let truth = truth_of(&s);
                assert_eq!(
                    fault, truth,
                    "{class:?} seed {seed} declared the wrong thing"
                );
                let state = evidence_of(&record, l.window);
                let window: Vec<_> = state.evidence().iter().cloned().collect();
                assert_eq!(
                    consistent_hypotheses(&state.public, &window),
                    vec![truth],
                    "{class:?} seed {seed}: declared before the evidence identified the truth"
                );
                declarations += 1;
            }
        }
        if class != EpisodeClass::NoFault {
            assert!(f64::from(ok) / f64::from(n) >= 0.95, "{class:?}: {ok}/{n}");
        }
    }
    assert!(
        declarations > 300,
        "only {declarations} declarations were checked"
    );
    assert!(probing > 200, "the arm hardly probed: {probing}");
}

#[test]
fn oracle_evidence_reaches_every_faulted_class_by_probing_where_the_stream_does_not_decide() {
    // The two probe counts the world's own headroom test pins: a JointlyDecisive episode needs
    // both parity samples, which the shared rule never buys.
    let samples = |record: &EpisodeRecord| {
        record
            .trajectory
            .iter()
            .filter(|s| {
                matches!(
                    s.action,
                    Action::Probe {
                        kind: ProbeKind::LatencySample | ProbeKind::ErrorSample,
                        ..
                    }
                )
            })
            .count()
    };
    for seed in 0..10u64 {
        let record = run_oracle(Variant::Evidence, seed, EpisodeClass::JointlyDecisive);
        assert!(record.verdict.success);
        assert_eq!(samples(&record), 2, "seed {seed}");
    }
}

#[test]
fn only_the_oracle_file_names_the_truth() {
    // The test-time half of the privileged path: no file under `policy/` except `oracle.rs`
    // names the evaluator, its truth type, the generated-episode type, the simulator or the
    // accessor. `scripts/check-no-oracle.sh` is the same check as a grep over the whole tree.
    fn has_word(text: &str, word: &str) -> bool {
        text.match_indices(word).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            let after = text[i + word.len()..].chars().next();
            let edge = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            edge(before) && edge(after)
        })
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/policy");
    let mut naming_truth = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).unwrap();
        if has_word(&text, "Truth") {
            naming_truth.push(name.clone());
        }
        if name == "oracle.rs" {
            continue;
        }
        for word in ["Truth", "Episode", "Simulator", "gordian_eval"] {
            assert!(!has_word(&text, word), "{name} names {word}");
        }
        // Built from pieces so that this file does not trip the guard it mirrors.
        for fragment in [["oracle", "::"].concat().as_str(), "reveal"] {
            assert!(!text.contains(fragment), "{name} contains {fragment}");
        }
    }
    assert_eq!(naming_truth, vec!["oracle.rs"]);
    // And nothing else in the crate's own manifest turns the hidden-state feature on.
    let cargo =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("reveal-hidden-state"));
}

#[test]
fn only_oracle_policies_are_built_from_the_truth() {
    // The registry half: `build` returns a policy for every arm except the two privileged ones,
    // which come back as a factory. (That nothing else takes a truth is by construction, with a
    // `compile_fail` doctest on the private policy type, and by the source scan above; it is not
    // something a runtime test can show.)
    let decide = DecideConfig::default();
    for spec in all_specs() {
        let privileged = matches!(
            policy::build(&spec, &decide, &arm_name(&spec), 0),
            Built::Privileged(_)
        );
        assert_eq!(privileged, spec.is_privileged(), "{:?}", spec.id());
    }
    // Through the privileged entry point the factory yields a policy, and its ledger entries are
    // labelled with the oracle's id.
    let factory = OracleFactory::new(Variant::Evidence, decide);
    assert_eq!(factory.variant(), Variant::Evidence);
    let l = limits();
    let record = {
        let mut components = standard_components();
        run_episode_privileged(
            &spec(1, EpisodeClass::Ambiguous, &l),
            &factory,
            &mut components,
            &l,
        )
        .unwrap()
    };
    assert!(
        record
            .ledger
            .iter()
            .any(|e| e.provenance.producer == "policy/oracle_evidence")
    );
}

// ---- the manifest ----

#[test]
fn a_manifest_refuses_an_oracle_arm_whose_name_does_not_say_privileged() {
    for spec in [PolicySpec::OracleImmediate, PolicySpec::OracleEvidence] {
        let bad = manifest("m", "oracle", spec.clone(), 2);
        assert!(bad.validate().unwrap_err().contains(PRIVILEGED));
        assert!(matches!(
            execute(&bad, &scratch("priv-bad")).unwrap_err(),
            gordian_run::recorder::RunError::Manifest(_)
        ));
        let ok = manifest("m", &arm_name(&spec), spec, 2);
        assert!(ok.validate().is_ok());
        assert!(ok.arm.contains("privileged"));
        // And the name is in the manifest the run writes.
        let dir = scratch("priv-ok");
        execute(&ok, &dir).unwrap();
        assert!(
            fs::read_to_string(dir.join("manifest.json"))
                .unwrap()
                .contains("_privileged")
        );
    }
    // Public arms need no such name.
    assert!(
        manifest("m", "anything", PolicySpec::AllComponents, 2)
            .validate()
            .is_ok()
    );
}

#[test]
fn policy_configuration_parses_validates_and_round_trips() {
    let parse = |s: &str| serde_json::from_str::<PolicySpec>(s);
    assert_eq!(
        parse(r#""heuristic_only""#).unwrap(),
        PolicySpec::HeuristicOnly
    );
    assert_eq!(
        parse(r#""fixed_pipeline""#).unwrap(),
        PolicySpec::FixedPipeline(fixed_pipeline::Config::default())
    );
    assert_eq!(
        parse(r#"{"policy":"fixed_pipeline","components":["verifier","heuristic"],"every":3}"#)
            .unwrap(),
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![VERIFIER_ID, HEURISTIC_ID],
            every: 3
        })
    );
    assert_eq!(
        parse(r#"{"policy":"random_matched","p":0.25}"#).unwrap(),
        PolicySpec::RandomMatched(random_matched::Config { p: 0.25 })
    );
    for bad in [
        r#""no_such_policy""#,
        r#"{"policy":"random_matched","p":1.5}"#,
        r#"{"policy":"random_matched","p":-0.1}"#,
        r#"{"policy":"random_matched","every":2}"#,
        r#"{"policy":"random_matched","components":["heuristic"]}"#,
        r#"{"policy":"fixed_pipeline","p":0.5}"#,
        r#"{"policy":"fixed_pipeline","components":[]}"#,
        r#"{"policy":"fixed_pipeline","components":["nope"]}"#,
        r#"{"policy":"fixed_pipeline","components":["heuristic","heuristic"]}"#,
        r#"{"policy":"fixed_pipeline","every":0}"#,
        r#"{"policy":"heuristic_only","p":0.5}"#,
        r#"{"policy":"oracle_evidence","every":1}"#,
        r#"{"policy":"all_components","bogus":1}"#,
    ] {
        assert!(parse(bad).is_err(), "{bad} was accepted");
    }
    for spec in all_specs() {
        let text = serde_json::to_string(&spec).unwrap();
        assert_eq!(parse(&text).unwrap(), spec, "{text}");
    }
    // Policies with nothing to configure are written as their id, as before.
    assert_eq!(
        serde_json::to_string(&PolicySpec::AllComponents).unwrap(),
        r#""all_components""#
    );
    // Every parameter is written, so a manifest records what ran.
    assert_eq!(
        serde_json::to_string(&PolicySpec::FixedPipeline(fixed_pipeline::Config::default()))
            .unwrap(),
        r#"{"policy":"fixed_pipeline","components":["heuristic","estimator","memory","verifier"],"every":1}"#
    );
}

#[test]
fn a_manifest_written_before_the_baselines_still_parses_and_runs() {
    // Before A6 the policy was a bare id and there was no `decide` section.
    let m = manifest("old", "heuristic_only", PolicySpec::HeuristicOnly, 2);
    let mut value: serde_json::Value = serde_json::from_str(&m.canonical_json()).unwrap();
    assert_eq!(value["policy"], "heuristic_only");
    value.as_object_mut().unwrap().remove("decide").unwrap();
    let old: Manifest = serde_json::from_value(value).unwrap();
    assert_eq!(old, m);
    let dir = scratch("old");
    execute(&old, &dir).unwrap();
    // The new section is written when a manifest is written now.
    assert!(m.canonical_json().contains("\"patience_ns\""));
}

#[test]
fn the_binary_initializes_a_manifest_for_every_policy_with_its_parameters() {
    let bin = env!("CARGO_BIN_EXE_gordian-run");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = scratch("init-all");
    let init = |name: &str, extra: &[&str]| {
        let out = dir.join(format!("{name}.json"));
        let status = std::process::Command::new(bin)
            .current_dir(&root)
            .args(["init", "--run-id", name])
            .args(["--seed-start", "0", "--seed-count", "2", "--out"])
            .arg(&out)
            .args(extra)
            .output()
            .unwrap();
        (status.status.success(), out)
    };
    let load =
        |p: &Path| -> Manifest { serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap() };

    for id in policy::KNOWN {
        let (ok, path) = init(id, &["--policy", id]);
        assert!(ok, "{id}");
        let m = load(&path);
        assert_eq!(m.policy.id(), PolicyId::new(*id));
        assert_eq!(m.arm.contains(PRIVILEGED), m.policy.is_privileged(), "{id}");
        assert_eq!(m.decide, DecideConfig::default());
    }
    let (ok, path) = init(
        "fp",
        &[
            "--policy",
            "fixed_pipeline",
            "--components",
            "heuristic,verifier",
            "--every",
            "4",
        ],
    );
    assert!(ok);
    assert_eq!(
        load(&path).policy,
        PolicySpec::FixedPipeline(fixed_pipeline::Config {
            components: vec![HEURISTIC_ID, VERIFIER_ID],
            every: 4
        })
    );
    let (ok, path) = init(
        "rm",
        &[
            "--policy",
            "random_matched",
            "--p",
            "0.3",
            "--patience-ns",
            "2000000000",
        ],
    );
    assert!(ok);
    let m = load(&path);
    assert_eq!(
        m.policy,
        PolicySpec::RandomMatched(random_matched::Config { p: 0.3 })
    );
    assert_eq!(m.decide.patience_ns, 2_000_000_000);

    // Refused: a parameter the policy does not have, an out-of-range one, and an oracle arm
    // without the word.
    assert!(!init("bad1", &["--policy", "heuristic_only", "--p", "0.3"]).0);
    assert!(!init("bad2", &["--policy", "random_matched", "--p", "2"]).0);
    assert!(!init("bad3", &["--policy", "oracle_evidence", "--arm", "oracle"]).0);
    assert!(
        init(
            "ok4",
            &["--policy", "oracle_evidence", "--arm", "my_privileged_arm"]
        )
        .0
    );
}

#[test]
fn the_declared_cost_of_the_rule_is_part_of_every_arms_scheduling_charge() {
    let state = fresh_state();
    let rule = Decider::default().declared_cost(&state);
    assert_eq!(rule.resource, Resource::Compute);
    assert!(rule.amount > 0);
    let decide = DecideConfig::default();
    for spec in non_privileged_specs() {
        let Built::Public(p) = policy::build(&spec, &decide, &arm_name(&spec), 0) else {
            unreachable!()
        };
        // Before any output exists every arm declares exactly the rule's base cost, and nothing
        // for its own selection.
        assert_eq!(
            p.declared_select_cost(&state),
            vec![Charge::new(Resource::Compute, rule.amount)],
            "{:?}",
            spec.id()
        );
    }
}

#[test]
fn memory_is_not_an_input_of_the_shared_rule() {
    // The rule reads the verifier, the estimator and the heuristic. An arm that runs only the
    // memory lookup has nothing to act on, so it abstains: running a component the rule ignores
    // is pure cost. Recorded in POLICIES.md as a property of the rule, not of the component.
    let spec = PolicySpec::FixedPipeline(fixed_pipeline::Config {
        components: vec![MEMORY_ID],
        every: 1,
    });
    let record = play_built(
        &spec,
        1,
        EpisodeClass::StaleMemory,
        &DecideConfig::default(),
    );
    assert!(record.verdict.abstained);
    assert!(record.components_run > 0);
    let _ = ESTIMATOR_ID;
}

#[test]
#[ignore = "a measurement, not a check: run with --release --nocapture; see POLICIES.md"]
fn measure_the_rule_against_its_declared_cost() {
    // Times `Decider::decide` on states built from generated streams, and prints one CSV row per
    // state: the quantities the declared cost is a function of, the declared cost, and two
    // measured times per call. `fresh` builds a new rule and hands it the step's outputs, so it
    // includes decoding them (what an arm that runs all components pays at each step); `steady`
    // calls the same rule again with no outputs. Rows go to stderr; the fit is in `POLICIES.md`.
    let l = limits();
    let decide = DecideConfig::default();
    eprintln!(
        "MEASURE,class,seed,cut,window,dec_outputs,dec_hyps,candidates,worlds,targets,probing,due,steady_declared_ns,fresh_ns,steady_ns"
    );
    for class in EpisodeClass::ALL {
        for seed in 0..12u64 {
            let ep = generate(&spec(seed, class, &l));
            let stream = ep.stream().to_vec();
            for cut in [stream.len() / 3, 2 * stream.len() / 3, stream.len()] {
                let mut state = WorkingState::new(ep.public_info(), l.window);
                for (at, obs) in &stream[..cut] {
                    state.admit(*at, obs.clone());
                }
                let mut components = standard_components();
                let outputs: Vec<_> = components
                    .iter_mut()
                    .map(|c| (c.id(), c.run(&state)))
                    .collect();
                let mut rule = Decider::new(decide);
                rule.decide(&state, &outputs);
                let f = rule.cost_features(&state);
                let declared = rule.declared_cost(&state).amount;
                // The decode term lags a step, so a state that has just decoded carries it.
                // `steady` below decodes nothing, so its declared cost has no decode term.
                let steady_declared = {
                    let mut idle = rule.clone();
                    idle.decide(&state, &[]);
                    idle.declared_cost(&state).amount
                };
                let _ = declared;
                let reps = 300u32;
                let started = std::time::Instant::now();
                for _ in 0..reps {
                    let mut fresh = Decider::new(decide);
                    std::hint::black_box(fresh.decide(&state, &outputs));
                }
                let fresh_ns = started.elapsed().as_nanos() / u128::from(reps);
                let started = std::time::Instant::now();
                for _ in 0..reps {
                    std::hint::black_box(rule.decide(&state, &[]));
                }
                let steady_ns = started.elapsed().as_nanos() / u128::from(reps);
                eprintln!(
                    "MEASURE,{class:?},{seed},{cut},{},{},{},{},{},{},{},{},{steady_declared},{fresh_ns},{steady_ns}",
                    f.window,
                    f.decoded_outputs,
                    f.decoded_hypotheses,
                    f.candidates,
                    f.worlds,
                    f.targets,
                    u8::from(f.probing),
                    u8::from(state.now >= Instant(decide.patience_ns))
                );
            }
        }
    }
}
