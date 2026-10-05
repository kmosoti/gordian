//! What each tier guarantees, checked against the first world's public rules and through the
//! stream's own interface, never by reading how the generator builds them.
//!
//! The tests that run the first world's four components and the shared decision rule on an
//! incident (plain incidents identified from their own evidence and in noise; hard incidents the
//! cheap rung cannot identify) moved to `crates/gordian-run/tests/stream_cheap.rs` with work item
//! R3, because `gordian-run` now depends on this crate.

use super::cheap::probing_sim;
use super::*;
use crate::kinds::{HardKind, Tier};
use crate::oracle::IncidentTruth;
use gordian_core::Instant;
use gordian_world::physics::{HIGH, consistent_hypotheses};
use gordian_world::{FaultKind, Observation, ServiceId};

fn no_regime(seed: u64, plain: u32, hard: u32) -> StreamParams {
    let mut p = with_mix(seed, plain, hard);
    p.regimes.clear();
    p
}

// ---- Hard

#[test]
fn the_first_moments_of_a_hard_or_decoy_incident_give_the_cheap_rung_every_kind_of_reading() {
    // Over phase 1 only, per tier: how often the public rules leave one hypothesis, a few, none,
    // or everything. The cheap rung's confidence must not be a free separator of the tiers.
    let t0 = crate::timing::T0_NS;
    let mut stats: std::collections::BTreeMap<Tier, [u32; 4]> = Default::default();
    for seed in 0..10 {
        let mut p = StreamParams::new(seed);
        p.regimes.clear();
        p.mix.plain_permille = 400;
        p.mix.hard_permille = 300;
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        for inc in &t.incidents {
            let ev: Evidence = evidence_of(&s, &t, inc.id)
                .into_iter()
                .filter(|(at, _)| at.0 < inc.onset_ns + t0)
                .collect();
            let open = consistent_hypotheses(&public, &ev);
            let bucket = if open.is_empty() {
                2
            } else if open.contains(&None) {
                3
            } else if open.len() == 1 {
                0
            } else {
                1
            };
            stats.entry(inc.tier).or_default()[bucket] += 1;
        }
    }
    println!("phase 1, [unique, few, empty, silent] per tier: {stats:?}");
    let plain = stats[&Tier::Plain];
    let plain_n: u32 = plain.iter().sum();
    assert!(
        (plain[0] + plain[1]) * 100 >= plain_n * 98,
        "plain incidents are read by the rules: {plain:?}"
    );
    for tier in [Tier::Hard, Tier::Decoy] {
        let st = stats[&tier];
        let n: u32 = st.iter().sum();
        for (i, name) in ["unique", "few", "empty", "silent"].iter().enumerate() {
            if i == 1 {
                continue;
            }
            assert!(st[i] * 100 >= n * 12, "{tier:?}: {name} too rare in {st:?}");
        }
    }
    // The two tiers must have the same mix of readings (they are drawn the same way): within
    // sampling error.
    let h = stats[&Tier::Hard];
    let d = stats[&Tier::Decoy];
    let (hn, dn) = (h.iter().sum::<u32>() as f64, d.iter().sum::<u32>() as f64);
    for i in 0..4 {
        let (ph, pd) = (h[i] as f64 / hn, d[i] as f64 / dn);
        let pooled = (h[i] + d[i]) as f64 / (hn + dn);
        let se = (pooled * (1.0 - pooled) * (1.0 / hn + 1.0 / dn))
            .sqrt()
            .max(1e-9);
        assert!(
            (ph - pd).abs() < 4.5 * se,
            "reading {i}: {ph:.3} vs {pd:.3}"
        );
    }
}

#[test]
fn hard_incidents_carry_decisive_evidence_the_free_filter_cannot_see() {
    // Every hard incident has decisive observations in a vocabulary outside the first world's
    // catalogue; the cheap rung's rules ignore them by construction.
    use gordian_world::physics::SignalText;
    let mut with_ext = 0;
    let mut total = 0;
    for seed in 0..6 {
        let p = no_regime(seed, 0, 1000);
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            total += 1;
            let ext = inc
                .decisive
                .iter()
                .filter(|id| {
                    matches!(&s.events()[id.0 as usize].1,
                        Observation::Message { text_id, .. } if SignalText::from_text_id(*text_id).is_none())
                })
                .count();
            assert!((3..=5).contains(&ext), "{ext} ext messages");
            with_ext += 1;
        }
    }
    assert_eq!(with_ext, total);
}

