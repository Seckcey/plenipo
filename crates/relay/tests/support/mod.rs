//! What the relay's tests share: a relay with short waits, Plenipo's real PC side with a stand-in
//! app behind it, raw WebSocket clients that speak the contract by hand, and a logger that keeps
//! every line so a test can prove what the relay never logs.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use plenipo_licensing::answer::{self, AnswerPayload};
use plenipo_licensing::{SignedAnswer, SubscriptionState};
use plenipo_relay::{ClientAddress, Config, Handle, Limits, Relay};
use plenipo_relay_contract::b64;
use plenipo_relay_contract::wire::{self, PcToRelay, RelayToPc, PC_PROOF_CONTEXT};
use plenipo_remote::code::Code;
use plenipo_remote::devices::{MemoryConfig, MemoryStore};
use plenipo_remote::keys::PcKeys;
use plenipo_remote::link::{self, LinkHost};
use plenipo_remote::protocol::Ask;
use plenipo_remote::protocol::PairStep;
use plenipo_remote::service::PairingView;
use plenipo_remote::stand_in::NetPhone;
use plenipo_remote::{Change, Clock, Host, Phone, Remote, Settings};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

// ---- Every log line, kept --------------------------------------------------------------------

static LOG_LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static LOGGER: Recorder = Recorder;
static INSTALL: Once = Once::new();

struct Recorder;

impl log::Log for Recorder {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        // The relay's own lines only (the WebSocket library traces every byte at this level).
        if record.target().starts_with("plenipo_relay") {
            LOG_LINES
                .lock()
                .unwrap()
                .push(format!("{} {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}

/// Keep every log line from here on (once per test program).
pub fn keep_logs() {
    INSTALL.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Debug);
    });
}

/// Every log line so far.
pub fn log_lines() -> Vec<String> {
    LOG_LINES.lock().unwrap().clone()
}

// ---- The relay under test ------------------------------------------------------------------

/// Short waits, so a test is quick; everything else as shipped.
pub fn quick_limits() -> Limits {
    Limits {
        first_message: Duration::from_secs(3),
        idle: Duration::from_secs(8),
        ping_every: Duration::from_secs(2),
        ..Limits::default()
    }
}

pub async fn start_relay(limits: Limits) -> Handle {
    start_relay_with_clock(limits, Arc::new(plenipo_licensing::clock)).await
}

/// A relay whose clock (Unix seconds) the test moves.
pub async fn start_relay_with_clock(limits: Limits, clock: plenipo_relay::Clock) -> Handle {
    keep_logs();
    Relay::start(Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        client_address: ClientAddress::Peer,
        limits,
        clock,
    })
    .await
    .expect("a free port")
}

pub fn now_secs() -> i64 {
    plenipo_licensing::clock()
}

/// 8 West's weekly answer for a test PC, signed with the contract's test key.
pub fn weekly_answer(state: SubscriptionState) -> SignedAnswer {
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
    answer::sign(&payload, &plenipo_licensing::trust::test_signing_key())
}

// ---- A stand-in for the app behind Plenipo's PC side ------------------------------------------

#[derive(Default)]
pub struct App {
    pub free: AtomicBool,
    pub records: Mutex<Vec<(String, Value)>>,
    pub carried: Mutex<Vec<String>>,
    pub answer_state: Mutex<Option<SubscriptionState>>,
}

impl App {
    pub fn records(&self, event: &str) -> Vec<Value> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| e == event)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

impl Host for App {
    fn pro(&self) -> bool {
        !self.free.load(Ordering::SeqCst)
    }

    fn approval(
        &self,
        _org: &str,
        _approval: &str,
    ) -> Option<plenipo_guard::remote::ApprovalFacts> {
        None
    }

    fn carry_out(&self, _phone: &Phone, ask: &Ask) -> Result<Value, String> {
        self.carried
            .lock()
            .unwrap()
            .push(ask.kind().map(|k| k.label()).unwrap_or("").to_owned());
        Ok(json!({ "done": true }))
    }

    fn record(&self, _org: Option<&str>, event: &str, payload: Value) {
        self.records
            .lock()
            .unwrap()
            .push((event.to_owned(), payload));
    }

    fn changed(&self, _what: Change) {}
}

impl LinkHost for App {
    fn check_address(&self, address: &str) -> Result<(), String> {
        if address.starts_with("http://127.0.0.1:") && address.ends_with("/plenipo/v1/pc") {
            Ok(())
        } else {
            Err(format!("Plenipo refused to reach {address}"))
        }
    }

    fn weekly_answer(&self) -> Option<SignedAnswer> {
        let state = (*self.answer_state.lock().unwrap()).unwrap_or(SubscriptionState::Active);
        Some(weekly_answer(state))
    }
}

/// A clock the tests move by hand (starting at the real time).
pub struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl TestClock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(
            u64::try_from(now_secs()).unwrap() * 1000,
        )))
    }

    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

// ---- Plenipo's PC, connected to the relay ----------------------------------------------------

pub struct World {
    pub relay: Handle,
    pub app: Arc<App>,
    pub clock: Arc<TestClock>,
    pub remote: Arc<Remote>,
    pub link: Option<tokio::task::JoinHandle<()>>,
}

impl World {
    pub async fn new() -> Self {
        Self::with_limits(quick_limits()).await
    }

