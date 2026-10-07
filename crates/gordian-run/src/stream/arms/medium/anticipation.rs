//! The anticipation layer of the medium noticer (work item A2, Lab 1): pair cells
//! ([`gordian_medium::anticipation`]) that learn, from the arm's own stream, which service's
//! alarms keep following which other service's alarms where the public graph does not connect
//! them, and a prediction made from such a learned edge.
//!
//! Everything in this documentation, the rule and the first values above all, was written and
//! committed before any run of the layer (`DESIGN.md` of `gordian-medium`, "Anticipation of hidden
//! edges (A2)", committed before this code). A change after a run is recorded in
//! `experiments/exploration/a2-anticipation.md` with its reason, never edited in here silently.
//!
//! # What the layer is
//!
//! A medium beside the noticing graph (and the engram layer, if any), held by the medium noticer
//! ([`super::noticing::MediumNoticer`]) when its parameters name `anticipation`, on the same tick,
//! fed every delivered observation in id order at the step that delivers it ([`Layer::take`]),
//! ticked after the noticing graph at each step ([`Layer::run_ticks`]), and charged to the arm's
//! bill with the noticing graph's. Its cells are the pair cells of every ordered pair of services
//! the public graph does not connect, three bands each. **Nothing carries across segments**: the
//! layer is built with the noticer at a segment's start and dropped at its end. Absent from the
//! parameters, the medium noticer is what it was, byte for byte.
//!
//! # The public readings
//!
//! - **First alarm**: an abnormal observation (the public rules' verdict) at a service with none at
//!   it in the previous `burst_gap_ns` (2 s): A1c's rule.
//! - **In burst**: a service whose latest abnormal observation is less than `burst_gap_ns` before
//!   the instant; its next one would not be a first alarm.
//! - **Unconnected**: distinct, and neither a transitive dependent of the other in the time-zero
//!   graph (`dependents_mask`).
//! - **Explained**: a first alarm at `s` at `t` is explained when a service `u` of which `s` is a
//!   transitive dependent had a first alarm in `[t - burst_ns, t]` (0.4 s), among those delivered
//!   before it: the rung's propagation rule.
//!
//! # The rule (fixed before any run)
//!
//! With `explained` on (the main form), a first alarm is **counted** when it is unexplained; off
//! (a labelled control), every first alarm is counted.
//!
//! 1. **Follow.** A counted first alarm at `b` at `t_b` credits, for every `a` and band `k`, the
//!    latest open trial `(a, b, k, t_a)` with `t_a < t_b <= t_a + w_k`: evidence
//!    `+gain * (1 - q)` to the pair cell `(a, b, k)` at `t_b`, with `q` the trial's chance (below).
//! 2. **Trial.** A counted first alarm at `a` at `t_a` opens, for every `b` unconnected to `a` and
//!    not in burst at `t_a`, one trial per band, with its **chance** `q = chance(rate_b, w_k)`
//!    (`1 - exp(-rate_b * w_k)`); a trial whose window closes without a follow is a **miss**:
//!    evidence `-gain * q` at its deadline. The level counts follows beyond chance (departure 83:
//!    the design weighted a follow 1 and a miss `q / (1 - q)`). `rate_b` is the rate
//!    at which `b`'s counted first alarms began **while `b` was quiet** (not in burst) before
//!    `t_a`, per second of `b`'s quiet time, under the rung scorer's prior:
//!    `(n_b + prior_mhz / 1000 * prior_s) / (quiet_b_s + prior_s)`. A trial is opened only while
//!    `b` is quiet, so this is the rate behind the chance of a follow; a pair whose follows come at
//!    that chance drifts by zero. (Departure 82: the design divided by the elapsed time, which
//!    counts `b`'s busy time too and so underestimates the chance; found by analysis before any
//!    run.)
//! 3. **Learned edge.** The layer holds `a -> b` in band `k` when the pair cell's level, read at
//!    the layer's next tick, is at least `threshold`.
//! 4. **Prediction.** At a trial at `a`, before its trials open, for each partner `b` (unconnected,
//!    not in burst: no alarm at `b` yet) with an edge held: the prediction `(a, b, w_k)` in the
//!    narrowest band held, with the step's instant. It goes to the trace only and changes nothing
//!    the arm does. It is resolved on the public side: `followed` at the first first alarm at `b`
//!    (counted or not) in `(t_a, t_a + w_k]`, else `expired` at the deadline.
//!
//! Evidence events enter the pair-cell medium at their instant's tick, or at the next tick to run
//! when that tick has run already. Every partner's three bands are read at every trial, whatever
//! the trace switch, so that the trace never changes what is charged.
//!
//! # Use: the attach switch (item 3; default off)
//!
//! With `attach` on, an abnormal observation at `b` at `t` that the rung's attach rule gives to no
//! anomaly, or only by its last rule (`b`'s own stale anomaly), is attached instead to the anomaly
//! sited at a service `a` with an edge `a -> b` held in band `k` whose current burst began in
//! `[t - w_k, t]`; if several, the latest burst, then the later anomaly ([`Layer::attach_target`],
//! used by the noticer where it attaches). The rung's first two rules are never overridden.
//!
//! # Parameters (first values, fixed before any run; reasons in `DESIGN.md`)
//!
//! | Parameter | First value |
//! |---|---|
//! | `bands_ns` | 0.4 s, 2 s, 10 s |
//! | `gain` | 1 |
//! | `threshold` | 2 |
//! | `tau_ns` | 150 s |
//! | `explained` | on |
//! | `attach` | off |
//! | miss weight prior | the rung's `prior_mhz` (100) over `prior_ns` (30 s) |
//!
//! # The trace
//!
//! With `trace` on and [`super::trace::TRACE_DIR_ENV`] naming a directory, the layer's marks are
//! appended at the end of each segment to `<dir>/anticipation-trace-<trace_key>.csv`
//! ([`FILE_HEADER`]; the kinds are [`Kind`]). Output only: nothing is read back.
//!
//! # What this is not
//!
//! It is not told whether an edge is real: the stream never says. A learned edge is a statement
//! about the arm's own history of public alarms; whether it is a hidden edge is scored from the
//! hidden side (W3).

