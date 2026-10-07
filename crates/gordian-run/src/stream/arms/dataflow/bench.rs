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
//! - `replace_logged`: 64 rows of a logged table replaced, the log drained. 64 writes.
//! - `dispatch`: [`project`] over 64 prebuilt deltas whose image is always nothing: the handling of a
//!   delta and nothing else. 64 fires.
//! - `scan`: eight range scans of 32 rows (the whole table for N = 10). 256 scans (80 for N = 10).
//! - `project`: 64 rows of a logged table replaced with an unchanged image, then
//!   [`project`] over their deltas into an unlogged table, which writes nothing. 64 writes,
//!   64 fires.
//! - `project_moving`: the same with an image that changes, so that each delta removes a row from
//!   the output and puts another. 192 writes, 64 fires.
//! - `mixed`: the headline, a batch with the proportions of the noticer's own work measured on real
//!   streams (see `MIX`): 12 probes, 8 writes of a logged table and 4 of an unlogged one, 33 scanned
//!   rows, and the 8 deltas handled by [`project`] (8 fires).
//!
//! Five more workloads run the whole program ([`Program`]: the rules and the relations, without the
//! seam adapter) over a generated 60 s scenario of public observations in the rung's call order, with
//! the profile's mix of rules reached (`program/<profile>`): `background` (abnormal singles and
//! benign readings only), `bursts` (a site and its dependents, and a stray before a burst), `ramps`
//! (one counter rising with strays and dips), `splits` (a burst, a silence, a burst elsewhere) and
//! `mixed` (all of them). They price what the isolating workloads cannot: the allocation, cloning and
//! rule arithmetic around the table operations. Their counts are the program's own. `noticer/<profile>`
//! is the same replay through [`DataflowNoticer`], the seam adapter included (the `Tracked` view the
//! rung reads), with the same counts: the difference from `program/<profile>` is what the adapter
//! costs and the modelled cost does not price.
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

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use gordian_core::Instant;
use gordian_run::stream::arms::dataflow::engine::{OpCounts, Prices, Table, project};
use gordian_run::stream::arms::dataflow::program::Program;
use gordian_run::stream::arms::dataflow::{DataflowNoticer, DataflowSpec};
use gordian_run::stream::arms::noticer::{BaseSpec, Noticer};
use gordian_run::stream::arms::noticer_ramp::RampSpec;
use gordian_run::stream::arms::noticer_reanchor::Isolation;
use gordian_run::stream::arms::noticer_split::SplitSpec;
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_stream::ObsId;
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{CounterName, Observation, ResourceKind, Service, ServiceId, Severity};

const SIZES: [u32; 3] = [10, 100, 1_000];
const BATCH: u32 = 64;
const MS: u64 = 1_000_000;
const STEP_NS: u64 = 500 * MS;
const SCENARIO_MS: u64 = 60_000;
const PROFILES: [&str; 5] = ["background", "bursts", "ramps", "splits", "mixed"];

/// The mix of the headline workload per batch: probes, writes that replace a row of a logged table
/// (each one a delta the operators handle, one fire), writes through an unlogged table, scanned
/// rows. The proportions of the noticer's own counted work on the tuning streams
/// (`c1-op-mix.csv`: per stream about 36 thousand probes, 33 thousand writes, 96 thousand scanned
/// rows, 23 thousand fires), at 64 operations to the batch.
const MIX: Mix = Mix {
    probes: 12,
    logged_writes: 8,
    plain_writes: 4,
    scans: 33,
};

struct Mix {
    probes: u32,
    logged_writes: u32,
    plain_writes: u32,
    scans: u32,
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

fn replace_batch(t: &mut T, n: u32, round: &mut u64) {
    *round += 1;
    for i in 0..BATCH {
        t.put((0, i % n), *round);
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
    for i in 0..MIX.logged_writes {
        logged.put((0, i % n), *round);
    }
    for i in 0..MIX.plain_writes / 2 {
        plain.put((1, i), *round);
    }
    for i in 0..MIX.plain_writes / 2 {
        plain.del(&(1, i));
    }
    let room = n.saturating_sub(MIX.scans) + 1;
    let start = (0, (*round as u32).wrapping_mul(13) % room);
    for row in probe_table.range(start..).take(MIX.scans as usize) {
        black_box(row);
    }
    project(&logged.drain(), out, |k, _| Some((*k, 0)))
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

        let mut group = c.benchmark_group("replace_logged");
        let mut t = filled(n, true);
        let mut round = 0u64;
        let before = t.counts();
        replace_batch(&mut t, n, &mut round);
        drop(t.drain());
        report(&format!("replace_logged/{n}"), t.counts().since(before));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                replace_batch(&mut t, n, &mut round);
                black_box(t.drain());
            });
        });
        group.finish();

        let mut group = c.benchmark_group("dispatch");
        let mut src = filled(n, true);
        let mut out = filled(n, false);
        replace_batch(&mut src, n, &mut 0);
        let deltas = src.drain();
        let fired = project(&deltas, &mut out, |_, _| None);
        report(
            &format!("dispatch/{n}"),
            OpCounts {
                fires: fired,
                ..OpCounts::default()
            },
        );
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(project(&deltas, &mut out, |_, _| None)));
        });
        group.finish();

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

