//! The decision rule every non-privileged arm shares.
//!
//! EXP-001's intervention is the scheduling policy only (charter section 6). An arm therefore
//! differs from another in which components it selects, never in what it does with their outputs.
//! This module is that one rule. [`crate::policy::Arm`] is the only non-privileged
//! [`crate::policy::Policy`] and delegates `decide` here, so the sharing is structural, and a
//! test checks it.
//!
//! The full statement, with the reasons for each choice, is in `POLICIES.md`. In short:
//!
//! 1. The rule stores the latest output of the verifier, the estimator and the heuristic, in
//!    decoded form. A component that was not selected leaves its previous output in place, so an
//!    arm that skips a component is acting on stale output and pays for it in decisions. An output
//!    is decoded once: when one arrives that is byte for byte the entries the rule already holds
//!    for that component (the component ran again on a window that told it nothing new), the
//!    decoded form is kept and the decoding is neither done, declared nor counted again
//!    (work item A6c).
//! 2. The candidate set is the first available of: the verifier's consistent set, the
//!    estimator's top-tied hypotheses, the heuristic's candidates. It is then narrowed by the
//!    probe results in the working state, using the world's public `probe_result` semantics. That
//!    narrowing is the rule's own bookkeeping for the probes it bought and nothing else: no
//!    consistency checking of passive observations happens here, because that is the verifier's
//!    job and an arm that never selects the verifier must not get it free.
//! 3. One hypothesis left: declare it (a lone "no fault" only once the patience has passed,
//!    because silence is also what a fault looks like before its onset).
//! 4. Several hypotheses, none of them "no fault": buy the affordable probe with the smallest
//!    expected size of the remaining set, if it is strictly smaller than the present size.
//! 5. At the patience deadline: declare the first-ranked hypothesis, or abstain with none.
//!    Otherwise wait.
//!
//! The harness also gives the rule one *final call* ([`Decider::decide_final`]) when it has found
//! that no affordable work is left or that the horizon was reached. The final call is the
//! patience deadline arriving early: `due` is true whatever the clock says, and nothing else about
//! the rule changes, so an arm that has run out of means declares what it has instead of ending
//! undecided. A test pins `decide_final` to `decide` at a state whose clock is at the deadline.
//!
//! The expected size is over *hypotheses* (the size of what `consistent_hypotheses` returns),
//! under a uniform prior over the consistent *worlds* (hypothesis plus the two hidden parity
//! bits). A probe that removes worlds but no hypothesis therefore scores no gain, which is why
//! this rule does not buy the first of a jointly decisive pair of samples.
//!
//! The rule is a pure function of the working state, the outputs it has been shown, and the
//! remaining limits it was told: no clock, no randomness, no I/O.

use gordian_components::ops::Unit;
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{ComponentOutput, ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, EntryKind, Instant, Resource};
use gordian_world::physics::{ENTANGLED, probe_cost, probe_result};
use gordian_world::{
    Action, Hypothesis, Observation, Probe, ProbeKind, ProbeResult, Service, ServiceId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The name of this rule, as [`crate::policy::Policy::decision_rule`] reports it.
pub const RULE: &str = "expected-set-size";

/// Default patience: 3 s of logical time. Every generated symptom is emitted within about half a
/// second of an onset that is at most a quarter of the default horizon, so by 3 s everything
/// that will arrive has arrived.
pub const DEFAULT_PATIENCE: Instant = Instant(3_000_000_000);

/// The parameters of the shared rule. One value per run, in the manifest, for every arm: a
/// per-arm patience would be a difference in `decide`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecideConfig {
    /// The deadline, in nanoseconds of logical time, at which the rule stops waiting and
    /// declares its first-ranked hypothesis (or abstains with none).
    pub patience_ns: u64,
}

impl Default for DecideConfig {
    fn default() -> Self {
        Self {
            patience_ns: DEFAULT_PATIENCE.0,
        }
    }
}

