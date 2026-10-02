//! Notices on your phone when Plenipo's page is closed (Phase 14 part 14C, ADR-144). Each notice is
//! sealed for one phone with the web's standard sealing (RFC 8291, `aes128gcm`), so only that phone
//! can read it, and signed with the PC's notice key (RFC 8292, VAPID), so a notice service takes it
//! only from this PC. The PC sends it straight to the phone's own notice service (Guard's purpose
//! **phone notices**); the relay never sees it.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes128Gcm, KeyInit};
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::{PublicKey, SecretKey};
use sha2::Sha256;

use crate::b64;
use crate::protocol::Subscription;

/// Each notice is one record of this size (RFC 8188); a notice is far smaller.
const RECORD_SIZE: u32 = 4096;
/// The salt, the record size, the key's length, and the PC's one-time key (65 bytes).
const HEADER: usize = 16 + 4 + 1 + 65;
/// The seal's tag.
const TAG: usize = 16;
/// The most a notice may say: one record, less its padding byte and the seal's tag.
pub const MAX_NOTICE: usize = RECORD_SIZE as usize - HEADER - TAG - 1;
/// Who to ask about this PC's notices (RFC 8292 `sub`): Plenipo's own site, never a person.
pub const CONTACT: &str = "https://getplenipo.com";
/// How long the signature on a notice lasts (a notice service takes at most 24 hours).
const SIGNED_FOR_SECS: i64 = 12 * 60 * 60;

/// Why a notice could not be sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NoticeError {
    #[error("the phone's notice keys are not right")]
    BadKeys,
    #[error("the phone's notice address is not a web address")]
    BadAddress,
    #[error("the notice is too long")]
    TooLong,
}

/// How the notice service should treat it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// `high` for what waits for you (an approval), `normal` otherwise.
    pub urgency: &'static str,
    /// How long the service keeps it for a phone that is off (seconds).
    pub keep_for_secs: u32,
    /// A newer notice with the same topic replaces one not delivered yet (up to 32 letters).
    pub topic: Option<String>,
}

/// A notice, sealed and signed, ready to send: one `POST` to `endpoint`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedNotice {
    pub endpoint: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// Seal `notice` for the phone at `to`, and sign it with the PC's notice key. `now`: Unix seconds.
pub fn seal(
    to: &Subscription,
    notice: &[u8],
    key: &SecretKey,
    now: i64,
    delivery: &Delivery,
) -> Result<SealedNotice, NoticeError> {
    let body = encrypt(to, notice, &one_time_key(), &crate::random::<16>())?;
    let mut headers = vec![
        ("Content-Encoding", "aes128gcm".to_owned()),
        ("Content-Type", "application/octet-stream".to_owned()),
        ("TTL", delivery.keep_for_secs.to_string()),
        ("Urgency", delivery.urgency.to_owned()),
        ("Authorization", vapid(&to.endpoint, key, now)?),
    ];
    if let Some(topic) = &delivery.topic {
        headers.push(("Topic", topic.clone()));
    }
    Ok(SealedNotice {
        endpoint: to.endpoint.clone(),
        headers,
        body,
    })
}

/// A key used for one notice only.
fn one_time_key() -> SecretKey {
    loop {
        if let Ok(key) = SecretKey::from_slice(&crate::random::<32>()) {
            return key;
        }
    }
}

fn expand<const N: usize>(salt: &[u8], ikm: &[u8], info: &[u8]) -> [u8; N] {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut out = [0u8; N];
    hk.expand(info, &mut out).expect("a short output");
    out
}

/// The phone's notice keys: its public key (65 bytes) and its secret (16 bytes).
fn phone_keys(to: &Subscription) -> Result<(PublicKey, Vec<u8>, [u8; 16]), NoticeError> {
    let bytes = b64::decode(&to.p256dh, 65)
        .filter(|k| k.len() == 65)
        .ok_or(NoticeError::BadKeys)?;
    let key = PublicKey::from_sec1_bytes(&bytes).map_err(|_| NoticeError::BadKeys)?;
    let auth = b64::decode_exact::<16>(&to.auth).ok_or(NoticeError::BadKeys)?;
    Ok((key, bytes, auth))
}

