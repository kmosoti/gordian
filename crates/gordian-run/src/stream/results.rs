//! `results.csv`, `incidents.csv` and `measured.csv` of a stream run.
//!
//! `results.csv` has one row per stream segment, keyed by the stream's seed; `incidents.csv` one
//! row per incident of each segment, keyed by (seed, incident id). Both are written in the order
//! the manifest lists the seeds, and both hold only values that are a function of the manifest, so
//! the same manifest gives byte-identical files (protocol replay); `measured.csv` holds the
//! wall-clock timings, which differ between runs and are never compared byte for byte.
//!
//! # `results.csv` columns
//!
//! ```text
//! run_id, arm_role, seed, duration_ns, observations, anomalies_noticed,
//! probes_used, declarations, declared_incident, declared_dismissal,
//! incidents_plain, incidents_hard, incidents_decoy, critical_incidents,
//! correct_plain, correct_hard, missed_plain, missed_hard,
//! critical_missed_plain, critical_missed_hard, wrong_declarations,
//! decoys_dismissed, decoys_alarmed, decoys_silent, false_alarms, false_alarms_on_background,
//! escalations_needed, escalations_unneeded, escalations_background,
//! hard_incidents_escalated, other_incidents_escalated, calls_informed, calls_correct,
//! reasoner_calls, reasoner_refs, reasoner_tokens, reasoner_modelled_ns, reasoner_latency_ns,
//! cheap_declarations, reasoner_declarations, escalations_refused, probes_refused,
//! calls_unanswered, reasoner_cost_ns,
//! bill_compute, bill_probes, bill_time, bill_comm,
//! components_run, components_skipped, rule_skipped, steps, stop_reason,
//! ops_component, ops_sched, modelled_component_ns, modelled_sched_ns, substrate_ns, total_cost_ns
//! ```
//!
//! - `arm_role` is `comparison`, `privileged` or `ablation`. An analysis compares `comparison`
//!   arms; the other two are headroom and knowledge references and say so on every row.
//! - `probes_used` to `declared_dismissal` and `reasoner_latency_ns` are read from the trajectory
//!   alone ([`TrajectoryCounts`](super::score::TrajectoryCounts)); the rest from `incidents_plain`
//!   to `reasoner_modelled_ns` are the evaluator's [`StreamTotals`](super::score::StreamTotals),
//!   one column per count, defined by the rows of the evaluator's `RULES.md`
//!   (`incidents_*` and `critical_incidents` S19; `correct_*`, `missed_*`, `critical_missed_*` and
//!   `wrong_declarations` S20; `decoys_*` S21; `false_alarms*` S22; `escalations_needed`,
//!   `_unneeded` and `_background` S23; `hard_incidents_escalated` and `other_incidents_escalated`
//!   S24; `calls_informed` and `calls_correct` S25; `reasoner_calls`, `_refs`, `_tokens` and
//!   `_modelled_ns` S26). The tier counts and the verdict columns are hidden-side facts, evaluator
//!   output like the first world's `class` and `critical_miss`: see `HARNESS.md`, section 11.
//! - The columns hold counts, not ratios. Escalation precision and recall, and any rate of
//!   correct incidents, must be pooled from counts across segments by the analysis
//!   (the evaluator's `RULES.md`, "Derived ratios"); a per-segment ratio is not written.
//! - `cheap_declarations` and `reasoner_declarations` split the declarations the stream recorded
//!   by where they came from (the shared cheap rung, or a reasoner's answer).
//! - `escalations_refused` counts calls refused with nothing charged (the segment's token limit
//!   would be exceeded, or the context was malformed); `calls_unanswered` counts accepted calls
//!   whose answer was due after the stream's end, paid for and never seen.
//! - **Cost.** The reasoner's cost is in its own units (`reasoner_calls`, `reasoner_refs`,
//!   `reasoner_tokens`, `reasoner_modelled_ns` the world's declared price, `reasoner_latency_ns`)
//!   and `reasoner_cost_ns` is the tokens converted at the manifest's exchange rate.
//!   `substrate_ns` is modelled substrate and rule cost: the components' and the shared rule's
//!   counted work, weighted (`modelled_component_ns` + `modelled_sched_ns`).
//!   `total_cost_ns = substrate_ns + reasoner_cost_ns`, with the reasoner's share also reported
//!   separately.
//! - `bill_compute` is declared substrate and rule cost (components, the shared rule, the arm's
//!   declared bookkeeping), the bill's hard limit; `bill_comm` is the reasoner's tokens, its hard
//!   limit; `bill_probes` and `bill_time` are probe units and probe time.
//! - `stop_reason` is `horizon` or `step_cap`.
//!
//! # `incidents.csv` columns
//!
//! ```text
//! run_id, arm_role, seed, incident, tier, family, critical,
//! correct_declarations, wrong_declarations, first_correct_at_ns, time_to_first_correct_ns,
//! correct_by_deadline, missed, critical_miss,
//! escalations, informed_escalations, correct_escalations
//! ```
//!
//! One row per incident of the stream, in id order, whether or not the arm touched it: the fields
//! of the evaluator's `IncidentVerdict`, plus `family`, the name of a hard incident's hard-fault
//! family (`compound`, `cascade`, `split_brain`, `slow_leak`) and empty for plain incidents and
//! decoys. `first_correct_at_ns` and `time_to_first_correct_ns` are empty when there was no
//! correct declaration (the episode files' `decision_at_ns` convention). `tier`, `family` and
//! `critical` are hidden state: this file is evaluator output, for the analysis only, and must
//! never be read by a policy or be a training input (`HARNESS.md`, section 11).
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

