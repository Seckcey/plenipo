//! Plenipo Desktop backend.
//!
//! The React frontend reaches Rust only through the typed commands in
//! [`commands`]. Each command must also be granted in
//! `capabilities/default.json`; nothing else is exposed.

pub mod commands;
pub mod smoke;

use tauri::{Builder, Manager as _, Runtime};

use smoke::{SmokeMode, SmokeTest};

/// Register Plenipo state, setup hooks, and the command surface on a builder.
/// Shared by the real app and the IPC boundary tests.
pub fn configure<R: Runtime>(builder: Builder<R>, smoke: SmokeTest) -> Builder<R> {
    builder
        .manage(smoke)
        .setup(|app| {
            let smoke = app.state::<SmokeTest>();
            if let SmokeMode::Enabled { timeout } = smoke.mode() {
                smoke.arm_watchdog(app.handle().clone(), timeout);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::frontend_ready
        ])
}

/// Build and run the Tauri application, returning the process exit code.
pub fn run() -> i32 {
    let smoke = SmokeTest::from_env();
    let outcome = smoke.clone();

    let runtime_code = configure(tauri::Builder::default(), smoke)
        .build(tauri::generate_context!())
        .expect("error while building Plenipo")
        .run_return(|_app, _event| {});

    // The runtime does not reliably propagate the code given to `AppHandle::exit`
    // on every platform, so the smoke outcome is tracked independently.
    outcome.resolve_exit_code(runtime_code)
}

#[cfg(test)]
mod ipc_boundary_tests {
    //! Exercise the real capability configuration through Tauri's mock runtime.

    use super::*;
    use plenipo_core::AppInfo;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{App, WebviewWindow, WebviewWindowBuilder};

    fn app() -> App<MockRuntime> {
        configure(mock_builder(), SmokeTest::new(SmokeMode::Disabled))
            .build(tauri::generate_context!())
            .expect("failed to build mock app")
    }

    fn window(app: &App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
        WebviewWindowBuilder::new(app, label, Default::default())
            .build()
            .expect("failed to build window")
    }

    /// The app's own origin in test (debug) builds: the configured `devUrl`.
    const LOCAL_ORIGIN: &str = "http://localhost:1420";

    fn invoke(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        invoke_from(window, cmd, LOCAL_ORIGIN)
    }

    fn invoke_from(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
        origin: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: origin.parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    #[test]
    fn main_window_can_get_app_info() {
        let app = app();
        let main = window(&app, "main");
        let info: AppInfo = invoke(&main, "get_app_info")
            .expect("get_app_info should be allowed for main")
            .deserialize()
            .unwrap();
        assert_eq!(info.name, "Plenipo");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn frontend_ready_is_a_no_op_outside_smoke_mode() {
        let app = app();
        let main = window(&app, "main");
        assert!(invoke(&main, "frontend_ready").is_ok());
    }

    #[test]
    fn unknown_command_is_rejected() {
        let app = app();
        let main = window(&app, "main");
        assert!(invoke(&main, "run_shell").is_err());
    }

    #[test]
    fn os_plugins_are_not_reachable() {
        let app = app();
        let main = window(&app, "main");
        for cmd in [
            "plugin:shell|execute",
            "plugin:fs|read_file",
            "plugin:http|fetch",
        ] {
            assert!(invoke(&main, cmd).is_err(), "{cmd} must not be reachable");
        }
    }

    #[test]
    fn remote_origins_are_denied_even_in_main_window() {
        let app = app();
        let main = window(&app, "main");
        assert!(invoke_from(&main, "get_app_info", "https://example.com").is_err());
    }

    #[test]
    fn windows_without_a_capability_grant_are_denied() {
        let app = app();
        let other = window(&app, "untrusted");
        assert!(invoke(&other, "get_app_info").is_err());
    }
}
