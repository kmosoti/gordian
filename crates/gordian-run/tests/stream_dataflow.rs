//! The incremental-dataflow noticer (work item C1): that it gives the hand-written composition's
//! answers, call by call, on real streams and on generated scenarios built to reach every rule; that
//! the notice record of B3's row is reproduced on the 100 tuning and 200 held-out streams; its counts,
//! its billing, its determinism and its spelling in a manifest.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test (`scripts/c1_gate.py`);
//! the tests here pin the same property on small runs. The reproduction on 100 tuning and 200
//! held-out streams is two ignored tests (they play 600 s streams through both noticers) that the
//! unit's report records the exit status of; `replay_costs` is the PI's tool for the cost columns.

mod stream_common;

use gordian_core::{Instant, Phase, Resource};
use gordian_run::stream::arms::dataflow::engine::Prices;
use gordian_run::stream::arms::dataflow::{
    DATAFLOW_COMPONENT, DATAFLOW_ID, DataflowNoticer, DataflowSpec,
};
use gordian_run::stream::arms::medium::{MediumNoticer, MediumParams};
use gordian_run::stream::arms::noticer::{
    self, BaseSpec, Notice, NoticeKind, Noticer, NoticerSpec, Tracked,
};
use gordian_run::stream::arms::noticer_ramp::RampSpec;
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::noticer_split::SplitSpec;
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::execute_stream;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamParams, generate};
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{CounterName, Observation, Service, ServiceId, Severity};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::Path;
use stream_common::*;

const MS: u64 = 1_000_000;
const STEP_NS: u64 = 500 * MS;
/// How much of the store the drivers hand the noticers: both read only what is newer than their
/// own cursor, which is never older than a step.
const WINDOW_NS: u64 = 10_000 * MS;

// ---- the B3 row -----------------------------------------------------------------------------------

fn exploration(file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../experiments/exploration")
        .join(file);
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")),
    )
    .unwrap()
}

/// B2's re-anchor as the manifest spells it (`scripts/m2_common.py`, `REANCHOR`).
fn reanchor_base() -> BaseSpec {
    BaseSpec::Reanchor {
        notice_z: Some(2.0),
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    }
}

/// The ramp and the split of the arm `ramp_split_over_re2`, from `b3-selected.json`.
fn b3_pieces() -> (RampSpec, SplitSpec) {
    let sel = exploration("b3-selected.json");
    let r = &sel["ramp"]["chosen"]["params"];
    let s = &sel["split_re2"]["chosen"]["params"];
    let n = |v: &Value| v.as_u64().unwrap();
    (
        RampSpec {
            gap_ns: n(&r["gap_ms"]) * MS,
            max_step: n(&r["max_step"]) as u32,
            max_drop: n(&r["max_drop"]) as u32,
            min_readings: n(&r["min_readings"]) as u32,
            min_rise: n(&r["min_rise"]) as u32,
            follow: None,
        },
        SplitSpec {
            gap_ns: n(&s["gap_ms"]) * MS,
            min_burst: n(&s["min_burst"]) as u32,
        },
    )
}

fn df_spec(base: BaseSpec, ramp: Option<RampSpec>, split: Option<SplitSpec>) -> DataflowSpec {
    DataflowSpec {
        base,
        ramp,
        split,
        billed: true,
    }
}

/// The hand-written noticer the dataflow spec stands for: B3's composition, or the base alone.
fn reference(spec: &DataflowSpec) -> NoticerSpec {
    match (spec.ramp, spec.split) {
        (None, None) => match spec.base {
            BaseSpec::Rung { notice_z } => NoticerSpec::Rung { notice_z },
            BaseSpec::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            } => NoticerSpec::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            },
        },
        (ramp, split) => NoticerSpec::Composed {
            base: spec.base,
            ramp,
            split,
        },
    }
}

fn b3_row() -> DataflowSpec {
    let (ramp, split) = b3_pieces();
    df_spec(reanchor_base(), Some(ramp), Some(split))
}

