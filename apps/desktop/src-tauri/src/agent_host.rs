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
use tauri::{AppHandle, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying [`AgentUpdate`] payloads to their organization's window.
pub const AGENT_EVENT: &str = "plenipo://agents";

struct TauriAgentSink<R: Runtime> {
    app: AppHandle<R>,
    /// The organization whose AI tools' sessions these are (Phase 21).
    org: String,
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
                let org = self.org.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    tools.plan_reported(&plan.runtime_id, plan.report.clone());
                    crate::orgs::emit_to_org(&app, &org, AGENT_EVENT, &AgentUpdate::Plan(plan));
                });
                return;
            }
        }
        crate::orgs::emit_to_org(&self.app, &self.org, AGENT_EVENT, &update);
    }
}

/// Build the agent runtime. Never fails; with `InMemory` persistence (tests) it sees no
/// installed runtimes, so tests never start real CLIs.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    org: &crate::orgs::OrgPlace,
    persistence: Persistence,
    ledger: Arc<Ledger>,
    supervisor: Supervisor,
) -> AgentRuntime {
    let (workspace_root, host) = match persistence {
        Persistence::AppData => (
            org.folder
                .as_deref()
                .map(|d| d.join("runtime").join("agent-workspaces"))
                .unwrap_or_else(|| std::env::temp_dir().join("plenipo-agent-workspaces")),
            HostEnv::current(),
        ),
        Persistence::InMemory => (
            std::env::temp_dir().join("plenipo-agent-workspaces"),
            HostEnv::new(None, None, None),
        ),
    };
    let mut config = AgentConfig::new(workspace_root);
    // AI tools with a settings folder of their own keep it beside the workspaces (ADR-082);
    // a folder only Plenipo uses, even when the app data folder is not found.
    if config.tool_homes.parent() == Some(std::env::temp_dir().as_path()) {
        config.tool_homes = std::env::temp_dir().join("plenipo-ai-tool-homes");
    }
    // Their sign-ins belong to the PC (ADR-094 §4): every organization's AI tools use the one
    // settings folder, the first organization's.
    if let (Persistence::AppData, Some(data)) = (persistence, org.data.as_deref()) {
        config.tool_homes = data.join("runtime").join("ai-tool-homes");
    }
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
        Arc::new(TauriAgentSink {
            app: app.clone(),
            org: org.id.clone(),
        }),
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