use super::adapters::{TickClock, TickLedger};
use super::trace::{NONE, TRACE_DIR_ENV};
use crate::stream::arms::noticer::Tracked;
use crate::stream::arms::rung::{Held, RungConfig, service_of};
use gordian_core::Instant;
use gordian_medium::{
    ConstantField, DiscardingEffector, Evidence, Field, Limits, Mark, MarkLog, Medium,
    NoPlasticity, OpCounts, PairCells, PairParams, Ports, Prices, ScriptedSense, Trace, Trials,
    chance,
};
use gordian_world::graph::dependents_mask;
use gordian_world::{Service, ServiceId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The anticipation layer's parameters, in the medium noticer's `anticipation` field. See the
/// module documentation for each value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnticipationConfig {
    /// Names the layer's trace file; nothing is carried under it.
    pub trace_key: u64,
    /// The bands' windows, nanoseconds, strictly ascending.
    #[serde(default = "bands")]
    pub bands_ns: [u64; 3],
    /// The scale of the evidence: a follow at chance `q` adds `gain * (1 - q)`, a miss takes
    /// `gain * q`.
    #[serde(default = "one")]
    pub gain: f32,
    /// The level at which an edge is held.
    #[serde(default = "two")]
    pub threshold: f32,
    /// The pair cell's decay time constant, nanoseconds.
    #[serde(default = "tau")]
    pub tau_ns: u64,
    /// Count only first alarms the public graph does not explain (off: a labelled control).
    #[serde(default = "yes")]
    pub explained: bool,
    /// The attach switch (item 3).
    #[serde(default)]
    pub attach: bool,
    /// Write the trace file.
    #[serde(default)]
    pub trace: bool,
}

