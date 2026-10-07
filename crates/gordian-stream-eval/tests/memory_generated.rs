//! `score_memory` against streams the generator makes, with `truth_from_stream` as the only
//! bridge (work item E1).
//!
//! The expectations are stated from the rules, not read back from the code under test: a
//! trajectory that declares every incident's truth early, every declaration marked as a recall
//! bound at the incident's own first observation, has every recall correct with a right source and
//! every plain, hard and decoy incident unasked correct; one that declares a wrong site has every
//! recall wrong with a right source (a collision). The populations (K1, K2) are checked against a
//! second, set-based reading of the truth, and the two numbers W2 measured from the hidden side
//! (83 hard recurrences and 51 same-family-elsewhere incidents on seeds 40000-40199) are checked by
//! an ignored test that runs the 200 streams.

use gordian_core::Instant;
use gordian_stream::oracle::{IncidentTruth, StreamTruth};
use gordian_stream::{
    Diagnosis, HardKind, StreamAction, StreamHypothesis, StreamKind, StreamOutcome, StreamParams,
    Tier, generate,
};
use gordian_stream_eval::{RecallEntry, RecallSource, StreamStep, score_memory, truth_from_stream};
use gordian_world::ServiceId;

const SEEDS: u64 = 40;

fn truths(seeds: impl Iterator<Item = u64>) -> Vec<(u64, StreamTruth)> {
    seeds
        .map(|seed| (seed, truth_from_stream(&generate(&StreamParams::new(seed)))))
        .collect()
}

/// One declaration per incident, each at the onset, saying `say(incident)`, each marked as a
/// recall bound at the incident's own first observation with its own truth as the stored answer.
fn declare_each(
    truth: &StreamTruth,
    say: impl Fn(&IncidentTruth) -> Diagnosis,
) -> (Vec<StreamStep>, Vec<RecallEntry>) {
    let mut steps = Vec::new();
    let mut recalls = Vec::new();
    for (k, inc) in truth.incidents.iter().enumerate() {
        let obs = *inc
            .observations
            .first()
            .expect("an incident has an observation");
        steps.push(StreamStep {
            at: Instant(inc.onset_ns),
            action: StreamAction::Declare {
                anchor: obs,
                diagnosis: say(inc),
            },
            outcome: StreamOutcome::Declared { index: k as u32 },
        });
        recalls.push(RecallEntry {
            step: k,
            source: Some(RecallSource {
                obs,
                diagnosis: inc.truth,
            }),
        });
    }
    (steps, recalls)
}

fn wrong_site(inc: &IncidentTruth) -> Diagnosis {
    let kind = StreamKind::Hard(HardKind::Compound);
    Some(StreamHypothesis {
        kind,
        site: ServiceId(inc.truth.map_or(0, |h| h.site.0) + 100),
    })
}

#[test]
fn declaring_every_truth_as_a_recall_is_unasked_correct_everywhere() {
    let mut incidents = 0u32;
    for (seed, truth) in truths(0..SEEDS) {
        let (steps, recalls) = declare_each(&truth, |inc| inc.truth);
        let v = score_memory(&truth, &steps, &recalls).expect("scores");
        let t = &v.totals;
        let n = truth.incidents.len() as u32;
        incidents += n;
        assert_eq!(t.recalls.correct_source_right, n, "seed {seed}");
        assert_eq!(t.recalls.total(), n, "seed {seed}");
        assert_eq!(t.recalls.wrong(), 0, "seed {seed}");
        assert_eq!(
            t.stale_wrong.plain + t.stale_wrong.hard + t.stale_wrong.decoy,
            0
        );
        assert_eq!(
            t.unasked_wrong.plain + t.unasked_wrong.hard + t.unasked_wrong.decoy,
            0
        );
        for (inc, row) in truth.incidents.iter().zip(&v.per_incident) {
            assert!(row.unasked_correct, "seed {seed} incident {}", inc.id);
            assert_eq!(row.recurrence_of, inc.recurrence_of);
        }
        // Every hard recurrence and every elsewhere incident is unasked correct here.
        assert_eq!(t.hard_recurrences_unasked_correct, t.hard_recurrences);
        assert_eq!(t.hard_elsewhere_unasked_correct, t.hard_elsewhere);
        assert_eq!(t.hard_reachable_unasked_correct, t.hard_reachable);
    }
    assert!(
        incidents > 500,
        "{incidents} incidents: the test would be vacuous"
    );
}

#[test]
fn declaring_a_wrong_site_as_a_recall_is_a_collision_everywhere_it_matters() {
    for (seed, truth) in truths(0..SEEDS) {
        let (steps, recalls) = declare_each(&truth, wrong_site);
        let v = score_memory(&truth, &steps, &recalls).expect("scores");
        let t = &v.totals;
        let n = truth.incidents.len() as u32;
        // Every recall is wrong; the source (the incident's own truth) is right.
        assert_eq!(t.recalls.wrong_source_right, n, "seed {seed}");
        assert_eq!(t.recalls.correct(), 0, "seed {seed}");
        let hard = truth
            .incidents
            .iter()
            .filter(|i| i.tier == Tier::Hard)
            .count() as u32;
        assert_eq!(t.stale_wrong.hard, hard, "seed {seed}");
        assert_eq!(t.recalls_on_hard.wrong_source_right, hard, "seed {seed}");
        assert_eq!(
            t.unasked_correct.hard + t.unasked_correct.plain + t.unasked_correct.decoy,
            0
        );
    }
}

