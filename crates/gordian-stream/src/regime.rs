//! The service graph and the physics in force at each instant.
//!
//! The first world's physics is fixed. In the stream it is the physics of *regime zero*; a
//! scheduled [`RegimeKind`] change alters part of what the generator uses from some instant on,
//! while the cheap rung keeps the first world's tables and the time-zero graph. What each change
//! alters, exactly:
//!
//! - `SignatureShift { kind, now_emits }`: from the change on, incidents of `kind` emit the
//!   characteristic message of `now_emits` instead of their own. Counters, propagation and probe
//!   results do not change.
//! - `EdgeAdd { dependent, dependency }`: from the change on, `dependent` depends on
//!   `dependency` in the true graph. Propagation, and the dependents a fault alarms at, follow
//!   the true graph. Probe results do not change.
//!
//! Probe semantics never change, so a probe tells the truth in every regime.

use crate::rng::{Gen, domain};
use gordian_core::Instant;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{SignalText, characteristic_message};
use gordian_world::{FaultKind, ResourceKind, Service, ServiceId};
use serde::{Deserialize, Serialize};

use crate::params::{RegimeKind, RegimeSchedule};

/// The physics a generator reads. Regime zero is the first world's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Physics {
    /// The characteristic message each kind emits, indexed by `FaultKind` declaration order.
    char_msg: [SignalText; 5],
}

impl Physics {
    pub(crate) fn base() -> Self {
        let mut char_msg = [SignalText::CheckHealth; 5];
        for k in FaultKind::ALL {
            char_msg[k as usize] = characteristic_message(k);
        }
        Self { char_msg }
    }

    /// The characteristic message `kind` emits in this regime.
    pub(crate) fn char_message(&self, kind: FaultKind) -> SignalText {
        self.char_msg[kind as usize]
    }
}

/// What a regime change did, as resolved from the seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimeDetail {
    /// `kind` now emits the characteristic message of `now_emits`.
    SignatureShift {
        /// The kind whose signature changed.
        kind: FaultKind,
        /// The kind whose characteristic message it now emits.
        now_emits: FaultKind,
    },
    /// `dependent` now depends on `dependency`.
    EdgeAdd {
        /// The service that gained a dependency.
        dependent: ServiceId,
        /// The service it now depends on.
        dependency: ServiceId,
    },
    /// The schedule asked for an edge but the graph had no edge to add.
    Nothing,
}

/// A resolved regime change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regime {
    /// When it takes effect.
    pub at: Instant,
    /// What it did.
    pub detail: RegimeDetail,
}

/// The physics and graph in force from `from` until the next epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Epoch {
    pub(crate) from: Instant,
    pub(crate) physics: Physics,
    pub(crate) services: Vec<Service>,
}

/// Every epoch of a stream, the first starting at zero.
pub(crate) fn epochs(
    seed: u64,
    initial: &[Service],
    schedule: &[RegimeSchedule],
) -> (Vec<Epoch>, Vec<Regime>) {
    let mut epochs = vec![Epoch {
        from: Instant::ZERO,
        physics: Physics::base(),
        services: initial.to_vec(),
    }];
    let mut regimes = Vec::new();
    for (i, s) in schedule.iter().enumerate() {
        let mut g = Gen::keyed(&[seed, domain::WORLD, 100 + i as u64]);
        let mut next = epochs.last().expect("one epoch exists").clone();
        next.from = Instant(s.at_ns);
        let detail = match s.change {
            RegimeKind::SignatureShift => {
                let kind = FaultKind::ALL[g.below(5) as usize];
                let others: Vec<FaultKind> =
                    FaultKind::ALL.into_iter().filter(|k| *k != kind).collect();
                let now_emits = others[g.below(4) as usize];
                next.physics.char_msg[kind as usize] = characteristic_message(now_emits);
                RegimeDetail::SignatureShift { kind, now_emits }
            }
            RegimeKind::EdgeAdd => {
                let services = &next.services;
                let mut candidates = Vec::new();
                for a in 1..services.len() {
                    for b in 0..a {
                        let linked = services[a].depends_on.iter().any(|d| d.index() == b);
                        let already = dependents_mask(services, ServiceId(b as u32))[a];
                        if !linked && !already {
                            candidates.push((a, b));
                        }
                    }
                }
                if candidates.is_empty() {
                    RegimeDetail::Nothing
                } else {
                    let (a, b) = candidates[g.below(candidates.len() as u64) as usize];
                    let dep = &mut next.services[a].depends_on;
                    dep.push(ServiceId(b as u32));
                    dep.sort();
                    RegimeDetail::EdgeAdd {
                        dependent: ServiceId(a as u32),
                        dependency: ServiceId(b as u32),
                    }
                }
            }
        };
        regimes.push(Regime {
            at: Instant(s.at_ns),
            detail,
        });
        epochs.push(next);
    }
    (epochs, regimes)
}

/// The epoch in force at `t`: the last one that started at or before it.
pub(crate) fn epoch_at(epochs: &[Epoch], t: Instant) -> &Epoch {
    let i = epochs.partition_point(|e| e.from <= t);
    &epochs[i.saturating_sub(1)]
}

/// True when neither service depends on the other, directly or transitively.
pub(crate) fn incomparable(services: &[Service], a: ServiceId, b: ServiceId) -> bool {
    a != b && !dependents_mask(services, a)[b.index()] && !dependents_mask(services, b)[a.index()]
}

/// The time-zero graph: 6 to 12 services, each depending on one earlier service and, with
/// probability 1/5 each, on every other earlier one. Retried (deterministically) until at least
/// two pairs of services are unconnected, which a cascade needs; the last attempt is used if
/// none succeeds, in which case the stream simply has no cascade.
pub(crate) fn gen_graph(seed: u64, min: u8, max: u8) -> Vec<Service> {
    let mut last = Vec::new();
    for attempt in 0..64u64 {
        let mut g = Gen::keyed(&[seed, domain::WORLD, 0, attempt]);
        let n = g.range(min as u64, max as u64) as usize;
        let mut services = Vec::with_capacity(n);
        for i in 0..n {
            let mut depends_on = Vec::new();
            if i > 0 {
                let first = g.below(i as u64) as usize;
                for j in 0..i {
                    if j == first || g.below(5) == 0 {
                        depends_on.push(ServiceId(j as u32));
                    }
                }
            }
            services.push(Service {
                id: ServiceId(i as u32),
                depends_on,
                resource: ResourceKind::ALL[g.below(4) as usize],
                config_hash: g.u64(),
                unreliable_health: false,
            });
        }
        let mut pairs = 0;
        for a in 0..n {
            for b in (a + 1)..n {
                if incomparable(&services, ServiceId(a as u32), ServiceId(b as u32)) {
                    pairs += 1;
                }
            }
        }
        let ok = pairs >= 2;
        last = services;
        if ok {
            break;
        }
    }
    last
}
