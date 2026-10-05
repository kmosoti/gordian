//! The stream arms: that they differ only in when they escalate, what each rule does, the
//! privileged arm and the ablation, and the textual guards on where the truth may go.

mod stream_common;

use gordian_core::Instant;
use gordian_run::policy::decide::RULE;
use gordian_run::stream::arms::always::Always;
use gordian_run::stream::arms::change::Change;
use gordian_run::stream::arms::never::Never;
use gordian_run::stream::arms::periodic::Periodic;
use gordian_run::stream::arms::random::Random;
use gordian_run::stream::arms::rung::{AnomalyView, RungConfig};
use gordian_run::stream::arms::threshold::Threshold;
use gordian_run::stream::arms::{ArmRole, EscalationRule};
use gordian_run::stream::results::results_row;
use gordian_run::stream::spec::{StreamPolicySpec, build_public, privileged_factory};
use gordian_stream::{
    HardKind, ObsId, ObsRef, StreamAction, StreamKind, StreamOutcome, Tier, generate,
};
use gordian_stream_reveal::truth_of;
use gordian_world::ServiceId;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use stream_common::*;

fn view(id: u32) -> AnomalyView {
    AnomalyView {
        id,
        site: ServiceId(0),
        anchor: ObsId(id),
        anchor_at: Instant(0),
        noticed_at: Instant(1_000_000_000),
        score: 4.0,
        peak_score: 4.0,
        digest: 11,
        attempts: 0,
        pending: 0,
        answered: 0,
        last_attempt_at: None,
        last_attempt_digest: None,
        cheap_declared: false,
        delivered: 100,
    }
}

fn secs(s: u64) -> Instant {
    Instant(s * 1_000_000_000)
}

// ---- The rules

#[test]
fn never_escalates_nothing_and_always_escalates_each_anomaly_once() {
    let views = [view(1), view(2)];
    assert!(Never.targets(secs(5), &views).is_empty());
    let mut always = Always;
    assert_eq!(always.targets(secs(5), &views), vec![1, 2]);
    let mut tried = view(2);
    tried.attempts = 1;
    assert_eq!(always.targets(secs(6), &[view(1), tried]), vec![1]);
}

#[test]
fn periodic_reviews_every_live_anomaly_on_a_global_schedule() {
    let mut rule = Periodic::new(10_000_000_000);
    let views = [view(1), view(2)];
    assert!(
        rule.targets(secs(3), &views).is_empty(),
        "before the first tick"
    );
    assert_eq!(
        rule.targets(secs(10), &views),
        vec![1, 2],
        "at the first tick"
    );
    assert!(rule.targets(secs(12), &views).is_empty(), "between ticks");
    let mut busy = view(2);
    busy.pending = 1;
    assert_eq!(
        rule.targets(secs(21), &[view(1), busy]),
        vec![1],
        "a call in flight is not asked again"
    );
    // A step that overshoots several ticks reviews once.
    assert_eq!(rule.targets(secs(95), &views), vec![1, 2]);
    assert!(rule.targets(secs(96), &views).is_empty());
    assert_eq!(rule.targets(secs(100), &views), vec![1, 2]);
}

#[test]
fn change_triggered_escalates_only_when_the_evidence_digest_changed() {
    let mut rule = Change;
    let fresh = view(1);
    assert_eq!(
        rule.targets(secs(2), std::slice::from_ref(&fresh)),
        vec![1],
        "no call yet"
    );
    let mut asked = fresh.clone();
    asked.attempts = 1;
    asked.last_attempt_digest = Some(11);
    assert!(
        rule.targets(secs(3), std::slice::from_ref(&asked))
            .is_empty(),
        "unchanged"
    );
    asked.digest = 12;
    assert_eq!(
        rule.targets(secs(4), std::slice::from_ref(&asked)),
        vec![1],
        "changed"
    );
    asked.pending = 1;
    assert!(rule.targets(secs(5), &[asked]).is_empty(), "in flight");
}

#[test]
fn threshold_escalates_above_tau_inside_the_wait_and_never_after_it() {
    let mut rule = Threshold::new(3.5, 6_000_000_000);
    let mut v = view(1);
    v.score = 3.0;
    assert!(
        rule.targets(secs(2), &[v.clone()]).is_empty(),
        "below tau: wait and see"
    );
    v.score = 3.6;
    assert_eq!(
        rule.targets(secs(4), &[v.clone()]),
        vec![1],
        "above tau within the wait"
    );
    // The wait ended while the score was below tau: never escalated, even if it rises later.
    v.noticed_at = secs(1);
    v.score = 5.0;
    assert!(rule.targets(secs(8), &[v.clone()]).is_empty());
    v.noticed_at = secs(3);
    assert_eq!(rule.targets(secs(8), &[v.clone()]), vec![1]);
    v.attempts = 1;
    assert!(rule.targets(secs(9), &[v]).is_empty(), "once");
}

