//! Talking to the account service (contract §1): every request Plenipo makes, written exactly as
//! the contract says, and every answer read back. Nothing here sends anything: a [`Transport`]
//! does (Guard's check, then HTTPS, in `plenipo-capabilities`), and the tests use the stand-in
//! service.
//!
//! - Paths and queries are built here only, from checked parts: an ID that is not an ID, or a
//!   name that is not a Community name, never reaches an address.
//! - Every answer is read up to a limit, and only as the type the contract names for it.
//! - Nothing here logs. A request's `Debug` never shows its body, and nothing keeps the pass.

use std::future::Future;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ids::{self, IdKind};
use crate::wire;

/// The part of every address before a request's path (contract §1).
pub const BASE_PATH: &str = "/v1/community";
/// The longest wait a pick-up asks for (contract §6).
pub const MOST_WAIT_SECS: u32 = 25;
/// The most bytes of an ordinary answer read.
pub const MOST_ANSWER: usize = 256 * 1024;
/// The most bytes of a pick-up's answer: 50 items, each with one sealed copy of up to 128 KiB.
pub const MOST_INBOX_ANSWER: usize = 12 * 1024 * 1024;
/// The most bytes of a picture.
pub const MOST_PICTURE: usize = 256 * 1024;

/// How a request is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

/// One request, ready to send.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    /// Everything after [`BASE_PATH`], starting with `/`, with its query already encoded.
    pub path: String,
    /// JSON, or none.
    pub body: Option<Vec<u8>>,
    /// Whether it carries this PC's pass: every request but `open` and the two sign-in requests
    /// (contract §1, §2).
    pub with_pass: bool,
    /// The most bytes of the answer read.
    pub most_answer: usize,
    /// How long the service may hold the request open, for a pick-up (contract §6).
    pub held_secs: u32,
}

impl std::fmt::Debug for Request {
    // Never the body: it can hold a profile, a report's proof, or sealed bytes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("body_bytes", &self.body.as_ref().map(Vec::len))
            .field("with_pass", &self.with_pass)
            .finish()
    }
}

/// What the service answered.
#[derive(Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
    /// `Retry-After`, in seconds, when the service sent one.
    pub retry_after: Option<u64>,
}

impl std::fmt::Debug for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Answer")
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .field("retry_after", &self.retry_after)
            .finish()
    }
}

/// What carries a request to the account service and brings its answer back. The pass, when the
/// request carries one, goes in `Authorization: Bearer <pass>` and nowhere else. An error is the
/// reason there was no answer, in plain words.
pub trait Transport: Send + Sync {
    fn send(
        &self,
        request: &Request,
        pass: Option<&str>,
    ) -> impl Future<Output = Result<Answer, String>> + Send;
}

/// One of the contract's error codes (contract §1). A code this copy does not know is kept as
/// it came, so a newer service never makes an answer unreadable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    BadRequest,
    TooLarge,
    Unauthorized,
    NotMember,
    EmailNotConfirmed,
    AccountLimited,
    TooYoung,
    AdultsOnly,
    NeedsPro,
    CommunityPaused,
    CommunityEnded,
    TermsChanged,
    NotFound,
    NotDelivered,
    WaitingForAccept,
    DevicesChanged,
    NotAcceptingObjectives,
    ItemIdReused,
    AlreadyMember,
    NameTaken,
    NameNotAllowed,
    NameChangeTooSoon,
    TooManyDevices,
    Waiting,
    SlowDown,
    Denied,
    Expired,
    ProofFailed,
    AlreadyThanked,
    TooNewToThank,
    TooMany,
    GifsUnavailable,
    NotOpen,
    UpdateNeeded,
    Unavailable,
    Other(String),
}

