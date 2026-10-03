//! Sealing for one PC (ADR-164 §1, contract §7): HPKE (RFC 9180), base mode, single shot, with
//! DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, and AES-256-GCM, to the receiving PC's sealing key.
//! `info` names the item and the receiving PC, so a copy opens only as that item, on that PC.
//! A sealed copy is the one-time public key (32 bytes) followed by the ciphertext.
//!
//! The sealing itself is the `hpke` crate's; `tests/vectors.rs` checks it against the published
//! HPKE test answers for exactly this suite, and against the contract's worked seal, made by a
//! second program.

use hpke::aead::AesGcm256;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::rand_core::{CryptoRng, Infallible, TryCryptoRng, TryRng};
use hpke::{Deserializable as _, OpModeR, OpModeS, Serializable as _};

/// The label every seal's `info` starts with.
pub const SEAL_CONTEXT: &str = "plenipo-community-seal.v1.";
/// The one-time public key at the front of every sealed copy.
pub const ENC_BYTES: usize = 32;
/// AES-GCM's tag at the end.
pub const TAG_BYTES: usize = 16;

type Kem = X25519HkdfSha256;

/// The `info` for one item's copy for one PC: `plenipo-community-seal.v1.<item_id>.<device_id>`.
pub fn info(item_id: &str, device_id: &str) -> Vec<u8> {
    format!("{SEAL_CONTEXT}{item_id}.{device_id}").into_bytes()
}

/// Seal `plaintext` for the PC whose sealing key is `recipient`. `None` only if the key is not
/// one anyone can seal to.
pub fn seal(recipient: &[u8; 32], info: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
    seal_with(recipient, info, &[], plaintext, &mut SystemRandom)
}

/// [`seal`] with `aad` and a given source of randomness: for the published test answers, whose
/// one-time keys are fixed and whose `aad` is not empty. Community's own `aad` is always empty.
pub(crate) fn seal_with(
    recipient: &[u8; 32],
    info: &[u8],
    aad: &[u8],
    plaintext: &[u8],
    random: &mut impl CryptoRng,
) -> Option<Vec<u8>> {
    let recipient = <Kem as hpke::Kem>::PublicKey::from_bytes(recipient).ok()?;
    let (enc, ciphertext) = hpke::single_shot_seal_with_rng::<AesGcm256, HkdfSha256, Kem>(
        &OpModeS::Base,
        &recipient,
        info,
        plaintext,
        aad,
        random,
    )
    .ok()?;
    let mut sealed = Vec::with_capacity(ENC_BYTES + ciphertext.len());
    sealed.extend_from_slice(&enc.to_bytes());
    sealed.extend_from_slice(&ciphertext);
    Some(sealed)
}

/// Open a sealed copy with this PC's sealing key. `None` if it was not sealed for this key and
/// this `info`, or was changed on the way.
pub fn open(private: &[u8; 32], info: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    open_with(private, info, &[], sealed)
}

/// [`open`] with `aad`, for the published test answers.
pub(crate) fn open_with(
    private: &[u8; 32],
    info: &[u8],
    aad: &[u8],
    sealed: &[u8],
) -> Option<Vec<u8>> {
    if sealed.len() < ENC_BYTES + TAG_BYTES {
        return None;
    }
    let (enc, ciphertext) = sealed.split_at(ENC_BYTES);
    let private = <Kem as hpke::Kem>::PrivateKey::from_bytes(private).ok()?;
    let enc = <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).ok()?;
    hpke::single_shot_open::<AesGcm256, HkdfSha256, Kem>(
        &OpModeR::Base,
        &private,
        &enc,
        info,
        ciphertext,
        aad,
    )
    .ok()
}

/// The operating system's random bytes, in the form HPKE asks for.
struct SystemRandom;

impl TryRng for SystemRandom {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        Ok(u32::from_le_bytes(crate::random()))
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        Ok(u64::from_le_bytes(crate::random()))
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        getrandom::fill(dst).expect("the operating system gives random bytes");
        Ok(())
    }
}

impl TryCryptoRng for SystemRandom {}

/// Bytes given ahead of time, as if they were random: the published test answers' one-time
/// keys. Tests only.
#[cfg(test)]
pub(crate) struct Given(pub Vec<u8>);

#[cfg(test)]
impl TryRng for Given {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        unreachable!("HPKE asks only for bytes")
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        unreachable!("HPKE asks only for bytes")
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        assert!(dst.len() <= self.0.len(), "more bytes asked for than given");
        let rest = self.0.split_off(dst.len());
        dst.copy_from_slice(&self.0);
        self.0 = rest;
        Ok(())
    }
}

#[cfg(test)]
impl TryCryptoRng for Given {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::PcKeys;

    #[test]
    fn a_copy_opens_only_on_its_pc_as_its_item() {
        let pat = PcKeys::generate();
        let info_here = info(
            "ci_01JC0D1E2F3G4H5J6K7M8N9P0Q",
            "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7C",
        );
        let sealed = seal(&pat.sealing_public(), &info_here, b"hello").unwrap();
        assert_eq!(sealed.len(), ENC_BYTES + 5 + TAG_BYTES);
        assert_eq!(
            open(pat.sealing_private(), &info_here, &sealed).unwrap(),
            b"hello"
        );

        let other_pc = info(
            "ci_01JC0D1E2F3G4H5J6K7M8N9P0Q",
            "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7D",
        );
        assert_eq!(
            open(pat.sealing_private(), &other_pc, &sealed),
            None,
            "another PC's info"
        );
        let other_item = info(
            "ci_01JC0D1E2F3G4H5J6K7M8N9P0R",
            "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7C",
        );
        assert_eq!(
            open(pat.sealing_private(), &other_item, &sealed),
            None,
            "another item"
        );
        let someone_else = PcKeys::generate();
        assert_eq!(
            open(someone_else.sealing_private(), &info_here, &sealed),
            None,
            "another key"
        );
        for at in [0, ENC_BYTES, sealed.len() - 1] {
            let mut changed = sealed.clone();
            changed[at] ^= 1;
            assert_eq!(
                open(pat.sealing_private(), &info_here, &changed),
                None,
                "byte {at}"
            );
        }
        assert_eq!(
            open(pat.sealing_private(), &info_here, &sealed[..40]),
            None,
            "cut short"
        );
        assert_ne!(
            seal(&pat.sealing_public(), &info_here, b"hello").unwrap(),
            sealed,
            "a fresh one-time key every time"
        );
    }

    #[test]
    fn a_key_nobody_can_seal_to_is_refused() {
        // All zeros is X25519's point of low order: the shared secret would be all zeros.
        assert_eq!(seal(&[0u8; 32], b"info", b"hello"), None);
    }
}
