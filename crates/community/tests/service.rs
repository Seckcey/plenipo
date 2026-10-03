//! Community on this PC (ADR-162, ADR-170), against the stand-in service: the switch, Coming
//! soon, signing in with a code, the age box, joining, signing out, leaving, and what is kept,
//! recorded, and sent.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use plenipo_community::profile::{ProfileDraft, ProfileShown, Tile, TileStatus};
use plenipo_community::service::{Clock, Community, Recorder, Settings, Stage, Store};
use plenipo_community::stand_in::{AccountId, Openness, StandIn};

const START: i64 = 1_790_000_000;
const PC_NAME: &str = "FRANKIE-DESKTOP";

/// The owner's tile.
fn tile() -> Tile {
    Tile {
        status: TileStatus::DoNotDisturb,
        mood: Some("focused".into()),
        message: "Feeling great!".into(),
        picture: Some(plenipo_community::stand_in::tiny_png(16, 16)),
    }
}

/// What the owner added, every part shown.
fn draft() -> ProfileDraft {
    ProfileDraft {
        display_name: "Frank Gonzalez".into(),
        company: "8 West Ventures, LLC".into(),
        business_kinds: vec!["it_services".into()],
        business_line: "IT help for small businesses".into(),
        region: "US-CA".into(),
        shown: ProfileShown::default(),
    }
}

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

