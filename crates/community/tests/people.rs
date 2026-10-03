//! Finding people (ADR-163 §4, §6), against the stand-in service: the directory, New this week,
//! Find someone, a card, a picture, and Invite by email, each as two people on their own PCs.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use plenipo_community::client;
use plenipo_community::people::{Found, PeoplePage};
use plenipo_community::service::{Clock, Community, Recorder, Settings, Stage, Store};
use plenipo_community::stand_in::{tiny_png, AccountId, StandIn};
use plenipo_community::wire;

const START: i64 = 1_790_000_000;

/// The Vault and the settings file, in memory.
#[derive(Default)]
struct Memory {
    pc: Mutex<Option<String>>,
    settings: Mutex<Option<Settings>>,
}

impl Store for Memory {
    fn read_pc(&self) -> Result<Option<String>, String> {
        Ok(self.pc.lock().unwrap().clone())
    }
    fn write_pc(&self, kept: &str) -> Result<(), String> {
        *self.pc.lock().unwrap() = Some(kept.to_owned());
        Ok(())
    }
    fn erase_pc(&self) -> Result<(), String> {
        *self.pc.lock().unwrap() = None;
        Ok(())
    }
    fn read_settings(&self) -> Result<Option<Settings>, String> {
        Ok(self.settings.lock().unwrap().clone())
    }
    fn write_settings(&self, settings: &Settings) -> Result<(), String> {
        *self.settings.lock().unwrap() = Some(settings.clone());
        Ok(())
    }
}

/// The Ledger, in memory.
#[derive(Default)]
struct Events(Mutex<Vec<(String, serde_json::Value)>>);

