//! The generated stream: public observations beside hidden incidents.
//!
//! [`Stream`] is deliberately neither `Serialize` nor `Debug`-transparent: it holds the tiers,
//! the true hypotheses, the deadlines and every observation's label. A policy never holds one;
//! the harness holds a [`crate::StreamSimulator`] built from it. The first world derived
//! `Serialize` on `Episode` and so on its hidden state (review log, A1); this type does not, and
//! a test pins that.

use crate::incident::{Family, Incident, Mode, build, dependents, make_pool};
use crate::kinds::{HardKind, Tier};
use crate::labels::ObsLabel;
use crate::noise;
use crate::params::{StreamParams, StreamPublic, timing};
use crate::regime::{Epoch, Regime, epoch_at, epochs, gen_graph, incomparable};
use crate::rng::{Gen, domain};
use gordian_core::Instant;
use gordian_world::{FaultKind, Observation, Service, ServiceId};

/// Size of the pool of free-form message ids. Sixteen are the hard kinds' vocabulary, four each.
pub(crate) const POOL_SIZE: usize = 48;

/// A generated stream. Hidden state lives in private fields; see `oracle::reveal`.
#[derive(Clone, PartialEq)]
pub struct Stream {
    pub(crate) params: StreamParams,
    pub(crate) services: Vec<Service>,
    pub(crate) events: Vec<(Instant, Observation)>,
    pub(crate) labels: Vec<ObsLabel>,
    pub(crate) incidents: Vec<Incident>,
    pub(crate) decisive_total: Vec<u32>,
    pub(crate) epochs: Vec<Epoch>,
    pub(crate) regimes: Vec<Regime>,
    pub(crate) skipped: u32,
}

impl std::fmt::Debug for Stream {
    /// Counts only. The derived `Debug` would print every hidden field into any log a harness
    /// writes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("observations", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl Stream {
    /// The passive observation stream, sorted by instant. Public by construction: it is what the
    /// simulator delivers to a policy.
    pub fn events(&self) -> &[(Instant, Observation)] {
        &self.events
    }

    /// What a policy may know at the start.
    pub fn public_info(&self) -> StreamPublic {
        StreamPublic {
            services: self.services.clone(),
            duration_ns: self.params.duration_ns,
            deadlines: self.params.deadlines,
            reasoner_cost: self.params.reasoner.cost,
            max_context: self.params.max_context,
            budget: self.params.budget,
        }
    }
}

/// Generate a stream. A pure function of `params`: no clock, no global state, no hash-order
/// dependence. The parameters are normalized first.
pub fn generate(params: &StreamParams) -> Stream {
    build_stream(params, None)
}

/// What a test may force on the incident that arrives with index `arrival`, after every draw is
/// made: its tier, or (for a plain incident that leaves two kinds open) which of the two it is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Forced {
    pub(crate) arrival: u32,
    pub(crate) tier: Option<Tier>,
    pub(crate) duo_kind: Option<FaultKind>,
}

/// Test hook: the same draws as [`generate`], with `forced` applied.
#[cfg(test)]
pub(crate) fn generate_forced(params: &StreamParams, forced: Forced) -> Stream {
    build_stream(params, Some(forced))
}

const ORDERED_PAIRS: [(FaultKind, FaultKind); 6] = [
    (FaultKind::ResourceExhausted, FaultKind::ConfigDrift),
    (FaultKind::ResourceExhausted, FaultKind::CredentialExpired),
    (FaultKind::ConfigDrift, FaultKind::ResourceExhausted),
    (FaultKind::ConfigDrift, FaultKind::CredentialExpired),
    (FaultKind::CredentialExpired, FaultKind::ResourceExhausted),
    (FaultKind::CredentialExpired, FaultKind::ConfigDrift),
];

