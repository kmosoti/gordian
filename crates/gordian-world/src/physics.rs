//! The public rules of the world.
//!
//! The *rules* in this module are public knowledge: which symptoms each fault kind produces at
//! its site and at its dependents, what each probe reveals, and what each probe costs. The
//! *instance* (which fault, where, with which hidden bits) is hidden. A policy may read this
//! module; it may not read the instance.
//!
//! # Symptoms
//!
//! An observation is *abnormal* when it is a counter at or above [`HIGH`], a catalogue message
//! other than [`SignalText::CheckHealth`], or a snapshot whose hash differs from the one in the
//! public graph. Benign counters, free-form messages, `CheckHealth` messages and unchanged
//! snapshots are permitted under every hypothesis and carry no information.
//!
//! A fault of kind `k` at site `s` permits abnormal observations as in this table. "Site" means
//! the observation is about `s`; "dependent" means it is about a service that depends on `s`,
//! directly or transitively.
//!
//! | kind | site counters | site messages | dependent counters | dependent messages |
//! |---|---|---|---|---|
//! | ResourceExhausted | ErrorRate, Latency, Saturation | OutOfResource | ErrorRate, Latency | |
//! | ConfigDrift | ErrorRate | ConfigRejected | ErrorRate | |
//! | DependencyDown | ErrorRate, Latency, Restarts | ServiceDown, MixedSignals | ErrorRate, Latency | UpstreamUnreachable |
//! | CredentialExpired | ErrorRate, AuthFailures | Unauthorized | ErrorRate | |
//! | Intermittent | ErrorRate, Latency | Flapping, MixedSignals | Latency | |
//!
//! Plus: a snapshot at `v` whose hash differs from the public one is permitted only by a
//! `ConfigDrift` at `v`.
//!
//! Two structural rules:
//!
//! 1. *Propagation requires a cause.* An abnormal observation about a dependent of `s` is
//!    permitted only if an earlier observation in the evidence is an `ErrorRate` counter at or
//!    above [`HIGH`] at `s` itself.
//! 2. *No fault, no symptoms.* The hypothesis "no fault" permits no abnormal observation.
//!
//! The rules are permissive, never necessary: absence of an observation never contradicts a
//! hypothesis (it may simply not have arrived). The only absence-style constraint is rule 1.
//!
//! # Probes
//!
//! A probe measures the underlying cause, which exists from time zero, so a probe is valid at
//! any instant. Probes against any service other than the fault site behave as if the service
//! were healthy.
//!
//! | probe | result at the fault site | result elsewhere |
//! |---|---|---|
//! | HealthCheck | Positive (any kind) | Negative; `Inconclusive` suggesting itself on a service with `unreliable_health` |
//! | ResourceUsage | Positive iff ResourceExhausted | Negative |
//! | ConfigSnapshot | `ConfigHash(new)` iff ConfigDrift, else `ConfigHash(public hash)` | `ConfigHash(public hash)` |
//! | CredentialCheck | Positive iff CredentialExpired | Negative |
//! | LatencySample | Positive iff bit 1 is set, and the fault is DependencyDown or Intermittent | Negative |
//! | ErrorSample | Positive iff bit 2 is set, same restriction | Negative |
//!
//! The two hidden bits obey the *parity rule*: for a fault of kind [`ENTANGLED`]`.0`
//! (DependencyDown) the bits are equal, for [`ENTANGLED`]`.1` (Intermittent) they differ. Each
//! bit alone is uniform under either kind, so one sample leaves the kinds as likely as before;
//! two samples decide. DependencyDown and Intermittent have no dedicated probe, so the samples
//! are the only probe route to telling them apart when no characteristic message was emitted.
//!
//! # Correction
//!
//! `Correct { site }` resolves a fault located at `site` and does nothing elsewhere; the result
//! is reported as `Observation::Correction`. The pre-generated passive stream is not changed by
//! a correction.
//!
//! # Costs
//!
//! See [`probe_cost`] and [`correct_cost`]. Every probe costs at least one `Resource::Probes` and
//! some `Resource::Time`.

