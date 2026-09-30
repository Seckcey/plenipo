//! Typed Tauri command boundary. Inputs are validated here; DTOs come from
//! `plenipo-core` / `plenipo-runtime` so the TypeScript bindings stay in lockstep.
//!
//! There is deliberately no command that accepts an executable, arguments, environment
//! variables, or a working directory. The UI can only name a pre-approved launch profile, or
//! (Phase 3) an agent runtime ID plus an objective that Core sends on stdin. Handoffs between
//! workers (Phase 4) are requested by the workers themselves and checked by Liaison; the UI
//! can only allow them for a new session, read them, and cancel a waiting turn. The
//! organization (Phase 5) is changed only through validated Workforce operations; the Ledger
//! enforces its structure, and no command names a session, a prompt, or a path to open. Model
//! policy (Phase 6) is configuration: the UI names AI tools, model names (validated like every
//! model name), roles, and choices; it never selects a worker's AI tool directly. Permissions
//! (Phase 7) are configuration too: the UI edits permission sets, rules, and secret references,
//! answers approvals, and revokes grants by ID. It can never run a tool itself, and a secret's
//! value only goes in (to the operating system's protected storage), never back out. Browser and
//! computer control (Phase 10) is the owner's to stop, take over, and allow again; the UI edits
//! the website lists, opens Plenipo's browser for the owner, and reads kept screenshots by
//! their ID only — it can never drive the browser or the mouse and keyboard. Servers (Phase 11)
//! are configuration too: the UI edits the server list, sends a key or password once (to the
//! operating system's protected storage), reads a server's identity to pin it, and tests a
//! connection; it can never run a command on a server for a worker. The owner's own terminal
//! (Phase 12, ADR-031) is the one place the owner types freely, on this PC or on a server: it
//! names a place (this PC, or a server by its ID), never a program or a path; Plenipo picks the
//! shell, checks the server's pinned ID, and signs in without showing the sign-in. Its commands
//! are the owner's alone (the main window's), never a worker's tool.

use std::sync::Arc;

use plenipo_capabilities::broker::TerminalSink;
use plenipo_capabilities::browser::BrowserStatus;
use plenipo_capabilities::control::{ControlKind, ControlStatus};
use plenipo_capabilities::{
    ApprovalQueue, Broker, BrokerError, PermissionsSnapshot, Screenshot, ServerIdentity,
    ServerTest, ServersSnapshot, TaskRecord, TerminalEvent, TerminalInfo, TerminalPlace,
    TerminalSettings, TerminalShell,
};
use plenipo_core::{AppInfo, CommandError, LocalPath, SyntheticTaskAction};
use plenipo_guard::{
    BrowserChoice, CommandRules, Guard, GuardError, GuardOptions, PermissionSetInput, SecretInput,
    SensitiveKind, SensitiveRule, ServerInput, Switches, WebsiteRules,
};
use plenipo_ledger::{
    ActivityScope, ActivitySeries, BackupInfo, ExportInfo, IntegrityReport, Ledger, LedgerError,
    LedgerEvent, LedgerStatus, NewTask, NoticeSettings, Task, TaskState, TaskTimeline, WorkRecord,
    DB_FILE_NAME,
};
use plenipo_liaison::{Liaison, LiaisonError, LiaisonOverview, TaskHandoffs, TaskTree};
use plenipo_router::{
    ModelInput, RolePolicy, Router, RouterError, RoutingOptions, RoutingSnapshot,
};
use plenipo_runtime::agent::{
    AgentOverview, AgentRuntime, AgentRuntimeInfo, AgentSession, AgentSessionDetail,
};
use plenipo_runtime::{
    ExecutionOutput, ExecutionRecord, RuntimeError, RuntimeOverview, Supervisor,
};
use plenipo_workforce::{
    DepartmentInput, DevelopmentInput, HireInput, HomeView, LeadInput, LearningSnapshot,
    ObjectiveReport, OrgSnapshot, OversightRole, PositionPatchInput, ProjectInput, ProjectWork,
    RoleInput, RoleJob, RoleUpdate, TitleTheme, WorkView, Workforce, WorkforceError,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

use crate::orgs::Org;
use crate::runtime_host::Persistence;
use crate::smoke::SmokeTest;

/// Return identity information about the running application.
#[tauri::command]
pub fn get_app_info<R: Runtime>(app: AppHandle<R>) -> Result<AppInfo, CommandError> {
    Ok(app_info_for(&app.package_info().version.to_string()))
}

/// Called by the UI once the shell has rendered successfully.
/// Only has an effect in smoke-test mode, where it ends the process with success.
#[tauri::command]
pub fn frontend_ready<R: Runtime>(
    app: AppHandle<R>,
    smoke: State<'_, SmokeTest>,
) -> Result<(), CommandError> {
    if smoke.is_enabled() {
        crate::upkeep_commands::smoke_ready(&app, &smoke);
    }
    Ok(())
}

/// Profiles, execution history (newest first), active count, and notices.
#[tauri::command]
pub fn get_runtime_overview(
    supervisor: Org<'_, Supervisor>,
) -> Result<RuntimeOverview, CommandError> {
    Ok(supervisor.overview())
}

/// Launch an approved profile by ID.
#[tauri::command]
pub async fn start_execution(
    supervisor: Org<'_, Supervisor>,
    profile_id: String,
) -> Result<ExecutionRecord, CommandError> {
    validate_profile_id(&profile_id)?;
    supervisor
        .start(&profile_id)
        .await
        .map_err(to_command_error)
}

/// Terminate an execution's process tree and return its final record.
#[tauri::command]
pub async fn cancel_execution(
    supervisor: Org<'_, Supervisor>,
    execution_id: String,
) -> Result<ExecutionRecord, CommandError> {
    validate_execution_id(&execution_id)?;
    supervisor
        .cancel(&execution_id)
        .await
        .map_err(to_command_error)
}

/// Buffered output for an execution (used to rebuild the view after a reload).
#[tauri::command]
pub fn get_execution_output(
    supervisor: Org<'_, Supervisor>,
    execution_id: String,
) -> Result<ExecutionOutput, CommandError> {
    validate_execution_id(&execution_id)?;
    supervisor.output(&execution_id).map_err(to_command_error)
}

// ---- Ledger ------------------------------------------------------------------------------

/// Actor recorded for actions taken by the person using the app.
const OWNER: &str = "owner";

/// Run ledger work off the main thread.
pub(crate) async fn with_ledger<T: Send + 'static>(
    ledger: &Arc<Ledger>,
    f: impl FnOnce(&Ledger) -> Result<T, LedgerError> + Send + 'static,
) -> Result<T, CommandError> {
    let ledger = Arc::clone(ledger);
    tauri::async_runtime::spawn_blocking(move || f(&ledger))
        .await
        .map_err(|e| CommandError::internal(format!("ledger task failed: {e}")))?
        .map_err(ledger_error)
}

pub(crate) fn ledger_error(e: LedgerError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

pub(crate) fn validate_task_id(id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input("invalid task id"))
}

#[tauri::command]
pub async fn get_ledger_status(ledger: Org<'_, Arc<Ledger>>) -> Result<LedgerStatus, CommandError> {
    with_ledger(&ledger, Ledger::status).await
}

/// Tasks, newest first (at most 500).
#[tauri::command]
pub async fn list_tasks(ledger: Org<'_, Arc<Ledger>>) -> Result<Vec<Task>, CommandError> {
    with_ledger(&ledger, |l| l.list_tasks(500)).await
}

/// A task's complete ordered activity trail and its direct children.
#[tauri::command]
pub async fn get_task_timeline(
    ledger: Org<'_, Arc<Ledger>>,
    task_id: String,
) -> Result<TaskTimeline, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| l.task_timeline(&task_id)).await
}

/// Most recent events across the ledger, newest first (at most 200).
#[tauri::command]
pub async fn list_recent_events(
    ledger: Org<'_, Arc<Ledger>>,
) -> Result<Vec<LedgerEvent>, CommandError> {
    with_ledger(&ledger, |l| l.recent_events(200)).await
}

fn validate_scope(scope: &ActivityScope) -> Result<(), CommandError> {
    match scope {
        ActivityScope::All => Ok(()),
        ActivityScope::Department(id)
        | ActivityScope::Project(id)
        | ActivityScope::Position(id) => {
            if id.is_empty() || id.len() > 64 {
                Err(CommandError::invalid_input("invalid activity scope"))
            } else {
                Ok(())
            }
        }
    }
}

