//! Free and Pro on this PC (Phase 11A: ADR-021, ADR-022, ADR-110, ADR-115, ADR-116).
//!
//! One license for the whole PC (ADR-110): the key is kept in the Vault under the first
//! organization's name, and its record (not a secret) in Plenipo's data folder. Every
//! organization's services share the PC's [`Entitlements`]; this keeps its edition current.
//!
//! A copy with no key never contacts 8 West (ADR-115). With a key, Plenipo checks once a week
//! through Guard (the license check's one address), and tries again sooner after a failure. Pro
//! stays on for 30 days after 8 West's last signed answer, so winding the PC's clock back never
//! extends it (ADR-116). Nothing is deleted when Pro ends (ADR-021).
//!
//! Where the check goes is built into each copy: 8 West's address, or, in copies built for the
//! tests only, a stand-in on this computer (`PLENIPO_LICENSE_STAND_IN` at build time). Never a
//! setting, never an environment variable at run time.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use plenipo_capabilities::vault::{self, SecretStore};
use plenipo_core::CommandError;
use plenipo_guard::{Guard, OutboundRules};
use plenipo_ledger::{Ledger, NewEvent};
use plenipo_licensing::{
    CheckOutcome, Entitlements, License, LicenseReason, LicenseView, Limit, Record, Usage,
};
use serde_json::json;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::orgs::Orgs;

/// The key's name in the Vault (under the first organization's name).
pub const VAULT_ID: &str = "plenipo-license-key";
/// The record (the last answer, the clock, the last try), in Plenipo's data folder.
pub const RECORD_FILE: &str = "license.json";
/// Told to every window when the license changes (Settings → License reads it again).
pub const LICENSE_EVENT: &str = "plenipo://license";
/// Ledger events (the first organization's). Never the key itself: its ID only.
pub const KEY_ENTERED: &str = "license.key_entered";
/// A key that did not check: why, in plain words (never what was typed).
pub const KEY_REFUSED: &str = "license.key_refused";
pub const KEY_REMOVED: &str = "license.key_removed";
pub const CHECKED: &str = "license.checked";
pub const CHECK_FAILED: &str = "license.check_failed";
pub const EDITION_CHANGED: &str = "license.edition_changed";
/// How often Plenipo looks at whether a check is due, or Pro has ended by the clock.
pub const LOOK_EVERY: Duration = Duration::from_secs(10 * 60);
/// The first look waits this long after Plenipo starts.
pub const FIRST_LOOK: Duration = Duration::from_secs(30);
/// The longest key text accepted at the boundary (bytes).
pub const MAX_KEY_BYTES: usize = 4 * plenipo_licensing::key::MAX_KEY_CHARS;
/// Recorded as the one who acted: Plenipo itself.
const PLENIPO: &str = "plenipo";

/// Where this copy's weekly check goes, and the rules Guard checks that address against. A
/// stand-in is used only by a copy that also trusts the contract's test key: a release never
/// sends its check anywhere but 8 West.
pub fn built_check() -> (String, OutboundRules) {
    check_for(
        option_env!("PLENIPO_LICENSE_STAND_IN")
            .filter(|_| plenipo_licensing::trust::built_for_tests()),
    )
}

pub(crate) fn check_for(stand_in: Option<&str>) -> (String, OutboundRules) {
    let stand_in = stand_in
        .map(str::trim)
        .filter(|b| b.starts_with("http://127.0.0.1:"));
    let rules = OutboundRules::default().with_license_stand_in(stand_in);
    let address = match (stand_in, rules.license_test_port) {
        (Some(base), Some(_)) => format!("{}/v1/check", base.trim_end_matches('/')),
        _ => plenipo_licensing::CHECK_ADDRESS.to_owned(),
    };
    (address, rules)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The clocks the license goes by.
#[derive(Clone, Copy)]
pub(crate) struct Clocks {
    /// The PC's clock, in Unix seconds.
    pub pc: fn() -> i64,
    /// Seconds on a clock that only moves forward while Plenipo runs (not the PC's clock).
    pub running: fn() -> i64,
}

impl Clocks {
    pub(crate) const REAL: Self = Self {
        pc: plenipo_licensing::clock,
        running: running_seconds,
    };
}

fn running_seconds() -> i64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    i64::try_from(
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs(),
    )
    .unwrap_or(i64::MAX)
}

