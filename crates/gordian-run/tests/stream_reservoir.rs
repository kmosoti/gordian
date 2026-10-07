//! The reservoir noticer (work item L2): the generator and the fixed weights, the reservoir step
//! against a dense reference, the readout against a batch ridge solution, the residual-to-notice
//! rule on hand-made observations, learning on a repeated pattern, determinism, the carry across
//! segments, the counted cost and the manifest.
//!
//! The byte-identity gate against R6's recorded hashes is a run, not a test (`scripts/l2_gate.py`).

// Stopped mid-unit (chief, 2026-10-07): lints in code left mid-edit, allowed with this note for the PI
// to clear when the unit resumes.
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::needless_range_loop)]
mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::SegmentRecord;
use gordian_run::stream::arms::noticer::{Notice, Noticer, NoticerSpec};
use gordian_run::stream::arms::reservoir::carry;
use gordian_run::stream::arms::reservoir::esn::{
    N_IN, N_OUT, Readout, Weights, fan_in_for, fill_regressor, regressor_len, softsign,
};
use gordian_run::stream::arms::reservoir::noticing::{
    ESN_OP_NS, RESERVOIR_COMPONENT, ReservoirNoticer,
};
use gordian_run::stream::arms::reservoir::params::ReservoirParams;
use gordian_run::stream::arms::reservoir::rng::Draws;
use gordian_run::stream::arms::reservoir::{RESERVOIR_ID, build};
use gordian_run::stream::arms::rung::{Held, RungConfig, Store, is_abnormal};
use gordian_stream::{ObsId, generate};
use gordian_world::{CounterName, Observation, ServiceId, Severity};
use stream_common::*;

const MS: u64 = 1_000_000;

fn services_of(seed: u64) -> Vec<gordian_world::Service> {
    public_of(&params(seed, 150)).services
}

fn counter(service: u32, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service: ServiceId(service),
        name,
        value,
    }
}

/// Hand-made observations: `(at in ms, observation)`, in order, with ids from zero and the public
/// rules' verdict.
fn script(services: &[gordian_world::Service], items: &[(u64, Observation)]) -> Vec<Held> {
    items
        .iter()
        .enumerate()
        .map(|(i, (ms, o))| Held {
            id: ObsId(i as u32),
            at: Instant(ms * MS),
            abnormal: is_abnormal(o, services),
            obs: o.clone(),
        })
        .collect()
}

/// Play `items` to `n` as the rung does (a step every 500 ms, abnormal observations observed on
/// delivery, the store holding everything delivered), up to and including the step at or after
/// `until_ms`. Returns the notices with the step's instant in ms.
fn drive(n: &mut ReservoirNoticer, items: &[Held], until_ms: u64) -> Vec<(u64, Notice)> {
    let (mut next, mut now) = (0, 0u64);
    let mut held: Vec<Held> = Vec::new();
    let mut out = Vec::new();
    loop {
        now += 500;
        while next < items.len() && items[next].at.0 <= now * MS {
            if items[next].abnormal {
                n.observe(&items[next]);
            }
            held.push(items[next].clone());
            next += 1;
        }
        for x in n.notice(Instant(now * MS), &Store::with(held.clone())) {
            out.push((now, x));
        }
        if now >= until_ms {
            return out;
        }
    }
}

fn off(threshold: f64, key: u64) -> ReservoirParams {
    ReservoirParams {
        learning: false,
        carry: false,
        threshold,
        ..ReservoirParams::standard(5, key)
    }
}

fn noticer(p: ReservoirParams, seed: u64) -> ReservoirNoticer {
    ReservoirNoticer::new(p, RungConfig::default(), &services_of(seed)).unwrap()
}

// ---- the generator and the weights

#[test]
fn the_generator_is_a_function_of_its_key_and_draws_in_range() {
    let a: Vec<u64> = {
        let mut d = Draws::keyed(&[1, 2]);
        (0..8).map(|_| d.next_u64()).collect()
    };
    let b: Vec<u64> = {
        let mut d = Draws::keyed(&[1, 2]);
        (0..8).map(|_| d.next_u64()).collect()
    };
    let c: Vec<u64> = {
        let mut d = Draws::keyed(&[2, 1]);
        (0..8).map(|_| d.next_u64()).collect()
    };
    assert_eq!(a, b);
    assert_ne!(a, c, "the order of the key's words matters");
    let mut d = Draws::keyed(&[9]);
    for _ in 0..2000 {
        let u = d.unit();
        assert!((0.0..1.0).contains(&u));
        let s = d.signed();
        assert!((-1.0..1.0).contains(&s));
        assert!(d.below(7) < 7);
    }
    // Both coin faces and every residue of `below(5)` occur.
    let mut seen = [false; 5];
    let mut heads = 0;
    for _ in 0..500 {
        seen[d.below(5) as usize] = true;
        heads += usize::from(d.coin());
    }
    assert!(seen.iter().all(|s| *s));
    assert!((150..350).contains(&heads), "{heads}");
}

