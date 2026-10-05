//! The privileged arms: `oracle_immediate` and `oracle_evidence`.
//!
//! These are the charter's "small-world oracle (privileged)" baseline: whether the environment
//! has measurable headroom. They are the only policies in the crate that use the episode's truth,
//! and this is the only file that names it (`scripts/check-no-oracle.sh` allows exactly this
//! path).
//!
//! # How the truth reaches them, and only them
//!
//! The [`super::Policy`] trait has no place for a truth, and no policy outside this file has a
//! constructor that takes one. The harness holds the episode's truth for scoring; to build a
//! privileged arm it calls [`OracleFactory::build`] through
//! `harness::run_episode_privileged`, the one entry point that takes a factory instead of a
//! policy. [`OracleFactory`] has private fields and one constructor, so it is the only thing
//! that entry point accepts; `OraclePolicy`, the type it builds, has no public constructor. A
//! test checks that no other file in `policy/` names the truth, and `Built` in `policy/mod.rs`
//! is how the registry tells the recorder which entry point an arm needs.
//!
//! Every output of these arms must say so: `Manifest::validate` rejects an oracle arm whose
//! `arm` does not contain `privileged`.
//!
//! # The two arms
//!
//! `oracle_immediate` declares the true hypothesis at the first step, before any observation. It
//! is the ceiling: what a perfect answer scores at no cost.
//!
//! `oracle_evidence` is an ideal observer. It uses the truth only to choose probes; it declares
//! only when the *public* evidence in its working state, read with the world's public rules
//! (`consistent_worlds`), leaves exactly the true hypothesis. At each step it computes the fewest
//! probes that would make that so, buys the first of them, and re-plans when the result arrives.
//! The plan has to work whatever the two hidden parity bits are, because the truth carries the
//! hypothesis and not the bits (the evaluator's `Truth` has no bits): each remaining rival world
//! must be told apart from every bit assignment of the true hypothesis that the evidence still
//! allows. With no plan that fits the remaining limits it waits, and at the patience deadline it
//! abstains. It never declares a hypothesis the public evidence has not identified, so on a
//! `NoFault` episode it can declare "no fault" only after probes have excluded every fault.
//!
//! The planner is exact for the minimum number of probes: iterative deepening over a hitting-set
//! formulation, branching on the smallest unhit constraint, with a node cap. If the cap is
//! reached it plans nothing, which the arm treats as "no plan" rather than as a claim of
//! minimality. Among plans of equal length the first found is used, cheaper probes first.
//!
//! # The final call
//!
//! When the harness has found that no affordable work is left, or that the horizon was reached, it
//! calls [`Policy::decide_final`] once more (`HARNESS.md`, section 1). An oracle treats that as its
//! own deadline arriving, because that is what it is:
//!
//! - `oracle_immediate` declares the truth, as it does at every call. It has declared at the first
//!   step, so the harness never reaches a final call for it; the answer exists so that the arm is
//!   defined there.
//! - `oracle_evidence` declares the truth if the public evidence identifies it, and otherwise
//!   abstains, exactly as at the patience deadline. It does not buy a probe (nothing could follow),
//!   and it does not declare the truth from the hidden state: an ideal observer that did would stop
//!   being one precisely when it matters. A final call that makes it abstain is therefore a
//!   statement that the evidence it could afford did not identify the fault.
//!
//! Neither oracle changes at default limits: both always decided before the first final call
//! (`tests/baselines.rs` pins this).
//!
//! These arms are free: `zero_cost` is declared for their scheduling, because they are a ceiling
//! and not a cost-bearing mechanism. They select no components.

use super::decide::{DecideConfig, Remaining, probe_units};
use super::{Policy, PolicyId, zero_cost};
use gordian_components::{ComponentOutput, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Instant};
use gordian_eval::Truth;
use gordian_world::physics::{consistent_worlds, probe_result};
use gordian_world::{Action, Hypothesis, Probe, ProbeKind, Service};

/// The id of `oracle_immediate`.
pub const ID_IMMEDIATE: &str = "oracle_immediate";
/// The id of `oracle_evidence`.
pub const ID_EVIDENCE: &str = "oracle_evidence";

