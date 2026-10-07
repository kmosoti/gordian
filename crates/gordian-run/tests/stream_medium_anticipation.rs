//! The medium noticer's anticipation layer (work item A2): its configuration in a manifest, the
//! rule (first alarms, the public graph's explanation, trials only for partners not in burst,
//! the miss weight at the partner's chance rate), a learned edge and the prediction it makes,
//! the attach switch, the trace's rows, the charge on the bill, and that with the switch off the
//! layer changes nothing the arm does.
//!
//! The byte-identity gates against R6's recorded hashes and A1a's, A1c's and A1d's kept runs are
//! runs, not tests (`experiments/exploration/scripts/a2_gate.py`).

mod stream_common;

use gordian_core::{Instant, Phase, Resource};
use gordian_medium::{Mark, MarkLog};
use gordian_run::stream::arms::medium::anticipation::{
    self, AnticipationConfig, FILE_HEADER, Kind, Layer,
};
use gordian_run::stream::arms::medium::{MEDIUM_COMPONENT, MediumNoticer, MediumParams};
use gordian_run::stream::arms::noticer::{Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamPublic};
use gordian_world::graph::dependents_mask;
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use serde_json::{Value, json};
use stream_common::*;

const MS: u64 = 1_000_000;

fn alarm(service: u32) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name: CounterName::ErrorRate,
        value: 80,
    }
}

fn public() -> StreamPublic {
    public_of(&params(0, 150))
}

/// `dep[a][b]`: `b` is a transitive dependent of `a`.
fn deps(public: &StreamPublic) -> Vec<Vec<bool>> {
    let s = &public.services;
    s.iter().map(|x| dependents_mask(s, x.id)).collect()
}

fn unconnected(public: &StreamPublic) -> Vec<(u32, u32)> {
    let d = deps(public);
    let n = d.len();
    (0..n)
        .flat_map(|a| (0..n).map(move |b| (a, b)))
        .filter(|&(a, b)| a != b && !d[a][b] && !d[b][a])
        .map(|(a, b)| (a as u32, b as u32))
        .collect()
}

fn config(key: u64) -> AnticipationConfig {
    anticipation::reset_segments(key);
    AnticipationConfig {
        trace_key: key,
        trace: true,
        ..AnticipationConfig::default()
    }
}

fn with(a: Option<AnticipationConfig>) -> MediumParams {
    MediumParams {
        ramp: false,
        anticipation: a,
        ..MediumParams::default()
    }
}

/// Plays observations through a medium noticer as the rung does: every 500 ms, the abnormal ones
/// observed, then a notice with everything delivered.
struct Drive {
    noticer: MediumNoticer,
    all: Vec<Held>,
    next: usize,
    items: Vec<Held>,
    now_ms: u64,
}

impl Drive {
    fn new(params: MediumParams, public: &StreamPublic, obs: &[(u64, Observation)]) -> Self {
        let mut obs = obs.to_vec();
        obs.sort_by_key(|(t, _)| *t);
        Self {
            noticer: MediumNoticer::new(params, RungConfig::default(), &public.services).unwrap(),
            all: obs
                .iter()
                .enumerate()
                .map(|(i, (ms, o))| Held {
                    id: ObsId(i as u32),
                    at: Instant(ms * MS),
                    abnormal: is_abnormal(o, &public.services),
                    obs: o.clone(),
                })
                .collect(),
            next: 0,
            items: Vec::new(),
            now_ms: 0,
        }
    }

    fn until(&mut self, ms: u64) {
        while self.now_ms <= ms {
            let now = self.now_ms * MS;
            while self.next < self.all.len() && self.all[self.next].at.0 <= now {
                let h = self.all[self.next].clone();
                if h.abnormal {
                    self.noticer.observe(&h);
                }
                self.items.push(h);
                self.next += 1;
            }
            let store = Store::with(self.items.clone());
            self.noticer.notice(Instant(now), &store);
            for id in self.noticer.retirable(Instant(now)) {
                self.noticer.retire(id);
            }
            self.now_ms += 500;
        }
    }

    fn layer(&self) -> &Layer {
        self.noticer.anticipation().unwrap()
    }

