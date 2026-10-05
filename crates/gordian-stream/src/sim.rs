//! The policy-facing interface: a [`StreamSimulator`] that delivers observations and applies
//! actions, mirroring the first world's `Simulator`.
//!
//! A policy never holds a [`Stream`]. It reads passive observations and the reasoner's answers
//! from [`StreamSimulator::observe_until`] and acts through [`StreamSimulator::apply`].
//!
//! # What a policy can do
//!
//! | action | effect | cost |
//! |---|---|---|
//! | `Probe { kind, target }` | the first world's probe, against whichever live incident owns `target` | the first world's probe cost |
//! | `Escalate { context, question }` | asks the simulated reasoner; the answer arrives through `observe_until` after the declared latency | `ReasonerCostSpec::cost(context.len())`, charged before the answer is produced |
//! | `Declare { anchor, diagnosis }` | says which incident `anchor` belongs to and what is wrong with it; `None` says it is not an incident | free |
//!
//! # Why a declaration is anchored on an observation
//!
//! A declaration must name an incident, and a policy cannot be given a handle to one: a handle
//! issued when an incident starts would tell the policy that something is starting, which is the
//! relevance judgement the world exists to make costly, and it would do so for decoys too. A
//! time window would be ambiguous whenever two incidents overlap and could be widened to cover
//! everything. An observation the policy holds is something it can only have obtained by
//! observing, belongs to exactly one incident or to none, and is what the policy already uses to
//! say which incident it is talking about when it escalates. The evaluator resolves the anchor
//! with the stream's labels; an anchor on background is a declaration about nothing. See
//! `HIDDEN-DESIGN.md`, section 7.
//!
//! # Time
//!
//! Instants only move forward. A probe returns immediately with `ready_at`, as in the first
//! world; the reasoner's answer is *delivered* by `observe_until` once `ready_at` has passed, so a
//! policy cannot act on an answer before it exists.

use crate::kinds::{Diagnosis, ObsId, ObsRef, Question};
use crate::labels::{EvidenceRole, ObsLabel};
use crate::params::ReasonerCost;
use crate::probe;
use crate::reasoner;
use crate::stream::Stream;
use gordian_core::{Budget, Charge, Instant, Resource};
use gordian_world::physics::probe_cost;
use gordian_world::step::CostSummary;
use gordian_world::{Observation, Probe, ProbeKind, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What a policy can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamAction {
    /// Run a probe.
    Probe {
        /// What to measure.
        kind: ProbeKind,
        /// Where.
        target: ServiceId,
    },
    /// Ask the simulated reasoner.
    Escalate {
        /// The observations the reasoner is given, by reference. Each must be held, and none may
        /// repeat.
        context: Vec<ObsRef>,
        /// What is asked.
        question: Question,
    },
    /// Declare what is wrong with the incident that `anchor` belongs to.
    Declare {
        /// An observation the policy holds.
        anchor: ObsId,
        /// The diagnosis. `None` says `anchor` is not part of an incident.
        diagnosis: Diagnosis,
    },
}

/// The reasoner's answer. A hypothesis, never a measurement: it enters a ledger as
/// `EntryKind::Hypothesis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// The observation the question was about.
    pub focus: ObsId,
    /// What the reasoner says.
    pub diagnosis: Diagnosis,
}

/// What `observe_until` delivers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEvent {
    /// A passive observation.
    Observed {
        /// Its id.
        id: ObsId,
        /// When it was emitted.
        #[serde(with = "crate::timeserde::instant")]
        at: Instant,
        /// What it is.
        obs: Observation,
    },
    /// An answer of the reasoner, delivered at its `ready_at`.
    Answered {
        /// The call it answers (the `call` of `StreamOutcome::Escalated`).
        call: u32,
        /// When it became available.
        #[serde(with = "crate::timeserde::instant")]
        at: Instant,
        /// The answer.
        answer: Answer,
    },
}

impl StreamEvent {
    /// The instant of the event.
    pub fn at(&self) -> Instant {
        match self {
            StreamEvent::Observed { at, .. } | StreamEvent::Answered { at, .. } => *at,
        }
    }

    /// The kind of ledger entry the event belongs in. An observation is a measurement. The
    /// reasoner's answer is a hypothesis: the ledger types keep the two apart, and so does this
    /// event type, so that a harness cannot record an answer as something sensed.
    pub fn entry_kind(&self) -> gordian_core::EntryKind {
        match self {
            StreamEvent::Observed { .. } => gordian_core::EntryKind::Measurement,
            StreamEvent::Answered { .. } => gordian_core::EntryKind::Hypothesis,
        }
    }
}

