//! Cost micro-benchmark of the dataflow engine (work item C1): measured time per batch of counted
//! operations at tables of 10, 100 and 1,000 rows, set beside the declared prices of
//! [`Prices::DECLARED`], as `gordian-medium`'s `tick` benchmark does for the medium.
//!
//! This file is a `[[bench]]` of `gordian-run` whose source lives with the code it prices
//! (`crates/gordian-run/Cargo.toml`). It uses only the engine, so it is held to the textual ban of
//! `scripts/check-no-oracle.sh` like every file under `arms/`; the same noticer measured in place,
//! on the public observations of real streams, is the ignored test `replay_costs` of
//! `tests/stream_dataflow.rs`.
//!
//! Workloads, each a batch of 64 operations of one kind in a steady state (each batch leaves the
//! table as it found it), at three table sizes:
//!
//! - `probe`: 64 point reads of a table of N rows. 64 probes.
//! - `write`: 32 rows put and the same 32 removed, in an unlogged table. 64 writes.
//! - `write_logged`: the same in a table that logs a delta for every change, the log drained after
//!   the batch. 64 writes.
//! - `scan`: eight range scans of 32 rows (the whole table for N = 10). 256 scans (80 for N = 10).
//! - `project`: 64 rows of a logged table replaced with an unchanged image, then
//!   [`project`] over their deltas into an unlogged table, which writes nothing. 64 writes,
//!   64 fires.
//! - `project_moving`: the same with an image that changes, so that each delta removes a row from
//!   the output and puts another. 192 writes, 64 fires.
//! - `mixed`: the headline, a batch with the proportions of the noticer's own work measured on real
//!   streams (see `MIX`): probes, writes through logged and unlogged tables, scans and fires.
//!
//! Differences between the isolating workloads give per-kind estimates (`scripts/c1_prices.py`):
//! probe from `probe`, write from `write` and `write_logged`, scan from `scan`, fire from
//! `project` less `write_logged`. The per-batch operation counts of each workload are printed to
//! stderr once, as `counts <id> <probes> <writes> <scans> <fires>`, read from the engine's own
//! counters.
//!
//! Run through the runner on cores 0-2:
//! `scripts/cgroup-run.sh --name c1-bench --cpus 0-2 --cpu-quota 300 --memory 2G --report <file> -- cargo bench -p gordian-run --bench dataflow`

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gordian_run::stream::arms::dataflow::engine::{OpCounts, Prices, Table, project};

const SIZES: [u32; 3] = [10, 100, 1_000];
const BATCH: u32 = 64;

/// The mix of the headline workload per batch: probes, logged puts and removals, unlogged puts and
/// removals, scanned rows, projected deltas. Set from the noticer's counted work on the tuning
/// streams (`c1-op-mix.csv`).
const MIX: Mix = Mix {
    probes: 20,
    logged_writes: 12,
    plain_writes: 8,
    scans: 24,
    fires: 12,
};

struct Mix {
    probes: u32,
    logged_writes: u32,
    plain_writes: u32,
    scans: u32,
    fires: u32,
}

type T = Table<(u32, u32), u64>;

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(60)
}

fn filled(n: u32, logged: bool) -> T {
    let mut t = if logged { T::logged() } else { T::new() };
    for i in 0..n {
        t.put((0, i), u64::from(i));
    }
    drop(t.drain());
    t
}

fn counts_of(tables: &[&T], fires: u64) -> OpCounts {
    tables.iter().fold(
        OpCounts {
            fires,
            ..OpCounts::default()
        },
        |acc, t| acc.plus(t.counts()),
    )
}

fn report(id: &str, c: OpCounts) {
    eprintln!(
        "counts {id} {} {} {} {} (declared {} ns)",
        c.probes,
        c.writes,
        c.scans,
        c.fires,
        c.modelled_ns(&Prices::DECLARED)
    );
}

fn probe_batch(t: &T, n: u32) {
    for i in 0..BATCH {
        black_box(t.get(&(0, i.wrapping_mul(7_919) % n)));
    }
}

fn write_batch(t: &mut T) {
    for i in 0..BATCH / 2 {
        t.put((1, i), u64::from(i));
    }
    for i in 0..BATCH / 2 {
        t.del(&(1, i));
    }
}

