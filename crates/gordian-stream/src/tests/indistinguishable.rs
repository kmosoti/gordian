//! A decoy is indistinguishable from a hard incident in the first moments, by every statistic
//! tried, and the construction that makes it so is checked directly.
//!
//! Three checks, from the strongest and most specific to the most general:
//!
//! 1. *Construction.* Regenerate the same seed with one incident's tier swapped after every draw
//!    is made. Everything public in its first `T0` must be byte-identical, including what a probe
//!    returns. This is the first world's kind-swap test (review log, A1), which found a real
//!    leak there.
//! 2. *Statistics.* Plug-in mutual information between the tier and each of fifteen views of the
//!    incident's own first-`T0` evidence, against 100 permutations of the tier, as the
//!    coordinator's earlier check did. The evidence is the incident's own, perfectly segmented
//!    from noise and from other incidents, which is the best case for a policy.
//! 3. *Power.* A view of what happens after `T0` must separate the tiers, or the test above
//!    could not have failed.

use super::*;
use crate::kinds::Tier;
use crate::oracle::IncidentTruth;
use crate::rng::Gen;
use crate::stream::{Forced, generate_forced};
use crate::timing::T0_NS;
use gordian_core::Instant;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{HIGH, SignalText, consistent_hypotheses, signature};
use gordian_world::{CounterName, FaultKind, Observation, Probe, ProbeKind, ServiceId};
use std::collections::BTreeMap;

fn mix_params(seed: u64) -> StreamParams {
    // Half hard, half decoy, no plain, no recurrence: every incident is one of the two tiers
    // under comparison, and no two are copies of each other.
    let mut p = StreamParams::new(seed);
    p.mix.plain_permille = 0;
    p.mix.hard_permille = 500;
    p.recurrence_permille = 0;
    p
}

fn window(s: &crate::Stream, from: u64, to: u64) -> Vec<(Instant, Observation)> {
    s.events()
        .iter()
        .filter(|(at, _)| at.0 >= from && at.0 < to)
        .cloned()
        .collect()
}

fn probe_vector(
    s: &crate::Stream,
    inc: &IncidentTruth,
    at: Instant,
) -> Vec<gordian_world::ProbeResult> {
    let hidden = &s.incidents[inc.id as usize];
    let mut out = Vec::new();
    for who in &inc.occupies {
        for kind in ProbeKind::ALL {
            out.push(crate::probe::answer(
                hidden,
                &s.services,
                Probe { kind, target: *who },
                at,
            ));
        }
    }
    out
}

#[test]
fn swapping_hard_for_decoy_and_back_changes_nothing_public_for_the_first_six_seconds() {
    let (mut swapped, mut later_differs) = (0, 0);
    for seed in 0..40 {
        let p = mix_params(seed);
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            let to = match inc.tier {
                Tier::Hard => Tier::Decoy,
                Tier::Decoy => Tier::Hard,
                Tier::Plain => unreachable!(),
            };
            let forced = generate_forced(
                &p,
                Forced {
                    arrival: inc.arrival,
                    tier: Some(to),
                    duo_kind: None,
                },
            );
            let ft = crate::oracle::reveal(&forced);
            let finc = ft
                .incidents
                .iter()
                .find(|i| i.arrival == inc.arrival)
                .expect("the swapped incident exists");
            assert_eq!(finc.tier, to);
            assert_eq!(
                finc.shape, inc.shape,
                "the family must not depend on the tier"
            );
            assert_eq!(finc.occupies, inc.occupies);
            assert_eq!(finc.onset_ns, inc.onset_ns);

            let end = inc.onset_ns + T0_NS;
            // Everything public before the end of phase 1: all incidents, all noise.
            assert_eq!(
                window(&s, 0, end),
                window(&forced, 0, end),
                "seed {seed} incident {}: the public stream before onset + T0 depends on the tier",
                inc.id
            );
            // Probes: every kind at every service the incident occupies, at the last instant of
            // phase 1 and at the first.
            for at in [Instant(inc.onset_ns + 1), Instant(end - 1)] {
                assert_eq!(
                    probe_vector(&s, inc, at),
                    probe_vector(&forced, finc, at),
                    "seed {seed} incident {}: probes differ at {at:?}",
                    inc.id
                );
            }
            swapped += 1;
            // Power: afterwards they do differ.
            let w = |st: &crate::Stream| window(st, end, inc.onset_ns + 30_000_000_000);
            if w(&s) != w(&forced) {
                later_differs += 1;
            }
        }
    }
    println!("{swapped} incidents swapped between hard and decoy");
    assert!(swapped > 300, "{swapped}");
    assert_eq!(
        later_differs, swapped,
        "after T0 the tiers must be separable"
    );
}

