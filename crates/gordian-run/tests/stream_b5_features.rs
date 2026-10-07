//! B5's feature log: the public score's features at the ask instant, for every anomaly that is live
//! then, beside the evaluator's class of the notice. The tool half is an ignored test (the unit's
//! report records its exit status); the other half checks that the logging arm asks about what
//! `always_escalate` asks about, so that a feature read by it is a feature read at the instant the
//! selector would read it.
//!
//! Tool: `B5_MANIFEST=manifest.json B5_ARMS=arm,arm B5_SEEDS=start,count B5_OUT=file.csv
//! cargo test --release -p gordian-run --test stream_b5_features -- --ignored --nocapture`. For each
//! named arm of the manifest it plays each seed with the arm's noticer (the manifest's
//! `StreamManifest::rung_for`), the rung's own context and retirement, and a rule that asks about
//! every anomaly `delay` after notice (B5_DELAY_S, default 16, R5's) and writes down the features
//! (`public_budgeted::Features`) of each anomaly at the step that makes it ready. One row per
//! notice of the arm, ready or not (an anomaly the rung retires before the delay has no features
//! and is a row with `ready` false).

mod stream_common;

use gordian_core::Instant;
use gordian_run::policy::PolicyId;
use gordian_run::stream::arms::EscalationRule;
use gordian_run::stream::arms::StreamArm;
use gordian_run::stream::arms::public_budgeted::Features;
use gordian_run::stream::arms::rung::AnomalyView;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::run_segment;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;
use stream_common::*;

const S: u64 = 1_000_000_000;

/// What the logger wrote down about one ready anomaly.
#[derive(Debug, Clone)]
struct Logged {
    now: Instant,
    view: AnomalyView,
    features: Features,
}

type Log = Rc<RefCell<BTreeMap<u32, Logged>>>;

/// Asks about every anomaly `delay_ns` after notice (as `always_escalate` does) and logs its
/// features at the step that makes it ready. Keeps the consistency checker's verdict current, as
/// the budgeted selector does.
struct Logger {
    delay_ns: u64,
    ask: bool,
    seen: BTreeSet<u32>,
    log: Log,
}

impl EscalationRule for Logger {
    fn id(&self) -> PolicyId {
        PolicyId::new("b5_feature_logger")
    }

    fn monitors(&self) -> bool {
        true
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut out = Vec::new();
        for v in views {
            if v.attempts == 0
                && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)
                && self.seen.insert(v.id)
            {
                self.log.borrow_mut().insert(
                    v.id,
                    Logged {
                        now,
                        view: v.clone(),
                        features: Features::of(v, now),
                    },
                );
                if self.ask {
                    out.push(v.id);
                }
            }
        }
        out
    }
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

