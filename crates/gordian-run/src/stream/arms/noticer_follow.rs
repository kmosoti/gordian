//! The follow-up rule on a ramp-noticed anomaly (work item B4): retire it, or keep it, from the
//! counter's readings after the ramp was noticed.
//!
//! # The question it puts
//!
//! B3's ramp noticer notices a slow leak within the background budget and notices the look-alike
//! decoys as readily: a decoy's counter ramps like a leak's, so the first five readings do not
//! separate them (B3's report, point 3). The queue's hypothesis is that something public does, later:
//! *a decoy's readings turn benign; a leak's keep rising*. This rule tests it. It acts after the
//! notice, so the leak is noticed as early as before (the notice is made at the reading that
//! completes the ramp, and is on the record whether or not the rule later retires the anomaly), and
//! what changes is whether the anomaly lives on to be asked about. Status: built, with the readings
//! below stated before any tuning; its four parameters are tuned on seeds 10000-10099
//! (`scripts/b4_*.py`), under a rule fixed before the run.
//!
//! # What it is, in one paragraph
//!
//! For each anomaly the ramp noticer opens ([`super::noticer_ramp::RampNoticer`]) the rule keeps a
//! *watch*: the key (service and counter name) of the chain that crossed, the value of the reading
//! that completed the ramp (the *completing reading*, the fifth of the chosen ramp) and then, in
//! delivery order, every later reading at that key. It decides **keep** or **withdraw** once, from
//! those readings, and a withdrawn anomaly is retired as the rung retires any anomaly (it is
//! given the rule's final call, then forgotten) at the next step at which nothing is pending for
//! it. It reads counter values and instants and nothing else: no label, no counter name's meaning,
//! no tier, and not the public rules' verdict on a reading.
//!
//! # The readings, stated before any tuning
//!
//! - **"Readings after the first five."** The readings at the chain's key that follow the
//!   completing reading in delivery order. *Every* reading at the key counts, whether or not it
//!   continues the chain: a reading that does not continue the chain (a drop of more than the
//!   chain's `max_drop`, or a jump of more than its `max_step`) is the kind of reading the rule is
//!   looking for. The first five are the ramp's own `min_readings`, whatever it is tuned to.
//! - **"Turn benign."** Two ways, both parameters. A follow-up reading more than `max_fall` below
//!   the highest value the key has had since the completing reading (the completing reading
//!   included) is a reversal and withdraws the anomaly at once (`max_fall = u32::MAX` never
//!   does). And a decision at the end of the window, below.
//! - **"Keep rising."** After `readings` follow-up readings the anomaly is kept if the latest of
//!   them is at least `min_gain` above the completing reading's value, and withdrawn if it is not.
//!   `min_gain = 0` keeps an anomaly whose counter has not fallen below where it was noticed. The
//!   latest reading, not the best or a mean: a leak's readings are noisy, and a reading
//!   that is a stray is judged for what it is; the window `readings` is how much of that noise the
//!   rule averages away by waiting, and `max_fall` is how large a single dip is tolerated.
//! - **The window can be cut short by time.** If fewer than `readings` follow-up readings have
//!   arrived `horizon_ns` after the completing reading, the rule decides with what has arrived:
//!   with none, withdraw (nothing continued the counter's life); with some, as above on the latest.
//!   The instant is that of the readings, not of the harness's steps, and the decision is made at
//!   the step that processes the reading or finds the horizon passed.
//! - **"Retires the anomaly or keeps it."** Keep: the anomaly is left to the rung and the selector as
//!   any is, and is never reconsidered. Withdraw: it is retirable from that step on, and the record
//!   of the retirement says the rule made it ([`super::noticer::RetireCause::Followup`]). An anomaly
//!   that is asked about before the rule withdraws it is retired after the answer, as any is; the
//!   evaluator's selection accounting (E4, E5) separates the two.
//! - **Only ramp-opened anomalies.** An anomaly the base noticer opens (or the splitting noticer)
//!   is never watched, whatever it is anchored on.
//! - **The notice stays.** The notice record and every notice measure (N1 to N16) are what they are
//!   without the rule; a retirement is a retirement record and moves none of them. What the rule
//!   can change in them is later notices: a retired anomaly no longer takes the abnormal observations
//!   at its site, which then open candidates of the base's, and the background budget must be
//!   read on the arm with the rule on.
//!
//! # What it cannot do
//!
//! Anything that needs the readings to say something they do not. If a decoy's counter keeps rising
//! for as long as a leak's does, the rule keeps it; if a leak's counter plateaus or dips beyond
//! `max_fall` the rule withdraws it. Both errors are counted by the evaluator (E5), and the
//! tuning rule in `scripts/b4_common.py` forbids any leak withdrawn on the tuning streams.

use super::noticer_ramp::Key;
use gordian_core::Instant;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The parameters of the follow-up rule. Every one is written to a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FollowSpec {
    /// How many readings after the completing one the rule waits for (at least 1).
    pub readings: u32,
    /// How long after the completing reading the rule waits for them, nanoseconds (at least 1).
    pub horizon_ns: u64,
    /// How far above the completing reading's value the latest follow-up reading must be for the
    /// anomaly to be kept.
    pub min_gain: u32,
    /// How far below the highest value since the completing reading a follow-up reading may be
    /// before the anomaly is withdrawn at once; `u32::MAX` never withdraws on a reversal.
    pub max_fall: u32,
}

