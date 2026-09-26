//! The append-only event trail.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::Value;

use crate::dto::{LedgerEvent, NewEvent};
use crate::error::{LedgerError, Result};
use crate::rows::{self, EVENT_COLUMNS};
use crate::Ledger;

/// `lower_snake(.lower_snake)+`, 3–64 bytes, e.g. `task.state_changed`.
pub(crate) fn validate_event_type(t: &str) -> Result<()> {
    let segment_ok = |s: &str| {
        !s.is_empty()
            && s.starts_with(|c: char| c.is_ascii_lowercase())
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    let parts: Vec<&str> = t.split('.').collect();
    if (3..=64).contains(&t.len()) && parts.len() >= 2 && parts.iter().all(|p| segment_ok(p)) {
        Ok(())
    } else {
        Err(LedgerError::InvalidInput(format!(
            "invalid event type {t:?}"
        )))
    }
}

/// Insert one event inside an open transaction.
pub(crate) fn insert(conn: &Connection, e: NewEvent) -> Result<LedgerEvent> {
    validate_event_type(&e.event_type)?;
    if e.source.trim().is_empty() || e.source.len() > 200 {
        return Err(LedgerError::InvalidInput(
            "event source must be 1–200 characters".into(),
        ));
    }
    let payload = if e.payload.is_null() {
        Value::Object(Default::default())
    } else {
        e.payload
    };
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = crate::now_ms();
    conn.execute(
        "INSERT INTO events (id, task_id, execution_id, source, destination, event_type, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            e.task_id,
            e.execution_id,
            e.source,
            e.destination,
            e.event_type,
            payload.to_string(),
            created_at as i64
        ],
    )?;
    Ok(LedgerEvent {
        seq: rows::u64_of(conn.last_insert_rowid()),
        id,
        task_id: e.task_id,
        execution_id: e.execution_id,
        source: e.source,
        destination: e.destination,
        event_type: e.event_type,
        payload,
        created_at,
    })
}

impl Ledger {
    /// Append a free-standing event (e.g. a system or diagnostic event).
    pub fn append_event(&self, event: NewEvent) -> Result<LedgerEvent> {
        self.write(|tx, events| {
            if let Some(task_id) = &event.task_id {
                let exists: Option<String> = tx
                    .query_row("SELECT id FROM tasks WHERE id = ?1", [task_id], |r| {
                        r.get(0)
                    })
                    .optional()?;
                if exists.is_none() {
                    return Err(LedgerError::NotFound(format!("task {task_id}")));
                }
            }
            let e = insert(tx, event)?;
            events.push(e.clone());
            Ok(e)
        })
    }