    fn id_at(&self, ms: u64) -> u32 {
        self.all.iter().position(|h| h.at.0 == ms * MS).unwrap() as u32
    }

    fn marks(&self, kind: Kind) -> Vec<Mark> {
        self.layer()
            .marks()
            .iter()
            .filter(|m| m.kind == kind as u16)
            .copied()
            .collect()
    }
}

fn tag(partner: u32, band: usize) -> u32 {
    (partner << 8) | band as u32
}

// ---- the configuration

#[test]
fn the_first_values_are_the_documented_ones_and_an_a1_manifest_reads_and_writes_as_before() {
    let c = AnticipationConfig::default();
    assert_eq!(c.bands_ns, [400 * MS, 2_000 * MS, 10_000 * MS]);
    assert_eq!((c.gain, c.threshold), (1.0, 2.0));
    assert_eq!(c.tau_ns, 150_000 * MS);
    assert!(c.explained && !c.attach && !c.trace);
    // A manifest without the layer: no `anticipation` key written, and read back the same.
    let text = serde_json::to_value(NoticerSpec::Medium(MediumParams::default())).unwrap();
    assert!(text.get("anticipation").is_none());
    // The layer with only its key: every other field takes its first value.
    let mut t = text.clone();
    t["anticipation"] = json!({ "trace_key": 7 });
    let NoticerSpec::Medium(p) = serde_json::from_value(t).unwrap() else {
        panic!("not a medium");
    };
    assert_eq!(
        p.anticipation,
        Some(AnticipationConfig {
            trace_key: 7,
            ..AnticipationConfig::default()
        })
    );
    // Unknown fields and bad bands are refused.
    let mut bad = text.clone();
    bad["anticipation"] = json!({ "trace_key": 7, "nope": 1 });
    assert!(serde_json::from_value::<NoticerSpec>(bad).is_err());
    let mut p = MediumParams {
        anticipation: Some(AnticipationConfig {
            bands_ns: [2_000 * MS, 400 * MS, 10_000 * MS],
            ..AnticipationConfig::default()
        }),
        ..MediumParams::default()
    };
    assert!(NoticerSpec::Medium(p).validate().is_err());
    p.anticipation = Some(AnticipationConfig::default());
    assert!(NoticerSpec::Medium(p).validate().is_ok());
}

#[test]
fn the_layer_has_pair_cells_for_every_unconnected_ordered_pair_and_no_other() {
    let public = public();
    let d = Drive::new(with(Some(config(9_200))), &public, &[]);
    let pairs = unconnected(&public);
    assert!(!pairs.is_empty());
    let cells = d.layer().cells();
    assert_eq!(cells.pairs().len(), pairs.len());
    for (a, b) in &pairs {
        assert!(cells.has(*a as u16, *b as u16));
    }
    let dep = deps(&public);
    for (a, row) in dep.iter().enumerate() {
        for (b, connected) in row.iter().enumerate() {
            if *connected {
                assert!(!cells.has(a as u16, b as u16) && !cells.has(b as u16, a as u16));
            }
        }
    }
    assert_eq!(d.layer().medium().cells().len(), pairs.len() * 3 * 2);
}

// ---- the rule

/// A pair `(a, b)` that the public graph does not connect, with no alarm elsewhere.
fn pair(public: &StreamPublic) -> (u32, u32) {
    unconnected(public)[0]
}

/// `a` alarms at each of `at_ms` and `b` `gap_ms` later.
fn co_alarms(a: u32, b: u32, at_ms: &[u64], gap_ms: u64) -> Vec<(u64, Observation)> {
    at_ms
        .iter()
        .flat_map(|&t| [(t, alarm(a)), (t + gap_ms, alarm(b))])
        .collect()
}

