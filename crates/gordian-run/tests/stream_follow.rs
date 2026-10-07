//! The follow-up rule on a ramp-noticed anomaly (work item B4): the decision as a pure function of
//! the readings after the completing one, every boundary of the readings the module documentation
//! states, the rule through the rung (keep, withdraw, the record of the retirement), its identity
//! with the ramp noticer when it cannot act, and the spelling in a manifest.

mod stream_common;

use gordian_run::stream::arms::noticer::{BaseSpec, NoticeKind, NoticerSpec, RetireCause};
use gordian_run::stream::arms::noticer_follow::{FollowSpec, Follower, Seen, Verdict};
use gordian_run::stream::arms::noticer_ramp::RampSpec;
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::noticer_split::SplitSpec;
use gordian_run::stream::arms::rung::{Rung, RungConfig};
use gordian_stream::{ObsId, StreamEvent, StreamPublic};
use gordian_world::{CounterName, Observation, ServiceId};
use serde_json::json;
use stream_common::*;

const MS: u64 = 1_000_000;
const S: u64 = 1_000;

fn follow() -> FollowSpec {
    FollowSpec {
        readings: 3,
        horizon_ns: 10_000 * MS,
        min_gain: 5,
        max_fall: 4,
    }
}

fn ramp(follow: Option<FollowSpec>) -> RampSpec {
    RampSpec {
        gap_ns: 2_000 * MS,
        max_step: 12,
        max_drop: 2,
        min_readings: 4,
        min_rise: 10,
        follow,
    }
}

fn composed(ramp: Option<RampSpec>, split: Option<SplitSpec>) -> NoticerSpec {
    NoticerSpec::Composed {
        base: BaseSpec::Rung { notice_z: None },
        ramp,
        split,
    }
}

fn key() -> (ServiceId, CounterName) {
    (ServiceId(0), CounterName::Saturation)
}

fn seen(level0: u64, readings: u32, peak: u64, last: u64) -> Seen {
    Seen {
        level0,
        readings,
        peak,
        last,
    }
}

// ---- the spec and the manifest

#[test]
fn the_follow_up_rule_is_written_and_read_with_every_parameter_and_changes_the_id() {
    let value = json!({
        "noticer": "composed",
        "base": {"noticer": "rung"},
        "ramp": {
            "gap_ns": 2_000_000_000u64, "max_step": 12, "max_drop": 2, "min_readings": 4,
            "min_rise": 10,
            "follow": {"readings": 3, "horizon_ns": 10_000_000_000u64, "min_gain": 5, "max_fall": 4}
        }
    });
    let parsed: NoticerSpec = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed, composed(Some(ramp(Some(follow()))), None));
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    assert_eq!(parsed.id(), "ramp_follow");
    // Without the rule the spelling and the id are B3's, and `follow` is not written.
    let without = composed(Some(ramp(None)), None);
    let text = serde_json::to_string(&without).unwrap();
    assert!(!text.contains("follow"), "{text}");
    assert_eq!(without.id(), "ramp");
    // The ids of every composition with the rule.
    let reanchor = BaseSpec::Reanchor {
        notice_z: Some(2.0),
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    };
    let split = Some(SplitSpec {
        gap_ns: 3_000 * MS,
        min_burst: 3,
    });
    let id = |base: BaseSpec, split: Option<SplitSpec>| {
        NoticerSpec::Composed {
            base,
            ramp: Some(ramp(Some(follow()))),
            split,
        }
        .id()
    };
    assert_eq!(id(BaseSpec::Rung { notice_z: None }, None), "ramp_follow");
    assert_eq!(id(reanchor, None), "ramp_follow_reanchor");
    assert_eq!(
        id(BaseSpec::Rung { notice_z: None }, split),
        "ramp_split_follow"
    );
    assert_eq!(id(reanchor, split), "ramp_split_follow_reanchor");
    // A rule without a ramp is not a spelling: the split alone keeps its id.
    assert_eq!(composed(None, split).id(), "split");
    // An unknown parameter and a missing one are errors.
    for bad in [
        json!({"readings": 3, "horizon_ns": 1, "min_gain": 5, "max_fall": 4, "q": 1}),
        json!({"readings": 3, "horizon_ns": 1, "min_gain": 5}),
    ] {
        let mut v = value.clone();
        v["ramp"]["follow"] = bad;
        assert!(serde_json::from_value::<NoticerSpec>(v).is_err());
    }
}