fn scan_batch(t: &T, n: u32, scans: u32) {
    let room = n.saturating_sub(32) + 1;
    for j in 0..scans {
        let start = (0, j.wrapping_mul(13) % room);
        for row in t.range(start..).take(32) {
            black_box(row);
        }
    }
}

fn project_batch(src: &mut T, out: &mut T, n: u32, moving: bool, round: &mut u64) -> u64 {
    *round += 1;
    for i in 0..BATCH {
        src.put((0, i % n), *round);
    }
    let deltas = src.drain();
    if moving {
        project(&deltas, out, |k, v| Some(((k.0, *v as u32), *v)))
    } else {
        project(&deltas, out, |k, _| Some((*k, 0)))
    }
}

fn mixed_batch(
    logged: &mut T,
    plain: &mut T,
    out: &mut T,
    probe_table: &T,
    n: u32,
    round: &mut u64,
) -> u64 {
    *round += 1;
    for i in 0..MIX.probes {
        black_box(probe_table.get(&(0, i.wrapping_mul(7_919) % n)));
    }
    for i in 0..MIX.logged_writes / 2 {
        logged.put((1, i), *round);
    }
    for i in 0..MIX.logged_writes / 2 {
        logged.del(&(1, i));
    }
    for i in 0..MIX.plain_writes / 2 {
        plain.put((1, i), *round);
    }
    for i in 0..MIX.plain_writes / 2 {
        plain.del(&(1, i));
    }
    scan_batch(probe_table, n, MIX.scans.div_ceil(32).max(1));
    let deltas = logged.drain();
    let fired = project(&deltas, out, |k, _| Some((*k, 0)));
    fired.min(u64::from(MIX.fires))
}

fn bench(c: &mut Criterion) {
    for n in SIZES {
        let mut group = c.benchmark_group("probe");
        let t = filled(n, false);
        let before = t.counts();
        probe_batch(&t, n);
        report(&format!("probe/{n}"), t.counts().since(before));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| probe_batch(&t, n));
        });
        group.finish();

        for (name, logged) in [("write", false), ("write_logged", true)] {
            let mut group = c.benchmark_group(name);
            let mut t = filled(n, logged);
            let before = t.counts();
            write_batch(&mut t);
            drop(t.drain());
            report(&format!("{name}/{n}"), t.counts().since(before));
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| {
                    write_batch(&mut t);
                    black_box(t.drain());
                });
            });
            group.finish();
        }

        let mut group = c.benchmark_group("scan");
        let t = filled(n, false);
        let before = t.counts();
        scan_batch(&t, n, 8);
        report(&format!("scan/{n}"), t.counts().since(before));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| scan_batch(&t, n, 8));
        });
        group.finish();

        for (name, moving) in [("project", false), ("project_moving", true)] {
            let mut group = c.benchmark_group(name);
            let mut src = filled(n, true);
            let mut out = filled(n, false);
            let mut round = 0u64;
            let before = counts_of(&[&src, &out], 0);
            let fired = project_batch(&mut src, &mut out, n, moving, &mut round);
            report(
                &format!("{name}/{n}"),
                counts_of(&[&src, &out], fired).since(before),
            );
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                b.iter(|| black_box(project_batch(&mut src, &mut out, n, moving, &mut round)));
            });
            group.finish();
        }

        let mut group = c.benchmark_group("mixed");
        let mut logged = filled(n, true);
        let mut plain = filled(n, false);
        let mut out = filled(n, false);
        let probe_table = filled(n, false);
        let mut round = 0u64;
        let before = counts_of(&[&logged, &plain, &out, &probe_table], 0);
        let fired = mixed_batch(
            &mut logged,
            &mut plain,
            &mut out,
            &probe_table,
            n,
            &mut round,
        );
        report(
            &format!("mixed/{n}"),
            counts_of(&[&logged, &plain, &out, &probe_table], fired).since(before),
        );
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                black_box(mixed_batch(
                    &mut logged,
                    &mut plain,
                    &mut out,
                    &probe_table,
                    n,
                    &mut round,
                ))
            });
        });
        group.finish();
    }
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench
}
criterion_main!(benches);
