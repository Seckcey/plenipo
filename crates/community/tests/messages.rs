//! Private messages (ADR-164), against the stand-in service: requests, accepting, sealing for
//! every PC, picking up with every check, the safety code, reactions, Delete for me, leaving, and
//! a dishonest service. Each person is on their own PC, with their own Ledger.

mod common;

use common::{Pc, World};
use plenipo_community::messages::{self, PICK_UP_WAIT_SECS};
use plenipo_community::stand_in::Bad;

const HELLO: &str = "Hi Pat, do you build decks?";

async fn pick_up(pc: &Pc) -> messages::PickedUp {
    pc.community.pick_up(&pc.ledger, 0).await.unwrap()
}

fn texts(pc: &Pc, with: &str) -> Vec<String> {
    messages::conversation(&pc.ledger, with, None)
        .unwrap()
        .map(|c| c.messages.into_iter().filter_map(|m| m.text).collect())
        .unwrap_or_default()
}

fn state(pc: &Pc, with: &str) -> String {
    messages::conversations(&pc.ledger)
        .unwrap()
        .into_iter()
        .find(|c| c.member_id == with)
        .map(|c| c.state)
        .unwrap_or_default()
}

#[tokio::test]
async fn a_first_message_is_a_request_until_it_is_accepted() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, false).await;
    let (f, p) = (frank.member_id(), pat.member_id());

    let sent = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", HELLO, None)
        .await
        .unwrap();
    assert_eq!(sent.state, "delivered");
    assert!(sent.accepted_at.is_some());
    assert_eq!(state(&frank, &p), "requestedByMe");
    assert!(frank
        .events
        .names()
        .contains(&"community.conversation_started".to_owned()));

    // Nothing more until Pat accepts, and nothing is sent trying.
    let before = world.service.seen().len();
    let refused = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "Hello?", None)
        .await
        .unwrap_err();
    assert_eq!(refused.0, "Wait for @pat-lee to accept your first message.");
    assert_eq!(world.service.seen().len(), before);

    // Pat's PC picks it up: it waits in Requests, with its proof kept for a report.
    world.later(10);
    let picked = pick_up(&pat).await;
    assert_eq!(picked.kept, 1);
    assert_eq!(state(&pat, &f), "requestedByThem");
    let request = messages::conversation(&pat.ledger, &f, None)
        .unwrap()
        .unwrap();
    assert_eq!(request.person.name, "frank-g");
    assert_eq!(request.messages[0].text.as_deref(), Some(HELLO));
    assert!(request.messages[0].request && request.messages[0].reportable);
    // Picked up once only: 8 West deleted Pat's copy.
    assert_eq!(pick_up(&pat).await.kept, 0);

    // Pat, without Pro, can't write first, but accepts and answers.
    let refused = pat
        .community
        .send_message(&pat.ledger, &f, "frank-g", "Yes!", None)
        .await
        .unwrap_err();
    assert!(refused.0.contains("Accept"), "{}", refused.0);
    pat.community.accept(&pat.ledger, &f).await.unwrap();
    assert_eq!(state(&pat, &f), "accepted");
    world.later(10);
    pat.community
        .send_message(&pat.ledger, &f, "frank-g", "Yes, decks and porches.", None)
        .await
        .unwrap();

    // Frank hears that Pat accepted, and gets the answer.
    world.later(10);
    pick_up(&frank).await;
    assert_eq!(state(&frank, &p), "accepted");
    assert_eq!(texts(&frank, &p), [HELLO, "Yes, decks and porches."]);
    let names = frank.events.names();
    assert!(names.contains(&"community.conversation_accepted".to_owned()));

    // Never a word of a message in the Ledger's events.
    for pc in [&frank, &pat] {
        let events = pc.events.all_text();
        assert!(!events.contains("decks"), "{events}");
    }
}

