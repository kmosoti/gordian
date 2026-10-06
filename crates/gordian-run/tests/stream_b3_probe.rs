//! A diagnostic of B3 (ignored; run by hand): replay generated streams through a noticer in the
//! harness's 0.5 s steps, emulating the rung's retirement of quiet noticed anomalies, and print, for
//! each hard non-leak incident that no notice anchors correctly, the anomaly that first held each of
//! its first observations and the evidence of that anomaly. The emulation reproduces the harness's
//! notice record for `never_escalate` (checked by hand on the tuning streams: the same ten incidents
//! are unanchored under the re-anchor as in the run's `notice_incidents.csv`).
//!
//! It reads the evaluator's truth (which observations belong to which incident) to label what it
//! prints: it is an analysis tool, not an arm, and nothing it prints reaches a noticer.
//!
//! ```text
//! B3_SEEDS=10000-10099 B3_SPEC=reanchor|split|rung|rung2 [B3_GAP=2 B3_MB=2] [B3_TRACE=513.5-516.5] \
//!   cargo test -p gordian-run --test stream_b3_probe -- --ignored --nocapture
//! ```

use gordian_run::stream::arms::noticer::{self, BaseSpec, NoticerSpec};
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_stream::{HardKind, ObsId, StreamParams, Tier, generate};
use gordian_stream_eval::truth_from_stream;
use std::collections::BTreeMap;

fn spec_of(name: &str) -> NoticerSpec {
    match name {
        "rung" => NoticerSpec::Rung { notice_z: None },
        "rung2" => NoticerSpec::Rung {
            notice_z: Some(2.0),
        },
        "split" => {
            let gap: f64 = std::env::var("B3_GAP")
                .unwrap_or("2".into())
                .parse()
                .unwrap();
            let mb: u32 = std::env::var("B3_MB")
                .unwrap_or("2".into())
                .parse()
                .unwrap();
            NoticerSpec::Composed {
                base: BaseSpec::Reanchor {
                    notice_z: Some(2.0),
                    gap_ns: 20_000_000,
                    min_burst: 2,
                    isolation: Isolation::Site,
                },
                ramp: None,
                split: Some(gordian_run::stream::arms::noticer_split::SplitSpec {
                    gap_ns: (gap * 1e9) as u64,
                    min_burst: mb,
                }),
            }
        }
        _ => NoticerSpec::Reanchor {
            notice_z: Some(2.0),
            gap_ns: 20_000_000,
            min_burst: 2,
            isolation: Isolation::Site,
        },
    }
}

