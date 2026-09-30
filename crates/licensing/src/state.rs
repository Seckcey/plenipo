//! What Plenipo keeps about the license between runs, and how it decides Free or Pro
//! (ADR-022, ADR-116).
//!
//! **Pro drops in exactly two cases**: the newest signed answer says the subscription ended (or
//! was cancelled and its paid period is over), or 30 days pass with no successful check.
//! Anything else — no internet, the service down, a timeout, a garbage or badly signed answer,
//! an answer about an unknown key — leaves Pro on and tries again later.
//!
//! **Time.** "Now" is the latest of the PC's clock, the latest clock time Plenipo has ever seen,
//! and the newest signed answer's time. The 30 days end 30 days after the newest signed answer
//! (or, before the first one, after the key was entered). So winding the clock back never
//! extends them, and replaying an old answer never resets them.

use serde::{Deserialize, Serialize};

use crate::answer::{self, AnswerPayload, SignedAnswer, SubscriptionState};
use crate::entitlements::Edition;
use crate::key::LicenseKey;

pub const DAY: i64 = 24 * 3600;
/// Pro keeps working this long between successful checks.
pub const GRACE: i64 = 30 * DAY;
/// A Pro copy checks in at most this often when checks succeed.
pub const EVERY: i64 = 7 * DAY;
/// After failed checks, Plenipo tries again after these waits (the last repeats).
pub const RETRY_AFTER: [i64; 5] = [3600, 3 * 3600, 6 * 3600, 12 * 3600, DAY];

/// Kept in the data folder (not a secret: the key itself is in the Vault).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    /// The key this record is about.
    pub key_id: Option<String>,
    /// When the key was entered (Plenipo's "now" then).
    pub entered_at: Option<i64>,
    /// The newest answer that was a successful check (not `unknown`), as it travelled.
    pub answer: Option<SignedAnswer>,
    /// The latest clock time Plenipo has seen.
    pub clock_high: i64,
    /// When Plenipo last tried to check (the PC's clock).
    pub last_attempt: Option<i64>,
    /// Failed checks in a row since the last success.
    pub failures: u32,
    /// Why the last check failed, in plain words.
    pub last_problem: Option<String>,
}

/// Why the edition is what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NoKey,
    /// Entered, not checked yet: Pro until `grace_ends`.
    NotCheckedYet,
    Active,
    /// Cancelled: Pro until `ends_at`.
    Cancelling,
    /// The subscription ended.
    Ended,
    /// No successful check for 30 days.
    NoCheck,
}

/// The license now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub edition: Edition,
    pub reason: Reason,
    /// When Pro drops if no check succeeds before then.
    pub grace_ends: Option<i64>,
    pub paid_through: Option<i64>,
    pub ends_at: Option<i64>,
    /// When the last successful check was (the service's time).
    pub last_success: Option<i64>,
}

impl Record {
    /// A fresh record for a key entered at `now`.
    pub fn entered(key: &LicenseKey, clock: i64, previous: &Record) -> Self {
        let now = clock.max(previous.clock_high);
        Self {
            key_id: Some(key.key_id().to_owned()),
            entered_at: Some(now),
            answer: None,
            clock_high: now,
            last_attempt: None,
            failures: 0,
            last_problem: None,
        }
    }

    /// The newest answer, checked again (a record edited by hand keeps nothing it cannot prove).
    pub fn verified_answer(&self) -> Option<AnswerPayload> {
        let payload = answer::verify(self.answer.as_ref()?).ok()?;
        (Some(payload.key_id.as_str()) == self.key_id.as_deref()).then_some(payload)
    }

    /// Plenipo's "now": never earlier than a time it has already seen.
    pub fn now(&self, clock: i64) -> i64 {
        let answered = self.verified_answer().map_or(0, |a| a.as_of);
        clock.max(self.clock_high).max(answered)
    }

    /// Remember the clock (call on start and on every check).
    pub fn saw_clock(&mut self, clock: i64) {
        self.clock_high = self.clock_high.max(clock);
    }