/// A department's, project's, or position's history (Phase 12: their pages): the events of
/// its team's tasks and its projects' tasks, newest first, before event `before`.
#[tauri::command]
pub async fn get_scope_events(
    ledger: Org<'_, Arc<Ledger>>,
    scope: ActivityScope,
    before: Option<u64>,
    limit: u32,
) -> Result<Vec<LedgerEvent>, CommandError> {
    validate_scope(&scope)?;
    with_ledger(&ledger, move |l| l.scope_events(&scope, before, limit)).await
}

/// A task's events with those of every task under it (Phase 12: the task page), newest first,
/// before event `before`.
#[tauri::command]
pub async fn get_task_events(
    ledger: Org<'_, Arc<Ledger>>,
    task_id: String,
    before: Option<u64>,
    limit: u32,
) -> Result<Vec<LedgerEvent>, CommandError> {
    validate_id("task", &task_id)?;
    with_ledger(&ledger, move |l| l.tree_events(&task_id, before, limit)).await
}

/// A project's page (Phase 12): its pull requests, artifacts, and recent decisions.
#[tauri::command]
pub async fn get_project_record(
    ledger: Org<'_, Arc<Ledger>>,
    project_id: String,
) -> Result<WorkRecord, CommandError> {
    validate_id("project", &project_id)?;
    with_ledger(&ledger, move |l| {
        l.work_record(plenipo_ledger::WorkOf::Project(&project_id), 100)
    })
    .await
}

/// Activity for each scope, counted into `buckets` time buckets over `[from, to)` (Phase 12A:
/// the activity strips). The Ledger checks the range, the bucket count, and each scope.
#[tauri::command]
pub async fn get_activity(
    ledger: Org<'_, Arc<Ledger>>,
    scopes: Vec<ActivityScope>,
    from: u64,
    to: u64,
    buckets: u32,
) -> Result<Vec<ActivitySeries>, CommandError> {
    scopes.iter().try_for_each(validate_scope)?;
    with_ledger(&ledger, move |l| l.activity(&scopes, from, to, buckets)).await
}

/// Diagnostics: create a synthetic task to exercise the ledger end to end.
#[tauri::command]
pub async fn create_synthetic_task(ledger: Org<'_, Arc<Ledger>>) -> Result<Task, CommandError> {
    with_ledger(&ledger, |l| {
        let n = l.list_tasks(1000)?.len() + 1;
        l.create_task(
            synthetic(None, format!("Synthetic diagnostic task #{n}")),
            OWNER,
        )
    })
    .await
}

/// Diagnostics: apply an action to a synthetic task. Only tasks created as synthetic can be
/// changed this way; the ledger's state machine still decides what is allowed.
#[tauri::command]
pub async fn advance_synthetic_task(
    ledger: Org<'_, Arc<Ledger>>,
    task_id: String,
    action: SyntheticTaskAction,
) -> Result<Task, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| {
        let task = l
            .task(&task_id)?
            .ok_or_else(|| LedgerError::NotFound(format!("task {task_id}")))?;
        if task.metadata.get("synthetic") != Some(&serde_json::Value::Bool(true)) {
            return Err(LedgerError::InvalidInput(
                "only synthetic diagnostic tasks can be changed from Diagnostics".into(),
            ));
        }
        let to = match action {
            SyntheticTaskAction::AddChild => {
                let n = l.child_tasks(&task_id)?.len() + 1;
                return l.create_task(
                    synthetic(
                        Some(task_id.clone()),
                        format!("{} — step {n}", task.objective),
                    ),
                    OWNER,
                );
            }
            SyntheticTaskAction::Start | SyntheticTaskAction::Resume => TaskState::Running,
            SyntheticTaskAction::Block => TaskState::Blocked,
            SyntheticTaskAction::AwaitApproval => TaskState::AwaitingApproval,
            SyntheticTaskAction::Complete => TaskState::Succeeded,
            SyntheticTaskAction::Fail => TaskState::Failed,
            SyntheticTaskAction::Cancel => TaskState::Cancelled,
        };
        l.transition_task(&task_id, to, OWNER, Some("diagnostics"))
    })
    .await
}

fn synthetic(parent: Option<String>, objective: String) -> NewTask {
    NewTask {
        parent_task_id: parent,
        requested_by: OWNER.into(),
        objective,
        acceptance_criteria: "Diagnostic only: exercises the ledger.".into(),
        priority: 3,
        metadata: serde_json::json!({ "synthetic": true }),
        ..NewTask::default()
    }
}

/// Full integrity check (can take a moment on large ledgers).
#[tauri::command]
pub async fn run_integrity_check(
    ledger: Org<'_, Arc<Ledger>>,
) -> Result<IntegrityReport, CommandError> {
    with_ledger(&ledger, Ledger::integrity_check).await
}

/// Verified backup into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn create_ledger_backup(
    ledger: Org<'_, Arc<Ledger>>,
) -> Result<BackupInfo, CommandError> {
    with_ledger(&ledger, |l| {
        let info = l.backup(None)?;
        crate::backup_host::record(l, plenipo_ledger::BackupKind::Manual, &info, OWNER);
        Ok(info)
    })
    .await
}

/// JSON export into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn export_ledger(ledger: Org<'_, Arc<Ledger>>) -> Result<ExportInfo, CommandError> {
    with_ledger(&ledger, |l| l.export_json(None)).await
}

// ---- Agent runtimes (Phase 3) ------------------------------------------------------------

/// Longest objective accepted at the boundary (bytes); the runtime enforces 10,000 characters.
const MAX_OBJECTIVE_BYTES: usize = 40_000;

pub(crate) fn validate_runtime_id(id: &str) -> Result<(), CommandError> {
    let ok = (1..=32).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CommandError::invalid_input("invalid runtime id"))
    }
}

fn validate_session_id(id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input("invalid session id"))
}

fn validate_objective(objective: &str) -> Result<(), CommandError> {
    if objective.len() > MAX_OBJECTIVE_BYTES {
        return Err(CommandError::invalid_input("the objective is too long"));
    }
    Ok(())
}

/// Runtimes (installation, sign-in, capabilities), sessions (newest first), and notices.
#[tauri::command]
pub async fn get_agent_overview(
    agents: Org<'_, AgentRuntime>,
) -> Result<AgentOverview, CommandError> {
    agents.overview().await.map_err(to_command_error)
}

/// Re-detect every runtime's installation and sign-in.
#[tauri::command]
pub async fn refresh_agent_runtimes(
    agents: Org<'_, AgentRuntime>,
) -> Result<Vec<AgentRuntimeInfo>, CommandError> {
    Ok(agents.refresh().await)
}

/// A session with its turns and recent live activity.
#[tauri::command]
pub async fn get_agent_session(
    agents: Org<'_, AgentRuntime>,
    session_id: String,
) -> Result<AgentSessionDetail, CommandError> {
    validate_session_id(&session_id)?;
    agents.session(&session_id).await.map_err(to_command_error)
}

/// Start a new session on a runtime with a first objective (startSession + submitTask).
/// With `handoffs`, the worker may ask other workers for help through Liaison (Phase 4).
#[tauri::command]
pub async fn start_agent_session(
    liaison: Org<'_, Liaison>,
    runtime_id: String,
    objective: String,
    model: Option<String>,
    handoffs: Option<bool>,
) -> Result<AgentSessionDetail, CommandError> {
    validate_runtime_id(&runtime_id)?;
    validate_objective(&objective)?;
    liaison
        .start_session(
            &runtime_id,
            &objective,
            model.as_deref(),
            handoffs.unwrap_or(false),
        )
        .await
        .map_err(to_command_error)
}

/// Give an existing session its next objective (resumeSession + submitTask). A handoff
/// worker's session takes work only through Liaison.
#[tauri::command]
pub async fn resume_agent_session(
    liaison: Org<'_, Liaison>,
    session_id: String,
    objective: String,
) -> Result<AgentSessionDetail, CommandError> {
    validate_session_id(&session_id)?;
    validate_objective(&objective)?;
    liaison
        .resume_session(&session_id, &objective)
        .await
        .map_err(to_command_error)
}

/// Cancel the session's running turn, or end a turn waiting for handoff replies; resolves
/// once the turn is recorded. Liaison then stops the handoffs it was waiting for.
#[tauri::command]
pub async fn cancel_agent_turn(
    agents: Org<'_, AgentRuntime>,
    session_id: String,
) -> Result<AgentSessionDetail, CommandError> {
    validate_session_id(&session_id)?;
    agents
        .cancel_turn(&session_id)
        .await
        .map_err(to_command_error)
}

/// Close a session (no further turns).
#[tauri::command]
pub async fn close_agent_session(
    agents: Org<'_, AgentRuntime>,
    session_id: String,
) -> Result<AgentSession, CommandError> {
    validate_session_id(&session_id)?;
    agents
        .close_session(&session_id)
        .await
        .map_err(to_command_error)
}

