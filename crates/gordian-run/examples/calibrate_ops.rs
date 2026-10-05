//! Times each component and the shared rule against the work they count (work item A8b).
//!
//! For a set of fixed windows and recorded states it prints one JSON line per timing: which
//! component, what the window was (class, size, services, records, where it came from), the
//! operation counts of one call by unit name, and the *minimum* over repeated timings of the
//! per-call time. The minimum, because interference from the host only ever adds time. A line's
//! `set` says what it is for: `fit` lines are the ones weights are fitted to, `heldout` lines are
//! new episodes at window sizes the fit never saw, and `real` lines (and `real_fit` lines, which only the rule is fitted to) are states recorded from
//! episodes played by the real harness, so that the rule and the components are also judged on
//! the windows, probe results and candidate sets an experiment produces. `crates/gordian-run/
//! calibrate_ops.py` reads the lines, fits the weights and reports the validity of the counters.
//!
//! Run it on one idle core through the runner, after building, so that no compiler runs during a
//! measurement:
//!
//! ```text
//! cargo build --release --example calibrate_ops
//! scripts/cgroup-run.sh --name calib-ops --cpus 2 --cpu-quota 100 --memory 2G -- \
//!     target/release/examples/calibrate_ops > target/calibrate-ops.jsonl
//! python3 crates/gordian-run/calibrate_ops.py target/calibrate-ops.jsonl
//! ```
//!
//! The program times nothing a policy can see and writes nothing but those lines. Environment:
//! `CALIBRATE_REPS` (timings per datum, default 25) and `CALIBRATE_SCALE` (a divisor of the number
//! of recorded states kept, default 1; raise it for a quick run).

use gordian_components::{
    Component, ComponentOutput, ConsistencyVerifier, CountEstimator, Ops, PriorRecordLookup,
    RuleHeuristic, WorkingState, ops,
};
use gordian_core::{Bill, ComponentId, Instant};
use gordian_run::harness::{Limits, run_episode};
use gordian_run::policy::all_components::AllComponents;
use gordian_run::policy::decide::{DecideConfig, RULE_UNITS};
use gordian_run::policy::fixed_pipeline::{self, Pipeline};
use gordian_run::policy::heuristic_only::Heuristic;
use gordian_run::policy::random_matched::{RandomSubset, rng_seed};
use gordian_run::policy::{Arm, Policy, PolicyId, Selector};
use gordian_world::episode::PriorRecord;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{HIGH, SignalText, SymptomTag};
use gordian_world::{
    Action, CounterName, Episode, EpisodeClass, EpisodeSpec, FaultKind, Observation, Outcome,
    ProbeKind, ServiceId, Severity, Simulator, generate,
};
use serde_json::json;
use std::cell::RefCell;
use std::hint::black_box;
use std::io::Write;
use std::rc::Rc;
use std::time::Instant as Wall;

