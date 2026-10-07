//! The record rung (work item E1): the key, the gate, the table, the confirmation policies, the
//! reset and carry, the recall's source, and the arm that declares a recall with no escalation.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`experiments/exploration/scripts/e1_gate.py`).

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::noticer::{BaseSpec, Noticer, NoticerSpec, RecallSource};
use gordian_run::stream::arms::noticer_reanchor::{Isolation, ReanchorNoticer};
use gordian_run::stream::arms::noticer_record::{
    KeyForm, KeyLevel, RecordConfirm, RecordNoticer, RecordParams, band_of, carry, gate_band,
    key_of,
};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::arms::{Source, StepInput, StreamPolicy};
use gordian_run::stream::spec::{StreamPolicySpec, build_public};
use gordian_stream::{
    Answer, Diagnosis, HardKind, ObsId, Question, StreamAction, StreamEvent, StreamHypothesis,
    StreamKind, StreamPublic,
};
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use serde_json::json;
use stream_common::*;

const MS: u64 = 1_000_000;

fn counter(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

fn message(service: u32, text_id: u64) -> Observation {
    Observation::Message {
        service: ServiceId(service),
        text_id,
        severity: Severity::Low,
    }
}

fn hard(kind: HardKind, site: u32) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site: ServiceId(site),
    })
}

fn public() -> StreamPublic {
    public_of(&params(0, 150))
}

/// The later re-anchor B2 selected: site isolation, 20 ms, bursts of two, z = 2.
fn base() -> BaseSpec {
    BaseSpec::Reanchor {
        notice_z: Some(2.0),
        gap_ns: 20 * MS,
        min_burst: 2,
        isolation: Isolation::Site,
    }
}

fn record(
    form: KeyForm,
    level: KeyLevel,
    confirm: RecordConfirm,
    reset: bool,
    key: u64,
) -> RecordParams {
    carry::reset(key);
    RecordParams {
        base: base(),
        form,
        level,
        confirm,
        reset,
        settle_ns: 1_000 * MS,
        msg_ids: false,
        state_key: key,
    }
}

/// A burst at `site` at `t` ms: four alarms of three kinds and a catalogue message within 40 ms,
/// then an alarm of a fourth kind 3 s later.
fn burst(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 120)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, counter(site, CounterName::Saturation, 90)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
        (t + 3_000, counter(site, CounterName::Restarts, 210)),
    ]
}

/// A record noticer fed scripted observations, with the gate under the test's control.
struct Drive {
    noticer: RecordNoticer<ReanchorNoticer>,
    held: Vec<Held>,
    public: StreamPublic,
}

impl Drive {
    fn new(p: RecordParams) -> Self {
        let public = public();
        let cfg = RungConfig {
            notice_z: 2.0,
            ..RungConfig::default()
        };
        let noticer = RecordNoticer::new(
            ReanchorNoticer::new(cfg, &public.services, 20 * MS, 2, Isolation::Site),
            p,
        );
        Self {
            noticer,
            held: Vec::new(),
            public,
        }
    }

    fn store(&self) -> Store {
        Store::with(self.held.clone())
    }

    /// Deliver `obs` and notice at `now_ms`; the ids given to them.
    fn feed(&mut self, now_ms: u64, obs: &[(u64, Observation)]) -> Vec<ObsId> {
        let mut ids = Vec::new();
        for (ms, o) in obs {
            let id = ObsId(self.held.len() as u32);
            let h = Held {
                id,
                at: Instant(ms * MS),
                abnormal: is_abnormal(o, &self.public.services),
                obs: o.clone(),
            };
            if h.abnormal {
                self.noticer.observe(&h);
            }
            self.held.push(h);
            ids.push(id);
        }
        let store = self.store();
        self.noticer.notice(Instant(now_ms * MS), &store);
        ids
    }

    /// The recalls at `now_ms` with the gate open since `since_ms` on every noticed anomaly named.
    fn recalls(
        &mut self,
        now_ms: u64,
        gate: &[(u32, u64)],
    ) -> Vec<gordian_run::stream::arms::noticer::MemoryRecall> {
        let store = self.store();
        let open: Vec<(u32, Instant)> = gate.iter().map(|(a, ms)| (*a, Instant(ms * MS))).collect();
        self.noticer.gated_by(Instant(now_ms * MS), &store, &open)
    }