// ---- Liaison (Phase 4) -------------------------------------------------------------------

/// Run Liaison queries (Ledger reads) off the main thread.
async fn with_liaison<T: Send + 'static>(
    liaison: &Liaison,
    f: impl FnOnce(&Liaison) -> Result<T, LiaisonError> + Send + 'static,
) -> Result<T, CommandError> {
    let liaison = liaison.clone();
    tauri::async_runtime::spawn_blocking(move || f(&liaison))
        .await
        .map_err(|e| CommandError::internal(format!("liaison task failed: {e}")))?
        .map_err(|e| {
            if e.is_caller_error() {
                CommandError::invalid_input(e.to_string())
            } else {
                CommandError::internal(e.to_string())
            }
        })
}

/// The handoff that created a task (if any) and the handoffs it made, with their replies.
#[tauri::command]
pub async fn get_task_handoffs(
    liaison: Org<'_, Liaison>,
    task_id: String,
) -> Result<TaskHandoffs, CommandError> {
    validate_task_id(&task_id)?;
    with_liaison(&liaison, move |l| l.task_handoffs(&task_id)).await
}

/// A task's whole delegation tree, from its root task, depth-first.
#[tauri::command]
pub async fn get_task_tree(
    liaison: Org<'_, Liaison>,
    task_id: String,
) -> Result<TaskTree, CommandError> {
    validate_task_id(&task_id)?;
    with_liaison(&liaison, move |l| l.task_tree(&task_id)).await
}

/// Liaison's protocol, limits, destinations, open handoffs, and notices.
#[tauri::command]
pub async fn get_liaison_overview(
    liaison: Org<'_, Liaison>,
) -> Result<LiaisonOverview, CommandError> {
    with_liaison(&liaison, Liaison::overview).await
}

// ---- Workforce (Phase 5) -----------------------------------------------------------------

/// Longest text field accepted at the boundary (bytes); the Ledger enforces the real limits.
const MAX_FIELD_BYTES: usize = 8_000;

pub(crate) fn workforce_error(e: WorkforceError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

/// Run Workforce work (Ledger reads and writes) off the main thread.
pub(crate) async fn with_workforce<T: Send + 'static>(
    workforce: &Workforce,
    f: impl FnOnce(&Workforce) -> Result<T, WorkforceError> + Send + 'static,
) -> Result<T, CommandError> {
    let workforce = workforce.clone();
    tauri::async_runtime::spawn_blocking(move || f(&workforce))
        .await
        .map_err(|e| CommandError::internal(format!("workforce task failed: {e}")))?
        .map_err(workforce_error)
}

/// An organization record ID (a UUID).
pub(crate) fn validate_id(what: &str, id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input(format!("invalid {what} id")))
}

pub(crate) fn validate_optional_id(what: &str, id: Option<&str>) -> Result<(), CommandError> {
    id.map_or(Ok(()), |id| validate_id(what, id))
}

pub(crate) fn bounded(what: &str, value: &str) -> Result<(), CommandError> {
    if value.len() > MAX_FIELD_BYTES {
        Err(CommandError::invalid_input(format!("{what} is too long")))
    } else {
        Ok(())
    }
}

pub(crate) fn bounded_optional(what: &str, value: Option<&str>) -> Result<(), CommandError> {
    value.map_or(Ok(()), |v| bounded(what, v))
}

pub(crate) fn validate_runtimes(ids: &[String]) -> Result<(), CommandError> {
    if ids.len() > 16 {
        return Err(CommandError::invalid_input("too many runtimes"));
    }
    ids.iter().try_for_each(|r| validate_runtime_id(r))
}

fn validate_lead(lead: &LeadInput) -> Result<(), CommandError> {
    validate_id("role", &lead.role_id)?;
    bounded("the title", &lead.title)?;
    validate_optional_id("saved agent", lead.from_workforce.as_deref())?;
    lead.runtime_id
        .as_deref()
        .map_or(Ok(()), validate_runtime_id)?;
    bounded_optional("the model", lead.model.as_deref())
}

fn validate_department(input: &DepartmentInput) -> Result<(), CommandError> {
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    validate_optional_id("position", input.reports_to.as_deref())?;
    input.head.as_ref().map_or(Ok(()), validate_lead)
}

fn validate_project(input: &ProjectInput) -> Result<(), CommandError> {
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    bounded_optional("the repository", input.repository_url.as_deref())?;
    bounded_optional("the local folder", input.local_path.as_deref())?;
    bounded_optional(
        "the capability profile",
        input.capability_profile.as_deref(),
    )?;
    validate_runtimes(&input.allowed_runtimes)?;
    validate_optional_id("department", input.department_id.as_deref())?;
    input.coordinator.as_ref().map_or(Ok(()), validate_lead)
}

/// The organization: departments, projects, positions with live status, oversight, and stats.
#[tauri::command]
pub async fn get_organization(workforce: Org<'_, Workforce>) -> Result<OrgSnapshot, CommandError> {
    with_workforce(&workforce, Workforce::snapshot).await
}

/// The work a position owns and its team's unfinished work (`positionId` omitted: the whole
/// organization's).
#[tauri::command]
pub async fn get_work(
    workforce: Org<'_, Workforce>,
    position_id: Option<String>,
) -> Result<WorkView, CommandError> {
    validate_optional_id("position", position_id.as_deref())?;
    with_workforce(&workforce, move |w| w.work(position_id.as_deref())).await
}

/// Home (Phase 12): objectives still going, those finished in the last week with their
/// answers, and what is stuck.
#[tauri::command]
pub async fn get_home(workforce: Org<'_, Workforce>) -> Result<HomeView, CommandError> {
    with_workforce(&workforce, Workforce::home).await
}

/// A task's page (Phase 12): the pull requests, artifacts, decisions, and approvals of the
/// task and every task under it.
#[tauri::command]
pub async fn get_task_record(
    broker: Org<'_, Broker>,
    task_id: String,
) -> Result<TaskRecord, CommandError> {
    validate_id("task", &task_id)?;
    with_broker(&broker, move |b| b.task_record(&task_id)).await
}

// ---- Settings → Local paths (Phase 12) --------------------------------------------------

/// Where Plenipo keeps its files on this computer: shown to you, never opened or changed from
/// the screen.
#[tauri::command]
pub async fn get_local_paths<R: Runtime>(
    app: AppHandle<R>,
    ledger: Org<'_, Arc<Ledger>>,
    persistence: State<'_, Persistence>,
) -> Result<Vec<LocalPath>, CommandError> {
    use tauri::Manager as _;
    let shown = |p: &std::path::Path| p.display().to_string();
    let temporary = || "A temporary place (nothing is kept this session)".to_owned();
    // Plenipo's folder is used unless Plenipo runs for a test. The Ledger alone can be
    // temporary: when its file could not be opened, this session's is kept in memory while
    // the other folders are still used.
    let data = app
        .path()
        .app_local_data_dir()
        .ok()
        .filter(|_| *persistence == Persistence::AppData);
    let mut paths = vec![LocalPath {
        label: "Plenipo's own files".into(),
        path: data.as_deref().map_or_else(temporary, shown),
        kept: data.is_some(),
    }];
    let (ledger_file, backups, kept) = match (ledger.path(), &data) {
        (Some(file), _) => (
            shown(file),
            ledger.backups_dir().as_deref().map(shown),
            true,
        ),
        (None, Some(data)) => {
            let folder = data.join("ledger");
            (
                format!(
                    "{} (could not be opened: this session's Ledger is temporary, see \
                     Diagnostics)",
                    shown(&folder.join(DB_FILE_NAME))
                ),
                Some(shown(&folder.join("backups"))),
                false,
            )
        }
        (None, None) => (temporary(), None, false),
    };
    paths.push(LocalPath {
        label: "Everything that happened (the Ledger)".into(),
        path: ledger_file,
        kept,
    });
    paths.push(LocalPath {
        label: "Backups and exports of the Ledger".into(),
        kept: backups.is_some(),
        path: backups.unwrap_or_else(temporary),
    });
    if let Some(data) = data {
        for (label, folder) in [
            (
                "Working copies of your projects",
                data.join("working-copies"),
            ),
            ("Screenshots workers kept", data.join("screenshots")),
            (
                "Plenipo's browser (its own profile)",
                data.join("browser-profile"),
            ),
            (
                "Workers' scratch folders",
                data.join("runtime").join("agent-workspaces"),
            ),
            ("Plenipo's log files", data.join("logs")),
            ("Diagnostics files you saved", data.join("diagnostics")),
        ] {
            paths.push(LocalPath {
                label: label.into(),
                path: shown(&folder),
                kept: true,
            });
        }
    }
    Ok(paths)
}

