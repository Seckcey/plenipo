//! A stand-in for 8 West's account service, as far as Community goes (contract
//! `contracts/community/v1`), for the tests only. Never in a release.
//!
//! It is the account service as the contract describes it, in Rust, in memory. It checks what the
//! contract says to check, in the order the contract says, and answers with the contract's own
//! error codes. It signs its stamps with the contract's published test key.
//!
//! - [`StandIn::handle`] is the whole service, one request at a time. [`StandIn`] is also a
//!   [`Transport`] (in this process), and [`StandIn::serve`] listens on `127.0.0.1` over plain
//!   HTTP, for tests that need a real connection.
//! - A test plays 8 West's side with the methods that are not requests: add an account, press
//!   **Allow**, pause or end a member, hide a part of a profile, close a report.
//! - [`Bad`] makes it a **bad service**: it changes, repeats, or makes up the items it hands
//!   out, so a test can check that a PC does not trust it.
//!
//! **Not done yet:** linked organizations (§9) and collaborators (§10). While those parts are
//! closed, their paths answer `not_open`, like the contract says. When they are open, the stand-in
//! answers `not_found` for every one of their paths, and for items of their kinds, until a later
//! part of Phase 24 teaches it more. GIFs (§14) always answer `gifs_unavailable`.
//!
//! **Limits (§15)** the stand-in keeps: 5 PCs for an account, the size of a sealed copy and of a
//! picture, the size of a request, 50 items for a pick-up, and the cards a member may see in a
//! day ([`StandIn::set_card_limit`]). The rest of §15 (the counts an hour, a minute, or a day) it
//! does not keep.
//!
//! **The clock** does not move by itself. It starts at the real time and moves only with
//! [`StandIn::set_now`]. (A pick-up that waits does wait for real seconds.)

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::time::Instant;

use crate::client::{self, Answer, Request, Transport};
use crate::ids::{self, new_id, IdKind};
use crate::wire::{
    self, AgeGroup, BusinessKind, CardStatus, HiddenPart, InboxKind, ItemKind, NoticeType,
    PointsReason, ReportOutcome, Standing,
};
use crate::{b64, item, seal, session, stamp};

/// The name of the stamping key: the contract's published test key.
pub const STAMPING_KEY_ID: &str = "test-stamp-1";
/// The contract's published test stamping key (`test-stamping-key.json`).
const STAMPING_KEY_FILE: &str =
    include_str!("../../../contracts/community/v1/test-stamping-key.json");

const DAY: i64 = 86_400;
/// How long a name is held after its member leaves (contract §3).
const HOLD_NAME_SECS: i64 = 90 * DAY;
/// The most often a name may change (contract §3).
const NAME_CHANGE_SECS: i64 = 30 * DAY;
/// How long an item nobody picks up, or a request nobody answers, is kept (contract §5, §6).
const KEEP_SECS: i64 = 30 * DAY;
/// How long a new member is on **New this week** (contract §4).
const NEW_SECS: i64 = 7 * DAY;
/// How long the same item sent again gives the same answer (contract §6).
const SAME_ITEM_SECS: i64 = DAY;
/// The same address is invited at most once in this long (contract §13).
const INVITE_SECS: i64 = 30 * DAY;
/// Most PCs signed in for one account (contract §15).
const MOST_DEVICES: usize = 5;
/// How long a sign-in code lasts, and how soon it may be asked about (contract §2).
const SIGN_IN_SECS: i64 = 600;
const SIGN_IN_INTERVAL: i64 = 5;
/// Cards on a page of the directory (contract §4), and items in a pick-up (contract §6).
const PAGE: usize = 20;
const MOST_PICK_UP: usize = 50;
/// The most a pick-up may wait (contract §6).
const MOST_WAIT: u64 = 25;
/// What a busy service tells a PC to wait (contract §1).
const BUSY_RETRY_SECS: u64 = 30;
/// The most bytes of a request, by the contract's table (§15).
const MOST_BODY_ITEMS: usize = 1536 * 1024;
const MOST_BODY_BIG: usize = 400 * 1024;
const MOST_BODY_OTHER: usize = 16 * 1024;
/// The letters a sign-in code is made of (contract §2).
const CODE_LETTERS: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ23456789";
/// A Community name may not contain these (contract §3), with the dashes taken out.
const RESERVED_WORDS: [&str; 4] = ["8west", "plenipo", "support", "admin"];

/// Which parts of Community are open (contract §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Openness {
    /// Not open at all: every request answers `not_open`.
    Closed,
    /// The people part is open, and maybe linked organizations and collaborators.
    Open { links: bool, collaborators: bool },
}

/// What a dishonest service might do to the items it hands out (they are changed when handed out,
/// never where they are kept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bad {
    /// Nothing: an honest service.
    Honest,
    /// Flip a byte of every sealed copy.
    ChangeSealed,
    /// Sign each stamp again, as if the item were for someone else.
    ChangeStamp,
    /// Hand out the last item handed out to this PC again.
    Replay,
    /// Hand out an item nobody sent.
    Invent,
}

/// An 8 West account, made by [`StandIn::add_account`]. It is a member once it joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(usize);

/// A report as 8 West got it.
#[derive(Debug, Clone)]
pub struct KeptReport {
    pub report_id: String,
    pub about: String,
    /// The member who made the report.
    pub by: String,
    pub reason: wire::ReportReason,
    pub note: Option<String>,
    pub what: wire::ReportWhat,
    /// The proofs, as they came (for `items`; only the ones not kept before).
    pub items: Vec<wire::ReportItem>,
    /// For `profile`: the card as it was.
    pub card: Option<wire::Card>,
    /// For `profile`: the picture as it was.
    pub picture: Option<Vec<u8>>,
    pub made_at: i64,
    /// How 8 West finished it, or none while it is open.
    pub closed: Option<ReportOutcome>,
}

/// A request as it arrived, for a test to look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub method: String,
    /// The path with its query, as asked.
    pub path: String,
    /// Exactly the headers that came, with lowercase names.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Seen {
    /// One header's value, by its lowercase name.
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }
}

/// An email invitation, as 8 West kept it (contract §13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    /// The address as it was typed.
    pub email: String,
    /// The member who asked.
    pub by: String,
    pub at: i64,
}

/// The contract's published test stamping key, which signs every stamp here.
fn stamping_key() -> SigningKey {
    let file: Value =
        serde_json::from_str(STAMPING_KEY_FILE).expect("the test stamping key file is JSON");
    let seed = file["seedHex"]
        .as_str()
        .expect("the test stamping key file has a seed");
    let mut bytes = [0u8; 32];
    assert_eq!(seed.len(), 64, "the seed is 32 bytes in hex");
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&seed[2 * i..2 * i + 2], 16).expect("the seed is hex");
    }
    SigningKey::from_bytes(&bytes)
}

/// The public half of the stamping key: what a test checks a stamp with.
pub fn stamping_public() -> VerifyingKey {
    stamping_key().verifying_key()
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn real_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

// ---- Versions (contract §1) -------------------------------------------------------------------

/// A Plenipo version: numbers, and maybe a suffix like `rc.1`, which counts as below the same
/// numbers without one.
#[derive(Debug, Clone)]
struct Version {
    numbers: Vec<u64>,
    suffix: Option<String>,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let (numbers, suffix) = match text.split_once('-') {
            Some((numbers, suffix)) if !suffix.is_empty() => (numbers, Some(suffix.to_owned())),
            Some(_) => return None,
            None => (text, None),
        };
        let numbers: Option<Vec<u64>> = numbers.split('.').map(|n| n.parse().ok()).collect();
        let numbers = numbers?;
        (!numbers.is_empty()).then_some(Self { numbers, suffix })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let longest = self.numbers.len().max(other.numbers.len());
        for i in 0..longest {
            let a = self.numbers.get(i).copied().unwrap_or(0);
            let b = other.numbers.get(i).copied().unwrap_or(0);
            if a != b {
                return a.cmp(&b);
            }
        }
        // The same numbers: no suffix is above any suffix.
        match (&self.suffix, &other.suffix) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ---- Dates (contract §12: a week starts Monday, Pacific time) --------------------------------

/// Days since 1970-01-01 for a civil date (Howard Hinnant's method).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The year of a day number.
fn year_of_days(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(month <= 2)
}

/// The day number of the `n`th Sunday of a month.
fn nth_sunday(year: i64, month: i64, n: i64) -> i64 {
    let first = days_from_civil(year, month, 1);
    // 1970-01-01 was a Thursday.
    let weekday = (first + 4).rem_euclid(7);
    first + (7 - weekday) % 7 + 7 * (n - 1)
}

/// How many seconds Pacific time is from UTC at `unix` (US daylight saving time rule).
fn pacific_offset(unix: i64) -> i64 {
    let year = year_of_days(unix.div_euclid(DAY));
    // Starts the second Sunday of March at 02:00 PST, ends the first Sunday of November at 02:00 PDT.
    let starts = nth_sunday(year, 3, 2) * DAY + 10 * 3_600;
    let ends = nth_sunday(year, 11, 1) * DAY + 9 * 3_600;
    if (starts..ends).contains(&unix) {
        -7 * 3_600
    } else {
        -8 * 3_600
    }
}

/// When the week `unix` is in began: Monday 00:00, Pacific time (contract §12).
fn week_start(unix: i64) -> i64 {
    let local = unix + pacific_offset(unix);
    let days = local.div_euclid(DAY);
    // 1970-01-01 was a Thursday, the fourth day counting from Monday.
    let monday = days - (days + 3).rem_euclid(7);
    let midnight = monday * DAY;
    midnight - pacific_offset(midnight + 8 * 3_600)
}

// ---- Pictures (contract §3) -------------------------------------------------------------------

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in bytes {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// A small real PNG (a grey square), for a test to upload as a picture.
pub fn tiny_png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.push(0);
        raw.extend(std::iter::repeat_n(0x80u8, width as usize));
    }
    // zlib, in stored blocks: no squeezing, but real.
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let length = block.len() as u16;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8 bits, grey, plain, not interlaced.
    header.extend_from_slice(&[8, 0, 0, 0, 0]);
    let mut png = PNG_SIGNATURE.to_vec();
    push_chunk(&mut png, b"IHDR", &header);
    push_chunk(&mut png, b"IDAT", &zlib);
    push_chunk(&mut png, b"IEND", &[]);
    png
}

enum PngProblem {
    NotAPng,
    TooBig,
}

/// Check a PNG the way the contract asks, as far as a stand-in goes: the signature, every chunk's
/// length and checksum, the header (at most 256 x 256, not interlaced), some image data, and the
/// end. It does not unpack the image data. Gives back only the image itself: no text, no other
/// extra chunks.
fn check_png(png: &[u8]) -> Result<Vec<u8>, PngProblem> {
    if png.len() > client::MOST_PICTURE {
        return Err(PngProblem::TooBig);
    }
    if !png.starts_with(PNG_SIGNATURE) {
        return Err(PngProblem::NotAPng);
    }
    let mut kept = PNG_SIGNATURE.to_vec();
    let mut at = PNG_SIGNATURE.len();
    let (mut first, mut data_seen, mut ended) = (true, false, false);
    while at < png.len() {
        if ended || png.len() - at < 12 {
            return Err(PngProblem::NotAPng);
        }
        let length = u32::from_be_bytes(png[at..at + 4].try_into().expect("four bytes")) as usize;
        if length > png.len() - at - 12 {
            return Err(PngProblem::NotAPng);
        }
        let kind: [u8; 4] = png[at + 4..at + 8].try_into().expect("four bytes");
        let data = &png[at + 8..at + 8 + length];
        let crc = u32::from_be_bytes(
            png[at + 8 + length..at + 12 + length]
                .try_into()
                .expect("four bytes"),
        );
        if crc != crc32(&png[at + 4..at + 8 + length]) {
            return Err(PngProblem::NotAPng);
        }
        if first {
            if &kind != b"IHDR" || length != 13 {
                return Err(PngProblem::NotAPng);
            }
            let width = u32::from_be_bytes(data[0..4].try_into().expect("four bytes"));
            let height = u32::from_be_bytes(data[4..8].try_into().expect("four bytes"));
            if width == 0 || height == 0 {
                return Err(PngProblem::NotAPng);
            }
            if width > 256 || height > 256 {
                return Err(PngProblem::TooBig);
            }
            if data[12] != 0 {
                // Interlaced.
                return Err(PngProblem::NotAPng);
            }
            first = false;
        }
        match &kind {
            b"IDAT" => data_seen = true,
            b"IEND" => {
                if length != 0 {
                    return Err(PngProblem::NotAPng);
                }
                ended = true;
            }
            _ => {}
        }
        if matches!(&kind, b"IHDR" | b"PLTE" | b"tRNS" | b"IDAT" | b"IEND") {
            kept.extend_from_slice(&png[at..at + 12 + length]);
        }
        at += 12 + length;
    }
    if first || !data_seen || !ended {
        return Err(PngProblem::NotAPng);
    }
    Ok(kept)
}

// ---- Words in a request -----------------------------------------------------------------------

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// One value from a query, decoded. An empty one counts as not given.
fn query_get(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| percent_decode(v))
        .filter(|v| !v.is_empty())
}

