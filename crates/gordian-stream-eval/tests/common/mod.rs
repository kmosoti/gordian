//! Shared by the test files: building a `StreamTruth` by hand, and the step and call builders
//! used by the generated-trajectory tests. Nothing here calls the stream's generator.

#![allow(dead_code)]

use gordian_core::Instant;
use gordian_stream::oracle::{
    EvidenceRole, IncidentTruth, NoiseKind, ObsLabel, ShapeTruth, StreamTruth,
};
use gordian_stream::{ObsId, StreamHypothesis, Tier};
use serde::Deserialize;

/// The fixture form of an incident: only the fields the evaluator reads. Everything else in an
/// `IncidentTruth` is filled with a neutral value, so a fixture states what the rules depend on
/// and nothing more.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactIncident {
    /// Defaults to the incident's position; set to build an inconsistent truth (S27).
    pub id: Option<u32>,
    pub tier: Tier,
    pub critical: bool,
    pub onset_ns: u64,
    pub deadline_ns: Option<u64>,
    pub truth: Option<StreamHypothesis>,
}

/// The fixture form of a truth: duration, incidents, and one label per observation (`null` for
/// background, an incident number for an observation of that incident).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactTruth {
    pub duration_ns: u64,
    pub incidents: Vec<CompactIncident>,
    pub labels: Vec<Option<u32>>,
}

impl CompactTruth {
    pub fn build(&self) -> StreamTruth {
        hand_truth(self.duration_ns, &self.incidents, &self.labels)
    }
}

pub fn hand_truth(
    duration_ns: u64,
    incidents: &[CompactIncident],
    labels: &[Option<u32>],
) -> StreamTruth {
    let incident_truths = incidents
        .iter()
        .enumerate()
        .map(|(position, c)| IncidentTruth {
            id: c.id.unwrap_or(position as u32),
            arrival: position as u32,
            tier: c.tier,
            critical: c.critical,
            onset_ns: c.onset_ns,
            deadline_ns: c.deadline_ns,
            truth: c.truth,
            difficulty: 0.0,
            live_end_ns: c.deadline_ns.unwrap_or(c.onset_ns),
            busy_until_ns: c.deadline_ns.unwrap_or(c.onset_ns),
            recurrence_of: None,
            occupies: Vec::new(),
            shape: ShapeTruth {
                known_kind: None,
                duo: false,
                hard_kind: None,
                contradicts_early: None,
                other: None,
                mimics: None,
                pair: None,
            },
            decisive: Vec::new(),
            observations: labels
                .iter()
                .enumerate()
                .filter(|(_, l)| **l == Some(c.id.unwrap_or(position as u32)))
                .map(|(i, _)| ObsId(i as u32))
                .collect(),
        })
        .collect();
    StreamTruth {
        duration_ns,
        services: Vec::new(),
        incidents: incident_truths,
        labels: labels
            .iter()
            .map(|l| match l {
                None => ObsLabel::Background(NoiseKind::Benign),
                Some(id) => ObsLabel::Incident {
                    id: *id,
                    role: EvidenceRole::Pulse,
                },
            })
            .collect(),
        regimes: Vec::new(),
        skipped_arrivals: 0,
    }
}

pub fn ns(secs: u64) -> Instant {
    Instant(secs * 1_000_000_000)
}
