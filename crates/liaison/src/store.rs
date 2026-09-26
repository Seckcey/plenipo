//! Ledger-backed persistence for the agent runtime: executions (supervisor records), runtime
//! sessions, and turns. The desktop app wires these; they live here because Liaison records its
//! handoff steps in the same transactions as turn state (ADR-008), and its tests need them.

use std::sync::Arc;

use plenipo_ledger::{
    ExecutionRow, Ledger, NewEvent, NewRuntimeSession, NewTask, RuntimeSession,
    RuntimeSessionState, Task, TaskState,
};
use plenipo_runtime::agent::{
    AgentEvent, AgentSession, AgentTurn, Effort, SessionChange, SessionState, SessionStore,
    StepNote, TurnInput, TurnOutcome, TurnRef, TurnResult, TurnStep, TurnTask, OWNER,
};
use plenipo_runtime::store::Loaded;
use plenipo_runtime::{
    AgentAttribution, ExecutionRecord, ExecutionState, ExecutionStore, TokenUsage,
};
use serde_json::{json, Value};

/// Supervisor persistence backed by the ledger's `executions` table.
pub struct LedgerExecutionStore(pub Arc<Ledger>);

impl ExecutionStore for LedgerExecutionStore {
    fn load(&self, limit: usize) -> Loaded {
        match self
            .0
            .recent_executions(u32::try_from(limit).unwrap_or(u32::MAX))
        {
            Ok(rows) => {
                let mut notices = Vec::new();
                let mut records: Vec<ExecutionRecord> = rows
                    .into_iter()
                    .filter_map(|row| match to_record(row) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            notices.push(e);
                            None
                        }
                    })
                    .collect();
                records.reverse(); // oldest first
                Loaded { records, notices }
            }
            Err(e) => Loaded {
                records: vec![],
                notices: vec![format!(
                    "Could not read execution history from the ledger: {e}"
                )],
            },
        }
    }

    fn save(&self, record: &ExecutionRecord) -> Result<(), String> {
        self.0
            .upsert_execution(&to_row(record), "runtime")
            .map_err(|e| e.to_string())
    }
}

