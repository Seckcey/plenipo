//! The sign above all other windows while a worker uses the mouse and keyboard (Phase 10,
//! ADR-020): who it is, and **Stop** and **Take over** buttons, always within reach even when
//! Plenipo's own window is hidden or behind others. It never takes the keyboard focus. It is a
//! separate window with its own narrow grant (`capabilities/indicator.json`): it can read the
//! control status, stop, and take over — nothing else.

use plenipo_capabilities::control::ControlStatus;
use tauri::{AppHandle, Manager as _, Runtime, WebviewUrl, WebviewWindowBuilder};

/// The window's label (its grant is by label).
pub const LABEL: &str = "control-indicator";
const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 76.0;

/// Show the sign while a worker has the mouse and keyboard; hide it otherwise.
pub fn update<R: Runtime>(app: &AppHandle<R>, status: &ControlStatus) {
    let show = status.desktop_active();
    match app.get_webview_window(LABEL) {
        Some(window) => {
            if show {
                let _ = window.show();
                let _ = window.set_always_on_top(true);
            } else {
                let _ = window.hide();
            }
        }
        None if show => {
            let built = WebviewWindowBuilder::new(
                app,
                LABEL,
                WebviewUrl::App("index.html#indicator".into()),
            )
            .title("Plenipo: a worker is using your mouse and keyboard")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible_on_all_workspaces(true)
            .build();
            match built {
                Ok(window) => {
                    // Top center of the main screen.
                    if let Ok(Some(monitor)) = window.primary_monitor() {
                        let scale = monitor.scale_factor();
                        let size = monitor.size().to_logical::<f64>(scale);
                        let _ = window.set_position(tauri::LogicalPosition::new(
                            ((size.width - WIDTH) / 2.0).max(0.0),
                            8.0,
                        ));
                    }
                }
                Err(e) => eprintln!("[plenipo] the control sign could not be shown: {e}"),
            }
        }
        None => {}
    }
}
