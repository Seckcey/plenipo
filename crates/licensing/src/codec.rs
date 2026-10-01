//! Signed texts: a payload in base64url (no padding) and an Ed25519 signature over a context
//! string followed by that base64 text. Signing the text rather than the JSON means both sides
//! check exactly the bytes that travelled, with no rules about how JSON is written. The context
//! string keeps a license key from ever passing as a weekly answer, and the other way round.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer as _, SigningKey};

use crate::trust;

/// What a license key's signature covers, before its payload.
pub const KEY_CONTEXT: &str = "plenipo-license-key.v1.";
/// What a weekly answer's signature covers, before its payload.
pub const ANSWER_CONTEXT: &str = "plenipo-license-answer.v1.";

/// Why a signed text was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignError {
    /// Not base64url, or not the right length.
    Malformed,
    /// Signed by a key Plenipo does not trust.
    UnknownSigner,
    /// The signature does not match: changed, damaged, or signed by another key.
    BadSignature,
}

pub fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    URL_SAFE_NO_PAD.decode(text).ok()
}

fn message(context: &str, payload_b64: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(context.len() + payload_b64.len());
    m.extend_from_slice(context.as_bytes());
    m.extend_from_slice(payload_b64.as_bytes());
    m
}

/// Check that `signature_b64` is `signer`'s signature over `context` + `payload_b64`.
pub fn verify(
    context: &str,
    payload_b64: &str,
    signature_b64: &str,
    signer: &str,
) -> Result<(), SignError> {
    let bytes = decode(signature_b64).ok_or(SignError::Malformed)?;
    let bytes: [u8; 64] = bytes.try_into().map_err(|_| SignError::Malformed)?;
    let signature = Signature::from_bytes(&bytes);
    let key = trust::find(signer).ok_or(SignError::UnknownSigner)?;
    key.verify_strict(&message(context, payload_b64), &signature)
        .map_err(|_| SignError::BadSignature)
}

/// Sign `context` + `payload_b64` with `key` (the tests, and the contract's examples; 8 West's
/// own keys sign only in the vault).
pub fn sign(context: &str, payload_b64: &str, key: &SigningKey) -> String {
    encode(&key.sign(&message(context, payload_b64)).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_url_safe_without_padding_and_strict() {
        assert_eq!(encode(b"\xfb\xff"), "-_8");
        assert_eq!(decode("-_8"), Some(b"\xfb\xff".to_vec()));
        for bad in ["", "a+b", "a/b", "ab==", "a b", "é"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_signature_covers_its_context_and_its_exact_text() {
        let key = trust::test_signing_key();
        let sig = sign(KEY_CONTEXT, "e30", &key);
        assert_eq!(verify(KEY_CONTEXT, "e30", &sig, trust::TEST_KEY_ID), Ok(()));
        // The same text under the other context, or one byte changed, fails.
        assert_eq!(
            verify(ANSWER_CONTEXT, "e30", &sig, trust::TEST_KEY_ID),
            Err(SignError::BadSignature)
        );
        assert_eq!(
            verify(KEY_CONTEXT, "e31", &sig, trust::TEST_KEY_ID),
            Err(SignError::BadSignature)
        );
        assert_eq!(
            verify(KEY_CONTEXT, "e30", &sig, "prod-9"),
            Err(SignError::UnknownSigner)
        );
        assert_eq!(
            verify(KEY_CONTEXT, "e30", "abc", trust::TEST_KEY_ID),
            Err(SignError::Malformed)
        );
        // Another key's signature, claimed as the test key's.
        let other = SigningKey::from_bytes(&[7u8; 32]);
        let forged = sign(KEY_CONTEXT, "e30", &other);
        assert_eq!(
            verify(KEY_CONTEXT, "e30", &forged, trust::TEST_KEY_ID),
            Err(SignError::BadSignature)
        );
    }
}