// ---- Decoys

#[test]
fn a_decoy_alarms_like_an_incident_and_then_resolves_by_itself() {
    let t0 = crate::timing::T0_NS;
    let (mut n, mut with_alarm) = (0, 0);
    for seed in 0..15 {
        let p = no_regime(seed, 0, 0); // every incident a decoy
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            assert_eq!(inc.tier, Tier::Decoy);
            assert!(inc.truth.is_none() && inc.deadline_ns.is_none());
            let span = inc.live_end_ns - inc.onset_ns;
            assert!(
                (t0..=t0 + crate::timing::DECOY_SPAN_NS).contains(&span),
                "{span}"
            );
            n += 1;
            let site = inc.occupies[0];
            let series: Vec<(u64, u64)> = inc
                .observations
                .iter()
                .filter_map(|id| match &s.events()[id.0 as usize] {
                    (
                        at,
                        Observation::Counter {
                            service,
                            value,
                            name,
                        },
                    ) if *service == site
                        && matches!(
                            name,
                            gordian_world::CounterName::ErrorRate
                                | gordian_world::CounterName::Saturation
                        ) =>
                    {
                        Some((at.0, *value))
                    }
                    _ => None,
                })
                .collect();
            // Before it resolves it alarms; after, never again.
            let early_alarm = series
                .iter()
                .any(|(at, v)| *at < inc.live_end_ns && *v >= HIGH)
                || inc.shape.hard_kind == Some(HardKind::SlowLeak);
            if early_alarm {
                with_alarm += 1;
            }
            assert!(
                series
                    .iter()
                    .filter(|(at, _)| *at >= inc.live_end_ns)
                    .all(|(_, v)| *v < HIGH),
                "a decoy alarmed after it resolved"
            );
            let after = series
                .iter()
                .filter(|(at, _)| *at >= inc.live_end_ns)
                .count();
            assert_eq!(
                after,
                crate::timing::RECOVERY_TOTAL,
                "ten benign readings follow"
            );
            // The first five are the decisive ones.
            let decisive_after = inc
                .decisive
                .iter()
                .filter(|id| s.events()[id.0 as usize].0.0 >= inc.live_end_ns)
                .count();
            assert_eq!(decisive_after, crate::timing::RECOVERY_DECISIVE);
        }
    }
    assert!(n > 60, "{n}");
    assert_eq!(n, with_alarm, "every decoy alarmed before it resolved");
}

#[test]
fn hard_incidents_keep_alarming_until_after_their_deadline() {
    for seed in 0..10 {
        let p = no_regime(seed, 0, 1000);
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            let deadline = inc.deadline_ns.unwrap();
            assert!(inc.live_end_ns >= deadline + crate::timing::GRACE_NS.0);
            let site = inc.occupies[0];
            let last_alarm = inc
                .observations
                .iter()
                .filter_map(|id| match &s.events()[id.0 as usize] {
                    (at, Observation::Counter { service, value, .. })
                        if *service == site && *value >= HIGH =>
                    {
                        Some(at.0)
                    }
                    _ => None,
                })
                .max();
            // A leak may not have crossed the threshold at all before its stream ended; the
            // others always alarm until the deadline (a flap hides at most a few beats). An
            // incident whose deadline falls after the end of the stream is cut off by it.
            if inc.shape.hard_kind != Some(HardKind::SlowLeak) && deadline <= t.duration_ns {
                let last = last_alarm.expect("a hard incident alarms");
                assert!(
                    last + 8_000_000_000 >= deadline,
                    "last alarm {last} vs deadline {deadline}"
                );
            }
        }
    }
}

// ---- Deadlines