// Declared cost of one call, in picoseconds per unit so that slopes of a few nanoseconds keep
// their precision; the charge is `Resource::Compute` nanoseconds. Fitted on 2026-10-04 on the
// 4 vCPU Xeon (release profile, not pinned: the sandbox refused `taskset` for builds, and another
// worker was benchmarking) by the ignored test `measure_the_rule_against_its_declared_cost` in
// `tests/baselines.rs`, weighted least squares on relative error, rounded. What was measured and
// what the constants do not cover: `POLICIES.md`, section 3.
const BASE_PS: u64 = 45_000;
const SCAN_PS: u64 = 950;
const WORLD_PS: u64 = 10_500;
const EVAL_PS: u64 = 36_000;
const DECODE_OUTPUT_PS: u64 = 530_000;
const DECODE_HYPOTHESIS_PS: u64 = 115_000;
// Work item A6c: an arriving output is compared with the entries held for its component, and only
// decoded when they differ. The comparison is charged per byte it looks at (`COMPARED_BYTES`).
const COMPARE_BYTE_PS: u64 = 33;

// Counted operations (work item A8b; `CALIBRATION.md`, section 9). The shared rule counts the
// work of the scheduling path it is part of, the same way a component does:
//
// - `calls`: one per `decide` or final call (the step's fixed cost: the cost declaration, the
//   selection and the decision, which run once per step);
// - `decoded_outputs`: component outputs whose entry was decoded (an output byte for byte equal to
//   the one already held for its component is not decoded: work item A6c);
// - `decoded_ranked`: candidates in the entries decoded;
// - `worlds`: worlds built from a candidate set, and worlds visited when a probe is scored;
// - `probe_evals`: calls of `probe_result` made to score candidate probes, one per (probe, world);
// - `compared_bytes`: payload bytes of an arriving output that were compared with the held copy
//   (the entries' shape agreed, so the comparison had to look at the bytes; an upper bound when
//   they turn out to differ, since the comparison stops at the first differing byte).
//
// What is not a unit, and why (`CALIBRATION.md`, section 9): the scan of the window for bought
// probes (0.3 ns an observation, under 3% of a call at the largest window), the check of bought
// probes against a world, the grouping of worlds by a probe's result and the per-probe set-up of
// scoring (each fitted at under 2% of the time, and not separable from `probe_evals` and `worlds`:
// dropping all four changes no fit by more than 0.0002 in R^2), and corrections (the rule never
// buys one).
const R_CALLS: usize = 0;
const R_DECODED_OUTPUTS: usize = 1;
const R_DECODED_RANKED: usize = 2;
const R_WORLDS: usize = 3;
const R_PROBE_EVALS: usize = 4;
const R_COMPARED_BYTES: usize = 5;
/// How many units the rule has.
const R_UNITS: usize = 6;

// Weights, picoseconds per unit: fitted 2026-10-05 after work item A6c (the rule recognises a
// repeated output): the weights of calibrate_ops.py (fixed windows and recorded states in a
// loop, 5 runs of 25 timings, the rule timed on its comparison path and, alternating padded
// outputs, its decoding path) scaled to what the harness pays (calibrate_insitu.py, 3 runs of 5
// passes); only the rule's table was rewritten.
// In situ: 1.37 x the weights fitted on fixed windows in a loop, plus
// 130 ns on the per-call unit(s) (`calibrate_insitu.py`).
// Non-negative least squares on the minimum time per call, weighted by 1/time, with no
// intercept beyond the explicit per-call unit; rounded to three figures. What each unit
// counts, the fit, its validity and its limits: CALIBRATION.md, section 9. Recalibrate
// after a change to the CPU, the release profile, the harness, or the code that is counted.
#[rustfmt::skip]
/// The shared rule's units and their weights.
pub const RULE_UNITS: &[Unit] = &[
    Unit { name: "calls", weight_ps: 246000 },
    Unit { name: "decoded_outputs", weight_ps: 365000 },
    Unit { name: "decoded_ranked", weight_ps: 158000 },
    Unit { name: "worlds", weight_ps: 20000 },
    Unit { name: "probe_evals", weight_ps: 33200 },
    Unit { name: "compared_bytes", weight_ps: 45 },
];

