//! The privileged arm: `oracle_escalation`.
//!
//! The charter's "oracle escalation (privileged)": whether the world has measurable headroom for
//! escalation control. It is the only stream policy that uses the stream's truth, and this is the
//! only arm file that acts on it (`scripts/check-no-oracle.sh` bans the truth's accessors and the
//! plan from `arms/`).
//!
//! # What it is
//!
//! The same cheap rung and the same declaring procedure as every comparison arm
//! ([`super::arms::StreamArm`]), with a different [`EscalationRule`]: one that knows which
//! incidents are hard. So its escalation decisions are the only thing privileged about it, which
//! is what a headroom ceiling for *escalation control* has to isolate. It escalates exactly the
//! hard incidents, and each exactly once, at the earliest instant the reasoner can be given all of
//! the incident's decisive evidence (the instant its last decisive observation has been delivered:
//! `q = 1`), with that evidence and nothing else as the context. It never escalates a plain
//! incident or a decoy, and it holds back the cheap rung's declaration for a hard incident until
//! the answer has arrived, since it knows that declaration is wrong.
//!
//! *Reading of the specification.* The brief says "at onset, with their decisive evidence once it
//! exists". An escalation at onset has no decisive evidence to give (a hard incident's decisive
//! evidence arrives 6 to 16 s after onset), so it would waste a call at `q` near 0. This arm knows
//! the onset, so it needs no noticing; it asks when the evidence exists. If the coordinator meant
//! something else (two calls per hard incident, one at onset), it is a change to
//! `OracleEscalation::direct`.
//!
//! # The two decomposing arms (work item R5)
//!
//! `oracle_escalation` bundles three privileges: which anomalies are hard (selection), when to
//! ask (the instant the decisive evidence has been delivered) and what to put in the context (the
//! decisive evidence and nothing else). R5 separates them, and every one of the following arms
//! is the shared cheap rung and the shared declaring procedure with a different rule, built by
//! the same factory from the same plan:
//!
//! - `oracle_selection_privileged` knows which noticed anomalies are hard (those whose anchor
//!   belongs to a hard incident) and escalates exactly those, once each, `delay_ns` after they are
//!   noticed, with the context the rung builds ([`super::arms::rung::Rung::context`], the
//!   function every comparison arm uses). Its only privilege is the choice of anomalies: it
//!   neither reads the decisive-evidence labels nor builds a context of its own, and it has no
//!   say in when the cheap rung declares (the default hold, an escalation in flight, applies).
//! - `oracle_decoy_privileged` knows which noticed anomalies are decoys (anchor in a decoy) and
//!   dismisses them: declares "not an incident" at the anchor the step they are noticed, after
//!   which the cheap rung never declares for them and no longer reviews them. It escalates
//!   nothing; for every other anomaly it is `never_escalate`.
//!
//! Both are privileged by role and by name, and reach the truth the way `oracle_escalation`
//! does: through the plan, by the harness, and nothing else. Their rules are private to this
//! file.
//!
//! # Work item R6
//!
//! `oracle_selection` uses whichever context builder its rung is configured with
//! ([`super::arms::context`]): the builder is an option of the shared rung, so the selection
//! oracle takes any public builder with no change here, and its only privilege stays the choice of
//! anomalies. `oracle_escalation` stays the decisive-evidence ceiling, unchanged.
//!
//! `oracle_selection_context` is a **supplementary** arm, labelled as such in R6's report and not
//! used by R6's criterion. It separates context from timing in `oracle_escalation`'s ceiling: it
//! knows which noticed anomalies are hard (as `oracle_selection` does) and asks `delay_ns` after
//! each is noticed (as `oracle_selection` does), but its context is the incident's decisive
//! evidence among what has been delivered by then, which is a context privilege and nothing
//! else. The difference between it and `oracle_selection` is what a perfect context is worth at a
//! public instant; the difference between `oracle_escalation` and it is what firing at the
//! instant the evidence has all arrived is worth.
//!
//! # Work item R10: `oracle_notice`
//!
//! `oracle_notice` is `oracle_selection` plus one privilege, noticing. The selection oracle asks
//! only about anomalies the shared rung noticed, so it makes no call on an incident the rung
//! never noticed, and for one the rung notices late (the slow leak is noticed only once its
//! counter crosses the alarm level) its delay runs from the late notice. The notice oracle is
//! told, from the plan, which observation begins each hard incident
//! ([`PlanIncident::first`]) and treats it as noticed at the step that observation is delivered:
//!
//! - **Injected notice.** For every hard incident, at the first step at which its first
//!   observation has been delivered, the arm records a notice anchored at that observation, whose
//!   site is the service the observation names. The notice is the arm's own record; it is not an
//!   anomaly of the shared rung, so it changes nothing the rung notices, reviews or probes. The
//!   only trace in the rung is the one a call leaves for every arm: a reasoner's answer about an
//!   observation is credited to the rung anomaly that owns it, if there is one, which then no
//!   longer declares on its own.
//! - **The rung's later notice is ignored for escalation.** The arm escalates nothing through
//!   [`EscalationRule::targets`]: an anomaly the rung notices, whether of the same incident or
//!   another, causes no call. Each injected notice causes exactly one, `delay_ns` after it, so a
//!   hard incident is asked about once whatever the rung does.
//! - **Context.** The context is the one the rung's configured builder makes for that anchor and
//!   site at the instant of the call ([`super::arms::rung::Rung::context_at`], reached through
//!   [`DirectCtx::contexts`]), the function [`super::arms::rung::Rung::context`] applies to a noticed
//!   anomaly. The arm builds no context of its own and reads no decisive label.
//! - **Hold.** The default: [`EscalationRule::holds`] is not overridden, so the shared rung's
//!   declarations are held back exactly where they are held for the selection oracle, while a call
//!   about a noticed anomaly is in flight. A direct call is not about a noticed anomaly, so it
//!   holds nothing; the rung declares for its own anomalies as it does for every arm, and a
//!   reasoner's answer is declared when it arrives.
//!
//! What the plan gives it: the hard flag and the first observation of each incident, and nothing
//! else (a test builds plans that differ in every other field and checks the arm does not move).
//!
//! # Work item B2: `hold_until_asked`
//!
//! `oracle_selection` asks about a noticed anomaly `delay_ns` after it was noticed, and only if the
//! anomaly is still live then. The shared rung retires an anomaly after its quiet time (6 s with no
//! abnormal observation), so with a delay longer than that, an anomaly that went quiet is retired
//! before the oracle asks and is never asked about: the selection oracle's quality then measures
//! how long the noticer's anomalies live as much as what they were anchored on. The option
//! `hold_until_asked` (off by default, and then the arm is byte for byte what it was) keeps an
//! anomaly the oracle *will* ask about live until it has asked: a quiet anomaly whose anchor
//! belongs to a hard incident and which has had no escalation proposed ([`AnomalyView::attempts`] is
//! zero) is not given the rule's final call and not retired ([`EscalationRule::keeps`]). Once asked
//! it retires as every anomaly does. Its only privilege is the one the arm already has, which
//! anomalies are hard.
//!
//! What the hold changes besides the retirement, stated because a reader will look for it: a live
//! anomaly stays in the noticer's set, so the rung's last-resort attach rule (an abnormal
//! observation at the site of a stale anomaly joins it) can take a later observation at that
//! site into the held anomaly instead of opening a new candidate. That can change what is noticed
//! later at that site, within the delay. The notice record of a run with the hold is therefore the
//! record of that run, and is reported as such, not read as the record of the same noticer without
//! it.
//!
//! # How the truth reaches it, and only it
//!
//! [`StreamPolicy`] and [`EscalationRule`] have no place for a truth. [`OracleFactory::build`]
//! takes an [`OraclePlan`], the facts about the segment's hard incidents that this arm acts on, and
//! is called by `harness::run_segment_privileged`, the one entry point that takes a factory
//! instead of an arm. The plan is plain data: the harness fills it by reading the truth it holds
//! aside (field access on a value it never names; no type of the truth is written anywhere in this
//! crate, which is what lets the stream's reveal accessor crate go: see `HARNESS.md`, section 11).
//! Nothing in an arm can obtain a truth, so nothing in an arm can fill a plan with true facts, and
//! `scripts/check-no-oracle.sh` and a test ban the plan's name, the evaluator crate and its
//! accessors from every arm file. `OracleEscalation`, the rule the factory builds, is private to
//! this file:
//!
//! ```compile_fail
//! // `OracleEscalation` has no public name, so nothing else can construct one.
//! use gordian_run::stream::privileged::OracleEscalation;
//! ```
//!
//! The manifest rejects this arm unless its name contains `privileged`, and every `results.csv`
//! row carries `arm_role = privileged`.

