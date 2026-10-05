//! A deterministic score for every hypothesis, from the evidence in the window.
//!
//! The hypothesis space is "no fault" plus every `(FaultKind, site)` over the public services:
//! `1 + 5 * services` hypotheses, in the order the world's checker uses (no fault, then by site,
//! then by kind).
//!
//! # Score
//!
//! Each *informative* observation is one the public rules can discriminate on: a counter at or
//! above `physics::HIGH`, a catalogue message other than `CheckHealth`, a snapshot whose hash
//! differs from the public one, a probe result, a correction result. Benign counters, free-form
//! messages, `CheckHealth`, and unchanged snapshots are neutral and ignored. A hypothesis either
//! *permits* an informative observation under the public rules or *forbids* it, so for every
//! hypothesis `permits + forbids = informative`.
//!
//! ```text
//! score(h) = BASE + permits(h) - PENALTY * forbids(h)        (saturating at 0)
//! ```
//!
//! `BASE` is the same constant for every hypothesis, including "no fault": the prior is uniform
//! over the `1 + 5 * services` hypotheses, and it was not fitted to how the world's generator
//! draws faults (which is not uniform; see the world's `DESIGN.md`, section 5). Because the
//! constant is the same everywhere it does not change the ranking; it only keeps the `u32` score
//! non-negative. The score is a count-based ranking, not a probability.
//!
//! The physics treats an observation a hypothesis does not permit as a contradiction, so a
//! hypothesis with any forbidden observation scores strictly below every hypothesis with none.
//! When the window holds an anchor-complete stream the set of top-scoring hypotheses is therefore
//! exactly the set `physics::consistent_hypotheses` returns, whenever that set is not empty
//! (tested). When it is empty the ranking is a least-contradicted list, which is the purpose of
//! scoring instead of filtering.
//!
//! # How it differs from the verifier
//!
//! - Each probe result is judged on its own, over every value of the two hidden parity bits. The
//!   verifier keeps one pair of bits for all probe results together, so it can exclude a
//!   hypothesis that two probe results contradict jointly and this estimator does not.
//! - `PENALTY` is a constant that was not tuned. It matters only for ranking hypotheses that are
//!   all contradicted.

use crate::cost::{affine_ns, compute};
use crate::ops::{Ops, Unit};
use crate::payload::{HypothesisEntry, Ranked, hypothesis_entry, unique_best};
use crate::{
    Component, ComponentOutput, ComputationRequest, ESTIMATOR_ID, VERIFIER_ID, WorkingState,
};
use gordian_core::{Charge, ComponentId, Instant};
use gordian_world::physics::{ENTANGLED, HIGH, Role, SignalText, counters, messages, probe_result};
use gordian_world::{
    CounterName, FaultKind, Hypothesis, Observation, Probe, ProbeKind, ProbeResult, ServiceId,
};
use std::cmp::Reverse;

// Declared cost, `Resource::Compute` nanoseconds: `A_NS + B_PS * n / 1000 + C_PS * s / 1000` for
// a window of `n` observations over `s` services.
// Fitted on 2026-10-04, Intel(R) Xeon(R) Processor @ 2.80GHz (4 vCPU VM), `bench` profile with the
// workspace release settings (`debug = 1`, thin LTO, one codegen unit), on core 2 under
// `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100`, then `calibrate.py` (weighted least squares
// on relative error), constants rounded from the fits of three runs, checked against a fourth.
// The coordinator reruns the calibration before merge. Ratios and the shape of the fit:
// CALIBRATION.md. Recalibrate after a change to the CPU, the release profile, or this
// component's code.
const A_NS: u64 = 485;
const B_PS: u64 = 2_930;
const C_PS: u64 = 101_000;

// Counted operations (work item A8b; `CALIBRATION.md`, section 9). One `run` counts:
//
// - `calls`: one per call (the entry's envelope: it always lists the top five);
// - `scanned`: observations the one pass over the window looked at;
// - `permit_scans`: informative observations, each of which scans the five fault kinds' permit
//   tables at its own service;
// - `ancestors`: ancestors examined to find the sites an observation at a dependent is
//   anchored at;
// - `site_updates`: anchored sites updated, each scanning the five kinds again;
// - `probe_evals`: calls of `probe_result` made to judge a probe result against a hypothesis;
// - `hypotheses`: hypotheses scored (`1 + 5 * services`), which is also what the ancestor sets
//   built from the graph scale with (services, a factor of five apart), so the graph has no unit
//   of its own;
// - `sort_cmps`: comparisons the ranking sort made, which follow how much the scores differ: a
//   window with nothing informative leaves every score equal and the sort finds that out in one
//   pass, while a window that separates the hypotheses makes it work.
const U_CALLS: usize = 0;
const U_SCANNED: usize = 1;
const U_PERMIT_SCANS: usize = 2;
const U_ANCESTORS: usize = 3;
const U_SITE_UPDATES: usize = 4;
const U_PROBE_EVALS: usize = 5;
const U_HYPOTHESES: usize = 6;
const U_SORT_CMPS: usize = 7;

