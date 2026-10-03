//! The PC's side of phone access (ADR-141 to ADR-145): pairing, meetings, sign-in, and every
//! request a phone makes.
//!
//! [`Remote`] does not touch the network itself. The relay link ([`crate::link`]) gives it what
//! the relay says ([`Remote::from_relay`]) and sends what it asks ([`ToRelay`]). It does not carry
//! requests out itself either: Guard decides each one (`plenipo_guard::remote::decide`), and the
//! app's own services carry it out through [`Host`], on a thread that may wait.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use plenipo_guard::remote::{self as guard_remote, ApprovalFacts, KeptOnPc, PhoneState, Why};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snow::{HandshakeState, TransportState};
use tokio::sync::mpsc::UnboundedSender;
use ts_rs::TS;

use crate::code::Code;
use crate::devices::{clean_name, Config, ConfigFile, Device, DeviceView, Kept, KeyStore};
use crate::keys::PcKeys;
use crate::limits::{
    Checks, DeadCodes, Meetings, CODE_TRIES, CONNS_PER_DEVICE, MAX_CONNS, MEETING_DEADLINE_MS,
};
use crate::noise::{self, Assembler};
use crate::protocol::{
    Ask, Changed, Event, MeetingHello, MeetingWelcome, PairHello, PairStep, PasskeyRequest, PcSays,
    PhoneNotice, PhoneSays, Reply, SignedIn, SignedOutWhy,
};
use crate::webauthn::{self, Expect};
use crate::webpush::{self, Delivery, NoticeError, SealedNotice};
use crate::wire::{PcToRelay, RelayToPc};
use crate::{b64, RemoteError, Result};

/// How the service tells time (the tests move it by hand).
pub trait Clock: Send + Sync + 'static {
    /// Unix milliseconds.
    fn now_ms(&self) -> u64;
}

/// The PC's clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}

/// What the app gives phone access.
pub trait Host: Send + Sync + 'static {
    /// Phone access is Pro now (`Entitlements::check(Limit::PhoneAccess)`).
    fn pro(&self) -> bool;
    /// What a waiting approval is (`None`: no approval of that ID waits in that organization).
    fn approval(&self, org: &str, approval: &str) -> Option<ApprovalFacts>;
    /// Carry out a request Guard allowed, with the same core function the main window uses (on
    /// a thread that may wait). The answer, or why it did not work, in plain words.
    fn carry_out(&self, phone: &Phone, ask: &Ask) -> std::result::Result<Value, String>;
    /// Record an event in the Ledger: `org`'s, or the first organization's.
    fn record(&self, org: Option<&str>, event: &str, payload: Value);
    /// Something Settings → Devices shows changed.
    fn changed(&self, what: Change);
    /// Whether Guard lets notices go to `endpoint` (part 14C): a phone's own notice service, or
    /// the tests' stand-in in a copy built for the tests. Why not, in plain words.
    fn notice_address(&self, endpoint: &str) -> std::result::Result<(), String> {
        plenipo_guard::OutboundRules::default()
            .check(plenipo_guard::outbound::Purpose::PhoneNotices, endpoint)
            .map(|_| ())
    }
}

/// What changed, for Settings → Devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The list of phones (added, renamed, removed, paused, signed in or out).
    Devices,
    /// Adding a phone (a code, "Is this your phone?", done, or not).
    Pairing,
    /// The switch, or the relay's connection.
    Switch,
    /// Someone keeps trying to reach the PC as a phone (ADR-143 §8).
    Attempts,
}

/// The phone a request came from, as the record names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    pub id: String,
    pub name: String,
}

/// What a notice service said about one notice (part 14C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivered {
    /// It took the notice.
    Sent,
    /// The phone's notice address is gone: the phone turned notices off, or its browser forgot.
    Gone,
    /// It did not take it, in plain words.
    Failed(String),
}

/// A notice's topic: 32 letters from base64url, the same for each notice about one thing.
fn topic(tag: &str) -> String {
    use sha2::Digest as _;
    b64::encode(&sha2::Sha256::digest(tag.as_bytes()))[..32].to_owned()
}

/// What the service is built with.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The phone's page (`https://remote.getplenipo.com`, or the tests' stand-in).
    pub origin: String,
    /// The passkeys' site (`remote.getplenipo.com`).
    pub rp_id: String,
    /// What the phone calls this PC.
    pub pc_name: String,
    /// Plenipo's version.
    pub version: String,
}

/// What the service asks the relay link to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToRelay {
    Send { conn: String, data: Vec<u8> },
    Close { conn: String },
    Mailbox(String),
    CloseMailbox,
    Drop { phone: String, until: i64 },
}

impl ToRelay {
    /// The relay's own message.
    pub fn message(&self) -> PcToRelay {
        match self {
            Self::Send { conn, data } => PcToRelay::Send {
                conn: conn.clone(),
                data: b64::encode(data),
            },
            Self::Close { conn } => PcToRelay::Close { conn: conn.clone() },
            Self::Mailbox(m) => PcToRelay::Mailbox { mailbox: m.clone() },
            Self::CloseMailbox => PcToRelay::CloseMailbox,
            Self::Drop { phone, until } => PcToRelay::Drop {
                phone: phone.clone(),
                until: *until,
            },
        }
    }
}

/// Adding a phone, as Settings → Devices shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, rename = "PairingView")]
pub enum PairingView {
    /// The code is shown: scan it or type it on the phone.
    Showing {
        /// "7K3Q-M9TX-2HFD-R8WB".
        code: String,
        /// The pairing address, as the picture code holds it.
        link: String,
        /// The picture code (QR code).
        qr: crate::qr::Qr,
        #[ts(type = "number")]
        ends_at: u64,
        /// Wrong tries so far (it dies at 3).
        wrong: u32,
    },
    /// "Is this your phone?"
    Asking {
        name: String,
        browser: String,
        #[ts(type = "number")]
        since: u64,
        /// Six digits the phone shows too, from the meeting itself (ADR-212): the owner compares
        /// them, because a name alone proves nothing.
        check: String,
    },
    /// The owner said yes: the phone is making its passkey.
    MakingPasskey { name: String },
}

/// Phone access, as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteView {
    /// Settings → Switches → **Use Plenipo from another device**.
    pub switched_on: bool,
    /// Connected to 8 West's relay now.
    pub connected: bool,
    pub devices: Vec<DeviceView>,
    #[ts(optional)]
    pub pairing: Option<PairingView>,
    /// **Add a phone** is paused after wrong codes, until then (Unix milliseconds).
    #[ts(optional, type = "number")]
    pub pairing_paused_until: Option<u64>,
    pub kept: KeptOnPc,
    /// The PC stopped answering new meetings after failed ones, until then.
    #[ts(optional, type = "number")]
    pub meetings_stopped_until: Option<u64>,
    /// Why the relay did not connect, in plain words.
    #[ts(optional)]
    pub relay_problem: Option<String>,
    /// Settings → Notifications → **Notices on my phones** (part 14C).
    pub phone_notices: bool,
}

/// Settings → Devices, as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteSettings {
    pub remote: RemoteView,
    /// Phone access is part of Pro: is this PC on Pro?
    pub pro: bool,
    /// The relay is not live for this copy yet: the switch says "Coming soon".
    pub coming_soon: bool,
    /// The phone's page.
    pub page: String,
    /// What the phone calls this PC.
    pub pc_name: String,
    /// The kinds of sensitive action, with their plain names, for **Keep these approvals on my PC
    /// only**.
    pub sensitive: Vec<SensitiveChoice>,
}

/// One kind of sensitive action, with its plain name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SensitiveChoice {
    pub kind: plenipo_guard::SensitiveKind,
    pub label: String,
}

impl SensitiveChoice {
    /// Every kind Guard knows, in its order.
    pub fn all() -> Vec<Self> {
        plenipo_guard::SensitiveKind::ALL
            .into_iter()
            .map(|kind| Self {
                kind,
                label: kind.label().to_owned(),
            })
            .collect()
    }
}

/// A meeting's lock: the meeting itself, then the open line.
enum Lock {
    Meeting(Box<HandshakeState>),
    Open(Box<TransportState>),
    /// Between the two, for a moment.
    Gone,
}

