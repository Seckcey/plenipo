//! The stand-in account service (`plenipo_community::stand_in`), used the way Plenipo's own
//! tests will use it: as the `Transport`, in this process, and, for a few tests, over a real
//! connection to `127.0.0.1`.
//!
//! These tests check that the stand-in does what `contracts/community/v1/README.md` says, so
//! that a test that passes against it means something. Each test names the part of the
//! contract it holds the stand-in to.

use std::fmt::Debug;
use std::time::{Duration, Instant};

use plenipo_community::client::{self, Answer, ErrorCode, Failure, Request};
use plenipo_community::ids::{new_id, IdKind};
use plenipo_community::item::{self, Dropped, Envelope, Opened, Sealed, ThisPc};
use plenipo_community::session::{self, Finish, Opening, SignedInPc};
use plenipo_community::stand_in::{self, AccountId, Bad, Openness, StandIn};
use plenipo_community::wire::{self, InboxItem, InboxKind, ItemKind};
use plenipo_community::{b64, stamp};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

/// A day in September 2026: the clock for every test, so the ages below are the same every year.
const NOW: i64 = 1_790_000_000;
const DAY: i64 = 86_400;
/// The terms version a new stand-in service starts with.
const TERMS: &str = "2026-10-01";
const ADULT: u16 = 1985;
const TEEN: u16 = 2010;

// ---- Helpers ----------------------------------------------------------------------------------

fn world() -> StandIn {
    let service = StandIn::new();
    service.set_now(NOW);
    service
}

/// A person: an account, signed in on one PC, joined as a member.
struct Person {
    account: AccountId,
    pc: SignedInPc,
    id: String,
    name: String,
}

/// Sign this account in on a new PC, the way the app does: start, then ask until it is allowed.
async fn sign_in(service: &StandIn, account: AccountId, pc_name: &str) -> SignedInPc {
    service.auto_allow(Some(account));
    let started = session::start(service, pc_name, "1.20.0", NOW)
        .await
        .expect("the service answers");
    let (finish, _) = session::finish(service, started)
        .await
        .expect("the service answers");
    service.auto_allow(None);
    match finish {
        Finish::SignedIn(pc, _) => *pc,
        other => panic!("not signed in: {other:?}"),
    }
}

async fn join_as(
    service: &StandIn,
    pc: &SignedInPc,
    name: &str,
    birth_month: u8,
    birth_year: u16,
) -> Result<wire::Me, Failure> {
    session::join(
        service,
        pc,
        &wire::Join {
            name: name.to_owned(),
            birth_month,
            birth_year,
            terms: TERMS.to_owned(),
        },
    )
    .await
}

async fn member(service: &StandIn, name: &str, pro: bool, birth_year: u16) -> Person {
    let account = service.add_account(
        &format!("{name} Account"),
        &format!("{name}@example.com"),
        pro,
    );
    let pc = sign_in(service, account, &format!("{name}-PC")).await;
    let me = join_as(service, &pc, name, 1, birth_year)
        .await
        .expect("joined");
    Person {
        account,
        id: me.member.expect("a member").member_id,
        pc,
        name: name.to_owned(),
    }
}

/// A request from `pc`, read as `T`.
async fn call<T: DeserializeOwned>(
    service: &StandIn,
    pc: &SignedInPc,
    request: Request,
    ok: u16,
) -> Result<T, Failure> {
    let answer = client::send(service, &request, Some(pc.pass())).await?;
    client::read(answer, ok)
}

async fn call_empty(
    service: &StandIn,
    pc: &SignedInPc,
    request: Request,
    ok: u16,
) -> Result<(), Failure> {
    let answer = client::send(service, &request, Some(pc.pass())).await?;
    client::read_empty(answer, ok)
}

/// The error code a request ended with.
fn code<T: Debug>(result: Result<T, Failure>) -> ErrorCode {
    result
        .expect_err("the service refused")
        .code()
        .expect("the service answered with one of the contract's errors")
        .clone()
}

fn service_error(failure: Failure) -> client::ServiceError {
    match failure {
        Failure::Service(error) => error,
        other => panic!("not a service error: {other:?}"),
    }
}

/// A request written by hand, with the headers Plenipo sends and the pass if there is one.
fn raw(service: &StandIn, method: &str, path: &str, pass: Option<&str>, body: &str) -> Answer {
    let mut headers = vec![
        ("Accept".to_owned(), "application/json".to_owned()),
        (
            "User-Agent".to_owned(),
            format!("Plenipo/{}", env!("CARGO_PKG_VERSION")),
        ),
    ];
    if let Some(pass) = pass {
        headers.push(("Authorization".to_owned(), format!("Bearer {pass}")));
    }
    service.handle(method, path, &headers, body.as_bytes())
}

fn error_code_of(answer: &Answer) -> String {
    serde_json::from_slice::<wire::Error>(&answer.body)
        .unwrap_or_else(|_| panic!("not an error: {answer:?}"))
        .error
}

async fn devices_of(
    service: &StandIn,
    asking: &SignedInPc,
    member_id: &str,
) -> Vec<wire::DevicePublic> {
    let devices: wire::Devices = call(service, asking, client::devices_of(member_id).unwrap(), 200)
        .await
        .expect("devices");
    devices.devices
}

/// The PCs a message from `from` to `to` is sealed for: all of theirs, and the sender's others.
async fn pcs_for(service: &StandIn, from: &Person, to: &Person) -> Vec<wire::DevicePublic> {
    let mut pcs = devices_of(service, &from.pc, &to.id).await;
    pcs.extend(
        devices_of(service, &from.pc, &from.id)
            .await
            .into_iter()
            .filter(|d| d.device_id != from.pc.device_id()),
    );
    pcs
}

/// A person's one PC, as the service lists it.
fn public_of(person: &Person) -> wire::DevicePublic {
    wire::DevicePublic {
        device_id: person.pc.device_id().to_owned(),
        signing_key: person.pc.keys().signing_key_text(),
        sealing_key: person.pc.keys().sealing_key_text(),
    }
}

fn seal_text(from: &Person, to: &Person, text: &str, pcs: &[wire::DevicePublic]) -> Sealed {
    seal_as(from, &to.id, ItemKind::Message, text_body(text), pcs)
}

fn text_body(text: &str) -> serde_json::Value {
    serde_json::json!({ "text": text, "gif": null, "sticker": null, "reply_to": null })
}

fn seal_as(
    from: &Person,
    to: &str,
    kind: ItemKind,
    body: serde_json::Value,
    pcs: &[wire::DevicePublic],
) -> Sealed {
    let payload = wire::ItemPayload {
        v: 1,
        item_id: new_id(IdKind::Item),
        kind,
        from: from.id.clone(),
        from_device: from.pc.device_id().to_owned(),
        to: to.to_owned(),
        reference: None,
        sent_at: NOW,
        body,
    };
    item::seal_item(from.pc.keys(), &payload, pcs).expect("sealed")
}

fn item_send(sealed: &Sealed) -> wire::ItemSend {
    wire::ItemSend {
        item_id: sealed.item_id.clone(),
        to: sealed.to.clone(),
        kind: sealed.kind,
        reference: sealed.reference.clone(),
        tag: sealed.tag.clone(),
        copies: sealed.copies.clone(),
    }
}

async fn send_sealed(
    service: &StandIn,
    from: &Person,
    sealed: &Sealed,
) -> Result<wire::ItemSent, Failure> {
    call(
        service,
        &from.pc,
        client::send_item(&item_send(sealed)),
        200,
    )
    .await
}

/// Seal a message for everyone it is for, and send it.
async fn say(
    service: &StandIn,
    from: &Person,
    to: &Person,
    text: &str,
) -> Result<(wire::ItemSent, Sealed), Failure> {
    let pcs = pcs_for(service, from, to).await;
    let sealed = seal_text(from, to, text, &pcs);
    let sent = send_sealed(service, from, &sealed).await?;
    Ok((sent, sealed))
}

async fn pick_up_on(service: &StandIn, pc: &SignedInPc, wait: u32) -> wire::Inbox {
    call(service, pc, client::pick_up(wait), 200)
        .await
        .expect("picked up")
}

async fn pick_up(service: &StandIn, who: &Person) -> wire::Inbox {
    pick_up_on(service, &who.pc, 0).await
}

async fn ack(service: &StandIn, pc: &SignedInPc, inbox: &wire::Inbox) {
    let item_ids = inbox.items.iter().map(|i| i.item_id.clone()).collect();
    call_empty(service, pc, client::ack(&wire::Ack { item_ids }), 204)
        .await
        .expect("done");
}

fn notices(inbox: &wire::Inbox) -> Vec<&wire::Notice> {
    inbox
        .items
        .iter()
        .filter_map(|i| i.notice.as_ref())
        .collect()
}

fn sealed_items(inbox: &wire::Inbox) -> Vec<&InboxItem> {
    inbox
        .items
        .iter()
        .filter(|i| matches!(i.kind, InboxKind::Item(_)))
        .collect()
}

/// A PC opens an item the way the contract says (contract §7).
fn open(
    pc: &SignedInPc,
    member_id: &str,
    item: &InboxItem,
    sender_pcs: &[wire::DevicePublic],
) -> Result<Opened, Dropped> {
    let InboxKind::Item(kind) = item.kind else {
        panic!("a notice has nothing to open");
    };
    let envelope = Envelope {
        item_id: &item.item_id,
        kind,
        from: item.from.as_deref().expect("an item has a sender"),
        reference: item.reference.as_deref(),
        sealed: item.sealed.as_deref().expect("an item is sealed"),
        tag: item.tag.as_deref().expect("an item has a tag"),
        stamp: item.stamp.as_deref().expect("an item is stamped"),
    };
    item::open_item(
        &ThisPc {
            keys: pc.keys(),
            device_id: pc.device_id(),
            member_id,
        },
        &envelope,
        sender_pcs,
    )
}

fn text_of(opened: &Opened) -> String {
    opened.payload.body["text"]
        .as_str()
        .expect("a message with text")
        .to_owned()
}

async fn me_of(service: &StandIn, who: &Person) -> wire::Me {
    session::me(service, &who.pc).await.expect("me")
}

async fn member_of(service: &StandIn, who: &Person) -> wire::Member {
    me_of(service, who).await.member.expect("a member")
}

async fn card_of(service: &StandIn, viewer: &Person, id: &str) -> Result<wire::Card, Failure> {
    call(service, &viewer.pc, client::person(id).unwrap(), 200).await
}

/// Two people who talk: `first` asked, `second` accepted.
async fn talking(service: &StandIn, first: &Person, second: &Person) {
    say(service, first, second, "Hello!").await.expect("asked");
    call_empty(
        service,
        &second.pc,
        client::accept_contact(&first.id).unwrap(),
        204,
    )
    .await
    .expect("accepted");
}

// ---- 1. Is Community open? (contract §1) ------------------------------------------------------

#[tokio::test]
async fn is_community_open_asks_nothing_and_follows_the_service() {
    let service = StandIn::new();
    service.set_open(Openness::Closed);
    assert_eq!(session::check_open(&service).await, Opening::NotOpen);
    let seen = service.seen();
    assert_eq!(
        seen.len(),
        1,
        "one request, and the service kept nothing else"
    );
    assert_eq!(seen[0].method, "GET");
    assert_eq!(seen[0].path, "/v1/community/open");
    assert!(seen[0].body.is_empty(), "no body");
    assert_eq!(seen[0].header("authorization"), None, "no pass");

    service.set_open(Openness::Open {
        links: false,
        collaborators: true,
    });
    assert_eq!(
        session::check_open(&service).await,
        Opening::Open(wire::Open {
            links: false,
            collaborators: true
        })
    );

    service.set_busy(true);
    assert_eq!(session::check_open(&service).await, Opening::Unreachable);
    let busy = raw(&service, "GET", "/v1/community/open", None, "");
    assert_eq!((busy.status, busy.retry_after), (503, Some(30)));
    assert_eq!(error_code_of(&busy), "unavailable");
    service.set_busy(false);

    service.set_lowest_version(Some("999.0.0"));
    assert_eq!(session::check_open(&service).await, Opening::UpdateNeeded);
    service.set_lowest_version(None);
    assert!(matches!(
        session::check_open(&service).await,
        Opening::Open(_)
    ));
}

#[tokio::test]
async fn the_checks_come_in_the_contracts_order() {
    let service = world();
    let hand = |service: &StandIn, agent: Option<&str>, method: &str, path: &str| {
        let headers: Vec<(String, String)> = agent
            .map(|a| vec![("user-agent".to_owned(), a.to_owned())])
            .unwrap_or_default();
        service.handle(method, path, &headers, b"")
    };

    // Busy first, over everything.
    service.set_open(Openness::Closed);
    service.set_lowest_version(Some("999.0.0"));
    service.set_busy(true);
    assert_eq!(
        error_code_of(&hand(&service, None, "GET", "/v1/community/open")),
        "unavailable"
    );
    // Then not open, before the version, and before knowing whether the path is a path.
    service.set_busy(false);
    let closed = hand(&service, None, "GET", "/v1/community/nothing-here");
    assert_eq!(
        (closed.status, error_code_of(&closed).as_str()),
        (503, "not_open")
    );
    // Then the version, for every request, `open` and sign-in included.
    service.set_open(Openness::Open {
        links: false,
        collaborators: false,
    });
    for (method, path) in [
        ("GET", "/v1/community/open"),
        ("POST", "/v1/community/sign-in/start"),
        ("GET", "/v1/community/me"),
        ("GET", "/v1/community/nothing-here"),
    ] {
        let answer = hand(&service, Some("Plenipo/1.0.0"), method, path);
        assert_eq!(
            (answer.status, error_code_of(&answer).as_str()),
            (403, "update_needed"),
            "{path}"
        );
    }
    // Numbers are compared part by part, and a suffix is below the same numbers.
    service.set_lowest_version(Some("1.20.0"));
    for (agent, fine) in [
        (Some("Plenipo/1.20.0"), true),
        (Some("Plenipo/1.20.1"), true),
        (Some("Plenipo/2.0.0-rc.1"), true),
        (Some("Plenipo/1.100.0"), true),
        (Some("Plenipo/1.20.0-rc.1"), false),
        (Some("Plenipo/1.9.9"), false),
        (Some("Plenipo/"), false),
        (Some("curl/8.0"), false),
        (None, false),
    ] {
        let answer = hand(&service, agent, "GET", "/v1/community/open");
        assert_eq!(answer.status == 200, fine, "{agent:?}");
    }
    // Then the routes.
    service.set_lowest_version(None);
    let unknown = hand(&service, None, "GET", "/v1/community/nothing-here");
    assert_eq!(
        (unknown.status, error_code_of(&unknown).as_str()),
        (404, "not_found")
    );
    let outside = hand(&service, None, "GET", "/elsewhere");
    assert_eq!(outside.status, 404);
    let wrong_method = hand(&service, None, "POST", "/v1/community/open");
    assert_eq!(
        wrong_method.status, 404,
        "a known path with another method is not found"
    );
    let no_pass = hand(&service, None, "GET", "/v1/community/me");
    assert_eq!(
        (no_pass.status, error_code_of(&no_pass).as_str()),
        (401, "unauthorized")
    );
    let wrong_pass = raw(
        &service,
        "GET",
        "/v1/community/me",
        Some(&b64::encode(&[7u8; 32])),
        "",
    );
    assert_eq!(wrong_pass.status, 401);
}

// ---- 2. Signing in and out (contract §2) ------------------------------------------------------

