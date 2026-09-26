//! Capability broker DTOs shared with the frontend (camelCase on the wire).

use plenipo_guard::{Capability, GuardSettings, Level, Risk, SensitiveKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where an approval stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl From<plenipo_ledger::ApprovalState> for ApprovalStatus {
    fn from(s: plenipo_ledger::ApprovalState) -> Self {
        match s {
            plenipo_ledger::ApprovalState::Pending => Self::Pending,
            plenipo_ledger::ApprovalState::Approved => Self::Approved,
            plenipo_ledger::ApprovalState::Rejected => Self::Rejected,
            plenipo_ledger::ApprovalState::Expired => Self::Expired,
        }
    }
}

/// An approval card: what a worker wants to do, why it needs the owner, and the outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalView {
    pub id: String,
    pub task_id: String,
    pub status: ApprovalStatus,
    #[ts(type = "number")]
    pub requested_at: u64,
    #[ts(type = "number | null")]
    pub expires_at: Option<u64>,
    #[ts(type = "number | null")]
    pub resolved_at: Option<u64>,
    #[ts(optional)]
    pub resolved_by: Option<String>,
    /// The worker's title ("Backend Developer").
    pub worker: String,
    pub role: String,
    #[ts(optional)]
    pub project: Option<String>,
    #[ts(optional)]
    pub folder: Option<String>,
    #[ts(optional)]
    pub capability: Option<Capability>,
    /// "Run programs".
    pub capability_label: String,
    /// What it wants to do, in a line ("Run npm publish").
    pub summary: String,
    /// The exact action: a command line, a path, a script.
    pub detail: String,
    /// Why it needs the owner.
    pub reason: String,
    #[ts(optional)]
    pub risk: Option<Risk>,
    pub risk_label: String,
    #[ts(optional)]
    pub sensitive: Option<SensitiveKind>,
    #[ts(optional)]
    pub sensitive_label: Option<String>,
    #[ts(optional)]
    pub session_id: Option<String>,
    #[ts(optional)]
    pub grant_id: Option<String>,
    /// A worker is waiting for the answer right now.
    pub waiting: bool,
    /// The note recorded with the outcome.
    #[ts(optional)]
    pub note: Option<String>,
}

/// Pending approvals (oldest first) and recent outcomes (newest first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalQueue {
    pub pending: Vec<ApprovalView>,
    pub recent: Vec<ApprovalView>,
}

/// One permission a grant holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GrantPermission {
    pub capability: Capability,
    pub label: String,
    pub level: Level,
}

/// A worker's permissions in use now: one step of one task (the plan's runtime grant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GrantView {
    pub grant_id: String,
    pub task_id: String,
    pub session_id: String,
    pub step: u32,
    #[ts(optional)]
    pub position_id: Option<String>,
    pub worker: String,
    pub role: String,
    #[ts(optional)]
    pub project: Option<String>,
    #[ts(optional)]
    pub folder: Option<String>,
    pub permissions: Vec<GrantPermission>,
    #[ts(type = "number")]
    pub opened_at: u64,
    pub revoked: bool,
    /// Tool calls carried out, blocked, and sent to the owner.
    pub used: u32,
    pub blocked: u32,
    pub asked: u32,
}

/// A request Guard blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedView {
    #[ts(type = "number")]
    pub at: u64,
    #[ts(optional)]
    pub task_id: Option<String>,
    pub worker: String,
    pub capability_label: String,
    pub summary: String,
    pub reason: String,
}

/// The operating system's protected storage for secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VaultStatus {
    pub available: bool,
    /// "Windows Credential Manager".
    pub label: String,
    #[ts(optional)]
    pub detail: Option<String>,
    /// IDs of secrets whose value is stored.
    pub stored: Vec<String>,
}

/// Plenipo's tool server for workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsStatus {
    pub running: bool,
    pub detail: String,
}

/// Everything Settings → Permissions and the Approvals page show besides the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PermissionsSnapshot {
    pub settings: GuardSettings,
    pub vault: VaultStatus,
    pub tools: ToolsStatus,
    /// Workers using permissions now.
    pub grants: Vec<GrantView>,
    /// Recently blocked requests, newest first.
    pub blocked: Vec<BlockedView>,
    pub notices: Vec<String>,
}