impl Recorder for Events {
    fn record(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

struct Time(AtomicI64);

impl Clock for Time {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// One person's PC.
struct Pc {
    community: Community<StandIn>,
    events: Arc<Events>,
}

struct World {
    service: StandIn,
    time: Arc<Time>,
}

impl World {
    fn new() -> Self {
        let service = StandIn::new();
        service.set_now(START);
        Self {
            service,
            time: Arc::new(Time(AtomicI64::new(START))),
        }
    }

    fn later(&self, secs: i64) {
        let now = self.time.0.fetch_add(secs, Ordering::SeqCst) + secs;
        self.service.set_now(now);
    }

    /// A person with an 8 West account, signed in and joined on their own PC.
    async fn person(&self, account: &str, name: &str, birth_year: u16) -> Pc {
        let id = self
            .service
            .add_account(account, &format!("{name}@example.com"), true);
        let pc = self.pc();
        self.sign_in(&pc.community, id).await;
        let terms = pc.community.view(true).terms.expect("the terms version");
        join(&pc.community, name, birth_year, &terms).await;
        pc
    }

    fn pc(&self) -> Pc {
        let events = Arc::new(Events::default());
        let community = Community::load(
            self.service.clone(),
            Arc::new(Memory::default()),
            events.clone(),
            self.time.clone(),
            env!("CARGO_PKG_VERSION"),
            "A-PC",
        );
        Pc { community, events }
    }

    async fn sign_in(&self, community: &Community<StandIn>, account: AccountId) {
        community.turn_on().await.unwrap();
        let code = community.view(true).code.expect("a code to type");
        assert!(self.service.allow(&code, account));
        self.later(5);
        assert_eq!(community.poll().await, None, "signed in");
    }
}

/// Join, as Settings → Community does.
async fn join(community: &Community<StandIn>, name: &str, birth_year: u16, terms: &str) {
    community.join(name, 3, birth_year, terms).await.unwrap();
}

/// Show a profile, as Save does.
async fn show(pc: &Pc, display_name: &str, company: &str, kind: wire::BusinessKind, region: &str) {
    let profile = wire::Profile {
        display_name: Some(display_name.into()),
        status: Some(wire::ProfileStatus::Available),
        mood: Some(wire::Mood::Focused),
        message: Some("Building things".into()),
        company: Some(company.into()),
        business_kinds: vec![kind],
        business_line: Some("Homes and repairs".into()),
        region: Some(region.into()),
    };
    pc.community
        .as_member(&client::set_profile(&profile), 200)
        .await
        .unwrap();
}

async fn appear_offline(pc: &Pc, offline: bool) {
    let body = wire::Presence {
        appear_offline: offline,
    };
    pc.community
        .as_member(&client::set_presence(&body), 200)
        .await
        .unwrap();
}

fn names(page: &PeoplePage) -> Vec<&str> {
    page.cards.iter().map(|c| c.name.as_str()).collect()
}

#[tokio::test]
async fn the_directory_lists_adults_and_finds_them_by_words_kind_and_place() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985).await;
    show(
        &pat,
        "Pat Lee",
        "Lee Builders",
        wire::BusinessKind::Construction,
        "US-CA",
    )
    .await;
    let sam = world.person("Sam Ortiz", "sam-ortiz", 1990).await;
    show(
        &sam,
        "Sam Ortiz",
        "Ortiz Books",
        wire::BusinessKind::Accounting,
        "US-TX",
    )
    .await;

    let all = frank.community.directory("", "", "", "").await.unwrap();
    assert!(names(&all).contains(&"pat-lee") && names(&all).contains(&"sam-ortiz"));
    let card = all.cards.iter().find(|c| c.name == "pat-lee").unwrap();
    assert_eq!(card.display_name.as_deref(), Some("Pat Lee"));
    assert_eq!(card.company.as_deref(), Some("Lee Builders"));
    assert_eq!(card.business_kinds, ["construction"]);
    assert_eq!(card.status.as_deref(), Some("available"));
    assert_eq!(card.mood.as_deref(), Some("focused"));

    let by_words = frank
        .community
        .directory("builders", "", "", "")
        .await
        .unwrap();
    assert_eq!(names(&by_words), ["pat-lee"]);
    let by_kind = frank
        .community
        .directory("", "accounting", "", "")
        .await
        .unwrap();
    assert_eq!(names(&by_kind), ["sam-ortiz"]);
    let by_place = frank
        .community
        .directory("", "", "US-CA", "")
        .await
        .unwrap();
    assert_eq!(names(&by_place), ["pat-lee"]);
    let in_the_country = frank.community.directory("", "", "US", "").await.unwrap();
    assert!(names(&in_the_country).len() >= 2);

    // Everyone here joined today, so everyone is New this week.
    let new = frank.community.new_this_week("").await.unwrap();
    assert!(names(&new).contains(&"pat-lee"));
}

#[tokio::test]
async fn a_search_that_could_change_the_address_is_refused_before_anything_is_sent() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    let before = world.service.seen().len();
    for (q, kind, region) in [
        ("", "legal&region=US", ""),
        ("", "", "US/../me"),
        (&*"a".repeat(61), "", ""),
        ("two\nlines", "", ""),
    ] {
        assert!(frank
            .community
            .directory(q, kind, region, "")
            .await
            .is_err());
    }
    assert!(frank.community.find("../me").await.is_err());
    assert!(frank.community.find("a").await.is_err());
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
}