    /// The id of the anomaly whose anchor is the first observation delivered at or after `obs`.
    fn anomaly_from(&self, obs: u32) -> u32 {
        self.noticer
            .anomalies()
            .iter()
            .find(|a| a.anchor.0 >= obs)
            .map(|a| a.id)
            .expect("an anomaly anchored at or after the observation")
    }

    fn anchor_of(&self, anomaly: u32) -> ObsId {
        self.noticer.tracked(anomaly).expect("tracked").anchor
    }
}

/// Teach a drive one answer: a burst at `site` at 1 s, the gate open from 2 s, snapshot at 4 s,
/// answered `answer` about its anchor. Returns the anchor.
fn teach(d: &mut Drive, site: u32, answer: Diagnosis) -> ObsId {
    d.feed(4_500, &burst(site, 1_000));
    assert!(d.noticer.tracked(0).is_some_and(|t| t.noticed_at.is_some()));
    assert!(
        d.recalls(4_500, &[(0, 2_000)]).is_empty(),
        "nothing is stored yet"
    );
    let anchor = d.anchor_of(0);
    d.noticer.answered(anchor, answer);
    // The rung retires an anomaly that has been quiet for a while; a burst at the same site later
    // is then an anomaly of its own, not more evidence for the old one.
    d.noticer.retire(0);
    anchor
}

// ---- the key

#[test]
fn bands_and_the_gate_delay_are_cut_where_the_documentation_cuts_them() {
    for (value, band) in [
        (HIGH, 0),
        (2 * HIGH - 1, 0),
        (2 * HIGH, 1),
        (4 * HIGH - 1, 1),
        (4 * HIGH, 2),
        (900, 2),
    ] {
        assert_eq!(band_of(value), band, "{value}");
    }
    let s = 1_000_000_000u64;
    for (ns, band) in [
        (0, 0),
        (s - 1, 0),
        (s, 1),
        (3 * s - 1, 1),
        (3 * s, 2),
        (6 * s - 1, 2),
        (6 * s, 3),
        (10 * s - 1, 3),
        (10 * s, 4),
    ] {
        assert_eq!(gate_band(ns), band, "{ns}");
    }
}

fn evidence_of(
    site: u32,
    obs: &[(u64, Observation)],
    public: &StreamPublic,
) -> (Vec<Held>, ServiceId) {
    let held = obs
        .iter()
        .enumerate()
        .map(|(i, (ms, o))| Held {
            id: ObsId(i as u32),
            at: Instant(ms * MS),
            abnormal: is_abnormal(o, &public.services),
            obs: o.clone(),
        })
        .filter(|h| h.abnormal)
        .collect();
    (held, ServiceId(site))
}

fn key(p: &RecordParams, site: u32, obs: &[(u64, Observation)], gate_ns: u64) -> Vec<u64> {
    let public = public();
    let (held, s) = evidence_of(site, obs, &public);
    let evidence: Vec<(&Held, ServiceId)> = held.iter().map(|h| (h, s)).collect();
    // The store holds every observation, benign ones included; the evidence is the abnormal ones.
    let store = Store::with(obs.iter().enumerate().map(|(i, (ms, o))| Held {
        id: ObsId(i as u32),
        at: Instant(ms * MS),
        abnormal: is_abnormal(o, &public.services),
        obs: o.clone(),
    }));
    key_of(
        p,
        s,
        &evidence,
        gate_ns,
        &store,
        Instant(obs[0].0 * MS),
        Instant(5_000 * MS),
    )
}

#[test]
fn the_family_key_holds_no_id_and_the_site_key_holds_the_site() {
    let fam = record(
        KeyForm::Family,
        KeyLevel::Timing,
        RecordConfirm::Never,
        true,
        9_100,
    );
    let site = record(
        KeyForm::Site,
        KeyLevel::Timing,
        RecordConfirm::Never,
        true,
        9_101,
    );
    let (a, b) = (burst(2, 1_000), burst(5, 1_000));
    let gate = 3_000 * MS;
    assert_eq!(
        key(&fam, 2, &a, gate),
        key(&fam, 5, &b, gate),
        "service numbers are not in it"
    );
    assert_ne!(key(&site, 2, &a, gate), key(&site, 5, &b, gate));
    // The site key is the family key and one more feature.
    let (fk, sk) = (key(&fam, 2, &a, gate), key(&site, 2, &a, gate));
    assert_eq!(sk.len(), fk.len() + 1);
    assert!(fk.iter().all(|f| sk.contains(f)));
    // A free-form message id is in neither, unless msg_ids is on (site-keyed only).
    let mut with_id = burst(2, 1_000);
    with_id.push((1_500, message(2, 0x00AB_CDEF_0123_4567)));
    assert_eq!(key(&site, 2, &with_id, gate), sk);
    let mut ids_on = site;
    ids_on.msg_ids = true;
    assert_eq!(key(&ids_on, 2, &with_id, gate).len(), sk.len() + 1);
    assert_eq!(key(&ids_on, 2, &a, gate), sk);
}

