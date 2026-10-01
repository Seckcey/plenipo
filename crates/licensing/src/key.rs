//! The license key (ADR-104 §6): `plenipo1.<payload>.<signature>`.
//!
//! The payload is JSON in base64url: the format version, the edition (Pro or Partner), how many
//! organizations it covers (ADR-119), the key ID, the holder (the buyer's name or company, never
//! their email), the plan, the paid-through date, when it was issued, and which signing key signed
//! it. The signature covers [`KEY_CONTEXT`] followed by the
//! payload's base64 text. Plenipo checks it on the owner's own PC, with no network.

use std::fmt;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::codec::{self, SignError, KEY_CONTEXT};

/// Every license key starts with this.
pub const KEY_PREFIX: &str = "plenipo1.";
/// The longest key text accepted (a real key is about 400 characters).
pub const MAX_KEY_CHARS: usize = 2048;
/// The longest holder name.
pub const MAX_HOLDER_CHARS: usize = 120;

/// The plan a key was bought on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Plan {
    Monthly,
    Yearly,
}

/// Which paid edition a key is for (ADR-119): Pro for one owner's own business, Partner for
/// companies that run Plenipo for clients. Both are Pro inside Plenipo; they differ in how many
/// organizations they cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum KeyEdition {
    Pro,
    Partner,
}

/// How many organizations a key covers on the PC (ADR-119): a number, or `"unlimited"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Organizations {
    Up(u32),
    Unlimited(UnlimitedWord),
}

/// The word `"unlimited"`, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlimitedWord {
    Unlimited,
}

impl Organizations {
    pub const UNLIMITED: Self = Self::Unlimited(UnlimitedWord::Unlimited);

    /// The most organizations covered (`None`: no limit).
    pub fn most(self) -> Option<u32> {
        match self {
            Self::Up(n) => Some(n),
            Self::Unlimited(_) => None,
        }
    }
}

/// What a license key says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPayload {
    /// Format version: 1.
    pub v: u32,
    /// `pro` or `partner`.
    pub edition: String,
    /// How many organizations it covers (ADR-119): Pro 3; Partner 10, 25, or `"unlimited"`.
    pub organizations: Organizations,
    /// `lk_` and 26 letters and digits: the only thing the weekly check sends about the key.
    pub key_id: String,
    /// The buyer's name or company, as they typed it.
    pub holder: String,
    pub plan: Plan,
    /// Unix seconds.
    pub paid_through: i64,
    /// Unix seconds.
    pub issued_at: i64,
    /// Which signing key signed it ([`crate::trust`]).
    pub signer: String,
}

/// A license key whose signature Plenipo has checked.
#[derive(Clone, PartialEq, Eq)]
pub struct LicenseKey {
    text: String,
    payload: KeyPayload,
}

impl LicenseKey {
    /// The key as entered, without spaces or line breaks. A secret: kept only in the Vault.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn payload(&self) -> &KeyPayload {
        &self.payload
    }

    pub fn key_id(&self) -> &str {
        &self.payload.key_id
    }

    /// Pro or Partner (checked when the key was read).
    pub fn edition(&self) -> KeyEdition {
        if self.payload.edition == "partner" {
            KeyEdition::Partner
        } else {
            KeyEdition::Pro
        }
    }

    /// The most organizations it covers (`None`: no limit).
    pub fn organizations(&self) -> Option<u32> {
        self.payload.organizations.most()
    }
}

impl fmt::Debug for LicenseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The key text is a secret: never in a log line or an error.
        f.debug_struct("LicenseKey")
            .field("key_id", &self.payload.key_id)
            .finish_non_exhaustive()
    }
}

/// Why a key was refused, in plain words (ADR-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error(
        "That doesn't look like a Plenipo license key. Copy the whole key from the email 8 West \
         sent you, and paste it again."
    )]
    Malformed,
    #[error("This key is for a newer version of Plenipo. Update Plenipo, then enter it again.")]
    Newer,
    #[error("This key wasn't signed by 8 West, so Plenipo can't use it.")]
    UnknownSigner,
    #[error("This key has been changed or damaged. Copy it again from the email 8 West sent you.")]
    Damaged,
    #[error("This key isn't a Plenipo Pro or Partner key.")]
    NotPro,
}

/// The key as pasted, without any spaces or line breaks an email may have added.
pub fn normalize(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether `id` is a key ID: `lk_` and 26 letters and digits (Crockford's base 32, capitals).
pub fn is_key_id(id: &str) -> bool {
    id.strip_prefix("lk_").is_some_and(|rest| {
        rest.len() == 26
            && rest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b.is_ascii_uppercase() && !b"ILOU".contains(&b)))
    })
}

fn holder_ok(holder: &str) -> bool {
    let n = holder.chars().count();
    !holder.trim().is_empty() && n <= MAX_HOLDER_CHARS && !holder.chars().any(char::is_control)
}

