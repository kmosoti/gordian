//! Properties of `score_stream` over generated scenarios. The scenarios are built here from the
//! stream's types and a hand-built truth; the stream's generator is not involved.
//!
//! Besides the properties the plan names (a correct declaration with the wrong site or kind is
//! incorrect, one moved past the deadline is a miss, scoring is deterministic), the scenarios are
//! scored a second time by `reference`, a deliberately plain re-implementation that works one
//! incident at a time over lists of the accepted declarations and escalations. It is the oracle
//! for the single-pass scorer, as AGENTS.md asks of anything that will be optimised.

mod common;

use common::{CompactIncident, hand_truth};
use gordian_core::Instant;
use gordian_stream::oracle::StreamTruth;
use gordian_stream::{
    Diagnosis, HardKind, ObsId, ObsRef, Question, ReasonerCost, StreamAction, StreamHypothesis,
    StreamKind, StreamOutcome, StreamRefusal, Tier,
};
use gordian_stream_eval::{
    CallSummary, EscalationCounts, IncidentVerdict, ReasonerUsage, ScoredCounts, StreamStep,
    StreamTotals, StreamVerdict, TierCounts, score_stream,
};
use gordian_world::sense::{Probe, ProbeKind, ProbeResult};
use gordian_world::step::CostSummary;
use gordian_world::{FaultKind, Observation, ServiceId};
use proptest::prelude::*;

const S: u64 = 1_000_000_000;
const DURATION: u64 = 600 * S;

// ---------------------------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------------------------

fn arb_kind() -> impl Strategy<Value = StreamKind> {
    prop_oneof![
        (0..FaultKind::ALL.len()).prop_map(|i| StreamKind::Known(FaultKind::ALL[i])),
        (0..HardKind::ALL.len()).prop_map(|i| StreamKind::Hard(HardKind::ALL[i])),
    ]
}

fn arb_hypothesis() -> impl Strategy<Value = StreamHypothesis> {
    (arb_kind(), 0..8u32).prop_map(|(kind, site)| StreamHypothesis {
        kind,
        site: ServiceId(site),
    })
}

#[derive(Debug, Clone)]
struct IncSpec {
    tier: u8,
    critical: bool,
    gap_s: u64,
    window_s: u64,
    truth: StreamHypothesis,
    nobs: usize,
}

fn arb_inc() -> impl Strategy<Value = IncSpec> {
    (
        0..3u8,
        any::<bool>(),
        1..40u64,
        5..60u64,
        arb_hypothesis(),
        1..4usize,
    )
        .prop_map(|(tier, critical, gap_s, window_s, truth, nobs)| IncSpec {
            tier,
            critical,
            gap_s,
            window_s,
            truth,
            nobs,
        })
}

#[derive(Debug, Clone)]
struct StepSpec {
    kind: u8,
    gap_ns: u64,
    pick: usize,
    diag_choice: u8,
    diag: StreamHypothesis,
    refs: usize,
    informed: bool,
    correct: bool,
    tokens: u64,
    modelled: u64,
}

fn arb_step() -> impl Strategy<Value = StepSpec> {
    (
        (0..5u8, 0..15 * S, any::<usize>(), 0..3u8, arb_hypothesis()),
        (
            0..6usize,
            any::<bool>(),
            any::<bool>(),
            0..2000u64,
            0..2_000_000_000u64,
        ),
    )
        .prop_map(
            |(
                (kind, gap_ns, pick, diag_choice, diag),
                (refs, informed, correct, tokens, modelled),
            )| {
                StepSpec {
                    kind,
                    gap_ns,
                    pick,
                    diag_choice,
                    diag,
                    refs,
                    informed,
                    correct,
                    tokens,
                    modelled,
                }
            },
        )
}

#[derive(Debug, Clone)]
struct Spec {
    incs: Vec<IncSpec>,
    background: usize,
    shuffle: Vec<u32>,
    steps: Vec<StepSpec>,
}