/// A Ledger event to record once the license is no longer held.
type Event = (&'static str, serde_json::Value);

/// The license on this PC, and the PC's Free or Pro.
pub struct LicenseHost {
    entitlements: Arc<Entitlements>,
    store: Arc<dyn SecretStore>,
    record_file: Option<PathBuf>,
    version: String,
    address: String,
    rules: OutboundRules,
    clocks: Clocks,
    /// The running clock and the PC's clock at one moment: while Plenipo runs, its time moves on
    /// from there even if the PC's clock is set back.
    anchor: Mutex<(i64, i64)>,
    license: Mutex<License>,
    /// Why the key could not be read from the Vault. The record is then left as it was, and each
    /// look tries the Vault again.
    unreadable: Mutex<Option<String>>,
    /// The record as last written (nothing is written when it has not changed).
    saved: Mutex<Vec<u8>>,
    /// One check at a time.
    checking: tokio::sync::Mutex<()>,
}

impl LicenseHost {
    /// What was kept: the key from `store`, the record from `record_file` (`None`: nothing is
    /// kept, the tests).
    pub fn load(
        store: Arc<dyn SecretStore>,
        record_file: Option<PathBuf>,
        version: &str,
    ) -> Arc<Self> {
        Self::with(store, record_file, version, built_check(), Clocks::REAL)
    }

    pub(crate) fn with(
        store: Arc<dyn SecretStore>,
        record_file: Option<PathBuf>,
        version: &str,
        (address, rules): (String, OutboundRules),
        clocks: Clocks,
    ) -> Arc<Self> {
        let now = (clocks.pc)();
        let (text, unreadable) = read_key(store.as_ref());
        let saved = read_record_bytes(record_file.as_deref());
        let record: Record = serde_json::from_slice(&saved).unwrap_or_default();
        let license = License::load(text.as_deref(), record, now);
        let host = Arc::new(Self {
            entitlements: Entitlements::new(license.edition(now)),
            store,
            record_file,
            version: version.to_owned(),
            address,
            rules,
            clocks,
            anchor: Mutex::new(((clocks.running)(), now)),
            license: Mutex::new(license),
            unreadable: Mutex::new(unreadable),
            saved: Mutex::new(saved),
            checking: tokio::sync::Mutex::new(()),
        });
        host.save(&lock(&host.license));
        host
    }

    /// The PC's Free or Pro, shared by every organization's services.
    pub fn entitlements(&self) -> Arc<Entitlements> {
        Arc::clone(&self.entitlements)
    }

    /// Plenipo's time: the PC's clock, or more when the clock was set back while Plenipo ran.
    fn now(&self) -> i64 {
        let (running, pc) = *lock(&self.anchor);
        (self.clocks.pc)().max(pc.saturating_add((self.clocks.running)().saturating_sub(running)))
    }

    /// Start Plenipo's time again from the PC's clock (8 West's newest answer showed the clock
    /// that held it was ahead).
    fn restart_time(&self) {
        *lock(&self.anchor) = ((self.clocks.running)(), (self.clocks.pc)());
    }

    /// Settings → License (never the key itself).
    pub fn view(&self) -> LicenseView {
        let now = self.now();
        let mut view = lock(&self.license).view(now);
        if view.problem.is_none() {
            view.problem.clone_from(&lock(&self.unreadable));
        }
        view
    }

    /// Enter a key (Settings → License): checked at once, with no network, kept in the Vault,
    /// and Pro from now. The caller starts the first check.
    pub fn enter(&self, text: &str, ledger: &Ledger) -> Result<LicenseView, CommandError> {
        let key = plenipo_licensing::key::parse(text).map_err(|e| {
            record(ledger, KEY_REFUSED, json!({ "reason": e.to_string() }));
            CommandError::invalid_input(e.to_string())
        })?;
        // Kept in the Vault first: the key is in use only once it is kept.
        vault::put(self.store.as_ref(), VAULT_ID, key.text()).map_err(|e| {
            CommandError::internal(format!(
                "Plenipo couldn't keep the key in {}: {e}",
                self.store.label()
            ))
        })?;
        let was_unreadable = lock(&self.unreadable).take().is_some();
        let now = self.now();
        let mut events = vec![(KEY_ENTERED, json!({ "keyId": key.key_id() }))];
        let view = {
            let mut license = lock(&self.license);
            if was_unreadable {
                // The record in memory was never read with its key: take the one kept on disk.
                let record =
                    serde_json::from_slice(&read_record_bytes(self.record_file.as_deref()))
                        .unwrap_or_default();
                *license = License::load(None, record, now);
            }
            license
                .enter(key.text(), now)
                .map_err(|e| CommandError::invalid_input(e.to_string()))?;
            self.save(&license);
            events.extend(self.settle(&license, now));
            license.view(now)
        };
        record_all(ledger, events);
        Ok(view)
    }

    /// Remove the key: Free from now, with nothing else changed (ADR-021).
    pub fn remove(&self, ledger: &Ledger) -> Result<LicenseView, CommandError> {
        vault::erase(self.store.as_ref(), VAULT_ID).map_err(|e| {
            CommandError::internal(format!(
                "Plenipo couldn't remove the key from {}: {e}",
                self.store.label()
            ))
        })?;
        *lock(&self.unreadable) = None;
        let now = self.now();
        let mut events = Vec::new();
        let view = {
            let mut license = lock(&self.license);
            if let Some(key) = license.remove() {
                events.push((KEY_REMOVED, json!({ "keyId": key.key_id() })));
            }
            self.save(&license);
            events.extend(self.settle(&license, now));
            license.view(now)
        };
        record_all(ledger, events);
        Ok(view)
    }

    /// Check with 8 West now: the key's ID and this version, nothing else (ADR-022). With no
    /// key, nothing is sent (a Free copy never checks in, ADR-115).
    pub async fn check(&self, guard: &Guard, ledger: &Ledger) -> LicenseView {
        let _one = self.checking.lock().await;
        self.read_the_vault_again(ledger);
        let request = lock(&self.license).check_request(&self.version);
        let Some((key_id, body)) = request else {
            return self.view();
        };
        let answer =
            plenipo_capabilities::license_check::post(guard, &self.rules, &self.address, body)
                .await;
        let mut events = Vec::new();
        {
            let mut license = lock(&self.license);
            // The key was removed or replaced while the check ran: its answer is not this one's.
            if license.key().map(|k| k.key_id()) == Some(key_id.as_str()) {
                // The next check is timed by the PC's clock.
                let pc = (self.clocks.pc)();
                let before = license.record().clock_high;
                let outcome = match answer {
                    Ok(body) => license.answered(&body, pc),
                    Err(why) => license.failed(&why, pc),
                };
                if license.record().clock_high < before {
                    self.restart_time();
                }
                match outcome {
                    CheckOutcome::Answered(state) => {
                        events.push((CHECKED, json!({ "keyId": key_id, "state": state })));
                    }
                    CheckOutcome::Failed(why) => {
                        log::info!("the weekly license check did not go through: {why}");
                        events.push((CHECK_FAILED, json!({ "keyId": key_id, "problem": why })));
                    }
                }
                self.save(&license);
                events.extend(self.settle(&license, self.now()));
            }
        }
        record_all(ledger, events);
        self.view()
    }

    /// The regular look: try the Vault again if it could not be read, remember the time, check
    /// when a check is due, and follow the edition (Pro ends by the clock when a cancelled
    /// subscription reaches its end, or 30 days pass with no check). True when something changed
    /// that the screen shows.
    pub async fn look(&self, guard: &Guard, ledger: &Ledger) -> bool {
        let read_again = self.read_the_vault_again(ledger);
        let (pc, now) = ((self.clocks.pc)(), self.now());
        let (due, events) = {
            let mut license = lock(&self.license);
            license.saw_clock(now);
            self.save(&license);
            let due = license.check_due(pc);
            let events = if due {
                Vec::new()
            } else {
                self.settle(&license, now).into_iter().collect()
            };
            (due, events)
        };
        if due {
            self.check(guard, ledger).await;
            return true;
        }
        let changed = !events.is_empty();
        record_all(ledger, events);
        changed || read_again
    }

    /// When the key could not be read from the Vault, try again; on success, carry on with the
    /// record kept on disk. True when the key was read now.
    fn read_the_vault_again(&self, ledger: &Ledger) -> bool {
        if lock(&self.unreadable).is_none() {
            return false;
        }
        let Ok(text) = vault::read(self.store.as_ref(), VAULT_ID) else {
            return false;
        };
        let now = self.now();
        let record: Record =
            serde_json::from_slice(&read_record_bytes(self.record_file.as_deref()))
                .unwrap_or_default();
        let events: Vec<Event> = {
            let mut license = lock(&self.license);
            *license = License::load(text.as_deref(), record, now);
            *lock(&self.unreadable) = None;
            self.save(&license);
            self.settle(&license, now).into_iter().collect()
        };
        log::info!("the license key could be read from the Vault again");
        record_all(ledger, events);
        true
    }

    /// Whether `limit` allows the owner's action now, in plain words when it does not.
    pub fn allow(&self, limit: Limit) -> Result<(), CommandError> {
        self.entitlements
            .check(limit)
            .into_result()
            .map_err(|b| CommandError::part_of_pro(b.message))
    }

    /// Follow the license's edition; the event to record when it changed.
    fn settle(&self, license: &License, now: i64) -> Option<Event> {
        let status = license.status(now);
        let before = self.entitlements.edition();
        if !self.entitlements.set_edition(status.edition) {
            return None;
        }
        let reason = LicenseReason::from(status.reason);
        log::info!("Plenipo is now on {:?} ({reason:?})", status.edition);
        Some((
            EDITION_CHANGED,
            json!({ "from": before, "to": status.edition, "reason": reason }),
        ))
    }

    /// Keep the record (when the key could be read, and it changed).
    fn save(&self, license: &License) {
        if lock(&self.unreadable).is_some() {
            return;
        }
        let Some(file) = &self.record_file else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(license.record()) else {
            return;
        };
        let mut saved = lock(&self.saved);
        if *saved == bytes {
            return;
        }
        match write_record(file, &bytes) {
            Ok(()) => *saved = bytes,
            Err(e) => log::warn!("could not keep the license record: {e}"),
        }
    }
}

/// The key kept in the Vault, or why it could not be read (in plain words).
fn read_key(store: &dyn SecretStore) -> (Option<String>, Option<String>) {
    match vault::read(store, VAULT_ID) {
        Ok(text) => (text, None),
        Err(e) => {
            log::warn!("the license key could not be read from the Vault: {e}");
            (
                None,
                Some(format!(
                    "Plenipo couldn't read your license key from {}. It tries again every few \
                     minutes; you can also enter the key again.",
                    store.label()
                )),
            )
        }
    }
}

fn read_record_bytes(file: Option<&Path>) -> Vec<u8> {
    file.and_then(|f| std::fs::read(f).ok()).unwrap_or_default()
}

fn record_all(ledger: &Ledger, events: Vec<Event>) {
    for (event_type, payload) in events {
        record(ledger, event_type, payload);
    }
}

fn write_record(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = file.with_extension("json.tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, file)
}

fn record(ledger: &Ledger, event_type: &str, payload: serde_json::Value) {
    if let Err(e) = ledger.append_event(NewEvent {
        source: PLENIPO.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    }) {
        log::warn!("could not record {event_type}: {e}");
    }
}

/// Tell every window the license changed.
pub fn changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(LICENSE_EVENT, ());
}

/// Check with 8 West now, without waiting (after a key is entered); every window hears when it
/// is done.
pub fn check_soon<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (Some(host), Some(first)) = (
            app.try_state::<Arc<LicenseHost>>()
                .map(|h| h.inner().clone()),
            app.try_state::<Arc<Orgs>>().and_then(|o| o.first()),
        ) else {
            return;
        };
        host.check(&first.guard, &first.ledger).await;
        changed(&app);
    });
}

