//! The recall gate (work item A1c, Lab 1): a recall of the engram layer is acted on only for an
//! anomaly that the public rules cannot explain.
//!
//! Everything in this documentation was written and committed before any run of a gated arm. A
//! change after a run is recorded in `experiments/exploration/a1c-recall-gate.md` with its reason,
//! never edited in here silently.
//!
//! # The public reading
//!
//! "An anomaly the public rules cannot explain" is read as the rung's own consistency checker:
//! [`AnomalyView::contradicted_since`] is set, that is, **the latest check of the first world's
//! consistency verifier on the evidence attached to the anomaly (its abnormal observations and the
//! probes bought for it) found no hypothesis consistent with it** (an empty candidate set, or
//! damaged evidence: the first world's own word for "contradictory", `rung::verdict_is_empty`).
//! Nothing else enters the gate: not the anomaly's score, not its age, not whether the cheap rung
//! has declared, not the tier, not anything the evaluator knows.
//!
//! - **Who keeps the verdict.** The rung, for an arm whose noticer asks
//!   ([`crate::stream::arms::noticer::Noticer::needs_verdicts`]); the same verifier, the same
//!   cadence and the same bill as for `contradiction_escalation` (R5): a check of a noticed anomaly
//!   that received evidence since its last check and has had no escalation, at most once per
//!   `review_ns`, through the meter (charged, counted, recorded), plus the verdict of every review
//!   of the shared rule. A gated arm pays for its checks; nothing is free.
//! - **When it is read.** At each step, after that step's checks (the arm runs them before it takes
//!   the recalls), on the views of that step. A recall resolved to an anomaly whose verdict is not
//!   empty is **kept for at most one review period of the rung** (`review_ns`, 500 ms), and read
//!   again at each step: the checker runs at most once per review period, so the evidence that
//!   completed the recall's pattern may be checked up to one period after the recall. A recall
//!   still not admitted then is dropped (counted). A recall for an anomaly that has been escalated
//!   or answered meanwhile, or that has retired, is dropped (counted).
//! - **What a dropped recall leaves.** Nothing: the anomaly is not marked recalled, so a later
//!   recall of it (the same engram after its refractory time, or another) is gated afresh. A
//!   recall is an event; the gate is a condition on the event, not on the anomaly's history.
//!
//! # What the gate guarantees, and what it does not
//!
//! A gated recall is declared only for an anomaly whose evidence the public checker could not
//! explain at the recall (within one review period). So, **on an anomaly whose evidence the checker
//! finds consistent throughout, a gated arm does exactly what the same arm without memory does**:
//! no recall is declared, the shared rule's declaration is made as in the control, and the anomaly
//! is escalated as in the control (`tests/stream_medium_gate.rs`). That is the sense in which a
//! gated recall "fires only where the cheap rung would have returned no hypothesis": the cheap
//! rung's verifier returned none.
//!
//! It is not a guarantee that a gated recall never displaces a correct declaration, and the report
//! says so: (1) the shared rule does not stop at an empty verifier set; it falls back to the
//! estimator's top-tied hypotheses, then the heuristic's (`policy/decide.rs`, the candidate set is
//! "the first available"), so where the checker is empty the cheap rung may still declare, and a
//! recall made before it preempts that fallback declaration, which can be right; (2) a recall
//! after the cheap rung has declared adds a declaration (A1a's rule, unchanged); (3) a verdict can
//! be empty for a while and then consistent again (evidence of another incident attached, then
//! more evidence), and a recall in the empty stretch is admitted. R5 measured that the public
//! checker contradicts 97% of plain anomalies at some point (review log, R5); the gate is a
//! condition at the recall's instant, and how often plain anomalies are contradicted *then* is
//! what the smoke measures.

//!
//! # A1d: where a recall may speak (`stale`)
//!
//! Written and committed before any run of an A1d arm (`DESIGN.md` of `gordian-medium`, "The
//! engram under a non-privileged selector (A1d)", committed before this code). The gate `stale` is
//! `contradicted` and one more condition: **the anomaly carries no declaration made strictly after
//! the checker's last consistent verdict on it** (the rung keeps both instants and hands the ids
//! whose declaration stands to the noticer before each reading,
//! [`crate::stream::arms::noticer::Noticer::standing_declarations`]). A declaration made when the
//! checker last explained the evidence, or before, has since been contradicted (it is stale) and a
//! recall may speak beside it; a declaration made after (or with no consistent verdict ever) stands,
//! and the recall does not speak. The wait and what a dropped recall leaves are `contradicted`'s.
//! A recall dropped at the end of its wait is counted by the last reason it was read with: a
//! consistent verdict, or a standing declaration.

use crate::stream::arms::rung::AnomalyView;
use serde::{Deserialize, Serialize};

/// Which recalls the arm acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallGate {
    /// Every recall (A1a).
    #[default]
    None,
    /// Only a recall of an anomaly whose latest public consistency check found no consistent
    /// hypothesis ([`AnomalyView::contradicted_since`] set).
    Contradicted,
    /// `Contradicted`, and only on an anomaly with no standing declaration (work item A1d; the
    /// module documentation, "A1d").
    Stale,
}

impl RecallGate {
    /// Whether this is no gate.
    pub fn is_none(&self) -> bool {
        *self == RecallGate::None
    }

    /// Whether the gate needs the rung's consistency verdicts.
    pub fn needs_verdicts(self) -> bool {
        self != RecallGate::None
    }

    /// Whether the gate reads standing declarations (A1d).
    pub fn reads_declarations(self) -> bool {
        self == RecallGate::Stale
    }

    /// Whether a recall of the anomaly `view` is admitted now, on the verdict alone (for
    /// `stale`, the condition on declarations is [`RecallGate::reading`]'s).
    pub fn admits(self, view: &AnomalyView) -> bool {
        match self {
            RecallGate::None => true,
            RecallGate::Contradicted | RecallGate::Stale => view.contradicted_since.is_some(),
        }
    }

    /// What the gate reads of the anomaly `view`, given whether it carries a standing declaration
    /// (`standing`; read only by `stale`).
    pub fn reading(self, view: &AnomalyView, standing: bool) -> Reading {
        if !self.admits(view) {
            Reading::Consistent
        } else if self.reads_declarations() && standing {
            Reading::Standing
        } else {
            Reading::Admit
        }
    }
}

/// What the gate read of an anomaly at one step (A1d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Admitted.
    Admit,
    /// Not admitted: the latest verdict found a consistent hypothesis (or none was made).
    Consistent,
    /// Not admitted: contradicted, but the anomaly carries a standing declaration (`stale` only).
    Standing,
}
