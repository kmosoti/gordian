//! The splitting noticer (work item B3): the rule as a pure function with every boundary of the
//! readings its module documentation states, the move of observations between anomalies, the
//! failure it answers through the rung, and its identity with the base when it cannot split.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::noticer::{BaseSpec, NoticeKind, NoticerSpec, Tracked};
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::noticer_split::{SplitSpec, split_picks};
use gordian_run::stream::arms::rung::{Held, Rung, RungConfig};
use gordian_stream::{ObsId, StreamEvent, StreamPublic};
use gordian_world::graph::dependents_mask;
use gordian_world::{CounterName, Observation, ServiceId};
use stream_common::*;

const MS: u64 = 1_000_000;
const S: u64 = 1_000;
/// The rung's burst window, 0.4 s.
const BURST_NS: u64 = 400 * MS;

fn counter(service: ServiceId, value: u64) -> Observation {
    Observation::Counter {
        service,
        name: CounterName::ErrorRate,
        value,
    }
}

fn spec(gap_ms: u64, min_burst: u32) -> SplitSpec {
    SplitSpec {
        gap_ns: gap_ms * MS,
        min_burst,
    }
}

fn composed(split: SplitSpec) -> NoticerSpec {
    NoticerSpec::Composed {
        base: BaseSpec::Rung { notice_z: None },
        ramp: None,
        split: Some(split),
    }
}

/// A site, a dependent of it, and a stranger.
fn picks3(public: &StreamPublic) -> (ServiceId, ServiceId, ServiceId) {
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

/// A tracked anomaly holding the observations `obs` (milliseconds, service), in order, the first
/// being the anchor and its service the site.
fn tracked(public: &StreamPublic, obs: &[(u64, ServiceId)]) -> Tracked {
    let held = |i: usize, ms: u64, s: ServiceId| Held {
        id: ObsId(i as u32),
        at: at(ms),
        obs: counter(s, 90),
        abnormal: true,
    };
    let (ms0, s0) = obs[0];
    let mut a = Tracked::new(0, &held(0, ms0, s0), s0, &public.services, 2_000 * MS);
    for (i, (ms, s)) in obs.iter().enumerate().skip(1) {
        a.note_attached(&held(i, *ms, *s), *s, 2_000 * MS);
    }
    a
}

fn picks(a: &Tracked, gap_ms: u64, min_burst: u32, now_ms: u64) -> Option<Vec<usize>> {
    split_picks(a, gap_ms * MS, min_burst, BURST_NS, at(now_ms))
}

/// Three observations at `s` from `t` ms, 10 ms apart.
fn burst3(s: ServiceId, t: u64) -> Vec<(u64, ServiceId)> {
    vec![(t, s), (t + 10, s), (t + 20, s)]
}

fn cat(parts: &[Vec<(u64, ServiceId)>]) -> Vec<(u64, ServiceId)> {
    parts.iter().flatten().copied().collect()
}

// ---- the spec

#[test]
fn the_split_parameters_are_checked_and_written() {
    assert!(spec(2_000, 2).validate().is_ok());
    assert!(spec(0, 2).validate().unwrap_err().contains("gap_ns"));
    assert!(spec(1, 1).validate().unwrap_err().contains("min_burst"));
    let value = serde_json::json!({
        "noticer": "composed",
        "base": {"noticer": "rung", "notice_z": 2.0},
        "split": {"gap_ns": 2_000_000_000u64, "min_burst": 3}
    });
    let parsed: NoticerSpec = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        parsed,
        NoticerSpec::Composed {
            base: BaseSpec::Rung {
                notice_z: Some(2.0)
            },
            ramp: None,
            split: Some(spec(2_000, 3)),
        }
    );
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    assert_eq!(parsed.id(), "split");
    assert!(
        serde_json::from_value::<NoticerSpec>(serde_json::json!({
            "noticer": "composed", "base": {"noticer": "rung"}, "split": {"gap_ns": 1, "min_burst": 2, "x": 1}
        }))
        .is_err()
    );
}