#[test]
fn deadlines_follow_the_declared_distributions() {
    let params = StreamParams::new(0);
    let spec = params.deadlines;
    let mut by: std::collections::BTreeMap<(Tier, bool), Vec<f64>> = Default::default();
    for seed in 0..150 {
        let mut p = StreamParams::new(seed);
        p.recurrence_permille = 0;
        p.mix.plain_permille = 500;
        p.mix.hard_permille = 500;
        p.critical.plain_permille = 500;
        p.critical.hard_permille = 500;
        let (_, t) = with_truth(&p);
        for inc in &t.incidents {
            let w = spec.window(inc.tier, inc.critical).unwrap();
            let d = inc.deadline_ns.unwrap() - inc.onset_ns;
            assert!((w.lo_ns..=w.hi_ns).contains(&d), "{:?} {d}", inc.tier);
            by.entry((inc.tier, inc.critical))
                .or_default()
                .push((d - w.lo_ns) as f64 / (w.hi_ns - w.lo_ns) as f64);
        }
    }
    assert_eq!(by.len(), 4);
    for ((tier, crit), xs) in &by {
        let n = xs.len() as f64;
        assert!(n > 200.0, "{tier:?} {crit}: {n}");
        let mean = xs.iter().sum::<f64>() / n;
        // Uniform on [0, 1]: mean 1/2, variance 1/12.
        assert!(
            (mean - 0.5).abs() < 4.5 * (1.0 / 12.0f64 / n).sqrt(),
            "{tier:?} {crit}: {mean}"
        );
        let low = xs.iter().filter(|x| **x < 0.25).count() as f64;
        assert!(
            (low / n - 0.25).abs() < 4.5 * (0.25 * 0.75 / n).sqrt(),
            "{tier:?} {crit}: {low}/{n}"
        );
    }
    // Critical hard deadlines are the shortest of the hard ones, by declaration.
    assert!(spec.hard_critical.hi_ns <= spec.hard.hi_ns);
    assert!(spec.hard_critical.lo_ns < spec.hard.lo_ns);
    assert!(spec.window(Tier::Decoy, false).is_none());
}

#[test]
fn critical_shares_match_the_parameters() {
    let (mut plain, mut plain_c, mut hard, mut hard_c) = (0u32, 0u32, 0u32, 0u32);
    for seed in 0..150 {
        let mut p = StreamParams::new(seed);
        p.recurrence_permille = 0;
        let (_, t) = with_truth(&p);
        for i in &t.incidents {
            match i.tier {
                Tier::Plain => {
                    plain += 1;
                    plain_c += i.critical as u32;
                }
                Tier::Hard => {
                    hard += 1;
                    hard_c += i.critical as u32;
                }
                Tier::Decoy => {}
            }
        }
    }
    let check = |c: u32, n: u32, p: f64| {
        let sd = (n as f64 * p * (1.0 - p)).sqrt();
        assert!((c as f64 - n as f64 * p).abs() < 4.5 * sd, "{c}/{n} vs {p}");
    };
    check(plain_c, plain, 0.15);
    check(hard_c, hard, 0.30);
}

// ---- Probes

#[test]
fn hard_probe_rules_are_what_the_documentation_says() {
    use crate::{StreamAction, StreamOutcome};
    use gordian_world::{ProbeKind, ProbeResult};
    let mut checked = std::collections::BTreeSet::new();
    for seed in 0..10 {
        let p = no_regime(seed, 0, 1000);
        let (s, t) = with_truth(&p);
        let template = probing_sim(&p);
        for inc in &t.incidents {
            let site = inc.occupies[0];
            let at = Instant(inc.onset_ns + 1_000_000_000);
            let probe = |kind, target| {
                let mut sim = template.clone();
                match sim.apply(StreamAction::Probe { kind, target }, at) {
                    StreamOutcome::Probed {
                        observation: Observation::Probed { result, .. },
                        ..
                    } => result,
                    o => panic!("{o:?}"),
                }
            };
            let hk = inc.shape.hard_kind.unwrap();
            checked.insert(hk);
            assert_eq!(probe(ProbeKind::HealthCheck, site), ProbeResult::Positive);
            match hk {
                HardKind::Compound => {
                    let (a, b) = inc.shape.pair.unwrap();
                    let has = |k| a == k || b == k;
                    assert_eq!(
                        probe(ProbeKind::ResourceUsage, site) == ProbeResult::Positive,
                        has(FaultKind::ResourceExhausted)
                    );
                    assert_eq!(
                        probe(ProbeKind::CredentialCheck, site) == ProbeResult::Positive,
                        has(FaultKind::CredentialExpired)
                    );
                    assert_eq!(probe(ProbeKind::LatencySample, site), ProbeResult::Negative);
                    let public = s.public_info().services[site.index()].config_hash;
                    let changed = !matches!(
                        probe(ProbeKind::ConfigSnapshot, site),
                        ProbeResult::ConfigHash(h) if h == public
                    );
                    assert_eq!(changed, has(FaultKind::ConfigDrift));
                }
                HardKind::SplitBrain => {
                    let peer = inc.shape.other.unwrap();
                    for who in [site, peer] {
                        assert_eq!(probe(ProbeKind::LatencySample, who), ProbeResult::Positive);
                        assert_eq!(probe(ProbeKind::ErrorSample, who), ProbeResult::Positive);
                        assert_eq!(probe(ProbeKind::ResourceUsage, who), ProbeResult::Negative);
                    }
                }
                HardKind::Cascade => {
                    let partner = inc.shape.other.unwrap();
                    assert_eq!(
                        probe(ProbeKind::HealthCheck, partner),
                        ProbeResult::Positive
                    );
                    assert_eq!(
                        probe(ProbeKind::ResourceUsage, partner),
                        ProbeResult::Negative
                    );
                }
                HardKind::SlowLeak => {
                    // Before the series crosses the threshold the usage probe says healthy.
                    let early = Instant(inc.onset_ns + 1_000);
                    let mut sim = template.clone();
                    let r = match sim.apply(
                        StreamAction::Probe {
                            kind: ProbeKind::ResourceUsage,
                            target: site,
                        },
                        early,
                    ) {
                        StreamOutcome::Probed {
                            observation: Observation::Probed { result, .. },
                            ..
                        } => result,
                        o => panic!("{o:?}"),
                    };
                    assert_eq!(r, ProbeResult::Negative);
                }
            }
        }
    }
    assert_eq!(checked.len(), 4);
}

