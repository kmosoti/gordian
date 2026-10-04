//! Helpers for the declared cost models.
//!
//! Every model is affine in a few size variables, in `Resource::Compute` nanoseconds. Slopes are
//! stored in picoseconds per unit so that costs of a few nanoseconds per observation keep their
//! precision in integer arithmetic.

use gordian_core::{Charge, Resource};

/// `a_ns + b_ps * x / 1000`, saturating.
pub(crate) fn affine_ns(a_ns: u64, b_ps: u64, x: usize) -> u64 {
    a_ns.saturating_add(b_ps.saturating_mul(x as u64) / 1000)
}

/// One compute charge of `ns` nanoseconds.
pub(crate) fn compute(ns: u64) -> Charge {
    Charge::new(Resource::Compute, ns)
}