#[test]
fn the_levels_are_cumulative_and_each_adds_what_the_documentation_says() {
    let a = burst(2, 1_000);
    let gate = 3_000 * MS;
    let at = |level| {
        let p = record(KeyForm::Family, level, RecordConfirm::Never, true, 9_102);
        key(&p, 2, &a, gate)
    };
    let (k, b, t) = (
        at(KeyLevel::Kinds),
        at(KeyLevel::Bands),
        at(KeyLevel::Timing),
    );
    assert!(k.iter().all(|f| b.contains(f)) && b.iter().all(|f| t.contains(f)));
    // Bands: ErrorRate (80, 85: band 0), Latency 120 (1), Saturation 90 (0), Restarts 210 (2): four.
    assert_eq!(b.len() - k.len(), 4);
    assert_eq!(t.len() - b.len(), 1, "the gate delay band");
    // Kinds: ErrorRate, Latency, Saturation, Restarts counters over HIGH, the OutOfResource message,
    // and the anchor's kind (ErrorRate).
    assert_eq!(k.len(), 6);
    // A reading in another band changes the Bands key and not the Kinds key.
    let mut hotter = burst(2, 1_000);
    hotter[0].1 = counter(2, CounterName::ErrorRate, 400);
    let (pk, pb) = (
        record(
            KeyForm::Family,
            KeyLevel::Kinds,
            RecordConfirm::Never,
            true,
            9_103,
        ),
        record(
            KeyForm::Family,
            KeyLevel::Bands,
            RecordConfirm::Never,
            true,
            9_104,
        ),
    );
    assert_eq!(key(&pk, 2, &hotter, gate), key(&pk, 2, &a, gate));
    assert_ne!(key(&pb, 2, &hotter, gate), key(&pb, 2, &a, gate));
    // The gate delay changes the Timing key and not the Bands key, across a band edge.
    let pt = record(
        KeyForm::Family,
        KeyLevel::Timing,
        RecordConfirm::Never,
        true,
        9_105,
    );
    assert_ne!(key(&pt, 2, &a, 2_900 * MS), key(&pt, 2, &a, 3_100 * MS));
    assert_eq!(key(&pb, 2, &a, 2_900 * MS), key(&pb, 2, &a, 3_100 * MS));
}

// ---- the gate, the table and the recall

#[test]
fn nothing_is_recalled_before_the_gate_opens_and_settles() {
    let mut d = Drive::new(record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Never,
        true,
        9_110,
    ));
    teach(&mut d, 2, hard(HardKind::Compound, 2));
    // A second, identical burst at another site, noticed and not contradicted: no recall, no
    // snapshot, even though the table holds its key.
    d.feed(24_500, &burst(5, 21_000));
    let second = d.anomaly_from(6);
    assert!(d.recalls(24_500, &[]).is_empty());
    assert_eq!(
        d.noticer.stats().snapshots,
        1,
        "only the first anomaly was read"
    );
    // Open since 23.9 s: not settled at 24.5 s (needs 24.9 s).
    assert!(d.recalls(24_500, &[(second, 23_900)]).is_empty());
    assert_eq!(d.noticer.stats().snapshots, 1);
    // Settled: a recall.
    let r = d.recalls(25_000, &[(second, 23_900)]);
    assert_eq!(r.len(), 1);
    assert_eq!(d.noticer.stats().snapshots, 2);
    carry::reset(9_110);
}

#[test]
fn a_bound_answer_is_recalled_for_the_same_key_with_its_source_and_the_site_substituted() {
    let mut d = Drive::new(record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Never,
        true,
        9_111,
    ));
    let anchor = teach(&mut d, 2, hard(HardKind::Compound, 2));
    d.feed(24_500, &burst(5, 21_000));
    let second = d.anomaly_from(6);
    let r = d.recalls(25_000, &[(second, 23_900)]);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].anomaly, second);
    assert_eq!(
        r[0].diagnosis,
        hard(HardKind::Compound, 5),
        "the stored kind at the new anomaly's site"
    );
    assert!(!r[0].confirm);
    // The source is the answer's observation and the answer as the reasoner gave it, site and all.
    assert_eq!(
        d.noticer.recall_source(second),
        Some(RecallSource {
            obs: anchor,
            diagnosis: hard(HardKind::Compound, 2),
            age: 0
        })
    );
    // An anomaly is offered once.
    assert!(d.recalls(25_500, &[(second, 23_900)]).is_empty());
    carry::reset(9_111);
}