use super::arms::rung::{AnomalyView, RungConfig};
use super::arms::{ArmRole, DirectCtx, DirectRequest, EscalationRule, StreamArm, StreamPolicy};
use crate::policy::PolicyId;
use gordian_core::Instant;
use gordian_stream::{ObsId, ObsRef, StreamPublic};
use std::collections::{BTreeMap, BTreeSet};

/// The id of `oracle_escalation`.
pub const ID: &str = "oracle_escalation";

/// The id of `oracle_selection`: escalates exactly the hard anomalies with the rung's context.
pub const SELECTION_ID: &str = "oracle_selection";

/// The id of `oracle_decoy`: dismisses exactly the decoy anomalies and escalates nothing.
pub const DECOY_ID: &str = "oracle_decoy";

/// The id of `oracle_selection_context` (R6, supplementary): `oracle_selection`'s choice of
/// anomalies and delay, with the decisive evidence delivered so far as the context.
pub const SELECTION_CONTEXT_ID: &str = "oracle_selection_context";

/// The id of `oracle_notice` (R10): `oracle_selection` plus one privilege, noticing. It escalates
/// each hard incident once, `delay_ns` after a notice injected at the step the incident's first
/// observation is delivered, with the context the rung's builder makes.
pub const NOTICE_ID: &str = "oracle_notice";