impl ErrorCode {
    pub fn from_code(code: &str) -> Self {
        match code {
            "bad_request" => Self::BadRequest,
            "too_large" => Self::TooLarge,
            "unauthorized" => Self::Unauthorized,
            "not_member" => Self::NotMember,
            "email_not_confirmed" => Self::EmailNotConfirmed,
            "account_limited" => Self::AccountLimited,
            "too_young" => Self::TooYoung,
            "adults_only" => Self::AdultsOnly,
            "needs_pro" => Self::NeedsPro,
            "community_paused" => Self::CommunityPaused,
            "community_ended" => Self::CommunityEnded,
            "terms_changed" => Self::TermsChanged,
            "not_found" => Self::NotFound,
            "not_delivered" => Self::NotDelivered,
            "waiting_for_accept" => Self::WaitingForAccept,
            "devices_changed" => Self::DevicesChanged,
            "not_accepting_objectives" => Self::NotAcceptingObjectives,
            "item_id_reused" => Self::ItemIdReused,
            "already_member" => Self::AlreadyMember,
            "name_taken" => Self::NameTaken,
            "name_not_allowed" => Self::NameNotAllowed,
            "name_change_too_soon" => Self::NameChangeTooSoon,
            "too_many_devices" => Self::TooManyDevices,
            "waiting" => Self::Waiting,
            "slow_down" => Self::SlowDown,
            "denied" => Self::Denied,
            "expired" => Self::Expired,
            "proof_failed" => Self::ProofFailed,
            "already_thanked" => Self::AlreadyThanked,
            "too_new_to_thank" => Self::TooNewToThank,
            "too_many" => Self::TooMany,
            "gifs_unavailable" => Self::GifsUnavailable,
            "not_open" => Self::NotOpen,
            "update_needed" => Self::UpdateNeeded,
            "unavailable" => Self::Unavailable,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// An error the service answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceError {
    pub status: u16,
    pub code: ErrorCode,
    /// One plain sentence Plenipo may show, from 8 West (contract §1): shown as text only.
    pub message: String,
    pub retry_after: Option<u64>,
    /// For `proof_failed`: which reported item failed.
    pub item: Option<u32>,
}

/// Why a request did not get the answer it wanted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The service answered with one of the contract's errors.
    Service(ServiceError),
    /// No answer: no internet, the service did not answer in time, or Guard refused the address.
    Unreachable(String),
    /// An answer that is not what the contract says.
    BadAnswer,
}

impl Failure {
    /// The service's error code, when it answered with one.
    pub fn code(&self) -> Option<&ErrorCode> {
        match self {
            Self::Service(e) => Some(&e.code),
            _ => None,
        }
    }
}

/// The longest error message shown (the contract's schema).
const MOST_MESSAGE_CHARS: usize = 300;

/// Send `request`, with `pass` when it carries one, and bring back the answer or why there was
/// none.
pub async fn send(
    transport: &impl Transport,
    request: &Request,
    pass: Option<&str>,
) -> Result<Answer, Failure> {
    let pass = if request.with_pass { pass } else { None };
    transport
        .send(request, pass)
        .await
        .map_err(Failure::Unreachable)
}

/// Read an answer: the type `T` when the status is `ok`, the service's error otherwise.
pub fn read<T: DeserializeOwned>(answer: Answer, ok: u16) -> Result<T, Failure> {
    if answer.status == ok {
        return serde_json::from_slice(&answer.body).map_err(|_| Failure::BadAnswer);
    }
    Err(error_of(answer))
}

/// Read an answer that has no body: `204` (or `202` for an email invitation).
pub fn read_empty(answer: Answer, ok: u16) -> Result<(), Failure> {
    if answer.status == ok {
        return Ok(());
    }
    Err(error_of(answer))
}

/// The service's error in an answer that is not the one wanted. An answer that is not one of
/// the contract's errors (a proxy's page, an empty body) is a bad answer, except that a `503`
/// with no error of the contract's is the service being busy.
fn error_of(answer: Answer) -> Failure {
    match serde_json::from_slice::<wire::Error>(&answer.body) {
        Ok(error) if (400..600).contains(&answer.status) => Failure::Service(ServiceError {
            status: answer.status,
            code: ErrorCode::from_code(&error.error),
            message: error.message.chars().take(MOST_MESSAGE_CHARS).collect(),
            retry_after: answer.retry_after,
            item: error.item,
        }),
        _ if answer.status == 503 => Failure::Service(ServiceError {
            status: 503,
            code: ErrorCode::Unavailable,
            message: "Community can't be reached right now. Nothing was changed.".into(),
            retry_after: answer.retry_after,
            item: None,
        }),
        _ => Failure::BadAnswer,
    }
}

/// Why a request could not be written: a part that is not what the contract allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAllowed(pub &'static str);

fn json(body: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(body).expect("a request always serializes")
}

fn get(path: String) -> Request {
    Request {
        method: Method::Get,
        path,
        body: None,
        with_pass: true,
        most_answer: MOST_ANSWER,
        held_secs: 0,
    }
}

fn with_body(method: Method, path: &str, body: &impl Serialize) -> Request {
    Request {
        method,
        path: path.to_owned(),
        body: Some(json(body)),
        with_pass: true,
        most_answer: MOST_ANSWER,
        held_secs: 0,
    }
}

fn bare(method: Method, path: String) -> Request {
    Request {
        method,
        path,
        body: None,
        with_pass: true,
        most_answer: MOST_ANSWER,
        held_secs: 0,
    }
}

/// An ID of this kind, for an address.
fn id(kind: IdKind, text: &str) -> Result<&str, NotAllowed> {
    if ids::is_id(kind, text) {
        Ok(text)
    } else {
        Err(NotAllowed("That isn't an ID the contract allows."))
    }
}

/// Whether `name` is a Community name (contract §3): 3 to 30 of `a-z`, `0-9`, and `-`, starting
/// and ending with a letter or a number. The service decides which names are allowed; this only
/// keeps anything else out of an address.
pub fn is_community_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (3..=30).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && bytes[0] != b'-'
        && bytes[bytes.len() - 1] != b'-'
}

