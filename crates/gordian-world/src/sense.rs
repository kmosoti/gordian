//! What a policy can sense: passive observations and probe results.
//!
//! Every [`Observation`] is a measurement in the ledger's sense: it makes no interpretation.
//! Mapping an observation to a fault hypothesis is what `physics::consistent_hypotheses` does,
//! and a component that does so is producing a hypothesis, not a measurement.

use crate::graph::ServiceId;
use serde::{Deserialize, Serialize};

/// Name of a counter. A counter at or above `physics::HIGH` is abnormal; below it, benign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CounterName {
    /// Errors per second, percent of requests.
    ErrorRate,
    /// Request latency, scaled so that 50 is the alarm threshold.
    Latency,
    /// Resource saturation, percent.
    Saturation,
    /// Authentication failures per second.
    AuthFailures,
    /// Process restarts per minute.
    Restarts,
}

impl CounterName {
    /// Every counter name.
    pub const ALL: [CounterName; 5] = [
        CounterName::ErrorRate,
        CounterName::Latency,
        CounterName::Saturation,
        CounterName::AuthFailures,
        CounterName::Restarts,
    ];
}

/// Severity attached to a message by its emitter. The rules do not depend on it; a policy that
/// ranks by severity can therefore starve a true low-severity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Lowest.
    Low,
    /// Middle.
    Medium,
    /// High.
    High,
    /// Highest.
    Critical,
}

impl Severity {
    /// Every severity, ascending.
    pub const ALL: [Severity; 4] = [
        Severity::Low,
        Severity::Medium,
        Severity::High,
        Severity::Critical,
    ];
}

/// A diagnostic probe a policy can pay for. What each reveals is table `physics::probe_result`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ProbeKind {
    /// Is anything wrong at the target?
    HealthCheck,
    /// Is the target's resource exhausted?
    ResourceUsage,
    /// What is the target's configuration hash now?
    ConfigSnapshot,
    /// Is the target's credential expired?
    CredentialCheck,
    /// First of two samples that jointly decide `DependencyDown` versus `Intermittent`.
    LatencySample,
    /// Second of those two samples.
    ErrorSample,
}

impl ProbeKind {
    /// Every probe kind.
    pub const ALL: [ProbeKind; 6] = [
        ProbeKind::HealthCheck,
        ProbeKind::ResourceUsage,
        ProbeKind::ConfigSnapshot,
        ProbeKind::CredentialCheck,
        ProbeKind::LatencySample,
        ProbeKind::ErrorSample,
    ];
}

/// A probe: a kind aimed at a target service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Probe {
    /// What to measure.
    pub kind: ProbeKind,
    /// Where to measure it.
    pub target: ServiceId,
}

/// The result of a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProbeResult {
    /// The probe's predicate holds at the target.
    Positive,
    /// It does not.
    Negative,
    /// The target's configuration hash.
    ConfigHash(u64),
    /// The probe could not decide, and suggests running another probe.
    Inconclusive {
        /// The suggested follow-up, if any.
        suggest: Option<Probe>,
    },
}

/// One observation. Probe results and correction effects are observations too: they are derived
/// from hidden state, delivered through [`crate::step::Outcome`], and cost budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Observation {
    /// A counter reading.
    Counter {
        /// Where it was read.
        service: ServiceId,
        /// Which counter.
        name: CounterName,
        /// The reading.
        value: u64,
    },
    /// A log message. `text_id` below `physics::CATALOGUE_LIMIT` is a catalogue message with a
    /// public meaning; any other id is free-form text with none.
    Message {
        /// The emitting service.
        service: ServiceId,
        /// Identifier of the message text.
        text_id: u64,
        /// Severity attached by the emitter.
        severity: Severity,
    },
    /// A configuration snapshot of a service.
    Snapshot {
        /// The service.
        service: ServiceId,
        /// Its configuration hash when the snapshot was taken.
        config_hash: u64,
    },
    /// The result of a paid probe.
    Probed {
        /// The probe that was run.
        probe: Probe,
        /// What it returned.
        result: ProbeResult,
    },
    /// The visible effect of a paid correction attempt.
    Correction {
        /// Where the correction was applied.
        site: ServiceId,
        /// True when a fault existed at `site` and the correction resolved it.
        resolved: bool,
    },
}
