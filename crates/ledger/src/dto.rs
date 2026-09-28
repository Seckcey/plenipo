//! Ledger data types. Those shown in the UI derive `TS` and are exported to `@plenipo/types`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Task lifecycle.
///
/// ```text
/// queued ──▶ running ──▶ succeeded | failed | cancelled
///   │          │  ▲
///   │          ▼  │
///   │       blocked / awaitingApproval ──▶ failed | cancelled
///   └──▶ failed | cancelled
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskState {
    Queued,
    Running,
    Blocked,
    AwaitingApproval,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskState {
    pub const ALL: [Self; 7] = [
        Self::Queued,
        Self::Running,
        Self::Blocked,
        Self::AwaitingApproval,
        Self::Succeeded,
        Self::Failed,
        Self::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::AwaitingApproval => "awaitingApproval",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use TaskState::*;
        matches!(
            (self, next),
            (Queued, Running | Failed | Cancelled)
                | (
                    Running,
                    Blocked | AwaitingApproval | Succeeded | Failed | Cancelled
                )
                | (Blocked, Running | Failed | Cancelled)
                | (AwaitingApproval, Running | Failed | Cancelled)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub parent_task_id: Option<String>,
    pub requested_by: String,
    pub assigned_to: Option<String>,
    pub project_id: Option<String>,
    pub objective: String,
    pub acceptance_criteria: String,
    /// 0 (highest) – 4 (lowest).
    pub priority: u8,
    pub state: TaskState,
    #[ts(type = "Record<string, unknown>")]
    pub metadata: Value,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
    #[ts(type = "number | null")]
    pub completed_at: Option<u64>,
}

/// Input for [`crate::Ledger::create_task`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewTask {
    pub parent_task_id: Option<String>,
    pub requested_by: String,
    pub assigned_to: Option<String>,
    pub project_id: Option<String>,
    pub objective: String,
    pub acceptance_criteria: String,
    pub priority: u8,
    /// Must be a JSON object (or `Null`, meaning `{}`).
    pub metadata: Value,
}

/// One entry in the append-only activity trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LedgerEvent {
    /// Global, strictly increasing order.
    #[ts(type = "number")]
    pub seq: u64,
    pub id: String,
    pub task_id: Option<String>,
    pub execution_id: Option<String>,
    pub source: String,
    pub destination: Option<String>,
    /// Dotted lowercase name, e.g. `task.state_changed`.
    pub event_type: String,
    #[ts(type = "unknown")]
    pub payload: Value,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// Input for [`crate::Ledger::append_event`].
#[derive(Debug, Clone, Default)]
pub struct NewEvent {
    pub task_id: Option<String>,
    pub execution_id: Option<String>,
    pub source: String,
    pub destination: Option<String>,
    pub event_type: String,
    pub payload: Value,
}

/// A task with its complete ordered trail and direct children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskTimeline {
    pub task: Task,
    pub events: Vec<LedgerEvent>,
    pub children: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IntegrityReport {
    pub ok: bool,
    /// `["ok"]` when healthy; otherwise SQLite's findings.
    pub messages: Vec<String>,
    #[ts(type = "number")]
    pub checked_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupInfo {
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub created_at: u64,
    /// The backup was reopened and passed an integrity check.
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportInfo {
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LedgerStatus {
    /// `null` for an in-memory ledger.
    pub path: Option<String>,
    pub schema_version: u32,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub task_count: u64,
    #[ts(type = "number")]
    pub event_count: u64,
    #[ts(type = "number")]
    pub execution_count: u64,
    /// Recovery, migration, and backup notices the user should see.
    pub notices: Vec<String>,
    pub last_integrity_check: Option<IntegrityReport>,
    pub last_backup: Option<BackupInfo>,
    /// False when the ledger could not be opened and Plenipo is running on a temporary one.
    pub persistent: bool,
}

// ---- Organization ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleType {
    Superintendent,
    DepartmentManager,
    ProjectCoordinator,
    Worker,
}

impl RoleType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Superintendent => "superintendent",
            Self::DepartmentManager => "department_manager",
            Self::ProjectCoordinator => "project_coordinator",
            Self::Worker => "worker",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Superintendent,
            Self::DepartmentManager,
            Self::ProjectCoordinator,
            Self::Worker,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub role_type: RoleType,
    pub persistent: bool,
    pub model_policy_id: Option<String>,
    pub capability_profile_id: Option<String>,
    pub metadata: Value,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Department {
    pub id: String,
    pub name: String,
    pub description: String,
    pub manager_role_id: Option<String>,
    /// `active` or `inactive`.
    pub status: String,
    pub metadata: Value,
    pub created_at: u64,
    /// The position that heads the department (Phase 5).
    pub head_position_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub local_path: Option<String>,
    pub repository_url: Option<String>,
    pub department_id: Option<String>,
    pub metadata: Value,
    pub created_at: u64,
    pub description: String,
    /// The project's coordinator position (Phase 5).
    pub coordinator_position_id: Option<String>,
    /// Runtime IDs the project's workers may use; empty allows none.
    pub allowed_runtimes: Vec<String>,
    /// The permission set (Guard, Phase 7) that limits what the project's workers may do;
    /// `None`: no limit.
    pub capability_profile: Option<String>,
    /// `active` or `archived`.
    pub status: String,
    /// Each objective gets its own branch and working copy when the folder is a git repository
    /// (Phase 8, ADR-016).
    pub branch_per_objective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLifecycle {
    Starting,
    Active,
    Idle,
    Retired,
    Failed,
}

impl AgentLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Retired => "retired",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Starting,
            Self::Active,
            Self::Idle,
            Self::Retired,
            Self::Failed,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use AgentLifecycle::*;
        matches!(
            (self, next),
            (Starting, Active | Failed | Retired)
                | (Active, Idle | Retired | Failed)
                | (Idle, Active | Retired | Failed)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstance {
    pub id: String,
    pub role_id: String,
    pub runtime_provider: Option<String>,
    pub provider_session_id: Option<String>,
    pub project_id: Option<String>,
    pub lifecycle_state: AgentLifecycle,
    pub metadata: Value,
    pub created_at: u64,
    pub last_seen_at: u64,
    /// The position it fills (Phase 5).
    pub position_id: Option<String>,
    /// The runtime (adapter ID) it runs on.
    pub runtime_id: Option<String>,
    pub model: Option<String>,
    /// For a worker spawned by an on-demand position: the task it exists for. Its lifecycle
    /// follows that task (ADR-009 §4).
    pub task_id: Option<String>,
    pub retired_at: Option<u64>,
}

impl AgentInstance {
    /// In the active workforce (not retired or failed).
    pub fn is_active(&self) -> bool {
        !matches!(
            self.lifecycle_state,
            AgentLifecycle::Retired | AgentLifecycle::Failed
        )
    }
}

// ---- Workforce (Phase 5, ADR-009) ------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PositionState {
    Active,
    Archived,
}

impl PositionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Active, Self::Archived]
            .into_iter()
            .find(|v| v.as_str() == s)
    }
}

/// A place in the organization chart. Its role decides whether it is persistent (held by one
/// agent at a time, or vacant) or on demand (a new agent for every task delegated to it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub id: String,
    pub title: String,
    pub role_id: String,
    /// The supervisor; `None` means the position reports to the owner.
    pub reports_to: Option<String>,
    /// The runtime (adapter ID) the owner fixed for it, or `None`: automatic — its role's model
    /// policy picks the runtime and model for each worker or agent (Phase 6, ADR-011).
    pub runtime_id: Option<String>,
    /// The fixed model (`None`: the runtime's default). Always `None` for an automatic position.
    pub model: Option<String>,
    pub state: PositionState,
    pub sort_key: i64,
    pub metadata: Value,
    pub created_at: u64,
    pub updated_at: u64,
    pub archived_at: Option<u64>,
}

/// What an oversight assignment makes a position do for a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OversightKind {
    Review,
    Qa,
    Security,
}

