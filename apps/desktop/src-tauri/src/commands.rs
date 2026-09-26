//! Typed Tauri command boundary. Inputs are validated here; DTOs come from
//! `plenipo-core` / `plenipo-runtime` so the TypeScript bindings stay in lockstep.
//!
//! There is deliberately no command that accepts an executable, arguments, environment
//! variables, or a working directory. The UI can only name a pre-approved launch profile.

use std::sync::Arc;

use plenipo_core::{AppInfo, CommandError, SyntheticTaskAction};
use plenipo_ledger::{
    BackupInfo, ExportInfo, IntegrityReport, Ledger, LedgerError, LedgerEvent, LedgerStatus,
    NewTask, Task, TaskState, TaskTimeline,
};
use plenipo_runtime::{
    ExecutionOutput, ExecutionRecord, RuntimeError, RuntimeOverview, Supervisor,
};
use tauri::{AppHandle, Runtime, State};

use crate::smoke::{SmokeTest, EXIT_READY};

/// Return identity information about the running application.
#[tauri::command]
pub fn get_app_info<R: Runtime>(app: AppHandle<R>) -> Result<AppInfo, CommandError> {
    Ok(app_info_for(&app.package_info().version.to_string()))
}

/// Called by the UI once the shell has rendered successfully.
/// Only has an effect in smoke-test mode, where it ends the process with success.
#[tauri::command]
pub fn frontend_ready<R: Runtime>(
    app: AppHandle<R>,
    smoke: State<'_, SmokeTest>,
) -> Result<(), CommandError> {
    if smoke.is_enabled() && smoke.record(EXIT_READY) {
        eprintln!("[plenipo] smoke test: frontend reported ready; exiting 0");
        app.exit(EXIT_READY);
    }
    Ok(())
}

/// Profiles, execution history (newest first), active count, and notices.
#[tauri::command]
pub fn get_runtime_overview(
    supervisor: State<'_, Supervisor>,
) -> Result<RuntimeOverview, CommandError> {
    Ok(supervisor.overview())
}

/// Launch an approved profile by ID.
#[tauri::command]
pub async fn start_execution(
    supervisor: State<'_, Supervisor>,
    profile_id: String,
) -> Result<ExecutionRecord, CommandError> {
    validate_profile_id(&profile_id)?;
    supervisor
        .start(&profile_id)
        .await
        .map_err(to_command_error)
}

/// Terminate an execution's process tree and return its final record.
#[tauri::command]
pub async fn cancel_execution(
    supervisor: State<'_, Supervisor>,
    execution_id: String,
) -> Result<ExecutionRecord, CommandError> {
    validate_execution_id(&execution_id)?;
    supervisor
        .cancel(&execution_id)
        .await
        .map_err(to_command_error)
}

/// Buffered output for an execution (used to rebuild the view after a reload).
#[tauri::command]
pub fn get_execution_output(
    supervisor: State<'_, Supervisor>,
    execution_id: String,
) -> Result<ExecutionOutput, CommandError> {
    validate_execution_id(&execution_id)?;
    supervisor.output(&execution_id).map_err(to_command_error)
}

// ---- Ledger ------------------------------------------------------------------------------

/// Actor recorded for actions taken by the person using the app.
const OWNER: &str = "owner";

/// Run ledger work off the main thread.
async fn with_ledger<T: Send + 'static>(
    ledger: &Arc<Ledger>,
    f: impl FnOnce(&Ledger) -> Result<T, LedgerError> + Send + 'static,
) -> Result<T, CommandError> {
    let ledger = Arc::clone(ledger);
    tauri::async_runtime::spawn_blocking(move || f(&ledger))
        .await
        .map_err(|e| CommandError::internal(format!("ledger task failed: {e}")))?
        .map_err(ledger_error)
}

fn ledger_error(e: LedgerError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

fn validate_task_id(id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input("invalid task id"))
}

#[tauri::command]
pub async fn get_ledger_status(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<LedgerStatus, CommandError> {
    with_ledger(&ledger, Ledger::status).await
}

/// Tasks, newest first (at most 500).
#[tauri::command]
pub async fn list_tasks(ledger: State<'_, Arc<Ledger>>) -> Result<Vec<Task>, CommandError> {
    with_ledger(&ledger, |l| l.list_tasks(500)).await
}

/// A task's complete ordered activity trail and its direct children.
#[tauri::command]
pub async fn get_task_timeline(
    ledger: State<'_, Arc<Ledger>>,
    task_id: String,
) -> Result<TaskTimeline, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| l.task_timeline(&task_id)).await
}

