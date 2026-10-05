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
//! ```
//!
//! A parameter an arm does not have, or an out-of-range one, is a parse error. Every parameter is
//! written (a defaulted one is filled in at parse time), so a manifest records what ran. The
//! shared cheap rung's parameters are not here: they are the same for every arm, so they are the
//! manifest's `rung`.
//!
//! # Roles
//!
//! Three arms are not comparison arms. `oracle_escalation` is privileged (it is built from the
//! stream's truth) and `ablation_hidden_rules` encodes the hidden rules of the stream's hard
//! incidents. [`StreamManifest::validate`](super::manifest::StreamManifest::validate) rejects a
//! privileged arm whose name lacks `privileged` and an ablation arm whose name lacks `ablation`,
//! so that every output that carries the arm's name says what it is, and every `results.csv` row
//! carries an `arm_role` column.

use super::arms::always::{self, Always};
use super::arms::change::{self, Change};
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
    privileged::ID,
    ablation::ID,
];

/// An arm and its configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamPolicySpec {
    /// `never_escalate`.
    Never,
    /// `always_escalate`.
    Always,
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
    /// `random_escalation` with its probability.
    Random {
        /// The probability a noticed anomaly is escalated.
        p: f64,
    },
    /// `oracle_escalation`: privileged.
    Oracle,
    /// `ablation_hidden_rules`: encodes the stream's hidden rules.
    Ablation,
}

impl StreamPolicySpec {
    /// The arm with `id` and its default configuration, or why there is none.
    pub fn from_id(id: &str) -> Result<Self, String> {
        Self::from_parts(id, None, None, None, None)
    }

    /// The arm `id` with the given parameters. A parameter the arm does not have is an error.
    pub fn from_parts(
        id: &str,
        period_ns: Option<u64>,
        tau: Option<f64>,
        wait_ns: Option<u64>,
        p: Option<f64>,
    ) -> Result<Self, String> {
        let stray = |name: &str| format!("policy {id:?} has no parameter {name:?}");
        let only = |allowed: &[&str]| -> Result<(), String> {
            for (name, given) in [
                ("period_ns", period_ns.is_some()),
                ("tau", tau.is_some()),
                ("wait_ns", wait_ns.is_some()),
                ("p", p.is_some()),
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
                only(&[])?;
                Self::Always
            }
            change::ID => {
                only(&[])?;
                Self::Change
            }
            privileged::ID => {
                only(&[])?;
                Self::Oracle
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
                only(&["p"])?;
                Self::Random {
                    p: p.unwrap_or(random::DEFAULT_P),
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
            Self::Random { p } if !(p.is_finite() && (0.0..=1.0).contains(p)) => {
                Err(format!("random_escalation needs p in [0, 1], got {p}"))
            }
            _ => Ok(()),
        }
    }

    fn id_str(&self) -> &'static str {
        match self {
            Self::Never => never::ID,
            Self::Always => always::ID,
            Self::Periodic { .. } => periodic::ID,
            Self::Change => change::ID,
            Self::Threshold { .. } => threshold::ID,
            Self::Random { .. } => random::ID,
            Self::Oracle => privileged::ID,
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
            Self::Oracle => ArmRole::Privileged,
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
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Repr {
    Id(String),
    Full(Tagged),
}

impl Serialize for StreamPolicySpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let tagged = |period_ns, tau, wait_ns, p| Tagged {
            policy: self.id_str().to_owned(),
            period_ns,
            tau,
            wait_ns,
            p,
        };
        match self {
            Self::Periodic { period_ns } => {
                tagged(Some(*period_ns), None, None, None).serialize(serializer)
            }
            Self::Threshold { tau, wait_ns } => {
                tagged(None, Some(*tau), Some(*wait_ns), None).serialize(serializer)
            }
            Self::Random { p } => tagged(None, None, None, Some(*p)).serialize(serializer),
            other => serializer.serialize_str(other.id_str()),
        }
    }
}

impl<'de> Deserialize<'de> for StreamPolicySpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let result = match Repr::deserialize(deserializer)? {
            Repr::Id(id) => Self::from_id(&id),
            Repr::Full(t) => Self::from_parts(&t.policy, t.period_ns, t.tau, t.wait_ns, t.p),
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
    matches!(spec, StreamPolicySpec::Oracle).then(|| privileged::OracleFactory::new(rung.clone()))
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
        StreamPolicySpec::Always => Box::new(StreamArm::with(Always, public, config)),
        StreamPolicySpec::Periodic { period_ns } => {
            Box::new(StreamArm::with(Periodic::new(*period_ns), public, config))
        }
        StreamPolicySpec::Change => Box::new(StreamArm::with(Change, public, config)),
        StreamPolicySpec::Threshold { tau, wait_ns } => Box::new(StreamArm::with(
            Threshold::new(*tau, *wait_ns),
            public,
            config,
        )),
        StreamPolicySpec::Random { p } => Box::new(StreamArm::with(
            Random::new(*p, rng_seed(stream_seed, arm)),
            public,
            config,
        )),
        StreamPolicySpec::Ablation => Box::new(StreamArm::with(
            ablation::HiddenRules::new(),
            public,
            config,
        )),
        StreamPolicySpec::Oracle => return None,
    })
}