#[test]
fn the_site_form_recalls_only_at_its_own_site_and_declares_the_stored_diagnosis() {
    let mut d = Drive::new(record(
        KeyForm::Site,
        KeyLevel::Bands,
        RecordConfirm::Never,
        true,
        9_112,
    ));
    teach(&mut d, 2, hard(HardKind::Compound, 2));
    // Another site: the same evidence, another key.
    d.feed(24_500, &burst(5, 21_000));
    let other = d.anomaly_from(6);
    assert!(d.recalls(25_000, &[(other, 23_900)]).is_empty());
    // The same site again, later.
    d.feed(64_500, &burst(2, 61_000));
    let again = d.anomaly_from(12);
    let r = d.recalls(65_000, &[(again, 63_900)]);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].diagnosis, hard(HardKind::Compound, 2));
    carry::reset(9_112);
}

#[test]
fn a_family_answer_naming_another_site_is_not_bound_and_one_with_no_snapshot_is_not_bound() {
    let mut d = Drive::new(record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Never,
        true,
        9_113,
    ));
    // The answer names service 9, the anomaly's site is 2.
    let anchor = teach(&mut d, 2, hard(HardKind::Cascade, 9));
    assert_eq!(d.noticer.stats().unbound_other_site, 1);
    assert_eq!(d.noticer.stats().binds, 0);
    assert!(d.noticer.table().is_empty());
    // "Not an incident" names no site: bound.
    d.noticer.answered(anchor, None);
    assert_eq!(d.noticer.stats().binds, 1);
    // An answer about an observation no snapshot holds.
    d.noticer.answered(ObsId(999), hard(HardKind::Compound, 2));
    assert_eq!(d.noticer.stats().unbound_no_snapshot, 1);
    assert_eq!(d.noticer.table().len(), 1);
    carry::reset(9_113);
}

#[test]
fn the_newest_answer_replaces_the_stored_one_and_a_disagreement_disputes_the_entry() {
    let mut d = Drive::new(record(
        KeyForm::Site,
        KeyLevel::Kinds,
        RecordConfirm::OnContradiction,
        true,
        9_114,
    ));
    let anchor = teach(&mut d, 2, hard(HardKind::Compound, 2));
    let key_of_entry = d.noticer.table().keys().next().cloned().unwrap();
    assert!(!d.noticer.table()[&key_of_entry].disputed);
    d.noticer.answered(anchor, hard(HardKind::SplitBrain, 2));
    let e = d.noticer.table()[&key_of_entry];
    assert_eq!(
        (e.stored, e.answers, e.disputed),
        (hard(HardKind::SplitBrain, 2), 2, true)
    );
    // A recall of a disputed entry is confirmed, not declared, and names no source.
    d.feed(24_500, &burst(2, 21_000));
    let second = d.anomaly_from(6);
    let r = d.recalls(25_000, &[(second, 23_900)]);
    assert_eq!(r.len(), 1);
    assert!(r[0].confirm);
    assert!(d.noticer.recall_source(second).is_none());
    // An agreeing answer clears it.
    d.noticer.answered(anchor, hard(HardKind::SplitBrain, 2));
    assert!(!d.noticer.table()[&key_of_entry].disputed);
    carry::reset(9_114);
}

