//! The seven hand-written archetypes (docs/medium-ports.md, section 5).
//!
//! One function per archetype, many instances, parameters per cell. Every function is
//! `(params, state, inputs, field, ctx) -> (state', activation, ...)`; `ctx.dt` is the number of
//! ticks since the cell last ran, which is how a sparse cell applies leak and ageing lazily instead
//! of running every tick. The rest of the context (the tick, its length, the bins elapsed, an
//! oscillator's period) serves the oscillome's forms (M1b), which are marked below; M1's forms
//! read `dt` only. Arithmetic is `f32` with `+ - * /`, comparison, `min`, `max` and `abs`
//! only; the one power the archetypes need ([`pow_det`]) is repeated squaring in those
//! operations, so it returns the same bits wherever IEEE single precision does.
//!
//! # Parameters and state, per archetype
//!
//! Thresholds compare with `>=` throughout. "Integer" parameters must be whole numbers in
//! `0..=2^24` (exact in `f32`). Unused parameters must be finite and are ignored.
//!
//! | Archetype | params | state | activation | support |
//! |---|---|---|---|---|
//! | `Sense` | 0 mode: 0 presence (1), 1 sum of event values, 2 count of events | none | by mode | the events routed to it this tick |
//! | `Integrator` | 0 leak per tick in `[0, 1]`; 1 threshold; 2 reset: 0 to zero on firing, 1 keep (decay only); 3 lookback (integer) | 0 level | the level, on the tick it rises across the threshold; else 0 | accumulated, pruned to the lookback, cleared on reset |
//! | `Novelty` | 0 rate in `(0, 1]`; 1 k `>= 0`; 2 floor `>= 0`; 3 warm-up runs (integer); 4 gaps: 0 ignored, 1 each silent tick is a zero input | 0 mean, 1 mean absolute deviation, 2 runs seen | the deviation, when it exceeds `k * dev + floor` after warm-up; else 0 | this run's inputs |
//! | `Coincidence` | 0 n (integer 1..=8); 1 window w (integer; ticks, bins or microseconds by mode); 2 consume: 0 no, 1 clear on firing; 3 lookback (integer); 4 mode (M1b): 0 sliding window in ticks, 1 binned by a rhythm, 2 ordered by event time; 5 mode 1: rhythm index, mode 2: lead (0 none, 1 slot 0 first); 6 mode 1: bins per cycle (integer `>= 1`) | modes 0 and 1: slot i: ticks (bins) since the last positive arrival on incoming synapse i, or -1; mode 2: slot i < 4: ticks since the event time of that arrival, slot 4 + i: its offset in microseconds | the number of slots within the window, when at least n (and, mode 2 with a lead, slot 0 first); else 0 | accumulated, pruned to the lookback, cleared when consumed |
//! | `Gate` | 0 field index (integer `< F`); 1 threshold; 2 sense: 0 open when the scalar `>=` threshold, 1 open when `<` | none | the input sum while open; else 0 | this run's inputs |
//! | `Latch` | 0 threshold; 1 hold h ticks (integer); 2 retire (M1b): 0 no, 1 propose `retire` when the hold expires; 3 the retire kind (integer `<= 65535`) | 0 held value, 1 ticks of hold left or -1 | the held value on the firing tick and the h ticks after it; else 0 | the inputs that fired it, held (with retire: merged across re-firings within one hold) |
//! | `Emit` | 0 threshold; 1 kind (integer `<= 65535`); 2 lookback (integer); 3 refractory ticks (integer) | 0 ticks since the last proposal, or -1 | the input sum when it proposes; else 0 | this run's inputs |
//! | `Oscillator` (M1b) | 0 threshold `> 0`; 1 floor `>= 0`; 2 most cycles (integer, 0 for no limit); its period is the delay of its one self-synapse, its decay per cycle that synapse's weight | 0 amplitude, 1 ticks since the phase reset or -1 when stopped, 2 cycles since the reset | the amplitude at the reset and at each cycle; else 0 | the inputs that reset it, kept while it runs |

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::types::{EventRef, F, Field, P, R, S, SynapseId};