// Weights, picoseconds per unit: fitted 2026-10-05; the weights of calibrate_ops.py (fixed
// windows in a loop, 5 runs of 25 timings) scaled to what the harness pays
// (calibrate_insitu.py, 3 runs of 5 passes).
// In situ: 1.39 x the weights fitted on fixed windows in a loop, plus
// 23.6 ns on the per-call unit(s) (`calibrate_insitu.py`).
// Non-negative least squares on the minimum time per call, weighted by 1/time, with no
// intercept beyond the explicit per-call unit; rounded to three figures. What each unit
// counts, the fit, its validity and its limits: CALIBRATION.md, section 9. Recalibrate
// after a change to the CPU, the release profile, the harness, or the code that is counted.
#[rustfmt::skip]
/// The estimator's units and their weights.
pub const UNITS: &[Unit] = &[
    Unit { name: "calls", weight_ps: 805000 },
    Unit { name: "scanned", weight_ps: 3430 },
    Unit { name: "permit_scans", weight_ps: 7970 },
    Unit { name: "ancestors", weight_ps: 838 },
    Unit { name: "site_updates", weight_ps: 3310 },
    Unit { name: "probe_evals", weight_ps: 8870 },
    Unit { name: "hypotheses", weight_ps: 1620 },
    Unit { name: "sort_cmps", weight_ps: 4480 },
];

/// The count-based estimator.
#[derive(Debug, Clone, Copy, Default)]
pub struct CountEstimator;

impl CountEstimator {
    /// The score every hypothesis starts from. Identical for all hypotheses.
    pub const BASE: u32 = 1 << 20;
    /// The score lost per forbidden observation. Not tuned.
    pub const PENALTY: u32 = 3;
    /// How many top hypotheses an emitted entry lists.
    pub const TOP_K: usize = 5;

    /// The estimator.
    pub fn new() -> Self {
        Self
    }

    /// The score of every hypothesis, in the order no fault, then by site, then by kind.
    pub fn scores(&self, input: &WorkingState) -> Vec<(Hypothesis, u32)> {
        self.scores_counted(input, &mut Ops::zero(ESTIMATOR_ID))
    }

    /// [`CountEstimator::scores`], adding the work done to `ops`.
    fn scores_counted(&self, input: &WorkingState, ops: &mut Ops) -> Vec<(Hypothesis, u32)> {
        let (permits, informative) = tally(input, ops);
        let score = |p: u32| -> u32 {
            let gain = u64::from(Self::BASE) + u64::from(p);
            let loss = u64::from(Self::PENALTY) * u64::from(informative - p);
            gain.saturating_sub(loss) as u32
        };
        permits
            .iter()
            .enumerate()
            .map(|(i, p)| (hypothesis_at(i), score(*p)))
            .collect()
    }
}

fn kind_index(kind: FaultKind) -> usize {
    match kind {
        FaultKind::ResourceExhausted => 0,
        FaultKind::ConfigDrift => 1,
        FaultKind::DependencyDown => 2,
        FaultKind::CredentialExpired => 3,
        FaultKind::Intermittent => 4,
    }
}

fn hyp_index(site: usize, kind: FaultKind) -> usize {
    1 + site * 5 + kind_index(kind)
}

fn hypothesis_at(index: usize) -> Hypothesis {
    if index == 0 {
        None
    } else {
        let i = index - 1;
        Some((FaultKind::ALL[i % 5], ServiceId((i / 5) as u32)))
    }
}

const ALL_BITS: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];
const NO_BITS: [(bool, bool); 1] = [(false, false)];

