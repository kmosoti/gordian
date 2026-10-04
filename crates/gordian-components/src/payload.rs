//! The payload of the hypothesis entries components emit, as `serde_json` bytes.
//!
//! One type, [`HypothesisEntry`], describes everything a component asserts. A reader decodes it
//! with [`decode`]; a harness can use it to fill `WorkingState::hypotheses`.

use gordian_core::EntryKind;
use gordian_world::Hypothesis;
use serde::{Deserialize, Serialize};

/// One candidate hypothesis with the component's score for it, if the component scores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ranked {
    /// `None` is "no fault"; `Some((kind, site))` is a fault.
    pub hypothesis: Hypothesis,
    /// The component's score, higher is better. `None` when the component does not score.
    pub score: Option<u32>,
}

/// What a component asserts about the hidden state. An interpretation, never a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HypothesisEntry {
    /// A set of candidate hypotheses, best first when the component ranks.
    Candidates {
        /// The component's name.
        source: String,
        /// Which rule, record match, or check produced the candidates, in a few words.
        basis: String,
        /// The candidates. Ties keep a fixed order (no fault, then site, then fault kind).
        ranked: Vec<Ranked>,
        /// How many candidates share the best score. For an unscored component this is the
        /// number of candidates. `1` means a unique best.
        tied_at_top: u32,
    },
    /// The evidence in the window is contradictory under the public rules. This says the
    /// *window* is damaged (for example the observation that anchors the site was evicted), not
    /// that the world is contradictory, and it says nothing about which hypothesis is true.
    EvidenceDamaged {
        /// The component's name.
        source: String,
        /// Number of observations in the window that was checked.
        window: u32,
    },
}

/// The entry a component emits for `entry`. The kind is always [`EntryKind::Hypothesis`]; there
/// is deliberately no way to build a component entry of another kind.
pub fn hypothesis_entry(entry: &HypothesisEntry) -> (EntryKind, Vec<u8>) {
    // Serializing these types cannot fail: every field is an enum, an integer, or a string.
    let bytes = serde_json::to_vec(entry).expect("hypothesis entry serializes");
    (EntryKind::Hypothesis, bytes)
}

/// Decode a hypothesis entry payload.
pub fn decode(payload: &[u8]) -> Result<HypothesisEntry, serde_json::Error> {
    serde_json::from_slice(payload)
}

/// The proposal implied by a ranking: the first candidate if it is the unique best.
pub(crate) fn unique_best(ranked: &[Ranked], tied_at_top: u32) -> Option<Hypothesis> {
    if tied_at_top == 1 {
        ranked.first().map(|r| r.hypothesis)
    } else {
        None
    }
}
