//! `score_stream` against streams the generator makes, with `truth_from_stream` and
//! `calls_from_sim` as the only bridges.
//!
//! The expectations are stated from the tier definitions and the rules, not read back from the
//! code under test: a trajectory that declares every incident's truth early is correct
//! everywhere; a silent one misses everything; and the evaluator's reading of which incident an
//! escalation is about agrees with the simulator's own record of it.

use gordian_core::Instant;
use gordian_stream::oracle::{self, IncidentTruth, StreamTruth};
use gordian_stream::{
    Diagnosis, HardKind, ObsId, ObsRef, Question, Stream, StreamAction, StreamHypothesis,
    StreamKind, StreamOutcome, StreamParams, StreamSimulator, Tier, generate,
};
use gordian_stream_eval::{
    StreamStep, StreamVerdict, calls_from_sim, score_stream, truth_from_stream,
};
use gordian_world::ServiceId;

const SEEDS: u64 = 30;

fn streams() -> impl Iterator<Item = (u64, Stream)> {
    (0..SEEDS).map(|seed| (seed, generate(&StreamParams::new(seed))))
}

/// The instant of an incident's first observation: when a policy can first anchor on it.
fn first_observation(stream: &Stream, inc: &IncidentTruth) -> (ObsId, Instant) {
    let id = *inc
        .observations
        .first()
        .unwrap_or_else(|| panic!("incident {} has no observation", inc.id));
    (id, stream.events()[id.0 as usize].0)
}

/// Run one declaration per incident through a real simulator, each at its first observation,
/// saying `say(incident)`.
fn declare_each(
    stream: &Stream,
    truth: &StreamTruth,
    say: impl Fn(&IncidentTruth) -> Diagnosis,
) -> (Vec<StreamStep>, StreamSimulator) {
    let mut plan: Vec<(Instant, ObsId, Diagnosis)> = truth
        .incidents
        .iter()
        .map(|inc| {
            let (id, at) = first_observation(stream, inc);
            (at, id, say(inc))
        })
        .collect();
    plan.sort_by_key(|(at, id, _)| (*at, *id));
    let mut sim = StreamSimulator::new(stream.clone());
    let mut steps = Vec::new();
    for (at, anchor, diagnosis) in plan {
        sim.observe_until(at);
        let action = StreamAction::Declare { anchor, diagnosis };
        let outcome = sim.apply(action.clone(), at);
        assert!(
            matches!(outcome, StreamOutcome::Declared { .. }),
            "the simulator refused a declaration at an observation it had delivered: {outcome:?}"
        );
        steps.push(StreamStep {
            at,
            action,
            outcome,
        });
    }
    (steps, sim)
}

fn tier_counts(truth: &StreamTruth) -> (u32, u32, u32) {
    let n = |t: Tier| truth.incidents.iter().filter(|i| i.tier == t).count() as u32;
    (n(Tier::Plain), n(Tier::Hard), n(Tier::Decoy))
}

#[test]
fn the_generated_streams_have_every_tier_and_critical_incidents() {
    // The tests below would pass vacuously on streams without these.
    let (mut plain, mut hard, mut decoy, mut critical_hard, mut critical_plain) = (0, 0, 0, 0, 0);
    for (_, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let (p, h, d) = tier_counts(&truth);
        plain += p;
        hard += h;
        decoy += d;
        critical_hard += truth
            .incidents
            .iter()
            .filter(|i| i.critical && i.tier == Tier::Hard)
            .count();
        critical_plain += truth
            .incidents
            .iter()
            .filter(|i| i.critical && i.tier == Tier::Plain)
            .count();
    }
    assert!(
        plain > 100 && hard > 20 && decoy > 20,
        "{plain} {hard} {decoy}"
    );
    assert!(
        critical_hard > 5 && critical_plain > 5,
        "{critical_hard} {critical_plain}"
    );
}

/// The assumption behind S34: no observation of an incident precedes the incident's onset.
#[test]
fn no_observation_of_an_incident_precedes_its_onset() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        for (i, (at, _)) in stream.events().iter().enumerate() {
            if let Some(k) = truth.incident_of(ObsId(i as u32)) {
                assert!(
                    at.0 >= truth.incidents[k as usize].onset_ns,
                    "seed {seed}: observation {i} of incident {k} precedes its onset"
                );
            }
        }
        assert_eq!(truth.labels.len(), stream.events().len(), "seed {seed}");
        for (k, inc) in truth.incidents.iter().enumerate() {
            assert_eq!(inc.id as usize, k, "seed {seed}: ids are dense (S27)");
        }
    }
}

