//! The only functions in this crate that call the stream's oracle.

use crate::step::CallSummary;
use gordian_core::Instant;
use gordian_stream::oracle::{self, StreamTruth};
use gordian_stream::{Stream, StreamSimulator};

/// The hidden truth of a generated stream, read through `gordian_stream::oracle::reveal`.
///
/// Fixtures build a [`StreamTruth`] by hand; this is the one place that reads a real stream.
pub fn truth_from_stream(stream: &Stream) -> StreamTruth {
    oracle::reveal(stream)
}

/// The hidden side of every reasoner call a simulator accepted, in order, read through
/// `gordian_stream::oracle::calls`.
///
/// `focus` is the trace's own, so the scorer cross-checks it against the trajectory's `Escalate`
/// step (S35).
pub fn calls_from_sim(sim: &StreamSimulator) -> Vec<CallSummary> {
    oracle::calls(sim)
        .iter()
        .map(|c| CallSummary {
            at: Instant(c.at_ns),
            ready_at: Instant(c.ready_at_ns),
            focus: Some(c.focus),
            refs: c.refs,
            informed: c.informed,
            correct: c.correct,
        })
        .collect()
}
