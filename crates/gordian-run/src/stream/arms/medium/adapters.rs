//! The medium's ports on the stream world (work item M2, build items 1 and 3): sense, clock and
//! ledger. The effector is in [`super::noticing`], where proposals become notices.
//!
//! # Sense: one observation, one event
//!
//! Every delivered passive observation becomes one [`Event`], **benign readings included**, with
//! its value (the synthesis decision in `docs/review-log.md`: a slow leak is benign under the
//! public rules until it crosses the alarm line, so a sense adapter that dropped benign readings
//! would hand the medium the public noticers' blindness). [`encode`] is the mapping:
//!
//! | Observation | address `(domain, node, channel)` | value | tags |
//! |---|---|---|---|
//! | `Counter { service, name, value }` | `(0, service, CH_COUNTER)` | the reading | verdict, counter name, (abnormal) kind |
//! | `Message { service, text_id, severity }` | `(0, service, CH_MESSAGE)` | 1 | verdict, message id (folded), severity, (abnormal) kind |
//! | `Snapshot { service, config_hash }` | `(0, service, CH_SNAPSHOT)` | 1 | verdict, (abnormal) kind |
//! | `Probed`, `Correction` | none: not delivered through the noticing seam | | |
//!
//! - An abnormal observation also carries its **kind** ([`abnormal_kind_tag`]: which counter, a
//!   message, a snapshot), so that a sense cell can select abnormal readings of one kind.
//! - The **verdict** tag is the public rules' ([`super::super::rung::is_abnormal`], carried in
//!   [`Held::abnormal`]): [`TAG_ABNORMAL`] or [`TAG_BENIGN`].
//! - The **node** is the service's index; the **channel** says which kind of observation it is
//!   (counter, message, snapshot, probe). A probe result never reaches a noticer through B1's
//!   seam (the rung admits it to the working state of the anomaly that bought it), so
//!   [`CH_PROBE`] is reserved and unused.
//! - A **message id** is a `u64` and a [`Tag`] a `u32`. The fold, and its collision rule: a
//!   catalogue id (below `CATALOGUE_LIMIT`, 2^16) becomes `0x4000_0000 | id`, exact, so
//!   catalogue ids never collide with each other or with anything else; a free-form id becomes
//!   `0x8000_0000 | ((lo ^ hi) & 0x7FFF_FFFF)`, the exclusive-or of its two halves folded to 31
//!   bits, so two free-form ids collide exactly when their folds are equal, and the medium then
//!   treats them as one symbol. No cell of M2's graph reads a message tag, so in M2 a collision
//!   changes nothing; the rule is stated for the graphs that will.
//! - **Time**: the event's tick is `at / tick_ns` and its offset `at mod tick_ns` (a tick of at
//!   most `u32::MAX` ns, about 4.29 s, keeps the offset exact; the sweep's longest is 2 s). Its
//!   `seq` is the observation's id, which is unique and in delivery order, so an
//!   [`gordian_medium::EventRef`] names the observation it came from and orders as it does.
//!
//! # Clock
//!
//! [`TickClock`]: the tick of a logical instant is `at / tick_ns`. A tick is run only when it is
//! complete, that is when the harness's instant has reached its end (`(t + 1) * tick_ns <= now`),
//! so that every observation of the tick has been delivered when it runs; the medium refuses a
//! tick that is not the one after the last, so every tick is run, empty ones included.
//!
//! # Ledger
//!
//! [`TickLedger`]: the medium's operation counts per tick, summed, priced at the spec's declared
//! prices (200 / 25 / 40 / 2 ns, `Prices::DECLARED`) plus [`TICK_PRICE_NS`] per tick (the chief's
//! decision on M1b: a medium that ticks is billed for ticking). The noticer hands the sum since
//! the last charge to the arm, which charges it to the bill like a component call.

use crate::stream::arms::rung::Held;
use gordian_core::Instant;
use gordian_medium::{
    Address, Clock, Event, Ledger as MediumLedger, OpCounts, Prices, Sense, Tag, Truncation,
};
use gordian_world::Observation;
use gordian_world::physics::CATALOGUE_LIMIT;
use gordian_world::{CounterName, Severity};
use std::collections::VecDeque;

/// The one domain of the stream world's addresses.
pub const DOMAIN: u16 = 0;
/// Channel of counter readings.
pub const CH_COUNTER: u16 = 0;
/// Channel of log messages.
pub const CH_MESSAGE: u16 = 1;
/// Channel of configuration snapshots.
pub const CH_SNAPSHOT: u16 = 2;
/// Channel of probe results: reserved, never delivered through the noticing seam.
pub const CH_PROBE: u16 = 3;

/// The public rules call the observation abnormal.
pub const TAG_ABNORMAL: Tag = Tag(1);
/// The public rules call the observation benign.
pub const TAG_BENIGN: Tag = Tag(2);

/// Price of one tick, nanoseconds (the chief's decision on M1b's benchmark miss).
pub const TICK_PRICE_NS: u64 = 200;

/// The tag naming a counter.
pub fn counter_tag(name: CounterName) -> Tag {
    let index = CounterName::ALL
        .iter()
        .position(|n| *n == name)
        .unwrap_or(0);
    Tag(0x10 + index as u32)
}

/// The tag naming a severity.
pub fn severity_tag(severity: Severity) -> Tag {
    Tag(0x20
        + match severity {
            Severity::Low => 0,
            Severity::Medium => 1,
            Severity::High => 2,
            Severity::Critical => 3,
        })
}