    /// Complete trail for a task, oldest first.
    pub fn events_for_task(&self, task_id: &str) -> Result<Vec<LedgerEvent>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM events WHERE task_id = ?1 ORDER BY seq"
            ))?;
            let rows = stmt
                .query_map([task_id], rows::event)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Trail for an execution, oldest first.
    pub fn events_for_execution(&self, execution_id: &str) -> Result<Vec<LedgerEvent>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM events WHERE execution_id = ?1 ORDER BY seq"
            ))?;
            let rows = stmt
                .query_map([execution_id], rows::event)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// The task's most recent event of `event_type`.
    pub fn last_task_event(&self, task_id: &str, event_type: &str) -> Result<Option<LedgerEvent>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {EVENT_COLUMNS} FROM events WHERE task_id = ?1 AND event_type = ?2
                     ORDER BY seq DESC LIMIT 1"
                ),
                [task_id, event_type],
                rows::event,
            )
            .optional()?)
        })
    }

    /// How many events of `event_type` the task's trail holds.
    pub fn count_task_events(&self, task_id: &str, event_type: &str) -> Result<u32> {
        self.read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM events WHERE task_id = ?1 AND event_type = ?2",
                [task_id, event_type],
                |r| r.get(0),
            )?)
        })
    }

    /// Most recent events across the whole ledger, newest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<LedgerEvent>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM events ORDER BY seq DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 1000)], rows::event)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use serde_json::json;

    #[test]
    fn event_types() {
        for good in [
            "task.created",
            "execution.timed_out",
            "org.role_created",
            "a.b.c",
        ] {
            assert!(validate_event_type(good).is_ok(), "{good}");
        }
        for bad in [
            "task",
            "Task.created",
            "task..x",
            ".x",
            "x.",
            "task.state-changed",
            "t.é",
        ] {
            assert!(validate_event_type(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn append_and_order() {
        let l = ledger();
        let t = task(&l, "ordering");
        for n in 0..5 {
            l.append_event(NewEvent {
                task_id: Some(t.id.clone()),
                source: "test".into(),
                event_type: "synthetic.tick".into(),
                payload: json!({ "n": n }),
                ..NewEvent::default()
            })
            .unwrap();
        }
        let trail = l.events_for_task(&t.id).unwrap();
        assert_eq!(trail.len(), 6); // task.created + 5 ticks
        assert!(trail.windows(2).all(|w| w[0].seq < w[1].seq));
        assert_eq!(trail[0].event_type, "task.created");
        let ticks: Vec<_> = trail[1..]
            .iter()
            .map(|e| e.payload["n"].as_i64().unwrap())
            .collect();
        assert_eq!(ticks, [0, 1, 2, 3, 4]);
        let recent = l.recent_events(2).unwrap();
        assert_eq!(recent[0].payload["n"], 4);
    }

    #[test]
    fn latest_and_counted_events_of_a_type() {
        let l = ledger();
        let t = task(&l, "typed");
        assert!(l
            .last_task_event(&t.id, "synthetic.tick")
            .unwrap()
            .is_none());
        for n in 0..3 {
            l.append_event(NewEvent {
                task_id: Some(t.id.clone()),
                source: "test".into(),
                event_type: "synthetic.tick".into(),
                payload: json!({ "n": n }),
                ..NewEvent::default()
            })
            .unwrap();
        }
        let last = l.last_task_event(&t.id, "synthetic.tick").unwrap().unwrap();
        assert_eq!(last.payload["n"], 2);
        assert_eq!(l.count_task_events(&t.id, "synthetic.tick").unwrap(), 3);
        assert_eq!(l.count_task_events(&t.id, "task.created").unwrap(), 1);
        assert_eq!(l.count_task_events("missing", "task.created").unwrap(), 0);
    }

    #[test]
    fn events_for_unknown_task_are_rejected() {
        let l = ledger();
        let err = l
            .append_event(NewEvent {
                task_id: Some("nope".into()),
                source: "test".into(),
                event_type: "synthetic.tick".into(),
                ..NewEvent::default()
            })
            .unwrap_err();
        assert!(matches!(err, LedgerError::NotFound(_)));
    }

    #[test]
    fn events_are_append_only_at_the_database_level() {
        let l = ledger();
        task(&l, "immutable");
        let c = l.conn();
        let update = c.execute("UPDATE events SET event_type = 'tampered.event'", []);
        assert!(update.unwrap_err().to_string().contains("append-only"));
        let delete = c.execute("DELETE FROM events", []);
        assert!(delete.unwrap_err().to_string().contains("append-only"));
    }

    #[test]
    fn listener_sees_committed_events_only() {
        use std::sync::{Arc, Mutex};
        let l = ledger();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        l.add_listener(Arc::new(move |e: &LedgerEvent| {
            sink.lock().unwrap().push(e.event_type.clone())
        }));
        let t = task(&l, "listened");
        // A rejected write commits nothing (the rejection itself is recorded separately).
        let _ = l.append_event(NewEvent {
            source: "test".into(),
            event_type: "BAD".into(),
            ..NewEvent::default()
        });
        l.transition_task(&t.id, crate::TaskState::Running, "test", None)
            .unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            ["task.created", "task.state_changed"]
        );
    }
}