/// Check a license key: its shape, then its signature, then what it says.
pub fn parse(text: &str) -> Result<LicenseKey, KeyError> {
    let text = normalize(text);
    if text.len() > MAX_KEY_CHARS {
        return Err(KeyError::Malformed);
    }
    let rest = text.strip_prefix(KEY_PREFIX).ok_or(KeyError::Malformed)?;
    let mut parts = rest.split('.');
    let (Some(payload_b64), Some(signature_b64), None) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(KeyError::Malformed);
    };
    let bytes = codec::decode(payload_b64).ok_or(KeyError::Malformed)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| KeyError::Malformed)?;
    match raw.get("v").and_then(serde_json::Value::as_u64) {
        Some(1) => {}
        Some(v) if v > 1 => return Err(KeyError::Newer),
        _ => return Err(KeyError::Malformed),
    }
    let payload: KeyPayload = serde_json::from_value(raw).map_err(|_| KeyError::Malformed)?;
    codec::verify(KEY_CONTEXT, payload_b64, signature_b64, &payload.signer).map_err(
        |e| match e {
            SignError::Malformed => KeyError::Malformed,
            SignError::UnknownSigner => KeyError::UnknownSigner,
            SignError::BadSignature => KeyError::Damaged,
        },
    )?;
    if payload.edition != "pro" && payload.edition != "partner" {
        return Err(KeyError::NotPro);
    }
    if payload.organizations == Organizations::Up(0) {
        return Err(KeyError::Malformed);
    }
    if !is_key_id(&payload.key_id)
        || !holder_ok(&payload.holder)
        || payload.issued_at <= 0
        || payload.paid_through < payload.issued_at
    {
        return Err(KeyError::Malformed);
    }
    Ok(LicenseKey { text, payload })
}

