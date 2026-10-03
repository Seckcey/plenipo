//! Workforce DTOs shared with the frontend (camelCase on the wire). Runtimes and providers are
//! data values; no vendor appears in a type.

use plenipo_ledger::{LoanUntil, TaskState, TilePlace};
use plenipo_router::{ModelFeature, ModelRule, RouteDecision};
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
    /// Its working instructions (ADR-019), written into every worker's instructions.
    pub job: RoleJob,
    /// Its specialties in use (ADR-042), by name.
    pub specialties: Vec<SpecialtyInfo>,
    /// Learning on for this role's agents unless turned off (ADR-041).
    pub learns: bool,
    /// Its lessons are kept without asking (ADR-024).
    pub learns_on_its_own: bool,
}

/// What a specialty suggests (ADR-042). Suggestions never change anything by themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct SpecialtySuggest {
    /// What a model should be able to do.
    pub needs: Vec<ModelFeature>,
    /// The smallest context size, in tokens.
    pub min_context_tokens: Option<u32>,
    /// Models from the owner's list, in order (the owner's own specialties only).
    pub models: Vec<String>,
    /// Permissions its work usually needs (the plan's names, e.g. `ssh.connect`).
    pub permissions: Vec<String>,
}

/// A specialty of a role (ADR-042).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpecialtyInfo {
    pub id: String,
    pub role_id: String,
    pub name: String,
    /// The title suggested for a new position with it.
    pub title: String,
    /// Plenipo's own (its lines cannot be changed).
    pub built_in: bool,
    /// The lines it adds to its role's working instructions.
    pub job: RoleJob,
    pub suggest: SpecialtySuggest,
}

/// The owner's specialty: a new one (with `roleId`) or changes to one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SpecialtyInput {
    /// Creating only: the role it belongs to.
    #[ts(optional)]
    pub role_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub job: RoleJob,
    #[serde(default)]
    pub suggest: SpecialtySuggest,
}

