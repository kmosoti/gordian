//! The shared cheap rung: what counts as abnormal, how anomalies are formed from public
//! statistics, where their anchors and contexts come from, and how their conclusions are declared.

mod stream_common;

use gordian_core::Instant;
use gordian_run::stream::arms::rung::{Conclusion, Rung, RungConfig, is_abnormal};
use gordian_run::stream::arms::{Proposed, Source};
use gordian_stream::{
    ObsId, ObsRef, StreamAction, StreamEvent, StreamHypothesis, StreamKind, StreamPublic,
};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{CATALOGUE_LIMIT, HIGH, SignalText};
use gordian_world::{CounterName, FaultKind, Observation, ServiceId, Severity};
use stream_common::*;

struct Feed {
    next: u32,
    rung: Rung,
}

impl Feed {
    fn new(public: &StreamPublic, cfg: RungConfig) -> Self {
        Self {
            next: 0,
            rung: Rung::new(public, cfg),
        }
    }

    /// Deliver `observations` (instants in milliseconds) in one step at `now_ms`, then notice.
    fn step(&mut self, now_ms: u64, observations: &[(u64, Observation)]) {
        let events: Vec<StreamEvent> = observations
            .iter()
            .map(|(ms, obs)| {
                let id = ObsId(self.next);
                self.next += 1;
                StreamEvent::Observed {
                    id,
                    at: at(*ms),
                    obs: obs.clone(),
                }
            })
            .collect();
        self.rung.absorb(&events, &[], at(now_ms));
        self.rung.notice(at(now_ms));
    }
}

fn counter(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

fn message(service: ServiceId, text_id: u64) -> Observation {
    Observation::Message {
        service,
        text_id,
        severity: Severity::Medium,
    }
}

/// A service with at least two dependents, one of its dependents, and one that is neither it nor
/// connected to it, from the graph of `public`.
fn picks(public: &StreamPublic) -> (ServiceId, ServiceId, ServiceId) {
    for s in 0..public.services.len() {
        let site = ServiceId(s as u32);
        let mask = dependents_mask(&public.services, site);
        let dependents: Vec<usize> = (0..mask.len()).filter(|i| mask[*i]).collect();
        let stranger = (0..mask.len()).find(|i| !mask[*i] && *i != s);
        if dependents.len() >= 2
            && let Some(z) = stranger
        {
            return (site, ServiceId(dependents[0] as u32), ServiceId(z as u32));
        }
    }
    panic!("no service has two dependents and a stranger");
}

fn burst(site: ServiceId, dependent: ServiceId, t: u64) -> Vec<(u64, Observation)> {
    vec![
        (t, counter(site, CounterName::ErrorRate, 80)),
        (t + 10, counter(site, CounterName::Latency, 90)),
        (t + 20, message(site, SignalText::OutOfResource.text_id())),
        (t + 30, counter(site, CounterName::Saturation, 90)),
        (t + 40, counter(dependent, CounterName::ErrorRate, 70)),
        (t + 50, counter(dependent, CounterName::Latency, 70)),
    ]
}

#[test]
fn abnormal_is_what_the_first_worlds_public_rules_say() {
    let public = public_of(&params(0, 150));
    let svc = &public.services;
    let s = ServiceId(0);
    assert!(is_abnormal(&counter(s, CounterName::ErrorRate, HIGH), svc));
    assert!(!is_abnormal(
        &counter(s, CounterName::ErrorRate, HIGH - 1),
        svc
    ));
    for text in SignalText::ALL {
        let expected = text != SignalText::CheckHealth;
        assert_eq!(
            is_abnormal(&message(s, text.text_id()), svc),
            expected,
            "{text:?}"
        );
    }
    assert!(
        !is_abnormal(&message(s, CATALOGUE_LIMIT + 5), svc),
        "free-form text carries no meaning"
    );
    let hash = svc[0].config_hash;
    assert!(!is_abnormal(
        &Observation::Snapshot {
            service: s,
            config_hash: hash
        },
        svc
    ));
    assert!(is_abnormal(
        &Observation::Snapshot {
            service: s,
            config_hash: hash + 1
        },
        svc
    ));
}

#[test]
fn a_burst_is_noticed_and_an_isolated_stray_is_not() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(1_000, &[(500, counter(site, CounterName::ErrorRate, 90))]);
    feed.step(
        2_000,
        &[(1_500, message(site, SignalText::Unauthorized.text_id()))],
    );
    assert!(
        feed.rung.views(at(2_000)).is_empty(),
        "two strays seconds apart are background"
    );
    assert_eq!(feed.rung.noticed_total(), 0);
    feed.step(10_000, &burst(site, dependent, 9_800));
    let views = feed.rung.views(at(10_000));
    assert_eq!(views.len(), 1, "{views:?}");
    assert_eq!(feed.rung.noticed_total(), 1);
    assert_eq!(views[0].site, site);
    assert_eq!(views[0].noticed_at, at(10_000));
    assert!(views[0].score >= RungConfig::default().notice_z);
    assert_eq!(views[0].delivered, feed.next);
}