// ---- Notices (Phase 12) ------------------------------------------------------------------

/// Settings → Notifications: which pop-up notices you get, and when.
#[tauri::command]
pub async fn get_notice_settings(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<NoticeSettings, CommandError> {
    with_ledger(&ledger, Ledger::notice_settings).await
}

/// Keep your choices for pop-up notices.
#[tauri::command]
pub async fn set_notice_settings<R: Runtime>(
    app: AppHandle<R>,
    ledger: State<'_, Arc<Ledger>>,
    settings: NoticeSettings,
) -> Result<NoticeSettings, CommandError> {
    let saved = with_ledger(&ledger, move |l| l.set_notice_settings(&settings, OWNER)).await?;
    // Your choices for the PC: every organization follows (Phase 21, ADR-094 §6).
    crate::org_host::preferences_changed(&crate::orgs::all_stacks(&app));
    Ok(saved)
}

/// Show a notice now, to check that the system shows Plenipo's notices. Its words are
/// Plenipo's own; nothing from the page goes into it.
#[tauri::command]
pub async fn send_test_notice(
    notices: State<'_, Arc<crate::notices::Notices>>,
) -> Result<(), CommandError> {
    let notices = Arc::clone(&notices);
    tauri::async_runtime::spawn_blocking(move || notices.show_now(crate::notices::test_notice()))
        .await
        .map_err(|e| CommandError::internal(format!("the notice could not be sent: {e}")))?
        .map_err(|e| CommandError::internal(format!("The system did not show the notice: {e}")))
}

#[tauri::command]
pub async fn rename_organization(
    workforce: Org<'_, Workforce>,
    name: String,
) -> Result<OrgSnapshot, CommandError> {
    bounded("the name", &name)?;
    with_workforce(&workforce, move |w| w.rename(&name)).await
}

/// What the app calls the ranks (display only; agents keep the plain titles).
#[tauri::command]
pub async fn set_organization_titles(
    workforce: Org<'_, Workforce>,
    titles: TitleTheme,
) -> Result<OrgSnapshot, CommandError> {
    with_workforce(&workforce, move |w| w.set_titles(titles)).await
}

#[tauri::command]
pub async fn create_role(
    workforce: Org<'_, Workforce>,
    input: RoleInput,
) -> Result<OrgSnapshot, CommandError> {
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    if let Some(job) = &input.job {
        validate_job(job)?;
    }
    with_workforce(&workforce, move |w| w.create_role(&input)).await
}

/// Change a role the owner created: its name, description, and working instructions.
#[tauri::command]
pub async fn update_role(
    workforce: Org<'_, Workforce>,
    role_id: String,
    input: RoleUpdate,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("role", &role_id)?;
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    validate_job(&input.job)?;
    with_workforce(&workforce, move |w| w.update_role(&role_id, &input)).await
}

// ---- Learning (ADR-024) ---------------------------------------------------------------------

/// Learning's settings, and the lessons waiting for you and kept.
#[tauri::command]
pub async fn get_learning(workforce: Org<'_, Workforce>) -> Result<LearningSnapshot, CommandError> {
    with_workforce(&workforce, |w| w.learning()).await
}

/// Worker learning on or off (Settings → Switches).
#[tauri::command]
pub async fn set_learning(
    workforce: Org<'_, Workforce>,
    enabled: bool,
) -> Result<LearningSnapshot, CommandError> {
    with_workforce(&workforce, move |w| w.set_learning(enabled)).await
}

/// Whether a role learns on its own (its lessons kept without asking you).
#[tauri::command]
pub async fn set_role_learning(
    workforce: Org<'_, Workforce>,
    role_id: String,
    auto: bool,
) -> Result<LearningSnapshot, CommandError> {
    validate_id("role", &role_id)?;
    with_workforce(&workforce, move |w| w.set_role_learning(&role_id, auto)).await
}

/// Keep a waiting lesson (in your own words, when `text` is given) or discard it.
#[tauri::command]
pub async fn decide_lesson(
    workforce: Org<'_, Workforce>,
    lesson_id: String,
    keep: bool,
    text: Option<String>,
) -> Result<LearningSnapshot, CommandError> {
    validate_id("lesson", &lesson_id)?;
    bounded_optional("the lesson", text.as_deref())?;
    with_workforce(&workforce, move |w| {
        w.decide_lesson(&lesson_id, keep, text.as_deref())
    })
    .await
}

/// Remove a kept lesson: the role's later workers no longer get it.
#[tauri::command]
pub async fn remove_lesson(
    workforce: Org<'_, Workforce>,
    lesson_id: String,
) -> Result<LearningSnapshot, CommandError> {
    validate_id("lesson", &lesson_id)?;
    with_workforce(&workforce, move |w| w.remove_lesson(&lesson_id)).await
}

/// A role's working instructions: a few short lines in each part (the Workforce checks them
/// in detail).
pub(crate) fn validate_job(job: &RoleJob) -> Result<(), CommandError> {
    for (what, lines) in [
        ("what the role does", &job.duties),
        ("what it hands back", &job.returns),
        ("what it must not do", &job.limits),
        ("when it asks for help", &job.ask_lead),
    ] {
        if lines.len() > 64 {
            return Err(CommandError::invalid_input(format!(
                "{what} has too many lines"
            )));
        }
        lines.iter().try_for_each(|l| bounded(what, l))?;
    }
    Ok(())
}

/// Create a department with its head position.
#[tauri::command]
pub async fn create_department(
    workforce: Org<'_, Workforce>,
    input: DepartmentInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_department(&input)?;
    with_workforce(&workforce, move |w| w.create_department(&input)).await
}

#[tauri::command]
pub async fn update_department(
    workforce: Org<'_, Workforce>,
    department_id: String,
    input: DepartmentInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("department", &department_id)?;
    validate_department(&input)?;
    with_workforce(&workforce, move |w| {
        w.update_department(&department_id, &input)
    })
    .await
}

/// Create a project with its coordinator position. The local folder is recorded, never opened.
#[tauri::command]
pub async fn create_project(
    workforce: Org<'_, Workforce>,
    guard: Org<'_, Guard>,
    input: ProjectInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_project(&input)?;
    check_project_limit(&guard, input.capability_profile.as_deref()).await?;
    with_workforce(&workforce, move |w| w.create_project(&input)).await
}

/// Set up a software project from the Development template (Phase 8): the Development
/// department and its VP when missing, the project with its supervisor, and the standard team.
#[tauri::command]
pub async fn set_up_development(
    workforce: Org<'_, Workforce>,
    guard: Org<'_, Guard>,
    input: DevelopmentInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_project(&input.project)?;
    if input.project.department_id.is_some() || input.project.coordinator.is_some() {
        return Err(CommandError::invalid_input(
            "the Development template chooses the department and the supervisor",
        ));
    }
    if let Some(r) = &input.runtime_id {
        validate_runtimes(std::slice::from_ref(r))?;
    }
    check_project_limit(&guard, input.project.capability_profile.as_deref()).await?;
    with_workforce(&workforce, move |w| w.set_up_development(&input)).await
}

/// Plenipo's record of the objective a task belongs to (Phase 8): tasks, workers and AI models,
/// files, tests, branches, pull requests, findings, and approvals.
#[tauri::command]
pub async fn get_objective_report(
    workforce: Org<'_, Workforce>,
    task_id: String,
) -> Result<ObjectiveReport, CommandError> {
    validate_id("task", &task_id)?;
    with_workforce(&workforce, move |w| w.objective_report(&task_id)).await
}

/// A project's recent objectives and working copies (Phase 8: the Projects page).
#[tauri::command]
pub async fn get_project_work(
    workforce: Org<'_, Workforce>,
    project_id: String,
) -> Result<ProjectWork, CommandError> {
    validate_id("project", &project_id)?;
    with_workforce(&workforce, move |w| w.project_work(&project_id)).await
}

/// Remove a finished objective's working copy (its branch stays); returns its project's work.
#[tauri::command]
pub async fn remove_workspace(
    workforce: Org<'_, Workforce>,
    broker: Org<'_, Broker>,
    workspace_id: String,
) -> Result<ProjectWork, CommandError> {
    validate_id("working copy", &workspace_id)?;
    let removed = with_broker(&broker, move |b| b.remove_workspace(&workspace_id, OWNER)).await?;
    with_workforce(&workforce, move |w| w.project_work(&removed.project_id)).await
}

#[tauri::command]
pub async fn update_project(
    workforce: Org<'_, Workforce>,
    guard: Org<'_, Guard>,
    project_id: String,
    input: ProjectInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("project", &project_id)?;
    validate_project(&input)?;
    check_project_limit(&guard, input.capability_profile.as_deref()).await?;
    with_workforce(&workforce, move |w| w.update_project(&project_id, &input)).await
}

/// Archive a project and its whole team once none of it has unfinished work.
#[tauri::command]
pub async fn archive_project(
    workforce: Org<'_, Workforce>,
    project_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("project", &project_id)?;
    with_workforce(&workforce, move |w| w.archive_project(&project_id)).await
}

/// Hire into a team: a new position (and, for a persistent one, its agent).
#[tauri::command]
pub async fn hire_position(
    workforce: Org<'_, Workforce>,
    input: HireInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("role", &input.role_id)?;
    bounded("the title", &input.title)?;
    validate_optional_id("position", input.reports_to.as_deref())?;
    input
        .runtime_id
        .as_deref()
        .map_or(Ok(()), validate_runtime_id)?;
    bounded_optional("the model", input.model.as_deref())?;
    validate_optional_id("specialty", input.specialty_id.as_deref())?;
    with_workforce(&workforce, move |w| w.hire(&input)).await
}

/// Hire an agent into a vacant persistent position.
#[tauri::command]
pub async fn fill_position(
    workforce: Org<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.fill(&position_id)).await
}