/// Look now and then for as long as Plenipo runs: a check when one is due (a copy with no key
/// never has one), and the edition when the clock ends Pro.
pub fn start<R: Runtime>(app: &AppHandle<R>, host: Arc<LicenseHost>) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("plenipo-license".into())
        .spawn(move || {
            std::thread::sleep(FIRST_LOOK);
            loop {
                if let Some(first) = app.try_state::<Arc<Orgs>>().and_then(|o| o.first()) {
                    if tauri::async_runtime::block_on(host.look(&first.guard, &first.ledger)) {
                        changed(&app);
                    }
                }
                std::thread::sleep(LOOK_EVERY);
            }
        });
}

/// What is live on the PC now, counted across every organization (ADR-110).
pub struct PcUsage(pub Weak<Orgs>);

impl PcUsage {
    fn sum(&self, count: impl Fn(&Ledger) -> u32) -> u32 {
        self.0.upgrade().map_or(0, |orgs| {
            orgs.stacks()
                .iter()
                .map(|s| count(&s.ledger))
                .fold(0, u32::saturating_add)
        })
    }
}

impl Usage for PcUsage {
    fn organizations(&self) -> u32 {
        self.0.upgrade().map_or(0, |orgs| {
            let live = orgs
                .entries()
                .iter()
                .filter(|e| e.archived_at.is_none())
                .count();
            u32::try_from(live).unwrap_or(u32::MAX)
        })
    }