#[test]
fn the_two_kinds_a_duo_leaves_open_are_the_same_in_the_whole_public_stream() {
    let mut p = with_mix(3, 1000, 0);
    p.regimes.clear();
    p.recurrence_permille = 0;
    let (mut n, mut probe_differs) = (0, 0);
    for seed in 0..30 {
        p.seed = seed;
        let (s, t) = with_truth(&p);
        for inc in t.incidents.iter().filter(|i| i.shape.duo) {
            let truth_kind = inc.shape.known_kind.unwrap();
            let other = if truth_kind == FaultKind::ResourceExhausted {
                FaultKind::DependencyDown
            } else {
                FaultKind::ResourceExhausted
            };
            let forced = generate_forced(
                &p,
                Forced {
                    arrival: inc.arrival,
                    tier: None,
                    duo_kind: Some(other),
                },
            );
            let ft = crate::oracle::reveal(&forced);
            let finc = ft
                .incidents
                .iter()
                .find(|i| i.arrival == inc.arrival)
                .unwrap();
            assert_eq!(finc.shape.known_kind, Some(other));
            assert_eq!(
                s.events(),
                forced.events(),
                "the stream reveals which kind it is"
            );
            n += 1;
            // The probe is where they part.
            let at = Instant(inc.onset_ns + 1_000_000_000);
            if probe_vector(&s, inc, at) != probe_vector(&forced, finc, at) {
                probe_differs += 1;
            }
        }
    }
    assert!(n > 40, "{n}");
    assert_eq!(probe_differs, n);
}

// ---- Mutual information

fn mutual_information(xs: &[usize], ys: &[bool]) -> f64 {
    let n = xs.len() as f64;
    let mut joint: BTreeMap<(usize, bool), f64> = BTreeMap::new();
    let mut px: BTreeMap<usize, f64> = BTreeMap::new();
    let mut py = [0.0f64; 2];
    for (x, y) in xs.iter().zip(ys) {
        *joint.entry((*x, *y)).or_insert(0.0) += 1.0;
        *px.entry(*x).or_insert(0.0) += 1.0;
        py[*y as usize] += 1.0;
    }
    joint
        .iter()
        .map(|((x, y), c)| {
            let pxy = c / n;
            pxy * (pxy / ((px[x] / n) * (py[*y as usize] / n))).ln()
        })
        .sum()
}

fn permuted(ys: &[bool], g: &mut Gen) -> Vec<bool> {
    let mut v = ys.to_vec();
    for i in (1..v.len()).rev() {
        v.swap(i, g.below(i as u64 + 1) as usize);
    }
    v
}

/// A view is a function of the evidence to a label; labels are interned to small integers.
struct Views {
    names: Vec<&'static str>,
    columns: Vec<Vec<String>>,
}

impl Views {
    fn new() -> Self {
        Self {
            names: Vec::new(),
            columns: Vec::new(),
        }
    }
    fn push(&mut self, name: &'static str, row: usize, value: String) {
        let idx = match self.names.iter().position(|n| *n == name) {
            Some(i) => i,
            None => {
                self.names.push(name);
                self.columns.push(Vec::new());
                self.names.len() - 1
            }
        };
        let col = &mut self.columns[idx];
        assert_eq!(
            col.len(),
            row,
            "views must be pushed once per incident, in order"
        );
        col.push(value);
    }
}