/// Largest integer parameter: every integer up to `2^24` is exact in `f32`.
pub const MAX_INT_PARAM: f32 = 16_777_216.0;

/// Most sources of an ordered coincidence (mode 2): each needs two state slots, an age and an
/// offset.
pub const ORDERED_SLOTS: usize = S / 2;

/// Magnitude bound on activations, states and message values. A value that would leave
/// `[-BOUND, BOUND]` is clamped to it, and a NaN becomes zero, so that one runaway cell cannot
/// turn the medium's arithmetic into infinities and NaNs (which would also make persisted bytes
/// depend on NaN payloads).
pub const BOUND: f32 = 1.0e30;

/// The archetypes. The tag of each (used by the persisted encoding) is explicit and never derived
/// from the enum's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Archetype {
    /// Receives external events matching an address pattern.
    Sense,
    /// Leaky accumulation; fires when the level rises across a threshold.
    Integrator,
    /// Running estimate of its input; fires on deviation.
    Novelty,
    /// Fires when at least `n` distinct incoming synapses delivered within `w` ticks.
    Coincidence,
    /// Passes its input only while a field scalar is on the open side of a threshold.
    Gate,
    /// Holds its activation for `h` ticks after firing.
    Latch,
    /// Turns input at or above a threshold into a proposal.
    Emit,
    /// The phase-reset form of `Latch` (M1b, section 4b, local oscillators): an input at or above
    /// its threshold resets its phase and fires; it then fires once per period, by a delayed
    /// message to itself over its self-synapse, until its amplitude falls below its floor or it
    /// has run its cycles. It never runs on a tick without a message.
    Oscillator,
}

impl Archetype {
    /// Every archetype, in tag order.
    pub const ALL: [Archetype; 8] = [
        Archetype::Sense,
        Archetype::Integrator,
        Archetype::Novelty,
        Archetype::Coincidence,
        Archetype::Gate,
        Archetype::Latch,
        Archetype::Emit,
        Archetype::Oscillator,
    ];

