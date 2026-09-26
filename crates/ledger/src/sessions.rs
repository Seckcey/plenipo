//! Agent runtime sessions (Phase 3, ADR-007). Every change writes its `session.*` event in the
//! same transaction.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use crate::dto::{NewEvent, NewRuntimeSession, RuntimeSession, RuntimeSessionState, Task};
use crate::error::{LedgerError, Result};
use crate::events;
use crate::rows::{self, json as parse_json, metadata_text, opt_u64, parse_enum, u64_of};
use crate::Ledger;

/// `+s.id` drops the column's TEXT affinity so SQLite can use the `tasks_by_session`
/// expression index (a SEARCH, not a SCAN, per session).
const SESSION_COLUMNS: &str = "s.id, s.runtime, s.provider, s.provider_session_id, \
    s.provider_session_confirmed, s.model, s.title, s.working_dir, s.state, s.metadata, \
    s.created_at, s.updated_at, s.closed_at, \
    (SELECT COUNT(*) FROM tasks t WHERE json_extract(t.metadata, '$.sessionId') = +s.id)";

/// Expression matching the `tasks_by_session` index.
const TASK_SESSION: &str = "json_extract(metadata, '$.sessionId')";

fn session_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RuntimeSession> {
    Ok(RuntimeSession {
        id: r.get(0)?,
        runtime: r.get(1)?,
        provider: r.get(2)?,
        provider_session_id: r.get(3)?,
        provider_session_confirmed: r.get::<_, i64>(4)? != 0,
        model: r.get(5)?,
        title: r.get(6)?,
        working_dir: r.get(7)?,
        state: parse_enum(8, r.get(8)?, RuntimeSessionState::parse)?,
        metadata: parse_json(r.get(9)?),
        created_at: u64_of(r.get(10)?),
        updated_at: u64_of(r.get(11)?),
        closed_at: opt_u64(r.get(12)?),
        turn_count: r.get(13)?,
    })
}

fn get(conn: &Connection, id: &str) -> Result<Option<RuntimeSession>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SESSION_COLUMNS} FROM runtime_sessions s WHERE s.id = ?1"),
            [id],
            session_row,
        )
        .optional()?)
}

fn require(conn: &Connection, id: &str) -> Result<RuntimeSession> {
    get(conn, id)?.ok_or_else(|| LedgerError::NotFound(format!("runtime session {id}")))
}

/// `[0-9a-z-]{1,64}` (UUIDs and similar).
fn validate_id(id: &str) -> Result<()> {
    let ok = (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(LedgerError::InvalidInput(format!(
            "invalid runtime session id {id:?}"
        )))
    }
}

fn session_event(
    id: &str,
    actor: &str,
    event_type: &str,
    mut payload: serde_json::Value,
) -> NewEvent {
    if let Some(map) = payload.as_object_mut() {
        map.insert("sessionId".into(), json!(id));
    }
    NewEvent {
        source: actor.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    }
}