    pub async fn with_limits(limits: Limits) -> Self {
        let relay = start_relay(limits).await;
        let app = Arc::new(App::default());
        let clock = TestClock::new();
        let remote = Remote::new(
            Settings {
                origin: plenipo_remote::PAGE_ORIGIN.into(),
                rp_id: plenipo_remote::RP_ID.into(),
                pc_name: "Office PC".into(),
                version: "1.19.3".into(),
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

    pub async fn connect(&mut self) {
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

    pub async fn disconnect(&mut self) {
        if let Some(l) = self.link.take() {
            l.abort();
            let _ = l.await;
        }
        self.remote.relay_down(None);
        wait_for(|| !self.relay.pc_connected(), "the PC to leave the relay").await;
    }

    pub fn code(&self) -> Code {
        let Some(PairingView::Showing { code, .. }) = self.remote.view().pairing else {
            panic!("no code is shown");
        };
        Code::parse(&code).unwrap()
    }

    /// Show a code on the PC (Add a phone), once the relay has its mailbox open.
    pub async fn new_code(&self) -> Code {
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
    pub async fn paired_phone(&self, name: &str) -> NetPhone {
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

pub async fn wait_for(mut what: impl FnMut() -> bool, label: &str) {
    for _ in 0..400 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out waiting for {label}");
}

// ---- Raw clients that speak the contract by hand -----------------------------------------------

/// Open a WebSocket to `address` (`http://` or `ws://`).
pub async fn open(address: &str) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let (ws, _) = tokio_tungstenite::connect_async(link::socket_address(address)).await?;
    Ok(ws)
}

/// The HTTP status the relay's door answered with, when it did not let a WebSocket in.
pub fn door_status(error: &tokio_tungstenite::tungstenite::Error) -> Option<u16> {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => Some(response.status().as_u16()),
        _ => None,
    }
}

/// The next text message, or `None` when the connection is over (closed, or no text for 5 s).
pub async fn next_text(ws: &mut Socket) -> Option<String> {
    next_text_within(ws, Duration::from_secs(5)).await
}

/// The next text message within `wait` (pings and pongs pass by), or `None`.
pub async fn next_text_within(ws: &mut Socket, wait: Duration) -> Option<String> {
    let until = tokio::time::Instant::now() + wait;
    loop {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        let next = tokio::time::timeout(left, ws.next()).await.ok()?;
        match next {
            Some(Ok(Message::Text(text))) => return Some(text.as_str().to_owned()),
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
            Some(Ok(_)) => {}
        }
    }
}

pub async fn send_text(ws: &mut Socket, text: impl Into<String>) {
    ws.send(Message::text(text.into())).await.expect("sent");
}

/// Wait until the relay closes this connection (at most `wait`); false if it stays open.
pub async fn closed_within(ws: &mut Socket, wait: Duration) -> bool {
    let until = tokio::time::Instant::now() + wait;
    while tokio::time::Instant::now() < until {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => return true,
            Ok(Some(Ok(_))) => {}
            Err(_) => return false,
        }
    }
    false
}

/// A PC by hand: the challenge, a hello with `keys` and a weekly answer in `state`. Gives the open
/// connection after `welcome`, or the relay's refusal code.
pub async fn raw_pc(
    relay: &Handle,
    keys: &PcKeys,
    state: SubscriptionState,
) -> Result<Socket, String> {
    let mut ws = open(&relay.pc_address()).await.map_err(|e| e.to_string())?;
    let first = next_text(&mut ws).await.ok_or("no challenge")?;
    let Some(RelayToPc::Challenge { nonce }) = wire::read(&first) else {
        return Err(format!("not a challenge: {first}"));
    };
    let hello = PcToRelay::Hello {
        v: 1,
        key: b64::encode(&keys.relay_public()),
        proof: b64::encode(&keys.relay_sign(PC_PROOF_CONTEXT, &nonce)),
        answer: weekly_answer(state),
    };
    send_text(&mut ws, wire::write(&hello)).await;
    match wire::read::<RelayToPc>(&next_text(&mut ws).await.ok_or("no welcome")?) {
        Some(RelayToPc::Welcome { pc }) if pc == keys.fingerprint() => Ok(ws),
        Some(RelayToPc::Refused { code }) => Err(code),
        other => Err(format!("unexpected: {other:?}")),
    }
}

/// A pass for a phone of `keys`'s PC, good for 90 days.
pub fn pass_for(keys: &PcKeys, phone: &str) -> String {
    plenipo_remote::pass::issue(keys, phone, now_secs(), plenipo_remote::PASS_LIFE_SECS)
}

pub fn phone_id(byte: u8) -> String {
    b64::encode(&[byte; 16])
}

/// A phone by hand, with `pass`: gives the open connection after `ready`, or the refusal code.
pub async fn raw_phone(relay: &Handle, first: &str) -> Result<Socket, String> {
    let mut ws = open(&relay.phone_address())
        .await
        .map_err(|e| e.to_string())?;
    send_text(&mut ws, first).await;
    match next_text(&mut ws).await.ok_or("no answer")?.as_str() {
        r#"{"t":"ready"}"# => Ok(ws),
        other => {
            let v: Value = serde_json::from_str(other).map_err(|e| e.to_string())?;
            Err(v["code"].as_str().unwrap_or(other).to_owned())
        }
    }
}

pub fn pass_message(pass: &str) -> String {
    json!({ "t": "pass", "pass": pass }).to_string()
}

pub fn mailbox_message(mailbox: &str) -> String {
    json!({ "t": "mailbox", "mailbox": mailbox }).to_string()
}

pub fn data_message(data: &str) -> String {
    json!({ "t": "data", "data": data }).to_string()
}

/// `GET path` on the relay by plain HTTP: the status and the body.
pub async fn http_get(relay: &Handle, path: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = TcpStream::connect(relay.address())
        .await
        .expect("connected");
    stream
        .write_all(format!("GET {path} HTTP/1.1\r\nHost: relay\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut text = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut text)).await;
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

pub fn counts(relay: &Handle) -> HashMap<String, u64> {
    relay.stats().refused.into_iter().collect()
}
