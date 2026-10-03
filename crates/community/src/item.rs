//! Sealed items (contract §6, §7): sealing one for every PC it is for, and opening one that
//! arrived, with every check the contract lists, in its order.
//!
//! Sending: the item's payload is written as JSON and base64url; this PC signs it; a fresh
//! report key makes the report tag; and the payload, the signature, and the report key are
//! sealed once for each PC. 8 West sees only the envelope and the tag (ADR-164 §1, §6).
//!
//! Receiving: a copy is kept only if it opens with this PC's key as this item, its signature is
//! one of the sender's PCs', its tag matches, its payload says the same as its envelope, and its
//! stamp says the same too. Anything else is dropped, and only the reason is recorded: never a
//! word of it.

use hmac::{KeyInit as _, Mac as _};
use serde::Serialize;

use crate::ids::{self, IdKind};
use crate::keys::{self, PcKeys};
use crate::wire::{DevicePublic, ItemCopy, ItemKind, ItemPayload, SealedContent};
use crate::{b64, seal, stamp, CommunityError, Result};

/// The label a PC signs an item's payload under.
pub const ITEM_CONTEXT: &str = "plenipo-community-item.v1.";
/// The label the report tag is made under.
pub const REPORT_CONTEXT: &str = "plenipo-community-report.v1.";
/// The most copies of one item: 5 PCs of the person it is for, and 4 more of the sender's.
pub const MOST_COPIES: usize = 9;

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

/// An item sealed and ready to send (`ItemSend`), with what this PC keeps of it.
#[derive(Debug, Clone)]
pub struct Sealed {
    pub item_id: String,
    pub kind: ItemKind,
    pub to: String,
    pub reference: Option<String>,
    /// The report proof's commitment.
    pub tag: String,
    /// One copy for each PC.
    pub copies: Vec<ItemCopy>,
    /// What this PC keeps, so the item can be shown here and, by the person it was for, reported.
    pub kept: Kept,
}

/// What a PC keeps of an item: the payload exactly as signed, the signature, and the report key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub payload: String,
    pub sig: String,
    pub fk: String,
}

/// The report tag: HMAC-SHA256 with the report key over the label and the payload's text.
pub fn report_tag(fk: &[u8; 32], payload: &str) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(fk).expect("HMAC takes a key of any length");
    mac.update(REPORT_CONTEXT.as_bytes());
    mac.update(payload.as_bytes());
    mac.finalize().into_bytes().into()
}

/// Whether the report tag matches, compared in constant time.
fn tag_matches(fk: &[u8; 32], payload: &str, tag: &[u8; 32]) -> bool {
    let mut mac = HmacSha256::new_from_slice(fk).expect("HMAC takes a key of any length");
    mac.update(REPORT_CONTEXT.as_bytes());
    mac.update(payload.as_bytes());
    mac.verify_slice(tag).is_ok()
}

/// Seal `payload` for each of `pcs`: every PC of the person it is for, and every other PC of
/// the sender, never the sending PC itself (contract §6). `payload.from_device` must be this
/// PC, whose keys are `keys`.
pub fn seal_item<B: Serialize>(
    keys: &PcKeys,
    payload: &ItemPayload<B>,
    pcs: &[DevicePublic],
) -> Result<Sealed> {
    let invalid = |why: &str| CommunityError::Invalid(why.to_owned());
    if payload.v != 1
        || !ids::is_id(IdKind::Item, &payload.item_id)
        || !ids::is_id(IdKind::Member, &payload.from)
        || !ids::is_id(IdKind::Member, &payload.to)
        || !ids::is_id(IdKind::Device, &payload.from_device)
    {
        return Err(invalid("This item isn't one the contract allows."));
    }
    if pcs.is_empty() || pcs.len() > MOST_COPIES {
        return Err(invalid("An item goes to between 1 and 9 PCs."));
    }
    let mut seen = std::collections::BTreeSet::new();
    for pc in pcs {
        if pc.device_id == payload.from_device || !seen.insert(pc.device_id.as_str()) {
            return Err(invalid("An item goes once to each other PC."));
        }
    }

    let json = serde_json::to_vec(payload).map_err(|_| invalid("This item can't be written."))?;
    let text = b64::encode(&json);
    let sig = b64::encode(&keys.sign(ITEM_CONTEXT, &text));
    let fk: [u8; 32] = crate::random();
    let tag = b64::encode(&report_tag(&fk, &text));
    let kept = Kept {
        payload: text,
        sig,
        fk: b64::encode(&fk),
    };
    let inside = serde_json::to_vec(&SealedContent {
        payload: kept.payload.clone(),
        sig: kept.sig.clone(),
        fk: kept.fk.clone(),
    })
    .expect("a sealed content always serializes");

    let most = payload.kind.most_sealed_bytes();
    let mut copies = Vec::with_capacity(pcs.len());
    for pc in pcs {
        if !ids::is_id(IdKind::Device, &pc.device_id) {
            return Err(invalid("A PC's ID isn't one the contract allows."));
        }
        let key = b64::decode_exact::<32>(&pc.sealing_key)
            .ok_or_else(|| invalid("A PC's sealing key can't be read."))?;
        let sealed = seal::seal(&key, &seal::info(&payload.item_id, &pc.device_id), &inside)
            .ok_or_else(|| invalid("A PC's sealing key can't be sealed to."))?;
        if sealed.len() > most {
            return Err(invalid("This is too long to send."));
        }
        copies.push(ItemCopy {
            device_id: pc.device_id.clone(),
            sealed: b64::encode(&sealed),
        });
    }
    Ok(Sealed {
        item_id: payload.item_id.clone(),
        kind: payload.kind,
        to: payload.to.clone(),
        reference: payload.reference.clone(),
        tag,
        copies,
        kept,
    })
}

