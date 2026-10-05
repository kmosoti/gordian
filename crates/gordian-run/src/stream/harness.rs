//! The stream loop: play one stream segment with one arm and score it.
//!
//! [`run_segment`] is to a stream what [`crate::harness::run_episode`] is to an episode, and
//! reuses its accounting: a fresh [`Budget`](gordian_core::Budget), [`Bill`] and [`Ledger`] per
//! segment; every component and rule call charged before it runs, timed with the monotonic clock
//! at the boundary and counted in declared units ([`super::meter`]); timings recorded as
//! `Measurement` entries from `harness/timer`, never as `Accounting`; observations recorded as
//! measurements and the reasoner's answers as hypotheses; the privileged path for the oracle arm.
//! This is, with `oracle.rs`, the only file in the crate that generates a stream or builds its
//! truth (a test checks the source), because the episode harness's structural test allows
//! generating only in a file named `harness.rs`.
//!
//! # One step
//!
//! | # | What | Ledger |
//! |---|---|---|
//! | 1 | `observe_until(now)`, plus the probe results that are ready, delivered to the arm | one `Measurement` per observation and per probe result (`harness/sensor`); one `Hypothesis` per reasoner answer (`harness/reasoner`) |
//! | 2 | charge the arm's declared bookkeeping cost under `Phase::Scheduling`; a refusal means the arm may only take in what was delivered and declare answers | `Accounting` |
//! | 3 | `arm.step`: the arm runs components and the shared rule through the meter and returns actions | `Accounting`, `harness/timer` and the components' `Hypothesis` entries, from the meter |
//! | 4 | validate, charge and apply each action in order (below) | `Decision`, `Accounting`, `Outcome` |
//! | 5 | the step lasts at least `step_ns` of logical time | |
//!
//! The loop stops when the next step would start at or after the stream's duration. Then the arm
//! gets its final call at the duration: what was left is delivered, and every anomaly still
//! undecided gets the shared rule's final call. A segment ends at `horizon` unless the step cap
//! (a runaway guard, no final call) stops it first.
//!
//! # Paying for the reasoner
//!
//! An escalation is paid for **before the call exists**. In order: (1) the harness validates the
//! context against what the arm has been delivered, using only public information; a malformed
//! call is refused and nothing is charged. (2) The bill's dry run: if the call does not fit in
//! the segment's token limit it is refused, the refusal is recorded, and neither the bill nor the
//! stream's own budget has moved. (3) The call's tokens are charged to the bill, a recorded
//! `Accounting` entry under `Phase::Communication`. (4) Only then is the call made. If the stream
//! refuses a call the bill has already paid, that is a harness defect
//! ([`StreamHarnessError::BillDisagreement`]), never a result. The stream's budget is derived from
//! the same limits, so the two cannot disagree unless the harness is wrong.
//!
//! Declarations are free. Probes are charged as the episode harness charges them: dry run, then
//! the world, then the bill, in one action.
//!
//! # The truth stays aside
//!
//! The stream is generated and its [`StreamTruth`] built in [`run_segment`], immediately, as the
//! episode harness builds `Truth`. The stream moves into the simulator, the truth is held in a
//! local that nothing hands to an arm, and the reasoner's call records are read once at the end.
//! Both go to the scorer after the last step and nowhere else. The privileged arm is built from
//! the truth by [`OracleFactory::build`], inside [`run_segment_privileged`].

use super::arms::{Applied, Proposed, Source, StepInput, StreamPolicy};
use super::manifest::{Exchange, StreamLimits};
use super::meter::{Meter, Totals};
use super::privileged::OracleFactory;
use super::score::{CallSummary, StreamEvalError, StreamScorer, StreamStep, StreamVerdict};
use crate::harness::{
    Charged, EpisodeOps, HarnessError, Measured, affordable, append, charge, elapsed_ns, payload,
    record_timing, timed,
};
use gordian_core::{Bill, EntryKind, Instant, Ledger, ManualClock, Phase, Resource};
use gordian_core::{Charge, EntryId};
use gordian_stream::{
    ObsId, ObsRef, Question, StreamAction, StreamEvent, StreamOutcome, StreamParams, StreamPublic,
    StreamSimulator, generate,
};
use gordian_stream_reveal::{StreamTruth, call_records, truth_of};
use gordian_world::Observation;
use gordian_world::physics::probe_cost;
use gordian_world::step::CostSummary;
use serde_json::json;
use std::collections::BTreeSet;
use std::fmt;
use std::time::Instant as Wall;

