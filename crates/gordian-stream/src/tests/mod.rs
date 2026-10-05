//! Tests for the stream world.
//!
//! They live inside the crate (not under `tests/`) so that `oracle` is available under
//! `cfg(test)` without a self-referencing dev-dependency, as in the first world.

use crate::oracle::{StreamTruth, reveal};
use crate::{Stream, StreamParams, generate};

/// A default-parameter stream.
pub(crate) fn stream(seed: u64) -> Stream {
    generate(&StreamParams::new(seed))
}

/// A stream and its truth.
pub(crate) fn with_truth(params: &StreamParams) -> (Stream, StreamTruth) {
    let s = generate(params);
    let t = reveal(&s);
    (s, t)
}

#[test]
fn it_builds() {
    let (s, t) = with_truth(&StreamParams::new(1));
    assert!(!s.events().is_empty());
    assert!(!t.incidents.is_empty());
}
