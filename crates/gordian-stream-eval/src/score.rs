//! Scoring a recorded stream trajectory against hidden truth.
//!
//! The rules are in `RULES.md` next to this crate's `Cargo.toml`, one numbered row each; the
//! comments below cite them as S1, S2, and so on. [`score_stream`] is a pure function of its
//! arguments: it reads no clock, draws no randomness, and keeps no state between calls.
//!
//! # How a trajectory is read
//!
//! A step counts toward the score only if the stream accepted it (its outcome is not
//! `Refused`, S3). What an accepted `Declare` says about which incident it concerns is read from
//! its anchor through the truth's labels (S2); what an accepted `Escalate` was about is read the
//! same way from its focus. The scorer does not run the stream: it does not check that a probe's
//! result is what the stream would have returned, or that a refusal was justified. It does
//! check that the recording is internally coherent (S27 to S36), because an incoherent one means
//! the harness recorded something the stream could not have produced.

use crate::error::StreamEvalError;
use crate::step::{CallSummary, StreamStep};
use crate::verdict::{IncidentVerdict, StreamTotals, StreamVerdict};
use gordian_core::Instant;
use gordian_stream::oracle::{ObsLabel, StreamTruth};
use gordian_stream::{ObsId, Question, StreamAction, StreamOutcome, Tier};

/// What the trajectory did about one incident.
#[derive(Debug, Clone, Default)]
struct Acc {
    correct: u32,
    wrong: u32,
    first_correct: Option<Instant>,
    escalations: u32,
    informed: u32,
    correct_calls: u32,
}

/// Check the truth is coherent (S27, S28, S29), in that order.
fn check_truth(truth: &StreamTruth) -> Result<(), StreamEvalError> {
    for (index, inc) in truth.incidents.iter().enumerate() {
        if inc.id as usize != index {
            return Err(StreamEvalError::IncidentIdMismatch { index, id: inc.id });
        }
    }
    for inc in &truth.incidents {
        let coherent = match inc.tier {
            Tier::Decoy => inc.truth.is_none() && inc.deadline_ns.is_none() && !inc.critical,
            Tier::Plain | Tier::Hard => inc.truth.is_some() && inc.deadline_ns.is_some(),
        };
        if !coherent {
            return Err(StreamEvalError::TierContradictsTruth { incident: inc.id });
        }
    }
    for (i, label) in truth.labels.iter().enumerate() {
        if let ObsLabel::Incident { id, .. } = label
            && *id as usize >= truth.incidents.len()
        {
            return Err(StreamEvalError::LabelOfUnknownIncident {
                obs: ObsId(i as u32),
                incident: *id,
            });
        }
    }
    Ok(())
}

/// An accepted step names `obs`: it must exist (S33) and its incident must have begun (S34).
fn check_named(
    truth: &StreamTruth,
    index: usize,
    at: Instant,
    obs: ObsId,
) -> Result<(), StreamEvalError> {
    if obs.0 as usize >= truth.labels.len() {
        return Err(StreamEvalError::UnknownObservation { index, obs });
    }
    if let Some(id) = truth.incident_of(obs)
        && at.0 < truth.incidents[id as usize].onset_ns
    {
        return Err(StreamEvalError::ObservationNotYetEmitted { index, obs });
    }
    Ok(())
}