/// Why a segment stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStop {
    /// The next step would have started at or after the stream's duration, and the arm had its
    /// final call.
    Horizon,
    /// The step cap was reached: a runaway guard, with no final call.
    StepCap,
}

impl StreamStop {
    /// The value written to the `stop_reason` column of `results.csv`.
    pub fn as_str(self) -> &'static str {
        match self {
            StreamStop::Horizon => "horizon",
            StreamStop::StepCap => "step_cap",
        }
    }
}

/// A defect in the stream harness or in how it was called. Never a result: a run that hits one
/// fails.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamHarnessError {
    /// A defect the episode harness's accounting layer reports (ledger, payload).
    Harness(HarnessError),
    /// The scorer refused the trajectory.
    Eval(StreamEvalError),
    /// The limits are unusable.
    InvalidLimits(String),
    /// The stream's own budget differs from what the limits require.
    BudgetMismatch(String),
    /// The bill and the stream disagree about an action's cost or acceptance.
    BillDisagreement(String),
}

impl fmt::Display for StreamHarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamHarnessError::Harness(e) => write!(f, "{e}"),
            StreamHarnessError::Eval(e) => write!(f, "scorer refused the trajectory: {e}"),
            StreamHarnessError::InvalidLimits(why) => write!(f, "invalid limits: {why}"),
            StreamHarnessError::BudgetMismatch(why) => {
                write!(f, "stream budget differs from the limits: {why}")
            }
            StreamHarnessError::BillDisagreement(why) => {
                write!(f, "bill and stream disagree: {why}")
            }
        }
    }
}

impl std::error::Error for StreamHarnessError {}

impl From<HarnessError> for StreamHarnessError {
    fn from(e: HarnessError) -> Self {
        StreamHarnessError::Harness(e)
    }
}

impl From<StreamEvalError> for StreamHarnessError {
    fn from(e: StreamEvalError) -> Self {
        StreamHarnessError::Eval(e)
    }
}

/// Counts the harness keeps beside the scorer's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SegmentCounts {
    /// Declarations the stream recorded that came from the cheap rung.
    pub cheap_declarations: u32,
    /// Declarations the stream recorded that were a reasoner's answer.
    pub reasoner_declarations: u32,
    /// Escalations refused: the segment's token limit, or a malformed context. Nothing charged.
    pub escalations_refused: u32,
    /// Probes the bill refused.
    pub probes_refused: u32,
    /// Escalations the stream accepted whose answer was due after the stream's end.
    pub calls_unanswered: u32,
}

/// Everything one segment produced.
#[derive(Debug, Clone)]
pub struct SegmentRecord {
    /// The stream's seed.
    pub seed: u64,
    /// The arm's id.
    pub arm_id: String,
    /// What the arm is.
    pub role: super::arms::ArmRole,
    /// The scorer's verdict.
    pub verdict: StreamVerdict,
    /// The live bill at the end of the segment.
    pub bill: Bill,
    /// The ledger: observations, answers, accounting, decisions, outcomes, timings.
    pub ledger: Ledger,
    /// The actions the stream answered, in order.
    pub trajectory: Vec<StreamStep>,
    /// Steps the loop took (the final call is not a step).
    pub steps: u32,
    /// Why the loop stopped.
    pub stop: StreamStop,
    /// Counts the harness keeps.
    pub counts: SegmentCounts,
    /// Anomalies the arm's cheap rung noticed.
    pub anomalies_noticed: u32,
    /// Passive observations delivered.
    pub observations: u32,
    /// Component calls charged and run.
    pub components_run: u32,
    /// Component calls the bill refused.
    pub components_skipped: u32,
    /// Rule calls the bill refused.
    pub rule_skipped: u32,
    /// Wall-clock durations.
    pub measured: Measured,
    /// Counted operations, and with the weights the modelled cost. Deterministic.
    pub ops: EpisodeOps,
    /// The reasoner's tokens converted to modelled nanoseconds at the manifest's exchange rate.
    pub reasoner_cost_ns: u64,
    /// What the arm was told at the start. Public by construction.
    pub public: StreamPublic,
    /// The stream's passive observations in full, including what was never delivered. Public by
    /// construction (`Stream::events`).
    pub public_stream: Vec<(Instant, Observation)>,
}

