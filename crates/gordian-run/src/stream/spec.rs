//! The stream arms as the manifest writes them, and the registry that builds one.
//!
//! In JSON an arm with nothing to configure is its id, `"never_escalate"`. An arm with
//! parameters is an object with a `policy` field and its parameters:
//!
//! ```json
//! "never_escalate"
//! {"policy": "periodic_escalation", "period_ns": 10000000000}
//! {"policy": "threshold_score", "tau": 3.5, "wait_ns": 6000000000}
//! {"policy": "random_escalation", "p": 0.5}
//! {"policy": "random_escalation", "p": 0.5, "delay_ns": 8000000000}
//! {"policy": "always_escalate", "delay_ns": 8000000000}
//! {"policy": "contradiction_escalation", "delay_ns": 8000000000, "persist_ns": 2000000000}
//! {"policy": "oracle_selection", "delay_ns": 8000000000}
//! {"policy": "oracle_selection_context", "delay_ns": 8000000000}
//! ```
//!
//! `delay_ns` (`always_escalate`, `random_escalation`, `contradiction_escalation` and
//! `oracle_selection`) is how long after an anomaly is noticed the arm escalates it. Its default is 0, the arm as R3 built it; a delay of 0 is not written
//! (`always_escalate` is then its bare id, and `random_escalation` has no `delay_ns`), so a
//! manifest written before the parameter existed is the same text as one written now.
//!
//! `persist_ns` (`contradiction_escalation`) is how long the consistency checker must have found
//! no hypothesis consistent with the anomaly's evidence before the arm escalates; its default is 0
//! and, like a zero delay, it is not written.
//!
//! A parameter an arm does not have, or an out-of-range one, is a parse error. Every parameter is
//! written (a defaulted one is filled in at parse time), so a manifest records what ran. The
//! shared cheap rung's parameters are not here: they are the same for every arm, so they are the
//! manifest's `rung`.
//!
//! # Roles
//!
//! Six arms are not comparison arms. `oracle_escalation`, `oracle_selection`,
//! `oracle_selection_context` (R6's supplementary ceiling) and `oracle_decoy` are privileged (they are built from the stream's truth) and `ablation_hidden_rules` encodes
//! the hidden rules of the stream's hard incidents. [`StreamManifest::validate`](super::manifest::StreamManifest::validate) rejects a
//! privileged arm whose name lacks `privileged` and an ablation arm whose name lacks `ablation`,
//! so that every output that carries the arm's name says what it is, and every `results.csv` row
//! carries an `arm_role` column.

use super::arms::always::{self, Always};
use super::arms::change::{self, Change};
use super::arms::contradiction::{self, Contradiction};
use super::arms::never::{self, Never};
use super::arms::periodic::{self, Periodic};
use super::arms::random::{self, Random};
use super::arms::rung::RungConfig;
use super::arms::threshold::{self, Threshold};
use super::arms::{ArmRole, StreamArm, StreamPolicy, ablation};
use super::privileged;
use crate::policy::PolicyId;
use crate::policy::random_matched::rng_seed;
use gordian_stream::StreamPublic;
use serde::{Deserialize, Serialize};

/// Ids of the arms [`build`] knows.
pub const KNOWN: &[&str] = &[
    never::ID,
    always::ID,
    periodic::ID,
    change::ID,
    threshold::ID,
    random::ID,
    contradiction::ID,
    privileged::ID,
    privileged::SELECTION_ID,
    privileged::DECOY_ID,
    privileged::SELECTION_CONTEXT_ID,
    ablation::ID,
];

