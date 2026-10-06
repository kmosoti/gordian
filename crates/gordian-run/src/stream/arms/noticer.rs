//! The noticing seam (work item B1): what turns held observations into noticed anomalies.
//!
//! The shared cheap rung ([`super::rung`]) used to bundle four things: noticing (which abnormal
//! observations make an anomaly worth attention, and where it is anchored), attaching (which
//! observations belong to a noticed anomaly), concluding (the first world's components and the
//! shared rule) and declaring. This file separates the first from the rest. A [`Noticer`] is
//! given the public view and yields **noticed anomalies** ([`Notice`]: anchor, site, attached
//! observations) and **retirements**; everything downstream of a notice (the working state, the
//! components, the shared rule, declaring, the context builders, the escalation rules) is
//! [`super::rung::Rung`]'s and does not depend on which noticer produced it.
//!
//! # What a noticer reads
//!
//! Each observation the stream delivers, with the public rules' verdict on it ([`Held`]); the
//! observations the rung holds ([`Store`]), at the moment it is asked to notice; the public graph
//! (given at construction); and the instant. Nothing else: no tier, no label, no incident, no
//! decisive evidence, no reasoner answer. This file is held to the textual ban of
//! `scripts/check-no-oracle.sh` like every file under `arms/`.
//!
//! # What a noticer owns
//!
//! The anomalies it tracks ([`Tracked`]): their anchor, site, attached observations and the
//! evidence digest's content, from the first abnormal observation attached to them. A tracked
//! anomaly that has been noticed ([`Tracked::noticed_at`]) is one the rung then works on. The
//! rung owns the rest (reviews, probes, escalations, declarations) and tells the noticer when an
//! anomaly is finished with ([`Noticer::retire`]).
//!
//! # The noticers built
//!
//! | Id | What it does |
//! |---|---|
//! | `rung` ([`super::noticer_rung::RungNoticer`]) | the rung's own noticing, unchanged: candidates grouped by the public graph, each scored by a z-score of its recent abnormal count against a baseline learned from the stream so far, noticed when the score crosses `notice_z`, then re-anchored on the densest burst |
//! | `change_triggered` ([`super::noticer_change::ChangeTriggered`]) | a notice on the first abnormal observation at a node after `quiet_ns` without one there, anchored on it |
//! | `earliest_anchor` ([`super::noticer_rung::EarliestAnchor`]) | the rung's noticer with its anchor moved to the earliest abnormal observation at the anomaly's site within `lookback_ns` before the rung's anchor |
//! | `reanchor` ([`super::noticer_reanchor::ReanchorNoticer`], work item B2) | the rung's noticer with a later re-anchor: an anomaly anchored on an isolated abnormal observation (none other at its site within `gap_ns`) whose attached evidence holds a burst that begins after the anchor is re-anchored on that burst's first observation, at the moment of notice |
//! | `ramp`, `split`, `ramp_split` and each with `_reanchor` ([`NoticerSpec::Composed`], work item B3) | a base noticer (the rung's, or the later re-anchor) with [`super::noticer_ramp::RampNoticer`] (a per-node trend detector on counter readings, benign ones included, which opens a noticed anomaly of its own on a smooth rise, anchored on the rise's first reading), [`super::noticer_split::SplitNoticer`] (an anomaly holding a later burst at other sites, after a silence, is two anomalies), or both, wrapped around it. Both open and move anomalies in the base's own set, so everything downstream is shared |
//!
//! # The record
//!
//! Every notice and every retirement is recorded as a [`NoticeLogEntry`] (anchor, site, the
//! instants, the noticer's id) and reaches the run output through the harness. The log holds
//! public information only; it is scored against the hidden record by the evaluator, elsewhere.

