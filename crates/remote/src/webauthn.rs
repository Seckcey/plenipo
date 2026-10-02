//! Passkeys (ADR-142): the phone proves it is the owner with its face, fingerprint, or passcode,
//! and the PC checks the answer itself — not the relay, and not 8 West.
//!
//! This is the small part of WebAuthn Plenipo needs: a platform passkey made with the person
//! checked (`userVerification: "required"`), its public key taken from the browser
//! (`getPublicKey()`, so no attestation is read), and sign-in answers checked for the address
//! they came from, the site, the challenge, the signature, the flags that say the person was
//! there and was checked, and a signature counter that never goes backwards (a counter that stays
//! 0, common for synced passkeys, is allowed).

use p256::ecdsa::signature::Verifier as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use ts_rs::TS;

use crate::b64;

/// WebAuthn's name for ECDSA with P-256 and SHA-256.
pub const ES256: i64 = -7;
/// WebAuthn's name for Ed25519.
pub const EDDSA: i64 = -8;

/// The person was there.
const UP: u8 = 0x01;
/// The person was checked (face, fingerprint, or passcode).
const UV: u8 = 0x04;
/// A new passkey's own data follows.
const AT: u8 = 0x40;

/// The DER start of a P-256 public key (SubjectPublicKeyInfo), before its 65-byte point.
const P256_SPKI: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];
/// The DER start of an Ed25519 public key, before its 32 bytes.
const ED25519_SPKI: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// The most bytes read from any one field.
const MAX_FIELD: usize = 4096;

/// A new passkey, as the phone's page sends it after `navigator.credentials.create()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct NewPasskey {
    /// The credential's ID (base64url).
    pub id: String,
    /// `response.getPublicKey()`: the public key, DER (base64url).
    pub public_key: String,
    /// `response.getPublicKeyAlgorithm()`: -7 (P-256) or -8 (Ed25519).
    #[ts(type = "number")]
    pub algorithm: i64,
    /// `response.getAuthenticatorData()` (base64url).
    pub authenticator_data: String,
    /// `response.clientDataJSON` (base64url).
    pub client_data: String,
}

/// A sign-in answer, as the phone's page sends it after `navigator.credentials.get()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PasskeyAnswer {
    pub id: String,
    pub authenticator_data: String,
    pub client_data: String,
    pub signature: String,
}

/// A phone's passkey, as the PC keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Passkey {
    /// The credential's ID (base64url).
    pub id: String,
    pub algorithm: i64,
    /// The public key: P-256's uncompressed point, or Ed25519's 32 bytes (base64url).
    pub public: String,
    /// The last signature counter seen.
    pub counter: u32,
}

/// What the answer must be for.
#[derive(Debug, Clone, Copy)]
pub struct Expect<'a> {
    /// The page's address (`https://remote.getplenipo.com`).
    pub origin: &'a str,
    /// The passkeys' site (`remote.getplenipo.com`).
    pub rp_id: &'a str,
    /// The challenge the PC gave.
    pub challenge: &'a [u8],
}

/// Why a passkey or an answer was refused (each in plain words).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasskeyError {
    #[error("the phone's answer could not be read")]
    Unreadable,
    #[error("the phone's answer came from another page")]
    WrongPage,
    #[error("the phone's answer was for another check")]
    WrongChallenge,
    #[error("the phone did not check that it is you (face, fingerprint, or passcode)")]
    NotChecked,
    #[error("the phone's answer was not signed by its passkey")]
    BadSignature,
    #[error("the phone's answer is older than one already used")]
    CounterWentBack,
    #[error("that is not this phone's passkey")]
    OtherPasskey,
    #[error("this kind of passkey is not one Plenipo uses")]
    UnknownKind,
}

/// The parts of `authenticatorData` Plenipo reads.
struct AuthData<'a> {
    rp_id_hash: &'a [u8],
    flags: u8,
    counter: u32,
    /// The new credential's ID (only when made).
    credential: Option<&'a [u8]>,
}

