//! `score_selection` against a plain reference written from `RULES.md` (E1 to E8), on random records,
//! and the properties the rules imply (work item B4). The reference is deliberately unlike the
//! scorer: it walks every escalation against every notice with no index, and builds each count from
//! the per-notice answers; a slip in either shows as a disagreement.

mod common;

use common::{CompactIncident, hand_truth};
use gordian_core::Instant;
use gordian_stream::{HardKind, ObsId, StreamHypothesis, Tier};
use gordian_stream_eval::{
    EscalationEntry, IncidentClass, NoticeEntry, SelectionError, SelectionRetire, SelectionTrace,
    SelectionVerdict, score_selection,
};
use proptest::prelude::*;

const OBS: usize = 12;

fn hypothesis() -> StreamHypothesis {
    serde_json::from_str(r#"{"kind":{"Known":"ConfigDrift"},"site":3}"#).unwrap()
}

#[derive(Debug, Clone)]
struct Spec {
    /// Tier code and whether a hard incident is the slow leak, per incident.
    incidents: Vec<(u8, bool)>,
    /// The incident each observation belongs to, `OBS` of them (an index modulo the incidents, or
    /// background when it is `None`).
    labels: Vec<Option<usize>>,
    /// Notices: (anchor, at in seconds).
    notices: Vec<(usize, u64)>,
    /// For each notice (by position), `None` for not retired or (seconds after notice, by a rule).
    retire: Vec<Option<(u64, bool)>>,
    /// Escalations: (focus, at, tokens, modelled ns).
    escalations: Vec<(usize, u64, u64, u64)>,
}

fn spec() -> impl Strategy<Value = Spec> {
    (1usize..5)
        .prop_flat_map(|n_inc| {
            (
                proptest::collection::vec((0u8..3, any::<bool>()), n_inc),
                proptest::collection::vec(proptest::option::of(0usize..n_inc), OBS),
                proptest::collection::vec((0usize..OBS, 0u64..40), 0..9),
                proptest::collection::vec(proptest::option::of((0u64..30, any::<bool>())), 9),
                proptest::collection::vec((0usize..OBS, 0u64..80, 1u64..900, 1u64..9_000), 0..12),
            )
        })
        .prop_map(|(incidents, labels, notices, retire, escalations)| Spec {
            incidents,
            labels,
            notices,
            retire,
            escalations,
        })
}

fn build(spec: &Spec) -> (gordian_stream::oracle::StreamTruth, SelectionTrace) {
    let incidents: Vec<CompactIncident> = spec
        .incidents
        .iter()
        .map(|(code, leak)| {
            let tier = match code {
                0 => Tier::Plain,
                1 => Tier::Hard,
                _ => Tier::Decoy,
            };
            let decoy = tier == Tier::Decoy;
            CompactIncident {
                id: None,
                tier,
                critical: false,
                onset_ns: 1,
                deadline_ns: (!decoy).then_some(1_000_000_000_000),
                truth: (!decoy).then(hypothesis),
                occupies: Vec::new(),
                hard_kind: (tier == Tier::Hard).then_some(if *leak {
                    HardKind::SlowLeak
                } else {
                    HardKind::Compound
                }),
                recurrence_of: None,
                contradicts_early: None,
            }
        })
        .collect();
    let labels: Vec<Option<u32>> = spec.labels.iter().map(|l| l.map(|i| i as u32)).collect();
    let truth = hand_truth(2_000_000_000_000, &incidents, &labels);
    // Notices in time order, one anomaly each; a notice is retired at most once, after it.
    let mut notices: Vec<(usize, u64, usize)> = spec
        .notices
        .iter()
        .enumerate()
        .map(|(i, (a, t))| (*a, *t, i))
        .collect();
    notices.sort_by_key(|(_, t, i)| (*t, *i));
    let entries: Vec<NoticeEntry> = notices
        .iter()
        .enumerate()
        .map(|(k, (a, t, _))| NoticeEntry {
            anomaly: k as u32,
            anchor: ObsId(*a as u32),
            at: Instant(*t * 1_000_000_000),
            site: None,
        })
        .collect();
    let mut retirements: Vec<SelectionRetire> = notices
        .iter()
        .enumerate()
        .filter_map(|(k, (_, t, orig))| {
            spec.retire
                .get(*orig)
                .copied()
                .flatten()
                .map(|(after, by_rule)| SelectionRetire {
                    anomaly: k as u32,
                    at: Instant((*t + after) * 1_000_000_000),
                    followup: by_rule,
                })
        })
        .collect();
    retirements.sort_by_key(|r| (r.at, r.anomaly));
    let escalations = spec
        .escalations
        .iter()
        .map(|(f, t, tokens, ns)| EscalationEntry {
            at: Instant(*t * 1_000_000_000),
            focus: ObsId(*f as u32),
            tokens: *tokens,
            modelled_ns: *ns,
        })
        .collect();
    (
        truth,
        SelectionTrace {
            notices: entries,
            retirements,
            escalations,
        },
    )
}

fn class(spec: &Spec, obs: usize) -> IncidentClass {
    match spec.labels[obs] {
        None => IncidentClass::Background,
        Some(i) => match spec.incidents[i] {
            (0, _) => IncidentClass::Plain,
            (1, true) => IncidentClass::Leak,
            (1, false) => IncidentClass::Hard,
            _ => IncidentClass::Decoy,
        },
    }
}

/// The reference: E1 to E7 read literally, one escalation and one notice at a time.
fn reference(spec: &Spec, trace: &SelectionTrace) -> SelectionVerdict {
    let n = trace.notices.len();
    let retired_at = |k: usize| {
        trace
            .retirements
            .iter()
            .find(|r| r.anomaly == k as u32)
            .map(|r| (r.at, r.followup))
    };
    let mut escalations_about = vec![0u32; n];
    let mut first = vec![None::<Instant>; n];
    let mut verdict = SelectionVerdict {
        per_notice: Vec::new(),
        escalations: Default::default(),
        notices: Default::default(),
    };
    for e in &trace.escalations {
        let c = class(spec, e.focus.0 as usize);
        let bump = |t: &mut gordian_stream_eval::ByClass<u64>, v: u64| match c {
            IncidentClass::Background => t.background += v,
            IncidentClass::Plain => t.plain += v,
            IncidentClass::Hard => t.hard += v,
            IncidentClass::Leak => t.leak += v,
            IncidentClass::Decoy => t.decoy += v,
        };
        let mut calls = verdict.escalations.calls;
        let mut tokens = verdict.escalations.tokens;
        let mut ns = verdict.escalations.modelled_ns;
        match c {
            IncidentClass::Background => calls.background += 1,
            IncidentClass::Plain => calls.plain += 1,
            IncidentClass::Hard => calls.hard += 1,
            IncidentClass::Leak => calls.leak += 1,
            IncidentClass::Decoy => calls.decoy += 1,
        }
        bump(&mut tokens, e.tokens);
        bump(&mut ns, e.modelled_ns);
        verdict.escalations.calls = calls;
        verdict.escalations.tokens = tokens;
        verdict.escalations.modelled_ns = ns;
        // The latest notice at the focus, made by then, not retired before.
        let mut target = None;
        for k in 0..n {
            let made = trace.notices[k].anchor == e.focus && trace.notices[k].at <= e.at;
            let live = retired_at(k).is_none_or(|(at, _)| at >= e.at);
            if made && live {
                target = Some(k);
            }
        }
        match target {
            Some(k) => {
                escalations_about[k] += 1;
                first[k] = Some(first[k].map_or(e.at, |f| f.min(e.at)));
            }
            None => verdict.escalations.unattributed += 1,
        }
    }
    for k in 0..n {
        let anchor = trace.notices[k].anchor.0 as usize;
        let c = class(spec, anchor);
        let (r_at, by_rule) = match retired_at(k) {
            Some((at, f)) => (Some(at), f),
            None => (None, false),
        };
        let rbe = r_at.is_some() && escalations_about[k] == 0;
        verdict.per_notice.push(gordian_stream_eval::NoticeOutcome {
            anomaly: k as u32,
            incident: spec.labels[anchor].map(|i| i as u32),
            class: c,
            escalations: escalations_about[k],
            first_escalation_at: first[k],
            retired_at: r_at,
            followup: by_rule,
            retired_before_escalation: rbe,
        });
        let add = |t: &mut gordian_stream_eval::ByClass<u32>| match c {
            IncidentClass::Background => t.background += 1,
            IncidentClass::Plain => t.plain += 1,
            IncidentClass::Hard => t.hard += 1,
            IncidentClass::Leak => t.leak += 1,
            IncidentClass::Decoy => t.decoy += 1,
        };
        add(&mut verdict.notices.notices);
        if escalations_about[k] > 0 {
            add(&mut verdict.notices.escalated);
        }
        if rbe {
            add(&mut verdict.notices.retired_before_escalation);
        }
        if by_rule {
            add(&mut verdict.notices.followup_retired);
            if rbe {
                add(&mut verdict.notices.followup_before_escalation);
            }
        }
    }
    verdict
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn the_scorer_agrees_with_the_reference(s in spec()) {
        let (truth, trace) = build(&s);
        let got = score_selection(&truth, &trace).unwrap();
        prop_assert_eq!(got, reference(&s, &trace));
    }

    #[test]
    fn the_totals_add_up_the_way_the_rules_say(s in spec()) {
        let (truth, trace) = build(&s);
        let v = score_selection(&truth, &trace).unwrap();
        let e = &v.escalations;
        let all = |f: &dyn Fn(IncidentClass) -> u64| IncidentClass::ALL.iter().map(|c| f(*c)).sum::<u64>();
        // E2: every call is in one class, with its cost.
        prop_assert_eq!(all(&|c| u64::from(e.calls.get(c))), trace.escalations.len() as u64);
        prop_assert_eq!(all(&|c| e.tokens.get(c)), trace.escalations.iter().map(|x| x.tokens).sum::<u64>());
        prop_assert_eq!(all(&|c| e.modelled_ns.get(c)), trace.escalations.iter().map(|x| x.modelled_ns).sum::<u64>());
        // E1: the calls about a notice and the unattributed ones are all the calls.
        let about: u32 = v.per_notice.iter().map(|o| o.escalations).sum();
        prop_assert_eq!(u64::from(about + e.unattributed), trace.escalations.len() as u64);
        // E7: no notice is both escalated and retired before escalation; the follow-up counts are
        // inside the retired ones.
        for c in IncidentClass::ALL {
            let n = &v.notices;
            prop_assert!(n.escalated.get(c) + n.retired_before_escalation.get(c) <= n.notices.get(c));
            prop_assert!(n.followup_before_escalation.get(c) <= n.followup_retired.get(c));
            prop_assert!(n.followup_before_escalation.get(c) <= n.retired_before_escalation.get(c));
        }
        // E3: one outcome per notice, in order.
        prop_assert_eq!(v.per_notice.len(), trace.notices.len());
        for (o, n) in v.per_notice.iter().zip(&trace.notices) {
            prop_assert_eq!(o.anomaly, n.anomaly);
        }
    }

    #[test]
    fn dropping_the_escalations_changes_only_what_they_change(s in spec()) {
        let (truth, trace) = build(&s);
        let with = score_selection(&truth, &trace).unwrap();
        let mut none = trace.clone();
        none.escalations.clear();
        let without = score_selection(&truth, &none).unwrap();
        // The notices and their classes are the same; with no call nothing is escalated and every
        // retired notice was retired before escalation.
        for (a, b) in with.per_notice.iter().zip(&without.per_notice) {
            prop_assert_eq!((a.anomaly, a.class, a.retired_at, a.followup), (b.anomaly, b.class, b.retired_at, b.followup));
            prop_assert_eq!(b.escalations, 0);
            prop_assert_eq!(b.retired_before_escalation, b.retired_at.is_some());
        }
        prop_assert_eq!(without.escalations.unattributed, 0);
        prop_assert_eq!(without.notices.notices, with.notices.notices);
    }

    #[test]
    fn a_record_that_names_no_observation_is_refused(s in spec(), bad in OBS as u32..OBS as u32 + 5, which in 0usize..2) {
        let (truth, mut trace) = build(&s);
        if which == 0 {
            trace.notices.push(NoticeEntry { anomaly: 999, anchor: ObsId(bad), at: Instant(100_000_000_000), site: None });
            prop_assert_eq!(score_selection(&truth, &trace), Err(SelectionError::UnknownObservation { obs: ObsId(bad) }));
        } else {
            trace.escalations.push(EscalationEntry { at: Instant(1), focus: ObsId(bad), tokens: 1, modelled_ns: 1 });
            prop_assert_eq!(score_selection(&truth, &trace), Err(SelectionError::UnknownObservation { obs: ObsId(bad) }));
        }
    }
}