fn bands() -> [u64; 3] {
    [400_000_000, 2_000_000_000, 10_000_000_000]
}

fn one() -> f32 {
    1.0
}

fn two() -> f32 {
    2.0
}

fn tau() -> u64 {
    150_000_000_000
}

fn yes() -> bool {
    true
}

impl Default for AnticipationConfig {
    /// The first values (module documentation), trace off.
    fn default() -> Self {
        Self {
            trace_key: 0,
            bands_ns: bands(),
            gain: one(),
            threshold: two(),
            tau_ns: tau(),
            explained: true,
            attach: false,
            trace: false,
        }
    }
}

impl AnticipationConfig {
    /// The crate's parameters.
    pub fn params(&self) -> PairParams {
        PairParams {
            domain: super::adapters::DOMAIN,
            bands_ns: self.bands_ns.to_vec(),
            gain: self.gain,
            threshold: self.threshold,
            tau_ns: self.tau_ns,
        }
    }

    /// Check the parameters for a tick of `tick_ns`, and that a store builds.
    pub fn validate(&self, tick_ns: u64) -> Result<(), String> {
        let err = |e: String| format!("noticer medium anticipation: {e}");
        self.params().validate().map_err(err)?;
        PairCells::build(
            self.params(),
            &[(0, 1)],
            tick_ns,
            Limits::default(),
            Prices::DECLARED,
        )
        .map_err(err)?;
        Ok(())
    }
}

/// The header of a trace file.
pub const FILE_HEADER: &str = "segment,at_ns,kind,service,obs,partner,band,value";

/// What a mark records (its `kind`). Subject: a service; event: an observation id; tag:
/// `partner << 8 | band`; value: per kind (module documentation of `DESIGN.md`, "The trace").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Kind {
    /// A first alarm; value 1 counted, 0 not (explained).
    FirstAlarm = 1,
    /// An edge held at a trial, per band held; value the level in thousandths.
    Held = 2,
    /// A prediction; value the anomaly owning the alarm, -1 if none.
    Prediction = 3,
    /// A prediction followed on the public side; value the partner's alarm.
    Followed = 4,
    /// A prediction not followed; at its deadline.
    Expired = 5,
    /// Evidence: a follow; value in thousandths.
    Follow = 6,
    /// Evidence: a miss; value in thousandths.
    Miss = 7,
    /// A pair cell's level at the segment's end, thousandths.
    LevelEnd = 8,
    /// The segment's end; value the number of pair cells.
    SegmentEnd = 9,
}

impl Kind {
    /// Every kind, in code order.
    pub const ALL: [Kind; 9] = [
        Kind::FirstAlarm,
        Kind::Held,
        Kind::Prediction,
        Kind::Followed,
        Kind::Expired,
        Kind::Follow,
        Kind::Miss,
        Kind::LevelEnd,
        Kind::SegmentEnd,
    ];

    /// The kind of a code.
    pub fn of(code: u16) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| *k as u16 == code)
    }

    /// The name written to the file.
    pub fn name(self) -> &'static str {
        match self {
            Kind::FirstAlarm => "first_alarm",
            Kind::Held => "held",
            Kind::Prediction => "prediction",
            Kind::Followed => "followed",
            Kind::Expired => "expired",
            Kind::Follow => "follow",
            Kind::Miss => "miss",
            Kind::LevelEnd => "level_end",
            Kind::SegmentEnd => "segment_end",
        }
    }
}

/// The tag of a mark naming `partner` and `band`.
fn pair_tag(partner: u32, band: usize) -> u32 {
    (partner << 8) | band as u32
}

/// Thousandths of `x`, rounded toward zero (for the trace only).
fn milli(x: f32) -> i64 {
    (f64::from(x) * 1000.0) as i64
}

