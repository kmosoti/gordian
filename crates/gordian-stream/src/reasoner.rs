//! The simulated reasoner.
//!
//! It lives on the hidden side. A policy reaches it only through `StreamAction::Escalate`, and
//! gets its answer back as a hypothesis, never as a measurement, after the declared latency.
//!
//! # The invariant
//!
//! **The answer depends on the truth only through the decisive evidence in the context.** A real
//! model's broad knowledge helps it interpret evidence; it cannot conjure an answer from none.
//! The reasoner is built so that this holds by construction and not by tuning:
//!
//! - A call is *informed* with probability `h(q, d)`, where `q` is the fraction of the focus
//!   incident's decisive evidence in the context (counted from hidden labels over the whole
//!   incident, including evidence not yet arrived), `d` is the incident's difficulty, and
//!
//!   ```text
//!   h(q, d) = (sigma(a + b q - c d) - sigma(a - c d)) / (1 - sigma(a - c d))
//!   ```
//!
//!   so `h(0, d) = 0` and `h` rises with `q` (for `b > 0`) to the value `sigma(a + b - c d)` would
//!   have had in a law that gave an uninformed call no chance. `(a, b, c)` are the swept parameters.
//! - An informed call answers the truth.
//! - An uninformed call answers a *guess* drawn from [`guess_distribution`], a function of the
//!   context, the focus's service and the public rules and of nothing else: it takes no truth, no
//!   tier, no label. It reads the context's observations near the focus through the first world's
//!   public checker, as the cheap rung would, and picks uniformly among the hypotheses that
//!   checker leaves open and that put the cause at the focus or upstream of it; when it leaves
//!   none (the evidence contradicts every single fault) the guess is "not an incident" or one of
//!   the five known kinds at the focus. A guess never names a hard kind.
//!
//! The accuracy of a call is therefore `p = p0 + (1 - p0) h`, where `p0` is the share of the guess
//! distribution that falls on the truth, the accuracy of a truth-independent guess. At `q = 0` the
//! answer is a function of the context alone, so its mutual information with the truth, given the
//! context, is zero (tested); `p0` is not an extra parameter, it is what the public evidence gets
//! anyone, and is large exactly when the public rules already settle the incident.
//!
//! # Draws, and why repeating a question buys nothing
//!
//! ChaCha8 keyed by `(stream seed, subject, fingerprint)` where the subject is the incident (or,
//! for a question about background, the focus observation) and the fingerprint is a hash of the
//! focus and of the context's references sorted into canonical order. An identical question gets
//! an identical answer, however often it is asked and however the references are ordered.
//!
//! For different contexts about one incident the informed-ness is correlated by a Gaussian copula:
//! `z = sqrt(rho) z_incident + sqrt(1 - rho) z_context`, informed iff `Phi(z) < h`, where
//! `z_incident` is drawn once per incident and `z_context` per fingerprint. Each call's marginal
//! stays `h`. `rho` is a stream parameter (default 0.7). The guess is drawn independently per
//! fingerprint.
//!
//! # What this assumes, and what it does not model
//!
//! That better context gives better answers, monotonically, and that noise in the context costs
//! but never hurts: both are in the law by construction and are tested against a real model in
//! EXP-106, not here. That a model's errors on different contexts of one incident are correlated
//! with a single coefficient `rho` and Gaussian dependence, which is a stand-in. That the guess
//! reads the first world's rules and nothing more. That the reasoner knows the truth when informed
//! and degrades by a logistic in `q` and `d`.

use crate::incident::Incident;
use crate::kinds::{Diagnosis, StreamHypothesis, StreamKind};
use crate::params::ReasonerSpec;
use crate::rng::{Gen, det_norm_inv, domain, sigmoid};
use crate::stream::Stream;
use gordian_core::Instant;
use gordian_world::episode::PublicInfo;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{FaultKind, Hypothesis, Observation, ServiceId};

/// The probability that a call is informed: zero without decisive evidence, rising with `q`.
pub(crate) fn informed_probability(spec: &ReasonerSpec, q: f64, d: f64) -> f64 {
    let base = sigmoid(spec.a - spec.c * d);
    let full = sigmoid(spec.a + spec.b * q - spec.c * d);
    if 1.0 - base <= 1e-15 {
        return 0.0;
    }
    ((full - base) / (1.0 - base)).clamp(0.0, 1.0)
}

