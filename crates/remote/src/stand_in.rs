//! Stand-ins for the tests (ADR-143 §14): a passkey, 8 West's relay, and a phone. Never in a
//! release.
//!
//! - [`Authenticator`] makes passkeys and sign-in answers as a phone's keychain does.
//! - [`Relay`] follows the relay contract (`contracts/phone-relay/v1`), and has a **bad relay**
//!   mode ([`Bad`]) that reads, changes, copies, drops, and makes up messages.
//! - [`NetPhone`] is a phone that pairs, meets, signs in, and asks, through the relay.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use ed25519_dalek::{Signer as _, Verifier as _};
use futures_util::{SinkExt as _, StreamExt as _};
use sha2::{Digest as _, Sha256};
use snow::TransportState;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

use crate::b64;
use crate::code::Code;
use crate::noise::{self, Assembler};
use crate::protocol::{
    Ask, Event, MeetingHello, MeetingWelcome, PairHello, PairStep, PcSays, PhoneSays, Reply,
};
use crate::webauthn::{NewPasskey, PasskeyAnswer, EDDSA, ES256};
use crate::wire::{self, codes, PcToRelay, PhoneToRelay, RelayToPc, RelayToPhone};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- A passkey ----------------------------------------------------------------------------------

enum PasskeyKey {
    P256(p256::ecdsa::SigningKey),
    Ed25519(ed25519_dalek::SigningKey),
}

impl Clone for PasskeyKey {
    fn clone(&self) -> Self {
        match self {
            Self::P256(k) => Self::P256(k.clone()),
            Self::Ed25519(k) => Self::Ed25519(k.clone()),
        }
    }
}

/// A phone's keychain, making passkeys and answers the way a phone does.
pub struct Authenticator {
    algorithm: i64,
    origin: String,
    rp_id: String,
    key: PasskeyKey,
    /// The credential's ID.
    pub id: Vec<u8>,
    counter: u32,
    /// The counter moves on with each use (synced passkeys keep it at 0).
    pub counts: bool,
    /// The person was checked (face, fingerprint, or passcode).
    pub verify_user: bool,
}

impl Authenticator {
    pub fn new(algorithm: i64, origin: &str, rp_id: &str) -> Self {
        let key = if algorithm == EDDSA {
            PasskeyKey::Ed25519(ed25519_dalek::SigningKey::from_bytes(&crate::random()))
        } else {
            loop {
                if let Ok(k) = p256::ecdsa::SigningKey::from_slice(&crate::random::<32>()) {
                    break PasskeyKey::P256(k);
                }
            }
        };
        Self {
            algorithm,
            origin: origin.to_owned(),
            rp_id: rp_id.to_owned(),
            key,
            id: crate::random::<32>().to_vec(),
            counter: 0,
            counts: true,
            verify_user: true,
        }
    }

    /// Use the same passkey as `other` (to make an answer for another page or site).
    pub fn adopt(&mut self, other: &Self) {
        self.algorithm = other.algorithm;
        self.key = other.key.clone();
        self.id.clone_from(&other.id);
        self.counter = other.counter;
    }

    fn flags(&self, made: bool) -> u8 {
        let mut f = 0x01;
        if self.verify_user {
            f |= 0x04;
        }
        if made {
            f |= 0x40;
        }
        f
    }

    fn auth_data(&mut self, made: bool) -> Vec<u8> {
        if self.counts {
            self.counter += 1;
        }
        let mut data = Sha256::digest(self.rp_id.as_bytes()).to_vec();
        data.push(self.flags(made));
        data.extend_from_slice(&self.counter.to_be_bytes());
        if made {
            data.extend_from_slice(&[0u8; 16]);
            data.extend_from_slice(&u16::try_from(self.id.len()).expect("short").to_be_bytes());
            data.extend_from_slice(&self.id);
            // A stand-in for the public key in COSE form (not read by Plenipo).
            data.extend_from_slice(&[0xa1, 0x01, 0x02]);
        }
        data
    }

