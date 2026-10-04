//! Privileged access to hidden state.
//!
//! **Only `gordian-eval` and the oracle baseline may call [`reveal`].** A policy that imports this
//! module fails review. The module exists only under the cargo feature `reveal-hidden-state`
//! (or in this crate's own tests). Cargo unifies features across a build, so the feature is a
//! guard rather than a proof; `scripts/check-no-oracle.sh` is the second guard.

use crate::episode::{Episode, StreamLabel};
use crate::fault::Fault;
use serde::{Deserialize, Serialize};

/// The hidden part of an episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenState {
    /// The faults in the world. At most one in this version of the generator.
    pub faults: Vec<Fault>,
    /// The two parity bits behind `LatencySample` and `ErrorSample`.
    pub bits: (bool, bool),
    /// The configuration hash a `ConfigDrift` fault leaves at its site.
    pub drift_hash: Option<u64>,
    /// What each entry of the public stream is, parallel to `Episode::stream`.
    pub labels: Vec<StreamLabel>,
}

/// Reveal the hidden state of `episode`.
pub fn reveal(episode: &Episode) -> HiddenState {
    let h = &episode.hidden;
    HiddenState {
        faults: h.faults.clone(),
        bits: h.bits,
        drift_hash: h.drift_hash,
        labels: h.labels.clone(),
    }
}
