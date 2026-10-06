//! The splitting noticer (work item B3): an anomaly that holds two bursts at different sites,
//! separated in time, is two anomalies.
//!
//! # The failure it answers
//!
//! B2's re-anchor left 18 of 372 hard incidents not anchor-correct; 8 were never noticed, and for
//! five of those no background notice lies within 5 s before them, so mis-anchoring onto a stray is
//! not what holds them. The hypothesis this noticer tests (it is a hypothesis, not a finding) is
//! that another incident's anomaly absorbed their observations: the attach rule is public and
//! graph-based, it gives an observation to the anomaly that is speaking at its site, to the burst a
//! dependent follows, and last to the site's stale anomaly, and an observation that belongs to a
//! different incident can be given to an anomaly that is already noticed, where it makes no notice
//! of its own. Splitting puts a later burst at a different site back into its own anomaly. Whether
//! that recovers anything is what the evaluator's measures answer.
//!
//! # The rule, and the readings of it stated before any tuning
//!
//! At each step, before the base noticer scores its candidates, every anomaly the base tracks
//! (noticed or not) is examined by [`split_picks`], a pure function. Every word of "when an
//! anomaly's attached observations form two bursts at different sites separated by more than a
//! gap, the later burst becomes its own anomaly anchored at its first observation" is a reading,
//! and the readings were chosen after looking at how the public attach rule fills an anomaly (the
//! tuning streams' unanchored incidents, in the diagnostic `tests/stream_b3_probe.rs` of the report):
//!
//! - **Attached observations.** The abnormal observations the base attached to the anomaly, in
//!   delivery order, the anchor first, as they stand at the step. No benign observation, no store.
//! - **A cluster** is a maximal run of attached observations in which each is at most
//!   [`super::rung::RungConfig::burst_ns`] (the rung's burst window, 0.4 s) after the one before. A
//!   **burst** is a cluster of at least `min_burst` attached observations. A cluster a heartbeat
//!   of the anomaly's site opens is one cluster with the observations that follow it within the
//!   window: the observation a burst begins with is not what makes it a burst at another site.
//! - **At different sites.** The anomaly's site is the service of its anchor. The *foreign*
//!   observations of a cluster are those about any other service. A later burst is a cluster, not
//!   the first, with at least `min_burst` foreign observations. This is the reading of "at
//!   different sites" as "a burst of observations about services other than the anomaly's site":
//!   the observation that opens the cluster may be the site's own (the public attach rule opens a
//!   propagation window with any observation at the site, a lone heartbeat included, and attaches
//!   a dependent's observations to it).
//! - **Separated by more than a gap.** The silence between the last observation of the latest
//!   earlier *burst* (a cluster of at least `min_burst` attached observations, of any services,
//!   that ends before the later one begins) and the first foreign observation of the later burst
//!   is strictly more than `gap_ns`. The observations the anomaly attached in between that do not
//!   make a burst (a site's heartbeats, a stray) do not shorten the gap. No earlier burst, no
//!   split: the first cluster is the anomaly's own, and what propagates within the window of an
//!   anomaly's own burst is read as its own.
//! - **The later burst is complete.** A cluster is complete when `burst_ns` has passed since its
//!   last observation, at the instant of the step. A cluster still growing is examined at a later
//!   step, so the split takes every foreign observation of the burst and not those delivered so far.
//! - **What moves.** The foreign observations of that one cluster. The anchor of the new anomaly is
//!   the first of them, its site that observation's service, and the evidence digest, the region and
//!   the burst timing of both anomalies are rebuilt from what each now holds
//!   ([`super::noticer::Tracked::split_off`]). Foreign observations of later clusters stay, and are
//!   examined (after the cluster they follow has been taken) as the same rule says, in the same
//!   step. The new anomaly takes a fresh id from the base's set. A cluster that holds the
//!   observations of two incidents at different dependents is one burst and becomes one anomaly,
//!   anchored on the first of them. The observations leave the old anomaly's evidence in the
//!   noticer; the rung's working state of an anomaly that was already noticed keeps what it
//!   admitted, which is a limit of the seam (a noticer cannot withdraw evidence from the rung).
//! - **Noticed or not.** Never at the step it is made. The new anomaly is a candidate in the base's
//!   set, which the base scores and notices when its score crosses the threshold, or forgets, as it
//!   does any candidate: splitting changes what is grouped with what, and what is noticed stays the
//!   base's rule. (Noticing the new anomaly at once, on the strength of the old one's notice, was
//!   considered and rejected on a diagnostic of the tuning streams: two stray observations at
//!   dependents of a speaking anomaly make a burst of two, and an anomaly noticed on them is a false
//!   notice that then takes the real incident's burst when it arrives at that site, anchored on the
//!   stray.)
//! - **When.** At each step before the base notices, so that a candidate holding two bursts is two
//!   candidates when its score is read, and the base's re-anchor on the densest burst cannot drop
//!   an earlier burst out of an anomaly that it then anchors on a later one.
//! - **What is not covered.** Two incidents at the same site; a stray at an upstream service that
//!   a dependent's first observations attach to (one cluster, no earlier burst); two bursts closer
//!   than `gap_ns`; a burst of fewer than `min_burst` foreign observations; two incidents' bursts
//!   within one cluster.
//!
//! Public information only: the observations as delivered, the public rules' verdict on each, the
//! public graph. It reads no label, no incident, no tier and nothing about hidden structure.