/// The work the shared rule did, in the units of [`RULE_UNITS`].
///
/// Produced by [`Decider`] and by nothing else: the fields are private, the only constructor
/// outside this module is [`RuleOps::ZERO`] (nothing was done: what a privileged arm reports),
/// and the rule never reads its own count, so a count cannot change a decision. Taken by the
/// harness after a call ([`crate::policy::Policy::take_ops`]), never passed to a policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleOps {
    counts: [u64; R_UNITS],
}

impl RuleOps {
    /// No work.
    pub const ZERO: RuleOps = RuleOps {
        counts: [0; R_UNITS],
    };

    fn add(&mut self, unit: usize, n: u64) {
        self.counts[unit] = self.counts[unit].saturating_add(n);
    }

    /// The counts, one per unit of [`RULE_UNITS`], in the same order.
    pub fn counts(&self) -> &[u64] {
        &self.counts
    }

    /// Add `other`'s counts to these.
    pub fn accumulate(&mut self, other: &RuleOps) {
        for (a, b) in self.counts.iter_mut().zip(other.counts.iter()) {
            *a = a.saturating_add(*b);
        }
    }

    /// The counts added together. Informational: the units differ.
    pub fn total(&self) -> u64 {
        self.counts.iter().fold(0u64, |s, c| s.saturating_add(*c))
    }

    /// Modelled cost in picoseconds: the sum over units of weight times count, saturating.
    pub fn modelled_ps(&self) -> u64 {
        RULE_UNITS
            .iter()
            .zip(self.counts.iter())
            .fold(0u64, |s, (u, c)| {
                s.saturating_add(u.weight_ps.saturating_mul(*c))
            })
    }
}

impl Default for RuleOps {
    fn default() -> Self {
        Self::ZERO
    }
}

/// The quantities the rule's declared cost is a function of; see [`Decider::declared_cost`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostFeatures {
    /// Observations in the window.
    pub window: u64,
    /// Outputs the previous call decoded (at most one each from the verifier, estimator and
    /// heuristic). An output equal to the one already held is not decoded and is not here.
    pub decoded_outputs: u64,
    /// Hypotheses in those outputs.
    pub decoded_hypotheses: u64,
    /// Payload bytes the previous call compared with held outputs (`COMPARE_BYTE_PS` each).
    pub compared_bytes: u64,
    /// Hypotheses in the first available stored set.
    pub candidates: u64,
    /// Worlds of that set before narrowing.
    pub worlds: u64,
    /// Distinct sites among its hypotheses.
    pub targets: u64,
    /// Whether the rule could buy a probe from it: several hypotheses, none of them "no fault".
    pub probing: bool,
}

/// A world: a hypothesis and the two hidden parity bits. For hypotheses outside the entangled
/// pair the bits are `(false, false)` and carry nothing.
pub type World = (Hypothesis, (bool, bool));

/// What is left to spend, as far as the rule has been told. `None` means not told yet, which is
/// treated as affordable; the harness refuses what the bill cannot pay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Remaining {
    /// Probe units left.
    pub probes: Option<u64>,
    /// Probe time left, nanoseconds.
    pub time_ns: Option<u64>,
}

impl Remaining {
    /// What `bill`'s budget has left of the two resources probes use.
    pub fn of(bill: &Bill) -> Self {
        Self {
            probes: bill.budget().remaining(Resource::Probes),
            time_ns: bill.budget().remaining(Resource::Time),
        }
    }

    /// Whether `probe` fits. Unknown remainders count as fitting.
    pub fn affords(&self, probe: Probe) -> bool {
        let (units, time) = probe_units(probe.kind);
        self.probes.is_none_or(|r| units <= r) && self.time_ns.is_none_or(|r| time <= r)
    }
}