impl Events {
    fn names(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(e, _)| e.clone())
            .collect()
    }
    fn all_text(&self) -> String {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(e, p)| format!("{e} {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One clock for this PC and the stand-in service.
struct Time(AtomicI64);

impl Clock for Time {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct World {
    service: StandIn,
    store: Arc<Memory>,
    events: Arc<Events>,
    time: Arc<Time>,
    frank: AccountId,
}

impl World {
    fn new() -> Self {
        let service = StandIn::new();
        service.set_now(START);
        let frank = service.add_account("Frank Gonzalez", "frank@example.com", true);
        Self {
            service,
            store: Arc::new(Memory::default()),
            events: Arc::new(Events::default()),
            time: Arc::new(Time(AtomicI64::new(START))),
            frank,
        }
    }

    /// Community on this PC, as Plenipo starts it.
    fn community(&self) -> Community<StandIn> {
        Community::load(
            self.service.clone(),
            self.store.clone(),
            self.events.clone(),
            self.time.clone(),
            env!("CARGO_PKG_VERSION"),
            PC_NAME,
        )
    }

    fn later(&self, secs: i64) {
        let now = self.time.0.fetch_add(secs, Ordering::SeqCst) + secs;
        self.service.set_now(now);
    }

    /// Press the switch, see the code, allow it on the account site, and wait for it.
    async fn sign_in(&self, community: &Community<StandIn>) {
        community.turn_on().await.unwrap();
        let view = community.view(true);
        assert_eq!(view.stage, Stage::SigningIn);
        let code = view.code.expect("a code to type");
        assert!(self.service.allow(&code, self.frank));
        self.later(5);
        assert_eq!(community.poll().await, None, "signed in");
    }

    async fn join(&self, community: &Community<StandIn>, name: &str) {
        let terms = community.view(true).terms.expect("the terms version");
        community
            .join(name, 3, 1980, &terms, &draft(), &tile())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_copy_whose_owner_never_turns_community_on_sends_nothing() {
    let world = World::new();
    let community = world.community();
    assert_eq!(community.view(false).stage, Stage::Off);
    community.refresh().await;
    community.cancel_sign_in();
    assert_eq!(community.poll().await, None);
    assert!(community.leave().await.is_ok());
    let _ = community.view(true);
    assert!(
        world.service.seen().is_empty(),
        "nothing at all reached 8 West"
    );
}

#[tokio::test]
async fn coming_soon_until_8_west_opens_it_and_then_the_same_copy_works() {
    let world = World::new();
    world.service.set_open(Openness::Closed);
    let community = world.community();
    community.turn_on().await.unwrap();
    let view = community.view(true);
    assert_eq!(view.stage, Stage::ComingSoon);
    assert!(!view.switched_on, "the switch stays off");
    assert!(view.coming_soon);
    // Only "is it open?" was asked: no pass, no body, nothing about the PC.
    let seen = world.service.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].path.as_str()),
        ("GET", "/v1/community/open")
    );
    assert!(seen[0].body.is_empty());
    assert!(seen[0].header("authorization").is_none());

    // This PC remembers it, and still asks nothing by itself.
    let again = world.community();
    assert_eq!(again.view(true).stage, Stage::ComingSoon);
    again.refresh().await;
    assert_eq!(world.service.seen().len(), 1);

    // 8 West opens Community: Check again starts signing in, with no new release.
    world.service.set_open(Openness::Open {
        links: false,
        collaborators: false,
    });
    again.turn_on().await.unwrap();
    let view = again.view(true);
    assert_eq!(view.stage, Stage::SigningIn);
    assert!(!view.coming_soon);
    assert!(view.code.is_some());
}

#[tokio::test]
async fn a_busy_service_is_never_coming_soon_and_an_old_version_is_told_to_update() {
    let world = World::new();
    world.service.set_busy(true);
    let community = world.community();
    community.turn_on().await.unwrap();
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Unreachable);
    assert!(!view.coming_soon);
    assert_eq!(
        view.problem.as_deref(),
        Some("Community can't be reached right now. Nothing was changed.")
    );

    world.service.set_busy(false);
    world.service.set_lowest_version(Some("999.0.0"));
    community.turn_on().await.unwrap();
    assert_eq!(community.view(true).stage, Stage::UpdateNeeded);
}

#[tokio::test]
async fn signing_in_shows_a_code_waits_for_allow_and_then_asks_for_the_age_name_and_terms() {
    let world = World::new();
    let community = world.community();
    community.turn_on().await.unwrap();
    let code = community.view(true).code.unwrap();
    assert_eq!(code.len(), 7);

    // Not allowed yet: wait and ask again.
    world.later(5);
    assert_eq!(community.poll().await, Some(5));
    assert_eq!(community.view(true).stage, Stage::SigningIn);

    assert!(world.service.allow(&code, world.frank));
    world.later(5);
    assert_eq!(community.poll().await, None);
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Joining);
    assert_eq!(view.account_name.as_deref(), Some("Frank Gonzalez"));
    assert!(view.code.is_none());
    assert!(!view.switched_on, "on only once this PC has joined");
    assert!(
        world.store.read_pc().unwrap().is_some(),
        "the sign-in is in the Vault"
    );
    assert_eq!(world.events.names(), ["community.signed_in"]);

    world.join(&community, "frank-g").await;
    let view = community.view(true);
    assert_eq!(view.stage, Stage::SignedIn);
    assert!(view.switched_on);
    let member = view.member.unwrap();
    assert_eq!(member.name, "frank-g");
    assert!(member.adult && member.can_start);
    assert_eq!(
        world.events.names(),
        [
            "community.signed_in",
            "community.joined",
            "community.profile_changed"
        ]
    );

    // Plenipo starts again: signed in, and it asks only who it is.
    let again = world.community();
    assert_eq!(again.view(true).stage, Stage::SignedIn);
    let before = world.service.seen().len();
    again.refresh().await;
    assert_eq!(again.view(true).member.unwrap().name, "frank-g");
    let seen = world.service.seen();
    assert_eq!(seen.len(), before + 1);
    assert_eq!(seen.last().unwrap().path, "/v1/community/me");
}

