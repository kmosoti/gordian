//! Pair cells: a decayed net count of co-alarm evidence per ordered pair of nodes and band (work
//! item A2; the design is in `DESIGN.md`, "Anticipation of hidden edges (A2)", written before
//! this code).
//!
//! For each ordered pair `(a, b)` a store is built for, and each of its nested bands `w_k`, two
//! ordinary cells: `ev[a, b, k]`, a `Sense` cell summing the values of the evidence events
//! addressed to the pair's edge node ([`crate::pair_node`]) on channel [`EV_CHANNEL`]` + k`, and
//! `cnt[a, b, k]`, an `Integrator` that only decays (time constant `tau`, never reset), fed by it
//! with weight 1: the **pair cell**, whose level is the sum of the evidence, each decayed by
//! `exp(-age / tau)`. The store **holds an edge** `a -> b` in band `k` when that level, read at the
//! start of a tick, is at least `theta`. There is no floor: decay is the only forgetting.
//!
//! The evidence is the adapter's to make; this module also gives the world-agnostic bookkeeping
//! that turns two series of instants into evidence ([`Trials`]: a trial at `a` is followed when a
//! `b` instant falls in its window, else it is a miss at its deadline) and the chance of a follow
//! ([`chance`]): with a follow worth `1 - q` and a miss worth `-q` at chance `q`, the evidence is
//! the count of follows beyond chance, which drifts by zero under chance. The crate knows nodes,
//! instants and evidence; never services, alarms or graphs. The names label a mechanism; they
//! claim nothing about what it finds.
//!
//! # Determinism and cost
//!
//! Pairs and trials are kept in a fixed order; the only map is a `BTreeMap`; no clock, no I/O, no
//! randomness. The cells' work is the medium's, counted per tick. A level read computes a decay,
//! as a cell run does, and is counted as one cell update in [`PairCells::take_work`], for the
//! adapter to price. The trials' bookkeeping is the sense side's arithmetic and is not counted.

use std::collections::BTreeMap;

use crate::archetype::{pow_det, sane};
use crate::engram::{NODE_LIMIT, pair_node};
use crate::medium::Medium;
use crate::oscillome::{Oscillome, TimeTarget, exp_det};
use crate::spec::{MediumBuilder, SenseMode};
use crate::types::{Address, CellId, Event, Limits, OpCounts, Pattern, Prices};

/// The channel of band 0's evidence events; band `k`'s is `EV_CHANNEL + k`.
pub const EV_CHANNEL: u16 = 16;

/// Most bands a store may have.
pub const MAX_BANDS: usize = 4;

/// A store's parameters (see the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct PairParams {
    /// The domain of the evidence events' addresses.
    pub domain: u16,
    /// The bands' windows, nanoseconds, strictly ascending, each positive.
    pub bands_ns: Vec<u64>,
    /// The scale of the evidence (a follow at chance `q` adds `gain * (1 - q)`, a miss takes
    /// `gain * q`; the adapter's to apply).
    pub gain: f32,
    /// The level at which an edge is held.
    pub threshold: f32,
    /// The pair cell's decay time constant, nanoseconds.
    pub tau_ns: u64,
}

impl PairParams {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.bands_ns.len();
        if n == 0 || n > MAX_BANDS {
            return Err(format!("pair cells: 1 to {MAX_BANDS} bands"));
        }
        if self.bands_ns[0] == 0 || self.bands_ns.windows(2).any(|w| w[0] >= w[1]) {
            return Err("pair cells: bands must be positive and strictly ascending".to_owned());
        }
        if !(self.gain.is_finite() && self.gain > 0.0) {
            return Err("pair cells: gain must be positive".to_owned());
        }
        if !self.threshold.is_finite() {
            return Err("pair cells: threshold must be finite".to_owned());
        }
        if self.tau_ns == 0 {
            return Err("pair cells: tau must be positive".to_owned());
        }
        Ok(())
    }
}

/// The pair cells of a store: which cell holds each `(pair, band)` count, and the reads' work.
#[derive(Debug, Clone)]
pub struct PairCells {
    params: PairParams,
    tick_len_ns: u64,
    pairs: Vec<(u16, u16)>,
    index: BTreeMap<(u16, u16), usize>,
    /// `cnt[pair, band]` at `pair * bands + band`.
    counters: Vec<CellId>,
    work: OpCounts,
}