use super::noticer_ramp::{RampNoticer, RampSpec};
use super::noticer_rung::RungBased;
use super::noticer_split::{SplitNoticer, SplitSpec};
use super::rung::{Held, RungConfig, Store};
use gordian_core::Instant;
use gordian_stream::{Diagnosis, ObsId};
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{SymptomTag, signature};
use gordian_world::{Observation, Service, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Which noticer an arm uses, and its parameters. Written to a manifest as an object tagged by
/// `noticer`; the default (`rung` with no override) is never written, so a manifest written before
/// noticers existed is the same text as one written now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "noticer", rename_all = "snake_case", deny_unknown_fields)]
pub enum NoticerSpec {
    /// The rung's own noticing. `notice_z` replaces the rung's `notice_z` for this noticer when
    /// given (the sweep of work item R10, spelled per arm).
    Rung {
        /// The z-score at which a candidate anomaly is noticed, if not the rung's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notice_z: Option<f64>,
    },
    /// A notice on the first abnormal observation at a node after a quiet period.
    ChangeTriggered {
        /// How long a node must have had no abnormal observation, nanoseconds.
        quiet_ns: u64,
    },
    /// The rung's noticer with the anchor moved earlier.
    EarliestAnchor {
        /// How far before the rung's anchor an abnormal observation at the site may be, nanoseconds.
        lookback_ns: u64,
    },
    /// The rung's noticer with a later re-anchor (work item B2). Every parameter is written.
    Reanchor {
        /// The z-score at which a candidate anomaly is noticed, if not the rung's (as for `Rung`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notice_z: Option<f64>,
        /// The gap `g`: the anchor is isolated when no other attached observation (of the service
        /// `isolation` says) falls strictly before its instant plus this, nanoseconds.
        gap_ns: u64,
        /// The fewest attached observations, the first included, in the rung's `burst_ns` that make
        /// a burst. At least 2.
        min_burst: u32,
        /// Which attached observations make the anchor non-isolated.
        isolation: super::noticer_reanchor::Isolation,
    },
    /// The medium as a noticer (work item M2, Lab 1): a hand-designed graph of cells on
    /// `gordian-medium`, fed every delivered observation with its value
    /// ([`super::medium`]).
    Medium(super::medium::MediumParams),
    /// A base noticer with a ramp noticer and/or a splitting noticer wrapped around it (work item
    /// B3). At least one of `ramp` and `split` is given; a manifest naming neither is refused.
    Composed {
        /// The noticer underneath: the rung's, or the later re-anchor.
        base: BaseSpec,
        /// The ramp noticer's parameters, if it is part of this noticer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ramp: Option<super::noticer_ramp::RampSpec>,
        /// The splitting noticer's parameters, if it is part of this noticer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        split: Option<super::noticer_split::SplitSpec>,
    },
}

/// The noticer a composed noticer ([`NoticerSpec::Composed`]) is built on: one that works over a
/// [`super::noticer_rung::RungNoticer`]'s set of anomalies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "noticer", rename_all = "snake_case", deny_unknown_fields)]
pub enum BaseSpec {
    /// The rung's own noticing (as [`NoticerSpec::Rung`]).
    Rung {
        /// The z-score at which a candidate anomaly is noticed, if not the rung's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notice_z: Option<f64>,
    },
    /// The later re-anchor (as [`NoticerSpec::Reanchor`]).
    Reanchor {
        /// The z-score at which a candidate anomaly is noticed, if not the rung's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notice_z: Option<f64>,
        /// The isolation gap, nanoseconds.
        gap_ns: u64,
        /// The fewest attached observations that make a burst.
        min_burst: u32,
        /// Which attached observations make the anchor non-isolated.
        isolation: super::noticer_reanchor::Isolation,
    },
}

impl BaseSpec {
    /// Check the parameters, as the noticer of the same name would.
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Self::Rung { notice_z } => NoticerSpec::Rung { notice_z }.validate(),
            Self::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            } => NoticerSpec::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            }
            .validate(),
        }
    }
}

impl Default for NoticerSpec {
    fn default() -> Self {
        Self::Rung { notice_z: None }
    }
}