/// The probe units and probe time a probe kind is declared to cost.
pub fn probe_units(kind: ProbeKind) -> (u64, u64) {
    let mut units = (0, 0);
    for charge in probe_cost(kind) {
        match charge.resource {
            Resource::Probes => units.0 += charge.amount,
            Resource::Time => units.1 += charge.amount,
            _ => {}
        }
    }
    units
}

/// The probe results and correction effects in a window: what the arm bought.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bought {
    /// Probe results in window order.
    pub probes: Vec<(Probe, ProbeResult)>,
    /// Correction effects in window order: the site and whether it was resolved.
    pub corrections: Vec<(ServiceId, bool)>,
}

impl Bought {
    /// The probe results and corrections among `evidence`.
    pub fn from_evidence<'a>(
        evidence: impl IntoIterator<Item = &'a (Instant, Observation)>,
    ) -> Self {
        let mut bought = Bought::default();
        for (_, observation) in evidence {
            match observation {
                Observation::Probed { probe, result } => bought.probes.push((*probe, *result)),
                Observation::Correction { site, resolved } => {
                    bought.corrections.push((*site, *resolved));
                }
                _ => {}
            }
        }
        bought
    }
}

/// The drifted configuration hash to assume at `target`: the one already seen in a probe result,
/// or, when none was, the public hash plus one. The checker uses the same convention, and the
/// value only matters in that it differs from the public hash.
fn drift_hash(services: &[Service], target: ServiceId, bought: &Bought) -> Option<u64> {
    let start = services.get(target.index())?.config_hash;
    let seen = bought
        .probes
        .iter()
        .find_map(|(probe, result)| match result {
            ProbeResult::ConfigHash(h)
                if probe.kind == ProbeKind::ConfigSnapshot
                    && probe.target == target
                    && *h != start =>
            {
                Some(*h)
            }
            _ => None,
        });
    Some(seen.unwrap_or(start.wrapping_add(1)))
}

/// The hidden-bit assignments that matter for `hypothesis`, in the checker's order.
fn bit_options(hypothesis: Hypothesis) -> &'static [(bool, bool)] {
    match hypothesis {
        Some((kind, _)) if kind == ENTANGLED.0 => &[(false, false), (true, true)],
        Some((kind, _)) if kind == ENTANGLED.1 => &[(false, true), (true, false)],
        _ => &[(false, false)],
    }
}

fn probe_agrees(
    services: &[Service],
    world: World,
    probe: Probe,
    result: ProbeResult,
    bought: &Bought,
) -> bool {
    let Some(drift) = drift_hash(services, probe.target, bought) else {
        // A probe at a service that is not in the graph says nothing about these worlds.
        return true;
    };
    probe_result(services, world.0, world.1, drift, probe) == result
}

/// The worlds of `hypotheses` that every probe result and correction in `bought` allows.
///
/// This is not a consistency check of passive observations: a hypothesis a symptom contradicts
/// stays, unless a probe contradicts it too. Narrowing by symptoms is the verifier's work.
pub fn worlds_of(services: &[Service], hypotheses: &[Hypothesis], bought: &Bought) -> Vec<World> {
    worlds_of_counted(services, hypotheses, bought, &mut RuleOps::default())
}

/// [`worlds_of`], adding the work done to `ops`.
fn worlds_of_counted(
    services: &[Service],
    hypotheses: &[Hypothesis],
    bought: &Bought,
    ops: &mut RuleOps,
) -> Vec<World> {
    let mut worlds = Vec::new();
    for h in hypotheses {
        for bits in bit_options(*h) {
            ops.add(R_WORLDS, 1);
            let world = (*h, *bits);
            let probes_agree = bought
                .probes
                .iter()
                .all(|(probe, result)| probe_agrees(services, world, *probe, *result, bought));
            let corrections_agree = bought
                .corrections
                .iter()
                .all(|(site, resolved)| h.is_some_and(|(_, s)| s == *site) == *resolved);
            if probes_agree && corrections_agree {
                worlds.push(world);
            }
        }
    }
    worlds
}

