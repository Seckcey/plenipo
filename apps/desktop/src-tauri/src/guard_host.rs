//! Creates Plenipo Guard and the capability broker (Phase 7, ADR-013) for the desktop app:
//! workers of the organization get Plenipo's tools through the broker, the agent runtime hides
//! secrets with the broker's filter, and secret values live in the operating system's
//! protected storage (Windows Credential Manager) under the app's identifier.

use std::sync::Arc;

use plenipo_capabilities::{Broker, BrokerConfig, MemorySecretStore, OsSecretStore, SecretStore};
use plenipo_guard::Guard;
use plenipo_ledger::Ledger;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use tauri::{AppHandle, Manager as _, Runtime};

use crate::runtime_host::Persistence;

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
    let (store, tickets): (Arc<dyn SecretStore>, _) = match persistence {
        Persistence::AppData => (
            Arc::new(OsSecretStore::new(app.config().identifier.clone())),
            app.path()
                .app_local_data_dir()
                .map(|d| d.join("runtime").join("tool-tickets"))
                .unwrap_or_else(|_| std::env::temp_dir().join("plenipo-tool-tickets")),
        ),
        Persistence::InMemory => (
            Arc::new(MemorySecretStore::default()),
            std::env::temp_dir().join(format!("plenipo-tool-tickets-{}", std::process::id())),
        ),
    };
    // The AI tools start Plenipo itself as the relay (`--plenipo-tools=<ticket>`).
    let relay = std::env::current_exe().unwrap_or_default();
    let broker = Broker::new(
        guard.clone(),
        supervisor,
        store,
        BrokerConfig::new(relay, tickets),
    );
    agents.set_tools(Arc::new(broker.clone()));
    agents.set_filter(broker.text_filter());
    (guard, broker)
}

/// Open the tool server on the loopback address. Without it workers get no tools (and a
/// notice says so).
pub fn start(broker: &Broker) {
    if let Err(e) = tauri::async_runtime::block_on(broker.start()) {
        eprintln!("[plenipo] tool server unavailable: {e}");
    }
}
