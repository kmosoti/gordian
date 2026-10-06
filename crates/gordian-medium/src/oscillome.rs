//! The oscillome (docs/medium-ports.md section 4b; work item M1b): the nested oscillations the
//! medium keeps beside its base tick, the seconds-to-ticks conversions done once at build time,
//! and the Oscillome Engine that computes phases, finds cycle boundaries and keeps a summary per
//! cycle.
//!
//! The names label a mechanism. Nothing here claims that the rhythms help the medium notice
//! anything; the ablation in M2's follow-up decides that (section 4b, "Where the analogy breaks").
//!
//! # The oscillations (decision 1)
//!
//! The base tick is the fastest oscillation, whatever its length; [`Oscillome::periods_ns`] lists
//! the slower ones only (first set: 10 s and 100 s), at most [`R`], each longer than the tick.
//!
//! # Phases and boundaries (decision 3)
//!
//! For a rhythm of period `P` ns on a tick of `L` ns, with `x = tick * L` computed in `u128`:
//!
//! - the **cycle index** of a tick is `floor(x / P)`, the cycle that contains the tick's start;
//! - the **phase** is `q / 2^24` with `q = floor((x mod P) * 2^24 / P)`, all in integers, then
//!   one exact conversion into `f32` (`q < 2^24` is exact, and the division by `2^24` is a
//!   change of exponent). So the phase is a pure function of the tick and the two lengths, lies in
//!   `[0, 1 - 2^-24]`, and is never rounded up to 1. Its resolution is `P / 2^24` (6 ns at 100 s);
//! - the **boundary tick** of a cycle is the tick in which the cycle index changes: the last
//!   tick whose start lies in the cycle, `cycle(t + 1) != cycle(t)`. Boundary work (the cycle
//!   summary, scheduled plasticity, scheduled trace samples) happens at the end of that tick, so a
//!   summary covers exactly the ticks whose start lies in its cycle. When `P` is not a multiple of
//!   `L` the cycles hold `floor(P / L)` or `ceil(P / L)` ticks ([`OscillomeEngine::ticks_per_cycle`]).
//!
//! # Seconds to ticks (decision 2)
//!
//! A [`Timed`] entry gives one time-like quantity in nanoseconds and names the parameter or
//! synapse delay it sets. At build time ([`crate::MediumSpec::resolved`]) each is converted by its
//! [`TimeKind`]: delays to the nearest tick, half up, at least one tick (a zero-tick delay, "next
//! pass", is never produced); lookbacks, windows, holds and refractory periods up (ceiling);
//! decays given as a time constant `tau` to `exp(-L / tau)` per tick, and rates to
//! `1 - exp(-L / tau)`, by [`exp_det`] in `f64`, rounded once to `f32`. Offset windows of the
//! ordered coincidence are microseconds and do not depend on the tick. The resulting
//! [`Conversion`]s are the table the report carries.

use serde::{Deserialize, Serialize};

use crate::archetype::{Archetype, MAX_INT_PARAM};
use crate::medium::TickSummary;
use crate::spec::{CellSpec, SpecError, SynapseSpec};
use crate::types::{CellId, OpCounts, P, R, SynapseId};

/// `2^24`, the denominator of a phase.
const PHASE_ONE: u128 = 1 << 24;