    pub(crate) fn tag(self) -> u8 {
        match self {
            Archetype::Sense => 0,
            Archetype::Integrator => 1,
            Archetype::Novelty => 2,
            Archetype::Coincidence => 3,
            Archetype::Gate => 4,
            Archetype::Latch => 5,
            Archetype::Emit => 6,
            Archetype::Oscillator => 7,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Archetype> {
        Archetype::ALL.get(usize::from(tag)).copied()
    }

    /// The state a new cell of this archetype starts in.
    pub fn initial_state(self) -> [f32; S] {
        match self {
            Archetype::Coincidence => [-1.0; S],
            Archetype::Latch | Archetype::Oscillator => {
                let mut s = [0.0; S];
                s[1] = -1.0;
                s
            }
            Archetype::Emit => {
                let mut s = [0.0; S];
                s[0] = -1.0;
                s
            }
            _ => [0.0; S],
        }
    }

    /// Field scalars one run reads. Known before the run, so the operation limit can be checked
    /// before any work is done. A binned coincidence reads its rhythm's phase (M1b).
    pub fn field_reads(self, params: &[f32; P]) -> u64 {
        match self {
            Archetype::Gate => 1,
            Archetype::Coincidence if params[4] == 1.0 => 1,
            _ => 0,
        }
    }

    /// Whether these parameters select one of the oscillome's forms (M1b): a binned or ordered
    /// coincidence, a retiring latch, or an oscillator. A medium that uses none of them, no phase
    /// gate and no oscillome element is M1's medium.
    pub fn uses_oscillome(self, params: &[f32; P]) -> bool {
        match self {
            Archetype::Coincidence => params[4] != 0.0,
            Archetype::Latch => params[2] != 0.0,
            Archetype::Oscillator => true,
            _ => false,
        }
    }

    /// The lookback, in ticks, to which an accumulating archetype prunes its support; `None` for
    /// archetypes whose support is not accumulated.
    pub(crate) fn support_lookback(self, params: &[f32; P]) -> Option<u64> {
        match self {
            Archetype::Integrator | Archetype::Coincidence => Some(params[3] as u64),
            _ => None,
        }
    }

    /// Check `params` for this archetype.
    pub fn validate(self, params: &[f32; P]) -> Result<(), ParamError> {
        for (index, p) in params.iter().enumerate() {
            if !p.is_finite() {
                return Err(ParamError {
                    index,
                    reason: "not finite",
                });
            }
        }
        let int = |index: usize, lo: f32, hi: f32| -> Result<(), ParamError> {
            let v = params[index];
            if v < lo || v > hi || (v as u64) as f32 != v {
                Err(ParamError {
                    index,
                    reason: "not an integer in range",
                })
            } else {
                Ok(())
            }
        };
        let range = |index: usize, lo: f32, hi: f32, reason| -> Result<(), ParamError> {
            let v = params[index];
            if v < lo || v > hi {
                Err(ParamError { index, reason })
            } else {
                Ok(())
            }
        };
        match self {
            Archetype::Sense => int(0, 0.0, 2.0),
            Archetype::Integrator => {
                range(0, 0.0, 1.0, "leak outside [0, 1]")?;
                int(2, 0.0, 1.0)?;
                int(3, 0.0, MAX_INT_PARAM)
            }
            Archetype::Novelty => {
                if !(params[0] > 0.0 && params[0] <= 1.0) {
                    return Err(ParamError {
                        index: 0,
                        reason: "rate outside (0, 1]",
                    });
                }
                range(1, 0.0, BOUND, "k negative or too large")?;
                range(2, 0.0, BOUND, "floor negative or too large")?;
                int(3, 0.0, MAX_INT_PARAM)?;
                int(4, 0.0, 1.0)
            }
            Archetype::Coincidence => {
                int(0, 1.0, S as f32)?;
                int(1, 0.0, MAX_INT_PARAM)?;
                int(2, 0.0, 1.0)?;
                int(3, 0.0, MAX_INT_PARAM)?;
                int(4, 0.0, 2.0)?;
                match params[4] as u8 {
                    1 => {
                        int(5, 0.0, (R - 1) as f32)?;
                        int(6, 1.0, MAX_INT_PARAM)
                    }
                    2 => {
                        int(0, 1.0, ORDERED_SLOTS as f32)?;
                        int(5, 0.0, 1.0)
                    }
                    _ => Ok(()),
                }
            }
            Archetype::Gate => {
                int(0, 0.0, (F - 1) as f32)?;
                int(2, 0.0, 1.0)
            }
            Archetype::Latch => {
                int(1, 0.0, MAX_INT_PARAM)?;
                int(2, 0.0, 1.0)?;
                if params[2] == 1.0 {
                    int(3, 0.0, f32::from(u16::MAX))
                } else {
                    Ok(())
                }
            }
            Archetype::Oscillator => {
                if !(params[0] > 0.0 && params[0] <= BOUND) {
                    return Err(ParamError {
                        index: 0,
                        reason: "threshold not positive",
                    });
                }
                range(1, 0.0, BOUND, "floor negative or too large")?;
                int(2, 0.0, MAX_INT_PARAM)
            }
            Archetype::Emit => {
                int(1, 0.0, f32::from(u16::MAX))?;
                int(2, 0.0, MAX_INT_PARAM)?;
                int(3, 0.0, MAX_INT_PARAM)
            }
        }
    }
}

/// Why a cell's parameters were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamError {
    /// The parameter's index.
    pub index: usize,
    /// What is wrong with it.
    pub reason: &'static str,
}

/// The event references an input carries.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Refs {
    /// None (a wake).
    None,
    /// One event (a routed event).
    One(EventRef),
    /// The sender's support, shared by every message of one run.
    Many(Arc<[EventRef]>),
}

