//! The AI tools page's service in the desktop app (Phase 19, ADR-058 to ADR-060): created with
//! the other services, told when an AI tool reports its plan and when a task ends, and woken
//! every hour to look for new versions (at most once a day, only while Plenipo runs: it lives in
//! the tray, ADR-037).

use std::sync::Arc;
use std::time::Duration;

use plenipo_capabilities::ai_tools::{AiTools, UpdateBy};
use plenipo_capabilities::Broker;
use plenipo_guard::OutboundRules;
use plenipo_ledger::Ledger;
use plenipo_runtime::agent::AgentRuntime;

/// The first look, a few minutes after Plenipo starts (after its own update check).
const FIRST_LOOK: Duration = Duration::from_secs(4 * 60);
/// Then every hour; the look itself happens at most once a day.
const EVERY: Duration = Duration::from_secs(60 * 60);

/// A stand-in for the AI tools' release lists on this computer, built into copies of Plenipo
/// made for the end-to-end tests only (never a setting, never an environment variable at run
/// time); Guard allows only `http://127.0.0.1:<port>`.
pub fn built_release_base() -> Option<String> {
    option_env!("PLENIPO_AI_TOOL_RELEASES")
        .map(str::trim)
        .filter(|b| b.starts_with("http://127.0.0.1:"))
        .map(str::to_owned)
}

/// Build the service and bring back what it kept (the models each AI tool reported, and the
/// tools given no tasks).
pub fn create(agents: &AgentRuntime, broker: &Broker) -> AiTools {
    let tools = AiTools::new(
        agents.clone(),
        broker.clone(),
        OutboundRules::default(),
        built_release_base(),
    );
    tools.restore();
    tools
}

/// A task ended on an AI tool: one that reports its plan through its check is asked again
/// (at most every five minutes).
pub fn listen(ledger: &Arc<Ledger>, tools: &AiTools) {
    let tools = tools.clone();
    ledger.add_listener(Arc::new(move |event| {
        if event.event_type == "agent.result" {
            if let Some(runtime) = event.source.strip_prefix("agent:") {
                tools.task_ended(runtime);
            }
        }
    }));
}

/// The daily look for new versions, by itself.
pub fn start_daily(tools: AiTools) {
    let _ = std::thread::Builder::new()
        .name("plenipo-ai-tools-daily".into())
        .spawn(move || {
            std::thread::sleep(FIRST_LOOK);
            // When Plenipo starts, each AI tool's models (ADR-060 §5), unless the daily look
            // just asked for them.
            if !tauri::async_runtime::block_on(tools.daily()) {
                tauri::async_runtime::block_on(tools.check_models());
            }
            loop {
                std::thread::sleep(EVERY);
                tauri::async_runtime::block_on(tools.daily());
            }
        });
}

/// Look now, for the owner (the page's button).
pub async fn look_now(tools: &AiTools) -> plenipo_capabilities::ai_tools::AiToolsPage {
    tools.look_for_new_versions(UpdateBy::Owner).await
}
