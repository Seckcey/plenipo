//! Starting and closing (Phase 13, ADR-036, background work): Plenipo lives in the tray and the
//! window comes and goes.
//!
//! - **Start with Windows** (off until the owner turns it on): Windows' own per-user "Run at
//!   sign-in" list, value `Plenipo`, starting Plenipo in the tray with [`IN_TRAY_ARG`]. No
//!   administrator. The uninstaller removes the same value.
//! - **When I close the window**: keep Plenipo in the tray while work is going (the starting
//!   choice), always keep it in the tray, or quit and stop the work.
//! - **One Plenipo at a time**: opening it again shows the one already running; opening it with
//!   [`QUIT_ARG`] asks the running one to quit cleanly (the installer uses it).

use plenipo_core::{CloseWindow, StartAndClose};
use plenipo_ledger::Ledger;
use serde_json::json;
use tauri::{AppHandle, Manager as _, Runtime};

/// Started by Windows at sign-in: stay in the tray, no window.
pub const IN_TRAY_ARG: &str = "--in-tray";
/// Ask the Plenipo that is running to quit the normal way (stopping and recording its work).
pub const QUIT_ARG: &str = "--quit";
/// The name in Windows' "Run at sign-in" list (the uninstaller removes this exact name).
pub const RUN_NAME: &str = "Plenipo";
/// Where the choice is kept, in the Ledger's `preferences` setting.
const FIELD: &str = "closeWindow";
const PREFERENCES: &str = "preferences";

/// What to do when the owner closes the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Hide the window; Plenipo stays in the tray.
    Hide,
    /// Quit Plenipo the normal way.
    Quit,
}

/// Decide what closing the window does. `tray`: there is a tray icon to come back from.
pub fn close_action(choice: CloseWindow, work_going: bool, tray: bool) -> CloseAction {
    match choice {
        CloseWindow::KeepWhileWorking if work_going && tray => CloseAction::Hide,
        CloseWindow::AlwaysKeep if tray => CloseAction::Hide,
        _ => CloseAction::Quit,
    }
}

/// The owner's choice (the starting choice when none was made, or it cannot be read).
pub fn close_window(ledger: &Ledger) -> CloseWindow {
    ledger
        .setting(PREFERENCES)
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v[FIELD].clone()).ok())
        .unwrap_or_default()
}

pub fn set_close_window(ledger: &Ledger, choice: CloseWindow) -> plenipo_ledger::Result<()> {
    ledger.merge_setting(PREFERENCES, &json!({ FIELD: choice }), "owner")?;
    Ok(())
}

/// Plenipo was started by Windows at sign-in.
pub fn started_in_tray(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == IN_TRAY_ARG)
}

/// Plenipo was opened to ask the running one to quit.
pub fn asked_to_quit(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == QUIT_ARG)
}

/// The Start with Windows plugin, when this Plenipo has it (Windows; not in tests).
fn autostart<R: Runtime>(
    app: &AppHandle<R>,
) -> Option<tauri::State<'_, tauri_plugin_autostart::AutoLaunchManager>> {
    app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
}

/// Whether Windows starts Plenipo at sign-in (`None`: this computer cannot).
pub fn start_with_windows<R: Runtime>(app: &AppHandle<R>) -> Option<bool> {
    let manager = autostart(app)?;
    match manager.is_enabled() {
        Ok(on) => Some(on),
        Err(e) => {
            log::warn!("Plenipo could not read whether it starts with Windows: {e}");
            Some(false)
        }
    }
}

/// Turn Start with Windows on or off.
pub fn set_start_with_windows<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<(), String> {
    let manager =
        autostart(app).ok_or("Starting with Windows is not available on this computer.")?;
    let done = if on {
        manager.enable()
    } else {
        manager.disable()
    };
    done.map_err(|e| format!("Windows did not accept the change: {e}"))?;
    log::info!(
        "start with Windows turned {}",
        if on { "on" } else { "off" }
    );
    Ok(())
}

/// Settings → Start and close.
pub fn settings<R: Runtime>(app: &AppHandle<R>, ledger: &Ledger) -> StartAndClose {
    let start = start_with_windows(app);
    StartAndClose {
        start_with_windows: start.unwrap_or(false),
        can_start_with_windows: start.is_some(),
        close_window: close_window(ledger),
    }
}

/// The autostart plugin, starting Plenipo in the tray under the name the uninstaller removes.
pub fn autostart_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::Builder::new()
        .app_name(RUN_NAME)
        .args([IN_TRAY_ARG])
        .build()
}

/// Show the main window (creating it again if it was closed for good).
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let window = match app.get_webview_window("main") {
        Some(w) => Some(w),
        None => recreate_main_window(app),
    };
    if let Some(window) = window {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Build the main window again from the app's configuration.
pub fn recreate_main_window<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")?
        .clone();
    match tauri::WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build()) {
        Ok(w) => Some(w),
        Err(e) => {
            log::error!("the window could not be opened again: {e}");
            None
        }
    }
}

/// A second Plenipo was opened: show this one, or quit if that is what it asked.
pub fn on_second_launch<R: Runtime>(app: &AppHandle<R>, args: &[String]) {
    if asked_to_quit(args) {
        log::info!("asked to quit by a second launch (the installer, or the owner)");
        app.exit(0);
    } else if !started_in_tray(args) {
        show_main_window(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_window_keeps_work_going_unless_the_owner_chose_otherwise() {
        use CloseAction::{Hide, Quit};
        use CloseWindow::{AlwaysKeep, KeepWhileWorking};
        // The starting choice: hide while work is going, quit when nothing is.
        assert_eq!(close_action(KeepWhileWorking, true, true), Hide);
        assert_eq!(close_action(KeepWhileWorking, false, true), Quit);
        // Always keep: hide even when idle.
        assert_eq!(close_action(AlwaysKeep, false, true), Hide);
        // Quit and stop the work: quit even when work is going.
        assert_eq!(close_action(CloseWindow::Quit, true, true), Quit);
        // No tray to come back from: never hide the only way back.
        assert_eq!(close_action(KeepWhileWorking, true, false), Quit);
        assert_eq!(close_action(AlwaysKeep, true, false), Quit);
    }

    #[test]
    fn the_choice_is_kept_in_the_ledger_and_read_safely() {
        let l = Ledger::open_in_memory().unwrap();
        assert_eq!(close_window(&l), CloseWindow::KeepWhileWorking);
        set_close_window(&l, CloseWindow::AlwaysKeep).unwrap();
        assert_eq!(close_window(&l), CloseWindow::AlwaysKeep);
        // Other preferences are kept.
        let prefs = l.setting(PREFERENCES).unwrap().unwrap();
        assert_eq!(prefs[FIELD], "alwaysKeep");
        // A damaged value falls back to the starting choice.
        l.merge_setting(PREFERENCES, &json!({ FIELD: 42 }), "x")
            .unwrap();
        assert_eq!(close_window(&l), CloseWindow::KeepWhileWorking);
    }

    #[test]
    fn launch_arguments_are_read_after_the_program_name() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(started_in_tray(&args(&["plenipo.exe", "--in-tray"])));
        assert!(!started_in_tray(&args(&["--in-tray"])));
        assert!(asked_to_quit(&args(&["plenipo.exe", "--quit"])));
        assert!(!asked_to_quit(&args(&["plenipo.exe"])));
    }
}
