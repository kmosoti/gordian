//! `public_budgeted`: a call-budgeted public selector, built from a public score per anomaly at the
//! instant the question would be asked (work item B5).
//!
//! B4 showed that the two public selectors ask about 77 to 99 percent of what "always escalate"
//! asks about, so a cost axis measured under them is the shared plain-incident bill. EXP-101's cost
//! axis is therefore a **matched number of reasoner calls per stream**, spent on the anomalies a
//! public score ranks highest. This rule is that selector. Status: built, with the readings below
//! written before any tuning or run; its weights and threshold are tuned per budget `k` on seeds
//! 10000-10099 (`scripts/b5_*.py`), by a rule fixed in `scripts/b5_common.py` before the run.
//!
//! # What it does
//!
//! At every step, the anomalies that are **ready** (live, never asked about, at least `delay_ns`
//! since they were noticed, and not yet looked at by this rule) are scored from the rung's public
//! state at that instant; they are taken in descending score (ties by anomaly id, ascending) and
//! each is asked about once if budget remains and its score reaches the threshold. Every ready
//! anomaly is looked at exactly once, at the first step at which it is ready: an anomaly passed
//! over then is never asked about later, and the budget is a count of questions proposed.
//!
//! # Readings of the brief's words (fixed before any tuning)
//!
//! - **"A public score per live anomaly at the ask instant, from the rung's own state."** The score
//!   is a weighted sum of five features, each scaled to `[0, 1]`, read from [`AnomalyView`] at the
//!   step that makes the anomaly ready, and nothing else:
//!   - `contradiction`: 1 when the public consistency checker's latest verdict on the evidence
//!     attached to the anomaly is "no consistent hypothesis" ([`AnomalyView::contradicted_since`]
//!     is set), else 0. The reading is the instantaneous verdict, not "for `t` seconds": B4 found
//!     that `t` hardly mattered.
//!   - `silence`: 1 when the cheap rung has declared nothing about the anomaly
//!     ([`AnomalyView::cheap_declared`] false), else 0.
//!   - `evidence` ("abnormal count"): `ln(1 + n) / ln(1 + 64)` clipped at 1, `n` being the abnormal
//!     observations attached ([`AnomalyView::evidence`]); a log scale because a burst's count has a
//!     long tail.
//!   - `services` ("services involved"): the distinct services the attached observations are about
//!     ([`AnomalyView::services`], added by this unit), over 8, clipped at 1.
//!   - `age`: seconds from the anomaly's anchor to the step ([`AnomalyView::anchor_at`]), over 120 s
//!     (the rung's retention), clipped at 1. Age is measured from the anchor and not from the
//!     notice, because every ready anomaly is at least `delay_ns` past its notice, so age since
//!     notice is constant at the ask instant and could carry nothing.
//! - **"The noticer's strict-precision proxy where it exists."** It exists for no noticer in this
//!   tree: no noticer attaches a per-anomaly precision figure to its anomalies (strict precision is
//!   an evaluator measure over a noticer's notices, N16 of the evaluator's rules, and nothing a
//!   noticer reads), and the rung's score and peak score are z-scores of the abnormal count, which
//!   is what `evidence` already carries. So the score has no such term. Should a noticer expose one
//!   later (the medium's confirmation devices are the candidate), it is a sixth feature of a new
//!   unit, and this rule is not changed.
//! - **"A per-stream budget of `k` calls."** At most `k` questions are proposed per segment. A
//!   question the bill refuses still counts as proposed (the harness never refused one in B4's
//!   runs). The budget is the rule's own and does not count calls other arms or rules make.
//! - **"Ask about the top `k` by score as they become ready."** The future is unknown at the ask
//!   instant, so "the top `k`" cannot be taken over all anomalies of the segment (an anomaly asked
//!   about after its deadline is worth nothing). It is realised online: among the anomalies ready
//!   at one step, in descending score, and across steps by the threshold `threshold`, which keeps
//!   budget for a later anomaly that scores higher than the threshold. With every weight zero and
//!   the threshold at zero it is "ask about the first `k` that become ready", the score-free
//!   baseline of the same budget.
//! - **"As they become ready."** Ready is `now >= noticed_at + delay_ns` at a step where the rung
//!   still holds the anomaly. An anomaly the rung retires first (quiet for 6 s) is never ready and
//!   is never asked about, which is the retirement every public arm is subject to; the delay sweep
//!   of the same unit measures what that does to the selection oracle.
//! - **`delay_ns`** is a manifest parameter and is not tuned: 16 s in the tables, R5's delay, the
//!   one the selection oracle uses, so that selection is the only thing that differs from it.
//!
//! # What it cannot do, by construction
//!
//! It reads the rung's public state only. It cannot tell a hard incident from a plain one except
//! through what the five features carry (the unit measures that, feature by feature, as AUCs on the
//! tuning streams), it cannot ask about an anomaly that is not live at the ask instant, it cannot
//! ask twice, and it keeps no memory across segments.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The id this arm is registered under.
pub const ID: &str = "public_budgeted";