/// An arm and its configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamPolicySpec {
    /// `never_escalate`.
    Never,
    /// `always_escalate` with its delay after notice.
    Always {
        /// Nanoseconds after notice before the anomaly is escalated.
        delay_ns: u64,
    },
    /// `periodic_escalation` with its period.
    Periodic {
        /// Nanoseconds between reviews.
        period_ns: u64,
    },
    /// `change_triggered`.
    Change,
    /// `threshold_score` with its threshold and wait.
    Threshold {
        /// The score at or above which an anomaly is escalated.
        tau: f64,
        /// How long after notice an anomaly below `tau` is waited on.
        wait_ns: u64,
    },
    /// `random_escalation` with its probability and delay after notice.
    Random {
        /// The probability a noticed anomaly is escalated.
        p: f64,
        /// Nanoseconds after notice before a selected anomaly is escalated.
        delay_ns: u64,
    },
    /// `contradiction_escalation` with its delay after notice and its persistence.
    Contradiction {
        /// Nanoseconds after notice before an anomaly may be escalated.
        delay_ns: u64,
        /// Nanoseconds the consistency checker must have found no hypothesis.
        persist_ns: u64,
    },
    /// `oracle_escalation`: privileged.
    Oracle,
    /// `oracle_selection`: privileged; escalates exactly the hard anomalies.
    OracleSelection {
        /// Nanoseconds after notice before a hard anomaly is escalated.
        delay_ns: u64,
    },
    /// `oracle_decoy`: privileged; dismisses exactly the decoy anomalies.
    OracleDecoy,
    /// `oracle_selection_context` (R6, supplementary): privileged; escalates exactly the hard
    /// anomalies with the decisive evidence delivered so far as the context.
    OracleSelectionContext {
        /// Nanoseconds after notice before a hard anomaly is escalated.
        delay_ns: u64,
    },
    /// `ablation_hidden_rules`: encodes the stream's hidden rules.
    Ablation,
}

impl StreamPolicySpec {
    /// The arm with `id` and its default configuration, or why there is none.
    pub fn from_id(id: &str) -> Result<Self, String> {
        Self::from_parts(id, None, None, None, None, None, None)
    }

    /// The arm `id` with the given parameters. A parameter the arm does not have is an error.
    pub fn from_parts(
        id: &str,
        period_ns: Option<u64>,
        tau: Option<f64>,
        wait_ns: Option<u64>,
        p: Option<f64>,
        delay_ns: Option<u64>,
        persist_ns: Option<u64>,
    ) -> Result<Self, String> {
        let stray = |name: &str| format!("policy {id:?} has no parameter {name:?}");
        let only = |allowed: &[&str]| -> Result<(), String> {
            for (name, given) in [
                ("period_ns", period_ns.is_some()),
                ("tau", tau.is_some()),
                ("wait_ns", wait_ns.is_some()),
                ("p", p.is_some()),
                ("delay_ns", delay_ns.is_some()),
                ("persist_ns", persist_ns.is_some()),
            ] {
                if given && !allowed.contains(&name) {
                    return Err(stray(name));
                }
            }
            Ok(())
        };
        let spec = match id {
            never::ID => {
                only(&[])?;
                Self::Never
            }
            always::ID => {
                only(&["delay_ns"])?;
                Self::Always {
                    delay_ns: delay_ns.unwrap_or(always::DEFAULT_DELAY_NS),
                }
            }
            change::ID => {
                only(&[])?;
                Self::Change
            }
            privileged::ID => {
                only(&[])?;
                Self::Oracle
            }
            privileged::SELECTION_ID => {
                only(&["delay_ns"])?;
                Self::OracleSelection {
                    delay_ns: delay_ns.unwrap_or(0),
                }
            }
            privileged::DECOY_ID => {
                only(&[])?;
                Self::OracleDecoy
            }
            privileged::SELECTION_CONTEXT_ID => {
                only(&["delay_ns"])?;
                Self::OracleSelectionContext {
                    delay_ns: delay_ns.unwrap_or(0),
                }
            }
            contradiction::ID => {
                only(&["delay_ns", "persist_ns"])?;
                Self::Contradiction {
                    delay_ns: delay_ns.unwrap_or(contradiction::DEFAULT_DELAY_NS),
                    persist_ns: persist_ns.unwrap_or(contradiction::DEFAULT_PERSIST_NS),
                }
            }
            ablation::ID => {
                only(&[])?;
                Self::Ablation
            }
            periodic::ID => {
                only(&["period_ns"])?;
                Self::Periodic {
                    period_ns: period_ns.unwrap_or(periodic::DEFAULT_PERIOD_NS),
                }
            }
            threshold::ID => {
                only(&["tau", "wait_ns"])?;
                Self::Threshold {
                    tau: tau.unwrap_or(threshold::DEFAULT_TAU),
                    wait_ns: wait_ns.unwrap_or(threshold::DEFAULT_WAIT_NS),
                }
            }
            random::ID => {
                only(&["p", "delay_ns"])?;
                Self::Random {
                    p: p.unwrap_or(random::DEFAULT_P),
                    delay_ns: delay_ns.unwrap_or(random::DEFAULT_DELAY_NS),
                }
            }
            other => return Err(format!("unknown policy {other:?}; known: {KNOWN:?}")),
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Periodic { period_ns } if *period_ns == 0 => {
                Err("periodic_escalation needs period_ns of at least 1".to_owned())
            }
            Self::Threshold { tau, .. } if !tau.is_finite() => {
                Err("threshold_score needs a finite tau".to_owned())
            }
            Self::Random { p, .. } if !(p.is_finite() && (0.0..=1.0).contains(p)) => {
                Err(format!("random_escalation needs p in [0, 1], got {p}"))
            }
            _ => Ok(()),
        }
    }

