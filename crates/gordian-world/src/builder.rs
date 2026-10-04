//! Episode generation. Internal; the public entry point is `episode::generate`.
//!
//! Draw discipline: every random draw that fixes the *instance* (kind, site, bits, drift hash,
//! decoy, onset) is taken unconditionally and in a fixed order, so that two episodes from the
//! same seed whose truths differ only in kind within an ambiguity set have byte-identical public
//! streams. `tests::leakage` relies on this.

use crate::episode::{
    ComponentDirective, ComponentMode, Episode, EpisodeClass, EpisodeSpec, Hidden, PriorRecord,
    StreamLabel,
};
use crate::fault::{Fault, FaultKind};
use crate::graph::{ResourceKind, Service, ServiceId, World};
use crate::physics::{
    CATALOGUE_LIMIT, ENTANGLED, HIGH, Role, SignalText, SymptomTag, characteristic_message,
    counters, messages,
};
use crate::sense::{CounterName, Observation, Severity};
use gordian_core::Instant;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

const MS: u64 = 1_000_000;

/// Seeded generator with the few helpers the builder needs.
struct Gen(ChaCha8Rng);

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Gen {
    /// The 32-byte ChaCha seed is expanded from the u64 with splitmix64 here, not with
    /// `SeedableRng::seed_from_u64`, so that no library's seed expansion is a hidden input.
    fn new(seed: u64) -> Self {
        let mut state = seed;
        let mut bytes = [0u8; 32];
        for chunk in bytes.chunks_mut(8) {
            chunk.copy_from_slice(&splitmix(&mut state).to_le_bytes());
        }
        Gen(ChaCha8Rng::from_seed(bytes))
    }

    fn u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// Uniform in `0..n` by rejection sampling. `n` must be positive.
    fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.u64();
            if x >= threshold {
                return x % n;
            }
        }
    }

    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// Which symptom pattern the true signal takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Profile {
    /// No fault.
    NoFault,
    /// Only the shared first symptom: every kind fits.
    Plain,
    /// Latency plus `MixedSignals`: `DependencyDown` and `Intermittent` fit.
    Mixed,
    /// The full signature of the true kind, so that the stream alone decides it.
    Identified,
    /// One low-severity characteristic message and nothing else.
    Quiet,
    /// A changed snapshot, then the plain shared symptom after a delay.
    DelayedPlain,
}

const PLAIN_KINDS: [FaultKind; 3] = [
    FaultKind::ResourceExhausted,
    FaultKind::ConfigDrift,
    FaultKind::CredentialExpired,
];

fn gen_world(g: &mut Gen, spec: &EpisodeSpec) -> World {
    let n = g.range(spec.min_services as u64, spec.max_services as u64) as usize;
    let mut services = Vec::with_capacity(n);
    for i in 0..n {
        let mut depends_on = Vec::new();
        if i > 0 {
            let first = g.below(i as u64) as usize;
            for j in 0..i {
                if j == first || g.chance(1, 4) {
                    depends_on.push(ServiceId(j as u32));
                }
            }
        }
        let resource = ResourceKind::ALL[g.below(4) as usize];
        services.push(Service {
            id: ServiceId(i as u32),
            depends_on,
            resource,
            config_hash: g.u64(),
            unreliable_health: false,
        });
    }
    World { services }
}

/// One true observation, at `off_ms` after onset.
struct Sig {
    off_ms: u64,
    obs: Observation,
}

fn hot(g: &mut Gen) -> u64 {
    g.range(HIGH + 10, 100)
}

fn msg(service: ServiceId, text: SignalText, severity: Severity) -> Observation {
    Observation::Message {
        service,
        text_id: text.text_id(),
        severity,
    }
}