/// Count, for each hypothesis, the informative observations it permits, and the number of
/// informative observations.
///
/// The permit rules restate those of `physics::consistent_worlds` as counts. A test pins the two
/// together: over generated streams, the hypotheses with no forbidden observation are exactly the
/// checker's set.
fn tally(input: &WorkingState, ops: &mut Ops) -> (Vec<u32>, u32) {
    let services = &input.public.services;
    let s_count = services.len();
    let mut permits = vec![0u32; 1 + 5 * s_count];
    let mut informative = 0u32;

    // Bit d of `anc[v]` is set when v depends on d, directly or transitively. Edges run from
    // higher to lower index, so one forward pass closes the relation.
    let mut anc = vec![0u64; s_count];
    for (v, service) in services.iter().enumerate() {
        for d in &service.depends_on {
            let di = d.index();
            if di < v && di < 64 {
                anc[v] |= (1u64 << di) | anc[di];
            }
        }
    }
    let mut error_seen: Vec<Option<Instant>> = vec![None; s_count];
    let mut drift: Vec<Option<u64>> = vec![None; s_count];

    // Bit s is set when an observation at `v` counts as a dependent observation for a fault at
    // `s`: `v` depends on `s`, and an `ErrorRate` at or above `HIGH` at `s` appeared earlier in
    // the window at an instant not later than `t`.
    let anchored = |v: usize, t: Instant, error_seen: &[Option<Instant>]| -> u64 {
        let mut out = 0u64;
        let mut m = anc[v];
        while m != 0 {
            let s = m.trailing_zeros() as usize;
            m &= m - 1;
            if error_seen[s].is_some_and(|first| first <= t) {
                out |= 1u64 << s;
            }
        }
        out
    };

    for (t, obs) in input.evidence() {
        ops.add(U_SCANNED, 1);
        match obs {
            Observation::Counter {
                service,
                name,
                value,
            } => {
                let v = service.index();
                if v >= s_count {
                    // A reference outside the graph contradicts every hypothesis.
                    informative += 1;
                    continue;
                }
                if *value < HIGH {
                    continue;
                }
                informative += 1;
                ops.add(U_PERMIT_SCANS, 1);
                for kind in FaultKind::ALL {
                    if counters(kind, Role::Site).contains(name) {
                        permits[hyp_index(v, kind)] += 1;
                    }
                }
                ops.add(U_ANCESTORS, u64::from(anc[v].count_ones()));
                let mut sites = anchored(v, *t, &error_seen);
                ops.add(U_SITE_UPDATES, u64::from(sites.count_ones()));
                while sites != 0 {
                    let s = sites.trailing_zeros() as usize;
                    sites &= sites - 1;
                    for kind in FaultKind::ALL {
                        if counters(kind, Role::Dependent).contains(name) {
                            permits[hyp_index(s, kind)] += 1;
                        }
                    }
                }
                if *name == CounterName::ErrorRate {
                    error_seen[v] = Some(error_seen[v].map_or(*t, |first| first.min(*t)));
                }
            }
            Observation::Message {
                service, text_id, ..
            } => {
                let v = service.index();
                if v >= s_count {
                    informative += 1;
                    continue;
                }
                let Some(text) = SignalText::from_text_id(*text_id) else {
                    continue;
                };
                if text == SignalText::CheckHealth {
                    continue;
                }
                informative += 1;
                ops.add(U_PERMIT_SCANS, 1);
                for kind in FaultKind::ALL {
                    if messages(kind, Role::Site).contains(&text) {
                        permits[hyp_index(v, kind)] += 1;
                    }
                }
                ops.add(U_ANCESTORS, u64::from(anc[v].count_ones()));
                let mut sites = anchored(v, *t, &error_seen);
                ops.add(U_SITE_UPDATES, u64::from(sites.count_ones()));
                while sites != 0 {
                    let s = sites.trailing_zeros() as usize;
                    sites &= sites - 1;
                    for kind in FaultKind::ALL {
                        if messages(kind, Role::Dependent).contains(&text) {
                            permits[hyp_index(s, kind)] += 1;
                        }
                    }
                }
            }
            Observation::Snapshot {
                service,
                config_hash,
            } => {
                let v = service.index();
                if v >= s_count {
                    informative += 1;
                    continue;
                }
                if *config_hash == services[v].config_hash {
                    continue;
                }
                informative += 1;
                ops.add(U_PERMIT_SCANS, 1);
                // Only a drift at this service permits a changed snapshot, and only if every
                // changed hash seen at the service is the same one.
                if note_drift(&mut drift[v], *config_hash) {
                    permits[hyp_index(v, FaultKind::ConfigDrift)] += 1;
                }
            }
            Observation::Probed { probe, result } => {
                informative += 1;
                let target = probe.target.index();
                if target >= s_count {
                    continue;
                }
                let start = services[target].config_hash;
                let mut consistent_drift = true;
                if probe.kind == ProbeKind::ConfigSnapshot
                    && let ProbeResult::ConfigHash(h) = result
                    && *h != start
                {
                    consistent_drift = note_drift(&mut drift[target], *h);
                }
                let drift_hash = drift[target].unwrap_or(start.wrapping_add(1));
                if consistent_drift {
                    let mut evaluated = 0u64;
                    for (i, count) in permits.iter_mut().enumerate() {
                        if probe_permits(
                            services,
                            hypothesis_at(i),
                            drift_hash,
                            *probe,
                            *result,
                            &mut evaluated,
                        ) {
                            *count += 1;
                        }
                    }
                    ops.add(U_PROBE_EVALS, evaluated);
                }
            }
            Observation::Correction { site, resolved } => {
                informative += 1;
                if site.index() >= s_count {
                    continue;
                }
                for (i, count) in permits.iter_mut().enumerate() {
                    let at_site = hypothesis_at(i).is_some_and(|(_, s)| s == *site);
                    if at_site == *resolved {
                        *count += 1;
                    }
                }
            }
        }
    }
    (permits, informative)
}

