//! The written contract (`contracts/phone-relay/v1`): example relay messages, an example pass,
//! what is made from a pairing code, and the lock's fixed test answers. The relay's repository
//! tests against the relay messages and the pass; the phone's page (`apps/remote`) tests its own
//! lock against the same test answers. Run with `PLENIPO_WRITE_CONTRACT=1` to write the files
//! again after a deliberate change; every other run checks that they still match, byte for byte.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::b64;
use crate::code::{hex, Code};
use crate::keys::PcKeys;
use crate::noise::{self, PAIRING_PROLOGUE};
use crate::protocol::{MeetingHello, MeetingWelcome, PairHello};
use crate::wire::{PcToRelay, PhoneToRelay, RelayToPc, RelayToPhone, SignedAnswer};

fn folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/phone-relay/v1")
}

/// Fixed keys for the examples (public on purpose: never used anywhere else).
const PHONE_STATIC: [u8; 32] = *b"plenipo-contract-phone-static-01";
const PC_STATIC: [u8; 32] = *b"plenipo-contract-pc-static-key01";
const PHONE_EPHEMERAL_PAIR: [u8; 32] = *b"plenipo-contract-phone-ephem-p01";
const PC_EPHEMERAL_PAIR: [u8; 32] = *b"plenipo-contract-pc-ephemeral-p1";
const PHONE_EPHEMERAL_KK: [u8; 32] = *b"plenipo-contract-phone-ephem-k01";
const PC_EPHEMERAL_KK: [u8; 32] = *b"plenipo-contract-pc-ephemeral-k1";
/// The example PC's relay key seed, and its notice key and passkey user (for the pass).
const PC_RELAY_SEED: [u8; 32] = *b"plenipo-contract-pc-relay-key-01";
const PHONE_ID: &str = "cGxlbmlwby1waG9uZS0wMQ";
const NOW: i64 = 1_791_000_000;

fn write_or_check(name: &str, text: &str) {
    let path = folder().join(name);
    if std::env::var_os("PLENIPO_WRITE_CONTRACT").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(&path, text).expect("written");
        return;
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} is missing ({e}); write it with PLENIPO_WRITE_CONTRACT=1",
            path.display()
        )
    });
    assert_eq!(
        kept.replace("\r\n", "\n"),
        text,
        "{} drifted from the code; write it again with PLENIPO_WRITE_CONTRACT=1 if the change \
         is deliberate",
        path.display()
    );
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("JSON");
    s.push('\n');
    s
}

fn example_keys() -> PcKeys {
    PcKeys::fixed_for_contract(PC_STATIC, PC_RELAY_SEED)
}