impl NoticerSpec {
    /// Whether this is the default, which is not written to a manifest.
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Rung { notice_z: None })
    }

    /// The noticer's id, as the run output writes it.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Rung { .. } => RUNG_ID,
            Self::ChangeTriggered { .. } => CHANGE_ID,
            Self::EarliestAnchor { .. } => EARLIEST_ID,
            Self::Reanchor { .. } => REANCHOR_ID,
            Self::Medium(_) => super::medium::MEDIUM_ID,
            Self::Composed { base, ramp, split } => composed_id_with(
                base,
                ramp.is_some(),
                split.is_some(),
                ramp.is_some_and(|r| r.follow.is_some()),
            ),
        }
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Rung {
                notice_z: Some(z), ..
            } if !z.is_finite() => Err("noticer rung: notice_z must be finite".to_owned()),
            Self::Reanchor {
                notice_z: Some(z), ..
            } if !z.is_finite() => Err("noticer reanchor: notice_z must be finite".to_owned()),
            Self::Reanchor { gap_ns: 0, .. } => {
                Err("noticer reanchor: gap_ns must be at least 1".to_owned())
            }
            Self::Reanchor { min_burst, .. } if *min_burst < 2 => {
                Err("noticer reanchor: min_burst must be at least 2".to_owned())
            }
            Self::Medium(params) => params.validate(),
            Self::Composed {
                ramp: None,
                split: None,
                ..
            } => Err("noticer composed: needs a ramp, a split or both".to_owned()),
            Self::Composed { base, ramp, split } => {
                base.validate()?;
                if let Some(r) = ramp {
                    r.validate()?;
                }
                if let Some(sp) = split {
                    sp.validate()?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The id of the rung's own noticer.
pub const RUNG_ID: &str = "rung";
/// The id of the change-triggered noticer.
pub const CHANGE_ID: &str = "change_triggered";
/// The id of the earliest-anchor noticer.
pub const EARLIEST_ID: &str = "earliest_anchor";
/// The id of the later re-anchor noticer (work item B2).
pub const REANCHOR_ID: &str = "reanchor";

/// The id of a composed noticer (work item B3): the pieces it has, then its base when that is the
/// later re-anchor (`ramp`, `split`, `ramp_split`, and each with `_reanchor`). The ids are
/// written to the run output unquoted and contain no comma.
pub fn composed_id(base: &BaseSpec, ramp: bool, split: bool) -> &'static str {
    composed_id_with(base, ramp, split, false)
}

/// As [`composed_id`], with `follow` (work item B4): the ramp noticer has a follow-up rule
/// ([`super::noticer_follow::FollowSpec`]), so the id says so (`ramp_follow`,
/// `ramp_follow_reanchor`, `ramp_split_follow`, `ramp_split_follow_reanchor`). `follow` without
/// `ramp` is not a spelling the manifest has (the rule is part of the ramp's spec) and reads as
/// `follow = false`.
pub fn composed_id_with(base: &BaseSpec, ramp: bool, split: bool, follow: bool) -> &'static str {
    let reanchor = matches!(base, BaseSpec::Reanchor { .. });
    match (ramp, split, reanchor, follow && ramp) {
        (true, false, false, false) => "ramp",
        (true, false, true, false) => "ramp_reanchor",
        (false, true, false, _) => "split",
        (false, true, true, _) => "split_reanchor",
        (true, true, false, false) => "ramp_split",
        (true, true, true, false) => "ramp_split_reanchor",
        (true, false, false, true) => "ramp_follow",
        (true, false, true, true) => "ramp_follow_reanchor",
        (true, true, false, true) => "ramp_split_follow",
        (true, true, true, true) => "ramp_split_follow_reanchor",
        (false, false, false, _) => "composed_rung",
        (false, false, true, _) => "composed_reanchor",
    }
}

/// A noticed anomaly, as a noticer yields it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The anomaly's id (dense from zero in order of creation; unique within a noticer).
    pub id: u32,
    /// Where the anomaly is anchored: the observation a declaration and a question are about.
    pub anchor: ObsId,
    /// When the anchor was emitted.
    pub anchor_at: Instant,
    /// The service the anomaly is about.
    pub site: ServiceId,
    /// The abnormal observations attached to it now, in delivery order, the anchor first.
    pub attached: Vec<ObsId>,
}

/// What the harness writes for a notice or a retirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// The anomaly was noticed.
    Notice,
    /// The anomaly was retired: the rung is finished with it.
    Retire,
}

impl NoticeKind {
    /// The word written to the run output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Notice => "notice",
            Self::Retire => "retire",
        }
    }
}

/// One line of the notice record: public information only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoticeLogEntry {
    /// A notice or a retirement.
    pub kind: NoticeKind,
    /// The noticer that tracked the anomaly.
    pub noticer: &'static str,
    /// The anomaly's id.
    pub anomaly: u32,
    /// Its anchor.
    pub anchor: ObsId,
    /// Its site.
    pub site: ServiceId,
    /// When its anchor was emitted.
    pub anchor_at: Instant,
    /// The instant of the step at which it was noticed or retired.
    pub at: Instant,
    /// Why the anomaly was retired (work item B4); `None` for a notice. The run output's
    /// `notice_events.csv` does not carry it (its columns are B1's and B2's); the selection files
    /// do.
    pub cause: Option<RetireCause>,
}

