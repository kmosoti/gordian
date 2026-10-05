//! Context builders (work item R6): what an arm puts in the reasoner's context.
//!
//! The shared cheap rung builds the context of every escalation, so a builder is an option of the
//! rung, not of an arm: any arm (a comparison arm or a privileged one) can use any of them, and
//! two arms that differ in builder differ in nothing else. The builder an arm uses is the
//! manifest's [`StreamArmSpec::context`](crate::stream::manifest::StreamArmSpec) for that arm, or
//! the manifest's `rung.context` when the arm names none.
//!
//! # What is built, and what is not
//!
//! | Builder | The references it returns |
//! |---|---|
//! | `rung` | the shared rung's own: observations at the anomaly's site from `context_lookback_ns` before its anchor, and at a dependent of the site within the burst window after the anchor. The default; its code is [`super::rung::Rung::context`] and has not changed |
//! | `window` | every held observation of the last `window_ns` before now, at any service, benign and abnormal alike, the `max_refs` most recent, most recent first |
//! | `cooccur` | every held observation, from `context_lookback_ns` before the anomaly's anchor to now, at the anomaly's site and at each service whose abnormal readings *began* within `delta_ns` of the anchor |
//! | `neighbourhood` | every held observation, from `context_lookback_ns` before the anchor to now, at each service within `hops` hops of the anomaly's site in the public dependency graph (edges taken in either direction) |
//!
//! `cooccur` and `neighbourhood` keep, when more than `max_refs` qualify, the first quarter and the
//! most recent rest, as the rung does. A service's abnormal readings *began* at an abnormal
//! observation that is not preceded, at that service, by another abnormal observation within
//! `burst_gap_ns` (the rung's own silence threshold for a new burst), so a service that has been
//! alarming for a while does not co-occur with a newer anomaly.
//!
//! # What a builder reads
//!
//! A [`PublicView`]: the observations the rung holds (with the public rules' verdict on each, the
//! rung's own), the public dependency graph, the instant of the call, and the anomaly's anchor
//! instant and site (what the rung attached and noticed, from public observations). Each builder
//! is a pure function of that view and its parameters: no clock, no randomness, no other input. A
//! builder cannot name a tier, a label, a decisive observation or the hidden rules: this file is
//! one of the policy files that `scripts/check-no-oracle.sh` and a test ban from naming them.
//!
//! Not built: any builder that learns, any that reads the reasoner's earlier answers, and any
//! that combines these (a window over the neighbourhood, say). The builder's own computation is
//! not charged in the modelled cost (the rung's context construction never was); it is measured
//! in `measured_sched_ns`.

use super::rung::{Store, service_of};
use gordian_core::Instant;
use gordian_stream::ObsRef;
use gordian_world::{Service, ServiceId};
use serde::{Deserialize, Serialize};

/// How the shared rung builds the context of an escalation. See the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "builder", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextBuilder {
    /// The rung's own context: the anomaly's attached observations. The default.
    #[default]
    Rung,
    /// Every observation of the last `window_ns`, across all services, capped.
    Window {
        /// How far back from the instant of the call, nanoseconds.
        window_ns: u64,
        /// Most references returned.
        max_refs: u32,
    },
    /// Observations at services whose abnormal readings began within `delta_ns` of the anomaly's.
    Cooccur {
        /// The largest gap between the two beginnings, nanoseconds.
        delta_ns: u64,
        /// Most references returned.
        max_refs: u32,
    },
    /// Observations at services within `hops` of the anomaly's site in the public graph.
    Neighbourhood {
        /// The largest number of edges from the site; 0 is the site alone.
        hops: u32,
        /// Most references returned.
        max_refs: u32,
    },
}

impl ContextBuilder {
    /// Whether this is the default builder. The default is not written to a manifest, so a
    /// manifest written before builders existed is the same text as one written now.
    pub fn is_rung(&self) -> bool {
        matches!(self, Self::Rung)
    }

    /// The builder's name, as the manifest and the analysis write it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rung => "rung",
            Self::Window { .. } => "window",
            Self::Cooccur { .. } => "cooccur",
            Self::Neighbourhood { .. } => "neighbourhood",
        }
    }

    /// The cap on references the builder declares, if it has one of its own (`rung` takes the
    /// rung's `context_max_refs`).
    pub fn max_refs(&self) -> Option<u32> {
        match self {
            Self::Rung => None,
            Self::Window { max_refs, .. }
            | Self::Cooccur { max_refs, .. }
            | Self::Neighbourhood { max_refs, .. } => Some(*max_refs),
        }
    }

    /// Check the parameters against the stream's public context limit.
    pub fn validate(&self, max_context: u32) -> Result<(), String> {
        if let Some(cap) = self.max_refs() {
            if cap == 0 {
                return Err(format!(
                    "context {}: max_refs must be at least 1",
                    self.name()
                ));
            }
            if cap > max_context {
                return Err(format!(
                    "context {}: max_refs {cap} exceeds the stream's context limit {max_context}",
                    self.name()
                ));
            }
        }
        if let Self::Window { window_ns: 0, .. } = self {
            return Err("context window: window_ns must be positive".to_owned());
        }
        Ok(())
    }
}

