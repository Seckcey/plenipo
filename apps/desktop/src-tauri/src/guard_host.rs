//! Creates Plenipo Guard and the capability broker (Phase 7, ADR-013) for the desktop app:
//! workers of the organization get Plenipo's tools through the broker, the agent runtime hides
//! secrets with the broker's filter, and secret values live in the operating system's
//! protected storage (Windows Credential Manager) under the app's identifier.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use plenipo_capabilities::browser::BrowserConfig;
use plenipo_capabilities::connections::ConnectionsConfig;
use plenipo_capabilities::control::{ControlCenter, ControlStatus};
use plenipo_capabilities::watch::WatchUpdate;
use plenipo_capabilities::{Broker, BrokerConfig, MemorySecretStore, OsSecretStore, SecretStore};
use plenipo_guard::Guard;
use plenipo_ledger::Ledger;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying [`ControlStatus`] to every window (Phase 10).
pub const CONTROL_EVENT: &str = "plenipo://control";
/// At most this many windows' pages hear Watch at once; a page that reloaded without saying
/// goodbye is the oldest, and goes first.
pub const MAX_WATCH_SUBSCRIBERS: usize = 32;

/// Who hears Watch's updates (Phase 18, ADR-055): each is a channel the main window opened with
/// `subscribe_watch`, which no other window or web page may call. Not an event: a page listening
/// to every event would hear an event sent to the main window too.
#[derive(Clone, Default)]
pub struct WatchSubscribers(Arc<Mutex<Subscribers>>);

#[derive(Default)]
struct Subscribers {
    next: u32,
    channels: BTreeMap<u32, Channel<WatchUpdate>>,
}

impl WatchSubscribers {
    fn lock(&self) -> std::sync::MutexGuard<'_, Subscribers> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Send Watch's updates to `channel` from now on; returns the number that stops them.
    pub fn add(&self, channel: Channel<WatchUpdate>) -> u32 {
        let mut s = self.lock();
        s.next = s.next.wrapping_add(1);
        let id = s.next;
        s.channels.insert(id, channel);
        while s.channels.len() > MAX_WATCH_SUBSCRIBERS {
            s.channels.pop_first();
        }
        id
    }

    /// Stop sending to the channel `id` names; whether it was there.
    pub fn remove(&self, id: u32) -> bool {
        self.lock().channels.remove(&id).is_some()
    }

    /// How many channels hear Watch now.
    pub fn len(&self) -> usize {
        self.lock().channels.len()
    }

    /// Whether no channel hears Watch now.
    pub fn is_empty(&self) -> bool {
        self.lock().channels.is_empty()
    }

    /// Send `update` to every channel; one that cannot take it any more is forgotten.
    pub fn send(&self, update: &WatchUpdate) {
        let mut s = self.lock();
        s.channels
            .retain(|_, channel| match channel.send(update.clone()) {
                Ok(()) => true,
                Err(e) => {
                    log::warn!("a window stopped hearing Watch: {e}");
                    false
                }
            });
    }
}