/// What a connection is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// A phone pairing through the mailbox.
    Mailbox,
    /// A paired phone, by its ID on the PC.
    Phone(String),
}

struct Conn {
    kind: Kind,
    lock: Lock,
    /// When the relay said it joined (Unix milliseconds): a meeting not finished within
    /// `MEETING_DEADLINE_MS` of this is closed.
    since: u64,
    /// Meeting messages read so far.
    read: u8,
    assembler: Assembler,
    /// The sign-in challenge given in this meeting, and when it ends.
    challenge: Option<([u8; 32], u64)>,
    /// Sent by a notice's button (ADR-142 §5).
    notice: bool,
}

/// One phone's sign-in and its recent requests (not kept: a restart signs every phone out).
#[derive(Default)]
struct Live {
    signed_in_at: Option<u64>,
    last_request: u64,
    /// Request IDs seen in this sign-in (a copy is refused).
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
    /// Each request's answer, for a phone whose connection dropped.
    outcomes: VecDeque<(String, Reply)>,
    checks: Checks,
}

const SEEN_KEPT: usize = 4096;
const OUTCOMES_KEPT: usize = 64;

/// What happens after the owner says yes.
enum Candidate {
    /// "Is this your phone?", with the six digits the phone shows too (ADR-212).
    AskingOwner { check: String },
    MakingPasskey {
        device: String,
        phone: String,
        challenge: [u8; 32],
    },
}

struct Pairing {
    code: Code,
    ends: u64,
    wrong: u32,
    /// The phone whose first meeting worked: its connection, what it said, and its key.
    candidate: Option<(String, PairHello, [u8; 32], Candidate, u64)>,
}

struct State {
    config: Config,
    keys: Option<PcKeys>,
    devices: Vec<Device>,
    live: HashMap<String, Live>,
    conns: HashMap<String, Conn>,
    pairing: Option<Pairing>,
    meetings: Meetings,
    dead_codes: DeadCodes,
    relay_problem: Option<String>,
}

/// The PC's side of phone access.
pub struct Remote {
    settings: Settings,
    store: Arc<dyn KeyStore>,
    config_file: Arc<dyn ConfigFile>,
    host: Arc<dyn Host>,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
    out: Mutex<Option<UnboundedSender<ToRelay>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn secs(ms: u64) -> i64 {
    i64::try_from(ms / 1000).unwrap_or(i64::MAX)
}

impl Remote {
    /// The service, with what was kept: the switch from `config_file`, the phones from `store`.
    pub fn new(
        settings: Settings,
        store: Arc<dyn KeyStore>,
        config_file: Arc<dyn ConfigFile>,
        host: Arc<dyn Host>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        let config = Config::read(config_file.as_ref());
        let kept = Kept {
            store: store.as_ref(),
        };
        let devices = kept.devices().unwrap_or_else(|e| {
            log::warn!("the phones could not be read from the Vault: {e}");
            Vec::new()
        });
        Arc::new(Self {
            settings,
            store,
            config_file,
            host,
            clock,
            state: Mutex::new(State {
                config,
                keys: None,
                devices,
                live: HashMap::new(),
                conns: HashMap::new(),
                pairing: None,
                meetings: Meetings::default(),
                dead_codes: DeadCodes::default(),
                relay_problem: None,
            }),
            out: Mutex::new(None),
        })
    }

    fn now(&self) -> u64 {
        self.clock.now_ms()
    }

    fn kept(&self) -> Kept<'_> {
        Kept {
            store: self.store.as_ref(),
        }
    }

    fn expect<'a>(&'a self, challenge: &'a [u8]) -> Expect<'a> {
        Expect {
            origin: &self.settings.origin,
            rp_id: &self.settings.rp_id,
            challenge,
        }
    }

    // ---- What Settings shows, and the owner's actions on the PC ---------------------------

    /// Phone access, as Settings shows it.
    pub fn view(&self) -> RemoteView {
        let now = self.now();
        let st = lock(&self.state);
        RemoteView {
            switched_on: st.config.switched_on,
            connected: lock(&self.out).is_some(),
            devices: st
                .devices
                .iter()
                .map(|d| DeviceView {
                    id: d.id.clone(),
                    name: d.name.clone(),
                    browser: d.browser.clone(),
                    added_at: d.added_at,
                    last_seen_at: d.last_seen_at,
                    paused: d.paused,
                    signed_in: st.live.get(&d.id).is_some_and(|l| signed_in(l, now)),
                    notices: d.notices.is_some(),
                })
                .collect(),
            pairing: st.pairing.as_ref().map(|p| self.pairing_view(p)),
            pairing_paused_until: st.dead_codes.paused_until(now),
            kept: st.config.kept.clone(),
            meetings_stopped_until: (!st.meetings.answering(now))
                .then(|| st.meetings.stopped_until()),
            relay_problem: st.relay_problem.clone(),
            phone_notices: !st.config.phone_notices_off,
        }
    }

    fn pairing_view(&self, p: &Pairing) -> PairingView {
        match &p.candidate {
            None => {
                let link = p.code.link(&self.settings.origin);
                PairingView::Showing {
                    code: p.code.shown(),
                    qr: crate::qr::Qr::of(&link),
                    link,
                    ends_at: p.ends,
                    wrong: p.wrong,
                }
            }
            Some((_, hello, _, Candidate::AskingOwner { check }, since)) => PairingView::Asking {
                name: hello.name.clone(),
                browser: hello.browser.clone(),
                since: *since,
                check: check.clone(),
            },
            Some((_, hello, _, Candidate::MakingPasskey { .. }, _)) => PairingView::MakingPasskey {
                name: hello.name.clone(),
            },
        }
    }

    /// Is the switch on?
    pub fn switched_on(&self) -> bool {
        lock(&self.state).config.switched_on
    }

    /// The PC's keys (made the first time).
    fn keys(&self, st: &mut State) -> Result<PcKeys> {
        if let Some(k) = &st.keys {
            return Ok(k.clone());
        }
        let keys = self.kept().keys()?;
        st.keys = Some(keys.clone());
        Ok(keys)
    }

    /// The PC's relay key's public half and its fingerprint (made the first time).
    pub fn relay_identity(&self) -> Result<PcKeys> {
        let mut st = lock(&self.state);
        self.keys(&mut st)
    }

    /// Turn phone access on or off (Settings → Switches). Off cuts every phone off at once
    /// (ADR-143 §7); the phones stay listed.
    pub fn set_switched_on(&self, on: bool) -> Result<()> {
        let mut st = lock(&self.state);
        if st.config.switched_on == on {
            return Ok(());
        }
        if on {
            self.keys(&mut st)?;
        }
        let mut config = st.config.clone();
        config.switched_on = on;
        config.write(self.config_file.as_ref())?;
        st.config = config;
        if !on {
            self.end_everything(&mut st, SignedOutWhy::SwitchedOff);
        }
        drop(st);
        self.host.record(
            None,
            if on {
                "remote.switched_on"
            } else {
                "remote.switched_off"
            },
            json!({}),
        );
        self.host.changed(Change::Switch);
        Ok(())
    }

    /// The approvals kept on the PC (Settings → Devices).
    /// Settings → Notifications → **Notices on my phones** (part 14C, ADR-144 §7).
    pub fn set_phone_notices(&self, on: bool) -> Result<()> {
        let mut st = lock(&self.state);
        let mut config = st.config.clone();
        config.phone_notices_off = !on;
        config.write(self.config_file.as_ref())?;
        st.config = config;
        drop(st);
        self.host
            .record(None, "remote.phone_notices_switched", json!({ "on": on }));
        self.host.changed(Change::Switch);
        Ok(())
    }