// ---- the rule

#[test]
fn a_later_burst_at_another_site_after_a_gap_is_split_and_it_is_the_burst_not_the_anomaly() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let a = tracked(&public, &cat(&[burst3(site, 1_000), burst3(dep, 5_000)]));
    // Indices 3, 4, 5: the later burst's observations.
    assert_eq!(picks(&a, 2_000, 3, 6_000), Some(vec![3, 4, 5]));
    // The stranger, which is not a dependent, is a foreign site as well.
}

#[test]
fn the_silence_is_measured_from_the_end_of_the_earlier_burst_and_must_be_more_than_the_gap() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    // The earlier burst ends at 1,020 ms and the later one begins at 5,000: 3,980 ms.
    let a = tracked(&public, &cat(&[burst3(site, 1_000), burst3(dep, 5_000)]));
    assert_eq!(picks(&a, 3_979, 3, 6_000), Some(vec![3, 4, 5]));
    assert_eq!(
        picks(&a, 3_980, 3, 6_000),
        None,
        "exactly the gap is not more than it"
    );
    assert_eq!(picks(&a, 3_981, 3, 6_000), None);
}

#[test]
fn heartbeats_between_the_bursts_do_not_shorten_the_gap_and_a_later_burst_at_the_site_is_not_split()
{
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    // A site observation every 1.5 s between the bursts: singleton clusters, not bursts.
    let a = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            vec![(2_500, site), (4_000, site)],
            burst3(dep, 5_000),
        ]),
    );
    assert_eq!(picks(&a, 2_000, 3, 6_000), Some(vec![5, 6, 7]));
    // The same bursts at the site itself are one anomaly speaking again.
    let b = tracked(&public, &cat(&[burst3(site, 1_000), burst3(site, 5_000)]));
    assert_eq!(picks(&b, 2_000, 3, 6_000), None);
}

#[test]
fn a_cluster_a_site_heartbeat_opens_is_split_by_its_foreign_observations_only() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    // The public attach rule opens a propagation window with any observation at the site, and
    // attaches a dependent's observations within 0.4 s of it: one cluster, whose first observation is
    // the site's and whose foreign observations are the dependent's. Indices 3 (the site's) stays.
    let a = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            vec![(5_000, site)],
            vec![(5_100, dep), (5_110, dep), (5_120, dep)],
        ]),
    );
    assert_eq!(picks(&a, 2_000, 3, 6_000), Some(vec![4, 5, 6]));
    // The first foreign observation is where the silence ends: 5,100 - 1,020 = 4,080 ms.
    assert_eq!(picks(&a, 4_079, 3, 6_000), Some(vec![4, 5, 6]));
    assert_eq!(picks(&a, 4_080, 3, 6_000), None);
}

#[test]
fn the_first_cluster_is_never_split_and_an_earlier_burst_is_needed() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    // A propagation inside the anomaly's own first burst is its own.
    let a = tracked(
        &public,
        &[(1_000, site), (1_010, dep), (1_020, dep), (1_030, dep)],
    );
    assert_eq!(picks(&a, 1, 2, 9_000), None);
    // A stray first (a cluster of one, not a burst at min_burst 2), then a burst elsewhere: there is
    // no earlier burst, so no split.
    let b = tracked(&public, &cat(&[vec![(1_000, site)], burst3(dep, 5_000)]));
    assert_eq!(picks(&b, 1_000, 2, 9_000), None);
    // With a burst of the site's before it the same later burst is split.
    let c = tracked(
        &public,
        &cat(&[vec![(1_000, site), (1_010, site)], burst3(dep, 5_000)]),
    );
    assert_eq!(picks(&c, 1_000, 2, 9_000), Some(vec![2, 3, 4]));
}

