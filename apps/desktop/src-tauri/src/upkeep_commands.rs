//! Phase 13 commands: recovery, starting and closing, backups and restore, the diagnostics
//! file, and updates (ADR-037, background work; ADR-038, updates). Every one is the main
//! window's alone (capabilities/default.json): the sign and web pages are refused.
//!
//! None takes a path: a backup is named from the list Core gives, the diagnostics file goes
//! where Core chooses, and an update comes only from the address built into this copy.

use std::sync::Arc;
use std::time::Duration;

use plenipo_core::{
    CommandError, DiagnosticsFile, RecoveryStatus, StartAndClose, StartAndCloseInput, UpdateState,
    UpdateStatus,
};
use plenipo_guard::Guard;
use plenipo_ledger::{Ledger, LedgerBackups, NewEvent, NewTask, TaskState};
use plenipo_liaison::Liaison;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use plenipo_workforce::Workforce;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager as _, Runtime, State};

use crate::commands::{
    ledger_error, to_command_error, validate_task_id, with_ledger, workforce_error,
};
use crate::orgs::Org;
use crate::recovery::{self, RecoveryState, RunAgain};
use crate::runtime_host::Persistence;
use crate::smoke::{Scenario, SmokeTest, EXIT_READY};
use crate::update_host::{self, Updates};
use crate::window_watch::{self, WindowWatch};
use crate::{settings_health, start_close, SettingsProblems};

const OWNER: &str = "owner";

/// An AI tool's name from its ID ("claude-code" → "Claude Code").
fn tool_names(agents: Option<&AgentRuntime>) -> impl Fn(&str) -> String {
    let runtimes = agents.map(AgentRuntime::runtimes).unwrap_or_default();
    move |id: &str| {
        runtimes
            .iter()
            .find(|r| r.id == id)
            .map_or_else(|| id.to_owned(), |r| r.label.clone())
    }
}

/// The window shows the first organization (a window with no organization of its own is the
/// first one's).
fn shows_first(stack: Option<&crate::orgs::OrgStack>) -> bool {
    stack.is_none_or(|s| s.id() == crate::orgs::FIRST)
}

/// What recovery has to tell window `label` (its organization's; none: the first one's).
pub(crate) fn recovery_status<R: Runtime>(
    app: &AppHandle<R>,
    label: Option<&str>,
) -> Result<RecoveryStatus, CommandError> {
    let stack = label.and_then(|l| crate::orgs::stack_of(app, l));
    let ledger = stack.as_ref().map_or_else(
        || app.state::<Arc<Ledger>>().inner().clone(),
        |s| s.ledger.clone(),
    );
    let state = app.state::<Arc<RecoveryState>>();
    // Settings Plenipo could not read are checked in the first organization only (ADR-094,
    // limits): only its windows say so, and only there can they be reset.
    let problems = if shows_first(stack.as_deref()) {
        app.state::<SettingsProblems>()
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    } else {
        Vec::new()
    };
    let agents = stack
        .as_ref()
        .map(|s| s.agents.clone())
        .or_else(|| app.try_state::<AgentRuntime>().map(|a| a.inner().clone()));
    let tool = tool_names(agents.as_ref());
    recovery::status(&ledger, &state, problems, &tool).map_err(ledger_error)
}

/// One organization's notice that Plenipo stopped unexpectedly, and what stopped with it, if it
/// has one (Phase 14: a phone sees each organization's).
pub(crate) fn org_recovery<R: Runtime>(
    app: &AppHandle<R>,
    stack: &crate::orgs::OrgStack,
) -> Result<Option<plenipo_core::Recovery>, CommandError> {
    let state = app.state::<Arc<RecoveryState>>();
    let tool = tool_names(Some(&stack.agents));
    recovery::status(&stack.ledger, &state, Vec::new(), &tool)
        .map(|s| s.recovery)
        .map_err(ledger_error)
}