/// Let a persistent position's agent go (the position stays, vacant).
#[tauri::command]
pub async fn vacate_position(
    workforce: Org<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.vacate(&position_id)).await
}

#[tauri::command]
pub async fn update_position(
    workforce: Org<'_, Workforce>,
    position_id: String,
    input: PositionPatchInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    bounded_optional("the title", input.title.as_deref())?;
    // An empty AI tool makes the position automatic.
    if let Some(r) = input.runtime_id.as_deref().filter(|r| !r.is_empty()) {
        validate_runtime_id(r)?;
    }
    bounded_optional("the model", input.model.as_deref())?;
    // An empty specialty clears it.
    validate_optional_id(
        "specialty",
        input.specialty_id.as_deref().filter(|s| !s.is_empty()),
    )?;
    with_workforce(&workforce, move |w| w.update_position(&position_id, &input)).await
}

/// Make a position report to another (`reportsTo` null: the owner).
#[tauri::command]
pub async fn move_position(
    workforce: Org<'_, Workforce>,
    position_id: String,
    reports_to: Option<String>,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    validate_optional_id("position", reports_to.as_deref())?;
    with_workforce(&workforce, move |w| {
        w.move_position(&position_id, reports_to.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn archive_position(
    workforce: Org<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.archive_position(&position_id)).await
}

/// Assign an on-demand position to review, QA, or security-audit a team.
#[tauri::command]
pub async fn assign_oversight(
    workforce: Org<'_, Workforce>,
    overseer_id: String,
    target_id: String,
    role: OversightRole,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &overseer_id)?;
    validate_id("position", &target_id)?;
    with_workforce(&workforce, move |w| {
        w.assign_oversight(&overseer_id, &target_id, role)
    })
    .await
}

#[tauri::command]
pub async fn end_oversight(
    workforce: Org<'_, Workforce>,
    oversight_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("oversight assignment", &oversight_id)?;
    with_workforce(&workforce, move |w| w.end_oversight(&oversight_id)).await
}

/// Give a staffed persistent position's agent an objective. Core builds its instructions and
/// chooses its session; the UI names only the position and, optionally, the project the
/// objective is about (one its team runs, Phase 8).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn give_objective<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    workforce: Org<'_, Workforce>,
    broker: Org<'_, Broker>,
    drops: State<'_, crate::files_commands::Drops>,
    position_id: String,
    objective: String,
    project_id: Option<String>,
    files: Option<Vec<plenipo_capabilities::dto::ObjectiveFile>>,
) -> Result<AgentSessionDetail, CommandError> {
    use plenipo_capabilities::dto::ObjectiveFile;
    validate_id("position", &position_id)?;
    validate_optional_id("project", project_id.as_deref())?;
    validate_objective(&objective)?;
    // The files the owner put on it (Phase 21, ADR-093 §19–§22): named or copied first, so its
    // first worker finds them.
    let files = files.unwrap_or_default();
    let staged = if files.is_empty() {
        None
    } else {
        let Some(project) = project_id.clone() else {
            return Err(CommandError::invalid_input(
                "Files can go only on an objective for a project: its workers need a folder.",
            ));
        };
        if files.len() > plenipo_capabilities::broker::attachments::MAX_FILES {
            return Err(CommandError::invalid_input(
                "An objective can have up to 20 files.",
            ));
        }
        let mut sources = Vec::with_capacity(files.len());
        for file in files {
            sources.push(match file {
                ObjectiveFile::File { root, path } => {
                    let b = broker.inner().clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        b.owner_file_path(&root, &path, false)
                    })
                    .await
                    .map_err(|e| CommandError::internal(e.to_string()))?
                    .map_err(broker_error)?
                }
                ObjectiveFile::Dropped { drop, index } => {
                    drops.path(window.label(), &drop, index)?
                }
            });
        }
        let b = broker.inner().clone();
        let staged =
            tauri::async_runtime::spawn_blocking(move || b.stage_files(&project, &sources))
                .await
                .map_err(|e| CommandError::internal(e.to_string()))?
                .map_err(broker_error)?;
        Some(staged)
    };
    let text = staged
        .as_ref()
        .map_or_else(|| objective.clone(), |s| s.objective(&objective));
    let given = workforce
        .give_objective(&position_id, &text, project_id.as_deref())
        .await
        .map_err(workforce_error);
    match (&given, &staged) {
        (Ok(detail), Some(staged)) => {
            if let Some(turn) = detail.turns.last() {
                broker.record_files(&turn.task_id, staged);
            }
        }
        (Err(_), Some(staged)) => broker.discard_files(staged),
        _ => {}
    }
    given
}

// ---- Model policy and routing (Phase 6) --------------------------------------------------------

pub(crate) fn router_error(e: RouterError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

/// Run Router work (Ledger reads and writes) off the main thread.
async fn with_router<T: Send + 'static>(
    router: &Router,
    f: impl FnOnce(&Router) -> Result<T, RouterError> + Send + 'static,
) -> Result<T, CommandError> {
    let router = router.clone();
    tauri::async_runtime::spawn_blocking(move || f(&router))
        .await
        .map_err(|e| CommandError::internal(format!("router task failed: {e}")))?
        .map_err(router_error)
}

/// Models, AI tools (with usage limits), every role's model choices with the model its next
/// worker would get, models seen in use, and options.
#[tauri::command]
pub async fn get_routing(router: Org<'_, Router>) -> Result<RoutingSnapshot, CommandError> {
    with_router(&router, Router::snapshot).await
}

/// Add a model to the registry (no `id`) or change one.
#[tauri::command]
pub async fn save_model(
    router: Org<'_, Router>,
    workforce: Org<'_, Workforce>,
    input: ModelInput,
) -> Result<RoutingSnapshot, CommandError> {
    validate_optional_id("model", input.id.as_deref())?;
    validate_runtime_id(&input.runtime_id)?;
    bounded_optional("the model name", input.name.as_deref())?;
    bounded("the label", &input.label)?;
    let saved = with_router(&router, move |r| r.save_model(&input)).await?;
    refresh_efforts(&workforce).await;
    Ok(saved)
}

/// After a change to models or rules, let open conversations take their new effort from their
/// next task (ADR-041 §7). Best effort: a problem becomes a notice on the Organization page.
pub(crate) async fn refresh_efforts(workforce: &Workforce) {
    let workforce = workforce.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || workforce.refresh_open_efforts()).await;
}

/// Remove a model the owner added; it leaves every role's list.
#[tauri::command]
pub async fn remove_model(
    router: Org<'_, Router>,
    workforce: Org<'_, Workforce>,
    model_id: String,
) -> Result<RoutingSnapshot, CommandError> {
    validate_id("model", &model_id)?;
    let saved = with_router(&router, move |r| r.remove_model(&model_id)).await?;
    refresh_efforts(&workforce).await;
    Ok(saved)
}

/// Replace a role's model policy.
#[tauri::command]
pub async fn set_role_policy(
    router: Org<'_, Router>,
    workforce: Org<'_, Workforce>,
    role_id: String,
    policy: RolePolicy,
) -> Result<RoutingSnapshot, CommandError> {
    validate_id("role", &role_id)?;
    for id in policy.models.iter().chain(policy.efforts.keys()) {
        validate_id("model", id)?;
    }
    validate_runtimes(&policy.never_companies)?;
    let saved = with_router(&router, move |r| r.set_policy(&role_id, &policy)).await?;
    refresh_efforts(&workforce).await;
    Ok(saved)
}