    /// `notice`, sealed for each phone that takes notices now (part 14C, ADR-144): the phone
    /// asked for them, it is not paused, phone access is on, the PC is on Pro, and notices on
    /// your phones are on. What the PC sends: one sealed notice to each phone's own notice
    /// service, signed with the PC's notice key.
    pub fn sealed_notices(
        &self,
        notice: &PhoneNotice,
    ) -> Vec<(Phone, std::result::Result<SealedNotice, NoticeError>)> {
        if !self.host.pro() {
            return Vec::new();
        }
        let now = self.now();
        let mut st = lock(&self.state);
        if !st.config.switched_on || st.config.phone_notices_off {
            return Vec::new();
        }
        let Ok(keys) = self.keys(&mut st) else {
            return Vec::new();
        };
        let key = keys.notice_key();
        let payload = serde_json::to_vec(notice).expect("a notice is JSON");
        // What waits for you comes at once, and is kept by the notice service only while it
        // can still be answered; the rest can wait an hour for a phone that is off.
        let urgent = matches!(notice.kind.as_str(), "approvals" | "checks");
        let delivery = Delivery {
            urgency: if urgent { "high" } else { "normal" },
            keep_for_secs: if urgent { 600 } else { 3600 },
            topic: Some(topic(&notice.tag)),
        };
        st.devices
            .iter()
            .filter(|d| !d.paused)
            .filter_map(|d| {
                let to = d.notices.clone()?;
                let phone = Phone {
                    id: d.id.clone(),
                    name: d.name.clone(),
                };
                Some((
                    phone,
                    webpush::seal(&to, &payload, &key, secs(now), &delivery),
                ))
            })
            .collect()
    }

    /// What a notice service said about a notice to `phone` (recorded with its kind, never what
    /// it said). A phone whose notice address is gone stops getting notices until it asks again.
    pub fn notice_delivered(&self, phone: &Phone, kind: &str, delivered: Delivered) {
        match delivered {
            Delivered::Sent => self.host.record(
                None,
                "remote.notice_sent",
                json!({ "device": phone.id, "name": phone.name, "kind": kind }),
            ),
            Delivered::Gone => {
                self.notices_gone(&phone.id);
                self.host.record(
                    None,
                    "remote.notice_failed",
                    json!({ "device": phone.id, "name": phone.name, "kind": kind, "why": "gone" }),
                );
            }
            Delivered::Failed(why) => self.host.record(
                None,
                "remote.notice_failed",
                json!({ "device": phone.id, "name": phone.name, "kind": kind, "why": why }),
            ),
        }
    }

    pub fn set_kept(&self, kept: KeptOnPc) -> Result<KeptOnPc> {
        let kept = kept.tidy();
        let mut st = lock(&self.state);
        let mut config = st.config.clone();
        config.kept = kept.clone();
        config.write(self.config_file.as_ref())?;
        st.config = config;
        drop(st);
        self.host.record(
            None,
            "remote.kept_on_pc_changed",
            serde_json::to_value(&kept).unwrap_or(Value::Null),
        );
        self.host.changed(Change::Devices);
        Ok(kept)
    }

    /// The approvals kept on the PC now.
    pub fn kept_on_pc(&self) -> KeptOnPc {
        lock(&self.state).config.kept.clone()
    }

