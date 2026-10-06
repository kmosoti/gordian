//! The medium as a noticer (work item M2): the sense, clock, effector and ledger adapters, the
//! charge on the bill, the graph, and that nothing of it touches an arm that does not use it.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`scripts/m2_gate.py`); the tests here pin the same property on small runs.

mod stream_common;

use gordian_core::{Instant, Phase, Resource};
use gordian_medium::{Prices, Tag};
use gordian_run::stream::arms::medium::adapters::{
    ABNORMAL_KIND, CH_COUNTER, CH_MESSAGE, CH_SNAPSHOT, TAG_ABNORMAL, TAG_BENIGN, TICK_PRICE_NS,
    TickClock, abnormal_kind_tag, counter_tag, encode, message_tag, severity_tag,
};
use gordian_run::stream::arms::medium::{
    CoincidenceForm, Confirm, MEDIUM_COMPONENT, MEDIUM_ID, MediumNoticer, MediumParams,
};
use gordian_run::stream::arms::noticer::{Notice, NoticeKind, Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::execute_stream;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamParams, StreamPublic, generate};
use gordian_world::physics::{CATALOGUE_LIMIT, HIGH, SignalText};
use gordian_world::{CounterName, Observation, Probe, ProbeKind, ProbeResult, ServiceId, Severity};
use serde_json::{Value, json};
use std::fmt::Write as _;
use stream_common::*;

const MS: u64 = 1_000_000;