fn state_str(state: ExecutionState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// `runtime` value for plain supervised processes (no agent attribution).
const LOCAL_PROCESS: &str = "local-process";

pub fn to_row(r: &ExecutionRecord) -> ExecutionRow {
    let agent = r.agent.as_ref();
    ExecutionRow {
        id: r.id.clone(),
        task_id: agent.map(|a| a.task_id.clone()),
        runtime: agent.map_or_else(|| LOCAL_PROCESS.into(), |a| a.runtime_id.clone()),
        provider: agent.map(|a| a.provider.clone()),
        model: agent.and_then(|a| a.model.clone()),
        // Plenipo's session ID; the provider's own ID is kept with the usage metadata.
        session_id: agent.map(|a| a.session_id.clone()),
        process_id: r.pid,
        profile_id: Some(r.profile_id.clone()),
        label: r.label.clone(),
        executable: Some(r.executable.clone()),
        args: r.args.clone(),
        working_dir: Some(r.working_dir.clone()),
        state: state_str(r.state),
        exit_code: r.exit_code,
        detail: r.detail.clone(),
        started_at: r.started_at,
        ended_at: r.ended_at,
        usage_metadata: agent.map_or(
            Value::Null,
            |a| json!({ "providerSessionId": a.provider_session_id, "usage": a.usage }),
        ),
    }
}

fn to_attribution(row: &ExecutionRow) -> Option<Box<AgentAttribution>> {
    if row.runtime == LOCAL_PROCESS {
        return None;
    }
    let meta = &row.usage_metadata;
    Some(Box::new(AgentAttribution {
        runtime_id: row.runtime.clone(),
        provider: row.provider.clone()?,
        session_id: row.session_id.clone()?,
        task_id: row.task_id.clone()?,
        model: row.model.clone(),
        provider_session_id: meta
            .get("providerSessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        usage: meta
            .get("usage")
            .and_then(|u| serde_json::from_value::<TokenUsage>(u.clone()).ok()),
    }))
}

fn to_record(row: ExecutionRow) -> Result<ExecutionRecord, String> {
    let state: ExecutionState =
        serde_json::from_value(serde_json::Value::String(row.state.clone()))
            .map_err(|_| format!("Execution {} has an unknown state {:?}", row.id, row.state))?;
    let agent = to_attribution(&row);
    Ok(ExecutionRecord {
        id: row.id,
        profile_id: row.profile_id.unwrap_or_default(),
        label: row.label,
        executable: row.executable.unwrap_or_default(),
        args: row.args,
        working_dir: row.working_dir.unwrap_or_default(),
        pid: row.process_id,
        state,
        exit_code: row.exit_code,
        detail: row.detail,
        started_at: row.started_at,
        ended_at: row.ended_at,
        agent,
    })
}

/// Sessions and turns in the Ledger: sessions in `runtime_sessions`; each turn is a task whose
/// metadata names the session, with its activity, one `agent.result` per step (and a final
/// one), and — for handoffs — the Liaison records written in the same transactions.
pub struct LedgerSessionStore(pub Arc<Ledger>);

fn to_session(s: RuntimeSession) -> AgentSession {
    AgentSession {
        id: s.id,
        runtime_id: s.runtime,
        provider: s.provider,
        provider_session_id: s.provider_session_id,
        provider_session_confirmed: s.provider_session_confirmed,
        model: s.model,
        effort: s.effort.as_deref().and_then(Effort::parse),
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
        waiting_task_id: None,
        metadata: if s.metadata.is_object() {
            s.metadata
        } else {
            json!({})
        },
    }
}

fn final_state(outcome: TurnOutcome) -> TaskState {
    match outcome {
        TurnOutcome::Completed => TaskState::Succeeded,
        TurnOutcome::Cancelled => TaskState::Cancelled,
        _ => TaskState::Failed,
    }
}

/// The `agent.result` event for a step's (or a turn's final) result.
pub fn result_event(turn: &TurnRef<'_>, result: &TurnResult) -> Result<NewEvent, String> {
    let mut payload = serde_json::to_value(result).map_err(|e| e.to_string())?;
    payload["sessionId"] = json!(turn.session_id);
    if let Some(step) = turn.step {
        payload["step"] = json!(step);
    }
    Ok(NewEvent {
        execution_id: turn.execution_id.map(str::to_owned),
        source: turn.actor.into(),
        event_type: "agent.result".into(),
        payload,
        ..NewEvent::default()
    })
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
        let executions = self
            .0
            .executions_for_task(&task.id)
            .map_err(|e| e.to_string())?;
        let results: Vec<_> = self
            .0
            .events_for_task(&task.id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|e| e.event_type == "agent.result")
            .collect();
        let parse = |payload: &Value| serde_json::from_value::<TurnResult>(payload.clone()).ok();
        let started = |execution: &Option<String>| {
            executions
                .iter()
                .find(|x| Some(&x.id) == execution.as_ref())
                .map(|x| x.started_at)
        };
        let mut steps: Vec<TurnStep> = results
            .iter()
            .filter_map(|e| {
                let number = u32::try_from(e.payload.get("step")?.as_u64()?).ok()?;
                Some(TurnStep {
                    number,
                    execution_id: e.execution_id.clone(),
                    running: false,
                    result: parse(&e.payload),
                    started_at: started(&e.execution_id),
                    ended_at: Some(e.created_at),
                })
            })
            .collect();
        let result = if task.state.is_terminal() {
            results.last().and_then(|e| parse(&e.payload))
        } else {
            None
        };
        // Turns recorded before steps existed (Phase 3): a finished run is step 1.
        if steps.is_empty() {
            if let (Some(last), Some(execution)) = (results.last(), executions.last()) {
                if last.execution_id.as_deref() == Some(execution.id.as_str()) {
                    steps.push(TurnStep {
                        number: 1,
                        execution_id: Some(execution.id.clone()),
                        running: false,
                        result: parse(&last.payload),
                        started_at: Some(execution.started_at),
                        ended_at: Some(last.created_at),
                    });
                }
            }
        }
        Ok(AgentTurn {
            task_id: task.id,
            session_id,
            number,
            objective: task.objective,
            execution_id: executions.last().map(|e| e.id.clone()),
            running: task.state == TaskState::Running,
            waiting: task.state == TaskState::Blocked,
            result,
            steps,
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
        let actor = if session.metadata["liaison"]["origin"] == "handoff" {
            "liaison"
        } else {
            OWNER
        };
        self.0
            .open_runtime_session(
                NewRuntimeSession {
                    id: session.id.clone(),
                    runtime: session.runtime_id.clone(),
                    provider: session.provider.clone(),
                    model: session.model.clone(),
                    effort: session.effort.map(|e| e.as_str().to_owned()),
                    title: session.title.clone(),
                    working_dir: session.working_dir.clone(),
                    metadata: session.metadata.clone(),
                },
                actor,
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
        input: &TurnInput,
    ) -> Result<String, String> {
        let actor = format!("agent:{}", session.runtime_id);
        match &input.task {
            TurnTask::New {
                requested_by,
                metadata,
                project_id,
            } => {
                let mut metadata = if metadata.is_object() {
                    metadata.clone()
                } else {
                    json!({})
                };
                metadata["sessionId"] = json!(session.id);
                metadata["runtimeId"] = json!(session.runtime_id);
                metadata["turn"] = json!(number);
                let task = self
                    .0
                    .create_task(
                        NewTask {
                            requested_by: requested_by.clone(),
                            assigned_to: Some(session.runtime_id.clone()),
                            project_id: project_id.clone(),
                            objective: input.objective.clone(),
                            metadata,
                            ..NewTask::default()
                        },
                        requested_by,
                    )
                    .map_err(|e| e.to_string())?;
                self.0
                    .transition_task(&task.id, TaskState::Running, &actor, Some("turn started"))
                    .map_err(|e| e.to_string())?;
                Ok(task.id)
            }
            TurnTask::Existing { task_id } => {
                let task = self
                    .0
                    .task(task_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("unknown task {task_id}"))?;
                if task.metadata["sessionId"].as_str() != Some(session.id.as_str()) {
                    return Err(format!(
                        "task {task_id} is not assigned to session {}",
                        session.id
                    ));
                }
                let handoff = self
                    .0
                    .liaison_request_for_child(task_id)
                    .map_err(|e| e.to_string())?;
                let started = match handoff {
                    // Starting a handoff's child also marks the request dispatched.
                    Some(_) => self.0.begin_handoff_turn(task_id, &session.id, &actor),
                    None => self.0.transition_task(
                        task_id,
                        TaskState::Running,
                        &actor,
                        Some("turn started"),
                    ),
                };
                started.map(|t| t.id).map_err(|e| e.to_string())
            }
        }
    }

    fn begin_step(&self, turn: &TurnRef<'_>, note: &StepNote) -> Result<(), String> {
        // `deliver`: the Liaison replies this step receives, marked delivered with the step.
        let deliver: Vec<String> = note
            .data
            .get("deliver")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let reason = Some(note.reason.as_str()).filter(|r| !r.is_empty());
        let resumed = if deliver.is_empty() {
            self.0
                .transition_task(turn.task_id, TaskState::Running, turn.actor, reason)
        } else {
            self.0.resume_with_replies(
                turn.task_id,
                &deliver,
                reason.unwrap_or("continuing"),
                turn.actor,
            )
        };
        resumed.map(|_| ()).map_err(|e| e.to_string())
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
        self.0
            .complete_task(
                turn.task_id,
                final_state(result.outcome),
                turn.actor,
                Some(&result.summary),
                result_event(turn, result)?,
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

    fn record(id: &str, state: ExecutionState) -> ExecutionRecord {
        ExecutionRecord {
            id: id.into(),
            profile_id: "diagnostic.echo".into(),
            label: "Echo test".into(),
            executable: "/x".into(),
            args: vec!["--plenipo-diagnostic=echo".into()],
            working_dir: "/".into(),
            pid: Some(7),
            state,
            exit_code: Some(0),
            detail: None,
            started_at: 10,
            ended_at: Some(20),
            agent: None,
        }
    }

    #[test]
    fn records_round_trip_through_the_ledger() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let store = LedgerExecutionStore(ledger.clone());
        let r = record("e1", ExecutionState::TimedOut);
        store.save(&r).unwrap();
        let loaded = store.load(10);
        assert_eq!(loaded.records, [r]);
        assert!(loaded.notices.is_empty());
        let events = ledger.events_for_execution("e1").unwrap();
        assert_eq!(events[0].event_type, "execution.timed_out");
    }

    #[test]
    fn agent_attribution_round_trips_through_the_ledger() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let task = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "owner".into(),
                    objective: "Say hello".into(),
                    ..Default::default()
                },
                "owner",
            )
            .unwrap();
        let store = LedgerExecutionStore(ledger.clone());
        let mut r = record("e2", ExecutionState::Succeeded);
        r.agent = Some(Box::new(AgentAttribution {
            runtime_id: "claude-code".into(),
            provider: "anthropic".into(),
            session_id: "s-1".into(),
            task_id: task.id.clone(),
            model: Some("model-a".into()),
            provider_session_id: Some("p-9".into()),
            usage: Some(TokenUsage {
                input_tokens: 10,
                cached_input_tokens: 2,
                output_tokens: 5,
            }),
        }));
        store.save(&r).unwrap();
        assert_eq!(store.load(10).records, [r]);
        let row = ledger.execution("e2").unwrap().unwrap();
        assert_eq!(row.runtime, "claude-code");
        assert_eq!(row.provider.as_deref(), Some("anthropic"));
        assert_eq!(row.task_id.as_deref(), Some(task.id.as_str()));
        // The execution appears in its task's trail.
        let trail = ledger.events_for_task(&task.id).unwrap();
        assert!(trail.iter().any(|e| e.event_type == "execution.succeeded"));
    }

    fn session(id: &str) -> AgentSession {
        AgentSession {
            id: id.into(),
            runtime_id: "codex".into(),
            provider: "openai".into(),
            provider_session_id: None,
            provider_session_confirmed: false,
            model: None,
            effort: None,
            title: "Say hello".into(),
            state: SessionState::Open,
            working_dir: "/w/s".into(),
            created_at: 1,
            updated_at: 1,
            turn_count: 0,
            active_task_id: None,
            waiting_task_id: None,
            metadata: json!({}),
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
        let task_id = store
            .begin_turn(&s, 1, &TurnInput::owner("Say hello"))
            .unwrap();
        let turn = TurnRef {
            session_id: "s-1",
            task_id: &task_id,
            execution_id: None,
            step: Some(1),
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
        assert_eq!(turns[0].steps.len(), 1);
        assert_eq!(turns[0].steps[0].number, 1);
        assert_eq!(
            turns[0].steps[0].result,
            Some(result(TurnOutcome::Completed))
        );

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
            let task_id = store.begin_turn(&s, 1, &TurnInput::owner("x")).unwrap();
            let turn = TurnRef {
                session_id: "s-2",
                task_id: &task_id,
                execution_id: None,
                step: None,
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
