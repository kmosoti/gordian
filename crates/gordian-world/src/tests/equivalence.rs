//! The optimized checker against the reference (work item A5b).
//!
//! `physics::consistent_worlds` and `physics::consistent_hypotheses` must return exactly what
//! `consistent_worlds_reference` and `consistent_hypotheses_reference` return, element for element
//! and in the same order, for every input. Each comparison below checks both pairs. Three
//! sources of input, because each reaches something the others do not:
//!
//! 1. every prefix of generated episodes of all eleven classes (the inputs the experiments will
//!    produce), then the same stream with probe results appended (the generator emits none);
//! 2. an adversarial generator for late-anchor streams, up to 2048 entries: alarms at the site
//!    that are not `ErrorRate`, the site's `ErrorRate` somewhere in the stream, then alarms at
//!    its dependents, under five instant disciplines and five kinds of disturbance;
//! 3. random sequences over the whole public alphabet of observations (dangling services,
//!    boundary counter values, non-catalogue text, wrong-typed probe results, unsorted and tied
//!    instants), both uniform and biased towards what a random hidden world permits so that
//!    the consistent set is often not empty.
//!
//! Each run prints its case and comparison counts (`cargo test -p gordian-world equivalence --
//! --nocapture`), and asserts that a minimum share of comparisons returned a non-empty set, so
//! the equivalence cannot hold only because everything was contradicted.

use super::*;
use crate::episode::{BudgetSpec, PublicInfo};
use crate::fault::FaultKind;
use crate::graph::World;
use crate::physics::{
    HIGH, SignalText, consistent_hypotheses_counted, consistent_hypotheses_reference,
    consistent_worlds, consistent_worlds_counted, consistent_worlds_reference, counters, messages,
    probe_result,
};
use crate::sense::{CounterName, Probe, ProbeResult, Severity};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use std::cell::Cell;

/// A small deterministic generator for the detail of a generated case. The proptest strategies
/// choose the shape (lengths, positions, modes); this fills in the content.
struct Mix(u64);

impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())]
    }
}

/// Counts what a run compared, and how many of those comparisons were not trivially empty.
#[derive(Default)]
struct Tally {
    cases: Cell<u64>,
    comparisons: Cell<u64>,
    non_empty: Cell<u64>,
    /// Comparisons on at least 256 entries whose consistent set was not empty: the shape in
    /// which the reference is slow, with survivors, so the anchor test is exercised.
    long_non_empty: Cell<u64>,
    /// Comparisons whose consistent set held something other than "no fault".
    some_fault: Cell<u64>,
}

fn bump(c: &Cell<u64>) {
    c.set(c.get() + 1);
}

/// Compares the two pairs of functions on one input.
fn agree(
    public: &PublicInfo,
    evidence: &[(Instant, Observation)],
    t: &Tally,
) -> Result<(), TestCaseError> {
    let worlds_ref = consistent_worlds_reference(public, evidence);
    prop_assert_eq!(&consistent_worlds(public, evidence), &worlds_ref);
    let hyps_ref = consistent_hypotheses_reference(public, evidence);
    prop_assert_eq!(&consistent_hypotheses(public, evidence), &hyps_ref);
    // Work item A8b: the counted variants return what the plain functions and the references
    // return, and the same counts every time (the counts are a function of the input alone).
    let (worlds_counted, ops) = consistent_worlds_counted(public, evidence);
    prop_assert_eq!(&worlds_counted, &worlds_ref);
    let (hyps_counted, ops_h) = consistent_hypotheses_counted(public, evidence);
    prop_assert_eq!(&hyps_counted, &hyps_ref);
    prop_assert_eq!(ops, ops_h);
    prop_assert_eq!(consistent_worlds_counted(public, evidence).1, ops);
    prop_assert!(ops.scanned <= evidence.len() as u64);
    bump(&t.comparisons);
    if !hyps_ref.is_empty() {
        bump(&t.non_empty);
        if evidence.len() >= 256 {
            bump(&t.long_non_empty);
        }
        if hyps_ref.iter().any(|h| h.is_some()) {
            bump(&t.some_fault);
        }
    }
    Ok(())
}

/// Runs `check` over `cases` generated values and prints the tally.
fn run<S: Strategy>(
    name: &str,
    cases: u32,
    strategy: S,
    check: impl Fn(S::Value, &Tally) -> Result<(), TestCaseError>,
) -> Tally {
    let tally = Tally::default();
    let mut runner = TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    });
    let result = runner.run(&strategy, |v| {
        bump(&tally.cases);
        check(v, &tally)
    });
    println!(
        "equivalence {name}: cases={} comparisons={} non_empty={} long_non_empty={} some_fault={}",
        tally.cases.get(),
        tally.comparisons.get(),
        tally.non_empty.get(),
        tally.long_non_empty.get(),
        tally.some_fault.get(),
    );
    if let Err(e) = result {
        panic!("{name}: {e}");
    }
    tally
}

