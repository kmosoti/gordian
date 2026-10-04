//! Criterion benchmarks behind the components' declared cost models.
//!
//! Three sweeps per component, each over the pool described in `workload`:
//!
//! - `<component>_n`: window size 16, 64, 256, 1024 (services drawn per episode, 4 to 12);
//! - `<component>_s`: 4, 8, 12 services at window size 256;
//! - `<component>_h`: window sizes 32, 128, 512, 2048, held out of the fit;
//! - `memory_r`: 0, 16, 128, 1024 never-matching records added to the generated ones, at window
//!   size 64.
//!
//! Run pinned to core 2 with `taskset -c 2 cargo bench -p gordian-components`. The calibration
//! test (`tests/cost_calibration.rs`) reads the medians from `target/criterion`.

mod workload;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, PriorRecordLookup, RuleHeuristic, WorkingState,
};
use std::hint::black_box;
use std::time::Duration;

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(60)
}

/// One component run on every window of the pool.
fn run_pool(component: &mut dyn Component, pool: &[WorkingState]) {
    for w in pool {
        black_box(component.run(black_box(w)));
    }
}

fn components() -> Vec<(&'static str, Box<dyn Component>)> {
    vec![
        ("heuristic", Box::new(RuleHeuristic::new())),
        ("estimator", Box::new(CountEstimator::new())),
        ("memory", Box::new(PriorRecordLookup::new())),
        ("verifier", Box::new(ConsistencyVerifier::new())),
    ]
}

fn bench_all(c: &mut Criterion) {
    for (name, mut component) in components() {
        let mut group = c.benchmark_group(format!("{name}_n"));
        for n in workload::SIZES {
            let pool = workload::windows(n, None);
            group.bench_with_input(BenchmarkId::from_parameter(n), &pool, |b, pool| {
                b.iter(|| run_pool(component.as_mut(), pool));
            });
        }
        group.finish();
    }
    for (name, mut component) in components() {
        let mut group = c.benchmark_group(format!("{name}_s"));
        for s in workload::SERVICE_COUNTS {
            let pool = workload::windows(workload::SERVICE_SWEEP_N, Some(s));
            group.bench_with_input(BenchmarkId::from_parameter(s), &pool, |b, pool| {
                b.iter(|| run_pool(component.as_mut(), pool));
            });
        }
        group.finish();
    }
    for (name, mut component) in components() {
        let mut group = c.benchmark_group(format!("{name}_h"));
        for n in workload::HELD_OUT_SIZES {
            let pool = workload::windows(n, None);
            group.bench_with_input(BenchmarkId::from_parameter(n), &pool, |b, pool| {
                b.iter(|| run_pool(component.as_mut(), pool));
            });
        }
        group.finish();
    }
    let mut group = c.benchmark_group("memory_r");
    let base = workload::windows(workload::RECORD_SWEEP_N, None);
    let mut lookup = PriorRecordLookup::new();
    for extra in workload::PAD_COUNTS {
        let pool = workload::with_padding(&base, extra);
        group.bench_with_input(BenchmarkId::from_parameter(extra), &pool, |b, pool| {
            b.iter(|| run_pool(&mut lookup, pool));
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench_all
}
criterion_main!(benches);
