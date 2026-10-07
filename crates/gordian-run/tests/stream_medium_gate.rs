//! The recall gate and the two-site key of the medium noticer's engram layer (work item A1c): the
//! gate's public reading, the displacement test (on an anomaly the public checker explains, a gated
//! arm does what the arm without memory does), the gate's wait, the late-feature switch, and the
//! two-site key from relation events to a recall at the pair's first node.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test
//! (`experiments/exploration/scripts/a1c_gate.py`).

mod stream_common;

use gordian_core::Instant;
use gordian_medium::{FeatureRole, KeySite, Tag};
use gordian_run::stream::arms::medium::engram::{RELATION, carry, relation_tag};
use gordian_run::stream::arms::medium::{
    ConfirmPolicy, EngramConfig, MediumNoticer, MediumParams, RecallGate, SiteMode,
};
use gordian_run::stream::arms::noticer::{MemoryRecall, Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{AnomalyView, Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::arms::{Source, StepInput};
use gordian_run::stream::spec::{StreamPolicySpec, build_public};
use gordian_stream::{
    Answer, Diagnosis, HardKind, ObsId, Question, StreamAction, StreamEvent, StreamHypothesis,
    StreamKind, StreamPublic,
};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::SignalText;
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

fn public() -> StreamPublic {
    public_of(&params(0, 150))
}

/// A1a's test burst at `site` (`tests/stream_medium_engram.rs`): two alarms, a catalogue message,
/// a free-form message and a third alarm within 40 ms, then 3 s later an alarm of kind `late`.
/// With `late = Saturation` the first world's checker explains it (`ResourceExhausted` at the
/// site permits every symptom); with `late = AuthFailures` it does not (no kind permits both
/// `OutOfResource` and `AuthFailures`).
fn burst(site: u32, t: u64, late: CounterName) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 120)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, message(site, 0x00AB_CDEF_0123_4567)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
        (t + 3_000, counter(site, late, 90)),
    ]
}

fn hard(kind: HardKind, site: u32) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site: ServiceId(site),
    })
}

fn engram(key: u64, gate: RecallGate, late: Option<bool>, bind: bool) -> EngramConfig {
    carry::reset(key);
    EngramConfig {
        state_key: key,
        bind,
        site: SiteMode::Family,
        confirm: ConfirmPolicy::Never,
        gate,
        late_feature: late,
        ..EngramConfig::default()
    }
}

fn with_engram(engram: Option<EngramConfig>) -> MediumParams {
    MediumParams {
        ramp: false,
        engram,
        ..MediumParams::default()
    }
}

// ---- the gate's reading and the switches

fn view(contradicted_since: Option<Instant>) -> AnomalyView {
    AnomalyView {
        id: 0,
        site: ServiceId(0),
        anchor: ObsId(0),
        anchor_at: Instant(0),
        noticed_at: Instant(0),
        score: 0.0,
        peak_score: 0.0,
        digest: 0,
        attempts: 0,
        pending: 0,
        answered: 0,
        last_attempt_at: None,
        last_attempt_digest: None,
        cheap_declared: true,
        delivered: 0,
        evidence: 0,
        contradicted_since,
        services: 1,
    }
}

#[test]
fn the_gate_reads_the_public_checkers_verdict_and_nothing_else() {
    let g = RecallGate::Contradicted;
    assert!(!g.admits(&view(None)));
    assert!(g.admits(&view(Some(Instant(5)))));
    // Nothing else in the view moves it: score, age, a cheap declaration, the evidence.
    let mut v = view(None);
    v.score = 99.0;
    v.peak_score = 99.0;
    v.evidence = 40;
    v.services = 5;
    v.cheap_declared = false;
    assert!(!g.admits(&v));
    assert!(RecallGate::None.admits(&view(None)));
    assert!(g.needs_verdicts() && !RecallGate::None.needs_verdicts());
}

#[test]
fn the_late_switch_defaults_off_for_a_gated_layer_and_on_for_an_ungated_one() {
    let mut c = EngramConfig::default();
    assert!(c.requires_late(), "ungated: A1a's requirement");
    c.gate = RecallGate::Contradicted;
    assert!(!c.requires_late(), "gated: off by default");
    c.late_feature = Some(true);
    assert!(c.requires_late());
    c.gate = RecallGate::None;
    c.late_feature = Some(false);
    assert!(!c.requires_late());
}

