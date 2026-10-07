//! The engram layer of the medium noticer (work item A1a): the key, the outcome, bind from the
//! arm's own answers, recall to a declaration with no escalation, the site switch, the
//! confirmation policies, the carry across segments and the cost.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`experiments/exploration/scripts/a1a_gate.py`).

mod stream_common;

use gordian_core::Instant;
use gordian_medium::Tag;
use gordian_run::stream::arms::medium::adapters::{ABNORMAL_KIND, abnormal_kind_tag, message_tag};
use gordian_run::stream::arms::medium::engram::{
    BAND, band_tag, carry, diagnosis_of, features, outcome_of, outcome_tag,
};
use gordian_run::stream::arms::medium::{
    ConfirmPolicy, EngramConfig, MediumNoticer, MediumParams, SiteMode,
};
use gordian_run::stream::arms::noticer::{MemoryRecall, Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::arms::{Source, StepInput};
use gordian_run::stream::spec::{StreamPolicySpec, build_public};
use gordian_stream::{
    Answer, Diagnosis, HardKind, ObsId, ObsRef, Question, StreamAction, StreamEvent,
    StreamHypothesis, StreamKind, StreamPublic,
};
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{CounterName, FaultKind, Observation, ServiceId, Severity};
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

fn held(id: u32, at_ms: u64, obs: Observation, public: &StreamPublic) -> Held {
    Held {
        id: ObsId(id),
        at: Instant(at_ms * MS),
        abnormal: is_abnormal(&obs, &public.services),
        obs,
    }
}

fn public() -> StreamPublic {
    public_of(&params(0, 150))
}

/// A burst at `site`: two alarms, a catalogue message, a free-form message and a third alarm,
/// within 40 ms (the onset path notices three alarms at once), then, 3 s later (after the first
/// phase), an alarm of another kind.
fn burst(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 120)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, message(site, 0x00AB_CDEF_0123_4567)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
        (t + 3_000, counter(site, CounterName::Saturation, 90)),
    ]
}

fn hard(kind: HardKind, site: u32) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site: ServiceId(site),
    })
}

fn engram(key: u64, site: SiteMode, confirm: ConfirmPolicy) -> EngramConfig {
    carry::reset(key);
    EngramConfig {
        state_key: key,
        site,
        confirm,
        ..EngramConfig::default()
    }
}

/// The onset path only, at 100 ms, with `engram`.
fn with_engram(engram: Option<EngramConfig>) -> MediumParams {
    MediumParams {
        ramp: false,
        engram,
        ..MediumParams::default()
    }
}

/// Drives a noticer as the rung does, every 500 ms, over a fixed list of observations; answers
/// are given between steps.
struct Drive {
    noticer: MediumNoticer,
    all: Vec<Held>,
    next: usize,
    items: Vec<Held>,
    now_ms: u64,
    notices: Vec<(u64, u32, ObsId, ServiceId)>,
    recalls: Vec<(u64, MemoryRecall)>,
}

impl Drive {
    fn new(params: MediumParams, public: &StreamPublic, obs: &[(u64, Observation)]) -> Self {
        Self {
            noticer: MediumNoticer::new(params, RungConfig::default(), &public.services).unwrap(),
            all: obs
                .iter()
                .enumerate()
                .map(|(i, (ms, o))| held(i as u32, *ms, o.clone(), public))
                .collect(),
            next: 0,
            items: Vec::new(),
            now_ms: 0,
            notices: Vec::new(),
            recalls: Vec::new(),
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
            for n in self.noticer.notice(Instant(now), &store) {
                self.notices.push((self.now_ms, n.id, n.anchor, n.site));
            }
            for r in self.noticer.recalls() {
                self.recalls.push((self.now_ms, r));
            }
            for id in self.noticer.retirable(Instant(now)) {
                self.noticer.retire(id);
            }
            self.now_ms += 500;
        }
    }
}