/// The spec of the oscillome: the tick length, the slower rhythms, the quantities given in
/// seconds, and the schedules. Every element is off in [`Oscillome::default`], and a medium whose
/// oscillome is off and that uses no oscillome form of a cell or gate is M1's medium, byte for
/// byte (decision 9).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oscillome {
    /// The base tick's length in nanoseconds, 0 when unknown (off). Required by rhythms, by
    /// quantities in seconds and by the ordered coincidence; when set, the medium refuses a tick
    /// whose clock reports another length. At most `u32::MAX` (about 4.29 s), the range of an
    /// event's `offset_ns`.
    #[serde(default)]
    pub tick_len_ns: u64,
    /// The slower rhythms' periods in nanoseconds, each longer than the tick, at most [`R`]. Their
    /// phases are broadcast in the field (`Field::phases`, same order) every tick.
    #[serde(default)]
    pub periods_ns: Vec<u64>,
    /// Quantities given in time and converted at build time.
    #[serde(default)]
    pub seconds: Vec<Timed>,
    /// Keep a summary per cycle of every rhythm (decision 6).
    #[serde(default)]
    pub cycle_summary: bool,
    /// Run the plasticity port at the boundaries of this rhythm, with that rhythm's cycle
    /// summary, instead of at the end of every tick. Requires `cycle_summary`.
    #[serde(default)]
    pub plasticity_rhythm: Option<u8>,
    /// Offer the trace port only the boundary ticks of this rhythm.
    #[serde(default)]
    pub trace_rhythm: Option<u8>,
}

impl Oscillome {
    /// Whether every element is off.
    pub fn is_off(&self) -> bool {
        *self == Oscillome::default()
    }
}

/// What a [`Timed`] entry sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TimeTarget {
    /// Parameter `index` of cell `cell`.
    Param {
        /// The cell.
        cell: CellId,
        /// The parameter's index.
        index: u8,
    },
    /// The delay of synapse `synapse`.
    Delay {
        /// The synapse.
        synapse: SynapseId,
    },
}

/// One quantity given in time: a duration, or a time constant for decays and rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timed {
    /// What it sets.
    pub target: TimeTarget,
    /// The duration or time constant, in nanoseconds.
    pub ns: u64,
}

/// How a quantity in time becomes a value in the tick's units. The kind is fixed by the target:
/// see [`time_kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeKind {
    /// Ticks, to the nearest, half up, at least one (synapse delays).
    Delay,
    /// Ticks, rounded up (lookbacks, windows, holds, refractory periods).
    Span,
    /// `exp(-tick / tau)` per tick (an integrator's leak).
    Decay,
    /// `1 - exp(-tick / tau)` per tick (a novelty cell's rate).
    Rate,
    /// Microseconds, rounded up, independent of the tick (an ordered coincidence's window).
    Micros,
}

/// One conversion done at build time: a row of the conversion table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conversion {
    /// What was set.
    pub target: TimeTarget,
    /// How.
    pub kind: TimeKind,
    /// The quantity in time, in nanoseconds.
    pub ns: u64,
    /// The tick length it was converted for.
    pub tick_len_ns: u64,
    /// The value written: ticks, microseconds or a per-tick factor.
    pub value: f32,
}

impl Conversion {
    /// The quantity in ticks before rounding, for display.
    pub fn exact_ticks(&self) -> f64 {
        self.ns as f64 / self.tick_len_ns as f64
    }

    /// Whether this is a delay converted to a single tick: every delay shorter than one and a
    /// half ticks (and every delay shorter than half a tick, which the one-tick minimum raises).
    pub fn collapsed_to_one_tick(&self) -> bool {
        self.kind == TimeKind::Delay && self.value == 1.0
    }
}

/// The kind of conversion a parameter takes, or `None` if the parameter is not a time. For the
/// coincidence window it depends on the mode (parameter 4): ticks in the sliding window, none in
/// the binned mode (its window counts bins), microseconds in the ordered mode.
pub fn time_kind(archetype: Archetype, index: usize, params: &[f32; P]) -> Option<TimeKind> {
    match (archetype, index) {
        (Archetype::Integrator, 0) => Some(TimeKind::Decay),
        (Archetype::Integrator, 3) => Some(TimeKind::Span),
        (Archetype::Novelty, 0) => Some(TimeKind::Rate),
        (Archetype::Coincidence, 1) => match params[4] as u8 {
            0 => Some(TimeKind::Span),
            2 => Some(TimeKind::Micros),
            _ => None,
        },
        (Archetype::Coincidence, 3) => Some(TimeKind::Span),
        (Archetype::Latch, 1) => Some(TimeKind::Span),
        (Archetype::Emit, 2 | 3) => Some(TimeKind::Span),
        _ => None,
    }
}