#[test]
fn a_burst_needs_min_burst_foreign_observations_and_the_earlier_one_min_burst_of_any() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let a = tracked(
        &public,
        &cat(&[burst3(site, 1_000), vec![(5_000, dep), (5_010, dep)]]),
    );
    assert_eq!(picks(&a, 2_000, 2, 9_000), Some(vec![3, 4]));
    assert_eq!(
        picks(&a, 2_000, 3, 9_000),
        None,
        "two foreign observations are not a burst of three"
    );
    // The earlier burst has three observations; at min_burst 4 it is not a burst either.
    let b = tracked(&public, &cat(&[burst3(site, 1_000), burst3(dep, 5_000)]));
    assert_eq!(picks(&b, 2_000, 3, 9_000), Some(vec![3, 4, 5]));
    assert_eq!(picks(&b, 2_000, 4, 9_000), None);
    // Foreign observations are counted, not the cluster's size: a cluster of a site observation and
    // two foreign ones is a burst of two at min_burst 2 and not 3.
    let c = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            vec![(5_000, site), (5_010, dep), (5_020, dep)],
        ]),
    );
    assert_eq!(picks(&c, 2_000, 2, 9_000), Some(vec![4, 5]));
    assert_eq!(picks(&c, 2_000, 3, 9_000), None);
}

#[test]
fn a_cluster_is_split_only_when_complete_at_the_instant_of_the_step() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let a = tracked(&public, &cat(&[burst3(site, 1_000), burst3(dep, 5_000)]));
    // The cluster's last observation is at 5,020 ms; it is complete once the burst window (400 ms)
    // has passed: strictly after 5,420 ms.
    assert_eq!(picks(&a, 2_000, 3, 5_420), None);
    assert_eq!(picks(&a, 2_000, 3, 5_421), Some(vec![3, 4, 5]));
    // A stricter example at the nanosecond: one nanosecond after is complete.
    let last = at(5_020).0;
    assert_eq!(
        split_picks(&a, 2_000 * MS, 3, BURST_NS, Instant(last + BURST_NS)),
        None
    );
    assert_eq!(
        split_picks(&a, 2_000 * MS, 3, BURST_NS, Instant(last + BURST_NS + 1)),
        Some(vec![3, 4, 5])
    );
}

#[test]
fn only_the_first_later_burst_is_returned_and_the_rest_wait_for_the_next_examination() {
    let public = public_of(&params(0, 150));
    let (site, dep, stranger) = picks3(&public);
    let a = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            burst3(dep, 5_000),
            burst3(stranger, 9_000),
        ]),
    );
    assert_eq!(picks(&a, 2_000, 3, 20_000), Some(vec![3, 4, 5]));
}

// ---- the move

#[test]
fn splitting_off_moves_the_observations_and_rebuilds_what_is_derived() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let mut a = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            vec![(5_000, site)],
            vec![(5_100, dep), (5_110, dep), (5_120, dep)],
            vec![(7_000, site)],
        ]),
    );
    let digest_before = a.digest();
    let b = a
        .split_off(&[4, 5, 6], 9, &public.services, 2_000 * MS)
        .expect("a valid split");
    // The new anomaly: anchored on the first moved, about its service, with its own region.
    assert_eq!(
        (b.id, b.anchor, b.anchor_at, b.site),
        (9, ObsId(4), at(5_100), dep)
    );
    assert_eq!(
        b.attached.iter().map(|(_, o, _)| o.0).collect::<Vec<_>>(),
        vec![4, 5, 6]
    );
    assert_eq!(b.region, dependents_mask(&public.services, dep));
    assert!(b.noticed_at.is_none());
    assert_eq!(b.last_abnormal_at, at(5_120));
    // The old one keeps the rest, in order, with the timing of what remains.
    assert_eq!(
        a.attached.iter().map(|(_, o, _)| o.0).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 7]
    );
    assert_eq!(a.anchor, ObsId(0));
    assert_eq!(a.last_abnormal_at, at(7_000));
    // Its digest, which counted the dependent's services, is rebuilt: it changed, and a fresh
    // anomaly with exactly its remaining observations has the same one.
    assert_ne!(a.digest(), digest_before);
    let fresh = tracked(
        &public,
        &cat(&[
            burst3(site, 1_000),
            vec![(5_000, site)],
            vec![(7_000, site)],
        ]),
    );
    assert_eq!(a.digest(), fresh.digest());
    // The moved anomaly's digest is that of an anomaly holding exactly its observations.
    let only = tracked(&public, &[(5_100, dep), (5_110, dep), (5_120, dep)]);
    assert_eq!(b.digest(), only.digest());
}

