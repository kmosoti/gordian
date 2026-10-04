//! The episode loop: verdicts, accounting, directives, stopping.
//!
//! Expected values are written by hand from the world's public rules (`gordian-world`
//! `DESIGN.md`, section 1: HealthCheck costs 1 probe and 1 ms, LatencySample 2 probes and 8 ms,
//! Correct 3 probes and 50 ms; declaring and abstaining are free) and from the evaluator's
//! numbered rules (`gordian-eval` `RULES.md`), not read back from the code under test.

mod common;

use common::*;
use gordian_core::{
    Bill, Charge, ComponentId, EntryKind, Instant, Phase, Resource, decode_accounting,
};
use gordian_run::harness::{HarnessError, Limits, StopReason};
use gordian_run::policy::scripted::ScriptedPolicy;
use gordian_world::{
    Action, ComponentMode, EpisodeClass, Observation, Outcome, ProbeKind, Refusal, ServiceId,
    generate,
};

const MS: u64 = 1_000_000;

fn declare(spec_seed: u64, class: EpisodeClass) -> Action {
    let s = spec(spec_seed, class, &limits());
    let (kind, site) = truth_fault(&s);
    Action::Declare {
        fault: Some((kind, site)),
    }
}

#[test]
fn world_costs_the_tests_assume_are_the_world_costs() {
    // If the world's cost table changes, the hand-written expectations below must be revisited.
    use gordian_world::physics::{correct_cost, probe_cost};
    assert_eq!(
        probe_cost(ProbeKind::HealthCheck),
        vec![
            Charge::new(Resource::Probes, 1),
            Charge::new(Resource::Time, MS)
        ]
    );
    assert_eq!(
        probe_cost(ProbeKind::LatencySample),
        vec![
            Charge::new(Resource::Probes, 2),
            Charge::new(Resource::Time, 8 * MS)
        ]
    );
    assert_eq!(
        correct_cost(),
        vec![
            Charge::new(Resource::Probes, 3),
            Charge::new(Resource::Time, 50 * MS)
        ]
    );
}

// ---- verdicts, one evaluator rule each ----

#[test]
fn r2_declaring_the_true_fault_is_success_at_time_zero_for_free() {
    for class in [
        EpisodeClass::Ambiguous,
        EpisodeClass::CriticalFault,
        EpisodeClass::JointlyDecisive,
        EpisodeClass::QuietUrgent,
    ] {
        let record = play_script(3, class, vec![declare(3, class)], &limits());
        let v = &record.verdict;
        assert!(v.success, "{class:?}");
        assert!(!v.critical_miss && !v.false_alarm && !v.abstained && !v.undecided);
        assert_eq!((v.probes_used, v.corrections), (0, 0));
        assert_eq!(v.decision_at, Some(Instant(0)));
        assert_eq!(record.stop, StopReason::Terminal);
        assert_eq!(record.steps, 1);
        // Nothing was spent: the scripted policy declares a zero scheduling cost.
        for r in [
            Resource::Compute,
            Resource::Memory,
            Resource::Time,
            Resource::Probes,
            Resource::Communication,
        ] {
            assert_eq!(record.bill.total(r), 0, "{class:?} {r:?}");
        }
        assert_eq!(record.trajectory.len(), 1);
        assert_eq!(record.trajectory[0].outcome, Outcome::Declared);
    }
}

#[test]
fn r3_right_kind_at_the_wrong_site_is_not_success_and_a_critical_miss_when_critical() {
    for (class, critical) in [
        (EpisodeClass::Ambiguous, false),
        (EpisodeClass::CriticalFault, true),
    ] {
        let s = spec(5, class, &limits());
        let (kind, site) = truth_fault(&s);
        let wrong = ServiceId((site.0 + 1) % service_count(&s));
        let record = play_script(
            5,
            class,
            vec![Action::Declare {
                fault: Some((kind, wrong)),
            }],
            &limits(),
        );
        let v = &record.verdict;
        assert!(!v.success, "{class:?}");
        assert_eq!(v.critical_miss, critical, "{class:?}");
        assert!(!v.undecided && !v.abstained && !v.false_alarm);
    }
}

#[test]
fn r4_declaring_no_fault_on_a_faulted_class_is_neither_success_nor_false_alarm() {
    let record = play_script(
        2,
        EpisodeClass::Ambiguous,
        vec![Action::Declare { fault: None }],
        &limits(),
    );
    let v = &record.verdict;
    assert!(!v.success && !v.false_alarm && !v.critical_miss);
}

#[test]
fn r5_r8_abstaining_on_no_fault_is_success_and_an_abstention() {
    let record = play_script(4, EpisodeClass::NoFault, vec![Action::Abstain], &limits());
    let v = &record.verdict;
    assert!(v.success && v.abstained);
    assert!(!v.false_alarm && !v.critical_miss && !v.undecided);
}

#[test]
fn r6_declaring_a_fault_on_no_fault_is_a_false_alarm() {
    let s = spec(4, EpisodeClass::Ambiguous, &limits());
    let (kind, site) = truth_fault(&s);
    let record = play_script(
        4,
        EpisodeClass::NoFault,
        vec![Action::Declare {
            fault: Some((kind, site)),
        }],
        &limits(),
    );
    let v = &record.verdict;
    assert!(!v.success && v.false_alarm);
}