    fn client_data(&self, kind: &str, challenge: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "type": kind,
            "challenge": b64::encode(challenge),
            "origin": self.origin,
            "crossOrigin": false,
        }))
        .expect("JSON")
    }

    fn public_der(&self) -> Vec<u8> {
        match &self.key {
            PasskeyKey::P256(k) => {
                let mut der = vec![
                    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
                    0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
                ];
                der.extend_from_slice(k.verifying_key().to_sec1_point(false).as_bytes());
                der
            }
            PasskeyKey::Ed25519(k) => {
                let mut der = vec![
                    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
                ];
                der.extend_from_slice(&k.verifying_key().to_bytes());
                der
            }
        }
    }

    /// `navigator.credentials.create()`.
    pub fn create(&mut self, challenge: &[u8]) -> NewPasskey {
        let data = self.auth_data(true);
        NewPasskey {
            id: b64::encode(&self.id),
            public_key: b64::encode(&self.public_der()),
            algorithm: self.algorithm,
            authenticator_data: b64::encode(&data),
            client_data: b64::encode(&self.client_data("webauthn.create", challenge)),
        }
    }

    /// `navigator.credentials.get()`.
    pub fn get(&mut self, challenge: &[u8]) -> PasskeyAnswer {
        let data = self.auth_data(false);
        let client = self.client_data("webauthn.get", challenge);
        let mut signed = data.clone();
        signed.extend_from_slice(&Sha256::digest(&client));
        let signature = match &self.key {
            PasskeyKey::P256(k) => {
                let sig: p256::ecdsa::Signature = k.sign(&signed);
                sig.to_der().as_bytes().to_vec()
            }
            PasskeyKey::Ed25519(k) => k.sign(&signed).to_bytes().to_vec(),
        };
        PasskeyAnswer {
            id: b64::encode(&self.id),
            authenticator_data: b64::encode(&data),
            client_data: b64::encode(&client),
            signature: b64::encode(&signature),
        }
    }
}

impl Default for Authenticator {
    fn default() -> Self {
        Self::new(ES256, crate::PAGE_ORIGIN, crate::RP_ID)
    }
}

// ---- The relay ----------------------------------------------------------------------------------

/// How a bad relay misbehaves with what phones send the PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bad {
    /// Passes everything as it is (a good relay that still keeps a copy to look at).
    #[default]
    Honest,
    /// Flips a bit in each message to the PC.
    Change,
    /// Passes each message to the PC twice.
    Replay,
    /// Drops every message to the PC.
    Drop,
    /// Adds a made-up message after each one to the PC.
    Invent,
}

struct PcLink {
    out: UnboundedSender<Message>,
    relay_public: [u8; 32],
    mailbox: Option<String>,
    mailbox_tries: u32,
    dropped: HashMap<String, i64>,
}

struct PhoneLink {
    out: UnboundedSender<Message>,
    pc: String,
    phone: Option<String>,
}

#[derive(Default)]
struct Hub {
    pcs: HashMap<String, PcLink>,
    phones: HashMap<String, PhoneLink>,
    next: u64,
    bad: Bad,
    /// Every sealed message it passed, both ways (to check it could not read them).
    seen: Vec<Vec<u8>>,
    /// Every sealed message from a phone to the PC (to replay one later).
    to_pc: Vec<(String, String)>,
    /// Relay messages refused, by code.
    refused: Vec<String>,
    /// Every message a PC sent, as it arrived (to check what Plenipo sends, ADR-143 §10).
    pc_said: Vec<String>,
}

/// A stand-in for 8 West's relay on 127.0.0.1.
#[derive(Clone)]
pub struct Relay {
    hub: Arc<Mutex<Hub>>,
    pub address: SocketAddr,
    clock: fn() -> i64,
}

fn send(out: &UnboundedSender<Message>, text: String) {
    let _ = out.send(Message::text(text));
}

impl Relay {
    /// Start one on a free port.
    pub async fn start() -> Self {
        Self::start_on("127.0.0.1:0", plenipo_licensing::clock).await
    }

