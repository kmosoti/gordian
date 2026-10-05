//! Fixed diagnosis components for the small world, and the bounded working state they read.
//!
//! Built (work item A5 in `docs/local-test-plan.md`):
//!
//! - [`WorkingState`]: the charter's bounded working view (section 3.1), with a recency-window
//!   eviction rule.
//! - The [`Component`] trait and four components: [`RuleHeuristic`], [`CountEstimator`],
//!   [`PriorRecordLookup`], [`ConsistencyVerifier`].
//! - A declared cost per component, an affine function of input size in `Resource::Compute`
//!   nanoseconds, fitted to criterion medians on one CPU (see `CALIBRATION.md`). It is what a
//!   policy sees and what the bill enforces.
//! - A count of the work each call did, in the component's own units ([`ops`], work item A8b),
//!   priced by weights fitted to minimum timings and scaled in situ. It is what the harness records as the
//!   components' cost; it follows the content of the window where the declared cost follows
//!   only its size.
//!
//! Not built here: scheduling, the run harness that owns the ledger and the bill, and any
//! learned component. A component does not append to the ledger; it returns entries and the
//! harness appends them.
//!
//! # What components may see
//!
//! A component reads a [`WorkingState`]: `PublicInfo` and the observations admitted into the
//! window. It never reads hidden simulator state, and this crate does not enable the world's
//! hidden-state feature. Probe results reach a component only as observations.
//!
//! # Measurements and hypotheses
//!
//! Only sensing produces [`EntryKind::Measurement`], and sensing is not a component. A component
//! that interprets observations emits [`EntryKind::Hypothesis`] entries, never a measurement.
//! [`payload::hypothesis_entry`] is the only constructor of component entries and fixes the kind.
//!
//! # Proposals
//!
//! [`ComponentOutput::proposal`] is `Some(h)` only when the component's own ranking has a unique
//! best hypothesis `h`. A tie is not a proposal: the full ranking is in the entry for a consumer
//! that wants to guess. `Some(None)` proposes "no fault".
//!
//! # Requests
//!
//! A component may ask for another to run, never itself. The request graph is acyclic by
//! construction: the heuristic, estimator, and lookup may request the verifier; the verifier
//! requests nothing. Whether a request is honoured is the scheduler's decision.

#![forbid(unsafe_code)]

pub mod estimator;
pub mod heuristic;
pub mod memory;
pub mod ops;
pub mod payload;
pub mod verifier;
pub mod working;

mod cost;
mod symptoms;

pub use estimator::CountEstimator;
pub use heuristic::RuleHeuristic;
pub use memory::PriorRecordLookup;
pub use ops::Ops;
pub use verifier::ConsistencyVerifier;
pub use working::WorkingState;

use gordian_core::{Charge, ComponentId, EntryKind};
use gordian_world::Hypothesis;

/// Identity of [`RuleHeuristic`]. The numbering is this crate's choice; the world's harness
/// directives name components by an index that is independent of any registry, so the harness
/// maps between the two.
pub const HEURISTIC_ID: ComponentId = ComponentId(0);
/// Identity of [`CountEstimator`].
pub const ESTIMATOR_ID: ComponentId = ComponentId(1);
/// Identity of [`PriorRecordLookup`].
pub const MEMORY_ID: ComponentId = ComponentId(2);
/// Identity of [`ConsistencyVerifier`].
pub const VERIFIER_ID: ComponentId = ComponentId(3);

/// A component's ask that another computation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputationRequest {
    /// The component asked to run.
    pub component: ComponentId,
    /// Why, in a few words, for the ledger.
    pub reason: String,
}

/// What one run of a component produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentOutput {
    /// Entries for the harness to append to the ledger, each with its kind and a `serde_json`
    /// payload. Every entry a component here produces is [`EntryKind::Hypothesis`].
    pub entries: Vec<(EntryKind, Vec<u8>)>,
    /// Computations this run asks for.
    pub requests: Vec<ComputationRequest>,
    /// The component's unique best hypothesis, if it has one (see the crate documentation).
    pub proposal: Option<Hypothesis>,
}

/// A fixed computation over the working state.
pub trait Component {
    /// Stable identity, for cost attribution.
    fn id(&self) -> ComponentId;

    /// What running on `input` is declared to cost. Deterministic, and a function of the input's
    /// size only (window length, number of services, number of prior records), never of its
    /// content.
    fn declared_cost(&self, input: &WorkingState) -> Vec<Charge>;

    /// Run on `input` and count the work done. Deterministic given `input` and the component's
    /// configuration, in output and in count. The clock is `input.now`; no component reads a wall
    /// clock. The count ([`Ops`]) is the work the call did in the component's declared units
    /// (observations scanned, records compared, worlds evaluated, ...), and is what the harness
    /// prices; see [`ops`]. Counting never changes the output.
    fn run_counted(&mut self, input: &WorkingState) -> (ComponentOutput, Ops);

    /// Run on `input`. The output of [`Component::run_counted`] without its count: what a policy
    /// sees, and what every caller that does not price the work uses.
    fn run(&mut self, input: &WorkingState) -> ComponentOutput {
        self.run_counted(input).0
    }
}
