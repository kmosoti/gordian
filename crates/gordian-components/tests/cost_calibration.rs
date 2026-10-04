//! Declared cost against measured cost.
//!
//! Wall-clock dependent, so every test here is `#[ignore]`. Run, after the benchmarks, with
//!
//! ```text
//! taskset -c 2 cargo bench -p gordian-components
//! taskset -c 2 cargo test -p gordian-components --release --test cost_calibration -- --ignored --test-threads=1
//! ```
//!
//! The acceptance test compares, for every benchmark id, the sum of the components' declared
//! `Resource::Compute` over the pool of windows that benchmark ran on against criterion's median
//! for one pass over that pool (`target/criterion/<group>/<id>/new/estimates.json`). It passes
//! when every declared cost is within 25% of the median. The table of ratios is written to
//! `target/cost-calibration.md`, and the pool features used to fit the constants to
//! `target/cost-calibration.json` (see `calibrate.py`).
//!
//! A second test times each episode class separately with a plain loop and reports how far the
//! class-averaged declared cost is from each class. It asserts nothing: a size-only declared cost
//! cannot be right for every class, and the spread is the figure to read.

#[path = "../benches/workload/mod.rs"]
mod workload;

use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, PriorRecordLookup, RuleHeuristic, WorkingState,
};
use gordian_core::Resource;
use std::fmt::Write as _;
use std::path::PathBuf;

const TOLERANCE: f64 = 0.25;

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

fn median_ns(group: &str, id: usize) -> f64 {
    let path = target_dir()
        .join("criterion")
        .join(group)
        .join(id.to_string())
        .join("new/estimates.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "no criterion estimate at {}: {e}. Run `taskset -c 2 cargo bench -p gordian-components` first",
            path.display()
        )
    });
    let json: serde_json::Value = serde_json::from_str(&text).expect("estimates.json parses");
    json["median"]["point_estimate"]
        .as_f64()
        .expect("median point estimate")
}

fn declared(component: &dyn Component, resource: Resource, w: &WorkingState) -> u64 {
    component
        .declared_cost(w)
        .iter()
        .filter(|c| c.resource == resource)
        .map(|c| c.amount)
        .sum()
}

fn component(name: &str) -> Box<dyn Component> {
    match name {
        "heuristic" => Box::new(RuleHeuristic::new()),
        "estimator" => Box::new(CountEstimator::new()),
        "memory" => Box::new(PriorRecordLookup::new()),
        "verifier" => Box::new(ConsistencyVerifier::new()),
        other => panic!("unknown component {other}"),
    }
}

/// One benchmark id: where criterion put it and the pool it ran on.
struct Case {
    group: String,
    id: usize,
    component: &'static str,
    pool: Vec<WorkingState>,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for name in ["heuristic", "estimator", "memory", "verifier"] {
        for n in workload::SIZES {
            out.push(Case {
                group: format!("{name}_n"),
                id: n,
                component: name,
                pool: workload::windows(n, None),
            });
        }
    }
    for name in ["heuristic", "estimator", "memory", "verifier"] {
        for s in workload::SERVICE_COUNTS {
            out.push(Case {
                group: format!("{name}_s"),
                id: s as usize,
                component: name,
                pool: workload::windows(workload::SERVICE_SWEEP_N, Some(s)),
            });
        }
    }
    for name in ["heuristic", "estimator", "memory", "verifier"] {
        for n in workload::HELD_OUT_SIZES {
            out.push(Case {
                group: format!("{name}_h"),
                id: n,
                component: name,
                pool: workload::windows(n, None),
            });
        }
    }
    let base = workload::windows(workload::RECORD_SWEEP_N, None);
    for extra in workload::PAD_COUNTS {
        out.push(Case {
            group: "memory_r".to_string(),
            id: extra,
            component: "memory",
            pool: workload::with_padding(&base, extra),
        });
    }
    out
}

