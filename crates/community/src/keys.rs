//! Each PC's own Community keys (ADR-162 §3): an Ed25519 key that signs what this PC sends, and
//! an X25519 key that other PCs seal items for. They are made new each time the PC signs in
//! (contract §2), kept in the Vault, and never shown, logged, or sent: the account service
//! learns only the public halves.

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable as _, Kem as _, Serializable as _};
use serde::{Deserialize, Serialize};

use crate::{b64, CommunityError, Result};

/// The Vault entry that holds a signed-in PC's keys.
pub const KEYS_ID: &str = "plenipo-community-pc";

/// This PC's Community keys.
pub struct PcKeys {
    signing: SigningKey,
    sealing: [u8; 32],
}

/// The keys as the Vault keeps them: one JSON value, base64url inside.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kept {
    v: u8,
    signing: String,
    sealing: String,
}

impl PcKeys {
    /// New keys, from the operating system's random bytes.
    pub fn generate() -> Self {
        Self::from_secrets(crate::random(), crate::random())
    }

    /// Keys from known secrets: an Ed25519 seed and an X25519 private key. For the tests' worked
    /// examples, and for reading the Vault.
    pub fn from_secrets(signing_seed: [u8; 32], sealing_private: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&signing_seed),
            sealing: sealing_private,
        }
    }

    /// The public half of the signing key.
    pub fn signing_public(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// The public half of the sealing key.
    pub fn sealing_public(&self) -> [u8; 32] {
        let private = <X25519HkdfSha256 as hpke::Kem>::PrivateKey::from_bytes(&self.sealing)
            .expect("32 bytes are an X25519 private key");
        let public = X25519HkdfSha256::sk_to_pk(&private);
        let mut out = [0u8; 32];
        out.copy_from_slice(&public.to_bytes());
        out
    }

    /// The signing key's public half as the contract writes it (`signing_key`).
    pub fn signing_key_text(&self) -> String {
        b64::encode(&self.signing_public())
    }

    /// The sealing key's public half as the contract writes it (`sealing_key`).
    pub fn sealing_key_text(&self) -> String {
        b64::encode(&self.sealing_public())
    }

    /// A signature over the ASCII bytes of `context` followed by `text`, as every signature in
    /// the contract is made (a label first, so a signature means nothing anywhere else).
    pub fn sign(&self, context: &str, text: &str) -> [u8; 64] {
        let mut message = Vec::with_capacity(context.len() + text.len());
        message.extend_from_slice(context.as_bytes());
        message.extend_from_slice(text.as_bytes());
        self.signing.sign(&message).to_bytes()
    }

    /// The X25519 private key, for opening seals in this crate only.
    pub(crate) fn sealing_private(&self) -> &[u8; 32] {
        &self.sealing
    }

    /// The keys as the Vault keeps them.
    pub fn write(&self) -> String {
        serde_json::to_string(&Kept {
            v: 1,
            signing: b64::encode(&self.signing.to_bytes()),
            sealing: b64::encode(&self.sealing),
        })
        .expect("the keys always serialize")
    }

    /// Keys read back from the Vault.
    pub fn read(text: &str) -> Result<Self> {
        let unreadable =
            || CommunityError::Invalid("Community's keys on this computer can't be read.".into());
        let kept: Kept = serde_json::from_str(text).map_err(|_| unreadable())?;
        if kept.v != 1 {
            return Err(unreadable());
        }
        let signing = b64::decode_exact::<32>(&kept.signing).ok_or_else(unreadable)?;
        let sealing = b64::decode_exact::<32>(&kept.sealing).ok_or_else(unreadable)?;
        Ok(Self::from_secrets(signing, sealing))
    }
}

impl std::fmt::Debug for PcKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcKeys")
            .field("signing_key", &self.signing_key_text())
            .field("sealing_key", &self.sealing_key_text())
            .finish_non_exhaustive()
    }
}

/// Check a signature made like [`PcKeys::sign`], with a PC's public signing key as the contract
/// writes it. Refuses a key or a signature that is not exactly right.
pub fn verify(signing_key: &str, context: &str, text: &str, signature: &str) -> bool {
    let Some(key) = b64::decode_exact::<32>(signing_key) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else {
        return false;
    };
    let Some(signature) = b64::decode_exact::<64>(signature) else {
        return false;
    };
    let mut message = Vec::with_capacity(context.len() + text.len());
    message.extend_from_slice(context.as_bytes());
    message.extend_from_slice(text.as_bytes());
    key.verify_strict(&message, &ed25519_dalek::Signature::from_bytes(&signature))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_kept_and_read_back_and_never_shown() {
        let keys = PcKeys::generate();
        let kept = keys.write();
        let back = PcKeys::read(&kept).unwrap();
        assert_eq!(back.signing_public(), keys.signing_public());
        assert_eq!(back.sealing_public(), keys.sealing_public());
        let shown = format!("{keys:?}");
        assert!(shown.contains(&keys.signing_key_text()));
        let secret: Kept = serde_json::from_str(&kept).unwrap();
        assert!(
            !shown.contains(&secret.signing),
            "the private halves are never shown"
        );
        assert!(!shown.contains(&secret.sealing));
        assert_ne!(PcKeys::generate().signing_public(), keys.signing_public());
    }

    #[test]
    fn unreadable_keys_are_refused() {
        for bad in [
            "",
            "{}",
            r#"{"v":2,"signing":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","sealing":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
            r#"{"v":1,"signing":"AAAA","sealing":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
            r#"{"v":1,"signing":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","sealing":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","more":1}"#,
        ] {
            assert!(PcKeys::read(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_signature_checks_only_for_its_label_text_and_key() {
        let keys = PcKeys::generate();
        let sig = b64::encode(&keys.sign("plenipo-community-item.v1.", "abc"));
        let key = keys.signing_key_text();
        assert!(verify(&key, "plenipo-community-item.v1.", "abc", &sig));
        assert!(
            !verify(&key, "plenipo-community-stamp.v1.", "abc", &sig),
            "another label"
        );
        assert!(
            !verify(&key, "plenipo-community-item.v1.", "abd", &sig),
            "other words"
        );
        let other = PcKeys::generate().signing_key_text();
        assert!(
            !verify(&other, "plenipo-community-item.v1.", "abc", &sig),
            "another key"
        );
        assert!(!verify(&key, "plenipo-community-item.v1.", "abc", "AAAA"));
    }
}