use super::noticer::{Notice, Noticer, Tracked};
use super::noticer_rung::{RungBased, RungNoticer};
use super::rung::{Held, Store};
use gordian_core::Instant;
use gordian_world::Service;
use serde::{Deserialize, Serialize};

/// The parameters of the splitting noticer. Every one is written to a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitSpec {
    /// A later burst must begin more than this after the attached observation before it,
    /// nanoseconds.
    pub gap_ns: u64,
    /// The fewest observations, about services other than the anomaly's site and the first
    /// included, in the rung's burst window that make a burst. At least 2.
    pub min_burst: u32,
}

impl SplitSpec {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.gap_ns == 0 {
            return Err("noticer split: gap_ns must be at least 1".to_owned());
        }
        if self.min_burst < 2 {
            return Err("noticer split: min_burst must be at least 2".to_owned());
        }
        Ok(())
    }
}

/// The attached observations (indices into `anomaly.attached`) that a split of `anomaly` moves out
/// of it at the step at `now`, if the rule applies: see the module documentation. `None` when it
/// does not.
pub fn split_picks(
    anomaly: &Tracked,
    gap_ns: u64,
    min_burst: u32,
    burst_ns: u64,
    now: Instant,
) -> Option<Vec<usize>> {
    let at = &anomaly.attached;
    let site = anomaly.site;
    let min_burst = min_burst as usize;
    // Clusters, as ranges of indices.
    let mut clusters: Vec<(usize, usize)> = Vec::new();
    for i in 0..at.len() {
        match clusters.last_mut() {
            Some((_, end)) if at[i].0.0 <= at[i - 1].0.0.saturating_add(burst_ns) => *end = i + 1,
            _ => clusters.push((i, i + 1)),
        }
    }
    for (m, &(from, to)) in clusters.iter().enumerate().skip(1) {
        let foreign: Vec<usize> = (from..to).filter(|&j| at[j].2 != site).collect();
        let Some(&first_foreign) = foreign.first() else {
            continue;
        };
        if foreign.len() < min_burst || now.0 <= at[to - 1].0.0.saturating_add(burst_ns) {
            continue;
        }
        let Some(&(_, earlier_end)) = clusters[..m].iter().rev().find(|(f, t)| t - f >= min_burst)
        else {
            continue;
        };
        let silence = at[first_foreign]
            .0
            .0
            .saturating_sub(at[earlier_end - 1].0.0);
        if silence > gap_ns {
            return Some(foreign);
        }
    }
    None
}

/// A base noticer with the splitting noticer composed over it. See the module documentation.
#[derive(Debug, Clone)]
pub struct SplitNoticer<B> {
    inner: B,
    spec: SplitSpec,
    services: Vec<Service>,
    burst_ns: u64,
    burst_gap_ns: u64,
    splits: u32,
    id: &'static str,
}

impl<B: RungBased> SplitNoticer<B> {
    /// `inner` with anomalies split as `spec` says; `id` is the id the run output writes.
    pub fn new(inner: B, spec: SplitSpec, id: &'static str) -> Self {
        let services = inner.rung().services().to_vec();
        let burst_ns = inner.rung().config().burst_ns;
        let burst_gap_ns = inner.rung().config().burst_gap_ns;
        Self {
            inner,
            spec,
            services,
            burst_ns,
            burst_gap_ns,
            splits: 0,
            id,
        }
    }

    /// How many splits have been made.
    pub fn splits(&self) -> u32 {
        self.splits
    }

    /// Examine every anomaly, splitting as the rule says. A new anomaly joins the base's set as a
    /// candidate.
    fn split_pass(&mut self, now: Instant) {
        let (spec, burst_ns, burst_gap_ns) = (self.spec, self.burst_ns, self.burst_gap_ns);
        let mut i = 0;
        loop {
            let rung = self.inner.rung_mut();
            let picks = {
                let Some(a) = rung.anomalies_vec_mut().get(i) else {
                    break;
                };
                split_picks(a, spec.gap_ns, spec.min_burst, burst_ns, now)
            };
            let Some(picks) = picks else {
                i += 1;
                continue;
            };
            let new_id = rung.take_id();
            let list = rung.anomalies_vec_mut();
            match list[i].split_off(&picks, new_id, &self.services, burst_gap_ns) {
                Some(b) => {
                    list.push(b);
                    self.splits += 1;
                    // Examine the same anomaly again: it may hold another later burst.
                }
                None => i += 1,
            }
        }
    }
}

impl<B: RungBased> Noticer for SplitNoticer<B> {
    fn id(&self) -> &'static str {
        self.id
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        self.inner.observe(held)
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        self.split_pass(now);
        self.inner.notice(now, store)
    }

    fn anomalies(&self) -> &[Tracked] {
        self.inner.anomalies()
    }

    fn score(&self, id: u32, now: Instant) -> f64 {
        self.inner.score(id, now)
    }

    fn refresh(&mut self, now: Instant) {
        self.inner.refresh(now);
    }

    fn retirable(&self, now: Instant) -> Vec<u32> {
        self.inner.retirable(now)
    }

    fn retire(&mut self, id: u32) {
        self.inner.retire(id);
    }
}

impl<B: RungBased> RungBased for SplitNoticer<B> {
    fn rung(&self) -> &RungNoticer {
        self.inner.rung()
    }

    fn rung_mut(&mut self) -> &mut RungNoticer {
        self.inner.rung_mut()
    }
}