impl Refs {
    pub(crate) fn as_slice(&self) -> &[EventRef] {
        match self {
            Refs::None => &[],
            Refs::One(r) => std::slice::from_ref(r),
            Refs::Many(rs) => rs,
        }
    }
}

/// Where an input came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Origin {
    /// The cell asked to run again this tick (a latch holding).
    Wake,
    /// A routed event, `order` being its position in the tick's sorted events.
    Event { order: u32 },
    /// A message over a synapse.
    Synapse {
        sent_tick: u64,
        sent_pass: u8,
        synapse: SynapseId,
        /// The synapse's position among its target's incoming synapses, by id.
        slot: u32,
        /// The synapse goes from its target to itself (an oscillator's clock).
        self_loop: bool,
    },
}

/// One input to one run of a cell.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Input {
    pub(crate) value: f32,
    pub(crate) origin: Origin,
    pub(crate) refs: Refs,
}

impl Input {
    /// The canonical order of a cell's inputs: wakes, then events in the tick's event order, then
    /// messages by `(sent tick, sent pass, synapse id)`. It is total (a synapse sends at most once
    /// per pass) and does not depend on the order in which inputs were collected.
    pub(crate) fn order_key(&self) -> (u8, u64, u64, u32) {
        match self.origin {
            Origin::Wake => (0, 0, 0, 0),
            Origin::Event { order } => (1, u64::from(order), 0, 0),
            Origin::Synapse {
                sent_tick,
                sent_pass,
                synapse,
                ..
            } => (2, sent_tick, u64::from(sent_pass), synapse.0),
        }
    }
}

/// How a run's support (the events it cites) is formed from the cell's previous support and
/// this run's inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportRule {
    /// The references of this run's inputs.
    Replace,
    /// The previous support and this run's references, pruned to the archetype's lookback.
    Merge,
    /// The previous support, unchanged.
    Keep,
    /// The references of this run's inputs other than self-loop messages (an oscillator's
    /// reset: its own pending cycle does not belong to the new phase).
    ReplaceExternal,
}

/// A proposal a run asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Emission {
    /// An emitter's notice: kind and lookback from its parameters, strength its activation.
    Notice,
    /// A latch's retirement (M1b decision 7): the whole held support, with this kind and the
    /// held value as strength.
    Retire { kind: u16, strength: f32 },
}

/// What one run of a cell knows besides its parameters, state, inputs and the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Ctx {
    /// The tick.
    pub(crate) tick: u64,
    /// Ticks since the cell last ran (0 if it already ran this tick or never ran).
    pub(crate) dt: u64,
    /// The tick length in nanoseconds (0 when the oscillome is off).
    pub(crate) tick_len_ns: u64,
    /// For a binned coincidence: bins of its rhythm elapsed since it last ran.
    pub(crate) bins_elapsed: u64,
    /// For an oscillator: its period, the delay of its self-synapse, in ticks.
    pub(crate) period: u64,
}

/// What one run produced besides the new state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RunOut {
    pub(crate) activation: f32,
    /// Run again next tick, with no input (a latch holding).
    pub(crate) wake: bool,
    pub(crate) support: SupportRule,
    /// Forget the support after this run's messages have taken it.
    pub(crate) clear_support: bool,
    /// Make a proposal from this run (emitters, and retiring latches).
    pub(crate) emit: Option<Emission>,
}

/// Clamp to `[-BOUND, BOUND]`; NaN becomes zero.
pub(crate) fn sane(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(-BOUND, BOUND)
    }
}

/// `base^n` by repeated squaring, in `f32` multiplications only.
pub(crate) fn pow_det(base: f32, mut n: u64) -> f32 {
    let mut result = 1.0f32;
    let mut b = base;
    while n > 0 {
        if n & 1 == 1 {
            result *= b;
        }
        n >>= 1;
        if n > 0 {
            b *= b;
        }
    }
    result
}