#[test]
fn declaring_every_incidents_truth_early_scores_every_incident_correct() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let (steps, sim) = declare_each(&stream, &truth, |inc| inc.truth);
        let calls = calls_from_sim(&sim);
        let v = score_stream(&truth, &steps, &calls).unwrap_or_else(|e| panic!("seed {seed}: {e}"));

        assert_eq!(v.per_incident.len(), truth.incidents.len(), "seed {seed}");
        for (inc, row) in truth.incidents.iter().zip(&v.per_incident) {
            let ctx = format!("seed {seed}, incident {} ({:?})", inc.id, inc.tier);
            assert_eq!(row.id, inc.id, "{ctx}");
            assert_eq!(row.tier, inc.tier, "{ctx}");
            assert_eq!(row.critical, inc.critical, "{ctx}");
            assert_eq!(row.correct_declarations, 1, "{ctx}");
            assert_eq!(row.wrong_declarations, 0, "{ctx}");
            assert!(!row.missed && !row.critical_miss, "{ctx}");
            assert_eq!(row.escalations, 0, "{ctx}");
            // The first observation is at or after the onset, within a few seconds of it.
            let ttc = row.time_to_first_correct_ns.expect("a correct declaration");
            assert!(ttc < 3_000_000_000, "{ctx}: {ttc}");
            match inc.tier {
                Tier::Plain | Tier::Hard => assert!(row.correct_by_deadline, "{ctx}"),
                // A decoy's truth is `None`: declaring it is a dismissal, not a deadline hit.
                Tier::Decoy => assert!(!row.correct_by_deadline, "{ctx}"),
            }
        }
        let (plain, hard, decoy) = tier_counts(&truth);
        let t = &v.totals;
        assert_eq!(
            (t.incidents.plain, t.incidents.hard, t.incidents.decoy),
            (plain, hard, decoy)
        );
        assert_eq!(
            (t.correct.plain, t.correct.hard),
            (plain, hard),
            "seed {seed}"
        );
        assert_eq!((t.missed.plain, t.missed.hard), (0, 0), "seed {seed}");
        assert_eq!(
            (t.critical_missed.plain, t.critical_missed.hard),
            (0, 0),
            "seed {seed}"
        );
        assert_eq!(t.wrong_declarations, 0, "seed {seed}");
        assert_eq!(t.false_alarms, 0, "seed {seed}: no false alarms");
        assert_eq!(t.false_alarms_on_background, 0, "seed {seed}");
        assert_eq!(t.decoys_dismissed, decoy, "seed {seed}");
        assert_eq!((t.decoys_alarmed, t.decoys_silent), (0, 0), "seed {seed}");
        assert_eq!(t.reasoner.calls, 0, "seed {seed}");
    }
}

#[test]
fn a_silent_trajectory_misses_every_incident_and_every_critical_one_critically() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let sim = StreamSimulator::new(stream.clone());
        let v = score_stream(&truth, &[], &calls_from_sim(&sim))
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        for (inc, row) in truth.incidents.iter().zip(&v.per_incident) {
            let ctx = format!("seed {seed}, incident {} ({:?})", inc.id, inc.tier);
            assert_eq!(
                row.correct_declarations + row.wrong_declarations,
                0,
                "{ctx}"
            );
            assert_eq!(row.first_correct_at, None, "{ctx}");
            assert!(!row.correct_by_deadline, "{ctx}");
            match inc.tier {
                Tier::Plain | Tier::Hard => {
                    assert!(row.missed, "{ctx}");
                    assert_eq!(row.critical_miss, inc.critical, "{ctx}");
                }
                // Silence about a decoy is neither a miss nor an error.
                Tier::Decoy => assert!(!row.missed && !row.critical_miss, "{ctx}"),
            }
        }
        let (plain, hard, decoy) = tier_counts(&truth);
        let crit = |t: Tier| {
            truth
                .incidents
                .iter()
                .filter(|i| i.tier == t && i.critical)
                .count() as u32
        };
        let t = &v.totals;
        assert_eq!(
            (t.missed.plain, t.missed.hard),
            (plain, hard),
            "seed {seed}"
        );
        assert_eq!(
            (t.critical_missed.plain, t.critical_missed.hard),
            (crit(Tier::Plain), crit(Tier::Hard)),
            "seed {seed}"
        );
        assert_eq!((t.correct.plain, t.correct.hard), (0, 0), "seed {seed}");
        assert_eq!(t.decoys_silent, decoy, "seed {seed}");
        assert_eq!(
            (t.decoys_dismissed, t.decoys_alarmed, t.false_alarms),
            (0, 0, 0)
        );
        assert_eq!(
            t.critical_incidents,
            truth.incidents.iter().filter(|i| i.critical).count() as u32
        );
    }
}

