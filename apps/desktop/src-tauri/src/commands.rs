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
//! model name), roles, and choices; it never selects a worker's AI tool directly.

use std::sync::Arc;

use plenipo_core::{AppInfo, CommandError, SyntheticTaskAction};
use plenipo_ledger::{
    BackupInfo, ExportInfo, IntegrityReport, Ledger, LedgerError, LedgerEvent, LedgerStatus,
    NewTask, Task, TaskState, TaskTimeline,
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
    DepartmentInput, HireInput, LeadInput, OrgSnapshot, OversightRole, PositionPatchInput,
    ProjectInput, RoleInput, TitleTheme, WorkView, Workforce, WorkforceError,
};
use tauri::{AppHandle, Runtime, State};

use crate::smoke::{SmokeTest, EXIT_READY};

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
    if smoke.is_enabled() && smoke.record(EXIT_READY) {
        eprintln!("[plenipo] smoke test: frontend reported ready; exiting 0");
        app.exit(EXIT_READY);
    }
    Ok(())
}

/// Profiles, execution history (newest first), active count, and notices.
#[tauri::command]
pub fn get_runtime_overview(
    supervisor: State<'_, Supervisor>,
) -> Result<RuntimeOverview, CommandError> {
    Ok(supervisor.overview())
}

/// Launch an approved profile by ID.
#[tauri::command]
pub async fn start_execution(
    supervisor: State<'_, Supervisor>,
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
    supervisor: State<'_, Supervisor>,
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
    supervisor: State<'_, Supervisor>,
    execution_id: String,
) -> Result<ExecutionOutput, CommandError> {
    validate_execution_id(&execution_id)?;
    supervisor.output(&execution_id).map_err(to_command_error)
}

// ---- Ledger ------------------------------------------------------------------------------

/// Actor recorded for actions taken by the person using the app.
const OWNER: &str = "owner";

/// Run ledger work off the main thread.
async fn with_ledger<T: Send + 'static>(
    ledger: &Arc<Ledger>,
    f: impl FnOnce(&Ledger) -> Result<T, LedgerError> + Send + 'static,
) -> Result<T, CommandError> {
    let ledger = Arc::clone(ledger);
    tauri::async_runtime::spawn_blocking(move || f(&ledger))
        .await
        .map_err(|e| CommandError::internal(format!("ledger task failed: {e}")))?
        .map_err(ledger_error)
}

fn ledger_error(e: LedgerError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

fn validate_task_id(id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input("invalid task id"))
}

#[tauri::command]
pub async fn get_ledger_status(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<LedgerStatus, CommandError> {
    with_ledger(&ledger, Ledger::status).await
}

/// Tasks, newest first (at most 500).
#[tauri::command]
pub async fn list_tasks(ledger: State<'_, Arc<Ledger>>) -> Result<Vec<Task>, CommandError> {
    with_ledger(&ledger, |l| l.list_tasks(500)).await
}

/// A task's complete ordered activity trail and its direct children.
#[tauri::command]
pub async fn get_task_timeline(
    ledger: State<'_, Arc<Ledger>>,
    task_id: String,
) -> Result<TaskTimeline, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| l.task_timeline(&task_id)).await
}

/// Most recent events across the ledger, newest first (at most 200).
#[tauri::command]
pub async fn list_recent_events(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<Vec<LedgerEvent>, CommandError> {
    with_ledger(&ledger, |l| l.recent_events(200)).await
}

/// Diagnostics: create a synthetic task to exercise the ledger end to end.
#[tauri::command]
pub async fn create_synthetic_task(ledger: State<'_, Arc<Ledger>>) -> Result<Task, CommandError> {
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
    ledger: State<'_, Arc<Ledger>>,
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
    ledger: State<'_, Arc<Ledger>>,
) -> Result<IntegrityReport, CommandError> {
    with_ledger(&ledger, Ledger::integrity_check).await
}

/// Verified backup into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn create_ledger_backup(
    ledger: State<'_, Arc<Ledger>>,
) -> Result<BackupInfo, CommandError> {
    with_ledger(&ledger, |l| l.backup(None)).await
}