fn counter(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

fn message(service: u32, text: SignalText) -> Observation {
    Observation::Message {
        service: ServiceId(service),
        text_id: text.text_id(),
        severity: Severity::High,
    }
}

fn held(id: u32, at_ms: u64, obs: Observation, public: &StreamPublic) -> Held {
    Held {
        id: ObsId(id),
        at: Instant(at_ms * MS),
        abnormal: is_abnormal(&obs, &public.services),
        obs,
    }
}

/// The public graph of a test stream.
fn public() -> StreamPublic {
    public_of(&params(0, 150))
}

/// A service with at least one dependent, one of its dependents, and a service that is neither.
fn site_and_dependent(public: &StreamPublic) -> (u32, u32) {
    use gordian_world::graph::dependents_mask;
    for s in 0..public.services.len() {
        let mask = dependents_mask(&public.services, ServiceId(s as u32));
        if let Some(d) = (0..mask.len()).find(|i| *i != s && mask[*i]) {
            return (s as u32, d as u32);
        }
    }
    panic!("no service has a dependent");
}

/// Drives a noticer as the rung does: at each step (every `step_ms`), the observations emitted
/// by then are delivered (abnormal ones offered to `observe`), stored, and `notice` is called.
struct Drive {
    noticer: MediumNoticer,
    items: Vec<Held>,
    notices: Vec<(u64, Notice)>,
    retirable: Vec<(u64, Vec<u32>)>,
}

impl Drive {
    fn new(params: MediumParams, public: &StreamPublic) -> Self {
        Self {
            noticer: MediumNoticer::new(params, RungConfig::default(), &public.services).unwrap(),
            items: Vec::new(),
            notices: Vec::new(),
            retirable: Vec::new(),
        }
    }

    /// Play `observations` (instant in ms, observation; in time order) up to `until_ms`.
    fn play(&mut self, observations: &[(u64, Observation)], until_ms: u64, public: &StreamPublic) {
        let all: Vec<Held> = observations
            .iter()
            .enumerate()
            .map(|(i, (ms, o))| held(i as u32, *ms, o.clone(), public))
            .collect();
        let mut next = 0;
        let mut now = 0;
        while now <= until_ms {
            while next < all.len() && all[next].at.0 <= now * MS {
                if all[next].abnormal {
                    self.noticer.observe(&all[next]);
                }
                self.items.push(all[next].clone());
                next += 1;
            }
            let store = Store::with(self.items.clone());
            for n in self.noticer.notice(Instant(now * MS), &store) {
                self.notices.push((now, n));
            }
            let r = self.noticer.retirable(Instant(now * MS));
            if !r.is_empty() {
                for id in &r {
                    self.noticer.retire(*id);
                }
                self.retirable.push((now, r));
            }
            now += 500;
        }
    }
}

/// Parameters that isolate one path: the onset path only, at a 100 ms tick.
fn onset_only() -> MediumParams {
    MediumParams {
        ramp: false,
        ..MediumParams::default()
    }
}

// ---- the sense adapter

#[test]
fn every_observation_becomes_an_event_with_its_value_benign_readings_included() {
    let p = public();
    let tick = 100 * MS;
    let benign = held(7, 1_234, counter(2, CounterName::Saturation, 31), &p);
    let e = encode(&benign, tick).unwrap();
    assert_eq!(e.value, 31.0);
    assert_eq!(e.tick, 12);
    assert_eq!(e.offset_ns, (34 * MS) as u32);
    assert_eq!(e.seq, 7);
    assert_eq!(
        (e.source.domain, e.source.node, e.source.channel),
        (0, 2, CH_COUNTER)
    );
    assert_eq!(
        e.tags,
        vec![TAG_BENIGN, counter_tag(CounterName::Saturation)]
    );

    let alarm = held(8, 1_250, counter(2, CounterName::ErrorRate, HIGH), &p);
    let e = encode(&alarm, tick).unwrap();
    assert_eq!(e.value, HIGH as f32);
    assert_eq!(e.tags[0], TAG_ABNORMAL);
    // An abnormal reading carries its kind; a benign one does not.
    assert_eq!(e.tags[2], abnormal_kind_tag(&alarm.obs));
    assert_eq!(e.tags[2], Tag(ABNORMAL_KIND));

    let m = held(9, 2_000, message(1, SignalText::OutOfResource), &p);
    let e = encode(&m, tick).unwrap();
    assert_eq!(e.source.channel, CH_MESSAGE);
    assert_eq!(e.value, 1.0);
    assert_eq!(
        e.tags,
        vec![
            TAG_ABNORMAL,
            message_tag(SignalText::OutOfResource.text_id()),
            severity_tag(Severity::High),
            Tag(ABNORMAL_KIND + 5)
        ]
    );
    let health = held(10, 2_000, message(1, SignalText::CheckHealth), &p);
    assert_eq!(encode(&health, tick).unwrap().tags[0], TAG_BENIGN);

    let snap = Observation::Snapshot {
        service: ServiceId(0),
        config_hash: p.services[0].config_hash,
    };
    let e = encode(&held(11, 0, snap, &p), tick).unwrap();
    assert_eq!((e.source.channel, e.value), (CH_SNAPSHOT, 1.0));
    assert_eq!(e.tags, vec![TAG_BENIGN]);

    // Probe results and corrections are not about a service and never reach a noticer.
    let probed = Observation::Probed {
        probe: Probe {
            kind: ProbeKind::HealthCheck,
            target: ServiceId(0),
        },
        result: ProbeResult::Negative,
    };
    assert!(encode(&held(12, 0, probed, &p), tick).is_none());
}

#[test]
fn the_offset_keeps_order_inside_a_long_tick() {
    let p = public();
    let a = encode(
        &held(0, 4_020, counter(0, CounterName::ErrorRate, 80), &p),
        2_000 * MS,
    );
    let b = encode(
        &held(1, 4_150, counter(1, CounterName::ErrorRate, 80), &p),
        2_000 * MS,
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!((a.tick, b.tick), (2, 2));
    assert!(a.reference() < b.reference());
    assert_eq!(b.offset_ns - a.offset_ns, (130 * MS) as u32);
}

#[test]
fn message_ids_fold_by_the_stated_rule() {
    // A catalogue id is exact and distinct from every other tag.
    for t in SignalText::ALL {
        assert_eq!(
            message_tag(t.text_id()),
            Tag(0x4000_0000 | t.text_id() as u32)
        );
    }
    assert_eq!(message_tag(CATALOGUE_LIMIT - 1).0, 0x4000_0000 | 0xFFFF);
    // A free-form id is the exclusive-or of its halves, folded to 31 bits, with the top bit set.
    let id: u64 = 0x1234_5678_9ABC_DEF0;
    assert_eq!(
        message_tag(id).0,
        0x8000_0000 | ((0x9ABC_DEF0u32 ^ 0x1234_5678) & 0x7FFF_FFFF)
    );
    // Two free-form ids with equal folds collide, and that is the stated rule.
    let a: u64 = (1 << 32) | 0x0001_0000;
    let b: u64 = 0x0001_0001;
    assert_ne!(a, b);
    assert_eq!(message_tag(a), message_tag(b));
    // A free-form id never collides with a catalogue id.
    assert_ne!(message_tag(CATALOGUE_LIMIT).0 & 0xC000_0000, 0x4000_0000);
}

// ---- the clock adapter

#[test]
fn only_complete_ticks_run() {
    assert_eq!(TickClock::complete_before(100 * MS, Instant(0)), 0);
    assert_eq!(TickClock::complete_before(100 * MS, Instant(99 * MS)), 0);
    assert_eq!(TickClock::complete_before(100 * MS, Instant(100 * MS)), 1);
    assert_eq!(TickClock::complete_before(500 * MS, Instant(1_000 * MS)), 2);
    assert_eq!(
        TickClock::complete_before(2_000 * MS, Instant(3_999 * MS)),
        1
    );
    // Every tick runs, empty ones included, and each is billed.
    let p = public();
    let mut d = Drive::new(onset_only(), &p);
    d.play(&[], 2_000, &p);
    assert_eq!(d.noticer.ledger().total_ticks, 20);
    assert_eq!(d.noticer.total_ns(), 20 * TICK_PRICE_NS);
}

#[test]
fn benign_readings_reach_the_medium_and_the_control_drops_them() {
    let p = public();
    let obs: Vec<(u64, Observation)> = (0..10)
        .map(|i| (100 + 300 * i, counter(1, CounterName::Latency, 10 + i)))
        .collect();
    let mut all = Drive::new(MediumParams::default(), &p);
    all.play(&obs, 4_000, &p);
    // Two counter sense cells (presence, value) match each reading.
    assert_eq!(all.noticer.ledger().total_counts.event_routings, 20);
    let mut control = Drive::new(
        MediumParams {
            abnormal_only: true,
            ..MediumParams::default()
        },
        &p,
    );
    control.play(&obs, 4_000, &p);
    assert_eq!(control.noticer.ledger().total_counts.event_routings, 0);
}

// ---- the effector adapter: notices, anchors, retirements

/// A burst at `site`: four abnormal observations within 40 ms.
fn burst(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 90)),
        (t + 20, message(site, SignalText::OutOfResource)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
    ]
}

#[test]
fn a_burst_is_noticed_and_anchored_on_it_not_on_a_stray_before_it() {
    let p = public();
    let (site, _) = site_and_dependent(&p);
    let mut obs = vec![(1_000, counter(site, CounterName::ErrorRate, 70))];
    obs.extend(burst(site, 1_600));
    let mut d = Drive::new(onset_only(), &p);
    d.play(&obs, 4_000, &p);
    assert_eq!(d.notices.len(), 1, "{:?}", d.notices);
    let (at, n) = &d.notices[0];
    // The stray is observation 0, the burst 1 to 4. Lookback 200 ms (two ticks) excludes the
    // stray, 600 ms earlier; the anchor is the burst's first observation.
    assert_eq!(n.anchor, ObsId(1));
    assert_eq!(n.anchor_at, Instant(1_600 * MS));
    assert_eq!(n.site, ServiceId(site));
    assert_eq!(n.attached[0], n.anchor);
    assert!(n.attached.iter().all(|o| o.0 >= 1));
    // Noticed at the first step after the tick that crossed (the burst is in tick 16, complete
    // at 1,700 ms).
    assert_eq!(*at, 2_000);

    // With a lookback long enough to reach the stray, the anchor is the stray: the rule is the
    // earliest event in the support, and the lookback is what keeps a stray out of it.
    let mut long = Drive::new(
        MediumParams {
            lookback_ns: 1_000 * MS,
            onset_tau_ns: 2_000 * MS,
            ..onset_only()
        },
        &p,
    );
    long.play(&obs, 4_000, &p);
    assert_eq!(long.notices[0].1.anchor, ObsId(0));
}

