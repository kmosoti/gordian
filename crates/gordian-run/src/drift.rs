//! The drift-control workload (plan A8).
//!
//! Wall time on this VM drifts within a session (one benchmark moved 249, 268, 324, 325 µs over
//! four consecutive runs), so a measured-cost comparison needs a record of the machine's speed
//! taken alongside the run. Every `drift_block` episodes the harness runs this fixed workload and
//! writes how long it took to `drift.csv`. The analysis package reports the spread of those
//! timings and the ratio of the last to the first (`analysis/gordian_analysis/drift.py`).
//!
//! # The workload
//!
//! The consistency verifier (`gordian_components::ConsistencyVerifier`, the component whose
//! checker is the most expensive in the system) run [`REPS`] times on one fixed window: the
//! first [`WINDOW`] observations of the public stream of the episode `(DRIFT_SEED, DRIFT_CLASS)`
//! with fixed generator parameters ([`NOISE_RATE`], [`SERVICES`]; the horizon and the rest are
//! the world's defaults), not the manifest's. The window is built once
//! per run, outside every timer. The same bytes are checked every time on every machine, so a
//! change in the timing is a change in the machine (or in the verifier's code, between builds).
//!
//! # What it may not do
//!
//! - *Influence an arm.* It shares nothing with an episode: its own generated episode, its own
//!   component instance, no random stream, no ledger, no bill. It runs between episodes and its
//!   output is dropped. A test runs the same manifest with the workload at every episode and
//!   effectively never and compares every arm's `results.csv` byte for byte. (It can still
//!   change the machine's cache and frequency state seen by the next episode; that is a
//!   property of any between-episode work, bounded by the ratio of its cost to the episodes',
//!   and is what the position and drift diagnostics are for.)
//! - *Be charged.* It runs outside every `std::time::Instant` bracket an episode has, so it is
//!   in no arm's `measured.csv` and in no bill. Its time is recorded in `drift.csv` as harness
//!   overhead, and the driver adds it to the measured total when it computes
//!   `internal_external_ratio`.

use crate::harness::public_window;
use gordian_components::{Component, ConsistencyVerifier, WorkingState};
use gordian_world::{EpisodeClass, EpisodeSpec};
use std::hint::black_box;
use std::time::Instant as Wall;

/// The seed of the fixed episode. Arbitrary; nothing was searched for.
pub const DRIFT_SEED: u64 = 8_675_309;

/// The class of the fixed episode: the one whose public stream is longest (80% irrelevant
/// messages), so the window is full.
pub const DRIFT_CLASS: EpisodeClass = EpisodeClass::NoiseFlood;

/// The fixed episode's noise rate: the generator's maximum, so that its stream is long enough to
/// fill the window (at the default rate of 3 it holds fewer than 100 observations).
pub const NOISE_RATE: u32 = 50;

/// The fixed episode's number of services: the generator's maximum.
pub const SERVICES: u8 = 12;

/// Observations in the fixed window, the default window of the harness's limits.
pub const WINDOW: usize = 256;

/// Verifier runs per timing.
pub const REPS: u32 = 2_000;

/// The header of `drift.csv`.
pub const DRIFT_HEADER: &str = "run_id,block,units_done,reps,ns,min_ns";

/// One timing of the workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriftSample {
    /// Index of the block, from 0, in the order the blocks ran.
    pub block: u32,
    /// Episodes (`(seed, class)` units, each played once per arm) finished before the block.
    pub units_done: u64,
    /// Verifier runs timed.
    pub reps: u32,
    /// Sum of the runs' wall times, nanoseconds.
    pub ns: u64,
    /// The shortest single run, nanoseconds. Less sensitive to a preemption than `ns`.
    pub min_ns: u64,
}

/// The fixed window and a verifier to run on it.
pub struct Workload {
    state: WorkingState,
    verifier: ConsistencyVerifier,
}

impl Workload {
    /// Build the fixed window. Not timed.
    pub fn new() -> Self {
        let mut spec = EpisodeSpec::new(DRIFT_SEED, DRIFT_CLASS);
        spec.noise_rate = NOISE_RATE;
        spec.min_services = SERVICES;
        spec.max_services = SERVICES;
        Self {
            state: public_window(&spec, WINDOW),
            verifier: ConsistencyVerifier::new(),
        }
    }

    /// Observations in the window.
    pub fn window(&self) -> usize {
        self.state.size()
    }

    /// Run the verifier [`REPS`] times, timing each run with `std::time::Instant`, and return
    /// the sample for `block`, which ran after `units_done` episodes.
    pub fn run_block(&mut self, block: u32, units_done: u64) -> DriftSample {
        let (mut ns, mut min_ns) = (0u64, u64::MAX);
        for _ in 0..REPS {
            let started = Wall::now();
            let output = self.verifier.run(black_box(&self.state));
            let took = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            black_box(output);
            ns = ns.saturating_add(took);
            min_ns = min_ns.min(took);
        }
        DriftSample {
            block,
            units_done,
            reps: REPS,
            ns,
            min_ns,
        }
    }
}

impl Default for Workload {
    fn default() -> Self {
        Self::new()
    }
}

/// One line of `drift.csv`, without a trailing newline.
pub fn drift_row(run_id: &str, sample: &DriftSample) -> String {
    format!(
        "{run_id},{block},{units},{reps},{ns},{min}",
        block = sample.block,
        units = sample.units_done,
        reps = sample.reps,
        ns = sample.ns,
        min = sample.min_ns,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_full_and_the_workload_is_the_same_every_time() {
        let a = Workload::new();
        let b = Workload::new();
        assert_eq!(
            a.window(),
            WINDOW,
            "the fixed episode's stream fills the window"
        );
        assert_eq!(
            a.state, b.state,
            "the window is a function of the constants"
        );
    }

    #[test]
    fn a_block_times_every_rep() {
        let mut workload = Workload::new();
        let sample = workload.run_block(3, 150);
        assert_eq!(
            (sample.block, sample.units_done, sample.reps),
            (3, 150, REPS)
        );
        assert!(sample.min_ns > 0, "a verifier run takes measurable time");
        assert!(sample.ns >= sample.min_ns * u64::from(REPS));
        assert_eq!(
            drift_row("r", &sample),
            format!("r,3,150,{REPS},{},{}", sample.ns, sample.min_ns)
        );
    }
}