/// `ns` as ticks of `tick_len_ns`: nearest, half up, at least one.
pub fn delay_ticks(ns: u64, tick_len_ns: u64) -> u64 {
    let len = u128::from(tick_len_ns.max(1));
    let n = (u128::from(ns) + len / 2) / len;
    u64::try_from(n).unwrap_or(u64::MAX).max(1)
}

/// `ns` as ticks of `tick_len_ns`, rounded up.
pub fn span_ticks(ns: u64, tick_len_ns: u64) -> u64 {
    let len = u128::from(tick_len_ns.max(1));
    u64::try_from(u128::from(ns).div_ceil(len)).unwrap_or(u64::MAX)
}

/// The per-tick factor `exp(-tick / tau)`, in `f64` by [`exp_det`], rounded once to `f32`. A time
/// constant of zero gives 0.
pub fn decay_per_tick(tau_ns: u64, tick_len_ns: u64) -> f32 {
    if tau_ns == 0 {
        return 0.0;
    }
    exp_det(-(tick_len_ns as f64) / (tau_ns as f64)) as f32
}

/// The per-tick rate `1 - exp(-tick / tau)`, in `f64`, rounded once to `f32`. A time constant of
/// zero gives 1.
pub fn rate_per_tick(tau_ns: u64, tick_len_ns: u64) -> f32 {
    if tau_ns == 0 {
        return 1.0;
    }
    (1.0 - exp_det(-(tick_len_ns as f64) / (tau_ns as f64))) as f32
}

/// `exp(x)` from basic IEEE operations only, as `gordian-stream/src/rng.rs` computes it (copied:
/// this crate depends on `gordian-core` only). `x` is clamped to `[-700, 700]`; `x = k ln 2 + r`
/// with `|r| <= ln 2 / 2`, then 29 terms of the Taylor series of `exp(r)`, then a scaling by
/// `2^k`. Used at build time only; the tick never calls it.
pub fn exp_det(x: f64) -> f64 {
    const LN2: f64 = std::f64::consts::LN_2;
    let x = x.clamp(-700.0, 700.0);
    let k = (x / LN2).round();
    let r = x - k * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for i in 1..30 {
        term *= r / f64::from(i);
        sum += term;
    }
    let scale = f64::from_bits(((k as i64 + 1023) as u64) << 52);
    sum * scale
}

/// Nanoseconds in `seconds`, rounded to the nearest (a convenience for writing specs).
pub fn secs(seconds: f64) -> u64 {
    (seconds * 1.0e9).round() as u64
}

impl Oscillome {
    /// Check the oscillome's own structure (not the cells that refer to it).
    pub(crate) fn validate(&self) -> Result<(), SpecError> {
        let bad = |reason| Err(SpecError::BadOscillome(reason));
        if self.tick_len_ns > u64::from(u32::MAX) {
            return bad("tick_len_ns above u32::MAX, the range of offset_ns");
        }
        if self.periods_ns.len() > R {
            return bad("more rhythms than the field has phases (R)");
        }
        if !self.periods_ns.is_empty() && self.tick_len_ns == 0 {
            return bad("rhythms need tick_len_ns");
        }
        if self.periods_ns.iter().any(|p| *p <= self.tick_len_ns) {
            return bad("a rhythm's period must be longer than the tick (the tick is the fastest)");
        }
        if self.cycle_summary && self.periods_ns.is_empty() {
            return bad("cycle_summary needs a rhythm");
        }
        let n = self.periods_ns.len();
        if self.plasticity_rhythm.is_some_and(|r| usize::from(r) >= n) {
            return bad("plasticity_rhythm names no rhythm");
        }
        if self.plasticity_rhythm.is_some() && !self.cycle_summary {
            return bad("plasticity_rhythm needs cycle_summary");
        }
        if self.trace_rhythm.is_some_and(|r| usize::from(r) >= n) {
            return bad("trace_rhythm names no rhythm");
        }
        if !self.seconds.is_empty() && self.tick_len_ns == 0 {
            return bad("quantities in seconds need tick_len_ns");
        }
        Ok(())
    }