#[test]
fn a_lone_stray_is_not_noticed_and_later_alarms_join_the_noticed_anomaly() {
    let p = public();
    let (site, _) = site_and_dependent(&p);
    let mut d = Drive::new(onset_only(), &p);
    let mut obs = vec![(500, counter(site, CounterName::ErrorRate, 70))];
    obs.extend(burst(site, 3_000));
    obs.push((5_000, counter(site, CounterName::ErrorRate, 77)));
    d.play(&obs, 6_000, &p);
    assert_eq!(d.notices.len(), 1);
    let id = d.notices[0].1.id;
    let a = d.noticer.tracked(id).unwrap();
    assert!(a.owns(ObsId(5)), "the later alarm joined: {:?}", a.attached);
    assert!(!a.owns(ObsId(0)));
}

#[test]
fn a_noticed_anomaly_retires_when_its_service_has_been_quiet_for_the_hold() {
    let p = public();
    let (site, _) = site_and_dependent(&p);
    let mut d = Drive::new(onset_only(), &p);
    let mut obs = burst(site, 1_000);
    // An alarm at the site while the hold is open keeps it open.
    obs.push((4_000, counter(site, CounterName::ErrorRate, 90)));
    d.play(&obs, 14_000, &p);
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.retirable.len(), 1);
    let (at, ids) = &d.retirable[0];
    assert_eq!(ids, &vec![d.notices[0].1.id]);
    // The last alarm at 4,000 ms (tick 40) re-opened the 6 s hold: 60 ticks after it, the latch
    // sees the hold expire at tick 101 and proposes, noticed at the next step.
    assert!(*at >= 10_000 && *at <= 10_500, "retired at {at}");
}

/// M3: with `ramp_inhibit`, a ramp at a service where an anomaly is open is not noticed again
/// while the anomaly's hold lasts; a ramp alone is noticed as before, anchored as before.
#[test]
fn an_open_anomaly_silences_the_ramp_at_its_service() {
    let p = public();
    let ramp: Vec<(u64, Observation)> = (0..8)
        .map(|i| {
            (
                2_000 + 1_200 * i,
                counter(3, CounterName::Saturation, 22 + 4 * i),
            )
        })
        .collect();
    let inhibit = MediumParams {
        ramp_inhibit: true,
        ..MediumParams::default()
    };
    inhibit.validate().unwrap();
    // A ramp alone: the same notice with and without the inhibition.
    let mut a = Drive::new(MediumParams::default(), &p);
    a.play(&ramp, 14_000, &p);
    let mut b = Drive::new(inhibit, &p);
    b.play(&ramp, 14_000, &p);
    assert!(!a.notices.is_empty());
    assert_eq!(
        a.notices.iter().map(|(_, n)| n.anchor).collect::<Vec<_>>(),
        b.notices.iter().map(|(_, n)| n.anchor).collect::<Vec<_>>()
    );
    // A burst at the same service first (noticed at about 1.5 s, held for 6 s): without the
    // inhibition the ramp is noticed a second time while the burst's anomaly is open; with it,
    // not before the hold has expired.
    let mut obs = burst(3, 1_000);
    obs.extend(ramp.iter().cloned());
    obs.sort_by_key(|(t, _)| *t);
    let ramp_notices_before = |d: &Drive, ms: u64| {
        d.notices
            .iter()
            .filter(|(at, n)| *at < ms && n.anchor.0 >= 4)
            .count()
    };
    let mut a = Drive::new(MediumParams::default(), &p);
    a.play(&obs, 14_000, &p);
    assert_eq!(ramp_notices_before(&a, 7_000), 1, "{:?}", a.notices);
    let mut b = Drive::new(inhibit, &p);
    b.play(&obs, 14_000, &p);
    assert_eq!(ramp_notices_before(&b, 7_000), 0, "{:?}", b.notices);
    // The burst itself is noticed either way.
    assert!(b.notices.iter().any(|(_, n)| n.anchor == ObsId(0)));
}

#[test]
fn a_ramp_of_benign_readings_is_noticed_and_the_control_cannot_see_it() {
    let p = public();
    // A counter that reads every 1.2 s and climbs by 4: benign until it reaches 50.
    let ramp: Vec<(u64, Observation)> = (0..8)
        .map(|i| {
            (
                2_000 + 1_200 * i,
                counter(3, CounterName::Saturation, 22 + 4 * i),
            )
        })
        .collect();
    let mut d = Drive::new(MediumParams::default(), &p);
    d.play(&ramp, 14_000, &p);
    assert!(!d.notices.is_empty());
    let n = &d.notices[0].1;
    assert_eq!(n.site, ServiceId(3));
    // Its anchor is a reading of the ramp below the alarm line (the eighth reading is 50): the
    // first, since the lookback (3 s) reaches it when the third reading crosses.
    assert!((n.anchor.0 as usize) < 7);
    assert_eq!(n.anchor, ObsId(0));
    let mut control = Drive::new(
        MediumParams {
            abnormal_only: true,
            ..MediumParams::default()
        },
        &p,
    );
    control.play(&ramp, 14_000, &p);
    assert!(control.notices.is_empty());
    // Readings far apart, each drawn afresh, are not a ramp.
    let noise: Vec<(u64, Observation)> = [12, 40, 3, 27, 44, 9, 31, 18]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                2_000 + 1_200 * i as u64,
                counter(3, CounterName::Saturation, *v),
            )
        })
        .collect();
    let mut d = Drive::new(MediumParams::default(), &p);
    d.play(&noise, 14_000, &p);
    assert!(d.notices.is_empty(), "{:?}", d.notices);
}