/// Everything a builder may read: public information the rung holds. Nothing else is passed.
#[derive(Debug, Clone, Copy)]
pub struct PublicView<'a> {
    /// The observations the rung holds, in id (time) order.
    pub store: &'a Store,
    /// The public dependency graph.
    pub services: &'a [Service],
    /// The instant of the call (the latest step the rung has taken in).
    pub now: Instant,
    /// When the anomaly's anchor was emitted.
    pub anchor_at: Instant,
    /// The service the anomaly is anchored at.
    pub site: ServiceId,
    /// The rung's silence threshold for a new burst, nanoseconds.
    pub burst_gap_ns: u64,
    /// How far before an anchor a context may reach, nanoseconds.
    pub lookback_ns: u64,
}

/// Keep the first quarter and the most recent rest when `refs` is longer than `cap`.
pub fn cap_head_tail(refs: Vec<ObsRef>, cap: usize) -> Vec<ObsRef> {
    if refs.len() <= cap {
        return refs;
    }
    let head = cap / 4;
    let tail = cap - head;
    let mut kept: Vec<ObsRef> = refs[..head].to_vec();
    kept.extend_from_slice(&refs[refs.len() - tail..]);
    kept
}

/// The context `builder` builds from `view`. `Rung` is not built here (it needs the anomaly's
/// region, which the rung holds): it returns `None`.
pub fn build(builder: &ContextBuilder, view: &PublicView<'_>) -> Option<Vec<ObsRef>> {
    Some(match *builder {
        ContextBuilder::Rung => return None,
        ContextBuilder::Window {
            window_ns,
            max_refs,
        } => window(view, window_ns, max_refs),
        ContextBuilder::Cooccur { delta_ns, max_refs } => cooccur(view, delta_ns, max_refs),
        ContextBuilder::Neighbourhood { hops, max_refs } => neighbourhood(view, hops, max_refs),
    })
}

/// `window`: the `max_refs` most recent observations of the last `window_ns`, most recent first.
pub fn window(view: &PublicView<'_>, window_ns: u64, max_refs: u32) -> Vec<ObsRef> {
    let from = view.now.0.saturating_sub(window_ns);
    let mut refs = Vec::new();
    for held in view.store.iter().rev() {
        if refs.len() >= max_refs as usize || held.at.0 < from {
            break;
        }
        refs.push(ObsRef::Passive(held.id));
    }
    refs
}

/// `cooccur`: observations at the site and at every service whose abnormal readings began within
/// `delta_ns` of the anchor, from the lookback before the anchor to now.
pub fn cooccur(view: &PublicView<'_>, delta_ns: u64, max_refs: u32) -> Vec<ObsRef> {
    let n = view.services.len();
    let mut member = vec![false; n];
    if let Some(slot) = member.get_mut(view.site.index()) {
        *slot = true;
    }
    let lo = view.anchor_at.0.saturating_sub(delta_ns);
    let hi = view.anchor_at.0.saturating_add(delta_ns);
    let mut last_abnormal: Vec<Option<u64>> = vec![None; n];
    for held in view.store.iter() {
        if !held.abnormal {
            continue;
        }
        let Some(s) = service_of(&held.obs).map(ServiceId::index) else {
            continue;
        };
        let Some(prev) = last_abnormal.get_mut(s) else {
            continue;
        };
        let began = prev.is_none_or(|p| held.at.0.saturating_sub(p) >= view.burst_gap_ns);
        if began && (lo..=hi).contains(&held.at.0) {
            member[s] = true;
        }
        *prev = Some(held.at.0);
    }
    from_services(view, &member, max_refs)
}

/// `neighbourhood`: observations at services within `hops` of the site, from the lookback before
/// the anchor to now.
pub fn neighbourhood(view: &PublicView<'_>, hops: u32, max_refs: u32) -> Vec<ObsRef> {
    let member = within_hops(view.services, view.site, hops);
    from_services(view, &member, max_refs)
}

/// The services within `hops` edges of `site`, edges taken in either direction.
pub fn within_hops(services: &[Service], site: ServiceId, hops: u32) -> Vec<bool> {
    let n = services.len();
    let mut dist = vec![u32::MAX; n];
    if site.index() >= n {
        return vec![false; n];
    }
    dist[site.index()] = 0;
    for h in 0..hops {
        for (i, svc) in services.iter().enumerate() {
            for d in &svc.depends_on {
                let d = d.index();
                if d >= n {
                    continue;
                }
                if dist[i] == h && dist[d] == u32::MAX {
                    dist[d] = h + 1;
                }
                if dist[d] == h && dist[i] == u32::MAX {
                    dist[i] = h + 1;
                }
            }
        }
    }
    dist.into_iter().map(|d| d != u32::MAX).collect()
}

/// Every held observation at a member service from the lookback before the anchor, in id order,
/// capped as the rung caps.
fn from_services(view: &PublicView<'_>, member: &[bool], max_refs: u32) -> Vec<ObsRef> {
    let from = view.anchor_at.0.saturating_sub(view.lookback_ns);
    let refs: Vec<ObsRef> = view
        .store
        .iter()
        .filter(|held| {
            held.at.0 >= from
                && service_of(&held.obs)
                    .is_some_and(|s| member.get(s.index()).copied().unwrap_or(false))
        })
        .map(|held| ObsRef::Passive(held.id))
        .collect();
    cap_head_tail(refs, max_refs as usize)
}
