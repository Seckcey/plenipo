//! Plenipo licensing (Phase 11A): Free and Pro, the license key, the weekly check, and the one
//! place every Free limit is decided.
//!
//! - [`key`]: the license key, checked on the PC against 8 West's public keys ([`trust`]).
//! - [`answer`]: the weekly check's exact request, and the signed answer Plenipo trusts.
//! - [`state`] and [`license`]: Free or Pro now — the 30-day grace, the clock rule, and when to
//!   check next. A Free copy never checks in.
//! - [`entitlements`]: [`Entitlements::check`], the single enforcement point.
//!
//! The contract both sides test against is `contracts/license-check/v1` (ADR-101 §3).
//! Plenipo is made by 8 West Ventures, LLC.

pub mod answer;
pub mod codec;
pub mod entitlements;
pub mod key;
pub mod license;
pub mod state;
pub mod trust;
pub mod words;

#[cfg(test)]
mod contract;

pub use answer::{SignedAnswer, SubscriptionState};
pub use entitlements::{Admission, Blocked, Decision, Edition, Entitlements, Limit, Usage};
pub use key::{KeyEdition, KeyError, LicenseKey, Organizations, Plan};
pub use license::{CheckOutcome, FreeLimits, License, LicenseReason, LicenseView};
pub use state::{Record, RecordCopy};

/// Where the weekly check goes (ADR-105): built into every copy, and never changed.
pub const CHECK_ADDRESS: &str = "https://account.getplenipo.com/v1/check";

/// The PC's clock, in Unix seconds.
pub fn clock() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
