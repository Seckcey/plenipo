//! Serializable runtime DTOs shared with the frontend (camelCase on the wire).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lifecycle state of one execution.
///
/// ```text
/// starting ──▶ running ──▶ succeeded | failed | cancelled | timedOut
///     │            └─────▶ interrupted   (found running after Plenipo restarted)
///     └──▶ failed | cancelled | interrupted
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExecutionState {
    Starting,
    Running,
    /// Exited with code 0.
    Succeeded,
    /// Non-zero exit, killed by a signal, or failed to start.
    Failed,
    Cancelled,
    TimedOut,
    /// Plenipo stopped while this execution was active; final outcome unknown.
    Interrupted,
}

impl ExecutionState {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Starting | Self::Running)
    }

    /// Whether moving from `self` to `next` is a legal lifecycle transition.
    pub fn can_transition_to(self, next: Self) -> bool {
        use ExecutionState::*;
        matches!(
            (self, next),
            (Starting, Running | Failed | Cancelled | Interrupted)
                | (
                    Running,
                    Succeeded | Failed | Cancelled | TimedOut | Interrupted
                )
        )
    }
}

/// Durable metadata about one launch. Never contains output or environment values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecutionRecord {
    /// Unique per launch (UUID v4).
    pub id: String,
    pub profile_id: String,
    pub label: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub pid: Option<u32>,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    /// Human-readable explanation of the final state, when useful.
    pub detail: Option<String>,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub ended_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// One line of child output. `seq` is monotonic per execution across both streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputLine {
    #[ts(type = "number")]
    pub seq: u64,
    pub stream: OutputStream,
    pub text: String,
    /// The original line exceeded the per-line limit and was cut.
    pub truncated: bool,
    #[ts(type = "number")]
    pub ts: u64,
}

/// A batch of new output lines for one execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputBatch {
    pub execution_id: String,
    pub lines: Vec<OutputLine>,
}

/// The execution's record changed (started, running, or reached a final state).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LifecycleEvent {
    pub record: ExecutionRecord,
}

/// Event streamed from the runtime to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RuntimeEvent {
    Output(OutputBatch),
    Lifecycle(LifecycleEvent),
}

/// What the UI may launch. Deliberately excludes executable, args, and environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchProfileInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    #[ts(type = "number")]
    pub max_runtime_secs: u64,
}

/// Buffered output of one execution (most recent lines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecutionOutput {
    pub execution_id: String,
    pub lines: Vec<OutputLine>,
    /// Older lines evicted from the bounded buffer.
    #[ts(type = "number")]
    pub dropped: u64,
    /// False when output was not retained (e.g. from a previous Plenipo session).
    pub available: bool,
}

/// Snapshot used to (re)build the runtime UI, e.g. after the webview reloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeOverview {
    pub profiles: Vec<LaunchProfileInfo>,
    /// Newest first.
    pub executions: Vec<ExecutionRecord>,
    pub active_count: u32,
    /// Recovery and configuration notices that the user should see.
    pub notices: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use ExecutionState::*;

    const ALL: [ExecutionState; 7] = [
        Starting,
        Running,
        Succeeded,
        Failed,
        Cancelled,
        TimedOut,
        Interrupted,
    ];

    #[test]
    fn legal_transitions() {
        for (from, to) in [
            (Starting, Running),
            (Starting, Failed),
            (Starting, Cancelled),
            (Starting, Interrupted),
            (Running, Succeeded),
            (Running, Failed),
            (Running, Cancelled),
            (Running, TimedOut),
            (Running, Interrupted),
        ] {
            assert!(from.can_transition_to(to), "{from:?} -> {to:?}");
        }
    }

    #[test]
    fn terminal_states_never_transition() {
        for from in ALL.into_iter().filter(|s| s.is_terminal()) {
            for to in ALL {
                assert!(!from.can_transition_to(to), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn illegal_non_terminal_transitions() {
        assert!(!Starting.can_transition_to(Starting));
        assert!(!Starting.can_transition_to(Succeeded));
        assert!(!Starting.can_transition_to(TimedOut));
        assert!(!Running.can_transition_to(Starting));
        assert!(!Running.can_transition_to(Running));
    }

    #[test]
    fn runtime_event_wire_format() {
        let event = RuntimeEvent::Output(OutputBatch {
            execution_id: "e1".into(),
            lines: vec![OutputLine {
                seq: 1,
                stream: OutputStream::Stderr,
                text: "hi".into(),
                truncated: false,
                ts: 5,
            }],
        });
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "kind": "output",
                "executionId": "e1",
                "lines": [{ "seq": 1, "stream": "stderr", "text": "hi", "truncated": false, "ts": 5 }]
            })
        );
        assert_eq!(serde_json::to_value(TimedOut).unwrap(), json!("timedOut"));
    }
}
