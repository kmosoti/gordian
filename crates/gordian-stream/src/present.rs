//! Builders for the first seconds of an incident: the shapes of evidence that the public physics
//! reads. Every function draws from the generator it is given in an order that does not depend on
//! the incident's tier (the caller passes a generator keyed by the incident's arrival index, and
//! a hard incident and a decoy of the same family make the same calls).

use crate::params::MS;
use crate::regime::Physics;
use crate::rng::Gen;
use gordian_world::physics::{HIGH, Role, SignalText, counters, messages};
use gordian_world::{CounterName, FaultKind, Observation, ServiceId, Severity};

/// One piece of evidence, `off_ns` after a reference instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sig {
    pub(crate) off_ns: u64,
    pub(crate) obs: Observation,
}

/// An abnormal counter value.
pub(crate) fn hot(g: &mut Gen) -> u64 {
    g.range(HIGH + 10, 100)
}

/// A benign counter value.
pub(crate) fn calm(g: &mut Gen) -> u64 {
    g.below(HIGH)
}

pub(crate) fn severity(g: &mut Gen) -> Severity {
    Severity::ALL[g.below(4) as usize]
}

pub(crate) fn text(service: ServiceId, t: SignalText, sev: Severity) -> Observation {
    Observation::Message {
        service,
        text_id: t.text_id(),
        severity: sev,
    }
}

pub(crate) fn counter(service: ServiceId, name: CounterName, value: u64) -> Observation {
    Observation::Counter {
        service,
        name,
        value,
    }
}

fn at_ms(g: &mut Gen, lo: u64, hi: u64) -> u64 {
    g.range(lo, hi) * MS
}

/// The full signature of every kind in `kinds` at `site`, as the first world's `Identified`
/// profile builds it: one `ErrorRate` anchor at offset zero, each kind's other site counters and
/// characteristic message within 20 ms, and each dependent's counters and messages within 60 ms.
/// With more than one kind the signature is the union, which no single hypothesis explains.
pub(crate) fn identified(
    g: &mut Gen,
    phys: &Physics,
    kinds: &[FaultKind],
    site: ServiceId,
    deps: &[ServiceId],
) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: 0,
        obs: counter(site, CounterName::ErrorRate, hot(g)),
    }];
    let mut seen_counters: Vec<(ServiceId, CounterName)> = vec![(site, CounterName::ErrorRate)];
    let mut seen_texts: Vec<(ServiceId, SignalText)> = Vec::new();
    for kind in kinds {
        for name in counters(*kind, Role::Site) {
            if seen_counters.contains(&(site, *name)) {
                continue;
            }
            seen_counters.push((site, *name));
            let off_ns = at_ms(g, 1, 20);
            out.push(Sig {
                off_ns,
                obs: counter(site, *name, hot(g)),
            });
        }
        let t = phys.char_message(*kind);
        if !seen_texts.contains(&(site, t)) {
            seen_texts.push((site, t));
            let off_ns = at_ms(g, 1, 20);
            let sev = severity(g);
            out.push(Sig {
                off_ns,
                obs: text(site, t, sev),
            });
        }
    }
    for dep in deps {
        for kind in kinds {
            for name in counters(*kind, Role::Dependent) {
                if seen_counters.contains(&(*dep, *name)) {
                    continue;
                }
                seen_counters.push((*dep, *name));
                let off_ns = at_ms(g, 1, 60);
                out.push(Sig {
                    off_ns,
                    obs: counter(*dep, *name, hot(g)),
                });
            }
            for t in messages(*kind, Role::Dependent) {
                if seen_texts.contains(&(*dep, *t)) {
                    continue;
                }
                seen_texts.push((*dep, *t));
                let off_ns = at_ms(g, 1, 60);
                let sev = severity(g);
                out.push(Sig {
                    off_ns,
                    obs: text(*dep, *t, sev),
                });
            }
        }
    }
    out
}