#[tokio::test]
async fn signing_in_waits_for_allow_and_the_pass_travels_only_in_authorization() {
    let service = world();
    let account = service.add_account("Frank Gonzalez", "frank@example.com", true);

    let mut started = session::start(&service, "FRANKIE-DESKTOP", "1.20.0", NOW)
        .await
        .unwrap();
    let user_code = started.user_code.clone();
    // The first ask: nobody pressed Allow yet. Then the app waits `interval` seconds, each time.
    let mut now = NOW;
    let (finish, next) = session::finish(&service, started).await.unwrap();
    assert!(matches!(finish, Finish::Waiting), "{finish:?}");
    started = next.expect("still signing in");
    now += 5;
    service.set_now(now);
    assert!(
        service.allow(&user_code, account),
        "the person pressed Allow"
    );
    let (finish, next) = session::finish(&service, started).await.unwrap();
    assert!(next.is_none());
    let Finish::SignedIn(pc, me) = finish else {
        panic!("not signed in: {finish:?}");
    };

    // Who this is, with the pass.
    assert_eq!(me.account.name, "Frank Gonzalez");
    assert_eq!(me.device_id, pc.device_id());
    assert_eq!(me.devices.len(), 1);
    assert_eq!(me.devices[0].name, "FRANKIE-DESKTOP");
    assert_eq!(me.terms, TERMS);
    assert!(me.member.is_none(), "not a member until it joins");
    assert_eq!(session::me(&service, &pc).await.unwrap(), *me);
    assert_eq!(
        service.sign_in_emails(),
        vec![("frank@example.com".to_owned(), "FRANKIE-DESKTOP".to_owned())],
        "8 West emails the account when a PC starts using Community"
    );

    // The pass is in `Authorization` and nowhere else: not in an address, not in a body.
    let pass = pc.pass();
    for request in service.seen() {
        assert!(!request.path.contains(pass), "{}", request.path);
        assert!(
            !String::from_utf8_lossy(&request.body).contains(pass),
            "{}",
            request.path
        );
        let has = request
            .headers
            .iter()
            .filter(|(_, value)| value.contains(pass))
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert!(has.is_empty() || has == ["authorization"], "{has:?}");
    }
    let last = service.seen().pop().unwrap();
    assert_eq!(
        last.header("authorization"),
        Some(format!("Bearer {pass}").as_str())
    );

    // Signing out: the pass stops working, and signing out again is fine.
    session::sign_out(&service, &pc).await.unwrap();
    assert_eq!(
        code(session::me(&service, &pc).await),
        ErrorCode::Unauthorized
    );
    session::sign_out(&service, &pc).await.unwrap();
}

#[tokio::test]
async fn plenipo_sends_exactly_the_headers_the_contract_names() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    say(&service, &alice, &bob, "Hi").await.unwrap();
    pick_up(&service, &bob).await;
    let _ = session::check_open(&service).await;

    let version = format!("Plenipo/{}", env!("CARGO_PKG_VERSION"));
    let requests = service.seen();
    assert!(requests.len() > 10);
    for request in &requests {
        let mut names: Vec<&str> = request.headers.iter().map(|(n, _)| n.as_str()).collect();
        names.sort_unstable();
        let mut want = vec!["accept", "user-agent"];
        if !request.body.is_empty() {
            want.push("content-type");
        }
        let public = request.path == "/v1/community/open"
            || request.path.starts_with("/v1/community/sign-in/");
        if !public {
            want.push("authorization");
        }
        want.sort_unstable();
        assert_eq!(names, want, "{} {}", request.method, request.path);
        assert_eq!(request.header("accept"), Some("application/json"));
        assert_eq!(request.header("user-agent"), Some(version.as_str()));
        if !request.body.is_empty() {
            assert_eq!(request.header("content-type"), Some("application/json"));
        }
    }
}

#[tokio::test]
async fn a_sign_in_that_is_asked_about_too_soon_is_told_to_slow_down() {
    let service = world();
    let started = session::start(&service, "PC", "1.20.0", NOW).await.unwrap();
    let (finish, started) = session::finish(&service, started).await.unwrap();
    assert!(matches!(finish, Finish::Waiting));
    // Asked again at once.
    let (finish, started) = session::finish(&service, started.unwrap()).await.unwrap();
    assert!(matches!(finish, Finish::SlowDown), "{finish:?}");
    // Five more seconds each time: 5 was not enough, 10 is.
    service.set_now(NOW + 5);
    let (finish, started) = session::finish(&service, started.unwrap()).await.unwrap();
    assert!(matches!(finish, Finish::SlowDown), "{finish:?}");
    service.set_now(NOW + 5 + 15);
    let (finish, started) = session::finish(&service, started.unwrap()).await.unwrap();
    assert!(matches!(finish, Finish::Waiting), "{finish:?}");
    // The code works once, and runs out after 10 minutes.
    service.set_now(NOW + 5 + 15 + 600);
    let (finish, _) = session::finish(&service, started.unwrap()).await.unwrap();
    assert!(matches!(finish, Finish::Expired), "{finish:?}");
}

#[tokio::test]
async fn a_person_can_press_dont_allow_and_a_code_works_once() {
    let service = world();
    let account = service.add_account("Pat", "pat@example.com", false);
    let started = session::start(&service, "PC", "1.20.0", NOW).await.unwrap();
    assert!(!service.allow("ZZZ-ZZZ", account), "no such code");
    assert!(service.deny(&started.user_code));
    assert!(!service.deny(&started.user_code), "only while it waits");
    let (finish, _) = session::finish(&service, started).await.unwrap();
    assert!(matches!(finish, Finish::Denied), "{finish:?}");

    // Written by hand: the answer to a good sign-in, asked about again.
    let keys = plenipo_community::keys::PcKeys::generate();
    let start = wire::SignInStart {
        device_name: "PC".into(),
        app_version: "1.20.0".into(),
        signing_key: keys.signing_key_text(),
        sealing_key: keys.sealing_key_text(),
    };
    let started: wire::SignInStarted = client::read(
        client::send(&service, &client::sign_in_start(&start), None)
            .await
            .unwrap(),
        200,
    )
    .unwrap();
    assert_eq!(started.device_code.len(), 43);
    assert_eq!(started.expires_in, 600);
    assert_eq!(started.interval, 5);
    assert_eq!(started.verification_uri, session::CONNECT_PAGE);
    let letters = "BCDFGHJKLMNPQRSTVWXZ23456789";
    let code_ok = started.user_code.len() == 7
        && started.user_code.as_bytes()[3] == b'-'
        && started
            .user_code
            .chars()
            .enumerate()
            .all(|(i, c)| i == 3 || letters.contains(c));
    assert!(code_ok, "{}", started.user_code);
    let ask = |proof: String| {
        client::sign_in_token(&wire::SignInToken {
            device_code: started.device_code.clone(),
            proof,
        })
    };
    let proof = b64::encode(&keys.sign(session::SIGN_IN_CONTEXT, &started.device_code));
    // A proof by other keys is not this PC's.
    let strangers = plenipo_community::keys::PcKeys::generate();
    let wrong = b64::encode(&strangers.sign(session::SIGN_IN_CONTEXT, &started.device_code));
    let answer = client::send(&service, &ask(wrong), None).await.unwrap();
    assert_eq!(error_code_of(&answer), "bad_request");

    service.allow(&started.user_code, account);
    let answer = client::send(&service, &ask(proof.clone()), None)
        .await
        .unwrap();
    let signed_in: wire::SignedIn = client::read(answer, 200).unwrap();
    assert_eq!(signed_in.pass.len(), 43);
    assert!(plenipo_community::ids::is_id(
        IdKind::Device,
        &signed_in.device_id
    ));
    service.set_now(NOW + 5);
    let again = client::send(&service, &ask(proof), None).await.unwrap();
    assert_eq!(error_code_of(&again), "expired", "the code works once");
}

#[tokio::test]
async fn an_account_may_have_five_pcs_and_the_others_are_told_of_a_new_one() {
    let service = world();
    let pat = member(&service, "pat-lee", true, ADULT).await;
    let mut more = Vec::new();
    for n in 2..=5 {
        more.push(sign_in(&service, pat.account, &format!("PC-{n}")).await);
    }
    assert_eq!(me_of(&service, &pat).await.devices.len(), 5);
    // The sixth is not allowed: the person is asked to remove one first.
    let started = session::start(&service, "PC-6", "1.20.0", NOW)
        .await
        .unwrap();
    assert!(!service.allow(&started.user_code, pat.account));
    service.auto_allow(Some(pat.account));
    let sixth = session::start(&service, "PC-6", "1.20.0", NOW)
        .await
        .unwrap();
    let (finish, _) = session::finish(&service, sixth).await.unwrap();
    assert!(matches!(finish, Finish::Waiting), "{finish:?}");
    service.auto_allow(None);

    // Every PC but the new one was told about each new PC.
    let first = pick_up(&service, &pat).await;
    let told = notices(&first)
        .iter()
        .filter(|n| n.kind == wire::NoticeType::MyDevicesChanged)
        .count();
    assert_eq!(told, 4);
    let last = pick_up_on(&service, &more[3], 0).await;
    assert!(
        notices(&last).is_empty(),
        "the newest PC was not told about itself"
    );

    // Signing a PC out tells the others, and its keys are gone.
    let gone = more[0].device_id().to_owned();
    session::sign_out(&service, &more[0]).await.unwrap();
    let after = pick_up(&service, &pat).await;
    assert_eq!(notices(&after).len(), 5);
    let devices = devices_of(&service, &pat.pc, &pat.id).await;
    assert_eq!(devices.len(), 4);
    assert!(devices.iter().all(|d| d.device_id != gone));
}

// ---- 3. Joining (contract §3) -----------------------------------------------------------------

#[tokio::test]
async fn joining_follows_the_age_name_and_terms_rules() {
    let service = world();
    let account = service.add_account("Pat Lee", "pat@example.com", true);
    let pc = sign_in(&service, account, "PC").await;

    // Under 13: nothing is kept. A birthday counts only once its month has passed: born in
    // September 2013 is 12 in September 2026, and 13 in October.
    for (month, year) in [(1, 2020), (9, 2013)] {
        let refused = join_as(&service, &pc, "pat-lee", month, year).await;
        assert_eq!(code(refused), ErrorCode::TooYoung, "{month}/{year}");
    }
    assert!(session::me(&service, &pc).await.unwrap().member.is_none());

    // Names.
    for bad in [
        "plenipo-fan",
        "my-8west",
        "Support",
        "ad-min",
        "ab",
        "-pat",
        "pat-",
        "pat lee",
        &"a".repeat(31),
    ] {
        let refused = join_as(&service, &pc, bad, 1, ADULT).await;
        assert_eq!(code(refused), ErrorCode::NameNotAllowed, "{bad}");
    }
    let wrong_terms = session::join(
        &service,
        &pc,
        &wire::Join {
            name: "pat-lee".into(),
            birth_month: 1,
            birth_year: ADULT,
            terms: "2020-01-01".into(),
        },
    )
    .await;
    assert_eq!(code(wrong_terms), ErrorCode::TermsChanged);

    // A good join, as an adult with Pro.
    let me = join_as(&service, &pc, "pat-lee", 8, 2013).await.unwrap();
    let member = me.member.unwrap();
    assert_eq!(member.name, "pat-lee");
    assert_eq!(
        member.age_group,
        wire::AgeGroup::Teen,
        "13 in August, 13 is a teen"
    );
    assert!(!member.can_start, "a teen cannot start things");
    assert_eq!(member.standing, wire::Standing::Ok);
    assert_eq!(member.terms_accepted, TERMS);
    assert_eq!(member.name_change_at, None);
    assert!(!member.has_picture && member.hidden_parts.is_empty());
    assert_eq!(
        code(join_as(&service, &pc, "pat-lee-2", 1, ADULT).await),
        ErrorCode::AlreadyMember
    );

    // Someone else cannot have the same name.
    let other = signed_in_account(&service, "lee-co", false).await;
    assert_eq!(
        code(join_as(&service, &other, "pat-lee", 1, ADULT).await),
        ErrorCode::NameTaken
    );
}

async fn signed_in_account(service: &StandIn, name: &str, pro: bool) -> SignedInPc {
    let account = service.add_account(name, &format!("{name}@example.com"), pro);
    sign_in(service, account, "PC").await
}

#[tokio::test]
async fn an_adult_can_start_things_only_with_pro_and_in_good_standing() {
    let service = world();
    let with_pro = member(&service, "pro-pat", true, ADULT).await;
    let without = member(&service, "plain-pat", false, ADULT).await;
    let teen = member(&service, "teen-pat", true, TEEN).await;
    assert!(member_of(&service, &with_pro).await.can_start);
    assert!(!member_of(&service, &without).await.can_start);
    let teen_member = member_of(&service, &teen).await;
    assert_eq!(teen_member.age_group, wire::AgeGroup::Teen);
    assert!(!teen_member.can_start);

    // Pro ends: can_start follows at once.
    service.set_pro(with_pro.account, false);
    assert!(!member_of(&service, &with_pro).await.can_start);
    service.set_pro(with_pro.account, true);
    assert!(member_of(&service, &with_pro).await.can_start);

    // Standing: paused is not good standing, and it ends by itself.
    service.pause(&with_pro.id, NOW + 100);
    let paused = member_of(&service, &with_pro).await;
    assert_eq!(paused.standing, wire::Standing::Paused);
    assert_eq!(paused.paused_until, Some(NOW + 100));
    assert!(!paused.can_start);
    service.set_now(NOW + 100);
    let back = member_of(&service, &with_pro).await;
    assert_eq!(back.standing, wire::Standing::Ok);
    assert_eq!(back.paused_until, None);
    assert!(back.can_start);

    // A member who turns 18 becomes an adult by themselves: born January 2008 is 18 in
    // September 2026 (the month after January 2008 + 18 years), and born January 2009 is not
    // yet, but is a year later.
    let just_18 = member(&service, "just-18", true, 2008).await;
    assert_eq!(
        member_of(&service, &just_18).await.age_group,
        wire::AgeGroup::Adult
    );
    let still_teen = member(&service, "still-teen", true, 2009).await;
    assert_eq!(
        member_of(&service, &still_teen).await.age_group,
        wire::AgeGroup::Teen
    );
    service.set_now(NOW + 366 * DAY);
    assert_eq!(
        member_of(&service, &still_teen).await.age_group,
        wire::AgeGroup::Adult
    );
}

// ---- 4. A sealed message, and a report of it (contract §6, §7, §11) ----------------------------