/// What the privileged arm is told about one incident of the segment: facts the harness read from
/// the truth it holds aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanIncident {
    /// The incident's id.
    pub id: u32,
    /// Whether the incident is hard.
    pub hard: bool,
    /// Whether the incident is a decoy.
    pub decoy: bool,
    /// Every decisive observation of the incident, in stream order.
    pub decisive: Vec<ObsId>,
    /// The incident's first observation.
    pub first: Option<ObsId>,
}

/// The facts the privileged arm acts on: which incident each observation belongs to, and each
/// incident's tier and decisive evidence.
///
/// Plain data, built by the harness from the truth (`harness.rs`, in `play`). Nothing outside the
/// harness can read the truth, so nothing else can fill one with true facts; see the module
/// documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OraclePlan {
    owner: Vec<Option<u32>>,
    incidents: Vec<PlanIncident>,
}

impl OraclePlan {
    /// A plan from the incident each observation belongs to (`None` for background), parallel to
    /// the stream's observations, and the segment's incidents.
    pub fn new(owner: Vec<Option<u32>>, incidents: Vec<PlanIncident>) -> Self {
        Self { owner, incidents }
    }
}

/// Builds the privileged arm for a segment once the harness hands over its plan.
///
/// The only way to build one. The constructor takes no plan, so a factory can be made anywhere
/// but can only be used by the harness, which holds the truth. The rule it builds is private to
/// this file.
#[derive(Debug, Clone)]
pub struct OracleFactory {
    rung: RungConfig,
    mode: Mode,
}

/// Which privileged arm a factory builds.
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// `oracle_escalation`.
    Escalation,
    /// `oracle_selection`, with its delay after notice and whether it keeps an anomaly it will
    /// ask about live until it has asked (work item B2).
    Selection { delay_ns: u64, hold: bool },
    /// `oracle_decoy`.
    Decoy,
    /// `oracle_selection_context`, with its delay after notice.
    SelectionContext { delay_ns: u64 },
    /// `oracle_notice`, with its delay after the injected notice.
    Notice { delay_ns: u64 },
}

impl OracleFactory {
    /// A factory of `oracle_escalation` whose arm shares the cheap rung `rung`.
    pub fn new(rung: RungConfig) -> Self {
        Self {
            rung,
            mode: Mode::Escalation,
        }
    }

    /// A factory of `oracle_selection`, which escalates each hard anomaly `delay_ns` after it is
    /// noticed, with the context of the shared cheap rung `rung`.
    pub fn selection(rung: RungConfig, delay_ns: u64) -> Self {
        Self::selection_with(rung, delay_ns, false)
    }

