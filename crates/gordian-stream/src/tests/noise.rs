//! The first world's free noise filter does not work here.
//!
//! The first world separates signal from noise by the catalogue: noise is free-form text, signals
//! are catalogue messages and abnormal counters, and a component that reads the rules keeps
//! exactly what the physics can use. This file measures that filter on the first world (where it
//! is perfect) and on the stream (where it is not).

use super::*;
use crate::kinds::Tier;
use crate::labels::{EvidenceRole, NoiseKind, ObsLabel};
use gordian_world::episode::EpisodeClass;
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{
    EpisodeSpec, Observation, generate as generate_episode, oracle as world_oracle,
};

/// The first world's free filter: keep what the public physics can discriminate on. A counter at
/// or above the alarm threshold, a catalogue message other than `CheckHealth`, a snapshot whose
/// hash differs from the public one. Free-form messages, benign counters and unchanged snapshots
/// are dropped.
fn free_filter(o: &Observation, public_hash: &dyn Fn(gordian_world::ServiceId) -> u64) -> bool {
    match o {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => {
            matches!(SignalText::from_text_id(*text_id), Some(t) if t != SignalText::CheckHealth)
        }
        Observation::Snapshot {
            service,
            config_hash,
        } => *config_hash != public_hash(*service),
        _ => false,
    }
}

#[test]
fn the_free_filter_is_perfect_on_the_first_worlds_noise_flood() {
    // The control: on the first world the filter keeps every signal and nothing else.
    let (mut kept, mut signals, mut both) = (0u64, 0u64, 0u64);
    for seed in 0..40 {
        let ep = generate_episode(&EpisodeSpec::new(seed, EpisodeClass::NoiseFlood));
        let hidden = world_oracle::reveal(&ep);
        let hash = |s: gordian_world::ServiceId| ep.world().services[s.index()].config_hash;
        for ((_, o), label) in ep.stream().iter().zip(&hidden.labels) {
            let keep = free_filter(o, &hash);
            let signal = matches!(label, gordian_world::episode::StreamLabel::Signal { .. });
            kept += keep as u64;
            signals += signal as u64;
            both += (keep && signal) as u64;
        }
    }
    assert!(signals > 0);
    assert_eq!(kept, both, "the first world's filter keeps noise");
    assert_eq!(signals, both, "the first world's filter drops signal");
}

#[test]
fn the_free_filter_fails_on_the_stream() {
    let mut kept_incident = 0u64;
    let mut kept_background = 0u64;
    let mut by_noise: std::collections::BTreeMap<NoiseKind, u64> = Default::default();
    let (mut msg_in_range, mut msg_in_range_incident) = (0u64, 0u64);
    let (mut hard_decisive, mut hard_decisive_kept) = (0u64, 0u64);
    let mut decisive_total = 0u64;
    let mut decisive_kept = 0u64;
    for seed in 0..20 {
        let (s, t) = with_truth(&StreamParams::new(seed));
        let hash = |sv: gordian_world::ServiceId| t.services[sv.index()].config_hash;
        for (i, (_, o)) in s.events().iter().enumerate() {
            let keep = free_filter(o, &hash);
            let label = t.labels[i];
            if keep {
                match label {
                    ObsLabel::Incident { .. } => kept_incident += 1,
                    ObsLabel::Background(k) => {
                        kept_background += 1;
                        *by_noise.entry(k).or_insert(0) += 1;
                    }
                }
            }
            if let Observation::Message { text_id, .. } = o
                && SignalText::from_text_id(*text_id).is_some()
            {
                msg_in_range += 1;
                msg_in_range_incident += matches!(label, ObsLabel::Incident { .. }) as u64;
            }
            if let ObsLabel::Incident { id, role } = label
                && role == EvidenceRole::Decisive
            {
                decisive_total += 1;
                decisive_kept += keep as u64;
                if t.incidents[id as usize].tier == Tier::Hard {
                    hard_decisive += 1;
                    hard_decisive_kept += keep as u64;
                }
            }
        }
    }
    let precision = kept_incident as f64 / (kept_incident + kept_background) as f64;
    let msg_precision = msg_in_range_incident as f64 / msg_in_range as f64;
    let hard_recall = hard_decisive_kept as f64 / hard_decisive as f64;
    let decisive_recall = decisive_kept as f64 / decisive_total as f64;
    println!(
        "free filter on the stream: precision {precision:.3}; catalogue-range messages that \
         belong to an incident {msg_precision:.3}; recall of decisive evidence {decisive_recall:.3} \
         (hard incidents {hard_recall:.3}); background kept by type {by_noise:?}"
    );
    // On the first world both are 1.000. Here, a large share of what the filter keeps is
    // background, and it drops decisive evidence.
    assert!(precision < 0.80, "precision {precision}");
    assert!(msg_precision < 0.35, "message precision {msg_precision}");
    assert!(
        hard_recall < 0.70,
        "recall of hard incidents' decisive evidence {hard_recall}"
    );
    // The strays and blips and mini-bursts all get through: three independent ways to fail.
    for kind in [
        NoiseKind::CatalogueStray,
        NoiseKind::Blip,
        NoiseKind::MiniBurst,
    ] {
        assert!(by_noise.get(&kind).copied().unwrap_or(0) > 100, "{kind:?}");
    }
    // Nothing free-form got through, as in the first world: the failure is not the old one.
    assert_eq!(by_noise.get(&NoiseKind::FreeForm), None);
}

