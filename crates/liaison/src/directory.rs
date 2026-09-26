//! Organization-aware destinations (Phase 5, ADR-009).
//!
//! Liaison itself knows only runtimes. When the Workforce engine installs a [`Directory`], a
//! worker that is a member of the organization — its session and tasks carry a `workforce`
//! record — is told who it is and which team members it may hand work to (`role:<name>`), and
//! its requests are placed on those members instead of on raw runtimes. Sessions outside the
//! organization behave as before.

use plenipo_ledger::{NewWorker, Task};
use serde_json::Value;

use crate::context::Destination;

/// Who a member is and whom it may address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    /// A short description of the worker (position, role, project) for its instructions.
    pub identity: String,
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
    /// The worker recorded with the child task, in the same transaction.
    pub worker: NewWorker,
    /// The child's `workforce` record (its task's and its session's metadata). It must name
    /// `worker` (`agentId`, `positionId`).
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
    /// working on `requester`. `Err` is the refusal reason, which the requester is told.
    fn place(&self, workforce: &Value, requester: &Task, name: &str) -> Result<Placement, String>;
}
