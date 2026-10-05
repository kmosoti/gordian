//! Background observations: six independent Poisson processes, none of which belongs to an
//! incident.
//!
//! The first world's noise was free-form messages with a random 64-bit id, which a component that
//! knows the catalogue discards for free. Here the background contains the things an incident
//! also contains:
//!
//! - `CatalogueStray`: catalogue messages (all eight, including `CheckHealth`) at random services;
//! - `Blip`: isolated abnormal counter readings at random services;
//! - `MiniBurst`: an `ErrorRate` anchor and a characteristic message, as the first moments of a
//!   plain incident, which then stop (the heartbeats that follow a real incident never come);
//! - `FreeForm`: messages whose ids come from the same pool as the hard incidents' decisive
//!   messages, for 70% of them, so an id does not identify a hard incident by itself.
//!
//! A hard incident's decisive evidence is free-form-looking messages, so the first world's filter
//! (keep what the catalogue and the physics read) also *drops* decisive evidence here.

use crate::labels::NoiseKind;
use crate::params::NoiseSpec;
use crate::present::{calm, counter, hot, mini_burst, severity};
use crate::regime::{Epoch, epoch_at};
use crate::rng::{Gen, domain};
use gordian_core::Instant;
use gordian_world::physics::SignalText;
use gordian_world::{CounterName, FaultKind, Observation, ServiceId};

/// One noise observation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NoiseItem {
    pub(crate) at: u64,
    pub(crate) obs: Observation,
    pub(crate) kind: NoiseKind,
}

/// Instants of a Poisson process of `mhz` milli-hertz on `[0, duration_ns]`, with the generator
/// that produced them (so the caller can keep drawing content from it).
fn times(g: &mut Gen, mhz: u64, duration_ns: u64) -> Vec<u64> {
    if mhz == 0 {
        return Vec::new();
    }
    let mean = 1_000_000_000_000u64 / mhz;
    let mut out = Vec::new();
    let mut t = 0u64;
    loop {
        t = t.saturating_add(g.exp_gap_ns(mean.max(1)));
        if t > duration_ns {
            return out;
        }
        out.push(t);
    }
}

/// Every background observation of a stream.
pub(crate) fn generate(
    seed: u64,
    spec: &NoiseSpec,
    duration_ns: u64,
    n_services: usize,
    epochs: &[Epoch],
    pool: &[u64],
) -> Vec<NoiseItem> {
    let n = n_services as u64;
    let svc = |g: &mut Gen| ServiceId(g.below(n) as u32);
    let mut out = Vec::new();

    let mut g = Gen::keyed(&[seed, domain::NOISE, 0]);
    for t in times(&mut g, spec.benign_mhz, duration_ns) {
        let service = svc(&mut g);
        let name = CounterName::ALL[g.below(5) as usize];
        out.push(NoiseItem {
            at: t,
            obs: counter(service, name, calm(&mut g)),
            kind: NoiseKind::Benign,
        });
    }

    let mut g = Gen::keyed(&[seed, domain::NOISE, 1]);
    for t in times(&mut g, spec.freeform_mhz, duration_ns) {
        let service = svc(&mut g);
        let from_pool = g.below(10) < 7;
        let pick = g.below(pool.len() as u64) as usize;
        let fresh = g.u64() | (1 << 40);
        let sev = severity(&mut g);
        out.push(NoiseItem {
            at: t,
            obs: Observation::Message {
                service,
                text_id: if from_pool { pool[pick] } else { fresh },
                severity: sev,
            },
            kind: NoiseKind::FreeForm,
        });
    }

    let mut g = Gen::keyed(&[seed, domain::NOISE, 2]);
    for t in times(&mut g, spec.catalogue_mhz, duration_ns) {
        let service = svc(&mut g);
        let text = SignalText::ALL[g.below(8) as usize];
        let sev = severity(&mut g);
        out.push(NoiseItem {
            at: t,
            obs: Observation::Message {
                service,
                text_id: text.text_id(),
                severity: sev,
            },
            kind: NoiseKind::CatalogueStray,
        });
    }

    let mut g = Gen::keyed(&[seed, domain::NOISE, 3]);
    for t in times(&mut g, spec.blip_mhz, duration_ns) {
        let service = svc(&mut g);
        let name = CounterName::ALL[g.below(5) as usize];
        out.push(NoiseItem {
            at: t,
            obs: counter(service, name, hot(&mut g)),
            kind: NoiseKind::Blip,
        });
    }

    let mut g = Gen::keyed(&[seed, domain::NOISE, 4]);
    for t in times(&mut g, spec.burst_mhz, duration_ns) {
        let service = svc(&mut g);
        let kind = FaultKind::ALL[g.below(5) as usize];
        let phys = &epoch_at(epochs, Instant(t)).physics;
        for s in mini_burst(&mut g, phys, kind, service) {
            out.push(NoiseItem {
                at: t + s.off_ns,
                obs: s.obs,
                kind: NoiseKind::MiniBurst,
            });
        }
    }

    let mut g = Gen::keyed(&[seed, domain::NOISE, 5]);
    for t in times(&mut g, spec.snapshot_mhz, duration_ns) {
        let service = svc(&mut g);
        let hash = epochs[0].services[service.index()].config_hash;
        out.push(NoiseItem {
            at: t,
            obs: Observation::Snapshot {
                service,
                config_hash: hash,
            },
            kind: NoiseKind::Snapshot,
        });
    }
    out
}
