//! The later re-anchor noticer (work item B2): the rule as a pure function, its boundaries, the
//! readings the module documentation states, the failure it answers (a stray before a burst), the
//! identity with the rung's noticer when it never moves, and the files.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`experiments/exploration/scripts/b1_gate.py`); the tests of `stream_noticer.rs` pin the same
//! property of the seam on small streams.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::noticer::{NoticeKind, NoticerSpec, Tracked};
use gordian_run::stream::arms::noticer_reanchor::{Isolation, later_burst_start};
use gordian_run::stream::arms::rung::{Held, Rung, RungConfig};
use gordian_run::stream::execute_stream;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamEvent, StreamPublic};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::SignalText;
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use serde_json::{Value, json};
use stream_common::*;

const S: u64 = 1_000;
const MS: u64 = 1_000_000;
/// The rung's own burst window, 0.4 s.
const BURST_NS: u64 = 400 * MS;

fn counter(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

fn message(service: ServiceId, text: SignalText) -> Observation {
    Observation::Message {
        service,
        text_id: text.text_id(),
        severity: Severity::Medium,
    }
}

fn spec(gap_ms: u64, min_burst: u32, isolation: Isolation) -> NoticerSpec {
    NoticerSpec::Reanchor {
        notice_z: None,
        gap_ns: gap_ms * MS,
        min_burst,
        isolation,
    }
}

/// Two services where the second depends on the first, and a stranger.
fn picks(public: &StreamPublic) -> (ServiceId, ServiceId, ServiceId) {
    for s in 0..public.services.len() {
        let site = ServiceId(s as u32);
        let mask = dependents_mask(&public.services, site);
        let dependents: Vec<usize> = (0..mask.len()).filter(|i| mask[*i]).collect();
        let stranger = (0..mask.len()).find(|i| !mask[*i] && *i != s);
        if let (Some(d), Some(z)) = (dependents.first(), stranger) {
            return (site, ServiceId(*d as u32), ServiceId(z as u32));
        }
    }
    panic!("no service has a dependent and a stranger");
}

/// Five abnormal observations about `service` in 40 ms from `t` ms.
fn burst(service: ServiceId, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(service, CounterName::ErrorRate, 80)),
        (t + 10, counter(service, CounterName::Latency, 90)),
        (t + 20, message(service, SignalText::OutOfResource)),
        (t + 30, counter(service, CounterName::Saturation, 90)),
        (t + 40, counter(service, CounterName::ErrorRate, 85)),
    ]
}

fn stray(service: ServiceId, t: u64) -> (u64, Observation) {
    (t, counter(service, CounterName::Latency, 70))
}

/// A rung with `noticer`, fed the observations in one step at `now_ms`.
struct Feed {
    rung: Rung,
}

impl Feed {
    fn new(public: &StreamPublic, noticer: NoticerSpec) -> Self {
        let cfg = RungConfig {
            noticer,
            ..RungConfig::default()
        };
        Self {
            rung: Rung::new(public, cfg),
        }
    }

    fn step(&mut self, now_ms: u64, observations: &[(u64, Observation)]) -> Vec<ObsId> {
        let events: Vec<StreamEvent> = observations
            .iter()
            .enumerate()
            .map(|(i, (ms, obs))| StreamEvent::Observed {
                id: ObsId(i as u32),
                at: at(*ms),
                obs: obs.clone(),
            })
            .collect();
        self.rung.absorb(&events, &[], at(now_ms));
        self.rung.notice(at(now_ms));
        (0..observations.len()).map(|i| ObsId(i as u32)).collect()
    }

    /// (anchor, anchor instant) of every notice, in order.
    fn anchors(&self) -> Vec<(ObsId, Instant)> {
        self.rung
            .notice_log()
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .map(|e| (e.anchor, e.anchor_at))
            .collect()
    }

    fn sites(&self) -> Vec<ServiceId> {
        self.rung
            .notice_log()
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .map(|e| e.site)
            .collect()
    }
}