use super::arms::noticer::NoticeKind;
use super::harness::SegmentRecord;
use gordian_core::Resource;
use gordian_stream::Tier;
use std::fmt::Write as _;

/// The header of `results.csv`.
pub const RESULTS_HEADER: &str = "run_id,arm_role,seed,duration_ns,observations,anomalies_noticed,probes_used,declarations,declared_incident,declared_dismissal,incidents_plain,incidents_hard,incidents_decoy,critical_incidents,correct_plain,correct_hard,missed_plain,missed_hard,critical_missed_plain,critical_missed_hard,wrong_declarations,decoys_dismissed,decoys_alarmed,decoys_silent,false_alarms,false_alarms_on_background,escalations_needed,escalations_unneeded,escalations_background,hard_incidents_escalated,other_incidents_escalated,calls_informed,calls_correct,reasoner_calls,reasoner_refs,reasoner_tokens,reasoner_modelled_ns,reasoner_latency_ns,cheap_declarations,reasoner_declarations,escalations_refused,probes_refused,calls_unanswered,reasoner_cost_ns,bill_compute,bill_probes,bill_time,bill_comm,components_run,components_skipped,rule_skipped,steps,stop_reason,ops_component,ops_sched,modelled_component_ns,modelled_sched_ns,substrate_ns,total_cost_ns";

/// The header of `incidents.csv`.
pub const INCIDENTS_HEADER: &str = "run_id,arm_role,seed,incident,tier,family,critical,correct_declarations,wrong_declarations,first_correct_at_ns,time_to_first_correct_ns,correct_by_deadline,missed,critical_miss,escalations,informed_escalations,correct_escalations";

/// The header of `results.csv`.
pub fn results_header() -> String {
    RESULTS_HEADER.to_owned()
}

/// The header of `incidents.csv`.
pub fn incidents_header() -> String {
    INCIDENTS_HEADER.to_owned()
}

/// The header of `measured.csv`.
pub const MEASURED_HEADER: &str =
    "run_id,seed,measured_component_ns,measured_sched_ns,measured_harness_ns,arm_position";