fn auth_data(bytes: &[u8]) -> Result<AuthData<'_>, PasskeyError> {
    if bytes.len() < 37 {
        return Err(PasskeyError::Unreadable);
    }
    let flags = bytes[32];
    let counter = u32::from_be_bytes(bytes[33..37].try_into().expect("four bytes"));
    let credential = if flags & AT != 0 {
        // aaguid (16), then the ID's length (2), then the ID.
        let len_at = 37 + 16;
        let len = usize::from(u16::from_be_bytes(
            bytes
                .get(len_at..len_at + 2)
                .ok_or(PasskeyError::Unreadable)?
                .try_into()
                .expect("two bytes"),
        ));
        let start = len_at + 2;
        Some(
            bytes
                .get(start..start + len)
                .ok_or(PasskeyError::Unreadable)?,
        )
    } else {
        None
    };
    Ok(AuthData {
        rp_id_hash: &bytes[..32],
        flags,
        counter,
        credential,
    })
}

/// Check `clientDataJSON`: its kind, challenge, and page.
fn client_data(bytes: &[u8], kind: &str, expect: &Expect<'_>) -> Result<(), PasskeyError> {
    let json: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| PasskeyError::Unreadable)?;
    let field = |name: &str| json.get(name).and_then(serde_json::Value::as_str);
    if field("type") != Some(kind) {
        return Err(PasskeyError::Unreadable);
    }
    if field("origin") != Some(expect.origin)
        || json.get("crossOrigin").and_then(serde_json::Value::as_bool) == Some(true)
    {
        return Err(PasskeyError::WrongPage);
    }
    let challenge = field("challenge")
        .and_then(|c| b64::decode(c, MAX_FIELD))
        .ok_or(PasskeyError::Unreadable)?;
    if challenge != expect.challenge {
        return Err(PasskeyError::WrongChallenge);
    }
    Ok(())
}

fn field(text: &str) -> Result<Vec<u8>, PasskeyError> {
    b64::decode(text, MAX_FIELD).ok_or(PasskeyError::Unreadable)
}

fn site_and_flags(data: &AuthData<'_>, expect: &Expect<'_>) -> Result<(), PasskeyError> {
    if data.rp_id_hash != Sha256::digest(expect.rp_id.as_bytes()).as_slice() {
        return Err(PasskeyError::WrongPage);
    }
    if data.flags & UP == 0 || data.flags & UV == 0 {
        return Err(PasskeyError::NotChecked);
    }
    Ok(())
}

/// Check a new passkey, made for the PC's challenge, and keep what the PC needs of it.
pub fn register(new: &NewPasskey, expect: &Expect<'_>) -> Result<Passkey, PasskeyError> {
    let id = field(&new.id)?;
    let auth = field(&new.authenticator_data)?;
    client_data(&field(&new.client_data)?, "webauthn.create", expect)?;
    let data = auth_data(&auth)?;
    site_and_flags(&data, expect)?;
    if data.credential != Some(id.as_slice()) {
        return Err(PasskeyError::OtherPasskey);
    }
    let der = field(&new.public_key)?;
    let public = match new.algorithm {
        ES256 => {
            let point = der
                .strip_prefix(&P256_SPKI[..])
                .ok_or(PasskeyError::UnknownKind)?;
            p256::ecdsa::VerifyingKey::from_sec1_bytes(point)
                .map_err(|_| PasskeyError::UnknownKind)?;
            point.to_vec()
        }
        EDDSA => {
            let key: [u8; 32] = der
                .strip_prefix(&ED25519_SPKI[..])
                .and_then(|k| k.try_into().ok())
                .ok_or(PasskeyError::UnknownKind)?;
            ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| PasskeyError::UnknownKind)?;
            key.to_vec()
        }
        _ => return Err(PasskeyError::UnknownKind),
    };
    Ok(Passkey {
        id: b64::encode(&id),
        algorithm: new.algorithm,
        public: b64::encode(&public),
        counter: data.counter,
    })
}