#[test]
fn the_follow_up_parameters_are_checked() {
    let bad = |f: FollowSpec| composed(Some(ramp(Some(f))), None).validate().unwrap_err();
    assert!(
        bad(FollowSpec {
            readings: 0,
            ..follow()
        })
        .contains("readings")
    );
    assert!(
        bad(FollowSpec {
            horizon_ns: 0,
            ..follow()
        })
        .contains("horizon_ns")
    );
    assert!(
        composed(Some(ramp(Some(follow()))), None)
            .validate()
            .is_ok()
    );
    // The loosest values are allowed.
    assert!(
        composed(
            Some(ramp(Some(FollowSpec {
                readings: 1,
                horizon_ns: 1,
                min_gain: 0,
                max_fall: u32::MAX,
            }))),
            None
        )
        .validate()
        .is_ok()
    );
}

// ---- the decision

#[test]
fn a_reversal_withdraws_at_once_and_a_fall_of_exactly_max_fall_does_not() {
    let f = follow(); // readings 3, min_gain 5, max_fall 4
    // One follow-up reading, 4 below the peak: not a reversal, and too few readings to decide.
    assert_eq!(f.judge(&seen(40, 1, 40, 36)), None);
    // 5 below the peak is.
    assert_eq!(f.judge(&seen(40, 1, 40, 35)), Some(Verdict::Withdraw));
    // The peak is the highest since the completing reading, the follow-up readings included.
    assert_eq!(f.judge(&seen(40, 2, 52, 48)), None);
    assert_eq!(f.judge(&seen(40, 2, 52, 47)), Some(Verdict::Withdraw));
    // No follow-up reading yet: a reversal cannot be seen, and nothing decides.
    assert_eq!(f.judge(&seen(40, 0, 40, 40)), None);
    // A rule that tolerates every fall never withdraws on one.
    let loose = FollowSpec {
        max_fall: u32::MAX,
        ..f
    };
    assert_eq!(loose.judge(&seen(40, 1, 100, 0)), None);
}

#[test]
fn after_the_window_the_latest_reading_decides_and_min_gain_is_inclusive() {
    let f = follow();
    // Three follow-up readings, the latest 5 above the completing reading's 40: kept.
    assert_eq!(f.judge(&seen(40, 3, 45, 45)), Some(Verdict::Keep));
    // 4 above: withdrawn.
    assert_eq!(f.judge(&seen(40, 3, 44, 44)), Some(Verdict::Withdraw));
    // The latest reading, not the best: 60 then 42 is a fall of 18 (a reversal) at any window, and
    // 60 then 47 (a fall of 13) is too. With a tolerant `max_fall` the latest is what is judged.
    let tolerant = FollowSpec { max_fall: 100, ..f };
    assert_eq!(
        tolerant.judge(&seen(40, 3, 60, 42)),
        Some(Verdict::Withdraw)
    );
    assert_eq!(tolerant.judge(&seen(40, 3, 60, 45)), Some(Verdict::Keep));
    // Two readings of three: undecided.
    assert_eq!(f.judge(&seen(40, 2, 50, 50)), None);
    // `min_gain` zero keeps a counter that is where it was.
    let flat = FollowSpec { min_gain: 0, ..f };
    assert_eq!(flat.judge(&seen(40, 3, 40, 40)), Some(Verdict::Keep));
    assert_eq!(flat.judge(&seen(40, 3, 41, 39)), Some(Verdict::Withdraw));
    // One reading is a window of one.
    let one = FollowSpec { readings: 1, ..f };
    assert_eq!(one.judge(&seen(40, 1, 46, 46)), Some(Verdict::Keep));
    assert_eq!(one.judge(&seen(40, 1, 44, 44)), Some(Verdict::Withdraw));
}

#[test]
fn at_the_horizon_none_withdraws_and_some_are_judged_on_the_latest() {
    let f = follow();
    assert_eq!(f.settle(&seen(40, 0, 40, 40)), Verdict::Withdraw);
    // With no reading even a `min_gain` of zero does not keep: nothing continued the counter.
    let flat = FollowSpec { min_gain: 0, ..f };
    assert_eq!(flat.settle(&seen(40, 0, 40, 40)), Verdict::Withdraw);
    assert_eq!(f.settle(&seen(40, 1, 45, 45)), Verdict::Keep);
    assert_eq!(f.settle(&seen(40, 2, 45, 44)), Verdict::Withdraw);
}

// ---- the follower

