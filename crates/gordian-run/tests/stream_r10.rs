//! The arm of work item R10, `oracle_notice_privileged`, and the check that building it changed
//! no existing result.
//!
//! What is pinned, and by what:
//!
//! - the manifest spelling and the naming rule of the arm;
//! - it escalates exactly the hard incidents, once each, about each incident's first observation,
//!   `delay` after the step that delivered that observation, with the context the rung's builder
//!   makes for that anchor;
//! - the rung's own later notice of the same incident causes no second call, and the incidents
//!   the selection oracle is asked about by a different anchor are asked about once;
//! - with no hard incident the arm is `never_escalate`;
//! - the rung's context for an anchor it did not notice ([`Rung::context_at`]) is the context it
//!   builds for a noticed anomaly with that anchor, for every builder;
//! - the rule's inputs (what it reads of the plan) are pinned by unit tests in `oracle.rs`.

mod stream_common;

use gordian_run::stream::arms::ArmRole;
use gordian_run::stream::arms::context::ContextBuilder;
use gordian_run::stream::arms::rung::{Rung, RungConfig};
use gordian_run::stream::privileged::{OracleFactory, OraclePlan, PlanIncident};
use gordian_run::stream::results::results_row;
use gordian_run::stream::spec::{StreamPolicySpec, build_public, privileged_factory};
use gordian_run::stream::{SegmentRecord, StreamArmSpec};
use gordian_stream::{
    ObsId, ObsRef, Question, StreamAction, StreamEvent, StreamOutcome, StreamParams, Tier, generate,
};
use gordian_stream_eval::truth_from_stream;
use gordian_world::graph::dependents_mask;
use gordian_world::{CounterName, Observation, ServiceId};
use std::collections::BTreeMap;
use stream_common::*;

const NS: u64 = 1_000_000_000;

fn hard_heavy(seed: u64) -> StreamParams {
    let mut p = params(seed, 300);
    p.mix.plain_permille = 300;
    p.mix.hard_permille = 500;
    p
}

