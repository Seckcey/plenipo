//! Community (Phase 24, ADR-162, ADR-170): the app's side of signing in to your 8 West account.
//!
//! [`plenipo_community::service::Community`] holds the switch, Coming soon, the code, joining,
//! signing out, and leaving. This module gives it the app: Guard's check of every address (the
//! first organization's Guard, with Community's purpose), the Vault (the first organization's,
//! like the license key and the phone's keys), `community.json` for the switch, and the Ledger's
//! shared record (the first organization's, ADR-094 §5).
//!
//! Where the account service is, is built into each copy: `account.getplenipo.com`, or, in copies
//! built for the tests only, a stand-in on this computer (`PLENIPO_COMMUNITY_STAND_IN` at build
//! time). Never a setting, never an environment variable at run time. Nothing here contacts
//! 8 West unless the owner turned Community on (ADR-115, ADR-170 §2).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

use plenipo_capabilities::community_http::CommunityHttp;
use plenipo_capabilities::connections::Opener;
use plenipo_capabilities::vault::{self, SecretStore};
use plenipo_community::client::{Answer, Request, Transport};
use plenipo_community::keys::KEYS_ID;
use plenipo_community::profile::{self, Tile, TileStatus};
use plenipo_community::service::{
    Clock, Community, CommunityView, Recorder, Settings, Stage, Store,
};
use plenipo_guard::{OutboundRules, OWNER, PLENIPO};
use plenipo_ledger::{LedgerEvent, NewEvent};
use plenipo_licensing::Limit;
use serde_json::Value;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::license_host::LicenseHost;
use crate::orgs::{OrgStack, Orgs};

/// Told to every window when Community changes (Settings → Community reads it again).
pub const COMMUNITY_EVENT: &str = "plenipo://community";
/// The switch, in Plenipo's data folder (not secret).
pub const CONFIG_FILE: &str = "community.json";
/// 8 West's account service.
pub const ACCOUNT_ORIGIN: &str = "https://account.getplenipo.com";
/// The published terms of service, opened in the owner's own browser. The Community terms join
/// them once the attorney approves (`docs/legal/phase-24`); the approved text lives there, never
/// in Plenipo (ADR-170 §8).
pub const TERMS_PAGE: &str = "https://getplenipo.com/terms/";
/// The Ledger event of a change to your tile (ADR-056).
const TILE_CHANGED: &str = "owner.profile_changed";
/// How often a closed Community is checked again, while the switch is on (ADR-170 §4).
const CLOSED_LOOK_EVERY: Duration = Duration::from_secs(60 * 60);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where this copy's Community goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The account service's origin.
    pub origin: String,
    /// The rules Guard checks every address against.
    pub rules: OutboundRules,
}

/// This copy's Community, as built.
pub fn built() -> Built {
    built_for(
        option_env!("PLENIPO_COMMUNITY_STAND_IN"),
        plenipo_licensing::trust::built_for_tests(),
    )
}

/// [`built`], from what the build gave: a stand-in only in a copy built for the tests, and only
/// on this computer.
pub fn built_for(stand_in: Option<&str>, for_tests: bool) -> Built {
    match stand_in.filter(|s| for_tests && s.starts_with("http://127.0.0.1:")) {
        Some(stand_in) => Built {
            origin: stand_in.trim_end_matches('/').to_owned(),
            rules: OutboundRules::default().with_community_stand_in(Some(stand_in)),
        },
        None => Built {
            origin: ACCOUNT_ORIGIN.to_owned(),
            rules: OutboundRules::default(),
        },
    }
}

/// Where Community's Guard and Ledger come from: the first organization's (the PC's shared
/// record), found when first needed.
#[derive(Clone)]
pub(crate) struct Sources {
    pub guard: Arc<dyn Fn() -> Option<plenipo_guard::Guard> + Send + Sync>,
    pub ledger: Arc<dyn Fn() -> Option<Arc<plenipo_ledger::Ledger>> + Send + Sync>,
}