fn ctr(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

fn msg(service: ServiceId, text_id: u64) -> Observation {
    Observation::Message {
        service,
        text_id,
        severity: Severity::Low,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Every prefix of generated episodes, all eleven classes.
// ---------------------------------------------------------------------------------------------

fn spec_strategy() -> impl Strategy<Value = EpisodeSpec> {
    (
        any::<u64>(),
        0usize..11,
        0u64..30_000_000_000,
        0u32..8,
        (0u8..15, 0u8..15),
        0u32..30,
    )
        .prop_map(
            |(seed, class, horizon, noise_rate, (lo, hi), delay_k)| EpisodeSpec {
                seed,
                class: EpisodeClass::ALL[class],
                horizon: Instant(horizon),
                noise_rate,
                // The default budget, so that every probe below is accepted.
                budget: BudgetSpec::default(),
                min_services: lo,
                max_services: hi,
                delay_k,
            },
        )
}

#[test]
fn equals_reference_on_every_prefix_of_generated_episodes() {
    let tally = run("every-prefix", 250, spec_strategy(), |spec, t| {
        let ep = generate(&spec);
        let public = ep.public_info();
        let stream = ep.stream();
        for cut in 0..=stream.len() {
            agree(&public, &stream[..cut], t)?;
        }
        // The generator emits no probe results or corrections. Append every probe kind at every
        // service, cumulatively, after the empty prefix, the half-way prefix and the whole stream.
        for cut in [0, stream.len() / 2, stream.len()] {
            for site in 0..ep.world().len() as u32 {
                let mut evidence: Evidence = stream[..cut].to_vec();
                for kind in ProbeKind::ALL {
                    evidence.push(probe_obs(&ep, kind, ServiceId(site)));
                    agree(&public, &evidence, t)?;
                }
            }
        }
        Ok(())
    });
    // Streams are consistent with their true hypothesis at every prefix (soundness), so every
    // generated comparison is non-empty; a drop here would mean the generator or the checker
    // changed under this test.
    assert_eq!(tally.non_empty.get(), tally.comparisons.get());
}

#[test]
fn the_prefix_test_covers_every_class() {
    // The proptest above draws classes at random; this pins that all eleven are compared, over
    // fixed seeds and with the full stream and every prefix.
    let t = Tally::default();
    for class in EpisodeClass::ALL {
        for seed in 0..6 {
            let ep = episode(class, seed);
            let public = ep.public_info();
            for cut in 0..=ep.stream().len() {
                agree(&public, &ep.stream()[..cut], &t).unwrap();
            }
        }
    }
    println!(
        "equivalence all-classes-fixed: comparisons={} non_empty={}",
        t.comparisons.get(),
        t.non_empty.get()
    );
    assert_eq!(t.non_empty.get(), t.comparisons.get());
}

// ---------------------------------------------------------------------------------------------
// 2. Adversarial late-anchor streams.
// ---------------------------------------------------------------------------------------------

/// The knobs of one late-anchor stream. The detail (which alarm, which service) comes from
/// `detail`.
#[derive(Debug, Clone)]
struct LateAnchor {
    graph_seed: u64,
    class: usize,
    /// Number of observations, 1 to 2048.
    n: usize,
    /// Position of the site's `ErrorRate`, in thousandths of the way through the stream.
    anchor_pm: u32,
    /// Share, in thousandths, of the observations after the anchor that are alarms at dependents.
    dep_pm: u32,
    /// 0 increasing instants; 1 many ties; 2 random order; 3 decreasing; 4 increasing but the
    /// anchor carries another observation's instant.
    time_mode: u8,
    /// Bit 0: a second anchor later in the stream with the earliest possible instant. Bit 1: a
    /// dependent alarm before the anchor. Bit 2: an `ErrorRate` at an unrelated service. Bit 3: a
    /// tail of probe results and corrections that fit a random hidden world. Bit 4: a drifted
    /// snapshot at the site.
    extras: u8,
    detail: u64,
}

fn late_anchor_strategy() -> impl Strategy<Value = LateAnchor> {
    let length = prop_oneof![1 => 1usize..=16, 2 => 17usize..=256, 3 => 257usize..=2048];
    (
        any::<u64>(),
        0usize..11,
        length,
        // Half the cases put the anchor in the middle two fifths, where the reference is worst.
        prop_oneof![1 => 0u32..=1000, 1 => 300u32..=700],
        prop_oneof![1 => 0u32..=1000, 2 => 900u32..=1000],
        prop_oneof![3 => Just(0u8), 2 => Just(1u8), 2 => Just(2u8), 1 => Just(3u8), 1 => Just(4u8)],
        // Most disturbances empty the result (a dependent alarm before the anchor, an anchor at
        // an unrelated service, a drifted snapshot), so they are mostly left to the last arm.
        prop_oneof![4 => Just(0u8), 2 => Just(1u8), 2 => Just(8u8), 1 => 0u8..32],
        any::<u64>(),
    )
        .prop_map(
            |(graph_seed, class, n, anchor_pm, dep_pm, time_mode, extras, detail)| LateAnchor {
                graph_seed,
                class,
                n,
                anchor_pm,
                dep_pm,
                time_mode,
                extras,
                detail,
            },
        )
}

/// A hidden world, for building observations that it permits.
#[derive(Clone, Copy)]
struct Hidden {
    h: crate::Hypothesis,
    bits: (bool, bool),
    drift_hash: u64,
}

/// A probe result or correction effect that `hidden` would produce.
fn probe_or_correction(rng: &mut Mix, public: &PublicInfo, hidden: Hidden) -> Observation {
    let n = public.services.len();
    let target = |rng: &mut Mix| match hidden.h {
        Some((_, site)) if rng.below(2) == 0 => site,
        _ => ServiceId(rng.below(n) as u32),
    };
    if rng.below(6) == 0 {
        let site = target(rng);
        return Observation::Correction {
            site,
            resolved: hidden.h.is_some_and(|(_, s)| s == site),
        };
    }
    let probe = Probe {
        kind: rng.pick(&ProbeKind::ALL),
        target: target(rng),
    };
    let result = probe_result(
        &public.services,
        hidden.h,
        hidden.bits,
        hidden.drift_hash,
        probe,
    );
    Observation::Probed { probe, result }
}

fn build_late_anchor(p: &LateAnchor) -> (PublicInfo, Evidence) {
    let public =
        generate(&EpisodeSpec::new(p.graph_seed, EpisodeClass::ALL[p.class])).public_info();
    let world = World {
        services: public.services.clone(),
    };
    let ns = world.len();
    let mut rng = Mix(p.detail);
    let with_dependents: Vec<ServiceId> = (0..ns as u32)
        .map(ServiceId)
        .filter(|s| !world.dependents_of(*s).is_empty())
        .collect();
    let site = rng.pick(&with_dependents);
    let deps = world.dependents_of(site);
    let alarm = |rng: &mut Mix| HIGH + rng.below(50) as u64;
    let any_service = |rng: &mut Mix| ServiceId(rng.below(ns) as u32);

    let n = p.n;
    let anchor_at = (n - 1) * p.anchor_pm as usize / 1000;
    // Palettes are chosen once per case, not per entry: one entry that no surviving hypothesis
    // permits empties the result, and with up to 2000 entries a per-entry chance of that would
    // contradict nearly every long case before the dependents were reached. Each palette keeps
    // at least one hypothesis alive at the site (the comments name them); `poison` is a
    // per-entry chance of about one entry per four cases that kills all.
    let pre_palette = rng.below(4); // 0 Latency only; 1 + Saturation; 2 + Restarts, MixedSignals; 3 + MixedSignals
    let dep_palette = rng.below(4); // 0 Latency; 1 + ErrorRate; 2 + ErrorRate, UpstreamUnreachable; 3 as 0 with poison
    let poison_in_4n = |rng: &mut Mix| dep_palette == 3 && rng.below(4 * n) == 0;
    let mut obs: Vec<Observation> = Vec::with_capacity(n);
    for i in 0..n {
        let o = if i == anchor_at {
            ctr(site, CounterName::ErrorRate, alarm(&mut rng))
        } else if i < anchor_at {
            // Before the anchor: alarms at the site that are not `ErrorRate`, so that
            // ResourceExhausted, DependencyDown and Intermittent can survive to the dependents.
            match (pre_palette, rng.below(8)) {
                (_, 0) => benign(&mut rng, ns),
                (1, 1) => ctr(site, CounterName::Saturation, alarm(&mut rng)),
                (2, 1) => ctr(site, CounterName::Restarts, alarm(&mut rng)),
                (2 | 3, 2) => msg(site, SignalText::MixedSignals.text_id()),
                _ => ctr(site, CounterName::Latency, alarm(&mut rng)),
            }
        } else if rng.below(1000) < p.dep_pm as usize {
            let dep = rng.pick(&deps);
            if poison_in_4n(&mut rng) {
                ctr(dep, CounterName::Saturation, alarm(&mut rng))
            } else {
                match (dep_palette, rng.below(6)) {
                    (1 | 2, 0 | 1) => ctr(dep, CounterName::ErrorRate, alarm(&mut rng)),
                    (2, 2) => msg(dep, SignalText::UpstreamUnreachable.text_id()),
                    _ => ctr(dep, CounterName::Latency, alarm(&mut rng)),
                }
            }
        } else {
            match rng.below(4) {
                0 => ctr(site, CounterName::Latency, alarm(&mut rng)),
                _ => benign(&mut rng, ns),
            }
        };
        obs.push(o);
    }

    let mut times: Vec<u64> = match p.time_mode {
        0 | 4 => (0..n).map(|i| i as u64 + 1).collect(),
        1 => (0..n).map(|i| (i / 8) as u64 + 1).collect(),
        // Random instants, but the anchor is the earliest of all: the instants are out of order
        // and ties are common, yet every dependent alarm is anchored.
        2 => (0..n).map(|_| 1 + rng.below(n) as u64).collect(),
        // Strictly decreasing: every dependent alarm comes before its anchor in time.
        _ => (0..n).map(|i| (n - i) as u64).collect(),
    };
    if p.time_mode == 2 {
        times[anchor_at] = 0;
    }
    if p.time_mode == 4 {
        // The anchor takes another entry's instant: later than some dependents, earlier than the
        // rest.
        times[anchor_at] = times[rng.below(n)];
    }
    // Extras. Each replaces an existing entry, so the length stays `n`.
    // A second anchor with the earliest instant. Where the instants decrease it is what makes
    // the dependents after it survive, and it matters whether it sits right after the first
    // anchor (so the earliest, not the first, anchor decides) or somewhere later.
    let second_anchor = p.extras & 1 != 0 || (p.time_mode == 3 && rng.below(2) == 0);
    if second_anchor && anchor_at + 1 < n {
        let at = if rng.below(2) == 0 {
            anchor_at + 1
        } else {
            anchor_at + 1 + rng.below(n - anchor_at - 1)
        };
        obs[at] = ctr(site, CounterName::ErrorRate, alarm(&mut rng));
        times[at] = 0;
    }
    if p.extras & 2 != 0 && anchor_at > 0 {
        let at = rng.below(anchor_at);
        obs[at] = ctr(rng.pick(&deps), CounterName::Latency, alarm(&mut rng));
    }
    if p.extras & 4 != 0 {
        let at = rng.below(n);
        if at != anchor_at {
            obs[at] = ctr(
                any_service(&mut rng),
                CounterName::ErrorRate,
                alarm(&mut rng),
            );
        }
    }
    if p.extras & 8 != 0 {
        let hidden = Hidden {
            h: Some((rng.pick(&FaultKind::ALL), site)),
            bits: rng.pick(&[(false, false), (true, true), (false, true), (true, false)]),
            drift_hash: 0xD1F7,
        };
        // Replace up to the last eight entries, never the anchor or anything before it.
        for o in obs.iter_mut().skip(n.saturating_sub(8).max(anchor_at + 1)) {
            *o = probe_or_correction(&mut rng, &public, hidden);
        }
    }
    if p.extras & 16 != 0 {
        let at = rng.below(n);
        if at != anchor_at {
            obs[at] = Observation::Snapshot {
                service: site,
                config_hash: public.services[site.index()].config_hash.wrapping_add(1),
            };
        }
    }
    (public, times.into_iter().map(Instant).zip(obs).collect())
}

/// An observation that no rule can discriminate on, or a free-form message.
fn benign(rng: &mut Mix, ns: usize) -> Observation {
    let service = ServiceId(rng.below(ns) as u32);
    match rng.below(3) {
        0 => ctr(
            service,
            rng.pick(&CounterName::ALL),
            rng.below(HIGH as usize) as u64,
        ),
        1 => msg(service, crate::physics::CATALOGUE_LIMIT + rng.next() % 1000),
        _ => msg(service, SignalText::CheckHealth.text_id()),
    }
}

#[test]
fn equals_reference_on_adversarial_late_anchor_streams() {
    let tally = run("late-anchor", 320, late_anchor_strategy(), |p, t| {
        let (public, evidence) = build_late_anchor(&p);
        prop_assert_eq!(evidence.len(), p.n);
        agree(&public, &evidence, t)
    });
    // The point of the generator is the slow path with survivors: demand it was reached.
    assert!(
        tally.long_non_empty.get() >= 40,
        "only {} long, non-empty cases of {}",
        tally.long_non_empty.get(),
        tally.cases.get()
    );
    assert!(tally.some_fault.get() >= 80);
}

#[test]
fn the_pure_late_anchor_shape_is_compared_at_every_length() {
    // No disturbance, increasing instants, anchor in the middle: the shape of the benchmark and
    // of `cost_calibration::content_envelope_at_window_size_256`, at lengths on both sides of
    // every power of two up to 2048.
    let t = Tally::default();
    let public = episode(EpisodeClass::Ambiguous, 3).public_info();
    let world = World {
        services: public.services.clone(),
    };
    let (site, dep) = (ServiceId(0), world.dependents_of(ServiceId(0))[0]);
    for n in (1..=11)
        .flat_map(|k| [(1usize << k) - 1, 1 << k, (1 << k) + 1])
        .filter(|n| *n <= 2048)
    {
        let evidence: Evidence = (0..n)
            .map(|i| {
                let o = match i {
                    i if i < n / 2 => ctr(site, CounterName::Latency, 99),
                    i if i == n / 2 => ctr(site, CounterName::ErrorRate, 99),
                    _ => ctr(dep, CounterName::Latency, 99),
                };
                (Instant(i as u64 + 1), o)
            })
            .collect();
        agree(&public, &evidence, &t).unwrap();
    }
    println!(
        "equivalence pure-late-anchor: comparisons={} non_empty={} long_non_empty={}",
        t.comparisons.get(),
        t.non_empty.get(),
        t.long_non_empty.get()
    );
    assert!(t.long_non_empty.get() >= 10);
}

/// The other late-anchor shape, in the words of the work item: the dependents' alarms first and
/// the site's `ErrorRate` last. No world that has the site as its site can explain the early
/// dependent alarms, and the worlds that have a dependent as their site cannot explain an
/// `ErrorRate` at an ancestor, so the consistent set is empty; what this input checks is that
/// the optimized pass rejects for the same reason and never lets an anchor that comes *later* in
/// the list (or the evidence) explain an alarm that came before it. `tail` extra dependent
/// alarms may follow the anchor, which then are anchored but do not bring the early ones back.
#[derive(Debug, Clone)]
struct DependentsFirst {
    graph_seed: u64,
    class: usize,
    /// Number of observations, 1 to 2048.
    n: usize,
    /// Entries after the anchor, 0 to 4 (capped so that the anchor is not first).
    tail: usize,
    /// 0 increasing instants; 1 all equal; 2 decreasing (every anchor instant is earlier than
    /// the alarms before it); 3 the anchor is given the earliest instant of all.
    time_mode: u8,
    /// Share, in thousandths, of the entries before the anchor that are dependents' alarms; the
    /// rest are benign. The first entry is always a dependent's alarm.
    dep_pm: u32,
    detail: u64,
}

fn dependents_first_strategy() -> impl Strategy<Value = DependentsFirst> {
    (
        any::<u64>(),
        0usize..11,
        prop_oneof![1 => 2usize..=16, 2 => 17usize..=256, 3 => 257usize..=2048],
        0usize..=4,
        0u8..4,
        prop_oneof![1 => 0u32..=1000, 2 => 900u32..=1000],
        any::<u64>(),
    )
        .prop_map(
            |(graph_seed, class, n, tail, time_mode, dep_pm, detail)| DependentsFirst {
                graph_seed,
                class,
                n,
                tail,
                time_mode,
                dep_pm,
                detail,
            },
        )
}

fn build_dependents_first(p: &DependentsFirst) -> (PublicInfo, Evidence, ServiceId) {
    let public =
        generate(&EpisodeSpec::new(p.graph_seed, EpisodeClass::ALL[p.class])).public_info();
    let world = World {
        services: public.services.clone(),
    };
    let mut rng = Mix(p.detail);
    let with_dependents: Vec<ServiceId> = (0..world.len() as u32)
        .map(ServiceId)
        .filter(|s| !world.dependents_of(*s).is_empty())
        .collect();
    let site = rng.pick(&with_dependents);
    let deps = world.dependents_of(site);
    let tail = p.tail.min(p.n - 2);
    let anchor_at = p.n - 1 - tail;
    let alarm = |rng: &mut Mix| HIGH + rng.below(50) as u64;
    let mut obs: Vec<Observation> = Vec::with_capacity(p.n);
    for i in 0..p.n {
        let o = if i == anchor_at {
            ctr(site, CounterName::ErrorRate, alarm(&mut rng))
        } else if i > anchor_at || i == 0 || rng.below(1000) < p.dep_pm as usize {
            let dep = rng.pick(&deps);
            let name = rng.pick(&[CounterName::Latency, CounterName::ErrorRate]);
            ctr(dep, name, alarm(&mut rng))
        } else {
            benign(&mut rng, world.len())
        };
        obs.push(o);
    }
    let mut times: Vec<u64> = match p.time_mode {
        0 | 3 => (0..p.n).map(|i| i as u64 + 1).collect(),
        1 => vec![7; p.n],
        _ => (0..p.n).map(|i| (p.n - i) as u64).collect(),
    };
    if p.time_mode == 3 {
        times[anchor_at] = 0;
    }
    (
        public,
        times.into_iter().map(Instant).zip(obs).collect(),
        site,
    )
}

#[test]
fn equals_reference_on_dependents_first_anchor_last_streams() {
    let long_pure = Cell::new(0u64);
    let tally = run(
        "dependents-first",
        300,
        dependents_first_strategy(),
        |p, t| {
            let (public, evidence, _site) = build_dependents_first(&p);
            prop_assert_eq!(evidence.len(), p.n);
            agree(&public, &evidence, t)?;
            if p.tail == 0 && p.n >= 256 {
                // The pure shape: dependents' alarms, then the anchor, nothing after.
                prop_assert_eq!(consistent_hypotheses_reference(&public, &evidence), vec![]);
                bump(&long_pure);
            }
            Ok(())
        },
    );
    println!(
        "equivalence dependents-first: pure shape at n >= 256 in {} cases",
        long_pure.get()
    );
    // Coverage of the pure shape here is random: each case is pure with probability about 0.10
    // (tail = 0 with 1/5, n >= 256 with about 1/2), so over 300 cases the count is about 30 with
    // a standard deviation of about 5.2. The earlier floor of 20 sat 1.9 standard deviations
    // below the mean and failed in about 2-3% of runs. The pure shape is now guaranteed by the
    // deterministic sweep below; this floor only catches a broken strategy, at about 3.9
    // standard deviations (chance failure about 5e-5).
    assert!(long_pure.get() >= 10, "{}", long_pure.get());
    assert_eq!(tally.cases.get(), 300);
}

/// The pure dependents-first shape at fixed lengths, for every class, time mode and two
/// dependent shares: coverage that does not depend on the proptest seed.
#[test]
fn the_pure_dependents_first_shape_is_compared_at_every_length() {
    let tally = Tally::default();
    let mut compared = 0u64;
    for class in 0..EpisodeClass::ALL.len() {
        for n in [256usize, 512, 1024, 2048] {
            for time_mode in 0u8..4 {
                for dep_pm in [900u32, 1000] {
                    let p = DependentsFirst {
                        graph_seed: 0x5eed_0000 + class as u64,
                        class,
                        n,
                        tail: 0,
                        time_mode,
                        dep_pm,
                        detail: (n as u64) << 8 | u64::from(time_mode),
                    };
                    let (public, evidence, _site) = build_dependents_first(&p);
                    assert_eq!(evidence.len(), n);
                    agree(&public, &evidence, &tally).unwrap_or_else(|e| panic!("{p:?}: {e}"));
                    assert_eq!(
                        consistent_hypotheses_reference(&public, &evidence),
                        vec![],
                        "{p:?}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 11 * 4 * 4 * 2);
}

// ---------------------------------------------------------------------------------------------
// 3. Random sequences over the public alphabet of observations.
// ---------------------------------------------------------------------------------------------

/// One observation drawn uniformly over the kinds of the alphabet, with boundary values and a
/// small chance of a service outside the graph.
fn random_observation(rng: &mut Mix, public: &PublicInfo) -> Observation {
    let ns = public.services.len();
    let service = |rng: &mut Mix| {
        if rng.below(40) == 0 {
            ServiceId(ns as u32 + rng.below(3) as u32)
        } else {
            ServiceId(rng.below(ns) as u32)
        }
    };
    let hash = |rng: &mut Mix, s: ServiceId| {
        let public_hash = public.services.get(s.index()).map_or(0, |x| x.config_hash);
        rng.pick(&[
            public_hash,
            public_hash,
            public_hash.wrapping_add(1),
            777,
            778,
            0,
        ])
    };
    match rng.below(10) {
        0..=2 => ctr(
            service(rng),
            rng.pick(&CounterName::ALL),
            rng.pick(&[0, 1, HIGH - 1, HIGH, HIGH + 1, 99, u64::MAX]),
        ),
        3..=5 => {
            let text_id = match rng.below(10) {
                0..=6 => rng.pick(&SignalText::ALL).text_id(),
                7 => rng.pick(&[0, 0xFF, 0x108, 0xFFFF, 0x1_0000, 0x1_0001]),
                _ => rng.next(),
            };
            Observation::Message {
                service: service(rng),
                text_id,
                severity: rng.pick(&Severity::ALL),
            }
        }
        6 => {
            let s = service(rng);
            Observation::Snapshot {
                service: s,
                config_hash: hash(rng, s),
            }
        }
        7 | 8 => {
            let probe = Probe {
                kind: rng.pick(&ProbeKind::ALL),
                target: service(rng),
            };
            let suggest = Probe {
                kind: rng.pick(&ProbeKind::ALL),
                target: service(rng),
            };
            let result = match rng.below(6) {
                0 => ProbeResult::Positive,
                1 => ProbeResult::Negative,
                2 | 3 => ProbeResult::ConfigHash(hash(rng, probe.target)),
                4 => ProbeResult::Inconclusive {
                    suggest: Some(probe),
                },
                _ => ProbeResult::Inconclusive {
                    suggest: if rng.below(2) == 0 {
                        None
                    } else {
                        Some(suggest)
                    },
                },
            };
            Observation::Probed { probe, result }
        }
        _ => Observation::Correction {
            site: service(rng),
            resolved: rng.below(2) == 0,
        },
    }
}

/// One observation that the hidden world `hidden` permits (and, for alarms at dependents, that is
/// permitted once a site `ErrorRate` has come earlier; the caller decides where to put it).
fn plausible_observation(rng: &mut Mix, public: &PublicInfo, hidden: Hidden) -> Observation {
    let ns = public.services.len();
    let Some((kind, site)) = hidden.h else {
        // No fault permits no abnormal observation: benign counters, free text, probes and
        // corrections that say "healthy".
        return match rng.below(4) {
            0 => benign(rng, ns),
            1 => probe_or_correction(rng, public, hidden),
            2 => {
                let service = ServiceId(rng.below(ns) as u32);
                Observation::Snapshot {
                    service,
                    config_hash: public.services[service.index()].config_hash,
                }
            }
            _ => benign(rng, ns),
        };
    };
    let world = World {
        services: public.services.clone(),
    };
    let deps = world.dependents_of(site);
    let (service, role) = if !deps.is_empty() && rng.below(2) == 0 {
        (rng.pick(&deps), crate::physics::Role::Dependent)
    } else {
        (site, crate::physics::Role::Site)
    };
    match rng.below(8) {
        0..=2 => {
            let names = counters(kind, role);
            ctr(service, rng.pick(names), HIGH + rng.below(50) as u64)
        }
        3 | 4 => match messages(kind, role) {
            [] => benign(rng, ns),
            ms => msg(service, rng.pick(ms).text_id()),
        },
        5 => Observation::Snapshot {
            service: site,
            config_hash: if kind == FaultKind::ConfigDrift {
                hidden.drift_hash
            } else {
                public.services[site.index()].config_hash
            },
        },
        6 => probe_or_correction(rng, public, hidden),
        _ => benign(rng, ns),
    }
}

#[derive(Debug, Clone)]
struct RandomEvidence {
    graph_seed: u64,
    class: usize,
    len: usize,
    /// 0 for uniform. Otherwise the percentage of entries drawn from the hidden world's
    /// permitted observations.
    plausible_pct: u32,
    /// Instants are drawn from `0..time_range`; 1 makes them all equal.
    time_range: u64,
    /// Sort by instant (stable) after drawing.
    sorted: bool,
    /// Begin with the site's `ErrorRate`, as generated streams do.
    anchor_first: bool,
    detail: u64,
}

fn random_evidence_strategy(plausible: bool) -> impl Strategy<Value = RandomEvidence> {
    let pct = if plausible { 40u32..=100 } else { 0u32..=0 };
    (
        any::<u64>(),
        0usize..11,
        prop_oneof![3 => 0usize..=12, 2 => 13usize..=64, 1 => 65usize..=300],
        pct,
        prop_oneof![1 => 1u64..=2, 2 => 3u64..=10, 1 => 11u64..=400],
        any::<bool>(),
        any::<bool>(),
        any::<u64>(),
    )
        .prop_map(
            |(graph_seed, class, len, plausible_pct, time_range, sorted, anchor_first, detail)| {
                RandomEvidence {
                    graph_seed,
                    class,
                    len,
                    plausible_pct,
                    time_range,
                    sorted,
                    anchor_first,
                    detail,
                }
            },
        )
}

fn build_random(p: &RandomEvidence) -> (PublicInfo, Evidence) {
    let public =
        generate(&EpisodeSpec::new(p.graph_seed, EpisodeClass::ALL[p.class])).public_info();
    let ns = public.services.len();
    let mut rng = Mix(p.detail);
    let hidden = Hidden {
        h: if rng.below(8) == 0 {
            None
        } else {
            Some((rng.pick(&FaultKind::ALL), ServiceId(rng.below(ns) as u32)))
        },
        bits: rng.pick(&[(false, false), (true, true), (false, true), (true, false)]),
        drift_hash: 0xD1F7,
    };
    let mut evidence: Evidence = Vec::with_capacity(p.len);
    for i in 0..p.len {
        let o = match (i, hidden.h) {
            (0, Some((_, site))) if p.anchor_first && p.plausible_pct > 0 => {
                ctr(site, CounterName::ErrorRate, HIGH + 7)
            }
            _ if (rng.below(100) as u32) < p.plausible_pct => {
                plausible_observation(&mut rng, &public, hidden)
            }
            _ => random_observation(&mut rng, &public),
        };
        evidence.push((Instant(rng.next() % p.time_range), o));
    }
    if p.sorted {
        evidence.sort_by_key(|(t, _)| *t);
    }
    (public, evidence)
}

#[test]
fn equals_reference_on_uniformly_random_evidence() {
    let tally = run(
        "random-uniform",
        2000,
        random_evidence_strategy(false),
        |p, t| {
            let (public, evidence) = build_random(&p);
            agree(&public, &evidence, t)
        },
    );
    // Uniform evidence is mostly contradictory, which is the point of it (rejection paths, the
    // dangling-service return, the single-drift return); the next test covers the survivors.
    assert!(tally.non_empty.get() >= 100, "{}", tally.non_empty.get());
}

#[test]
fn equals_reference_on_random_evidence_biased_towards_a_hidden_world() {
    let tally = run(
        "random-plausible",
        3000,
        random_evidence_strategy(true),
        |p, t| {
            let (public, evidence) = build_random(&p);
            agree(&public, &evidence, t)
        },
    );
    assert!(
        tally.non_empty.get() >= 600 && tally.some_fault.get() >= 400,
        "non_empty {} some_fault {} of {}",
        tally.non_empty.get(),
        tally.some_fault.get(),
        tally.comparisons.get()
    );
}

// ---------------------------------------------------------------------------------------------
// Hand-built cases with expected values, for the part of the rule the optimization touches:
// "an earlier `ErrorRate` alarm at the site with an instant no later than this observation's".
// ---------------------------------------------------------------------------------------------

mod anchor_semantics {
    use super::*;
    use crate::fault::FaultKind::*;

    /// A chain 0 <- 1 <- 2 <- 3 (each depends on the one before) plus service 4 depending on 0,
    /// the graph of `checker.rs`.
    fn public() -> PublicInfo {
        use crate::graph::{ResourceKind, Service};
        let dep = |i: u32, ds: &[u32]| Service {
            id: ServiceId(i),
            depends_on: ds.iter().copied().map(ServiceId).collect(),
            resource: ResourceKind::Cpu,
            config_hash: 1000 + i as u64,
            unreliable_health: false,
        };
        PublicInfo {
            services: vec![
                dep(0, &[]),
                dep(1, &[0]),
                dep(2, &[1]),
                dep(3, &[2]),
                dep(4, &[0]),
            ],
            prior_records: vec![],
            horizon: Instant(10_000_000_000),
            budget: BudgetSpec::default(),
        }
    }

    fn both(public: &PublicInfo, evidence: &[(Instant, Observation)]) -> Vec<crate::Hypothesis> {
        let expected = consistent_hypotheses_reference(public, evidence);
        assert_eq!(consistent_hypotheses(public, evidence), expected);
        assert_eq!(
            consistent_worlds(public, evidence),
            consistent_worlds_reference(public, evidence)
        );
        expected
    }

    fn at(pairs: &[(u64, Observation)]) -> Evidence {
        pairs
            .iter()
            .map(|(t, o)| (Instant(*t), o.clone()))
            .collect()
    }

    #[test]
    fn an_anchor_with_a_later_instant_does_not_anchor() {
        let public = public();
        let (site, dep) = (ServiceId(0), ServiceId(1));
        assert!(public.services[dep.index()].depends_on.contains(&site));
        // The anchor comes first in the list but carries instant 5; the dependent alarm carries 3.
        let ev = at(&[
            (5, ctr(site, CounterName::ErrorRate, 90)),
            (3, ctr(dep, CounterName::Latency, 90)),
        ]);
        // Dependent latency alarm with no anchor at or before instant 3: contradicted for every
        // world whose site is 0; for a world whose site is `dep` itself, the ErrorRate at 0
        // is an alarm at an ancestor and is not permitted. Nothing survives.
        assert_eq!(both(&public, &ev), vec![]);
    }

    #[test]
    fn an_anchor_at_the_same_instant_anchors() {
        let public = public();
        let (site, dep) = (ServiceId(0), ServiceId(1));
        let ev = at(&[
            (3, ctr(site, CounterName::ErrorRate, 90)),
            (3, ctr(dep, CounterName::Latency, 90)),
        ]);
        assert_eq!(
            both(&public, &ev),
            vec![
                Some((ResourceExhausted, site)),
                Some((DependencyDown, site)),
                Some((Intermittent, site)),
            ]
        );
    }

    #[test]
    fn the_earliest_anchor_decides_not_the_first_in_the_list() {
        let public = public();
        let (site, dep) = (ServiceId(0), ServiceId(1));
        // Two anchors: the first in the list is late (instant 9), the second early (instant 2).
        // The dependent alarm at instant 4 is anchored by the second one only.
        let ev = at(&[
            (9, ctr(site, CounterName::ErrorRate, 90)),
            (2, ctr(site, CounterName::ErrorRate, 91)),
            (4, ctr(dep, CounterName::Latency, 90)),
        ]);
        assert_eq!(
            both(&public, &ev),
            vec![
                Some((ResourceExhausted, site)),
                Some((DependencyDown, site)),
                Some((Intermittent, site)),
            ]
        );
        // The dependent alarm *before* the anchor in the list is not anchored by it, even when
        // the instants would allow it: the rule is about earlier observations, not earlier time.
        let ev = at(&[
            (4, ctr(dep, CounterName::Latency, 90)),
            (2, ctr(site, CounterName::ErrorRate, 91)),
        ]);
        assert_eq!(both(&public, &ev), vec![]);
    }

    #[test]
    fn only_alarms_at_the_worlds_own_site_anchor_it() {
        let public = public();
        // ErrorRate at 1 (a dependent of 0) does not anchor a world with site 0, and does not
        // anchor a world with site 1 for alarms at 2 unless it comes first at 1.
        let ev = at(&[
            (1, ctr(ServiceId(1), CounterName::ErrorRate, 90)),
            (2, ctr(ServiceId(2), CounterName::Latency, 90)),
        ]);
        let got = both(&public, &ev);
        assert!(
            got.iter().flatten().all(|(_, s)| *s == ServiceId(1)),
            "{got:?}"
        );
        assert!(!got.is_empty());
    }

    #[test]
    fn a_below_threshold_error_rate_is_not_an_anchor() {
        let public = public();
        let (site, dep) = (ServiceId(0), ServiceId(1));
        let ev = at(&[
            (1, ctr(site, CounterName::ErrorRate, HIGH - 1)),
            (2, ctr(dep, CounterName::Latency, 90)),
        ]);
        // The 49 is benign; the dependent alarm has no anchor; worlds with site `dep` accept the
        // latency as a site alarm (ResourceExhausted, DependencyDown, Intermittent at `dep`).
        let got = both(&public, &ev);
        assert!(got.iter().flatten().all(|(_, s)| *s == dep), "{got:?}");
    }
}