    /// Apply every [`Timed`] entry to `cells` and `synapses`, returning the conversions in entry
    /// order. Refuses a target that does not exist, is not a time, or is named twice, and a value
    /// that does not fit (a delay over 255 ticks, a span or window over `2^24`).
    pub(crate) fn apply(
        &self,
        cells: &mut [CellSpec],
        synapses: &mut [SynapseSpec],
    ) -> Result<Vec<Conversion>, SpecError> {
        let len = self.tick_len_ns;
        let mut targets: Vec<TimeTarget> = self.seconds.iter().map(|t| t.target).collect();
        targets.sort_unstable();
        if targets.windows(2).any(|w| w[0] == w[1]) {
            return Err(SpecError::BadTimed {
                index: 0,
                reason: "a target is named twice",
            });
        }
        let mut out = Vec::with_capacity(self.seconds.len());
        for (index, timed) in self.seconds.iter().enumerate() {
            let err = |reason| SpecError::BadTimed { index, reason };
            let (kind, value) = match timed.target {
                TimeTarget::Delay { synapse } => {
                    let s = synapses
                        .get_mut(synapse.0 as usize)
                        .ok_or(err("no such synapse"))?;
                    let ticks = delay_ticks(timed.ns, len);
                    s.delay_ticks = u8::try_from(ticks).map_err(|_| err("delay over 255 ticks"))?;
                    (TimeKind::Delay, ticks as f32)
                }
                TimeTarget::Param { cell, index: p } => {
                    let c = cells.get_mut(cell.0 as usize).ok_or(err("no such cell"))?;
                    let p = usize::from(p);
                    if p >= P {
                        return Err(err("no such parameter"));
                    }
                    let kind = time_kind(c.archetype, p, &c.params)
                        .ok_or(err("the parameter is not a time"))?;
                    let value = match kind {
                        TimeKind::Delay | TimeKind::Span => {
                            let t = span_ticks(timed.ns, len);
                            if t > MAX_INT_PARAM as u64 {
                                return Err(err("span over 2^24 ticks"));
                            }
                            t as f32
                        }
                        TimeKind::Micros => {
                            let us = timed.ns.div_ceil(1_000);
                            if us > MAX_INT_PARAM as u64 {
                                return Err(err("window over 2^24 microseconds"));
                            }
                            us as f32
                        }
                        TimeKind::Decay => decay_per_tick(timed.ns, len),
                        TimeKind::Rate => rate_per_tick(timed.ns, len),
                    };
                    c.params[p] = value;
                    (kind, value)
                }
            };
            out.push(Conversion {
                target: timed.target,
                kind,
                ns: timed.ns,
                tick_len_ns: len,
                value,
            });
        }
        Ok(out)
    }
}

/// The cycle index of `tick`: `floor(tick * tick_len_ns / period_ns)`, in integers.
pub fn cycle_index(tick: u64, tick_len_ns: u64, period_ns: u64) -> u64 {
    let x = u128::from(tick) * u128::from(tick_len_ns);
    u64::try_from(x / u128::from(period_ns.max(1))).unwrap_or(u64::MAX)
}

/// The phase of `tick` in `[0, 1)`: `floor((tick * tick_len_ns mod period_ns) * 2^24 /
/// period_ns) / 2^24`, in integers, then one exact conversion into `f32`.
pub fn phase_of(tick: u64, tick_len_ns: u64, period_ns: u64) -> f32 {
    let period = u128::from(period_ns.max(1));
    let rem = (u128::from(tick) * u128::from(tick_len_ns)) % period;
    let q = (rem * PHASE_ONE) / period;
    // q < 2^24: exact in f32, and dividing by 2^24 only changes the exponent.
    q as f32 / PHASE_ONE as f32
}

/// The bin index of `tick` when each cycle of `period_ns` is cut into `bins` equal bins:
/// `floor(tick * tick_len_ns * bins / period_ns)`.
pub fn bin_index(tick: u64, tick_len_ns: u64, period_ns: u64, bins: u64) -> u64 {
    let x = u128::from(tick) * u128::from(tick_len_ns) * u128::from(bins);
    u64::try_from(x / u128::from(period_ns.max(1))).unwrap_or(u64::MAX)
}