    /// A check succeeded with `signed` (already checked for this key).
    pub fn succeeded(&mut self, signed: SignedAnswer, payload: &AnswerPayload, clock: i64) {
        self.saw_clock(clock);
        self.last_attempt = Some(clock);
        if payload.state == SubscriptionState::Unknown {
            // Not a success: the 30 days go on counting from the last real answer.
            self.failed("8 West doesn't know this key yet", clock);
            return;
        }
        // Never go back to an older answer (a replayed one).
        let newer = self
            .verified_answer()
            .is_none_or(|kept| payload.as_of >= kept.as_of);
        if newer {
            self.answer = Some(signed);
        }
        self.failures = 0;
        self.last_problem = None;
    }

    /// A check failed, for `why` (plain words).
    pub fn failed(&mut self, why: &str, clock: i64) {
        self.saw_clock(clock);
        self.last_attempt = Some(clock);
        self.failures = self.failures.saturating_add(1);
        self.last_problem = Some(why.to_owned());
    }

    /// When to check next, by the PC's clock; `None` with no key (a Free copy never checks).
    pub fn next_check(&self) -> Option<i64> {
        self.key_id.as_ref()?;
        let Some(last) = self.last_attempt else {
            // Never tried: now.
            return Some(i64::MIN);
        };
        if self.failures == 0 {
            return Some(last.saturating_add(EVERY));
        }
        let i = (self.failures as usize - 1).min(RETRY_AFTER.len() - 1);
        Some(last.saturating_add(RETRY_AFTER[i]))
    }