#[tokio::test]
async fn a_sealed_message_arrives_stamped_and_can_be_reported() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;

    let (sent, sealed) = say(&service, &alice, &bob, "Hi Bob, want to talk shop? 👋")
        .await
        .unwrap();
    assert!(sent.request, "a first message is a request");
    assert_eq!(sent.accepted_at, NOW);
    assert_eq!(sent.item_id, sealed.item_id);
    let stamped = stamp::check(&sent.stamp, &stand_in::stamping_public())
        .expect("stamped with the contract's test key");
    assert_eq!(
        (stamped.from.as_str(), stamped.to.as_str(), stamped.at),
        (alice.id.as_str(), bob.id.as_str(), NOW)
    );
    assert_eq!(stamped.signer, stand_in::STAMPING_KEY_ID);
    assert_eq!(stamped.tag, sealed.tag);

    // Bob picks it up, opens it, and the stamp on it is the one Alice got.
    let inbox = pick_up(&service, &bob).await;
    assert!(!inbox.more);
    let items = sealed_items(&inbox);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].from.as_deref(), Some(alice.id.as_str()));
    assert!(items[0].request, "delivered as a request");
    assert_eq!(items[0].stamp.as_deref(), Some(sent.stamp.as_str()));
    let alice_pcs = devices_of(&service, &bob.pc, &alice.id).await;
    let opened = open(&bob.pc, &bob.id, items[0], &alice_pcs).expect("it opens");
    assert_eq!(text_of(&opened), "Hi Bob, want to talk shop? 👋");
    assert!(stamp::check(&opened.stamp, &stand_in::stamping_public()).is_some());

    // The stand-in never held the words: it carried only the envelope.
    for request in service.seen() {
        assert!(!String::from_utf8_lossy(&request.body).contains("talk shop"));
    }

    // Bob reports it, and 8 West keeps it.
    let proof = wire::ReportItem {
        stamp: opened.stamp.clone(),
        payload: opened.kept.payload.clone(),
        fk: opened.kept.fk.clone(),
    };
    let report = wire::Report {
        about: alice.id.clone(),
        reason: wire::ReportReason::Spam,
        note: Some("Keeps asking.".into()),
        what: wire::ReportWhat::Items,
        items: vec![proof.clone()],
    };
    let made: wire::ReportMade = call(&service, &bob.pc, client::report(&report), 200)
        .await
        .unwrap();
    assert!(plenipo_community::ids::is_id(
        IdKind::Report,
        &made.report_id
    ));
    let kept = service.reports();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].report_id, made.report_id);
    assert_eq!(
        (kept[0].about.as_str(), kept[0].by.as_str()),
        (alice.id.as_str(), bob.id.as_str())
    );
    assert_eq!(kept[0].reason, wire::ReportReason::Spam);
    assert_eq!(kept[0].note.as_deref(), Some("Keeps asking."));
    assert_eq!(kept[0].items, vec![proof.clone()], "kept as it came");

    // The same item for the same reason, while the report is open, is the earlier report. Another
    // reason is a new report.
    let again: wire::ReportMade = call(&service, &bob.pc, client::report(&report), 200)
        .await
        .unwrap();
    assert_eq!(again.report_id, made.report_id);
    assert_eq!(service.reports().len(), 1);
    let other_reason = wire::Report {
        reason: wire::ReportReason::Harassment,
        ..report.clone()
    };
    let new: wire::ReportMade = call(&service, &bob.pc, client::report(&other_reason), 200)
        .await
        .unwrap();
    assert_ne!(new.report_id, made.report_id);
    // When 8 West is done, the reporter is told, and a new report of it is a new report.
    service.close_report(&made.report_id, true);
    let told = pick_up(&service, &bob).await;
    let closed: Vec<_> = notices(&told)
        .into_iter()
        .filter(|n| n.kind == wire::NoticeType::ReportClosed)
        .collect();
    assert_eq!(closed.len(), 1);
    assert_eq!(
        closed[0].report_id.as_deref(),
        Some(made.report_id.as_str())
    );
    assert_eq!(closed[0].outcome, Some(wire::ReportOutcome::Action));
    let after: wire::ReportMade = call(&service, &bob.pc, client::report(&report), 200)
        .await
        .unwrap();
    assert_ne!(after.report_id, made.report_id);
}

#[tokio::test]
async fn a_report_whose_proof_does_not_check_out_is_refused_with_the_item() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    let carol = member(&service, "carol", true, ADULT).await;
    say(&service, &alice, &bob, "Hello").await.unwrap();
    let inbox = pick_up(&service, &bob).await;
    let alice_pcs = devices_of(&service, &bob.pc, &alice.id).await;
    let opened = open(&bob.pc, &bob.id, sealed_items(&inbox)[0], &alice_pcs).unwrap();
    let good = wire::ReportItem {
        stamp: opened.stamp.clone(),
        payload: opened.kept.payload.clone(),
        fk: opened.kept.fk.clone(),
    };
    let report = |about: &str, items: Vec<wire::ReportItem>| wire::Report {
        about: about.to_owned(),
        reason: wire::ReportReason::Scam,
        note: None,
        what: wire::ReportWhat::Items,
        items,
    };
    let refused = |failure: Failure| {
        let error = service_error(failure);
        assert_eq!((error.status, error.code), (400, ErrorCode::ProofFailed));
        error.item
    };

    // Words that are not the words that were sent: the HMAC does not match.
    let mut changed = good.clone();
    let json = b64::decode(&good.payload, 100_000).unwrap();
    let mut payload: serde_json::Value = serde_json::from_slice(&json).unwrap();
    payload["body"]["text"] = "Send me money".into();
    changed.payload = b64::encode(&serde_json::to_vec(&payload).unwrap());
    let failure = call::<wire::ReportMade>(
        &service,
        &bob.pc,
        client::report(&report(&alice.id, vec![changed])),
        200,
    )
    .await
    .unwrap_err();
    assert_eq!(refused(failure), Some(0));

    // The index of the one that fails.
    let mut wrong_key = good.clone();
    wrong_key.fk = b64::encode(&[1u8; 32]);
    let failure = call::<wire::ReportMade>(
        &service,
        &bob.pc,
        client::report(&report(&alice.id, vec![good.clone(), wrong_key])),
        200,
    )
    .await
    .unwrap_err();
    assert_eq!(refused(failure), Some(1));

    // A stamp for another sender, another reporter, a changed stamp, or one nobody signed.
    for (about, who) in [(&carol.id, &bob), (&alice.id, &carol)] {
        let failure = call::<wire::ReportMade>(
            &service,
            &who.pc,
            client::report(&report(about, vec![good.clone()])),
            200,
        )
        .await
        .unwrap_err();
        assert_eq!(refused(failure), Some(0), "{} about {}", who.name, about);
    }
    let mut forged = good.clone();
    let (text, signature) = forged.stamp.split_once('.').unwrap();
    let mut flipped = b64::decode(signature, 64).unwrap();
    flipped[0] ^= 1;
    forged.stamp = format!("{text}.{}", b64::encode(&flipped));
    let failure = call::<wire::ReportMade>(
        &service,
        &bob.pc,
        client::report(&report(&alice.id, vec![forged])),
        200,
    )
    .await
    .unwrap_err();
    assert_eq!(refused(failure), Some(0));
    assert!(
        service.reports().is_empty(),
        "a report with a bad proof keeps nothing"
    );

    // An items report may be about someone who left; the stamps prove what they sent.
    call_empty(&service, &alice.pc, client::leave(), 204)
        .await
        .unwrap();
    let made: wire::ReportMade = call(
        &service,
        &bob.pc,
        client::report(&report(&alice.id, vec![good])),
        200,
    )
    .await
    .unwrap();
    assert!(!made.report_id.is_empty());
    // But a person or profile report needs someone Bob can look up.
    for what in [wire::ReportWhat::Person, wire::ReportWhat::Profile] {
        let failure = call::<wire::ReportMade>(
            &service,
            &bob.pc,
            client::report(&wire::Report {
                about: alice.id.clone(),
                reason: wire::ReportReason::Spam,
                note: None,
                what,
                items: Vec::new(),
            }),
            200,
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code(), Some(&ErrorCode::NotFound));
    }
}

// ---- 5. Blocks (contract §8) ------------------------------------------------------------------

#[tokio::test]
async fn a_block_stops_everything_and_the_blocked_person_is_not_told() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    talking(&service, &alice, &bob).await;
    ack(&service, &alice.pc, &pick_up(&service, &alice).await).await;
    // Alice's last message waits, and so does Bob's.
    say(&service, &alice, &bob, "Are you there?").await.unwrap();
    say(&service, &bob, &alice, "Yes").await.unwrap();

    call_empty(&service, &bob.pc, client::block(&alice.id).unwrap(), 204)
        .await
        .unwrap();
    let blocked: wire::Blocks = call(&service, &bob.pc, client::blocks(), 200)
        .await
        .unwrap();
    assert_eq!(blocked.blocks.len(), 1);
    assert_eq!(blocked.blocks[0].member_id, alice.id);
    assert_eq!(blocked.blocks[0].name, "alice");
    assert_eq!(blocked.blocks[0].blocked_at, NOW);

    // What still waited, between the two, is gone, both ways.
    assert!(pick_up(&service, &bob).await.items.is_empty());
    assert!(pick_up(&service, &alice).await.items.is_empty());

    // Alice's next message is not delivered, and everything about Bob is not found. She cannot
    // fetch his PCs now, so she seals for the PC she knew.
    let sealed = seal_text(&alice, &bob, "Why?", &[public_of(&bob)]);
    assert_eq!(
        code(send_sealed(&service, &alice, &sealed).await),
        ErrorCode::NotDelivered
    );
    assert_eq!(
        code(card_of(&service, &alice, &bob.id).await),
        ErrorCode::NotFound
    );
    let by_name: Result<wire::Card, _> =
        call(&service, &alice.pc, client::by_name("bob").unwrap(), 200).await;
    assert_eq!(code(by_name), ErrorCode::NotFound);
    let devices: Result<wire::Devices, _> = call(
        &service,
        &alice.pc,
        client::devices_of(&bob.id).unwrap(),
        200,
    )
    .await;
    assert_eq!(code(devices), ErrorCode::NotFound);
    let picture = client::send(
        &service,
        &client::picture_of(&bob.id).unwrap(),
        Some(alice.pc.pass()),
    )
    .await
    .unwrap();
    assert_eq!(picture.status, 404);
    // Bob cannot write to Alice either while the block stands. Alice is told nothing.
    let sealed = seal_text(&bob, &alice, "Sorry", &[public_of(&alice)]);
    assert_eq!(
        code(send_sealed(&service, &bob, &sealed).await),
        ErrorCode::NotDelivered
    );
    assert!(
        pick_up(&service, &alice).await.items.is_empty(),
        "no word of the block"
    );
    assert!(alice_has_no_contacts(&service, &alice).await);

    // Unblocking brings nothing back: a new conversation starts with a new request.
    call_empty(&service, &bob.pc, client::unblock(&alice.id).unwrap(), 204)
        .await
        .unwrap();
    let (sent, _) = say(&service, &alice, &bob, "Hello again").await.unwrap();
    assert!(sent.request);
}

async fn alice_has_no_contacts(service: &StandIn, alice: &Person) -> bool {
    let contacts: wire::Contacts = call(service, &alice.pc, client::contacts(), 200)
        .await
        .unwrap();
    contacts.contacts.is_empty()
}

// ---- 6. Over a real connection ----------------------------------------------------------------

struct HttpAnswer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpAnswer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A tiny HTTP/1.1 client: one request on a connection that stays open, and its answer.
async fn http(
    stream: &mut TcpStream,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> HttpAnswer {
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if !body.is_empty() {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();

    let mut buffer = Vec::new();
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).await.unwrap();
        assert!(n > 0, "the connection closed in the middle of an answer");
        buffer.extend_from_slice(&chunk[..n]);
    };
    let text = String::from_utf8(buffer[..head_end].to_vec()).unwrap();
    let mut lines = text.split("\r\n");
    let status: u16 = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).await.unwrap();
        assert!(n > 0, "the connection closed in the middle of an answer");
        body.extend_from_slice(&chunk[..n]);
    }
    HttpAnswer {
        status,
        headers,
        body,
    }
}

async fn connect(address: &str) -> TcpStream {
    TcpStream::connect(address.strip_prefix("http://").unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn over_a_real_connection_it_answers_in_json_and_keeps_the_connection() {
    let service = world();
    let address = service.serve().await;
    assert!(address.starts_with("http://127.0.0.1:"), "{address}");
    let mut stream = connect(&address).await;

    let open = http(
        &mut stream,
        "GET",
        "/v1/community/open",
        &[("Accept", "application/json")],
        b"",
    )
    .await;
    assert_eq!(open.status, 200);
    assert_eq!(open.header("content-type"), Some("application/json"));
    let open_body: wire::Open = serde_json::from_slice(&open.body).unwrap();
    assert_eq!(
        open_body,
        wire::Open {
            links: false,
            collaborators: false
        }
    );

    // The same connection again, with a body, and an error.
    let refused = http(
        &mut stream,
        "POST",
        "/v1/community/sign-in/start",
        &[("Content-Type", "application/json")],
        b"{\"nonsense\": true}",
    )
    .await;
    assert_eq!(refused.status, 400);
    let error: wire::Error = serde_json::from_slice(&refused.body).unwrap();
    assert_eq!(error.error, "bad_request");
    let unknown = http(&mut stream, "GET", "/v1/community/nothing-here", &[], b"").await;
    assert_eq!(unknown.status, 404);

    // What came over the wire is what the stand-in saw: the headers as sent, and the host.
    let seen = service.seen();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].header("host"), Some("127.0.0.1"));
    assert_eq!(seen[0].header("accept"), Some("application/json"));
    assert_eq!(seen[1].header("content-length"), Some("18"));
    assert_eq!(seen[1].body, b"{\"nonsense\": true}");

    // Busy: 503 with Retry-After, in JSON.
    service.set_busy(true);
    let busy = http(&mut stream, "GET", "/v1/community/open", &[], b"").await;
    assert_eq!(busy.status, 503);
    assert_eq!(busy.header("retry-after"), Some("30"));
    assert_eq!(busy.header("content-type"), Some("application/json"));
    // An old-style HTTP/1.0 request is answered, and then the connection is closed.
    let mut other = connect(&address).await;
    other
        .write_all(b"GET /v1/community/open HTTP/1.0\r\n\r\n")
        .await
        .unwrap();
    let mut all = Vec::new();
    other.read_to_end(&mut all).await.unwrap();
    assert!(String::from_utf8_lossy(&all).starts_with("HTTP/1.1 503"));
}

#[tokio::test]
async fn a_pick_up_that_waits_holds_on_over_a_real_connection() {
    let service = world();
    let pat = member(&service, "pat-lee", false, ADULT).await;
    let address = service.serve().await;
    let mut stream = connect(&address).await;
    let authorization = format!("Bearer {}", pat.pc.pass());

    let started = Instant::now();
    let answer = http(
        &mut stream,
        "GET",
        "/v1/community/items?wait=2",
        &[("Authorization", authorization.as_str())],
        b"",
    )
    .await;
    let waited = started.elapsed();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, br#"{"items":[],"more":false}"#);
    assert!(
        waited >= Duration::from_millis(1800) && waited < Duration::from_secs(6),
        "waited {waited:?}"
    );

    // Without waiting, it answers at once.
    let started = Instant::now();
    let answer = http(
        &mut stream,
        "GET",
        "/v1/community/items",
        &[("Authorization", authorization.as_str())],
        b"",
    )
    .await;
    assert_eq!(answer.status, 200);
    assert!(started.elapsed() < Duration::from_secs(1));

    // Something that arrives while it waits cuts the wait short.
    let sender = member(&service, "alice", true, ADULT).await;
    let started = Instant::now();
    let with_pass = [("Authorization", authorization.as_str())];
    let (answer, sent) = tokio::join!(
        http(
            &mut stream,
            "GET",
            "/v1/community/items?wait=20",
            &with_pass,
            b""
        ),
        async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            say(&service, &sender, &pat, "Hello").await.unwrap()
        }
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    let inbox: wire::Inbox = serde_json::from_slice(&answer.body).unwrap();
    assert_eq!(inbox.items.len(), 1);
    assert_eq!(inbox.items[0].item_id, sent.0.item_id);

    // A picture is `image/png`, with its version as the ETag.
    let png = stand_in::tiny_png(32, 32);
    let me: wire::Me = call(
        &service,
        &pat.pc,
        client::set_picture(&wire::PictureUpload {
            png: b64::encode(&png),
        }),
        200,
    )
    .await
    .unwrap();
    let version = me.member.unwrap().picture_version.unwrap();
    let path = format!("/v1/community/people/{}/picture", pat.id);
    let picture = http(
        &mut stream,
        "GET",
        &path,
        &[("Authorization", authorization.as_str())],
        b"",
    )
    .await;
    assert_eq!(picture.status, 200);
    assert_eq!(picture.header("content-type"), Some("image/png"));
    assert_eq!(
        picture.header("etag"),
        Some(format!("\"{version}\"").as_str())
    );
    assert_eq!(picture.body, png);
}

#[tokio::test]
async fn a_pick_up_that_waits_also_holds_on_in_this_process() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    // Nothing waiting, and a short wait: it answers when the time is up.
    let started = Instant::now();
    let inbox = pick_up_on(&service, &bob.pc, 1).await;
    assert!(inbox.items.is_empty() && !inbox.more);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "{:?}",
        started.elapsed()
    );

    // A message sent while it waits ends the wait.
    let started = Instant::now();
    let (inbox, _) = tokio::join!(pick_up_on(&service, &bob.pc, 25), async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        say(&service, &alice, &bob, "Hello").await.unwrap();
    });
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(inbox.items.len(), 1);
}

// ---- More helpers -----------------------------------------------------------------------------