#[test]
fn random_draws_once_per_anomaly_whatever_happened_before() {
    let seed = [7u8; 32];
    // The same decisions however the anomalies are shown over time.
    let mut at_once = Random::new(0.5, seed);
    let all: Vec<u32> = at_once.targets(secs(1), &(0..40).map(view).collect::<Vec<_>>());
    let mut over_time = Random::new(0.5, seed);
    let mut chosen = Vec::new();
    for n in 1..=40u32 {
        let views: Vec<AnomalyView> = (0..n).map(view).collect();
        chosen.extend(over_time.targets(secs(u64::from(n)), &views));
    }
    assert_eq!(all, chosen);
    // An anomaly that is shown again is not drawn for again.
    assert!(
        at_once
            .targets(secs(2), &(0..40).map(view).collect::<Vec<_>>())
            .is_empty()
    );
    assert!(!all.is_empty() && all.len() < 40);
    // p = 0 and p = 1 are never and always.
    let views: Vec<AnomalyView> = (0..30).map(view).collect();
    assert!(Random::new(0.0, seed).targets(secs(1), &views).is_empty());
    assert_eq!(Random::new(1.0, seed).targets(secs(1), &views).len(), 30);
    // The frequency is p, to within sampling error (binomial: sd 22 over 5,000 at p = 0.3).
    let many: Vec<AnomalyView> = (0..5_000).map(view).collect();
    let hits = Random::new(0.3, [1u8; 32]).targets(secs(1), &many).len() as f64;
    assert!((hits - 1_500.0).abs() < 5.0 * 32.4, "{hits}");
    // Different seeds draw different streams.
    assert_ne!(
        Random::new(0.5, [1u8; 32]).targets(secs(1), &many),
        Random::new(0.5, [2u8; 32]).targets(secs(1), &many)
    );
}

#[test]
fn a_call_in_flight_holds_the_cheap_declaration_by_default() {
    let mut v = view(1);
    assert!(!Never.holds(&v));
    v.pending = 1;
    assert!(Never.holds(&v));
    assert!(Always.holds(&v));
}

// ---- One decision procedure

fn comparison_specs() -> Vec<(&'static str, StreamPolicySpec)> {
    ALL_ARMS
        .iter()
        .copied()
        .filter(|id| *id != "oracle_escalation")
        .map(|id| (id, StreamPolicySpec::from_id(id).unwrap()))
        .collect()
}

#[test]
fn every_public_arm_reports_the_one_decision_rule_and_its_role() {
    let p = params(1, 150);
    let public = public_of(&p);
    let rung = RungConfig::default();
    for (id, spec) in comparison_specs() {
        let arm = build_public(&spec, &rung, &public, id, 1).unwrap();
        assert_eq!(arm.decision_rule(), RULE, "{id}");
        assert_eq!(arm.id().0, id);
        let expected = if id == "ablation_hidden_rules" {
            ArmRole::Ablation
        } else {
            ArmRole::Comparison
        };
        assert_eq!(arm.role(), expected, "{id}");
    }
    // The privileged arm is not built from public information.
    let oracle = StreamPolicySpec::from_id("oracle_escalation").unwrap();
    assert!(build_public(&oracle, &rung, &public, "oracle_escalation_privileged", 1).is_none());
    assert!(privileged_factory(&oracle, &rung).is_some());
    assert!(privileged_factory(&StreamPolicySpec::Never, &rung).is_none());
}

fn run_row(p: &gordian_stream::StreamParams, spec: &StreamPolicySpec) -> String {
    let record = play(p, spec, &limits(p)).unwrap();
    results_row("x", &record)
}

#[test]
fn arms_that_escalate_alike_are_the_same_arm() {
    // EXP-101's intervention is when and what an arm escalates and nothing else, so arms whose
    // escalation rules agree must produce byte-identical rows. Never, a random arm with p = 0, a
    // periodic arm that never ticks and a threshold arm that is never reached are one arm;
    // always, a random arm with p = 1 and a threshold arm that is always reached are another.
    for seed in [2, 5] {
        let p = params(seed, 200);
        let never = run_row(&p, &StreamPolicySpec::Never);
        for other in [
            StreamPolicySpec::Random { p: 0.0 },
            StreamPolicySpec::Periodic {
                period_ns: 1_000_000_000_000,
            },
            StreamPolicySpec::Threshold {
                tau: 1e9,
                wait_ns: 6_000_000_000,
            },
        ] {
            assert_eq!(never, run_row(&p, &other), "{other:?} seed {seed}");
        }
        let always = run_row(&p, &StreamPolicySpec::Always);
        assert_ne!(never, always, "escalating changes the row");
        for other in [
            StreamPolicySpec::Random { p: 1.0 },
            StreamPolicySpec::Threshold {
                tau: -1e9,
                wait_ns: 6_000_000_000,
            },
        ] {
            assert_eq!(always, run_row(&p, &other), "{other:?} seed {seed}");
        }
    }
}

