//! Fault kinds and fault instances.

use crate::graph::ServiceId;
use gordian_core::Instant;
use serde::{Deserialize, Serialize};

/// What is wrong. The symptoms each kind produces are public and live in `physics`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FaultKind {
    /// The service's resource is saturated.
    ResourceExhausted,
    /// The service's configuration changed and is now rejected.
    ConfigDrift,
    /// The service is down; its dependents cannot reach it.
    DependencyDown,
    /// The service's credential expired.
    CredentialExpired,
    /// The service flaps between working and failing.
    Intermittent,
}

impl FaultKind {
    /// Every kind, in declaration order.
    pub const ALL: [FaultKind; 5] = [
        FaultKind::ResourceExhausted,
        FaultKind::ConfigDrift,
        FaultKind::DependencyDown,
        FaultKind::CredentialExpired,
        FaultKind::Intermittent,
    ];
}

/// A claim about the hidden state: `None` is "no fault", `Some((kind, site))` is a fault.
pub type Hypothesis = Option<(FaultKind, ServiceId)>;

/// A fault in a generated episode. Hidden state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// What is wrong.
    pub kind: FaultKind,
    /// Where. For `DependencyDown` the site is the service that is down.
    pub site: ServiceId,
    /// When the first symptom is emitted. The underlying cause exists from time zero: probes see
    /// it at any time, passive symptoms begin here.
    #[serde(with = "crate::timeserde::instant")]
    pub onset: Instant,
    /// Whether a miss or a wrong correction is scored under the separate critical bound.
    pub critical: bool,
}

impl Fault {
    /// The fault as a hypothesis.
    pub fn hypothesis(&self) -> Hypothesis {
        Some((self.kind, self.site))
    }
}