/// RFC 8291 §3 and RFC 8188: `plaintext` sealed for the phone's keys, as one record, with the
/// header that carries the salt and the one-time public key.
pub fn encrypt(
    to: &Subscription,
    plaintext: &[u8],
    one_time: &SecretKey,
    salt: &[u8; 16],
) -> Result<Vec<u8>, NoticeError> {
    if plaintext.len() > MAX_NOTICE {
        return Err(NoticeError::TooLong);
    }
    let (phone, phone_bytes, auth) = phone_keys(to)?;
    let ours = one_time.public_key().to_sec1_bytes();
    let shared = p256::ecdh::diffie_hellman(one_time.to_nonzero_scalar(), phone.as_affine());
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&phone_bytes);
    key_info.extend_from_slice(&ours);
    let ikm: [u8; 32] = expand(&auth, shared.raw_secret_bytes().as_slice(), &key_info);
    let cek: [u8; 16] = expand(salt, &ikm, b"Content-Encoding: aes128gcm\0");
    let nonce: [u8; 12] = expand(salt, &ikm, b"Content-Encoding: nonce\0");
    let mut padded = plaintext.to_vec();
    padded.push(2); // the last record
    let sealed = Aes128Gcm::new_from_slice(&cek)
        .expect("a 16-byte key")
        .encrypt(&nonce.into(), padded.as_slice())
        .map_err(|_| NoticeError::BadKeys)?;
    let mut body = Vec::with_capacity(HEADER + sealed.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(65);
    body.extend_from_slice(&ours);
    body.extend_from_slice(&sealed);
    Ok(body)
}

/// RFC 8292: `vapid t=<a token signed with the PC's notice key>, k=<its public key>`, for the
/// notice service at `endpoint`, good for 12 hours from `now` (Unix seconds).
pub fn vapid(endpoint: &str, key: &SecretKey, now: i64) -> Result<String, NoticeError> {
    let url = url::Url::parse(endpoint).map_err(|_| NoticeError::BadAddress)?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err(NoticeError::BadAddress);
    }
    let audience = url.origin().ascii_serialization();
    let header = b64::encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = serde_json::json!({
        "aud": audience,
        "exp": now + SIGNED_FOR_SECS,
        "sub": CONTACT,
    });
    let claims = b64::encode(claims.to_string().as_bytes());
    let signed = format!("{header}.{claims}");
    let signature: p256::ecdsa::Signature =
        p256::ecdsa::SigningKey::from(key).sign(signed.as_bytes());
    Ok(format!(
        "vapid t={signed}.{}, k={}",
        b64::encode(&signature.to_bytes()),
        b64::encode(&key.public_key().to_sec1_bytes())
    ))
}

