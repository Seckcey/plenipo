//! System tray: show the window, see how many programs are running, stop them, or quit; and
//! (Phase 10) see who uses Plenipo's browser or the mouse and keyboard, and stop all of it at
//! once.

use plenipo_capabilities::control::{ControlKind, ControlState, ControlStatus};
use plenipo_capabilities::Broker;
use plenipo_runtime::Supervisor;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager as _, Runtime};

/// Present only when the tray was created successfully.
pub struct Tray<R: Runtime> {
    icon: TrayIcon<R>,
    active: MenuItem<R>,
    control: MenuItem<R>,
    stop_control: MenuItem<R>,
}

/// What the tray says about control.
const NO_CONTROL: &str = "No worker is using the browser or the mouse and keyboard";

pub fn create<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Plenipo", true, None::<&str>)?;
    let control = MenuItem::with_id(app, "control", NO_CONTROL, false, None::<&str>)?;
    let stop_control = MenuItem::with_id(
        app,
        "stop_control",
        "Stop all browser and desktop control",
        true,
        None::<&str>,
    )?;
    let active = MenuItem::with_id(app, "active", "No programs running", false, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop_all", "Stop all programs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Plenipo", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &control,
            &stop_control,
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
    app.manage(Tray {
        icon,
        active,
        control,
        stop_control,
    });
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
        // The emergency stop (Phase 10): all browser and desktop control halts at once.
        "stop_control" => {
            if let Some(broker) = app.try_state::<Broker>() {
                let broker = broker.inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = broker.stop_all_control("owner").await {
                        eprintln!("[plenipo] could not stop control: {e}");
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

/// Update the tray's count of running programs.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let (Some(tray), Some(sup)) = (app.try_state::<Tray<R>>(), app.try_state::<Supervisor>())
    else {
        return;
    };
    let text = match sup.active_count() {
        0 => "No programs running".to_owned(),
        1 => "1 program running".to_owned(),
        n => format!("{n} programs running"),
    };
    let _ = tray.active.set_text(&text);
    let _ = tray.icon.set_tooltip(Some(format!("Plenipo — {text}")));
}

/// What the tray says about workers using the browser or the mouse and keyboard (Phase 10).
pub fn control_words(status: &ControlStatus) -> String {
    if status.stopped {
        return "Control is stopped: no worker may use the browser or the desktop".into();
    }
    let active: Vec<String> = status
        .sessions
        .iter()
        .filter(|s| s.state == ControlState::Active)
        .map(|s| match s.kind {
            ControlKind::Browser => format!("{} is using Plenipo's browser", s.worker),
            ControlKind::Desktop => format!("{} is using your mouse and keyboard", s.worker),
        })
        .collect();
    match active.as_slice() {
        [] => NO_CONTROL.into(),
        [one] => one.clone(),
        [first, rest @ ..] => format!("{first} (and {} more)", rest.len()),
    }
}

/// Update the tray for a change of control.
pub fn show_control<R: Runtime>(app: &AppHandle<R>, status: &ControlStatus) {
    let Some(tray) = app.try_state::<Tray<R>>() else {
        return;
    };
    let words = control_words(status);
    let _ = tray.control.set_text(&words);
    let _ = tray.stop_control.set_enabled(!status.stopped);
    if status.active() || status.stopped {
        let _ = tray.icon.set_tooltip(Some(format!("Plenipo — {words}")));
    } else {
        refresh(app);
    }
}

#[cfg(test)]
mod tests {
    use plenipo_capabilities::control::ControlSession;

    use super::*;

    #[test]
    fn the_tray_says_who_has_control() {
        let mut status = ControlStatus::default();
        assert_eq!(control_words(&status), NO_CONTROL);
        let session = |worker: &str, kind| ControlSession {
            id: "x".into(),
            grant_id: "g".into(),
            task_id: "t".into(),
            worker: worker.into(),
            kind,
            state: ControlState::Active,
            detail: None,
            last_action: None,
            since: 0,
        };
        status
            .sessions
            .push(session("Web Assistant", ControlKind::Browser));
        assert_eq!(
            control_words(&status),
            "Web Assistant is using Plenipo's browser"
        );
        status
            .sessions
            .push(session("Desk Operator", ControlKind::Desktop));
        assert_eq!(
            control_words(&status),
            "Web Assistant is using Plenipo's browser (and 1 more)"
        );
        status.stopped = true;
        assert!(control_words(&status).starts_with("Control is stopped"));
    }
}