    fn id_str(&self) -> &'static str {
        match self {
            Self::Never => never::ID,
            Self::Always { .. } => always::ID,
            Self::Periodic { .. } => periodic::ID,
            Self::Change => change::ID,
            Self::Threshold { .. } => threshold::ID,
            Self::Random { .. } => random::ID,
            Self::Contradiction { .. } => contradiction::ID,
            Self::Oracle => privileged::ID,
            Self::OracleSelection { .. } => privileged::SELECTION_ID,
            Self::OracleDecoy => privileged::DECOY_ID,
            Self::OracleSelectionContext { .. } => privileged::SELECTION_CONTEXT_ID,
            Self::Ablation => ablation::ID,
        }
    }

    /// The arm's id.
    pub fn id(&self) -> PolicyId {
        PolicyId::new(self.id_str())
    }

    /// What the arm is: its name must say so unless it is a comparison arm.
    pub fn role(&self) -> ArmRole {
        match self {
            Self::Oracle
            | Self::OracleSelection { .. }
            | Self::OracleSelectionContext { .. }
            | Self::OracleDecoy => ArmRole::Privileged,
            Self::Ablation => ArmRole::Ablation,
            _ => ArmRole::Comparison,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tagged {
    policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    period_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tau: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wait_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delay_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    persist_ns: Option<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Repr {
    Id(String),
    Full(Tagged),
}

impl Serialize for StreamPolicySpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let tagged = |period_ns, tau, wait_ns, p, delay_ns, persist_ns| Tagged {
            policy: self.id_str().to_owned(),
            period_ns,
            tau,
            wait_ns,
            p,
            delay_ns,
            persist_ns,
        };
        // A delay of zero is the arm as it was before the parameter existed and is not written.
        let delay = |d: u64| (d != 0).then_some(d);
        match self {
            Self::Periodic { period_ns } => {
                tagged(Some(*period_ns), None, None, None, None, None).serialize(serializer)
            }
            Self::Threshold { tau, wait_ns } => {
                tagged(None, Some(*tau), Some(*wait_ns), None, None, None).serialize(serializer)
            }
            Self::Random { p, delay_ns } => {
                tagged(None, None, None, Some(*p), delay(*delay_ns), None).serialize(serializer)
            }
            Self::Always { delay_ns } if *delay_ns != 0 => {
                tagged(None, None, None, None, Some(*delay_ns), None).serialize(serializer)
            }
            Self::Contradiction {
                delay_ns,
                persist_ns,
            } if *delay_ns != 0 || *persist_ns != 0 => {
                tagged(None, None, None, None, delay(*delay_ns), delay(*persist_ns))
                    .serialize(serializer)
            }
            Self::OracleSelection { delay_ns } | Self::OracleSelectionContext { delay_ns }
                if *delay_ns != 0 =>
            {
                tagged(None, None, None, None, Some(*delay_ns), None).serialize(serializer)
            }
            other => serializer.serialize_str(other.id_str()),
        }
    }
}

impl<'de> Deserialize<'de> for StreamPolicySpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let result = match Repr::deserialize(deserializer)? {
            Repr::Id(id) => Self::from_id(&id),
            Repr::Full(t) => Self::from_parts(
                &t.policy,
                t.period_ns,
                t.tau,
                t.wait_ns,
                t.p,
                t.delay_ns,
                t.persist_ns,
            ),
        };
        result.map_err(serde::de::Error::custom)
    }
}