fn arb_spec() -> impl Strategy<Value = Spec> {
    (
        prop::collection::vec(arb_inc(), 0..6),
        1..5usize,
        prop::collection::vec(any::<u32>(), 1..8),
        prop::collection::vec(arb_step(), 0..16),
    )
        .prop_map(|(incs, background, shuffle, steps)| Spec {
            incs,
            background,
            shuffle,
            steps,
        })
}

struct Scenario {
    truth: StreamTruth,
    steps: Vec<StreamStep>,
    calls: Vec<CallSummary>,
}

fn refusal(kind: u8) -> StreamRefusal {
    match kind % 3 {
        0 => StreamRefusal::PastDuration,
        1 => StreamRefusal::UnknownRef(ObsRef::Passive(ObsId(u32::MAX))),
        _ => StreamRefusal::ReasonerBudgetExceeded { needed_ns: 1 },
    }
}

/// A coherent scenario: every accepted step names an observation that exists and whose incident
/// has begun, call numbers run from zero, and every call summary matches its step.
fn build(spec: &Spec) -> Scenario {
    // Incidents.
    let mut incidents = Vec::new();
    let mut onset = 0u64;
    for inc in &spec.incs {
        onset += inc.gap_s * S;
        let tier = match inc.tier {
            0 => Tier::Plain,
            1 => Tier::Hard,
            _ => Tier::Decoy,
        };
        let decoy = tier == Tier::Decoy;
        incidents.push(CompactIncident {
            id: None,
            tier,
            critical: inc.critical && !decoy,
            onset_ns: onset,
            deadline_ns: (!decoy).then_some(onset + inc.window_s * S),
            truth: (!decoy).then_some(inc.truth),
            occupies: Vec::new(),
        });
    }
    // Labels, in a shuffled order.
    let mut owners: Vec<Option<u32>> = Vec::new();
    for (k, inc) in spec.incs.iter().enumerate() {
        owners.extend(std::iter::repeat_n(Some(k as u32), inc.nobs));
    }
    owners.extend(std::iter::repeat_n(None, spec.background));
    let mut keyed: Vec<(u64, Option<u32>)> = owners
        .into_iter()
        .enumerate()
        .map(|(i, o)| {
            let salt = u64::from(spec.shuffle[i % spec.shuffle.len()]);
            ((i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt, o)
        })
        .collect();
    keyed.sort_by_key(|(k, _)| *k);
    let labels: Vec<Option<u32>> = keyed.into_iter().map(|(_, o)| o).collect();
    let truth = hand_truth(DURATION, &incidents, &labels);

    // Steps.
    let mut steps = Vec::new();
    let mut calls = Vec::new();
    let mut at = 0u64;
    let (mut n_decl, mut n_probe) = (0u32, 0u32);
    for s in &spec.steps {
        at += s.gap_ns;
        let eligible: Vec<usize> = labels
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_none_or(|k| incidents[k as usize].onset_ns <= at))
            .map(|(i, _)| i)
            .collect();
        let obs = eligible[s.pick % eligible.len()];
        let owner = labels[obs];
        let instant = Instant(at);
        let context: Vec<ObsRef> = (0..s.refs)
            .map(|i| ObsRef::Passive(ObsId(i as u32)))
            .collect();
        match s.kind {
            0 | 3 => {
                let diagnosis: Diagnosis = match (s.diag_choice, owner) {
                    (0, Some(k)) => incidents[k as usize].truth,
                    (1, _) => None,
                    _ => Some(s.diag),
                };
                let action = StreamAction::Declare {
                    anchor: ObsId(obs as u32),
                    diagnosis,
                };
                let outcome = if s.kind == 0 {
                    n_decl += 1;
                    StreamOutcome::Declared { index: n_decl - 1 }
                } else {
                    StreamOutcome::Refused(refusal(s.diag_choice))
                };
                steps.push(StreamStep {
                    at: instant,
                    action,
                    outcome,
                });
            }
            1 | 4 => {
                let action = StreamAction::Escalate {
                    context,
                    question: Question::Diagnose {
                        focus: ObsId(obs as u32),
                    },
                };
                let outcome = if s.kind == 1 {
                    let ready_at = Instant(at + 2 * S);
                    calls.push(CallSummary {
                        at: instant,
                        ready_at,
                        focus: None,
                        refs: s.refs as u32,
                        informed: s.informed,
                        correct: s.correct,
                    });
                    StreamOutcome::Escalated {
                        call: calls.len() as u32 - 1,
                        ready_at,
                        cost: ReasonerCost {
                            calls: 1,
                            tokens: s.tokens,
                            modelled_ns: s.modelled,
                            latency_ns: 2 * S,
                        },
                    }
                } else {
                    StreamOutcome::Refused(refusal(s.diag_choice))
                };
                steps.push(StreamStep {
                    at: instant,
                    action,
                    outcome,
                });
            }
            _ => {
                let probe = Probe {
                    kind: ProbeKind::HealthCheck,
                    target: ServiceId(0),
                };
                steps.push(StreamStep {
                    at: instant,
                    action: StreamAction::Probe {
                        kind: probe.kind,
                        target: probe.target,
                    },
                    outcome: StreamOutcome::Probed {
                        probe: n_probe,
                        observation: Observation::Probed {
                            probe,
                            result: ProbeResult::Negative,
                        },
                        ready_at: instant,
                        cost: CostSummary {
                            probes: 1,
                            time_ns: 1,
                        },
                    },
                });
                n_probe += 1;
            }
        }
    }
    Scenario {
        truth,
        steps,
        calls,
    }
}

