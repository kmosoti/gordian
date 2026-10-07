//! `public_change`: escalate an anomaly once, at R5's delay after it is noticed, if its attached
//! evidence has grown by `k` observations since the notice (work item B4).
//!
//! The second non-privileged selector B4 asks for: *"a change-triggered rule: escalate once per
//! anomaly when its attached evidence grows by `k` observations after notice"*. It is the
//! charter's change-triggered baseline read at the level of one anomaly's evidence, and it differs
//! from the existing `change_triggered` arm (which escalates at notice and again when the evidence
//! digest changes) in what counts as a change (a count, not a digest), in asking at most once, and
//! in asking at R5's delay. Status: built, with the readings below stated before any tuning; `k`
//! is tuned on seeds 10000-10099 (`scripts/b4_*.py`).
//!
//! # Readings of the brief's words (fixed before any run)
//!
//! - **"Attached evidence."** [`AnomalyView::evidence`]: the abnormal observations the noticer has
//!   attached to the anomaly. For the rung's noticers, the re-anchor and the splitting noticer
//!   that is the count of abnormal observations of the anomaly; for a ramp-noticed anomaly it
//!   also holds the readings of the chain, benign ones included (the ramp attaches every reading
//!   that continues its chain); for the medium it is what the medium's proposals cite. The count is
//!   read from the view; the rule reads nothing else of the anomaly.
//! - **"After notice."** The count is read the first time the rule sees the anomaly, which is the
//!   step at which it is noticed (the views are made after the noticer has run), and that is the
//!   baseline; the evidence has *grown by `k`* when `evidence >= baseline + k`. A count that
//!   falls (a re-anchor or a split moves observations out of the anomaly) is not growth, and the
//!   baseline is not lowered: the anomaly must regain its baseline and then grow.
//! - **"Once per anomaly."** `attempts == 0`: a second call is never made.
//! - **"When."** The first step at or after `noticed_at + delay_ns` at which the growth is `k` or
//!   more, while the anomaly is live. Growth that happens and is complete before `delay_ns` still
//!   counts: the rule compares the count at the step with the baseline, not the growth since the
//!   previous step. `delay_ns` is R5's 16 s, fixed by the table and not tuned, so that selection
//!   is the only variable between this arm and the selection oracle.
//!
//! # What it cannot do, by construction
//!
//! An anomaly that stops growing before it is noticed again (a resolved decoy) is not asked about
//! and neither is a hard incident whose second phase carries few observations; a plain incident
//! whose burst keeps attaching observations is asked about as readily as a hard one.

use super::EscalationRule;
use super::rung::AnomalyView;
use crate::policy::PolicyId;
use gordian_core::Instant;
use std::collections::BTreeMap;

/// The id this arm is registered under.
pub const ID: &str = "public_change";

/// The rule.
#[derive(Debug, Clone)]
pub struct PublicChange {
    delay_ns: u64,
    k: u32,
    /// The evidence each anomaly had when the rule first saw it.
    baseline: BTreeMap<u32, u32>,
}

impl PublicChange {
    /// A rule that asks `delay_ns` after notice about an anomaly whose evidence has grown by at
    /// least `k` (at least 1) since the notice.
    pub fn new(delay_ns: u64, k: u32) -> Self {
        Self {
            delay_ns,
            k: k.max(1),
            baseline: BTreeMap::new(),
        }
    }
}

impl EscalationRule for PublicChange {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }

    fn targets(&mut self, now: Instant, views: &[AnomalyView]) -> Vec<u32> {
        let mut out = Vec::new();
        for v in views {
            let base = *self.baseline.entry(v.id).or_insert(v.evidence);
            if v.attempts == 0
                && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)
                && v.evidence >= base.saturating_add(self.k)
            {
                out.push(v.id);
            }
        }
        out
    }
}