#[test]
fn the_weights_are_a_function_of_the_seed_and_the_size_and_are_sparse() {
    let a = Weights::new(7, 32, 0.9, 1.0);
    let b = Weights::new(7, 32, 0.9, 1.0);
    let c = Weights::new(8, 32, 0.9, 1.0);
    assert_eq!(a.dense_recurrent(), b.dense_recurrent());
    assert_eq!(a.input_weights(), b.input_weights());
    assert_eq!(a.biases(), b.biases());
    assert_ne!(a.dense_recurrent(), c.dense_recurrent());
    assert_eq!(a.size(), 32);
    assert_eq!(a.fan_in(), fan_in_for(32));
    assert_eq!(fan_in_for(16), 3);
    assert_eq!(fan_in_for(32), 4);
    assert_eq!(fan_in_for(64), 8);
    let dense = a.dense_recurrent();
    let mag = 0.9 / (a.fan_in() as f64).sqrt();
    for i in 0..32 {
        let row = &dense[i * 32..(i + 1) * 32];
        let nz: Vec<f64> = row.iter().copied().filter(|v| *v != 0.0).collect();
        assert_eq!(nz.len(), a.fan_in(), "unit {i} has fan_in distinct sources");
        assert!(nz.iter().all(|v| (v.abs() - mag).abs() < 1e-15));
    }
    assert!(a.input_weights().iter().all(|w| (-1.0..1.0).contains(w)));
    assert!(a.biases().iter().all(|w| w.abs() <= 0.2));
    // The input scale scales the input weights and nothing else.
    let s = Weights::new(7, 32, 0.9, 2.0);
    assert_eq!(s.dense_recurrent(), a.dense_recurrent());
    for (x, y) in s.input_weights().iter().zip(a.input_weights()) {
        assert!((x - 2.0 * y).abs() < 1e-15);
    }
}

/// The growth rate of `||A^k v||`, from the log norm over a stretch of powers (a Gelfand
/// estimate of the spectral radius; test code, so `ln` is allowed).
fn realized_radius(a: &[f64], n: usize) -> f64 {
    let mut v = vec![1.0 / (n as f64).sqrt(); n];
    let (mut log_sum, mut at_100) = (0.0, 0.0);
    for k in 1..=400 {
        let mut w = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                w[i] += a[i * n + j] * v[j];
            }
        }
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm == 0.0 {
            return 0.0;
        }
        log_sum += norm.ln();
        v = w.iter().map(|x| x / norm).collect();
        if k == 100 {
            at_100 = log_sum;
        }
    }
    ((log_sum - at_100) / 300.0).exp()
}

#[test]
fn the_realized_spectral_radius_is_near_the_nominal() {
    // The nominal radius is the random-matrix value `sqrt(fan_in) * magnitude`; a small sparse
    // matrix's realized radius scatters around it. Averaged over seeds it is within a quarter.
    for (n, nominal) in [(32usize, 0.9), (64, 0.9), (64, 0.5), (32, 1.1)] {
        let mean: f64 = (0..24u64)
            .map(|seed| {
                let w = Weights::new(seed, n, nominal, 1.0);
                realized_radius(&w.dense_recurrent(), n)
            })
            .sum::<f64>()
            / 24.0;
        assert!(
            (mean / nominal - 1.0).abs() < 0.25,
            "n {n} nominal {nominal}: mean realized {mean}"
        );
    }
}

#[test]
fn the_activation_is_bounded_odd_and_increasing() {
    assert_eq!(softsign(0.0), 0.0);
    let mut last = -1.0;
    for i in -2000..=2000 {
        let x = f64::from(i) / 40.0;
        let y = softsign(x);
        assert!(y > -1.0 && y < 1.0);
        assert_eq!(softsign(-x), -y);
        assert!(y >= last);
        last = y;
    }
    assert_eq!(softsign(1.0), 0.5);
    assert_eq!(softsign(-3.0), -0.75);
}

// ---- the reservoir step against a dense reference

/// The leaky update by the book: dense matrices, every input entry multiplied.
fn reference_step(w: &Weights, x: &[f64], u: &[f64; N_IN], leak: f64) -> Vec<f64> {
    let n = w.size();
    let rec = w.dense_recurrent();
    (0..n)
        .map(|i| {
            let mut acc = w.biases()[i];
            for j in 0..n {
                acc += rec[i * n + j] * x[j];
            }
            for c in 0..N_IN {
                acc += w.input_weights()[i * N_IN + c] * u[c];
            }
            (1.0 - leak) * x[i] + leak * (acc / (1.0 + acc.abs()))
        })
        .collect()
}

#[test]
fn the_step_agrees_with_a_dense_reference_and_counts_what_it_ran() {
    let w = Weights::new(11, 32, 0.9, 1.0);
    let mut d = Draws::keyed(&[3]);
    let mut x = vec![0.0; 32];
    let mut y = vec![0.0; 32];
    let mut r = vec![0.0; 32];
    for _ in 0..300 {
        let mut u = [0.0; N_IN];
        for slot in u.iter_mut() {
            if d.below(4) == 0 {
                *slot = d.unit() * 2.0;
            }
        }
        let nz: Vec<usize> = (0..N_IN).filter(|&i| u[i] != 0.0).collect();
        let ops = w.step(&x, &u, &nz, 0.3, &mut y);
        assert_eq!(ops, (32 * (w.fan_in() + nz.len() + 7)) as u64);
        r = reference_step(&w, &x, &u, 0.3);
        for (a, b) in y.iter().zip(&r) {
            assert!((a - b).abs() < 1e-12, "{a} {b}");
        }
        x.copy_from_slice(&y);
        assert!(x.iter().all(|v| v.abs() < 1.0));
    }
    assert!(r.iter().any(|v| v.abs() > 1e-3), "the state is not dead");
}

// ---- the readout against a batch ridge solution