    fn departments(&self) -> u32 {
        self.sum(|l| l.live_departments().unwrap_or(0))
    }

    fn projects(&self) -> u32 {
        self.sum(|l| l.live_projects().unwrap_or(0))
    }

    fn workers_on_the_job(&self) -> Vec<String> {
        self.0.upgrade().map_or_else(Vec::new, |orgs| {
            orgs.stacks()
                .iter()
                .flat_map(|s| s.ledger.tasks_on_the_job().unwrap_or_default())
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    use plenipo_capabilities::MemorySecretStore;
    use plenipo_licensing::answer::{self, AnswerPayload, SubscriptionState};
    use plenipo_licensing::{trust, Edition};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// The contract's test key (ADR-104): signed by the test key, which only test builds trust.
    fn test_key() -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../contracts/license-check/v1/keys/valid.txt"),
        )
        .unwrap()
        .trim()
        .to_owned()
    }

    fn key_id(text: &str) -> String {
        plenipo_licensing::key::parse(text)
            .unwrap()
            .key_id()
            .to_owned()
    }

    /// The tests' clock (one per test binary: tests that move it run one at a time).
    static NOW: AtomicI64 = AtomicI64::new(1_790_000_000);
    static CLOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn clock() -> i64 {
        NOW.load(Ordering::SeqCst)
    }

    fn set_clock(at: i64) {
        NOW.store(at, Ordering::SeqCst);
    }

    /// The tests' running clock (moves only when a test says so).
    static RUNNING: AtomicI64 = AtomicI64::new(0);

    fn running() -> i64 {
        RUNNING.load(Ordering::SeqCst)
    }

    struct Service {
        port: u16,
        /// Every request's body, as it arrived.
        seen: Arc<Mutex<Vec<Vec<u8>>>>,
        /// What 8 West says, and as of when (`None`: it answers 503).
        says: Arc<Mutex<Option<(SubscriptionState, i64)>>>,
    }

    /// A stand-in for 8 West's check on this computer, signing its answers with the test key.
    async fn service() -> Service {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let says = Arc::new(Mutex::new(Some((SubscriptionState::Active, clock()))));
        let (s, a) = (seen.clone(), says.clone());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let length: usize = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let text = String::from_utf8_lossy(&request).into_owned();
                let body = text
                    .split_once("\r\n\r\n")
                    .map(|(_, b)| b.as_bytes().to_vec())
                    .unwrap_or_default();
                lock(&s).push(body.clone());
                let says = *lock(&a);
                let response = match says {
                    None => "HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\r\n".into(),
                    Some((state, as_of)) => {
                        let asked: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        let payload = AnswerPayload {
                            v: 1,
                            key_id: asked["key_id"].as_str().unwrap().to_owned(),
                            state,
                            paid_through: Some(as_of + 30 * 86_400),
                            ends_at: (state == SubscriptionState::Cancelled)
                                .then_some(as_of + 10 * 86_400),
                            as_of,
                            signer: trust::TEST_KEY_ID.to_owned(),
                        };
                        let answer = String::from_utf8(answer::body(&answer::sign(
                            &payload,
                            &trust::test_signing_key(),
                        )))
                        .unwrap();
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{answer}",
                            answer.len()
                        )
                    }
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        Service { port, seen, says }
    }

    fn host(store: Arc<dyn SecretStore>, file: Option<PathBuf>, port: u16) -> Arc<LicenseHost> {
        LicenseHost::with(
            store,
            file,
            "1.18.0",
            check_for(Some(&format!("http://127.0.0.1:{port}"))),
            Clocks { pc: clock, running },
        )
    }

    fn ledger() -> (Arc<Ledger>, Guard) {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let guard = Guard::new(ledger.clone());
        (ledger, guard)
    }

    fn events(ledger: &Ledger, event_type: &str) -> Vec<serde_json::Value> {
        ledger
            .recent_events(200)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.payload)
            .collect()
    }

