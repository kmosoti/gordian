//! Deterministic reference core for Gordian.
//!
//! This crate holds the parts of the system whose behaviour must be exactly replayable from a
//! recorded ledger: time as an explicit value, resource budgets as explicit accounting, and an
//! append-only ledger that keeps *measurements* distinct from *hypotheses*.
//!
//! Nothing in this crate reads a wall clock, draws randomness, or performs I/O. Every effect
//! enters as an argument and is recorded at the boundary. That is the "protocol replay" claim of
//! the charter, section 11, and it is the only claim this crate makes.

#![forbid(unsafe_code)]

pub mod budget;
pub mod clock;
pub mod ledger;

pub use budget::{Budget, BudgetError, Charge, Resource};
pub use clock::{Instant, ManualClock};
pub use ledger::{Entry, EntryId, EntryKind, Ledger, Provenance};