/// Two agreeing answers: the strength a recall needs (threshold 1.5).
fn answer_twice(d: &mut Drive, focus: ObsId, diagnosis: Diagnosis) {
    d.noticer.answered(focus, diagnosis);
    d.noticer.answered(focus, diagnosis);
}

fn script(sites: &[(u32, u64)]) -> Vec<(u64, Observation)> {
    let mut out: Vec<(u64, Observation)> = sites.iter().flat_map(|(s, t)| burst(*s, *t)).collect();
    out.sort_by_key(|(t, _)| *t);
    out
}

// ---- the key and the outcome

#[test]
fn the_key_is_kinds_bands_and_message_ids_marked_late_after_the_first_phase() {
    let p = public();
    let hs: Vec<Held> = burst(2, 1_000)
        .into_iter()
        .enumerate()
        .map(|(i, (t, o))| held(i as u32, t, o, &p))
        .collect();
    let onset_end = Instant(3_000 * MS);
    let early = |t: Tag| (t, false);
    let family = features(&hs, onset_end, false);
    let expected = vec![
        early(Tag(ABNORMAL_KIND)),
        early(Tag(BAND)),
        early(Tag(ABNORMAL_KIND + 1)),
        early(Tag(BAND + 4 + 1)),
        early(abnormal_kind_tag(&hs[2].obs)),
        early(message_tag(SignalText::OutOfResource.text_id())),
        (Tag(ABNORMAL_KIND + 2), true),
        (Tag(BAND + 8), true),
    ];
    assert_eq!(family, expected, "a family key holds no free-form id");
    let site = features(&hs, onset_end, true);
    assert_eq!(site.len(), 9);
    assert_eq!(site[6], early(message_tag(0x00AB_CDEF_0123_4567)));
    assert!(!hs[3].abnormal, "a free-form message is benign");
    // A feature seen in the first phase is not late when it repeats later.
    let again = held(9, 5_000, counter(2, CounterName::ErrorRate, 70), &p);
    let f = features(hs.iter().chain([&again]), onset_end, false);
    assert_eq!(
        f.iter().filter(|(t, _)| *t == Tag(ABNORMAL_KIND)).count(),
        1
    );
    assert!(!f[0].1);
    let b = |v| band_tag(&counter(0, CounterName::Restarts, v));
    assert_eq!(b(HIGH - 1), None);
    assert_eq!(b(HIGH), Some(Tag(BAND + 16)));
    assert_eq!(b(2 * HIGH), Some(Tag(BAND + 17)));
    assert_eq!(b(4 * HIGH), Some(Tag(BAND + 18)));
    assert_eq!(band_tag(&hs[2].obs), None);
}

#[test]
fn outcome_tags_name_every_diagnosis_and_read_back() {
    let mut diagnoses: Vec<Diagnosis> = vec![None];
    for k in FaultKind::ALL {
        diagnoses.push(Some(StreamHypothesis {
            kind: StreamKind::Known(k),
            site: ServiceId(3),
        }));
    }
    for k in HardKind::ALL {
        diagnoses.push(hard(k, 3));
    }
    let tags: Vec<u32> = diagnoses.iter().map(|d| outcome_tag(d).0).collect();
    assert_eq!(tags, (0..10).collect::<Vec<_>>());
    for d in &diagnoses {
        let site = d.map(|h| h.site);
        assert_eq!(diagnosis_of(outcome_tag(d), site), Some(*d));
    }
    assert_eq!(diagnosis_of(Tag(10), Some(ServiceId(0))), None);
    assert_eq!(diagnosis_of(Tag(6), None), None, "a kind needs a site");
    use gordian_medium::OutcomeSite;
    assert_eq!(
        outcome_of(&hard(HardKind::Compound, 3), ServiceId(3)).site,
        OutcomeSite::Support
    );
    assert_eq!(
        outcome_of(&hard(HardKind::Cascade, 5), ServiceId(3)).site,
        OutcomeSite::Fixed(5)
    );
    assert_eq!(outcome_of(&None, ServiceId(3)).site, OutcomeSite::None);
}

