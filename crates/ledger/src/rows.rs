//! Row mapping helpers.

use rusqlite::Row;
use serde_json::Value;

use crate::dto::*;

pub(crate) const TASK_COLUMNS: &str = "id, parent_task_id, requested_by, assigned_to, project_id, \
    objective, acceptance_criteria, priority, state, metadata, created_at, updated_at, \
    started_at, completed_at";

pub(crate) const EVENT_COLUMNS: &str =
    "seq, id, task_id, execution_id, source, destination, event_type, payload, created_at";

pub(crate) fn json(text: String) -> Value {
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

pub(crate) fn opt_u64(v: Option<i64>) -> Option<u64> {
    v.map(|n| u64::try_from(n).unwrap_or(0))
}

pub(crate) fn u64_of(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

/// Convert a stored enum string, failing the row read if the value is unknown.
pub(crate) fn parse_enum<T>(
    idx: usize,
    value: String,
    parse: impl Fn(&str) -> Option<T>,
) -> rusqlite::Result<T> {
    parse(&value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            format!("unknown value {value:?}").into(),
        )
    })
}

pub(crate) fn task(r: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        parent_task_id: r.get(1)?,
        requested_by: r.get(2)?,
        assigned_to: r.get(3)?,
        project_id: r.get(4)?,
        objective: r.get(5)?,
        acceptance_criteria: r.get(6)?,
        priority: r.get(7)?,
        state: parse_enum(8, r.get(8)?, TaskState::parse)?,
        metadata: json(r.get(9)?),
        created_at: u64_of(r.get(10)?),
        updated_at: u64_of(r.get(11)?),
        started_at: opt_u64(r.get(12)?),
        completed_at: opt_u64(r.get(13)?),
    })
}

pub(crate) fn event(r: &Row<'_>) -> rusqlite::Result<LedgerEvent> {
    Ok(LedgerEvent {
        seq: u64_of(r.get(0)?),
        id: r.get(1)?,
        task_id: r.get(2)?,
        execution_id: r.get(3)?,
        source: r.get(4)?,
        destination: r.get(5)?,
        event_type: r.get(6)?,
        payload: json(r.get(7)?),
        created_at: u64_of(r.get(8)?),
    })
}

/// `metadata` must be a JSON object; `Null` means an empty object.
pub(crate) fn metadata_text(value: &Value) -> crate::Result<String> {
    match value {
        Value::Null => Ok("{}".into()),
        Value::Object(_) => Ok(value.to_string()),
        _ => Err(crate::LedgerError::InvalidInput(
            "metadata must be a JSON object".into(),
        )),
    }
}