/// One line of `results.csv` for `record`, without a trailing newline.
pub fn results_row(run_id: &str, record: &SegmentRecord) -> String {
    let b = &record.bill;
    let t = &record.verdict.totals;
    let c = &record.trajectory_counts;
    let mut row = String::new();
    write!(
        row,
        "{run_id},{role},{seed},{duration},{observations},{noticed},{probes},{decl},{decl_inc},{decl_dis},{inc_plain},{inc_hard},{inc_decoy},{critical},{ok_plain},{ok_hard},{miss_plain},{miss_hard},{cmiss_plain},{cmiss_hard},{wrong},{dismissed},{alarmed},{silent},{false_alarms},{false_bg},{esc_needed},{esc_unneeded},{esc_bg},{hard_esc},{other_esc},{informed},{correct},{r_calls},{r_refs},{r_tokens},{r_modelled},{r_latency},{cheap},{reasoner},{esc_refused},{probe_refused},{unanswered},{reasoner_cost},{bc},{bp},{bt},{bk},{run},{skipped},{rule_skipped},{steps},{stop},{ops_c},{ops_s},{mod_c},{mod_s},{substrate},{total}",
        role = record.role.as_str(),
        seed = record.seed,
        duration = record.public.duration_ns,
        observations = record.observations,
        noticed = record.anomalies_noticed,
        probes = c.probes_used,
        decl = c.declarations,
        decl_inc = c.declared_incident,
        decl_dis = c.declared_dismissal,
        inc_plain = t.incidents.plain,
        inc_hard = t.incidents.hard,
        inc_decoy = t.incidents.decoy,
        critical = t.critical_incidents,
        ok_plain = t.correct.plain,
        ok_hard = t.correct.hard,
        miss_plain = t.missed.plain,
        miss_hard = t.missed.hard,
        cmiss_plain = t.critical_missed.plain,
        cmiss_hard = t.critical_missed.hard,
        wrong = t.wrong_declarations,
        dismissed = t.decoys_dismissed,
        alarmed = t.decoys_alarmed,
        silent = t.decoys_silent,
        false_alarms = t.false_alarms,
        false_bg = t.false_alarms_on_background,
        esc_needed = t.escalations.needed,
        esc_unneeded = t.escalations.unneeded,
        esc_bg = t.escalations.background,
        hard_esc = t.escalations.hard_incidents_escalated,
        other_esc = t.escalations.other_incidents_escalated,
        informed = t.escalations.informed,
        correct = t.escalations.correct,
        r_calls = t.reasoner.calls,
        r_refs = t.reasoner.refs,
        r_tokens = t.reasoner.tokens,
        r_modelled = t.reasoner.modelled_ns,
        r_latency = c.reasoner_latency_ns,
        cheap = record.counts.cheap_declarations,
        reasoner = record.counts.reasoner_declarations,
        esc_refused = record.counts.escalations_refused,
        probe_refused = record.counts.probes_refused,
        unanswered = record.counts.calls_unanswered,
        reasoner_cost = record.reasoner_cost_ns,
        bc = b.total(Resource::Compute),
        bp = b.total(Resource::Probes),
        bt = b.total(Resource::Time),
        bk = b.total(Resource::Communication),
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

/// The name of a tier in `incidents.csv`.
fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Plain => "plain",
        Tier::Hard => "hard",
        Tier::Decoy => "decoy",
    }
}

/// The lines of `incidents.csv` for `record`: one per incident, in id order, each without a
/// trailing newline.
pub fn incident_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    debug_assert_eq!(
        record.verdict.per_incident.len(),
        record.incident_families.len()
    );
    record
        .verdict
        .per_incident
        .iter()
        .zip(&record.incident_families)
        .map(|(v, family)| {
            let first = v
                .first_correct_at
                .map_or_else(String::new, |at| at.0.to_string());
            let ttf = v
                .time_to_first_correct_ns
                .map_or_else(String::new, |ns| ns.to_string());
            format!(
                "{run_id},{role},{seed},{id},{tier},{family},{critical},{ok},{wrong},{first},{ttf},{by_deadline},{missed},{cmiss},{esc},{informed},{correct}",
                role = record.role.as_str(),
                seed = record.seed,
                id = v.id,
                tier = tier_name(v.tier),
                critical = v.critical,
                ok = v.correct_declarations,
                wrong = v.wrong_declarations,
                by_deadline = v.correct_by_deadline,
                missed = v.missed,
                cmiss = v.critical_miss,
                esc = v.escalations,
                informed = v.informed_escalations,
                correct = v.correct_escalations,
            )
        })
        .collect()
}

/// The header of `notices.csv`.
pub const NOTICES_HEADER: &str = "run_id,arm_role,seed,noticer,notices,notices_on_background,notices_on_plain,notices_on_hard,notices_on_decoy,retirements,noticed_plain,noticed_hard,noticed_decoy,anchor_correct_plain,anchor_correct_hard,anchor_correct_decoy";

