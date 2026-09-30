//! The weekly check (ADR-022, ADR-116): what Plenipo sends, and the signed answer it trusts.
//!
//! **The request** is exactly `{"key_id":"…","app_version":"…"}`: the key ID and the app version,
//! in that order, with no spaces — and nothing else. A test checks it byte for byte, and the
//! contract (`contracts/license-check/v1`) keeps a copy.
//!
//! **The answer** is `{"answer":"<payload>","signature":"<signature>"}`: the payload is JSON in
//! base64url, and the signature covers [`ANSWER_CONTEXT`] followed by the payload's base64 text,
//! made with 8 West's signing key. An answer that is not signed right counts as a failed check:
//! Pro stays on, and Plenipo tries again later. A bad answer never switches Pro off by itself.

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::codec::{self, SignError, ANSWER_CONTEXT};
use crate::key::is_key_id;

/// The largest answer read.
pub const MAX_ANSWER_BYTES: usize = 4096;
/// The earliest time an answer may give (2026-01-01): anything before is not a real answer.
const EARLIEST: i64 = 1_767_225_600;

#[derive(Serialize)]
struct CheckRequest<'a> {
    key_id: &'a str,
    app_version: &'a str,
}

/// The whole body of the weekly check: the key ID and the app version, and nothing else.
pub fn request_body(key_id: &str, app_version: &str) -> Vec<u8> {
    serde_json::to_vec(&CheckRequest {
        key_id,
        app_version,
    })
    .expect("two strings are JSON")
}

/// What the account service says about a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SubscriptionState {
    /// Paid, or Stripe is still retrying a failed payment.
    Active,
    /// Cancelled: Pro stays until `ends_at`, the end of the paid period.
    Cancelled,
    /// Over: Pro ends now.
    Ended,
    /// The service does not know this key. Not a successful check: Pro stays, and the 30 days go
    /// on counting (fail-open, ADR-022 §3).
    Unknown,
}

/// What a weekly answer says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerPayload {
    /// Format version: 1.
    pub v: u32,
    pub key_id: String,
    pub state: SubscriptionState,
    /// Unix seconds; `None` when unknown.
    pub paid_through: Option<i64>,
    /// When Pro ends (cancelled) or ended (ended), in Unix seconds.
    pub ends_at: Option<i64>,
    /// The service's time when it answered, in Unix seconds: the 30 days count from here.
    pub as_of: i64,
    /// Which signing key signed it.
    pub signer: String,
}

/// An answer as it travelled: kept so Plenipo can check its signature again after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedAnswer {
    pub answer: String,
    pub signature: String,
}

/// Why an answer was not used. Each counts as a failed check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AnswerError {
    #[error("8 West's answer could not be read")]
    Unreadable,
    #[error("8 West's answer was not signed by a key Plenipo trusts")]
    UnknownSigner,
    #[error("8 West's answer was not signed correctly")]
    BadSignature,
    #[error("8 West answered about another key")]
    OtherKey,
}

/// Read, and check, an answer body for the key `key_id`.
pub fn accept(body: &[u8], key_id: &str) -> Result<(SignedAnswer, AnswerPayload), AnswerError> {
    if body.len() > MAX_ANSWER_BYTES {
        return Err(AnswerError::Unreadable);
    }
    let signed: SignedAnswer = serde_json::from_slice(body).map_err(|_| AnswerError::Unreadable)?;
    let payload = verify(&signed)?;
    if payload.key_id != key_id {
        return Err(AnswerError::OtherKey);
    }
    Ok((signed, payload))
}

/// Check a kept answer's signature and what it says.
pub fn verify(signed: &SignedAnswer) -> Result<AnswerPayload, AnswerError> {
    let bytes = codec::decode(&signed.answer).ok_or(AnswerError::Unreadable)?;
    let payload: AnswerPayload =
        serde_json::from_slice(&bytes).map_err(|_| AnswerError::Unreadable)?;
    codec::verify(
        ANSWER_CONTEXT,
        &signed.answer,
        &signed.signature,
        &payload.signer,
    )
    .map_err(|e| match e {
        SignError::Malformed => AnswerError::Unreadable,
        SignError::UnknownSigner => AnswerError::UnknownSigner,
        SignError::BadSignature => AnswerError::BadSignature,
    })?;
    let sensible = payload.v == 1
        && is_key_id(&payload.key_id)
        && payload.as_of >= EARLIEST
        && match payload.state {
            SubscriptionState::Cancelled => payload.ends_at.is_some(),
            _ => true,
        };
    if !sensible {
        return Err(AnswerError::Unreadable);
    }
    Ok(payload)
}

