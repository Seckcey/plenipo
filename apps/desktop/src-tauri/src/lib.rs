//! Plenipo Desktop backend.
//!
//! The React frontend reaches Rust only through the typed commands in
//! [`commands`]. Each command must also be granted in
//! `capabilities/default.json`; nothing else is exposed.

pub mod agent_host;
pub mod commands;
pub mod ledger_host;
pub mod runtime_host;
pub mod smoke;
pub mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use tauri::{Builder, Manager as _, RunEvent, Runtime, WindowEvent};

use runtime_host::Persistence;
use smoke::{SmokeMode, SmokeTest};

/// How long quitting waits for owned processes to be terminated and recorded.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Environment-dependent pieces, so tests can run without a tray or on-disk state.
#[derive(Debug, Clone, Copy)]
pub struct ShellOptions {
    pub tray: bool,
    pub persistence: Persistence,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            tray: true,
            persistence: Persistence::AppData,
        }
    }
}

#[derive(Default)]
struct ShutdownState(AtomicBool);

/// Register Plenipo state, setup hooks, window behavior, and the command surface.
/// Shared by the real app and the IPC boundary tests.
pub fn configure<R: Runtime>(
    builder: Builder<R>,
    smoke: SmokeTest,
    options: ShellOptions,
) -> Builder<R> {
    builder
        .manage(smoke)
        .manage(ShutdownState::default())
        .setup(move |app| {
            let smoke = app.state::<SmokeTest>();
            if let SmokeMode::Enabled { timeout } = smoke.mode() {
                smoke.arm_watchdog(app.handle().clone(), timeout);
            }
            let ledger = ledger_host::open(app.handle(), options.persistence);
            app.manage(ledger.clone());
            let supervisor =
                runtime_host::create_supervisor(app.handle(), options.persistence, ledger.clone());
            let agents = agent_host::create(
                app.handle(),
                options.persistence,
                ledger,
                supervisor.clone(),
            );
            if options.persistence == Persistence::AppData {
                agent_host::detect_in_background(&agents);
            }
            app.manage(supervisor);
            app.manage(agents);
            quit_on_termination_signal(app.handle().clone());
            if options.tray {
                // A missing tray (e.g. no status-notifier host on Linux) is not fatal.
                if let Err(e) = tray::create(app) {
                    eprintln!("[plenipo] system tray unavailable: {e}");
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window must not silently kill running work: hide to the tray
            // instead. With nothing running (or no tray to come back from), close normally.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let active = app
                    .try_state::<Supervisor>()
                    .map_or(0, |s| s.active_count());
                if window.label() == "main" && active > 0 && tray::exists(app) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::frontend_ready,
            commands::get_runtime_overview,
            commands::start_execution,
            commands::cancel_execution,
            commands::get_execution_output,
            commands::get_ledger_status,
            commands::list_tasks,
            commands::get_task_timeline,
            commands::list_recent_events,
            commands::create_synthetic_task,
            commands::advance_synthetic_task,
            commands::run_integrity_check,
            commands::create_ledger_backup,
            commands::export_ledger,
            commands::get_agent_overview,
            commands::refresh_agent_runtimes,
            commands::get_agent_session,
            commands::start_agent_session,
            commands::resume_agent_session,
            commands::cancel_agent_turn,
            commands::close_agent_session
        ])
}

/// Handle app-level events. On the first exit request, terminate owned processes and record
/// their final state before letting the app exit.
pub fn on_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: RunEvent) {
    if let RunEvent::ExitRequested { api, code, .. } = event {
        let first = !app.state::<ShutdownState>().0.swap(true, Ordering::SeqCst);
        if first {
            api.prevent_exit();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                // The agent runtime stops its turns through the supervisor and records their
                // results; the supervisor then has nothing left to stop.
                let mut stopped = 0;
                if let Some(agents) = app.try_state::<AgentRuntime>() {
                    stopped += agents.shutdown(SHUTDOWN_GRACE).await;
                }
                if let Some(supervisor) = app.try_state::<Supervisor>() {
                    stopped += supervisor.shutdown(SHUTDOWN_GRACE).await;
                }
                if stopped > 0 {
                    eprintln!("[plenipo] terminated {stopped} running process(es) on exit");
                }
                app.exit(code.unwrap_or(0));
            });
        }
    }
}

