//! The worked examples, checked byte for byte: the published HPKE test answers for Community's
//! suite, and the contract's worked item, seal, stamp, report proof, and safety code
//! (`contracts/community/v1/examples/vector-*.json`, `test-stamping-key.json`). The contract's
//! worked seal and safety code were made by a second program, written separately and checked
//! against the same published answers, so these tests check two programs against each other.

use std::path::{Path, PathBuf};

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde_json::Value;

use crate::item::{self, Dropped, Envelope, ThisPc};
use crate::keys::{self, PcKeys};
use crate::seal::{self, Given};
use crate::wire::{DevicePublic, ItemKind, ItemPayload, MessageBody, StampPayload};
use crate::{b64, safety, stamp};

fn contract() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/community/v1")
}

fn json(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn example(name: &str) -> Value {
    json(contract().join("examples").join(name))
}

fn text(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key}"))
        .to_owned()
}

fn hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex32(text: &str) -> [u8; 32] {
    hex(text).try_into().unwrap()
}

#[test]
fn sealing_matches_the_published_hpke_test_answers() {
    let file = json(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hpke-x25519-sha256-aes256gcm.json"),
    );
    let v = &file["vector"];
    assert_eq!(
        (&v["mode"], &v["kem_id"], &v["kdf_id"], &v["aead_id"]),
        (
            &Value::from(0),
            &Value::from(32),
            &Value::from(1),
            &Value::from(2)
        ),
        "the answers are for Community's own suite"
    );
    // The receiving key: its public half is the published one.
    let receiver = PcKeys::from_secrets([0; 32], hex32(&text(v, "skRm")));
    assert_eq!(receiver.sealing_public().to_vec(), hex(&text(v, "pkRm")));

    // Sealing with the published one-time key material gives the published answer.
    let info = hex(&text(v, "info"));
    let first = &v["encryptions"][0];
    let (aad, pt, ct) = (
        hex(&text(first, "aad")),
        hex(&text(first, "pt")),
        hex(&text(first, "ct")),
    );
    let sealed = seal::seal_with(
        &receiver.sealing_public(),
        &info,
        &aad,
        &pt,
        &mut Given(hex(&text(v, "ikmE"))),
    )
    .unwrap();
    assert_eq!(
        sealed[..seal::ENC_BYTES].to_vec(),
        hex(&text(v, "enc")),
        "the one-time key"
    );
    assert_eq!(sealed[seal::ENC_BYTES..].to_vec(), ct, "the ciphertext");

    // And opening the published answer gives the published words.
    let mut published = hex(&text(v, "enc"));
    published.extend_from_slice(&ct);
    let private = hex32(&text(v, "skRm"));
    assert_eq!(
        seal::open_with(&private, &info, &aad, &published).unwrap(),
        pt
    );
    assert_eq!(
        seal::open_with(&private, &info, &[], &published),
        None,
        "another aad"
    );
}

#[test]
fn the_contracts_worked_seal() {
    let v = example("vector-seal.json");
    let device = &v["device"];
    let receiver = PcKeys::from_secrets([0; 32], hex32(&text(device, "sealing_private_hex")));
    assert_eq!(receiver.sealing_key_text(), text(device, "sealing_key"));

    let info = seal::info(&text(&v, "item_id"), &text(device, "device_id"));
    assert_eq!(String::from_utf8(info.clone()).unwrap(), text(&v, "info"));
    let inside = text(&v, "sealed_content_json");
    let sealed = seal::seal_with(
        &receiver.sealing_public(),
        &info,
        &[],
        inside.as_bytes(),
        &mut Given(hex(&text(&v, "ephemeral_ikm_hex"))),
    )
    .unwrap();
    assert_eq!(
        b64::encode(&sealed),
        text(&v, "sealed"),
        "the same sealed bytes"
    );
    assert_eq!(b64::encode(&sealed[..seal::ENC_BYTES]), text(&v, "enc"));

    let published = b64::decode(&text(&v, "sealed"), 4096).unwrap();
    let private = hex32(&text(device, "sealing_private_hex"));
    assert_eq!(
        seal::open(&private, &info, &published).unwrap(),
        inside.as_bytes()
    );
    assert_eq!(
        seal::open(&private, text(&v, "wrong_info").as_bytes(), &published),
        None
    );
}

