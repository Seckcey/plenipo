//! Block, Unblock, Report, and Delete my Community data from this PC (ADR-167, ADR-168 §3),
//! against the stand-in service.

mod common;

use common::{Pc, World};
use plenipo_community::block_report::ReportOf;
use plenipo_community::messages;
use plenipo_community::wire::{ReportReason, ReportWhat};
use plenipo_ledger::community::PersonState;

const HELLO: &str = "Hi Frank, want to buy cheap followers?";

async fn pick_up(pc: &Pc) -> messages::PickedUp {
    pc.community.pick_up(&pc.ledger, 0).await.unwrap()
}

/// Pat writes to Frank, and Frank picks it up.
async fn pat_writes(world: &World, pat: &Pc, frank: &Pc, text: &str) -> String {
    world.later(5);
    let sent = pat
        .community
        .send_message(&pat.ledger, &frank.member_id(), "frank-g", text, None)
        .await
        .unwrap();
    pick_up(frank).await;
    sent.item_id
}

#[tokio::test]
async fn a_block_works_at_once_and_the_blocked_person_is_not_told() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());
    pat_writes(&world, &pat, &frank, HELLO).await;

    frank
        .community
        .block(&frank.ledger, &p, "pat-lee")
        .await
        .unwrap();
    let person = frank.ledger.community_person(&p).unwrap().unwrap();
    assert!(person.blocked);
    assert_ne!(person.state, PersonState::Accepted);
    assert!(frank
        .events
        .names()
        .contains(&"community.blocked".to_owned()));
    // What is on Frank's PC stays.
    assert_eq!(
        messages::conversation(&frank.ledger, &p, None)
            .unwrap()
            .unwrap()
            .messages
            .len(),
        1
    );

    // Pat's next message isn't delivered, and Pat is told nothing else.
    world.later(5);
    let refused = pat
        .community
        .send_message(&pat.ledger, &f, "frank-g", "Hello?", None)
        .await;
    assert!(refused.is_err());
    assert!(!pat.events.all_text().contains("blocked"));
    assert_eq!(pick_up(&frank).await.kept, 0);

    // Frank can't write to Pat while Pat is blocked, and nothing is sent trying.
    let before = world.service.seen().len();
    let refused = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "Stop", None)
        .await
        .unwrap_err();
    assert_eq!(refused.0, "You blocked @pat-lee. Unblock them first.");
    assert_eq!(world.service.seen().len(), before);

    // Settings → Community → Blocked lists Pat, also on Frank's other PC.
    let laptop = world.another_pc(&frank).await;
    let blocked = laptop.community.blocked(&laptop.ledger).await.unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].name, "pat-lee");
    assert!(laptop.ledger.community_person(&p).unwrap().unwrap().blocked);

    frank.community.unblock(&frank.ledger, &p).await.unwrap();
    assert!(!frank.ledger.community_person(&p).unwrap().unwrap().blocked);
    assert!(frank
        .events
        .names()
        .contains(&"community.unblocked".to_owned()));
    assert!(frank
        .community
        .blocked(&frank.ledger)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn this_pc_refuses_a_blocked_person_even_if_8_west_hands_their_message_out() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let p = pat.member_id();
    pat_writes(&world, &pat, &frank, HELLO).await;
    frank.community.accept(&frank.ledger, &p).await.unwrap();
    pick_up(&pat).await;
    // Only this PC knows of the block (as if 8 West had missed it).
    let mut person = frank.ledger.community_person(&p).unwrap().unwrap();
    person.blocked = true;
    frank.ledger.community_put_person(&person).unwrap();

    world.later(5);
    pat.community
        .send_message(
            &pat.ledger,
            &frank.member_id(),
            "frank-g",
            "Still there?",
            None,
        )
        .await
        .unwrap();
    let picked = pick_up(&frank).await;
    assert_eq!(picked.kept, 0);
    assert_eq!(picked.dropped, 1);
    assert!(frank.events.all_text().contains("\"blocked\""));
    assert_eq!(
        pick_up(&frank).await.dropped,
        0,
        "done with it: it never comes back"
    );
}

#[tokio::test]
async fn a_report_carries_the_ticked_messages_with_their_proof_and_nothing_else() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());
    let first = pat_writes(&world, &pat, &frank, HELLO).await;
    frank.community.accept(&frank.ledger, &p).await.unwrap();
    pick_up(&pat).await;
    let second = pat_writes(&world, &pat, &frank, "Last chance, 50% off").await;
    world.later(5);
    let mine = frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "No thanks.", None)
        .await
        .unwrap();

    frank
        .community
        .report(
            &frank.ledger,
            &p,
            &ReportOf::Messages {
                item_ids: vec![first.clone(), second.clone()],
            },
            ReportReason::Scam,
            "They keep selling followers.",
        )
        .await
        .unwrap();
    let reports = world.service.reports();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].about, p);
    assert_eq!(reports[0].by, f);
    assert_eq!(reports[0].reason, ReportReason::Scam);
    assert_eq!(reports[0].what, ReportWhat::Items);
    assert_eq!(reports[0].items.len(), 2, "8 West checked both proofs");

    // The Ledger says who, what, and why; never the note or the words.
    let events = frank.events.all_text();
    assert!(events.contains("community.reported"));
    assert!(events.contains("scam") && events.contains("pat-lee"));
    assert!(!events.contains("followers") && !events.contains("50%"));

    // Only their messages, still on this PC, and at most 20, go in a report; nothing is sent
    // trying otherwise.
    let before = world.service.seen().len();
    for (of, why) in [
        (
            ReportOf::Messages {
                item_ids: vec![mine.item_id.clone()],
            },
            "your own message",
        ),
        (ReportOf::Messages { item_ids: vec![] }, "none ticked"),
        (
            ReportOf::Messages {
                item_ids: vec![first.clone(); 21],
            },
            "too many",
        ),
    ] {
        assert!(
            frank
                .community
                .report(&frank.ledger, &p, &of, ReportReason::Spam, "")
                .await
                .is_err(),
            "{why}"
        );
    }
    frank
        .community
        .delete_for_me(&frank.ledger, &first)
        .unwrap();
    assert!(frank
        .community
        .report(
            &frank.ledger,
            &p,
            &ReportOf::Messages {
                item_ids: vec![first]
            },
            ReportReason::Spam,
            ""
        )
        .await
        .is_err());
    assert_eq!(world.service.seen().len(), before);

    // A person or a profile needs no messages.
    frank
        .community
        .report(
            &frank.ledger,
            &p,
            &ReportOf::Profile,
            ReportReason::Impersonation,
            "",
        )
        .await
        .unwrap();
    assert_eq!(world.service.reports().len(), 2);
}

#[tokio::test]
async fn delete_my_community_data_empties_this_pc_only() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    pat_writes(&world, &pat, &frank, HELLO).await;
    let before = world.service.seen().len();
    frank.community.delete_my_data(&frank.ledger).unwrap();
    assert!(messages::conversations(&frank.ledger).unwrap().is_empty());
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
    assert!(frank
        .events
        .names()
        .contains(&"community.data_deleted".to_owned()));
    // Pat keeps theirs.
    assert_eq!(messages::conversations(&pat.ledger).unwrap().len(), 1);
}
