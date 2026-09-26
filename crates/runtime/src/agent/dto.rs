//! Provider-neutral agent runtime DTOs shared with the frontend (camelCase on the wire).
//!
//! Nothing here names a vendor: runtimes and providers are identified by data values
//! (`runtimeId`, `provider`) that only the adapters define.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dto::TokenUsage;

/// Whether a runtime's CLI was found and runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallState {
    /// Detection has not finished yet.
    Checking,
    Installed,
    NotInstalled,
    /// Found, but in a form Plenipo will not run (e.g. a Windows npm `.cmd` shim).
    Unsupported,
    /// Found, but it failed to report its version.
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Installation {
    pub state: InstallState,
    pub executable: Option<String>,
    pub version: Option<String>,
    /// Explanation when not installed/unsupported/broken.
    pub detail: Option<String>,
}

/// Sign-in state as reported by the runtime's own status command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuthState {
    /// Not checked yet.
    Checking,
    /// Signed in with a subscription account (the only billing Plenipo uses by default).
    Subscription,
    /// Signed in, but the billing method was not recognized.
    Unverified,
    /// Signed in with an API key: usage would be billed to the API. Refused.
    ApiKey,
    /// Routed through a third-party cloud provider. Refused.
    ThirdPartyCloud,
    SignedOut,
    /// The status command failed or is not supported by this version.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthStatus {
    pub state: AuthState,
    /// Sign-in method as a short label (never an account identifier).
    pub method: Option<String>,
    pub detail: Option<String>,
}

/// What a runtime supports through its adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeCapabilities {
    /// Text arrives incrementally while the agent writes it.
    pub streaming_text: bool,
    pub resume: bool,
    pub cancel: bool,
    /// The runtime reports a structured final result (not just text on stdout).
    pub structured_results: bool,
    /// Checks its reported credential source during every turn.
    pub billing_checked_per_turn: bool,
    /// Human-readable description of what the agent may do in this phase.
    pub tool_posture: String,
}

/// Provider diagnostics for one runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentRuntimeInfo {
    /// Stable adapter ID, e.g. `claude-code`.
    pub id: String,
    pub label: String,
    pub provider: String,
    pub provider_label: String,
    pub installation: Installation,
    pub auth: AuthStatus,
    pub capabilities: RuntimeCapabilities,
    pub install_hint: String,
    pub login_hint: String,
    /// Installed and signed in with an allowed method: tasks can start.
    pub ready: bool,
    #[ts(type = "number | null")]
    pub checked_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeLevel {
    Info,
    Warning,
}

/// One normalized piece of agent activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AgentEvent {
    /// The provider confirmed its session and (when reported) the model.
    SessionStarted {
        provider_session_id: Option<String>,
        model: Option<String>,
    },
    /// Incremental text (live view only; not stored).
    TextDelta {
        text: String,
    },
    /// A complete assistant message.
    Message {
        text: String,
    },
    /// Reasoning summary, when the provider shares one.
    Reasoning {
        text: String,
    },
    ToolUse {
        tool: String,
        summary: String,
    },
    ToolResult {
        tool: Option<String>,
        is_error: bool,
        summary: String,
    },
    Notice {
        level: NoticeLevel,
        text: String,
    },
    Usage {
        usage: TokenUsage,
    },
}

impl AgentEvent {
    /// Ledger event type for activity worth keeping, `None` for live-only activity.
    pub fn ledger_type(&self) -> Option<&'static str> {
        match self {
            Self::SessionStarted { .. } => Some("agent.session_bound"),
            Self::Message { .. } => Some("agent.message"),
            Self::ToolUse { .. } => Some("agent.tool_use"),
            Self::ToolResult { .. } => Some("agent.tool_result"),
            Self::Notice { .. } => Some("agent.notice"),
            Self::TextDelta { .. } | Self::Reasoning { .. } | Self::Usage { .. } => None,
        }
    }
}

/// How a turn ended, normalized across providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TurnOutcome {
    Completed,
    /// The provider reported an error not covered below.
    Failed,
    Cancelled,
    TimedOut,
    /// A subscription usage or rate limit was reached. Plenipo never switches provider.
    UsageLimited,
    /// Not signed in, or the sign-in expired.
    AuthRequired,
    /// The runtime would have billed an API key or third-party cloud.
    BillingNotAllowed,
    /// The CLI is missing, could not start, or the provider was unreachable.
    ProviderUnavailable,
    /// The output could not be understood.
    MalformedOutput,
    /// The process ended without reporting a result.
    Crashed,
    /// Plenipo stopped while the turn was running.
    Interrupted,
}

impl TurnOutcome {
    pub fn is_success(self) -> bool {
        self == Self::Completed
    }
}

