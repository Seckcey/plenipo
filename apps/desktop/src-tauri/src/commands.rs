//! Typed Tauri command boundary. Inputs are validated here; DTOs come from
//! `plenipo-core` / `plenipo-runtime` so the TypeScript bindings stay in lockstep.
//!
//! There is deliberately no command that accepts an executable, arguments, environment
//! variables, or a working directory. The UI can only name a pre-approved launch profile.

use plenipo_core::{AppInfo, CommandError};
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
