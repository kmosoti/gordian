//! W3's hidden-side feature extraction for the phase-2 collision floor and for scoring A2.
//!
//! Evaluator-side (`reveal-hidden-state`); nothing here reaches an arm. For every incident it
//! reads the incident's own observations (public bytes, segmented by the hidden label, which is the
//! best case for any key: no other incident's or background symptom is in them) and writes, for a
//! ladder of cutoffs after the incident's first abnormal observation, the invariant features a key
//! could be built from, as E1's record rung builds them (`noticer_record.rs`, `key_of`, copied
//! here) and as A1c's two-site key adds a partner (`medium/`), plus the public checker's verdict
//! (`gordian_world::physics::consistent_hypotheses`) on two evidence sets:
//!
//! - **attached**: the abnormal observations at the site and at its dependents in the time-zero
//!   public graph. This is what the rung's anomaly holds for a site (`noticer::attach_target`: an
//!   observation at the site, or at a dependent within the site's burst; a service the public
//!   graph does not connect to the site starts an anomaly of its own).
//! - **all**: every abnormal observation of the incident, a partner's and a peer's included.
//!
//! "Abnormal" is the rung's own predicate (`rung::is_abnormal`): a counter at or above `HIGH`, a
//! catalogue message other than `CheckHealth`, a snapshot whose hash differs from the public
//! graph's.