/// JSON export into the ledger's backups folder (location chosen by Core, not the UI).
#[tauri::command]
pub async fn export_ledger(ledger: State<'_, Arc<Ledger>>) -> Result<ExportInfo, CommandError> {
    with_ledger(&ledger, |l| l.export_json(None)).await
}

// ---- Agent runtimes (Phase 3) ------------------------------------------------------------

/// Longest objective accepted at the boundary (bytes); the runtime enforces 10,000 characters.
const MAX_OBJECTIVE_BYTES: usize = 40_000;

fn validate_runtime_id(id: &str) -> Result<(), CommandError> {
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
    agents: State<'_, AgentRuntime>,
) -> Result<AgentOverview, CommandError> {
    agents.overview().await.map_err(to_command_error)
}

/// Re-detect every runtime's installation and sign-in.
#[tauri::command]
pub async fn refresh_agent_runtimes(
    agents: State<'_, AgentRuntime>,
) -> Result<Vec<AgentRuntimeInfo>, CommandError> {
    Ok(agents.refresh().await)
}

/// A session with its turns and recent live activity.
#[tauri::command]
pub async fn get_agent_session(
    agents: State<'_, AgentRuntime>,
    session_id: String,
) -> Result<AgentSessionDetail, CommandError> {
    validate_session_id(&session_id)?;
    agents.session(&session_id).await.map_err(to_command_error)
}

/// Start a new session on a runtime with a first objective (startSession + submitTask).
/// With `handoffs`, the worker may ask other workers for help through Liaison (Phase 4).
#[tauri::command]
pub async fn start_agent_session(
    liaison: State<'_, Liaison>,
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
    liaison: State<'_, Liaison>,
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
    agents: State<'_, AgentRuntime>,
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
    agents: State<'_, AgentRuntime>,
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
    liaison: State<'_, Liaison>,
    task_id: String,
) -> Result<TaskHandoffs, CommandError> {
    validate_task_id(&task_id)?;
    with_liaison(&liaison, move |l| l.task_handoffs(&task_id)).await
}

/// A task's whole delegation tree, from its root task, depth-first.
#[tauri::command]
pub async fn get_task_tree(
    liaison: State<'_, Liaison>,
    task_id: String,
) -> Result<TaskTree, CommandError> {
    validate_task_id(&task_id)?;
    with_liaison(&liaison, move |l| l.task_tree(&task_id)).await
}

/// Liaison's protocol, limits, destinations, open handoffs, and notices.
#[tauri::command]
pub async fn get_liaison_overview(
    liaison: State<'_, Liaison>,
) -> Result<LiaisonOverview, CommandError> {
    with_liaison(&liaison, Liaison::overview).await
}

// ---- Workforce (Phase 5) -----------------------------------------------------------------

/// Longest text field accepted at the boundary (bytes); the Ledger enforces the real limits.
const MAX_FIELD_BYTES: usize = 8_000;

fn workforce_error(e: WorkforceError) -> CommandError {
    if e.is_caller_error() {
        CommandError::invalid_input(e.to_string())
    } else {
        CommandError::internal(e.to_string())
    }
}

/// Run Workforce work (Ledger reads and writes) off the main thread.
async fn with_workforce<T: Send + 'static>(
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
fn validate_id(what: &str, id: &str) -> Result<(), CommandError> {
    validate_execution_id(id).map_err(|_| CommandError::invalid_input(format!("invalid {what} id")))
}

fn validate_optional_id(what: &str, id: Option<&str>) -> Result<(), CommandError> {
    id.map_or(Ok(()), |id| validate_id(what, id))
}

fn bounded(what: &str, value: &str) -> Result<(), CommandError> {
    if value.len() > MAX_FIELD_BYTES {
        Err(CommandError::invalid_input(format!("{what} is too long")))
    } else {
        Ok(())
    }
}