#[test]
fn an_a1a_configuration_reads_and_writes_as_before_and_the_new_fields_are_checked() {
    // A1a's text (no gate, no late switch, no two-site field) reads as A1a's form and writes back
    // without the new fields.
    let a1a = json!({
        "state_key": 7, "site": "family", "generalise": true, "confirm": "never",
        "onset_ns": 2_000_000_000u64, "key_span_ns": 10_000_000_000u64, "min_features": 2,
        "gain": 1.0, "threshold": 1.5, "max_strength": 4.0, "penalty": 1.0, "decay": 0.99,
        "decay_period_ns": 100_000_000_000u64, "refractory_ns": 6_000_000_000u64
    });
    let c: EngramConfig = serde_json::from_value(a1a).unwrap();
    assert_eq!(c.gate, RecallGate::None);
    assert_eq!((c.late_feature, c.two_site), (None, false));
    assert!(c.requires_late());
    let text = serde_json::to_value(c).unwrap();
    for f in ["gate", "late_feature", "two_site"] {
        assert!(
            text.get(f).is_none(),
            "{f} is not written for an A1a configuration"
        );
    }
    let mut g = c;
    g.gate = RecallGate::Contradicted;
    g.two_site = true;
    g.late_feature = Some(true);
    let text = serde_json::to_value(g).unwrap();
    assert_eq!(text["gate"], json!("contradicted"));
    assert_eq!(text["two_site"], json!(true));
    assert_eq!(serde_json::from_value::<EngramConfig>(text).unwrap(), g);
    let spec = |c: EngramConfig| NoticerSpec::Medium(with_engram(Some(c))).validate();
    assert!(spec(g).is_ok());
    let mut bad = g;
    bad.site = SiteMode::Site;
    assert!(spec(bad).is_err(), "the two-site key is family-keyed only");
}

// ---- the arm: the displacement test

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

    /// Bind the pattern of `burst(0, 1_000, late)` to a hard diagnosis (two agreeing answers,
    /// the strength a recall needs), then play `burst(1, 20_000, late)`: the actions about it.
    fn learn_then_play(&mut self, late: CounterName) -> Vec<(u64, Source, StreamAction)> {
        self.play(&burst(0, 1_000, late), 0, 4_000);
        let answer = Answer {
            focus: ObsId(0),
            diagnosis: hard(HardKind::Compound, 0),
        };
        self.step(4_500, &[], &[answer, answer]);
        self.play(&burst(1, 20_000, late), 5_000, 30_000)
    }
}

fn recalled(actions: &[(u64, Source, StreamAction)]) -> Vec<(u64, Diagnosis)> {
    actions
        .iter()
        .filter_map(|(t, s, a)| match (s, a) {
            (Source::Recall, StreamAction::Declare { diagnosis, .. }) => Some((*t, *diagnosis)),
            _ => None,
        })
        .collect()
}

fn escalated(actions: &[(u64, Source, StreamAction)]) -> Vec<u64> {
    actions
        .iter()
        .filter_map(|(t, _, a)| match a {
            StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } if focus.0 >= 6 => Some(*t),
            _ => None,
        })
        .collect()
}

fn cheap(actions: &[(u64, Source, StreamAction)]) -> Vec<(u64, Diagnosis)> {
    actions
        .iter()
        .filter_map(|(t, s, a)| match (s, a) {
            (Source::CheapRung, StreamAction::Declare { anchor, diagnosis }) if anchor.0 >= 6 => {
                Some((*t, *diagnosis))
            }
            _ => None,
        })
        .collect()
}

