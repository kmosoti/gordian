//! The noticing seam (work item B1): the default noticer is the rung's own, the two other noticers
//! do what their documentation says, the record of notices reaches the run output, and none of it
//! touches `results.csv` or `incidents.csv`.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`experiments/exploration/scripts/b1_gate.py`); the tests here pin the same property on small
//! streams.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::noticer::{NoticeKind, NoticerSpec};
use gordian_run::stream::arms::rung::{Rung, RungConfig};
use gordian_run::stream::execute_stream;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::spec::StreamPolicySpec;
use gordian_stream::{ObsId, StreamEvent, StreamPublic};
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use stream_common::*;

const S: u64 = 1_000; // milliseconds in a second

/// The headers of `results.csv` and `incidents.csv` as they were before the seam: the columns the
/// analysis loader's schema guard and every earlier experiment's hashes depend on.
const PRE_SEAM_RESULTS_HEADER: &str = "run_id,arm_role,seed,duration_ns,observations,anomalies_noticed,probes_used,declarations,declared_incident,declared_dismissal,incidents_plain,incidents_hard,incidents_decoy,critical_incidents,correct_plain,correct_hard,missed_plain,missed_hard,critical_missed_plain,critical_missed_hard,wrong_declarations,decoys_dismissed,decoys_alarmed,decoys_silent,false_alarms,false_alarms_on_background,escalations_needed,escalations_unneeded,escalations_background,hard_incidents_escalated,other_incidents_escalated,calls_informed,calls_correct,reasoner_calls,reasoner_refs,reasoner_tokens,reasoner_modelled_ns,reasoner_latency_ns,cheap_declarations,reasoner_declarations,escalations_refused,probes_refused,calls_unanswered,reasoner_cost_ns,bill_compute,bill_probes,bill_time,bill_comm,components_run,components_skipped,rule_skipped,steps,stop_reason,ops_component,ops_sched,modelled_component_ns,modelled_sched_ns,substrate_ns,total_cost_ns";
const PRE_SEAM_INCIDENTS_HEADER: &str = "run_id,arm_role,seed,incident,tier,family,critical,correct_declarations,wrong_declarations,first_correct_at_ns,time_to_first_correct_ns,correct_by_deadline,missed,critical_miss,escalations,informed_escalations,correct_escalations";

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

/// Feeds a rung with the noticer `spec` observations at stated instants, and notices.
struct Feed {
    next: u32,
    rung: Rung,
}

impl Feed {
    fn new(public: &StreamPublic, spec: NoticerSpec) -> Self {
        let cfg = RungConfig {
            noticer: spec,
            ..RungConfig::default()
        };
        Self {
            next: 0,
            rung: Rung::new(public, cfg),
        }
    }

    /// Deliver `observations` (instants in milliseconds) in one step at `now_ms`, then notice.
    /// Returns the ids of the observations, in order.
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

    fn notices(&self) -> Vec<(u32, ObsId, Instant, Instant)> {
        self.rung
            .notice_log()
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .map(|e| (e.anomaly, e.anchor, e.anchor_at, e.at))
            .collect()
    }
}

fn spec_change(q_s: u64) -> NoticerSpec {
    NoticerSpec::ChangeTriggered {
        quiet_ns: q_s * 1_000_000_000,
    }
}

fn spec_earliest(l_ms: u64) -> NoticerSpec {
    NoticerSpec::EarliestAnchor {
        lookback_ns: l_ms * 1_000_000,
    }
}

/// Two services where the second depends on the first (the first's dependent), and a stranger.
fn picks(public: &StreamPublic) -> (ServiceId, ServiceId, ServiceId) {
    use gordian_world::graph::dependents_mask;
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

fn burst(site: ServiceId, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 90)),
        (t + 20, message(site, SignalText::OutOfResource)),
        (t + 30, counter(site, CounterName::Saturation, 90)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
    ]
}

// ---- the spec and the manifest

#[test]
fn the_default_noticer_is_the_rungs_and_is_never_written_to_a_manifest() {
    assert!(NoticerSpec::default().is_default());
    assert!(RungConfig::default().noticer.is_default());
    let m = all_arms("noticer-default", 1, 150, 0);
    let value: Value = serde_json::from_str(&m.canonical_json()).unwrap();
    assert!(value.get("noticers").is_none());
    assert!(value["rung"].get("noticer").is_none());
    // A manifest written before the seam existed parses to the same manifest.
    let back: StreamManifest = serde_json::from_value(value).unwrap();
    assert_eq!(back, m);
    // A rung built from it notices as the rung does.
    let public = public_of(&params(0, 150));
    assert_eq!(
        Rung::new(&public, RungConfig::default()).noticer_id(),
        "rung"
    );
}