fn bounded_optional(what: &str, value: Option<&str>) -> Result<(), CommandError> {
    value.map_or(Ok(()), |v| bounded(what, v))
}

fn validate_runtimes(ids: &[String]) -> Result<(), CommandError> {
    if ids.len() > 16 {
        return Err(CommandError::invalid_input("too many runtimes"));
    }
    ids.iter().try_for_each(|r| validate_runtime_id(r))
}

fn validate_lead(lead: &LeadInput) -> Result<(), CommandError> {
    validate_id("role", &lead.role_id)?;
    bounded("the title", &lead.title)?;
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
pub async fn get_organization(
    workforce: State<'_, Workforce>,
) -> Result<OrgSnapshot, CommandError> {
    with_workforce(&workforce, Workforce::snapshot).await
}

/// The work a position owns and its team's unfinished work (`positionId` omitted: the whole
/// organization's).
#[tauri::command]
pub async fn get_work(
    workforce: State<'_, Workforce>,
    position_id: Option<String>,
) -> Result<WorkView, CommandError> {
    validate_optional_id("position", position_id.as_deref())?;
    with_workforce(&workforce, move |w| w.work(position_id.as_deref())).await
}

#[tauri::command]
pub async fn rename_organization(
    workforce: State<'_, Workforce>,
    name: String,
) -> Result<OrgSnapshot, CommandError> {
    bounded("the name", &name)?;
    with_workforce(&workforce, move |w| w.rename(&name)).await
}

/// What the app calls the ranks (display only; agents keep the plain titles).
#[tauri::command]
pub async fn set_organization_titles(
    workforce: State<'_, Workforce>,
    titles: TitleTheme,
) -> Result<OrgSnapshot, CommandError> {
    with_workforce(&workforce, move |w| w.set_titles(titles)).await
}

#[tauri::command]
pub async fn create_role(
    workforce: State<'_, Workforce>,
    input: RoleInput,
) -> Result<OrgSnapshot, CommandError> {
    bounded("the name", &input.name)?;
    bounded("the description", &input.description)?;
    with_workforce(&workforce, move |w| w.create_role(&input)).await
}

/// Create a department with its head position.
#[tauri::command]
pub async fn create_department(
    workforce: State<'_, Workforce>,
    input: DepartmentInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_department(&input)?;
    with_workforce(&workforce, move |w| w.create_department(&input)).await
}

#[tauri::command]
pub async fn update_department(
    workforce: State<'_, Workforce>,
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

/// Delete a department that has no projects (its head position is archived).
#[tauri::command]
pub async fn remove_department(
    workforce: State<'_, Workforce>,
    department_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("department", &department_id)?;
    with_workforce(&workforce, move |w| w.remove_department(&department_id)).await
}

/// Create a project with its coordinator position. The local folder is recorded, never opened.
#[tauri::command]
pub async fn create_project(
    workforce: State<'_, Workforce>,
    input: ProjectInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_project(&input)?;
    with_workforce(&workforce, move |w| w.create_project(&input)).await
}

#[tauri::command]
pub async fn update_project(
    workforce: State<'_, Workforce>,
    project_id: String,
    input: ProjectInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("project", &project_id)?;
    validate_project(&input)?;
    with_workforce(&workforce, move |w| w.update_project(&project_id, &input)).await
}

/// Archive a project and its whole team once none of it has unfinished work.
#[tauri::command]
pub async fn archive_project(
    workforce: State<'_, Workforce>,
    project_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("project", &project_id)?;
    with_workforce(&workforce, move |w| w.archive_project(&project_id)).await
}

/// Hire into a team: a new position (and, for a persistent one, its agent).
#[tauri::command]
pub async fn hire_position(
    workforce: State<'_, Workforce>,
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
    with_workforce(&workforce, move |w| w.hire(&input)).await
}

/// Hire an agent into a vacant persistent position.
#[tauri::command]
pub async fn fill_position(
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.fill(&position_id)).await
}

/// Let a persistent position's agent go (the position stays, vacant).
#[tauri::command]
pub async fn vacate_position(
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.vacate(&position_id)).await
}