/// Create Guard and the broker, and give the agent runtime its tools and secret filter.
/// Never fails; problems become notices on the Permissions page.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    org: &crate::orgs::OrgPlace,
    persistence: Persistence,
    ledger: Arc<Ledger>,
    supervisor: Supervisor,
    agents: &AgentRuntime,
    control: ControlCenter,
) -> (Guard, Broker, WatchSubscribers) {
    let guard = Guard::new(ledger);
    let (store, tickets, data, files): (Arc<dyn SecretStore>, _, _, _) = match persistence {
        Persistence::AppData => {
            let data = org
                .folder
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join("plenipo"));
            // Plenipo's own folder in Documents, where the owner looks for files (ADR-201).
            let files = app
                .path()
                .document_dir()
                .map(|documents| documents.join("Plenipo"))
                .unwrap_or_else(|_| data.join("files"));
            (
                // The organization's secrets under its own name (ADR-094 §9).
                Arc::new(OsSecretStore::new(org.vault.clone())),
                data.join("runtime").join("tool-tickets"),
                data,
                files,
            )
        }
        Persistence::InMemory => {
            // Each organization's own folder, so two never share tickets or working copies.
            let temp = std::env::temp_dir().join(format!(
                "plenipo-{}{}",
                std::process::id(),
                if org.is_first() {
                    String::new()
                } else {
                    format!("-{}", org.id)
                }
            ));
            (
                Arc::new(MemorySecretStore::default()),
                temp.join("tool-tickets"),
                temp.clone(),
                temp.join("files"),
            )
        }
    };
    // The AI tools start Plenipo itself as the relay (`--plenipo-tools=<ticket>`).
    let relay = std::env::current_exe().unwrap_or_default();
    let mut config = BrokerConfig::new(relay, tickets);
    // Each objective's branch and working copy (Phase 8, ADR-016).
    config.workspaces_dir = data.join("working-copies");
    // Copies of the files the owner put on objectives (Phase 21, ADR-093 §21).
    config.attachments_dir = data.join("attachments");
    // Plenipo's browser's own profile, and the screenshots kept as evidence (Phase 10).
    config.browser = BrowserConfig::new(data.join("browser-profile"));
    config.screenshots_dir = data.join("screenshots");
    // Work that belongs to no project folder: its scratch pad in the organization folder
    // (ADR-205), or `<Documents>/Plenipo/<organization>/<position>` for an organization without
    // one (ADR-201), so a worker can always save its files and the owner can find them.
    config.files_dir = Some(files);
    // The system's own folders, above which a junction is not looked at when a step's folder in
    // the organization folder is checked (ADR-205), as the folder keeper does.
    config.trusted_places = crate::folder_commands::trusted(app);
    // Connections (Phase 20): the app ID this copy signs in to Microsoft 365 with, and the
    // stand-in for the services in copies built for the end-to-end tests.
    config.connections = connections_config();
    // The app's own tests reach the shell even on test machines that run everything as
    // administrator (GitHub's Windows machines do); Plenipo itself always refuses then.
    #[cfg(test)]
    {
        config.terminal_refuses_administrator = false;
    }
    let broker = Broker::sharing_control(guard.clone(), supervisor, store, config, control);
    agents.set_tools(Arc::new(broker.clone()));
    agents.set_filter(broker.text_filter());
    // Paid AI keys and the spending caps (Phase 16 Wave 3, ADR-085), before anything below
    // returns: every organization's AI tools read its own keys and caps.
    agents.set_paid_gate(plenipo_capabilities::paid::gate(&broker));
    // Watch (Phase 18, ADR-055): the file changes a worker makes, to its organization's window's
    // own channels only — never another organization's, the sign window, or a web page.
    let subscribers = WatchSubscribers::default();
    let hearing = subscribers.clone();
    broker
        .watch()
        .set_listener(Arc::new(move |update: &WatchUpdate| hearing.send(update)));
    // The record of who uses the browser and the screen is the PC's: the first organization's
    // broker tells the windows, the tray, and the sign for every organization.
    if !org.is_first() {
        return (guard, broker, subscribers);
    }
    // Who uses the browser or the mouse and keyboard: every window, the tray, and the sign
    // above all windows while a worker uses the mouse and keyboard (Phase 10).
    // The tray and the sign are updated off this thread, always to the newest status, so a
    // slower update never puts back an older one (the listener hears changes in order).
    let handle = app.clone();
    let latest = Arc::new(std::sync::Mutex::new(ControlStatus::default()));
    broker.set_control_listener(Arc::new(move |status: &ControlStatus| {
        if let Err(e) = handle.emit(CONTROL_EVENT, status) {
            log::warn!("failed to emit control event: {e}");
        }
        *latest.lock().unwrap_or_else(|p| p.into_inner()) = status.clone();
        let (app, latest) = (handle.clone(), Arc::clone(&latest));
        tauri::async_runtime::spawn(async move {
            let status = latest.lock().unwrap_or_else(|p| p.into_inner()).clone();
            crate::tray::show_control(&app, &status);
            crate::indicator::update(&app, &status);
        });
    }));
    (guard, broker, subscribers)
}

/// How this copy of Plenipo connects (ADR-065 §6, ADR-070 §3): 8 West's Microsoft app ID and
/// Slack app's client ID (public, not secrets), and, in copies built for the end-to-end tests
/// only, the stand-in for the services on this computer. All are built in by the build
/// (`PLENIPO_MICROSOFT_APP_ID`, `PLENIPO_SLACK_CLIENT_ID`, `PLENIPO_CONNECTIONS_STAND_IN`): never a
/// setting, never an environment variable at run time. A copy built without one shows that
/// service as not ready to sign in (an organization or workspace can still use its own app,
/// Advanced). Google has no 8 West app: the owner saves their own (ADR-070 §4).
pub fn connections_config() -> ConnectionsConfig {
    ConnectionsConfig {
        microsoft_app_id: option_env!("PLENIPO_MICROSOFT_APP_ID")
            .map(str::trim)
            .filter(|id| plenipo_guard::connections::is_guid(id))
            .map(str::to_lowercase),
        slack_client_id: option_env!("PLENIPO_SLACK_CLIENT_ID")
            .map(str::trim)
            .filter(|id| plenipo_guard::connections::is_slack_client_id(id))
            .map(str::to_owned),
        // Slack's own fixed sign-in ports (ADR-070 §5.2).
        slack_ports: None,
        stand_in: option_env!("PLENIPO_CONNECTIONS_STAND_IN")
            .map(str::trim)
            .filter(|b| b.starts_with("http://127.0.0.1:"))
            .map(str::to_owned),
    }
}

/// Open the tool server on the loopback address. Without it workers get no tools (and a
/// notice says so).
pub fn start(broker: &Broker) {
    if let Err(e) = tauri::async_runtime::block_on(broker.start()) {
        log::warn!("tool server unavailable: {e}");
    }
}