#[tokio::test]
async fn under_18_and_appearing_offline_are_never_listed_and_found_only_for_a_request() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    // Born March 2011: 15 in September 2026.
    let teen = world.person("Robin Young", "robin-y", 2011).await;
    show(
        &teen,
        "Robin",
        "School Club",
        wire::BusinessKind::Education,
        "US-CA",
    )
    .await;
    let pat = world.person("Pat Lee", "pat-lee", 1985).await;

    let all = frank.community.directory("", "", "", "").await.unwrap();
    assert!(!names(&all).contains(&"robin-y"), "never in the directory");
    let new = frank.community.new_this_week("").await.unwrap();
    assert!(!names(&new).contains(&"robin-y"), "never in New this week");
    let found = frank.community.find("@Robin-Y").await.unwrap();
    assert!(
        matches!(&found, Found::RequestOnly { name, .. } if name == "robin-y"),
        "{found:?}"
    );

    // Pat appears offline: out of the directory at once, and found only for a request.
    assert!(names(&frank.community.directory("", "", "", "").await.unwrap()).contains(&"pat-lee"));
    appear_offline(&pat, true).await;
    assert!(!names(&frank.community.directory("", "", "", "").await.unwrap()).contains(&"pat-lee"));
    assert!(matches!(
        frank.community.find("pat-lee").await.unwrap(),
        Found::RequestOnly { .. }
    ));
    appear_offline(&pat, false).await;
    let Found::Card { card } = frank.community.find("pat-lee").await.unwrap() else {
        panic!("Pat's card");
    };
    assert_eq!(card.name, "pat-lee");
    // The same card by ID, and nobody by a name nobody has.
    let by_id = frank
        .community
        .card(&card.member_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_id.name, "pat-lee");
    assert_eq!(
        frank.community.find("nobody-here").await.unwrap(),
        Found::NoOne
    );
}

#[tokio::test]
async fn a_picture_comes_only_as_a_small_real_png() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    let pat = world.person("Pat Lee", "pat-lee", 1985).await;
    let png = tiny_png(32, 32);
    let upload = wire::PictureUpload {
        png: plenipo_community::b64::encode(&png),
    };
    pat.community
        .as_member(&client::set_picture(&upload), 200)
        .await
        .unwrap();
    let Found::Card { card } = frank.community.find("pat-lee").await.unwrap() else {
        panic!("Pat's card");
    };
    assert!(card.has_picture);
    assert_eq!(frank.community.picture(&card.member_id).await, Some(png));
    // An ID that is not one is never put in an address.
    let before = world.service.seen().len();
    assert_eq!(frank.community.picture("../../me").await, None);
    assert_eq!(world.service.seen().len(), before);
}

#[tokio::test]
async fn an_invitation_says_the_same_every_time_and_the_ledger_never_keeps_the_address() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    frank
        .community
        .invite_by_email(" casey@example.com ")
        .await
        .unwrap();
    // The same address again, inside 30 days: the same answer, and no second email.
    frank
        .community
        .invite_by_email("Casey@Example.com")
        .await
        .unwrap();
    assert_eq!(world.service.invitations().len(), 1);
    let kept = frank.events.0.lock().unwrap().clone();
    let invited: Vec<_> = kept
        .iter()
        .filter(|(e, _)| e == "community.invited")
        .collect();
    assert_eq!(invited.len(), 2);
    assert!(
        !format!("{kept:?}").contains("casey"),
        "the Ledger never keeps the address"
    );

    let before = world.service.seen().len();
    for bad in ["casey", "a@b.com, c@d.com", "casey@example"] {
        let refused = frank.community.invite_by_email(bad).await.unwrap_err();
        assert_eq!(refused.0, "Type one email address.");
    }
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
}

#[tokio::test]
async fn too_many_look_ups_say_so_in_8_wests_words_and_nothing_works_signed_out() {
    let world = World::new();
    let frank = world.person("Frank Gonzalez", "frank-g", 1980).await;
    let _pat = world.person("Pat Lee", "pat-lee", 1985).await;
    world.service.set_card_limit(1);
    frank.community.directory("", "", "", "").await.unwrap();
    let refused = frank.community.directory("", "", "", "").await.unwrap_err();
    assert!(!refused.0.is_empty());
    assert_ne!(
        refused.0,
        "Community can't be reached right now. Nothing was changed."
    );

    let signed_out = world.pc();
    let before = world.service.seen().len();
    let refused = signed_out
        .community
        .directory("", "", "", "")
        .await
        .unwrap_err();
    assert_eq!(refused.0, "Sign in to Community first.");
    assert_eq!(signed_out.community.view(true).stage, Stage::Off);
    assert_eq!(world.service.seen().len(), before, "nothing was sent");
}