/// Declaring something about every incident, but the wrong thing: nothing is correct, every
/// decoy is alarmed, and the false alarms are exactly the decoys.
#[test]
fn declaring_a_wrong_site_for_every_incident_scores_wrong_and_alarms_every_decoy() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let wrong_site = |inc: &IncidentTruth| -> Diagnosis {
            Some(match inc.truth {
                Some(h) => StreamHypothesis {
                    kind: h.kind,
                    site: ServiceId((h.site.0 + 1) % truth.services.len() as u32),
                },
                None => StreamHypothesis {
                    kind: StreamKind::Hard(HardKind::SlowLeak),
                    site: ServiceId(0),
                },
            })
        };
        let (steps, sim) = declare_each(&stream, &truth, wrong_site);
        let v = score_stream(&truth, &steps, &calls_from_sim(&sim))
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let (plain, hard, decoy) = tier_counts(&truth);
        let t = &v.totals;
        assert_eq!((t.correct.plain, t.correct.hard), (0, 0), "seed {seed}");
        assert_eq!(
            (t.missed.plain, t.missed.hard),
            (plain, hard),
            "seed {seed}"
        );
        assert_eq!(t.wrong_declarations, plain + hard, "seed {seed}");
        assert_eq!(t.decoys_alarmed, decoy, "seed {seed}");
        assert_eq!(t.false_alarms, decoy, "seed {seed}");
        assert_eq!(t.decoys_dismissed, 0, "seed {seed}");
    }
}

/// Escalate about each incident's first observation with its decisive evidence in the context,
/// and about one background observation, through a real simulator. The evaluator's reading of
/// what each call was about, its informed and correct counts and its costs must agree with the
/// simulator's own record.
#[test]
fn the_evaluators_reading_of_escalations_agrees_with_the_simulators_record() {
    let mut informed_somewhere = false;
    let mut uninformed_somewhere = false;
    for (seed, stream) in streams().take(12) {
        let truth = truth_from_stream(&stream);
        let end = Instant(truth.duration_ns);
        let mut sim = StreamSimulator::new(stream.clone());
        sim.observe_until(end);

        // One call per incident, then two about background observations, then one repeat.
        let mut focuses: Vec<(ObsId, Vec<ObsRef>)> = Vec::new();
        for inc in &truth.incidents {
            let first = inc.observations[0];
            let context = inc.decisive.iter().map(|o| ObsRef::Passive(*o)).collect();
            focuses.push((first, context));
        }
        let background: Vec<ObsId> = truth
            .labels
            .iter()
            .enumerate()
            .filter(|(i, _)| truth.incident_of(ObsId(*i as u32)).is_none())
            .map(|(i, _)| ObsId(i as u32))
            .take(2)
            .collect();
        assert_eq!(background.len(), 2, "seed {seed}");
        for b in &background {
            focuses.push((*b, vec![ObsRef::Passive(*b)]));
        }
        if let Some(inc) = truth.incidents.first() {
            focuses.push((inc.observations[0], vec![]));
        }

        let mut steps = Vec::new();
        for (focus, context) in focuses {
            let action = StreamAction::Escalate {
                context,
                question: Question::Diagnose { focus },
            };
            let outcome = sim.apply(action.clone(), end);
            assert!(
                matches!(outcome, StreamOutcome::Escalated { .. }),
                "seed {seed}: {outcome:?}"
            );
            steps.push(StreamStep {
                at: end,
                action,
                outcome,
            });
        }
        let calls = calls_from_sim(&sim);
        // `calls_from_sim` fills the focus from the stream's trace (work item R3b); it is the
        // focus of the escalation that made the call.
        for (call, step) in calls.iter().zip(&steps) {
            let StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } = &step.action
            else {
                unreachable!("every step here is an escalation");
            };
            assert_eq!(call.focus, Some(*focus), "seed {seed}");
        }
        let v: StreamVerdict =
            score_stream(&truth, &steps, &calls).unwrap_or_else(|e| panic!("seed {seed}: {e}"));

        // The simulator's own record of which incident each call was about.
        let hidden = oracle::calls(&sim);
        assert_eq!(hidden.len(), steps.len(), "seed {seed}");
        for (inc, row) in truth.incidents.iter().zip(&v.per_incident) {
            let mine: Vec<_> = hidden
                .iter()
                .filter(|c| c.incident == Some(inc.id))
                .collect();
            assert_eq!(
                row.escalations as usize,
                mine.len(),
                "seed {seed} inc {}",
                inc.id
            );
            assert_eq!(
                row.informed_escalations as usize,
                mine.iter().filter(|c| c.informed).count(),
                "seed {seed} inc {}",
                inc.id
            );
            assert_eq!(
                row.correct_escalations as usize,
                mine.iter().filter(|c| c.correct).count(),
                "seed {seed} inc {}",
                inc.id
            );
            informed_somewhere |= mine.iter().any(|c| c.informed);
            uninformed_somewhere |= mine.iter().any(|c| !c.informed);
        }
        let tier_of = |c: &oracle::CallTrace| c.incident.map(|k| truth.incidents[k as usize].tier);
        let e = &v.totals.escalations;
        assert_eq!(
            e.needed as usize,
            hidden
                .iter()
                .filter(|c| tier_of(c) == Some(Tier::Hard))
                .count(),
            "seed {seed}"
        );
        assert_eq!(
            e.unneeded as usize,
            hidden
                .iter()
                .filter(|c| matches!(tier_of(c), Some(Tier::Plain | Tier::Decoy)))
                .count(),
            "seed {seed}"
        );
        assert_eq!(
            e.background as usize,
            hidden.iter().filter(|c| c.incident.is_none()).count(),
            "seed {seed}"
        );
        assert_eq!(e.background, 2, "seed {seed}");
        assert_eq!(
            e.informed as usize,
            hidden.iter().filter(|c| c.informed).count()
        );
        assert_eq!(
            e.correct as usize,
            hidden.iter().filter(|c| c.correct).count()
        );
        // Cost: the sum of what each outcome declared, and of the context lengths.
        let (mut tokens, mut ns, mut refs) = (0u64, 0u64, 0u64);
        for s in &steps {
            if let (StreamAction::Escalate { context, .. }, StreamOutcome::Escalated { cost, .. }) =
                (&s.action, &s.outcome)
            {
                tokens += cost.tokens;
                ns += cost.modelled_ns;
                refs += context.len() as u64;
            }
        }
        assert_eq!(v.totals.reasoner.tokens, tokens, "seed {seed}");
        assert_eq!(v.totals.reasoner.modelled_ns, ns, "seed {seed}");
        assert_eq!(v.totals.reasoner.refs, refs, "seed {seed}");
        assert_eq!(
            v.totals.reasoner.calls as usize,
            hidden.len(),
            "seed {seed}"
        );
        // The declared cost is the stream's: tokens are the base plus a term per reference.
        assert_eq!(tokens, 400 * hidden.len() as u64 + 20 * refs, "seed {seed}");
    }
    assert!(
        informed_somewhere,
        "no informed call: the test has no power"
    );
    assert!(
        uninformed_somewhere,
        "no uninformed call: the test has no power"
    );
}