/// The factory of the privileged arm, if `spec` is it. Such an arm is built from the segment's
/// truth by the harness ([`crate::stream::harness::run_segment_privileged`]) and by nothing else.
pub fn privileged_factory(
    spec: &StreamPolicySpec,
    rung: &RungConfig,
) -> Option<privileged::OracleFactory> {
    match spec {
        StreamPolicySpec::Oracle => Some(privileged::OracleFactory::new(rung.clone())),
        StreamPolicySpec::OracleSelection { delay_ns } => Some(
            privileged::OracleFactory::selection(rung.clone(), *delay_ns),
        ),
        StreamPolicySpec::OracleSelectionContext { delay_ns } => Some(
            privileged::OracleFactory::selection_context(rung.clone(), *delay_ns),
        ),
        StreamPolicySpec::OracleDecoy => Some(privileged::OracleFactory::decoy(rung.clone())),
        _ => None,
    }
}

/// A fresh arm for one segment of the arm named `arm`, whose stream has seed `stream_seed`, or
/// `None` for the privileged arm, which has [`privileged_factory`] instead.
///
/// `rung` is the shared cheap rung's parameters, the same for every arm of a run. The seed is
/// used only to derive the random arm's generator, together with the arm name; nothing else sees
/// it. `public` is what an arm may know of the stream.
pub fn build_public(
    spec: &StreamPolicySpec,
    rung: &RungConfig,
    public: &StreamPublic,
    arm: &str,
    stream_seed: u64,
) -> Option<Box<dyn StreamPolicy>> {
    let config = rung.clone();
    Some(match spec {
        StreamPolicySpec::Never => Box::new(StreamArm::with(Never, public, config)),
        StreamPolicySpec::Always { delay_ns } => {
            Box::new(StreamArm::with(Always::new(*delay_ns), public, config))
        }
        StreamPolicySpec::Periodic { period_ns } => {
            Box::new(StreamArm::with(Periodic::new(*period_ns), public, config))
        }
        StreamPolicySpec::Change => Box::new(StreamArm::with(Change, public, config)),
        StreamPolicySpec::Threshold { tau, wait_ns } => Box::new(StreamArm::with(
            Threshold::new(*tau, *wait_ns),
            public,
            config,
        )),
        StreamPolicySpec::Random { p, delay_ns } => Box::new(StreamArm::with(
            Random::with_delay(*p, *delay_ns, rng_seed(stream_seed, arm)),
            public,
            config,
        )),
        StreamPolicySpec::Contradiction {
            delay_ns,
            persist_ns,
        } => Box::new(StreamArm::with(
            Contradiction::new(*delay_ns, *persist_ns),
            public,
            config,
        )),
        StreamPolicySpec::Ablation => Box::new(StreamArm::with(
            ablation::HiddenRules::new(),
            public,
            config,
        )),
        StreamPolicySpec::Oracle
        | StreamPolicySpec::OracleSelection { .. }
        | StreamPolicySpec::OracleSelectionContext { .. }
        | StreamPolicySpec::OracleDecoy => return None,
    })
}