/// One candidate probe and how well it is expected to shrink the candidate set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeScore {
    /// The probe.
    pub probe: Probe,
    /// `sum over results r of |worlds giving r| * |hypotheses among them|`. Divided by the
    /// number of worlds this is the expected number of hypotheses left. Kept as an integer so
    /// the comparison is exact.
    pub numerator: u64,
}

/// The distinct hypotheses of `worlds`, in order. Worlds of one hypothesis are consecutive (they
/// are built from a duplicate-free candidate set, one hypothesis at a time), so comparing with the
/// previous one is enough.
fn distinct(worlds: &[World]) -> Vec<Hypothesis> {
    let mut out: Vec<Hypothesis> = Vec::new();
    for (h, _) in worlds {
        if out.last() != Some(h) {
            out.push(*h);
        }
    }
    out
}

/// Score every probe that could tell two worlds apart.
///
/// A probe at a service that is nobody's candidate site gives the healthy answer in every world,
/// so only candidate sites are tried; when "no fault" is a candidate every service is. For each
/// probe the worlds are split by the result the public `probe_result` semantics give them. The
/// worlds that give result `r` are exactly the worlds `consistent_worlds` would keep after `r`
/// is added to the evidence, so the number of hypotheses among them is the size of the set
/// `consistent_hypotheses` would return for that outcome. A test pins this to the checker.
pub fn score_probes(services: &[Service], worlds: &[World], bought: &Bought) -> Vec<ProbeScore> {
    score_probes_counted(services, worlds, bought, &mut RuleOps::default())
}

/// [`score_probes`], adding the work done to `ops`.
fn score_probes_counted(
    services: &[Service],
    worlds: &[World],
    bought: &Bought,
    ops: &mut RuleOps,
) -> Vec<ProbeScore> {
    ops.add(R_WORLDS, worlds.len() as u64);
    let targets: BTreeSet<ServiceId> = if worlds.iter().any(|(h, _)| h.is_none()) {
        services.iter().map(|s| s.id).collect()
    } else {
        worlds
            .iter()
            .filter_map(|(h, _)| h.map(|(_, site)| site))
            .collect()
    };
    let mut out = Vec::new();
    for target in targets {
        let Some(drift) = drift_hash(services, target, bought) else {
            continue;
        };
        for kind in ProbeKind::ALL {
            let probe = Probe { kind, target };
            let mut groups: Vec<(ProbeResult, u64, Vec<Hypothesis>)> = Vec::new();
            for (h, bits) in worlds {
                let result = probe_result(services, *h, *bits, drift, probe);
                ops.add(R_PROBE_EVALS, 1);
                match groups.iter_mut().find(|g| g.0 == result) {
                    Some(group) => {
                        group.1 += 1;
                        if !group.2.contains(h) {
                            group.2.push(*h);
                        }
                    }
                    None => groups.push((result, 1, vec![*h])),
                }
            }
            let numerator = groups.iter().map(|g| g.1 * g.2.len() as u64).sum();
            out.push(ProbeScore { probe, numerator });
        }
    }
    out
}

/// What the rule holds of one component's latest output: the entries as they arrived and what
/// they decoded to. Keeping the entries is what lets the next output be recognised as the same.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    entries: Vec<(EntryKind, Vec<u8>)>,
    /// What [`offered`] made of `entries`.
    set: Option<Vec<Hypothesis>>,
}

/// Payload bytes a comparison of `arriving` with `held` has to look at: all of `arriving`'s when
/// both have the same entries' kinds and lengths (so `==` must reach the bytes), none otherwise
/// (it stops at a length). Counting only: nothing reads it but the rule's count and its declared
/// cost, and it never decides whether an output is the same.
fn compared_bytes(held: &[(EntryKind, Vec<u8>)], arriving: &[(EntryKind, Vec<u8>)]) -> u64 {
    let same_shape = held.len() == arriving.len()
        && held
            .iter()
            .zip(arriving)
            .all(|(a, b)| a.0 == b.0 && a.1.len() == b.1.len());
    if same_shape {
        arriving.iter().map(|(_, bytes)| bytes.len() as u64).sum()
    } else {
        0
    }
}