// ---- bind and recall at the noticer

#[test]
fn an_answer_binds_and_the_same_pattern_elsewhere_is_recalled_with_the_site_substituted() {
    let p = public();
    let cfg = engram(9_001, SiteMode::Family, ConfirmPolicy::Never);
    let mut d = Drive::new(
        with_engram(Some(cfg)),
        &p,
        &script(&[(0, 1_000), (1, 20_000)]),
    );
    d.until(4_500);
    assert_eq!(d.notices.len(), 1);
    let (_, _, anchor, site) = d.notices[0];
    assert_eq!(site, ServiceId(0));
    d.noticer.answered(anchor, hard(HardKind::Compound, 0));
    let layer = d.noticer.engram().unwrap();
    assert_eq!(layer.engrams().engrams().len(), 1);
    assert_eq!(layer.engrams().engrams()[0].key.features().len(), 8);
    assert_eq!(layer.engrams().engrams()[0].strength, 1.0);
    // One answer is not a memory; a second agreeing one makes the strength 2.
    d.noticer.answered(anchor, hard(HardKind::Compound, 0));
    let s = d.noticer.engram().unwrap().engrams().engrams()[0].strength;
    assert_eq!(s, 2.0);
    assert!(d.recalls.is_empty());
    d.until(21_000);
    assert!(
        d.recalls.is_empty(),
        "the recall waits for the late feature"
    );
    d.until(25_000);
    let second = d.notices.iter().find(|n| n.3 == ServiceId(1)).unwrap();
    assert_eq!(
        d.recalls.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
        vec![MemoryRecall {
            anomaly: second.1,
            diagnosis: hard(HardKind::Compound, 1),
            confirm: false,
        }]
    );
    let stats = d.noticer.engram().unwrap().stats();
    assert_eq!(
        (stats.binds, stats.recalls, stats.recalls_matched),
        (2, 1, 1)
    );
    // Recalled once the late feature (at 23 s) had arrived.
    assert_eq!(d.recalls[0].0, 23_500);
    carry::reset(9_001);
}

#[test]
fn a_site_keyed_engram_recalls_at_its_own_service_only() {
    let p = public();
    let cfg = engram(9_002, SiteMode::Site, ConfirmPolicy::Never);
    let mut d = Drive::new(
        with_engram(Some(cfg)),
        &p,
        &script(&[(0, 1_000), (1, 20_000), (0, 40_000)]),
    );
    d.until(4_500);
    let (_, _, anchor, _) = d.notices[0];
    answer_twice(&mut d, anchor, hard(HardKind::SplitBrain, 0));
    d.until(30_000);
    assert!(d.recalls.is_empty(), "not at another service");
    d.until(45_000);
    assert_eq!(d.recalls.len(), 1);
    assert_eq!(d.recalls[0].1.diagnosis, hard(HardKind::SplitBrain, 0));
    carry::reset(9_002);
}

#[test]
fn without_bind_or_without_the_layer_nothing_is_recalled() {
    let p = public();
    let obs = script(&[(0, 1_000), (1, 20_000)]);
    let mut off = engram(9_003, SiteMode::Family, ConfirmPolicy::Never);
    off.bind = false;
    for params in [with_engram(Some(off)), with_engram(None)] {
        let mut d = Drive::new(params, &p, &obs);
        d.until(4_500);
        let (_, _, anchor, _) = d.notices[0];
        answer_twice(&mut d, anchor, hard(HardKind::Compound, 0));
        d.until(25_000);
        assert!(d.recalls.is_empty());
        assert_eq!(d.notices.len(), 2);
    }
    carry::reset(9_003);
}