#[test]
fn a_reading_is_a_follow_up_reading_of_the_watches_opened_before_it_and_only_at_its_key() {
    let mut fl = Follower::new(follow());
    fl.open(7, key(), 40, at(10_000));
    assert_eq!((fl.opened(), fl.watching()), (1, 1));
    // Another counter, and another service, are not follow-up readings.
    assert!(
        fl.feed((ServiceId(0), CounterName::Latency), 5).is_empty(),
        "another counter"
    );
    assert!(
        fl.feed((ServiceId(1), CounterName::Saturation), 5)
            .is_empty()
    );
    // Three readings at the key, rising: the third decides, keep.
    assert!(fl.feed(key(), 44).is_empty());
    assert!(fl.feed(key(), 48).is_empty());
    assert_eq!(fl.feed(key(), 50), vec![(7, Verdict::Keep)]);
    assert_eq!(fl.watching(), 0);
    assert!(!fl.is_withdrawn(7));
    // A decided watch takes no more readings.
    assert!(fl.feed(key(), 0).is_empty());
}

#[test]
fn the_completing_reading_is_part_of_the_peak_and_a_reversal_needs_a_follow_up_reading() {
    let mut fl = Follower::new(follow()); // max_fall 4
    fl.open(3, key(), 40, at(10_000));
    // The first follow-up reading is 10 below the completing one: a reversal at once, so the
    // completing reading counts in the peak (a peak that began at zero would see no fall here).
    assert_eq!(fl.feed(key(), 30), vec![(3, Verdict::Withdraw)]);
    // The guard is on the number of follow-up readings, not only on the values: with none, no
    // reversal is seen, whatever the peak and last values a caller hands `judge`.
    assert_eq!(follow().judge(&seen(40, 0, 100, 0)), None);
}

#[test]
fn a_follower_counts_the_watches_it_opened_and_the_ones_still_undecided() {
    let mut fl = Follower::new(follow());
    assert_eq!((fl.opened(), fl.watching()), (0, 0));
    fl.open(1, key(), 40, at(10_000));
    fl.open(2, key(), 40, at(11_000));
    assert_eq!((fl.opened(), fl.watching()), (2, 2));
    // A decision closes a watch and does not reduce the count of those opened.
    assert_eq!(fl.expire(at(25_000)).len(), 2);
    assert_eq!((fl.opened(), fl.watching()), (2, 0));
}

#[test]
fn a_reversal_withdraws_before_the_window_ends_and_the_anomaly_is_listed_until_it_is_forgotten() {
    let mut fl = Follower::new(follow());
    fl.open(3, key(), 40, at(10_000));
    assert!(fl.feed(key(), 42).is_empty());
    assert_eq!(fl.feed(key(), 30), vec![(3, Verdict::Withdraw)]);
    assert!(fl.is_withdrawn(3));
    assert_eq!(fl.withdrawn().collect::<Vec<_>>(), vec![3]);
    // The rung has retired it: it is no longer tracked, and the follower forgets it.
    fl.retain(|a| a != 3);
    assert!(!fl.is_withdrawn(3));
    assert_eq!(fl.withdrawn().count(), 0);
}

#[test]
fn two_watches_at_one_key_each_decide_on_their_own_readings() {
    let mut fl = Follower::new(FollowSpec {
        readings: 1,
        ..follow()
    });
    fl.open(1, key(), 40, at(10_000));
    assert_eq!(fl.feed(key(), 50), vec![(1, Verdict::Keep)]);
    // A second ramp at the same key, opened later, starts from its own completing reading.
    fl.open(2, key(), 60, at(30_000));
    assert_eq!(fl.feed(key(), 62), vec![(2, Verdict::Withdraw)]);
    assert!(!fl.is_withdrawn(1));
    assert!(fl.is_withdrawn(2));
    // Both opened in one step see the same reading and decide in the order they were opened.
    let mut fl = Follower::new(FollowSpec {
        readings: 1,
        ..follow()
    });
    fl.open(5, key(), 10, at(1_000));
    fl.open(4, key(), 70, at(1_000));
    assert_eq!(
        fl.feed(key(), 20),
        vec![(5, Verdict::Keep), (4, Verdict::Withdraw)]
    );
}

