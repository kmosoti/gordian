//! What the evaluator knows about an episode: its class and its true faults.

use gordian_world::{Episode, EpisodeClass, Fault, FaultKind, ServiceId};
use serde::{Deserialize, Serialize};

/// The hidden truth of one episode, as the evaluator needs it.
///
/// Fixtures build a `Truth` by hand. [`Truth::from_episode`] is the only place in this crate that
/// calls the world's oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Truth {
    /// The episode class. `NoFault` means the correct answer is "no fault"; every other class
    /// has at least one fault.
    pub class: EpisodeClass,
    /// The true faults. Empty exactly when `class` is `NoFault`. The generator makes at most one;
    /// the scoring rules (`RULES.md`, R19) are stated for any number.
    pub faults: Vec<Fault>,
}

impl Truth {
    /// The truth of a generated episode, read through `gordian_world::oracle::reveal` and the
    /// episode's own spec.
    pub fn from_episode(episode: &Episode) -> Truth {
        let hidden = gordian_world::oracle::reveal(episode);
        Truth {
            class: episode.spec().class,
            faults: hidden.faults,
        }
    }

    /// True when the class is `NoFault`.
    pub(crate) fn is_no_fault(&self) -> bool {
        self.class == EpisodeClass::NoFault
    }

    /// True when `NoFault` has no faults and every other class has at least one (`RULES.md`, R18).
    pub(crate) fn is_consistent(&self) -> bool {
        self.is_no_fault() == self.faults.is_empty()
    }

    /// True when some true fault has this kind and site (`RULES.md`, R2, R3, R19).
    pub(crate) fn has_fault(&self, kind: FaultKind, site: ServiceId) -> bool {
        self.faults.iter().any(|f| f.kind == kind && f.site == site)
    }

    /// True when some true fault is critical (`RULES.md`, R7).
    pub(crate) fn has_critical_fault(&self) -> bool {
        self.faults.iter().any(|f| f.critical)
    }
}
