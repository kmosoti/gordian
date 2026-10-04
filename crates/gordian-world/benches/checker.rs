//! The optimized consistency checker against the reference (work item A5b).
//!
//! Two implementations, `reference` (`physics::consistent_hypotheses_reference`) and `optimized`
//! (`physics::consistent_hypotheses`), at evidence lengths n in {64, 256, 1024, 2048}, on three
//! shapes:
//!
//! - `late_anchor`: `n / 2` alarms at the site that are not `ErrorRate`, the site's `ErrorRate`,
//!   then `n / 2 - 1` alarms at a dependent. The shape that is quadratic in the reference. One
//!   call per iteration.
//! - `early_anchor`: the same entries with the site's `ErrorRate` first. The shape generated
//!   streams have. One call per iteration.
//! - `generated`: two episodes of each of the eleven classes (22 windows), each cut at `n`
//!   entries, or repeated with shifted instants when the stream is shorter. One iteration runs
//!   the checker once on every window, so the number is the cost of one call averaged over the
//!   class mix, times 22.
//!
//! Run through the runner on one core:
//! `scripts/cgroup-run.sh --name checker-bench --cpus 2 --cpu-quota 100 -- cargo bench -p gordian-world --bench checker`
//! (or `taskset -c 2 cargo bench ...`). The profile is `bench`, which inherits `release`.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use gordian_core::Instant;
use gordian_world::episode::PublicInfo;
use gordian_world::physics::{consistent_hypotheses, consistent_hypotheses_reference};
use gordian_world::{
    CounterName, Episode, EpisodeClass, EpisodeSpec, Observation, ServiceId, generate,
};
use std::hint::black_box;
use std::time::Duration;

const SIZES: [usize; 4] = [64, 256, 1024, 2048];
const WORKLOAD_SEED: u64 = 0x5EED_0000;

type Evidence = Vec<(Instant, Observation)>;

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(60)
}

/// The public graph the two anchor shapes run on: nine services, `ServiceId(1)` depends on
/// `ServiceId(0)`.
fn anchor_graph() -> (PublicInfo, ServiceId, ServiceId) {
    let public = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous)).public_info();
    let dependent = public
        .services
        .iter()
        .find(|s| !s.depends_on.is_empty())
        .expect("a service with a dependency");
    let (dep, site) = (dependent.id, dependent.depends_on[0]);
    (public, site, dep)
}

fn anchor_shape(n: usize, site: ServiceId, dep: ServiceId, anchor_at: usize) -> Evidence {
    let counter = |service, name| Observation::Counter {
        service,
        name,
        value: 99,
    };
    (0..n)
        .map(|i| {
            let o = if i == anchor_at {
                counter(site, CounterName::ErrorRate)
            } else if i < n / 2 {
                counter(site, CounterName::Latency)
            } else {
                counter(dep, CounterName::Latency)
            };
            (Instant(i as u64 + 1), o)
        })
        .collect()
}

/// `n` entries of `ep`'s stream, repeated with shifted instants if the stream is shorter.
fn window(ep: &Episode, n: usize) -> Evidence {
    let stream = ep.stream();
    let lap = ep.spec().horizon.0 + 1;
    (0..n)
        .map(|k| {
            let (t, o) = &stream[k % stream.len()];
            (Instant(t.0 + (k / stream.len()) as u64 * lap), o.clone())
        })
        .collect()
}

fn generated_pool(n: usize) -> Vec<(PublicInfo, Evidence)> {
    let mut out = Vec::new();
    for (i, class) in EpisodeClass::ALL.into_iter().enumerate() {
        for k in 0..2u64 {
            let ep = generate(&EpisodeSpec::new(
                WORKLOAD_SEED + (i as u64) * 16 + k,
                class,
            ));
            out.push((ep.public_info(), window(&ep, n)));
        }
    }
    out
}

fn bench_anchor_shape(c: &mut Criterion, group: &str, early: bool) {
    let (public, site, dep) = anchor_graph();
    let mut group = c.benchmark_group(group);
    for n in SIZES {
        let evidence = anchor_shape(n, site, dep, if early { 0 } else { n / 2 });
        assert_eq!(
            consistent_hypotheses(&public, &evidence),
            consistent_hypotheses_reference(&public, &evidence),
            "the two checkers disagree on the benchmark input"
        );
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("reference", n), &evidence, |b, ev| {
            b.iter(|| {
                black_box(consistent_hypotheses_reference(
                    black_box(&public),
                    black_box(ev),
                ))
            });
        });
        group.bench_with_input(BenchmarkId::new("optimized", n), &evidence, |b, ev| {
            b.iter(|| black_box(consistent_hypotheses(black_box(&public), black_box(ev))));
        });
    }
    group.finish();
}

fn bench_generated(c: &mut Criterion) {
    let mut group = c.benchmark_group("generated");
    for n in SIZES {
        let pool = generated_pool(n);
        for (public, evidence) in &pool {
            assert_eq!(
                consistent_hypotheses(public, evidence),
                consistent_hypotheses_reference(public, evidence),
                "the two checkers disagree on the benchmark input"
            );
        }
        group.throughput(Throughput::Elements((n * pool.len()) as u64));
        group.bench_with_input(BenchmarkId::new("reference", n), &pool, |b, pool| {
            b.iter(|| {
                for (public, evidence) in pool {
                    black_box(consistent_hypotheses_reference(
                        black_box(public),
                        black_box(evidence),
                    ));
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("optimized", n), &pool, |b, pool| {
            b.iter(|| {
                for (public, evidence) in pool {
                    black_box(consistent_hypotheses(
                        black_box(public),
                        black_box(evidence),
                    ));
                }
            });
        });
    }
    group.finish();
}

fn bench_all(c: &mut Criterion) {
    bench_anchor_shape(c, "late_anchor", false);
    bench_anchor_shape(c, "early_anchor", true);
    bench_generated(c);
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench_all
}
criterion_main!(benches);