#[test]
fn the_horizon_decides_a_watch_that_has_too_few_readings() {
    let mut fl = Follower::new(follow()); // horizon 10 s
    fl.open(1, key(), 40, at(10_000));
    fl.open(2, key(), 40, at(10_000));
    fl.open(3, (ServiceId(1), CounterName::Saturation), 40, at(10_000));
    // Two readings at the first key, none at the other.
    fl.feed(key(), 46);
    fl.feed(key(), 47);
    assert!(fl.expire(at(19_999)).is_empty(), "not yet");
    // At exactly the horizon the watches decide: 1 and 2 saw 46 and 47 (kept, 47 is 7 above), 3
    // saw nothing (withdrawn).
    let v = fl.expire(at(20_000));
    assert_eq!(
        v,
        vec![
            (1, Verdict::Keep),
            (2, Verdict::Keep),
            (3, Verdict::Withdraw)
        ]
    );
    assert_eq!(fl.watching(), 0);
    assert!(fl.is_withdrawn(3) && !fl.is_withdrawn(1));
    // Nothing is decided twice.
    assert!(fl.expire(at(30_000)).is_empty());
}

#[test]
fn retain_forgets_the_watches_of_anomalies_that_are_gone() {
    let mut fl = Follower::new(follow());
    fl.open(1, key(), 40, at(0));
    fl.open(2, key(), 40, at(0));
    fl.retain(|a| a == 2);
    assert_eq!(fl.watching(), 1);
    fl.feed(key(), 0);
    fl.feed(key(), 0);
    fl.feed(key(), 0);
    assert!(fl.is_withdrawn(2));
}

// ---- through the rung

/// A rung fed the observations of each step, with ids running on across steps.
struct Run {
    rung: Rung,
    next: u32,
}

impl Run {
    fn new(public: &StreamPublic, noticer: NoticerSpec) -> Self {
        Self {
            rung: Rung::new(
                public,
                RungConfig {
                    noticer,
                    ..RungConfig::default()
                },
            ),
            next: 0,
        }
    }

    fn step(&mut self, now_ms: u64, observations: &[(u64, Observation)]) {
        let events: Vec<StreamEvent> = observations
            .iter()
            .map(|(ms, obs)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: obs.clone(),
                }
            })
            .collect();
        self.rung.absorb(&events, &[], at(now_ms));
        self.rung.notice(at(now_ms));
    }
}

fn readings(t0: u64, values: &[u64]) -> Vec<(u64, Observation)> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                t0 + 1_200 * i as u64,
                Observation::Counter {
                    service: ServiceId(0),
                    name: CounterName::Saturation,
                    value: *v,
                },
            )
        })
        .collect()
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

/// Crossing at the fourth reading (39, the rise 11 over the first 28): the follow-up readings are
/// the ones after it.
const KEEPS_RISING: [u64; 10] = [28, 29, 33, 39, 44, 48, 50, 55, 60, 64];
/// Rises and falls back: 40, 36, then 30 (a fall of 10 from the peak of 40, beyond `max_fall` 4).
const TURNS: [u64; 8] = [28, 29, 33, 39, 40, 36, 30, 22];

#[test]
fn a_ramp_that_keeps_rising_is_kept_and_one_that_turns_is_withdrawn_and_retired_by_the_rule() {
    let public = public_of(&params(0, 150));
    let spec = composed(Some(ramp(Some(follow()))), None);
    // Keeps rising: after three follow-up readings (44, 48, 50) the rule keeps it. Nothing is
    // retirable while readings keep arriving.
    let mut run = Run::new(&public, spec);
    let obs = readings(10_000, &KEEPS_RISING);
    play(&mut run, &obs, 24 * S);
    assert_eq!(run.rung.noticed_total(), 1);
    assert!(run.rung.quiet(at(24 * S)).is_empty(), "kept: not retirable");
    // Turns: the third follow-up reading (30) is a reversal; the anomaly is retirable at that step,
    // well before its 6 s of quiet.
    let mut run = Run::new(&public, spec);
    let obs = readings(10_000, &TURNS);
    // Readings at 10.0, 11.2, 12.4, 13.6 (the completing one), 14.8 (40), 16.0 (36), 17.2 (30).
    play(&mut run, &obs, 16_500);
    assert_eq!(run.rung.noticed_total(), 1);
    assert!(
        run.rung.quiet(at(16_500)).is_empty(),
        "36 is 4 below the peak 40: not yet a reversal"
    );
    play_more(&mut run, &obs, 16_500, 17_500);
    let ids = run.rung.quiet(at(17_500));
    assert_eq!(ids.len(), 1, "the anomaly is retirable once 30 is read");
    run.rung.retire(ids[0]);
    let log = run.rung.notice_log();
    let notices = log.iter().filter(|e| e.kind == NoticeKind::Notice).count();
    assert_eq!(notices, 1, "the notice stays on the record");
    let retire = log.iter().find(|e| e.kind == NoticeKind::Retire).unwrap();
    assert_eq!(retire.cause, Some(RetireCause::Followup));
    assert_eq!(retire.at, at(17_500));
    assert!(
        log.iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .all(|e| e.cause.is_none())
    );
}

