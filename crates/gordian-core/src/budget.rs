//! Explicit resource accounting.
//!
//! The charter requires hard resource limits in every experimental condition and a complete bill
//! that includes sensing, scheduling, and communication, not only component execution. A
//! [`Budget`] is that bill. It refuses a charge that would exceed a limit and records nothing
//! on refusal, so an exhausted budget is a stable fact rather than a negative number.

use std::collections::BTreeMap;

/// A resource the budget tracks. Units are declared per experiment, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    /// Compute, in whatever unit the experiment declares (operations, ns, tokens).
    Compute,
    /// Peak or integrated memory, in bytes or byte-seconds as declared.
    Memory,
    /// Wall-clock or logical time consumed, in nanoseconds.
    Time,
    /// Diagnostic probes issued against the environment.
    Probes,
    /// Messages or bytes exchanged between components.
    Communication,
}

/// One debit against one resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charge {
    /// The resource debited.
    pub resource: Resource,
    /// The amount, in the resource's declared unit.
    pub amount: u64,
}

impl Charge {
    /// A charge of `amount` against `resource`.
    pub fn new(resource: Resource, amount: u64) -> Self {
        Self { resource, amount }
    }
}

/// Why a charge was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// Applying the charge would take the resource past its limit. Nothing was debited.
    Exceeded {
        /// The resource whose limit would be crossed.
        resource: Resource,
        /// Amount still available before the charge.
        remaining: u64,
        /// Amount requested.
        requested: u64,
    },
    /// The resource has no declared limit, so it cannot be charged.
    Undeclared(Resource),
}

/// A set of hard limits and the spend against them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Budget {
    limits: BTreeMap<Resource, u64>,
    spent: BTreeMap<Resource, u64>,
}

impl Budget {
    /// An empty budget. Every resource is undeclared until [`Budget::with_limit`] declares it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a hard limit for `resource`.
    pub fn with_limit(mut self, resource: Resource, limit: u64) -> Self {
        self.limits.insert(resource, limit);
        self.spent.entry(resource).or_insert(0);
        self
    }

    /// The declared limit, if any.
    pub fn limit(&self, resource: Resource) -> Option<u64> {
        self.limits.get(&resource).copied()
    }

    /// Total debited so far against `resource`. Zero for undeclared resources.
    pub fn spent(&self, resource: Resource) -> u64 {
        self.spent.get(&resource).copied().unwrap_or(0)
    }

    /// Amount still available, if the resource is declared.
    pub fn remaining(&self, resource: Resource) -> Option<u64> {
        self.limit(resource)
            .map(|limit| limit.saturating_sub(self.spent(resource)))
    }

    /// Apply a charge, or refuse it without changing anything.
    pub fn charge(&mut self, charge: Charge) -> Result<(), BudgetError> {
        let remaining = self
            .remaining(charge.resource)
            .ok_or(BudgetError::Undeclared(charge.resource))?;
        if charge.amount > remaining {
            return Err(BudgetError::Exceeded {
                resource: charge.resource,
                remaining,
                requested: charge.amount,
            });
        }
        *self.spent.entry(charge.resource).or_insert(0) += charge.amount;
        Ok(())
    }

    /// Apply every charge or none of them.
    ///
    /// A computation that bills several resources must not be half-charged when one of them
    /// runs out; the caller would otherwise be unable to tell a refused computation from a
    /// partially executed one.
    pub fn charge_all(&mut self, charges: &[Charge]) -> Result<(), BudgetError> {
        let mut trial = self.clone();
        for charge in charges {
            trial.charge(*charge)?;
        }
        *self = trial;
        Ok(())
    }

    /// True once any declared resource is fully spent.
    pub fn exhausted(&self) -> bool {
        self.limits
            .keys()
            .any(|resource| self.remaining(*resource) == Some(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_within_limit_is_recorded() {
        let mut budget = Budget::new().with_limit(Resource::Compute, 10);
        assert!(budget.charge(Charge::new(Resource::Compute, 4)).is_ok());
        assert_eq!(budget.spent(Resource::Compute), 4);
        assert_eq!(budget.remaining(Resource::Compute), Some(6));
        assert!(!budget.exhausted());
    }

    #[test]
    fn charge_past_limit_is_refused_and_records_nothing() {
        let mut budget = Budget::new().with_limit(Resource::Probes, 2);
        budget.charge(Charge::new(Resource::Probes, 2)).unwrap();
        let err = budget.charge(Charge::new(Resource::Probes, 1)).unwrap_err();
        assert_eq!(
            err,
            BudgetError::Exceeded {
                resource: Resource::Probes,
                remaining: 0,
                requested: 1,
            }
        );
        assert_eq!(budget.spent(Resource::Probes), 2);
        assert!(budget.exhausted());
    }

    #[test]
    fn undeclared_resource_cannot_be_charged() {
        let mut budget = Budget::new().with_limit(Resource::Compute, 10);
        assert_eq!(
            budget.charge(Charge::new(Resource::Memory, 1)),
            Err(BudgetError::Undeclared(Resource::Memory))
        );
        assert_eq!(budget.spent(Resource::Memory), 0);
    }

    #[test]
    fn charge_all_is_atomic() {
        let mut budget = Budget::new()
            .with_limit(Resource::Compute, 10)
            .with_limit(Resource::Time, 1);
        let before = budget.clone();
        let result = budget.charge_all(&[
            Charge::new(Resource::Compute, 5),
            Charge::new(Resource::Time, 2),
        ]);
        assert!(matches!(
            result,
            Err(BudgetError::Exceeded {
                resource: Resource::Time,
                ..
            })
        ));
        assert_eq!(budget, before);
    }

    #[test]
    fn zero_limit_is_exhausted_immediately() {
        let budget = Budget::new().with_limit(Resource::Communication, 0);
        assert!(budget.exhausted());
    }
}