impl Ledger {
    /// Record a new, open session and `session.opened`.
    pub fn open_runtime_session(
        &self,
        new: NewRuntimeSession,
        actor: &str,
    ) -> Result<RuntimeSession> {
        validate_id(&new.id)?;
        let metadata = metadata_text(&new.metadata)?;
        self.write(|tx, out| {
            let now = crate::now_ms() as i64;
            tx.execute(
                "INSERT INTO runtime_sessions (id, runtime, provider, model, title, working_dir,
                     state, metadata, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?8, ?8)",
                params![
                    new.id,
                    new.runtime,
                    new.provider,
                    new.model,
                    new.title,
                    new.working_dir,
                    metadata,
                    now
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _)
                    if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    LedgerError::InvalidInput(format!("runtime session rejected: {e}"))
                }
                other => other.into(),
            })?;
            out.push(events::insert(
                tx,
                session_event(
                    &new.id,
                    actor,
                    "session.opened",
                    json!({
                        "runtime": new.runtime,
                        "provider": new.provider,
                        "model": new.model,
                        "workingDir": new.working_dir,
                    }),
                ),
            )?);
            require(tx, &new.id)
        })
    }

    /// Record the provider's session ID and/or model (`session.bound`). Unchanged values
    /// write nothing. A closed session cannot be rebound.
    pub fn bind_runtime_session(
        &self,
        id: &str,
        provider_session_id: Option<&str>,
        model: Option<&str>,
        actor: &str,
    ) -> Result<RuntimeSession> {
        self.write(|tx, out| {
            let current = require(tx, id)?;
            if current.state == RuntimeSessionState::Closed {
                return Err(LedgerError::InvalidInput(format!(
                    "runtime session {id} is closed"
                )));
            }
            let provider_id = provider_session_id.or(current.provider_session_id.as_deref());
            let model = model.or(current.model.as_deref());
            let confirmed = provider_session_id.is_some() || current.provider_session_confirmed;
            if provider_id == current.provider_session_id.as_deref()
                && model == current.model.as_deref()
                && confirmed == current.provider_session_confirmed
            {
                return Ok(current);
            }
            tx.execute(
                "UPDATE runtime_sessions SET provider_session_id = ?2, provider_session_confirmed = ?3,
                     model = ?4, updated_at = ?5 WHERE id = ?1",
                params![id, provider_id, confirmed, model, crate::now_ms() as i64],
            )?;
            out.push(events::insert(
                tx,
                session_event(
                    id,
                    actor,
                    "session.bound",
                    json!({
                        "providerSessionId": provider_id,
                        "previousProviderSessionId": current.provider_session_id,
                        "model": model,
                    }),
                ),
            )?);
            require(tx, id)
        })
    }

    /// Update `updated_at` only (no event).
    pub fn touch_runtime_session(&self, id: &str) -> Result<()> {
        self.write(|tx, _| {
            let changed = tx.execute(
                "UPDATE runtime_sessions SET updated_at = ?2 WHERE id = ?1",
                params![id, crate::now_ms() as i64],
            )?;
            if changed == 0 {
                return Err(LedgerError::NotFound(format!("runtime session {id}")));
            }
            Ok(())
        })
    }

    /// Close a session (`session.closed`). Closing a closed session changes nothing.
    pub fn close_runtime_session(&self, id: &str, actor: &str) -> Result<RuntimeSession> {
        self.write(|tx, out| {
            let current = require(tx, id)?;
            if current.state == RuntimeSessionState::Closed {
                return Ok(current);
            }
            let now = crate::now_ms() as i64;
            tx.execute(
                "UPDATE runtime_sessions SET state = 'closed', closed_at = ?2, updated_at = ?2
                 WHERE id = ?1",
                params![id, now],
            )?;
            out.push(events::insert(
                tx,
                session_event(id, actor, "session.closed", json!({})),
            )?);
            require(tx, id)
        })
    }

    pub fn runtime_session(&self, id: &str) -> Result<Option<RuntimeSession>> {
        self.read(|c| get(c, id))
    }

    /// Most recently active first.
    pub fn list_runtime_sessions(&self, limit: u32) -> Result<Vec<RuntimeSession>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SESSION_COLUMNS} FROM runtime_sessions s
                 ORDER BY s.updated_at DESC, s.rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 10_000)], session_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Open sessions whose metadata marks them as handoff workers (`liaison.origin`).
    pub fn open_handoff_sessions(&self) -> Result<Vec<RuntimeSession>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SESSION_COLUMNS} FROM runtime_sessions s
                 WHERE s.state = 'open' AND json_extract(s.metadata, '$.liaison.origin') = 'handoff'
                 ORDER BY s.created_at, s.rowid"
            ))?;
            let rows = stmt
                .query_map([], session_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// A session's turns (tasks), oldest first.
    pub fn session_tasks(&self, session_id: &str) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {} FROM tasks WHERE {TASK_SESSION} = ?1 ORDER BY created_at, rowid",
                rows::TASK_COLUMNS
            ))?;
            let rows = stmt
                .query_map([session_id], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Session turns that never reached a final state (left when Plenipo stopped).
    pub fn unfinished_session_tasks(&self) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {} FROM tasks WHERE {TASK_SESSION} IS NOT NULL
                   AND state NOT IN ('succeeded', 'failed', 'cancelled')
                 ORDER BY created_at, rowid",
                rows::TASK_COLUMNS
            ))?;
            let rows = stmt
                .query_map([], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{NewTask, TaskState};
    use crate::test_support::*;

    fn new_session(id: &str) -> NewRuntimeSession {
        NewRuntimeSession {
            id: id.into(),
            runtime: "claude-code".into(),
            provider: "anthropic".into(),
            model: None,
            title: "Say hello".into(),
            working_dir: "/work/s1".into(),
            metadata: serde_json::Value::Null,
        }
    }

    fn turn(l: &Ledger, session: &str, objective: &str) -> Task {
        l.create_task(
            NewTask {
                requested_by: "owner".into(),
                objective: objective.into(),
                metadata: json!({ "sessionId": session, "turn": 1 }),
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
    }

    #[test]
    fn session_lifecycle_writes_its_trail() {
        let l = ledger();
        let s = l.open_runtime_session(new_session("s-1"), "owner").unwrap();
        assert_eq!(s.state, RuntimeSessionState::Open);
        assert!(!s.provider_session_confirmed);
        assert_eq!(s.turn_count, 0);

        let b = l
            .bind_runtime_session("s-1", Some("p-1"), Some("model-a"), "runtime")
            .unwrap();
        assert_eq!(b.provider_session_id.as_deref(), Some("p-1"));
        assert!(b.provider_session_confirmed);
        // Unchanged binding writes nothing.
        l.bind_runtime_session("s-1", Some("p-1"), None, "runtime")
            .unwrap();
        l.touch_runtime_session("s-1").unwrap();
        let c = l.close_runtime_session("s-1", "owner").unwrap();
        assert_eq!(c.state, RuntimeSessionState::Closed);
        assert!(c.closed_at.is_some());
        assert_eq!(
            l.close_runtime_session("s-1", "owner").unwrap().state,
            RuntimeSessionState::Closed
        );
        assert!(l
            .bind_runtime_session("s-1", Some("p-2"), None, "runtime")
            .is_err());

        let types: Vec<_> = l
            .recent_events(50)
            .unwrap()
            .into_iter()
            .rev()
            .filter(|e| e.payload["sessionId"] == "s-1")
            .map(|e| e.event_type)
            .collect();
        assert_eq!(types, ["session.opened", "session.bound", "session.closed"]);
    }

    #[test]
    fn turns_are_tasks_linked_by_metadata() {
        let l = ledger();
        l.open_runtime_session(new_session("s-1"), "owner").unwrap();
        l.open_runtime_session(new_session("s-2"), "owner").unwrap();
        let a = turn(&l, "s-1", "first");
        let b = turn(&l, "s-1", "second");
        turn(&l, "s-2", "other");
        let _unrelated = task(&l, "not a turn");
        let ids: Vec<_> = l
            .session_tasks("s-1")
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, [a.id.clone(), b.id.clone()]);
        assert_eq!(l.runtime_session("s-1").unwrap().unwrap().turn_count, 2);

        l.transition_task(&a.id, TaskState::Running, "w", None)
            .unwrap();
        l.transition_task(&a.id, TaskState::Succeeded, "w", None)
            .unwrap();
        let unfinished: Vec<_> = l
            .unfinished_session_tasks()
            .unwrap()
            .into_iter()
            .map(|t| t.objective)
            .collect();
        assert_eq!(
            unfinished,
            ["second", "other"],
            "unrelated tasks are not turns"
        );
        // Most recently active first.
        l.touch_runtime_session("s-1").unwrap();
        let listed: Vec<_> = l
            .list_runtime_sessions(10)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(listed, ["s-1", "s-2"]);
    }

    #[test]
    fn handoff_worker_sessions_are_found_by_their_metadata() {
        let l = ledger();
        l.open_runtime_session(new_session("s-owner"), "owner")
            .unwrap();
        let mut worker = new_session("s-worker");
        worker.metadata = json!({ "liaison": { "enabled": true, "origin": "handoff" } });
        l.open_runtime_session(worker, "liaison").unwrap();
        let mut root = new_session("s-root");
        root.metadata = json!({ "liaison": { "enabled": true, "origin": "owner" } });
        l.open_runtime_session(root, "owner").unwrap();
        let open: Vec<_> = l
            .open_handoff_sessions()
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(open, ["s-worker"]);
        assert_eq!(
            l.runtime_session("s-worker").unwrap().unwrap().metadata["liaison"]["origin"],
            "handoff"
        );
        l.close_runtime_session("s-worker", "liaison").unwrap();
        assert!(l.open_handoff_sessions().unwrap().is_empty());
    }

    #[test]
    fn invalid_sessions_are_rejected() {
        let l = ledger();
        for bad in ["", "Has Caps", "../x", &"a".repeat(65)] {
            assert!(
                l.open_runtime_session(new_session(bad), "owner").is_err(),
                "{bad:?}"
            );
        }
        let mut no_title = new_session("s-3");
        no_title.title = String::new();
        assert!(matches!(
            l.open_runtime_session(no_title, "owner"),
            Err(LedgerError::InvalidInput(_))
        ));
        l.open_runtime_session(new_session("s-4"), "owner").unwrap();
        assert!(l.open_runtime_session(new_session("s-4"), "owner").is_err());
        assert!(matches!(
            l.touch_runtime_session("missing"),
            Err(LedgerError::NotFound(_))
        ));
        assert!(l.runtime_session("missing").unwrap().is_none());
    }
}
