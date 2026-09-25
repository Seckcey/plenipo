//! Typed Tauri command boundary. Inputs are validated here; DTOs come from
//! `plenipo-core` so the TypeScript bindings stay in lockstep.

use plenipo_core::{AppInfo, CommandError};
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

fn app_info_for(version: &str) -> AppInfo {
    AppInfo::current(version)
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
}
