//! Starting and closing (Phase 13, ADR-037, background work): Plenipo lives in the tray and the
//! window comes and goes.
//!
//! - **Start with Windows** (off until the owner turns it on): Windows' own per-user "Run at
//!   sign-in" list, value `Plenipo`, starting Plenipo in the tray with [`IN_TRAY_ARG`]. No
//!   administrator. The uninstaller removes the same value.
//! - **When I close the window**: keep Plenipo in the tray while work is going (the starting
//!   choice), always keep it in the tray, or quit and stop the work.
//! - **One Plenipo at a time**: opening it again shows the one already running; opening it with
//!   [`QUIT_ARG`] asks the running one to quit cleanly (the installer uses it).

use std::path::Path;

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

/// Marks a restart Plenipo asked for itself (after a restore, or an update that could not
/// start): the window shows after it even if the first start was in the tray. In the `run`
/// folder of Plenipo's own folder.
pub const SHOW_AFTER_RESTART: &str = "show-window-after-restart";

/// Ask the next start to show the window (before a restart Plenipo asks for itself).
pub fn show_after_restart(data_dir: &Path) {
    let run = data_dir.join("run");
    let _ = std::fs::create_dir_all(&run);
    if let Err(e) = std::fs::write(run.join(SHOW_AFTER_RESTART), b"") {
        log::warn!("the window may stay in the tray after the restart: {e}");
    }
}

/// Whether this start follows a restart Plenipo asked for (the mark is used once).
pub fn shows_after_restart(data_dir: &Path) -> bool {
    std::fs::remove_file(data_dir.join("run").join(SHOW_AFTER_RESTART)).is_ok()
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
    let on = if cfg!(target_os = "linux") {
        Ok(sign_in_entry::file().is_some_and(|f| f.is_file()))
    } else {
        manager.is_enabled().map_err(|e| e.to_string())
    };
    match on {
        Ok(on) => Some(on),
        Err(e) => {
            log::warn!("Plenipo could not read whether it starts with Windows: {e}");
            Some(false)
        }
    }
}

/// Linux's sign-in list: a desktop entry in the owner's autostart folder (Phase 23), written by
/// Plenipo itself. The plugin's own does not quote the program (an AppImage in a folder with a
/// space in its name would never start) and cannot make a missing folder.
mod sign_in_entry {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    /// `autostart/Plenipo.desktop` in the owner's settings folder.
    pub fn file() -> Option<PathBuf> {
        file_in(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }

    pub(super) fn file_in(config: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
        let config = config
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                home.map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|h| h.join(".config"))
            })?;
        Some(
            config
                .join("autostart")
                .join(format!("{}.desktop", super::RUN_NAME)),
        )
    }

    /// Start this copy at sign-in: the AppImage it runs from, or the installed program.
    pub fn turn_on() -> Result<(), String> {
        let file = file().ok_or("the home folder is not known")?;
        let program = match crate::update_host::own_appimage() {
            Some(appimage) => appimage,
            None => std::env::current_exe().map_err(|e| e.to_string())?,
        };
        let text = entry(&program)?;
        let folder = file.parent().ok_or("no autostart folder")?;
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let next = folder.join(format!(".{}.desktop.new", super::RUN_NAME));
        std::fs::write(&next, text).map_err(|e| e.to_string())?;
        std::fs::rename(&next, &file).map_err(|e| e.to_string())
    }

    pub fn turn_off() -> Result<(), String> {
        let Some(file) = file() else { return Ok(()) };
        match std::fs::remove_file(file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    /// The desktop entry that starts `program` in the tray.
    pub(super) fn entry(program: &Path) -> Result<String, String> {
        let program = program
            .to_str()
            .ok_or("the program's path is not plain text")?;
        Ok(format!(
            "[Desktop Entry]\nType=Application\nName={name}\nComment=Starts {name} in the tray \
             when you sign in\nExec={program} {in_tray}\nTerminal=false\nStartupNotify=false\n\
             X-GNOME-Autostart-enabled=true\n",
            name = super::RUN_NAME,
            program = quoted(program)?,
            in_tray = super::IN_TRAY_ARG,
        ))
    }

    /// One argument of `Exec`, by the desktop entry rules: in double quotes, with `"`, `` ` ``,
    /// `$`, and `\` escaped, then every `\` escaped again (the file's own rule), and `%` doubled.
    pub(super) fn quoted(arg: &str) -> Result<String, String> {
        if arg.chars().any(char::is_control) {
            return Err("the program's path has a line break in it".into());
        }
        let mut out = String::from("\"");
        for c in arg.chars() {
            match c {
                '"' | '`' | '$' => {
                    out.push_str("\\\\");
                    out.push(c);
                }
                '\\' => out.push_str("\\\\\\\\"),
                '%' => out.push_str("%%"),
                c => out.push(c),
            }
        }
        out.push('"');
        Ok(out)
    }
}

/// "Starting with Windows is not available on this computer." in the system's own words
/// ("Opening Plenipo when you log in" on a Mac, ADR-155).
pub fn not_available() -> String {
    format!(
        "{} is not available on this computer.",
        plenipo_core::WORDS.starting_at_sign_in
    )
}

/// Turn Start with Windows on or off.
pub fn set_start_with_windows<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<(), String> {
    let manager = autostart(app).ok_or_else(not_available)?;
    let done = if cfg!(target_os = "linux") {
        if on {
            sign_in_entry::turn_on()
        } else {
            sign_in_entry::turn_off()
        }
    } else if on {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    };
    done.map_err(|e| {
        format!(
            "{} did not accept the change: {e}",
            plenipo_core::words::sentence_start(plenipo_core::WORDS.the_system)
        )
    })?;
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
    let shown = |window: tauri::WebviewWindow<R>, app: &AppHandle<R>| {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        crate::workspace_windows::show_popouts(app, window.label());
    };
    match app.get_webview_window("main") {
        Some(window) => shown(window, app),
        // Called from the tray's and a second launch's handlers: building a window there locks
        // up on Windows (WebView2), so it is built off this thread (Phase 25, item 1.1).
        None => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(window) = recreate_main_window(&app) {
                    shown(window, &app);
                }
            });
        }
    }
}

