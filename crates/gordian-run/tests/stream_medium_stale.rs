//! Where a recall of the engram layer may speak (work item A1d, item 2): the `stale` gate. A
//! recall is declared only on an anomaly the public checker cannot explain now and that carries no
//! declaration made strictly after the checker's last consistent verdict: memory corrects a cheap
//! declaration the rules have since contradicted, and never adds to one that stands. Also the
//! trace's counters (item 3), read from the layer's marks.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::medium::engram::carry;
use gordian_run::stream::arms::medium::trace::{
    self as etrace, BindNote, FILE_HEADER, NONE, TraceKind,
};
use gordian_run::stream::arms::medium::{
    ConfirmPolicy, EngramConfig, MediumNoticer, MediumParams, Reading, RecallGate, SiteMode,
};
use gordian_run::stream::arms::noticer::{MemoryRecall, Noticer, NoticerSpec};
use gordian_run::stream::arms::rung::{AnomalyView, Held, RungConfig, Store, is_abnormal};
use gordian_run::stream::arms::{Source, StepInput};
use gordian_run::stream::spec::{StreamPolicySpec, build_public};
use gordian_stream::{
    Answer, Diagnosis, HardKind, ObsId, Question, StreamAction, StreamEvent, StreamHypothesis,
    StreamKind, StreamPublic,
};
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

/// A1c's test burst (`tests/stream_medium_gate.rs`): the evidence at 0 to 40 ms is explained by
/// `ResourceExhausted` at the site; 3 s later an alarm of kind `late`. With `late = AuthFailures`
/// the checker finds no consistent hypothesis from then on: a cheap declaration made at 0.5 s on
/// a consistent verdict has since been contradicted (stale).
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

/// A pattern whose contradicting alarm comes early (at 30 ms, before the cheap rung's first
/// review: no kind permits both `OutOfResource` and `AuthFailures`) and a saturation alarm 3 s
/// later. With `latency` (a latency alarm at 10 ms) the cheap rung declares at its first review
/// on evidence the checker has never explained (the shared rule's fallback), so its declaration
/// stands when the recall comes; without it the pattern has six key features before the
/// saturation alarm, so a key bound on it holds the saturation alarm, and a recall of it waits for
/// that alarm.
fn contradicted_early(site: u32, t: u64, latency: bool) -> Vec<(u64, Observation)> {
    let mut out = vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, counter(site, CounterName::AuthFailures, 90)),
        (t + 40, counter(site, CounterName::ErrorRate, 85)),
        (t + 3_000, counter(site, CounterName::Saturation, 90)),
    ];
    if latency {
        out.push((t + 10, counter(site, CounterName::Latency, 120)));
        out.sort_by_key(|(at, _)| *at);
    }
    out
}

fn hard(kind: HardKind, site: u32) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site: ServiceId(site),
    })
}