#[test]
fn every_coincidence_form_builds_and_propagation_can_notice() {
    let p = public();
    let (site, dep) = site_and_dependent(&p);
    // One alarm at the site and one at its dependent 60 ms later: not three at one place.
    let obs = vec![
        (1_000, counter(site, CounterName::ErrorRate, 80)),
        (1_060, counter(dep, CounterName::ErrorRate, 80)),
    ];
    for form in [
        CoincidenceForm::Sliding,
        CoincidenceForm::Binned,
        CoincidenceForm::Ordered,
    ] {
        let params = MediumParams {
            coincidence: form,
            propagation: true,
            rhythms: form == CoincidenceForm::Binned,
            onset_threshold: 5.0,
            ..onset_only()
        };
        params.validate().unwrap();
        let mut d = Drive::new(params, &p);
        d.play(&obs, 3_000, &p);
        assert_eq!(d.notices.len(), 1, "{form:?}");
        assert_eq!(d.notices[0].1.anchor, ObsId(0), "{form:?}");
    }
    // Ordered: the dependent first is not propagation.
    let reversed = vec![
        (1_000, counter(dep, CounterName::ErrorRate, 80)),
        (1_060, counter(site, CounterName::ErrorRate, 80)),
    ];
    let mut d = Drive::new(
        MediumParams {
            coincidence: CoincidenceForm::Ordered,
            propagation: true,
            onset_threshold: 5.0,
            ..onset_only()
        },
        &p,
    );
    d.play(&reversed, 3_000, &p);
    assert!(d.notices.iter().all(|(_, n)| n.site != ServiceId(site)));
}

#[test]
fn a_burst_is_two_kinds_within_the_window_read_from_the_offsets() {
    let p = public();
    let (site, _) = site_and_dependent(&p);
    let burst_only = |tick_ms: u64, form: CoincidenceForm| MediumParams {
        tick_ns: tick_ms * MS,
        onset: false,
        ramp: false,
        burst: true,
        coincidence: form,
        burst_window_ns: 25 * MS,
        ..MediumParams::default()
    };
    // A stray 160 ms before; then an error rate and a message 12 ms apart.
    let tight = vec![
        (2_150, counter(site, CounterName::Latency, 70)),
        (2_310, counter(site, CounterName::ErrorRate, 80)),
        (2_322, message(site, SignalText::OutOfResource)),
    ];
    // Two kinds 60 ms apart; two error-rate alarms 5 ms apart (one kind).
    let loose = vec![
        (2_010, counter(site, CounterName::ErrorRate, 80)),
        (2_070, message(site, SignalText::OutOfResource)),
    ];
    let same = vec![
        (2_010, counter(site, CounterName::ErrorRate, 80)),
        (2_015, counter(site, CounterName::ErrorRate, 90)),
    ];
    for tick in [100, 500, 2_000] {
        let mut d = Drive::new(burst_only(tick, CoincidenceForm::Ordered), &p);
        d.play(&tight, 6_000, &p);
        assert_eq!(d.notices.len(), 1, "tick {tick}");
        let anchor = d.notices[0].1.anchor;
        // At 100 ms the stray is in an earlier tick and pruned (lookback 0); at 500 ms and 2 s it
        // shares the burst's tick and is the earliest event cited: the rule's limit, stated.
        if tick == 100 {
            assert_eq!(anchor, ObsId(1));
        } else {
            assert_eq!(anchor, ObsId(0), "tick {tick}");
        }
        for (name, obs) in [("loose", &loose), ("same", &same)] {
            let mut d = Drive::new(burst_only(tick, CoincidenceForm::Ordered), &p);
            d.play(obs, 6_000, &p);
            assert!(d.notices.is_empty(), "{name} at tick {tick}");
        }
    }
    // The sliding form counts ticks, not offsets: two kinds 60 ms apart in one 500 ms tick are a
    // burst for it.
    let mut d = Drive::new(burst_only(500, CoincidenceForm::Sliding), &p);
    d.play(&loose, 6_000, &p);
    assert_eq!(d.notices.len(), 1);
}

/// M3: the burst's sub-tick lookback keeps a stray that shares the burst's tick out of the
/// anchor at every tick length, and arrivals at event resolution see a burst behind a stray of
/// one of its own kinds.
#[test]
fn with_a_sub_tick_lookback_a_stray_in_the_bursts_tick_is_not_the_anchor() {
    let p = public();
    let (site, _) = site_and_dependent(&p);
    let params = |tick_ms: u64, sub_ms: u64, every: bool| MediumParams {
        tick_ns: tick_ms * MS,
        onset: false,
        ramp: false,
        burst: true,
        coincidence: CoincidenceForm::Ordered,
        burst_window_ns: 25 * MS,
        burst_subtick_ns: sub_ms * MS,
        burst_every_event: every,
        ..MediumParams::default()
    };
    // A latency stray 160 ms before an error rate and a message 12 ms apart (as above).
    let tight = vec![
        (2_150, counter(site, CounterName::Latency, 70)),
        (2_310, counter(site, CounterName::ErrorRate, 80)),
        (2_322, message(site, SignalText::OutOfResource)),
    ];
    for tick in [100, 500, 2_000] {
        let mut d = Drive::new(params(tick, 25, false), &p);
        d.play(&tight, 6_000, &p);
        assert_eq!(d.notices.len(), 1, "tick {tick}");
        assert_eq!(d.notices[0].1.anchor, ObsId(1), "tick {tick}");
        assert!(!d.notices[0].1.attached.contains(&ObsId(0)));
    }
    // A stray of the burst's own kind (an error rate) 160 ms before it, in the same 2 s tick:
    // M1b's arrival stands at the stray, so no burst is seen in that tick; at event resolution
    // the burst is seen and anchored on its first event.
    let same_kind = vec![
        (2_150, counter(site, CounterName::ErrorRate, 70)),
        (2_310, counter(site, CounterName::ErrorRate, 80)),
        (2_322, message(site, SignalText::OutOfResource)),
    ];
    let mut d = Drive::new(params(2_000, 25, false), &p);
    d.play(&same_kind, 6_000, &p);
    assert!(d.notices.is_empty(), "{:?}", d.notices);
    let mut d = Drive::new(params(2_000, 25, true), &p);
    d.play(&same_kind, 6_000, &p);
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.notices[0].1.anchor, ObsId(1));
    // Arrivals at event resolution need the ordered form.
    let sliding = MediumParams {
        coincidence: CoincidenceForm::Sliding,
        ..params(500, 0, true)
    };
    assert!(sliding.validate().is_err());
}