    /// A factory of `oracle_selection` with the option `hold_until_asked` (work item B2): as
    /// [`OracleFactory::selection`], and an anomaly the oracle will ask about stays live until it
    /// has been asked. `hold = false` is `selection` exactly.
    pub fn selection_with(rung: RungConfig, delay_ns: u64, hold: bool) -> Self {
        Self {
            rung,
            mode: Mode::Selection { delay_ns, hold },
        }
    }

    /// A factory of `oracle_selection_context` (R6, supplementary), which escalates each hard
    /// anomaly `delay_ns` after it is noticed, with the incident's decisive evidence delivered by
    /// then as the context.
    pub fn selection_context(rung: RungConfig, delay_ns: u64) -> Self {
        Self {
            rung,
            mode: Mode::SelectionContext { delay_ns },
        }
    }

    /// A factory of `oracle_notice` (R10): the selection oracle with one more privilege, a notice
    /// injected at the step each hard incident's first observation is delivered. It escalates
    /// each hard incident `delay_ns` after that step, with the context of the shared cheap rung
    /// `rung`.
    pub fn notice(rung: RungConfig, delay_ns: u64) -> Self {
        Self {
            rung,
            mode: Mode::Notice { delay_ns },
        }
    }

    /// A factory of `oracle_decoy`, which dismisses each decoy anomaly and escalates nothing.
    pub fn decoy(rung: RungConfig) -> Self {
        Self {
            rung,
            mode: Mode::Decoy,
        }
    }

    /// The arm for the segment whose hard incidents `plan` describes and whose public information
    /// is `public`. Called by the harness.
    pub fn build(&self, plan: &OraclePlan, public: &StreamPublic) -> Box<dyn StreamPolicy> {
        let ids_where = |keep: &dyn Fn(&PlanIncident) -> bool| -> BTreeSet<u32> {
            plan.incidents
                .iter()
                .filter(|i| keep(i))
                .map(|i| i.id)
                .collect()
        };
        match self.mode {
            Mode::Escalation => {
                let hard = plan
                    .incidents
                    .iter()
                    .filter(|i| i.hard && !i.decisive.is_empty())
                    .map(|i| HardIncident {
                        id: i.id,
                        decisive: i.decisive.clone(),
                        last: i.decisive.iter().map(|o| o.0).max().unwrap_or(0),
                        first: i.first.unwrap_or(i.decisive[0]),
                    })
                    .collect();
                Box::new(StreamArm::with(
                    OracleEscalation {
                        owner: plan.owner.clone(),
                        hard,
                        hard_ids: ids_where(&|i| i.hard),
                        done: BTreeSet::new(),
                    },
                    public,
                    self.rung.clone(),
                ))
            }
            Mode::Selection { delay_ns, hold } => Box::new(StreamArm::with(
                SelectionOracle {
                    owner: plan.owner.clone(),
                    hard_ids: ids_where(&|i| i.hard),
                    delay_ns,
                    hold_until_asked: hold,
                },
                public,
                self.rung.clone(),
            )),
            Mode::SelectionContext { delay_ns } => Box::new(StreamArm::with(
                SelectionContextOracle {
                    owner: plan.owner.clone(),
                    decisive: plan
                        .incidents
                        .iter()
                        .filter(|i| i.hard)
                        .map(|i| (i.id, i.decisive.clone()))
                        .collect(),
                    delay_ns,
                    asked: BTreeSet::new(),
                },
                public,
                self.rung.clone(),
            )),
            Mode::Notice { delay_ns } => Box::new(StreamArm::with(
                NoticeOracle::from_plan(plan, delay_ns),
                public,
                self.rung.clone(),
            )),
            Mode::Decoy => Box::new(StreamArm::with(
                DecoyOracle {
                    owner: plan.owner.clone(),
                    decoy_ids: ids_where(&|i| i.decoy),
                },
                public,
                self.rung.clone(),
            )),
        }
    }
}

struct HardIncident {
    id: u32,
    decisive: Vec<ObsId>,
    /// The id of its last decisive observation.
    last: u32,
    /// Its first observation, the focus when no noticed anomaly is about it.
    first: ObsId,
}

/// The privileged rule. Has no public constructor.
struct OracleEscalation {
    /// The incident each observation belongs to.
    owner: Vec<Option<u32>>,
    hard: Vec<HardIncident>,
    hard_ids: BTreeSet<u32>,
    done: BTreeSet<u32>,
}