/// The lock's fixed test answers (`noise-vectors.json`); the noise tests read the pairing hash.
pub(crate) fn handshake_vectors() -> Value {
    use snow::Builder;
    let code = Code::parse(crate::code::tests::FIXED).expect("the fixed code");
    let psk = code.psk();
    // The first meeting.
    let mut phone = Builder::new(noise::PAIRING.parse().expect("a pattern"))
        .local_private_key(&PHONE_STATIC)
        .and_then(|b| b.prologue(PAIRING_PROLOGUE))
        .and_then(|b| b.psk(3, &psk))
        .map(|b| b.fixed_ephemeral_key_for_testing_only(&PHONE_EPHEMERAL_PAIR))
        .and_then(Builder::build_initiator)
        .expect("the phone's side");
    let mut pc = Builder::new(noise::PAIRING.parse().expect("a pattern"))
        .local_private_key(&PC_STATIC)
        .and_then(|b| b.prologue(PAIRING_PROLOGUE))
        .and_then(|b| b.psk(3, &psk))
        .map(|b| b.fixed_ephemeral_key_for_testing_only(&PC_EPHEMERAL_PAIR))
        .and_then(Builder::build_responder)
        .expect("the PC's side");
    let hello = serde_json::to_vec(&PairHello {
        name: "Frank's iPhone".into(),
        browser: "Safari on iPhone".into(),
    })
    .expect("JSON");
    let m1 = noise::write(&mut phone, b"").expect("m1");
    noise::read(&mut pc, &m1).expect("m1");
    let m2 = noise::write(&mut pc, b"").expect("m2");
    noise::read(&mut phone, &m2).expect("m2");
    let m3 = noise::write(&mut phone, &hello).expect("m3");
    assert_eq!(noise::read(&mut pc, &m3).expect("m3"), hello);
    let hash = hex(pc.get_handshake_hash());
    let mut phone = phone.into_transport_mode().expect("open");
    let mut pc = pc.into_transport_mode().expect("open");
    let p1 = noise::seal(&mut phone, br#"{"t":"hello from the phone"}"#).expect("sealed");
    let c1 = noise::seal(&mut pc, br#"{"t":"hello from the PC"}"#).expect("sealed");
    let pairing = json!({
        "protocol": noise::PAIRING,
        "prologue": hex(PAIRING_PROLOGUE),
        "code": crate::code::tests::FIXED,
        "psk": hex(&psk),
        "phoneStatic": hex(&PHONE_STATIC),
        "phoneStaticPublic": hex(&noise::public_of(&PHONE_STATIC)),
        "pcStatic": hex(&PC_STATIC),
        "pcStaticPublic": hex(&noise::public_of(&PC_STATIC)),
        "phoneEphemeral": hex(&PHONE_EPHEMERAL_PAIR),
        "pcEphemeral": hex(&PC_EPHEMERAL_PAIR),
        "messages": [
            { "from": "phone", "payload": "", "message": hex(&m1) },
            { "from": "pc", "payload": "", "message": hex(&m2) },
            { "from": "phone", "payload": hex(&hello), "message": hex(&m3) },
        ],
        "handshakeHash": hash,
        "sealed": [
            { "from": "phone", "plain": hex(br#"{"t":"hello from the phone"}"#), "pieces": p1.iter().map(|p| hex(p)).collect::<Vec<_>>() },
            { "from": "pc", "plain": hex(br#"{"t":"hello from the PC"}"#), "pieces": c1.iter().map(|p| hex(p)).collect::<Vec<_>>() },
        ],
    });

    // An everyday meeting.
    let fingerprint = example_keys().fingerprint();
    let prologue = noise::everyday_prologue(&fingerprint, PHONE_ID);
    let pc_public = noise::public_of(&PC_STATIC);
    let phone_public = noise::public_of(&PHONE_STATIC);
    let mut phone = Builder::new(noise::EVERYDAY.parse().expect("a pattern"))
        .local_private_key(&PHONE_STATIC)
        .and_then(|b| b.remote_public_key(&pc_public))
        .and_then(|b| b.prologue(&prologue))
        .map(|b| b.fixed_ephemeral_key_for_testing_only(&PHONE_EPHEMERAL_KK))
        .and_then(Builder::build_initiator)
        .expect("the phone's side");
    let mut pc = Builder::new(noise::EVERYDAY.parse().expect("a pattern"))
        .local_private_key(&PC_STATIC)
        .and_then(|b| b.remote_public_key(&phone_public))
        .and_then(|b| b.prologue(&prologue))
        .map(|b| b.fixed_ephemeral_key_for_testing_only(&PC_EPHEMERAL_KK))
        .and_then(Builder::build_responder)
        .expect("the PC's side");
    let meeting_hello = serde_json::to_vec(&MeetingHello {
        notice: false,
        page: "1.19.0".into(),
    })
    .expect("JSON");
    let welcome = serde_json::to_vec(&MeetingWelcome {
        signed_in: false,
        challenge: Some(b64::encode(&[0x42; 32])),
        pc_name: "Office PC".into(),
        version: "1.19.0".into(),
        // The published test answers stay the same: a welcome without notices.
        notice_key: None,
    })
    .expect("JSON");
    let m1 = noise::write(&mut phone, &meeting_hello).expect("m1");
    assert_eq!(noise::read(&mut pc, &m1).expect("m1"), meeting_hello);
    let m2 = noise::write(&mut pc, &welcome).expect("m2");
    assert_eq!(noise::read(&mut phone, &m2).expect("m2"), welcome);
    let hash = hex(pc.get_handshake_hash());
    let mut phone = phone.into_transport_mode().expect("open");
    let mut pc = pc.into_transport_mode().expect("open");
    // A long message, in pieces.
    let long: Vec<u8> = (0..70_000u32).map(|i| b'a' + (i % 26) as u8).collect();
    let p1 = noise::seal(&mut phone, br#"{"t":"ask"}"#).expect("sealed");
    let c1 = noise::seal(&mut pc, &long).expect("sealed");
    let everyday = json!({
        "protocol": noise::EVERYDAY,
        "prologue": hex(&prologue),
        "pc": fingerprint,
        "phone": PHONE_ID,
        "phoneStatic": hex(&PHONE_STATIC),
        "phoneStaticPublic": hex(&phone_public),
        "pcStatic": hex(&PC_STATIC),
        "pcStaticPublic": hex(&pc_public),
        "phoneEphemeral": hex(&PHONE_EPHEMERAL_KK),
        "pcEphemeral": hex(&PC_EPHEMERAL_KK),
        "messages": [
            { "from": "phone", "payload": hex(&meeting_hello), "message": hex(&m1) },
            { "from": "pc", "payload": hex(&welcome), "message": hex(&m2) },
        ],
        "handshakeHash": hash,
        "sealed": [
            { "from": "phone", "plain": hex(br#"{"t":"ask"}"#), "pieces": p1.iter().map(|p| hex(p)).collect::<Vec<_>>() },
            { "from": "pc", "plainLength": long.len(), "plainPattern": "a to z, again and again", "pieces": c1.iter().map(|p| hex(p)).collect::<Vec<_>>() },
        ],
    });
    json!({ "pairing": pairing, "everyday": everyday })
}

fn relay_examples() -> Value {
    let keys = example_keys();
    let nonce = b64::encode(&[0x11; 32]);
    let pass = crate::pass::issue(&keys, PHONE_ID, NOW, crate::PASS_LIFE_SECS);
    let answer = SignedAnswer {
        answer: "<8 West's signed weekly answer: see contracts/license-check/v1/answers>".into(),
        signature: "<its signature>".into(),
    };
    let to_value = |s: String| serde_json::from_str::<Value>(&s).expect("JSON");
    json!({
        "pc": {
            "relayToPc": [
                to_value(crate::wire::write(&RelayToPc::Challenge { nonce: nonce.clone() })),
                to_value(crate::wire::write(&RelayToPc::Welcome { pc: keys.fingerprint() })),
                to_value(crate::wire::write(&RelayToPc::Refused { code: "not_pro".into() })),
                to_value(crate::wire::write(&RelayToPc::Joined { conn: "c000001".into(), phone: Some(PHONE_ID.into()), mailbox: false })),
                to_value(crate::wire::write(&RelayToPc::Joined { conn: "c000002".into(), phone: None, mailbox: true })),
                to_value(crate::wire::write(&RelayToPc::Data { conn: "c000001".into(), data: b64::encode(b"<sealed>") })),
                to_value(crate::wire::write(&RelayToPc::Left { conn: "c000001".into() })),
                to_value(crate::wire::write(&RelayToPc::Error { code: "too_big".into() })),
            ],
            "pcToRelay": [
                to_value(crate::wire::write(&PcToRelay::Hello {
                    v: 1,
                    key: b64::encode(&keys.relay_public()),
                    proof: b64::encode(&keys.relay_sign(crate::wire::PC_PROOF_CONTEXT, &nonce)),
                    answer,
                })),
                to_value(crate::wire::write(&PcToRelay::Mailbox { mailbox: Code::parse(crate::code::tests::FIXED).expect("fixed").mailbox() })),
                to_value(crate::wire::write(&PcToRelay::CloseMailbox)),
                to_value(crate::wire::write(&PcToRelay::Drop { phone: PHONE_ID.into(), until: NOW + crate::PASS_LIFE_SECS })),
                to_value(crate::wire::write(&PcToRelay::Send { conn: "c000001".into(), data: b64::encode(b"<sealed>") })),
                to_value(crate::wire::write(&PcToRelay::Close { conn: "c000001".into() })),
            ],
        },
        "phone": {
            "phoneToRelay": [
                to_value(crate::wire::write(&PhoneToRelay::Pass { pass: pass.clone() })),
                to_value(crate::wire::write(&PhoneToRelay::Mailbox { mailbox: Code::parse(crate::code::tests::FIXED).expect("fixed").mailbox() })),
                to_value(crate::wire::write(&PhoneToRelay::Data { data: b64::encode(b"<sealed>") })),
            ],
            "relayToPhone": [
                to_value(crate::wire::write(&RelayToPhone::Ready)),
                to_value(crate::wire::write(&RelayToPhone::Refused { code: "bad_pass".into() })),
                to_value(crate::wire::write(&RelayToPhone::Data { data: b64::encode(b"<sealed>") })),
                to_value(crate::wire::write(&RelayToPhone::PcOffline)),
            ],
        },
        "examplePc": {
            "relayKeySeed": hex(&PC_RELAY_SEED),
            "relayKeyPublic": b64::encode(&keys.relay_public()),
            "fingerprint": keys.fingerprint(),
            "challengeNonce": nonce,
        },
        "examplePass": {
            "pass": pass,
            "checkedAt": NOW,
            "phone": PHONE_ID,
            "endsAt": NOW + crate::PASS_LIFE_SECS,
        },
        "codes": crate::wire::codes::ALL,
    })
}

#[test]
fn the_contract_files_match_the_code() {
    write_or_check("relay-messages.json", &pretty(&relay_examples()));
    write_or_check("noise-vectors.json", &pretty(&handshake_vectors()));
    let code = Code::parse(crate::code::tests::FIXED).expect("fixed");
    write_or_check(
        "pairing-code.json",
        &pretty(&json!({
            "code": code.plain(),
            "shown": code.shown(),
            "mailbox": code.mailbox(),
            "psk": hex(&code.psk()),
            "hkdf": { "hash": "SHA-256", "salt": "plenipo-remote-pairing.v1", "ikm": "the code's 16 characters, ASCII", "info": { "mailbox": "mailbox (16 bytes, hex)", "psk": "psk (32 bytes)" } },
            "link": code.link(crate::PAGE_ORIGIN),
        })),
    );
}

#[test]
fn the_example_pass_checks_with_the_example_key() {
    let keys = example_keys();
    let pass = crate::pass::issue(&keys, PHONE_ID, NOW, crate::PASS_LIFE_SECS);
    let checked = crate::pass::check(&pass, &keys.relay_public(), NOW + 1).expect("good");
    assert_eq!(checked.phone, PHONE_ID);
}