#[test]
fn a_noticer_is_written_and_read_as_an_object_tagged_by_its_name() {
    for (value, spec) in [
        (json!({"noticer": "rung"}), NoticerSpec::default()),
        (
            json!({"noticer": "rung", "notice_z": 2.0}),
            NoticerSpec::Rung {
                notice_z: Some(2.0),
            },
        ),
        (
            json!({"noticer": "change_triggered", "quiet_ns": 8_000_000_000u64}),
            spec_change(8),
        ),
        (
            json!({"noticer": "earliest_anchor", "lookback_ns": 500_000_000u64}),
            spec_earliest(500),
        ),
    ] {
        let parsed: NoticerSpec = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(parsed, spec);
        assert_eq!(serde_json::to_value(spec).unwrap(), value);
    }
    // A parameter a noticer does not have is an error.
    assert!(
        serde_json::from_value::<NoticerSpec>(json!({"noticer": "rung", "quiet_ns": 1})).is_err()
    );
    assert!(serde_json::from_value::<NoticerSpec>(json!({"noticer": "nothing"})).is_err());
    assert_eq!(spec_change(1).id(), "change_triggered");
    assert_eq!(spec_earliest(1).id(), "earliest_anchor");
}

#[test]
fn a_manifest_gives_a_noticer_to_an_arm_that_exists_and_each_arms_own_manifest_keeps_only_its_own()
{
    let mut m = all_arms("noticer-map", 1, 150, 0);
    m.noticers
        .insert("never_escalate".to_owned(), spec_change(8));
    m.validate().unwrap();
    let text = m.canonical_json();
    let back: StreamManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    let first = &m.arms[0];
    let second = &m.arms[1];
    assert_eq!(m.rung_for(first).noticer, spec_change(8));
    assert!(m.rung_for(second).noticer.is_default());
    // The arm's own manifest carries its noticer, another arm's carries none.
    assert_eq!(m.single_arm(0).noticers.len(), 1);
    assert!(m.single_arm(1).noticers.is_empty());
    assert_eq!(
        m.single_arm(0).rung_for(&m.single_arm(0).arms[0]).noticer,
        spec_change(8)
    );
    // An arm that is not in the manifest, or a noticer with a bad parameter, is refused.
    let mut bad = m.clone();
    bad.noticers
        .insert("no_such_arm".to_owned(), spec_change(8));
    assert!(bad.validate().unwrap_err().contains("not an arm"));
    let mut bad = m.clone();
    bad.noticers.insert(
        "never_escalate".to_owned(),
        NoticerSpec::Rung {
            notice_z: Some(f64::NAN),
        },
    );
    assert!(bad.validate().is_err());
}

// ---- the rung's own noticer at another threshold

#[test]
fn the_rung_noticer_at_a_lower_threshold_notices_a_smaller_burst() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let four = &burst(site, 9_800)[..4];
    let mut default = Feed::new(&public, NoticerSpec::default());
    default.step(10 * S, four);
    assert!(
        default.notices().is_empty(),
        "four observations are below z = 3"
    );
    let mut z2 = Feed::new(
        &public,
        NoticerSpec::Rung {
            notice_z: Some(2.0),
        },
    );
    z2.step(10 * S, four);
    assert_eq!(z2.notices().len(), 1, "four observations cross z = 2");
    let mut z3 = Feed::new(
        &public,
        NoticerSpec::Rung {
            notice_z: Some(3.0),
        },
    );
    z3.step(10 * S, four);
    assert!(z3.notices().is_empty(), "an explicit 3.0 is the default");
    assert_eq!(z2.rung.noticer_id(), "rung");
}

// ---- ChangeTriggered

