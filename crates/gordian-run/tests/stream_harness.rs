//! The stream loop: accounting, the reasoner's hard limit, the ledger, the trajectory, the final
//! call, the step cap and the structural claims about where the truth goes.

mod stream_common;

use gordian_core::{Bill, EntryKind, Instant, Resource, decode_accounting};
use gordian_run::policy::PolicyId;
use gordian_run::stream::arms::rung::RungConfig;
use gordian_run::stream::arms::{
    ArmReport, ArmRole, Proposed, Source, StepInput, StreamArm, StreamPolicy, never,
};
use gordian_run::stream::manifest::{Exchange, StreamLimits};
use gordian_run::stream::meter::Meter;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_run::stream::{SegmentRecord, StreamHarnessError, run_segment};
use gordian_stream::{
    ObsId, ObsRef, Question, StreamAction, StreamOutcome, StreamParams, StreamPublic,
};
use std::collections::BTreeSet;
use stream_common::*;

/// An arm that does exactly what a closure says, for testing the harness. It never notices or
/// concludes anything; it is a test instrument, as `policy::scripted` is for the episode harness.
struct Scripted<F: FnMut(&StepInput<'_>, &mut Vec<ObsId>) -> Vec<Proposed>> {
    step: F,
    seen: Vec<ObsId>,
    applied: Vec<Vec<(u64, bool)>>,
}

impl<F: FnMut(&StepInput<'_>, &mut Vec<ObsId>) -> Vec<Proposed>> StreamPolicy for Scripted<F> {
    fn id(&self) -> PolicyId {
        PolicyId::new("scripted")
    }

    fn role(&self) -> ArmRole {
        ArmRole::Comparison
    }

    fn declared_step_cost(&self, _input: &StepInput<'_>) -> Vec<gordian_core::Charge> {
        vec![gordian_core::Charge::new(Resource::Compute, 0)]
    }

    fn step(
        &mut self,
        input: &StepInput<'_>,
        _work_allowed: bool,
        _meter: &mut Meter<'_>,
    ) -> Vec<Proposed> {
        self.applied
            .push(input.applied.iter().map(|a| (a.tag, a.accepted)).collect());
        for event in input.events {
            if let gordian_stream::StreamEvent::Observed { id, .. } = event {
                self.seen.push(*id);
            }
        }
        (self.step)(input, &mut self.seen)
    }

    fn finish(&mut self, _input: &StepInput<'_>, _meter: &mut Meter<'_>) -> Vec<Proposed> {
        Vec::new()
    }

    fn report(&self) -> ArmReport {
        ArmReport::default()
    }
}