use crate::episode::PublicInfo;
use crate::fault::{FaultKind, Hypothesis};
use crate::graph::{Service, ServiceId, dependents_mask};
use crate::sense::{CounterName, Observation, Probe, ProbeKind, ProbeResult};
use gordian_core::{Charge, Instant, Resource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A counter at or above this value is abnormal.
pub const HIGH: u64 = 50;

/// Message text ids below this are catalogue messages with a public meaning. Others are
/// free-form and meaningless.
pub const CATALOGUE_LIMIT: u64 = 1 << 16;

/// Smallest number of services in an episode.
pub const MIN_SERVICES: u8 = 4;

/// Largest number of services in an episode.
pub const MAX_SERVICES: u8 = 12;

/// The two fault kinds linked by the parity rule: bits equal for `.0`, different for `.1`.
pub const ENTANGLED: (FaultKind, FaultKind) = (FaultKind::DependencyDown, FaultKind::Intermittent);

/// A catalogue message with a public meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SignalText {
    /// Site message of `ResourceExhausted`.
    OutOfResource,
    /// Site message of `ConfigDrift`.
    ConfigRejected,
    /// Site message of `DependencyDown`.
    ServiceDown,
    /// Site message of `CredentialExpired`.
    Unauthorized,
    /// Site message of `Intermittent`.
    Flapping,
    /// Dependent message of `DependencyDown`.
    UpstreamUnreachable,
    /// Site message permitted by `DependencyDown` and `Intermittent` alike.
    MixedSignals,
    /// A suggestion to check a service's health. Carries no diagnostic meaning.
    CheckHealth,
}

impl SignalText {
    /// Every catalogue message.
    pub const ALL: [SignalText; 8] = [
        SignalText::OutOfResource,
        SignalText::ConfigRejected,
        SignalText::ServiceDown,
        SignalText::Unauthorized,
        SignalText::Flapping,
        SignalText::UpstreamUnreachable,
        SignalText::MixedSignals,
        SignalText::CheckHealth,
    ];

    /// The text id of this catalogue message.
    pub fn text_id(self) -> u64 {
        0x100
            + match self {
                SignalText::OutOfResource => 0,
                SignalText::ConfigRejected => 1,
                SignalText::ServiceDown => 2,
                SignalText::Unauthorized => 3,
                SignalText::Flapping => 4,
                SignalText::UpstreamUnreachable => 5,
                SignalText::MixedSignals => 6,
                SignalText::CheckHealth => 7,
            }
    }

    /// The catalogue message with this id, or `None` for free-form text.
    pub fn from_text_id(id: u64) -> Option<SignalText> {
        SignalText::ALL.into_iter().find(|t| t.text_id() == id)
    }
}

/// Where an observation is relative to the fault site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The observation is about the fault site itself.
    Site,
    /// The observation is about a service depending on the fault site.
    Dependent,
}

/// Abnormal counters a fault of `kind` permits at a service in `role`.
pub fn counters(kind: FaultKind, role: Role) -> &'static [CounterName] {
    use CounterName::*;
    match (kind, role) {
        (FaultKind::ResourceExhausted, Role::Site) => &[ErrorRate, Latency, Saturation],
        (FaultKind::ResourceExhausted, Role::Dependent) => &[ErrorRate, Latency],
        (FaultKind::ConfigDrift, _) => &[ErrorRate],
        (FaultKind::DependencyDown, Role::Site) => &[ErrorRate, Latency, Restarts],
        (FaultKind::DependencyDown, Role::Dependent) => &[ErrorRate, Latency],
        (FaultKind::CredentialExpired, Role::Site) => &[ErrorRate, AuthFailures],
        (FaultKind::CredentialExpired, Role::Dependent) => &[ErrorRate],
        (FaultKind::Intermittent, Role::Site) => &[ErrorRate, Latency],
        (FaultKind::Intermittent, Role::Dependent) => &[Latency],
    }
}