fn pair(
    spec: &DataflowSpec,
    cfg: &RungConfig,
    services: &[Service],
) -> (Box<dyn Noticer>, Box<dyn Noticer>) {
    (
        noticer::build(&reference(spec), cfg, services),
        Box::new(DataflowNoticer::new(*spec, cfg.clone(), services)),
    )
}

// ---- streams ---------------------------------------------------------------------------------------

fn held_of(sp: &StreamParams) -> (Vec<Held>, Vec<Service>) {
    let stream = generate(sp);
    let public = stream.public_info();
    let all = stream
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
    (all, public.services)
}

/// Everything two noticers did over a stream, for the tests that need more than equality.
#[derive(Debug, Default)]
struct Record {
    /// (step instant, notice) in order.
    notices: Vec<(u64, Notice)>,
    /// (step instant, anomaly id) in order.
    retired: Vec<(u64, u32)>,
    /// How many abnormal observations were offered.
    offered: u64,
}

/// A fixed-up instant and the checks of one noticer against the other.
fn same_view(a: &dyn Noticer, b: &dyn Noticer, now: u64, what: &str) {
    let (xa, xb) = (a.anomalies(), b.anomalies());
    let ids = |x: &[Tracked]| x.iter().map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(ids(xa), ids(xb), "{what}: the anomalies held, at {now}");
    for (p, q) in xa.iter().zip(xb) {
        let tag = format!("{what}: anomaly {} at {now}", p.id);
        assert_eq!(p.site, q.site, "{tag}: site");
        assert_eq!(p.region, q.region, "{tag}: region");
        assert_eq!(p.anchor, q.anchor, "{tag}: anchor");
        assert_eq!(p.anchor_at, q.anchor_at, "{tag}: anchor_at");
        assert_eq!(p.attached, q.attached, "{tag}: attached");
        assert_eq!(
            p.last_abnormal_at, q.last_abnormal_at,
            "{tag}: last_abnormal_at"
        );
        assert_eq!(p.noticed_at, q.noticed_at, "{tag}: noticed_at");
        assert_eq!(
            p.peak_score.to_bits(),
            q.peak_score.to_bits(),
            "{tag}: peak_score"
        );
        assert_eq!(p.digest(), q.digest(), "{tag}: digest");
        assert_eq!(
            a.score(p.id, Instant(now)).to_bits(),
            b.score(q.id, Instant(now)).to_bits(),
            "{tag}: score"
        );
    }
    assert_eq!(
        a.score(u32::MAX - 1, Instant(now)).to_bits(),
        b.score(u32::MAX - 1, Instant(now)).to_bits(),
        "{what}: the score of an anomaly that is not held"
    );
}

/// Play `events` through both noticers as the rung does (at each step: the observations emitted by
/// then, the abnormal ones offered to `observe`, then `notice`, the views, the retirements) and
/// compare every answer.
fn lockstep(
    events: &[Held],
    duration_ns: u64,
    a: &mut dyn Noticer,
    b: &mut dyn Noticer,
    what: &str,
) -> Record {
    let mut window: VecDeque<Held> = VecDeque::new();
    let mut record = Record::default();
    let (mut next, mut now) = (0, 0u64);
    while now < duration_ns {
        while next < events.len() && events[next].at.0 <= now {
            let h = &events[next];
            if h.abnormal {
                record.offered += 1;
                assert_eq!(
                    a.observe(h),
                    b.observe(h),
                    "{what}: observe {} at {now}",
                    h.id.0
                );
            }
            window.push_back(h.clone());
            next += 1;
        }
        while window
            .front()
            .is_some_and(|h| h.at.0.saturating_add(WINDOW_NS) < now)
        {
            window.pop_front();
        }
        let store = Store::with(window.iter().cloned());
        let (na, nb) = (
            a.notice(Instant(now), &store),
            b.notice(Instant(now), &store),
        );
        assert_eq!(na, nb, "{what}: the notices of the step at {now}");
        record.notices.extend(na.into_iter().map(|n| (now, n)));
        same_view(a, b, now, what);
        a.refresh(Instant(now));
        b.refresh(Instant(now));
        same_view(a, b, now, what);
        let (ra, rb) = (a.retirable(Instant(now)), b.retirable(Instant(now)));
        assert_eq!(ra, rb, "{what}: retirable at {now}");
        for id in ra {
            a.retire(id);
            b.retire(id);
            record.retired.push((now, id));
        }
        same_view(a, b, now, what);
        now += STEP_NS;
    }
    record
}