fn scripted<F: FnMut(&StepInput<'_>, &mut Vec<ObsId>) -> Vec<Proposed> + 'static>(
    f: F,
) -> Box<dyn StreamPolicy> {
    Box::new(Scripted {
        step: f,
        seen: Vec::new(),
        applied: Vec::new(),
    })
}

fn escalate(tag: u64, refs: &[ObsId], focus: ObsId) -> Proposed {
    Proposed {
        tag,
        action: StreamAction::Escalate {
            context: refs.iter().map(|o| ObsRef::Passive(*o)).collect(),
            question: Question::Diagnose { focus },
        },
        source: Source::Escalation,
    }
}

/// Parameters and limits whose reasoner budget is `tokens`.
fn with_tokens(seed: u64, duration_s: u64, tokens: u64) -> (StreamParams, StreamLimits) {
    let mut p = params(seed, duration_s);
    let mut l = limits(&p);
    l.reasoner_tokens = tokens;
    p.budget = l.world_budget(&p.reasoner.cost);
    (p, l)
}

fn play_with(
    p: &StreamParams,
    l: &StreamLimits,
    make: impl Fn(&StreamPublic) -> Box<dyn StreamPolicy>,
) -> Result<SegmentRecord, StreamHarnessError> {
    run_segment(p, &make, l, &Exchange::default())
}

/// An arm that asks the reasoner about its first `n` observations at the first step that has
/// them, and again `gap` steps later.
fn asks_twice(n: usize, gap: usize) -> Box<dyn StreamPolicy> {
    let mut step = 0usize;
    let mut first: Option<usize> = None;
    scripted(move |_, seen| {
        step += 1;
        if first.is_none() && seen.len() >= n + 3 {
            first = Some(step);
            return vec![escalate(1, &seen[..n], seen[0])];
        }
        if first.is_some_and(|f| step == f + gap) {
            return vec![escalate(2, &seen[1..=n], seen[0])];
        }
        Vec::new()
    })
}

/// An accounting entry: its id, whether it was accepted, its charges.
type Accounted = (usize, bool, Vec<(Resource, u64)>);

fn accounting(record: &SegmentRecord) -> Vec<Accounted> {
    record
        .ledger
        .iter()
        .filter(|e| e.kind == EntryKind::Accounting)
        .map(|e| {
            let a = decode_accounting(&e.payload).unwrap();
            (
                e.id.0 as usize,
                a.accepted,
                a.charges.iter().map(|c| (c.resource, c.amount)).collect(),
            )
        })
        .collect()
}

#[test]
fn a_reasoner_call_is_paid_for_before_it_is_made() {
    let (p, l) = with_tokens(3, 150, 460);
    let record = play_with(&p, &l, |_| asks_twice(3, 10)).unwrap();
    // The first call fits exactly (400 + 3 * 20 tokens); the second does not fit in what is left.
    assert_eq!(record.verdict.totals.reasoner.calls, 1);
    assert_eq!(record.verdict.totals.reasoner.tokens, 460);
    assert_eq!(record.counts.escalations_refused, 1);
    assert_eq!(record.bill.total(Resource::Communication), 460);

    // In the ledger, the accepted charge comes before the stream's outcome for the call, which
    // comes before the answer that call produced.
    let paid: Vec<usize> = accounting(&record)
        .into_iter()
        .filter(|(_, accepted, charges)| {
            *accepted && charges.iter().any(|(r, _)| *r == Resource::Communication)
        })
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(paid.len(), 1, "one reasoner charge was accepted");
    let outcome = record
        .ledger
        .iter()
        .find(|e| {
            e.kind == EntryKind::Outcome
                && String::from_utf8_lossy(&e.payload).contains("Escalated")
        })
        .expect("the call's outcome is recorded");
    let answer = record
        .ledger
        .iter()
        .find(|e| e.kind == EntryKind::Hypothesis && e.provenance.producer == "harness/reasoner")
        .expect("the answer arrives before the stream ends");
    assert!(paid[0] < outcome.id.0 as usize, "paid before the call");
    assert!(outcome.id.0 < answer.id.0, "the answer follows the call");
    // The answer is a hypothesis and not a measurement.
    assert_eq!(answer.kind, EntryKind::Hypothesis);
    assert!(
        record
            .ledger
            .iter()
            .filter(|e| e.provenance.producer == "harness/sensor")
            .all(|e| e.kind == EntryKind::Measurement)
    );
}

#[test]
fn a_call_that_does_not_fit_is_refused_and_nothing_is_charged() {
    let (p, l) = with_tokens(3, 150, 460);
    let record = play_with(&p, &l, |_| asks_twice(3, 10)).unwrap();
    // The refused call left a refused accounting entry, and no accepted one: nothing moved.
    let refused: Vec<_> = accounting(&record)
        .into_iter()
        .filter(|(_, accepted, charges)| {
            !*accepted && charges.iter().any(|(r, _)| *r == Resource::Communication)
        })
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].2, vec![(Resource::Communication, 460)]);
    // The stream was never asked: exactly one escalation is in the trajectory, and the stream's
    // own reasoner spend is that one call's declared price.
    let asked = record
        .trajectory
        .iter()
        .filter(|s| matches!(s.action, StreamAction::Escalate { .. }))
        .count();
    assert_eq!(asked, 1);
    assert_eq!(record.verdict.totals.reasoner.modelled_ns, 460 * 250_000);
    // Bill and ledger agree.
    let replayed = Bill::replay(l.budget(), &record.ledger).unwrap();
    for r in [
        Resource::Compute,
        Resource::Probes,
        Resource::Time,
        Resource::Communication,
    ] {
        assert_eq!(replayed.total(r), record.bill.total(r), "{r:?}");
    }
}

#[test]
fn a_first_call_over_the_limit_is_refused_with_nothing_charged() {
    let (p, l) = with_tokens(3, 150, 100);
    let record = play_with(&p, &l, |_| asks_twice(3, 10)).unwrap();
    assert_eq!(record.verdict.totals.reasoner.calls, 0);
    assert_eq!(record.counts.escalations_refused, 2);
    assert_eq!(record.bill.total(Resource::Communication), 0);
    assert_eq!(record.verdict.totals.reasoner.modelled_ns, 0);
    assert!(
        record
            .ledger
            .iter()
            .all(|e| e.provenance.producer != "harness/reasoner" || e.kind != EntryKind::Hypothesis),
        "no answer exists"
    );
}

