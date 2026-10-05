//! Tests for the stream world.
//!
//! They live inside the crate (not under `tests/`) so that `oracle` is available under
//! `cfg(test)` without a self-referencing dev-dependency, as in the first world.

mod cheap;
mod determinism;
mod indistinguishable;
mod noise;
mod questions;
mod reasoner;
mod recurrence;
mod soundness;
mod structure;
mod tiers;

use crate::oracle::{StreamTruth, reveal};
use crate::{ObsId, Stream, StreamParams, generate};
use gordian_core::Instant;
use gordian_world::Observation;

pub(crate) type Evidence = Vec<(Instant, Observation)>;

/// A stream and its truth.
pub(crate) fn with_truth(params: &StreamParams) -> (Stream, StreamTruth) {
    let s = generate(params);
    let t = reveal(&s);
    (s, t)
}

/// Every observation labelled with incident `id`, in stream order.
pub(crate) fn evidence_of(s: &Stream, t: &StreamTruth, id: u32) -> Evidence {
    t.incidents[id as usize]
        .observations
        .iter()
        .map(|o: &ObsId| s.events()[o.0 as usize].clone())
        .collect()
}

/// Parameters with the tier mix replaced.
pub(crate) fn with_mix(seed: u64, plain: u32, hard: u32) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.mix.plain_permille = plain;
    p.mix.hard_permille = hard;
    p
}