#[test]
fn an_answer_about_an_anomaly_no_longer_held_binds_nothing() {
    let p = public();
    let cfg = engram(9_004, SiteMode::Family, ConfirmPolicy::Never);
    let mut d = Drive::new(with_engram(Some(cfg)), &p, &script(&[(0, 1_000)]));
    d.until(20_000); // retired after the 6 s hold
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    let layer = d.noticer.engram().unwrap();
    assert!(layer.engrams().engrams().is_empty());
    assert_eq!(layer.stats().answers_unheld, 1);
    carry::reset(9_004);
}

#[test]
fn family_engrams_carry_to_the_next_segment_and_site_keyed_ones_do_not_by_default() {
    let p = public();
    // (site mode, carry, carry_site, carried)
    let cases = [
        (SiteMode::Family, true, false, true),
        (SiteMode::Family, false, false, false),
        (SiteMode::Site, true, false, false),
        (SiteMode::Site, true, true, true),
    ];
    for (mode, carry_on, carry_site, carried) in cases {
        let mut cfg = engram(9_005, mode, ConfirmPolicy::Never);
        cfg.carry = carry_on;
        cfg.carry_site = carry_site;
        let mut first = Drive::new(with_engram(Some(cfg)), &p, &script(&[(0, 1_000)]));
        first.until(4_500);
        let (_, _, anchor, _) = first.notices[0];
        answer_twice(&mut first, anchor, hard(HardKind::Cascade, 0));
        // The next segment: a fresh noticer, the clock from 0 again; the same service number in
        // the site-keyed case, so that only the carry decides.
        let site = if mode == SiteMode::Family { 2 } else { 0 };
        let mut next = Drive::new(with_engram(Some(cfg)), &p, &script(&[(site, 5_000)]));
        next.until(10_000);
        if carried {
            assert_eq!(next.recalls.len(), 1, "{mode:?}");
            assert_eq!(next.recalls[0].1.diagnosis, hard(HardKind::Cascade, site));
            assert_eq!(next.noticer.engram().unwrap().recall_count(), 1);
        } else {
            assert!(next.recalls.is_empty(), "{mode:?} carry {carry_on}");
        }
        carry::reset(9_005);
    }
}

#[test]
fn an_answer_with_no_late_evidence_or_a_family_answer_elsewhere_is_not_bound() {
    let p = public();
    let early_only: Vec<(u64, Observation)> = burst(0, 1_000).into_iter().take(5).collect();
    let cfg = engram(9_012, SiteMode::Family, ConfirmPolicy::Never);
    let mut d = Drive::new(with_engram(Some(cfg)), &p, &early_only);
    d.until(4_500);
    let (_, _, anchor, _) = d.notices[0];
    d.noticer.answered(anchor, hard(HardKind::Compound, 0));
    let stats = *d.noticer.engram().unwrap().stats();
    assert_eq!((stats.no_late, stats.binds), (1, 0));
    let mut d = Drive::new(with_engram(Some(cfg)), &p, &script(&[(0, 1_000)]));
    d.until(4_500);
    let (_, _, anchor, _) = d.notices[0];
    d.noticer.answered(anchor, hard(HardKind::Cascade, 3));
    let stats = *d.noticer.engram().unwrap().stats();
    assert_eq!((stats.elsewhere, stats.binds), (1, 0));
    carry::reset(9_012);
}

#[test]
fn every_kth_recall_is_confirmed() {
    let p = public();
    let cfg = engram(9_006, SiteMode::Family, ConfirmPolicy::Every { k: 2 });
    let mut d = Drive::new(
        with_engram(Some(cfg)),
        &p,
        &script(&[(0, 1_000), (1, 20_000), (2, 40_000), (3, 60_000)]),
    );
    d.until(4_500);
    let (_, _, anchor, _) = d.notices[0];
    answer_twice(&mut d, anchor, hard(HardKind::Compound, 0));
    d.until(65_000);
    let confirms: Vec<bool> = d.recalls.iter().map(|(_, r)| r.confirm).collect();
    assert_eq!(confirms, vec![false, true, false]);
    carry::reset(9_006);
}