/// Sign an answer (the tests' stand-in service, and the contract's examples).
pub fn sign(payload: &AnswerPayload, key: &SigningKey) -> SignedAnswer {
    let answer = codec::encode(&serde_json::to_vec(payload).expect("an answer is JSON"));
    let signature = codec::sign(ANSWER_CONTEXT, &answer, key);
    SignedAnswer { answer, signature }
}

/// The body the service sends for `payload`.
pub fn body(signed: &SignedAnswer) -> Vec<u8> {
    serde_json::to_vec(signed).expect("an answer is JSON")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::key::tests::KEY_ID;
    use crate::trust;

    pub(crate) const AS_OF: i64 = 1_791_000_000;

    pub(crate) fn answer(state: SubscriptionState) -> AnswerPayload {
        AnswerPayload {
            v: 1,
            key_id: KEY_ID.into(),
            state,
            paid_through: Some(1_822_000_000),
            ends_at: (state == SubscriptionState::Cancelled).then_some(1_792_000_000),
            as_of: AS_OF,
            signer: trust::TEST_KEY_ID.into(),
        }
    }

    pub(crate) fn signed(payload: &AnswerPayload) -> Vec<u8> {
        body(&sign(payload, &trust::test_signing_key()))
    }

    #[test]
    fn the_check_sends_the_key_id_and_the_app_version_and_nothing_else() {
        assert_eq!(
            request_body(KEY_ID, "1.18.0"),
            br#"{"key_id":"lk_01JABCDEFGHJKMNPQRSTVWXYZ0","app_version":"1.18.0"}"#
        );
    }

    #[test]
    fn each_state_is_accepted_when_signed_by_a_trusted_key() {
        for state in [
            SubscriptionState::Active,
            SubscriptionState::Cancelled,
            SubscriptionState::Ended,
            SubscriptionState::Unknown,
        ] {
            let p = answer(state);
            let (kept, got) = accept(&signed(&p), KEY_ID).unwrap();
            assert_eq!(got, p);
            assert_eq!(verify(&kept).unwrap(), p);
        }
    }

    #[test]
    fn an_answer_not_signed_right_is_refused() {
        let other = SigningKey::from_bytes(&[3u8; 32]);
        let p = answer(SubscriptionState::Active);
        assert_eq!(
            accept(&body(&sign(&p, &other)), KEY_ID),
            Err(AnswerError::BadSignature)
        );
        let mut p2 = p.clone();
        p2.signer = "prod-9".into();
        assert_eq!(
            accept(&body(&sign(&p2, &other)), KEY_ID),
            Err(AnswerError::UnknownSigner)
        );
        // A changed answer under the original signature.
        let mut s = sign(&p, &trust::test_signing_key());
        let mut ended = p.clone();
        ended.state = SubscriptionState::Ended;
        s.answer = codec::encode(&serde_json::to_vec(&ended).unwrap());
        assert_eq!(accept(&body(&s), KEY_ID), Err(AnswerError::BadSignature));
    }

    #[test]
    fn a_license_key_never_passes_as_an_answer() {
        // Signed with the key's context, not the answer's.
        let p = answer(SubscriptionState::Active);
        let b64 = codec::encode(&serde_json::to_vec(&p).unwrap());
        let sig = codec::sign(codec::KEY_CONTEXT, &b64, &trust::test_signing_key());
        let s = SignedAnswer {
            answer: b64,
            signature: sig,
        };
        assert_eq!(accept(&body(&s), KEY_ID), Err(AnswerError::BadSignature));
    }

    #[test]
    fn garbage_and_answers_about_another_key_are_refused() {
        for bad in [
            b"".as_slice(),
            b"<html>502 Bad Gateway</html>",
            b"{}",
            br#"{"answer":"x","signature":"y"}"#,
            br#"{"answer":"e30","signature":"e30","extra":1}"#,
        ] {
            assert!(accept(bad, KEY_ID).is_err());
        }
        assert_eq!(
            accept(&vec![b' '; MAX_ANSWER_BYTES + 1], KEY_ID),
            Err(AnswerError::Unreadable)
        );
        assert_eq!(
            accept(
                &signed(&answer(SubscriptionState::Active)),
                "lk_0000000000000000000000000X"
            ),
            Err(AnswerError::OtherKey)
        );
    }

    #[test]
    fn a_cancelled_answer_must_say_when_pro_ends() {
        let mut p = answer(SubscriptionState::Cancelled);
        p.ends_at = None;
        assert_eq!(accept(&signed(&p), KEY_ID), Err(AnswerError::Unreadable));
    }

    #[test]
    fn an_answer_from_before_2026_is_not_real() {
        let mut p = answer(SubscriptionState::Active);
        p.as_of = 1_000;
        assert_eq!(accept(&signed(&p), KEY_ID), Err(AnswerError::Unreadable));
    }
}
