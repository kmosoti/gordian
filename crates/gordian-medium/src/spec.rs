//! `MediumSpec`, the serializable description a medium is built from (for manifests), and
//! [`MediumBuilder`], a typed way to write one.

use serde::{Deserialize, Serialize};

use crate::archetype::{Archetype, ParamError};
use crate::medium::Medium;
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

/// A whole medium: cells, synapses, limits and prices.
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
}

impl MediumSpec {
    /// Check the spec against its own limits and the archetypes' rules.
    pub fn validate(&self) -> Result<(), SpecError> {
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
        let mut incoming = vec![0usize; self.cells.len()];
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
                _ => {}
            }
            incoming[syn.to.0 as usize] += 1;
        }
        for (i, cell) in self.cells.iter().enumerate() {
            if cell.archetype == Archetype::Coincidence && incoming[i] > S {
                return Err(SpecError::TooManyInputs {
                    cell: CellId(i as u32),
                    inputs: incoming[i],
                });
            }
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