    /// Free or Pro at `clock`, for `key` (the one in the Vault, already checked).
    pub fn status(&self, key: Option<&LicenseKey>, clock: i64) -> Status {
        let free = |reason| Status {
            edition: Edition::Free,
            reason,
            grace_ends: None,
            paid_through: None,
            ends_at: None,
            last_success: None,
        };
        let Some(key) = key else {
            return free(Reason::NoKey);
        };
        if self.key_id.as_deref() != Some(key.key_id()) {
            // A record about another key: the key was entered, and nothing is known yet.
            return free(Reason::NoKey);
        }
        let now = self.now(clock);
        let answer = self.verified_answer();
        let anchor = answer
            .as_ref()
            .map(|a| a.as_of)
            .or(self.entered_at)
            .unwrap_or(now);
        let grace_ends = anchor.saturating_add(GRACE);
        let mut status = Status {
            edition: Edition::Pro,
            reason: Reason::NotCheckedYet,
            grace_ends: Some(grace_ends),
            paid_through: answer
                .as_ref()
                .and_then(|a| a.paid_through)
                .or(Some(key.payload().paid_through)),
            ends_at: answer.as_ref().and_then(|a| a.ends_at),
            last_success: answer.as_ref().map(|a| a.as_of),
        };
        if let Some(a) = &answer {
            match a.state {
                SubscriptionState::Active | SubscriptionState::Unknown => {
                    status.reason = Reason::Active;
                }
                SubscriptionState::Cancelled => {
                    status.reason = Reason::Cancelling;
                    if a.ends_at.is_some_and(|end| now >= end) {
                        status.edition = Edition::Free;
                        status.reason = Reason::Ended;
                    }
                }
                SubscriptionState::Ended => {
                    status.edition = Edition::Free;
                    status.reason = Reason::Ended;
                }
            }
        }
        if status.edition == Edition::Pro && now >= grace_ends {
            status.edition = Edition::Free;
            status.reason = Reason::NoCheck;
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::tests::{answer, AS_OF};
    use crate::key::tests::valid_key;
    use crate::key::{self};
    use crate::trust;

    fn key() -> LicenseKey {
        key::parse(&valid_key()).unwrap()
    }

    fn entered(at: i64) -> Record {
        Record::entered(&key(), at, &Record::default())
    }

    fn answered(state: SubscriptionState) -> Record {
        let mut r = entered(AS_OF - 100);
        let p = answer(state);
        r.succeeded(answer::sign(&p, &trust::test_signing_key()), &p, AS_OF);
        r
    }

    #[test]
    fn no_key_is_free_and_never_checks() {
        let r = Record::default();
        assert_eq!(r.status(None, AS_OF).edition, Edition::Free);
        assert_eq!(r.status(None, AS_OF).reason, Reason::NoKey);
        assert_eq!(r.next_check(), None);
    }

    #[test]
    fn a_key_entered_turns_pro_on_at_once_and_checks_now() {
        let r = entered(AS_OF);
        let s = r.status(Some(&key()), AS_OF);
        assert_eq!(s.edition, Edition::Pro);
        assert_eq!(s.reason, Reason::NotCheckedYet);
        assert_eq!(s.grace_ends, Some(AS_OF + GRACE));
        assert_eq!(r.next_check(), Some(i64::MIN));
    }

    #[test]
    fn no_internet_keeps_pro_through_day_30_and_drops_on_day_31() {
        let r = answered(SubscriptionState::Active);
        let k = key();
        assert_eq!(r.status(Some(&k), AS_OF + 29 * DAY).edition, Edition::Pro);
        assert_eq!(
            r.status(Some(&k), AS_OF + 30 * DAY - 1).edition,
            Edition::Pro
        );
        let day31 = r.status(Some(&k), AS_OF + 30 * DAY);
        assert_eq!(day31.edition, Edition::Free);
        assert_eq!(day31.reason, Reason::NoCheck);
        // The same before any check: 30 days from entering the key.
        let fresh = entered(AS_OF);
        assert_eq!(
            fresh.status(Some(&k), AS_OF + 30 * DAY - 1).edition,
            Edition::Pro
        );
        assert_eq!(
            fresh.status(Some(&k), AS_OF + 30 * DAY).edition,
            Edition::Free
        );
    }

    #[test]
    fn failed_checks_keep_pro_on_and_retry_later() {
        let mut r = answered(SubscriptionState::Active);
        let k = key();
        for (i, wait) in RETRY_AFTER.iter().chain([&DAY, &DAY]).enumerate() {
            let at = AS_OF + (i as i64 + 1) * 3600;
            r.failed("8 West's service didn't answer", at);
            assert_eq!(r.status(Some(&k), at).edition, Edition::Pro);
            assert_eq!(r.next_check(), Some(at + wait));
        }
        assert_eq!(
            r.last_problem.as_deref(),
            Some("8 West's service didn't answer")
        );
        // A success afterwards goes back to weekly.
        let p = answer(SubscriptionState::Active);
        let mut later = p.clone();
        later.as_of = AS_OF + 10 * DAY;
        r.succeeded(
            answer::sign(&later, &trust::test_signing_key()),
            &later,
            AS_OF + 10 * DAY,
        );
        assert_eq!(r.failures, 0);
        assert_eq!(r.next_check(), Some(AS_OF + 10 * DAY + EVERY));
        assert_eq!(r.last_problem, None);
    }

    #[test]
    fn cancelled_keeps_pro_until_the_end_of_the_paid_period_then_drops() {
        let r = answered(SubscriptionState::Cancelled);
        let k = key();
        let end = answer(SubscriptionState::Cancelled).ends_at.unwrap();
        let before = r.status(Some(&k), end - 1);
        assert_eq!(before.edition, Edition::Pro);
        assert_eq!(before.reason, Reason::Cancelling);
        assert_eq!(before.ends_at, Some(end));
        let after = r.status(Some(&k), end);
        assert_eq!(after.edition, Edition::Free);
        assert_eq!(after.reason, Reason::Ended);
    }

    #[test]
    fn ended_drops_pro_at_once() {
        let r = answered(SubscriptionState::Ended);
        let s = r.status(Some(&key()), AS_OF);
        assert_eq!(s.edition, Edition::Free);
        assert_eq!(s.reason, Reason::Ended);
    }

    #[test]
    fn an_unknown_key_is_not_a_success_and_keeps_pro_on() {
        let mut r = answered(SubscriptionState::Active);
        let k = key();
        let mut unknown = answer(SubscriptionState::Unknown);
        unknown.as_of = AS_OF + 5 * DAY;
        r.succeeded(
            answer::sign(&unknown, &trust::test_signing_key()),
            &unknown,
            AS_OF + 5 * DAY,
        );
        let s = r.status(Some(&k), AS_OF + 5 * DAY);
        assert_eq!(s.edition, Edition::Pro);
        // The 30 days still count from the last real answer.
        assert_eq!(s.grace_ends, Some(AS_OF + GRACE));
        assert_eq!(r.failures, 1);
    }

    #[test]
    fn winding_the_clock_back_does_not_extend_the_grace() {
        let mut r = answered(SubscriptionState::Active);
        let k = key();
        // Plenipo ran on day 31 once: Free.
        r.saw_clock(AS_OF + 31 * DAY);
        // The clock is wound back to day 2: still Free.
        let s = r.status(Some(&k), AS_OF + 2 * DAY);
        assert_eq!(s.edition, Edition::Free);
        assert_eq!(s.reason, Reason::NoCheck);
        // Even with a clock set before the last answer, "now" is never earlier than it.
        let fresh = answered(SubscriptionState::Active);
        assert_eq!(fresh.now(AS_OF - 100 * DAY), AS_OF);
    }

    #[test]
    fn replaying_an_old_answer_does_not_reset_the_grace() {
        let mut r = answered(SubscriptionState::Active);
        let k = key();
        let mut newer = answer(SubscriptionState::Active);
        newer.as_of = AS_OF + 20 * DAY;
        r.succeeded(
            answer::sign(&newer, &trust::test_signing_key()),
            &newer,
            AS_OF + 20 * DAY,
        );
        // The old answer again.
        let old = answer(SubscriptionState::Active);
        r.succeeded(
            answer::sign(&old, &trust::test_signing_key()),
            &old,
            AS_OF + 21 * DAY,
        );
        assert_eq!(r.verified_answer().unwrap().as_of, AS_OF + 20 * DAY);
        assert_eq!(r.status(Some(&k), AS_OF + 49 * DAY).edition, Edition::Pro);
        assert_eq!(r.status(Some(&k), AS_OF + 50 * DAY).edition, Edition::Free);
    }

    #[test]
    fn a_record_edited_by_hand_keeps_only_what_it_can_prove() {
        let mut r = answered(SubscriptionState::Active);
        let k = key();
        // The kept answer's text changed: its signature no longer matches, so it is ignored.
        let mut ended = answer(SubscriptionState::Active);
        ended.as_of = AS_OF + 1000 * DAY;
        r.answer.as_mut().unwrap().answer =
            crate::codec::encode(&serde_json::to_vec(&ended).unwrap());
        assert!(r.verified_answer().is_none());
        // Only the key's entry date counts now.
        assert_eq!(
            r.status(Some(&k), AS_OF).grace_ends,
            Some(AS_OF - 100 + GRACE)
        );
    }

    #[test]
    fn a_record_about_another_key_gives_nothing() {
        let mut r = answered(SubscriptionState::Active);
        r.key_id = Some("lk_0000000000000000000000000X".into());
        assert_eq!(r.status(Some(&key()), AS_OF).edition, Edition::Free);
        assert!(r.verified_answer().is_none());
    }

    #[test]
    fn a_record_survives_being_written_and_read() {
        let r = answered(SubscriptionState::Cancelled);
        let text = serde_json::to_string(&r).unwrap();
        assert!(
            !text.contains(crate::key::KEY_PREFIX),
            "never the key itself"
        );
        let back: Record = serde_json::from_str(&text).unwrap();
        assert_eq!(back, r);
        // An empty or old file reads as nothing known.
        assert_eq!(
            serde_json::from_str::<Record>("{}").unwrap(),
            Record::default()
        );
    }
}
