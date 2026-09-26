//! Creates the agent runtime service (Phase 3, ADR-007) for the desktop app: sessions and
//! turns are recorded in the Ledger, updates stream to the UI, and runtimes are detected in
//! the background at startup.

use std::sync::Arc;

use plenipo_ledger::{
    Ledger, NewEvent, NewRuntimeSession, NewTask, RuntimeSession, RuntimeSessionState, Task,
    TaskState,
};
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentEvent, AgentRuntime, AgentSession, AgentSink, AgentTurn,
    AgentUpdate, HostEnv, SessionChange, SessionState, SessionStore, TurnOutcome, TurnRef,
    TurnResult,
};
use plenipo_runtime::Supervisor;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying [`AgentUpdate`] payloads to the main window.
pub const AGENT_EVENT: &str = "plenipo://agents";

/// Actor recorded for actions taken by the person using the app.
const OWNER: &str = "owner";

struct TauriAgentSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AgentSink for TauriAgentSink<R> {
    fn emit(&self, update: AgentUpdate) {
        if let Err(e) = self.app.emit_to("main", AGENT_EVENT, &update) {
            eprintln!("[plenipo] failed to emit agent update: {e}");
        }
    }
}

/// Build the agent runtime. Never fails; with `InMemory` persistence (tests) it sees no
/// installed runtimes, so tests never start real CLIs.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    persistence: Persistence,
    ledger: Arc<Ledger>,
    supervisor: Supervisor,
) -> AgentRuntime {
    let (workspace_root, host) = match persistence {
        Persistence::AppData => (
            app.path()
                .app_local_data_dir()
                .map(|d| d.join("runtime").join("agent-workspaces"))
                .unwrap_or_else(|_| std::env::temp_dir().join("plenipo-agent-workspaces")),
            HostEnv::current(),
        ),
        Persistence::InMemory => (
            std::env::temp_dir().join("plenipo-agent-workspaces"),
            HostEnv::new(None, None, None),
        ),
    };
    AgentRuntime::new(
        AgentConfig::new(workspace_root),
        builtin_adapters(),
        supervisor,
        Arc::new(LedgerSessionStore(ledger)),
        Arc::new(TauriAgentSink { app: app.clone() }),
        host,
    )
}

/// Detect runtimes without delaying startup.
pub fn detect_in_background(runtime: &AgentRuntime) {
    let runtime = runtime.clone();
    tauri::async_runtime::spawn(async move {
        runtime.refresh().await;
    });
}

/// Sessions and turns in the Ledger: sessions in `runtime_sessions`; each turn is a task whose
/// metadata names the session, with its activity and final `agent.result` on the task trail.
pub struct LedgerSessionStore(pub Arc<Ledger>);

fn to_session(s: RuntimeSession) -> AgentSession {
    AgentSession {
        id: s.id,
        runtime_id: s.runtime,
        provider: s.provider,
        provider_session_id: s.provider_session_id,
        provider_session_confirmed: s.provider_session_confirmed,
        model: s.model,
        title: s.title,
        state: match s.state {
            RuntimeSessionState::Open => SessionState::Open,
            RuntimeSessionState::Closed => SessionState::Closed,
        },
        working_dir: s.working_dir,
        created_at: s.created_at,
        updated_at: s.updated_at,
        turn_count: s.turn_count,
        active_task_id: None,
    }
}

fn final_state(outcome: TurnOutcome) -> TaskState {
    match outcome {
        TurnOutcome::Completed => TaskState::Succeeded,
        TurnOutcome::Cancelled => TaskState::Cancelled,
        _ => TaskState::Failed,
    }
}