impl OversightKind {
    pub const ALL: [Self; 3] = [Self::Review, Self::Qa, Self::Security];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Qa => "qa",
            Self::Security => "security",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// How the assignment reads, e.g. "QA evaluator".
    pub fn label(self) -> &'static str {
        match self {
            Self::Review => "reviewer",
            Self::Qa => "QA evaluator",
            Self::Security => "security auditor",
        }
    }
}

/// `overseer` reviews, QA-evaluates, or security-audits the team led by `target`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Oversight {
    pub id: String,
    pub kind: OversightKind,
    pub overseer_id: String,
    pub target_id: String,
    pub active: bool,
    pub created_at: u64,
    pub ended_at: Option<u64>,
}

/// Input for a new position (and, when `staffed`, its first incumbent).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewPosition {
    pub title: String,
    pub role_id: String,
    /// `None`: reports to the owner. Ignored for a department head created with its department
    /// when set by the caller, and for a coordinator (it reports to its department's head).
    pub reports_to: Option<String>,
    /// A fixed runtime, or `None`: automatic (see [`Position::runtime_id`]).
    pub runtime_id: Option<String>,
    /// The runtime's provider, recorded with agent instances.
    pub runtime_provider: Option<String>,
    pub model: Option<String>,
    /// Hire an incumbent now (persistent positions only).
    pub staffed: bool,
}