async fn recovery_status_off_thread<R: Runtime>(
    app: &AppHandle<R>,
    label: Option<String>,
) -> Result<RecoveryStatus, CommandError> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || recovery_status(&app, label.as_deref()))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
}

// ---- Recovery ------------------------------------------------------------------------------

/// What recovery has to tell the owner: how the last run ended and what stopped, a window that
/// was brought back, and settings that could not be read.
#[tauri::command]
pub async fn get_recovery_status<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
) -> Result<RecoveryStatus, CommandError> {
    recovery_status_off_thread(&app, Some(window.label().to_owned())).await
}

/// Run a stopped objective again: the same objective to the same worker, through the normal
/// checks (routing, permissions). Only an objective that stopped or failed can be.
#[tauri::command]
pub async fn run_again<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
    workforce: Org<'_, Workforce>,
    liaison: Org<'_, Liaison>,
    task_id: String,
) -> Result<RecoveryStatus, CommandError> {
    run_again_core(ledger.inner(), workforce.inner(), liaison.inner(), &task_id).await?;
    recovery_status_off_thread(&app, Some(window.label().to_owned())).await
}

/// Run a stopped objective again (the main window's **Run again**, and a phone's, Phase 14): the
/// same objective to the same worker, through the normal checks, recorded as run again.
pub(crate) async fn run_again_core(
    ledger: &Arc<Ledger>,
    workforce: &Workforce,
    liaison: &Liaison,
    task_id: &str,
) -> Result<(), CommandError> {
    validate_task_id(task_id)?;
    let id = task_id.to_owned();
    let l = Arc::clone(ledger);
    let task = tauri::async_runtime::spawn_blocking(move || {
        l.task(&id)?
            .ok_or_else(|| plenipo_ledger::LedgerError::NotFound(format!("task {id}")))
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(|e| CommandError::invalid_input(e.to_string()))?;
    let plan = recovery::run_again_plan(&task).map_err(CommandError::invalid_input)?;
    let detail = match plan {
        RunAgain::Position {
            position_id,
            project_id,
            objective,
        } => workforce
            .give_objective(&position_id, &objective, project_id.as_deref())
            .await
            .map_err(workforce_error)?,
        RunAgain::Conversation {
            session_id,
            objective,
        } => liaison
            .resume_session(&session_id, &objective)
            .await
            .map_err(to_command_error)?,
    };
    let new_task = detail.turns.last().map(|t| t.task_id.clone());
    let l = Arc::clone(ledger);
    let from = task_id.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        recovery::record_run_again(&l, &from, new_task.as_deref());
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))
}

/// Leave the stopped tasks stopped: the notice about the last run goes away.
#[tauri::command]
pub async fn dismiss_recovery<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
    id: String,
) -> Result<RecoveryStatus, CommandError> {
    crate::commands::validate_id("recovery", &id)?;
    with_ledger(&ledger, move |l| recovery::dismiss(l, &id)).await?;
    recovery_status_off_thread(&app, Some(window.label().to_owned())).await
}

/// The owner read that the window was brought back.
#[tauri::command]
pub async fn dismiss_window_recovery<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    state: State<'_, Arc<RecoveryState>>,
) -> Result<RecoveryStatus, CommandError> {
    state.dismiss_window();
    recovery_status_off_thread(&app, Some(window.label().to_owned())).await
}

/// The window's page is alive (every few seconds; ADR-037 item 3). `visible`: the page can be
/// seen (WebView2 may slow a hidden page down).
///
/// Only Plenipo's own window is watched: another organization's window saying it is alive
/// never hides that this one stopped.
#[tauri::command]
pub async fn window_alive<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    watch: State<'_, Arc<WindowWatch>>,
    state: State<'_, Arc<RecoveryState>>,
    ledger: State<'_, Arc<Ledger>>,
    visible: bool,
) -> Result<(), CommandError> {
    if window.label() != crate::workspace_windows::MAIN {
        return Ok(());
    }
    if let Some(reopened) = watch.alive(visible, plenipo_ledger::now_ms()) {
        let (ledger, state) = (Arc::clone(&ledger), Arc::clone(&state));
        tauri::async_runtime::spawn_blocking(move || {
            window_watch::record(&ledger, &state, reopened);
        });
    }
    Ok(())
}