impl PairCells {
    /// The medium of pair cells for `pairs` (ordered, distinct nodes below 128, no repeat), on a
    /// tick of `tick_len_ns`, and the store that reads it.
    pub fn build(
        params: PairParams,
        pairs: &[(u16, u16)],
        tick_len_ns: u64,
        limits: Limits,
        prices: Prices,
    ) -> Result<(Medium, PairCells), String> {
        params.validate()?;
        if tick_len_ns == 0 || tick_len_ns > u64::from(u32::MAX) {
            return Err("pair cells: the tick must be 1 ns to u32::MAX ns".to_owned());
        }
        let mut index = BTreeMap::new();
        for (i, &(a, b)) in pairs.iter().enumerate() {
            if a == b || a >= NODE_LIMIT || b >= NODE_LIMIT {
                return Err(format!("pair cells: bad pair ({a}, {b})"));
            }
            if index.insert((a, b), i).is_some() {
                return Err(format!("pair cells: pair ({a}, {b}) repeated"));
            }
        }
        let mut builder = MediumBuilder::new()
            .limits(limits)
            .prices(prices)
            .oscillome(Oscillome {
                tick_len_ns,
                ..Oscillome::default()
            });
        let mut counters = Vec::with_capacity(pairs.len() * params.bands_ns.len());
        for &(a, b) in pairs {
            for k in 0..params.bands_ns.len() {
                let ev = builder.sense(
                    Pattern {
                        domain: Some(params.domain),
                        node: Some(pair_node(a, b)),
                        channel: Some(EV_CHANNEL + k as u16),
                        tag: None,
                    },
                    SenseMode::Sum,
                );
                // The leak is set from `tau` when the medium is built.
                let cnt = builder.integrator(0.0, params.threshold, false, 0);
                builder.timed(
                    TimeTarget::Param {
                        cell: cnt,
                        index: 0,
                    },
                    params.tau_ns,
                );
                builder.synapse(ev, cnt, 1.0, 0);
                counters.push(cnt);
            }
        }
        let medium = builder.build().map_err(|e| format!("pair cells: {e:?}"))?;
        Ok((
            medium,
            PairCells {
                params,
                tick_len_ns,
                pairs: pairs.to_vec(),
                index,
                counters,
                work: OpCounts::default(),
            },
        ))
    }

    /// The parameters.
    pub fn params(&self) -> &PairParams {
        &self.params
    }

    /// The bands.
    pub fn bands(&self) -> usize {
        self.params.bands_ns.len()
    }

    /// The pairs, in the order given at build.
    pub fn pairs(&self) -> &[(u16, u16)] {
        &self.pairs
    }

    /// Whether the store has cells for `(a, b)`.
    pub fn has(&self, a: u16, b: u16) -> bool {
        self.index.contains_key(&(a, b))
    }

    /// The pair cell of `(a, b)` in band `k`.
    pub fn counter(&self, a: u16, b: u16, k: usize) -> Option<CellId> {
        let i = *self.index.get(&(a, b))?;
        (k < self.bands()).then(|| self.counters[i * self.bands() + k])
    }

    /// The evidence event `value` about the pair cell `(a, b, k)` at `at_ns`, numbered `seq`, for
    /// a medium whose next tick to run is `min_tick`: in the tick of `at_ns` with its offset, or in
    /// `min_tick` at offset 0 when that tick has already run. `None` for a pair or band the store
    /// does not have.
    pub fn evidence_event(
        &self,
        (a, b, k): (u16, u16, usize),
        value: f32,
        at_ns: u64,
        min_tick: u64,
        seq: u32,
    ) -> Option<Event> {
        self.counter(a, b, k)?;
        let len = self.tick_len_ns;
        let (tick, offset_ns) = if at_ns / len >= min_tick {
            (at_ns / len, (at_ns % len) as u32)
        } else {
            (min_tick, 0)
        };
        Some(Event {
            tick,
            offset_ns,
            source: Address {
                domain: self.params.domain,
                node: pair_node(a, b),
                channel: EV_CHANNEL + k as u16,
            },
            tags: Vec::new(),
            value,
            seq,
        })
    }

    /// The level of `(a, b)`'s pair cell in band `k` at the start of `tick`, before any input of
    /// `tick`: its state decayed by its leak over the ticks since it last ran (the integrator's
    /// own rule). Counted as one cell update. `None` for a pair or band the store does not have.
    pub fn level(&mut self, medium: &Medium, a: u16, b: u16, k: usize, tick: u64) -> Option<f32> {
        let id = self.counter(a, b, k)?;
        let cell = medium.cells().get(id.0 as usize)?;
        self.work.cell_updates += 1;
        let dt = cell.last_active.map_or(0, |t| tick.saturating_sub(t));
        Some(sane(cell.state[0] * pow_det(cell.params[0], dt)))
    }

    /// The narrowest band in which the store holds the edge `a -> b` at the start of `tick` (its
    /// level at least the threshold), reading the bands from the narrowest until one holds.
    pub fn held(&mut self, medium: &Medium, a: u16, b: u16, tick: u64) -> Option<usize> {
        (0..self.bands()).find(|&k| {
            self.level(medium, a, b, k, tick)
                .is_some_and(|l| l >= self.params.threshold)
        })
    }

    /// The reads' work since the last call, and forget it.
    pub fn take_work(&mut self) -> OpCounts {
        std::mem::take(&mut self.work)
    }
}

