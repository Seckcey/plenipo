//! 8 West's stamp on each sealed item (ADR-164 §6, contract §6): `<payload>.<signature>`, both
//! base64url. The payload says who sent what kind of item to whom, when 8 West took it, and its
//! report tag; the signature is the account service's own stamping key's, never the license
//! signing key. A PC keeps each item's stamp, so the person can report the item with a proof.
//!
//! A PC does not hold 8 West's stamping keys and does not check the signature: the account
//! service does, when a report comes in (contract §11). A PC checks that the stamp's payload
//! says the same as the item, so that what it keeps can be reported.

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};

use crate::b64;
use crate::wire::StampPayload;

/// The label the stamping key signs under.
pub const STAMP_CONTEXT: &str = "plenipo-community-stamp.v1.";
/// The longest stamp payload, as base64url text (the contract's schema).
const MOST_PAYLOAD_TEXT: usize = 1000;

/// A stamp's payload, read but not checked: what the stamp says.
pub fn read(stamp: &str) -> Option<StampPayload> {
    let (payload, signature) = stamp.split_once('.')?;
    if payload.is_empty() || payload.len() > MOST_PAYLOAD_TEXT {
        return None;
    }
    b64::decode_exact::<64>(signature)?;
    let bytes = b64::decode(payload, MOST_PAYLOAD_TEXT)?;
    let read: StampPayload = serde_json::from_slice(&bytes).ok()?;
    (read.v == 1).then_some(read)
}

/// A stamp's payload, if the stamping key whose public half is `key` signed it.
pub fn check(stamp: &str, key: &VerifyingKey) -> Option<StampPayload> {
    let (payload, signature) = stamp.split_once('.')?;
    let signature = b64::decode_exact::<64>(signature)?;
    let mut message = Vec::with_capacity(STAMP_CONTEXT.len() + payload.len());
    message.extend_from_slice(STAMP_CONTEXT.as_bytes());
    message.extend_from_slice(payload.as_bytes());
    key.verify_strict(&message, &ed25519_dalek::Signature::from_bytes(&signature))
        .ok()?;
    read(stamp)
}

/// Make a stamp, as the account service does: for the stand-in service in the tests, with the
/// contract's published test stamping key.
pub fn make(payload: &StampPayload, key: &SigningKey) -> String {
    let text = b64::encode(&serde_json::to_vec(payload).expect("a stamp always serializes"));
    let mut message = Vec::with_capacity(STAMP_CONTEXT.len() + text.len());
    message.extend_from_slice(STAMP_CONTEXT.as_bytes());
    message.extend_from_slice(text.as_bytes());
    let signature = key.sign(&message).to_bytes();
    format!("{text}.{}", b64::encode(&signature))
}
