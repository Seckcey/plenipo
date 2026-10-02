//! Phone access, end to end in Rust (Phase 14, ADR-141 to ADR-145): the PC's service and its
//! relay link, a stand-in relay on 127.0.0.1, and a stand-in phone, with a stand-in for the app's
//! own services. No real relay, no real phone, made-up data only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plenipo_guard::remote::{ApprovalFacts, KeptOnPc, Why};
use plenipo_guard::SensitiveKind;
use plenipo_licensing::answer::{self, AnswerPayload};
use plenipo_licensing::{SignedAnswer, SubscriptionState};
use plenipo_remote::code::Code;
use plenipo_remote::devices::{MemoryConfig, MemoryStore};
use plenipo_remote::link::{self, LinkHost};
use plenipo_remote::protocol::{
    Ask, Changed, Event, NoticeAbout, PairStep, PhoneNotice, SignedOutWhy,
};
use plenipo_remote::service::PairingView;
use plenipo_remote::stand_in::{Bad, NetPhone, PhoneError, Relay};
use plenipo_remote::{b64, Change, Clock, Host, Phone, Remote, Settings};
use serde_json::{json, Value};

// ---- A stand-in for the app ----------------------------------------------------------------

#[derive(Default)]
struct App {
    free: AtomicBool,
    /// Approvals: id → (facts, state).
    approvals: Mutex<HashMap<String, (ApprovalFacts, &'static str)>>,
    records: Mutex<Vec<(Option<String>, String, Value)>>,
    carried: Mutex<Vec<String>>,
    changes: Mutex<Vec<Change>>,
    answer_state: Mutex<Option<SubscriptionState>>,
}

impl App {
    fn records(&self, event: &str) -> Vec<Value> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e, _)| e == event)
            .map(|(_, _, v)| v.clone())
            .collect()
    }

    fn add_approval(&self, id: &str, facts: ApprovalFacts) {
        self.approvals
            .lock()
            .unwrap()
            .insert(id.into(), (facts, "pending"));
    }

    /// The owner answers on the PC (the first answer counts).
    fn answer_on_pc(&self, id: &str, state: &'static str) -> Result<(), String> {
        let mut a = self.approvals.lock().unwrap();
        let entry = a.get_mut(id).ok_or("gone")?;
        if entry.1 != "pending" {
            return Err(format!("that request was already {}", entry.1));
        }
        entry.1 = state;
        Ok(())
    }
}

impl Host for App {
    fn pro(&self) -> bool {
        !self.free.load(Ordering::SeqCst)
    }

    fn approval(&self, _org: &str, approval: &str) -> Option<ApprovalFacts> {
        self.approvals
            .lock()
            .unwrap()
            .get(approval)
            .filter(|(_, s)| *s == "pending")
            .map(|(f, _)| *f)
    }

    fn carry_out(&self, _phone: &Phone, ask: &Ask) -> Result<Value, String> {
        self.carried
            .lock()
            .unwrap()
            .push(ask.kind().map(|k| k.label()).unwrap_or("").to_owned());
        match ask {
            Ask::Approve { approval, .. } => self
                .answer_on_pc(approval, "approved")
                .map(|()| json!({ "status": "approved" })),
            Ask::Refuse { approval, .. } => self
                .answer_on_pc(approval, "refused")
                .map(|()| json!({ "status": "refused" })),
            Ask::ReadApprovals { .. } => Ok(json!([{
                "summary": "Approve: git push to Website",
                "detail": "git push origin main",
            }])),
            Ask::ReadHome { .. } => Ok(json!({ "home": "Waiting for you: 1" })),
            _ => Ok(json!({ "done": true })),
        }
    }

    fn record(&self, org: Option<&str>, event: &str, payload: Value) {
        self.records
            .lock()
            .unwrap()
            .push((org.map(str::to_owned), event.to_owned(), payload));
    }

    fn changed(&self, what: Change) {
        self.changes.lock().unwrap().push(what);
    }
}

fn now_secs() -> i64 {
    plenipo_licensing::clock()
}

impl LinkHost for App {
    fn check_address(&self, address: &str) -> Result<(), String> {
        // As Guard's rule in a copy built for the tests: only the stand-in's relay path.
        if address.starts_with("http://127.0.0.1:") && address.ends_with("/plenipo/v1/pc") {
            Ok(())
        } else {
            Err(format!("Plenipo refused to reach {address}"))
        }
    }