fn profile() -> wire::Profile {
    wire::Profile {
        display_name: None,
        status: None,
        mood: None,
        message: None,
        company: None,
        business_kinds: Vec::new(),
        business_line: None,
        region: None,
    }
}

async fn show(
    service: &StandIn,
    who: &Person,
    profile: &wire::Profile,
) -> Result<wire::Me, Failure> {
    call(service, &who.pc, client::set_profile(profile), 200).await
}

async fn set_presence(service: &StandIn, who: &Person, appear_offline: bool) -> wire::Me {
    call(
        service,
        &who.pc,
        client::set_presence(&wire::Presence { appear_offline }),
        200,
    )
    .await
    .expect("presence")
}

async fn dir(
    service: &StandIn,
    viewer: &Person,
    q: &str,
    kind: &str,
    region: &str,
    cursor: &str,
) -> Result<wire::CardPage, Failure> {
    call(
        service,
        &viewer.pc,
        client::directory(false, q, kind, region, cursor),
        200,
    )
    .await
}

fn names(page: &wire::CardPage) -> Vec<String> {
    let mut names: Vec<String> = page.cards.iter().map(|c| c.name.clone()).collect();
    names.sort();
    names
}

/// What a look-up by name answers: a card, or only that a request may be sent.
async fn by_name(
    service: &StandIn,
    viewer: &Person,
    name: &str,
) -> Result<serde_json::Value, Failure> {
    call(service, &viewer.pc, client::by_name(name).unwrap(), 200).await
}

async fn contacts_of(service: &StandIn, who: &Person) -> Vec<wire::Contact> {
    let contacts: wire::Contacts = call(service, &who.pc, client::contacts(), 200)
        .await
        .expect("contacts");
    contacts.contacts
}

async fn leave_conversation(service: &StandIn, who: &Person, with: &Person) {
    call_empty(
        service,
        &who.pc,
        client::leave_contact(&with.id).unwrap(),
        204,
    )
    .await
    .expect("left the conversation");
}

async fn react(
    service: &StandIn,
    from: &Person,
    to: &Person,
    item: &str,
) -> Result<wire::ItemSent, Failure> {
    let pcs = pcs_for(service, from, to).await;
    let sealed = seal_as(
        from,
        &to.id,
        ItemKind::Reaction,
        serde_json::json!({ "item": item, "emoji": "👍" }),
        &pcs,
    );
    send_sealed(service, from, &sealed).await
}

// ---- 7. The PCs a copy is for (contract §6) ---------------------------------------------------

#[tokio::test]
async fn copies_must_name_exactly_the_pcs_signed_in_now() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let stale = pcs_for(&service, &alice, &bob).await;

    // Bob signs in on a second PC: the list Alice has is out of date.
    let bob_laptop = sign_in(&service, bob.account, "bob-laptop").await;
    let sealed = seal_text(&alice, &bob, "Hi", &stale);
    let error = service_error(send_sealed(&service, &alice, &sealed).await.unwrap_err());
    assert_eq!((error.status, error.code), (409, ErrorCode::DevicesChanged));
    assert!(
        sealed_items(&pick_up(&service, &bob).await).is_empty(),
        "nothing was kept"
    );
    assert!(
        alice_has_no_contacts(&service, &alice).await,
        "and no request was made"
    );

    // Fetch them again and seal again: each PC gets a copy of its own.
    let (sent, _) = say(&service, &alice, &bob, "Hi").await.unwrap();
    assert!(sent.request);
    let alice_pcs = devices_of(&service, &bob.pc, &alice.id).await;
    let on_first = pick_up(&service, &bob).await;
    let on_laptop = pick_up_on(&service, &bob_laptop, 0).await;
    let (first, laptop) = (sealed_items(&on_first), sealed_items(&on_laptop));
    assert_eq!((first.len(), laptop.len()), (1, 1));
    assert_ne!(first[0].sealed, laptop[0].sealed);
    assert_eq!(
        text_of(&open(&bob.pc, &bob.id, first[0], &alice_pcs).unwrap()),
        "Hi"
    );
    assert_eq!(
        text_of(&open(&bob_laptop, &bob.id, laptop[0], &alice_pcs).unwrap()),
        "Hi"
    );
    assert_eq!(
        open(&bob_laptop, &bob.id, first[0], &alice_pcs).unwrap_err(),
        Dropped::NotForThisPc,
        "a copy opens only on its own PC"
    );
    assert!(
        notices(&on_first)
            .iter()
            .any(|n| n.kind == wire::NoticeType::MyDevicesChanged),
        "Bob's first PC was told about the new one"
    );

    // Acks are for one PC: the laptop's copy waits until the laptop is done.
    ack(&service, &bob.pc, &on_first).await;
    assert!(sealed_items(&pick_up(&service, &bob).await).is_empty());
    assert_eq!(
        sealed_items(&pick_up_on(&service, &bob_laptop, 0).await).len(),
        1
    );

    // The sender's other PCs get a copy too, and the sending PC does not.
    call_empty(
        &service,
        &bob.pc,
        client::accept_contact(&alice.id).unwrap(),
        204,
    )
    .await
    .unwrap();
    let alice_phone = sign_in(&service, alice.account, "alice-phone").await;
    let theirs_only = devices_of(&service, &alice.pc, &bob.id).await;
    let sealed = seal_text(&alice, &bob, "Second", &theirs_only);
    assert_eq!(
        code(send_sealed(&service, &alice, &sealed).await),
        ErrorCode::DevicesChanged,
        "Alice's phone is missing"
    );
    let (second, sealed) = say(&service, &alice, &bob, "Second").await.unwrap();
    assert!(!second.request);
    assert_eq!(sealed.copies.len(), 3, "Bob's two PCs and Alice's phone");
    let own = pick_up_on(&service, &alice_phone, 0).await;
    assert_eq!(sealed_items(&own).len(), 1);
    let alice_pcs = devices_of(&service, &alice.pc, &alice.id).await;
    let opened = open(&alice_phone, &alice.id, sealed_items(&own)[0], &alice_pcs)
        .expect("the sender's other PC opens it");
    assert_eq!(text_of(&opened), "Second");
    assert!(sealed_items(&pick_up(&service, &alice).await).is_empty());

    // An extra copy, a copy for the sending PC, or a copy short: all `devices_changed`.
    let pcs = pcs_for(&service, &alice, &bob).await;
    let good = item_send(&seal_text(&alice, &bob, "Third", &pcs));
    let mut extra = good.clone();
    extra.copies.push(wire::ItemCopy {
        device_id: new_id(IdKind::Device),
        sealed: good.copies[0].sealed.clone(),
    });
    let mut own_pc = good.clone();
    own_pc.copies.push(wire::ItemCopy {
        device_id: alice.pc.device_id().to_owned(),
        sealed: good.copies[0].sealed.clone(),
    });
    let mut short = good.clone();
    short.copies.pop();
    let mut twice = good.clone();
    twice.copies.pop();
    twice.copies.push(good.copies[0].clone());
    for (what, send) in [
        ("extra", extra),
        ("own", own_pc),
        ("short", short),
        ("twice", twice),
    ] {
        let result: Result<wire::ItemSent, _> =
            call(&service, &alice.pc, client::send_item(&send), 200).await;
        assert_eq!(code(result), ErrorCode::DevicesChanged, "{what}");
    }
    let sent: wire::ItemSent = call(&service, &alice.pc, client::send_item(&good), 200)
        .await
        .unwrap();
    assert_eq!(sent.item_id, good.item_id);

    // A receiver with no PC signed in: not delivered (not "devices changed").
    session::sign_out(&service, &bob.pc).await.unwrap();
    session::sign_out(&service, &bob_laptop).await.unwrap();
    let sealed = seal_text(&alice, &bob, "Anyone?", &[public_of(&bob)]);
    assert_eq!(
        code(send_sealed(&service, &alice, &sealed).await),
        ErrorCode::NotDelivered
    );
}

// ---- 8. Conversations (contract §5) -----------------------------------------------------------

#[tokio::test]
async fn a_first_message_is_a_request_until_the_person_it_is_for_accepts() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let carol = member(&service, "carol", true, ADULT).await;
    let named = wire::Profile {
        display_name: Some("Bob B".into()),
        ..profile()
    };
    show(&service, &bob, &named).await.unwrap();

    let (first, _) = say(&service, &alice, &bob, "Hello").await.unwrap();
    assert!(first.request);
    assert_eq!(
        code(say(&service, &alice, &bob, "Anyone there?").await),
        ErrorCode::WaitingForAccept
    );
    let inbox = pick_up(&service, &bob).await;
    assert_eq!(
        sealed_items(&inbox).len(),
        1,
        "the refused message was not kept"
    );
    assert!(sealed_items(&inbox)[0].request);

    // Who is who while it waits. The name on the tile is shown only while the two talk.
    let mine = contacts_of(&service, &alice).await;
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].member_id, bob.id);
    assert_eq!(mine[0].state, wire::ContactState::RequestedByMe);
    assert_eq!(mine[0].display_name, None);
    assert_eq!(mine[0].since, NOW);
    let theirs = contacts_of(&service, &bob).await;
    assert_eq!(theirs[0].member_id, alice.id);
    assert_eq!(theirs[0].state, wire::ContactState::RequestedByThem);

    // Only the person who got the request can open a conversation.
    let by_sender: Result<(), _> = call_empty(
        &service,
        &alice.pc,
        client::accept_contact(&bob.id).unwrap(),
        204,
    )
    .await;
    assert_eq!(code(by_sender), ErrorCode::NotFound);
    let nobody = call_empty(
        &service,
        &bob.pc,
        client::accept_contact(&carol.id).unwrap(),
        204,
    )
    .await;
    assert_eq!(code(nobody), ErrorCode::NotFound, "no request from Carol");
    call_empty(
        &service,
        &bob.pc,
        client::accept_contact(&alice.id).unwrap(),
        204,
    )
    .await
    .unwrap();
    // The sender is told.
    let told = pick_up(&service, &alice).await;
    let accepted: Vec<_> = notices(&told)
        .into_iter()
        .filter(|n| n.kind == wire::NoticeType::ContactAccepted)
        .collect();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].member_id.as_deref(), Some(bob.id.as_str()));

    // Now they talk.
    let (second, _) = say(&service, &alice, &bob, "Great").await.unwrap();
    assert!(!second.request);
    let mine = contacts_of(&service, &alice).await;
    assert_eq!(mine[0].state, wire::ContactState::Accepted);
    assert_eq!(mine[0].display_name.as_deref(), Some("Bob B"));
    assert_eq!(
        contacts_of(&service, &bob).await[0].state,
        wire::ContactState::Accepted
    );
    call_empty(
        &service,
        &bob.pc,
        client::accept_contact(&alice.id).unwrap(),
        204,
    )
    .await
    .expect("accepting twice is fine");
}

#[tokio::test]
async fn a_reply_also_accepts_the_request() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    say(&service, &alice, &bob, "Hello").await.unwrap();
    let (reply, _) = say(&service, &bob, &alice, "Hi Alice").await.unwrap();
    assert!(!reply.request, "an answer is not a request");
    let told = pick_up(&service, &alice).await;
    assert!(notices(&told)
        .iter()
        .any(|n| n.kind == wire::NoticeType::ContactAccepted));
    say(&service, &alice, &bob, "Good to hear from you")
        .await
        .unwrap();
}

#[tokio::test]
async fn a_request_can_be_taken_back_or_declined() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;

    // Taken back: its first message goes with it, and writing again is a new request.
    say(&service, &alice, &bob, "Hello").await.unwrap();
    leave_conversation(&service, &alice, &bob).await;
    assert!(sealed_items(&pick_up(&service, &bob).await).is_empty());
    assert!(alice_has_no_contacts(&service, &alice).await);
    let (again, _) = say(&service, &alice, &bob, "Hello again").await.unwrap();
    assert!(again.request);

    // Declined: the sender's next items are not delivered.
    leave_conversation(&service, &bob, &alice).await;
    assert_eq!(
        code(say(&service, &alice, &bob, "Please?").await),
        ErrorCode::NotDelivered
    );
    assert_eq!(
        contacts_of(&service, &bob).await[0].state,
        wire::ContactState::LeftByMe
    );
    assert_eq!(
        contacts_of(&service, &alice).await[0].state,
        wire::ContactState::LeftByThem
    );
    // Taking it back now does not undo the decline.
    leave_conversation(&service, &alice, &bob).await;
    assert_eq!(
        code(say(&service, &alice, &bob, "Please?").await),
        ErrorCode::NotDelivered
    );
    // A declined request is deleted after 30 days: then writing is a new request.
    service.set_now(NOW + 31 * DAY);
    let (fresh, _) = say(&service, &alice, &bob, "A month later").await.unwrap();
    assert!(fresh.request);
}

#[tokio::test]
async fn a_request_nobody_answers_is_deleted_after_thirty_days() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    say(&service, &alice, &bob, "Hello").await.unwrap();
    service.set_now(NOW + 29 * DAY);
    assert_eq!(
        code(say(&service, &alice, &bob, "Hello?").await),
        ErrorCode::WaitingForAccept
    );
    service.set_now(NOW + 31 * DAY);
    assert!(
        sealed_items(&pick_up(&service, &bob).await).is_empty(),
        "the item is gone too"
    );
    assert!(contacts_of(&service, &bob).await.is_empty());
    let (again, _) = say(&service, &alice, &bob, "Hello?").await.unwrap();
    assert!(again.request);
}

#[tokio::test]
async fn leaving_a_conversation_is_each_sides_own() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    talking(&service, &alice, &bob).await;

    // Bob leaves: Alice's next items are not delivered, until Bob writes again.
    leave_conversation(&service, &bob, &alice).await;
    assert_eq!(
        contacts_of(&service, &bob).await[0].state,
        wire::ContactState::LeftByMe
    );
    assert_eq!(
        contacts_of(&service, &alice).await[0].state,
        wire::ContactState::LeftByThem
    );
    assert_eq!(
        code(say(&service, &alice, &bob, "Bob?").await),
        ErrorCode::NotDelivered
    );
    let (back, _) = say(&service, &bob, &alice, "I'm back").await.unwrap();
    assert!(!back.request, "writing again is not a new request");
    say(&service, &alice, &bob, "Welcome back").await.unwrap();

    // Both leave: the conversation is over (contract §5). Writing again is a new request, which
    // needs an adult with Pro, and waits for the other person to accept it.
    leave_conversation(&service, &alice, &bob).await;
    leave_conversation(&service, &bob, &alice).await;
    assert!(contacts_of(&service, &alice).await.is_empty());
    assert!(contacts_of(&service, &bob).await.is_empty());
    assert_eq!(
        code(say(&service, &bob, &alice, "Hello?").await),
        ErrorCode::NeedsPro,
        "Bob has no Pro, so he cannot start again"
    );
    let (again, _) = say(&service, &alice, &bob, "Hello again").await.unwrap();
    assert!(again.request, "a new request");
    say(&service, &bob, &alice, "Hello!").await.unwrap();
}

#[tokio::test]
async fn only_an_adult_with_pro_starts_a_conversation_and_a_reaction_needs_two_who_talk() {
    let service = world();
    let teen = member(&service, "teen", true, TEEN).await;
    let plain = member(&service, "plain", false, ADULT).await;
    let pro = member(&service, "pro", true, ADULT).await;

    assert_eq!(
        code(say(&service, &teen, &pro, "Hi").await),
        ErrorCode::AdultsOnly
    );
    assert_eq!(
        code(say(&service, &plain, &pro, "Hi").await),
        ErrorCode::NeedsPro
    );
    // An adult's first message to a teen is a request, and the teen may answer it.
    let (ask, asked) = say(&service, &pro, &teen, "Hi").await.unwrap();
    assert!(ask.request);

    // A reaction needs the two to talk: not yet.
    assert_eq!(
        code(react(&service, &pro, &teen, &asked.item_id).await),
        ErrorCode::WaitingForAccept
    );
    assert_eq!(
        code(react(&service, &teen, &pro, &asked.item_id).await),
        ErrorCode::NotDelivered
    );
    assert_eq!(
        code(react(&service, &plain, &pro, &asked.item_id).await),
        ErrorCode::NotDelivered,
        "no conversation at all"
    );
    let (answer, _) = say(&service, &teen, &pro, "Hello!").await.unwrap();
    assert!(!answer.request);
    react(&service, &pro, &teen, &asked.item_id).await.unwrap();
    react(&service, &teen, &pro, &asked.item_id).await.unwrap();
    leave_conversation(&service, &teen, &pro).await;
    assert_eq!(
        code(react(&service, &pro, &teen, &asked.item_id).await),
        ErrorCode::NotDelivered
    );
}

