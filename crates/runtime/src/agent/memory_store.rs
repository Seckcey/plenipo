//! An in-memory [`SessionStore`] for tests and tools. The desktop app records sessions in
//! the Ledger instead.

use std::sync::{Mutex, MutexGuard};

use crate::agent::dto::{AgentEvent, AgentSession, AgentTurn, TurnResult, TurnStep};
use crate::agent::service::{SessionChange, SessionStore, StepNote, TurnInput, TurnRef, TurnTask};

#[derive(Debug, Default)]
struct Data {
    sessions: Vec<AgentSession>,
    turns: Vec<AgentTurn>,
    /// (task ID, event) in recording order.
    activity: Vec<(String, AgentEvent)>,
    changes: Vec<(String, SessionChange)>,
    /// (task ID, step reason) in recording order.
    notes: Vec<(String, StepNote)>,
}

#[derive(Debug, Default)]
pub struct MemorySessionStore {
    data: Mutex<Data>,
}

impl MemorySessionStore {
    fn lock(&self) -> MutexGuard<'_, Data> {
        self.data.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Recorded activity for one task, in order.
    pub fn activity(&self, task_id: &str) -> Vec<AgentEvent> {
        self.lock()
            .activity
            .iter()
            .filter(|(t, _)| t == task_id)
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// Recorded session changes, in order.
    pub fn changes(&self, session_id: &str) -> Vec<SessionChange> {
        self.lock()
            .changes
            .iter()
            .filter(|(s, _)| s == session_id)
            .map(|(_, c)| *c)
            .collect()
    }

    /// Notes recorded when a task continued, in order.
    pub fn notes(&self, task_id: &str) -> Vec<StepNote> {
        self.lock()
            .notes
            .iter()
            .filter(|(t, _)| t == task_id)
            .map(|(_, n)| n.clone())
            .collect()
    }

    /// Insert a turn directly (tests simulating a previous Plenipo session, or a task recorded
    /// for adoption by a later turn).
    pub fn insert_turn(&self, turn: AgentTurn) {
        self.lock().turns.push(turn);
    }

    /// Record a step's result and leave the turn waiting (what a [`crate::agent::TurnHook`]
    /// does before it returns `Suspended`).
    pub fn suspend(&self, turn_ref: &TurnRef<'_>, result: &TurnResult) -> Result<(), String> {
        let mut data = self.lock();
        let turn = data
            .turns
            .iter_mut()
            .find(|t| t.task_id == turn_ref.task_id)
            .ok_or_else(|| format!("unknown turn {}", turn_ref.task_id))?;
        if !turn.running {
            return Err(format!("turn {} is not running", turn_ref.task_id));
        }
        turn.running = false;
        turn.waiting = true;
        push_step(turn, turn_ref, result);
        Ok(())
    }
}

fn push_step(turn: &mut AgentTurn, turn_ref: &TurnRef<'_>, result: &TurnResult) {
    turn.execution_id = turn_ref
        .execution_id
        .map(str::to_owned)
        .or(turn.execution_id.take());
    if let Some(step) = turn_ref.step {
        turn.steps.push(TurnStep {
            number: step,
            execution_id: turn_ref.execution_id.map(str::to_owned),
            running: false,
            result: Some(result.clone()),
            started_at: None,
            ended_at: Some(crate::now_ms()),
        });
    }
}

impl SessionStore for MemorySessionStore {
    fn sessions(&self, limit: usize) -> Result<Vec<AgentSession>, String> {
        let data = self.lock();
        Ok(data.sessions.iter().rev().take(limit).cloned().collect())
    }

    fn session(&self, id: &str) -> Result<Option<AgentSession>, String> {
        Ok(self.lock().sessions.iter().find(|s| s.id == id).cloned())
    }

    fn open_session(&self, session: &AgentSession) -> Result<(), String> {
        let mut data = self.lock();
        if data.sessions.iter().any(|s| s.id == session.id) {
            return Err(format!("session {} already exists", session.id));
        }
        data.sessions.push(AgentSession {
            active_task_id: None,
            waiting_task_id: None,
            ..session.clone()
        });
        Ok(())
    }

