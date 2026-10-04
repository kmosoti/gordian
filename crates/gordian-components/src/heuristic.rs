//! A rule table from symptom pattern to candidate `(FaultKind, site)`.
//!
//! The table is written from the world's public physics (`physics::counters`, `physics::messages`
//! and the characteristic messages) with no learning and no use of how episodes are generated. It
//! is cheap: one pass over the window, then a walk down a short table. It can be wrong, in three
//! known ways:
//!
//! 1. **It guesses the site.** The physics says an observation at a dependent is permitted only
//!    once the site's own `ErrorRate` has appeared. The table takes the earliest high `ErrorRate`
//!    in the window as the site (or, for an upstream-unreachable message, the earliest one among
//!    the message's dependencies). If the window dropped that observation, or another service
//!    alarmed first, the site is wrong.
//! 2. **It reads only what is present.** A negative probe, or the absence of a symptom, changes
//!    nothing here. Ruling candidates out is the estimator's and the verifier's job.
//! 3. **Silence is "no fault".** A non-empty window with no abnormal observation yields the
//!    candidate "no fault". That is right for a no-fault episode and wrong for a fault whose
//!    symptoms have not arrived yet; the physics gives no way to tell them apart (every fault
//!    permits silence).
//!
//! # Table order and ties
//!
//! Rules are tried in the order of the `RULES` table; the first whose symptom is present and whose site
//! can be located decides. Specific symptoms come first, the bare `ErrorRate` last. Where a
//! symptom leaves several kinds possible the candidates are listed in `FaultKind` declaration
//! order, which is a convention and not a judgement about likelihood. Such a tie yields no
//! proposal (see the crate documentation); the candidates are in the entry.

use crate::cost::{affine_ns, compute};
use crate::payload::{HypothesisEntry, Ranked, hypothesis_entry, unique_best};
use crate::symptoms::{
    SLOT_CHANGED_CONFIG, SLOT_PROBE_CREDENTIAL, SLOT_PROBE_RESOURCE, Summary, counter_slot,
    summarize, text_slot,
};
use crate::{
    Component, ComponentOutput, ComputationRequest, HEURISTIC_ID, VERIFIER_ID, WorkingState,
};
use gordian_core::{Charge, ComponentId};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::SignalText;
use gordian_world::{CounterName, FaultKind, Hypothesis, PublicInfo, ServiceId};

// Declared cost, `Resource::Compute` nanoseconds: `A_NS + B_PS * n / 1000` for a window of `n`
// observations.
// Fitted on 2026-10-04, Intel(R) Xeon(R) Processor @ 2.80GHz (4 vCPU VM), `bench` profile, pinned
// to core 2: `taskset -c 2 cargo bench -p gordian-components`, then `calibrate.py` (weighted least
// squares on relative error), constants rounded from the fits of several runs. Ratios and the
// shape of the fit: CALIBRATION.md. Recalibrate after a change to the CPU, the release profile,
// or this component's code.
const A_NS: u64 = 685;
const B_PS: u64 = 3_700;

/// Where a rule puts the site of its candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SiteRule {
    /// The service the symptom was observed at.
    Subject,
    /// The earliest service with a high `ErrorRate` that the subject depends on.
    UpstreamOfSubject,
    /// The earliest service with a high `ErrorRate` in the window; the subject if there is none.
    EarliestError,
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rule {
    /// Short name, written into the entry.
    pub(crate) basis: &'static str,
    /// The symptom slot (see `symptoms`) that triggers the rule.
    pub(crate) slot: usize,
    /// The kinds the symptom leaves possible, in `FaultKind` declaration order.
    pub(crate) kinds: &'static [FaultKind],
    /// Where the candidates' site comes from.
    pub(crate) site: SiteRule,
}

use FaultKind::{ConfigDrift as CD, CredentialExpired as CE, DependencyDown as DD};
use FaultKind::{Intermittent as IN, ResourceExhausted as RE};