/// The brief's displacement test (A1c item 1). On an anomaly whose evidence the public checker
/// explains (`ResourceExhausted` at the site), the ungated memory recalls, adds a wrong declaration
/// and keeps the anomaly from being asked about; the gated memory does exactly what the arm
/// without memory does: the cheap rung's correct declaration, the escalation, nothing recalled.
#[test]
fn a_gated_recall_displaces_no_declaration_where_the_public_checker_explains_the_anomaly() {
    let late = CounterName::Saturation;
    let control = ArmDrive::new(
        Some(engram(9_100, RecallGate::None, Some(false), false)),
        5_000 * MS,
    )
    .learn_then_play(late);
    let ungated = ArmDrive::new(
        Some(engram(9_101, RecallGate::None, Some(false), true)),
        5_000 * MS,
    )
    .learn_then_play(late);
    let gated = ArmDrive::new(
        Some(engram(9_102, RecallGate::Contradicted, Some(false), true)),
        5_000 * MS,
    )
    .learn_then_play(late);

    // The control: the shared rule declares the right diagnosis, and the rule asks.
    let right = Some(StreamHypothesis {
        kind: StreamKind::Known(gordian_world::FaultKind::ResourceExhausted),
        site: ServiceId(1),
    });
    assert_eq!(cheap(&control).len(), 1);
    assert_eq!(cheap(&control)[0].1, right);
    assert_eq!(escalated(&control).len(), 1);
    assert!(recalled(&control).is_empty());

    // Ungated: the memory recalls the learned diagnosis; the anomaly is never asked about.
    assert_eq!(
        recalled(&ungated),
        vec![(23_500, hard(HardKind::Compound, 1))]
    );
    assert!(escalated(&ungated).is_empty());

    // Gated: every action, at every step, is the control's.
    assert_eq!(gated, control);
    for k in [9_100, 9_101, 9_102] {
        carry::reset(k);
    }
}

/// Where the public checker finds no consistent hypothesis, the gated recall is declared as the
/// ungated one is, at the same step.
#[test]
fn a_gated_recall_is_declared_where_the_public_checker_cannot_explain_the_anomaly() {
    let late = CounterName::AuthFailures;
    let ungated = ArmDrive::new(
        Some(engram(9_103, RecallGate::None, Some(false), true)),
        5_000 * MS,
    )
    .learn_then_play(late);
    let gated = ArmDrive::new(
        Some(engram(9_104, RecallGate::Contradicted, Some(false), true)),
        5_000 * MS,
    )
    .learn_then_play(late);
    let want = vec![(23_500, hard(HardKind::Compound, 1))];
    assert_eq!(recalled(&ungated), want);
    assert_eq!(recalled(&gated), want);
    assert!(
        escalated(&gated).is_empty(),
        "a recalled anomaly is never escalated"
    );
    for k in [9_103, 9_104] {
        carry::reset(k);
    }
}

// ---- the noticer: the gate's wait, and the two-site key

/// Drives a noticer as the rung does, every 500 ms; answers between steps; the recalls handed
/// over at each step, through the gate with the views `views` gives.
struct Drive {
    noticer: MediumNoticer,
    all: Vec<Held>,
    next: usize,
    items: Vec<Held>,
    now_ms: u64,
    recalls: Vec<(u64, MemoryRecall)>,
}

impl Drive {
    fn new(engram: EngramConfig, public: &StreamPublic, obs: &[(u64, Observation)]) -> Self {
        let mut obs = obs.to_vec();
        obs.sort_by_key(|(t, _)| *t);
        Self {
            noticer: MediumNoticer::new(
                with_engram(Some(engram)),
                RungConfig::default(),
                &public.services,
            )
            .unwrap(),
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
            recalls: Vec::new(),
        }
    }

    fn until(&mut self, ms: u64, views: &dyn Fn(&MediumNoticer, u64) -> Vec<AnomalyView>) {
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
            let v = views(&self.noticer, self.now_ms);
            for r in self.noticer.gated_recalls(Instant(now), &v) {
                self.recalls.push((self.now_ms, r));
            }
            for id in self.noticer.retirable(Instant(now)) {
                self.noticer.retire(id);
            }
            self.now_ms += 500;
        }
    }
}

/// Views of the noticed anomalies, contradicted from `from_ms` on (never with `None`).
fn views_from(from_ms: Option<u64>) -> impl Fn(&MediumNoticer, u64) -> Vec<AnomalyView> {
    move |n: &MediumNoticer, now_ms: u64| {
        n.anomalies()
            .iter()
            .filter(|a| a.noticed_at.is_some())
            .map(|a| {
                let mut v = view(from_ms.filter(|f| now_ms >= *f).map(|f| Instant(f * MS)));
                v.id = a.id;
                v.site = a.site;
                v
            })
            .collect()
    }
}