/// A tracked anomaly holding the abnormal observations `obs` (milliseconds, service), in order,
/// the first being the anchor.
fn tracked(public: &StreamPublic, obs: &[(u64, ServiceId)]) -> Tracked {
    let held = |i: usize, ms: u64, service: ServiceId| Held {
        id: ObsId(i as u32),
        at: at(ms),
        obs: counter(service, CounterName::ErrorRate, 90),
        abnormal: true,
    };
    let (ms0, s0) = obs[0];
    let mut a = Tracked::new(0, &held(0, ms0, s0), s0, &public.services, 2_000 * MS);
    for (i, (ms, s)) in obs.iter().enumerate().skip(1) {
        a.note_attached(&held(i, *ms, *s), *s, 2_000 * MS);
    }
    a
}

fn start(a: &Tracked, gap_ms: u64, min_burst: u32, isolation: Isolation) -> Option<usize> {
    later_burst_start(a, gap_ms * MS, min_burst, BURST_NS, isolation)
}

// ---- the spec and the manifest

#[test]
fn the_reanchor_noticer_is_written_and_read_with_every_parameter() {
    let value = json!({"noticer": "reanchor", "gap_ns": 200_000_000u64, "min_burst": 3, "isolation": "site"});
    let parsed: NoticerSpec = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed, spec(200, 3, Isolation::Site));
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    let with_z = json!({"noticer": "reanchor", "notice_z": 2.0, "gap_ns": 100_000_000u64, "min_burst": 2, "isolation": "any"});
    let parsed: NoticerSpec = serde_json::from_value(with_z.clone()).unwrap();
    assert_eq!(
        parsed,
        NoticerSpec::Reanchor {
            notice_z: Some(2.0),
            gap_ns: 100 * MS,
            min_burst: 2,
            isolation: Isolation::Any
        }
    );
    assert_eq!(serde_json::to_value(parsed).unwrap(), with_z);
    assert_eq!(parsed.id(), "reanchor");
    assert!(!parsed.is_default());
    // A parameter the noticer does not have, a missing one and an unknown word are errors.
    assert!(
        serde_json::from_value::<NoticerSpec>(
            json!({"noticer": "reanchor", "gap_ns": 1, "min_burst": 2, "isolation": "site", "quiet_ns": 1})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<NoticerSpec>(
            json!({"noticer": "reanchor", "gap_ns": 1, "min_burst": 2})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<NoticerSpec>(
            json!({"noticer": "reanchor", "gap_ns": 1, "min_burst": 2, "isolation": "near"})
        )
        .is_err()
    );
    assert_eq!(Isolation::Site.as_str(), "site");
    assert_eq!(Isolation::Any.as_str(), "any");
}

#[test]
fn the_reanchor_parameters_are_checked() {
    assert!(spec(200, 3, Isolation::Site).validate().is_ok());
    assert!(spec(1, 2, Isolation::Any).validate().is_ok());
    assert!(
        NoticerSpec::Reanchor {
            notice_z: None,
            gap_ns: 0,
            min_burst: 3,
            isolation: Isolation::Site
        }
        .validate()
        .unwrap_err()
        .contains("gap_ns")
    );
    assert!(
        spec(200, 1, Isolation::Site)
            .validate()
            .unwrap_err()
            .contains("min_burst")
    );
    assert!(
        NoticerSpec::Reanchor {
            notice_z: Some(f64::INFINITY),
            gap_ns: 1,
            min_burst: 2,
            isolation: Isolation::Site
        }
        .validate()
        .unwrap_err()
        .contains("notice_z")
    );
    let mut m = manifest(
        "reanchor-bad",
        &[("never_escalate", "never_escalate")],
        1,
        150,
        0,
    );
    m.noticers
        .insert("never_escalate".to_owned(), spec(0, 3, Isolation::Site));
    assert!(m.validate().is_err());
}

// ---- the rule, as a pure function

#[test]
fn an_isolated_anchor_moves_to_the_first_observation_that_begins_a_burst() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    // A stray, then a burst of four 300 ms later: isolated (gap 200 ms), the burst begins at 1.
    let a = tracked(
        &public,
        &[
            (1_000, site),
            (1_300, site),
            (1_310, site),
            (1_320, site),
            (1_330, site),
        ],
    );
    assert_eq!(start(&a, 200, 3, Isolation::Site), Some(1));
    assert_eq!(start(&a, 200, 4, Isolation::Site), Some(1));
    // Five observations in the window of the first of the burst are not there: it has four.
    assert_eq!(start(&a, 200, 5, Isolation::Site), None);
    // No second observation, or none that begins a burst: the anchor stays.
    let lone = tracked(&public, &[(1_000, site)]);
    assert_eq!(start(&lone, 200, 2, Isolation::Site), None);
}