/// `dt` as an `f32` age increment, saturated where the archetypes stop caring.
fn age_step(dt: u64) -> f32 {
    if dt > MAX_INT_PARAM as u64 {
        MAX_INT_PARAM * 2.0
    } else {
        dt as f32
    }
}

/// The sum of the values of the inputs that are not wakes, in canonical order.
fn input_sum(inputs: &[Input]) -> f32 {
    inputs
        .iter()
        .filter(|i| i.origin != Origin::Wake)
        .fold(0.0f32, |acc, i| sane(acc + i.value))
}

fn quiet(support: SupportRule) -> RunOut {
    RunOut {
        activation: 0.0,
        wake: false,
        support,
        clear_support: false,
        emit: None,
    }
}

/// Run `archetype` once. `inputs` are in canonical order and non-empty.
pub(crate) fn run(
    archetype: Archetype,
    params: &[f32; P],
    state: &mut [f32; S],
    inputs: &[Input],
    field: &Field,
    ctx: &Ctx,
) -> RunOut {
    let dt = ctx.dt;
    let out = match archetype {
        Archetype::Sense => sense(params, inputs),
        Archetype::Integrator => integrator(params, state, inputs, dt),
        Archetype::Novelty => novelty(params, state, inputs, dt),
        Archetype::Coincidence => match params[4] as u8 {
            0 => coincidence(params, state, inputs, dt),
            1 => coincidence(params, state, inputs, ctx.bins_elapsed),
            _ => ordered_coincidence(params, state, inputs, ctx),
        },
        Archetype::Gate => gate(params, inputs, field),
        Archetype::Latch => latch(params, state, inputs, dt),
        Archetype::Emit => emit(params, state, inputs, dt),
        Archetype::Oscillator => oscillator(params, state, inputs, ctx),
    };
    for s in state.iter_mut() {
        *s = sane(*s);
    }
    RunOut {
        activation: sane(out.activation),
        ..out
    }
}

fn sense(params: &[f32; P], inputs: &[Input]) -> RunOut {
    let events = inputs.iter().filter(|i| i.origin != Origin::Wake);
    let activation = match params[0] as u8 {
        0 => 1.0,
        1 => input_sum(inputs),
        _ => events.count() as f32,
    };
    RunOut {
        activation,
        ..quiet(SupportRule::Replace)
    }
}

fn integrator(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], dt: u64) -> RunOut {
    let (leak, threshold, reset) = (params[0], params[1], params[2] == 0.0);
    let decayed = sane(state[0] * pow_det(leak, dt));
    let level = sane(decayed + input_sum(inputs));
    let fire = decayed < threshold && level >= threshold;
    state[0] = if fire && reset { 0.0 } else { level };
    RunOut {
        activation: if fire { level } else { 0.0 },
        clear_support: fire && reset,
        ..quiet(SupportRule::Merge)
    }
}

fn novelty(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], dt: u64) -> RunOut {
    let (rate, k, floor, warmup, gaps) = (params[0], params[1], params[2], params[3], params[4]);
    let (mut mean, mut dev, runs) = (state[0], state[1], state[2]);
    if gaps == 1.0 && runs > 0.0 && dt > 1 {
        // n silent ticks, each a zero input: mean_n = b^n mean, and, since |0 - mean_i| =
        // |mean| b^i, dev_n = b^n dev + n rate |mean| b^(n-1), with b = 1 - rate.
        let n = dt - 1;
        let b = 1.0 - rate;
        let bn1 = pow_det(b, n - 1);
        let bn = bn1 * b;
        dev = sane(bn * dev + age_step(n) * rate * mean.abs() * bn1);
        mean = sane(bn * mean);
    }
    let x = input_sum(inputs);
    let deviation = sane((x - mean).abs());
    let band = sane(k * dev + floor);
    let fire = runs >= warmup && deviation > band;
    state[0] = sane(mean + rate * (x - mean));
    state[1] = sane(dev + rate * (deviation - dev));
    state[2] = (runs + 1.0).min(warmup.max(1.0));
    RunOut {
        activation: if fire { deviation } else { 0.0 },
        ..quiet(SupportRule::Replace)
    }
}

