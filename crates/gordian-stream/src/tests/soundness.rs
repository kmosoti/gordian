//! Soundness of the first world's public consistency checker on the parts of the physics it
//! covers: a plain incident's own evidence, at every prefix, never contradicts its true
//! hypothesis, in the regime the checker's rules describe.

use super::cheap::probing_sim;
use super::*;
use crate::{StreamAction, StreamOutcome};
use gordian_core::Instant;
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{Observation, ProbeKind};

fn truth_of(t: &crate::oracle::IncidentTruth) -> gordian_world::Hypothesis {
    match t.truth.unwrap().kind {
        crate::StreamKind::Known(k) => Some((k, t.truth.unwrap().site)),
        crate::StreamKind::Hard(_) => panic!("plain incidents have known kinds"),
    }
}

#[test]
fn the_checker_never_excludes_the_truth_on_a_plain_incidents_evidence_prefixes() {
    let (mut prefixes, mut incidents, mut duos) = (0, 0, 0);
    for seed in 0..14 {
        let mut p = with_mix(seed, 1000, 0);
        p.regimes.clear();
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        for inc in &t.incidents {
            let ev = evidence_of(&s, &t, inc.id);
            let truth = truth_of(inc);
            incidents += 1;
            duos += inc.shape.duo as usize;
            for end in 1..=ev.len() {
                let open = consistent_hypotheses(&public, &ev[..end]);
                assert!(
                    open.contains(&truth),
                    "seed {seed} incident {} prefix {end}: {truth:?} not in {open:?}",
                    inc.id
                );
                prefixes += 1;
            }
        }
    }
    println!("{incidents} plain incidents ({duos} duos), {prefixes} prefixes, all sound");
    assert!(incidents > 150 && prefixes > 5000);
}

#[test]
fn the_checker_stays_sound_when_the_incidents_own_probes_are_added() {
    let mut checked = 0;
    for seed in 0..8 {
        let mut p = with_mix(seed, 1000, 0);
        p.regimes.clear();
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        let template = probing_sim(&p);
        for inc in &t.incidents {
            let site = inc.occupies[0];
            let truth = truth_of(inc);
            let mut ev = evidence_of(&s, &t, inc.id);
            // Every probe at the site, at an instant while the incident is live.
            let mut sim = template.clone();
            let at = Instant(inc.onset_ns + 1_500_000_000);
            for kind in ProbeKind::ALL {
                let StreamOutcome::Probed { observation, .. } =
                    sim.apply(StreamAction::Probe { kind, target: site }, at)
                else {
                    panic!()
                };
                ev.push((at, observation));
                let open = consistent_hypotheses(&public, &ev);
                assert!(
                    open.contains(&truth),
                    "seed {seed} incident {} after {kind:?}",
                    inc.id
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 500);
}

#[test]
fn the_checker_is_sound_on_the_unaffected_incidents_of_a_stream_with_regime_changes() {
    // With the default schedule the checker's rules stop describing the world at 200 s. Before
    // that, every plain incident is sound; after, the ones neither change touches are.
    let (mut before, mut after_unaffected) = (0, 0);
    for seed in 0..14 {
        let p = with_mix(seed, 1000, 0);
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        let first = t.regimes[0].at_ns;
        for inc in &t.incidents {
            let ev = evidence_of(&s, &t, inc.id);
            let truth = truth_of(inc);
            let open = consistent_hypotheses(&public, &ev);
            if inc.onset_ns < first {
                before += 1;
                assert!(open.contains(&truth));
            } else if open.contains(&truth) {
                after_unaffected += 1;
            }
        }
    }
    assert!(
        before > 100 && after_unaffected > 100,
        "{before} {after_unaffected}"
    );
}

#[test]
fn soundness_is_not_vacuous_a_wrong_hypothesis_is_excluded() {
    // The checker must be able to exclude things, or "the truth is never excluded" says nothing.
    let mut p = with_mix(1, 1000, 0);
    p.regimes.clear();
    let (s, t) = with_truth(&p);
    let public = s.public_info().world_public_info();
    let mut excluded = 0;
    for inc in &t.incidents {
        let ev = evidence_of(&s, &t, inc.id);
        let open = consistent_hypotheses(&public, &ev);
        // Silence is always permitted until something abnormal arrives, and then "no fault" goes.
        assert!(
            !open.contains(&None)
                || ev
                    .iter()
                    .all(|(_, o)| !matches!(o, Observation::Counter { value, .. } if *value >= 50))
        );
        excluded += (open.len() < 1 + 5 * t.services.len()) as usize;
    }
    assert_eq!(excluded, t.incidents.len());
}