// ---------------------------------------------------------------------------------------------
// The reference scorer
// ---------------------------------------------------------------------------------------------

struct Decl {
    at: Instant,
    incident: Option<usize>,
    diagnosis: Diagnosis,
}

struct Esc {
    incident: Option<usize>,
    refs: u64,
    tokens: u64,
    modelled_ns: u64,
    call: usize,
}

fn reference(truth: &StreamTruth, steps: &[StreamStep], calls: &[CallSummary]) -> StreamVerdict {
    let mut decls = Vec::new();
    let mut escs = Vec::new();
    for s in steps {
        if matches!(s.outcome, StreamOutcome::Refused(_)) {
            continue;
        }
        match (&s.action, &s.outcome) {
            (StreamAction::Declare { anchor, diagnosis }, _) => decls.push(Decl {
                at: s.at,
                incident: truth.incident_of(*anchor).map(|i| i as usize),
                diagnosis: *diagnosis,
            }),
            (
                StreamAction::Escalate { context, question },
                StreamOutcome::Escalated { call, cost, .. },
            ) => {
                let Question::Diagnose { focus } = question;
                escs.push(Esc {
                    incident: truth.incident_of(*focus).map(|i| i as usize),
                    refs: context.len() as u64,
                    tokens: cost.tokens,
                    modelled_ns: cost.modelled_ns,
                    call: *call as usize,
                });
            }
            _ => {}
        }
    }

    let mut per_incident = Vec::new();
    for (k, inc) in truth.incidents.iter().enumerate() {
        let mine: Vec<&Decl> = decls.iter().filter(|d| d.incident == Some(k)).collect();
        let right: Vec<&&Decl> = mine.iter().filter(|d| d.diagnosis == inc.truth).collect();
        let wrong = mine.len() - right.len();
        let first = right.iter().map(|d| d.at).min();
        let on_time = match (first, inc.deadline_ns) {
            (Some(f), Some(dl)) => f.0 <= dl,
            _ => false,
        };
        let scored = inc.tier == Tier::Plain || inc.tier == Tier::Hard;
        let missed = scored && !on_time;
        let my_escs: Vec<&Esc> = escs.iter().filter(|e| e.incident == Some(k)).collect();
        per_incident.push(IncidentVerdict {
            id: inc.id,
            tier: inc.tier,
            critical: inc.critical,
            correct_declarations: right.len() as u32,
            wrong_declarations: wrong as u32,
            first_correct_at: first,
            time_to_first_correct_ns: first.map(|f| f.0 - inc.onset_ns),
            correct_by_deadline: on_time,
            missed,
            critical_miss: inc.critical && missed,
            escalations: my_escs.len() as u32,
            informed_escalations: my_escs.iter().filter(|e| calls[e.call].informed).count() as u32,
            correct_escalations: my_escs.iter().filter(|e| calls[e.call].correct).count() as u32,
        });
    }

    let count =
        |f: &dyn Fn(&IncidentVerdict) -> bool| per_incident.iter().filter(|i| f(i)).count() as u32;
    let tier_is = |t: Tier| move |i: &IncidentVerdict| i.tier == t;
    let bg_alarms = decls
        .iter()
        .filter(|d| d.incident.is_none() && d.diagnosis.is_some())
        .count() as u32;
    let decoy_alarms: u32 = per_incident
        .iter()
        .filter(|i| i.tier == Tier::Decoy)
        .map(|i| i.wrong_declarations)
        .sum();
    let tier_of = |e: &Esc| e.incident.map(|k| truth.incidents[k].tier);
    let escalated = |f: &dyn Fn(Tier) -> bool| {
        per_incident
            .iter()
            .filter(|i| i.escalations > 0 && f(i.tier))
            .count() as u32
    };
    let totals = StreamTotals {
        incidents: TierCounts {
            plain: count(&tier_is(Tier::Plain)),
            hard: count(&tier_is(Tier::Hard)),
            decoy: count(&tier_is(Tier::Decoy)),
        },
        critical_incidents: count(&|i| i.critical),
        correct: ScoredCounts {
            plain: count(&|i| i.tier == Tier::Plain && i.correct_by_deadline),
            hard: count(&|i| i.tier == Tier::Hard && i.correct_by_deadline),
        },
        missed: ScoredCounts {
            plain: count(&|i| i.tier == Tier::Plain && i.missed),
            hard: count(&|i| i.tier == Tier::Hard && i.missed),
        },
        critical_missed: ScoredCounts {
            plain: count(&|i| i.tier == Tier::Plain && i.critical_miss),
            hard: count(&|i| i.tier == Tier::Hard && i.critical_miss),
        },
        wrong_declarations: per_incident
            .iter()
            .filter(|i| i.tier != Tier::Decoy)
            .map(|i| i.wrong_declarations)
            .sum(),
        decoys_dismissed: count(&|i| i.tier == Tier::Decoy && i.correct_declarations > 0),
        decoys_alarmed: count(&|i| i.tier == Tier::Decoy && i.wrong_declarations > 0),
        decoys_silent: count(&|i| {
            i.tier == Tier::Decoy && i.correct_declarations + i.wrong_declarations == 0
        }),
        false_alarms: bg_alarms + decoy_alarms,
        false_alarms_on_background: bg_alarms,
        escalations: EscalationCounts {
            needed: escs
                .iter()
                .filter(|e| tier_of(e) == Some(Tier::Hard))
                .count() as u32,
            unneeded: escs
                .iter()
                .filter(|e| matches!(tier_of(e), Some(Tier::Plain | Tier::Decoy)))
                .count() as u32,
            background: escs.iter().filter(|e| e.incident.is_none()).count() as u32,
            hard_incidents_escalated: escalated(&|t| t == Tier::Hard),
            other_incidents_escalated: escalated(&|t| t != Tier::Hard),
            informed: escs.iter().filter(|e| calls[e.call].informed).count() as u32,
            correct: escs.iter().filter(|e| calls[e.call].correct).count() as u32,
        },
        reasoner: ReasonerUsage {
            calls: escs.len() as u64,
            refs: escs.iter().map(|e| e.refs).sum(),
            tokens: escs.iter().map(|e| e.tokens).sum(),
            modelled_ns: escs.iter().map(|e| e.modelled_ns).sum(),
        },
    };
    StreamVerdict {
        per_incident,
        totals,
    }
}