/// Choices for every role (what a usage limit does).
#[tauri::command]
pub async fn set_routing_options(
    router: Org<'_, Router>,
    options: RoutingOptions,
) -> Result<RoutingSnapshot, CommandError> {
    with_router(&router, move |r| r.set_options(options)).await
}

/// Try an AI tool again now, although it reported a usage limit.
#[tauri::command]
pub async fn clear_usage_limit(
    router: Org<'_, Router>,
    runtime_id: String,
) -> Result<RoutingSnapshot, CommandError> {
    validate_runtime_id(&runtime_id)?;
    with_router(&router, move |r| r.clear_limit(&runtime_id)).await
}

// ---- Permissions, approvals, and the Vault (Phase 7) ------------------------------------------

/// Longest secret value accepted at the boundary (bytes); the Vault enforces 10,000
/// characters.
const MAX_SECRET_BYTES: usize = 40_000;
/// Most entries in one list of rules.
const MAX_RULES: usize = 300;

pub(crate) fn guard_error(e: GuardError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

pub(crate) fn broker_error(e: BrokerError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

/// Run broker work (Ledger reads and writes, the operating system's secret store) off the main
/// thread, then return the Permissions page as it is afterwards.
pub(crate) async fn with_broker<T: Send + 'static>(
    broker: &Broker,
    f: impl FnOnce(&Broker) -> Result<T, BrokerError> + Send + 'static,
) -> Result<T, CommandError> {
    let broker = broker.clone();
    tauri::async_runtime::spawn_blocking(move || f(&broker))
        .await
        .map_err(|e| CommandError::internal(format!("permissions task failed: {e}")))?
        .map_err(broker_error)
}

async fn with_guard(
    broker: &Broker,
    f: impl FnOnce(&Guard) -> Result<(), GuardError> + Send + 'static,
) -> Result<PermissionsSnapshot, CommandError> {
    let broker = broker.clone();
    tauri::async_runtime::spawn_blocking(move || {
        f(broker.guard()).map_err(guard_error)?;
        broker.snapshot().map_err(broker_error)
    })
    .await
    .map_err(|e| CommandError::internal(format!("permissions task failed: {e}")))?
}

async fn check_project_limit(guard: &Guard, set_id: Option<&str>) -> Result<(), CommandError> {
    let guard = guard.clone();
    let set_id = set_id.map(str::to_owned);
    tauri::async_runtime::spawn_blocking(move || guard.check_project_limit(set_id.as_deref()))
        .await
        .map_err(|e| CommandError::internal(format!("permissions task failed: {e}")))?
        .map_err(guard_error)
}

/// A permission set's ID: a slug of lower-case letters, digits, and dashes.
fn validate_set_id(id: &str) -> Result<(), CommandError> {
    let ok = (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CommandError::invalid_input("invalid permission set id"))
    }
}

fn validate_rules(what: &str, list: &[String]) -> Result<(), CommandError> {
    if list.len() > MAX_RULES {
        return Err(CommandError::invalid_input(format!("too many {what}")));
    }
    list.iter().try_for_each(|r| bounded(what, r))
}

/// Everything Settings → Permissions shows: permission sets, who has which, the rules, the
/// Vault's secret references (never values), workers using permissions now, and recent blocks.
#[tauri::command]
pub async fn get_permissions(broker: Org<'_, Broker>) -> Result<PermissionsSnapshot, CommandError> {
    with_broker(&broker, Broker::snapshot).await
}

/// Add a permission set (no `id`) or change one.
#[tauri::command]
pub async fn save_permission_set(
    broker: Org<'_, Broker>,
    input: PermissionSetInput,
) -> Result<PermissionsSnapshot, CommandError> {
    if let Some(id) = &input.id {
        validate_set_id(id)?;
    }
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    with_guard(&broker, move |g| g.save_set(&input).map(|_| ())).await
}

/// Remove a permission set nothing uses (built-in sets stay).
#[tauri::command]
pub async fn remove_permission_set(
    broker: Org<'_, Broker>,
    set_id: String,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_set_id(&set_id)?;
    with_guard(&broker, move |g| g.remove_set(&set_id)).await
}

/// What a permission set is given to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssignTarget {
    /// The role's permissions (they grant).
    Role,
    /// A department's limit (it only narrows).
    Department,
}

/// Give a role a permission set, or limit a department to one (`setId` absent: none / no
/// limit). A project's limit is part of the project's settings.
#[tauri::command]
pub async fn assign_permissions(
    broker: Org<'_, Broker>,
    target: AssignTarget,
    id: String,
    set_id: Option<String>,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_id(
        match target {
            AssignTarget::Role => "role",
            AssignTarget::Department => "department",
        },
        &id,
    )?;
    if let Some(s) = &set_id {
        validate_set_id(s)?;
    }
    with_guard(&broker, move |g| match target {
        AssignTarget::Role => g.assign_role(&id, set_id.as_deref()),
        AssignTarget::Department => g.assign_department(&id, set_id.as_deref()),
    })
    .await
}

/// Replace the approved, always-ask, and blocked command lists.
#[tauri::command]
pub async fn set_command_rules(
    broker: Org<'_, Broker>,
    rules: CommandRules,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_rules("approved commands", &rules.approved)?;
    validate_rules("always-ask commands", &rules.ask)?;
    validate_rules("blocked commands", &rules.blocked)?;
    with_guard(&broker, move |g| g.set_commands(&rules)).await
}

/// Replace the blocked-file patterns.
#[tauri::command]
pub async fn set_blocked_files(
    broker: Org<'_, Broker>,
    patterns: Vec<String>,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_rules("file patterns", &patterns)?;
    with_guard(&broker, move |g| g.set_blocked_files(&patterns)).await
}

/// Whether a kind of sensitive action asks (the default) or is blocked. It can never be
/// allowed without asking.
#[tauri::command]
pub async fn set_sensitive_rule(
    broker: Org<'_, Broker>,
    kind: SensitiveKind,
    rule: SensitiveRule,
) -> Result<PermissionsSnapshot, CommandError> {
    with_guard(&broker, move |g| g.set_sensitive(kind, rule)).await
}

/// How long an approval waits for an answer.
#[tauri::command]
pub async fn set_guard_options(
    broker: Org<'_, Broker>,
    options: GuardOptions,
) -> Result<PermissionsSnapshot, CommandError> {
    with_guard(&broker, move |g| g.set_options(&options)).await
}

/// The owner's on/off switches (ADR-023). Switching Plenipo's browser, the screen, mouse, and
/// keyboard, or remote computers (SSH, Phase 11) off also stops any worker using it now.
#[tauri::command]
pub async fn set_switches(
    broker: Org<'_, Broker>,
    switches: Switches,
) -> Result<PermissionsSnapshot, CommandError> {
    let was = broker
        .guard()
        .config()
        .map(|c| c.switches)
        .unwrap_or_default();
    let (browser_off, desktop_off, servers_off) = (
        was.browser && !switches.browser,
        was.desktop && !switches.desktop,
        was.servers && !switches.servers,
    );
    let snapshot = with_guard(&broker, move |g| g.set_switches(&switches)).await?;
    let b = broker.inner().clone();
    if browser_off {
        b.switch_off_control(ControlKind::Browser)
            .await
            .map_err(broker_error)?;
    }
    if desktop_off {
        b.switch_off_control(ControlKind::Desktop)
            .await
            .map_err(broker_error)?;
    }
    if servers_off {
        b.switch_off_control(ControlKind::Server)
            .await
            .map_err(broker_error)?;
    }
    Ok(snapshot)
}

/// Store a secret: its value goes to the operating system's protected storage, only its
/// reference to Plenipo. The value is never returned.
#[tauri::command]
pub async fn save_secret(
    broker: Org<'_, Broker>,
    input: SecretInput,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_optional_id("secret", input.id.as_deref())?;
    bounded("the name", &input.name)?;
    bounded_optional("the variable name", input.env_var.as_deref())?;
    validate_rules("programs", &input.programs)?;
    if input
        .value
        .as_ref()
        .is_some_and(|v| v.len() > MAX_SECRET_BYTES)
    {
        return Err(CommandError::invalid_input("the secret is too long"));
    }
    with_broker(&broker, move |b| {
        b.save_secret(&input)?;
        b.snapshot()
    })
    .await
}

/// Remove a secret's value and its reference.
#[tauri::command]
pub async fn remove_secret(
    broker: Org<'_, Broker>,
    secret_id: String,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_id("secret", &secret_id)?;
    with_broker(&broker, move |b| {
        b.remove_secret(&secret_id)?;
        b.snapshot()
    })
    .await
}