#[test]
fn the_noise_the_filter_drops_is_most_of_the_background_but_the_decisive_evidence_goes_with_it() {
    // The decisive evidence of a hard incident is carried by messages whose ids look like
    // free-form noise, and the same ids occur in the noise.
    let mut in_noise = 0u64;
    let mut ext_in_incidents = std::collections::BTreeSet::new();
    for seed in 0..10 {
        let (s, t) = with_truth(&with_mix(seed, 0, 1000));
        for inc in &t.incidents {
            for id in &inc.decisive {
                if let Observation::Message { text_id, .. } = &s.events()[id.0 as usize].1
                    && SignalText::from_text_id(*text_id).is_none()
                {
                    ext_in_incidents.insert(*text_id);
                }
            }
        }
        for (i, (_, o)) in s.events().iter().enumerate() {
            if let (Observation::Message { text_id, .. }, ObsLabel::Background(NoiseKind::FreeForm)) =
                (o, t.labels[i])
                && ext_in_incidents.contains(text_id)
            {
                in_noise += 1;
            }
        }
    }
    assert!(!ext_in_incidents.is_empty());
    assert!(
        in_noise > 200,
        "hard incidents' vocabulary appears in noise only {in_noise} times"
    );
}

#[test]
fn noise_has_the_declared_rates() {
    let p = StreamParams::new(0);
    let mut counts: std::collections::BTreeMap<NoiseKind, u64> = Default::default();
    let streams = 10u64;
    for seed in 0..streams {
        let (_, t) = with_truth(&StreamParams::new(seed));
        for l in &t.labels {
            if let ObsLabel::Background(k) = l {
                *counts.entry(*k).or_insert(0) += 1;
            }
        }
    }
    let secs = (p.duration_ns / 1_000_000_000 * streams) as f64;
    let expect = |k: NoiseKind, mhz: u64, per_event: f64| {
        let want = mhz as f64 / 1000.0 * secs * per_event;
        let got = counts[&k] as f64;
        assert!(
            (got - want).abs() < 5.0 * want.sqrt() * per_event.max(1.0),
            "{k:?}: {got} vs {want}"
        );
    };
    expect(NoiseKind::Benign, p.noise.benign_mhz, 1.0);
    expect(NoiseKind::FreeForm, p.noise.freeform_mhz, 1.0);
    expect(NoiseKind::CatalogueStray, p.noise.catalogue_mhz, 1.0);
    expect(NoiseKind::Blip, p.noise.blip_mhz, 1.0);
    expect(NoiseKind::Snapshot, p.noise.snapshot_mhz, 1.0);
    expect(NoiseKind::MiniBurst, p.noise.burst_mhz, 2.0);
}

#[test]
fn a_mini_burst_looks_like_the_start_of_an_incident_and_has_no_heartbeat() {
    // Within its first 30 ms a mini-burst is an anchor and a characteristic message at one
    // service, as a plain incident's first moments are; nothing follows at that service from it.
    use gordian_world::physics::consistent_hypotheses;
    let (s, t) = with_truth(&StreamParams::new(4));
    let public = s.public_info().world_public_info();
    let mut seen = 0;
    let mut i = 0;
    while i + 1 < s.events().len() {
        if t.labels[i] == ObsLabel::Background(NoiseKind::MiniBurst)
            && t.labels[i + 1] == ObsLabel::Background(NoiseKind::MiniBurst)
        {
            let ev = vec![s.events()[i].clone(), s.events()[i + 1].clone()];
            // Both observations, read alone, leave a unique known hypothesis or a few: a mini-burst
            // is a believable incident to the public rules.
            let open = consistent_hypotheses(&public, &ev);
            if !open.is_empty() {
                seen += 1;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    assert!(seen > 3, "{seen}");
}
