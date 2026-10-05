//! The stream world's cheap-rung tests: the first world's four components and the shared decision
//! rule, run on this world's incidents.
//!
//! These tests lived in `gordian-stream` (`src/tests/tiers.rs`, `src/tests/cheap.rs` and
//! `src/tests/reasoner.rs`) while that crate dev-depended on `gordian-run` for `Decider` and
//! `standard_components`. Work item R3 made `gordian-run` depend on `gordian-stream`, so they moved
//! here and the dev-dependency cycle is gone. The bodies, thresholds and assertions are the ones
//! they had; what changed is plumbing only: `crate::` became `gordian_stream::`, the hidden truth
//! comes from `gordian_stream_reveal::truth_of` (the crate that switches on the stream's
//! hidden-state feature), and the helpers the tests shared with the rest of that crate's tests
//! (`with_truth`, `evidence_of`, `with_mix`, `no_regime`) are repeated below.

mod cheap {
    //! A driver that runs the first world's four components and the shared decision rule on a
    //! stream incident, the way the first world's harness runs them on an episode.
    //!
    //! This is the *cheap rung* of the tests: nothing here is a new policy. The components are
    //! `gordian_run::standard_components()`, the rule is `gordian_run::policy::decide::Decider`,
    //! and every probe goes through the stream simulator's own probe path. The only choices made
    //! here are the ones a harness makes: which observations the working state holds, the clock,
    //! and the probe budget (the first world's default, 12 probes and 250 ms of probe time).

    use super::*;
    use gordian_components::WorkingState;
    use gordian_run::policy::decide::{DecideConfig, Decider, Remaining};
    use gordian_run::standard_components;
    use gordian_stream::{StreamAction, StreamOutcome, StreamSimulator};
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
}

use cheap::{exhaustive_probes, probing_sim};
use gordian_core::Instant;
use gordian_stream::{HardKind, ObsId, Stream, StreamKind, StreamParams, Tier, generate};
use gordian_stream_reveal::{IncidentTruth, StreamTruth, truth_of};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{FaultKind, Hypothesis, Observation, ServiceId};

type Evidence = Vec<(Instant, Observation)>;

/// A stream and its truth.
fn with_truth(params: &StreamParams) -> (Stream, StreamTruth) {
    let s = generate(params);
    let t = truth_of(&s);
    (s, t)
}

/// Every observation labelled with incident `id`, in stream order.
fn evidence_of(s: &Stream, t: &StreamTruth, id: u32) -> Evidence {
    t.incidents[id as usize]
        .observations
        .iter()
        .map(|o: &ObsId| s.events()[o.0 as usize].clone())
        .collect()
}

/// Parameters with the tier mix replaced.
fn with_mix(seed: u64, plain: u32, hard: u32) -> StreamParams {
    let mut p = StreamParams::new(seed);
    p.mix.plain_permille = plain;
    p.mix.hard_permille = hard;
    p
}

fn no_regime(seed: u64, plain: u32, hard: u32) -> StreamParams {
    let mut p = with_mix(seed, plain, hard);
    p.regimes.clear();
    p
}

fn known_truth(i: &IncidentTruth) -> Hypothesis {
    match i.truth {
        Some(h) => match h.kind {
            StreamKind::Known(k) => Some((k, h.site)),
            StreamKind::Hard(_) => panic!("not a known kind"),
        },
        None => None,
    }
}

// ---- Plain