fn check_stream(sp: &StreamParams, spec: &DataflowSpec, cfg: &RungConfig, what: &str) -> Record {
    let (events, services) = held_of(sp);
    let (mut a, mut b) = pair(spec, cfg, &services);
    lockstep(&events, sp.duration_ns, a.as_mut(), b.as_mut(), what)
}

// ---- generated scenarios ----------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

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

/// A scenario of `dur_ms` of observations over the services of `services`, built to reach every
/// rule: bursts at a site and its dependents, ramps of one counter with strays and dips, a burst
/// at one site followed after a silence by a burst at others (the split), a lone stray followed by
/// a burst (the later re-anchor), and background of abnormal singles and benign readings.
fn scenario(seed: u64, services: &[Service], dur_ms: u64) -> Vec<Held> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let n = services.len() as u64;
    let mut evs: Vec<(u64, Observation)> = Vec::new();
    let names = CounterName::ALL;
    let pick_name = |rng: &mut Rng| names[rng.below(names.len() as u64) as usize];
    let texts = [
        SignalText::OutOfResource,
        SignalText::ServiceDown,
        SignalText::Flapping,
        SignalText::UpstreamUnreachable,
        SignalText::CheckHealth,
    ];
    let burst =
        |rng: &mut Rng, evs: &mut Vec<(u64, Observation)>, t0: u64, sites: &[u32], k: u64| {
            for j in 0..k {
                let at = t0 + rng.below(380) * MS + j;
                let s = sites[rng.below(sites.len() as u64) as usize];
                let obs = if rng.chance(1, 3) {
                    message(s, texts[rng.below(texts.len() as u64) as usize])
                } else {
                    counter(s, pick_name(rng), HIGH + rng.below(60))
                };
                evs.push((at, obs));
            }
        };
    for _ in 0..(12 + rng.below(18)) {
        let t0 = (1_000 + rng.below(dur_ms.saturating_sub(15_000))) * MS;
        let site = rng.below(n) as u32;
        let others: Vec<u32> = (0..n as u32).filter(|s| *s != site).collect();
        let deps: Vec<u32> = (site + 1..n as u32).collect();
        match rng.below(5) {
            0 => {
                let k = 2 + rng.below(6);
                let mut sites = vec![site];
                if !deps.is_empty() && rng.chance(2, 3) {
                    sites.push(deps[rng.below(deps.len() as u64) as usize]);
                }
                burst(&mut rng, &mut evs, t0, &sites, k);
            }
            1 => {
                let name = pick_name(&mut rng);
                let mut v = rng.below(30);
                let mut t = t0;
                for _ in 0..(4 + rng.below(12)) {
                    evs.push((t, counter(site, name, v)));
                    if rng.chance(1, 6) {
                        evs.push((t + rng.below(300) * MS, counter(site, name, rng.below(90))));
                    }
                    v = if rng.chance(1, 5) {
                        v.saturating_sub(rng.below(6))
                    } else {
                        v + rng.below(13)
                    };
                    t += (400 + rng.below(1_400)) * MS;
                }
            }
            2 => {
                let first = 3 + rng.below(3);
                burst(&mut rng, &mut evs, t0, &[site], first);
                let gap = 1_500 + rng.below(9_000);
                let pool = if others.is_empty() {
                    vec![site]
                } else {
                    others.clone()
                };
                let second = 2 + rng.below(4);
                burst(&mut rng, &mut evs, t0 + gap * MS, &pool, second);
                if rng.chance(1, 2) {
                    let later = t0 + (gap + 3_000 + rng.below(6_000)) * MS;
                    burst(&mut rng, &mut evs, later, &pool, 3);
                }
            }
            3 => {
                evs.push((t0, counter(site, pick_name(&mut rng), HIGH + rng.below(30))));
                let sites = if deps.is_empty() {
                    vec![site]
                } else {
                    vec![site, deps[0]]
                };
                let lag = rng.below(700);
                let k = 2 + rng.below(4);
                burst(&mut rng, &mut evs, t0 + lag * MS, &sites, k);
            }
            _ => {
                for j in 0..(3 + rng.below(8)) {
                    evs.push((
                        t0 + j * (200 + rng.below(1_300)) * MS,
                        message(site, texts[rng.below(texts.len() as u64) as usize]),
                    ));
                }
            }
        }
    }
    for _ in 0..(40 + rng.below(80)) {
        let t = rng.below(dur_ms) * MS;
        let s = rng.below(n) as u32;
        let v = if rng.chance(1, 8) {
            HIGH + rng.below(40)
        } else {
            rng.below(45)
        };
        evs.push((t, counter(s, pick_name(&mut rng), v)));
    }
    evs.sort_by_key(|(t, _)| *t);
    evs.into_iter()
        .enumerate()
        .map(|(i, (t, obs))| Held {
            id: ObsId(i as u32),
            at: Instant(t),
            abnormal: is_abnormal(&obs, services),
            obs,
        })
        .collect()
}