#[test]
fn every_kth_recall_of_the_arm_is_confirmed_counting_across_segments() {
    let p = record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Every { k: 2 },
        false,
        9_115,
    );
    let mut d = Drive::new(p);
    teach(&mut d, 2, hard(HardKind::Compound, 2));
    let mut flags = Vec::new();
    for (i, site) in [5u32, 7, 8].into_iter().enumerate() {
        let t0 = 21_000 + 40_000 * i as u64;
        d.feed(t0 + 3_500, &burst(site, t0));
        let id = (0..10)
            .find(|a| {
                d.noticer
                    .tracked(*a)
                    .is_some_and(|t| t.anchor_at.0 == t0 * MS)
            })
            .unwrap();
        let r = d.recalls(t0 + 4_000, &[(id, t0 + 2_900)]);
        assert_eq!(r.len(), 1);
        flags.push(r[0].confirm);
    }
    // Recalls 1, 2, 3 of the arm: the second is confirmed.
    assert_eq!(flags, vec![false, true, false]);
    // The next segment, same key: the count continues (3 recalls made, so the 4th is confirmed).
    let mut next = Drive::new(RecordParams { ..p });
    assert_eq!(
        next.noticer.table().len(),
        1,
        "carried: the table did not reset"
    );
    teach_again_and_recall_confirmed(&mut next);
    carry::reset(9_115);
}

fn teach_again_and_recall_confirmed(d: &mut Drive) {
    // The carried entry recalls for a new burst at once, as the 4th recall of the arm: confirmed.
    d.feed(4_500, &burst(6, 1_000));
    let r = d.recalls(5_000, &[(0, 2_900)]);
    assert_eq!(r.len(), 1);
    assert!(r[0].confirm, "the 4th recall of the arm, across segments");
}

#[test]
fn reset_empties_the_table_at_a_segment_and_carry_keeps_it() {
    for (reset, expected) in [(true, 0usize), (false, 1)] {
        let key = 9_120 + u64::from(reset);
        let p = record(
            KeyForm::Family,
            KeyLevel::Bands,
            RecordConfirm::Never,
            reset,
            key,
        );
        let mut first = Drive::new(p);
        teach(&mut first, 2, hard(HardKind::Compound, 2));
        assert_eq!(first.noticer.table().len(), 1);
        let second = Drive::new(p);
        assert_eq!(second.noticer.table().len(), expected, "reset = {reset}");
        carry::reset(key);
    }
}

// ---- the parameters

#[test]
fn the_record_spec_round_trips_validates_and_names_its_form() {
    let p = record(
        KeyForm::Site,
        KeyLevel::Timing,
        RecordConfirm::Every { k: 4 },
        true,
        9_130,
    );
    let spec = NoticerSpec::Record(p);
    let text = serde_json::to_value(spec).unwrap();
    assert_eq!(text["noticer"], json!("record"));
    assert_eq!(text["form"], json!("site"));
    assert_eq!(text["confirm"], json!({"every": {"k": 4}}));
    assert_eq!(serde_json::from_value::<NoticerSpec>(text).unwrap(), spec);
    assert_eq!(spec.id(), "record_site");
    assert_eq!(
        NoticerSpec::Record(RecordParams {
            form: KeyForm::Family,
            ..p
        })
        .id(),
        "record_family"
    );
    assert!(spec.validate().is_ok());
    assert!(
        NoticerSpec::Record(RecordParams {
            confirm: RecordConfirm::Every { k: 1 },
            ..p
        })
        .validate()
        .is_err()
    );
    assert!(
        NoticerSpec::Record(RecordParams {
            form: KeyForm::Family,
            msg_ids: true,
            ..p
        })
        .validate()
        .is_err()
    );
    assert!(
        NoticerSpec::Record(RecordParams { msg_ids: true, ..p })
            .validate()
            .is_ok()
    );
    assert!(!spec.is_default());
}

// ---- the arm

/// A drive of the arm over the record rung: `always_escalate` asking `delay_ns` after notice.
struct ArmDrive {
    arm: Box<dyn StreamPolicy>,
    public: StreamPublic,
    bill: gordian_core::Bill,
    ledger: gordian_core::Ledger,
    clock: gordian_core::ManualClock,
    totals: gordian_run::stream::meter::Totals,
    next: u32,
}

impl ArmDrive {
    fn new(record: Option<RecordParams>, delay_ns: u64) -> Self {
        let p = params(0, 150);
        let public = public_of(&p);
        let rung = RungConfig {
            noticer: record.map_or_else(
                || NoticerSpec::Reanchor {
                    notice_z: Some(2.0),
                    gap_ns: 20 * MS,
                    min_burst: 2,
                    isolation: Isolation::Site,
                },
                NoticerSpec::Record,
            ),
            ..RungConfig::default()
        };
        let spec = StreamPolicySpec::Always { delay_ns };
        let arm = build_public(&spec, &rung, &public, "always_escalate", 0).unwrap();
        let ids = gordian_run::stream::arms::rung::cheap_component_ids();
        Self {
            arm,
            bill: gordian_core::Bill::new(limits(&p).budget()),
            ledger: gordian_core::Ledger::new(),
            clock: gordian_core::ManualClock::new(),
            totals: gordian_run::stream::meter::Totals::new(&ids),
            next: 0,
            public,
        }
    }