impl OracleEscalation {
    fn incident_of(&self, obs: ObsId) -> Option<u32> {
        self.owner.get(obs.0 as usize).copied().flatten()
    }
}

impl EscalationRule for OracleEscalation {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn role(&self) -> ArmRole {
        ArmRole::Privileged
    }

    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        // Every escalation of this arm is direct: it does not wait to notice anything.
        Vec::new()
    }

    fn holds(&self, view: &AnomalyView) -> bool {
        view.pending > 0
            || (view.answered == 0
                && self
                    .incident_of(view.anchor)
                    .is_some_and(|i| self.hard_ids.contains(&i)))
    }

    fn direct(&mut self, ctx: &DirectCtx<'_>) -> Vec<DirectRequest> {
        let mut out = Vec::new();
        for h in &self.hard {
            if self.done.contains(&h.id) || ctx.delivered <= h.last {
                continue;
            }
            // When a noticed anomaly is about this incident, ask about its anchor, so that the
            // answer is the anomaly's and the cheap rung's declaration is superseded.
            let focus = ctx
                .views
                .iter()
                .find(|v| self.incident_of(v.anchor) == Some(h.id))
                .map_or(h.first, |v| v.anchor);
            self.done.insert(h.id);
            out.push(DirectRequest {
                focus,
                context: h.decisive.iter().map(|o| ObsRef::Passive(*o)).collect(),
            });
        }
        out
    }
}

/// The privileged selection rule. Has no public constructor.
struct SelectionOracle {
    /// The incident each observation belongs to.
    owner: Vec<Option<u32>>,
    hard_ids: BTreeSet<u32>,
    delay_ns: u64,
    /// Keep an anomaly the oracle will ask about live until it has asked (work item B2).
    hold_until_asked: bool,
}

impl SelectionOracle {
    /// Whether the anomaly anchored on `anchor` is one this oracle asks about: its anchor belongs
    /// to a hard incident.
    fn is_hard(&self, anchor: ObsId) -> bool {
        self.owner
            .get(anchor.0 as usize)
            .copied()
            .flatten()
            .is_some_and(|i| self.hard_ids.contains(&i))
    }
}

impl EscalationRule for SelectionOracle {
    fn id(&self) -> PolicyId {
        PolicyId::new(SELECTION_ID)
    }

    fn role(&self) -> ArmRole {
        ArmRole::Privileged
    }

    fn may_keep(&self) -> bool {
        self.hold_until_asked
    }

    /// With `hold_until_asked`: a quiet anomaly this oracle will ask about and has not yet asked
    /// about (no escalation proposed for it) is kept live. Once asked, it retires as any does.
    fn keeps(&self, view: &AnomalyView) -> bool {
        self.hold_until_asked && view.attempts == 0 && self.is_hard(view.anchor)
    }

    /// The noticed anomalies whose anchor belongs to a hard incident, once each, `delay_ns` after
    /// they were noticed. Nothing else about the anomaly is read, and the context is the rung's.
    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| {
                v.attempts == 0
                    && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)
                    && self
                        .owner
                        .get(v.anchor.0 as usize)
                        .copied()
                        .flatten()
                        .is_some_and(|i| self.hard_ids.contains(&i))
            })
            .map(|v| v.id)
            .collect()
    }
}

/// One injected notice: the anchor of a hard incident's first observation, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InjectedNotice {
    /// The incident's first observation: the anchor, and the question's focus.
    anchor: ObsId,
    /// The instant of the step at which the anchor was first seen delivered, once it has been.
    noticed_at: Option<Instant>,
    /// Whether the call about it has been made.
    asked: bool,
}

/// The privileged notice rule (R10). Has no public constructor.
///
/// Holds one [`InjectedNotice`] per hard incident that has a first observation, and nothing from
/// the plan but that.
struct NoticeOracle {
    delay_ns: u64,
    /// In stream order of the anchors.
    notices: Vec<InjectedNotice>,
}

impl NoticeOracle {
    /// From the plan's hard incidents and their first observations; no other field is read.
    fn from_plan(plan: &OraclePlan, delay_ns: u64) -> Self {
        let mut anchors: Vec<ObsId> = plan
            .incidents
            .iter()
            .filter(|i| i.hard)
            .filter_map(|i| i.first)
            .collect();
        anchors.sort_unstable();
        anchors.dedup();
        Self {
            delay_ns,
            notices: anchors
                .into_iter()
                .map(|anchor| InjectedNotice {
                    anchor,
                    noticed_at: None,
                    asked: false,
                })
                .collect(),
        }
    }
}