    /// Start one on `addr`, telling time by `clock` (Unix seconds).
    pub async fn start_on(addr: &str, clock: fn() -> i64) -> Self {
        let listener = TcpListener::bind(addr).await.expect("a free port");
        let address = listener.local_addr().expect("an address");
        let relay = Self {
            hub: Arc::new(Mutex::new(Hub::default())),
            address,
            clock,
        };
        let r = relay.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let r = r.clone();
                tokio::spawn(async move { r.serve(stream).await });
            }
        });
        relay
    }

    /// The address a PC connects to.
    pub fn pc_address(&self) -> String {
        format!(
            "http://{}{}",
            self.address,
            plenipo_guard::outbound::RELAY_PATH
        )
    }

    /// The address a phone connects to.
    pub fn phone_address(&self) -> String {
        format!("ws://{}/plenipo/v1/phone", self.address)
    }

    pub fn set_bad(&self, bad: Bad) {
        lock(&self.hub).bad = bad;
    }

    /// Every sealed message passed so far.
    pub fn seen(&self) -> Vec<Vec<u8>> {
        lock(&self.hub).seen.clone()
    }

    /// The relay's refusals so far, by code.
    pub fn refused(&self) -> Vec<String> {
        lock(&self.hub).refused.clone()
    }

    /// Every message a PC sent the relay, as it arrived.
    pub fn pc_said(&self) -> Vec<String> {
        lock(&self.hub).pc_said.clone()
    }

    /// Is a PC connected?
    pub fn pc_connected(&self) -> bool {
        !lock(&self.hub).pcs.is_empty()
    }

    /// Send the PC, as if from phone connection `conn`, a sealed message it already passed (a
    /// replay through the relay).
    pub fn replay_last_to_pc(&self) -> bool {
        let hub = lock(&self.hub);
        let Some((conn, data)) = hub.to_pc.last().cloned() else {
            return false;
        };
        let Some(phone) = hub.phones.get(&conn) else {
            return false;
        };
        let Some(pc) = hub.pcs.get(&phone.pc) else {
            return false;
        };
        send(&pc.out, wire::write(&RelayToPc::Data { conn, data }));
        true
    }

    /// Make up a sealed message to the PC from the newest phone connection.
    pub fn invent_to_pc(&self) -> bool {
        let hub = lock(&self.hub);
        let Some((conn, phone)) = hub.phones.iter().max_by_key(|(k, _)| (*k).clone()) else {
            return false;
        };
        let Some(pc) = hub.pcs.get(&phone.pc) else {
            return false;
        };
        let made_up = b64::encode(&crate::random::<64>());
        send(
            &pc.out,
            wire::write(&RelayToPc::Data {
                conn: conn.clone(),
                data: made_up,
            }),
        );
        true
    }

    // The WebSocket library's own callback type returns its own (large) error.
    #[allow(clippy::result_large_err)]
    async fn serve(self, stream: TcpStream) {
        let mut path = String::new();
        let callback = |req: &Request, resp: Response| {
            path = req.uri().path().to_owned();
            Ok(resp)
        };
        let Ok(ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await else {
            return;
        };
        let (mut sink, mut stream) = ws.split();
        let (out, mut outgoing) = unbounded_channel::<Message>();
        tokio::spawn(async move {
            while let Some(m) = outgoing.recv().await {
                let close = matches!(m, Message::Close(_));
                if sink.send(m).await.is_err() || close {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        match path.as_str() {
            "/plenipo/v1/pc" => self.serve_pc(out, &mut stream).await,
            "/plenipo/v1/phone" => self.serve_phone(out, &mut stream).await,
            _ => {
                let _ = out.send(Message::Close(None));
            }
        }
    }

    async fn serve_pc<S>(&self, out: UnboundedSender<Message>, stream: &mut S)
    where
        S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + Unpin,
    {
        let nonce = b64::encode(&crate::random::<32>());
        send(
            &out,
            wire::write(&RelayToPc::Challenge {
                nonce: nonce.clone(),
            }),
        );
        let Some(Ok(Message::Text(text))) = stream.next().await else {
            return;
        };
        lock(&self.hub).pc_said.push(text.to_string());
        let refuse = |code: &str| {
            lock(&self.hub).refused.push(code.to_owned());
            send(
                &out,
                wire::write(&RelayToPc::Refused {
                    code: code.to_owned(),
                }),
            );
            let _ = out.send(Message::Close(None));
        };
        let Some(PcToRelay::Hello {
            v: 1,
            key,
            proof,
            answer,
        }) = wire::read(text.as_str())
        else {
            refuse(codes::BAD_HELLO);
            return;
        };
        let (Some(key), Some(proof)) = (
            b64::decode_exact::<32>(&key),
            b64::decode_exact::<64>(&proof),
        ) else {
            refuse(codes::BAD_HELLO);
            return;
        };
        let proven = ed25519_dalek::VerifyingKey::from_bytes(&key).is_ok_and(|k| {
            k.verify(
                format!("{}{nonce}", wire::PC_PROOF_CONTEXT).as_bytes(),
                &ed25519_dalek::Signature::from_bytes(&proof),
            )
            .is_ok()
        });
        if !proven {
            refuse(codes::BAD_PROOF);
            return;
        }
        let now = (self.clock)();
        let pro = plenipo_licensing::answer::verify(&answer).is_ok_and(|a| {
            use plenipo_licensing::SubscriptionState as S;
            let fresh = now.saturating_sub(a.as_of) < 30 * 86_400;
            fresh
                && match a.state {
                    S::Active => true,
                    S::Cancelled => a.ends_at.is_some_and(|e| e > now),
                    S::Ended | S::Unknown => false,
                }
        });
        if !pro {
            refuse(codes::NOT_PRO);
            return;
        }
        let pc = crate::keys::fingerprint(&key);
        {
            let mut hub = lock(&self.hub);
            if let Some(old) = hub.pcs.remove(&pc) {
                let _ = old.out.send(Message::Close(None));
            }
            hub.pcs.insert(
                pc.clone(),
                PcLink {
                    out: out.clone(),
                    relay_public: key,
                    mailbox: None,
                    mailbox_tries: 0,
                    dropped: HashMap::new(),
                },
            );
        }
        send(&out, wire::write(&RelayToPc::Welcome { pc: pc.clone() }));
        while let Some(Ok(message)) = stream.next().await {
            let Message::Text(text) = message else {
                if matches!(message, Message::Close(_)) {
                    break;
                }
                continue;
            };
            lock(&self.hub).pc_said.push(text.to_string());
            let Some(m) = wire::read::<PcToRelay>(text.as_str()) else {
                continue;
            };
            let mut hub = lock(&self.hub);
            match m {
                PcToRelay::Mailbox { mailbox } => {
                    if let Some(p) = hub.pcs.get_mut(&pc) {
                        p.mailbox = Some(mailbox);
                        p.mailbox_tries = 0;
                    }
                }
                PcToRelay::CloseMailbox => {
                    if let Some(p) = hub.pcs.get_mut(&pc) {
                        p.mailbox = None;
                    }
                }
                PcToRelay::Drop { phone, until } => {
                    if let Some(p) = hub.pcs.get_mut(&pc) {
                        p.dropped.insert(phone.clone(), until);
                    }
                    let gone: Vec<String> = hub
                        .phones
                        .iter()
                        .filter(|(_, l)| l.pc == pc && l.phone.as_deref() == Some(&phone))
                        .map(|(k, _)| k.clone())
                        .collect();
                    // Each is told its pass is no longer good, then closed.
                    for conn in gone {
                        if let Some(l) = hub.phones.remove(&conn) {
                            send(
                                &l.out,
                                wire::write(&RelayToPhone::Refused {
                                    code: codes::BAD_PASS.into(),
                                }),
                            );
                            let _ = l.out.send(Message::Close(None));
                            hub.refused.push(codes::BAD_PASS.into());
                        }
                    }
                }
                PcToRelay::Send { conn, data } => {
                    let ok = hub.phones.get(&conn).is_some_and(|l| l.pc == pc);
                    if !ok {
                        send(
                            &out,
                            wire::write(&RelayToPc::Error {
                                code: codes::UNKNOWN_CONN.into(),
                            }),
                        );
                        continue;
                    }
                    if let Some(bytes) = b64::decode(&data, wire::MAX_DATA) {
                        hub.seen.push(bytes);
                    }
                    if let Some(l) = hub.phones.get(&conn) {
                        send(&l.out, wire::write(&RelayToPhone::Data { data }));
                    }
                }
                PcToRelay::Close { conn } => {
                    if hub.phones.get(&conn).is_some_and(|l| l.pc == pc) {
                        if let Some(l) = hub.phones.remove(&conn) {
                            let _ = l.out.send(Message::Close(None));
                        }
                    }
                }
                PcToRelay::Hello { .. } => {}
            }
        }
        // The PC went away: its phones are told.
        let mut hub = lock(&self.hub);
        if hub.pcs.get(&pc).is_some_and(|p| p.out.same_channel(&out)) {
            hub.pcs.remove(&pc);
            let gone: Vec<String> = hub
                .phones
                .iter()
                .filter(|(_, l)| l.pc == pc)
                .map(|(k, _)| k.clone())
                .collect();
            for conn in gone {
                if let Some(l) = hub.phones.remove(&conn) {
                    send(&l.out, wire::write(&RelayToPhone::PcOffline));
                    let _ = l.out.send(Message::Close(None));
                }
            }
        }
    }

    async fn serve_phone<S>(&self, out: UnboundedSender<Message>, stream: &mut S)
    where
        S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + Unpin,
    {
        let Some(Ok(Message::Text(text))) = stream.next().await else {
            return;
        };
        let refuse = |code: &str| {
            lock(&self.hub).refused.push(code.to_owned());
            send(
                &out,
                wire::write(&RelayToPhone::Refused {
                    code: code.to_owned(),
                }),
            );
            let _ = out.send(Message::Close(None));
        };
        let now = (self.clock)();
        let (pc, phone) = match wire::read::<PhoneToRelay>(text.as_str()) {
            Some(PhoneToRelay::Pass { pass }) => {
                let Some(pc) = crate::pass::pc_of(&pass) else {
                    refuse(codes::BAD_PASS);
                    return;
                };
                let hub = lock(&self.hub);
                let Some(link) = hub.pcs.get(&pc) else {
                    drop(hub);
                    refuse(codes::PC_OFFLINE);
                    return;
                };
                match crate::pass::check(&pass, &link.relay_public, now) {
                    Ok(p) if link.dropped.get(&p.phone).is_none_or(|&until| until <= now) => {
                        (pc, Some(p.phone))
                    }
                    _ => {
                        drop(hub);
                        refuse(codes::BAD_PASS);
                        return;
                    }
                }
            }
            Some(PhoneToRelay::Mailbox { mailbox }) => {
                let mut hub = lock(&self.hub);
                let found = hub
                    .pcs
                    .iter_mut()
                    .find(|(_, l)| l.mailbox.as_deref() == Some(&mailbox));
                match found {
                    Some((pc, link)) if link.mailbox_tries < 3 => {
                        link.mailbox_tries += 1;
                        (pc.clone(), None)
                    }
                    Some(_) => {
                        drop(hub);
                        refuse(codes::TOO_MANY_TRIES);
                        return;
                    }
                    None => {
                        drop(hub);
                        refuse(codes::MAILBOX_CLOSED);
                        return;
                    }
                }
            }
            _ => {
                refuse(codes::BAD_PASS);
                return;
            }
        };
        let conn = {
            let mut hub = lock(&self.hub);
            hub.next += 1;
            let conn = format!("c{:06}", hub.next);
            hub.phones.insert(
                conn.clone(),
                PhoneLink {
                    out: out.clone(),
                    pc: pc.clone(),
                    phone: phone.clone(),
                },
            );
            if let Some(link) = hub.pcs.get(&pc) {
                let mailbox = phone.is_none();
                send(
                    &link.out,
                    wire::write(&RelayToPc::Joined {
                        conn: conn.clone(),
                        phone,
                        mailbox,
                    }),
                );
            }
            conn
        };
        send(&out, wire::write(&RelayToPhone::Ready));
        while let Some(Ok(message)) = stream.next().await {
            let Message::Text(text) = message else {
                if matches!(message, Message::Close(_)) {
                    break;
                }
                continue;
            };
            let Some(PhoneToRelay::Data { data }) = wire::read(text.as_str()) else {
                continue;
            };
            let mut hub = lock(&self.hub);
            let Some(bytes) = b64::decode(&data, wire::MAX_DATA) else {
                send(
                    &out,
                    wire::write(&RelayToPhone::Refused {
                        code: codes::TOO_BIG.into(),
                    }),
                );
                continue;
            };
            hub.seen.push(bytes.clone());
            hub.to_pc.push((conn.clone(), data.clone()));
            let bad = hub.bad;
            let Some(link) = hub.pcs.get(&pc) else {
                drop(hub);
                send(&out, wire::write(&RelayToPhone::PcOffline));
                let _ = out.send(Message::Close(None));
                return;
            };
            let pass = |data: String| {
                send(
                    &link.out,
                    wire::write(&RelayToPc::Data {
                        conn: conn.clone(),
                        data,
                    }),
                );
            };
            match bad {
                Bad::Honest => pass(data),
                Bad::Change => {
                    let mut changed = bytes;
                    let last = changed.len() - 1;
                    changed[last] ^= 0x01;
                    pass(b64::encode(&changed));
                }
                Bad::Replay => {
                    pass(data.clone());
                    pass(data);
                }
                Bad::Drop => {}
                Bad::Invent => {
                    pass(data);
                    pass(b64::encode(&crate::random::<48>()));
                }
            }
        }
        let mut hub = lock(&self.hub);
        if hub.phones.remove(&conn).is_some() {
            if let Some(link) = hub.pcs.get(&pc) {
                send(&link.out, wire::write(&RelayToPc::Left { conn }));
            }
        }
    }
}

// ---- A phone ------------------------------------------------------------------------------------

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

/// What a phone keeps after pairing.
#[derive(Debug, Clone)]
pub struct Paired {
    pub device: String,
    pub phone: String,
    pub pass: String,
    pub pc: String,
    pub pc_key: [u8; 32],
}

/// A stand-in phone, through the relay.
pub struct NetPhone {
    pub relay: String,
    pub noise: ([u8; 32], [u8; 32]),
    pub passkey: Authenticator,
    pub paired: Option<Paired>,
    socket: Option<Socket>,
    line: Option<TransportState>,
    assembler: Assembler,
    /// What the PC said when the meeting started.
    pub welcome: Option<MeetingWelcome>,
    /// Events the PC sent while the phone waited for a reply.
    pub events: Vec<Event>,
}

/// What went wrong for the stand-in phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhoneError {
    /// The relay refused, by code.
    Relay(String),
    /// The connection closed.
    Closed,
    /// The sealed meeting failed.
    Meeting,
    /// No answer in time.
    Timeout,
    Other(String),
}

impl NetPhone {
    pub fn new(relay: &str) -> Self {
        Self {
            relay: relay.to_owned(),
            noise: noise::new_keypair(),
            passkey: Authenticator::default(),
            paired: None,
            socket: None,
            line: None,
            assembler: Assembler::default(),
            welcome: None,
            events: Vec::new(),
        }
    }

    async fn open(&mut self, first: &PhoneToRelay) -> Result<(), PhoneError> {
        let (mut ws, _) = tokio_tungstenite::connect_async(&self.relay)
            .await
            .map_err(|e| PhoneError::Other(e.to_string()))?;
        ws.send(Message::text(wire::write(first)))
            .await
            .map_err(|_| PhoneError::Closed)?;
        self.socket = Some(ws);
        match self.relay_message().await? {
            RelayToPhone::Ready => Ok(()),
            RelayToPhone::Refused { code } => Err(PhoneError::Relay(code)),
            RelayToPhone::PcOffline => Err(PhoneError::Relay(codes::PC_OFFLINE.into())),
            RelayToPhone::Data { .. } => Err(PhoneError::Other("data before ready".into())),
        }
    }

    async fn relay_message(&mut self) -> Result<RelayToPhone, PhoneError> {
        let ws = self.socket.as_mut().ok_or(PhoneError::Closed)?;
        loop {
            let next = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
                .await
                .map_err(|_| PhoneError::Timeout)?;
            match next {
                Some(Ok(Message::Text(t))) => {
                    return wire::read(t.as_str())
                        .ok_or_else(|| PhoneError::Other("not a relay message".into()))
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                    return Err(PhoneError::Closed)
                }
                Some(Ok(_)) => {}
            }
        }
    }

    async fn send_raw(&mut self, data: Vec<u8>) -> Result<(), PhoneError> {
        let ws = self.socket.as_mut().ok_or(PhoneError::Closed)?;
        ws.send(Message::text(wire::write(&PhoneToRelay::Data {
            data: b64::encode(&data),
        })))
        .await
        .map_err(|_| PhoneError::Closed)
    }

    async fn data(&mut self) -> Result<Vec<u8>, PhoneError> {
        match self.relay_message().await? {
            RelayToPhone::Data { data } => b64::decode(&data, wire::MAX_DATA)
                .ok_or_else(|| PhoneError::Other("bad data".into())),
            RelayToPhone::PcOffline => Err(PhoneError::Relay(codes::PC_OFFLINE.into())),
            RelayToPhone::Refused { code } => Err(PhoneError::Relay(code)),
            RelayToPhone::Ready => Err(PhoneError::Other("ready twice".into())),
        }
    }

    /// Send a whole message on the open line.
    pub async fn say(&mut self, message: &PhoneSays) -> Result<(), PhoneError> {
        self.say_bytes(serde_json::to_vec(message).expect("JSON"))
            .await
    }

    /// Send anything, sealed on the open line (a page that asks for what is not on the list).
    pub async fn say_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PhoneError> {
        let line = self.line.as_mut().ok_or(PhoneError::Closed)?;
        let pieces = noise::seal(line, &bytes).map_err(|_| PhoneError::Meeting)?;
        for p in pieces {
            self.send_raw(p).await?;
        }
        Ok(())
    }

    /// The next whole message from the PC on the open line.
    pub async fn hear(&mut self) -> Result<PcSays, PhoneError> {
        loop {
            let data = self.data().await?;
            let line = self.line.as_mut().ok_or(PhoneError::Closed)?;
            let (more, piece) = noise::open(line, &data).map_err(|_| PhoneError::Meeting)?;
            if let Some(whole) = self
                .assembler
                .add(more, piece)
                .map_err(|_| PhoneError::Meeting)?
            {
                return serde_json::from_slice(&whole)
                    .map_err(|e| PhoneError::Other(e.to_string()));
            }
        }
    }

    /// Pairing, up to the PC's "Is this your phone?" (the test answers on the PC, then calls
    /// [`Self::finish_pairing`]).
    pub async fn start_pairing(&mut self, code: &Code, name: &str) -> Result<(), PhoneError> {
        self.start_pairing_with_psk(code, code.psk(), name).await
    }

    /// Pairing at `code`'s mailbox, but with `psk` as the shared key (a wrong code that found
    /// the right mailbox: what a relay guessing codes would do).
    pub async fn start_pairing_with_psk(
        &mut self,
        code: &Code,
        psk: [u8; 32],
        name: &str,
    ) -> Result<(), PhoneError> {
        self.open(&PhoneToRelay::Mailbox {
            mailbox: code.mailbox(),
        })
        .await?;
        let mut hs = noise::pairing_phone(&self.noise.0, &psk).map_err(|_| PhoneError::Meeting)?;
        let m1 = noise::write(&mut hs, b"").map_err(|_| PhoneError::Meeting)?;
        self.send_raw(m1).await?;
        let m2 = self.data().await?;
        noise::read(&mut hs, &m2).map_err(|_| PhoneError::Meeting)?;
        let hello = PairHello {
            name: name.to_owned(),
            browser: "Stand-in browser".into(),
        };
        let m3 = noise::write(&mut hs, &serde_json::to_vec(&hello).expect("JSON"))
            .map_err(|_| PhoneError::Meeting)?;
        let pc_key: [u8; 32] = hs
            .get_remote_static()
            .and_then(|k| k.try_into().ok())
            .ok_or(PhoneError::Meeting)?;
        self.line = Some(hs.into_transport_mode().map_err(|_| PhoneError::Meeting)?);
        self.send_raw(m3).await?;
        match self.hear().await? {
            PcSays::Pair {
                pair: PairStep::Waiting,
            } => {}
            other => return Err(PhoneError::Other(format!("{other:?}"))),
        }
        self.paired = Some(Paired {
            device: String::new(),
            phone: String::new(),
            pass: String::new(),
            pc: String::new(),
            pc_key,
        });
        Ok(())
    }

    /// After the PC's owner said yes (or no): make the passkey and finish.
    pub async fn finish_pairing(&mut self) -> Result<PairStep, PhoneError> {
        let step = match self.hear().await? {
            PcSays::Pair { pair } => pair,
            other => return Err(PhoneError::Other(format!("{other:?}"))),
        };
        let PairStep::Accepted {
            device,
            phone,
            pass,
            pc,
            passkey,
            ..
        } = step.clone()
        else {
            return Ok(step);
        };
        let challenge = b64::decode(&passkey.challenge, 64).ok_or(PhoneError::Meeting)?;
        let made = self.passkey.create(&challenge);
        self.say(&PhoneSays::Passkey { passkey: made }).await?;
        let done = match self.hear().await? {
            PcSays::Pair { pair } => pair,
            other => return Err(PhoneError::Other(format!("{other:?}"))),
        };
        if done == PairStep::Done {
            let p = self.paired.as_mut().ok_or(PhoneError::Meeting)?;
            p.device = device;
            p.phone = phone;
            p.pass = pass;
            p.pc = pc;
        }
        self.close().await;
        Ok(done)
    }

    /// An everyday meeting. `notice`: sent by a notice's button.
    pub async fn meet(&mut self, notice: bool) -> Result<MeetingWelcome, PhoneError> {
        let paired = self.paired.clone().ok_or(PhoneError::Meeting)?;
        // What an earlier meeting said is over.
        self.events.clear();
        self.open(&PhoneToRelay::Pass {
            pass: paired.pass.clone(),
        })
        .await?;
        let prologue = noise::everyday_prologue(&paired.pc, &paired.phone);
        let mut hs = noise::everyday_phone(&self.noise.0, &paired.pc_key, &prologue)
            .map_err(|_| PhoneError::Meeting)?;
        let hello = MeetingHello {
            notice,
            page: "stand-in".into(),
        };
        let m1 = noise::write(&mut hs, &serde_json::to_vec(&hello).expect("JSON"))
            .map_err(|_| PhoneError::Meeting)?;
        self.send_raw(m1).await?;
        let m2 = self.data().await?;
        let payload = noise::read(&mut hs, &m2).map_err(|_| PhoneError::Meeting)?;
        let welcome: MeetingWelcome =
            serde_json::from_slice(&payload).map_err(|e| PhoneError::Other(e.to_string()))?;
        self.line = Some(hs.into_transport_mode().map_err(|_| PhoneError::Meeting)?);
        self.assembler = Assembler::default();
        self.welcome = Some(welcome.clone());
        Ok(welcome)
    }

    /// Ask, and wait for the reply to this request (events that come first are kept).
    pub async fn ask_with(&mut self, id: &str, again: bool, ask: Ask) -> Result<Reply, PhoneError> {
        self.say(&PhoneSays::Ask {
            id: id.to_owned(),
            again,
            ask,
        })
        .await?;
        loop {
            match self.hear().await? {
                PcSays::Reply { reply } if reply.re == id => return Ok(reply),
                PcSays::Event { event } => self.events.push(event),
                _ => {}
            }
        }
    }

    pub async fn ask(&mut self, ask: Ask) -> Result<Reply, PhoneError> {
        let id = b64::encode(&crate::random::<16>());
        self.ask_with(&id, false, ask).await
    }

    /// Sign in with the passkey, to the meeting's challenge.
    pub async fn sign_in(&mut self) -> Result<Reply, PhoneError> {
        let challenge = self
            .welcome
            .as_ref()
            .and_then(|w| w.challenge.clone())
            .and_then(|c| b64::decode(&c, 64))
            .ok_or_else(|| PhoneError::Other("no challenge".into()))?;
        let answer = self.passkey.get(&challenge);
        let reply = self.ask(Ask::SignIn { answer }).await?;
        if let Some(ok) = &reply.ok {
            if let Some(pass) = ok.get("pass").and_then(|p| p.as_str()) {
                if let Some(p) = self.paired.as_mut() {
                    p.pass = pass.to_owned();
                }
            }
        }
        Ok(reply)
    }

    /// Wait for an event (or take one already heard).
    pub async fn event(&mut self) -> Result<Event, PhoneError> {
        if !self.events.is_empty() {
            return Ok(self.events.remove(0));
        }
        loop {
            if let PcSays::Event { event } = self.hear().await? {
                return Ok(event);
            }
        }
    }

    /// Send raw bytes as if sealed (for the tests' made-up messages).
    pub async fn send_made_up(&mut self, data: Vec<u8>) -> Result<(), PhoneError> {
        self.send_raw(data).await
    }

    pub async fn close(&mut self) {
        if let Some(mut ws) = self.socket.take() {
            let _ = ws.close(None).await;
        }
        self.line = None;
    }
}