#[tokio::test]
async fn under_13_sends_nothing_keeps_nothing_and_signs_this_pc_out() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    let terms = community.view(true).terms.unwrap();
    let before = world.service.seen();

    // Born in March 2015: 11 in September 2026.
    let refused = community
        .join("young-one", 3, 2015, &terms, &draft(), &tile())
        .await
        .unwrap_err();
    assert_eq!(refused.0, "Community is for people 13 and older.");
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Off);
    assert_eq!(
        view.problem.as_deref(),
        Some("Community is for people 13 and older.")
    );
    assert!(
        world.store.read_pc().unwrap().is_none(),
        "the sign-in is forgotten"
    );

    // The only request after it is signing this PC out: nothing of the answer was sent.
    let after: Vec<_> = world
        .service
        .seen()
        .into_iter()
        .skip(before.len())
        .collect();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].path, "/v1/community/sign-out");
    for seen in world.service.seen() {
        assert!(!String::from_utf8_lossy(&seen.body).contains("2015"));
    }
    assert!(
        !world.events.all_text().contains("2015"),
        "the Ledger keeps no age"
    );

    // A month and year that cannot be a birth is asked again, and nothing is sent.
    world.sign_in(&community).await;
    let terms = community.view(true).terms.unwrap();
    let count = world.service.seen().len();
    let refused = community
        .join("frank-g", 13, 1980, &terms, &draft(), &tile())
        .await
        .unwrap_err();
    assert_eq!(refused.0, "Choose your birth month and year.");
    assert_eq!(world.service.seen().len(), count);
    assert_eq!(community.view(true).stage, Stage::Joining);
}

#[tokio::test]
async fn a_name_the_service_refuses_is_said_in_its_words_and_can_be_changed() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    let terms = community.view(true).terms.unwrap();
    let refused = community
        .join("plenipo-help", 3, 1980, &terms, &draft(), &tile())
        .await
        .unwrap_err();
    assert!(!refused.0.is_empty());
    assert_eq!(community.view(true).stage, Stage::Joining);
    community
        .join("frank-g", 3, 1980, &terms, &draft(), &tile())
        .await
        .unwrap();
    assert_eq!(community.view(true).stage, Stage::SignedIn);
}

#[tokio::test]
async fn pressing_dont_allow_or_letting_the_code_run_out_keeps_nothing() {
    let world = World::new();
    let community = world.community();
    community.turn_on().await.unwrap();
    let code = community.view(true).code.unwrap();
    assert!(world.service.deny(&code));
    world.later(5);
    assert_eq!(community.poll().await, None);
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Off);
    assert_eq!(
        view.problem.as_deref(),
        Some("The sign-in was not allowed. Nothing was kept.")
    );
    assert!(world.store.read_pc().unwrap().is_none());

    community.turn_on().await.unwrap();
    world.later(601);
    assert_eq!(community.poll().await, None);
    assert_eq!(community.view(true).stage, Stage::Off);
    assert!(community.view(true).problem.unwrap().contains("ran out"));

    // Cancel: nothing kept, back to off.
    community.turn_on().await.unwrap();
    community.cancel_sign_in();
    assert_eq!(community.view(true).stage, Stage::Off);
    assert_eq!(community.poll().await, None);
}

#[tokio::test]
async fn signing_out_keeps_the_switch_on_and_signing_in_again_works() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    community.sign_out().await.unwrap();
    let view = community.view(true);
    assert_eq!(view.stage, Stage::SignedOut);
    assert!(view.switched_on);
    assert!(view.member.is_none());
    assert!(world.store.read_pc().unwrap().is_none());
    assert!(world
        .events
        .names()
        .contains(&"community.signed_out".to_owned()));

    // Sign in again: a member already, so straight back in.
    world.sign_in(&community).await;
    let view = community.view(true);
    assert_eq!(view.stage, Stage::SignedIn);
    assert_eq!(view.member.unwrap().name, "frank-g");
}

#[tokio::test]
async fn a_pc_removed_on_the_account_site_is_signed_out_here() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    // Another copy of this PC's pass signs out at 8 West, as Remove on the account site does.
    let other = world.community();
    other.sign_out().await.unwrap();
    community.refresh().await;
    let view = community.view(true);
    assert_eq!(view.stage, Stage::SignedOut);
    let events = world.events.all_text();
    assert!(events.contains("community.signed_out"));
}

