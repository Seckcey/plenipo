//! The PC's own keys for phone access, kept in the Vault (ADR-141 §8, ADR-143 §2):
//!
//! - its **Noise key** (X25519): the long-term key the phone's lock checks;
//! - its **relay key** (Ed25519): how the relay knows this PC, and what signs its phones' passes;
//! - its **notice key** (P-256): what signs its notices, so only this PC can send them (part
//!   14C, ADR-144 §2);
//! - its **passkey user** (16 random bytes): the "user" a phone's passkey belongs to.
//!
//! Made once, the first time phone access is switched on. Never in a log.

use ed25519_dalek::{Signer as _, SigningKey};
use serde::{Deserialize, Serialize};

use crate::b64;

/// A relay key's fingerprint: SHA-256 of its 32 bytes, base64url (43 characters). Shared with the
/// relay, which knows a PC by it.
pub use plenipo_relay_contract::fingerprint;

/// The keys, as kept.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Kept {
    v: u32,
    noise_private: String,
    noise_public: String,
    relay: String,
    notice: String,
    user: String,
}

/// The PC's keys for phone access.
#[derive(Clone)]
pub struct PcKeys {
    noise_private: [u8; 32],
    noise_public: [u8; 32],
    relay: [u8; 32],
    notice: [u8; 32],
    user: [u8; 16],
}

impl std::fmt::Debug for PcKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcKeys")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl PcKeys {
    /// New keys.
    pub fn new() -> Self {
        let pair = crate::noise::new_keypair();
        Self {
            noise_private: pair.0,
            noise_public: pair.1,
            relay: crate::random(),
            notice: new_notice_key(),
            user: crate::random(),
        }
    }

    /// Fixed keys for the written contract's examples (never used anywhere else).
    #[cfg(test)]
    pub(crate) fn fixed_for_contract(noise_private: [u8; 32], relay: [u8; 32]) -> Self {
        Self {
            noise_public: crate::noise::public_of(&noise_private),
            noise_private,
            relay,
            notice: [7; 32],
            user: [9; 16],
        }
    }

    /// Read kept keys (`None`: not keys Plenipo made).
    pub fn read(text: &str) -> Option<Self> {
        let kept: Kept = serde_json::from_str(text).ok()?;
        if kept.v != 1 {
            return None;
        }
        let keys = Self {
            noise_private: b64::decode_exact(&kept.noise_private)?,
            noise_public: b64::decode_exact(&kept.noise_public)?,
            relay: b64::decode_exact(&kept.relay)?,
            notice: b64::decode_exact(&kept.notice)?,
            user: b64::decode_exact(&kept.user)?,
        };
        // The pair must belong together, and the notice key must be a real key.
        (crate::noise::public_of(&keys.noise_private) == keys.noise_public
            && p256::SecretKey::from_slice(&keys.notice).is_ok())
        .then_some(keys)
    }

    /// The keys, to keep in the Vault.
    pub fn write(&self) -> String {
        serde_json::to_string(&Kept {
            v: 1,
            noise_private: b64::encode(&self.noise_private),
            noise_public: b64::encode(&self.noise_public),
            relay: b64::encode(&self.relay),
            notice: b64::encode(&self.notice),
            user: b64::encode(&self.user),
        })
        .expect("strings are JSON")
    }

    pub fn noise_private(&self) -> &[u8; 32] {
        &self.noise_private
    }

    pub fn noise_public(&self) -> &[u8; 32] {
        &self.noise_public
    }

    fn relay_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.relay)
    }

    /// The relay key's public half.
    pub fn relay_public(&self) -> [u8; 32] {
        self.relay_key().verifying_key().to_bytes()
    }

    /// How the relay knows this PC: the relay key's fingerprint (SHA-256, base64url).
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.relay_public())
    }

    /// Sign `context` followed by `text` with the relay key.
    pub fn relay_sign(&self, context: &str, text: &str) -> [u8; 64] {
        let mut message = Vec::with_capacity(context.len() + text.len());
        message.extend_from_slice(context.as_bytes());
        message.extend_from_slice(text.as_bytes());
        self.relay_key().sign(&message).to_bytes()
    }

    /// The notice key (part 14C).
    pub fn notice_key(&self) -> p256::SecretKey {
        p256::SecretKey::from_slice(&self.notice).expect("checked when made or read")
    }

    /// The notice key's public half, as the phone's page needs it to sign up for notices (the
    /// uncompressed point, base64url).
    pub fn notice_public(&self) -> String {
        let point = self.notice_key().public_key().to_sec1_bytes();
        b64::encode(&point)
    }

    /// The passkeys' user, base64url.
    pub fn user(&self) -> String {
        b64::encode(&self.user)
    }
}

impl Default for PcKeys {
    fn default() -> Self {
        Self::new()
    }
}

/// A new P-256 private key (a random scalar below the group's order).
fn new_notice_key() -> [u8; 32] {
    loop {
        let bytes: [u8; 32] = crate::random();
        if p256::SecretKey::from_slice(&bytes).is_ok() {
            return bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_kept_and_read_back() {
        let keys = PcKeys::new();
        let text = keys.write();
        let read = PcKeys::read(&text).unwrap();
        assert_eq!(read.noise_public(), keys.noise_public());
        assert_eq!(read.fingerprint(), keys.fingerprint());
        assert_eq!(read.notice_public(), keys.notice_public());
        assert_eq!(read.user(), keys.user());
        assert_eq!(keys.fingerprint().len(), 43);
        // Uncompressed P-256 point: 65 bytes.
        assert_eq!(b64::decode(&keys.notice_public(), 65).unwrap().len(), 65);
        // Never in a log.
        let shown = format!("{keys:?}");
        assert!(
            !shown.contains(&b64::encode(keys.noise_private())),
            "{shown}"
        );
    }

    #[test]
    fn broken_or_mixed_keys_are_not_read() {
        let a = PcKeys::new();
        let b = PcKeys::new();
        let mut kept: Kept = serde_json::from_str(&a.write()).unwrap();
        kept.noise_public = b64::encode(b.noise_public());
        assert!(PcKeys::read(&serde_json::to_string(&kept).unwrap()).is_none());
        assert!(PcKeys::read("{}").is_none());
        assert!(PcKeys::read("not json").is_none());
        let mut kept: Kept = serde_json::from_str(&a.write()).unwrap();
        kept.v = 2;
        assert!(PcKeys::read(&serde_json::to_string(&kept).unwrap()).is_none());
    }

    #[test]
    fn the_relay_key_signs() {
        use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
        let keys = PcKeys::new();
        let sig = keys.relay_sign("plenipo-relay-pc.v1.", "nonce");
        let public = VerifyingKey::from_bytes(&keys.relay_public()).unwrap();
        assert!(public
            .verify(b"plenipo-relay-pc.v1.nonce", &Signature::from_bytes(&sig))
            .is_ok());
    }
}