#[test]
fn escalating_changes_what_is_declared_and_never_what_is_noticed() {
    let p = params(5, 200);
    let l = limits(&p);
    let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    for spec in [
        StreamPolicySpec::Always,
        StreamPolicySpec::Periodic {
            period_ns: 10_000_000_000,
        },
        StreamPolicySpec::Change,
        StreamPolicySpec::Threshold {
            tau: 3.5,
            wait_ns: 6_000_000_000,
        },
        StreamPolicySpec::Random { p: 0.5 },
    ] {
        let record = play(&p, &spec, &l).unwrap();
        assert_eq!(
            record.anomalies_noticed, never.anomalies_noticed,
            "{spec:?} notices the same anomalies"
        );
        assert_eq!(record.observations, never.observations);
    }
}

#[test]
fn a_reasoner_answer_outranks_the_cheap_rung_and_is_declared_where_it_was_asked() {
    let p = params(5, 200);
    let record = play(&p, &StreamPolicySpec::Always, &limits(&p)).unwrap();
    assert!(record.verdict.reasoner_calls > 0);
    assert!(record.counts.reasoner_declarations > 0);
    // Every escalation names a focus, and an answer is declared on the observation it was about:
    // every reasoner declaration's anchor is the focus of some call.
    let foci: BTreeSet<ObsId> = record
        .trajectory
        .iter()
        .filter_map(|s| match &s.action {
            StreamAction::Escalate { question, .. } => {
                let gordian_stream::Question::Diagnose { focus } = question;
                Some(*focus)
            }
            _ => None,
        })
        .collect();
    let declared: BTreeSet<ObsId> = record
        .trajectory
        .iter()
        .filter_map(|s| match (&s.action, &s.outcome) {
            (StreamAction::Declare { anchor, .. }, StreamOutcome::Declared { .. }) => Some(*anchor),
            _ => None,
        })
        .collect();
    assert!(foci.iter().any(|f| declared.contains(f)));
}

// ---- The privileged arm

fn hard_heavy(seed: u64) -> gordian_stream::StreamParams {
    let mut p = params(seed, 300);
    p.mix.plain_permille = 300;
    p.mix.hard_permille = 500;
    p
}

#[test]
fn the_oracle_escalates_exactly_the_hard_incidents_once_each_with_their_decisive_evidence() {
    let mut escalated = 0;
    let mut hard_total = 0;
    for seed in 0..6 {
        let p = hard_heavy(seed);
        let truth = truth_of(&generate(&p));
        let record = play(&p, &StreamPolicySpec::Oracle, &limits(&p)).unwrap();
        let mut per_incident: BTreeMap<u32, Vec<BTreeSet<ObsRef>>> = BTreeMap::new();
        for step in &record.trajectory {
            if let StreamAction::Escalate { context, question } = &step.action {
                let gordian_stream::Question::Diagnose { focus } = question;
                let incident = truth
                    .incident_of(*focus)
                    .expect("the oracle asks about incidents");
                assert_eq!(truth.incidents[incident as usize].tier, Tier::Hard);
                per_incident
                    .entry(incident)
                    .or_default()
                    .push(context.iter().copied().collect());
            }
        }
        for inc in &truth.incidents {
            if inc.tier != Tier::Hard || inc.decisive.is_empty() {
                continue;
            }
            hard_total += 1;
            let asked = per_incident.get(&inc.id).map_or(0, Vec::len);
            assert_eq!(asked, 1, "seed {seed} incident {}", inc.id);
            let decisive: BTreeSet<ObsRef> =
                inc.decisive.iter().map(|o| ObsRef::Passive(*o)).collect();
            assert_eq!(
                per_incident[&inc.id][0], decisive,
                "all of it and nothing else"
            );
            escalated += 1;
        }
        // Nothing else was asked, and nothing was refused.
        assert_eq!(
            record.verdict.reasoner_calls as usize,
            per_incident.values().map(Vec::len).sum::<usize>()
        );
        assert_eq!(record.counts.escalations_refused, 0);
        assert_eq!(record.role, ArmRole::Privileged);
    }
    assert!(
        hard_total >= 8,
        "the test exercised {hard_total} hard incidents"
    );
    assert_eq!(escalated, hard_total);
}