fn gate_drive(key: u64, contradicted_from_ms: Option<u64>) -> Drive {
    let mut obs = burst(0, 1_000, CounterName::Saturation);
    obs.extend(burst(1, 20_000, CounterName::Saturation));
    let public = public();
    let mut d = Drive::new(
        engram(key, RecallGate::Contradicted, Some(false), true),
        &public,
        &obs,
    );
    let v = views_from(contradicted_from_ms);
    d.until(4_000, &v);
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.until(30_000, &v);
    d
}

#[test]
fn the_gate_waits_one_review_period_then_drops_the_recall() {
    // The engram recalls at 23.5 s (A1a's test). Contradicted at the recall: handed over at once.
    let d = gate_drive(9_110, Some(23_500));
    assert_eq!(d.recalls.len(), 1);
    assert_eq!(d.recalls[0].0, 23_500);
    assert_eq!(d.recalls[0].1.diagnosis, hard(HardKind::Compound, 1));
    // Contradicted one review period (500 ms) after the recall: handed over then.
    let d = gate_drive(9_111, Some(24_000));
    assert_eq!(
        d.recalls.iter().map(|r| r.0).collect::<Vec<_>>(),
        vec![24_000]
    );
    // Contradicted later than that: dropped, and counted.
    let d = gate_drive(9_112, Some(24_500));
    assert!(d.recalls.is_empty());
    let s = d.noticer.engram().unwrap().stats();
    assert_eq!((s.gate_offered, s.gate_admitted, s.gate_closed), (1, 0, 1));
    // Never contradicted: dropped.
    let d = gate_drive(9_113, None);
    assert!(d.recalls.is_empty());
    for k in 9_110..=9_113 {
        carry::reset(k);
    }
}

#[test]
fn relation_tags_carry_the_order_and_the_gap_in_three_bands() {
    let (b, g) = (400 * MS, 2_000 * MS);
    assert_eq!(relation_tag(false, 0, b, g), Tag(RELATION));
    assert_eq!(relation_tag(false, 399 * MS, b, g), Tag(RELATION));
    assert_eq!(relation_tag(false, 400 * MS, b, g), Tag(RELATION + 1));
    assert_eq!(relation_tag(true, 1_999 * MS, b, g), Tag(RELATION + 4 + 1));
    assert_eq!(relation_tag(true, 2_000 * MS, b, g), Tag(RELATION + 4 + 2));
    assert_eq!(relation_tag(false, 10_000 * MS, b, g), Tag(RELATION + 2));
}

/// Two services the public graph does not connect, and one connected to the first, in the test
/// stream's graph.
fn unconnected_pairs(public: &StreamPublic) -> Vec<(u32, u32)> {
    let s = &public.services;
    let masks: Vec<Vec<bool>> = s.iter().map(|x| dependents_mask(s, x.id)).collect();
    let n = s.len();
    (0..n)
        .flat_map(|a| (0..n).map(move |b| (a, b)))
        .filter(|&(a, b)| a != b && !masks[a][b] && !masks[b][a])
        .map(|(a, b)| (a as u32, b as u32))
        .collect()
}

/// A partner's alarms at `site` from `t`: a latency alarm and two error alarms within 20 ms.
fn partner(site: u32, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::Latency, 120)),
        (t + 10, counter(site, CounterName::ErrorRate, 80)),
        (t + 20, counter(site, CounterName::ErrorRate, 85)),
    ]
}

fn two_site(key: u64) -> EngramConfig {
    let mut c = engram(key, RecallGate::None, Some(false), true);
    c.two_site = true;
    c
}