fn intern(col: &[String]) -> Vec<usize> {
    let mut ids: BTreeMap<&String, usize> = BTreeMap::new();
    col.iter()
        .map(|v| {
            let next = ids.len();
            *ids.entry(v).or_insert(next)
        })
        .collect()
}

fn is_abnormal(o: &Observation) -> bool {
    match o {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => {
            matches!(SignalText::from_text_id(*text_id), Some(t) if t != SignalText::CheckHealth)
        }
        _ => false,
    }
}

fn service_of(o: &Observation) -> ServiceId {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => *service,
        Observation::Probed { probe, .. } => probe.target,
        Observation::Correction { site, .. } => *site,
    }
}

#[test]
fn no_public_statistic_of_the_first_six_seconds_separates_a_hard_incident_from_a_decoy() {
    let mut views = Views::new();
    let mut tiers: Vec<bool> = Vec::new(); // true = hard
    let mut control: Vec<String> = Vec::new();
    let mut row = 0usize;
    for seed in 0..150 {
        let p = mix_params(seed);
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        for inc in &t.incidents {
            let site = inc.occupies[0];
            let phase1_end = inc.onset_ns + T0_NS;
            let ev: Vec<(Instant, Observation)> = inc
                .observations
                .iter()
                .map(|o| s.events()[o.0 as usize].clone())
                .filter(|(at, _)| at.0 < phase1_end)
                .collect();
            let deps = dependents_mask(&t.services, site);
            let at_site: Vec<&(Instant, Observation)> = ev
                .iter()
                .filter(
                    |(_, o)| matches!(o, Observation::Counter { service, .. } if *service == site),
                )
                .collect();
            let series: Vec<&(Instant, Observation)> = at_site
                .iter()
                .copied()
                .filter(|(at, o)| {
                    at.0 > inc.onset_ns + 300_000_000
                        && matches!(
                            o,
                            Observation::Counter {
                                name: CounterName::ErrorRate | CounterName::Saturation,
                                ..
                            }
                        )
                })
                .collect();
            let open = consistent_hypotheses(&public, &ev);

            views.push("signature", row, format!("{:?}", signature(&ev)));
            views.push("observations", row, ev.len().to_string());
            views.push(
                "abnormal observations",
                row,
                ev.iter()
                    .filter(|(_, o)| is_abnormal(o))
                    .count()
                    .to_string(),
            );
            views.push("readings at the site", row, at_site.len().to_string());
            views.push(
                "flap pattern of the first four beats",
                row,
                series
                    .iter()
                    .take(4)
                    .map(|(_, o)| match o {
                        Observation::Counter { value, .. } if *value >= HIGH => 'H',
                        _ => 'b',
                    })
                    .collect(),
            );
            let services: std::collections::BTreeSet<ServiceId> =
                ev.iter().map(|(_, o)| service_of(o)).collect();
            views.push("distinct services", row, services.len().to_string());
            views.push(
                "services outside the site and its dependents",
                row,
                services
                    .iter()
                    .filter(|sv| **sv != site && !deps[sv.index()])
                    .count()
                    .to_string(),
            );
            let mut sev: Vec<String> = ev
                .iter()
                .filter_map(|(_, o)| match o {
                    Observation::Message { severity, .. } => Some(format!("{severity:?}")),
                    _ => None,
                })
                .collect();
            sev.sort();
            views.push("message severities", row, sev.join(","));
            views.push(
                "checker: size, empty, silent",
                row,
                format!(
                    "{} {} {}",
                    open.len(),
                    open.is_empty(),
                    open.contains(&None)
                ),
            );
            let mut kinds: Vec<String> = open
                .iter()
                .flatten()
                .map(|(k, _)| format!("{k:?}"))
                .collect();
            kinds.sort();
            kinds.dedup();
            views.push("checker: kinds left open", row, kinds.join(","));
            views.push(
                "gap before the first beat, 100 ms buckets",
                row,
                series.first().map_or("none".to_string(), |(at, _)| {
                    ((at.0 - inc.onset_ns) / 100_000_000).to_string()
                }),
            );
            views.push(
                "site: resource and dependents",
                row,
                format!(
                    "{:?} {}",
                    t.services[site.index()].resource,
                    deps.iter().filter(|d| **d).count()
                ),
            );
            // A configuration hash is unique to its service; what a policy can learn from it is
            // whether it equals the public one.
            let mut public_hash: Vec<u64> = Vec::new();
            for sv in &inc.occupies {
                for _ in ProbeKind::ALL {
                    public_hash.push(t.services[sv.index()].config_hash);
                }
            }
            let probes: Vec<String> = probe_vector(&s, inc, Instant(phase1_end - 1))
                .iter()
                .zip(&public_hash)
                .map(|(r, h)| match r {
                    gordian_world::ProbeResult::ConfigHash(x) => {
                        format!("hash changed: {}", x != h)
                    }
                    other => format!("{other:?}"),
                })
                .collect();
            views.push("probe results at the end of phase 1", row, probes.join(" "));
            views.push(
                "last saturation reading, buckets of 10",
                row,
                at_site
                    .iter()
                    .rev()
                    .find_map(|(_, o)| match o {
                        Observation::Counter {
                            name: CounterName::Saturation,
                            value,
                            ..
                        } => Some((value / 10).to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "none".into()),
            );
            views.push(
                "highest reading at the site, buckets of 10",
                row,
                at_site
                    .iter()
                    .filter_map(|(_, o)| match o {
                        Observation::Counter { value, .. } => Some(value / 10),
                        _ => None,
                    })
                    .max()
                    .map_or("none".into(), |v| v.to_string()),
            );
            tiers.push(inc.tier == Tier::Hard);

            // The control: what happens at the site after phase 1. A decoy that has resolved
            // reads benign every time; a hard incident flaps one beat in four.
            let benign_after = inc
                .observations
                .iter()
                .filter(|o| {
                    let (at, ob) = &s.events()[o.0 as usize];
                    at.0 >= phase1_end
                        && at.0 < inc.onset_ns + 30_000_000_000
                        && matches!(ob, Observation::Counter { service, value, .. } if *service == site && *value < HIGH)
                })
                .count();
            control.push(benign_after.to_string());
            row += 1;
        }
    }
    let hard = tiers.iter().filter(|h| **h).count();
    let decoy = tiers.len() - hard;
    println!(
        "{hard} hard incidents, {decoy} decoys, {} views",
        views.names.len()
    );
    assert!(hard > 800 && decoy > 800);
    assert!(views.names.len() >= 15);

    let mut g = Gen::keyed(&[0xA11CE]);
    let mut worst: (f64, &str) = (0.0, "");
    for (name, col) in views.names.iter().zip(&views.columns) {
        let xs = intern(col);
        let mi = mutual_information(&xs, &tiers);
        let base: Vec<f64> = (0..100)
            .map(|_| mutual_information(&xs, &permuted(&tiers, &mut g)))
            .collect();
        let max = base.iter().cloned().fold(0.0, f64::max);
        let mean = base.iter().sum::<f64>() / base.len() as f64;
        println!(
            "  {name}: {} values, MI {mi:.5}, permutation mean {mean:.5}, max {max:.5}",
            xs.iter().max().unwrap() + 1
        );
        if mi / max.max(1e-12) > worst.0 {
            worst = (mi / max.max(1e-12), name);
        }
        assert!(
            mi <= max,
            "view \"{name}\" separates the tiers: MI {mi} against {max}"
        );
    }
    println!(
        "closest to its baseline's maximum: \"{}\" at {:.2}x",
        worst.1, worst.0
    );

    // Power: the control separates them far beyond its baseline.
    let xs = intern(&control);
    let mi = mutual_information(&xs, &tiers);
    let max = (0..100)
        .map(|_| mutual_information(&xs, &permuted(&tiers, &mut g)))
        .fold(0.0, f64::max);
    println!("control (benign readings after phase 1): MI {mi:.5}, baseline max {max:.5}");
    assert!(
        mi > 5.0 * max,
        "the test cannot see a real difference: {mi} vs {max}"
    );
}