/// The sliding window (mode 0, `elapsed` in ticks) and the binned form (mode 1, `elapsed` in bins
/// of a rhythm): the same rule with ages counted in different units. With window 0 in mode 1, two
/// arrivals coincide when they share a bin.
fn coincidence(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], elapsed: u64) -> RunOut {
    let (n, window, consume) = (params[0], params[1], params[2] == 1.0);
    let step = age_step(elapsed);
    for age in state.iter_mut() {
        if *age >= 0.0 {
            let a = *age + step;
            *age = if a > window { -1.0 } else { a };
        }
    }
    for input in inputs {
        if let Origin::Synapse { slot, .. } = input.origin
            && input.value > 0.0
            && let Some(age) = state.get_mut(slot as usize)
        {
            *age = 0.0;
        }
    }
    let count = state.iter().filter(|a| **a >= 0.0).count() as f32;
    let fire = count >= n;
    if fire && consume {
        *state = [-1.0; S];
    }
    RunOut {
        activation: if fire { count } else { 0.0 },
        clear_support: fire && consume,
        ..quiet(SupportRule::Merge)
    }
}

fn gate(params: &[f32; P], inputs: &[Input], field: &Field) -> RunOut {
    let scalar = field.scalars[params[0] as usize];
    let open = if params[2] == 0.0 {
        scalar >= params[1]
    } else {
        scalar < params[1]
    };
    RunOut {
        activation: if open { input_sum(inputs) } else { 0.0 },
        ..quiet(SupportRule::Replace)
    }
}

fn latch(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], dt: u64) -> RunOut {
    let (threshold, hold, retire) = (params[0], params[1], params[2] == 1.0);
    let was_holding = state[1] >= 0.0;
    let mut left = state[1];
    if left >= 0.0 {
        left -= age_step(dt);
        if left < 0.0 {
            left = -1.0;
        }
    }
    let has_input = inputs.iter().any(|i| i.origin != Origin::Wake);
    let x = input_sum(inputs);
    let mut support = SupportRule::Keep;
    if has_input && x >= threshold {
        state[0] = x;
        left = hold;
        // A retiring latch keeps the anchor of the anomaly it holds across re-firings, so that
        // its retirement cites the event its notice cited.
        support = if retire && was_holding {
            SupportRule::Merge
        } else {
            SupportRule::Replace
        };
    }
    state[1] = left;
    if left >= 0.0 {
        RunOut {
            activation: state[0],
            // A retiring latch also wakes on the last tick of its hold, to see it expire.
            wake: if retire { true } else { left >= 1.0 },
            ..quiet(support)
        }
    } else {
        RunOut {
            clear_support: true,
            emit: (retire && was_holding).then_some(Emission::Retire {
                kind: params[3] as u16,
                strength: state[0],
            }),
            ..quiet(support)
        }
    }
}

