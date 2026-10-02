//! Plenipo's own PC side (`plenipo-remote`) and its stand-in phone, through **Plenipo's real
//! relay** rather than the tests' stand-in: pairing, signing in, asking, a removed phone, a PC that
//! goes away, a PC that is not Pro, and what the relay never logs or reads.

mod support;

use plenipo_licensing::SubscriptionState;
use plenipo_remote::link::{self, LinkHost};
use plenipo_remote::protocol::{Ask, Event, PairStep, SignedOutWhy};
use plenipo_remote::service::PairingView;
use plenipo_remote::stand_in::{NetPhone, PhoneError};
use std::sync::Arc;
use support::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_pairs_signs_in_and_asks_through_plenipos_relay() {
    let w = World::new().await;
    // Pairing, step by step (what `paired_phone` does), keeping the mailbox's name to look for.
    let code = w.new_code().await;
    let mailbox = code.mailbox();
    let mut phone = NetPhone::new(&w.relay.phone_address());
    phone.start_pairing(&code, "Frank's iPhone").await.unwrap();
    wait_for(
        || matches!(w.remote.view().pairing, Some(PairingView::Asking { .. })),
        "Is this your phone?",
    )
    .await;
    w.remote.answer_pairing(true).unwrap();
    assert_eq!(phone.finish_pairing().await.unwrap(), PairStep::Done);
    // Making the passkey signed it in: the first meeting needs no new check.
    let welcome = phone.meet(false).await.unwrap();
    assert!(welcome.signed_in);
    assert_eq!(welcome.pc_name, "Office PC");
    let reply = phone.ask(Ask::ReadControl).await.unwrap();
    assert!(reply.ok.is_some(), "{reply:?}");
    assert_eq!(
        w.app.carried.lock().unwrap().last().map(String::as_str),
        Some("see whether everything is stopped")
    );
    // A later meeting, once the sign-in lapsed, signs in with the passkey, through the relay.
    phone.close().await;
    wait_for(|| w.relay.stats().phones == 0, "the phone to leave").await;
    w.clock.advance(plenipo_remote::SIGNED_IN_IDLE_MS);
    w.remote.tick();
    let welcome = phone.meet(false).await.unwrap();
    assert!(!welcome.signed_in);
    let signed = phone.sign_in().await.unwrap();
    assert!(signed.ok.is_some(), "{signed:?}");
    let reply = phone.ask(Ask::ReadControl).await.unwrap();
    assert!(reply.ok.is_some(), "{reply:?}");
    assert_eq!(w.app.records("remote.signed_in").len(), 1);

    let stats = w.relay.stats();
    assert_eq!((stats.pcs, stats.phones, stats.connections), (1, 1, 2));
    assert!(stats.refused.is_empty(), "{:?}", stats.refused);

    // The relay passed everything sealed: the phone's name, which travelled inside, never showed.
    let seen = w.relay.seen();
    assert!(seen.len() >= 4, "{} sealed messages", seen.len());
    for sealed in &seen {
        assert!(!sealed.windows(5).any(|w| w == b"Frank"));
        assert!(!sealed.windows(7).any(|w| w == b"control"));
    }

    // Its logs hold counts, codes, and addresses only.
    let pass = phone.paired.as_ref().unwrap().pass.clone();
    let key = plenipo_relay_contract::b64::encode(&phone.paired.as_ref().unwrap().pc_key);
    for line in log_lines() {
        assert!(!line.contains(&pass), "a pass in the log: {line}");
        assert!(!line.contains(&mailbox), "a mailbox in the log: {line}");
        assert!(!line.contains(&key), "a key in the log: {line}");
        assert!(!line.contains("Frank"), "a name in the log: {line}");
        assert!(!line.contains("lk_01"), "a key ID in the log: {line}");
    }
    phone.close().await;
    wait_for(|| w.relay.stats().phones == 0, "the phone to leave").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pc_that_is_not_pro_is_refused() {
    let mut w = World::new().await;
    w.disconnect().await;
    for state in [SubscriptionState::Ended, SubscriptionState::Unknown] {
        *w.app.answer_state.lock().unwrap() = Some(state);
        let host: Arc<dyn LinkHost> = w.app.clone();
        let problem = link::connect_once(&w.remote, &w.relay.pc_address(), host.as_ref())
            .await
            .unwrap_err();
        assert!(problem.message.contains("not on Pro"), "{problem:?}");
    }
    assert_eq!(w.relay.refused(), ["not_pro", "not_pro"]);
    assert!(!w.relay.pc_connected());
    // Cancelled but still paid: served.
    *w.app.answer_state.lock().unwrap() = Some(SubscriptionState::Active);
    w.connect().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_phone_is_cut_off_at_once_and_its_pass_is_refused() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Lost phone").await;
    phone.meet(false).await.unwrap();
    let id = w.remote.view().devices[0].id.clone();
    w.remote.remove(&id).unwrap();
    assert_eq!(
        phone.event().await.unwrap(),
        Event::SignedOut {
            why: SignedOutWhy::Removed
        }
    );
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    // The relay refuses the dropped pass itself, before the PC hears of it.
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("bad_pass".into())
    );
    assert!(w.relay.refused().contains(&"bad_pass".to_owned()));
    assert_eq!(w.relay.stats().phones, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn when_the_pc_goes_away_its_phones_hear_pc_offline() {
    let mut w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.disconnect().await;
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("pc_offline".into())
    );
    assert_eq!(w.relay.stats().phones, 0);
    // Back: the phone's sign-in is still good, and nothing waited at the relay to run.
    w.connect().await;
    let welcome = phone.meet(false).await.unwrap();
    assert!(welcome.signed_in);
    assert!(w.app.carried.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stranger_cannot_pair_without_an_open_mailbox() {
    let w = World::new().await;
    let code = plenipo_remote::code::Code::new();
    let mut stranger = NetPhone::new(&w.relay.phone_address());
    assert_eq!(
        stranger.start_pairing(&code, "Stranger").await.unwrap_err(),
        PhoneError::Relay("mailbox_closed".into())
    );
    // A pass the PC never signed.
    let other = plenipo_remote::keys::PcKeys::new();
    let forged = pass_for(&other, &phone_id(1));
    assert_eq!(
        raw_phone(&w.relay, &pass_message(&forged))
            .await
            .unwrap_err(),
        "pc_offline"
    );
    assert_eq!(w.relay.refused(), ["mailbox_closed", "pc_offline"]);
}
