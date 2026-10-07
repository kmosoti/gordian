//! What is carried from one segment to the next: the trained readout.
//!
//! The harness plays one fresh arm per segment, and the segments of a run are played one after
//! another in seed order by one thread of one process. An arm that learns from its own run history
//! must keep it across segments, and the harness is another lab's territory, so the arm keeps it
//! here, as the learned noticer of work item L1 does: a keyed store in the process, written after
//! each step that trained and read when a segment begins.
//!
//! What this is, stated plainly: state outside the arm's inputs, with one writer and one reader,
//! the reservoir noticer of one key. Its content is a function of the observations that arm was
//! delivered in the segments before, in the order they were played; nothing else reads it. A
//! replay of the same manifest in a new process reproduces it exactly; two runs in one process
//! under one key do not (the second starts from the first's end), which is why every run names its
//! keys and tests reset them ([`reset`]). Only the readout is carried: the reservoir's weights are
//! drawn again from the manifest's seed and its activations start at zero every segment.

use super::esn::Readout;
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

static STORE: Mutex<BTreeMap<u64, Readout>> = Mutex::new(BTreeMap::new());

/// The readout carried under `key`, if any.
pub fn load(key: u64) -> Option<Readout> {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned()
}

/// Keep `readout` under `key`.
pub fn store(key: u64, readout: &Readout) {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, readout.clone());
}

/// Forget what is carried under `key`.
pub fn reset(key: u64) {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
}