#[test]
fn the_contracts_worked_item_signature_tag_and_stamp() {
    let v = example("vector-item.json");
    let device = &v["device"];
    let sender = PcKeys::from_secrets(hex32(&text(device, "signing_seed_hex")), [1; 32]);
    assert_eq!(sender.signing_key_text(), text(device, "signing_key"));

    // The payload is the JSON, base64url; Plenipo writes it the same way, byte for byte.
    let payload = text(&v, "payload");
    let payload_json = text(&v, "payload_json");
    assert_eq!(b64::encode(payload_json.as_bytes()), payload);
    let typed: ItemPayload<MessageBody> = serde_json::from_str(&payload_json).unwrap();
    assert_eq!(serde_json::to_string(&typed).unwrap(), payload_json);

    // The signature: Ed25519 is deterministic, so this PC's key makes the same one.
    let sig = text(&v, "sig");
    assert_eq!(b64::encode(&sender.sign(item::ITEM_CONTEXT, &payload)), sig);
    assert!(keys::verify(
        &text(device, "signing_key"),
        item::ITEM_CONTEXT,
        &payload,
        &sig
    ));

    // The report tag, and the changed words it does not match.
    let fk = b64::decode_exact::<32>(&text(&v, "fk")).unwrap();
    assert_eq!(
        b64::encode(&item::report_tag(&fk, &payload)),
        text(&v, "tag")
    );
    assert_ne!(
        b64::encode(&item::report_tag(&fk, &text(&v, "tampered_payload"))),
        text(&v, "tag")
    );

    // The stamp, by the contract's published test stamping key.
    let stamping = json(contract().join("test-stamping-key.json"));
    let key = SigningKey::from_bytes(&hex32(&text(&stamping, "seedHex")));
    let public = VerifyingKey::from_bytes(&hex32(&text(&stamping, "publicHex"))).unwrap();
    assert_eq!(key.verifying_key(), public);
    let stamped = stamp::check(&text(&v, "stamp"), &public).expect("the stamp checks");
    assert_eq!(
        serde_json::to_string(&stamped).unwrap(),
        text(&v, "stamp_payload_json")
    );
    assert_eq!(
        stamp::make(&stamped, &key),
        text(&v, "stamp"),
        "the stand-in stamps the same way"
    );
    assert_eq!(stamp::read(&text(&v, "stamp")), Some(stamped));
    let other = SigningKey::from_bytes(&[9; 32]).verifying_key();
    assert_eq!(
        stamp::check(&text(&v, "stamp"), &other),
        None,
        "another stamping key"
    );
}

/// The worked item, sealed for Pat's PC as the worked seal, as it arrives.
struct Arrival {
    pat: PcKeys,
    pat_device: String,
    pat_member: String,
    item_id: String,
    from: String,
    sealed: String,
    tag: String,
    stamp: String,
    sender_pcs: Vec<DevicePublic>,
}

fn arrival() -> Arrival {
    let item = example("vector-item.json");
    let s = example("vector-seal.json");
    let device = &s["device"];
    Arrival {
        pat: PcKeys::from_secrets([2; 32], hex32(&text(device, "sealing_private_hex"))),
        pat_device: text(device, "device_id"),
        pat_member: text(device, "member_id"),
        item_id: text(&s, "item_id"),
        from: text(&item["device"], "member_id"),
        sealed: text(&s, "sealed"),
        tag: text(&item, "tag"),
        stamp: text(&item, "stamp"),
        sender_pcs: vec![DevicePublic {
            device_id: text(&item["device"], "device_id"),
            signing_key: text(&item["device"], "signing_key"),
            sealing_key: b64::encode(&[5; 32]),
        }],
    }
}

impl Arrival {
    fn open(&self) -> Result<item::Opened, Dropped> {
        self.open_as(&self.envelope(), &self.sender_pcs)
    }

