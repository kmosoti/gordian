//! The later re-anchor noticer (work item B2): the rung's noticer, with a re-anchor onto a burst
//! that begins after an isolated anchor.
//!
//! # The failure it answers
//!
//! B1 measured that most of the rung's never-noticed incidents are mis-anchored, not missed: a
//! background stray (an abnormal observation at, or upstream of, the incident's site, a fraction of
//! a second before the incident's first observation) opens a candidate anomaly, the incident's burst
//! attaches to it by the public graph, the candidate crosses the threshold on the burst's
//! observations, and the rung's own re-anchor (the observation with the most attached observations
//! in the `burst_ns` after it) keeps the stray, because the stray's window already holds the whole
//! burst. The notice is then anchored on background, so by the evaluator's rule N1 the incident is
//! not noticed. The cheapest public fix B1 tried, an *earlier* anchor, makes it worse. This noticer
//! makes the fix B1 pointed at: a *later* anchor, onto the burst.
//!
//! # The rule, and the readings of it stated before any tuning
//!
//! The rung's noticer runs unchanged (candidates, the graph-based attach rule, the score, the
//! threshold, the rung's own re-anchor, forgetting). At the step that notices a candidate, after
//! the rung's own re-anchor, this noticer applies one more rule to the anomaly's attached
//! observations: *if the anchor is isolated and a burst begins after it, the anchor moves to that
//! burst's first observation.* [`later_burst_start`] is the rule as a pure function. Every word of it
//! is a reading, and each is a parameter or is fixed here:
//!
//! - **Attached observations.** Only the abnormal observations attached to the anomaly (the rung's
//!   `attached`, delivery order, the anchor first), at the moment of the notice. No benign
//!   observation, no store, no other anomaly.
//! - **Isolated** (gap `g`, `gap_ns`). The anchor is isolated when no other attached observation
//!   falls in the `g` after it: strictly before the anchor's instant plus `g`, so an observation
//!   exactly `g` after leaves the anchor isolated and an observation at the anchor's own instant
//!   does not. Which observations count is [`Isolation`]: `site` reads the brief literally ("no
//!   other abnormal observation at its site": only attached observations about the anchor's own
//!   service count; a burst at a dependent does not make the anchor non-isolated), `any` counts
//!   every attached observation. The two differ for a stray at an upstream service followed by an
//!   incident's burst at a dependent, and for a real incident whose one observation at its site is
//!   followed at once by its dependents' (`site` moves that anchor, `any` does not whenever the
//!   dependents follow within `g`).
//! - **Burst** (`min_burst` observations, window `burst_ns`). An attached observation begins a burst
//!   when it is *strictly later* than the anchor ("begins after the anchor") and at least
//!   `min_burst` attached observations, itself included, fall in the `burst_ns` from it (the rung's
//!   own window for "a burst", [`super::rung::RungConfig::burst_ns`], 0.4 s). The first such
//!   observation, in delivery order, is where the anchor moves. There is no other bound on how far
//!   after the anchor the burst may begin: the attach rule already bounds it (a dependent's within
//!   `burst_ns`, a site's own within two burst gaps).
//! - **The move.** As the rung's own re-anchor moves an anchor: the new anchor is the burst's first
//!   observation, its service is the anomaly's site, the region and the burst timing follow from it,
//!   and the observations before it leave the anomaly's evidence ([`Tracked::move_anchor_to`]).
//! - **When.** At the step that notices the candidate, and not afterwards. A candidate crosses the
//!   threshold only on at least four or five attached observations in the score window (a lone
//!   observation cannot), so an anchor that is a lone observation is noticed together with the burst
//!   that attached to it, and the move happens before the notice is recorded. An anomaly already
//!   noticed is not re-anchored: the notice log has no re-anchor entry, a notice is one record
//!   per anomaly, and a second notice would be a different noticer. A burst that begins only after
//!   the notice (the incident of a quiet anchor that was noticed on other observations) is not
//!   served, and this is a limit of the design, not of the measure.
//! - **The threshold.** `notice_z`, as the rung's: given, it replaces the rung's for this noticer,
//!   so the background budget can be spent on a lower threshold.
//!
//! Public information only: the observations as delivered, the public rules' verdict on each, the
//! public graph. The noticer reads no label, no tier, no incident, and nothing about hidden
//! structure; its parameters were tuned on the tuning streams against the evaluator's measures, as
//! every parameter of the baselines is, and the tuning is in the B2 report.