use gordian_core::Instant;
use gordian_stream::oracle::{ObsLabel, StreamTruth};
use gordian_stream::{ObsId, Stream};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{
    CATALOGUE_LIMIT, HIGH, SignalText, consistent_hypotheses, signature,
};
use gordian_world::{Observation, Service, ServiceId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

const S: u64 = 1_000_000_000;
/// The cutoffs, seconds after the incident's first abnormal observation.
pub const CUTS_S: [u64; 12] = [1, 2, 3, 4, 6, 8, 10, 13, 16, 20, 24, 32];
/// E1's settle time: the key is read this long after the gate opens.
const SETTLE_NS: u64 = S;

pub const ROWS_HEADER: &str = "seed,incident,cut,cut_ns,site_tags,anchor,bands,dep_tags,oth_tags,oth_n,partner,verdict_att,verdict_all,streak,ff_own,n_abn,gate_delay_ns\n";
pub const INC_HEADER: &str = "seed,incident,t0_ns,first_site_alarm_ns,gate_att_ns,gate_all_ns,n_abn,new_down_site,new_down_other\n";
pub const ALARMS_HEADER: &str = "seed,obs,at_ns,service,owner,role\n";
pub const GRAPH_HEADER: &str = "seed,service,depends_on\n";

/// E1's band of an abnormal counter reading (`noticer_record::band_of`).
fn band_of(value: u64) -> u64 {
    if value < 2 * HIGH {
        0
    } else if value < 4 * HIGH {
        1
    } else {
        2
    }
}

fn is_abnormal(obs: &Observation, services: &[Service]) -> bool {
    match obs {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => {
            matches!(SignalText::from_text_id(*text_id), Some(t) if t != SignalText::CheckHealth)
        }
        Observation::Snapshot {
            service,
            config_hash,
        } => services
            .get(service.index())
            .is_some_and(|s| s.config_hash != *config_hash),
        Observation::Probed { .. } | Observation::Correction { .. } => false,
    }
}

fn service_of(obs: &Observation) -> Option<ServiceId> {
    match obs {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => Some(*service),
        _ => None,
    }
}

fn tags_of(at: u64, obs: &Observation) -> Vec<String> {
    let mut t: Vec<String> = signature(&[(Instant(at), obs.clone())])
        .iter()
        .map(|x| format!("{x:?}"))
        .collect();
    if matches!(obs, Observation::Snapshot { .. }) {
        t.push("Snapshot".to_string());
    }
    t
}

fn counter_name(obs: &Observation) -> Option<String> {
    match obs {
        Observation::Counter { name, .. } => Some(format!("{name:?}")),
        _ => None,
    }
}

/// The verdict of the public checker on `ev`: `C` for no consistent hypothesis, else the sorted
/// distinct kinds left open (`-` for "no fault").
fn verdict(
    public: &gordian_world::episode::PublicInfo,
    ev: &[(Instant, Observation)],
) -> String {
    let open = consistent_hypotheses(public, ev);
    if open.is_empty() {
        return "C".to_string();
    }
    let kinds: BTreeSet<String> = open
        .iter()
        .map(|h| h.map_or("-".to_string(), |(k, _)| format!("{k:?}")))
        .collect();
    kinds.into_iter().collect::<Vec<_>>().join("|")
}

fn join(s: &BTreeSet<String>) -> String {
    s.iter().cloned().collect::<Vec<_>>().join("+")
}

/// One abnormal observation of an incident.
struct Abn {
    at: u64,
    obs: Observation,
    service: ServiceId,
}

/// First prefix of `abn` (in order) whose evidence the checker finds contradictory, as the instant
/// of the observation that completed it.
fn gate_of(
    public: &gordian_world::episode::PublicInfo,
    abn: &[&Abn],
) -> Option<u64> {
    let mut ev: Vec<(Instant, Observation)> = Vec::new();
    for a in abn {
        ev.push((Instant(a.at), a.obs.clone()));
        if consistent_hypotheses(public, &ev).is_empty() {
            return Some(a.at);
        }
    }
    None
}

/// The services newly downstream of `site` at `t` compared with the time-zero graph.
fn newly_downstream(old: &[Service], new: &[Service], site: ServiceId) -> Vec<u32> {
    let before = dependents_mask(old, site);
    let after = dependents_mask(new, site);
    before
        .iter()
        .zip(after.iter())
        .enumerate()
        .filter(|(_, (b, a))| **a && !**b)
        .map(|(i, _)| i as u32)
        .collect()
}

fn list(v: &[u32]) -> String {
    v.iter().map(u32::to_string).collect::<Vec<_>>().join("|")
}

#[allow(clippy::too_many_arguments)]
pub fn emit(
    seed: u64,
    stream: &Stream,
    truth: &StreamTruth,
    graph_at: &dyn Fn(u64) -> Vec<Service>,
    rows: &mut String,
    inc_rows: &mut String,
    alarms: Option<(&mut String, &mut String)>,
) {
    let events = stream.events();
    let services = &truth.services;
    let public = stream.public_info().world_public_info();

    if let Some((alarms_csv, graph_csv)) = alarms {
        for s in services {
            let deps: Vec<u32> = s.depends_on.iter().map(|d| d.0).collect();
            writeln!(graph_csv, "{seed},{},{}", s.id.0, list(&deps)).unwrap();
        }
        for (i, (at, ob)) in events.iter().enumerate() {
            if !is_abnormal(ob, services) {
                continue;
            }
            let Some(svc) = service_of(ob) else { continue };
            let (owner, role) = match truth.labels[i] {
                ObsLabel::Incident { id, role } => (id as i64, format!("{role:?}")),
                ObsLabel::Background(k) => (-1, format!("bg:{k:?}")),
            };
            writeln!(alarms_csv, "{seed},{i},{},{},{owner},{role}", at.0, svc.0).unwrap();
        }
    }

    for inc in &truth.incidents {
        let site = inc.occupies[0];
        let region = dependents_mask(services, site);
        let site_region_dependent = |s: ServiceId| region[s.index()];
        let own: Vec<(u64, &Observation)> = inc
            .observations
            .iter()
            .map(|o: &ObsId| {
                let (at, ob) = &events[o.0 as usize];
                (at.0, ob)
            })
            .collect();
        let abn: Vec<Abn> = own
            .iter()
            .filter(|(_, ob)| is_abnormal(ob, services))
            .filter_map(|(at, ob)| {
                service_of(ob).map(|svc| Abn {
                    at: *at,
                    obs: (*ob).clone(),
                    service: svc,
                })
            })
            .collect();
        let Some(first) = abn.first() else {
            writeln!(inc_rows, "{seed},{},,,,,0,,", inc.id).unwrap();
            continue;
        };
        let t0 = first.at;
        let attached: Vec<&Abn> = abn
            .iter()
            .filter(|a| a.service == site || site_region_dependent(a.service))
            .collect();
        let all: Vec<&Abn> = abn.iter().collect();
        let gate_att = gate_of(&public, &attached);
        let gate_all = gate_of(&public, &all);
        let first_site = abn.iter().find(|a| a.service == site).map(|a| a.at);

        // Services newly downstream of the site (and of the cascade's partner) when the incident
        // began, on the graph then in force.
        let g = graph_at(inc.onset_ns);
        let nd_site = newly_downstream(services, &g, site);
        let nd_other = inc
            .shape
            .other
            .filter(|_| inc.shape.hard_kind == Some(gordian_stream::HardKind::Cascade))
            .map(|p| newly_downstream(services, &g, p))
            .unwrap_or_default();
        writeln!(
            inc_rows,
            "{seed},{},{t0},{},{},{},{},{},{}",
            inc.id,
            first_site.map_or(String::new(), |v| v.to_string()),
            gate_att.map_or(String::new(), |v| (v - t0).to_string()),
            gate_all.map_or(String::new(), |v| (v - t0).to_string()),
            abn.len(),
            list(&nd_site),
            list(&nd_other),
        )
        .unwrap();
        // Every tier is described the same way; the tier is joined from incidents.csv.
        let mut cuts: Vec<(String, u64)> = CUTS_S
            .iter()
            .map(|c| (c.to_string(), c * S))
            .collect();
        if let Some(g) = gate_att {
            cuts.push(("snap".to_string(), g - t0 + SETTLE_NS));
        }
        for (label, cut_ns) in cuts {
            let end = t0 + cut_ns;
            let abn_c: Vec<&Abn> = abn.iter().filter(|a| a.at <= end).collect();
            let att_c: Vec<&Abn> = attached.iter().copied().filter(|a| a.at <= end).collect();

            // E1's key features, from the attached evidence at the site.
            let mut site_tags: BTreeSet<String> = BTreeSet::new();
            let mut bands: BTreeMap<String, u64> = BTreeMap::new();
            for a in att_c.iter().filter(|a| a.service == site) {
                site_tags.extend(tags_of(a.at, &a.obs));
                if let (Some(n), Observation::Counter { value, .. }) =
                    (counter_name(&a.obs), &a.obs)
                {
                    let b = bands.entry(n).or_insert(0);
                    *b = (*b).max(band_of(*value));
                }
            }
            let anchor = att_c
                .first()
                .filter(|a| a.service == site)
                .map(|a| join(&tags_of(a.at, &a.obs).into_iter().collect()))
                .unwrap_or_default();
            let bands_s = bands
                .iter()
                .map(|(n, b)| format!("{n}:{b}"))
                .collect::<Vec<_>>()
                .join("+");

            let mut dep_tags: BTreeSet<String> = BTreeSet::new();
            let mut oth_tags: BTreeSet<String> = BTreeSet::new();
            let mut oth_svcs: BTreeSet<u32> = BTreeSet::new();
            for a in &abn_c {
                if a.service == site {
                    continue;
                }
                if site_region_dependent(a.service) {
                    dep_tags.extend(tags_of(a.at, &a.obs));
                } else {
                    oth_tags.extend(tags_of(a.at, &a.obs));
                    oth_svcs.insert(a.service.0);
                }
            }

            // A1c's partner: the unconnected service (neither depends on the other in the public
            // graph) whose first alarm is nearest to the site's own first alarm, within 10 s.
            let site_first_c = abn_c.iter().find(|a| a.service == site).map(|a| a.at);
            let mut partner = String::new();
            if let Some(sf) = site_first_c {
                let mut best: Option<(u64, u32, i64)> = None;
                for p in &oth_svcs {
                    let pid = ServiceId(*p);
                    let unconnected = !dependents_mask(services, pid)[site.index()];
                    if !unconnected {
                        continue;
                    }
                    let pf = abn_c.iter().find(|a| a.service == pid).map(|a| a.at).unwrap();
                    let gap = pf as i64 - sf as i64;
                    if gap.unsigned_abs() > 10 * S {
                        continue;
                    }
                    if best.is_none_or(|(g, _, _)| gap.unsigned_abs() < g) {
                        best = Some((gap.unsigned_abs(), *p, gap));
                    }
                }
                if let Some((g, p, gap)) = best {
                    let mut ptags: BTreeSet<String> = BTreeSet::new();
                    for a in abn_c.iter().filter(|a| a.service.0 == p) {
                        ptags.extend(tags_of(a.at, &a.obs));
                    }
                    let rel = if gap < 0 {
                        "before"
                    } else if gap == 0 {
                        "same"
                    } else {
                        "after"
                    };
                    let band = if g < 400_000_000 {
                        0
                    } else if g < 2 * S {
                        1
                    } else {
                        2
                    };
                    partner = format!("{rel}:{band}:{}", join(&ptags));
                }
            }

            let ev_att: Vec<(Instant, Observation)> = att_c
                .iter()
                .map(|a| (Instant(a.at), a.obs.clone()))
                .collect();
            let ev_all: Vec<(Instant, Observation)> = abn_c
                .iter()
                .map(|a| (Instant(a.at), a.obs.clone()))
                .collect();
            let verdict_att = verdict(&public, &ev_att);
            let verdict_all = verdict(&public, &ev_all);

            // Benign streak at the site's counters (the decoy rule: five in a row).
            let mut streak = 0u64;
            for (at, ob) in &own {
                if *at > end {
                    break;
                }
                if let Observation::Counter {
                    service, value, ..
                } = ob
                    && *service == site
                {
                    if *value < HIGH {
                        streak += 1;
                    } else {
                        streak = 0;
                    }
                }
            }
            let streak = streak.min(5);
            let ff_own = own
                .iter()
                .filter(|(at, ob)| {
                    *at <= end
                        && matches!(ob, Observation::Message { text_id, .. } if *text_id >= CATALOGUE_LIMIT)
                })
                .count();
            let gate_delay = if label == "snap" {
                (cut_ns - SETTLE_NS).to_string()
            } else {
                String::new()
            };
            writeln!(
                rows,
                "{seed},{},{label},{cut_ns},{},{anchor},{bands_s},{},{},{},{partner},{verdict_att},{verdict_all},{streak},{ff_own},{},{gate_delay}",
                inc.id,
                join(&site_tags),
                join(&dep_tags),
                join(&oth_tags),
                oth_svcs.len().min(3),
                abn_c.len(),
            )
            .unwrap();
        }
    }
}