/// The rule table, most specific first. Every row is checked against `physics` by a unit test:
/// each listed kind permits the row's symptom at the site or at a dependent.
pub(crate) const RULES: &[Rule] = &[
    Rule {
        basis: "positive resource-usage probe",
        slot: SLOT_PROBE_RESOURCE,
        kinds: &[RE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "positive credential probe",
        slot: SLOT_PROBE_CREDENTIAL,
        kinds: &[CE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "changed configuration hash",
        slot: SLOT_CHANGED_CONFIG,
        kinds: &[CD],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "out-of-resource message",
        slot: text_slot(SignalText::OutOfResource),
        kinds: &[RE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "config-rejected message",
        slot: text_slot(SignalText::ConfigRejected),
        kinds: &[CD],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "service-down message",
        slot: text_slot(SignalText::ServiceDown),
        kinds: &[DD],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "unauthorized message",
        slot: text_slot(SignalText::Unauthorized),
        kinds: &[CE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "flapping message",
        slot: text_slot(SignalText::Flapping),
        kinds: &[IN],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "upstream-unreachable message",
        slot: text_slot(SignalText::UpstreamUnreachable),
        kinds: &[DD],
        site: SiteRule::UpstreamOfSubject,
    },
    Rule {
        basis: "high saturation counter",
        slot: counter_slot(CounterName::Saturation),
        kinds: &[RE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "high auth-failures counter",
        slot: counter_slot(CounterName::AuthFailures),
        kinds: &[CE],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "high restarts counter",
        slot: counter_slot(CounterName::Restarts),
        kinds: &[DD],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "mixed-signals message",
        slot: text_slot(SignalText::MixedSignals),
        kinds: &[DD, IN],
        site: SiteRule::Subject,
    },
    Rule {
        basis: "high latency counter",
        slot: counter_slot(CounterName::Latency),
        kinds: &[RE, DD, IN],
        site: SiteRule::EarliestError,
    },
    Rule {
        basis: "high error-rate counter",
        slot: counter_slot(CounterName::ErrorRate),
        kinds: &[RE, CD, DD, CE, IN],
        site: SiteRule::EarliestError,
    },
];

/// The heuristic over the rule table.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuleHeuristic;

impl RuleHeuristic {
    /// The heuristic.
    pub fn new() -> Self {
        Self
    }
}

fn locate(rule: &Rule, summary: &Summary, public: &PublicInfo) -> Option<ServiceId> {
    let subject = summary.first[rule.slot]?;
    match rule.site {
        SiteRule::Subject => Some(subject),
        SiteRule::EarliestError => Some(summary.first_error().unwrap_or(subject)),
        SiteRule::UpstreamOfSubject => summary.error_order.iter().copied().find(|site| {
            dependents_mask(&public.services, *site)
                .get(subject.index())
                .copied()
                .unwrap_or(false)
        }),
    }
}

/// The rule that decides, and its candidates. `None` when the window shows a symptom whose site
/// cannot be located, or when the window is empty.
fn decide(
    summary: &Summary,
    public: &PublicInfo,
    window: usize,
) -> Option<(&'static str, Vec<Hypothesis>)> {
    if window == 0 {
        return None;
    }
    if summary.mask == 0 {
        return Some(("silence: no abnormal observation", vec![None]));
    }
    RULES
        .iter()
        .filter(|rule| summary.mask & (1 << rule.slot) != 0)
        .find_map(|rule| {
            let site = locate(rule, summary, public)?;
            let candidates = rule.kinds.iter().map(|k| Some((*k, site))).collect();
            Some((rule.basis, candidates))
        })
}

impl Component for RuleHeuristic {
    fn id(&self) -> ComponentId {
        HEURISTIC_ID
    }

    fn declared_cost(&self, input: &WorkingState) -> Vec<Charge> {
        vec![compute(affine_ns(A_NS, B_PS, input.size()))]
    }

    fn run(&mut self, input: &WorkingState) -> ComponentOutput {
        let summary = summarize(&input.public, input.evidence());
        let Some((basis, candidates)) = decide(&summary, &input.public, input.size()) else {
            return ComponentOutput::default();
        };
        let tied = candidates.len() as u32;
        let ranked: Vec<Ranked> = candidates
            .into_iter()
            .map(|hypothesis| Ranked {
                hypothesis,
                score: None,
            })
            .collect();
        let proposal = unique_best(&ranked, tied);
        let mut requests = Vec::new();
        if proposal.is_some() {
            requests.push(ComputationRequest {
                component: VERIFIER_ID,
                reason: "check heuristic proposal".to_string(),
            });
        }
        let entry = HypothesisEntry::Candidates {
            source: "heuristic".to_string(),
            basis: basis.to_string(),
            ranked,
            tied_at_top: tied,
        };
        ComponentOutput {
            entries: vec![hypothesis_entry(&entry)],
            requests,
            proposal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symptoms::{N_SLOTS, tag_slot};
    use gordian_core::Instant;
    use gordian_world::physics::{HIGH, Role, SymptomTag, counters, messages};
    use gordian_world::{
        EpisodeClass, EpisodeSpec, Observation, Probe, ProbeKind, ProbeResult, Severity, generate,
    };
    use std::collections::VecDeque;

    /// For every row, each listed kind really permits the row's symptom under the public rules,
    /// so the table never offers a kind the physics excludes for that symptom, and no kind that
    /// the physics permits for a *characteristic* symptom is left out.
    #[test]
    fn rows_agree_with_the_public_physics() {
        for rule in RULES {
            assert!(rule.slot < N_SLOTS);
            let permits = |kind: FaultKind| -> bool {
                [Role::Site, Role::Dependent].into_iter().any(|role| {
                    for name in CounterName::ALL {
                        if counter_slot(name) == rule.slot && counters(kind, role).contains(&name) {
                            return true;
                        }
                    }
                    for text in SignalText::ALL {
                        if text_slot(text) == rule.slot && messages(kind, role).contains(&text) {
                            return true;
                        }
                    }
                    false
                })
            };
            let is_physics_symptom = rule.slot < SLOT_CHANGED_CONFIG;
            for kind in rule.kinds {
                let ok = if is_physics_symptom {
                    permits(*kind)
                } else if rule.slot == SLOT_CHANGED_CONFIG {
                    *kind == CD
                } else if rule.slot == SLOT_PROBE_RESOURCE {
                    *kind == RE
                } else {
                    *kind == CE
                };
                assert!(
                    ok,
                    "{} lists {kind:?}, which the physics excludes",
                    rule.basis
                );
            }
            if is_physics_symptom {
                // Completeness: the row lists every kind the physics permits for the symptom
                // at the *site* or at a dependent. (UpstreamUnreachable and Latency are
                // permitted in both roles.)
                let all: Vec<FaultKind> =
                    FaultKind::ALL.into_iter().filter(|k| permits(*k)).collect();
                assert_eq!(rule.kinds, &all[..], "{} omits or adds a kind", rule.basis);
            }
        }
    }

    #[test]
    fn rows_are_unique_by_slot_and_in_declaration_order() {
        let mut slots: Vec<usize> = RULES.iter().map(|r| r.slot).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), RULES.len());
        for rule in RULES {
            let positions: Vec<usize> = rule
                .kinds
                .iter()
                .map(|k| FaultKind::ALL.iter().position(|a| a == k).unwrap())
                .collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{}", rule.basis);
        }
        // Every physics tag has a row.
        for name in CounterName::ALL {
            assert!(
                RULES
                    .iter()
                    .any(|r| r.slot == tag_slot(SymptomTag::Counter(name)))
            );
        }
        for text in SignalText::ALL {
            if text != SignalText::CheckHealth {
                assert!(
                    RULES
                        .iter()
                        .any(|r| r.slot == tag_slot(SymptomTag::Text(text)))
                );
            }
        }
    }

    /// The observation that triggers `rule` and nothing earlier in the table, at `subject`.
    fn trigger(rule: &Rule, subject: ServiceId, public: &PublicInfo) -> Observation {
        for name in CounterName::ALL {
            if counter_slot(name) == rule.slot {
                return Observation::Counter {
                    service: subject,
                    name,
                    value: HIGH,
                };
            }
        }
        for text in SignalText::ALL {
            if text_slot(text) == rule.slot {
                return Observation::Message {
                    service: subject,
                    text_id: text.text_id(),
                    severity: Severity::Medium,
                };
            }
        }
        let probe = |kind| Observation::Probed {
            probe: Probe {
                kind,
                target: subject,
            },
            result: ProbeResult::Positive,
        };
        match rule.slot {
            SLOT_CHANGED_CONFIG => Observation::Snapshot {
                service: subject,
                config_hash: public.services[subject.index()].config_hash ^ 1,
            },
            SLOT_PROBE_RESOURCE => probe(ProbeKind::ResourceUsage),
            SLOT_PROBE_CREDENTIAL => probe(ProbeKind::CredentialCheck),
            other => panic!("no trigger for slot {other}"),
        }
    }

    /// Each row is reachable and no other row shadows it: with the site's `ErrorRate` as an
    /// anchor plus the row's own symptom, the row decides, and its candidates are its kinds at
    /// the site. A reordering of the table that lets a generic row (the bare `ErrorRate`, or
    /// latency) fire before a specific one fails here.
    #[test]
    fn every_row_decides_when_its_symptom_is_present() {
        let public = generate(&EpisodeSpec::new(3, EpisodeClass::Ambiguous)).public_info();
        let dependent = public
            .services
            .iter()
            .find(|s| !s.depends_on.is_empty())
            .unwrap();
        let (dependent, site) = (dependent.id, dependent.depends_on[0]);
        for rule in RULES {
            let subject = match rule.site {
                SiteRule::UpstreamOfSubject => dependent,
                _ => site,
            };
            let anchor = Observation::Counter {
                service: site,
                name: CounterName::ErrorRate,
                value: HIGH,
            };
            let evidence: VecDeque<_> = [
                (Instant(1), anchor),
                (Instant(2), trigger(rule, subject, &public)),
            ]
            .into();
            let summary = summarize(&public, &evidence);
            let (basis, candidates) =
                decide(&summary, &public, evidence.len()).expect("a rule decides");
            assert_eq!(basis, rule.basis);
            let expected: Vec<Hypothesis> = rule.kinds.iter().map(|k| Some((*k, site))).collect();
            assert_eq!(candidates, expected, "{}", rule.basis);
        }
    }
}
