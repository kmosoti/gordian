//! Checks the window against the world's public rules.
//!
//! The verifier calls `physics::consistent_hypotheses` on the window and returns the consistent
//! set: every hypothesis (including "no fault") that no observation in the window contradicts.
//! It does not rank, and it uses no prior record.
//!
//! # An empty set
//!
//! The rules permit and never require, and every generated episode's true hypothesis is
//! consistent with every prefix of its stream. An empty set therefore does not mean the world
//! contradicts itself. It means the *window* does: typically the early `ErrorRate` at the fault
//! site that anchors later observations at dependents was evicted (see
//! [`crate::working`]). The verifier then emits [`HypothesisEntry::EvidenceDamaged`] and
//! proposes nothing.
//!
//! # Proposal
//!
//! A proposal exists only when the set has exactly one member. That is rare before a probe has
//! been paid for, because a stream that only permits symptoms leaves several hypotheses open.

use crate::cost::{affine_ns, compute};
use crate::ops::{Ops, Unit};
use crate::payload::{HypothesisEntry, Ranked, hypothesis_entry, unique_best};
use crate::{Component, ComponentOutput, VERIFIER_ID, WorkingState};
use gordian_core::{Charge, ComponentId};
use gordian_world::physics::consistent_hypotheses_counted;

// Declared cost, `Resource::Compute` nanoseconds: `A_NS + B_PS * n / 1000 + C_PS * s / 1000` for
// a window of `n` observations over `s` services.
// Fitted on 2026-10-04, Intel(R) Xeon(R) Processor @ 2.80GHz (4 vCPU VM), `bench` profile with the
// workspace release settings (`debug = 1`, thin LTO, one codegen unit), on core 2 under
// `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100`, then `calibrate.py` (weighted least squares
// on relative error), constants rounded from the fits of three runs, checked against a fourth.
// The coordinator reruns the calibration before merge. Ratios and the shape of the fit:
// CALIBRATION.md. Recalibrate after a change to the CPU, the release profile, or this
// component's code.
// The checker this component calls, `physics::consistent_hypotheses`, is linear in `n` on every
// input shape, the late anchor included (gordian-world DESIGN.md, section 8), which is what makes
// an affine model right here. The previous constants were fitted before that change.
const A_NS: u64 = 415;
const B_PS: u64 = 5_770;
const C_PS: u64 = 156_000;

// Counted operations (work item A8b; `CALIBRATION.md`, section 9). One `run` counts:
//
// - `candidates` or `damaged`: exactly one of the two per call, the envelope of the entry the call
//   emits, which is a list of candidates when the consistent set is not empty and the shorter
//   damaged-evidence entry when it is empty (the window lost the anchor); the call's own fixed
//   cost is in them;
// - `scanned`: observations touched: the copy of the window into the checker's slice, and the
//   checker's first pass (so twice the window length unless the pass ends early);
// - `worlds`, `evals`, `probe_evals`: the checker's own counts (`physics::CheckerOps`): candidate
//   worlds tried, evaluations of a world against an observation, and against a probe result;
// - `ranked`: hypotheses written into the entry, which is the whole consistent set.
//
// The checker also counts the steps of building its dependents masks (`mask_steps`). The verifier
// does not price them: they depend on the graph alone, and the fit cannot tell them from `worlds`
// (dropping them changes no fit by more than 0.0002 in R^2).
const U_CANDIDATES: usize = 0;
const U_DAMAGED: usize = 1;
const U_SCANNED: usize = 2;
const U_WORLDS: usize = 3;
const U_EVALS: usize = 4;
const U_PROBE_EVALS: usize = 5;
const U_RANKED: usize = 6;

// Weights, picoseconds per unit: fitted 2026-10-05 on the 4 vCPU Xeon, core 2 under scripts/cgroup-run.sh, minimum over 5 runs of 25 timings per datum.
// Non-negative least squares on the minimum time per call, weighted by 1/time, with no
// intercept beyond the explicit per-call unit; rounded to three figures. What each unit
// counts, the fit, its validity and its limits: CALIBRATION.md, section 9. Recalibrate
// after a change to the CPU, the release profile, or the code that is counted.
#[rustfmt::skip]
/// The verifier's units and their weights.
pub const UNITS: &[Unit] = &[
    Unit { name: "candidates", weight_ps: 262000 },
    Unit { name: "damaged", weight_ps: 34400 },
    Unit { name: "scanned", weight_ps: 1540 },
    Unit { name: "worlds", weight_ps: 7440 },
    Unit { name: "evals", weight_ps: 6710 },
    Unit { name: "probe_evals", weight_ps: 21600 },
    Unit { name: "ranked", weight_ps: 61500 },
];

/// The consistency verifier.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConsistencyVerifier;

impl ConsistencyVerifier {
    /// The verifier.
    pub fn new() -> Self {
        Self
    }
}

impl Component for ConsistencyVerifier {
    fn id(&self) -> ComponentId {
        VERIFIER_ID
    }

    fn declared_cost(&self, input: &WorkingState) -> Vec<Charge> {
        let ns = affine_ns(A_NS, B_PS, input.size()).saturating_add(affine_ns(
            0,
            C_PS,
            input.public.services.len(),
        ));
        vec![compute(ns)]
    }

    fn run_counted(&mut self, input: &WorkingState) -> (ComponentOutput, Ops) {
        let mut ops = Ops::zero(VERIFIER_ID);
        let evidence = input.evidence_vec();
        ops.add(U_SCANNED, evidence.len() as u64);
        let (set, checked) = consistent_hypotheses_counted(&input.public, &evidence);
        ops.add(U_SCANNED, checked.scanned);
        ops.add(U_WORLDS, checked.worlds_tried);
        ops.add(U_EVALS, checked.evals);
        ops.add(U_PROBE_EVALS, checked.probe_evals);
        if set.is_empty() {
            ops.add(U_DAMAGED, 1);
            let entry = HypothesisEntry::EvidenceDamaged {
                source: "verifier".to_string(),
                window: input.size() as u32,
            };
            let output = ComponentOutput {
                entries: vec![hypothesis_entry(&entry)],
                requests: Vec::new(),
                proposal: None,
            };
            return (output, ops);
        }
        ops.add(U_CANDIDATES, 1);
        ops.add(U_RANKED, set.len() as u64);
        let ranked: Vec<Ranked> = set
            .into_iter()
            .map(|hypothesis| Ranked {
                hypothesis,
                score: None,
            })
            .collect();
        let tied = ranked.len() as u32;
        let proposal = unique_best(&ranked, tied);
        let entry = HypothesisEntry::Candidates {
            source: "verifier".to_string(),
            basis: "hypotheses no observation in the window contradicts".to_string(),
            ranked,
            tied_at_top: tied,
        };
        let output = ComponentOutput {
            entries: vec![hypothesis_entry(&entry)],
            requests: Vec::new(),
            proposal,
        };
        (output, ops)
    }
}