/// A query: each pair whose value is not empty, percent-encoded, in the order given.
fn query(pairs: &[(&str, &str)]) -> String {
    let encoded: Vec<String> = pairs
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(word, value)| format!("{word}={}", encode(value)))
        .collect();
    if encoded.is_empty() {
        String::new()
    } else {
        format!("?{}", encoded.join("&"))
    }
}

/// Percent-encode everything but letters, numbers, and `- . _ ~`.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// Every request, by the contract's section.

/// Is Community open? (contract §1). No pass, no body.
pub fn open() -> Request {
    Request {
        with_pass: false,
        ..get("/open".into())
    }
}

/// Start signing in (contract §2). No pass.
pub fn sign_in_start(body: &wire::SignInStart) -> Request {
    Request {
        with_pass: false,
        ..with_body(Method::Post, "/sign-in/start", body)
    }
}

/// Ask whether the person allowed it yet (contract §2). No pass.
pub fn sign_in_token(body: &wire::SignInToken) -> Request {
    Request {
        with_pass: false,
        ..with_body(Method::Post, "/sign-in/token", body)
    }
}

/// Sign this PC out (contract §2).
pub fn sign_out() -> Request {
    bare(Method::Post, "/sign-out".into())
}

/// Who this is (contract §3).
pub fn me() -> Request {
    get("/me".into())
}

/// Join Community (contract §3).
pub fn join(body: &wire::Join) -> Request {
    with_body(Method::Post, "/join", body)
}

/// Change the Community name (contract §3).
pub fn change_name(body: &wire::NameChange) -> Request {
    with_body(Method::Put, "/me/name", body)
}

/// Accept the Community terms again (contract §3).
pub fn accept_terms(body: &wire::TermsAccept) -> Request {
    with_body(Method::Put, "/me/terms", body)
}

/// Show these parts of the profile (contract §3).
pub fn set_profile(body: &wire::Profile) -> Request {
    with_body(Method::Put, "/me/profile", body)
}

/// Show this picture (contract §3).
pub fn set_picture(body: &wire::PictureUpload) -> Request {
    with_body(Method::Put, "/me/picture", body)
}

/// Stop showing a picture (contract §3).
pub fn remove_picture() -> Request {
    bare(Method::Delete, "/me/picture".into())
}

/// Appear offline, or not (contract §3).
pub fn set_presence(body: &wire::Presence) -> Request {
    with_body(Method::Put, "/me/presence", body)
}