/// Why a noticer's anomaly was retired (work item B4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetireCause {
    /// The anomaly was quiet: no abnormal observation attached to it for the rung's quiet time.
    /// Every retirement of every noticer before work item B4, and every one of a noticer without a
    /// follow-up rule.
    Quiet,
    /// The follow-up rule ([`super::noticer_follow`]) retired a ramp-noticed anomaly whose later
    /// readings did not keep rising.
    Followup,
}

impl RetireCause {
    /// The word written to the run output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Followup => "followup",
        }
    }
}

/// What a noticer is: the part of the rung that decides what is noticed and where it is anchored.
///
/// One instance per segment. The rung calls, in this order at each step: [`Noticer::observe`]
/// for each delivered passive observation, [`Noticer::notice`] once if the step may do work, and
/// later [`Noticer::retirable`] and [`Noticer::retire`]. A noticer is deterministic: a function of
/// what it has been given.
pub trait Noticer {
    /// The noticer's id.
    fn id(&self) -> &'static str;

    /// Take in one delivered passive observation, with the public rules' verdict on it
    /// ([`Held::abnormal`]). Returns the id of the anomaly it was attached to, if any.
    fn observe(&mut self, held: &Held) -> Option<u32>;

    /// The anomalies noticed at this step, in order, now marked noticed. `store` holds what has
    /// been delivered, including the observations just taken in.
    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice>;

    /// Every anomaly the noticer tracks, noticed or not, in order of creation.
    fn anomalies(&self) -> &[Tracked];

    /// The anomaly's score now: how strongly the evidence about it stands out. Meaningful for
    /// every noticer (the rung's z-score of the abnormal count over the score window), so that a
    /// rule that reads it works with any of them.
    fn score(&self, id: u32, now: Instant) -> f64;

    /// Bring the highest score each noticed anomaly has had up to date with `now`.
    fn refresh(&mut self, now: Instant);

    /// The noticed anomalies that have had no abnormal observation for the quiet time, by id: the
    /// ones the noticer holds ready to retire. The rung may decline (a call or a probe in flight).
    fn retirable(&self, now: Instant) -> Vec<u32>;

    /// The rung is finished with anomaly `id`: forget it.
    fn retire(&mut self, id: u32);

    /// Why anomaly `id`, which the rung is about to retire, is retirable (work item B4): the
    /// noticer's own follow-up rule said so ([`RetireCause::Followup`]) or it is quiet. Asked
    /// before [`Noticer::retire`]. Quiet, the default, for every noticer without a follow-up rule.
    fn retire_cause(&self, _id: u32) -> RetireCause {
        RetireCause::Quiet
    }

    /// The anomaly `id`, if tracked.
    fn tracked(&self, id: u32) -> Option<&Tracked> {
        self.anomalies().iter().find(|a| a.id == id)
    }

    /// The noticer's own counted work since the last call, priced, for the arm to charge to its
    /// bill (work item M2: the medium's operations at its declared prices plus a price per
    /// tick). `None`, the default, for a noticer whose work is bookkeeping (B1's three), which is
    /// what they were before the method existed.
    fn take_cost(&mut self) -> Option<NoticerCost> {
        None
    }

    /// The bill refused the cost [`Noticer::take_cost`] reported: the noticer stops doing
    /// counted work for the rest of the segment. Nothing, by default.
    fn refused(&mut self) {}
}

/// A noticer's own counted work, priced (work item M2), charged by the arm to its bill under
/// `Phase::Component(component)` like a component call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoticerCost {
    /// The id the charge is attributed to.
    pub component: gordian_core::ComponentId,
    /// Modelled compute, nanoseconds.
    pub compute_ns: u64,
}

/// One tracked anomaly: its anchor, site and attached evidence, from public observations.
#[derive(Debug, Clone)]
pub struct Tracked {
    /// Dense from zero in order of creation.
    pub id: u32,
    /// The service it is anchored at.
    pub site: ServiceId,
    /// The dependents of the site, by service index.
    pub region: Vec<bool>,
    /// Its anchor.
    pub anchor: ObsId,
    /// When the anchor was emitted.
    pub anchor_at: Instant,
    /// The abnormal observations attached to it: instant, id, service.
    pub attached: Vec<(Instant, ObsId, ServiceId)>,
    /// For each attached observation, its symptom tags and whether it is a changed snapshot, so
    /// that the evidence can be rebuilt when the anchor moves.
    sigs: Vec<(Vec<SymptomTag>, bool)>,
    /// The symptom tags of what is attached (service forgotten): the evidence digest's content.
    tags: BTreeSet<SymptomTag>,
    snapshot_changed: bool,
    services: BTreeSet<ServiceId>,
    /// When the last abnormal observation was attached to it.
    pub last_abnormal_at: Instant,
    /// When the last abnormal observation at its own site was attached.
    last_site_at: Instant,
    /// When the current burst began at the site: its first abnormal observation after a silence
    /// of the burst gap. Propagation to dependents is measured from here.
    burst_open_at: Instant,
    /// When it was noticed, once it has been.
    pub noticed_at: Option<Instant>,
    /// The highest score it has had since it was noticed.
    pub peak_score: f64,
}