/// One row of a trace file for `mark` in segment `segment`.
pub fn row(segment: u64, mark: &Mark) -> String {
    let id = |x: u32| {
        if x == NONE {
            String::new()
        } else {
            x.to_string()
        }
    };
    let (partner, band) = if mark.tag == NONE {
        (String::new(), String::new())
    } else {
        ((mark.tag >> 8).to_string(), (mark.tag & 0xFF).to_string())
    };
    format!(
        "{segment},{},{},{},{},{partner},{band},{}",
        mark.at_ns,
        Kind::of(mark.kind).map_or("unknown", Kind::name),
        id(mark.subject),
        id(mark.event),
        mark.value,
    )
}

/// What the layer counted over the segment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayerStats {
    /// First alarms seen.
    pub first_alarms: u64,
    /// Of those, counted (unexplained, or all with `explained` off).
    pub counted: u64,
    /// Trials opened (one per partner and band).
    pub trials: u64,
    /// Trials followed.
    pub follows: u64,
    /// Trials missed.
    pub misses: u64,
    /// Predictions made.
    pub predictions: u64,
    /// Of those, followed on the public side.
    pub followed: u64,
    /// Of those, expired.
    pub expired: u64,
    /// Observations the attach switch gave to an anomaly the rung's rule would not have.
    pub attached: u64,
    /// Ticks the pair-cell medium refused (a defect: should be none).
    pub step_errors: u64,
}

/// A prediction waiting for its public resolution.
#[derive(Debug, Clone, Copy)]
struct Open {
    a: u32,
    obs: u32,
    b: u32,
    band: usize,
    at_ns: u64,
    deadline_ns: u64,
}

/// The anticipation layer. See the module documentation.
pub struct Layer {
    cfg: AnticipationConfig,
    tick_ns: u64,
    medium: Medium,
    cells: PairCells,
    trials: Trials,
    sense: ScriptedSense,
    ledger: TickLedger,
    prices: Prices,
    next_tick: u64,
    seq: u32,
    stopped: bool,
    /// `unconnected[a]`: the services unconnected to `a`, ascending.
    unconnected: Vec<Vec<u32>>,
    /// `upstream[s][u]`: `s` is a transitive dependent of `u`.
    upstream: Vec<Vec<bool>>,
    burst_ns: u64,
    burst_gap_ns: u64,
    prior_per_s: f64,
    prior_s: f64,
    /// The latest abnormal observation per service.
    last_abnormal: BTreeMap<u32, u64>,
    /// Per service, the time in burst between its abnormal observations so far (each gap counted
    /// up to `burst_gap_ns`), nanoseconds.
    busy_closed: BTreeMap<u32, u64>,
    /// First alarms of the last `burst_ns`: service and instant.
    recent: VecDeque<(u32, u64)>,
    /// Counted first alarms per service so far.
    counted: BTreeMap<u32, u64>,
    open: Vec<Open>,
    stats: LayerStats,
    log: MarkLog,
    segment: u64,
}

impl std::fmt::Debug for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("anticipation::Layer")
            .field("cfg", &self.cfg)
            .field("pairs", &self.cells.pairs().len())
            .field("stats", &self.stats)
            .finish()
    }
}

