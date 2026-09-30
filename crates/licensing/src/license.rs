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
}

fn ms(secs: i64) -> i64 {
    secs.saturating_mul(1000)
}

impl License {
    /// What was kept: the key text from the Vault (checked again) and the record. A key that no
    /// longer checks (for example, a test key in a release build) is ignored.
    pub fn load(key_text: Option<&str>, record: Record, clock: i64) -> Self {
        let key = key_text.and_then(|t| key::parse(t).ok());
        let mut record = record;
        if key.as_ref().map(LicenseKey::key_id) != record.key_id.as_deref() {
            // The record is about another key (or none): start over for this one.
            record = match &key {
                Some(k) => Record::entered(k, clock, &record),
                None => Record {
                    clock_high: record.clock_high,
                    ..Record::default()
                },
            };
        }
        record.saw_clock(clock);
        Self { key, record }
    }

    pub fn key(&self) -> Option<&LicenseKey> {
        self.key.as_ref()
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    /// Enter a key. It is checked at once, with no network; on success it replaces any other.
    pub fn enter(&mut self, text: &str, clock: i64) -> Result<&LicenseKey, KeyError> {
        let key = key::parse(text)?;
        self.record = Record::entered(&key, clock, &self.record);
        Ok(self.key.insert(key))
    }

    /// Remove the key: Free, with nothing else changed.
    pub fn remove(&mut self) -> Option<LicenseKey> {
        self.record = Record {
            clock_high: self.record.clock_high,
            ..Record::default()
        };
        self.key.take()
    }

    pub fn status(&self, clock: i64) -> Status {
        self.record.status(self.key.as_ref(), clock)
    }

    pub fn edition(&self, clock: i64) -> Edition {
        self.status(clock).edition
    }

    /// Whether a check is due at `clock` (never, with no key: a Free copy never checks in).
    pub fn check_due(&self, clock: i64) -> bool {
        self.key.is_some() && self.record.next_check().is_some_and(|at| clock >= at)
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
        LicenseView {
            edition: status.edition,
            reason: status.reason.into(),
            key_id: payload.map(|p| p.key_id.clone()),
            holder: payload.map(|p| p.holder.clone()),
            plan: payload.map(|p| p.plan),
            paid_through: status.paid_through.map(ms),
            ends_at: status.ends_at.map(ms),
            last_checked: status.last_success.map(ms),
            last_tried: self.record.last_attempt.map(ms),
            next_check: self
                .key
                .as_ref()
                .and(self.record.next_check())
                .map(|at| ms(at.max(clock))),
            grace_ends: status.grace_ends.map(ms),
            problem: self.record.last_problem.clone(),
            free_limits: FREE_LIMITS,
            test_build: crate::trust::built_for_tests(),
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