#[test]
fn three_follows_within_0_4_s_learn_the_edge_and_the_next_alarm_predicts_the_partner() {
    let public = public();
    let (a, b) = pair(&public);
    let obs = co_alarms(a, b, &[10_000, 30_000, 50_000, 70_000], 100);
    let mut d = Drive::new(with(Some(config(9_201))), &public, &obs);
    d.until(80_000);
    let s = *d.layer().stats();
    assert_eq!(s.first_alarms, 8);
    assert_eq!(s.counted, 8, "no alarm is explained by the public graph");
    // Every follow of (a, b) is credited in all three bands; b's alarms come while a is in burst.
    let follows: Vec<Mark> = d
        .marks(Kind::Follow)
        .into_iter()
        .filter(|m| m.subject == a && m.tag >> 8 == b)
        .collect();
    assert_eq!(follows.len(), 4 * 3);
    // Held at the fourth alarm in the 0.4 s band (each follow worth 1 - q, q about 0.03: a level
    // about 2.25), not at the third (about 1.6). In the 10 s band a follow is worth about half
    // (b alarms within 10 s about half the time by chance): not held.
    let preds = d.marks(Kind::Prediction);
    assert_eq!(preds.len(), 1, "{preds:?}");
    let p = preds[0];
    assert_eq!((p.subject, p.event), (a, d.id_at(70_000)));
    assert_eq!(p.tag, tag(b, 0), "the narrowest band held");
    assert_eq!(p.at_ns, 70_000 * MS, "the step that delivered the alarm");
    let held = d.marks(Kind::Held);
    let band0: Vec<&Mark> = held.iter().filter(|m| m.tag == tag(b, 0)).collect();
    assert_eq!(band0.len(), 1);
    assert!(band0[0].value > 2_000 && band0[0].value < 2_400, "{held:?}");
    assert!(!held.iter().any(|m| m.tag == tag(b, 2)), "{held:?}");
    assert!(held.iter().all(|m| m.event == d.id_at(70_000)));
    // It is followed on the public side by b's alarm 100 ms later.
    let f = d.marks(Kind::Followed);
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].subject, f[0].event, f[0].value),
        (a, d.id_at(70_000), i64::from(d.id_at(70_100)))
    );
    assert_eq!((s.predictions, s.followed, s.expired), (1, 1, 0));
}

#[test]
fn a_partner_in_burst_gets_no_trial_and_no_prediction() {
    let public = public();
    let (a, b) = pair(&public);
    let mut obs = co_alarms(a, b, &[10_000, 30_000, 50_000], 100);
    // At 70 s, b is already alarming (from 69.5 s) when a alarms.
    obs.push((69_500, alarm(b)));
    obs.push((70_000, alarm(a)));
    let mut d = Drive::new(with(Some(config(9_202))), &public, &obs);
    d.until(80_000);
    assert!(d.marks(Kind::Prediction).is_empty());
    assert!(
        d.marks(Kind::Held).iter().any(|m| m.tag == tag(b, 0)),
        "the edge is held, the partner is busy"
    );
    // Trials opened at a's alarm at 70 s: every partner but b, three bands each.
    let partners_a = d.layer().unconnected(a).len() as u64;
    let partners_b = d.layer().unconnected(b).len() as u64;
    // a's four alarms with b free three times; b's four alarms, a in burst three times (at 10.1,
    // 30.1, 50.1 s) and free once (at 69.5 s).
    let want = 3 * (4 * partners_a - 1) + 3 * (4 * partners_b - 3);
    assert_eq!(d.layer().stats().trials, want);
}