#[test]
fn change_triggered_notices_the_first_abnormal_observation_at_a_quiet_node_and_anchors_there() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let mut feed = Feed::new(&public, spec_change(10));
    let ids = feed.step(
        2 * S,
        &[
            (S, counter(site, CounterName::ErrorRate, 90)),
            (1_500, counter(site, CounterName::Latency, 90)),
        ],
    );
    let notices = feed.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].1, ids[0], "anchored on the first");
    assert_eq!(notices[0].2, at(S));
    assert_eq!(
        notices[0].3,
        at(2 * S),
        "noticed at the step that delivered it"
    );
    let views = feed.rung.views(at(2 * S));
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].site, site);
    assert_eq!(views[0].anchor, ids[0]);
    assert_eq!(
        feed.rung.attached(views[0].id).len(),
        2,
        "the second joined it"
    );
    assert_eq!(feed.rung.noticer_id(), "change_triggered");
}

#[test]
fn change_triggered_needs_a_benign_observation_to_be_ignored_and_the_quiet_period_to_have_passed() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let mut feed = Feed::new(&public, spec_change(10));
    // A counter below the alarm level is not abnormal: no notice.
    feed.step(S, &[(500, counter(site, CounterName::ErrorRate, HIGH - 1))]);
    assert!(feed.notices().is_empty());
    feed.step(2 * S, &[(S, counter(site, CounterName::ErrorRate, 90))]);
    assert_eq!(feed.notices().len(), 1);
    // One millisecond short of the quiet period since the last abnormal observation (at 1 s):
    // not a notice.
    feed.step(11 * S, &[(10_999, counter(site, CounterName::Latency, 90))]);
    assert_eq!(
        feed.notices().len(),
        1,
        "10.999 s after the last is not quiet enough"
    );
    // Exactly the quiet period after the last abnormal observation (at 10.999 s): a notice.
    feed.step(
        22 * S,
        &[(20_999, counter(site, CounterName::Saturation, 90))],
    );
    assert_eq!(
        feed.notices().len(),
        2,
        "a quiet period of exactly q is quiet"
    );
    let n = feed.notices();
    assert_eq!(n[1].2, at(20_999));
}

#[test]
fn change_triggered_makes_one_notice_per_quiet_node_in_a_cascade() {
    let public = public_of(&params(0, 150));
    let (site, dependent, stranger) = picks(&public);
    let mut feed = Feed::new(&public, spec_change(10));
    feed.step(
        2 * S,
        &[
            (S, counter(site, CounterName::ErrorRate, 90)),
            (1_100, counter(dependent, CounterName::ErrorRate, 80)),
            (1_200, counter(stranger, CounterName::ErrorRate, 80)),
        ],
    );
    let n = feed.notices();
    assert_eq!(n.len(), 3, "each node was quiet: {n:?}");
    let sites: Vec<ServiceId> = feed.rung.views(at(2 * S)).iter().map(|v| v.site).collect();
    assert_eq!(sites, vec![site, dependent, stranger]);
}

#[test]
fn change_triggered_retires_a_quiet_anomaly_and_records_it() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    let mut feed = Feed::new(&public, spec_change(10));
    feed.step(2 * S, &[(S, counter(site, CounterName::ErrorRate, 90))]);
    let id = feed.rung.views(at(2 * S))[0].id;
    assert!(feed.rung.quiet(at(6 * S)).is_empty());
    assert_eq!(
        feed.rung.quiet(at(7 * S)),
        vec![id],
        "6 s after its last abnormal observation"
    );
    feed.rung.retire(id);
    let log = feed.rung.notice_log();
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].kind, NoticeKind::Retire);
    assert_eq!(log[1].anomaly, id);
    assert!(feed.rung.views(at(7 * S)).is_empty());
}

// ---- EarliestAnchor

fn stray_then_burst(feed: &mut Feed, site: ServiceId, stray_ms: u64) -> Vec<ObsId> {
    let mut obs = vec![(stray_ms, counter(site, CounterName::Latency, 70))];
    obs.extend(burst(site, 9_800));
    feed.step(10 * S, &obs)
}