#[test]
fn a_malformed_call_is_refused_without_charge() {
    let (p, l) = with_tokens(3, 150, 80_000);
    let mut step = 0;
    let record = play_with(&p, &l, move |_| {
        scripted(move |_, seen| {
            step += 1;
            if seen.len() >= 6 && step % 7 == 0 && step < 40 {
                let beyond = ObsId(seen.last().unwrap().0 + 1000);
                return vec![
                    // a duplicate reference
                    escalate(1, &[seen[0], seen[0]], seen[0]),
                    // a reference to an observation that has not been delivered
                    escalate(2, &[beyond], seen[0]),
                    // a focus that has not been delivered
                    escalate(3, &[seen[0]], beyond),
                ];
            }
            Vec::new()
        })
    })
    .unwrap();
    assert!(record.counts.escalations_refused >= 3);
    assert_eq!(record.verdict.totals.reasoner.calls, 0);
    assert_eq!(record.bill.total(Resource::Communication), 0);
    // Refused for the reason, not by the bill.
    assert!(record.ledger.iter().any(|e| e.kind == EntryKind::Outcome
        && String::from_utf8_lossy(&e.payload).contains("duplicate reference")));
}

#[test]
fn the_reasoner_limit_holds_for_an_arm_that_asks_about_everything() {
    let (p, l) = with_tokens(5, 200, 3_000);
    let record = play(&p, &StreamPolicySpec::Always, &l).unwrap();
    assert!(record.bill.total(Resource::Communication) <= 3_000);
    assert!(record.verdict.totals.reasoner.tokens <= 3_000);
    assert!(record.verdict.totals.reasoner.modelled_ns <= 3_000 * 250_000);
    assert!(
        record.counts.escalations_refused > 0,
        "the limit bound: {:?}",
        record.counts
    );
    // Refused calls are in the ledger as refused charges, not as spend.
    let accepted: u64 = accounting(&record)
        .into_iter()
        .filter(|(_, ok, _)| *ok)
        .flat_map(|(_, _, charges)| charges)
        .filter(|(r, _)| *r == Resource::Communication)
        .map(|(_, a)| a)
        .sum();
    assert_eq!(accepted, record.verdict.totals.reasoner.tokens);
}

#[test]
fn the_reasoners_cost_is_in_its_own_units_and_through_the_exchange_rate() {
    let (p, l) = with_tokens(5, 200, 80_000);
    let spec = StreamPolicySpec::Always;
    let rung = RungConfig::default();
    let run = |rate: u64| {
        run_segment(
            &p,
            &|public: &StreamPublic| {
                gordian_run::stream::spec::build_public(&spec, &rung, public, "always_escalate", 5)
                    .unwrap()
            },
            &l,
            &Exchange {
                reasoner_ns_per_token: rate,
            },
        )
        .unwrap()
    };
    let (a, b) = (run(250_000), run(1_000));
    assert!(a.verdict.totals.reasoner.calls > 0);
    // Own units: calls, tokens and declared latency do not depend on the exchange rate.
    assert_eq!(
        a.verdict.totals.reasoner.calls,
        b.verdict.totals.reasoner.calls
    );
    assert_eq!(
        a.verdict.totals.reasoner.tokens,
        b.verdict.totals.reasoner.tokens
    );
    assert_eq!(
        a.trajectory_counts.reasoner_latency_ns,
        b.trajectory_counts.reasoner_latency_ns
    );
    assert_eq!(
        a.verdict.totals.reasoner.modelled_ns,
        b.verdict.totals.reasoner.modelled_ns
    );
    // Converted: tokens times the rate, and total cost is substrate plus that.
    assert_eq!(
        a.reasoner_cost_ns,
        a.verdict.totals.reasoner.tokens * 250_000
    );
    assert_eq!(b.reasoner_cost_ns, b.verdict.totals.reasoner.tokens * 1_000);
    assert_eq!(a.total_cost_ns(), a.substrate_ns() + a.reasoner_cost_ns);
    assert_eq!(a.substrate_ns(), b.substrate_ns());
    assert!(a.substrate_ns() > 0);
    // The reasoner dwarfs the substrate at the world's price.
    assert!(a.reasoner_cost_ns > 100 * a.substrate_ns());
}