impl Layer {
    /// A layer for the public graph `services`, a tick of `tick_ns` and the rung's constants
    /// `rung`: pair cells for every unconnected ordered pair.
    pub fn new(
        cfg: AnticipationConfig,
        tick_ns: u64,
        services: &[Service],
        rung: &RungConfig,
    ) -> Result<Self, String> {
        cfg.validate(tick_ns)?;
        let n = services.len();
        let masks: Vec<Vec<bool>> = services
            .iter()
            .map(|s| dependents_mask(services, s.id))
            .collect();
        let dep = |a: usize, b: usize| masks[a].get(b).copied().unwrap_or(false);
        let unconnected: Vec<Vec<u32>> = (0..n)
            .map(|a| {
                (0..n)
                    .filter(|&b| b != a && !dep(a, b) && !dep(b, a))
                    .map(|b| b as u32)
                    .collect()
            })
            .collect();
        let upstream = (0..n)
            .map(|s| (0..n).map(|u| dep(u, s)).collect())
            .collect();
        let pairs: Vec<(u16, u16)> = unconnected
            .iter()
            .enumerate()
            .flat_map(|(a, bs)| bs.iter().map(move |&b| (a as u16, b as u16)))
            .collect();
        let (medium, cells) = PairCells::build(
            cfg.params(),
            &pairs,
            tick_ns,
            Limits::default(),
            Prices::DECLARED,
        )
        .map_err(|e| format!("noticer medium anticipation: {e}"))?;
        Ok(Self {
            prices: *medium.prices(),
            trials: Trials::new(cfg.bands_ns.to_vec()),
            cfg,
            tick_ns,
            medium,
            cells,
            sense: ScriptedSense::default(),
            ledger: TickLedger::default(),
            next_tick: 0,
            seq: 0,
            stopped: false,
            unconnected,
            upstream,
            burst_ns: rung.burst_ns,
            burst_gap_ns: rung.burst_gap_ns,
            prior_per_s: rung.prior_mhz as f64 / 1000.0,
            prior_s: rung.prior_ns as f64 / 1.0e9,
            last_abnormal: BTreeMap::new(),
            busy_closed: BTreeMap::new(),
            recent: VecDeque::new(),
            counted: BTreeMap::new(),
            open: Vec::new(),
            stats: LayerStats::default(),
            log: MarkLog::default(),
            segment: if cfg.trace {
                next_segment(cfg.trace_key)
            } else {
                0
            },
        })
    }

    /// The parameters.
    pub fn config(&self) -> &AnticipationConfig {
        &self.cfg
    }

    /// The pair-cell medium.
    pub fn medium(&self) -> &Medium {
        &self.medium
    }

    /// The pair cells.
    pub fn cells(&self) -> &PairCells {
        &self.cells
    }

    /// What the layer counted.
    pub fn stats(&self) -> &LayerStats {
        &self.stats
    }

    /// The marks written so far in this segment (empty unless `trace`).
    pub fn marks(&self) -> &[Mark] {
        &self.log.marks
    }

    /// The ledger port: the pair-cell medium's ticks and counts.
    pub fn ledger(&self) -> &TickLedger {
        &self.ledger
    }

    /// The services unconnected to `a`, ascending.
    pub fn unconnected(&self, a: u32) -> &[u32] {
        self.unconnected
            .get(a as usize)
            .map_or(&[], |v| v.as_slice())
    }

    fn mark(&mut self, kind: Kind, at_ns: u64, subject: u32, event: u32, tag: u32, value: i64) {
        if self.cfg.trace {
            self.log.mark(Mark {
                at_ns,
                kind: kind as u16,
                subject,
                event,
                tag,
                value,
            });
        }
    }

    /// Put one resolved trial into the medium as an evidence event, and mark it.
    fn push(&mut self, e: Evidence) {
        let t = e.trial;
        if let Some(ev) = self.cells.evidence_event(
            (t.a, t.b, t.band),
            e.value,
            e.at_ns,
            self.next_tick,
            self.seq,
        ) {
            self.seq = self.seq.wrapping_add(1);
            self.sense.by_tick.entry(ev.tick).or_default().push(ev);
        }
        let kind = if e.follow {
            self.stats.follows += 1;
            Kind::Follow
        } else {
            self.stats.misses += 1;
            Kind::Miss
        };
        self.mark(
            kind,
            e.at_ns,
            u32::from(t.a),
            t.token,
            pair_tag(u32::from(t.b), t.band),
            milli(e.value),
        );
    }

    /// Resolve every trial and prediction whose window closed before `before_ns`.
    fn expire(&mut self, before_ns: u64) {
        for e in self.trials.expire(before_ns) {
            self.push(e);
        }
        let (gone, keep): (Vec<Open>, Vec<Open>) = std::mem::take(&mut self.open)
            .into_iter()
            .partition(|p| p.deadline_ns < before_ns);
        self.open = keep;
        for p in gone {
            self.stats.expired += 1;
            self.mark(
                Kind::Expired,
                p.deadline_ns,
                p.a,
                p.obs,
                pair_tag(p.b, p.band),
                0,
            );
        }
    }