/// Mode 2: arrivals carry the time of the earliest event they cite, `(tick, offset_ns)`, kept as
/// an age in ticks and an offset in microseconds, so order inside a long tick survives. Slot i
/// counts when its time is within the window (microseconds) of the newest slot's; a slot whose
/// time is more than the window before the current tick's start expires. With a lead, slot 0 must
/// count and be no later than any counted slot. Times are compared in `i64` microseconds, built
/// from integers that are exact in the `f32` state.
fn ordered_coincidence(
    params: &[f32; P],
    state: &mut [f32; S],
    inputs: &[Input],
    ctx: &Ctx,
) -> RunOut {
    let (n, window, consume, lead) = (
        params[0] as usize,
        params[1] as i64,
        params[2] == 1.0,
        params[5] == 1.0,
    );
    let k = ORDERED_SLOTS;
    let len_us = (ctx.tick_len_ns / 1_000) as i64;
    let step = age_step(ctx.dt);
    for age in state.iter_mut().take(k) {
        if *age >= 0.0 {
            *age = (*age + step).min(MAX_INT_PARAM * 2.0);
        }
    }
    for input in inputs {
        if let Origin::Synapse { slot, .. } = input.origin
            && input.value > 0.0
            && (slot as usize) < k
        {
            let (age, offset_us) = match input.refs.as_slice().first() {
                Some(r) => (
                    age_step(ctx.tick.saturating_sub(r.tick)),
                    (r.offset_ns / 1_000) as f32,
                ),
                None => (0.0, 0.0),
            };
            state[slot as usize] = age;
            state[k + slot as usize] = offset_us;
        }
    }
    // Time of slot i in microseconds relative to the current tick's start.
    let time = |st: &[f32; S], i: usize| st[k + i] as i64 - st[i] as i64 * len_us;
    for i in 0..k {
        if state[i] >= 0.0 && -time(state, i) > window {
            state[i] = -1.0;
            state[k + i] = 0.0;
        }
    }
    // (slot, time) of the live slots, in slot order.
    let live: Vec<(usize, i64)> = (0..k)
        .filter(|&i| state[i] >= 0.0)
        .map(|i| (i, time(state, i)))
        .collect();
    let newest = live.iter().map(|&(_, t)| t).max();
    let counted: Vec<(usize, i64)> = match newest {
        Some(t) => live
            .into_iter()
            .filter(|&(_, ti)| t - ti <= window)
            .collect(),
        None => Vec::new(),
    };
    let led = !lead
        || counted
            .first()
            .is_some_and(|&(i, t0)| i == 0 && counted.iter().all(|&(_, ti)| t0 <= ti));
    let fire = counted.len() >= n && led;
    if fire && consume {
        *state = [-1.0; S];
    }
    RunOut {
        activation: if fire { counted.len() as f32 } else { 0.0 },
        clear_support: fire && consume,
        ..quiet(SupportRule::Merge)
    }
}

/// The phase-reset oscillator. An input other than its own message, summing to at least the
/// threshold, resets it: amplitude = that sum, age 0, cycles 0, and it fires (which sends its
/// message to itself, due one period later). Its own message, arriving when the age is a whole
/// number of periods, is a cycle: the amplitude becomes the message's value (the previous
/// amplitude times the self-synapse's weight) and it fires again, unless the amplitude is below
/// the floor or the cycles are used up, in which case it stops. A message of an earlier phase
/// (the age is not a whole number of periods, because a reset came in between) is ignored, and
/// that chain ends there.
fn oscillator(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], ctx: &Ctx) -> RunOut {
    let (threshold, floor, most) = (params[0], params[1], params[2]);
    let period = ctx.period.max(1);
    let running = state[1] >= 0.0;
    if running {
        state[1] = (state[1] + age_step(ctx.dt)).min(MAX_INT_PARAM * 2.0);
    }
    let external: Vec<&Input> = inputs
        .iter()
        .filter(|i| {
            !matches!(
                i.origin,
                Origin::Wake
                    | Origin::Synapse {
                        self_loop: true,
                        ..
                    }
            )
        })
        .collect();
    let x = external.iter().fold(0.0f32, |acc, i| sane(acc + i.value));
    if !external.is_empty() && x >= threshold {
        state[0] = x;
        state[1] = 0.0;
        state[2] = 0.0;
        return RunOut {
            activation: x,
            ..quiet(SupportRule::ReplaceExternal)
        };
    }
    // The latest of its own messages (inputs are in canonical order, latest sent last).
    let own = inputs.iter().rev().find(|i| {
        matches!(
            i.origin,
            Origin::Synapse {
                self_loop: true,
                ..
            }
        )
    });
    let age = state[1] as u64;
    match own {
        Some(m) if running && age > 0 && age.is_multiple_of(period) => {
            let cycles = state[2] + 1.0;
            let amplitude = m.value;
            if amplitude > 0.0 && amplitude >= floor && (most == 0.0 || cycles <= most) {
                state[0] = amplitude;
                state[2] = cycles;
                RunOut {
                    activation: amplitude,
                    ..quiet(SupportRule::Keep)
                }
            } else {
                state[0] = 0.0;
                state[1] = -1.0;
                state[2] = 0.0;
                RunOut {
                    clear_support: true,
                    ..quiet(SupportRule::Keep)
                }
            }
        }
        _ => quiet(SupportRule::Keep),
    }
}

