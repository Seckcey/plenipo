//! Phase 19 commands: the AI tools page — each tool's sign-in (through `open_terminal`), usage,
//! how it is paid for, its version and updates, and its models (ADR-058 to ADR-060). Every one is
//! the main window's alone (capabilities/default.json): the sign window and web pages are
//! refused.
//!
//! Nothing here takes a program, an argument, a path, or an address: only an AI tool's ID from a
//! fixed list and a choice. The programs they start (a sign-in tab, an update, a check) are each
//! tool's own, from its adapter, decided by Guard and run through the supervisor; the release
//! lists they read are fixed and go through Guard's gate.

use plenipo_capabilities::ai_tools::{
    AiToolUsage, AiTools, AiToolsPage, PaymentMethod, UpdateBy, MAX_USAGE_DAYS,
};
use plenipo_core::CommandError;
use tauri::State;

use crate::commands::{broker_error, validate_runtime_id};

/// The AI tools page's own part: each tool's newest version, update, plan, and payment.
#[tauri::command]
pub fn get_ai_tools(tools: State<'_, AiTools>) -> AiToolsPage {
    tools.page()
}

/// Check one AI tool again: its version, sign-in, models, and (where it reports it) plan.
#[tauri::command]
pub async fn check_ai_tool(
    tools: State<'_, AiTools>,
    runtime_id: String,
) -> Result<AiToolsPage, CommandError> {
    validate_runtime_id(&runtime_id)?;
    tools.check(&runtime_id).await.map_err(broker_error)
}

/// Look for each AI tool's newest version now.
#[tauri::command]
pub async fn check_ai_tool_versions(
    tools: State<'_, AiTools>,
) -> Result<AiToolsPage, CommandError> {
    Ok(crate::ai_tools_host::look_now(&tools).await)
}

/// An AI tool's usage for the days starting at `day_starts` (the last value ends the last day).
#[tauri::command]
pub async fn get_ai_tool_usage(
    tools: State<'_, AiTools>,
    runtime_id: String,
    day_starts: Vec<u64>,
) -> Result<AiToolUsage, CommandError> {
    validate_runtime_id(&runtime_id)?;
    if day_starts.len() > MAX_USAGE_DAYS + 1 {
        return Err(CommandError::invalid_input(format!(
            "usage is shown for at most {MAX_USAGE_DAYS} days"
        )));
    }
    let tools = tools.inner().clone();
    tauri::async_runtime::spawn_blocking(move || tools.usage(&runtime_id, &day_starts))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
        .map_err(broker_error)
}

/// Update an AI tool with its own update command: it waits while a task is using the tool.
#[tauri::command]
pub async fn update_ai_tool(
    tools: State<'_, AiTools>,
    runtime_id: String,
) -> Result<AiToolsPage, CommandError> {
    validate_runtime_id(&runtime_id)?;
    tools
        .update(&runtime_id, UpdateBy::Owner)
        .map_err(broker_error)
}

/// Stop an update that is still waiting for its AI tool to be free.
#[tauri::command]
pub fn cancel_ai_tool_update(
    tools: State<'_, AiTools>,
    runtime_id: String,
) -> Result<AiToolsPage, CommandError> {
    validate_runtime_id(&runtime_id)?;
    tools.cancel_update(&runtime_id).map_err(broker_error)
}

/// The switch: update AI tools by themselves, or ask first (the default).
#[tauri::command]
pub async fn set_ai_tools_auto_update(
    tools: State<'_, AiTools>,
    on: bool,
) -> Result<AiToolsPage, CommandError> {
    let tools = tools.inner().clone();
    tauri::async_runtime::spawn_blocking(move || tools.set_auto_update(on))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
        .map_err(broker_error)
}

/// How an AI tool is paid for: a subscription only, until spending caps exist (Phase 16).
#[tauri::command]
pub fn set_ai_tool_payment(
    tools: State<'_, AiTools>,
    runtime_id: String,
    method: PaymentMethod,
) -> Result<AiToolsPage, CommandError> {
    validate_runtime_id(&runtime_id)?;
    tools.set_payment(&runtime_id, method).map_err(broker_error)
}