impl SegmentRecord {
    /// Modelled substrate and rule cost: components plus the shared rule's counted work.
    pub fn substrate_ns(&self) -> u64 {
        self.ops
            .modelled_component_ns()
            .saturating_add(self.ops.modelled_sched_ns())
    }

    /// Total modelled cost: substrate and rule cost plus the reasoner's, converted.
    pub fn total_cost_ns(&self) -> u64 {
        self.substrate_ns().saturating_add(self.reasoner_cost_ns)
    }
}

/// Where the segment's arm comes from.
enum ArmSource<'a> {
    /// The caller builds it from the stream's public information; it is never shown the truth.
    Given(&'a dyn Fn(&StreamPublic) -> Box<dyn StreamPolicy>),
    /// Built from the truth once the stream exists.
    Privileged(&'a OracleFactory),
}

/// Play one segment and score it.
///
/// `make_arm` builds the arm from the stream's public information; it is the only thing the arm
/// is built from. `params` is the stream (its seed names the segment) and must carry the budget
/// `limits` requires.
///
/// # Errors
///
/// A [`StreamHarnessError`] is a defect in the harness or in the call, never a result.
pub fn run_segment(
    params: &StreamParams,
    make_arm: &dyn Fn(&StreamPublic) -> Box<dyn StreamPolicy>,
    limits: &StreamLimits,
    exchange: &Exchange,
    scorer: &dyn StreamScorer,
) -> Result<SegmentRecord, StreamHarnessError> {
    play(params, ArmSource::Given(make_arm), limits, exchange, scorer)
}

/// [`run_segment`] for the privileged arm: it is built here, from the segment's truth, by
/// `factory`. The only way in the crate for an arm to be built from the truth.
///
/// # Errors
///
/// As [`run_segment`].
pub fn run_segment_privileged(
    params: &StreamParams,
    factory: &OracleFactory,
    limits: &StreamLimits,
    exchange: &Exchange,
    scorer: &dyn StreamScorer,
) -> Result<SegmentRecord, StreamHarnessError> {
    play(
        params,
        ArmSource::Privileged(factory),
        limits,
        exchange,
        scorer,
    )
}

/// Everything the loop mutates, apart from the arm.
struct State {
    sim: StreamSimulator,
    bill: Bill,
    ledger: Ledger,
    clock: ManualClock,
    totals: Totals,
    trajectory: Vec<StreamStep>,
    ready_results: Vec<(u32, Instant, Observation)>,
    delivered: u32,
    probes_accepted: u32,
    last_apply: Instant,
    counts: SegmentCounts,
    bookkeeping_ns: u64,
    duration: Instant,
}

/// Deliver what has arrived by `until` and record it. Returns the events and the ready probe
/// results.
fn sense(
    st: &mut State,
    until: Instant,
) -> Result<(Vec<StreamEvent>, Vec<(u32, Instant, Observation)>), StreamHarnessError> {
    let events = st.sim.observe_until(until);
    let stamp = st.clock.now();
    for event in &events {
        match event {
            StreamEvent::Observed { id, at, obs } => {
                st.delivered = st.delivered.max(id.0.saturating_add(1));
                let body = payload(&json!({ "id": id.0, "at_ns": at.0, "observation": obs }))?;
                append(
                    &mut st.ledger,
                    stamp,
                    event.entry_kind(),
                    "harness/sensor".to_owned(),
                    Vec::new(),
                    body,
                )?;
            }
            StreamEvent::Answered { call, at, answer } => {
                let body = payload(&json!({
                    "call": call,
                    "at_ns": at.0,
                    "focus": answer.focus.0,
                    "diagnosis": answer.diagnosis,
                }))?;
                append(
                    &mut st.ledger,
                    stamp,
                    event.entry_kind(),
                    "harness/reasoner".to_owned(),
                    Vec::new(),
                    body,
                )?;
            }
        }
    }
    let results = std::mem::take(&mut st.ready_results);
    let (ready, waiting): (Vec<_>, Vec<_>) =
        results.into_iter().partition(|(_, at, _)| *at <= until);
    st.ready_results = waiting;
    for (index, at, obs) in &ready {
        let body = payload(&json!({ "probe": index, "at_ns": at.0, "observation": obs }))?;
        append(
            &mut st.ledger,
            stamp,
            EntryKind::Measurement,
            "harness/sensor".to_owned(),
            Vec::new(),
            body,
        )?;
    }
    Ok((events, ready))
}

/// Why a proposed escalation is not carried out, judged from public information alone, or `None`
/// when it is well formed. Mirrors the stream's own checks, so that a call the harness has paid
/// for is never refused by the stream.
fn escalation_fault(
    context: &[ObsRef],
    focus: ObsId,
    delivered: u32,
    probes: u32,
    max_context: u32,
) -> Option<&'static str> {
    let held = |r: &ObsRef| match r {
        ObsRef::Passive(id) => id.0 < delivered,
        ObsRef::Probe(n) => *n < probes,
    };
    if focus.0 >= delivered {
        return Some("focus not held");
    }
    if context.len() as u64 > u64::from(max_context) {
        return Some("context too large");
    }
    let mut seen = BTreeSet::new();
    for r in context {
        if !held(r) {
            return Some("reference not held");
        }
        if !seen.insert(*r) {
            return Some("duplicate reference");
        }
    }
    None
}