impl Sources {
    /// The first organization's, from the app.
    fn first<R: Runtime>(app: &AppHandle<R>) -> Self {
        fn first<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<OrgStack>> {
            app.try_state::<Arc<Orgs>>()
                .and_then(|orgs| orgs.inner().first())
        }
        let (a, b) = (app.clone(), app.clone());
        Self {
            guard: Arc::new(move || first(&a).map(|f| f.guard.clone())),
            ledger: Arc::new(move || first(&b).map(|f| f.ledger.clone())),
        }
    }
}

/// Guard's check, then HTTPS, with the first organization's Guard (made when it first opens).
pub struct HostTransport {
    sources: Sources,
    built: Built,
    http: OnceLock<CommunityHttp>,
}

impl HostTransport {
    fn http(&self) -> Result<&CommunityHttp, String> {
        if let Some(http) = self.http.get() {
            return Ok(http);
        }
        let guard = (self.sources.guard)().ok_or("Plenipo is still starting.")?;
        let http = CommunityHttp::new(guard, self.built.rules.clone(), &self.built.origin)?;
        Ok(self.http.get_or_init(|| http))
    }
}

impl Transport for HostTransport {
    async fn send(&self, request: &Request, pass: Option<&str>) -> Result<Answer, String> {
        let http = self.http()?.clone();
        http.send(request, pass).await
    }
}

/// The Vault, and `community.json`.
struct HostStore {
    vault: Arc<dyn SecretStore>,
    file: Option<PathBuf>,
    memory: Mutex<Option<Settings>>,
}

impl Store for HostStore {
    fn read_pc(&self) -> Result<Option<String>, String> {
        vault::read(self.vault.as_ref(), KEYS_ID).map_err(|e| e.to_string())
    }

    fn write_pc(&self, kept: &str) -> Result<(), String> {
        vault::put(self.vault.as_ref(), KEYS_ID, kept).map_err(|e| e.to_string())
    }

    fn erase_pc(&self) -> Result<(), String> {
        vault::erase(self.vault.as_ref(), KEYS_ID).map_err(|e| e.to_string())
    }