// Sizes of the fixed windows. Small ones matter: an episode starts with an empty window and
// grows it, and what a call costs with little in the window (nothing to scan, an early return, a
// short entry) is not what a large window costs.
const FIT_SIZES: &[usize] = &[0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
const HELDOUT_SIZES: &[usize] = &[3, 6, 12, 24, 48, 96, 192, 384, 768, 1536];
const FIT_SEEDS: std::ops::Range<u64> = 0..3;
const HELDOUT_SEEDS: std::ops::Range<u64> = 3..6;
const BASE_SEED: u64 = 0x0C0B_0000;
const PAD_COUNTS: [usize; 4] = [0, 64, 256, 1024];
const PROBE_COUNTS: [usize; 3] = [2, 6, 12];

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Minimum and median, in nanoseconds per call, of `reps` timings of `batch` calls of `f`.
fn measure(reps: usize, mut f: impl FnMut()) -> (f64, f64, usize) {
    for _ in 0..4 {
        f();
    }
    let started = Wall::now();
    f();
    let one = started.elapsed().as_nanos().max(1) as f64;
    // About 300 microseconds per timing, so that the clock's own cost is a small fraction.
    let batch = ((300_000.0 / one).ceil() as usize).clamp(1, 50_000);
    let mut samples = Vec::with_capacity(reps);
    for _ in 0..reps {
        let started = Wall::now();
        for _ in 0..batch {
            f();
        }
        samples.push(started.elapsed().as_nanos() as f64 / batch as f64);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[0], samples[samples.len() / 2], batch)
}

/// A window of exactly `n` observations of the episode's stream, repeated with shifted instants
/// when the stream is shorter (the same construction as the A5 benchmark workload).
fn window(ep: &Episode, n: usize) -> WorkingState {
    let stream = ep.stream();
    let lap = ep.spec().horizon.0 + 1;
    let mut w = WorkingState::new(ep.public_info(), n);
    for k in 0..n {
        let (t, o) = &stream[k % stream.len()];
        w.admit(Instant(t.0 + (k / stream.len()) as u64 * lap), o.clone());
    }
    w
}

/// `base` with up to `k` probe results appended, taken from the real simulator, cycling through
/// the probe kinds and the services from a seed-dependent start.
fn probed(ep: &Episode, base: &WorkingState, k: usize) -> WorkingState {
    let mut w = base.clone();
    let mut sim = Simulator::new(ep.clone());
    let services = ep.world().len() as u32;
    let mut at = w.now.0 + 1_000_000;
    for i in 0..k {
        let kind = ProbeKind::ALL[i % ProbeKind::ALL.len()];
        let target = ServiceId((i as u32 * 5 + ep.spec().seed as u32) % services);
        match sim.apply(Action::Probe { kind, target }, Instant(at)) {
            Outcome::Probed {
                observation,
                ready_at,
                ..
            } => {
                w.admit(Instant(ready_at.0.max(at)), observation);
                at = ready_at.0.max(at) + 1_000_000;
            }
            _ => break,
        }
    }
    w
}

/// A window the heuristic's upstream-site rule decides on, which no generated window is: the site's
/// own message always arrives with the dependent's and decides first. `k` high `ErrorRate`
/// alarms at services the subject does not depend on, then one at a service it does, then the
/// subject's `UpstreamUnreachable` message, after `pad` benign counters. The rule has to build
/// the dependents of `k + 1` sites before it finds the one the subject depends on.
fn upstream_window(ep: &Episode, k: usize, pad: usize) -> Option<WorkingState> {
    let public = ep.public_info();
    let subject = public
        .services
        .iter()
        .rev()
        .find(|s| !s.depends_on.is_empty())?;
    let (subject_id, site) = (subject.id, subject.depends_on[0]);
    let decoys: Vec<ServiceId> = public
        .services
        .iter()
        .map(|s| s.id)
        .filter(|id| *id != subject_id && *id != site)
        .filter(|id| !dependents_mask(&public.services, *id)[subject_id.index()])
        .take(k)
        .collect();
    if decoys.len() < k {
        return None;
    }
    let mut w = WorkingState::new(public, pad + k + 2);
    let mut at = 1_000_000u64;
    let mut admit = |obs: Observation| {
        w.admit(Instant(at), obs);
        at += 1_000_000;
    };
    for i in 0..pad {
        admit(Observation::Counter {
            service: ServiceId((i % 4) as u32),
            name: CounterName::Latency,
            value: 1,
        });
    }
    for id in decoys.into_iter().chain(std::iter::once(site)) {
        admit(Observation::Counter {
            service: id,
            name: CounterName::ErrorRate,
            value: HIGH,
        });
    }
    admit(Observation::Message {
        service: subject_id,
        text_id: SignalText::UpstreamUnreachable.text_id(),
        severity: Severity::Low,
    });
    Some(w)
}

/// `w` with `extra` more prior records that cannot match any window (as in the A5 benchmark).
fn padded(w: &WorkingState, extra: usize) -> WorkingState {
    let vocabulary: Vec<SymptomTag> = CounterName::ALL
        .into_iter()
        .map(SymptomTag::Counter)
        .chain(
            SignalText::ALL
                .into_iter()
                .filter(|t| *t != SignalText::CheckHealth)
                .map(SymptomTag::Text),
        )
        .collect();
    let mut w = w.clone();
    for i in 0..extra {
        let len = (i * 7) % 4;
        let mut tags: Vec<SymptomTag> = (0..len)
            .map(|j| vocabulary[(i * 5 + j * 3) % vocabulary.len()])
            .collect();
        tags.push(SymptomTag::Text(SignalText::CheckHealth));
        tags.sort();
        tags.dedup();
        w.public.prior_records.push(PriorRecord {
            signature: tags,
            resolution: FaultKind::ALL[i % 5],
        });
    }
    w
}

fn components() -> Vec<(&'static str, Box<dyn Component>)> {
    vec![
        ("heuristic", Box::new(RuleHeuristic::new())),
        ("estimator", Box::new(CountEstimator::new())),
        ("memory", Box::new(PriorRecordLookup::new())),
        ("verifier", Box::new(ConsistencyVerifier::new())),
    ]
}

/// What a datum is, for the fit script.
struct Tag<'a> {
    set: &'a str,
    source: &'a str,
    class: String,
    arm: &'a str,
    n: usize,
    probes: usize,
    extra_records: usize,
}