#[test]
fn a_service_with_no_live_incident_answers_probes_as_healthy() {
    use crate::{StreamAction, StreamOutcome};
    use gordian_world::{ProbeKind, ProbeResult};
    let p = no_regime(2, 800, 100);
    let s = generate(&p);
    let mut sim = probing_sim(&p);
    // Service 0 at time zero: nothing has started anywhere.
    for kind in ProbeKind::ALL {
        let StreamOutcome::Probed {
            observation: Observation::Probed { result, .. },
            ..
        } = sim.apply(
            StreamAction::Probe {
                kind,
                target: ServiceId(0),
            },
            Instant(1),
        )
        else {
            panic!()
        };
        match kind {
            ProbeKind::ConfigSnapshot => assert_eq!(
                result,
                ProbeResult::ConfigHash(s.public_info().services[0].config_hash)
            ),
            _ => assert_eq!(result, ProbeResult::Negative, "{kind:?}"),
        }
    }
}

#[test]
fn plain_probes_answer_as_the_first_world_does() {
    use crate::{StreamAction, StreamOutcome};
    use gordian_world::physics::probe_result;
    use gordian_world::{Probe, ProbeKind};
    let p = no_regime(3, 1000, 0);
    let (s, t) = with_truth(&p);
    let template = probing_sim(&p);
    let services = s.public_info().services;
    let mut n = 0;
    for inc in &t.incidents {
        let site = inc.occupies[0];
        let kind = inc.shape.known_kind.unwrap();
        let at = Instant(inc.onset_ns + 500_000_000);
        for pk in ProbeKind::ALL {
            let mut sim = template.clone();
            let StreamOutcome::Probed {
                observation: Observation::Probed { result, .. },
                ..
            } = sim.apply(
                StreamAction::Probe {
                    kind: pk,
                    target: site,
                },
                at,
            )
            else {
                panic!()
            };
            // Bits and the drift hash are hidden; the probes whose answer does not depend on
            // them are compared exactly, the rest by the property the first world gives them.
            let truth_probe = Probe {
                kind: pk,
                target: site,
            };
            let reference =
                probe_result(&services, Some((kind, site)), (true, true), 7, truth_probe);
            match pk {
                ProbeKind::LatencySample | ProbeKind::ErrorSample => {}
                ProbeKind::ConfigSnapshot => {}
                _ => assert_eq!(result, reference, "{pk:?} against {kind:?}"),
            }
            n += 1;
        }
    }
    assert!(n > 100);
}

