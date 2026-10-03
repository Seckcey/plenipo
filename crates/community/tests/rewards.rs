//! Rewards for taking part (ADR-169, ADR-172), against the stand-in service: points, places,
//! badges, the leaderboard, and Getting started. 8 West works out points; a PC never sends them.

mod common;

use common::World;
use plenipo_community::profile::{ProfileDraft, ProfileShown, Tile, TileStatus};
use plenipo_community::wire::{Badge, PointsReason};

#[tokio::test]
async fn points_and_badges_show_and_the_ledger_records_each_once() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let f = frank.member_id();
    world
        .service
        .give_points(&f, 2, PointsReason::ContactAccepted);
    world.later(60);
    world.service.give_points(&f, 3, PointsReason::Thanks);
    world
        .service
        .set_badges(&f, &[Badge::FoundingMember, Badge::Unknown]);

    let points = frank.community.points().await.unwrap();
    assert_eq!(points.total, 5);
    assert_eq!(
        points.badges,
        ["founding_member"],
        "a badge this copy doesn't know is left out"
    );
    assert_eq!(points.recent[0].reason, "thanks", "newest first");
    let events = || frank.events.all_text();
    assert_eq!(events().matches("community.points_earned").count(), 2);
    assert_eq!(events().matches("community.badge_earned").count(), 1);

    // Looking again records nothing new; a lost badge is recorded once.
    frank.community.points().await.unwrap();
    assert_eq!(events().matches("community.points_earned").count(), 2);
    world.service.set_badges(&f, &[]);
    frank.community.points().await.unwrap();
    frank.community.points().await.unwrap();
    assert_eq!(events().matches("community.badge_lost").count(), 1);

    // The leaderboard: Frank first this week, and his own place.
    world
        .service
        .give_points(&pat.member_id(), 2, PointsReason::ContactAccepted);
    let board = frank.community.leaderboard(false).await.unwrap();
    assert!(!board.all_time);
    assert_eq!(board.top[0].name, "frank-g");
    assert_eq!(board.my_place, Some(1));
    assert_eq!(board.my_points, 5);
    let all = frank.community.leaderboard(true).await.unwrap();
    assert!(all.all_time);
}

#[tokio::test]
async fn a_member_under_18_sees_their_points_and_is_never_on_the_leaderboard() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    // Born March 2011: 15 in September 2026.
    let teen = world.person("Robin Young", "robin-y", 2011, false).await;
    world
        .service
        .give_points(&teen.member_id(), 2, PointsReason::ContactAccepted);
    let points = teen.community.points().await.unwrap();
    assert_eq!(points.total, 2);
    assert_eq!(points.place_week, None);
    let board = frank.community.leaderboard(false).await.unwrap();
    assert!(board.top.iter().all(|row| row.name != "robin-y"));
    assert_eq!(
        teen.community.leaderboard(false).await.unwrap().my_place,
        None
    );
}

#[tokio::test]
async fn getting_started_ticks_its_three_steps_on_this_pc() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let steps = frank.community.getting_started();
    assert!(!steps.profile && !steps.found_someone && !steps.sent_a_message && !steps.closed);

    let draft = ProfileDraft {
        company: "8 West Ventures, LLC".into(),
        shown: ProfileShown::default(),
        ..ProfileDraft::default()
    };
    let tile = Tile {
        status: TileStatus::Available,
        mood: None,
        message: String::new(),
        picture: None,
    };
    frank.community.save_profile(&draft, &tile).await.unwrap();
    frank.community.find("pat-lee").await.unwrap();
    frank
        .community
        .send_message(&frank.ledger, &pat.member_id(), "pat-lee", "Hi Pat", None)
        .await
        .unwrap();
    let steps = frank.community.getting_started();
    assert!(steps.profile && steps.found_someone && steps.sent_a_message);

    let before = world.service.seen().len();
    frank.community.close_getting_started();
    assert!(frank.community.getting_started().closed);
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
}

#[tokio::test]
async fn messages_earn_no_points_and_appearing_offline_leaves_the_leaderboard() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980, true).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985, true).await;
    let (f, p) = (frank.member_id(), pat.member_id());
    frank
        .community
        .send_message(&frank.ledger, &p, "pat-lee", "Hi Pat", None)
        .await
        .unwrap();
    pat.community.pick_up(&pat.ledger, 0).await.unwrap();
    pat.community.accept(&pat.ledger, &f).await.unwrap();
    frank.community.pick_up(&frank.ledger, 0).await.unwrap();
    let after_accept = frank.community.points().await.unwrap().total;
    for n in 0..5 {
        world.later(5);
        frank
            .community
            .send_message(&frank.ledger, &p, "pat-lee", &format!("Message {n}"), None)
            .await
            .unwrap();
    }
    assert_eq!(
        frank.community.points().await.unwrap().total,
        after_accept,
        "sending messages earns nothing (ADR-169 §1)"
    );

    world
        .service
        .give_points(&f, 10, PointsReason::GettingStarted);
    let on_board = |board: &plenipo_community::rewards::LeaderboardView| {
        board.top.iter().any(|row| row.name == "frank-g")
    };
    assert!(on_board(&pat.community.leaderboard(false).await.unwrap()));
    frank.community.set_appear_offline(true).await.unwrap();
    assert!(!on_board(&pat.community.leaderboard(false).await.unwrap()));
    assert_eq!(
        frank.community.leaderboard(false).await.unwrap().my_place,
        None
    );
}