#[test]
fn an_alarm_the_public_graph_explains_is_neither_a_trial_nor_a_follow_unless_the_switch_is_off() {
    let public = public();
    let dep = deps(&public);
    let n = dep.len();
    // u -> s in the public graph (s a dependent of u), and x unconnected to s and to u.
    let un = unconnected(&public);
    let (u, s, x) = (0..n)
        .flat_map(|u| (0..n).map(move |s| (u, s)))
        .filter(|&(u, s)| u != s && dep[u][s])
        .find_map(|(u, s)| {
            let (u, s) = (u as u32, s as u32);
            un.iter()
                .find(|&&(x, y)| y == s && un.contains(&(x, u)))
                .map(|&(x, _)| (u, s, x))
        })
        .expect("the test graph has an edge and a service unconnected to both its ends");
    // x alarms at 10 s (a trial for s), u at 10.2 s and s at 10.25 s (explained by u).
    let obs = vec![(10_000, alarm(x)), (10_200, alarm(u)), (10_250, alarm(s))];
    let mut d = Drive::new(with(Some(config(9_203))), &public, &obs);
    d.until(30_000);
    let st = *d.layer().stats();
    assert_eq!((st.first_alarms, st.counted), (3, 2));
    let first: Vec<(u32, i64)> = d
        .marks(Kind::FirstAlarm)
        .iter()
        .map(|m| (m.subject, m.value))
        .collect();
    assert_eq!(first, vec![(x, 1), (u, 1), (s, 0)]);
    // x's trials for s all missed: s's explained alarm did not credit them.
    let xs = |kind| {
        d.marks(kind)
            .iter()
            .filter(|m| m.subject == x && m.tag >> 8 == s)
            .count()
    };
    assert_eq!((xs(Kind::Follow), xs(Kind::Miss)), (0, 3));
    // With the switch off every first alarm counts, and s's alarm follows x's in every band.
    let off = AnticipationConfig {
        explained: false,
        ..config(9_204)
    };
    let mut d = Drive::new(with(Some(off)), &public, &obs);
    d.until(30_000);
    assert_eq!(d.layer().stats().counted, 3);
    let follows = d
        .marks(Kind::Follow)
        .iter()
        .filter(|m| m.subject == x && m.tag >> 8 == s)
        .count();
    assert_eq!(follows, 3);
}

#[test]
fn a_miss_weighs_the_partners_chance_rate_under_the_rungs_prior() {
    let public = public();
    let (a, b) = pair(&public);
    // One alarm at a at 30 s; nothing at b: b's rate is the prior, 3 alarms over 30 s of quiet
    // time plus the prior's 30 s.
    let mut d = Drive::new(with(Some(config(9_205))), &public, &[(30_000, alarm(a))]);
    d.until(50_000);
    let rate = 3.0f64 / 60.0;
    let misses: Vec<(usize, i64)> = d
        .marks(Kind::Miss)
        .iter()
        .filter(|m| m.subject == a && m.tag >> 8 == b)
        .map(|m| ((m.tag & 0xFF) as usize, m.value))
        .collect();
    assert_eq!(misses.len(), 3);
    for (k, v) in misses {
        let w = [0.4, 2.0, 10.0][k];
        let want = -((1.0 - (-rate * w).exp()) * 1000.0);
        assert!((v as f64 - want).abs() <= 1.0, "band {k}: {v} {want}");
    }
    // The misses land at the deadlines.
    let at: Vec<u64> = d
        .marks(Kind::Miss)
        .iter()
        .filter(|m| m.subject == a && m.tag >> 8 == b)
        .map(|m| m.at_ns / MS)
        .collect();
    assert_eq!(at, vec![30_400, 32_000, 40_000]);
}

#[test]
fn the_partners_rate_is_its_counted_first_alarms_per_second_of_its_quiet_time() {
    let public = public();
    let (a, b) = pair(&public);
    // b alarms at 10 s and 10.5 s (one burst: in burst from 10 s to 12.5 s), then a at 30 s.
    let obs = vec![(10_000, alarm(b)), (10_500, alarm(b)), (30_000, alarm(a))];
    let mut d = Drive::new(with(Some(config(9_213))), &public, &obs);
    d.until(29_500);
    assert_eq!(d.layer().quiet_ns(b, 30_000 * MS), 27_500 * MS);
    assert_eq!(d.layer().quiet_ns(b, 11_000 * MS), 10_000 * MS);
    assert_eq!(d.layer().quiet_ns(a, 30_000 * MS), 30_000 * MS);
    d.until(50_000);
    // One counted first alarm over 27.5 s of quiet time, with the prior's 3 over 30 s.
    let rate = 4.0f64 / 57.5;
    let misses: Vec<i64> = d
        .marks(Kind::Miss)
        .iter()
        .filter(|m| m.subject == a && m.tag >> 8 == b)
        .map(|m| m.value)
        .collect();
    assert_eq!(misses.len(), 3);
    for (k, v) in misses.into_iter().enumerate() {
        let want = -((1.0 - (-rate * [0.4, 2.0, 10.0][k]).exp()) * 1000.0);
        assert!((v as f64 - want).abs() <= 1.0, "band {k}: {v} {want}");
    }
}