/// The true signal for a profile. Draw counts for `Plain` and `Mixed` do not depend
/// on `kind`.
fn signals(
    g: &mut Gen,
    profile: Profile,
    kind: FaultKind,
    site: ServiceId,
    world: &World,
) -> Vec<Sig> {
    let deps: Vec<ServiceId> = world.dependents_of(site).into_iter().take(6).collect();
    let anchor = Sig {
        off_ms: 0,
        obs: Observation::Counter {
            service: site,
            name: CounterName::ErrorRate,
            value: hot(g),
        },
    };
    let mut out = Vec::new();
    match profile {
        Profile::NoFault => {}
        Profile::Plain | Profile::DelayedPlain => out.push(anchor),
        Profile::Mixed => {
            out.push(anchor);
            let off_ms = g.range(1, 20);
            out.push(Sig {
                off_ms,
                obs: Observation::Counter {
                    service: site,
                    name: CounterName::Latency,
                    value: hot(g),
                },
            });
            let off_ms = g.range(1, 20);
            let severity = Severity::ALL[g.below(4) as usize];
            out.push(Sig {
                off_ms,
                obs: msg(site, SignalText::MixedSignals, severity),
            });
            for dep in deps {
                let off_ms = g.range(1, 60);
                out.push(Sig {
                    off_ms,
                    obs: Observation::Counter {
                        service: dep,
                        name: CounterName::Latency,
                        value: hot(g),
                    },
                });
            }
        }
        Profile::Identified => {
            out.push(anchor);
            for name in counters(kind, Role::Site)
                .iter()
                .filter(|n| **n != CounterName::ErrorRate)
            {
                let off_ms = g.range(1, 20);
                out.push(Sig {
                    off_ms,
                    obs: Observation::Counter {
                        service: site,
                        name: *name,
                        value: hot(g),
                    },
                });
            }
            let off_ms = g.range(1, 20);
            let severity = Severity::ALL[g.below(4) as usize];
            out.push(Sig {
                off_ms,
                obs: msg(site, characteristic_message(kind), severity),
            });
            for dep in deps {
                for name in counters(kind, Role::Dependent) {
                    let off_ms = g.range(1, 60);
                    out.push(Sig {
                        off_ms,
                        obs: Observation::Counter {
                            service: dep,
                            name: *name,
                            value: hot(g),
                        },
                    });
                }
                for text in messages(kind, Role::Dependent) {
                    let off_ms = g.range(1, 60);
                    out.push(Sig {
                        off_ms,
                        obs: msg(dep, *text, Severity::ALL[g.below(4) as usize]),
                    });
                }
            }
        }
        Profile::Quiet => out.push(Sig {
            off_ms: 0,
            obs: msg(site, characteristic_message(kind), Severity::Low),
        }),
    }
    out
}

/// An irrelevant observation. In a flood, always a free-form message with a uniform severity.
fn noise_obs(
    g: &mut Gen,
    services: &[Service],
    flood: bool,
    keep_snapshot_off: Option<ServiceId>,
) -> Observation {
    let n = services.len() as u64;
    let roll = if flood { 0 } else { g.below(10) };
    let service = ServiceId(g.below(n) as u32);
    if roll < 6 {
        let mut id = g.u64();
        if id < CATALOGUE_LIMIT {
            id |= 1 << 40;
        }
        Observation::Message {
            service,
            text_id: id,
            severity: Severity::ALL[g.below(4) as usize],
        }
    } else if roll < 9 {
        Observation::Counter {
            service,
            name: CounterName::ALL[g.below(5) as usize],
            value: g.below(HIGH),
        }
    } else {
        // A periodic, unchanged snapshot. Never for the drifted service: it would be stale.
        let service = if Some(service) == keep_snapshot_off {
            ServiceId(((service.0 as u64 + 1) % n) as u32)
        } else {
            service
        };
        Observation::Snapshot {
            service,
            config_hash: services[service.index()].config_hash,
        }
    }
}

fn base_records() -> Vec<PriorRecord> {
    let mut records = Vec::new();
    for kind in FaultKind::ALL {
        let site_counters = counters(kind, Role::Site)
            .iter()
            .map(|c| SymptomTag::Counter(*c));
        let text = SymptomTag::Text(characteristic_message(kind));
        let mut full: Vec<SymptomTag> = site_counters.chain([text]).collect();
        full.sort();
        records.push(PriorRecord {
            signature: full.clone(),
            resolution: kind,
        });
        records.push(PriorRecord {
            signature: vec![text],
            resolution: kind,
        });
        if kind == FaultKind::DependencyDown {
            let mut with_dep = full;
            with_dep.push(SymptomTag::Text(SignalText::UpstreamUnreachable));
            with_dep.sort();
            records.push(PriorRecord {
                signature: with_dep,
                resolution: kind,
            });
        }
    }
    records
}

pub(crate) fn build(spec: &EpisodeSpec) -> Episode {
    build_inner(spec, None)
}