/// Leave Community (contract §3).
pub fn leave() -> Request {
    bare(Method::Delete, "/me".into())
}

/// The directory, or New this week (contract §4).
pub fn directory(new_this_week: bool, q: &str, kind: &str, region: &str, cursor: &str) -> Request {
    let path = if new_this_week {
        format!("/directory/new{}", query(&[("cursor", cursor)]))
    } else {
        format!(
            "/directory{}",
            query(&[
                ("q", q),
                ("kind", kind),
                ("region", region),
                ("cursor", cursor)
            ])
        )
    };
    get(path)
}

/// A card by exact Community name (contract §4).
pub fn by_name(name: &str) -> Result<Request, NotAllowed> {
    if !is_community_name(name) {
        return Err(NotAllowed("That isn't a Community name."));
    }
    Ok(get(format!("/people/by-name/{name}")))
}

/// One member's card (contract §4).
pub fn person(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(get(format!("/people/{}", id(IdKind::Member, member_id)?)))
}

/// One member's picture (contract §4).
pub fn picture_of(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(Request {
        most_answer: MOST_PICTURE,
        ..get(format!(
            "/people/{}/picture",
            id(IdKind::Member, member_id)?
        ))
    })
}

/// One member's PCs and their public keys (contract §4).
pub fn devices_of(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(get(format!(
        "/people/{}/devices",
        id(IdKind::Member, member_id)?
    )))
}

/// Everyone this member talks with, or has asked or been asked (contract §5).
pub fn contacts() -> Request {
    get("/contacts".into())
}

/// Accept someone's first message (contract §5).
pub fn accept_contact(member_id: &str) -> Result<Request, NotAllowed> {
    let path = format!("/contacts/{}/accept", id(IdKind::Member, member_id)?);
    Ok(bare(Method::Post, path))
}

/// Leave this conversation, decline a request, or take one back (contract §5).
pub fn leave_contact(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(bare(
        Method::Delete,
        format!("/contacts/{}", id(IdKind::Member, member_id)?),
    ))
}

/// Send a sealed item (contract §6).
pub fn send_item(body: &wire::ItemSend) -> Request {
    with_body(Method::Post, "/items", body)
}

/// Pick up what is waiting for this PC, waiting up to `wait` seconds (contract §6).
pub fn pick_up(wait: u32) -> Request {
    let wait = wait.min(MOST_WAIT_SECS);
    Request {
        most_answer: MOST_INBOX_ANSWER,
        held_secs: wait,
        ..get(format!("/items?wait={wait}"))
    }
}

/// Done with these items (contract §6).
pub fn ack(body: &wire::Ack) -> Request {
    with_body(Method::Post, "/items/ack", body)
}

/// Who this member blocked (contract §8).
pub fn blocks() -> Request {
    get("/blocks".into())
}

/// Block someone (contract §8).
pub fn block(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(bare(
        Method::Put,
        format!("/blocks/{}", id(IdKind::Member, member_id)?),
    ))
}

/// Unblock someone (contract §8).
pub fn unblock(member_id: &str) -> Result<Request, NotAllowed> {
    Ok(bare(
        Method::Delete,
        format!("/blocks/{}", id(IdKind::Member, member_id)?),
    ))
}

/// Report someone, a profile, or items (contract §11).
pub fn report(body: &wire::Report) -> Request {
    with_body(Method::Post, "/reports", body)
}

/// Thank someone (contract §12).
pub fn thanks(body: &wire::Thanks) -> Request {
    with_body(Method::Post, "/thanks", body)
}

/// This member's points (contract §12).
pub fn points() -> Request {
    get("/me/points".into())
}

/// The leaderboard, this week or all time (contract §12).
pub fn leaderboard(all_time: bool) -> Request {
    let period = if all_time { "all" } else { "week" };
    get(format!("/leaderboard{}", query(&[("period", period)])))
}

