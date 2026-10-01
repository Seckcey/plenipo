//! Spending on paid AI keys (Phase 16 Wave 3, ADR-085; ADR-036 §2, every AI model worth having).
//!
//! The owner's **spending caps** — a monthly amount for the whole business, a department, or one
//! position — and a **record of every paid task**. Caps are the owner's to set, and none is needed
//! (the owner changed choice 1 on 2026-09-30): with no cap covering a task, it has no dollar
//! limit, and it is still priced and recorded. Before a paid task's request is sent, the most it
//! could cost is set aside, and only if it fits under every cap that covers it: the business, the
//! task's department, and its position. So the hard stop never goes over (choice 3); work can stop a
//! little before 100%. When the task ends, its record says what it really cost, or that the bill
//! could not be read ("not priced yet", counted at the most it could have cost, never as zero;
//! choice 4).
//!
//! The month is the calendar month in Pacific time (choice 2). Money is whole millionths of a
//! dollar ("micros"), never a floating-point number. At 80% of a cap the owner is warned once a
//! month, and when a cap stops paid work the owner is told once a month too (events that become
//! Windows notices and a banner on every page).
//!
//! Checking the caps and setting money aside happen in one Ledger transaction, so two tasks
//! can never both fit into the same last dollars. The key itself is never here: a record names
//! the key by its reference ID and name only.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::dto::{LedgerEvent, NewEvent};
use crate::error::{LedgerError, Result};
use crate::events;
use crate::rows::{json as parse_json, opt_u64, u64_of};
use crate::Ledger;

/// Where the caps are kept (Ledger settings).
pub const SETTING: &str = "spending";
/// One dollar, in the micros money is counted in.
pub const MICROS_PER_DOLLAR: u64 = 1_000_000;
/// The smallest cap: one cent a month.
pub const MIN_CAP_MICROS: u64 = 10_000;
/// The largest cap: a million dollars a month.
pub const MAX_CAP_MICROS: u64 = 1_000_000 * MICROS_PER_DOLLAR;
/// The most a single paid task may set aside: the largest cap.
pub const MAX_SET_ASIDE_MICROS: u64 = MAX_CAP_MICROS;
/// At most this many caps (the business, and one per department and position).
pub const MAX_CAPS: usize = 500;
/// The warning comes at this share of a cap.
pub const WARN_PERCENT: u64 = 80;
/// The spending page lists at most this many of the month's records, newest first.
pub const MAX_RECENT: usize = 50;
/// Longest note on a record.
const MAX_DETAIL: usize = 500;

const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 24 * HOUR_MS;

// ---- Caps ------------------------------------------------------------------------------------

/// What a spending cap covers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum CapCovers {
    /// Every paid task.
    Business,
    /// Paid tasks of the department's positions (worked out from who each reports to, or the
    /// team an agent is lent to).
    Department { id: String },
    /// One position's paid tasks.
    Position { id: String },
}

/// A monthly spending cap the owner set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpendingCap {
    pub id: String,
    pub covers: CapCovers,
    /// The monthly amount, in millionths of a dollar.
    #[ts(type = "number")]
    pub monthly_micros: u64,
    #[ts(type = "number")]
    pub set_at: u64,
    pub set_by: String,
}

/// What the owner has been told about a cap this month, so each is said once a month.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CapMarks {
    month: String,
    warned: bool,
    stopped: bool,
    /// Why paid work stopped under this cap, in plain words.
    stopped_why: Option<String>,
}

/// The setting: the caps, and what the owner has been told this month.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SpendingConfig {
    caps: Vec<SpendingCap>,
    marks: BTreeMap<String, CapMarks>,
}

impl SpendingConfig {
    fn read(c: &Connection) -> Result<Self> {
        let value = c
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [SETTING],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map_or(Value::Null, parse_json);
        Ok(if value.is_null() {
            Self::default()
        } else {
            serde_json::from_value(value).map_err(|e| {
                LedgerError::InvalidInput(format!("the spending caps could not be read: {e}"))
            })?
        })
    }

    fn write(&self, c: &Connection) -> Result<()> {
        c.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![SETTING, serde_json::to_string(self)?, now_i64()],
        )?;
        Ok(())
    }

    fn business(&self) -> Option<&SpendingCap> {
        self.caps.iter().find(|c| c.covers == CapCovers::Business)
    }

    fn marks_for(&mut self, cap_id: &str, month: &str) -> &mut CapMarks {
        let marks = self.marks.entry(cap_id.to_owned()).or_default();
        if marks.month != month {
            *marks = CapMarks {
                month: month.to_owned(),
                ..CapMarks::default()
            };
        }
        marks
    }
}

// ---- The spending page -----------------------------------------------------------------------

/// Where a cap stands this month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CapState {
    /// Under 80%.
    Ok,
    /// 80% or more of the cap is spent.
    Warning,
    /// The cap is used up, or it stopped a paid task this month.
    Stopped,
}

/// A cap and how much of it this month has used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapStatus {
    pub cap: SpendingCap,
    /// "The business", the department's name, or the position's title.
    pub label: String,
    /// The department or position is no longer in the organization.
    pub gone: bool,
    /// Spent this month (tasks not priced yet count at the most they could have cost).
    #[ts(type = "number")]
    pub spent_micros: u64,
    /// Set aside for paid tasks running now.
    #[ts(type = "number")]
    pub set_aside_micros: u64,
    /// What is left for new paid tasks this month.
    #[ts(type = "number")]
    pub left_micros: u64,
    pub state: CapState,
    /// Why paid work stopped under this cap this month, in plain words.
    pub stopped_why: Option<String>,
}

/// Where a spending record stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SpendingState {
    /// The task is running; the most it could cost is set aside.
    SetAside,
    /// The task ended and its bill was read.
    Spent,
    /// The task ended but its bill could not be read: counted at the most it could have cost.
    NotPriced,
    /// The request was never sent: nothing spent.
    Released,
}

impl SpendingState {
    fn as_str(self) -> &'static str {
        match self {
            Self::SetAside => "setAside",
            Self::Spent => "spent",
            Self::NotPriced => "notPriced",
            Self::Released => "released",
        }
    }

    fn parse(s: &str) -> rusqlite::Result<Self> {
        Ok(match s {
            "setAside" => Self::SetAside,
            "spent" => Self::Spent,
            "notPriced" => Self::NotPriced,
            "released" => Self::Released,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    format!("unknown spending state {other}").into(),
                ))
            }
        })
    }
}

/// How a task's cost was worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PricedBy {
    /// The service's own bill for the request (OpenRouter reports it).
    Service,
    /// Token counts priced from Plenipo's dated price list.
    PriceList,
}

impl PricedBy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::PriceList => "priceList",
        }
    }

    fn parse(s: Option<String>) -> Option<Self> {
        match s.as_deref() {
            Some("service") => Some(Self::Service),
            Some("priceList") => Some(Self::PriceList),
            _ => None,
        }
    }
}

/// One paid task's spending record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpendingRecord {
    pub id: String,
    pub task_id: Option<String>,
    pub position_id: Option<String>,
    pub position_title: Option<String>,
    pub department_id: Option<String>,
    pub department_name: Option<String>,
    pub runtime: String,
    pub model: String,
    /// The paid key's name (never the key).
    pub key_name: Option<String>,
    /// The Pacific month it counts in, "2026-10".
    pub month: String,
    pub state: SpendingState,
    /// The most it could cost, set aside before its request was sent.
    #[ts(type = "number")]
    pub set_aside_micros: u64,
    /// What it really cost, when its bill was read.
    #[ts(type = "number | null")]
    pub spent_micros: Option<u64>,
    pub priced_by: Option<PricedBy>,
    pub detail: Option<String>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub settled_at: Option<u64>,
}

/// The Spending caps page: this month, each cap, and the month's latest paid tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpendingPage {
    /// This Pacific month, "2026-10".
    pub month: String,
    #[ts(type = "number")]
    pub month_starts_at: u64,
    /// When the month turns over (midnight on the 1st, Pacific time).
    #[ts(type = "number")]
    pub resets_at: u64,
    /// The business cap exists. Not needed: without it the business has no dollar limit.
    pub has_business_cap: bool,
    /// The business cap first, then departments', then positions'.
    pub caps: Vec<CapStatus>,
    /// Spent this month on every paid task.
    #[ts(type = "number")]
    pub spent_micros: u64,
    /// Set aside for paid tasks running now.
    #[ts(type = "number")]
    pub set_aside_micros: u64,
    /// Paid tasks this month whose bill could not be read.
    pub not_priced: u32,
    /// This month's paid tasks, newest first (at most [`MAX_RECENT`]).
    pub recent: Vec<SpendingRecord>,
}

// ---- Setting money aside, and the bill ---------------------------------------------------------

/// A paid task about to send its request: who it is for, and the most it could cost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaidTask {
    pub task_id: Option<String>,
    pub execution_id: Option<String>,
    /// The position doing the work: its ID and title.
    pub position: Option<(String, String)>,
    /// The department the work counts for (the team it is lent to while on loan): its ID and
    /// name.
    pub department: Option<(String, String)>,
    pub runtime: String,
    pub model: String,
    /// The paid key by its reference ID and name (never the key).
    pub key: Option<(String, String)>,
    /// The most the task's request could cost, in micros.
    pub most_micros: u64,
}

