//! Signing this PC in to Community, and who it is once it is (ADR-162 §2, ADR-170, contract §1
//! to §3).
//!
//! - **Is Community open?** is asked only when the person presses the Community switch or
//!   **Check again** (ADR-170 §2). It carries nothing about the person or the PC.
//! - **Signing in** never sees a password: this PC makes new keys, asks for a short code, the
//!   person types it on the account site and presses **Allow**, and this PC gets its Community
//!   pass. The pass, the keys, and this PC's ID are kept in the Vault together, and never shown,
//!   logged, or sent anywhere but in `Authorization` to the account service.
//! - **The age** is checked here first: under 13, nothing is sent and nothing is kept (ADR-162
//!   §4). The service checks again.

use serde::{Deserialize, Serialize};

use crate::client::{self, ErrorCode, Failure, Transport};
use crate::ids::{self, IdKind};
use crate::keys::{self, PcKeys};
use crate::{b64, wire, CommunityError, Result};

/// The label a PC signs its device code under, to finish signing in (contract §2).
pub const SIGN_IN_CONTEXT: &str = "plenipo-community-sign-in.v1.";
/// The account site's page where the person types the code (contract §2). The PC opens only
/// this page, whatever an answer says.
pub const CONNECT_PAGE: &str = "https://account.getplenipo.com/community/connect";
/// A pass is 43 characters (contract §2).
const PASS_CHARS: usize = 43;
/// The letters a code is made of (contract §2).
const CODE_LETTERS: &str = "BCDFGHJKLMNPQRSTVWXZ23456789";
/// The longest PC name (contract §2).
const MOST_DEVICE_NAME_CHARS: usize = 60;

/// What "Is Community open?" found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// Open: the people part, and whether linked organizations and collaborators are.
    Open(wire::Open),
    /// Not open yet: **Coming soon**.
    NotOpen,
    /// This version of Plenipo may not use Community: **Update Plenipo to use Community**.
    UpdateNeeded,
    /// No answer, or the service is busy: never **Coming soon**.
    Unreachable,
}

/// Ask whether Community is open (contract §1). Only when the person asks (ADR-170 §2).
pub async fn check_open(transport: &impl Transport) -> Opening {
    let answer = client::send(transport, &client::open(), None).await;
    match answer.and_then(|a| client::read::<wire::Open>(a, 200)) {
        Ok(open) => Opening::Open(open),
        Err(failure) => match failure.code() {
            Some(ErrorCode::NotOpen) => Opening::NotOpen,
            Some(ErrorCode::UpdateNeeded) => Opening::UpdateNeeded,
            _ => Opening::Unreachable,
        },
    }
}

/// This PC, signed in: its keys, its pass, and its ID. Kept in the Vault as one value.
pub struct SignedInPc {
    keys: PcKeys,
    pass: String,
    device_id: String,
}

/// How the Vault keeps a signed-in PC.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kept {
    v: u8,
    keys: String,
    pass: String,
    device_id: String,
}

impl SignedInPc {
    pub fn keys(&self) -> &PcKeys {
        &self.keys
    }

    /// The pass, for `Authorization` only.
    pub fn pass(&self) -> &str {
        &self.pass
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// As the Vault keeps it, under [`keys::KEYS_ID`].
    pub fn write(&self) -> String {
        serde_json::to_string(&Kept {
            v: 1,
            keys: self.keys.write(),
            pass: self.pass.clone(),
            device_id: self.device_id.clone(),
        })
        .expect("a signed-in PC always serializes")
    }

    /// Read back from the Vault.
    pub fn read(text: &str) -> Result<Self> {
        let unreadable = || {
            CommunityError::Invalid("Community's sign-in on this computer can't be read.".into())
        };
        let kept: Kept = serde_json::from_str(text).map_err(|_| unreadable())?;
        if kept.v != 1 || !is_pass(&kept.pass) || !ids::is_id(IdKind::Device, &kept.device_id) {
            return Err(unreadable());
        }
        Ok(Self {
            keys: PcKeys::read(&kept.keys)?,
            pass: kept.pass,
            device_id: kept.device_id,
        })
    }
}

impl std::fmt::Debug for SignedInPc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedInPc")
            .field("device_id", &self.device_id)
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

/// Whether `text` looks like a pass: 43 characters of base64url.
fn is_pass(text: &str) -> bool {
    text.len() == PASS_CHARS
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether `code` looks like a sign-in code: `4KQ-7TD`.
fn is_user_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 7
        && bytes[3] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 3 || CODE_LETTERS.as_bytes().contains(b))
}

/// The PC's name as the person will see it on the account site: one line, no characters that
/// could disguise it, 1 to 60 characters, or "This computer".
pub fn device_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| !c.is_control() && !is_hidden(*c))
        .take(MOST_DEVICE_NAME_CHARS)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "This computer".into()
    } else {
        clean.into()
    }
}