#[test]
fn earliest_anchor_moves_the_anchor_to_the_earliest_abnormal_observation_at_the_site_in_reach() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    // The rung anchors on the burst at 9.8 s and drops the stray 1.5 s before it from the evidence.
    let mut rung = Feed::new(&public, NoticerSpec::default());
    let ids = stray_then_burst(&mut rung, site, 8_300);
    assert_eq!(rung.notices()[0].1, ids[1], "the rung anchors on the burst");
    // With a lookback of 2 s the stray is within reach.
    let mut early = Feed::new(&public, spec_earliest(2_000));
    let ids = stray_then_burst(&mut early, site, 8_300);
    let n = early.notices();
    assert_eq!(n.len(), 1);
    assert_eq!(
        n[0].1, ids[0],
        "anchored on the earlier observation at the site"
    );
    assert_eq!(n[0].2, at(8_300));
    let view = early.rung.views(at(10 * S)).remove(0);
    assert_eq!(view.anchor, ids[0]);
    assert_eq!(view.site, site);
    let attached = early.rung.attached(view.id);
    assert_eq!(
        attached[0].1, ids[0],
        "the anchor is the first attached observation"
    );
    assert_eq!(attached.len(), 6, "the stray and the five of the burst");
    assert_eq!(early.rung.noticer_id(), "earliest_anchor");
}

#[test]
fn earliest_anchor_reach_is_inclusive_and_a_lookback_of_zero_is_the_rung() {
    let public = public_of(&params(0, 150));
    let (site, _, _) = picks(&public);
    // The stray is exactly 1 s before the burst's first observation (9.8 s): in reach of 1 s.
    let mut exact = Feed::new(&public, spec_earliest(1_000));
    let ids = stray_then_burst(&mut exact, site, 8_800);
    assert_eq!(exact.notices()[0].1, ids[0]);
    // One millisecond further: out of reach.
    let mut short = Feed::new(&public, spec_earliest(1_000));
    let ids = stray_then_burst(&mut short, site, 8_799);
    assert_eq!(short.notices()[0].1, ids[1]);
    // A lookback of zero is exactly the rung: the same notice, anchor and evidence.
    let mut zero = Feed::new(&public, spec_earliest(0));
    let mut rung = Feed::new(&public, NoticerSpec::default());
    stray_then_burst(&mut zero, site, 8_300);
    stray_then_burst(&mut rung, site, 8_300);
    assert_eq!(zero.notices(), rung.notices());
    let (a, b) = (zero.rung.views(at(10 * S)), rung.rung.views(at(10 * S)));
    assert_eq!(a.len(), 1);
    assert_eq!((a[0].anchor, a[0].digest), (b[0].anchor, b[0].digest));
    assert_eq!(zero.rung.attached(a[0].id), rung.rung.attached(b[0].id));
}

#[test]
fn earliest_anchor_only_moves_to_abnormal_observations_at_the_site() {
    let public = public_of(&params(0, 150));
    let (site, _, stranger) = picks(&public);
    // A benign reading at the site and an abnormal one at another node, both in reach: neither is
    // the site's earliest abnormal observation.
    let mut feed = Feed::new(&public, spec_earliest(2_000));
    let mut obs = vec![
        (8_000, counter(site, CounterName::Latency, HIGH - 1)),
        (8_100, counter(stranger, CounterName::Latency, 90)),
    ];
    obs.extend(burst(site, 9_800));
    let ids = feed.step(10 * S, &obs);
    let n = feed.notices();
    assert_eq!(n.len(), 1);
    assert_eq!(
        n[0].1, ids[2],
        "the burst's first observation stays the anchor"
    );
}

// ---- the record, in whole segments

fn rung_with(noticer: NoticerSpec) -> RungConfig {
    RungConfig {
        noticer,
        ..RungConfig::default()
    }
}

#[test]
fn a_segment_records_every_notice_and_the_evaluator_scores_them() {
    let p = params(11, 200);
    let l = limits(&p);
    let spec = StreamPolicySpec::from_id("never_escalate").unwrap();
    for noticer in [NoticerSpec::default(), spec_change(8), spec_earliest(2_000)] {
        let record = play_with_rung(&p, &spec, &l, &rung_with(noticer)).unwrap();
        assert_eq!(record.noticer, noticer.id());
        let notices = record
            .notice_log
            .iter()
            .filter(|e| e.kind == NoticeKind::Notice)
            .count();
        let t = &record.notices.totals;
        assert_eq!(notices as u32, record.anomalies_noticed, "{}", noticer.id());
        assert_eq!(t.notices, record.anomalies_noticed);
        assert_eq!(
            t.on_background + t.on_plain + t.on_hard + t.on_decoy,
            t.notices
        );
        assert_eq!(record.notices.per_notice.len(), notices);
        assert_eq!(
            record.notices.per_incident.len(),
            record.verdict.per_incident.len()
        );
        assert_eq!(t.retirements as usize, record.notice_log.len() - notices);
        assert!(record.notice_log.iter().all(|e| e.noticer == noticer.id()));
        // The evaluator's tier of each incident is the stream verdict's.
        for (n, v) in record
            .notices
            .per_incident
            .iter()
            .zip(&record.verdict.per_incident)
        {
            assert_eq!((n.id, n.tier), (v.id, v.tier));
        }
    }
}