#[test]
fn a_decoy_answers_probes_like_a_sick_service_while_live_and_like_a_healthy_one_after() {
    use gordian_world::{Probe, ProbeKind, ProbeResult};
    let mut n = 0;
    for seed in 0..10 {
        let p = no_regime(seed, 0, 0);
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            let hidden = &s.incidents[inc.id as usize];
            let site = inc.occupies[0];
            let at = |t_ns: u64, kind, target| {
                crate::probe::answer(hidden, &s.services, Probe { kind, target }, Instant(t_ns))
            };
            // Live: a health check at the site is positive, as for a hard incident.
            assert_eq!(
                at(inc.onset_ns + 1_000_000, ProbeKind::HealthCheck, site),
                ProbeResult::Positive
            );
            // Resolved: every probe at every service it occupied reads healthy.
            for who in &inc.occupies {
                for kind in ProbeKind::ALL {
                    let healthy = gordian_world::physics::probe_result(
                        &s.services,
                        None,
                        (false, false),
                        0,
                        Probe { kind, target: *who },
                    );
                    assert_eq!(at(inc.live_end_ns, kind, *who), healthy, "{kind:?}");
                    assert_eq!(at(inc.live_end_ns + 5_000_000_000, kind, *who), healthy);
                }
            }
            n += 1;
        }
    }
    assert!(n > 60);
}

/// The earliest instant at which the incident's own readings at its site show that it has
/// resolved, by the simplest public rule: five benign heartbeats in a row, or, for the leak's
/// climbing reading, a reading ten or more below the highest so far.
fn recovery_seen_at(s: &crate::Stream, inc: &IncidentTruth) -> Option<u64> {
    let site = inc.occupies[0];
    let leak = inc.shape.hard_kind == Some(HardKind::SlowLeak);
    let mut run = 0;
    let mut max = 0u64;
    for o in &inc.observations {
        let (at, obs) = &s.events()[o.0 as usize];
        let Observation::Counter {
            service,
            name,
            value,
        } = obs
        else {
            continue;
        };
        if *service != site
            || !matches!(
                name,
                gordian_world::CounterName::ErrorRate | gordian_world::CounterName::Saturation
            )
            || at.0 < inc.onset_ns + 500_000_000
        {
            continue;
        }
        if leak {
            if *value + 10 <= max {
                return Some(at.0);
            }
            max = max.max(*value);
        } else if *value < HIGH {
            run += 1;
            if run == 5 {
                return Some(at.0);
            }
        } else {
            run = 0;
        }
    }
    None
}

#[test]
fn a_decoys_resolution_can_be_read_from_public_readings_but_only_after_waiting() {
    // The evidence that separates a decoy from a hard incident exists in the stream, and costs
    // time: the simplest rule that finds it fires about five beats after the decoy resolves.
    let (mut decoys, mut found, mut hard, mut false_alarms, mut premature) = (0, 0, 0, 0, 0);
    let mut delays = Vec::new();
    let mut before_20s = 0;
    for seed in 0..25 {
        let mut p = with_mix(seed, 0, 500);
        p.regimes.clear();
        let (s, t) = with_truth(&p);
        for inc in &t.incidents {
            let seen = recovery_seen_at(&s, inc);
            match inc.tier {
                Tier::Decoy => {
                    decoys += 1;
                    if let Some(at) = seen {
                        found += 1;
                        delays.push((at - inc.onset_ns) as f64 / 1e9);
                        before_20s += ((at - inc.onset_ns) < 20_000_000_000) as u32;
                        // Five flaps in a row happen by chance before it has resolved.
                        premature += (at < inc.live_end_ns) as u32;
                    }
                }
                Tier::Hard => {
                    hard += 1;
                    // Only count a false alarm before the deadline: afterwards the incident
                    // closes, which looks the same.
                    if let Some(at) = seen
                        && at < inc.deadline_ns.unwrap()
                    {
                        false_alarms += 1;
                    }
                }
                Tier::Plain => unreachable!(),
            }
        }
    }
    delays.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = delays[delays.len() / 2];
    println!(
        "decoys: {found}/{decoys} resolutions read from the stream, median {median:.1} s after \
         onset, {before_20s} before 20 s; hard incidents wrongly read as resolved before their \
         deadline: {false_alarms}/{hard}; read before the decoy had resolved: {premature}"
    );
    assert!(premature * 100 <= decoys * 3, "{premature}/{decoys}");
    assert!(found * 100 >= decoys * 97, "{found}/{decoys}");
    assert!(false_alarms * 100 <= hard * 3, "{false_alarms}/{hard}");
    // Waiting is not free: the median reading is some 19 s after onset, against hard-critical
    // deadlines of 20 to 40 s.
    assert!(median > 12.0 && median < 30.0, "{median}");
    assert!(
        before_20s * 10 < found * 7,
        "over 30% of decoys are still unread at 20 s: {before_20s}/{found} read"
    );
}
