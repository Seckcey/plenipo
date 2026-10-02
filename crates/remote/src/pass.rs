//! Relay passes (ADR-143 §4, ADR-147): what lets a paired phone through the relay to its PC.
//!
//! A pass is `<payload>.<signature>`, both base64url: the payload is JSON naming the PC (its
//! relay key's fingerprint), the phone (a random ID), and an end date; the signature is the PC's
//! relay key's, over [`CONTEXT`] followed by the payload's text. The relay checks it with the
//! key the PC showed when it connected. A pass only keeps strangers from reaching the PC: the
//! Noise lock is what keeps them out.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::b64;
use crate::keys::PcKeys;

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

/// A new pass for `phone`, ending `life_secs` from `now`.
pub fn issue(keys: &PcKeys, phone: &str, now: i64, life_secs: i64) -> String {
    let payload = PassPayload {
        v: 1,
        pc: keys.fingerprint(),
        phone: phone.to_owned(),
        exp: now.saturating_add(life_secs),
    };
    let text = b64::encode(&serde_json::to_vec(&payload).expect("a pass is JSON"));
    let sig = keys.relay_sign(CONTEXT, &text);
    format!("{text}.{}", b64::encode(&sig))
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
    if payload.pc != crate::keys::fingerprint(relay_public) {
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

/// The PC a pass names, without checking it (the relay looks the PC up by it first).
pub fn pc_of(pass: &str) -> Option<String> {
    let (text, _) = pass.split_once('.')?;
    let payload: PassPayload = serde_json::from_slice(&b64::decode(text, MAX_PASS)?).ok()?;
    Some(payload.pc)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_791_000_000;

    #[test]
    fn a_pass_lets_its_own_phone_reach_its_own_pc() {
        let keys = PcKeys::new();
        let phone = b64::encode(&[9u8; 16]);
        let pass = issue(&keys, &phone, NOW, 90 * 86_400);
        let ok = check(&pass, &keys.relay_public(), NOW + 1).unwrap();
        assert_eq!(ok.phone, phone);
        assert_eq!(ok.pc, keys.fingerprint());
        assert_eq!(pc_of(&pass), Some(keys.fingerprint()));
        assert!(pass.len() < MAX_PASS, "{}", pass.len());
    }

    #[test]
    fn a_bad_pass_is_refused() {
        let keys = PcKeys::new();
        let other = PcKeys::new();
        let phone = b64::encode(&[9u8; 16]);
        let pass = issue(&keys, &phone, NOW, 60);
        assert_eq!(
            check(&pass, &other.relay_public(), NOW),
            Err(PassError::OtherPc)
        );
        assert_eq!(
            check(&pass, &keys.relay_public(), NOW + 60),
            Err(PassError::Expired)
        );
        // Signed by another PC, but naming this one.
        let forged = issue(&other, &phone, NOW, 60);
        let (_, sig) = forged.split_once('.').unwrap();
        let (text, _) = pass.split_once('.').unwrap();
        assert_eq!(
            check(&format!("{text}.{sig}"), &keys.relay_public(), NOW),
            Err(PassError::BadSignature)
        );
        // Changed on the way: a later end date.
        let mut payload: PassPayload =
            serde_json::from_slice(&b64::decode(text, MAX_PASS).unwrap()).unwrap();
        payload.exp += 1_000_000;
        let changed = b64::encode(&serde_json::to_vec(&payload).unwrap());
        let (_, sig) = pass.split_once('.').unwrap();
        assert_eq!(
            check(&format!("{changed}.{sig}"), &keys.relay_public(), NOW),
            Err(PassError::BadSignature)
        );
        for bad in ["", "x", "a.b", &"a".repeat(600)] {
            assert_eq!(
                check(bad, &keys.relay_public(), NOW),
                Err(PassError::Malformed),
                "{bad}"
            );
        }
    }
}