/// Check a sign-in answer against the phone's passkey. The new counter, to keep.
pub fn check(
    passkey: &Passkey,
    answer: &PasskeyAnswer,
    expect: &Expect<'_>,
) -> Result<u32, PasskeyError> {
    if field(&answer.id)? != field(&passkey.id)? {
        return Err(PasskeyError::OtherPasskey);
    }
    let auth = field(&answer.authenticator_data)?;
    let client = field(&answer.client_data)?;
    client_data(&client, "webauthn.get", expect)?;
    let data = auth_data(&auth)?;
    site_and_flags(&data, expect)?;
    let mut signed = auth.clone();
    signed.extend_from_slice(&Sha256::digest(&client));
    let signature = field(&answer.signature)?;
    let public = field(&passkey.public)?;
    match passkey.algorithm {
        ES256 => {
            let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(&public)
                .map_err(|_| PasskeyError::UnknownKind)?;
            let sig = p256::ecdsa::Signature::from_der(&signature)
                .map_err(|_| PasskeyError::BadSignature)?;
            // An ECDSA signature and its "low-s" twin are both right; check the low one.
            let sig = sig.normalize_s();
            key.verify(&signed, &sig)
                .map_err(|_| PasskeyError::BadSignature)?;
        }
        EDDSA => {
            let key: [u8; 32] = public.try_into().map_err(|_| PasskeyError::UnknownKind)?;
            let key = ed25519_dalek::VerifyingKey::from_bytes(&key)
                .map_err(|_| PasskeyError::UnknownKind)?;
            let sig: [u8; 64] = signature
                .try_into()
                .map_err(|_| PasskeyError::BadSignature)?;
            key.verify_strict(&signed, &ed25519_dalek::Signature::from_bytes(&sig))
                .map_err(|_| PasskeyError::BadSignature)?;
        }
        _ => return Err(PasskeyError::UnknownKind),
    }
    if (data.counter != 0 || passkey.counter != 0) && data.counter <= passkey.counter {
        return Err(PasskeyError::CounterWentBack);
    }
    Ok(data.counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stand_in::Authenticator;

    const ORIGIN: &str = "https://remote.getplenipo.com";
    const RP: &str = "remote.getplenipo.com";

    fn expect(challenge: &[u8]) -> Expect<'_> {
        Expect {
            origin: ORIGIN,
            rp_id: RP,
            challenge,
        }
    }

    #[test]
    fn a_passkey_is_made_and_signs_in() {
        for algorithm in [ES256, EDDSA] {
            let mut auth = Authenticator::new(algorithm, ORIGIN, RP);
            let made = auth.create(b"challenge-1");
            let passkey = register(&made, &expect(b"challenge-1")).unwrap();
            assert_eq!(passkey.algorithm, algorithm);
            let answer = auth.get(b"challenge-2");
            let counter = check(&passkey, &answer, &expect(b"challenge-2")).unwrap();
            assert!(counter > passkey.counter);
        }
    }

    #[test]
    fn a_wrong_answer_is_refused() {
        let mut auth = Authenticator::new(ES256, ORIGIN, RP);
        let passkey = register(&auth.create(b"c"), &expect(b"c")).unwrap();
        // Another challenge.
        let answer = auth.get(b"one");
        assert_eq!(
            check(&passkey, &answer, &expect(b"two")),
            Err(PasskeyError::WrongChallenge)
        );
        // Another page.
        let mut other = Authenticator::new(ES256, "https://evil.example", RP);
        other.adopt(&auth);
        assert_eq!(
            check(&passkey, &other.get(b"x"), &expect(b"x")),
            Err(PasskeyError::WrongPage)
        );
        // Another site (relying party).
        let mut other = Authenticator::new(ES256, ORIGIN, "evil.example");
        other.adopt(&auth);
        assert_eq!(
            check(&passkey, &other.get(b"x"), &expect(b"x")),
            Err(PasskeyError::WrongPage)
        );
        // The person was not checked.
        auth.verify_user = false;
        assert_eq!(
            check(&passkey, &auth.get(b"x"), &expect(b"x")),
            Err(PasskeyError::NotChecked)
        );
        auth.verify_user = true;
        // Signed by another passkey with the same ID.
        let mut stranger = Authenticator::new(ES256, ORIGIN, RP);
        stranger.id = auth.id.clone();
        assert_eq!(
            check(&passkey, &stranger.get(b"x"), &expect(b"x")),
            Err(PasskeyError::BadSignature)
        );
        // Changed on the way: the signature no longer covers it.
        let mut answer = auth.get(b"x");
        let mut data = b64::decode(&answer.authenticator_data, 4096).unwrap();
        data[33] ^= 0x10;
        answer.authenticator_data = b64::encode(&data);
        assert_eq!(
            check(&passkey, &answer, &expect(b"x")),
            Err(PasskeyError::BadSignature)
        );
        // Another passkey's ID.
        let mut answer = auth.get(b"x");
        answer.id = b64::encode(b"another");
        assert_eq!(
            check(&passkey, &answer, &expect(b"x")),
            Err(PasskeyError::OtherPasskey)
        );
    }

    #[test]
    fn the_counter_never_goes_back_unless_it_stays_zero() {
        let mut auth = Authenticator::new(ES256, ORIGIN, RP);
        let mut passkey = register(&auth.create(b"c"), &expect(b"c")).unwrap();
        let old = auth.get(b"x");
        let newer = auth.get(b"x");
        passkey.counter = check(&passkey, &newer, &expect(b"x")).unwrap();
        assert_eq!(
            check(&passkey, &old, &expect(b"x")),
            Err(PasskeyError::CounterWentBack)
        );
        // A passkey that never counts (synced).
        let mut synced = Authenticator::new(ES256, ORIGIN, RP);
        synced.counts = false;
        let mut passkey = register(&synced.create(b"c"), &expect(b"c")).unwrap();
        assert_eq!(passkey.counter, 0);
        passkey.counter = check(&passkey, &synced.get(b"y"), &expect(b"y")).unwrap();
        assert_eq!(passkey.counter, 0);
        assert!(check(&passkey, &synced.get(b"z"), &expect(b"z")).is_ok());
    }

    #[test]
    fn a_new_passkey_must_be_checked_and_for_this_page() {
        let mut auth = Authenticator::new(ES256, ORIGIN, RP);
        auth.verify_user = false;
        assert_eq!(
            register(&auth.create(b"c"), &expect(b"c")),
            Err(PasskeyError::NotChecked)
        );
        let mut auth = Authenticator::new(ES256, ORIGIN, RP);
        let mut made = auth.create(b"c");
        assert_eq!(
            register(&made, &expect(b"d")),
            Err(PasskeyError::WrongChallenge)
        );
        made.algorithm = -257; // RS256: not a kind Plenipo uses.
        assert_eq!(
            register(&made, &expect(b"c")),
            Err(PasskeyError::UnknownKind)
        );
        let mut made = auth.create(b"c");
        made.id = b64::encode(b"not the one made");
        assert_eq!(
            register(&made, &expect(b"c")),
            Err(PasskeyError::OtherPasskey)
        );
        // A sign-in answer is not a new passkey.
        let answer = auth.get(b"c");
        let as_new = NewPasskey {
            id: answer.id,
            public_key: made.public_key,
            algorithm: ES256,
            authenticator_data: answer.authenticator_data,
            client_data: answer.client_data,
        };
        assert!(register(&as_new, &expect(b"c")).is_err());
    }
}