// ---- the program over generated scenarios --------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// Ten services, each depending on the two before it.
fn services() -> Vec<Service> {
    (0..10u32)
        .map(|i| Service {
            id: ServiceId(i),
            depends_on: (0..i).rev().take(2).rev().map(ServiceId).collect(),
            resource: ResourceKind::Cpu,
            config_hash: u64::from(i),
            unreliable_health: false,
        })
        .collect()
}

fn counter(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

fn message(service: u32) -> Observation {
    Observation::Message {
        service: ServiceId(service),
        text_id: SignalText::OutOfResource.text_id(),
        severity: Severity::High,
    }
}

type Events = Vec<(u64, Observation)>;

fn counter_name(r: &mut Rng) -> CounterName {
    CounterName::ALL[r.below(CounterName::ALL.len() as u64) as usize]
}

fn burst(r: &mut Rng, evs: &mut Events, t0: u64, sites: &[u32], k: u64) {
    for _ in 0..k {
        let s = sites[r.below(sites.len() as u64) as usize];
        let obs = if r.below(3) == 0 {
            message(s)
        } else {
            counter(s, counter_name(r), HIGH + r.below(60))
        };
        evs.push((t0 + r.below(380) * MS, obs));
    }
}

/// A scenario of the profile `profile`, in time order, with ids from zero.
fn scenario(profile: &str, services: &[Service]) -> Vec<Held> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let n = services.len() as u64;
    let mut evs: Events = Vec::new();
    let (bursts, ramps, splits, background) = match profile {
        "background" => (0, 0, 0, 160),
        "bursts" => (14, 0, 0, 30),
        "ramps" => (0, 8, 0, 30),
        "splits" => (0, 0, 8, 30),
        _ => (8, 5, 5, 80),
    };
    for _ in 0..bursts {
        let t0 = (1_000 + rng.below(SCENARIO_MS - 8_000)) * MS;
        let site = rng.below(n - 1) as u32;
        let k = 3 + rng.below(4);
        burst(&mut rng, &mut evs, t0, &[site, site + 1], k);
        if rng.below(2) == 0 {
            let stray = t0 + (3_000 + rng.below(3_000)) * MS;
            let name = counter_name(&mut rng);
            evs.push((stray, counter(site, name, HIGH + 5)));
            burst(&mut rng, &mut evs, stray + 300 * MS, &[site, site + 1], 3);
        }
    }
    for _ in 0..ramps {
        let t0 = (1_000 + rng.below(SCENARIO_MS - 20_000)) * MS;
        let site = rng.below(n) as u32;
        let name = counter_name(&mut rng);
        let (mut v, mut t) = (rng.below(30), t0);
        for _ in 0..(8 + rng.below(8)) {
            evs.push((t, counter(site, name, v)));
            if rng.below(6) == 0 {
                evs.push((t + rng.below(300) * MS, counter(site, name, rng.below(90))));
            }
            v = if rng.below(5) == 0 {
                v.saturating_sub(rng.below(6))
            } else {
                v + rng.below(13)
            };
            t += (600 + rng.below(1_000)) * MS;
        }
    }
    for _ in 0..splits {
        let t0 = (1_000 + rng.below(SCENARIO_MS - 20_000)) * MS;
        let site = rng.below(n - 1) as u32;
        burst(&mut rng, &mut evs, t0, &[site], 4);
        let later = t0 + (4_000 + rng.below(6_000)) * MS;
        burst(
            &mut rng,
            &mut evs,
            later,
            &[site + 1, (site + 2) % n as u32],
            4,
        );
    }
    for _ in 0..background {
        let v = if rng.below(8) == 0 {
            HIGH + rng.below(40)
        } else {
            rng.below(45)
        };
        let name = counter_name(&mut rng);
        let at = rng.below(SCENARIO_MS) * MS;
        evs.push((at, counter(rng.below(n) as u32, name, v)));
    }
    evs.sort_by_key(|(t, _)| *t);
    evs.into_iter()
        .enumerate()
        .map(|(i, (t, obs))| Held {
            id: ObsId(i as u32),
            at: Instant(t),
            abnormal: is_abnormal(&obs, services),
            obs,
        })
        .collect()
}