/// Project fields set when a project is created or updated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectSettings {
    pub name: String,
    pub description: String,
    pub repository_url: Option<String>,
    pub local_path: Option<String>,
    pub allowed_runtimes: Vec<String>,
    pub capability_profile: Option<String>,
    /// A branch and working copy per objective (Phase 8); `None`: on for a new project,
    /// unchanged for an existing one.
    pub branch_per_objective: Option<bool>,
}

/// Changes to a position; `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PositionPatch {
    pub title: Option<String>,
    /// A new runtime (with its provider), or `Some(None)`: automatic. For a staffed persistent
    /// position the incumbent is replaced: a runtime session belongs to one runtime.
    pub runtime: Option<Option<(String, Option<String>)>>,
    /// `Some(None)` clears the model.
    pub model: Option<Option<String>>,
}

/// A worker to record with a delegated child task (Liaison, ADR-009 §5).
#[derive(Debug, Clone, PartialEq)]
pub struct NewWorker {
    /// Chosen by the caller so the child's metadata can name it.
    pub agent_id: String,
    pub position_id: String,
    pub role_id: String,
    pub runtime_id: String,
    pub runtime_provider: Option<String>,
    pub model: Option<String>,
    pub project_id: Option<String>,
    /// Why the Router chose this runtime and model (recorded with `org.worker_spawned`), or null.
    pub routing: Value,
}

/// A working copy of a project's repository (a git worktree) on its own branch, made for one
/// objective (Phase 8, ADR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    /// The objective's workflow (Liaison's correlation ID).
    pub correlation_id: String,
    /// The objective's own task.
    pub root_task_id: Option<String>,
    /// Set for a second working copy, made from this one for a worker that changed files while
    /// another held it.
    pub parent_id: Option<String>,
    /// The repository's top folder (the owner's checkout).
    pub repository: String,
    /// Where the project folder is inside the repository (`""`: its top).
    pub subfolder: String,
    /// The working copy's top folder.
    pub path: String,
    pub branch: String,
    /// The branch it was made from, if the checkout was on one.
    pub base_ref: Option<String>,
    pub base_commit: String,
    pub state: WorkspaceState,
    /// The branch as last seen.
    pub facts: WorkspaceFacts,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    #[ts(type = "number | null")]
    pub removed_at: Option<u64>,
}