    /// Every phone signs out and every connection closes (the switch off, Pro ended).
    fn end_everything(&self, st: &mut State, why: SignedOutWhy) {
        let now = self.now();
        let signed: Vec<String> = st
            .live
            .iter()
            .filter(|(_, l)| signed_in(l, now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &signed {
            self.end_sign_in(st, id, why);
        }
        let conns: Vec<String> = st.conns.keys().cloned().collect();
        for conn in conns {
            self.close(st, &conn);
        }
        if st.pairing.take().is_some() {
            self.send(ToRelay::CloseMailbox);
        }
    }

    /// Pro ended, or the relay link stopped for good: phone access pauses (ADR-143 §7).
    pub fn pause(&self, why: SignedOutWhy) {
        let mut st = lock(&self.state);
        self.end_everything(&mut st, why);
        drop(st);
        self.host.changed(Change::Devices);
    }

    /// **Add a phone**: a new code, shown on the PC (ADR-141).
    pub fn start_pairing(&self) -> Result<PairingView> {
        let now = self.now();
        let mut st = lock(&self.state);
        if !st.config.switched_on {
            return Err(RemoteError::Refused(
                "Turn on Use Plenipo from another device first (Settings → Switches).".into(),
            ));
        }
        if !self.host.pro() {
            return Err(RemoteError::Refused(plenipo_licensing::words::message(
                plenipo_licensing::Limit::PhoneAccess,
            )));
        }
        if let Some(until) = st.dead_codes.paused_until(now) {
            let minutes = (until - now).div_ceil(60_000);
            return Err(RemoteError::Refused(format!(
                "Someone tried wrong codes. Add a phone works again in {minutes} minute{}.",
                if minutes == 1 { "" } else { "s" }
            )));
        }
        if st.devices.len() >= crate::MAX_DEVICES {
            return Err(RemoteError::Refused(format!(
                "Your PC keeps up to {} phones. Remove one first.",
                crate::MAX_DEVICES
            )));
        }
        self.keys(&mut st)?;
        // A new code replaces any other, and every connection to the old mailbox goes with it
        // (the old candidate's, and any that joined and never finished its meeting): none of
        // them can pair on the new code, and each would otherwise hold one of the mailbox's
        // few slots. Phones are not touched.
        st.pairing = None;
        let old_mailbox: Vec<String> = st
            .conns
            .iter()
            .filter(|(_, c)| c.kind == Kind::Mailbox)
            .map(|(conn, _)| conn.clone())
            .collect();
        for conn in &old_mailbox {
            self.close(&mut st, conn);
        }
        let code = Code::new();
        let mailbox = code.mailbox();
        let pairing = Pairing {
            code,
            ends: now + crate::CODE_LIFE_MS,
            wrong: 0,
            candidate: None,
        };
        let view = self.pairing_view(&pairing);
        st.pairing = Some(pairing);
        drop(st);
        self.send(ToRelay::Mailbox(mailbox));
        self.host.changed(Change::Pairing);
        Ok(view)
    }

    /// Stop adding a phone.
    pub fn cancel_pairing(&self) {
        let mut st = lock(&self.state);
        let Some(p) = st.pairing.take() else {
            return;
        };
        if let Some((conn, ..)) = &p.candidate {
            self.send_pair(
                &mut st,
                conn,
                PairStep::Refused {
                    message: "Adding this phone was cancelled on your PC.".into(),
                },
            );
            self.close(&mut st, conn);
        }
        drop(st);
        self.send(ToRelay::CloseMailbox);
        self.host.record(
            None,
            "remote.pairing_refused",
            json!({ "reason": "cancelled" }),
        );
        self.host.changed(Change::Pairing);
    }

    /// The owner's answer to "Is this your phone?".
    pub fn answer_pairing(&self, add: bool) -> Result<()> {
        let now = self.now();
        let mut st = lock(&self.state);
        let keys = self.keys(&mut st)?;
        let Some(pairing) = st.pairing.as_mut() else {
            return Err(RemoteError::Invalid(
                "No phone is waiting to be added.".into(),
            ));
        };
        let Some((conn, hello, _, step, _)) = pairing.candidate.as_mut() else {
            return Err(RemoteError::Invalid(
                "No phone is waiting to be added.".into(),
            ));
        };
        if !matches!(step, Candidate::AskingOwner { .. }) {
            return Err(RemoteError::Invalid("That phone was already added.".into()));
        }
        let conn = conn.clone();
        let name = hello.name.clone();
        if !add {
            st.pairing = None;
            self.send_pair(
                &mut st,
                &conn,
                PairStep::Refused {
                    message: "Your PC said this is not your phone.".into(),
                },
            );
            self.close(&mut st, &conn);
            drop(st);
            self.host.record(
                None,
                "remote.pairing_refused",
                json!({ "reason": "not_added", "name": name }),
            );
            self.host.changed(Change::Pairing);
            return Ok(());
        }
        let device = b64::encode(&crate::random::<16>());
        let phone = b64::encode(&crate::random::<16>());
        let challenge: [u8; 32] = crate::random();
        let pass = crate::pass::issue(&keys, &phone, secs(now), crate::PASS_LIFE_SECS);
        *step = Candidate::MakingPasskey {
            device: device.clone(),
            phone: phone.clone(),
            challenge,
        };
        let accepted = PairStep::Accepted {
            device,
            phone,
            pass,
            pc: keys.fingerprint(),
            pc_name: self.settings.pc_name.clone(),
            passkey: PasskeyRequest {
                challenge: b64::encode(&challenge),
                rp_id: self.settings.rp_id.clone(),
                user: keys.user(),
                user_name: format!("Plenipo on {}", self.settings.pc_name),
            },
        };
        self.send_pair(&mut st, &conn, accepted);
        drop(st);
        self.host.changed(Change::Pairing);
        Ok(())
    }

    /// Rename a phone (Settings → Devices).
    pub fn rename(&self, id: &str, name: &str) -> Result<()> {
        let mut st = lock(&self.state);
        let ids = ids(&st.devices);
        let device =
            st.devices.iter_mut().find(|d| d.id == id).ok_or_else(|| {
                RemoteError::Invalid("That phone is not on your PC's list.".into())
            })?;
        let name = clean_name(name, &device.name);
        device.name.clone_from(&name);
        let copy = device.clone();
        self.kept().put(&copy, &ids)?;
        drop(st);
        self.host.record(
            None,
            "remote.device_renamed",
            json!({ "device": id, "name": name }),
        );
        self.host.changed(Change::Devices);
        Ok(())
    }

    /// Un-pause a phone paused after failed checks (Settings → Devices).
    pub fn unpause(&self, id: &str) -> Result<()> {
        let mut st = lock(&self.state);
        let ids = ids(&st.devices);
        let device =
            st.devices.iter_mut().find(|d| d.id == id).ok_or_else(|| {
                RemoteError::Invalid("That phone is not on your PC's list.".into())
            })?;
        device.paused = false;
        let copy = device.clone();
        self.kept().put(&copy, &ids)?;
        st.live.entry(id.to_owned()).or_default().checks.clear();
        drop(st);
        self.host.record(
            None,
            "remote.device_unpaused",
            json!({ "device": id, "name": copy.name }),
        );
        self.host.changed(Change::Devices);
        Ok(())
    }

    /// **Remove** a phone: cut off at once (ADR-143 §7).
    pub fn remove(&self, id: &str) -> Result<()> {
        self.remove_by(id, "pc")
    }

    fn remove_by(&self, id: &str, by: &str) -> Result<()> {
        let now = self.now();
        let mut st = lock(&self.state);
        let Some(index) = st.devices.iter().position(|d| d.id == id) else {
            return Err(RemoteError::Invalid(
                "That phone is not on your PC's list.".into(),
            ));
        };
        let remaining: Vec<String> = st
            .devices
            .iter()
            .filter(|d| d.id != id)
            .map(|d| d.id.clone())
            .collect();
        self.kept().remove(id, &remaining)?;
        let device = st.devices.remove(index);
        let conns: Vec<String> = st
            .conns
            .iter()
            .filter(|(_, c)| c.kind == Kind::Phone(id.to_owned()))
            .map(|(k, _)| k.clone())
            .collect();
        for conn in conns {
            self.send_event(
                &mut st,
                &conn,
                Event::SignedOut {
                    why: SignedOutWhy::Removed,
                },
            );
            self.close(&mut st, &conn);
        }
        st.live.remove(id);
        drop(st);
        self.send(ToRelay::Drop {
            phone: device.phone.clone(),
            until: secs(now) + crate::PASS_LIFE_SECS,
        });
        self.host.record(
            None,
            "remote.device_removed",
            json!({ "device": id, "name": device.name, "by": by }),
        );
        self.host.changed(Change::Devices);
        Ok(())
    }

    // ---- The relay link -------------------------------------------------------------------

    /// The relay link is connected: commands go to `out`. What to send first.
    pub fn relay_up(&self, out: UnboundedSender<ToRelay>) {
        *lock(&self.out) = Some(out);
        let mailbox = {
            let mut st = lock(&self.state);
            st.relay_problem = None;
            st.pairing
                .as_ref()
                .filter(|p| p.candidate.is_none())
                .map(|p| p.code.mailbox())
        };
        if let Some(m) = mailbox {
            self.send(ToRelay::Mailbox(m));
        }
        self.host.record(None, "remote.relay_connected", json!({}));
        self.host.changed(Change::Switch);
    }

    /// The relay link dropped: every connection through it is gone (sign-ins stay, so the
    /// phones pick up again within 30 minutes).
    pub fn relay_down(&self, problem: Option<String>) {
        let was = lock(&self.out).take().is_some();
        let mut st = lock(&self.state);
        st.conns.clear();
        if let Some(p) = st.pairing.as_mut() {
            if p.candidate.take().is_some() {
                // The phone being added went with the link.
                st.pairing = None;
            }
        }
        if problem.is_some() {
            st.relay_problem = problem;
        }
        drop(st);
        if was {
            self.host
                .record(None, "remote.relay_disconnected", json!({}));
        }
        self.host.changed(Change::Switch);
    }

    /// Why the relay would not connect (shown in Settings).
    pub fn set_relay_problem(&self, problem: Option<String>) {
        lock(&self.state).relay_problem = problem;
        self.host.changed(Change::Switch);
    }

    fn send(&self, command: ToRelay) {
        if let Some(out) = lock(&self.out).as_ref() {
            let _ = out.send(command);
        }
    }

    /// Close a connection (the relay is told).
    fn close(&self, st: &mut State, conn: &str) {
        if st.conns.remove(conn).is_some() {
            self.send(ToRelay::Close {
                conn: conn.to_owned(),
            });
        }
    }

    /// Make room for one more connection of this kind, or say there is none. The relay says who
    /// joined and is trusted for nothing else (ADR-143), so the table has a ceiling: a relay
    /// that keeps saying phones joined, and never that they left, cannot grow it without end.
    /// Below the ceiling, a phone at its own limit gives up one connection for the new one: the
    /// oldest that never finished its meeting, if there is one (a join that went quiet, or a
    /// stolen pass holding slots), and only when every one of them is open, the oldest open one
    /// (an honest phone that changed networks, whose old lines the relay has not yet reported
    /// gone). So a stolen pass cannot push out the owner's live lines. The mailbox gets no such
    /// favour: an honest relay never sends more mailbox joins than the mailbox takes.
    fn make_room(&self, st: &mut State, kind: &Kind) -> bool {
        if st.conns.len() >= MAX_CONNS {
            return false;
        }
        let same = st.conns.iter().filter(|(_, c)| c.kind == *kind);
        if same.clone().count() < CONNS_PER_DEVICE {
            return true;
        }
        let oldest = |unfinished: bool| {
            same.clone()
                .filter(|(_, c)| !unfinished || matches!(c.lock, Lock::Meeting(_)))
                .min_by_key(|(_, c)| c.since)
                .map(|(conn, _)| conn.clone())
        };
        let gives_way = oldest(true).or_else(|| oldest(false));
        match (kind, gives_way) {
            (Kind::Phone(_), Some(conn)) => {
                self.close(st, &conn);
                true
            }
            _ => false,
        }
    }

    /// A connection the relay announced that there is no room for: closed, and counted as a
    /// failed meeting, so a relay that keeps doing it trips the stop (ADR-143 §8).
    fn no_room(&self, st: MutexGuard<'_, State>, conn: &str) {
        drop(st);
        self.send(ToRelay::Close {
            conn: conn.to_owned(),
        });
        self.count_failed_meeting();
    }

    /// Seal and send a message on an open connection (sealing and sending together, so the
    /// counters go out in order).
    fn send_sealed(&self, st: &mut State, conn: &str, message: &PcSays) {
        let Some(c) = st.conns.get_mut(conn) else {
            return;
        };
        let Lock::Open(transport) = &mut c.lock else {
            return;
        };
        let bytes = serde_json::to_vec(message).expect("a message is JSON");
        match noise::seal(transport, &bytes) {
            Ok(pieces) => {
                let out = lock(&self.out);
                if let Some(out) = out.as_ref() {
                    for data in pieces {
                        let _ = out.send(ToRelay::Send {
                            conn: conn.to_owned(),
                            data,
                        });
                    }
                }
            }
            Err(e) => log::warn!("a message to a phone could not be sealed: {e}"),
        }
    }

    fn send_pair(&self, st: &mut State, conn: &str, step: PairStep) {
        self.send_sealed(st, conn, &PcSays::Pair { pair: step });
    }

    fn send_event(&self, st: &mut State, conn: &str, event: Event) {
        self.send_sealed(st, conn, &PcSays::Event { event });
    }

    /// What the relay said.
    pub fn from_relay(self: &Arc<Self>, message: RelayToPc) {
        match message {
            RelayToPc::Joined {
                conn,
                phone,
                mailbox,
            } => {
                if mailbox {
                    self.mailbox_joined(&conn);
                } else if let Some(phone) = phone {
                    self.phone_joined(&conn, &phone);
                } else {
                    self.send(ToRelay::Close { conn });
                }
            }
            RelayToPc::Data { conn, data } => {
                let Some(bytes) = b64::decode(&data, crate::MAX_NOISE_MESSAGE) else {
                    self.failed_meeting(&conn);
                    return;
                };
                self.data(&conn, &bytes);
            }
            RelayToPc::Left { conn } => self.left(&conn),
            RelayToPc::Challenge { .. }
            | RelayToPc::Welcome { .. }
            | RelayToPc::Refused { .. }
            | RelayToPc::Error { .. } => {}
        }
    }

    fn mailbox_joined(&self, conn: &str) {
        let now = self.now();
        let mut st = lock(&self.state);
        let open = st
            .pairing
            .as_ref()
            .is_some_and(|p| p.candidate.is_none() && now < p.ends);
        let keys = match (open, self.keys(&mut st)) {
            (true, Ok(k)) => k,
            _ => {
                drop(st);
                self.send(ToRelay::Close {
                    conn: conn.to_owned(),
                });
                return;
            }
        };
        if !self.make_room(&mut st, &Kind::Mailbox) {
            self.no_room(st, conn);
            return;
        }
        let psk = st.pairing.as_ref().expect("open").code.psk();
        match noise::pairing_pc(keys.noise_private(), &psk) {
            Ok(hs) => {
                st.conns.insert(
                    conn.to_owned(),
                    Conn {
                        kind: Kind::Mailbox,
                        lock: Lock::Meeting(Box::new(hs)),
                        since: now,
                        read: 0,
                        assembler: Assembler::default(),
                        challenge: None,
                        notice: false,
                    },
                );
            }
            Err(e) => {
                log::warn!("a first meeting could not start: {e}");
                drop(st);
                self.send(ToRelay::Close {
                    conn: conn.to_owned(),
                });
            }
        }
    }

    fn phone_joined(&self, conn: &str, phone: &str) {
        let now = self.now();
        let mut st = lock(&self.state);
        if !st.config.switched_on || !st.meetings.answering(now) {
            drop(st);
            self.send(ToRelay::Close {
                conn: conn.to_owned(),
            });
            return;
        }
        let Ok(keys) = self.keys(&mut st) else {
            drop(st);
            self.send(ToRelay::Close {
                conn: conn.to_owned(),
            });
            return;
        };
        let device = st
            .devices
            .iter()
            .find(|d| d.phone == phone)
            .and_then(|d| Some((d.id.clone(), d.noise_key()?)));
        let Some((id, key)) = device else {
            // A pass the relay still takes, for a phone this PC does not know: removed while it
            // was away, and the relay has met this PC again since. The relay refuses its pass
            // from now on, telling the phone it is no longer on the list (`bad_pass`).
            drop(st);
            self.send(ToRelay::Drop {
                phone: phone.to_owned(),
                until: secs(now) + crate::PASS_LIFE_SECS,
            });
            self.count_failed_meeting();
            return;
        };
        let kind = Kind::Phone(id);
        if !self.make_room(&mut st, &kind) {
            self.no_room(st, conn);
            return;
        }
        let prologue = noise::everyday_prologue(&keys.fingerprint(), phone);
        match noise::everyday_pc(keys.noise_private(), &key, &prologue) {
            Ok(hs) => {
                st.conns.insert(
                    conn.to_owned(),
                    Conn {
                        kind,
                        lock: Lock::Meeting(Box::new(hs)),
                        since: now,
                        read: 0,
                        assembler: Assembler::default(),
                        challenge: None,
                        notice: false,
                    },
                );
            }
            Err(e) => {
                log::warn!("a meeting could not start: {e}");
                drop(st);
                self.send(ToRelay::Close {
                    conn: conn.to_owned(),
                });
            }
        }
    }

    fn left(&self, conn: &str) {
        let mut st = lock(&self.state);
        let Some(c) = st.conns.remove(conn) else {
            return;
        };
        if c.kind == Kind::Mailbox {
            let candidate = st
                .pairing
                .as_ref()
                .and_then(|p| p.candidate.as_ref())
                .is_some_and(|(cc, ..)| cc == conn);
            if candidate {
                st.pairing = None;
                drop(st);
                self.host.record(
                    None,
                    "remote.pairing_refused",
                    json!({ "reason": "phone_left" }),
                );
                self.host.changed(Change::Pairing);
            }
        }
    }

    /// A meeting failed: close it, and count it (ADR-143 §8).
    fn failed_meeting(&self, conn: &str) {
        lock(&self.state).conns.remove(conn);
        self.send(ToRelay::Close {
            conn: conn.to_owned(),
        });
        self.count_failed_meeting();
    }

    fn count_failed_meeting(&self) {
        let now = self.now();
        let mut st = lock(&self.state);
        if !st.meetings.failed(now) {
            return;
        }
        let until = st.meetings.stopped_until();
        drop(st);
        self.host
            .record(None, "remote.meetings_stopped", json!({ "until": until }));
        self.host.changed(Change::Attempts);
    }

    fn data(self: &Arc<Self>, conn: &str, bytes: &[u8]) {
        let mut st = lock(&self.state);
        let Some(c) = st.conns.get_mut(conn) else {
            drop(st);
            self.send(ToRelay::Close {
                conn: conn.to_owned(),
            });
            return;
        };
        let kind = c.kind.clone();
        if matches!(c.lock, Lock::Meeting(_)) {
            match kind {
                Kind::Mailbox => self.pairing_meeting(st, conn, bytes),
                Kind::Phone(id) => self.everyday_meeting(st, conn, &id, bytes),
            }
            return;
        }
        let Conn {
            lock: line,
            assembler,
            ..
        } = c;
        let Lock::Open(transport) = line else {
            drop(st);
            self.failed_meeting(conn);
            return;
        };
        let opened =
            noise::open(transport, bytes).and_then(|(more, piece)| assembler.add(more, piece));
        match opened {
            Ok(None) => {}
            Ok(Some(whole)) => match kind {
                Kind::Mailbox => self.pairing_message(st, conn, &whole),
                Kind::Phone(id) => {
                    drop(st);
                    self.phone_message(conn, &id, &whole);
                }
            },
            Err(_) => {
                // Copied, changed, dropped, out of order, or not ours: end the meeting.
                drop(st);
                self.failed_meeting(conn);
            }
        }
    }

    // ---- Pairing (ADR-141) ----------------------------------------------------------------

    fn pairing_meeting(&self, mut st: MutexGuard<'_, State>, conn: &str, bytes: &[u8]) {
        let now = self.now();
        let c = st.conns.get_mut(conn).expect("looked up");
        let read = c.read;
        let Lock::Meeting(hs) = &mut c.lock else {
            return;
        };
        let payload = match noise::read(hs, bytes) {
            Ok(p) => p,
            Err(_) => {
                drop(st);
                self.wrong_code(conn);
                return;
            }
        };
        c.read += 1;
        if read == 0 {
            // The first message carries nothing; the PC answers with the second.
            let m2 = if payload.is_empty() {
                noise::write(hs, b"").ok()
            } else {
                None
            };
            drop(st);
            match m2 {
                Some(data) => self.send(ToRelay::Send {
                    conn: conn.to_owned(),
                    data,
                }),
                None => self.wrong_code(conn),
            }
            return;
        }
        // The third message: the code was right, and it says what the phone calls itself.
        let key: Option<[u8; 32]> = hs.get_remote_static().and_then(|k| k.try_into().ok());
        let hello: Option<PairHello> = serde_json::from_slice(&payload).ok();
        // Six digits both screens show, from this meeting's own hash (ADR-212): another phone's
        // meeting with the same code makes different ones.
        let check = noise::check_digits(hs.get_handshake_hash());
        let Lock::Meeting(hs) = std::mem::replace(&mut c.lock, Lock::Gone) else {
            unreachable!("matched above")
        };
        let (Some(key), Some(hello), Ok(transport)) = (key, hello, hs.into_transport_mode()) else {
            drop(st);
            self.wrong_code(conn);
            return;
        };
        c.lock = Lock::Open(Box::new(transport));
        let hello = PairHello {
            name: clean_name(&hello.name, "Phone"),
            browser: clean_name(&hello.browser, "A web browser"),
        };
        let Some(p) = st.pairing.as_mut() else {
            // Add a phone was stopped on the PC while this phone was still meeting it.
            self.close(&mut st, conn);
            return;
        };
        if p.candidate.is_some() {
            // Another phone finished first with this code. This one is told why, so the owner's
            // own phone, if it lost, says what happened and what to do (ADR-212).
            self.send_pair(
                &mut st,
                conn,
                PairStep::Refused {
                    message: "Another phone already used this code. If that was not you, click \
                              Cancel on your PC and start again."
                        .into(),
                },
            );
            self.close(&mut st, conn);
            drop(st);
            self.host.record(
                None,
                "remote.pairing_refused",
                json!({ "reason": "used", "name": hello.name }),
            );
            return;
        }
        // The code is used: nobody else may use it.
        p.candidate = Some((
            conn.to_owned(),
            hello,
            key,
            Candidate::AskingOwner { check },
            now,
        ));
        self.send_pair(&mut st, conn, PairStep::Waiting);
        drop(st);
        self.send(ToRelay::CloseMailbox);
        self.host.changed(Change::Pairing);
    }

    /// A first meeting failed: a wrong code (ADR-141 §5).
    fn wrong_code(&self, conn: &str) {
        let now = self.now();
        let mut st = lock(&self.state);
        self.close(&mut st, conn);
        let Some(p) = st.pairing.as_mut() else {
            return;
        };
        if p.candidate.is_some() {
            return;
        }
        p.wrong += 1;
        if p.wrong < CODE_TRIES {
            drop(st);
            self.host.changed(Change::Pairing);
            return;
        }
        st.pairing = None;
        let paused = st.dead_codes.died(now);
        drop(st);
        self.send(ToRelay::CloseMailbox);
        self.host.record(
            None,
            "remote.pairing_refused",
            json!({ "reason": "wrong_code", "pairingPaused": paused }),
        );
        self.host.changed(Change::Pairing);
    }

    fn pairing_message(&self, mut st: MutexGuard<'_, State>, conn: &str, whole: &[u8]) {
        let now = self.now();
        let message: Option<PhoneSays> = serde_json::from_slice(whole).ok();
        let Some(PhoneSays::Passkey { passkey }) = message else {
            self.close(&mut st, conn);
            return;
        };
        let Some(pairing) = st.pairing.as_ref() else {
            self.close(&mut st, conn);
            return;
        };
        let Some((
            cc,
            hello,
            key,
            Candidate::MakingPasskey {
                device,
                phone,
                challenge,
            },
            _,
        )) = pairing.candidate.as_ref()
        else {
            self.close(&mut st, conn);
            return;
        };
        if cc != conn {
            self.close(&mut st, conn);
            return;
        }
        let (hello, key, device, phone, challenge) = (
            hello.clone(),
            *key,
            device.clone(),
            phone.clone(),
            *challenge,
        );
        st.pairing = None;
        let made = webauthn::register(&passkey, &self.expect(&challenge));
        let passkey = match made {
            Ok(p) => p,
            Err(e) => {
                self.send_pair(
                    &mut st,
                    conn,
                    PairStep::Failed {
                        message: format!("Your PC could not check this phone's passkey: {e}."),
                    },
                );
                self.close(&mut st, conn);
                drop(st);
                self.host.record(
                    None,
                    "remote.pairing_refused",
                    json!({ "reason": "passkey", "problem": e.to_string() }),
                );
                self.host.changed(Change::Pairing);
                return;
            }
        };
        let record = Device {
            id: device.clone(),
            name: hello.name.clone(),
            browser: hello.browser.clone(),
            phone,
            key: b64::encode(&key),
            passkey,
            added_at: now,
            last_seen_at: Some(now),
            paused: false,
            notices: None,
        };
        let mut all = ids(&st.devices);
        all.push(device.clone());
        if let Err(e) = self.kept().put(&record, &all) {
            self.send_pair(
                &mut st,
                conn,
                PairStep::Failed {
                    message: e.to_string(),
                },
            );
            self.close(&mut st, conn);
            drop(st);
            self.host.changed(Change::Pairing);
            return;
        }
        st.devices.push(record);
        // Making the passkey checked the owner (face, fingerprint, or passcode): signed in.
        let live = st.live.entry(device.clone()).or_default();
        live.signed_in_at = Some(now);
        live.last_request = now;
        self.send_pair(&mut st, conn, PairStep::Done);
        self.close(&mut st, conn);
        drop(st);
        self.host.record(
            None,
            "remote.device_added",
            json!({ "device": device, "name": hello.name, "browser": hello.browser }),
        );
        self.host.changed(Change::Pairing);
        self.host.changed(Change::Devices);
    }

    // ---- Everyday meetings, sign-in, and requests (ADR-142, ADR-143, ADR-145) ---------------

    fn everyday_meeting(&self, mut st: MutexGuard<'_, State>, conn: &str, id: &str, bytes: &[u8]) {
        let now = self.now();
        let signed = st.live.get(id).is_some_and(|l| signed_in(l, now));
        // The PC's notice key, for the phone to sign up for notices (part 14C).
        let notice_key = st.keys.as_ref().map(PcKeys::notice_public);
        let c = st.conns.get_mut(conn).expect("looked up");
        let Lock::Meeting(hs) = &mut c.lock else {
            return;
        };
        let Ok(payload) = noise::read(hs, bytes) else {
            drop(st);
            self.failed_meeting(conn);
            return;
        };
        let hello: MeetingHello = if payload.is_empty() {
            MeetingHello::default()
        } else if let Ok(h) = serde_json::from_slice(&payload) {
            h
        } else {
            drop(st);
            self.failed_meeting(conn);
            return;
        };
        let challenge: Option<[u8; 32]> = (!signed && !hello.notice).then(crate::random);
        let welcome = MeetingWelcome {
            signed_in: signed && !hello.notice,
            challenge: challenge.map(|c| b64::encode(&c)),
            pc_name: self.settings.pc_name.clone(),
            version: self.settings.version.clone(),
            notice_key,
        };
        let written = noise::write(
            hs,
            &serde_json::to_vec(&welcome).expect("a welcome is JSON"),
        );
        let Ok(m2) = written else {
            drop(st);
            self.failed_meeting(conn);
            return;
        };
        let Lock::Meeting(hs) = std::mem::replace(&mut c.lock, Lock::Gone) else {
            unreachable!("matched above")
        };
        let Ok(transport) = hs.into_transport_mode() else {
            drop(st);
            self.failed_meeting(conn);
            return;
        };
        c.lock = Lock::Open(Box::new(transport));
        c.notice = hello.notice;
        c.challenge = challenge.map(|ch| (ch, now + crate::CHALLENGE_LIFE_MS));
        // Sent while the state is held: the second message goes before any sealed one.
        let out = lock(&self.out);
        if let Some(out) = out.as_ref() {
            let _ = out.send(ToRelay::Send {
                conn: conn.to_owned(),
                data: m2,
            });
        }
    }

    fn phone_message(self: &Arc<Self>, conn: &str, id: &str, whole: &[u8]) {
        let message: Option<PhoneSays> = serde_json::from_slice(whole).ok();
        let Some(PhoneSays::Ask {
            id: request,
            again,
            ask,
        }) = message
        else {
            // Not a request (or not one on the list): the meeting ends.
            self.failed_meeting(conn);
            return;
        };
        if !b64::is_id(&request, 16) || !ask.sane() {
            self.reply(
                conn,
                id,
                Reply::refused(&request, None, "Your PC could not read that request."),
            );
            return;
        }
        if let Ask::SignIn { answer } = &ask {
            self.sign_in(conn, id, &request, answer);
            return;
        }
        let now = self.now();
        let mut st = lock(&self.state);
        let Some(device) = st.devices.iter().find(|d| d.id == id).cloned() else {
            self.close(&mut st, conn);
            return;
        };
        let from_notice = st.conns.get(conn).is_some_and(|c| c.notice);
        let live = st.live.entry(id.to_owned()).or_default();
        let signed = signed_in(live, now);
        // A copied request is refused: each ID once in a sign-in (ADR-143 §6).
        if live.seen.contains(&request) {
            drop(st);
            self.host.record(
                ask.org(),
                "remote.refused",
                json!({
                    "device": device.id, "name": device.name,
                    "kind": ask.kind().map(|k| k.label()), "why": "copied",
                }),
            );
            self.reply(
                conn,
                id,
                Reply::refused(&request, None, "Your PC already had that request."),
            );
            return;
        }
        live.seen.insert(request.clone());
        live.seen_order.push_back(request.clone());
        while live.seen_order.len() > SEEN_KEPT {
            if let Some(old) = live.seen_order.pop_front() {
                live.seen.remove(&old);
            }
        }
        // Only what the owner does keeps the phone signed in: a page the phone reads again by
        // itself (`again`) does not move the 30-minute clock (ADR-212, P-SRV-3).
        if signed && !from_notice && !again {
            live.last_request = now;
        }
        if let Ask::Outcome { of } = &ask {
            let found = live
                .outcomes
                .iter()
                .find(|(r, _)| r == of)
                .map(|(_, reply)| reply.clone());
            drop(st);
            let reply = if signed {
                Reply::ok(
                    &request,
                    json!({ "known": found.is_some(), "reply": found }),
                )
            } else {
                Reply::refused(
                    &request,
                    Some(Why::NotSignedIn),
                    "Sign in on this phone first.",
                )
            };
            self.reply(conn, id, reply);
            return;
        }
        let switched_on = st.config.switched_on;
        let kept = st.config.kept.clone();
        let phone_state = if device.paused {
            PhoneState::Paused
        } else {
            PhoneState::Paired
        };
        drop(st);
        let this = Arc::clone(self);
        let conn = conn.to_owned();
        let work = move || {
            this.decide_and_carry_out(
                &conn,
                &device,
                &request,
                again,
                &ask,
                Gate {
                    switched_on,
                    signed,
                    from_notice,
                    phone_state,
                    kept: &kept,
                },
            );
        };
        match tokio::runtime::Handle::try_current() {
            Ok(rt) => {
                rt.spawn_blocking(work);
            }
            Err(_) => work(),
        }
    }

    fn decide_and_carry_out(
        self: &Arc<Self>,
        conn: &str,
        device: &Device,
        request: &str,
        again: bool,
        ask: &Ask,
        gate: Gate<'_>,
    ) {
        let kind = ask.kind().expect("signing in is handled before");
        let approval = match ask {
            Ask::Approve { org, approval } | Ask::Refuse { org, approval } => {
                self.host.approval(org, approval)
            }
            _ => None,
        };
        let check = guard_remote::RemoteCheck {
            switched_on: gate.switched_on,
            pro: self.host.pro(),
            phone: gate.phone_state,
            signed_in: gate.signed,
            from_notice: gate.from_notice,
            kind,
            approval,
            kept: gate.kept,
        };
        let base = json!({
            "device": device.id,
            "name": device.name,
            "kind": kind.label(),
            "target": ask.target(),
            "fromNotice": gate.from_notice,
        });
        if let Err(refusal) = guard_remote::decide(&check) {
            let mut payload = base;
            payload["why"] = serde_json::to_value(refusal.why).unwrap_or(Value::Null);
            self.host.record(ask.org(), "remote.refused", payload);
            let reply = Reply::refused(request, Some(refusal.why), refusal.message);
            self.reply(conn, &device.id, reply);
            return;
        }
        if !(kind.reads() && again) {
            self.host.record(ask.org(), "remote.request", base);
        }
        let phone = Phone {
            id: device.id.clone(),
            name: device.name.clone(),
        };
        let reply = match ask {
            // Ended before it is answered: once the phone hears it is signed out, nothing more it
            // sends is carried out. The line stays open, for signing in again.
            Ask::SignOut => {
                let mut st = lock(&self.state);
                self.end_sign_in(&mut st, &device.id, SignedOutWhy::You);
                drop(st);
                self.reply(
                    conn,
                    &device.id,
                    Reply::ok(request, json!({ "signedOut": true })),
                );
                return;
            }
            // Answered first: removing the phone closes the line.
            Ask::RemoveThisPhone => {
                self.reply(
                    conn,
                    &device.id,
                    Reply::ok(request, json!({ "removed": true })),
                );
                if let Err(e) = self.remove_by(&device.id, "phone") {
                    log::warn!("a phone could not remove itself: {e}");
                }
                return;
            }
            Ask::NoticesOn { subscription } => {
                // Only an address Guard lets notices go to is kept.
                if let Err(why) = self.host.notice_address(&subscription.endpoint) {
                    Reply::refused(request, None, why)
                } else {
                    match self.set_notices(&device.id, Some(subscription.clone())) {
                        Ok(()) => Reply::ok(request, json!({ "notices": true })),
                        Err(e) => Reply::failed(request, e.to_string()),
                    }
                }
            }
            Ask::NoticesOff => match self.set_notices(&device.id, None) {
                Ok(()) => Reply::ok(request, json!({ "notices": false })),
                Err(e) => Reply::failed(request, e.to_string()),
            },
            _ => match self.host.carry_out(&phone, ask) {
                Ok(value) => Reply::ok(request, value),
                Err(why) => Reply::failed(request, why),
            },
        };
        self.reply(conn, &device.id, reply);
    }

    /// Answer a request, and keep the answer for a phone whose connection dropped.
    fn reply(&self, conn: &str, device: &str, reply: Reply) {
        let mut st = lock(&self.state);
        if let Some(live) = st.live.get_mut(device) {
            live.outcomes.push_back((reply.re.clone(), reply.clone()));
            while live.outcomes.len() > OUTCOMES_KEPT {
                live.outcomes.pop_front();
            }
        }
        self.send_sealed(&mut st, conn, &PcSays::Reply { reply });
    }

    fn sign_in(
        self: &Arc<Self>,
        conn: &str,
        id: &str,
        request: &str,
        answer: &webauthn::PasskeyAnswer,
    ) {
        let now = self.now();
        let mut st = lock(&self.state);
        let Some(device) = st.devices.iter().find(|d| d.id == id).cloned() else {
            self.close(&mut st, conn);
            return;
        };
        let refuse = |this: &Self, st: MutexGuard<'_, State>, why: Option<Why>, message: String| {
            drop(st);
            this.reply(conn, id, Reply::refused(request, why, message));
        };
        let gate = if !st.config.switched_on {
            Some(Why::SwitchedOff)
        } else if !self.host.pro() {
            Some(Why::NotPro)
        } else if device.paused {
            Some(Why::Paused)
        } else {
            None
        };
        if let Some(why) = gate {
            refuse(self, st, Some(why), guard_remote::Refusal::new(why).message);
            return;
        }
        let challenge = st
            .conns
            .get_mut(conn)
            .and_then(|c| c.challenge.take())
            .filter(|(_, ends)| now < *ends)
            .map(|(c, _)| c);
        let Some(challenge) = challenge else {
            refuse(
                self,
                st,
                None,
                "That sign-in was too slow or already used. Try again.".into(),
            );
            return;
        };
        match webauthn::check(&device.passkey, answer, &self.expect(&challenge)) {
            Ok(counter) => {
                let keys = match self.keys(&mut st) {
                    Ok(k) => k,
                    Err(e) => {
                        refuse(self, st, None, e.to_string());
                        return;
                    }
                };
                let all = ids(&st.devices);
                let index = st
                    .devices
                    .iter()
                    .position(|d| d.id == id)
                    .expect("found above");
                st.devices[index].passkey.counter = counter;
                st.devices[index].last_seen_at = Some(now);
                let copy = st.devices[index].clone();
                if let Err(e) = self.kept().put(&copy, &all) {
                    log::warn!("a phone's sign-in could not be kept: {e}");
                }
                let live = st.live.entry(id.to_owned()).or_default();
                live.signed_in_at = Some(now);
                live.last_request = now;
                live.seen.clear();
                live.seen_order.clear();
                live.outcomes.clear();
                live.checks.clear();
                let pass = crate::pass::issue(&keys, &copy.phone, secs(now), crate::PASS_LIFE_SECS);
                let signed = SignedIn {
                    pass,
                    ends_at: now + crate::SIGNED_IN_MOST_MS,
                };
                drop(st);
                self.host.record(
                    None,
                    "remote.signed_in",
                    json!({ "device": id, "name": copy.name }),
                );
                self.host.changed(Change::Devices);
                self.reply(
                    conn,
                    id,
                    Reply::ok(request, serde_json::to_value(signed).expect("JSON")),
                );
            }
            Err(e) => {
                let pause = st.live.entry(id.to_owned()).or_default().checks.failed(now);
                let mut paused_now = false;
                if pause {
                    let all = ids(&st.devices);
                    if let Some(d) = st.devices.iter_mut().find(|d| d.id == id) {
                        d.paused = true;
                        let copy = d.clone();
                        if let Err(e) = self.kept().put(&copy, &all) {
                            log::warn!("a paused phone could not be kept: {e}");
                        }
                        paused_now = true;
                    }
                }
                drop(st);
                self.host.record(
                    None,
                    "remote.check_refused",
                    json!({ "device": id, "name": device.name, "problem": e.to_string() }),
                );
                if paused_now {
                    self.host.record(
                        None,
                        "remote.device_paused",
                        json!({ "device": id, "name": device.name }),
                    );
                    self.host.changed(Change::Devices);
                    self.host.changed(Change::Attempts);
                }
                let (why, message) = if paused_now {
                    (
                        Some(Why::Paused),
                        guard_remote::Refusal::new(Why::Paused).message,
                    )
                } else {
                    (
                        None,
                        format!("Your PC could not check that it is you: {e}."),
                    )
                };
                self.reply(conn, id, Reply::refused(request, why, message));
            }
        }
    }

    /// End a phone's sign-in, telling its open connections and recording why.
    fn end_sign_in(&self, st: &mut State, id: &str, why: SignedOutWhy) {
        let Some(live) = st.live.get_mut(id) else {
            return;
        };
        if live.signed_in_at.take().is_none() {
            return;
        }
        live.seen.clear();
        live.seen_order.clear();
        let name = st
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.name.clone())
            .unwrap_or_default();
        let conns: Vec<String> = st
            .conns
            .iter()
            .filter(|(_, c)| c.kind == Kind::Phone(id.to_owned()) && !c.notice)
            .map(|(k, _)| k.clone())
            .collect();
        for conn in conns {
            self.send_event(st, &conn, Event::SignedOut { why });
        }
        self.host.record(
            None,
            "remote.signed_out",
            json!({ "device": id, "name": name, "why": why.word() }),
        );
    }

