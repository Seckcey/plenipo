//! Creates the agent runtime service (Phase 3, ADR-007) for the desktop app: sessions and
//! turns are recorded in the Ledger, updates stream to the UI, and runtimes are detected in
//! the background at startup.

use std::sync::Arc;

use plenipo_ledger::Ledger;
use plenipo_liaison::store::LedgerSessionStore;
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, Bridge, HostEnv,
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
        // An AI tool reported how much of the plan is used (Phase 19, ADR-060 §3): the AI tools
        // page keeps it, then hears of it. Kept off the task's own thread (the Ledger may be
        // busy), and before the page is told, so the page reads it back.
        if let AgentUpdate::Plan(plan) = &update {
            if let Some(tools) = self
                .app
                .try_state::<plenipo_capabilities::ai_tools::AiTools>()
            {
                let tools = tools.inner().clone();
                let app = self.app.clone();
                let plan = plan.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    tools.plan_reported(&plan.runtime_id, plan.report.clone());
                    if let Err(e) = app.emit_to("main", AGENT_EVENT, &AgentUpdate::Plan(plan)) {
                        log::warn!("failed to emit agent update: {e}");
                    }
                });
                return;
            }
        }
        if let Err(e) = self.app.emit_to("main", AGENT_EVENT, &update) {
            log::warn!("failed to emit agent update: {e}");
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
    let mut config = AgentConfig::new(workspace_root);
    // Plenipo itself is the Ollama bridge (ADR-017): `main` runs it before Tauri starts.
    config.bridge = std::env::current_exe().ok().map(|executable| Bridge {
        executable,
        args: vec![plenipo_runtime::agent::ollama::bridge::ARG.into()],
    });
    AgentRuntime::new(
        config,
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
