//! Tests for the small world.
//!
//! They live inside the crate (not under `tests/`) so that `oracle::reveal` is available under
//! `cfg(test)` without a self-referencing dev-dependency. The oracle is used here to read the
//! hidden truth that the properties are checked against.

mod checker;
mod classes;
mod leakage;
mod props;
mod sim;

use crate::episode::{Episode, EpisodeClass, EpisodeSpec};
use crate::fault::Hypothesis;
use crate::oracle::reveal;
use crate::physics::consistent_hypotheses;
use crate::sense::{Observation, ProbeKind};
use crate::step::{Action, Outcome, Simulator};
use crate::{ServiceId, generate};
use gordian_core::Instant;

pub(crate) type Evidence = Vec<(Instant, Observation)>;

/// A default-spec episode.
pub(crate) fn episode(class: EpisodeClass, seed: u64) -> Episode {
    generate(&EpisodeSpec::new(seed, class))
}

/// The true hypothesis, from the oracle.
pub(crate) fn truth(ep: &Episode) -> Hypothesis {
    reveal(ep).faults.first().and_then(|f| f.hypothesis())
}

/// Consistent hypotheses for `evidence` under the episode's public information.
pub(crate) fn consistent(ep: &Episode, evidence: &[(Instant, Observation)]) -> Vec<Hypothesis> {
    consistent_hypotheses(&ep.public_info(), evidence)
}

/// The observation a probe returns, obtained through the real simulator path.
pub(crate) fn probe_obs(
    ep: &Episode,
    kind: ProbeKind,
    target: ServiceId,
) -> (Instant, Observation) {
    let mut sim = Simulator::new(ep.clone());
    match sim.apply(Action::Probe { kind, target }, Instant::ZERO) {
        Outcome::Probed {
            observation,
            ready_at,
            ..
        } => (ready_at, observation),
        other => panic!("probe refused: {other:?}"),
    }
}

/// Evidence extended with a probe result.
pub(crate) fn with_probe(
    ep: &Episode,
    evidence: &[(Instant, Observation)],
    kind: ProbeKind,
    target: ServiceId,
) -> Evidence {
    let mut out = evidence.to_vec();
    out.push(probe_obs(ep, kind, target));
    out
}

/// Seeds used by per-class unit tests. Enough to cover every branch of every class.
pub(crate) const SEEDS: std::ops::Range<u64> = 0..60;

pub(crate) fn is_abnormal(ep: &Episode, o: &Observation) -> bool {
    use crate::physics::{HIGH, SignalText};
    match o {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => matches!(
            SignalText::from_text_id(*text_id),
            Some(t) if t != SignalText::CheckHealth
        ),
        Observation::Snapshot {
            service,
            config_hash,
        } => *config_hash != ep.world().services[service.index()].config_hash,
        _ => false,
    }
}
