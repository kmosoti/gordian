//! The simulated reasoner.
//!
//! It lives on the hidden side. A policy reaches it only through `StreamAction::Escalate`, and
//! gets its answer back as a hypothesis, never as a measurement, after the declared latency.
//!
//! # The law
//!
//! An answer is correct with probability
//!
//! ```text
//! p = sigma(a + b q - c d)
//! ```
//!
//! where `q` is the fraction of the focus incident's decisive evidence that is in the context
//! (counted from hidden labels, over the whole incident, including evidence that has not arrived
//! yet), `d` is the incident's difficulty, and `(a, b, c)` are stream parameters. Nothing else
//! enters: not the size of the context, not how much of it is noise, not whether the question
//! was asked before. A question about an observation that belongs to no incident has `q = 1` and
//! `d` equal to `DifficultySpec::background`, and its correct answer is `None`.
//!
//! # Draws
//!
//! The draws come from ChaCha8 keyed by `(stream seed, incident id, call index)`, where the call
//! index counts the calls *about that incident*. Two policies that ask their first question about
//! the same incident therefore see the same draws, whatever else they asked about, which makes
//! paired comparisons of policies sharper. Three words are drawn in order: the correctness draw,
//! the draw that picks a wrong answer, and nothing else. The correctness draw is compared with
//! `p` as `u < p`.
//!
//! # A wrong answer is a plausible one
//!
//! When the draw says wrong, the answer is picked from a pool of hypotheses that are false but
//! that the evidence could suggest, weighted: for a hard incident, mostly the known kind the
//! cheap rung is led to (the one the first moments imitate), then another hard kind at the same
//! site, the right kind at a neighbouring service, or "not an incident"; for a decoy, the hard
//! kind it imitates or the known kind it is read as; for a plain incident, another kind that its
//! signature leaves open or the right kind next door; for background, a known kind at the
//! observation's service. Never the truth, never noise.
//!
//! # What this assumes, and what it does not model
//!
//! Draws are independent across calls, so asking the same question three times and taking the
//! majority is a sound way to raise accuracy here. A real model's repeated samples are
//! correlated, so any policy that escalates repeatedly benefits more than it should. The law is
//! monotone in `q` by assumption: more of the decisive evidence never hurts, and noise in the
//! context never hurts, only costs. Both are assumptions of the simulated reasoner and are
//! tested against a real model in EXP-106, not here.

use crate::incident::{Family, Incident};
use crate::kinds::{Diagnosis, HardKind, StreamHypothesis, StreamKind, Tier};
use crate::params::ReasonerSpec;
use crate::rng::{Gen, domain, sigmoid};
use crate::stream::Stream;
use gordian_world::{FaultKind, Service, ServiceId};

/// The probability that the reasoner is right: `sigma(a + b q - c d)`.
pub(crate) fn accuracy(spec: &ReasonerSpec, q: f64, d: f64) -> f64 {
    sigmoid(spec.a + spec.b * q - spec.c * d)
}

/// What the reasoner said and the hidden facts behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Verdict {
    pub(crate) diagnosis: Diagnosis,
    pub(crate) p: f64,
    pub(crate) correct: bool,
}

fn neighbours(services: &[Service], site: ServiceId) -> Vec<ServiceId> {
    let mut out: Vec<ServiceId> = Vec::new();
    for s in services {
        let linked = s.id != site
            && (s.depends_on.contains(&site) || services[site.index()].depends_on.contains(&s.id));
        if linked {
            out.push(s.id);
        }
    }
    if out.is_empty() {
        let other = ServiceId(((site.index() + 1) % services.len()) as u32);
        out.push(other);
    }
    out
}

fn known(kind: FaultKind, site: ServiceId) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Known(kind),
        site,
    })
}

fn hard(kind: HardKind, site: ServiceId) -> Diagnosis {
    Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site,
    })
}

/// The weighted pool of plausible wrong answers about `inc`, or about a background observation
/// at `focus_site` when `inc` is `None`.
fn wrong_pool(
    services: &[Service],
    inc: Option<&Incident>,
    focus_site: ServiceId,
) -> Vec<(Diagnosis, f64)> {
    let mut pool: Vec<(Diagnosis, f64)> = Vec::new();
    match inc {
        None => {
            for k in FaultKind::ALL {
                pool.push((known(k, focus_site), 1.0));
            }
        }
        Some(inc) => {
            let site = inc.site;
            let near = neighbours(services, site);
            let w_near = 2.0 / near.len() as f64;
            match (inc.tier, inc.family) {
                (Tier::Plain, Family::Known { kind, duo }) => {
                    for k in FaultKind::ALL.into_iter().filter(|k| *k != kind) {
                        let open =
                            matches!(k, FaultKind::ResourceExhausted | FaultKind::DependencyDown);
                        let w = if duo && open {
                            6.0
                        } else if duo {
                            0.5
                        } else {
                            1.0
                        };
                        pool.push((known(k, site), w));
                    }
                    for n in &near {
                        pool.push((known(kind, *n), w_near));
                    }
                    pool.push((None, 1.0));
                }
                (Tier::Hard, f) => {
                    let hk = f.hard_kind().expect("hard family");
                    if let Some(m) = f.mimic() {
                        pool.push((known(m, site), 4.0));
                    }
                    for other in HardKind::ALL.into_iter().filter(|k| *k != hk) {
                        pool.push((hard(other, site), 1.0));
                    }
                    for n in &near {
                        pool.push((hard(hk, *n), w_near));
                    }
                    pool.push((None, 2.0));
                }
                (Tier::Decoy, f) => {
                    let hk = f.hard_kind().expect("decoy imitates a hard family");
                    pool.push((hard(hk, site), 4.0));
                    if let Some(m) = f.mimic() {
                        pool.push((known(m, site), 3.0));
                    }
                    for other in HardKind::ALL.into_iter().filter(|k| *k != hk) {
                        pool.push((hard(other, site), 1.0));
                    }
                }
                (Tier::Plain, _) => unreachable!("a plain incident has a known family"),
            }
        }
    }
    let truth = inc.and_then(|i| i.truth);
    pool.retain(|(d, _)| *d != truth);
    pool
}

/// The reasoner's answer to the `call_index`-th question about `incident` (`None` for a question
/// about background at `focus_site`), with decisive fraction `q`.
pub(crate) fn answer(
    stream: &Stream,
    incident: Option<&Incident>,
    focus_site: ServiceId,
    call_index: u32,
    q: f64,
) -> Verdict {
    let spec = &stream.params.reasoner;
    let d = incident.map_or(stream.params.difficulty.background, |i| i.difficulty);
    let p = accuracy(spec, q, d);
    let id = incident.map_or(u32::MAX as u64, |i| i.id as u64);
    let mut g = Gen::keyed(&[stream.params.seed, domain::REASONER, id, call_index as u64]);
    let u_correct = g.unit();
    let u_pick = g.unit();
    let truth = incident.and_then(|i| i.truth);
    if u_correct < p {
        return Verdict {
            diagnosis: truth,
            p,
            correct: true,
        };
    }
    let pool = wrong_pool(&stream.services, incident, focus_site);
    let total: f64 = pool.iter().map(|(_, w)| *w).sum();
    let x = u_pick * total;
    let mut acc = 0.0;
    let mut pick = pool.last().expect("a wrong pool is never empty").0;
    for (d, w) in &pool {
        acc += *w;
        if x < acc {
            pick = *d;
            break;
        }
    }
    Verdict {
        diagnosis: pick,
        p,
        correct: false,
    }
}
