//! Relay passes (ADR-143 §4, ADR-147): what lets a paired phone through the relay to its PC.
//!
//! A pass is `<payload>.<signature>`, both base64url: the payload is JSON naming the PC (its
//! relay key's fingerprint), the phone (a random ID), and an end date; the signature is the PC's
//! relay key's, over [`CONTEXT`] followed by the payload's text. The relay checks it with the
//! key the PC showed when it connected ([`check`], shared with the relay through
//! `plenipo-relay-contract`). A pass only keeps strangers from reaching the PC: the Noise lock is
//! what keeps them out.

use crate::b64;
use crate::keys::PcKeys;

pub use plenipo_relay_contract::pass::{check, pc_of, PassError, PassPayload, CONTEXT, MAX_PASS};

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