impl Workspace {
    /// The project folder inside the working copy.
    pub fn folder(&self) -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(&self.path);
        for part in self.subfolder.split('/').filter(|p| !p.is_empty()) {
            path.push(part);
        }
        path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkspaceState {
    Active,
    /// Its folder was removed; the branch stays in the repository.
    Removed,
}

impl WorkspaceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Removed => "removed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

/// What is on a working copy's branch: its commits since it was made and the files changed,
/// committed or not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct WorkspaceFacts {
    pub head: Option<String>,
    /// Newest first.
    pub commits: Vec<CommitInfo>,
    pub files: Vec<FileChange>,
    /// Files changed but not committed.
    pub uncommitted: u32,
    /// The branch has been pushed to the remote server.
    pub pushed: bool,
    #[ts(type = "number | null")]
    pub checked_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitInfo {
    /// Short hash.
    pub hash: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChange {
    /// Relative to the repository's top folder, with `/`.
    pub path: String,
    /// Lines added and removed (`None` for a binary file or one not yet in git).
    pub added: Option<u32>,
    pub removed: Option<u32>,
    /// In a commit on the branch; otherwise only in the working copy.
    pub committed: bool,
}

/// Input for [`crate::Ledger::create_workspace`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkspace {
    pub project_id: String,
    pub correlation_id: String,
    pub root_task_id: Option<String>,
    pub parent_id: Option<String>,
    pub repository: String,
    pub subfolder: String,
    pub path: String,
    pub branch: String,
    pub base_ref: Option<String>,
    pub base_commit: String,
}

/// The conversation that runs a task delegated to a full-time member of the organization
/// (Phase 8, ADR-016): the member's own runtime session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildConversation {
    pub session_id: String,
    pub runtime_id: String,
    pub model: Option<String>,
    /// The effort level (`None`: the runtime's default).
    pub effort: Option<String>,
}

/// The runtime and model an automatic position's agent starts its conversation on, and why
/// (Phase 6, ADR-011).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRoute {
    pub runtime_id: String,
    pub runtime_provider: Option<String>,
    pub model: Option<String>,
    /// The Router's decision, recorded with `org.agent_routed`.
    pub routing: Value,
}

/// The latest usage-limit or success result of agent turns (usage/capacity state, Phase 6).
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcomeRecord {
    pub runtime: String,
    pub model: Option<String>,
    /// `usageLimited` or `completed`.
    pub outcome: String,
    pub error: Option<String>,
    pub summary: Option<String>,
    /// When the result was recorded (ms).
    pub at: u64,
}

/// A model a runtime reported running (or was asked to run), from the executions.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenModel {
    pub runtime: String,
    pub model: String,
    pub runs: u32,
    pub last_used: u64,
}

/// The whole organization as recorded: everything a snapshot is built from, in one read.
#[derive(Debug, Clone, PartialEq)]
pub struct OrgRecords {
    pub roles: Vec<Role>,
    pub departments: Vec<Department>,
    pub projects: Vec<Project>,
    /// Active and archived positions.
    pub positions: Vec<Position>,
    /// Agents in the active workforce.
    pub agents: Vec<AgentInstance>,
    /// Active oversight assignments.
    pub oversight: Vec<Oversight>,
    /// Per position: agents that have left the workforce (retired, failed).
    pub former_agents: std::collections::HashMap<String, FormerAgents>,
}

/// History of a position's agents that have left the workforce.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormerAgents {
    pub retired: u32,
    pub failed: u32,
    pub last_retired_at: Option<u64>,
}

// ---- Execution, approval, artifact ----------------------------------------------------

/// Durable record of one runtime execution (local process now; provider sessions later).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRow {
    pub id: String,
    pub task_id: Option<String>,
    /// e.g. `local-process`; provider runtimes in Phase 3.
    pub runtime: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub process_id: Option<u32>,
    pub profile_id: Option<String>,
    pub label: String,
    pub executable: Option<String>,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    /// Runtime execution state (`starting`, `running`, `succeeded`, …).
    pub state: String,
    pub exit_code: Option<i32>,
    pub detail: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub usage_metadata: Value,
}

pub const EXECUTION_STATES: [&str; 7] = [
    "starting",
    "running",
    "succeeded",
    "failed",
    "cancelled",
    "timedOut",
    "interrupted",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl ApprovalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Pending, Self::Approved, Self::Rejected, Self::Expired]
            .into_iter()
            .find(|t| t.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: String,
    pub task_id: String,
    pub action_type: String,
    pub request_payload: Value,
    pub state: ApprovalState,
    pub requested_at: u64,
    pub expires_at: Option<u64>,
    pub resolved_at: Option<u64>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub task_id: Option<String>,
    pub artifact_type: String,
    pub local_path: Option<String>,
    pub uri: Option<String>,
    pub hash: Option<String>,
    pub metadata: Value,
    pub created_at: u64,
}