impl LedgerSessionStore {
    fn to_turn(&self, task: Task) -> Result<AgentTurn, String> {
        let session_id = task.metadata["sessionId"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let number = task.metadata["turn"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        let execution_id = self
            .0
            .executions_for_task(&task.id)
            .map_err(|e| e.to_string())?
            .pop()
            .map(|e| e.id);
        let result = if task.state.is_terminal() {
            self.0
                .events_for_task(&task.id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .rev()
                .find(|e| e.event_type == "agent.result")
                .and_then(|e| serde_json::from_value::<TurnResult>(e.payload).ok())
        } else {
            None
        };
        Ok(AgentTurn {
            task_id: task.id,
            session_id,
            number,
            objective: task.objective,
            execution_id,
            running: !task.state.is_terminal(),
            result,
            started_at: task.created_at,
            ended_at: task.completed_at,
        })
    }
}

impl SessionStore for LedgerSessionStore {
    fn sessions(&self, limit: usize) -> Result<Vec<AgentSession>, String> {
        let limit = u32::try_from(limit).unwrap_or(u32::MAX);
        self.0
            .list_runtime_sessions(limit)
            .map(|rows| rows.into_iter().map(to_session).collect())
            .map_err(|e| e.to_string())
    }

    fn session(&self, id: &str) -> Result<Option<AgentSession>, String> {
        self.0
            .runtime_session(id)
            .map(|s| s.map(to_session))
            .map_err(|e| e.to_string())
    }

    fn open_session(&self, session: &AgentSession) -> Result<(), String> {
        self.0
            .open_runtime_session(
                NewRuntimeSession {
                    id: session.id.clone(),
                    runtime: session.runtime_id.clone(),
                    provider: session.provider.clone(),
                    model: session.model.clone(),
                    title: session.title.clone(),
                    working_dir: session.working_dir.clone(),
                    metadata: Value::Null,
                },
                OWNER,
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn save_session(&self, session: &AgentSession, change: SessionChange) -> Result<(), String> {
        let actor = format!("agent:{}", session.runtime_id);
        let result = match change {
            SessionChange::Bound => self
                .0
                .bind_runtime_session(
                    &session.id,
                    session
                        .provider_session_id
                        .as_deref()
                        .filter(|_| session.provider_session_confirmed),
                    session.model.as_deref(),
                    &actor,
                )
                .map(|_| ()),
            SessionChange::Closed => self.0.close_runtime_session(&session.id, OWNER).map(|_| ()),
            SessionChange::Touched => self.0.touch_runtime_session(&session.id),
        };
        result.map_err(|e| e.to_string())
    }

    fn begin_turn(
        &self,
        session: &AgentSession,
        number: u32,
        objective: &str,
    ) -> Result<String, String> {
        let task = self
            .0
            .create_task(
                NewTask {
                    requested_by: OWNER.into(),
                    assigned_to: Some(session.runtime_id.clone()),
                    objective: objective.into(),
                    metadata: json!({
                        "sessionId": session.id,
                        "runtimeId": session.runtime_id,
                        "turn": number,
                    }),
                    ..NewTask::default()
                },
                OWNER,
            )
            .map_err(|e| e.to_string())?;
        self.0
            .transition_task(
                &task.id,
                TaskState::Running,
                &format!("agent:{}", session.runtime_id),
                Some("turn started"),
            )
            .map_err(|e| e.to_string())?;
        Ok(task.id)
    }

    fn record_activity(&self, turn: &TurnRef<'_>, event: &AgentEvent) -> Result<(), String> {
        let Some(event_type) = event.ledger_type() else {
            return Ok(());
        };
        self.0
            .append_event(NewEvent {
                task_id: Some(turn.task_id.into()),
                execution_id: turn.execution_id.map(str::to_owned),
                source: turn.actor.into(),
                event_type: event_type.into(),
                payload: serde_json::to_value(event).map_err(|e| e.to_string())?,
                ..NewEvent::default()
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn finish_turn(&self, turn: &TurnRef<'_>, result: &TurnResult) -> Result<(), String> {
        let mut payload = serde_json::to_value(result).map_err(|e| e.to_string())?;
        payload["sessionId"] = json!(turn.session_id);
        self.0
            .complete_task(
                turn.task_id,
                final_state(result.outcome),
                turn.actor,
                Some(&result.summary),
                NewEvent {
                    execution_id: turn.execution_id.map(str::to_owned),
                    source: turn.actor.into(),
                    event_type: "agent.result".into(),
                    payload,
                    ..NewEvent::default()
                },
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn turns(&self, session_id: &str) -> Result<Vec<AgentTurn>, String> {
        self.0
            .session_tasks(session_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| self.to_turn(t))
            .collect()
    }

    fn unfinished_turns(&self) -> Result<Vec<AgentTurn>, String> {
        self.0
            .unfinished_session_tasks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| self.to_turn(t))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_runtime::agent::NoticeLevel;

    fn session(id: &str) -> AgentSession {
        AgentSession {
            id: id.into(),
            runtime_id: "codex".into(),
            provider: "openai".into(),
            provider_session_id: None,
            provider_session_confirmed: false,
            model: None,
            title: "Say hello".into(),
            state: SessionState::Open,
            working_dir: "/w/s".into(),
            created_at: 1,
            updated_at: 1,
            turn_count: 0,
            active_task_id: None,
        }
    }

    fn result(outcome: TurnOutcome) -> TurnResult {
        TurnResult {
            outcome,
            summary: "Hello!".into(),
            text: Some("Hello!".into()),
            error: None,
            provider_session_id: Some("thread-1".into()),
            model: None,
            usage: None,
            duration_ms: Some(10),
            ignored_lines: 0,
        }
    }

    #[test]
    fn sessions_and_turns_round_trip_through_the_ledger() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let store = LedgerSessionStore(ledger.clone());
        let mut s = session("s-1");
        store.open_session(&s).unwrap();
        let task_id = store.begin_turn(&s, 1, "Say hello").unwrap();
        let turn = TurnRef {
            session_id: "s-1",
            task_id: &task_id,
            execution_id: None,
            actor: "agent:codex",
        };
        store
            .record_activity(
                &turn,
                &AgentEvent::Message {
                    text: "Hello!".into(),
                },
            )
            .unwrap();
        // Live-only activity is not stored.
        store
            .record_activity(&turn, &AgentEvent::TextDelta { text: "H".into() })
            .unwrap();
        s.provider_session_id = Some("thread-1".into());
        s.provider_session_confirmed = true;
        store.save_session(&s, SessionChange::Bound).unwrap();

        let running = store.unfinished_turns().unwrap();
        assert_eq!(running.len(), 1);
        assert!(running[0].running);
        store
            .finish_turn(&turn, &result(TurnOutcome::Completed))
            .unwrap();
        assert!(store.unfinished_turns().unwrap().is_empty());

        let turns = store.turns("s-1").unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].number, 1);
        assert_eq!(turns[0].objective, "Say hello");
        assert!(!turns[0].running);
        assert_eq!(turns[0].result, Some(result(TurnOutcome::Completed)));

        let loaded = store.session("s-1").unwrap().unwrap();
        assert_eq!(loaded.turn_count, 1);
        assert!(loaded.provider_session_confirmed);
        assert_eq!(loaded.provider_session_id.as_deref(), Some("thread-1"));

        // The task's trail tells the whole story, in order.
        let task = ledger.task(&task_id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Succeeded);
        assert_eq!(task.assigned_to.as_deref(), Some("codex"));
        let trail: Vec<_> = ledger
            .events_for_task(&task_id)
            .unwrap()
            .into_iter()
            .map(|e| (e.event_type, e.source))
            .collect();
        let types: Vec<_> = trail.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            types,
            [
                "task.created",
                "task.state_changed",
                "agent.message",
                "agent.result",
                "task.state_changed"
            ]
        );
        assert_eq!(trail[2].1, "agent:codex");

        store.save_session(&s, SessionChange::Closed).unwrap();
        assert_eq!(store.sessions(10).unwrap()[0].state, SessionState::Closed);
    }

    #[test]
    fn outcomes_map_to_final_task_states() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let store = LedgerSessionStore(ledger.clone());
        let s = session("s-2");
        store.open_session(&s).unwrap();
        for (outcome, state) in [
            (TurnOutcome::Completed, TaskState::Succeeded),
            (TurnOutcome::Cancelled, TaskState::Cancelled),
            (TurnOutcome::UsageLimited, TaskState::Failed),
            (TurnOutcome::Interrupted, TaskState::Failed),
        ] {
            let task_id = store.begin_turn(&s, 1, "x").unwrap();
            let turn = TurnRef {
                session_id: "s-2",
                task_id: &task_id,
                execution_id: None,
                actor: "plenipo",
            };
            store.finish_turn(&turn, &result(outcome)).unwrap();
            assert_eq!(ledger.task(&task_id).unwrap().unwrap().state, state);
            // Finishing twice is refused (the state machine is final).
            assert!(store.finish_turn(&turn, &result(outcome)).is_err());
        }
        let notice = AgentEvent::Notice {
            level: NoticeLevel::Warning,
            text: "x".into(),
        };
        assert_eq!(notice.ledger_type(), Some("agent.notice"));
    }
}
