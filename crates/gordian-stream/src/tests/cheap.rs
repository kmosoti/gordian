//! A probing helper for the tests that need a simulator to buy probes from.
//!
//! The driver that ran the first world's four components and the shared decision rule on a stream
//! incident (the "cheap rung" of the tests) lived here until work item R3, when `gordian-run`
//! came to depend on this crate. It moved, with the tests that use it, to
//! `crates/gordian-run/tests/stream_cheap.rs`; what stays is the helper that needs nothing from
//! `gordian-run`.

use super::*;
use crate::StreamSimulator;

/// A simulator with probe budget to spare, for the cheap rung to buy probes from. Probes are
/// about the stream and not about what has been observed, so nothing is delivered; callers pass
/// the instants and clone this for every run, because instants must not go backwards.
pub(crate) fn probing_sim(params: &StreamParams) -> StreamSimulator {
    let mut p = params.clone();
    p.budget.probes = 1_000_000;
    p.budget.probe_time_ns = u64::MAX / 4;
    StreamSimulator::new(generate(&p))
}