/// Reset settings Plenipo could not read to their starting values (after a backup of the
/// Ledger, which keeps the damaged ones).
#[tauri::command]
pub async fn reset_settings<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
    guard: Org<'_, Guard>,
    agents: Org<'_, AgentRuntime>,
    key: String,
) -> Result<RecoveryStatus, CommandError> {
    if key.len() > 64 {
        return Err(CommandError::invalid_input("unknown settings"));
    }
    if !shows_first(crate::orgs::stack_of(&app, window.label()).as_deref()) {
        return Err(CommandError::invalid_input(
            "Reset these settings from your first organization's window.",
        ));
    }
    let (l, g, k) = (Arc::clone(&ledger), guard.inner().clone(), key.clone());
    tauri::async_runtime::spawn_blocking(move || settings_health::reset(&l, &g, &k))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
        .map_err(CommandError::invalid_input)?;
    // A spending caps reset switches paid AI keys off: the paid AI tools say so at once.
    if key == settings_health::SPENDING {
        crate::spending_commands::recheck_paid_tools(&agents);
    }
    app.state::<SettingsProblems>()
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|p| p.key != key);
    recovery_status_off_thread(&app, Some(window.label().to_owned())).await
}

// ---- Start and close (ADR-037) ---------------------------------------------------------------

/// Settings → Start and close.
#[tauri::command]
pub async fn get_start_and_close<R: Runtime>(
    app: AppHandle<R>,
    ledger: State<'_, Arc<Ledger>>,
) -> Result<StartAndClose, CommandError> {
    let l = Arc::clone(&ledger);
    tauri::async_runtime::spawn_blocking(move || start_close::settings(&app, &l))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))
}

/// Change Settings → Start and close.
#[tauri::command]
pub async fn set_start_and_close<R: Runtime>(
    app: AppHandle<R>,
    ledger: State<'_, Arc<Ledger>>,
    input: StartAndCloseInput,
) -> Result<StartAndClose, CommandError> {
    let l = Arc::clone(&ledger);
    tauri::async_runtime::spawn_blocking(move || {
        start_close::set_close_window(&l, input.close_window).map_err(ledger_error)?;
        match start_close::start_with_windows(&app) {
            Some(on) if on != input.start_with_windows => {
                start_close::set_start_with_windows(&app, input.start_with_windows)
                    .map_err(CommandError::invalid_input)?;
            }
            None if input.start_with_windows => {
                return Err(CommandError::invalid_input(
                    "Starting with Windows is not available on this computer.",
                ));
            }
            _ => {}
        }
        Ok(start_close::settings(&app, &l))
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}

// ---- Backups and restore -----------------------------------------------------------------------

/// The Ledger's file: the open one's, or, when Plenipo runs on a temporary Ledger because it
/// could not open its own (one from a newer Plenipo, after going back to an older version), that
/// file, so its backups can still be listed and restored.
///
/// Always the file of window `label`'s own organization, never another's (ADR-094 §10).
fn ledger_file<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    ledger: &Ledger,
) -> Option<std::path::PathBuf> {
    if let Some(path) = ledger.path() {
        return Some(path.to_owned());
    }
    let kept = matches!(
        app.try_state::<Persistence>().as_deref(),
        Some(Persistence::AppData)
    );
    let own = match crate::orgs::stack_of(app, label) {
        Some(stack) => stack
            .place
            .folder
            .as_deref()
            .map(crate::ledger_host::ledger_file),
        None => crate::ledger_host::ledger_path(app).ok(),
    };
    kept.then_some(own).flatten().filter(|p| p.exists())
}

fn overview(file: Option<&std::path::Path>) -> Result<LedgerBackups, plenipo_ledger::LedgerError> {
    match file {
        Some(path) => plenipo_ledger::backups::overview_at(path),
        None => Ok(LedgerBackups {
            backups: Vec::new(),
            pending_restore: None,
            folder: None,
        }),
    }
}

/// Every backup of the Ledger, newest first, and a restore waiting for the next start.
#[tauri::command]
pub async fn list_ledger_backups<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
) -> Result<LedgerBackups, CommandError> {
    let file = ledger_file(&app, window.label(), &ledger);
    with_ledger(&ledger, move |_| overview(file.as_deref())).await
}

