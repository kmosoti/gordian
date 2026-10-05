//! What the shared cheap rung noticed, stream by stream, against the incidents of the stream: the
//! instrument of work item R10's decomposition and threshold sweep
//! (`experiments/exploration/scripts/r10_notices.py`).
//!
//! ```text
//! cargo run --release -p gordian-run --example r10_notices -- --manifest FILE --first SEED --count N
//! ```
//!
//! It plays the shared cheap rung of the manifest (`rung`, with its `notice_z`) under a rule that
//! escalates nothing, over each seed's stream with the manifest's limits, and writes one JSON line
//! per seed: every incident (tier, hard family, onset, first observation and its instant, and
//! whether a hard incident's first moments already contradict the first world's rules) and every
//! anomaly the rung noticed (anchor, site, the instants of anchor and notice, the incident the
//! anchor belongs to, the incidents among its attached observations at the last step it was seen).
//!
//! The rung's notices do not depend on the escalation rule (R4's decomposition relied on this),
//! so these are the notices of every arm that shares the rung; the analysis checks it against the
//! ledgers of a diagnostic run (a call of `oracle_selection` is made `delay_ns` after the notice
//! of the anomaly it is about). The labels are read the way the evaluator reads them, so this
//! file is analysis tooling and no arm: it is never linked into an arm or the harness, and its
//! output is read by one analysis script.

use gordian_core::Instant;
use gordian_run::policy::PolicyId;
use gordian_run::stream::arms::rung::AnomalyView;
use gordian_run::stream::arms::{EscalationRule, RecognizeCtx, StreamArm, StreamPolicy};
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::run_segment;
use gordian_run::stream::score::family_name;
use gordian_stream::{Diagnosis, ObsId, StreamPublic, Tier, generate};
use gordian_stream_eval::truth_from_stream;
use serde_json::json;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;

#[derive(Default)]
struct Seen {
    /// The first view of each noticed anomaly (its notice instant never changes).
    views: BTreeMap<u32, AnomalyView>,
    /// The attached observations at the last step each anomaly was seen.
    attached: BTreeMap<u32, Vec<ObsId>>,
}

/// Escalates nothing and records what the rung noticed.
struct Probe {
    seen: Rc<RefCell<Seen>>,
}

impl EscalationRule for Probe {
    fn id(&self) -> PolicyId {
        PolicyId::new("never_escalate")
    }

    fn revises(&self) -> bool {
        true
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut seen = self.seen.borrow_mut();
        for v in views {
            seen.views.entry(v.id).or_insert_with(|| v.clone());
        }
        Vec::new()
    }

    fn recognize(&self, ctx: &RecognizeCtx<'_>) -> Option<Diagnosis> {
        self.seen.borrow_mut().attached.insert(
            ctx.view.id,
            ctx.attached.iter().map(|(_, o, _)| *o).collect(),
        );
        None
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: r10_notices --manifest FILE --first SEED --count N");
    ExitCode::from(2)
}

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Plain => "plain",
        Tier::Hard => "hard",
        Tier::Decoy => "decoy",
    }
}

fn main() -> ExitCode {
    let (mut manifest, mut first, mut count) = (None, None, None);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--manifest" => manifest = Some(value),
            "--first" => first = value.parse::<u64>().ok(),
            "--count" => count = value.parse::<u64>().ok(),
            _ => return usage(),
        }
    }
    let (Some(manifest), Some(first), Some(count)) = (manifest, first, count) else {
        return usage();
    };
    let manifest: StreamManifest = match std::fs::read_to_string(&manifest)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("r10_notices: {manifest}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for seed in first..first + count {
        let params = manifest.params_for(seed);
        let stream = generate(&params);
        let truth = truth_from_stream(&stream);
        let events = stream.events().to_vec();
        let seen = Rc::new(RefCell::new(Seen::default()));
        let handle = Rc::clone(&seen);
        let rung = manifest.rung.clone();
        let record = run_segment(
            &params,
            &|public: &StreamPublic| -> Box<dyn StreamPolicy> {
                Box::new(StreamArm::with(
                    Probe {
                        seen: Rc::clone(&handle),
                    },
                    public,
                    rung.clone(),
                ))
            },
            &manifest.limits,
            &manifest.exchange,
        )
        .expect("a segment plays");
        assert_eq!(record.observations as usize, events.len());
        let incidents: Vec<_> = truth
            .incidents
            .iter()
            .map(|i| {
                let first_obs = i.observations.first().copied();
                json!({
                    "id": i.id,
                    "tier": tier_name(i.tier),
                    "family": i.shape.hard_kind.map_or("", family_name),
                    "critical": i.critical,
                    "onset_ns": i.onset_ns,
                    "deadline_ns": i.deadline_ns,
                    "contradicts_early": i.shape.contradicts_early,
                    "recurrence": i.recurrence_of.is_some(),
                    "first_obs": first_obs.map(|o| o.0),
                    "first_at_ns": first_obs.map(|o| events[o.0 as usize].0.0),
                    "observations": i.observations.len(),
                })
            })
            .collect();
        let seen = seen.borrow();
        let notices: Vec<_> = seen
            .views
            .values()
            .map(|v| {
                let owner = truth.incident_of(v.anchor);
                let attached: BTreeSet<u32> = seen
                    .attached
                    .get(&v.id)
                    .into_iter()
                    .flatten()
                    .filter_map(|o| truth.incident_of(*o))
                    .collect();
                json!({
                    "anomaly": v.id,
                    "anchor": v.anchor.0,
                    "site": v.site.0,
                    "anchor_at_ns": v.anchor_at.0,
                    "noticed_at_ns": v.noticed_at.0,
                    "anchor_incident": owner,
                    "attached_incidents": attached,
                    "attached": seen.attached.get(&v.id).map_or(0, Vec::len),
                })
            })
            .collect();
        let line = json!({
            "seed": seed,
            "observations": events.len(),
            "anomalies_noticed": record.anomalies_noticed,
            "incidents": incidents,
            "notices": notices,
        });
        if writeln!(out, "{line}").is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
