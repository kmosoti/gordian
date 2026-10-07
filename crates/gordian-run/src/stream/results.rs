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
//! ops_component, ops_sched, modelled_component_ns, modelled_sched_ns, substrate_ns, total_cost_ns,
//! recall_declarations, noticer_ns
//! ```
//!
//! The last two columns are work item E1's, appended at the end so that every column before them is
//! byte for byte what it was (R6's held-out replay is compared on those columns). `recall_declarations`
//! counts the declarations whose source is a noticer's memory (`Source::Recall`); they are also
//! counted in `cheap_declarations`, as they were before the column existed. `noticer_ns` is what the
//! arm's noticer charged to the bill for its own counted work, modelled nanoseconds (the medium's
//! operations at their declared prices; zero for a noticer whose work is bookkeeping, which is every
//! noticer but the medium's). It is **not** in `total_cost_ns`, which has never held it: the full
//! modelled cost of an arm is `total_cost_ns + noticer_ns`. A table lookup or a bind of the
//! record rung (`arms/noticer_record.rs`) is bookkeeping and is not priced.
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
//!
//! # The notice files (work item B1)
//!
//! Three more files per arm, beside the three above, which they leave byte for byte as they were
//! (`NOTICES_HEADER`, `NOTICE_INCIDENTS_HEADER` and `NOTICE_EVENTS_HEADER`; the measures are the
//! evaluator's, rules N1 to N12 of its `RULES.md`):
//!
//! - `notices.csv`, one row per stream: `noticer` (the id of the noticer the arm's rung used),
//!   `notices`, the notices by what the anchor belongs to (`notices_on_background`, `_on_plain`,
//!   `_on_hard`, `_on_decoy`), `retirements`, and the incidents `noticed_*` and `anchor_correct_*`
//!   by tier. Counts, never ratios.
//! - `notice_incidents.csv`, one row per incident in id order: `first_observation_at_ns`,
//!   `notices` about it (anchor belongs to it), `noticed`, `first_notice_at_ns`,
//!   `notice_latency_ns` (from the first observation) and `anchor_correct`. The three instants are
//!   empty when there is nothing to report.
//! - `notice_events.csv`, one row per notice and retirement in the order recorded: `event`,
//!   `anomaly`, `anchor`, `site`, `anchor_at_ns`, `at_ns`, and the evaluator's reading of a notice
//!   (`incident` the anchor belongs to, `anchor_offset_ns`, `anchor_correct`); empty for a
//!   retirement, and `incident` and `anchor_offset_ns` empty for a notice on background.
//!
//! Work item B2 adds columns at the end of each (the site check and the correct notices of
//! the evaluator's N13 to N16), so that a loader written for the B1 files fails its schema guard
//! rather than reading them silently: `notices.csv` gains `notices_site_correct` and
//! `notices_anchor_site_correct` (counts of notices), and `site_correct_*` and
//! `anchor_site_correct_*` by tier (counts of incidents); `notice_incidents.csv` gains
//! `site_correct` and `anchor_site_correct`; `notice_events.csv` gains the same two for each notice
//! (empty for a retirement, `false` for a notice anchored on background).
//!
//! `tier`, `family` and the evaluator's readings are hidden-side facts, as in `incidents.csv`:
//! evaluator output for the analysis, never a policy input. All three files are deterministic.
//!
//! # The memory files (work item E1)
//!
//! Three more files per arm (`MEMORY_HEADER`, `MEMORY_INCIDENTS_HEADER` and `RECALLS_HEADER`; the
//! measures are the evaluator's, rules K1 to K10 of its `RULES.md`), which leave every other file
//! byte for byte as it was but for the two appended columns of `results.csv`:
//!
//! - `memory.csv`, one row per stream: the declarations made from memory in six cells (correct or
//!   wrong, crossed with whether the answer the memory stored was right, wrong or unrecorded), by
//!   what the anchor belongs to, and the incidents unasked correct, unasked wrong and stale wrong
//!   by tier, and the hard recurrences, same-family-elsewhere and reachable incidents with how many
//!   were unasked correct. Counts, never ratios.
//! - `memory_incidents.csv`, one row per incident in id order: `recurrence_of`,
//!   `same_family_earlier`, the declarations and escalations about it, the three flags, and its
//!   recalls in the six cells.
//! - `recalls.csv`, one row per declaration made from memory: its instant, anchor and diagnosis,
//!   the observation the memory was bound at and the answer it stored (empty when the memory does not
//!   say), and the evaluator's reading (the incident and tier of the anchor, whether it was correct,
//!   whether the source was right, wrong or unknown, and the source's incident).