/// Catalogue messages a fault of `kind` permits at a service in `role`.
pub fn messages(kind: FaultKind, role: Role) -> &'static [SignalText] {
    use SignalText::*;
    match (kind, role) {
        (FaultKind::ResourceExhausted, Role::Site) => &[OutOfResource],
        (FaultKind::ConfigDrift, Role::Site) => &[ConfigRejected],
        (FaultKind::DependencyDown, Role::Site) => &[ServiceDown, MixedSignals],
        (FaultKind::DependencyDown, Role::Dependent) => &[UpstreamUnreachable],
        (FaultKind::CredentialExpired, Role::Site) => &[Unauthorized],
        (FaultKind::Intermittent, Role::Site) => &[Flapping, MixedSignals],
        _ => &[],
    }
}

/// The site message that only `kind` permits. Seeing it at a service fixes kind and site.
pub fn characteristic_message(kind: FaultKind) -> SignalText {
    match kind {
        FaultKind::ResourceExhausted => SignalText::OutOfResource,
        FaultKind::ConfigDrift => SignalText::ConfigRejected,
        FaultKind::DependencyDown => SignalText::ServiceDown,
        FaultKind::CredentialExpired => SignalText::Unauthorized,
        FaultKind::Intermittent => SignalText::Flapping,
    }
}

/// Declared cost of a probe. Every probe costs at least one probe and some time.
///
/// | probe | probes | time |
/// |---|---|---|
/// | HealthCheck | 1 | 1 ms |
/// | ResourceUsage | 1 | 2 ms |
/// | ConfigSnapshot | 1 | 3 ms |
/// | CredentialCheck | 1 | 2 ms |
/// | LatencySample | 2 | 8 ms |
/// | ErrorSample | 2 | 8 ms |
pub fn probe_cost(kind: ProbeKind) -> Vec<Charge> {
    const MS: u64 = 1_000_000;
    let (probes, time) = match kind {
        ProbeKind::HealthCheck => (1, MS),
        ProbeKind::ResourceUsage => (1, 2 * MS),
        ProbeKind::ConfigSnapshot => (1, 3 * MS),
        ProbeKind::CredentialCheck => (1, 2 * MS),
        ProbeKind::LatencySample | ProbeKind::ErrorSample => (2, 8 * MS),
    };
    vec![
        Charge::new(Resource::Probes, probes),
        Charge::new(Resource::Time, time),
    ]
}

/// Declared cost of a correction attempt: 3 probes and 50 ms.
pub fn correct_cost() -> Vec<Charge> {
    vec![
        Charge::new(Resource::Probes, 3),
        Charge::new(Resource::Time, 50_000_000),
    ]
}

/// What a probe returns, given the truth. This is the single definition of probe semantics: the
/// simulator calls it with the hidden state, and the consistency checker calls it with each
/// candidate hypothesis.
///
/// `bits` are the two hidden parity bits. `drift_hash` is the configuration hash a `ConfigDrift`
/// fault leaves at its site; it is ignored otherwise. The target must be a service in
/// `services`; an unknown target behaves as a healthy service with hash zero.
pub fn probe_result(
    services: &[Service],
    truth: Hypothesis,
    bits: (bool, bool),
    drift_hash: u64,
    probe: Probe,
) -> ProbeResult {
    let service = services.get(probe.target.index());
    let at_target =
        |pred: fn(FaultKind) -> bool| truth.is_some_and(|(k, s)| s == probe.target && pred(k));
    let verdict = |b: bool| {
        if b {
            ProbeResult::Positive
        } else {
            ProbeResult::Negative
        }
    };
    let entangled = |k: FaultKind| k == ENTANGLED.0 || k == ENTANGLED.1;
    match probe.kind {
        ProbeKind::HealthCheck => {
            if service.is_some_and(|s| s.unreliable_health) {
                ProbeResult::Inconclusive {
                    suggest: Some(probe),
                }
            } else {
                verdict(at_target(|_| true))
            }
        }
        ProbeKind::ResourceUsage => verdict(at_target(|k| k == FaultKind::ResourceExhausted)),
        ProbeKind::CredentialCheck => verdict(at_target(|k| k == FaultKind::CredentialExpired)),
        ProbeKind::ConfigSnapshot => {
            let drifted = at_target(|k| k == FaultKind::ConfigDrift);
            let hash = if drifted {
                drift_hash
            } else {
                service.map_or(0, |s| s.config_hash)
            };
            ProbeResult::ConfigHash(hash)
        }
        ProbeKind::LatencySample => verdict(bits.0 && at_target(entangled)),
        ProbeKind::ErrorSample => verdict(bits.1 && at_target(entangled)),
    }
}

