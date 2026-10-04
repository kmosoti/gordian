//! The append-only event ledger.
//!
//! The ledger records what entered the system and what happened, with provenance. Two rules from
//! the charter are enforced here rather than left to convention:
//!
//! 1. **Perception can itself be inference.** A measured value and an interpretation of it are
//!    different [`EntryKind`]s. The type system will not let a hypothesis be filed as a
//!    measurement.
//! 2. **References are recorded dependencies, not causation.** An entry may cite earlier entries
//!    it was computed from. The ledger checks those citations point backwards; it makes no claim
//!    that they are complete or causal.
//!
//! The ledger stores an opaque `Vec<u8>` payload. Component-private representations stay
//! private; what crosses the boundary is a typed envelope and a byte string whose encoding the
//! producer version declares.

use crate::clock::Instant;

/// Position of an entry in the ledger. Dense, zero-based, never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(pub u64);

/// What an entry asserts. The variants are the charter's distinctions; do not merge them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A value read from a sense: a counter, a message, a snapshot. Makes no interpretation.
    Measurement,
    /// An interpretation or inference about hidden state. May be wrong.
    Hypothesis,
    /// A request to run a computation, with its expected cost.
    ComputationRequest,
    /// The result of a computation, citing its request.
    ComputationResult,
    /// A decision taken against the environment or the task.
    Decision,
    /// An observed consequence of a decision.
    Outcome,
    /// A budget charge or refusal.
    Accounting,
}

/// Who produced an entry and from what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The component that produced the entry.
    pub producer: String,
    /// The producer's version or content hash. Replay compares this, not the name.
    pub producer_version: String,
    /// Earlier entries this one was computed from. Recorded dependencies, not causation.
    pub inputs: Vec<EntryId>,
}

/// One immutable record in the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Position in the ledger.
    pub id: EntryId,
    /// Logical time at which the entry was admitted.
    pub at: Instant,
    /// What the entry asserts.
    pub kind: EntryKind,
    /// Who produced it and from what.
    pub provenance: Provenance,
    /// Opaque payload in the producer's declared encoding.
    pub payload: Vec<u8>,
}

/// Why an append was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// An input citation names an entry that does not exist yet.
    ForwardReference {
        /// The citation.
        cited: EntryId,
        /// The id the new entry would have received.
        next: EntryId,
    },
    /// The entry's time is earlier than the previous entry's time.
    TimeWentBackwards {
        /// Time of the last admitted entry.
        last: Instant,
        /// Time offered for the new entry.
        offered: Instant,
    },
}

/// An append-only sequence of entries.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    entries: Vec<Entry>,
}

impl Ledger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing has been admitted.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry with `id`, if it exists.
    pub fn get(&self, id: EntryId) -> Option<&Entry> {
        usize::try_from(id.0)
            .ok()
            .and_then(|index| self.entries.get(index))
    }

    /// All entries in admission order.
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// Time of the most recently admitted entry.
    pub fn last_instant(&self) -> Option<Instant> {
        self.entries.last().map(|entry| entry.at)
    }

    /// Admit an entry. Returns its id.
    ///
    /// Refuses a citation of an entry that does not exist yet and refuses time running
    /// backwards. Equal times are allowed: several entries may be admitted in one logical step.
    pub fn append(
        &mut self,
        at: Instant,
        kind: EntryKind,
        provenance: Provenance,
        payload: Vec<u8>,
    ) -> Result<EntryId, LedgerError> {
        let next = EntryId(self.entries.len() as u64);
        if let Some(last) = self.last_instant()
            && at < last
        {
            return Err(LedgerError::TimeWentBackwards { last, offered: at });
        }
        if let Some(&cited) = provenance.inputs.iter().find(|input| **input >= next) {
            return Err(LedgerError::ForwardReference { cited, next });
        }
        self.entries.push(Entry {
            id: next,
            at,
            kind,
            provenance,
            payload,
        });
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov(producer: &str, inputs: Vec<EntryId>) -> Provenance {
        Provenance {
            producer: producer.to_owned(),
            producer_version: "test".to_owned(),
            inputs,
        }
    }

    #[test]
    fn ids_are_dense_and_in_admission_order() {
        let mut ledger = Ledger::new();
        let a = ledger
            .append(
                Instant(1),
                EntryKind::Measurement,
                prov("sensor", vec![]),
                b"7".to_vec(),
            )
            .unwrap();
        let b = ledger
            .append(
                Instant(1),
                EntryKind::Measurement,
                prov("sensor", vec![]),
                b"8".to_vec(),
            )
            .unwrap();
        assert_eq!(a, EntryId(0));
        assert_eq!(b, EntryId(1));
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger.get(b).unwrap().payload, b"8");
    }

    #[test]
    fn hypothesis_cites_measurement_but_stays_distinct() {
        let mut ledger = Ledger::new();
        let m = ledger
            .append(
                Instant(1),
                EntryKind::Measurement,
                prov("sensor", vec![]),
                vec![],
            )
            .unwrap();
        let h = ledger
            .append(
                Instant(2),
                EntryKind::Hypothesis,
                prov("analyzer", vec![m]),
                vec![],
            )
            .unwrap();
        assert_eq!(ledger.get(m).unwrap().kind, EntryKind::Measurement);
        assert_eq!(ledger.get(h).unwrap().kind, EntryKind::Hypothesis);
        assert_eq!(ledger.get(h).unwrap().provenance.inputs, vec![m]);
    }

    #[test]
    fn forward_reference_is_refused() {
        let mut ledger = Ledger::new();
        let err = ledger
            .append(
                Instant(1),
                EntryKind::Hypothesis,
                prov("analyzer", vec![EntryId(0)]),
                vec![],
            )
            .unwrap_err();
        assert_eq!(
            err,
            LedgerError::ForwardReference {
                cited: EntryId(0),
                next: EntryId(0),
            }
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn time_cannot_go_backwards() {
        let mut ledger = Ledger::new();
        ledger
            .append(
                Instant(5),
                EntryKind::Measurement,
                prov("sensor", vec![]),
                vec![],
            )
            .unwrap();
        let err = ledger
            .append(
                Instant(4),
                EntryKind::Measurement,
                prov("sensor", vec![]),
                vec![],
            )
            .unwrap_err();
        assert_eq!(
            err,
            LedgerError::TimeWentBackwards {
                last: Instant(5),
                offered: Instant(4),
            }
        );
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn replaying_the_same_appends_yields_an_identical_ledger() {
        let script = |ledger: &mut Ledger| {
            let m = ledger
                .append(
                    Instant(1),
                    EntryKind::Measurement,
                    prov("sensor", vec![]),
                    b"x".to_vec(),
                )
                .unwrap();
            ledger
                .append(
                    Instant(3),
                    EntryKind::Hypothesis,
                    prov("analyzer", vec![m]),
                    b"y".to_vec(),
                )
                .unwrap();
        };
        let mut first = Ledger::new();
        let mut second = Ledger::new();
        script(&mut first);
        script(&mut second);
        assert_eq!(
            first.iter().collect::<Vec<_>>(),
            second.iter().collect::<Vec<_>>()
        );
    }
}