    /// The time `b` was quiet (not in burst) in `[0, at_ns]`, nanoseconds.
    pub fn quiet_ns(&self, b: u32, at_ns: u64) -> u64 {
        let closed = self.busy_closed.get(&b).copied().unwrap_or(0);
        let open = self
            .last_abnormal
            .get(&b)
            .map_or(0, |l| at_ns.saturating_sub(*l).min(self.burst_gap_ns));
        at_ns.saturating_sub(closed.saturating_add(open))
    }

    /// The rate at which `b`'s counted first alarms began per second of its quiet time before
    /// `at_ns`, under the prior.
    fn rate(&self, b: u32, at_ns: u64) -> f64 {
        let n = self.counted.get(&b).copied().unwrap_or(0) as f64;
        let quiet = self.quiet_ns(b, at_ns) as f64 / 1.0e9;
        (n + self.prior_per_s * self.prior_s) / (quiet + self.prior_s)
    }

    /// Take in one delivered observation, at the step of instant `now`; `owner` is the noticer's
    /// anomaly that owns it, if any (for the prediction's trace row).
    pub fn take(&mut self, held: &Held, now: Instant, owner: Option<u32>) {
        if self.stopped || !held.abnormal {
            return;
        }
        let Some(ServiceId(s)) = service_of(&held.obs) else {
            return;
        };
        let t = held.at.0;
        self.expire(t);
        let first = self
            .last_abnormal
            .get(&s)
            .is_none_or(|l| t >= l.saturating_add(self.burst_gap_ns));
        if let Some(l) = self.last_abnormal.insert(s, t) {
            *self.busy_closed.entry(s).or_insert(0) += t.saturating_sub(l).min(self.burst_gap_ns);
        }
        if !first {
            return;
        }
        self.stats.first_alarms += 1;
        while self
            .recent
            .front()
            .is_some_and(|(_, at)| at.saturating_add(self.burst_ns) < t)
        {
            self.recent.pop_front();
        }
        let explained = self.recent.iter().any(|&(u, _)| {
            u != s
                && self
                    .upstream
                    .get(s as usize)
                    .and_then(|r| r.get(u as usize))
                    .copied()
                    .unwrap_or(false)
        });
        self.recent.push_back((s, t));
        let counted = !self.cfg.explained || !explained;
        let obs = held.id.0;
        self.mark(Kind::FirstAlarm, t, s, obs, NONE, i64::from(counted));
        // Predictions about `s`, on the public side: any first alarm resolves them.
        let (hit, keep): (Vec<Open>, Vec<Open>) = std::mem::take(&mut self.open)
            .into_iter()
            .partition(|p| p.b == s && p.at_ns < t && t <= p.deadline_ns);
        self.open = keep;
        for p in hit {
            self.stats.followed += 1;
            self.mark(
                Kind::Followed,
                t,
                p.a,
                p.obs,
                pair_tag(p.b, p.band),
                i64::from(obs),
            );
        }
        if !counted {
            return;
        }
        self.stats.counted += 1;
        // `s` as the partner: follows.
        for e in self.trials.follow(s as u16, t, obs) {
            self.push(e);
        }
        // `s` as the first: read every partner's cells, predict, open trials.
        let tick = self.next_tick;
        let partners = self.unconnected(s).to_vec();
        for b in partners {
            let levels: Vec<f32> = (0..self.cells.bands())
                .map(|k| {
                    self.cells
                        .level(&self.medium, s as u16, b as u16, k, tick)
                        .unwrap_or(0.0)
                })
                .collect();
            let threshold = self.cfg.threshold;
            for (k, l) in levels.iter().enumerate() {
                if *l >= threshold {
                    self.mark(Kind::Held, now.0, s, obs, pair_tag(b, k), milli(*l));
                }
            }
            let in_burst = self
                .last_abnormal
                .get(&b)
                .is_some_and(|l| t < l.saturating_add(self.burst_gap_ns));
            if in_burst {
                continue;
            }
            if let Some(k) = levels.iter().position(|l| *l >= threshold) {
                self.stats.predictions += 1;
                self.mark(
                    Kind::Prediction,
                    now.0,
                    s,
                    obs,
                    pair_tag(b, k),
                    owner.map_or(-1, i64::from),
                );
                self.open.push(Open {
                    a: s,
                    obs,
                    b,
                    band: k,
                    at_ns: t,
                    deadline_ns: t.saturating_add(self.cfg.bands_ns[k]),
                });
            }
            let rate = self.rate(b, t);
            let gain = self.cfg.gain;
            let weights: Vec<(f32, f32)> = self
                .cfg
                .bands_ns
                .iter()
                .map(|w| {
                    let q = chance(rate, *w);
                    (gain * (1.0 - q), gain * q)
                })
                .collect();
            self.trials.open(s as u16, b as u16, t, obs, &weights);
            self.stats.trials += self.cfg.bands_ns.len() as u64;
        }
        *self.counted.entry(s).or_insert(0) += 1;
    }