#[test]
fn a_split_that_cannot_be_made_moves_nothing() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let base = tracked(&public, &cat(&[burst3(site, 1_000), burst3(dep, 5_000)]));
    for bad in [
        vec![],
        vec![0, 3],
        vec![3, 3],
        vec![4, 3],
        vec![3, 4, 6],
        vec![9],
    ] {
        let mut a = base.clone();
        assert!(
            a.split_off(&bad, 1, &public.services, 2_000 * MS).is_none(),
            "{bad:?}"
        );
        assert_eq!(a.attached, base.attached, "{bad:?}");
    }
}

// ---- the failure it answers, through the rung

fn rung_with(noticer: NoticerSpec) -> RungConfig {
    RungConfig {
        noticer,
        ..RungConfig::default()
    }
}

struct Run {
    rung: Rung,
    next: u32,
}

impl Run {
    fn new(public: &StreamPublic, noticer: NoticerSpec) -> Self {
        Self {
            rung: Rung::new(public, rung_with(noticer)),
            next: 0,
        }
    }

    fn step(&mut self, now_ms: u64, observations: &[(u64, Observation)]) -> Vec<ObsId> {
        let mut ids = Vec::new();
        let events: Vec<StreamEvent> = observations
            .iter()
            .map(|(ms, obs)| {
                let id = ObsId(self.next);
                self.next += 1;
                ids.push(id);
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: obs.clone(),
                }
            })
            .collect();
        self.rung.absorb(&events, &[], at(now_ms));
        self.rung.notice(at(now_ms));
        ids
    }

    fn notices(&self) -> Vec<(ObsId, ServiceId, Instant)> {
        self.rung
            .notice_log()
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .map(|e| (e.anchor, e.site, e.at))
            .collect()
    }
}

/// An incident's burst at `site` at 10 s; a heartbeat of the site at 14 s opens a new burst window
/// there and six observations at a dependent follow within 0.4 s: the public attach rule gives them
/// to the anomaly of the first burst.
fn absorbed(site: ServiceId, dep: ServiceId) -> Vec<(u64, Observation)> {
    let mut obs: Vec<(u64, Observation)> = (0..5)
        .map(|i| (10_000 + 10 * i, counter(site, 80 + i)))
        .collect();
    obs.push((14_000, counter(site, 70)));
    obs.extend((0..6).map(|i| (14_050 + 40 * i, counter(dep, 85))));
    obs
}

fn play(run: &mut Run, obs: &[(u64, Observation)], until_ms: u64) {
    let mut now = 500;
    let mut i = 0;
    while now <= until_ms {
        let mut batch = Vec::new();
        while i < obs.len() && obs[i].0 <= now {
            batch.push(obs[i].clone());
            i += 1;
        }
        run.step(now, &batch);
        now += 500;
    }
}

