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

use plenipo_liaison::{Liaison, LiaisonConfig};
use plenipo_router::Router;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use plenipo_workforce::Workforce;
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
                ledger.clone(),
                supervisor.clone(),
            );
            if options.persistence == Persistence::AppData {
                agent_host::detect_in_background(&agents);
            }
            // Liaison (Phase 4): handoffs between workers, reconciled from the Ledger.
            let liaison = Liaison::new(ledger.clone(), agents.clone(), LiaisonConfig::default());
            tauri::async_runtime::spawn(liaison.clone().run());
            // Router (Phase 6): model registry and role model policies.
            let router = Router::new(ledger.clone(), agents.clone());
            // Workforce (Phase 5): the organization, and Liaison's directory for its members,
            // whose workers the Router places.
            let workforce = Workforce::new(ledger, agents.clone(), liaison.clone(), router.clone());
            app.manage(supervisor);
            app.manage(agents);
            app.manage(liaison);
            app.manage(router);
            app.manage(workforce);
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
            commands::close_agent_session,
            commands::get_task_handoffs,
            commands::get_task_tree,
            commands::get_liaison_overview,
            commands::get_organization,
            commands::get_work,
            commands::rename_organization,
            commands::set_organization_titles,
            commands::create_role,
            commands::create_department,
            commands::update_department,
            commands::remove_department,
            commands::create_project,
            commands::update_project,
            commands::archive_project,
            commands::hire_position,
            commands::fill_position,
            commands::vacate_position,
            commands::update_position,
            commands::move_position,
            commands::archive_position,
            commands::assign_oversight,
            commands::end_oversight,
            commands::give_objective,
            commands::get_routing,
            commands::save_model,
            commands::remove_model,
            commands::set_role_policy,
            commands::set_routing_options,
            commands::clear_usage_limit,
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
                // Liaison stops handing out work first. The agent runtime then stops its turns
                // through the supervisor and records their results (waiting turns stay as
                // recorded; the next start marks them interrupted); the supervisor then has
                // nothing left to stop.
                if let Some(liaison) = app.try_state::<Liaison>() {
                    liaison.shutdown();
                }
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
            ledger.clone(),
            supervisor.clone(),
        );
        // Queries only: the reconciliation loop is not needed without running workers.
        let liaison = Liaison::new(ledger.clone(), agents.clone(), LiaisonConfig::default());
        let router = Router::new(ledger.clone(), agents.clone());
        let workforce = Workforce::new(ledger, agents.clone(), liaison.clone(), router.clone());
        app.manage(supervisor);
        app.manage(agents);
        app.manage(liaison);
        app.manage(router);
        app.manage(workforce);
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

    // ---- Liaison (Phase 4) -----------------------------------------------------------------

    #[test]
    fn liaison_overview_reports_protocol_limits_and_destinations() {
        let app = app();
        let main = window(&app, "main");
        let overview: plenipo_liaison::LiaisonOverview =
            body(invoke(&main, "get_liaison_overview"));
        assert_eq!(overview.protocol, "plenipo-liaison/1");
        assert_eq!(overview.context_format, "plenipo-context/1");
        assert_eq!(
            (
                overview.limits.max_depth,
                overview.limits.max_requests_per_answer,
                overview.limits.max_rounds,
                overview.limits.max_workflow_handoffs
            ),
            (3, 3, 5, 12)
        );
        let addresses: Vec<_> = overview
            .destinations
            .iter()
            .map(|d| d.address.as_str())
            .collect();
        assert_eq!(addresses, ["claude-code", "codex"]);
        assert!(overview.destinations.iter().all(|d| !d.ready));
        assert_eq!(overview.open_handoffs, 0);
    }

    #[test]
    fn task_handoffs_and_trees_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let parent = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "owner".into(),
                    objective: "plan the release".into(),
                    ..Default::default()
                },
                "owner",
            )
            .unwrap();
        let child = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    parent_task_id: Some(parent.id.clone()),
                    requested_by: "agent:codex".into(),
                    objective: "review the plan".into(),
                    ..Default::default()
                },
                "owner",
            )
            .unwrap();
        let handoffs: plenipo_liaison::TaskHandoffs = body(invoke_json(
            &main,
            "get_task_handoffs",
            serde_json::json!({ "taskId": parent.id }),
        ));
        assert_eq!(handoffs.task_id, parent.id);
        assert!(handoffs.sent.is_empty() && handoffs.received.is_none());
        let tree: plenipo_liaison::TaskTree = body(invoke_json(
            &main,
            "get_task_tree",
            serde_json::json!({ "taskId": child.id }),
        ));
        assert_eq!(
            (tree.root_id.as_str(), tree.focus_id.as_str()),
            (parent.id.as_str(), child.id.as_str())
        );
        let depths: Vec<_> = tree.nodes.iter().map(|n| n.depth).collect();
        assert_eq!(depths, [0, 1]);
        for cmd in ["get_task_handoffs", "get_task_tree"] {
            let err = invoke_json(&main, cmd, serde_json::json!({ "taskId": "../../etc" }))
                .expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}");
            let err =
                invoke_json(&main, cmd, serde_json::json!({ "taskId": SESSION })).expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}: {err}");
            assert!(
                invoke_json(&main, cmd, serde_json::json!({})).is_err(),
                "{cmd}"
            );
        }
    }

    #[test]
    fn sessions_that_allow_handoffs_are_refused_like_any_other_when_not_installed() {
        let app = app();
        let main = window(&app, "main");
        let err = invoke_json(
            &main,
            "start_agent_session",
            serde_json::json!({ "runtimeId": "codex", "objective": "hello", "handoffs": true }),
        )
        .expect_err("not installed");
        assert_eq!(err["kind"], "invalidInput");
        assert!(
            err["message"].as_str().unwrap().contains("not available"),
            "{err}"
        );
        // The flag is a boolean; nothing else is accepted in its place.
        assert!(invoke_json(
            &main,
            "start_agent_session",
            serde_json::json!({ "runtimeId": "codex", "objective": "hello", "handoffs": "yes" }),
        )
        .is_err());
        let status: plenipo_ledger::LedgerStatus = body(invoke(&main, "get_ledger_status"));
        assert_eq!(status.task_count, 0, "a refused turn records nothing");
    }

    #[test]
    fn liaison_commands_denied_for_ungranted_windows_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let args = serde_json::json!({ "taskId": SESSION });
        for cmd in ["get_liaison_overview", "get_task_handoffs", "get_task_tree"] {
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_with(&main, cmd, args.clone(), "https://example.com").is_err(),
                "{cmd}"
            );
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

    // ---- Workforce (Phase 5) ---------------------------------------------------------------

    fn role_id(snapshot: &plenipo_workforce::OrgSnapshot, name: &str) -> String {
        snapshot
            .roles
            .iter()
            .find(|r| r.name == name)
            .unwrap()
            .id
            .clone()
    }

    #[test]
    fn the_organization_starts_empty_with_role_templates() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        assert_eq!(org.name, "Organization");
        assert!(org.departments.is_empty() && org.positions.is_empty());
        assert!(org.roles.iter().filter(|r| r.template).count() >= 10);
        let runtimes: Vec<_> = org.runtimes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(runtimes, ["claude-code", "codex"]);
        let work: plenipo_workforce::WorkView = body(invoke(&main, "get_work"));
        assert!(work.running.is_empty() && work.position_id.is_none());
    }

    #[test]
    fn the_organization_is_built_and_changed_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let lead = |role: &str, title: &str| {
            serde_json::json!({
                "roleId": role_id(&org, role), "title": title, "runtimeId": "claude-code"
            })
        };
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Development", "description": "Builds the products",
                "head": lead("Manager", "Development Manager"),
            }}),
        ));
        let dept = s.departments[0].clone();
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_project",
            serde_json::json!({ "input": {
                "name": "Cloudline", "description": "",
                "repositoryUrl": "https://github.com/example/cloudline",
                "localPath": "D:\\projects\\cloudline",
                "allowedRuntimes": ["claude-code", "codex"],
                "capabilityProfile": "development",
                "departmentId": dept.id,
                "coordinator": lead("Supervisor", "Cloudline Supervisor"),
            }}),
        ));
        let coordinator = s.projects[0].coordinator_position_id.clone().unwrap();
        let hire = |title: &str, role: &str, to: &str, runtime: &str| {
            let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
                &main,
                "hire_position",
                serde_json::json!({ "input": {
                    "roleId": role_id(&org, role), "title": title,
                    "reportsTo": to, "runtimeId": runtime,
                }}),
            ));
            s.positions
                .iter()
                .find(|p| p.title == title)
                .unwrap()
                .id
                .clone()
        };
        let dev = hire(
            "Senior Developer",
            "Senior Developer",
            &coordinator,
            "codex",
        );
        let head = dept.head_position_id.clone().unwrap();
        let qa = hire("QA Engineer", "QA Engineer", &head, "claude-code");
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "assign_oversight",
            serde_json::json!({ "overseerId": qa, "targetId": coordinator, "role": "qa" }),
        ));
        assert_eq!(s.oversight.len(), 1);
        let oversight = s.oversight[0].id.clone();
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "update_position",
            serde_json::json!({ "positionId": dev, "input": { "title": "Backend Developer" } }),
        ));
        assert!(s.positions.iter().any(|p| p.title == "Backend Developer"));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "move_position",
            serde_json::json!({ "positionId": dev, "reportsTo": head }),
        ));
        let moved = s.positions.iter().find(|p| p.id == dev).unwrap();
        assert_eq!(moved.reports_to.as_deref(), Some(head.as_str()));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "set_organization_titles",
            serde_json::json!({ "titles": "marineCorps" }),
        ));
        assert_eq!(s.titles, plenipo_workforce::TitleTheme::MarineCorps);
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "rename_organization",
            serde_json::json!({ "name": "8 West Ventures" }),
        ));
        assert_eq!(s.name, "8 West Ventures");
        assert_eq!(
            s.titles,
            plenipo_workforce::TitleTheme::MarineCorps,
            "renaming keeps the titles"
        );
        let work: plenipo_workforce::WorkView = body(invoke_json(
            &main,
            "get_work",
            serde_json::json!({ "positionId": coordinator }),
        ));
        assert_eq!(work.position_id.as_deref(), Some(coordinator.as_str()));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "end_oversight",
            serde_json::json!({ "oversightId": oversight }),
        ));
        assert!(s.oversight.is_empty());
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "archive_position",
            serde_json::json!({ "positionId": qa }),
        ));
        assert!(!s.positions.iter().find(|p| p.id == qa).unwrap().active);
        // The structure rules come back as clear refusals.
        let err = invoke_json(
            &main,
            "archive_position",
            serde_json::json!({ "positionId": coordinator }),
        )
        .expect_err("coordinates a project");
        assert_eq!(err["kind"], "invalidInput");
        assert!(err["message"]
            .as_str()
            .unwrap()
            .contains("archive the project"));
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let events: Vec<String> = ledger
            .recent_events(200)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        for want in [
            "org.department_created",
            "org.project_created",
            "org.position_created",
            "org.position_moved",
            "org.oversight_assigned",
            "org.oversight_ended",
            "org.settings_changed",
        ] {
            assert!(events.iter().any(|e| e == want), "{want}");
        }
    }

    #[test]
    fn workforce_commands_validate_input_and_accept_no_extra_fields() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let worker = role_id(&org, "Senior Developer");
        for (cmd, args) in [
            ("fill_position", serde_json::json!({ "positionId": "../x" })),
            (
                "get_work",
                serde_json::json!({ "positionId": "C:\\Windows" }),
            ),
            (
                "move_position",
                serde_json::json!({ "positionId": SESSION, "reportsTo": "../x" }),
            ),
            (
                "hire_position",
                serde_json::json!({ "input": {
                    "roleId": worker, "title": "x", "reportsTo": null, "runtimeId": "Claude Code"
                }}),
            ),
            (
                "hire_position",
                serde_json::json!({ "input": {
                    "roleId": worker, "title": "x".repeat(8_001), "reportsTo": null,
                    "runtimeId": "codex"
                }}),
            ),
            (
                // Extra fields cannot smuggle in a command, path, or session.
                "hire_position",
                serde_json::json!({ "input": {
                    "roleId": worker, "title": "x", "reportsTo": null, "runtimeId": "codex",
                    "executable": "/bin/sh", "sessionId": SESSION
                }}),
            ),
            (
                "assign_oversight",
                serde_json::json!({ "overseerId": SESSION, "targetId": SESSION, "role": "admin" }),
            ),
            (
                "create_project",
                serde_json::json!({ "input": {
                    "name": "p", "description": "", "allowedRuntimes": ["../codex"]
                }}),
            ),
            (
                "give_objective",
                serde_json::json!({ "positionId": "../x", "objective": "hi" }),
            ),
            // Only the known title themes.
            (
                "set_organization_titles",
                serde_json::json!({ "titles": "starfleet" }),
            ),
            (
                "give_objective",
                serde_json::json!({ "positionId": SESSION, "objective": "x".repeat(40_001) }),
            ),
        ] {
            let err = invoke_json(&main, cmd, args.clone()).expect_err(cmd);
            // Malformed arguments are refused before any command code runs.
            assert!(
                err["kind"] == "invalidInput" || err.is_string(),
                "{cmd} {args}: {err}"
            );
        }
        // Unknown positions are reported, not created.
        let err = invoke_json(
            &main,
            "fill_position",
            serde_json::json!({ "positionId": SESSION }),
        )
        .expect_err("unknown");
        assert_eq!(err["kind"], "invalidInput");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        assert!(org.positions.is_empty());
    }

    #[test]
    fn objectives_to_an_uninstalled_runtime_are_refused_and_record_nothing() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Development", "description": "",
                "head": { "roleId": role_id(&org, "Manager"),
                          "title": "Development Manager", "runtimeId": "claude-code" },
            }}),
        ));
        let head = s.departments[0].head_position_id.clone().unwrap();
        let err = invoke_json(
            &main,
            "give_objective",
            serde_json::json!({ "positionId": head, "objective": "Plan the quarter" }),
        )
        .expect_err("not installed");
        assert_eq!(err["kind"], "invalidInput");
        assert!(
            err["message"].as_str().unwrap().contains("not available"),
            "{err}"
        );
        let status: plenipo_ledger::LedgerStatus = body(invoke(&main, "get_ledger_status"));
        assert_eq!(status.task_count, 0, "a refused objective records nothing");
    }

    #[test]
    fn workforce_commands_denied_for_ungranted_windows_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        for cmd in [
            "get_organization",
            "get_work",
            "create_department",
            "hire_position",
            "move_position",
            "assign_oversight",
            "give_objective",
            "get_routing",
            "save_model",
            "remove_model",
            "set_role_policy",
            "set_routing_options",
            "clear_usage_limit",
        ] {
            let args = serde_json::json!({ "positionId": SESSION, "objective": "x" });
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    // ---- Model policy and routing (Phase 6) ----------------------------------------------

    fn model_id(s: &plenipo_router::RoutingSnapshot, label: &str) -> String {
        s.models
            .iter()
            .find(|m| m.label == label)
            .unwrap()
            .id
            .clone()
    }

    #[test]
    fn model_policy_is_configured_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let s: plenipo_router::RoutingSnapshot = body(invoke(&main, "get_routing"));
        // Each AI tool's default model is listed; nothing is signed in, so nothing is chosen.
        let labels: Vec<&str> = s.models.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Claude Code (default model)", "Codex (default model)"]
        );
        assert!(s.tools.iter().all(|t| !t.available));
        assert!(!s.api_billing);
        let designer = s.roles.iter().find(|r| r.role_name == "Designer").unwrap();
        assert_eq!(
            designer.policy.needs.len(),
            2,
            "the template's starting policy"
        );
        assert!(designer.next.choice.is_none());
        let s: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "save_model",
            serde_json::json!({ "input": {
                "runtimeId": "claude-code", "name": "opus", "label": "Opus",
                "features": ["vision"], "contextTokens": 200000, "cost": "premium",
            }}),
        ));
        let opus = model_id(&s, "Opus");
        let codex = model_id(&s, "Codex (default model)");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let dev = role_id(&org, "Senior Developer");
        let s: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "set_role_policy",
            serde_json::json!({ "roleId": dev, "policy": {
                "models": [opus, codex], "needs": [], "minContextTokens": null,
                "neverCompanies": ["openai"], "cost": "any", "crossCompany": "prefer",
            }}),
        ));
        let view = s.roles.iter().find(|r| r.role_id == dev).unwrap();
        assert_eq!(view.policy.models.len(), 2);
        assert!(view
            .next
            .reason
            .starts_with("No model can take Senior Developer's work now"));
        let s: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "set_routing_options",
            serde_json::json!({ "options": { "onUsageLimit": "nextChoice" } }),
        ));
        assert_eq!(
            s.options.on_usage_limit,
            plenipo_router::LimitBehavior::NextChoice
        );
        let _: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "clear_usage_limit",
            serde_json::json!({ "runtimeId": "codex" }),
        ));
        let s: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "remove_model",
            serde_json::json!({ "modelId": opus }),
        ));
        assert_eq!(
            s.roles
                .iter()
                .find(|r| r.role_id == dev)
                .unwrap()
                .policy
                .models,
            std::slice::from_ref(&codex)
        );
        // Built-in entries stay; the refusal says why.
        let err = invoke_json(
            &main,
            "remove_model",
            serde_json::json!({ "modelId": codex }),
        )
        .expect_err("built in");
        assert_eq!(err["kind"], "invalidInput");
        // Positions hired without an AI tool follow their role's policy; "" makes one automatic.
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Development", "description": "",
                "head": { "roleId": role_id(&org, "Manager"), "title": "Development Manager" },
            }}),
        ));
        let head = s.positions[0].clone();
        assert!(head.automatic && head.runtime_id.is_none());
        assert!(head.route.unwrap().choice.is_none(), "nothing is signed in");
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "hire_position",
            serde_json::json!({ "input": {
                "roleId": dev, "title": "Fixed Developer", "reportsTo": head.id,
                "runtimeId": "codex",
            }}),
        ));
        let fixed = s
            .positions
            .iter()
            .find(|p| p.title == "Fixed Developer")
            .unwrap();
        assert!(!fixed.automatic);
        assert!(fixed.route.as_ref().unwrap().fixed);
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "update_position",
            serde_json::json!({ "positionId": fixed.id, "input": { "runtimeId": "" } }),
        ));
        assert!(
            s.positions
                .iter()
                .find(|p| p.id == fixed.id)
                .unwrap()
                .automatic
        );
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let events: Vec<String> = ledger
            .recent_events(200)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        for want in [
            "router.models_added",
            "router.policies_added",
            "router.model_saved",
            "router.policy_changed",
            "router.options_changed",
            "router.limit_cleared",
            "router.model_removed",
        ] {
            assert!(events.iter().any(|e| e == want), "{want}");
        }
    }

    #[test]
    fn routing_commands_validate_input_and_accept_no_extra_fields() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let dev = role_id(&org, "Senior Developer");
        let model = |extra: serde_json::Value| {
            let mut input = serde_json::json!({
                "runtimeId": "codex", "name": "gpt-x", "label": "GPT",
                "features": [], "cost": "standard",
            });
            for (k, v) in extra.as_object().unwrap() {
                input[k] = v.clone();
            }
            serde_json::json!({ "input": input })
        };
        let policy = |extra: serde_json::Value| {
            let mut p = serde_json::json!({ "models": [] });
            for (k, v) in extra.as_object().unwrap() {
                p[k] = v.clone();
            }
            serde_json::json!({ "roleId": dev, "policy": p })
        };
        for (cmd, args) in [
            (
                "save_model",
                model(serde_json::json!({ "runtimeId": "../codex" })),
            ),
            (
                "save_model",
                model(serde_json::json!({ "runtimeId": "gemini" })),
            ),
            (
                "save_model",
                model(serde_json::json!({ "name": "--dangerously-skip" })),
            ),
            (
                "save_model",
                model(serde_json::json!({ "name": "C:\\model" })),
            ),
            (
                "save_model",
                model(serde_json::json!({ "features": ["mindReading"] })),
            ),
            ("save_model", model(serde_json::json!({ "id": "../x" }))),
            // Extra fields cannot smuggle in an executable or arguments.
            (
                "save_model",
                model(serde_json::json!({ "executable": "/bin/sh" })),
            ),
            ("remove_model", serde_json::json!({ "modelId": "../x" })),
            ("remove_model", serde_json::json!({ "modelId": SESSION })),
            (
                "set_role_policy",
                policy(serde_json::json!({ "models": ["../x"] })),
            ),
            (
                "set_role_policy",
                policy(serde_json::json!({ "models": [SESSION] })),
            ),
            (
                "set_role_policy",
                policy(serde_json::json!({ "neverCompanies": ["Open AI"] })),
            ),
            (
                "set_role_policy",
                policy(serde_json::json!({ "runtimeId": "codex" })),
            ),
            (
                "set_role_policy",
                serde_json::json!({ "roleId": SESSION, "policy": { "models": [] } }),
            ),
            (
                "set_routing_options",
                serde_json::json!({ "options": { "onUsageLimit": "useApiBilling" } }),
            ),
            (
                "set_routing_options",
                serde_json::json!({ "options": { "apiBilling": true } }),
            ),
            (
                "clear_usage_limit",
                serde_json::json!({ "runtimeId": "Claude Code" }),
            ),
            (
                "clear_usage_limit",
                serde_json::json!({ "runtimeId": "gemini" }),
            ),
        ] {
            let err = invoke_json(&main, cmd, args.clone()).expect_err(cmd);
            assert!(
                err["kind"] == "invalidInput" || err.is_string(),
                "{cmd} {args}: {err}"
            );
        }
        // Nothing was changed by the refusals.
        let s: plenipo_router::RoutingSnapshot = body(invoke(&main, "get_routing"));
        assert_eq!(s.models.len(), 2);
        assert_eq!(
            s.options.on_usage_limit,
            plenipo_router::LimitBehavior::Wait
        );
    }
}
