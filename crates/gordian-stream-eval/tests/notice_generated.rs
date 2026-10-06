//! `score_notices` against streams the generator makes (work item B2: the site check, rules N13 to
//! N16, on real truths).
//!
//! The hand-written cases pin the rules on hand-built truths, whose incidents carry the site a
//! fixture gives them. These tests check the one thing the fixtures cannot: that on generated
//! streams the incident's site (N13, the first service it occupies) is the site of its true
//! diagnosis for every plain and hard incident, so that a notice naming the diagnosis's site is
//! site-correct and one naming any other service is not. The expectations are stated from the
//! rules, not read back from the code under test.

use gordian_core::Instant;
use gordian_stream::oracle::{IncidentTruth, StreamTruth};
use gordian_stream::{ObsId, Stream, StreamParams, Tier, generate};
use gordian_stream_eval::{NoticeEntry, NoticeTrace, score_notices, truth_from_stream};

const SEEDS: u64 = 30;

fn streams() -> impl Iterator<Item = (u64, Stream)> {
    (0..SEEDS).map(|seed| (seed, generate(&StreamParams::new(seed))))
}

fn obs_at(stream: &Stream) -> Vec<Instant> {
    stream.events().iter().map(|(at, _)| *at).collect()
}

fn first(inc: &IncidentTruth) -> ObsId {
    *inc.observations
        .first()
        .unwrap_or_else(|| panic!("incident {} has no observation", inc.id))
}

/// One notice per incident, on its first observation, yielded at that observation's instant, about
/// the service `site_of` says.
fn one_per_incident(
    truth: &StreamTruth,
    at: &[Instant],
    site_of: impl Fn(&IncidentTruth) -> Option<u32>,
) -> NoticeTrace {
    let mut notices: Vec<NoticeEntry> = truth
        .incidents
        .iter()
        .map(|inc| {
            let anchor = first(inc);
            NoticeEntry {
                anomaly: inc.id,
                anchor,
                at: at[anchor.0 as usize],
                site: site_of(inc),
            }
        })
        .collect();
    notices.sort_by_key(|n| (n.at, n.anomaly));
    NoticeTrace {
        notices,
        retirements: Vec::new(),
    }
}

#[test]
fn the_first_occupied_service_is_the_site_of_the_true_diagnosis() {
    let (mut checked_plain, mut checked_hard, mut checked_decoy) = (0, 0, 0);
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        for inc in &truth.incidents {
            let occupied = inc
                .occupies
                .first()
                .unwrap_or_else(|| panic!("seed {seed}: incident {} occupies nothing", inc.id));
            match (inc.tier, inc.truth) {
                (Tier::Decoy, None) => checked_decoy += 1,
                (Tier::Plain, Some(h)) => {
                    assert_eq!(h.site, *occupied, "seed {seed}: plain incident {}", inc.id);
                    checked_plain += 1;
                }
                (Tier::Hard, Some(h)) => {
                    assert_eq!(h.site, *occupied, "seed {seed}: hard incident {}", inc.id);
                    checked_hard += 1;
                }
                (tier, truth) => panic!("seed {seed}: {tier:?} incident with truth {truth:?}"),
            }
        }
    }
    // The assertions above would pass vacuously on streams without these.
    assert!(
        checked_plain > 0 && checked_hard > 0 && checked_decoy > 0,
        "{checked_plain} {checked_hard} {checked_decoy}"
    );
}

#[test]
fn a_notice_about_the_incidents_site_on_its_first_observation_is_correct_on_every_tier() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let at = obs_at(&stream);
        let trace = one_per_incident(&truth, &at, |inc| inc.occupies.first().map(|s| s.0));
        let v = score_notices(&truth, &at, &trace).expect("scores");
        let t = &v.totals;
        let n = truth.incidents.len() as u32;
        assert_eq!(t.notices, n, "seed {seed}");
        assert_eq!(t.notices_site_correct, n, "seed {seed}");
        assert_eq!(t.notices_anchor_site_correct, n, "seed {seed}");
        assert_eq!(t.on_background, 0, "seed {seed}");
        assert_eq!(t.precision(), (n > 0).then_some(1.0), "seed {seed}");
        assert_eq!(t.strict_precision(), (n > 0).then_some(1.0), "seed {seed}");
        for i in &v.per_incident {
            assert!(
                i.noticed && i.anchor_correct && i.site_correct && i.anchor_site_correct,
                "seed {seed}: incident {}",
                i.id
            );
        }
    }
}

#[test]
fn a_notice_about_any_other_service_is_anchor_correct_and_never_site_correct() {
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let at = obs_at(&stream);
        let services = stream.public_info().services.len() as u32;
        // The service after the site, wrapping: always another one.
        let trace = one_per_incident(&truth, &at, |inc| {
            inc.occupies.first().map(|s| (s.0 + 1) % services)
        });
        let v = score_notices(&truth, &at, &trace).expect("scores");
        let t = &v.totals;
        assert_eq!(t.notices_site_correct, 0, "seed {seed}");
        assert_eq!(t.notices_anchor_site_correct, 0, "seed {seed}");
        assert_eq!(t.strict_precision().unwrap_or(0.0), 0.0, "seed {seed}");
        // The anchors are the incidents' first observations: the anchor check still passes.
        assert_eq!(
            t.anchor_correct.plain + t.anchor_correct.hard + t.anchor_correct.decoy,
            truth.incidents.len() as u32,
            "seed {seed}"
        );
    }
}

#[test]
fn a_notice_on_every_observation_has_low_precision_however_often_it_is_right() {
    // The flood of B1's finding, in miniature: a notice on every observation, about the observation's
    // own incident's site where it has one, and about service 0 otherwise. Every incident is noticed
    // with an anchor-correct, site-correct notice, and precision is the share of observations that
    // belong to an incident, which is below one on a stream with background.
    let mut saw_background = false;
    for (seed, stream) in streams() {
        let truth = truth_from_stream(&stream);
        let at = obs_at(&stream);
        let notices: Vec<NoticeEntry> = (0..truth.labels.len())
            .map(|i| {
                let anchor = ObsId(i as u32);
                let site = truth
                    .incident_of(anchor)
                    .and_then(|id| truth.incidents[id as usize].occupies.first().map(|s| s.0))
                    .or(Some(0));
                NoticeEntry {
                    anomaly: i as u32,
                    anchor,
                    at: at[i],
                    site,
                }
            })
            .collect();
        let trace = NoticeTrace {
            notices,
            retirements: Vec::new(),
        };
        let v = score_notices(&truth, &at, &trace).expect("scores");
        let t = &v.totals;
        let on_incidents = t.on_incidents();
        assert_eq!(on_incidents, t.notices - t.on_background, "seed {seed}");
        saw_background |= t.on_background > 0;
        assert_eq!(
            t.precision(),
            Some(f64::from(on_incidents) / f64::from(t.notices)),
            "seed {seed}"
        );
        // Every notice on an incident names that incident's site: all of them are site-correct.
        assert_eq!(t.notices_site_correct, on_incidents, "seed {seed}");
        assert!(
            t.strict_precision().unwrap() <= t.precision().unwrap(),
            "seed {seed}"
        );
        for i in &v.per_incident {
            assert!(
                i.noticed && i.anchor_site_correct,
                "seed {seed}: incident {}",
                i.id
            );
        }
    }
    assert!(saw_background, "the streams have background observations");
}
