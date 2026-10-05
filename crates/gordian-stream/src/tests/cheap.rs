//! A driver that runs the first world's four components and the shared decision rule on a stream
//! incident, the way the first world's harness runs them on an episode.
//!
//! This is the *cheap rung* of the tests: nothing here is a new policy. The components are
//! `gordian_run::standard_components()`, the rule is `gordian_run::policy::decide::Decider`, and
//! every probe goes through the stream simulator's own probe path. The only choices made here are
//! the ones a harness makes: which observations the working state holds, the clock, and the
//! probe budget (the first world's default, 12 probes and 250 ms of probe time).

use super::*;
use crate::{StreamAction, StreamOutcome, StreamSimulator};
use gordian_components::WorkingState;
use gordian_run::policy::decide::{DecideConfig, Decider, Remaining};
use gordian_run::standard_components;
use gordian_world::episode::PublicInfo;
use gordian_world::physics::{consistent_hypotheses, probe_cost};
use gordian_world::{Action, Hypothesis, Observation, Probe};
use std::collections::BTreeSet;

/// What the cheap rung did with one incident's evidence.
#[derive(Debug, Clone)]
pub(crate) struct CheapRun {
    /// `Some(h)` when the rule declared `h` (including "no fault"); `None` when it abstained or
    /// never decided.
    pub(crate) declared: Option<Hypothesis>,
    /// Probes bought.
    pub(crate) probes: u32,
    /// Everything the working state held at the end, probes included.
    pub(crate) evidence: Evidence,
}

impl CheapRun {
    /// The hypotheses the public rules leave open on everything the run saw.
    pub(crate) fn consistent(&self, public: &PublicInfo) -> Vec<Hypothesis> {
        consistent_hypotheses(public, &self.evidence)
    }
}

/// Run the cheap rung on `evidence`, whose first observation is taken as time zero. Instants in
/// the returned evidence are relative to it, as the first world's episodes start at zero and the
/// shared rule's patience is measured from there.
///
/// `patience_ns` is the shared rule's deadline for declaring. `sim` answers the probes the rule
/// buys, at the instant `base + t`, where `base` is the absolute instant of `evidence[0]`; it
/// must have a probe budget of its own large enough for the run.
pub(crate) fn run(
    public: &PublicInfo,
    evidence: &Evidence,
    patience_ns: u64,
    sim: &mut StreamSimulator,
) -> CheapRun {
    let base = evidence[0].0;
    let rel = |t: gordian_core::Instant| t.0 - base.0;
    let mut state = WorkingState::new(public.clone(), 8192);
    let mut decider = Decider::new(DecideConfig { patience_ns });
    let mut components = standard_components();
    let (mut probes_left, mut time_left) = (12u64, 250_000_000u64);
    decider.note_remaining(Remaining {
        probes: Some(probes_left),
        time_ns: Some(time_left),
    });

    // The harness looks every half second, starting after the burst (which lasts under 250 ms).
    // Looking at every observation's instant instead would show the rule half-built signatures
    // and make it buy probes the first world's 50 ms steps would also have bought on a partial
    // window; that is a cost of watching too closely, not a question of identifiability.
    const STEP: u64 = 500_000_000;
    let mut agenda: BTreeSet<u64> = BTreeSet::new();
    let mut tick = STEP;
    while tick <= patience_ns + 2_000_000_000 {
        agenda.insert(tick);
        tick += STEP;
    }
    let mut next = 0usize;
    let mut pending: Vec<(u64, Observation)> = Vec::new();
    let mut held: Evidence = Vec::new();
    let mut probes = 0u32;
    let mut declared: Option<Hypothesis> = None;
    let mut decided = false;

    while let Some(t) = agenda.pop_first() {
        // Admit everything that has happened by `t`, in time order.
        loop {
            let ev_t = evidence.get(next).map(|(at, _)| rel(*at));
            let pe_t = pending.iter().map(|(at, _)| *at).min();
            let take_evidence = match (ev_t, pe_t) {
                (Some(a), Some(b)) => a <= b && a <= t,
                (Some(a), None) => a <= t,
                _ => false,
            };
            if take_evidence {
                let (at, o) = &evidence[next];
                let at = gordian_core::Instant(rel(*at));
                state.admit(at, o.clone());
                held.push((at, o.clone()));
                next += 1;
                continue;
            }
            if let Some(pt) = pe_t
                && pt <= t
            {
                let i = pending.iter().position(|(at, _)| *at == pt).unwrap();
                let (at, o) = pending.remove(i);
                let at = gordian_core::Instant(at);
                state.admit(at, o.clone());
                held.push((at, o));
                continue;
            }
            break;
        }
        state.now = state.now.max(gordian_core::Instant(t));
        let outputs: Vec<_> = components
            .iter_mut()
            .map(|c| (c.id(), c.run(&state)))
            .collect();
        let decision = decider.decide(&state, &outputs);
        if std::env::var("CHEAP_DEBUG").is_ok() {
            eprintln!("t={t} window={} -> {decision:?}", state.size());
        }
        match decision {
            None => {}
            Some(Action::Declare { fault }) => {
                declared = Some(fault);
                decided = true;
                break;
            }
            Some(Action::Abstain) => {
                decided = true;
                break;
            }
            Some(Action::Probe { kind, target }) => {
                let probe = Probe { kind, target };
                let cost = probe_cost(kind);
                let (units, time) = cost.iter().fold((0, 0), |(u, ti), c| match c.resource {
                    gordian_core::Resource::Probes => (u + c.amount, ti),
                    gordian_core::Resource::Time => (u, ti + c.amount),
                    _ => (u, ti),
                });
                if units > probes_left || time > time_left {
                    continue;
                }
                probes_left -= units;
                time_left -= time;
                decider.note_remaining(Remaining {
                    probes: Some(probes_left),
                    time_ns: Some(time_left),
                });
                let at = gordian_core::Instant(base.0 + t);
                let outcome = sim.apply(
                    StreamAction::Probe {
                        kind: probe.kind,
                        target: probe.target,
                    },
                    at,
                );
                let StreamOutcome::Probed { observation, .. } = outcome else {
                    panic!("probe refused: {outcome:?}");
                };
                probes += 1;
                let ready = t + time;
                pending.push((ready, observation));
                agenda.insert(ready);
            }
            Some(Action::Correct { .. }) => {}
        }
    }
    if !decided && let Some(Action::Declare { fault }) = decider.decide_final(&state) {
        declared = Some(fault);
    }
    CheapRun {
        declared,
        probes,
        evidence: held,
    }
}

/// A simulator with probe budget to spare, for the cheap rung to buy probes from. Probes are
/// about the stream and not about what has been observed, so nothing is delivered; callers pass
/// the instants and clone this for every run, because instants must not go backwards.
pub(crate) fn probing_sim(params: &StreamParams) -> StreamSimulator {
    let mut p = params.clone();
    p.budget.probes = 1_000_000;
    p.budget.probe_time_ns = u64::MAX / 4;
    StreamSimulator::new(generate(&p))
}

/// The result of every probe kind at every service, as the simulator answers at `at`.
pub(crate) fn exhaustive_probes(
    sim: &mut StreamSimulator,
    n_services: usize,
    at: gordian_core::Instant,
) -> Evidence {
    let mut out = Vec::new();
    for s in 0..n_services {
        for kind in gordian_world::ProbeKind::ALL {
            let target = gordian_world::ServiceId(s as u32);
            let StreamOutcome::Probed { observation, .. } =
                sim.apply(StreamAction::Probe { kind, target }, at)
            else {
                panic!("probe refused")
            };
            out.push((at, observation));
        }
    }
    out
}