#[test]
fn the_anchor_is_in_the_burst_not_at_the_stray_that_came_first() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    // A stray at the site 1.5 s before the burst: it attaches to the same anomaly, and the
    // anchor must still be an observation of the burst.
    feed.step(9_000, &[(8_300, counter(site, CounterName::Latency, 70))]);
    let start = feed.next;
    feed.step(10_000, &burst(site, dependent, 9_800));
    let view = feed.rung.views(at(10_000)).remove(0);
    assert!(view.anchor.0 >= start, "anchored on the stray: {view:?}");
    assert_eq!(view.anchor_at, at(9_800));
    assert_eq!(view.site, site);
}

#[test]
fn a_stray_at_a_dependency_does_not_take_the_burst_at_its_dependent() {
    // The graph is dense: the site depends on services that depend on others. A stray at one of
    // the services the site depends on, shortly before the burst, must not become the anomaly
    // the burst is attached to (the burst's own site is the anomaly's site).
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let ancestors: Vec<ServiceId> = (0..public.services.len())
        .map(|i| ServiceId(i as u32))
        .filter(|a| *a != site && dependents_mask(&public.services, *a)[site.index()])
        .collect();
    let Some(ancestor) = ancestors.first().copied() else {
        return; // the first service of a graph depends on nothing
    };
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(
        9_500,
        &[(9_000, counter(ancestor, CounterName::Latency, 70))],
    );
    feed.step(10_500, &burst(site, dependent, 9_800));
    let views = feed.rung.views(at(10_500));
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].site, site);
}

#[test]
fn a_heartbeat_at_a_site_opens_no_propagation_window() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let id = feed.rung.views(at(10_000))[0].id;
    let before = feed.rung.attached(id).len();
    // Heartbeats a second apart at the site, and a dependent's observation within 400 ms of one
    // of them: that observation is not propagation, so it is not attached to the anomaly.
    feed.step(
        12_000,
        &[
            (11_000, counter(site, CounterName::ErrorRate, 80)),
            (12_000, counter(site, CounterName::ErrorRate, 85)),
            (12_100, counter(dependent, CounterName::ErrorRate, 77)),
        ],
    );
    let attached = feed.rung.attached(id);
    assert_eq!(
        attached.len(),
        before + 2,
        "the two site heartbeats and nothing else"
    );
    assert!(
        attached
            .iter()
            .all(|(_, _, s)| *s != dependent || attached.len() < before + 3)
    );
    assert_eq!(
        attached
            .iter()
            .filter(|(t, _, s)| *s == dependent && *t == at(12_100))
            .count(),
        0
    );
}