#[test]
fn a_burst_the_attach_rule_gave_to_an_earlier_anomaly_is_noticed_in_its_own_right() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    let obs = absorbed(site, dep);
    // The rung alone: the dependent's observations are the first anomaly's, and there is one notice.
    let mut rung = Run::new(&public, NoticerSpec::default());
    play(&mut rung, &obs, 20 * S);
    assert_eq!(rung.notices().len(), 1);
    assert_eq!(rung.notices()[0].1, site);
    // The split noticer: the dependent's burst is its own anomaly, anchored on its first
    // observation (id 6: five at the site, the heartbeat, then the dependent's), noticed by the
    // base's rule at the step after the cluster is complete (14.25 s + 0.4 s, the step at 15 s).
    let mut split = Run::new(&public, composed(spec(2_000, 3)));
    play(&mut split, &obs, 20 * S);
    let notices = split.notices();
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert_eq!(notices[0].1, site);
    assert_eq!(notices[1], (ObsId(6), dep, at(15_000)));
    assert_eq!(split.rung.noticer_id(), "split");
    // The first anomaly no longer holds the dependent's observations.
    let views = split.rung.views(at(15_000));
    let first = views.iter().find(|v| v.site == site).unwrap();
    let held: Vec<u32> = split
        .rung
        .attached(first.id)
        .iter()
        .map(|(_, o, _)| o.0)
        .collect();
    assert_eq!(held, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn a_split_off_burst_that_is_too_small_to_be_noticed_stays_a_candidate() {
    let public = public_of(&params(0, 150));
    let (site, dep, _) = picks3(&public);
    // Two observations at the dependent after the heartbeat: a burst at min_burst 2, split off, and
    // not noticed (the base's score needs more): splitting does not notice.
    let mut obs = absorbed(site, dep);
    obs.truncate(7);
    obs.push((14_090, counter(dep, 85)));
    let mut split = Run::new(&public, composed(spec(2_000, 2)));
    play(&mut split, &obs, 20 * S);
    assert_eq!(split.notices().len(), 1, "the first anomaly's only");
    assert_eq!(split.rung.noticed_total(), 1);
    assert_eq!(
        split.rung.debug_anomalies().len(),
        2,
        "but there are two anomalies: {:?}",
        split.rung.debug_anomalies()
    );
}

// ---- the identity with the base

fn log_of(noticer: NoticerSpec, seed: u64) -> Vec<String> {
    let arm = gordian_run::stream::spec::StreamPolicySpec::from_id("never_escalate").unwrap();
    let p = params(seed, 200);
    let l = limits(&p);
    let rec = play_with_rung(&p, &arm, &l, &rung_with(noticer)).unwrap();
    assert!(rec.notice_log.iter().all(|e| e.anchor_at <= e.at));
    rec.notice_log
        .iter()
        .map(|e| {
            format!(
                "{:?} {} {:?} {:?} {:?} {:?}",
                e.kind, e.anomaly, e.anchor, e.site, e.anchor_at, e.at
            )
        })
        .collect()
}

#[test]
fn a_split_noticer_that_cannot_split_is_its_base_in_whole_segments() {
    // A gap longer than the stream: no silence is longer.
    let never = SplitSpec {
        gap_ns: u64::MAX / 4,
        min_burst: 2,
    };
    let reanchor = |split: Option<SplitSpec>| NoticerSpec::Composed {
        base: BaseSpec::Reanchor {
            notice_z: Some(2.0),
            gap_ns: 20 * MS,
            min_burst: 2,
            isolation: Isolation::Site,
        },
        ramp: None,
        split,
    };
    let plain_reanchor = NoticerSpec::Reanchor {
        notice_z: Some(2.0),
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    };
    for seed in [11, 12] {
        let base = log_of(NoticerSpec::default(), seed);
        assert!(!base.is_empty());
        assert_eq!(log_of(composed(never), seed), base, "seed {seed}");
        assert_eq!(
            log_of(reanchor(Some(never)), seed),
            log_of(plain_reanchor, seed),
            "seed {seed}, over the re-anchor"
        );
    }
}

#[test]
fn a_split_noticer_that_can_split_changes_the_record_somewhere_and_the_evaluator_accepts_it() {
    let loose = SplitSpec {
        gap_ns: 1_000 * MS,
        min_burst: 2,
    };
    let mut changed = 0;
    for seed in [11, 12, 13, 14] {
        changed +=
            usize::from(log_of(composed(loose), seed) != log_of(NoticerSpec::default(), seed));
    }
    assert!(
        changed > 0,
        "a loose split makes a split somewhere in four segments"
    );
}
