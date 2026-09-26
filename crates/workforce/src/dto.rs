//! Workforce DTOs shared with the frontend (camelCase on the wire). Runtimes and providers are
//! data values; no vendor appears in a type.

use plenipo_ledger::TaskState;
use plenipo_router::RouteDecision;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The class of a position, from its role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PositionKind {
    Superintendent,
    DepartmentManager,
    ProjectCoordinator,
    Worker,
}

/// How a position is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Staffing {
    /// Held by one agent at a time, which keeps its conversation.
    Persistent,
    /// A new, ephemeral worker for every task delegated to it.
    OnDemand,
}

/// What the app calls each rank of the chain of command — Worker, Supervisor, Manager, VP, and
/// the owner at the top (President in `Business`) — or a branch's ranks, or the Mafia's. Display
/// only: agents are always given the plain titles (ADR-010).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TitleTheme {
    #[default]
    Business,
    Army,
    Navy,
    AirForce,
    MarineCorps,
    CoastGuard,
    SpaceForce,
    Mafia,
}

/// What a position is doing, shown on its node (always with a text label, never color alone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PositionStatus {
    /// A persistent position without an agent.
    Vacant,
    /// Ready and not working.
    Idle,
    /// Working on a task now.
    Working,
    /// Waiting for replies to its handoffs.
    Waiting,
    /// Has work queued for a free worker slot, and nothing running.
    Queued,
    /// Cannot take work: its runtime is not ready or not allowed by its project.
    Unavailable,
    Archived,
}

/// What an oversight assignment makes a position do for a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OversightRole {
    Review,
    Qa,
    Security,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: PositionKind,
    pub staffing: Staffing,
    /// Seeded by Plenipo (vs. created by the owner).
    pub template: bool,
    /// Name of the glyph the canvas draws for it (`code`, `review`, `qa`, `shield`, …).
    pub glyph: String,
    /// What the role is for; part of its workers' instructions.
    pub purpose: Vec<String>,
    /// Capabilities the role's work calls for (its template). What a worker may actually do
    /// comes from the role's permission set in Guard (Phase 7).
    pub default_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DepartmentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub head_position_id: Option<String>,
    /// Projects of the department (active and archived).
    pub project_ids: Vec<String>,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub department_id: Option<String>,
    pub repository_url: Option<String>,
    /// The folder the project's workers work in: Plenipo's tools are confined to it (Phase 7).
    pub local_path: Option<String>,
    /// Runtime IDs its workers may use; empty allows none.
    pub allowed_runtimes: Vec<String>,
    /// The permission set that limits what the project's workers may do (Phase 7); `None`: no
    /// limit.
    pub capability_profile: Option<String>,
    pub coordinator_position_id: Option<String>,
    pub active: bool,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// The agent holding a persistent position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentInfo {
    pub id: String,
    /// The runtime of its conversation; `None` for an automatic position's agent before its
    /// first objective (the Router picks one then).
    pub runtime_id: Option<String>,
    pub model: Option<String>,
    /// Its open runtime session (its conversation), once it has had an objective.
    pub session_id: Option<String>,
    #[ts(type = "number")]
    pub hired_at: u64,
}

/// A worker spawned by an on-demand position, while it is in the active workforce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkerInfo {
    pub agent_id: String,
    pub task_id: String,
    /// First line of its task's objective.
    pub objective: String,
    /// `queued` (waiting for a worker slot), `running`, or `blocked` (waiting for replies).
    pub state: TaskState,
    pub session_id: Option<String>,
    pub runtime_id: String,
    pub model: Option<String>,
    /// Why it got this runtime and model (the Router's explanation).
    pub routing: Option<String>,
    /// The task that delegated to it.
    pub parent_task_id: Option<String>,
    #[ts(type = "number")]
    pub spawned_at: u64,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
}

/// A task as the organization shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskBrief {
    pub id: String,
    /// First line of the objective.
    pub objective: String,
    pub state: TaskState,
    pub position_id: Option<String>,
    pub position_title: Option<String>,
    pub project_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub session_id: Option<String>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
    #[ts(type = "number | null")]
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkCounts {
    pub working: u32,
    pub waiting: u32,
    pub queued: u32,
}