/// Restore the Ledger from one of its backups, named from the list (never a path). Plenipo
/// keeps the Ledger as it is now as a backup, then restarts to restore it: running work stops
/// the normal way first.
#[tauri::command]
pub async fn restore_ledger_backup<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
    name: String,
) -> Result<LedgerBackups, CommandError> {
    if name.len() > 200 {
        return Err(CommandError::invalid_input(
            "that is not one of the Ledger's backups",
        ));
    }
    let file = ledger_file(&app, window.label(), &ledger);
    let overview = with_ledger(&ledger, move |l| {
        let path = file.as_deref().ok_or_else(|| {
            plenipo_ledger::LedgerError::InvalidInput(
                "a temporary Ledger has no backups to restore".into(),
            )
        })?;
        let backup = plenipo_ledger::backups::request_restore(path, &name)?;
        l.append_event(NewEvent {
            source: OWNER.into(),
            event_type: "ledger.restore_requested".into(),
            payload: json!({ "backup": backup.name }),
            ..NewEvent::default()
        })?;
        overview(Some(path))
    })
    .await?;
    log::warn!("a restore of the Ledger was asked for; Plenipo restarts to do it");
    restart_plenipo(&app);
    Ok(overview)
}

/// Forget a restore that was asked for but has not happened yet.
#[tauri::command]
pub async fn cancel_ledger_restore<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    ledger: Org<'_, Arc<Ledger>>,
) -> Result<LedgerBackups, CommandError> {
    let file = ledger_file(&app, window.label(), &ledger);
    with_ledger(&ledger, move |_| {
        if let Some(path) = file.as_deref() {
            plenipo_ledger::backups::cancel_restore(path)?;
        }
        overview(file.as_deref())
    })
    .await
}

/// Stop the work the normal way, then start Plenipo again.
fn restart_plenipo<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let the answer reach the window first.
        tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_millis(500)))
            .await
            .ok();
        crate::stop_work(&app).await;
        crate::mark_stopped(&app);
        if let Ok(data) = app.path().app_local_data_dir() {
            start_close::show_after_restart(&data);
        }
        app.request_restart();
    });
}

// ---- The diagnostics file ----------------------------------------------------------------------

