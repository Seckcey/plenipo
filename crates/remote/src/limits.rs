//! Wrong tries slow the PC, then stop it for a while (ADR-141 §5, ADR-142 §6, ADR-143 §8).
//!
//! - **Meetings that fail** (an unknown phone, a bad message): after 10 in a minute, the PC stops
//!   answering new meetings for 1 minute, then 2, 4, and so on up to 15 minutes. A phone already
//!   signed in keeps working.
//! - **Dead pairing codes:** after 3 in an hour, **Add a phone** pauses for 15 minutes.
//! - **Refused passkey answers:** 3 within 10 minutes pause that phone until the owner un-pauses
//!   it on the PC.

use std::collections::VecDeque;

/// Failed meetings in this window count together.
pub const MEETING_WINDOW_MS: u64 = 60_000;
/// This many failed meetings in the window stop the PC answering.
pub const MEETING_FAILURES: usize = 10;
/// The first stop.
pub const FIRST_STOP_MS: u64 = 60_000;
/// The longest stop.
pub const LONGEST_STOP_MS: u64 = 15 * 60_000;
/// After this long with no stop, stops start again from the first.
pub const CALM_MS: u64 = 60 * 60_000;

/// Dead pairing codes in this window count together.
pub const DEAD_CODE_WINDOW_MS: u64 = 60 * 60_000;
/// This many dead codes pause **Add a phone**.
pub const DEAD_CODES: usize = 3;
/// For this long.
pub const PAIRING_PAUSE_MS: u64 = 15 * 60_000;
/// Wrong tries one code takes before it dies.
pub const CODE_TRIES: u32 = 3;

/// Refused passkey answers in this window count together.
pub const CHECK_WINDOW_MS: u64 = 10 * 60_000;
/// This many pause the phone.
pub const CHECK_FAILURES: usize = 3;

fn forget_before(times: &mut VecDeque<u64>, since: u64) {
    while times.front().is_some_and(|&t| t < since) {
        times.pop_front();
    }
}

/// Failed meetings, and the PC's stops.
#[derive(Debug, Default)]
pub struct Meetings {
    failures: VecDeque<u64>,
    stopped_until: u64,
    /// How many stops in a row (each twice as long).
    stops: u32,
    last_stop: u64,
}

impl Meetings {
    /// Is the PC answering new meetings now?
    pub fn answering(&self, now: u64) -> bool {
        now >= self.stopped_until
    }

    /// When the PC answers again.
    pub fn stopped_until(&self) -> u64 {
        self.stopped_until
    }

    /// A meeting failed. True when this one stopped the PC answering.
    pub fn failed(&mut self, now: u64) -> bool {
        forget_before(&mut self.failures, now.saturating_sub(MEETING_WINDOW_MS));
        self.failures.push_back(now);
        if self.failures.len() < MEETING_FAILURES || !self.answering(now) {
            return false;
        }
        if now.saturating_sub(self.last_stop) > CALM_MS {
            self.stops = 0;
        }
        let stop = FIRST_STOP_MS
            .saturating_mul(1u64 << self.stops.min(10))
            .min(LONGEST_STOP_MS);
        self.stops = self.stops.saturating_add(1);
        self.stopped_until = now + stop;
        self.last_stop = now;
        self.failures.clear();
        true
    }
}

/// Dead pairing codes, and **Add a phone**'s pause.
#[derive(Debug, Default)]
pub struct DeadCodes {
    deaths: VecDeque<u64>,
    paused_until: u64,
}

impl DeadCodes {
    pub fn paused_until(&self, now: u64) -> Option<u64> {
        (now < self.paused_until).then_some(self.paused_until)
    }

    /// A code died of wrong tries. True when this paused **Add a phone**.
    pub fn died(&mut self, now: u64) -> bool {
        forget_before(&mut self.deaths, now.saturating_sub(DEAD_CODE_WINDOW_MS));
        self.deaths.push_back(now);
        if self.deaths.len() >= DEAD_CODES {
            self.paused_until = now + PAIRING_PAUSE_MS;
            self.deaths.clear();
            return true;
        }
        false
    }
}

/// One phone's refused passkey answers.
#[derive(Debug, Default, Clone)]
pub struct Checks {
    failures: VecDeque<u64>,
}

impl Checks {
    /// An answer was refused. True when the phone should be paused.
    pub fn failed(&mut self, now: u64) -> bool {
        forget_before(&mut self.failures, now.saturating_sub(CHECK_WINDOW_MS));
        self.failures.push_back(now);
        self.failures.len() >= CHECK_FAILURES
    }

    pub fn clear(&mut self) {
        self.failures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_meetings_slow_then_stop_the_pc() {
        let mut m = Meetings::default();
        let t = 1_000_000;
        for i in 0..9 {
            assert!(!m.failed(t + i));
            assert!(m.answering(t + i));
        }
        assert!(m.failed(t + 9), "the tenth in a minute stops it");
        assert!(!m.answering(t + 10));
        assert!(m.answering(t + 9 + FIRST_STOP_MS));
        // Again at once: twice as long.
        let t2 = t + 9 + FIRST_STOP_MS;
        for i in 0..10 {
            m.failed(t2 + i);
        }
        assert_eq!(m.stopped_until(), t2 + 9 + 2 * FIRST_STOP_MS);
        // And never longer than 15 minutes.
        let mut now = m.stopped_until();
        let mut last = 0;
        for _ in 0..10 {
            for i in 0..10 {
                m.failed(now + i);
            }
            last = m.stopped_until() - (now + 9);
            assert!(last <= LONGEST_STOP_MS);
            now = m.stopped_until();
        }
        assert_eq!(last, LONGEST_STOP_MS);
        // After an hour of calm, back to the first stop.
        let later = now + CALM_MS + 1;
        for i in 0..10 {
            m.failed(later + i);
        }
        assert_eq!(m.stopped_until(), later + 9 + FIRST_STOP_MS);
    }

    #[test]
    fn spread_out_failures_never_stop_the_pc() {
        let mut m = Meetings::default();
        for i in 0..100 {
            assert!(!m.failed(i * 7_000), "one every 7 seconds");
        }
    }

    #[test]
    fn three_dead_codes_in_an_hour_pause_adding_a_phone() {
        let mut d = DeadCodes::default();
        assert!(!d.died(0));
        assert!(!d.died(10));
        assert!(d.died(20));
        assert_eq!(d.paused_until(21), Some(20 + PAIRING_PAUSE_MS));
        assert_eq!(d.paused_until(20 + PAIRING_PAUSE_MS), None);
        // Spread over more than an hour: no pause.
        let mut d = DeadCodes::default();
        assert!(!d.died(0));
        assert!(!d.died(DEAD_CODE_WINDOW_MS));
        assert!(!d.died(2 * DEAD_CODE_WINDOW_MS + 1));
    }

    #[test]
    fn three_refused_checks_pause_a_phone() {
        let mut c = Checks::default();
        assert!(!c.failed(0));
        assert!(!c.failed(1));
        assert!(c.failed(2));
        let mut c = Checks::default();
        assert!(!c.failed(0));
        assert!(!c.failed(CHECK_WINDOW_MS));
        assert!(!c.failed(2 * CHECK_WINDOW_MS + 1));
    }
}