fn engram(key: u64, gate: RecallGate, bind: bool) -> EngramConfig {
    carry::reset(key);
    etrace::reset_segments(key);
    EngramConfig {
        state_key: key,
        bind,
        site: SiteMode::Family,
        confirm: ConfirmPolicy::Never,
        gate,
        late_feature: Some(false),
        trace: true,
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

// ---- the gate's reading

#[test]
fn the_stale_gate_reads_the_verdict_and_standing_declarations() {
    let g = RecallGate::Stale;
    assert!(g.needs_verdicts() && g.reads_declarations());
    assert!(!RecallGate::Contradicted.reads_declarations());
    let c = view(Some(Instant(5)));
    assert_eq!(g.reading(&c, false), Reading::Admit);
    assert_eq!(g.reading(&c, true), Reading::Standing);
    assert_eq!(g.reading(&view(None), false), Reading::Consistent);
    assert_eq!(g.reading(&view(None), true), Reading::Consistent);
    // A1c's gate ignores declarations.
    assert_eq!(RecallGate::Contradicted.reading(&c, true), Reading::Admit);
    // The text: `stale`, and the trace switch is written only when on.
    let mut cfg = EngramConfig {
        gate: g,
        ..EngramConfig::default()
    };
    let text = serde_json::to_value(cfg).unwrap();
    assert_eq!(text["gate"], json!("stale"));
    assert!(text.get("trace").is_none());
    cfg.trace = true;
    let text = serde_json::to_value(cfg).unwrap();
    assert_eq!(text["trace"], json!(true));
    assert_eq!(serde_json::from_value::<EngramConfig>(text).unwrap(), cfg);
    assert!(
        NoticerSpec::Medium(with_engram(Some(cfg)))
            .validate()
            .is_ok()
    );
}

// ---- the arm: where a recall may speak

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

    /// Bind the pattern `learn(0, 1_000)` to a hard diagnosis (two agreeing answers, the
    /// strength a recall needs), then play `pattern(1, 20_000)`: the actions about it.
    fn learn_then_play(
        &mut self,
        learn: &dyn Fn(u32, u64) -> Vec<(u64, Observation)>,
        pattern: &dyn Fn(u32, u64) -> Vec<(u64, Observation)>,
    ) -> Vec<(u64, Source, StreamAction)> {
        self.play(&learn(0, 1_000), 0, 4_000);
        let answer = Answer {
            focus: ObsId(0),
            diagnosis: hard(HardKind::Compound, 0),
        };
        self.step(4_500, &[], &[answer, answer]);
        self.play(&pattern(1, 20_000), 5_000, 30_000)
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

/// The play phase's escalations (the pattern at service 1 begins at 20 s).
fn escalated(actions: &[(u64, Source, StreamAction)]) -> Vec<u64> {
    actions
        .iter()
        .filter_map(|(t, _, a)| match a {
            StreamAction::Escalate {
                question: Question::Diagnose { focus },
                ..
            } if *t >= 20_000 && focus.0 > 0 => Some(*t),
            _ => None,
        })
        .collect()
}

/// The play phase's cheap declarations.
fn cheap(actions: &[(u64, Source, StreamAction)]) -> Vec<(u64, Diagnosis)> {
    actions
        .iter()
        .filter_map(|(t, s, a)| match (s, a) {
            (Source::CheapRung, StreamAction::Declare { anchor, diagnosis })
                if *t >= 20_000 && anchor.0 > 0 =>
            {
                Some((*t, *diagnosis))
            }
            _ => None,
        })
        .collect()
}

const DELAY: u64 = 5_000 * MS;

fn three(
    keys: [u64; 3],
    learn: &dyn Fn(u32, u64) -> Vec<(u64, Observation)>,
    pattern: &dyn Fn(u32, u64) -> Vec<(u64, Observation)>,
) -> [Vec<(u64, Source, StreamAction)>; 3] {
    let out = [
        // The arm without memory (the layer, bind off, the same gate: the same checks).
        ArmDrive::new(Some(engram(keys[0], RecallGate::Stale, false)), DELAY)
            .learn_then_play(learn, pattern),
        // A1c's gate.
        ArmDrive::new(Some(engram(keys[1], RecallGate::Contradicted, true)), DELAY)
            .learn_then_play(learn, pattern),
        // A1d's gate.
        ArmDrive::new(Some(engram(keys[2], RecallGate::Stale, true)), DELAY)
            .learn_then_play(learn, pattern),
    ];
    for k in keys {
        carry::reset(k);
        etrace::reset_segments(k);
    }
    out
}

/// The brief's test (A1d item 2), first half. The cheap rung declares on a consistent verdict;
/// later evidence makes the checker empty. The declaration is stale, and the stale-gated memory
/// speaks beside it, at the step A1c's gate does; the anomaly is then never asked about.
#[test]
fn memory_corrects_a_cheap_declaration_the_rules_have_since_contradicted() {
    let pattern = |s, t| burst(s, t, CounterName::AuthFailures);
    let [control, a1c, stale] = three([9_200, 9_201, 9_202], &pattern, &pattern);
    // The control: the cheap rung declares before the contradicting alarm, and the rule asks.
    let decl = cheap(&control);
    assert_eq!(decl.len(), 1, "{control:?}");
    assert!(decl[0].0 < 23_000, "declared before the contradiction");
    assert_eq!(escalated(&control).len(), 1);
    assert!(recalled(&control).is_empty());
    // A1c's gate and A1d's: the same recall at the same step, beside the cheap declaration.
    let want = vec![(23_500, hard(HardKind::Compound, 1))];
    assert_eq!(recalled(&a1c), want);
    assert_eq!(recalled(&stale), want);
    assert_eq!(cheap(&stale), decl);
    assert!(escalated(&stale).is_empty());
}

/// The brief's test (A1d item 2), second half. The checker never explains the evidence, so the
/// cheap rung's declaration is made after its last consistent verdict (there is none) and stands.
/// A1c's gate lets the memory add to it; the stale gate does not: every action, at every step, is
/// the arm's without memory.
#[test]
fn memory_never_adds_to_a_declaration_that_stands() {
    let [control, a1c, stale] = three(
        [9_210, 9_211, 9_212],
        &|s, t| contradicted_early(s, t, false),
        &|s, t| contradicted_early(s, t, true),
    );
    let decl = cheap(&control);
    assert_eq!(decl.len(), 1, "the cheap rung declares: {control:?}");
    assert!(decl[0].0 < 23_000, "before the recall: {decl:?}");
    let a1c_recalls = recalled(&a1c);
    assert_eq!(a1c_recalls.len(), 1, "A1c's gate adds the recall: {a1c:?}");
    assert!(a1c_recalls[0].0 > decl[0].0);
    assert!(recalled(&stale).is_empty());
    assert_eq!(stale, control);
}

// ---- the noticer: the gate's wait under a standing declaration, and the trace

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

    /// Step every 500 ms to `ms`: every noticed anomaly contradicted, and standing from
    /// `standing_ms` to `clear_ms` (half-open).
    fn until(&mut self, ms: u64, standing: Option<(u64, u64)>) {
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
            let views: Vec<AnomalyView> = self
                .noticer
                .anomalies()
                .iter()
                .filter(|a| a.noticed_at.is_some())
                .map(|a| {
                    let mut v = view(Some(Instant(0)));
                    v.id = a.id;
                    v.site = a.site;
                    v
                })
                .collect();
            let on = standing.is_some_and(|(a, b)| self.now_ms >= a && self.now_ms < b);
            let ids: Vec<u32> = if on {
                views.iter().map(|v| v.id).collect()
            } else {
                Vec::new()
            };
            self.noticer.standing_declarations(&ids);
            for r in self.noticer.gated_recalls(Instant(now), &views) {
                self.noticer.recall_declared(Instant(now), r.anomaly, true);
                self.recalls.push((self.now_ms, r));
            }
            self.now_ms += 500;
        }
    }
}

fn kinds(n: &MediumNoticer, kind: TraceKind) -> Vec<(u64, u32, u32, u32, i64)> {
    n.engram()
        .unwrap()
        .marks()
        .iter()
        .filter(|m| m.kind == kind.code())
        .map(|m| (m.at_ns / MS, m.subject, m.event, m.tag, m.value))
        .collect()
}

fn stale_drive(key: u64, standing: Option<(u64, u64)>) -> Drive {
    let mut obs = burst(0, 1_000, CounterName::Saturation);
    obs.extend(burst(1, 20_000, CounterName::Saturation));
    let public = public_of(&params(0, 150));
    let mut d = Drive::new(engram(key, RecallGate::Stale, true), &public, &obs);
    d.until(4_000, standing);
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.noticer.answered(ObsId(0), None);
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.noticer.answered(ObsId(99), hard(HardKind::Compound, 0));
    d.until(30_000, standing);
    d
}

#[test]
fn a_standing_declaration_holds_the_recall_for_the_wait_then_drops_it() {
    // No standing declaration: admitted at the recall (23.5 s, A1c's test).
    let d = stale_drive(9_220, None);
    assert_eq!(d.recalls.iter().map(|r| r.0).collect::<Vec<_>>(), [23_500]);
    // Standing at the recall, stale one review period later: admitted then.
    let d = stale_drive(9_221, Some((0, 24_000)));
    assert_eq!(d.recalls.iter().map(|r| r.0).collect::<Vec<_>>(), [24_000]);
    // Standing throughout the wait: dropped, and counted as standing.
    let d = stale_drive(9_222, Some((0, 60_000)));
    assert!(d.recalls.is_empty());
    let s = d.noticer.engram().unwrap().stats();
    assert_eq!((s.gate_offered, s.gate_admitted, s.gate_closed), (1, 0, 1));
    assert_eq!(kinds(&d.noticer, TraceKind::GatedStanding).len(), 1);
    assert!(kinds(&d.noticer, TraceKind::GatedConsistent).is_empty());
    for k in 9_220..=9_222 {
        carry::reset(k);
        etrace::reset_segments(k);
    }
}

#[test]
fn the_trace_counts_answers_by_kind_and_recalls_offered_admitted_declared_with_instants() {
    let d = stale_drive(9_230, None);
    let n = &d.noticer;
    // Answers: every one is marked, by outcome tag (hard compound is 6 + its index; "not an
    // incident" is 0), with what bind did, at the step that took it (4.5 s, the next step).
    let answers = kinds(n, TraceKind::Answer);
    assert_eq!(answers.len(), 5, "{answers:?}");
    assert!(answers.iter().all(|a| a.0 == 4_500), "{answers:?}");
    let notes: Vec<BindNote> = answers.iter().map(|a| BindNote::of(a.4).unwrap()).collect();
    assert_eq!(
        notes,
        [
            BindNote::Created,
            BindNote::Created,
            BindNote::Strengthened,
            BindNote::Strengthened,
            BindNote::Unheld,
        ]
    );
    assert_eq!(answers[1].3, 0, "not an incident");
    assert!(answers[0].3 >= 6, "a hard kind");
    assert_eq!((answers[4].1, answers[4].2), (NONE, 99));
    // The disagreeing answer weakened the first engram ("not an incident" against the hard
    // kind), and each later hard answer weakened the engram "not an incident" made: one engram
    // each time, marked after the answer that did it, with its tag.
    let c = kinds(n, TraceKind::Contradicted);
    assert_eq!(c.len(), 3, "{c:?}");
    assert!(c.iter().all(|x| x.0 == 4_500 && x.4 == 1), "{c:?}");
    assert_eq!(c[0].3, 0);
    assert!(c[1].3 >= 6 && c[2].3 >= 6);
    // One recall, at 23.5 s: made, offered, admitted, declared, all at that instant.
    for k in [
        TraceKind::Recall,
        TraceKind::Offered,
        TraceKind::Admitted,
        TraceKind::Declared,
    ] {
        let m = kinds(n, k);
        assert_eq!(m.len(), 1, "{k:?}: {m:?}");
        assert_eq!(m[0].0, 23_500, "{k:?}");
    }
    let offered = kinds(n, TraceKind::Offered)[0];
    assert_eq!(offered.1, d.recalls[0].1.anomaly);
    assert!(offered.3 >= 6);
    // The file's rows: one per mark, the answer's bind result named.
    let row = etrace::row(
        3,
        &n.engram()
            .unwrap()
            .marks()
            .iter()
            .find(|m| m.kind == TraceKind::Answer.code())
            .copied()
            .unwrap(),
    );
    assert!(row.starts_with("3,4500000000,answer,"), "{row}");
    assert!(row.ends_with(",0,created"), "{row}");
    assert_eq!(FILE_HEADER.split(',').count(), row.split(',').count());
    carry::reset(9_230);
    etrace::reset_segments(9_230);
}

#[test]
fn without_the_switch_the_layer_marks_nothing() {
    let mut cfg = engram(9_240, RecallGate::Stale, true);
    cfg.trace = false;
    let mut obs = burst(0, 1_000, CounterName::Saturation);
    obs.extend(burst(1, 20_000, CounterName::Saturation));
    let public = public_of(&params(0, 150));
    let mut d = Drive::new(cfg, &public, &obs);
    d.until(4_000, None);
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.noticer.answered(ObsId(0), hard(HardKind::Compound, 0));
    d.until(30_000, None);
    assert_eq!(d.recalls.len(), 1);
    assert!(d.noticer.engram().unwrap().marks().is_empty());
    carry::reset(9_240);
}

#[test]
fn the_trace_file_has_a_header_and_one_row_per_mark_appended_per_segment() {
    let dir = scratch("a1d-trace-file");
    let key = 9_250;
    let path = etrace::file_in(&dir, key);
    let _ = std::fs::remove_file(&path);
    let mark = |kind: TraceKind, value: i64| gordian_medium::Mark {
        at_ns: 1_500 * MS,
        kind: kind.code(),
        subject: 3,
        event: NONE,
        tag: 7,
        value,
    };
    let log = gordian_medium::MarkLog {
        marks: vec![
            mark(TraceKind::Answer, BindNote::Generalised.code()),
            mark(TraceKind::SegmentEnd, 2),
        ],
    };
    let before = etrace::write_errors();
    etrace::append_to(&dir, key, 0, &log);
    etrace::append_to(&dir, key, 1, &log);
    assert_eq!(etrace::write_errors(), before);
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        [
            FILE_HEADER,
            "0,1500000000,answer,3,,7,2,generalised",
            "0,1500000000,segment_end,3,,,2,",
            "1,1500000000,answer,3,,7,2,generalised",
            "1,1500000000,segment_end,3,,,2,",
        ]
    );
}
