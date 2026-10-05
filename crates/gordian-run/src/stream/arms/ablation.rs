//! `ablation_hidden_rules`: the cheap rung with the stream's hidden rules injected.
//!
//! **This is an ablation, not a baseline, and never an arm of a comparison.** It exists so that
//! the headroom check (R4) can show what knowledge of the hidden rules is worth: the gap between
//! it and the plain cheap rung is what the hard tier asks of knowledge alone, and the gap between
//! it and the reasoner is what the reasoner adds beyond that knowledge (`DESIGN.md` of the stream
//! world, sections 4.2 and 12). Every other file in this directory is forbidden to know what this
//! one knows; the manifest rejects the arm unless its name contains `ablation`, and every
//! `results.csv` row carries `arm_role = ablation`. A test checks that this is the only arm file
//! that overrides [`EscalationRule::recognize`] and the only one that names the hard kinds.
//!
//! # What it knows
//!
//! Read from the stream's design record, which is experimenter knowledge: hard incidents
//! (compounds, cascades, split brains and slow leaks) break the first world's public rules in
//! recognisable ways, and the arm encodes one recogniser per family. It escalates nothing. It is
//! the shared rung (the same noticing, the same components, the same shared rule, the same
//! declaring procedure) plus [`HiddenRules::recognize`], which declares a hard incident as soon
//! as its pattern is complete and outranks the shared rule's conclusion for that anomaly.
//!
//! - *Cascade*: in the burst that opens an anomaly, two or more abnormal counters at a service
//!   the public graph does not connect to the anomaly's site (a hidden edge), which no other
//!   incident was already alarming. The site is the anomaly's: the first to alarm.
//! - *Split brain*: `MixedSignals` at the anomaly's site and at another service within the same
//!   burst, which no single fault explains.
//! - *Compound*: at the site, within the burst, counters that no single known kind permits, or
//!   the characteristic messages of two known kinds; or, later, a second known kind's
//!   characteristic message at the site together with the hidden vocabulary (below).
//! - *Slow leak*: a series of benign `Saturation` readings at the site that rose steadily before
//!   the first alarm there.
//! - *The hidden vocabulary*: a hard incident's second phase brings 3 to 5 free-form messages
//!   at its site within 10 s; background brings them at a service at about 0.15 a second. The
//!   arm treats five or more at the site in the window `[6 s, 17 s]` after the anchor as the
//!   second phase of a hard incident.
//!
//! What it does not know, and therefore cannot do: which ids belong to which hard kind (a fresh
//! random pool per stream, which the world intends to be learnable and which this arm does not
//! learn), which family a hard incident is when only its second phase shows, the reasoner's
//! answers, or anything about which incident is a decoy. It cannot see a slow leak until the
//! leak crosses the alarm threshold, because the shared rung only notices anomalies from abnormal
//! observations.
//!
//! # How far to trust it
//!
//! These are recognisers written from a description, by someone who also read the world's
//! checks, and they are simple. Their accuracy is measured (`tests/stream_arms.rs`, against the
//! truth, in tests only), and it is part of what R4 reports: an ablation that recognises half the
//! hard incidents says what half-knowledge is worth, not what knowledge is worth.

use super::rung::{AnomalyView, Held, service_of};
use super::{EscalationRule, RecognizeCtx};
use crate::policy::PolicyId;
use gordian_core::Instant;
use gordian_stream::{Diagnosis, HardKind, StreamHypothesis, StreamKind};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{
    CATALOGUE_LIMIT, HIGH, Role, SignalText, characteristic_message, counters,
};
use gordian_world::{CounterName, FaultKind, Observation, ServiceId};
use std::collections::BTreeSet;

/// The id this arm is registered under.
pub const ID: &str = "ablation_hidden_rules";

/// The burst window after an anomaly's anchor, nanoseconds.
const BURST_NS: u64 = 400_000_000;

/// Free-form messages at the site, in the second phase's window, that make a hard incident.
const HIDDEN_VOCABULARY: usize = 5;

/// The rule: never escalate, but recognise the hidden rules.
#[derive(Debug, Clone, Copy, Default)]
pub struct HiddenRules;

impl HiddenRules {
    /// The ablation.
    pub fn new() -> Self {
        Self
    }
}

fn hard(kind: HardKind, site: ServiceId) -> Option<Diagnosis> {
    Some(Some(StreamHypothesis {
        kind: StreamKind::Hard(kind),
        site,
    }))
}

fn is_free_form(h: &Held) -> bool {
    matches!(&h.obs, Observation::Message { text_id, .. } if *text_id >= CATALOGUE_LIMIT)
}

impl EscalationRule for HiddenRules {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn role(&self) -> super::ArmRole {
        super::ArmRole::Ablation
    }

    fn targets(&mut self, _now: Instant, _views: &[AnomalyView]) -> Vec<u32> {
        Vec::new()
    }

