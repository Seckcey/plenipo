//! The license on this PC: the key (from the Vault), the record (from the data folder), and
//! what they mean now. The app's license host keeps one, saves it after each change, and does
//! the weekly check's network part; everything that decides is here, so it can be tested
//! without a network or a clock.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::answer::{self, SubscriptionState};
use crate::entitlements::{
    Edition, FREE_DEPARTMENTS, FREE_ORGANIZATIONS, FREE_PROJECTS, FREE_WORKERS_AT_ONCE,
};
use crate::key::{self, KeyError, LicenseKey, Plan};
use crate::state::{Reason, Record, Status};

/// Why the edition is what it is (Settings → License says it in words).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LicenseReason {
    /// No key: the Free edition.
    NoKey,
    /// A key was entered and not checked with 8 West yet.
    NotCheckedYet,
    /// Paid.
    Active,
    /// Cancelled: Pro until the end of the paid period.
    Cancelling,
    /// The subscription ended.
    Ended,
    /// No successful check for 30 days.
    NoCheck,
}

impl From<Reason> for LicenseReason {
    fn from(r: Reason) -> Self {
        match r {
            Reason::NoKey => Self::NoKey,
            Reason::NotCheckedYet => Self::NotCheckedYet,
            Reason::Active => Self::Active,
            Reason::Cancelling => Self::Cancelling,
            Reason::Ended => Self::Ended,
            Reason::NoCheck => Self::NoCheck,
        }
    }
}

/// Free's limits, as the screen shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FreeLimits {
    pub organizations: u32,
    pub departments: u32,
    pub projects: u32,
    pub workers_at_once: u32,
}

pub const FREE_LIMITS: FreeLimits = FreeLimits {
    organizations: FREE_ORGANIZATIONS,
    departments: FREE_DEPARTMENTS,
    projects: FREE_PROJECTS,
    workers_at_once: FREE_WORKERS_AT_ONCE,
};

/// Settings → License. Never the key itself; times are milliseconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LicenseView {
    pub edition: Edition,
    pub reason: LicenseReason,
    /// The key's ID (what the weekly check sends).
    pub key_id: Option<String>,
    pub holder: Option<String>,
    pub plan: Option<Plan>,
    #[ts(type = "number | null")]
    pub paid_through: Option<i64>,
    /// When Pro ends (cancelled) or ended.
    #[ts(type = "number | null")]
    pub ends_at: Option<i64>,
    /// The last successful check (8 West's time).
    #[ts(type = "number | null")]
    pub last_checked: Option<i64>,
    /// The last time Plenipo tried to check.
    #[ts(type = "number | null")]
    pub last_tried: Option<i64>,
    /// When Plenipo checks next.
    #[ts(type = "number | null")]
    pub next_check: Option<i64>,
    /// When Pro drops if no check succeeds before then.
    #[ts(type = "number | null")]
    pub grace_ends: Option<i64>,
    /// Why the last check failed, in plain words.
    pub problem: Option<String>,
    pub free_limits: FreeLimits,
    /// Copies built for the tests say so on screen.
    pub test_build: bool,
    /// How many days the PC's clock was ahead of 8 West's at the last check, when more than one.
    pub clock_ahead_days: Option<u32>,
}

/// What the last check did, for the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    Answered(SubscriptionState),
    Failed(String),
}

/// The license on this PC.
#[derive(Debug, Clone, Default)]
pub struct License {
    key: Option<LicenseKey>,
    record: Record,
    /// Why the key kept in the Vault no longer checks (a signing key retired, say), in plain
    /// words. Its record is kept for when a key that checks is entered.
    kept_key_problem: Option<String>,
}

fn ms(secs: i64) -> i64 {
    secs.saturating_mul(1000)
}

impl License {
    /// What was kept: the key text from the Vault (checked again) and the record. A key that no
    /// longer checks (for example, a test key in a release build) is not used, and the screen
    /// says why. With no key, the record is kept as it is: entering the same key again picks it
    /// up, so removing and re-entering a key never restarts the 30 days.
    pub fn load(key_text: Option<&str>, record: Record, clock: i64) -> Self {
        let (key, kept_key_problem) = match key_text.map(key::parse) {
            Some(Ok(k)) => (Some(k), None),
            Some(Err(e)) => (None, Some(e.to_string())),
            None => (None, None),
        };
        let mut record = record;
        if let Some(k) = &key {
            if record.key_id.as_deref() == Some(k.key_id()) {
                // The time it was entered is only believed when it is not in the future: a record
                // changed by hand cannot move the 30 days on.
                let now = record.now(clock);
                if record.entered_at.is_none_or(|at| at > now) {
                    record.entered_at = Some(now);
                }
            } else {
                // The record is about another key: start over for this one.
                record = Record::entered(k, clock, &record);
            }
        }
        record.saw_clock(clock);
        Self {
            key,
            record,
            kept_key_problem,
        }
    }