impl Tracked {
    /// A new anomaly with `held` (about `service`) as its first attached observation.
    pub fn new(
        id: u32,
        held: &Held,
        service: ServiceId,
        services: &[Service],
        gap_ns: u64,
    ) -> Self {
        let mut anomaly = Self {
            id,
            site: service,
            region: dependents_mask(services, service),
            anchor: held.id,
            anchor_at: held.at,
            attached: Vec::new(),
            sigs: Vec::new(),
            tags: BTreeSet::new(),
            snapshot_changed: false,
            services: BTreeSet::new(),
            last_abnormal_at: held.at,
            last_site_at: held.at,
            burst_open_at: held.at,
            noticed_at: None,
            peak_score: f64::NEG_INFINITY,
        };
        anomaly.note_attached(held, service, gap_ns);
        anomaly
    }

    /// The instant `at` relative to the anchor, zero before it.
    pub fn rel(&self, at: Instant) -> Instant {
        Instant(at.0.saturating_sub(self.anchor_at.0))
    }

    /// Whether the anomaly's anchor or one of its attached observations is `id`.
    pub fn owns(&self, id: ObsId) -> bool {
        id == self.anchor || self.attached.iter().any(|(_, o, _)| *o == id)
    }

    /// The tags and snapshot flag of one observation.
    fn signature_of(held: &Held) -> (Vec<SymptomTag>, bool) {
        let snapshot = matches!(held.obs, Observation::Snapshot { .. });
        let tags = if snapshot {
            Vec::new()
        } else {
            signature(&[(held.at, held.obs.clone())])
        };
        (tags, snapshot)
    }

    /// Attach `held`, an abnormal observation about `service`.
    pub fn note_attached(&mut self, held: &Held, service: ServiceId, burst_gap_ns: u64) {
        let (tags, snapshot) = Self::signature_of(held);
        self.attached.push((held.at, held.id, service));
        self.last_abnormal_at = held.at;
        if service == self.site {
            if held.at.0 >= self.last_site_at.0.saturating_add(burst_gap_ns)
                || self.attached.len() == 1
            {
                self.burst_open_at = held.at;
            }
            self.last_site_at = held.at;
        }
        self.services.insert(service);
        self.snapshot_changed |= snapshot;
        self.tags.extend(tags.iter().copied());
        self.sigs.push((tags, snapshot));
    }

    /// Recompute what is derived from `attached` and `sigs` after they changed at the front.
    fn rebuild(&mut self) {
        self.tags = self
            .sigs
            .iter()
            .flat_map(|(t, _)| t.iter().copied())
            .collect();
        self.snapshot_changed = self.sigs.iter().any(|(_, snap)| *snap);
        self.services = self.attached.iter().map(|(_, _, s)| *s).collect();
    }

    /// Move the anchor to the start of the densest burst among the attached observations: the
    /// attached observation with the most attached observations in the `cluster_ns` from it (the
    /// earliest on a tie), and make its service the anomaly's site.
    ///
    /// An anomaly's first attached observation may be background (an isolated blip or stray at
    /// the same service, or at a service the site depends on, a moment before the incident
    /// began). The anchor is what a declaration and a question are *about*, and the site is what
    /// the region is built from, so both must belong to the thing noticed and not to whatever
    /// happened to come first. A real incident begins with a burst (several abnormal observations
    /// within a few hundred milliseconds); a stray is alone. Observations before the new anchor
    /// are dropped from the anomaly's evidence. Public statistics only.
    pub fn reanchor(&mut self, cluster_ns: u64, services: &[Service]) {
        let density = |k: usize| {
            let from = self.attached[k].0.0;
            self.attached[k..]
                .iter()
                .take_while(|(t, _, _)| t.0 <= from.saturating_add(cluster_ns))
                .count()
        };
        let mut chosen = 0;
        let mut best = 0;
        for k in 0..self.attached.len() {
            let d = density(k);
            if d > best {
                best = d;
                chosen = k;
            }
        }
        self.move_anchor_to(chosen, services);
    }

