//! Hidden incident plans and the observations each produces.
//!
//! An incident is generated in up to four parts, each from its own generator keyed by the
//! incident's arrival index:
//!
//! 1. **Burst** (`domain::BURST`): the first 300 ms of evidence. For a plain incident this is a
//!    first-world signature. For a hard incident and a decoy of the same family it is the same
//!    draws, so the same observations.
//! 2. **Heartbeats** (`domain::PULSE`): one reading every 0.8 to 1.5 s at the site, until the
//!    incident's last effect, then ten benign readings. Every beat draws its gap, its flap
//!    coin, an abnormal value and a benign value whether or not it uses them, so a hard
//!    incident and a decoy draw identically until the decoy resolves.
//! 3. **Phase-2 evidence** (`domain::PHASE2`): hard incidents only. Observations in
//!    `[T0, T0 + 10 s)` that break the first world's rules (visible to the cheap checker as a
//!    contradiction) or carry ids from a hidden vocabulary (invisible to it).
//! 4. **Recovery**: a decoy's heartbeats turn benign at its resolution time.
//!
//! The tier appears in the decisions *about* the draws (which role a reading has, whether
//! phase-2 evidence is emitted), never in the draws themselves.

use crate::kinds::{Diagnosis, HardKind, StreamHypothesis, StreamKind, Tier};
use crate::labels::EvidenceRole;
use crate::params::{MS, timing};
use crate::present::{
    Sig, calm, counter, duo, hot, identified, mixed, partner_alarm, peer_alarm, severity, text,
};
use crate::regime::Physics;
use crate::rng::{Gen, domain};
use gordian_core::Instant;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::HIGH;
use gordian_world::{CounterName, FaultKind, Observation, Service, ServiceId};

/// How a hard incident or decoy presents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Phase 1 looks like a plain incident of a known kind; the evidence that breaks the first
    /// world's rules arrives in phase 2.
    Mimic,
    /// Phase 1 already breaks the first world's rules.
    Contradict,
}

/// The structure of an incident: what recurrence repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// A plain incident of a known kind. `duo` leaves two kinds open and needs one probe.
    Known { kind: FaultKind, duo: bool },
    /// Kinds `a` and `b` at one site. `a` shows first.
    Compound {
        a: FaultKind,
        b: FaultKind,
        mode: Mode,
    },
    /// Known kind `a` at the site, damaging `partner` over a hidden edge.
    Cascade {
        a: FaultKind,
        partner: ServiceId,
        mode: Mode,
    },
    /// The site and `peer` both act as primary.
    SplitBrain { peer: ServiceId, mode: Mode },
    /// A slow climb of the site's saturation reading.
    Leak,
}

impl Family {
    /// The hard kind this family presents as, if any.
    pub(crate) fn hard_kind(&self) -> Option<HardKind> {
        match self {
            Family::Known { .. } => None,
            Family::Compound { .. } => Some(HardKind::Compound),
            Family::Cascade { .. } => Some(HardKind::Cascade),
            Family::SplitBrain { .. } => Some(HardKind::SplitBrain),
            Family::Leak => Some(HardKind::SlowLeak),
        }
    }

    /// The known kind the cheap rung is led to by the first moments, if any.
    pub(crate) fn mimic(&self) -> Option<FaultKind> {
        match self {
            Family::Known { .. } => None,
            Family::Compound { a, .. } | Family::Cascade { a, .. } => Some(*a),
            Family::SplitBrain { .. } => Some(FaultKind::DependencyDown),
            Family::Leak => Some(FaultKind::ResourceExhausted),
        }
    }

    /// The services this family involves besides the site.
    pub(crate) fn others(&self) -> Option<ServiceId> {
        match self {
            Family::Cascade { partner, .. } => Some(*partner),
            Family::SplitBrain { peer, .. } => Some(*peer),
            _ => None,
        }
    }
}

/// A planned incident. Hidden state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Incident {
    pub(crate) id: u32,
    pub(crate) arrival: u32,
    pub(crate) tier: Tier,
    pub(crate) critical: bool,
    pub(crate) onset: Instant,
    pub(crate) site: ServiceId,
    pub(crate) family: Family,
    pub(crate) truth: Diagnosis,
    pub(crate) difficulty: f64,
    pub(crate) deadline: Option<Instant>,
    /// The last instant the incident is live: a decoy's resolution, or the others' closure.
    pub(crate) live_end: Instant,
    /// The incident reserves its services until here.
    pub(crate) busy_until: Instant,
    pub(crate) occupies: Vec<ServiceId>,
    pub(crate) bits: (bool, bool),
    pub(crate) drift_hash: u64,
    pub(crate) recurrence_of: Option<u32>,
    /// The first heartbeat of a leak that read at or above the alarm threshold.
    pub(crate) cross: Option<Instant>,
}