#[test]
#[ignore]
fn probe() {
    let seeds = std::env::var("B3_SEEDS").unwrap_or("10000-10003".into());
    let (a, b) = seeds.split_once('-').unwrap();
    let (a, b): (u64, u64) = (a.parse().unwrap(), b.parse().unwrap());
    let spec = spec_of(&std::env::var("B3_SPEC").unwrap_or("reanchor".into()));
    let cfg = RungConfig::default();
    for seed in a..=b {
        let stream = generate(&StreamParams::new(seed));
        let truth = truth_from_stream(&stream);
        let public = stream.public_info();
        let mut n = noticer::build(&spec, &cfg, &public.services);
        let events = stream.events().to_vec();
        let mut held_all: Vec<Held> = Vec::new();
        let mut next = 0usize;
        // first owner of each observation: (anomaly id, site, anchor_at ms, noticed_at?)
        let mut owner: BTreeMap<u32, (u32, u32, u64)> = BTreeMap::new();
        let mut notices: Vec<(u32, ObsId, u64, u32, u64)> = Vec::new(); // anomaly, anchor, anchor_at, site, at
        let mut snaps: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        let mut now = 0u64;
        while now <= truth.duration_ns + 20_000_000_000 {
            now += 500_000_000;
            while next < events.len() && events[next].0.0 <= now {
                let (at, obs) = &events[next];
                let abnormal = is_abnormal(obs, &public.services);
                let held = Held {
                    id: ObsId(next as u32),
                    at: *at,
                    obs: obs.clone(),
                    abnormal,
                };
                if abnormal {
                    n.observe(&held);
                }
                held_all.push(held);
                next += 1;
            }
            let from = now.saturating_sub(125_000_000_000);
            let start = held_all.partition_point(|h| h.at.0 < from);
            let store = Store::with(held_all[start..].iter().cloned());
            for x in n.notice(gordian_core::Instant(now), &store) {
                notices.push((x.id, x.anchor, x.anchor_at.0, x.site.0, now));
            }
            for a in n.anomalies() {
                for (t, o, _) in &a.attached {
                    owner.entry(o.0).or_insert((a.id, a.site.0, a.anchor_at.0));
                    let _ = t;
                }
            }
            for inc in &truth.incidents {
                if inc.tier != Tier::Hard
                    || inc.shape.hard_kind == Some(HardKind::SlowLeak)
                    || snaps.contains_key(&inc.id)
                {
                    continue;
                }
                let first = inc.observations[0];
                let first_at = events[first.0 as usize].0.0;
                if now < first_at + 1_000_000_000 {
                    continue;
                }
                let mut v = Vec::new();
                for a in n.anomalies() {
                    if a.attached.iter().any(|(_, o, _)| *o == first) {
                        v.push(format!("  anomaly {} site {} anchor {:.2}s noticed {:?}: attached (t, svc, incident): {}", a.id, a.site.0, a.anchor_at.0 as f64 / 1e9, a.noticed_at.map(|t| t.0 as f64 / 1e9),
                            a.attached.iter().map(|(t, o, s)| format!("{:.2}/s{}/{}", t.0 as f64 / 1e9, s.0, truth.incident_of(*o).map_or("bg".to_string(), |i| i.to_string()))).collect::<Vec<_>>().join(" ")));
                    }
                }
                snaps.insert(inc.id, v);
            }
            if let Ok(w) = std::env::var("B3_TRACE") {
                let (t0, t1) = w.split_once('-').unwrap();
                let (t0, t1): (f64, f64) = (t0.parse().unwrap(), t1.parse().unwrap());
                let t = now as f64 / 1e9;
                if t >= t0 && t <= t1 {
                    println!("-- step {t:.1}s");
                    for a in n.anomalies() {
                        println!(
                            "   anomaly {} site {} anchor {:.2}s noticed {:?} attached {}",
                            a.id,
                            a.site.0,
                            a.anchor_at.0 as f64 / 1e9,
                            a.noticed_at.map(|t| t.0 as f64 / 1e9),
                            a.attached
                                .iter()
                                .map(|(t, o, s)| format!(
                                    "{:.2}/s{}/{}",
                                    t.0 as f64 / 1e9,
                                    s.0,
                                    truth
                                        .incident_of(*o)
                                        .map_or("bg".to_string(), |i| i.to_string())
                                ))
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                    }
                }
            }
            for id in n.retirable(gordian_core::Instant(now)) {
                n.retire(id);
            }
        }
        for inc in &truth.incidents {
            if inc.tier != Tier::Hard || inc.shape.hard_kind == Some(HardKind::SlowLeak) {
                continue;
            }
            let first = inc.observations[0];
            let first_at = events[first.0 as usize].0.0;
            let correct = notices.iter().any(|(_, anchor, anchor_at, _, _)| {
                truth.incident_of(*anchor) == Some(inc.id)
                    && anchor_at.abs_diff(first_at) <= 1_000_000_000
            });
            if correct {
                continue;
            }
            let noticed = notices
                .iter()
                .any(|(_, anchor, _, _, _)| truth.incident_of(*anchor) == Some(inc.id));
            println!(
                "seed {seed} incident {} {:?} first {:.2}s site {:?} occupies {:?} noticed {noticed} onset {:.2}s",
                inc.id,
                inc.shape.hard_kind,
                first_at as f64 / 1e9,
                inc.occupies.first(),
                inc.occupies.iter().map(|s| s.0).collect::<Vec<_>>(),
                inc.onset_ns as f64 / 1e9
            );
            if let Some(v) = snaps.get(&inc.id) {
                for l in v {
                    println!("{l}");
                }
            }
            for o in inc.observations.iter().take(8) {
                let (at, ob) = &events[o.0 as usize];
                let own = owner.get(&o.0);
                let ownlabel = own.map(|(id, site, anchor_at)| {
                    format!(
                        "anomaly {id} site {site} anchor {:.2}s; its notice at {:?}",
                        *anchor_at as f64 / 1e9,
                        notices
                            .iter()
                            .find(|x| x.0 == *id)
                            .map(|x| (x.4 as f64 / 1e9, truth.incident_of(x.1)))
                    )
                });
                let abn = is_abnormal(ob, &public.services);
                println!(
                    "    {:7.2}s abn={abn} {} -> {}",
                    at.0 as f64 / 1e9,
                    short(ob),
                    ownlabel.unwrap_or("(not attached)".into())
                );
            }
        }
    }
}

fn short(o: &gordian_world::Observation) -> String {
    use gordian_world::Observation::*;
    match o {
        Counter {
            service,
            name,
            value,
        } => format!("s{} {:?}={}", service.0, name, value),
        Message {
            service, text_id, ..
        } => format!("s{} msg {}", service.0, text_id),
        Snapshot { service, .. } => format!("s{} snapshot", service.0),
        _ => "other".into(),
    }
}

/// What the ramp detector does on whole streams, counted: counter readings fed, chain comparisons
/// made, the most chains alive at once, over the seeds of `B3_SEEDS`, with the ramp parameters of
/// `B3_RAMP` (`gap_ms,max_step,max_drop,min_readings,min_rise`). The noticers' own operations are
/// not billed by the harness (the rung's noticing is not either), so this is the only count of them.
///
/// ```text
/// B3_SEEDS=20000-20199 B3_RAMP=2000,10,2,4,10 \
///   cargo test --release -p gordian-run --test stream_b3_probe -- --ignored --nocapture ramp_operations
/// ```
#[test]
#[ignore]
fn ramp_operations() {
    use gordian_run::stream::arms::noticer_ramp::{RampDetector, RampSpec};
    let seeds = std::env::var("B3_SEEDS").unwrap_or("10000-10009".into());
    let (a, b) = seeds.split_once('-').unwrap();
    let (a, b): (u64, u64) = (a.parse().unwrap(), b.parse().unwrap());
    let p: Vec<u64> = std::env::var("B3_RAMP")
        .unwrap_or("2000,10,2,4,10".into())
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let spec = RampSpec {
        gap_ns: p[0] * 1_000_000,
        max_step: p[1] as u32,
        max_drop: p[2] as u32,
        min_readings: p[3] as u32,
        min_rise: p[4] as u32,
    };
    let (mut readings, mut comparisons, mut observations, mut streams, mut peak) =
        (0u64, 0u64, 0u64, 0u64, 0usize);
    for seed in a..=b {
        let stream = generate(&StreamParams::new(seed));
        let mut det = RampDetector::new(spec);
        for (i, (at, obs)) in stream.events().iter().enumerate() {
            let held = Held {
                id: ObsId(i as u32),
                at: *at,
                obs: obs.clone(),
                abnormal: false,
            };
            det.feed(&held);
            peak = peak.max(det.live_chains());
        }
        readings += det.readings_seen();
        comparisons += det.comparisons();
        observations += stream.events().len() as u64;
        streams += 1;
    }
    println!(
        "streams {streams} observations/stream {:.1} counter readings/stream {:.1} comparisons/stream {:.1} peak live chains {peak}",
        observations as f64 / streams as f64,
        readings as f64 / streams as f64,
        comparisons as f64 / streams as f64
    );
}
