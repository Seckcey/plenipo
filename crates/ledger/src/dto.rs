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
#[derive(Debug, Clone, Default)]
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
