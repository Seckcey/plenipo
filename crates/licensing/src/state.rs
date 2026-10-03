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
//!
//! **The paid period is a ceiling** (P-DESK-1, ADR-211). With no signed answer at all, Pro also
//! ends 30 days after the key's own paid-through date, so a record that is lost or deleted, which
//! starts the 30 days again, never carries Pro past the paid period. A signed answer from 8 West
//! moves it (a renewal), and an "unknown" answer lifts it, as ADR-022 §3 says. A copy of the
//! record's entry time and newest answer is kept in the Vault next to the key ([`RecordCopy`]),
//! so losing the record loses nothing a paying owner needs.

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
/// How far the PC's clock may be from 8 West's before Plenipo says so.
pub const CLOCK_SLACK: i64 = DAY;

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
    /// How far the PC's clock was ahead of 8 West's at the last successful check, in seconds,
    /// when it was more than [`CLOCK_SLACK`] (Settings → License says so).
    pub clock_ahead: Option<i64>,
    /// The newest "unknown" answer, as it travelled: not a success, but signed proof that 8 West
    /// was reached and does not know the key, so the paid-period ceiling does not apply.
    pub unknown: Option<SignedAnswer>,
}

/// What the Vault keeps next to the key (P-DESK-1, ADR-211): the two things in the record that
/// only time or 8 West can give back — when the key was entered, and 8 West's newest signed
/// answer. Not a secret (no key text). A record deleted or lost by itself then starts nothing
/// again. It raises the bar for someone who deletes files; it is not a wall against the PC's
/// owner.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecordCopy {
    pub key_id: Option<String>,
    pub entered_at: Option<i64>,
    pub answer: Option<SignedAnswer>,
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
            clock_ahead: None,
            unknown: None,
        }
    }

    /// The newest answer, checked again (a record edited by hand keeps nothing it cannot prove).
    pub fn verified_answer(&self) -> Option<AnswerPayload> {
        let payload = answer::verify(self.answer.as_ref()?).ok()?;
        (Some(payload.key_id.as_str()) == self.key_id.as_deref()).then_some(payload)
    }

    /// The newest "unknown" answer, checked again.
    fn verified_unknown(&self) -> Option<AnswerPayload> {
        let payload = answer::verify(self.unknown.as_ref()?).ok()?;
        (payload.state == SubscriptionState::Unknown
            && Some(payload.key_id.as_str()) == self.key_id.as_deref())
        .then_some(payload)
    }

    /// What the Vault keeps of this record ([`RecordCopy`]): only an answer that checks.
    pub fn copy(&self) -> RecordCopy {
        RecordCopy {
            key_id: self.key_id.clone(),
            entered_at: self.entered_at,
            answer: self.verified_answer().and(self.answer.clone()),
        }
    }

    /// Take back from the Vault's copy, for the key `key_id`, what this record lost or had moved:
    /// the earlier entry time and the newer signed answer. A copy about another key changes
    /// nothing. A record that is empty, unreadable, or about another key starts again from the
    /// copy (and, with nothing to take, as a key entered now).
    pub fn restore(&mut self, copy: &RecordCopy, key_id: &str, clock: i64) {
        if copy.key_id.as_deref() != Some(key_id) {
            return;
        }
        if self.key_id.as_deref() != Some(key_id) {
            *self = Self {
                key_id: Some(key_id.to_owned()),
                clock_high: clock.max(self.clock_high),
                ..Self::default()
            };
        }
        // The newer signed answer about this key ("unknown" is never kept there).
        let theirs = copy.answer.as_ref().and_then(|signed| {
            answer::verify(signed)
                .ok()
                .filter(|p| p.key_id == key_id && p.state != SubscriptionState::Unknown)
                .map(|p| (signed, p.as_of))
        });
        if let Some((signed, as_of)) = theirs {
            if self.verified_answer().is_none_or(|mine| as_of > mine.as_of) {
                self.answer = Some(signed.clone());
            }
        }
        // The earlier entry time, believed only when it is not in the future: a copy changed by
        // hand cannot move the 30 days on.
        let now = self.now(clock);
        if let Some(at) = copy.entered_at.filter(|&at| at <= now) {
            self.entered_at = Some(self.entered_at.map_or(at, |mine| mine.min(at)));
        }
        // Nothing to take: as a key entered now.
        self.entered_at.get_or_insert(now);
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
            // Not a success: the 30 days go on counting from the last real answer. Kept, signed,
            // as proof that 8 West was reached (the paid-period ceiling then does not apply).
            if self
                .verified_unknown()
                .is_none_or(|kept| payload.as_of >= kept.as_of)
            {
                self.unknown = Some(signed);
            }
            self.failed("8 West doesn't know this key yet", clock);
            return;
        }
        let kept = self.verified_answer().map(|a| a.as_of);
        // Never go back to an older answer (a replayed one).
        if kept.is_none_or(|k| payload.as_of >= k) {
            self.answer = Some(signed);
        }
        // A strictly newer signed answer carries 8 West's own time: a PC clock that was once set
        // ahead no longer holds Plenipo's "now" there. A replayed answer is never newer, so this
        // never lets a clock wound back gain a day.
        if kept.is_none_or(|k| payload.as_of > k) {
            self.clock_high = clock
                .max(payload.as_of)
                .min(payload.as_of.saturating_add(CLOCK_SLACK));
        }
        let ahead = clock.saturating_sub(payload.as_of);
        self.clock_ahead = (ahead > CLOCK_SLACK).then_some(ahead);
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
        let mut grace_ends = anchor.saturating_add(GRACE);
        // The paid period is a ceiling (P-DESK-1, ADR-211): Pro lasts at most 30 days past the
        // newest paid-through date Plenipo can prove, the key's own or a later one in 8 West's
        // signed answer (a renewal; an answer also proves its own time). So with any signed
        // answer, the 30 days without a check end first, as before. With none, a record lost
        // or deleted, which starts the 30 days again, never carries Pro past the paid period. An
        // "unknown" answer since then lifts it: what 8 West does not know never takes Pro
        // (ADR-022 §3).
        let heard_unknown = self.verified_unknown().is_some_and(|u| u.as_of >= anchor);
        if !heard_unknown {
            let paid = [
                Some(key.payload().paid_through),
                answer.as_ref().map(|a| a.as_of),
                answer.as_ref().and_then(|a| a.paid_through),
                answer.as_ref().and_then(|a| a.ends_at),
            ]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(key.payload().paid_through);
            grace_ends = grace_ends.min(paid.saturating_add(GRACE));
        }
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
    use crate::key::tests::{valid_key, KEY_ID};
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

    /// A monthly key paid through `paid_through`. A renewal never mails a new key, so its own
    /// date goes stale while 8 West's answers move on.
    fn monthly(paid_through: i64) -> LicenseKey {
        let mut p = key::tests::payload();
        p.plan = key::Plan::Monthly;
        p.issued_at = paid_through - 30 * DAY;
        p.paid_through = paid_through;
        key::parse(&key::mint(&p, &trust::test_signing_key())).unwrap()
    }

    /// 8 West's signed answer about the test key, as of `as_of`.
    fn signed_answer(
        state: SubscriptionState,
        as_of: i64,
        paid_through: Option<i64>,
    ) -> (SignedAnswer, AnswerPayload) {
        let mut p = answer(state);
        p.as_of = as_of;
        p.paid_through = paid_through;
        (answer::sign(&p, &trust::test_signing_key()), p)
    }

    /// P-DESK-1: a record started again (deleted, lost, or changed) once the key's paid period
    /// and 30 days are over is Free. 8 West's signed answer with a later paid-through date (a
    /// renewal) is Pro.
    #[test]
    fn a_record_started_again_after_the_paid_period_and_30_days_is_free() {
        let paid = AS_OF + 10 * DAY;
        let k = monthly(paid);
        let late = paid + GRACE + 1;
        let mut r = Record::entered(&k, late, &Record::default());
        let s = r.status(Some(&k), late);
        assert_eq!(s.edition, Edition::Free);
        assert_eq!(s.reason, Reason::NoCheck);
        assert_eq!(s.grace_ends, Some(paid + GRACE));
        // Renewed: 8 West's answer carries the paid-through date on.
        let (sa, p) = signed_answer(SubscriptionState::Active, late, Some(late + 20 * DAY));
        r.succeeded(sa, &p, late);
        let s = r.status(Some(&k), late);
        assert_eq!(s.edition, Edition::Pro);
        assert_eq!(s.reason, Reason::Active);
        assert_eq!(s.grace_ends, Some(late + GRACE));
    }

    /// The paid period never cuts a paying owner short. A key still in its paid period keeps
    /// the full 30 days. Once 8 West has answered, only the 30 days without a check count, even
    /// with the key's own date long past (a monthly key, renewed) or no paid-through date in the
    /// answer.
    #[test]
    fn the_paid_period_never_cuts_a_paying_owner_short() {
        // A yearly key, offline from the day it is entered: the 30 days, as before.
        let r = entered(AS_OF);
        assert_eq!(
            r.status(Some(&key()), AS_OF).grace_ends,
            Some(AS_OF + GRACE)
        );
        assert_eq!(
            r.status(Some(&key()), AS_OF + GRACE - 1).edition,
            Edition::Pro
        );
        // A monthly key twenty days before its month ends: still the full 30 days.
        let k = monthly(AS_OF + 20 * DAY);
        let r = Record::entered(&k, AS_OF, &Record::default());
        assert_eq!(r.status(Some(&k), AS_OF).grace_ends, Some(AS_OF + GRACE));
        assert_eq!(r.status(Some(&k), AS_OF + GRACE - 1).edition, Edition::Pro);
        // A monthly key ten months on, renewed every month, offline after 8 West's last answer:
        // Pro through day 29, Free on day 30, as before. With or without a paid-through date in
        // the answer (its own time proves it was paid then).
        let k = monthly(AS_OF - 300 * DAY);
        for paid_through in [Some(AS_OF + 20 * DAY), None] {
            let mut r = Record::entered(&k, AS_OF - 330 * DAY, &Record::default());
            let (sa, p) = signed_answer(SubscriptionState::Active, AS_OF, paid_through);
            r.succeeded(sa, &p, AS_OF);
            let s = r.status(Some(&k), AS_OF + 29 * DAY);
            assert_eq!(s.edition, Edition::Pro, "{paid_through:?}");
            assert_eq!(s.grace_ends, Some(AS_OF + GRACE));
            assert_eq!(r.status(Some(&k), AS_OF + GRACE).edition, Edition::Free);
        }
    }

    /// ADR-022 §3 stands: an "unknown" answer keeps Pro on, even for a key past its paid period
    /// (what 8 West does not know never takes Pro). The 30 days still count from entering it.
    #[test]
    fn an_unknown_answer_keeps_pro_on_past_the_paid_period() {
        let k = monthly(AS_OF - 100 * DAY);
        let mut r = Record::entered(&k, AS_OF, &Record::default());
        assert_eq!(r.status(Some(&k), AS_OF).edition, Edition::Free);
        let (sa, p) = signed_answer(SubscriptionState::Unknown, AS_OF + 60, None);
        r.succeeded(sa, &p, AS_OF + 60);
        let s = r.status(Some(&k), AS_OF + 60);
        assert_eq!(s.edition, Edition::Pro);
        assert_eq!(s.reason, Reason::NotCheckedYet);
        assert_eq!(s.grace_ends, Some(AS_OF + GRACE));
        assert_eq!(r.failures, 1, "still not a success");
        assert_eq!(r.status(Some(&k), AS_OF + GRACE).edition, Edition::Free);
        // It is kept, signed, through a restart.
        let back: Record = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back.status(Some(&k), AS_OF + 60).edition, Edition::Pro);
        // An "unknown" from before the key was entered proves nothing about this record.
        let mut again = Record::entered(&k, AS_OF + 2 * DAY, &Record::default());
        again.unknown.clone_from(&r.unknown);
        assert_eq!(
            again.status(Some(&k), AS_OF + 2 * DAY).edition,
            Edition::Free
        );
        // Nor does one changed by hand.
        let mut changed = r.clone();
        let mut later = answer(SubscriptionState::Unknown);
        later.as_of = AS_OF + 1000 * DAY;
        changed.unknown.as_mut().unwrap().answer =
            crate::codec::encode(&serde_json::to_vec(&later).unwrap());
        assert_eq!(changed.status(Some(&k), AS_OF + 60).edition, Edition::Free);
    }

    /// P-DESK-1: the Vault's copy brings back what a lost record had, the entry time and 8
    /// West's newest answer, so nothing starts again.
    #[test]
    fn the_vault_copy_brings_back_what_a_lost_record_had() {
        let k = key();
        let r = answered(SubscriptionState::Active);
        let copy = r.copy();
        assert!(
            !serde_json::to_string(&copy)
                .unwrap()
                .contains(crate::key::KEY_PREFIX),
            "never the key itself"
        );
        let later = AS_OF + 10 * DAY;
        // The record deleted, or unreadable: nothing known, then the copy.
        let mut lost = Record::default();
        lost.restore(&copy, KEY_ID, later);
        assert_eq!(lost.entered_at, r.entered_at);
        assert_eq!(lost.verified_answer().unwrap().as_of, AS_OF);
        assert_eq!(lost.status(Some(&k), later).grace_ends, Some(AS_OF + GRACE));
        assert_eq!(lost.status(Some(&k), AS_OF + GRACE).edition, Edition::Free);
        assert_eq!(lost.next_check(), Some(i64::MIN), "it checks at once");
        // An "ended" answer is not forgotten.
        let mut lost = Record::default();
        lost.restore(&answered(SubscriptionState::Ended).copy(), KEY_ID, later);
        assert_eq!(lost.status(Some(&k), later).reason, Reason::Ended);
        // A record started again (entered now, no answer) takes the earlier time and the answer.
        let mut fresh = Record::entered(&k, later, &Record::default());
        fresh.restore(&copy, KEY_ID, later);
        assert_eq!(fresh.entered_at, Some(AS_OF - 100));
        assert_eq!(fresh.status(Some(&k), later), r.status(Some(&k), later));
        // A copy about another key changes nothing.
        let mut other = Record::default();
        other.restore(&copy, "lk_0000000000000000000000000X", later);
        assert_eq!(other, Record::default());
    }

    /// The copy keeps only what it can prove, and only moves forward: an older answer never
    /// replaces a newer one, and a later entry time never moves an earlier one. A copy changed
    /// by hand (an answer whose signature fails, an entry time in the future) or holding an
    /// "unknown" answer gives nothing.
    #[test]
    fn the_vault_copy_keeps_only_what_it_can_prove_and_only_moves_forward() {
        let mut newer = answered(SubscriptionState::Active);
        let (sa, p) = signed_answer(
            SubscriptionState::Active,
            AS_OF + 20 * DAY,
            Some(1_822_000_000),
        );
        newer.succeeded(sa, &p, AS_OF + 20 * DAY);
        let older = answered(SubscriptionState::Active).copy();
        let at = AS_OF + 21 * DAY;
        let mut r = newer.clone();
        r.restore(&older, KEY_ID, at);
        assert_eq!(r.verified_answer().unwrap().as_of, AS_OF + 20 * DAY);
        let mut late = older.clone();
        late.entered_at = Some(AS_OF + 5 * DAY);
        r.restore(&late, KEY_ID, at);
        assert_eq!(r.entered_at, Some(AS_OF - 100));
        // Changed by hand.
        let mut forged = newer.copy();
        let mut far = answer(SubscriptionState::Active);
        far.as_of = AS_OF + 1000 * DAY;
        forged.answer.as_mut().unwrap().answer =
            crate::codec::encode(&serde_json::to_vec(&far).unwrap());
        forged.entered_at = Some(AS_OF + 1000 * DAY);
        let mut lost = Record::default();
        lost.restore(&forged, KEY_ID, at);
        assert!(lost.answer.is_none());
        assert_eq!(lost.entered_at, Some(at), "as a key entered now");
        // An answer that does not check is never copied either.
        let mut bad = newer.clone();
        bad.answer.clone_from(&forged.answer);
        assert_eq!(bad.copy().answer, None);
        // An "unknown" answer is not one to keep.
        let (unknown, _) = signed_answer(SubscriptionState::Unknown, at, None);
        let copy = RecordCopy {
            key_id: Some(KEY_ID.into()),
            entered_at: None,
            answer: Some(unknown),
        };
        let mut lost = Record::default();
        lost.restore(&copy, KEY_ID, at);
        assert!(lost.answer.is_none());
    }

    /// The paid period follows Plenipo's "now", as the 30 days do. A clock moved back changes
    /// nothing for a paying owner and gains no days. A clock moved forward by mistake drops Pro
    /// only past the paid period and 30 days, and 8 West's next answer undoes it.
    #[test]
    fn a_clock_moved_back_or_forward_and_the_paid_period() {
        let paid = AS_OF + 10 * DAY;
        let k = monthly(paid);
        let mut r = Record::entered(&k, AS_OF, &Record::default());
        let s = r.status(Some(&k), AS_OF - 7 * DAY);
        assert_eq!(s.edition, Edition::Pro);
        assert_eq!(s.grace_ends, Some(AS_OF + GRACE));
        // Forward by mistake, past the paid period and 30 days: Free...
        let ahead = paid + GRACE + DAY;
        r.saw_clock(ahead);
        assert_eq!(r.status(Some(&k), ahead).edition, Edition::Free);
        // ...and winding it back does not bring Pro back by itself...
        assert_eq!(r.status(Some(&k), AS_OF + DAY).edition, Edition::Free);
        // ...but 8 West's answer, as of its own time, does.
        let (sa, p) = signed_answer(
            SubscriptionState::Active,
            AS_OF + DAY,
            Some(paid + 30 * DAY),
        );
        r.succeeded(sa, &p, AS_OF + DAY);
        assert_eq!(r.status(Some(&k), AS_OF + DAY).edition, Edition::Pro);
    }
}