/// Approvals waiting for an answer (oldest first) and recent outcomes.
#[tauri::command]
pub async fn get_approvals(broker: Org<'_, Broker>) -> Result<ApprovalQueue, CommandError> {
    with_broker(&broker, Broker::approvals).await
}

/// Approve or refuse one waiting request; returns the queue as it is afterwards.
#[tauri::command]
pub async fn resolve_approval(
    broker: Org<'_, Broker>,
    approval_id: String,
    approve: bool,
) -> Result<ApprovalQueue, CommandError> {
    validate_id("approval", &approval_id)?;
    with_broker(&broker, move |b| {
        b.resolve_approval(&approval_id, approve, OWNER)?;
        b.approvals()
    })
    .await
}

/// End a worker's permissions now: its running programs stop, its waiting requests are
/// refused, and it can use no more tools in this step.
#[tauri::command]
pub async fn revoke_grant(
    broker: Org<'_, Broker>,
    grant_id: String,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_id("permission grant", &grant_id)?;
    with_broker(&broker, move |b| {
        b.revoke(&grant_id, OWNER)?;
        b.snapshot()
    })
    .await
}

// ---- Browser and computer control (Phase 10) --------------------------------------------------

/// Who uses Plenipo's browser or the mouse and keyboard now, and whether you stopped control.
#[tauri::command]
pub fn get_control_status(broker: State<'_, Broker>) -> Result<ControlStatus, CommandError> {
    Ok(broker.control_status())
}

/// The emergency stop: all browser, desktop, and server work halts at once, and stays stopped
/// until you allow it again. The PC's: the first organization's broker keeps the one record every
/// organization shares, and the sign (which shows no organization) may press it too.
#[tauri::command]
pub async fn stop_all_control<R: Runtime>(
    app: AppHandle<R>,
    broker: State<'_, Broker>,
) -> Result<ControlStatus, CommandError> {
    let broker = broker.inner().clone();
    // Every organization's workers (Phase 21, ADR-094 §7): the PC's one record stops them all,
    // and each organization's Ledger records its own part.
    let others: Vec<Broker> = crate::orgs::all_stacks(&app)
        .iter()
        .map(|s| s.broker.clone())
        .collect();
    if others.len() <= 1 {
        return broker.stop_all_control(OWNER).await.map_err(broker_error);
    }
    let stopped = broker.control_center().stop_all();
    for b in &others {
        b.stopped_all(&stopped, OWNER).await.map_err(broker_error)?;
    }
    Ok(broker.control_status())
}

/// Take over a worker's use of the browser or the mouse and keyboard, or disconnect it from its
/// servers: that worker stops.
#[tauri::command]
pub async fn take_over_control<R: Runtime>(
    app: AppHandle<R>,
    broker: State<'_, Broker>,
    session_id: String,
) -> Result<ControlStatus, CommandError> {
    let valid = session_id.split_once(':').is_some_and(|(kind, id)| {
        matches!(kind, "browser" | "desktop" | "server") && validate_execution_id(id).is_ok()
    });
    if !valid {
        return Err(CommandError::invalid_input("invalid control session id"));
    }
    // The organization whose worker it is (Phase 21): any window may take over any worker
    // using the PC's screen, mouse, and keyboard.
    let broker = crate::orgs::all_stacks(&app)
        .iter()
        .map(|s| s.broker.clone())
        .find(|b| b.owns_control_session(&session_id))
        .unwrap_or_else(|| broker.inner().clone());
    broker
        .take_over(&session_id, "you pressed Take over in Plenipo")
        .await
        .map_err(broker_error)
}

/// Let workers use the browser and the desktop again after a stop.
#[tauri::command]
pub async fn allow_control<R: Runtime>(
    app: AppHandle<R>,
    broker: State<'_, Broker>,
) -> Result<ControlStatus, CommandError> {
    // Allowed again in every organization, each recording it (Phase 21).
    let stacks = crate::orgs::all_stacks(&app);
    if stacks.len() <= 1 {
        return with_broker(&broker, |b| b.allow_control(OWNER)).await;
    }
    for s in &stacks {
        s.broker.allow_control(OWNER).map_err(broker_error)?;
    }
    Ok(broker.control_status())
}

/// The website lists: allowed, blocked, and what other websites do.
#[tauri::command]
pub async fn set_website_rules(
    broker: Org<'_, Broker>,
    rules: WebsiteRules,
) -> Result<PermissionsSnapshot, CommandError> {
    validate_rules("allowed websites", &rules.allowed)?;
    validate_rules("blocked websites", &rules.blocked)?;
    with_guard(&broker, move |g| g.set_websites(&rules)).await
}

/// Plenipo's browser: which one, whether it runs, and its own profile folder.
#[tauri::command]
pub async fn get_browser_status(broker: Org<'_, Broker>) -> Result<BrowserStatus, CommandError> {
    Ok(broker.browser_status().await)
}

/// Which browser is Plenipo's browser (ADR-028): Automatic, Microsoft Edge, or Google Chrome.
/// An open browser stays open; the choice is used from its next start.
#[tauri::command]
pub async fn set_browser_choice(
    broker: Org<'_, Broker>,
    choice: BrowserChoice,
) -> Result<BrowserStatus, CommandError> {
    with_guard(&broker, move |g| g.set_browser_choice(choice)).await?;
    Ok(broker.browser_status().await)
}

/// Open Plenipo's browser for you (for example to sign in to a website workers will use).
#[tauri::command]
pub async fn open_browser(
    broker: Org<'_, Broker>,
    url: Option<String>,
) -> Result<BrowserStatus, CommandError> {
    bounded_optional("the address", url.as_deref())?;
    let broker = broker.inner().clone();
    broker
        .open_browser_for_owner(url.as_deref())
        .await
        .map_err(broker_error)?;
    Ok(broker.browser_status().await)
}

/// A kept screenshot, by its ID (Plenipo chooses the file; the UI never names a path).
#[tauri::command]
pub async fn get_screenshot(
    broker: Org<'_, Broker>,
    artifact_id: String,
) -> Result<Screenshot, CommandError> {
    validate_id("screenshot", &artifact_id)?;
    with_broker(&broker, move |b| b.screenshot_view(&artifact_id)).await
}

// ---- Servers (Phase 11) ------------------------------------------------------------------------

/// Everything Settings → Servers shows: the servers (never their keys or passwords), the roles
/// that may be allowed to use them, the kinds of commands, and where sign-ins are kept.
#[tauri::command]
pub async fn get_servers(broker: Org<'_, Broker>) -> Result<ServersSnapshot, CommandError> {
    with_broker(&broker, Broker::servers).await
}

/// Add a server (no `id`) or change one. A key, passphrase, or password is sent once and kept
/// only in the operating system's protected storage.
#[tauri::command]
pub async fn save_server(
    broker: Org<'_, Broker>,
    input: ServerInput,
) -> Result<ServersSnapshot, CommandError> {
    validate_optional_id("server", input.id.as_deref())?;
    bounded("the name", &input.name)?;
    bounded("the address", &input.host)?;
    bounded("the user name", &input.user)?;
    if input.roles.len() > 100 {
        return Err(CommandError::invalid_input("too many roles"));
    }
    input
        .roles
        .iter()
        .try_for_each(|r| validate_id("role", r))?;
    validate_rules("folders", &input.folders)?;
    validate_rules("forwarded ports", &input.forwards)?;
    if input.classes.len() > 16 {
        return Err(CommandError::invalid_input("too many kinds of commands"));
    }
    if let Some(k) = &input.host_key {
        bounded("the host key", &k.fingerprint)?;
        bounded("the host key type", &k.algorithm)?;
    }
    for v in [&input.key, &input.passphrase, &input.password] {
        if v.as_ref().is_some_and(|v| v.len() > MAX_SECRET_BYTES) {
            return Err(CommandError::invalid_input("the sign-in value is too long"));
        }
    }
    with_broker(&broker, move |b| b.save_server(&input)).await
}

/// Remove a server: workers connected to it are disconnected, and its sign-in is removed from
/// the operating system's protected storage.
#[tauri::command]
pub async fn remove_server(
    broker: Org<'_, Broker>,
    server_id: String,
) -> Result<ServersSnapshot, CommandError> {
    validate_id("server", &server_id)?;
    with_broker(&broker, move |b| b.remove_server(&server_id)).await
}