#[test]
fn the_anchor_is_isolated_when_the_next_observation_is_at_least_g_after_it() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let obs = |gap: u64| {
        [
            (1_000, site),
            (1_000 + gap, site),
            (1_010 + gap, site),
            (1_020 + gap, site),
        ]
    };
    // Exactly g after the anchor: isolated, so it moves.
    assert_eq!(
        start(&tracked(&public, &obs(300)), 300, 3, Isolation::Site),
        Some(1)
    );
    assert_eq!(
        start(&tracked(&public, &obs(300)), 300, 3, Isolation::Any),
        Some(1)
    );
    // One millisecond short of g: not isolated, so it stays.
    assert_eq!(
        start(&tracked(&public, &obs(299)), 300, 3, Isolation::Site),
        None
    );
    assert_eq!(
        start(&tracked(&public, &obs(299)), 300, 3, Isolation::Any),
        None
    );
    // At the anchor's own instant: not isolated, and not later than the anchor either.
    let same = tracked(
        &public,
        &[(1_000, site), (1_000, site), (1_010, site), (1_020, site)],
    );
    assert_eq!(start(&same, 1, 3, Isolation::Site), None);
    // A burst far after the anchor is still a burst after it: there is no upper bound on the lag.
    let spread = tracked(
        &public,
        &[(1_000, site), (1_500, site), (1_510, site), (1_520, site)],
    );
    assert_eq!(start(&spread, 300, 3, Isolation::Site), Some(1));
}

#[test]
fn site_counts_only_observations_about_the_anchors_service_and_any_counts_every_one() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    // A stray at the site, then a burst at a dependent beginning 100 ms later: no other observation
    // at the site, so the anchor is isolated under `site` and the burst (at the dependent, strictly
    // later) is where it moves. Under `any` the dependent's observation is within g = 200 ms.
    let a = tracked(
        &public,
        &[
            (1_000, site),
            (1_100, dependent),
            (1_110, dependent),
            (1_120, dependent),
        ],
    );
    assert_eq!(start(&a, 200, 3, Isolation::Site), Some(1));
    assert_eq!(start(&a, 200, 3, Isolation::Any), None);
    // With g = 100 ms the dependent's first is exactly g after: isolated under both.
    assert_eq!(start(&a, 100, 3, Isolation::Any), Some(1));
    // An observation about the site itself inside g makes the anchor non-isolated under both.
    let b = tracked(
        &public,
        &[
            (1_000, site),
            (1_050, site),
            (1_400, dependent),
            (1_410, dependent),
            (1_420, dependent),
        ],
    );
    assert_eq!(start(&b, 200, 3, Isolation::Site), None);
    assert_eq!(start(&b, 200, 3, Isolation::Any), None);
}