impl EscalationRule for NoticeOracle {
    fn id(&self) -> PolicyId {
        PolicyId::new(NOTICE_ID)
    }

    fn role(&self) -> ArmRole {
        ArmRole::Privileged
    }

    /// The rung's own notices cause no escalation: every call of this arm is about an injected
    /// notice and goes through [`EscalationRule::direct`].
    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }

    /// Notices each hard incident at the first step that finds its first observation delivered,
    /// and asks about it, once, `delay_ns` after that step, with the context the rung's builder
    /// makes for that anchor now.
    fn direct(&mut self, ctx: &DirectCtx<'_>) -> Vec<DirectRequest> {
        let mut out = Vec::new();
        for notice in &mut self.notices {
            if notice.noticed_at.is_none() && notice.anchor.0 < ctx.delivered {
                notice.noticed_at = Some(ctx.now);
            }
            let Some(noticed_at) = notice.noticed_at else {
                continue;
            };
            if notice.asked || ctx.now.0 < noticed_at.0.saturating_add(self.delay_ns) {
                continue;
            }
            notice.asked = true;
            out.push(DirectRequest {
                focus: notice.anchor,
                context: ctx.contexts.at(notice.anchor),
            });
        }
        out
    }
}

/// The privileged selection-and-context rule (R6, supplementary). Has no public constructor.
struct SelectionContextOracle {
    /// The incident each observation belongs to.
    owner: Vec<Option<u32>>,
    /// Each hard incident's decisive observations, in stream order.
    decisive: BTreeMap<u32, Vec<ObsId>>,
    delay_ns: u64,
    /// The noticed anomalies already asked about.
    asked: BTreeSet<u32>,
}

impl EscalationRule for SelectionContextOracle {
    fn id(&self) -> PolicyId {
        PolicyId::new(SELECTION_CONTEXT_ID)
    }

    fn role(&self) -> ArmRole {
        ArmRole::Privileged
    }

    /// Every escalation of this arm is direct: its context is not the rung's.
    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }

    /// The cheap rung's declaration is held back, as for `oracle_selection`, while the call about
    /// the anomaly is unanswered.
    fn holds(&self, view: &AnomalyView) -> bool {
        view.pending > 0 || (self.asked.contains(&view.id) && view.answered == 0)
    }

    /// Each noticed anomaly whose anchor belongs to a hard incident, once, `delay_ns` after it was
    /// noticed, with the incident's decisive observations that have been delivered.
    fn direct(&mut self, ctx: &DirectCtx<'_>) -> Vec<DirectRequest> {
        let mut out = Vec::new();
        for v in ctx.views {
            if v.attempts != 0
                || self.asked.contains(&v.id)
                || ctx.now.0 < v.noticed_at.0.saturating_add(self.delay_ns)
            {
                continue;
            }
            let Some(evidence) = self
                .owner
                .get(v.anchor.0 as usize)
                .copied()
                .flatten()
                .and_then(|i| self.decisive.get(&i))
            else {
                continue;
            };
            self.asked.insert(v.id);
            out.push(DirectRequest {
                focus: v.anchor,
                context: evidence
                    .iter()
                    .filter(|o| o.0 < ctx.delivered)
                    .map(|o| ObsRef::Passive(*o))
                    .collect(),
            });
        }
        out
    }
}

/// The privileged decoy rule. Has no public constructor.
struct DecoyOracle {
    /// The incident each observation belongs to.
    owner: Vec<Option<u32>>,
    decoy_ids: BTreeSet<u32>,
}

impl EscalationRule for DecoyOracle {
    fn id(&self) -> PolicyId {
        PolicyId::new(DECOY_ID)
    }

    fn role(&self) -> ArmRole {
        ArmRole::Privileged
    }

    /// Escalates nothing.
    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }

    /// The noticed anomalies whose anchor belongs to a decoy.
    fn dismissals(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        views
            .iter()
            .filter(|v| {
                self.owner
                    .get(v.anchor.0 as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|i| self.decoy_ids.contains(&i))
            })
            .map(|v| v.id)
            .collect()
    }
}