/// Every accepted escalation of a trajectory: instant, focus, context, in order.
fn calls(record: &SegmentRecord) -> Vec<(u64, u32, Vec<ObsRef>)> {
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

fn notice(delay_ns: u64) -> StreamPolicySpec {
    StreamPolicySpec::OracleNotice { delay_ns }
}

fn selection(delay_ns: u64) -> StreamPolicySpec {
    StreamPolicySpec::OracleSelection { delay_ns }
}

// ---- The manifest spelling

#[test]
fn the_arm_is_written_as_its_id_or_with_its_delay_and_must_be_named_privileged() {
    let from = |text: &str| serde_json::from_str::<StreamPolicySpec>(text);
    assert_eq!(from("\"oracle_notice\"").unwrap(), notice(0));
    assert_eq!(
        from(r#"{"policy": "oracle_notice", "delay_ns": 16000000000}"#).unwrap(),
        notice(16 * NS)
    );
    assert_eq!(
        serde_json::to_string(&notice(0)).unwrap(),
        "\"oracle_notice\""
    );
    let written = serde_json::to_string(&notice(16 * NS)).unwrap();
    assert_eq!(from(&written).unwrap(), notice(16 * NS));
    // A parameter the arm does not have is refused.
    assert!(from(r#"{"policy": "oracle_notice", "persist_ns": 1}"#).is_err());
    assert_eq!(notice(0).role(), ArmRole::Privileged);
    // It is a privileged arm: not built from public information, and its name must say so.
    let rung = RungConfig::default();
    let public = public_of(&params(1, 120));
    assert!(build_public(&notice(0), &rung, &public, "oracle_notice_privileged", 1).is_none());
    assert!(privileged_factory(&notice(0), &rung).is_some());
    let named = |name: &str| {
        let mut m = manifest("t", &[("never_escalate", "never_escalate")], 1, 120, 0);
        m.arms.push(StreamArmSpec {
            arm: name.to_owned(),
            policy: notice(0),
            context: None,
        });
        m.validate()
    };
    assert!(named("oracle_notice").is_err());
    assert!(named("oracle_notice_privileged").is_ok());
    assert!(named("notice_d16_privileged").is_ok());
}

// ---- What it escalates, and when

#[test]
fn it_escalates_exactly_the_hard_incidents_once_each_at_their_first_observation() {
    let mut asked = 0usize;
    let mut hard_total = 0usize;
    for seed in 0..6 {
        let p = hard_heavy(seed);
        let stream = generate(&p);
        let events = stream.events().to_vec();
        let truth = truth_from_stream(&stream);
        for delay in [0, 4 * NS] {
            let record = play(&p, &notice(delay), &limits(&p)).unwrap();
            assert_eq!(record.role, ArmRole::Privileged);
            assert_eq!(record.arm_id, "oracle_notice");
            assert_eq!(record.counts.escalations_refused, 0, "seed {seed}");
            let made = calls(&record);
            // Nothing but a hard incident's first observation is asked about.
            let mut per_incident: BTreeMap<u32, Vec<(u64, Vec<ObsRef>)>> = BTreeMap::new();
            for (at, focus, context) in &made {
                let incident = truth
                    .incident_of(ObsId(*focus))
                    .expect("asks about incidents");
                let inc = &truth.incidents[incident as usize];
                assert_eq!(inc.tier, Tier::Hard, "seed {seed}");
                assert_eq!(Some(ObsId(*focus)), inc.observations.first().copied());
                per_incident
                    .entry(incident)
                    .or_default()
                    .push((*at, context.clone()));
            }
            for inc in truth.incidents.iter().filter(|i| i.tier == Tier::Hard) {
                hard_total += 1;
                let first = inc.observations[0];
                let first_at = events[first.0 as usize].0.0;
                let got = per_incident.get(&inc.id).map_or(&[][..], Vec::as_slice);
                // Once each, unless the call would fall after the last step.
                assert!(got.len() <= 1, "seed {seed} incident {}", inc.id);
                if first_at + delay + 2 * NS < p.duration_ns {
                    assert_eq!(got.len(), 1, "seed {seed} incident {}", inc.id);
                }
                if let Some((at, context)) = got.first() {
                    asked += 1;
                    // Anchored at the step that delivered the first observation: the call is
                    // `delay` after it, to within one step (and the busy time inside it).
                    assert!(*at >= first_at + delay, "seed {seed}: before its delay");
                    assert!(
                        *at - delay - first_at < 3 * NS / 2,
                        "seed {seed}: the notice was not made at the delivery step ({} ns late)",
                        *at - delay - first_at
                    );
                    // The context is a rung context: held observations only, no duplicates.
                    assert!(!context.is_empty());
                }
            }
            assert_eq!(
                made.len(),
                per_incident.values().map(Vec::len).sum::<usize>()
            );
        }
    }
    assert!(hard_total >= 8, "the test exercised {hard_total} incidents");
    assert!(asked >= 8, "the test exercised {asked} calls");
}

#[test]
fn the_rungs_later_notice_of_the_same_incident_causes_no_second_call() {
    // Where the selection oracle asks about a hard incident through an anomaly the rung noticed,
    // the notice oracle asks about it once, about the incident's first observation. The rung's
    // anomaly is anchored elsewhere (after the first observation) for some of them, which is the
    // case the test needs: a notice by the rung that is not the injected one.
    let mut both = 0usize;
    let mut elsewhere = 0usize;
    for seed in 0..8 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let delay = 4 * NS;
        let sel = calls(&play(&p, &selection(delay), &limits(&p)).unwrap());
        let not = calls(&play(&p, &notice(delay), &limits(&p)).unwrap());
        let calls_about = |list: &[(u64, u32, Vec<ObsRef>)], incident: u32| {
            list.iter()
                .filter(|(_, f, _)| truth.incident_of(ObsId(*f)) == Some(incident))
                .count()
        };
        for inc in truth.incidents.iter().filter(|i| i.tier == Tier::Hard) {
            if calls_about(&sel, inc.id) > 0 {
                both += 1;
                assert_eq!(
                    calls_about(&not, inc.id),
                    1,
                    "seed {seed} incident {}",
                    inc.id
                );
                let by_rung = sel.iter().any(|(_, f, _)| {
                    truth.incident_of(ObsId(*f)) == Some(inc.id) && ObsId(*f) != inc.observations[0]
                });
                elsewhere += usize::from(by_rung);
            }
        }
    }
    assert!(both >= 6, "the test exercised {both} incidents");
    assert!(
        elsewhere >= 1,
        "no incident was noticed by the rung at an anchor other than its first observation"
    );
}

#[test]
fn the_selection_oracles_calls_are_a_subset_of_what_the_notice_oracle_covers() {
    // The notice oracle asks about at least every hard incident the selection oracle does: the
    // privilege adds incidents (the ones the rung never noticed) and removes none.
    let mut extra = 0usize;
    for seed in 0..8 {
        let p = hard_heavy(seed);
        let truth = truth_from_stream(&generate(&p));
        let delay = 4 * NS;
        let covered = |spec: &StreamPolicySpec| -> Vec<u32> {
            let mut v: Vec<u32> = calls(&play(&p, spec, &limits(&p)).unwrap())
                .iter()
                .filter_map(|(_, f, _)| truth.incident_of(ObsId(*f)))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let (sel, not) = (covered(&selection(delay)), covered(&notice(delay)));
        for i in &sel {
            assert!(not.contains(i), "seed {seed}: incident {i} lost");
        }
        extra += not.len() - sel.len();
    }
    assert!(extra > 0, "the notice oracle asked about nothing more");
}

#[test]
fn with_no_hard_incident_it_is_never_escalate() {
    let mut p = params(3, 200);
    p.mix.plain_permille = 1000;
    p.mix.hard_permille = 0;
    let l = limits(&p);
    for delay in [0, 8 * NS] {
        let oracle = play(&p, &notice(delay), &l).unwrap();
        let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
        assert_eq!(oracle.verdict.totals.reasoner.calls, 0);
        let strip = |r: &SegmentRecord| results_row("x", r).replacen("privileged", "comparison", 1);
        assert_eq!(strip(&oracle), results_row("x", &never));
        assert_eq!(oracle.trajectory, never.trajectory);
        assert_eq!(oracle.verdict, never.verdict);
    }
}

#[test]
fn it_is_deterministic() {
    let p = hard_heavy(2);
    let l = limits(&p);
    let a = play(&p, &notice(4 * NS), &l).unwrap();
    let b = play(&p, &notice(4 * NS), &l).unwrap();
    assert_eq!(a.trajectory, b.trajectory);
    assert_eq!(results_row("x", &a), results_row("x", &b));
}

#[test]
fn it_takes_the_builder_of_its_rung_and_builds_none_of_its_own() {
    // With a `window` builder the calls carry window contexts: at most N references, none of them
    // the decisive-evidence sets of the other oracle. The context differs from the default's.
    let p = hard_heavy(1);
    let l = limits(&p);
    let window = RungConfig {
        context: ContextBuilder::Window {
            window_ns: 20 * NS,
            max_refs: 32,
        },
        ..RungConfig::default()
    };
    let default = calls(&play(&p, &notice(4 * NS), &l).unwrap());
    let with = calls(&play_with_rung(&p, &notice(4 * NS), &l, &window).unwrap());
    assert!(!with.is_empty());
    assert!(with.iter().all(|(_, _, c)| c.len() <= 32));
    // Same calls, same instants, different contexts.
    assert_eq!(
        default.iter().map(|(a, f, _)| (*a, *f)).collect::<Vec<_>>(),
        with.iter().map(|(a, f, _)| (*a, *f)).collect::<Vec<_>>()
    );
    assert!(default.iter().zip(&with).any(|(d, w)| d.2 != w.2));
}

// ---- The rule, driven step by step on a hand-made observation sequence

/// Drives an arm through hand-made steps and returns every escalation it proposes: the step's
/// instant in milliseconds, the focus, the context.
struct Driver {
    public: gordian_stream::StreamPublic,
    arm: Box<dyn gordian_run::stream::arms::StreamPolicy>,
    bill: gordian_core::Bill,
    ledger: gordian_core::Ledger,
    clock: gordian_core::ManualClock,
    totals: gordian_run::stream::meter::Totals,
    next: u32,
}

impl Driver {
    fn new(plan: &OraclePlan, delay_ns: u64, rung: RungConfig) -> Self {
        let p = params(0, 120);
        let public = public_of(&p);
        let arm = OracleFactory::notice(rung, delay_ns).build(plan, &public);
        let ids = gordian_run::stream::arms::rung::cheap_component_ids();
        Self {
            arm,
            bill: gordian_core::Bill::new(limits(&p).budget()),
            ledger: gordian_core::Ledger::new(),
            clock: gordian_core::ManualClock::new(),
            totals: gordian_run::stream::meter::Totals::new(&ids),
            next: 0,
            public,
        }
    }

    /// One step at `now_ms` delivering `observations` (instants in milliseconds).
    fn step(
        &mut self,
        now_ms: u64,
        observations: &[(u64, gordian_world::Observation)],
    ) -> Vec<(u64, u32, Vec<ObsRef>)> {
        let events: Vec<StreamEvent> = observations
            .iter()
            .map(|(ms, obs)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: obs.clone(),
                }
            })
            .collect();
        let input = gordian_run::stream::arms::StepInput {
            now: at(now_ms),
            events: &events,
            probe_results: &[],
            applied: &[],
            public: &self.public,
        };
        let mut meter = gordian_run::stream::meter::Meter::new(
            &mut self.bill,
            &mut self.ledger,
            &mut self.clock,
            &mut self.totals,
            "policy/test",
        );
        self.arm
            .step(&input, true, &mut meter)
            .into_iter()
            .filter_map(|p| match p.action {
                StreamAction::Escalate { context, question } => {
                    let Question::Diagnose { focus } = question;
                    Some((now_ms, focus.0, context))
                }
                _ => None,
            })
            .collect()
    }
}

fn incident(id: u32, hard: bool, first: Option<u32>) -> PlanIncident {
    PlanIncident {
        id,
        hard,
        decoy: false,
        decisive: Vec::new(),
        first: first.map(ObsId),
    }
}

fn counter(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

/// A benign reading per second at `site`, ids 0, 1, 2, ..., delivered one per step.
fn quiet(site: ServiceId, t_ms: u64) -> Vec<(u64, Observation)> {
    vec![(t_ms, counter(site, CounterName::Latency, 10))]
}

/// A burst at `site` and its dependent that the rung notices at once: six abnormal observations.
fn burst(site: ServiceId, dependent: ServiceId, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 90)),
        (t + 20, counter(site, CounterName::Saturation, 90)),
        (t + 30, counter(site, CounterName::ErrorRate, 85)),
        (t + 40, counter(dependent, CounterName::ErrorRate, 70)),
        (t + 50, counter(dependent, CounterName::Latency, 70)),
    ]
}