/// What the medium did during one cycle of one rhythm: the sums of the tick summaries of the
/// ticks whose start lies in the cycle (decision 6). Handed to the plasticity port at the
/// boundary of the rhythm the spec names, and reported in [`TickSummary::completed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CycleSummary {
    /// The rhythm's index in the spec.
    pub rhythm: u8,
    /// The cycle's index.
    pub cycle: u64,
    /// The first tick run in the cycle (the medium may start or be restored mid-cycle).
    pub first_tick: Option<u64>,
    /// Ticks run in the cycle.
    pub ticks: u64,
    /// Operations, summed.
    pub counts: OpCounts,
    /// Passes, summed.
    pub passes: u64,
    /// Distinct cells active per tick, summed over the ticks.
    pub active_cell_ticks: u64,
    /// Ticks that hit a limit.
    pub truncated_ticks: u64,
    /// Zero-delay messages carried to the next tick, summed.
    pub carried: u64,
    /// Event references dropped at `max_refs`, summed.
    pub refs_dropped: u64,
    /// Emitter firings without an anchor, summed.
    pub unanchored: u64,
}

impl CycleSummary {
    fn add(&mut self, tick: u64, cycle: u64, s: &TickSummary) {
        if self.first_tick.is_none() {
            self.first_tick = Some(tick);
            self.cycle = cycle;
        }
        self.ticks += 1;
        self.counts.accumulate(&s.counts);
        self.passes += u64::from(s.passes);
        self.active_cell_ticks += s.active_cells;
        self.truncated_ticks += u64::from(s.truncation.is_some());
        self.carried += s.carried;
        self.refs_dropped += s.refs_dropped;
        self.unanchored += s.unanchored;
    }
}

/// The Oscillome Engine: computes phases, cycle indices, bins and boundaries from the tick index
/// (pure functions of the tick and the two lengths), and keeps the per-cycle summaries (the only
/// state it has, persisted with the medium).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OscillomeEngine {
    tick_len_ns: u64,
    periods_ns: Vec<u64>,
    /// The summaries in progress, one per rhythm, when the spec keeps them.
    pub(crate) summaries: Option<Vec<CycleSummary>>,
}

impl OscillomeEngine {
    /// The engine of `spec`, with empty summaries.
    pub fn new(spec: &Oscillome) -> Self {
        OscillomeEngine {
            tick_len_ns: spec.tick_len_ns,
            periods_ns: spec.periods_ns.clone(),
            summaries: spec.cycle_summary.then(|| {
                (0..spec.periods_ns.len())
                    .map(|r| CycleSummary {
                        rhythm: r as u8,
                        ..CycleSummary::default()
                    })
                    .collect()
            }),
        }
    }

    /// The tick length, in nanoseconds (0 when the oscillome is off).
    pub fn tick_len_ns(&self) -> u64 {
        self.tick_len_ns
    }

    /// Number of rhythms.
    pub fn rhythms(&self) -> usize {
        self.periods_ns.len()
    }

    /// The cycle index of `tick` in rhythm `r`.
    pub fn cycle(&self, r: usize, tick: u64) -> u64 {
        cycle_index(tick, self.tick_len_ns, self.periods_ns[r])
    }

    /// The phase of `tick` in rhythm `r`.
    pub fn phase(&self, r: usize, tick: u64) -> f32 {
        phase_of(tick, self.tick_len_ns, self.periods_ns[r])
    }

    /// Whether `tick` is a boundary tick of rhythm `r`: the cycle index changes during it.
    pub fn is_boundary(&self, r: usize, tick: u64) -> bool {
        self.cycle(r, tick.saturating_add(1)) != self.cycle(r, tick)
    }