// ---------------------------------------------------------------------------------------------
// Properties of whole scenarios
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn scoring_is_deterministic(spec in arb_spec()) {
        let sc = build(&spec);
        let first = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        let again = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        prop_assert_eq!(&first, &again);
        let (t, s, c) = (sc.truth.clone(), sc.steps.clone(), sc.calls.clone());
        prop_assert_eq!(&first, &score_stream(&t, &s, &c).expect("coherent scenario"));
    }

    #[test]
    fn scoring_agrees_with_the_reference(spec in arb_spec()) {
        let sc = build(&spec);
        let got = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        prop_assert_eq!(got, reference(&sc.truth, &sc.steps, &sc.calls));
    }

    /// S3: refused steps, at any instant not before the last step, change nothing, and neither
    /// does an accepted probe.
    #[test]
    fn refused_steps_and_probes_change_nothing(spec in arb_spec(), kind in 0..4u8, anchor in any::<u32>()) {
        let sc = build(&spec);
        let before = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        let mut steps = sc.steps.clone();
        let at = steps.last().map_or(Instant(0), |s| s.at);
        // Anchors that exist nowhere: a refused step is not checked (S3).
        let anchor = ObsId(anchor);
        let extra = match kind {
            0 => StreamStep {
                at,
                action: StreamAction::Declare { anchor, diagnosis: None },
                outcome: StreamOutcome::Refused(StreamRefusal::UnknownRef(ObsRef::Passive(anchor))),
            },
            1 => StreamStep {
                at,
                action: StreamAction::Escalate {
                    context: vec![],
                    question: Question::Diagnose { focus: anchor },
                },
                outcome: StreamOutcome::Refused(StreamRefusal::ReasonerBudgetExceeded { needed_ns: 1 }),
            },
            2 => StreamStep {
                at: Instant(DURATION + S),
                action: StreamAction::Declare { anchor, diagnosis: None },
                outcome: StreamOutcome::Refused(StreamRefusal::PastDuration),
            },
            _ => {
                let probe = Probe { kind: ProbeKind::ResourceUsage, target: ServiceId(1) };
                StreamStep {
                    at,
                    action: StreamAction::Probe { kind: probe.kind, target: probe.target },
                    outcome: StreamOutcome::Probed {
                        probe: 999,
                        observation: Observation::Probed { probe, result: ProbeResult::Positive },
                        ready_at: at,
                        cost: CostSummary { probes: 1, time_ns: 1 },
                    },
                }
            }
        };
        steps.push(extra);
        let after = score_stream(&sc.truth, &steps, &sc.calls).expect("still coherent");
        prop_assert_eq!(before, after);
    }

    /// S11: a further wrong declaration about an incident adds one wrong declaration and takes
    /// nothing from a correct one.
    #[test]
    fn a_wrong_declaration_never_removes_a_correct_one(spec in arb_spec(), pick in any::<usize>()) {
        let sc = build(&spec);
        prop_assume!(!sc.truth.incidents.is_empty());
        let before = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        let k = pick % sc.truth.incidents.len();
        let inc = &sc.truth.incidents[k];
        let anchor = sc.truth.labels.iter().enumerate()
            .find(|(i, _)| sc.truth.incident_of(ObsId(*i as u32)) == Some(k as u32))
            .map(|(i, _)| ObsId(i as u32));
        let anchor = anchor.expect("every incident has an observation");
        // A hypothesis that is not the truth.
        let wrong = Some(StreamHypothesis { kind: StreamKind::Hard(HardKind::SlowLeak), site: ServiceId(99) });
        prop_assert_ne!(wrong, inc.truth);
        let at = sc.steps.last().map_or(Instant(inc.onset_ns), |s| Instant(s.at.0.max(inc.onset_ns)));
        let mut steps = sc.steps.clone();
        steps.push(StreamStep {
            at,
            action: StreamAction::Declare { anchor, diagnosis: wrong },
            outcome: StreamOutcome::Declared { index: 999 },
        });
        let after = score_stream(&sc.truth, &steps, &sc.calls).expect("still coherent");
        let (b, a) = (&before.per_incident[k], &after.per_incident[k]);
        prop_assert_eq!(a.wrong_declarations, b.wrong_declarations + 1);
        prop_assert_eq!(a.correct_declarations, b.correct_declarations);
        prop_assert_eq!(a.correct_by_deadline, b.correct_by_deadline);
        prop_assert_eq!(a.first_correct_at, b.first_correct_at);
        prop_assert_eq!(a.missed, b.missed);
    }

    /// S3, S26: an accepted escalation adds exactly one call, in exactly one class.
    #[test]
    fn totals_account_for_every_call_once(spec in arb_spec()) {
        let sc = build(&spec);
        let v = score_stream(&sc.truth, &sc.steps, &sc.calls).expect("coherent scenario");
        let e = &v.totals.escalations;
        prop_assert_eq!(u64::from(e.needed + e.unneeded + e.background), v.totals.reasoner.calls);
        prop_assert_eq!(v.totals.reasoner.calls as usize, sc.calls.len());
        prop_assert_eq!(v.per_incident.len(), sc.truth.incidents.len());
        let t = &v.totals.incidents;
        prop_assert_eq!(
            (t.plain + t.hard + t.decoy) as usize,
            sc.truth.incidents.len()
        );
        // Every decoy is silent, or dismissed, or alarmed, or both of the last two.
        prop_assert!(
            v.totals.decoys_silent + v.totals.decoys_dismissed + v.totals.decoys_alarmed >= t.decoy
        );
        prop_assert!(v.totals.decoys_silent <= t.decoy);
        let p = v.totals.escalation_precision();
        prop_assert_eq!(p.is_some(), v.totals.reasoner.calls > 0);
        if let Some(p) = p {
            prop_assert!((0.0..=1.0).contains(&p));
        }
        let r = v.totals.escalation_recall();
        prop_assert_eq!(r.is_some(), t.hard > 0);
        if let Some(r) = r {
            prop_assert!((0.0..=1.0).contains(&r));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The properties the plan names: one incident, one correct declaration, then one change
// ---------------------------------------------------------------------------------------------

struct One {
    truth: StreamTruth,
    hypothesis: StreamHypothesis,
    onset: u64,
    deadline: u64,
}

fn one(
    hard: bool,
    critical: bool,
    onset_s: u64,
    window_s: u64,
    hypothesis: StreamHypothesis,
) -> One {
    let onset = onset_s * S;
    let deadline = onset + window_s * S;
    let incident = CompactIncident {
        id: None,
        tier: if hard { Tier::Hard } else { Tier::Plain },
        critical,
        onset_ns: onset,
        deadline_ns: Some(deadline),
        truth: Some(hypothesis),
        occupies: Vec::new(),
    };
    One {
        truth: hand_truth(DURATION, &[incident], &[Some(0)]),
        hypothesis,
        onset,
        deadline,
    }
}

fn declare(at: u64, diagnosis: Diagnosis) -> Vec<StreamStep> {
    vec![StreamStep {
        at: Instant(at),
        action: StreamAction::Declare {
            anchor: ObsId(0),
            diagnosis,
        },
        outcome: StreamOutcome::Declared { index: 0 },
    }]
}

fn the_verdict(one: &One, at: u64, diagnosis: Diagnosis) -> IncidentVerdict {
    let v = score_stream(&one.truth, &declare(at, diagnosis), &[]).expect("coherent");
    v.per_incident.into_iter().next().expect("one incident")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn a_correct_declaration_in_time_is_correct(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64,
        h in arb_hypothesis(), offset in 0.0..=1.0f64,
    ) {
        let o = one(hard, critical, onset_s, window_s, h);
        let at = o.onset + ((o.deadline - o.onset) as f64 * offset) as u64;
        let v = the_verdict(&o, at, Some(o.hypothesis));
        prop_assert!(v.correct_by_deadline);
        prop_assert!(!v.missed);
        prop_assert!(!v.critical_miss);
        prop_assert_eq!(v.correct_declarations, 1);
        prop_assert_eq!(v.wrong_declarations, 0);
        prop_assert_eq!(v.first_correct_at, Some(Instant(at)));
        prop_assert_eq!(v.time_to_first_correct_ns, Some(at - o.onset));
    }

    #[test]
    fn changing_the_site_of_a_correct_declaration_makes_it_incorrect(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64,
        h in arb_hypothesis(), offset in 0.0..=1.0f64, other in 0..8u32,
    ) {
        prop_assume!(other != h.site.0);
        let o = one(hard, critical, onset_s, window_s, h);
        let at = o.onset + ((o.deadline - o.onset) as f64 * offset) as u64;
        let changed = StreamHypothesis { kind: h.kind, site: ServiceId(other) };
        let v = the_verdict(&o, at, Some(changed));
        prop_assert!(!v.correct_by_deadline);
        prop_assert!(v.missed);
        prop_assert_eq!(v.critical_miss, critical);
        prop_assert_eq!(v.correct_declarations, 0);
        prop_assert_eq!(v.wrong_declarations, 1);
        prop_assert_eq!(v.first_correct_at, None);
    }

    #[test]
    fn changing_the_kind_of_a_correct_declaration_makes_it_incorrect(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64,
        h in arb_hypothesis(), offset in 0.0..=1.0f64, other in arb_kind(),
    ) {
        prop_assume!(other != h.kind);
        let o = one(hard, critical, onset_s, window_s, h);
        let at = o.onset + ((o.deadline - o.onset) as f64 * offset) as u64;
        let changed = StreamHypothesis { kind: other, site: h.site };
        let v = the_verdict(&o, at, Some(changed));
        prop_assert!(!v.correct_by_deadline);
        prop_assert!(v.missed);
        prop_assert_eq!(v.critical_miss, critical);
        prop_assert_eq!(v.correct_declarations, 0);
        prop_assert_eq!(v.wrong_declarations, 1);
        prop_assert_eq!(v.first_correct_at, None);
    }

    #[test]
    fn declaring_nothing_is_wrong_about_a_real_incident(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64,
        h in arb_hypothesis(), offset in 0.0..=1.0f64,
    ) {
        let o = one(hard, critical, onset_s, window_s, h);
        let at = o.onset + ((o.deadline - o.onset) as f64 * offset) as u64;
        let v = the_verdict(&o, at, None);
        prop_assert!(v.missed);
        prop_assert_eq!(v.wrong_declarations, 1);
    }

    #[test]
    fn moving_a_correct_declaration_past_the_deadline_makes_it_a_miss(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64,
        h in arb_hypothesis(), late_ns in 1..100 * S,
    ) {
        let o = one(hard, critical, onset_s, window_s, h);
        let at = o.deadline + late_ns;
        let v = the_verdict(&o, at, Some(o.hypothesis));
        prop_assert!(!v.correct_by_deadline);
        prop_assert!(v.missed);
        prop_assert_eq!(v.critical_miss, critical);
        // Late, not wrong.
        prop_assert_eq!(v.correct_declarations, 1);
        prop_assert_eq!(v.wrong_declarations, 0);
        prop_assert_eq!(v.first_correct_at, Some(Instant(at)));
    }

    #[test]
    fn the_deadline_itself_is_on_time(
        hard in any::<bool>(), critical in any::<bool>(),
        onset_s in 0..100u64, window_s in 1..200u64, h in arb_hypothesis(),
    ) {
        let o = one(hard, critical, onset_s, window_s, h);
        let v = the_verdict(&o, o.deadline, Some(o.hypothesis));
        prop_assert!(v.correct_by_deadline);
        let v = the_verdict(&o, o.deadline + 1, Some(o.hypothesis));
        prop_assert!(!v.correct_by_deadline);
    }
}