fn emit(params: &[f32; P], state: &mut [f32; S], inputs: &[Input], dt: u64) -> RunOut {
    let (threshold, refractory) = (params[0], params[3]);
    let mut since = state[0];
    if since >= 0.0 {
        since = (since + age_step(dt)).min(MAX_INT_PARAM * 2.0);
    }
    let x = input_sum(inputs);
    let fire = x >= threshold && (since < 0.0 || since > refractory);
    if fire {
        since = 0.0;
    }
    state[0] = since;
    RunOut {
        activation: if fire { x } else { 0.0 },
        emit: fire.then_some(Emission::Notice),
        ..quiet(SupportRule::Replace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow_det_matches_repeated_multiplication() {
        for base in [0.0f32, 0.5, 0.9, 1.0, 1.1] {
            let mut slow = 1.0f32;
            for n in 0..40u64 {
                // Not bit-identical in general (different association), but close.
                assert!((pow_det(base, n) - slow).abs() <= 1e-5 * slow.abs().max(1.0));
                slow *= base;
            }
        }
        assert_eq!(pow_det(0.5, 3), 0.125);
        assert_eq!(pow_det(0.9, 0), 1.0);
        assert_eq!(pow_det(0.9, u64::MAX), 0.0);
    }

    #[test]
    fn pow_det_returns_pinned_bits() {
        // A platform whose single-precision multiplication differed would fail here. The bits
        // were computed independently (Python, rounding each product of the same squaring
        // sequence to single precision), not copied from this function's output.
        assert_eq!(pow_det(0.9, 7).to_bits(), 0x3ef4_e351);
        assert_eq!(pow_det(0.99, 100).to_bits(), 0x3ebb_68ae);
    }

    #[test]
    fn sane_clamps_and_kills_nan() {
        assert_eq!(sane(f32::NAN), 0.0);
        assert_eq!(sane(f32::INFINITY), BOUND);
        assert_eq!(sane(f32::NEG_INFINITY), -BOUND);
        assert_eq!(sane(3.5), 3.5);
    }

    #[test]
    fn tags_round_trip() {
        for a in Archetype::ALL {
            assert_eq!(Archetype::from_tag(a.tag()), Some(a));
        }
        assert_eq!(Archetype::from_tag(8), None);
    }

    #[test]
    fn validation_refuses_bad_parameters() {
        let mut p = [0.0f32; P];
        assert!(Archetype::Sense.validate(&p).is_ok());
        p[0] = 3.0;
        assert_eq!(Archetype::Sense.validate(&p).unwrap_err().index, 0);
        p[0] = f32::NAN;
        assert_eq!(
            Archetype::Emit.validate(&p).unwrap_err().reason,
            "not finite"
        );
        let mut c = [0.0f32; P];
        c[0] = 9.0;
        assert!(Archetype::Coincidence.validate(&c).is_err());
        c[0] = 2.5;
        assert!(Archetype::Coincidence.validate(&c).is_err());
        c[0] = 2.0;
        assert!(Archetype::Coincidence.validate(&c).is_ok());
        let mut n = [0.0f32; P];
        assert!(Archetype::Novelty.validate(&n).is_err(), "rate 0");
        n[0] = 0.5;
        assert!(Archetype::Novelty.validate(&n).is_ok());
        let mut g = [0.0f32; P];
        g[0] = F as f32;
        assert!(Archetype::Gate.validate(&g).is_err());
    }
}