#[test]
#[ignore = "wall-clock: run after `cargo bench`, see the module documentation"]
fn declared_cost_is_within_25_percent_of_the_criterion_median() {
    let mut table = String::from(
        "| benchmark | windows with an entry | median (us) | declared (us) | declared / median |\n|---|---:|---:|---:|---:|\n",
    );
    let mut features = Vec::new();
    let mut failures = Vec::new();
    for case in cases() {
        let c = component(case.component);
        let median = median_ns(&case.group, case.id);
        let declared_ns: u64 = case
            .pool
            .iter()
            .map(|w| declared(c.as_ref(), Resource::Compute, w))
            .sum();
        let ratio = declared_ns as f64 / median;
        let emitted = case
            .pool
            .iter()
            .filter(|w| !component(case.component).run(w).entries.is_empty())
            .count();
        writeln!(
            table,
            "| {}/{} | {}/{} | {:.2} | {:.2} | {:.3} |",
            case.group,
            case.id,
            emitted,
            case.pool.len(),
            median / 1e3,
            declared_ns as f64 / 1e3,
            ratio
        )
        .unwrap();
        if (ratio - 1.0).abs() > TOLERANCE {
            failures.push(format!("{}/{}: ratio {ratio:.3}", case.group, case.id));
        }
        let sum_n: usize = case.pool.iter().map(|w| w.size()).sum();
        let sum_s: usize = case.pool.iter().map(|w| w.public.services.len()).sum();
        let sum_ns: usize = case
            .pool
            .iter()
            .map(|w| w.size() * w.public.services.len())
            .sum();
        let sum_r: usize = case.pool.iter().map(|w| w.public.prior_records.len()).sum();
        features.push(serde_json::json!({
            "emitted": emitted,
            "group": case.group, "id": case.id, "component": case.component,
            "median_ns": median, "declared_ns": declared_ns,
            "windows": case.pool.len(), "sum_n": sum_n, "sum_s": sum_s,
            "sum_ns": sum_ns, "sum_r": sum_r,
        }));
    }
    let dir = target_dir();
    std::fs::write(dir.join("cost-calibration.md"), &table).expect("write table");
    std::fs::write(
        dir.join("cost-calibration.json"),
        serde_json::to_string_pretty(&features).unwrap(),
    )
    .expect("write features");
    println!("{table}");
    assert!(
        failures.is_empty(),
        "declared cost is more than {:.0}% from the criterion median at: {failures:?}",
        TOLERANCE * 100.0
    );
}

#[test]
#[ignore = "wall-clock: reports a spread, asserts nothing"]
fn class_spread_at_window_size_256() {
    if cfg!(debug_assertions) {
        println!("class spread needs --release; skipped in a debug build");
        return;
    }
    let n = workload::SERVICE_SWEEP_N;
    let episodes = workload::episodes(None);
    let mut out = String::from(
        "| component | class | measured per run (us) | declared (us) | declared / measured |\n|---|---|---:|---:|---:|\n",
    );
    for name in ["heuristic", "estimator", "memory", "verifier"] {
        let mut c = component(name);
        for (i, class) in gordian_world::EpisodeClass::ALL.into_iter().enumerate() {
            let windows: Vec<WorkingState> = episodes[i * workload::SEEDS_PER_CLASS as usize
                ..(i + 1) * workload::SEEDS_PER_CLASS as usize]
                .iter()
                .map(|ep| workload::window(ep, n))
                .collect();
            // Median over 15 batches of 200 passes over the class's windows.
            let mut batches: Vec<f64> = (0..15)
                .map(|_| {
                    let start = std::time::Instant::now();
                    for _ in 0..200 {
                        for w in &windows {
                            std::hint::black_box(c.run(std::hint::black_box(w)));
                        }
                    }
                    start.elapsed().as_nanos() as f64 / (200.0 * windows.len() as f64)
                })
                .collect();
            batches.sort_by(f64::total_cmp);
            let measured = batches[batches.len() / 2];
            let declared_ns = windows
                .iter()
                .map(|w| declared(c.as_ref(), Resource::Compute, w))
                .sum::<u64>() as f64
                / windows.len() as f64;
            writeln!(
                out,
                "| {name} | {class:?} | {:.2} | {:.2} | {:.2} |",
                measured / 1e3,
                declared_ns / 1e3,
                declared_ns / measured
            )
            .unwrap();
        }
    }
    std::fs::write(target_dir().join("cost-class-spread.md"), &out).expect("write spread");
    println!("{out}");
}

