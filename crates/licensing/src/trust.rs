//! The signing keys Plenipo trusts (ADR-104, the signing key in AWS KMS).
//!
//! 8 West's signing keys live in a cloud key vault and never leave it. Plenipo carries only their
//! public halves: the key in use and one spare, so the key can be replaced with an ordinary
//! update. A license key or a weekly answer names the key that signed it (`signer`); anything
//! signed by a key not listed here is refused.
//!
//! Copies of Plenipo built for the tests (the `test-keys` feature, and this crate's own tests)
//! also trust the contract's test key. Release builds never do.

use ed25519_dalek::VerifyingKey;

/// One public key Plenipo trusts, by the name a signed text gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedKey {
    pub id: &'static str,
    pub public: [u8; 32],
}

/// 8 West's production signing keys (AWS KMS, `alias/plenipo-license-current` and
/// `alias/plenipo-license-spare`), their public halves only.
pub const PRODUCTION: &[TrustedKey] = &[];

/// The contract's test signing key: its private half is published in
/// `contracts/license-check/v1/test-signing-key.json` on purpose, so both sides can sign and
/// check the examples. It is trusted only by copies built for the tests.
pub const TEST_KEY_ID: &str = "test-1";

/// The test key's private half (32 bytes). Public on purpose: never trusted by a release build.
pub const TEST_SEED: [u8; 32] = *b"plenipo-contract-test-signing-k1";

/// Whether this copy of Plenipo was built for the tests.
pub const fn built_for_tests() -> bool {
    cfg!(any(test, feature = "test-keys"))
}

/// The public key named `id`, if Plenipo trusts it.
pub fn find(id: &str) -> Option<VerifyingKey> {
    if let Some(k) = PRODUCTION.iter().find(|k| k.id == id) {
        return VerifyingKey::from_bytes(&k.public).ok();
    }
    if built_for_tests() && id == TEST_KEY_ID {
        return Some(test_signing_key().verifying_key());
    }
    None
}

/// The contract's test signing key (tests only).
pub fn test_signing_key() -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&TEST_SEED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_production_key_is_a_real_public_key() {
        for k in PRODUCTION {
            assert!(
                VerifyingKey::from_bytes(&k.public).is_ok(),
                "{} is not a public key",
                k.id
            );
            assert_ne!(k.id, TEST_KEY_ID);
        }
        let mut ids: Vec<_> = PRODUCTION.iter().map(|k| k.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), PRODUCTION.len(), "each key has its own name");
    }

    #[test]
    fn the_test_key_is_trusted_only_in_copies_built_for_the_tests() {
        // This crate's own tests are such a copy.
        assert!(built_for_tests());
        assert_eq!(find(TEST_KEY_ID), Some(test_signing_key().verifying_key()));
        assert!(find("nobody").is_none());
    }
}