use super::noticer::{Notice, Noticer, REANCHOR_ID, Tracked};
use super::noticer_rung::RungNoticer;
use super::rung::{Held, RungConfig, Store};
use gordian_core::Instant;
use gordian_world::Service;
use serde::{Deserialize, Serialize};

/// Which attached observations make the anchor non-isolated. See the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    /// Only attached observations about the anchor's own service.
    Site,
    /// Every attached observation.
    Any,
}

impl Isolation {
    /// The word written to a manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Any => "any",
        }
    }
}

/// The index of the attached observation the anchor of `anomaly` moves to, if the later re-anchor
/// rule applies: the anchor is isolated (no other attached observation, of the service `isolation`
/// says, strictly before the anchor's instant plus `gap_ns`) and some later attached observation
/// begins a burst (strictly later than the anchor, with at least `min_burst` attached observations
/// including itself in the `burst_ns` from it). `None` when the anchor stays.
pub fn later_burst_start(
    anomaly: &Tracked,
    gap_ns: u64,
    min_burst: u32,
    burst_ns: u64,
    isolation: Isolation,
) -> Option<usize> {
    let attached = &anomaly.attached;
    let (t0, _, s0) = *attached.first()?;
    let near = attached[1..].iter().any(|(t, _, s)| {
        t.0 < t0.0.saturating_add(gap_ns) && (isolation == Isolation::Any || *s == s0)
    });
    if near {
        return None;
    }
    (1..attached.len()).find(|&k| {
        let tk = attached[k].0;
        tk > t0
            && attached[k..]
                .iter()
                .take_while(|(t, _, _)| t.0 <= tk.0.saturating_add(burst_ns))
                .count()
                >= min_burst as usize
    })
}

/// The rung's noticer with the later re-anchor. See the module documentation.
#[derive(Debug, Clone)]
pub struct ReanchorNoticer {
    inner: RungNoticer,
    services: Vec<Service>,
    gap_ns: u64,
    min_burst: u32,
    burst_ns: u64,
    isolation: Isolation,
}

impl ReanchorNoticer {
    /// The rung's noticer under `cfg` for the public graph `services`, re-anchoring with isolation
    /// gap `gap_ns`, bursts of at least `min_burst` observations, and `isolation`. The burst
    /// window is `cfg.burst_ns`.
    pub fn new(
        cfg: RungConfig,
        services: &[Service],
        gap_ns: u64,
        min_burst: u32,
        isolation: Isolation,
    ) -> Self {
        let burst_ns = cfg.burst_ns;
        Self {
            inner: RungNoticer::new(cfg, services),
            services: services.to_vec(),
            gap_ns,
            min_burst,
            burst_ns,
            isolation,
        }
    }
}

impl Noticer for ReanchorNoticer {
    fn id(&self) -> &'static str {
        REANCHOR_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        self.inner.observe(held)
    }

    fn notice(&mut self, now: Instant, _store: &Store) -> Vec<Notice> {
        let crossed = self.inner.cross(now);
        for &i in &crossed {
            let a = &mut self.inner.anomalies_mut()[i];
            if let Some(k) = later_burst_start(
                a,
                self.gap_ns,
                self.min_burst,
                self.burst_ns,
                self.isolation,
            ) {
                a.move_anchor_to(k, &self.services);
            }
        }
        let out = crossed
            .iter()
            .map(|&i| RungNoticer::notice_of(&self.inner.anomalies()[i]))
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
