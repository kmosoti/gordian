//! What is carried from one segment to the next.
//!
//! The harness plays one fresh arm per segment, and the segments of a run are played one after
//! another in seed order by one thread of one process (`execute_stream`). An arm that learns from
//! its own run history must keep that history across segments, and the harness is another lab's
//! territory, so the learned arm keeps it here: a keyed store in the process, written at every
//! boundary and read when a segment begins.
//!
//! What this is, stated plainly: hidden state outside the arm's inputs, with one writer and one
//! reader, the learned noticer of one key. Its content is a function of the observations that arm
//! was delivered in the segments before, in the order they were played; it is never read by
//! anything else. A replay of the same manifest in a new process reproduces it exactly; two runs
//! in one process under one key do not (the second starts from the first's end), which is why
//! every run names its keys and tests reset them ([`reset`]).

use super::learner::Learned;
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

static STORE: Mutex<BTreeMap<u64, Learned>> = Mutex::new(BTreeMap::new());

/// The state carried under `key`, if any.
pub fn load(key: u64) -> Option<Learned> {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned()
}

/// Keep `state` under `key`.
pub fn store(key: u64, state: &Learned) {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, state.clone());
}

/// Forget what is carried under `key`.
pub fn reset(key: u64) {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
}