#[test]
fn a_contradicted_or_disputed_recall_is_confirmed_under_on_contradiction() {
    let p = public();
    let cfg = engram(9_007, SiteMode::Family, ConfirmPolicy::OnContradiction);
    let mut d = Drive::new(
        with_engram(Some(cfg)),
        &p,
        &script(&[(0, 1_000), (0, 20_000), (1, 40_000)]),
    );
    d.until(4_500);
    let (_, _, a0, _) = d.notices[0];
    for _ in 0..4 {
        d.noticer.answered(a0, hard(HardKind::Compound, 0)); // strength 4
    }
    d.until(25_000);
    // The same pattern at the same service is recalled (not confirmed: no contradiction yet)...
    assert_eq!(d.recalls.len(), 1);
    assert!(!d.recalls[0].1.confirm);
    // ... and answered otherwise, twice: the engram is contradicted twice (strength 2), and a
    // second engram is bound with the other outcome (strength 2).
    let (_, _, a1, _) = *d.notices.last().unwrap();
    answer_twice(&mut d, a1, hard(HardKind::Cascade, 0));
    let engrams = d.noticer.engram().unwrap().engrams().engrams().to_vec();
    assert_eq!(engrams.len(), 2);
    assert_eq!((engrams[0].strength, engrams[0].contradictions), (2.0, 2));
    assert_eq!(engrams[1].strength, 2.0);
    d.until(45_000);
    // Both recall at service 1; the first (equal strength, lower id) is taken, and confirmed.
    assert_eq!(d.recalls.len(), 2);
    assert_eq!(d.recalls[1].1.diagnosis, hard(HardKind::Compound, 1));
    assert!(d.recalls[1].1.confirm);
    assert_eq!(d.noticer.engram().unwrap().stats().recalls_redundant, 1);
    carry::reset(9_007);
}

#[test]
fn the_layer_is_charged_its_ticks_and_its_plasticity_work() {
    let p = public();
    let obs = script(&[(0, 1_000), (1, 20_000)]);
    let mut off = engram(9_008, SiteMode::Family, ConfirmPolicy::Never);
    off.bind = false;
    let mut plain = Drive::new(with_engram(None), &p, &obs);
    let mut layer = Drive::new(with_engram(Some(off)), &p, &obs);
    plain.until(30_000);
    layer.until(30_000);
    let a = plain.noticer.take_cost().unwrap().compute_ns;
    let b = layer.noticer.take_cost().unwrap().compute_ns;
    // 300 ticks at 200 ns, and one routing (40 ns) per abnormal observation or message.
    let routed = obs.len() as u64;
    assert_eq!(b - a, 300 * 200 + routed * 40);
    // With bind on, the bind's cells and synapses are charged when it happens.
    let on = engram(9_008, SiteMode::Family, ConfirmPolicy::Never);
    let mut d = Drive::new(with_engram(Some(on)), &p, &obs);
    d.until(4_500);
    d.noticer.take_cost();
    let (_, _, anchor, _) = d.notices[0];
    d.noticer.answered(anchor, hard(HardKind::Compound, 0));
    let m = d.noticer.engram().unwrap().medium();
    let (cells, synapses) = (m.cells().len() as u64, m.synapses().len() as u64);
    assert_eq!(
        d.noticer.take_cost().unwrap().compute_ns,
        cells * 200 + synapses * 25
    );
    carry::reset(9_008);
}