#[test]
fn a_noticer_changes_what_is_noticed_and_nothing_the_record_does_not_hold() {
    let p = params(11, 200);
    let l = limits(&p);
    let spec = StreamPolicySpec::from_id("never_escalate").unwrap();
    let rung = play_with_rung(&p, &spec, &l, &rung_with(NoticerSpec::default())).unwrap();
    let explicit = play_with_rung(
        &p,
        &spec,
        &l,
        &rung_with(NoticerSpec::Rung {
            notice_z: Some(3.0),
        }),
    )
    .unwrap();
    assert_eq!(
        rung.notice_log, explicit.notice_log,
        "an explicit default is the default"
    );
    assert_eq!(rung.verdict, explicit.verdict);
    let change = play_with_rung(&p, &spec, &l, &rung_with(spec_change(2))).unwrap();
    assert!(
        change.anomalies_noticed > rung.anomalies_noticed,
        "a short quiet period notices more than the rung: {} against {}",
        change.anomalies_noticed,
        rung.anomalies_noticed
    );
    assert_eq!(change.observations, rung.observations);
}

#[test]
fn the_selection_oracle_works_on_the_anomalies_of_whatever_noticer_its_rung_has() {
    let p = params(11, 200);
    let l = limits(&p);
    let spec = StreamPolicySpec::from_parts(
        "oracle_selection",
        None,
        None,
        None,
        None,
        Some(8_000_000_000),
        None,
        None,
    )
    .unwrap();
    let base = play_with_rung(&p, &spec, &l, &rung_with(NoticerSpec::default())).unwrap();
    let change = play_with_rung(&p, &spec, &l, &rung_with(spec_change(8))).unwrap();
    // The oracle escalates only anomalies anchored in hard incidents, so every call is about one.
    assert_eq!(base.verdict.totals.escalations.unneeded, 0);
    assert_eq!(change.verdict.totals.escalations.unneeded, 0);
    assert_eq!(change.verdict.totals.escalations.background, 0);
    assert_eq!(change.noticer, "change_triggered");
}

// ---- the files

fn run(m: &StreamManifest, name: &str) -> std::path::PathBuf {
    let out = scratch(name).join("run");
    execute_stream(m, &out).unwrap_or_else(|e| panic!("{e}"));
    out
}

#[test]
fn the_run_writes_the_notice_files_beside_files_the_noticer_does_not_touch() {
    let mut m = manifest(
        "noticer-files",
        &[
            ("never_escalate", "never_escalate"),
            ("always_escalate", "always_escalate"),
        ],
        2,
        200,
        3,
    );
    let plain = run(&m, "noticer-files-plain");
    m.noticers
        .insert("never_escalate".to_owned(), spec_change(8));
    let with = run(&m, "noticer-files-change");
    // An arm with no entry in `noticers`, in a run where another arm has one, writes the same
    // results.csv, incidents.csv, notices.csv, notice_incidents.csv and notice_events.csv as in a
    // run where none has.
    for file in [
        "results.csv",
        "incidents.csv",
        "notices.csv",
        "notice_incidents.csv",
        "notice_events.csv",
    ] {
        assert_eq!(
            read(&plain.join("always_escalate"), file),
            read(&with.join("always_escalate"), file),
            "{file}"
        );
    }
    // The arm with one writes other notices, and its manifest records the noticer.
    let (a, b) = (
        read(&plain.join("never_escalate"), "notices.csv"),
        read(&with.join("never_escalate"), "notices.csv"),
    );
    assert_ne!(a, b);
    assert_eq!(cell(&a, 0, "noticer"), "rung");
    assert_eq!(cell(&b, 0, "noticer"), "change_triggered");
    let arm_manifest: Value =
        serde_json::from_str(&read(&with.join("never_escalate"), "manifest.json")).unwrap();
    assert_eq!(
        arm_manifest["noticers"]["never_escalate"]["noticer"],
        "change_triggered"
    );
    // The files of the arm's declarations are the arm's, whatever it noticed with: the columns of
    // results.csv are the old ones, and the two work item E1 appended at the end.
    let header = read(&with.join("never_escalate"), "results.csv");
    assert_eq!(
        header.lines().next().unwrap(),
        format!("{PRE_SEAM_RESULTS_HEADER},recall_declarations,noticer_ns")
    );
    let header = read(&with.join("never_escalate"), "incidents.csv");
    assert_eq!(header.lines().next().unwrap(), PRE_SEAM_INCIDENTS_HEADER);
}