/// Make a key from `payload` signed with `key` (the tests and the contract's examples; 8 West's
/// keys are made by the account service, signed in the vault).
pub fn mint(payload: &KeyPayload, key: &SigningKey) -> String {
    let payload_b64 = codec::encode(&serde_json::to_vec(payload).expect("a payload is JSON"));
    let signature = codec::sign(KEY_CONTEXT, &payload_b64, key);
    format!("{KEY_PREFIX}{payload_b64}.{signature}")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::trust;

    pub(crate) const KEY_ID: &str = "lk_01JABCDEFGHJKMNPQRSTVWXYZ0";

    pub(crate) fn payload() -> KeyPayload {
        KeyPayload {
            v: 1,
            edition: "pro".into(),
            organizations: Organizations::Up(3),
            key_id: KEY_ID.into(),
            holder: "Contoso IT".into(),
            plan: Plan::Yearly,
            paid_through: 1_822_000_000,
            issued_at: 1_790_000_000,
            signer: trust::TEST_KEY_ID.into(),
        }
    }

    pub(crate) fn valid_key() -> String {
        mint(&payload(), &trust::test_signing_key())
    }

    #[test]
    fn a_valid_key_is_accepted_with_what_it_says() {
        let key = parse(&valid_key()).unwrap();
        assert_eq!(key.payload(), &payload());
        assert_eq!(key.key_id(), KEY_ID);
        assert!(key.text().starts_with(KEY_PREFIX));
    }

    #[test]
    fn spaces_and_line_breaks_from_an_email_are_ignored() {
        let text = valid_key();
        let (a, b) = text.split_at(60);
        let wrapped = format!("  {a}\r\n  {b}\n");
        assert_eq!(parse(&wrapped).unwrap().text(), text);
    }

    /// ADR-119: a key says Pro or Partner, and how many organizations it covers: a number, or
    /// "unlimited". A key that says neither, or zero, is refused.
    #[test]
    fn a_key_says_its_edition_and_how_many_organizations_it_covers() {
        let signer = trust::test_signing_key();
        let key = parse(&valid_key()).unwrap();
        assert_eq!(key.edition(), KeyEdition::Pro);
        assert_eq!(key.organizations(), Some(3));
        let mut p = payload();
        p.edition = "partner".into();
        p.organizations = Organizations::UNLIMITED;
        let partner = parse(&mint(&p, &signer)).unwrap();
        assert_eq!(partner.edition(), KeyEdition::Partner);
        assert_eq!(partner.organizations(), None);
        let json = String::from_utf8(
            codec::decode(
                partner
                    .text()
                    .strip_prefix(KEY_PREFIX)
                    .unwrap()
                    .split('.')
                    .next()
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(json.contains(r#""organizations":"unlimited""#), "{json}");
        p.organizations = Organizations::Up(0);
        assert_eq!(parse(&mint(&p, &signer)), Err(KeyError::Malformed));
        // A key with no number of organizations is refused (never read as unlimited).
        let mut raw = serde_json::to_value(payload()).unwrap();
        raw.as_object_mut().unwrap().remove("organizations");
        let b64 = codec::encode(&serde_json::to_vec(&raw).unwrap());
        let sig = codec::sign(KEY_CONTEXT, &b64, &signer);
        assert_eq!(
            parse(&format!("{KEY_PREFIX}{b64}.{sig}")),
            Err(KeyError::Malformed)
        );
        for word in [r#""Unlimited""#, r#""lots""#, "-1", "3.5"] {
            let mut raw = serde_json::to_value(payload()).unwrap();
            raw["organizations"] = serde_json::from_str(word).unwrap();
            let b64 = codec::encode(&serde_json::to_vec(&raw).unwrap());
            let sig = codec::sign(KEY_CONTEXT, &b64, &signer);
            assert_eq!(
                parse(&format!("{KEY_PREFIX}{b64}.{sig}")),
                Err(KeyError::Malformed),
                "{word}"
            );
        }
    }

    #[test]
    fn a_tampered_payload_is_refused() {
        let text = valid_key();
        let rest = text.strip_prefix(KEY_PREFIX).unwrap();
        let (_, sig) = rest.split_once('.').unwrap();
        let mut changed = payload();
        changed.paid_through += 365 * 24 * 3600;
        let changed_b64 = codec::encode(&serde_json::to_vec(&changed).unwrap());
        let forged = format!("{KEY_PREFIX}{changed_b64}.{sig}");
        assert_eq!(parse(&forged), Err(KeyError::Damaged));
    }

    #[test]
    fn a_key_signed_by_another_key_is_refused() {
        let other = SigningKey::from_bytes(&[9u8; 32]);
        // Claiming the test key's name, signed by another key.
        assert_eq!(parse(&mint(&payload(), &other)), Err(KeyError::Damaged));
        // Naming a key Plenipo does not trust.
        let mut p = payload();
        p.signer = "prod-9".into();
        assert_eq!(parse(&mint(&p, &other)), Err(KeyError::UnknownSigner));
    }

    #[test]
    fn malformed_keys_are_refused_in_plain_words() {
        let good = valid_key();
        for bad in [
            String::new(),
            "hello".into(),
            good.replacen(KEY_PREFIX, "plenipo2.", 1),
            format!("{good}.extra"),
            good.replace('.', ""),
            format!("{KEY_PREFIX}!!!.abc"),
            format!("{KEY_PREFIX}{}.{}", codec::encode(b"not json"), "sig"),
            "x".repeat(MAX_KEY_CHARS + 1),
        ] {
            assert_eq!(parse(&bad), Err(KeyError::Malformed), "{bad}");
        }
        assert!(KeyError::Malformed
            .to_string()
            .contains("Copy the whole key"));
    }

    #[test]
    fn what_a_key_says_is_checked_after_its_signature() {
        let key = trust::test_signing_key();
        let mut p = payload();
        p.edition = "enterprise".into();
        assert_eq!(parse(&mint(&p, &key)), Err(KeyError::NotPro));
        for id in [
            "lk_short",
            "xx_01JABCDEFGHJKMNPQRSTVWXYZ0",
            "lk_01JABCDEFGHJKMNPQRSTVWXYZI",
        ] {
            let mut p = payload();
            p.key_id = id.into();
            assert_eq!(parse(&mint(&p, &key)), Err(KeyError::Malformed), "{id}");
        }
        let mut p = payload();
        p.holder = " ".into();
        assert_eq!(parse(&mint(&p, &key)), Err(KeyError::Malformed));
        let mut p = payload();
        p.holder = "a\u{7}b".into();
        assert_eq!(parse(&mint(&p, &key)), Err(KeyError::Malformed));
        let mut p = payload();
        p.paid_through = p.issued_at - 1;
        assert_eq!(parse(&mint(&p, &key)), Err(KeyError::Malformed));
    }

    #[test]
    fn a_newer_format_asks_for_an_update() {
        let key = trust::test_signing_key();
        let json = serde_json::json!({ "v": 2, "edition": "pro" });
        let b64 = codec::encode(json.to_string().as_bytes());
        let sig = codec::sign(KEY_CONTEXT, &b64, &key);
        assert_eq!(
            parse(&format!("{KEY_PREFIX}{b64}.{sig}")),
            Err(KeyError::Newer)
        );
    }

    #[test]
    fn unknown_fields_are_refused() {
        let key = trust::test_signing_key();
        let mut json = serde_json::to_value(payload()).unwrap();
        json["seats"] = 100.into();
        let b64 = codec::encode(json.to_string().as_bytes());
        let sig = codec::sign(KEY_CONTEXT, &b64, &key);
        assert_eq!(
            parse(&format!("{KEY_PREFIX}{b64}.{sig}")),
            Err(KeyError::Malformed)
        );
    }

    #[test]
    fn the_key_text_never_shows_in_debug_output() {
        let key = parse(&valid_key()).unwrap();
        let shown = format!("{key:?}");
        assert!(shown.contains(KEY_ID));
        assert!(!shown.contains(KEY_PREFIX));
    }

    #[test]
    fn key_ids_are_checked_exactly() {
        assert!(is_key_id(KEY_ID));
        assert!(!is_key_id("lk_01jabcdefghjkmnpqrstvwxyz0"));
        assert!(!is_key_id("lk_01JABCDEFGHJKMNPQRSTVWXYZ"));
        assert!(!is_key_id("LK_01JABCDEFGHJKMNPQRSTVWXYZ0"));
    }
}
