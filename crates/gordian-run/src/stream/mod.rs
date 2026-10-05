//! The stream harness and the conventional escalation baselines (work item R3 in
//! `docs/local-test-plan.md`).
//!
//! The episode harness plays isolated episodes of the first world. This module plays **stream
//! segments** of `gordian-stream`: one long-running world with plain, hard and decoy incidents, a
//! simulated reasoner that costs several orders of magnitude more than a component call, and a
//! cheap rung that must decide what deserves it. What is built here, and what is not, is stated
//! once; the code is authoritative and the files below say how each part works.
//!
//! | File | What |
//! |---|---|
//! | `harness.rs` | the loop, the accounting, the privileged entry point; the only place a stream is generated and its truth built |
//! | `meter.rs` | the one way an arm runs a component or the shared rule: charged first, timed, counted, recorded |
//! | `arms/` | the shared cheap rung (`rung.rs`), its context builders (`context.rs`, R6) and the escalation rules (`never`, `always`, `periodic`, `change`, `threshold`, `random`, `contradiction`), and the labelled ablation (`ablation.rs`) |
//! | `oracle.rs` | the privileged arms (`oracle_escalation`, R5's `oracle_selection` and `oracle_decoy`, and R6's supplementary `oracle_selection_context`), and R10's `oracle_notice`, declared as module `privileged` |
//! | `score.rs` | the stream evaluator's types as the harness uses them, the counts read from the trajectory alone, the hard-fault family names |
//! | `manifest.rs`, `spec.rs` | the manifest, the arms as it writes them, the registry |
//! | `results.rs`, `recorder.rs` | `results.csv`, `incidents.csv`, `measured.csv`, `drift.csv`, the events sample, interleaving |
//!
//! # Segments
//!
//! The unit of replication is the **stream segment: one whole stream, `duration_ns` long (600 s
//! by default), generated from one seed.** Its length is the stream's, not a slice of it, for the
//! reasons of charter section 8 (an episode is an episode; a million events from one trajectory are
//! not a million replications):
//!
//! - The incidents of one stream are dependent. They share a service graph, a noise process, a
//!   vocabulary of hidden message ids, recurrences of earlier incidents and the regime changes
//!   (at 200 s and 400 s by default) that make an arm's earlier knowledge stale; an arm that learns
//!   from its own history (the substrate, later) carries that history from one incident to the
//!   next. Slicing a stream into segments would hand the analysis several rows that share all of
//!   it, each looking like an independent replication.
//! - Each arm's budget (probes, probe time, reasoner tokens, substrate compute) is per stream, as
//!   the stream's own budget is, so a fresh budget and bill per segment is a fresh budget and bill
//!   per stream.
//! - A shorter or longer segment is a manifest parameter (`stream_params.duration_ns`): a stream
//!   of that duration, generated afresh from each seed, never a cut of a longer one. Independent
//!   environment instances are the seeds.
//!
//! One consequence the analysis must respect: a segment holds about 27 incidents, about two or
//! three of them hard, so it is a *cluster*. Per-incident outcomes are aggregated per segment
//! (or analysed with cluster-robust intervals); counting incidents from different segments as
//! independent is the error section 8 warns of. The headroom check (R4) needs on the order of 200
//! segments for 450 hard incidents.
//!
//! # What is costed
//!
//! Total cost is modelled substrate and rule cost plus reasoner cost (charter section 12), with
//! the reasoner's cost reported separately and in its own units:
//!
//! - *Substrate and rule*: the counted operations of the components the cheap rung runs and of the
//!   shared decision rule, weighted by the calibrated constants of `gordian-components` and
//!   `policy::decide` (the A8b modelled cost, unchanged). **Not counted**: the arm's own
//!   bookkeeping (the anomaly tracker, scores, the context builder) and the harness's. The
//!   bookkeeping is charged to the bill as one declared cost per step (placeholder constants, not
//!   fitted: `arms/mod.rs`) and appears in `measured_sched_ns`, but has no calibrated counted
//!   weights, so it is outside the modelled cost. It is small against the components' and far
//!   smaller than the reasoner's, but an experiment that varies it (a scoring arm against a
//!   scoreless one) does not see it priced until it is calibrated, which is open work.
//! - *Reasoner*: calls, references, tokens, the world's declared price (`reasoner_modelled_ns`) and
//!   declared latency in `results.csv`; the tokens times the manifest's exchange rate
//!   (`manifest.exchange`) is `reasoner_cost_ns`. The hard limit on the
//!   reasoner is the segment's token budget, enforced by the bill before every call.
//!
//! # Placeholders
//!
//! Nothing here was tuned against any result. The cheap rung's parameters ([`arms::rung::RungConfig`])
//! were read off the stream's public rates (about 1.1 abnormal-looking background observations a
//! second over 8 to 12 services, incidents burst and then beat about once a second). The
//! threshold arm's `tau` and wait, the periodic arm's period and the random arm's `p` are
//! placeholders to be tuned per budget before any comparison (charter section 7: tune each
//! baseline before comparing). The substrate allowance (2 s of declared compute per segment) and
//! the 500 ms step are provisional and do not bind at the defaults.
//!
//! # The arm-knowledge rule
//!
//! Baselines and the substrate may use the stream's public rules and what they learn from their
//! own run history, never the hidden rules of `gordian-stream/HIDDEN-DESIGN.md` (AGENTS.md, "The rule
//! that matters most"). The shared rung encodes none of them: its only knowledge is the first
//! world's physics, through the components and the shared rule, and public statistics. The
//! knowledge-injected cheap rung exists only as `ablation_hidden_rules`.

pub mod arms;
pub mod harness;
pub mod manifest;
pub mod meter;
pub mod recorder;
pub mod results;
pub mod score;
pub mod spec;

#[path = "oracle.rs"]
pub mod privileged;

pub use harness::{
    SegmentCounts, SegmentRecord, StreamHarnessError, StreamStop, run_segment,
    run_segment_privileged,
};
pub use manifest::{Exchange, StreamArmSpec, StreamLimits, StreamManifest};
pub use recorder::{StreamRunError, StreamRunReport, execute_stream};
pub use score::{StreamVerdict, TrajectoryCounts};
pub use spec::StreamPolicySpec;