    fn envelope(&self) -> Envelope<'_> {
        Envelope {
            item_id: &self.item_id,
            kind: ItemKind::Message,
            from: &self.from,
            reference: None,
            sealed: &self.sealed,
            tag: &self.tag,
            stamp: &self.stamp,
        }
    }

    fn open_as(
        &self,
        envelope: &Envelope<'_>,
        pcs: &[DevicePublic],
    ) -> Result<item::Opened, Dropped> {
        let pc = ThisPc {
            keys: &self.pat,
            device_id: &self.pat_device,
            member_id: &self.pat_member,
        };
        item::open_item(&pc, envelope, pcs)
    }
}

#[test]
fn the_worked_item_arrives_and_passes_every_check() {
    let a = arrival();
    let opened = a.open().expect("the worked item opens");
    let v = example("vector-item.json");
    assert_eq!(opened.kept.payload, text(&v, "payload"));
    assert_eq!(opened.kept.sig, text(&v, "sig"));
    assert_eq!(opened.kept.fk, text(&v, "fk"));
    assert_eq!(opened.stamp, text(&v, "stamp"));
    let body: MessageBody = serde_json::from_value(opened.payload.body.clone()).unwrap();
    assert!(body.text.is_some());
}

#[test]
fn an_item_that_fails_any_check_is_dropped() {
    let a = arrival();

    // Not this PC: another PC's ID, or another PC's key.
    let other_id = "cd_01JB7Q8R9S0T1V2W3X4Y5Z6A7D".to_owned();
    let elsewhere = Arrival {
        pat_device: other_id,
        ..arrival()
    };
    assert_eq!(elsewhere.open().unwrap_err(), Dropped::NotForThisPc);
    let other_key = Arrival {
        pat: PcKeys::generate(),
        ..arrival()
    };
    assert_eq!(other_key.open().unwrap_err(), Dropped::NotForThisPc);

    // Signed by none of the sender's PCs signed in now, or a PC whose key is another.
    assert_eq!(
        a.open_as(&a.envelope(), &[]).unwrap_err(),
        Dropped::UnknownPc
    );
    let mut swapped = a.sender_pcs.clone();
    swapped[0].signing_key = PcKeys::generate().signing_key_text();
    assert_eq!(
        a.open_as(&a.envelope(), &swapped).unwrap_err(),
        Dropped::BadSignature
    );

    // Another tag on the envelope.
    let tag = b64::encode(&[3; 32]);
    let env = Envelope {
        tag: &tag,
        ..a.envelope()
    };
    assert_eq!(a.open_as(&env, &a.sender_pcs).unwrap_err(), Dropped::BadTag);

    // The envelope says another sender, kind, or link than the payload.
    let someone = "cm_01JZZZZZZZZZZZZZZZZZZZZZZZ";
    for env in [
        Envelope {
            from: someone,
            ..a.envelope()
        },
        Envelope {
            kind: ItemKind::Reaction,
            ..a.envelope()
        },
        Envelope {
            reference: Some("cl_01JZZZZZZZZZZZZZZZZZZZZZZZ"),
            ..a.envelope()
        },
    ] {
        let dropped = a.open_as(&env, &a.sender_pcs).unwrap_err();
        assert!(
            matches!(dropped, Dropped::Mismatch | Dropped::NotForThisPc),
            "{dropped:?}"
        );
    }

    // An item for someone else, even sealed for this PC.
    let not_ours = Arrival {
        pat_member: someone.to_owned(),
        ..arrival()
    };
    assert_eq!(not_ours.open().unwrap_err(), Dropped::Mismatch);

    // A stamp that says something else, or is not a stamp.
    let mut stamped = stamp::read(&a.stamp).unwrap();
    stamped.to = someone.to_owned();
    let changed = stamp::make(&stamped, &SigningKey::from_bytes(&[4; 32]));
    let env = Envelope {
        stamp: &changed,
        ..a.envelope()
    };
    assert_eq!(
        a.open_as(&env, &a.sender_pcs).unwrap_err(),
        Dropped::Mismatch
    );
    let env = Envelope {
        stamp: "not.a stamp",
        ..a.envelope()
    };
    assert_eq!(
        a.open_as(&env, &a.sender_pcs).unwrap_err(),
        Dropped::Mismatch
    );

    // A changed byte anywhere in the sealed copy.
    let bytes = b64::decode(&a.sealed, 4096).unwrap();
    for at in [0, 40, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[at] ^= 0x10;
        let changed = b64::encode(&changed);
        let env = Envelope {
            sealed: &changed,
            ..a.envelope()
        };
        assert_eq!(
            a.open_as(&env, &a.sender_pcs).unwrap_err(),
            Dropped::NotForThisPc
        );
    }
}

