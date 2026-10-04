//! The phase-attributed cost bill.
//!
//! The charter (section 4) requires that cost include sensing, scheduling, communication, and
//! storage, not only component execution. A [`Bill`] wraps a [`Budget`] and attributes every
//! accepted charge to a [`Phase`], so the same spend can be read as a total per resource or as a
//! breakdown by phase. Every charge in the system is meant to go through a `Bill`, never a bare
//! `Budget`.
//!
//! The bill is also replayable (charter section 11, protocol replay). [`Bill::charge_recorded`]
//! appends one [`EntryKind::Accounting`] entry per attempted charge, accepted or refused, using
//! the fixed binary encoding below, and [`Bill::replay`] rebuilds a bill from those entries.
//!
//! # Accounting payload encoding
//!
//! Version 1, little-endian, no padding:
//!
//! ```text
//! offset  size  field
//! 0       1     version = 1
//! 1       1     phase tag: 0 Sensing, 1 Scheduling, 2 Component, 3 Communication, 4 Storage
//! 2       4     component id (u32), present only when the phase tag is 2
//! .       1     accepted flag: 0 refused, 1 accepted
//! .       4     charge count n (u32)
//! .       9*n   n charges, each: resource tag (u8), amount (u64)
//!               resource tag: 0 Compute, 1 Memory, 2 Time, 3 Probes, 4 Communication
//! ```
//!
//! Decoding rejects an unknown version or tag, a flag other than 0 or 1, truncated input, and
//! trailing bytes. Tags are explicit and never derived from enum discriminants, so reordering
//! the Rust enums cannot silently change the meaning of recorded ledgers.

use std::collections::BTreeMap;
use std::fmt;

use crate::budget::{Budget, BudgetError, Charge, Resource};
use crate::clock::Instant;
use crate::ledger::{EntryId, EntryKind, Ledger, LedgerError, Provenance};

/// Identifies a component for cost attribution. Components themselves arrive in a later item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub u32);

/// The part of the system a charge is attributed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Reading the environment: probes, counters, snapshots.
    Sensing,
    /// Deciding what to run next, including salience estimation.
    Scheduling,
    /// Executing one component.
    Component(ComponentId),
    /// Moving messages between components.
    Communication,
    /// Holding or retrieving state.
    Storage,
}

/// A budget plus the attribution of what has been spent to phases.
///
/// Invariant: for every resource, the sum over phases equals [`Budget::spent`] of the inner
/// budget, provided the budget handed to [`Bill::new`] had nothing spent yet. Spend already on a
/// budget at construction is not attributed to any phase, so give `new` a fresh budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bill {
    budget: Budget,
    per_phase: BTreeMap<Phase, BTreeMap<Resource, u64>>,
}

