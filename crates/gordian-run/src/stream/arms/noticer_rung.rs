//! The rung's own noticer, and the noticer that moves its anchor earlier (work item B1).
//!
//! [`RungNoticer`] is the noticing the shared cheap rung did before the seam existed, moved here
//! unchanged: [`super::noticer::attach_target`] groups abnormal observations into candidate
//! anomalies by the public graph (same service; or a dependent of the anomaly's site within a
//! short burst window, which is what propagation looks like), each is scored by
//! [`super::noticer::Scorer`] (a z-score of its recent abnormal count against a baseline the
//! noticer learns from its own history of the stream), and a candidate whose score crosses
//! `notice_z` is noticed and re-anchored on the densest burst among its attached observations.
//! Candidates that never cross the threshold are forgotten once they have been silent for a score
//! window. Public statistics only; its parameters are [`super::rung::RungConfig`]'s.
//!
//! [`EarliestAnchor`] is the same noticer with one change at the moment of notice: the anchor is
//! moved to the earliest abnormal observation at the anomaly's site within `lookback_ns` before
//! the rung's anchor, the cheapest public fix for an anchor that landed a moment after the
//! incident's first abnormal observation (or on a stray before it, when the stray is the earliest
//! thing at the site; the noticer cannot tell). It is a reading of that description, stated here:
//!
//! - *Earliest*: the held observation with the smallest id among those that are abnormal, about the
//!   site, earlier than the rung's anchor, and whose instant is at most `lookback_ns` before the
//!   anchor's. None: the rung's anchor stands.
//! - *The evidence moves with the anchor.* Every such observation (the earliest and the abnormal
//!   observations at the site between it and the rung's anchor) is put in front of the anomaly's
//!   attached evidence, so that the anchor is still the first attached observation, as the shared
//!   rule and the context builders assume. The observations are copied: another tracked anomaly
//!   that holds them keeps them, so a stray can belong to two anomalies' evidence.
//! - *Nothing else moves.* The site, the region and the burst timing the attach rules use are the
//!   rung's, so what attaches afterwards is what would have attached to the rung's own anomaly.

use super::noticer::{
    EARLIEST_ID, Notice, Noticer, RUNG_ID, Scorer, Tracked, attach_target, quiet_ids,
};
use super::rung::{Held, RungConfig, Store, service_of};
use gordian_core::Instant;
use gordian_world::Service;
use std::collections::BTreeSet;

/// The rung's own noticer. See the module documentation.
#[derive(Debug, Clone)]
pub struct RungNoticer {
    cfg: RungConfig,
    services: Vec<Service>,
    scorer: Scorer,
    anomalies: Vec<Tracked>,
    next_id: u32,
}

impl RungNoticer {
    /// A noticer for the public graph `services` under `cfg`.
    pub fn new(cfg: RungConfig, services: &[Service]) -> Self {
        let scorer = Scorer::new(&cfg, services.len());
        Self {
            cfg,
            services: services.to_vec(),
            scorer,
            anomalies: Vec::new(),
            next_id: 0,
        }
    }

    /// Notice the candidates whose score has crossed the threshold, and forget the ones that
    /// never will. Returns the index of each anomaly noticed, in order.
    pub(super) fn cross(&mut self, now: Instant) -> Vec<usize> {
        let mut crossed = Vec::new();
        for (i, a) in self.anomalies.iter().enumerate() {
            if a.noticed_at.is_none() && self.scorer.score(a, now) >= self.cfg.notice_z {
                crossed.push(i);
            }
        }
        for &i in &crossed {
            self.anomalies[i].reanchor(self.cfg.burst_ns, &self.services);
            self.anomalies[i].noticed_at = Some(now);
        }
        crossed
    }

    /// Forget the candidates that were never noticed and have been silent for a score window.
    pub(super) fn forget_stale(&mut self, now: Instant) {
        let ttl = self.cfg.score_window_ns;
        self.anomalies.retain(|a| {
            a.noticed_at.is_some() || now.0 <= a.last_abnormal_at.0.saturating_add(ttl)
        });
    }

    /// The tracked anomalies, mutably, for a noticer built on this one (work item B2).
    pub(super) fn anomalies_mut(&mut self) -> &mut [Tracked] {
        &mut self.anomalies
    }