/// A configuration of the pieces, varied by `seed` so that the rules fire often on a short scenario.
fn varied(seed: u64) -> (DataflowSpec, RungConfig) {
    let mut rng = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
    let cfg = RungConfig {
        burst_ns: (200 + rng.below(300)) * MS,
        burst_gap_ns: (1_000 + rng.below(2_000)) * MS,
        quiet_ns: (3_000 + rng.below(4_000)) * MS,
        score_window_ns: (4_000 + rng.below(5_000)) * MS,
        ..RungConfig::default()
    };
    let z = [1.0, 1.5, 2.0, 3.0][rng.below(4) as usize];
    let base = if rng.chance(1, 2) {
        BaseSpec::Rung { notice_z: Some(z) }
    } else {
        BaseSpec::Reanchor {
            notice_z: Some(z),
            gap_ns: (10 + rng.below(400)) * MS,
            min_burst: 2 + rng.below(2) as u32,
            isolation: if rng.chance(1, 2) {
                Isolation::Site
            } else {
                Isolation::Any
            },
        }
    };
    let ramp = rng.chance(3, 4).then(|| RampSpec {
        gap_ns: (1_200 + rng.below(1_800)) * MS,
        max_step: 8 + rng.below(8) as u32,
        max_drop: rng.below(6) as u32,
        min_readings: 3 + rng.below(3) as u32,
        min_rise: 6 + rng.below(14) as u32,
        follow: None,
    });
    let split = rng.chance(3, 4).then(|| SplitSpec {
        gap_ns: (1_000 + rng.below(4_000)) * MS,
        min_burst: 2 + rng.below(2) as u32,
    });
    (df_spec(base, ramp, split), cfg)
}

// ---- the tests --------------------------------------------------------------------------------------

#[test]
fn the_spelling_roundtrips_and_a_follow_up_rule_is_refused() {
    let spec = NoticerSpec::Dataflow(b3_row());
    let text = serde_json::to_string(&spec).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["noticer"], json!("dataflow"));
    assert_eq!(v["base"]["noticer"], json!("reanchor"));
    assert!(v.get("billed").is_none(), "billed is written only when off");
    assert_eq!(serde_json::from_str::<NoticerSpec>(&text).unwrap(), spec);
    assert_eq!(spec.id(), DATAFLOW_ID);
    spec.validate().unwrap();
    let off = NoticerSpec::Dataflow(DataflowSpec {
        billed: false,
        ..b3_row()
    });
    let v: Value = serde_json::to_value(off).unwrap();
    assert_eq!(v["billed"], json!(false));
    assert_eq!(serde_json::from_value::<NoticerSpec>(v).unwrap(), off);
    let (mut ramp, _) = b3_pieces();
    ramp.follow = Some(gordian_run::stream::arms::noticer_follow::FollowSpec {
        readings: 2,
        horizon_ns: 1_000 * MS,
        min_gain: 1,
        max_fall: u32::MAX,
    });
    assert!(
        NoticerSpec::Dataflow(df_spec(reanchor_base(), Some(ramp), None))
            .validate()
            .is_err()
    );
    let bad = NoticerSpec::Dataflow(df_spec(
        BaseSpec::Reanchor {
            notice_z: None,
            gap_ns: 0,
            min_burst: 2,
            isolation: Isolation::Site,
        },
        None,
        None,
    ));
    assert!(bad.validate().is_err());
    assert!(
        serde_json::from_str::<NoticerSpec>(
            r#"{"noticer":"dataflow","base":{"noticer":"rung"},"surprise":1}"#
        )
        .is_err()
    );
}