    /// Move the anchor to attached observation `k` (an index into `attached`; 0 is the anchor now,
    /// and an index past the end is nothing: neither moves it): the observations before it leave
    /// the anomaly's evidence, its service becomes the site, and the region and the burst timing
    /// follow from it. The one move [`Tracked::reanchor`] and the later re-anchor of
    /// [`super::noticer_reanchor`] make.
    pub fn move_anchor_to(&mut self, k: usize, services: &[Service]) {
        if k == 0 || k >= self.attached.len() {
            return;
        }
        self.attached.drain(..k);
        self.sigs.drain(..k);
        self.rebuild();
        self.anchor = self.attached[0].1;
        self.anchor_at = self.attached[0].0;
        self.site = self.attached[0].2;
        self.region = dependents_mask(services, self.site);
        self.last_site_at = self
            .attached
            .iter()
            .rev()
            .find(|(_, _, s)| *s == self.site)
            .map_or(self.anchor_at, |(t, _, _)| *t);
        self.burst_open_at = self.anchor_at;
    }

    /// Put `earlier` (abnormal observations about the anomaly's site, oldest first, all before the
    /// anchor) in front of the attached evidence and anchor the anomaly on the first of them. The
    /// site, the region and the burst timing are the anomaly's own and do not move.
    pub fn prepend(&mut self, earlier: &[Held]) {
        let Some(first) = earlier.first() else {
            return;
        };
        let front: Vec<(Instant, ObsId, ServiceId)> =
            earlier.iter().map(|h| (h.at, h.id, self.site)).collect();
        let sigs: Vec<(Vec<SymptomTag>, bool)> = earlier.iter().map(Self::signature_of).collect();
        self.attached.splice(0..0, front);
        self.sigs.splice(0..0, sigs);
        self.rebuild();
        self.anchor = first.id;
        self.anchor_at = first.at;
    }

    /// Recompute the instants derived from `attached`: when the last abnormal observation was
    /// attached, when the last one at the site was, and when the current burst at the site began
    /// (the first observation at the site, or one after a silence of `burst_gap_ns`: the rule
    /// [`Tracked::note_attached`] applies one observation at a time).
    fn retime(&mut self, burst_gap_ns: u64) {
        self.last_abnormal_at = self.attached.last().map_or(self.anchor_at, |(t, _, _)| *t);
        let mut last_site: Option<Instant> = None;
        let mut open = self.anchor_at;
        for (t, _, s) in &self.attached {
            if *s == self.site {
                if last_site.is_none_or(|l| t.0 >= l.0.saturating_add(burst_gap_ns)) {
                    open = *t;
                }
                last_site = Some(*t);
            }
        }
        self.last_site_at = last_site.unwrap_or(self.anchor_at);
        self.burst_open_at = open;
    }

    /// Move the attached observations at `picks` (indices into `attached`, strictly increasing,
    /// none of them 0) out of this anomaly into a new anomaly `id` (work item B3). The first moved
    /// observation is the new anomaly's anchor and its service its site; the region, the evidence
    /// digest's content and the burst timing of both anomalies are rebuilt from what each now
    /// holds. The new anomaly is not noticed. An index that is 0, repeated, out of order or past
    /// the end is a caller's error and moves nothing (the result is `None`), as is an empty list.
    pub fn split_off(
        &mut self,
        picks: &[usize],
        id: u32,
        services: &[Service],
        burst_gap_ns: u64,
    ) -> Option<Tracked> {
        let valid = !picks.is_empty()
            && picks.windows(2).all(|w| w[0] < w[1])
            && picks[0] >= 1
            && picks[picks.len() - 1] < self.attached.len();
        if !valid {
            return None;
        }
        let mut moved = Vec::with_capacity(picks.len());
        let mut moved_sigs = Vec::with_capacity(picks.len());
        let mut kept = Vec::with_capacity(self.attached.len() - picks.len());
        let mut kept_sigs = Vec::with_capacity(self.attached.len() - picks.len());
        let mut next = picks.iter().copied().peekable();
        for (i, (a, s)) in self.attached.drain(..).zip(self.sigs.drain(..)).enumerate() {
            if next.peek() == Some(&i) {
                next.next();
                moved.push(a);
                moved_sigs.push(s);
            } else {
                kept.push(a);
                kept_sigs.push(s);
            }
        }
        self.attached = kept;
        self.sigs = kept_sigs;
        self.rebuild();
        self.retime(burst_gap_ns);
        let (anchor_at, anchor, site) = moved[0];
        let mut split = Tracked {
            id,
            site,
            region: dependents_mask(services, site),
            anchor,
            anchor_at,
            attached: moved,
            sigs: moved_sigs,
            tags: BTreeSet::new(),
            snapshot_changed: false,
            services: BTreeSet::new(),
            last_abnormal_at: anchor_at,
            last_site_at: anchor_at,
            burst_open_at: anchor_at,
            noticed_at: None,
            peak_score: f64::NEG_INFINITY,
        };
        split.rebuild();
        split.retime(burst_gap_ns);
        Some(split)
    }

