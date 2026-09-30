//! Plenipo Desktop backend.
//!
//! The React frontend reaches Rust only through the typed commands in
//! [`commands`]. Each command must also be granted in
//! `capabilities/default.json`; nothing else is exposed.

pub mod agent_host;
pub mod ai_tools_commands;
pub mod ai_tools_host;
pub mod backup_host;
pub mod canvas_commands;
pub mod commands;
pub mod connections_commands;
pub mod diagnostics;
pub mod files_commands;
pub mod guard_host;
pub mod indicator;
pub mod ledger_host;
pub mod logs;
pub mod notices;
pub mod org_commands;
pub mod org_host;
pub mod orgs;
pub mod owner_commands;
pub mod recovery;
pub mod runtime_host;
pub mod settings_health;
pub mod smoke;
pub mod start_close;
pub mod tool_holds;
pub mod tray;
pub mod uninstall;
pub mod update_host;
pub mod upkeep_commands;
pub mod window_watch;
pub mod workspace_commands;
pub mod workspace_windows;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use plenipo_liaison::Liaison;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use tauri::webview::PageLoadEvent;
use tauri::{Builder, Manager as _, RunEvent, Runtime, WindowEvent};

use runtime_host::Persistence;
use smoke::{SmokeMode, SmokeTest};

/// How long quitting waits for owned processes to be terminated and recorded.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Environment-dependent pieces, so tests can run without a tray, on-disk state, or the
/// system's notices.
#[derive(Debug, Clone, Copy)]
pub struct ShellOptions {
    pub tray: bool,
    pub persistence: Persistence,
    /// Where pop-up notices go (the system's, or kept for tests).
    pub notices: notices::Output,
    /// Start with Windows can be turned on (Phase 13; never in tests, which must not change
    /// the computer's sign-in list).
    pub autostart: bool,
    /// Watch the window's page and bring it back if it stops (Phase 13).
    pub window_watch: bool,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            tray: true,
            persistence: Persistence::AppData,
            notices: notices::Output::System,
            autostart: true,
            window_watch: true,
        }
    }
}

/// This run's "Plenipo is running" note (Phase 13): none for a temporary Ledger.
pub struct RunNote(pub Option<Arc<recovery::RunNoteKeeper>>);

impl RunNote {
    /// A clean exit: the next start will not report a crash.
    pub fn finish(&self) {
        if let Some(keeper) = &self.0 {
            keeper.finish();
        }
    }
}

/// Settings Plenipo could not read at start (Phase 13).
pub struct SettingsProblems(pub std::sync::Mutex<Vec<plenipo_core::SettingsProblem>>);

/// Plenipo was started by Windows at sign-in (stay in the tray).
pub struct StartedInTray(pub bool);

/// Work is going: a program is running, a terminal is open, or a task is not finished.
pub fn work_going<R: Runtime>(app: &tauri::AppHandle<R>) -> bool {
    // Every organization's (Phase 21, ADR-094 §7).
    let busy = orgs::all_stacks(app).iter().any(|s| s.busy());
    // An AI tool being updated, or waiting to be (Phase 19, ADR-059 §3).
    let updates = app
        .try_state::<plenipo_capabilities::ai_tools::AiTools>()
        .map_or(0, |t| t.updates_going());
    busy || updates > 0
}

/// Where quitting is: running, stopping owned processes, or done and free to exit.
#[derive(Default)]
struct ShutdownState(AtomicU8);

const RUNNING: u8 = 0;
const STOPPING: u8 = 1;
const STOPPED: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
enum ExitStep {
    /// The first request: hold the exit and stop owned processes.
    Start,
    /// A request while that runs, such as the window being destroyed: hold it too, or the
    /// app would exit before the processes' final state is recorded.
    Hold,
    /// The shutdown has finished; this is its own exit.
    Allow,
}

impl ShutdownState {
    fn exit_requested(&self) -> ExitStep {
        match self
            .0
            .compare_exchange(RUNNING, STOPPING, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => ExitStep::Start,
            Err(STOPPING) => ExitStep::Hold,
            Err(_) => ExitStep::Allow,
        }
    }

    fn finished(&self) {
        self.0.store(STOPPED, Ordering::SeqCst);
    }
}

/// Register Plenipo state, setup hooks, window behavior, and the command surface.
/// Shared by the real app and the IPC boundary tests.
pub fn configure<R: Runtime>(
    builder: Builder<R>,
    smoke: SmokeTest,
    options: ShellOptions,
) -> Builder<R> {
    // The notification plugin shows Plenipo's notices. Its own commands are granted to no
    // window (capabilities/default.json), so only Plenipo decides what a notice says.
    let builder = builder.plugin(tauri_plugin_notification::init());
    // Start with Windows (Phase 13): its own commands are granted to no window either;
    // Settings uses Plenipo's own command.
    let builder = if options.autostart {
        builder.plugin(start_close::autostart_plugin())
    } else {
        builder
    };
    builder
        .manage(smoke)
        .manage(ShutdownState::default())
        .setup(move |app| {
            let args: Vec<String> = std::env::args().collect();
            // Opened only to ask a running Plenipo to quit, and none was running: nothing to do.
            if start_close::asked_to_quit(&args) {
                app.handle().exit(0);
                return Ok(());
            }
            let smoke = app.state::<SmokeTest>();
            if let SmokeMode::Enabled { timeout } = smoke.mode() {
                smoke.arm_watchdog(app.handle().clone(), timeout);
            }
            let version = app.package_info().version.to_string();
            let data = match options.persistence {
                Persistence::AppData => app.path().app_local_data_dir().ok(),
                Persistence::InMemory => None,
            };
            // The data folder holds the Ledger, screenshots, logs, and diagnostics: on Unix it
            // is readable by the owner's account only (on Windows it is already, through its
            // access control list). Made so before anything is written in it.
            let private = data.as_ref().map(|data| {
                std::fs::create_dir_all(data)
                    .and_then(|()| plenipo_ledger::owner_only::folder(data))
            });
            // Phase 13: the log files first, so everything after is in them.
            if let Some(data) = &data {
                logs::install(&data.join("logs"));
            }
            log::info!(
                "Plenipo {version} is starting ({} {})",
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            if let Some(Err(e)) = private {
                log::warn!("The data folder could not be made readable by this account only: {e}");
            }
            // How the last run ended (read before this run's note replaces it).
            let (previous, keeper) = match &data {
                Some(data) => {
                    let note = data.join("run").join(recovery::RUN_NOTE);
                    let previous = recovery::previous_end(&note, recovery::boot_ms());
                    (
                        previous,
                        Some(recovery::RunNoteKeeper::start(note, &version)),
                    )
                }
                None => (recovery::PreviousEnd::Clean, None),
            };
            // More than one organization (Phase 21, ADR-094): the list, then the first one, as
            // Plenipo always opened it; then every other one that is not archived.
            let orgs = Arc::new(orgs::Orgs::load(data.as_deref()));
            app.manage(orgs.clone());
            let identifier = app.config().identifier.clone();
            let place = |id: &str| orgs::OrgPlace {
                id: id.to_owned(),
                folder: data.as_ref().map(|d| orgs::folder_of(d, id)),
                data: data.clone(),
                vault: orgs::vault_name(&identifier, id),
            };
            // A restore chosen in Diagnostics happens first, and a layout change backs the
            // Ledger up itself (`pre-migration-v<n>`).
            let opened = org_host::open_ledger(
                app.handle(),
                &place(orgs::FIRST),
                options.persistence,
                &version,
                |changes_layout| {
                    if let (Some(keeper), true) = (&keeper, changes_layout) {
                        keeper.set_phase(recovery::Phase::ChangingLayout);
                    }
                },
            );
            if let Some(keeper) = &keeper {
                keeper.set_phase(recovery::Phase::Running);
                keeper.keep_beating();
            }
            let ledger = opened.ledger.clone();
            app.manage(ledger.clone());
            app.manage(options.persistence);
            // Where the pop-out panels were (Phase 21, ADR-092).
            app.manage(workspace_windows::PopOuts::new(
                data.as_ref()
                    .map(|d| d.join(workspace_windows::PLACES_FILE)),
            ));
            app.manage(org_host::Defaults {
                persistence: options.persistence,
                notices: options.notices,
                gather: notices::GATHER,
                run: true,
            });
            let opening = org_host::Opening {
                persistence: options.persistence,
                notices: options.notices,
                gather: notices::GATHER,
                version: &version,
                previous: &previous,
                run: true,
                control: orgs.control(),
                first: None,
            };
            let first = org_host::build(app.handle(), place(orgs::FIRST), ledger.clone(), &opening);
            orgs.insert(first.clone());
            orgs.set_name(orgs::FIRST, &orgs::name_in(&first.ledger));
            for entry in orgs.entries() {
                if entry.id == orgs::FIRST || entry.archived_at.is_some() {
                    continue;
                }
                let place = place(&entry.id);
                let opened = org_host::open_ledger(
                    app.handle(),
                    &place,
                    options.persistence,
                    &version,
                    |_| {},
                );
                let stack = org_host::build(
                    app.handle(),
                    place,
                    opened.ledger,
                    &org_host::Opening {
                        first: Some(&first),
                        ..opening.clone()
                    },
                );
                orgs.set_name(&entry.id, &orgs::name_in(&stack.ledger));
                orgs.insert(stack);
            }
            let (supervisor, agents, liaison, router, guard, broker, workforce) = (
                first.supervisor.clone(),
                first.agents.clone(),
                first.liaison.clone(),
                first.router.clone(),
                first.guard.clone(),
                first.broker.clone(),
                first.workforce.clone(),
            );
            app.manage(first.notices.clone());
            app.manage(first.watchers.clone());
            // Files dropped on a window from File Explorer, by ticket (Phase 21).
            app.manage(files_commands::Drops::default());
            // Opening the owner's files in another program, or in File Explorer (Phase 21).
            app.manage(files_commands::Outside(Arc::new(
                files_commands::SystemFileOpener,
            )));
            // The AI tools page (Phase 19): sign-in tabs, updates, usage, and models; the PC's,
            // kept with the first organization (ADR-094 §4).
            let ai_tools = ai_tools_host::create(&agents, &broker);
            ai_tools_host::listen(&ledger, &ai_tools);
            if options.persistence == Persistence::AppData {
                ai_tools_host::start_daily(ai_tools.clone());
            }
            // An AI tool's update holds every organization's new work for it (ADR-094 §4).
            app.manage(Arc::new(tool_holds::UpdateHolds::default()));
            tool_holds::watch(app.handle());
            let problems = settings_health::problems(&guard, &router);
            // The logs hide every organization's secrets.
            orgs::filter_logs(&orgs);
            let updates = update_host::Updates::new(&version, update_host::built_source());
            if let Some(data) = &data {
                update_host::clean_up(&data.join(update_host::FOLDER));
                if updates.checks_by_itself() {
                    upkeep_commands::check_for_updates_daily(
                        updates.clone(),
                        guard.clone(),
                        ledger.clone(),
                    );
                }
            }
            // Started by Windows at sign-in: stay in the tray (ADR-037), unless this start is a
            // restart Plenipo asked for itself (a restore), which shows the window.
            let in_tray = start_close::started_in_tray(&args)
                && !data
                    .as_deref()
                    .is_some_and(start_close::shows_after_restart);
            manage_upkeep(app.handle(), RunNote(keeper), problems, updates, in_tray);
            if options.window_watch {
                window_watch::start(
                    app.handle(),
                    app.state::<Arc<window_watch::WindowWatch>>()
                        .inner()
                        .clone(),
                    upkeep_commands::window_timing(&app.state::<SmokeTest>()),
                );
            }
            app.manage(guard);
            app.manage(broker);
            app.manage(ai_tools);
            app.manage(supervisor);
            app.manage(agents);
            app.manage(liaison);
            app.manage(router);
            app.manage(workforce);
            quit_on_termination_signal(app.handle().clone());
            if options.tray {
                // A missing tray (e.g. no status-notifier host on Linux) is not fatal.
                if let Err(e) = tray::create(app) {
                    log::warn!("system tray unavailable: {e}");
                }
            }
            // The first organization's window (Phase 21: built here, so that it may open its
            // pop-out panels). It starts hidden in the tray; otherwise it shows at once.
            match workspace_windows::build_org_window(
                app.handle(),
                workspace_windows::MAIN,
                !in_tray,
            ) {
                // It names the organization it shows (Phase 21).
                Ok(main) => org_commands::set_title(&main, &orgs, &orgs.main_shows()),
                Err(e) => log::error!("the window could not open: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // A pop-out panel (Phase 21, ADR-092): Plenipo keeps where it is, and when it
            // closes, its organization's window puts the panel back in a dock.
            if workspace_windows::parse_popout(window.label()).is_some() {
                match event {
                    WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                        workspace_windows::remember_place(window);
                    }
                    // Only the owner closing it puts the panel back: a pop-out Plenipo closes
                    // itself (Quit, a reload, Reset layout) stays in the kept layout.
                    WindowEvent::CloseRequested { .. } => {
                        workspace_windows::closed(window.app_handle(), window.label());
                    }
                    _ => {}
                }
                return;
            }
            // Files dropped from File Explorer (Phase 21, ADR-093 §20): Plenipo keeps where they
            // are and gives the page a ticket.
            if let WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, position }) = event {
                files_commands::dropped(window, paths.clone(), *position);
                return;
            }
            // Closing the window never stops approved work unless the owner chose that
            // (ADR-037): hide to the tray while work is going (or always), or quit the normal
            // way, which stops the work and records it.
            // Another organization's own window closed (Phase 21, ADR-094 §14): its work goes on;
            // the terminals it showed end.
            if window.label().starts_with(workspace_windows::ORG_PREFIX) {
                if let WindowEvent::Destroyed = event {
                    let app = window.app_handle();
                    if let Some(orgs) = app.try_state::<Arc<orgs::Orgs>>() {
                        org_commands::leave(app, &orgs, window.label(), "its window closed");
                        orgs.unbind(window.label());
                        orgs::list_changed(app);
                    }
                }
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let app = window.app_handle();
                let choice = app
                    .try_state::<Arc<plenipo_ledger::Ledger>>()
                    .map(|l| start_close::close_window(&l))
                    .unwrap_or_default();
                api.prevent_close();
                match start_close::close_action(choice, work_going(app), tray::exists(app)) {
                    start_close::CloseAction::Hide => {
                        let _ = window.hide();
                        workspace_windows::hide_popouts(app, window.label());
                    }
                    start_close::CloseAction::Quit => app.exit(0),
                }
            }
        })
        .on_page_load(|webview, payload| {
            // The terminals belong to the page that shows them: when the main window's page
            // loads again (a reload), the terminals it showed end instead of running unseen.
            // Its pop-outs close too; the page opens them again (ADR-092 §10). Every
            // organization's window alike (Phase 21).
            if workspace_windows::is_org_window(webview.label())
                && payload.event() == PageLoadEvent::Started
            {
                let app = webview.app_handle();
                match app.try_state::<Arc<orgs::Orgs>>() {
                    Some(orgs) => {
                        org_commands::leave(app, &orgs, webview.label(), "the window was reloaded");
                    }
                    None => workspace_windows::close_popouts(app, webview.label()),
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
            commands::get_activity,
            commands::get_scope_events,
            commands::get_task_events,
            commands::get_project_record,
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
            commands::get_home,
            commands::get_task_record,
            commands::get_notice_settings,
            commands::set_notice_settings,
            commands::send_test_notice,
            commands::get_local_paths,
            commands::rename_organization,
            commands::set_organization_titles,
            commands::create_role,
            commands::create_department,
            commands::update_department,
            commands::create_project,
            commands::update_project,
            commands::archive_project,
            owner_commands::set_model_rule,
            owner_commands::set_role_learns,
            owner_commands::set_agent_learning,
            owner_commands::create_specialty,
            owner_commands::update_specialty,
            owner_commands::remove_specialty,
            owner_commands::archive_department,
            owner_commands::bring_back_position,
            owner_commands::bring_back_project,
            owner_commands::bring_back_department,
            owner_commands::preview_delete_for_good,
            owner_commands::delete_for_good,
            owner_commands::save_to_workforce,
            owner_commands::hire_from_workforce,
            owner_commands::delete_saved_agent,
            canvas_commands::place_tiles,
            canvas_commands::tidy_up,
            canvas_commands::retarget_oversight,
            canvas_commands::get_live_view,
            canvas_commands::lend_agent,
            canvas_commands::send_home,
            canvas_commands::get_watch,
            canvas_commands::get_watch_change,
            canvas_commands::subscribe_watch,
            canvas_commands::unsubscribe_watch,
            canvas_commands::get_owner_profile,
            canvas_commands::set_owner_profile,
            ai_tools_commands::get_ai_tools,
            ai_tools_commands::check_ai_tool,
            ai_tools_commands::check_ai_tool_versions,
            ai_tools_commands::get_ai_tool_usage,
            ai_tools_commands::update_ai_tool,
            ai_tools_commands::cancel_ai_tool_update,
            ai_tools_commands::set_ai_tools_auto_update,
            ai_tools_commands::set_ai_tool_payment,
            connections_commands::get_connections,
            connections_commands::connect_connection,
            connections_commands::cancel_connection_sign_in,
            connections_commands::disconnect_connection,
            connections_commands::set_connection_parts,
            connections_commands::set_connection_access,
            connections_commands::set_connection_send_list,
            connections_commands::set_connection_own_app,
            connections_commands::save_connection_app,
            connections_commands::add_connection,
            connections_commands::remove_connection,
            connections_commands::save_connection_key,
            connections_commands::add_add_on,
            connections_commands::change_add_on,
            connections_commands::remove_add_on,
            connections_commands::check_add_on_tools,
            connections_commands::set_add_on_tools,
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
            commands::get_permissions,
            commands::save_permission_set,
            commands::remove_permission_set,
            commands::assign_permissions,
            commands::set_command_rules,
            commands::set_blocked_files,
            commands::set_sensitive_rule,
            commands::set_guard_options,
            commands::save_secret,
            commands::remove_secret,
            commands::get_approvals,
            commands::resolve_approval,
            commands::revoke_grant,
            commands::set_up_development,
            commands::get_objective_report,
            commands::get_project_work,
            commands::remove_workspace,
            commands::update_role,
            commands::get_control_status,
            commands::stop_all_control,
            commands::take_over_control,
            commands::allow_control,
            commands::set_website_rules,
            commands::set_switches,
            commands::get_learning,
            commands::set_learning,
            commands::set_role_learning,
            commands::decide_lesson,
            commands::remove_lesson,
            commands::get_browser_status,
            commands::set_browser_choice,
            commands::open_browser,
            commands::get_screenshot,
            commands::get_servers,
            commands::save_server,
            commands::remove_server,
            commands::check_server_identity,
            commands::test_server,
            commands::stop_server_command,
            commands::get_terminal_settings,
            commands::set_terminal_shell,
            commands::open_terminal,
            commands::write_terminal,
            commands::resize_terminal,
            commands::close_terminal,
            upkeep_commands::get_recovery_status,
            upkeep_commands::run_again,
            upkeep_commands::dismiss_recovery,
            upkeep_commands::dismiss_window_recovery,
            upkeep_commands::window_alive,
            upkeep_commands::reset_settings,
            upkeep_commands::get_start_and_close,
            upkeep_commands::set_start_and_close,
            upkeep_commands::list_ledger_backups,
            upkeep_commands::restore_ledger_backup,
            upkeep_commands::cancel_ledger_restore,
            upkeep_commands::save_diagnostics_file,
            upkeep_commands::get_update_status,
            upkeep_commands::check_for_updates,
            upkeep_commands::install_update,
            workspace_commands::prepare_pop_out,
            workspace_commands::focus_pop_out,
            workspace_commands::reset_pop_outs,
            files_commands::get_file_roots,
            files_commands::list_folder,
            files_commands::read_file,
            files_commands::save_file,
            files_commands::open_file_outside,
            files_commands::show_in_folder,
            files_commands::get_changing_files,
            workspace_commands::close_pop_out,
            org_commands::get_organizations,
            org_commands::create_organization,
            org_commands::switch_organization,
            org_commands::open_organization_window,
            org_commands::archive_organization,
            org_commands::bring_back_organization,
            org_commands::preview_delete_organization,
            org_commands::delete_organization_for_good,
        ])
}

/// Put Phase 13's state in place (the app's setup, and the IPC tests).
pub fn manage_upkeep<R: Runtime>(
    app: &tauri::AppHandle<R>,
    note: RunNote,
    problems: Vec<plenipo_core::SettingsProblem>,
    updates: Arc<update_host::Updates>,
    in_tray: bool,
) {
    app.manage(note);
    app.manage(SettingsProblems(std::sync::Mutex::new(problems)));
    app.manage(updates);
    app.manage(StartedInTray(in_tray));
    app.manage(Arc::new(recovery::RecoveryState::default()));
    app.manage(Arc::new(window_watch::WindowWatch::default()));
}

/// Stop the work the normal way, recording how it ended: Liaison stops handing out work, the
/// owner's terminals end, then running AI tool turns and programs are stopped. Used by Quit,
/// by installing an update, and by restarting to restore a backup. Returns how many programs
/// were stopped.
pub async fn stop_work<R: Runtime>(app: &tauri::AppHandle<R>) -> usize {
    // Every organization's work, all at once (Phase 21, ADR-094 §7); each Ledger records its
    // own.
    let stacks = orgs::all_stacks(app);
    if !stacks.is_empty() {
        let stopping: Vec<_> = stacks
            .into_iter()
            .map(|s| {
                tauri::async_runtime::spawn(async move {
                    org_host::stop(&s, "Plenipo closed", SHUTDOWN_GRACE).await
                })
            })
            .collect();
        let mut stopped = 0;
        for s in stopping {
            stopped += s.await.unwrap_or(0);
        }
        if stopped > 0 {
            log::info!("terminated {stopped} running process(es) on exit");
        }
        return stopped;
    }
    // Liaison stops handing out work first. The agent runtime then stops its turns through
    // the supervisor and records their results (waiting turns stay as recorded; the next start
    // marks them interrupted); the supervisor then has nothing left to stop.
    if let Some(liaison) = app.try_state::<Liaison>() {
        liaison.shutdown();
    }
    // The owner's terminals end with Plenipo (and their closing is recorded).
    if let Some(broker) = app.try_state::<plenipo_capabilities::Broker>() {
        broker.close_all_terminals("Plenipo closed");
        // A server terminal waits up to 2 s for the server to answer its close.
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        while !broker.open_terminals().is_empty() && std::time::Instant::now() < deadline {
            tokio_sleep(Duration::from_millis(25)).await;
        }
    }
    let mut stopped = 0;
    if let Some(agents) = app.try_state::<AgentRuntime>() {
        stopped += agents.shutdown(SHUTDOWN_GRACE).await;
    }
    if let Some(supervisor) = app.try_state::<Supervisor>() {
        stopped += supervisor.shutdown(SHUTDOWN_GRACE).await;
    }
    if stopped > 0 {
        log::info!("terminated {stopped} running process(es) on exit");
    }
    stopped
}

/// The work is stopped and recorded: the exit that follows is Plenipo's own, not a crash.
pub fn mark_stopped<R: Runtime>(app: &tauri::AppHandle<R>) {
    app.state::<ShutdownState>().finished();
    if let Some(note) = app.try_state::<RunNote>() {
        note.finish();
    }
    if let Some(logs) = logs::installed() {
        logs.flush();
    }
}

/// Handle app-level events. On the first exit request, terminate owned processes and record
/// their final state before letting the app exit. Exit requests that arrive meanwhile (the
/// window being destroyed, a second Quit) are held until that is done.
pub fn on_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: RunEvent) {
    match event {
        // The last window closed because Plenipo is opening it again (a crashed window,
        // ADR-037): that is not a quit.
        RunEvent::ExitRequested {
            api, code: None, ..
        } if app
            .try_state::<Arc<window_watch::WindowWatch>>()
            .is_some_and(|w| w.is_reopening(plenipo_ledger::now_ms())) =>
        {
            api.prevent_exit();
        }
        RunEvent::ExitRequested { api, code, .. } => {
            let step = app.state::<ShutdownState>().exit_requested();
            if step != ExitStep::Allow {
                api.prevent_exit();
            }
            if step == ExitStep::Start {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    stop_work(&app).await;
                    log::info!("Plenipo quit");
                    mark_stopped(&app);
                    app.exit(code.unwrap_or(0));
                });
            }
        }
        RunEvent::Exit => {
            // Not after Plenipo's own shutdown (which removed the note): Windows is ending the
            // session (a restart, a shutdown, or signing out) and closes Plenipo, as it closes
            // every program, before the work can be stopped. Say so in the note, for the next
            // start (ADR-037 item 7).
            if let Some(note) = app.try_state::<RunNote>() {
                if let Some(keeper) = note.0.as_ref().filter(|k| k.is_running()) {
                    log::warn!("Windows is ending the session; Plenipo is being closed");
                    keeper.set_phase(recovery::Phase::EndedByWindows);
                }
            }
            if let Some(logs) = logs::installed() {
                logs.flush();
            }
        }
        _ => {}
    }
}