/// The candidate set the rule acts on, and the worlds behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct View {
    hypotheses: Vec<Hypothesis>,
    worlds: Vec<World>,
}

/// The hypotheses a component output offers: the leading tie of its first candidate list.
///
/// For the verifier that is the whole consistent set, for the heuristic all its candidates, and
/// for the estimator the hypotheses sharing its best score (as far as its entry lists them).
/// `None` when the output holds no candidates or says the window is damaged.
fn offered(output: &ComponentOutput, ops: &mut RuleOps) -> Option<Vec<Hypothesis>> {
    output.entries.iter().find_map(|(_, bytes)| {
        ops.add(R_DECODED_OUTPUTS, 1);
        let HypothesisEntry::Candidates { ranked, .. } = decode(bytes).ok()? else {
            return None;
        };
        ops.add(R_DECODED_RANKED, ranked.len() as u64);
        let top = ranked.first()?.score;
        let set: Vec<Hypothesis> = ranked
            .iter()
            .take_while(|r| r.score == top)
            .map(|r| r.hypothesis)
            .collect();
        Some(set)
    })
}

/// The shared decision rule, with the state it keeps between steps.
#[derive(Debug, Clone)]
pub struct Decider {
    patience: Instant,
    verifier: Option<Held>,
    estimator: Option<Held>,
    heuristic: Option<Held>,
    remaining: Remaining,
    /// Whether an arriving output equal to the held one is recognised and not decoded again
    /// (work item A6c). False only for [`Decider::without_reuse`], the reference.
    reuse: bool,
    /// Outputs and hypotheses the last `decide` call decoded, and the bytes it compared: the work
    /// whose cost the next step's declared cost carries.
    decoded: (u64, u64, u64),
    /// Work done since the harness last took the count ([`Decider::take_ops`]). Written by the
    /// rule's own steps and read by nothing in them.
    ops: RuleOps,
}

impl Default for Decider {
    fn default() -> Self {
        Self::new(DecideConfig::default())
    }
}

impl Decider {
    /// A rule that has seen no output yet.
    pub fn new(config: DecideConfig) -> Self {
        Self {
            patience: Instant(config.patience_ns),
            verifier: None,
            estimator: None,
            heuristic: None,
            remaining: Remaining::default(),
            reuse: true,
            decoded: (0, 0, 0),
            ops: RuleOps::ZERO,
        }
    }

    /// The reference rule: the same rule that decodes every output it is shown, whether or not it
    /// equals the one it holds. This is the rule as it was before work item A6c. No arm uses it;
    /// it is the oracle the tests hold [`Decider::new`] to (every decision equal, on the same
    /// inputs), as the plan asks for a simple reference beside anything optimised on the hot path.
    pub fn without_reuse(config: DecideConfig) -> Self {
        Self {
            reuse: false,
            ..Self::new(config)
        }
    }

    /// The patience deadline.
    pub fn patience(&self) -> Instant {
        self.patience
    }

    /// Tell the rule what is left to spend. Called by [`crate::policy::Arm`] from `select`, the
    /// one place a policy sees the bill, the same way for every arm.
    pub fn note_remaining(&mut self, remaining: Remaining) {
        self.remaining = remaining;
    }

    /// The work done since the last call of this method, which resets the count. The harness
    /// calls it after each timed scheduling call and sums what it returns; nothing in the rule
    /// reads the count.
    pub fn take_ops(&mut self) -> RuleOps {
        std::mem::take(&mut self.ops)
    }

