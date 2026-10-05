//! How often does the public consistency checker contradict the evidence of a noticed anomaly,
//! by the tier of the incident the anomaly is about? An R5 diagnostic, on tuning streams.
//!
//! ```text
//! cargo run --release -p gordian-run --example r5_contradiction_probe -- FIRST_SEED COUNT [-v]
//! ```
//!
//! It plays the shared cheap rung with a rule that escalates nothing, asks the rung to keep the
//! checker's verdict current, and records for each noticed anomaly: the incident its anchor
//! belongs to (the hidden label, read the way the evaluator reads it, so this file is analysis
//! tooling and no arm), whether the checker ever found no hypothesis, how long after the anomaly
//! was noticed it first did, and whether it still would have if every attached observation that
//! belongs to another incident or to the background were removed. With `-v` it prints the
//! evidence of a few anomalies that were contradicted. Nothing here feeds an arm.

use gordian_core::Instant;
use gordian_run::policy::PolicyId;
use gordian_run::stream::arms::rung::{AnomalyView, RungConfig};
use gordian_run::stream::arms::{EscalationRule, RecognizeCtx, StreamArm, StreamPolicy};
use gordian_run::stream::manifest::{Exchange, StreamLimits};
use gordian_run::stream::run_segment;
use gordian_run::stream::score::family_name;
use gordian_stream::{Diagnosis, ObsId, StreamParams, StreamPublic, Tier, generate};
use gordian_stream_eval::truth_from_stream;
use gordian_world::Observation;
use gordian_world::physics::consistent_hypotheses;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

type Attached = Vec<(Instant, ObsId, gordian_world::ServiceId)>;

#[derive(Default)]
struct Seen {
    /// The last view of each anomaly and the first time it was contradicted.
    views: BTreeMap<u32, AnomalyView>,
    /// The attached observations at the last step each anomaly was seen.
    attached: BTreeMap<u32, Attached>,
    /// The evidence as the checker would see it at the first contradiction: attached up to then.
    at_first: BTreeMap<u32, Attached>,
}

struct Probe {
    seen: Rc<RefCell<Seen>>,
}

impl EscalationRule for Probe {
    fn id(&self) -> PolicyId {
        PolicyId::new("never_escalate")
    }

    fn monitors(&self) -> bool {
        true
    }

    fn revises(&self) -> bool {
        true
    }

    fn targets(&mut self, _now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut seen = self.seen.borrow_mut();
        for v in views {
            seen.views.insert(v.id, v.clone());
        }
        Vec::new()
    }

    fn recognize(&self, ctx: &RecognizeCtx<'_>) -> Option<Diagnosis> {
        let mut seen = self.seen.borrow_mut();
        seen.attached.insert(ctx.view.id, ctx.attached.to_vec());
        if ctx.view.contradicted_since.is_some() && !seen.at_first.contains_key(&ctx.view.id) {
            seen.at_first.insert(ctx.view.id, ctx.attached.to_vec());
        }
        None
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let first: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(10_000);
    let count: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(10);
    let verbose = args.next().is_some_and(|a| a == "-v");
    // (tier, family) -> [anomalies, contradicted at notice, ever contradicted, still contradicted
    // without the evidence of other incidents]
    let mut table: BTreeMap<(String, String), [u32; 4]> = BTreeMap::new();
    let mut first_after: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut shown = 0;
    for seed in first..first + count {
        let mut p = StreamParams::new(seed);
        p.duration_ns = 600_000_000_000;
        let truth = truth_from_stream(&generate(&p));
        let limits = StreamLimits::matching(
            &p,
            gordian_run::stream::manifest::DEFAULT_COMPUTE_NS,
            gordian_run::stream::manifest::DEFAULT_STEP_NS,
        );
        let seen = Rc::new(RefCell::new(Seen::default()));
        let handle = Rc::clone(&seen);
        let record = run_segment(
            &p,
            &|public: &StreamPublic| -> Box<dyn StreamPolicy> {
                Box::new(StreamArm::with(
                    Probe {
                        seen: Rc::clone(&handle),
                    },
                    public,
                    RungConfig::default(),
                ))
            },
            &limits,
            &Exchange::default(),
        )
        .expect("a segment plays");
        let public = record.public.clone();
        let info = public.world_public_info();
        let obs_of = |id: ObsId| -> Observation { record.public_stream[id.0 as usize].1.clone() };
        let at_of = |id: ObsId| -> Instant { record.public_stream[id.0 as usize].0 };
        let seen = seen.borrow();
        for (id, view) in &seen.views {
            let (tier, family, incident) = match truth.incident_of(view.anchor) {
                None => ("background".to_owned(), String::new(), None),
                Some(i) => {
                    let inc = &truth.incidents[i as usize];
                    let tier = match inc.tier {
                        Tier::Plain => "plain",
                        Tier::Hard => "hard",
                        Tier::Decoy => "decoy",
                    };
                    (
                        tier.to_owned(),
                        inc.shape.hard_kind.map_or("", family_name).to_owned(),
                        Some(i),
                    )
                }
            };
            let row = table.entry((tier.clone(), family.clone())).or_default();
            row[0] += 1;
            let noticed = view.noticed_at;
            if let Some(since) = view.contradicted_since {
                row[2] += 1;
                if since == noticed {
                    row[1] += 1;
                }
                first_after
                    .entry(format!("{tier}/{family}"))
                    .or_default()
                    .push((since.0 - noticed.0) as f64 / 1e9);
                // Without the evidence that belongs to other incidents or to the background.
                let evidence = |keep_foreign: bool| -> Vec<(Instant, Observation)> {
                    let list = seen.attached.get(id).cloned().unwrap_or_default();
                    let anchor_at = list.first().map_or(Instant(0), |(t, _, _)| *t);
                    list.iter()
                        .filter(|(_, o, _)| keep_foreign || truth.incident_of(*o) == incident)
                        .map(|(t, o, _)| (Instant(t.0.saturating_sub(anchor_at.0)), obs_of(*o)))
                        .collect()
                };
                let own = evidence(false);
                if consistent_hypotheses(&info, &own).is_empty() {
                    row[3] += 1;
                } else if verbose && shown < 6 && tier == "plain" {
                    shown += 1;
                    println!(
                        "--- seed {seed} anomaly {id} plain, contradicted only with foreign evidence"
                    );
                    for (t, o, s) in seen.at_first.get(id).unwrap() {
                        println!(
                            "    {:>9.3}s service {:>2} obs {:>5} incident {:?}  {:?}",
                            at_of(*o).0 as f64 / 1e9,
                            s.0,
                            o.0,
                            truth.incident_of(*o),
                            obs_of(*o)
                        );
                        let _ = t;
                    }
                }
            }
        }
    }
    println!(
        "tier/family: anomalies, contradicted at notice, ever contradicted, own-evidence-only contradicted at end of life"
    );
    for ((tier, family), r) in &table {
        println!(
            "{tier:>10}/{family:<12} {:>5} {:>5} {:>5} {:>5}",
            r[0], r[1], r[2], r[3]
        );
    }
    for (k, v) in &first_after {
        let mut v = v.clone();
        v.sort_by(f64::total_cmp);
        println!(
            "{k:<24} first contradiction after notice (s): median {:.2}, p90 {:.2}, n {}",
            v[v.len() / 2],
            v[v.len() * 9 / 10],
            v.len()
        );
    }
}