#[test]
fn r7_r8_abstaining_on_a_critical_fault_is_a_critical_miss() {
    let record = play_script(
        6,
        EpisodeClass::QuietUrgent,
        vec![Action::Abstain],
        &limits(),
    );
    let v = &record.verdict;
    assert!(v.abstained && !v.success && v.critical_miss);
}

#[test]
fn r10_r13_a_probe_is_billed_in_units_counted_as_an_action_and_delays_the_next_step() {
    // HealthCheck: 1 probe, 1 ms. The step takes the larger of 50 ms and its busy time.
    let s = spec(3, EpisodeClass::Ambiguous, &limits());
    let (kind, site) = truth_fault(&s);
    let record = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: site,
            },
            Action::Declare {
                fault: Some((kind, site)),
            },
        ],
        &limits(),
    );
    let v = &record.verdict;
    assert_eq!(v.probes_used, 1);
    assert_eq!(v.decision_at, Some(Instant(50 * MS)));
    assert_eq!(record.bill.total(Resource::Probes), 1);
    assert_eq!(record.bill.total(Resource::Time), MS);
    assert_eq!(
        record.bill.by_phase(Resource::Probes).collect::<Vec<_>>(),
        vec![(Phase::Sensing, 1)]
    );
    assert!(v.success);
    assert_eq!(record.steps, 2);
}

#[test]
fn a_step_that_is_busier_than_the_quantum_takes_as_long_as_its_busy_time() {
    // Correct: 3 probes, 50 ms. With 10 ms steps the next step starts at 50 ms; a 1 ms probe
    // under the same quantum lets the next step start at 10 ms.
    let mut l = limits();
    l.step_ns = 10 * MS;
    let s = spec(3, EpisodeClass::Ambiguous, &l);
    let (_, site) = truth_fault(&s);
    let correct = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![Action::Correct { site }, Action::Abstain],
        &l,
    );
    assert_eq!(correct.verdict.corrections, 1);
    assert_eq!(correct.verdict.decision_at, Some(Instant(50 * MS)));
    assert_eq!(correct.bill.total(Resource::Probes), 3);
    assert_eq!(correct.bill.total(Resource::Time), 50 * MS);
    let probe = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: site,
            },
            Action::Abstain,
        ],
        &l,
    );
    assert_eq!(probe.verdict.decision_at, Some(Instant(10 * MS)));
}

#[test]
fn r11_r6_a_correction_on_no_fault_is_a_false_alarm_even_when_the_run_then_abstains() {
    let record = play_script(
        4,
        EpisodeClass::NoFault,
        vec![Action::Correct { site: ServiceId(0) }, Action::Abstain],
        &limits(),
    );
    let v = &record.verdict;
    assert_eq!(v.corrections, 1);
    assert!(v.success && v.false_alarm && v.abstained);
}

#[test]
fn the_probe_result_reaches_the_policy_at_the_next_step_in_instant_order() {
    let s = spec(3, EpisodeClass::Ambiguous, &limits());
    let (_, site) = truth_fault(&s);
    let probe = Action::Probe {
        kind: ProbeKind::HealthCheck,
        target: site,
    };
    let (mut spy, log) = Spy::new(ScriptedPolicy::actions(vec![probe, Action::Abstain]));
    let record = play(3, EpisodeClass::Ambiguous, &mut spy, &mut [], &limits()).unwrap();
    assert_eq!(record.steps, 2);
    let log = log.borrow();
    assert!(
        !log[0]
            .evidence
            .iter()
            .any(|(_, o)| matches!(o, Observation::Probed { .. })),
        "the result cannot be in the state of the step that asked"
    );
    let probed_at = log[1]
        .evidence
        .iter()
        .position(|(_, o)| matches!(o, Observation::Probed { .. }))
        .expect("the result is in the next step's state");
    // Instants are non-decreasing across the whole window, so the result sits at its own
    // instant (the probe's ready time, 1 ms) among whatever arrived meanwhile.
    let instants: Vec<u64> = log[1].evidence.iter().map(|(t, _)| t.0).collect();
    assert!(instants.windows(2).all(|w| w[0] <= w[1]), "{instants:?}");
    assert_eq!(log[1].evidence[probed_at].0, Instant(MS));
}

// ---- budget reconciliation ----

#[test]
fn a_bill_refusal_asks_not_the_world_and_leaves_no_trajectory_step() {
    // 3 probe units: LatencySample takes 2 and leaves 1, so Correct (3) is refused by the bill,
    // and HealthCheck (1) is still affordable, which keeps the run from being out of means.
    let mut l = limits();
    l.probes = 3;
    let s = spec(3, EpisodeClass::Ambiguous, &l);
    let (kind, site) = truth_fault(&s);
    let record = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![
            Action::Probe {
                kind: ProbeKind::LatencySample,
                target: site,
            },
            Action::Correct { site },
            Action::Declare {
                fault: Some((kind, site)),
            },
        ],
        &l,
    );
    // The refused Correct is not in the trajectory and not counted.
    assert_eq!(record.trajectory.len(), 2);
    assert_eq!(record.verdict.corrections, 0);
    assert_eq!(record.verdict.probes_used, 1);
    assert!(record.verdict.success);
    assert_eq!(record.bill.total(Resource::Probes), 2);
    assert_eq!(record.bill.total(Resource::Time), 8 * MS);
    // The refusal is in the ledger, as a refused accounting entry for the correction's cost.
    let refused: Vec<_> = record
        .ledger
        .iter()
        .filter(|e| e.kind == EntryKind::Accounting)
        .map(|e| decode_accounting(&e.payload).unwrap())
        .filter(|a| !a.accepted)
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].phase, Phase::Sensing);
    assert_eq!(
        refused[0].charges,
        vec![
            Charge::new(Resource::Probes, 3),
            Charge::new(Resource::Time, 50 * MS)
        ]
    );
}