#[test]
fn a_two_site_key_binds_the_pair_and_recalls_at_the_pair_elsewhere() {
    let public = public();
    let pairs = unconnected_pairs(&public);
    let (a, b) = pairs[0];
    // A second unconnected pair with other services.
    let &(c, e) = pairs
        .iter()
        .find(|(x, y)| ![a, b].contains(x) && ![a, b].contains(y))
        .expect("the test graph has two disjoint unconnected pairs");
    // Training: A alarms at 1 s, its unconnected partner B 1 s later (band 1, partner after).
    let mut obs = burst(a, 1_000, CounterName::Saturation);
    obs.extend(partner(b, 2_000));
    // Recall: the same at C and E from 40 s.
    obs.extend(burst(c, 40_000, CounterName::Saturation));
    obs.extend(partner(e, 41_000));
    let mut d = Drive::new(two_site(9_120), &public, &obs);
    let none = views_from(Some(0));
    d.until(6_000, &none);
    let focus = d.all.iter().position(|h| h.at.0 == 1_000 * MS).unwrap() as u32;
    d.noticer.answered(ObsId(focus), hard(HardKind::Cascade, a));
    d.noticer.answered(ObsId(focus), hard(HardKind::Cascade, a));
    let layer = d.noticer.engram().unwrap();
    assert_eq!(layer.stats().pair_binds, 2);
    let g = &layer.engrams().engrams()[0];
    assert_eq!(g.key.site, KeySite::Pair);
    assert_eq!(g.key.roles()[0], FeatureRole::Relation);
    assert_eq!(
        g.key.features()[0],
        Tag(RELATION + 1),
        "partner after, within 2 s"
    );
    assert_eq!(g.key.roles()[1], FeatureRole::Partner);
    assert!(layer.stats().relation_events >= 2);
    d.until(50_000, &none);
    let rs: Vec<&MemoryRecall> = d.recalls.iter().map(|(_, r)| r).collect();
    assert_eq!(rs.len(), 1, "{rs:?}");
    assert_eq!(
        rs[0].diagnosis,
        hard(HardKind::Cascade, c),
        "at the pair's first node"
    );
    assert!(d.noticer.engram().unwrap().stats().pair_recalls >= 1);
    carry::reset(9_120);
}

#[test]
fn a_two_site_engram_does_not_recall_without_its_partner_or_with_another_gap() {
    let public = public();
    let pairs = unconnected_pairs(&public);
    let (a, b) = pairs[0];
    let &(c, e) = pairs
        .iter()
        .find(|(x, y)| ![a, b].contains(x) && ![a, b].contains(y))
        .unwrap();
    let train = |obs: &mut Vec<(u64, Observation)>| {
        obs.extend(burst(a, 1_000, CounterName::Saturation));
        obs.extend(partner(b, 2_000));
    };
    for (case, test) in [
        ("no partner", burst(c, 40_000, CounterName::Saturation)),
        ("partner 5 s later", {
            let mut o = burst(c, 40_000, CounterName::Saturation);
            o.extend(partner(e, 45_000));
            o
        }),
    ] {
        let mut obs = Vec::new();
        train(&mut obs);
        obs.extend(test);
        let mut d = Drive::new(two_site(9_121), &public, &obs);
        let none = views_from(Some(0));
        d.until(6_000, &none);
        let focus = d.all.iter().position(|h| h.at.0 == 1_000 * MS).unwrap() as u32;
        d.noticer.answered(ObsId(focus), hard(HardKind::Cascade, a));
        d.noticer.answered(ObsId(focus), hard(HardKind::Cascade, a));
        d.until(60_000, &none);
        assert!(d.recalls.is_empty(), "{case}: {:?}", d.recalls);
        carry::reset(9_121);
    }
}

#[test]
fn an_anomaly_with_no_unconnected_partner_keeps_the_one_site_key() {
    let public = public();
    let s = &public.services;
    // A service and one connected to it (a dependent), if the graph has one.
    let Some((a, dep)) = (0..s.len()).find_map(|a| {
        let m = dependents_mask(s, s[a].id);
        m.iter().position(|x| *x).map(|d| (a as u32, d as u32))
    }) else {
        return;
    };
    let mut obs = burst(a, 1_000, CounterName::Saturation);
    obs.extend(partner(dep, 1_200));
    // No other service alarms within the span.
    let mut d = Drive::new(two_site(9_122), &public, &obs);
    let none = views_from(Some(0));
    d.until(6_000, &none);
    let focus = d.all.iter().position(|h| h.at.0 == 1_000 * MS).unwrap() as u32;
    d.noticer.answered(ObsId(focus), hard(HardKind::Cascade, a));
    let layer = d.noticer.engram().unwrap();
    assert_eq!(layer.stats().pair_binds, 0);
    assert_eq!(layer.stats().binds, 1);
    assert_eq!(layer.engrams().engrams()[0].key.site, KeySite::Variable);
    carry::reset(9_122);
}