/// One step of the rung's call order: the abnormal observations delivered, the store, the instant.
struct Step {
    now: Instant,
    abnormal: Vec<Held>,
    store: Store,
}

fn steps_of(events: &[Held]) -> Vec<Step> {
    let mut out = Vec::new();
    let (mut next, mut now) = (0, 0u64);
    while now < SCENARIO_MS * MS {
        let from = next;
        while next < events.len() && events[next].at.0 <= now {
            next += 1;
        }
        let window: Vec<Held> = events[..next]
            .iter()
            .filter(|h| h.at.0 + 10_000 * MS >= now)
            .cloned()
            .collect();
        out.push(Step {
            now: Instant(now),
            abnormal: events[from..next]
                .iter()
                .filter(|h| h.abnormal)
                .cloned()
                .collect(),
            store: Store::with(window),
        });
        now += STEP_NS;
    }
    out
}

fn spec() -> DataflowSpec {
    DataflowSpec {
        base: BaseSpec::Reanchor {
            notice_z: Some(2.0),
            gap_ns: 20 * MS,
            min_burst: 2,
            isolation: Isolation::Site,
        },
        ramp: Some(RampSpec {
            gap_ns: 1_600 * MS,
            max_step: 10,
            max_drop: 4,
            min_readings: 5,
            min_rise: 15,
            follow: None,
        }),
        split: Some(SplitSpec {
            gap_ns: 3_000 * MS,
            min_burst: 3,
        }),
        billed: true,
    }
}

fn replay(p: &mut Program, steps: &[Step]) -> usize {
    let mut notices = 0;
    for s in steps {
        for h in &s.abnormal {
            black_box(p.observe(h));
        }
        notices += p.notice(s.now, &s.store).len();
        p.refresh(s.now);
        for id in p.retirable(s.now) {
            p.retire(id);
        }
    }
    notices
}

fn replay_noticer(n: &mut DataflowNoticer, steps: &[Step]) -> usize {
    let mut notices = 0;
    for s in steps {
        for h in &s.abnormal {
            black_box(n.observe(h));
        }
        notices += n.notice(s.now, &s.store).len();
        n.refresh(s.now);
        for id in n.retirable(s.now) {
            n.retire(id);
        }
    }
    notices
}

fn bench_noticer(c: &mut Criterion) {
    let services = services();
    let cfg = RungConfig::default();
    let mut group = c.benchmark_group("noticer");
    for profile in PROFILES {
        let steps = steps_of(&scenario(profile, &services));
        let mut n = DataflowNoticer::new(spec(), cfg.clone(), &services);
        let notices = replay_noticer(&mut n, &steps);
        report(&format!("noticer/{profile}"), n.counts());
        eprintln!("noticer/{profile} notices {notices}");
        group.bench_function(BenchmarkId::from_parameter(profile), |b| {
            b.iter_batched(
                || DataflowNoticer::new(spec(), cfg.clone(), &services),
                |mut n| {
                    black_box(replay_noticer(&mut n, &steps));
                    n
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_program(c: &mut Criterion) {
    let services = services();
    let cfg = RungConfig::default();
    let mut group = c.benchmark_group("program");
    for profile in PROFILES {
        let steps = steps_of(&scenario(profile, &services));
        let mut p = Program::new(&spec(), &cfg, &services);
        let notices = replay(&mut p, &steps);
        report(&format!("program/{profile}"), p.counts());
        eprintln!("program/{profile} notices {notices}");
        group.bench_function(BenchmarkId::from_parameter(profile), |b| {
            b.iter_batched(
                || Program::new(&spec(), &cfg, &services),
                |mut p| {
                    black_box(replay(&mut p, &steps));
                    p
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench, bench_program, bench_noticer
}
criterion_main!(benches);