impl Bill {
    /// A bill with nothing attributed, enforcing `budget`'s limits.
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            per_phase: BTreeMap::new(),
        }
    }

    /// The inner budget, which holds the limits and the running totals.
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// Debit one charge to `phase`, or refuse it and leave the bill exactly as it was.
    pub fn charge(&mut self, phase: Phase, charge: Charge) -> Result<(), BudgetError> {
        self.charge_all(phase, &[charge])
    }

    /// Debit every charge to `phase`, or none of them.
    ///
    /// A refusal leaves both the budget and the attribution untouched. Charges of zero are
    /// checked against the budget like any other but add no row to the attribution.
    pub fn charge_all(&mut self, phase: Phase, charges: &[Charge]) -> Result<(), BudgetError> {
        // `Budget::charge_all` is atomic, so the attribution below only runs after acceptance.
        self.budget.charge_all(charges)?;
        for charge in charges {
            if charge.amount > 0 {
                // Cannot overflow: the sum over phases never exceeds the budget's spend, which
                // the budget keeps at or under its limit.
                *self
                    .per_phase
                    .entry(phase)
                    .or_default()
                    .entry(charge.resource)
                    .or_insert(0) += charge.amount;
            }
        }
        Ok(())
    }

    /// Total accepted spend against `resource`, across all phases.
    pub fn total(&self, resource: Resource) -> u64 {
        self.budget.spent(resource)
    }

    /// Accepted spend against `resource`, per phase, in phase order. Phases with no spend on
    /// `resource` are omitted.
    pub fn by_phase(&self, resource: Resource) -> impl Iterator<Item = (Phase, u64)> + '_ {
        self.per_phase
            .iter()
            .filter_map(move |(phase, spent)| spent.get(&resource).map(|amount| (*phase, *amount)))
    }

    /// Apply a charge atomically and record it in `ledger` as one
    /// [`EntryKind::Accounting`] entry, whether it was accepted or refused.
    ///
    /// Semantics:
    ///
    /// - `Ok(id)`: the charge was accepted, the bill was debited, and entry `id` records it with
    ///   `accepted = true`.
    /// - `Err(Refused { entry, error })`: the budget refused the charge. The bill is unchanged,
    ///   but entry `entry` was still appended with `accepted = false`, so refusals are auditable.
    ///   [`Bill::replay`] ignores such entries.
    /// - `Err(Ledger(error))`: the ledger refused the append. Neither the bill nor the ledger
    ///   changed. A charge is never applied without its record.
    pub fn charge_recorded(
        &mut self,
        ledger: &mut Ledger,
        at: Instant,
        provenance: Provenance,
        phase: Phase,
        charges: &[Charge],
    ) -> Result<EntryId, RecordedChargeError> {
        let mut trial = self.clone();
        let outcome = trial.charge_all(phase, charges);
        let payload = encode_accounting(phase, charges, outcome.is_ok());
        let entry = ledger
            .append(at, EntryKind::Accounting, provenance, payload)
            .map_err(RecordedChargeError::Ledger)?;
        match outcome {
            Ok(()) => {
                *self = trial;
                Ok(entry)
            }
            Err(error) => Err(RecordedChargeError::Refused { entry, error }),
        }
    }

    /// Rebuild a bill from the accepted [`EntryKind::Accounting`] entries of `ledger`, starting
    /// from `budget` (which should be the fresh budget the original bill started with).
    ///
    /// Entries of other kinds and refused accounting entries are skipped. A malformed payload,
    /// or an accepted entry the budget now refuses, means the ledger does not describe a run of
    /// this budget and is reported rather than skipped.
    pub fn replay(budget: Budget, ledger: &Ledger) -> Result<Bill, ReplayError> {
        let mut bill = Bill::new(budget);
        for entry in ledger.iter().filter(|e| e.kind == EntryKind::Accounting) {
            let record =
                decode_accounting(&entry.payload).map_err(|error| ReplayError::Decode {
                    entry: entry.id,
                    error,
                })?;
            if record.accepted {
                bill.charge_all(record.phase, &record.charges)
                    .map_err(|error| ReplayError::Inconsistent {
                        entry: entry.id,
                        error,
                    })?;
            }
        }
        Ok(bill)
    }
}

/// Why [`Bill::charge_recorded`] did not return an accepted entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedChargeError {
    /// The budget refused the charge. The bill is unchanged; the refusal was recorded as `entry`.
    Refused {
        /// The accounting entry that records the refusal.
        entry: EntryId,
        /// Why the budget refused.
        error: BudgetError,
    },
    /// The ledger refused the append. Neither the bill nor the ledger changed.
    Ledger(LedgerError),
}

/// Why [`Bill::replay`] could not rebuild a bill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// An accounting entry's payload could not be decoded.
    Decode {
        /// The offending entry.
        entry: EntryId,
        /// What was wrong with the payload.
        error: DecodeError,
    },
    /// An entry recorded as accepted was refused by the replay budget.
    Inconsistent {
        /// The offending entry.
        entry: EntryId,
        /// Why the budget refused it.
        error: BudgetError,
    },
}

/// One decoded accounting payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountingRecord {
    /// The phase the charges were attributed to.
    pub phase: Phase,
    /// The charges attempted together, in order.
    pub charges: Vec<Charge>,
    /// Whether the budget accepted them.
    pub accepted: bool,
}