    fn revises(&self) -> bool {
        true
    }

    fn recognize(&self, ctx: &RecognizeCtx<'_>) -> Option<Diagnosis> {
        let site = ctx.view.site;
        let start = ctx.view.anchor_at.0;
        let burst: Vec<&Held> = ctx
            .store
            .iter()
            .filter(|h| h.abnormal && h.at.0 >= start && h.at.0 <= start + BURST_NS)
            .collect();
        let dependents = dependents_mask(&ctx.public.services, site);
        let connected =
            |s: ServiceId| s == site || dependents.get(s.index()).copied().unwrap_or(false);

        // Cascade: a burst at a service the public graph does not connect to the site, that no
        // other incident was already alarming in the two seconds before.
        let mut stranger_counters: Vec<ServiceId> = Vec::new();
        for h in &burst {
            if let Observation::Counter { service, .. } = &h.obs
                && !connected(*service)
            {
                stranger_counters.push(*service);
            }
        }
        for z in stranger_counters.iter().collect::<BTreeSet<_>>() {
            let in_burst = stranger_counters.iter().filter(|s| *s == z).count();
            let already = ctx.store.iter().any(|h| {
                h.abnormal
                    && service_of(&h.obs) == Some(*z)
                    && h.at.0 + 2_000_000_000 < start
                    && h.at.0 + 6_000_000_000 > start
            });
            if in_burst >= 2 && !already {
                return hard(HardKind::Cascade, site);
            }
        }

        // Split brain: `MixedSignals` at the site and at another service in the same burst.
        let mixed: BTreeSet<ServiceId> = burst
            .iter()
            .filter_map(|h| match &h.obs {
                Observation::Message {
                    service, text_id, ..
                } if SignalText::from_text_id(*text_id) == Some(SignalText::MixedSignals) => {
                    Some(*service)
                }
                _ => None,
            })
            .collect();
        if mixed.contains(&site) && mixed.len() >= 2 {
            return hard(HardKind::SplitBrain, site);
        }

        // Compound, in the burst: counters at the site that no single known kind permits, or the
        // characteristic messages of two known kinds.
        let site_counters: BTreeSet<CounterName> = burst
            .iter()
            .filter_map(|h| match &h.obs {
                Observation::Counter {
                    service,
                    name,
                    value,
                } if *service == site && *value >= HIGH => Some(*name),
                _ => None,
            })
            .collect();
        let characteristic_at_site = |from: u64, to: u64| -> BTreeSet<FaultKind> {
            ctx.store
                .iter()
                .filter(|h| h.abnormal && h.at.0 >= from && h.at.0 <= to)
                .filter_map(|h| match &h.obs {
                    Observation::Message {
                        service, text_id, ..
                    } if *service == site => {
                        let text = SignalText::from_text_id(*text_id)?;
                        FaultKind::ALL
                            .into_iter()
                            .find(|k| characteristic_message(*k) == text)
                    }
                    _ => None,
                })
                .collect()
        };
        let unexplained = !site_counters.is_empty()
            && !FaultKind::ALL.iter().any(|k| {
                site_counters
                    .iter()
                    .all(|c| counters(*k, Role::Site).contains(c))
            });
        if unexplained || characteristic_at_site(start, start + BURST_NS).len() >= 2 {
            return hard(HardKind::Compound, site);
        }

        // The second phase: five or more free-form messages at the site in [6 s, 17 s] after the
        // anchor, with a second known kind's characteristic message at the site.
        let late = ctx
            .store
            .iter()
            .filter(|h| {
                h.at.0 >= start + 6_000_000_000
                    && h.at.0 <= start + 17_000_000_000
                    && service_of(&h.obs) == Some(site)
                    && is_free_form(h)
            })
            .count();
        if late >= HIDDEN_VOCABULARY
            && characteristic_at_site(start, start + 17_000_000_000).len() >= 2
        {
            return hard(HardKind::Compound, site);
        }

        // Slow leak: benign saturation readings at the site rising before the first alarm.
        let ramp: Vec<(u64, u64)> = ctx
            .store
            .iter()
            .filter(|h| h.at.0 < start && h.at.0 + 40_000_000_000 > start)
            .filter_map(|h| match &h.obs {
                Observation::Counter {
                    service,
                    name: CounterName::Saturation,
                    value,
                } if *service == site && *value < HIGH => Some((h.at.0, *value)),
                _ => None,
            })
            .collect();
        if ramp.len() >= 4 && ramp.windows(2).all(|w| w[1].1 >= w[0].1) {
            let (t0, v0) = ramp[0];
            let (t1, v1) = ramp[ramp.len() - 1];
            if t1 > t0 && v1 > v0 && (v1 - v0) as f64 / ((t1 - t0) as f64 / 1e9) >= 1.5 {
                return hard(HardKind::SlowLeak, site);
            }
        }
        None
    }
}
