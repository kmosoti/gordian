//! What a probe returns in the stream.
//!
//! A plain incident answers exactly as the first world does (`physics::probe_result`). The hard
//! kinds answer by rules that are not in `physics`, which is what lets a probe mislead the cheap
//! rung: it confirms a known kind that is only half of what is wrong, or it confirms a pattern
//! the cheap rung reads as a different kind. A decoy answers as the hard kind it imitates for as
//! long as it is live, which is the same as a hard incident does, and as a healthy service after.
//! A service that belongs to no live incident answers as a healthy one.
//!
//! | model | at the site | elsewhere |
//! |---|---|---|
//! | plain, kind `k` | the first world's table with truth `(k, site)` | healthy |
//! | compound `(a, b)` | `HealthCheck` positive; each of `a` and `b` answers its dedicated probe positive; the parity samples negative | healthy |
//! | cascade `(a, partner)` | as plain kind `a` | at the partner: `HealthCheck` positive, all else healthy |
//! | split brain | `HealthCheck` positive and both parity samples positive, which the first world's parity rule reads as `DependencyDown` | the same at the peer |
//! | leak | `HealthCheck` positive; `ResourceUsage` positive from the first reading at or above the threshold | healthy |
//!
//! The answers of a probe do not depend on the tier: a decoy and a hard incident of one family
//! answer identically until the decoy resolves, which is at least [`crate::timing::T0_NS`] after
//! onset. Probe cost and `ready_at` depend on the probe alone.

use crate::incident::{Family, Incident};
use gordian_core::Instant;
use gordian_world::physics::probe_result;
use gordian_world::{FaultKind, Probe, ProbeKind, ProbeResult, Service};

fn healthy(services: &[Service], probe: Probe) -> ProbeResult {
    probe_result(services, None, (false, false), 0, probe)
}

fn verdict(b: bool) -> ProbeResult {
    if b {
        ProbeResult::Positive
    } else {
        ProbeResult::Negative
    }
}

/// True while the incident affects what a probe sees.
pub(crate) fn live(inc: &Incident, now: Instant) -> bool {
    inc.onset <= now && now < inc.live_end
}

/// The result of `probe` at `now` against `inc`, whose services must include the probe's target.
/// `services` is the graph at time zero (public hashes).
pub(crate) fn answer(
    inc: &Incident,
    services: &[Service],
    probe: Probe,
    now: Instant,
) -> ProbeResult {
    let target = probe.target;
    if !live(inc, now) || !inc.occupies.contains(&target) {
        return healthy(services, probe);
    }
    let at_site = target == inc.site;
    match inc.family {
        Family::Known { kind, .. } => probe_result(
            services,
            Some((kind, inc.site)),
            inc.bits,
            inc.drift_hash,
            probe,
        ),
        Family::Compound { a, b, .. } => {
            if !at_site {
                return healthy(services, probe);
            }
            let has = |k: FaultKind| a == k || b == k;
            match probe.kind {
                ProbeKind::HealthCheck => ProbeResult::Positive,
                ProbeKind::ResourceUsage => verdict(has(FaultKind::ResourceExhausted)),
                ProbeKind::CredentialCheck => verdict(has(FaultKind::CredentialExpired)),
                ProbeKind::ConfigSnapshot => {
                    if has(FaultKind::ConfigDrift) {
                        ProbeResult::ConfigHash(inc.drift_hash)
                    } else {
                        healthy(services, probe)
                    }
                }
                ProbeKind::LatencySample | ProbeKind::ErrorSample => ProbeResult::Negative,
            }
        }
        Family::Cascade { a, .. } => {
            if at_site {
                probe_result(
                    services,
                    Some((a, inc.site)),
                    inc.bits,
                    inc.drift_hash,
                    probe,
                )
            } else if probe.kind == ProbeKind::HealthCheck {
                ProbeResult::Positive
            } else {
                healthy(services, probe)
            }
        }
        Family::SplitBrain { .. } => match probe.kind {
            ProbeKind::HealthCheck | ProbeKind::LatencySample | ProbeKind::ErrorSample => {
                ProbeResult::Positive
            }
            _ => healthy(services, probe),
        },
        Family::Leak => {
            if !at_site {
                return healthy(services, probe);
            }
            match probe.kind {
                ProbeKind::HealthCheck => ProbeResult::Positive,
                ProbeKind::ResourceUsage => verdict(inc.cross.is_some_and(|c| now >= c)),
                _ => healthy(services, probe),
            }
        }
    }
}