#[test]
fn a_burst_is_min_burst_observations_in_the_window_and_begins_strictly_after_the_anchor() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    // The window is the rung's burst window, 400 ms: the first observation counts, an observation
    // exactly 400 ms after it counts, one 401 ms after it does not.
    let inside = tracked(
        &public,
        &[(1_000, site), (2_000, site), (2_200, site), (2_400, site)],
    );
    assert_eq!(start(&inside, 100, 3, Isolation::Site), Some(1));
    let outside = tracked(
        &public,
        &[(1_000, site), (2_000, site), (2_200, site), (2_401, site)],
    );
    assert_eq!(start(&outside, 100, 3, Isolation::Site), None);
    // The first observation that begins a burst, not the densest: two clusters, the first of three
    // and the second of five.
    let two = tracked(
        &public,
        &[
            (1_000, site),
            (2_000, site),
            (2_050, site),
            (2_100, site),
            (4_000, site),
            (4_010, site),
            (4_020, site),
            (4_030, site),
            (4_040, site),
        ],
    );
    assert_eq!(start(&two, 100, 3, Isolation::Site), Some(1));
    // With min_burst 4 the first cluster does not qualify and the second does.
    assert_eq!(start(&two, 100, 4, Isolation::Site), Some(4));
    // A "burst" at the anchor's own instant does not begin after it, under `site`, where the
    // anchor is isolated (the observations are at a dependent) and so only the burst's start is
    // in question.
    let same = tracked(
        &public,
        &[
            (1_000, site),
            (1_000, dependent),
            (1_000, dependent),
            (1_000, dependent),
        ],
    );
    assert_eq!(start(&same, 100, 3, Isolation::Site), None);
    // A burst that begins later but is itself in the window of an earlier observation: the earlier
    // observation (strictly later than the anchor) is the one that begins it.
    let chain = tracked(
        &public,
        &[
            (1_000, site),
            (1_500, dependent),
            (1_700, dependent),
            (1_900, dependent),
        ],
    );
    assert_eq!(start(&chain, 100, 3, Isolation::Site), Some(1));
    assert_eq!(
        start(&chain, 100, 4, Isolation::Site),
        None,
        "1.5 s to 1.9 s holds 3, not 4"
    );
}

#[test]
fn moving_the_anchor_drops_what_came_before_and_follows_the_new_anchors_service() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut a = tracked(
        &public,
        &[
            (1_000, site),
            (1_300, dependent),
            (1_310, dependent),
            (1_320, dependent),
        ],
    );
    let k = start(&a, 200, 3, Isolation::Site).unwrap();
    a.move_anchor_to(k, &public.services);
    assert_eq!(
        (a.anchor, a.anchor_at, a.site),
        (ObsId(1), at(1_300), dependent)
    );
    assert_eq!(a.attached.len(), 3);
    assert_eq!(
        a.attached[0].1,
        ObsId(1),
        "the anchor is the first attached observation"
    );
    assert_eq!(a.region, dependents_mask(&public.services, dependent));
    // Index 0 and an index past the end move nothing.
    let before = (a.anchor, a.attached.len());
    a.move_anchor_to(0, &public.services);
    a.move_anchor_to(99, &public.services);
    assert_eq!((a.anchor, a.attached.len()), before);
}

// ---- the failure it answers, through the rung

#[test]
fn a_stray_before_a_burst_is_the_rungs_anchor_and_not_this_noticers() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let mut obs = vec![stray(site, 9_500)];
    obs.extend(burst(site, 9_800));
    // The rung anchors on the stray: its window holds the whole burst.
    let mut rung = Feed::new(&public, NoticerSpec::default());
    let ids = rung.step(10 * S, &obs);
    assert_eq!(rung.anchors(), vec![(ids[0], at(9_500))]);
    // The later re-anchor moves it onto the burst's first observation, which is the notice's anchor,
    // and the site and the evidence follow.
    let mut re = Feed::new(&public, spec(200, 3, Isolation::Site));
    let ids = re.step(10 * S, &obs);
    assert_eq!(re.anchors(), vec![(ids[1], at(9_800))]);
    assert_eq!(re.sites(), vec![site]);
    let view = re.rung.views(at(10 * S)).remove(0);
    assert_eq!(
        (view.anchor, view.anchor_at, view.site),
        (ids[1], at(9_800), site)
    );
    let attached = re.rung.attached(view.id);
    assert_eq!(attached.len(), 5, "the burst, without the stray");
    assert_eq!(attached[0].1, ids[1]);
    assert_eq!(re.rung.noticer_id(), "reanchor");
    // The log has one notice per anomaly, as every noticer's does.
    assert_eq!(re.rung.noticed_total(), 1);
    assert_eq!(re.rung.notice_log().len(), 1);
}

#[test]
fn a_stray_closer_than_g_to_the_burst_cannot_be_told_from_its_start() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let mut obs = vec![stray(site, 9_700)];
    obs.extend(burst(site, 9_800));
    let mut re = Feed::new(&public, spec(200, 3, Isolation::Site));
    let ids = re.step(10 * S, &obs);
    assert_eq!(
        re.anchors(),
        vec![(ids[0], at(9_700))],
        "100 ms is inside g = 200 ms"
    );
    let mut finer = Feed::new(&public, spec(50, 3, Isolation::Site));
    let ids = finer.step(10 * S, &obs);
    assert_eq!(
        finer.anchors(),
        vec![(ids[1], at(9_800))],
        "but outside g = 50 ms"
    );
}