// ---- 9. Finding people (contract §4) ----------------------------------------------------------

#[tokio::test]
async fn the_directory_lists_adults_in_good_standing_who_do_not_appear_offline() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let carol = member(&service, "carol", false, ADULT).await;
    let teen = member(&service, "teen", true, TEEN).await;
    let busy_bob = wire::Profile {
        display_name: Some("Bob B".into()),
        status: Some(wire::ProfileStatus::Busy),
        mood: Some(wire::Mood::Focused),
        ..profile()
    };
    show(&service, &bob, &busy_bob).await.unwrap();
    let busy_teen = wire::Profile {
        display_name: Some("Tee".into()),
        status: Some(wire::ProfileStatus::Busy),
        mood: Some(wire::Mood::Good),
        ..profile()
    };
    show(&service, &teen, &busy_teen).await.unwrap();

    let page = dir(&service, &alice, "", "", "", "").await.unwrap();
    assert_eq!(
        names(&page),
        ["alice", "bob", "carol"],
        "a teen is never listed"
    );
    assert_eq!(page.next, None);
    let bob_card = page.cards.iter().find(|c| c.name == "bob").unwrap();
    assert_eq!(bob_card.status, Some(wire::CardStatus::Busy));
    assert_eq!(bob_card.mood, Some(wire::Mood::Focused));
    assert_eq!(bob_card.display_name.as_deref(), Some("Bob B"));

    // A teen: by name, only a request may be sent; as a person, not found.
    let answer = by_name(&service, &alice, "teen").await.unwrap();
    assert_eq!(answer["request_only"], true);
    assert_eq!(answer["member_id"], teen.id.as_str());
    assert_eq!(
        answer.as_object().unwrap().len(),
        3,
        "member_id, name, request_only"
    );
    assert_eq!(
        code(card_of(&service, &alice, &teen.id).await),
        ErrorCode::NotFound
    );
    let new_page: wire::CardPage = call(
        &service,
        &alice.pc,
        client::directory(true, "", "", "", ""),
        200,
    )
    .await
    .unwrap();
    assert_eq!(names(&new_page), ["alice", "bob", "carol"]);

    // Appearing offline: out of the directory and New this week at once.
    assert!(
        set_presence(&service, &bob, true)
            .await
            .member
            .unwrap()
            .appear_offline
    );
    let page = dir(&service, &alice, "", "", "", "").await.unwrap();
    assert_eq!(names(&page), ["alice", "carol"]);
    let new_page: wire::CardPage = call(
        &service,
        &alice.pc,
        client::directory(true, "", "", "", ""),
        200,
    )
    .await
    .unwrap();
    assert_eq!(names(&new_page), ["alice", "carol"]);
    let answer = by_name(&service, &alice, "bob").await.unwrap();
    assert_eq!(
        answer["request_only"], true,
        "a look-up of the name answers only request_only"
    );
    assert_eq!(
        code(card_of(&service, &alice, &bob.id).await),
        ErrorCode::NotFound
    );
    // People who know him see Offline.
    talking(&service, &alice, &bob).await;
    let known = card_of(&service, &alice, &bob.id).await.unwrap();
    assert_eq!(known.status, Some(wire::CardStatus::Offline));
    // And he is listed again when he is not offline.
    set_presence(&service, &bob, false).await;
    let page = dir(&service, &alice, "", "", "", "").await.unwrap();
    assert_eq!(names(&page), ["alice", "bob", "carol"]);

    // Not in good standing: not listed.
    service.pause(&carol.id, NOW + 100);
    service.end(&bob.id);
    let page = dir(&service, &alice, "", "", "", "").await.unwrap();
    assert_eq!(names(&page), ["alice"]);

    // A teen's card: after they talk, an adult sees it, but never the status or the mood.
    say(&service, &alice, &teen, "Hi there").await.unwrap();
    say(&service, &teen, &alice, "Hello").await.unwrap();
    let card = card_of(&service, &alice, &teen.id).await.unwrap();
    assert_eq!(card.display_name.as_deref(), Some("Tee"));
    assert_eq!((card.status, card.mood), (None, None));
    let by_name = by_name(&service, &alice, "teen").await.unwrap();
    assert_eq!(by_name["name"], "teen");
    assert!(
        by_name.get("request_only").is_none(),
        "they talk: the whole card"
    );
    // The teen sees the adults' status and mood.
    let seen_by_teen = card_of(&service, &teen, &alice.id).await.unwrap();
    assert_eq!(seen_by_teen.name, "alice");
}

#[tokio::test]
async fn the_directory_searches_filters_pages_and_puts_most_points_first() {
    let service = world();
    let viewer = member(&service, "viewer", true, ADULT).await;
    let mut people = Vec::new();
    for n in 0..24 {
        people.push(member(&service, &format!("person-{n:02}"), false, ADULT).await);
    }
    let set = |company: Option<&str>,
               name: Option<&str>,
               kinds: Vec<wire::BusinessKind>,
               region: Option<&str>| wire::Profile {
        company: company.map(str::to_owned),
        display_name: name.map(str::to_owned),
        business_kinds: kinds,
        region: region.map(str::to_owned),
        ..profile()
    };
    show(
        &service,
        &people[3],
        &set(
            Some("Acme Builders"),
            None,
            vec![wire::BusinessKind::Construction],
            Some("US-CA"),
        ),
    )
    .await
    .unwrap();
    show(
        &service,
        &people[5],
        &set(
            None,
            None,
            vec![
                wire::BusinessKind::Accounting,
                wire::BusinessKind::Construction,
            ],
            Some("US-NY"),
        ),
    )
    .await
    .unwrap();
    show(
        &service,
        &people[7],
        &set(None, Some("Pat Lee"), vec![], Some("CA")),
    )
    .await
    .unwrap();
    service.give_points(&people[9].id, 50, wire::PointsReason::Thanks);
    service.give_points(&people[2].id, 30, wire::PointsReason::Thanks);

    // 25 members: 20 on the first page, most points first, then the rest.
    let first = dir(&service, &viewer, "", "", "", "").await.unwrap();
    assert_eq!(first.cards.len(), 20);
    assert_eq!(first.cards[0].name, "person-09");
    assert_eq!(first.cards[0].points, 50);
    assert_eq!(first.cards[1].name, "person-02");
    let next = first.next.clone().expect("more pages");
    let second = dir(&service, &viewer, "", "", "", &next).await.unwrap();
    assert_eq!(second.cards.len(), 5);
    assert_eq!(second.next, None);
    let mut all = names(&first);
    all.extend(names(&second));
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 25, "every member once");

    // Search: the start of a name, or any part of a display name, a company, or what they do.
    let found = |page: Result<wire::CardPage, Failure>| names(&page.unwrap());
    assert_eq!(
        found(dir(&service, &viewer, "acme", "", "", "").await),
        ["person-03"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "BUILD", "", "", "").await),
        ["person-03"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "lee", "", "", "").await),
        ["person-07"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "person-1", "", "", "").await).len(),
        10
    );
    assert!(
        found(dir(&service, &viewer, "erson-1", "", "", "").await).is_empty(),
        "the start of a name"
    );
    // A kind, a country (which also matches its states), a state.
    assert_eq!(
        found(dir(&service, &viewer, "", "construction", "", "").await),
        ["person-03", "person-05"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "", "", "US", "").await),
        ["person-03", "person-05"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "", "", "US-CA", "").await),
        ["person-03"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "", "", "CA", "").await),
        ["person-07"]
    );
    assert_eq!(
        found(dir(&service, &viewer, "", "accounting", "US", "").await),
        ["person-05"]
    );
    // Not what the contract says.
    let too_long = "a".repeat(61);
    for bad in [
        dir(&service, &viewer, &too_long, "", "", "").await,
        dir(&service, &viewer, "", "plumbing", "", "").await,
        dir(&service, &viewer, "", "", "usa", "").await,
        dir(&service, &viewer, "", "", "", "!!").await,
    ] {
        assert_eq!(code(bad), ErrorCode::BadRequest);
    }

    // New this week: everyone just joined. A week later nobody is, but all are still listed.
    let page = |new: bool| client::directory(new, "", "", "", "");
    let new: wire::CardPage = call(&service, &viewer.pc, page(true), 200).await.unwrap();
    assert_eq!(new.cards.len(), 20);
    service.set_now(NOW + 8 * DAY);
    let new: wire::CardPage = call(&service, &viewer.pc, page(true), 200).await.unwrap();
    assert!(new.cards.is_empty());
    assert_eq!(
        dir(&service, &viewer, "", "", "", "")
            .await
            .unwrap()
            .cards
            .len(),
        20
    );
}

#[tokio::test]
async fn a_member_may_see_only_so_many_cards_in_a_day() {
    let service = world();
    service.set_card_limit(3);
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    for _ in 0..3 {
        card_of(&service, &alice, &bob.id).await.unwrap();
    }
    let error = service_error(card_of(&service, &alice, &bob.id).await.unwrap_err());
    assert_eq!(
        (error.status, error.code.clone()),
        (429, ErrorCode::TooMany)
    );
    let wait = error.retry_after.expect("Retry-After");
    assert!((1..=86_400).contains(&wait), "{wait}");
    // Every way of seeing a card counts, the member's own card does not, and other members are
    // not held up by it.
    assert_eq!(
        code(by_name(&service, &alice, "bob").await),
        ErrorCode::TooMany
    );
    assert_eq!(
        code(dir(&service, &alice, "", "", "", "").await),
        ErrorCode::TooMany
    );
    card_of(&service, &alice, &alice.id).await.unwrap();
    card_of(&service, &bob, &alice.id).await.unwrap();
    // The next day it is allowed again.
    service.set_now(NOW + DAY);
    card_of(&service, &alice, &bob.id).await.unwrap();

    // The cards a page returns count, one each: past the limit, no more.
    service.set_card_limit(4);
    service.set_now(NOW + 2 * DAY);
    assert_eq!(
        dir(&service, &alice, "", "", "", "")
            .await
            .unwrap()
            .cards
            .len(),
        2
    );
    assert_eq!(
        dir(&service, &alice, "", "", "", "")
            .await
            .unwrap()
            .cards
            .len(),
        2
    );
    assert_eq!(
        code(dir(&service, &alice, "", "", "", "").await),
        ErrorCode::TooMany
    );
}

// ---- 10. Terms, standing, leaving (contract §3) -----------------------------------------------

#[tokio::test]
async fn new_terms_must_be_accepted_before_anything_is_sent_or_shown() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    talking(&service, &alice, &bob).await;
    service.set_terms("2026-11-01");

    let me = me_of(&service, &alice).await;
    assert_eq!(me.terms, "2026-11-01");
    assert_eq!(me.member.unwrap().terms_accepted, TERMS);
    let error = service_error(say(&service, &alice, &bob, "Hi").await.unwrap_err());
    assert_eq!((error.status, error.code), (409, ErrorCode::TermsChanged));
    let invite = wire::InviteEmail {
        email: "pat@example.com".into(),
    };
    assert_eq!(
        code(call::<wire::Empty>(&service, &alice.pc, client::invite_email(&invite), 202).await),
        ErrorCode::TermsChanged
    );
    // Showing needs the new terms; hiding does not. Reading is fine.
    let named = wire::Profile {
        display_name: Some("Alice".into()),
        ..profile()
    };
    assert_eq!(
        code(show(&service, &alice, &named).await),
        ErrorCode::TermsChanged
    );
    show(&service, &alice, &profile())
        .await
        .expect("hiding is always allowed");
    card_of(&service, &alice, &bob.id).await.unwrap();
    pick_up(&service, &alice).await;

    // Accepting: the version must be the current one.
    let wrong = client::accept_terms(&wire::TermsAccept {
        terms: TERMS.to_owned(),
    });
    assert_eq!(
        code(call::<wire::Me>(&service, &alice.pc, wrong, 200).await),
        ErrorCode::TermsChanged
    );
    let right = client::accept_terms(&wire::TermsAccept {
        terms: "2026-11-01".to_owned(),
    });
    let me: wire::Me = call(&service, &alice.pc, right, 200).await.unwrap();
    assert_eq!(me.member.unwrap().terms_accepted, "2026-11-01");
    say(&service, &alice, &bob, "Hi").await.unwrap();
    // A new member has to accept the current ones too.
    let account = service.add_account("Dee", "dee@example.com", false);
    let pc = sign_in(&service, account, "PC").await;
    assert_eq!(
        code(join_as(&service, &pc, "dee-dee", 1, ADULT).await),
        ErrorCode::TermsChanged
    );
}

#[tokio::test]
async fn a_paused_or_ended_member_may_hide_and_read_but_not_show_or_send() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    talking(&service, &alice, &bob).await;
    ack(&service, &alice.pc, &pick_up(&service, &alice).await).await;

    service.pause(&alice.id, NOW + 1_000);
    let inbox = pick_up(&service, &alice).await;
    let told = notices(&inbox);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].kind, wire::NoticeType::StandingChanged);
    assert_eq!(told[0].standing, Some(wire::NoticeStanding::Paused));
    assert_eq!(told[0].paused_until, Some(NOW + 1_000));
    // A paused member can do nothing that shows or sends, not even ask for a PC list: so this
    // message is sealed for the PC that was known.
    let known = [public_of(&bob)];
    let sealed = seal_text(&alice, &bob, "Hi", &known);
    let error = service_error(send_sealed(&service, &alice, &sealed).await.unwrap_err());
    assert_eq!(
        (error.status, error.code),
        (403, ErrorCode::CommunityPaused)
    );
    assert_eq!(
        code(dir(&service, &alice, "", "", "", "").await),
        ErrorCode::CommunityPaused
    );
    let devices: Result<wire::Devices, _> = call(
        &service,
        &alice.pc,
        client::devices_of(&bob.id).unwrap(),
        200,
    )
    .await;
    assert_eq!(code(devices), ErrorCode::CommunityPaused);
    let named = wire::Profile {
        display_name: Some("Alice".into()),
        ..profile()
    };
    assert_eq!(
        code(show(&service, &alice, &named).await),
        ErrorCode::CommunityPaused
    );
    show(&service, &alice, &profile())
        .await
        .expect("hiding is allowed while paused");
    let me = me_of(&service, &alice).await;
    assert_eq!(me.member.unwrap().standing, wire::Standing::Paused);
    // It ends by itself.
    service.set_now(NOW + 1_000);
    say(&service, &alice, &bob, "Hi again").await.unwrap();

    // Ended: the same, for good, and it stays through leaving and joining again.
    service.end(&alice.id);
    let sealed = seal_text(&alice, &bob, "Hi", &known);
    let error = service_error(send_sealed(&service, &alice, &sealed).await.unwrap_err());
    assert_eq!((error.status, error.code), (403, ErrorCode::CommunityEnded));
    assert_eq!(
        me_of(&service, &alice).await.member.unwrap().standing,
        wire::Standing::Ended
    );
    call_empty(&service, &alice.pc, client::leave(), 204)
        .await
        .unwrap();
    let pc = sign_in(&service, alice.account, "again").await;
    let me = join_as(&service, &pc, "alice", 1, ADULT).await.unwrap();
    assert_eq!(me.member.unwrap().standing, wire::Standing::Ended);
}