#[test]
fn an_arm_with_nothing_affordable_left_stops_undecided() {
    // 2 probe units: LatencySample spends them all. No component, no probe and no correction is
    // affordable afterwards, so the loop stops, even though declaring would still be free.
    let mut l = limits();
    l.probes = 2;
    let s = spec(3, EpisodeClass::CriticalFault, &l);
    let (kind, site) = truth_fault(&s);
    let record = play_script(
        3,
        EpisodeClass::CriticalFault,
        vec![
            Action::Probe {
                kind: ProbeKind::LatencySample,
                target: site,
            },
            Action::Declare {
                fault: Some((kind, site)),
            },
        ],
        &l,
    );
    assert_eq!(record.stop, StopReason::BudgetExhausted);
    assert!(record.verdict.undecided && !record.verdict.success);
    assert!(
        record.verdict.critical_miss,
        "undecided on a critical fault is a miss (R7, R9)"
    );
    assert_eq!(record.verdict.decision_at, None);
    assert_eq!(record.steps, 1);
}

#[test]
fn a_world_refusal_charges_nothing_and_is_a_refused_step() {
    // The bill can afford a probe of a service that does not exist; the world refuses it.
    let record = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: ServiceId(99),
            },
            Action::Abstain,
        ],
        &limits(),
    );
    assert_eq!(record.trajectory.len(), 2);
    assert_eq!(
        record.trajectory[0].outcome,
        Outcome::Refused(Refusal::UnknownService(ServiceId(99)))
    );
    assert_eq!(record.verdict.probes_used, 0);
    assert_eq!(record.bill.total(Resource::Probes), 0);
    assert_eq!(record.bill.total(Resource::Time), 0);
}

#[test]
fn the_bill_and_the_world_agree_on_what_the_actions_cost() {
    // Whatever the world charged for the actions it carried out is exactly what the bill holds
    // for sensing: nothing counted twice, nothing left uncounted.
    let s = spec(8, EpisodeClass::JointlyDecisive, &limits());
    let (kind, site) = truth_fault(&s);
    let record = play_script(
        8,
        EpisodeClass::JointlyDecisive,
        vec![
            Action::Probe {
                kind: ProbeKind::LatencySample,
                target: site,
            },
            Action::Probe {
                kind: ProbeKind::ErrorSample,
                target: site,
            },
            Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: site,
            },
            Action::Declare {
                fault: Some((kind, site)),
            },
        ],
        &limits(),
    );
    let (mut probes, mut time) = (0, 0);
    for step in &record.trajectory {
        if let Outcome::Probed { cost, .. } | Outcome::Corrected { cost, .. } = &step.outcome {
            probes += cost.probes;
            time += cost.time_ns;
        }
    }
    assert_eq!((probes, time), (5, 17 * MS)); // 2 + 2 + 1 units; 8 + 8 + 1 ms
    let sensing = |r| {
        record
            .bill
            .by_phase(r)
            .filter(|(p, _)| *p == Phase::Sensing)
            .map(|(_, a)| a)
            .sum::<u64>()
    };
    assert_eq!(sensing(Resource::Probes), probes);
    assert_eq!(sensing(Resource::Time), time);
    assert_eq!(record.bill.total(Resource::Probes), probes);
    assert_eq!(record.bill.total(Resource::Time), time);
}

// ---- accounting replay ----