#[test]
fn a_burst_with_no_stray_is_anchored_as_the_rung_anchors_it() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let obs = burst(site, 9_800);
    let mut rung = Feed::new(&public, NoticerSpec::default());
    let mut re = Feed::new(&public, spec(200, 3, Isolation::Site));
    rung.step(10 * S, &obs);
    re.step(10 * S, &obs);
    assert_eq!(rung.anchors(), re.anchors());
    assert_eq!(rung.anchors()[0].1, at(9_800));
    let (a, b) = (rung.rung.views(at(10 * S)), re.rung.views(at(10 * S)));
    assert_eq!((a[0].anchor, a[0].digest), (b[0].anchor, b[0].digest));
    assert_eq!(rung.rung.attached(a[0].id), re.rung.attached(b[0].id));
    // The score a rule may read and the peak it has had are the rung's: a number above the
    // threshold now, the same number later, when the burst has left the score window and the
    // score has fallen below the peak.
    assert!(a[0].score >= 3.0, "{}", a[0].score);
    assert_eq!(a[0].score, b[0].score);
    assert_eq!(a[0].peak_score, b[0].peak_score);
    assert!(a[0].peak_score >= 3.0);
    let (a, b) = (rung.rung.views(at(20 * S)), re.rung.views(at(20 * S)));
    assert_eq!(a[0].score, b[0].score);
    assert_eq!(a[0].peak_score, b[0].peak_score);
    assert!(a[0].score < a[0].peak_score);
}

#[test]
fn a_stray_at_an_upstream_service_and_a_burst_at_its_dependent_move_to_the_dependent() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    // The dependent's observations within the stray's burst window attach to the stray's anomaly
    // as propagation, so the candidate is the stray's; the burst is the dependent's own.
    let mut obs = vec![stray(site, 9_500)];
    obs.extend(burst(dependent, 9_800));
    let mut rung = Feed::new(&public, NoticerSpec::default());
    let ids = rung.step(10 * S, &obs);
    assert_eq!(
        rung.anchors()[0].0,
        ids[0],
        "the rung keeps the stray as anchor"
    );
    assert_eq!(rung.sites(), vec![site]);
    let mut re = Feed::new(&public, spec(200, 3, Isolation::Site));
    let ids = re.step(10 * S, &obs);
    assert_eq!(re.anchors(), vec![(ids[1], at(9_800))]);
    assert_eq!(
        re.sites(),
        vec![dependent],
        "the new anchor's service is the site"
    );
}

#[test]
fn the_two_readings_of_isolated_differ_for_a_dependent_that_follows_the_anchor_at_once() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    // One observation at the site, the dependents' burst 100 ms later (a real incident whose site
    // speaks once, or a stray at the site before a dependent's incident: the two cannot be told
    // apart by the observations alone).
    let mut obs = vec![stray(site, 9_700)];
    obs.extend(burst(dependent, 9_800));
    let mut by_site = Feed::new(&public, spec(200, 3, Isolation::Site));
    let ids = by_site.step(10 * S, &obs);
    assert_eq!(
        by_site.anchors()[0].0,
        ids[1],
        "`site` moves the anchor to the dependents' burst"
    );
    let mut by_any = Feed::new(&public, spec(200, 3, Isolation::Any));
    let ids = by_any.step(10 * S, &obs);
    assert_eq!(
        by_any.anchors()[0].0,
        ids[0],
        "`any` does not: the dependent is within g"
    );
}

// ---- the identity with the rung

fn rung_with(noticer: NoticerSpec) -> RungConfig {
    RungConfig {
        noticer,
        ..RungConfig::default()
    }
}

