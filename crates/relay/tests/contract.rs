//! The relay pins the written contract (`contracts/phone-relay/v1`): every example message reads
//! and writes back the same, the example pass checks with the example PC's key, the example proof
//! checks against the example challenge, and the codes are the contract's.

use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use plenipo_relay_contract::wire::{self, codes, PcToRelay, PhoneToRelay, RelayToPc, RelayToPhone};
use plenipo_relay_contract::{b64, fingerprint, pass};
use serde_json::Value;

fn contract() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/phone-relay/v1/relay-messages.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("the contract's messages"))
        .expect("JSON")
}

fn round_trip<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(examples: &Value) {
    for example in examples.as_array().expect("a list") {
        let text = example.to_string();
        let message: T =
            wire::read(&text).unwrap_or_else(|| panic!("the relay cannot read {text}"));
        let written: Value = serde_json::from_str(&wire::write(&message)).unwrap();
        assert_eq!(&written, example, "written back differently");
    }
}

#[test]
fn every_example_message_reads_and_writes_the_same() {
    let c = contract();
    round_trip::<PcToRelay>(&c["pc"]["pcToRelay"]);
    round_trip::<RelayToPc>(&c["pc"]["relayToPc"]);
    round_trip::<PhoneToRelay>(&c["phone"]["phoneToRelay"]);
    round_trip::<RelayToPhone>(&c["phone"]["relayToPhone"]);
}

#[test]
fn the_codes_are_the_contracts() {
    let c = contract();
    let listed: Vec<&str> = c["codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(listed, codes::ALL.to_vec());
}

#[test]
fn the_example_pass_and_proof_check_with_the_example_pc() {
    let c = contract();
    let key: [u8; 32] =
        b64::decode_exact(c["examplePc"]["relayKeyPublic"].as_str().unwrap()).unwrap();
    assert_eq!(
        fingerprint(&key),
        c["examplePc"]["fingerprint"].as_str().unwrap()
    );
    let pass_text = c["examplePass"]["pass"].as_str().unwrap();
    let checked_at = c["examplePass"]["checkedAt"].as_i64().unwrap();
    let payload = pass::check(pass_text, &key, checked_at).expect("the example pass checks");
    assert_eq!(payload.phone, c["examplePass"]["phone"].as_str().unwrap());
    assert_eq!(payload.exp, c["examplePass"]["endsAt"].as_i64().unwrap());
    assert_eq!(
        pass::check(pass_text, &key, payload.exp),
        Err(pass::PassError::Expired)
    );
    // The hello's proof covers the challenge's nonce, exactly as sent.
    let hello = &c["pc"]["pcToRelay"][0];
    assert_eq!(hello["t"], "hello");
    let proof: [u8; 64] = b64::decode_exact(hello["proof"].as_str().unwrap()).unwrap();
    let nonce = c["examplePc"]["challengeNonce"].as_str().unwrap();
    VerifyingKey::from_bytes(&key)
        .unwrap()
        .verify_strict(
            format!("{}{nonce}", wire::PC_PROOF_CONTEXT).as_bytes(),
            &Signature::from_bytes(&proof),
        )
        .expect("the example proof checks");
}