#[tokio::test]
async fn leaving_deletes_everything_holds_the_name_and_joining_again_keeps_the_member() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    let carol = member(&service, "carol", true, ADULT).await;
    let dave = member(&service, "dave", false, ADULT).await;
    let named = wire::Profile {
        display_name: Some("Bob B".into()),
        ..profile()
    };
    show(&service, &bob, &named).await.unwrap();
    call::<wire::Me>(
        &service,
        &bob.pc,
        client::set_picture(&wire::PictureUpload {
            png: b64::encode(&stand_in::tiny_png(16, 16)),
        }),
        200,
    )
    .await
    .unwrap();
    service.give_points(&bob.id, 5, wire::PointsReason::Thanks);
    // Carol blocked Bob, and Bob blocked Dave. Alice's message to Bob waits.
    call_empty(&service, &carol.pc, client::block(&bob.id).unwrap(), 204)
        .await
        .unwrap();
    call_empty(&service, &bob.pc, client::block(&dave.id).unwrap(), 204)
        .await
        .unwrap();
    say(&service, &alice, &bob, "Hello Bob").await.unwrap();

    call_empty(&service, &bob.pc, client::leave(), 204)
        .await
        .unwrap();
    assert_eq!(
        code(session::me(&service, &bob.pc).await),
        ErrorCode::Unauthorized,
        "every PC is signed out"
    );
    assert_eq!(
        code(card_of(&service, &alice, &bob.id).await),
        ErrorCode::NotFound
    );
    let devices: Result<wire::Devices, _> = call(
        &service,
        &alice.pc,
        client::devices_of(&bob.id).unwrap(),
        200,
    )
    .await;
    assert_eq!(code(devices), ErrorCode::NotFound);
    let sealed = seal_text(&alice, &bob, "Bob?", &[public_of(&bob)]);
    assert_eq!(
        code(send_sealed(&service, &alice, &sealed).await),
        ErrorCode::NotDelivered
    );
    let page = dir(&service, &alice, "", "", "", "").await.unwrap();
    assert!(!names(&page).contains(&"bob".to_owned()));

    // The name is held for 90 days, from the one who left.
    assert_eq!(
        code(
            join_as(
                &service,
                &signed_in_account(&service, "eve", false).await,
                "bob",
                1,
                ADULT
            )
            .await
        ),
        ErrorCode::NameTaken
    );
    service.set_now(NOW + 89 * DAY);
    assert_eq!(
        code(
            join_as(
                &service,
                &signed_in_account(&service, "fay", false).await,
                "bob",
                1,
                ADULT
            )
            .await
        ),
        ErrorCode::NameTaken
    );

    // Joining again: the same member, with nothing of before, and Carol's block still stands.
    let pc = sign_in(&service, bob.account, "bob-again").await;
    let me = join_as(&service, &pc, "bob", 1, ADULT).await.unwrap();
    let again = me.member.unwrap();
    assert_eq!(again.member_id, bob.id, "the same member_id");
    assert_eq!(again.profile, profile());
    assert!(!again.has_picture);
    let back = Person {
        account: bob.account,
        pc,
        id: bob.id.clone(),
        name: "bob".into(),
    };
    assert!(
        sealed_items(&pick_up(&service, &back).await).is_empty(),
        "what waited is gone"
    );
    let blocks: wire::Blocks = call(&service, &back.pc, client::blocks(), 200)
        .await
        .unwrap();
    assert!(blocks.blocks.is_empty(), "the blocks Bob made are gone");
    assert_eq!(
        code(card_of(&service, &back, &carol.id).await),
        ErrorCode::NotFound,
        "the block Carol made still applies"
    );
    let points: wire::Points = call(&service, &back.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!(points.total, 0, "points are deleted");

    // After 90 days a name someone left is free for anyone.
    let dana = member(&service, "dana", false, ADULT).await;
    call_empty(&service, &dana.pc, client::leave(), 204)
        .await
        .unwrap();
    service.set_now(NOW + 89 * DAY + 90 * DAY - 1);
    let taker = signed_in_account(&service, "gus", false).await;
    assert_eq!(
        code(join_as(&service, &taker, "dana", 1, ADULT).await),
        ErrorCode::NameTaken
    );
    service.set_now(NOW + 89 * DAY + 90 * DAY);
    join_as(&service, &taker, "dana", 1, ADULT).await.unwrap();
}

#[tokio::test]
async fn a_name_may_change_once_every_thirty_days() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    member(&service, "bob", true, ADULT).await;
    let change = |name: &str| {
        client::change_name(&wire::NameChange {
            name: name.to_owned(),
        })
    };
    let me: wire::Me = call(&service, &alice.pc, change("alice-two"), 200)
        .await
        .unwrap();
    let member = me.member.unwrap();
    assert_eq!(member.name, "alice-two");
    assert_eq!(member.name_change_at, Some(NOW + 30 * DAY));
    let too_soon: Result<wire::Me, _> = call(&service, &alice.pc, change("alice-three"), 200).await;
    assert_eq!(code(too_soon), ErrorCode::NameChangeTooSoon);
    service.set_now(NOW + 30 * DAY - 1);
    let still: Result<wire::Me, _> = call(&service, &alice.pc, change("alice-three"), 200).await;
    assert_eq!(code(still), ErrorCode::NameChangeTooSoon);
    service.set_now(NOW + 30 * DAY);
    let taken: Result<wire::Me, _> = call(&service, &alice.pc, change("bob"), 200).await;
    assert_eq!(code(taken), ErrorCode::NameTaken);
    let reserved: Result<wire::Me, _> =
        call(&service, &alice.pc, change("plenipo-alice"), 200).await;
    assert_eq!(code(reserved), ErrorCode::NameNotAllowed);
    let me: wire::Me = call(&service, &alice.pc, change("alice-three"), 200)
        .await
        .unwrap();
    assert_eq!(me.member.unwrap().name, "alice-three");
}

// ---- 11. Profiles and pictures (contract §3) --------------------------------------------------

#[tokio::test]
async fn a_profile_is_replaced_whole_and_checked() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let full = wire::Profile {
        display_name: Some("Alice Lee".into()),
        status: Some(wire::ProfileStatus::Away),
        mood: Some(wire::Mood::Celebrating),
        message: Some("Back on Monday".into()),
        company: Some("Lee & Co".into()),
        business_kinds: vec![wire::BusinessKind::Legal, wire::BusinessKind::Consulting],
        business_line: Some("Contracts for small shops".into()),
        region: Some("US-OR".into()),
    };
    let me = show(&service, &alice, &full).await.unwrap();
    assert_eq!(me.member.unwrap().profile, full);
    let card = card_of(&service, &bob, &alice.id).await.unwrap();
    assert_eq!(card.display_name.as_deref(), Some("Alice Lee"));
    assert_eq!(card.status, Some(wire::CardStatus::Away));
    assert_eq!(card.business_kinds, full.business_kinds);
    assert_eq!(card.region.as_deref(), Some("US-OR"));

    // It replaces the whole profile.
    let only_name = wire::Profile {
        display_name: Some("Alice".into()),
        ..profile()
    };
    let me = show(&service, &alice, &only_name).await.unwrap();
    assert_eq!(me.member.unwrap().profile, only_name);

    // What the contract does not allow.
    let bad = |change: &dyn Fn(&mut wire::Profile)| {
        let mut p = full.clone();
        change(&mut p);
        p
    };
    let refused = [
        bad(&|p| p.display_name = Some(String::new())),
        bad(&|p| p.display_name = Some("a".repeat(61))),
        bad(&|p| p.display_name = Some("two\nlines".into())),
        bad(&|p| p.message = Some("a".repeat(81))),
        bad(&|p| p.company = Some("tab\there".into())),
        bad(&|p| p.business_line = Some("a".repeat(81))),
        bad(&|p| p.business_kinds = vec![wire::BusinessKind::Legal; 2]),
        bad(&|p| {
            p.business_kinds = vec![
                wire::BusinessKind::Legal,
                wire::BusinessKind::Retail,
                wire::BusinessKind::Media,
                wire::BusinessKind::Travel,
            ];
        }),
        bad(&|p| p.region = Some("usa".into())),
        bad(&|p| p.region = Some("us".into())),
        bad(&|p| p.region = Some("US-california".into())),
    ];
    for profile in &refused {
        assert_eq!(
            code(show(&service, &alice, profile).await),
            ErrorCode::BadRequest,
            "{profile:?}"
        );
    }
    assert_eq!(
        member_of(&service, &alice).await.profile,
        only_name,
        "a refused profile changes nothing"
    );
    // A field the contract does not name, or one left out.
    let pass = alice.pc.pass();
    let with_more = r#"{"display_name":null,"status":null,"mood":null,"message":null,"company":null,"business_kinds":[],"business_line":null,"region":null,"more":1}"#;
    let left_out = r#"{"display_name":null,"status":null,"mood":null,"message":null,"company":null,"business_kinds":[],"business_line":null}"#;
    let bad_word = r#"{"display_name":null,"status":"dancing","mood":null,"message":null,"company":null,"business_kinds":[],"business_line":null,"region":null}"#;
    for body in [with_more, left_out, bad_word] {
        let answer = raw(
            &service,
            "PUT",
            "/v1/community/me/profile",
            Some(pass),
            body,
        );
        assert_eq!(
            (answer.status, error_code_of(&answer).as_str()),
            (400, "bad_request"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn pictures_are_checked_kept_as_the_image_alone_and_can_be_removed() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let upload = |png: &[u8]| {
        client::set_picture(&wire::PictureUpload {
            png: b64::encode(png),
        })
    };
    let get = |viewer: &Person, id: &str| {
        let request = client::picture_of(id).unwrap();
        let pass = viewer.pc.pass().to_owned();
        let service = service.clone();
        async move { client::send(&service, &request, Some(&pass)).await.unwrap() }
    };

    let png = stand_in::tiny_png(64, 64);
    let me: wire::Me = call(&service, &alice.pc, upload(&png), 200).await.unwrap();
    let member = me.member.unwrap();
    assert!(member.has_picture);
    let version = member.picture_version.expect("a version");
    assert!(!version.is_empty() && version.len() <= 64);
    let card = card_of(&service, &bob, &alice.id).await.unwrap();
    assert_eq!(
        (card.has_picture, card.picture_version.as_deref()),
        (true, Some(version.as_str()))
    );
    let answer = get(&bob, &alice.id).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, png);

    // The version changes when the picture does, and not when it does not.
    let same: wire::Me = call(&service, &alice.pc, upload(&png), 200).await.unwrap();
    assert_eq!(
        same.member.unwrap().picture_version.as_deref(),
        Some(version.as_str())
    );
    let other: wire::Me = call(
        &service,
        &alice.pc,
        upload(&stand_in::tiny_png(32, 32)),
        200,
    )
    .await
    .unwrap();
    assert_ne!(
        other.member.unwrap().picture_version.as_deref(),
        Some(version.as_str())
    );

    // Not a picture, too many pixels, too many bytes.
    for (what, body, want) in [
        ("text", b"hello world".to_vec(), ErrorCode::BadRequest),
        (
            "cut short",
            png[..png.len() - 5].to_vec(),
            ErrorCode::BadRequest,
        ),
        ("wide", stand_in::tiny_png(257, 4), ErrorCode::TooLarge),
        ("tall", stand_in::tiny_png(4, 257), ErrorCode::TooLarge),
        (
            "heavy",
            {
                let mut heavy = b"\x89PNG\r\n\x1a\n".to_vec();
                heavy.resize(300 * 1024, 0);
                heavy
            },
            ErrorCode::TooLarge,
        ),
    ] {
        let result: Result<wire::Me, _> = call(&service, &alice.pc, upload(&body), 200).await;
        assert_eq!(code(result), want, "{what}");
    }
    let not_base64 = raw(
        &service,
        "PUT",
        "/v1/community/me/picture",
        Some(alice.pc.pass()),
        r#"{"png":"not base64!"}"#,
    );
    assert_eq!(error_code_of(&not_base64), "bad_request");

    // Removing it.
    let me: wire::Me = call(&service, &alice.pc, client::remove_picture(), 200)
        .await
        .unwrap();
    assert!(!me.member.unwrap().has_picture, "the answer is Me");
    assert!(!member_of(&service, &alice).await.has_picture);
    assert_eq!(get(&bob, &alice.id).await.status, 404);
}

#[tokio::test]
async fn parts_8_west_hid_stay_hidden_until_8_west_shows_them() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let full = wire::Profile {
        display_name: Some("Alice Lee".into()),
        message: Some("Hello".into()),
        company: Some("Lee & Co".into()),
        ..profile()
    };
    show(&service, &alice, &full).await.unwrap();
    call::<wire::Me>(
        &service,
        &alice.pc,
        client::set_picture(&wire::PictureUpload {
            png: b64::encode(&stand_in::tiny_png(16, 16)),
        }),
        200,
    )
    .await
    .unwrap();

    use wire::HiddenPart::{DisplayName, Message, Picture};
    service.hide_parts(&alice.id, &[Picture, DisplayName, Message]);
    let told = pick_up(&service, &alice).await;
    let hidden: Vec<_> = notices(&told)
        .into_iter()
        .filter(|n| n.kind == wire::NoticeType::ProfileHidden)
        .collect();
    assert_eq!(hidden.len(), 1);
    assert_eq!(
        hidden[0].parts.as_deref(),
        Some(&[Picture, DisplayName, Message][..])
    );
    let member = member_of(&service, &alice).await;
    assert_eq!(member.hidden_parts, [Picture, DisplayName, Message]);
    assert!(!member.has_picture);
    assert_eq!(
        (member.profile.display_name, member.profile.message),
        (None, None)
    );
    assert_eq!(member.profile.company.as_deref(), Some("Lee & Co"));
    let card = card_of(&service, &bob, &alice.id).await.unwrap();
    assert_eq!((card.display_name, card.has_picture), (None, false));
    let picture = client::send(
        &service,
        &client::picture_of(&alice.id).unwrap(),
        Some(bob.pc.pass()),
    )
    .await
    .unwrap();
    assert_eq!(picture.status, 404);

    // Sending them again changes nothing, and a new picture is refused.
    let me = show(&service, &alice, &full).await.unwrap();
    assert_eq!(me.member.unwrap().profile.display_name, None);
    let upload = client::set_picture(&wire::PictureUpload {
        png: b64::encode(&stand_in::tiny_png(16, 16)),
    });
    assert_eq!(
        code(call::<wire::Me>(&service, &alice.pc, upload, 200).await),
        ErrorCode::BadRequest
    );

    // 8 West shows the picture again: the same one, and the notice says what is still hidden.
    service.hide_parts(&alice.id, &[DisplayName, Message]);
    assert!(member_of(&service, &alice).await.has_picture);
    let told = pick_up(&service, &alice).await;
    let last = notices(&told)
        .into_iter()
        .rfind(|n| n.kind == wire::NoticeType::ProfileHidden)
        .unwrap();
    assert_eq!(last.parts.as_deref(), Some(&[DisplayName, Message][..]));
}

// ---- 12. Items (contract §6, §15) -------------------------------------------------------------

#[tokio::test]
async fn the_same_item_sent_again_gets_the_same_answer_and_another_item_may_not_reuse_its_id() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let pcs = pcs_for(&service, &alice, &bob).await;
    let sealed = seal_text(&alice, &bob, "Once", &pcs);
    let first = send_sealed(&service, &alice, &sealed).await.unwrap();
    let again = send_sealed(&service, &alice, &sealed).await.unwrap();
    assert_eq!(first, again, "the answer is the same, stamp and all");
    assert_eq!(
        sealed_items(&pick_up(&service, &bob).await).len(),
        1,
        "and it was taken once"
    );

    let mut other = item_send(&sealed);
    other.tag = b64::encode(&[9u8; 32]);
    let reused: Result<wire::ItemSent, _> =
        call(&service, &alice.pc, client::send_item(&other), 200).await;
    let error = service_error(reused.unwrap_err());
    assert_eq!((error.status, error.code), (409, ErrorCode::ItemIdReused));
    // Another member cannot use it either.
    let carol = member(&service, "carol", true, ADULT).await;
    let theirs = devices_of(&service, &carol.pc, &bob.id).await;
    let mut stolen = item_send(&seal_text(&carol, &bob, "Mine", &theirs));
    stolen.item_id = sealed.item_id.clone();
    let result: Result<wire::ItemSent, _> =
        call(&service, &carol.pc, client::send_item(&stolen), 200).await;
    assert_eq!(code(result), ErrorCode::ItemIdReused);
}

