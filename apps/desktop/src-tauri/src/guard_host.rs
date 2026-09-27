//! Creates Plenipo Guard and the capability broker (Phase 7, ADR-013) for the desktop app:
//! workers of the organization get Plenipo's tools through the broker, the agent runtime hides
//! secrets with the broker's filter, and secret values live in the operating system's
//! protected storage (Windows Credential Manager) under the app's identifier.

use std::sync::Arc;

use plenipo_capabilities::browser::BrowserConfig;
use plenipo_capabilities::control::ControlStatus;
use plenipo_capabilities::{Broker, BrokerConfig, MemorySecretStore, OsSecretStore, SecretStore};
use plenipo_guard::Guard;
use plenipo_ledger::Ledger;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying [`ControlStatus`] to every window (Phase 10).
pub const CONTROL_EVENT: &str = "plenipo://control";

/// Create Guard and the broker, and give the agent runtime its tools and secret filter.
/// Never fails; problems become notices on the Permissions page.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    persistence: Persistence,
    ledger: Arc<Ledger>,
    supervisor: Supervisor,
    agents: &AgentRuntime,
) -> (Guard, Broker) {
    let guard = Guard::new(ledger);
    let (store, tickets, data): (Arc<dyn SecretStore>, _, _) = match persistence {
        Persistence::AppData => {
            let data = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("plenipo"));
            (
                Arc::new(OsSecretStore::new(app.config().identifier.clone())),
                data.join("runtime").join("tool-tickets"),
                data,
            )
        }
        Persistence::InMemory => {
            let temp = std::env::temp_dir().join(format!("plenipo-{}", std::process::id()));
            (
                Arc::new(MemorySecretStore::default()),
                temp.join("tool-tickets"),
                temp,
            )
        }
    };
    // The AI tools start Plenipo itself as the relay (`--plenipo-tools=<ticket>`).
    let relay = std::env::current_exe().unwrap_or_default();
    let mut config = BrokerConfig::new(relay, tickets);
    // Each objective's branch and working copy (Phase 8, ADR-016).
    config.workspaces_dir = data.join("working-copies");
    // Plenipo's browser's own profile, and the screenshots kept as evidence (Phase 10).
    config.browser = BrowserConfig::new(data.join("browser-profile"));
    config.screenshots_dir = data.join("screenshots");
    // The app's own tests reach the shell even on test machines that run everything as
    // administrator (GitHub's Windows machines do); Plenipo itself always refuses then.
    #[cfg(test)]
    {
        config.terminal_refuses_administrator = false;
    }
    let broker = Broker::new(guard.clone(), supervisor, store, config);
    agents.set_tools(Arc::new(broker.clone()));
    agents.set_filter(broker.text_filter());
    // Who uses the browser or the mouse and keyboard: every window, the tray, and the sign
    // above all windows while a worker uses the mouse and keyboard (Phase 10).
    // The tray and the sign are updated off this thread, always to the newest status, so a
    // slower update never puts back an older one (the listener hears changes in order).
    let handle = app.clone();
    let latest = Arc::new(std::sync::Mutex::new(ControlStatus::default()));
    broker.set_control_listener(Arc::new(move |status: &ControlStatus| {
        if let Err(e) = handle.emit(CONTROL_EVENT, status) {
            eprintln!("[plenipo] failed to emit control event: {e}");
        }
        *latest.lock().unwrap_or_else(|p| p.into_inner()) = status.clone();
        let (app, latest) = (handle.clone(), Arc::clone(&latest));
        tauri::async_runtime::spawn(async move {
            let status = latest.lock().unwrap_or_else(|p| p.into_inner()).clone();
            crate::tray::show_control(&app, &status);
            crate::indicator::update(&app, &status);
        });
    }));
    (guard, broker)
}

/// Open the tool server on the loopback address. Without it workers get no tools (and a
/// notice says so).
pub fn start(broker: &Broker) {
    if let Err(e) = tauri::async_runtime::block_on(broker.start()) {
        eprintln!("[plenipo] tool server unavailable: {e}");
    }
}