    /// The observation a declaration of `diagnosis` is anchored on: the first attached abnormal
    /// observation at the diagnosed site if there is one, else the anomaly's anchor. The rule
    /// names the site from the evidence, and the evidence at that site is what the declaration
    /// is about.
    pub fn anchor_for(&self, diagnosis: &Diagnosis) -> ObsId {
        diagnosis
            .and_then(|h| {
                self.attached
                    .iter()
                    .find(|(_, _, s)| *s == h.site)
                    .map(|(_, o, _)| *o)
            })
            .unwrap_or(self.anchor)
    }

    /// A digest of the evidence about the anomaly: its symptom tags and the number of services
    /// they are about. FNV-1a over their text, so it is stable. A repeated heartbeat does not
    /// change it; a new kind of symptom, or a new service, does.
    pub fn digest(&self) -> u64 {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3);
            }
        };
        for t in &self.tags {
            feed(format!("{t:?};").as_bytes());
        }
        feed(
            format!(
                "snapshot={};services={}",
                self.snapshot_changed,
                self.services.len()
            )
            .as_bytes(),
        );
        h
    }
}

/// Which tracked anomaly an abnormal observation about `service` at `at` belongs to, if any (an
/// index into `anomalies`).
///
/// An observation about the site of an anomaly that is still speaking (its last observation at
/// its site within two burst gaps) is that anomaly's, even if it falls in another site's burst
/// window: a site's heartbeat is not propagation. Otherwise, propagation: an observation about a
/// dependent of an anomaly's site, within the burst window of the burst that began at that site,
/// belongs to that burst; when several anomalies qualify, the one whose burst began most
/// recently. Last, the same site's stale anomaly. The orders matter: letting a stale isolated
/// observation at the dependent swallow the dependent's share of someone else's burst, measuring
/// the window from an anomaly's first observation, or letting another incident's burst window
/// take a site's own heartbeat, each fragments incidents and lands declarations on background.
pub fn attach_target(
    anomalies: &[Tracked],
    service: ServiceId,
    at: Instant,
    burst_ns: u64,
    gap_ns: u64,
) -> Option<usize> {
    let speaking = gap_ns.saturating_mul(2);
    anomalies
        .iter()
        .rposition(|a| a.site == service && at.0 <= a.last_site_at.0.saturating_add(speaking))
        .or_else(|| {
            anomalies
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.site != service
                        && a.region.get(service.index()).copied().unwrap_or(false)
                        && at.0 >= a.burst_open_at.0
                        && at.0 <= a.burst_open_at.0.saturating_add(burst_ns)
                })
                .max_by_key(|(i, a)| (a.burst_open_at, std::cmp::Reverse(*i)))
                .map(|(i, _)| i)
        })
        .or_else(|| anomalies.iter().rposition(|a| a.site == service))
}

/// The anomalies whose last abnormal observation is `quiet_ns` or more behind `now`, among the
/// noticed ones, by id.
pub fn quiet_ids(anomalies: &[Tracked], now: Instant, quiet_ns: u64) -> Vec<u32> {
    anomalies
        .iter()
        .filter(|a| {
            a.noticed_at.is_some() && now.0 >= a.last_abnormal_at.0.saturating_add(quiet_ns)
        })
        .map(|a| a.id)
        .collect()
}

/// The score every noticer reports: the z-score of an anomaly's abnormal count over the score
/// window against the abnormal rate per service learned from the stream so far, with a prior.
/// Basic IEEE arithmetic only, so it replays bit for bit.
#[derive(Debug, Clone)]
pub struct Scorer {
    services: usize,
    prior_ns: u64,
    prior_mhz: u64,
    window_ns: u64,
    abnormal_seen: u64,
}