/// Why an action was refused. A refusal charges nothing and changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamRefusal {
    /// `now` is earlier than a time already passed to this simulator.
    TimeWentBackwards,
    /// `now` is after the end of the stream.
    PastDuration,
    /// The action names a service that is not in the graph.
    UnknownService(ServiceId),
    /// A reference names an observation or probe the policy does not hold yet.
    UnknownRef(ObsRef),
    /// The same reference appears twice in a context.
    DuplicateRef(ObsRef),
    /// The context is longer than the stream allows.
    ContextTooLarge {
        /// Its length.
        len: u32,
        /// The limit.
        max: u32,
    },
    /// The probe does not fit in the remaining probe budget.
    ProbeBudgetExceeded {
        /// What it would have cost.
        cost: CostSummary,
    },
    /// The call does not fit in the remaining reasoner budget.
    ReasonerBudgetExceeded {
        /// What it would have cost, in modelled nanoseconds.
        needed_ns: u64,
    },
}

/// The result of an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamOutcome {
    /// A probe ran.
    Probed {
        /// Its index among accepted probes, the `n` of `ObsRef::Probe(n)`.
        probe: u32,
        /// The result, as an observation.
        observation: Observation,
        /// When the result is available: `now` plus the probe's time cost.
        #[serde(with = "crate::timeserde::instant")]
        ready_at: Instant,
        /// What the probe cost.
        cost: CostSummary,
    },
    /// The reasoner was called and paid for. The answer is delivered by `observe_until`.
    Escalated {
        /// Index among accepted calls.
        call: u32,
        /// When the answer is delivered.
        #[serde(with = "crate::timeserde::instant")]
        ready_at: Instant,
        /// What the call cost.
        cost: ReasonerCost,
    },
    /// A declaration was recorded. Its correctness is not reported.
    Declared {
        /// Index among accepted declarations.
        index: u32,
    },
    /// The action was not carried out.
    Refused(StreamRefusal),
}

/// Budget left, in the three resources the stream limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamRemaining {
    /// Probe units.
    pub probes: u64,
    /// Probe time, nanoseconds.
    pub probe_time_ns: u64,
    /// Modelled nanoseconds of reasoner cost.
    pub reasoner_ns: u64,
}

/// A reasoner call, as recorded on the hidden side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CallRecord {
    pub(crate) incident: Option<u32>,
    pub(crate) fingerprint: u64,
    pub(crate) at: Instant,
    pub(crate) ready_at: Instant,
    pub(crate) refs: u32,
    pub(crate) q: f64,
    pub(crate) d: f64,
    pub(crate) h: f64,
    pub(crate) p0: f64,
    pub(crate) p: f64,
    pub(crate) informed: bool,
    pub(crate) correct: bool,
    pub(crate) focus: ObsId,
    pub(crate) diagnosis: Diagnosis,
}

/// A declaration, as recorded on the hidden side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeclRecord {
    pub(crate) at: Instant,
    pub(crate) anchor: ObsId,
    pub(crate) diagnosis: Diagnosis,
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    call: u32,
    ready_at: Instant,
    answer: Answer,
}

/// Delivers a stream to a policy.
///
/// The simulator never reveals whether a declaration or an answer was right; the evaluator scores
/// the recorded trajectory against hidden state.
#[derive(Clone)]
pub struct StreamSimulator {
    stream: Stream,
    budget: Budget,
    cursor: usize,
    last_now: Instant,
    probes_done: u32,
    pending: Vec<Pending>,
    calls: Vec<CallRecord>,
    world: gordian_world::episode::PublicInfo,
    probe_obs: Vec<(Instant, Observation)>,
    declarations: Vec<DeclRecord>,
}

impl std::fmt::Debug for StreamSimulator {
    /// Counts only: the simulator holds a hidden stream.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamSimulator")
            .field("delivered", &self.cursor)
            .field("probes", &self.probes_done)
            .field("calls", &self.calls.len())
            .finish_non_exhaustive()
    }
}

impl StreamSimulator {
    /// A simulator at the start of `stream`, with the stream's budget.
    pub fn new(stream: Stream) -> Self {
        let budget = stream.params.budget.to_budget();
        let world = stream.public_info().world_public_info();
        Self {
            stream,
            budget,
            cursor: 0,
            last_now: Instant::ZERO,
            probes_done: 0,
            pending: Vec::new(),
            calls: Vec::new(),
            world,
            probe_obs: Vec::new(),
            declarations: Vec::new(),
        }
    }

    /// What a policy may know at the start.
    pub fn public_info(&self) -> crate::params::StreamPublic {
        self.stream.public_info()
    }