    fn read_settings(&self) -> Result<Option<Settings>, String> {
        match &self.file {
            Some(f) => match std::fs::read_to_string(f) {
                Ok(text) => serde_json::from_str(&text)
                    .map(Some)
                    .map_err(|e| e.to_string()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            },
            None => Ok(lock(&self.memory).clone()),
        }
    }

    fn write_settings(&self, settings: &Settings) -> Result<(), String> {
        match &self.file {
            Some(f) => {
                let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
                let temp = f.with_extension("json.tmp");
                std::fs::write(&temp, text)
                    .and_then(|()| std::fs::rename(&temp, f))
                    .map_err(|e| e.to_string())
            }
            None => {
                *lock(&self.memory) = Some(settings.clone());
                Ok(())
            }
        }
    }
}

/// The Ledger's shared record: the first organization's.
struct HostRecorder {
    sources: Sources,
}

impl Recorder for HostRecorder {
    fn record(&self, event: &str, payload: Value) {
        let Some(ledger) = (self.sources.ledger)() else {
            return;
        };
        // What the owner did is the owner's; a sign-in ended by 8 West is Plenipo's to tell.
        let source = if event == "community.signed_out" && payload["why"] == "removed" {
            PLENIPO
        } else {
            OWNER
        };
        if let Err(e) = ledger.append_event(NewEvent {
            task_id: None,
            execution_id: None,
            source: source.into(),
            destination: None,
            event_type: event.into(),
            payload,
        }) {
            log::warn!("Community could not record {event}: {e}");
        }
    }
}

/// Your tile (ADR-056), as Community uses it: from the first organization's Ledger, where it is
/// kept. An empty tile before that organization opens.
fn tile_of(ledger: Option<&plenipo_ledger::Ledger>) -> Tile {
    let kept = ledger
        .and_then(|l| plenipo_workforce::owner::profile(l).ok())
        .unwrap_or_default();
    let word = |value: serde_json::Result<Value>| {
        value
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    };
    Tile {
        status: TileStatus::from_word(&word(serde_json::to_value(kept.status))),
        mood: kept
            .mood
            .map(|m| word(serde_json::to_value(m)))
            .filter(|w| !w.is_empty()),
        message: kept.message,
        picture: kept
            .picture
            .as_deref()
            .and_then(profile::picture_from_base64),
    }
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Community, as the app holds it (app state).
pub struct CommunityState {
    pub community: Arc<Community<HostTransport>>,
    pub built: Built,
    sources: Sources,
    license: Arc<LicenseHost>,
    /// Opens the account site's sign-in page and the terms in the owner's own browser.
    pub opener: Arc<dyn Opener>,
    /// The sign-in's asking, while it runs.
    asking: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl CommunityState {
    /// The account site's page where the owner types the code (contract §2).
    pub fn sign_in_page(&self) -> String {
        if self.built.origin == ACCOUNT_ORIGIN {
            plenipo_community::session::CONNECT_PAGE.to_owned()
        } else {
            format!("{}/community/connect", self.built.origin)
        }
    }

    /// The first organization's Ledger, where Community's messages are kept (ADR-164 §10).
    pub fn ledger(&self) -> Option<Arc<plenipo_ledger::Ledger>> {
        (self.sources.ledger)()
    }

    /// Your tile, as it is now.
    pub fn tile(&self) -> Tile {
        tile_of((self.sources.ledger)().as_deref())
    }

    /// Settings → Community, as the screen shows it.
    pub fn view(&self) -> CommunityView {
        let pro = self
            .license
            .entitlements()
            .check(Limit::CommunityStart)
            .is_allowed();
        self.community.view(pro)
    }
}

/// Make Community, with what was kept: the switch in `community.json`, the signed-in PC in the
/// first organization's Vault (`vault`). `data`: Plenipo's data folder (`None`: keep it in
/// memory). Sends nothing.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    license: Arc<LicenseHost>,
    opener: Arc<dyn Opener>,
    vault: Arc<dyn SecretStore>,
    data: Option<PathBuf>,
    version: &str,
    built: Built,
) -> Arc<CommunityState> {
    create_with(
        Sources::first(app),
        license,
        opener,
        vault,
        data,
        version,
        built,
    )
}

/// [`create`], with where Guard and the Ledger come from given (the tests').
pub(crate) fn create_with(
    sources: Sources,
    license: Arc<LicenseHost>,
    opener: Arc<dyn Opener>,
    vault: Arc<dyn SecretStore>,
    data: Option<PathBuf>,
    version: &str,
    built: Built,
) -> Arc<CommunityState> {
    let community = Community::load(
        HostTransport {
            sources: sources.clone(),
            built: built.clone(),
            http: OnceLock::new(),
        },
        Arc::new(HostStore {
            vault,
            file: data.as_ref().map(|d| d.join(CONFIG_FILE)),
            memory: Mutex::new(None),
        }),
        Arc::new(HostRecorder {
            sources: sources.clone(),
        }),
        Arc::new(SystemClock),
        version,
        &crate::remote_host::pc_name(),
    );
    Arc::new(CommunityState {
        community: Arc::new(community),
        built,
        sources,
        license,
        opener,
        asking: Mutex::new(None),
    })
}

/// Tell every window that Community changed.
pub fn changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(COMMUNITY_EVENT, "changed");
}

/// Send your tile again whenever it changes (ADR-163 §1). [`Community::tile_changed`] sends
/// nothing unless this PC is signed in with a profile saved, and nothing while you appear offline.
fn send_the_tile_when_it_changes(state: &Arc<CommunityState>) {
    let Some(ledger) = (state.sources.ledger)() else {
        return;
    };
    let state = Arc::downgrade(state);
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if event.event_type != TILE_CHANGED {
            return;
        }
        let Some(state) = state.upgrade() else {
            return;
        };
        tauri::async_runtime::spawn(async move {
            let tile = state.tile();
            state.community.tile_changed(&tile).await;
        });
    }));
}

/// Once the app is up: if this PC is signed in, ask who it is and send your tile, and look again
/// now and then while 8 West has Community closed. A PC that is not signed in asks nothing
/// (ADR-115, ADR-170 §2).
pub fn start<R: Runtime>(app: &AppHandle<R>, state: Arc<CommunityState>) {
    send_the_tile_when_it_changes(&state);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if state.community.signed_in_pc().is_some() {
            state.community.refresh().await;
            // Your tile may have changed while 8 West could not be reached.
            state.community.tile_changed(&state.tile()).await;
            changed(&app);
        }
        loop {
            tokio::time::sleep(CLOSED_LOOK_EVERY).await;
            if state.view().stage == Stage::Closed {
                state.community.refresh().await;
                changed(&app);
            }
        }
    });
}