/// Record a changed hash for a service. False when it differs from one already recorded.
fn note_drift(slot: &mut Option<u64>, hash: u64) -> bool {
    *slot.get_or_insert(hash) == hash
}

/// Whether `truth` could have produced `result` for `probe`, for some value of the two hidden
/// bits (which only matter for the entangled kinds).
fn probe_permits(
    services: &[gordian_world::Service],
    truth: Hypothesis,
    drift_hash: u64,
    probe: Probe,
    result: ProbeResult,
    evaluated: &mut u64,
) -> bool {
    let entangled = truth.is_some_and(|(k, _)| k == ENTANGLED.0 || k == ENTANGLED.1);
    let options: &[(bool, bool)] = if entangled { &ALL_BITS } else { &NO_BITS };
    options.iter().any(|bits| {
        *evaluated += 1;
        probe_result(services, truth, *bits, drift_hash, probe) == result
    })
}

impl Component for CountEstimator {
    fn id(&self) -> ComponentId {
        ESTIMATOR_ID
    }

    fn declared_cost(&self, input: &WorkingState) -> Vec<Charge> {
        let ns = affine_ns(A_NS, B_PS, input.size()).saturating_add(affine_ns(
            0,
            C_PS,
            input.public.services.len(),
        ));
        vec![compute(ns)]
    }

    fn run_counted(&mut self, input: &WorkingState) -> (ComponentOutput, Ops) {
        let mut ops = Ops::zero(ESTIMATOR_ID);
        ops.add(U_CALLS, 1);
        let scores = self.scores_counted(input, &mut ops);
        ops.add(U_HYPOTHESES, scores.len() as u64);
        let mut order: Vec<usize> = (0..scores.len()).collect();
        // Best score first; equal scores keep hypothesis order. Indices are unique, so the sort
        // is total and the result does not depend on the sort algorithm. The comparator counts
        // its own calls.
        let mut comparisons = 0u64;
        order.sort_unstable_by(|a, b| {
            comparisons += 1;
            (Reverse(scores[*a].1), *a).cmp(&(Reverse(scores[*b].1), *b))
        });
        ops.add(U_SORT_CMPS, comparisons);
        let top = scores[order[0]].1;
        let tied = order.iter().take_while(|i| scores[**i].1 == top).count() as u32;
        let ranked: Vec<Ranked> = order
            .iter()
            .take(Self::TOP_K)
            .map(|i| Ranked {
                hypothesis: scores[*i].0,
                score: Some(scores[*i].1),
            })
            .collect();
        let proposal = unique_best(&ranked, tied);
        let mut requests = Vec::new();
        if proposal.is_some() {
            requests.push(ComputationRequest {
                component: VERIFIER_ID,
                reason: "confirm estimator's leading hypothesis".to_string(),
            });
        }
        let entry = HypothesisEntry::Candidates {
            source: "estimator".to_string(),
            basis: "count of permitted minus forbidden observations".to_string(),
            ranked,
            tied_at_top: tied,
        };
        let output = ComponentOutput {
            entries: vec![hypothesis_entry(&entry)],
            requests,
            proposal,
        };
        (output, ops)
    }
}