    /// Resolve what closed before `now` and run every tick complete at `now`.
    pub fn run_ticks(&mut self, now: Instant) {
        if self.stopped {
            return;
        }
        self.expire(now.0);
        let complete = TickClock::complete_before(self.tick_ns, now);
        let mut field = ConstantField(Field::default());
        let mut plasticity = NoPlasticity;
        let mut effector = DiscardingEffector::default();
        while self.next_tick < complete && !self.stopped {
            let mut clock = TickClock {
                tick: self.next_tick,
                tick_ns: self.tick_ns,
            };
            let mut ports = Ports {
                clock: &mut clock,
                sense: &mut self.sense,
                field: &mut field,
                effector: &mut effector,
                ledger: &mut self.ledger,
                trace: &mut self.log,
                plasticity: &mut plasticity,
            };
            if self.medium.step(&mut ports).is_err() {
                self.stats.step_errors += 1;
                self.stopped = true;
            }
            self.next_tick += 1;
        }
    }

    /// The narrowest band in which the layer holds `a -> b` now (its next tick), if any.
    pub fn held(&mut self, a: u32, b: u32) -> Option<usize> {
        let (Ok(a), Ok(b)) = (u16::try_from(a), u16::try_from(b)) else {
            return None;
        };
        if !self.cells.has(a, b) {
            return None;
        }
        self.cells.held(&self.medium, a, b, self.next_tick)
    }

    /// The attach switch's target for an abnormal observation at `service` at `at` (module
    /// documentation, "Use"): the anomaly sited at a service with an edge to `service` held in
    /// band `k` whose current burst began in `[at - w_k, at]`; the latest burst, then the later
    /// anomaly. `None` when the switch is off or none qualifies. Counted in the stats when found.
    pub fn attach_target(
        &mut self,
        anomalies: &[Tracked],
        service: ServiceId,
        at: Instant,
    ) -> Option<usize> {
        if !self.cfg.attach || self.stopped {
            return None;
        }
        let mut best: Option<(Instant, usize)> = None;
        for (i, a) in anomalies.iter().enumerate() {
            if a.site == service {
                continue;
            }
            let Some(k) = self.held(a.site.0, service.0) else {
                continue;
            };
            let open = a.burst_open_at();
            if at.0 < open.0 || at.0 > open.0.saturating_add(self.cfg.bands_ns[k]) {
                continue;
            }
            if best.is_none_or(|(o, _)| open.0 >= o.0) {
                best = Some((open, i));
            }
        }
        let found = best.map(|(_, i)| i);
        if found.is_some() {
            self.stats.attached += 1;
        }
        found
    }

    /// Stop: no more ticks or evidence (the arm's bill refused a charge).
    pub fn refused(&mut self) {
        self.stopped = true;
    }