#[tokio::test]
async fn your_other_pcs_get_what_you_sent_and_the_same_safety_code_shows_on_both_sides() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let laptop = world.another_pc(&frank).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());

    frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", HELLO, None)
        .await
        .unwrap();
    world.later(5);
    assert_eq!(pick_up(&laptop).await.kept, 1);
    let copy = messages::conversation(&laptop.ledger, &p, None)
        .unwrap()
        .unwrap();
    assert!(copy.messages[0].outgoing, "yours, from your other PC");
    assert_eq!(copy.messages[0].text.as_deref(), Some(HELLO));

    pick_up(&pat).await;
    let at_frank = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    let at_pat = messages::conversation(&pat.ledger, &f, None)
        .unwrap()
        .unwrap();
    let code = at_frank.safety_code.clone().expect("a safety code");
    assert_eq!(code.len(), 14, "{code}");
    assert_eq!(at_pat.safety_code.as_deref(), Some(code.as_str()));
    assert!(!at_frank.person.computers_changed);

    // Pat signs in on a second PC: Frank's next message says Pat's computers changed.
    let _pats_laptop = world.another_pc(&pat).await;
    pat.community.accept(&pat.ledger, &f).await.unwrap();
    world.later(5);
    pick_up(&frank).await;
    frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "One more thing", None)
        .await
        .unwrap();
    let now = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    assert_ne!(now.safety_code.as_deref(), Some(code.as_str()));
    assert!(now.person.computers_changed);
    frank
        .community
        .safety_code_checked(&frank.ledger, &p)
        .unwrap();
    let checked = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    assert!(!checked.person.computers_changed);
}

#[tokio::test]
async fn reactions_come_and_go_and_delete_for_me_stays_on_this_pc() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());
    let sent = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", HELLO, None)
        .await
        .unwrap();
    pick_up(&pat).await;
    pat.community.accept(&pat.ledger, &f).await.unwrap();

    assert!(pat
        .community
        .react(&pat.ledger, &sent.item_id, Some("🦄"))
        .await
        .is_err());
    pat.community
        .react(&pat.ledger, &sent.item_id, Some("👍"))
        .await
        .unwrap();
    world.later(5);
    pick_up(&frank).await;
    let seen = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    assert_eq!(seen.messages.len(), 1, "a reaction is not a message");
    assert_eq!(seen.messages[0].reactions.len(), 1);
    assert_eq!(seen.messages[0].reactions[0].emoji, "👍");
    assert!(!seen.messages[0].reactions[0].mine);

    world.later(5);
    pat.community
        .react(&pat.ledger, &sent.item_id, None)
        .await
        .unwrap();
    world.later(5);
    pick_up(&frank).await;
    let taken_back = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    assert!(taken_back.messages[0].reactions.is_empty());

    // Delete for me: gone here, still on Pat's PC, and nothing was sent.
    let before = world.service.seen().len();
    frank
        .community
        .delete_for_me(&frank.ledger, &sent.item_id)
        .unwrap();
    assert!(texts(&frank, &p).is_empty());
    assert_eq!(texts(&pat, &f), [HELLO]);
    assert_eq!(world.service.seen().len(), before);
}

#[tokio::test]
async fn leaving_a_conversation_stops_their_messages() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());
    frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", HELLO, None)
        .await
        .unwrap();
    pick_up(&pat).await;
    pat.community.accept(&pat.ledger, &f).await.unwrap();
    pick_up(&frank).await;

    pat.community
        .leave_conversation(&pat.ledger, &f)
        .await
        .unwrap();
    assert_eq!(state(&pat, &f), "leftByMe");
    assert!(pat
        .events
        .names()
        .contains(&"community.conversation_left".to_owned()));
    world.later(5);
    let refused = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "Are you there?", None)
        .await
        .unwrap_err();
    assert!(!refused.0.is_empty());
    let kept = messages::conversation(&frank.ledger, &p, None)
        .unwrap()
        .unwrap();
    let last = kept.messages.last().unwrap();
    assert_eq!(last.text.as_deref(), Some("Are you there?"));
    assert_eq!(last.state, "notDelivered");
}