    pub fn key(&self) -> Option<&LicenseKey> {
        self.key.as_ref()
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    /// Enter a key. It is checked at once, with no network; on success it replaces any other.
    /// The same key entered again keeps its record: typing it in again never restarts the 30
    /// days without a check.
    pub fn enter(&mut self, text: &str, clock: i64) -> Result<&LicenseKey, KeyError> {
        let key = key::parse(text)?;
        if self.record.key_id.as_deref() == Some(key.key_id()) {
            self.record.saw_clock(clock);
        } else {
            self.record = Record::entered(&key, clock, &self.record);
        }
        self.kept_key_problem = None;
        Ok(self.key.insert(key))
    }

    /// Remove the key: Free, with nothing else changed. Its record stays, so entering the same
    /// key again carries on where it was (never a new 30 days, never forgetting an "ended").
    pub fn remove(&mut self) -> Option<LicenseKey> {
        self.kept_key_problem = None;
        self.key.take()
    }

    pub fn status(&self, clock: i64) -> Status {
        self.record.status(self.key.as_ref(), clock)
    }

    pub fn edition(&self, clock: i64) -> Edition {
        self.status(clock).edition
    }

    /// Whether a check is due at `clock`, the PC's clock (never, with no key: a Free copy never
    /// checks in). A clock set back past the last try makes one due, so a clock that was once
    /// ahead cannot put the next check off.
    pub fn check_due(&self, clock: i64) -> bool {
        self.key.is_some()
            && (self.record.next_check().is_some_and(|at| clock >= at)
                || self
                    .record
                    .last_attempt
                    .is_some_and(|at| at > clock.saturating_add(3600)))
    }

    /// The key ID and the exact body of a check, when there is a key.
    pub fn check_request(&self, app_version: &str) -> Option<(String, Vec<u8>)> {
        let id = self.key.as_ref()?.key_id().to_owned();
        let body = answer::request_body(&id, app_version);
        Some((id, body))
    }

    /// Record an answer's body (status 200). A body that does not check counts as a failure.
    pub fn answered(&mut self, body: &[u8], clock: i64) -> CheckOutcome {
        let Some(key) = &self.key else {
            return CheckOutcome::Failed("there is no key".into());
        };
        match answer::accept(body, key.key_id()) {
            Ok((signed, payload)) => {
                self.record.succeeded(signed, &payload, clock);
                CheckOutcome::Answered(payload.state)
            }
            Err(e) => {
                let why = e.to_string();
                self.record.failed(&why, clock);
                CheckOutcome::Failed(why)
            }
        }
    }

    /// Record a check that failed before any answer (no internet, the service down, a timeout,
    /// an error status), in plain words.
    pub fn failed(&mut self, why: &str, clock: i64) -> CheckOutcome {
        self.record.failed(why, clock);
        CheckOutcome::Failed(why.to_owned())
    }

    /// Remember the clock (Plenipo's "now" never goes back).
    pub fn saw_clock(&mut self, clock: i64) {
        self.record.saw_clock(clock);
    }

    /// Settings → License.
    pub fn view(&self, clock: i64) -> LicenseView {
        let status = self.status(clock);
        let payload = self.key.as_ref().map(LicenseKey::payload);
        let has_key = self.key.is_some();
        let problem = if has_key {
            self.record.last_problem.clone()
        } else {
            self.kept_key_problem.as_ref().map(|why| {
                format!(
                    "The license key kept on this PC no longer works: {why} Enter the newest key 8 \
                     West emailed you."
                )
            })
        };
        LicenseView {
            edition: status.edition,
            reason: status.reason.into(),
            key_id: payload.map(|p| p.key_id.clone()),
            holder: payload.map(|p| p.holder.clone()),
            plan: payload.map(|p| p.plan),
            paid_through: status.paid_through.map(ms),
            ends_at: status.ends_at.map(ms),
            last_checked: status.last_success.map(ms),
            last_tried: self.record.last_attempt.filter(|_| has_key).map(ms),
            next_check: self
                .key
                .as_ref()
                .and(self.record.next_check())
                .map(|at| ms(at.max(clock))),
            grace_ends: status.grace_ends.map(ms),
            problem,
            free_limits: FREE_LIMITS,
            test_build: crate::trust::built_for_tests(),
            clock_ahead_days: self
                .record
                .clock_ahead
                .filter(|_| has_key)
                .map(|s| u32::try_from(s / crate::state::DAY).unwrap_or(u32::MAX)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::tests::{answer, signed, AS_OF};
    use crate::key::tests::{valid_key, KEY_ID};
    use crate::state::{DAY, EVERY};

    #[test]
    fn a_free_copy_has_no_key_and_never_asks_to_check() {
        let l = License::load(None, Record::default(), AS_OF);
        assert_eq!(l.edition(AS_OF), Edition::Free);
        assert!(!l.check_due(AS_OF));
        assert!(!l.check_due(i64::MAX));
        assert_eq!(l.check_request("1.18.0"), None);
        let v = l.view(AS_OF);
        assert_eq!(v.reason, LicenseReason::NoKey);
        assert_eq!(v.key_id, None);
        assert_eq!(v.next_check, None);
        assert_eq!(v.free_limits, FREE_LIMITS);
    }

    #[test]
    fn entering_a_key_turns_pro_on_at_once_and_a_check_is_due() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        assert_eq!(l.edition(AS_OF), Edition::Pro);
        assert!(l.check_due(AS_OF));
        let (id, body) = l.check_request("1.18.0").unwrap();
        assert_eq!(id, KEY_ID);
        assert_eq!(
            body,
            format!(r#"{{"key_id":"{KEY_ID}","app_version":"1.18.0"}}"#).into_bytes()
        );
        let v = l.view(AS_OF);
        assert_eq!(v.holder.as_deref(), Some("Contoso IT"));
        assert_eq!(v.plan, Some(Plan::Yearly));
        assert_eq!(v.reason, LicenseReason::NotCheckedYet);
    }

    #[test]
    fn entering_the_same_key_again_never_restarts_the_30_days() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        let grace = l.status(AS_OF).grace_ends;
        // Weeks with no check, then the key typed in again.
        let later = AS_OF + 25 * DAY;
        l.failed("no internet", later);
        l.enter(&valid_key(), later).unwrap();
        assert_eq!(l.status(later).grace_ends, grace);
        assert_eq!(l.view(later).reason, LicenseReason::Active);
        assert_eq!(l.edition(AS_OF + 31 * DAY), Edition::Free);
    }

    /// An answer from 8 West, signed, as of `as_of`.
    fn signed_at(state: SubscriptionState, as_of: i64) -> Vec<u8> {
        let mut a = answer(state);
        a.as_of = as_of;
        signed(&a)
    }

    /// Review finding: a clock once set far ahead held Plenipo's "now" there for good, so even a
    /// fresh "paid" answer left a paying owner on Free, and the next check was a year away.
    #[test]
    fn a_clock_set_ahead_once_is_undone_by_8_wests_next_answer() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        // The clock is wrong for a while: a year ahead, and a check fails then.
        l.saw_clock(AS_OF + 365 * DAY);
        l.failed("no internet", AS_OF + 365 * DAY);
        // The clock is put right. A check is due at once.
        let fixed = AS_OF + 2 * DAY;
        assert!(l.check_due(fixed));
        // 8 West answers "paid": Pro.
        l.answered(&signed_at(SubscriptionState::Active, fixed), fixed);
        assert_eq!(l.edition(fixed), Edition::Pro);
        assert_eq!(l.view(fixed).clock_ahead_days, None);
        // While the PC's clock is well ahead of 8 West's, the screen says so.
        let ahead = fixed + 40 * DAY;
        l.answered(&signed_at(SubscriptionState::Active, fixed + DAY), ahead);
        assert_eq!(l.view(ahead).clock_ahead_days, Some(39));
    }

    /// Lowering Plenipo's "now" takes a strictly newer signed answer: the answer already kept,
    /// sent again, never gives back days the clock took.
    #[test]
    fn a_replayed_answer_never_brings_back_what_the_clock_took() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        l.saw_clock(AS_OF + 31 * DAY);
        assert_eq!(l.edition(AS_OF + 31 * DAY), Edition::Free);
        // The clock wound back, and the old answer sent again.
        let back = AS_OF + DAY;
        l.answered(&signed(&answer(SubscriptionState::Active)), back);
        assert_eq!(l.edition(back), Edition::Free);
    }

    /// Review finding: removing a key and entering it again started a new 30 days, and forgot
    /// an "ended" answer.
    #[test]
    fn removing_and_entering_a_key_again_never_restarts_the_30_days_or_forgets_ended() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Ended)), AS_OF);
        l.remove();
        l.enter(&valid_key(), AS_OF + DAY).unwrap();
        assert_eq!(l.view(AS_OF + DAY).reason, LicenseReason::Ended);
        // Paid, then no check for 31 days; removed and entered again (after a restart too).
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        let later = AS_OF + 31 * DAY;
        l.remove();
        let mut again = License::load(None, l.record().clone(), later);
        assert_eq!(
            again.view(later).last_tried,
            None,
            "no key: nothing about checks"
        );
        again.enter(&valid_key(), later).unwrap();
        assert_eq!(again.edition(later), Edition::Free);
        assert_eq!(again.view(later).reason, LicenseReason::NoCheck);
    }

    /// Review finding: a kept key that no longer checks (its signing key retired) was dropped
    /// with no word on screen, and its record wiped.
    #[test]
    fn a_kept_key_that_no_longer_checks_says_why_and_keeps_its_record() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        let damaged = format!("{}x", valid_key());
        let kept = License::load(Some(&damaged), l.record().clone(), AS_OF + 60);
        assert_eq!(kept.edition(AS_OF + 60), Edition::Free);
        let problem = kept.view(AS_OF + 60).problem.unwrap();
        assert!(problem.contains("no longer works"), "{problem}");
        assert!(kept.record().answer.is_some(), "the record is kept");
    }

    /// Review finding: a record changed by hand (no time entered, or one in the future) made
    /// the 30 days slide on for ever.
    #[test]
    fn a_missing_or_future_entered_time_is_not_believed() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        let text = l.key().unwrap().text().to_owned();
        for entered_at in [None, Some(AS_OF + 1000 * DAY)] {
            let record = Record {
                entered_at,
                ..l.record().clone()
            };
            let loaded = License::load(Some(&text), record, AS_OF);
            assert_eq!(loaded.edition(AS_OF + 29 * DAY), Edition::Pro);
            assert_eq!(
                loaded.edition(AS_OF + 31 * DAY),
                Edition::Free,
                "{entered_at:?}"
            );
        }
    }

    #[test]
    fn a_bad_key_changes_nothing() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        assert_eq!(l.enter("nonsense", AS_OF), Err(KeyError::Malformed));
        assert_eq!(l.key().unwrap().key_id(), KEY_ID);
    }

    #[test]
    fn an_answer_is_recorded_and_the_next_check_is_a_week_later() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF - 10).unwrap();
        let out = l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        assert_eq!(out, CheckOutcome::Answered(SubscriptionState::Active));
        assert!(!l.check_due(AS_OF + EVERY - 1));
        assert!(l.check_due(AS_OF + EVERY));
        let v = l.view(AS_OF);
        assert_eq!(v.reason, LicenseReason::Active);
        assert_eq!(v.last_checked, Some(AS_OF * 1000));
    }

    #[test]
    fn garbage_from_the_service_is_a_failure_and_pro_stays_on() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        let out = l.answered(b"<html>oops</html>", AS_OF + 60);
        assert!(matches!(out, CheckOutcome::Failed(_)));
        assert_eq!(l.edition(AS_OF + 60), Edition::Pro);
        assert!(l.view(AS_OF + 60).problem.is_some());
    }

    #[test]
    fn removing_the_key_returns_to_free() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        assert!(l.remove().is_some());
        assert_eq!(l.edition(AS_OF), Edition::Free);
        assert!(!l.check_due(AS_OF + 100 * DAY));
        // The clock Plenipo saw is kept.
        assert_eq!(l.record().clock_high, AS_OF);
    }

    #[test]
    fn what_was_kept_loads_again_and_a_changed_record_is_started_over() {
        let mut l = License::default();
        l.enter(&valid_key(), AS_OF).unwrap();
        l.answered(&signed(&answer(SubscriptionState::Active)), AS_OF);
        let text = l.key().unwrap().text().to_owned();
        let record = l.record().clone();
        let again = License::load(Some(&text), record.clone(), AS_OF + 60);
        assert_eq!(again.view(AS_OF + 60).reason, LicenseReason::Active);
        // The key in the Vault is gone: Free.
        let gone = License::load(None, record.clone(), AS_OF + 60);
        assert_eq!(gone.edition(AS_OF + 60), Edition::Free);
        // A key that no longer checks is ignored.
        let broken = License::load(Some("plenipo1.x.y"), record, AS_OF + 60);
        assert_eq!(broken.edition(AS_OF + 60), Edition::Free);
    }
}
