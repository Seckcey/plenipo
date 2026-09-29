//! Organization-aware destinations (Phase 5, ADR-009).
//!
//! Liaison itself knows only runtimes. When the Workforce engine installs a [`Directory`], a
//! worker that is a member of the organization — its session and tasks carry a `workforce`
//! record — is told who it is and which team members it may hand work to (`role:<name>`), and
//! its requests are placed on those members instead of on raw runtimes. Sessions outside the
//! organization behave as before.

use plenipo_ledger::{ChildConversation, NewWorker, Task};
use plenipo_runtime::agent::{Effort, SessionStart, WorkDoneBy};
use serde_json::Value;

use crate::context::Destination;

/// Where a task delegated to a full-time member runs: the member's own conversation (Phase 8,
/// ADR-016).
#[derive(Debug, Clone, PartialEq)]
pub struct MemberConversation {
    pub conversation: ChildConversation,
    /// Set when the member has no open conversation yet: the session to start, with the same
    /// ID as `conversation` and metadata naming the member.
    pub start: Option<SessionStart>,
}

/// Who a member is and whom it may address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    /// A short description of the worker (position, role, project) for its instructions.
    pub identity: String,
    /// Who a full-time member is, in one line ("You are Website Supervisor, the Supervisor of
    /// the Website project in Acme."), for the short reminder that stands in for its
    /// instructions once its conversation has them (ADR-044). `None`: the reminder says only
    /// that its instructions still apply.
    pub reminder: Option<String>,
    /// The team members it may hand work to (`role:<title>` addresses).
    pub members: Vec<Destination>,
}

/// Where an accepted request to a team member goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// The canonical address recorded as the request's destination, e.g. `role:QA Engineer`.
    pub address: String,
    /// How the destination is shown, e.g. "QA Engineer (Claude Code)".
    pub label: String,
    pub runtime_id: String,
    pub model: Option<String>,
    /// The effort level the worker runs at (`None`: the runtime's default).
    pub effort: Option<Effort>,
    /// The worker recorded with the child task, in the same transaction; `None` when the task
    /// goes to a full-time member, which does it in its own conversation (ADR-016).
    pub worker: Option<NewWorker>,
    /// For a full-time member: the conversation expected to run the task (checked again when
    /// the task is dispatched, see [`Directory::conversation`]).
    pub conversation: Option<ChildConversation>,
    /// The child's `workforce` record (its task's and its session's metadata). It must name
    /// `worker` (`agentId`, `positionId`), and may say why its runtime and model were chosen
    /// (`routing`). A full-time member's record names the member and has `"fullTime": true`.
    pub workforce: Value,
    /// Who the child worker is, for its instructions.
    pub identity: String,
    /// The project the child task belongs to.
    pub project_id: Option<String>,
}

/// Supplied by the Workforce engine. Calls may read the Ledger (they run on blocking threads).
pub trait Directory: Send + Sync + 'static {
    /// The team of the member described by `workforce` (a session's or task's `workforce`
    /// record), or `None` when it is not a member of the organization.
    fn team(&self, workforce: &Value) -> Option<Team>;

    /// Place a request for `role:<name>` made by the member described by `workforce` while
    /// working on `requester`. `reviewed` is the work the request is about (the tasks it
    /// references, or else the requester's own), each as its AI tool and model, for
    /// cross-company review by who made the models (Phase 6, ADR-081). `Err` is the refusal
    /// reason, which the requester is told.
    fn place(
        &self,
        workforce: &Value,
        requester: &Task,
        name: &str,
        reviewed: &[WorkDoneBy],
    ) -> Result<Placement, String>;

    /// The conversation that runs a task delegated to the full-time member described by
    /// `workforce` (a child task's record with `"fullTime": true`): its open conversation, or a
    /// new one to start. `Err` is why the member cannot take the task now (vacant, archived,
    /// its AI tool not allowed), which fails the task with that reason.
    fn conversation(&self, workforce: &Value) -> Result<MemberConversation, String> {
        let _ = workforce;
        Err("this organization does not hand work to full-time members".into())
    }
}

/// True when a child task's `workforce` record sends it to a full-time member's own
/// conversation.
pub fn is_full_time(workforce: &Value) -> bool {
    workforce["fullTime"].as_bool() == Some(true)
}
