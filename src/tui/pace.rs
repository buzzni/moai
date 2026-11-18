//! The pace of the automatic refresh (moai-ffk9.15v) — how long the file window's stamp sweep, its changed-files list
//! and a diff pane rest between two looks.
//!
//! **The rest is counted from the end of the last look, never from its start**: a look that takes longer than the rest
//! would otherwise start the next before it ended, and slow looks would pile up. **It is the longer of [`MIN`] and ten
//! times what the last look took** ([`SLOW`]) — a `git` that takes 2 s runs once in 20 s, so the refresh never holds
//! more than a tenth of the machine. **Looks that found nothing make it longer**, doubling up to [`QUIET_MAX`]; one that
//! found a change, or a key the person pressed there, brings it back to [`MIN`].
//!
//! The clock is a value ([`Clock`]) — the tests run time forward by hand instead of sleeping.

use std::sync::Arc;
use std::time::{Duration, Instant};

/// The shortest rest.
pub const MIN: Duration = Duration::from_secs(2);

/// The longest rest quiet looks grow to — a slow look's ten times may still be longer.
pub const QUIET_MAX: Duration = Duration::from_secs(10);

/// The rest is at least this many times the last look.
pub const SLOW: u32 = 10;

/// What time it is — `Instant::now`, or a test's hand-moved clock.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// The real clock.
pub fn real() -> Clock {
    Arc::new(Instant::now)
}

/// How long to rest after a look that took `took`, the `quiet`th in a row that found nothing.
pub fn rest(took: Duration, quiet: u32) -> Duration {
    let calm = MIN.saturating_mul(1u32 << quiet.min(8)).min(QUIET_MAX);
    calm.max(took.saturating_mul(SLOW))
}

/// One kind of look's pacing: when the last ended, how long it took, how many in a row found nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    ended: Instant,
    took: Duration,
    quiet: u32,
}

impl Pace {
    /// A pace whose first look is due a rest after `now` — what was just read needs no look at once.
    pub fn new(now: Instant) -> Pace {
        Pace { ended: now, took: Duration::ZERO, quiet: 0 }
    }

    /// Whether the next look is due.
    pub fn due(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.ended) >= rest(self.took, self.quiet)
    }

    /// A look ended at `now` after `took`; `changed` when it found something.
    pub fn done(&mut self, now: Instant, took: Duration, changed: bool) {
        self.ended = now;
        self.took = took;
        self.quiet = if changed { 0 } else { self.quiet.saturating_add(1) };
    }

    /// The person pressed a key there — the rest goes back to the shortest.
    pub fn wake(&mut self) {
        self.quiet = 0;
    }

    /// The rest now owed.
    #[cfg(test)]
    pub fn resting(&self) -> Duration {
        rest(self.took, self.quiet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The rest is the longer of 2 s and ten times the last look**, and quiet looks stretch it to 10 s only.
    #[test]
    fn the_rest_is_the_longer_of_two_seconds_and_ten_times_the_look() {
        assert_eq!(rest(Duration::ZERO, 0), MIN);
        assert_eq!(rest(Duration::from_millis(100), 0), MIN);
        assert_eq!(rest(Duration::from_secs(3), 0), Duration::from_secs(30), "a 3 s look rests less than 30 s");
        assert_eq!(
            rest(Duration::from_secs(2), 5),
            Duration::from_secs(20),
            "quiet must not shorten a slow look's rest"
        );
        let grown: Vec<_> = (0..6).map(|q| rest(Duration::ZERO, q).as_secs()).collect();
        assert_eq!(grown, [2, 4, 8, 10, 10, 10], "quiet looks do not grow to 10 s and stop there");
        assert_eq!(rest(Duration::ZERO, u32::MAX), QUIET_MAX);
    }

    /// **Counted from the end**: a look that took 3 s is followed 30 s after it ended, not after it began.
    #[test]
    fn the_rest_is_counted_from_the_end_of_the_look() {
        let t0 = Instant::now();
        let mut p = Pace::new(t0);
        assert!(!p.due(t0 + Duration::from_millis(1999)));
        assert!(p.due(t0 + MIN));
        // Began at 2 s, ended at 5 s.
        p.done(t0 + Duration::from_secs(5), Duration::from_secs(3), true);
        assert!(!p.due(t0 + Duration::from_secs(32)), "rested from the start of the look");
        assert!(!p.due(t0 + Duration::from_secs(34)));
        assert!(p.due(t0 + Duration::from_secs(35)));
    }

    /// **Quiet grows the rest, a change or a key brings it back.**
    #[test]
    fn quiet_looks_slow_down_and_a_key_or_a_change_speeds_up() {
        let t0 = Instant::now();
        let mut p = Pace::new(t0);
        for _ in 0..5 {
            p.done(t0, Duration::ZERO, false);
        }
        assert_eq!(p.resting(), QUIET_MAX);
        assert!(!p.due(t0 + Duration::from_secs(9)));
        p.wake();
        assert_eq!(p.resting(), MIN, "a key left the rest at 10 s");
        assert!(p.due(t0 + MIN));
        p.done(t0, Duration::ZERO, false);
        p.done(t0, Duration::ZERO, true);
        assert_eq!(p.resting(), MIN, "a change left the rest long");
    }
}