/// Characters a person cannot see that can change how words read (contract §2: "no control
/// characters or text-direction marks").
fn is_hidden(c: char) -> bool {
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

/// A sign-in under way: the code to show, and this PC's new keys, waiting for **Allow**.
pub struct SigningIn {
    keys: PcKeys,
    device_code: String,
    /// "Enter this code: 4KQ-7TD".
    pub user_code: String,
    /// When the code runs out (Unix seconds).
    pub expires_at: i64,
    /// How often to ask whether the person allowed it, in seconds.
    pub interval: u32,
}

impl std::fmt::Debug for SigningIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningIn")
            .field("user_code", &self.user_code)
            .field("expires_at", &self.expires_at)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

/// Start signing in: make this PC's new keys and ask for a code (contract §2).
pub async fn start(
    transport: &impl Transport,
    device: &str,
    app_version: &str,
    now: i64,
) -> std::result::Result<SigningIn, Failure> {
    let keys = PcKeys::generate();
    let body = wire::SignInStart {
        device_name: device_name(device),
        app_version: app_version.to_owned(),
        signing_key: keys.signing_key_text(),
        sealing_key: keys.sealing_key_text(),
    };
    let answer = client::send(transport, &client::sign_in_start(&body), None).await?;
    let started: wire::SignInStarted = client::read(answer, 200)?;
    if started.device_code.len() != PASS_CHARS || !is_user_code(&started.user_code) {
        return Err(Failure::BadAnswer);
    }
    Ok(SigningIn {
        keys,
        device_code: started.device_code,
        user_code: started.user_code,
        // Never longer than the contract's 10 minutes, whatever the answer says.
        expires_at: now + i64::from(started.expires_in.min(600)),
        // Never more often than every 5 seconds.
        interval: started.interval.max(5),
    })
}

/// Where a sign-in got to.
#[derive(Debug)]
pub enum Finish {
    /// The person has not pressed **Allow** yet.
    Waiting,
    /// Asked too soon: wait 5 seconds longer each time.
    SlowDown,
    /// The person pressed **Don't allow**.
    Denied,
    /// The code ran out.
    Expired,
    /// Signed in.
    SignedIn(Box<SignedInPc>, Box<wire::Me>),
}

/// Ask whether the person allowed the sign-in yet (contract §2).
pub async fn finish(
    transport: &impl Transport,
    signing_in: SigningIn,
) -> std::result::Result<(Finish, Option<SigningIn>), Failure> {
    let proof = b64::encode(
        &signing_in
            .keys
            .sign(SIGN_IN_CONTEXT, &signing_in.device_code),
    );
    let body = wire::SignInToken {
        device_code: signing_in.device_code.clone(),
        proof,
    };
    let answer = client::send(transport, &client::sign_in_token(&body), None).await?;
    match client::read::<wire::SignedIn>(answer, 200) {
        Ok(signed_in) => {
            if !is_pass(&signed_in.pass)
                || !ids::is_id(IdKind::Device, &signed_in.device_id)
                || signed_in.me.device_id != signed_in.device_id
            {
                return Err(Failure::BadAnswer);
            }
            let pc = SignedInPc {
                keys: signing_in.keys,
                pass: signed_in.pass,
                device_id: signed_in.device_id,
            };
            Ok((Finish::SignedIn(Box::new(pc), Box::new(signed_in.me)), None))
        }
        Err(failure) => match failure.code() {
            Some(ErrorCode::Waiting) => Ok((Finish::Waiting, Some(signing_in))),
            Some(ErrorCode::SlowDown) => Ok((Finish::SlowDown, Some(signing_in))),
            Some(ErrorCode::Denied) => Ok((Finish::Denied, None)),
            Some(ErrorCode::Expired) => Ok((Finish::Expired, None)),
            _ => Err(failure),
        },
    }
}

/// Who this is, now (contract §3).
pub async fn me(
    transport: &impl Transport,
    pc: &SignedInPc,
) -> std::result::Result<wire::Me, Failure> {
    let answer = client::send(transport, &client::me(), Some(pc.pass())).await?;
    client::read(answer, 200)
}

/// Sign this PC out (contract §2). Already signed out (`unauthorized`) is signed out too.
pub async fn sign_out(
    transport: &impl Transport,
    pc: &SignedInPc,
) -> std::result::Result<(), Failure> {
    let answer = client::send(transport, &client::sign_out(), Some(pc.pass())).await?;
    match client::read_empty(answer, 204) {
        Err(failure) if failure.code() == Some(&ErrorCode::Unauthorized) => Ok(()),
        other => other,
    }
}

/// Leave Community (contract §3, ADR-167 §15): every PC of this member is signed out at 8 West.
pub async fn leave(
    transport: &impl Transport,
    pc: &SignedInPc,
) -> std::result::Result<(), Failure> {
    let answer = client::send(transport, &client::leave(), Some(pc.pass())).await?;
    client::read_empty(answer, 204)
}

/// How old someone is by the contract's rule (contract §3): a birthday counts only once its month
/// has passed, so a person turns 13 the month after their birth month.
pub fn age(birth_month: u8, birth_year: u16, month: u8, year: u16) -> Option<u16> {
    if !(1..=12).contains(&birth_month) || !(1..=12).contains(&month) || birth_year > year {
        return None;
    }
    let years = year - birth_year;
    if month <= birth_month {
        years.checked_sub(1)
    } else {
        Some(years)
    }
}

/// What the age box says before anything is sent (ADR-162 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeCheck {
    /// Not a month and year that can be a birth: asked again.
    NotADate,
    /// Under 13: "Community is for people 13 and older". Nothing is sent, nothing is kept.
    TooYoung,
    /// 13 or older: the service checks again, and keeps the month and year.
    OldEnough,
}