#[test]
fn a_reanchor_noticer_that_never_moves_is_the_rung_in_whole_segments() {
    // `any` with a gap longer than the stream: no anchor is ever isolated (a candidate that crosses
    // has other attached observations), so nothing moves and the record is the rung's.
    let never = NoticerSpec::Reanchor {
        notice_z: None,
        gap_ns: u64::MAX / 4,
        min_burst: 2,
        isolation: Isolation::Any,
    };
    let spec = StreamPolicySpec::from_id("never_escalate").unwrap();
    for seed in [11, 12] {
        let p = params(seed, 200);
        let l = limits(&p);
        let rung = play_with_rung(&p, &spec, &l, &rung_with(NoticerSpec::default())).unwrap();
        let re = play_with_rung(&p, &spec, &l, &rung_with(never)).unwrap();
        assert_eq!(re.noticer, "reanchor");
        assert!(rung.anomalies_noticed > 0);
        assert_eq!(
            rung.notice_log
                .iter()
                .map(|e| (e.kind, e.anomaly, e.anchor, e.site, e.anchor_at, e.at))
                .collect::<Vec<_>>(),
            re.notice_log
                .iter()
                .map(|e| (e.kind, e.anomaly, e.anchor, e.site, e.anchor_at, e.at))
                .collect::<Vec<_>>(),
            "seed {seed}"
        );
        assert_eq!(rung.verdict, re.verdict, "seed {seed}");
        assert_eq!(rung.notices, re.notices, "seed {seed}");
    }
}

#[test]
fn a_reanchor_noticer_changes_only_the_anchors_the_rung_gives() {
    // On whole segments, with a gap that moves some anchors: one notice per anomaly the rung
    // reports, the evaluator scores them all, and nothing but the noticer's own record differs in
    // shape from the rung's.
    let moving = spec(100, 2, Isolation::Site);
    let spec_arm = StreamPolicySpec::from_id("never_escalate").unwrap();
    let mut moved = 0;
    for seed in [11, 12, 13] {
        let p = params(seed, 200);
        let l = limits(&p);
        let rung = play_with_rung(&p, &spec_arm, &l, &rung_with(NoticerSpec::default())).unwrap();
        let re = play_with_rung(&p, &spec_arm, &l, &rung_with(moving)).unwrap();
        let notices = re
            .notice_log
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .count() as u32;
        assert_eq!(notices, re.anomalies_noticed);
        assert_eq!(re.notices.totals.notices, notices);
        assert!(re.notice_log.iter().all(|e| e.noticer == "reanchor"));
        assert_eq!(re.observations, rung.observations);
        // The anchors differ for some notices.
        let a: Vec<_> = rung.notice_log.iter().map(|e| e.anchor).collect();
        let b: Vec<_> = re.notice_log.iter().map(|e| e.anchor).collect();
        moved += usize::from(a != b);
        assert!(re.notice_log.iter().all(|e| e.anchor_at <= e.at));
    }
    assert!(
        moved > 0,
        "a gap of 100 ms moves an anchor somewhere in three streams"
    );
}

// ---- the files

#[test]
fn the_run_writes_the_reanchor_noticers_record_and_replays_it_byte_for_byte() {
    let mut m = manifest(
        "reanchor-files",
        &[("a", "never_escalate"), ("b", "never_escalate")],
        2,
        200,
        9,
    );
    m.noticers
        .insert("b".to_owned(), spec(100, 2, Isolation::Site));
    m.validate().unwrap();
    let text = m.canonical_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["noticers"]["b"]["noticer"], "reanchor");
    assert_eq!(value["noticers"]["b"]["isolation"], "site");
    let back: StreamManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    let run = |name: &str| {
        let out = scratch(name).join("run");
        execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
        out
    };
    let (x, y) = (run("reanchor-replay-a"), run("reanchor-replay-b"));
    for arm in ["a", "b"] {
        for file in [
            "notices.csv",
            "notice_incidents.csv",
            "notice_events.csv",
            "results.csv",
        ] {
            assert_eq!(
                read(&x.join(arm), file),
                read(&y.join(arm), file),
                "{arm}/{file}"
            );
        }
    }
    let notices = read(&x.join("b"), "notices.csv");
    assert_eq!(cell(&notices, 0, "noticer"), "reanchor");
    // The arm with the default noticer is the rung's, in a run where another arm has one.
    assert_eq!(
        cell(&read(&x.join("a"), "notices.csv"), 0, "noticer"),
        "rung"
    );
}