/// The stream refuses an anchor it has not delivered; the refused step scores nothing (S3).
#[test]
fn a_declaration_the_stream_refused_scores_nothing() {
    let (seed, stream) = streams().next().expect("a stream");
    let truth = truth_from_stream(&stream);
    let mut sim = StreamSimulator::new(stream.clone());
    // Nothing has been delivered yet.
    let action = StreamAction::Declare {
        anchor: ObsId(0),
        diagnosis: truth.incidents[0].truth,
    };
    let outcome = sim.apply(action.clone(), Instant(0));
    assert!(
        matches!(outcome, StreamOutcome::Refused(_)),
        "seed {seed}: {outcome:?}"
    );
    let steps = [StreamStep {
        at: Instant(0),
        action,
        outcome,
    }];
    let v = score_stream(&truth, &steps, &[]).expect("scores");
    let silent = score_stream(&truth, &[], &[]).expect("scores");
    assert_eq!(v, silent);
}

#[test]
fn truth_and_verdict_are_reproducible_and_survive_json() {
    let (_, a) = streams().next().expect("a stream");
    let (_, b) = streams().next().expect("a stream");
    let (ta, tb) = (truth_from_stream(&a), truth_from_stream(&b));
    assert_eq!(ta, tb);
    let json = serde_json::to_string(&ta).expect("serializes");
    let back: StreamTruth = serde_json::from_str(&json).expect("parses");
    // serde_json's default float parser can differ from the written value in the last digit, so
    // `difficulty`, which the scorer never reads, is set aside; everything else is exact.
    let strip = |t: &StreamTruth| {
        let mut t = t.clone();
        for i in &mut t.incidents {
            i.difficulty = 0.0;
        }
        t
    };
    assert_eq!(strip(&back), strip(&ta));
    for (x, y) in ta.incidents.iter().zip(&back.incidents) {
        assert!((x.difficulty - y.difficulty).abs() < 1e-12);
    }
    let (steps, sim) = declare_each(&a, &ta, |inc| inc.truth);
    let calls = calls_from_sim(&sim);
    let v1 = score_stream(&ta, &steps, &calls).expect("scores");
    let v2 = score_stream(&back, &steps, &calls).expect("scores");
    assert_eq!(v1, v2);
    let round: StreamVerdict = serde_json::from_str(&serde_json::to_string(&v1).unwrap()).unwrap();
    assert_eq!(round, v1);
}