fn emit(
    out: &mut impl Write,
    tag: &Tag,
    target: &str,
    w: &WorkingState,
    counts: serde_json::Value,
    timing: (f64, f64, usize),
) {
    let line = json!({
        "set": tag.set,
        "source": tag.source,
        "class": tag.class,
        "arm": tag.arm,
        "target": target,
        "n": w.size(),
        "capacity": w.capacity(),
        "services": w.public.services.len(),
        "records": w.public.prior_records.len(),
        "requested_n": tag.n,
        "probes": tag.probes,
        "extra_records": tag.extra_records,
        "counts": counts,
        "min_ns": timing.0,
        "med_ns": timing.1,
        "batch": timing.2,
    });
    writeln!(out, "{line}").expect("write a line");
}

fn component_counts(counted: &Ops) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (unit, count) in ops::units(counted.component()).iter().zip(counted.counts()) {
        map.insert(unit.name.to_string(), json!(count));
    }
    serde_json::Value::Object(map)
}

fn rule_counts(counts: &[u64]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (unit, count) in RULE_UNITS.iter().zip(counts) {
        map.insert(unit.name.to_string(), json!(count));
    }
    serde_json::Value::Object(map)
}

/// Time every component on `w` and print the lines.
fn time_components(out: &mut impl Write, reps: usize, tag: &Tag, w: &WorkingState) {
    for (name, mut component) in components() {
        let (_, counted) = component.run_counted(w);
        let again = component.run_counted(w).1;
        assert_eq!(counted, again, "counts must repeat");
        let timing = measure(reps, || {
            black_box(component.run_counted(black_box(w)));
        });
        emit(out, tag, name, w, component_counts(&counted), timing);
    }
}

// ---------------------------------------------------------------------------------------------
// Recorded states of real episodes.
// ---------------------------------------------------------------------------------------------

enum Kind {
    Step {
        outputs: Vec<(ComponentId, ComponentOutput)>,
    },
    Final,
}

struct Snapshot<S: Selector + Clone> {
    arm: Arm<S>,
    state: WorkingState,
    bill: Bill,
    kind: Kind,
}

/// A policy that behaves as `Arm` does and keeps a copy of the arm, the state and the bill
/// before each scheduling call. It is a measurement instrument: it changes nothing the arm does.
struct Recorder<S: Selector + Clone> {
    inner: Arm<S>,
    keep: usize,
    seen: usize,
    pending: Option<(Arm<S>, Bill)>,
    snaps: Rc<RefCell<Vec<Snapshot<S>>>>,
}

impl<S: Selector + Clone> Recorder<S> {
    fn kept(&mut self) -> bool {
        self.seen += 1;
        // A fixed integer hash of the position, so which states are kept does not depend on time.
        (((self.seen as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) & 0xFFFF)
            < (0x10000 / self.keep as u64)
    }
}

impl<S: Selector + Clone> Policy for Recorder<S> {
    fn id(&self) -> PolicyId {
        self.inner.id()
    }
    fn decision_rule(&self) -> &'static str {
        self.inner.decision_rule()
    }
    fn declared_select_cost(&self, state: &WorkingState) -> Vec<gordian_core::Charge> {
        self.inner.declared_select_cost(state)
    }
    fn select(&mut self, state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.pending = Some((self.inner.clone(), bill.clone()));
        self.inner.select(state, bill)
    }
    fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        if let Some((arm, bill)) = self.pending.take()
            && self.kept()
        {
            self.snaps.borrow_mut().push(Snapshot {
                arm,
                state: state.clone(),
                bill,
                kind: Kind::Step {
                    outputs: outputs.to_vec(),
                },
            });
        }
        self.inner.decide(state, outputs)
    }
    fn declared_final_cost(&self, state: &WorkingState) -> Vec<gordian_core::Charge> {
        self.inner.declared_final_cost(state)
    }
    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        // Final calls are rare; keep every one.
        self.snaps.borrow_mut().push(Snapshot {
            arm: self.inner.clone(),
            state: state.clone(),
            bill: Bill::new(Limits::default().budget()),
            kind: Kind::Final,
        });
        self.inner.decide_final(state)
    }
    fn take_ops(&mut self) -> gordian_run::policy::decide::RuleOps {
        self.inner.take_ops()
    }
}

