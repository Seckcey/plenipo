//! A PC's connection (`/plenipo/v1/pc`): the challenge, the hello with its proof and 8 West's
//! signed weekly answer, then the PC's commands until it leaves.

use ed25519_dalek::{Signature, VerifyingKey};
use plenipo_licensing::answer::AnswerPayload;
use plenipo_licensing::SubscriptionState;
use plenipo_relay_contract::wire::{self, codes, PcToRelay, RelayToPc, PC_PROOF_CONTEXT};
use plenipo_relay_contract::{b64, fingerprint};

use crate::hub::Hub;
use crate::limits::AddressKey;
use crate::line::{Line, Next};

/// A weekly answer older than this does not show Pro (the contract: less than 30 days).
const FRESH_SECS: i64 = 30 * 86_400;
/// Messages the relay could not read from one PC before it gives up on it.
const MOST_UNREADABLE: u32 = 20;

/// Does a checked weekly answer show a paid, current license at `now`?
pub(crate) fn shows_pro(answer: &AnswerPayload, now: i64) -> bool {
    let fresh = now.saturating_sub(answer.as_of) < FRESH_SECS;
    fresh
        && match answer.state {
            SubscriptionState::Active => true,
            SubscriptionState::Cancelled => answer.ends_at.is_some_and(|e| e > now),
            SubscriptionState::Ended | SubscriptionState::Unknown => false,
        }
}

fn refuse(hub: &Hub, line: &Line, address: AddressKey, code: &str) {
    hub.refused(code, address);
    line.link.send(wire::write(&RelayToPc::Refused {
        code: code.to_owned(),
    }));
    line.link.close();
}

fn error(line: &Line, code: &str) {
    line.link.send(wire::write(&RelayToPc::Error {
        code: code.to_owned(),
    }));
}

pub(crate) async fn serve(hub: &Hub, line: &mut Line, address: AddressKey) {
    let nonce = b64::encode(&crate::random32());
    line.link.send(wire::write(&RelayToPc::Challenge {
        nonce: nonce.clone(),
    }));
    let Next::Text(first) = line.first(hub.config.limits.first_message).await else {
        return;
    };
    let Some(PcToRelay::Hello {
        v: 1,
        key,
        proof,
        answer,
    }) = wire::read(&first)
    else {
        refuse(hub, line, address, codes::BAD_HELLO);
        return;
    };
    let (Some(key), Some(proof)) = (
        b64::decode_exact::<32>(&key),
        b64::decode_exact::<64>(&proof),
    ) else {
        refuse(hub, line, address, codes::BAD_HELLO);
        return;
    };
    let proven = VerifyingKey::from_bytes(&key).is_ok_and(|k| {
        k.verify_strict(
            format!("{PC_PROOF_CONTEXT}{nonce}").as_bytes(),
            &Signature::from_bytes(&proof),
        )
        .is_ok()
    });
    if !proven {
        refuse(hub, line, address, codes::BAD_PROOF);
        return;
    }
    // 8 West's signature with the public license keys, then what the answer says. The key ID
    // inside is looked at here and nowhere else: never kept, never logged. What stays, while
    // this PC is connected, is its mark (a salted hash this run alone can make), to count the
    // PCs on one license.
    let now = hub.now();
    let checked = plenipo_licensing::answer::verify(&answer)
        .ok()
        .filter(|a| shows_pro(a, now));
    drop(answer);
    let Some(checked) = checked else {
        refuse(hub, line, address, codes::NOT_PRO);
        return;
    };
    let license = hub.license_mark(&checked.key_id);
    drop(checked);
    let pc = fingerprint(&key);
    let generation = match hub.register_pc(&pc, key, line.link.clone(), license, address) {
        Ok(generation) => generation,
        Err(code) => {
            refuse(hub, line, address, code);
            return;
        }
    };
    line.link
        .send(wire::write(&RelayToPc::Welcome { pc: pc.clone() }));
    log::info!("pc connected ({address})");

    let mut unreadable = 0u32;
    loop {
        match line.next().await {
            Next::Text(text) => {
                let Some(message) = wire::read::<PcToRelay>(&text) else {
                    unreadable += 1;
                    if unreadable > MOST_UNREADABLE {
                        log::info!("pc sent too much the relay could not read ({address})");
                        line.link.close();
                        break;
                    }
                    continue;
                };
                match message {
                    PcToRelay::Mailbox { mailbox } if crate::is_mailbox_name(&mailbox) => {
                        hub.open_mailbox(&pc, generation, mailbox);
                    }
                    PcToRelay::CloseMailbox => hub.close_mailbox(&pc, generation),
                    PcToRelay::Drop { phone, until } if b64::is_id(&phone, 16) => {
                        hub.drop_phone(&pc, generation, &phone, until);
                    }
                    PcToRelay::Send { conn, data } => {
                        if !crate::fits(&data) {
                            error(line, codes::TOO_BIG);
                        } else if hub.send_to_phone(&pc, &conn, data).is_err() {
                            error(line, codes::UNKNOWN_CONN);
                        }
                    }
                    PcToRelay::Close { conn } => hub.close_phone(&pc, &conn),
                    PcToRelay::Mailbox { .. }
                    | PcToRelay::Drop { .. }
                    | PcToRelay::Hello { .. } => {
                        unreadable += 1;
                    }
                }
            }
            Next::TooFast => {
                hub.refused(codes::TOO_MANY_TRIES, address);
                error(line, codes::TOO_MANY_TRIES);
                line.link.close();
                break;
            }
            Next::Gone => break,
        }
    }
    hub.unregister_pc(&pc, generation);
    log::info!("pc left ({address})");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(state: SubscriptionState, as_of: i64, ends_at: Option<i64>) -> AnswerPayload {
        AnswerPayload {
            v: 1,
            key_id: "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T".into(),
            state,
            paid_through: None,
            ends_at,
            as_of,
            signer: "prod-1".into(),
        }
    }

    #[test]
    fn only_a_fresh_paid_answer_shows_pro() {
        let now = 1_791_000_000;
        let day = 86_400;
        assert!(shows_pro(
            &answer(SubscriptionState::Active, now, None),
            now
        ));
        assert!(shows_pro(
            &answer(SubscriptionState::Active, now - 29 * day, None),
            now
        ));
        assert!(!shows_pro(
            &answer(SubscriptionState::Active, now - 30 * day, None),
            now
        ));
        assert!(shows_pro(
            &answer(SubscriptionState::Cancelled, now, Some(now + 1)),
            now
        ));
        assert!(!shows_pro(
            &answer(SubscriptionState::Cancelled, now, Some(now)),
            now
        ));
        assert!(!shows_pro(
            &answer(SubscriptionState::Ended, now, Some(now - 1)),
            now
        ));
        assert!(!shows_pro(
            &answer(SubscriptionState::Unknown, now, None),
            now
        ));
    }

    #[test]
    fn mailbox_names_and_sealed_sizes_follow_the_contract() {
        assert!(crate::is_mailbox_name("589704e9466ff61d5f38c2fec2a2f8b8"));
        assert!(!crate::is_mailbox_name("589704E9466FF61D5F38C2FEC2A2F8B8"));
        assert!(!crate::is_mailbox_name("589704e9466ff61d5f38c2fec2a2f8b"));
        assert!(crate::fits(&b64::encode(&[1u8; 65_535])));
        assert!(!crate::fits(&b64::encode(&[1u8; 65_536])));
        assert!(!crate::fits("not base64url!"));
    }
}