/// While a code is shown: ask whether the owner pressed **Allow** yet, at the pace the service
/// asks for, until signing in is over. One asking at a time.
pub fn ask_until_signed_in<R: Runtime>(app: &AppHandle<R>, state: &Arc<CommunityState>) {
    let mut asking = lock(&state.asking);
    if asking.as_ref().is_some_and(|h| !h.inner().is_finished()) {
        return;
    }
    let app = app.clone();
    let state = state.clone();
    *asking = Some(tauri::async_runtime::spawn(async move {
        let mut wait = 5u64;
        loop {
            tokio::time::sleep(Duration::from_secs(wait)).await;
            match state.community.poll().await {
                Some(next) => wait = u64::from(next),
                None => {
                    changed(&app);
                    break;
                }
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_capabilities::MemorySecretStore;
    use plenipo_community::stand_in::{Openness, StandIn};
    use std::future::Future;
    use std::pin::Pin;

    /// No browser opens in the tests: the address is kept instead.
    #[derive(Default)]
    struct Kept(Mutex<Vec<String>>);

    impl Opener for Kept {
        fn open(
            &self,
            address: String,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
            lock(&self.0).push(address);
            Box::pin(async { Ok(()) })
        }
    }

    fn license() -> Arc<LicenseHost> {
        crate::license_host::LicenseHost::with(
            Arc::new(MemorySecretStore::default()),
            None,
            "1.9.0",
            crate::license_host::check_for(Some("http://127.0.0.1:9")),
            crate::license_host::Clocks::REAL,
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_copy_that_never_turns_community_on_never_contacts_8_west() {
        let service = StandIn::new();
        let origin = service.serve().await;
        let app = tauri::test::mock_app();
        let opener = Arc::new(Kept::default());
        let ledger = Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap());
        let guard = plenipo_guard::Guard::new(ledger.clone());
        let state = create_with(
            Sources {
                guard: Arc::new(move || Some(guard.clone())),
                ledger: Arc::new(move || Some(ledger.clone())),
            },
            license(),
            opener.clone(),
            Arc::new(MemorySecretStore::default()),
            None,
            "1.9.0",
            built_for(Some(&origin), true),
        );
        start(app.handle(), state.clone());
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(state.view().stage, Stage::Off);
        assert!(service.seen().is_empty(), "nothing reached 8 West");

        // Pressing the switch asks "is it open?": Coming soon while 8 West has it closed.
        service.set_open(Openness::Closed);
        state.community.turn_on().await.unwrap();
        assert_eq!(state.view().stage, Stage::ComingSoon);
        let seen = service.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, "/v1/community/open");

        // Open: the same copy signs in, and the sign-in page is the stand-in's own.
        service.set_open(Openness::Open {
            links: false,
            collaborators: false,
        });
        state.community.turn_on().await.unwrap();
        assert_eq!(state.view().stage, Stage::SigningIn);
        assert!(state.view().code.is_some());
        state.opener.open(state.sign_in_page()).await.unwrap();
        assert_eq!(
            lock(&opener.0).as_slice(),
            [format!("{origin}/community/connect")]
        );
    }

    #[test]
    fn a_stand_in_is_only_for_copies_built_for_the_tests_and_only_on_this_computer() {
        assert_eq!(built_for(None, true).origin, ACCOUNT_ORIGIN);
        assert_eq!(
            built_for(Some("http://127.0.0.1:8790"), false).origin,
            ACCOUNT_ORIGIN
        );
        assert_eq!(
            built_for(Some("https://evil.example"), true).origin,
            ACCOUNT_ORIGIN
        );
        let stand_in = built_for(Some("http://127.0.0.1:8790"), true);
        assert_eq!(stand_in.origin, "http://127.0.0.1:8790");
        assert_eq!(stand_in.rules.community_test_port, Some(8790));
        // A release is built without one.
        assert_eq!(
            built(),
            built_for(
                option_env!("PLENIPO_COMMUNITY_STAND_IN"),
                plenipo_licensing::trust::built_for_tests()
            )
        );
    }
}