use super::arms::noticer::NoticeKind;
use super::harness::SegmentRecord;
use gordian_core::Resource;
use gordian_stream::Tier;
use std::fmt::Write as _;

/// The header of `results.csv`.
pub const RESULTS_HEADER: &str = "run_id,arm_role,seed,duration_ns,observations,anomalies_noticed,probes_used,declarations,declared_incident,declared_dismissal,incidents_plain,incidents_hard,incidents_decoy,critical_incidents,correct_plain,correct_hard,missed_plain,missed_hard,critical_missed_plain,critical_missed_hard,wrong_declarations,decoys_dismissed,decoys_alarmed,decoys_silent,false_alarms,false_alarms_on_background,escalations_needed,escalations_unneeded,escalations_background,hard_incidents_escalated,other_incidents_escalated,calls_informed,calls_correct,reasoner_calls,reasoner_refs,reasoner_tokens,reasoner_modelled_ns,reasoner_latency_ns,cheap_declarations,reasoner_declarations,escalations_refused,probes_refused,calls_unanswered,reasoner_cost_ns,bill_compute,bill_probes,bill_time,bill_comm,components_run,components_skipped,rule_skipped,steps,stop_reason,ops_component,ops_sched,modelled_component_ns,modelled_sched_ns,substrate_ns,total_cost_ns,recall_declarations,noticer_ns";

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
        "{run_id},{role},{seed},{duration},{observations},{noticed},{probes},{decl},{decl_inc},{decl_dis},{inc_plain},{inc_hard},{inc_decoy},{critical},{ok_plain},{ok_hard},{miss_plain},{miss_hard},{cmiss_plain},{cmiss_hard},{wrong},{dismissed},{alarmed},{silent},{false_alarms},{false_bg},{esc_needed},{esc_unneeded},{esc_bg},{hard_esc},{other_esc},{informed},{correct},{r_calls},{r_refs},{r_tokens},{r_modelled},{r_latency},{cheap},{reasoner},{esc_refused},{probe_refused},{unanswered},{reasoner_cost},{bc},{bp},{bt},{bk},{run},{skipped},{rule_skipped},{steps},{stop},{ops_c},{ops_s},{mod_c},{mod_s},{substrate},{total},{recall_decl},{noticer_ns}",
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
        recall_decl = record.counts.recall_declarations,
        noticer_ns = record.noticer_ns,
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
pub const NOTICES_HEADER: &str = "run_id,arm_role,seed,noticer,notices,notices_on_background,notices_on_plain,notices_on_hard,notices_on_decoy,retirements,noticed_plain,noticed_hard,noticed_decoy,anchor_correct_plain,anchor_correct_hard,anchor_correct_decoy,notices_site_correct,notices_anchor_site_correct,site_correct_plain,site_correct_hard,site_correct_decoy,anchor_site_correct_plain,anchor_site_correct_hard,anchor_site_correct_decoy";

/// The header of `notice_incidents.csv`.
pub const NOTICE_INCIDENTS_HEADER: &str = "run_id,arm_role,seed,incident,tier,family,first_observation_at_ns,notices,noticed,first_notice_at_ns,notice_latency_ns,anchor_correct,site_correct,anchor_site_correct";

/// The header of `notice_events.csv`.
pub const NOTICE_EVENTS_HEADER: &str = "run_id,arm_role,seed,noticer,event,anomaly,anchor,site,anchor_at_ns,at_ns,incident,anchor_offset_ns,anchor_correct,site_correct,anchor_site_correct";

/// One line of `notices.csv` for `record`, without a trailing newline.
pub fn notices_row(run_id: &str, record: &SegmentRecord) -> String {
    let t = &record.notices.totals;
    format!(
        "{run_id},{role},{seed},{noticer},{notices},{bg},{plain},{hard},{decoy},{retired},{n_plain},{n_hard},{n_decoy},{a_plain},{a_hard},{a_decoy},{s_notices},{c_notices},{s_plain},{s_hard},{s_decoy},{c_plain},{c_hard},{c_decoy}",
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
        s_notices = t.notices_site_correct,
        c_notices = t.notices_anchor_site_correct,
        s_plain = t.site_correct.plain,
        s_hard = t.site_correct.hard,
        s_decoy = t.site_correct.decoy,
        c_plain = t.anchor_site_correct.plain,
        c_hard = t.anchor_site_correct.hard,
        c_decoy = t.anchor_site_correct.decoy,
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
                "{run_id},{role},{seed},{id},{tier},{family},{first},{notices},{noticed},{at},{latency},{correct},{site_correct},{both}",
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
                site_correct = v.site_correct,
                both = v.anchor_site_correct,
            )
        })
        .collect()
}