    /// The modelled cost of the work since the last call, nanoseconds, and forget it: the
    /// pair-cell medium's counts and ticks at the declared prices plus the level reads. `None`
    /// when there was none.
    pub fn take_ns(&mut self) -> Option<u64> {
        let reads: OpCounts = self.cells.take_work();
        let ticks = self.ledger.take_ns(&self.prices);
        let read_ns = reads.modelled_ps(&self.prices) / 1_000;
        match (ticks, read_ns) {
            (None, 0) => None,
            (t, r) => Some(t.unwrap_or(0) + r),
        }
    }

    /// The modelled cost of the layer's ticks over the segment, nanoseconds (reads not included).
    pub fn total_ns(&self) -> u64 {
        self.ledger.total_ns(&self.prices)
    }
}

impl Drop for Layer {
    /// The end of the segment: with `trace` on, every pair cell's level, a `segment_end` mark,
    /// and the segment's marks appended to the arm's trace file.
    fn drop(&mut self) {
        if !self.cfg.trace {
            return;
        }
        let at = self.next_tick.saturating_mul(self.tick_ns);
        let pairs = self.cells.pairs().to_vec();
        for (a, b) in pairs {
            for k in 0..self.cells.bands() {
                let l = self
                    .cells
                    .level(&self.medium, a, b, k, self.next_tick)
                    .unwrap_or(0.0);
                self.mark(
                    Kind::LevelEnd,
                    at,
                    u32::from(a),
                    NONE,
                    pair_tag(u32::from(b), k),
                    milli(l),
                );
            }
        }
        let n = (self.cells.pairs().len() * self.cells.bands()) as i64;
        self.mark(Kind::SegmentEnd, at, NONE, NONE, NONE, n);
        append(self.cfg.trace_key, self.segment, &self.log);
    }
}

/// Segments begun per trace key in this process (the trace's segment ordinal).
static SEGMENTS: Mutex<BTreeMap<u64, u64>> = Mutex::new(BTreeMap::new());

/// Trace files that could not be written, in this process.
static WRITE_ERRORS: AtomicU64 = AtomicU64::new(0);

/// The ordinal of a new segment of the layer keyed `key`: 0 for the first in the process.
pub fn next_segment(key: u64) -> u64 {
    let mut map = SEGMENTS.lock().unwrap_or_else(PoisonError::into_inner);
    let n = map.entry(key).or_insert(0);
    let out = *n;
    *n += 1;
    out
}

/// Forget the segment count of `key` (tests).
pub fn reset_segments(key: u64) {
    SEGMENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
}

/// Trace files that could not be written so far in this process.
pub fn write_errors() -> u64 {
    WRITE_ERRORS.load(Ordering::Relaxed)
}

/// The path of the trace file of the layer keyed `key` in `dir`.
pub fn file_in(dir: &std::path::Path, key: u64) -> std::path::PathBuf {
    dir.join(format!("anticipation-trace-{key}.csv"))
}

/// Append the marks of `log` for segment `segment` of the layer keyed `key` to its trace file in
/// the directory [`TRACE_DIR_ENV`] names. Nothing when the variable is unset or empty.
pub fn append(key: u64, segment: u64, log: &MarkLog) {
    if let Some(dir) = std::env::var_os(TRACE_DIR_ENV).filter(|d| !d.is_empty()) {
        append_to(std::path::Path::new(&dir), key, segment, log);
    }
}

/// Append the marks of `log` for segment `segment` to the trace file of the layer keyed `key` in
/// `dir`, writing the header when the file is new. A failure is counted ([`write_errors`]).
pub fn append_to(dir: &std::path::Path, key: u64, segment: u64, log: &MarkLog) {
    let path = file_in(dir, key);
    let mut text = String::new();
    if std::fs::metadata(&path).map_or(true, |m| m.len() == 0) {
        text.push_str(FILE_HEADER);
        text.push('\n');
    }
    for m in &log.marks {
        text.push_str(&row(segment, m));
        text.push('\n');
    }
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(text.as_bytes()))
    });
    if written.is_err() {
        WRITE_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}