fn refuse(
    st: &mut State,
    producer: &str,
    decision: EntryId,
    why: &str,
) -> Result<(), StreamHarnessError> {
    let at = st.clock.now();
    let body = payload(&json!({ "refused": why }))?;
    append(
        &mut st.ledger,
        at,
        EntryKind::Outcome,
        format!("harness/{producer}"),
        vec![decision],
        body,
    )?;
    Ok(())
}

/// Validate, charge and apply one proposed action.
fn apply_one(
    st: &mut State,
    public: &StreamPublic,
    producer: &str,
    proposed: Proposed,
) -> Result<Applied, StreamHarnessError> {
    let at = st.clock.now();
    let sim_now = at.min(st.duration).max(st.last_apply);
    let decision = append(
        &mut st.ledger,
        at,
        EntryKind::Decision,
        producer.to_owned(),
        Vec::new(),
        payload(&json!({ "source": proposed.source.as_str(), "action": &proposed.action }))?,
    )?;
    let tag = proposed.tag;
    let not_applied = Applied {
        tag,
        accepted: false,
    };
    match &proposed.action {
        StreamAction::Probe { kind, target } => {
            let cost = probe_cost(*kind);
            if !affordable(st.bill.budget(), &cost) {
                // The bill is the authority and is never looser than the stream, so the stream
                // is not asked. The refusal is recorded through the bill.
                match charge(
                    &mut st.bill,
                    &mut st.ledger,
                    at,
                    "harness/sensor",
                    Phase::Sensing,
                    &cost,
                )? {
                    Charged::Refused(_) => {}
                    Charged::Accepted(_) => {
                        return Err(StreamHarnessError::BillDisagreement(
                            "a probe charge that a dry run refused was accepted".to_owned(),
                        ));
                    }
                }
                refuse(st, "bill", decision, "bill")?;
                st.counts.probes_refused += 1;
                return Ok(not_applied);
            }
            let outcome = st.sim.apply(proposed.action.clone(), sim_now);
            st.last_apply = sim_now;
            append(
                &mut st.ledger,
                at,
                EntryKind::Outcome,
                "harness/stream".to_owned(),
                vec![decision],
                payload(&outcome)?,
            )?;
            st.trajectory.push(StreamStep {
                at: sim_now,
                action: proposed.action.clone(),
                outcome: outcome.clone(),
            });
            match outcome {
                StreamOutcome::Probed {
                    probe,
                    observation,
                    ready_at,
                    cost: world_cost,
                } => {
                    if world_cost != CostSummary::from_charges(&cost) {
                        return Err(StreamHarnessError::BillDisagreement(format!(
                            "the stream charged {world_cost:?} for a probe declared as {cost:?}"
                        )));
                    }
                    match charge(
                        &mut st.bill,
                        &mut st.ledger,
                        at,
                        "harness/sensor",
                        Phase::Sensing,
                        &cost,
                    )? {
                        Charged::Accepted(_) => {}
                        Charged::Refused(_) => {
                            return Err(StreamHarnessError::BillDisagreement(
                                "the bill refused a probe charge a dry run accepted".to_owned(),
                            ));
                        }
                    }
                    st.probes_accepted = st.probes_accepted.max(probe + 1);
                    st.ready_results.push((probe, ready_at, observation));
                    let _ = target;
                    Ok(Applied {
                        tag,
                        accepted: true,
                    })
                }
                StreamOutcome::Refused(_) => Ok(not_applied),
                other => Err(StreamHarnessError::BillDisagreement(format!(
                    "a probe was answered with {other:?}"
                ))),
            }
        }
        StreamAction::Escalate { context, question } => {
            let Question::Diagnose { focus } = question;
            if let Some(why) = escalation_fault(
                context,
                *focus,
                st.delivered,
                st.probes_accepted,
                public.max_context,
            ) {
                refuse(st, "validate", decision, why)?;
                st.counts.escalations_refused += 1;
                return Ok(not_applied);
            }
            let cost = public.reasoner_cost.cost(context.len());
            let charges = [Charge::new(Resource::Communication, cost.tokens)];
            if !affordable(st.bill.budget(), &charges) {
                match charge(
                    &mut st.bill,
                    &mut st.ledger,
                    at,
                    "harness/reasoner",
                    Phase::Communication,
                    &charges,
                )? {
                    Charged::Refused(_) => {}
                    Charged::Accepted(_) => {
                        return Err(StreamHarnessError::BillDisagreement(
                            "a reasoner charge that a dry run refused was accepted".to_owned(),
                        ));
                    }
                }
                refuse(st, "bill", decision, "bill")?;
                st.counts.escalations_refused += 1;
                return Ok(not_applied);
            }
            // Paid first. The call below is made only once the bill has accepted its cost and
            // the accounting entry is in the ledger.
            match charge(
                &mut st.bill,
                &mut st.ledger,
                at,
                "harness/reasoner",
                Phase::Communication,
                &charges,
            )? {
                Charged::Accepted(_) => {}
                Charged::Refused(_) => {
                    return Err(StreamHarnessError::BillDisagreement(
                        "the bill refused a reasoner charge a dry run accepted".to_owned(),
                    ));
                }
            }
            let outcome = st.sim.apply(proposed.action.clone(), sim_now);
            st.last_apply = sim_now;
            append(
                &mut st.ledger,
                at,
                EntryKind::Outcome,
                "harness/stream".to_owned(),
                vec![decision],
                payload(&outcome)?,
            )?;
            st.trajectory.push(StreamStep {
                at: sim_now,
                action: proposed.action.clone(),
                outcome: outcome.clone(),
            });
            match outcome {
                StreamOutcome::Escalated {
                    cost: world_cost,
                    ready_at,
                    ..
                } => {
                    if world_cost != cost {
                        return Err(StreamHarnessError::BillDisagreement(format!(
                            "the stream charged {world_cost:?} for a call declared as {cost:?}"
                        )));
                    }
                    if ready_at > st.duration {
                        st.counts.calls_unanswered += 1;
                    }
                    Ok(Applied {
                        tag,
                        accepted: true,
                    })
                }
                other => Err(StreamHarnessError::BillDisagreement(format!(
                    "the stream refused a call the bill had already paid for: {other:?}"
                ))),
            }
        }
        StreamAction::Declare { anchor, .. } => {
            if anchor.0 >= st.delivered {
                refuse(st, "validate", decision, "anchor not held")?;
                return Ok(not_applied);
            }
            let outcome = st.sim.apply(proposed.action.clone(), sim_now);
            st.last_apply = sim_now;
            append(
                &mut st.ledger,
                at,
                EntryKind::Outcome,
                "harness/stream".to_owned(),
                vec![decision],
                payload(&outcome)?,
            )?;
            st.trajectory.push(StreamStep {
                at: sim_now,
                action: proposed.action.clone(),
                outcome: outcome.clone(),
            });
            match outcome {
                StreamOutcome::Declared { .. } => {
                    match proposed.source {
                        Source::Reasoner => st.counts.reasoner_declarations += 1,
                        _ => st.counts.cheap_declarations += 1,
                    }
                    Ok(Applied {
                        tag,
                        accepted: true,
                    })
                }
                StreamOutcome::Refused(_) => Ok(not_applied),
                other => Err(StreamHarnessError::BillDisagreement(format!(
                    "a declaration was answered with {other:?}"
                ))),
            }
        }
    }
}