#[test]
fn nothing_carries_into_the_next_segment() {
    let public = public();
    let (a, b) = pair(&public);
    let obs = co_alarms(a, b, &[10_000, 30_000, 50_000, 70_000], 100);
    let key = 9_206;
    let mut d = Drive::new(with(Some(config(key))), &public, &obs);
    d.until(80_000);
    assert_eq!(d.layer().stats().predictions, 1);
    // A new segment under the same key starts with no evidence: one more co-alarm predicts nothing.
    let cfg = AnticipationConfig {
        trace_key: key,
        trace: true,
        ..AnticipationConfig::default()
    };
    let mut d2 = Drive::new(with(Some(cfg)), &public, &co_alarms(a, b, &[10_000], 100));
    d2.until(20_000);
    assert_eq!(d2.layer().stats().predictions, 0);
    assert!(d2.marks(Kind::Held).is_empty());
}

// ---- the attach switch (item 3)

/// The test burst of `tests/stream_medium.rs` at `site` from `t`: enough for the medium to notice.
fn burst(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, alarm(site)),
        (
            t + 10,
            Observation::Counter {
                service: ServiceId(site),
                name: CounterName::Latency,
                value: 120,
            },
        ),
        (
            t + 20,
            Observation::Message {
                service: ServiceId(site),
                text_id: gordian_world::physics::SignalText::OutOfResource.text_id(),
                severity: Severity::Low,
            },
        ),
        (t + 40, alarm(site)),
    ]
}

/// Learn `a -> b` from three co-alarms, then a burst at `a` at 70 s and one alarm at `b` 100 ms
/// after it; whether the anomaly noticed at `a` owns `b`'s alarm.
fn attach_case(key: u64, attach: bool) -> (bool, u64) {
    let public = public();
    let (a, b) = pair(&public);
    let mut obs = co_alarms(a, b, &[10_000, 30_000, 50_000], 100);
    obs.extend(burst(a, 70_000));
    obs.push((70_100, alarm(b)));
    let cfg = AnticipationConfig {
        attach,
        ..config(key)
    };
    let mut d = Drive::new(with(Some(cfg)), &public, &obs);
    d.until(72_000);
    let target = ObsId(d.id_at(70_100));
    let owner = d
        .noticer
        .anomalies()
        .iter()
        .find(|x| x.site == ServiceId(a) && x.anchor == ObsId(d.id_at(70_000)))
        .expect("the burst at a is noticed");
    (owner.owns(target), d.layer().stats().attached)
}

#[test]
fn with_the_switch_on_a_learned_edge_attaches_the_partners_alarm_to_the_anomaly() {
    assert_eq!(attach_case(9_207, true), (true, 1));
    assert_eq!(attach_case(9_208, false), (false, 0));
}

#[test]
fn the_switch_never_overrides_the_rungs_first_two_rules() {
    // With no learned edge, the switch changes nothing; with one, an alarm at the partner while
    // the partner's own anomaly is speaking stays with it (checked through the helper's base):
    // here, the partner bursts first at 69 s and is noticed, so its alarm at 70.1 s is its own.
    let public = public();
    let (a, b) = pair(&public);
    let mut obs = co_alarms(a, b, &[10_000, 30_000, 50_000], 100);
    obs.extend(burst(b, 69_000));
    obs.extend(burst(a, 70_000));
    obs.push((70_100, alarm(b)));
    let cfg = AnticipationConfig {
        attach: true,
        ..config(9_209)
    };
    let mut d = Drive::new(with(Some(cfg)), &public, &obs);
    d.until(72_000);
    let target = ObsId(d.id_at(70_100));
    let owners: Vec<ServiceId> = d
        .noticer
        .anomalies()
        .iter()
        .filter(|x| x.owns(target))
        .map(|x| x.site)
        .collect();
    assert_eq!(owners, vec![ServiceId(b)]);
    assert_eq!(d.layer().stats().attached, 0);
}

// ---- the trace