#[test]
fn the_oracle_is_the_shared_cheap_rung_with_privileged_escalation_only() {
    // On a stream with no hard incident the oracle has nothing to escalate, and what is left is
    // the shared rung: its row equals the never arm's.
    let mut p = params(3, 200);
    p.mix.plain_permille = 1000;
    p.mix.hard_permille = 0;
    let l = limits(&p);
    let oracle = play(&p, &StreamPolicySpec::Oracle, &l).unwrap();
    let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
    assert_eq!(oracle.verdict.reasoner_calls, 0);
    // Strip the columns that name the arm: the role.
    let strip = |r: &gordian_run::stream::SegmentRecord| {
        results_row("x", r).replacen("privileged", "comparison", 1)
    };
    assert_eq!(strip(&oracle), results_row("x", &never));
}

// ---- The ablation

#[test]
fn the_ablation_never_escalates_and_knows_some_of_what_the_cheap_rung_does_not() {
    let (mut hard, mut never_right, mut ablation_right) = (0u32, 0u32, 0u32);
    let (mut plain, mut never_plain, mut ablation_plain) = (0u32, 0u32, 0u32);
    let mut per_family: BTreeMap<HardKind, [u32; 2]> = BTreeMap::new();
    for seed in 0..8 {
        let p = hard_heavy(seed);
        let truth = truth_of(&generate(&p));
        let l = limits(&p);
        let right = |record: &gordian_run::stream::SegmentRecord| -> BTreeSet<u32> {
            let mut ok = BTreeSet::new();
            for step in &record.trajectory {
                if let (StreamAction::Declare { anchor, diagnosis }, StreamOutcome::Declared { .. }) =
                    (&step.action, &step.outcome)
                    && let Some(i) = truth.incident_of(*anchor)
                {
                    let inc = &truth.incidents[i as usize];
                    if *diagnosis == inc.truth && inc.deadline_ns.is_none_or(|d| step.at.0 <= d) {
                        ok.insert(i);
                    }
                }
            }
            ok
        };
        let never = play(&p, &StreamPolicySpec::Never, &l).unwrap();
        let ablation = play(&p, &StreamPolicySpec::Ablation, &l).unwrap();
        assert_eq!(ablation.verdict.reasoner_calls, 0, "it escalates nothing");
        assert_eq!(ablation.role, ArmRole::Ablation);
        let (n, a) = (right(&never), right(&ablation));
        for inc in &truth.incidents {
            match inc.tier {
                Tier::Hard => {
                    hard += 1;
                    never_right += u32::from(n.contains(&inc.id));
                    ablation_right += u32::from(a.contains(&inc.id));
                    let f = per_family.entry(inc.shape.hard_kind.unwrap()).or_default();
                    f[0] += 1;
                    f[1] += u32::from(a.contains(&inc.id));
                }
                Tier::Plain => {
                    plain += 1;
                    never_plain += u32::from(n.contains(&inc.id));
                    ablation_plain += u32::from(a.contains(&inc.id));
                }
                Tier::Decoy => {}
            }
        }
        // A hard-kind declaration is made only by the ablation.
        let hard_kinds = |record: &gordian_run::stream::SegmentRecord| {
            record
                .trajectory
                .iter()
                .filter(|s| {
                    matches!(&s.action, StreamAction::Declare { diagnosis: Some(h), .. }
                        if matches!(h.kind, StreamKind::Hard(_)))
                })
                .count()
        };
        assert_eq!(hard_kinds(&never), 0, "the shared rule knows no hard kind");
        let _ = hard_kinds(&ablation);
    }
    println!("ablation per hard family [incidents, recognised right in time]: {per_family:?}");
    println!(
        "ablation: hard right in time {ablation_right}/{hard} (cheap rung {never_right}), plain {ablation_plain}/{plain} (cheap rung {never_plain})"
    );
    assert!(hard >= 30, "{hard} hard incidents");
    // The hidden rules are worth something: it recognises a real share of the hard incidents
    // that the cheap rung gets none of, and it does not wreck the plain ones.
    assert!(ablation_right * 100 >= hard * 15, "{ablation_right}/{hard}");
    assert!(ablation_right > never_right);
    assert!(
        ablation_plain * 100 >= never_plain * 92,
        "{ablation_plain} vs {never_plain}"
    );
}

// ---- The guards

fn arm_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stream/arms");
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    out.sort();
    out
}

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        let edge = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        edge(before) && edge(after)
    })
}