/// Continue `run` from where `play` stopped at `from_ms` to `until_ms`, with the same 500 ms steps.
fn play_more(run: &mut Run, obs: &[(u64, Observation)], from_ms: u64, until_ms: u64) {
    let mut now = from_ms + 500;
    while now <= until_ms {
        let batch: Vec<(u64, Observation)> = obs
            .iter()
            .filter(|(ms, _)| *ms <= now && *ms > now - 500)
            .cloned()
            .collect();
        run.step(now, &batch);
        now += 500;
    }
}

#[test]
fn the_same_series_without_the_rule_retires_only_when_it_has_been_quiet() {
    let public = public_of(&params(0, 150));
    let mut run = Run::new(&public, composed(Some(ramp(None)), None));
    let obs = readings(10_000, &TURNS);
    play(&mut run, &obs, 17_500);
    assert!(run.rung.quiet(at(17_500)).is_empty());
    // The last reading is at 10 + 1.2 * 7 = 18.4 s; quiet 6 s later.
    play_more(&mut run, &obs, 17_500, 24_500);
    let ids = run.rung.quiet(at(24_500));
    assert_eq!(ids.len(), 1);
    run.rung.retire(ids[0]);
    let retire = run
        .rung
        .notice_log()
        .iter()
        .find(|e| e.kind == NoticeKind::Retire)
        .unwrap();
    assert_eq!(retire.cause, Some(RetireCause::Quiet));
}

#[test]
fn a_ramp_that_stops_is_withdrawn_at_the_horizon_before_it_would_have_been_quiet() {
    let public = public_of(&params(0, 150));
    // The ramp completes at 13.6 s and nothing follows. The rung retires a quiet anomaly 6 s after
    // its last observation (19.6 s); a horizon of 4 s withdraws it at 17.6 s, and the record says
    // the rule did it.
    let short = FollowSpec {
        horizon_ns: 4_000 * MS,
        ..follow()
    };
    let mut run = Run::new(&public, composed(Some(ramp(Some(short))), None));
    let obs = readings(10_000, &KEEPS_RISING[..4]);
    play(&mut run, &obs, 17_000);
    assert!(run.rung.quiet(at(17_000)).is_empty(), "before the horizon");
    play_more(&mut run, &obs, 17_000, 18_000);
    let ids = run.rung.quiet(at(18_000));
    assert_eq!(ids.len(), 1, "withdrawn at the horizon, not yet quiet");
    run.rung.retire(ids[0]);
    let retire = run
        .rung
        .notice_log()
        .iter()
        .find(|e| e.kind == NoticeKind::Retire)
        .unwrap();
    assert_eq!(retire.cause, Some(RetireCause::Followup));
}

#[test]
fn the_rule_watches_only_what_the_ramp_noticer_opens() {
    // The rung's own readings at or over 50 open anomalies of the base's, which the follower does
    // not know: a series that the base notices and the ramp does not (too slow a rise) is never
    // withdrawn, whatever follows it.
    let public = public_of(&params(0, 150));
    let slow = ramp(Some(FollowSpec {
        readings: 1,
        horizon_ns: 1,
        min_gain: 100,
        max_fall: 0,
    }));
    let spec = composed(
        Some(RampSpec {
            min_rise: 1_000,
            ..slow
        }),
        None,
    );
    let mut run = Run::new(&public, spec);
    let obs = readings(10_000, &KEEPS_RISING);
    play(&mut run, &obs, 30 * S);
    let followed = run
        .rung
        .notice_log()
        .iter()
        .filter(|e| e.cause == Some(RetireCause::Followup))
        .count();
    assert_eq!(followed, 0);
}

// ---- whole segments