/// Read a server's identity (its host key), for you to check and pin. Nothing is sent to sign
/// in.
#[tauri::command]
pub async fn check_server_identity(
    broker: Org<'_, Broker>,
    host: String,
    port: u16,
) -> Result<ServerIdentity, CommandError> {
    bounded("the address", &host)?;
    let broker = broker.inner().clone();
    broker
        .server_identity(&host, port)
        .await
        .map_err(broker_error)
}

/// Test a server's connection: connect with its pinned identity, sign in, and leave.
#[tauri::command]
pub async fn test_server(
    broker: Org<'_, Broker>,
    server_id: String,
) -> Result<ServerTest, CommandError> {
    validate_id("server", &server_id)?;
    let broker = broker.inner().clone();
    broker.test_server(&server_id).await.map_err(broker_error)
}

/// The owner's Stop in a worker's watch tab (Phase 12, ADR-031): the command running now is sent
/// TERM, then KILL. The worker's step goes on (Disconnect ends it: `take_over_control`).
#[tauri::command]
pub fn stop_server_command(
    broker: Org<'_, Broker>,
    command_id: String,
) -> Result<(), CommandError> {
    validate_id("command", &command_id)?;
    broker
        .stop_server_command(&command_id)
        .map_err(broker_error)
}

// ---- The owner's terminal (Phase 12, ADR-031) ---------------------------------------------------

/// Most bytes typed or pasted at once.
const MAX_TERMINAL_INPUT: usize = 64 * 1024;

/// Settings → Terminal: the shell for this PC, the choices, and the terminals open now.
#[tauri::command]
pub async fn get_terminal_settings(
    broker: State<'_, Broker>,
) -> Result<TerminalSettings, CommandError> {
    with_broker(&broker, Broker::terminal_settings).await
}

/// Choose the shell a new terminal on this PC starts. A choice, never a path.
#[tauri::command]
pub async fn set_terminal_shell<R: Runtime>(
    app: AppHandle<R>,
    broker: State<'_, Broker>,
    shell: TerminalShell,
) -> Result<TerminalSettings, CommandError> {
    let saved = with_broker(&broker, move |b| b.set_terminal_shell(shell)).await?;
    // Your choices for the PC: every organization's terminals follow (Phase 21, ADR-094 §6).
    crate::org_host::preferences_changed(&crate::orgs::all_stacks(&app));
    Ok(saved)
}

/// Open a terminal for you, on this PC or on a server from Settings → Servers. What it shows
/// comes on `events`, as it happens; nothing you type or see is recorded.
///
/// An AI tool's place (Phase 19, ADR-058) opens a tab that runs that tool's own sign-in or
/// sign-out program, from its fixed list, through the AI tools service: Guard decides first, and
/// the tool is checked again when the program ends. You sign in; Plenipo never types into it.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn open_terminal<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    orgs: State<'_, Arc<crate::orgs::Orgs>>,
    broker: Org<'_, Broker>,
    ai_tools: State<'_, plenipo_capabilities::ai_tools::AiTools>,
    place: TerminalPlace,
    cols: u16,
    rows: u16,
    events: Channel<TerminalEvent>,
) -> Result<TerminalInfo, CommandError> {
    if let TerminalPlace::Server { server_id } = &place {
        validate_id("server", server_id)?;
    }
    let sink: TerminalSink = std::sync::Arc::new(move |event| {
        // The window may have closed; the terminal then ends with Plenipo.
        let _ = events.send(event);
    });
    if let TerminalPlace::AiTool { runtime_id, action } = &place {
        validate_runtime_id(runtime_id)?;
        let ai_tools = ai_tools.inner().clone();
        let info = ai_tools
            .open_account(runtime_id, *action, cols, rows, sink)
            .await
            .map_err(broker_error)?;
        // The PC's (the first organization's broker runs it), shown in this window (Phase 21).
        orgs.own_ai_terminal(&info.id, window.label());
        return Ok(info);
    }
    let broker = broker.inner().clone();
    broker
        .open_terminal(&place, cols, rows, sink)
        .await
        .map_err(broker_error)
}

/// What you type (or paste) into a terminal.
#[tauri::command]
pub fn write_terminal<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    orgs: State<'_, Arc<crate::orgs::Orgs>>,
    broker: Org<'_, Broker>,
    terminal_id: String,
    data: String,
) -> Result<(), CommandError> {
    validate_id("terminal", &terminal_id)?;
    if data.len() > MAX_TERMINAL_INPUT {
        return Err(CommandError::invalid_input(
            "that is too much to paste at once",
        ));
    }
    terminal_broker(&window, &orgs, &broker, &terminal_id)
        .write_terminal(&terminal_id, data.as_bytes())
        .map_err(broker_error)
}

/// The broker running terminal `id` for this window: its organization's, or, for an AI tool's
/// sign-in this window opened, the first organization's (the PC's AI tools, Phase 21). Never
/// another window's.
fn terminal_broker<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    orgs: &crate::orgs::Orgs,
    broker: &Broker,
    id: &str,
) -> Broker {
    match orgs.ai_terminal_window(id) {
        Some(label) if label == window.label() => orgs
            .first()
            .map_or_else(|| broker.clone(), |f| f.broker.clone()),
        _ => broker.clone(),
    }
}

/// The terminal's panel changed size (in characters).
#[tauri::command]
pub fn resize_terminal<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    orgs: State<'_, Arc<crate::orgs::Orgs>>,
    broker: Org<'_, Broker>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    validate_id("terminal", &terminal_id)?;
    terminal_broker(&window, &orgs, &broker, &terminal_id)
        .resize_terminal(&terminal_id, cols, rows)
        .map_err(broker_error)
}

/// Close a terminal: its shell, and the programs it started, end.
#[tauri::command]
pub fn close_terminal<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    orgs: State<'_, Arc<crate::orgs::Orgs>>,
    broker: Org<'_, Broker>,
    terminal_id: String,
) -> Result<(), CommandError> {
    validate_id("terminal", &terminal_id)?;
    let result = terminal_broker(&window, &orgs, &broker, &terminal_id)
        .close_terminal(&terminal_id, "you closed it")
        .map_err(broker_error);
    orgs.forget_ai_terminal(&terminal_id);
    result
}

pub(crate) fn app_info_for(version: &str) -> AppInfo {
    AppInfo::current(version)
}

fn validate_profile_id(id: &str) -> Result<(), CommandError> {
    plenipo_runtime::profile::validate_profile_id(id).map_err(CommandError::invalid_input)
}

fn validate_execution_id(id: &str) -> Result<(), CommandError> {
    let valid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(CommandError::invalid_input("invalid execution id"))
    }
}

pub(crate) fn to_command_error(e: RuntimeError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_uses_supplied_version() {
        let info = app_info_for("9.9.9");
        assert_eq!(info.version, "9.9.9");
        assert_eq!(info.name, plenipo_core::PRODUCT_NAME);
    }

    #[test]
    fn get_app_info_reports_package_version() {
        let app = tauri::test::mock_app();
        let info = get_app_info(app.handle().clone()).unwrap();
        assert_eq!(info.version, app.package_info().version.to_string());
        assert_eq!(info.name, "Plenipo");
    }

    #[test]
    fn execution_id_validation() {
        assert!(validate_execution_id("0f8fad5b-d9cb-469f-a165-70867728950e").is_ok());
        for bad in [
            "",
            "nope",
            "../../etc/passwd",
            &"a".repeat(36),
            &"0".repeat(37),
        ] {
            let accepted = validate_execution_id(bad).is_ok();
            // 36 hex chars without dashes is harmless (it simply won't match), but
            // anything with path characters or the wrong length must be refused.
            if bad.len() != 36 || bad.contains('/') {
                assert!(!accepted, "{bad:?}");
            }
        }
    }

    #[test]
    fn agent_input_validation() {
        assert!(validate_runtime_id("claude-code").is_ok());
        for bad in ["", "Claude", "../codex", "codex --yolo", &"a".repeat(33)] {
            assert!(validate_runtime_id(bad).is_err(), "{bad:?}");
        }
        assert!(validate_session_id("0f8fad5b-d9cb-469f-a165-70867728950e").is_ok());
        assert!(validate_session_id("../../x").is_err());
        assert!(validate_objective(&"x".repeat(MAX_OBJECTIVE_BYTES)).is_ok());
        assert!(validate_objective(&"x".repeat(MAX_OBJECTIVE_BYTES + 1)).is_err());
    }

    #[test]
    fn profile_id_validation() {
        assert!(validate_profile_id("diagnostic.echo").is_ok());
        for bad in [
            "",
            "/bin/sh",
            "C:\\Windows\\cmd.exe",
            "diagnostic.echo && calc",
        ] {
            assert!(validate_profile_id(bad).is_err(), "{bad:?}");
        }
    }
}
