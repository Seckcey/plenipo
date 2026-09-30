//! The workspace's commands (Phase 21, ADR-092 and ADR-093): an organization's window asks for a
//! pop-out, brings one to the front, and forgets their places (Reset layout). Each is an
//! organization's window's alone (`capabilities/default.json`); a pop-out has none
//! (`capabilities/popout.json`).

use plenipo_core::{CommandError, PanelId, WindowPlace};
use tauri::{Manager as _, Runtime, State, WebviewWindow};

use crate::workspace_windows::{is_org_window, popout_label, PopOuts};

/// The window asking must be an organization's window (its permission file already makes sure;
/// this is the second lock).
fn org_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<String, CommandError> {
    let label = window.label();
    if is_org_window(label) {
        Ok(label.to_owned())
    } else {
        Err(CommandError::invalid_input(
            "Only an organization's window can do that.",
        ))
    }
}

/// The page is about to open `panel` in its own window (`place`: where the panel was dropped,
/// or none for where it was last). Plenipo allows the next `window.open` of this window's page
/// for that panel only, once, within a few seconds.
#[tauri::command]
pub fn prepare_pop_out<R: Runtime>(
    window: WebviewWindow<R>,
    popouts: State<'_, PopOuts>,
    panel: PanelId,
    place: Option<WindowPlace>,
) -> Result<(), CommandError> {
    let parent = org_window(&window)?;
    if let Some(p) = &place {
        if !p.is_sane() {
            return Err(CommandError::invalid_input(
                "That is not a place on the screen.",
            ));
        }
    }
    popouts.request(&parent, panel, place);
    Ok(())
}

/// Bring a popped-out panel's window to the front (the Terminal button, while the terminal is
/// popped out). `false` when it is not open.
#[tauri::command]
pub fn focus_pop_out<R: Runtime>(
    window: WebviewWindow<R>,
    panel: PanelId,
) -> Result<bool, CommandError> {
    let parent = org_window(&window)?;
    let Some(popout) = window.app_handle().get_webview_window(&popout_label(&parent, panel))
    else {
        return Ok(false);
    };
    let _ = popout.unminimize();
    let _ = popout.show();
    let _ = popout.set_focus();
    Ok(true)
}

/// Reset layout: close this window's pop-outs and forget where they were.
#[tauri::command]
pub fn reset_pop_outs<R: Runtime>(
    window: WebviewWindow<R>,
    popouts: State<'_, PopOuts>,
) -> Result<(), CommandError> {
    let parent = org_window(&window)?;
    crate::workspace_windows::close_popouts(window.app_handle(), &parent);
    popouts.forget(&parent);
    Ok(())
}