    /// The bin index of `tick` in rhythm `r` cut into `bins` bins per cycle.
    pub fn bin(&self, r: usize, bins: u64, tick: u64) -> u64 {
        bin_index(tick, self.tick_len_ns, self.periods_ns[r], bins)
    }

    /// Every rhythm's phase at `tick`, in spec order, zero past the last.
    pub fn phases(&self, tick: u64) -> [f32; R] {
        let mut out = [0.0; R];
        for (r, phase) in out.iter_mut().enumerate().take(self.periods_ns.len()) {
            *phase = self.phase(r, tick);
        }
        out
    }

    /// The fewest and the most ticks a cycle of rhythm `r` holds: equal when the period is a
    /// multiple of the tick, else one apart.
    pub fn ticks_per_cycle(&self, r: usize) -> (u64, u64) {
        let (p, l) = (self.periods_ns[r], self.tick_len_ns.max(1));
        if p.is_multiple_of(l) {
            (p / l, p / l)
        } else {
            (p / l, p / l + 1)
        }
    }

    /// The summaries of the cycles in progress, when the spec keeps them.
    pub fn summaries(&self) -> Option<&[CycleSummary]> {
        self.summaries.as_deref()
    }

    /// Add `tick`'s summary to every rhythm's cycle in progress; return, in rhythm order, the
    /// summaries of the cycles whose boundary `tick` is, and start their next cycles.
    pub(crate) fn record(&mut self, tick: u64, summary: &TickSummary) -> Vec<CycleSummary> {
        let mut completed = Vec::new();
        let Some(mut sums) = self.summaries.take() else {
            return completed;
        };
        for (r, acc) in sums.iter_mut().enumerate() {
            acc.add(tick, self.cycle(r, tick), summary);
            if self.is_boundary(r, tick) {
                completed.push(*acc);
                *acc = CycleSummary {
                    rhythm: r as u8,
                    ..CycleSummary::default()
                };
            }
        }
        self.summaries = Some(sums);
        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_det_agrees_with_the_platform_and_returns_pinned_bits() {
        let mut x = -30.0f64;
        while x < 30.0 {
            let (a, b) = (exp_det(x), x.exp());
            assert!(((a - b) / b).abs() < 1e-13, "exp {x}: {a} {b}");
            x += 0.173;
        }
        // The same inputs and bits as gordian-stream's det_exp, which this copies.
        assert_eq!(exp_det(1.0).to_bits(), 0x4005_bf0a_8b14_5768);
        assert_eq!(exp_det(-3.7).to_bits(), 0x3f99_511f_c687_1045);
        // The bits of the decays the conversion table uses, computed independently (Python,
        // the same series in IEEE double precision, then rounded to single precision with
        // struct.pack; each also agrees with math.exp after that rounding; exp(-0.1) in double
        // precision is one ulp above math.exp's).
        assert_eq!(exp_det(-0.1).to_bits(), 0x3fec_f46d_99d5_2b3b);
        assert_eq!(decay_per_tick(secs(1.0), secs(0.1)).to_bits(), 0x3f67_a36d);
        assert_eq!(decay_per_tick(secs(1.0), secs(0.5)).to_bits(), 0x3f1b_4598);
        assert_eq!(decay_per_tick(secs(1.0), secs(2.0)).to_bits(), 0x3e0a_9555);
        assert_eq!(decay_per_tick(secs(10.0), secs(2.0)).to_bits(), 0x3f51_9857);
        assert_eq!(rate_per_tick(secs(10.0), secs(0.1)).to_bits(), 0x3c23_0606);
    }

    #[test]
    fn delays_round_to_the_nearest_tick_with_a_minimum_of_one() {
        let ms = 1_000_000;
        assert_eq!(delay_ticks(0, 100 * ms), 1, "zero is raised to one tick");
        assert_eq!(delay_ticks(20 * ms, 100 * ms), 1);
        assert_eq!(delay_ticks(149 * ms, 100 * ms), 1);
        assert_eq!(delay_ticks(150 * ms, 100 * ms), 2, "half up");
        assert_eq!(delay_ticks(250 * ms, 500 * ms), 1);
        assert_eq!(delay_ticks(750 * ms, 500 * ms), 2);
        assert_eq!(delay_ticks(6_000 * ms, 2_000 * ms), 3);
    }

    #[test]
    fn spans_round_up() {
        let ms = 1_000_000;
        assert_eq!(span_ticks(0, 100 * ms), 0);
        assert_eq!(span_ticks(1, 100 * ms), 1);
        assert_eq!(span_ticks(100 * ms, 100 * ms), 1);
        assert_eq!(span_ticks(101 * ms, 100 * ms), 2);
        assert_eq!(span_ticks(16_000 * ms, 500 * ms), 32);
        assert_eq!(span_ticks(16_001 * ms, 2_000 * ms), 9);
    }

    #[test]
    fn phases_cycles_and_boundaries_on_a_whole_multiple() {
        // 10 s on 2 s ticks: five ticks per cycle, phases 0, 0.2, ..., 0.8, boundary at 4, 9, ...
        let e = OscillomeEngine::new(&Oscillome {
            tick_len_ns: secs(2.0),
            periods_ns: vec![secs(10.0)],
            ..Oscillome::default()
        });
        let phases: Vec<f32> = (0..6).map(|t| e.phase(0, t)).collect();
        // 0.2 is q = floor(0.2 * 2^24) = 3355443 over 2^24.
        assert_eq!(phases[0], 0.0);
        assert_eq!(phases[1], 3_355_443.0 / 16_777_216.0);
        assert_eq!(phases[5], 0.0);
        let boundaries: Vec<u64> = (0..15).filter(|t| e.is_boundary(0, *t)).collect();
        assert_eq!(boundaries, vec![4, 9, 14]);
        assert_eq!(e.ticks_per_cycle(0), (5, 5));
    }

    #[test]
    fn boundaries_fall_unevenly_when_the_period_is_not_a_multiple() {
        // 10 s on 300 ms ticks: cycles of 33 or 34 ticks (section 4b's example).
        let e = OscillomeEngine::new(&Oscillome {
            tick_len_ns: secs(0.3),
            periods_ns: vec![secs(10.0)],
            ..Oscillome::default()
        });
        let boundaries: Vec<u64> = (0..200).filter(|t| e.is_boundary(0, *t)).collect();
        // Tick t starts at 0.3 t s; cycle c starts at 10 c s. Last tick of cycle 0: 33 (9.9 s);
        // of cycle 1: 66 (19.8 s); of cycle 2: 99 (29.7 s; 100 starts at 30.0 s); then 133
        // (39.9 s), 166, 199.
        assert_eq!(boundaries, vec![33, 66, 99, 133, 166, 199]);
        let lengths: Vec<u64> = std::iter::once(boundaries[0] + 1)
            .chain(boundaries.windows(2).map(|w| w[1] - w[0]))
            .collect();
        assert_eq!(lengths, vec![34, 33, 33, 34, 33, 33]);
        assert_eq!(e.ticks_per_cycle(0), (33, 34));
    }

    #[test]
    fn a_phase_is_never_rounded_up_to_one() {
        // The last nanosecond of a 100 s cycle: rem / period = 1 - 1e-11, which an f32 division
        // of the two would round to 1.0.
        let p = secs(100.0);
        assert!(phase_of(p - 1, 1, p) < 1.0);
        assert_eq!(phase_of(p - 1, 1, p), 1.0 - 1.0 / 16_777_216.0);
        assert_eq!(phase_of(p, 1, p), 0.0);
    }

    #[test]
    fn bins_cut_a_cycle_evenly() {
        // 100 s cut into 10 bins, on 2 s ticks: a bin is 5 ticks.
        assert_eq!(bin_index(0, secs(2.0), secs(100.0), 10), 0);
        assert_eq!(bin_index(4, secs(2.0), secs(100.0), 10), 0);
        assert_eq!(bin_index(5, secs(2.0), secs(100.0), 10), 1);
        assert_eq!(bin_index(50, secs(2.0), secs(100.0), 10), 10);
    }
}