/// Money set aside for a paid task: settle it with [`Ledger::settle_spending`] when the task
/// ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAside {
    pub record_id: String,
    pub month: String,
    pub most_micros: u64,
}

/// Why a paid task may not start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingRefusal {
    /// The cap that stopped it.
    pub cap_id: Option<String>,
    /// What to tell the owner, in plain words.
    pub reason: String,
}

/// What a finished paid task cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bill {
    /// Its bill was read.
    Spent { micros: u64, priced_by: PricedBy },
    /// The bill could not be read; `detail` says why.
    NotPriced { detail: String },
    /// The request was never sent.
    Released,
}

/// What settling a task's spending found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settled {
    /// The caps whose amount this month is now passed (the bill was more than was set aside):
    /// the caller stops the task's work at once and the owner is told.
    pub passed: Vec<String>,
}

// ---- Money and Pacific months ----------------------------------------------------------------

/// `micros` as dollars for the owner: "$12.34"; under a cent, "$0.0042"; nothing, "$0.00".
pub fn dollars(micros: u64) -> String {
    if micros > 0 && micros < 10_000 {
        let tenth_thousandths = micros.div_ceil(100);
        return format!("$0.{:04}", tenth_thousandths.clamp(1, 9_999));
    }
    let cents = micros.saturating_add(5_000) / 10_000;
    let whole = cents / 100;
    let mut digits = whole.to_string();
    let mut grouped = String::new();
    while digits.len() > 3 {
        let tail = digits.split_off(digits.len() - 3);
        grouped = format!(",{tail}{grouped}");
    }
    format!("${digits}{grouped}.{:02}", cents % 100)
}

/// Money rounded up to the cent, for what a task could cost (never shown as less than it is).
fn dollars_up(micros: u64) -> String {
    if micros < 10_000 {
        dollars(micros)
    } else {
        dollars(micros.div_ceil(10_000).saturating_mul(10_000))
    }
}

/// Money rounded down to the cent, for what is left (never shown as more than it is).
fn dollars_down(micros: u64) -> String {
    if micros >= 10_000 {
        dollars(micros / 10_000 * 10_000)
    } else if micros >= 100 {
        format!("$0.{:04}", micros / 100)
    } else {
        dollars(0)
    }
}

/// Days since 1970-01-01 → (year, month, day) (the proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// (year, month, day) → days since 1970-01-01.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 0 = Sunday.
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// The day of the `nth` Sunday (1-based) of `month` in `year`, as days since 1970-01-01.
fn nth_sunday(year: i64, month: u32, nth: i64) -> i64 {
    let first = days_from_civil(year, month, 1);
    let to_sunday = (7 - weekday(first)).rem_euclid(7);
    first + to_sunday + 7 * (nth - 1)
}

/// Pacific time is on daylight saving time at `ms` (UTC): from 2:00 on the second Sunday in
/// March to 2:00 on the first Sunday in November, local time (United States rules since 2007;
/// change here if the law changes).
fn pacific_daylight(ms: u64) -> bool {
    let (year, _, _) = civil_from_days(i64::try_from(ms / DAY_MS).unwrap_or(0));
    let start = u64::try_from(nth_sunday(year, 3, 2)).unwrap_or(0) * DAY_MS + 10 * HOUR_MS;
    let end = u64::try_from(nth_sunday(year, 11, 1)).unwrap_or(0) * DAY_MS + 9 * HOUR_MS;
    (start..end).contains(&ms)
}

/// The Pacific month `ms` (UTC) falls in: (year, month).
pub fn pacific_month(ms: u64) -> (i64, u32) {
    let offset = if pacific_daylight(ms) { 7 } else { 8 } * HOUR_MS;
    let local = ms.saturating_sub(offset);
    let (year, month, _) = civil_from_days(i64::try_from(local / DAY_MS).unwrap_or(0));
    (year, month)
}

/// "2026-10".
pub fn month_key(year: i64, month: u32) -> String {
    format!("{year:04}-{month:02}")
}

/// Midnight on the 1st of the month, Pacific time, in ms (UTC). Daylight saving time never
/// changes at midnight (it changes at 2:00), and it is on at the start of April to November.
pub fn month_start_ms(year: i64, month: u32) -> u64 {
    let days = u64::try_from(days_from_civil(year, month, 1)).unwrap_or(0);
    let offset = if (4..=11).contains(&month) { 7 } else { 8 } * HOUR_MS;
    days * DAY_MS + offset
}