/// The chance that a Poisson process of `rate_per_s` puts at least one instant in a window of
/// `window_ns`: `1 - exp(-rate * w)`. A trial whose follow is worth `1 - q` and whose miss is worth
/// `-q` at this `q` has an expected evidence of zero when it is followed at that chance; its level
/// counts follows beyond chance. Computed with the oscillome's `exp_det` (basic IEEE operations
/// only), rounded once to `f32`; a negative or non-finite rate counts as zero.
pub fn chance(rate_per_s: f64, window_ns: u64) -> f32 {
    let r = if rate_per_s.is_finite() && rate_per_s > 0.0 {
        rate_per_s
    } else {
        0.0
    };
    (1.0 - exp_det(-r * window_ns as f64 / 1.0e9)) as f32
}

/// One open trial: an instant at `a`, waiting for an instant at `b` in band `band`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trial {
    /// The node whose instant opened it.
    pub a: u16,
    /// The node it waits for.
    pub b: u16,
    /// The band.
    pub band: usize,
    /// The instant that opened it, nanoseconds.
    pub at_ns: u64,
    /// The last instant a follow may come at: `at_ns + w_band`.
    pub deadline_ns: u64,
    /// What a follow adds.
    pub follow: f32,
    /// What a miss takes (positive; the evidence is its negation).
    pub miss: f32,
    /// The adapter's name for the instant that opened it (an observation id).
    pub token: u32,
}

/// One trial resolved: a follow (`value` = its follow weight) or a miss (`value` = minus its miss
/// weight).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evidence {
    /// The trial.
    pub trial: Trial,
    /// When it was resolved: the follow's instant, or the deadline.
    pub at_ns: u64,
    /// The signed evidence.
    pub value: f32,
    /// Whether it is a follow.
    pub follow: bool,
    /// The follow's token (the adapter's name for the instant at `b`); 0 for a miss.
    pub by: u32,
}

/// The open trials of a store, in the order they were opened (see the module documentation).
#[derive(Debug, Clone, Default)]
pub struct Trials {
    bands_ns: Vec<u64>,
    open: Vec<Trial>,
}

impl Trials {
    /// No open trial, for these bands.
    pub fn new(bands_ns: Vec<u64>) -> Self {
        Self {
            bands_ns,
            open: Vec::new(),
        }
    }

    /// Open, for partner `b`, one trial per band at `at_ns`, named `token`, with the weights
    /// `(follow, miss)` of each band (a missing one counts as zero).
    pub fn open(&mut self, a: u16, b: u16, at_ns: u64, token: u32, weights: &[(f32, f32)]) {
        for (k, w) in self.bands_ns.iter().enumerate() {
            self.open.push(Trial {
                a,
                b,
                band: k,
                at_ns,
                deadline_ns: at_ns.saturating_add(*w),
                follow: weights.get(k).map_or(0.0, |w| w.0),
                miss: weights.get(k).map_or(0.0, |w| w.1),
                token,
            });
        }
    }

    /// An instant at `b` at `at_ns`, named `token`: for every `a` and band, the latest open trial
    /// `(a, b, band)` with `at < at_ns <= deadline` is followed and closed. The follows, by `a`
    /// then band.
    pub fn follow(&mut self, b: u16, at_ns: u64, token: u32) -> Vec<Evidence> {
        // The latest trial per (a, band) that this instant falls in; ties: the later opened.
        let mut chosen: BTreeMap<(u16, usize), usize> = BTreeMap::new();
        for (i, t) in self.open.iter().enumerate() {
            if t.b == b && t.at_ns < at_ns && at_ns <= t.deadline_ns {
                let e = chosen.entry((t.a, t.band)).or_insert(i);
                if self.open[*e].at_ns <= t.at_ns {
                    *e = i;
                }
            }
        }
        let out: Vec<Evidence> = chosen
            .values()
            .map(|&i| Evidence {
                trial: self.open[i],
                at_ns,
                value: self.open[i].follow,
                follow: true,
                by: token,
            })
            .collect();
        let mut drop: Vec<usize> = chosen.into_values().collect();
        drop.sort_unstable();
        for i in drop.into_iter().rev() {
            self.open.remove(i);
        }
        out
    }

    /// Every open trial whose deadline is before `before_ns`, closed as a miss at its deadline,
    /// in deadline order (then the order opened).
    pub fn expire(&mut self, before_ns: u64) -> Vec<Evidence> {
        let (gone, keep): (Vec<Trial>, Vec<Trial>) = std::mem::take(&mut self.open)
            .into_iter()
            .partition(|t| t.deadline_ns < before_ns);
        self.open = keep;
        let mut out: Vec<Evidence> = gone
            .into_iter()
            .map(|t| Evidence {
                trial: t,
                at_ns: t.deadline_ns,
                value: -t.miss,
                follow: false,
                by: 0,
            })
            .collect();
        // A stable sort keeps the opening order among equal deadlines.
        out.sort_by_key(|e| e.at_ns);
        out
    }

    /// Trials still open.
    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// The open trials, in the order they were opened.
    pub fn open_trials(&self) -> &[Trial] {
        &self.open
    }
}