/// Open a sealed notice with the phone's own keys (what the phone does; the tests and the
/// stand-in phone use it).
pub fn decrypt(body: &[u8], phone: &SecretKey, auth: &[u8; 16]) -> Option<Vec<u8>> {
    let salt = body.get(..16)?;
    if body.get(16..20)? != RECORD_SIZE.to_be_bytes() || *body.get(20)? != 65 {
        return None;
    }
    let theirs_bytes = body.get(21..86)?;
    let theirs = PublicKey::from_sec1_bytes(theirs_bytes).ok()?;
    let mine = phone.public_key().to_sec1_bytes();
    let shared = p256::ecdh::diffie_hellman(phone.to_nonzero_scalar(), theirs.as_affine());
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&mine);
    key_info.extend_from_slice(theirs_bytes);
    let ikm: [u8; 32] = expand(auth, shared.raw_secret_bytes().as_slice(), &key_info);
    let cek: [u8; 16] = expand(salt, &ikm, b"Content-Encoding: aes128gcm\0");
    let nonce: [u8; 12] = expand(salt, &ikm, b"Content-Encoding: nonce\0");
    let mut plain = Aes128Gcm::new_from_slice(&cek)
        .ok()?
        .decrypt(&nonce.into(), &body[HEADER..])
        .ok()?;
    // The padding ends at the last record's delimiter.
    while plain.last() == Some(&0) {
        plain.pop();
    }
    (plain.pop() == Some(2)).then_some(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;

    /// RFC 8291 Appendix A, joined (the RFC wraps its long values).
    const AS_PRIVATE: &str = "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw";
    const UA_PUBLIC: &str =
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
    const UA_PRIVATE: &str = "q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94";
    const SALT: &str = "DGv6ra1nlYgDCS1FRnbzlw";
    const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
    const PLAINTEXT: &str = "When I grow up, I want to be a watermelon";
    const HEADER_B64: &str = "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8";
    const CIPHERTEXT: &str =
        "8pfeW0KbunFT06SuDKoJH9Ql87S1QUrdirN6GcG7sFz1y1sqLgVi1VhjVkHsUoEsbI_0LpXMuGvnzQ";

    fn key(text: &str) -> SecretKey {
        SecretKey::from_slice(&b64::decode_exact::<32>(text).unwrap()).unwrap()
    }

    fn phone() -> Subscription {
        Subscription {
            endpoint: "https://fcm.googleapis.com/fcm/send/abc".into(),
            p256dh: UA_PUBLIC.into(),
            auth: AUTH.into(),
        }
    }

    #[test]
    fn sealing_matches_the_standards_own_example() {
        let salt = b64::decode_exact::<16>(SALT).unwrap();
        let body = encrypt(&phone(), PLAINTEXT.as_bytes(), &key(AS_PRIVATE), &salt).unwrap();
        let mut want = b64::decode(HEADER_B64, 86).unwrap();
        want.extend(b64::decode(CIPHERTEXT, 64).unwrap());
        assert_eq!(body, want);
        // And the phone opens it with its own keys.
        let auth = b64::decode_exact::<16>(AUTH).unwrap();
        let opened = decrypt(&body, &key(UA_PRIVATE), &auth).unwrap();
        assert_eq!(opened, PLAINTEXT.as_bytes());
    }

    #[test]
    fn each_notice_is_sealed_anew_and_only_the_phone_can_open_it() {
        let pc = key(AS_PRIVATE);
        let how = Delivery {
            urgency: "high",
            keep_for_secs: 600,
            topic: Some("approval-a1".into()),
        };
        let a = seal(&phone(), b"Approve: git push", &pc, 1_000, &how).unwrap();
        let b = seal(&phone(), b"Approve: git push", &pc, 1_000, &how).unwrap();
        assert_ne!(a.body, b.body, "a new salt and key each time");
        let auth = b64::decode_exact::<16>(AUTH).unwrap();
        assert_eq!(
            decrypt(&a.body, &key(UA_PRIVATE), &auth).unwrap(),
            b"Approve: git push"
        );
        // Another phone's keys, or another secret, cannot open it.
        assert!(decrypt(&a.body, &key(AS_PRIVATE), &auth).is_none());
        assert!(decrypt(&a.body, &key(UA_PRIVATE), &[0; 16]).is_none());
        // Nothing of what it says is in the clear.
        assert!(!a.body.windows(7).any(|w| w == b"Approve"));
        let header = |name: &str| {
            a.headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(header("Content-Encoding").as_deref(), Some("aes128gcm"));
        assert_eq!(header("TTL").as_deref(), Some("600"));
        assert_eq!(header("Urgency").as_deref(), Some("high"));
        assert_eq!(header("Topic").as_deref(), Some("approval-a1"));
        assert_eq!(a.endpoint, "https://fcm.googleapis.com/fcm/send/abc");
    }

    #[test]
    fn the_signature_is_the_pcs_for_that_notice_service_and_lasts_12_hours() {
        let pc = key(AS_PRIVATE);
        let auth = vapid("https://fcm.googleapis.com/fcm/send/abc", &pc, 1_000).unwrap();
        let (token, public) = auth
            .strip_prefix("vapid t=")
            .and_then(|r| r.split_once(", k="))
            .unwrap();
        assert_eq!(
            public,
            b64::encode(&pc.public_key().to_sec1_bytes()),
            "the PC's notice key, as the phone signed up with it"
        );
        let mut parts = token.split('.');
        let (header, claims, signature) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let claims: serde_json::Value =
            serde_json::from_slice(&b64::decode(claims, 512).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert_eq!(claims["exp"], 1_000 + 12 * 60 * 60);
        assert_eq!(claims["sub"], CONTACT);
        let signature =
            p256::ecdsa::Signature::from_slice(&b64::decode(signature, 64).unwrap()).unwrap();
        let verifying = p256::ecdsa::VerifyingKey::from(pc.public_key());
        assert!(verifying
            .verify(
                format!("{header}.{}", token.split('.').nth(1).unwrap()).as_bytes(),
                &signature
            )
            .is_ok());
        assert_eq!(
            vapid("not an address", &pc, 0),
            Err(NoticeError::BadAddress)
        );
    }

    #[test]
    fn bad_keys_and_long_notices_are_refused() {
        let pc = key(AS_PRIVATE);
        let salt = [1u8; 16];
        let mut bad = phone();
        bad.p256dh = b64::encode(&[4u8; 65]);
        assert_eq!(encrypt(&bad, b"x", &pc, &salt), Err(NoticeError::BadKeys));
        let mut bad = phone();
        bad.auth = b64::encode(&[1u8; 8]);
        assert_eq!(encrypt(&bad, b"x", &pc, &salt), Err(NoticeError::BadKeys));
        let long = vec![b'x'; MAX_NOTICE + 1];
        assert_eq!(
            encrypt(&phone(), &long, &pc, &salt),
            Err(NoticeError::TooLong)
        );
        assert!(encrypt(&phone(), &long[1..], &pc, &salt).is_ok());
    }
}