    pub(super) fn notice_of(a: &Tracked) -> Notice {
        Notice {
            id: a.id,
            anchor: a.anchor,
            anchor_at: a.anchor_at,
            site: a.site,
            attached: a.attached.iter().map(|(_, o, _)| *o).collect(),
        }
    }
}

impl Noticer for RungNoticer {
    fn id(&self) -> &'static str {
        RUNG_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        if !held.abnormal {
            return None;
        }
        self.scorer.saw_abnormal();
        let service = service_of(&held.obs)?;
        let found = attach_target(
            &self.anomalies,
            service,
            held.at,
            self.cfg.burst_ns,
            self.cfg.burst_gap_ns,
        );
        match found {
            Some(i) => {
                self.anomalies[i].note_attached(held, service, self.cfg.burst_gap_ns);
                Some(self.anomalies[i].id)
            }
            None => {
                let id = self.next_id;
                self.next_id += 1;
                self.anomalies.push(Tracked::new(
                    id,
                    held,
                    service,
                    &self.services,
                    self.cfg.burst_gap_ns,
                ));
                Some(id)
            }
        }
    }

    fn notice(&mut self, now: Instant, _store: &Store) -> Vec<Notice> {
        let crossed = self.cross(now);
        let out = crossed
            .iter()
            .map(|&i| Self::notice_of(&self.anomalies[i]))
            .collect();
        self.forget_stale(now);
        out
    }

    fn anomalies(&self) -> &[Tracked] {
        &self.anomalies
    }

    fn score(&self, id: u32, now: Instant) -> f64 {
        self.tracked(id)
            .map_or(f64::NEG_INFINITY, |a| self.scorer.score(a, now))
    }

    fn refresh(&mut self, now: Instant) {
        for a in &mut self.anomalies {
            if a.noticed_at.is_some() {
                a.peak_score = a.peak_score.max(self.scorer.score(a, now));
            }
        }
    }

    fn retirable(&self, now: Instant) -> Vec<u32> {
        quiet_ids(&self.anomalies, now, self.cfg.quiet_ns)
    }

    fn retire(&mut self, id: u32) {
        self.anomalies.retain(|a| a.id != id);
    }
}

/// The rung's noticer with its anchor moved to the earliest abnormal observation at the anomaly's
/// site within `lookback_ns` before the rung's anchor. See the module documentation.
#[derive(Debug, Clone)]
pub struct EarliestAnchor {
    inner: RungNoticer,
    lookback_ns: u64,
}

impl EarliestAnchor {
    /// `inner`, with the anchor of each notice moved up to `lookback_ns` earlier.
    pub fn new(inner: RungNoticer, lookback_ns: u64) -> Self {
        Self { inner, lookback_ns }
    }

    /// Move the anchor of anomaly `id`, just noticed, if there is anything earlier at its site.
    fn move_earlier(&mut self, id: u32, store: &Store) {
        let Some(a) = self.inner.anomalies.iter_mut().find(|a| a.id == id) else {
            return;
        };
        let attached: BTreeSet<_> = a.attached.iter().map(|(_, o, _)| *o).collect();
        let earlier: Vec<Held> = store
            .iter()
            .filter(|h| {
                h.abnormal
                    && h.id < a.anchor
                    && !attached.contains(&h.id)
                    && h.at.0.saturating_add(self.lookback_ns) >= a.anchor_at.0
                    && service_of(&h.obs) == Some(a.site)
            })
            .cloned()
            .collect();
        a.prepend(&earlier);
    }
}

impl Noticer for EarliestAnchor {
    fn id(&self) -> &'static str {
        EARLIEST_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        self.inner.observe(held)
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        let crossed = self.inner.cross(now);
        let ids: Vec<u32> = crossed
            .iter()
            .map(|&i| self.inner.anomalies[i].id)
            .collect();
        for id in &ids {
            self.move_earlier(*id, store);
        }
        let out = ids
            .iter()
            .filter_map(|id| self.inner.tracked(*id).map(RungNoticer::notice_of))
            .collect();
        self.inner.forget_stale(now);
        out
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