/// M3: the confirmation read in event time works inside a 2 s tick, where a latch's hold cannot
/// be shorter than the tick.
#[test]
fn a_burst_is_confirmed_by_an_alarm_within_a_window_in_event_time() {
    let p = public();
    let (site, dep) = site_and_dependent(&p);
    let params = |lead: bool| MediumParams {
        tick_ns: 2_000 * MS,
        onset: false,
        ramp: false,
        burst: true,
        coincidence: CoincidenceForm::Ordered,
        burst_window_ns: 25 * MS,
        burst_subtick_ns: 25 * MS,
        burst_every_event: true,
        burst_confirm: Confirm::Dependents,
        confirm_window_ns: 100 * MS,
        confirm_lead: lead,
        ..MediumParams::default()
    };
    params(true).validate().unwrap();
    let two = vec![
        (2_310, counter(site, CounterName::ErrorRate, 80)),
        (2_322, message(site, SignalText::OutOfResource)),
    ];
    let with = |extra: (u64, Observation)| {
        let mut v = two.clone();
        v.push(extra);
        v.sort_by_key(|(t, _)| *t);
        v
    };
    // Alone: nothing.
    let mut d = Drive::new(params(true), &p);
    d.play(&two, 8_000, &p);
    assert!(d.notices.is_empty(), "{:?}", d.notices);
    // A dependent's alarm 40 ms after: one notice, anchored on the burst at the site.
    let mut d = Drive::new(params(true), &p);
    d.play(
        &with((2_362, counter(dep, CounterName::ErrorRate, 80))),
        8_000,
        &p,
    );
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.notices[0].1.anchor, ObsId(0));
    assert_eq!(d.notices[0].1.site, ServiceId(site));
    // 300 ms after: outside the window, nothing (a latch would hold for the whole 2 s tick).
    let mut d = Drive::new(params(true), &p);
    d.play(
        &with((2_622, counter(dep, CounterName::ErrorRate, 80))),
        8_000,
        &p,
    );
    assert!(d.notices.is_empty(), "{:?}", d.notices);
    // 30 ms before the burst: with the lead, the burst must come first, nothing; without it, the
    // notice cites the earlier alarm, which becomes the anchor (why the lead exists).
    let before = with((2_280, counter(dep, CounterName::ErrorRate, 80)));
    let mut d = Drive::new(params(true), &p);
    d.play(&before, 8_000, &p);
    assert!(d.notices.is_empty(), "{:?}", d.notices);
    let mut d = Drive::new(params(false), &p);
    d.play(&before, 8_000, &p);
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.notices[0].1.anchor, ObsId(0));
    assert_eq!(d.notices[0].1.site, ServiceId(dep));
    // It needs a confirmation to read.
    let bad = MediumParams {
        burst_confirm: Confirm::None,
        ..params(true)
    };
    assert!(bad.validate().is_err());
}

/// M3: with the cluster merge, a burst at a dependent just after the site's burst repeats the
/// site's anchor, so one anomaly is noticed, not two; a merge window shorter than the gap does
/// not merge; and it works across a tick edge.
#[test]
fn the_cluster_merge_notices_bursts_at_neighbouring_services_once() {
    let p = public();
    let (site, dep) = site_and_dependent(&p);
    let params = |merge_ms: u64| MediumParams {
        onset: false,
        ramp: false,
        burst: true,
        coincidence: CoincidenceForm::Ordered,
        burst_window_ns: 25 * MS,
        burst_subtick_ns: 25 * MS,
        merge_window_ns: merge_ms * MS,
        ..MediumParams::default()
    };
    let cluster = |t0: u64, t1: u64| {
        vec![
            (t0, counter(site, CounterName::ErrorRate, 80)),
            (t0 + 5, message(site, SignalText::OutOfResource)),
            (t1, counter(dep, CounterName::ErrorRate, 80)),
            (t1 + 6, counter(dep, CounterName::Latency, 90)),
        ]
    };
    for (t0, t1) in [(2_310, 2_340), (2_390, 2_420)] {
        let obs = cluster(t0, t1);
        let mut off = Drive::new(params(0), &p);
        off.play(&obs, 6_000, &p);
        assert_eq!(off.notices.len(), 2, "{t0}: {:?}", off.notices);
        let mut on = Drive::new(params(50), &p);
        on.play(&obs, 6_000, &p);
        assert_eq!(on.notices.len(), 1, "{t0}: {:?}", on.notices);
        assert_eq!(on.notices[0].1.anchor, ObsId(0));
        assert_eq!(on.notices[0].1.site, ServiceId(site));
        assert_eq!(on.noticer.stats().duplicate_notices, 1);
        let mut short = Drive::new(params(10), &p);
        short.play(&obs, 6_000, &p);
        assert_eq!(short.notices.len(), 2, "{t0}");
    }
    // The merge needs the burst cells.
    let bad = MediumParams {
        burst: false,
        coincidence: CoincidenceForm::Ordered,
        merge_window_ns: 50 * MS,
        ..MediumParams::default()
    };
    assert!(bad.validate().is_err());
    // And it builds with a confirmation and the three-kind path.
    let confirmed = MediumParams {
        burst_confirm: Confirm::All,
        burst3_window_ns: 20 * MS,
        ..params(100)
    };
    confirmed.validate().unwrap();
}