#[test]
fn a_segment_is_a_whole_stream_and_ends_at_the_horizon() {
    let p = params(2, 150);
    let l = limits(&p);
    let record = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    assert_eq!(record.steps as u64, 150_000_000_000 / l.step_ns);
    assert_eq!(record.stop.as_str(), "horizon");
    assert_eq!(record.observations as usize, record.public_stream.len());
    assert_eq!(record.public.duration_ns, 150_000_000_000);
    // Every declaration was made at or before the horizon.
    for step in &record.trajectory {
        assert!(step.at <= Instant(150_000_000_000));
    }
}

#[test]
fn a_stream_whose_budget_differs_from_the_limits_is_refused() {
    let p = params(2, 150);
    let mut l = limits(&p);
    l.probes += 1;
    let err = play(&p, &StreamPolicySpec::Never, &l).unwrap_err();
    assert!(
        matches!(err, StreamHarnessError::BudgetMismatch(_)),
        "{err}"
    );
    let mut l = limits(&p);
    l.reasoner_tokens += 1;
    assert!(matches!(
        play(&p, &StreamPolicySpec::Never, &l).unwrap_err(),
        StreamHarnessError::BudgetMismatch(_)
    ));
    let mut l = limits(&p);
    l.step_ns = 0;
    assert!(matches!(
        play(&p, &StreamPolicySpec::Never, &l).unwrap_err(),
        StreamHarnessError::InvalidLimits(_)
    ));
}

#[test]
fn the_step_cap_stops_a_runaway_with_no_final_call() {
    let p = params(2, 150);
    let mut l = limits(&p);
    l.max_steps = 7;
    let record = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    assert_eq!(record.steps, 7);
    assert_eq!(record.stop.as_str(), "step_cap");
    // No final call: nothing was declared at the horizon.
    assert!(
        record
            .trajectory
            .iter()
            .all(|s| s.at < Instant(150_000_000_000))
    );
}

#[test]
fn the_final_call_declares_what_an_arm_has_when_it_would_otherwise_say_nothing() {
    let p = params(4, 150);
    let l = limits(&p);
    let spec = StreamPolicySpec::Never;
    let first_declaration = |patience_ns: u64| {
        let rung = RungConfig {
            patience_ns,
            ..RungConfig::default()
        };
        let record = run_segment(
            &p,
            &|public: &StreamPublic| {
                gordian_run::stream::spec::build_public(&spec, &rung, public, "never_escalate", 4)
                    .unwrap()
            },
            &l,
            &Exchange::default(),
        )
        .unwrap();
        let first = record
            .trajectory
            .iter()
            .find(|s| matches!(s.action, StreamAction::Declare { .. }))
            .map(|s| s.at);
        (record.trajectory_counts.declarations, first)
    };
    let (patient_n, patient_first) = first_declaration(10_000_000_000_000);
    let (default_n, default_first) = first_declaration(RungConfig::default().patience_ns);
    // The rule's patience never passes, yet the arm still declares what it has: when an anomaly
    // goes quiet, or at the stream's end, it gets the rule's final call (which declares as the
    // rule does at its deadline). It is never earlier than the arm that waits three seconds.
    assert!(patient_n > 0, "the final call declared nothing");
    assert!(patient_first.unwrap() >= default_first.unwrap());
    assert!(default_n >= patient_n / 2);
}

#[test]
fn a_binding_compute_limit_stops_the_arm_working_and_the_bill_holds() {
    let p = params(6, 150);
    let mut l = limits(&p);
    l.compute = 5_000;
    let record = play(&p, &StreamPolicySpec::Always, &l).unwrap();
    assert!(record.bill.total(Resource::Compute) <= 5_000);
    assert!(
        record.components_run + record.components_skipped + record.rule_skipped > 0
            || record.bill.total(Resource::Compute) > 0
    );
    // With the limit this tight the arm cannot do the work that notices and concludes.
    let free = play(&p, &StreamPolicySpec::Always, &limits(&p)).unwrap();
    assert!(free.components_run > record.components_run);
    assert!(free.trajectory_counts.declarations >= record.trajectory_counts.declarations);
}

#[test]
fn the_arm_is_built_from_public_information_and_nothing_else() {
    // `run_segment` hands the arm builder a `StreamPublic` and nothing else: the builder's type
    // says so. The arm cannot be built with the truth through this entry point.
    let p = params(2, 150);
    let l = limits(&p);
    let seen = std::cell::RefCell::new(None);
    let record = play_with(&p, &l, |public| {
        *seen.borrow_mut() = Some(public.clone());
        Box::new(StreamArm::with(never::Never, public, RungConfig::default()))
    })
    .unwrap();
    assert_eq!(seen.borrow().as_ref().unwrap(), &record.public);
    assert_eq!(record.public, public_of(&p));
}