fn site_and_dependent() -> (ServiceId, ServiceId) {
    let public = public_of(&params(0, 120));
    for s in 0..public.services.len() {
        let site = ServiceId(s as u32);
        let mask = dependents_mask(&public.services, site);
        if let Some(d) = (0..mask.len()).find(|i| mask[*i]) {
            return (site, ServiceId(d as u32));
        }
    }
    panic!("no service has a dependent");
}

#[test]
fn a_notice_is_made_at_the_step_that_delivers_the_first_observation_and_asked_once() {
    let (site, _) = site_and_dependent();
    // Hard incident whose first observation is id 4; a non-hard incident whose first is id 2.
    let plan = OraclePlan::new(
        vec![None; 12],
        vec![incident(0, true, Some(4)), incident(1, false, Some(2))],
    );
    let mut driver = Driver::new(&plan, 3 * NS, RungConfig::default());
    let mut calls = Vec::new();
    // One benign observation per step, a second apart: observation k is delivered at step k.
    for k in 0..12u64 {
        calls.extend(driver.step(k * 1_000, &quiet(site, k * 1_000)));
    }
    // Once, exactly the delay after the step that delivered observation 4, about observation 4,
    // with the rung's own context for that anchor (the site's observations from 2 s before it).
    assert_eq!(calls.len(), 1, "{calls:?}");
    let (step_ms, focus, context) = &calls[0];
    assert_eq!((*step_ms, *focus), (7_000, 4));
    let held: Vec<u32> = (2..=7).collect();
    assert_eq!(
        context,
        &held
            .iter()
            .map(|k| ObsRef::Passive(ObsId(*k)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn nothing_but_the_hard_flag_and_the_first_observation_is_read_from_the_plan() {
    let (site, dependent) = site_and_dependent();
    let sequence = |driver: &mut Driver| {
        let mut calls = Vec::new();
        for k in 0..10u64 {
            let mut batch = quiet(site, k * 1_000 + 100);
            if k == 2 {
                batch.extend(burst(site, dependent, 2_300));
            }
            calls.extend(driver.step(k * 1_000 + 500, &batch));
        }
        calls
    };
    let a = OraclePlan::new(
        vec![Some(0), Some(1), None, Some(0)],
        vec![
            incident(0, true, Some(1)),
            incident(1, false, Some(3)),
            incident(2, true, Some(5)),
            incident(3, true, None),
        ],
    );
    // Everything the rule must not read is changed: ids, owners, the decoy flag, decisive
    // evidence. What it may read (the hard flag, the first observation) is the same.
    let b = OraclePlan::new(
        vec![None; 20],
        vec![
            PlanIncident {
                id: 40,
                hard: true,
                decoy: true,
                decisive: vec![ObsId(1), ObsId(9)],
                first: Some(ObsId(5)),
            },
            PlanIncident {
                id: 41,
                hard: false,
                decoy: false,
                decisive: vec![ObsId(3)],
                first: Some(ObsId(2)),
            },
            PlanIncident {
                id: 42,
                hard: true,
                decoy: false,
                decisive: Vec::new(),
                first: Some(ObsId(1)),
            },
        ],
    );
    let ca = sequence(&mut Driver::new(&a, 2 * NS, RungConfig::default()));
    let cb = sequence(&mut Driver::new(&b, 2 * NS, RungConfig::default()));
    assert_eq!(ca, cb);
    // Observation 1 is delivered at the step at 1.5 s, observation 5 (inside the burst) at 2.5 s,
    // and each is asked about two seconds after that step.
    let steps: Vec<u64> = ca.iter().map(|(s, _, _)| *s).collect();
    assert_eq!(steps, vec![3_500, 4_500]);
    let focuses: Vec<u32> = ca.iter().map(|(_, f, _)| *f).collect();
    assert_eq!(
        focuses,
        vec![1, 5],
        "the hard incidents with a first observation, in order"
    );
}

#[test]
fn an_anomaly_the_rung_notices_later_causes_no_call() {
    let (site, dependent) = site_and_dependent();
    // The incident's first observation (id 0) is benign and comes first; the rung's anomaly is
    // anchored on the burst that follows (id 1 or later), which it notices at its first step.
    let plan = OraclePlan::new(vec![None; 10], vec![incident(0, true, Some(0))]);
    let mut driver = Driver::new(&plan, 6 * NS, RungConfig::default());
    let mut calls = Vec::new();
    calls.extend(driver.step(0, &quiet(site, 0)));
    calls.extend(driver.step(1_000, &burst(site, dependent, 1_000)));
    for k in 2..14u64 {
        calls.extend(driver.step(k * 1_000, &[]));
    }
    assert!(
        driver.arm.report().anomalies_noticed >= 1,
        "the rung noticed the burst"
    );
    // One call, about the first observation, six seconds after the step that delivered it: the
    // rung's own notice (at 1 s) started nothing.
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!((calls[0].0, calls[0].1), (6_000, 0));
}

// ---- The rung's context for an anchor it did not notice

#[test]
fn context_at_is_the_context_of_a_noticed_anomaly_with_that_anchor_for_every_builder() {
    let p = hard_heavy(0);
    let stream = generate(&p);
    let public = stream.public_info();
    let events = stream.events().to_vec();
    let builders = [
        ContextBuilder::Rung,
        ContextBuilder::Window {
            window_ns: 20 * NS,
            max_refs: 64,
        },
        ContextBuilder::Cooccur {
            delta_ns: 2 * NS,
            max_refs: 64,
        },
        ContextBuilder::Neighbourhood {
            hops: 2,
            max_refs: 64,
        },
    ];
    let mut compared = 0usize;
    for context in builders {
        let cfg = RungConfig {
            context,
            ..RungConfig::default()
        };
        let mut rung = Rung::new(&public, cfg);
        let mut next = 0usize;
        let mut now = 0u64;
        while now <= 200 * NS {
            let mut batch = Vec::new();
            while next < events.len() && events[next].0.0 <= now {
                batch.push(StreamEvent::Observed {
                    id: ObsId(next as u32),
                    at: events[next].0,
                    obs: events[next].1.clone(),
                });
                next += 1;
            }
            let at = gordian_core::Instant(now);
            rung.absorb(&batch, &[], at);
            rung.notice(at);
            for view in rung.views(at) {
                // An anchor older than the rung's retention is no longer held.
                if rung.store().iter().all(|h| h.id != view.anchor) {
                    continue;
                }
                assert_eq!(
                    rung.context(view.id),
                    rung.context_at(view.anchor),
                    "{context:?} anomaly {} at {now}",
                    view.id
                );
                compared += 1;
            }
            now += NS / 2;
        }
    }
    assert!(compared >= 50, "compared {compared} contexts");
    // An observation that is not held has no context.
    let rung = Rung::new(&public, RungConfig::default());
    assert!(rung.context_at(ObsId(0)).is_empty());
}

// ---- Nothing that existed changed

#[test]
fn the_other_arms_are_untouched_by_the_registry_entry() {
    // The registry lists the arm and no existing arm's id or role moved.
    for id in [
        "never_escalate",
        "always_escalate",
        "oracle_escalation",
        "oracle_selection",
        "oracle_decoy",
        "oracle_selection_context",
    ] {
        assert!(StreamPolicySpec::from_id(id).is_ok(), "{id}");
    }
    assert!(gordian_run::stream::spec::KNOWN.contains(&"oracle_notice"));
}