#[test]
fn the_cheap_rung_identifies_every_plain_incident_from_its_own_evidence() {
    let (mut identified, mut duos, mut total) = (0, 0, 0);
    for seed in 0..12 {
        let p = no_regime(seed, 1000, 0);
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        let template = probing_sim(&p);
        for inc in &t.incidents {
            let ev = evidence_of(&s, &t, inc.id);
            let mut sim = template.clone();
            let run = cheap::run(&public, &ev, 3_000_000_000, &mut sim);
            total += 1;
            assert_eq!(
                run.declared,
                Some(known_truth(inc)),
                "seed {seed} incident {}: {:?}",
                inc.id,
                inc.shape
            );
            if inc.shape.duo {
                duos += 1;
                // Exactly one cheap probe settles a duo.
                assert_eq!(run.probes, 1, "{:?}", inc.shape);
                // From the stream alone the open set is exactly two kinds at the site.
                let before: Vec<Hypothesis> = consistent_hypotheses(&public, &ev);
                assert_eq!(before.len(), 2, "{before:?}");
            } else {
                assert_eq!(run.probes, 0, "{:?}", inc.shape);
                let before = consistent_hypotheses(&public, &ev);
                assert_eq!(before, vec![known_truth(inc)]);
            }
            identified += 1;
        }
    }
    assert_eq!(identified, total);
    println!(
        "cheap rung identified {identified} of {total} plain incidents from their own evidence ({duos} duos, one probe each)"
    );
    assert!(total > 200, "{total}");
    assert!(
        duos > 20,
        "the one-probe presentation was exercised only {duos} times"
    );
}

#[test]
fn a_cheap_rung_that_windows_by_site_still_identifies_most_plain_incidents_in_noise() {
    // The cheap rung is given everything the stream shows at the site and its dependents in the
    // first three seconds (the true site: an idealised segmentation), noise and other incidents
    // included. This is the number that says whether noise makes plain incidents unwinnable.
    let (mut ok, mut total) = (0, 0);
    for seed in 0..10 {
        let p = no_regime(seed, 1000, 0);
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        let template = probing_sim(&p);
        for inc in &t.incidents {
            let site = inc.occupies[0];
            let mask = dependents_mask(&t.services, site);
            let near = |sv: ServiceId| sv == site || mask[sv.index()];
            let ev: Evidence = s
                .events()
                .iter()
                .filter(|(at, o)| {
                    at.0 >= inc.onset_ns
                        && at.0 < inc.onset_ns + 3_000_000_000
                        && match o {
                            Observation::Counter { service, .. }
                            | Observation::Message { service, .. }
                            | Observation::Snapshot { service, .. } => near(*service),
                            _ => false,
                        }
                })
                .cloned()
                .collect();
            let mut sim = template.clone();
            let run = cheap::run(&public, &ev, 3_000_000_000, &mut sim);
            total += 1;
            if run.declared == Some(known_truth(inc)) {
                ok += 1;
            }
        }
    }
    let rate = ok as f64 / total as f64;
    println!("windowed cheap rung, plain incidents in noise: {ok}/{total} = {rate:.3}");
    assert!(rate > 0.85, "{ok}/{total}");
    assert!(
        rate < 1.0,
        "noise must cost the cheap rung something here too"
    );
}

// ---- Hard