/// Score `trajectory` and `calls` against `truth`.
///
/// Steps are read in order. Before any step the truth is checked (S27 to S29). For each step the
/// first failing check decides the error: time going backwards (S30), then the outcome (S31),
/// then the end of the stream (S32), then the observation named (S33, S34), then the call record
/// (S35). After the last step the call count is checked (S36). Equal instants are allowed.
pub fn score_stream(
    truth: &StreamTruth,
    trajectory: &[StreamStep],
    calls: &[CallSummary],
) -> Result<StreamVerdict, StreamEvalError> {
    check_truth(truth)?;

    let mut acc = vec![Acc::default(); truth.incidents.len()];
    let mut totals = StreamTotals::default();
    let mut previous: Option<Instant> = None;
    let mut accepted_calls = 0usize;

    for (index, step) in trajectory.iter().enumerate() {
        if previous.is_some_and(|p| step.at < p) {
            return Err(StreamEvalError::TimeWentBackwards { index });
        }
        previous = Some(step.at);

        match (&step.action, &step.outcome) {
            // A refusal changed nothing: not a declaration, an escalation or a cost (S3).
            (_, StreamOutcome::Refused(_)) => continue,
            (StreamAction::Probe { .. }, StreamOutcome::Probed { .. })
            | (StreamAction::Escalate { .. }, StreamOutcome::Escalated { .. })
            | (StreamAction::Declare { .. }, StreamOutcome::Declared { .. }) => {}
            _ => return Err(StreamEvalError::OutcomeMismatch { index }),
        }
        if step.at.0 > truth.duration_ns {
            return Err(StreamEvalError::ActionAfterEnd { index });
        }

        match (&step.action, &step.outcome) {
            (StreamAction::Declare { anchor, diagnosis }, _) => {
                check_named(truth, index, step.at, *anchor)?;
                match truth.incident_of(*anchor) {
                    // S2, S4, S5, S11, S14, S15.
                    Some(id) => {
                        let a = &mut acc[id as usize];
                        if *diagnosis == truth.incidents[id as usize].truth {
                            a.correct += 1;
                            a.first_correct.get_or_insert(step.at);
                        } else {
                            a.wrong += 1;
                        }
                    }
                    // S18: a claim about nothing is a false alarm; a dismissal of it is nothing.
                    None => {
                        if diagnosis.is_some() {
                            totals.false_alarms += 1;
                            totals.false_alarms_on_background += 1;
                        }
                    }
                }
            }
            (
                StreamAction::Escalate { context, question },
                StreamOutcome::Escalated {
                    call,
                    ready_at,
                    cost,
                },
            ) => {
                let Question::Diagnose { focus } = question;
                check_named(truth, index, step.at, *focus)?;
                // S35: the call summary at this position, and agreeing with this step.
                let mismatch = StreamEvalError::CallRecordMismatch { index, call: *call };
                if *call as usize != accepted_calls {
                    return Err(mismatch);
                }
                let Some(summary) = calls.get(accepted_calls) else {
                    return Err(mismatch);
                };
                if summary.at != step.at
                    || summary.ready_at != *ready_at
                    || u64::from(summary.refs) != context.len() as u64
                    || summary.focus.is_some_and(|f| f != *focus)
                {
                    return Err(mismatch);
                }
                accepted_calls += 1;

                // S26.
                totals.reasoner.calls += 1;
                totals.reasoner.refs = totals.reasoner.refs.saturating_add(context.len() as u64);
                totals.reasoner.tokens = totals.reasoner.tokens.saturating_add(cost.tokens);
                totals.reasoner.modelled_ns =
                    totals.reasoner.modelled_ns.saturating_add(cost.modelled_ns);
                // S25.
                if summary.informed {
                    totals.escalations.informed += 1;
                }
                if summary.correct {
                    totals.escalations.correct += 1;
                }
                // S13, S23.
                match truth.incident_of(*focus) {
                    Some(id) => {
                        let a = &mut acc[id as usize];
                        a.escalations += 1;
                        a.informed += u32::from(summary.informed);
                        a.correct_calls += u32::from(summary.correct);
                        match truth.incidents[id as usize].tier {
                            Tier::Hard => totals.escalations.needed += 1,
                            Tier::Plain | Tier::Decoy => totals.escalations.unneeded += 1,
                        }
                    }
                    None => totals.escalations.background += 1,
                }
            }
            _ => {}
        }
    }

    // S36.
    if accepted_calls != calls.len() {
        return Err(StreamEvalError::CallCountMismatch {
            escalations: accepted_calls,
            calls: calls.len(),
        });
    }

    let mut per_incident = Vec::with_capacity(truth.incidents.len());
    for (inc, a) in truth.incidents.iter().zip(&acc) {
        let scored = inc.tier != Tier::Decoy;
        // S6, S7: the earliest correct declaration decides, so it is on time or none is. A
        // decoy has no deadline, so it is never "by deadline".
        let correct_by_deadline = inc
            .deadline_ns
            .zip(a.first_correct)
            .is_some_and(|(deadline, at)| at.0 <= deadline);
        // S9, S10.
        let missed = scored && !correct_by_deadline;
        let critical_miss = inc.critical && missed;

        // S19 to S21, S22, S24.
        totals.critical_incidents += u32::from(inc.critical);
        match inc.tier {
            Tier::Plain => {
                totals.incidents.plain += 1;
                totals.correct.plain += u32::from(correct_by_deadline);
                totals.missed.plain += u32::from(missed);
                totals.critical_missed.plain += u32::from(critical_miss);
                totals.wrong_declarations += a.wrong;
                totals.escalations.other_incidents_escalated += u32::from(a.escalations > 0);
            }
            Tier::Hard => {
                totals.incidents.hard += 1;
                totals.correct.hard += u32::from(correct_by_deadline);
                totals.missed.hard += u32::from(missed);
                totals.critical_missed.hard += u32::from(critical_miss);
                totals.wrong_declarations += a.wrong;
                totals.escalations.hard_incidents_escalated += u32::from(a.escalations > 0);
            }
            Tier::Decoy => {
                totals.incidents.decoy += 1;
                totals.decoys_dismissed += u32::from(a.correct > 0);
                totals.decoys_alarmed += u32::from(a.wrong > 0);
                totals.decoys_silent += u32::from(a.correct + a.wrong == 0);
                totals.false_alarms += a.wrong;
                totals.escalations.other_incidents_escalated += u32::from(a.escalations > 0);
            }
        }

        per_incident.push(IncidentVerdict {
            id: inc.id,
            tier: inc.tier,
            critical: inc.critical,
            correct_declarations: a.correct,
            wrong_declarations: a.wrong,
            first_correct_at: a.first_correct,
            // S8.
            time_to_first_correct_ns: a.first_correct.map(|at| at.0.saturating_sub(inc.onset_ns)),
            correct_by_deadline,
            missed,
            critical_miss,
            escalations: a.escalations,
            informed_escalations: a.informed,
            correct_escalations: a.correct_calls,
        });
    }

    Ok(StreamVerdict {
        per_incident,
        totals,
    })
}