/// Agents that have left a position (history remains).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PositionHistory {
    pub retired: u32,
    pub failed: u32,
    #[ts(type = "number | null")]
    pub last_retired_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PositionInfo {
    pub id: String,
    pub title: String,
    pub role_id: String,
    pub role_name: String,
    pub kind: PositionKind,
    pub staffing: Staffing,
    /// The supervisor; `null` reports to the owner.
    pub reports_to: Option<String>,
    /// Its department: the nearest department head at or above it.
    pub department_id: Option<String>,
    /// Its project: the nearest coordinator at or above it.
    pub project_id: Option<String>,
    pub heads_department_id: Option<String>,
    pub coordinates_project_id: Option<String>,
    /// Its AI tool: the fixed one, its agent's conversation's, or the one its next worker would
    /// get; `None` when no model can take its work now.
    pub runtime_id: Option<String>,
    /// Its model (`None`: the AI tool's default), picked the same way.
    pub model: Option<String>,
    /// Follows its role's model policy (vs. an AI tool and model the owner fixed).
    pub automatic: bool,
    /// Where its next worker (or a new agent) would go, and why.
    pub route: Option<RouteDecision>,
    pub active: bool,
    #[ts(type = "number")]
    pub sort_key: i64,
    /// The incumbent of a persistent position.
    pub agent: Option<AgentInfo>,
    /// Live workers of an on-demand position.
    pub workers: Vec<WorkerInfo>,
    pub status: PositionStatus,
    /// Why it is unavailable, and similar notes.
    pub status_detail: Option<String>,
    /// A persistent agent's running or waiting task.
    pub current_task: Option<TaskBrief>,
    pub counts: WorkCounts,
    pub history: PositionHistory,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub archived_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OversightInfo {
    pub id: String,
    pub role: OversightRole,
    pub overseer_id: String,
    /// The lead of the team overseen.
    pub target_id: String,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgStats {
    pub departments: u32,
    pub projects: u32,
    pub positions: u32,
    /// Persistent positions with an agent.
    pub staffed: u32,
    pub vacant: u32,
    /// Workers spawned for tasks, still in the active workforce.
    pub active_workers: u32,
    pub working: u32,
    pub waiting: u32,
    pub queued: u32,
    pub completed_24h: u32,
    pub failed_24h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeBrief {
    pub id: String,
    pub label: String,
    pub ready: bool,
}

/// The whole organization for the canvas and directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgSnapshot {
    pub name: String,
    /// What the app calls the ranks (the owner's choice).
    pub titles: TitleTheme,
    pub roles: Vec<RoleInfo>,
    pub departments: Vec<DepartmentInfo>,
    pub projects: Vec<ProjectInfo>,
    /// Active positions first (in tree order), then archived ones.
    pub positions: Vec<PositionInfo>,
    pub oversight: Vec<OversightInfo>,
    pub stats: OrgStats,
    pub runtimes: Vec<RuntimeBrief>,
    pub notices: Vec<String>,
    #[ts(type = "number")]
    pub generated_at: u64,
}

/// The work a position owns: its own tasks, and its team's unfinished tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkView {
    /// `null` for the whole organization.
    pub position_id: Option<String>,
    pub running: Vec<TaskBrief>,
    pub waiting: Vec<TaskBrief>,
    pub queued: Vec<TaskBrief>,
    /// Finished tasks, newest first.
    pub recent: Vec<TaskBrief>,
    /// Unfinished tasks of the positions below it (for a lead).
    pub team: Vec<TaskBrief>,
}

// ---- Inputs ------------------------------------------------------------------------------

/// Hire a new position (and, for a persistent one, its agent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct HireInput {
    pub role_id: String,
    pub title: String,
    /// The supervisor; `null` reports to the owner.
    pub reports_to: Option<String>,
    /// A fixed AI tool; absent: automatic (the role's model policy picks).
    #[ts(optional)]
    pub runtime_id: Option<String>,
    /// A fixed model (only with a fixed AI tool).
    #[ts(optional)]
    pub model: Option<String>,
    /// Leave a persistent position vacant (hire later).
    #[ts(optional)]
    pub vacant: Option<bool>,
}

/// The head of a new department, or the coordinator of a new project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LeadInput {
    pub role_id: String,
    pub title: String,
    /// A fixed AI tool; absent: automatic.
    #[ts(optional)]
    pub runtime_id: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub vacant: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DepartmentInput {
    pub name: String,
    pub description: String,
    /// Required when creating a department: its head.
    #[ts(optional)]
    pub head: Option<LeadInput>,
    /// The head's supervisor (a superintendent); `null` reports to the owner.
    #[ts(optional)]
    pub reports_to: Option<String>,
    /// Updates only.
    #[ts(optional)]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ProjectInput {
    pub name: String,
    pub description: String,
    #[ts(optional)]
    pub repository_url: Option<String>,
    #[ts(optional)]
    pub local_path: Option<String>,
    pub allowed_runtimes: Vec<String>,
    #[ts(optional)]
    pub capability_profile: Option<String>,
    /// Creating only: the department and the coordinator.
    #[ts(optional)]
    pub department_id: Option<String>,
    #[ts(optional)]
    pub coordinator: Option<LeadInput>,
}

/// Changes to a position; absent fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PositionPatchInput {
    #[ts(optional)]
    pub title: Option<String>,
    /// A fixed AI tool; an empty string makes the position automatic.
    #[ts(optional)]
    pub runtime_id: Option<String>,
    /// An empty string clears the model.
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RoleInput {
    pub name: String,
    pub description: String,
    pub kind: PositionKind,
    pub staffing: Staffing,
}
