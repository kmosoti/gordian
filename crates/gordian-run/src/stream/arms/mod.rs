//! The stream arms: one shared cheap rung, and the escalation rules that make the arms differ.
//!
//! # The constraint that shapes everything
//!
//! EXP-101's intervention is *when and what an arm escalates* (charter section 6). Every arm that
//! is neither the privileged oracle nor the labelled ablation is therefore a [`StreamArm`]: the
//! one shared cheap rung and declaring procedure of [`rung`], plus an [`EscalationRule`] that says
//! which noticed anomalies to escalate, and when. A rule cannot reach the rung's declaration
//! logic, and `StreamArm` is the only [`StreamPolicy`] implementation outside the privileged
//! file, so two arms cannot differ in how they notice, conclude or declare. Tests in
//! `tests/stream_arms.rs` make that checkable and not merely stated.
//!
//! # The contradiction arm
//!
//! `contradiction_escalation` (work item R5) escalates on the cheap rung's own failure to explain an
//! anomaly: the consistency checker's verdict ([`rung::AnomalyView::contradicted_since`]), which the
//! rung keeps current only for a rule that asks ([`EscalationRule::monitors`]). It reads nothing the
//! other arms do not, and its context is the rung's.
//!
//! # The public selectors (work item B4)
//!
//! `public_threshold` and `public_change` are the non-privileged counterparts of the selection
//! oracle: escalation rules over any noticer's anomalies that choose which to ask about, at R5's
//! delay after notice and with the rung's context, from public information only
//! ([`public_threshold`], [`public_change`]). Their readings are stated in their module
//! documentation. They are the instrument that charges every notice, decoys and late plain ones
//! included, which the selection oracle never asks about.
//!
//! # What an arm may see
//!
//! A [`StepInput`]: the observations and reasoner answers the stream delivered, the probe results
//! that are ready, what became of its last proposals, and the stream's public information. Never
//! the generated stream, its parameters, its truth, the reasoner's call records, the verdict, the
//! bill or a timing. Everything a policy can do it does by returning [`Proposed`] actions, which
//! the harness validates, charges and applies; and by running components and the shared rule
//! through the [`crate::stream::meter::Meter`], which charges, times and counts them. The files
//! in this directory are held to the same textual ban as `policy/`
//! (`scripts/check-no-oracle.sh`).
//!
//! # Where the knowledge is
//!
//! `ablation.rs` is the only file here that knows the hidden rules of the stream's hard
//! incidents, and the only one that overrides [`EscalationRule::recognize`]. A test checks both.

pub mod ablation;
pub mod always;
pub mod change;
pub mod context;
pub mod contradiction;
pub mod learned;
pub mod medium;
pub mod never;
pub mod noticer;
pub mod noticer_change;
pub mod noticer_follow;
pub mod noticer_ramp;
pub mod noticer_reanchor;
pub mod noticer_rung;
pub mod noticer_split;
pub mod periodic;
pub mod public_change;
pub mod public_threshold;
pub mod random;
pub mod reservoir;
pub mod rung;
pub mod threshold;

use crate::policy::PolicyId;
use crate::policy::decide;
use crate::stream::meter::Meter;
use gordian_core::{Charge, Instant, Resource};
use gordian_stream::{Diagnosis, ObsId, ObsRef, Question, StreamAction, StreamEvent, StreamPublic};
use gordian_world::Observation;
use rung::{AnomalyView, Conclusion, Outstanding, Rung, RungConfig, Store};
use std::collections::BTreeMap;

/// What the harness shows an arm at a step.
#[derive(Debug, Clone)]
pub struct StepInput<'a> {
    /// The instant of the step.
    pub now: Instant,
    /// What the stream delivered since the last step: passive observations and reasoner answers,
    /// in order.
    pub events: &'a [StreamEvent],
    /// Probe results that are ready: the probe's index, the instant it was ready, the result.
    pub probe_results: &'a [(u32, Instant, Observation)],
    /// What became of the proposals of the previous step.
    pub applied: &'a [Applied],
    /// What an arm may know of the stream at the start.
    pub public: &'a StreamPublic,
}