#[test]
fn a_follow_up_rule_that_cannot_act_is_the_ramp_noticer_in_whole_segments() {
    // A ramp that can never cross never opens an anomaly, so the rule watches nothing: the notice
    // record, the retirements, the results and the selection files are the ramp's without it.
    let never = RampSpec {
        min_rise: u32::MAX,
        ..ramp(None)
    };
    let mut m = manifest(
        "follow-identity",
        &[("plain", "never_escalate"), ("with", "never_escalate")],
        2,
        200,
        9,
    );
    for arm in &mut m.arms {
        arm.policy = StreamPolicySpec::PublicChange {
            delay_ns: 1_000 * MS,
            k: 3,
        };
    }
    m.noticers
        .insert("plain".to_owned(), composed(Some(never), None));
    m.noticers.insert(
        "with".to_owned(),
        composed(
            Some(RampSpec {
                follow: Some(follow()),
                ..never
            }),
            None,
        ),
    );
    m.validate().unwrap();
    let out = scratch("follow-identity").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    for file in [
        "results.csv",
        "incidents.csv",
        "notice_events.csv",
        "selection.csv",
        "selection_notices.csv",
    ] {
        let a = without_run_id(&read(&out.join("plain"), file));
        // The noticer's id differs by name ("ramp_follow" against "ramp"): compare with it removed.
        let b: Vec<String> = without_run_id(&read(&out.join("with"), file))
            .into_iter()
            .map(|r| r.replace("ramp_follow", "ramp"))
            .collect();
        assert_eq!(a, b, "{file}");
    }
    assert_eq!(
        cell(&read(&out.join("with"), "notices.csv"), 0, "noticer"),
        "ramp_follow"
    );
}

#[test]
fn a_withdrawn_anomaly_is_written_as_a_follow_up_retirement_and_a_call_is_stamped_with_its_step() {
    // A rule no reading satisfies withdraws every anomaly the ramp opens, after one follow-up reading
    // or at the horizon; the same ramp without it never retires by the rule. Both arms ask at 1 s
    // about any anomaly the rung has not resolved, so some notices are asked about.
    let eager = FollowSpec {
        readings: 1,
        horizon_ns: 4_000 * MS,
        min_gain: u32::MAX,
        max_fall: u32::MAX,
    };
    let mut m = manifest(
        "follow-files",
        &[("plain", "never_escalate"), ("with", "never_escalate")],
        6,
        600,
        3,
    );
    for arm in &mut m.arms {
        arm.policy = StreamPolicySpec::PublicThreshold {
            delay_ns: 1_000 * MS,
            persist_ns: 0,
        };
    }
    m.noticers
        .insert("plain".to_owned(), composed(Some(ramp(None)), None));
    m.noticers
        .insert("with".to_owned(), composed(Some(ramp(Some(eager))), None));
    m.validate().unwrap();
    let step_ns = m.limits.step_ns;
    let out = scratch("follow-files").join("run");
    execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    let column = |t: &[Vec<String>], name: &str| t[0].iter().position(|c| c == name).unwrap();
    let followed = |arm: &str| {
        let t = rows(&read(&out.join(arm), "selection_notices.csv"));
        let c = column(&t, "retire_cause");
        t[1..].iter().filter(|r| r[c] == "followup").count() as u64
    };
    let counted = |arm: &str| {
        let t = rows(&read(&out.join(arm), "selection.csv"));
        let cols: Vec<usize> = t[0]
            .iter()
            .enumerate()
            .filter(|(_, c)| c.starts_with("followup_retired_"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(cols.len(), 5);
        t[1..]
            .iter()
            .flat_map(|r| cols.iter().map(|&i| r[i].parse::<u64>().unwrap()))
            .sum::<u64>()
    };
    assert_eq!(followed("plain"), 0);
    assert_eq!(counted("plain"), 0);
    assert!(
        followed("with") > 0,
        "the ramp opened an anomaly in six segments, and the rule withdrew it"
    );
    // The per-notice cause and the per-stream count are the same retirements.
    assert_eq!(followed("with"), counted("with"));
    // A call is stamped with the instant of the step that made it, which is a multiple of the step
    // quantum, and not with the clock after the arm's metered work.
    let mut asked = 0;
    for arm in ["plain", "with"] {
        let t = rows(&read(&out.join(arm), "selection_notices.csv"));
        let c = column(&t, "first_escalation_at_ns");
        for r in &t[1..] {
            if !r[c].is_empty() {
                asked += 1;
                assert_eq!(r[c].parse::<u64>().unwrap() % step_ns, 0, "{arm}: {}", r[c]);
            }
        }
    }
    assert!(asked > 0, "some notice was asked about");
}

use gordian_run::stream::execute_stream;
use gordian_run::stream::spec::StreamPolicySpec;
