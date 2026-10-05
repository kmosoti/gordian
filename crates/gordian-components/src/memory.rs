//! Lookup of prior records whose symptom signature matches the window.
//!
//! `PublicInfo::prior_records` holds `(signature, resolution)` pairs from earlier episodes. The
//! lookup reads the records, keeps those whose signature equals the set of symptoms in the
//! window, and returns their resolutions as hypotheses. Whether a lookup like this deserves the
//! name "memory" is a question for EXP-004, not something the name settles.
//!
//! # Matching
//!
//! The window's signature is the set of symptom tags it shows (a counter at or above
//! `physics::HIGH`, a catalogue message other than `CheckHealth`), the same definition as
//! `physics::signature`. A record matches when its signature is *equal* to that set. Subset,
//! superset, and fuzzy matches are alternatives an experiment can try. A window with no
//! symptoms matches nothing.
//!
//! A record carries a fault kind but no site. The site of each hypothesis is taken from the
//! window: the service where the kind's characteristic message was seen if there is one, else the
//! earliest service with a high `ErrorRate`, else the service of the first symptom.
//!
//! # Ranking, and being fooled
//!
//! Matching records vote for their resolution; the hypothesis for the kind with the most votes
//! ranks first, and there is a proposal only if that kind is strictly ahead. The lookup does not
//! check a record against the evidence: a stale record whose signature matches is returned like
//! any other, and that is deliberate (the stale-memory stressor of the charter, section 10, and
//! EXP-004 measure what it costs).
//!
//! # Cost
//!
//! Finding the matches reads each record once, so the lookup charges per record read, on top of
//! the work of summarizing the window. An optional read limit stops after the first `k` records;
//! the declared cost uses the number actually read.

use crate::cost::{affine_ns, compute};
use crate::ops::{Ops, Unit};
use crate::payload::{HypothesisEntry, Ranked, hypothesis_entry, unique_best};
use crate::symptoms::{Summary, TAG_MASK, summarize, tag_mask, text_slot};
use crate::{Component, ComponentOutput, ComputationRequest, MEMORY_ID, VERIFIER_ID, WorkingState};
use gordian_core::{Charge, ComponentId};
use gordian_world::physics::characteristic_message;
use gordian_world::{FaultKind, Hypothesis, ServiceId};
use std::cmp::Reverse;

// Declared cost, `Resource::Compute` nanoseconds:
// `A_NS + B_PS * n / 1000 + C_PS * r / 1000` for a window of `n` observations and `r` records
// read. `A_NS` includes the cost of emitting an entry in the share of windows where a record
// matched (18 of 22 in the calibration pool), so a window with no match costs less than declared.
// Fitted on 2026-10-04, Intel(R) Xeon(R) Processor @ 2.80GHz (4 vCPU VM), `bench` profile with the
// workspace release settings (`debug = 1`, thin LTO, one codegen unit), on core 2 under
// `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100`, then `calibrate.py` (weighted least squares
// on relative error), constants rounded from the fits of three runs, checked against a fourth.
// The coordinator reruns the calibration before merge. Ratios and the shape of the fit:
// CALIBRATION.md. Recalibrate after a change to the CPU, the release profile, or this
// component's code.
const A_NS: u64 = 275;
const B_PS: u64 = 2_140;
const C_PS: u64 = 2_710;

// Counted operations (work item A8b; `CALIBRATION.md`, section 9). One `run` counts:
//
// - `calls`: one per call;
// - `scanned`: observations the one pass over the window looked at;
// - `records`: prior records whose signature was compared with the window's;
// - `entries`: entries emitted (0 or 1; a window with no symptom or no matching record emits none);
// - `ranked`: candidates written into the entry.
const U_CALLS: usize = 0;
const U_SCANNED: usize = 1;
const U_RECORDS: usize = 2;
const U_ENTRIES: usize = 3;
const U_RANKED: usize = 4;