async fn tokio_sleep(d: Duration) {
    tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d))
        .await
        .ok();
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
        log::info!("termination signal received; shutting down");
        app.exit(0);
    });
    #[cfg(not(unix))]
    let _ = app;
}

/// Build and run the Tauri application, returning the process exit code.
pub fn run() -> i32 {
    let smoke = SmokeTest::from_env();
    let outcome = smoke.clone();

    // One Plenipo at a time (Phase 13, ADR-037): opening it again shows the one running, and
    // `--quit` asks it to quit the normal way. First, so a second launch stops before anything
    // else starts. Windows only: the target, and Linux test runs start one after another.
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        start_close::on_second_launch(app, &args);
    }));
    let runtime_code = configure(builder, smoke, ShellOptions::default())
        .build(tauri::generate_context!())
        .expect("error while building Plenipo")
        .run_return(on_run_event);

    // The runtime does not reliably propagate the code given to `AppHandle::exit`
    // on every platform, so the smoke outcome is tracked independently.
    outcome.resolve_exit_code(runtime_code)
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    #[test]
    fn exit_requests_wait_for_the_shutdown_to_finish() {
        let state = ShutdownState::default();
        assert_eq!(state.exit_requested(), ExitStep::Start);
        // The window destroyed mid-shutdown, then a second Quit: both wait.
        assert_eq!(state.exit_requested(), ExitStep::Hold);
        assert_eq!(state.exit_requested(), ExitStep::Hold);
        state.finished();
        assert_eq!(state.exit_requested(), ExitStep::Allow);
    }
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
        app_with_autostart(false)
    }

    /// `autostart`: with the Start with Windows plugin in place (its own commands must still be
    /// refused to every window; only Plenipo's command uses it).
    fn app_with_autostart(autostart: bool) -> App<MockRuntime> {
        let app = configure(
            mock_builder(),
            SmokeTest::new(SmokeMode::Disabled),
            ShellOptions {
                tray: false,
                persistence: Persistence::InMemory,
                notices: notices::Output::Kept,
                autostart,
                window_watch: false,
            },
        )
        .build(tauri::generate_context!())
        .expect("failed to build mock app");
        // The mock runtime does not run `setup`; install the organizations the same way: the
        // first one, kept in memory, its services built as the app builds them.
        let orgs = Arc::new(orgs::Orgs::load(None));
        app.manage(orgs.clone());
        let defaults = org_host::Defaults {
            persistence: Persistence::InMemory,
            notices: notices::Output::Kept,
            gather: Duration::from_millis(100),
            // Queries only: no tool server, reconciliation loop, or daily backup without
            // running workers.
            run: false,
        };
        app.manage(defaults);
        let place = orgs::OrgPlace {
            id: orgs::FIRST.into(),
            folder: None,
            data: None,
            vault: orgs::vault_name("test", orgs::FIRST),
        };
        let ledger = ledger_host::open(app.handle(), &place, Persistence::InMemory);
        app.manage(ledger.clone());
        app.manage(Persistence::InMemory);
        let first = org_host::build(
            app.handle(),
            place,
            ledger,
            &org_host::Opening {
                persistence: Persistence::InMemory,
                notices: notices::Output::Kept,
                gather: Duration::from_millis(100),
                version: "1.9.0",
                previous: &recovery::PreviousEnd::Clean,
                run: false,
                control: orgs.control(),
                first: None,
            },
        );
        orgs.insert(first.clone());
        // A copy built with an app ID would otherwise open the developer's real browser when a
        // test presses Connect: these tests open nothing.
        first.broker.set_connection_opener(Arc::new(NoBrowser));
        app.manage(first.notices.clone());
        app.manage(first.watchers.clone());
        app.manage(ai_tools_host::create(&first.agents, &first.broker));
        manage_upkeep(
            app.handle(),
            RunNote(None),
            Vec::new(),
            update_host::Updates::new("1.9.0", update_host::built_source()),
            false,
        );
        // Phase 21: pop-out places (kept in memory), drop tickets, and an opener that opens
        // nothing.
        app.manage(workspace_windows::PopOuts::new(None));
        app.manage(files_commands::Drops::default());
        app.manage(files_commands::Outside(Arc::new(NoOpener)));
        app.manage(first.guard.clone());
        app.manage(first.broker.clone());
        app.manage(first.supervisor.clone());
        app.manage(first.agents.clone());
        app.manage(first.liaison.clone());
        app.manage(first.router.clone());
        app.manage(first.workforce.clone());
        app
    }

    /// Opens no file (tests never start a program).
    struct NoOpener;

    impl files_commands::FileOpener for NoOpener {
        fn open(
            &self,
            _: plenipo_runtime::Supervisor,
            _: std::path::PathBuf,
            _: bool,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
        {
            Box::pin(async { Ok(()) })
        }
    }

    /// A browser that opens nothing.
    struct NoBrowser;

    impl plenipo_capabilities::connections::Opener for NoBrowser {
        fn open(
            &self,
            _: String,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
        {
            Box::pin(async { Ok(()) })
        }
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
        // Activity strips count the same events (Phase 12A).
        let now = plenipo_ledger::now_ms();
        let series: Vec<plenipo_ledger::ActivitySeries> = body(invoke_json(
            &main,
            "get_activity",
            serde_json::json!({
                "scopes": [{ "kind": "all" }],
                "from": now - 86_400_000,
                "to": now + 1,
                "buckets": 96,
            }),
        ));
        assert_eq!(series[0].buckets.len(), 96);
        let counted: u32 = series[0].buckets.iter().map(|b| b.events).sum();
        assert_eq!(counted as usize, events.len());
        for bad in [
            serde_json::json!({ "scopes": [{ "kind": "project", "id": "" }], "from": 0, "to": 1, "buckets": 1 }),
            serde_json::json!({ "scopes": [{ "kind": "all" }], "from": 5, "to": 5, "buckets": 96 }),
            serde_json::json!({ "scopes": [{ "kind": "all" }], "from": 0, "to": 1, "buckets": 5000 }),
        ] {
            let err = invoke_json(&main, "get_activity", bad).unwrap_err();
            assert_eq!(err["kind"], "invalidInput", "{err}");
        }
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
    fn activity_is_denied_to_other_windows_the_sign_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        // Valid arguments, so a refusal comes from the permission list, not from bad input.
        let now = plenipo_ledger::now_ms();
        let args = serde_json::json!({
            "scopes": [{ "kind": "all" }],
            "from": now - 86_400_000,
            "to": now,
            "buckets": 96,
        });
        assert!(invoke_json(&main, "get_activity", args.clone()).is_ok());
        assert!(invoke_json(&other, "get_activity", args.clone()).is_err());
        assert!(invoke_json(&sign, "get_activity", args.clone()).is_err());
        assert!(invoke_with(&main, "get_activity", args, "https://example.com").is_err());
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

    /// The AI tools this version ships, in display order.
    fn tool_ids() -> Vec<&'static str> {
        plenipo_runtime::agent::builtin_adapters()
            .iter()
            .map(|a| a.id())
            .collect()
    }

    #[test]
    fn agent_overview_lists_runtimes_without_starting_anything() {
        let app = app();
        let main = window(&app, "main");
        let overview: plenipo_runtime::agent::AgentOverview =
            body(invoke(&main, "get_agent_overview"));
        let ids: Vec<_> = overview.runtimes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, tool_ids());
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
            serde_json::json!({ "runtimeId": "no-such-tool", "objective": "x" }),
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
            (3, 3, 8, 16)
        );
        let addresses: Vec<_> = overview
            .destinations
            .iter()
            .map(|d| d.address.as_str())
            .collect();
        assert_eq!(addresses, tool_ids());
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
        assert_eq!(runtimes, tool_ids());
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
                "capabilityProfile": "developer",
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
        let defaults: Vec<String> = plenipo_runtime::agent::builtin_adapters()
            .iter()
            .map(|a| format!("{} (default model)", a.label()))
            .collect();
        assert_eq!(labels, defaults);
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
        // An AI tool whose company the role's choices never use cannot be fixed (ADR-041).
        let err = invoke_json(
            &main,
            "hire_position",
            serde_json::json!({ "input": {
                "roleId": dev, "title": "Fixed Developer", "reportsTo": head.id,
                "runtimeId": "codex",
            }}),
        )
        .expect_err("never OpenAI");
        assert!(
            err["message"]
                .as_str()
                .is_some_and(|m| m.contains("Senior Developer never uses OpenAI")),
            "{err}"
        );
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "hire_position",
            serde_json::json!({ "input": {
                "roleId": dev, "title": "Fixed Developer", "reportsTo": head.id,
                "runtimeId": "claude-code",
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
        assert_eq!(s.models.len(), tool_ids().len());
        assert_eq!(
            s.options.on_usage_limit,
            plenipo_router::LimitBehavior::Wait
        );
    }

    // ---- Permissions, approvals, and the Vault (Phase 7) -----------------------------------

    fn perms(
        r: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
    ) -> plenipo_capabilities::PermissionsSnapshot {
        body(r)
    }

    #[test]
    fn permissions_are_configured_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let s = perms(invoke(&main, "get_permissions"));
        assert_eq!(s.settings.capabilities.len(), 18);
        let ids: Vec<&str> = s.settings.sets.iter().map(|x| x.id.as_str()).collect();
        for id in [
            "read-only",
            "developer",
            "reviewer",
            "tester",
            "writer",
            "no-access",
        ] {
            assert!(ids.contains(&id), "{id}");
        }
        let dev = s
            .settings
            .roles
            .iter()
            .find(|r| r.role_name == "Senior Developer")
            .unwrap();
        assert_eq!(
            dev.set_id.as_deref(),
            Some("developer"),
            "seeded from the template"
        );
        assert!(s.vault.available, "tests use a memory store");
        assert!(
            !s.tools.running,
            "the mock app does not start the tool server"
        );
        let dev_role = dev.role_id.clone();
        let s = perms(invoke_json(
            &main,
            "save_permission_set",
            serde_json::json!({ "input": {
                "name": "Docs only", "description": "Reads and writes files.",
                "levels": { "filesystem.read": "allowed", "filesystem.write": "ask" },
            }}),
        ));
        let docs = s
            .settings
            .sets
            .iter()
            .find(|x| x.name == "Docs only")
            .unwrap()
            .clone();
        assert_eq!(docs.id, "docs-only");
        let s = perms(invoke_json(
            &main,
            "assign_permissions",
            serde_json::json!({ "target": "role", "id": dev_role, "setId": docs.id }),
        ));
        assert_eq!(
            s.settings
                .roles
                .iter()
                .find(|r| r.role_id == dev_role)
                .unwrap()
                .set_id
                .as_deref(),
            Some("docs-only")
        );
        let err = invoke_json(
            &main,
            "remove_permission_set",
            serde_json::json!({ "setId": "docs-only" }),
        )
        .expect_err("in use");
        assert!(
            err["message"]
                .as_str()
                .unwrap()
                .contains("still used by 1 role(s)"),
            "{err}"
        );
        let _ = perms(invoke_json(
            &main,
            "assign_permissions",
            serde_json::json!({ "target": "role", "id": dev_role }),
        ));
        let _ = perms(invoke_json(
            &main,
            "remove_permission_set",
            serde_json::json!({ "setId": "docs-only" }),
        ));
        // A department's limit.
        let org: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Development", "description": "",
                "head": { "roleId": role_id(&body(invoke(&main, "get_organization")), "Manager"),
                          "title": "Development Manager", "vacant": true },
            }}),
        ));
        let dept = org.departments[0].id.clone();
        let s = perms(invoke_json(
            &main,
            "assign_permissions",
            serde_json::json!({ "target": "department", "id": dept, "setId": "read-only" }),
        ));
        assert_eq!(
            s.settings.departments[0].set_id.as_deref(),
            Some("read-only")
        );
        // Rules and options.
        let s = perms(invoke_json(
            &main,
            "set_command_rules",
            serde_json::json!({ "rules": { "approved": ["cargo test *"], "ask": [], "blocked": ["curl *"] } }),
        ));
        assert_eq!(s.settings.commands.approved, ["cargo test *"]);
        let s = perms(invoke_json(
            &main,
            "set_blocked_files",
            serde_json::json!({ "patterns": [".env", "*.pem"] }),
        ));
        assert_eq!(s.settings.blocked_files, [".env", "*.pem"]);
        let s = perms(invoke_json(
            &main,
            "set_sensitive_rule",
            serde_json::json!({ "kind": "dns", "rule": "block" }),
        ));
        let dns = s
            .settings
            .sensitive
            .iter()
            .find(|k| k.kind == plenipo_guard::SensitiveKind::Dns)
            .unwrap();
        assert_eq!(dns.rule, plenipo_guard::SensitiveRule::Block);
        let s = perms(invoke_json(
            &main,
            "set_guard_options",
            serde_json::json!({ "options": { "approvalMinutes": 5 } }),
        ));
        assert_eq!(s.settings.options.approval_minutes, 5);
        // The Vault: the value goes in and never comes back out.
        let s = perms(invoke_json(
            &main,
            "save_secret",
            serde_json::json!({ "input": {
                "name": "GitHub token", "envVar": "GH_TOKEN", "programs": ["gh"],
                "value": "ghp_ipc_test_value_123",
            }}),
        ));
        let secret = s.settings.secrets[0].clone();
        assert_eq!(s.vault.stored, std::slice::from_ref(&secret.id));
        assert!(!serde_json::to_string(&s)
            .unwrap()
            .contains("ghp_ipc_test_value_123"));
        let s = perms(invoke_json(
            &main,
            "remove_secret",
            serde_json::json!({ "secretId": secret.id }),
        ));
        assert!(s.settings.secrets.is_empty());
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let events = ledger.recent_events(500).unwrap();
        assert!(!format!("{events:?}").contains("ghp_ipc_test_value_123"));
        let types: Vec<String> = events.into_iter().map(|e| e.event_type).collect();
        for want in [
            "guard.defaults_added",
            "guard.roles_seeded",
            "guard.set_added",
            "guard.role_assigned",
            "guard.set_removed",
            "guard.department_limited",
            "guard.commands_changed",
            "guard.files_changed",
            "guard.sensitive_changed",
            "guard.options_changed",
            "vault.secret_added",
            "vault.secret_removed",
        ] {
            assert!(types.iter().any(|t| t == want), "{want}");
        }
    }

    #[test]
    fn a_project_limit_must_be_one_of_the_permission_sets() {
        let app = app();
        let main = window(&app, "main");
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let org: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Development", "description": "",
                "head": { "roleId": role_id(&org, "Manager"), "title": "Development Manager", "vacant": true },
            }}),
        ));
        let dept = org.departments[0].id.clone();
        let project = |limit: &str| {
            serde_json::json!({ "input": {
                "name": "Website", "description": "", "allowedRuntimes": [],
                "capabilityProfile": limit, "departmentId": dept,
                "coordinator": { "roleId": role_id(&org, "Supervisor"), "title": "Website Supervisor", "vacant": true },
            }})
        };
        let err =
            invoke_json(&main, "create_project", project("development")).expect_err("unknown set");
        assert!(
            err["message"]
                .as_str()
                .unwrap()
                .contains("not one of your permission sets"),
            "{err}"
        );
        let org: plenipo_workforce::OrgSnapshot =
            body(invoke_json(&main, "create_project", project("read-only")));
        assert_eq!(
            org.projects[0].capability_profile.as_deref(),
            Some("read-only")
        );
        let s = perms(invoke(&main, "get_permissions"));
        let p = &s.settings.projects[0];
        assert_eq!(p.set_id.as_deref(), Some("read-only"));
        assert!(p.problem.as_deref().unwrap().contains("has no folder"));
    }

    #[test]
    fn approvals_are_answered_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let ledger = app
            .state::<std::sync::Arc<plenipo_ledger::Ledger>>()
            .inner()
            .clone();
        let task = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "owner".into(),
                    objective: "work".into(),
                    priority: 2,
                    ..plenipo_ledger::NewTask::default()
                },
                "owner",
            )
            .unwrap();
        ledger
            .transition_task(&task.id, plenipo_ledger::TaskState::Running, "w", None)
            .unwrap();
        let far = plenipo_ledger::now_ms() + 600_000;
        let a = ledger
            .request_action_approval(
                &task.id,
                "shell.exec",
                &serde_json::json!({ "summary": "run npm publish", "worker": "Backend Developer",
                                     "capability": "shell.exec", "riskLabel": "Runs a program" }),
                far,
                "agent:codex",
            )
            .unwrap();
        let q: plenipo_capabilities::ApprovalQueue = body(invoke(&main, "get_approvals"));
        assert_eq!(q.pending.len(), 1);
        assert_eq!(q.pending[0].summary, "run npm publish");
        assert_eq!(q.pending[0].capability_label, "Run programs");
        assert!(!q.pending[0].waiting, "no worker is waiting in this test");
        let q: plenipo_capabilities::ApprovalQueue = body(invoke_json(
            &main,
            "resolve_approval",
            serde_json::json!({ "approvalId": a.id, "approve": false }),
        ));
        assert!(q.pending.is_empty());
        assert_eq!(
            q.recent[0].status,
            plenipo_capabilities::ApprovalStatus::Rejected
        );
        assert_eq!(q.recent[0].resolved_by.as_deref(), Some("owner"));
        assert_eq!(
            ledger.task(&task.id).unwrap().unwrap().state,
            plenipo_ledger::TaskState::Running,
            "the task continues"
        );
        let err = invoke_json(
            &main,
            "resolve_approval",
            serde_json::json!({ "approvalId": a.id, "approve": true }),
        )
        .expect_err("answered once");
        assert!(
            err["message"].as_str().unwrap().contains("already refused"),
            "{err}"
        );
        let err = invoke_json(
            &main,
            "revoke_grant",
            serde_json::json!({ "grantId": SESSION }),
        )
        .expect_err("no such grant");
        assert_eq!(err["kind"], "invalidInput");
    }

    #[test]
    fn permission_commands_validate_input_and_accept_no_extra_fields() {
        let app = app();
        let main = window(&app, "main");
        for (cmd, args) in [
            (
                "save_permission_set",
                serde_json::json!({ "input": { "id": "../x", "name": "X", "description": "", "levels": {} } }),
            ),
            (
                "save_permission_set",
                serde_json::json!({ "input": { "name": "X", "description": "", "levels": { "root.everything": "allowed" } } }),
            ),
            (
                "save_permission_set",
                serde_json::json!({ "input": { "name": "X", "description": "", "levels": { "shell.exec": "always" } } }),
            ),
            (
                "save_permission_set",
                serde_json::json!({ "input": { "name": "X", "description": "", "levels": {}, "executable": "/bin/sh" } }),
            ),
            (
                "save_permission_set",
                serde_json::json!({ "input": { "name": "", "description": "", "levels": {} } }),
            ),
            (
                "remove_permission_set",
                serde_json::json!({ "setId": "developer" }),
            ),
            (
                "remove_permission_set",
                serde_json::json!({ "setId": "Developer!" }),
            ),
            (
                "assign_permissions",
                serde_json::json!({ "target": "project", "id": SESSION, "setId": "developer" }),
            ),
            (
                "assign_permissions",
                serde_json::json!({ "target": "role", "id": "../x", "setId": "developer" }),
            ),
            (
                "assign_permissions",
                serde_json::json!({ "target": "role", "id": SESSION, "setId": "developer" }),
            ),
            (
                "set_command_rules",
                serde_json::json!({ "rules": { "approved": ["/bin/rm *"], "ask": [], "blocked": [] } }),
            ),
            (
                "set_command_rules",
                serde_json::json!({ "rules": { "approved": [], "ask": [], "blocked": [], "shell": "bash" } }),
            ),
            (
                "set_blocked_files",
                serde_json::json!({ "patterns": ["!"] }),
            ),
            (
                "set_sensitive_rule",
                serde_json::json!({ "kind": "dns", "rule": "allow" }),
            ),
            (
                "set_sensitive_rule",
                serde_json::json!({ "kind": "everything", "rule": "ask" }),
            ),
            (
                "set_guard_options",
                serde_json::json!({ "options": { "approvalMinutes": 0 } }),
            ),
            (
                "set_guard_options",
                serde_json::json!({ "options": { "approvalMinutes": 5, "autoApprove": true } }),
            ),
            (
                "save_secret",
                serde_json::json!({ "input": { "name": "X", "programs": [] } }),
            ),
            (
                "save_secret",
                serde_json::json!({ "input": { "name": "X", "programs": ["gh"], "envVar": "PATH", "value": "abcdefgh" } }),
            ),
            (
                "save_secret",
                serde_json::json!({ "input": { "name": "X", "programs": [], "value": "x".repeat(40_001) } }),
            ),
            ("remove_secret", serde_json::json!({ "secretId": "../x" })),
            (
                "resolve_approval",
                serde_json::json!({ "approvalId": "../x", "approve": true }),
            ),
            (
                "resolve_approval",
                serde_json::json!({ "approvalId": SESSION, "approve": true }),
            ),
            ("revoke_grant", serde_json::json!({ "grantId": "../x" })),
        ] {
            let err = invoke_json(&main, cmd, args.clone()).expect_err(cmd);
            assert!(
                err["kind"] == "invalidInput" || err.is_string(),
                "{cmd} {args}: {err}"
            );
        }
        // Nothing was changed by the refusals.
        let s = perms(invoke(&main, "get_permissions"));
        assert!(s.settings.sets.iter().all(|x| x.built_in));
        assert_eq!(s.settings.options.approval_minutes, 10);
        assert!(s.settings.secrets.is_empty());
    }

    #[test]
    fn permission_commands_denied_for_ungranted_windows_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        for cmd in [
            "get_permissions",
            "save_permission_set",
            "remove_permission_set",
            "assign_permissions",
            "set_command_rules",
            "set_blocked_files",
            "set_sensitive_rule",
            "set_guard_options",
            "save_secret",
            "remove_secret",
            "get_approvals",
            "resolve_approval",
            "revoke_grant",
        ] {
            let args = serde_json::json!({ "approvalId": SESSION, "approve": true });
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    // ---- The Development department (Phase 8) ----------------------------------------------

    #[test]
    fn the_development_department_is_set_up_and_reported_through_ipc() {
        let app = app();
        let main = window(&app, "main");
        let project = |name: &str| {
            serde_json::json!({
                "name": name, "description": "",
                "repositoryUrl": "https://github.com/example/website",
                "localPath": "D:\\projects\\website",
                "allowedRuntimes": ["claude-code", "codex"],
            })
        };
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "set_up_development",
            serde_json::json!({ "input": { "project": project("Website"), "runtimeId": "claude-code" } }),
        ));
        assert_eq!(s.departments[0].name, "Development");
        let website = s.projects.iter().find(|p| p.name == "Website").unwrap();
        assert!(website.branch_per_objective);
        assert_eq!(
            s.positions
                .iter()
                .filter(|p| p.reports_to == website.coordinator_position_id)
                .count(),
            4,
            "the standard team"
        );
        // The template chooses the department and the supervisor; unknown fields are refused.
        let mut with_department = project("Cloudline");
        with_department["departmentId"] = serde_json::json!(s.departments[0].id);
        for input in [
            serde_json::json!({ "project": with_department }),
            serde_json::json!({ "project": project("Cloudline"), "runtimeId": "--help" }),
        ] {
            let err = invoke_json(
                &main,
                "set_up_development",
                serde_json::json!({ "input": input }),
            )
            .expect_err("refused");
            assert_eq!(err["kind"], "invalidInput", "{err}");
        }
        let err = invoke_json(
            &main,
            "set_up_development",
            serde_json::json!({ "input": { "project": project("Cloudline"), "extra": true } }),
        )
        .expect_err("unknown fields are refused");
        assert!(err.to_string().contains("unknown field"), "{err}");

        // A project's work: nothing yet.
        let work: plenipo_workforce::ProjectWork = body(invoke_json(
            &main,
            "get_project_work",
            serde_json::json!({ "projectId": website.id }),
        ));
        assert!(work.objectives.is_empty() && work.working_copies.is_empty());
        // Every Phase 8 command checks its IDs.
        for (cmd, args) in [
            (
                "get_project_work",
                serde_json::json!({ "projectId": "../x" }),
            ),
            ("get_objective_report", serde_json::json!({ "taskId": "x" })),
            (
                "remove_workspace",
                serde_json::json!({ "workspaceId": "x" }),
            ),
            (
                "give_objective",
                serde_json::json!({ "positionId": SESSION, "objective": "Hi", "projectId": "x" }),
            ),
        ] {
            let err = invoke_json(&main, cmd, args).expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}: {err}");
        }
        for (cmd, args) in [
            (
                "get_project_work",
                serde_json::json!({ "projectId": SESSION }),
            ),
            (
                "get_objective_report",
                serde_json::json!({ "taskId": SESSION }),
            ),
            (
                "remove_workspace",
                serde_json::json!({ "workspaceId": SESSION }),
            ),
        ] {
            assert!(invoke_json(&main, cmd, args).is_err(), "{cmd}: unknown ID");
        }
        // A synthetic task's report: the task alone, nothing handed on.
        let task: plenipo_ledger::Task = body(invoke(&main, "create_synthetic_task"));
        let report: plenipo_workforce::ObjectiveReport = body(invoke_json(
            &main,
            "get_objective_report",
            serde_json::json!({ "taskId": task.id }),
        ));
        assert_eq!(report.root_task_id, task.id);
        assert_eq!(report.tasks.len(), 1);
        assert!(report.files.is_empty() && report.approvals.is_empty());
    }

    #[test]
    fn development_commands_denied_for_ungranted_windows_and_remote_origins() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        for cmd in [
            "set_up_development",
            "get_objective_report",
            "get_project_work",
            "remove_workspace",
        ] {
            let args = serde_json::json!({ "projectId": SESSION, "taskId": SESSION });
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    #[test]
    fn control_and_websites_through_ipc() {
        use plenipo_capabilities::control::ControlStatus;
        let app = app();
        let main = window(&app, "main");
        // Nobody uses the browser or the desktop yet.
        let status: ControlStatus = body(invoke(&main, "get_control_status"));
        assert!(!status.stopped && status.sessions.is_empty());
        // The emergency stop holds until allowed again.
        let status: ControlStatus = body(invoke(&main, "stop_all_control"));
        assert!(status.stopped);
        let status: ControlStatus = body(invoke(&main, "allow_control"));
        assert!(!status.stopped);
        // Take over: session IDs are checked; nobody to take over is refused.
        for bad in [
            "x",
            "browser:../x",
            "window:0f8fad5b-d9cb-469f-a165-70867728950e",
        ] {
            let err = invoke_json(
                &main,
                "take_over_control",
                serde_json::json!({ "sessionId": bad }),
            )
            .expect_err(bad);
            assert_eq!(err["kind"], "invalidInput", "{bad}: {err}");
        }
        assert!(invoke_json(
            &main,
            "take_over_control",
            serde_json::json!({ "sessionId": format!("browser:{SESSION}") }),
        )
        .is_err());
        // Website lists: cleaned, and non-websites refused with the reason.
        let snap: plenipo_capabilities::PermissionsSnapshot = body(invoke_json(
            &main,
            "set_website_rules",
            serde_json::json!({ "rules": {
                "allowed": ["https://Example.com/x"], "blocked": ["linkedin.com"], "others": "block"
            } }),
        ));
        assert_eq!(snap.settings.websites.allowed, ["example.com"]);
        let err = invoke_json(
            &main,
            "set_website_rules",
            serde_json::json!({ "rules": { "allowed": ["file:///etc"], "blocked": [], "others": "ask" } }),
        )
        .expect_err("not a website");
        assert!(
            err["message"].as_str().unwrap().contains("not a website"),
            "{err}"
        );
        let err = invoke_json(
            &main,
            "set_website_rules",
            serde_json::json!({ "rules": { "allowed": [], "blocked": [], "others": "allow" } }),
        )
        .expect_err("others is ask or block");
        assert!(err.to_string().contains("unknown variant"), "{err}");
        // The owner's switches (ADR-023): the defaults, a change, and unknown fields refused.
        assert!(snap.settings.switches.browser && !snap.settings.switches.desktop);
        let snap: plenipo_capabilities::PermissionsSnapshot = body(invoke_json(
            &main,
            "set_switches",
            serde_json::json!({ "switches": {
                "browser": false, "desktop": true, "sendWithoutAsking": true,
                "buyWithoutAsking": false, "signInWithoutAsking": false,
                "captchaToOwner": false, "screenshots": false
            } }),
        ));
        let s = &snap.settings.switches;
        assert!(!s.browser && s.desktop && s.send_without_asking && !s.screenshots);
        let err = invoke_json(
            &main,
            "set_switches",
            serde_json::json!({ "switches": { "solveCaptchas": true } }),
        )
        .expect_err("no such switch");
        assert!(err.to_string().contains("unknown field"), "{err}");
        // Screenshots only by ID.
        let err = invoke_json(
            &main,
            "get_screenshot",
            serde_json::json!({ "artifactId": "../x" }),
        )
        .expect_err("bad id");
        assert_eq!(err["kind"], "invalidInput");
        assert!(invoke_json(
            &main,
            "get_screenshot",
            serde_json::json!({ "artifactId": SESSION })
        )
        .is_err());
        // The browser's status needs no browser to be running.
        let browser: plenipo_capabilities::browser::BrowserStatus =
            body(invoke(&main, "get_browser_status"));
        assert!(!browser.running);
        assert_eq!(browser.choice, plenipo_guard::BrowserChoice::Automatic);
        // Which browser (ADR-028): Edge or Chrome, and nothing else.
        let browser: plenipo_capabilities::browser::BrowserStatus = body(invoke_json(
            &main,
            "set_browser_choice",
            serde_json::json!({ "choice": "chrome" }),
        ));
        assert_eq!(browser.choice, plenipo_guard::BrowserChoice::Chrome);
        let err = invoke_json(
            &main,
            "set_browser_choice",
            serde_json::json!({ "choice": "/usr/bin/firefox" }),
        )
        .expect_err("no such browser");
        assert!(err.to_string().contains("unknown variant"), "{err}");
        // Built-in roles keep their instructions; custom ones can be edited.
        let org: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        let built_in = org.roles.iter().find(|r| r.template).unwrap();
        assert!(!built_in.job.duties.is_empty());
        let edit = serde_json::json!({ "name": "X", "description": "", "job": {
            "duties": ["a"], "returns": [], "limits": [], "askLead": [] } });
        let err = invoke_json(
            &main,
            "update_role",
            serde_json::json!({ "roleId": built_in.id, "input": edit }),
        )
        .expect_err("built-in");
        assert!(
            err["message"]
                .as_str()
                .unwrap()
                .contains("built-in roles keep"),
            "{err}"
        );
        let org: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_role",
            serde_json::json!({ "input": { "name": "Bookkeeper", "description": "Books.",
                "kind": "worker", "staffing": "onDemand",
                "job": { "duties": ["enter receipts"], "returns": [], "limits": ["never pay"], "askLead": [] } } }),
        ));
        let role = org.roles.iter().find(|r| r.name == "Bookkeeper").unwrap();
        assert_eq!(role.job.limits, ["never pay"]);
        let org: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "update_role",
            serde_json::json!({ "roleId": role.id, "input": edit }),
        ));
        assert!(org
            .roles
            .iter()
            .any(|r| r.name == "X" && r.job.duties == ["a"]));
    }

    /// Phase 11: the server list, sign-ins going only one way, identity checks, and tests.
    #[test]
    fn servers_are_configuration_only() {
        let app = app();
        let main = window(&app, "main");
        let snap: plenipo_capabilities::ServersSnapshot = body(invoke(&main, "get_servers"));
        assert!(snap.servers.is_empty());
        let ops = snap
            .roles
            .iter()
            .find(|r| r.name == "Operations Engineer")
            .expect("the Operations Engineer role");
        assert!(ops.can_connect, "it starts with the Servers set");
        assert_eq!(snap.classes.len(), 6);
        let server = |host: &str, roles: serde_json::Value| {
            serde_json::json!({ "input": {
                "name": "Dev box", "host": host, "port": 22, "user": "deploy",
                "environment": "development", "signIn": "password",
                "password": "a-password-value", "roles": roles,
                "classes": ["look"], "approval": "changes", "folders": ["/srv/app"], "forwards": []
            } })
        };
        let err = invoke_json(
            &main,
            "save_server",
            server("user@host", serde_json::json!([])),
        )
        .expect_err("not an address");
        assert!(
            err["message"]
                .as_str()
                .unwrap()
                .contains("not a server address"),
            "{err}"
        );
        let err = invoke_json(
            &main,
            "save_server",
            server("dev.test", serde_json::json!(["../x"])),
        )
        .expect_err("bad role id");
        assert_eq!(err["kind"], "invalidInput");
        let snap: plenipo_capabilities::ServersSnapshot = body(invoke_json(
            &main,
            "save_server",
            server("dev.test", serde_json::json!([ops.id])),
        ));
        let dev = &snap.servers[0];
        assert!(dev.stored.password);
        assert!(dev
            .problem
            .as_deref()
            .unwrap()
            .contains("not checked and pinned"));
        // The password went in, and never comes back out.
        let text = serde_json::to_string(&snap).unwrap();
        assert!(!text.contains("a-password-value"));
        // Reading an identity where nothing listens says so plainly.
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = invoke_json(
            &main,
            "check_server_identity",
            serde_json::json!({ "host": "127.0.0.1", "port": closed }),
        )
        .expect_err("nothing listens");
        assert!(
            err["message"].as_str().unwrap().contains("could not reach"),
            "{err}"
        );
        let test: plenipo_capabilities::ServerTest = body(invoke_json(
            &main,
            "test_server",
            serde_json::json!({ "serverId": dev.server.id }),
        ));
        assert!(
            !test.ok && test.message.contains("not checked and pinned"),
            "{test:?}"
        );
        // Disconnecting a worker checks the session ID; nobody to disconnect is refused.
        assert!(invoke_json(
            &main,
            "take_over_control",
            serde_json::json!({ "sessionId": format!("server:{SESSION}") }),
        )
        .is_err());
        let snap: plenipo_capabilities::ServersSnapshot = body(invoke_json(
            &main,
            "remove_server",
            serde_json::json!({ "serverId": dev.server.id }),
        ));
        assert!(snap.servers.is_empty());
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for cmd in [
            "get_servers",
            "save_server",
            "remove_server",
            "check_server_identity",
            "test_server",
        ] {
            assert!(invoke(&other, cmd).is_err(), "{cmd}");
            assert!(invoke(&sign, cmd).is_err(), "the sign must not reach {cmd}");
            assert!(
                invoke_from(&main, cmd, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    #[test]
    fn learning_through_ipc() {
        use plenipo_workforce::LearningSnapshot;
        let app = app();
        let main = window(&app, "main");
        let snap: LearningSnapshot = body(invoke(&main, "get_learning"));
        assert!(snap.enabled && snap.waiting.is_empty() && snap.kept.is_empty());
        let snap: LearningSnapshot = body(invoke_json(
            &main,
            "set_learning",
            serde_json::json!({ "enabled": false }),
        ));
        assert!(!snap.enabled);
        for (cmd, args) in [
            (
                "set_role_learning",
                serde_json::json!({ "roleId": "../x", "auto": true }),
            ),
            (
                "decide_lesson",
                serde_json::json!({ "lessonId": "../x", "keep": true }),
            ),
            ("remove_lesson", serde_json::json!({ "lessonId": "x y" })),
        ] {
            let err = invoke_json(&main, cmd, args).expect_err(cmd);
            assert_eq!(err["kind"], "invalidInput", "{cmd}: {err}");
        }
        // A role that does not exist, a lesson that does not exist.
        assert!(invoke_json(
            &main,
            "set_role_learning",
            serde_json::json!({ "roleId": SESSION, "auto": true }),
        )
        .is_err());
        assert!(invoke_json(
            &main,
            "decide_lesson",
            serde_json::json!({ "lessonId": SESSION, "keep": false }),
        )
        .is_err());
        let other = window(&app, "untrusted");
        for cmd in [
            "get_learning",
            "set_learning",
            "set_role_learning",
            "decide_lesson",
            "remove_lesson",
        ] {
            assert!(invoke(&other, cmd).is_err(), "{cmd}");
            assert!(
                invoke_from(&main, cmd, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    /// The pages (Phase 12): Home, and the history and records of a department, project,
    /// position, or task. The main window only; bad scopes and IDs are refused.
    #[test]
    fn the_pages_read_through_ipc_for_the_main_window_only() {
        use plenipo_capabilities::TaskRecord;
        use plenipo_ledger::{LedgerEvent, WorkRecord};
        use plenipo_workforce::HomeView;
        let app = app();
        let main = window(&app, "main");
        let home: HomeView = body(invoke(&main, "get_home"));
        assert!(home.current.is_empty() && home.finished.is_empty() && home.stuck.is_empty());
        let all = serde_json::json!({ "scope": { "kind": "all" }, "limit": 20 });
        let events: Vec<LedgerEvent> = body(invoke_json(&main, "get_scope_events", all.clone()));
        assert!(events.len() <= 20);
        let err = invoke_json(
            &main,
            "get_scope_events",
            serde_json::json!({ "scope": { "kind": "department", "id": "" }, "limit": 5 }),
        )
        .expect_err("an empty ID");
        assert_eq!(err["kind"], "invalidInput");
        // A department that does not exist is plainly refused.
        assert!(invoke_json(
            &main,
            "get_scope_events",
            serde_json::json!({ "scope": { "kind": "department", "id": SESSION }, "limit": 5 }),
        )
        .is_err());
        let task = serde_json::json!({ "taskId": SESSION, "limit": 10 });
        let tree: Vec<LedgerEvent> = body(invoke_json(&main, "get_task_events", task.clone()));
        assert!(tree.is_empty());
        let project = serde_json::json!({ "projectId": SESSION });
        let record: WorkRecord = body(invoke_json(&main, "get_project_record", project.clone()));
        assert!(record.pull_requests.is_empty());
        let task_record: TaskRecord = body(invoke_json(
            &main,
            "get_task_record",
            serde_json::json!({ "taskId": SESSION }),
        ));
        assert!(task_record.approvals.is_empty());
        for bad in ["../x", "not an id"] {
            assert!(invoke_json(
                &main,
                "get_task_record",
                serde_json::json!({ "taskId": bad })
            )
            .is_err());
            assert!(invoke_json(
                &main,
                "get_project_record",
                serde_json::json!({ "projectId": bad })
            )
            .is_err());
        }
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for (cmd, args) in [
            ("get_home", serde_json::json!({})),
            ("get_scope_events", all),
            ("get_task_events", task),
            ("get_project_record", project),
            ("get_task_record", serde_json::json!({ "taskId": SESSION })),
        ] {
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_json(&sign, cmd, args.clone()).is_err(),
                "the sign must not reach {cmd}"
            );
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    /// The owner's terminal (Phase 12, ADR-031): its commands work in the main window, take a
    /// place and never a program, and are refused to every other window, the sign, and web
    /// pages, so nothing but the owner can type into it.
    #[test]
    fn the_terminal_is_the_owners_alone() {
        use plenipo_capabilities::{TerminalInfo, TerminalSettings};
        let app = app();
        let main = window(&app, "main");
        let settings: TerminalSettings = body(invoke(&main, "get_terminal_settings"));
        assert_eq!(settings.shells.len(), 3);
        assert!(settings.open.is_empty());
        let open = |place: serde_json::Value| serde_json::json!({ "place": place, "cols": 80, "rows": 24, "events": "__CHANNEL__:7" });
        // A place, never a program: anything more, or a server ID that is not an ID, is refused.
        for bad in [
            serde_json::json!({ "kind": "thisPc", "program": "calc.exe" }),
            serde_json::json!({ "kind": "server", "serverId": "../x" }),
            serde_json::json!({ "kind": "program", "path": "C:/Windows/System32/cmd.exe" }),
        ] {
            assert!(
                invoke_json(&main, "open_terminal", open(bad.clone())).is_err(),
                "{bad}"
            );
        }
        let err = invoke_json(
            &main,
            "open_terminal",
            open(serde_json::json!({ "kind": "server", "serverId": SESSION })),
        )
        .expect_err("no such server");
        assert!(
            err["message"]
                .as_str()
                .unwrap()
                .contains("no longer in the list"),
            "{err}"
        );
        let info: TerminalInfo = body(invoke_json(
            &main,
            "open_terminal",
            open(serde_json::json!({ "kind": "thisPc" })),
        ));
        assert_eq!(info.title, "This PC");
        let id = info.id.clone();
        let typed = serde_json::json!({ "terminalId": id, "data": "echo hi\r" });
        let size = serde_json::json!({ "terminalId": id, "cols": 100, "rows": 30 });
        assert!(invoke_json(&main, "write_terminal", typed.clone()).is_ok());
        assert!(invoke_json(&main, "resize_terminal", size.clone()).is_ok());
        let err = invoke_json(
            &main,
            "write_terminal",
            serde_json::json!({ "terminalId": id, "data": "x".repeat(70_000) }),
        )
        .expect_err("too much at once");
        assert_eq!(err["kind"], "invalidInput");
        // Nobody else, even with a real terminal and valid arguments: the refusal comes from the
        // permission list, not from bad input.
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        let close = serde_json::json!({ "terminalId": id });
        let stop = serde_json::json!({ "commandId": SESSION });
        for (cmd, args) in [
            ("get_terminal_settings", serde_json::json!({})),
            (
                "set_terminal_shell",
                serde_json::json!({ "shell": "commandPrompt" }),
            ),
            (
                "open_terminal",
                open(serde_json::json!({ "kind": "thisPc" })),
            ),
            ("write_terminal", typed.clone()),
            ("resize_terminal", size.clone()),
            ("close_terminal", close.clone()),
            ("stop_server_command", stop.clone()),
        ] {
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_json(&sign, cmd, args.clone()).is_err(),
                "the sign must not reach {cmd}"
            );
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd} from a web page"
            );
        }
        let open_now: TerminalSettings = body(invoke(&main, "get_terminal_settings"));
        assert_eq!(open_now.open.len(), 1, "still open, untouched");
        assert!(invoke_json(&main, "close_terminal", close).is_ok());
        // Stopping a command that is not running is refused plainly.
        let err = invoke_json(&main, "stop_server_command", stop).expect_err("nothing runs");
        assert!(
            err["message"].as_str().unwrap().contains("not running"),
            "{err}"
        );
    }

    #[test]
    fn notices_and_local_paths_are_the_main_windows_alone() {
        use plenipo_ledger::NoticeSettings;
        let app = app();
        let main = window(&app, "main");
        let settings: NoticeSettings = body(invoke(&main, "get_notice_settings"));
        assert_eq!(settings, NoticeSettings::default());
        let quiet = serde_json::json!({ "settings": {
            "approvals": true, "checks": true, "problems": true, "finished": false,
            "lessons": false, "onlyWhenAway": false,
        } });
        let kept: NoticeSettings = body(invoke_json(&main, "set_notice_settings", quiet.clone()));
        assert!(!kept.finished && !kept.lessons && !kept.only_when_away);
        // Anything else in the choices is refused.
        let extra = serde_json::json!({ "settings": { "approvals": "yes" } });
        assert!(invoke_json(&main, "set_notice_settings", extra).is_err());
        // Where Plenipo keeps its files: shown, never chosen from the screen.
        let paths: Vec<plenipo_core::LocalPath> = body(invoke(&main, "get_local_paths"));
        assert_eq!(paths.len(), 3, "a temporary Ledger has only its own three");
        assert!(paths.iter().all(|p| !p.kept));
        // The test notice says Plenipo's own words; nothing from the page goes into it.
        assert!(invoke(&main, "send_test_notice").is_ok());
        let shown = app.state::<Arc<notices::Notices>>().kept();
        assert_eq!(shown, vec![notices::test_notice()]);
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for (cmd, args) in [
            ("get_notice_settings", serde_json::json!({})),
            ("set_notice_settings", quiet.clone()),
            ("send_test_notice", serde_json::json!({})),
            ("get_local_paths", serde_json::json!({})),
            (
                "plugin:notification|notify",
                serde_json::json!({ "options": { "title": "x" } }),
            ),
            (
                "plugin:notification|is_permission_granted",
                serde_json::json!({}),
            ),
        ] {
            assert!(invoke_json(&other, cmd, args.clone()).is_err(), "{cmd}");
            assert!(
                invoke_json(&sign, cmd, args.clone()).is_err(),
                "the sign must not reach {cmd}"
            );
            assert!(
                invoke_with(&main, cmd, args, "https://example.com").is_err(),
                "{cmd} from a web page"
            );
        }
        // The page cannot send a notice of its own either: the plugin is there, but its
        // commands are granted to no window.
        let err = invoke_json(
            &main,
            "plugin:notification|notify",
            serde_json::json!({ "options": { "title": "x" } }),
        )
        .expect_err("not granted");
        assert!(err.to_string().contains("not allowed"), "{err}");
        assert_eq!(app.state::<Arc<notices::Notices>>().kept().len(), 1);
    }

    /// Phase 13: recovery, Start and close, backups and restore, the diagnostics file, and
    /// updates are the main window's alone, and none takes a path.
    #[test]
    fn keeping_plenipo_dependable_is_the_main_windows_alone() {
        use plenipo_core::{RecoveryStatus, StartAndClose, UpdateState, UpdateStatus};
        let app = app();
        let main = window(&app, "main");
        let status: RecoveryStatus = body(invoke(&main, "get_recovery_status"));
        assert_eq!(
            status,
            RecoveryStatus::default(),
            "nothing to recover in a new Ledger"
        );
        let backups: plenipo_ledger::LedgerBackups = body(invoke(&main, "list_ledger_backups"));
        assert!(backups.backups.is_empty() && backups.pending_restore.is_none());
        let start: StartAndClose = body(invoke(&main, "get_start_and_close"));
        assert!(!start.can_start_with_windows && !start.start_with_windows);
        assert_eq!(
            start.close_window,
            plenipo_core::CloseWindow::KeepWhileWorking
        );
        // The close choice is kept; Start with Windows cannot be turned on without the plugin.
        let changed: StartAndClose = body(invoke_json(
            &main,
            "set_start_and_close",
            serde_json::json!({ "input": { "startWithWindows": false, "closeWindow": "alwaysKeep" } }),
        ));
        assert_eq!(changed.close_window, plenipo_core::CloseWindow::AlwaysKeep);
        let err = invoke_json(
            &main,
            "set_start_and_close",
            serde_json::json!({ "input": { "startWithWindows": true, "closeWindow": "quit" } }),
        )
        .expect_err("not available here");
        assert!(
            err["message"].as_str().unwrap().contains("not available"),
            "{err}"
        );
        assert!(invoke_json(
            &main,
            "set_start_and_close",
            serde_json::json!({ "input": { "startWithWindows": false, "closeWindow": "quit", "path": "x" } }),
        )
        .is_err());
        let updates: UpdateStatus = body(invoke(&main, "get_update_status"));
        assert_eq!(updates.state, UpdateState::NotChecked);
        assert!(!updates.can_install, "a test copy has no updater key");
        assert!(invoke_json(
            &main,
            "window_alive",
            serde_json::json!({ "visible": true })
        )
        .is_ok());
        // A restore names a backup from the list: never a path, never on a temporary Ledger.
        for name in [
            "../../plenipo.db",
            "C:/Windows/System32/evil.db",
            "daily-backup-1790000000000.db",
        ] {
            let err = invoke_json(
                &main,
                "restore_ledger_backup",
                serde_json::json!({ "name": name }),
            )
            .expect_err(name);
            assert_eq!(err["kind"], "invalidInput", "{name}: {err}");
        }
        // No diagnostics file for a temporary run, and no update to install.
        assert!(invoke(&main, "save_diagnostics_file").is_err());
        let err = invoke_json(
            &main,
            "install_update",
            serde_json::json!({ "stopWork": true }),
        )
        .expect_err("nothing to install");
        assert_eq!(err["kind"], "invalidInput");
        // Run again: a real task id only, and only an objective that stopped.
        assert!(invoke_json(&main, "run_again", serde_json::json!({ "taskId": "../x" })).is_err());
        let err = invoke_json(
            &main,
            "reset_settings",
            serde_json::json!({ "key": "organization" }),
        )
        .expect_err("only damaged settings Plenipo knows");
        assert_eq!(err["kind"], "invalidInput");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        let task = "0f8fad5b-d9cb-469f-a165-70867728950e";
        for (cmd, args) in [
            ("get_recovery_status", serde_json::json!({})),
            ("run_again", serde_json::json!({ "taskId": task })),
            ("dismiss_recovery", serde_json::json!({ "id": task })),
            ("dismiss_window_recovery", serde_json::json!({})),
            ("window_alive", serde_json::json!({ "visible": true })),
            ("reset_settings", serde_json::json!({ "key": "guard" })),
            ("get_start_and_close", serde_json::json!({})),
            (
                "set_start_and_close",
                serde_json::json!({ "input": { "startWithWindows": true, "closeWindow": "quit" } }),
            ),
            ("list_ledger_backups", serde_json::json!({})),
            (
                "restore_ledger_backup",
                serde_json::json!({ "name": "daily-backup-1790000000000.db" }),
            ),
            ("cancel_ledger_restore", serde_json::json!({})),
            ("save_diagnostics_file", serde_json::json!({})),
            ("get_update_status", serde_json::json!({})),
            ("check_for_updates", serde_json::json!({})),
            ("install_update", serde_json::json!({ "stopWork": true })),
        ] {
            // Refused by the permissions (not by the command itself, which would say something
            // else about these arguments).
            let refused = |answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
                           from: &str| {
                let err = answer.expect_err(from);
                assert!(
                    err.to_string().contains("not allowed"),
                    "{cmd} from {from}: {err}"
                );
            };
            refused(invoke_json(&other, cmd, args.clone()), "another window");
            refused(invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                invoke_with(&main, cmd, args, "https://example.com"),
                "a web page",
            );
        }
    }

    /// Start with Windows' own commands, and one-at-a-time's, are granted to no window: only
    /// Plenipo's command (Settings → Start and close) changes the sign-in list.
    #[test]
    fn the_start_with_windows_plugin_is_reachable_by_no_window() {
        let app = app_with_autostart(true);
        let main = window(&app, "main");
        let sign = window(&app, crate::indicator::LABEL);
        for cmd in [
            "plugin:autostart|enable",
            "plugin:autostart|disable",
            "plugin:autostart|is_enabled",
            "plugin:single-instance|anything",
            "plugin:updater|check",
            "plugin:updater|download_and_install",
            "plugin:process|restart",
        ] {
            let err = invoke(&main, cmd).expect_err(cmd);
            assert!(!err.is_null(), "{cmd}");
            assert!(invoke(&sign, cmd).is_err(), "the sign must not reach {cmd}");
        }
        // The plugin is there (so the refusal is the permissions', not a missing command).
        let err = invoke(&main, "plugin:autostart|is_enabled").unwrap_err();
        assert!(err.to_string().contains("not allowed"), "{err}");
    }

    #[test]
    fn the_control_sign_reaches_only_its_three_commands() {
        let app = app();
        let sign = window(&app, crate::indicator::LABEL);
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        assert!(invoke(&sign, "get_control_status").is_ok());
        assert!(invoke(&sign, "stop_all_control").is_ok());
        assert!(invoke(&main, "allow_control").is_ok());
        for cmd in [
            "get_permissions",
            "allow_control",
            "open_browser",
            "set_website_rules",
            "get_organization",
        ] {
            assert!(invoke(&sign, cmd).is_err(), "the sign must not reach {cmd}");
        }
        for cmd in [
            "get_control_status",
            "stop_all_control",
            "take_over_control",
            "allow_control",
            "set_website_rules",
            "set_switches",
            "get_browser_status",
            "set_browser_choice",
            "open_browser",
            "get_screenshot",
            "update_role",
        ] {
            assert!(invoke(&other, cmd).is_err(), "{cmd}");
            assert!(
                invoke_from(&main, cmd, "https://example.com").is_err(),
                "{cmd}"
            );
        }
    }

    // ---- The owner's control over workers (Phase 17) -------------------------------------

    /// The Phase 17 commands (ADR-041, ADR-042, ADR-043, ADR-045): the main window's alone.
    const OWNER_CONTROL: [&str; 15] = [
        "set_model_rule",
        "set_role_learns",
        "set_agent_learning",
        "create_specialty",
        "update_specialty",
        "remove_specialty",
        "archive_department",
        "bring_back_position",
        "bring_back_project",
        "bring_back_department",
        "preview_delete_for_good",
        "delete_for_good",
        "save_to_workforce",
        "hire_from_workforce",
        "delete_saved_agent",
    ];

    /// Arguments that fit every Phase 17 command (each takes the ones it names).
    fn owner_control_args() -> serde_json::Value {
        serde_json::json!({
            "target": { "layer": "organization" }, "rule": {},
            "roleId": SESSION, "learns": true, "positionId": SESSION,
            "input": { "name": "Databases" }, "specialtyId": SESSION,
            "departmentId": SESSION, "projectId": SESSION,
            "kind": "position", "id": SESSION, "save": [],
            "savedId": SESSION, "reportsTo": null, "title": null,
        })
    }

    #[test]
    fn the_owners_control_over_workers_is_the_main_windows_alone() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for cmd in OWNER_CONTROL {
            let args = owner_control_args();
            // Refused by the permissions (not by the command, which answers with its own kind of
            // error, as below).
            let refused = |answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
                           from: &str| {
                let err = answer.expect_err(from);
                assert!(
                    err.to_string().contains("not allowed"),
                    "{cmd} from {from}: {err}"
                );
            };
            refused(invoke_json(&other, cmd, args.clone()), "another window");
            refused(invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                invoke_with(&main, cmd, args.clone(), "https://example.com"),
                "a web page",
            );
            // The main window reaches the command itself.
            if let Err(err) = invoke_json(&main, cmd, args) {
                assert!(err["kind"].is_string(), "{cmd} from the main window: {err}");
            }
        }
    }

    #[test]
    fn the_phase_17_commands_check_what_they_are_given() {
        let app = app();
        let main = window(&app, "main");
        // Each is refused for the reason given, before it reaches the Ledger.
        for (cmd, args, why) in [
            (
                "set_model_rule",
                serde_json::json!({ "target": { "layer": "agent", "id": "../x" }, "rule": {} }),
                "invalid position id",
            ),
            (
                "set_model_rule",
                serde_json::json!({
                    "target": { "layer": "organization" },
                    "rule": { "models": ["x".repeat(500)] },
                }),
                "invalid model id",
            ),
            (
                // A rule has no room for a command, a path, or a key.
                "set_model_rule",
                serde_json::json!({
                    "target": { "layer": "organization" },
                    "rule": { "models": [], "command": "/bin/sh", "apiKey": "x" },
                }),
                "unknown field `",
            ),
            (
                "set_role_learns",
                serde_json::json!({ "roleId": "", "learns": false }),
                "invalid role id",
            ),
            (
                "create_specialty",
                serde_json::json!({ "input": { "roleId": SESSION, "name": "" } }),
                "the specialty's name",
            ),
            (
                "create_specialty",
                serde_json::json!({ "input": {
                    "roleId": SESSION, "name": "Databases", "path": "C:/Windows",
                }}),
                "unknown field `path`",
            ),
            (
                "create_specialty",
                serde_json::json!({ "input": {
                    "roleId": SESSION, "name": "Databases",
                    "suggest": { "needs": vec!["vision"; 9], "minContextTokens": null,
                                 "models": [], "permissions": [] },
                }}),
                "too many suggestions",
            ),
            (
                "preview_delete_for_good",
                serde_json::json!({ "kind": "everything", "id": SESSION }),
                "unknown variant `everything`",
            ),
            (
                "delete_for_good",
                serde_json::json!({ "kind": "position", "id": "../../x", "save": [] }),
                "invalid position id",
            ),
            (
                "delete_for_good",
                serde_json::json!({ "kind": "position", "id": SESSION, "save": ["../x"] }),
                "invalid position id",
            ),
            (
                "hire_from_workforce",
                serde_json::json!({ "savedId": SESSION, "reportsTo": "..", "title": null }),
                "invalid position id",
            ),
        ] {
            let err = invoke_json(&main, cmd, args.clone())
                .expect_err(&format!("{cmd} must refuse {args}"));
            let said = err["message"]
                .as_str()
                .map_or_else(|| err.to_string(), str::to_owned);
            assert!(
                said.contains(why),
                "{cmd} refused {args} with {said}, not {why}"
            );
        }
    }

    #[test]
    fn archive_bring_back_and_delete_for_good_through_ipc() {
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
        let dept = s.departments[0].id.clone();
        let head = s.departments[0].head_position_id.clone().unwrap();
        // A Senior Developer with one of the role's specialties.
        let developer = org
            .roles
            .iter()
            .find(|r| r.name == "Senior Developer")
            .unwrap();
        let databases = developer
            .specialties
            .iter()
            .find(|s| s.name == "Database")
            .expect("a built-in specialty")
            .id
            .clone();
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "hire_position",
            serde_json::json!({ "input": {
                "roleId": developer.id, "title": "Database Developer", "reportsTo": head,
                "runtimeId": "claude-code", "specialtyId": databases,
            }}),
        ));
        let dev = s
            .positions
            .iter()
            .find(|p| p.title == "Database Developer")
            .unwrap();
        assert_eq!(dev.specialty.as_deref(), Some("Database"));
        let dev = dev.id.clone();
        // One of the owner's own specialties.
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_specialty",
            serde_json::json!({ "input": {
                "roleId": developer.id, "name": "Payments", "title": "Payments Developer",
                "job": { "duties": ["Build the checkout."], "returns": [], "limits": [],
                         "askLead": [] },
            }}),
        ));
        let payments = s
            .roles
            .iter()
            .flat_map(|r| &r.specialties)
            .find(|x| x.name == "Payments")
            .unwrap();
        assert!(!payments.built_in);
        // Learning in layers.
        let learning: plenipo_workforce::LearningSnapshot = body(invoke_json(
            &main,
            "set_role_learns",
            serde_json::json!({ "roleId": developer.id, "learns": false }),
        ));
        assert_eq!(learning.off_roles, [developer.id.as_str()]);
        let learning: plenipo_workforce::LearningSnapshot = body(invoke_json(
            &main,
            "set_agent_learning",
            serde_json::json!({ "positionId": dev, "learns": true }),
        ));
        assert_eq!(learning.agents.get(&dev), Some(&true));
        // A department's rule.
        let routing: plenipo_router::RoutingSnapshot = body(invoke_json(
            &main,
            "set_model_rule",
            serde_json::json!({
                "target": { "layer": "department", "id": dept },
                "rule": { "effort": "high" },
            }),
        ));
        let rule = routing
            .departments
            .iter()
            .find(|r| r.department_id == dept)
            .unwrap();
        assert_eq!(rule.rule.effort, Some(plenipo_runtime::agent::Effort::High));
        // Delete for good only after archiving; archive the whole department.
        let err = invoke_json(
            &main,
            "delete_for_good",
            serde_json::json!({ "kind": "department", "id": dept, "save": [] }),
        )
        .expect_err("archive first");
        assert_eq!(err["kind"], "invalidInput", "{err}");
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "archive_department",
            serde_json::json!({ "departmentId": dept }),
        ));
        assert!(s.departments[0].archived_at.is_some());
        assert!(s.positions.iter().all(|p| !p.active));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "bring_back_department",
            serde_json::json!({ "departmentId": dept }),
        ));
        assert!(s.departments[0].archived_at.is_none());
        assert!(s.positions.iter().all(|p| p.active), "all came back");
        // Its permission limit goes with it when it is deleted for good (ADR-043 §10).
        let _: serde_json::Value = body(invoke_json(
            &main,
            "assign_permissions",
            serde_json::json!({ "target": "department", "id": dept, "setId": "read-only" }),
        ));
        let _: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "archive_department",
            serde_json::json!({ "departmentId": dept }),
        ));
        let preview: plenipo_workforce::DeletionPreview = body(invoke_json(
            &main,
            "preview_delete_for_good",
            serde_json::json!({ "kind": "department", "id": dept }),
        ));
        assert_eq!(preview.name, "Development");
        assert_eq!(preview.agents.len(), 2);
        // The developer moves to the Workforce; the rest is deleted for good.
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "delete_for_good",
            serde_json::json!({ "kind": "department", "id": dept, "save": [dev] }),
        ));
        assert!(s.departments[0].deleted);
        assert!(s.positions.iter().all(|p| p.deleted || p.in_workforce));
        let guard = app.state::<plenipo_guard::Guard>();
        assert!(!guard.config().unwrap().departments.contains_key(&dept));
        // A department deleted for good cannot be given a limit again.
        let err = invoke_json(
            &main,
            "assign_permissions",
            serde_json::json!({ "target": "department", "id": dept, "setId": "read-only" }),
        )
        .expect_err("it is gone");
        assert!(
            err["message"]
                .as_str()
                .is_some_and(|m| m.contains("no longer exists")),
            "{err}"
        );
        assert_eq!(s.workforce.len(), 1);
        let saved = s.workforce[0].clone();
        assert_eq!(saved.title, "Database Developer");
        assert_eq!(saved.specialty.as_deref(), Some("Database"));
        // A short record stays in the Ledger.
        let ledger = app.state::<std::sync::Arc<plenipo_ledger::Ledger>>();
        let events: Vec<String> = ledger
            .recent_events(200)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        for want in [
            "org.department_archived",
            "org.department_restored",
            "org.department_deleted",
            "org.agent_saved",
        ] {
            assert!(events.iter().any(|e| e == want), "{want}");
        }
        // Hire it back from the Workforce into a new department (a worker needs a lead).
        let err = invoke_json(
            &main,
            "hire_from_workforce",
            serde_json::json!({ "savedId": saved.id, "reportsTo": null, "title": null }),
        )
        .expect_err("a worker reports to a lead");
        assert_eq!(err["kind"], "invalidInput", "{err}");
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "create_department",
            serde_json::json!({ "input": {
                "name": "Platform", "description": "",
                "head": { "roleId": role_id(&org, "Manager"),
                          "title": "Platform Manager", "runtimeId": "claude-code" },
            }}),
        ));
        let platform = s
            .departments
            .iter()
            .find(|d| d.name == "Platform")
            .unwrap()
            .head_position_id
            .clone();
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "hire_from_workforce",
            serde_json::json!({ "savedId": saved.id, "reportsTo": platform, "title": null }),
        ));
        assert!(s.workforce.is_empty());
        let back = s
            .positions
            .iter()
            .find(|p| p.active && p.title == "Database Developer")
            .expect("hired back");
        assert_eq!(back.specialty.as_deref(), Some("Database"));
        // Archive it, save it on its own, then delete it from the Workforce.
        let _: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "archive_position",
            serde_json::json!({ "positionId": back.id }),
        ));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "save_to_workforce",
            serde_json::json!({ "positionId": back.id }),
        ));
        assert_eq!(s.workforce.len(), 1);
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "delete_saved_agent",
            serde_json::json!({ "savedId": s.workforce[0].id }),
        ));
        assert!(s.workforce.is_empty());
        // The owner's specialty can go once no agent on the chart has it.
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "remove_specialty",
            serde_json::json!({ "specialtyId": payments.id }),
        ));
        assert!(!s
            .roles
            .iter()
            .flat_map(|r| &r.specialties)
            .any(|x| x.name == "Payments"));
    }

    // ---- Phase 18: the canvas, lending, Watch, and the owner's tile ---------------------------

    const PHASE_18: [&str; 12] = [
        "place_tiles",
        "tidy_up",
        "retarget_oversight",
        "get_live_view",
        "lend_agent",
        "send_home",
        "get_watch",
        "get_watch_change",
        "subscribe_watch",
        "unsubscribe_watch",
        "get_owner_profile",
        "set_owner_profile",
    ];

    /// Arguments that fit every Phase 18 command (each takes the ones it names).
    fn phase_18_args() -> serde_json::Value {
        serde_json::json!({
            "places": [], "oversightId": SESSION, "overseerId": SESSION, "targetId": null,
            "positionId": SESSION, "toLeadId": SESSION, "until": "objective",
            "changeId": SESSION, "channel": "__CHANNEL__:1", "subscription": 1,
            "input": { "status": "available", "mood": null, "message": "",
                       "picture": { "kind": "keep" } },
        })
    }

    #[test]
    fn the_canvas_lending_watch_and_the_owners_tile_are_the_main_windows_alone() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for cmd in PHASE_18 {
            let args = phase_18_args();
            let refused = |answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
                           from: &str| {
                let err = answer.expect_err(from);
                assert!(
                    err.to_string().contains("not allowed"),
                    "{cmd} from {from}: {err}"
                );
            };
            refused(invoke_json(&other, cmd, args.clone()), "another window");
            refused(invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                invoke_with(&main, cmd, args.clone(), "https://example.com"),
                "a web page",
            );
            if let Err(err) = invoke_json(&main, cmd, args) {
                assert!(err["kind"].is_string(), "{cmd} from the main window: {err}");
            }
        }
        // Watch's commands only read: none takes anything to write, so none can change a
        // working copy (ADR-016, one writer per working copy).
        for cmd in ["get_watch", "get_watch_change"] {
            let err = invoke_json(
                &main,
                cmd,
                serde_json::json!({ "positionId": SESSION, "changeId": SESSION,
                                    "path": "src/app.rs", "content": "x" }),
            );
            if let Ok(body) = err {
                let text = format!("{body:?}");
                assert!(!text.contains("src/app.rs"), "{cmd} took a path: {text}");
            }
        }
    }

    #[test]
    fn the_phase_18_commands_check_what_they_are_given() {
        let app = app();
        let main = window(&app, "main");
        let many: Vec<serde_json::Value> = (0..501)
            .map(|_| serde_json::json!({ "tileId": "owner", "x": 0, "y": 0 }))
            .collect();
        for (cmd, args, why) in [
            (
                "place_tiles",
                serde_json::json!({ "places": [{ "tileId": "../x", "x": 1, "y": 2 }] }),
                "invalid tile id",
            ),
            (
                "place_tiles",
                serde_json::json!({ "places": many }),
                "at most 500",
            ),
            (
                "place_tiles",
                serde_json::json!({ "places": [{ "tileId": "owner", "x": 1e9, "y": 0 }] }),
                "on the canvas",
            ),
            (
                "retarget_oversight",
                serde_json::json!({ "oversightId": SESSION, "overseerId": null, "targetId": null }),
                "name the new overseer or the new team",
            ),
            (
                "retarget_oversight",
                serde_json::json!({ "oversightId": "..", "overseerId": SESSION }),
                "invalid oversight assignment id",
            ),
            (
                "lend_agent",
                serde_json::json!({ "positionId": SESSION, "toLeadId": SESSION, "until": "forever" }),
                "unknown variant `forever`",
            ),
            (
                "lend_agent",
                serde_json::json!({ "positionId": "a/b", "toLeadId": SESSION, "until": "returned" }),
                "invalid position id",
            ),
            (
                "send_home",
                serde_json::json!({ "positionId": "" }),
                "invalid position id",
            ),
            (
                "get_watch",
                serde_json::json!({ "positionId": "C:/Windows" }),
                "invalid position id",
            ),
            (
                "get_watch_change",
                serde_json::json!({ "changeId": "../../etc/passwd" }),
                "invalid change id",
            ),
            (
                "set_owner_profile",
                serde_json::json!({ "input": { "status": "invisible", "mood": null,
                    "message": "", "picture": { "kind": "keep" } } }),
                "unknown variant `invisible`",
            ),
            (
                // No room for a path: the picture comes as a small PNG, never a file to open.
                "set_owner_profile",
                serde_json::json!({ "input": { "status": "busy", "mood": null, "message": "",
                    "picture": { "kind": "keep" }, "path": "C:/Users/me/me.png" } }),
                "unknown field `path`",
            ),
            (
                "set_owner_profile",
                serde_json::json!({ "input": { "status": "busy", "mood": "great",
                    "message": "x".repeat(81), "picture": { "kind": "keep" } } }),
                "at most 80 characters",
            ),
            (
                "set_owner_profile",
                serde_json::json!({ "input": { "status": "busy", "mood": null, "message": "",
                    "picture": { "kind": "set", "png": "bm90IGEgcGljdHVyZQ==" } } }),
                "PNG of at most 256",
            ),
        ] {
            let err = invoke_json(&main, cmd, args.clone())
                .expect_err(&format!("{cmd} must refuse {args}"));
            let said = err["message"]
                .as_str()
                .map_or_else(|| err.to_string(), str::to_owned);
            assert!(
                said.contains(why),
                "{cmd} refused {args} with {said}, not {why}"
            );
        }
    }

    const PHASE_19: [&str; 8] = [
        "get_ai_tools",
        "check_ai_tool",
        "check_ai_tool_versions",
        "get_ai_tool_usage",
        "update_ai_tool",
        "cancel_ai_tool_update",
        "set_ai_tools_auto_update",
        "set_ai_tool_payment",
    ];

    /// Arguments that fit every Phase 19 command (each takes the ones it names).
    fn phase_19_args() -> serde_json::Value {
        serde_json::json!({
            "runtimeId": "codex", "dayStarts": [0, 86_400_000], "on": false,
            "method": "subscription",
        })
    }

    #[test]
    fn the_ai_tools_page_is_the_main_windows_alone() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        let refused = |cmd: &str,
                       answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
                       from: &str| {
            let err = answer.expect_err(from);
            assert!(
                err.to_string().contains("not allowed"),
                "{cmd} from {from}: {err}"
            );
        };
        for cmd in PHASE_19 {
            let args = phase_19_args();
            refused(
                cmd,
                invoke_json(&other, cmd, args.clone()),
                "another window",
            );
            refused(cmd, invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                cmd,
                invoke_with(&main, cmd, args.clone(), "https://example.com"),
                "a web page",
            );
            if let Err(err) = invoke_json(&main, cmd, args) {
                assert!(err["kind"].is_string(), "{cmd} from the main window: {err}");
            }
        }
        // A sign-in tab opens through `open_terminal`, which stays the main window's alone.
        let place = serde_json::json!({
            "place": { "kind": "aiTool", "runtimeId": "codex", "action": "signIn" },
            "cols": 80, "rows": 24, "events": "__CHANNEL__:9",
        });
        refused(
            "open_terminal",
            invoke_json(&other, "open_terminal", place.clone()),
            "another window",
        );
        refused(
            "open_terminal",
            invoke_json(&sign, "open_terminal", place.clone()),
            "the sign",
        );
        refused(
            "open_terminal",
            invoke_with(&main, "open_terminal", place.clone(), "https://example.com"),
            "a web page",
        );
        // From the main window it is Guard and the tool's rules that answer (no AI tool is
        // installed in these tests).
        let err = invoke_json(&main, "open_terminal", place).unwrap_err();
        assert!(
            err["message"]
                .as_str()
                .is_some_and(|m| m.contains("Codex is not installed")),
            "{err}"
        );
    }

    #[test]
    fn the_phase_19_commands_check_what_they_are_given() {
        let app = app();
        let main = window(&app, "main");
        let days: Vec<u64> = (0..62).map(|d| d * 86_400_000).collect();
        for (cmd, args, why) in [
            (
                "check_ai_tool",
                serde_json::json!({ "runtimeId": "../codex" }),
                "invalid runtime id",
            ),
            (
                "check_ai_tool",
                serde_json::json!({ "runtimeId": "calc" }),
                "Plenipo has no AI tool called \"calc\"",
            ),
            (
                "get_ai_tool_usage",
                serde_json::json!({ "runtimeId": "codex", "dayStarts": [5] }),
                "1 to 60 days",
            ),
            (
                "get_ai_tool_usage",
                serde_json::json!({ "runtimeId": "codex", "dayStarts": days }),
                "at most 60 days",
            ),
            (
                "get_ai_tool_usage",
                serde_json::json!({ "runtimeId": "codex", "dayStarts": [9, 3] }),
                "follow one another",
            ),
            (
                "update_ai_tool",
                serde_json::json!({ "runtimeId": "ollama" }),
                "updates itself",
            ),
            (
                "cancel_ai_tool_update",
                serde_json::json!({ "runtimeId": "codex" }),
                "Only an update that is still waiting",
            ),
            // Extra arguments are ignored: only the tool's ID reaches the command, which finds
            // nothing to update (no AI tool is installed in these tests) and starts nothing.
            (
                "update_ai_tool",
                serde_json::json!({ "runtimeId": "codex", "command": "npm install" }),
                "Codex is not installed on this PC, so there is nothing to update",
            ),
            (
                "set_ai_tool_payment",
                serde_json::json!({ "runtimeId": "codex", "method": "paidKey" }),
                "comes with spending caps",
            ),
            (
                "set_ai_tool_payment",
                serde_json::json!({ "runtimeId": "codex", "method": "free" }),
                "unknown variant `free`",
            ),
        ] {
            let answer = invoke_json(&main, cmd, args.clone());
            let err = answer.expect_err(&format!("{cmd} must refuse {args}"));
            let said = err["message"]
                .as_str()
                .map_or_else(|| err.to_string(), str::to_owned);
            assert!(
                said.contains(why),
                "{cmd} refused {args} with {said}, not {why}"
            );
        }
        // Places that name anything but a known AI tool and one of its two actions are refused
        // before anything runs.
        for place in [
            serde_json::json!({ "kind": "aiTool", "runtimeId": "calc", "action": "signIn" }),
            serde_json::json!({ "kind": "aiTool", "runtimeId": "codex", "action": "update" }),
            serde_json::json!({ "kind": "aiTool", "runtimeId": "codex", "action": "signIn",
                                "args": ["--with-api-key"] }),
            serde_json::json!({ "kind": "aiTool", "runtimeId": "codex", "action": "signIn",
                                "program": "C:/Windows/System32/cmd.exe" }),
        ] {
            let err = invoke_json(
                &main,
                "open_terminal",
                serde_json::json!({ "place": place, "cols": 80, "rows": 24,
                                    "events": "__CHANNEL__:9" }),
            )
            .unwrap_err();
            assert!(
                err.to_string().contains("invalid args `place`"),
                "{place}: {err}"
            );
        }
        // The page itself, and the switch, from the main window.
        let page: plenipo_capabilities::ai_tools::AiToolsPage = body(invoke(&main, "get_ai_tools"));
        assert!(!page.auto_update);
        assert_eq!(
            page.tools.len(),
            plenipo_runtime::agent::builtin_adapters().len()
        );
        let page: plenipo_capabilities::ai_tools::AiToolsPage = body(invoke_json(
            &main,
            "set_ai_tools_auto_update",
            serde_json::json!({ "on": true }),
        ));
        assert!(page.auto_update);
    }

    const PHASE_20: [&str; 17] = [
        "get_connections",
        "connect_connection",
        "cancel_connection_sign_in",
        "disconnect_connection",
        "set_connection_parts",
        "set_connection_access",
        "set_connection_send_list",
        "set_connection_own_app",
        // Part 20B (ADR-070).
        "save_connection_app",
        "add_connection",
        "remove_connection",
        // Part 20C (ADR-071): keys typed into a card, and add-on tools.
        "save_connection_key",
        "add_add_on",
        "change_add_on",
        "remove_add_on",
        "check_add_on_tools",
        "set_add_on_tools",
    ];

    /// Arguments that fit every Phase 20 command (each takes the ones it names).
    fn phase_20_args() -> serde_json::Value {
        serde_json::json!({
            "connectionId": "microsoft365", "kind": "work", "parts": { "mail": "readOnly" },
            "access": [], "list": [], "app": null, "service": "slack",
            "key": { "key": "pat-na1-ipc" }, "addOnId": "nothere", "change": {}, "marks": {},
            "addOn": { "name": "Nope", "program": "npx", "args": ["-y", "x"] },
        })
    }

    #[test]
    fn settings_connections_is_the_main_windows_alone() {
        let app = app();
        let main = window(&app, "main");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        let refused = |cmd: &str,
                       answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
                       from: &str| {
            let err = answer.expect_err(from);
            assert!(
                err.to_string().contains("not allowed"),
                "{cmd} from {from}: {err}"
            );
        };
        for cmd in PHASE_20 {
            let args = phase_20_args();
            refused(
                cmd,
                invoke_json(&other, cmd, args.clone()),
                "another window",
            );
            refused(cmd, invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                cmd,
                invoke_with(&main, cmd, args.clone(), "https://example.com"),
                "a web page",
            );
            if let Err(err) = invoke_json(&main, cmd, args) {
                assert!(err["kind"].is_string(), "{cmd} from the main window: {err}");
            }
        }
        // Nothing the refused windows asked for changed anything: the main window's own
        // `add_connection` above added one Slack workspace, and no more.
        let page: plenipo_capabilities::connections::ConnectionsPage =
            body(invoke(&main, "get_connections"));
        assert_eq!(
            page.services[0].connections[0].connection.state,
            plenipo_guard::ConnectionState::NotConnected
        );
        let slack: Vec<&str> = page.services[1]
            .connections
            .iter()
            .map(|c| c.connection.id.as_str())
            .collect();
        assert_eq!(slack, ["slack", "slack-2"]);
    }

    #[test]
    fn the_phase_20_commands_check_what_they_are_given() {
        let app = app();
        let main = window(&app, "main");
        let id = "microsoft365";
        let line =
            |who: serde_json::Value| serde_json::json!([{ "who": who, "level": "readOnly" }]);
        for (cmd, args, why) in [
            (
                "disconnect_connection",
                serde_json::json!({ "connectionId": "../microsoft365" }),
                "invalid connection id",
            ),
            (
                "connect_connection",
                serde_json::json!({ "connectionId": "outlook", "kind": "work" }),
                "invalid connection id",
            ),
            (
                "connect_connection",
                serde_json::json!({ "connectionId": id, "kind": "admin" }),
                "unknown variant `admin`",
            ),
            (
                "connect_connection",
                serde_json::json!({ "connectionId": "hubspot", "kind": "work" }),
                "HubSpot connects with a key",
            ),
            (
                "connect_connection",
                serde_json::json!({ "connectionId": "google", "kind": "work" }),
                "no app ID for Google",
            ),
            (
                "cancel_connection_sign_in",
                serde_json::json!({ "connectionId": id }),
                "No sign-in to Microsoft 365 is waiting",
            ),
            (
                "set_connection_parts",
                serde_json::json!({ "connectionId": id, "parts": { "mail": "everything" } }),
                "unknown variant `everything`",
            ),
            (
                "set_connection_parts",
                serde_json::json!({ "connectionId": id, "parts": { "inbox": "readOnly" } }),
                "unknown variant `inbox`",
            ),
            (
                "set_connection_access",
                serde_json::json!({ "connectionId": id,
                    "access": line(serde_json::json!({ "kind": "role", "id": "../x" })) }),
                "invalid role id",
            ),
            (
                "set_connection_access",
                serde_json::json!({ "connectionId": id,
                    "access": line(serde_json::json!({ "kind": "everyone", "id": "x" })) }),
                "a line is for a role or an agent",
            ),
            (
                "set_connection_access",
                serde_json::json!({ "connectionId": id,
                    "access": line(serde_json::json!({ "kind": "role", "id": "x", "all": true })) }),
                "unknown field `all`",
            ),
            (
                "set_connection_access",
                serde_json::json!({ "connectionId": id,
                    "access": line(serde_json::json!({ "kind": "role",
                        "id": "0f8fad5b-d9cb-469f-a165-70867728950e" })) }),
                "no longer exists",
            ),
            (
                "set_connection_access",
                serde_json::json!({ "connectionId": id, "access": (0..201)
                    .map(|n| serde_json::json!({ "who": { "kind": "role", "id": format!("r{n}") },
                        "level": "readOnly" }))
                    .collect::<Vec<_>>() }),
                "at most 200 lines",
            ),
            (
                "set_connection_send_list",
                serde_json::json!({ "connectionId": id, "list": vec!["a@b.co"; 201] }),
                "at most 200 entries",
            ),
            (
                "set_connection_send_list",
                serde_json::json!({ "connectionId": id, "list": ["everyone"] }),
                "is not an email address or an @domain",
            ),
            (
                "set_connection_own_app",
                serde_json::json!({ "connectionId": id,
                    "app": { "appId": "not-an-id", "tenant": "contoso.com" } }),
                "the app ID must look like",
            ),
            // No secret goes through Microsoft's card: an app's secret is refused by name.
            (
                "set_connection_own_app",
                serde_json::json!({ "connectionId": id, "app": {
                    "appId": "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0", "tenant": "contoso.com",
                    "clientSecret": "abc~123" } }),
                "unknown field `clientSecret`",
            ),
            (
                "set_connection_own_app",
                serde_json::json!({ "connectionId": id, "app": {
                    "appId": "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0", "tenant": "contoso.com",
                    "secretKept": true } }),
                "a Microsoft app needs no secret",
            ),
            // Part 20B: the owner's own Slack or Google app, and Slack's workspaces.
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "../google",
                    "app": { "clientId": "1.2" } }),
                "invalid connection id",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "google", "app": {
                    "clientId": "123456789012-a.apps.googleusercontent.com", "secret": "s",
                    "tenant": "x" } }),
                "unknown field `tenant`",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "google", "app": {
                    "clientId": "evil.example", "secret": "GOCSPX-abc" } }),
                "a Google app's client ID looks like",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "google", "app": {
                    "clientId": "123456789012-a.apps.googleusercontent.com" } }),
                "client secret too",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "google", "app": {
                    "clientId": "123456789012-a.apps.googleusercontent.com",
                    "secret": "x".repeat(201) } }),
                "that secret is too long",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": "slack", "app": {
                    "clientId": "1111111111.2222222222", "secret": "abc" } }),
                "A Slack app signs in with no secret",
            ),
            (
                "save_connection_app",
                serde_json::json!({ "connectionId": id, "app": { "clientId": "1.2" } }),
                "does not take a client ID here",
            ),
            (
                "add_connection",
                serde_json::json!({ "service": "myspace" }),
                "unknown variant `myspace`",
            ),
            (
                "add_connection",
                serde_json::json!({ "service": "google" }),
                "Plenipo keeps one Google account",
            ),
            (
                "add_connection",
                serde_json::json!({ "service": "stripe" }),
                "Plenipo keeps one Stripe account",
            ),
            (
                "remove_connection",
                serde_json::json!({ "connectionId": id }),
                "has one card",
            ),
            (
                "remove_connection",
                serde_json::json!({ "connectionId": "slack-x" }),
                "invalid connection id",
            ),
            (
                "set_connection_send_list",
                serde_json::json!({ "connectionId": "slack", "list": ["#general"] }),
                "its ID is at the bottom of About",
            ),
            (
                "set_connection_parts",
                serde_json::json!({ "connectionId": "slack", "parts": { "search": "fullAccess" } }),
                "Search only reads",
            ),
            // Part 20C: keys typed into a card, checked before anything is sent.
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "../hubspot", "key": { "key": "x" } }),
                "invalid connection id",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "stripe", "key": { "key": "sk_live_51IpcSecretKey" } }),
                "Plenipo takes only a restricted key",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "stripe", "key": { "key": "rk_test_1", "site": "https://x.com" } }),
                "Stripe does not take a site address",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "stripe", "key": { "secret": "x" } }),
                "not in the expected shape",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "stripe", "key": { "key": "r".repeat(401) } }),
                "that key is too long",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "wordpress", "key": {
                    "site": "http://shop.example.com", "user": "plenipo",
                    "password": "abcdEFGH1234ijklMNOP5678" } }),
                "only over https",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "wordpress", "key": {
                    "site": "https://shop.example.com", "user": "plenipo",
                    "password": "abcdEFGH1234ijklMNOP5678", "storeKey": "ck_1" } }),
                "or neither",
            ),
            (
                "save_connection_key",
                serde_json::json!({ "connectionId": "google", "key": { "key": "x" } }),
                "signs in in your browser",
            ),
            (
                "connect_connection",
                serde_json::json!({ "connectionId": "stripe", "kind": "work" }),
                "connects with a key",
            ),
            // Add-on tools: an installed program, never a shell or a downloader.
            (
                "add_add_on",
                serde_json::json!({ "addOn": { "name": "Notion", "program": "npx",
                    "args": ["-y", "@notionhq/notion-mcp-server"] } }),
                "downloads code each time",
            ),
            (
                "add_add_on",
                serde_json::json!({ "addOn": { "name": "Shell", "program": "powershell" } }),
                "is a shell",
            ),
            (
                "add_add_on",
                serde_json::json!({ "addOn": { "name": "Gone", "program": "/no/such/program-mcp" } }),
                "There is no program at",
            ),
            (
                "add_add_on",
                serde_json::json!({ "addOn": { "name": "x", "program": "x", "run": "y" } }),
                "unknown field `run`",
            ),
            (
                "add_add_on",
                serde_json::json!({ "addOn": { "name": "x", "program": "x",
                    "args": vec!["a"; 31] } }),
                "at most 30 arguments",
            ),
            (
                "change_add_on",
                serde_json::json!({ "addOnId": "../x", "change": {} }),
                "invalid add-on id",
            ),
            (
                "change_add_on",
                serde_json::json!({ "addOnId": "x", "change": { "on": true, "everything": 1 } }),
                "unknown field `everything`",
            ),
            (
                "change_add_on",
                serde_json::json!({ "addOnId": "x", "change": { "on": true } }),
                "no longer in the list",
            ),
            (
                "set_add_on_tools",
                serde_json::json!({ "addOnId": "x", "marks": { "lookup": "always" } }),
                "unknown variant `always`",
            ),
            (
                "remove_add_on",
                serde_json::json!({ "addOnId": "Notion" }),
                "invalid add-on id",
            ),
            (
                "check_add_on_tools",
                serde_json::json!({ "addOnId": "x" }),
                "no longer in the list",
            ),
        ] {
            let answer = invoke_json(&main, cmd, args.clone());
            let err = answer.expect_err(&format!("{cmd} must refuse {args}"));
            let said = err["message"]
                .as_str()
                .map_or_else(|| err.to_string(), str::to_owned);
            assert!(
                said.contains(why),
                "{cmd} refused {args} with {said}, not {why}"
            );
        }
        // From the main window: the page, a part's level, the list, and Disconnect (always
        // allowed). Connecting needs this copy's app ID.
        // A key refused before it is sent never comes back out in the answer.
        let refused = "sk_live_51IpcSecretKey";
        let err = invoke_json(
            &main,
            "save_connection_key",
            serde_json::json!({ "connectionId": "stripe", "key": { "key": refused } }),
        )
        .unwrap_err();
        assert!(!err.to_string().contains(refused), "{err}");
        // Nor a key sent in the wrong shape (as text, not in its box's field).
        let err = invoke_json(
            &main,
            "save_connection_key",
            serde_json::json!({ "connectionId": "stripe", "key": refused }),
        )
        .unwrap_err();
        assert!(!err.to_string().contains(refused), "{err}");
        // An installed program is added off, with no tools and nobody allowed, and removed.
        let program = std::env::current_exe().unwrap().display().to_string();
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "add_add_on",
            serde_json::json!({ "addOn": { "name": "Tickets", "program": program, "args": ["--stdio"] } }),
        ));
        let added = &page.add_ons[0];
        assert!(!added.on && added.tools.is_empty() && added.access.is_empty());
        assert_eq!(added.id, "tickets");
        let err = invoke_json(
            &main,
            "set_add_on_tools",
            serde_json::json!({ "addOnId": "tickets", "marks": { "lookup": "reading" } }),
        )
        .unwrap_err();
        assert!(err.to_string().contains("no tool named"), "{err}");
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "remove_add_on",
            serde_json::json!({ "addOnId": "tickets" }),
        ));
        assert!(page.add_ons.is_empty());
        let labels: Vec<&str> = page.services.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Microsoft 365",
                "Slack",
                "Google",
                "HubSpot",
                "Stripe",
                "WordPress and WooCommerce"
            ]
        );
        let has_app = guard_host::connections_config().microsoft_app_id.is_some();
        assert_eq!(page.services[0].connections[0].has_app, has_app);
        // Google's secret goes to the Vault and never comes back out; a Slack channel goes on
        // Slack's list by its ID; another Slack workspace gets its own card.
        let secret = "GOCSPX-ipc-test-7f3e5b1c";
        let answer = invoke_json(
            &main,
            "save_connection_app",
            serde_json::json!({ "connectionId": "google", "app": {
                "clientId": "123456789012-plenipotest.apps.googleusercontent.com",
                "secret": secret } }),
        )
        .unwrap();
        let tauri::ipc::InvokeResponseBody::Json(text) = &answer else {
            panic!("a JSON answer");
        };
        assert!(!text.contains(secret), "the secret came back out");
        let page: plenipo_capabilities::connections::ConnectionsPage = body(Ok(answer));
        let google = &page.services[2].connections[0];
        assert!(google.has_app);
        assert!(google.connection.own_app.as_ref().unwrap().secret_kept);
        let page: plenipo_capabilities::connections::ConnectionsPage =
            body(invoke(&main, "get_connections"));
        assert!(!serde_json::to_string(&page).unwrap().contains(secret));
        let _: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "save_connection_app",
            serde_json::json!({ "connectionId": "google", "app": null }),
        ));
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "set_connection_send_list",
            serde_json::json!({ "connectionId": "slack", "list": ["C0100000001", "@clientco.com"] }),
        ));
        assert_eq!(
            page.services[1].connections[0].connection.send_list,
            ["C0100000001", "@clientco.com"]
        );
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "add_connection",
            serde_json::json!({ "service": "slack" }),
        ));
        assert!(page.services[1].many);
        assert_eq!(page.services[1].connections.len(), 2);
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "remove_connection",
            serde_json::json!({ "connectionId": "slack-2" }),
        ));
        assert_eq!(page.services[1].connections.len(), 1);
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "set_connection_parts",
            serde_json::json!({ "connectionId": id, "parts": { "teams": "fullAccess" } }),
        ));
        assert_eq!(
            page.services[0].connections[0]
                .connection
                .part(plenipo_guard::Part::Teams),
            plenipo_guard::PartLevel::FullAccess
        );
        let page: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "set_connection_send_list",
            serde_json::json!({ "connectionId": id, "list": ["@ClientCo.com"] }),
        ));
        assert_eq!(
            page.services[0].connections[0].connection.send_list,
            ["@clientco.com"]
        );
        let _: plenipo_capabilities::connections::ConnectionsPage = body(invoke_json(
            &main,
            "disconnect_connection",
            serde_json::json!({ "connectionId": id }),
        ));
        if !has_app {
            let err = invoke_json(
                &main,
                "connect_connection",
                serde_json::json!({ "connectionId": id, "kind": "work" }),
            )
            .unwrap_err();
            assert!(
                err["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("no app ID for Microsoft 365")),
                "{err}"
            );
        }
    }

    #[test]
    fn places_lending_and_the_owners_tile_through_ipc() {
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
                "name": "Development", "description": "",
                "head": lead("Manager", "Development Manager"),
            }}),
        ));
        let dept = s.departments[0].id.clone();
        let project = |name: &str| {
            let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
                &main,
                "create_project",
                serde_json::json!({ "input": {
                    "name": name, "description": "",
                    "allowedRuntimes": ["claude-code"],
                    "departmentId": dept,
                    "coordinator": lead("Supervisor", &format!("{name} Supervisor")),
                }}),
            ));
            s.positions
                .iter()
                .find(|p| p.title == format!("{name} Supervisor"))
                .unwrap()
                .id
                .clone()
        };
        let website = project("Website");
        let shop = project("Shop");
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "hire_position",
            serde_json::json!({ "input": {
                "roleId": role_id(&org, "Security Auditor"), "title": "Security Auditor",
                "reportsTo": website, "runtimeId": "claude-code",
            }}),
        ));
        let auditor = s
            .positions
            .iter()
            .find(|p| p.title == "Security Auditor")
            .unwrap()
            .id
            .clone();

        // Tile places: saved, in the organization, and forgotten by Tidy up (returned for Undo).
        let ledger = app.state::<Arc<plenipo_ledger::Ledger>>().inner().clone();
        let events_before = ledger.recent_events(1000).unwrap().len();
        let _: () = body(invoke_json(
            &main,
            "place_tiles",
            serde_json::json!({ "places": [
                { "tileId": "owner", "x": -20.5, "y": 10 },
                { "tileId": auditor, "x": 900, "y": 300 },
                // No longer on the canvas (Undo of Tidy up can bring one back): skipped.
                { "tileId": SESSION, "x": 0, "y": 0 },
            ]}),
        ));
        let s: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        assert_eq!(s.places.len(), 2);
        assert!(s.places.iter().all(|p| p.tile_id != SESSION));
        assert_eq!(
            ledger.recent_events(1000).unwrap().len(),
            events_before,
            "placing tiles is not in the Activity trail"
        );
        let forgotten: Vec<plenipo_ledger::TilePlace> = body(invoke(&main, "tidy_up"));
        assert_eq!(forgotten.len(), 2);
        let s: plenipo_workforce::OrgSnapshot = body(invoke(&main, "get_organization"));
        assert!(s.places.is_empty());

        // Lend the auditor to the Shop team for one objective, then send it home.
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "lend_agent",
            serde_json::json!({ "positionId": auditor, "toLeadId": shop, "until": "objective" }),
        ));
        let lent = s.positions.iter().find(|p| p.id == auditor).unwrap();
        let loan = lent.loan.as_ref().unwrap();
        assert_eq!(loan.to, "Shop Supervisor");
        assert_eq!(loan.project.as_deref(), Some("Shop"));
        let err = invoke_json(
            &main,
            "archive_position",
            serde_json::json!({ "positionId": auditor }),
        )
        .unwrap_err();
        assert!(err["message"]
            .as_str()
            .unwrap()
            .contains("send it home first"));
        let s: plenipo_workforce::OrgSnapshot = body(invoke_json(
            &main,
            "send_home",
            serde_json::json!({ "positionId": auditor }),
        ));
        assert!(s
            .positions
            .iter()
            .find(|p| p.id == auditor)
            .unwrap()
            .loan
            .is_none());
        let types: Vec<String> = ledger
            .recent_events(50)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert!(types.contains(&"org.agent_lent".to_owned()));
        assert!(types.contains(&"org.agent_returned".to_owned()));

        // The owner's tile.
        let set: plenipo_workforce::OwnerProfile = body(invoke_json(
            &main,
            "set_owner_profile",
            serde_json::json!({ "input": { "status": "doNotDisturb", "mood": "focused",
                "message": "Deep work until 3", "picture": { "kind": "keep" } } }),
        ));
        assert_eq!(set.status, plenipo_workforce::OwnerStatus::DoNotDisturb);
        let got: plenipo_workforce::OwnerProfile = body(invoke(&main, "get_owner_profile"));
        assert_eq!(got, set);

        // Watch and the live view answer, with nothing to show yet.
        let view: plenipo_capabilities::watch::WatchView = body(invoke_json(
            &main,
            "get_watch",
            serde_json::json!({ "positionId": auditor }),
        ));
        assert!(view.changes.is_empty());
        let change: Option<plenipo_capabilities::watch::WatchFileView> = body(invoke_json(
            &main,
            "get_watch_change",
            serde_json::json!({ "changeId": SESSION }),
        ));
        assert!(change.is_none());
        let live: plenipo_capabilities::LiveView = body(invoke(&main, "get_live_view"));
        assert!(live.workers.is_empty());
    }

    /// Watch's updates reach only the channels the main window opened with `subscribe_watch`
    /// (the sign window and web pages are refused it); they are never an event, which a page
    /// listening to every event would hear.
    #[test]
    fn watch_updates_go_to_the_main_window_only() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tauri::Listener as _;
        let app = app();
        let main = window(&app, "main");
        let sign = window(&app, crate::indicator::LABEL);
        let subscribers = app.state::<guard_host::WatchSubscribers>().inner().clone();
        let channel = serde_json::json!({ "channel": "__CHANNEL__:7" });
        assert!(invoke_json(&sign, "subscribe_watch", channel.clone()).is_err());
        assert!(subscribers.is_empty());
        let opened: u32 = body(invoke_json(&main, "subscribe_watch", channel));
        assert_eq!(subscribers.len(), 1);
        let _: () = body(invoke_json(
            &main,
            "unsubscribe_watch",
            serde_json::json!({ "subscription": opened }),
        ));
        assert!(subscribers.is_empty());

        // What a channel hears; and no event, even to a listener for every window.
        let heard = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&heard);
        let mine = subscribers.add(tauri::ipc::Channel::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));
        let events = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&events);
        app.listen_any("plenipo://watch", move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let broker = app.state::<plenipo_capabilities::Broker>().inner().clone();
        let who = plenipo_capabilities::watch::Who {
            task_id: SESSION.into(),
            session_id: SESSION.into(),
            position_id: None,
            worker: "Senior Developer".into(),
            objective_task_id: SESSION.into(),
            root: None,
        };
        let write = |call: &str| {
            broker
                .watch()
                .writing(&who, call, "src/app.rs", "fn main() {}\n");
        };
        write("call-1");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while heard.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(heard.load(Ordering::SeqCst), 1, "the channel hears it");
        assert_eq!(events.load(Ordering::SeqCst), 0, "no event carries it");
        // Once it stops, it hears nothing more; a channel that fails is forgotten.
        assert!(subscribers.remove(mine));
        subscribers.add(tauri::ipc::Channel::new(|_| {
            Err(tauri::Error::FailedToReceiveMessage)
        }));
        write("call-2");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(heard.load(Ordering::SeqCst), 1);
        assert!(subscribers.is_empty());
        // A page that reloads without saying goodbye leaves its channel: the oldest go first.
        for _ in 0..guard_host::MAX_WATCH_SUBSCRIBERS + 5 {
            subscribers.add(tauri::ipc::Channel::new(|_| Ok(())));
        }
        assert_eq!(subscribers.len(), guard_host::MAX_WATCH_SUBSCRIBERS);
    }

    // ---- Phase 21: the workspace (ADR-092, ADR-093) --------------------------------------------

    const PHASE_21: [&str; 11] = [
        "prepare_pop_out",
        "focus_pop_out",
        "close_pop_out",
        "reset_pop_outs",
        "get_file_roots",
        "list_folder",
        "read_file",
        "save_file",
        "open_file_outside",
        "show_in_folder",
        "get_changing_files",
    ];

    /// Arguments that fit every Phase 21 command (each takes the ones it names).
    fn phase_21_args() -> serde_json::Value {
        serde_json::json!({
            "panel": "terminal", "place": null, "root": "project:nope", "path": "README.md",
            "text": "x", "bom": false, "lineEnding": "lf", "base": null,
        })
    }

    fn refused(
        cmd: &str,
        answer: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>,
        from: &str,
    ) {
        let err = answer.expect_err(&format!("{cmd} from {from}"));
        assert!(
            err.to_string().contains("not allowed"),
            "{cmd} from {from}: {err}"
        );
    }

    #[test]
    fn the_workspace_commands_are_an_organization_windows_alone() {
        let app = app();
        let main = window(&app, "main");
        let popout = window(&app, "popout-terminal--main--1");
        let other = window(&app, "untrusted");
        let sign = window(&app, crate::indicator::LABEL);
        for cmd in PHASE_21 {
            let args = phase_21_args();
            refused(cmd, invoke_json(&popout, cmd, args.clone()), "a pop-out");
            refused(
                cmd,
                invoke_json(&other, cmd, args.clone()),
                "another window",
            );
            refused(cmd, invoke_json(&sign, cmd, args.clone()), "the sign");
            refused(
                cmd,
                invoke_with(&main, cmd, args.clone(), "https://example.com"),
                "a web page",
            );
            if let Err(err) = invoke_json(&main, cmd, args) {
                assert!(err["kind"].is_string(), "{cmd} from the main window: {err}");
            }
        }
    }

    // ---- More than one organization (Phase 21, ADR-094) -------------------------------------

    /// Make organization `name` from the first window, and give it a window of its own.
    fn second_organization(
        app: &App<MockRuntime>,
        main: &WebviewWindow<MockRuntime>,
        name: &str,
    ) -> (String, WebviewWindow<MockRuntime>) {
        let made: plenipo_core::OrgSummary = body(invoke_json(
            main,
            "create_organization",
            serde_json::json!({ "name": name, "start": { "kind": "scratch" } }),
        ));
        assert!(!made.first && !made.archived);
        let label = orgs::window_label(&made.id);
        app.state::<Arc<orgs::Orgs>>().bind(&label, &made.id);
        (made.id, window(app, &label))
    }

    #[test]
    fn two_organizations_in_two_windows_never_cross() {
        let app = app();
        let main = window(&app, "main");
        let (id, client) = second_organization(&app, &main, "Client Co");
        // Each window shows its own organization.
        let names = |w: &WebviewWindow<MockRuntime>| -> String {
            body::<plenipo_workforce::OrgSnapshot>(invoke(w, "get_organization")).name
        };
        assert_eq!(names(&client), "Client Co");
        assert_ne!(names(&main), "Client Co");
        let listing: plenipo_core::OrgListing = body(invoke(&client, "get_organizations"));
        assert_eq!(listing.current, id);
        assert_eq!(listing.organizations.len(), 2);
        assert!(listing
            .organizations
            .iter()
            .any(|o| o.id == id && o.here && o.in_window));

        // Work: a task in the first organization is not the client's.
        let task: plenipo_ledger::Task = body(invoke(&main, "create_synthetic_task"));
        let mine: Vec<plenipo_ledger::Task> = body(invoke(&main, "list_tasks"));
        let theirs: Vec<plenipo_ledger::Task> = body(invoke(&client, "list_tasks"));
        assert!(mine.iter().any(|t| t.id == task.id));
        assert!(theirs.iter().all(|t| t.id != task.id));
        let timeline = invoke_json(
            &client,
            "get_task_timeline",
            serde_json::json!({ "taskId": task.id }),
        );
        assert!(
            timeline.is_err(),
            "the client's window cannot read the first's task"
        );

        // Secrets: saved in the client's window, only the client's Vault and settings have it.
        let s = perms(invoke_json(
            &client,
            "save_secret",
            serde_json::json!({ "input": {
                "name": "Client token", "envVar": "CLIENT_TOKEN", "programs": ["gh"],
                "value": "client_secret_value_123",
            }}),
        ));
        assert_eq!(s.settings.secrets.len(), 1);
        let first = perms(invoke(&main, "get_permissions"));
        assert!(first
            .settings
            .secrets
            .iter()
            .all(|x| x.name != "Client token"));
        assert!(first.vault.stored.is_empty());

        // Approvals: each organization's list is its own (the client's has none).
        let a: serde_json::Value = body(invoke(&client, "get_approvals"));
        assert_eq!(a["pending"].as_array().map_or(0, Vec::len), 0);

        // A pop-out of the client's window calls nothing; the sign, and a web page, reach no
        // organization's commands.
        let popout = window(&app, &format!("popout-terminal--{}--1", client.label()));
        let sign = window(&app, crate::indicator::LABEL);
        for w in [&popout, &sign] {
            for cmd in ["get_organizations", "list_tasks", "get_permissions"] {
                assert!(
                    invoke(w, cmd).is_err(),
                    "{} must not reach {cmd}",
                    w.label()
                );
            }
        }
        assert!(invoke_from(&client, "list_tasks", "https://example.com").is_err());
        // The organization commands are organizations' windows' alone.
        for cmd in [
            "get_organizations",
            "create_organization",
            "switch_organization",
            "open_organization_window",
            "archive_organization",
            "bring_back_organization",
            "preview_delete_organization",
            "delete_organization_for_good",
        ] {
            let args = serde_json::json!({ "id": id, "name": "X", "start": { "kind": "scratch" }, "save": [] });
            for (w, from) in [(&popout, "a pop-out"), (&sign, "the sign")] {
                refused(cmd, invoke_json(w, cmd, args.clone()), from);
            }
            refused(
                cmd,
                invoke_with(&client, cmd, args, "https://example.com"),
                "a web page",
            );
        }
    }

    #[test]
    fn an_organization_is_archived_brought_back_and_deleted_for_good() {
        let app = app();
        let main = window(&app, "main");
        let (id, client) = second_organization(&app, &main, "Client Co");
        drop(client);
        // The first organization stays.
        let refused = invoke_json(
            &main,
            "archive_organization",
            serde_json::json!({ "id": orgs::FIRST }),
        );
        assert!(refused.is_err());
        // Deleted only from the archive.
        assert!(invoke_json(
            &main,
            "delete_organization_for_good",
            serde_json::json!({ "id": id, "save": [] }),
        )
        .is_err());
        let listing: plenipo_core::OrgListing = body(invoke_json(
            &main,
            "archive_organization",
            serde_json::json!({ "id": id }),
        ));
        assert!(listing
            .organizations
            .iter()
            .any(|o| o.id == id && o.archived && !o.in_window));
        assert!(app.state::<Arc<orgs::Orgs>>().stack(&id).is_none());
        // An archived organization is not shown in a window.
        assert!(invoke_json(
            &main,
            "switch_organization",
            serde_json::json!({ "id": id }),
        )
        .is_err());
        let listing: plenipo_core::OrgListing = body(invoke_json(
            &main,
            "bring_back_organization",
            serde_json::json!({ "id": id }),
        ));
        assert!(listing
            .organizations
            .iter()
            .any(|o| o.id == id && !o.archived));
        // Its workers: a manager hired from your Workforce with the experience it brought, and
        // two agents in its own Workforce (one whose role your first organization has, one
        // whose role it does not).
        let orgs_state = app.state::<Arc<orgs::Orgs>>();
        let stack = orgs_state.stack(&id).unwrap();
        let first = orgs_state.first().unwrap();
        let role_in = |l: &plenipo_ledger::Ledger, name: &str| {
            l.list_roles()
                .unwrap()
                .into_iter()
                .find(|r| r.name == name)
                .unwrap()
                .id
        };
        let saved =
            |id: &str, title: &str, role_id: String, tasks: u64| plenipo_ledger::SavedAgent {
                id: id.into(),
                title: title.into(),
                role_id,
                specialty_id: None,
                from_position: None,
                settings: serde_json::json!({}),
                experience: serde_json::json!({ "tasksDone": tasks, "keptLessons": 0 }),
                lessons: Vec::new(),
                saved_at: 1,
            };
        first
            .ledger
            .put_saved_agent(&saved(
                "brought",
                "Operations Manager",
                role_in(&first.ledger, "Manager"),
                2,
            ))
            .unwrap();
        let s = stack
            .workforce
            .create_department(&plenipo_workforce::DepartmentInput {
                name: "Operations".into(),
                description: String::new(),
                head: Some(plenipo_workforce::LeadInput {
                    role_id: role_in(&stack.ledger, "Manager"),
                    title: "Operations Manager".into(),
                    runtime_id: Some("claude-code".into()),
                    model: None,
                    vacant: None,
                    from_workforce: Some("brought".into()),
                }),
                reports_to: None,
                active: None,
            })
            .unwrap();
        let head = s
            .departments
            .iter()
            .find(|d| d.name == "Operations")
            .unwrap()
            .head_position_id
            .clone()
            .unwrap();
        stack
            .ledger
            .put_saved_agent(&saved(
                "kept-home",
                "Kept Manager",
                role_in(&stack.ledger, "Manager"),
                1,
            ))
            .unwrap();
        let s = stack
            .workforce
            .create_role(&plenipo_workforce::RoleInput {
                name: "Bookkeeper".into(),
                description: "Keeps the books.".into(),
                kind: plenipo_workforce::PositionKind::Worker,
                staffing: plenipo_workforce::Staffing::OnDemand,
                job: None,
            })
            .unwrap();
        let bookkeeper = s.roles.iter().find(|r| r.name == "Bookkeeper").unwrap();
        stack
            .ledger
            .put_saved_agent(&saved("kept-here", "Bookkeeper", bookkeeper.id.clone(), 1))
            .unwrap();
        // Saved once, however often it is asked (a retry after a failure).
        for _ in 0..2 {
            stack
                .workforce
                .save_copy_to(&id, &head, &first.ledger)
                .unwrap();
        }
        drop(stack);
        body::<plenipo_core::OrgListing>(invoke_json(
            &main,
            "archive_organization",
            serde_json::json!({ "id": id }),
        ));
        let preview: plenipo_core::OrgDeletePreview = body(invoke_json(
            &main,
            "preview_delete_organization",
            serde_json::json!({ "id": id }),
        ));
        assert_eq!(preview.name, "Client Co");
        // The experience it brought counts; the agent whose role your first organization has
        // moved to your Workforce; the other is named as one that goes with it.
        assert_eq!(preview.experienced.len(), 1, "{:?}", preview.experienced);
        assert_eq!(preview.experienced[0].position_id, head);
        assert_eq!(preview.experienced[0].tasks_done, 2);
        assert!(first.ledger.saved_agent("kept-home").unwrap().is_some());
        assert_eq!(
            preview
                .not_saved
                .iter()
                .map(|w| w.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Bookkeeper"]
        );
        let listing: plenipo_core::OrgListing = body(invoke_json(
            &main,
            "delete_organization_for_good",
            serde_json::json!({ "id": id, "save": [head, head] }),
        ));
        assert!(listing.organizations.iter().all(|o| o.id != id));
        let managers = first
            .ledger
            .saved_agents()
            .unwrap()
            .into_iter()
            .filter(|s| s.title == "Operations Manager")
            .count();
        assert_eq!(managers, 1, "saved once");
        // Recorded in the first organization's Ledger.
        let events: Vec<plenipo_ledger::LedgerEvent> = body(invoke_json(
            &main,
            "list_recent_events",
            serde_json::json!({ "limit": 50 }),
        ));
        assert!(events
            .iter()
            .any(|e| e.event_type == "organization.deleted"));
    }

    /// Phase 21 review: what belongs to the first organization, or to Plenipo's own window,
    /// stays there; each window's paths, terminals, and heartbeat are its own.
    #[test]
    fn what_is_the_first_organizations_or_the_main_windows_stays_there() {
        let app = app();
        let main = window(&app, "main");
        let (_, client) = second_organization(&app, &main, "Client Co");
        // Settings Plenipo could not read: shown and reset in the first organization only.
        app.state::<SettingsProblems>()
            .0
            .lock()
            .unwrap()
            .push(plenipo_core::SettingsProblem {
                key: "routing".into(),
                label: "AI model choices".into(),
                message: "They could not be read.".into(),
            });
        let mine: plenipo_core::RecoveryStatus = body(invoke(&main, "get_recovery_status"));
        assert_eq!(mine.settings_problems.len(), 1);
        let theirs: plenipo_core::RecoveryStatus = body(invoke(&client, "get_recovery_status"));
        assert!(theirs.settings_problems.is_empty());
        let refused = invoke_json(
            &client,
            "reset_settings",
            serde_json::json!({ "key": "routing" }),
        )
        .unwrap_err();
        assert!(
            refused.to_string().contains("first organization"),
            "{refused}"
        );
        assert_eq!(app.state::<SettingsProblems>().0.lock().unwrap().len(), 1);
        // Only Plenipo's own window keeps the page watch going.
        let watch = app
            .state::<Arc<window_watch::WindowWatch>>()
            .inner()
            .clone();
        invoke_json(
            &client,
            "window_alive",
            serde_json::json!({ "visible": true }),
        )
        .unwrap();
        assert_eq!(
            watch.last_alive(),
            0,
            "another window's heartbeat is not the main one's"
        );
        invoke_json(
            &main,
            "window_alive",
            serde_json::json!({ "visible": true }),
        )
        .unwrap();
        assert!(watch.last_alive() > 0);
        // Another window's AI tool sign-in terminal is not this window's to type in or close.
        let orgs = app.state::<Arc<orgs::Orgs>>().inner().clone();
        let terminal = "0f8fad5b-d9cb-469f-a165-70867728950e";
        orgs.own_ai_terminal(terminal, client.label());
        for (cmd, args) in [
            (
                "write_terminal",
                serde_json::json!({ "terminalId": terminal, "data": "x" }),
            ),
            (
                "resize_terminal",
                serde_json::json!({ "terminalId": terminal, "cols": 80, "rows": 24 }),
            ),
            (
                "close_terminal",
                serde_json::json!({ "terminalId": terminal }),
            ),
        ] {
            let refused = invoke_json(&main, cmd, args).unwrap_err();
            assert!(
                refused.to_string().contains("another window"),
                "{cmd}: {refused}"
            );
        }
        assert_eq!(
            orgs.ai_terminal_window(terminal).as_deref(),
            Some(client.label())
        );
    }

    /// Phase 21 review: one window per organization, and one change to an organization at a
    /// time.
    #[test]
    fn each_organization_shows_in_one_window_and_changes_one_at_a_time() {
        let app = app();
        let main = window(&app, "main");
        let orgs = app.state::<Arc<orgs::Orgs>>().inner().clone();
        let (b, b_window) = second_organization(&app, &main, "Client Co");
        let c: plenipo_core::OrgSummary = body(invoke_json(
            &main,
            "create_organization",
            serde_json::json!({ "name": "Shop", "start": { "kind": "scratch" } }),
        ));
        // The client's window shows the shop now; the client opens in a window of its own.
        body::<plenipo_core::OrgOpened>(invoke_json(
            &b_window,
            "switch_organization",
            serde_json::json!({ "id": c.id }),
        ));
        body::<plenipo_core::OrgOpened>(invoke_json(
            &main,
            "open_organization_window",
            serde_json::json!({ "id": b }),
        ));
        assert_eq!(
            orgs.org_of_window(b_window.label()).as_deref(),
            Some(c.id.as_str())
        );
        let b_now = orgs.window_of(&b).expect("the client has a window");
        assert_ne!(b_now, b_window.label());

        // Plenipo's window shows a lab; the first organization opens in its own window.
        // Archiving the lab brings the first organization back to Plenipo's window, and closes
        // its other one: never two windows for one organization.
        let d: plenipo_core::OrgSummary = body(invoke_json(
            &main,
            "create_organization",
            serde_json::json!({ "name": "Lab", "start": { "kind": "scratch" } }),
        ));
        body::<plenipo_core::OrgOpened>(invoke_json(
            &main,
            "switch_organization",
            serde_json::json!({ "id": d.id }),
        ));
        body::<plenipo_core::OrgOpened>(invoke_json(
            &main,
            "open_organization_window",
            serde_json::json!({ "id": orgs::FIRST }),
        ));
        let first_window = orgs.window_of(orgs::FIRST).expect("the first has a window");
        assert_ne!(first_window, "main");
        body::<plenipo_core::OrgListing>(invoke_json(
            &main,
            "archive_organization",
            serde_json::json!({ "id": d.id }),
        ));
        assert_eq!(orgs.window_of(orgs::FIRST).as_deref(), Some("main"));
        assert_eq!(orgs.org_of_window(&first_window), None);

        // Brought back by two clicks at once: its services start once, on its own Ledger.
        body::<plenipo_core::OrgListing>(invoke_json(
            &main,
            "archive_organization",
            serde_json::json!({ "id": b }),
        ));
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    let _ = invoke_json(
                        &main,
                        "bring_back_organization",
                        serde_json::json!({ "id": b }),
                    );
                });
            }
        });
        let stack = orgs.stack(&b).expect("brought back");
        let events = stack.ledger.recent_events(50).unwrap();
        let count = |kind: &str| events.iter().filter(|e| e.event_type == kind).count();
        assert_eq!(count("organization.archived"), 1, "its own Ledger");
        assert_eq!(count("organization.brought_back"), 1);
    }

    #[test]
    fn your_tile_set_in_one_organizations_window_is_told_to_every_window() {
        use tauri::Listener as _;
        let app = app();
        let main = window(&app, "main");
        let (_, client) = second_organization(&app, &main, "Client Co");
        let heard = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let into = Arc::clone(&heard);
        app.listen_any(orgs::SHARED_EVENT, move |e| {
            into.lock().unwrap().push(e.payload().to_owned());
        });
        body::<serde_json::Value>(invoke_json(
            &client,
            "set_owner_profile",
            serde_json::json!({ "input": {
                "status": "busy",
                "mood": null,
                "message": "At the client",
                "picture": { "kind": "keep" },
            } }),
        ));
        assert_eq!(*heard.lock().unwrap(), vec!["\"tile\"".to_owned()]);
        assert_eq!(
            ledger_host::shared_change("org.saved_agent_moved"),
            Some("workforce")
        );
        assert_eq!(ledger_host::shared_change("task.created"), None);
    }

    #[test]
    fn a_new_organization_starts_from_scratch_a_copy_or_not_yet_a_template() {
        let app = app();
        let main = window(&app, "main");
        for (start, why) in [
            (
                serde_json::json!({ "kind": "template", "template": "agency" }),
                "later",
            ),
            (
                serde_json::json!({ "kind": "copy", "from": "../../x" }),
                "not one",
            ),
            (serde_json::json!({ "kind": "clone" }), "unknown"),
        ] {
            let err = invoke_json(
                &main,
                "create_organization",
                serde_json::json!({ "name": "Client Co", "start": start }),
            )
            .expect_err(why);
            assert!(!err.is_null(), "{why}");
        }
        assert!(invoke_json(
            &main,
            "create_organization",
            serde_json::json!({ "name": "  ", "start": { "kind": "scratch" } }),
        )
        .is_err());
        let copy: plenipo_core::OrgSummary = body(invoke_json(
            &main,
            "create_organization",
            serde_json::json!({ "name": "Copy Co", "start": { "kind": "copy", "from": orgs::FIRST } }),
        ));
        let stack = app.state::<Arc<orgs::Orgs>>().stack(&copy.id).unwrap();
        let first = app.state::<Arc<orgs::Orgs>>().first().unwrap();
        assert_eq!(
            stack.ledger.org_records().unwrap().roles.len(),
            first.ledger.org_records().unwrap().roles.len()
        );
        assert_eq!(orgs::name_in(&stack.ledger), "Copy Co");
    }

    #[test]
    fn a_popped_out_panel_can_call_nothing_itself() {
        let app = app();
        let popout = window(&app, "popout-files--main");
        // Its organization's window draws it and makes every call: the pop-out's own permission
        // file lists no command, Plenipo's or Tauri's.
        for cmd in [
            "get_app_info",
            "get_home",
            "get_organization",
            "get_control_status",
            "stop_all_control",
            "open_terminal",
            "write_terminal",
            "close_terminal",
            "get_watch",
            "subscribe_watch",
            "cancel_agent_turn",
            "give_objective",
            "get_approvals",
            "resolve_approval",
            "save_secret",
            "plugin:event|listen",
            "plugin:window|create",
            "plugin:webview|create_webview_window",
        ] {
            refused(
                cmd,
                invoke_json(&popout, cmd, serde_json::json!({})),
                "a pop-out",
            );
        }
        // A label that only looks like an organization's window gets nothing either.
        let look_alike = window(&app, "mainly");
        refused(
            "get_app_info",
            invoke_json(&look_alike, "get_app_info", serde_json::json!({})),
            "a look-alike window",
        );
    }

    #[test]
    fn the_file_commands_check_what_they_are_given() {
        let app = app();
        let main = window(&app, "main");
        for (cmd, args, why) in [
            (
                "read_file",
                serde_json::json!({ "root": "", "path": "README.md" }),
                "not a folder Plenipo knows",
            ),
            (
                "read_file",
                serde_json::json!({ "root": "project:nope", "path": "a\u{7}b" }),
                "not a file name",
            ),
            (
                "list_folder",
                serde_json::json!({ "root": "folder:x", "path": "" }),
                "not a folder Plenipo knows",
            ),
            (
                "save_file",
                serde_json::json!({ "root": "project:nope", "path": "a.txt", "text": "x",
                    "bom": false, "lineEnding": "lf", "base": "not-a-hash" }),
                "not a file's fingerprint",
            ),
            (
                "save_file",
                serde_json::json!({ "root": "project:nope", "path": "a.txt", "text": "x",
                    "bom": false, "lineEnding": "cr", "base": null }),
                "unknown variant",
            ),
            (
                "prepare_pop_out",
                serde_json::json!({ "panel": "details", "place": null }),
                "unknown variant",
            ),
            (
                "prepare_pop_out",
                serde_json::json!({ "panel": "files",
                    "place": { "x": 0.0, "y": 0.0, "width": 0.0, "height": 10.0 } }),
                "not a place on the screen",
            ),
        ] {
            let err = invoke_json(&main, cmd, args).expect_err(cmd);
            assert!(err.to_string().contains(why), "{cmd}: {err}");
        }
    }

    #[test]
    fn a_dropped_file_reaches_an_objective_only_through_a_drop_on_the_same_window() {
        let app = app();
        let main = window(&app, "main");
        let drops = app.state::<files_commands::Drops>();
        let elsewhere = drops.add("org-other", vec![std::path::PathBuf::from("/etc/hosts")]);
        let args = |drop: &str| {
            serde_json::json!({
                "positionId": "0f8fad5b-d9cb-469f-a165-70867728950e",
                "objective": "Use the file",
                "projectId": "0f8fad5b-d9cb-469f-a165-70867728950f",
                "files": [{ "kind": "dropped", "drop": drop, "index": 0 }],
            })
        };
        // A ticket from another window, or a made-up one, names no file.
        for drop in [elsewhere.as_str(), "made-up"] {
            let err = invoke_json(&main, "give_objective", args(drop)).expect_err(drop);
            assert!(
                err.to_string()
                    .contains("Drop that file on the objective again"),
                "{err}"
            );
        }
        // Files need a project (its workers' folder).
        let mine = drops.add("main", vec![std::path::PathBuf::from("/etc/hosts")]);
        let mut no_project = args(&mine);
        no_project["projectId"] = serde_json::Value::Null;
        let err = invoke_json(&main, "give_objective", no_project).expect_err("no project");
        assert!(err.to_string().contains("objective for a project"), "{err}");
        // A page cannot name a path on the PC: only a known folder and a path inside it.
        let mut named = args(&mine);
        named["files"] = serde_json::json!([{ "kind": "file", "root": "project:nope",
            "path": "/etc/hosts" }]);
        let err = invoke_json(&main, "give_objective", named).expect_err("a named path");
        assert!(
            err.to_string().contains("not a folder Plenipo knows"),
            "{err}"
        );
    }
}