fn ridge_solution(zs: &[Vec<f64>], ys: &[[f64; N_OUT]], ridge: f64) -> Vec<f64> {
    // W = Y^T Z (Z^T Z + ridge I)^-1, solved by Gauss-Jordan on the d x d normal matrix.
    let d = zs[0].len();
    let mut a = vec![0.0; d * d];
    let mut b = vec![0.0; N_OUT * d];
    for (z, y) in zs.iter().zip(ys) {
        for i in 0..d {
            for j in 0..d {
                a[i * d + j] += z[i] * z[j];
            }
            for o in 0..N_OUT {
                b[o * d + i] += y[o] * z[i];
            }
        }
    }
    for i in 0..d {
        a[i * d + i] += ridge;
    }
    // Invert a.
    let mut inv = vec![0.0; d * d];
    for i in 0..d {
        inv[i * d + i] = 1.0;
    }
    for col in 0..d {
        let pivot = (col..d)
            .max_by(|&p, &q| a[p * d + col].abs().total_cmp(&a[q * d + col].abs()))
            .unwrap();
        for j in 0..d {
            a.swap(col * d + j, pivot * d + j);
            inv.swap(col * d + j, pivot * d + j);
        }
        let p = a[col * d + col];
        for j in 0..d {
            a[col * d + j] /= p;
            inv[col * d + j] /= p;
        }
        for r in 0..d {
            if r != col {
                let f = a[r * d + col];
                for j in 0..d {
                    a[r * d + j] -= f * a[col * d + j];
                    inv[r * d + j] -= f * inv[col * d + j];
                }
            }
        }
    }
    let mut w = vec![0.0; N_OUT * d];
    for o in 0..N_OUT {
        for j in 0..d {
            for i in 0..d {
                w[o * d + j] += b[o * d + i] * inv[i * d + j];
            }
        }
    }
    w
}

#[test]
fn recursive_least_squares_equals_the_batch_ridge_solution() {
    let d = 9;
    let ridge = 0.1;
    let mut rng = Draws::keyed(&[44]);
    let truth: Vec<f64> = (0..N_OUT * d).map(|_| rng.signed()).collect();
    let mut readout = Readout::new(d, ridge);
    let mut scratch = vec![0.0; 2 * d];
    let (mut zs, mut ys) = (Vec::new(), Vec::new());
    for t in 0..400 {
        let mut z: Vec<f64> = (0..d).map(|_| rng.signed()).collect();
        z[d - 1] = 1.0;
        let mut y = [0.0; N_OUT];
        for o in 0..N_OUT {
            for j in 0..d {
                y[o] += truth[o * d + j] * z[j];
            }
            // A little deterministic noise, so that the ridge matters.
            y[o] += 0.01 * rng.signed();
        }
        let mut pred = [0.0; N_OUT];
        let p_ops = readout.predict(&z, &mut pred);
        assert_eq!(p_ops, (N_OUT * d) as u64);
        let mut err = [0.0; N_OUT];
        for o in 0..N_OUT {
            err[o] = y[o] - pred[o];
        }
        let u_ops = readout.update(&z, &err, &mut scratch);
        assert_eq!(
            u_ops,
            (d * d + d + 1 + d + N_OUT * d + d * (d + 1) / 2) as u64
        );
        zs.push(z);
        ys.push(y);
        assert_eq!(readout.updates(), t + 1);
    }
    let batch = ridge_solution(&zs, &ys, ridge);
    for (a, b) in readout.weights().iter().zip(&batch) {
        assert!((a - b).abs() < 1e-8, "rls {a} batch {b}");
    }
    for (a, b) in readout.weights().iter().zip(&truth) {
        assert!((a - b).abs() < 0.02, "rls {a} truth {b}");
    }
    // P stays exactly symmetric, and its diagonal fell below the prior's.
    let p = readout.covariance();
    for i in 0..d {
        for j in 0..d {
            assert_eq!(p[i * d + j], p[j * d + i]);
        }
        assert!(p[i * d + i] < 1.0 / ridge);
        assert!(p[i * d + i] > 0.0);
    }
}

#[test]
fn the_initial_readout_predicts_nothing_and_an_idle_regressor_leaves_it_alone() {
    let d = regressor_len(8);
    let mut r = Readout::new(d, 0.1);
    assert!(r.weights().iter().all(|w| *w == 0.0));
    let x = vec![0.0; 8];
    let mut z = vec![0.0; d];
    fill_regressor(&mut z, &x, &[0.0; N_IN]);
    assert_eq!(z[d - 1], 1.0);
    let mut out = [1.0; N_OUT];
    r.predict(&z, &mut out);
    assert_eq!(out, [0.0; N_OUT]);
    // Only the constant is excited; an update with zero error changes no weight.
    let mut scratch = vec![0.0; 2 * d];
    r.update(&z, &[0.0; N_OUT], &mut scratch);
    assert!(r.weights().iter().all(|w| *w == 0.0));
}

// ---- the residual-to-notice rule on hand-made observations

#[test]
fn quiet_benign_readings_make_no_notice_and_no_hot_tick() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            (120, counter(0, CounterName::ErrorRate, 10)),
            (700, counter(1, CounterName::Latency, 20)),
            (1400, counter(2, CounterName::Saturation, 30)),
        ],
    );
    let mut n = noticer(off(0.5, 1), 1);
    let got = drive(&mut n, &items, 3000);
    assert!(got.is_empty());
    assert_eq!(n.stats().hot, 0);
    assert!(n.stats().ticks >= 25);
}