#[test]
fn trace_rows_name_the_kind_service_observation_partner_and_band() {
    let m = |kind: Kind, subject: u32, event: u32, tag: u32, value: i64| Mark {
        at_ns: 5,
        kind: kind as u16,
        subject,
        event,
        tag,
        value,
    };
    let none = u32::MAX;
    assert_eq!(
        anticipation::row(2, &m(Kind::Prediction, 3, 41, tag(7, 1), -1)),
        "2,5,prediction,3,41,7,1,-1"
    );
    assert_eq!(
        anticipation::row(0, &m(Kind::FirstAlarm, 3, 41, none, 1)),
        "0,5,first_alarm,3,41,,,1"
    );
    assert_eq!(
        anticipation::row(0, &m(Kind::SegmentEnd, none, none, none, 99)),
        "0,5,segment_end,,,,,99"
    );
    let dir = scratch("a2-trace");
    let log = MarkLog {
        marks: vec![m(Kind::Followed, 1, 2, tag(3, 0), 9)],
    };
    anticipation::append_to(&dir, 77, 0, &log);
    anticipation::append_to(&dir, 77, 1, &log);
    let text = read(&dir, "anticipation-trace-77.csv");
    assert_eq!(
        text,
        format!("{FILE_HEADER}\n0,5,followed,1,2,3,0,9\n1,5,followed,1,2,3,0,9\n")
    );
}

#[test]
fn without_the_trace_switch_no_mark_is_kept_and_the_counts_are_the_same() {
    let public = public();
    let (a, b) = pair(&public);
    let obs = co_alarms(a, b, &[10_000, 30_000, 50_000, 70_000], 100);
    let mut on = Drive::new(with(Some(config(9_210))), &public, &obs);
    on.until(80_000);
    let off_cfg = AnticipationConfig {
        trace: false,
        ..config(9_211)
    };
    let mut off = Drive::new(with(Some(off_cfg)), &public, &obs);
    off.until(80_000);
    assert!(off.layer().marks().is_empty());
    assert_eq!(on.layer().stats(), off.layer().stats());
    assert_eq!(
        on.noticer.take_cost().map(|c| c.compute_ns),
        off.noticer.take_cost().map(|c| c.compute_ns),
        "the trace never changes what is charged"
    );
}

// ---- the arm: charged, and otherwise unchanged

fn m3_frozen() -> MediumParams {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/exploration/m3-selected.json"
    );
    let sel: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let NoticerSpec::Medium(p) =
        serde_json::from_value(sel["ticks"]["100"]["noticer"].clone()).unwrap()
    else {
        panic!("not a medium");
    };
    p
}

fn record(seed: u64, params: MediumParams) -> gordian_run::stream::SegmentRecord {
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

#[test]
fn on_real_streams_the_layer_is_charged_and_changes_no_notice_or_decision() {
    for seed in [3, 5] {
        let base = m3_frozen();
        let mut with_layer = base;
        with_layer.anticipation = Some(AnticipationConfig {
            trace_key: 9_212 + seed,
            ..AnticipationConfig::default()
        });
        let r0 = record(seed, base);
        let r1 = record(seed, with_layer);
        let notices = |r: &gordian_run::stream::SegmentRecord| {
            r.notice_log
                .iter()
                .map(|e| (e.kind, e.anomaly, e.anchor, e.site, e.anchor_at))
                .collect::<Vec<_>>()
        };
        assert_eq!(notices(&r0), notices(&r1), "seed {seed}");
        let decisions = |r: &gordian_run::stream::SegmentRecord| {
            r.verdict
                .per_incident
                .iter()
                .map(|v| {
                    (
                        v.id,
                        v.correct_declarations,
                        v.wrong_declarations,
                        v.correct_by_deadline,
                        v.missed,
                        v.escalations,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(decisions(&r0), decisions(&r1), "seed {seed}");
        let charged = |r: &gordian_run::stream::SegmentRecord| -> u64 {
            r.bill
                .by_phase(Resource::Compute)
                .filter(|(phase, _)| *phase == Phase::Component(MEDIUM_COMPONENT))
                .map(|(_, ns)| ns)
                .sum()
        };
        // The pair-cell medium ticks too: at least 200 ns more per tick of the noticer's.
        assert!(
            charged(&r1) >= charged(&r0) + 1_400 * 200,
            "seed {seed}: {} against {}",
            charged(&r1),
            charged(&r0)
        );
    }
}
