//! The phone relay's contract, in code (`contracts/phone-relay/v1`; ADR-143, the relay and the
//! lock; ADR-149, Plenipo runs its own relay). Plenipo is made by 8 West Ventures, LLC.
//!
//! Both sides of the relay use this one crate, so they can never drift apart:
//!
//! - the PC (`plenipo-remote`) writes and reads the relay's messages, and issues passes;
//! - Plenipo's relay (`plenipo-relay`) reads and writes the same messages, and checks passes.
//!
//! It holds only what the contract names: the relay's messages and short codes ([`wire`]),
//! base64url without padding ([`b64`]), a phone's pass ([`pass`]), and a PC's fingerprint
//! ([`fingerprint`]). Nothing here knows the lock (Noise), the phone's page, or any secret. The
//! written contract's JSON files are made from the PC's side (`plenipo-remote`'s `contract` test)
//! and checked from the relay's.

pub mod b64;
pub mod pass;
pub mod wire;

use sha2::{Digest as _, Sha256};

/// A relay key's fingerprint, how the relay knows a PC: SHA-256 of the Ed25519 public key's 32
/// bytes, base64url (43 characters).
pub fn fingerprint(relay_public: &[u8; 32]) -> String {
    b64::encode(&Sha256::digest(relay_public))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_43_characters_of_base64url() {
        let f = fingerprint(&[7u8; 32]);
        assert_eq!(f.len(), 43);
        assert!(b64::decode_exact::<32>(&f).is_some());
        assert_ne!(f, fingerprint(&[8u8; 32]));
    }
}