#[test]
fn a_burst_of_two_kinds_needs_a_confirming_alarm_and_keeps_its_anchor_at_the_service() {
    let p = public();
    let (site, dep) = site_and_dependent(&p);
    let params = MediumParams {
        onset: false,
        ramp: false,
        burst: true,
        coincidence: CoincidenceForm::Ordered,
        burst_window_ns: 25 * MS,
        burst_confirm: Confirm::Dependents,
        confirm_hold_ns: 300 * MS,
        burst3_window_ns: 25 * MS,
        ..MediumParams::default()
    };
    params.validate().unwrap();
    let two = vec![
        (2_310, counter(site, CounterName::ErrorRate, 80)),
        (2_322, message(site, SignalText::OutOfResource)),
    ];
    // Alone: no notice.
    let mut d = Drive::new(params, &p);
    d.play(&two, 6_000, &p);
    assert!(d.notices.is_empty(), "{:?}", d.notices);
    // With a dependent's alarm 40 ms later (the next tick): a notice, anchored at the site, the
    // dependent's alarm not cited.
    let mut confirmed = two.clone();
    confirmed.push((2_362, counter(dep, CounterName::ErrorRate, 80)));
    let mut d = Drive::new(params, &p);
    d.play(&confirmed, 6_000, &p);
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.notices[0].1.anchor, ObsId(0));
    assert_eq!(d.notices[0].1.site, ServiceId(site));
    // Three kinds at the site need no confirmation.
    let mut three = two.clone();
    three.push((2_330, counter(site, CounterName::Latency, 80)));
    let mut d = Drive::new(params, &p);
    d.play(&three, 6_000, &p);
    assert_eq!(d.notices.len(), 1);
    assert_eq!(d.notices[0].1.anchor, ObsId(0));
}

/// The frozen media (`experiments/exploration/m2-selected.json`) and every variant the held-out run
/// plays of them build, and the frozen ones use no rhythm, phase gate or oscillator.
#[test]
fn the_frozen_media_and_their_variants_validate() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/exploration/m2-selected.json"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        return; // before the freeze
    };
    let sel: Value = serde_json::from_str(&text).unwrap();
    for tick in ["100", "500", "2000"] {
        let noticer = sel["ticks"][tick]["noticer"].clone();
        let NoticerSpec::Medium(frozen) = serde_json::from_value(noticer).unwrap() else {
            panic!("not a medium");
        };
        frozen.validate().unwrap();
        assert!(!frozen.rhythms, "tick {tick}");
        let (spec, _) =
            gordian_run::stream::arms::medium::graph::spec(&frozen, &public().services).unwrap();
        assert!(spec.oscillome.periods_ns.is_empty());
        assert!(
            spec.synapses
                .iter()
                .all(|s| !matches!(s.gate, gordian_medium::Gate::Phase { .. }))
        );
        assert!(
            spec.cells
                .iter()
                .all(|c| c.archetype != gordian_medium::Archetype::Oscillator)
        );
        for variant in [
            MediumParams {
                coincidence: CoincidenceForm::Sliding,
                ..frozen
            },
            MediumParams {
                abnormal_only: true,
                ..frozen
            },
            MediumParams {
                coincidence: CoincidenceForm::Binned,
                rhythms: true,
                burst_window_ns: 100 * MS,
                burst3_window_ns: 100 * MS,
                ..frozen
            },
            MediumParams {
                burst_lookback_ns: 1_000 * MS,
                ..frozen
            },
        ] {
            variant.validate().unwrap();
        }
    }
}

// ---- the manifest

#[test]
fn the_medium_noticer_is_written_and_read_in_a_manifest() {
    let spec = NoticerSpec::Medium(MediumParams::default());
    assert_eq!(spec.id(), MEDIUM_ID);
    assert!(spec.validate().is_ok());
    let text = serde_json::to_value(spec).unwrap();
    assert_eq!(text["noticer"], json!("medium"));
    assert_eq!(text["tick_ns"], json!(100_000_000u64));
    let back: NoticerSpec = serde_json::from_value(text.clone()).unwrap();
    assert_eq!(back, spec);
    let mut bad = text;
    bad["no_such_field"] = json!(1);
    assert!(serde_json::from_value::<NoticerSpec>(bad).is_err());
    for tick_ns in [0, 100_000_001, u64::from(u32::MAX) + 1_000] {
        let p = MediumParams {
            tick_ns,
            ..MediumParams::default()
        };
        assert!(NoticerSpec::Medium(p).validate().is_err(), "{tick_ns}");
    }
    let binned = MediumParams {
        coincidence: CoincidenceForm::Binned,
        ..MediumParams::default()
    };
    assert!(
        binned.validate().is_err(),
        "a binned coincidence needs rhythms"
    );
}

// ---- the ledger adapter and the charge on the bill

/// The medium arm under the selection oracle, as the comparison runs it.
fn medium_arm_record(seed: u64, params: MediumParams) -> gordian_run::stream::SegmentRecord {
    let p = stream_common::params(seed, 150);
    let rung = RungConfig {
        noticer: NoticerSpec::Medium(params),
        ..RungConfig::default()
    };
    let spec = StreamPolicySpec::from_parts(
        "oracle_selection",
        None,
        None,
        None,
        None,
        Some(16_000_000_000),
        None,
        None,
    )
    .unwrap();
    play_with_rung(&p, &spec, &limits(&p), &rung).unwrap()
}

/// The medium's cost on the public observations of `seed`, replayed as the rung's steps deliver
/// them (every 500 ms; the last step starts before the stream's end).
fn replayed_ns(seed: u64, params: MediumParams) -> (u64, u64) {
    let p = stream_common::params(seed, 150);
    let stream = generate(&p);
    let public = stream.public_info();
    let all: Vec<Held> = stream
        .events()
        .iter()
        .enumerate()
        .map(|(i, (at, o))| Held {
            id: ObsId(i as u32),
            at: *at,
            abnormal: is_abnormal(o, &public.services),
            obs: o.clone(),
        })
        .collect();
    let mut n = MediumNoticer::new(params, RungConfig::default(), &public.services).unwrap();
    let mut items = Vec::new();
    let mut next = 0;
    let mut now = 0u64;
    while now < p.duration_ns {
        while next < all.len() && all[next].at.0 <= now {
            items.push(all[next].clone());
            next += 1;
        }
        n.notice(Instant(now), &Store::with(items.clone()));
        now += 500 * MS;
    }
    (n.total_ns(), n.ledger().total_ticks)
}