impl Scorer {
    /// A scorer for a graph of `services` services under the rung's parameters.
    pub fn new(cfg: &RungConfig, services: usize) -> Self {
        Self {
            services,
            prior_ns: cfg.prior_ns,
            prior_mhz: cfg.prior_mhz,
            window_ns: cfg.score_window_ns,
            abnormal_seen: 0,
        }
    }

    /// One more abnormal observation was delivered.
    pub fn saw_abnormal(&mut self) {
        self.abnormal_seen += 1;
    }

    /// The expected abnormal observations per service per second, learned from the stream so
    /// far with the configured prior.
    fn baseline_hz(&self, now: Instant) -> f64 {
        let services = self.services.max(1) as f64;
        let prior_s = self.prior_ns as f64 / 1e9;
        let prior_hz = self.prior_mhz as f64 / 1000.0;
        let t_s = now.0 as f64 / 1e9;
        (self.abnormal_seen as f64 + prior_hz * services * prior_s) / (services * (t_s + prior_s))
    }

    /// The score of `a` at `now`.
    pub fn score(&self, a: &Tracked, now: Instant) -> f64 {
        let window = self.window_ns;
        let from = now.0.saturating_sub(window);
        let n = a.attached.iter().filter(|(at, _, _)| at.0 > from).count() as f64;
        let mu = self.baseline_hz(now) * (window as f64 / 1e9);
        (n - mu) / (mu + 1.0).sqrt()
    }
}

/// The noticer `spec` names, for a stream with the public graph `services`, under the rung's
/// parameters `cfg`.
pub fn build(spec: &NoticerSpec, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    use super::noticer_change::ChangeTriggered;
    use super::noticer_reanchor::ReanchorNoticer;
    use super::noticer_rung::{EarliestAnchor, RungNoticer};
    match *spec {
        NoticerSpec::Rung { notice_z } => {
            let mut cfg = cfg.clone();
            if let Some(z) = notice_z {
                cfg.notice_z = z;
            }
            Box::new(RungNoticer::new(cfg, services))
        }
        NoticerSpec::ChangeTriggered { quiet_ns } => {
            Box::new(ChangeTriggered::new(cfg.clone(), services, quiet_ns))
        }
        NoticerSpec::EarliestAnchor { lookback_ns } => Box::new(EarliestAnchor::new(
            RungNoticer::new(cfg.clone(), services),
            lookback_ns,
        )),
        NoticerSpec::Reanchor {
            notice_z,
            gap_ns,
            min_burst,
            isolation,
        } => {
            let mut cfg = cfg.clone();
            if let Some(z) = notice_z {
                cfg.notice_z = z;
            }
            Box::new(ReanchorNoticer::new(
                cfg, services, gap_ns, min_burst, isolation,
            ))
        }
        NoticerSpec::Medium(params) => super::medium::build(&params, cfg, services),
        NoticerSpec::Composed { base, ramp, split } => match base {
            BaseSpec::Rung { notice_z } => {
                let mut cfg = cfg.clone();
                if let Some(z) = notice_z {
                    cfg.notice_z = z;
                }
                compose(RungNoticer::new(cfg, services), spec, ramp, split)
            }
            BaseSpec::Reanchor {
                notice_z,
                gap_ns,
                min_burst,
                isolation,
            } => {
                let mut cfg = cfg.clone();
                if let Some(z) = notice_z {
                    cfg.notice_z = z;
                }
                compose(
                    ReanchorNoticer::new(cfg, services, gap_ns, min_burst, isolation),
                    spec,
                    ramp,
                    split,
                )
            }
        },
    }
}

/// `base` with the splitting noticer wrapped around it if `split` is given, then the ramp noticer
/// around that if `ramp` is. The ramp notices first within a step (its anomalies are in the set
/// before the base's score is read), the split runs before the base notices (so a candidate holding
/// two bursts is two candidates when its score is read).
fn compose<B: RungBased + 'static>(
    base: B,
    spec: &NoticerSpec,
    ramp: Option<RampSpec>,
    split: Option<SplitSpec>,
) -> Box<dyn Noticer> {
    let id = spec.id();
    match (ramp, split) {
        (None, None) => Box::new(base),
        (None, Some(sp)) => Box::new(SplitNoticer::new(base, sp, id)),
        (Some(r), None) => Box::new(RampNoticer::new(base, r, id)),
        (Some(r), Some(sp)) => Box::new(RampNoticer::new(SplitNoticer::new(base, sp, id), r, id)),
    }
}