/// Save a diagnostics file (in Plenipo's own folder) to send when something went wrong. It
/// holds no task text, no answers, no terminal input, and no secrets.
#[tauri::command]
pub async fn save_diagnostics_file<R: Runtime>(
    app: AppHandle<R>,
    persistence: State<'_, Persistence>,
) -> Result<DiagnosticsFile, CommandError> {
    if *persistence != Persistence::AppData {
        return Err(CommandError::invalid_input(
            "diagnostics files are saved only when Plenipo keeps its files",
        ));
    }
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| CommandError::internal(e.to_string()))?
        .join("diagnostics");
    let about = about(&app).await;
    let ledger = Arc::clone(&app.state::<Arc<Ledger>>());
    let filter = app
        .try_state::<plenipo_capabilities::Broker>()
        .map(|b| b.text_filter())
        .unwrap_or_else(|| {
            let r = plenipo_guard::Redactor::default();
            Arc::new(move |t: &str| r.redact(t).into_owned())
        });
    let logs = crate::logs::installed()
        .map(|l| l.files())
        .unwrap_or_default();
    let file = tauri::async_runtime::spawn_blocking(move || {
        let events = ledger
            .recent_events(crate::diagnostics::EVENTS)
            .unwrap_or_default();
        crate::diagnostics::write(
            &dir,
            &crate::diagnostics::Contents {
                about,
                events,
                logs,
            },
            &filter,
        )
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(|e| CommandError::internal(format!("the diagnostics file could not be saved: {e}")))?;
    log::info!("a diagnostics file was saved");
    Ok(file)
}

/// What `about.json` says: the facts that help, nothing of the owner's work.
async fn about<R: Runtime>(app: &AppHandle<R>) -> Value {
    let ledger = Arc::clone(&app.state::<Arc<Ledger>>());
    let (status, integrity, backups) = {
        let l = Arc::clone(&ledger);
        tauri::async_runtime::spawn_blocking(move || {
            (
                l.status().ok(),
                l.integrity_check().ok(),
                l.backups().unwrap_or_default(),
            )
        })
        .await
        .unwrap_or((None, None, Vec::new()))
    };
    // Each AI tool's newest version and update (Phase 19): versions and states only.
    let page = app
        .try_state::<plenipo_capabilities::ai_tools::AiTools>()
        .map(|t| t.page());
    let tools: Vec<Value> = app
        .try_state::<AgentRuntime>()
        .map(|a| a.runtimes())
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let kept = page
                .as_ref()
                .and_then(|p| p.tools.iter().find(|t| t.runtime_id == r.id));
            json!({
                "id": r.id,
                "name": r.label,
                "installed": r.installation.state,
                "version": r.installation.version,
                "checkedVersion": r.checked_version,
                "newest": kept.and_then(|t| t.newest.clone()),
                "update": kept.map(|t| t.update.state),
                "givenNoTasks": kept.is_some_and(|t| t.out_of_service.is_some()),
                "signIn": r.auth.state,
                "ready": r.ready,
            })
        })
        .collect();
    let supervisor = app.try_state::<Supervisor>();
    json!({
        "savedAt": chrono::Local::now().to_rfc3339(),
        "plenipo": crate::commands::app_info_for(&app.package_info().version.to_string()),
        "windows": {
            "name": sysinfo::System::long_os_version(),
            "kernel": sysinfo::System::kernel_version(),
        },
        "ledger": status,
        "integrity": integrity,
        "backups": backups.iter().map(|b| json!({
            "name": b.name, "kind": b.kind, "createdAt": b.created_at,
            "sizeBytes": b.size_bytes, "restorable": b.restorable,
        })).collect::<Vec<_>>(),
        "lastRun": recovery_status_off_thread(app, None).await.ok().map(|s| last_run(&s)),
        "startAndClose": start_close::settings(app, &ledger),
        "updates": app.state::<Arc<Updates>>().status(),
        "aiTools": tools,
        "aiToolsUpdateByThemselves": page.as_ref().map(|p| p.auto_update),
        "connections": app
            .try_state::<Guard>()
            .and_then(|g| g.connections().ok())
            .map(|c| crate::diagnostics::connections(&c)),
        "programsRunning": supervisor.as_ref().map(|s| s.active_count()),
        "notices": {
            "programs": supervisor.as_ref().map(|s| s.overview().notices),
            "permissions": app.try_state::<Guard>().map(|g| g.notices()),
        },
    })
}

