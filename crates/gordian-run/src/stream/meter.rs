//! The accounting boundary the arms work through.
//!
//! The episode harness charges every component call and every scheduling call against the bill,
//! times each at the boundary, counts its operations, and records all of it in the ledger. An arm
//! of the stream harness does a variable amount of such work at every step (any number of live
//! anomalies, each with its own components and rule), so it cannot return a list of components
//! for the harness to run, as an episode policy does. It is handed a [`Meter`] instead, and the
//! meter is the only way it can run a component or the rule: **every call is charged before it
//! runs, refused without side effects if the bill cannot pay, timed with the harness's monotonic
//! clock, counted in the units of `gordian-components` and `policy::decide`, and recorded**, with
//! the same ledger entries (`harness/timer` measurements carrying `ops`) the episode harness
//! writes. An arm never sees the bill, a timing or a count.
//!
//! What is metered: components, and the shared rule's `decide` and final calls. What is not: the
//! arm's own bookkeeping (the anomaly tracker, scores, the context builder), which the harness
//! charges as one declared cost per step (placeholders, `Arm::declared_step_cost`) and measures
//! in `measured_sched_ns`, but which has no calibrated counted-operation weights and is therefore
//! not part of the modelled cost (`stream/mod.rs`, "What is costed").

use crate::harness::HarnessError;
use crate::harness::{Charged, append, busy_ns, charge, record_timing, timed};
use crate::policy::decide::{Decider, Remaining, RuleOps};
use gordian_components::{Component, ComponentOutput, Ops, WorkingState};
use gordian_core::{Bill, ComponentId, Instant, Ledger, ManualClock, Phase};
use gordian_world::Action;

/// The sums the meter keeps for one segment.
#[derive(Debug, Clone)]
pub struct Totals {
    /// Per component of the run, in the order the run lists them: counted operations.
    pub components: Vec<Ops>,
    /// The shared rule's counted operations.
    pub sched: RuleOps,
    /// Component calls that were charged and run.
    pub components_run: u32,
    /// Component calls the bill refused.
    pub components_skipped: u32,
    /// Rule calls (not final) the bill refused, so the rule did not run.
    pub rule_skipped: u32,
    /// Wall time of component runs, nanoseconds.
    pub component_ns: u64,
    /// Wall time of rule calls, nanoseconds.
    pub rule_ns: u64,
}

impl Totals {
    /// Empty sums over the given components.
    pub fn new(ids: &[ComponentId]) -> Self {
        Self {
            components: ids.iter().map(|id| Ops::zero(*id)).collect(),
            sched: RuleOps::ZERO,
            components_run: 0,
            components_skipped: 0,
            rule_skipped: 0,
            component_ns: 0,
            rule_ns: 0,
        }
    }
}

/// The result of a metered rule call.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleCall {
    /// The bill refused the call's declared cost and it was not a final call: the rule did not run.
    NotRun,
    /// The rule ran and returned this action (or nothing).
    Ran(Option<Action>),
}

/// What an arm works through. Created by the harness for one step; see the module documentation.
pub struct Meter<'a> {
    bill: &'a mut Bill,
    ledger: &'a mut Ledger,
    clock: &'a mut ManualClock,
    totals: &'a mut Totals,
    producer: &'a str,
    error: Option<HarnessError>,
}

impl<'a> Meter<'a> {
    /// A meter over the harness's bill, ledger, clock and sums.
    pub fn new(
        bill: &'a mut Bill,
        ledger: &'a mut Ledger,
        clock: &'a mut ManualClock,
        totals: &'a mut Totals,
        producer: &'a str,
    ) -> Self {
        Self {
            bill,
            ledger,
            clock,
            totals,
            producer,
            error: None,
        }
    }

    /// The harness defect a call hit, if any. A meter that has hit one refuses all later work,
    /// and the harness fails the run when the step returns.
    pub fn take_error(&mut self) -> Option<HarnessError> {
        self.error.take()
    }

    fn fail(&mut self, e: HarnessError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }

    /// The current logical time.
    pub fn now(&self) -> Instant {
        self.clock.now()
    }