    fn weekly_answer(&self) -> Option<SignedAnswer> {
        let state = (*self.answer_state.lock().unwrap()).unwrap_or(SubscriptionState::Active);
        let payload = AnswerPayload {
            v: 1,
            key_id: "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T".into(),
            state,
            paid_through: Some(now_secs() + 86_400),
            ends_at: match state {
                SubscriptionState::Cancelled | SubscriptionState::Ended => Some(now_secs() - 1),
                _ => None,
            },
            as_of: now_secs(),
            signer: plenipo_licensing::trust::TEST_KEY_ID.into(),
        };
        Some(answer::sign(
            &payload,
            &plenipo_licensing::trust::test_signing_key(),
        ))
    }
}

/// A clock the tests move by hand (starting at the real time).
struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl TestClock {
    fn new() -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(
            u64::try_from(now_secs()).unwrap() * 1000,
        )))
    }

    fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

struct World {
    relay: Relay,
    app: Arc<App>,
    clock: Arc<TestClock>,
    remote: Arc<Remote>,
    link: Option<tokio::task::JoinHandle<()>>,
}

const ORIGIN: &str = plenipo_remote::PAGE_ORIGIN;

impl World {
    async fn new() -> Self {
        let relay = Relay::start().await;
        let app = Arc::new(App::default());
        let clock = TestClock::new();
        let remote = Remote::new(
            Settings {
                origin: ORIGIN.into(),
                rp_id: plenipo_remote::RP_ID.into(),
                pc_name: "Office PC".into(),
                version: "1.19.0".into(),
            },
            Arc::new(MemoryStore::default()),
            Arc::new(MemoryConfig::default()),
            app.clone(),
            clock.clone(),
        );
        remote.set_switched_on(true).unwrap();
        let mut world = Self {
            relay,
            app,
            clock,
            remote,
            link: None,
        };
        world.connect().await;
        world
    }

    async fn connect(&mut self) {
        let host: Arc<dyn LinkHost> = self.app.clone();
        self.link = Some(tokio::spawn(link::run(
            self.remote.clone(),
            self.relay.pc_address(),
            host,
        )));
        wait_for(
            || self.remote.view().connected && self.relay.pc_connected(),
            "the PC to connect",
        )
        .await;
    }

    async fn disconnect(&mut self) {
        if let Some(l) = self.link.take() {
            l.abort();
            let _ = l.await;
        }
        self.remote.relay_down(None);
        wait_for(|| !self.relay.pc_connected(), "the PC to leave the relay").await;
    }

    fn code(&self) -> Code {
        let Some(PairingView::Showing { code, .. }) = self.remote.view().pairing else {
            panic!("no code is shown");
        };
        Code::parse(&code).unwrap()
    }

    /// Show a code on the PC (Add a phone), once the relay has its mailbox open: a person takes
    /// longer to type it than the PC takes to tell the relay.
    async fn new_code(&self) -> Code {
        self.remote.start_pairing().unwrap();
        let code = self.code();
        let mailbox = code.mailbox();
        wait_for(
            || self.relay.mailbox().as_deref() == Some(mailbox.as_str()),
            "the relay to open the mailbox",
        )
        .await;
        code
    }

    /// Pair a new phone, the owner saying yes.
    async fn paired_phone(&self, name: &str) -> NetPhone {
        let code = self.new_code().await;
        let mut phone = NetPhone::new(&self.relay.phone_address());
        phone.start_pairing(&code, name).await.unwrap();
        wait_for(
            || matches!(self.remote.view().pairing, Some(PairingView::Asking { .. })),
            "Is this your phone?",
        )
        .await;
        self.remote.answer_pairing(true).unwrap();
        assert_eq!(phone.finish_pairing().await.unwrap(), PairStep::Done);
        phone
    }
}

async fn wait_for(mut what: impl FnMut() -> bool, label: &str) {
    for _ in 0..400 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out waiting for {label}");
}

fn ok(reply: &plenipo_remote::protocol::Reply) -> &Value {
    reply
        .ok
        .as_ref()
        .unwrap_or_else(|| panic!("not ok: {reply:?}"))
}

fn refused_why(reply: &plenipo_remote::protocol::Reply) -> Option<Why> {
    reply
        .refused
        .as_ref()
        .unwrap_or_else(|| panic!("not refused: {reply:?}"))
        .why
}

