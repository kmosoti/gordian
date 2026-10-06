//! The change-triggered noticer (work item B1): the charter's change-triggered baseline, at the
//! level of noticing.
//!
//! [`ChangeTriggered`] notices on the first abnormal observation at a node after a quiet period
//! of `quiet_ns`, anchored there, and on nothing else. It has no score and no threshold: an
//! abnormal observation at a node that has had no abnormal observation for `quiet_ns` (or has
//! never had one) is a notice, at the step that delivers it, with that observation as its anchor
//! and the node as its site. It is a reading of that description, stated here:
//!
//! - *A node's quiet period* is measured from its last abnormal observation of any kind,
//!   whichever anomaly (or none) the observation was attached to. A quiet period of exactly
//!   `quiet_ns` is quiet.
//! - *A node that has never had an abnormal observation* is quiet: the noticer has no history
//!   before the stream begins.
//! - *Every onset is its own anomaly*, even when a dependent's onset falls inside another
//!   anomaly's burst window: noticing is per node, so a cascade makes one notice per node that
//!   was quiet. Which anomaly an observation that is not an onset joins is the rung's own rule
//!   ([`super::noticer::attach_target`]): the speaking anomaly at its site, else propagation from
//!   the burst that began at the site's upstream, else the site's stale one. One that joins none is
//!   not an anomaly and is dropped.
//! - *The score* a rule may read is the rung's (the z-score of the anomaly's abnormal count), so
//!   a rule that reads it works under any noticer; it has no part in noticing here.
//! - *Retirement* is the rung's: a noticed anomaly with no abnormal observation for `quiet_ns` of
//!   the rung's own configuration (`quiet_ns` of [`super::rung::RungConfig`]) is ready to retire.
//!   The two quiet times are different parameters.
//!
//! Public information only: the observations as delivered, the public rules' verdict on each and
//! the public graph.

use super::noticer::{CHANGE_ID, Notice, Noticer, Scorer, Tracked, attach_target, quiet_ids};
use super::rung::{Held, RungConfig, Store, service_of};
use gordian_core::Instant;
use gordian_world::{Service, ServiceId};
use std::collections::BTreeMap;

/// The change-triggered noticer. See the module documentation.
#[derive(Debug, Clone)]
pub struct ChangeTriggered {
    cfg: RungConfig,
    services: Vec<Service>,
    quiet_ns: u64,
    scorer: Scorer,
    /// The instant of the latest abnormal observation about each node.
    last_abnormal: BTreeMap<ServiceId, Instant>,
    anomalies: Vec<Tracked>,
    next_id: u32,
}

impl ChangeTriggered {
    /// A noticer for the public graph `services` that treats `quiet_ns` without an abnormal
    /// observation at a node as quiet. The rung's other parameters (`burst_ns`, `burst_gap_ns`,
    /// the retirement quiet time, the score's) are `cfg`'s.
    pub fn new(cfg: RungConfig, services: &[Service], quiet_ns: u64) -> Self {
        let scorer = Scorer::new(&cfg, services.len());
        Self {
            cfg,
            services: services.to_vec(),
            quiet_ns,
            scorer,
            last_abnormal: BTreeMap::new(),
            anomalies: Vec::new(),
            next_id: 0,
        }
    }
}

impl Noticer for ChangeTriggered {
    fn id(&self) -> &'static str {
        CHANGE_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        if !held.abnormal {
            return None;
        }
        self.scorer.saw_abnormal();
        let service = service_of(&held.obs)?;
        let onset = self
            .last_abnormal
            .get(&service)
            .is_none_or(|last| held.at.0 >= last.0.saturating_add(self.quiet_ns));
        self.last_abnormal.insert(service, held.at);
        if onset {
            let id = self.next_id;
            self.next_id += 1;
            self.anomalies.push(Tracked::new(
                id,
                held,
                service,
                &self.services,
                self.cfg.burst_gap_ns,
            ));
            return Some(id);
        }
        let i = attach_target(
            &self.anomalies,
            service,
            held.at,
            self.cfg.burst_ns,
            self.cfg.burst_gap_ns,
        )?;
        self.anomalies[i].note_attached(held, service, self.cfg.burst_gap_ns);
        Some(self.anomalies[i].id)
    }

    fn notice(&mut self, now: Instant, _store: &Store) -> Vec<Notice> {
        let mut out = Vec::new();
        for a in &mut self.anomalies {
            if a.noticed_at.is_none() {
                a.noticed_at = Some(now);
                out.push(Notice {
                    id: a.id,
                    anchor: a.anchor,
                    anchor_at: a.anchor_at,
                    site: a.site,
                    attached: a.attached.iter().map(|(_, o, _)| *o).collect(),
                });
            }
        }
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