/// Most recent events across the ledger, newest first (at most 200).
#[tauri::command]
pub async fn list_recent_events(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<Vec<LedgerEvent>, CommandError> {
    with_ledger(&ledger, |l| l.recent_events(200)).await
}

/// Diagnostics: create a synthetic task to exercise the ledger end to end.
#[tauri::command]
pub async fn create_synthetic_task(ledger: State<'_, Arc<Ledger>>) -> Result<Task, CommandError> {
    with_ledger(&ledger, |l| {
        let n = l.list_tasks(1000)?.len() + 1;
        l.create_task(
            synthetic(None, format!("Synthetic diagnostic task #{n}")),
            OWNER,
        )
    })
    .await
}

/// Diagnostics: apply an action to a synthetic task. Only tasks created as synthetic can be
/// changed this way; the ledger's state machine still decides what is allowed.
#[tauri::command]
pub async fn advance_synthetic_task(
    ledger: State<'_, Arc<Ledger>>,
    task_id: String,
    action: SyntheticTaskAction,
) -> Result<Task, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| {
        let task = l
            .task(&task_id)?
            .ok_or_else(|| LedgerError::NotFound(format!("task {task_id}")))?;
        if task.metadata.get("synthetic") != Some(&serde_json::Value::Bool(true)) {
            return Err(LedgerError::InvalidInput(
                "only synthetic diagnostic tasks can be changed from Diagnostics".into(),
            ));
        }
        let to = match action {
            SyntheticTaskAction::AddChild => {
                let n = l.child_tasks(&task_id)?.len() + 1;
                return l.create_task(
                    synthetic(
                        Some(task_id.clone()),
                        format!("{} — step {n}", task.objective),
                    ),
                    OWNER,
                );
            }
            SyntheticTaskAction::Start | SyntheticTaskAction::Resume => TaskState::Running,
            SyntheticTaskAction::Block => TaskState::Blocked,
            SyntheticTaskAction::AwaitApproval => TaskState::AwaitingApproval,
            SyntheticTaskAction::Complete => TaskState::Succeeded,
            SyntheticTaskAction::Fail => TaskState::Failed,
            SyntheticTaskAction::Cancel => TaskState::Cancelled,
        };
        l.transition_task(&task_id, to, OWNER, Some("diagnostics"))
    })
    .await
}

fn synthetic(parent: Option<String>, objective: String) -> NewTask {
    NewTask {
        parent_task_id: parent,
        requested_by: OWNER.into(),
        objective,
        acceptance_criteria: "Diagnostic only: exercises the ledger.".into(),
        priority: 3,
        metadata: serde_json::json!({ "synthetic": true }),
        ..NewTask::default()
    }
}

/// Full integrity check (can take a moment on large ledgers).
#[tauri::command]
pub async fn run_integrity_check(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<IntegrityReport, CommandError> {
    with_ledger(&ledger, Ledger::integrity_check).await
}

/// Verified backup into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn create_ledger_backup(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<BackupInfo, CommandError> {
    with_ledger(&ledger, |l| l.backup(None)).await
}

/// JSON export into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn export_ledger(ledger: State<'_, Arc<Ledger>>) -> Result<ExportInfo, CommandError> {
    with_ledger(&ledger, |l| l.export_json(None)).await
}

fn app_info_for(version: &str) -> AppInfo {
    AppInfo::current(version)
}

fn validate_profile_id(id: &str) -> Result<(), CommandError> {
    plenipo_runtime::profile::validate_profile_id(id).map_err(CommandError::invalid_input)
}

fn validate_execution_id(id: &str) -> Result<(), CommandError> {
    let valid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(CommandError::invalid_input("invalid execution id"))
    }
}

fn to_command_error(e: RuntimeError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_uses_supplied_version() {
        let info = app_info_for("9.9.9");
        assert_eq!(info.version, "9.9.9");
        assert_eq!(info.name, plenipo_core::PRODUCT_NAME);
    }

    #[test]
    fn get_app_info_reports_package_version() {
        let app = tauri::test::mock_app();
        let info = get_app_info(app.handle().clone()).unwrap();
        assert_eq!(info.version, app.package_info().version.to_string());
        assert_eq!(info.name, "Plenipo");
    }

    #[test]
    fn execution_id_validation() {
        assert!(validate_execution_id("0f8fad5b-d9cb-469f-a165-70867728950e").is_ok());
        for bad in [
            "",
            "nope",
            "../../etc/passwd",
            &"a".repeat(36),
            &"0".repeat(37),
        ] {
            let accepted = validate_execution_id(bad).is_ok();
            // 36 hex chars without dashes is harmless (it simply won't match), but
            // anything with path characters or the wrong length must be refused.
            if bad.len() != 36 || bad.contains('/') {
                assert!(!accepted, "{bad:?}");
            }
        }
    }

    #[test]
    fn profile_id_validation() {
        assert!(validate_profile_id("diagnostic.echo").is_ok());
        for bad in [
            "",
            "/bin/sh",
            "C:\\Windows\\cmd.exe",
            "diagnostic.echo && calc",
        ] {
            assert!(validate_profile_id(bad).is_err(), "{bad:?}");
        }
    }
}