#[test]
#[ignore = "a tool: see the module documentation"]
fn write_the_feature_log() {
    let text = std::fs::read_to_string(env("B5_MANIFEST")).unwrap();
    let manifest: StreamManifest = serde_json::from_str(&text).unwrap();
    manifest.validate().unwrap();
    let arms: Vec<String> = env("B5_ARMS").split(',').map(str::to_owned).collect();
    let seeds: Vec<u64> = env("B5_SEEDS")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let delay_s: u64 = std::env::var("B5_DELAY_S")
        .ok()
        .map_or(16, |s| s.parse().unwrap());
    let mut out = String::from(
        "arm,seed,anomaly,class,incident,ready,now_ns,noticed_at_ns,anchor_at_ns,contradiction,silence,evidence_n,services_n,age_s,score_z,peak_z,evidence,services,age,asked,first_escalation_at_ns\n",
    );
    for arm in &arms {
        let spec = manifest
            .arms
            .iter()
            .find(|a| &a.arm == arm)
            .unwrap_or_else(|| panic!("no arm {arm}"));
        let rung = manifest.rung_for(spec);
        for seed in seeds[0]..seeds[0] + seeds[1] {
            let params = manifest.params_for(seed);
            let log: Log = Rc::new(RefCell::new(BTreeMap::new()));
            let sink = Rc::clone(&log);
            let r = rung.clone();
            let record = run_segment(
                &params,
                &move |public: &gordian_stream::StreamPublic| {
                    Box::new(StreamArm::with(
                        Logger {
                            delay_ns: delay_s * S,
                            ask: true,
                            seen: BTreeSet::new(),
                            log: Rc::clone(&sink),
                        },
                        public,
                        r.clone(),
                    ))
                },
                &manifest.limits,
                &manifest.exchange,
            )
            .unwrap_or_else(|e| panic!("{arm} seed {seed}: {e}"));
            let log = log.borrow();
            for n in &record.selection.per_notice {
                let class = n.class.as_str();
                let incident = n.incident.map_or(String::new(), |i| i.to_string());
                let asked = n.escalations;
                let first = n
                    .first_escalation_at
                    .map_or(String::new(), |t| t.0.to_string());
                match log.get(&n.anomaly) {
                    Some(l) => {
                        let f = l.features;
                        writeln!(
                            out,
                            "{arm},{seed},{},{class},{incident},true,{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{asked},{first}",
                            n.anomaly,
                            l.now.0,
                            l.view.noticed_at.0,
                            l.view.anchor_at.0,
                            f.contradiction as u8,
                            f.silence as u8,
                            l.view.evidence,
                            l.view.services,
                            l.now.0.saturating_sub(l.view.anchor_at.0) as f64 / 1e9,
                            l.view.score,
                            l.view.peak_score,
                            f.evidence,
                            f.services,
                            f.age,
                        )
                        .unwrap();
                    }
                    None => {
                        writeln!(
                            out,
                            "{arm},{seed},{},{class},{incident},false,,,,,,,,,,,,,,{asked},{first}",
                            n.anomaly
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    std::fs::write(env("B5_OUT"), out).unwrap();
}

#[test]
fn the_logging_arm_asks_about_what_always_escalate_asks_about() {
    // Over short real segments: the notices asked about by the logger (which also keeps the checker
    // current) are those an always-escalate arm at the same delay asks about.
    let mut m = manifest("b5-logger", &[("always", "always_escalate")], 3, 300, 11);
    m.arms[0].policy = gordian_run::stream::spec::StreamPolicySpec::Always { delay_ns: 4 * S };
    let rung = m.rung_for(&m.arms[0]);
    for seed in 0..3u64 {
        let params = m.params_for(seed);
        let spec = m.arms[0].policy.clone();
        let always = play_with_rung(&params, &spec, &m.limits, &rung).unwrap();
        let log: Log = Rc::new(RefCell::new(BTreeMap::new()));
        let sink = Rc::clone(&log);
        let r = rung.clone();
        let logged = run_segment(
            &params,
            &move |public: &gordian_stream::StreamPublic| {
                Box::new(StreamArm::with(
                    Logger {
                        delay_ns: 4 * S,
                        ask: true,
                        seen: BTreeSet::new(),
                        log: Rc::clone(&sink),
                    },
                    public,
                    r.clone(),
                ))
            },
            &m.limits,
            &m.exchange,
        )
        .unwrap();
        let asked = |rec: &gordian_run::stream::SegmentRecord| -> Vec<u32> {
            rec.selection
                .per_notice
                .iter()
                .filter(|n| n.escalations > 0)
                .map(|n| n.anomaly)
                .collect()
        };
        assert_eq!(asked(&always), asked(&logged), "seed {seed}");
        // Every anomaly asked about was logged at its ready step, and none before it.
        let log = log.borrow();
        for n in asked(&logged) {
            let l = &log[&n];
            assert!(
                l.now.0 >= l.view.noticed_at.0 + 4 * S,
                "seed {seed} anomaly {n}"
            );
        }
        assert!(!log.is_empty());
    }
}
