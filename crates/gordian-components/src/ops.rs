//! Counted operations (work item A8b): what a component call did, in declared units.
//!
//! The charter's cost `C` was first defined as wall time at the harness boundary. On this VM wall
//! time carries bursts of stolen CPU time that land on one copy of an episode and not another
//! (`docs/review-log.md`, A8), so a ratio of totals moves by several percent with nothing
//! changed. A *count* of the work done does not move. Each component counts, in its own units,
//! the steps of the loops that dominate its time: observations scanned, records compared,
//! worlds and observations the checker evaluated, hypotheses a probe result was checked against,
//! entries and candidates encoded. A weight in picoseconds per unit turns the counts into
//! modelled nanoseconds. The weights are fitted to the *minimum* of repeated timings (a minimum,
//! because interference only ever adds time) of fixed windows, and then scaled to what a call
//! costs inside the harness, which is more than in a loop; see `CALIBRATION.md`, section 9.
//!
//! # What a count is
//!
//! An [`Ops`] belongs to one call of one component. It is a function of the call's input and of
//! the component's configuration: no clock, no randomness, nothing read from outside. It is
//! produced by [`crate::Component::run_counted`], next to the output it describes, and the only
//! constructors outside this crate are [`Ops::zero`], which says nothing was done, and the
//! arithmetic on existing counts. A policy never receives an `Ops`: the harness keeps them and
//! hands a policy only the [`crate::ComponentOutput`].
//!
//! # Units
//!
//! The units of each component are listed in its module (`UNITS`) and by [`units`]. A unit is
//! kept only if it is something the code does a measurable number of times that varies with
//! content, or a per-call constant that the fit shows is not negligible (`calls`). The weights
//! have no intercept other than the explicit `calls` unit: a per-call overhead is work (an
//! allocation, an entry's envelope) and is counted as work.

use gordian_core::ComponentId;

/// The most units any component has.
pub const MAX_UNITS: usize = 12;

/// One unit of operation and its weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    /// Stable name, as written by the calibration program and the calibration record.
    pub name: &'static str,
    /// Modelled cost of one such operation, in picoseconds (so that weights of a fraction of a
    /// nanosecond keep their precision in integer arithmetic).
    pub weight_ps: u64,
}

/// The work of one call (or the sum of several calls) of one component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ops {
    component: ComponentId,
    counts: [u64; MAX_UNITS],
}

impl Ops {
    /// No work, attributed to `component`. For a component that did nothing (it failed, or it is
    /// a test double) and as the start of a running sum.
    pub const fn zero(component: ComponentId) -> Self {
        Self {
            component,
            counts: [0; MAX_UNITS],
        }
    }

    /// Add `n` operations of unit number `unit` (an index into [`units`] of the component).
    pub(crate) fn add(&mut self, unit: usize, n: u64) {
        self.counts[unit] = self.counts[unit].saturating_add(n);
    }

    /// The component the work belongs to.
    pub fn component(&self) -> ComponentId {
        self.component
    }

    /// The counts, one per unit of [`units`] for this component, in the same order. A component
    /// this crate does not know has no units and an empty slice.
    pub fn counts(&self) -> &[u64] {
        &self.counts[..units(self.component).len()]
    }

    /// Add `other`'s counts to these. Both must belong to the same component.
    ///
    /// # Panics
    ///
    /// If the components differ: summing the units of two components would be meaningless.
    pub fn accumulate(&mut self, other: &Ops) {
        assert_eq!(
            self.component, other.component,
            "operation counts of different components cannot be summed"
        );
        for (a, b) in self.counts.iter_mut().zip(other.counts.iter()) {
            *a = a.saturating_add(*b);
        }
    }

    /// The counts added together. Informational: the units differ, a cost is a weighted sum.
    pub fn total(&self) -> u64 {
        self.counts.iter().fold(0u64, |s, c| s.saturating_add(*c))
    }

    /// Modelled cost in picoseconds: the sum over units of weight times count, saturating.
    pub fn modelled_ps(&self) -> u64 {
        units(self.component)
            .iter()
            .zip(self.counts.iter())
            .fold(0u64, |s, (u, c)| {
                s.saturating_add(u.weight_ps.saturating_mul(*c))
            })
    }
}

/// The units of `component`, in count order; empty for a component this crate does not define.
pub fn units(component: ComponentId) -> &'static [Unit] {
    use crate::{ESTIMATOR_ID, HEURISTIC_ID, MEMORY_ID, VERIFIER_ID};
    if component == HEURISTIC_ID {
        crate::heuristic::UNITS
    } else if component == ESTIMATOR_ID {
        crate::estimator::UNITS
    } else if component == MEMORY_ID {
        crate::memory::UNITS
    } else if component == VERIFIER_ID {
        crate::verifier::UNITS
    } else {
        &[]
    }
}
