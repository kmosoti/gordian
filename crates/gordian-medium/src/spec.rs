//! `MediumSpec`, the serializable description a medium is built from (for manifests), and
//! [`MediumBuilder`], a typed way to write one.

use serde::{Deserialize, Serialize};

use crate::archetype::{Archetype, ORDERED_SLOTS, ParamError};
use crate::medium::Medium;
use crate::oscillome::{Conversion, Oscillome, TimeTarget, Timed};
use crate::types::{CellId, F, Gate, Limits, P, Pattern, Prices, S, SynapseId};

/// One cell of a spec. Its id is its position in [`MediumSpec::cells`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellSpec {
    /// Which function the cell runs.
    pub archetype: Archetype,
    /// Its parameters (see the table in the `archetype` module).
    pub params: [f32; P],
    /// The address pattern, for `Sense` cells only (required there, refused elsewhere).
    pub pattern: Option<Pattern>,
}

/// One synapse of a spec. Its id is its position in [`MediumSpec::synapses`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynapseSpec {
    /// Source cell.
    pub from: CellId,
    /// Target cell.
    pub to: CellId,
    /// The message is `weight * activation`.
    pub weight: f32,
    /// Ticks of delay; 0 delivers in the next pass of the same tick.
    pub delay_ticks: u8,
    /// What must be active for the synapse to carry.
    pub gate: Gate,
    /// Whether the plasticity port may change its weight.
    pub plastic: bool,
}

/// A whole medium: cells, synapses, limits, prices and the oscillome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MediumSpec {
    /// Cells, in id order.
    pub cells: Vec<CellSpec>,
    /// Synapses, in id order.
    pub synapses: Vec<SynapseSpec>,
    /// Hard limits.
    pub limits: Limits,
    /// Declared prices.
    pub prices: Prices,
    /// The oscillome (M1b). Off by default, and then absent from the JSON, so that an M1 spec
    /// reads and writes the same text.
    #[serde(default, skip_serializing_if = "Oscillome::is_off")]
    pub oscillome: Oscillome,
}