#[test]
fn replaying_the_accounting_entries_rebuilds_the_live_bill() {
    let l = limits();
    let (mut components, _) = fakes(3, 1_000, 2 * MS);
    let s = spec(3, EpisodeClass::Ambiguous, &l);
    let (kind, site) = truth_fault(&s);
    let mut policy = ScriptedPolicy::new(vec![
        step(&[0, 1], None),
        step(
            &[2, 2, 7 % 3],
            Some(Action::Probe {
                kind: ProbeKind::HealthCheck,
                target: site,
            }),
        ),
        step(&[], Some(Action::Correct { site })),
        step(
            &[],
            Some(Action::Declare {
                fault: Some((kind, site)),
            }),
        ),
    ])
    .with_select_cost(vec![Charge::new(Resource::Compute, 10)]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    let replayed = Bill::replay(l.budget(), &record.ledger).expect("ledger replays");
    assert_eq!(replayed, record.bill);
    for r in [
        Resource::Compute,
        Resource::Time,
        Resource::Probes,
        Resource::Memory,
    ] {
        let sum: u64 = record.bill.by_phase(r).map(|(_, a)| a).sum();
        assert_eq!(sum, record.bill.total(r), "{r:?}");
    }
    // Hand-check the compute: 4 steps x 10 scheduling, then 1,000 per component run (0, 1, 2, 2
    // again is skipped as a duplicate, 1 again as 7 % 3).
    assert_eq!(record.components_run, 4);
    assert_eq!(record.components_skipped, 1);
    assert_eq!(record.bill.total(Resource::Compute), 4 * 10 + 4 * 1_000);
}

#[test]
fn every_episode_replays_its_bill_and_keeps_the_ledger_kinds_apart() {
    // Real components, real policy and a probing script, over every class and several seeds.
    let l = limits();
    let mut episodes = 0;
    for class in EpisodeClass::ALL {
        for seed in 0..6u64 {
            for busy in [false, true] {
                let mut components = gordian_run::standard_components();
                let mut heuristic = gordian_run::policy::heuristic_only::HeuristicOnly::new();
                let mut script = ScriptedPolicy::new(vec![
                    step(
                        &[0, 1, 2, 3],
                        Some(Action::Probe {
                            kind: ProbeKind::HealthCheck,
                            target: ServiceId(0),
                        }),
                    ),
                    step(&[1, 3], Some(Action::Correct { site: ServiceId(1) })),
                    step(&[0, 1, 2, 3], Some(Action::Abstain)),
                ]);
                let policy: &mut dyn gordian_run::Policy =
                    if busy { &mut script } else { &mut heuristic };
                let record = play(seed, class, policy, &mut components, &l).unwrap();
                episodes += 1;

                let replayed = Bill::replay(l.budget(), &record.ledger).expect("replays");
                assert_eq!(replayed, record.bill, "{class:?} {seed} busy={busy}");
                for r in [
                    Resource::Compute,
                    Resource::Memory,
                    Resource::Time,
                    Resource::Probes,
                    Resource::Communication,
                ] {
                    let sum: u64 = record.bill.by_phase(r).map(|(_, a)| a).sum();
                    assert_eq!(sum, record.bill.total(r));
                    assert!(record.bill.total(r) <= l.budget().limit(r).unwrap());
                }
                // Measurements come only from the sensor and the timer; hypotheses only from
                // components; accounting only from the policy, components and the sensing path.
                for e in record.ledger.iter() {
                    let producer = e.provenance.producer.as_str();
                    match e.kind {
                        EntryKind::Measurement => {
                            assert!(producer == "harness/sensor" || producer == "harness/timer")
                        }
                        EntryKind::Hypothesis => assert!(producer.starts_with("component/")),
                        EntryKind::Accounting => {
                            assert!(producer != "harness/timer");
                            assert!(
                                producer.starts_with("policy/")
                                    || producer.starts_with("component/")
                                    || producer == "harness/sensor"
                            );
                        }
                        _ => {}
                    }
                }
                // The scored trajectory is exactly what the world answered, in time order, and
                // ends at the close if there was one.
                let times: Vec<_> = record.trajectory.iter().map(|s| s.at).collect();
                assert!(times.windows(2).all(|w| w[0] <= w[1]));
                if let Some(last) = record.trajectory.last()
                    && matches!(last.outcome, Outcome::Declared | Outcome::Abstained)
                {
                    assert_eq!(record.stop, StopReason::Terminal);
                }
            }
        }
    }
    assert_eq!(episodes, 132);
}

#[test]
fn the_scheduling_cost_is_charged_under_scheduling_every_step() {
    let mut l = limits();
    l.compute = 2_500;
    let mut policy = ScriptedPolicy::never_decides()
        .with_select_cost(vec![Charge::new(Resource::Compute, 1_000)]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    // 1,000 at step 1 and at step 2; 500 is left, a third scheduling step is not affordable.
    assert_eq!(record.bill.total(Resource::Compute), 2_000);
    assert_eq!(
        record.bill.by_phase(Resource::Compute).collect::<Vec<_>>(),
        vec![(Phase::Scheduling, 2_000)]
    );
    assert_eq!(record.steps, 2);
    assert_eq!(record.stop, StopReason::BudgetExhausted);
    assert!(record.verdict.undecided);
}

#[test]
fn declared_time_moves_the_logical_clock_and_is_billed_to_the_phase_that_spent_it() {
    // Four components of 30 ms each: the first step is busy for 120 ms, longer than the 50 ms
    // quantum, so the next step, and the abstention, are at 120 ms.
    let l = limits();
    let (mut components, _) = fakes(4, 100, 30 * MS);
    let mut policy = ScriptedPolicy::new(vec![
        step(&[0, 1, 2, 3], None),
        step(&[], Some(Action::Abstain)),
    ]);
    let record = play(1, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    assert_eq!(record.verdict.decision_at, Some(Instant(120 * MS)));
    assert_eq!(record.bill.total(Resource::Time), 120 * MS);
    let by_phase: Vec<_> = record.bill.by_phase(Resource::Time).collect();
    assert_eq!(by_phase.len(), 4);
    assert!(
        by_phase
            .iter()
            .all(|(p, a)| matches!(p, Phase::Component(_)) && *a == 30 * MS)
    );

    // The policy's own declared time is charged under scheduling and moves the clock too.
    let mut policy = ScriptedPolicy::new(vec![step(&[], None), step(&[], Some(Action::Abstain))])
        .with_select_cost(vec![
            Charge::new(Resource::Compute, 5),
            Charge::new(Resource::Time, 80 * MS),
        ]);
    let record = play(1, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    // 80 ms of scheduling at step 1; step 2 starts at 80 ms, and spends another 80 ms before
    // deciding, so the abstention is recorded at 160 ms.
    assert_eq!(record.verdict.decision_at, Some(Instant(160 * MS)));
    assert_eq!(
        record.bill.by_phase(Resource::Time).collect::<Vec<_>>(),
        vec![(Phase::Scheduling, 160 * MS)]
    );
}

#[test]
fn a_component_the_bill_refuses_does_not_run_and_is_counted_skipped() {
    let mut l = limits();
    l.compute = 15_000;
    let (mut components, runs) = fakes(1, 10_000, 0);
    let s = spec(3, EpisodeClass::Ambiguous, &l);
    let (kind, site) = truth_fault(&s);
    let mut policy = ScriptedPolicy::new(vec![
        step(&[0], None),
        step(&[0], None),
        step(
            &[],
            Some(Action::Declare {
                fault: Some((kind, site)),
            }),
        ),
    ]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    assert_eq!(runs[0].get(), 1);
    assert_eq!((record.components_run, record.components_skipped), (1, 1));
    assert_eq!(record.bill.total(Resource::Compute), 10_000);
    assert_eq!(record.stop, StopReason::Terminal);
}

// ---- directives ----

#[test]
fn directives_fail_and_slow_change_the_bill_exactly_as_specified() {
    // Four fake components 0..3 each declaring 100 compute and 1 ms of time. The world's
    // directives name component indices 0..8; indices 4.. are not components of this run.
    let l = limits();
    let (mut seen_fail, mut seen_slow, mut seen_ignored) = (0, 0, 0);
    for seed in 0..80u64 {
        let s = spec(seed, EpisodeClass::ComponentTimeout, &l);
        let directives = generate(&s).harness_directives().to_vec();
        let (mut components, runs) = fakes(4, 100, MS);
        let mut policy = ScriptedPolicy::new(vec![
            step(&[0, 1, 2, 3], None),
            step(&[], Some(Action::Abstain)),
        ]);
        let record = play(
            seed,
            EpisodeClass::ComponentTimeout,
            &mut policy,
            &mut components,
            &l,
        )
        .unwrap();

        let mut expected_time = 0;
        let mut expected_compute = 0;
        let mut expected_failed = Vec::new();
        let mut expected_ignored = 0;
        for id in 0..4u32 {
            let mut factor = 1;
            for d in directives.iter().filter(|d| d.component == id) {
                match d.mode {
                    ComponentMode::Fail => expected_failed.push(id),
                    ComponentMode::Slow { factor: f } => factor = u64::from(f),
                }
            }
            expected_time += factor * MS;
            expected_compute += factor * 100;
        }
        for d in &directives {
            if d.component >= 4 {
                expected_ignored += 1;
            }
        }
        // Every component was charged, failed or not. A slowed one pays its factor in Compute
        // as well as in Time: a component's compute nanoseconds are its time (HARNESS.md, 5).
        assert_eq!(
            record.bill.total(Resource::Compute),
            expected_compute,
            "seed {seed}"
        );
        assert_eq!(
            record.bill.total(Resource::Time),
            expected_time,
            "seed {seed}"
        );
        assert_eq!(record.components_run, 4, "seed {seed}");
        assert_eq!(record.directives_ignored, expected_ignored, "seed {seed}");
        // A failed component's run was not called; the others ran once.
        for (id, counter) in runs.iter().enumerate() {
            let failed = expected_failed.contains(&(id as u32));
            assert_eq!(
                counter.get(),
                u32::from(!failed),
                "seed {seed} component {id}"
            );
        }
        // The ledger records only that nothing came out, not why.
        let nothing: Vec<_> = record
            .ledger
            .iter()
            .filter(|e| e.kind == EntryKind::ComputationResult)
            .collect();
        assert_eq!(nothing.len(), expected_failed.len(), "seed {seed}");
        // The second step starts when the first ended: 50 ms or the busy time, whichever is
        // longer, and that is where the abstention was recorded.
        assert_eq!(
            record.verdict.decision_at,
            Some(Instant((50 * MS).max(expected_time))),
            "seed {seed}"
        );
        seen_fail += expected_failed.len();
        seen_slow += directives
            .iter()
            .filter(|d| matches!(d.mode, ComponentMode::Slow { .. }) && d.component < 4)
            .count();
        seen_ignored += expected_ignored as usize;
    }
    // The loop must have exercised each case, or the equalities above prove little.
    assert!(
        seen_fail > 0 && seen_slow > 0 && seen_ignored > 0,
        "{seen_fail} {seen_slow} {seen_ignored}"
    );
}

#[test]
fn slow_changes_the_bill_and_the_clock_of_the_real_components() {
    // The A5 components declare only Compute nanoseconds. Since a component's compute
    // nanoseconds are its time (HARNESS.md, 5), a Slow directive multiplies the Compute charge
    // and the clock advance. A script that runs one component and abstains in the same step
    // records its decision when the component's busy time has passed, so the decision instant is
    // the clock advance, and the bill is the charge.
    let l = limits();
    let run_one = |class: EpisodeClass, seed: u64, id: u32| {
        let s = spec(seed, class, &l);
        let state = gordian_components::WorkingState::new(generate(&s).public_info(), l.window);
        let declared: u64 = gordian_run::standard_components()
            .iter()
            .find(|c| c.id() == ComponentId(id))
            .unwrap()
            .declared_cost(&state)
            .iter()
            .filter(|c| c.resource == Resource::Compute)
            .map(|c| c.amount)
            .sum();
        let mut policy = ScriptedPolicy::new(vec![step(&[id], Some(Action::Abstain))]);
        let mut components = gordian_run::standard_components();
        let record = play(seed, class, &mut policy, &mut components, &l).unwrap();
        (
            declared,
            record.bill.total(Resource::Compute),
            record.bill.total(Resource::Time),
            record.verdict.decision_at,
        )
    };
    let mut seen = 0;
    for seed in 0..300u64 {
        let directives = generate(&spec(seed, EpisodeClass::ComponentTimeout, &l))
            .harness_directives()
            .to_vec();
        let slow = directives.iter().find_map(|d| match d.mode {
            ComponentMode::Slow { factor } if d.component < 4 => Some((d.component, factor)),
            _ => None,
        });
        let Some((id, factor)) = slow else { continue };

        let (declared, bill, time, decision_at) = run_one(EpisodeClass::ComponentTimeout, seed, id);
        assert!(declared > 0);
        // The bill holds the multiplied amount, and the clock moved by it.
        assert_eq!(
            bill,
            declared * u64::from(factor),
            "seed {seed} component {id}"
        );
        assert_eq!(decision_at, Some(Instant(bill)), "seed {seed}");
        // The component declares no Time, so the Time bill is zero: the clock moved without it.
        assert_eq!(time, 0);

        // The same component on an episode without directives: the plain cost, and the plain
        // clock advance. The directive is what changed it.
        let (declared, bill, _, decision_at) = run_one(EpisodeClass::Ambiguous, seed, id);
        assert_eq!(bill, declared, "seed {seed} component {id}");
        assert_eq!(decision_at, Some(Instant(bill)));
        seen += 1;
        if seen == 6 {
            break;
        }
    }
    assert_eq!(
        seen, 6,
        "too few Slow directives on the real components in 300 seeds"
    );
}

#[test]
fn without_directives_the_same_components_cost_the_plain_sum() {
    let l = limits();
    let (mut components, runs) = fakes(4, 100, MS);
    let mut policy = ScriptedPolicy::new(vec![
        step(&[0, 1, 2, 3], None),
        step(&[], Some(Action::Abstain)),
    ]);
    let record = play(1, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    assert_eq!(record.bill.total(Resource::Compute), 400);
    assert_eq!(record.bill.total(Resource::Time), 4 * MS);
    assert_eq!(record.directives_ignored, 0);
    assert!(runs.iter().all(|r| r.get() == 1));
    assert_eq!(
        record
            .ledger
            .iter()
            .filter(|e| e.kind == EntryKind::ComputationResult)
            .count(),
        0
    );
}

#[test]
fn a_failed_component_gives_the_policy_no_output_and_the_others_do() {
    // A ComponentTimeout seed with a Fail directive on component 0 or 1.
    let l = limits();
    let failing = |seed: u64| -> Vec<u32> {
        let s = spec(seed, EpisodeClass::ComponentTimeout, &l);
        generate(&s)
            .harness_directives()
            .iter()
            .filter(|d| d.component < 2 && d.mode == ComponentMode::Fail)
            .map(|d| d.component)
            .collect()
    };
    let seed = (0..200u64)
        .find(|seed| !failing(*seed).is_empty())
        .expect("such a seed exists in 200");
    let failed = failing(seed);
    // The fakes return an entry, so "no output" is distinguishable from "empty output".
    let mut components: Vec<Box<dyn gordian_components::Component>> = Vec::new();
    for id in 0..2u32 {
        let (mut fake, _) = Fake::new(id, vec![Charge::new(Resource::Compute, 1)]);
        fake.entries = vec![(EntryKind::Hypothesis, b"{}".to_vec())];
        components.push(Box::new(fake));
    }
    let (mut spy, log) = Spy::new(ScriptedPolicy::new(vec![step(
        &[0, 1],
        Some(Action::Abstain),
    )]));
    play(
        seed,
        EpisodeClass::ComponentTimeout,
        &mut spy,
        &mut components,
        &l,
    )
    .unwrap();
    let outputs = log.borrow()[0].outputs.clone();
    let expected: Vec<ComponentId> = (0..2u32)
        .filter(|id| !failed.contains(id))
        .map(ComponentId)
        .collect();
    assert_eq!(outputs, expected);
}

// ---- stopping ----

#[test]
fn a_policy_that_never_decides_stops_at_the_step_cap_undecided() {
    let mut l = limits();
    l.max_steps = 7;
    let mut policy = ScriptedPolicy::never_decides();
    let record = play(3, EpisodeClass::CriticalFault, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::StepCap);
    assert_eq!(record.steps, 7);
    let v = &record.verdict;
    assert!(v.undecided && !v.success && !v.abstained);
    assert!(v.critical_miss, "undecided on a critical fault is a miss");
    assert_eq!(v.decision_at, None);
    assert!(record.trajectory.is_empty());
}

#[test]
fn an_idle_policy_runs_to_the_horizon_and_is_undecided() {
    // 10 s horizon, 50 ms steps: steps start at 0, 50, ..., 10,000 ms, which is 201 steps; the
    // clock then passes the horizon.
    let l = limits();
    let mut policy = ScriptedPolicy::never_decides();
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap();
    assert_eq!(record.stop, StopReason::Horizon);
    assert_eq!(record.steps, 201);
    assert!(record.verdict.undecided);
}

#[test]
fn nothing_is_recorded_after_the_close() {
    let record = play_script(
        3,
        EpisodeClass::Ambiguous,
        vec![Action::Abstain, Action::Abstain, Action::Abstain],
        &limits(),
    );
    assert_eq!(record.trajectory.len(), 1);
    assert_eq!(record.steps, 1);
    let last = record.ledger.iter().last().unwrap();
    // Outcome entry of the abstention is the last thing in the ledger.
    assert_eq!(last.kind, EntryKind::Outcome);
}

// ---- measurement at the boundary ----

#[test]
fn timings_are_measurements_from_the_timer_never_accounting_and_sum_to_the_measured_columns() {
    let l = limits();
    let (mut components, _) = fakes(2, 10, 0);
    let mut policy = ScriptedPolicy::new(vec![
        step(&[0, 1], None),
        step(&[1], None),
        step(&[], Some(Action::Abstain)),
    ]);
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    let timer: Vec<_> = record
        .ledger
        .iter()
        .filter(|e| e.provenance.producer == "harness/timer")
        .collect();
    assert!(!timer.is_empty());
    assert!(timer.iter().all(|e| e.kind == EntryKind::Measurement));
    let mut by_what = std::collections::BTreeMap::<String, u64>::new();
    for e in &timer {
        let v: serde_json::Value = serde_json::from_slice(&e.payload).unwrap();
        *by_what
            .entry(v["what"].as_str().unwrap().to_owned())
            .or_default() += v["ns"].as_u64().unwrap();
    }
    assert_eq!(by_what["component"], record.measured.component_ns);
    assert_eq!(
        by_what["select"] + by_what["decide"],
        record.measured.sched_ns
    );
    // Three components were run (0, 1, 1): three component timings.
    let component_timings = timer
        .iter()
        .filter(|e| {
            let v: serde_json::Value = serde_json::from_slice(&e.payload).unwrap();
            v["what"] == "component"
        })
        .count();
    assert_eq!(component_timings, 3);
    // And nothing from the timer is accounting, so replay is not disturbed.
    assert!(
        record
            .ledger
            .iter()
            .filter(|e| e.kind == EntryKind::Accounting)
            .all(|e| e.provenance.producer != "harness/timer")
    );
    // The harness's own share is what is left of the episode's wall time.
    assert!(record.measured.harness_ns > 0);
}

#[test]
fn measured_shares_add_up_to_no_more_than_the_wall_time_of_the_call() {
    // Five runs of a component that really takes 3 ms. The three shares partition the episode's
    // own wall time, so their sum cannot exceed the time the caller saw around the call, and the
    // component share is at least the sleeping.
    let l = limits();
    let mut components: Vec<Box<dyn gordian_components::Component>> = Vec::new();
    let (mut fake, _) = Fake::new(0, vec![Charge::new(Resource::Compute, 1)]);
    fake.sleep = std::time::Duration::from_millis(3);
    components.push(Box::new(fake));
    // The policy's decide takes 3 ms too, five times over.
    let (mut policy, _) = Spy::new(ScriptedPolicy::new(vec![
        step(&[0], None),
        step(&[0], None),
        step(&[0], None),
        step(&[0], None),
        step(&[0], Some(Action::Abstain)),
    ]));
    policy.sleep = std::time::Duration::from_millis(3);
    let outer = std::time::Instant::now();
    let record = play(3, EpisodeClass::Ambiguous, &mut policy, &mut components, &l).unwrap();
    let outer_ns = outer.elapsed().as_nanos() as u64;
    let m = record.measured;
    assert!(m.component_ns >= 15_000_000, "{m:?}");
    assert!(m.sched_ns >= 15_000_000, "{m:?}");
    assert!(
        m.component_ns + m.sched_ns + m.harness_ns <= outer_ns,
        "{m:?} vs {outer_ns}"
    );
}

#[test]
fn the_ledger_without_its_timings_is_identical_across_runs() {
    // Protocol replay at the level of entries: same decisions, same charges, same payloads. The
    // timer entries are the only nondeterministic ones, and they sit at fixed positions.
    let run = || {
        let l = limits();
        let (mut components, _) = fakes(3, 50, MS);
        let mut policy = ScriptedPolicy::new(vec![
            step(&[0, 1, 2], None),
            step(
                &[1],
                Some(Action::Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(0),
                }),
            ),
            step(&[2], Some(Action::Abstain)),
        ]);
        play(
            11,
            EpisodeClass::NoiseFlood,
            &mut policy,
            &mut components,
            &l,
        )
        .unwrap()
    };
    let strip = |record: &gordian_run::EpisodeRecord| -> Vec<_> {
        record
            .ledger
            .iter()
            .map(|e| {
                let payload = if e.provenance.producer == "harness/timer" {
                    Vec::new()
                } else {
                    e.payload.clone()
                };
                (e.id, e.at, e.kind, e.provenance.clone(), payload)
            })
            .collect()
    };
    assert_eq!(strip(&run()), strip(&run()));
}

// ---- working state ----

#[test]
fn the_policy_is_shown_observations_in_stream_order_and_a_bounded_window() {
    let mut l = limits();
    l.window = 5;
    let (mut spy, log) = Spy::new(ScriptedPolicy::never_decides());
    l.max_steps = 120;
    play(3, EpisodeClass::NoiseFlood, &mut spy, &mut [], &l).unwrap();
    let log = log.borrow();
    assert_eq!(log.len(), 120);
    assert!(log.iter().all(|s| s.evidence.len() <= 5));
    assert!(
        log.last().unwrap().evidence.len() == 5,
        "a noise flood fills a window of 5"
    );
    for seen in log.iter() {
        let instants: Vec<u64> = seen.evidence.iter().map(|(t, _)| t.0).collect();
        assert!(instants.windows(2).all(|w| w[0] <= w[1]));
        assert!(instants.iter().all(|t| *t <= seen.now.0));
    }
}

#[test]
fn the_harness_keeps_scored_hypotheses_and_pending_requests_in_the_working_state() {
    use gordian_components::{ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID};
    let l = limits();
    // 100 steps of the heuristic, one of the estimator, one of the verifier, one idle.
    let mut script: Vec<_> = (0..100).map(|_| step(&[HEURISTIC_ID.0], None)).collect();
    script.push(step(&[ESTIMATOR_ID.0], None));
    script.push(step(&[VERIFIER_ID.0], None));
    script.push(step(&[], None));
    let (mut spy, log) = Spy::new(ScriptedPolicy::new(script));
    let mut components = gordian_run::standard_components();
    play(3, EpisodeClass::NoiseFlood, &mut spy, &mut components, &l).unwrap();
    let log = log.borrow();
    // Nothing is scored until the estimator has run, and then at most eight hypotheses are kept.
    assert_eq!(log[100].hypotheses, 0);
    assert!(
        (1..=8).contains(&log[101].hypotheses),
        "{}",
        log[101].hypotheses
    );
    // The heuristic asked for the verifier at some step; the request stays pending until the
    // verifier has run, and is gone the step after.
    assert!(log[..100].iter().any(|s| s.pending.contains(&VERIFIER_ID)));
    assert!(log[101].pending.contains(&VERIFIER_ID));
    assert!(!log[102].pending.contains(&VERIFIER_ID));
}

#[test]
fn the_policy_can_read_what_remains_of_the_probe_budget_from_the_bill() {
    let l = limits();
    let (mut spy, log) = Spy::new(ScriptedPolicy::actions(vec![
        Action::Probe {
            kind: ProbeKind::LatencySample,
            target: ServiceId(0),
        },
        Action::Abstain,
        Action::Abstain,
    ]));
    play(3, EpisodeClass::Ambiguous, &mut spy, &mut [], &l).unwrap();
    let log = log.borrow();
    assert_eq!(log[0].probes_remaining, Some(12));
    assert_eq!(log[1].probes_remaining, Some(10));
}

// ---- how the harness may be called ----

#[test]
fn a_spec_whose_world_budget_differs_from_the_limits_is_refused() {
    let l = limits();
    let mut s = spec(1, EpisodeClass::Ambiguous, &l);
    s.budget.probes += 1;
    let mut policy = ScriptedPolicy::never_decides();
    let err = gordian_run::run_episode(&s, &mut policy, &mut [], &l).unwrap_err();
    assert!(matches!(err, HarnessError::BudgetMismatch { .. }), "{err}");
}

#[test]
fn selecting_a_component_the_run_does_not_have_is_an_error() {
    let l = limits();
    let mut policy = ScriptedPolicy::new(vec![step(&[9], None)]);
    let err = play(1, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap_err();
    assert_eq!(err, HarnessError::UnknownComponent(ComponentId(9)));
}

#[test]
fn two_components_with_one_id_are_refused() {
    let l = limits();
    let (mut a, _) = fakes(1, 1, 0);
    let (mut b, _) = fakes(1, 1, 0);
    a.append(&mut b);
    let mut policy = ScriptedPolicy::never_decides();
    let err = play(1, EpisodeClass::Ambiguous, &mut policy, &mut a, &l).unwrap_err();
    assert_eq!(err, HarnessError::DuplicateComponent(ComponentId(0)));
}

#[test]
fn limits_that_stop_time_from_advancing_are_refused() {
    let mut l = limits();
    l.step_ns = 0;
    let mut policy = ScriptedPolicy::never_decides();
    let err = play(1, EpisodeClass::Ambiguous, &mut policy, &mut [], &l).unwrap_err();
    assert!(matches!(err, HarnessError::InvalidLimits(_)));
    let l = Limits {
        max_steps: 0,
        ..Limits::default()
    };
    assert!(l.validate().is_err());
}

#[test]
fn only_the_harness_generates_episodes_or_builds_truth() {
    // Structural check on this crate's own source: the episode is generated, and the truth
    // built from it, in harness.rs and nowhere else.
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut offenders = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("harness.rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("generate(") || text.contains("from_episode") {
                    offenders.push(path);
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}