    #[test]
    fn a_copy_checks_only_8_wests_one_address_unless_built_for_the_tests() {
        let (address, rules) = check_for(None);
        assert_eq!(address, plenipo_licensing::CHECK_ADDRESS);
        assert_eq!(rules.license_test_port, None);
        // Only a stand-in on this computer, and only over its own port.
        for not_local in ["https://example.com", "http://10.0.0.1:80", ""] {
            let (address, rules) = check_for(Some(not_local));
            assert_eq!(address, plenipo_licensing::CHECK_ADDRESS, "{not_local}");
            assert_eq!(rules.license_test_port, None);
        }
        let (address, rules) = check_for(Some("http://127.0.0.1:8768/"));
        assert_eq!(address, "http://127.0.0.1:8768/v1/check");
        assert_eq!(rules.license_test_port, Some(8768));
    }

    /// Phase 11A's test: a Free copy never contacts 8 West (ADR-115). Its looks and a "Check
    /// now" send nothing, and it stays Free.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_free_copy_never_contacts_8_west() {
        let _one = CLOCK.lock().await;
        let s = service().await;
        let (ledger, guard) = ledger();
        let h = host(Arc::new(MemorySecretStore::default()), None, s.port);
        assert_eq!(h.entitlements().edition(), Edition::Free);
        for _ in 0..3 {
            assert!(!h.look(&guard, &ledger).await);
            let view = h.check(&guard, &ledger).await;
            assert_eq!(view.edition, Edition::Free);
            set_clock(clock() + 8 * 86_400);
        }
        assert!(lock(&s.seen).is_empty(), "nothing was sent");
        assert!(events(&ledger, CHECKED).is_empty());
        assert!(events(&ledger, CHECK_FAILED).is_empty());
    }

    /// Entering a key: Pro at once, kept in the Vault (never in the record or the Ledger), and
    /// the check sends exactly the key's ID and this version.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_key_makes_pro_and_the_check_sends_only_its_id_and_the_version() {
        let _one = CLOCK.lock().await;
        let s = service().await;
        let (ledger, guard) = ledger();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(RECORD_FILE);
        let store = Arc::new(MemorySecretStore::default());
        let h = host(store.clone(), Some(file.clone()), s.port);
        let text = test_key();
        let id = key_id(&text);
        let view = h.enter(&format!("  {text}\n"), &ledger).unwrap();
        assert_eq!(view.edition, Edition::Pro);
        assert_eq!(view.reason, LicenseReason::NotCheckedYet);
        assert_eq!(h.entitlements().edition(), Edition::Pro);
        assert_eq!(
            vault::read(store.as_ref(), VAULT_ID).unwrap(),
            Some(text.clone())
        );
        let view = h.check(&guard, &ledger).await;
        assert_eq!(view.reason, LicenseReason::Active);
        assert_eq!(
            lock(&s.seen).as_slice(),
            [answer::request_body(&id, "1.18.0")],
            "byte for byte"
        );
        // The key is never written anywhere but the Vault.
        let kept = std::fs::read_to_string(&file).unwrap();
        assert!(!kept.contains(&text));
        let recorded = serde_json::to_string(&ledger.recent_events(200).unwrap()).unwrap();
        assert!(!recorded.contains(&text));
        assert_eq!(events(&ledger, KEY_ENTERED), [json!({ "keyId": id })]);
        assert_eq!(
            events(&ledger, CHECKED),
            [json!({ "keyId": id, "state": "active" })]
        );
        // A bad key changes nothing, and is recorded by its reason only.
        let err = h.enter("plenipo1.nope", &ledger).unwrap_err();
        assert_eq!(err.kind, plenipo_core::CommandErrorKind::InvalidInput);
        assert_eq!(h.view().reason, LicenseReason::Active);
        assert_eq!(
            events(&ledger, KEY_REFUSED),
            [json!({ "reason": err.message })]
        );
        let recorded = serde_json::to_string(&ledger.recent_events(200).unwrap()).unwrap();
        assert!(!recorded.contains("plenipo1.nope"));
        // Plenipo starts again: the same license, read from the Vault and the record.
        let again = host(store.clone(), Some(file), s.port);
        assert_eq!(again.view().reason, LicenseReason::Active);
        assert_eq!(again.entitlements().edition(), Edition::Pro);
    }

    /// When 8 West cannot be reached, Pro stays on for 30 days after its last answer, then
    /// Free; a later answer brings Pro back. Winding the clock back never extends the 30 days
    /// (ADR-116). Nothing is deleted.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pro_stays_30_days_without_a_check_and_the_clock_cannot_extend_it() {
        let _one = CLOCK.lock().await;
        let start = clock();
        let s = service().await;
        let (ledger, guard) = ledger();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(RECORD_FILE);
        let store = Arc::new(MemorySecretStore::default());
        let h = host(store.clone(), Some(file.clone()), s.port);
        h.enter(&test_key(), &ledger).unwrap();
        *lock(&s.says) = Some((SubscriptionState::Active, start));
        assert_eq!(h.check(&guard, &ledger).await.reason, LicenseReason::Active);
        // 8 West is down from now on.
        *lock(&s.says) = None;
        set_clock(start + 29 * 86_400);
        assert!(h.look(&guard, &ledger).await);
        let view = h.view();
        assert_eq!(view.edition, Edition::Pro);
        assert!(view.problem.unwrap().contains("503"));
        // A day later, Free.
        set_clock(start + 30 * 86_400 + 1);
        h.look(&guard, &ledger).await;
        assert_eq!(h.entitlements().edition(), Edition::Free);
        assert_eq!(h.view().reason, LicenseReason::NoCheck);
        assert_eq!(events(&ledger, EDITION_CHANGED)[0]["to"], "free");
        // Winding the clock back (and starting again) does not bring Pro back.
        set_clock(start + 5 * 86_400);
        let again = host(store.clone(), Some(file.clone()), s.port);
        assert_eq!(again.entitlements().edition(), Edition::Free);
        assert_eq!(again.view().reason, LicenseReason::NoCheck);
        // 8 West answers again: Pro.
        set_clock(start + 31 * 86_400);
        *lock(&s.says) = Some((SubscriptionState::Active, start + 31 * 86_400));
        assert_eq!(again.check(&guard, &ledger).await.edition, Edition::Pro);
        assert_eq!(again.entitlements().edition(), Edition::Pro);
        set_clock(start);
    }

    /// An ended subscription: Free at once, the key kept; removing the key is Free too, and
    /// takes the key from the Vault.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_ended_subscription_or_a_removed_key_is_free_and_nothing_else_changes() {
        let _one = CLOCK.lock().await;
        let s = service().await;
        let (ledger, guard) = ledger();
        let store = Arc::new(MemorySecretStore::default());
        let h = host(store.clone(), None, s.port);
        h.enter(&test_key(), &ledger).unwrap();
        *lock(&s.says) = Some((SubscriptionState::Ended, clock()));
        let view = h.check(&guard, &ledger).await;
        assert_eq!(view.edition, Edition::Free);
        assert_eq!(view.reason, LicenseReason::Ended);
        assert!(view.key_id.is_some(), "the key is kept");
        let view = h.remove(&ledger).unwrap();
        assert_eq!(view.reason, LicenseReason::NoKey);
        assert_eq!(vault::read(store.as_ref(), VAULT_ID).unwrap(), None);
        assert_eq!(events(&ledger, KEY_REMOVED).len(), 1);
    }

    /// A Vault that cannot be read at start: Free for now, said on screen, and the record is
    /// left whole for the next start.
    #[test]
    fn a_vault_that_cannot_be_read_leaves_the_record_alone() {
        let _one = CLOCK.blocking_lock();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(RECORD_FILE);
        let (ledger, _) = ledger();
        let store = Arc::new(MemorySecretStore::default());
        let h = host(store.clone(), Some(file.clone()), 9);
        h.enter(&test_key(), &ledger).unwrap();
        let kept = std::fs::read(&file).unwrap();
        let broken = Arc::new(Broken);
        let h = host(broken, Some(file.clone()), 9);
        assert_eq!(h.entitlements().edition(), Edition::Free);
        assert!(h.view().problem.unwrap().contains("couldn't read"));
        assert_eq!(std::fs::read(&file).unwrap(), kept);
    }

    /// Review finding: a Vault that could not be read when Plenipo started kept a paying owner
    /// on Free until a restart. Each look now tries it again.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_vault_that_answers_again_brings_the_key_back_at_the_next_look() {
        let _one = CLOCK.lock().await;
        let s = service().await;
        let (ledger, guard) = ledger();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(RECORD_FILE);
        let store = Arc::new(MemorySecretStore::default());
        let h = host(store.clone(), Some(file.clone()), s.port);
        h.enter(&test_key(), &ledger).unwrap();
        assert_eq!(h.check(&guard, &ledger).await.reason, LicenseReason::Active);
        // Plenipo starts while the Vault does not answer.
        let flaky = Arc::new(Flaky {
            inner: store,
            failing: std::sync::atomic::AtomicBool::new(true),
        });
        let again = host(flaky.clone(), Some(file), s.port);
        assert_eq!(again.entitlements().edition(), Edition::Free);
        // It answers again: the next look brings the key back, with its record.
        flaky.failing.store(false, Ordering::SeqCst);
        assert!(again.look(&guard, &ledger).await);
        assert_eq!(again.entitlements().edition(), Edition::Pro);
        assert_eq!(again.view().reason, LicenseReason::Active);
    }

    /// Review finding: a clock held back stopped Plenipo's time instead of being caught. While
    /// Plenipo runs, its time moves on with the running clock.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn holding_the_clock_back_while_plenipo_runs_does_not_stop_its_time() {
        let _one = CLOCK.lock().await;
        let s = service().await;
        let (ledger, guard) = ledger();
        let h = host(Arc::new(MemorySecretStore::default()), None, s.port);
        h.enter(&test_key(), &ledger).unwrap();
        assert_eq!(h.check(&guard, &ledger).await.reason, LicenseReason::Active);
        // 8 West is down; 31 days pass while the PC's clock is held where it was.
        *lock(&s.says) = None;
        RUNNING.store(31 * 86_400, Ordering::SeqCst);
        h.look(&guard, &ledger).await;
        RUNNING.store(0, Ordering::SeqCst);
        assert_eq!(h.entitlements().edition(), Edition::Free);
        assert_eq!(h.view().reason, LicenseReason::NoCheck);
    }

    /// A store that fails until told otherwise.
    struct Flaky {
        inner: Arc<MemorySecretStore>,
        failing: std::sync::atomic::AtomicBool,
    }

    impl SecretStore for Flaky {
        fn label(&self) -> &str {
            "Windows Credential Manager"
        }
        fn check(&self) -> Result<(), String> {
            self.inner.check()
        }
        fn set(&self, id: &str, value: &str) -> Result<(), String> {
            self.inner.set(id, value)
        }
        fn get(&self, id: &str) -> Result<Option<String>, String> {
            if self.failing.load(Ordering::SeqCst) {
                return Err("locked".into());
            }
            self.inner.get(id)
        }
        fn delete(&self, id: &str) -> Result<(), String> {
            self.inner.delete(id)
        }
    }

    struct Broken;

    impl SecretStore for Broken {
        fn label(&self) -> &str {
            "Windows Credential Manager"
        }
        fn check(&self) -> Result<(), String> {
            Err("locked".into())
        }
        fn set(&self, _: &str, _: &str) -> Result<(), String> {
            Err("locked".into())
        }
        fn get(&self, _: &str) -> Result<Option<String>, String> {
            Err("locked".into())
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            Err("locked".into())
        }
    }
}