/// How the last run ended, for the diagnostics file: the cause, times, and counts, never what
/// the stopped tasks were about (their objectives are the owner's words).
fn last_run(status: &RecoveryStatus) -> Value {
    json!({
        "recovery": status.recovery.as_ref().map(|r| json!({
            "cause": r.cause,
            "lastSeenAt": r.last_seen_at,
            "foundAt": r.found_at,
            "previousVersion": r.previous_version,
            "stoppedTasks": r.stopped_tasks.len(),
            "stoppedPrograms": r.stopped_programs,
        })),
        "window": status.window,
        "settingsProblems": status.settings_problems.iter().map(|p| &p.label).collect::<Vec<_>>(),
    })
}

// ---- Updates (ADR-038) ------------------------------------------------------------------------

/// Settings → Updates.
#[tauri::command]
pub fn get_update_status(updates: State<'_, Arc<Updates>>) -> Result<UpdateStatus, CommandError> {
    Ok(updates.status())
}

/// Check now whether a newer version exists (GitHub Releases, through Guard).
#[tauri::command]
pub async fn check_for_updates(
    updates: State<'_, Arc<Updates>>,
    guard: State<'_, Guard>,
    ledger: State<'_, Arc<Ledger>>,
) -> Result<UpdateStatus, CommandError> {
    Ok(updates.check_now(&guard, &ledger).await)
}

/// Install the newer version: only when the owner says so, and only one 8 West signed.
/// `stop_work`: the owner agreed that running work stops. On success Plenipo quits and the
/// installer opens the new version.
#[tauri::command]
pub async fn install_update<R: Runtime>(
    app: AppHandle<R>,
    stop_work: bool,
) -> Result<UpdateStatus, CommandError> {
    install(&app, stop_work).await
}

async fn install<R: Runtime>(
    app: &AppHandle<R>,
    stop_work: bool,
) -> Result<UpdateStatus, CommandError> {
    let updates = Arc::clone(&app.state::<Arc<Updates>>());
    let ledger = Arc::clone(&app.state::<Arc<Ledger>>());
    let guard = app.state::<Guard>().inner().clone();
    if updates.status().state == UpdateState::Installing {
        return Err(CommandError::invalid_input(
            "the update is already being installed",
        ));
    }
    if crate::work_going(app) && !stop_work {
        return Err(CommandError::invalid_input(
            "Work is running. Installing the update stops it; say so to go ahead.",
        ));
    }
    if !cfg!(windows) {
        let message = "Updates are installed on Windows only.".to_owned();
        updates.install_failed(&ledger, message.clone());
        return Err(CommandError::invalid_input(message));
    }
    let (release, bytes) = updates
        .download(&guard)
        .await
        .map_err(CommandError::invalid_input)?;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| CommandError::internal(e.to_string()))?
        .join(update_host::FOLDER);
    let version = app.package_info().version.to_string();
    let (l, v, r) = (Arc::clone(&ledger), version.clone(), release.clone());
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        update_host::prepare(&l, &dir, &v, &r, &bytes)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?;
    let path = match prepared {
        Ok(path) => path,
        Err(message) => {
            updates.install_failed(&ledger, message.clone());
            return Err(CommandError::invalid_input(message));
        }
    };
    log::warn!("installing Plenipo {} (from {version})", release.version);
    crate::stop_work(app).await;
    crate::mark_stopped(app);
    match update_host::start_installer(&path) {
        Ok(()) => {
            app.exit(0);
            Ok(updates.status())
        }
        Err(e) => {
            let message = format!(
                "The installer could not be started ({e}). Plenipo {version} is still installed; \
                 the work that was running was stopped. Plenipo restarts now."
            );
            updates.install_failed(&ledger, message.clone());
            // (Not in a launch test, which reports the failure and ends instead.)
            if !app.try_state::<SmokeTest>().is_some_and(|s| s.is_enabled()) {
                if let Ok(data) = app.path().app_local_data_dir() {
                    start_close::show_after_restart(&data);
                }
                app.request_restart();
            }
            Err(CommandError::internal(message))
        }
    }
}