// ---- Phase 12: what the pages show ----------------------------------------------------------

/// A pull request a worker opened (Phase 8), for a project's or a task's page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullRequestRef {
    pub url: String,
    #[ts(type = "number | null")]
    pub number: Option<u64>,
    /// The task that opened it.
    pub task_id: String,
    pub worker: Option<String>,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// Something kept as evidence (a screenshot), for a project's or a task's page. Plenipo opens
/// it by its ID; the screen never names a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactView {
    pub id: String,
    pub task_id: Option<String>,
    /// "screenshot".
    pub kind: String,
    /// What it shows, when Plenipo noted it (a page's title or address).
    pub label: Option<String>,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// A piece of work's pull requests, artifacts, and decisions (newest first): the pages of a
/// project and of a task.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkRecord {
    pub pull_requests: Vec<PullRequestRef>,
    pub artifacts: Vec<ArtifactView>,
    /// Approvals answered or expired, refusals, handoffs refused, lessons kept or discarded,
    /// the owner's stops and take-overs, and why each worker got its AI tool.
    pub decisions: Vec<LedgerEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use TaskState::*;

    #[test]
    fn task_state_round_trips() {
        for s in TaskState::ALL {
            assert_eq!(TaskState::parse(s.as_str()), Some(s));
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                Value::String(s.as_str().into())
            );
        }
    }

    #[test]
    fn terminal_states_are_final() {
        for from in TaskState::ALL.into_iter().filter(|s| s.is_terminal()) {
            for to in TaskState::ALL {
                assert!(!from.can_transition_to(to), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn allowed_task_transitions() {
        let allowed = [
            (Queued, Running),
            (Queued, Failed),
            (Queued, Cancelled),
            (Running, Blocked),
            (Running, AwaitingApproval),
            (Running, Succeeded),
            (Running, Failed),
            (Running, Cancelled),
            (Blocked, Running),
            (Blocked, Failed),
            (Blocked, Cancelled),
            (AwaitingApproval, Running),
            (AwaitingApproval, Failed),
            (AwaitingApproval, Cancelled),
        ];
        for from in TaskState::ALL {
            for to in TaskState::ALL {
                assert_eq!(
                    from.can_transition_to(to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn agent_lifecycle_transitions() {
        use AgentLifecycle::*;
        assert!(Starting.can_transition_to(Active));
        assert!(Active.can_transition_to(Idle));
        assert!(Idle.can_transition_to(Active));
        assert!(!Retired.can_transition_to(Active));
        assert!(!Failed.can_transition_to(Active));
        assert!(!Starting.can_transition_to(Idle));
    }
}

// ---- Runtime sessions (Phase 3, ADR-007) -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeSessionState {
    Open,
    Closed,
}

impl RuntimeSessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Open, Self::Closed]
            .into_iter()
            .find(|v| v.as_str() == s)
    }
}

/// One conversation with an agent runtime. Its turns are tasks whose metadata carries
/// `sessionId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSession {
    pub id: String,
    /// Adapter ID, e.g. `claude-code`.
    pub runtime: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub provider_session_confirmed: bool,
    pub model: Option<String>,
    pub title: String,
    pub working_dir: String,
    pub state: RuntimeSessionState,
    pub metadata: Value,
    pub created_at: u64,
    pub updated_at: u64,
    pub closed_at: Option<u64>,
    /// Number of turns (tasks) recorded for the session.
    pub turn_count: u32,
    /// Effort level every turn runs at (`None`: the runtime's default).
    pub effort: Option<String>,
}

/// Input for [`crate::Ledger::open_runtime_session`]. The caller chooses the ID (it also names
/// the session's working directory).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewRuntimeSession {
    pub id: String,
    pub runtime: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub title: String,
    pub working_dir: String,
    pub metadata: Value,
}

// ---- Liaison messages (Phase 4, ADR-008) --------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageKind {
    /// Asks for a child task on another worker.
    Request,
    /// Carries the child's result (or Liaison's refusal) back to the requesting task.
    Reply,
}

impl MessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Reply => "reply",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Request, Self::Reply]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