/// Why a spec was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum SpecError {
    /// More cells than `limits.max_cells`.
    TooManyCells {
        /// Cells in the spec.
        cells: usize,
        /// The limit.
        limit: u32,
    },
    /// More synapses than `limits.max_synapses`.
    TooManySynapses {
        /// Synapses in the spec.
        synapses: usize,
        /// The limit.
        limit: u32,
    },
    /// A limit that would make every tick truncate or every cell useless.
    BadLimits(&'static str),
    /// A cell's parameters.
    BadParam {
        /// The cell.
        cell: CellId,
        /// What is wrong.
        error: ParamError,
    },
    /// A sense cell without a pattern.
    SenseWithoutPattern(CellId),
    /// A pattern on a cell that is not a sense cell.
    PatternOnNonSense(CellId),
    /// A synapse names a cell that does not exist.
    UnknownCell {
        /// The synapse.
        synapse: SynapseId,
        /// The missing cell.
        cell: CellId,
    },
    /// A synapse targets a sense cell, which receives events only.
    SynapseIntoSense(SynapseId),
    /// A synapse's weight is not finite.
    NonFiniteWeight(SynapseId),
    /// A field gate names a scalar that does not exist.
    BadFieldGate(SynapseId),
    /// A coincidence cell has more incoming synapses than it has state slots.
    TooManyInputs {
        /// The cell.
        cell: CellId,
        /// Its incoming synapses.
        inputs: usize,
    },
    /// The oscillome's own structure (M1b).
    BadOscillome(&'static str),
    /// A quantity given in time (M1b): its position in `oscillome.seconds`.
    BadTimed {
        /// Position of the entry.
        index: usize,
        /// What is wrong.
        reason: &'static str,
    },
    /// A phase gate names no rhythm, or its window is not two distinct bounds in `[0, 1]`.
    BadPhaseGate(SynapseId),
    /// A cell's oscillome form does not fit the oscillome or its synapses (M1b).
    BadForm {
        /// The cell.
        cell: CellId,
        /// What is wrong.
        reason: &'static str,
    },
}

impl MediumSpec {
    /// Check the spec against its own limits, the archetypes' rules and its oscillome.
    pub fn validate(&self) -> Result<(), SpecError> {
        self.resolved().map(|_| ())
    }

    /// The spec with every quantity given in time converted for its tick length (decision 2),
    /// validated, and the conversions done, in the order of `oscillome.seconds`. A medium is
    /// built from the resolved spec; resolving a resolved spec changes nothing.
    pub fn resolved(&self) -> Result<(MediumSpec, Vec<Conversion>), SpecError> {
        self.oscillome.validate()?;
        let mut spec = self.clone();
        let conversions = self.oscillome.apply(&mut spec.cells, &mut spec.synapses)?;
        spec.validate_structure()?;
        Ok((spec, conversions))
    }

    /// The conversion table of this spec at its tick length.
    pub fn conversions(&self) -> Result<Vec<Conversion>, SpecError> {
        self.resolved().map(|(_, c)| c)
    }

    /// Whether the spec uses any element of the oscillome: an oscillome that is not off, a phase
    /// gate, or an oscillome form of a cell. A spec that uses none is an M1 spec, and its medium
    /// persists in M1's encoding (decision 9).
    pub fn uses_oscillome(&self) -> bool {
        !self.oscillome.is_off()
            || self
                .cells
                .iter()
                .any(|c| c.archetype.uses_oscillome(&c.params))
            || self
                .synapses
                .iter()
                .any(|s| matches!(s.gate, Gate::Phase { .. }))
    }

    fn validate_structure(&self) -> Result<(), SpecError> {
        let l = &self.limits;
        if self.cells.len() > l.max_cells as usize {
            return Err(SpecError::TooManyCells {
                cells: self.cells.len(),
                limit: l.max_cells,
            });
        }
        if self.synapses.len() > l.max_synapses as usize {
            return Err(SpecError::TooManySynapses {
                synapses: self.synapses.len(),
                limit: l.max_synapses,
            });
        }
        if l.max_passes == 0 {
            return Err(SpecError::BadLimits("max_passes must be at least 1"));
        }
        if l.max_refs == 0 {
            return Err(SpecError::BadLimits("max_refs must be at least 1"));
        }
        for (i, cell) in self.cells.iter().enumerate() {
            let id = CellId(i as u32);
            cell.archetype
                .validate(&cell.params)
                .map_err(|error| SpecError::BadParam { cell: id, error })?;
            match (cell.archetype, cell.pattern) {
                (Archetype::Sense, None) => return Err(SpecError::SenseWithoutPattern(id)),
                (a, Some(_)) if a != Archetype::Sense => {
                    return Err(SpecError::PatternOnNonSense(id));
                }
                _ => {}
            }
        }
        let rhythms = self.oscillome.periods_ns.len();
        let mut incoming = vec![0usize; self.cells.len()];
        let mut self_loops: Vec<Vec<SynapseId>> = vec![Vec::new(); self.cells.len()];
        for (i, syn) in self.synapses.iter().enumerate() {
            let id = SynapseId(i as u32);
            for cell in [syn.from, syn.to] {
                if cell.0 as usize >= self.cells.len() {
                    return Err(SpecError::UnknownCell { synapse: id, cell });
                }
            }
            if self.cells[syn.to.0 as usize].archetype == Archetype::Sense {
                return Err(SpecError::SynapseIntoSense(id));
            }
            if !syn.weight.is_finite() {
                return Err(SpecError::NonFiniteWeight(id));
            }
            match syn.gate {
                Gate::Field(k) if k as usize >= F => return Err(SpecError::BadFieldGate(id)),
                Gate::Cell(c) if c.0 as usize >= self.cells.len() => {
                    return Err(SpecError::UnknownCell {
                        synapse: id,
                        cell: c,
                    });
                }
                Gate::Phase { rhythm, from, to } => {
                    let bound = |x: f32| (0.0..=1.0).contains(&x);
                    if usize::from(rhythm) >= rhythms || !bound(from) || !bound(to) || from == to {
                        return Err(SpecError::BadPhaseGate(id));
                    }
                }
                _ => {}
            }
            incoming[syn.to.0 as usize] += 1;
            if syn.from == syn.to {
                self_loops[syn.to.0 as usize].push(id);
            }
        }
        for (i, cell) in self.cells.iter().enumerate() {
            if cell.archetype == Archetype::Coincidence && incoming[i] > S {
                return Err(SpecError::TooManyInputs {
                    cell: CellId(i as u32),
                    inputs: incoming[i],
                });
            }
        }
        for (i, cell) in self.cells.iter().enumerate() {
            self.validate_form(CellId(i as u32), cell, incoming[i], &self_loops[i])?;
        }
        Ok(())
    }

    /// The oscillome forms' requirements on the oscillome and on a cell's synapses.
    fn validate_form(
        &self,
        id: CellId,
        cell: &CellSpec,
        incoming: usize,
        self_loops: &[SynapseId],
    ) -> Result<(), SpecError> {
        let o = &self.oscillome;
        let bad = |reason| Err(SpecError::BadForm { cell: id, reason });
        let p = &cell.params;
        // A sub-tick lookback (M3) reads event times: tick * tick length + offset.
        if cell.archetype.sub_tick_lookback_us(p).is_some() && o.tick_len_ns == 0 {
            return bad("a sub-tick lookback needs the oscillome's tick length");
        }
        match cell.archetype {
            Archetype::Coincidence if p[4] == 1.0 => {
                let r = p[5] as usize;
                let Some(&period) = o.periods_ns.get(r) else {
                    return bad("a binned coincidence names no rhythm");
                };
                // A bin may not be shorter than the tick.
                if u128::from(period) < u128::from(o.tick_len_ns) * (p[6] as u128) {
                    return bad("bins shorter than the tick");
                }
            }
            Archetype::Coincidence if p[4] == 2.0 => {
                if o.tick_len_ns == 0 || !o.tick_len_ns.is_multiple_of(1_000) {
                    return bad("an ordered coincidence needs a tick length in whole microseconds");
                }
                if incoming > ORDERED_SLOTS {
                    return Err(SpecError::TooManyInputs {
                        cell: id,
                        inputs: incoming,
                    });
                }
            }
            Archetype::Oscillator => {
                let [sid] = self_loops else {
                    return bad("an oscillator needs exactly one self-synapse");
                };
                let s = &self.synapses[sid.0 as usize];
                if s.gate != Gate::None || s.plastic || s.delay_ticks == 0 {
                    return bad("an oscillator's self-synapse is ungated, fixed, with a delay");
                }
                if !(s.weight > 0.0 && s.weight <= 1.0) {
                    return bad("an oscillator's decay per cycle (self weight) outside (0, 1]");
                }
                if p[2] == 0.0 && !(s.weight < 1.0 && p[1] > 0.0) {
                    return bad("an oscillator must stop: a cycle limit, or decay to a floor");
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// How a sense cell turns its events into activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseMode {
    /// 1 whenever at least one event arrived.
    Presence,
    /// The sum of the events' values.
    Sum,
    /// The number of events.
    Count,
}

/// Builds a [`MediumSpec`] cell by cell. Ids are handed out densely in call order.
#[derive(Debug, Clone, Default)]
pub struct MediumBuilder {
    spec: MediumSpec,
}

fn params(values: &[f32]) -> [f32; P] {
    let mut p = [0.0; P];
    p[..values.len()].copy_from_slice(values);
    p
}

impl MediumBuilder {
    /// An empty spec with the default limits and the declared prices.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the limits.
    pub fn limits(mut self, limits: Limits) -> Self {
        self.spec.limits = limits;
        self
    }

    /// Set the prices.
    pub fn prices(mut self, prices: Prices) -> Self {
        self.spec.prices = prices;
        self
    }

    /// Set the oscillome (its `seconds` entries are kept, and [`MediumBuilder::timed`] adds to
    /// them).
    pub fn oscillome(mut self, oscillome: Oscillome) -> Self {
        let seconds = std::mem::take(&mut self.spec.oscillome.seconds);
        self.spec.oscillome = oscillome;
        self.spec.oscillome.seconds.splice(0..0, seconds);
        self
    }

    /// Give `target` in time: `ns` nanoseconds (a duration, or a time constant for a decay or a
    /// rate), converted when the medium is built.
    pub fn timed(&mut self, target: TimeTarget, ns: u64) {
        self.spec.oscillome.seconds.push(Timed { target, ns });
    }

    /// Add a cell with raw parameters. Validation happens in [`MediumBuilder::build`].
    pub fn cell(
        &mut self,
        archetype: Archetype,
        params: [f32; P],
        pattern: Option<Pattern>,
    ) -> CellId {
        let id = CellId(self.spec.cells.len() as u32);
        self.spec.cells.push(CellSpec {
            archetype,
            params,
            pattern,
        });
        id
    }

    /// A sense cell.
    pub fn sense(&mut self, pattern: Pattern, mode: SenseMode) -> CellId {
        let m = match mode {
            SenseMode::Presence => 0.0,
            SenseMode::Sum => 1.0,
            SenseMode::Count => 2.0,
        };
        self.cell(Archetype::Sense, params(&[m]), Some(pattern))
    }

    /// An integrator. `reset`: back to zero on firing (else it only decays).
    pub fn integrator(&mut self, leak: f32, threshold: f32, reset: bool, lookback: u32) -> CellId {
        let r = if reset { 0.0 } else { 1.0 };
        self.cell(
            Archetype::Integrator,
            params(&[leak, threshold, r, lookback as f32]),
            None,
        )
    }

    /// A novelty cell. `gaps_are_zero`: each silent tick counts as a zero input.
    pub fn novelty(
        &mut self,
        rate: f32,
        k: f32,
        floor: f32,
        warmup: u32,
        gaps_are_zero: bool,
    ) -> CellId {
        let g = if gaps_are_zero { 1.0 } else { 0.0 };
        self.cell(
            Archetype::Novelty,
            params(&[rate, k, floor, warmup as f32, g]),
            None,
        )
    }

    /// A coincidence cell.
    pub fn coincidence(&mut self, n: u8, window: u32, consume: bool, lookback: u32) -> CellId {
        let c = if consume { 1.0 } else { 0.0 };
        self.cell(
            Archetype::Coincidence,
            params(&[f32::from(n), window as f32, c, lookback as f32]),
            None,
        )
    }

    /// A coincidence binned by rhythm `rhythm` cut into `bins_per_cycle` bins: arrivals coincide
    /// when at most `window` bins apart (0: in the same bin).
    pub fn coincidence_binned(
        &mut self,
        n: u8,
        rhythm: u8,
        bins_per_cycle: u32,
        window: u32,
        consume: bool,
        lookback: u32,
    ) -> CellId {
        let c = if consume { 1.0 } else { 0.0 };
        self.cell(
            Archetype::Coincidence,
            params(&[
                f32::from(n),
                window as f32,
                c,
                lookback as f32,
                1.0,
                f32::from(rhythm),
                bins_per_cycle as f32,
            ]),
            None,
        )
    }

    /// A coincidence ordered by event time: arrivals coincide when the events they cite are at
    /// most `window_us` microseconds apart; with `lead`, the source on its first incoming synapse
    /// must be the earliest. At most four incoming synapses.
    pub fn coincidence_ordered(
        &mut self,
        n: u8,
        window_us: u32,
        lead: bool,
        consume: bool,
        lookback: u32,
    ) -> CellId {
        let c = if consume { 1.0 } else { 0.0 };
        let l = if lead { 1.0 } else { 0.0 };
        self.cell(
            Archetype::Coincidence,
            params(&[f32::from(n), window_us as f32, c, lookback as f32, 2.0, l]),
            None,
        )
    }

    /// Set parameter `index` of `cell` (for the parameters no constructor takes: an ordered
    /// coincidence's arrivals at event resolution, parameter 6, and the sub-tick lookback of an
    /// integrator or a coincidence, parameter 7, which [`MediumBuilder::timed`] can also give in
    /// time; M3). Validation happens when the spec is built. Does nothing for a cell or index
    /// that does not exist.
    pub fn set_param(&mut self, cell: CellId, index: usize, value: f32) {
        if let Some(p) = self
            .spec
            .cells
            .get_mut(cell.0 as usize)
            .and_then(|c| c.params.get_mut(index))
        {
            *p = value;
        }
    }

    /// A latch that proposes `retire` (of `kind`) when its hold expires.
    pub fn latch_retiring(&mut self, threshold: f32, hold: u32, kind: u16) -> CellId {
        self.cell(
            Archetype::Latch,
            params(&[threshold, hold as f32, 1.0, f32::from(kind)]),
            None,
        )
    }

    /// An oscillator and its self-synapse (its clock): period `period_ticks`, amplitude
    /// multiplied by `decay` each cycle, stopping below `floor` or after `cycles` cycles (0: no
    /// limit). Give the period in time with [`MediumBuilder::timed`] on the returned synapse.
    pub fn oscillator(
        &mut self,
        threshold: f32,
        floor: f32,
        cycles: u32,
        decay: f32,
        period_ticks: u8,
    ) -> (CellId, SynapseId) {
        let id = self.cell(
            Archetype::Oscillator,
            params(&[threshold, floor, cycles as f32]),
            None,
        );
        let clock = self.synapse(id, id, decay, period_ticks);
        (id, clock)
    }

    /// A gate on field scalar `index`. `open_above`: open while the scalar is at or above the
    /// threshold (else while it is below).
    pub fn gate(&mut self, index: u8, threshold: f32, open_above: bool) -> CellId {
        let s = if open_above { 0.0 } else { 1.0 };
        self.cell(
            Archetype::Gate,
            params(&[f32::from(index), threshold, s]),
            None,
        )
    }

    /// A latch.
    pub fn latch(&mut self, threshold: f32, hold: u32) -> CellId {
        self.cell(Archetype::Latch, params(&[threshold, hold as f32]), None)
    }

    /// An emitter.
    pub fn emit(&mut self, threshold: f32, kind: u16, lookback: u32, refractory: u32) -> CellId {
        self.cell(
            Archetype::Emit,
            params(&[
                threshold,
                f32::from(kind),
                lookback as f32,
                refractory as f32,
            ]),
            None,
        )
    }

    /// An ungated, non-plastic synapse.
    pub fn synapse(&mut self, from: CellId, to: CellId, weight: f32, delay_ticks: u8) -> SynapseId {
        self.synapse_with(SynapseSpec {
            from,
            to,
            weight,
            delay_ticks,
            gate: Gate::None,
            plastic: false,
        })
    }

    /// Any synapse.
    pub fn synapse_with(&mut self, synapse: SynapseSpec) -> SynapseId {
        let id = SynapseId(self.spec.synapses.len() as u32);
        self.spec.synapses.push(synapse);
        id
    }

    /// The spec so far.
    pub fn spec(&self) -> &MediumSpec {
        &self.spec
    }

    /// Finish the spec.
    pub fn into_spec(self) -> MediumSpec {
        self.spec
    }

    /// Validate and build.
    pub fn build(self) -> Result<Medium, SpecError> {
        Medium::from_spec(&self.spec)
    }
}
