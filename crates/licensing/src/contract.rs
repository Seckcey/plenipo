//! The written contract (`contracts/license-check/v1`, ADR-101 §3): example requests, keys, and
//! signed answers, made with the contract's test key. The account service tests against the same
//! files. Run with `PLENIPO_WRITE_CONTRACT=1` to write them again after a deliberate change; every
//! other run checks that they still match, byte for byte.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;

use crate::answer::{self, AnswerPayload, SubscriptionState};
use crate::codec;
use crate::key::{self, KeyEdition, KeyError, KeyPayload, Organizations, Plan};
use crate::trust;

pub const KEY_ID: &str = "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T";
/// A Partner key's ID (ADR-119).
pub const PARTNER_KEY_ID: &str = "lk_01J9XW3T5B8K2M4N6P7Q8R9S0V";
pub const APP_VERSION: &str = "1.18.0";
const ISSUED_AT: i64 = 1_790_000_000;
const PAID_THROUGH: i64 = 1_822_400_000;
const AS_OF: i64 = 1_791_000_000;
const ENDS_AT: i64 = 1_792_900_000;

fn folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/license-check/v1")
}

fn key_payload() -> KeyPayload {
    KeyPayload {
        v: 1,
        edition: "pro".into(),
        organizations: Organizations::Up(3),
        key_id: KEY_ID.into(),
        holder: "Contoso IT".into(),
        plan: Plan::Yearly,
        paid_through: PAID_THROUGH,
        issued_at: ISSUED_AT,
        signer: trust::TEST_KEY_ID.into(),
    }
}

fn answer_payload(state: SubscriptionState) -> AnswerPayload {
    AnswerPayload {
        v: 1,
        key_id: KEY_ID.into(),
        state,
        paid_through: match state {
            SubscriptionState::Unknown => None,
            _ => Some(PAID_THROUGH),
        },
        ends_at: match state {
            SubscriptionState::Cancelled | SubscriptionState::Ended => Some(ENDS_AT),
            _ => None,
        },
        as_of: AS_OF,
        signer: trust::TEST_KEY_ID.into(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every file of the contract that is made from code, with its exact bytes.
fn files() -> Vec<(&'static str, Vec<u8>)> {
    let test = trust::test_signing_key();
    let other = SigningKey::from_bytes(&[0x42; 32]);
    let valid = key::mint(&key_payload(), &test);
    // The same signature over a changed payload.
    let tampered = {
        let mut p = key_payload();
        p.paid_through += 10 * 365 * 24 * 3600;
        let sig = valid.rsplit('.').next().unwrap();
        let b64 = codec::encode(&serde_json::to_vec(&p).unwrap());
        format!("{}{b64}.{sig}", key::KEY_PREFIX)
    };
    // A Partner key covering any number of organizations (ADR-119).
    let partner = key::mint(
        &KeyPayload {
            edition: "partner".into(),
            organizations: Organizations::UNLIMITED,
            key_id: PARTNER_KEY_ID.into(),
            holder: "Fabrikam Managed IT".into(),
            plan: Plan::Monthly,
            ..key_payload()
        },
        &test,
    );
    let unknown_signer = {
        let mut p = key_payload();
        p.signer = "prod-9".into();
        key::mint(&p, &other)
    };
    let body = |state| answer::body(&answer::sign(&answer_payload(state), &test));
    let forged = answer::body(&answer::sign(
        &answer_payload(SubscriptionState::Active),
        &other,
    ));
    let test_key = serde_json::json!({
        "id": trust::TEST_KEY_ID,
        "note": "TEST ONLY. Published on purpose so both sides can sign and check the examples. \
                 Release builds of Plenipo never trust it.",
        "seedHex": hex(&trust::TEST_SEED),
        "publicHex": hex(test.verifying_key().as_bytes()),
    });
    let mut test_key = serde_json::to_vec_pretty(&test_key).unwrap();
    test_key.push(b'\n');
    vec![
        ("request.json", answer::request_body(KEY_ID, APP_VERSION)),
        ("test-signing-key.json", test_key),
        ("keys/valid.txt", valid.into_bytes()),
        ("keys/partner-unlimited.txt", partner.into_bytes()),
        ("keys/tampered.txt", tampered.into_bytes()),
        ("keys/unknown-signer.txt", unknown_signer.into_bytes()),
        ("answers/active.json", body(SubscriptionState::Active)),
        ("answers/cancelled.json", body(SubscriptionState::Cancelled)),
        ("answers/ended.json", body(SubscriptionState::Ended)),
        ("answers/unknown.json", body(SubscriptionState::Unknown)),
        ("answers/wrong-signature.json", forged),
    ]
}

#[test]
fn the_contract_files_match_the_code_byte_for_byte() {
    let write = std::env::var_os("PLENIPO_WRITE_CONTRACT").is_some();
    for (name, bytes) in files() {
        let path = folder().join(name);
        if write {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let kept = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            kept == bytes,
            "contracts/license-check/v1/{name} differs from the code; if the change is meant, \
             run this test with PLENIPO_WRITE_CONTRACT=1 and change the account service too"
        );
    }
}

#[test]
fn the_contracts_examples_mean_what_they_say() {
    let read = |name: &str| std::fs::read(folder().join(name)).unwrap();
    let text = |name: &str| String::from_utf8(read(name)).unwrap();
    // The request: the key ID and the app version, and nothing else.
    let request: serde_json::Value = serde_json::from_slice(&read("request.json")).unwrap();
    assert_eq!(
        request,
        serde_json::json!({ "key_id": KEY_ID, "app_version": APP_VERSION })
    );
    // The keys: a Pro key for 3 organizations, and a Partner key for any number.
    let valid = key::parse(&text("keys/valid.txt")).unwrap();
    assert_eq!(valid.key_id(), KEY_ID);
    assert_eq!(valid.edition(), KeyEdition::Pro);
    assert_eq!(valid.organizations(), Some(3));
    let partner = key::parse(&text("keys/partner-unlimited.txt")).unwrap();
    assert_eq!(partner.key_id(), PARTNER_KEY_ID);
    assert_eq!(partner.edition(), KeyEdition::Partner);
    assert_eq!(partner.organizations(), None);
    assert_eq!(
        key::parse(&text("keys/tampered.txt")),
        Err(KeyError::Damaged)
    );
    assert_eq!(
        key::parse(&text("keys/unknown-signer.txt")),
        Err(KeyError::UnknownSigner)
    );
    // The answers.
    for (name, state) in [
        ("answers/active.json", SubscriptionState::Active),
        ("answers/cancelled.json", SubscriptionState::Cancelled),
        ("answers/ended.json", SubscriptionState::Ended),
        ("answers/unknown.json", SubscriptionState::Unknown),
    ] {
        let (_, p) = answer::accept(&read(name), KEY_ID).unwrap();
        assert_eq!(p.state, state, "{name}");
    }
    assert!(answer::accept(&read("answers/wrong-signature.json"), KEY_ID).is_err());
}