/// Invite someone by email (contract §13).
pub fn invite_email(body: &wire::InviteEmail) -> Request {
    with_body(Method::Post, "/invite-email", body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAT: &str = "cm_01JB7Q8R9S0T1V2W3X4Y5Z6A7B";

    #[test]
    fn paths_are_built_only_from_checked_parts() {
        assert_eq!(open().path, "/open");
        assert!(!open().with_pass, "is it open? carries no pass");
        assert_eq!(person(PAT).unwrap().path, format!("/people/{PAT}"));
        assert_eq!(
            devices_of(PAT).unwrap().path,
            format!("/people/{PAT}/devices")
        );
        assert_eq!(by_name("pat-lee").unwrap().path, "/people/by-name/pat-lee");
        for bad in [
            "../me",
            "cm_01JB7Q8R9S0T1V2W3X4Y5Z6A7B/../x",
            "",
            "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7B",
        ] {
            assert!(person(bad).is_err(), "{bad}");
            assert!(block(bad).is_err(), "{bad}");
        }
        let too_long = "a".repeat(31);
        for bad in [
            "Pat", "pa", "-pat", "pat-", "pat lee", "pat/lee", "pät", &too_long,
        ] {
            assert!(by_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn queries_are_encoded_and_empty_parts_left_out() {
        assert_eq!(directory(false, "", "", "", "").path, "/directory");
        assert_eq!(
            directory(false, "Pat Lee & co", "construction", "US-CA", "").path,
            "/directory?q=Pat%20Lee%20%26%20co&kind=construction&region=US-CA"
        );
        assert_eq!(
            directory(false, "café", "", "", "").path,
            "/directory?q=caf%C3%A9"
        );
        assert_eq!(
            directory(true, "ignored", "", "", "abc").path,
            "/directory/new?cursor=abc"
        );
        assert_eq!(
            pick_up(60).path,
            "/items?wait=25",
            "never longer than 25 seconds"
        );
        assert_eq!(pick_up(60).held_secs, 25);
        assert_eq!(leaderboard(true).path, "/leaderboard?period=all");
    }

    #[test]
    fn a_requests_body_is_never_shown() {
        let request = invite_email(&wire::InviteEmail {
            email: "pat@example.com".into(),
        });
        assert!(!format!("{request:?}").contains("pat@example.com"));
    }

    #[test]
    fn answers_and_errors_are_read_as_the_contract_says() {
        let ok = Answer {
            status: 200,
            body: br#"{"links":false,"collaborators":true,"later":1}"#.to_vec(),
            retry_after: None,
        };
        assert_eq!(
            read::<wire::Open>(ok, 200).unwrap(),
            wire::Open {
                links: false,
                collaborators: true
            }
        );
        let not_open = Answer {
            status: 503,
            body: br#"{"error":"not_open","message":"Community isn't open yet."}"#.to_vec(),
            retry_after: None,
        };
        assert_eq!(
            read::<wire::Open>(not_open, 200).unwrap_err().code(),
            Some(&ErrorCode::NotOpen)
        );
        let newer = Answer {
            status: 409,
            body: br#"{"error":"something_new","message":"New."}"#.to_vec(),
            retry_after: None,
        };
        assert_eq!(
            read_empty(newer, 204).unwrap_err().code(),
            Some(&ErrorCode::Other("something_new".into()))
        );
        let busy = Answer {
            status: 503,
            body: b"<html>busy</html>".to_vec(),
            retry_after: Some(30),
        };
        let Failure::Service(busy) = read::<wire::Open>(busy, 200).unwrap_err() else {
            panic!("a 503 with no contract error is the service being busy");
        };
        assert_eq!(
            (busy.code, busy.retry_after),
            (ErrorCode::Unavailable, Some(30))
        );
        let proxy = Answer {
            status: 403,
            body: b"<html>no</html>".to_vec(),
            retry_after: None,
        };
        assert_eq!(
            read::<wire::Open>(proxy, 200).unwrap_err(),
            Failure::BadAnswer
        );
        let long = Answer {
            status: 400,
            body: format!(
                r#"{{"error":"bad_request","message":"{}"}}"#,
                "a".repeat(5000)
            )
            .into_bytes(),
            retry_after: None,
        };
        let Failure::Service(long) = read_empty(long, 204).unwrap_err() else {
            panic!("a long message is still an error")
        };
        assert_eq!(long.message.chars().count(), 300, "a message is cut short");
    }
}