/// Check once a day, by itself (copies that can install updates).
pub fn check_for_updates_daily(updates: Arc<Updates>, guard: Guard, ledger: Arc<Ledger>) {
    let _ = std::thread::Builder::new()
        .name("plenipo-update-check".into())
        .spawn(move || {
            std::thread::sleep(update_host::FIRST_CHECK);
            loop {
                tauri::async_runtime::block_on(updates.check_now(&guard, &ledger));
                std::thread::sleep(update_host::CHECK_EVERY);
            }
        });
}

// ---- Smoke tests (CI) -----------------------------------------------------------------------

/// The window watch's timing: quicker in the window-crash smoke test.
pub fn window_timing(smoke: &SmokeTest) -> window_watch::Timing {
    if smoke.is_enabled() && smoke.scenario() == Scenario::WindowCrash {
        window_watch::Timing {
            quiet: Duration::from_secs(8),
            every: Duration::from_secs(1),
        }
    } else {
        window_watch::Timing::default()
    }
}

/// What the app found, for the Windows installer tests to check.
fn smoke_report<R: Runtime>(app: &AppHandle<R>, extra: Value) -> Value {
    let ledger = app.state::<Arc<Ledger>>();
    let backups = ledger.backups().unwrap_or_default();
    let status = ledger.status().ok();
    let types: Vec<String> = ledger
        .recent_events(300)
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    json!({
        "version": app.package_info().version.to_string(),
        "persistent": status.as_ref().map(|s| s.persistent),
        "schemaVersion": status.as_ref().map(|s| s.schema_version),
        "ledgerNotices": status.as_ref().map(|s| s.notices.clone()),
        "lastVersion": crate::backup_host::last_version(&ledger),
        "recovery": recovery_status(app, None).ok(),
        "backups": backups.iter().map(|b| json!({ "name": b.name, "kind": b.kind })).collect::<Vec<_>>(),
        "updates": app.state::<Arc<Updates>>().status(),
        "logFiles": crate::logs::installed().map_or(0, |l| l.files().len()),
        "eventTypes": types,
        "extra": extra,
    })
}

fn write_report<R: Runtime>(app: &AppHandle<R>, smoke: &SmokeTest, extra: Value) {
    let Some(path) = smoke.report_path() else {
        return;
    };
    let report = smoke_report(app, extra);
    let tmp = path.with_extension("tmp");
    let written = serde_json::to_vec_pretty(&report)
        .map_err(std::io::Error::other)
        .and_then(|b| std::fs::write(&tmp, b))
        .and_then(|()| std::fs::rename(&tmp, path));
    if let Err(e) = written {
        log::error!("smoke test: the report could not be written: {e}");
    }
}