    fn step(
        &mut self,
        now_ms: u64,
        obs: &[(u64, Observation)],
        answers: &[Answer],
    ) -> Vec<(Source, StreamAction, Option<RecallSource>)> {
        let mut events: Vec<StreamEvent> = obs
            .iter()
            .map(|(ms, o)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: Instant(ms * MS),
                    obs: o.clone(),
                }
            })
            .collect();
        for (i, a) in answers.iter().enumerate() {
            events.push(StreamEvent::Answered {
                call: i as u32,
                at: Instant(now_ms * MS),
                answer: *a,
            });
        }
        let input = StepInput {
            now: Instant(now_ms * MS),
            events: &events,
            probe_results: &[],
            applied: &[],
            public: &self.public,
        };
        let mut meter = gordian_run::stream::meter::Meter::new(
            &mut self.bill,
            &mut self.ledger,
            &mut self.clock,
            &mut self.totals,
            "policy/test",
        );
        self.arm
            .step(&input, true, &mut meter)
            .into_iter()
            .map(|p| (p.source, p.action, p.recall))
            .collect()
    }

    fn play(
        &mut self,
        obs: &[(u64, Observation)],
        from_ms: u64,
        to_ms: u64,
    ) -> Vec<(u64, Source, StreamAction, Option<RecallSource>)> {
        let mut out = Vec::new();
        let mut t = from_ms;
        while t <= to_ms {
            let due: Vec<(u64, Observation)> = obs
                .iter()
                .filter(|(ms, _)| *ms <= t && *ms + 500 > t)
                .cloned()
                .collect();
            for (s, a, r) in self.step(t, &due, &[]) {
                out.push((t, s, a, r));
            }
            t += 500;
        }
        out
    }
}

/// A burst the public rules cannot explain: every counter alarmed and messages of two kinds that no
/// single fault of the first world produces together.
fn unexplained(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 120)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, counter(site, CounterName::AuthFailures, 90)),
        (t + 40, message(site, SignalText::Unauthorized.text_id())),
        (t + 50, counter(site, CounterName::Restarts, 210)),
        (t + 60, counter(site, CounterName::Saturation, 95)),
    ]
}

fn asked(actions: &[(u64, Source, StreamAction, Option<RecallSource>)], from_obs: u32) -> Vec<u64> {
    actions
        .iter()
        .filter_map(|(t, _, a, _)| match a {
            StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } if focus.0 >= from_obs => Some(*t),
            _ => None,
        })
        .collect()
}

#[test]
fn the_arm_declares_a_recall_with_its_source_and_never_asks_the_recalled_anomaly() {
    for with in [true, false] {
        let key = 9_140;
        let p = record(
            KeyForm::Family,
            KeyLevel::Kinds,
            RecordConfirm::Never,
            true,
            key,
        );
        // Asking 5 s after the notice, which the first burst's evidence outlasts (the rung retires
        // an anomaly that has been quiet for 6 s).
        let mut d = ArmDrive::new(with.then_some(p), 5_000 * MS);
        let first = unexplained(2, 1_000);
        let mut all = d.play(&first, 0, 21_000);
        let focus = all
            .iter()
            .find_map(|(_, _, a, _)| match a {
                StreamAction::Escalate {
                    question: Question::Diagnose { focus },
                    ..
                } => Some(*focus),
                _ => None,
            })
            .expect("the rule asks about the first burst");
        assert!(
            all.iter().all(|(_, s, _, _)| *s != Source::Recall),
            "nothing to recall yet"
        );
        // The answer, declared as the reasoner's and bound to the key read when the gate had been
        // open for a second.
        let answer = Answer {
            focus,
            diagnosis: hard(HardKind::Compound, 2),
        };
        d.step(22_000, &[], &[answer]);
        let second_burst = unexplained(5, 40_000);
        all = d.play(&second_burst, 39_500, 70_000);
        let recalled: Vec<_> = all
            .iter()
            .filter(|(_, s, _, _)| *s == Source::Recall)
            .collect();
        let asked_second = asked(&all, 7);
        if with {
            assert_eq!(recalled.len(), 1, "one recall: {all:?}");
            let (t, _, StreamAction::Declare { diagnosis, .. }, source) = recalled[0] else {
                panic!("a recall is a declaration");
            };
            assert_eq!(*diagnosis, hard(HardKind::Compound, 5));
            assert_eq!(
                *source,
                Some(RecallSource {
                    obs: focus,
                    diagnosis: hard(HardKind::Compound, 2),
                    age: 0
                })
            );
            assert!(
                *t < 40_000 + 16_000,
                "before the rule would have asked: {t}"
            );
            assert!(
                asked_second.is_empty(),
                "a recalled anomaly is never asked about: {asked_second:?}"
            );
        } else {
            assert!(recalled.is_empty());
            assert_eq!(
                asked_second.len(),
                1,
                "the memoryless control asks about the second burst"
            );
        }
        carry::reset(key);
    }
}