/// What became of one proposal: the harness accepted and applied it, or refused it (the bill
/// could not pay, or the action was malformed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// The proposal's tag.
    pub tag: u64,
    /// Whether it was carried out.
    pub accepted: bool,
}

/// Where a proposed action came from, for the ledger and the per-source counts of `results.csv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A declaration from the shared cheap rung (or, in the ablation, its recognizers).
    CheapRung,
    /// A declaration of a reasoner's answer.
    Reasoner,
    /// An escalation.
    Escalation,
    /// A probe bought by the shared rule.
    Probe,
}

impl Source {
    /// The name written to the ledger.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::CheapRung => "cheap_rung",
            Source::Reasoner => "reasoner",
            Source::Escalation => "escalation",
            Source::Probe => "probe",
        }
    }
}

/// An action an arm asks the harness to carry out.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    /// An opaque token the harness returns in [`Applied`]; 0 when the arm does not care.
    pub tag: u64,
    /// The action.
    pub action: StreamAction,
    /// Where it came from.
    pub source: Source,
}

/// What an arm is: compared, privileged or an ablation. Written into every output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmRole {
    /// An arm of a comparison.
    Comparison,
    /// Uses hidden state; the headroom ceiling only.
    Privileged,
    /// Encodes hidden rules; shows what that knowledge is worth and is never a comparison arm.
    Ablation,
}

impl ArmRole {
    /// The word written to the `arm_role` column.
    pub fn as_str(self) -> &'static str {
        match self {
            ArmRole::Comparison => "comparison",
            ArmRole::Privileged => "privileged",
            ArmRole::Ablation => "ablation",
        }
    }
}

use serde::{Deserialize, Serialize};

/// What an arm reports about itself at the end of a segment. Public bookkeeping only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArmReport {
    /// Anomalies the cheap rung noticed.
    pub anomalies_noticed: u32,
}

/// A policy of the stream harness: one instance plays one segment.
pub trait StreamPolicy {
    /// Stable identity, written to the ledger and the manifest.
    fn id(&self) -> PolicyId;

    /// The name of the rule that turns evidence into declarations. Every arm but the privileged
    /// one reports [`decide::RULE`]; a test asserts it.
    fn decision_rule(&self) -> &'static str {
        "unspecified"
    }

    /// The arm's role.
    fn role(&self) -> ArmRole;

    /// What the arm's own bookkeeping at this step is declared to cost, charged under
    /// `Phase::Scheduling` before [`StreamPolicy::step`]. A function of the input and the arm's
    /// own fields, with no side effects.
    fn declared_step_cost(&self, input: &StepInput<'_>) -> Vec<Charge>;

    /// Take in the step and return the actions to carry out, in order. `work_allowed` is false
    /// when the bill refused the step's declared cost: the arm then only takes in what was
    /// delivered and declares reasoner answers, which is free.
    fn step(
        &mut self,
        input: &StepInput<'_>,
        work_allowed: bool,
        meter: &mut Meter<'_>,
    ) -> Vec<Proposed>;

    /// The final call, at the end of the stream: take in what is left and declare what the arm
    /// has, as the first world's final call does for an arm that has run out of means.
    fn finish(&mut self, input: &StepInput<'_>, meter: &mut Meter<'_>) -> Vec<Proposed>;

    /// The arm's own counts.
    fn report(&self) -> ArmReport;

    /// The id of the noticer whose anomalies the arm works on (work item B1); `none` for an arm
    /// that has no noticer.
    fn noticer_id(&self) -> &'static str {
        "none"
    }

    /// Every notice and retirement so far, in order: public information only. The harness writes
    /// it to the run output and scores it, beside the arm's declarations.
    fn notice_log(&self) -> &[noticer::NoticeLogEntry] {
        &[]
    }
}

/// What a rule may see when it makes direct requests (the privileged arms' only).
#[derive(Debug, Clone, Copy)]
pub struct DirectCtx<'a> {
    /// The instant of the step.
    pub now: Instant,
    /// Observations delivered so far.
    pub delivered: u32,
    /// The noticed anomalies.
    pub views: &'a [AnomalyView],
    /// The context the rung's configured builder makes for an anchor the rung did not notice.
    pub contexts: RungContexts<'a>,
}