#[tokio::test]
async fn a_sealed_copy_has_a_size_limit_and_an_item_must_be_what_the_contract_says() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let copy = |bytes: usize| wire::ItemCopy {
        device_id: bob.pc.device_id().to_owned(),
        sealed: b64::encode(&vec![0u8; bytes]),
    };
    let send = |copies: Vec<wire::ItemCopy>| wire::ItemSend {
        item_id: new_id(IdKind::Item),
        to: bob.id.clone(),
        kind: ItemKind::Message,
        reference: None,
        tag: b64::encode(&[3u8; 32]),
        copies,
    };
    let try_send = |item: wire::ItemSend| {
        let service = service.clone();
        let alice_pass = alice.pc.pass().to_owned();
        async move {
            let answer = client::send(&service, &client::send_item(&item), Some(&alice_pass))
                .await
                .unwrap();
            answer
        }
    };

    // 32 KiB is fine for a message; one byte more is too large.
    let big = try_send(send(vec![copy(32 * 1024 + 1)])).await;
    assert_eq!(
        (big.status, error_code_of(&big).as_str()),
        (400, "too_large")
    );
    let fits = try_send(send(vec![copy(32 * 1024)])).await;
    assert_eq!(fits.status, 200);
    // A request is not made by an item that was refused (Bob was never asked twice).
    assert_eq!(sealed_items(&pick_up(&service, &bob).await).len(), 1);

    let mut wrong = Vec::new();
    let mut no_copies = send(Vec::new());
    no_copies.copies.clear();
    wrong.push(("no copies", no_copies));
    let mut item = send(vec![copy(100)]);
    item.item_id = "ci_nope".into();
    wrong.push(("item id", item));
    let mut item = send(vec![copy(100)]);
    item.to = alice.id.clone();
    wrong.push(("to yourself", item));
    let mut item = send(vec![copy(100)]);
    item.to = bob.pc.device_id().to_owned();
    wrong.push(("to a PC", item));
    let mut item = send(vec![copy(100)]);
    item.tag = b64::encode(&[3u8; 31]);
    wrong.push(("tag", item));
    let mut item = send(vec![copy(100)]);
    item.reference = Some(new_id(IdKind::Link));
    wrong.push(("a message with a link", item));
    let mut item = send(vec![copy(100)]);
    item.copies[0].sealed = "not base64url!".into();
    wrong.push(("sealed", item));
    let mut item = send(vec![copy(100)]);
    item.copies[0].device_id = "cd_nope".into();
    wrong.push(("copy's PC", item));
    for (what, item) in wrong {
        let answer = try_send(item).await;
        assert_eq!(
            (answer.status, error_code_of(&answer).as_str()),
            (400, "bad_request"),
            "{what}"
        );
    }
    // Written by hand: the `ref` left out, an unknown field, an unknown kind.
    let pass = alice.pc.pass();
    for body in [
        r#"{"item_id":"ci_01JC0D1E2F3G4H5J6K7M8N9P0Q","to":"cm_01JA2B3C4D5E6F7G8H9J0K1M2N","kind":"message","tag":"SOo1KgruVUeLExPsuuanNaNSkHHMsR0cYRXue8vv7XU","copies":[]}"#,
        r#"{"item_id":"ci_01JC0D1E2F3G4H5J6K7M8N9P0Q","to":"cm_01JA2B3C4D5E6F7G8H9J0K1M2N","kind":"shout","ref":null,"tag":"SOo1KgruVUeLExPsuuanNaNSkHHMsR0cYRXue8vv7XU","copies":[]}"#,
        r#"{"item_id":"ci_01JC0D1E2F3G4H5J6K7M8N9P0Q","to":"cm_01JA2B3C4D5E6F7G8H9J0K1M2N","kind":"message","ref":null,"tag":"SOo1KgruVUeLExPsuuanNaNSkHHMsR0cYRXue8vv7XU","copies":[],"more":1}"#,
    ] {
        let answer = raw(&service, "POST", "/v1/community/items", Some(pass), body);
        assert_eq!(
            (answer.status, error_code_of(&answer).as_str()),
            (400, "bad_request"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn items_wait_oldest_first_fifty_at_a_time_until_acked_and_are_deleted_after_thirty_days() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    talking(&service, &alice, &bob).await;
    let mut sent = Vec::new();
    for n in 0..51 {
        let (answer, _) = say(&service, &alice, &bob, &format!("Message {n}"))
            .await
            .unwrap();
        sent.push(answer.item_id);
    }
    // "Hello!" from talking, then 51 more: 52 items.
    let page = pick_up(&service, &bob).await;
    assert_eq!(page.items.len(), 50);
    assert!(page.more);
    let ids: Vec<&str> = page.items.iter().map(|i| i.item_id.as_str()).collect();
    assert_eq!(
        &ids[1..],
        &sent.iter().map(String::as_str).collect::<Vec<_>>()[..49],
        "oldest first"
    );
    // Nothing is taken away until it is acked.
    assert_eq!(pick_up(&service, &bob).await.items.len(), 50);
    ack(&service, &bob.pc, &page).await;
    let rest = pick_up(&service, &bob).await;
    assert_eq!(rest.items.len(), 2);
    assert!(!rest.more);
    let ids: Vec<&str> = rest.items.iter().map(|i| i.item_id.as_str()).collect();
    assert_eq!(ids, [sent[49].as_str(), sent[50].as_str()]);

    // An ack of items that are not there is fine. It must name 1 to 100 items.
    let unknown = wire::Ack {
        item_ids: vec![new_id(IdKind::Item)],
    };
    call_empty(&service, &bob.pc, client::ack(&unknown), 204)
        .await
        .unwrap();
    for item_ids in [Vec::new(), (0..101).map(|_| new_id(IdKind::Item)).collect()] {
        let result = call_empty(&service, &bob.pc, client::ack(&wire::Ack { item_ids }), 204).await;
        assert_eq!(code(result), ErrorCode::BadRequest);
    }
    let dup = new_id(IdKind::Item);
    let twice = wire::Ack {
        item_ids: vec![dup.clone(), dup],
    };
    assert_eq!(
        code(call_empty(&service, &bob.pc, client::ack(&twice), 204).await),
        ErrorCode::BadRequest
    );

    // Not picked up for 30 days: deleted.
    service.set_now(NOW + 30 * DAY - 1);
    assert_eq!(pick_up(&service, &bob).await.items.len(), 2);
    service.set_now(NOW + 30 * DAY);
    assert!(pick_up(&service, &bob).await.items.is_empty());
    let wait = pick_up_on(&service, &bob.pc, 0).await;
    assert!(!wait.more);
    let too_long = raw(
        &service,
        "GET",
        "/v1/community/items?wait=26",
        Some(bob.pc.pass()),
        "",
    );
    assert_eq!(error_code_of(&too_long), "bad_request");
    let not_a_number = raw(
        &service,
        "GET",
        "/v1/community/items?wait=soon",
        Some(bob.pc.pass()),
        "",
    );
    assert_eq!(error_code_of(&not_a_number), "bad_request");
}

// ---- 13. A bad service (the PC's own checks) --------------------------------------------------

#[tokio::test]
async fn a_service_that_changes_repeats_or_makes_up_items_is_caught_by_the_pc() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let (sent, _) = say(&service, &alice, &bob, "Hello").await.unwrap();
    let alice_pcs = devices_of(&service, &bob.pc, &alice.id).await;
    let opens = |inbox: &wire::Inbox| {
        sealed_items(inbox)
            .into_iter()
            .map(|item| open(&bob.pc, &bob.id, item, &alice_pcs).map(|o| text_of(&o)))
            .collect::<Vec<_>>()
    };

    // An honest service.
    assert_eq!(
        opens(&pick_up(&service, &bob).await),
        [Ok("Hello".to_owned())]
    );

    // A flipped byte in a sealed copy: it does not open.
    service.set_bad(Bad::ChangeSealed);
    let changed = pick_up(&service, &bob).await;
    assert_eq!(opens(&changed), [Err(Dropped::NotForThisPc)]);

    // A stamp signed again as if the item were for someone else: the stamp does not match the
    // item, and a report with it does not check out.
    service.set_bad(Bad::ChangeStamp);
    let restamped = pick_up(&service, &bob).await;
    assert_eq!(opens(&restamped), [Err(Dropped::Mismatch)]);
    let honest = {
        service.set_bad(Bad::Honest);
        pick_up(&service, &bob).await
    };
    let real = open(&bob.pc, &bob.id, sealed_items(&honest)[0], &alice_pcs).unwrap();
    let bad_stamp = restamped.items[0].stamp.clone().unwrap();
    assert_ne!(bad_stamp, sent.stamp);
    let report = wire::Report {
        about: alice.id.clone(),
        reason: wire::ReportReason::Spam,
        note: None,
        what: wire::ReportWhat::Items,
        items: vec![wire::ReportItem {
            stamp: bad_stamp,
            payload: real.kept.payload.clone(),
            fk: real.kept.fk.clone(),
        }],
    };
    let result: Result<wire::ReportMade, _> =
        call(&service, &bob.pc, client::report(&report), 200).await;
    assert_eq!(service_error(result.unwrap_err()).item, Some(0));

    // The last item handed out, handed out again, even after it was acked.
    ack(&service, &bob.pc, &honest).await;
    service.set_bad(Bad::Replay);
    let again = pick_up(&service, &bob).await;
    assert_eq!(
        again.items.len(),
        1,
        "nothing waits, but the last one comes again"
    );
    assert_eq!(again.items[0].item_id, sent.item_id);
    assert_eq!(
        opens(&again),
        [Ok("Hello".to_owned())],
        "a good item, a second time"
    );

    // An item nobody sent: sealed for this PC, with a true stamp, and the wrong signature.
    service.set_bad(Bad::Honest);
    call_empty(
        &service,
        &bob.pc,
        client::accept_contact(&alice.id).unwrap(),
        204,
    )
    .await
    .unwrap();
    say(&service, &alice, &bob, "Second").await.unwrap();
    service.set_bad(Bad::Invent);
    let fresh = pick_up(&service, &bob).await;
    let results = opens(&fresh);
    assert_eq!(fresh.items.len(), 2, "the real one, and one more");
    assert_eq!(results[0], Ok("Second".to_owned()));
    assert_eq!(results[1], Err(Dropped::BadSignature));
    assert_ne!(fresh.items[0].item_id, fresh.items[1].item_id);
    assert!(stamp::check(
        fresh.items[1].stamp.as_deref().unwrap(),
        &stand_in::stamping_public()
    )
    .is_some());
    // What the service keeps is never changed: honest again, it is as it was.
    service.set_bad(Bad::Honest);
    assert_eq!(
        opens(&pick_up(&service, &bob).await),
        [Ok("Second".to_owned())]
    );
}

// ---- 14. Reports of a person or a profile (contract §11) --------------------------------------

#[tokio::test]
async fn a_person_or_profile_report_needs_someone_the_reporter_could_look_up_or_blocked() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let teen = member(&service, "teen", false, TEEN).await;
    let carol = member(&service, "carol", false, ADULT).await;
    let png = stand_in::tiny_png(16, 16);
    call::<wire::Me>(
        &service,
        &bob.pc,
        client::set_picture(&wire::PictureUpload {
            png: b64::encode(&png),
        }),
        200,
    )
    .await
    .unwrap();
    let named = wire::Profile {
        display_name: Some("Bob B".into()),
        ..profile()
    };
    show(&service, &bob, &named).await.unwrap();
    let report = |about: &str, what: wire::ReportWhat| wire::Report {
        about: about.to_owned(),
        reason: wire::ReportReason::Impersonation,
        note: Some("Not who they say".into()),
        what,
        items: Vec::new(),
    };
    let make = |who: &Person, report: wire::Report| {
        let service = service.clone();
        let pass = who.pc.pass().to_owned();
        async move {
            let answer = client::send(&service, &client::report(&report), Some(&pass))
                .await
                .unwrap();
            client::read::<wire::ReportMade>(answer, 200)
        }
    };

    let person = make(&alice, report(&bob.id, wire::ReportWhat::Person))
        .await
        .unwrap();
    let again = make(&alice, report(&bob.id, wire::ReportWhat::Person))
        .await
        .unwrap();
    assert_eq!(
        again.report_id, person.report_id,
        "the same report while it is open"
    );
    let profile_report = make(&alice, report(&bob.id, wire::ReportWhat::Profile))
        .await
        .unwrap();
    assert_ne!(profile_report.report_id, person.report_id);
    let kept = service.reports();
    assert_eq!(kept.len(), 2);
    let profile_kept = kept
        .iter()
        .find(|r| r.report_id == profile_report.report_id)
        .unwrap();
    assert_eq!(
        profile_kept.card.as_ref().unwrap().display_name.as_deref(),
        Some("Bob B")
    );
    assert_eq!(
        profile_kept.picture.as_deref(),
        Some(&png[..]),
        "a copy of the picture as it is now"
    );
    assert!(profile_kept.items.is_empty());

    // Someone she cannot look up (a teen, someone who left, someone who is not there): not found,
    // which is also what a block would look like.
    let nobody = new_id(IdKind::Member);
    for about in [&teen.id, &nobody] {
        let refused = make(&alice, report(about, wire::ReportWhat::Person)).await;
        assert_eq!(code(refused), ErrorCode::NotFound, "{about}");
    }
    // Someone she blocked, even one she could not look up now, she can report.
    set_presence(&service, &carol, true).await;
    assert_eq!(
        code(make(&alice, report(&carol.id, wire::ReportWhat::Person)).await),
        ErrorCode::NotFound
    );
    call_empty(&service, &alice.pc, client::block(&carol.id).unwrap(), 204)
        .await
        .unwrap();
    make(&alice, report(&carol.id, wire::ReportWhat::Person))
        .await
        .unwrap();
    // Not about herself, and a report's parts are what the contract says.
    assert_eq!(
        code(make(&alice, report(&alice.id, wire::ReportWhat::Person)).await),
        ErrorCode::BadRequest
    );
    let mut with_items = report(&bob.id, wire::ReportWhat::Person);
    with_items.items.push(wire::ReportItem {
        stamp: "a.b".into(),
        payload: "a".into(),
        fk: "a".into(),
    });
    assert_eq!(code(make(&alice, with_items).await), ErrorCode::BadRequest);
    assert_eq!(
        code(make(&alice, report(&bob.id, wire::ReportWhat::Items)).await),
        ErrorCode::BadRequest,
        "items needs 1 to 20"
    );
    let mut long_note = report(&bob.id, wire::ReportWhat::Person);
    long_note.note = Some("a".repeat(1001));
    assert_eq!(code(make(&alice, long_note).await), ErrorCode::BadRequest);
    let mut empty_note = report(&bob.id, wire::ReportWhat::Person);
    empty_note.note = Some(String::new());
    assert_eq!(code(make(&alice, empty_note).await), ErrorCode::BadRequest);
}

// ---- 15. Points and the leaderboard (contract §12) --------------------------------------------