/// The lookup's units and their weights.
pub const UNITS: &[Unit] = &[
    Unit {
        name: "calls",
        weight_ps: 0,
    },
    Unit {
        name: "scanned",
        weight_ps: 0,
    },
    Unit {
        name: "records",
        weight_ps: 0,
    },
    Unit {
        name: "entries",
        weight_ps: 0,
    },
    Unit {
        name: "ranked",
        weight_ps: 0,
    },
];

/// The prior-record lookup.
#[derive(Debug, Clone, Copy)]
pub struct PriorRecordLookup {
    read_limit: usize,
}

impl Default for PriorRecordLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl PriorRecordLookup {
    /// A lookup that reads every record.
    pub fn new() -> Self {
        Self {
            read_limit: usize::MAX,
        }
    }

    /// A lookup that reads at most the first `limit` records, in the order `PublicInfo` lists
    /// them.
    pub fn with_read_limit(limit: usize) -> Self {
        Self { read_limit: limit }
    }

    fn records_read(&self, input: &WorkingState) -> usize {
        input.public.prior_records.len().min(self.read_limit)
    }
}

fn site_for(kind: FaultKind, summary: &Summary) -> Option<ServiceId> {
    summary.first[text_slot(characteristic_message(kind))]
        .or(summary.first_error())
        .or(summary.first_abnormal)
}

impl Component for PriorRecordLookup {
    fn id(&self) -> ComponentId {
        MEMORY_ID
    }

    fn declared_cost(&self, input: &WorkingState) -> Vec<Charge> {
        let read = self.records_read(input);
        let ns = affine_ns(A_NS, B_PS, input.size()).saturating_add(affine_ns(0, C_PS, read));
        vec![compute(ns)]
    }

    fn run_counted(&mut self, input: &WorkingState) -> (ComponentOutput, Ops) {
        let mut ops = Ops::zero(MEMORY_ID);
        ops.add(U_CALLS, 1);
        ops.add(U_SCANNED, input.size() as u64);
        let summary = summarize(&input.public, input.evidence());
        let window = summary.mask & TAG_MASK;
        if window == 0 {
            return (ComponentOutput::default(), ops);
        }
        let mut votes = [0u32; 5];
        for record in input
            .public
            .prior_records
            .iter()
            .take(self.records_read(input))
        {
            ops.add(U_RECORDS, 1);
            if tag_mask(&record.signature) == window {
                votes[kind_index(record.resolution)] += 1;
            }
        }
        // Kinds with at least one vote, most votes first, ties in `FaultKind` order.
        let mut kinds: Vec<(FaultKind, u32)> = FaultKind::ALL
            .into_iter()
            .zip(votes)
            .filter(|(_, v)| *v > 0)
            .collect();
        kinds.sort_by_key(|(k, v)| (Reverse(*v), kind_index(*k)));
        let ranked: Vec<Ranked> = kinds
            .iter()
            .filter_map(|(kind, v)| {
                let site = site_for(*kind, &summary)?;
                Some(Ranked {
                    hypothesis: Some((*kind, site)),
                    score: Some(*v),
                })
            })
            .collect();
        let Some(top) = ranked.first().and_then(|r| r.score) else {
            return (ComponentOutput::default(), ops);
        };
        ops.add(U_ENTRIES, 1);
        ops.add(U_RANKED, ranked.len() as u64);
        let tied = ranked.iter().take_while(|r| r.score == Some(top)).count() as u32;
        let proposal: Option<Hypothesis> = unique_best(&ranked, tied);
        let mut requests = Vec::new();
        if proposal.is_some() {
            requests.push(ComputationRequest {
                component: VERIFIER_ID,
                reason: "check prior-record resolution".to_string(),
            });
        }
        let entry = HypothesisEntry::Candidates {
            source: "prior-record lookup".to_string(),
            basis: "records whose signature equals the window's".to_string(),
            ranked,
            tied_at_top: tied,
        };
        let output = ComponentOutput {
            entries: vec![hypothesis_entry(&entry)],
            requests,
            proposal,
        };
        (output, ops)
    }
}

fn kind_index(kind: FaultKind) -> usize {
    FaultKind::ALL
        .iter()
        .position(|k| *k == kind)
        .unwrap_or_default()
}
