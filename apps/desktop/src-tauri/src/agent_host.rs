//! Creates the agent runtime service (Phase 3, ADR-007) for the desktop app: sessions and
//! turns are recorded in the Ledger, updates stream to the UI, and runtimes are detected in
//! the background at startup.

use std::sync::Arc;

use plenipo_ledger::Ledger;
use plenipo_liaison::store::LedgerSessionStore;
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, HostEnv,
};
use plenipo_runtime::Supervisor;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying [`AgentUpdate`] payloads to the main window.
pub const AGENT_EVENT: &str = "plenipo://agents";

struct TauriAgentSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AgentSink for TauriAgentSink<R> {
    fn emit(&self, update: AgentUpdate) {
        if let Err(e) = self.app.emit_to("main", AGENT_EVENT, &update) {
            eprintln!("[plenipo] failed to emit agent update: {e}");
        }
    }
}

/// Build the agent runtime. Never fails; with `InMemory` persistence (tests) it sees no
/// installed runtimes, so tests never start real CLIs.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    persistence: Persistence,
    ledger: Arc<Ledger>,
    supervisor: Supervisor,
) -> AgentRuntime {
    let (workspace_root, host) = match persistence {
        Persistence::AppData => (
            app.path()
                .app_local_data_dir()
                .map(|d| d.join("runtime").join("agent-workspaces"))
                .unwrap_or_else(|_| std::env::temp_dir().join("plenipo-agent-workspaces")),
            HostEnv::current(),
        ),
        Persistence::InMemory => (
            std::env::temp_dir().join("plenipo-agent-workspaces"),
            HostEnv::new(None, None, None),
        ),
    };
    AgentRuntime::new(
        AgentConfig::new(workspace_root),
        builtin_adapters(),
        supervisor,
        Arc::new(LedgerSessionStore(ledger)),
        Arc::new(TauriAgentSink { app: app.clone() }),
        host,
    )
}

/// Detect runtimes without delaying startup.
pub fn detect_in_background(runtime: &AgentRuntime) {
    let runtime = runtime.clone();
    tauri::async_runtime::spawn(async move {
        runtime.refresh().await;
    });
}