/// The shared rung's context builder, offered to a rule's direct requests: the references the
/// configured builder ([`rung::RungConfig::context`]) makes for a question about an observation,
/// as [`Rung::context_at`] says. It lets a privileged rule ask about an anchor of its own with
/// the same context a noticed anomaly there would get, without building a context of its own.
#[derive(Clone, Copy)]
pub struct RungContexts<'a>(&'a Rung);

impl RungContexts<'_> {
    /// The references the configured builder makes for a question about the held observation
    /// `anchor`; empty when the observation is not held.
    pub fn at(&self, anchor: ObsId) -> Vec<ObsRef> {
        self.0.context_at(anchor)
    }
}

impl std::fmt::Debug for RungContexts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RungContexts")
    }
}

/// A question a rule asks the reasoner directly, not about a noticed anomaly.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectRequest {
    /// The observation the question is about.
    pub focus: ObsId,
    /// The references to put in the context.
    pub context: Vec<ObsRef>,
}

/// What a recognizer is shown (the ablation's).
pub struct RecognizeCtx<'a> {
    /// The anomaly.
    pub view: &'a AnomalyView,
    /// The observations held.
    pub store: &'a Store,
    /// The abnormal observations attached to the anomaly.
    pub attached: &'a [(Instant, ObsId, gordian_world::ServiceId)],
    /// The stream's public information.
    pub public: &'a StreamPublic,
}

/// The part of an arm that is allowed to differ: which noticed anomalies it escalates, and when.
pub trait EscalationRule {
    /// Stable identity of the arm.
    fn id(&self) -> PolicyId;

    /// The arm's role. Comparison unless the rule says otherwise.
    fn role(&self) -> ArmRole {
        ArmRole::Comparison
    }

    /// The anomalies (by id) to escalate now, among the noticed ones in `views`. An anomaly named
    /// here is escalated once, with the context the rung builds, whatever else has happened to it.
    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32>;

    /// Whether the cheap rung's declaration for this anomaly is held back. By default while an
    /// escalation about it is in flight: its answer will outrank the cheap rung's, and declaring
    /// the cheap rung's meanwhile would add a wrong declaration the arm is about to replace.
    fn holds(&self, view: &AnomalyView) -> bool {
        view.pending > 0
    }

    /// Whether the rule may keep anomalies from retiring ([`EscalationRule::keeps`]). False by
    /// default, and then the step does not look at the views of the quiet anomalies at all, so an
    /// arm whose rule does not keep does exactly what it did before the hook existed. Only the
    /// selection oracle with `hold_until_asked` keeps any (work item B2).
    fn may_keep(&self) -> bool {
        false
    }

    /// Whether this quiet anomaly is kept live: not given the rule's final call and not retired
    /// this step, though it has had no abnormal observation for the rung's quiet time. Asked only
    /// when [`EscalationRule::may_keep`] is true. The rung retires it at the first step at which
    /// this is false and the anomaly is still quiet.
    fn keeps(&self, _view: &AnomalyView) -> bool {
        false
    }

    /// Questions to ask the reasoner that are not about a noticed anomaly. Only the privileged
    /// arm makes any.
    fn direct(&mut self, _ctx: &DirectCtx<'_>) -> Vec<DirectRequest> {
        Vec::new()
    }

    /// The noticed anomalies (by id) to dismiss now: declared "not an incident" at their anchor,
    /// and never declared for by the cheap rung. No escalation is involved. Only the privileged
    /// decoy arm dismisses anything.
    fn dismissals(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }

    /// Whether the rule reads [`AnomalyView::contradicted_since`], so that the rung must keep the
    /// consistency checker's verdict on each anomaly up to date
    /// ([`rung::Rung::set_monitor`]). Only `contradiction_escalation` does. When false the rung
    /// does exactly what it does without the feature.
    fn monitors(&self) -> bool {
        false
    }

    /// Whether the arm has recognisers ([`EscalationRule::recognize`]) to run on every noticed
    /// anomaly, outranking the shared rule's conclusion. Only the ablation does.
    fn revises(&self) -> bool {
        false
    }