#[test]
fn the_baseline_is_learned_from_the_stream_and_replays_bit_for_bit() {
    let public = public_of(&params(0, 150));
    let (site, dependent, stranger) = picks(&public);
    let history = |noisy: bool| {
        let mut feed = Feed::new(&public, RungConfig::default());
        if noisy {
            for k in 0..60u64 {
                let svc = ServiceId((k % public.services.len() as u64) as u32);
                if svc != site {
                    feed.step(
                        1_000 * (k + 1),
                        &[(1_000 * (k + 1) - 10, counter(svc, CounterName::Latency, 70))],
                    );
                } else {
                    feed.step(1_000 * (k + 1), &[]);
                }
            }
        }
        feed.step(80_000, &burst(site, dependent, 79_800));
        let _ = stranger;
        feed
    };
    let (mut quiet_a, mut quiet_b, mut noisy) = (history(false), history(false), history(true));
    let score = |f: &mut Feed| f.rung.views(at(80_000)).first().map(|v| v.score);
    let (a, b) = (score(&mut quiet_a).unwrap(), score(&mut quiet_b).unwrap());
    assert_eq!(
        a.to_bits(),
        b.to_bits(),
        "the same history gives the same bits"
    );
    // The same burst is less surprising against a noisier background.
    let n = score(&mut noisy).unwrap_or(f64::NEG_INFINITY);
    assert!(n < a, "{n} vs {a}");
}

#[test]
fn a_context_holds_the_evidence_at_the_site_and_the_burst_at_its_dependents_and_is_capped() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let cfg = RungConfig {
        context_max_refs: 8,
        ..RungConfig::default()
    };
    let mut feed = Feed::new(&public, cfg.clone());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let id = feed.rung.views(at(10_000))[0].id;
    // Later: a benign reading and free-form text at the site, and an observation at the dependent
    // long after the burst (another incident's, as far as the rung can tell).
    feed.step(
        14_000,
        &[
            (11_000, counter(site, CounterName::Saturation, 10)),
            (12_000, message(site, CATALOGUE_LIMIT + 7)),
            (13_000, counter(dependent, CounterName::ErrorRate, 77)),
        ],
    );
    let uncapped = {
        let mut c = cfg.clone();
        c.context_max_refs = 100;
        let mut f = Feed::new(&public, c);
        f.step(10_000, &burst(site, dependent, 9_800));
        f.step(
            14_000,
            &[
                (11_000, counter(site, CounterName::Saturation, 10)),
                (12_000, message(site, CATALOGUE_LIMIT + 7)),
                (13_000, counter(dependent, CounterName::ErrorRate, 77)),
            ],
        );
        let id = f.rung.views(at(14_000))[0].id;
        f.rung.context(id)
    };
    // The burst (6), the benign reading and the free-form message: 8. The dependent's late
    // observation (id 8) is not in it.
    assert_eq!(uncapped.len(), 8, "{uncapped:?}");
    assert!(!uncapped.contains(&ObsRef::Passive(ObsId(8))));
    assert!(
        uncapped.contains(&ObsRef::Passive(ObsId(6))),
        "the benign reading at the site"
    );
    assert!(
        uncapped.contains(&ObsRef::Passive(ObsId(7))),
        "the free-form message at the site"
    );
    // Capped: the context cannot exceed the cap, and keeps the first quarter (the burst's start).
    let capped = feed.rung.context(id);
    assert!(capped.len() <= 8);
    let mut sorted = capped.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), capped.len(), "no reference repeats");
    // A tighter cap keeps the head and the tail.
    let mut tight = cfg;
    tight.context_max_refs = 4;
    let mut f = Feed::new(&public, tight);
    f.step(10_000, &burst(site, dependent, 9_800));
    f.step(
        14_000,
        &[
            (11_000, counter(site, CounterName::Saturation, 10)),
            (12_000, message(site, CATALOGUE_LIMIT + 7)),
        ],
    );
    let id = f.rung.views(at(14_000))[0].id;
    let ctx = f.rung.context(id);
    assert_eq!(ctx.len(), 4);
    assert_eq!(ctx[0], ObsRef::Passive(ObsId(0)), "the head is kept");
    assert_eq!(
        *ctx.last().unwrap(),
        ObsRef::Passive(ObsId(7)),
        "so is the tail"
    );
}

