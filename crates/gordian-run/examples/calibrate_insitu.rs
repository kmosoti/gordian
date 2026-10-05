//! Calibrates the counted operations in situ (work item A8b): against what each call costs inside
//! the harness, not in a tight loop.
//!
//! `calibrate_ops` times every component and the rule on fixed windows in a loop. That shows
//! whether the counters track the work (they do: `CALIBRATION.md`, section 9), but a call in an
//! episode runs between other work, on a window that has just changed, with its output kept, and
//! costs 1.5 to 2.7 times as much. An experiment pays the second price. This program plays real
//! episodes through the real harness, reads the harness's own timer entries (each carries the
//! nanoseconds of one component call or scheduling call and the operation counts of that call),
//! repeats the same episodes several times (`CALIBRATE_PASSES`, default 5), and keeps, for every
//! call, the *minimum* over the passes: the episodes are deterministic, so call number `k` of an
//! episode is the same call in every pass, and interference only ever adds time.
//!
//! One JSON line per call, in the schema `calibrate_ops.py` reads. Sets: `fit` (arms that select
//! varied subsets of the components, seeds 100 to 129), `heldout` (the same arms on seeds 200 to
//! 209) and `real` (`heuristic_only` and `all_components`, seeds 0 to 19: the arms and episodes of
//! the non-identical-arm check, which are in neither of the others). A scheduling step is the
//! harness's `select` and `decide` timer entries together, as `measured_sched_ns` sums them; the
//! final call is its own datum.
//!
//! ```text
//! cargo build --release --example calibrate_insitu
//! scripts/cgroup-run.sh --name calib-insitu --cpus 2 --cpu-quota 100 --memory 2G -- \
//!     target/release/examples/calibrate_insitu > target/calibrate-insitu.jsonl
//! python3 crates/gordian-run/calibrate_ops.py target/calibrate-insitu.jsonl [more runs ...]
//! ```

use gordian_components::ops;
use gordian_core::EntryKind;
use gordian_run::harness::{Limits, run_episode, standard_components};
use gordian_run::policy::decide::{DecideConfig, RULE_UNITS};
use gordian_run::policy::{self, Built, PolicySpec};
use gordian_world::{EpisodeClass, EpisodeSpec};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;

/// One call as the harness timed it.
struct Call {
    target: String,
    kind: &'static str,
    counts: Vec<u64>,
    ns: u64,
}

struct Arm {
    set: &'static str,
    name: &'static str,
    spec: PolicySpec,
    seeds: std::ops::Range<u64>,
}

fn arms() -> Vec<Arm> {
    let fixed = |names: &[&str], every| {
        PolicySpec::from_parts(
            "fixed_pipeline",
            Some(names.iter().map(|n| (*n).to_owned()).collect()),
            Some(every),
            None,
        )
        .expect("a valid pipeline")
    };
    let random = |p| {
        PolicySpec::from_parts("random_matched", None, None, Some(p)).expect("a valid probability")
    };
    let mut out = Vec::new();
    for (set, seeds) in [("fit", 100..130u64), ("heldout", 200..210u64)] {
        out.push(Arm {
            set,
            name: "random_50",
            spec: random(0.5),
            seeds: seeds.clone(),
        });
        out.push(Arm {
            set,
            name: "random_25",
            spec: random(0.25),
            seeds: seeds.clone(),
        });
        out.push(Arm {
            set,
            name: "pipeline_he",
            spec: fixed(&["heuristic", "estimator"], 1),
            seeds: seeds.clone(),
        });
        out.push(Arm {
            set,
            name: "pipeline_hvm_2",
            spec: fixed(&["heuristic", "verifier", "memory"], 2),
            seeds: seeds.clone(),
        });
    }
    for (name, spec) in [
        ("heuristic_only", PolicySpec::HeuristicOnly),
        ("all_components", PolicySpec::AllComponents),
    ] {
        out.push(Arm {
            set: "real",
            name,
            spec,
            seeds: 0..20,
        });
    }
    out
}