#[tokio::test]
async fn closed_again_after_signing_in_keeps_everything_and_carries_on_when_open() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    world.service.set_open(Openness::Closed);
    community.refresh().await;
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Closed);
    assert!(view.switched_on);
    assert!(
        world.store.read_pc().unwrap().is_some(),
        "nothing is forgotten"
    );
    world.service.set_open(Openness::Open {
        links: false,
        collaborators: false,
    });
    community.refresh().await;
    assert_eq!(community.view(true).stage, Stage::SignedIn);
}

#[tokio::test]
async fn leaving_community_turns_the_switch_off_and_ends_it_at_8_west() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    community.leave().await.unwrap();
    let view = community.view(true);
    assert_eq!(view.stage, Stage::Off);
    assert!(!view.switched_on);
    assert!(world.store.read_pc().unwrap().is_none());
    assert_eq!(world.events.names().last().unwrap(), "community.left");
    let last = world.service.seen().pop().unwrap();
    assert_eq!(
        (last.method.as_str(), last.path.as_str()),
        ("DELETE", "/v1/community/me")
    );
}

#[tokio::test]
async fn what_plenipo_sends_is_exactly_what_the_contract_names() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    community.refresh().await;
    community.sign_out().await.unwrap();

    let keys = |body: &[u8]| -> BTreeSet<String> {
        if body.is_empty() {
            return BTreeSet::new();
        }
        let value: serde_json::Value = serde_json::from_slice(body).unwrap();
        value.as_object().unwrap().keys().cloned().collect()
    };
    let set =
        |names: &[&str]| -> BTreeSet<String> { names.iter().map(|n| n.to_string()).collect() };
    for seen in world.service.seen() {
        let expected = match (seen.method.as_str(), seen.path.as_str()) {
            ("GET", "/v1/community/open") => set(&[]),
            ("POST", "/v1/community/sign-in/start") => {
                set(&["device_name", "app_version", "signing_key", "sealing_key"])
            }
            ("POST", "/v1/community/sign-in/token") => set(&["device_code", "proof"]),
            ("POST", "/v1/community/join") => set(&["name", "birth_month", "birth_year", "terms"]),
            ("PUT", "/v1/community/me/profile") => set(&[
                "display_name",
                "status",
                "mood",
                "message",
                "company",
                "business_kinds",
                "business_line",
                "region",
            ]),
            ("PUT", "/v1/community/me/picture") => set(&["png"]),
            ("GET", "/v1/community/me") | ("POST", "/v1/community/sign-out") => set(&[]),
            other => panic!("Plenipo sent something the contract doesn't name here: {other:?}"),
        };
        assert_eq!(keys(&seen.body), expected, "{} {}", seen.method, seen.path);
        // Only the contract's headers.
        let mut names: Vec<&str> = seen.headers.iter().map(|(n, _)| n.as_str()).collect();
        names.sort();
        names.retain(|n| {
            !matches!(
                *n,
                "accept" | "user-agent" | "content-type" | "authorization"
            )
        });
        assert!(names.is_empty(), "{names:?}");
        let body = String::from_utf8_lossy(&seen.body);
        assert!(body.contains(PC_NAME) == seen.path.ends_with("/sign-in/start"));
    }

    // The Ledger names the account and the PC, never the pass, a key, or the code.
    let events = world.events.all_text();
    assert!(events.contains("Frank Gonzalez"));
    for seen in world.service.seen() {
        if let Some(auth) = seen.header("authorization") {
            let pass = auth.trim_start_matches("Bearer ");
            assert!(!events.contains(pass), "never the pass");
        }
        if seen.path.ends_with("/sign-in/start") {
            let start: serde_json::Value = serde_json::from_slice(&seen.body).unwrap();
            for key in ["signing_key", "sealing_key"] {
                assert!(
                    !events.contains(start[key].as_str().unwrap()),
                    "never a key"
                );
            }
        }
    }
}