#[test]
fn with_the_readout_at_its_prior_the_residual_energy_is_the_inputs_own() {
    let services = services_of(1);
    // A lone abnormal error-rate reading of 70: one abnormal count (0.5) and the value (0.7).
    let items = script(&services, &[(250, counter(3, CounterName::ErrorRate, 70))]);
    let mut n = noticer(off(10.0, 1), 1);
    n.log_energies();
    drive(&mut n, &items, 1000);
    let at_tick_2 = n
        .energies()
        .iter()
        .find(|(t, node, _)| *t == 2 && *node == 3)
        .unwrap()
        .2;
    assert!((at_tick_2 - (0.25 + 0.49)).abs() < 1e-12, "{at_tick_2}");
    // Every other node-tick has no residual at all.
    for (t, node, e) in n.energies() {
        if !(*t == 2 && *node == 3) {
            assert_eq!(*e, 0.0, "tick {t} node {node}");
        }
    }
}

#[test]
fn a_lone_abnormal_reading_is_below_a_threshold_of_one_and_above_a_half() {
    let services = services_of(1);
    let items = script(&services, &[(250, counter(3, CounterName::ErrorRate, 70))]);
    let mut high = noticer(off(1.0, 1), 1);
    assert!(drive(&mut high, &items, 1500).is_empty());
    let mut low = noticer(off(0.5, 1), 1);
    let got = drive(&mut low, &items, 1500);
    assert_eq!(got.len(), 1);
    let (step, notice) = &got[0];
    assert_eq!(notice.anchor, ObsId(0));
    assert_eq!(notice.site, ServiceId(3));
    assert_eq!(notice.anchor_at, Instant(250 * MS));
    assert_eq!(notice.attached, vec![ObsId(0)]);
    // Made at the step at which tick 2 is complete, not at the anchor's instant.
    assert_eq!(*step, 500);
}

#[test]
fn a_two_kind_burst_in_one_tick_is_noticed_and_anchored_on_its_first_abnormal_observation() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            // a benign reading first in the tick, then the burst
            (1000, counter(3, CounterName::Saturation, 30)),
            (1010, counter(3, CounterName::ErrorRate, 90)),
            (1030, counter(3, CounterName::Latency, 80)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    n.log_energies();
    let got = drive(&mut n, &items, 2500);
    assert_eq!(got.len(), 1);
    let (_, notice) = &got[0];
    // The benign reading is the tick's first observation; the anchor is its first abnormal one.
    assert_eq!(notice.anchor, ObsId(1));
    assert_eq!(notice.site, ServiceId(3));
    assert_eq!(notice.attached, vec![ObsId(1), ObsId(2)]);
    let e = n
        .energies()
        .iter()
        .find(|(t, node, _)| *t == 10 && *node == 3)
        .unwrap()
        .2;
    // error rate 90: 0.25 + 0.81; latency 80: 0.25 + 0.64; saturation 30: 0.09.
    assert!(
        (e - (0.25 + 0.81 + 0.25 + 0.64 + 0.09)).abs() < 1e-12,
        "{e}"
    );
    assert_eq!(n.stats().runs, 1);
    assert_eq!(n.stats().notices, 1);
}

#[test]
fn a_run_of_hot_ticks_is_one_notice_anchored_at_its_earliest_tick() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            (1000, counter(2, CounterName::ErrorRate, 90)),
            (1020, counter(2, CounterName::Latency, 80)),
            (1110, counter(2, CounterName::ErrorRate, 95)),
            (1130, counter(2, CounterName::Latency, 85)),
            (1210, counter(2, CounterName::ErrorRate, 99)),
            (1230, counter(2, CounterName::Latency, 90)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    let got = drive(&mut n, &items, 3000);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].1.anchor, ObsId(0));
    assert_eq!(n.stats().hot, 3);
    assert_eq!(n.stats().runs, 1);
    // The later observations of the run are attached to the anomaly.
    let a = n.anomalies();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].attached.len(), 6);
}

#[test]
fn a_second_run_at_a_node_whose_anomaly_is_still_speaking_is_suppressed() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            (1000, counter(2, CounterName::ErrorRate, 90)),
            (1020, counter(2, CounterName::Latency, 80)),
            // a quiet tick, then another burst at the same node inside the anomaly's speaking window
            (2000, counter(2, CounterName::ErrorRate, 92)),
            (2020, counter(2, CounterName::Latency, 83)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    let got = drive(&mut n, &items, 3500);
    assert_eq!(got.len(), 1);
    assert_eq!(n.stats().runs, 2);
    assert_eq!(n.stats().notices, 1);
    // The second burst's observations were attached to the first anomaly on delivery, so its
    // anchor belonged to a live anomaly and no notice was made.
    assert_eq!(n.stats().suppressed, 1);
    assert_eq!(n.anomalies()[0].attached.len(), 4);
}