fn play(
    params: &StreamParams,
    source: ArmSource<'_>,
    limits: &StreamLimits,
    exchange: &Exchange,
    scorer: &dyn StreamScorer,
) -> Result<SegmentRecord, StreamHarnessError> {
    let started = Wall::now();
    limits
        .validate()
        .map_err(StreamHarnessError::InvalidLimits)?;

    // The stream exists in this block only. Its truth is built from it immediately, what the
    // harness needs from it is copied out, and then it moves into the simulator. After this
    // block nothing but the simulator holds it, and the truth is held in a local that no arm is
    // ever handed.
    let stream = generate(params);
    let truth: StreamTruth = truth_of(&stream);
    let public = stream.public_info();
    let public_stream = stream.events().to_vec();
    let seed = params.seed;
    let expected = limits.world_budget(&public.reasoner_cost);
    if public.budget != expected {
        return Err(StreamHarnessError::BudgetMismatch(format!(
            "the stream carries {:?}, the limits require {expected:?}",
            public.budget
        )));
    }
    let duration = Instant(public.duration_ns);
    let sim = StreamSimulator::new(stream);
    let mut policy: Box<dyn StreamPolicy> = match source {
        ArmSource::Given(make) => make(&public),
        ArmSource::Privileged(factory) => factory.build(&truth, &public),
    };
    let producer = format!("policy/{}", policy.id().0);
    let role = policy.role();
    let arm_id = policy.id().0;

    let ids = super::arms::rung::cheap_component_ids();
    let mut st = State {
        sim,
        bill: Bill::new(limits.budget()),
        ledger: Ledger::new(),
        clock: ManualClock::new(),
        totals: Totals::new(&ids),
        trajectory: Vec::new(),
        ready_results: Vec::new(),
        delivered: 0,
        probes_accepted: 0,
        last_apply: Instant::ZERO,
        counts: SegmentCounts::default(),
        bookkeeping_ns: 0,
        duration,
    };

    let mut applied: Vec<Applied> = Vec::new();
    let mut steps = 0u32;
    let mut stop = StreamStop::Horizon;
    loop {
        let step_start = st.clock.now();
        if step_start >= duration {
            break;
        }
        if steps >= limits.max_steps {
            stop = StreamStop::StepCap;
            break;
        }
        steps += 1;

        let (events, probe_results) = sense(&mut st, step_start)?;
        let input = StepInput {
            now: step_start,
            events: &events,
            probe_results: &probe_results,
            applied: &applied,
            public: &public,
        };

        // The arm's declared bookkeeping, charged before it works.
        let (cost, cost_ns) = timed(|| policy.declared_step_cost(&input));
        let (accounting, work_allowed) = match charge(
            &mut st.bill,
            &mut st.ledger,
            st.clock.now(),
            &producer,
            Phase::Scheduling,
            &cost,
        )? {
            Charged::Accepted(entry) => (entry, true),
            Charged::Refused(entry) => (entry, false),
        };

        let component_before = st.totals.component_ns;
        let rule_before = st.totals.rule_ns;
        let (proposals, step_ns) = {
            let mut meter = Meter::new(
                &mut st.bill,
                &mut st.ledger,
                &mut st.clock,
                &mut st.totals,
                &producer,
            );
            let (proposals, ns) = timed(|| policy.step(&input, work_allowed, &mut meter));
            if let Some(error) = meter.take_error() {
                return Err(error.into());
            }
            (proposals, ns)
        };
        let component_ns = st.totals.component_ns - component_before;
        let rule_ns = st.totals.rule_ns - rule_before;
        let bookkeeping = cost_ns
            .saturating_add(step_ns)
            .saturating_sub(component_ns)
            .saturating_sub(rule_ns);
        st.bookkeeping_ns = st.bookkeeping_ns.saturating_add(bookkeeping);
        record_timing(
            &mut st.ledger,
            st.clock.now(),
            "bookkeeping",
            None,
            bookkeeping,
            &[],
            vec![accounting],
        )?;

        applied.clear();
        for proposed in proposals {
            let result = apply_one(&mut st, &public, &producer, proposed)?;
            applied.push(result);
        }

        // A step takes at least the quantum; idle time is not billed.
        let step_end = Instant(step_start.0.saturating_add(limits.step_ns));
        if let Some(rest) = step_end.since(st.clock.now()) {
            st.clock.advance(rest);
        }
    }

    // The final call, at the stream's end. A step-capped segment does not get one: the cap is a
    // runaway guard and says nothing about the arm's means or the stream's time.
    if stop == StreamStop::Horizon {
        if let Some(rest) = duration.since(st.clock.now()) {
            st.clock.advance(rest);
        }
        let (events, probe_results) = sense(&mut st, duration)?;
        let input = StepInput {
            now: duration,
            events: &events,
            probe_results: &probe_results,
            applied: &applied,
            public: &public,
        };
        let component_before = st.totals.component_ns;
        let rule_before = st.totals.rule_ns;
        let (proposals, ns) = {
            let mut meter = Meter::new(
                &mut st.bill,
                &mut st.ledger,
                &mut st.clock,
                &mut st.totals,
                &producer,
            );
            let (proposals, ns) = timed(|| policy.finish(&input, &mut meter));
            if let Some(error) = meter.take_error() {
                return Err(error.into());
            }
            (proposals, ns)
        };
        let bookkeeping = ns
            .saturating_sub(st.totals.component_ns - component_before)
            .saturating_sub(st.totals.rule_ns - rule_before);
        st.bookkeeping_ns = st.bookkeeping_ns.saturating_add(bookkeeping);
        record_timing(
            &mut st.ledger,
            st.clock.now(),
            "bookkeeping",
            None,
            bookkeeping,
            &[],
            Vec::new(),
        )?;
        for proposed in proposals {
            apply_one(&mut st, &public, &producer, proposed)?;
        }
    }

    let calls: Vec<CallSummary> = call_records(&st.sim);
    let verdict = scorer.score_stream(&truth, &st.trajectory, &calls)?;
    let State {
        bill,
        ledger,
        totals,
        trajectory,
        delivered,
        counts,
        bookkeeping_ns,
        ..
    } = st;
    let total = elapsed_ns(started);
    let mut measured = Measured {
        component_ns: totals.component_ns,
        sched_ns: totals.rule_ns.saturating_add(bookkeeping_ns),
        harness_ns: 0,
    };
    measured.harness_ns = total
        .saturating_sub(measured.component_ns)
        .saturating_sub(measured.sched_ns);
    let report = policy.report();
    Ok(SegmentRecord {
        seed,
        arm_id,
        role,
        verdict,
        bill,
        ledger,
        trajectory,
        steps,
        stop,
        counts,
        anomalies_noticed: report.anomalies_noticed,
        observations: delivered,
        components_run: totals.components_run,
        components_skipped: totals.components_skipped,
        rule_skipped: totals.rule_skipped,
        measured,
        ops: EpisodeOps {
            components: totals.components,
            sched: totals.sched,
        },
        reasoner_cost_ns: verdict
            .reasoner_tokens
            .saturating_mul(exchange.reasoner_ns_per_token),
        public,
        public_stream,
    })
}