#[tokio::test]
async fn a_first_message_accepted_gives_two_points_once_for_each_person() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    talking(&service, &alice, &bob).await;
    let points: wire::Points = call(&service, &alice.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!((points.total, points.week), (2, 2));
    assert_eq!(points.recent.len(), 1);
    assert_eq!(points.recent[0].points, 2);
    assert_eq!(points.recent[0].reason, wire::PointsReason::ContactAccepted);
    assert_eq!(points.recent[0].at, NOW);
    assert_eq!(
        points.free_months,
        Some(wire::FreeMonths {
            this_year: 0,
            max: 12
        })
    );
    assert_eq!((points.place_week, points.place_all), (Some(1), Some(1)));
    assert!(points.badges.is_empty());
    let bob_points: wire::Points = call(&service, &bob.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!(bob_points.total, 0, "the one who accepts gets nothing");
    assert_eq!(card_of(&service, &bob, &alice.id).await.unwrap().points, 2);

    // A second first message to the same person, accepted again, gives nothing more.
    call_empty(&service, &bob.pc, client::block(&alice.id).unwrap(), 204)
        .await
        .unwrap();
    call_empty(&service, &bob.pc, client::unblock(&alice.id).unwrap(), 204)
        .await
        .unwrap();
    talking(&service, &alice, &bob).await;
    let points: wire::Points = call(&service, &alice.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!(points.total, 2);

    // A week starts Monday, Pacific time; the total stays.
    service.set_now(NOW + 8 * DAY);
    let later: wire::Points = call(&service, &alice.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!((later.total, later.week), (2, 0));
    assert!(later.week_started_at > points.week_started_at);
    assert!(
        later.week_started_at <= NOW + 8 * DAY && NOW + 8 * DAY - later.week_started_at < 7 * DAY
    );
}

#[tokio::test]
async fn the_leaderboard_leaves_out_teens_the_offline_and_anyone_blocked_either_way() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", false, ADULT).await;
    let carol = member(&service, "carol", false, ADULT).await;
    let dave = member(&service, "dave", false, ADULT).await;
    let erin = member(&service, "erin", false, ADULT).await;
    let teen = member(&service, "teen", false, TEEN).await;
    service.give_points(&carol.id, 10, wire::PointsReason::Thanks);
    service.give_points(&alice.id, 7, wire::PointsReason::Thanks);
    service.give_points(&bob.id, 3, wire::PointsReason::Thanks);
    service.give_points(&teen.id, 99, wire::PointsReason::Thanks);
    service.give_points(&erin.id, 50, wire::PointsReason::Thanks);
    let board = |who: &Person, period: &str| {
        let service = service.clone();
        let pass = who.pc.pass().to_owned();
        let path = format!("/v1/community/leaderboard?period={period}");
        move || {
            let answer = raw(&service, "GET", &path, Some(&pass), "");
            serde_json::from_slice::<wire::Leaderboard>(&answer.body)
                .unwrap_or_else(|_| panic!("{answer:?}"))
        }
    };
    let rows = |board: &wire::Leaderboard| -> Vec<(u32, String, i64)> {
        board
            .top
            .iter()
            .map(|r| (r.place, r.name.clone(), r.points))
            .collect()
    };

    // Everyone listed, most points first; the teen is not on it. Erin is paused, then offline.
    service.pause(&erin.id, NOW + 1_000);
    let week = board(&alice, "week")();
    assert_eq!(week.period, wire::LeaderboardPeriod::Week);
    assert!(week.since.is_some());
    assert_eq!(
        rows(&week),
        [
            (1, "carol".into(), 10),
            (2, "alice".into(), 7),
            (3, "bob".into(), 3),
            (4, "dave".into(), 0)
        ]
    );
    assert_eq!(
        week.me,
        Some(wire::LeaderboardMe {
            place: Some(2),
            points: 7
        })
    );
    let all = board(&alice, "all")();
    assert_eq!(all.period, wire::LeaderboardPeriod::All);
    assert_eq!(all.since, None);
    assert_eq!(rows(&all).len(), 4);
    let points: wire::Points = call(&service, &alice.pc, client::points(), 200)
        .await
        .unwrap();
    assert_eq!((points.place_week, points.place_all), (Some(2), Some(2)));

    // Appearing offline: off the board, and the member's own place is null.
    set_presence(&service, &carol, true).await;
    assert_eq!(
        rows(&board(&alice, "week")()).first().map(|r| r.1.clone()),
        Some("alice".into())
    );
    let mine = board(&carol, "week")();
    assert_eq!(
        mine.me,
        Some(wire::LeaderboardMe {
            place: None,
            points: 10
        })
    );
    assert!(mine.top.iter().all(|r| r.name != "carol"));

    // Blocked either way, for the one asking only.
    call_empty(&service, &alice.pc, client::block(&dave.id).unwrap(), 204)
        .await
        .unwrap();
    assert!(board(&alice, "week")().top.iter().all(|r| r.name != "dave"));
    assert!(board(&dave, "week")().top.iter().all(|r| r.name != "alice"));
    assert!(
        board(&bob, "week")().top.iter().any(|r| r.name == "dave"),
        "others still see him"
    );
    // Places count only the members shown.
    assert_eq!(
        rows(&board(&alice, "week")()),
        [(1, "alice".into(), 7), (2, "bob".into(), 3)]
    );

    // The period has to be one of the two.
    for path in [
        "/v1/community/leaderboard",
        "/v1/community/leaderboard?period=month",
    ] {
        let answer = raw(&service, "GET", path, Some(alice.pc.pass()), "");
        assert_eq!(error_code_of(&answer), "bad_request", "{path}");
    }
}

// ---- 16. Inviting by email, GIFs, and the parts that open later (contract §13, §14, §1) -------

#[tokio::test]
async fn an_email_invitation_is_always_202_and_each_address_is_invited_once_in_thirty_days() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    let plain = member(&service, "plain", false, ADULT).await;
    let teen = member(&service, "teen", true, TEEN).await;
    let invite = |who: &Person, email: &str| {
        let answer = raw(
            &service,
            "POST",
            "/v1/community/invite-email",
            Some(who.pc.pass()),
            &serde_json::json!({ "email": email }).to_string(),
        );
        answer
    };

    let first = invite(&alice, "Pat.Lee+work@Gmail.com");
    assert_eq!(first.status, 202);
    assert_eq!(first.body, b"{}");
    let kept = service.invitations();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].email, "Pat.Lee+work@Gmail.com", "as it was typed");
    assert_eq!(kept[0].by, alice.id);
    // Another spelling of the same mailbox, from anyone: 202, and nothing more.
    for (who, email) in [
        (&alice, "patlee@gmail.com"),
        (&bob, "PATLEE+x@googlemail.com"),
    ] {
        let answer = invite(who, email);
        assert_eq!((answer.status, answer.body.as_slice()), (202, &b"{}"[..]));
    }
    assert_eq!(service.invitations().len(), 1);
    assert_eq!(invite(&alice, "other@example.com").status, 202);
    assert_eq!(service.invitations().len(), 2);
    // Outside Gmail, dots count.
    invite(&alice, "pat.lee@example.com");
    invite(&alice, "patlee@example.com");
    assert_eq!(service.invitations().len(), 4);
    // After 30 days the same address may be invited again.
    service.set_now(NOW + 30 * DAY);
    assert!(
        service.invitations().is_empty(),
        "the old ones are forgotten"
    );
    invite(&bob, "patlee@gmail.com");
    assert_eq!(service.invitations().len(), 1);

    // Starting things needs `can_start`.
    assert_eq!(error_code_of(&invite(&plain, "x@example.com")), "needs_pro");
    assert_eq!(
        error_code_of(&invite(&teen, "x@example.com")),
        "adults_only"
    );
    assert_eq!(
        error_code_of(&invite(&alice, "not an address")),
        "bad_request"
    );
}

#[tokio::test]
async fn parts_that_open_later_answer_not_open_and_when_open_not_found_here() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let bob = member(&service, "bob", true, ADULT).await;
    talking(&service, &alice, &bob).await;
    let pass = alice.pc.pass();
    let ask = |method: &str, path: &str, pass: Option<&str>, body: &str| {
        let answer = raw(&service, method, path, pass, body);
        let code = serde_json::from_slice::<wire::Error>(&answer.body)
            .map(|e| e.error)
            .unwrap_or_default();
        (answer.status, code)
    };
    let link = new_id(IdKind::Link);
    let collab = new_id(IdKind::Collab);
    let link_paths = [
        ("GET", "/v1/community/links".to_owned()),
        ("POST", "/v1/community/links".to_owned()),
        ("POST", format!("/v1/community/links/{link}/accept")),
        ("PUT", format!("/v1/community/links/{link}/paused")),
        ("DELETE", format!("/v1/community/links/{link}")),
    ];
    let collab_paths = [
        ("GET", "/v1/community/collaborations".to_owned()),
        ("POST", "/v1/community/collaborations".to_owned()),
        (
            "POST",
            format!("/v1/community/collaborations/{collab}/accept"),
        ),
        ("DELETE", format!("/v1/community/collaborations/{collab}")),
    ];
    let item_of = |kind: ItemKind, reference: Option<String>| {
        let mut send = item_send(&seal_text(&alice, &bob, "x", &[public_of(&bob)]));
        send.kind = kind;
        send.reference = reference;
        serde_json::to_string(&send).unwrap()
    };
    let thanks = |for_what: &str, reference: &str| {
        serde_json::json!({ "to": bob.id, "for": for_what, "ref": reference }).to_string()
    };

    // Closed: `not_open`, with or without a pass, and for the items and thanks of those parts.
    for (method, path) in link_paths.iter().chain(&collab_paths) {
        assert_eq!(
            ask(method, path, None, ""),
            (503, "not_open".into()),
            "{path}"
        );
        assert_eq!(
            ask(method, path, Some(pass), ""),
            (503, "not_open".into()),
            "{path}"
        );
    }
    for (kind, reference) in [
        (ItemKind::LinkNote, link.clone()),
        (ItemKind::Objective, link.clone()),
        (ItemKind::ObjectiveState, link.clone()),
        (ItemKind::Answer, link.clone()),
        (ItemKind::CollabNote, collab.clone()),
    ] {
        let body = item_of(kind, Some(reference));
        assert_eq!(
            ask("POST", "/v1/community/items", Some(pass), &body),
            (503, "not_open".into()),
            "{kind:?}"
        );
    }
    assert_eq!(
        ask(
            "POST",
            "/v1/community/thanks",
            Some(pass),
            &thanks("link_answer", &link)
        ),
        (503, "not_open".into())
    );
    assert_eq!(
        ask(
            "POST",
            "/v1/community/thanks",
            Some(pass),
            &thanks("collaborator", &collab)
        ),
        (503, "not_open".into())
    );
    // Everything else keeps working.
    say(&service, &alice, &bob, "Still works").await.unwrap();

    // Links open, collaborators not: the links answer `not_found` here, the collaborators
    // still `not_open`.
    service.set_open(Openness::Open {
        links: true,
        collaborators: false,
    });
    for (method, path) in &link_paths {
        assert_eq!(
            ask(method, path, None, ""),
            (401, "unauthorized".into()),
            "{path}"
        );
        assert_eq!(
            ask(method, path, Some(pass), ""),
            (404, "not_found".into()),
            "{path}"
        );
    }
    assert_eq!(
        ask("GET", "/v1/community/collaborations", Some(pass), ""),
        (503, "not_open".into())
    );
    let body = item_of(ItemKind::LinkNote, Some(link.clone()));
    assert_eq!(
        ask("POST", "/v1/community/items", Some(pass), &body),
        (404, "not_found".into())
    );
    // Open, a link note still has to name a link (and a message none).
    let no_link = item_of(ItemKind::LinkNote, None);
    assert_eq!(
        ask("POST", "/v1/community/items", Some(pass), &no_link),
        (400, "bad_request".into())
    );
    assert_eq!(
        ask(
            "POST",
            "/v1/community/thanks",
            Some(pass),
            &thanks("link_answer", &link)
        ),
        (404, "not_found".into())
    );
    assert_eq!(
        ask(
            "POST",
            "/v1/community/thanks",
            Some(pass),
            &thanks("collaborator", &collab)
        ),
        (503, "not_open".into())
    );
    // A thanks for a link, with a collaboration's ID, is not what the contract says.
    assert_eq!(
        ask(
            "POST",
            "/v1/community/thanks",
            Some(pass),
            &thanks("link_answer", &collab)
        ),
        (400, "bad_request".into())
    );

    service.set_open(Openness::Open {
        links: true,
        collaborators: true,
    });
    for (method, path) in &collab_paths {
        assert_eq!(
            ask(method, path, Some(pass), ""),
            (404, "not_found".into()),
            "{path}"
        );
    }
    let open = ask("GET", "/v1/community/open", None, "");
    assert_eq!(open.0, 200);

    // A GIF search: 8 West has not chosen a library yet.
    assert_eq!(
        ask("GET", "/v1/community/gifs?q=cat", Some(pass), ""),
        (503, "gifs_unavailable".into())
    );
}

#[tokio::test]
async fn bodies_and_addresses_are_checked_as_the_contract_says() {
    let service = world();
    let alice = member(&service, "alice", true, ADULT).await;
    let pass = alice.pc.pass();
    let ask = |method: &str, path: &str, body: &str| {
        let answer = raw(&service, method, path, Some(pass), body);
        (answer.status, error_code_of(&answer))
    };
    let bad = (400, "bad_request".to_owned());
    for (method, path, body) in [
        (
            "PUT",
            "/v1/community/me/presence",
            r#"{"appear_offline": true, "more": 1}"#,
        ),
        ("PUT", "/v1/community/me/presence", "{}"),
        ("PUT", "/v1/community/me/presence", "not json"),
        ("PUT", "/v1/community/me/presence", "[]"),
        ("PUT", "/v1/community/me/presence", ""),
        ("PUT", "/v1/community/me/name", r#"{"name": 5}"#),
        ("PUT", "/v1/community/me/terms", r#"{"terms": null}"#),
        ("POST", "/v1/community/items/ack", r#"{"item_ids": "all"}"#),
        ("POST", "/v1/community/reports", r#"{"about": "cm_x"}"#),
        ("POST", "/v1/community/thanks", r#"{"to": 1}"#),
        ("POST", "/v1/community/invite-email", r#"{"email": "a"}"#),
        ("POST", "/v1/community/join", r#"{"name": "x"}"#),
    ] {
        assert_eq!(ask(method, path, body), bad, "{path} {body}");
    }
    // Too many bytes for the route.
    let big = format!(r#"{{"name": "{}"}}"#, "a".repeat(17 * 1024));
    assert_eq!(
        ask("PUT", "/v1/community/me/name", &big),
        (400, "too_large".to_owned())
    );
    // Looking people up with something that is not an ID or a name.
    for path in [
        "/v1/community/people/not-an-id",
        "/v1/community/people/not-an-id/devices",
        "/v1/community/people/by-name/Not-A-Name",
        "/v1/community/people/by-name/ab",
        "/v1/community/people/cm_01JA2B3C4D5E6F7G8H9J0K1M2N",
        "/v1/community/people/cm_01JA2B3C4D5E6F7G8H9J0K1M2N/picture",
        "/v1/community/people/cm_01JA2B3C4D5E6F7G8H9J0K1M2N/devices",
    ] {
        assert_eq!(
            ask("GET", path, ""),
            (404, "not_found".to_owned()),
            "{path}"
        );
    }
    assert_eq!(
        ask("PUT", "/v1/community/blocks/not-an-id", ""),
        (404, "not_found".to_owned())
    );
    assert_eq!(
        ask("PUT", &format!("/v1/community/blocks/{}", alice.id), ""),
        bad,
        "not yourself"
    );
    assert_eq!(
        ask("POST", "/v1/community/contacts/not-an-id/accept", ""),
        (404, "not_found".to_owned())
    );
    assert_eq!(
        ask("GET", "/v1/community/me/", ""),
        (404, "not_found".to_owned()),
        "a trailing slash is another path"
    );
    assert_eq!(
        ask("PATCH", "/v1/community/me", ""),
        (404, "not_found".to_owned())
    );
    // A member who has not joined is not a member.
    let account = service.add_account("Newcomer", "new@example.com", true);
    let pc = sign_in(&service, account, "PC").await;
    let answer = raw(
        &service,
        "GET",
        "/v1/community/contacts",
        Some(pc.pass()),
        "",
    );
    assert_eq!(
        (answer.status, error_code_of(&answer).as_str()),
        (403, "not_member")
    );
    let me = raw(&service, "GET", "/v1/community/me", Some(pc.pass()), "");
    assert_eq!(me.status, 200, "but they may ask who they are");
}