#[test]
fn the_cheap_rung_cannot_identify_a_hard_incident_even_given_everything() {
    // Per family: how many incidents the public rules found contradictory at the end of a
    // patient run, and how many the shared rule declared as the known kind the first moments
    // imitate at the true site.
    let mut counts: std::collections::BTreeMap<HardKind, [u32; 4]> = Default::default();
    let patient = 40_000_000_000u64;
    for seed in 0..8 {
        let p = no_regime(seed, 0, 1000);
        let (s, t) = with_truth(&p);
        let public = s.public_info().world_public_info();
        let template = probing_sim(&p);
        for inc in &t.incidents {
            assert_eq!(inc.tier, Tier::Hard);
            let hk = inc.shape.hard_kind.unwrap();
            let ev = evidence_of(&s, &t, inc.id);
            let site = inc.occupies[0];

            // 1. The rule, patient enough to see every decisive observation, with the first
            //    world's default probe budget. Whatever it declares, it is a first-world
            //    hypothesis, and the truth is a hard kind: it cannot be right.
            let mut sim = template.clone();
            let run = cheap::run(&public, &ev, patient, &mut sim);
            let entry = counts.entry(hk).or_default();
            entry[0] += 1;
            entry[3] += (inc.shape.contradicts_early == Some(true)) as u32;
            if run.consistent(&public).is_empty() {
                entry[1] += 1;
            }
            if matches!(run.declared, Some(Some((k, at))) if Some(k) == inc.shape.mimics && at == site)
            {
                entry[2] += 1;
            }

            // 2. Exhaustive probing, beyond any budget: every probe at every service after
            //    everything has arrived. Not one known hypothesis explains a compound, a
            //    cascade or a split brain; a leak is explained, wrongly, as resource exhaustion.
            let mut sim2 = template.clone();
            let end = Instant(inc.onset_ns + 25_000_000_000);
            let mut all = ev.clone();
            all.extend(exhaustive_probes(&mut sim2, t.services.len(), end));
            let open = consistent_hypotheses(&public, &all);
            if hk == HardKind::SlowLeak {
                assert!(
                    open.iter()
                        .all(|h| matches!(h, Some((FaultKind::ResourceExhausted, _)))),
                    "{open:?}"
                );
            } else {
                assert!(
                    open.is_empty(),
                    "seed {seed} incident {} ({hk:?}): the public rules explain it: {open:?}",
                    inc.id
                );
            }
        }
    }
    println!(
        "hard incidents: [total, rules contradictory in what the run saw, declared the imitated \
         kind at the site, contradictory from the start]"
    );
    for (hk, c) in &counts {
        println!("  {hk:?}: {c:?}");
    }
    for hk in HardKind::ALL {
        let [n, contradictory, imitated, from_start] = counts[&hk];
        assert!(n >= 20, "{hk:?}: only {n} incidents");
        match hk {
            // A leak never contradicts the rules: it is read as a plain resource problem.
            HardKind::SlowLeak => {
                assert_eq!(contradictory, 0);
                assert!(imitated * 10 >= n * 9, "{imitated}/{n}");
            }
            // A split brain leaves two kinds open, so the rule waits, and by the time it
            // declares the contradicting evidence has arrived.
            HardKind::SplitBrain => assert_eq!(contradictory, n),
            // A compound or cascade that imitates a plain incident is declared at once, as the
            // imitated kind, before the evidence that breaks the rules arrives; one that
            // contradicts the rules from the start is seen to.
            HardKind::Compound | HardKind::Cascade => {
                assert_eq!(contradictory, from_start, "{hk:?}");
                assert!(from_start > 0 && from_start < n, "both modes occur: {hk:?}");
            }
        }
    }
    // The shared rule still names a confident known kind at the right site for a large share of
    // all hard incidents, which is what makes a confident cheap answer a poor router.
    let total: u32 = counts.values().map(|c| c[0]).sum();
    let imitated: u32 = counts.values().map(|c| c[2]).sum();
    assert!(imitated * 2 > total, "{imitated}/{total}");
}

// ---- Cost

#[test]
fn a_call_costs_at_least_ten_thousand_typical_component_calls() {
    use gordian_components::WorkingState;
    use gordian_core::Resource;
    let p = StreamParams::new(0);
    let (s, _) = with_truth(&p);
    let public = s.public_info().world_public_info();
    let mut costs: Vec<u64> = Vec::new();
    for window in [16usize, 64, 256] {
        let mut state = WorkingState::new(public.clone(), window);
        for (at, o) in s.events().iter().take(window) {
            state.admit(*at, o.clone());
        }
        for c in gordian_run::standard_components() {
            let ns: u64 = c
                .declared_cost(&state)
                .iter()
                .filter(|ch| ch.resource == Resource::Compute)
                .map(|ch| ch.amount)
                .sum();
            costs.push(ns);
        }
    }
    costs.sort();
    let typical = costs[costs.len() / 2];
    let base = p.reasoner.cost.cost(0).modelled_ns;
    println!(
        "median component call {typical} ns, reasoner base {base} ns, ratio {}",
        base / typical
    );
    assert!(typical > 0);
    assert!(base >= 10_000 * typical, "base {base} vs typical {typical}");
    assert!(base >= 1_000 * *costs.last().unwrap());
    let c100 = p.reasoner.cost.cost(100);
    assert!(c100.modelled_ns > 3 * base && c100.tokens == 400 + 100 * 20);
    assert_eq!(c100.calls, 1);
}
