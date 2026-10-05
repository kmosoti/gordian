//! Hidden labels on every observation of the stream. Reachable only through `oracle`.

use serde::{Deserialize, Serialize};

/// What a background observation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NoiseKind {
    /// A benign counter reading.
    Benign,
    /// A free-form message.
    FreeForm,
    /// A catalogue message at a random service.
    CatalogueStray,
    /// An isolated abnormal counter reading.
    Blip,
    /// One observation of a burst that looks like the start of a plain incident and stops.
    MiniBurst,
    /// An unchanged configuration snapshot.
    Snapshot,
}

/// What an observation of an incident is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceRole {
    /// Part of the first moments of a hard incident or a decoy: shared by the two tiers, never
    /// decisive.
    Presentation,
    /// A heartbeat reading of a live incident, or the reading series of a leak.
    Pulse,
    /// Decisive evidence: a plain incident's signature, a hard incident's phase-2 evidence, or
    /// the first benign heartbeats after a decoy resolves. The reasoner's `q` counts these.
    Decisive,
    /// A benign reading after an incident closed or a decoy's recovery was already shown.
    Closure,
}

/// The label of one observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObsLabel {
    /// Belongs to no incident.
    Background(NoiseKind),
    /// Belongs to the incident with this id.
    Incident {
        /// The incident's id, dense from zero in order of arrival.
        id: u32,
        /// What the observation is to that incident.
        role: EvidenceRole,
    },
}
