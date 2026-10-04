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
use crate::payload::{HypothesisEntry, Ranked, hypothesis_entry, unique_best};
use crate::{Component, ComponentOutput, VERIFIER_ID, WorkingState};
use gordian_core::{Charge, ComponentId};
use gordian_world::physics::consistent_hypotheses;

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

    fn run(&mut self, input: &WorkingState) -> ComponentOutput {
        let evidence = input.evidence_vec();
        let set = consistent_hypotheses(&input.public, &evidence);
        if set.is_empty() {
            let entry = HypothesisEntry::EvidenceDamaged {
                source: "verifier".to_string(),
                window: input.size() as u32,
            };
            return ComponentOutput {
                entries: vec![hypothesis_entry(&entry)],
                requests: Vec::new(),
                proposal: None,
            };
        }
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
        ComponentOutput {
            entries: vec![hypothesis_entry(&entry)],
            requests: Vec::new(),
            proposal,
        }
    }
}