#[test]
fn the_engram_field_is_absent_unless_given_and_its_checks_refuse_what_they_name() {
    let text = serde_json::to_value(NoticerSpec::Medium(MediumParams::default())).unwrap();
    assert!(text.get("engram").is_none());
    let cfg = engram(9_009, SiteMode::Site, ConfirmPolicy::Every { k: 4 });
    let spec = NoticerSpec::Medium(with_engram(Some(cfg)));
    let text = serde_json::to_value(spec).unwrap();
    assert_eq!(text["engram"]["site"], json!("site"));
    assert_eq!(text["engram"]["confirm"], json!({"every": {"k": 4}}));
    assert_eq!(serde_json::from_value::<NoticerSpec>(text).unwrap(), spec);
    assert!(spec.validate().is_ok());
    let bad = |f: &dyn Fn(&mut EngramConfig)| {
        let mut c = cfg;
        f(&mut c);
        NoticerSpec::Medium(with_engram(Some(c)))
            .validate()
            .is_err()
    };
    assert!(bad(&|c| c.confirm = ConfirmPolicy::Every { k: 0 }));
    assert!(bad(&|c| c.key_span_ns = 0));
    assert!(bad(&|c| c.decay = 0.0));
    assert!(bad(&|c| c.decay = 1.5));
    assert!(bad(&|c| c.decay_period_ns = 100 * MS));
    assert!(bad(&|c| c.min_features = 0));
    assert!(bad(&|c| c.min_features = 9));
    assert!(bad(&|c| c.threshold = 0.0));
    assert!(bad(&|c| c.gain = f32::NAN));
    assert!(bad(&|c| c.max_strength = 0.5));
    assert!(bad(&|c| c.penalty = -1.0));
    carry::reset(9_009);
}

// ---- the arm: a recall is a declaration without an escalation

struct ArmDrive {
    arm: Box<dyn gordian_run::stream::arms::StreamPolicy>,
    public: StreamPublic,
    bill: gordian_core::Bill,
    ledger: gordian_core::Ledger,
    clock: gordian_core::ManualClock,
    totals: gordian_run::stream::meter::Totals,
    next: u32,
}

impl ArmDrive {
    /// `always_escalate` with `delay_ns` after notice, over the medium with `engram`.
    fn new(engram: Option<EngramConfig>, delay_ns: u64) -> Self {
        let p = params(0, 150);
        let public = public_of(&p);
        let rung = RungConfig {
            noticer: NoticerSpec::Medium(with_engram(engram)),
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
    ) -> Vec<(Source, StreamAction)> {
        let mut events: Vec<StreamEvent> = obs
            .iter()
            .map(|(ms, o)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: o.clone(),
                }
            })
            .collect();
        for (i, a) in answers.iter().enumerate() {
            events.push(StreamEvent::Answered {
                call: i as u32,
                at: at(now_ms),
                answer: *a,
            });
        }
        let input = StepInput {
            now: at(now_ms),
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
            .map(|p| (p.source, p.action))
            .collect()
    }

    /// Steps every 500 ms over `obs` from `from_ms` to `to_ms`; the actions, with their step.
    fn play(
        &mut self,
        obs: &[(u64, Observation)],
        from_ms: u64,
        to_ms: u64,
    ) -> Vec<(u64, Source, StreamAction)> {
        let mut out = Vec::new();
        let mut t = from_ms;
        while t <= to_ms {
            let due: Vec<(u64, Observation)> = obs
                .iter()
                .filter(|(ms, _)| *ms <= t && *ms + 500 > t)
                .cloned()
                .collect();
            for (s, a) in self.step(t, &due, &[]) {
                out.push((t, s, a));
            }
            t += 500;
        }
        out
    }
}

fn escalated_focus(actions: &[(u64, Source, StreamAction)]) -> Vec<ObsId> {
    actions
        .iter()
        .filter_map(|(_, _, a)| match a {
            StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } => Some(*focus),
            _ => None,
        })
        .collect()
}

/// The escalations about the second burst (observations 6 and later).
fn asked_about_second(actions: &[(u64, Source, StreamAction)]) -> Vec<ObsId> {
    escalated_focus(actions)
        .into_iter()
        .filter(|o| o.0 >= 6)
        .collect()
}

