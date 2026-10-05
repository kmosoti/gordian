//! The privileged arm: `oracle_escalation`.
//!
//! The charter's "oracle escalation (privileged)": whether the world has measurable headroom for
//! escalation control. It is the only stream policy that uses the stream's truth, and this is the
//! only arm file that names it (`scripts/check-no-oracle.sh` bans the truth's types from
//! `arms/`; the harness and this file are where they may appear).
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
//! # How the truth reaches it, and only it
//!
//! [`StreamPolicy`] and [`EscalationRule`] have no place for a truth. [`OracleFactory::build`]
//! takes one, and is called by `harness::run_segment_privileged`, the one entry point that takes a
//! factory instead of an arm. `OracleEscalation`, the rule it builds, is private to this file:
//!
//! ```compile_fail
//! // `OracleEscalation` has no public name, so nothing else can construct one from a truth.
//! use gordian_run::stream::privileged::OracleEscalation;
//! ```
//!
//! The manifest rejects this arm unless its name contains `privileged`, and every `results.csv`
//! row carries `arm_role = privileged`.

use super::arms::rung::{AnomalyView, RungConfig};
use super::arms::{ArmRole, DirectCtx, DirectRequest, EscalationRule, StreamArm, StreamPolicy};
use crate::policy::PolicyId;
use gordian_core::Instant;
use gordian_stream::{ObsId, ObsRef, StreamPublic, Tier};
use gordian_stream_reveal::StreamTruth;
use std::collections::BTreeSet;

/// The id of `oracle_escalation`.
pub const ID: &str = "oracle_escalation";

/// Builds the privileged arm for a segment once the harness hands over its truth.
///
/// The only way to build one. The constructor takes no truth, so a factory can be made anywhere
/// but can only be used by the harness, which holds the truth. The rule it builds is private to
/// this file.
#[derive(Debug, Clone)]
pub struct OracleFactory {
    rung: RungConfig,
}

impl OracleFactory {
    /// A factory whose arm shares the cheap rung `rung`.
    pub fn new(rung: RungConfig) -> Self {
        Self { rung }
    }

    /// The arm for the segment whose truth is `truth` and whose public information is `public`.
    /// Called by the harness.
    pub fn build(&self, truth: &StreamTruth, public: &StreamPublic) -> Box<dyn StreamPolicy> {
        let owner: Vec<Option<u32>> = (0..truth.labels.len())
            .map(|i| truth.incident_of(ObsId(i as u32)))
            .collect();
        let hard = truth
            .incidents
            .iter()
            .filter(|i| i.tier == Tier::Hard && !i.decisive.is_empty())
            .map(|i| HardIncident {
                id: i.id,
                decisive: i.decisive.clone(),
                last: i.decisive.iter().map(|o| o.0).max().unwrap_or(0),
                first: i.observations.first().copied().unwrap_or(i.decisive[0]),
            })
            .collect();
        let hard_ids: BTreeSet<u32> = truth
            .incidents
            .iter()
            .filter(|i| i.tier == Tier::Hard)
            .map(|i| i.id)
            .collect();
        Box::new(StreamArm::with(
            OracleEscalation {
                owner,
                hard,
                hard_ids,
                done: BTreeSet::new(),
            },
            public,
            self.rung.clone(),
        ))
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
