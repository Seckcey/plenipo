//! Liaison DTOs shared with the frontend (camelCase on the wire). Provider names never appear
//! in types: runtimes are data values.

use plenipo_ledger::{Task, TaskState};
use plenipo_runtime::agent::TurnOutcome;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a handoff request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HandoffState {
    /// Accepted; its child task waits for a worker slot.
    Accepted,
    /// Its child worker is running (or itself waiting).
    Dispatched,
    /// The child finished; its reply is recorded.
    Answered,
    /// The requesting task ended first; the child was stopped.
    Cancelled,
    /// Refused by Liaison; the requester is told why.
    Rejected,
}

/// Where a reply stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReplyState {
    /// Waiting to be delivered with the requester's other replies.
    Pending,
    /// Delivered: the requester continued with it.
    Delivered,
    /// Not delivered: the requester had stopped waiting.
    Discarded,
}

/// How a handoff ended, from the requester's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HandoffOutcome {
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    UsageLimited,
    AuthRequired,
    BillingNotAllowed,
    ProviderUnavailable,
    MalformedOutput,
    Crashed,
    Interrupted,
    /// Liaison refused the request.
    Rejected,
}

impl From<TurnOutcome> for HandoffOutcome {
    fn from(o: TurnOutcome) -> Self {
        match o {
            TurnOutcome::Completed => Self::Completed,
            TurnOutcome::Failed => Self::Failed,
            TurnOutcome::Cancelled => Self::Cancelled,
            TurnOutcome::TimedOut => Self::TimedOut,
            TurnOutcome::UsageLimited => Self::UsageLimited,
            TurnOutcome::AuthRequired => Self::AuthRequired,
            TurnOutcome::BillingNotAllowed => Self::BillingNotAllowed,
            TurnOutcome::ProviderUnavailable => Self::ProviderUnavailable,
            TurnOutcome::MalformedOutput => Self::MalformedOutput,
            TurnOutcome::Crashed => Self::Crashed,
            TurnOutcome::Interrupted => Self::Interrupted,
        }
    }
}

impl HandoffOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timedOut",
            Self::UsageLimited => "usageLimited",
            Self::AuthRequired => "authRequired",
            Self::BillingNotAllowed => "billingNotAllowed",
            Self::ProviderUnavailable => "providerUnavailable",
            Self::MalformedOutput => "malformedOutput",
            Self::Crashed => "crashed",
            Self::Interrupted => "interrupted",
            Self::Rejected => "rejected",
        }
    }
}

/// One piece of context passed with a request (content is not repeated here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextSummary {
    /// `answer`, `excerpt`, or `task`.
    pub kind: String,
    pub title: String,
    pub chars: u32,
}

/// A reply as the requester receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReplyView {
    pub message_id: String,
    pub state: ReplyState,
    pub outcome: HandoffOutcome,
    pub summary: String,
    /// The child's final answer (capped).
    pub text: Option<String>,
    pub error: Option<String>,
    /// Who answered: a worker session, or `liaison` for a refusal.
    pub source: String,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// One handoff request with its child and reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HandoffView {
    pub message_id: String,
    pub correlation_id: String,
    pub state: HandoffState,
    pub requester_task_id: String,
    /// The requester's address (`session:<id>`).
    pub requester: String,
    pub requester_runtime_id: Option<String>,
    /// The requester's step whose answer made the request.
    pub step: Option<u32>,
    /// The destination as written (`runtime:<id>`, `role:<name>`, …).
    pub destination: String,
    pub destination_label: String,
    pub objective: String,
    pub acceptance_criteria: String,
    pub priority: u8,
    /// The child's depth in the workflow (the owner's task is 0).
    pub depth: u32,
    pub context: Vec<ContextSummary>,
    pub artifacts: Vec<String>,
    pub capabilities_requested: Vec<String>,
    /// Why Liaison refused it.
    pub rejection: Option<String>,
    pub child_task_id: Option<String>,
    pub child_session_id: Option<String>,
    pub child_state: Option<TaskState>,
    pub reply: Option<ReplyView>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
}

/// A task's handoffs: the one that created it (if any) and those it made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskHandoffs {
    pub task_id: String,
    pub correlation_id: Option<String>,
    pub depth: Option<u32>,
    pub received: Option<HandoffView>,
    pub sent: Vec<HandoffView>,
}

/// How a task in a tree came to be, when a handoff created it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HandoffBrief {
    pub message_id: String,
    pub state: HandoffState,
    pub destination_label: String,
    pub reply_outcome: Option<HandoffOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskTreeNode {
    pub task: Task,
    /// 0 for the root.
    pub depth: u32,
    /// The runtime that worked on the task, when an agent did.
    pub runtime_id: Option<String>,
    pub runtime_label: Option<String>,
    pub session_id: Option<String>,
    pub handoff: Option<HandoffBrief>,
}

/// A task's whole delegation tree (from its root), depth-first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskTree {
    pub root_id: String,
    /// The task the tree was asked for.
    pub focus_id: String,
    pub correlation_id: Option<String>,
    pub nodes: Vec<TaskTreeNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiaisonLimits {
    pub max_depth: u32,
    pub max_requests_per_answer: u32,
    pub max_rounds: u32,
    pub max_workflow_handoffs: u32,
}

/// A worker a handoff can go to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DestinationInfo {
    /// How workers address it, e.g. `claude-code`.
    pub address: String,
    pub runtime_id: String,
    pub label: String,
    pub ready: bool,
}

/// Liaison's settings and state, for Settings and Diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiaisonOverview {
    pub protocol: String,
    pub context_format: String,
    pub limits: LiaisonLimits,
    pub destinations: Vec<DestinationInfo>,
    /// Requests accepted or running.
    pub open_handoffs: u32,
    pub notices: Vec<String>,
}
