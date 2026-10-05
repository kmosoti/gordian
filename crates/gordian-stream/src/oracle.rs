//! Privileged access to hidden stream state.
//!
//! **Only the stream evaluator and the privileged oracle arm may call anything here.** A policy
//! that imports this module fails review. The module exists only under the cargo feature
//! `reveal-hidden-state` (or in this crate's own tests). Cargo unifies features across a build,
//! so the feature is a guard rather than a proof; `scripts/check-no-oracle.sh` is the second
//! guard.
//!
//! What is here, and nothing else, is hidden: the tier, deadline, criticality, true hypothesis,
//! difficulty and structure of every incident; the label of every observation (background or
//! incident, and for an incident its role, which includes "decisive"); the resolved regime
//! changes; and, from a simulator, every reasoner call with its `q`, `d`, `p` and whether it was
//! correct, and every declaration. The evaluator scores from [`StreamTruth`] and a recorded
//! trajectory, not from a [`Stream`], so that hand-written fixtures stay independent of the
//! generator.

use crate::incident::{Family, Mode};
use crate::kinds::{Diagnosis, HardKind, ObsId, Tier};
pub use crate::labels::{EvidenceRole, NoiseKind, ObsLabel};
pub use crate::regime::{Regime, RegimeDetail};
use crate::sim::StreamSimulator;
use crate::stream::Stream;
use gordian_world::{FaultKind, Service, ServiceId};
use serde::{Deserialize, Serialize};

/// The structure of an incident, for the evaluator and for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeTruth {
    /// The known kind of a plain incident.
    pub known_kind: Option<FaultKind>,
    /// A plain incident whose signature leaves two kinds open and needs one probe.
    pub duo: bool,
    /// The hard kind a hard incident has, or a decoy imitates.
    pub hard_kind: Option<HardKind>,
    /// For a hard incident or decoy: true when its first moments already break the first world's
    /// rules, false when they imitate a plain incident.
    pub contradicts_early: Option<bool>,
    /// The cascade's partner or the split brain's peer.
    pub other: Option<ServiceId>,
    /// The known kind the first moments lead the cheap rung to.
    pub mimics: Option<FaultKind>,
    /// A compound's two kinds, the one that shows first first.
    pub pair: Option<(FaultKind, FaultKind)>,
}

/// The truth about one incident.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncidentTruth {
    /// Dense from zero in order of arrival.
    pub id: u32,
    /// Index of the arrival that produced it. Larger than `id` when earlier arrivals were
    /// dropped because no suitable service was free.
    pub arrival: u32,
    /// Plain, hard or decoy.
    pub tier: Tier,
    /// Whether a miss is scored under the critical bound.
    pub critical: bool,
    /// When the first symptom is emitted.
    pub onset_ns: u64,
    /// A correct declaration after this instant is a miss. `None` for a decoy.
    pub deadline_ns: Option<u64>,
    /// The true hypothesis; `None` for a decoy.
    pub truth: Diagnosis,
    /// The incident's difficulty `d`.
    pub difficulty: f64,
    /// A decoy's resolution, or the others' closure.
    pub live_end_ns: u64,
    /// The services of the incident stay reserved until here: no other incident starts on them
    /// before this instant.
    pub busy_until_ns: u64,
    /// The incident this one repeats, if any.
    pub recurrence_of: Option<u32>,
    /// The services the incident occupies (site first).
    pub occupies: Vec<ServiceId>,
    /// Its structure.
    pub shape: ShapeTruth,
    /// Every decisive observation of the incident, in stream order.
    pub decisive: Vec<ObsId>,
    /// Every observation labelled with the incident, in stream order.
    pub observations: Vec<ObsId>,
}

/// The truth about a stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamTruth {
    /// Logical duration, nanoseconds.
    pub duration_ns: u64,
    /// The graph at time zero.
    pub services: Vec<Service>,
    /// Every incident.
    pub incidents: Vec<IncidentTruth>,
    /// The label of every observation, parallel to the delivered stream.
    pub labels: Vec<ObsLabel>,
    /// Resolved regime changes, in time order.
    pub regimes: Vec<TruthRegime>,
    /// Arrivals dropped because no suitable service was free.
    pub skipped_arrivals: u32,
}

/// A resolved regime change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruthRegime {
    /// When it takes effect.
    pub at_ns: u64,
    /// What it did.
    pub detail: RegimeDetail,
}

impl StreamTruth {
    /// The incident an observation belongs to, if any.
    pub fn incident_of(&self, id: ObsId) -> Option<u32> {
        match self.labels.get(id.0 as usize)? {
            ObsLabel::Incident { id, .. } => Some(*id),
            ObsLabel::Background(_) => None,
        }
    }
}