    fn set_notices(
        &self,
        id: &str,
        subscription: Option<crate::protocol::Subscription>,
    ) -> Result<()> {
        let mut st = lock(&self.state);
        let all = ids(&st.devices);
        let device =
            st.devices.iter_mut().find(|d| d.id == id).ok_or_else(|| {
                RemoteError::Invalid("That phone is not on your PC's list.".into())
            })?;
        device.notices = subscription;
        let copy = device.clone();
        self.kept().put(&copy, &all)?;
        drop(st);
        self.host.changed(Change::Devices);
        Ok(())
    }

    /// The phones that take notices now, and where (part 14C).
    pub fn notice_targets(&self) -> Vec<(Phone, crate::protocol::Subscription)> {
        lock(&self.state)
            .devices
            .iter()
            .filter(|d| !d.paused)
            .filter_map(|d| {
                Some((
                    Phone {
                        id: d.id.clone(),
                        name: d.name.clone(),
                    },
                    d.notices.clone()?,
                ))
            })
            .collect()
    }

    /// Forget a phone's notice address (its notice service said it is gone).
    pub fn notices_gone(&self, id: &str) {
        let _ = self.set_notices(id, None);
    }

    /// Something changed on the PC: every signed-in phone reads that page again.
    pub fn notify(&self, org: Option<&str>, what: Changed) {
        let now = self.now();
        let mut st = lock(&self.state);
        let conns: Vec<String> = st
            .conns
            .iter()
            .filter(|(_, c)| {
                !c.notice
                    && matches!(c.lock, Lock::Open(_))
                    && match &c.kind {
                        Kind::Phone(id) => st.live.get(id).is_some_and(|l| signed_in(l, now)),
                        Kind::Mailbox => false,
                    }
            })
            .map(|(k, _)| k.clone())
            .collect();
        for conn in conns {
            self.send_event(
                &mut st,
                &conn,
                Event::Changed {
                    org: org.map(str::to_owned),
                    what,
                },
            );
        }
    }