/// What the reasoner said and the hidden facts behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Verdict {
    pub(crate) diagnosis: Diagnosis,
    /// The probability the call is informed, `h(q, d)`.
    pub(crate) h: f64,
    /// The share of the guess distribution on the truth.
    pub(crate) p0: f64,
    /// The probability the answer is right, `p0 + (1 - p0) h`.
    pub(crate) p: f64,
    /// Whether this call was informed.
    pub(crate) informed: bool,
    /// Whether the answer equals the truth.
    pub(crate) correct: bool,
}

/// The service an observation is about.
pub(crate) fn service_of(o: &Observation) -> ServiceId {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => *service,
        Observation::Probed { probe, .. } => probe.target,
        Observation::Correction { site, .. } => *site,
    }
}

fn known(h: Hypothesis) -> Diagnosis {
    h.map(|(kind, site)| StreamHypothesis {
        kind: StreamKind::Known(kind),
        site,
    })
}

/// The multiset of hypotheses an uninformed call guesses from, each equally likely. A function of
/// the public rules, the focus's service and the context's observations, and of nothing hidden:
/// it takes no incident, no truth and no label.
pub(crate) fn guess_distribution(
    public: &PublicInfo,
    focus_site: ServiceId,
    context: &[(Instant, Observation)],
) -> Vec<Diagnosis> {
    let services = &public.services;
    let mut upstream = vec![false; services.len()];
    let mut stack = vec![focus_site];
    while let Some(s) = stack.pop() {
        for d in &services[s.index()].depends_on {
            if !upstream[d.index()] {
                upstream[d.index()] = true;
                stack.push(*d);
            }
        }
    }
    let downstream = dependents_mask(services, focus_site);
    let near = |s: ServiceId| s == focus_site || upstream[s.index()] || downstream[s.index()];
    let mut evidence: Vec<(Instant, Observation)> = context
        .iter()
        .filter(|(_, o)| near(service_of(o)))
        .cloned()
        .collect();
    evidence.sort_by_key(|(at, _)| *at);
    let open: Vec<Hypothesis> = consistent_hypotheses(public, &evidence)
        .into_iter()
        .filter(|h| match h {
            None => true,
            Some((_, site)) => *site == focus_site || upstream[site.index()],
        })
        .collect();
    if !open.is_empty() {
        return open.into_iter().map(known).collect();
    }
    let mut fallback: Vec<Diagnosis> = vec![None];
    fallback.extend(
        FaultKind::ALL
            .into_iter()
            .map(|k| known(Some((k, focus_site)))),
    );
    fallback
}

/// A hash of the question: the focus and the context's references in canonical (sorted) order.
/// FNV-1a over fixed-width words, so the same set gives the same value on every platform.
pub(crate) fn fingerprint(focus: u32, sorted_refs: impl IntoIterator<Item = (u8, u32)>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |w: u64| {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(focus as u64);
    for (tag, id) in sorted_refs {
        eat(((tag as u64) << 32) | id as u64);
    }
    h
}

/// The reasoner's answer to the question with this `fingerprint` about `incident` (`None` for a
/// question about background; `subject` then identifies the focus observation), with decisive
/// fraction `q`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn answer(
    stream: &Stream,
    public: &PublicInfo,
    incident: Option<&Incident>,
    subject: u64,
    focus_site: ServiceId,
    context: &[(Instant, Observation)],
    fingerprint: u64,
    q: f64,
) -> Verdict {
    let spec = &stream.params.reasoner;
    let d = incident.map_or(stream.params.difficulty.background, |i| i.difficulty);
    let h = informed_probability(spec, q, d);
    let seed = stream.params.seed;
    let u_incident = Gen::keyed(&[seed, domain::REASONER, subject, 0]).unit_open();
    let mut g = Gen::keyed(&[seed, domain::REASONER, subject, 1, fingerprint]);
    let u_context = g.unit_open();
    let u_guess = g.unit();
    let rho = spec.rho.clamp(0.0, 1.0);
    let z = rho.sqrt() * det_norm_inv(u_incident) + (1.0 - rho).sqrt() * det_norm_inv(u_context);
    let informed = h > 0.0 && z < det_norm_inv(h);

    let truth = incident.and_then(|i| i.truth);
    let guesses = guess_distribution(public, focus_site, context);
    let p0 = guesses.iter().filter(|g| **g == truth).count() as f64 / guesses.len() as f64;
    let diagnosis = if informed {
        truth
    } else {
        guesses[((u_guess * guesses.len() as f64) as usize).min(guesses.len() - 1)]
    };
    Verdict {
        diagnosis,
        h,
        p0,
        p: p0 + (1.0 - p0) * h,
        informed,
        correct: diagnosis == truth,
    }
}