/// How much an agent has learned and done (ADR-045): 10 for each lesson it wrote that the owner
/// keeps, 1 for each task it finished.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExperienceInfo {
    pub score: u32,
    pub kept_lessons: u32,
    pub tasks_done: u32,
    /// Above the organization's average.
    pub experienced: bool,
    /// Its answers sent back because they didn't match Plenipo's record (Phase 25, item 4.8).
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(as = "Option<u32>", optional)]
    pub answers_sent_back: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` passes a reference
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Whether an agent learns, and which setting decided (ADR-041).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LearningInfo {
    pub learns: bool,
    pub from: crate::learning::LearningFrom,
    /// The agent's own setting (`None`: it follows its role).
    pub own: Option<bool>,
}

/// What an archived item was archived with (ADR-043).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArchivedWith {
    /// `project` or `department`.
    pub kind: String,
    pub id: String,
    pub name: String,
}

/// A role's working instructions (ADR-019): what it is responsible for, what it hands back,
/// what it must not do, and when it asks its lead for help. Plain words, one item per entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct RoleJob {
    pub duties: Vec<String>,
    pub returns: Vec<String>,
    pub limits: Vec<String>,
    pub ask_lead: Vec<String>,
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
    /// Archived with everything in it (ADR-043).
    #[ts(type = "number | null")]
    pub archived_at: Option<u64>,
    /// Deleted for good: a short record that still names it.
    pub deleted: bool,
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
    /// Each objective gets its own branch and working copy when the folder is a git repository
    /// (Phase 8).
    pub branch_per_objective: bool,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub archived_at: Option<u64>,
    /// Archived with its department (ADR-043).
    pub archived_with: Option<ArchivedWith>,
    /// Deleted for good: a short record that still names it.
    pub deleted: bool,
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
    /// Its specialty (ADR-042): its ID and name (the name stays when it was removed).
    pub specialty_id: Option<String>,
    pub specialty: Option<String>,
    /// How much it has learned and done (ADR-045).
    pub experience: ExperienceInfo,
    /// Whether it learns, and why (ADR-041).
    pub learning: LearningInfo,
    /// Its own model and effort rule (ADR-041), when it has one.
    pub own_rule: Option<ModelRule>,
    /// What it was archived with (ADR-043).
    pub archived_with: Option<ArchivedWith>,
    /// Deleted for good: a short record that still names it (ADR-043).
    pub deleted: bool,
    /// Moved to the owner's Workforce (ADR-045).
    pub in_workforce: bool,
    /// Lent to another team now (ADR-054).
    pub loan: Option<LoanInfo>,
}

/// Where a lent agent is helping (ADR-054).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoanInfo {
    /// The lead of the team it helps, and that lead's title ("Shop Supervisor").
    pub to_lead_id: String,
    pub to: String,
    /// That team's project, when it has one.
    pub project: Option<String>,
    pub until: LoanUntil,
    /// The objective it joined (its root task), once that team handed it work.
    pub objective_task_id: Option<String>,
    /// Sent home while working: it goes home when its task ends.
    pub going_home: bool,
    #[ts(type = "number")]
    pub since: u64,
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
    /// The AI company behind it ("Anthropic"), whose cloud its models run in (Phase 18: the
    /// canvas's "where" and its AI company filter; each model's own maker comes with Phase 16).
    pub company: String,
    /// Paid per use with the owner's key (ADR-085), not a subscription: the pickers list these
    /// after the subscriptions, and only once a key is saved (Phase 25, item 1.5).
    pub paid: bool,
}

/// The whole organization for the canvas and directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgSnapshot {
    pub name: String,
    /// What the app calls the ranks (the owner's choice).
    pub titles: TitleTheme,
    pub roles: Vec<RoleInfo>,
    pub departments: Vec<DepartmentInfo>,
    pub projects: Vec<ProjectInfo>,
    /// Active positions first (in tree order), then archived ones, then the short records of
    /// deleted ones (so older work still names them).
    pub positions: Vec<PositionInfo>,
    /// The agents the owner saved to hire again (ADR-045), most recent first.
    pub workforce: Vec<SavedAgentInfo>,
    /// The organization's average experience, over agents that finished a task (ADR-045).
    pub average_experience: u32,
    pub oversight: Vec<OversightInfo>,
    pub stats: OrgStats,
    pub runtimes: Vec<RuntimeBrief>,
    pub notices: Vec<String>,
    /// The tiles the owner placed by hand on the canvas (ADR-053); the rest follow the automatic
    /// layout.
    pub places: Vec<TilePlace>,
    /// The templates to start from (Phase 25, item 2.8).
    pub templates: Templates,
    #[ts(type = "number")]
    pub generated_at: u64,
}

/// A ready-made setup to start from (Phase 25, item 2.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    /// What it adds, one line each: "Development: Development Manager, with a Senior Developer,
    /// a Code Reviewer, …".
    pub adds: Vec<String>,
    /// More than one department: part of Pro (Free keeps one department).
    pub pro: bool,
}

/// The templates an organization and a department can start from (Phase 25, item 2.8).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Templates {
    pub organizations: Vec<TemplateInfo>,
    pub departments: Vec<TemplateInfo>,
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

/// An agent in the Workforce (ADR-045).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SavedAgentInfo {
    pub id: String,
    pub title: String,
    pub role_id: String,
    pub role_name: String,
    pub specialty_id: Option<String>,
    pub specialty: Option<String>,
    pub experience: ExperienceInfo,
    /// Where it worked (projects and departments), nearest first.
    pub places: Vec<String>,
    #[ts(type = "number | null")]
    pub first_worked: Option<u64>,
    #[ts(type = "number | null")]
    pub last_worked: Option<u64>,
    /// The lessons it wrote that the owner kept.
    pub lessons: Vec<String>,
    /// A fixed AI tool and model, when it had one.
    pub runtime_id: Option<String>,
    pub model: Option<String>,
    #[ts(type = "number")]
    pub saved_at: u64,
}

/// What deleting something for good would take along (ADR-043, ADR-045), for the confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeletionPreview {
    /// `position`, `project`, or `department`.
    pub kind: String,
    pub id: String,
    pub name: String,
    /// Every agent that goes, parents first.
    pub agents: Vec<DeletionAgent>,
    /// Projects that go (by name).
    pub projects: Vec<String>,
    /// Departments that go (by name).
    pub departments: Vec<String>,
    pub average_experience: u32,
}

/// An agent a deletion takes along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeletionAgent {
    pub position_id: String,
    pub title: String,
    pub role_name: String,
    pub experience: ExperienceInfo,
}

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
    /// One of the role's specialties (ADR-042).
    #[serde(default)]
    #[ts(optional)]
    pub specialty_id: Option<String>,
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
    /// An agent from the owner's Workforce with this role (ADR-045): its title, AI settings,
    /// specialty, and experience come with it.
    #[serde(default)]
    #[ts(optional)]
    pub from_workforce: Option<String>,
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
    /// A branch and working copy per objective (Phase 8); absent: on for a new project,
    /// unchanged for an existing one.
    #[ts(optional)]
    pub branch_per_objective: Option<bool>,
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
    /// One of its role's specialties; an empty string clears it (ADR-042).
    #[serde(default)]
    #[ts(optional)]
    pub specialty_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RoleInput {
    pub name: String,
    pub description: String,
    pub kind: PositionKind,
    pub staffing: Staffing,
    /// What the role does, in the owner's words (ADR-019).
    #[serde(default)]
    #[ts(optional)]
    pub job: Option<RoleJob>,
}

/// A change to a role the owner created (built-in roles keep their instructions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RoleUpdate {
    pub name: String,
    pub description: String,
    pub job: RoleJob,
}

/// One of the owner's orders in a position's chain of command (ADR-202), as its chat's Tasks list
/// shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainOrder {
    /// The task it started: the turn of the conversation that took it.
    pub task_id: String,
    /// When the owner gave it.
    #[ts(type = "number")]
    pub at: u64,
    /// Who it is for.
    pub position_id: String,
    pub position: String,
    /// The lead whose conversation took it, for an on-call position's order.
    pub via: Option<String>,
    /// The owner's words.
    pub words: String,
    /// The leads it went past, nearest first: each was told.
    pub leads: Vec<String>,
    /// This position's part in it.
    pub part: ChainPart,
    pub standing: ChainStanding,
    /// What came back up to this position, once the work ended.
    pub result: Option<String>,
    #[ts(type = "number | null")]
    pub reported_at: Option<u64>,
}

/// A position's part in one of the owner's orders (ADR-202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChainPart {
    /// It was given the order.
    Doer,
    /// Its conversation took the order for an on-call position on its team.
    Via,
    /// The order went past it, and it was told.
    Told,
}

/// Where the work of one of the owner's orders stands (ADR-202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChainStanding {
    Waiting,
    Working,
    Done,
    Failed,
    Stopped,
}

/// One of a project's objectives, in brief (Phase 8: the Projects page).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObjectiveBrief {
    pub root_task_id: String,
    pub objective: String,
    /// Who was given it.
    pub position_title: Option<String>,
    pub state: plenipo_ledger::TaskState,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub completed_at: Option<u64>,
    /// Tasks in its tree (its own included), those still going, and those that failed.
    pub tasks: u32,
    pub active: u32,
    pub failed: u32,
    pub waiting_approvals: u32,
    /// Its branch, when it has a working copy.
    pub branch: Option<String>,
    /// The project it touched (Phase 12).
    pub project_id: Option<String>,
    /// The first part of the answer of whoever was given it, once there is one (Phase 12).
    pub answer: Option<String>,
}

/// Home (Phase 12): objectives still going, those finished in the last week with their
/// answers, and what is stuck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HomeView {
    /// The newest 50, newest first.
    pub current: Vec<ObjectiveBrief>,
    /// The last 20 to finish in the last week, the last to finish first.
    pub finished: Vec<ObjectiveBrief>,
    /// The newest problem of each piece of work still in trouble (the last week), newest first.
    pub stuck: Vec<StuckItem>,
    /// How many objectives are going (all of them, not only those listed).
    pub going: u32,
    /// How many objectives finished in the last 24 hours (all of them).
    pub finished_day: u32,
}

/// Something stuck (Home): its newest problem, and the task it belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StuckItem {
    pub event: plenipo_ledger::LedgerEvent,
    /// Its task, in brief (a server whose ID changed has none).
    pub task: Option<TaskBrief>,
}

/// A project's objectives and working copies (Phase 8: the Projects page).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectWork {
    pub project_id: String,
    /// Newest first.
    pub objectives: Vec<ObjectiveBrief>,
    /// Newest first.
    pub working_copies: Vec<plenipo_ledger::Workspace>,
}

/// Set up a software project in the Development department (Phase 8): the department and its VP
/// when missing, then the project with its supervisor and the standard team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DevelopmentInput {
    /// The project (its department and supervisor come from the template).
    pub project: ProjectInput,
    /// The AI tool of the VP and the supervisor; absent: automatic. The team is always
    /// automatic (each role's model choices pick).
    #[ts(optional)]
    pub runtime_id: Option<String>,
    /// The department the project joins (Phase 25, item 2.7); absent: the template's own,
    /// made with its VP when there is none.
    #[serde(default)]
    #[ts(optional)]
    pub department_id: Option<String>,
    /// Jobs (by title) to hire a new worker for even when the department has one for them.
    /// Every other job uses a matching worker the department already has (Phase 25, item 2.7).
    #[serde(default)]
    #[ts(optional)]
    pub hire_new: Option<Vec<String>>,
}

/// Work an AI tool's usage limit stopped, for one AI tool (Phase 25, item 4.2; ADR-253): the
/// notice that says when Plenipo picks it back up, and the owner's choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitWait {
    pub runtime_id: String,
    /// Its name on screen ("Claude Code").
    pub label: String,
    /// The company whose plan ran out, when it gives usage resets ("Anthropic", "OpenAI"):
    /// **Use a reset** opens its own page. Plenipo never uses one for the owner.
    pub reset_company: Option<String>,
    /// When the limit was reached.
    #[ts(type = "number")]
    pub since: u64,
    /// When Plenipo picks the work back up: the reset time the AI tool reported, or an hour
    /// after the limit when it reported none. `null`: the limit is over; it is picked up now.
    #[ts(type = "number | null")]
    pub until: Option<u64>,
    /// `until` is the reset time the AI tool reported (not Plenipo's hour).
    pub reported: bool,
    /// The work waiting, oldest first.
    pub work: Vec<LimitWaitWork>,
}

/// One objective a usage limit stopped (Phase 25, item 4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitWaitWork {
    pub task_id: String,
    /// The objective, as the owner gave it.
    pub objective: String,
    /// Who was doing it (a position's title), when it was a member of the organization.
    pub who: Option<String>,
}