    /// A conclusion reached by rules the shared rule does not have. **Only
    /// `ablation_hidden_rules` overrides this**; a test checks it. For every other arm it is
    /// `None`, and the rung's conclusion stands.
    fn recognize(&self, _ctx: &RecognizeCtx<'_>) -> Option<Diagnosis> {
        None
    }
}

/// Declared cost of an arm's bookkeeping, in `Resource::Compute` nanoseconds: a base, a term per
/// event absorbed and a term per live anomaly. **Placeholders, not fitted**: they bound the
/// bookkeeping through the bill and nothing more. The bookkeeping is measured (in
/// `measured_sched_ns`) and is not in the modelled cost, which has no calibrated weights for it.
const STEP_BASE_NS: u64 = 400;
const STEP_EVENT_NS: u64 = 60;
const STEP_ANOMALY_NS: u64 = 250;

/// A rule together with the shared cheap rung. The only non-privileged [`StreamPolicy`].
pub struct StreamArm<E: EscalationRule> {
    rule: E,
    rung: Rung,
    outstanding: BTreeMap<u64, Outstanding>,
    next_tag: u64,
}

impl<E: EscalationRule> StreamArm<E> {
    /// `rule` over a fresh rung for a stream with `public` information.
    pub fn with(rule: E, public: &StreamPublic, config: RungConfig) -> Self {
        let mut rung = Rung::new(public, config);
        rung.set_monitor(rule.monitors());
        Self {
            rule,
            rung,
            outstanding: BTreeMap::new(),
            next_tag: 1,
        }
    }

    /// The rule.
    pub fn rule(&self) -> &E {
        &self.rule
    }

    fn tag(&mut self, what: Outstanding) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.outstanding.insert(tag, what);
        tag
    }

    fn view_of(&mut self, id: u32, now: Instant) -> Option<AnomalyView> {
        self.rung.views(now).into_iter().find(|v| v.id == id)
    }
}

impl<E: EscalationRule> StreamPolicy for StreamArm<E> {
    fn id(&self) -> PolicyId {
        self.rule.id()
    }