// ---- the run output

fn record_manifest(run_id: &str, seeds: u64) -> gordian_run::stream::manifest::StreamManifest {
    let mut m = manifest(
        run_id,
        &[
            ("never_escalate", "never_escalate"),
            ("always_escalate", "always_escalate"),
        ],
        seeds,
        200,
        4,
    );
    let p = record(
        KeyForm::Family,
        KeyLevel::Kinds,
        RecordConfirm::Never,
        true,
        9_150,
    );
    m.noticers
        .insert("always_escalate".to_owned(), NoticerSpec::Record(p));
    m
}

#[test]
fn the_run_writes_the_memory_files_and_they_agree_with_results_and_incidents() {
    let m = record_manifest("record-files", 3);
    let out = scratch("record-files").join("run");
    gordian_run::stream::execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    carry::reset(9_150);
    for arm in ["never_escalate", "always_escalate"] {
        let dir = out.join(arm);
        let results = read(&dir, "results.csv");
        let incidents = read(&dir, "incidents.csv");
        let memory = read(&dir, "memory.csv");
        let per_incident = read(&dir, "memory_incidents.csv");
        let recalls = read(&dir, "recalls.csv");
        assert!(
            results
                .lines()
                .next()
                .unwrap()
                .ends_with(",total_cost_ns,recall_declarations,noticer_ns")
        );
        assert_eq!(memory.lines().count(), 1 + 3, "one row per stream");
        assert_eq!(per_incident.lines().count(), incidents.lines().count());
        assert_eq!(
            memory.lines().next().unwrap().split(',').count(),
            memory.lines().nth(1).unwrap().split(',').count()
        );
        // The harness's count of declarations from memory, the evaluator's cells and the rows of
        // recalls.csv are three readings of one thing.
        let mut rows_in_recalls = 0usize;
        for i in 0..3 {
            let decl: usize = cell(&results, i, "recall_declarations").parse().unwrap();
            let cheap: usize = cell(&results, i, "cheap_declarations").parse().unwrap();
            let cells: usize = cell(&memory, i, "recalls").parse().unwrap();
            assert_eq!(decl, cells, "{arm} stream {i}");
            assert!(
                decl <= cheap,
                "recalls are also counted with the cheap rung's"
            );
            rows_in_recalls += decl;
        }
        assert_eq!(recalls.lines().count(), 1 + rows_in_recalls);
        assert_eq!(
            cell(&results, 0, "noticer_ns"),
            "0",
            "no noticer of this run charges the bill"
        );
        if arm == "never_escalate" {
            assert_eq!(rows_in_recalls, 0);
        }
        let seed0 = cell(&memory, 0, "seed");
        assert_eq!(seed0, cell(&results, 0, "seed"));
    }
    // The noticer ids of the two arms.
    assert_eq!(
        cell(
            &read(&out.join("always_escalate"), "memory.csv"),
            0,
            "noticer"
        ),
        "record_family"
    );
    assert_eq!(
        cell(
            &read(&out.join("never_escalate"), "memory.csv"),
            0,
            "noticer"
        ),
        "rung"
    );
}