/// Most probes the planner will consider: 12 services times 6 kinds fits a `u128` mask.
const MAX_PROBES: usize = 128;
/// Search nodes per depth before the planner gives up.
const NODE_CAP: u32 = 400_000;

/// Which privileged arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Declare the truth at the first step.
    Immediate,
    /// Declare when the public evidence identifies the truth, buying the shortest identifying
    /// probe sequence.
    Evidence,
}

/// Builds a privileged arm for an episode once the harness hands over its truth.
///
/// The only way to build one of these policies. Fields are private and the constructor takes no
/// truth, so a factory can be made anywhere but can only be used by the harness, which holds the
/// truth. The policy type it builds is private to this file:
///
/// ```compile_fail
/// // `OraclePolicy` has no public name, so nothing else can construct one from a truth.
/// use gordian_run::policy::privileged::OraclePolicy;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OracleFactory {
    variant: Variant,
    patience: Instant,
}

impl OracleFactory {
    /// A factory for `variant`, whose evidence arm gives up and abstains at `decide`'s patience.
    pub fn new(variant: Variant, decide: DecideConfig) -> Self {
        Self {
            variant,
            patience: Instant(decide.patience_ns),
        }
    }

    /// The policy for the episode whose truth is `truth`. Called by the harness.
    pub fn build(&self, truth: &Truth) -> Box<dyn Policy> {
        // The generator makes at most one fault; the first is the hypothesis.
        let hypothesis = truth.faults.first().map(|f| (f.kind, f.site));
        Box::new(OraclePolicy {
            variant: self.variant,
            truth: hypothesis,
            patience: self.patience,
            remaining: Remaining::default(),
        })
    }

    /// Which arm this builds.
    pub fn variant(&self) -> Variant {
        self.variant
    }
}

/// The privileged policy. Has no public constructor.
struct OraclePolicy {
    variant: Variant,
    truth: Hypothesis,
    patience: Instant,
    remaining: Remaining,
}

impl OraclePolicy {
    /// `last` is the harness's final call: the deadline counts as passed and no probe is bought.
    fn decide_evidence(&self, state: &WorkingState, last: bool) -> Option<Action> {
        let evidence: Vec<_> = state.evidence().iter().cloned().collect();
        let worlds = consistent_worlds(&state.public, &evidence);
        let mut hypotheses: Vec<Hypothesis> = Vec::new();
        for (h, _) in &worlds {
            if !hypotheses.contains(h) {
                hypotheses.push(*h);
            }
        }
        if hypotheses == [self.truth] {
            return Some(Action::Declare { fault: self.truth });
        }
        if !last && let Some(probe) = self.next_probe(&state.public.services, &worlds) {
            return Some(Action::Probe {
                kind: probe.kind,
                target: probe.target,
            });
        }
        (last || state.now >= self.patience).then_some(Action::Abstain)
    }