    fn save_session(&self, session: &AgentSession, change: SessionChange) -> Result<(), String> {
        let mut data = self.lock();
        let slot = data
            .sessions
            .iter_mut()
            .find(|s| s.id == session.id)
            .ok_or_else(|| format!("unknown session {}", session.id))?;
        let turn_count = slot.turn_count;
        *slot = AgentSession {
            active_task_id: None,
            waiting_task_id: None,
            turn_count,
            ..session.clone()
        };
        data.changes.push((session.id.clone(), change));
        Ok(())
    }

    fn begin_turn(
        &self,
        session: &AgentSession,
        number: u32,
        input: &TurnInput,
    ) -> Result<String, String> {
        let mut data = self.lock();
        let task_id = match &input.task {
            TurnTask::New { .. } => {
                let task_id = uuid::Uuid::new_v4().to_string();
                data.turns.push(AgentTurn {
                    task_id: task_id.clone(),
                    session_id: session.id.clone(),
                    number,
                    objective: input.objective.clone(),
                    execution_id: None,
                    running: true,
                    waiting: false,
                    result: None,
                    steps: Vec::new(),
                    started_at: crate::now_ms(),
                    ended_at: None,
                });
                task_id
            }
            TurnTask::Existing { task_id } => {
                let turn = data
                    .turns
                    .iter_mut()
                    .find(|t| &t.task_id == task_id)
                    .ok_or_else(|| format!("unknown task {task_id}"))?;
                if turn.running || turn.waiting || turn.result.is_some() {
                    return Err(format!("task {task_id} has already started"));
                }
                turn.session_id = session.id.clone();
                turn.number = number;
                turn.running = true;
                task_id.clone()
            }
        };
        if let Some(s) = data.sessions.iter_mut().find(|s| s.id == session.id) {
            s.turn_count = number;
        }
        Ok(task_id)
    }

    fn begin_step(&self, turn_ref: &TurnRef<'_>, note: &StepNote) -> Result<(), String> {
        let mut data = self.lock();
        let turn = data
            .turns
            .iter_mut()
            .find(|t| t.task_id == turn_ref.task_id)
            .ok_or_else(|| format!("unknown turn {}", turn_ref.task_id))?;
        if !turn.waiting {
            return Err(format!("turn {} is not waiting", turn_ref.task_id));
        }
        turn.waiting = false;
        turn.running = true;
        data.notes.push((turn_ref.task_id.to_owned(), note.clone()));
        Ok(())
    }

    fn record_activity(&self, turn: &TurnRef<'_>, event: &AgentEvent) -> Result<(), String> {
        self.lock()
            .activity
            .push((turn.task_id.to_owned(), event.clone()));
        Ok(())
    }

    fn finish_turn(&self, turn_ref: &TurnRef<'_>, result: &TurnResult) -> Result<(), String> {
        let mut data = self.lock();
        let turn = data
            .turns
            .iter_mut()
            .find(|t| t.task_id == turn_ref.task_id)
            .ok_or_else(|| format!("unknown turn {}", turn_ref.task_id))?;
        if turn.result.is_some() {
            return Err(format!("turn {} already finished", turn_ref.task_id));
        }
        turn.running = false;
        turn.waiting = false;
        push_step(turn, turn_ref, result);
        turn.result = Some(result.clone());
        turn.ended_at = Some(crate::now_ms());
        Ok(())
    }

    fn turns(&self, session_id: &str) -> Result<Vec<AgentTurn>, String> {
        Ok(self
            .lock()
            .turns
            .iter()
            .filter(|t| t.session_id == session_id)
            .cloned()
            .collect())
    }

    fn unfinished_turns(&self) -> Result<Vec<AgentTurn>, String> {
        Ok(self
            .lock()
            .turns
            .iter()
            .filter(|t| t.result.is_none())
            .cloned()
            .collect())
    }
}