#[test]
fn an_item_sealed_here_opens_on_every_pc_it_is_for_and_nowhere_else() {
    let alice_member = "cm_01JA2B3C4D5E6F7G8H9J0K1M2N";
    let pat_member = "cm_01JB7Q8R9S0T1V2W3X4Y5Z6A7B";
    let (alice_here, alice_there) = (PcKeys::generate(), PcKeys::generate());
    let (pat_one, pat_two) = (PcKeys::generate(), PcKeys::generate());
    let pc = |id: &str, keys: &PcKeys| DevicePublic {
        device_id: id.to_owned(),
        signing_key: keys.signing_key_text(),
        sealing_key: keys.sealing_key_text(),
    };
    let alice_pcs = [
        pc("cd_0000000000000000000000000A", &alice_here),
        pc("cd_0000000000000000000000000B", &alice_there),
    ];
    let pat_pcs = [
        pc("cd_0000000000000000000000000C", &pat_one),
        pc("cd_0000000000000000000000000D", &pat_two),
    ];

    let payload = ItemPayload {
        v: 1,
        item_id: crate::ids::new_id(crate::ids::IdKind::Item),
        kind: ItemKind::Message,
        from: alice_member.to_owned(),
        from_device: alice_pcs[0].device_id.clone(),
        to: pat_member.to_owned(),
        reference: None,
        sent_at: 1_790_000_000,
        body: MessageBody {
            text: Some("Hello 👋 مرحبا".to_owned()),
            gif: None,
            sticker: None,
            reply_to: None,
        },
    };
    let for_pcs = [pat_pcs[0].clone(), pat_pcs[1].clone(), alice_pcs[1].clone()];
    let sealed = item::seal_item(&alice_here, &payload, &for_pcs).unwrap();
    assert_eq!(sealed.copies.len(), 3);

    // The service stamps it with the published test key.
    let stamping = json(contract().join("test-stamping-key.json"));
    let key = SigningKey::from_bytes(&hex32(&text(&stamping, "seedHex")));
    let stamp = stamp::make(
        &StampPayload {
            v: 1,
            item_id: sealed.item_id.clone(),
            from: alice_member.to_owned(),
            to: pat_member.to_owned(),
            kind: ItemKind::Message,
            reference: None,
            tag: sealed.tag.clone(),
            at: 1_790_000_002,
            signer: "test-stamp-1".to_owned(),
        },
        &key,
    );

    let open = |keys: &PcKeys, member: &str, copy: usize| {
        let env = Envelope {
            item_id: &sealed.item_id,
            kind: ItemKind::Message,
            from: alice_member,
            reference: None,
            sealed: &sealed.copies[copy].sealed,
            tag: &sealed.tag,
            stamp: &stamp,
        };
        let pc = ThisPc {
            keys,
            device_id: &sealed.copies[copy].device_id,
            member_id: member,
        };
        item::open_item(&pc, &env, &alice_pcs)
    };
    for (keys, member, copy) in [
        (&pat_one, pat_member, 0),
        (&pat_two, pat_member, 1),
        (&alice_there, alice_member, 2),
    ] {
        let opened = open(keys, member, copy).expect("every PC it is for opens it");
        assert_eq!(opened.kept, sealed.kept);
        let body: MessageBody = serde_json::from_value(opened.payload.body).unwrap();
        assert_eq!(body.text.as_deref(), Some("Hello 👋 مرحبا"));
    }
    // Pat's first PC cannot open the copy for Pat's second.
    assert_eq!(
        open(&pat_one, pat_member, 1).unwrap_err(),
        Dropped::NotForThisPc
    );
    // A stranger's PC cannot open any.
    let stranger = PcKeys::generate();
    for copy in 0..3 {
        assert_eq!(
            open(&stranger, pat_member, copy).unwrap_err(),
            Dropped::NotForThisPc
        );
    }
    // The envelope never holds the words.
    for copy in &sealed.copies {
        assert!(!copy.sealed.contains("Hello"));
    }
}