    /// The regular look (every few seconds while the relay link runs): sign-ins that lapsed,
    /// codes that ran out, and challenges that ran out.
    pub fn tick(&self) {
        let now = self.now();
        let mut st = lock(&self.state);
        let lapsed: Vec<(String, SignedOutWhy)> = st
            .live
            .iter()
            .filter_map(|(id, l)| {
                let at = l.signed_in_at?;
                if now.saturating_sub(at) >= crate::SIGNED_IN_MOST_MS {
                    Some((id.clone(), SignedOutWhy::TwelveHours))
                } else if now.saturating_sub(l.last_request) >= crate::SIGNED_IN_IDLE_MS {
                    Some((id.clone(), SignedOutWhy::Idle))
                } else {
                    None
                }
            })
            .collect();
        let any_lapsed = !lapsed.is_empty();
        for (id, why) in lapsed {
            self.end_sign_in(&mut st, &id, why);
        }
        let mut pairing_changed = false;
        if let Some(p) = &st.pairing {
            let (expired, conn) = match &p.candidate {
                None => (now >= p.ends, None),
                Some((conn, _, _, _, since)) => (
                    now.saturating_sub(*since) >= crate::PAIRING_FINISH_MS,
                    Some(conn.clone()),
                ),
            };
            if expired {
                st.pairing = None;
                if let Some(conn) = conn {
                    self.send_pair(
                        &mut st,
                        &conn,
                        PairStep::Refused {
                            message: "Adding this phone took too long. Start again on your PC."
                                .into(),
                        },
                    );
                    self.close(&mut st, &conn);
                }
                self.send(ToRelay::CloseMailbox);
                pairing_changed = true;
            }
        }
        for c in st.conns.values_mut() {
            if c.challenge.is_some_and(|(_, ends)| now >= ends) {
                c.challenge = None;
            }
        }
        // A meeting that has not finished in its time: the connection joined and went quiet,
        // or sent a first message and no more. Closed, so it cannot hold a slot (phone or
        // mailbox), and not counted as a failed meeting: a held slot is not a wrong try, and
        // counting it would let a stranger trip the stop on purpose.
        let quiet: Vec<String> = st
            .conns
            .iter()
            .filter(|(_, c)| {
                matches!(c.lock, Lock::Meeting(_))
                    && now.saturating_sub(c.since) >= MEETING_DEADLINE_MS
            })
            .map(|(conn, _)| conn.clone())
            .collect();
        for conn in &quiet {
            log::debug!("a meeting did not finish in time: closed");
            self.close(&mut st, conn);
        }
        drop(st);
        if pairing_changed {
            self.host.record(
                None,
                "remote.pairing_refused",
                json!({ "reason": "expired" }),
            );
            self.host.changed(Change::Pairing);
        }
        if any_lapsed {
            self.host.changed(Change::Devices);
        }
    }
}

/// What Guard needs to know about the phone, gathered when the request arrived.
struct Gate<'a> {
    switched_on: bool,
    signed: bool,
    from_notice: bool,
    phone_state: PhoneState,
    kept: &'a KeptOnPc,
}

fn signed_in(live: &Live, now: u64) -> bool {
    live.signed_in_at.is_some_and(|at| {
        now.saturating_sub(at) < crate::SIGNED_IN_MOST_MS
            && now.saturating_sub(live.last_request) < crate::SIGNED_IN_IDLE_MS
    })
}

fn ids(devices: &[Device]) -> Vec<String> {
    devices.iter().map(|d| d.id.clone()).collect()
}