#[test]
fn no_arm_file_can_name_the_truth_the_call_records_the_stream_or_its_parameters() {
    // `scripts/check-no-oracle.sh` is the same check as a grep over the whole tree; this repeats
    // it so that `cargo test` fails without the script.
    let files = arm_files();
    assert!(files.len() >= 9);
    for (name, text) in &files {
        for word in [
            "StreamTruth",
            "CallSummary",
            "CallTrace",
            "IncidentTruth",
            "StreamVerdict",
            "StreamParams",
            "StreamSimulator",
            "Stream",
            "Truth",
            "Episode",
            "Simulator",
            "gordian_eval",
            "gordian_stream_reveal",
        ] {
            assert!(!has_word(text, word), "{name} names {word}");
        }
        for fragment in [
            ["oracle", "::"].concat().as_str(),
            "reveal",
            "truth_of",
            "call_records",
        ] {
            assert!(!text.contains(fragment), "{name} contains {fragment}");
        }
    }
}

fn stream_sources() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stream")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    out
}

#[test]
fn only_the_harness_the_scorer_seam_and_the_privileged_file_touch_the_truth() {
    let allowed = ["harness.rs", "score.rs", "oracle.rs"];
    let mut naming_truth = Vec::new();
    for (path, text) in stream_sources() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let touches = has_word(&text, "StreamTruth")
            || text.contains("gordian_stream_reveal")
            || text.contains("truth_of(")
            || text.contains("call_records(");
        if touches {
            naming_truth.push(name.clone());
            assert!(
                allowed.contains(&name.as_str()),
                "{name} reaches hidden state"
            );
        }
        // The stream is generated in the harness and nowhere else, and its truth is built there.
        if text.contains("generate(") || text.contains("truth_of(") {
            assert_eq!(
                name,
                "harness.rs",
                "{} builds a stream or its truth",
                path.display()
            );
        }
    }
    naming_truth.sort();
    // The harness and the scorer's seam hold it; the privileged arm is built from it.
    assert_eq!(naming_truth, vec!["harness.rs", "oracle.rs", "score.rs"]);
    // And the crate's own manifest does not switch the feature on (the reveal crate does).
    let cargo =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("reveal-hidden-state"));
}

#[test]
fn only_the_arm_wrapper_implements_the_policy_trait_and_no_rule_defines_a_decision() {
    let mut implementers = Vec::new();
    for (name, text) in arm_files() {
        if text.contains("impl<E: EscalationRule> StreamPolicy for")
            || text.contains("impl StreamPolicy for")
        {
            implementers.push(name.clone());
        }
        if name != "rung.rs" && name != "mod.rs" {
            for fragment in [
                "fn decide(",
                "fn decide_final(",
                "fn declared_final_cost(",
                "Decider",
            ] {
                assert!(!text.contains(fragment), "{name} contains {fragment}");
            }
        }
    }
    assert_eq!(implementers, vec!["mod.rs"]);
    // The privileged file builds its arm from the same wrapper.
    for (path, text) in stream_sources() {
        if path.ends_with("oracle.rs") {
            assert!(text.contains("StreamArm::with("));
            assert!(!text.contains("impl StreamPolicy for"));
        }
    }
}

#[test]
fn only_the_ablation_knows_the_hard_kinds_and_overrides_the_recogniser() {
    for (name, text) in arm_files() {
        let overrides = text.contains("fn recognize(&self, ctx");
        let hard_kinds = has_word(&text, "HardKind");
        match name.as_str() {
            "ablation.rs" => {
                assert!(overrides, "the ablation has recognisers");
                assert!(hard_kinds);
            }
            // The trait declares the default (`_ctx`), and the wrapper calls it.
            _ => {
                assert!(!overrides, "{name} overrides the recogniser");
                assert!(!hard_kinds, "{name} names the hard kinds");
            }
        }
    }
    // The privileged file does not override it either, and the harness never names it.
    for (path, text) in stream_sources() {
        if !path.ends_with("ablation.rs")
            && !path.starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stream/arms"))
        {
            assert!(!text.contains("fn recognize("), "{}", path.display());
        }
    }
}

#[test]
fn every_policy_spec_is_either_a_comparison_arm_the_privileged_arm_or_the_ablation() {
    let mut roles: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in ALL_ARMS {
        let spec = StreamPolicySpec::from_id(id).unwrap();
        roles.entry(spec.role().as_str()).or_default().push(id);
    }
    assert_eq!(roles["privileged"], vec!["oracle_escalation"]);
    assert_eq!(roles["ablation"], vec!["ablation_hidden_rules"]);
    assert_eq!(roles["comparison"].len(), 6);
}
