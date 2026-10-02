//! Relay passes (ADR-143 §4, ADR-147): what lets a paired phone through the relay to its PC, as
//! the relay checks them. The PC issues them (`plenipo-remote`'s `pass::issue`).
//!
//! A pass is `<payload>.<signature>`, both base64url: the payload is JSON naming the PC (its
//! relay key's fingerprint), the phone (a random ID), and an end date; the signature is the PC's
//! relay key's, over [`CONTEXT`] followed by the payload's text. The relay checks it with the key
//! the PC showed when it connected. A pass only keeps strangers from reaching the PC: the Noise
//! lock is what keeps them out.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::b64;

/// What a pass's signature covers, before the payload.
pub const CONTEXT: &str = "plenipo-relay-pass.v1.";
/// The longest pass accepted.
pub const MAX_PASS: usize = 512;

/// What a pass says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassPayload {
    pub v: u32,
    /// The PC: its relay key's fingerprint.
    pub pc: String,
    /// The phone: a random ID (16 bytes, base64url).
    pub phone: String,
    /// When it stops working, Unix seconds.
    pub exp: i64,
}

/// Why a pass is not good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassError {
    Malformed,
    OtherPc,
    BadSignature,
    Expired,
}

/// Check a pass against the PC's relay key (what the relay does), at `now`.
pub fn check(pass: &str, relay_public: &[u8; 32], now: i64) -> Result<PassPayload, PassError> {
    if pass.len() > MAX_PASS {
        return Err(PassError::Malformed);
    }
    let (text, sig) = pass.split_once('.').ok_or(PassError::Malformed)?;
    let payload: PassPayload = b64::decode(text, MAX_PASS)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(PassError::Malformed)?;
    if payload.v != 1 || !b64::is_id(&payload.phone, 16) {
        return Err(PassError::Malformed);
    }
    if payload.pc != crate::fingerprint(relay_public) {
        return Err(PassError::OtherPc);
    }
    let sig: [u8; 64] = b64::decode_exact(sig).ok_or(PassError::Malformed)?;
    let key = VerifyingKey::from_bytes(relay_public).map_err(|_| PassError::BadSignature)?;
    let mut message = Vec::with_capacity(CONTEXT.len() + text.len());
    message.extend_from_slice(CONTEXT.as_bytes());
    message.extend_from_slice(text.as_bytes());
    key.verify_strict(&message, &Signature::from_bytes(&sig))
        .map_err(|_| PassError::BadSignature)?;
    if payload.exp <= now {
        return Err(PassError::Expired);
    }
    Ok(payload)
}

/// The PC a pass names, without checking it (the relay looks the PC up by it first). Anything
/// that is not a pass of a sensible size says nothing.
pub fn pc_of(pass: &str) -> Option<String> {
    if pass.len() > MAX_PASS {
        return None;
    }
    let (text, _) = pass.split_once('.')?;
    let payload: PassPayload = serde_json::from_slice(&b64::decode(text, MAX_PASS)?).ok()?;
    Some(payload.pc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    const NOW: i64 = 1_791_000_000;

    /// Sign a pass the way the PC does (`plenipo-remote`'s `pass::issue`).
    fn issue(key: &SigningKey, phone: &str, exp: i64) -> String {
        let payload = PassPayload {
            v: 1,
            pc: crate::fingerprint(&key.verifying_key().to_bytes()),
            phone: phone.to_owned(),
            exp,
        };
        let text = b64::encode(&serde_json::to_vec(&payload).unwrap());
        let sig = key.sign(format!("{CONTEXT}{text}").as_bytes()).to_bytes();
        format!("{text}.{}", b64::encode(&sig))
    }

    #[test]
    fn a_pass_checks_with_its_own_pcs_key_only() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let other = SigningKey::from_bytes(&[2u8; 32]);
        let phone = b64::encode(&[9u8; 16]);
        let pass = issue(&key, &phone, NOW + 60);
        let ok = check(&pass, &key.verifying_key().to_bytes(), NOW).unwrap();
        assert_eq!(ok.phone, phone);
        assert_eq!(pc_of(&pass).as_deref(), Some(ok.pc.as_str()));
        assert_eq!(
            check(&pass, &other.verifying_key().to_bytes(), NOW),
            Err(PassError::OtherPc)
        );
        assert_eq!(
            check(&pass, &key.verifying_key().to_bytes(), NOW + 60),
            Err(PassError::Expired)
        );
        // Signed by another key, but naming this PC.
        let (text, _) = pass.split_once('.').unwrap();
        let forged = issue(&other, &phone, NOW + 60);
        let (_, sig) = forged.split_once('.').unwrap();
        assert_eq!(
            check(
                &format!("{text}.{sig}"),
                &key.verifying_key().to_bytes(),
                NOW
            ),
            Err(PassError::BadSignature)
        );
        for bad in ["", "x", "a.b", &"a".repeat(600)] {
            assert_eq!(
                check(bad, &key.verifying_key().to_bytes(), NOW),
                Err(PassError::Malformed),
                "{bad}"
            );
            assert!(pc_of(bad).is_none() || bad == "a.b", "{bad}");
        }
        assert!(pc_of(&"a".repeat(600)).is_none());
    }
}
