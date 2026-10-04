//! Time as an explicit, replayable value.
//!
//! The core never asks the operating system what time it is. A driver advances a
//! [`ManualClock`]; every decision the core makes is a function of the clock value it was handed.

/// A point in logical time, in nanoseconds since the start of the run.
///
/// The unit is nanoseconds so that measured wall-clock durations can be recorded without
/// rounding when a driver chooses to replay real timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Instant(pub u64);

impl Instant {
    /// The start of the run.
    pub const ZERO: Instant = Instant(0);

    /// Nanoseconds elapsed since `earlier`, or `None` if `earlier` is later than `self`.
    pub fn since(self, earlier: Instant) -> Option<u64> {
        self.0.checked_sub(earlier.0)
    }
}

/// A clock that only moves when the driver moves it.
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    now: Instant,
}

impl ManualClock {
    /// A clock at [`Instant::ZERO`].
    pub fn new() -> Self {
        Self::default()
    }

    /// A clock starting at `start`.
    pub fn starting_at(start: Instant) -> Self {
        Self { now: start }
    }

    /// The current logical time.
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Advance by `nanos`. Saturates rather than wrapping so that a replayed trace with an
    /// absurd duration fails loudly in accounting rather than silently going back in time.
    pub fn advance(&mut self, nanos: u64) -> Instant {
        self.now = Instant(self.now.0.saturating_add(nanos));
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_only_moves_when_advanced() {
        let mut clock = ManualClock::new();
        assert_eq!(clock.now(), Instant::ZERO);
        assert_eq!(clock.now(), Instant::ZERO);
        clock.advance(5);
        assert_eq!(clock.now(), Instant(5));
        clock.advance(0);
        assert_eq!(clock.now(), Instant(5));
    }

    #[test]
    fn since_is_none_when_earlier_is_later() {
        assert_eq!(Instant(10).since(Instant(3)), Some(7));
        assert_eq!(Instant(3).since(Instant(10)), None);
    }

    #[test]
    fn advance_saturates() {
        let mut clock = ManualClock::starting_at(Instant(u64::MAX - 1));
        clock.advance(10);
        assert_eq!(clock.now(), Instant(u64::MAX));
    }
}