    /// Take an output that arrived: the component ran this step and produced it.
    ///
    /// An output whose entries equal, byte for byte, the ones held for the component decodes to
    /// what the held ones did, so the held decoded form stays and nothing is decoded (work item
    /// A6c). Otherwise it is decoded and replaces the held one. `==` on the entries is the whole
    /// test of "the same": the rule assumes nothing about how a component produces its output.
    fn absorb(&mut self, id: ComponentId, output: &ComponentOutput, ops: &mut RuleOps) {
        let slot = if id == VERIFIER_ID {
            &mut self.verifier
        } else if id == ESTIMATOR_ID {
            &mut self.estimator
        } else if id == HEURISTIC_ID {
            &mut self.heuristic
        } else {
            return;
        };
        if self.reuse
            && let Some(held) = slot.as_ref()
        {
            let bytes = compared_bytes(&held.entries, &output.entries);
            ops.add(R_COMPARED_BYTES, bytes);
            self.decoded.2 += bytes;
            if held.entries == output.entries {
                return;
            }
        }
        let set = offered(output, ops);
        if !output.entries.is_empty() {
            self.decoded.0 += 1;
            self.decoded.1 += set.as_ref().map_or(0, |set| set.len() as u64);
        }
        *slot = Some(Held {
            entries: output.entries.clone(),
            set,
        });
    }

    fn sources(&self) -> impl Iterator<Item = &Vec<Hypothesis>> {
        [&self.verifier, &self.estimator, &self.heuristic]
            .into_iter()
            .filter_map(|held| held.as_ref()?.set.as_ref())
    }

    fn view(&self, state: &WorkingState, bought: &Bought, ops: &mut RuleOps) -> Option<View> {
        self.sources().find_map(|set| {
            let worlds = worlds_of_counted(&state.public.services, set, bought, ops);
            if worlds.is_empty() {
                None
            } else {
                Some(View {
                    hypotheses: distinct(&worlds),
                    worlds,
                })
            }
        })
    }

    /// The quantities the declared cost is a function of.
    pub fn cost_features(&self, state: &WorkingState) -> CostFeatures {
        let mut features = CostFeatures {
            window: state.size() as u64,
            decoded_outputs: self.decoded.0,
            decoded_hypotheses: self.decoded.1,
            compared_bytes: self.decoded.2,
            candidates: 0,
            worlds: 0,
            targets: 0,
            probing: false,
        };
        if let Some(set) = self.sources().next() {
            features.candidates = set.len() as u64;
            features.worlds = set.iter().map(|h| bit_options(*h).len() as u64).sum();
            features.probing = set.len() > 1 && !set.contains(&None);
            features.targets = set
                .iter()
                .filter_map(|h| h.map(|(_, s)| s))
                .collect::<BTreeSet<_>>()
                .len() as u64;
        }
        features
    }

    /// The declared cost of one `decide` call at `state`, in `Resource::Compute` nanoseconds.
    ///
    /// A function of [`CostFeatures`] only: a base; a scan of the window for bought probes; the
    /// narrowing of the candidate set's worlds; when a probe could be chosen, the evaluation of
    /// every candidate probe against every world; and the decoding of the outputs the *previous*
    /// call received. That last term is a lag, not a guess: which outputs a step brings is not
    /// known before `select`, but the decoding work of every step is charged at the next one, so
    /// the whole episode's decoding is charged except the last step's, which the final call
    /// ([`Decider::declared_final_cost`]) picks up when the harness makes one. The narrowing and
    /// scoring terms are upper bounds, because narrowing by bought probes only removes worlds. Constants
    /// and their fit: `POLICIES.md`, section 3.
    pub fn declared_cost(&self, state: &WorkingState) -> Charge {
        self.cost(state, true)
    }

    /// The declared cost of the final call ([`Decider::decide_final`]) at `state`.
    ///
    /// [`Decider::declared_cost`] without the probe-evaluation term: the final call declares or
    /// abstains and never scores a probe, so charging for the evaluation would bill work the call
    /// does not do. It still carries the decoding of the previous call's outputs, because the
    /// final call is the next call after the last step, so the whole episode's decoding is
    /// charged.
    pub fn declared_final_cost(&self, state: &WorkingState) -> Charge {
        self.cost(state, false)
    }

