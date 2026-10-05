//! Evaluator-side question records for R8, the distractor sensitivity of a real model.
//!
//! **Evaluator-side only.** A record holds the truth beside what a question may show a model, so
//! it exists only under the cargo feature `reveal-hidden-state` (or in this crate's tests) and
//! must never be read by a policy. The prompt builder that consumes it is an evaluator-side
//! script and shows a model the services, the focus observation, the decisive evidence and
//! the distractors it draws, and nothing else.
//!
//! For each incident of the requested tiers the record holds:
//!
//! - the **focus**: the first observation labelled with the incident, which names the incident
//!   the question is about;
//! - the incident's **decisive evidence**, in full and in stream order;
//! - its **truth**;
//! - a **pool** of candidate distractors: every other observation within `window_ns` of the
//!   focus (both sides) that belongs to the background or to another incident, in stream order.
//!   The focus incident's own observations that are not decisive (its first moments, its
//!   heartbeats, its closure) are in neither the decisive evidence nor the pool.
//!
//! A slow leak is never a question (its evidence is a benign series, a different kind of
//! question); decoys are never questions. Which questions a run uses, how many distractors it
//! draws and how it renders them are the runner's, not this module's.

use crate::kinds::{Diagnosis, ObsId, Tier};
use crate::labels::ObsLabel;
use crate::oracle::{IncidentTruth, reveal};
use crate::stream::Stream;
use gordian_world::{FaultKind, Observation, Service, ServiceId};
use serde::Serialize;

/// The half-width of the distractor pool's window around the focus: the plan's 40 s.
pub const POOL_WINDOW_NS: u64 = 40_000_000_000;

/// One observation of the stream as a question carries it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObsRecord {
    /// Position in the delivered stream.
    pub id: u32,
    /// Delivery instant, nanoseconds.
    pub at_ns: u64,
    /// The observation itself.
    pub obs: Observation,
}

/// One question: an incident, its evidence and its candidate distractors.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuestionRecord {
    /// The stream's seed.
    pub seed: u64,
    /// The incident's id within the stream.
    pub incident: u32,
    /// Plain or hard (never a decoy).
    pub tier: Tier,
    /// `Known:<kind>`, `Compound`, `Cascade` or `SplitBrain`.
    pub family: String,
    /// A plain incident whose signature leaves two kinds open (one probe would settle it).
    pub duo: bool,
    /// For a hard incident: `contradict` when its first moments already break the public rules,
    /// `mimic` when they imitate a plain incident.
    pub mode: Option<String>,
    /// The cascade's partner or the split brain's peer.
    pub other: Option<ServiceId>,
    /// The incident this one repeats, if any.
    pub recurrence_of: Option<u32>,
    /// The true hypothesis.
    pub truth: Diagnosis,
    /// The incident's onset, nanoseconds.
    pub onset_ns: u64,
    /// The focus observation.
    pub focus: ObsRecord,
    /// The public graph at time zero.
    pub services: Vec<Service>,
    /// All of the incident's decisive evidence, in stream order.
    pub decisive: Vec<ObsRecord>,
    /// Number of observations in the pool.
    pub pool_size: usize,
    /// Candidate distractors, in stream order.
    pub pool: Vec<ObsRecord>,
}

fn record(stream: &Stream, id: ObsId) -> ObsRecord {
    let (at, obs) = &stream.events()[id.0 as usize];
    ObsRecord {
        id: id.0,
        at_ns: at.0,
        obs: obs.clone(),
    }
}

fn family_name(inc: &IncidentTruth) -> Option<String> {
    if let Some(kind) = inc.shape.known_kind {
        let name = match kind {
            FaultKind::ResourceExhausted => "ResourceExhausted",
            FaultKind::ConfigDrift => "ConfigDrift",
            FaultKind::DependencyDown => "DependencyDown",
            FaultKind::CredentialExpired => "CredentialExpired",
            FaultKind::Intermittent => "Intermittent",
        };
        return Some(format!("Known:{name}"));
    }
    inc.shape.hard_kind.and_then(|k| match k {
        crate::HardKind::Compound => Some("Compound".to_string()),
        crate::HardKind::Cascade => Some("Cascade".to_string()),
        crate::HardKind::SplitBrain => Some("SplitBrain".to_string()),
        // A leak's evidence is a benign series; it is not a question here.
        crate::HardKind::SlowLeak => None,
    })
}

/// The questions of one stream: every plain incident (when `plain`) and every hard incident
/// other than a slow leak (when `hard`), in incident order, each with its pool of distractors
/// within `window_ns` of the focus.
pub fn questions(stream: &Stream, window_ns: u64, plain: bool, hard: bool) -> Vec<QuestionRecord> {
    let truth = reveal(stream);
    let mut out = Vec::new();
    for inc in &truth.incidents {
        let wanted = match inc.tier {
            Tier::Plain => plain,
            Tier::Hard => hard,
            Tier::Decoy => false,
        };
        if !wanted {
            continue;
        }
        let Some(family) = family_name(inc) else {
            continue;
        };
        let Some(&focus_id) = inc.observations.first() else {
            continue;
        };
        let focus = record(stream, focus_id);
        let lo = focus.at_ns.saturating_sub(window_ns);
        let hi = focus.at_ns.saturating_add(window_ns);
        let pool: Vec<ObsRecord> = stream
            .events()
            .iter()
            .enumerate()
            .filter(|(i, (at, _))| {
                at.0 >= lo
                    && at.0 <= hi
                    && match truth.labels[*i] {
                        ObsLabel::Background(_) => true,
                        ObsLabel::Incident { id, .. } => id != inc.id,
                    }
            })
            .map(|(i, _)| record(stream, ObsId(i as u32)))
            .collect();
        out.push(QuestionRecord {
            seed: stream.params.seed,
            incident: inc.id,
            tier: inc.tier,
            family,
            duo: inc.shape.duo,
            mode: inc.shape.contradicts_early.map(|c| {
                if c {
                    "contradict".to_string()
                } else {
                    "mimic".to_string()
                }
            }),
            other: inc.shape.other,
            recurrence_of: inc.recurrence_of,
            truth: inc.truth,
            onset_ns: inc.onset_ns,
            focus,
            services: stream.services.clone(),
            decisive: inc.decisive.iter().map(|id| record(stream, *id)).collect(),
            pool_size: pool.len(),
            pool,
        });
    }
    out
}