    /// The first probe of the shortest plan that identifies the truth, if one fits.
    fn next_probe(
        &self,
        services: &[Service],
        worlds: &[(Hypothesis, (bool, bool))],
    ) -> Option<Probe> {
        let truth_worlds: Vec<_> = worlds.iter().filter(|(h, _)| *h == self.truth).collect();
        if truth_worlds.is_empty() {
            return None;
        }
        let probes: Vec<Probe> = services
            .iter()
            .flat_map(|s| {
                ProbeKind::ALL
                    .into_iter()
                    .map(|kind| Probe { kind, target: s.id })
            })
            .filter(|p| self.remaining.affords(*p))
            .take(MAX_PROBES)
            .collect();
        let result_of = |world: &(Hypothesis, (bool, bool)), probe: Probe| {
            let start = services[probe.target.index()].config_hash;
            probe_result(services, world.0, world.1, start.wrapping_add(1), probe)
        };

        // One constraint per (rival world, true bit assignment): the probes that give the two
        // different results. The plan must hit every one.
        let mut sets: Vec<u128> = Vec::new();
        for rival in worlds.iter().filter(|(h, _)| *h != self.truth) {
            for truth_world in &truth_worlds {
                let mut mask = 0u128;
                for (i, probe) in probes.iter().enumerate() {
                    if result_of(rival, *probe) != result_of(truth_world, *probe) {
                        mask |= 1u128 << i;
                    }
                }
                if mask == 0 {
                    // Nothing available tells this rival from the truth.
                    return None;
                }
                sets.push(mask);
            }
        }
        sets.sort_unstable();
        sets.dedup();
        let snapshot = sets.clone();
        sets.retain(|s| !snapshot.iter().any(|t| t != s && t & s == *t));

        let costs: Vec<(u64, u64)> = probes.iter().map(|p| probe_units(p.kind)).collect();
        let budget = (
            self.remaining.probes.unwrap_or(u64::MAX),
            self.remaining.time_ns.unwrap_or(u64::MAX),
        );
        for depth in 1..=probes.len().min(12) as u32 {
            let mut nodes = 0;
            match hit(&sets, &costs, 0, (0, 0), depth, budget, &mut nodes) {
                Some(plan) => {
                    return (0..probes.len())
                        .filter(|i| plan & (1u128 << i) != 0)
                        .min_by_key(|i| (costs[*i], *i))
                        .map(|i| probes[i]);
                }
                None if nodes > NODE_CAP => return None,
                None => {}
            }
        }
        None
    }
}

/// A set of at most `depth` more probes, added to `chosen`, that meets every set in `sets` and
/// fits `budget`. Branches on the smallest set not yet met, so the search is complete for the
/// depth. `None` when there is none, or when `NODE_CAP` nodes were spent.
fn hit(
    sets: &[u128],
    costs: &[(u64, u64)],
    chosen: u128,
    spent: (u64, u64),
    depth: u32,
    budget: (u64, u64),
    nodes: &mut u32,
) -> Option<u128> {
    *nodes += 1;
    if *nodes > NODE_CAP {
        return None;
    }
    let Some(set) = sets
        .iter()
        .filter(|s| **s & chosen == 0)
        .min_by_key(|s| s.count_ones())
    else {
        return Some(chosen);
    };
    if depth == 0 {
        return None;
    }
    let mut options: Vec<usize> = (0..128).filter(|i| set & (1u128 << i) != 0).collect();
    options.sort_by_key(|i| (costs[*i], *i));
    for i in options {
        let spent = (spent.0 + costs[i].0, spent.1 + costs[i].1);
        if spent.0 > budget.0 || spent.1 > budget.1 {
            continue;
        }
        if let Some(plan) = hit(
            sets,
            costs,
            chosen | (1u128 << i),
            spent,
            depth - 1,
            budget,
            nodes,
        ) {
            return Some(plan);
        }
    }
    None
}

impl Policy for OraclePolicy {
    fn id(&self) -> PolicyId {
        PolicyId::new(match self.variant {
            Variant::Immediate => ID_IMMEDIATE,
            Variant::Evidence => ID_EVIDENCE,
        })
    }

    fn decision_rule(&self) -> &'static str {
        match self.variant {
            Variant::Immediate => "privileged: declare the truth",
            Variant::Evidence => "privileged: ideal observer",
        }
    }

    fn declared_select_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn select(&mut self, _state: &WorkingState, bill: &Bill) -> Vec<ComponentId> {
        self.remaining = Remaining::of(bill);
        Vec::new()
    }

    fn decide(
        &mut self,
        state: &WorkingState,
        _outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        match self.variant {
            Variant::Immediate => Some(Action::Declare { fault: self.truth }),
            Variant::Evidence => self.decide_evidence(state, false),
        }
    }

    fn declared_final_cost(&self, _state: &WorkingState) -> Vec<Charge> {
        zero_cost()
    }

    fn decide_final(&mut self, state: &WorkingState) -> Option<Action> {
        match self.variant {
            Variant::Immediate => Some(Action::Declare { fault: self.truth }),
            Variant::Evidence => self.decide_evidence(state, true),
        }
    }
}