/// Build the main window again from the app's configuration.
pub fn recreate_main_window<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
    match crate::workspace_windows::build_org_window(app, crate::workspace_windows::MAIN, true) {
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
    fn a_restart_plenipo_asked_for_shows_the_window_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!shows_after_restart(dir.path()));
        show_after_restart(dir.path());
        assert!(shows_after_restart(dir.path()));
        assert!(!shows_after_restart(dir.path()), "used once");
    }

    #[test]
    fn launch_arguments_are_read_after_the_program_name() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(started_in_tray(&args(&["plenipo.exe", "--in-tray"])));
        assert!(!started_in_tray(&args(&["--in-tray"])));
        assert!(asked_to_quit(&args(&["plenipo.exe", "--quit"])));
        assert!(!asked_to_quit(&args(&["plenipo.exe"])));
    }

    /// Phase 23: Linux's sign-in entry lives in the owner's settings folder.
    #[cfg(unix)]
    #[test]
    fn linux_keeps_the_sign_in_entry_in_the_owners_settings() {
        use std::ffi::OsString;
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            sign_in_entry::file_in(None, os("/home/sam")).unwrap(),
            Path::new("/home/sam/.config/autostart/Plenipo.desktop")
        );
        assert_eq!(
            sign_in_entry::file_in(os("/home/sam/cfg"), os("/home/sam")).unwrap(),
            Path::new("/home/sam/cfg/autostart/Plenipo.desktop")
        );
        assert_eq!(
            sign_in_entry::file_in(os("relative"), os("/home/sam")).unwrap(),
            Path::new("/home/sam/.config/autostart/Plenipo.desktop"),
            "a relative setting is ignored"
        );
        assert!(sign_in_entry::file_in(None, None).is_none());
    }

    /// Phase 23: Linux's sign-in entry quotes the program, wherever it is.
    #[test]
    fn linux_starts_plenipo_at_sign_in_from_any_folder() {
        let entry = sign_in_entry::entry(Path::new("/home/sam/My Apps/Plenipo.AppImage")).unwrap();
        assert!(
            entry.contains("\nExec=\"/home/sam/My Apps/Plenipo.AppImage\" --in-tray\n"),
            "{entry}"
        );
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\nName=Plenipo\n"));
        // `"`, `$`, and `\` are escaped twice over; `%` is doubled.
        assert_eq!(
            sign_in_entry::quoted(r#"/a "b" $c \d 100%"#).unwrap(),
            r#""/a \\"b\\" \\$c \\\\d 100%%""#
        );
        assert!(sign_in_entry::quoted("/a\nb").is_err());
    }
}
