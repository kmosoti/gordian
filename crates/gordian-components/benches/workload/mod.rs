//! The workload the cost models are calibrated on, shared by `benches/components.rs` and
//! `tests/cost_calibration.rs` so that both measure and declare cost over the same windows.
//!
//! A *pool* holds two generated episodes of every class (22 windows). A window of `n`
//! observations is the episode's stream cut at `n`, or, when the stream is shorter, repeated
//! with its instants shifted so they keep increasing. One benchmark iteration runs a component
//! once on every window of the pool, so the number criterion reports is the cost of one run
//! *averaged over the class mix, times the pool size*, and the declared cost to compare against
//! is the sum of `declared_cost` over the same pool.
//!
//! This mix is a choice. It weights every class equally, which no experiment will; the spread
//! between classes is reported separately by the calibration test.
#![allow(dead_code)]

use gordian_components::WorkingState;
use gordian_core::Instant;
use gordian_world::episode::PriorRecord;
use gordian_world::physics::{SignalText, SymptomTag};
use gordian_world::{CounterName, Episode, EpisodeClass, EpisodeSpec, generate};

/// Window sizes of the main sweep.
pub const SIZES: [usize; 4] = [16, 64, 256, 1024];
/// Window sizes held out of the fit: three between the fitted sizes and one beyond the largest.
/// The constants are not fitted to these, so they test the model on sizes it has not seen.
pub const HELD_OUT_SIZES: [usize; 4] = [32, 128, 512, 2048];
/// Service counts of the service sweep.
pub const SERVICE_COUNTS: [u8; 3] = [4, 8, 12];
/// Window size of the service sweep.
pub const SERVICE_SWEEP_N: usize = 256;
/// Records added to each window's own, in the record sweep (see [`with_padding`]).
pub const PAD_COUNTS: [usize; 4] = [0, 16, 128, 1024];
/// Window size of the record sweep.
pub const RECORD_SWEEP_N: usize = 64;
/// Episodes per class in a pool.
pub const SEEDS_PER_CLASS: u64 = 2;

const WORKLOAD_SEED: u64 = 0x5EED_0000;

/// The episodes of the pool, optionally with a fixed number of services.
pub fn episodes(services: Option<u8>) -> Vec<Episode> {
    let mut out = Vec::new();
    for (i, class) in EpisodeClass::ALL.into_iter().enumerate() {
        for k in 0..SEEDS_PER_CLASS {
            let mut spec = EpisodeSpec::new(WORKLOAD_SEED + (i as u64) * 16 + k, class);
            if let Some(s) = services {
                spec.min_services = s;
                spec.max_services = s;
            }
            out.push(generate(&spec));
        }
    }
    out
}

/// A working state holding exactly `n` observations of `ep`'s stream (repeated if needed).
pub fn window(ep: &Episode, n: usize) -> WorkingState {
    let stream = ep.stream();
    assert!(!stream.is_empty(), "workload episodes have observations");
    let lap = ep.spec().horizon.0 + 1;
    let mut w = WorkingState::new(ep.public_info(), n);
    for k in 0..n {
        let (t, o) = &stream[k % stream.len()];
        let shifted = Instant(t.0 + (k / stream.len()) as u64 * lap);
        w.admit(shifted, o.clone());
    }
    assert_eq!(w.size(), n);
    w
}

/// The pool of windows of size `n`.
pub fn windows(n: usize, services: Option<u8>) -> Vec<WorkingState> {
    episodes(services).iter().map(|ep| window(ep, n)).collect()
}

/// The same windows with `extra` more prior records after each window's own.
///
/// Every added record carries the tag of `CheckHealth`, which no window ever shows, so none of
/// them can match: the number of records read grows while the matches (and therefore the entries
/// emitted) stay what the generator produced. The other tags of a pad record, one to four of
/// them, are chosen by a fixed arithmetic rule so the records have realistic lengths.
pub fn with_padding(pool: &[WorkingState], extra: usize) -> Vec<WorkingState> {
    let vocabulary: Vec<SymptomTag> = CounterName::ALL
        .into_iter()
        .map(SymptomTag::Counter)
        .chain(
            SignalText::ALL
                .into_iter()
                .filter(|t| *t != SignalText::CheckHealth)
                .map(SymptomTag::Text),
        )
        .collect();
    let pads: Vec<PriorRecord> = (0..extra)
        .map(|i| {
            let len = (i * 7) % 4;
            let mut tags: Vec<SymptomTag> = (0..len)
                .map(|j| vocabulary[(i * 5 + j * 3) % vocabulary.len()])
                .collect();
            tags.push(SymptomTag::Text(SignalText::CheckHealth));
            tags.sort();
            tags.dedup();
            PriorRecord {
                signature: tags,
                resolution: gordian_world::FaultKind::ALL[i % 5],
            }
        })
        .collect();
    pool.iter()
        .map(|w| {
            let mut w = w.clone();
            w.public.prior_records.extend(pads.iter().cloned());
            w
        })
        .collect()
}