#[test]
fn a_noticed_anomaly_retires_when_it_has_been_quiet_long_enough() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let cfg = RungConfig::default();
    let mut feed = Feed::new(&public, cfg.clone());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let id = feed.rung.views(at(10_000))[0].id;
    // The last abnormal observation of the burst is at 9,850 ms.
    assert!(
        feed.rung
            .quiet(at(9_850 + cfg.quiet_ns / 2_000_000))
            .is_empty()
    );
    assert!(
        feed.rung
            .quiet(Instant(9_850_000_000 + cfg.quiet_ns))
            .contains(&id)
    );
    feed.rung.retire(id);
    assert!(feed.rung.views(at(20_000)).is_empty());
    assert_eq!(feed.rung.live(), 0);
}

fn declared_diagnosis(p: &Proposed) -> (ObsId, Option<StreamHypothesis>) {
    match &p.action {
        StreamAction::Declare { anchor, diagnosis } => (*anchor, *diagnosis),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_declaration_is_anchored_on_the_evidence_at_the_diagnosed_site() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let view = feed.rung.views(at(10_000)).remove(0);
    let truth_site = StreamHypothesis {
        kind: StreamKind::Known(FaultKind::ResourceExhausted),
        site,
    };
    // Diagnosed at the dependent: anchored on the first attached observation there.
    let at_dependent = StreamHypothesis {
        kind: StreamKind::Known(FaultKind::DependencyDown),
        site: dependent,
    };
    let p = feed
        .rung
        .conclude(view.id, Conclusion::Declare(Some(at_dependent)), false, 0)
        .unwrap();
    assert_eq!(p.source, Source::CheapRung);
    let (anchor, diagnosis) = declared_diagnosis(&p);
    assert_eq!(diagnosis, Some(at_dependent));
    assert_eq!(anchor, ObsId(4), "the dependent's first counter");
    // The rung declares at most once per anomaly.
    assert!(
        feed.rung
            .conclude(view.id, Conclusion::Declare(Some(truth_site)), false, 0)
            .is_none()
    );
}

#[test]
fn a_held_conclusion_is_kept_and_a_reasoners_answer_outranks_it() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let view = feed.rung.views(at(10_000)).remove(0);
    let cheap = StreamHypothesis {
        kind: StreamKind::Known(FaultKind::ResourceExhausted),
        site,
    };
    // Held back (an escalation is in flight): nothing is declared, and the conclusion waits.
    assert!(
        feed.rung
            .conclude(view.id, Conclusion::Declare(Some(cheap)), true, 0)
            .is_none()
    );
    assert_eq!(feed.rung.deferred(view.id), Some(Some(cheap)));
    // The answer arrives and is declared where it was asked; the deferred conclusion is dropped.
    let answer = Some(StreamHypothesis {
        kind: StreamKind::Known(FaultKind::ConfigDrift),
        site,
    });
    assert!(feed.rung.take_answer(view.anchor, answer));
    assert_eq!(feed.rung.deferred(view.id), None);
    assert!(feed.rung.declare_deferred(view.id).is_none());
    // The same answer again is not declared again; a different one is.
    assert!(!feed.rung.take_answer(view.anchor, answer));
    assert!(feed.rung.take_answer(view.anchor, None));
}

#[test]
fn a_released_hold_declares_the_conclusion_once() {
    let public = public_of(&params(0, 150));
    let (site, dependent, _) = picks(&public);
    let mut feed = Feed::new(&public, RungConfig::default());
    feed.step(10_000, &burst(site, dependent, 9_800));
    let view = feed.rung.views(at(10_000)).remove(0);
    let cheap = StreamHypothesis {
        kind: StreamKind::Known(FaultKind::ResourceExhausted),
        site,
    };
    feed.rung
        .conclude(view.id, Conclusion::Declare(Some(cheap)), true, 0);
    let p = feed
        .rung
        .declare_deferred(view.id)
        .expect("the hold lifted");
    assert_eq!(declared_diagnosis(&p).1, Some(cheap));
    assert!(feed.rung.declare_deferred(view.id).is_none(), "once");
    assert!(feed.rung.undecided().is_empty());
}