fn record_arm<S: Selector + Clone + 'static>(
    out: &mut impl Write,
    reps: usize,
    scale: usize,
    name: &'static str,
    make: &dyn Fn(u64) -> Arm<S>,
    limits: &Limits,
) {
    for class in EpisodeClass::ALL {
        for seed in 100..107u64 {
            let mut spec = EpisodeSpec::new(seed, class);
            spec.budget = limits.world_budget();
            let snaps = Rc::new(RefCell::new(Vec::new()));
            let mut recorder = Recorder {
                inner: make(seed),
                keep: 8 * scale,
                seen: 0,
                pending: None,
                snaps: snaps.clone(),
            };
            let mut comps = gordian_run::standard_components();
            run_episode(&spec, &mut recorder, &mut comps, limits).expect("the episode runs");
            // The first four seeds are what the rule's weights are fitted to (the rule has no
            // fixed windows: its input is a candidate set, and only episodes make those); the
            // other three are held out, and the components are timed on those states only.
            let held_out = seed >= 104;
            for snap in snaps.borrow().iter() {
                let tag = Tag {
                    set: if held_out { "real" } else { "real_fit" },
                    source: "episode",
                    class: format!("{class:?}"),
                    arm: name,
                    n: snap.state.size(),
                    probes: 0,
                    extra_records: 0,
                };
                time_rule(out, reps, &tag, snap);
                if held_out && matches!(snap.kind, Kind::Step { .. }) {
                    time_components(out, reps, &tag, &snap.state);
                }
            }
        }
    }
}

fn time_rule<S: Selector + Clone>(
    out: &mut impl Write,
    reps: usize,
    tag: &Tag,
    snap: &Snapshot<S>,
) {
    let mut arm = snap.arm.clone();
    let state = &snap.state;
    match &snap.kind {
        Kind::Step { outputs } => {
            // One real call first; then the arm is in the state the same call finds it in on
            // every later repeat, which is the state the counts and the timings are taken in.
            let step = |arm: &mut Arm<S>| {
                black_box(arm.declared_select_cost(black_box(state)));
                black_box(arm.select(black_box(state), &snap.bill));
                black_box(arm.decide(black_box(state), black_box(outputs)));
            };
            for _ in 0..3 {
                step(&mut arm);
            }
            arm.take_ops();
            step(&mut arm);
            let counted = arm.take_ops();
            let timing = measure(reps, || {
                step(&mut arm);
                black_box(arm.take_ops());
            });
            emit(
                out,
                tag,
                "rule",
                state,
                rule_counts(counted.counts()),
                timing,
            );
        }
        Kind::Final => {
            let step = |arm: &mut Arm<S>| {
                black_box(arm.declared_final_cost(black_box(state)));
                black_box(arm.decide_final(black_box(state)));
            };
            for _ in 0..3 {
                step(&mut arm);
            }
            arm.take_ops();
            step(&mut arm);
            let counted = arm.take_ops();
            let timing = measure(reps, || {
                step(&mut arm);
                black_box(arm.take_ops());
            });
            emit(
                out,
                tag,
                "rule",
                state,
                rule_counts(counted.counts()),
                timing,
            );
        }
    }
}