/// Why an accounting payload could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The payload ended before a complete record was read.
    Truncated,
    /// The version byte is not one this code understands.
    UnsupportedVersion(u8),
    /// The phase tag is not assigned.
    UnknownPhaseTag(u8),
    /// The resource tag is not assigned.
    UnknownResourceTag(u8),
    /// The accepted flag was neither 0 nor 1.
    InvalidAcceptedFlag(u8),
    /// Bytes remained after the last charge.
    TrailingBytes,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "accounting payload is truncated"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported accounting version {v}"),
            Self::UnknownPhaseTag(t) => write!(f, "unknown phase tag {t}"),
            Self::UnknownResourceTag(t) => write!(f, "unknown resource tag {t}"),
            Self::InvalidAcceptedFlag(b) => write!(f, "invalid accepted flag {b}"),
            Self::TrailingBytes => write!(f, "accounting payload has trailing bytes"),
        }
    }
}

impl std::error::Error for DecodeError {}

const VERSION: u8 = 1;
const CHARGE_LEN: usize = 1 + 8;

fn resource_tag(resource: Resource) -> u8 {
    match resource {
        Resource::Compute => 0,
        Resource::Memory => 1,
        Resource::Time => 2,
        Resource::Probes => 3,
        Resource::Communication => 4,
    }
}

fn resource_from_tag(tag: u8) -> Result<Resource, DecodeError> {
    match tag {
        0 => Ok(Resource::Compute),
        1 => Ok(Resource::Memory),
        2 => Ok(Resource::Time),
        3 => Ok(Resource::Probes),
        4 => Ok(Resource::Communication),
        other => Err(DecodeError::UnknownResourceTag(other)),
    }
}

/// Encode one accounting record in the version-1 format described in the module docs.
///
/// # Panics
///
/// Panics if `charges` has more than `u32::MAX` elements.
pub fn encode_accounting(phase: Phase, charges: &[Charge], accepted: bool) -> Vec<u8> {
    let count = u32::try_from(charges.len()).expect("charge count fits in u32");
    let mut out = Vec::with_capacity(11 + CHARGE_LEN * charges.len());
    out.push(VERSION);
    match phase {
        Phase::Sensing => out.push(0),
        Phase::Scheduling => out.push(1),
        Phase::Component(ComponentId(id)) => {
            out.push(2);
            out.extend_from_slice(&id.to_le_bytes());
        }
        Phase::Communication => out.push(3),
        Phase::Storage => out.push(4),
    }
    out.push(u8::from(accepted));
    out.extend_from_slice(&count.to_le_bytes());
    for charge in charges {
        out.push(resource_tag(charge.resource));
        out.extend_from_slice(&charge.amount.to_le_bytes());
    }
    out
}

/// A cursor over the payload that reports truncation instead of panicking.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let (head, tail) = self
            .rest
            .split_first_chunk::<N>()
            .ok_or(DecodeError::Truncated)?;
        self.rest = tail;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }
}