#[test]
fn a_segment_scores_its_recalls_and_counts_them_three_ways() {
    for seed in 0..4u64 {
        let p = params(seed, 200);
        let l = limits(&p);
        let spec = StreamPolicySpec::Always {
            delay_ns: 5_000 * MS,
        };
        let key = 9_160 + seed;
        let rec = record(
            KeyForm::Family,
            KeyLevel::Kinds,
            RecordConfirm::Never,
            true,
            key,
        );
        let rung = RungConfig {
            noticer: NoticerSpec::Record(rec),
            ..RungConfig::default()
        };
        let r = play_with_rung(&p, &spec, &l, &rung).unwrap();
        carry::reset(key);
        assert_eq!(r.noticer, "record_family");
        assert_eq!(r.counts.recall_declarations as usize, r.recalls.len());
        assert_eq!(
            r.counts.recall_declarations,
            r.memory.totals.recalls.total()
        );
        assert_eq!(r.memory.per_recall.len(), r.recalls.len());
        // The monitored arm checks consistency: components ran for it.
        assert!(r.components_run > 0);
        // The recall record names declarations of the trajectory.
        for e in &r.recalls {
            assert!(matches!(
                r.trajectory[e.step].action,
                StreamAction::Declare { .. }
            ));
        }
    }
}

#[test]
fn a_carried_entry_says_how_many_segments_ago_it_was_bound() {
    let key = 9_170;
    let p = record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Never,
        false,
        key,
    );
    // Segment 0 binds; segment 1 recalls it (age 1); segment 2 recalls it again (age 2); a new
    // answer in segment 2 rebinds it, so segment 3 recalls it at age 1.
    let mut s0 = Drive::new(p);
    teach(&mut s0, 2, hard(HardKind::Compound, 2));
    let age_in = |d: &mut Drive| {
        d.feed(4_500, &burst(5, 1_000));
        let r = d.recalls(5_000, &[(0, 2_900)]);
        assert_eq!(r.len(), 1);
        d.noticer
            .recall_source(0)
            .expect("a declared recall names its source")
    };
    let mut s1 = Drive::new(p);
    assert_eq!(age_in(&mut s1).age, 1);
    let mut s2 = Drive::new(p);
    assert_eq!(age_in(&mut s2).age, 2);
    let anchor = s2.anchor_of(0);
    s2.noticer.answered(anchor, hard(HardKind::Compound, 5));
    let mut s3 = Drive::new(p);
    let source = age_in(&mut s3);
    assert_eq!(source.age, 1);
    assert_eq!(source.diagnosis, hard(HardKind::Compound, 5));
    // An arm that resets always binds and recalls in one segment.
    let q = record(
        KeyForm::Family,
        KeyLevel::Bands,
        RecordConfirm::Never,
        true,
        key + 1,
    );
    let mut a = Drive::new(q);
    teach(&mut a, 2, hard(HardKind::Compound, 2));
    let mut b = Drive::new(q);
    b.feed(4_500, &burst(5, 1_000));
    assert!(
        b.recalls(5_000, &[(0, 2_900)]).is_empty(),
        "nothing carried"
    );
    carry::reset(key);
    carry::reset(key + 1);
}

#[test]
fn a_carried_memory_is_judged_against_the_earlier_streams_truth_in_a_real_run() {
    // Twelve streams in order, a carried family-keyed arm under the selection oracle: whatever it
    // recalls from an earlier stream, the harness must find that stream's truth for its source
    // (a recall whose source it cannot find is a harness defect, and the run would stop).
    let mut m = manifest(
        "record-carry",
        &[("oracle_selection_privileged", "oracle_selection")],
        12,
        300,
        6,
    );
    m.arms[0].policy = StreamPolicySpec::from_id("oracle_selection").unwrap();
    let p = record(
        KeyForm::Family,
        KeyLevel::Kinds,
        RecordConfirm::Never,
        false,
        9_180,
    );
    m.noticers.insert(
        "oracle_selection_privileged".to_owned(),
        NoticerSpec::Record(p),
    );
    let out = scratch("record-carry").join("run");
    gordian_run::stream::execute_stream(&m, &out).unwrap_or_else(|e| panic!("{e}"));
    carry::reset(9_180);
    let recalls = read(&out.join("oracle_selection_privileged"), "recalls.csv");
    let table = rows(&recalls);
    let col = |name: &str| table[0].iter().position(|c| c == name).unwrap();
    let seeds: Vec<u64> = (0..12).collect();
    for r in table.iter().skip(1) {
        let age: u64 = r[col("source_age")].parse().unwrap_or(0);
        let source_seed: u64 = r[col("source_seed")].parse().expect("a source has a seed");
        let seed: u64 = r[col("seed")].parse().unwrap();
        assert!(source_seed <= seed && seeds.contains(&source_seed));
        assert_eq!(age == 0, source_seed == seed, "{r:?}");
        assert!(
            ["right", "wrong"].contains(&r[col("source_class")].as_str()),
            "{r:?}"
        );
    }
}