/// An item's envelope as it arrived (an `InboxItem` that is not a notice).
#[derive(Debug, Clone)]
pub struct Envelope<'a> {
    pub item_id: &'a str,
    pub kind: ItemKind,
    pub from: &'a str,
    pub reference: Option<&'a str>,
    /// This PC's sealed copy, base64url.
    pub sealed: &'a str,
    pub tag: &'a str,
    pub stamp: &'a str,
}

/// An item that arrived and passed every check.
#[derive(Debug, Clone)]
pub struct Opened {
    /// The payload, with its body still plain JSON: check it for its kind before showing it.
    pub payload: ItemPayload,
    /// What this PC keeps, with the stamp, so the person can report it.
    pub kept: Kept,
    pub stamp: String,
}

/// Why an item that arrived was dropped. Recorded without a word of the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// It does not open with this PC's key as this item.
    NotForThisPc,
    /// What is inside is not what the contract says.
    Unreadable,
    /// It is signed by none of the sender's PCs signed in now.
    UnknownPc,
    /// Its signature does not check.
    BadSignature,
    /// Its report tag does not match.
    BadTag,
    /// Its payload or its stamp says something else than its envelope.
    Mismatch,
}

impl Dropped {
    /// The reason, for the Ledger.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotForThisPc => "not_for_this_pc",
            Self::Unreadable => "unreadable",
            Self::UnknownPc => "unknown_pc",
            Self::BadSignature => "bad_signature",
            Self::BadTag => "bad_tag",
            Self::Mismatch => "mismatch",
        }
    }
}

/// Who this PC is, to open what arrives for it.
pub struct ThisPc<'a> {
    pub keys: &'a PcKeys,
    pub device_id: &'a str,
    pub member_id: &'a str,
}

/// Open an item that arrived, with the contract's checks in its order (contract §7), and then
/// two of this PC's own: the item is to or from this member, and its stamp says the same as its
/// envelope. `sender_pcs` is the sender's PCs as the service lists them now.
pub fn open_item(
    pc: &ThisPc<'_>,
    envelope: &Envelope<'_>,
    sender_pcs: &[DevicePublic],
) -> std::result::Result<Opened, Dropped> {
    // The seal opens with this PC's key and `info`.
    let sealed = b64::decode(envelope.sealed, envelope.kind.most_sealed_bytes())
        .ok_or(Dropped::NotForThisPc)?;
    let inside = seal::open(
        pc.keys.sealing_private(),
        &seal::info(envelope.item_id, pc.device_id),
        &sealed,
    )
    .ok_or(Dropped::NotForThisPc)?;

    // `SealedContent` and `ItemPayload` parse.
    let content: SealedContent =
        serde_json::from_slice(&inside).map_err(|_| Dropped::Unreadable)?;
    let json = b64::decode(&content.payload, sealed.len()).ok_or(Dropped::Unreadable)?;
    let payload: ItemPayload = serde_json::from_slice(&json).map_err(|_| Dropped::Unreadable)?;
    let fk = b64::decode_exact::<32>(&content.fk).ok_or(Dropped::Unreadable)?;
    if payload.v != 1 || !payload.body.is_object() {
        return Err(Dropped::Unreadable);
    }

    // `from_device` is one of `from`'s PCs, and `sig` checks with its signing key.
    let signer = sender_pcs
        .iter()
        .find(|d| d.device_id == payload.from_device)
        .ok_or(Dropped::UnknownPc)?;
    if !keys::verify(
        &signer.signing_key,
        ITEM_CONTEXT,
        &content.payload,
        &content.sig,
    ) {
        return Err(Dropped::BadSignature);
    }

    // The report tag over the payload, with the report key, equals the envelope's.
    let tag = b64::decode_exact::<32>(envelope.tag).ok_or(Dropped::BadTag)?;
    if !tag_matches(&fk, &content.payload, &tag) {
        return Err(Dropped::BadTag);
    }

    // `item_id`, `kind`, `from`, `to`, and `ref` in the payload equal the envelope's.
    let stamped = stamp::read(envelope.stamp).ok_or(Dropped::Mismatch)?;
    let same_envelope = payload.item_id == envelope.item_id
        && payload.kind == envelope.kind
        && payload.from == envelope.from
        && payload.reference.as_deref() == envelope.reference
        && payload.to == stamped.to;
    // This PC's own: it is to this member, or from this member (a copy of what they sent from
    // another of their PCs); and the stamp says the same as the envelope.
    let ours = payload.to == pc.member_id || payload.from == pc.member_id;
    let same_stamp = stamped.item_id == envelope.item_id
        && stamped.kind == envelope.kind
        && stamped.from == envelope.from
        && stamped.reference.as_deref() == envelope.reference
        && stamped.tag == envelope.tag;
    if !(same_envelope && ours && same_stamp) {
        return Err(Dropped::Mismatch);
    }

    Ok(Opened {
        payload,
        kept: Kept {
            payload: content.payload,
            sig: content.sig,
            fk: content.fk,
        },
        stamp: envelope.stamp.to_owned(),
    })
}