fn build_stream(params: &StreamParams, forced: Option<Forced>) -> Stream {
    let p = params.normalized();
    let seed = p.seed;
    let services = gen_graph(seed, p.min_services, p.max_services);
    let (epochs, regimes) = epochs(seed, &services, &p.regimes);
    let pool = make_pool(seed, POOL_SIZE);

    let mut arrivals = Gen::keyed(&[seed, domain::ARRIVAL]);
    let last_arrival = p.duration_ns.saturating_sub(p.tail_ns);
    let mut t = 0u64;
    let mut arrival = 0u32;
    let mut skipped = 0u32;
    let mut incidents: Vec<Incident> = Vec::new();
    let mut items: Vec<(u64, Observation, ObsLabel)> = Vec::new();

    loop {
        t = t.saturating_add(arrivals.exp_gap_ns(p.mean_gap_ns));
        if t > last_arrival {
            break;
        }
        let this = arrival;
        arrival += 1;

        let ep = epoch_at(&epochs, Instant(t));
        let svc = &ep.services;
        let n = svc.len();

        // Every draw, unconditionally and in a fixed order: nothing below changes how many
        // words any generator consumes, so the tier (which a test may force) cannot change them.
        let mut gs = Gen::keyed(&[seed, domain::SHAPE, this as u64]);
        let tier_u = gs.below(1000) as u32;
        let rec_coin = gs.below(1000) as u32;
        let rec_pick = gs.u64();
        let known_draw = gs.below(5) as usize;
        let hard_draw = gs.below(4) as usize;
        let site_draw = gs.u64();
        let partner_draw = gs.u64();
        let mode_draw = gs.below(2);
        let profile_draw = gs.below(1000) as u32;
        let pair_draw = gs.below(6) as usize;
        let casc_draw = gs.below(2) as usize;
        let b1 = gs.below(2) == 1;
        let drift_draw = gs.u64();
        let crit_draw = gs.below(1000) as u32;
        let mut gm = Gen::keyed(&[seed, domain::MISC, this as u64]);
        let d_u = gm.unit();
        let dl_u = gm.unit();
        let grace_u = gm.unit();
        let rec_u = gm.unit();

        let busy = |s: ServiceId| {
            incidents
                .iter()
                .any(|i| i.occupies.contains(&s) && i.busy_until.0 > t)
        };
        let free: Vec<ServiceId> = (0..n as u32).map(ServiceId).filter(|s| !busy(*s)).collect();

        // Recurrence: repeat an earlier incident whose services are all free again.
        let eligible: Vec<usize> = incidents
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                i.occupies.iter().all(|s| !busy(*s))
                    && match i.family {
                        Family::Cascade { partner, .. } => incomparable(svc, i.site, partner),
                        _ => true,
                    }
            })
            .map(|(k, _)| k)
            .collect();
        let recurring = rec_coin < p.recurrence_permille && !eligible.is_empty();

        let (mut tier, family, site, critical, difficulty, recurrence_of);
        if recurring {
            let tmpl = &incidents[eligible[(rec_pick % eligible.len() as u64) as usize]];
            tier = tmpl.tier;
            family = tmpl.family;
            site = tmpl.site;
            critical = tmpl.critical;
            difficulty = tmpl.difficulty;
            recurrence_of = Some(tmpl.id);
        } else {
            tier = if tier_u < p.mix.plain_permille {
                Tier::Plain
            } else if tier_u < p.mix.plain_permille + p.mix.hard_permille {
                Tier::Hard
            } else {
                Tier::Decoy
            };
            if let Some(f) = forced
                && f.arrival == this
                && let Some(forced_tier) = f.tier
            {
                tier = forced_tier;
            }
            recurrence_of = None;
            let picked = match tier {
                Tier::Plain => {
                    if free.is_empty() {
                        None
                    } else {
                        let site = free[(site_draw % free.len() as u64) as usize];
                        let kind = FaultKind::ALL[known_draw];
                        let duo = matches!(
                            kind,
                            FaultKind::ResourceExhausted | FaultKind::DependencyDown
                        ) && profile_draw < 400
                            && !dependents(svc, site).is_empty();
                        Some((Family::Known { kind, duo }, site))
                    }
                }
                Tier::Hard | Tier::Decoy => {
                    let mode = if mode_draw == 0 {
                        Mode::Mimic
                    } else {
                        Mode::Contradict
                    };
                    let mut chosen = None;
                    for off in 0..4 {
                        let hk = HardKind::ALL[(hard_draw + off) % 4];
                        let found = match hk {
                            HardKind::Compound => pick_site(&free, site_draw).map(|s| {
                                let (a, b) = ORDERED_PAIRS[pair_draw];
                                (Family::Compound { a, b, mode }, s)
                            }),
                            HardKind::SlowLeak => {
                                pick_site(&free, site_draw).map(|s| (Family::Leak, s))
                            }
                            HardKind::SplitBrain => {
                                if free.len() >= 2 {
                                    let s = free[(site_draw % free.len() as u64) as usize];
                                    let rest: Vec<ServiceId> =
                                        free.iter().copied().filter(|x| *x != s).collect();
                                    let peer = rest[(partner_draw % rest.len() as u64) as usize];
                                    Some((Family::SplitBrain { peer, mode }, s))
                                } else {
                                    None
                                }
                            }
                            HardKind::Cascade => {
                                let with_partner: Vec<ServiceId> = free
                                    .iter()
                                    .copied()
                                    .filter(|s| free.iter().any(|q| incomparable(svc, *s, *q)))
                                    .collect();
                                pick_site(&with_partner, site_draw).map(|s| {
                                    let partners: Vec<ServiceId> = free
                                        .iter()
                                        .copied()
                                        .filter(|q| incomparable(svc, s, *q))
                                        .collect();
                                    let partner =
                                        partners[(partner_draw % partners.len() as u64) as usize];
                                    let a =
                                        [FaultKind::ResourceExhausted, FaultKind::DependencyDown]
                                            [casc_draw];
                                    (Family::Cascade { a, partner, mode }, s)
                                })
                            }
                        };
                        if found.is_some() {
                            chosen = found;
                            break;
                        }
                    }
                    chosen
                }
            };
            let Some((family_new, site_new)) = picked else {
                skipped += 1;
                continue;
            };
            family = match (forced, family_new) {
                (
                    Some(Forced {
                        arrival,
                        duo_kind: Some(k),
                        ..
                    }),
                    Family::Known { duo: true, .. },
                ) if arrival == this => Family::Known { kind: k, duo: true },
                (_, f) => f,
            };
            site = site_new;
            critical = match tier {
                Tier::Plain => crit_draw < p.critical.plain_permille,
                Tier::Hard => crit_draw < p.critical.hard_permille,
                Tier::Decoy => false,
            };
            let range = match tier {
                Tier::Plain => p.difficulty.plain,
                Tier::Hard => p.difficulty.hard,
                Tier::Decoy => p.difficulty.decoy,
            };
            difficulty = range.lo + d_u * (range.hi - range.lo);
        }

        let onset = Instant(t);
        let deadline = p
            .deadlines
            .window(tier, critical)
            .map(|w| Instant(t + w.lo_ns + (dl_u * (w.hi_ns - w.lo_ns) as f64) as u64));
        let live_end = match deadline {
            Some(d) => {
                let (lo, hi) = timing::GRACE_NS;
                Instant(d.0 + lo + (grace_u * (hi - lo) as f64) as u64)
            }
            None => Instant(t + timing::T0_NS + (rec_u * timing::DECOY_SPAN_NS as f64) as u64),
        };
        let busy_until = Instant(
            live_end.0
                + timing::RECOVERY_TOTAL as u64 * timing::BEAT_MS.1 * 1_000_000
                + timing::BUSY_TAIL_NS,
        );
        let mut occupies = vec![site];
        occupies.extend(family.others());
        let entangled_kind = match family {
            Family::Known { kind, .. } => Some(kind),
            Family::Cascade { a, .. } => Some(a),
            _ => None,
        };
        let bits = match (family, entangled_kind) {
            (Family::SplitBrain { .. }, _) => (true, true),
            (_, Some(FaultKind::DependencyDown)) => (b1, b1),
            (_, Some(FaultKind::Intermittent)) => (b1, !b1),
            _ => (false, false),
        };
        let start_hash = services[site.index()].config_hash;
        let drift_hash = if drift_draw == start_hash {
            drift_draw.wrapping_add(1)
        } else {
            drift_draw
        };
        let mut inc = Incident {
            id: incidents.len() as u32,
            arrival: this,
            tier,
            critical,
            onset,
            site,
            family,
            truth: Incident::truth_of(tier, &family, site),
            difficulty,
            deadline,
            live_end,
            busy_until,
            occupies,
            bits,
            drift_hash,
            recurrence_of,
            cross: None,
        };
        let generated = build(&mut inc, seed, svc, &ep.physics, &pool);
        for it in generated {
            items.push((
                it.at,
                it.obs,
                ObsLabel::Incident {
                    id: inc.id,
                    role: it.role,
                },
            ));
        }
        incidents.push(inc);
    }

    for n in noise::generate(
        seed,
        &p.noise,
        p.duration_ns,
        services.len(),
        &epochs,
        &pool,
    ) {
        items.push((n.at, n.obs, ObsLabel::Background(n.kind)));
    }

    // Stable sort: ties keep generation order, so incident evidence precedes noise at an equal
    // instant and an incident's own observations keep the order they were built in.
    items.retain(|(at, _, _)| *at <= p.duration_ns);
    items.sort_by_key(|(at, _, _)| *at);
    let mut events = Vec::with_capacity(items.len());
    let mut labels = Vec::with_capacity(items.len());
    let mut decisive_total = vec![0u32; incidents.len()];
    for (at, obs, label) in items {
        if let ObsLabel::Incident {
            id,
            role: crate::labels::EvidenceRole::Decisive,
        } = label
        {
            decisive_total[id as usize] += 1;
        }
        events.push((Instant(at), obs));
        labels.push(label);
    }

    Stream {
        params: p,
        services,
        events,
        labels,
        incidents,
        decisive_total,
        epochs,
        regimes,
        skipped,
    }
}

fn pick_site(sites: &[ServiceId], draw: u64) -> Option<ServiceId> {
    if sites.is_empty() {
        None
    } else {
        Some(sites[(draw % sites.len() as u64) as usize])
    }
}