#[test]
fn the_bill_is_charged_the_mediums_counts_at_the_declared_prices_plus_the_tick_price() {
    for tick_ms in [100, 500, 2_000] {
        let params = MediumParams {
            tick_ns: tick_ms * MS,
            ..MediumParams::default()
        };
        let record = medium_arm_record(3, params);
        let charged: u64 = record
            .bill
            .by_phase(Resource::Compute)
            .filter(|(phase, _)| *phase == Phase::Component(MEDIUM_COMPONENT))
            .map(|(_, ns)| ns)
            .sum();
        let (replayed, ticks) = replayed_ns(3, params);
        assert_eq!(charged, replayed, "tick {tick_ms} ms");
        // The last step starts at 149.5 s: the ticks complete by then.
        assert_eq!(ticks, 149_500 / tick_ms, "tick {tick_ms} ms");
        assert!(
            charged > ticks * TICK_PRICE_NS,
            "the medium did counted work"
        );
        assert_eq!(record.noticer, MEDIUM_ID);
    }
    // The declared prices are the calibrated ones.
    assert_eq!(Prices::DECLARED.cell_update_ps, 200_000);
}

#[test]
fn a_refused_charge_stops_the_medium() {
    let seed = 3;
    let params = MediumParams::default();
    let (full, _) = replayed_ns(seed, params);
    let p = stream_common::params(seed, 150);
    let mut l = limits(&p);
    // Enough substrate compute for the arm's bookkeeping, not for the medium's whole segment.
    l.compute = 400_000 + full / 4;
    let rung = RungConfig {
        noticer: NoticerSpec::Medium(params),
        ..RungConfig::default()
    };
    let spec = StreamPolicySpec::from_id("never_escalate").unwrap();
    let r = play_with_rung(&p, &spec, &l, &rung).unwrap();
    let charged: u64 = r
        .bill
        .by_phase(Resource::Compute)
        .filter(|(phase, _)| *phase == Phase::Component(MEDIUM_COMPONENT))
        .map(|(_, ns)| ns)
        .sum();
    assert!(charged > 0 && charged < full, "charged {charged} of {full}");
    assert!(r.bill.total(Resource::Compute) <= l.compute);
}

#[test]
fn medium_runs_are_deterministic() {
    let a = medium_arm_record(5, MediumParams::default());
    let b = medium_arm_record(5, MediumParams::default());
    assert_eq!(a.notice_log, b.notice_log);
    assert_eq!(a.trajectory, b.trajectory);
    assert!(a.notice_log.iter().any(|e| e.kind == NoticeKind::Notice));
}

// ---- nothing changes for an arm that does not use the medium

#[test]
fn an_arm_without_the_medium_writes_the_same_files_beside_a_medium_arm() {
    let arms = [
        ("sel_rung_privileged", "oracle_selection"),
        ("never_escalate", "never_escalate"),
    ];
    let alone = manifest("m2-alone", &arms, 2, 150, 7);
    let mut with: StreamManifest = alone.clone();
    with.run_id = "m2-with".to_owned();
    with.arms
        .push(gordian_run::stream::manifest::StreamArmSpec {
            arm: "sel_medium_privileged".to_owned(),
            policy: StreamPolicySpec::from_id("oracle_selection").unwrap(),
            context: None,
        });
    with.noticers.insert(
        "sel_medium_privileged".to_owned(),
        NoticerSpec::Medium(MediumParams::default()),
    );
    with.validate().unwrap();
    let a = scratch("m2-alone");
    let w = scratch("m2-with");
    execute_stream(&alone, &a).unwrap();
    execute_stream(&with, &w).unwrap();
    for arm in ["sel_rung_privileged", "never_escalate"] {
        for file in [
            "results.csv",
            "incidents.csv",
            "notices.csv",
            "notice_incidents.csv",
            "notice_events.csv",
        ] {
            assert_eq!(
                without_run_id(&read(&a.join(arm), file)),
                without_run_id(&read(&w.join(arm), file)),
                "{arm}/{file}"
            );
        }
    }
    let notices = read(&w.join("sel_medium_privileged"), "notices.csv");
    assert_eq!(cell(&notices, 0, "noticer"), MEDIUM_ID);
    let m: Value = serde_json::from_str(&read(&w, "manifest.json")).unwrap();
    assert_eq!(
        m["noticers"]["sel_medium_privileged"]["noticer"],
        json!("medium")
    );
}