#[test]
fn a_recall_is_declared_with_no_escalation_and_the_control_escalates() {
    let b0 = burst(0, 1_000);
    let b1 = burst(1, 20_000);
    for with in [true, false] {
        let cfg = engram(9_010, SiteMode::Family, ConfirmPolicy::Never);
        // The rule asks 5 s after notice: after the second burst's late evidence (3 s).
        let mut d = ArmDrive::new(with.then_some(cfg), 5_000 * MS);
        d.play(&b0, 0, 4_000);
        // Two answers about the first burst arrive (two calls agreeing: the strength a recall
        // needs); the first is declared as the reasoner's and both are bound.
        let answer = Answer {
            focus: ObsId(0),
            diagnosis: hard(HardKind::Compound, 0),
        };
        let declared = d.step(4_500, &[], &[answer, answer]);
        assert!(declared.iter().any(|(s, a)| *s == Source::Reasoner
            && matches!(a, StreamAction::Declare { diagnosis, .. } if *diagnosis == answer.diagnosis)));
        let second = d.play(&b1, 5_000, 30_000);
        let asked = asked_about_second(&second);
        let recalled: Vec<&(u64, Source, StreamAction)> = second
            .iter()
            .filter(|(_, s, _)| *s == Source::Recall)
            .collect();
        if with {
            assert!(
                asked.is_empty(),
                "a recalled anomaly is never escalated: {asked:?}"
            );
            assert_eq!(recalled.len(), 1);
            let (at, _, StreamAction::Declare { anchor, diagnosis }) = recalled[0] else {
                panic!("a recall is a declaration");
            };
            assert_eq!(*diagnosis, hard(HardKind::Compound, 1));
            assert!(anchor.0 >= 6, "anchored in the second burst");
            assert_eq!(
                *at, 23_500,
                "after the late evidence, before the rule's 25.5 s"
            );
            // The shared rule never declares for it after the recall.
            assert!(!second.iter().any(|(t, s, a)| *s == Source::CheapRung
                && *t >= 23_500
                && matches!(a, StreamAction::Declare { anchor: x, .. } if x.0 >= 6)));
        } else {
            assert_eq!(asked.len(), 1, "the control asks about the second burst");
            assert!(recalled.is_empty());
        }
        carry::reset(9_010);
    }
}

#[test]
fn a_rule_that_asks_at_notice_is_not_preempted_by_a_recall_that_waits_for_late_evidence() {
    let cfg = engram(9_013, SiteMode::Family, ConfirmPolicy::Never);
    let mut d = ArmDrive::new(Some(cfg), 0);
    d.play(&burst(0, 1_000), 0, 4_000);
    let answer = Answer {
        focus: ObsId(0),
        diagnosis: hard(HardKind::Compound, 0),
    };
    d.step(4_500, &[], &[answer, answer]);
    let second = d.play(&burst(1, 20_000), 5_000, 30_000);
    // Asked at notice, before the late evidence: the recall comes after the question and the
    // arm drops it (an anomaly already asked about is not recalled).
    assert_eq!(asked_about_second(&second).len(), 1);
    assert!(second.iter().all(|(_, s, _)| *s != Source::Recall));
    carry::reset(9_013);
}

#[test]
fn a_confirmed_recall_is_asked_about_and_not_declared() {
    let cfg = engram(9_011, SiteMode::Family, ConfirmPolicy::Every { k: 1 });
    let mut d = ArmDrive::new(Some(cfg), 5_000 * MS);
    d.play(&burst(0, 1_000), 0, 4_000);
    let answer = Answer {
        focus: ObsId(0),
        diagnosis: hard(HardKind::Compound, 0),
    };
    d.step(4_500, &[], &[answer, answer]);
    let second = d.play(&burst(1, 20_000), 5_000, 30_000);
    assert!(second.iter().all(|(_, s, _)| *s != Source::Recall));
    // Asked once, at the recall (23.5 s), not at the rule's 25.5 s.
    let asked: Vec<u64> = second
        .iter()
        .filter_map(|(t, _, a)| match a {
            StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } if focus.0 >= 6 => Some(*t),
            _ => None,
        })
        .collect();
    assert_eq!(asked, vec![23_500]);
    let _ = ObsRef::Passive(ObsId(0));
    carry::reset(9_011);
}