/// Characters a person cannot see that can change how words read.
fn is_hidden_mark(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// A country (`US`) or a US state (`US-CA`) (contract §3).
fn is_region(text: &str) -> bool {
    let (country, state) = match text.split_once('-') {
        Some((country, state)) => (country, Some(state)),
        None => (text, None),
    };
    country.len() == 2
        && country.bytes().all(|b| b.is_ascii_uppercase())
        && state.is_none_or(|s| {
            (1..=3).contains(&s.len())
                && s.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
}

/// A text of a profile: one line, no control characters, 1 to `most` characters.
fn profile_text_ok(text: &Option<String>, most: usize) -> bool {
    text.as_ref()
        .is_none_or(|s| (1..=most).contains(&s.chars().count()) && !s.chars().any(char::is_control))
}

fn profile_ok(profile: &wire::Profile) -> bool {
    let kinds = &profile.business_kinds;
    let no_repeats = kinds
        .iter()
        .enumerate()
        .all(|(i, kind)| !kinds[i + 1..].contains(kind));
    profile_text_ok(&profile.display_name, 60)
        && profile_text_ok(&profile.message, 80)
        && profile_text_ok(&profile.company, 80)
        && profile_text_ok(&profile.business_line, 80)
        && kinds.len() <= 3
        && no_repeats
        && profile.region.as_deref().is_none_or(is_region)
}

/// Whether `new` only hides or keeps what `old` shows (contract §3).
fn only_hides(old: &wire::Profile, new: &wire::Profile) -> bool {
    fn hides<T: PartialEq>(old: &Option<T>, new: &Option<T>) -> bool {
        new.is_none() || new == old
    }
    hides(&old.display_name, &new.display_name)
        && hides(&old.status, &new.status)
        && hides(&old.mood, &new.mood)
        && hides(&old.message, &new.message)
        && hides(&old.company, &new.company)
        && hides(&old.business_line, &new.business_line)
        && hides(&old.region, &new.region)
        && new
            .business_kinds
            .iter()
            .all(|kind| old.business_kinds.contains(kind))
}

/// Take out the parts 8 West hid.
fn drop_hidden(profile: &mut wire::Profile, hidden: &[HiddenPart]) {
    for part in hidden {
        match part {
            HiddenPart::DisplayName => profile.display_name = None,
            HiddenPart::Message => profile.message = None,
            HiddenPart::Company => profile.company = None,
            HiddenPart::BusinessLine => profile.business_line = None,
            HiddenPart::Picture | HiddenPart::Unknown => {}
        }
    }
}

fn empty_profile() -> wire::Profile {
    wire::Profile {
        display_name: None,
        status: None,
        mood: None,
        message: None,
        company: None,
        business_kinds: Vec::new(),
        business_line: None,
        region: None,
    }
}

/// An address with its capital letters, its `+tag`, and, for Gmail, its dots taken out (contract
/// §13): two spellings of one mailbox are one address.
fn mailbox(email: &str) -> Option<String> {
    let (local, domain) = email.trim().rsplit_once('@')?;
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    let mut domain = domain.to_lowercase();
    if domain == "googlemail.com" {
        domain = "gmail.com".into();
    }
    let local = local.to_lowercase();
    let local = local.split('+').next().unwrap_or("");
    let local = if domain == "gmail.com" {
        local.replace('.', "")
    } else {
        local.to_owned()
    };
    Some(format!("{local}@{domain}"))
}

// ---- Answers ----------------------------------------------------------------------------------

/// What the service answers, before it is turned into an [`Answer`] or sent over HTTP.
struct Reply {
    status: u16,
    body: Vec<u8>,
    retry_after: Option<u64>,
    content_type: &'static str,
    etag: Option<String>,
    /// An answer to a pick-up that waits, with nothing in it: the pick-up may hold on.
    empty_inbox: bool,
}

fn reply(status: u16, body: Vec<u8>) -> Reply {
    Reply {
        status,
        body,
        retry_after: None,
        content_type: "application/json",
        etag: None,
        empty_inbox: false,
    }
}

fn json<T: Serialize>(status: u16, body: &T) -> Reply {
    reply(
        status,
        serde_json::to_vec(body).expect("an answer always serializes"),
    )
}

fn no_content() -> Reply {
    reply(204, Vec::new())
}

/// An error answer: one of the contract's codes (contract §1).
struct Fail {
    code: &'static str,
    status: u16,
    message: &'static str,
    retry_after: Option<u64>,
    item: Option<u32>,
}

impl Fail {
    fn of(code: &'static str) -> Self {
        let (status, message) = match code {
            "bad_request" => (400, "That isn't what Community expects."),
            "too_large" => (400, "That is too big."),
            "unauthorized" => (401, "Sign in to Community again."),
            "not_member" => (403, "This account hasn't joined Community."),
            "email_not_confirmed" => (403, "Confirm your email first."),
            "account_limited" => (403, "8 West limited this account."),
            "too_young" => (403, "Community is for people 13 and older."),
            "adults_only" => (403, "Starting things in Community needs 18 or older."),
            "needs_pro" => (403, "This needs Plenipo Pro."),
            "community_paused" => (403, "8 West paused your Community for now."),
            "community_ended" => (403, "8 West ended your Community."),
            "terms_changed" => (
                409,
                "The Community terms changed. Read and accept them again.",
            ),
            "not_found" => (404, "That isn't here."),
            "not_delivered" => (403, "They can't get this."),
            "waiting_for_accept" => (409, "They haven't accepted your first message yet."),
            "devices_changed" => (409, "Their computers changed. Try again."),
            "not_accepting_objectives" => (403, "They aren't accepting objectives for now."),
            "item_id_reused" => (409, "That item ID was already used for something else."),
            "already_member" => (409, "This account already joined Community."),
            "name_taken" => (409, "Someone has that name."),
            "name_not_allowed" => (400, "That name isn't allowed."),
            "name_change_too_soon" => (409, "You changed your name in the last 30 days."),
            "too_many_devices" => (409, "This account already has 5 PCs signed in."),
            "waiting" => (400, "Waiting for you to press Allow."),
            "slow_down" => (400, "Asking too often. Wait a little longer."),
            "denied" => (400, "You pressed Don't allow."),
            "expired" => (400, "That code ran out."),
            "proof_failed" => (
                400,
                "One of these messages can't be proven, so the report wasn't sent.",
            ),
            "already_thanked" => (409, "You thanked them in the last 7 days."),
            "too_new_to_thank" => (403, "You can thank people after 7 days in Community."),
            "too_many" => (429, "That is too many for now. Try again later."),
            "gifs_unavailable" => (503, "GIF search isn't available."),
            "not_open" => (503, "Community isn't open yet."),
            "update_needed" => (403, "Update Plenipo to use Community."),
            "unavailable" => (503, "Community is busy right now. Nothing was changed."),
            other => panic!("`{other}` is not one of the contract's error codes"),
        };
        Self {
            code,
            status,
            message,
            retry_after: None,
            item: None,
        }
    }

    fn retry(mut self, secs: u64) -> Self {
        self.retry_after = Some(secs);
        self
    }

    fn into_reply(self) -> Reply {
        let mut answer = json(
            self.status,
            &wire::Error {
                error: self.code.to_owned(),
                message: self.message.to_owned(),
                item: self.item,
            },
        );
        answer.retry_after = self.retry_after;
        answer
    }
}

/// Read a request's body as the type the contract names for it. Any field the contract does not
/// name, or a missing one in `required`, is `bad_request`.
fn parse<T: DeserializeOwned>(body: &[u8], required: &[&str]) -> Result<T, Fail> {
    let bad = || Fail::of("bad_request");
    let value: Value = serde_json::from_slice(body).map_err(|_| bad())?;
    let object = value.as_object().ok_or_else(bad)?;
    if required.iter().any(|key| !object.contains_key(*key)) {
        return Err(bad());
    }
    serde_json::from_value(value).map_err(|_| bad())
}

// ---- What the service keeps -------------------------------------------------------------------

struct Account {
    name: String,
    email: String,
    pro: bool,
    /// The member this account joined as. Kept after leaving, so joining again gives the same one.
    member: Option<String>,
}

/// One PC signed in to an account (contract §2).
struct Device {
    id: String,
    account: AccountId,
    name: String,
    signing_key: String,
    sealing_key: String,
    added_at: i64,
    last_seen_at: i64,
}

enum SignInState {
    Waiting,
    Allowed(AccountId),
    Denied,
    /// Used up (or told it was denied): the code works once.
    Done,
}

/// A sign-in under way, by its device code.
struct Pending {
    user_code: String,
    device_name: String,
    signing_key: String,
    sealing_key: String,
    started_at: i64,
    last_ask: Option<i64>,
    /// Seconds between asks. It grows by 5 each time it is asked too soon.
    interval: i64,
    state: SignInState,
}

struct MemberRec {
    id: String,
    account: AccountId,
    name: String,
    joined_at: i64,
    birth_month: u8,
    birth_year: u16,
    terms_accepted: String,
    standing: Standing,
    paused_until: Option<i64>,
    appear_offline: bool,
    profile: wire::Profile,
    picture: Option<Vec<u8>>,
    picture_version: Option<String>,
    name_change_at: Option<i64>,
    hidden: Vec<HiddenPart>,
    /// Left Community. The record stays, for the same `member_id` on joining again.
    left: bool,
}

/// One thing waiting for one PC, and who it is between (so a block can find it).
struct Queued {
    item: wire::InboxItem,
    from: Option<String>,
    to: Option<String>,
}

/// An item sent, kept for a day so that sending it again gives the same answer (contract §6).
struct Sent {
    sender: String,
    send: wire::ItemSend,
    answer: Vec<u8>,
    at: i64,
}

/// What two members have said to each other (contract §5).
struct Conv {
    requester: String,
    other: String,
    since: i64,
    accepted: bool,
    left_requester: bool,
    left_other: bool,
    /// When the person who got the request declined it.
    declined_at: Option<i64>,
}

impl Conv {
    fn left(&self, who: &str) -> bool {
        if who == self.requester {
            self.left_requester
        } else {
            self.left_other
        }
    }

    fn set_left(&mut self, who: &str, left: bool) {
        if who == self.requester {
            self.left_requester = left;
        } else {
            self.left_other = left;
        }
    }

    fn other_than(&self, who: &str) -> &str {
        if who == self.requester {
            &self.other
        } else {
            &self.requester
        }
    }
}

struct PointChange {
    member: String,
    points: i64,
    reason: PointsReason,
    at: i64,
    /// The other person it came from, for the 20-in-30-days rule.
    from: Option<String>,
}

struct BlockRec {
    blocker: String,
    blocked: String,
    at: i64,
}

/// Who is asking: the PC the pass belongs to.
struct Ctx {
    device: String,
    account: AccountId,
}

/// What a plain first look at a message from one member to another found (contract §5).
enum Plan {
    /// A first message: a request.
    NewRequest,
    /// Talking already.
    Plain,
    /// Talking, but this member had left: writing opens their own side again.
    Reopen,
    /// A reply to a request: it accepts the request.
    Reply,
}

struct State {
    now: i64,
    open: Openness,
    lowest: Option<String>,
    busy: bool,
    terms: String,
    card_limit: u32,
    auto_allow: Option<AccountId>,
    bad: Bad,
    stamp_key: SigningKey,
    accounts: Vec<Account>,
    members: HashMap<String, MemberRec>,
    devices: Vec<Device>,
    /// A pass, and the PC it belongs to.
    passes: HashMap<String, String>,
    /// A sign-in under way, by its device code.
    pending: HashMap<String, Pending>,
    /// A name held after its member left: who held it, and until when.
    held_names: HashMap<String, (String, i64)>,
    /// What waits for each PC, oldest first.
    queues: HashMap<String, Vec<Queued>>,
    /// The last item handed out to each PC (for a service that repeats).
    handed: HashMap<String, wire::InboxItem>,
    sent: HashMap<String, Sent>,
    convs: HashMap<(String, String), Conv>,
    blocks: Vec<BlockRec>,
    reports: Vec<KeptReport>,
    points: Vec<PointChange>,
    /// A first message accepted already gave its points: (sender, who accepted).
    awarded: HashSet<(String, String)>,
    /// Cards seen: (member, day number).
    cards_seen: HashMap<(String, i64), u32>,
    invitations: Vec<(String, Invitation)>,
    /// The emails 8 West sends when a PC starts using Community: (address, PC's name).
    sign_in_emails: Vec<(String, String)>,
    seen: Vec<Seen>,
}

/// The key of a conversation: its two members, in order.
fn conv_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

fn blank_notice(kind: NoticeType) -> wire::Notice {
    wire::Notice {
        kind,
        member_id: None,
        link_id: None,
        collab_id: None,
        paused: None,
        standing: None,
        paused_until: None,
        report_id: None,
        outcome: None,
        parts: None,
    }
}

impl State {
    fn new() -> Self {
        Self {
            now: real_now(),
            open: Openness::Open {
                links: false,
                collaborators: false,
            },
            lowest: None,
            busy: false,
            terms: "2026-10-01".into(),
            card_limit: 200,
            auto_allow: None,
            bad: Bad::Honest,
            stamp_key: stamping_key(),
            accounts: Vec::new(),
            members: HashMap::new(),
            devices: Vec::new(),
            passes: HashMap::new(),
            pending: HashMap::new(),
            held_names: HashMap::new(),
            queues: HashMap::new(),
            handed: HashMap::new(),
            sent: HashMap::new(),
            convs: HashMap::new(),
            blocks: Vec::new(),
            reports: Vec::new(),
            points: Vec::new(),
            awarded: HashSet::new(),
            cards_seen: HashMap::new(),
            invitations: Vec::new(),
            sign_in_emails: Vec::new(),
            seen: Vec::new(),
        }
    }

    // -- Who is who

    fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    fn devices_of(&self, account: AccountId) -> Vec<&Device> {
        self.devices
            .iter()
            .filter(|d| d.account == account)
            .collect()
    }

    /// The member this account is now (joined, and not left).
    fn current_member(&self, account: AccountId) -> Option<String> {
        let id = self.accounts.get(account.0)?.member.as_ref()?;
        self.members
            .get(id)
            .filter(|m| !m.left)
            .map(|m| m.id.clone())
    }

    /// A member who has not left.
    fn live(&self, id: &str) -> Option<&MemberRec> {
        self.members.get(id).filter(|m| !m.left)
    }

    fn rec(&self, id: &str) -> &MemberRec {
        self.members
            .get(id)
            .expect("a member the stand-in just found is there")
    }

    fn standing_of(&self, m: &MemberRec) -> Standing {
        match m.standing {
            Standing::Paused if m.paused_until.is_some_and(|until| until <= self.now) => {
                Standing::Ok
            }
            standing => standing,
        }
    }

    fn age_group(&self, m: &MemberRec) -> AgeGroup {
        let (month, year) = session::month_and_year(self.now);
        match session::age(m.birth_month, m.birth_year, month, year) {
            Some(age) if age >= 18 => AgeGroup::Adult,
            _ => AgeGroup::Teen,
        }
    }

    fn is_pro(&self, m: &MemberRec) -> bool {
        self.accounts[m.account.0].pro
    }

    /// An adult, with Pro, in good standing (contract §3).
    fn can_start(&self, m: &MemberRec) -> bool {
        self.age_group(m) == AgeGroup::Adult
            && self.is_pro(m)
            && self.standing_of(m) == Standing::Ok
    }

    /// In the directory and on the leaderboard: an adult, in good standing, not appearing offline.
    fn listed(&self, m: &MemberRec) -> bool {
        !m.left
            && self.age_group(m) == AgeGroup::Adult
            && self.standing_of(m) == Standing::Ok
            && !m.appear_offline
    }

    fn has_picture(m: &MemberRec) -> bool {
        m.picture.is_some() && !m.hidden.contains(&HiddenPart::Picture)
    }

    fn member_wire(&self, m: &MemberRec) -> wire::Member {
        let standing = self.standing_of(m);
        wire::Member {
            member_id: m.id.clone(),
            name: m.name.clone(),
            joined_at: m.joined_at,
            age_group: self.age_group(m),
            standing,
            paused_until: if standing == Standing::Paused {
                m.paused_until
            } else {
                None
            },
            terms_accepted: m.terms_accepted.clone(),
            can_start: self.can_start(m),
            appear_offline: m.appear_offline,
            profile: m.profile.clone(),
            has_picture: Self::has_picture(m),
            picture_version: m.picture_version.clone().filter(|_| Self::has_picture(m)),
            name_change_at: m.name_change_at,
            hidden_parts: m.hidden.clone(),
        }
    }

    /// `Me` for one PC (contract §3).
    fn me_for(&self, device_id: &str) -> wire::Me {
        let device = self
            .device(device_id)
            .expect("a PC the stand-in just found is there");
        let account = &self.accounts[device.account.0];
        wire::Me {
            account: wire::Account {
                name: account.name.clone(),
            },
            device_id: device.id.clone(),
            devices: self
                .devices_of(device.account)
                .into_iter()
                .map(|d| wire::Device {
                    device_id: d.id.clone(),
                    name: d.name.clone(),
                    added_at: d.added_at,
                    last_seen_at: d.last_seen_at,
                })
                .collect(),
            terms: self.terms.clone(),
            member: self
                .current_member(device.account)
                .map(|id| self.member_wire(self.rec(&id))),
        }
    }

    /// What `viewer` sees of `m`: a card (contract §4). Adults never see a member under 18's
    /// status or mood.
    fn card_of(&self, m: &MemberRec, viewer_is_adult: bool) -> wire::Card {
        let hide_feelings = viewer_is_adult && self.age_group(m) == AgeGroup::Teen;
        let status = if hide_feelings {
            None
        } else if m.appear_offline {
            Some(CardStatus::Offline)
        } else {
            m.profile.status.map(|s| match s {
                wire::ProfileStatus::Available => CardStatus::Available,
                wire::ProfileStatus::Busy => CardStatus::Busy,
                wire::ProfileStatus::Away => CardStatus::Away,
            })
        };
        wire::Card {
            member_id: m.id.clone(),
            name: m.name.clone(),
            display_name: m.profile.display_name.clone(),
            status,
            mood: m.profile.mood.filter(|_| !hide_feelings),
            message: m.profile.message.clone(),
            company: m.profile.company.clone(),
            business_kinds: m.profile.business_kinds.clone(),
            business_line: m.profile.business_line.clone(),
            region: m.profile.region.clone(),
            has_picture: Self::has_picture(m),
            picture_version: m.picture_version.clone().filter(|_| Self::has_picture(m)),
            badges: Vec::new(),
            points: self.points_of(&m.id, None),
            thanked_by: 0,
        }
    }

    // -- Blocks and conversations

    fn blocked_by(&self, blocker: &str, blocked: &str) -> bool {
        self.blocks
            .iter()
            .any(|b| b.blocker == blocker && b.blocked == blocked)
    }

    fn blocked_either(&self, a: &str, b: &str) -> bool {
        self.blocked_by(a, b) || self.blocked_by(b, a)
    }

    fn conv(&self, a: &str, b: &str) -> Option<&Conv> {
        self.convs.get(&conv_key(a, b))
    }

    /// The two talk: one accepted the other's first message, and neither left (contract §5).
    fn talks(&self, a: &str, b: &str) -> bool {
        self.conv(a, b)
            .is_some_and(|c| c.accepted && !c.left_requester && !c.left_other)
    }

    /// May `viewer` see `target`'s card (contract §4)? Not counting a block against `viewer`.
    fn may_see(&self, viewer: &str, target: &MemberRec) -> bool {
        viewer == target.id || self.listed(target) || self.talks(viewer, &target.id)
    }

    // -- Waiting things

    fn push(&mut self, device_id: &str, queued: Queued) {
        self.queues
            .entry(device_id.to_owned())
            .or_default()
            .push(queued);
    }

    /// Tell every PC of an account something the service itself says (contract §6).
    fn tell_account(&mut self, account: AccountId, except: Option<&str>, notice: &wire::Notice) {
        let ids: Vec<String> = self
            .devices_of(account)
            .into_iter()
            .filter(|d| Some(d.id.as_str()) != except)
            .map(|d| d.id.clone())
            .collect();
        for id in ids {
            let queued = Queued {
                item: wire::InboxItem {
                    item_id: new_id(IdKind::Item),
                    kind: InboxKind::Notice,
                    from: None,
                    reference: None,
                    sealed: None,
                    tag: None,
                    stamp: None,
                    accepted_at: self.now,
                    request: false,
                    notice: Some(notice.clone()),
                },
                from: None,
                to: None,
            };
            self.push(&id, queued);
        }
    }

    fn tell_member(&mut self, member_id: &str, notice: &wire::Notice) {
        if let Some(account) = self.live(member_id).map(|m| m.account) {
            self.tell_account(account, None, notice);
        }
    }

    /// Delete the sealed items waiting between two members, both ways (contract §8).
    fn drop_waiting_between(&mut self, a: &str, b: &str) {
        for queue in self.queues.values_mut() {
            queue.retain(|q| {
                let between = match (&q.from, &q.to) {
                    (Some(from), Some(to)) => (from == a && to == b) || (from == b && to == a),
                    _ => false,
                };
                !between
            });
        }
    }

    /// Delete the sealed items waiting that one member sent to another.
    fn drop_waiting_from(&mut self, from: &str, to: &str) {
        for queue in self.queues.values_mut() {
            queue.retain(|q| !(q.from.as_deref() == Some(from) && q.to.as_deref() == Some(to)));
        }
    }

    fn remove_device(&mut self, id: &str) {
        self.devices.retain(|d| d.id != id);
        self.passes.retain(|_, device| device != id);
        self.queues.remove(id);
        self.handed.remove(id);
    }

    /// Throw away what has run out (contract §3, §5, §6).
    fn purge(&mut self) {
        let now = self.now;
        for queue in self.queues.values_mut() {
            queue.retain(|q| q.item.accepted_at + KEEP_SECS > now);
        }
        self.convs
            .retain(|_, c| c.accepted || c.declined_at.unwrap_or(c.since) + KEEP_SECS > now);
        self.sent.retain(|_, s| s.at + SAME_ITEM_SECS > now);
        self.held_names.retain(|_, (_, until)| *until > now);
    }

    // -- Points

    /// Points for the whole time, or since a moment.
    fn points_of(&self, member: &str, since: Option<i64>) -> i64 {
        self.points
            .iter()
            .filter(|p| p.member == member && since.is_none_or(|s| p.at >= s))
            .map(|p| p.points)
            .sum()
    }

    /// Give points, within the contract's limits: 100 a week, and 20 in 30 days from one other
    /// person (contract §12).
    fn add_points(&mut self, member: &str, points: i64, reason: PointsReason, from: Option<&str>) {
        let this_week = self.points_of(member, Some(week_start(self.now)));
        let mut allowed = points.min(100 - this_week);
        if let Some(from) = from {
            let from_them: i64 = self
                .points
                .iter()
                .filter(|p| {
                    p.member == member
                        && p.from.as_deref() == Some(from)
                        && p.at > self.now - 30 * DAY
                })
                .map(|p| p.points)
                .sum();
            allowed = allowed.min(20 - from_them);
        }
        if allowed > 0 {
            self.points.push(PointChange {
                member: member.to_owned(),
                points: allowed,
                reason,
                at: self.now,
                from: from.map(str::to_owned),
            });
        }
    }

    /// The members `viewer` may see on a leaderboard, best first (contract §12): listed, and not
    /// blocked either way. Everyone is in it, even with no points.
    fn board(&self, viewer: &str, since: Option<i64>) -> Vec<(&MemberRec, i64)> {
        let mut rows: Vec<(&MemberRec, i64)> = self
            .members
            .values()
            .filter(|m| self.listed(m) && (m.id == viewer || !self.blocked_either(viewer, &m.id)))
            .map(|m| (m, self.points_of(&m.id, since)))
            .collect();
        rows.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(a.0.joined_at.cmp(&b.0.joined_at))
                .then(a.0.id.cmp(&b.0.id))
        });
        rows
    }

    /// Count cards a member saw today. Past the limit, no more (contract §15).
    fn cards_used(&self, member: &str) -> u32 {
        let day = self.now.div_euclid(DAY);
        self.cards_seen
            .get(&(member.to_owned(), day))
            .copied()
            .unwrap_or(0)
    }

    fn check_cards_left(&self, member: &str) -> Result<(), Fail> {
        if self.cards_used(member) >= self.card_limit {
            let tomorrow = (self.now.div_euclid(DAY) + 1) * DAY;
            let wait = u64::try_from(tomorrow - self.now).unwrap_or(1).max(1);
            return Err(Fail::of("too_many").retry(wait));
        }
        Ok(())
    }

    fn count_cards(&mut self, member: &str, n: usize) {
        let day = self.now.div_euclid(DAY);
        *self.cards_seen.entry((member.to_owned(), day)).or_default() +=
            u32::try_from(n).unwrap_or(u32::MAX);
    }
}

// ---- The routes (contract §1 to §14) ----------------------------------------------------------

/// Every path the contract names, with its method.
enum Route<'a> {
    Open,
    SignInStart,
    SignInToken,
    SignOut,
    Me,
    Join,
    ChangeName,
    AcceptTerms,
    SetProfile,
    SetPicture,
    RemovePicture,
    SetPresence,
    Leave,
    Points,
    Directory,
    DirectoryNew,
    ByName(&'a str),
    Person(&'a str),
    PersonPicture(&'a str),
    PersonDevices(&'a str),
    Contacts,
    AcceptContact(&'a str),
    LeaveContact(&'a str),
    SendItem,
    PickUp,
    Ack,
    Blocks,
    Block(&'a str),
    Unblock(&'a str),
    Report,
    Thanks,
    Leaderboard,
    InviteEmail,
    Gifs,
    /// Any path of §9 (linked organizations).
    Links,
    /// Any path of §10 (collaborators).
    Collaborations,
}

impl<'a> Route<'a> {
    /// The route for a method and the path's parts after `/v1/community/`. None for a path the
    /// contract does not name, and for a known path with another method.
    fn find(method: &str, segments: &[&'a str]) -> Option<Self> {
        Some(match (method, segments) {
            ("GET", ["open"]) => Self::Open,
            ("POST", ["sign-in", "start"]) => Self::SignInStart,
            ("POST", ["sign-in", "token"]) => Self::SignInToken,
            ("POST", ["sign-out"]) => Self::SignOut,
            ("GET", ["me"]) => Self::Me,
            ("POST", ["join"]) => Self::Join,
            ("PUT", ["me", "name"]) => Self::ChangeName,
            ("PUT", ["me", "terms"]) => Self::AcceptTerms,
            ("PUT", ["me", "profile"]) => Self::SetProfile,
            ("PUT", ["me", "picture"]) => Self::SetPicture,
            ("DELETE", ["me", "picture"]) => Self::RemovePicture,
            ("PUT", ["me", "presence"]) => Self::SetPresence,
            ("DELETE", ["me"]) => Self::Leave,
            ("GET", ["me", "points"]) => Self::Points,
            ("GET", ["directory"]) => Self::Directory,
            ("GET", ["directory", "new"]) => Self::DirectoryNew,
            ("GET", ["people", "by-name", name]) => Self::ByName(name),
            ("GET", ["people", id]) => Self::Person(id),
            ("GET", ["people", id, "picture"]) => Self::PersonPicture(id),
            ("GET", ["people", id, "devices"]) => Self::PersonDevices(id),
            ("GET", ["contacts"]) => Self::Contacts,
            ("POST", ["contacts", id, "accept"]) => Self::AcceptContact(id),
            ("DELETE", ["contacts", id]) => Self::LeaveContact(id),
            ("POST", ["items"]) => Self::SendItem,
            ("GET", ["items"]) => Self::PickUp,
            ("POST", ["items", "ack"]) => Self::Ack,
            ("GET", ["blocks"]) => Self::Blocks,
            ("PUT", ["blocks", id]) => Self::Block(id),
            ("DELETE", ["blocks", id]) => Self::Unblock(id),
            ("POST", ["reports"]) => Self::Report,
            ("POST", ["thanks"]) => Self::Thanks,
            ("GET", ["leaderboard"]) => Self::Leaderboard,
            ("POST", ["invite-email"]) => Self::InviteEmail,
            ("GET", ["gifs"]) => Self::Gifs,
            ("POST" | "GET", ["links"])
            | ("POST", ["links", _, "accept"])
            | ("PUT", ["links", _, "paused"])
            | ("DELETE", ["links", _]) => Self::Links,
            ("POST" | "GET", ["collaborations"])
            | ("POST", ["collaborations", _, "accept"])
            | ("DELETE", ["collaborations", _]) => Self::Collaborations,
            _ => return None,
        })
    }
}

impl State {
    /// The whole service, for one request (contract §1): busy, then not open, then the version,
    /// then the routes. A request that is refused before the routes is read no further.
    fn respond(
        &mut self,
        method: &str,
        target: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<Reply, Fail> {
        if self.busy {
            return Err(Fail::of("unavailable").retry(BUSY_RETRY_SECS));
        }
        let Openness::Open {
            links,
            collaborators,
        } = self.open
        else {
            return Err(Fail::of("not_open"));
        };
        self.check_version(headers)?;
        self.purge();

        let not_found = || Fail::of("not_found");
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let rest = path.strip_prefix("/v1/community/").ok_or_else(not_found)?;
        let segments: Vec<&str> = rest.split('/').collect();
        let route = Route::find(method, &segments).ok_or_else(not_found)?;

        // The parts that open later answer `not_open` before anything else is read.
        match route {
            Route::Links if !links => return Err(Fail::of("not_open")),
            Route::Collaborations if !collaborators => return Err(Fail::of("not_open")),
            _ => {}
        }
        if matches!(route, Route::Open | Route::SignInStart | Route::SignInToken) {
            if body.len() > MOST_BODY_OTHER {
                return Err(Fail::of("too_large"));
            }
            return match route {
                Route::Open => Ok(json(
                    200,
                    &wire::Open {
                        links,
                        collaborators,
                    },
                )),
                Route::SignInStart => self.sign_in_start(body),
                _ => self.sign_in_token(body),
            };
        }

        let ctx = self.authorize(headers)?;
        let most = match route {
            Route::SendItem => MOST_BODY_ITEMS,
            Route::SetPicture | Route::Report => MOST_BODY_BIG,
            _ => MOST_BODY_OTHER,
        };
        if body.len() > most {
            return Err(Fail::of("too_large"));
        }
        self.route(route, &ctx, query, body)
    }

    fn check_version(&self, headers: &[(String, String)]) -> Result<(), Fail> {
        let Some(lowest) = self.lowest.as_deref().and_then(Version::parse) else {
            return Ok(());
        };
        let this = header(headers, "user-agent")
            .and_then(|agent| agent.strip_prefix("Plenipo/"))
            .and_then(Version::parse);
        if this.is_some_and(|version| version >= lowest) {
            Ok(())
        } else {
            Err(Fail::of("update_needed"))
        }
    }

    /// The PC a pass belongs to (contract §1).
    fn authorize(&mut self, headers: &[(String, String)]) -> Result<Ctx, Fail> {
        let unauthorized = || Fail::of("unauthorized");
        let pass = header(headers, "authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(unauthorized)?;
        let device_id = self.passes.get(pass).cloned().ok_or_else(unauthorized)?;
        let now = self.now;
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.id == device_id)
            .ok_or_else(unauthorized)?;
        device.last_seen_at = now;
        Ok(Ctx {
            device: device_id,
            account: device.account,
        })
    }

    fn route(
        &mut self,
        route: Route<'_>,
        ctx: &Ctx,
        query: &str,
        body: &[u8],
    ) -> Result<Reply, Fail> {
        match route {
            Route::Open | Route::SignInStart | Route::SignInToken => {
                unreachable!("answered before a pass is looked for")
            }
            Route::SignOut => Ok(self.sign_out(ctx)),
            Route::Me => Ok(json(200, &self.me_for(&ctx.device))),
            Route::Join => self.join(ctx, body),
            Route::ChangeName => self.change_name(ctx, body),
            Route::AcceptTerms => self.accept_terms(ctx, body),
            Route::SetProfile => self.set_profile(ctx, body),
            Route::SetPicture => self.set_picture(ctx, body),
            Route::RemovePicture => self.remove_picture(ctx),
            Route::SetPresence => self.set_presence(ctx, body),
            Route::Leave => self.leave(ctx),
            Route::Points => self.points(ctx),
            Route::Directory => self.directory(ctx, query, false),
            Route::DirectoryNew => self.directory(ctx, query, true),
            Route::ByName(name) => self.by_name(ctx, name),
            Route::Person(id) => self.person(ctx, id),
            Route::PersonPicture(id) => self.person_picture(ctx, id),
            Route::PersonDevices(id) => self.person_devices(ctx, id),
            Route::Contacts => self.contacts(ctx),
            Route::AcceptContact(id) => self.accept_contact(ctx, id),
            Route::LeaveContact(id) => self.leave_contact(ctx, id),
            Route::SendItem => self.send_item(ctx, body),
            Route::PickUp => self.pick_up(ctx, query),
            Route::Ack => self.ack(ctx, body),
            Route::Blocks => self.list_blocks(ctx),
            Route::Block(id) => self.block(ctx, id),
            Route::Unblock(id) => self.unblock(ctx, id),
            Route::Report => self.report(ctx, body),
            Route::Thanks => self.thanks(ctx, body),
            Route::Leaderboard => self.leaderboard(ctx, query),
            Route::InviteEmail => self.invite_email(ctx, body),
            Route::Gifs => {
                self.active(ctx)?;
                Err(Fail::of("gifs_unavailable"))
            }
            // Linked organizations and collaborators are open, but not in this stand-in yet.
            Route::Links | Route::Collaborations => Err(Fail::of("not_found")),
        }
    }

    // -- What a member may do

    /// The member this PC's account is: not_member until it joins, or after it leaves.
    fn joined(&self, ctx: &Ctx) -> Result<String, Fail> {
        self.current_member(ctx.account)
            .ok_or_else(|| Fail::of("not_member"))
    }

    fn standing_gate(&self, member: &str) -> Result<(), Fail> {
        match self.standing_of(self.rec(member)) {
            Standing::Ok => Ok(()),
            Standing::Paused => Err(Fail::of("community_paused")),
            Standing::Ended => Err(Fail::of("community_ended")),
        }
    }

    /// A member in good standing.
    fn active(&self, ctx: &Ctx) -> Result<String, Fail> {
        let member = self.joined(ctx)?;
        self.standing_gate(&member)?;
        Ok(member)
    }

    /// A member in good standing who accepted the current terms: what sending or starting
    /// anything needs (contract §3).
    fn full(&self, ctx: &Ctx) -> Result<String, Fail> {
        let member = self.active(ctx)?;
        if self.rec(&member).terms_accepted != self.terms {
            return Err(Fail::of("terms_changed"));
        }
        Ok(member)
    }

    // -- Signing in (contract §2)

    fn sign_in_start(&mut self, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let start: wire::SignInStart = parse(body, &[])?;
        let name_length = start.device_name.chars().count();
        let version_length = start.app_version.chars().count();
        if !(1..=60).contains(&name_length)
            || start
                .device_name
                .chars()
                .any(|c| c.is_control() || is_hidden_mark(c))
            || !(1..=32).contains(&version_length)
            || start.app_version.chars().any(char::is_control)
        {
            return Err(bad());
        }
        let signing = b64::decode_exact::<32>(&start.signing_key).ok_or_else(bad)?;
        VerifyingKey::from_bytes(&signing).map_err(|_| bad())?;
        b64::decode_exact::<32>(&start.sealing_key).ok_or_else(bad)?;

        let device_code = b64::encode(&crate::random::<32>());
        let user_code = loop {
            let mut letters = String::new();
            while letters.len() < 6 {
                let byte = crate::random::<1>()[0];
                // 224 is the biggest multiple of 28 in a byte: no letter is likelier than another.
                if byte < 224 {
                    letters.push(CODE_LETTERS[usize::from(byte) % CODE_LETTERS.len()] as char);
                }
            }
            let code = format!("{}-{}", &letters[..3], &letters[3..]);
            if !self.pending.values().any(|p| p.user_code == code) {
                break code;
            }
        };
        self.pending.insert(
            device_code.clone(),
            Pending {
                user_code: user_code.clone(),
                device_name: start.device_name,
                signing_key: start.signing_key,
                sealing_key: start.sealing_key,
                started_at: self.now,
                last_ask: None,
                interval: SIGN_IN_INTERVAL,
                state: SignInState::Waiting,
            },
        );
        if let Some(account) = self.auto_allow {
            self.allow_code(&user_code, account);
        }
        Ok(json(
            200,
            &wire::SignInStarted {
                device_code,
                user_code,
                verification_uri: session::CONNECT_PAGE.to_owned(),
                expires_in: u32::try_from(SIGN_IN_SECS).expect("600 fits"),
                interval: u32::try_from(SIGN_IN_INTERVAL).expect("5 fits"),
            },
        ))
    }

    /// The person typed this code on the account site and pressed **Allow** (contract §2). False
    /// for a code that is not waiting, and for an account that already has 5 PCs.
    fn allow_code(&mut self, user_code: &str, account: AccountId) -> bool {
        let normal = |code: &str| code.replace('-', "").to_uppercase();
        let want = normal(user_code);
        let now = self.now;
        let full = self.devices_of(account).len() >= MOST_DEVICES;
        if account.0 >= self.accounts.len() {
            return false;
        }
        let Some(pending) = self.pending.values_mut().find(|p| {
            normal(&p.user_code) == want
                && matches!(p.state, SignInState::Waiting)
                && now < p.started_at + SIGN_IN_SECS
        }) else {
            return false;
        };
        if full {
            return false;
        }
        pending.state = SignInState::Allowed(account);
        true
    }

    fn sign_in_token(&mut self, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let token: wire::SignInToken = parse(body, &[])?;
        let now = self.now;
        let pending = self.pending.get_mut(&token.device_code).ok_or_else(bad)?;
        if !session::proof_checks(&pending.signing_key, &token.device_code, &token.proof) {
            return Err(bad());
        }
        // Asked too soon: wait 5 seconds longer each time.
        if let Some(last) = pending.last_ask {
            if now - last < pending.interval {
                pending.interval += 5;
                pending.last_ask = Some(now);
                return Err(Fail::of("slow_down"));
            }
        }
        pending.last_ask = Some(now);
        if matches!(pending.state, SignInState::Done) || now >= pending.started_at + SIGN_IN_SECS {
            pending.state = SignInState::Done;
            return Err(Fail::of("expired"));
        }
        let account = match pending.state {
            SignInState::Waiting => return Err(Fail::of("waiting")),
            SignInState::Denied => {
                pending.state = SignInState::Done;
                return Err(Fail::of("denied"));
            }
            SignInState::Allowed(account) => account,
            SignInState::Done => unreachable!("answered above"),
        };
        if self.devices_of(account).len() >= MOST_DEVICES {
            return Err(Fail::of("too_many_devices"));
        }

        let pending = self
            .pending
            .get_mut(&token.device_code)
            .expect("found above");
        pending.state = SignInState::Done;
        let (name, signing_key, sealing_key) = (
            pending.device_name.clone(),
            pending.signing_key.clone(),
            pending.sealing_key.clone(),
        );
        let device_id = new_id(IdKind::Device);
        let pass = b64::encode(&crate::random::<32>());
        self.devices.push(Device {
            id: device_id.clone(),
            account,
            name: name.clone(),
            signing_key,
            sealing_key,
            added_at: now,
            last_seen_at: now,
        });
        self.passes.insert(pass.clone(), device_id.clone());
        self.sign_in_emails
            .push((self.accounts[account.0].email.clone(), name));
        if self.current_member(account).is_some() {
            self.tell_account(
                account,
                Some(&device_id),
                &blank_notice(NoticeType::MyDevicesChanged),
            );
        }
        let me = self.me_for(&device_id);
        Ok(json(
            200,
            &wire::SignedIn {
                pass,
                device_id,
                me,
            },
        ))
    }

    fn sign_out(&mut self, ctx: &Ctx) -> Reply {
        let was_member = self.current_member(ctx.account).is_some();
        self.remove_device(&ctx.device);
        if was_member {
            self.tell_account(
                ctx.account,
                None,
                &blank_notice(NoticeType::MyDevicesChanged),
            );
        }
        no_content()
    }

    // -- Membership and profile (contract §3)

    /// Whether `name` may be taken by `who` (a member changing their name) or by whoever joins.
    fn check_name(&self, name: &str, who: Option<&str>, account: AccountId) -> Result<(), Fail> {
        if !client::is_community_name(name) {
            return Err(Fail::of("name_not_allowed"));
        }
        let squeezed = name.replace('-', "");
        if RESERVED_WORDS.iter().any(|word| squeezed.contains(word)) {
            return Err(Fail::of("name_not_allowed"));
        }
        let mine = self.accounts[account.0].member.as_deref();
        let taken = self
            .members
            .values()
            .any(|m| !m.left && m.name == name && Some(m.id.as_str()) != who);
        let held = self
            .held_names
            .get(name)
            .is_some_and(|(holder, until)| *until > self.now && Some(holder.as_str()) != mine);
        if taken || held {
            return Err(Fail::of("name_taken"));
        }
        Ok(())
    }

    fn join(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let join: wire::Join = parse(body, &[])?;
        if self.current_member(ctx.account).is_some() {
            return Err(Fail::of("already_member"));
        }
        if join.terms != self.terms {
            return Err(Fail::of("terms_changed"));
        }
        let (month, year) = session::month_and_year(self.now);
        if join.birth_year < 1900 {
            return Err(bad());
        }
        let age = session::age(join.birth_month, join.birth_year, month, year).ok_or_else(bad)?;
        if age < 13 {
            return Err(Fail::of("too_young"));
        }
        self.check_name(&join.name, None, ctx.account)?;

        let id = self.accounts[ctx.account.0]
            .member
            .clone()
            .unwrap_or_else(|| new_id(IdKind::Member));
        // 8 West's own decisions about a member stay across leaving and joining again.
        let (standing, paused_until) = self
            .members
            .get(&id)
            .map_or((Standing::Ok, None), |m| (m.standing, m.paused_until));
        self.members.insert(
            id.clone(),
            MemberRec {
                id: id.clone(),
                account: ctx.account,
                name: join.name,
                joined_at: self.now,
                birth_month: join.birth_month,
                birth_year: join.birth_year,
                terms_accepted: join.terms,
                standing,
                paused_until,
                appear_offline: false,
                profile: empty_profile(),
                picture: None,
                picture_version: None,
                name_change_at: None,
                hidden: Vec::new(),
                left: false,
            },
        );
        self.accounts[ctx.account.0].member = Some(id);
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn change_name(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let id = self.full(ctx)?;
        let change: wire::NameChange = parse(body, &["name"])?;
        if self
            .rec(&id)
            .name_change_at
            .is_some_and(|earliest| self.now < earliest)
        {
            return Err(Fail::of("name_change_too_soon"));
        }
        self.check_name(&change.name, Some(&id), ctx.account)?;
        let now = self.now;
        let member = self.members.get_mut(&id).expect("found above");
        member.name = change.name;
        member.name_change_at = Some(now + NAME_CHANGE_SECS);
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn accept_terms(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let id = self.joined(ctx)?;
        let accept: wire::TermsAccept = parse(body, &["terms"])?;
        if accept.terms != self.terms {
            return Err(Fail::of("terms_changed"));
        }
        let terms = self.terms.clone();
        self.members
            .get_mut(&id)
            .expect("found above")
            .terms_accepted = terms;
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn set_profile(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let id = self.joined(ctx)?;
        // All eight fields, and no others.
        let mut profile: wire::Profile = parse(
            body,
            &[
                "display_name",
                "status",
                "mood",
                "message",
                "company",
                "business_kinds",
                "business_line",
                "region",
            ],
        )?;
        let eight = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|v| v.as_object().map(|o| o.len() == 8))
            .unwrap_or(false);
        if !eight || !profile_ok(&profile) {
            return Err(Fail::of("bad_request"));
        }
        // Hiding is always allowed; showing or changing needs full standing (contract §3).
        if !only_hides(&self.rec(&id).profile, &profile) {
            self.full(ctx)?;
        }
        let member = self.members.get_mut(&id).expect("found above");
        drop_hidden(&mut profile, &member.hidden);
        member.profile = profile;
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn set_picture(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let id = self.full(ctx)?;
        let upload: wire::PictureUpload = parse(body, &["png"])?;
        if self.rec(&id).hidden.contains(&HiddenPart::Picture) {
            return Err(Fail::of("bad_request"));
        }
        let bytes =
            b64::decode(&upload.png, MOST_BODY_BIG).ok_or_else(|| Fail::of("bad_request"))?;
        let kept = match check_png(&bytes) {
            Ok(kept) => kept,
            Err(PngProblem::NotAPng) => return Err(Fail::of("bad_request")),
            Err(PngProblem::TooBig) => return Err(Fail::of("too_large")),
        };
        let version = b64::encode(&Sha256::digest(&kept)[..16]);
        let member = self.members.get_mut(&id).expect("found above");
        member.picture = Some(kept);
        member.picture_version = Some(version);
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn remove_picture(&mut self, ctx: &Ctx) -> Result<Reply, Fail> {
        let id = self.joined(ctx)?;
        let member = self.members.get_mut(&id).expect("found above");
        member.picture = None;
        member.picture_version = None;
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    fn set_presence(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let id = self.joined(ctx)?;
        let presence: wire::Presence = parse(body, &["appear_offline"])?;
        if !presence.appear_offline {
            self.full(ctx)?;
        }
        self.members
            .get_mut(&id)
            .expect("found above")
            .appear_offline = presence.appear_offline;
        Ok(json(200, &self.me_for(&ctx.device)))
    }

    /// Leave Community (contract §3): every PC signed out, and everything of the member's
    /// deleted at once. The name is held for 90 days.
    fn leave(&mut self, ctx: &Ctx) -> Result<Reply, Fail> {
        let id = self.joined(ctx)?;
        let pcs: Vec<String> = self
            .devices_of(ctx.account)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        for pc in pcs {
            self.remove_device(&pc);
        }
        for queue in self.queues.values_mut() {
            queue.retain(|q| q.from.as_deref() != Some(&id) && q.to.as_deref() != Some(&id));
        }
        self.convs.retain(|(a, b), _| *a != id && *b != id);
        self.blocks.retain(|b| b.blocker != id);
        self.points.retain(|p| p.member != id);
        self.awarded.retain(|(sender, _)| *sender != id);
        self.sent.retain(|_, s| s.sender != id);
        let now = self.now;
        let member = self.members.get_mut(&id).expect("found above");
        self.held_names
            .insert(member.name.clone(), (id.clone(), now + HOLD_NAME_SECS));
        member.left = true;
        member.profile = empty_profile();
        member.picture = None;
        member.picture_version = None;
        member.appear_offline = false;
        member.hidden.clear();
        member.name_change_at = None;
        Ok(no_content())
    }

    // -- Finding people (contract §4)

    fn directory(&mut self, ctx: &Ctx, query: &str, new_only: bool) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let me = self.active(ctx)?;
        let (mut q, mut kind, mut region) = (String::new(), None, None);
        if !new_only {
            q = query_get(query, "q").unwrap_or_default();
            if q.chars().count() > 60 {
                return Err(bad());
            }
            if let Some(k) = query_get(query, "kind") {
                kind = Some(
                    serde_json::from_value::<BusinessKind>(Value::String(k)).map_err(|_| bad())?,
                );
            }
            match query_get(query, "region") {
                Some(r) if is_region(&r) => region = Some(r),
                Some(_) => return Err(bad()),
                None => {}
            }
        }
        let offset = match query_get(query, "cursor") {
            None => 0,
            Some(cursor) => b64::decode(&cursor, 20)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|text| text.parse::<usize>().ok())
                .ok_or_else(bad)?,
        };
        self.check_cards_left(&me)?;

        let viewer_is_adult = self.age_group(self.rec(&me)) == AgeGroup::Adult;
        let q = q.to_lowercase();
        let (cards, next) = {
            let mut found: Vec<&MemberRec> = self
                .members
                .values()
                .filter(|m| self.listed(m) && !self.blocked_by(&m.id, &me))
                .filter(|m| !new_only || m.joined_at > self.now - NEW_SECS)
                .filter(|m| {
                    q.is_empty()
                        || m.name.starts_with(&q)
                        || [
                            &m.profile.display_name,
                            &m.profile.company,
                            &m.profile.business_line,
                        ]
                        .iter()
                        .any(|text| text.as_ref().is_some_and(|t| t.to_lowercase().contains(&q)))
                })
                .filter(|m| kind.is_none_or(|k| m.profile.business_kinds.contains(&k)))
                .filter(|m| {
                    region.as_ref().is_none_or(|r| {
                        m.profile.region.as_ref().is_some_and(|mine| {
                            mine == r || (r.len() == 2 && mine.starts_with(&format!("{r}-")))
                        })
                    })
                })
                .collect();
            let points: HashMap<&str, i64> = found
                .iter()
                .map(|m| (m.id.as_str(), self.points_of(&m.id, None)))
                .collect();
            found.sort_by(|a, b| {
                points[b.id.as_str()]
                    .cmp(&points[a.id.as_str()])
                    .then(b.joined_at.cmp(&a.joined_at))
                    .then(a.id.cmp(&b.id))
            });
            let cards: Vec<wire::Card> = found
                .iter()
                .skip(offset)
                .take(PAGE)
                .map(|m| self.card_of(m, viewer_is_adult))
                .collect();
            let next = (offset + PAGE < found.len())
                .then(|| b64::encode((offset + PAGE).to_string().as_bytes()));
            (cards, next)
        };
        self.count_cards(&me, cards.len());
        Ok(json(200, &wire::CardPage { cards, next }))
    }

    fn by_name(&mut self, ctx: &Ctx, name: &str) -> Result<Reply, Fail> {
        let not_found = || Fail::of("not_found");
        let me = self.active(ctx)?;
        if !client::is_community_name(name) {
            return Err(not_found());
        }
        let target = self
            .members
            .values()
            .find(|m| !m.left && m.name == name)
            .ok_or_else(not_found)?;
        if target.id != me && self.blocked_by(&target.id, &me) {
            return Err(not_found());
        }
        self.check_cards_left(&me)?;
        let viewer_is_adult = self.age_group(self.rec(&me)) == AgeGroup::Adult;
        // Someone under 18, or appearing offline, can only be asked: unless they are talking.
        let only_ask = (self.age_group(target) == AgeGroup::Teen || target.appear_offline)
            && target.id != me
            && !self.talks(&me, &target.id);
        let answer = if only_ask {
            json(
                200,
                &wire::RequestOnly {
                    member_id: target.id.clone(),
                    name: target.name.clone(),
                    request_only: true,
                },
            )
        } else {
            json(200, &self.card_of(target, viewer_is_adult))
        };
        self.count_cards(&me, 1);
        Ok(answer)
    }

    /// The member `id` names, if `me` may know they are there (contract §4): not left, and not
    /// one who blocked `me`.
    fn findable(&self, me: &str, id: &str) -> Result<&MemberRec, Fail> {
        let not_found = || Fail::of("not_found");
        if !ids::is_id(IdKind::Member, id) {
            return Err(not_found());
        }
        let target = self.live(id).ok_or_else(not_found)?;
        if target.id != me && self.blocked_by(&target.id, me) {
            return Err(not_found());
        }
        Ok(target)
    }

    fn person(&mut self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let target = self.findable(&me, id)?;
        if !self.may_see(&me, target) {
            return Err(Fail::of("not_found"));
        }
        if target.id != me {
            self.check_cards_left(&me)?;
        }
        let viewer_is_adult = self.age_group(self.rec(&me)) == AgeGroup::Adult;
        let answer = json(200, &self.card_of(target, viewer_is_adult));
        if id != me {
            self.count_cards(&me, 1);
        }
        Ok(answer)
    }

    fn person_picture(&self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let target = self.findable(&me, id)?;
        if !self.may_see(&me, target) || !Self::has_picture(target) {
            return Err(Fail::of("not_found"));
        }
        let mut answer = reply(200, target.picture.clone().unwrap_or_default());
        answer.content_type = "image/png";
        answer.etag.clone_from(&target.picture_version);
        Ok(answer)
    }

    fn person_devices(&self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let target = self.findable(&me, id)?;
        // Anyone who is not blocked either way, and yourself.
        if target.id != me && self.blocked_by(&me, &target.id) {
            return Err(Fail::of("not_found"));
        }
        let devices = self
            .devices_of(target.account)
            .into_iter()
            .map(|d| wire::DevicePublic {
                device_id: d.id.clone(),
                signing_key: d.signing_key.clone(),
                sealing_key: d.sealing_key.clone(),
            })
            .collect();
        Ok(json(
            200,
            &wire::Devices {
                member_id: target.id.clone(),
                devices,
            },
        ))
    }

    // -- Conversations (contract §5)

    fn contacts(&self, ctx: &Ctx) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let mut contacts: Vec<wire::Contact> = Vec::new();
        for conv in self
            .convs
            .values()
            .filter(|c| c.requester == me || c.other == me)
        {
            let them = conv.other_than(&me);
            let Some(other) = self.members.get(them) else {
                continue;
            };
            let state = if conv.left(&me) {
                wire::ContactState::LeftByMe
            } else if conv.left(them) {
                wire::ContactState::LeftByThem
            } else if conv.accepted {
                wire::ContactState::Accepted
            } else if conv.requester == me {
                wire::ContactState::RequestedByMe
            } else {
                wire::ContactState::RequestedByThem
            };
            contacts.push(wire::Contact {
                member_id: other.id.clone(),
                name: other.name.clone(),
                display_name: if self.talks(&me, them) {
                    other.profile.display_name.clone()
                } else {
                    None
                },
                state,
                since: conv.since,
            });
        }
        contacts.sort_by(|a, b| a.since.cmp(&b.since).then(a.name.cmp(&b.name)));
        Ok(json(200, &wire::Contacts { contacts }))
    }

    /// Someone accepted a first message: the sender is told, and gets 2 points, once for each
    /// person (contract §5, §12).
    fn request_accepted(&mut self, requester: &str, accepter: &str) {
        let mut notice = blank_notice(NoticeType::ContactAccepted);
        notice.member_id = Some(accepter.to_owned());
        self.tell_member(requester, &notice);
        if self
            .awarded
            .insert((requester.to_owned(), accepter.to_owned()))
        {
            self.add_points(requester, 2, PointsReason::ContactAccepted, Some(accepter));
        }
    }

    fn accept_contact(&mut self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.full(ctx)?;
        let not_found = || Fail::of("not_found");
        if !ids::is_id(IdKind::Member, id) {
            return Err(not_found());
        }
        let conv = self
            .convs
            .get_mut(&conv_key(&me, id))
            .ok_or_else(not_found)?;
        // Only the person who got the request can open a conversation.
        if conv.requester == me {
            return Err(not_found());
        }
        if conv.accepted {
            return Ok(no_content());
        }
        conv.accepted = true;
        conv.set_left(&me, false);
        conv.declined_at = None;
        let requester = conv.requester.clone();
        self.request_accepted(&requester, &me);
        Ok(no_content())
    }

    fn leave_contact(&mut self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.joined(ctx)?;
        let not_found = || Fail::of("not_found");
        if !ids::is_id(IdKind::Member, id) {
            return Err(not_found());
        }
        let key = conv_key(&me, id);
        let now = self.now;
        let conv = self.convs.get_mut(&key).ok_or_else(not_found)?;
        if conv.accepted {
            conv.set_left(&me, true);
            // Both have left: the conversation is over, and writing again is a new request
            // (contract §5).
            if conv.left(id) {
                self.convs.remove(&key);
            }
        } else if conv.requester == me {
            // Take the request back, and its first message if it still waits, unless the
            // other person already declined it: then it stays declined.
            if !conv.left_other {
                self.convs.remove(&key);
                self.drop_waiting_from(&me, id);
            }
        } else {
            conv.set_left(&me, true);
            conv.declined_at = Some(now);
        }
        Ok(no_content())
    }

    /// What a message or a reaction from `from` to `to` is, by what the two have said so far
    /// (contract §5, §6).
    fn plan(&self, from: &str, to: &str, kind: ItemKind) -> Result<Plan, Fail> {
        let not_delivered = || Fail::of("not_delivered");
        let waiting = |c: &Conv| {
            if c.left(to) {
                not_delivered()
            } else {
                Fail::of("waiting_for_accept")
            }
        };
        let conv = self.conv(from, to);
        if kind == ItemKind::Reaction {
            // A reaction needs the two to talk.
            return match conv {
                None => Err(not_delivered()),
                Some(c) if c.accepted && !c.left(from) && !c.left(to) => Ok(Plan::Plain),
                Some(c) if !c.accepted && c.requester == from => Err(waiting(c)),
                Some(_) => Err(not_delivered()),
            };
        }
        match conv {
            None => {
                let sender = self.rec(from);
                if self.age_group(sender) != AgeGroup::Adult {
                    return Err(Fail::of("adults_only"));
                }
                if !self.is_pro(sender) {
                    return Err(Fail::of("needs_pro"));
                }
                Ok(Plan::NewRequest)
            }
            Some(c) if c.accepted => {
                if c.left(to) {
                    Err(not_delivered())
                } else if c.left(from) {
                    Ok(Plan::Reopen)
                } else {
                    Ok(Plan::Plain)
                }
            }
            Some(c) if c.requester == from => Err(waiting(c)),
            // A reply to a request also accepts it.
            Some(_) => Ok(Plan::Reply),
        }
    }

    // -- Sealed items (contract §6)

    fn send_item(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let me = self.full(ctx)?;
        let send: wire::ItemSend = parse(body, &["item_id", "to", "kind", "ref", "tag", "copies"])?;

        // The parts that open later.
        if let Openness::Open {
            links,
            collaborators,
        } = self.open
        {
            let closed = match send.kind {
                ItemKind::LinkNote
                | ItemKind::Objective
                | ItemKind::ObjectiveState
                | ItemKind::Answer => !links,
                ItemKind::CollabNote => !collaborators,
                ItemKind::Message | ItemKind::Reaction => false,
            };
            if closed {
                return Err(Fail::of("not_open"));
            }
        }

        // What the body must look like.
        let reference_ok = match (send.kind, &send.reference) {
            (ItemKind::Message | ItemKind::Reaction, None) => true,
            (
                ItemKind::LinkNote
                | ItemKind::Objective
                | ItemKind::ObjectiveState
                | ItemKind::Answer,
                Some(r),
            ) => ids::is_id(IdKind::Link, r),
            (ItemKind::CollabNote, Some(r)) => ids::is_id(IdKind::Collab, r),
            _ => false,
        };
        if !ids::is_id(IdKind::Item, &send.item_id)
            || !ids::is_id(IdKind::Member, &send.to)
            || !reference_ok
            || b64::decode_exact::<32>(&send.tag).is_none()
            || send.copies.is_empty()
            || send.copies.len() > item::MOST_COPIES
            || send.to == me
        {
            return Err(bad());
        }
        for copy in &send.copies {
            if !ids::is_id(IdKind::Device, &copy.device_id) {
                return Err(bad());
            }
            let sealed = b64::decode(&copy.sealed, 2 * MOST_BODY_ITEMS)
                .filter(|s| !s.is_empty())
                .ok_or_else(bad)?;
            if sealed.len() > send.kind.most_sealed_bytes() {
                return Err(Fail::of("too_large"));
            }
        }

        // The same item again, within a day, gives the same answer.
        if let Some(old) = self.sent.get(&send.item_id) {
            if old.sender == me && old.send == send {
                return Ok(reply(200, old.answer.clone()));
            }
            return Err(Fail::of("item_id_reused"));
        }

        // Can they get it? A block either way, a receiver who left, or one with no PC: no.
        let receiver_pcs: Vec<String> = self
            .live(&send.to)
            .map(|m| {
                self.devices_of(m.account)
                    .into_iter()
                    .map(|d| d.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if self.blocked_either(&me, &send.to) || receiver_pcs.is_empty() {
            return Err(Fail::of("not_delivered"));
        }
        // Linked organizations and collaborators are open, but not in this stand-in yet.
        if !matches!(send.kind, ItemKind::Message | ItemKind::Reaction) {
            return Err(Fail::of("not_found"));
        }
        // Writing again opens the writer's own side, and only that (`Plan::Reopen`): if the
        // other person left, it is not delivered and nothing changes.
        let plan = self.plan(&me, &send.to, send.kind)?;

        // The copies must name exactly the PCs of the receiver, and the sender's other PCs.
        let mut expected: HashSet<String> = receiver_pcs.into_iter().collect();
        expected.extend(
            self.devices_of(ctx.account)
                .into_iter()
                .filter(|d| d.id != ctx.device)
                .map(|d| d.id.clone()),
        );
        let given: HashSet<String> = send.copies.iter().map(|c| c.device_id.clone()).collect();
        if given != expected || given.len() != send.copies.len() {
            return Err(Fail::of("devices_changed"));
        }

        // Taken.
        let now = self.now;
        let key = conv_key(&me, &send.to);
        let request = matches!(plan, Plan::NewRequest);
        match plan {
            Plan::NewRequest => {
                self.convs.insert(
                    key,
                    Conv {
                        requester: me.clone(),
                        other: send.to.clone(),
                        since: now,
                        accepted: false,
                        left_requester: false,
                        left_other: false,
                        declined_at: None,
                    },
                );
            }
            Plan::Plain => {}
            Plan::Reopen => {
                if let Some(conv) = self.convs.get_mut(&key) {
                    conv.set_left(&me, false);
                }
            }
            Plan::Reply => {
                if let Some(conv) = self.convs.get_mut(&key) {
                    conv.accepted = true;
                    conv.set_left(&me, false);
                    conv.declined_at = None;
                }
                self.request_accepted(&send.to, &me);
            }
        }
        let stamp = stamp::make(
            &wire::StampPayload {
                v: 1,
                item_id: send.item_id.clone(),
                from: me.clone(),
                to: send.to.clone(),
                kind: send.kind,
                reference: send.reference.clone(),
                tag: send.tag.clone(),
                at: now,
                signer: STAMPING_KEY_ID.to_owned(),
            },
            &self.stamp_key,
        );
        for copy in &send.copies {
            let queued = Queued {
                item: wire::InboxItem {
                    item_id: send.item_id.clone(),
                    kind: InboxKind::Item(send.kind),
                    from: Some(me.clone()),
                    reference: send.reference.clone(),
                    sealed: Some(copy.sealed.clone()),
                    tag: Some(send.tag.clone()),
                    stamp: Some(stamp.clone()),
                    accepted_at: now,
                    request,
                    notice: None,
                },
                from: Some(me.clone()),
                to: Some(send.to.clone()),
            };
            self.push(&copy.device_id, queued);
        }
        let answer = serde_json::to_vec(&wire::ItemSent {
            item_id: send.item_id.clone(),
            accepted_at: now,
            stamp,
            request,
        })
        .expect("an answer always serializes");
        self.sent.insert(
            send.item_id.clone(),
            Sent {
                sender: me,
                send,
                answer: answer.clone(),
                at: now,
            },
        );
        Ok(reply(200, answer))
    }

    fn pick_up(&mut self, ctx: &Ctx, query: &str) -> Result<Reply, Fail> {
        self.joined(ctx)?;
        let wait = match query_get(query, "wait") {
            None => 0,
            Some(wait) => wait
                .parse::<u64>()
                .ok()
                .filter(|wait| *wait <= MOST_WAIT)
                .ok_or_else(|| Fail::of("bad_request"))?,
        };
        let queue = self.queues.get(&ctx.device);
        let mut items: Vec<wire::InboxItem> = queue
            .map(|q| {
                q.iter()
                    .take(MOST_PICK_UP)
                    .map(|q| q.item.clone())
                    .collect()
            })
            .unwrap_or_default();
        let more = queue.is_some_and(|q| q.len() > MOST_PICK_UP);
        let last = items.last().cloned();

        // A bad service changes what it hands out, never what it keeps.
        match self.bad {
            Bad::Honest => {}
            Bad::ChangeSealed => {
                for item in &mut items {
                    if let Some(sealed) = &mut item.sealed {
                        *sealed = flip_a_byte(sealed);
                    }
                }
            }
            Bad::ChangeStamp => {
                for item in &mut items {
                    if let Some(stamp) = &item.stamp {
                        item.stamp = Some(self.restamped(stamp));
                    }
                }
            }
            Bad::Replay => {
                if let Some(again) = self.handed.get(&ctx.device) {
                    items.push(again.clone());
                }
            }
            Bad::Invent => items.push(self.invented(&ctx.device)),
        }
        if let Some(last) = last {
            self.handed.insert(ctx.device.clone(), last);
        }
        let empty = items.is_empty();
        let mut answer = json(200, &wire::Inbox { items, more });
        answer.empty_inbox = empty && wait > 0;
        Ok(answer)
    }

    /// A stamp signed again as if the item were for someone else.
    fn restamped(&self, stamp: &str) -> String {
        match stamp::read(stamp) {
            Some(mut payload) => {
                payload.to = new_id(IdKind::Member);
                stamp::make(&payload, &self.stamp_key)
            }
            None => stamp.to_owned(),
        }
    }

    /// An item nobody sent, sealed for this PC as a service that knows its keys can, with a
    /// stamp that is truly signed. Its signature is not the sender's.
    fn invented(&self, device_id: &str) -> wire::InboxItem {
        let device = self
            .device(device_id)
            .expect("a PC the stand-in just found is there");
        let me = self
            .current_member(device.account)
            .unwrap_or_else(|| new_id(IdKind::Member));
        // Said to be from someone who has a PC, so the check that fails is the signature.
        let (from, from_device) = self
            .members
            .values()
            .filter(|m| !m.left && m.id != me)
            .find_map(|m| {
                self.devices_of(m.account)
                    .first()
                    .map(|d| (m.id.clone(), d.id.clone()))
            })
            .unwrap_or_else(|| (new_id(IdKind::Member), new_id(IdKind::Device)));
        let item_id = new_id(IdKind::Item);
        let payload = wire::ItemPayload {
            v: 1,
            item_id: item_id.clone(),
            kind: ItemKind::Message,
            from: from.clone(),
            from_device,
            to: me.clone(),
            reference: None,
            sent_at: self.now,
            body: wire::MessageBody {
                text: Some("Nobody sent this.".to_owned()),
                gif: None,
                sticker: None,
                reply_to: None,
            },
        };
        let text = b64::encode(&serde_json::to_vec(&payload).expect("a payload serializes"));
        let fk: [u8; 32] = crate::random();
        let tag = b64::encode(&item::report_tag(&fk, &text));
        let inside = serde_json::to_vec(&wire::SealedContent {
            payload: text,
            sig: b64::encode(&crate::random::<64>()),
            fk: b64::encode(&fk),
        })
        .expect("a sealed content serializes");
        let sealed = b64::decode_exact::<32>(&device.sealing_key)
            .and_then(|key| seal::seal(&key, &seal::info(&item_id, device_id), &inside))
            .unwrap_or_else(|| crate::random::<64>().to_vec());
        let stamp = stamp::make(
            &wire::StampPayload {
                v: 1,
                item_id: item_id.clone(),
                from: from.clone(),
                to: me,
                kind: ItemKind::Message,
                reference: None,
                tag: tag.clone(),
                at: self.now,
                signer: STAMPING_KEY_ID.to_owned(),
            },
            &self.stamp_key,
        );
        wire::InboxItem {
            item_id,
            kind: InboxKind::Item(ItemKind::Message),
            from: Some(from),
            reference: None,
            sealed: Some(b64::encode(&sealed)),
            tag: Some(tag),
            stamp: Some(stamp),
            accepted_at: self.now,
            request: false,
            notice: None,
        }
    }

    fn ack(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        self.joined(ctx)?;
        let ack: wire::Ack = parse(body, &["item_ids"])?;
        let done: HashSet<&str> = ack.item_ids.iter().map(String::as_str).collect();
        if ack.item_ids.is_empty()
            || ack.item_ids.len() > 100
            || done.len() != ack.item_ids.len()
            || ack.item_ids.iter().any(|id| !ids::is_id(IdKind::Item, id))
        {
            return Err(bad());
        }
        if let Some(queue) = self.queues.get_mut(&ctx.device) {
            queue.retain(|q| !done.contains(q.item.item_id.as_str()));
        }
        Ok(no_content())
    }

    // -- Blocks (contract §8)

    fn list_blocks(&self, ctx: &Ctx) -> Result<Reply, Fail> {
        let me = self.joined(ctx)?;
        let mut blocks: Vec<wire::Block> = self
            .blocks
            .iter()
            .filter(|b| b.blocker == me)
            .filter_map(|b| {
                self.members.get(&b.blocked).map(|m| wire::Block {
                    member_id: m.id.clone(),
                    name: m.name.clone(),
                    blocked_at: b.at,
                })
            })
            .collect();
        blocks.sort_by(|a, b| a.blocked_at.cmp(&b.blocked_at).then(a.name.cmp(&b.name)));
        Ok(json(200, &wire::Blocks { blocks }))
    }

    fn block(&mut self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.joined(ctx)?;
        if !ids::is_id(IdKind::Member, id) || !self.members.contains_key(id) {
            return Err(Fail::of("not_found"));
        }
        if id == me {
            return Err(Fail::of("bad_request"));
        }
        if self.blocked_by(&me, id) {
            return Ok(no_content());
        }
        self.blocks.push(BlockRec {
            blocker: me.clone(),
            blocked: id.to_owned(),
            at: self.now,
        });
        // Waiting items go both ways, and the conversation ends. They are not told.
        self.drop_waiting_between(&me, id);
        self.convs.remove(&conv_key(&me, id));
        Ok(no_content())
    }

    fn unblock(&mut self, ctx: &Ctx, id: &str) -> Result<Reply, Fail> {
        let me = self.joined(ctx)?;
        self.blocks
            .retain(|b| !(b.blocker == me && b.blocked == id));
        Ok(no_content())
    }

    // -- Reports (contract §11)

    /// Whether one reported item is real: 8 West's stamp, and the words that go with it
    /// (contract §11). Gives the item's ID.
    fn prove(&self, proof: &wire::ReportItem, about: &str, reporter: &str) -> Option<String> {
        let stamped = stamp::check(&proof.stamp, &self.stamp_key.verifying_key())?;
        if stamped.signer != STAMPING_KEY_ID || stamped.from != about || stamped.to != reporter {
            return None;
        }
        let fk = b64::decode_exact::<32>(&proof.fk)?;
        if b64::encode(&item::report_tag(&fk, &proof.payload)) != stamped.tag {
            return None;
        }
        let bytes = b64::decode(&proof.payload, MOST_BODY_BIG)?;
        let payload: wire::ItemPayload = serde_json::from_slice(&bytes).ok()?;
        let same = payload.v == 1
            && payload.item_id == stamped.item_id
            && payload.kind == stamped.kind
            && payload.from == stamped.from
            && payload.to == stamped.to
            && payload.reference == stamped.reference;
        same.then_some(stamped.item_id)
    }

    fn report(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let me = self.active(ctx)?;
        let report: wire::Report = parse(body, &["about", "reason", "note", "what", "items"])?;
        let items_ok = match report.what {
            wire::ReportWhat::Items => (1..=20).contains(&report.items.len()),
            wire::ReportWhat::Person | wire::ReportWhat::Profile => report.items.is_empty(),
        };
        if !ids::is_id(IdKind::Member, &report.about)
            || report.about == me
            || !items_ok
            || report
                .note
                .as_ref()
                .is_some_and(|n| !(1..=1000).contains(&n.chars().count()))
        {
            return Err(bad());
        }

        let open = |r: &&KeptReport| {
            r.by == me && r.about == report.about && r.reason == report.reason && r.closed.is_none()
        };
        let mut kept_items = Vec::new();
        let (mut card, mut picture) = (None, None);
        let report_id = match report.what {
            wire::ReportWhat::Items => {
                let mut item_ids = Vec::new();
                for (i, proof) in report.items.iter().enumerate() {
                    let id = self.prove(proof, &report.about, &me).ok_or_else(|| {
                        let mut fail = Fail::of("proof_failed");
                        fail.item = u32::try_from(i).ok();
                        fail
                    })?;
                    item_ids.push(id);
                }
                // An item already reported for this reason, in a report still open, is not kept
                // again. If every item was, the answer is that earlier report.
                let already = |id: &String| {
                    self.reports.iter().filter(open).find(|r| {
                        r.items
                            .iter()
                            .any(|kept| stamp::read(&kept.stamp).is_some_and(|s| s.item_id == *id))
                    })
                };
                if item_ids.iter().all(|id| already(id).is_some()) {
                    let earlier = already(&item_ids[0]).expect("checked just above");
                    return Ok(json(
                        200,
                        &wire::ReportMade {
                            report_id: earlier.report_id.clone(),
                        },
                    ));
                }
                for (proof, id) in report.items.iter().zip(&item_ids) {
                    if already(id).is_none() {
                        kept_items.push(proof.clone());
                    }
                }
                new_id(IdKind::Report)
            }
            wire::ReportWhat::Person | wire::ReportWhat::Profile => {
                // Someone the reporter could look up, or blocked.
                let target = self.members.get(&report.about).filter(|m| !m.left);
                let known = target.filter(|t| {
                    self.blocked_by(&me, &t.id)
                        || (!self.blocked_by(&t.id, &me) && self.may_see(&me, t))
                });
                let Some(target) = known else {
                    return Err(Fail::of("not_found"));
                };
                if let Some(earlier) = self
                    .reports
                    .iter()
                    .filter(open)
                    .find(|r| r.what == report.what)
                {
                    return Ok(json(
                        200,
                        &wire::ReportMade {
                            report_id: earlier.report_id.clone(),
                        },
                    ));
                }
                if report.what == wire::ReportWhat::Profile {
                    let viewer_is_adult = self.age_group(self.rec(&me)) == AgeGroup::Adult;
                    card = Some(self.card_of(target, viewer_is_adult));
                    picture = target.picture.clone().filter(|_| Self::has_picture(target));
                }
                new_id(IdKind::Report)
            }
        };
        self.reports.push(KeptReport {
            report_id: report_id.clone(),
            about: report.about,
            by: me,
            reason: report.reason,
            note: report.note,
            what: report.what,
            items: kept_items,
            card,
            picture,
            made_at: self.now,
            closed: None,
        });
        Ok(json(200, &wire::ReportMade { report_id }))
    }

    // -- Thanks, points, and the leaderboard (contract §12)

    fn thanks(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        self.full(ctx)?;
        let thanks: wire::Thanks = parse(body, &["to", "for", "ref"])?;
        let (kind, closed) = match (thanks.for_what, self.open) {
            (wire::ThanksFor::LinkAnswer, Openness::Open { links, .. }) => (IdKind::Link, !links),
            (wire::ThanksFor::Collaborator, Openness::Open { collaborators, .. }) => {
                (IdKind::Collab, !collaborators)
            }
            (_, Openness::Closed) => unreachable!("a closed service answers before this"),
        };
        if !ids::is_id(IdKind::Member, &thanks.to) || !ids::is_id(kind, &thanks.reference) {
            return Err(Fail::of("bad_request"));
        }
        if closed {
            return Err(Fail::of("not_open"));
        }
        // Linked organizations and collaborators are open, but not in this stand-in yet.
        Err(Fail::of("not_found"))
    }

    fn points(&self, ctx: &Ctx) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let week = week_start(self.now);
        let place = |since| {
            self.board(&me, since)
                .iter()
                .position(|(m, _)| m.id == me)
                .map(|i| u32::try_from(i + 1).unwrap_or(u32::MAX))
        };
        let recent = self
            .points
            .iter()
            .rev()
            .filter(|p| p.member == me)
            .take(20)
            .map(|p| wire::PointsChange {
                points: p.points,
                reason: p.reason,
                at: p.at,
            })
            .collect();
        let adult = self.age_group(self.rec(&me)) == AgeGroup::Adult;
        Ok(json(
            200,
            &wire::Points {
                total: self.points_of(&me, None),
                week: self.points_of(&me, Some(week)),
                week_started_at: week,
                place_week: place(Some(week)),
                place_all: place(None),
                badges: Vec::new(),
                thanked_by: 0,
                recent,
                free_months: adult.then_some(wire::FreeMonths {
                    this_year: 0,
                    max: 12,
                }),
            },
        ))
    }

    fn leaderboard(&self, ctx: &Ctx, query: &str) -> Result<Reply, Fail> {
        let me = self.active(ctx)?;
        let (period, since) = match query_get(query, "period").as_deref() {
            Some("week") => (wire::LeaderboardPeriod::Week, Some(week_start(self.now))),
            Some("all") => (wire::LeaderboardPeriod::All, None),
            _ => return Err(Fail::of("bad_request")),
        };
        let board = self.board(&me, since);
        let top = board
            .iter()
            .take(50)
            .enumerate()
            .map(|(i, (m, points))| wire::LeaderboardRow {
                place: u32::try_from(i + 1).unwrap_or(u32::MAX),
                member_id: m.id.clone(),
                name: m.name.clone(),
                display_name: m.profile.display_name.clone(),
                has_picture: Self::has_picture(m),
                picture_version: m.picture_version.clone().filter(|_| Self::has_picture(m)),
                badges: Vec::new(),
                points: *points,
            })
            .collect();
        let place = board
            .iter()
            .position(|(m, _)| m.id == me)
            .map(|i| u32::try_from(i + 1).unwrap_or(u32::MAX));
        Ok(json(
            200,
            &wire::Leaderboard {
                period,
                since,
                top,
                me: Some(wire::LeaderboardMe {
                    place,
                    points: self.points_of(&me, since),
                }),
            },
        ))
    }

    // -- Inviting by email (contract §13)

    fn invite_email(&mut self, ctx: &Ctx, body: &[u8]) -> Result<Reply, Fail> {
        let bad = || Fail::of("bad_request");
        let me = self.full(ctx)?;
        let invite: wire::InviteEmail = parse(body, &["email"])?;
        if !(3..=254).contains(&invite.email.chars().count()) {
            return Err(bad());
        }
        let address = mailbox(&invite.email).ok_or_else(bad)?;
        let sender = self.rec(&me);
        if self.age_group(sender) != AgeGroup::Adult {
            return Err(Fail::of("adults_only"));
        }
        if !self.is_pro(sender) {
            return Err(Fail::of("needs_pro"));
        }
        // One invitation for each address in 30 days, from anyone, and nobody is told.
        let recent = self
            .invitations
            .iter()
            .any(|(key, i)| *key == address && i.at + INVITE_SECS > self.now);
        if !recent {
            self.invitations.push((
                address,
                Invitation {
                    email: invite.email,
                    by: me,
                    at: self.now,
                },
            ));
        }
        Ok(json(202, &wire::Empty {}))
    }
}

/// A sealed copy with its last byte changed.
fn flip_a_byte(sealed: &str) -> String {
    match b64::decode(sealed, 2 * MOST_BODY_ITEMS) {
        Some(mut bytes) if !bytes.is_empty() => {
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
            b64::encode(&bytes)
        }
        _ => sealed.to_owned(),
    }
}

// ---- 8 West's side, for a test to play ---------------------------------------------------------

impl State {
    fn pause_member(&mut self, member_id: &str, until: i64) {
        let Some(member) = self.members.get_mut(member_id).filter(|m| !m.left) else {
            return;
        };
        member.standing = Standing::Paused;
        member.paused_until = Some(until);
        let mut notice = blank_notice(NoticeType::StandingChanged);
        notice.standing = Some(wire::NoticeStanding::Paused);
        notice.paused_until = Some(until);
        self.tell_member(member_id, &notice);
    }

    fn end_member(&mut self, member_id: &str) {
        let Some(member) = self.members.get_mut(member_id).filter(|m| !m.left) else {
            return;
        };
        member.standing = Standing::Ended;
        member.paused_until = None;
        let mut notice = blank_notice(NoticeType::StandingChanged);
        notice.standing = Some(wire::NoticeStanding::Ended);
        self.tell_member(member_id, &notice);
    }

    fn hide_parts(&mut self, member_id: &str, parts: &[HiddenPart]) {
        let Some(member) = self.members.get_mut(member_id).filter(|m| !m.left) else {
            return;
        };
        let mut hidden: Vec<HiddenPart> = Vec::new();
        for part in parts {
            if !hidden.contains(part) {
                hidden.push(*part);
            }
        }
        drop_hidden(&mut member.profile, &hidden);
        member.hidden.clone_from(&hidden);
        let mut notice = blank_notice(NoticeType::ProfileHidden);
        notice.parts = Some(hidden);
        self.tell_member(member_id, &notice);
    }

    fn close_report(&mut self, report_id: &str, action: bool) {
        let outcome = if action {
            ReportOutcome::Action
        } else {
            ReportOutcome::NoAction
        };
        let Some(report) = self.reports.iter_mut().find(|r| r.report_id == report_id) else {
            return;
        };
        report.closed = Some(outcome);
        let reporter = report.by.clone();
        let mut notice = blank_notice(NoticeType::ReportClosed);
        notice.report_id = Some(report_id.to_owned());
        notice.outcome = Some(outcome);
        self.tell_member(&reporter, &notice);
    }
}

// ---- The stand-in service ---------------------------------------------------------------------

struct Inner {
    state: Mutex<State>,
    /// Told whenever a request changes something, so a pick-up that waits looks again.
    notify: Notify,
}

/// 8 West's account service, as far as Community goes, in memory. Cheap to clone: every copy is
/// the same service.
#[derive(Clone)]
pub struct StandIn {
    inner: Arc<Inner>,
}

impl Default for StandIn {
    fn default() -> Self {
        Self::new()
    }
}

impl StandIn {
    /// A service that is open for the people part, with linked organizations and collaborators
    /// closed, and no lowest version.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::new()),
                notify: Notify::new(),
            }),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut lock(&self.inner.state))
    }

    /// Wake the pick-ups that are waiting.
    fn wake(&self) {
        self.inner.notify.notify_waiters();
    }

    /// Open or close Community, and its parts (contract §1).
    pub fn set_open(&self, open: Openness) {
        self.with(|s| s.open = open);
    }

    /// The lowest version of Plenipo allowed in Community, or none (contract §1).
    ///
    /// # Panics
    /// If `version` is not numbers with dots and maybe a `-suffix`, like `1.20.0` or `2.0.0-rc.1`.
    pub fn set_lowest_version(&self, version: Option<&str>) {
        if let Some(version) = version {
            assert!(
                Version::parse(version).is_some(),
                "`{version}` is not a version like 1.20.0"
            );
        }
        self.with(|s| s.lowest = version.map(str::to_owned));
    }

    /// While busy, every request answers `unavailable`, with `Retry-After: 30`.
    pub fn set_busy(&self, busy: bool) {
        self.with(|s| s.busy = busy);
    }

    /// Set the clock, in Unix seconds. It starts at the real time and never moves by itself.
    pub fn set_now(&self, unix: i64) {
        self.with(|s| s.now = unix);
    }

    /// 8 West changes the Community terms: a member must accept them again (contract §3).
    pub fn set_terms(&self, version: &str) {
        self.with(|s| s.terms = version.to_owned());
    }

    /// The most cards a member may see in a day (contract §15). 200 at first.
    pub fn set_card_limit(&self, n: u32) {
        self.with(|s| s.card_limit = n);
    }

    /// Make the service honest, or dishonest in one way (see [`Bad`]).
    pub fn set_bad(&self, bad: Bad) {
        self.with(|s| s.bad = bad);
    }

    /// An 8 West account. It is not a member until a PC of it joins.
    pub fn add_account(&self, name: &str, email: &str, pro: bool) -> AccountId {
        self.with(|s| {
            s.accounts.push(Account {
                name: name.to_owned(),
                email: email.to_owned(),
                pro,
                member: None,
            });
            AccountId(s.accounts.len() - 1)
        })
    }

    /// The account gets Plenipo Pro, or loses it.
    pub fn set_pro(&self, account: AccountId, pro: bool) {
        self.with(|s| {
            if let Some(account) = s.accounts.get_mut(account.0) {
                account.pro = pro;
            }
        });
    }

    /// The person typed the code on the account site, signed in as `account`, and pressed
    /// **Allow**. False if no sign-in is waiting for that code, or if the account already has 5 PCs.
    pub fn allow(&self, user_code: &str, account: AccountId) -> bool {
        self.with(|s| s.allow_code(user_code, account))
    }

    /// The person typed the code and pressed **Don't allow**. False if no sign-in is waiting.
    pub fn deny(&self, user_code: &str) -> bool {
        self.with(|s| {
            let want = user_code.replace('-', "").to_uppercase();
            let now = s.now;
            let Some(pending) = s.pending.values_mut().find(|p| {
                p.user_code.replace('-', "") == want
                    && matches!(p.state, SignInState::Waiting)
                    && now < p.started_at + SIGN_IN_SECS
            }) else {
                return false;
            };
            pending.state = SignInState::Denied;
            true
        })
    }

    /// Every new sign-in is allowed at once, for this account (or for none: back to normal).
    /// A sign-in for an account that already has 5 PCs still waits.
    pub fn auto_allow(&self, account: Option<AccountId>) {
        self.with(|s| s.auto_allow = account);
    }

    /// 8 West pauses a member's Community until a time. The member's PCs get `standing_changed`.
    pub fn pause(&self, member_id: &str, until: i64) {
        self.with(|s| s.pause_member(member_id, until));
        self.wake();
    }

    /// 8 West ends a member's Community. The member's PCs get `standing_changed`.
    pub fn end(&self, member_id: &str) {
        self.with(|s| s.end_member(member_id));
        self.wake();
    }

    /// 8 West hides these parts of a member's profile, and no others (an empty list shows them
    /// again). The member's PCs get `profile_hidden`.
    pub fn hide_parts(&self, member_id: &str, parts: &[HiddenPart]) {
        self.with(|s| s.hide_parts(member_id, parts));
        self.wake();
    }

    /// 8 West finishes a report. The member who made it gets `report_closed`.
    pub fn close_report(&self, report_id: &str, action: bool) {
        self.with(|s| s.close_report(report_id, action));
        self.wake();
    }

    /// 8 West gives a member points (or takes them away, with a negative number), outside the
    /// limits that points have (contract §12).
    pub fn give_points(&self, member_id: &str, points: i64, reason: PointsReason) {
        self.with(|s| {
            let at = s.now;
            s.points.push(PointChange {
                member: member_id.to_owned(),
                points,
                reason,
                at,
                from: None,
            });
        });
    }

    /// What 8 West received as reports.
    pub fn reports(&self) -> Vec<KeptReport> {
        self.with(|s| s.reports.clone())
    }

    /// Every request, as it arrived, oldest first.
    pub fn seen(&self) -> Vec<Seen> {
        self.with(|s| s.seen.clone())
    }

    /// The invitations 8 West would email (contract §13), one for each address in 30 days.
    pub fn invitations(&self) -> Vec<Invitation> {
        self.with(|s| {
            s.invitations
                .iter()
                .filter(|(_, i)| i.at + INVITE_SECS > s.now)
                .map(|(_, i)| i.clone())
                .collect()
        })
    }

    /// The emails 8 West sends the account when a PC starts using Community as it (contract §2):
    /// the account's address, and the PC's name.
    pub fn sign_in_emails(&self) -> Vec<(String, String)> {
        self.with(|s| s.sign_in_emails.clone())
    }

    fn record(&self, method: &str, target: &str, headers: &[(String, String)], body: &[u8]) {
        self.with(|s| {
            s.seen.push(Seen {
                method: method.to_owned(),
                path: target.to_owned(),
                headers: headers
                    .iter()
                    .map(|(n, v)| (n.to_ascii_lowercase(), v.clone()))
                    .collect(),
                body: body.to_vec(),
            });
        });
    }

    fn respond(
        &self,
        method: &str,
        target: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Reply {
        self.with(|s| s.respond(method, target, headers, body))
            .unwrap_or_else(Fail::into_reply)
    }

    /// The whole service for one request, without waiting. A pick-up with `wait` answers at once
    /// with what is there (see [`StandIn::serve`] and the [`Transport`] for the waiting one).
    /// `path_and_query` starts with `/v1/community/`. Header names may be in any case.
    pub fn handle(
        &self,
        method: &str,
        path_and_query: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Answer {
        self.record(method, path_and_query, headers, body);
        let reply = self.respond(method, path_and_query, headers, body);
        if method != "GET" {
            self.wake();
        }
        Answer {
            status: reply.status,
            body: reply.body,
            retry_after: reply.retry_after,
        }
    }

    /// Like [`StandIn::handle`], but a pick-up with `wait` and nothing waiting holds on, up to
    /// that many seconds, until something arrives (contract §6).
    async fn handle_waiting(
        &self,
        method: &str,
        target: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Reply {
        self.record(method, target, headers, body);
        let hold = held_for(method, target);
        let until = hold.map(|secs| Instant::now() + Duration::from_secs(secs));
        let answer = loop {
            // Ask to be told before looking, so nothing that arrives in between is missed.
            let mut told = pin!(self.inner.notify.notified());
            told.as_mut().enable();
            let answer = self.respond(method, target, headers, body);
            let Some(until) = until.filter(|_| answer.empty_inbox) else {
                break answer;
            };
            if tokio::time::timeout_at(until, told).await.is_err() {
                break answer;
            }
        };
        if method != "GET" {
            self.wake();
        }
        answer
    }

    /// Listen on `127.0.0.1`, on any free port, over plain HTTP/1.1, and answer as the service
    /// does: keep-alive connections, bodies with `Content-Length`, `Content-Type:
    /// application/json` (`image/png` for a picture), `Retry-After` when there is one, and pick-ups
    /// that wait. Gives back the address, like `http://127.0.0.1:41234`. It listens until the
    /// test's runtime ends. Each connection has its own task.
    pub async fn serve(&self) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the stand-in service can listen on this PC");
        let port = listener
            .local_addr()
            .expect("a listening socket has an address")
            .port();
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                };
                let service = service.clone();
                tokio::spawn(async move { service.serve_connection(stream).await });
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    async fn serve_connection(self, mut stream: TcpStream) {
        let mut buffer: Vec<u8> = Vec::new();
        loop {
            // The head: the request line and the headers, up to a blank line.
            let head_end = loop {
                if let Some(at) = find(&buffer, b"\r\n\r\n") {
                    break at;
                }
                if buffer.len() > MOST_HEAD || !read_more(&mut stream, &mut buffer).await {
                    return;
                }
            };
            let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
            let mut lines = head.split("\r\n");
            let mut request_line = lines.next().unwrap_or("").split(' ');
            let (Some(method), Some(target), Some(version)) = (
                request_line.next(),
                request_line.next(),
                request_line.next(),
            ) else {
                return;
            };
            let headers: Vec<(String, String)> = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
                .collect();
            // Plenipo never sends a chunked body; this stand-in does not read one.
            if header(&headers, "transfer-encoding").is_some() {
                return;
            }
            let length = match header(&headers, "content-length") {
                None => 0,
                Some(length) => match length.parse::<usize>() {
                    Ok(length) if length <= MOST_BODY_HTTP => length,
                    _ => return,
                },
            };
            let body_start = head_end + 4;
            while buffer.len() < body_start + length {
                if !read_more(&mut stream, &mut buffer).await {
                    return;
                }
            }
            let body = buffer[body_start..body_start + length].to_vec();
            buffer.drain(..body_start + length);
            let closing = version == "HTTP/1.0"
                || header(&headers, "connection").is_some_and(|c| c.eq_ignore_ascii_case("close"));

            let answer = self.handle_waiting(method, target, &headers, &body).await;
            if stream.write_all(&http_bytes(&answer)).await.is_err() || closing {
                return;
            }
        }
    }
}

/// The most bytes of a request's head, and of a request's body, that the stand-in reads over HTTP.
const MOST_HEAD: usize = 32 * 1024;
const MOST_BODY_HTTP: usize = 4 * 1024 * 1024;

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read what the connection has next. False when it is closed or broken.
async fn read_more(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> bool {
    let mut chunk = [0u8; 8192];
    match stream.read(&mut chunk).await {
        Ok(0) | Err(_) => false,
        Ok(n) => {
            buffer.extend_from_slice(&chunk[..n]);
            true
        }
    }
}

/// How long a pick-up asks to be held, if it is one: `GET /v1/community/items?wait=1` to `25`.
fn held_for(method: &str, target: &str) -> Option<u64> {
    if method != "GET" {
        return None;
    }
    let (path, query) = target.split_once('?')?;
    if path != "/v1/community/items" {
        return None;
    }
    let wait: u64 = query_get(query, "wait")?.parse().ok()?;
    (1..=MOST_WAIT).contains(&wait).then_some(wait)
}

/// An answer as HTTP/1.1 writes it.
fn http_bytes(answer: &Reply) -> Vec<u8> {
    let words = match answer.status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Unknown",
    };
    let mut head = format!("HTTP/1.1 {} {words}\r\n", answer.status);
    // A 204 has no body, and says nothing about one.
    if answer.status != 204 {
        head.push_str(&format!(
            "Content-Type: {}\r\nContent-Length: {}\r\n",
            answer.content_type,
            answer.body.len()
        ));
    }
    if let Some(secs) = answer.retry_after {
        head.push_str(&format!("Retry-After: {secs}\r\n"));
    }
    if let Some(etag) = &answer.etag {
        head.push_str(&format!("ETag: \"{etag}\"\r\n"));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(&answer.body);
    bytes
}

/// In this process: the headers Plenipo sends and no others (contract §1), and then the service.
impl Transport for StandIn {
    fn send(
        &self,
        request: &Request,
        pass: Option<&str>,
    ) -> impl Future<Output = Result<Answer, String>> + Send {
        let mut headers = vec![
            ("accept".to_owned(), "application/json".to_owned()),
            (
                "user-agent".to_owned(),
                format!("Plenipo/{}", env!("CARGO_PKG_VERSION")),
            ),
        ];
        if request.body.is_some() {
            headers.push(("content-type".to_owned(), "application/json".to_owned()));
        }
        if let Some(pass) = pass {
            headers.push(("authorization".to_owned(), format!("Bearer {pass}")));
        }
        let method = request.method.as_str();
        let path = format!("{}{}", client::BASE_PATH, request.path);
        let body = request.body.clone().unwrap_or_default();
        let service = self.clone();
        async move {
            let answer = service.handle_waiting(method, &path, &headers, &body).await;
            Ok(Answer {
                status: answer.status,
                body: answer.body,
                retry_after: answer.retry_after,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamping_key_is_the_contracts() {
        let file: Value = serde_json::from_str(STAMPING_KEY_FILE).unwrap();
        assert_eq!(file["id"], STAMPING_KEY_ID);
        let public = stamping_public();
        let hex: String = public
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(file["publicHex"], hex.as_str());
    }

    #[test]
    fn versions_compare_as_numbers_and_a_suffix_is_below() {
        let v = |text: &str| Version::parse(text).unwrap();
        assert!(v("1.20.0") > v("1.9.0"), "numbers, not letters");
        assert!(v("1.20.0") > v("1.20.0-rc.1"), "a suffix is below");
        assert!(
            v("1.20.1-rc.1") > v("1.20.0"),
            "but only below the same numbers"
        );
        assert!(v("1.20") > v("1.20.0-rc.1"), "missing numbers count as 0");
        assert_eq!(v("1.20").cmp(&v("1.20.0")), std::cmp::Ordering::Equal);
        assert!(Version::parse("").is_none());
        assert!(Version::parse("one.two").is_none());
        assert!(Version::parse("1.2-").is_none());
    }

    #[test]
    fn a_week_starts_monday_morning_pacific_time() {
        // Saturday 2026-10-03 12:00 UTC is in the week that began Monday 2026-09-28 00:00 PDT
        // (07:00 UTC).
        let monday = days_from_civil(2026, 9, 28) * DAY + 7 * 3_600;
        assert_eq!(
            week_start(days_from_civil(2026, 10, 3) * DAY + 12 * 3_600),
            monday
        );
        assert_eq!(
            week_start(monday),
            monday,
            "the first second is in the week"
        );
        assert_eq!(
            week_start(monday - 1),
            monday - 7 * DAY,
            "the second before is not"
        );
        // In winter the offset is 8 hours: Monday 2026-12-07 00:00 PST is 08:00 UTC.
        let winter = days_from_civil(2026, 12, 7) * DAY + 8 * 3_600;
        assert_eq!(week_start(winter + 3 * DAY), winter);
        // Daylight saving time begins Sunday 2026-03-08 and ends Sunday 2026-11-01.
        assert_eq!(
            pacific_offset(days_from_civil(2026, 3, 8) * DAY + 9 * 3_600),
            -8 * 3_600
        );
        assert_eq!(
            pacific_offset(days_from_civil(2026, 3, 8) * DAY + 10 * 3_600),
            -7 * 3_600
        );
        assert_eq!(
            pacific_offset(days_from_civil(2026, 11, 1) * DAY + 8 * 3_600),
            -7 * 3_600
        );
        assert_eq!(
            pacific_offset(days_from_civil(2026, 11, 1) * DAY + 9 * 3_600),
            -8 * 3_600
        );
    }

    #[test]
    fn a_picture_is_a_real_small_png_and_nothing_else() {
        let png = tiny_png(64, 32);
        assert_eq!(check_png(&png).ok(), Some(png.clone()));
        assert!(matches!(check_png(b"hello"), Err(PngProblem::NotAPng)));
        assert!(matches!(
            check_png(&tiny_png(300, 10)),
            Err(PngProblem::TooBig)
        ));
        let mut changed = png.clone();
        let last = changed.len() - 20;
        changed[last] ^= 1;
        assert!(
            matches!(check_png(&changed), Err(PngProblem::NotAPng)),
            "a checksum"
        );
        assert!(matches!(
            check_png(&png[..png.len() - 1]),
            Err(PngProblem::NotAPng)
        ));
        // Extra chunks (like text) are dropped, and the rest is kept.
        let mut with_text = png[..png.len() - 12].to_vec();
        push_chunk(&mut with_text, b"tEXt", b"Comment\0hello");
        push_chunk(&mut with_text, b"IEND", &[]);
        assert_eq!(check_png(&with_text).ok(), Some(png));
    }

    #[test]
    fn two_spellings_of_one_mailbox_are_one_address() {
        assert_eq!(
            mailbox("Pat.Lee+work@Gmail.com"),
            mailbox("patlee@googlemail.com")
        );
        assert_ne!(
            mailbox("pat.lee@example.com"),
            mailbox("patlee@example.com")
        );
        assert_eq!(mailbox("not an address"), None);
    }

    #[test]
    fn queries_are_decoded() {
        assert_eq!(
            query_get("q=Pat%20Lee&kind=", "q").as_deref(),
            Some("Pat Lee")
        );
        assert_eq!(query_get("q=Pat&kind=", "kind"), None, "empty is not given");
        assert_eq!(query_get("wait=2", "wait").as_deref(), Some("2"));
        assert_eq!(held_for("GET", "/v1/community/items?wait=2"), Some(2));
        assert_eq!(held_for("GET", "/v1/community/items?wait=0"), None);
        assert_eq!(held_for("GET", "/v1/community/items"), None);
        assert_eq!(held_for("POST", "/v1/community/items?wait=2"), None);
    }
}