/// The kind of an abnormal observation, as a tag: the five counters by name, a message, a
/// snapshot (`ABNORMAL_KIND + 0` to `+ 6`). Carried by abnormal observations only, so that one sense
/// cell's pattern (which holds one tag) can select "an abnormal reading of this kind".
pub fn abnormal_kind_tag(obs: &Observation) -> Tag {
    let kind = match obs {
        Observation::Counter { name, .. } => {
            CounterName::ALL.iter().position(|n| n == name).unwrap_or(0) as u32
        }
        Observation::Message { .. } => 5,
        _ => 6,
    };
    Tag(ABNORMAL_KIND + kind)
}

/// The first abnormal-kind tag (see [`abnormal_kind_tag`]).
pub const ABNORMAL_KIND: u32 = 0x100;

/// A message id folded into a tag. See the module documentation for the collision rule.
pub fn message_tag(text_id: u64) -> Tag {
    if text_id < CATALOGUE_LIMIT {
        Tag(0x4000_0000 | text_id as u32)
    } else {
        let folded = ((text_id ^ (text_id >> 32)) as u32) & 0x7FFF_FFFF;
        Tag(0x8000_0000 | folded)
    }
}

/// The event for one delivered observation on a tick of `tick_ns`, or `None` for an observation
/// that is not about a service (probe results and corrections, which the seam does not carry).
pub fn encode(held: &Held, tick_ns: u64) -> Option<Event> {
    let tick_ns = tick_ns.max(1);
    let verdict = if held.abnormal {
        TAG_ABNORMAL
    } else {
        TAG_BENIGN
    };
    let (service, channel, value, mut tags) = match &held.obs {
        Observation::Counter {
            service,
            name,
            value,
        } => (
            *service,
            CH_COUNTER,
            *value as f32,
            vec![verdict, counter_tag(*name)],
        ),
        Observation::Message {
            service,
            text_id,
            severity,
        } => (
            *service,
            CH_MESSAGE,
            1.0,
            vec![verdict, message_tag(*text_id), severity_tag(*severity)],
        ),
        Observation::Snapshot { service, .. } => (*service, CH_SNAPSHOT, 1.0, vec![verdict]),
        Observation::Probed { .. } | Observation::Correction { .. } => return None,
    };
    if held.abnormal {
        tags.push(abnormal_kind_tag(&held.obs));
    }
    Some(Event {
        tick: held.at.0 / tick_ns,
        offset_ns: u32::try_from(held.at.0 % tick_ns).unwrap_or(u32::MAX),
        source: Address {
            domain: DOMAIN,
            node: u16::try_from(service.0).unwrap_or(u16::MAX),
            channel,
        },
        tags,
        value,
        seq: held.id.0,
    })
}

/// The clock port: names the tick being run, knows the tick length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickClock {
    /// The tick being run.
    pub tick: u64,
    /// The tick length, nanoseconds.
    pub tick_ns: u64,
}

impl TickClock {
    /// The ticks that are complete at the harness's instant `now`: every tick `t` with
    /// `(t + 1) * tick_ns <= now`, that is `t < complete_before(now)`.
    pub fn complete_before(tick_ns: u64, now: Instant) -> u64 {
        now.0 / tick_ns.max(1)
    }
}

impl Clock for TickClock {
    fn now(&self) -> u64 {
        self.tick
    }

    fn tick_len_ns(&self) -> u64 {
        self.tick_ns
    }
}

/// The sense port: events encoded as they were delivered, handed out tick by tick.
#[derive(Debug, Clone, Default)]
pub struct DeliveredSense {
    pending: VecDeque<Event>,
}

impl DeliveredSense {
    /// Queue an event. Events are queued in delivery order, which is time order.
    pub fn push(&mut self, event: Event) {
        self.pending.push_back(event);
    }

    /// Events queued and not yet handed out.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether none is queued.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Sense for DeliveredSense {
    /// Every queued event of `tick`. Events of an earlier tick (none, when ticks run in order
    /// and events are queued in time order) are handed out too, so that the medium refuses the
    /// tick rather than dropping them silently.
    fn events(&mut self, tick: u64) -> Vec<Event> {
        let mut out = Vec::new();
        while self.pending.front().is_some_and(|e| e.tick <= tick) {
            if let Some(e) = self.pending.pop_front() {
                out.push(e);
            }
        }
        out
    }
}

/// The ledger port: counts per tick, summed since the last charge and over the segment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TickLedger {
    /// Ticks run since the last charge.
    pub ticks: u64,
    /// Counts since the last charge.
    pub counts: OpCounts,
    /// Ticks run over the segment.
    pub total_ticks: u64,
    /// Counts over the segment.
    pub total_counts: OpCounts,
    /// Ticks that hit a limit, over the segment.
    pub truncated_ticks: u64,
}

impl MediumLedger for TickLedger {
    fn record(&mut self, _tick: u64, counts: &OpCounts, truncation: Option<&Truncation>) {
        self.ticks += 1;
        self.total_ticks += 1;
        self.counts.accumulate(counts);
        self.total_counts.accumulate(counts);
        if truncation.is_some() {
            self.truncated_ticks += 1;
        }
    }
}

impl TickLedger {
    /// The modelled cost of the work since the last charge, nanoseconds, and forget it: the
    /// counts at `prices` plus [`TICK_PRICE_NS`] per tick. `None` when no tick ran.
    pub fn take_ns(&mut self, prices: &Prices) -> Option<u64> {
        if self.ticks == 0 {
            return None;
        }
        let ns = self.counts.modelled_ps(prices) / 1_000 + TICK_PRICE_NS * self.ticks;
        self.ticks = 0;
        self.counts = OpCounts::default();
        Some(ns)
    }

    /// The modelled cost of everything over the segment, nanoseconds.
    pub fn total_ns(&self, prices: &Prices) -> u64 {
        self.total_counts.modelled_ps(prices) / 1_000 + TICK_PRICE_NS * self.total_ticks
    }
}