    /// What is left to spend of the two resources probes use.
    pub fn remaining(&self) -> Remaining {
        Remaining::of(self.bill)
    }

    /// Run `component` on `state` if the bill accepts its declared cost, charging first.
    ///
    /// Returns `None` when the bill refused the charge (the refusal is recorded and the component
    /// did not run) or after a harness defect. Otherwise the clock advances by the component's
    /// busy time, the component runs timed, its operation count is added to the sums, and its
    /// entries are recorded.
    pub fn run_component(
        &mut self,
        component: &mut dyn Component,
        state: &WorkingState,
    ) -> Option<ComponentOutput> {
        if self.error.is_some() {
            return None;
        }
        let id = component.id();
        let cost = component.declared_cost(state);
        let producer = format!("component/{}", id.0);
        let accounting = match charge(
            self.bill,
            self.ledger,
            self.clock.now(),
            &producer,
            Phase::Component(id),
            &cost,
        ) {
            Ok(Charged::Accepted(accounting)) => accounting,
            Ok(Charged::Refused(_)) => {
                self.totals.components_skipped += 1;
                return None;
            }
            Err(e) => {
                self.fail(e);
                return None;
            }
        };
        self.totals.components_run += 1;
        self.clock.advance(busy_ns(&cost));
        let ((output, counted), ns) = timed(|| component.run_counted(state));
        if let Some(sum) = self
            .totals
            .components
            .iter_mut()
            .find(|o| o.component() == id)
        {
            sum.accumulate(&counted);
        }
        self.totals.component_ns = self.totals.component_ns.saturating_add(ns);
        let recorded = record_timing(
            self.ledger,
            self.clock.now(),
            "component",
            Some(id),
            ns,
            counted.counts(),
            vec![accounting],
        )
        .and_then(|()| {
            for (kind, body) in &output.entries {
                append(
                    self.ledger,
                    self.clock.now(),
                    *kind,
                    producer.clone(),
                    vec![accounting],
                    body.clone(),
                )?;
            }
            Ok(())
        });
        if let Err(e) = recorded {
            self.fail(e);
            return None;
        }
        Some(output)
    }

    /// Call the shared rule on `outputs` (`decide`) or as its final call, charging its declared
    /// cost first under `Phase::Scheduling`.
    ///
    /// A refused charge means the rule does not run ([`RuleCall::NotRun`]) unless this is a
    /// final call, which is made anyway: declaring is free, and making the verdict depend on
    /// whether the last few microseconds of compute remained would reintroduce the artefact the
    /// final call exists to remove (`HARNESS.md`, section 1). The refusal is recorded either way.
    pub fn rule_call(
        &mut self,
        decider: &mut Decider,
        state: &WorkingState,
        outputs: &[(ComponentId, ComponentOutput)],
        final_call: bool,
    ) -> RuleCall {
        if self.error.is_some() {
            return RuleCall::NotRun;
        }
        decider.note_remaining(Remaining::of(self.bill));
        let cost = vec![if final_call {
            decider.declared_final_cost(state)
        } else {
            decider.declared_cost(state)
        }];
        let (accounting, accepted) = match charge(
            self.bill,
            self.ledger,
            self.clock.now(),
            self.producer,
            Phase::Scheduling,
            &cost,
        ) {
            Ok(Charged::Accepted(entry)) => (entry, true),
            Ok(Charged::Refused(entry)) => (entry, false),
            Err(e) => {
                self.fail(e);
                return RuleCall::NotRun;
            }
        };
        if !accepted && !final_call {
            self.totals.rule_skipped += 1;
            return RuleCall::NotRun;
        }
        let (action, ns) = timed(|| {
            if final_call {
                decider.decide_final(state)
            } else {
                decider.decide(state, outputs)
            }
        });
        let ops = decider.take_ops();
        self.totals.sched.accumulate(&ops);
        self.totals.rule_ns = self.totals.rule_ns.saturating_add(ns);
        if let Err(e) = record_timing(
            self.ledger,
            self.clock.now(),
            "decide",
            None,
            ns,
            ops.counts(),
            vec![accounting],
        ) {
            self.fail(e);
        }
        RuleCall::Ran(action)
    }
}