/// Decode a payload produced by [`encode_accounting`]. Never panics on malformed input.
pub fn decode_accounting(bytes: &[u8]) -> Result<AccountingRecord, DecodeError> {
    let mut reader = Reader { rest: bytes };
    let version = reader.u8()?;
    if version != VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let phase = match reader.u8()? {
        0 => Phase::Sensing,
        1 => Phase::Scheduling,
        2 => Phase::Component(ComponentId(reader.u32()?)),
        3 => Phase::Communication,
        4 => Phase::Storage,
        other => return Err(DecodeError::UnknownPhaseTag(other)),
    };
    let accepted = match reader.u8()? {
        0 => false,
        1 => true,
        other => return Err(DecodeError::InvalidAcceptedFlag(other)),
    };
    let count = reader.u32()? as usize;
    // Check the length before allocating so a hostile count cannot demand a huge Vec.
    let needed = count
        .checked_mul(CHARGE_LEN)
        .ok_or(DecodeError::Truncated)?;
    if reader.rest.len() < needed {
        return Err(DecodeError::Truncated);
    }
    let mut charges = Vec::with_capacity(count);
    for _ in 0..count {
        let resource = resource_from_tag(reader.u8()?)?;
        let amount = reader.u64()?;
        charges.push(Charge { resource, amount });
    }
    if !reader.rest.is_empty() {
        return Err(DecodeError::TrailingBytes);
    }
    Ok(AccountingRecord {
        phase,
        charges,
        accepted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const RESOURCES: [Resource; 5] = [
        Resource::Compute,
        Resource::Memory,
        Resource::Time,
        Resource::Probes,
        Resource::Communication,
    ];

    fn prov() -> Provenance {
        Provenance {
            producer: "bill".to_owned(),
            producer_version: "test".to_owned(),
            inputs: vec![],
        }
    }

    fn budget() -> Budget {
        Budget::new()
            .with_limit(Resource::Compute, 10)
            .with_limit(Resource::Time, 3)
    }

    fn phase_strategy() -> impl Strategy<Value = Phase> {
        prop_oneof![
            Just(Phase::Sensing),
            Just(Phase::Scheduling),
            any::<u32>().prop_map(|id| Phase::Component(ComponentId(id))),
            Just(Phase::Communication),
            Just(Phase::Storage),
        ]
    }

    /// Small component ids so that phases collide often enough to exercise accumulation.
    fn small_phase_strategy() -> impl Strategy<Value = Phase> {
        prop_oneof![
            Just(Phase::Sensing),
            Just(Phase::Scheduling),
            (0u32..3).prop_map(|id| Phase::Component(ComponentId(id))),
            Just(Phase::Communication),
            Just(Phase::Storage),
        ]
    }

    fn resource_strategy() -> impl Strategy<Value = Resource> {
        proptest::sample::select(RESOURCES.to_vec())
    }

    fn charge_strategy(max_amount: u64) -> impl Strategy<Value = Charge> {
        (resource_strategy(), 0..=max_amount).prop_map(|(r, a)| Charge::new(r, a))
    }

    /// Limits for a random subset of resources; the rest stay undeclared so that both refusal
    /// kinds occur. Limits are small enough that sequences regularly hit them.
    fn budget_strategy() -> impl Strategy<Value = Budget> {
        proptest::collection::vec(proptest::option::of(0u64..60), RESOURCES.len()).prop_map(
            |limits| {
                RESOURCES
                    .iter()
                    .zip(limits)
                    .fold(Budget::new(), |budget, (r, limit)| match limit {
                        Some(limit) => budget.with_limit(*r, limit),
                        None => budget,
                    })
            },
        )
    }

    type Step = (Phase, Vec<Charge>);

    fn steps_strategy() -> impl Strategy<Value = Vec<Step>> {
        proptest::collection::vec(
            (
                small_phase_strategy(),
                proptest::collection::vec(charge_strategy(15), 0..4),
            ),
            0..30,
        )
    }

    proptest! {
        #[test]
        fn phase_sums_equal_total_equal_budget_spent(
            budget in budget_strategy(),
            steps in steps_strategy(),
        ) {
            let mut bill = Bill::new(budget);
            for (phase, charges) in &steps {
                // Refusals are expected and fine; the invariant must hold either way.
                let _ = bill.charge_all(*phase, charges);
                for r in RESOURCES {
                    let sum: u64 = bill.by_phase(r).map(|(_, amount)| amount).sum();
                    prop_assert_eq!(sum, bill.total(r));
                    prop_assert_eq!(bill.total(r), bill.budget().spent(r));
                }
            }
        }

        #[test]
        fn refused_charge_all_leaves_bill_unchanged(
            budget in budget_strategy(),
            steps in steps_strategy(),
        ) {
            let mut bill = Bill::new(budget);
            for (phase, charges) in &steps {
                let before = bill.clone();
                if bill.charge_all(*phase, charges).is_err() {
                    prop_assert_eq!(&bill, &before);
                }
            }
        }

        #[test]
        fn refused_single_charge_leaves_bill_unchanged(
            budget in budget_strategy(),
            steps in proptest::collection::vec((small_phase_strategy(), charge_strategy(15)), 0..30),
        ) {
            let mut bill = Bill::new(budget);
            for (phase, charge) in steps {
                let before = bill.clone();
                if bill.charge(phase, charge).is_err() {
                    prop_assert_eq!(&bill, &before);
                }
            }
        }

        #[test]
        fn encode_then_decode_is_identity(
            phase in phase_strategy(),
            charges in proptest::collection::vec(
                (resource_strategy(), any::<u64>()).prop_map(|(r, a)| Charge::new(r, a)),
                0..8,
            ),
            accepted in any::<bool>(),
        ) {
            let bytes = encode_accounting(phase, &charges, accepted);
            let record = decode_accounting(&bytes).unwrap();
            prop_assert_eq!(record, AccountingRecord { phase, charges, accepted });
        }

        #[test]
        fn truncated_input_is_an_error(
            phase in phase_strategy(),
            charges in proptest::collection::vec(charge_strategy(u64::MAX), 0..8),
            accepted in any::<bool>(),
            cut in any::<prop::sample::Index>(),
        ) {
            let bytes = encode_accounting(phase, &charges, accepted);
            let len = cut.index(bytes.len()); // strictly shorter than the full encoding
            prop_assert!(decode_accounting(&bytes[..len]).is_err());
        }

        #[test]
        fn trailing_bytes_are_an_error(
            phase in phase_strategy(),
            charges in proptest::collection::vec(charge_strategy(u64::MAX), 0..8),
            accepted in any::<bool>(),
            extra in proptest::collection::vec(any::<u8>(), 1..8),
        ) {
            let mut bytes = encode_accounting(phase, &charges, accepted);
            bytes.extend_from_slice(&extra);
            prop_assert_eq!(decode_accounting(&bytes), Err(DecodeError::TrailingBytes));
        }

        #[test]
        fn wrong_version_is_an_error(
            phase in phase_strategy(),
            charges in proptest::collection::vec(charge_strategy(u64::MAX), 0..8),
            accepted in any::<bool>(),
            version in any::<u8>().prop_filter("not the current version", |v| *v != VERSION),
        ) {
            let mut bytes = encode_accounting(phase, &charges, accepted);
            bytes[0] = version;
            prop_assert_eq!(
                decode_accounting(&bytes),
                Err(DecodeError::UnsupportedVersion(version))
            );
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = decode_accounting(&bytes);
        }

        #[test]
        fn replay_of_recorded_charges_equals_the_live_bill(
            budget in budget_strategy(),
            steps in steps_strategy(),
        ) {
            let mut live = Bill::new(budget.clone());
            let mut ledger = Ledger::new();
            for (i, (phase, charges)) in steps.iter().enumerate() {
                let result = live.charge_recorded(
                    &mut ledger,
                    Instant(i as u64),
                    prov(),
                    *phase,
                    charges,
                );
                // Exactly one entry per attempt, accepted or refused.
                prop_assert_eq!(ledger.len(), i + 1);
                if let Err(RecordedChargeError::Ledger(e)) = result {
                    return Err(TestCaseError::fail(format!("unexpected ledger error {e:?}")));
                }
            }
            let replayed = Bill::replay(budget, &ledger).unwrap();
            prop_assert_eq!(replayed, live);
        }
    }

    #[test]
    fn charges_accumulate_per_phase() {
        let mut bill = Bill::new(budget());
        bill.charge(Phase::Sensing, Charge::new(Resource::Compute, 2))
            .unwrap();
        bill.charge(Phase::Sensing, Charge::new(Resource::Compute, 3))
            .unwrap();
        bill.charge(Phase::Storage, Charge::new(Resource::Compute, 1))
            .unwrap();
        assert_eq!(
            bill.by_phase(Resource::Compute).collect::<Vec<_>>(),
            vec![(Phase::Sensing, 5), (Phase::Storage, 1)]
        );
        assert_eq!(bill.total(Resource::Compute), 6);
        assert_eq!(bill.by_phase(Resource::Time).count(), 0);
    }

    #[test]
    fn exceeded_charge_is_refused_and_bill_unchanged() {
        let mut bill = Bill::new(budget());
        bill.charge(Phase::Scheduling, Charge::new(Resource::Compute, 8))
            .unwrap();
        let before = bill.clone();
        let err = bill
            .charge(Phase::Sensing, Charge::new(Resource::Compute, 3))
            .unwrap_err();
        assert_eq!(
            err,
            BudgetError::Exceeded {
                resource: Resource::Compute,
                remaining: 2,
                requested: 3,
            }
        );
        assert_eq!(bill, before);
    }

    #[test]
    fn undeclared_resource_is_refused_and_bill_unchanged() {
        let mut bill = Bill::new(budget());
        let before = bill.clone();
        let err = bill
            .charge(Phase::Sensing, Charge::new(Resource::Probes, 0))
            .unwrap_err();
        assert_eq!(err, BudgetError::Undeclared(Resource::Probes));
        assert_eq!(bill, before);
    }

    #[test]
    fn charge_all_is_atomic_across_phase_attribution() {
        let mut bill = Bill::new(budget());
        let before = bill.clone();
        let err = bill
            .charge_all(
                Phase::Component(ComponentId(7)),
                &[
                    Charge::new(Resource::Compute, 5),
                    Charge::new(Resource::Time, 4),
                ],
            )
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetError::Exceeded {
                resource: Resource::Time,
                ..
            }
        ));
        assert_eq!(bill, before);
        assert_eq!(bill.by_phase(Resource::Compute).count(), 0);
    }

    #[test]
    fn charge_all_checks_repeated_resources_cumulatively() {
        let mut bill = Bill::new(budget());
        let result = bill.charge_all(
            Phase::Sensing,
            &[
                Charge::new(Resource::Time, 2),
                Charge::new(Resource::Time, 2),
            ],
        );
        assert!(result.is_err());
        assert_eq!(bill.total(Resource::Time), 0);
    }

    #[test]
    fn zero_charge_adds_no_attribution_row() {
        let mut bill = Bill::new(budget());
        bill.charge(Phase::Sensing, Charge::new(Resource::Compute, 0))
            .unwrap();
        assert_eq!(bill.by_phase(Resource::Compute).count(), 0);
    }

    #[test]
    fn charge_recorded_appends_one_accounting_entry_when_accepted() {
        let mut bill = Bill::new(budget());
        let mut ledger = Ledger::new();
        let charges = [Charge::new(Resource::Compute, 4)];
        let id = bill
            .charge_recorded(&mut ledger, Instant(1), prov(), Phase::Storage, &charges)
            .unwrap();
        assert_eq!(ledger.len(), 1);
        let entry = ledger.get(id).unwrap();
        assert_eq!(entry.kind, EntryKind::Accounting);
        assert_eq!(
            decode_accounting(&entry.payload).unwrap(),
            AccountingRecord {
                phase: Phase::Storage,
                charges: charges.to_vec(),
                accepted: true,
            }
        );
        assert_eq!(bill.total(Resource::Compute), 4);
    }

    #[test]
    fn charge_recorded_records_a_refusal_and_leaves_bill_unchanged() {
        let mut bill = Bill::new(budget());
        let mut ledger = Ledger::new();
        let before = bill.clone();
        let err = bill
            .charge_recorded(
                &mut ledger,
                Instant(1),
                prov(),
                Phase::Sensing,
                &[Charge::new(Resource::Compute, 11)],
            )
            .unwrap_err();
        let RecordedChargeError::Refused { entry, error } = err else {
            panic!("expected a budget refusal, got {err:?}");
        };
        assert!(matches!(error, BudgetError::Exceeded { .. }));
        assert_eq!(bill, before);
        assert_eq!(ledger.len(), 1);
        let record = decode_accounting(&ledger.get(entry).unwrap().payload).unwrap();
        assert!(!record.accepted);
    }

    #[test]
    fn ledger_error_changes_neither_bill_nor_ledger() {
        let mut bill = Bill::new(budget());
        let mut ledger = Ledger::new();
        bill.charge_recorded(
            &mut ledger,
            Instant(5),
            prov(),
            Phase::Sensing,
            &[Charge::new(Resource::Compute, 1)],
        )
        .unwrap();
        let bill_before = bill.clone();
        let err = bill
            .charge_recorded(
                &mut ledger,
                Instant(4),
                prov(),
                Phase::Sensing,
                &[Charge::new(Resource::Compute, 1)],
            )
            .unwrap_err();
        assert_eq!(
            err,
            RecordedChargeError::Ledger(LedgerError::TimeWentBackwards {
                last: Instant(5),
                offered: Instant(4),
            })
        );
        assert_eq!(bill, bill_before);
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn ledger_error_on_a_would_be_refusal_still_changes_nothing() {
        let mut bill = Bill::new(budget());
        let mut ledger = Ledger::new();
        let mut bad = prov();
        bad.inputs = vec![EntryId(9)];
        let err = bill
            .charge_recorded(
                &mut ledger,
                Instant(1),
                bad,
                Phase::Sensing,
                &[Charge::new(Resource::Compute, 99)],
            )
            .unwrap_err();
        assert!(matches!(
            err,
            RecordedChargeError::Ledger(LedgerError::ForwardReference { .. })
        ));
        assert!(ledger.is_empty());
    }

    #[test]
    fn replay_skips_refused_and_non_accounting_entries() {
        let mut bill = Bill::new(budget());
        let mut ledger = Ledger::new();
        ledger
            .append(Instant(0), EntryKind::Measurement, prov(), vec![0xff])
            .unwrap();
        bill.charge_recorded(
            &mut ledger,
            Instant(1),
            prov(),
            Phase::Sensing,
            &[Charge::new(Resource::Compute, 6)],
        )
        .unwrap();
        let _ = bill.charge_recorded(
            &mut ledger,
            Instant(2),
            prov(),
            Phase::Sensing,
            &[Charge::new(Resource::Compute, 6)],
        );
        assert_eq!(ledger.len(), 3);
        assert_eq!(Bill::replay(budget(), &ledger).unwrap(), bill);
    }

    #[test]
    fn replay_reports_a_malformed_payload() {
        let mut ledger = Ledger::new();
        ledger
            .append(Instant(0), EntryKind::Accounting, prov(), vec![1, 9])
            .unwrap();
        assert_eq!(
            Bill::replay(budget(), &ledger).unwrap_err(),
            ReplayError::Decode {
                entry: EntryId(0),
                error: DecodeError::UnknownPhaseTag(9),
            }
        );
    }

    #[test]
    fn replay_reports_an_accepted_entry_the_budget_refuses() {
        let mut ledger = Ledger::new();
        ledger
            .append(
                Instant(0),
                EntryKind::Accounting,
                prov(),
                encode_accounting(Phase::Sensing, &[Charge::new(Resource::Compute, 11)], true),
            )
            .unwrap();
        assert!(matches!(
            Bill::replay(budget(), &ledger).unwrap_err(),
            ReplayError::Inconsistent {
                entry: EntryId(0),
                error: BudgetError::Exceeded { .. },
            }
        ));
    }

    #[test]
    fn encoding_is_the_documented_layout() {
        let bytes = encode_accounting(
            Phase::Component(ComponentId(0x0102_0304)),
            &[Charge::new(Resource::Time, 0x0A)],
            true,
        );
        assert_eq!(
            bytes,
            vec![
                1, // version
                2, 0x04, 0x03, 0x02, 0x01, // phase tag and component id, little-endian
                1,    // accepted
                1, 0, 0, 0, // count
                2, 0x0A, 0, 0, 0, 0, 0, 0, 0, // Time, 10
            ]
        );
    }

    #[test]
    fn decode_error_paths() {
        assert_eq!(decode_accounting(&[]), Err(DecodeError::Truncated));
        assert_eq!(
            decode_accounting(&[2, 0, 1, 0, 0, 0, 0]),
            Err(DecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            decode_accounting(&[1, 5, 1, 0, 0, 0, 0]),
            Err(DecodeError::UnknownPhaseTag(5))
        );
        assert_eq!(
            decode_accounting(&[1, 0, 2, 0, 0, 0, 0]),
            Err(DecodeError::InvalidAcceptedFlag(2))
        );
        let mut unknown_resource = vec![1, 0, 1, 1, 0, 0, 0, 9];
        unknown_resource.extend_from_slice(&[0; 8]);
        assert_eq!(
            decode_accounting(&unknown_resource),
            Err(DecodeError::UnknownResourceTag(9))
        );
        // Component phase with its id cut short.
        assert_eq!(
            decode_accounting(&[1, 2, 1, 2]),
            Err(DecodeError::Truncated)
        );
        let mut trailing = encode_accounting(Phase::Sensing, &[], true);
        trailing.push(0);
        assert_eq!(
            decode_accounting(&trailing),
            Err(DecodeError::TrailingBytes)
        );
    }

    #[test]
    fn hostile_charge_count_is_rejected_without_allocating() {
        let bytes = [1, 0, 1, 0xff, 0xff, 0xff, 0xff];
        assert_eq!(decode_accounting(&bytes), Err(DecodeError::Truncated));
    }

    #[test]
    fn decode_error_displays() {
        assert_eq!(
            DecodeError::UnsupportedVersion(3).to_string(),
            "unsupported accounting version 3"
        );
    }
}