/// Message lifecycle.
///
/// ```text
/// request:  accepted ──▶ dispatched ──▶ answered        rejected (refused on arrival)
///               └──────────┴──────▶ cancelled
/// reply:    pending ──▶ delivered | discarded
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageState {
    Accepted,
    Dispatched,
    Answered,
    Cancelled,
    Rejected,
    Pending,
    Delivered,
    Discarded,
}

impl MessageState {
    pub const ALL: [Self; 8] = [
        Self::Accepted,
        Self::Dispatched,
        Self::Answered,
        Self::Cancelled,
        Self::Rejected,
        Self::Pending,
        Self::Delivered,
        Self::Discarded,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Dispatched => "dispatched",
            Self::Answered => "answered",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Discarded => "discarded",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// A request still waiting for its reply.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Accepted | Self::Dispatched)
    }
}

/// One persisted Liaison message. Everything but `state` and `updated_at` is immutable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiaisonMessage {
    pub id: String,
    /// Shared by every message of one workflow.
    pub correlation_id: String,
    pub kind: MessageKind,
    /// For a reply: the request it answers.
    pub in_reply_to: Option<String>,
    /// The requesting task (for a reply: the task it is addressed to).
    pub task_id: String,
    /// The child task created for a request (for a reply: the child that answered, if any).
    pub child_task_id: Option<String>,
    /// Address of the sender, e.g. `session:<id>`, or `liaison` for Plenipo's own replies.
    pub source: String,
    /// Address of the receiver, e.g. `runtime:claude-code` or `session:<id>`.
    pub destination: String,
    pub state: MessageState,
    pub dedupe_key: String,
    /// The full envelope (JSON object).
    pub envelope: Value,
    pub created_at: u64,
    pub updated_at: u64,
}

/// A request recorded by [`crate::Ledger::suspend_for_handoffs`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewHandoffRequest {
    pub message_id: String,
    pub correlation_id: String,
    /// Identical requests share a key: recording one twice changes nothing.
    pub dedupe_key: String,
    pub source: String,
    pub destination: String,
    /// The request envelope (JSON object).
    pub envelope: Value,
    /// Payload of `liaison.handoff_requested` on the requesting task (JSON object).
    pub summary: Value,
    pub decision: HandoffDecision,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HandoffDecision {
    /// Create `child` (queued) under the requesting task. `received` is the payload of
    /// `liaison.handoff_received` on the child (JSON object). `worker`, when the request went
    /// to a position in the organization, is recorded as the agent spawned for the child
    /// (`org.worker_spawned`), in the same transaction.
    Accept {
        child: NewTask,
        received: Value,
        worker: Option<Box<NewWorker>>,
    },
    /// Refuse the request. The requester learns why through `reply`, which is recorded as
    /// pending with the request.
    Reject { reason: String, reply: NewReply },
}

/// A reply to a request. Its claimed correlation, request, and child must match the recorded
/// request, or it is refused.
#[derive(Debug, Clone, PartialEq)]
pub struct NewReply {
    pub message_id: String,
    pub correlation_id: String,
    pub in_reply_to: String,
    /// The child task that answered (`None` for Liaison's own replies, e.g. a refusal).
    pub child_task_id: Option<String>,
    pub source: String,
    /// The reply envelope (JSON object).
    pub envelope: Value,
    /// Extra payload for `liaison.reply_sent` / `liaison.reply_received` (JSON object).
    pub summary: Value,
}

/// What [`crate::Ledger::suspend_for_handoffs`] recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct Suspended {
    pub task: Task,
    /// The requests, in order (found rather than created when replayed).
    pub requests: Vec<LiaisonMessage>,
    /// Child tasks created by this call.
    pub children: Vec<Task>,
    /// Every request had already been recorded: nothing changed.
    pub replayed: bool,
}