    /// Budget still available.
    pub fn remaining(&self) -> StreamRemaining {
        StreamRemaining {
            probes: self.budget.remaining(Resource::Probes).unwrap_or(0),
            probe_time_ns: self.budget.remaining(Resource::Time).unwrap_or(0),
            reasoner_ns: self.budget.remaining(Resource::Compute).unwrap_or(0),
        }
    }

    /// Everything not yet delivered with an instant at or before `now`: passive observations in
    /// stream order, and reasoner answers whose `ready_at` has passed, merged by instant
    /// (observations first at a tie, answers in the order they were asked).
    pub fn observe_until(&mut self, now: Instant) -> Vec<StreamEvent> {
        self.last_now = self.last_now.max(now);
        let events = &self.stream.events;
        let end = self.cursor + events[self.cursor..].partition_point(|(at, _)| *at <= now);
        let mut out: Vec<StreamEvent> = events[self.cursor..end]
            .iter()
            .enumerate()
            .map(|(i, (at, obs))| StreamEvent::Observed {
                id: ObsId((self.cursor + i) as u32),
                at: *at,
                obs: obs.clone(),
            })
            .collect();
        self.cursor = end;
        let (ready, later): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| p.ready_at <= now);
        self.pending = later;
        let mut ready = ready;
        ready.sort_by_key(|p| (p.ready_at, p.call));
        let mut merged = Vec::with_capacity(out.len() + ready.len());
        let mut obs_iter = out.drain(..).peekable();
        for p in ready {
            while obs_iter.peek().is_some_and(|o| o.at() <= p.ready_at) {
                merged.push(obs_iter.next().expect("peeked"));
            }
            merged.push(StreamEvent::Answered {
                call: p.call,
                at: p.ready_at,
                answer: p.answer,
            });
        }
        merged.extend(obs_iter);
        merged
    }

    fn held(&self, r: ObsRef) -> bool {
        match r {
            ObsRef::Passive(id) => (id.0 as usize) < self.cursor,
            ObsRef::Probe(n) => n < self.probes_done,
        }
    }

    /// Apply an action at `now`.
    pub fn apply(&mut self, action: StreamAction, now: Instant) -> StreamOutcome {
        if now < self.last_now {
            return StreamOutcome::Refused(StreamRefusal::TimeWentBackwards);
        }
        if now.0 > self.stream.params.duration_ns {
            return StreamOutcome::Refused(StreamRefusal::PastDuration);
        }
        match action {
            StreamAction::Probe { kind, target } => {
                if target.index() >= self.stream.services.len() {
                    return StreamOutcome::Refused(StreamRefusal::UnknownService(target));
                }
                let charges = probe_cost(kind);
                if self.budget.charge_all(&charges).is_err() {
                    return StreamOutcome::Refused(StreamRefusal::ProbeBudgetExceeded {
                        cost: CostSummary::from_charges(&charges),
                    });
                }
                self.last_now = now;
                let cost = CostSummary::from_charges(&charges);
                let probe = Probe { kind, target };
                let result = match self
                    .stream
                    .incidents
                    .iter()
                    .rev()
                    .find(|i| probe::live(i, now) && i.occupies.contains(&target))
                {
                    Some(inc) => probe::answer(inc, &self.stream.services, probe, now),
                    None => gordian_world::physics::probe_result(
                        &self.stream.services,
                        None,
                        (false, false),
                        0,
                        probe,
                    ),
                };
                let index = self.probes_done;
                self.probes_done += 1;
                let observation = Observation::Probed { probe, result };
                self.probe_obs.push((now, observation.clone()));
                StreamOutcome::Probed {
                    probe: index,
                    observation,
                    ready_at: Instant(now.0.saturating_add(cost.time_ns)),
                    cost,
                }
            }
            StreamAction::Escalate { context, question } => {
                let Question::Diagnose { focus } = question;
                if !self.held(ObsRef::Passive(focus)) {
                    return StreamOutcome::Refused(StreamRefusal::UnknownRef(ObsRef::Passive(
                        focus,
                    )));
                }
                let max = self.stream.params.max_context;
                if context.len() as u64 > max as u64 {
                    return StreamOutcome::Refused(StreamRefusal::ContextTooLarge {
                        len: context.len() as u32,
                        max,
                    });
                }
                let mut seen = BTreeSet::new();
                for r in &context {
                    if !self.held(*r) {
                        return StreamOutcome::Refused(StreamRefusal::UnknownRef(*r));
                    }
                    if !seen.insert(*r) {
                        return StreamOutcome::Refused(StreamRefusal::DuplicateRef(*r));
                    }
                }
                let cost = self.stream.params.reasoner.cost.cost(context.len());
                let charge: Charge = crate::params::StreamBudgetSpec::reasoner_charge(&cost);
                // Paid first. Nothing below runs unless the charge was accepted, and nothing
                // above changed any state, so a refused call leaves no trace and consumes no draw.
                if self.budget.charge(charge).is_err() {
                    return StreamOutcome::Refused(StreamRefusal::ReasonerBudgetExceeded {
                        needed_ns: cost.modelled_ns,
                    });
                }
                self.last_now = now;
                let incident = match self.stream.labels[focus.0 as usize] {
                    ObsLabel::Incident { id, .. } => Some(id),
                    ObsLabel::Background(_) => None,
                };
                // Decisive evidence of the focus incident present in the context (a probe
                // reference is never decisive, and a question about background has no focus
                // incident, so nothing is). Everything else in the context is a distractor.
                let present = match incident {
                    None => 0,
                    Some(id) => context
                        .iter()
                        .filter(|r| match r {
                            ObsRef::Passive(o) => matches!(
                                self.stream.labels[o.0 as usize],
                                ObsLabel::Incident { id: i, role: EvidenceRole::Decisive } if i == id
                            ),
                            ObsRef::Probe(_) => false,
                        })
                        .count(),
                };
                let distractors = context.len() - present;
                let q = match incident {
                    None => 1.0,
                    Some(id) => {
                        let total = self.stream.decisive_total[id as usize];
                        if total == 0 {
                            1.0
                        } else {
                            present as f64 / total as f64
                        }
                    }
                };
                let inc = incident.map(|id| &self.stream.incidents[id as usize]);
                let focus_site =
                    crate::reasoner::service_of(&self.stream.events[focus.0 as usize].1);
                let context_obs: Vec<(Instant, Observation)> = context
                    .iter()
                    .map(|r| match r {
                        ObsRef::Passive(o) => self.stream.events[o.0 as usize].clone(),
                        ObsRef::Probe(n) => self.probe_obs[*n as usize].clone(),
                    })
                    .collect();
                // `seen` is the context as a sorted set: the canonical form the draws are keyed by.
                let fingerprint = reasoner::fingerprint(
                    focus.0,
                    seen.iter().map(|r| match r {
                        ObsRef::Passive(o) => (0u8, o.0),
                        ObsRef::Probe(n) => (1u8, *n),
                    }),
                );
                // A question about background has no incident; its draws are keyed by the focus.
                let subject = incident.map_or((1u64 << 32) + focus.0 as u64, |id| id as u64);
                let verdict = reasoner::answer(
                    &self.stream,
                    &self.world,
                    inc,
                    subject,
                    focus_site,
                    &context_obs,
                    fingerprint,
                    q,
                    distractors,
                );
                let call = self.calls.len() as u32;
                let ready_at = Instant(now.0.saturating_add(cost.latency_ns));
                self.calls.push(CallRecord {
                    incident,
                    fingerprint,
                    at: now,
                    ready_at,
                    refs: context.len() as u32,
                    q,
                    d: inc.map_or(self.stream.params.difficulty.background, |i| i.difficulty),
                    h: verdict.h,
                    p0: verdict.p0,
                    p: verdict.p,
                    informed: verdict.informed,
                    correct: verdict.correct,
                    focus,
                    diagnosis: verdict.diagnosis,
                });
                self.pending.push(Pending {
                    call,
                    ready_at,
                    answer: Answer {
                        focus,
                        diagnosis: verdict.diagnosis,
                    },
                });
                StreamOutcome::Escalated {
                    call,
                    ready_at,
                    cost,
                }
            }
            StreamAction::Declare { anchor, diagnosis } => {
                if !self.held(ObsRef::Passive(anchor)) {
                    return StreamOutcome::Refused(StreamRefusal::UnknownRef(ObsRef::Passive(
                        anchor,
                    )));
                }
                if let Some(h) = diagnosis
                    && h.site.index() >= self.stream.services.len()
                {
                    return StreamOutcome::Refused(StreamRefusal::UnknownService(h.site));
                }
                self.last_now = now;
                let index = self.declarations.len() as u32;
                self.declarations.push(DeclRecord {
                    at: now,
                    anchor,
                    diagnosis,
                });
                StreamOutcome::Declared { index }
            }
        }
    }

    #[cfg(any(test, feature = "reveal-hidden-state"))]
    pub(crate) fn hidden(&self) -> (&Stream, &[CallRecord], &[DeclRecord]) {
        (&self.stream, &self.calls, &self.declarations)
    }
}