/// Reveal the hidden state of `stream`.
pub fn reveal(stream: &Stream) -> StreamTruth {
    let mut incidents = Vec::with_capacity(stream.incidents.len());
    for inc in &stream.incidents {
        let mut decisive = Vec::new();
        let mut observations = Vec::new();
        for (i, label) in stream.labels.iter().enumerate() {
            if let ObsLabel::Incident { id, role } = label
                && *id == inc.id
            {
                observations.push(ObsId(i as u32));
                if *role == EvidenceRole::Decisive {
                    decisive.push(ObsId(i as u32));
                }
            }
        }
        let shape = match inc.family {
            Family::Known { kind, duo } => ShapeTruth {
                known_kind: Some(kind),
                duo,
                hard_kind: None,
                contradicts_early: None,
                other: None,
                mimics: None,
                pair: None,
            },
            f => ShapeTruth {
                known_kind: None,
                duo: false,
                hard_kind: f.hard_kind(),
                contradicts_early: match f {
                    Family::Compound { mode, .. }
                    | Family::Cascade { mode, .. }
                    | Family::SplitBrain { mode, .. } => Some(mode == Mode::Contradict),
                    _ => None,
                },
                other: f.others(),
                mimics: f.mimic(),
                pair: match f {
                    Family::Compound { a, b, .. } => Some((a, b)),
                    _ => None,
                },
            },
        };
        incidents.push(IncidentTruth {
            id: inc.id,
            arrival: inc.arrival,
            tier: inc.tier,
            critical: inc.critical,
            onset_ns: inc.onset.0,
            deadline_ns: inc.deadline.map(|d| d.0),
            truth: inc.truth,
            difficulty: inc.difficulty,
            live_end_ns: inc.live_end.0,
            busy_until_ns: inc.busy_until.0,
            recurrence_of: inc.recurrence_of,
            occupies: inc.occupies.clone(),
            shape,
            decisive,
            observations,
        });
    }
    StreamTruth {
        duration_ns: stream.params.duration_ns,
        services: stream.services.clone(),
        incidents,
        labels: stream.labels.clone(),
        regimes: stream
            .regimes
            .iter()
            .map(|r| TruthRegime {
                at_ns: r.at.0,
                detail: r.detail,
            })
            .collect(),
        skipped_arrivals: stream.skipped,
    }
}

/// One reasoner call and the hidden facts behind its answer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CallTrace {
    /// Index among accepted calls.
    pub call: u32,
    /// The incident asked about; `None` for a question about background.
    pub incident: Option<u32>,
    /// The observation the question was about: the focus of the escalation that made the call.
    pub focus: ObsId,
    /// The fingerprint of the question (focus and sorted context): the last word of the draw's
    /// key.
    pub fingerprint: u64,
    /// When the call was made, nanoseconds.
    pub at_ns: u64,
    /// When the answer was delivered, nanoseconds.
    pub ready_at_ns: u64,
    /// References in the context.
    pub refs: u32,
    /// Fraction of the incident's decisive evidence in the context.
    pub q: f64,
    /// The incident's difficulty.
    pub d: f64,
    /// The probability that the call was informed, `h(q, d)`.
    pub h: f64,
    /// The share of the guess distribution that falls on the truth: the accuracy of a
    /// truth-independent guess from this context.
    pub p0: f64,
    /// The probability the answer was right, `p0 + (1 - p0) h`.
    pub p: f64,
    /// Whether this call was informed (answered the truth because of decisive evidence).
    pub informed: bool,
    /// Whether the answer equals the truth (an informed call, or a lucky guess).
    pub correct: bool,
    /// What was answered.
    pub diagnosis: Diagnosis,
}

/// One declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationTrace {
    /// When, nanoseconds.
    pub at_ns: u64,
    /// The anchor observation.
    pub anchor: ObsId,
    /// What was declared.
    pub diagnosis: Diagnosis,
}

/// Every reasoner call a simulator accepted, in order.
pub fn calls(sim: &StreamSimulator) -> Vec<CallTrace> {
    sim.hidden()
        .1
        .iter()
        .enumerate()
        .map(|(i, c)| CallTrace {
            call: i as u32,
            incident: c.incident,
            focus: c.focus,
            fingerprint: c.fingerprint,
            at_ns: c.at.0,
            ready_at_ns: c.ready_at.0,
            refs: c.refs,
            q: c.q,
            d: c.d,
            h: c.h,
            p0: c.p0,
            p: c.p,
            informed: c.informed,
            correct: c.correct,
            diagnosis: c.diagnosis,
        })
        .collect()
}

/// Every declaration a simulator accepted, in order.
pub fn declarations(sim: &StreamSimulator) -> Vec<DeclarationTrace> {
    sim.hidden()
        .2
        .iter()
        .map(|d| DeclarationTrace {
            at_ns: d.at.0,
            anchor: d.anchor,
            diagnosis: d.diagnosis,
        })
        .collect()
}

/// The stream a simulator was built from.
pub fn stream_of(sim: &StreamSimulator) -> &Stream {
    sim.hidden().0
}