/// The month after (year, month).
fn next_month(year: i64, month: u32) -> (i64, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

// ---- Helpers ---------------------------------------------------------------------------------

fn now_i64() -> i64 {
    i64::try_from(crate::now_ms()).unwrap_or(i64::MAX)
}

fn invalid(message: impl Into<String>) -> LedgerError {
    LedgerError::InvalidInput(message.into())
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// An optional ID: 1–64 characters, one line.
fn clean_id(what: &str, v: Option<&str>) -> Result<Option<String>> {
    match v.map(str::trim) {
        None => Ok(None),
        Some(v) if v.is_empty() || v.chars().count() > 64 || v.chars().any(char::is_control) => {
            Err(invalid(format!(
                "{what} must be 1–64 characters on one line"
            )))
        }
        Some(v) => Ok(Some(v.to_owned())),
    }
}

/// An optional name: at most `max` characters, one line (cut, not refused: it is a label).
fn clean_label(v: Option<&str>, max: usize) -> Option<String> {
    v.map(|v| {
        v.chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect::<String>()
            .trim()
            .to_owned()
    })
    .filter(|v| !v.is_empty())
}

fn event(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    actor: &str,
    task_id: Option<&str>,
    event_type: &str,
    payload: Value,
) -> Result<()> {
    out.push(events::insert(
        tx,
        NewEvent {
            task_id: task_id.map(str::to_owned),
            source: actor.to_owned(),
            event_type: event_type.to_owned(),
            payload,
            ..NewEvent::default()
        },
    )?);
    Ok(())
}

/// How much `covers` has used in `month`: (counted, set aside now). Counted is what was spent,
/// tasks not priced yet at the most they could have cost, and what is set aside for running
/// tasks; released records count as nothing.
fn used(c: &Connection, month: &str, covers: &CapCovers) -> Result<(u64, u64)> {
    let (department, position) = match covers {
        CapCovers::Business => (None, None),
        CapCovers::Department { id } => (Some(id.as_str()), None),
        CapCovers::Position { id } => (None, Some(id.as_str())),
    };
    let (counted, pending): (i64, i64) = c.query_row(
        "SELECT
             COALESCE(SUM(CASE state WHEN 'spent' THEN spent_micros
                                     WHEN 'released' THEN 0
                                     ELSE set_aside_micros END), 0),
             COALESCE(SUM(CASE state WHEN 'setAside' THEN set_aside_micros ELSE 0 END), 0)
         FROM spending
         WHERE month = ?1
           AND (?2 IS NULL OR department_id = ?2)
           AND (?3 IS NULL OR position_id = ?3)",
        params![month, department, position],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((u64_of(counted), u64_of(pending)))
}

/// The caps that cover a task of `position` in `department`: the business's, then the
/// department's and the position's, when the owner set them.
fn covering<'a>(
    config: &'a SpendingConfig,
    department: Option<&str>,
    position: Option<&str>,
) -> Vec<&'a SpendingCap> {
    config
        .caps
        .iter()
        .filter(|cap| match &cap.covers {
            CapCovers::Business => true,
            CapCovers::Department { id } => Some(id.as_str()) == department,
            CapCovers::Position { id } => Some(id.as_str()) == position,
        })
        .collect()
}

/// "The business", a department's name, or a position's title; and whether it is gone.
fn label_of(c: &Connection, covers: &CapCovers) -> Result<(String, bool)> {
    let found: Option<String> = match covers {
        CapCovers::Business => return Ok(("The business".to_owned(), false)),
        CapCovers::Department { id } => c
            .query_row(
                "SELECT name FROM departments WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?,
        CapCovers::Position { id } => c
            .query_row(
                "SELECT title FROM positions WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?,
    };
    Ok(match found {
        Some(name) => (name, false),
        None => (
            match covers {
                CapCovers::Department { .. } => "A department no longer in the organization",
                _ => "A position no longer in the organization",
            }
            .to_owned(),
            true,
        ),
    })
}

/// "the Business cap" / "Operations's cap": how a refusal names a cap.
fn cap_words(label: &str, covers: &CapCovers) -> String {
    match covers {
        CapCovers::Business => "the business's spending cap".to_owned(),
        _ => format!("the spending cap for {label}"),
    }
}

fn record(r: &rusqlite::Row<'_>) -> rusqlite::Result<SpendingRecord> {
    Ok(SpendingRecord {
        id: r.get(0)?,
        task_id: r.get(1)?,
        position_id: r.get(2)?,
        position_title: r.get(3)?,
        department_id: r.get(4)?,
        department_name: r.get(5)?,
        runtime: r.get(6)?,
        model: r.get(7)?,
        key_name: r.get(8)?,
        month: r.get(9)?,
        state: SpendingState::parse(&r.get::<_, String>(10)?)?,
        set_aside_micros: u64_of(r.get(11)?),
        spent_micros: opt_u64(r.get(12)?),
        priced_by: PricedBy::parse(r.get(13)?),
        detail: r.get(14)?,
        created_at: u64_of(r.get(15)?),
        settled_at: opt_u64(r.get(16)?),
    })
}

const RECORD_COLUMNS: &str = "id, task_id, position_id, position_title, department_id, \
     department_name, runtime, model, key_name, month, state, set_aside_micros, spent_micros, \
     priced_by, detail, created_at, settled_at";

/// A settled record, for [`after_spending`].
struct Settling<'a> {
    /// The record's own month.
    month: &'a str,
    /// The month it is now (Pacific time).
    this_month: &'a str,
    department: Option<&'a str>,
    position: Option<&'a str>,
    task_id: Option<&'a str>,
    /// The bill was more than was set aside for it.
    over_set_aside: bool,
}

/// After a record is settled. For the current month only, each cap covering it is warned at 80%
/// and stopped at 100%, once each: a late bill from an earlier month changes no mark of this
/// month's and says nothing about "this month". Returns the caps this bill passed: only when it
/// was more than was set aside for it and the month's spending is now over the cap (the caller
/// stops the task's work at once).
fn after_spending(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    config: &mut SpendingConfig,
    record: &Settling<'_>,
) -> Result<Vec<String>> {
    let month = record.month;
    let caps: Vec<SpendingCap> = covering(config, record.department, record.position)
        .into_iter()
        .cloned()
        .collect();
    let mut passed = Vec::new();
    let mut changed = false;
    for cap in caps {
        let (counted, pending) = used(tx, month, &cap.covers)?;
        let (label, _) = label_of(tx, &cap.covers)?;
        if record.over_set_aside && counted > cap.monthly_micros {
            passed.push(label.clone());
            event(
                tx,
                out,
                "plenipo",
                record.task_id,
                "spending.passed",
                json!({
                    "capId": cap.id,
                    "covers": cap.covers,
                    "label": label,
                    "month": month,
                    "countedMicros": counted,
                    "capMicros": cap.monthly_micros,
                }),
            )?;
        }
        if month != record.this_month {
            continue;
        }
        let settled = counted.saturating_sub(pending);
        let percent = settled.saturating_mul(100) / cap.monthly_micros.max(1);
        let marks = config.marks_for(&cap.id, month);
        if settled.saturating_mul(100) >= cap.monthly_micros.saturating_mul(WARN_PERCENT)
            && !marks.warned
        {
            marks.warned = true;
            changed = true;
            event(
                tx,
                out,
                "plenipo",
                None,
                "spending.warning",
                json!({
                    "capId": cap.id,
                    "covers": cap.covers,
                    "label": label,
                    "month": month,
                    "percent": percent,
                    "spentMicros": settled,
                    "capMicros": cap.monthly_micros,
                }),
            )?;
        }
        if settled >= cap.monthly_micros && !marks.stopped {
            marks.stopped = true;
            marks.stopped_why = Some(format!(
                "{} of {} is spent this month.",
                dollars(settled),
                dollars(cap.monthly_micros)
            ));
            changed = true;
            event(
                tx,
                out,
                "plenipo",
                None,
                "spending.stopped",
                json!({
                    "capId": cap.id,
                    "covers": cap.covers,
                    "label": label,
                    "month": month,
                    "why": "full",
                    "spentMicros": settled,
                    "capMicros": cap.monthly_micros,
                }),
            )?;
        }
    }
    if changed {
        config.write(tx)?;
    }
    Ok(passed)
}

// ---- The Ledger's spending -------------------------------------------------------------------

impl Ledger {
    /// The Spending caps page at `now` (ms).
    pub fn spending_page(&self, now: u64) -> Result<SpendingPage> {
        let (year, month) = pacific_month(now);
        let key = month_key(year, month);
        let (next_year, next) = next_month(year, month);
        self.read(|c| {
            let config = SpendingConfig::read(c)?;
            let mut caps = Vec::new();
            let order = |covers: &CapCovers| match covers {
                CapCovers::Business => 0,
                CapCovers::Department { .. } => 1,
                CapCovers::Position { .. } => 2,
            };
            let mut sorted: Vec<&SpendingCap> = config.caps.iter().collect();
            sorted.sort_by_key(|cap| order(&cap.covers));
            for cap in sorted {
                let (counted, pending) = used(c, &key, &cap.covers)?;
                let (label, gone) = label_of(c, &cap.covers)?;
                let settled = counted.saturating_sub(pending);
                let marks = config
                    .marks
                    .get(&cap.id)
                    .filter(|m| m.month == key)
                    .cloned()
                    .unwrap_or_default();
                let state = if marks.stopped || settled >= cap.monthly_micros {
                    CapState::Stopped
                } else if settled.saturating_mul(100)
                    >= cap.monthly_micros.saturating_mul(WARN_PERCENT)
                {
                    CapState::Warning
                } else {
                    CapState::Ok
                };
                caps.push(CapStatus {
                    cap: cap.clone(),
                    label,
                    gone,
                    spent_micros: settled,
                    set_aside_micros: pending,
                    left_micros: cap.monthly_micros.saturating_sub(counted),
                    state,
                    stopped_why: if state == CapState::Stopped {
                        marks.stopped_why.or_else(|| {
                            Some(format!(
                                "{} of {} is spent this month.",
                                dollars(settled),
                                dollars(cap.monthly_micros)
                            ))
                        })
                    } else {
                        None
                    },
                });
            }
            let (counted, pending) = used(c, &key, &CapCovers::Business)?;
            let not_priced: i64 = c.query_row(
                "SELECT COUNT(*) FROM spending WHERE month = ?1 AND state = 'notPriced'",
                [&key],
                |r| r.get(0),
            )?;
            let mut stmt = c.prepare(&format!(
                "SELECT {RECORD_COLUMNS} FROM spending WHERE month = ?1
                 ORDER BY created_at DESC, id DESC LIMIT ?2"
            ))?;
            let recent = stmt
                .query_map(params![key, to_i64(MAX_RECENT as u64)], record)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(SpendingPage {
                month: key.clone(),
                month_starts_at: month_start_ms(year, month),
                resets_at: month_start_ms(next_year, next),
                has_business_cap: config.business().is_some(),
                caps,
                spent_micros: counted.saturating_sub(pending),
                set_aside_micros: pending,
                not_priced: u32::try_from(not_priced).unwrap_or(u32::MAX),
                recent,
            })
        })
    }

    /// What is left this month for a paid task of `position` in `department`: the smallest amount
    /// left under the caps covering it, or `u64::MAX` when no cap covers it (no dollar limit).
    /// Always `Some`: the Router's `None` is a Ledger it could not read.
    pub fn spending_room(
        &self,
        department: Option<&str>,
        position: Option<&str>,
        now: u64,
    ) -> Result<Option<u64>> {
        let (year, month) = pacific_month(now);
        let key = month_key(year, month);
        self.read(|c| {
            let config = SpendingConfig::read(c)?;
            let mut room = u64::MAX;
            for cap in covering(&config, department, position) {
                let (counted, _) = used(c, &key, &cap.covers)?;
                room = room.min(cap.monthly_micros.saturating_sub(counted));
            }
            Ok(Some(room))
        })
    }

    /// The owner's caps.
    pub fn spending_caps(&self) -> Result<Vec<SpendingCap>> {
        self.read(|c| Ok(SpendingConfig::read(c)?.caps))
    }

    /// The business cap exists. Not needed: without it the business has no dollar limit.
    pub fn has_business_cap(&self) -> Result<bool> {
        self.read(|c| Ok(SpendingConfig::read(c)?.business().is_some()))
    }

    /// Set (or change) the monthly cap for `covers`. One cap per business, department, or
    /// position; a department or position must be in the organization. Raising a cap that
    /// stopped paid work this month lets work go on.
    pub fn set_spending_cap(
        &self,
        covers: &CapCovers,
        monthly_micros: u64,
        actor: &str,
        now: u64,
    ) -> Result<SpendingCap> {
        if !(MIN_CAP_MICROS..=MAX_CAP_MICROS).contains(&monthly_micros) {
            return Err(invalid(format!(
                "a spending cap must be from {} to {} a month",
                dollars(MIN_CAP_MICROS),
                dollars(MAX_CAP_MICROS)
            )));
        }
        let covers = match covers {
            CapCovers::Business => CapCovers::Business,
            CapCovers::Department { id } => CapCovers::Department {
                id: clean_id("the department", Some(id))?.unwrap_or_default(),
            },
            CapCovers::Position { id } => CapCovers::Position {
                id: clean_id("the position", Some(id))?.unwrap_or_default(),
            },
        };
        let actor = clean_label(Some(actor), 64).unwrap_or_else(|| "owner".to_owned());
        self.write(|tx, out| {
            let (label, gone) = label_of(tx, &covers)?;
            if gone {
                return Err(match &covers {
                    CapCovers::Department { id } => {
                        LedgerError::NotFound(format!("department {id}"))
                    }
                    CapCovers::Position { id } => LedgerError::NotFound(format!("position {id}")),
                    CapCovers::Business => unreachable!("the business is never gone"),
                });
            }
            let mut config = SpendingConfig::read(tx)?;
            let (year, month) = pacific_month(now);
            let key = month_key(year, month);
            let previous = config.caps.iter().position(|c| c.covers == covers);
            let cap = match previous {
                Some(i) => {
                    let cap = &mut config.caps[i];
                    let before = cap.monthly_micros;
                    cap.monthly_micros = monthly_micros;
                    cap.set_at = now;
                    cap.set_by.clone_from(&actor);
                    let cap = cap.clone();
                    if monthly_micros > before {
                        let (counted, _) = used(tx, &key, &cap.covers)?;
                        let marks = config.marks_for(&cap.id, &key);
                        if counted < monthly_micros {
                            marks.stopped = false;
                            marks.stopped_why = None;
                        }
                        if counted.saturating_mul(100) < monthly_micros.saturating_mul(WARN_PERCENT)
                        {
                            marks.warned = false;
                        }
                    }
                    event(
                        tx,
                        out,
                        &actor,
                        None,
                        "spending.cap_set",
                        json!({
                            "capId": cap.id,
                            "covers": cap.covers,
                            "label": label,
                            "monthlyMicros": monthly_micros,
                            "previousMicros": before,
                        }),
                    )?;
                    cap
                }
                None => {
                    if config.caps.len() >= MAX_CAPS {
                        return Err(invalid(format!("at most {MAX_CAPS} spending caps")));
                    }
                    let cap = SpendingCap {
                        id: uuid::Uuid::new_v4().to_string(),
                        covers: covers.clone(),
                        monthly_micros,
                        set_at: now,
                        set_by: actor.clone(),
                    };
                    config.caps.push(cap.clone());
                    event(
                        tx,
                        out,
                        &actor,
                        None,
                        "spending.cap_set",
                        json!({
                            "capId": cap.id,
                            "covers": cap.covers,
                            "label": label,
                            "monthlyMicros": monthly_micros,
                            "previousMicros": Value::Null,
                        }),
                    )?;
                    cap
                }
            };
            config.write(tx)?;
            Ok(cap)
        })
    }

    /// Remove a cap, the business's included: no cap is ever needed.
    pub fn remove_spending_cap(&self, cap_id: &str, actor: &str) -> Result<()> {
        let cap_id = clean_id("the cap", Some(cap_id))?.unwrap_or_default();
        let actor = clean_label(Some(actor), 64).unwrap_or_else(|| "owner".to_owned());
        self.write(|tx, out| {
            let mut config = SpendingConfig::read(tx)?;
            let i = config
                .caps
                .iter()
                .position(|c| c.id == cap_id)
                .ok_or_else(|| LedgerError::NotFound(format!("spending cap {cap_id}")))?;
            let cap = config.caps.remove(i);
            config.marks.remove(&cap.id);
            let (label, _) = label_of(tx, &cap.covers)?;
            event(
                tx,
                out,
                &actor,
                None,
                "spending.cap_removed",
                json!({ "capId": cap.id, "covers": cap.covers, "label": label }),
            )?;
            config.write(tx)
        })
    }

    /// Set aside the most `task` could cost, if it fits under every cap that covers it (the
    /// business, its department, its position). With no cap covering it, it always fits (no
    /// dollar limit). Refused when it does not fit; a refusal is recorded, and the owner is told
    /// once a month per cap.
    pub fn set_aside_spending(
        &self,
        task: &PaidTask,
        now: u64,
    ) -> Result<std::result::Result<SetAside, SpendingRefusal>> {
        if task.most_micros > MAX_SET_ASIDE_MICROS {
            return Err(invalid("a paid task cannot set aside that much"));
        }
        let task_id = clean_id("the task", task.task_id.as_deref())?;
        let execution_id = clean_id("the execution", task.execution_id.as_deref())?;
        let position_id = clean_id("the position", task.position.as_ref().map(|p| p.0.as_str()))?;
        let position_title = clean_label(task.position.as_ref().map(|p| p.1.as_str()), 200);
        let department_id = clean_id(
            "the department",
            task.department.as_ref().map(|d| d.0.as_str()),
        )?;
        let department_name = clean_label(task.department.as_ref().map(|d| d.1.as_str()), 200);
        let runtime = clean_id("the AI tool", Some(&task.runtime))?.unwrap_or_default();
        let model = clean_label(Some(&task.model), 200)
            .ok_or_else(|| invalid("a paid task names its model"))?;
        let key_id = clean_id("the key", task.key.as_ref().map(|k| k.0.as_str()))?;
        let key_name = clean_label(task.key.as_ref().map(|k| k.1.as_str()), 100);
        let (year, month) = pacific_month(now);
        let key = month_key(year, month);
        self.write(|tx, out| {
            let mut config = SpendingConfig::read(tx)?;
            let refuse = |tx: &Connection,
                          out: &mut Vec<LedgerEvent>,
                          config: &mut SpendingConfig,
                          cap: Option<&SpendingCap>,
                          reason: String|
             -> Result<std::result::Result<SetAside, SpendingRefusal>> {
                event(
                    tx,
                    out,
                    "plenipo",
                    task_id.as_deref(),
                    "spending.refused",
                    json!({
                        "capId": cap.map(|c| c.id.clone()),
                        "reason": reason,
                        "mostMicros": task.most_micros,
                        "runtime": runtime,
                        "model": model,
                        "keyName": key_name,
                    }),
                )?;
                if let Some(cap) = cap {
                    let (label, _) = label_of(tx, &cap.covers)?;
                    let marks = config.marks_for(&cap.id, &key);
                    if !marks.stopped {
                        marks.stopped = true;
                        marks.stopped_why = Some(reason.clone());
                        event(
                            tx,
                            out,
                            "plenipo",
                            None,
                            "spending.stopped",
                            json!({
                                "capId": cap.id,
                                "covers": cap.covers,
                                "label": label,
                                "month": key,
                                "why": "refused",
                                "reason": reason,
                                "capMicros": cap.monthly_micros,
                            }),
                        )?;
                    }
                    config.write(tx)?;
                }
                Ok(Err(SpendingRefusal {
                    cap_id: cap.map(|c| c.id.clone()),
                    reason,
                }))
            };
            // The cap with the least room decides; with none, there is no dollar limit.
            let mut tightest: Option<(SpendingCap, u64)> = None;
            for cap in covering(&config, department_id.as_deref(), position_id.as_deref()) {
                let (counted, _) = used(tx, &key, &cap.covers)?;
                let left = cap.monthly_micros.saturating_sub(counted);
                if tightest.as_ref().is_none_or(|(_, l)| left < *l) {
                    tightest = Some((cap.clone(), left));
                }
            }
            if let Some((cap, left)) = tightest.filter(|(_, left)| task.most_micros > *left) {
                let (label, _) = label_of(tx, &cap.covers)?;
                let reason = format!(
                    "Not started: this task could cost up to {}, and {} is left this month under \
                     {} ({} a month).",
                    dollars_up(task.most_micros),
                    dollars_down(left),
                    cap_words(&label, &cap.covers),
                    dollars(cap.monthly_micros)
                );
                return refuse(tx, out, &mut config, Some(&cap), reason);
            }
            let record_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO spending (id, task_id, execution_id, position_id, position_title,
                     department_id, department_name, runtime, model, key_id, key_name, month,
                     state, set_aside_micros, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'setAside', ?13, ?14)",
                params![
                    record_id,
                    task_id,
                    execution_id,
                    position_id,
                    position_title,
                    department_id,
                    department_name,
                    runtime,
                    model,
                    key_id,
                    key_name,
                    key,
                    to_i64(task.most_micros),
                    to_i64(now),
                ],
            )?;
            event(
                tx,
                out,
                "plenipo",
                task_id.as_deref(),
                "spending.set_aside",
                json!({
                    "recordId": record_id,
                    "month": key,
                    "mostMicros": task.most_micros,
                    "runtime": runtime,
                    "model": model,
                    "keyName": key_name,
                    "positionId": position_id,
                    "departmentId": department_id,
                }),
            )?;
            Ok(Ok(SetAside {
                record_id,
                month: key.clone(),
                most_micros: task.most_micros,
            }))
        })
    }

    /// Settle a paid task's spending when it ends: what it really cost, that its bill could not
    /// be read (counted at the most it could have cost), or that its request was never sent.
    pub fn settle_spending(&self, record_id: &str, bill: &Bill, now: u64) -> Result<Settled> {
        let record_id = clean_id("the spending record", Some(record_id))?.unwrap_or_default();
        self.write(|tx, out| {
            // Task, department, position, month, state, and what was set aside.
            type Row = (
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                String,
                i64,
            );
            let row: Option<Row> = tx
                .query_row(
                    "SELECT task_id, department_id, position_id, month, state, set_aside_micros
                     FROM spending WHERE id = ?1",
                    [&record_id],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
                )
                .optional()?;
            let (task_id, department, position, month, state, set_aside) =
                row.ok_or_else(|| LedgerError::NotFound(format!("spending record {record_id}")))?;
            if state != SpendingState::SetAside.as_str() {
                return Err(LedgerError::InvalidTransition {
                    entity: "spending record",
                    id: record_id.clone(),
                    from: state,
                    to: "settled".into(),
                });
            }
            let mut over_set_aside = false;
            let (state, spent, priced_by, detail) = match bill {
                Bill::Spent { micros, priced_by } => {
                    // A bill beyond any cap is a mistake somewhere: it is recorded at the most any
                    // cap could be, which stops paid work, and says so. It is never refused, so a
                    // known bill is never counted as less.
                    let (micros, detail) = if *micros > MAX_SET_ASIDE_MICROS {
                        (
                            MAX_SET_ASIDE_MICROS,
                            Some(format!(
                                "The bill read was more than {}, so it is recorded as that much.",
                                dollars(MAX_SET_ASIDE_MICROS)
                            )),
                        )
                    } else {
                        (*micros, None)
                    };
                    over_set_aside = micros > u64_of(set_aside);
                    (
                        SpendingState::Spent,
                        Some(to_i64(micros)),
                        Some(priced_by.as_str()),
                        detail,
                    )
                }
                Bill::NotPriced { detail } => (
                    SpendingState::NotPriced,
                    None,
                    None,
                    clean_label(Some(detail), MAX_DETAIL),
                ),
                Bill::Released => (SpendingState::Released, None, None, None),
            };
            tx.execute(
                "UPDATE spending SET state = ?2, spent_micros = ?3, priced_by = ?4, detail = ?5,
                     settled_at = ?6
                 WHERE id = ?1",
                params![
                    record_id,
                    state.as_str(),
                    spent,
                    priced_by,
                    detail,
                    to_i64(now)
                ],
            )?;
            event(
                tx,
                out,
                "plenipo",
                task_id.as_deref(),
                "spending.recorded",
                json!({
                    "recordId": record_id,
                    "month": month,
                    "state": state,
                    "spentMicros": spent,
                    "pricedBy": priced_by,
                    "detail": detail,
                }),
            )?;
            // The bill is recorded even when the caps cannot be read (a damaged setting): only
            // the warnings wait, and no new paid task starts until the caps can be read.
            let (year, this) = pacific_month(now);
            let this_month = month_key(year, this);
            let passed = match SpendingConfig::read(tx) {
                Ok(mut config) => after_spending(
                    tx,
                    out,
                    &mut config,
                    &Settling {
                        month: &month,
                        this_month: &this_month,
                        department: department.as_deref(),
                        position: position.as_deref(),
                        task_id: task_id.as_deref(),
                        over_set_aside,
                    },
                )?,
                Err(_) => Vec::new(),
            };
            Ok(Settled { passed })
        })
    }

    /// After a restart: money still set aside before `started` (when this run of Plenipo began)
    /// belongs to tasks that stopped with the last run, whose bills were never read. Each is
    /// counted at the most it could have cost ("not priced yet"), never freed, so a cap is never
    /// passed unseen. A task this run started is left alone. Returns how many were found.
    pub fn recover_spending(&self, started: u64, now: u64) -> Result<usize> {
        self.write(|tx, out| {
            let mut stmt = tx.prepare(
                "SELECT id, task_id, department_id, position_id, month
                 FROM spending WHERE state = 'setAside' AND created_at < ?1 ORDER BY created_at",
            )?;
            type Left = (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
            );
            let left: Vec<Left> = stmt
                .query_map([to_i64(started)], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            let detail = "Plenipo stopped before this task's bill was read, so it counts at the \
                          most it could have cost.";
            // Counted even when the caps cannot be read; only the warnings wait.
            let mut config = SpendingConfig::read(tx).ok();
            let (year, this) = pacific_month(now);
            let this_month = month_key(year, this);
            for (id, task_id, department, position, month) in &left {
                tx.execute(
                    "UPDATE spending SET state = 'notPriced', detail = ?2, settled_at = ?3
                     WHERE id = ?1",
                    params![id, detail, to_i64(now)],
                )?;
                event(
                    tx,
                    out,
                    "plenipo",
                    task_id.as_deref(),
                    "spending.recorded",
                    json!({
                        "recordId": id,
                        "month": month,
                        "state": SpendingState::NotPriced,
                        "spentMicros": Value::Null,
                        "pricedBy": Value::Null,
                        "detail": detail,
                    }),
                )?;
                if let Some(config) = config.as_mut() {
                    after_spending(
                        tx,
                        out,
                        config,
                        &Settling {
                            month,
                            this_month: &this_month,
                            department: department.as_deref(),
                            position: position.as_deref(),
                            task_id: task_id.as_deref(),
                            over_set_aside: false,
                        },
                    )?;
                }
            }
            Ok(left.len())
        })
    }

    /// After a restore (ADR-085): the Ledger as it was before is kept as the backup `kept` (a
    /// file name in the backups folder), and its spending records are carried into the restored
    /// Ledger. So money spent since the backup was made still counts, and restoring cannot open
    /// room under a cap. A record the restored Ledger has as set aside takes the kept one's
    /// settlement. Returns how many records came back.
    pub fn carry_spending_from_backup(&self, kept: &str) -> Result<usize> {
        if kept.is_empty() || kept.contains(['/', '\\']) || kept.contains("..") {
            return Err(invalid("that is not a backup's name"));
        }
        let Some(path) = self.backups_dir().map(|d| d.join(kept)) else {
            return Ok(0);
        };
        if !path.is_file() {
            return Ok(0);
        }
        self.conn().execute(
            "ATTACH DATABASE ?1 AS kept",
            [path.to_string_lossy().as_ref()],
        )?;
        let carried = self.carry_attached(kept);
        let _ = self.conn().execute_batch("DETACH DATABASE kept");
        carried
    }

    fn carry_attached(&self, kept: &str) -> Result<usize> {
        let has_table: bool = self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM kept.sqlite_master
                            WHERE type = 'table' AND name = 'spending')",
            [],
            |r| r.get(0),
        )?;
        if !has_table {
            return Ok(0);
        }
        self.write(|tx, out| {
            const COLUMNS: &str = "id, task_id, execution_id, position_id, position_title,                  department_id, department_name, runtime, model, key_id, key_name, month, state,                  set_aside_micros, spent_micros, priced_by, detail, created_at, settled_at";
            let added = tx.execute(
                &format!(
                    "INSERT INTO main.spending ({COLUMNS})
                     SELECT {COLUMNS} FROM kept.spending
                     WHERE id NOT IN (SELECT id FROM main.spending)"
                ),
                [],
            )?;
            let settled = tx.execute(
                "UPDATE main.spending
                 SET state = k.state, spent_micros = k.spent_micros, priced_by = k.priced_by,
                     detail = k.detail, settled_at = k.settled_at
                 FROM kept.spending AS k
                 WHERE main.spending.id = k.id
                   AND main.spending.state = 'setAside' AND k.state <> 'setAside'",
                [],
            )?;
            if added + settled > 0 {
                event(
                    tx,
                    out,
                    "plenipo",
                    None,
                    "spending.carried_over",
                    json!({ "backup": kept, "records": added, "settled": settled }),
                )?;
            }
            Ok(added)
        })
    }

    /// A task's spending records, oldest first.
    pub fn spending_for_task(&self, task_id: &str) -> Result<Vec<SpendingRecord>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {RECORD_COLUMNS} FROM spending WHERE task_id = ?1
                 ORDER BY created_at, id"
            ))?;
            let rows = stmt
                .query_map([task_id], record)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{NewPosition, RoleType};
    use crate::workforce::RoleTemplate;

    const D: u64 = MICROS_PER_DOLLAR;

    /// 2026-10-15 12:00 Pacific (daylight saving time).
    const OCT_15: u64 = 1_792_090_800_000;

    fn ledger() -> Ledger {
        Ledger::open_in_memory().unwrap()
    }

    fn task(most: u64) -> PaidTask {
        PaidTask {
            task_id: None,
            runtime: "openrouter".into(),
            model: "moonshotai/kimi-k3".into(),
            key: Some(("key-1".into(), "Office key".into())),
            most_micros: most,
            ..PaidTask::default()
        }
    }

    fn position(l: &Ledger) -> (String, String) {
        let roles = l
            .ensure_roles(
                &[RoleTemplate {
                    name: "Superintendent",
                    description: "",
                    role_type: RoleType::Superintendent,
                    persistent: true,
                    metadata: Value::Null,
                    formerly: &[],
                }],
                "plenipo",
            )
            .unwrap();
        let (p, _) = l
            .create_position(
                &NewPosition {
                    title: "Chief of Staff".into(),
                    role_id: roles[0].id.clone(),
                    runtime_id: Some("codex".into()),
                    runtime_provider: Some("openai".into()),
                    staffed: false,
                    ..NewPosition::default()
                },
                "owner",
            )
            .unwrap();
        (p.id, p.title)
    }

    fn fits(r: std::result::Result<SetAside, SpendingRefusal>) -> SetAside {
        r.unwrap_or_else(|refusal| panic!("refused: {}", refusal.reason))
    }

    fn refused(r: std::result::Result<SetAside, SpendingRefusal>) -> SpendingRefusal {
        match r {
            Ok(set) => panic!("it fit: {set:?}"),
            Err(refusal) => refusal,
        }
    }

    fn types(l: &Ledger, prefix: &str) -> Vec<String> {
        l.recent_events(1_000)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .filter(|t| t.starts_with(prefix))
            .collect()
    }

    // ---- Money and months ----

    #[test]
    fn dollars_read_the_way_people_write_them() {
        assert_eq!(dollars(0), "$0.00");
        assert_eq!(dollars(42), "$0.0001");
        assert_eq!(dollars(4_200), "$0.0042");
        assert_eq!(dollars(10_000), "$0.01");
        assert_eq!(dollars(12_345_678), "$12.35");
        assert_eq!(dollars(50 * D), "$50.00");
        assert_eq!(dollars(1_234_567 * D), "$1,234,567.00");
        assert_eq!(dollars(u64::MAX), "$18,446,744,073,709.55");
    }

    #[test]
    fn a_month_is_the_pacific_calendar_month() {
        let utc = |y: i64, m: u32, d: u32, h: u64| {
            u64::try_from(days_from_civil(y, m, d)).unwrap() * DAY_MS + h * HOUR_MS
        };
        // 1 October, 06:59 UTC is still 30 September, 23:59 in Pacific daylight time.
        assert_eq!(pacific_month(utc(2026, 10, 1, 6) + 59 * 60_000), (2026, 9));
        assert_eq!(pacific_month(utc(2026, 10, 1, 7)), (2026, 10));
        // 1 January, 07:59 UTC is still 31 December in Pacific standard time.
        assert_eq!(pacific_month(utc(2027, 1, 1, 7) + 59 * 60_000), (2026, 12));
        assert_eq!(pacific_month(utc(2027, 1, 1, 8)), (2027, 1));
        assert_eq!(pacific_month(OCT_15), (2026, 10));
        // Midnight on the 1st, Pacific time.
        assert_eq!(month_start_ms(2026, 10), utc(2026, 10, 1, 7));
        assert_eq!(month_start_ms(2026, 11), utc(2026, 11, 1, 7));
        assert_eq!(month_start_ms(2026, 12), utc(2026, 12, 1, 8));
        assert_eq!(month_start_ms(2027, 3), utc(2027, 3, 1, 8));
        assert_eq!(month_start_ms(2027, 4), utc(2027, 4, 1, 7));
        assert_eq!(month_key(2026, 9), "2026-09");
        assert_eq!(next_month(2026, 12), (2027, 1));
    }

    #[test]
    fn daylight_saving_time_follows_the_united_states_rules() {
        let utc = |y: i64, m: u32, d: u32, h: u64| {
            u64::try_from(days_from_civil(y, m, d)).unwrap() * DAY_MS + h * HOUR_MS
        };
        // 2026: from 8 March, 2:00 PST (10:00 UTC) to 1 November, 2:00 PDT (09:00 UTC).
        assert!(!pacific_daylight(utc(2026, 3, 8, 10) - 1));
        assert!(pacific_daylight(utc(2026, 3, 8, 10)));
        assert!(pacific_daylight(utc(2026, 11, 1, 9) - 1));
        assert!(!pacific_daylight(utc(2026, 11, 1, 9)));
        // 2027: 14 March and 7 November.
        assert!(pacific_daylight(utc(2027, 3, 14, 10)));
        assert!(!pacific_daylight(utc(2027, 3, 13, 12)));
        assert!(!pacific_daylight(utc(2027, 11, 7, 9)));
        assert_eq!(nth_sunday(2026, 3, 2), days_from_civil(2026, 3, 8));
        assert_eq!(nth_sunday(2026, 11, 1), days_from_civil(2026, 11, 1));
        for days in [-1_000_000, -1, 0, 1, 20_000, 1_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    // ---- Caps ----

    #[test]
    fn a_paid_task_starts_without_any_cap_and_is_still_recorded() {
        let l = ledger();
        assert!(!l.has_business_cap().unwrap());
        assert_eq!(l.spending_room(None, None, OCT_15).unwrap(), Some(u64::MAX));
        let set = fits(l.set_aside_spending(&task(900 * D), OCT_15).unwrap());
        assert_eq!(set.most_micros, 900 * D);
        assert_eq!(types(&l, "spending."), ["spending.set_aside"]);
        let page = l.spending_page(OCT_15).unwrap();
        assert!(!page.has_business_cap && page.caps.is_empty());
        assert_eq!(page.set_aside_micros, 900 * D);
        assert_eq!(page.recent.len(), 1);
        // A cap the owner sets later still holds the work under it.
        l.set_spending_cap(&CapCovers::Business, 950 * D, "owner", OCT_15)
            .unwrap();
        assert_eq!(l.spending_room(None, None, OCT_15).unwrap(), Some(50 * D));
        refused(l.set_aside_spending(&task(60 * D), OCT_15).unwrap());
    }

    #[test]
    fn caps_are_checked_and_one_per_business_department_or_position() {
        let l = ledger();
        for bad in [0, MIN_CAP_MICROS - 1, MAX_CAP_MICROS + 1] {
            assert!(l
                .set_spending_cap(&CapCovers::Business, bad, "owner", OCT_15)
                .is_err());
        }
        let first = l
            .set_spending_cap(&CapCovers::Business, 50 * D, "owner", OCT_15)
            .unwrap();
        let again = l
            .set_spending_cap(&CapCovers::Business, 80 * D, "owner", OCT_15)
            .unwrap();
        assert_eq!(first.id, again.id, "the same cap, changed");
        assert_eq!(l.spending_caps().unwrap().len(), 1);
        assert!(matches!(
            l.set_spending_cap(
                &CapCovers::Department { id: "nope".into() },
                D,
                "owner",
                OCT_15
            ),
            Err(LedgerError::NotFound(_))
        ));
        assert!(matches!(
            l.set_spending_cap(
                &CapCovers::Position { id: "nope".into() },
                D,
                "owner",
                OCT_15
            ),
            Err(LedgerError::NotFound(_))
        ));
        let dept = l
            .create_department("Operations", "", None, "owner")
            .unwrap();
        let (pos, _) = position(&l);
        l.set_spending_cap(
            &CapCovers::Department { id: dept.id },
            20 * D,
            "owner",
            OCT_15,
        )
        .unwrap();
        l.set_spending_cap(&CapCovers::Position { id: pos }, 5 * D, "owner", OCT_15)
            .unwrap();
        let page = l.spending_page(OCT_15).unwrap();
        let labels: Vec<_> = page.caps.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["The business", "Operations", "Chief of Staff"]);
        assert_eq!(types(&l, "spending.cap_set").len(), 4);
    }

    #[test]
    fn the_business_cap_can_always_be_removed() {
        let l = ledger();
        let cap = l
            .set_spending_cap(&CapCovers::Business, 50 * D, "owner", OCT_15)
            .unwrap();
        l.remove_spending_cap(&cap.id, "owner").unwrap();
        assert!(!l.has_business_cap().unwrap());
        assert_eq!(types(&l, "spending.cap_removed").len(), 1);
    }

    // ---- Setting aside and the hard stop ----

    #[test]
    fn the_hard_stop_never_goes_over() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let a = fits(l.set_aside_spending(&task(6 * D), OCT_15).unwrap());
        // $6 set aside: a task that could cost $5 does not fit under the $4 left.
        let refusal = refused(l.set_aside_spending(&task(5 * D), OCT_15).unwrap());
        assert!(
            refusal
                .reason
                .contains("could cost up to $5.00, and $4.00 is left"),
            "{}",
            refusal.reason
        );
        assert!(refusal
            .reason
            .contains("business's spending cap ($10.00 a month)"));
        // Its bill was $3: now $7 is left.
        l.settle_spending(
            &a.record_id,
            &Bill::Spent {
                micros: 3 * D,
                priced_by: PricedBy::Service,
            },
            OCT_15,
        )
        .unwrap();
        fits(l.set_aside_spending(&task(7 * D), OCT_15).unwrap());
        let page = l.spending_page(OCT_15).unwrap();
        assert_eq!(page.spent_micros, 3 * D);
        assert_eq!(page.set_aside_micros, 7 * D);
        assert_eq!(page.caps[0].left_micros, 0);
        assert_eq!(page.caps[0].state, CapState::Stopped, "a task was refused");
        assert!(page.caps[0]
            .stopped_why
            .as_deref()
            .unwrap()
            .starts_with("Not started"));
        // Exactly nothing more fits, not even a millionth of a dollar.
        refused(l.set_aside_spending(&task(1), OCT_15).unwrap());
        // Told once this month, however many tasks it stopped.
        assert_eq!(types(&l, "spending.stopped").len(), 1);
        assert_eq!(types(&l, "spending.refused").len(), 2);
    }

    #[test]
    fn a_cap_is_enforced_for_the_business_a_department_and_one_position() {
        let l = ledger();
        let dept = l
            .create_department("Operations", "", None, "owner")
            .unwrap();
        let (pos, title) = position(&l);
        l.set_spending_cap(&CapCovers::Business, 100 * D, "owner", OCT_15)
            .unwrap();
        l.set_spending_cap(
            &CapCovers::Department {
                id: dept.id.clone(),
            },
            20 * D,
            "owner",
            OCT_15,
        )
        .unwrap();
        l.set_spending_cap(
            &CapCovers::Position { id: pos.clone() },
            5 * D,
            "owner",
            OCT_15,
        )
        .unwrap();
        let in_dept = |most: u64, with_position: bool| PaidTask {
            department: Some((dept.id.clone(), "Operations".into())),
            position: with_position.then(|| (pos.clone(), title.clone())),
            ..task(most)
        };
        // The position's $5 is the smallest amount left.
        let refusal = refused(l.set_aside_spending(&in_dept(6 * D, true), OCT_15).unwrap());
        assert!(
            refusal.reason.contains("spending cap for Chief of Staff"),
            "{}",
            refusal.reason
        );
        fits(l.set_aside_spending(&in_dept(5 * D, true), OCT_15).unwrap());
        // Another position in the department: the department's $15 left decides.
        let refusal = refused(
            l.set_aside_spending(&in_dept(16 * D, false), OCT_15)
                .unwrap(),
        );
        assert!(
            refusal.reason.contains("spending cap for Operations"),
            "{}",
            refusal.reason
        );
        fits(
            l.set_aside_spending(&in_dept(15 * D, false), OCT_15)
                .unwrap(),
        );
        // Outside the department: the business's $80 left.
        refused(l.set_aside_spending(&task(81 * D), OCT_15).unwrap());
        fits(l.set_aside_spending(&task(80 * D), OCT_15).unwrap());
        refused(l.set_aside_spending(&task(1), OCT_15).unwrap());
    }

    #[test]
    fn warned_at_80_percent_and_stopped_at_100_percent_once_a_month() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let spend = |micros: u64| {
            let set = fits(l.set_aside_spending(&task(micros), OCT_15).unwrap());
            l.settle_spending(
                &set.record_id,
                &Bill::Spent {
                    micros,
                    priced_by: PricedBy::PriceList,
                },
                OCT_15,
            )
            .unwrap()
        };
        spend(7 * D);
        assert!(types(&l, "spending.warning").is_empty());
        assert_eq!(l.spending_page(OCT_15).unwrap().caps[0].state, CapState::Ok);
        spend(D);
        assert_eq!(types(&l, "spending.warning").len(), 1);
        assert_eq!(
            l.spending_page(OCT_15).unwrap().caps[0].state,
            CapState::Warning
        );
        spend(D);
        assert_eq!(types(&l, "spending.warning").len(), 1, "once a month");
        let settled = spend(D);
        assert!(settled.passed.is_empty(), "exactly the cap, not over it");
        assert_eq!(types(&l, "spending.stopped").len(), 1);
        let page = l.spending_page(OCT_15).unwrap();
        assert_eq!(page.caps[0].state, CapState::Stopped);
        assert_eq!(
            page.caps[0].stopped_why.as_deref(),
            Some("$10.00 of $10.00 is spent this month.")
        );
        // A new month starts again.
        let nov = month_start_ms(2026, 11);
        let page = l.spending_page(nov).unwrap();
        assert_eq!(page.month, "2026-11");
        assert_eq!(page.caps[0].state, CapState::Ok);
        assert_eq!(page.caps[0].left_micros, 10 * D);
        fits(l.set_aside_spending(&task(10 * D), nov).unwrap());
    }

    #[test]
    fn a_bill_over_what_was_set_aside_says_which_cap_it_passed() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(9 * D), OCT_15).unwrap());
        let settled = l
            .settle_spending(
                &set.record_id,
                &Bill::Spent {
                    micros: 12 * D,
                    priced_by: PricedBy::Service,
                },
                OCT_15,
            )
            .unwrap();
        assert_eq!(settled.passed, ["The business"]);
        assert_eq!(types(&l, "spending.passed").len(), 1);
        assert_eq!(types(&l, "spending.stopped").len(), 1);
        let page = l.spending_page(OCT_15).unwrap();
        assert_eq!(page.caps[0].left_micros, 0);
        assert_eq!(page.spent_micros, 12 * D);
    }

    #[test]
    fn raising_a_cap_lets_stopped_work_go_on() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        refused(l.set_aside_spending(&task(11 * D), OCT_15).unwrap());
        assert_eq!(
            l.spending_page(OCT_15).unwrap().caps[0].state,
            CapState::Stopped
        );
        l.set_spending_cap(&CapCovers::Business, 20 * D, "owner", OCT_15)
            .unwrap();
        assert_eq!(l.spending_page(OCT_15).unwrap().caps[0].state, CapState::Ok);
        fits(l.set_aside_spending(&task(11 * D), OCT_15).unwrap());
    }

    #[test]
    fn not_priced_yet_is_never_counted_as_zero() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(4 * D), OCT_15).unwrap());
        l.settle_spending(
            &set.record_id,
            &Bill::NotPriced {
                detail: "The service sent no bill.".into(),
            },
            OCT_15,
        )
        .unwrap();
        let page = l.spending_page(OCT_15).unwrap();
        assert_eq!(page.spent_micros, 4 * D, "at the most it could have cost");
        assert_eq!(page.not_priced, 1);
        assert_eq!(page.recent[0].state, SpendingState::NotPriced);
        assert_eq!(page.recent[0].spent_micros, None);
        assert_eq!(
            page.recent[0].detail.as_deref(),
            Some("The service sent no bill.")
        );
        refused(l.set_aside_spending(&task(6 * D + 1), OCT_15).unwrap());
    }

    #[test]
    fn a_request_never_sent_frees_what_was_set_aside() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(10 * D), OCT_15).unwrap());
        refused(l.set_aside_spending(&task(1), OCT_15).unwrap());
        l.settle_spending(&set.record_id, &Bill::Released, OCT_15)
            .unwrap();
        fits(l.set_aside_spending(&task(10 * D), OCT_15).unwrap());
    }

    #[test]
    fn a_settled_record_never_changes_and_none_is_deleted() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(D), OCT_15).unwrap());
        l.settle_spending(&set.record_id, &Bill::Released, OCT_15)
            .unwrap();
        assert!(matches!(
            l.settle_spending(&set.record_id, &Bill::Released, OCT_15),
            Err(LedgerError::InvalidTransition { .. })
        ));
        let c = l.conn();
        assert!(c
            .execute(
                "UPDATE spending SET spent_micros = 0 WHERE id = ?1",
                [&set.record_id]
            )
            .is_err());
        assert!(c
            .execute("DELETE FROM spending WHERE id = ?1", [&set.record_id])
            .is_err());
    }

    #[test]
    fn after_a_restart_money_set_aside_counts_at_the_most_it_could_have_cost() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        fits(l.set_aside_spending(&task(9 * D), OCT_15).unwrap());
        assert_eq!(l.recover_spending(OCT_15 + 1, OCT_15 + 1).unwrap(), 1);
        assert_eq!(
            l.recover_spending(OCT_15 + 1, OCT_15 + 1).unwrap(),
            0,
            "only once"
        );
        let page = l.spending_page(OCT_15).unwrap();
        assert_eq!(page.set_aside_micros, 0);
        assert_eq!(page.spent_micros, 9 * D);
        assert_eq!(page.not_priced, 1);
        assert_eq!(page.caps[0].state, CapState::Warning);
        assert!(page.recent[0]
            .detail
            .as_deref()
            .unwrap()
            .starts_with("Plenipo stopped before"));
    }

    #[test]
    fn a_record_names_the_key_its_task_and_where_it_counts() {
        let l = ledger();
        let dept = l
            .create_department("Operations", "", None, "owner")
            .unwrap();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let t = l
            .create_task(
                crate::dto::NewTask {
                    requested_by: "owner".into(),
                    objective: "Summarize".into(),
                    ..crate::dto::NewTask::default()
                },
                "owner",
            )
            .unwrap();
        let set = fits(
            l.set_aside_spending(
                &PaidTask {
                    task_id: Some(t.id.clone()),
                    department: Some((dept.id.clone(), "Operations".into())),
                    ..task(2 * D)
                },
                OCT_15,
            )
            .unwrap(),
        );
        l.settle_spending(
            &set.record_id,
            &Bill::Spent {
                micros: 1_234_567,
                priced_by: PricedBy::Service,
            },
            OCT_15,
        )
        .unwrap();
        let records = l.spending_for_task(&t.id).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.key_name.as_deref(), Some("Office key"));
        assert_eq!(r.department_name.as_deref(), Some("Operations"));
        assert_eq!(r.spent_micros, Some(1_234_567));
        assert_eq!(r.priced_by, Some(PricedBy::Service));
        assert_eq!(r.month, "2026-10");
        // The task's own trail holds its spending.
        let trail = l.task_timeline(&t.id).unwrap();
        let kinds: Vec<_> = trail.events.iter().map(|e| e.event_type.as_str()).collect();
        assert!(kinds.contains(&"spending.set_aside"), "{kinds:?}");
        assert!(kinds.contains(&"spending.recorded"), "{kinds:?}");
    }

    // ---- Found in review (Phase 16 Wave 3, part 1) ----

    fn spent(l: &Ledger, micros: u64, now: u64) -> SetAside {
        let set = fits(l.set_aside_spending(&task(micros), now).unwrap());
        l.settle_spending(
            &set.record_id,
            &Bill::Spent {
                micros,
                priced_by: PricedBy::Service,
            },
            now,
        )
        .unwrap();
        set
    }

    #[test]
    fn a_late_bill_from_last_month_leaves_this_months_marks_alone() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        // A task set aside late on 31 October, still running at midnight.
        let late = fits(l.set_aside_spending(&task(9 * D), OCT_15).unwrap());
        let nov = month_start_ms(2026, 11) + 1;
        spent(&l, 9 * D, nov);
        refused(l.set_aside_spending(&task(2 * D), nov).unwrap());
        assert_eq!(types(&l, "spending.warning").len(), 1);
        assert_eq!(types(&l, "spending.stopped").len(), 1);
        // October's bill arrives in November: it counts in October, and changes nothing of
        // November's: no warning about "this month", and November still says it stopped.
        l.settle_spending(
            &late.record_id,
            &Bill::Spent {
                micros: 9 * D,
                priced_by: PricedBy::Service,
            },
            nov,
        )
        .unwrap();
        assert_eq!(types(&l, "spending.warning").len(), 1);
        let page = l.spending_page(nov).unwrap();
        assert_eq!(page.caps[0].state, CapState::Stopped);
        assert!(page.caps[0].stopped_why.is_some());
        refused(l.set_aside_spending(&task(2 * D), nov).unwrap());
        assert_eq!(
            types(&l, "spending.stopped").len(),
            1,
            "told once in November"
        );
        assert_eq!(l.spending_page(OCT_15).unwrap().spent_micros, 9 * D);
    }

    #[test]
    fn passed_names_only_a_bill_that_came_in_over_its_set_aside() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let a = fits(l.set_aside_spending(&task(4 * D), OCT_15).unwrap());
        let b = fits(l.set_aside_spending(&task(4 * D), OCT_15).unwrap());
        // The owner lowers the cap below what is counted.
        l.set_spending_cap(&CapCovers::Business, 5 * D, "owner", OCT_15)
            .unwrap();
        // A task that never sent its request passed nothing.
        let settled = l
            .settle_spending(&a.record_id, &Bill::Released, OCT_15)
            .unwrap();
        assert!(settled.passed.is_empty());
        // A bill within what was set aside passed nothing either.
        let settled = l
            .settle_spending(
                &b.record_id,
                &Bill::Spent {
                    micros: 3 * D,
                    priced_by: PricedBy::Service,
                },
                OCT_15,
            )
            .unwrap();
        assert!(settled.passed.is_empty());
        assert!(types(&l, "spending.passed").is_empty());
        // A bill over what was set aside, past the cap: that one.
        let c = fits(l.set_aside_spending(&task(D), OCT_15).unwrap());
        let settled = l
            .settle_spending(
                &c.record_id,
                &Bill::Spent {
                    micros: 3 * D,
                    priced_by: PricedBy::Service,
                },
                OCT_15,
            )
            .unwrap();
        assert_eq!(settled.passed, ["The business"]);
        assert_eq!(types(&l, "spending.passed").len(), 1);
    }

    #[test]
    fn a_bill_beyond_any_cap_is_recorded_at_the_most_not_refused() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(5 * D), OCT_15).unwrap());
        let settled = l
            .settle_spending(
                &set.record_id,
                &Bill::Spent {
                    micros: MAX_SET_ASIDE_MICROS + 1,
                    priced_by: PricedBy::Service,
                },
                OCT_15,
            )
            .unwrap();
        assert_eq!(settled.passed, ["The business"]);
        let r = &l.spending_page(OCT_15).unwrap().recent[0];
        assert_eq!(r.spent_micros, Some(MAX_SET_ASIDE_MICROS));
        assert!(r
            .detail
            .as_deref()
            .unwrap()
            .contains("recorded as that much"));
    }

    #[test]
    fn a_bill_is_recorded_even_when_the_caps_cannot_be_read() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        let set = fits(l.set_aside_spending(&task(2 * D), OCT_15).unwrap());
        // A later version's cap kind, read by this one (a downgrade), or a damaged setting.
        l.put_setting(
            SETTING,
            &json!({ "caps": [{ "id": "x", "covers": { "kind": "everyone" } }] }),
            "test",
        )
        .unwrap();
        l.settle_spending(
            &set.record_id,
            &Bill::Spent {
                micros: 5 * D,
                priced_by: PricedBy::Service,
            },
            OCT_15,
        )
        .unwrap();
        let r = &l.spending_for_task_or_all()[0];
        assert_eq!(r.state, SpendingState::Spent);
        assert_eq!(r.spent_micros, Some(5 * D));
        // No new paid task starts until the caps can be read again.
        assert!(l.set_aside_spending(&task(1), OCT_15).is_err());
        // After a restart, nothing is left set aside to recover, and recovery itself works.
        assert_eq!(l.recover_spending(OCT_15 + 1, OCT_15 + 1).unwrap(), 0);
    }

    impl Ledger {
        /// Every record, oldest first (tests only).
        fn spending_for_task_or_all(&self) -> Vec<SpendingRecord> {
            self.read(|c| {
                let mut stmt = c.prepare(&format!(
                    "SELECT {RECORD_COLUMNS} FROM spending ORDER BY created_at"
                ))?;
                let rows = stmt
                    .query_map([], record)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .unwrap()
        }
    }

    #[test]
    fn a_refusal_never_shows_a_task_as_cheaper_or_more_as_left() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        spent(&l, 5_995_001, OCT_15);
        // 4,004,999 micros left; the task could cost 4,000,001 + 10,000.
        let refusal = refused(l.set_aside_spending(&task(4_010_001), OCT_15).unwrap());
        assert!(
            refusal
                .reason
                .contains("could cost up to $4.02, and $4.00 is left"),
            "{}",
            refusal.reason
        );
        assert_eq!(dollars_up(4_000_001), "$4.01");
        assert_eq!(dollars_down(4_009_999), "$4.00");
        assert_eq!(dollars_down(4_200), "$0.0042");
        assert_eq!(dollars_down(42), "$0.00");
        assert_eq!(dollars_up(42), "$0.0001");
    }

    #[test]
    fn after_a_restart_a_task_this_run_started_is_left_alone() {
        let l = ledger();
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", OCT_15)
            .unwrap();
        fits(l.set_aside_spending(&task(D), OCT_15).unwrap());
        // This run started at OCT_15, when the task began: it is this run's own.
        assert_eq!(l.recover_spending(OCT_15, OCT_15 + 5).unwrap(), 0);
        assert_eq!(l.spending_page(OCT_15).unwrap().set_aside_micros, D);
        // Started after it: it belongs to the last run.
        assert_eq!(l.recover_spending(OCT_15 + 1, OCT_15 + 5).unwrap(), 1);
    }

    #[test]
    fn a_restore_keeps_the_months_spending() {
        let dir = tempfile::tempdir().unwrap();
        let now = OCT_15;
        // The Ledger as it is now: $90 spent this month.
        let current = Ledger::open(&dir.path().join("now").join("plenipo.db")).unwrap();
        current
            .set_spending_cap(&CapCovers::Business, 100 * D, "owner", now)
            .unwrap();
        spent(&current, 90 * D, now);
        let running = fits(current.set_aside_spending(&task(3 * D), now).unwrap());
        let kept = current
            .backup_of_kind(crate::backups::BackupKind::BeforeRestore, None)
            .unwrap();
        // The restored Ledger: an older backup, with the cap but none of this month's spending.
        let restored = Ledger::open(&dir.path().join("then").join("plenipo.db")).unwrap();
        restored
            .set_spending_cap(&CapCovers::Business, 100 * D, "owner", now)
            .unwrap();
        let name = std::path::Path::new(&kept.path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(restored.backups_dir().unwrap()).unwrap();
        std::fs::copy(&kept.path, restored.backups_dir().unwrap().join(&name)).unwrap();
        fits(restored.set_aside_spending(&task(20 * D), now).unwrap());
        assert_eq!(restored.carry_spending_from_backup(&name).unwrap(), 2);
        let page = restored.spending_page(now).unwrap();
        assert_eq!(page.spent_micros, 90 * D);
        assert_eq!(page.set_aside_micros, 23 * D);
        // No room was opened by restoring: $100 - $90 - $23 set aside.
        refused(restored.set_aside_spending(&task(1), now).unwrap());
        // Carried once; the task still running settles in the restored Ledger.
        assert_eq!(restored.carry_spending_from_backup(&name).unwrap(), 0);
        restored
            .settle_spending(&running.record_id, &Bill::Released, now)
            .unwrap();
        assert!(restored.carry_spending_from_backup("../escape.db").is_err());
        assert_eq!(
            restored.carry_spending_from_backup("missing.db").unwrap(),
            0
        );
        assert_eq!(types(&restored, "spending.carried_over").len(), 1);
    }
}