impl Incident {
    /// The hypothesis that is true of a plain or hard incident; `None` for a decoy.
    pub(crate) fn truth_of(tier: Tier, family: &Family, site: ServiceId) -> Diagnosis {
        let kind = match (tier, family) {
            (Tier::Decoy, _) => return None,
            (_, Family::Known { kind, .. }) => StreamKind::Known(*kind),
            (_, f) => StreamKind::Hard(f.hard_kind().expect("non-known family is hard")),
        };
        Some(StreamHypothesis { kind, site })
    }
}

/// One generated observation of an incident, at an absolute instant.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub(crate) at: u64,
    pub(crate) obs: Observation,
    pub(crate) role: EvidenceRole,
}

/// Services that depend on `site`, ascending.
pub(crate) fn dependents(services: &[Service], site: ServiceId) -> Vec<ServiceId> {
    dependents_mask(services, site)
        .iter()
        .enumerate()
        .filter(|(_, hit)| **hit)
        .map(|(i, _)| ServiceId(i as u32))
        .collect()
}

/// Index of a hard kind, for the id vocabulary.
fn hard_index(k: HardKind) -> usize {
    HardKind::ALL
        .iter()
        .position(|x| *x == k)
        .expect("kind is in ALL")
}

/// The observations of `inc`, and (as a side effect) the instant a leak first crosses the
/// threshold. `services` is the graph in force at the incident's onset.
pub(crate) fn build(
    inc: &mut Incident,
    seed: u64,
    services: &[Service],
    phys: &Physics,
    pool: &[u64],
) -> Vec<Item> {
    let onset = inc.onset.0;
    let site = inc.site;
    let deps = dependents(services, site);
    let mut items: Vec<Item> = Vec::new();
    let push = |items: &mut Vec<Item>, sigs: Vec<Sig>, role: EvidenceRole| {
        for s in sigs {
            items.push(Item {
                at: onset + s.off_ns,
                obs: s.obs,
                role,
            });
        }
    };

    // 1. Burst.
    let mut gb = Gen::keyed(&[seed, domain::BURST, inc.arrival as u64]);
    let burst_role = if inc.tier == Tier::Plain {
        EvidenceRole::Decisive
    } else {
        EvidenceRole::Presentation
    };
    let sigs = match inc.family {
        Family::Known { kind, duo: true } => {
            let _ = kind;
            duo(&mut gb, site, &deps)
        }
        Family::Known { kind, duo: false } => identified(&mut gb, phys, &[kind], site, &deps),
        Family::Compound { a, b, mode } => match mode {
            Mode::Mimic => identified(&mut gb, phys, &[a], site, &deps),
            Mode::Contradict => identified(&mut gb, phys, &[a, b], site, &deps),
        },
        Family::Cascade { a, partner, mode } => {
            let mut s = identified(&mut gb, phys, &[a], site, &deps);
            if mode == Mode::Contradict {
                let pdeps = dependents(services, partner);
                let base = gb.range(20, 150) * MS + gb.below(MS);
                s.extend(partner_alarm(&mut gb, partner, &pdeps, base));
            }
            s
        }
        Family::SplitBrain { peer, mode } => {
            let mut s = mixed(&mut gb, site, &deps);
            if mode == Mode::Contradict {
                let base = gb.range(20, 150) * MS + gb.below(MS);
                s.extend(peer_alarm(&mut gb, peer, base));
            }
            s
        }
        Family::Leak => Vec::new(),
    };
    push(&mut items, sigs, burst_role);

    // 3. Phase-2 evidence, hard incidents only.
    if inc.tier == Tier::Hard {
        let mut gp = Gen::keyed(&[seed, domain::PHASE2, inc.arrival as u64]);
        let window_ms = timing::PHASE2_NS / MS;
        let start = |g: &mut Gen| onset + timing::T0_NS + g.below(window_ms) * MS + g.below(MS);
        let hk = inc
            .family
            .hard_kind()
            .expect("hard incident has a hard family");
        // Evidence that breaks the first world's rules, for the families that start by mimicking.
        match inc.family {
            Family::Compound {
                b,
                mode: Mode::Mimic,
                ..
            } => {
                let t0 = start(&mut gp);
                let sev = severity(&mut gp);
                items.push(Item {
                    at: t0,
                    obs: text(site, phys.char_message(b), sev),
                    role: EvidenceRole::Decisive,
                });
                for name in gordian_world::physics::counters(b, gordian_world::physics::Role::Site)
                    .iter()
                    .filter(|n| **n != CounterName::ErrorRate)
                {
                    let off = gp.range(1, 20) * MS + gp.below(MS);
                    items.push(Item {
                        at: t0 + off,
                        obs: counter(site, *name, hot(&mut gp)),
                        role: EvidenceRole::Decisive,
                    });
                }
            }
            Family::Cascade {
                partner,
                mode: Mode::Mimic,
                ..
            } => {
                let t0 = start(&mut gp);
                let pdeps = dependents(services, partner);
                for s in partner_alarm(&mut gp, partner, &pdeps, 0) {
                    items.push(Item {
                        at: t0 + s.off_ns,
                        obs: s.obs,
                        role: EvidenceRole::Decisive,
                    });
                }
            }
            Family::SplitBrain {
                peer,
                mode: Mode::Mimic,
            } => {
                let t0 = start(&mut gp);
                for s in peer_alarm(&mut gp, peer, 0) {
                    items.push(Item {
                        at: t0 + s.off_ns,
                        obs: s.obs,
                        role: EvidenceRole::Decisive,
                    });
                }
            }
            _ => {}
        }
        // Evidence in a vocabulary the first world does not have.
        let n_ext = gp.range(3, 5);
        for j in 0..n_ext {
            let t = start(&mut gp);
            let id = pool[hard_index(hk) * 4 + gp.below(4) as usize];
            let service = match inc.family.others() {
                Some(other) if j % 2 == 1 => other,
                _ => site,
            };
            let sev = severity(&mut gp);
            items.push(Item {
                at: t,
                obs: Observation::Message {
                    service,
                    text_id: id,
                    severity: sev,
                },
                role: EvidenceRole::Decisive,
            });
        }
    }

    // 2 and 4. Heartbeats, and the recovery or closure that follows them.
    let mut gu = Gen::keyed(&[seed, domain::PULSE, inc.arrival as u64]);
    let leak = inc.family == Family::Leak;
    let base = gu.range(15, 25) as i64;
    let slope_x10 = gu.range(25, 40) as i64;
    let live_end_ms = (inc.live_end.0 - onset) / MS;
    let mut t_ms = 0u64;
    let mut after = 0usize;
    inc.cross = None;
    loop {
        let gap = gu.range(timing::BEAT_MS.0, timing::BEAT_MS.1);
        let flap = gu.permille(timing::FLAP_PERMILLE);
        let high = hot(&mut gu);
        let low = calm(&mut gu);
        let jitter = gu.below(5) as i64 - 2;
        let sub_ms = gu.below(MS);
        t_ms += gap;
        let live = t_ms < live_end_ms;
        let (name, value) = if leak {
            let v = if live {
                (base + slope_x10 * t_ms as i64 / 10_000 + jitter).clamp(0, 100) as u64
            } else {
                (base + jitter).max(0) as u64
            };
            if live && v >= HIGH && inc.cross.is_none() {
                inc.cross = Some(Instant(onset + t_ms * MS + sub_ms));
            }
            (CounterName::Saturation, v)
        } else {
            let v = if live && !flap { high } else { low };
            (CounterName::ErrorRate, v)
        };
        let role = if live {
            EvidenceRole::Pulse
        } else if inc.tier == Tier::Decoy && after < timing::RECOVERY_DECISIVE {
            EvidenceRole::Decisive
        } else {
            EvidenceRole::Closure
        };
        if !live {
            after += 1;
        }
        items.push(Item {
            at: onset + t_ms * MS + sub_ms,
            obs: counter(site, name, value),
            role,
        });
        if after >= timing::RECOVERY_TOTAL {
            break;
        }
    }
    items
}

/// Free-form message ids a stream uses: `n` ids at or above the catalogue limit.
pub(crate) fn make_pool(seed: u64, n: usize) -> Vec<u64> {
    let mut g = Gen::keyed(&[seed, domain::POOL]);
    let mut pool: Vec<u64> = Vec::with_capacity(n);
    while pool.len() < n {
        let id = g.u64() | (1 << 40);
        if !pool.contains(&id) {
            pool.push(id);
        }
    }
    pool
}