#[test]
fn the_notice_files_have_their_columns_and_add_up() {
    let m = manifest(
        "noticer-cols",
        &[("never_escalate", "never_escalate")],
        3,
        200,
        5,
    );
    let out = run(&m, "noticer-cols");
    let dir = out.join("never_escalate");
    let notices = read(&dir, "notices.csv");
    let incidents = read(&dir, "notice_incidents.csv");
    let events = read(&dir, "notice_events.csv");
    let results = read(&dir, "results.csv");
    let inc_all = read(&dir, "incidents.csv");
    assert_eq!(
        notices.lines().next().unwrap(),
        "run_id,arm_role,seed,noticer,notices,notices_on_background,notices_on_plain,notices_on_hard,notices_on_decoy,retirements,noticed_plain,noticed_hard,noticed_decoy,anchor_correct_plain,anchor_correct_hard,anchor_correct_decoy,notices_site_correct,notices_anchor_site_correct,site_correct_plain,site_correct_hard,site_correct_decoy,anchor_site_correct_plain,anchor_site_correct_hard,anchor_site_correct_decoy"
    );
    assert_eq!(
        incidents.lines().next().unwrap(),
        "run_id,arm_role,seed,incident,tier,family,first_observation_at_ns,notices,noticed,first_notice_at_ns,notice_latency_ns,anchor_correct,site_correct,anchor_site_correct"
    );
    assert_eq!(
        events.lines().next().unwrap(),
        "run_id,arm_role,seed,noticer,event,anomaly,anchor,site,anchor_at_ns,at_ns,incident,anchor_offset_ns,anchor_correct,site_correct,anchor_site_correct"
    );
    assert_eq!(notices.lines().count(), 1 + 3, "one row per stream");
    // One row per incident, keyed as incidents.csv is.
    assert_eq!(incidents.lines().count(), inc_all.lines().count());
    let key = |csv: &str, i: usize| -> (String, String, String) {
        (
            cell(csv, i, "seed"),
            cell(csv, i, "incident"),
            cell(csv, i, "tier"),
        )
    };
    for i in 0..incidents.lines().count() - 1 {
        assert_eq!(key(&incidents, i), key(&inc_all, i));
    }
    // The stream rows are the incident rows summed, and the event rows are the notices.
    for s in 0..3 {
        let seed = cell(&notices, s, "seed");
        let mine: Vec<usize> = (0..incidents.lines().count() - 1)
            .filter(|i| cell(&incidents, *i, "seed") == seed)
            .collect();
        let sum = |col: &str| -> u32 {
            mine.iter()
                .map(|i| cell(&incidents, *i, col).parse::<u32>().unwrap())
                .sum()
        };
        let noticed = |tier: &str| {
            mine.iter()
                .filter(|i| {
                    cell(&incidents, **i, "tier") == tier
                        && cell(&incidents, **i, "noticed") == "true"
                })
                .count() as u32
        };
        let total: u32 = cell(&notices, s, "notices").parse().unwrap();
        let bg: u32 = cell(&notices, s, "notices_on_background").parse().unwrap();
        assert_eq!(sum("notices") + bg, total);
        assert_eq!(
            noticed("hard"),
            cell(&notices, s, "noticed_hard").parse::<u32>().unwrap()
        );
        assert_eq!(
            noticed("plain"),
            cell(&notices, s, "noticed_plain").parse::<u32>().unwrap()
        );
        assert_eq!(
            cell(&notices, s, "notices"),
            cell(&results, s, "anomalies_noticed"),
            "one notice per anomaly the rung reports noticing"
        );
        let (mut n_events, mut r_events) = (0u32, 0u32);
        for e in 0..events.lines().count() - 1 {
            if cell(&events, e, "seed") != seed {
                continue;
            }
            match cell(&events, e, "event").as_str() {
                "notice" => n_events += 1,
                "retire" => r_events += 1,
                other => panic!("event {other}"),
            }
        }
        assert_eq!(n_events, total);
        assert_eq!(r_events.to_string(), cell(&notices, s, "retirements"));
    }
    // A notice event on an incident names it; one on background names none.
    let mut with_incident = 0;
    for e in 0..events.lines().count() - 1 {
        let incident = cell(&events, e, "incident");
        let correct = cell(&events, e, "anchor_correct");
        match cell(&events, e, "event").as_str() {
            "notice" if incident.is_empty() => {
                assert_eq!(correct, "false");
                assert_eq!(cell(&events, e, "site_correct"), "false");
                assert_eq!(cell(&events, e, "anchor_site_correct"), "false");
                assert!(cell(&events, e, "anchor_offset_ns").is_empty());
            }
            "notice" => {
                with_incident += 1;
                assert!(!cell(&events, e, "anchor_offset_ns").is_empty());
                assert!(correct == "true" || correct == "false");
                // N15: anchor-and-site-correct is the conjunction, read from the same row.
                let site = cell(&events, e, "site_correct");
                assert!(site == "true" || site == "false");
                assert_eq!(
                    cell(&events, e, "anchor_site_correct") == "true",
                    correct == "true" && site == "true"
                );
            }
            _ => assert!(
                incident.is_empty()
                    && correct.is_empty()
                    && cell(&events, e, "site_correct").is_empty()
                    && cell(&events, e, "anchor_site_correct").is_empty()
            ),
        }
    }
    // The stream rows count the event rows (N14, N15).
    for s in 0..3 {
        let seed = cell(&notices, s, "seed");
        let count = |col: &str| -> u32 {
            (0..events.lines().count() - 1)
                .filter(|e| cell(&events, *e, "seed") == seed && cell(&events, *e, col) == "true")
                .count() as u32
        };
        assert_eq!(
            count("site_correct").to_string(),
            cell(&notices, s, "notices_site_correct")
        );
        assert_eq!(
            count("anchor_site_correct").to_string(),
            cell(&notices, s, "notices_anchor_site_correct")
        );
        // Incidents with such a notice, by tier, read from the incident rows.
        for (col, prefix) in [
            ("site_correct", "site_correct"),
            ("anchor_site_correct", "anchor_site_correct"),
        ] {
            for tier in ["plain", "hard", "decoy"] {
                let n = (0..incidents.lines().count() - 1)
                    .filter(|i| {
                        cell(&incidents, *i, "seed") == seed
                            && cell(&incidents, *i, "tier") == tier
                            && cell(&incidents, *i, col) == "true"
                    })
                    .count();
                assert_eq!(
                    n.to_string(),
                    cell(&notices, s, &format!("{prefix}_{tier}")),
                    "{col} {tier}"
                );
            }
        }
    }
    assert!(
        with_incident > 0,
        "a 200 s stream has an incident the rung notices"
    );
}

#[test]
fn the_notice_files_are_byte_identical_on_replay() {
    let mut m = manifest(
        "noticer-replay",
        &[
            ("a", "never_escalate"),
            ("b", "never_escalate"),
            ("c", "never_escalate"),
        ],
        2,
        200,
        9,
    );
    m.noticers.insert("b".to_owned(), spec_change(4));
    m.noticers.insert("c".to_owned(), spec_earliest(2_000));
    let (x, y) = (run(&m, "noticer-replay-a"), run(&m, "noticer-replay-b"));
    let mut by_noticer: BTreeMap<String, String> = BTreeMap::new();
    for arm in ["a", "b", "c"] {
        for file in ["notices.csv", "notice_incidents.csv", "notice_events.csv"] {
            let (rx, ry) = (read(&x.join(arm), file), read(&y.join(arm), file));
            assert_eq!(rx, ry, "{arm}/{file}");
        }
        by_noticer.insert(arm.to_owned(), read(&x.join(arm), "notice_events.csv"));
    }
    // Arms that differ only in noticer have the same stream and different records.
    assert_ne!(by_noticer["a"], by_noticer["b"]);
    assert_ne!(by_noticer["a"], by_noticer["c"]);
}