    fn decision_rule(&self) -> &'static str {
        decide::RULE
    }

    fn role(&self) -> ArmRole {
        self.rule.role()
    }

    fn declared_step_cost(&self, input: &StepInput<'_>) -> Vec<Charge> {
        let events = input.events.len() as u64;
        let live = self.rung.live() as u64;
        vec![Charge::new(
            Resource::Compute,
            STEP_BASE_NS + STEP_EVENT_NS * events + STEP_ANOMALY_NS * live,
        )]
    }

    fn step(
        &mut self,
        input: &StepInput<'_>,
        work_allowed: bool,
        meter: &mut Meter<'_>,
    ) -> Vec<Proposed> {
        let now = input.now;
        let mut out = Vec::new();
        self.rung.applied(input.applied, &mut self.outstanding, now);
        let answers = self.rung.absorb(input.events, input.probe_results, now);
        for (focus, diagnosis) in answers {
            if self.rung.take_answer(focus, diagnosis) {
                out.push(Proposed {
                    tag: 0,
                    action: StreamAction::Declare {
                        anchor: focus,
                        diagnosis,
                    },
                    source: Source::Reasoner,
                });
            }
        }
        if !work_allowed {
            return out;
        }
        self.rung.notice(now);
        // A noticer that does counted work (the medium, work item M2) is charged for it like a
        // component call; B1's noticers report none and nothing here runs for them.
        if let Some(cost) = self.rung.take_noticer_cost()
            && !meter.charge_noticer(cost)
        {
            self.rung.noticer_refused();
        }

        // Consistency checks, for a rule that reads them (`contradiction_escalation`; the list is
        // empty for every other arm): before the views, so the rule sees this step's verdicts.
        for id in self.rung.due_checks(now) {
            self.rung.check(id, now, meter);
        }

        // Dismissals: before the reviews, so that a dismissed anomaly is not concluded about.
        let views = self.rung.views(now);
        for id in self.rule.dismissals(now, &views) {
            if let Some(proposed) = self.rung.dismiss(id) {
                out.push(proposed);
            }
        }

        // Escalations: decided before the reviews, so that a cheap conclusion reached at the same
        // step sees the hold.
        let mut targets = self.rule.targets(now, &views);
        targets.sort_unstable();
        targets.dedup();
        for id in targets {
            let Some(view) = views.iter().find(|v| v.id == id) else {
                continue;
            };
            let context = self.rung.context(id);
            let tag = self.tag(Outstanding::Escalation { anomaly: id });
            self.rung.note_attempt(id, now, view.digest);
            out.push(Proposed {
                tag,
                action: StreamAction::Escalate {
                    context,
                    question: Question::Diagnose { focus: view.anchor },
                },
                source: Source::Escalation,
            });
        }
        for request in self.rule.direct(&DirectCtx {
            now,
            delivered: self.rung.delivered(),
            views: &views,
            contexts: RungContexts(&self.rung),
        }) {
            out.push(Proposed {
                tag: 0,
                action: StreamAction::Escalate {
                    context: request.context,
                    question: Question::Diagnose {
                        focus: request.focus,
                    },
                },
                source: Source::Escalation,
            });
        }

        // Reviews: the cheap components and the shared rule, through the meter.
        for id in self.rung.due(now) {
            let Some(conclusion) = self.rung.review(id, now, meter) else {
                continue;
            };
            let Some(view) = self.view_of(id, now) else {
                continue;
            };
            let held = self.rule.holds(&view);
            let probe = match &conclusion {
                Conclusion::Probe(p) => Some(*p),
                _ => None,
            };
            let tag = match probe {
                Some(p) => self.tag(Outstanding::Probe {
                    anomaly: id,
                    probe: p,
                }),
                None => 0,
            };
            if let Some(proposed) = self.rung.conclude(id, conclusion, held, tag) {
                out.push(proposed);
            }
        }

        // Recognisers, for an arm that has any: they read the anomaly's evidence directly and
        // run whether or not the shared rule has concluded.
        if self.rule.revises() {
            for view in self.rung.views(now) {
                let attached = self.rung.attached(view.id);
                let ctx = RecognizeCtx {
                    view: &view,
                    store: self.rung.store(),
                    attached: &attached,
                    public: input.public,
                };
                if let Some(diagnosis) = self.rule.recognize(&ctx)
                    && let Some(proposed) = self.rung.declare_recognized(view.id, diagnosis)
                {
                    out.push(proposed);
                }
            }
        }

        // Declarations whose hold has lifted.
        for view in self.rung.views(now) {
            if !self.rule.holds(&view)
                && self.rung.deferred(view.id).is_some()
                && let Some(proposed) = self.rung.declare_deferred(view.id)
            {
                out.push(proposed);
            }
        }

        // Quiet anomalies get the rule's final call and retire, unless the rule keeps them live
        // (only the selection oracle with `hold_until_asked`, which keeps an anomaly it will ask
        // about until it has asked).
        for id in self.rung.quiet(now) {
            if self.rule.may_keep()
                && let Some(view) = self.view_of(id, now)
                && self.rule.keeps(&view)
            {
                continue;
            }
            if let Some(proposed) = self.rung.finish_one(id, meter, 0) {
                out.push(proposed);
            }
            self.rung.retire(id);
        }
        out
    }

    fn finish(&mut self, input: &StepInput<'_>, meter: &mut Meter<'_>) -> Vec<Proposed> {
        let mut out = Vec::new();
        self.rung
            .applied(input.applied, &mut self.outstanding, input.now);
        let answers = self
            .rung
            .absorb(input.events, input.probe_results, input.now);
        for (focus, diagnosis) in answers {
            if self.rung.take_answer(focus, diagnosis) {
                out.push(Proposed {
                    tag: 0,
                    action: StreamAction::Declare {
                        anchor: focus,
                        diagnosis,
                    },
                    source: Source::Reasoner,
                });
            }
        }
        for id in self.rung.undecided() {
            if let Some(proposed) = self.rung.finish_one(id, meter, 0) {
                out.push(proposed);
            }
        }
        out
    }

    fn report(&self) -> ArmReport {
        ArmReport {
            anomalies_noticed: self.rung.noticed_total(),
        }
    }

    fn noticer_id(&self) -> &'static str {
        self.rung.noticer_id()
    }

    fn notice_log(&self) -> &[noticer::NoticeLogEntry] {
        self.rung.notice_log()
    }
}
