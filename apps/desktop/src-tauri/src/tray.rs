//! System tray: show the window, see how many runtimes are active, stop them, or quit.

use plenipo_runtime::Supervisor;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager as _, Runtime};

/// Present only when the tray was created successfully.
pub struct Tray<R: Runtime> {
    icon: TrayIcon<R>,
    active: MenuItem<R>,
}

pub fn create<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Plenipo", true, None::<&str>)?;
    let active = MenuItem::with_id(app, "active", "No active runtimes", false, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop_all", "Stop all runtimes", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Plenipo", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &active,
            &stop,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Plenipo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let icon = builder.build(app)?;
    app.manage(Tray { icon, active });
    Ok(())
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_main_window(app),
        "stop_all" => {
            if let Some(sup) = app.try_state::<Supervisor>() {
                let sup = sup.inner().clone();
                tauri::async_runtime::spawn(async move {
                    for record in sup.overview().executions {
                        if !record.state.is_terminal() {
                            let _ = sup.cancel(&record.id).await;
                        }
                    }
                });
            }
        }
        // Goes through RunEvent::ExitRequested, which performs the graceful shutdown.
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn exists<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Tray<R>>().is_some()
}

/// Update the tray's active-runtime count.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let (Some(tray), Some(sup)) = (app.try_state::<Tray<R>>(), app.try_state::<Supervisor>())
    else {
        return;
    };
    let text = match sup.active_count() {
        0 => "No active runtimes".to_owned(),
        1 => "1 active runtime".to_owned(),
        n => format!("{n} active runtimes"),
    };
    let _ = tray.active.set_text(&text);
    let _ = tray.icon.set_tooltip(Some(format!("Plenipo — {text}")));
}
