//! `results.csv` and `measured.csv` of a stream run.
//!
//! One row per stream segment, keyed by the stream's seed, written in the order the manifest lists
//! the seeds. `results.csv` holds only values that are a function of the manifest, so the same
//! manifest gives a byte-identical file (protocol replay); `measured.csv` holds the wall-clock
//! timings, which differ between runs and are never compared byte for byte.
//!
//! # `results.csv` columns
//!
//! ```text
//! run_id, arm_role, seed, duration_ns, observations, anomalies_noticed,
//! <the scorer's columns: StreamVerdict::HEADER>,
//! cheap_declarations, reasoner_declarations, escalations_refused, probes_refused,
//! calls_unanswered, reasoner_cost_ns,
//! bill_compute, bill_probes, bill_time, bill_comm,
//! components_run, components_skipped, rule_skipped, steps, stop_reason,
//! ops_component, ops_sched, modelled_component_ns, modelled_sched_ns, substrate_ns, total_cost_ns
//! ```
//!
//! - `arm_role` is `comparison`, `privileged` or `ablation`. An analysis compares `comparison`
//!   arms; the other two are headroom and knowledge references and say so on every row.
//! - The scorer's columns are whatever the scorer reports. With the count scorer they are
//!   `probes_used, declarations, declared_incident, declared_dismissal, reasoner_calls,
//!   reasoner_tokens, reasoner_latency_ns, reasoner_declared_ns`: how much the arm did and what
//!   the reasoner charged in its own units. Whether a declaration was right is the evaluator's.
//! - `cheap_declarations` and `reasoner_declarations` split the declarations the stream recorded
//!   by where they came from (the shared cheap rung, or a reasoner's answer).
//! - `escalations_refused` counts calls refused with nothing charged (the segment's token limit
//!   would be exceeded, or the context was malformed); `calls_unanswered` counts accepted calls
//!   whose answer was due after the stream's end, paid for and never seen.
//! - **Cost.** The reasoner's cost is in its own units in the scorer's columns (`reasoner_calls`,
//!   `reasoner_tokens`, `reasoner_latency_ns`) and `reasoner_cost_ns` is the tokens converted at
//!   the manifest's exchange rate. `substrate_ns` is modelled substrate and rule cost: the
//!   components' and the shared rule's counted work, weighted (`modelled_component_ns` +
//!   `modelled_sched_ns`). `total_cost_ns = substrate_ns + reasoner_cost_ns`, with the reasoner's
//!   share also reported separately.
//! - `bill_compute` is declared substrate and rule cost (components, the shared rule, the arm's
//!   declared bookkeeping), the bill's hard limit; `bill_comm` is the reasoner's tokens, its hard
//!   limit; `bill_probes` and `bill_time` are probe units and probe time.
//! - `stop_reason` is `horizon` or `step_cap`.
//!
//! # `measured.csv` columns
//!
//! ```text
//! run_id, seed, measured_component_ns, measured_sched_ns, measured_harness_ns, arm_position
//! ```
//!
//! Named as the episode harness names them, so that `scripts/run-driver.sh` sums them for
//! `internal_external_ratio` unchanged. `measured_sched_ns` is the rule's calls plus the arm's own
//! bookkeeping (the tracker, scores, the context builder).

use super::harness::SegmentRecord;
use super::score::StreamVerdict;
use gordian_core::Resource;
use std::fmt::Write as _;

/// The header of `results.csv`.
pub fn results_header() -> String {
    format!(
        "run_id,arm_role,seed,duration_ns,observations,anomalies_noticed,{},cheap_declarations,reasoner_declarations,escalations_refused,probes_refused,calls_unanswered,reasoner_cost_ns,bill_compute,bill_probes,bill_time,bill_comm,components_run,components_skipped,rule_skipped,steps,stop_reason,ops_component,ops_sched,modelled_component_ns,modelled_sched_ns,substrate_ns,total_cost_ns",
        StreamVerdict::HEADER
    )
}

/// The header of `measured.csv`.
pub const MEASURED_HEADER: &str =
    "run_id,seed,measured_component_ns,measured_sched_ns,measured_harness_ns,arm_position";

/// One line of `results.csv` for `record`, without a trailing newline.
pub fn results_row(run_id: &str, record: &SegmentRecord) -> String {
    let b = &record.bill;
    let mut row = String::new();
    write!(
        row,
        "{run_id},{role},{seed},{duration},{observations},{noticed},{verdict},{cheap},{reasoner},{esc_refused},{probe_refused},{unanswered},{reasoner_cost},{c},{p},{t},{k},{run},{skipped},{rule_skipped},{steps},{stop},{ops_c},{ops_s},{mod_c},{mod_s},{substrate},{total}",
        role = record.role.as_str(),
        seed = record.seed,
        duration = record.public.duration_ns,
        observations = record.observations,
        noticed = record.anomalies_noticed,
        verdict = record.verdict.row(),
        cheap = record.counts.cheap_declarations,
        reasoner = record.counts.reasoner_declarations,
        esc_refused = record.counts.escalations_refused,
        probe_refused = record.counts.probes_refused,
        unanswered = record.counts.calls_unanswered,
        reasoner_cost = record.reasoner_cost_ns,
        c = b.total(Resource::Compute),
        p = b.total(Resource::Probes),
        t = b.total(Resource::Time),
        k = b.total(Resource::Communication),
        run = record.components_run,
        skipped = record.components_skipped,
        rule_skipped = record.rule_skipped,
        steps = record.steps,
        stop = record.stop.as_str(),
        ops_c = record.ops.ops_component(),
        ops_s = record.ops.ops_sched(),
        mod_c = record.ops.modelled_component_ns(),
        mod_s = record.ops.modelled_sched_ns(),
        substrate = record.substrate_ns(),
        total = record.total_cost_ns(),
    )
    .expect("writing to a String cannot fail");
    row
}

/// One line of `measured.csv` for `record`, which was played at `arm_position` in the order of its
/// segment, without a trailing newline.
pub fn measured_row(run_id: &str, record: &SegmentRecord, arm_position: usize) -> String {
    format!(
        "{run_id},{seed},{c},{s},{h},{arm_position}",
        seed = record.seed,
        c = record.measured.component_ns,
        s = record.measured.sched_ns,
        h = record.measured.harness_ns,
    )
}