#[test]
fn it_gives_the_hand_written_answers_on_generated_scenarios() {
    let services = public_of(&params(0, 150)).services;
    let (mut notices, mut retired, mut ramps) = (0, 0, 0);
    for seed in 1..=240u64 {
        let (spec, cfg) = varied(seed);
        let events = scenario(seed, &services, 90_000);
        let (mut a, mut b) = pair(&spec, &cfg, &services);
        let r = lockstep(
            &events,
            90_000 * MS,
            a.as_mut(),
            b.as_mut(),
            &format!("scenario {seed}"),
        );
        notices += r.notices.len();
        retired += r.retired.len();
        ramps += r
            .notices
            .iter()
            .filter(|(_, n)| n.attached.len() >= 3 && spec.ramp.is_some())
            .count();
    }
    // The scenarios reach the rules: there are notices, retirements and notices with evidence.
    assert!(notices > 400, "{notices} notices");
    assert!(retired > 200, "{retired} retirements");
    assert!(
        ramps > 100,
        "{ramps} notices with at least three observations"
    );
}

#[test]
fn it_gives_the_hand_written_answers_for_every_combination_on_real_streams() {
    let (ramp, split) = b3_pieces();
    let bases = [BaseSpec::Rung { notice_z: None }, reanchor_base()];
    for base in bases {
        for r in [None, Some(ramp)] {
            for s in [None, Some(split)] {
                let spec = df_spec(base, r, s);
                for seed in [2u64, 5] {
                    let sp = params(seed, 150);
                    check_stream(
                        &sp,
                        &spec,
                        &RungConfig::default(),
                        &format!(
                            "{base:?} ramp {} split {} seed {seed}",
                            r.is_some(),
                            s.is_some()
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn it_reproduces_the_b3_row_on_real_streams() {
    let spec = b3_row();
    let (mut notices, mut retired) = (0, 0);
    for seed in 10_000..10_006u64 {
        let r = check_stream(
            &StreamParams::new(seed),
            &spec,
            &RungConfig::default(),
            &format!("tuning seed {seed}"),
        );
        notices += r.notices.len();
        retired += r.retired.len();
    }
    assert!(
        notices > 60 && retired > 40,
        "{notices} notices, {retired} retirements"
    );
}

/// The unit's reproduction check: the notice record of B3's row equals the dataflow noticer's on
/// every stream of `first..first + count`. Also asserts the counts an honest comparison needs.
fn reproduce(first: u64, count: u64) {
    let spec = b3_row();
    let (mut notices, mut retired, mut splits_or_ramps) = (0usize, 0usize, 0usize);
    for seed in first..first + count {
        let r = check_stream(
            &StreamParams::new(seed),
            &spec,
            &RungConfig::default(),
            &format!("seed {seed}"),
        );
        notices += r.notices.len();
        retired += r.retired.len();
        splits_or_ramps += r
            .notices
            .iter()
            .filter(|(_, n)| n.attached.len() >= 5)
            .count();
    }
    eprintln!(
        "reproduced {count} streams from {first}: {notices} notices, {retired} retirements, {splits_or_ramps} notices with at least five observations"
    );
    assert!(notices > 0 && retired > 0);
}

#[test]
fn it_reproduces_the_b3_row_on_the_tuning_streams() {
    reproduce(10_000, 100);
}

#[test]
fn it_reproduces_the_b3_row_on_the_held_out_streams() {
    reproduce(20_000, 200);
}

#[test]
fn it_is_deterministic_and_counts_what_it_does() {
    let sp = params(7, 150);
    let (events, services) = held_of(&sp);
    let run = || {
        let mut n = DataflowNoticer::new(b3_row(), RungConfig::default(), &services);
        let mut twin = DataflowNoticer::new(b3_row(), RungConfig::default(), &services);
        let r = lockstep(&events, sp.duration_ns, &mut n, &mut twin, "twin");
        (r.notices, r.retired, n.counts(), twin.counts())
    };
    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(first.2, first.3);
    let c = first.2;
    assert!(
        c.probes > 0 && c.writes > 0 && c.scans > 0 && c.fires > 0,
        "{c:?}"
    );
}

#[test]
fn the_cost_it_reports_is_its_counted_work_at_the_declared_prices() {
    let sp = params(3, 150);
    let (events, services) = held_of(&sp);
    let mut n = DataflowNoticer::new(b3_row(), RungConfig::default(), &services);
    let mut billed = 0u64;
    let mut store: VecDeque<Held> = VecDeque::new();
    let (mut next, mut now) = (0, 0u64);
    while now < sp.duration_ns {
        while next < events.len() && events[next].at.0 <= now {
            if events[next].abnormal {
                n.observe(&events[next]);
            }
            store.push_back(events[next].clone());
            next += 1;
        }
        n.notice(Instant(now), &Store::with(store.iter().cloned()));
        if let Some(c) = n.take_cost() {
            assert_eq!(c.component, DATAFLOW_COMPONENT);
            billed += c.compute_ns;
        }
        now += STEP_NS;
    }
    assert!(billed > 0);
    assert_eq!(
        billed,
        n.counts().modelled_ns(&Prices::DECLARED),
        "nothing counted after the last step here"
    );
    assert!(n.take_cost().is_none(), "nothing new to report");
    // Unbilled reports nothing.
    let mut quiet = DataflowNoticer::new(
        DataflowSpec {
            billed: false,
            ..b3_row()
        },
        RungConfig::default(),
        &services,
    );
    quiet.observe(&events.iter().find(|h| h.abnormal).unwrap().clone());
    assert!(quiet.take_cost().is_none());
}

fn arm_record(seed: u64, noticer: NoticerSpec) -> gordian_run::stream::SegmentRecord {
    let p = params(seed, 150);
    let rung = RungConfig {
        noticer,
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

fn strip(log: &[noticer::NoticeLogEntry]) -> Vec<(NoticeKind, u32, u32, u32, u64, u64)> {
    log.iter()
        .map(|e| {
            (
                e.kind,
                e.anomaly,
                e.anchor.0,
                e.site.0,
                e.anchor_at.0,
                e.at.0,
            )
        })
        .collect()
}

#[test]
fn in_the_harness_the_notice_record_is_the_hand_written_arms_and_the_bill_is_charged() {
    let row = b3_row();
    for seed in [3u64, 4] {
        let hand = arm_record(seed, reference(&row));
        let df = arm_record(seed, NoticerSpec::Dataflow(row));
        let free = arm_record(
            seed,
            NoticerSpec::Dataflow(DataflowSpec {
                billed: false,
                ..row
            }),
        );
        assert_eq!(df.noticer, DATAFLOW_ID);
        assert_eq!(
            strip(&hand.notice_log),
            strip(&df.notice_log),
            "seed {seed}"
        );
        assert_eq!(
            strip(&hand.notice_log),
            strip(&free.notice_log),
            "seed {seed}, unbilled"
        );
        assert!(df.notice_log.iter().any(|e| e.kind == NoticeKind::Notice));
        let charged = |r: &gordian_run::stream::SegmentRecord| -> u64 {
            r.bill
                .by_phase(Resource::Compute)
                .filter(|(phase, _)| *phase == Phase::Component(DATAFLOW_COMPONENT))
                .map(|(_, ns)| ns)
                .sum()
        };
        assert!(charged(&df) > 0, "seed {seed}");
        assert_eq!(charged(&free), 0);
        assert_eq!(charged(&hand), 0);
        assert_eq!(
            hand.trajectory, free.trajectory,
            "seed {seed}: the unbilled arm's actions"
        );
    }
}

#[test]
fn an_arm_with_the_dataflow_noticer_writes_the_hand_written_arms_files() {
    let row = b3_row();
    let arms = [
        ("sel_hand_privileged", "oracle_selection"),
        ("sel_df_privileged", "oracle_selection"),
        ("sel_free_privileged", "oracle_selection"),
    ];
    let mut m = manifest("c1-files", &arms, 2, 150, 7);
    m.noticers
        .insert("sel_hand_privileged".to_owned(), reference(&row));
    m.noticers
        .insert("sel_df_privileged".to_owned(), NoticerSpec::Dataflow(row));
    m.noticers.insert(
        "sel_free_privileged".to_owned(),
        NoticerSpec::Dataflow(DataflowSpec { billed: false, ..row }),
    );
    m.validate().unwrap();
    let dir = scratch("c1-files");
    execute_stream(&m, &dir).unwrap();
    let without = |csv: &str, col: &str| -> Vec<String> {
        let t = rows(csv);
        let c = t[0].iter().position(|h| h == col);
        t[1..]
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|(i, _)| *i != 0 && Some(*i) != c)
                    .map(|(_, v)| v.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect()
    };
    let same = |other: &str, file: &str| {
        assert_eq!(
            without(&read(&dir.join("sel_hand_privileged"), file), "noticer"),
            without(&read(&dir.join(other), file), "noticer"),
            "{other}/{file}"
        );
    };
    // The notice record is the hand-written arm's whether or not the bill is charged; the
    // declaration instants of incidents.csv move by the microseconds the charge advances the
    // logical clock, so those are the unbilled arm's.
    for file in ["notice_events.csv", "notice_incidents.csv"] {
        same("sel_df_privileged", file);
        same("sel_free_privileged", file);
    }
    same("sel_free_privileged", "incidents.csv");
    let notices = read(&dir.join("sel_df_privileged"), "notices.csv");
    assert_eq!(cell(&notices, 0, "noticer"), DATAFLOW_ID);
}

// ---- the PI's tool for the cost columns --------------------------------------------------------------

/// A noticer that does nothing, to time the driver's own loop.
struct Null;

impl Noticer for Null {
    fn id(&self) -> &'static str {
        "null"
    }
    fn observe(&mut self, _: &Held) -> Option<u32> {
        None
    }
    fn notice(&mut self, _: Instant, _: &Store) -> Vec<Notice> {
        Vec::new()
    }
    fn anomalies(&self) -> &[Tracked] {
        &[]
    }
    fn score(&self, _: u32, _: Instant) -> f64 {
        f64::NEG_INFINITY
    }
    fn refresh(&mut self, _: Instant) {}
    fn retirable(&self, _: Instant) -> Vec<u32> {
        Vec::new()
    }
    fn retire(&mut self, _: u32) {}
}

/// Drive `n` as the rung does, timing the whole loop; returns (nanoseconds, notices, billed ns).
fn timed_replay<N: Noticer + ?Sized>(
    n: &mut N,
    events: &[Held],
    duration_ns: u64,
) -> (u64, u64, u64) {
    let start = std::time::Instant::now();
    let mut window: VecDeque<Held> = VecDeque::new();
    let (mut next, mut now, mut notices, mut billed) = (0, 0u64, 0u64, 0u64);
    while now < duration_ns {
        while next < events.len() && events[next].at.0 <= now {
            if events[next].abnormal {
                n.observe(&events[next]);
            }
            window.push_back(events[next].clone());
            next += 1;
        }
        while window
            .front()
            .is_some_and(|h| h.at.0.saturating_add(WINDOW_NS) < now)
        {
            window.pop_front();
        }
        let store = Store::with(window.iter().cloned());
        notices += n.notice(Instant(now), &store).len() as u64;
        if let Some(c) = n.take_cost() {
            billed += c.compute_ns;
        }
        n.refresh(Instant(now));
        for id in n.retirable(Instant(now)) {
            n.retire(id);
        }
        now += STEP_NS;
    }
    (start.elapsed().as_nanos() as u64, notices, billed)
}

/// The run of `f` with the least wall time, of `reps`.
fn least(reps: usize, f: &mut dyn FnMut() -> (u64, u64, u64)) -> (u64, u64, u64) {
    let mut best = f();
    for _ in 1..reps {
        let r = f();
        if r.0 < best.0 {
            best = r;
        }
    }
    best
}

/// The cost columns of C1, per stream, on the public observations of `C1_SEEDS` (`first:count`):
/// each noticer is replayed `C1_REPS` times (default 3) and the least wall time is kept, minus
/// the driver loop's own (a noticer that does nothing); the dataflow noticer's counts and the
/// medium's counts and modelled cost come from the replay, which equals the in-run charge up to
/// the tail of the last step. Written to `C1_OUT`.
#[test]
#[ignore = "a tool for the PI: writes a file; run it in release under the runner"]
fn replay_costs() {
    let seeds = std::env::var("C1_SEEDS").unwrap_or_else(|_| "20000:2".to_owned());
    let out = std::env::var("C1_OUT").unwrap_or_else(|_| "c1-costs.csv".to_owned());
    let reps: usize = std::env::var("C1_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let (first, count) = seeds.split_once(':').expect("first:count");
    let (first, count): (u64, u64) = (first.parse().unwrap(), count.parse().unwrap());
    let row = b3_row();
    let medium: MediumParams = {
        let sel = exploration("m2-selected.json");
        let mut v = sel["ticks"]["100"]["noticer"].clone();
        v.as_object_mut().unwrap().remove("noticer");
        serde_json::from_value(v).unwrap()
    };
    let mut text = String::from(
        "seed,observations,abnormal,null_ns,b3_ns,dataflow_ns,medium_ns,b3_notices,dataflow_notices,medium_notices,df_probes,df_writes,df_scans,df_fires,df_modelled_ns,df_billed_ns,med_ticks,med_cell_updates,med_synapse_traversals,med_event_routings,med_field_reads,med_modelled_ns\n",
    );
    for seed in first..first + count {
        let sp = StreamParams::new(seed);
        let (events, services) = held_of(&sp);
        let cfg = RungConfig::default();
        let (null_ns, _, _) = least(reps, &mut || {
            timed_replay(&mut Null, &events, sp.duration_ns)
        });
        let (b3_ns, b3_notices, _) = least(reps, &mut || {
            let mut n = noticer::build(&reference(&row), &cfg, &services);
            timed_replay(n.as_mut(), &events, sp.duration_ns)
        });
        let mut counts = Default::default();
        let mut modelled = 0;
        let (df_ns, df_notices, df_billed) = least(reps, &mut || {
            let mut n = DataflowNoticer::new(row, cfg.clone(), &services);
            let r = timed_replay(&mut n, &events, sp.duration_ns);
            counts = n.counts();
            modelled = n.total_ns();
            r
        });
        let mut ledger = None;
        let (med_ns, med_notices, _) = least(reps, &mut || {
            let mut n = MediumNoticer::new(medium, cfg.clone(), &services).unwrap();
            let r = timed_replay(&mut n, &events, sp.duration_ns);
            ledger = Some((
                n.ledger().total_ticks,
                n.ledger().total_counts,
                n.total_ns(),
            ));
            r
        });
        let (ticks, mc, med_modelled) = ledger.unwrap();
        writeln!(
            text,
            "{seed},{},{},{null_ns},{b3_ns},{df_ns},{med_ns},{b3_notices},{df_notices},{med_notices},{},{},{},{},{modelled},{df_billed},{ticks},{},{},{},{},{med_modelled}",
            events.len(),
            events.iter().filter(|h| h.abnormal).count(),
            counts.probes,
            counts.writes,
            counts.scans,
            counts.fires,
            mc.cell_updates,
            mc.synapse_traversals,
            mc.event_routings,
            mc.field_reads,
        )
        .unwrap();
    }
    std::fs::write(&out, text).unwrap();
}
