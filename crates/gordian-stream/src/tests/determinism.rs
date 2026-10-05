//! Generation and replay are pure functions of the seed and the parameters.

use super::*;
use crate::{ObsRef, Question, StreamAction, StreamEvent, StreamOutcome, StreamSimulator};
use gordian_world::physics::HIGH;
use gordian_world::{Observation, ProbeKind, ServiceId};

#[test]
fn generating_twice_gives_equal_streams() {
    for seed in 0..25 {
        let p = StreamParams::new(seed);
        assert_eq!(generate(&p), generate(&p), "seed {seed}");
    }
    // Under other parameters too.
    let mut p = StreamParams::new(3);
    p.mean_gap_ns = 5_000_000_000;
    p.recurrence_permille = 700;
    p.mix.hard_permille = 400;
    p.regimes.clear();
    assert_eq!(generate(&p), generate(&p));
}

#[test]
fn different_seeds_give_different_streams() {
    let a = generate(&StreamParams::new(1));
    let b = generate(&StreamParams::new(2));
    assert_ne!(a.events(), b.events());
}

#[test]
fn normalization_is_idempotent_and_generate_normalizes() {
    let mut p = StreamParams::new(4);
    p.min_services = 1;
    p.max_services = 200;
    p.mix.plain_permille = 900;
    p.mix.hard_permille = 900;
    p.recurrence_permille = 5000;
    let n = p.normalized();
    assert_eq!(n, n.normalized());
    assert_eq!((n.min_services, n.max_services), (6, 12));
    assert_eq!(n.mix.plain_permille + n.mix.hard_permille, 1000);
    assert_eq!(generate(&p), generate(&n));
}

#[test]
fn params_survive_a_json_round_trip() {
    let p = StreamParams::new(11);
    let text = serde_json::to_string(&p).unwrap();
    let back: StreamParams = serde_json::from_str(&text).unwrap();
    assert_eq!(p, back);
    assert_eq!(generate(&p), generate(&back));
}

/// A fixed policy, expressed as a function of what it has observed so far, run to the end of the
/// stream. Returns a transcript of everything the simulator said.
fn run_script(params: &StreamParams) -> Vec<String> {
    let mut sim = StreamSimulator::new(generate(params));
    let mut log = Vec::new();
    let mut held: Vec<crate::ObsId> = Vec::new();
    let mut last_abnormal: Option<crate::ObsId> = None;
    let step = 2_000_000_000u64;
    let mut now = 0u64;
    let mut tick = 0u32;
    while now <= params.duration_ns {
        let at = gordian_core::Instant(now);
        for e in sim.observe_until(at) {
            if let StreamEvent::Observed { id, obs, .. } = &e {
                held.push(*id);
                if matches!(obs, Observation::Counter { value, .. } if *value >= HIGH) {
                    last_abnormal = Some(*id);
                }
            }
            log.push(format!("{e:?}"));
        }
        tick += 1;
        if let Some(focus) = last_abnormal {
            match tick % 5 {
                0 => {
                    let o = sim.apply(
                        StreamAction::Probe {
                            kind: ProbeKind::ALL[(tick as usize / 5) % 6],
                            target: ServiceId(tick % 6),
                        },
                        at,
                    );
                    log.push(format!("{o:?}"));
                }
                1 => {
                    let context: Vec<ObsRef> = held
                        .iter()
                        .rev()
                        .take(12)
                        .map(|i| ObsRef::Passive(*i))
                        .collect();
                    let o = sim.apply(
                        StreamAction::Escalate {
                            context,
                            question: Question::Diagnose { focus },
                        },
                        at,
                    );
                    log.push(format!("{o:?}"));
                }
                2 => {
                    let o = sim.apply(
                        StreamAction::Declare {
                            anchor: focus,
                            diagnosis: None,
                        },
                        at,
                    );
                    log.push(format!("{o:?}"));
                }
                _ => {}
            }
        }
        now += step;
    }
    log.push(format!("{:?}", sim.remaining()));
    log
}

#[test]
fn replaying_a_fixed_action_script_gives_the_same_transcript() {
    for seed in [1u64, 2, 3] {
        let p = StreamParams::new(seed);
        let a = run_script(&p);
        let b = run_script(&p);
        assert_eq!(a, b, "seed {seed}");
        // The script must have done things, or the comparison proves nothing.
        assert!(a.iter().any(|l| l.starts_with("Probed")), "no probe ran");
        assert!(
            a.iter().any(|l| l.starts_with("Escalated")),
            "no escalation ran"
        );
        assert!(
            a.iter().any(|l| l.starts_with("Declared")),
            "no declaration ran"
        );
        assert!(
            a.iter()
                .any(|l| l.starts_with("Answered") || l.contains("Answered {"))
        );
    }
}

#[test]
fn a_cloned_simulator_replays_from_the_clone_point() {
    let mut sim = StreamSimulator::new(generate(&StreamParams::new(5)));
    let first = sim.observe_until(gordian_core::Instant(60_000_000_000));
    let focus = first
        .iter()
        .find_map(|e| match e {
            StreamEvent::Observed { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap();
    let mut fork = sim.clone();
    let ask = |s: &mut StreamSimulator| {
        let o = s.apply(
            StreamAction::Escalate {
                context: vec![ObsRef::Passive(focus)],
                question: Question::Diagnose { focus },
            },
            gordian_core::Instant(60_000_000_000),
        );
        assert!(matches!(o, StreamOutcome::Escalated { .. }));
        s.observe_until(gordian_core::Instant(70_000_000_000))
    };
    assert_eq!(ask(&mut sim), ask(&mut fork));
}