/// Treat SIGTERM/SIGINT (logout, `kill`, Ctrl+C in a dev terminal) like "Quit": run the
/// graceful shutdown instead of dying with owned processes still running. Windows needs no
/// equivalent here: children live in a kill-on-close Job Object.
fn quit_on_termination_signal<R: Runtime>(app: tauri::AppHandle<R>) {
    #[cfg(unix)]
    tauri::async_runtime::spawn(async move {
        use tokio::signal::unix::{signal, SignalKind};
        let (Ok(mut term), Ok(mut int)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            return;
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
        eprintln!("[plenipo] termination signal received; shutting down");
        app.exit(0);
    });
    #[cfg(not(unix))]
    let _ = app;
}

/// Build and run the Tauri application, returning the process exit code.
pub fn run() -> i32 {
    let smoke = SmokeTest::from_env();
    let outcome = smoke.clone();

    let runtime_code = configure(tauri::Builder::default(), smoke, ShellOptions::default())
        .build(tauri::generate_context!())
        .expect("error while building Plenipo")
        .run_return(on_run_event);

    // The runtime does not reliably propagate the code given to `AppHandle::exit`
    // on every platform, so the smoke outcome is tracked independently.
    outcome.resolve_exit_code(runtime_code)
}

#[cfg(test)]
mod ipc_boundary_tests {
    //! Exercise the real capability configuration through Tauri's mock runtime.

    use super::*;
    use plenipo_core::AppInfo;
    use plenipo_runtime::RuntimeOverview;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{App, WebviewWindow, WebviewWindowBuilder};

    fn app() -> App<MockRuntime> {
        let app = configure(
            mock_builder(),
            SmokeTest::new(SmokeMode::Disabled),
            ShellOptions {
                tray: false,
                persistence: Persistence::InMemory,
            },
        )
        .build(tauri::generate_context!())
        .expect("failed to build mock app");
        // The mock runtime does not run `setup`; install the ledger and runtime the same way.
        let ledger = ledger_host::open(app.handle(), Persistence::InMemory);
        app.manage(ledger.clone());
        let supervisor =
            runtime_host::create_supervisor(app.handle(), Persistence::InMemory, ledger.clone());
        let agents = agent_host::create(
            app.handle(),
            Persistence::InMemory,
            ledger,
            supervisor.clone(),
        );
        app.manage(supervisor);
        app.manage(agents);
        app
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
        invoke_with(window, cmd, serde_json::json!({}), LOCAL_ORIGIN)
    }

    fn invoke_from(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
        origin: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        invoke_with(window, cmd, serde_json::json!({}), origin)
    }

    fn invoke_json(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
        args: serde_json::Value,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        invoke_with(window, cmd, args, LOCAL_ORIGIN)
    }

    fn invoke_with(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
        args: serde_json::Value,
        origin: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: origin.parse().unwrap(),
                body: InvokeBody::Json(args),
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
    fn runtime_overview_lists_only_approved_profiles() {
        let app = app();
        let main = window(&app, "main");
        let overview: RuntimeOverview = invoke(&main, "get_runtime_overview")
            .expect("overview allowed")
            .deserialize()
            .unwrap();
        let ids: Vec<_> = overview.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "diagnostic.echo",
                "diagnostic.failure",
                "diagnostic.long-running"
            ]
        );
        assert_eq!(overview.active_count, 0);
        // Profile DTOs expose no executable, args, or environment to the UI.
        let raw = serde_json::to_value(&overview.profiles)
            .unwrap()
            .to_string();
        for field in ["executable", "args", "env", "workingDir"] {
            assert!(!raw.contains(field), "profile DTO leaks {field}");
        }
    }

    #[test]
    fn start_execution_rejects_unknown_and_malformed_profiles() {
        let app = app();
        let main = window(&app, "main");
        for profile_id in ["does.not.exist", "/bin/sh", "cmd.exe /c calc", ""] {
            let err = invoke_json(
                &main,
                "start_execution",
                serde_json::json!({ "profileId": profile_id }),
            )
            .expect_err(profile_id);
            assert_eq!(err["kind"], "invalidInput", "{profile_id}: {err}");
        }
    }

    #[test]
    fn start_execution_ignores_attempts_to_supply_a_command() {
        // Extra fields are not part of the command's signature and cannot influence it.
        let app = app();
        let main = window(&app, "main");
        let err = invoke_json(
            &main,
            "start_execution",
            serde_json::json!({
                "profileId": "not.a.profile",
                "executable": "/bin/sh",
                "args": ["-c", "echo pwned"],
                "env": { "X": "1" },
            }),
        )
        .expect_err("must be rejected");
        assert_eq!(err["kind"], "invalidInput");
        assert!(invoke_json(&main, "start_execution", serde_json::json!({})).is_err());
    }

    #[test]
    fn cancel_and_output_validate_execution_ids() {
        let app = app();
        let main = window(&app, "main");
        for cmd in ["cancel_execution", "get_execution_output"] {
            let err = invoke_json(&main, cmd, serde_json::json!({ "executionId": "../x" }))
                .expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput");
            let err = invoke_json(
                &main,
                cmd,
                serde_json::json!({ "executionId": "0f8fad5b-d9cb-469f-a165-70867728950e" }),
            )
            .expect_err(cmd);
            assert!(err["message"]
                .as_str()
                .unwrap()
                .contains("unknown execution"));
        }
    }

    #[test]
    fn runtime_commands_denied_for_ungranted_windows() {
        let app = app();
        let other = window(&app, "untrusted");
        assert!(invoke(&other, "get_runtime_overview").is_err());
        assert!(invoke_json(
            &other,
            "start_execution",
            serde_json::json!({ "profileId": "diagnostic.echo" })
        )
        .is_err());
    }

    fn body<T: serde::de::DeserializeOwned>(
        r: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
    ) -> T {
        r.expect("command should succeed").deserialize().unwrap()
    }

    #[test]
    fn synthetic_task_lifecycle_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let status: plenipo_ledger::LedgerStatus = body(invoke(&main, "get_ledger_status"));
        assert_eq!(
            status.schema_version,
            plenipo_ledger::migrate::latest(plenipo_ledger::MIGRATIONS)
        );
        assert!(!status.persistent, "tests use an in-memory ledger");

        let task: plenipo_ledger::Task = body(invoke(&main, "create_synthetic_task"));
        assert_eq!(task.state, plenipo_ledger::TaskState::Queued);
        let id = serde_json::json!(task.id);

        // Invalid transition: queued -> succeeded is rejected and recorded.
        let err = invoke_json(
            &main,
            "advance_synthetic_task",
            serde_json::json!({ "taskId": id, "action": "complete" }),
        )
        .expect_err("queued -> succeeded must be rejected");
        assert_eq!(err["kind"], "invalidInput");
        assert!(err["message"]
            .as_str()
            .unwrap()
            .contains("queued -> succeeded"));

        for action in ["start", "addChild", "awaitApproval", "resume", "complete"] {
            let _: plenipo_ledger::Task = body(invoke_json(
                &main,
                "advance_synthetic_task",
                serde_json::json!({ "taskId": id, "action": action }),
            ));
        }
        let timeline: plenipo_ledger::TaskTimeline = body(invoke_json(
            &main,
            "get_task_timeline",
            serde_json::json!({ "taskId": id }),
        ));
        let types: Vec<_> = timeline
            .events
            .iter()
            .map(|e| e.event_type.as_str())
            .collect();
        assert_eq!(
            types,
            [
                "task.created",
                "task.transition_rejected",
                "task.state_changed",
                "task.child_created",
                "task.state_changed",
                "task.state_changed",
                "task.state_changed"
            ]
        );
        assert!(timeline.events.windows(2).all(|w| w[0].seq < w[1].seq));
        assert_eq!(timeline.task.state, plenipo_ledger::TaskState::Succeeded);
        assert_eq!(timeline.children.len(), 1);

        let tasks: Vec<plenipo_ledger::Task> = body(invoke(&main, "list_tasks"));
        assert_eq!(tasks.len(), 2);
        let events: Vec<plenipo_ledger::LedgerEvent> = body(invoke(&main, "list_recent_events"));
        assert_eq!(events[0].event_type, "task.state_changed");
        let report: plenipo_ledger::IntegrityReport = body(invoke(&main, "run_integrity_check"));
        assert!(report.ok);
    }

    #[test]
    fn only_synthetic_tasks_can_be_changed_from_the_ui() {
        let app = app();
        let main = window(&app, "main");
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let real = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "coordinator".into(),
                    objective: "real work".into(),
                    ..Default::default()
                },
                "coordinator",
            )
            .unwrap();
        let err = invoke_json(
            &main,
            "advance_synthetic_task",
            serde_json::json!({ "taskId": real.id, "action": "cancel" }),
        )
        .expect_err("non-synthetic");
        assert!(err["message"].as_str().unwrap().contains("only synthetic"));
        assert_eq!(
            ledger.task(&real.id).unwrap().unwrap().state,
            plenipo_ledger::TaskState::Queued
        );

        let bad_action = invoke_json(
            &main,
            "advance_synthetic_task",
            serde_json::json!({ "taskId": real.id, "action": "deleteEverything" }),
        );
        assert!(bad_action.is_err());
        let bad_id = invoke_json(
            &main,
            "get_task_timeline",
            serde_json::json!({ "taskId": "../../etc" }),
        )
        .expect_err("bad id");
        assert_eq!(bad_id["kind"], "invalidInput");
    }

    #[test]
    fn backups_need_a_real_ledger_and_never_take_a_path_from_the_ui() {
        let app = app();
        let main = window(&app, "main");
        // In-memory (test) ledger: refused cleanly rather than writing somewhere unexpected.
        let err = invoke_json(
            &main,
            "create_ledger_backup",
            serde_json::json!({ "path": "C:/Windows/System32/evil.db" }),
        )
        .expect_err("in-memory ledger has no backup dir");
        assert_eq!(err["kind"], "invalidInput");
        assert!(invoke(&main, "export_ledger").is_err());
    }

    #[test]
    fn ledger_commands_denied_for_ungranted_windows() {
        let app = app();
        let other = window(&app, "untrusted");
        for cmd in [
            "get_ledger_status",
            "list_tasks",
            "create_synthetic_task",
            "create_ledger_backup",
        ] {
            assert!(invoke(&other, cmd).is_err(), "{cmd}");
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

    // ---- Agent runtimes (Phase 3) ---------------------------------------------------------

    const SESSION: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

    #[test]
    fn agent_overview_lists_runtimes_without_starting_anything() {
        let app = app();
        let main = window(&app, "main");
        let overview: plenipo_runtime::agent::AgentOverview =
            body(invoke(&main, "get_agent_overview"));
        let ids: Vec<_> = overview.runtimes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["claude-code", "codex"]);
        assert!(overview.sessions.is_empty());
        assert!(overview.runtimes.iter().all(|r| !r.ready));
    }

    #[test]
    fn agent_runtimes_not_installed_refuse_work_clearly() {
        let app = app();
        let main = window(&app, "main");
        // Tests see no installed runtimes (hermetic: no real CLI is ever started).
        let runtimes: Vec<plenipo_runtime::agent::AgentRuntimeInfo> =
            body(invoke(&main, "refresh_agent_runtimes"));
        assert!(runtimes
            .iter()
            .all(|r| r.installation.state == plenipo_runtime::agent::InstallState::NotInstalled));
        let err = invoke_json(
            &main,
            "start_agent_session",
            serde_json::json!({ "runtimeId": "codex", "objective": "hello" }),
        )
        .expect_err("not installed");
        assert_eq!(err["kind"], "invalidInput");
        assert!(
            err["message"].as_str().unwrap().contains("not available"),
            "{err}"
        );
        let status: plenipo_ledger::LedgerStatus = body(invoke(&main, "get_ledger_status"));
        assert_eq!(status.task_count, 0, "a refused turn records nothing");
    }

    #[test]
    fn agent_commands_validate_input_and_accept_no_commands_or_paths() {
        let app = app();
        let main = window(&app, "main");
        for args in [
            serde_json::json!({ "runtimeId": "../claude", "objective": "x" }),
            serde_json::json!({ "runtimeId": "gemini", "objective": "x" }),
            serde_json::json!({ "runtimeId": "codex", "objective": "   " }),
            serde_json::json!({ "runtimeId": "codex", "objective": "x".repeat(40_001) }),
            serde_json::json!({ "runtimeId": "codex", "objective": "x", "model": "--yolo" }),
            // Extra fields cannot influence the launch.
            serde_json::json!({
                "runtimeId": "codex", "objective": " ", "executable": "/bin/sh",
                "args": ["-c", "echo pwned"], "workingDir": "/"
            }),
        ] {
            let err = invoke_json(&main, "start_agent_session", args.clone()).expect_err("bad");
            assert_eq!(err["kind"], "invalidInput", "{args}: {err}");
        }
        for cmd in [
            "get_agent_session",
            "cancel_agent_turn",
            "close_agent_session",
            "resume_agent_session",
        ] {
            let err = invoke_json(
                &main,
                cmd,
                serde_json::json!({ "sessionId": "../x", "objective": "hi" }),
            )
            .expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}");
            let err = invoke_json(
                &main,
                cmd,
                serde_json::json!({ "sessionId": SESSION, "objective": "hi" }),
            )
            .expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}: {err}");
        }
    }

    #[test]
    fn agent_commands_denied_for_ungranted_windows_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        for cmd in ["get_agent_overview", "refresh_agent_runtimes"] {
            assert!(invoke(&other, cmd).is_err(), "{cmd}");
            assert!(
                invoke_from(&main, cmd, "https://example.com").is_err(),
                "{cmd}"
            );
        }
        assert!(invoke_json(
            &other,
            "start_agent_session",
            serde_json::json!({ "runtimeId": "codex", "objective": "hi" })
        )
        .is_err());
    }
}