#[tauri::command]
pub async fn update_position(
    workforce: State<'_, Workforce>,
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
    with_workforce(&workforce, move |w| w.update_position(&position_id, &input)).await
}

/// Make a position report to another (`reportsTo` null: the owner).
#[tauri::command]
pub async fn move_position(
    workforce: State<'_, Workforce>,
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
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.archive_position(&position_id)).await
}

/// Assign an on-demand position to review, QA, or security-audit a team.
#[tauri::command]
pub async fn assign_oversight(
    workforce: State<'_, Workforce>,
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
    workforce: State<'_, Workforce>,
    oversight_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("oversight assignment", &oversight_id)?;
    with_workforce(&workforce, move |w| w.end_oversight(&oversight_id)).await
}

/// Give a staffed persistent position's agent an objective. Core builds its instructions and
/// chooses its session; the UI names only the position.
#[tauri::command]
pub async fn give_objective(
    workforce: State<'_, Workforce>,
    position_id: String,
    objective: String,
) -> Result<AgentSessionDetail, CommandError> {
    validate_id("position", &position_id)?;
    validate_objective(&objective)?;
    workforce
        .give_objective(&position_id, &objective)
        .await
        .map_err(workforce_error)
}

// ---- Model policy and routing (Phase 6) --------------------------------------------------------

fn router_error(e: RouterError) -> CommandError {
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
pub async fn get_routing(router: State<'_, Router>) -> Result<RoutingSnapshot, CommandError> {
    with_router(&router, Router::snapshot).await
}

/// Add a model to the registry (no `id`) or change one.
#[tauri::command]
pub async fn save_model(
    router: State<'_, Router>,
    input: ModelInput,
) -> Result<RoutingSnapshot, CommandError> {
    validate_optional_id("model", input.id.as_deref())?;
    validate_runtime_id(&input.runtime_id)?;
    bounded_optional("the model name", input.name.as_deref())?;
    bounded("the label", &input.label)?;
    with_router(&router, move |r| r.save_model(&input)).await
}

/// Remove a model the owner added; it leaves every role's list.
#[tauri::command]
pub async fn remove_model(
    router: State<'_, Router>,
    model_id: String,
) -> Result<RoutingSnapshot, CommandError> {
    validate_id("model", &model_id)?;
    with_router(&router, move |r| r.remove_model(&model_id)).await
}

/// Replace a role's model policy.
#[tauri::command]
pub async fn set_role_policy(
    router: State<'_, Router>,
    role_id: String,
    policy: RolePolicy,
) -> Result<RoutingSnapshot, CommandError> {
    validate_id("role", &role_id)?;
    for id in &policy.models {
        validate_id("model", id)?;
    }
    validate_runtimes(&policy.never_companies)?;
    with_router(&router, move |r| r.set_policy(&role_id, &policy)).await
}

/// Choices for every role (what a usage limit does).
#[tauri::command]
pub async fn set_routing_options(
    router: State<'_, Router>,
    options: RoutingOptions,
) -> Result<RoutingSnapshot, CommandError> {
    with_router(&router, move |r| r.set_options(options)).await
}

/// Try an AI tool again now, although it reported a usage limit.
#[tauri::command]
pub async fn clear_usage_limit(
    router: State<'_, Router>,
    runtime_id: String,
) -> Result<RoutingSnapshot, CommandError> {
    validate_runtime_id(&runtime_id)?;
    with_router(&router, move |r| r.clear_limit(&runtime_id)).await
}

fn app_info_for(version: &str) -> AppInfo {
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

fn to_command_error(e: RuntimeError) -> CommandError {
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