#[test]
fn an_escalation_about_each_incident_clears_every_unasked_flag() {
    use gordian_stream::{Question, StreamOutcome};
    for (seed, truth) in truths(0..8) {
        let (mut steps, recalls) = declare_each(&truth, wrong_site);
        for inc in &truth.incidents {
            let focus = *inc.observations.first().expect("an observation");
            steps.push(StreamStep {
                at: Instant(inc.onset_ns),
                action: StreamAction::Escalate {
                    context: Vec::new(),
                    question: Question::Diagnose { focus },
                },
                outcome: StreamOutcome::Escalated {
                    call: 0,
                    ready_at: Instant(inc.onset_ns),
                    cost: gordian_stream::ReasonerCost {
                        calls: 1,
                        tokens: 400,
                        modelled_ns: 100_000_000,
                        latency_ns: 0,
                    },
                },
            });
        }
        let v = score_memory(&truth, &steps, &recalls).expect("scores");
        for row in &v.per_incident {
            assert!(
                !row.unasked_wrong && !row.unasked_correct && !row.stale_wrong,
                "seed {seed}"
            );
            assert_eq!(
                row.recalls.wrong_source_right, 1,
                "seed {seed}: the recall is still counted"
            );
        }
    }
}

/// The class of a hard incident as a set-based reading: (kind, mode) of every hard incident, by
/// site, then each incident asks whether its class appears at a different site among the incidents
/// before it.
fn reference_same_family_earlier(truth: &StreamTruth) -> Vec<bool> {
    let mut seen: Vec<(HardKind, Option<bool>, ServiceId)> = Vec::new();
    let mut out = Vec::new();
    for inc in &truth.incidents {
        let class = (inc.tier == Tier::Hard)
            .then_some(inc.shape.hard_kind)
            .flatten()
            .map(|k| (k, inc.shape.contradicts_early));
        let site = inc.occupies.first().copied();
        out.push(match (class, site) {
            (Some((k, m)), Some(s)) => seen
                .iter()
                .any(|(k2, m2, s2)| *k2 == k && *m2 == m && *s2 != s),
            _ => false,
        });
        if let (Some((k, m)), Some(s)) = (class, site) {
            seen.push((k, m, s));
        }
    }
    out
}

#[test]
fn the_populations_agree_with_a_second_reading_of_the_truth() {
    let (mut recurrences, mut elsewhere, mut both) = (0u32, 0u32, 0u32);
    for (seed, truth) in truths(0..SEEDS * 3) {
        let (steps, recalls) = declare_each(&truth, |inc| inc.truth);
        let v = score_memory(&truth, &steps, &recalls).expect("scores");
        let reference = reference_same_family_earlier(&truth);
        let flags: Vec<bool> = v
            .per_incident
            .iter()
            .map(|r| r.same_family_earlier)
            .collect();
        assert_eq!(flags, reference, "seed {seed}");
        let hard_recurrences = truth
            .incidents
            .iter()
            .filter(|i| i.tier == Tier::Hard && i.recurrence_of.is_some())
            .count() as u32;
        assert_eq!(v.totals.hard_recurrences, hard_recurrences, "seed {seed}");
        recurrences += hard_recurrences;
        elsewhere += v.totals.hard_elsewhere;
        both += truth
            .incidents
            .iter()
            .zip(&reference)
            .filter(|(i, f)| i.tier == Tier::Hard && i.recurrence_of.is_some() && **f)
            .count() as u32;
        // The generator's promise (W2 1.1): a recurrence has its template's tier, class and site.
        for inc in &truth.incidents {
            let Some(t) = inc.recurrence_of else { continue };
            let template = &truth.incidents[t as usize];
            assert_eq!(inc.tier, template.tier, "seed {seed}");
            assert_eq!(inc.shape.hard_kind, template.shape.hard_kind, "seed {seed}");
            assert_eq!(
                inc.shape.contradicts_early, template.shape.contradicts_early,
                "seed {seed}"
            );
            assert_eq!(
                inc.occupies.first(),
                template.occupies.first(),
                "seed {seed}"
            );
            assert!(t < inc.id, "seed {seed}: the template is earlier");
        }
    }
    // Vacuity: all three populations occur in the sample.
    assert!(
        recurrences > 10 && elsewhere > 10 && both > 0,
        "{recurrences} {elsewhere} {both}"
    );
}

/// W2 measured, from the hidden side, 83 hard recurrences and 51 hard incidents with an earlier
/// same-family-and-mode incident at another site and no recurrence, on seeds 40000 to 40199
/// (`w2-learnable-laws.md`, sections 2.1 and 3); and 44 and 33 on 10000 to 10099. The rules K1 and
/// K2 are the same definitions. Run with `cargo test -- --ignored`; 300 streams.
#[test]
#[ignore = "generates 300 streams; run once and record the result"]
fn the_populations_reproduce_w2s_counts() {
    for (range, want_hard, want_recurrences, want_elsewhere) in [
        (40_000..40_200u64, 512u32, 83u32, 51u32),
        (10_000..10_100u64, 243u32, 44u32, 33u32),
    ] {
        let (mut hard, mut recurrences, mut elsewhere) = (0u32, 0u32, 0u32);
        for (_, truth) in truths(range.clone()) {
            let v = score_memory(&truth, &[], &[]).expect("scores");
            hard += truth
                .incidents
                .iter()
                .filter(|i| i.tier == Tier::Hard)
                .count() as u32;
            recurrences += v.totals.hard_recurrences;
            elsewhere += v.totals.hard_elsewhere;
        }
        assert_eq!(
            (hard, recurrences, elsewhere),
            (want_hard, want_recurrences, want_elsewhere),
            "seeds {range:?}"
        );
    }
}
