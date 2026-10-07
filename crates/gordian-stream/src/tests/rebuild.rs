//! `oracle::rebuilt_incident` (W2's experimenter-side accessor) rebuilds what the stream holds,
//! and undoes a regime change only for the incidents it altered.

use super::*;
use crate::oracle::{rebuilt_incident, reveal};

fn actual(s: &Stream, t: &StreamTruth, id: u32) -> Vec<(u64, Observation)> {
    let onset = t.incidents[id as usize].onset_ns;
    evidence_of(s, t, id)
        .into_iter()
        .map(|(at, obs)| (at.0 - onset, obs))
        .collect()
}

#[test]
fn rebuilt_with_the_streams_own_physics_and_graph_is_what_the_stream_holds() {
    for seed in 0..25u64 {
        let s = generate(&StreamParams::new(seed));
        let t = reveal(&s);
        for inc in &t.incidents {
            let rebuilt = rebuilt_incident(&s, inc.id, false, false).expect("known incident");
            assert_eq!(
                rebuilt,
                actual(&s, &t, inc.id),
                "seed {seed} incident {}",
                inc.id
            );
        }
    }
}

#[test]
fn undoing_the_regimes_changes_only_incidents_after_the_first_change_and_has_power() {
    let (mut by_physics, mut by_graph) = (0u32, 0u32);
    for seed in 0..60u64 {
        let s = generate(&StreamParams::new(seed));
        let t = reveal(&s);
        let first = t.regimes.first().map(|r| r.at_ns).unwrap_or(u64::MAX);
        for inc in &t.incidents {
            let a = actual(&s, &t, inc.id);
            let base = rebuilt_incident(&s, inc.id, true, false).expect("known incident");
            let zero = rebuilt_incident(&s, inc.id, false, true).expect("known incident");
            if inc.onset_ns < first {
                assert_eq!(base, a, "seed {seed} incident {} before any change", inc.id);
                assert_eq!(zero, a, "seed {seed} incident {} before any change", inc.id);
            }
            by_physics += (base != a) as u32;
            by_graph += (zero != a) as u32;
        }
    }
    assert!(
        by_physics > 0,
        "no incident was altered by the signature shift"
    );
    assert!(by_graph > 0, "no incident was altered by the added edge");
}

#[test]
fn an_unknown_incident_has_no_rebuild() {
    let s = generate(&StreamParams::new(1));
    assert!(rebuilt_incident(&s, 10_000, false, false).is_none());
}