/// A symptom tag, with the service forgotten. Used to match against prior records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SymptomTag {
    /// An abnormal counter of this name.
    Counter(CounterName),
    /// A catalogue message (other than `CheckHealth`).
    Text(SignalText),
}

/// The sorted, de-duplicated tags of every abnormal counter and catalogue message in
/// `evidence`. Probe results, snapshots and corrections do not contribute.
pub fn signature(evidence: &[(Instant, Observation)]) -> Vec<SymptomTag> {
    let mut tags: Vec<SymptomTag> = evidence
        .iter()
        .filter_map(|(_, o)| match o {
            Observation::Counter { name, value, .. } if *value >= HIGH => {
                Some(SymptomTag::Counter(*name))
            }
            Observation::Message { text_id, .. } => match SignalText::from_text_id(*text_id) {
                Some(SignalText::CheckHealth) | None => None,
                Some(t) => Some(SymptomTag::Text(t)),
            },
            _ => None,
        })
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn role_of(masks: &[Vec<bool>], site: ServiceId, service: ServiceId) -> Option<Role> {
    if service == site {
        Some(Role::Site)
    } else if masks
        .get(site.index())
        .and_then(|m| m.get(service.index()))
        .copied()
        .unwrap_or(false)
    {
        Some(Role::Dependent)
    } else {
        None
    }
}

/// Every (hypothesis, hidden bits) pair that the evidence does not contradict.
///
/// For a hypothesis outside the parity pair the bits are irrelevant and reported as
/// `(false, false)`. For `DependencyDown` the pair is `(b, b)`, for `Intermittent` `(b, !b)`.
/// Counting these worlds, with a uniform prior over them, gives the posterior over kinds.
/// Evidence that refers to a service not in the graph contradicts everything.
///
/// Returns exactly what [`consistent_worlds_reference`] returns, in the same order, for every
/// input; the equivalence is tested, not proved. Complexity is the same as
/// [`consistent_hypotheses`], whose documentation has the argument: time
/// `O(s * n * log s + s * (s + e))`, space `O(n + s^2)`.
pub fn consistent_worlds(
    public: &PublicInfo,
    evidence: &[(Instant, Observation)],
) -> Vec<(Hypothesis, (bool, bool))> {
    let services = &public.services;
    let n = services.len();
    let valid = |id: ServiceId| id.index() < n;

    // Pass 1: reject dangling references, keep only observations that can discriminate, and
    // collect the (single) drifted configuration hash per service.
    let mut drift: BTreeMap<ServiceId, u64> = BTreeMap::new();
    let mut note_drift =
        |service: ServiceId, hash: u64| -> bool { *drift.entry(service).or_insert(hash) == hash };
    let mut informative: Vec<(Instant, &Observation)> = Vec::new();
    for (t, o) in evidence {
        match o {
            Observation::Counter { service, value, .. } => {
                if !valid(*service) {
                    return Vec::new();
                }
                if *value >= HIGH {
                    informative.push((*t, o));
                }
            }
            Observation::Message {
                service, text_id, ..
            } => {
                if !valid(*service) {
                    return Vec::new();
                }
                match SignalText::from_text_id(*text_id) {
                    None | Some(SignalText::CheckHealth) => {}
                    Some(_) => informative.push((*t, o)),
                }
            }
            Observation::Snapshot {
                service,
                config_hash,
            } => {
                if !valid(*service) {
                    return Vec::new();
                }
                if *config_hash != services[service.index()].config_hash {
                    if !note_drift(*service, *config_hash) {
                        return Vec::new();
                    }
                    informative.push((*t, o));
                }
            }
            Observation::Probed { probe, result } => {
                if !valid(probe.target) {
                    return Vec::new();
                }
                if let (ProbeKind::ConfigSnapshot, ProbeResult::ConfigHash(h)) =
                    (probe.kind, result)
                    && *h != services[probe.target.index()].config_hash
                    && !note_drift(probe.target, *h)
                {
                    return Vec::new();
                }
                informative.push((*t, o));
            }
            Observation::Correction { site, .. } => {
                if !valid(*site) {
                    return Vec::new();
                }
                informative.push((*t, o));
            }
        }
    }

    let masks: Vec<Vec<bool>> = (0..n)
        .map(|s| dependents_mask(services, ServiceId(s as u32)))
        .collect();

    let explains = |h: Hypothesis, bits: (bool, bool)| -> bool {
        // The earliest instant of an `ErrorRate` alarm at `h`'s site among the observations
        // already passed. `anchored` at observation `i` is "some `j < i` is such an alarm with
        // `t_j <= t_i`", which is `min t_j <= t_i`; the minimum is kept as the pass goes on, so
        // no observation is looked at twice. (The reference rescans `informative[..i]` instead.)
        let mut earliest_anchor: Option<Instant> = None;
        for (t, o) in informative.iter() {
            let anchored = earliest_anchor.is_some_and(|a| a <= *t);
            let ok = match o {
                Observation::Counter { service, name, .. } => h.is_some_and(|(k, s)| {
                    role_of(&masks, s, *service).is_some_and(|role| {
                        counters(k, role).contains(name) && (role == Role::Site || anchored)
                    })
                }),
                Observation::Message {
                    service, text_id, ..
                } => match (SignalText::from_text_id(*text_id), h) {
                    (Some(text), Some((k, s))) => {
                        role_of(&masks, s, *service).is_some_and(|role| {
                            messages(k, role).contains(&text) && (role == Role::Site || anchored)
                        })
                    }
                    _ => false,
                },
                Observation::Snapshot { service, .. } => {
                    h == Some((FaultKind::ConfigDrift, *service))
                }
                Observation::Probed { probe, result } => {
                    let start = services[probe.target.index()].config_hash;
                    let drift_hash = drift
                        .get(&probe.target)
                        .copied()
                        .unwrap_or(start.wrapping_add(1));
                    probe_result(services, h, bits, drift_hash, *probe) == *result
                }
                Observation::Correction { site, resolved } => {
                    h.is_some_and(|(_, s)| s == *site) == *resolved
                }
            };
            if !ok {
                return false;
            }
            if let (
                Some((_, site)),
                Observation::Counter {
                    service,
                    name: CounterName::ErrorRate,
                    ..
                },
            ) = (h, o)
                && *service == site
                && earliest_anchor.is_none_or(|a| *t < a)
            {
                earliest_anchor = Some(*t);
            }
        }
        true
    };

    let mut worlds = Vec::new();
    if explains(None, (false, false)) {
        worlds.push((None, (false, false)));
    }
    for site in 0..n {
        for kind in FaultKind::ALL {
            let h = Some((kind, ServiceId(site as u32)));
            let options: &[(bool, bool)] = if kind == ENTANGLED.0 {
                &[(false, false), (true, true)]
            } else if kind == ENTANGLED.1 {
                &[(false, true), (true, false)]
            } else {
                &[(false, false)]
            };
            for bits in options {
                if explains(h, *bits) {
                    worlds.push((h, *bits));
                }
            }
        }
    }
    worlds
}

/// The set of hypotheses the evidence does not contradict, under the public rules.
///
/// The semantics of the rules above, computed in time linear in the evidence length for a graph
/// of bounded size (see Complexity). The true hypothesis of every generated episode is always in
/// the result for every prefix of its evidence (soundness, tested). Prior records are advisory
/// and play no part here: a record can be stale.
///
/// Order: "no fault" first, then by site, then by kind.
///
/// Returns exactly what [`consistent_hypotheses_reference`] returns for every input (tested
/// against it on generated, adversarial and arbitrary evidence; not proved).
///
/// # Complexity
///
/// Let `n` be the length of `evidence`, `s` the number of services and `e` the number of
/// dependency edges (at most `s^2` unless an edge is repeated). The checker considers
/// `W = 1 + 7s` candidate worlds: no fault, and for each site five kinds, two of them under two
/// settings of the hidden bits.
///
/// - Time `O(s * n * log s + s * (s + e))`: one pass over the evidence to drop uninformative
///   observations and note drifted hashes (`O(1)` per observation, plus a `log s` map lookup for
///   snapshots and probes); `s` dependent-set masks at `O(s + e)` each; then one pass
///   over the informative observations per world, each observation checked in `O(1)` (table
///   lookups of at most a handful of entries, plus a `log s` drift lookup for probes). A world is
///   dropped at its first contradiction, so this is an upper bound. With `s <= MAX_SERVICES`,
///   as in every generated episode, this is linear in `n` with a constant of up to
///   `W = 85` passes, plus a term in `s` and `e` that does not depend on `n`.
/// - Space `O(n + s^2)` beyond the input: the informative observations by reference (at most
///   `n`), the masks (`s^2` booleans), the drift map (`O(s)`), and the result (at most `W`).
///
/// The argument for linearity in `n` is in the pass over the informative observations. The
/// propagation rule asks, for an abnormal observation at a dependent of the world's site, whether
/// an earlier `ErrorRate` alarm at that site exists with an instant no later than the
/// observation's own. That is "the smallest instant among earlier site alarms is at most this
/// one", so the pass keeps that smallest instant in a single variable and answers the question
/// in `O(1)`. [`consistent_hypotheses_reference`] answers it by rescanning the earlier
/// observations and is `O(s * n^2)` when the anchor comes late (`DESIGN.md`, section 8).
pub fn consistent_hypotheses(
    public: &PublicInfo,
    evidence: &[(Instant, Observation)],
) -> Vec<Hypothesis> {
    let mut out: Vec<Hypothesis> = Vec::new();
    for (h, _) in consistent_worlds(public, evidence) {
        if out.last() != Some(&h) {
            out.push(h);
        }
    }
    out
}

/// Every (hypothesis, hidden bits) pair that the evidence does not contradict.
///
/// For a hypothesis outside the parity pair the bits are irrelevant and reported as
/// `(false, false)`. For `DependencyDown` the pair is `(b, b)`, for `Intermittent` `(b, !b)`.
/// Counting these worlds, with a uniform prior over them, gives the posterior over kinds.
/// Evidence that refers to a service not in the graph contradicts everything.
///
/// The oracle for [`consistent_worlds`]: the implementation as it was before the optimization,
/// kept verbatim and never to be optimized. It answers "is there an earlier `ErrorRate` alarm at
/// the site?" by rescanning the earlier observations, which is `O(s * n^2)` in time when the
/// anchor comes late (`DESIGN.md`, section 8). Its output defines what the optimized function
/// must return. A change to the public rules must be made to both functions; the equivalence
/// tests then show whether the two still agree.
pub fn consistent_worlds_reference(
    public: &PublicInfo,
    evidence: &[(Instant, Observation)],
) -> Vec<(Hypothesis, (bool, bool))> {
    let services = &public.services;
    let n = services.len();
    let valid = |id: ServiceId| id.index() < n;

    // Pass 1: reject dangling references, keep only observations that can discriminate, and
    // collect the (single) drifted configuration hash per service.
    let mut drift: BTreeMap<ServiceId, u64> = BTreeMap::new();
    let mut note_drift =
        |service: ServiceId, hash: u64| -> bool { *drift.entry(service).or_insert(hash) == hash };
    let mut informative: Vec<(Instant, &Observation)> = Vec::new();
    for (t, o) in evidence {
        match o {
            Observation::Counter { service, value, .. } => {
                if !valid(*service) {
                    return Vec::new();
                }
                if *value >= HIGH {
                    informative.push((*t, o));
                }
            }
            Observation::Message {
                service, text_id, ..
            } => {
                if !valid(*service) {
                    return Vec::new();
                }
                match SignalText::from_text_id(*text_id) {
                    None | Some(SignalText::CheckHealth) => {}
                    Some(_) => informative.push((*t, o)),
                }
            }
            Observation::Snapshot {
                service,
                config_hash,
            } => {
                if !valid(*service) {
                    return Vec::new();
                }
                if *config_hash != services[service.index()].config_hash {
                    if !note_drift(*service, *config_hash) {
                        return Vec::new();
                    }
                    informative.push((*t, o));
                }
            }
            Observation::Probed { probe, result } => {
                if !valid(probe.target) {
                    return Vec::new();
                }
                if let (ProbeKind::ConfigSnapshot, ProbeResult::ConfigHash(h)) =
                    (probe.kind, result)
                    && *h != services[probe.target.index()].config_hash
                    && !note_drift(probe.target, *h)
                {
                    return Vec::new();
                }
                informative.push((*t, o));
            }
            Observation::Correction { site, .. } => {
                if !valid(*site) {
                    return Vec::new();
                }
                informative.push((*t, o));
            }
        }
    }

    let masks: Vec<Vec<bool>> = (0..n)
        .map(|s| dependents_mask(services, ServiceId(s as u32)))
        .collect();

    let explains = |h: Hypothesis, bits: (bool, bool)| -> bool {
        for (i, (t, o)) in informative.iter().enumerate() {
            let anchored = |site: ServiceId| {
                informative[..i].iter().any(|(t2, o2)| {
                    *t2 <= *t
                        && matches!(o2, Observation::Counter { service, name: CounterName::ErrorRate, .. }
                            if *service == site)
                })
            };
            let ok = match o {
                Observation::Counter { service, name, .. } => h.is_some_and(|(k, s)| {
                    role_of(&masks, s, *service).is_some_and(|role| {
                        counters(k, role).contains(name) && (role == Role::Site || anchored(s))
                    })
                }),
                Observation::Message {
                    service, text_id, ..
                } => match (SignalText::from_text_id(*text_id), h) {
                    (Some(text), Some((k, s))) => {
                        role_of(&masks, s, *service).is_some_and(|role| {
                            messages(k, role).contains(&text) && (role == Role::Site || anchored(s))
                        })
                    }
                    _ => false,
                },
                Observation::Snapshot { service, .. } => {
                    h == Some((FaultKind::ConfigDrift, *service))
                }
                Observation::Probed { probe, result } => {
                    let start = services[probe.target.index()].config_hash;
                    let drift_hash = drift
                        .get(&probe.target)
                        .copied()
                        .unwrap_or(start.wrapping_add(1));
                    probe_result(services, h, bits, drift_hash, *probe) == *result
                }
                Observation::Correction { site, resolved } => {
                    h.is_some_and(|(_, s)| s == *site) == *resolved
                }
            };
            if !ok {
                return false;
            }
        }
        true
    };

    let mut worlds = Vec::new();
    if explains(None, (false, false)) {
        worlds.push((None, (false, false)));
    }
    for site in 0..n {
        for kind in FaultKind::ALL {
            let h = Some((kind, ServiceId(site as u32)));
            let options: &[(bool, bool)] = if kind == ENTANGLED.0 {
                &[(false, false), (true, true)]
            } else if kind == ENTANGLED.1 {
                &[(false, true), (true, false)]
            } else {
                &[(false, false)]
            };
            for bits in options {
                if explains(h, *bits) {
                    worlds.push((h, *bits));
                }
            }
        }
    }
    worlds
}

/// The set of hypotheses the evidence does not contradict, under the public rules.
///
/// This is the reference semantics of the rules above, and the oracle for
/// [`consistent_hypotheses`]: the function as it was before the optimization, behaviour
/// unchanged, built on [`consistent_worlds_reference`]. It is quadratic in the evidence length
/// when the site's anchoring `ErrorRate` comes late (`DESIGN.md`, section 8); do not call it on a
/// hot path. The true hypothesis of every generated episode is always in the result for every
/// prefix of its evidence (soundness, tested). Prior records are advisory and play no part here:
/// a record can be stale.
///
/// Order: "no fault" first, then by site, then by kind.
pub fn consistent_hypotheses_reference(
    public: &PublicInfo,
    evidence: &[(Instant, Observation)],
) -> Vec<Hypothesis> {
    let mut out: Vec<Hypothesis> = Vec::new();
    for (h, _) in consistent_worlds_reference(public, evidence) {
        if out.last() != Some(&h) {
            out.push(h);
        }
    }
    out
}
