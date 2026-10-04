//! One pass over the window that notes which symptoms are present and where they were first
//! seen. The heuristic and the prior-record lookup share it, so they agree on what a symptom is.
//!
//! A *symptom* here is what the world's `physics` module calls abnormal: a counter at or above
//! `physics::HIGH`, or a catalogue message other than `CheckHealth`. Three more slots record
//! evidence that a fault kind is *present* at a service: a snapshot or a configuration probe
//! whose hash differs from the public one, and a positive `ResourceUsage` or `CredentialCheck`
//! probe. Those three are not part of a prior record's signature.

use gordian_core::Instant;
use gordian_world::graph::ServiceId;
use gordian_world::physics::{HIGH, SignalText, SymptomTag};
use gordian_world::{CounterName, Observation, ProbeKind, ProbeResult, PublicInfo};
use std::collections::VecDeque;

/// Slot of a counter symptom.
pub(crate) const fn counter_slot(name: CounterName) -> usize {
    match name {
        CounterName::ErrorRate => 0,
        CounterName::Latency => 1,
        CounterName::Saturation => 2,
        CounterName::AuthFailures => 3,
        CounterName::Restarts => 4,
    }
}

/// Slot of a catalogue message symptom.
pub(crate) const fn text_slot(text: SignalText) -> usize {
    5 + match text {
        SignalText::OutOfResource => 0,
        SignalText::ConfigRejected => 1,
        SignalText::ServiceDown => 2,
        SignalText::Unauthorized => 3,
        SignalText::Flapping => 4,
        SignalText::UpstreamUnreachable => 5,
        SignalText::MixedSignals => 6,
        SignalText::CheckHealth => 7,
    }
}

/// A snapshot or configuration probe whose hash differs from the public one.
pub(crate) const SLOT_CHANGED_CONFIG: usize = 13;
/// A `ResourceUsage` probe that came back positive.
pub(crate) const SLOT_PROBE_RESOURCE: usize = 14;
/// A `CredentialCheck` probe that came back positive.
pub(crate) const SLOT_PROBE_CREDENTIAL: usize = 15;
/// Number of slots.
pub(crate) const N_SLOTS: usize = 16;
/// Slots 0 to 12 are the symptom tags of `physics::signature`.
pub(crate) const TAG_MASK: u16 = 0x1FFF;

/// Slot of a symptom tag.
pub(crate) fn tag_slot(tag: SymptomTag) -> usize {
    match tag {
        SymptomTag::Counter(name) => counter_slot(name),
        SymptomTag::Text(text) => text_slot(text),
    }
}

/// The bit set of a list of symptom tags.
pub(crate) fn tag_mask(tags: &[SymptomTag]) -> u16 {
    tags.iter().fold(0, |m, t| m | (1 << tag_slot(*t)))
}

/// Which symptoms the window holds and where each was first seen.
#[derive(Debug, Clone)]
pub(crate) struct Summary {
    /// Bit `s` is set when slot `s` was seen.
    pub(crate) mask: u16,
    /// The service of the first (oldest in the window) observation of each slot.
    pub(crate) first: [Option<ServiceId>; N_SLOTS],
    /// The service of the first symptom of any slot.
    pub(crate) first_abnormal: Option<ServiceId>,
    /// Services with a high `ErrorRate`, in order of first appearance. The first is the
    /// heuristic's guess at the fault site: the world emits the site's own `ErrorRate` first.
    pub(crate) error_order: Vec<ServiceId>,
    seen_error: u64,
}

impl Summary {
    fn note(&mut self, slot: usize, service: ServiceId) {
        self.mask |= 1 << slot;
        self.first[slot].get_or_insert(service);
        self.first_abnormal.get_or_insert(service);
    }

    fn note_error(&mut self, service: ServiceId) {
        // The world has at most `physics::MAX_SERVICES` (12) services, so a u64 set suffices.
        let bit = 1u64 << (service.index() & 63);
        if self.seen_error & bit == 0 {
            self.seen_error |= bit;
            self.error_order.push(service);
        }
    }

    /// The earliest service with a high `ErrorRate`.
    pub(crate) fn first_error(&self) -> Option<ServiceId> {
        self.error_order.first().copied()
    }
}

/// Summarize `evidence`. Observations that name a service outside the public graph are skipped,
/// so no hypothesis built from a summary names a service that does not exist.
pub(crate) fn summarize(
    public: &PublicInfo,
    evidence: &VecDeque<(Instant, Observation)>,
) -> Summary {
    let services = &public.services;
    let valid = |id: ServiceId| id.index() < services.len();
    let mut s = Summary {
        mask: 0,
        first: [None; N_SLOTS],
        first_abnormal: None,
        error_order: Vec::new(),
        seen_error: 0,
    };
    for (_, obs) in evidence {
        match obs {
            Observation::Counter {
                service,
                name,
                value,
            } if *value >= HIGH && valid(*service) => {
                s.note(counter_slot(*name), *service);
                if *name == CounterName::ErrorRate {
                    s.note_error(*service);
                }
            }
            Observation::Message {
                service, text_id, ..
            } if valid(*service) => {
                if let Some(text) = SignalText::from_text_id(*text_id)
                    && text != SignalText::CheckHealth
                {
                    s.note(text_slot(text), *service);
                }
            }
            Observation::Snapshot {
                service,
                config_hash,
            } if valid(*service) && services[service.index()].config_hash != *config_hash => {
                s.note(SLOT_CHANGED_CONFIG, *service);
            }
            Observation::Probed { probe, result } if valid(probe.target) => {
                match (probe.kind, result) {
                    (ProbeKind::ResourceUsage, ProbeResult::Positive) => {
                        s.note(SLOT_PROBE_RESOURCE, probe.target);
                    }
                    (ProbeKind::CredentialCheck, ProbeResult::Positive) => {
                        s.note(SLOT_PROBE_CREDENTIAL, probe.target);
                    }
                    (ProbeKind::ConfigSnapshot, ProbeResult::ConfigHash(h))
                        if services[probe.target.index()].config_hash != *h =>
                    {
                        s.note(SLOT_CHANGED_CONFIG, probe.target);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working::WorkingState;
    use gordian_world::physics::signature;
    use gordian_world::{EpisodeClass, EpisodeSpec, generate};

    /// The mask of the symptom slots equals the mask of the world's own `signature`, so the
    /// bit-set fast path and the reference implementation agree on what the window shows.
    #[test]
    fn mask_agrees_with_the_reference_signature() {
        for class in EpisodeClass::ALL {
            for seed in 0..12 {
                let ep = generate(&EpisodeSpec::new(seed, class));
                let mut w = WorkingState::new(ep.public_info(), ep.stream().len());
                for (t, o) in ep.stream() {
                    w.admit(*t, o.clone());
                }
                let summary = summarize(&w.public, w.evidence());
                let reference = tag_mask(&signature(ep.stream()));
                assert_eq!(summary.mask & TAG_MASK, reference, "{class:?} seed {seed}");
            }
        }
    }

    #[test]
    fn slots_are_distinct_and_in_range() {
        let mut slots: Vec<usize> = CounterName::ALL.iter().map(|c| counter_slot(*c)).collect();
        slots.extend(SignalText::ALL.iter().map(|t| text_slot(*t)));
        slots.extend([
            SLOT_CHANGED_CONFIG,
            SLOT_PROBE_RESOURCE,
            SLOT_PROBE_CREDENTIAL,
        ]);
        let mut sorted = slots.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), slots.len());
        assert!(slots.iter().all(|s| *s < N_SLOTS));
    }
}