/// Normalized result of one turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnResult {
    pub outcome: TurnOutcome,
    /// One human-readable line.
    pub summary: String,
    /// The agent's final answer (capped).
    pub text: Option<String>,
    /// Provider or process error detail (capped).
    pub error: Option<String>,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    /// Output lines that were not understood (malformed or unknown event types).
    pub ignored_lines: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SessionState {
    Open,
    Closed,
}

/// A conversation with one runtime, which can span several turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSession {
    /// Plenipo's session ID (UUID v4).
    pub id: String,
    pub runtime_id: String,
    pub provider: String,
    /// The provider's own session/thread ID.
    pub provider_session_id: Option<String>,
    /// The provider has reported `provider_session_id`, so it can be resumed.
    pub provider_session_confirmed: bool,
    pub model: Option<String>,
    pub title: String,
    pub state: SessionState,
    pub working_dir: String,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    pub turn_count: u32,
    /// Task ID of the turn currently running, if any.
    pub active_task_id: Option<String>,
    /// Task ID of a turn waiting to be continued (for example for handoff replies), if any.
    /// The session takes no other turn meanwhile.
    pub waiting_task_id: Option<String>,
    /// Settings stored with the session by the component that started it (for example
    /// Plenipo Liaison). Opaque to the runtime.
    #[ts(type = "Record<string, unknown>")]
    pub metadata: serde_json::Value,
}

/// One turn: an objective given to the session, recorded as a Ledger task. A turn normally
/// runs one step; a turn that waited (for example for handoff replies, ADR-008) continues with
/// further steps in the same provider session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentTurn {
    pub task_id: String,
    pub session_id: String,
    pub number: u32,
    pub objective: String,
    /// The latest step's execution.
    pub execution_id: Option<String>,
    /// A step is running.
    pub running: bool,
    /// Waiting to be continued; no step is running.
    pub waiting: bool,
    /// The final result, once the turn has finished.
    pub result: Option<TurnResult>,
    /// Steps in order: finished ones with their results, then the running one.
    pub steps: Vec<TurnStep>,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub ended_at: Option<u64>,
}

/// One run of a turn's runtime process: the objective, or a continuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnStep {
    /// 1 for the objective, 2+ for continuations.
    pub number: u32,
    pub execution_id: Option<String>,
    pub running: bool,
    pub result: Option<TurnResult>,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
    #[ts(type = "number | null")]
    pub ended_at: Option<u64>,
}

/// Live activity for one turn. `seq` increases per turn: step `n` numbers its activity from
/// `(n - 1) * STEP_SEQ + 1` ([`crate::agent::STEP_SEQ`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentActivity {
    pub session_id: String,
    pub task_id: String,
    #[ts(type = "number")]
    pub seq: u64,
    #[ts(type = "number")]
    pub ts: u64,
    pub event: AgentEvent,
}

/// Snapshot of the agent runtimes and sessions (rebuilds the UI after a reload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentOverview {
    pub runtimes: Vec<AgentRuntimeInfo>,
    /// Newest first.
    pub sessions: Vec<AgentSession>,
    pub notices: Vec<String>,
}

/// One session with its turns (oldest first) and buffered live activity of recent turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSessionDetail {
    pub session: AgentSession,
    pub turns: Vec<AgentTurn>,
    pub activity: Vec<AgentActivity>,
}

/// Update streamed from the agent runtime to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AgentUpdate {
    Activity(AgentActivity),
    Turn(AgentTurn),
    Session(AgentSession),
    Runtimes(RuntimesUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimesUpdate {
    pub runtimes: Vec<AgentRuntimeInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_event_wire_format() {
        let event = AgentEvent::SessionStarted {
            provider_session_id: Some("p1".into()),
            model: None,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "sessionStarted", "providerSessionId": "p1", "model": null })
        );
        let event = AgentEvent::ToolResult {
            tool: None,
            is_error: true,
            summary: "x".into(),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "toolResult", "tool": null, "isError": true, "summary": "x" })
        );
        assert_eq!(
            serde_json::to_value(TurnOutcome::BillingNotAllowed).unwrap(),
            json!("billingNotAllowed")
        );
    }

    #[test]
    fn only_durable_activity_has_a_ledger_type() {
        let text = |t: &str| t.to_owned();
        assert_eq!(
            AgentEvent::TextDelta { text: text("a") }.ledger_type(),
            None
        );
        assert_eq!(
            AgentEvent::Message { text: text("a") }.ledger_type(),
            Some("agent.message")
        );
        assert_eq!(
            AgentEvent::Usage {
                usage: TokenUsage::default()
            }
            .ledger_type(),
            None
        );
    }
}