/// Test hook: same draws as `build`, but the true kind is replaced after all draws are made.
#[cfg(test)]
pub(crate) fn build_forced(spec: &EpisodeSpec, kind: FaultKind) -> Episode {
    build_inner(spec, Some(kind))
}

fn build_inner(spec: &EpisodeSpec, forced: Option<FaultKind>) -> Episode {
    let spec = spec.normalized();
    let mut g = Gen::new(spec.seed);
    let mut world = gen_world(&mut g, &spec);
    let n = world.services.len() as u64;

    // Instance draws, unconditional and in fixed order.
    let coin = g.below(4);
    let kind_draw = g.below(5) as usize;
    let site = ServiceId(g.below(n) as u32);
    let decoy_draw = g.below(n - 1) as u32;
    let b1 = g.chance(1, 2);
    let drift_draw = g.u64();

    let (profile, natural) = match spec.class {
        EpisodeClass::NoFault => (Profile::NoFault, None),
        EpisodeClass::Ambiguous => (Profile::Plain, Some(PLAIN_KINDS[kind_draw % 3])),
        EpisodeClass::DelayedConfigChange => (Profile::DelayedPlain, Some(FaultKind::ConfigDrift)),
        EpisodeClass::NoiseFlood | EpisodeClass::Duplicates | EpisodeClass::ComponentTimeout => {
            (Profile::Identified, Some(FaultKind::ALL[kind_draw]))
        }
        EpisodeClass::JointlyDecisive => (
            Profile::Mixed,
            Some(if kind_draw.is_multiple_of(2) {
                ENTANGLED.0
            } else {
                ENTANGLED.1
            }),
        ),
        EpisodeClass::CriticalFault => {
            if coin < 2 {
                (Profile::Plain, Some(PLAIN_KINDS[kind_draw % 3]))
            } else {
                (Profile::Identified, Some(FaultKind::ALL[kind_draw]))
            }
        }
        EpisodeClass::QuietUrgent => (Profile::Quiet, Some(FaultKind::ALL[kind_draw])),
        EpisodeClass::FeedbackBait | EpisodeClass::StaleMemory => {
            (Profile::Plain, Some(PLAIN_KINDS[kind_draw % 3]))
        }
    };
    let kind = natural.map(|k| forced.unwrap_or(k));
    let critical = matches!(
        spec.class,
        EpisodeClass::CriticalFault | EpisodeClass::QuietUrgent
    );
    let horizon_ns = spec.horizon.0;
    let hz_ms = horizon_ns / MS;

    // Timeline in milliseconds. Delayed class: snapshot at ts, first symptom `gap` later.
    let (ts_ms, gap_ms, onset_ms) = if profile == Profile::DelayedPlain {
        let ts = g.range(10, 300);
        let gap = spec.delay_k as u64 + 6 + g.range(0, 50);
        (ts, gap, ts + gap)
    } else {
        let onset = g.range(hz_ms / 10, hz_ms / 4);
        (onset, 0, onset)
    };

    let mut items: Vec<(u64, Observation, StreamLabel)> = Vec::new();
    let mut fault = None;
    let mut bits = (false, false);
    let mut drift_hash = None;

    if let Some(kind) = kind {
        fault = Some(Fault {
            kind,
            site,
            onset: Instant(onset_ms * MS),
            critical,
        });
        if kind == ENTANGLED.0 {
            bits = (b1, b1);
        } else if kind == ENTANGLED.1 {
            bits = (b1, !b1);
        }
        let start = world.services[site.index()].config_hash;
        if kind == FaultKind::ConfigDrift {
            let drift = if drift_draw == start {
                drift_draw.wrapping_add(1)
            } else {
                drift_draw
            };
            drift_hash = Some(drift);
        }

        let mut group = 0u32;
        if profile == Profile::DelayedPlain {
            items.push((
                ts_ms * MS,
                Observation::Snapshot {
                    service: site,
                    config_hash: drift_hash.unwrap_or(start),
                },
                StreamLabel::Signal { group },
            ));
            group += 1;
        }
        let duplicate = spec.class == EpisodeClass::Duplicates;
        for sig in signals(&mut g, profile, kind, site, &world) {
            let copies = if duplicate { g.range(2, 5) } else { 1 };
            let mut at = (onset_ms + sig.off_ms) * MS;
            for c in 0..copies {
                if c > 0 {
                    at += g.range(1, 20) * MS;
                }
                items.push((at, sig.obs.clone(), StreamLabel::Signal { group }));
            }
            group += 1;
        }
    }

    if spec.class == EpisodeClass::FeedbackBait {
        let decoy = ServiceId(if decoy_draw >= site.0 {
            decoy_draw + 1
        } else {
            decoy_draw
        });
        world.services[decoy.index()].unreliable_health = true;
        for _ in 0..g.range(2, 4) {
            let at = (onset_ms + g.range(0, 200)) * MS;
            items.push((
                at,
                msg(decoy, SignalText::CheckHealth, Severity::Medium),
                StreamLabel::Bait,
            ));
        }
    }

    // Noise.
    let flood = matches!(
        spec.class,
        EpisodeClass::NoiseFlood | EpisodeClass::QuietUrgent
    );
    let true_count = items.len() as u64;
    let base = true_count.max(8);
    let mut noise_count = spec.noise_rate as u64 * base;
    if flood {
        noise_count = noise_count.max(4 * true_count).max(50);
    }
    // Only in DelayedConfigChange is the site's periodic snapshot suppressed (a stale unchanged
    // snapshot after the change would be misleading). The site is public there anyway: the
    // changed snapshot names it. In every other class suppression would leak the site, and for
    // a kind-dependent rule the kind, through the *absence* of snapshots. The leakage tests
    // caught the kind-dependent version of this.
    let avoid = fault
        .as_ref()
        .filter(|_| profile == Profile::DelayedPlain)
        .map(|f| f.site);
    for _ in 0..noise_count {
        let at = g.below(horizon_ns + 1);
        let obs = noise_obs(&mut g, &world.services, flood, avoid);
        items.push((at, obs, StreamLabel::Noise));
    }
    if profile == Profile::DelayedPlain {
        // At least k irrelevant observations strictly between the snapshot and the first symptom.
        let (lo, hi) = (ts_ms * MS, onset_ms * MS);
        debug_assert!(hi - lo > 2 * MS && gap_ms > spec.delay_k as u64);
        for _ in 0..spec.delay_k as u64 + g.range(0, 3) {
            let at = lo + 1 + g.below(hi - lo - 1);
            let obs = noise_obs(&mut g, &world.services, false, avoid);
            items.push((at, obs, StreamLabel::Noise));
        }
    }

    // Stable sort: ties keep insertion order, so true signals precede noise at equal instants.
    items.sort_by_key(|(at, _, _)| *at);
    let stream: Vec<(Instant, Observation)> = items
        .iter()
        .map(|(at, obs, _)| (Instant(*at), obs.clone()))
        .collect();
    let labels: Vec<StreamLabel> = items.iter().map(|(_, _, l)| *l).collect();

    // Prior records: fixed correct ones, plus one volatile one (for the plain first symptom)
    // whose resolution is drawn in every episode. In StaleMemory it is forced wrong.
    let mut prior = base_records();
    let v_plain = FaultKind::ALL[g.below(5) as usize];
    let shift = g.range(1, 4) as usize;
    let plain_sig = vec![SymptomTag::Counter(CounterName::ErrorRate)];
    let wrong = |k: FaultKind| FaultKind::ALL[(k as usize + shift) % 5];
    let mut plain_res = v_plain;
    if let (EpisodeClass::StaleMemory, Some(kind)) = (spec.class, kind)
        && plain_res == kind
    {
        plain_res = wrong(kind);
    }
    prior.push(PriorRecord {
        signature: plain_sig,
        resolution: plain_res,
    });

    // Harness directives.
    let mut directives = Vec::new();
    if spec.class == EpisodeClass::ComponentTimeout {
        let count = g.range(1, 3) as usize;
        let mut pool: Vec<u32> = (0..8).collect();
        for _ in 0..count {
            let idx = g.below(pool.len() as u64) as usize;
            let component = pool.remove(idx);
            let mode = if g.chance(1, 2) {
                ComponentMode::Fail
            } else {
                ComponentMode::Slow {
                    factor: g.range(2, 10) as u32,
                }
            };
            directives.push(ComponentDirective { component, mode });
        }
    }

    Episode {
        spec,
        world,
        stream,
        directives,
        prior,
        hidden: Hidden {
            faults: fault.into_iter().collect(),
            bits,
            drift_hash,
            labels,
        },
    }
}