/// Check the age box. `month` and `year` are today's.
pub fn check_age(birth_month: u8, birth_year: u16, month: u8, year: u16) -> AgeCheck {
    if birth_year < 1900 {
        return AgeCheck::NotADate;
    }
    match age(birth_month, birth_year, month, year) {
        None => AgeCheck::NotADate,
        Some(age) if age < 13 => AgeCheck::TooYoung,
        Some(_) => AgeCheck::OldEnough,
    }
}

/// Join Community (contract §3), after the age box said 13 or older.
pub async fn join(
    transport: &impl Transport,
    pc: &SignedInPc,
    body: &wire::Join,
) -> std::result::Result<wire::Me, Failure> {
    let answer = client::send(transport, &client::join(body), Some(pc.pass())).await?;
    client::read(answer, 200)
}

/// Today's month and year from Unix seconds (UTC): enough for a month-and-year rule, which the
/// service checks again.
pub fn month_and_year(unix: i64) -> (u8, u16) {
    // Days since 1970-01-01, then the civil date (Howard Hinnant's method).
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (month as u8, year as u16)
}

/// Whether a PC's signing key and signature are right: a check the tests and the stand-in
/// service use for the sign-in's proof.
pub fn proof_checks(signing_key: &str, device_code: &str, proof: &str) -> bool {
    keys::verify(signing_key, SIGN_IN_CONTEXT, device_code, proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_age_rule_is_the_contracts() {
        // Born March 2013: 12 in March 2026, 13 in April 2026.
        assert_eq!(age(3, 2013, 3, 2026), Some(12));
        assert_eq!(age(3, 2013, 4, 2026), Some(13));
        assert_eq!(check_age(3, 2013, 3, 2026), AgeCheck::TooYoung);
        assert_eq!(check_age(3, 2013, 4, 2026), AgeCheck::OldEnough);
        assert_eq!(check_age(12, 2008, 12, 2026), AgeCheck::OldEnough);
        assert_eq!(check_age(13, 2000, 1, 2026), AgeCheck::NotADate);
        assert_eq!(check_age(1, 2027, 1, 2026), AgeCheck::NotADate);
        assert_eq!(check_age(1, 1800, 1, 2026), AgeCheck::NotADate);
        assert_eq!(check_age(5, 2026, 6, 2026), AgeCheck::TooYoung);
    }

    #[test]
    fn todays_month_and_year() {
        assert_eq!(month_and_year(0), (1, 1970));
        assert_eq!(month_and_year(1_790_000_000), (9, 2026));
        assert_eq!(month_and_year(1_772_323_200), (3, 2026), "2026-03-01");
        assert_eq!(
            month_and_year(1_772_323_199),
            (2, 2026),
            "the second before"
        );
        assert_eq!(month_and_year(951_782_400), (2, 2000), "a leap day");
    }

    #[test]
    fn a_pcs_name_is_one_plain_line() {
        assert_eq!(device_name("FRANKIE-DESKTOP"), "FRANKIE-DESKTOP");
        assert_eq!(device_name("evil\u{202E}txt.exe"), "eviltxt.exe");
        assert_eq!(device_name("two\nlines"), "twolines");
        assert_eq!(device_name("  "), "This computer");
        assert_eq!(device_name(&"a".repeat(80)).chars().count(), 60);
    }

    #[test]
    fn a_signed_in_pc_is_kept_and_its_pass_never_shown() {
        let pc = SignedInPc {
            keys: PcKeys::generate(),
            pass: "p".repeat(43),
            device_id: "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7C".into(),
        };
        let back = SignedInPc::read(&pc.write()).unwrap();
        assert_eq!(back.pass(), pc.pass());
        assert_eq!(back.device_id(), pc.device_id());
        assert_eq!(back.keys().signing_public(), pc.keys().signing_public());
        assert!(!format!("{pc:?}").contains(&"p".repeat(43)));
        assert!(SignedInPc::read("{}").is_err());
        let short_pass = pc.write().replace(&"p".repeat(43), "short");
        assert!(SignedInPc::read(&short_pass).is_err());
    }

    #[test]
    fn codes_look_like_the_contracts() {
        assert!(is_user_code("4KQ-7TD"));
        assert!(!is_user_code("4KQ7TD"));
        assert!(!is_user_code("4kq-7td"));
        assert!(
            !is_user_code("AEI-OU1"),
            "vowels and 1 are not in the code's letters"
        );
    }
}