/// The header of `notice_incidents.csv`.
pub const NOTICE_INCIDENTS_HEADER: &str = "run_id,arm_role,seed,incident,tier,family,first_observation_at_ns,notices,noticed,first_notice_at_ns,notice_latency_ns,anchor_correct";

/// The header of `notice_events.csv`.
pub const NOTICE_EVENTS_HEADER: &str = "run_id,arm_role,seed,noticer,event,anomaly,anchor,site,anchor_at_ns,at_ns,incident,anchor_offset_ns,anchor_correct";

/// One line of `notices.csv` for `record`, without a trailing newline.
pub fn notices_row(run_id: &str, record: &SegmentRecord) -> String {
    let t = &record.notices.totals;
    format!(
        "{run_id},{role},{seed},{noticer},{notices},{bg},{plain},{hard},{decoy},{retired},{n_plain},{n_hard},{n_decoy},{a_plain},{a_hard},{a_decoy}",
        role = record.role.as_str(),
        seed = record.seed,
        noticer = record.noticer,
        notices = t.notices,
        bg = t.on_background,
        plain = t.on_plain,
        hard = t.on_hard,
        decoy = t.on_decoy,
        retired = t.retirements,
        n_plain = t.noticed.plain,
        n_hard = t.noticed.hard,
        n_decoy = t.noticed.decoy,
        a_plain = t.anchor_correct.plain,
        a_hard = t.anchor_correct.hard,
        a_decoy = t.anchor_correct.decoy,
    )
}

/// The lines of `notice_incidents.csv` for `record`: one per incident, in id order, each without
/// a trailing newline.
pub fn notice_incident_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    debug_assert_eq!(
        record.notices.per_incident.len(),
        record.incident_families.len()
    );
    let opt = |x: Option<u64>| x.map_or_else(String::new, |n| n.to_string());
    record
        .notices
        .per_incident
        .iter()
        .zip(&record.incident_families)
        .map(|(v, family)| {
            format!(
                "{run_id},{role},{seed},{id},{tier},{family},{first},{notices},{noticed},{at},{latency},{correct}",
                role = record.role.as_str(),
                seed = record.seed,
                id = v.id,
                tier = tier_name(v.tier),
                first = opt(v.first_observation_at.map(|t| t.0)),
                notices = v.notices,
                noticed = v.noticed,
                at = opt(v.first_notice_at.map(|t| t.0)),
                latency = opt(v.notice_latency_ns),
                correct = v.anchor_correct,
            )
        })
        .collect()
}

/// The lines of `notice_events.csv` for `record`: every notice and retirement the noticer
/// recorded, in order, each without a trailing newline. The public fields (`anchor`, `site`, the
/// instants) are what the noticer recorded; `incident`, `anchor_offset_ns` and `anchor_correct`
/// are the evaluator's reading of a notice (empty for a retirement and, but for `anchor_correct`,
/// for a notice anchored on background).
pub fn notice_event_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    let mut scores = record.notices.per_notice.iter();
    record
        .notice_log
        .iter()
        .map(|e| {
            let (incident, offset, correct) = match e.kind {
                NoticeKind::Notice => {
                    let s = scores
                        .next()
                        .expect("the evaluator scores every notice of the log");
                    (
                        s.incident.map_or_else(String::new, |i| i.to_string()),
                        s.anchor_offset_ns
                            .map_or_else(String::new, |n| n.to_string()),
                        s.anchor_correct.to_string(),
                    )
                }
                NoticeKind::Retire => (String::new(), String::new(), String::new()),
            };
            format!(
                "{run_id},{role},{seed},{noticer},{event},{anomaly},{anchor},{site},{anchor_at},{at},{incident},{offset},{correct}",
                role = record.role.as_str(),
                seed = record.seed,
                noticer = e.noticer,
                event = e.kind.as_str(),
                anomaly = e.anomaly,
                anchor = e.anchor.0,
                site = e.site.0,
                anchor_at = e.anchor_at.0,
                at = e.at.0,
            )
        })
        .collect()
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
