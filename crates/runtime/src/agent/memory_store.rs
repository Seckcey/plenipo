//! An in-memory [`SessionStore`] for tests and tools. The desktop app records sessions in
//! the Ledger instead.

use std::sync::{Mutex, MutexGuard};

use crate::agent::dto::{AgentEvent, AgentSession, AgentTurn, TurnResult};
use crate::agent::service::{SessionChange, SessionStore};

#[derive(Debug, Default)]
struct Data {
    sessions: Vec<AgentSession>,
    turns: Vec<AgentTurn>,
    /// (task ID, event) in recording order.
    activity: Vec<(String, AgentEvent)>,
    changes: Vec<(String, SessionChange)>,
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

    /// Insert a turn directly (tests simulating a previous Plenipo session).
    pub fn insert_turn(&self, turn: AgentTurn) {
        self.lock().turns.push(turn);
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
        objective: &str,
    ) -> Result<String, String> {
        let mut data = self.lock();
        let task_id = uuid::Uuid::new_v4().to_string();
        data.turns.push(AgentTurn {
            task_id: task_id.clone(),
            session_id: session.id.clone(),
            number,
            objective: objective.to_owned(),
            execution_id: None,
            running: true,
            result: None,
            started_at: crate::now_ms(),
            ended_at: None,
        });
        if let Some(s) = data.sessions.iter_mut().find(|s| s.id == session.id) {
            s.turn_count = number;
        }
        Ok(task_id)
    }

    fn record_activity(
        &self,
        _session_id: &str,
        task_id: &str,
        _execution_id: Option<&str>,
        event: &AgentEvent,
    ) -> Result<(), String> {
        self.lock()
            .activity
            .push((task_id.to_owned(), event.clone()));
        Ok(())
    }

    fn finish_turn(
        &self,
        _session_id: &str,
        task_id: &str,
        execution_id: Option<&str>,
        result: &TurnResult,
    ) -> Result<(), String> {
        let mut data = self.lock();
        let turn = data
            .turns
            .iter_mut()
            .find(|t| t.task_id == task_id)
            .ok_or_else(|| format!("unknown turn {task_id}"))?;
        if !turn.running {
            return Err(format!("turn {task_id} already finished"));
        }
        turn.running = false;
        turn.execution_id = execution_id.map(str::to_owned).or(turn.execution_id.take());
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
            .filter(|t| t.running)
            .cloned()
            .collect())
    }
}