/// The evidence count at which the `evidence` feature reaches 1.
pub const EVIDENCE_CAP: f64 = 64.0;

/// The service count at which the `services` feature reaches 1.
pub const SERVICES_CAP: f64 = 8.0;

/// The age, in nanoseconds from the anchor, at which the `age` feature reaches 1: the rung's
/// retention of observations.
pub const AGE_CAP_NS: f64 = 120_000_000_000.0;

/// The five features of the score, each in `[0, 1]`, in the order the weights are written.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Features {
    /// The checker's latest verdict on the attached evidence is "no consistent hypothesis".
    pub contradiction: f64,
    /// The cheap rung has declared nothing about the anomaly.
    pub silence: f64,
    /// Attached abnormal observations, log scale.
    pub evidence: f64,
    /// Distinct services the attached observations are about.
    pub services: f64,
    /// Seconds from the anchor to the instant of the step.
    pub age: f64,
}

/// The names of the features, in field order.
pub const FEATURE_NAMES: [&str; 5] = ["contradiction", "silence", "evidence", "services", "age"];

impl Features {
    /// The features of `view` at `now`: the module documentation's readings, and nothing else.
    pub fn of(view: &AnomalyView, now: Instant) -> Self {
        let clip = |x: f64| x.clamp(0.0, 1.0);
        Self {
            contradiction: f64::from(u8::from(view.contradicted_since.is_some())),
            silence: f64::from(u8::from(!view.cheap_declared)),
            evidence: clip((1.0 + f64::from(view.evidence)).ln() / (1.0 + EVIDENCE_CAP).ln()),
            services: clip(f64::from(view.services) / SERVICES_CAP),
            age: clip(now.0.saturating_sub(view.anchor_at.0) as f64 / AGE_CAP_NS),
        }
    }

    /// The features as an array, in [`FEATURE_NAMES`] order.
    pub fn to_array(&self) -> [f64; 5] {
        [
            self.contradiction,
            self.silence,
            self.evidence,
            self.services,
            self.age,
        ]
    }
}

/// The score's weights and the threshold. Written to a manifest as one object. A weight may be
/// negative; every number is finite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Score {
    /// The threshold: an anomaly is asked about only if its score reaches this.
    pub threshold: f64,
    /// Weight of [`Features::contradiction`].
    pub contradiction: f64,
    /// Weight of [`Features::silence`].
    pub silence: f64,
    /// Weight of [`Features::evidence`].
    pub evidence: f64,
    /// Weight of [`Features::services`].
    pub services: f64,
    /// Weight of [`Features::age`].
    pub age: f64,
}

impl Score {
    /// Whether every number is finite.
    pub fn is_finite(&self) -> bool {
        [
            self.threshold,
            self.contradiction,
            self.silence,
            self.evidence,
            self.services,
            self.age,
        ]
        .iter()
        .all(|x| x.is_finite())
    }

    /// The weighted sum of `f`.
    pub fn of(&self, f: &Features) -> f64 {
        self.contradiction * f.contradiction
            + self.silence * f.silence
            + self.evidence * f.evidence
            + self.services * f.services
            + self.age * f.age
    }
}

/// The rule.
#[derive(Debug, Clone)]
pub struct PublicBudgeted {
    delay_ns: u64,
    k: u32,
    score: Score,
    /// Anomalies this rule has looked at (asked about or passed over).
    seen: BTreeSet<u32>,
    /// Questions proposed so far.
    spent: u32,
}

impl PublicBudgeted {
    /// A rule that asks at most `k` questions, about anomalies ready `delay_ns` after notice whose
    /// `score` reaches its threshold, best first.
    pub fn new(delay_ns: u64, k: u32, score: Score) -> Self {
        Self {
            delay_ns,
            k,
            score,
            seen: BTreeSet::new(),
            spent: 0,
        }
    }

    /// Questions proposed so far.
    pub fn spent(&self) -> u32 {
        self.spent
    }
}

impl EscalationRule for PublicBudgeted {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    /// Always true, whatever the weights: the checker's runs are billed through the meter, and an
    /// arm that read the verdict only when its weight is nonzero would be billed differently from
    /// one that does not, which is a difference between selectors that is not selection.
    fn monitors(&self) -> bool {
        true
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut ready: Vec<(f64, u32)> = Vec::new();
        for v in views {
            if v.attempts == 0
                && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)
                && self.seen.insert(v.id)
            {
                ready.push((self.score.of(&Features::of(v, now)), v.id));
            }
        }
        ready.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut out = Vec::new();
        for (score, id) in ready {
            if self.spent >= self.k {
                break;
            }
            if score >= self.score.threshold {
                self.spent += 1;
                out.push(id);
            }
        }
        out
    }
}
