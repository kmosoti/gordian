//! The escalation delay of `always_escalate` and `random_escalation` (work item R4).
//!
//! A delay of zero must be the arm as R3 built it, byte for byte. The fixtures in
//! `fixtures/stream-delay0/` were written by the binary built from the commit before the delay
//! existed (`30091e3`), from the manifest beside them: 6 streams of the default 600 s, four arms
//! (`always_escalate`, `random_escalation` at p = 0.5 and p = 0.3, `never_escalate`), through
//! `scripts/cgroup-run.sh`. Each arm's `results.csv` and `incidents.csv` are compared here with
//! what the current code writes from the same manifest. The manifest is in the old spelling
//! (no `delay_ns`), so the test also checks that it still parses.

mod stream_common;

use gordian_run::stream::execute_stream;
use gordian_run::stream::manifest::StreamManifest;
use gordian_run::stream::results::results_row;
use gordian_run::stream::spec::StreamPolicySpec;
use std::fs;
use std::path::{Path, PathBuf};
use stream_common::*;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stream-delay0")
}

fn fixture_manifest() -> StreamManifest {
    let text = fs::read_to_string(fixture_dir().join("manifest.json")).unwrap();
    serde_json::from_str(&text).expect("the pre-delay manifest still parses")
}

fn run(m: &StreamManifest, name: &str) -> PathBuf {
    let out = scratch(name).join("run");
    execute_stream(m, &out).unwrap_or_else(|e| panic!("{e}"));
    out
}

fn assert_fixture(m: &StreamManifest, out: &Path) {
    for arm in &m.arms {
        for file in ["results.csv", "incidents.csv"] {
            let expected =
                fs::read_to_string(fixture_dir().join(format!("{}.{file}", arm.arm))).unwrap();
            let got = fs::read_to_string(out.join(&arm.arm).join(file)).unwrap();
            assert!(
                expected == got,
                "{} {file} differs from the pre-delay fixture",
                arm.arm
            );
        }
    }
}

#[test]
fn a_delay_of_zero_reproduces_the_pre_delay_results_byte_for_byte() {
    let m = fixture_manifest();
    assert_eq!(m.arms.len(), 4);
    assert_fixture(&m, &run(&m, "delay0-implicit"));
}

#[test]
fn an_explicit_delay_of_zero_is_the_same_arm_as_none() {
    let mut m = fixture_manifest();
    for arm in &mut m.arms {
        arm.policy = match arm.policy.clone() {
            StreamPolicySpec::Always { .. } => StreamPolicySpec::Always { delay_ns: 0 },
            StreamPolicySpec::Random { p, .. } => StreamPolicySpec::Random { p, delay_ns: 0 },
            other => other,
        };
    }
    // Written without the zero, so the manifest text of these arms is the pre-delay text.
    let written: serde_json::Value = serde_json::from_str(&m.canonical_json()).unwrap();
    let original: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_dir().join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(written["arms"], original["arms"]);
    assert_fixture(&m, &run(&m, "delay0-explicit"));
}

#[test]
fn a_delay_changes_when_the_arm_escalates_and_a_delay_past_the_stream_is_never() {
    let p = params(5, 200);
    let row = |spec: &StreamPolicySpec| results_row("x", &play(&p, spec, &limits(&p)).unwrap());
    let never = row(&StreamPolicySpec::Never);
    let now = row(&StreamPolicySpec::Always { delay_ns: 0 });
    let later = row(&StreamPolicySpec::Always {
        delay_ns: 8_000_000_000,
    });
    assert_ne!(now, later, "the delay moves the escalations");
    let beyond = 1_000_000_000_000;
    assert_eq!(never, row(&StreamPolicySpec::Always { delay_ns: beyond }));
    assert_eq!(
        never,
        row(&StreamPolicySpec::Random {
            p: 1.0,
            delay_ns: beyond
        })
    );
    assert_eq!(
        now,
        row(&StreamPolicySpec::Random {
            p: 1.0,
            delay_ns: 0
        })
    );
    assert_eq!(
        later,
        row(&StreamPolicySpec::Random {
            p: 1.0,
            delay_ns: 8_000_000_000
        }),
        "random with p = 1 is always, at any delay"
    );
}
