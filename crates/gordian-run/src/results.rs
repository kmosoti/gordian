//! `results.csv` and `measured.csv`.
//!
//! One row per episode, keyed by `(seed, class)`, written in execution order
//! (`Manifest::episodes`). `results.csv` holds only values that are a function of the manifest,
//! so the same manifest gives a byte-identical file (protocol replay). `measured.csv` holds the
//! wall-clock timings, which differ between runs and are never compared byte for byte.
//!
//! # `results.csv` columns
//!
//! The first nineteen columns are the plan's (`docs/local-test-plan.md`, A4), in its order.
//! `directives_ignored` and `stop_reason` are additions at the end, so that the plan's columns
//! keep their positions.
//!
//! ```text
//! run_id, seed, class, success, critical_miss, false_alarm, abstained, undecided,
//! probes_used, corrections, decision_at_ns, bill_compute, bill_memory, bill_time,
//! bill_probes, bill_comm, bill_storage, components_run, components_skipped,
//! directives_ignored, stop_reason
//! ```
//!
//! - Booleans are `true` or `false`.
//! - `probes_used` counts probe actions the world carried out; `bill_probes` is probe units.
//! - `decision_at_ns` is empty when the episode is `undecided`. It is not replaced by the stop
//!   time: an episode that never decided has no decision time.
//! - `bill_compute`, `bill_memory`, `bill_time`, `bill_probes` and `bill_comm` are the bill's
//!   totals for those resources *excluding* what was attributed to `Phase::Storage`;
//!   `bill_storage` is everything attributed to `Phase::Storage`, all resources summed. The six
//!   columns therefore partition the bill: no unit appears twice.
//! - `components_run` counts selected components that were charged (including those a `Fail`
//!   directive made produce nothing); `components_skipped` counts selected components the bill
//!   refused, and repeated selections of one component within a step.
//!
//! # `measured.csv` columns
//!
//! ```text
//! run_id, seed, class, measured_component_ns, measured_sched_ns, measured_harness_ns,
//! arm_position
//! ```
//!
//! `arm_position` is the arm's position in the order its episode was played in: 0 for the arm
//! that played the episode first, 1 for the next, and always 0 in a one-arm run
//! ([`crate::interleave`]). It is a function of the manifest, not a timing, but it lives here
//! because it is only used to read the timings.

use crate::harness::EpisodeRecord;
use gordian_core::{Bill, Phase, Resource};
use std::fmt::Write as _;

/// The header of `results.csv`.
pub const RESULTS_HEADER: &str = "run_id,seed,class,success,critical_miss,false_alarm,abstained,undecided,probes_used,corrections,decision_at_ns,bill_compute,bill_memory,bill_time,bill_probes,bill_comm,bill_storage,components_run,components_skipped,directives_ignored,stop_reason";

/// The header of `measured.csv`.
pub const MEASURED_HEADER: &str =
    "run_id,seed,class,measured_component_ns,measured_sched_ns,measured_harness_ns,arm_position";

const RESOURCES: [Resource; 5] = [
    Resource::Compute,
    Resource::Memory,
    Resource::Time,
    Resource::Probes,
    Resource::Communication,
];

/// The six bill columns, partitioning the bill (see the module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BillColumns {
    /// `Resource::Compute`, outside the storage phase.
    pub compute: u64,
    /// `Resource::Memory`, outside the storage phase.
    pub memory: u64,
    /// `Resource::Time`, outside the storage phase.
    pub time: u64,
    /// `Resource::Probes`, outside the storage phase.
    pub probes: u64,
    /// `Resource::Communication`, outside the storage phase.
    pub comm: u64,
    /// Everything attributed to `Phase::Storage`.
    pub storage: u64,
}

impl BillColumns {
    /// The columns of `bill`.
    pub fn of(bill: &Bill) -> Self {
        let storage_of = |r: Resource| -> u64 {
            bill.by_phase(r)
                .filter(|(phase, _)| *phase == Phase::Storage)
                .map(|(_, amount)| amount)
                .sum()
        };
        let outside = |r: Resource| bill.total(r).saturating_sub(storage_of(r));
        Self {
            compute: outside(Resource::Compute),
            memory: outside(Resource::Memory),
            time: outside(Resource::Time),
            probes: outside(Resource::Probes),
            comm: outside(Resource::Communication),
            storage: RESOURCES.iter().map(|r| storage_of(*r)).sum(),
        }
    }
}

/// The name of an episode class as written in the CSV files and the manifest.
pub fn class_name(class: gordian_world::EpisodeClass) -> String {
    format!("{class:?}")
}

/// One line of `results.csv` for `record`, without a trailing newline.
pub fn results_row(run_id: &str, record: &EpisodeRecord) -> String {
    let v = &record.verdict;
    let b = BillColumns::of(&record.bill);
    let decision = v.decision_at.map(|t| t.0.to_string()).unwrap_or_default();
    let mut row = String::new();
    write!(
        row,
        "{run_id},{seed},{class},{success},{critical},{alarm},{abstained},{undecided},{probes},{corrections},{decision},{c},{m},{t},{p},{k},{s},{run},{skipped},{ignored},{stop}",
        seed = record.seed,
        class = class_name(record.class),
        success = v.success,
        critical = v.critical_miss,
        alarm = v.false_alarm,
        abstained = v.abstained,
        undecided = v.undecided,
        probes = v.probes_used,
        corrections = v.corrections,
        c = b.compute,
        m = b.memory,
        t = b.time,
        p = b.probes,
        k = b.comm,
        s = b.storage,
        run = record.components_run,
        skipped = record.components_skipped,
        ignored = record.directives_ignored,
        stop = record.stop.as_str(),
    )
    .expect("writing to a String cannot fail");
    row
}

/// One line of `measured.csv` for `record`, which was played at `arm_position` in the order of
/// its episode, without a trailing newline.
pub fn measured_row(run_id: &str, record: &EpisodeRecord, arm_position: usize) -> String {
    format!(
        "{run_id},{seed},{class},{c},{s},{h},{arm_position}",
        seed = record.seed,
        class = class_name(record.class),
        c = record.measured.component_ns,
        s = record.measured.sched_ns,
        h = record.measured.harness_ns,
    )
}