/// What [`crate::Ledger::answer_request`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplyOutcome {
    Recorded(LiaisonMessage),
    /// The request was already answered by this reply.
    AlreadyAnswered(LiaisonMessage),
    /// The request no longer takes replies (it was cancelled or rejected).
    NotOpen(MessageState),
}

/// What [`crate::Ledger::cancel_request`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum CancelOutcome {
    /// The request was no longer open: nothing changed.
    NotOpen,
    /// The request is cancelled. `child` is its child task as it is now: a child that had not
    /// started is cancelled with it; a running or waiting one must be stopped by its runtime.
    Cancelled { child: Option<Box<Task>> },
}

/// An open request with the current states of its tasks.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenRequest {
    pub message: LiaisonMessage,
    pub parent_state: TaskState,
    pub child: Option<Task>,
}

/// A lesson a worker learned from its work (ADR-024): what would help the next worker in its
/// role. It waits for the owner unless the role learns on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Lesson {
    pub id: String,
    pub role_id: String,
    /// The task it came from.
    pub task_id: Option<String>,
    pub position_id: Option<String>,
    /// Who learned it ("Web Assistant").
    pub worker: String,
    pub text: String,
    pub state: LessonState,
    /// Its task (or a task handed on from it) used websites, the screen, or a server (Phase 11):
    /// such lessons always wait for the owner.
    pub from_web: bool,
    /// The project of the task it came from: only workers on that project get it. `None`:
    /// every worker of the role (ADR-040).
    pub project_id: Option<String>,
    /// Why a lesson of a role that learns on its own waits for the owner anyway, in the owner's
    /// words (ADR-040). `None` for every other lesson.
    pub held_reason: Option<String>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub decided_at: Option<u64>,
    pub decided_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LessonState {
    /// Waiting for the owner's Keep or Discard.
    Waiting,
    /// In the instructions of the role's later workers.
    Kept,
    Discarded,
    /// Kept, then removed by the owner.
    Removed,
}

impl LessonState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Kept => "kept",
            Self::Discarded => "discarded",
            Self::Removed => "removed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "waiting" => Some(Self::Waiting),
            "kept" => Some(Self::Kept),
            "discarded" => Some(Self::Discarded),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

/// Input for [`crate::Ledger::add_lessons`]: what one task's worker learned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLessons {
    pub role_id: String,
    pub task_id: String,
    pub position_id: Option<String>,
    pub worker: String,
    pub texts: Vec<String>,
    pub from_web: bool,
    /// Keep them at once (the role learns on its own); otherwise they wait for the owner.
    pub keep: bool,
    /// The project of the task they came from, if any (ADR-040).
    pub project_id: Option<String>,
    /// Why they wait although the role learns on its own, for the owner (ADR-040). Only with
    /// `keep` false.
    pub held_reason: Option<String>,
}

/// Which part of the organization an activity series covers (Phase 12A, ADR-030 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
#[ts(export)]
pub enum ActivityScope {
    /// Everything in the Ledger.
    All,
    /// A department: its projects' tasks and the work of every position under its head.
    Department(String),
    /// A project: its tasks and the work of its supervisor's team.
    Project(String),
    /// A position and the positions reporting to it.
    Position(String),
}

/// One time bucket of an activity series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityBucket {
    /// Everything recorded in the bucket.
    pub events: u32,
    /// Failures, refusals, and timeouts (a task blocked on handoff replies is not a problem).
    pub problems: u32,
    /// Requests for the owner's approval.
    pub waiting: u32,
}

/// Events counted in fixed time buckets, for activity strips and timelines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivitySeries {
    /// Start of the first bucket (ms since the Unix epoch).
    #[ts(type = "number")]
    pub from: u64,
    /// End of the last bucket (ms since the Unix epoch).
    #[ts(type = "number")]
    pub to: u64,
    /// Width of each bucket (ms).
    #[ts(type = "number")]
    pub bucket_ms: u64,
    pub buckets: Vec<ActivityBucket>,
}