#[test]
fn bursts_at_two_unrelated_nodes_in_one_tick_are_two_notices_in_the_order_of_their_anchors() {
    let services = services_of(1);
    // Pick two services neither of which is in the other's dependents, from the public graph.
    let mut pair = None;
    'outer: for a in 0..services.len() {
        for b in (a + 1)..services.len() {
            let ra = gordian_world::graph::dependents_mask(&services, ServiceId(a as u32));
            let rb = gordian_world::graph::dependents_mask(&services, ServiceId(b as u32));
            if !ra[b] && !rb[a] {
                pair = Some((a as u32, b as u32));
                break 'outer;
            }
        }
    }
    let (a, b) = pair.expect("two unrelated services");
    let items = script(
        &services,
        &[
            (1010, counter(b, CounterName::ErrorRate, 90)),
            (1020, counter(a, CounterName::ErrorRate, 91)),
            (1030, counter(b, CounterName::Latency, 80)),
            (1040, counter(a, CounterName::Latency, 81)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    let got = drive(&mut n, &items, 2500);
    assert_eq!(got.len(), 2);
    // The order of the anchors' delivery, not the nodes' numbering: b began first.
    assert_eq!(got[0].1.site, ServiceId(b));
    assert_eq!(got[1].1.site, ServiceId(a));
    assert_eq!(got[0].1.anchor, ObsId(0));
    assert_eq!(got[1].1.anchor, ObsId(1));
}

#[test]
fn a_dependent_that_alarms_in_the_same_tick_joins_the_sites_anomaly() {
    let services = services_of(1);
    // A site and one of its dependents, from the public graph.
    let mut pair = None;
    for site in 0..services.len() {
        let region = gordian_world::graph::dependents_mask(&services, ServiceId(site as u32));
        if let Some(dep) = (0..services.len()).find(|d| region[*d] && *d != site) {
            pair = Some((site as u32, dep as u32));
            break;
        }
    }
    let (site, dep) = pair.expect("a site with a dependent");
    let items = script(
        &services,
        &[
            (1010, counter(site, CounterName::ErrorRate, 90)),
            (1020, counter(dep, CounterName::ErrorRate, 91)),
            (1030, counter(site, CounterName::Latency, 80)),
            (1040, counter(dep, CounterName::Latency, 81)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    let got = drive(&mut n, &items, 2500);
    assert_eq!(got.len(), 1, "one incident, one notice");
    assert_eq!(got[0].1.site, ServiceId(site));
    assert_eq!(got[0].1.anchor, ObsId(0));
    assert_eq!(n.stats().runs, 2);
    assert_eq!(n.stats().suppressed, 1);
    assert_eq!(n.anomalies()[0].attached.len(), 4);
}

#[test]
fn an_anomaly_retires_when_it_has_been_quiet_for_the_rungs_quiet_time() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            (1000, counter(4, CounterName::ErrorRate, 90)),
            (1020, counter(4, CounterName::Latency, 80)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    drive(&mut n, &items, 2500);
    let id = n.anomalies()[0].id;
    let quiet = RungConfig::default().quiet_ns;
    let last = n.anomalies()[0].last_abnormal_at;
    assert!(n.retirable(Instant(last.0 + quiet - 1)).is_empty());
    assert_eq!(n.retirable(Instant(last.0 + quiet)), vec![id]);
    n.retire(id);
    assert!(n.anomalies().is_empty());
    assert!(n.tracked(id).is_none());
}

#[test]
fn a_refused_charge_stops_the_noticer() {
    let services = services_of(1);
    let items = script(
        &services,
        &[
            (1000, counter(4, CounterName::ErrorRate, 90)),
            (1020, counter(4, CounterName::Latency, 80)),
        ],
    );
    let mut n = noticer(off(1.0, 1), 1);
    n.refused();
    assert!(drive(&mut n, &items, 2500).is_empty());
    assert_eq!(n.stats().ticks, 0);
}

#[test]
fn messages_and_snapshots_are_kinds_of_their_own() {
    let services = services_of(1);
    let msg = |service: u32| Observation::Message {
        service: ServiceId(service),
        text_id: gordian_world::physics::SignalText::ServiceDown.text_id(),
        severity: Severity::High,
    };
    let items = script(
        &services,
        &[
            (1010, msg(1)),
            (1030, counter(1, CounterName::ErrorRate, 90)),
        ],
    );
    assert!(items[0].abnormal && items[1].abnormal);
    let mut n = noticer(off(1.0, 1), 1);
    n.log_energies();
    drive(&mut n, &items, 2000);
    let e = n
        .energies()
        .iter()
        .find(|(t, node, _)| *t == 10 && *node == 1)
        .unwrap()
        .2;
    // a message: 0.25; an error-rate reading of 90: 0.25 + 0.81.
    assert!((e - 1.31).abs() < 1e-12, "{e}");
}

// ---- learning

/// A node that, every two seconds, shows an abnormal error-rate reading and, a tick later, an
/// abnormal latency reading: a pattern a readout can learn.
fn paired_pattern(services: &[gordian_world::Service], pairs: u64) -> Vec<Held> {
    let mut items = Vec::new();
    for k in 0..pairs {
        let base = 1000 + 2000 * k;
        items.push((base + 10, counter(0, CounterName::ErrorRate, 80)));
        items.push((base + 110, counter(0, CounterName::Latency, 80)));
    }
    script(services, &items)
}

#[test]
fn learning_makes_a_repeated_pattern_less_surprising_and_the_control_does_not() {
    let services = services_of(1);
    let items = paired_pattern(&services, 60);
    let until = 1000 + 2000 * 60 + 1000;
    let run = |learning: bool, key: u64| {
        let p = ReservoirParams {
            learning,
            carry: false,
            threshold: 100.0,
            ..ReservoirParams::standard(5, key)
        };
        let mut n = noticer(p, 1);
        n.log_energies();
        drive(&mut n, &items, until);
        n.energies().to_vec()
    };
    let learned = run(true, 11);
    let control = run(false, 12);
    // The second tick of pair k is tick (1000 + 2000 k + 110) / 100.
    let second = |e: &[(u64, u16, f64)], k: u64| {
        e.iter()
            .find(|(t, node, _)| *t == (1000 + 2000 * k + 110) / 100 && *node == 0)
            .unwrap()
            .2
    };
    // The control's residual at that tick is always the latency reading's own: 0.25 + 0.64.
    for k in 0..60 {
        assert!((second(&control, k) - 0.89).abs() < 1e-12);
    }
    // The first pair is unpredicted (the readout is at its prior: the residual is the control's);
    // an exact recurring association is learned from one example by least squares, so the
    // later pairs are almost fully explained.
    let first = second(&learned, 0);
    let late: f64 = (50..60).map(|k| second(&learned, k)).sum::<f64>() / 10.0;
    assert!(
        (first - 0.89).abs() < 0.01,
        "before it has seen the pattern: {first}"
    );
    assert!(late < 0.1, "after 50 repetitions: {late}");
}

#[test]
fn on_a_real_stream_the_trained_readout_has_a_lower_total_residual_than_none() {
    let seed = 10_003;
    let sp = params(seed, 150);
    let stream = generate(&sp);
    let services = stream.public_info().services;
    let all: Vec<Held> = stream
        .events()
        .iter()
        .enumerate()
        .map(|(i, (at, o))| Held {
            id: ObsId(i as u32),
            at: *at,
            abnormal: is_abnormal(o, &services),
            obs: o.clone(),
        })
        .collect();
    let total = |learning: bool, key: u64| {
        carry::reset(key);
        let p = ReservoirParams {
            learning,
            carry: false,
            threshold: 1.0,
            ..ReservoirParams::standard(5, key)
        };
        let mut n = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
        n.log_energies();
        drive(&mut n, &all, 150_000);
        n.energies().iter().map(|e| e.2).sum::<f64>()
    };
    let (on, offs) = (total(true, 21), total(false, 22));
    assert!(on < offs, "trained {on}, untrained {offs}");
}

// ---- determinism, carry

/// Play the public observations of a stream to a noticer as the rung does and return the notices
/// (with the step's instant) and the noticer's cost charges.
fn replay(n: &mut ReservoirNoticer, seed: u64, duration_s: u64) -> (Vec<(u64, Notice)>, Vec<u64>) {
    replay_params(n, &params(seed, duration_s))
}

fn replay_params(
    n: &mut ReservoirNoticer,
    sp: &gordian_stream::StreamParams,
) -> (Vec<(u64, Notice)>, Vec<u64>) {
    let stream = generate(sp);
    let public = stream.public_info();
    let all: Vec<Held> = stream
        .events()
        .iter()
        .enumerate()
        .map(|(i, (at, o))| Held {
            id: ObsId(i as u32),
            at: *at,
            abnormal: is_abnormal(o, &public.services),
            obs: o.clone(),
        })
        .collect();
    let (mut items, mut next, mut now) = (Vec::new(), 0, 0u64);
    let (mut notices, mut costs) = (Vec::new(), Vec::new());
    while now < sp.duration_ns {
        while next < all.len() && all[next].at.0 <= now {
            if all[next].abnormal {
                n.observe(&all[next]);
            }
            items.push(all[next].clone());
            next += 1;
        }
        for x in n.notice(Instant(now), &Store::with(items.clone())) {
            notices.push((now, x));
        }
        if let Some(c) = n.take_cost() {
            costs.push(c.compute_ns);
        }
        for id in n.retirable(Instant(now)) {
            n.retire(id);
        }
        now += 500 * MS;
    }
    (notices, costs)
}

#[test]
fn the_same_stream_twice_gives_identical_notices_and_costs() {
    let seed = 10_007;
    let services = services_of(seed);
    for (learning, carry_on, key) in [(false, false, 31), (true, false, 32), (true, true, 33)] {
        let p = ReservoirParams {
            learning,
            carry: carry_on,
            threshold: 0.8,
            ..ReservoirParams::standard(5, key)
        };
        carry::reset(key);
        let mut a = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
        let ra = replay(&mut a, seed, 100);
        carry::reset(key);
        let mut b = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
        let rb = replay(&mut b, seed, 100);
        assert_eq!(ra, rb, "learning {learning} carry {carry_on}");
        assert_eq!(a.readout(), b.readout());
        assert_eq!(a.stats(), b.stats());
        assert!(!ra.0.is_empty(), "the replay noticed something");
        carry::reset(key);
    }
}

#[test]
fn the_readout_is_carried_across_segments_under_its_key_and_only_then() {
    let seed = 10_008;
    let services = services_of(seed);
    let key = 41;
    carry::reset(key);
    let p = ReservoirParams {
        threshold: 0.8,
        ..ReservoirParams::standard(5, key)
    };
    let mut first = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
    assert_eq!(first.readout().updates(), 0);
    replay(&mut first, seed, 60);
    let trained = first.readout().clone();
    assert!(trained.updates() > 0);
    // The next segment starts from it.
    let second = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
    assert_eq!(second.readout(), &trained);
    // Another key, carry off, learning off: the prior.
    for q in [
        ReservoirParams { state_key: 42, ..p },
        ReservoirParams { carry: false, ..p },
        ReservoirParams {
            learning: false,
            ..p
        },
    ] {
        let n = ReservoirNoticer::new(q, RungConfig::default(), &services).unwrap();
        assert_eq!(n.readout().updates(), 0, "{q:?}");
    }
    // A carried readout of another size is not used.
    let q = ReservoirParams { size: 16, ..p };
    let n = ReservoirNoticer::new(q, RungConfig::default(), &services).unwrap();
    assert_eq!(n.readout().updates(), 0);
    carry::reset(key);
}

#[test]
fn the_readout_stays_finite_symmetric_and_positive_over_a_real_stream() {
    let seed = 10_010;
    let services = services_of(seed);
    let p = ReservoirParams {
        carry: false,
        ..ReservoirParams::standard(5, 91)
    };
    let mut n = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
    replay(&mut n, seed, 150);
    let r = n.readout();
    let d = r.dim();
    assert!(r.updates() > 10_000);
    assert!(r.weights().iter().all(|w| w.is_finite() && w.abs() < 100.0));
    let cov = r.covariance();
    for i in 0..d {
        assert!(cov[i * d + i].is_finite() && cov[i * d + i] > 0.0);
        for j in 0..d {
            assert_eq!(cov[i * d + j], cov[j * d + i]);
            // Cauchy-Schwarz for a positive semi-definite matrix.
            assert!(
                cov[i * d + j].abs() <= (cov[i * d + i] * cov[j * d + j]).sqrt() * (1.0 + 1e-9)
            );
        }
    }
}

#[test]
fn the_learning_off_control_never_trains_and_never_carries() {
    let seed = 10_009;
    let services = services_of(seed);
    let key = 51;
    carry::reset(key);
    let p = ReservoirParams {
        learning: false,
        carry: true,
        ..ReservoirParams::standard(5, key)
    };
    let mut n = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
    replay(&mut n, seed, 60);
    assert_eq!(n.readout().updates(), 0);
    assert!(n.readout().weights().iter().all(|w| *w == 0.0));
    assert!(carry::load(key).is_none());
}

// ---- the cost

#[test]
fn the_cost_is_counted_from_the_work_done_and_priced_per_operation() {
    let services = services_of(2);
    let nodes = services.len() as u64;
    for learning in [false, true] {
        let size = 16usize;
        let p = ReservoirParams {
            learning,
            carry: false,
            size: size as u32,
            ..ReservoirParams::standard(5, 61)
        };
        let mut n = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
        // 10 s of nothing: 100 complete ticks at the step of 10 s.
        n.notice(Instant(10_000 * MS), &Store::default());
        let ticks = 100u64;
        let d = regressor_len(size);
        let fan = fan_in_for(size);
        let per_tick = nodes
            * (3 * N_OUT as u64
                + (size * (fan + 7)) as u64
                + (N_OUT * d) as u64
                + if learning {
                    (d * d + d + 1 + d + N_OUT * d + d * (d + 1) / 2) as u64
                } else {
                    0
                });
        let start = nodes * (N_OUT * d) as u64;
        let cost = n.take_cost().expect("work was done");
        assert_eq!(cost.component, RESERVOIR_COMPONENT);
        // With learning the update's first call finds only the constant excited (idle ticks): the
        // denominator is above one, so every update is counted.
        assert_eq!(
            cost.compute_ns,
            (start + ticks * per_tick) * ESN_OP_NS,
            "learning {learning}"
        );
        assert_eq!(n.ops_total() * ESN_OP_NS, n.total_ns());
        assert!(n.take_cost().is_none(), "reported once");
    }
}

// ---- the manifest

#[test]
fn the_reservoir_noticer_is_written_read_and_validated() {
    let p = ReservoirParams::standard(77, 5);
    let spec = NoticerSpec::Reservoir(p);
    spec.validate().unwrap();
    let text = serde_json::to_string(&spec).unwrap();
    assert!(text.contains("\"noticer\":\"reservoir\""));
    let back: NoticerSpec = serde_json::from_str(&text).unwrap();
    assert_eq!(back, spec);
    assert_eq!(spec.id(), RESERVOIR_ID);
    assert_eq!(spec.id(), "reservoir");
    for bad in [
        ReservoirParams { size: 2, ..p },
        ReservoirParams { size: 4096, ..p },
        ReservoirParams { leak: 0.0, ..p },
        ReservoirParams { leak: 1.5, ..p },
        ReservoirParams {
            spectral_radius: 0.0,
            ..p
        },
        ReservoirParams {
            spectral_radius: f64::NAN,
            ..p
        },
        ReservoirParams {
            threshold: 0.0,
            ..p
        },
        ReservoirParams { tick_ns: 5, ..p },
        ReservoirParams { ridge: 0.0, ..p },
        ReservoirParams {
            input_scale: -1.0,
            ..p
        },
    ] {
        assert!(NoticerSpec::Reservoir(bad).validate().is_err(), "{bad:?}");
    }
    assert!(
        serde_json::from_str::<NoticerSpec>(
            "{\"noticer\":\"reservoir\",\"seed\":1,\"surprise\":2}"
        )
        .is_err(),
        "an unknown field is refused"
    );
    let n = build(&p, &RungConfig::default(), &services_of(1));
    assert_eq!(n.id(), "reservoir");
}

// ---- in a run

fn notice_rows(rec: &SegmentRecord) -> usize {
    rec.notice_log.len()
}

#[test]
fn an_arm_without_the_reservoir_writes_the_same_files_beside_a_reservoir_arm() {
    use gordian_run::stream::execute_stream;
    use gordian_run::stream::manifest::StreamArmSpec;
    use gordian_run::stream::spec::StreamPolicySpec;
    let arms = [
        ("sel_rung_privileged", "oracle_selection"),
        ("never_escalate", "never_escalate"),
    ];
    let alone = manifest("l2-alone", &arms, 2, 150, 7);
    let mut with = alone.clone();
    with.run_id = "l2-with".to_owned();
    with.arms.push(StreamArmSpec {
        arm: "sel_reservoir_privileged".to_owned(),
        policy: StreamPolicySpec::from_id("oracle_selection").unwrap(),
        context: None,
    });
    let mut p = ReservoirParams::standard(5, 71);
    p.threshold = 1.0;
    with.noticers.insert(
        "sel_reservoir_privileged".to_owned(),
        NoticerSpec::Reservoir(p),
    );
    with.validate().unwrap();
    let a = scratch("l2-alone");
    let w = scratch("l2-with");
    carry::reset(71);
    execute_stream(&alone, &a).unwrap();
    execute_stream(&with, &w).unwrap();
    carry::reset(71);
    for arm in ["sel_rung_privileged", "never_escalate"] {
        for file in [
            "results.csv",
            "incidents.csv",
            "notices.csv",
            "notice_incidents.csv",
            "notice_events.csv",
        ] {
            assert_eq!(
                without_run_id(&read(&a.join(arm), file)),
                without_run_id(&read(&w.join(arm), file)),
                "{arm}/{file}"
            );
        }
    }
    let notices = read(&w.join("sel_reservoir_privileged"), "notices.csv");
    assert!(
        notices.lines().count() > 1,
        "the reservoir noticed something"
    );
    assert_eq!(cell(&notices, 0, "noticer"), RESERVOIR_ID);
}

#[test]
fn a_segment_in_the_harness_is_a_function_of_its_inputs() {
    use gordian_run::stream::spec::StreamPolicySpec;
    let sp = params(10_011, 150);
    let l = limits(&sp);
    let spec = StreamPolicySpec::from_id("oracle_selection").unwrap();
    let mut p = ReservoirParams::standard(5, 81);
    p.learning = false;
    p.carry = false;
    let mut rung = RungConfig::default();
    rung.noticer = NoticerSpec::Reservoir(p);
    let a = play_with_rung(&sp, &spec, &l, &rung).unwrap();
    let b = play_with_rung(&sp, &spec, &l, &rung).unwrap();
    assert_eq!(a.notice_log, b.notice_log);
    assert_eq!(a.trajectory, b.trajectory);
    assert!(notice_rows(&a) > 0);
}

// ---- a tool: the residual energies over a run's seeds

/// Replays the reservoir noticer `L2_NOTICER` (the manifest's JSON for it) over the streams
/// `L2_SEEDS` (`first:count`, in order, a learning arm carrying what it learns) and writes one row
/// per stream to `L2_OUT`: how many node-ticks exceed each of a ladder of thresholds, the runs and
/// notices made at the noticer's own threshold, the readout's updates, the mean energy, the
/// operations counted and the wall time. Public observations and the noticer's own state only; the
/// evaluator's measures are not read.
#[test]
#[ignore = "a tool for the PI: writes a file"]
fn reservoir_energies() {
    use std::fmt::Write as _;
    let seeds = std::env::var("L2_SEEDS").unwrap_or_else(|_| "10000:3".to_owned());
    let out = std::env::var("L2_OUT").unwrap_or_else(|_| "l2-energies.csv".to_owned());
    let json = std::env::var("L2_NOTICER").expect("L2_NOTICER");
    let (first, count) = seeds.split_once(':').expect("first:count");
    let (first, count): (u64, u64) = (first.parse().unwrap(), count.parse().unwrap());
    let NoticerSpec::Reservoir(p) = serde_json::from_str::<NoticerSpec>(&json).unwrap() else {
        panic!("not a reservoir noticer");
    };
    const LADDER: [f64; 9] = [0.3, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0];
    carry::reset(p.state_key);
    let mut text = String::from(
        "seed,nodes,ticks,notices,runs,hot,suppressed,updates,mean_energy,ops,wall_ms",
    );
    for t in LADDER {
        write!(text, ",over_{t}").unwrap();
    }
    text.push('\n');
    for seed in first..first + count {
        let sp = gordian_stream::StreamParams::new(seed);
        let services = generate(&sp).public_info().services;
        let mut n = ReservoirNoticer::new(p, RungConfig::default(), &services).unwrap();
        n.log_energies();
        let start = std::time::Instant::now();
        let (notices, _) = replay_params(&mut n, &sp);
        let wall = start.elapsed().as_millis();
        let e = n.energies();
        let mean = e.iter().map(|x| x.2).sum::<f64>() / e.len().max(1) as f64;
        let st = n.stats();
        write!(
            text,
            "{seed},{},{},{},{},{},{},{},{mean:.6},{},{wall}",
            services.len(),
            st.ticks,
            notices.len(),
            st.runs,
            st.hot,
            st.suppressed,
            st.updates,
            n.ops_total(),
        )
        .unwrap();
        for t in LADDER {
            write!(text, ",{}", e.iter().filter(|x| x.2 > t).count()).unwrap();
        }
        text.push('\n');
    }
    carry::reset(p.state_key);
    std::fs::write(&out, text).expect("write the energies");
}
