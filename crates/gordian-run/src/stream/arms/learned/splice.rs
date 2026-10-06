//! Changing a running medium's cell parameters without losing its state.
//!
//! `gordian-medium` lets the plasticity port change the weights of plastic synapses and nothing
//! else, and the learned arm's quantities are cell parameters (a coincidence window, an emitter's
//! lookback, an integrator's threshold). The crate is another lab's territory, so the arm does it
//! from outside, through the one public interface that carries both: the persisted bytes.
//! [`transplant`] writes the parameters of a freshly built medium (the same graph with new
//! constants) over the parameter words of the old medium's persisted bytes, and restores. State,
//! activations, supports, messages in flight, wakes, the last tick, the totals and the cycle
//! summaries are the old medium's, bit for bit; the parameters are the fresh medium's.
//!
//! The walk over the bytes follows the layout documented at the head of
//! `gordian-medium/src/persist.rs`. It is checked, every time, by decoding the result and
//! comparing it to both media; any disagreement is an error, not a repair, so a change of that
//! layout fails loudly here instead of corrupting a run.
//!
//! The persisted spec must hold no quantity in time (`oscillome.seconds` empty): such a medium
//! stores the converted values and refuses bytes whose stored value differs from the conversion
//! of the stored time, so the learned arm builds its graph with the conversions already applied
//! ([`super::noticing`]).

use gordian_medium::{Medium, P, S};

const MAGIC_AND_VERSION: usize = 5;
const LIMITS: usize = 4 + 4 + 8 + 4 + 2 + 1;
const PRICES: usize = 5 * 8;
const TOTALS: usize = 5 * 8;

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.at = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or("persisted bytes end early")?;
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.bytes.get(self.at).ok_or("persisted bytes end early")?;
        self.at += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self
            .bytes
            .get(self.at..self.at + 2)
            .ok_or("persisted bytes end early")?;
        self.at += 2;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or("persisted bytes end early")?;
        self.at += 4;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// `opt`: a flag byte, then `n` bytes if it is 1.
    fn opt(&mut self, n: usize) -> Result<(), String> {
        match self.u8()? {
            0 => Ok(()),
            1 => self.skip(n),
            _ => Err("bad flag".to_owned()),
        }
    }
}

/// Where each cell's parameter words start in `bytes`, in cell order.
fn param_offsets(bytes: &[u8]) -> Result<Vec<usize>, String> {
    let mut c = Cursor { bytes, at: 0 };
    c.skip(MAGIC_AND_VERSION + LIMITS + PRICES)?;
    c.opt(8)?; // last_tick
    c.skip(TOTALS)?;
    let n = c.u32()? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        c.skip(1)?; // archetype tag
        out.push(c.at);
        c.skip(4 * P + 4 * S + 4)?; // params, state, activation
        c.opt(8)?; // last_active
        if c.u8()? == 1 {
            c.opt(2)?; // domain
            c.opt(2)?; // node
            c.opt(2)?; // channel
            c.opt(4)?; // tag
        }
        let refs = usize::from(c.u16()?);
        c.skip(16 * refs)?;
    }
    Ok(out)
}

/// A medium with `fresh`'s cell parameters and `old`'s everything else. `fresh` must be the same
/// graph as `old` (same cells and synapses in the same order) built with other constants.
pub fn transplant(old: &Medium, fresh: &Medium) -> Result<Medium, String> {
    if old.cells().len() != fresh.cells().len() || old.synapses().len() != fresh.synapses().len() {
        return Err("the fresh medium is not the same graph".to_owned());
    }
    let mut bytes = old.to_bytes();
    let offsets = param_offsets(&bytes)?;
    if offsets.len() != fresh.cells().len() {
        return Err("the byte walk found another number of cells".to_owned());
    }
    for (at, cell) in offsets.iter().zip(fresh.cells()) {
        for (k, x) in cell.params.iter().enumerate() {
            let w = at + 4 * k;
            bytes[w..w + 4].copy_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    let new =
        Medium::from_bytes(&bytes).map_err(|e| format!("transplanted bytes refused: {e:?}"))?;
    for ((n, o), f) in new.cells().iter().zip(old.cells()).zip(fresh.cells()) {
        let same_params = n
            .params
            .iter()
            .zip(&f.params)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        let same_state = n
            .state
            .iter()
            .zip(&o.state)
            .all(|(a, b)| a.to_bits() == b.to_bits())
            && n.activation.to_bits() == o.activation.to_bits()
            && n.last_active == o.last_active
            && n.support == o.support
            && n.archetype == o.archetype
            && n.pattern == o.pattern;
        if !(same_params && same_state) {
            return Err("transplant check failed: a cell differs from its sources".to_owned());
        }
    }
    if new.last_tick() != old.last_tick()
        || new.pending_messages() != old.pending_messages()
        || new.totals() != old.totals()
        || new.synapses() != old.synapses()
    {
        return Err("transplant check failed: the medium's own state differs".to_owned());
    }
    Ok(new)
}