#[tokio::test]
async fn starting_a_conversation_is_part_of_pro_and_leaves_nothing_behind() {
    let world = World::new();
    let pat = world.person("Pat Lee", "pat-lee", 1985, false).await;
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let refused = pat
        .community
        .send_message(&pat.ledger, &frank.member_id(), "frank-g", HELLO, None)
        .await
        .unwrap_err();
    assert!(!refused.0.is_empty());
    assert!(
        texts(&pat, &frank.member_id()).is_empty(),
        "nothing left behind"
    );
    assert_eq!(pick_up(&frank).await.kept, 0);
}

#[tokio::test]
async fn a_dishonest_service_cannot_change_repeat_or_make_up_a_message() {
    for bad in [Bad::ChangeSealed, Bad::ChangeStamp, Bad::Invent] {
        let world = World::new();
        let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
        let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
        frank
            .community
            .send_message(&frank.ledger, &pat.member_id(), "pat-lee", HELLO, None)
            .await
            .unwrap();
        world.service.set_bad(bad);
        let picked = pick_up(&pat).await;
        assert!(picked.dropped >= 1, "{bad:?}");
        // A changed item is dropped; a made-up one is dropped beside the real one.
        let kept: Vec<String> = messages::conversations(&pat.ledger)
            .unwrap()
            .iter()
            .flat_map(|c| texts(&pat, &c.member_id))
            .collect();
        if bad == Bad::Invent {
            assert_eq!(picked.kept, 1);
            assert_eq!(kept, [HELLO]);
        } else {
            assert_eq!(picked.kept, 0, "{bad:?}");
            assert!(kept.is_empty(), "{bad:?}");
        }
        let events = pat.events.all_text();
        assert!(events.contains("community.item_dropped"), "{bad:?}");
        assert!(!events.contains("decks"), "{bad:?}");
    }

    // Handed out again: kept once.
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    frank
        .community
        .send_message(&frank.ledger, &pat.member_id(), "pat-lee", HELLO, None)
        .await
        .unwrap();
    assert_eq!(pick_up(&pat).await.kept, 1);
    world.service.set_bad(Bad::Replay);
    let again = pick_up(&pat).await;
    assert_eq!(again.kept, 0);
    assert_eq!(texts(&pat, &frank.member_id()), [HELLO]);
}

#[tokio::test]
async fn what_cannot_be_a_message_is_refused_before_anything_is_sent() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let p = pat.member_id();
    let before = world.service.seen().len();
    let too_long = "x".repeat(4001);
    for (to, name, text, reply) in [
        (p.as_str(), "pat-lee", "   ", None),
        (p.as_str(), "pat-lee", too_long.as_str(), None),
        ("../me", "pat-lee", HELLO, None),
        (p.as_str(), "Pat Lee", HELLO, None),
        (p.as_str(), "pat-lee", HELLO, Some("not-an-item")),
    ] {
        assert!(frank
            .community
            .send_message(&frank.ledger, to, name, text, reply)
            .await
            .is_err());
    }
    let me = frank.member_id();
    assert!(frank
        .community
        .send_message(&frank.ledger, &me, "frank-g", HELLO, None)
        .await
        .is_err());
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
    assert!(messages::conversations(&frank.ledger).unwrap().is_empty());
    // Waiting never longer than the contract allows.
    assert!(PICK_UP_WAIT_SECS <= 25);
}

#[tokio::test]
async fn a_message_8_west_cannot_check_just_now_waits_for_the_next_pick_up() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    frank
        .community
        .send_message(&frank.ledger, &pat.member_id(), "pat-lee", HELLO, None)
        .await
        .unwrap();
    // 8 West answers the inbox, then is too busy to list Frank's computers.
    world.service.set_busy_after(1);
    let picked = pat.community.pick_up(&pat.ledger, 0).await;
    world.service.set_busy(false);
    if let Ok(picked) = picked {
        assert_eq!(picked.kept, 0);
        assert_eq!(picked.dropped, 0, "never dropped for a busy moment");
    }
    assert!(!pat.events.all_text().contains("item_dropped"));
    // Next time it is there.
    assert_eq!(pick_up(&pat).await.kept, 1);
    assert_eq!(texts(&pat, &frank.member_id()), [HELLO]);
}
