//! The bounded working view of the charter (section 3.1).
//!
//! A [`WorkingState`] holds what currently matters: the public information, a bounded window of
//! recent evidence, the unresolved hypotheses, and the pending computations. A component reads it
//! instead of rereading the system's whole history from the ledger.
//!
//! # Eviction rule
//!
//! The evidence window is a recency window. [`WorkingState::admit`] appends an observation at the
//! back and, when the window already holds `capacity` observations, first drops the one admitted
//! longest ago. The rule looks at admission order only, never at an observation's content or its
//! instant, so it is deterministic and the same admit sequence always leaves the same window.
//!
//! This rule is simple on purpose and has a known cost. The earliest `ErrorRate` counter at the
//! fault site is what pins the site (`physics` rule 1, "propagation requires a cause"), and it
//! is also the first thing a recency window drops. Once it is gone, later observations at
//! dependents have no cause in the window and `consistent_hypotheses` can return the empty set.
//! [`crate::ConsistencyVerifier`] reports that as damaged evidence. Retention rules that keep
//! the anchor are a treatment for an experiment (EXP-004 compares window policies); none is
//! built here.

use crate::ComputationRequest;
use gordian_core::Instant;
use gordian_world::{Hypothesis, Observation, PublicInfo};
use std::collections::VecDeque;

/// The bounded working view components read.
///
/// `evidence` and `capacity` are private so that the size bound cannot be bypassed by pushing to
/// the window directly; read them through [`WorkingState::evidence`] and
/// [`WorkingState::capacity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingState {
    /// What a policy may know at episode start. Never contains hidden state.
    pub public: PublicInfo,
    evidence: VecDeque<(Instant, Observation)>,
    capacity: usize,
    /// Unresolved hypotheses with a score. Components do not read or write this field; the
    /// harness keeps it (for instance from [`crate::CountEstimator::scores`]).
    pub hypotheses: Vec<(Hypothesis, u32)>,
    /// Computations requested and not yet run. Kept by the harness, not by components.
    pub pending: Vec<ComputationRequest>,
    /// The current logical time. [`WorkingState::admit`] raises it to the instant of the
    /// observation admitted when that instant is later; the harness may also set it directly.
    pub now: Instant,
}

impl WorkingState {
    /// An empty working state over `public`, holding at most `capacity` observations. A capacity
    /// of zero is allowed and admits nothing.
    pub fn new(public: PublicInfo, capacity: usize) -> Self {
        Self {
            public,
            evidence: VecDeque::with_capacity(capacity.min(4096)),
            capacity,
            hypotheses: Vec::new(),
            pending: Vec::new(),
            now: Instant::ZERO,
        }
    }

    /// Admit `obs`, observed at `at`, dropping the oldest admitted observation if the window is
    /// full (see the module documentation for the rule).
    ///
    /// Observations are kept in admission order, which is the order the checker reads them in;
    /// admit them in stream order.
    pub fn admit(&mut self, at: Instant, obs: Observation) {
        self.now = self.now.max(at);
        if self.capacity == 0 {
            return;
        }
        if self.evidence.len() >= self.capacity {
            self.evidence.pop_front();
        }
        self.evidence.push_back((at, obs));
    }

    /// Number of observations in the window. Never exceeds [`WorkingState::capacity`].
    ///
    /// Only the evidence window is bounded by `capacity`; `hypotheses` and `pending` are the
    /// harness's to bound.
    pub fn size(&self) -> usize {
        self.evidence.len()
    }

    /// The declared maximum window size.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The window, oldest admitted first.
    pub fn evidence(&self) -> &VecDeque<(Instant, Observation)> {
        &self.evidence
    }

    /// The window copied into one contiguous vector, oldest first, for the world's checker,
    /// which takes a slice.
    pub(crate) fn evidence_vec(&self) -> Vec<(Instant, Observation)> {
        let (front, back) = self.evidence.as_slices();
        let mut out = Vec::with_capacity(self.evidence.len());
        out.extend_from_slice(front);
        out.extend_from_slice(back);
        out
    }
}
