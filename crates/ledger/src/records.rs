//! Executions, approvals, and artifacts.

use rusqlite::{params, OptionalExtension as _};
use serde_json::{json, Value};

use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::events;
use crate::rows::{json as parse_json, metadata_text, opt_u64, parse_enum, u64_of};
use crate::Ledger;

const EXEC_COLS: &str =
    "id, task_id, runtime, provider, model, session_id, process_id, profile_id, label, \
    executable, args, working_dir, state, exit_code, detail, started_at, ended_at, usage_metadata";

fn exec_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionRow> {
    Ok(ExecutionRow {
        id: r.get(0)?,
        task_id: r.get(1)?,
        runtime: r.get(2)?,
        provider: r.get(3)?,
        model: r.get(4)?,
        session_id: r.get(5)?,
        process_id: r.get(6)?,
        profile_id: r.get(7)?,
        label: r.get(8)?,
        executable: r.get(9)?,
        args: serde_json::from_str(&r.get::<_, String>(10)?).unwrap_or_default(),
        working_dir: r.get(11)?,
        state: r.get(12)?,
        exit_code: r.get(13)?,
        detail: r.get(14)?,
        started_at: u64_of(r.get(15)?),
        ended_at: opt_u64(r.get(16)?),
        usage_metadata: parse_json(r.get(17)?),
    })
}

const APPROVAL_COLS: &str =
    "id, task_id, action_type, request_payload, state, requested_at, expires_at, resolved_at, resolved_by";

fn approval_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    Ok(Approval {
        id: r.get(0)?,
        task_id: r.get(1)?,
        action_type: r.get(2)?,
        request_payload: parse_json(r.get(3)?),
        state: parse_enum(4, r.get(4)?, ApprovalState::parse)?,
        requested_at: u64_of(r.get(5)?),
        expires_at: opt_u64(r.get(6)?),
        resolved_at: opt_u64(r.get(7)?),
        resolved_by: r.get(8)?,
    })
}

const ARTIFACT_COLS: &str =
    "id, task_id, artifact_type, local_path, uri, hash, metadata, created_at";

fn artifact_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: r.get(0)?,
        task_id: r.get(1)?,
        artifact_type: r.get(2)?,
        local_path: r.get(3)?,
        uri: r.get(4)?,
        hash: r.get(5)?,
        metadata: parse_json(r.get(6)?),
        created_at: u64_of(r.get(7)?),
    })
}

/// `timedOut` → `timed_out` for event names.
fn snake(state: &str) -> String {
    let mut out = String::new();
    for c in state.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Decision on a pending approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Reject,
}

impl Ledger {
    // ---- Executions ---------------------------------------------------------------------

    /// Insert or update an execution. A new record or a state change writes
    /// `execution.<state>` to the trail in the same transaction.
    pub fn upsert_execution(&self, row: &ExecutionRow, actor: &str) -> Result<()> {
        if !EXECUTION_STATES.contains(&row.state.as_str()) {
            return Err(LedgerError::InvalidInput(format!(
                "unknown execution state {:?}",
                row.state
            )));
        }
        let usage = match &row.usage_metadata {
            Value::Null => "{}".to_owned(),
            v => v.to_string(),
        };
        self.write(|tx, out| {
            let previous: Option<String> = tx
                .query_row("SELECT state FROM executions WHERE id = ?1", [&row.id], |r| r.get(0))
                .optional()?;
            tx.execute(
                "INSERT INTO executions (id, task_id, runtime, provider, model, session_id, process_id,
                     profile_id, label, executable, args, working_dir, state, exit_code, detail,
                     started_at, ended_at, usage_metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                 ON CONFLICT (id) DO UPDATE SET
                     task_id = excluded.task_id, provider = excluded.provider, model = excluded.model,
                     session_id = excluded.session_id, process_id = excluded.process_id,
                     state = excluded.state, exit_code = excluded.exit_code, detail = excluded.detail,
                     ended_at = excluded.ended_at, usage_metadata = excluded.usage_metadata",
                params![
                    row.id,
                    row.task_id,
                    row.runtime,
                    row.provider,
                    row.model,
                    row.session_id,
                    row.process_id,
                    row.profile_id,
                    row.label,
                    row.executable,
                    serde_json::to_string(&row.args)?,
                    row.working_dir,
                    row.state,
                    row.exit_code,
                    row.detail,
                    row.started_at as i64,
                    row.ended_at.map(|v| v as i64),
                    usage
                ],
            )?;
            if previous.as_deref() != Some(row.state.as_str()) {
                out.push(events::insert(
                    tx,
                    NewEvent {
                        task_id: row.task_id.clone(),
                        execution_id: Some(row.id.clone()),
                        source: actor.into(),
                        event_type: format!("execution.{}", snake(&row.state)),
                        payload: json!({
                            "label": row.label,
                            "profileId": row.profile_id,
                            "runtime": row.runtime,
                            "provider": row.provider,
                            "model": row.model,
                            "pid": row.process_id,
                            "exitCode": row.exit_code,
                            "detail": row.detail,
                        }),
                        ..NewEvent::default()
                    },
                )?);
            }
            Ok(())
        })
    }