impl FollowSpec {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.readings == 0 {
            return Err("noticer ramp follow: readings must be at least 1".to_owned());
        }
        if self.horizon_ns == 0 {
            return Err("noticer ramp follow: horizon_ns must be at least 1".to_owned());
        }
        Ok(())
    }
}

/// What the rule concluded about one anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Leave the anomaly to the rung.
    Keep,
    /// Retire the anomaly.
    Withdraw,
}

/// What one watch has seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    /// The completing reading's value.
    pub level0: u64,
    /// Follow-up readings so far.
    pub readings: u32,
    /// The highest value since the completing reading, that reading included.
    pub peak: u64,
    /// The latest follow-up reading's value (the completing reading's when there is none).
    pub last: u64,
}

impl FollowSpec {
    /// The verdict, if there is one yet, after the readings in `seen` and before the horizon: a
    /// reversal withdraws; `readings` follow-up readings decide on the latest; fewer decide nothing.
    pub fn judge(&self, seen: &Seen) -> Option<Verdict> {
        if seen.readings >= 1 && seen.peak.saturating_sub(seen.last) > u64::from(self.max_fall) {
            return Some(Verdict::Withdraw);
        }
        if seen.readings >= self.readings {
            return Some(self.settle(seen));
        }
        None
    }

    /// The verdict on the readings seen, whatever their number: none seen withdraws; some are judged
    /// on the latest. What the rule says at the horizon, and after `readings` of them.
    pub fn settle(&self, seen: &Seen) -> Verdict {
        if seen.readings >= 1 && seen.last >= seen.level0.saturating_add(u64::from(self.min_gain)) {
            Verdict::Keep
        } else {
            Verdict::Withdraw
        }
    }
}

#[derive(Debug, Clone)]
struct Watch {
    anomaly: u32,
    key: Key,
    t0: Instant,
    seen: Seen,
}

/// The watches of every ramp-opened anomaly, and the ones withdrawn. Pure: a function of the
/// readings fed to it, in order, and of the instants it is asked at.
#[derive(Debug, Clone)]
pub struct Follower {
    spec: FollowSpec,
    watches: Vec<Watch>,
    withdrawn: BTreeSet<u32>,
    opened: u64,
}

impl Follower {
    /// A follower with no watch.
    pub fn new(spec: FollowSpec) -> Self {
        Self {
            spec,
            watches: Vec::new(),
            withdrawn: BTreeSet::new(),
            opened: 0,
        }
    }

    /// Watches opened so far.
    pub fn opened(&self) -> u64 {
        self.opened
    }

    /// Watches still undecided.
    pub fn watching(&self) -> usize {
        self.watches.len()
    }

    /// Start watching `anomaly`, whose ramp at `key` was completed by a reading of `level0` at `t0`.
    pub fn open(&mut self, anomaly: u32, key: Key, level0: u64, t0: Instant) {
        self.opened += 1;
        self.watches.push(Watch {
            anomaly,
            key,
            t0,
            seen: Seen {
                level0,
                readings: 0,
                peak: level0,
                last: level0,
            },
        });
    }

    /// A reading of `value` at `key`. Every undecided watch of the key takes it in and may decide.
    /// Returns the verdicts made, by anomaly, in the order the watches were opened.
    pub fn feed(&mut self, key: Key, value: u64) -> Vec<(u32, Verdict)> {
        let spec = self.spec;
        let mut out = Vec::new();
        for w in self.watches.iter_mut().filter(|w| w.key == key) {
            w.seen.readings += 1;
            w.seen.peak = w.seen.peak.max(value);
            w.seen.last = value;
            if let Some(v) = spec.judge(&w.seen) {
                out.push((w.anomaly, v));
            }
        }
        self.conclude(&out);
        out
    }

    /// The watches whose horizon has passed at `now` decide with what they have.
    pub fn expire(&mut self, now: Instant) -> Vec<(u32, Verdict)> {
        let spec = self.spec;
        let out: Vec<(u32, Verdict)> = self
            .watches
            .iter()
            .filter(|w| now.0 >= w.t0.0.saturating_add(spec.horizon_ns))
            .map(|w| (w.anomaly, spec.settle(&w.seen)))
            .collect();
        self.conclude(&out);
        out
    }

    fn conclude(&mut self, verdicts: &[(u32, Verdict)]) {
        for (anomaly, v) in verdicts {
            self.watches.retain(|w| w.anomaly != *anomaly);
            if *v == Verdict::Withdraw {
                self.withdrawn.insert(*anomaly);
            }
        }
    }

    /// Forget what is known of every anomaly `live` says is no longer tracked (quiet and retired,
    /// or taken into another): its watch, and its withdrawal once the rung has retired it.
    pub fn retain(&mut self, live: impl Fn(u32) -> bool) {
        self.watches.retain(|w| live(w.anomaly));
        self.withdrawn.retain(|a| live(*a));
    }

    /// Whether the rule has withdrawn `anomaly` and the rung has not yet retired it.
    pub fn is_withdrawn(&self, anomaly: u32) -> bool {
        self.withdrawn.contains(&anomaly)
    }

    /// The anomalies withdrawn and not yet retired, ascending.
    pub fn withdrawn(&self) -> impl Iterator<Item = u32> + '_ {
        self.withdrawn.iter().copied()
    }
}