/// The window reported ready in a smoke run.
pub fn smoke_ready<R: Runtime>(app: &AppHandle<R>, smoke: &SmokeTest) {
    let count = smoke.ready_count();
    match smoke.scenario() {
        Scenario::Ready => {
            if smoke.record(EXIT_READY) {
                write_report(app, smoke, Value::Null);
                log::info!("smoke test: frontend reported ready; exiting 0");
                app.exit(EXIT_READY);
            }
        }
        Scenario::Stay if count == 1 => {
            smoke.record(EXIT_READY);
            write_report(app, smoke, json!({ "stage": "ready" }));
            log::info!("smoke test: ready; staying open until ended");
        }
        Scenario::StartWork if count == 1 => {
            smoke.record(EXIT_READY);
            let (app, smoke) = (app.clone(), smoke.clone());
            tauri::async_runtime::spawn(async move {
                let started = start_work(&app).await;
                write_report(&app, &smoke, started);
                log::info!("smoke test: work started; waiting to be ended");
            });
        }
        Scenario::WindowCrash if count == 1 => {
            write_report(app, smoke, json!({ "stage": "ready" }));
            log::info!("smoke test: waiting for the window to be ended and brought back");
        }
        Scenario::WindowCrash => {
            let (app, smoke) = (app.clone(), smoke.clone());
            std::thread::spawn(move || {
                for _ in 0..200 {
                    let back = app.state::<Arc<RecoveryState>>().window();
                    if let Some(back) = back {
                        if smoke.record(EXIT_READY) {
                            write_report(&app, &smoke, json!({ "stage": "back", "window": back }));
                            log::info!("smoke test: the window was brought back; exiting 0");
                            app.exit(EXIT_READY);
                        }
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
        // The outcome is what happens: 0 once the installer was started (the install quits with
        // 0), 2 when no update was found or it could not be installed (with the reason in the
        // report).
        Scenario::Update if count == 1 => {
            let (app, smoke) = (app.clone(), smoke.clone());
            tauri::async_runtime::spawn(async move {
                let updates = Arc::clone(&app.state::<Arc<Updates>>());
                let guard = app.state::<Guard>().inner().clone();
                let ledger = Arc::clone(&app.state::<Arc<Ledger>>());
                let status = updates.check_now(&guard, &ledger).await;
                if status.state != UpdateState::Available {
                    let why = status.message.unwrap_or_else(|| "no newer version".into());
                    write_report(&app, &smoke, json!({ "stage": "notFound", "error": why }));
                    smoke.record(2);
                    app.exit(2);
                    return;
                }
                write_report(&app, &smoke, json!({ "stage": "installing" }));
                if let Err(e) = install(&app, true).await {
                    write_report(
                        &app,
                        &smoke,
                        json!({ "stage": "failed", "error": e.message }),
                    );
                    smoke.record(2);
                    app.exit(2);
                }
            });
        }
        _ => {}
    }
}

/// Start a long program and a running task (the smoke test then ends Plenipo mid-work).
async fn start_work<R: Runtime>(app: &AppHandle<R>) -> Value {
    let execution = match app.try_state::<Supervisor>() {
        Some(s) => s.start("diagnostic.long-running").await.ok().map(|r| r.id),
        None => None,
    };
    let ledger = Arc::clone(&app.state::<Arc<Ledger>>());
    let task = tauri::async_runtime::spawn_blocking(move || {
        let task = ledger
            .create_task(
                NewTask {
                    requested_by: OWNER.into(),
                    objective: "Smoke test: a task left running".into(),
                    metadata: json!({ "smoke": true }),
                    ..NewTask::default()
                },
                OWNER,
            )
            .ok()?;
        ledger
            .transition_task(&task.id, TaskState::Running, OWNER, None)
            .ok()
            .map(|t| t.id)
    })
    .await
    .ok()
    .flatten();
    json!({ "execution": execution, "task": task })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_core::{Recovery, RecoveryCause, SettingsProblem, StoppedTask};

    #[test]
    fn the_diagnostics_file_says_how_the_last_run_ended_without_what_the_tasks_were() {
        let status = RecoveryStatus {
            recovery: Some(Recovery {
                id: "e1".into(),
                cause: RecoveryCause::Crash,
                last_seen_at: Some(1),
                found_at: 2,
                previous_version: Some("1.9.0".into()),
                stopped_tasks: vec![StoppedTask {
                    task_id: "t1".into(),
                    objective: "Email the confidential price list to Dana".into(),
                    who: Some("Sales Manager".into()),
                    can_run_again: true,
                    run_again_as: None,
                }],
                stopped_programs: 1,
            }),
            window: None,
            settings_problems: vec![SettingsProblem {
                key: "guard".into(),
                label: "Permissions".into(),
                message: "expected a list at line 3".into(),
            }],
        };
        let text = last_run(&status).to_string();
        assert!(text.contains("\"cause\":\"crash\""), "{text}");
        assert!(text.contains("\"stoppedTasks\":1"), "{text}");
        assert!(text.contains("Permissions"), "{text}");
        for private in ["confidential", "Dana", "Sales Manager", "t1"] {
            assert!(!text.contains(private), "{private} in {text}");
        }
    }
}