/// The timer entries of one episode, grouped into calls: a component call each, and a scheduling
/// step (`select` then `decide`) or a final call (a `decide` with no `select` before it).
fn calls_of(ledger: &gordian_core::Ledger) -> Vec<Call> {
    let mut calls = Vec::new();
    let mut pending: Option<(u64, Vec<u64>)> = None;
    for entry in ledger.iter() {
        if entry.kind != EntryKind::Measurement || entry.provenance.producer != "harness/timer" {
            continue;
        }
        let v: Value = serde_json::from_slice(&entry.payload).expect("a timer payload is JSON");
        let ns = v["ns"].as_u64().expect("ns");
        let counts: Vec<u64> = v["ops"]
            .as_array()
            .expect("ops")
            .iter()
            .map(|c| c.as_u64().expect("a count"))
            .collect();
        match v["what"].as_str().expect("what") {
            "component" => calls.push(Call {
                target: format!("component:{}", v["component"].as_u64().expect("component")),
                kind: "component",
                counts,
                ns,
            }),
            "select" => pending = Some((ns, counts)),
            "decide" => {
                let (kind, ns, counts) = match pending.take() {
                    Some((sns, scounts)) => (
                        "step",
                        sns + ns,
                        scounts
                            .iter()
                            .zip(counts.iter())
                            .map(|(a, b)| a + b)
                            .collect(),
                    ),
                    None => ("final", ns, counts),
                };
                calls.push(Call {
                    target: "rule".to_owned(),
                    kind,
                    counts,
                    ns,
                });
            }
            other => panic!("unknown timer entry {other}"),
        }
    }
    calls
}

fn main() {
    let passes: usize = std::env::var("CALIBRATE_PASSES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let limits = Limits::default();
    let decide = DecideConfig::default();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let component_names = ["heuristic", "estimator", "memory", "verifier"];

    for arm in arms() {
        for class in EpisodeClass::ALL {
            for seed in arm.seeds.clone() {
                // The same episode, `passes` times; the same call is the same index every time.
                let mut best: Option<Vec<Call>> = None;
                let mut worst: Vec<u64> = Vec::new();
                for _ in 0..passes {
                    let mut spec = EpisodeSpec::new(seed, class);
                    spec.budget = limits.world_budget();
                    let mut components = standard_components();
                    let record = match policy::build(&arm.spec, &decide, arm.name, seed) {
                        Built::Public(mut p) => {
                            run_episode(&spec, p.as_mut(), &mut components, &limits)
                        }
                        Built::Privileged(_) => panic!("a privileged arm is not calibrated"),
                    }
                    .expect("the episode runs");
                    let calls = calls_of(&record.ledger);
                    match best.as_mut() {
                        None => {
                            worst = calls.iter().map(|c| c.ns).collect();
                            best = Some(calls);
                        }
                        Some(b) => {
                            assert_eq!(b.len(), calls.len(), "the passes played different calls");
                            for (i, (x, y)) in b.iter_mut().zip(calls).enumerate() {
                                assert_eq!(x.counts, y.counts, "call {i} counted differently");
                                x.ns = x.ns.min(y.ns);
                                worst[i] = worst[i].max(y.ns);
                            }
                        }
                    }
                }
                for (call, worst_ns) in best.expect("a pass ran").into_iter().zip(worst) {
                    let mut named = BTreeMap::new();
                    let target = if let Some(id) = call.target.strip_prefix("component:") {
                        let id: u32 = id.parse().expect("component id");
                        let units = ops::units(gordian_core::ComponentId(id));
                        for (u, c) in units.iter().zip(&call.counts) {
                            named.insert(u.name, *c);
                        }
                        component_names[id as usize].to_owned()
                    } else {
                        for (u, c) in RULE_UNITS.iter().zip(&call.counts) {
                            named.insert(u.name, *c);
                        }
                        "rule".to_owned()
                    };
                    let n = named
                        .get("scanned")
                        .or_else(|| named.get("decoded_ranked"))
                        .copied()
                        .unwrap_or(0);
                    let line = json!({
                        "set": arm.set,
                        "source": "insitu",
                        "class": format!("{class:?}"),
                        "arm": arm.name,
                        "seed": seed,
                        "target": target,
                        "kind": call.kind,
                        "n": n,
                        "services": 0,
                        "probes": 0,
                        "counts": named,
                        "min_ns": call.ns as f64,
                        "max_ns": worst_ns as f64,
                        "passes": passes,
                    });
                    writeln!(out, "{line}").expect("write a line");
                }
            }
        }
        out.flush().expect("flush");
    }
}