    pub fn execution(&self, id: &str) -> Result<Option<ExecutionRow>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!("SELECT {EXEC_COLS} FROM executions WHERE id = ?1"),
                [id],
                exec_row,
            )
            .optional()?)
        })
    }

    /// A task's executions, oldest first.
    pub fn executions_for_task(&self, task_id: &str) -> Result<Vec<ExecutionRow>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {EXEC_COLS} FROM executions WHERE task_id = ?1 ORDER BY started_at, rowid"
            ))?;
            let rows = stmt
                .query_map([task_id], exec_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Most recent executions, newest first.
    pub fn recent_executions(&self, limit: u32) -> Result<Vec<ExecutionRow>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {EXEC_COLS} FROM executions ORDER BY started_at DESC, rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 10_000)], exec_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    // ---- Approvals ----------------------------------------------------------------------

    pub fn request_approval(
        &self,
        task_id: &str,
        action_type: &str,
        payload: &Value,
        expires_at: Option<u64>,
        actor: &str,
    ) -> Result<Approval> {
        let payload_text = metadata_text(payload)?;
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO approvals (id, task_id, action_type, request_payload, state, requested_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
                params![id, task_id, action_type, payload_text, crate::now_ms() as i64, expires_at.map(|v| v as i64)],
            )
            .map_err(|e| {
                if e.to_string().contains("FOREIGN KEY") {
                    LedgerError::NotFound(format!("task {task_id}"))
                } else {
                    e.into()
                }
            })?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(task_id.into()),
                    source: actor.into(),
                    event_type: "approval.requested".into(),
                    payload: json!({ "approvalId": id, "actionType": action_type, "expiresAt": expires_at }),
                    ..NewEvent::default()
                },
            )?);
            tx.query_row(&format!("SELECT {APPROVAL_COLS} FROM approvals WHERE id = ?1"), [&id], approval_row)
                .map_err(Into::into)
        })
    }

    /// Approve or reject a pending approval. An approval past `expires_at` is marked
    /// `expired` (and recorded) instead, and the decision is refused.
    pub fn resolve_approval(
        &self,
        id: &str,
        decision: Decision,
        resolved_by: &str,
    ) -> Result<Approval> {
        let now = crate::now_ms();
        let outcome = self.write(|tx, out| {
            let current = tx
                .query_row(&format!("SELECT {APPROVAL_COLS} FROM approvals WHERE id = ?1"), [id], approval_row)
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("approval {id}")))?;
            if current.state != ApprovalState::Pending {
                return Err(LedgerError::InvalidTransition {
                    entity: "approval",
                    id: id.into(),
                    from: current.state.as_str().into(),
                    to: match decision {
                        Decision::Approve => "approved",
                        Decision::Reject => "rejected",
                    }
                    .into(),
                });
            }
            let expired = current.expires_at.is_some_and(|t| t <= now);
            let next = match (expired, decision) {
                (true, _) => ApprovalState::Expired,
                (false, Decision::Approve) => ApprovalState::Approved,
                (false, Decision::Reject) => ApprovalState::Rejected,
            };
            tx.execute(
                "UPDATE approvals SET state = ?2, resolved_at = ?3, resolved_by = ?4 WHERE id = ?1",
                params![id, next.as_str(), now as i64, if expired { None } else { Some(resolved_by) }],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(current.task_id.clone()),
                    source: resolved_by.into(),
                    event_type: if expired { "approval.expired".into() } else { "approval.resolved".into() },
                    payload: json!({ "approvalId": id, "actionType": current.action_type, "state": next }),
                    ..NewEvent::default()
                },
            )?);
            let updated = tx
                .query_row(&format!("SELECT {APPROVAL_COLS} FROM approvals WHERE id = ?1"), [id], approval_row)?;
            Ok((updated, expired))
        })?;
        match outcome {
            (approval, false) => Ok(approval),
            (_, true) => Err(LedgerError::InvalidInput(format!(
                "approval {id} has expired"
            ))),
        }
    }

    pub fn approvals_for_task(&self, task_id: &str) -> Result<Vec<Approval>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {APPROVAL_COLS} FROM approvals WHERE task_id = ?1 ORDER BY requested_at, rowid"
            ))?;
            let rows = stmt.query_map([task_id], approval_row)?.collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    // ---- Artifacts ----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn record_artifact(
        &self,
        task_id: Option<&str>,
        artifact_type: &str,
        local_path: Option<&str>,
        uri: Option<&str>,
        hash: Option<&str>,
        metadata: &Value,
        actor: &str,
    ) -> Result<Artifact> {
        if local_path.is_none() && uri.is_none() {
            return Err(LedgerError::InvalidInput(
                "an artifact needs a local path or a URI".into(),
            ));
        }
        let metadata = metadata_text(metadata)?;
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO artifacts (id, task_id, artifact_type, local_path, uri, hash, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, task_id, artifact_type, local_path, uri, hash, metadata, crate::now_ms() as i64],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: task_id.map(Into::into),
                    source: actor.into(),
                    event_type: "artifact.recorded".into(),
                    payload: json!({ "artifactId": id, "type": artifact_type, "path": local_path, "uri": uri, "hash": hash }),
                    ..NewEvent::default()
                },
            )?);
            tx.query_row(&format!("SELECT {ARTIFACT_COLS} FROM artifacts WHERE id = ?1"), [&id], artifact_row)
                .map_err(Into::into)
        })
    }

    pub fn artifact(&self, id: &str) -> Result<Option<Artifact>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!("SELECT {ARTIFACT_COLS} FROM artifacts WHERE id = ?1"),
                [id],
                artifact_row,
            )
            .optional()?)
        })
    }

    pub fn artifacts_for_task(&self, task_id: &str) -> Result<Vec<Artifact>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {ARTIFACT_COLS} FROM artifacts WHERE task_id = ?1 ORDER BY created_at, rowid"
            ))?;
            let rows = stmt.query_map([task_id], artifact_row)?.collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn exec(id: &str, state: &str) -> ExecutionRow {
        ExecutionRow {
            id: id.into(),
            task_id: None,
            runtime: "local-process".into(),
            provider: None,
            model: None,
            session_id: None,
            process_id: Some(10),
            profile_id: Some("diagnostic.echo".into()),
            label: "Echo test".into(),
            executable: Some("/x".into()),
            args: vec!["--a".into()],
            working_dir: Some("/".into()),
            state: state.into(),
            exit_code: None,
            detail: None,
            started_at: 5,
            ended_at: None,
            usage_metadata: Value::Null,
        }
    }

    #[test]
    fn execution_upsert_records_state_changes_once() {
        let l = ledger();
        l.upsert_execution(&exec("e1", "starting"), "runtime")
            .unwrap();
        l.upsert_execution(&exec("e1", "running"), "runtime")
            .unwrap();
        l.upsert_execution(&exec("e1", "running"), "runtime")
            .unwrap(); // no change → no event
        let mut done = exec("e1", "timedOut");
        done.ended_at = Some(9);
        done.provider = Some("provider-x".into());
        done.model = Some("model-y".into());
        done.usage_metadata = json!({ "inputTokens": 10 });
        l.upsert_execution(&done, "runtime").unwrap();

        let stored = l.execution("e1").unwrap().unwrap();
        assert_eq!(stored.state, "timedOut");
        assert_eq!(stored.args, ["--a"]);
        assert_eq!(stored.model.as_deref(), Some("model-y"));
        assert_eq!(stored.usage_metadata["inputTokens"], 10);
        let types: Vec<_> = l
            .events_for_execution("e1")
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert_eq!(
            types,
            [
                "execution.starting",
                "execution.running",
                "execution.timed_out"
            ]
        );
        assert!(l
            .upsert_execution(&exec("e2", "exploded"), "runtime")
            .is_err());
        assert_eq!(l.recent_executions(10).unwrap().len(), 1);
    }

    #[test]
    fn approvals_lifecycle() {
        let l = ledger();
        let t = task(&l, "needs approval");
        let a = l
            .request_approval(
                &t.id,
                "production.deploy",
                &json!({ "target": "prod" }),
                None,
                "worker",
            )
            .unwrap();
        assert_eq!(a.state, ApprovalState::Pending);
        let a = l
            .resolve_approval(&a.id, Decision::Approve, "owner")
            .unwrap();
        assert_eq!(a.state, ApprovalState::Approved);
        assert_eq!(a.resolved_by.as_deref(), Some("owner"));
        assert!(matches!(
            l.resolve_approval(&a.id, Decision::Reject, "owner"),
            Err(LedgerError::InvalidTransition { .. })
        ));

        let r = l
            .request_approval(&t.id, "dns.change", &Value::Null, None, "worker")
            .unwrap();
        assert_eq!(
            l.resolve_approval(&r.id, Decision::Reject, "owner")
                .unwrap()
                .state,
            ApprovalState::Rejected
        );

        let e = l
            .request_approval(&t.id, "db.drop", &Value::Null, Some(1), "worker")
            .unwrap();
        assert!(matches!(
            l.resolve_approval(&e.id, Decision::Approve, "owner"),
            Err(LedgerError::InvalidInput(_))
        ));
        let stored = l.approvals_for_task(&t.id).unwrap();
        assert_eq!(stored[2].state, ApprovalState::Expired);
        assert_eq!(stored[2].resolved_by, None);

        let trail: Vec<_> = l
            .events_for_task(&t.id)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert_eq!(
            trail,
            [
                "task.created",
                "approval.requested",
                "approval.resolved",
                "approval.requested",
                "approval.resolved",
                "approval.requested",
                "approval.expired"
            ]
        );
        assert!(matches!(
            l.request_approval("missing", "x", &Value::Null, None, "w"),
            Err(LedgerError::NotFound(_))
        ));
    }

    #[test]
    fn artifacts() {
        let l = ledger();
        let t = task(&l, "produces files");
        let a = l
            .record_artifact(
                Some(&t.id),
                "file",
                Some("C:/out/report.md"),
                None,
                Some("sha256:abc"),
                &Value::Null,
                "worker",
            )
            .unwrap();
        assert_eq!(
            l.artifacts_for_task(&t.id).unwrap(),
            std::slice::from_ref(&a)
        );
        assert_eq!(l.artifact(&a.id).unwrap(), Some(a));
        assert!(l.artifact("missing").unwrap().is_none());
        assert!(l
            .record_artifact(Some(&t.id), "file", None, None, None, &Value::Null, "w")
            .is_err());
        assert_eq!(
            l.events_for_task(&t.id).unwrap().last().unwrap().event_type,
            "artifact.recorded"
        );
    }
}
