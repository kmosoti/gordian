//! The stream's vocabulary of hypotheses and references.
//!
//! Everything here is public: a policy may name any hypothesis, including the hard kinds. What is
//! hidden is which hypothesis is true, and the rules that produce a hard kind's symptoms.

use gordian_world::{FaultKind, ServiceId};
use serde::{Deserialize, Serialize};

/// The three kinds of incident a stream contains. The tier of an incident is hidden state: a
/// policy never receives it, and the generator is built so that no public statistic of the first
/// moments separates a hard incident from a decoy (`HIDDEN-DESIGN.md`, section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    /// Obeys the first world's public physics. The cheap rung identifies it from the stream or
    /// with one probe.
    Plain,
    /// Obeys rules the cheap rung does not have. Not identifiable by the cheap rung.
    Hard,
    /// Looks like a hard incident for its first seconds and then resolves by itself.
    Decoy,
}

/// Fault kinds whose rules are not in the first world's public physics. A policy can name them
/// in a hypothesis; the rules that produce their symptoms are known only to the simulated
/// reasoner (`HIDDEN-DESIGN.md`, section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HardKind {
    /// Two known fault kinds at one site at once. The site is the faulty service.
    Compound,
    /// A known fault at a site that also damages a service the public graph does not connect to
    /// it, over an edge the public graph does not have. The site is the root, the first to alarm.
    Cascade,
    /// Two services both act as primary. The site is the one that alarmed first.
    SplitBrain,
    /// A resource leak: a slow climb in a benign-range reading that later crosses the alarm
    /// threshold. The site is the leaking service.
    SlowLeak,
}

impl HardKind {
    /// Every hard kind, in declaration order.
    pub const ALL: [HardKind; 4] = [
        HardKind::Compound,
        HardKind::Cascade,
        HardKind::SplitBrain,
        HardKind::SlowLeak,
    ];
}

/// A kind of fault in the stream: one of the first world's five, or a hard kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StreamKind {
    /// A kind with public rules.
    Known(FaultKind),
    /// A kind without.
    Hard(HardKind),
}

/// A claim that an incident of `kind` is at `site`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StreamHypothesis {
    /// What is wrong.
    pub kind: StreamKind,
    /// Where (see [`HardKind`] for what the site of each hard kind is).
    pub site: ServiceId,
}

/// A diagnosis: `None` says the observation or incident asked about is not an incident (a decoy
/// that resolves by itself, or background).
pub type Diagnosis = Option<StreamHypothesis>;

/// Index of a passive observation in the stream, in delivery order. Dense: a policy that has
/// observed everything up to some instant holds exactly the ids `0..n` for some `n`, so an id
/// carries no information beyond "the n-th observation".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObsId(pub u32);

/// A reference to an observation a policy holds, for the reasoner's context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ObsRef {
    /// A passive observation delivered by `observe_until`.
    Passive(ObsId),
    /// The result of the n-th accepted probe, counting from zero.
    Probe(u32),
}

/// What the reasoner is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Question {
    /// What is wrong with the incident that observation `focus` belongs to? The answer is a
    /// [`Diagnosis`]; `None` says the observation is not part of an incident. The focus names
    /// the incident asked about; it is evidence only if it is also in the context.
    Diagnose {
        /// An observation the policy holds.
        focus: ObsId,
    },
}