/// The member as 8 West has them now.
async fn member(community: &Community<StandIn>) -> plenipo_community::wire::Member {
    let pc = community.signed_in_pc().expect("signed in");
    plenipo_community::session::me(community.transport(), &pc)
        .await
        .unwrap()
        .member
        .expect("a member")
}

#[tokio::test]
async fn joining_sends_the_profile_as_what_people_see_showed_it() {
    use plenipo_community::wire::{BusinessKind, Mood, ProfileStatus};
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    let m = member(&community).await;
    assert_eq!(m.profile.display_name.as_deref(), Some("Frank Gonzalez"));
    assert_eq!(
        m.profile.status,
        Some(ProfileStatus::Busy),
        "Do not disturb shows as Busy"
    );
    assert_eq!(m.profile.mood, Some(Mood::Focused));
    assert_eq!(m.profile.message.as_deref(), Some("Feeling great!"));
    assert_eq!(m.profile.business_kinds, [BusinessKind::ItServices]);
    assert_eq!(m.profile.region.as_deref(), Some("US-CA"));
    assert!(m.has_picture);
    // The Ledger names the parts shown, never the words or the picture.
    let events = world.events.all_text();
    assert!(events.contains("community.profile_changed"));
    assert!(!events.contains("Feeling great"));
    assert!(!events.contains("IT help"));
}

#[tokio::test]
async fn an_unticked_part_is_hidden_at_once_and_the_picture_removed() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    let mut d = draft();
    d.shown.picture = false;
    d.shown.message = false;
    d.shown.region = false;
    community.save_profile(&d, &tile()).await.unwrap();
    let m = member(&community).await;
    assert!(!m.has_picture);
    assert_eq!(m.profile.message, None);
    assert_eq!(m.profile.region, None);
    assert_eq!(m.profile.company.as_deref(), Some("8 West Ventures, LLC"));
    assert_eq!(
        community.view(true).profile,
        d,
        "the boxes are kept on this computer"
    );

    // The same picture is not sent twice.
    d.shown.picture = true;
    community.save_profile(&d, &tile()).await.unwrap();
    let pictures = |w: &World| {
        w.service
            .seen()
            .iter()
            .filter(|s| s.method == "PUT" && s.path == "/v1/community/me/picture")
            .count()
    };
    let before = pictures(&world);
    community.save_profile(&d, &tile()).await.unwrap();
    assert_eq!(pictures(&world), before);
}

#[tokio::test]
async fn appearing_offline_stops_sending_the_tile_until_online_again() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    world.join(&community, "frank-g").await;
    community.set_appear_offline(true).await.unwrap();
    assert!(community.view(true).member.unwrap().appear_offline);
    assert!(world
        .events
        .names()
        .contains(&"community.appeared_offline".to_owned()));

    let before = world.service.seen().len();
    let mut changed = tile();
    changed.message = "On holiday".into();
    community.tile_changed(&changed).await;
    assert_eq!(
        world.service.seen().len(),
        before,
        "nothing is sent while offline"
    );

    community.set_appear_offline(false).await.unwrap();
    community.tile_changed(&changed).await;
    assert_eq!(
        member(&community).await.profile.message.as_deref(),
        Some("On holiday")
    );
    assert!(world
        .events
        .names()
        .contains(&"community.appeared_online".to_owned()));
}

#[tokio::test]
async fn a_profile_that_cannot_be_on_a_card_stops_the_join_before_anything_is_sent() {
    let world = World::new();
    let community = world.community();
    world.sign_in(&community).await;
    let terms = community.view(true).terms.unwrap();
    let before = world.service.seen().len();
    let mut d = draft();
    d.region = "Somewhere".into();
    let refused = community
        .join("frank-g", 3, 1980, &terms, &d, &tile())
        .await
        .unwrap_err();
    assert_eq!(refused.0, "Choose a state or a country.");
    assert_eq!(world.service.seen().len(), before);
    assert_eq!(community.view(true).stage, Stage::Joining);
}