/// The medium's own cost per stream, replayed on the public observations of the streams named by
/// `M2_COST_SEEDS` (`first:count`) as the rung's steps deliver them, for each medium in
/// `M2_COST_PARAMS` (a JSON object, label to noticer), written as CSV to `M2_COST_OUT`, with the
/// conversion table of each medium to `M2_COST_OUT` + `.conversions.csv`. The replay equals the
/// in-run charge (`the_bill_is_charged_the_mediums_counts_at_the_declared_prices_plus_the_tick_price`).
#[test]
#[ignore = "a tool for the PI: writes files"]
fn replay_medium_cost() {
    let seeds = std::env::var("M2_COST_SEEDS").unwrap_or_else(|_| "20000:2".to_owned());
    let out = std::env::var("M2_COST_OUT").unwrap_or_else(|_| "m2-cost.csv".to_owned());
    let params: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("M2_COST_PARAMS").unwrap()).unwrap(),
    )
    .unwrap();
    let (first, count) = seeds.split_once(':').expect("first:count");
    let first: u64 = first.parse().unwrap();
    let count: u64 = count.parse().unwrap();
    let mut text = String::from(
        "label,seed,ticks,cell_updates,synapse_traversals,event_routings,field_reads,proposals,truncated_ticks,modelled_ns\n",
    );
    let mut conv = String::from("label,target,kind,ns,tick_len_ns,value\n");
    for (label, noticer) in params.as_object().unwrap() {
        let NoticerSpec::Medium(p) = serde_json::from_value(noticer.clone()).unwrap() else {
            panic!("{label}: not a medium");
        };
        for seed in first..first + count {
            let sp = StreamParams::new(seed);
            let stream = generate(&sp);
            let public = stream.public_info();
            if seed == first {
                let (spec, _) =
                    gordian_run::stream::arms::medium::graph::spec(&p, &public.services).unwrap();
                for c in spec.conversions().unwrap() {
                    writeln!(
                        conv,
                        "{label},\"{:?}\",{:?},{},{},{}",
                        c.target, c.kind, c.ns, c.tick_len_ns, c.value
                    )
                    .unwrap();
                }
            }
            let all: Vec<Held> = stream
                .events()
                .iter()
                .enumerate()
                .map(|(i, (at, o))| Held {
                    id: ObsId(i as u32),
                    at: *at,
                    abnormal: is_abnormal(o, &public.services),
                    obs: o.clone(),
                })
                .collect();
            let mut n = MediumNoticer::new(p, RungConfig::default(), &public.services).unwrap();
            let mut store: std::collections::VecDeque<Held> = std::collections::VecDeque::new();
            let (mut next, mut now) = (0, 0u64);
            while now < sp.duration_ns {
                while next < all.len() && all[next].at.0 <= now {
                    store.push_back(all[next].clone());
                    next += 1;
                }
                while store
                    .front()
                    .is_some_and(|h| h.at.0 + 120_000_000_000 < now)
                {
                    store.pop_front();
                }
                n.notice(Instant(now), &Store::with(store.iter().cloned()));
                now += 500 * MS;
            }
            let l = n.ledger();
            let c = l.total_counts;
            writeln!(
                text,
                "{label},{seed},{},{},{},{},{},{},{},{}",
                l.total_ticks,
                c.cell_updates,
                c.synapse_traversals,
                c.event_routings,
                c.field_reads,
                c.proposals,
                l.truncated_ticks,
                n.total_ns()
            )
            .unwrap();
        }
    }
    std::fs::write(&out, text).unwrap();
    std::fs::write(format!("{out}.conversions.csv"), conv).unwrap();
}

/// Public observations of the streams named by `M2_DUMP_SEEDS` (`first:count`), written as CSV
/// to `M2_DUMP_OUT`: for designing the graph on the tuning streams from public data. Every
/// column is public (the observation, its instant and id, the public rules' verdict).
#[test]
#[ignore = "a tool for the PI: writes a file"]
fn dump_public_observations() {
    let seeds = std::env::var("M2_DUMP_SEEDS").unwrap_or_else(|_| "10000:2".to_owned());
    let out = std::env::var("M2_DUMP_OUT").unwrap_or_else(|_| "m2-dump.csv".to_owned());
    let (first, count) = seeds.split_once(':').expect("first:count");
    let first: u64 = first.parse().expect("first seed");
    let count: u64 = count.parse().expect("count");
    let mut text = String::from("seed,id,at_ns,service,kind,name,value,abnormal\n");
    for seed in first..first + count {
        let stream = generate(&StreamParams::new(seed));
        let public = stream.public_info();
        for (i, (at, obs)) in stream.events().iter().enumerate() {
            let abnormal = is_abnormal(obs, &public.services);
            let (service, kind, name, value) = match obs {
                Observation::Counter {
                    service,
                    name,
                    value,
                } => (service.0, "counter", format!("{name:?}"), *value),
                Observation::Message {
                    service,
                    text_id,
                    severity,
                } => (service.0, "message", format!("{text_id}/{severity:?}"), 1),
                Observation::Snapshot { service, .. } => (service.0, "snapshot", String::new(), 1),
                _ => continue,
            };
            writeln!(
                text,
                "{seed},{i},{},{service},{kind},{name},{value},{}",
                at.0,
                u8::from(abnormal)
            )
            .expect("write to a string");
        }
    }
    std::fs::write(&out, text).expect("write the dump");
}

// ---- the frozen M2 media, pinned before M3 changes the medium crate

/// FNV-1a 64 over text.
fn fnv(h: &mut u64, text: &str) {
    for b in text.as_bytes() {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The frozen media of M2 (`m2-selected.json`), by tick length in milliseconds.
fn m2_frozen(tick: &str) -> MediumParams {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/exploration/m2-selected.json"
    );
    let sel: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let NoticerSpec::Medium(p) =
        serde_json::from_value(sel["ticks"][tick]["noticer"].clone()).unwrap()
    else {
        panic!("not a medium");
    };
    p
}

/// What the frozen M2 media do on two short streams under the selection oracle (every notice
/// and retirement, the trajectory, and the medium's charge on the bill), at each tick length.
fn m2_frozen_digest() -> (u64, usize) {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut notices = 0;
    for tick in ["100", "500", "2000"] {
        for seed in [3u64, 5] {
            let r = medium_arm_record(seed, m2_frozen(tick));
            notices += r
                .notice_log
                .iter()
                .filter(|e| e.kind == NoticeKind::Notice)
                .count();
            fnv(&mut h, &format!("{:?}", r.notice_log));
            fnv(&mut h, &format!("{:?}", r.trajectory));
            let charged: u64 = r
                .bill
                .by_phase(Resource::Compute)
                .filter(|(phase, _)| *phase == Phase::Component(MEDIUM_COMPONENT))
                .map(|(_, ns)| ns)
                .sum();
            fnv(&mut h, &charged.to_string());
        }
    }
    (h, notices)
}

/// The digest M2's frozen media produced on M2's merged code, before any M3 change (commit
/// "Pin M1b's bytes as digests before sub-tick pruning changes anything"): with sub-tick pruning
/// off, M3's medium must reproduce it.
const M2_FROZEN_DIGEST: u64 = 0xbdee_d209_4069_7150;

#[test]
fn the_frozen_m2_media_notice_as_they_did_with_sub_tick_pruning_off() {
    let (digest, notices) = m2_frozen_digest();
    eprintln!("m2 frozen digest {digest:#018x}, {notices} notices");
    assert!(notices > 10, "not vacuous");
    assert_eq!(digest, M2_FROZEN_DIGEST);
}