/// Measured time of one run, best of five batches of 200 runs, in nanoseconds.
fn time_ns(c: &mut dyn Component, w: &WorkingState) -> f64 {
    (0..5)
        .map(|_| {
            let start = std::time::Instant::now();
            for _ in 0..200 {
                std::hint::black_box(c.run(std::hint::black_box(w)));
            }
            start.elapsed().as_nanos() as f64 / 200.0
        })
        .fold(f64::MAX, f64::min)
}

#[test]
#[ignore = "wall-clock: reports how far cost moves with content at a fixed size, asserts nothing"]
fn content_envelope_at_window_size_256() {
    use gordian_core::Instant;
    use gordian_world::{CounterName, EpisodeClass, EpisodeSpec, Observation, ServiceId, generate};

    if cfg!(debug_assertions) {
        println!("content envelope needs --release; skipped in a debug build");
        return;
    }
    let public = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous)).public_info();
    let dependent = public
        .services
        .iter()
        .find(|s| !s.depends_on.is_empty())
        .expect("a service with a dependency");
    let (dep, site) = (dependent.id, dependent.depends_on[0]);
    let counter = |service: ServiceId, name, value| Observation::Counter {
        service,
        name,
        value,
    };
    let build = |n: usize, f: &dyn Fn(usize) -> Observation| {
        let mut w = WorkingState::new(public.clone(), n);
        for i in 0..n {
            w.admit(Instant(i as u64 + 1), f(i));
        }
        w
    };
    let n = workload::SERVICE_SWEEP_N;
    // Counters below the alarm threshold: nothing the rules can discriminate on.
    let benign = build(n, &|_| counter(site, CounterName::Latency, 3));
    // The site's alarm first, then alarms alternating between the site and its dependent: every
    // observation informative, the anchor early (the shape the generator produces).
    let informative = build(n, &|i| match i {
        0 => counter(site, CounterName::ErrorRate, 99),
        i if i % 2 == 0 => counter(site, CounterName::Latency, 99),
        _ => counter(dep, CounterName::Latency, 99),
    });
    // Half the window is alarms at the site that are not `ErrorRate`, then the site's
    // `ErrorRate`, then alarms at the dependent: each dependent alarm makes the checker scan back
    // to the late anchor. The generator never produces this shape.
    let late_anchor = build(n, &|i| match i {
        i if i < n / 2 => counter(site, CounterName::Latency, 99),
        i if i == n / 2 => counter(site, CounterName::ErrorRate, 99),
        _ => counter(dep, CounterName::Latency, 99),
    });
    let mut out = String::from(
        "| component | declared (us) | all benign | all informative, anchor first | informative, anchor late |\n|---|---:|---:|---:|---:|\n",
    );
    for name in ["heuristic", "estimator", "memory", "verifier"] {
        let mut c = component(name);
        let declared_ns = declared(c.as_ref(), Resource::Compute, &benign) as f64;
        let ratios: Vec<String> = [&benign, &informative, &late_anchor]
            .into_iter()
            .map(|w| format!("{:.2}x", time_ns(c.as_mut(), w) / declared_ns))
            .collect();
        writeln!(
            out,
            "| {name} | {:.2} | {} | {} | {} |",
            declared_ns / 1e3,
            ratios[0],
            ratios[1],
            ratios[2]
        )
        .unwrap();
    }
    std::fs::write(target_dir().join("cost-content-envelope.md"), &out).expect("write envelope");
    println!("{out}");
}