    fn cost(&self, state: &WorkingState, may_probe: bool) -> Charge {
        let f = self.cost_features(state);
        let mut ps = BASE_PS
            .saturating_add(SCAN_PS.saturating_mul(f.window))
            .saturating_add(WORLD_PS.saturating_mul(f.worlds))
            .saturating_add(DECODE_OUTPUT_PS.saturating_mul(f.decoded_outputs))
            .saturating_add(DECODE_HYPOTHESIS_PS.saturating_mul(f.decoded_hypotheses))
            .saturating_add(COMPARE_BYTE_PS.saturating_mul(f.compared_bytes));
        if may_probe && f.probing {
            let evaluations = ProbeKind::ALL.len() as u64 * f.targets * f.worlds;
            ps = ps.saturating_add(EVAL_PS.saturating_mul(evaluations));
        }
        Charge::new(Resource::Compute, ps.div_ceil(1000))
    }

    /// The rule. `outputs` holds the outputs of the components that ran this step.
    pub fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        self.decide_at(state, outputs, false)
    }

    /// The final call: the rule as at the patience deadline, whatever the clock says. No
    /// component ran, so there are no outputs; the rule acts on the last it was shown.
    ///
    /// The one difference from [`Decider::decide`] is that the deadline counts as passed. That is
    /// the whole of it, so with a single hypothesis left it declares it, with several it declares
    /// the first-ranked, and with none it abstains. It never buys a probe: at the deadline the
    /// rule does not either.
    pub fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        self.decide_at(state, &[], true)
    }

    fn decide_at(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
        last: bool,
    ) -> Option<Action> {
        let mut ops = RuleOps::ZERO;
        ops.add(R_CALLS, 1);
        let action = self.decide_counted(state, outputs, last, &mut ops);
        self.ops.accumulate(&ops);
        action
    }

    fn decide_counted(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
        last: bool,
        ops: &mut RuleOps,
    ) -> Option<Action> {
        self.decoded = (0, 0, 0);
        for (id, output) in outputs {
            self.absorb(*id, output, ops);
        }
        let bought = Bought::from_evidence(state.evidence());
        let view = self.view(state, &bought, ops);
        // The final call is the only thing `last` changes: it makes the deadline due.
        let due = last || state.now >= self.patience;

        let Some(view) = view else {
            return due.then_some(Action::Abstain);
        };
        if let [only] = view.hypotheses.as_slice() {
            return match only {
                Some(fault) => Some(Action::Declare {
                    fault: Some(*fault),
                }),
                None => due.then_some(Action::Declare { fault: None }),
            };
        }
        if due {
            return Some(Action::Declare {
                fault: view.hypotheses[0],
            });
        }
        if view.hypotheses.contains(&None) {
            // Silence is what every fault looks like before its onset; wait for evidence.
            return None;
        }
        self.best_probe(state, &view, &bought, ops)
            .map(|probe| Action::Probe {
                kind: probe.kind,
                target: probe.target,
            })
    }

    /// The affordable probe with the smallest expected remaining set, if it beats the present
    /// size. Ties go to the cheaper probe (units, then time), then to the earlier kind and the
    /// lower service id.
    fn best_probe(
        &self,
        state: &WorkingState,
        view: &View,
        bought: &Bought,
        ops: &mut RuleOps,
    ) -> Option<Probe> {
        let bound = view.hypotheses.len() as u64 * view.worlds.len() as u64;
        score_probes_counted(&state.public.services, &view.worlds, bought, ops)
            .into_iter()
            .filter(|s| s.numerator < bound && self.remaining.affords(s.probe))
            .min_by_key(|s| {
                let (units, time) = probe_units(s.probe.kind);
                (
                    s.numerator,
                    units,
                    time,
                    s.probe.kind as u8,
                    s.probe.target.0,
                )
            })
            .map(|s| s.probe)
    }
}
