//! The relay against the internet: bad hellos and proofs, messages over the limit, mailbox tries,
//! one connection per PC, the limits per address and in all, timeouts, the off switch, `/healthz`,
//! and a clean stop. Every client here speaks the contract by hand.

mod support;

use std::time::Duration;

use plenipo_licensing::SubscriptionState;
use plenipo_relay::Limits;
use plenipo_relay_contract::b64;
use plenipo_relay_contract::wire::{self, PcToRelay, RelayToPc, RelayToPhone};
use plenipo_remote::keys::PcKeys;
use serde_json::json;
use support::*;

fn read_pc(text: &str) -> RelayToPc {
    wire::read(text).unwrap_or_else(|| panic!("not a relay message: {text}"))
}

fn read_phone(text: &str) -> RelayToPhone {
    wire::read(text).unwrap_or_else(|| panic!("not a relay message: {text}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_hello_or_proof_is_refused_and_closed() {
    let relay = start_relay(quick_limits()).await;
    // Not a hello at all.
    let mut ws = open(&relay.pc_address()).await.unwrap();
    assert!(matches!(
        read_pc(&next_text(&mut ws).await.unwrap()),
        RelayToPc::Challenge { .. }
    ));
    send_text(
        &mut ws,
        r#"{"t":"mailbox","mailbox":"00000000000000000000000000000000"}"#,
    )
    .await;
    assert_eq!(
        read_pc(&next_text(&mut ws).await.unwrap()),
        RelayToPc::Refused {
            code: "bad_hello".into()
        }
    );
    assert!(closed_within(&mut ws, Duration::from_secs(3)).await);
    // A hello whose proof is for another challenge.
    let keys = PcKeys::new();
    let mut ws = open(&relay.pc_address()).await.unwrap();
    let _challenge = next_text(&mut ws).await.unwrap();
    let hello = PcToRelay::Hello {
        v: 1,
        key: b64::encode(&keys.relay_public()),
        proof: b64::encode(&keys.relay_sign(wire::PC_PROOF_CONTEXT, "another nonce")),
        answer: weekly_answer(SubscriptionState::Active),
    };
    send_text(&mut ws, wire::write(&hello)).await;
    assert_eq!(
        read_pc(&next_text(&mut ws).await.unwrap()),
        RelayToPc::Refused {
            code: "bad_proof".into()
        }
    );
    // A good proof, but a cancelled subscription that has ended.
    assert_eq!(
        raw_pc(&relay, &keys, SubscriptionState::Cancelled)
            .await
            .unwrap_err(),
        "not_pro"
    );
    let c = counts(&relay);
    assert_eq!((c["bad_hello"], c["bad_proof"], c["not_pro"]), (1, 1, 1));
    assert_eq!(relay.stats().pcs, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pc_sends_to_its_own_phones_only_and_never_over_the_limit() {
    let relay = start_relay(quick_limits()).await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let mut phone = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(7))))
        .await
        .unwrap();
    let RelayToPc::Joined {
        conn,
        phone: Some(id),
        mailbox: false,
    } = read_pc(&next_text(&mut pc).await.unwrap())
    else {
        panic!("no joined");
    };
    assert_eq!(id, phone_id(7));
    // Both ways, as they are.
    let sealed = b64::encode(&[0xAB; 100]);
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Send {
            conn: conn.clone(),
            data: sealed.clone(),
        }),
    )
    .await;
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::Data {
            data: sealed.clone()
        }
    );
    send_text(&mut phone, data_message(&sealed)).await;
    assert_eq!(
        read_pc(&next_text(&mut pc).await.unwrap()),
        RelayToPc::Data {
            conn: conn.clone(),
            data: sealed.clone()
        }
    );
    // The largest allowed passes; one byte more does not.
    let largest = b64::encode(&[1u8; 65_535]);
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Send {
            conn: conn.clone(),
            data: largest.clone(),
        }),
    )
    .await;
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::Data { data: largest }
    );
    let too_big = b64::encode(&[1u8; 65_536]);
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Send {
            conn: conn.clone(),
            data: too_big.clone(),
        }),
    )
    .await;
    assert_eq!(
        read_pc(&next_text(&mut pc).await.unwrap()),
        RelayToPc::Error {
            code: "too_big".into()
        }
    );
    // A connection that is not one of its phones'.
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Send {
            conn: "c999999".into(),
            data: sealed.clone(),
        }),
    )
    .await;
    assert_eq!(
        read_pc(&next_text(&mut pc).await.unwrap()),
        RelayToPc::Error {
            code: "unknown_conn".into()
        }
    );
    // Another PC cannot reach this PC's phone.
    let other = PcKeys::new();
    let mut other_pc = raw_pc(&relay, &other, SubscriptionState::Active)
        .await
        .unwrap();
    send_text(
        &mut other_pc,
        wire::write(&PcToRelay::Send {
            conn: conn.clone(),
            data: sealed.clone(),
        }),
    )
    .await;
    assert_eq!(
        read_pc(&next_text(&mut other_pc).await.unwrap()),
        RelayToPc::Error {
            code: "unknown_conn".into()
        }
    );
    let stray = next_text_within(&mut phone, Duration::from_secs(1)).await;
    assert!(stray.is_none(), "reached the phone: {stray:?}");
    // The phone over the limit is refused and closed; the PC hears it left.
    send_text(&mut phone, data_message(&too_big)).await;
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::Refused {
            code: "too_big".into()
        }
    );
    assert!(closed_within(&mut phone, Duration::from_secs(3)).await);
    assert_eq!(
        read_pc(&next_text(&mut pc).await.unwrap()),
        RelayToPc::Left { conn }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pc_closes_and_drops_its_phones() {
    let relay = start_relay(quick_limits()).await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let pass = pass_for(&keys, &phone_id(3));
    let mut first = raw_phone(&relay, &pass_message(&pass)).await.unwrap();
    let RelayToPc::Joined { conn, .. } = read_pc(&next_text(&mut pc).await.unwrap()) else {
        panic!("no joined");
    };
    // Close: the phone is gone, and no `left` comes back (the PC asked).
    send_text(&mut pc, wire::write(&PcToRelay::Close { conn })).await;
    assert!(closed_within(&mut first, Duration::from_secs(3)).await);
    // Drop: each of that phone's connections hears bad_pass; the pass is refused until `until`.
    let mut second = raw_phone(&relay, &pass_message(&pass)).await.unwrap();
    let RelayToPc::Joined { .. } = read_pc(&next_text(&mut pc).await.unwrap()) else {
        panic!("no joined");
    };
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Drop {
            phone: phone_id(3),
            until: now_secs() + 3600,
        }),
    )
    .await;
    assert_eq!(
        read_phone(&next_text(&mut second).await.unwrap()),
        RelayToPhone::Refused {
            code: "bad_pass".into()
        }
    );
    assert!(closed_within(&mut second, Duration::from_secs(3)).await);
    assert_eq!(
        raw_phone(&relay, &pass_message(&pass)).await.unwrap_err(),
        "bad_pass"
    );
    // Another phone of the same PC is not dropped.
    let other = pass_for(&keys, &phone_id(4));
    assert!(raw_phone(&relay, &pass_message(&other)).await.is_ok());
    // A drop that has run out is forgotten.
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Drop {
            phone: phone_id(3),
            until: now_secs() - 1,
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(raw_phone(&relay, &pass_message(&pass)).await.is_ok());
    // The PC leaves: every phone hears pc_offline.
    let mut third = raw_phone(&relay, &pass_message(&pass)).await.unwrap();
    drop(pc);
    assert_eq!(
        read_phone(&next_text(&mut third).await.unwrap()),
        RelayToPhone::PcOffline
    );
    assert!(closed_within(&mut third, Duration::from_secs(3)).await);
    wait_for(|| relay.stats().connections == 0, "every connection to go").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mailbox_takes_three_phones_then_closes_and_runs_out() {
    let relay = start_relay(Limits {
        mailbox_life: Duration::from_millis(1500),
        ..quick_limits()
    })
    .await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let name = "589704e9466ff61d5f38c2fec2a2f8b8";
    assert_eq!(
        raw_phone(&relay, &mailbox_message(name)).await.unwrap_err(),
        "mailbox_closed"
    );
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Mailbox {
            mailbox: name.into(),
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut joined = Vec::new();
    for _ in 0..3 {
        joined.push(raw_phone(&relay, &mailbox_message(name)).await.unwrap());
        assert!(matches!(
            read_pc(&next_text(&mut pc).await.unwrap()),
            RelayToPc::Joined {
                phone: None,
                mailbox: true,
                ..
            }
        ));
    }
    assert_eq!(
        raw_phone(&relay, &mailbox_message(name)).await.unwrap_err(),
        "too_many_tries"
    );
    // A new mailbox replaces the old; the old name is closed.
    let other = "0123456789abcdef0123456789abcdef";
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Mailbox {
            mailbox: other.into(),
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        raw_phone(&relay, &mailbox_message(name)).await.unwrap_err(),
        "mailbox_closed"
    );
    assert!(raw_phone(&relay, &mailbox_message(other)).await.is_ok());
    // Closed by the PC.
    send_text(&mut pc, wire::write(&PcToRelay::CloseMailbox)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        raw_phone(&relay, &mailbox_message(other))
            .await
            .unwrap_err(),
        "mailbox_closed"
    );
    // Open too long: gone by itself.
    send_text(
        &mut pc,
        wire::write(&PcToRelay::Mailbox {
            mailbox: other.into(),
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1700)).await;
    assert_eq!(
        raw_phone(&relay, &mailbox_message(other))
            .await
            .unwrap_err(),
        "mailbox_closed"
    );
    // A name that is not 32 hex digits is a bad first message.
    assert_eq!(
        raw_phone(&relay, &mailbox_message("not-a-mailbox"))
            .await
            .unwrap_err(),
        "bad_pass"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_pc_connection_replaces_the_old_one() {
    let relay = start_relay(quick_limits()).await;
    let keys = PcKeys::new();
    let mut old = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let mut phone = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(1))))
        .await
        .unwrap();
    assert!(matches!(
        read_pc(&next_text(&mut old).await.unwrap()),
        RelayToPc::Joined { .. }
    ));
    let mut new = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    assert!(closed_within(&mut old, Duration::from_secs(3)).await);
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::PcOffline
    );
    assert_eq!(relay.stats().pcs, 1);
    // The new connection serves the phone when it comes back.
    let mut again = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(1))))
        .await
        .unwrap();
    assert!(matches!(
        read_pc(&next_text(&mut new).await.unwrap()),
        RelayToPc::Joined { .. }
    ));
    send_text(&mut again, data_message("AAAA")).await;
    assert!(matches!(
        read_pc(&next_text(&mut new).await.unwrap()),
        RelayToPc::Data { .. }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_door_keeps_the_limits_per_address_and_in_all() {
    let relay = start_relay(Limits {
        connections_per_address: 2,
        ..quick_limits()
    })
    .await;
    let _a = open(&relay.phone_address()).await.unwrap();
    let _b = open(&relay.phone_address()).await.unwrap();
    let third = open(&relay.phone_address()).await.unwrap_err();
    assert_eq!(door_status(&third), Some(429));
    drop(_a);
    wait_for(|| relay.stats().connections == 1, "a connection to go").await;
    assert!(open(&relay.phone_address()).await.is_ok());
    assert_eq!(relay.stats().turned_away, 1);

    let relay = start_relay(Limits {
        connections: 1,
        ..quick_limits()
    })
    .await;
    let _only = open(&relay.pc_address()).await.unwrap();
    let full = open(&relay.phone_address()).await.unwrap_err();
    assert_eq!(door_status(&full), Some(503));
    // /healthz still answers while full.
    assert_eq!(http_get(&relay, "/healthz").await.0, 200);

    // Too many refusals from one address: the door says wait.
    let relay = start_relay(Limits {
        tries_per_address_per_minute: 2,
        ..quick_limits()
    })
    .await;
    for _ in 0..2 {
        assert_eq!(
            raw_phone(&relay, "{\"t\":\"nonsense\"}").await.unwrap_err(),
            "bad_pass"
        );
    }
    let waited = open(&relay.phone_address()).await.unwrap_err();
    assert_eq!(door_status(&waited), Some(429));

    // Too many new connections in a minute.
    let relay = start_relay(Limits {
        new_per_address_per_minute: 3,
        ..quick_limits()
    })
    .await;
    for _ in 0..3 {
        drop(open(&relay.phone_address()).await.unwrap());
    }
    assert_eq!(
        door_status(&open(&relay.phone_address()).await.unwrap_err()),
        Some(429)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_that_says_nothing_is_closed_and_pongs_keep_one_alive() {
    let relay = start_relay(Limits {
        first_message: Duration::from_millis(400),
        idle: Duration::from_millis(900),
        ping_every: Duration::from_millis(300),
        ..Limits::default()
    })
    .await;
    // No first message in time.
    let mut quiet = open(&relay.pc_address()).await.unwrap();
    let started = tokio::time::Instant::now();
    assert!(closed_within(&mut quiet, Duration::from_secs(3)).await);
    assert!(started.elapsed() < Duration::from_secs(2));
    // A PC that stops answering (not even pongs) is closed after the idle time.
    let keys = PcKeys::new();
    let pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(relay.stats().pcs, 0, "the quiet PC is gone");
    drop(pc);
    // A PC that answers the relay's pings (reading does that) stays as long as it likes.
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    assert!(
        !closed_within(&mut pc, Duration::from_millis(2000)).await,
        "pongs keep it"
    );
    assert_eq!(relay.stats().pcs, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_over_its_budget_of_messages_is_told_and_closed() {
    let relay = start_relay(Limits {
        messages_per_minute: 4,
        ..quick_limits()
    })
    .await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    // The hello was the first message of this minute; three more make four; the next is too fast.
    // (A minute's edge could fall in between: then the budget simply starts again.)
    for _ in 0..3 {
        send_text(&mut pc, wire::write(&PcToRelay::CloseMailbox)).await;
    }
    send_text(&mut pc, wire::write(&PcToRelay::CloseMailbox)).await;
    send_text(&mut pc, wire::write(&PcToRelay::CloseMailbox)).await;
    assert_eq!(
        read_pc(&next_text(&mut pc).await.unwrap()),
        RelayToPc::Error {
            code: "too_many_tries".into()
        }
    );
    assert!(closed_within(&mut pc, Duration::from_secs(3)).await);
    assert_eq!(counts(&relay)["too_many_tries"], 1);
    // A phone the same.
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let mut phone = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(9))))
        .await
        .unwrap();
    let _ = next_text(&mut pc).await;
    for _ in 0..5 {
        send_text(&mut phone, data_message("AAAA")).await;
    }
    let mut refused = false;
    while let Some(text) = next_text(&mut phone).await {
        if read_phone(&text)
            == (RelayToPhone::Refused {
                code: "too_many_tries".into(),
            })
        {
            refused = true;
        }
    }
    assert!(refused, "the phone was told");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_off_switch_closes_everything_and_turns_new_ones_away() {
    let relay = start_relay(quick_limits()).await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let mut phone = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(2))))
        .await
        .unwrap();
    let _ = next_text(&mut pc).await;
    assert_eq!(http_get(&relay, "/healthz").await, (200, "ok\n".into()));
    relay.set_off(true);
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::PcOffline
    );
    assert!(closed_within(&mut phone, Duration::from_secs(3)).await);
    assert!(closed_within(&mut pc, Duration::from_secs(3)).await);
    assert_eq!(
        door_status(&open(&relay.pc_address()).await.unwrap_err()),
        Some(503)
    );
    assert_eq!(http_get(&relay, "/healthz").await, (503, "off\n".into()));
    assert!(relay.stats().off);
    relay.set_off(false);
    assert!(raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .is_ok());
    assert_eq!(http_get(&relay, "/healthz").await.0, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_door_answers_plain_http_in_plain_words() {
    let relay = start_relay(quick_limits()).await;
    assert_eq!(http_get(&relay, "/healthz").await, (200, "ok\n".into()));
    assert_eq!(http_get(&relay, "/").await, (404, "not found\n".into()));
    assert_eq!(http_get(&relay, "/plenipo/v1/pc").await.0, 404);
    assert_eq!(http_get(&relay, "/plenipo/v1/phone?x=1").await.0, 404);
    let elsewhere = open(&format!("http://{}/plenipo/v2/pc", relay.address()))
        .await
        .unwrap_err();
    assert_eq!(door_status(&elsewhere), Some(404));
    // Not HTTP at all.
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(relay.address())
        .await
        .unwrap();
    stream
        .write_all(b"\x16\x03\x01 not http\r\n\r\n")
        .await
        .unwrap();
    let mut text = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut text)).await;
    assert!(text.starts_with("HTTP/1.1 400"), "{text}");
    assert_eq!(relay.stats().connections, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_closes_every_connection_and_the_port() {
    let relay = start_relay(quick_limits()).await;
    let keys = PcKeys::new();
    let mut pc = raw_pc(&relay, &keys, SubscriptionState::Active)
        .await
        .unwrap();
    let mut phone = raw_phone(&relay, &pass_message(&pass_for(&keys, &phone_id(5))))
        .await
        .unwrap();
    let _ = next_text(&mut pc).await;
    relay.shutdown(Duration::from_secs(5)).await;
    assert_eq!(
        read_phone(&next_text(&mut phone).await.unwrap()),
        RelayToPhone::PcOffline
    );
    assert!(closed_within(&mut phone, Duration::from_secs(3)).await);
    assert!(closed_within(&mut pc, Duration::from_secs(3)).await);
    assert_eq!(relay.stats().connections, 0);
    assert!(
        tokio::net::TcpStream::connect(relay.address())
            .await
            .is_err(),
        "the port is closed"
    );
    let _ = json!({});
}