// ---- Pairing (ADR-141) -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_is_paired_at_the_pc_and_signs_in() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Frank's iPhone").await;
    let view = w.remote.view();
    assert_eq!(view.devices.len(), 1);
    assert_eq!(view.devices[0].name, "Frank's iPhone");
    assert_eq!(view.devices[0].browser, "Stand-in browser");
    assert!(view.pairing.is_none());
    assert_eq!(w.app.records("remote.device_added").len(), 1);

    // Making the passkey signed it in: the first meeting needs no new check.
    let welcome = phone.meet(false).await.unwrap();
    assert!(welcome.signed_in);
    assert_eq!(welcome.pc_name, "Office PC");
    let reply = phone
        .ask(Ask::ReadHome {
            org: "first".into(),
        })
        .await
        .unwrap();
    assert_eq!(ok(&reply)["home"], "Waiting for you: 1");
    let requests = w.app.records("remote.request");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["name"], "Frank's iPhone");
    assert_eq!(requests[0]["kind"], "read Home");

    // A later meeting, once the sign-in lapsed, signs in with the passkey.
    phone.close().await;
    w.clock.advance(plenipo_remote::SIGNED_IN_IDLE_MS);
    w.remote.tick();
    let welcome = phone.meet(false).await.unwrap();
    assert!(!welcome.signed_in);
    assert!(welcome.challenge.is_some());
    let refused = phone
        .ask(Ask::ReadHome {
            org: "first".into(),
        })
        .await
        .unwrap();
    assert_eq!(refused_why(&refused), Some(Why::NotSignedIn));
    let signed = phone.sign_in().await.unwrap();
    assert!(ok(&signed)["pass"].is_string());
    let reply = phone
        .ask(Ask::ReadHome {
            org: "first".into(),
        })
        .await
        .unwrap();
    assert!(reply.ok.is_some());
    assert_eq!(w.app.records("remote.signed_in").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_code_fails_three_times_and_dies() {
    let w = World::new().await;
    let right = w.new_code().await;
    let wrong = Code::new();
    for n in 1..=3 {
        // A wrong code finds no mailbox at all (the relay knows the mailbox by the code)...
        let mut phone = NetPhone::new(&w.relay.phone_address());
        let err = phone.start_pairing(&wrong, "x").await.unwrap_err();
        assert_eq!(err, PhoneError::Relay("mailbox_closed".into()), "try {n}");
    }
    // ...and the right mailbox with the wrong code (a phone that guessed the mailbox) fails the
    // first meeting: three of those kill the code.
    for n in 1..=3 {
        let mut phone = NetPhone::new(&w.relay.phone_address());
        let err = phone
            .start_pairing_with_psk(&right, [7u8; 32], "x")
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                PhoneError::Closed | PhoneError::Timeout | PhoneError::Meeting
            ),
            "try {n}: {err:?}"
        );
        wait_for(
            || match w.remote.view().pairing {
                Some(PairingView::Showing { wrong, .. }) => wrong == n,
                None => n == 3,
                _ => false,
            },
            "the wrong try to count",
        )
        .await;
    }
    assert!(w.remote.view().pairing.is_none(), "the code died");
    let refused = w.app.records("remote.pairing_refused");
    assert_eq!(refused.last().unwrap()["reason"], "wrong_code");
    // The right code no longer works either.
    let mut phone = NetPhone::new(&w.relay.phone_address());
    assert!(phone.start_pairing(&right, "x").await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn three_dead_codes_pause_adding_a_phone() {
    let w = World::new().await;
    for _ in 0..3 {
        let code = w.new_code().await;
        for _ in 0..3 {
            let mut phone = NetPhone::new(&w.relay.phone_address());
            let _ = phone.start_pairing_with_psk(&code, [9u8; 32], "x").await;
        }
        wait_for(|| w.remote.view().pairing.is_none(), "the code to die").await;
    }
    let err = w.remote.start_pairing().unwrap_err();
    assert!(err.to_string().contains("wrong codes"), "{err}");
    assert!(w.remote.view().pairing_paused_until.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expired_code_is_refused() {
    let w = World::new().await;
    let code = w.new_code().await;
    w.clock.advance(plenipo_remote::CODE_LIFE_MS);
    w.remote.tick();
    assert!(w.remote.view().pairing.is_none());
    wait_for(
        || w.relay.mailbox().is_none(),
        "the relay to close the mailbox",
    )
    .await;
    let refused = w.app.records("remote.pairing_refused");
    assert_eq!(refused.last().unwrap()["reason"], "expired");
    let mut phone = NetPhone::new(&w.relay.phone_address());
    assert_eq!(
        phone.start_pairing(&code, "late").await.unwrap_err(),
        PhoneError::Relay("mailbox_closed".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_added_until_the_owner_says_yes() {
    let w = World::new().await;
    let code = w.new_code().await;
    let mut phone = NetPhone::new(&w.relay.phone_address());
    phone.start_pairing(&code, "Not mine").await.unwrap();
    wait_for(
        || matches!(w.remote.view().pairing, Some(PairingView::Asking { .. })),
        "Is this your phone?",
    )
    .await;
    assert!(w.remote.view().devices.is_empty());
    // A second phone with the same code cannot join: the code is used.
    let mut other = NetPhone::new(&w.relay.phone_address());
    assert!(other.start_pairing(&code, "Also not mine").await.is_err());
    w.remote.answer_pairing(false).unwrap();
    let step = phone.finish_pairing().await.unwrap();
    assert!(matches!(step, PairStep::Refused { .. }), "{step:?}");
    assert!(w.remote.view().devices.is_empty());
    assert_eq!(
        w.app.records("remote.pairing_refused").last().unwrap()["reason"],
        "not_added"
    );
}

// ---- Meetings, sign-in, and ending them (ADR-142, ADR-143) ---------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_phone_is_refused() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Mine").await;
    // A stranger who copied this phone's pass, but not its key, fails the very first message.
    let mut stranger = NetPhone::new(&w.relay.phone_address());
    stranger.paired = phone.paired.clone();
    assert!(stranger.meet(false).await.is_err());
    // The real phone still meets.
    assert!(phone.meet(false).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_phone_is_refused_at_once() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Lost phone").await;
    phone.meet(false).await.unwrap();
    let id = w.remote.view().devices[0].id.clone();
    w.remote.remove(&id).unwrap();
    // Told at once, and cut off.
    assert_eq!(
        phone.event().await.unwrap(),
        Event::SignedOut {
            why: SignedOutWhy::Removed
        }
    );
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    // Its pass no longer opens the relay.
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("bad_pass".into())
    );
    assert!(w.remote.view().devices.is_empty());
    assert_eq!(w.app.records("remote.device_removed")[0]["by"], "pc");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_removed_while_away_is_told_it_is_not_listed() {
    let mut w = World::new().await;
    let mut phone = w.paired_phone("Drawer phone").await;
    phone.meet(false).await.unwrap();
    phone.close().await;
    let id = w.remote.view().devices[0].id.clone();
    w.remote.remove(&id).unwrap();
    // The relay keeps nothing: once it meets the PC again, it has forgotten the drop.
    w.disconnect().await;
    w.connect().await;
    let failed = w.app.records("remote.meetings_stopped").len();
    let bad_passes = || {
        w.relay
            .refused()
            .iter()
            .filter(|c| *c == "bad_pass")
            .count()
    };
    let before = bad_passes();
    // The phone comes back: its pass still opens the relay, but the PC does not know it, so the
    // relay is told to refuse it, and the phone hears it is no longer on the list.
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("bad_pass".into())
    );
    // From then on the relay refuses it before the PC hears of it.
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("bad_pass".into())
    );
    assert_eq!(bad_passes() - before, 2);
    assert_eq!(w.app.records("remote.meetings_stopped").len(), failed);
    assert!(w.remote.view().devices.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pc_sends_the_relay_only_what_the_contract_lists() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Frank's phone").await;
    // Paired is signed in.
    assert!(phone.meet(false).await.unwrap().signed_in);
    phone.ask(Ask::ReadControl).await.unwrap();
    let id = w.remote.view().devices[0].id.clone();
    w.remote.remove(&id).unwrap();
    wait_for(
        || {
            w.relay
                .pc_said()
                .iter()
                .any(|t| t.contains(r#""t":"drop""#))
        },
        "the PC to drop the phone's pass",
    )
    .await;
    let said = w.relay.pc_said();
    // First: the PC's relay key, its proof that it holds it, and 8 West's signed weekly answer.
    let hello: Value = serde_json::from_str(&said[0]).unwrap();
    let mut fields: Vec<&String> = hello.as_object().unwrap().keys().collect();
    fields.sort();
    assert_eq!(fields, ["answer", "key", "proof", "t", "v"]);
    assert_eq!(hello["t"], "hello");
    let shown: SignedAnswer = serde_json::from_value(hello["answer"].clone()).unwrap();
    assert_eq!(
        answer::verify(&shown).unwrap().key_id,
        "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T"
    );
    // Then only the pairing mailbox, sealed messages, and the passes it drops: never a word of
    // what the phone and the PC said, the phone's name, or the PC's.
    for text in &said[1..] {
        let m: Value = serde_json::from_str(text).unwrap();
        let t = m["t"].as_str().unwrap();
        assert!(
            ["mailbox", "close_mailbox", "drop", "send", "close"].contains(&t),
            "{text}"
        );
        for words in [
            "Frank's phone",
            "Office PC",
            "readControl",
            "signIn",
            "passkey",
        ] {
            assert!(!text.contains(words), "{words} in {text}");
        }
    }
    assert!(said.iter().any(|t| t.contains(r#""t":"send""#)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_cannot_ask_for_anything_off_the_list() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Mine").await;
    assert!(phone.meet(false).await.unwrap().signed_in);
    let id = b64::encode(&[7u8; 16]);
    // A program, the terminal, a file, a setting: none is on the phone's list.
    for asked in [
        json!({ "kind": "runProgram", "program": "cmd.exe", "args": ["/c", "dir"] }),
        json!({ "kind": "readFile", "path": "C:/Users/me/secrets.txt" }),
        json!({ "kind": "openTerminal" }),
        json!({ "kind": "setSwitch", "switch": "remote", "on": true }),
        json!({ "kind": "approve", "org": "first", "approval": "a1", "andAlso": "rm -rf" }),
    ] {
        let said = json!({ "t": "ask", "id": id, "again": false, "ask": asked });
        phone
            .say_bytes(serde_json::to_vec(&said).unwrap())
            .await
            .unwrap();
        // The PC ends the meeting at once, and carries out nothing.
        assert!(phone.hear().await.is_err(), "{asked}");
        assert!(w.app.carried.lock().unwrap().is_empty(), "{asked}");
        phone.meet(false).await.unwrap();
    }
    assert!(w.app.records("remote.request").is_empty());
}

/// A phone's notice keys, as its browser makes them when it signs up for notices.
fn notice_keys() -> (p256::SecretKey, plenipo_remote::protocol::Subscription) {
    let key = loop {
        if let Ok(k) = p256::SecretKey::from_slice(&plenipo_remote::random::<32>()) {
            break k;
        }
    };
    let to = plenipo_remote::protocol::Subscription {
        endpoint: "https://fcm.googleapis.com/fcm/send/phone-1".into(),
        p256dh: b64::encode(&key.public_key().to_sec1_bytes()),
        auth: b64::encode(&[5u8; 16]),
    };
    (key, to)
}

fn a_notice() -> PhoneNotice {
    PhoneNotice {
        v: 1,
        kind: "approvals".into(),
        org: "first".into(),
        title: "Senior Developer is waiting for your OK".into(),
        body: "Git push origin".into(),
        about: Some(NoticeAbout::Approval { id: "a1".into() }),
        tag: "approval:first:a1".into(),
        at: 1,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn notices_go_only_to_phones_that_asked_and_only_they_can_read_them() {
    let w = World::new().await;
    let mut asked = w.paired_phone("Asked").await;
    let mut quiet = w.paired_phone("Quiet").await;
    // The PC gives its notice key in every meeting; a phone signs up with it.
    let welcome = asked.meet(false).await.unwrap();
    let notice_key = welcome.notice_key.clone().expect("the PC's notice key");
    assert_eq!(
        quiet.meet(false).await.unwrap().notice_key,
        Some(notice_key.clone())
    );
    let (phone_key, to) = notice_keys();
    let r = asked
        .ask(Ask::NoticesOn {
            subscription: to.clone(),
        })
        .await
        .unwrap();
    assert_eq!(ok(&r)["notices"], true);

    // One sealed notice, for the phone that asked, to its own notice service.
    let sealed = w.remote.sealed_notices(&a_notice());
    assert_eq!(sealed.len(), 1);
    let (phone, notice) = &sealed[0];
    assert_eq!(phone.name, "Asked");
    let notice = notice.as_ref().unwrap();
    assert_eq!(notice.endpoint, to.endpoint);
    let header = |name: &str| {
        notice
            .headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    assert!(header("Authorization").ends_with(&format!(", k={notice_key}")));
    assert_eq!(header("Urgency"), "high");
    // Only that phone can open it, and it says exactly the notice.
    let opened = plenipo_remote::webpush::decrypt(&notice.body, &phone_key, &[5u8; 16]).unwrap();
    let read: PhoneNotice = serde_json::from_slice(&opened).unwrap();
    assert_eq!(read, a_notice());
    assert!(!notice.body.windows(8).any(|w| w == b"git push"));

    // Notices on my phones, off on the PC: nothing goes out.
    w.remote.set_phone_notices(false).unwrap();
    assert!(!w.remote.view().phone_notices);
    assert!(w.remote.sealed_notices(&a_notice()).is_empty());
    w.remote.set_phone_notices(true).unwrap();
    assert_eq!(w.remote.sealed_notices(&a_notice()).len(), 1);
    // Free: nothing goes out either.
    w.app.free.store(true, Ordering::SeqCst);
    assert!(w.remote.sealed_notices(&a_notice()).is_empty());
    w.app.free.store(false, Ordering::SeqCst);

    // The notice service says the address is gone: the PC forgets it, and says so.
    w.remote
        .notice_delivered(phone, "approvals", plenipo_remote::service::Delivered::Gone);
    assert!(w.remote.sealed_notices(&a_notice()).is_empty());
    let failed = w.app.records("remote.notice_failed");
    assert_eq!(failed[0]["why"], "gone");
    assert_eq!(failed[0]["kind"], "approvals");
    assert!(
        !failed[0].to_string().contains("git push"),
        "never what it said"
    );
    w.remote
        .notice_delivered(phone, "approvals", plenipo_remote::service::Delivered::Sent);
    assert_eq!(w.app.records("remote.notice_sent")[0]["name"], "Asked");
}

#[tokio::test(flavor = "multi_thread")]
async fn signing_out_and_lapsing_end_the_sign_in() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    let out = phone.ask(Ask::SignOut).await.unwrap();
    assert_eq!(ok(&out)["signedOut"], true);
    let after = phone.ask(Ask::ReadControl).await.unwrap();
    assert_eq!(refused_why(&after), Some(Why::NotSignedIn));

    // Signed in again, then 12 hours pass while it keeps working.
    phone.close().await;
    phone.meet(false).await.unwrap();
    phone.sign_in().await.unwrap();
    for _ in 0..25 {
        w.clock.advance(29 * 60 * 1000);
        phone.ask(Ask::ReadControl).await.unwrap();
    }
    w.remote.tick();
    assert_eq!(
        phone.event().await.unwrap(),
        Event::SignedOut {
            why: SignedOutWhy::TwelveHours
        }
    );
    let whys: Vec<Value> = w
        .app
        .records("remote.signed_out")
        .iter()
        .map(|r| r["why"].clone())
        .collect();
    assert_eq!(whys, [json!("you"), json!("twelve_hours")]);
}

#[tokio::test(flavor = "multi_thread")]
async fn switching_off_cuts_every_phone_off_at_once() {
    let w = World::new().await;
    let mut a = w.paired_phone("A").await;
    let mut b = w.paired_phone("B").await;
    a.meet(false).await.unwrap();
    b.meet(false).await.unwrap();
    w.remote.set_switched_on(false).unwrap();
    for phone in [&mut a, &mut b] {
        assert_eq!(
            phone.event().await.unwrap(),
            Event::SignedOut {
                why: SignedOutWhy::SwitchedOff
            }
        );
        assert!(phone.ask(Ask::ReadControl).await.is_err());
    }
    // The phones stay listed.
    assert_eq!(w.remote.view().devices.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn three_refused_checks_pause_the_phone() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    w.clock.advance(plenipo_remote::SIGNED_IN_IDLE_MS);
    w.remote.tick();
    phone.passkey.verify_user = false;
    for n in 1..=3 {
        phone.close().await;
        phone.meet(false).await.unwrap();
        let r = phone.sign_in().await.unwrap();
        let expect = if n == 3 { Some(Why::Paused) } else { None };
        assert_eq!(refused_why(&r), expect, "try {n}");
    }
    let view = w.remote.view();
    assert!(view.devices[0].paused);
    assert_eq!(w.app.records("remote.check_refused").len(), 3);
    assert_eq!(w.app.records("remote.device_paused").len(), 1);
    // Even a right answer is refused until the owner un-pauses it on the PC.
    phone.passkey.verify_user = true;
    phone.close().await;
    phone.meet(false).await.unwrap();
    assert_eq!(
        refused_why(&phone.sign_in().await.unwrap()),
        Some(Why::Paused)
    );
    w.remote.unpause(&view.devices[0].id).unwrap();
    phone.close().await;
    phone.meet(false).await.unwrap();
    assert!(phone.sign_in().await.unwrap().ok.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn too_many_failed_meetings_slow_then_stop_the_pc() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Mine").await;
    phone.meet(false).await.unwrap();
    for _ in 0..10 {
        let mut stranger = NetPhone::new(&w.relay.phone_address());
        stranger.paired = phone.paired.clone();
        let _ = stranger.meet(false).await;
    }
    wait_for(
        || w.remote.view().meetings_stopped_until.is_some(),
        "the PC to stop answering",
    )
    .await;
    assert_eq!(w.app.records("remote.meetings_stopped").len(), 1);
    // A new meeting is not answered, even the owner's own...
    let mut again = NetPhone::new(&w.relay.phone_address());
    again.paired = phone.paired.clone();
    again.noise = phone.noise;
    assert!(again.meet(false).await.is_err());
    // ...but the phone already signed in keeps working.
    assert!(phone.ask(Ask::ReadControl).await.unwrap().ok.is_some());
    // A minute later, the PC answers again.
    w.clock.advance(60_000);
    assert!(again.meet(false).await.is_ok());
}

// ---- Copies, and a relay that misbehaves (ADR-143 §6, §14) --------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_copied_request_is_refused() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.app.add_approval("a1", ApprovalFacts::default());
    let id = b64::encode(&[5u8; 16]);
    let ask = Ask::Refuse {
        org: "first".into(),
        approval: "a1".into(),
    };
    let first = phone.ask_with(&id, false, ask.clone()).await.unwrap();
    assert!(first.ok.is_some());
    // The same request ID again, sealed again by the phone: refused.
    let second = phone.ask_with(&id, false, ask).await.unwrap();
    assert!(second.refused.is_some(), "{second:?}");
    assert_eq!(w.app.records("remote.refused")[0]["why"], "copied");
    assert_eq!(*w.app.carried.lock().unwrap(), ["refuse"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_cannot_read_change_replay_or_invent_a_request() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.app.add_approval("a1", ApprovalFacts::default());
    let secret_objective = "Ship the quarterly numbers to Contoso";
    phone.ask(Ask::ReadApprovals { org: None }).await.unwrap();
    phone
        .ask(Ask::SendObjective {
            org: "first".into(),
            position: "manager".into(),
            project: None,
            text: secret_objective.into(),
        })
        .await
        .unwrap();
    // Read: nothing the relay passed holds the words, in any form.
    for sealed in w.relay.seen() {
        let text = String::from_utf8_lossy(&sealed);
        for word in [
            "git push",
            "Contoso",
            "quarterly",
            "Frank",
            "approve",
            "first",
        ] {
            assert!(!text.contains(word), "the relay saw {word:?}");
        }
    }

    // Changed: the PC ends the meeting.
    w.relay.set_bad(Bad::Change);
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    w.relay.set_bad(Bad::Honest);

    // Replayed through the relay: the copy fails, and the meeting ends.
    phone.meet(false).await.unwrap();
    phone
        .ask(Ask::Refuse {
            org: "first".into(),
            approval: "a1".into(),
        })
        .await
        .unwrap();
    let carried = w.app.carried.lock().unwrap().len();
    assert!(w.relay.replay_last_to_pc());
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    assert_eq!(
        w.app.carried.lock().unwrap().len(),
        carried,
        "nothing ran twice"
    );

    // Invented: the PC ends the meeting, and nothing runs.
    phone.meet(false).await.unwrap();
    assert!(w.relay.invent_to_pc());
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    assert_eq!(w.app.carried.lock().unwrap().len(), carried);

    // Dropped: the phone hears nothing back (and says so after waiting).
    phone.meet(false).await.unwrap();
    w.relay.set_bad(Bad::Drop);
    assert_eq!(
        phone.ask(Ask::ReadControl).await.unwrap_err(),
        PhoneError::Timeout
    );
}

// ---- Guard decides every request (ADR-145) ------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn approving_and_refusing_go_through_guard_and_the_first_answer_counts() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.app.add_approval("a1", ApprovalFacts::default());
    w.app.add_approval("a2", ApprovalFacts::default());
    // The phone first: approved; then the PC's answer changes nothing.
    let r = phone
        .ask(Ask::Approve {
            org: "first".into(),
            approval: "a1".into(),
        })
        .await
        .unwrap();
    assert_eq!(ok(&r)["status"], "approved");
    assert!(w.app.answer_on_pc("a1", "refused").is_err());
    // The PC first: then the phone's answer is not one of a waiting approval.
    w.app.answer_on_pc("a2", "refused").unwrap();
    let r = phone
        .ask(Ask::Approve {
            org: "first".into(),
            approval: "a2".into(),
        })
        .await
        .unwrap();
    assert_eq!(refused_why(&r), Some(Why::NotAnApproval));
    assert_eq!(w.app.approvals.lock().unwrap()["a2"].1, "refused");
}

#[tokio::test(flavor = "multi_thread")]
async fn approvals_kept_on_the_pc_cannot_be_answered_from_a_phone() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.remote
        .set_kept(KeptOnPc {
            every: false,
            production_servers: false,
            kinds: vec![SensitiveKind::Payment],
        })
        .unwrap();
    w.app.add_approval(
        "pay",
        ApprovalFacts {
            sensitive: Some(SensitiveKind::Payment),
            environment: None,
        },
    );
    for ask in [
        Ask::Approve {
            org: "first".into(),
            approval: "pay".into(),
        },
        Ask::Refuse {
            org: "first".into(),
            approval: "pay".into(),
        },
    ] {
        let r = phone.ask(ask).await.unwrap();
        assert_eq!(refused_why(&r), Some(Why::KeptOnPc));
    }
    assert_eq!(w.app.approvals.lock().unwrap()["pay"].1, "pending");
    assert!(w.app.carried.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_notice_may_only_say_no() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    w.clock.advance(plenipo_remote::SIGNED_IN_IDLE_MS);
    w.remote.tick();
    w.app.add_approval("a1", ApprovalFacts::default());
    // A notice's button: not signed in, the phone's own key only.
    let welcome = phone.meet(true).await.unwrap();
    assert!(!welcome.signed_in);
    assert!(welcome.challenge.is_none());
    let r = phone
        .ask(Ask::Approve {
            org: "first".into(),
            approval: "a1".into(),
        })
        .await
        .unwrap();
    assert_eq!(refused_why(&r), Some(Why::NotFromANotice));
    let r = phone
        .ask(Ask::Refuse {
            org: "first".into(),
            approval: "a1".into(),
        })
        .await
        .unwrap();
    assert_eq!(ok(&r)["status"], "refused");
    assert_eq!(w.app.records("remote.request")[0]["fromNotice"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn free_and_pro() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.app.free.store(true, Ordering::SeqCst);
    let r = phone.ask(Ask::ReadControl).await.unwrap();
    assert_eq!(refused_why(&r), Some(Why::NotPro));
    let err = w.remote.start_pairing().unwrap_err();
    assert!(err.to_string().contains("part of Pro"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_serves_only_a_pc_on_pro() {
    let mut w = World::new().await;
    w.disconnect().await;
    *w.app.answer_state.lock().unwrap() = Some(SubscriptionState::Ended);
    let host: Arc<dyn LinkHost> = w.app.clone();
    let result = link::connect_once(&w.remote, &w.relay.pc_address(), host.as_ref()).await;
    let problem = result.unwrap_err();
    assert!(problem.message.contains("not on Pro"), "{problem:?}");
    assert_eq!(w.relay.refused(), ["not_pro"]);
    // Guard's rule: only the relay's own address.
    let elsewhere = link::connect_once(
        &w.remote,
        "https://example.com/plenipo/v1/pc",
        host.as_ref(),
    )
    .await
    .unwrap_err();
    assert!(elsewhere.message.contains("refused"), "{elsewhere:?}");
}

// ---- The PC offline, and a connection lost part way (ADR-143 §9) ---------------------------

#[tokio::test(flavor = "multi_thread")]
async fn when_the_pc_is_offline_the_phone_is_told_and_nothing_changes() {
    let mut w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.disconnect().await;
    assert!(phone.ask(Ask::ReadControl).await.is_err());
    assert_eq!(
        phone.meet(false).await.unwrap_err(),
        PhoneError::Relay("pc_offline".into())
    );
    // Back: the sign-in is still good, and nothing waited at the relay to run.
    w.connect().await;
    let welcome = phone.meet(false).await.unwrap();
    assert!(welcome.signed_in);
    assert!(w.app.carried.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn after_a_lost_connection_the_phone_asks_what_happened() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.app.add_approval("a1", ApprovalFacts::default());
    let id = b64::encode(&[8u8; 16]);
    phone
        .ask_with(
            &id,
            false,
            Ask::Approve {
                org: "first".into(),
                approval: "a1".into(),
            },
        )
        .await
        .unwrap();
    phone.close().await;
    phone.meet(false).await.unwrap();
    let r = phone.ask(Ask::Outcome { of: id }).await.unwrap();
    assert_eq!(ok(&r)["known"], true);
    assert_eq!(ok(&r)["reply"]["ok"]["status"], "approved");
    let unknown = phone
        .ask(Ask::Outcome {
            of: b64::encode(&[9u8; 16]),
        })
        .await
        .unwrap();
    assert_eq!(ok(&unknown)["known"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_in_phones_hear_what_changed() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    w.remote.notify(Some("first"), Changed::Approvals);
    assert_eq!(
        phone.event().await.unwrap(),
        Event::Changed {
            org: Some("first".into()),
            what: Changed::Approvals
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_can_remove_itself() {
    let w = World::new().await;
    let mut phone = w.paired_phone("Phone").await;
    phone.meet(false).await.unwrap();
    let r = phone.ask(Ask::RemoveThisPhone).await.unwrap();
    assert_eq!(ok(&r)["removed"], true);
    // The PC answers first, then removes the phone and records it.
    wait_for(
        || !w.app.records("remote.device_removed").is_empty(),
        "the removal to be recorded",
    )
    .await;
    assert!(w.remote.view().devices.is_empty());
    assert_eq!(w.app.records("remote.device_removed")[0]["by"], "phone");
}
