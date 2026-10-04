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
//! 1. The rule stores the latest output of the verifier, the estimator and the heuristic. A
//!    component that was not selected leaves its previous output in place, so an arm that skips a
//!    component is acting on stale output and pays for it in decisions.
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
//! The expected size is over *hypotheses* (the size of what `consistent_hypotheses` returns),
//! under a uniform prior over the consistent *worlds* (hypothesis plus the two hidden parity
//! bits). A probe that removes worlds but no hypothesis therefore scores no gain, which is why
//! this rule does not buy the first of a jointly decisive pair of samples.
//!
//! The rule is a pure function of the working state, the outputs it has been shown, and the
//! remaining limits it was told: no clock, no randomness, no I/O.

use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{ComponentOutput, ESTIMATOR_ID, HEURISTIC_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Bill, Charge, ComponentId, Instant, Resource};
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

// Declared cost of one call, `Resource::Compute` nanoseconds. See `POLICIES.md`, section 3, for
// how these were measured and what they do not cover.
const BASE_NS: u64 = 400;
const SCAN_PS_PER_OBSERVATION: u64 = 2_000;
const EVAL_NS_PER_WORLD_PROBE: u64 = 30;

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
    let mut worlds = Vec::new();
    for h in hypotheses {
        for bits in bit_options(*h) {
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

/// The distinct hypotheses of `worlds`, in order of first appearance.
fn distinct(worlds: &[World]) -> Vec<Hypothesis> {
    let mut out: Vec<Hypothesis> = Vec::new();
    for (h, _) in worlds {
        if !out.contains(h) {
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
fn offered(output: &ComponentOutput) -> Option<Vec<Hypothesis>> {
    output.entries.iter().find_map(|(_, bytes)| {
        let HypothesisEntry::Candidates { ranked, .. } = decode(bytes).ok()? else {
            return None;
        };
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
    verifier: Option<Vec<Hypothesis>>,
    estimator: Option<Vec<Hypothesis>>,
    heuristic: Option<Vec<Hypothesis>>,
    remaining: Remaining,
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

    fn absorb(&mut self, id: ComponentId, output: &ComponentOutput) {
        let slot = if id == VERIFIER_ID {
            &mut self.verifier
        } else if id == ESTIMATOR_ID {
            &mut self.estimator
        } else if id == HEURISTIC_ID {
            &mut self.heuristic
        } else {
            return;
        };
        *slot = offered(output);
    }

    fn sources(&self) -> impl Iterator<Item = &Vec<Hypothesis>> {
        [&self.verifier, &self.estimator, &self.heuristic]
            .into_iter()
            .flatten()
    }

    fn view(&self, state: &WorkingState, bought: &Bought) -> Option<View> {
        self.sources().find_map(|set| {
            let worlds = worlds_of(&state.public.services, set, bought);
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

    /// The declared cost of one `decide` call at `state`, in `Resource::Compute` nanoseconds.
    ///
    /// A function of the window size and the size of the stored candidate set only: a base, a
    /// scan of the window for bought probes, and, when a probe could be chosen, an evaluation of
    /// every candidate probe against every world. It is an upper bound on the work, because the
    /// narrowing by bought probes only removes worlds.
    pub fn declared_cost(&self, state: &WorkingState) -> Charge {
        let mut ns = BASE_NS
            .saturating_add(SCAN_PS_PER_OBSERVATION.saturating_mul(state.size() as u64) / 1000);
        if let Some(set) = self.sources().next()
            && set.len() > 1
            && !set.contains(&None)
        {
            let worlds: u64 = set.iter().map(|h| bit_options(*h).len() as u64).sum();
            let targets = set
                .iter()
                .filter_map(|h| h.map(|(_, s)| s))
                .collect::<BTreeSet<_>>()
                .len() as u64;
            let evaluations = ProbeKind::ALL.len() as u64 * targets * worlds;
            ns = ns.saturating_add(EVAL_NS_PER_WORLD_PROBE.saturating_mul(evaluations));
        }
        Charge::new(Resource::Compute, ns)
    }

    /// The rule. `outputs` holds the outputs of the components that ran this step.
    pub fn decide(
        &mut self,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
    ) -> Option<Action> {
        for (id, output) in outputs {
            self.absorb(*id, output);
        }
        let bought = Bought::from_evidence(state.evidence());
        let view = self.view(state, &bought);
        let due = state.now >= self.patience;

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
        self.best_probe(state, &view, &bought)
            .map(|probe| Action::Probe {
                kind: probe.kind,
                target: probe.target,
            })
    }

    /// The affordable probe with the smallest expected remaining set, if it beats the present
    /// size. Ties go to the cheaper probe (units, then time), then to the earlier kind and the
    /// lower service id.
    fn best_probe(&self, state: &WorkingState, view: &View, bought: &Bought) -> Option<Probe> {
        let bound = view.hypotheses.len() as u64 * view.worlds.len() as u64;
        score_probes(&state.public.services, &view.worlds, bought)
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