/// The lines of `notice_events.csv` for `record`: every notice and retirement the noticer
/// recorded, in order, each without a trailing newline. The public fields (`anchor`, `site`, the
/// instants) are what the noticer recorded; `incident`, `anchor_offset_ns`, `anchor_correct`,
/// `site_correct` and `anchor_site_correct` are the evaluator's reading of a notice (empty for a
/// retirement and, but for the three flags, for a notice anchored on background).
pub fn notice_event_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    let mut scores = record.notices.per_notice.iter();
    record
        .notice_log
        .iter()
        .map(|e| {
            let (incident, offset, correct, site_correct, both) = match e.kind {
                NoticeKind::Notice => {
                    let s = scores
                        .next()
                        .expect("the evaluator scores every notice of the log");
                    (
                        s.incident.map_or_else(String::new, |i| i.to_string()),
                        s.anchor_offset_ns
                            .map_or_else(String::new, |n| n.to_string()),
                        s.anchor_correct.to_string(),
                        s.site_correct.to_string(),
                        s.anchor_site_correct.to_string(),
                    )
                }
                NoticeKind::Retire => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            };
            format!(
                "{run_id},{role},{seed},{noticer},{event},{anomaly},{anchor},{site},{anchor_at},{at},{incident},{offset},{correct},{site_correct},{both}",
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

/// The header of `selection.csv` (work item B4): for each class of what a call's focus or a notice's
/// anchor belongs to (`background`, `plain`, `hard`, `leak`, `decoy`; the evaluator's `RULES.md`, E2),
/// the accepted escalations (`calls_*`) and the tokens and modelled nanoseconds they declared
/// (E2), and the notices (`notices_*`), those escalated (`escalated_*`), retired before escalation
/// (`retired_before_escalation_*`, E4), retired by a follow-up rule (`followup_retired_*`, E5) and
/// retired by one before escalation (`followup_before_escalation_*`); then the escalations about no
/// notice (E1). Counts, never ratios.
pub const SELECTION_HEADER: &str = "run_id,arm_role,seed,noticer,calls_background,calls_plain,calls_hard,calls_leak,calls_decoy,tokens_background,tokens_plain,tokens_hard,tokens_leak,tokens_decoy,modelled_ns_background,modelled_ns_plain,modelled_ns_hard,modelled_ns_leak,modelled_ns_decoy,notices_background,notices_plain,notices_hard,notices_leak,notices_decoy,escalated_background,escalated_plain,escalated_hard,escalated_leak,escalated_decoy,retired_before_escalation_background,retired_before_escalation_plain,retired_before_escalation_hard,retired_before_escalation_leak,retired_before_escalation_decoy,followup_retired_background,followup_retired_plain,followup_retired_hard,followup_retired_leak,followup_retired_decoy,followup_before_escalation_background,followup_before_escalation_plain,followup_before_escalation_hard,followup_before_escalation_leak,followup_before_escalation_decoy,escalations_unattributed";

/// The header of `selection_notices.csv` (work item B4): one row per notice, in the order recorded.
pub const SELECTION_NOTICES_HEADER: &str = "run_id,arm_role,seed,noticer,anomaly,incident,class,escalations,first_escalation_at_ns,retired_at_ns,retire_cause,retired_before_escalation";

/// One line of `selection.csv` for `record`, without a trailing newline.
pub fn selection_row(run_id: &str, record: &SegmentRecord) -> String {
    let s = &record.selection;
    format!(
        "{run_id},{role},{seed},{noticer},{calls_background},{calls_plain},{calls_hard},{calls_leak},{calls_decoy},{tokens_background},{tokens_plain},{tokens_hard},{tokens_leak},{tokens_decoy},{modelled_ns_background},{modelled_ns_plain},{modelled_ns_hard},{modelled_ns_leak},{modelled_ns_decoy},{notices_background},{notices_plain},{notices_hard},{notices_leak},{notices_decoy},{escalated_background},{escalated_plain},{escalated_hard},{escalated_leak},{escalated_decoy},{rbe_background},{rbe_plain},{rbe_hard},{rbe_leak},{rbe_decoy},{fr_background},{fr_plain},{fr_hard},{fr_leak},{fr_decoy},{fbe_background},{fbe_plain},{fbe_hard},{fbe_leak},{fbe_decoy},{unattributed}",
        role = record.role.as_str(),
        seed = record.seed,
        noticer = record.noticer,
        calls_background = s.escalations.calls.background,
        calls_plain = s.escalations.calls.plain,
        calls_hard = s.escalations.calls.hard,
        calls_leak = s.escalations.calls.leak,
        calls_decoy = s.escalations.calls.decoy,
        tokens_background = s.escalations.tokens.background,
        tokens_plain = s.escalations.tokens.plain,
        tokens_hard = s.escalations.tokens.hard,
        tokens_leak = s.escalations.tokens.leak,
        tokens_decoy = s.escalations.tokens.decoy,
        modelled_ns_background = s.escalations.modelled_ns.background,
        modelled_ns_plain = s.escalations.modelled_ns.plain,
        modelled_ns_hard = s.escalations.modelled_ns.hard,
        modelled_ns_leak = s.escalations.modelled_ns.leak,
        modelled_ns_decoy = s.escalations.modelled_ns.decoy,
        notices_background = s.notices.notices.background,
        notices_plain = s.notices.notices.plain,
        notices_hard = s.notices.notices.hard,
        notices_leak = s.notices.notices.leak,
        notices_decoy = s.notices.notices.decoy,
        escalated_background = s.notices.escalated.background,
        escalated_plain = s.notices.escalated.plain,
        escalated_hard = s.notices.escalated.hard,
        escalated_leak = s.notices.escalated.leak,
        escalated_decoy = s.notices.escalated.decoy,
        rbe_background = s.notices.retired_before_escalation.background,
        rbe_plain = s.notices.retired_before_escalation.plain,
        rbe_hard = s.notices.retired_before_escalation.hard,
        rbe_leak = s.notices.retired_before_escalation.leak,
        rbe_decoy = s.notices.retired_before_escalation.decoy,
        fr_background = s.notices.followup_retired.background,
        fr_plain = s.notices.followup_retired.plain,
        fr_hard = s.notices.followup_retired.hard,
        fr_leak = s.notices.followup_retired.leak,
        fr_decoy = s.notices.followup_retired.decoy,
        fbe_background = s.notices.followup_before_escalation.background,
        fbe_plain = s.notices.followup_before_escalation.plain,
        fbe_hard = s.notices.followup_before_escalation.hard,
        fbe_leak = s.notices.followup_before_escalation.leak,
        fbe_decoy = s.notices.followup_before_escalation.decoy,
        unattributed = s.escalations.unattributed,
    )
}

/// The lines of `selection_notices.csv` for `record`: one per notice, in the order recorded, each
/// without a trailing newline. `incident` is empty for a notice anchored on background;
/// `first_escalation_at_ns` is empty when no escalation is about the notice and `retired_at_ns` and
/// `retire_cause` (`quiet` or `followup`) when it was not retired by the end of the stream.
pub fn selection_notice_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    let opt = |x: Option<u64>| x.map_or_else(String::new, |n| n.to_string());
    record
        .selection
        .per_notice
        .iter()
        .map(|o| {
            format!(
                "{run_id},{role},{seed},{noticer},{anomaly},{incident},{class},{esc},{first},{retired},{cause},{rbe}",
                role = record.role.as_str(),
                seed = record.seed,
                noticer = record.noticer,
                anomaly = o.anomaly,
                incident = o.incident.map_or_else(String::new, |i| i.to_string()),
                class = o.class.as_str(),
                esc = o.escalations,
                first = opt(o.first_escalation_at.map(|t| t.0)),
                retired = opt(o.retired_at.map(|t| t.0)),
                cause = match (o.retired_at, o.followup) {
                    (None, _) => "",
                    (Some(_), true) => "followup",
                    (Some(_), false) => "quiet",
                },
                rbe = o.retired_before_escalation,
            )
        })
        .collect()
}

/// The header of `memory.csv` (work item E1): per stream, the declarations made from memory in the
/// evaluator's six cells (`recalls_correct_source_right` and so on: outcome crossed with whether the
/// stored answer was right, wrong or unrecorded; `RULES.md` K5, K6), by what the anchor belongs to,
/// and the incidents with an unasked correct declaration, an unasked wrong one and a wrong recall
/// with no escalation, by tier (K3, K4, K6), and the three populations of K7. Counts, never ratios.
pub const MEMORY_HEADER: &str = "run_id,arm_role,seed,noticer,recalls,recalls_correct_source_right,recalls_correct_source_wrong,recalls_correct_source_unknown,recalls_wrong_source_right,recalls_wrong_source_wrong,recalls_wrong_source_unknown,recalls_plain_correct,recalls_plain_wrong,recalls_hard_correct,recalls_hard_wrong,recalls_decoy_correct,recalls_decoy_wrong,recalls_background_correct,recalls_background_wrong,unasked_correct_plain,unasked_correct_hard,unasked_correct_decoy,unasked_wrong_plain,unasked_wrong_hard,unasked_wrong_decoy,stale_wrong_plain,stale_wrong_hard,stale_wrong_decoy,hard_recurrences,hard_recurrences_unasked_correct,hard_elsewhere,hard_elsewhere_unasked_correct,hard_reachable,hard_reachable_unasked_correct";

/// The header of `memory_incidents.csv` (work item E1): one row per incident, in id order, with the
/// evaluator's reading (`RULES.md` K1 to K6) and the incident's recalls in the six cells.
pub const MEMORY_INCIDENTS_HEADER: &str = "run_id,arm_role,seed,incident,tier,family,recurrence_of,same_family_earlier,correct_declarations,wrong_declarations,escalations,unasked_correct,unasked_wrong,stale_wrong,recalls,recalls_correct_source_right,recalls_correct_source_wrong,recalls_correct_source_unknown,recalls_wrong_source_right,recalls_wrong_source_wrong,recalls_wrong_source_unknown";

/// The header of `recalls.csv` (work item E1): one row per declaration made from memory, in the
/// order made. `at_ns`, `anchor`, `declared`, `source_obs` and `stored` are the arm's public record
/// (a diagnosis is `none` or `kind@site`; `source_obs` and `stored` are empty when the memory does
/// not say what it was bound at); `incident`, `tier`, `correct`, `source_class` and
/// `source_incident` are the evaluator's reading (K5): `incident` and `tier` are empty for an anchor
/// on background, `source_incident` for a source on background, no source or a source in an
/// earlier stream. `source_age` is how many segments before the recall the memory was bound (0: the
/// same stream) and `source_seed` the seed of that stream, read from the arm's own seeds in the
/// order it played them; `source_obs` is an observation number in that stream. Both are empty when
/// the memory says nothing of its source.
pub const RECALLS_HEADER: &str = "run_id,arm_role,seed,step,at_ns,anchor,declared,source_obs,stored,incident,tier,correct,source_class,source_incident,source_age,source_seed";

/// A diagnosis as `none` or `kind@site`, without a comma.
fn diagnosis_text(d: &gordian_stream::Diagnosis) -> String {
    d.map_or_else(
        || "none".to_owned(),
        |h| format!("{:?}@{}", h.kind, h.site.0),
    )
}

/// One line of `memory.csv` for `record`, without a trailing newline.
pub fn memory_row(run_id: &str, record: &SegmentRecord) -> String {
    let t = &record.memory.totals;
    let r = &t.recalls;
    let c = |cells: &super::score::RecallCells| (cells.correct(), cells.wrong());
    let (pc, pw) = c(&t.recalls_on_plain);
    let (hc, hw) = c(&t.recalls_on_hard);
    let (dc, dw) = c(&t.recalls_on_decoy);
    let (bc, bw) = c(&t.recalls_on_background);
    format!(
        "{run_id},{role},{seed},{noticer},{total},{csr},{csw},{csu},{wsr},{wsw},{wsu},{pc},{pw},{hc},{hw},{dc},{dw},{bc},{bw},{uc_p},{uc_h},{uc_d},{uw_p},{uw_h},{uw_d},{sw_p},{sw_h},{sw_d},{hr},{hru},{he},{heu},{hx},{hxu}",
        role = record.role.as_str(),
        seed = record.seed,
        noticer = record.noticer,
        total = r.total(),
        csr = r.correct_source_right,
        csw = r.correct_source_wrong,
        csu = r.correct_source_unknown,
        wsr = r.wrong_source_right,
        wsw = r.wrong_source_wrong,
        wsu = r.wrong_source_unknown,
        uc_p = t.unasked_correct.plain,
        uc_h = t.unasked_correct.hard,
        uc_d = t.unasked_correct.decoy,
        uw_p = t.unasked_wrong.plain,
        uw_h = t.unasked_wrong.hard,
        uw_d = t.unasked_wrong.decoy,
        sw_p = t.stale_wrong.plain,
        sw_h = t.stale_wrong.hard,
        sw_d = t.stale_wrong.decoy,
        hr = t.hard_recurrences,
        hru = t.hard_recurrences_unasked_correct,
        he = t.hard_elsewhere,
        heu = t.hard_elsewhere_unasked_correct,
        hx = t.hard_reachable,
        hxu = t.hard_reachable_unasked_correct,
    )
}

/// The lines of `memory_incidents.csv` for `record`: one per incident, in id order, each without a
/// trailing newline.
pub fn memory_incident_rows(run_id: &str, record: &SegmentRecord) -> Vec<String> {
    debug_assert_eq!(
        record.memory.per_incident.len(),
        record.incident_families.len()
    );
    record
        .memory
        .per_incident
        .iter()
        .zip(&record.incident_families)
        .map(|(v, family)| {
            format!(
                "{run_id},{role},{seed},{id},{tier},{family},{recurrence},{elsewhere},{ok},{wrong},{esc},{uc},{uw},{sw},{recalls},{csr},{csw},{csu},{wsr},{wsw},{wsu}",
                role = record.role.as_str(),
                seed = record.seed,
                id = v.id,
                tier = tier_name(v.tier),
                recurrence = v.recurrence_of.map_or_else(String::new, |i| i.to_string()),
                elsewhere = v.same_family_earlier,
                ok = v.correct_declarations,
                wrong = v.wrong_declarations,
                esc = v.escalations,
                uc = v.unasked_correct,
                uw = v.unasked_wrong,
                sw = v.stale_wrong,
                recalls = v.recalls.total(),
                csr = v.recalls.correct_source_right,
                csw = v.recalls.correct_source_wrong,
                csu = v.recalls.correct_source_unknown,
                wsr = v.recalls.wrong_source_right,
                wsw = v.recalls.wrong_source_wrong,
                wsu = v.recalls.wrong_source_unknown,
            )
        })
        .collect()
}

/// The lines of `recalls.csv` for `record`: one per declaration made from memory, in the order
/// made, each without a trailing newline.
pub fn recall_rows(run_id: &str, record: &SegmentRecord, earlier_seeds: &[u64]) -> Vec<String> {
    debug_assert_eq!(record.memory.per_recall.len(), record.recalls.len());
    record
        .memory
        .per_recall
        .iter()
        .zip(&record.recalls)
        .zip(&record.recall_ages)
        .map(|((score, entry), age)| {
            let step = &record.trajectory[entry.step];
            let (at, anchor, declared) = match &step.action {
                gordian_stream::StreamAction::Declare { anchor, diagnosis } => {
                    (step.at.0, anchor.0, diagnosis_text(diagnosis))
                }
                _ => (step.at.0, 0, String::new()),
            };
            let (source_obs, stored) = entry.source.map_or_else(
                || (String::new(), String::new()),
                |s| (s.obs.0.to_string(), diagnosis_text(&s.diagnosis)),
            );
            let (source_age, source_seed) = match entry.source {
                None => (String::new(), String::new()),
                Some(_) if *age == 0 => ("0".to_owned(), record.seed.to_string()),
                Some(_) => (
                    age.to_string(),
                    earlier_seeds
                        .len()
                        .checked_sub(*age as usize)
                        .and_then(|i| earlier_seeds.get(i))
                        .map_or_else(String::new, |s| s.to_string()),
                ),
            };
            format!(
                "{run_id},{role},{seed},{step},{at},{anchor},{declared},{source_obs},{stored},{incident},{tier},{correct},{class},{source_incident},{source_age},{source_seed}",
                role = record.role.as_str(),
                seed = record.seed,
                step = score.step,
                incident = score.incident.map_or_else(String::new, |i| i.to_string()),
                tier = score.tier.map_or("", tier_name),
                correct = score.correct,
                class = match score.source {
                    super::score::SourceClass::Right => "right",
                    super::score::SourceClass::Wrong => "wrong",
                    super::score::SourceClass::Unknown => "unknown",
                },
                source_incident = score
                    .source_incident
                    .map_or_else(String::new, |i| i.to_string()),
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