#[test]
fn the_trajectory_holds_every_action_the_stream_answered_in_order() {
    let p = params(8, 200);
    let l = limits(&p);
    for spec in [StreamPolicySpec::Never, StreamPolicySpec::Always] {
        let record = play(&p, &spec, &l).unwrap();
        let mut last = Instant::ZERO;
        let (mut probes, mut calls, mut decls) = (0u32, 0u32, 0u32);
        let mut contexts = BTreeSet::new();
        for step in &record.trajectory {
            assert!(step.at >= last);
            last = step.at;
            match (&step.action, &step.outcome) {
                (StreamAction::Probe { .. }, StreamOutcome::Probed { .. }) => probes += 1,
                (StreamAction::Escalate { context, .. }, StreamOutcome::Escalated { .. }) => {
                    calls += 1;
                    contexts.insert(context.len());
                }
                (StreamAction::Declare { .. }, StreamOutcome::Declared { .. }) => decls += 1,
                other => panic!("an unexpected step {other:?}"),
            }
        }
        assert_eq!(probes, record.trajectory_counts.probes_used);
        assert_eq!(u64::from(calls), record.verdict.totals.reasoner.calls);
        assert_eq!(decls, record.trajectory_counts.declarations);
        assert_eq!(
            record.counts.cheap_declarations + record.counts.reasoner_declarations,
            decls
        );
        // Probes are in the bill as probe units and in the trajectory as probes.
        assert!(record.bill.total(Resource::Probes) >= u64::from(probes));
    }
}

// ---- Measured timings and counted operations

#[test]
fn timings_are_measurements_from_the_timer_and_sum_to_what_measured_csv_holds() {
    let p = params(5, 200);
    let record = play(&p, &StreamPolicySpec::Always, &limits(&p)).unwrap();
    let (mut component_ns, mut sched_ns) = (0u64, 0u64);
    let mut component_ops: std::collections::BTreeMap<u64, Vec<u64>> = Default::default();
    let mut rule_ops: Vec<u64> = Vec::new();
    for entry in record
        .ledger
        .iter()
        .filter(|e| e.provenance.producer == "harness/timer")
    {
        // A timing is a measurement, never accounting: `Bill::replay` would reject it.
        assert_eq!(entry.kind, EntryKind::Measurement);
        let v: serde_json::Value = serde_json::from_slice(&entry.payload).unwrap();
        let ns = v["ns"].as_u64().unwrap();
        let ops: Vec<u64> = v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap())
            .collect();
        match v["what"].as_str().unwrap() {
            "component" => {
                component_ns += ns;
                let id = v["component"].as_u64().unwrap();
                let sum = component_ops
                    .entry(id)
                    .or_insert_with(|| vec![0; ops.len()]);
                for (s, o) in sum.iter_mut().zip(&ops) {
                    *s += o;
                }
            }
            "decide" => {
                sched_ns += ns;
                if rule_ops.is_empty() {
                    rule_ops = vec![0; ops.len()];
                }
                for (s, o) in rule_ops.iter_mut().zip(&ops) {
                    *s += o;
                }
            }
            "bookkeeping" => sched_ns += ns,
            other => panic!("{other}"),
        }
    }
    assert!(component_ns > 0 && sched_ns > 0);
    assert_eq!(component_ns, record.measured.component_ns);
    assert_eq!(sched_ns, record.measured.sched_ns);
    // The counted operations in the ledger are the ones the record sums, per component and for
    // the rule, and they are deterministic (a second run counts the same).
    for ops in &record.ops.components {
        let from_ledger = component_ops.get(&u64::from(ops.component().0));
        match from_ledger {
            Some(counts) => assert_eq!(counts.as_slice(), ops.counts()),
            None => assert_eq!(ops.total(), 0),
        }
    }
    assert_eq!(rule_ops.as_slice(), record.ops.sched.counts());
    let again = play(&p, &StreamPolicySpec::Always, &limits(&p)).unwrap();
    assert_eq!(again.ops.ops_component(), record.ops.ops_component());
    assert_eq!(again.ops.ops_sched(), record.ops.ops_sched());
    assert_eq!(
        again.ops.modelled_component_ns(),
        record.ops.modelled_component_ns()
    );
    assert!(record.ops.ops_component() > 0 && record.ops.ops_sched() > 0);
}