#[test]
fn sealing_refuses_what_the_contract_does_not_allow() {
    let keys = PcKeys::generate();
    let pat = PcKeys::generate();
    let pat_pc = DevicePublic {
        device_id: "cd_0000000000000000000000000C".to_owned(),
        signing_key: pat.signing_key_text(),
        sealing_key: pat.sealing_key_text(),
    };
    let payload = |text: String| ItemPayload {
        v: 1,
        item_id: crate::ids::new_id(crate::ids::IdKind::Item),
        kind: ItemKind::Message,
        from: "cm_01JA2B3C4D5E6F7G8H9J0K1M2N".to_owned(),
        from_device: "cd_0000000000000000000000000A".to_owned(),
        to: "cm_01JB7Q8R9S0T1V2W3X4Y5Z6A7B".to_owned(),
        reference: None,
        sent_at: 1,
        body: MessageBody {
            text: Some(text),
            gif: None,
            sticker: None,
            reply_to: None,
        },
    };
    assert!(
        item::seal_item(&keys, &payload("hi".into()), &[]).is_err(),
        "no PC"
    );
    let this_pc = DevicePublic {
        device_id: "cd_0000000000000000000000000A".to_owned(),
        ..pat_pc.clone()
    };
    assert!(
        item::seal_item(&keys, &payload("hi".into()), &[this_pc]).is_err(),
        "the sending PC"
    );
    assert!(
        item::seal_item(
            &keys,
            &payload("hi".into()),
            &[pat_pc.clone(), pat_pc.clone()]
        )
        .is_err(),
        "the same PC twice"
    );
    let bad_key = DevicePublic {
        sealing_key: "AAAA".to_owned(),
        ..pat_pc.clone()
    };
    assert!(item::seal_item(&keys, &payload("hi".into()), &[bad_key]).is_err());
    let low_order = DevicePublic {
        sealing_key: b64::encode(&[0; 32]),
        ..pat_pc.clone()
    };
    assert!(item::seal_item(&keys, &payload("hi".into()), &[low_order]).is_err());
    assert!(
        item::seal_item(
            &keys,
            &payload("x".repeat(40_000)),
            std::slice::from_ref(&pat_pc)
        )
        .is_err(),
        "too long"
    );
    let mut bad_id = payload("hi".into());
    bad_id.item_id = "ci_nope".into();
    assert!(item::seal_item(&keys, &bad_id, &[pat_pc]).is_err());
}

#[test]
fn the_contracts_worked_safety_code() {
    let v = example("vector-safety-code.json");
    let person = |p: &Value| -> (String, Vec<DevicePublic>) {
        (
            text(p, "member_id"),
            serde_json::from_value(p["devices"].clone()).unwrap(),
        )
    };
    let (a, a_pcs) = person(&v["people"][0]);
    let (b, b_pcs) = person(&v["people"][1]);
    let code = text(&v, "code");
    assert_eq!(safety::safety_code((&a, &a_pcs), (&b, &b_pcs)), code);
    assert_eq!(
        safety::safety_code((&b, &b_pcs), (&a, &a_pcs)),
        code,
        "either side first"
    );

    let mut reordered = b_pcs.clone();
    reordered.reverse();
    assert_eq!(
        safety::safety_code((&a, &a_pcs), (&b, &reordered)),
        code,
        "in any order"
    );

    let (_, changed) = person(&v["changed"]["people"][1]);
    let changed_code = safety::safety_code((&a, &a_pcs), (&b, &changed));
    assert_eq!(changed_code, text(&v["changed"], "code"));
    assert_ne!(changed_code, code, "a changed sealing key changes the code");
    assert_ne!(
        safety::safety_code((&a, &a_pcs), (&b, &b_pcs[..1])),
        code,
        "a removed PC"
    );
    assert_eq!(code.len(), 14);
}