fn main() {
    let reps = env_usize("CALIBRATE_REPS", 25);
    let scale = env_usize("CALIBRATE_SCALE", 1);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    // Fixed windows: the stream of one episode cut (or repeated) to a size.
    for (set, seeds, sizes) in [
        ("fit", FIT_SEEDS, FIT_SIZES),
        ("heldout", HELDOUT_SEEDS, HELDOUT_SIZES),
    ] {
        for (ci, class) in EpisodeClass::ALL.into_iter().enumerate() {
            for k in seeds.clone() {
                let ep = generate(&EpisodeSpec::new(BASE_SEED + ci as u64 * 16 + k, class));
                for &n in sizes {
                    let w = window(&ep, n);
                    let tag = Tag {
                        set,
                        source: "pool",
                        class: format!("{class:?}"),
                        arm: "",
                        n,
                        probes: 0,
                        extra_records: 0,
                    };
                    time_components(&mut out, reps, &tag, &w);
                }
            }
        }
        out.flush().unwrap();
    }

    // Windows on which the heuristic's upstream-site rule decides.
    for (set, seeds) in [("fit", 0..3u64), ("heldout", 3..6u64)] {
        for k in seeds {
            let ep = generate(&EpisodeSpec::new(
                BASE_SEED + 0x1000 + k,
                EpisodeClass::Ambiguous,
            ));
            for decoys in 0..4usize {
                for pad in [0usize, 24] {
                    let Some(w) = upstream_window(&ep, decoys, pad) else {
                        continue;
                    };
                    let tag = Tag {
                        set,
                        source: "upstream",
                        class: "Synthetic".to_string(),
                        arm: "",
                        n: w.size(),
                        probes: 0,
                        extra_records: 0,
                    };
                    time_components(&mut out, reps, &tag, &w);
                }
            }
        }
    }
    out.flush().unwrap();

    // Windows with probe results in them, which generated streams do not have and every arm that
    // probes produces, and windows over many prior records.
    for (set, seeds) in [("fit", 0..2u64), ("heldout", 3..5u64)] {
        for (ci, class) in EpisodeClass::ALL.into_iter().enumerate() {
            for k in seeds.clone() {
                let ep = generate(&EpisodeSpec::new(BASE_SEED + ci as u64 * 16 + k, class));
                for n in [64usize, 256] {
                    let base = window(&ep, n);
                    for probes in PROBE_COUNTS {
                        let w = probed(&ep, &base, probes);
                        let tag = Tag {
                            set,
                            source: "probed",
                            class: format!("{class:?}"),
                            arm: "",
                            n,
                            probes,
                            extra_records: 0,
                        };
                        time_components(&mut out, reps, &tag, &w);
                    }
                }
                let base = window(&ep, 64);
                for extra in PAD_COUNTS {
                    let w = padded(&base, extra);
                    let tag = Tag {
                        set,
                        source: "padded",
                        class: format!("{class:?}"),
                        arm: "",
                        n: 64,
                        probes: 0,
                        extra_records: extra,
                    };
                    for (name, mut component) in components() {
                        if name != "memory" {
                            continue;
                        }
                        let (_, counted) = component.run_counted(&w);
                        let timing = measure(reps, || {
                            black_box(component.run_counted(black_box(&w)));
                        });
                        emit(&mut out, &tag, name, &w, component_counts(&counted), timing);
                    }
                }
            }
        }
        out.flush().unwrap();
    }

    // States recorded from real episodes: default limits, and a binding compute budget under
    // which arms run out of affordable work and the harness makes its final call.
    let default = Limits::default();
    let mut tight = Limits::default();
    tight.compute = 250_000;
    for limits in [&default, &tight] {
        let config = DecideConfig::default();
        record_arm(
            &mut out,
            reps,
            scale,
            "heuristic_only",
            &|_| Arm::with(Heuristic, config),
            limits,
        );
        record_arm(
            &mut out,
            reps,
            scale,
            "all_components",
            &|_| Arm::with(AllComponents, config),
            limits,
        );
        record_arm(
            &mut out,
            reps,
            scale,
            "fixed_pipeline",
            &|_| Arm::with(Pipeline::new(fixed_pipeline::Config::default()), config),
            limits,
        );
        record_arm(
            &mut out,
            reps,
            scale,
            "random_matched",
            &|seed| {
                Arm::with(
                    RandomSubset::new(0.5, rng_seed(seed, "random_matched")),
                    config,
                )
            },
            limits,
        );
        out.flush().unwrap();
    }
}