/// The signature that leaves exactly `ResourceExhausted` and `DependencyDown` open: the anchor,
/// `Latency` at the site, and `ErrorRate` at one dependent. One `ResourceUsage` probe at the site
/// settles it. Requires at least one dependent.
pub(crate) fn duo(g: &mut Gen, site: ServiceId, deps: &[ServiceId]) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: 0,
        obs: counter(site, CounterName::ErrorRate, hot(g)),
    }];
    let off_ns = at_ms(g, 1, 20);
    out.push(Sig {
        off_ns,
        obs: counter(site, CounterName::Latency, hot(g)),
    });
    let dep = deps[g.below(deps.len() as u64) as usize];
    let off_ns = at_ms(g, 21, 60);
    out.push(Sig {
        off_ns,
        obs: counter(dep, CounterName::ErrorRate, hot(g)),
    });
    out
}

/// The first world's `Mixed` profile: the anchor, `Latency` and `MixedSignals` at the site, and
/// `Latency` at up to three dependents. Leaves `DependencyDown` and `Intermittent` open.
pub(crate) fn mixed(g: &mut Gen, site: ServiceId, deps: &[ServiceId]) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: 0,
        obs: counter(site, CounterName::ErrorRate, hot(g)),
    }];
    let off_ns = at_ms(g, 1, 20);
    out.push(Sig {
        off_ns,
        obs: counter(site, CounterName::Latency, hot(g)),
    });
    let off_ns = at_ms(g, 1, 20);
    let sev = severity(g);
    out.push(Sig {
        off_ns,
        obs: text(site, SignalText::MixedSignals, sev),
    });
    for dep in deps.iter().take(3) {
        let off_ns = at_ms(g, 21, 60);
        out.push(Sig {
            off_ns,
            obs: counter(*dep, CounterName::Latency, hot(g)),
        });
    }
    out
}

/// The second member of a split brain alarming: `ErrorRate`, `Latency` and `MixedSignals` at
/// `peer`, starting `base_ns` after the reference instant.
pub(crate) fn peer_alarm(g: &mut Gen, peer: ServiceId, base_ns: u64) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: base_ns,
        obs: counter(peer, CounterName::ErrorRate, hot(g)),
    }];
    let off_ns = base_ns + at_ms(g, 1, 20);
    out.push(Sig {
        off_ns,
        obs: counter(peer, CounterName::Latency, hot(g)),
    });
    let off_ns = base_ns + at_ms(g, 1, 20);
    let sev = severity(g);
    out.push(Sig {
        off_ns,
        obs: text(peer, SignalText::MixedSignals, sev),
    });
    out
}

/// A cascade reaching its partner: `ErrorRate` and `Latency` at `partner` starting `base_ns`
/// after the reference instant, and `ErrorRate` at up to three of the partner's dependents.
pub(crate) fn partner_alarm(
    g: &mut Gen,
    partner: ServiceId,
    partner_deps: &[ServiceId],
    base_ns: u64,
) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: base_ns,
        obs: counter(partner, CounterName::ErrorRate, hot(g)),
    }];
    let off_ns = base_ns + at_ms(g, 1, 20);
    out.push(Sig {
        off_ns,
        obs: counter(partner, CounterName::Latency, hot(g)),
    });
    for dep in partner_deps.iter().take(3) {
        let off_ns = base_ns + at_ms(g, 21, 80);
        out.push(Sig {
            off_ns,
            obs: counter(*dep, CounterName::ErrorRate, hot(g)),
        });
    }
    out
}

/// A noise burst that looks like the first moments of an incident of `kind` at `site`: the
/// anchor and the characteristic message, then nothing.
pub(crate) fn mini_burst(
    g: &mut Gen,
    phys: &Physics,
    kind: FaultKind,
    site: ServiceId,
) -> Vec<Sig> {
    let mut out = vec![Sig {
        off_ns: 0,
        obs: counter(site, CounterName::ErrorRate, hot(g)),
    }];
    let off_ns = at_ms(g, 1, 30);
    let sev = severity(g);
    out.push(Sig {
        off_ns,
        obs: text(site, phys.char_message(kind), sev),
    });
    out
}
