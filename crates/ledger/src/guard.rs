//! Approvals of workers' actions (Phase 7, ADR-013): a pending approval pauses its task
//! (`awaitingApproval`) in the same transaction that records it, and the task continues
//! (`running`) when its last pending approval is settled.

use rusqlite::{params, OptionalExtension as _};
use serde_json::{json, Value};

use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::rows::{self, metadata_text, EVENT_COLUMNS};
use crate::{events, tasks, Ledger};

const APPROVAL_COLS: &str =
    "id, task_id, action_type, request_payload, state, requested_at, expires_at, resolved_at, resolved_by";

fn approval_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    Ok(Approval {
        id: r.get(0)?,
        task_id: r.get(1)?,
        action_type: r.get(2)?,
        request_payload: rows::json(r.get(3)?),
        state: rows::parse_enum(4, r.get(4)?, ApprovalState::parse)?,
        requested_at: rows::u64_of(r.get(5)?),
        expires_at: rows::opt_u64(r.get(6)?),
        resolved_at: rows::opt_u64(r.get(7)?),
        resolved_by: r.get(8)?,
    })
}

fn get(tx: &rusqlite::Connection, id: &str) -> Result<Option<Approval>> {
    Ok(tx
        .query_row(
            &format!("SELECT {APPROVAL_COLS} FROM approvals WHERE id = ?1"),
            [id],
            approval_row,
        )
        .optional()?)
}

fn pending_for_task(tx: &rusqlite::Connection, task_id: &str) -> Result<i64> {
    Ok(tx.query_row(
        "SELECT COUNT(*) FROM approvals WHERE task_id = ?1 AND state = 'pending'",
        [task_id],
        |r| r.get(0),
    )?)
}

impl Ledger {
    /// Ask the owner to approve an action of a running task: records the approval and moves
    /// the task to `awaitingApproval` in one transaction. `payload` is what the approval card
    /// shows (already free of secrets); it is also recorded in the `approval.requested` event.
    pub fn request_action_approval(
        &self,
        task_id: &str,
        action_type: &str,
        payload: &Value,
        expires_at: u64,
        actor: &str,
    ) -> Result<Approval> {
        let payload_text = metadata_text(payload)?;
        if action_type.is_empty() || action_type.len() > 100 {
            return Err(LedgerError::InvalidInput("invalid action type".into()));
        }
        self.write(|tx, out| {
            let task = tasks::require(tx, task_id)?;
            match task.state {
                TaskState::Running | TaskState::AwaitingApproval => {}
                other => {
                    return Err(LedgerError::InvalidInput(format!(
                        "task {task_id} is {} and cannot wait for an approval",
                        other.as_str()
                    )))
                }
            }
            let id = uuid::Uuid::new_v4().to_string();
            let now = crate::now_ms();
            tx.execute(
                "INSERT INTO approvals (id, task_id, action_type, request_payload, state, requested_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
                params![id, task_id, action_type, payload_text, now as i64, expires_at as i64],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(task_id.into()),
                    source: actor.into(),
                    event_type: "approval.requested".into(),
                    payload: json!({
                        "approvalId": id,
                        "actionType": action_type,
                        "expiresAt": expires_at,
                        "request": payload,
                    }),
                    ..NewEvent::default()
                },
            )?);
            if task.state == TaskState::Running {
                let summary = payload["summary"].as_str().unwrap_or(action_type);
                tasks::transition(
                    tx,
                    out,
                    task_id,
                    TaskState::AwaitingApproval,
                    actor,
                    Some(&format!("Waiting for your approval: {summary}")),
                )?;
            }
            get(tx, &id)?.ok_or_else(|| LedgerError::NotFound(format!("approval {id}")))
        })
    }

    /// Settle a pending approval: `Approved` or `Rejected` by `resolved_by`, or `Expired`
    /// (`resolved_by` then records who noticed, e.g. `plenipo`). `note` is recorded with it.
    /// When its task is waiting for approval and nothing else of it is pending, the task
    /// continues (`running`). A settled approval cannot change again.
    pub fn settle_approval(
        &self,
        id: &str,
        state: ApprovalState,
        resolved_by: &str,
        note: Option<&str>,
    ) -> Result<Approval> {
        if state == ApprovalState::Pending {
            return Err(LedgerError::InvalidInput(
                "an approval can only be settled as approved, rejected, or expired".into(),
            ));
        }
        self.write(|tx, out| {
            let current =
                get(tx, id)?.ok_or_else(|| LedgerError::NotFound(format!("approval {id}")))?;
            if current.state != ApprovalState::Pending {
                return Err(LedgerError::InvalidTransition {
                    entity: "approval",
                    id: id.into(),
                    from: current.state.as_str().into(),
                    to: state.as_str().into(),
                });
            }
            let now = crate::now_ms();
            let by = (state != ApprovalState::Expired).then_some(resolved_by);
            tx.execute(
                "UPDATE approvals SET state = ?2, resolved_at = ?3, resolved_by = ?4 WHERE id = ?1",
                params![id, state.as_str(), now as i64, by],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(current.task_id.clone()),
                    source: resolved_by.into(),
                    event_type: if state == ApprovalState::Expired {
                        "approval.expired".into()
                    } else {
                        "approval.resolved".into()
                    },
                    payload: json!({
                        "approvalId": id,
                        "actionType": current.action_type,
                        "state": state,
                        "note": note,
                        "summary": current.request_payload["summary"],
                    }),
                    ..NewEvent::default()
                },
            )?);
            let task = tasks::require(tx, &current.task_id)?;
            if task.state == TaskState::AwaitingApproval && pending_for_task(tx, &task.id)? == 0 {
                let why = match state {
                    ApprovalState::Approved => "Approved; continuing",
                    ApprovalState::Rejected => "Not approved; continuing without it",
                    _ => "The approval expired; continuing without it",
                };
                tasks::transition(
                    tx,
                    out,
                    &task.id,
                    TaskState::Running,
                    resolved_by,
                    Some(why),
                )?;
            }
            get(tx, id)?.ok_or_else(|| LedgerError::NotFound(format!("approval {id}")))
        })
    }

    pub fn approval(&self, id: &str) -> Result<Option<Approval>> {
        self.read(|c| get(c, id))
    }

    /// Pending approvals, oldest first.
    pub fn pending_approvals(&self) -> Result<Vec<Approval>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {APPROVAL_COLS} FROM approvals WHERE state = 'pending'
                 ORDER BY requested_at, rowid"
            ))?;
            let rows = stmt
                .query_map([], approval_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Settled approvals, newest first.
    pub fn settled_approvals(&self, limit: u32) -> Result<Vec<Approval>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {APPROVAL_COLS} FROM approvals WHERE state != 'pending'
                 ORDER BY resolved_at DESC, rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 1000)], approval_row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Mark every pending approval expired without touching its task (Plenipo stopped while
    /// they waited; the tasks themselves are recovered by the agent runtime). Returns how many.
    pub fn expire_pending_approvals(&self, reason: &str, actor: &str) -> Result<usize> {
        self.write(|tx, out| {
            let pending: Vec<Approval> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT {APPROVAL_COLS} FROM approvals WHERE state = 'pending'"
                ))?;
                let rows = stmt
                    .query_map([], approval_row)?
                    .collect::<rusqlite::Result<_>>()?;
                rows
            };
            let now = crate::now_ms() as i64;
            for a in &pending {
                tx.execute(
                    "UPDATE approvals SET state = 'expired', resolved_at = ?2 WHERE id = ?1",
                    params![a.id, now],
                )?;
                out.push(events::insert(
                    tx,
                    NewEvent {
                        task_id: Some(a.task_id.clone()),
                        source: actor.into(),
                        event_type: "approval.expired".into(),
                        payload: json!({
                            "approvalId": a.id,
                            "actionType": a.action_type,
                            "state": ApprovalState::Expired,
                            "note": reason,
                            "summary": a.request_payload["summary"],
                        }),
                        ..NewEvent::default()
                    },
                )?);
            }
            Ok(pending.len())
        })
    }

    /// The most recent events of the given types, newest first.
    pub fn events_of_types(&self, types: &[&str], limit: u32) -> Result<Vec<LedgerEvent>> {
        if types.is_empty() {
            return Ok(Vec::new());
        }
        self.read(|c| {
            let marks = vec!["?"; types.len()].join(", ");
            let mut stmt = c.prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM events WHERE event_type IN ({marks})
                 ORDER BY seq DESC LIMIT {}",
                limit.clamp(1, 1000)
            ))?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(types.iter()), rows::event)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn running(l: &Ledger) -> Task {
        let t = task(l, "work");
        l.transition_task(&t.id, TaskState::Running, "w", None)
            .unwrap()
    }

    fn states(l: &Ledger, task_id: &str) -> Vec<String> {
        l.events_for_task(task_id)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == "task.state_changed")
            .map(|e| e.payload["to"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn an_approval_pauses_its_task_until_the_last_one_is_settled() {
        let l = ledger();
        let t = running(&l);
        let payload = json!({ "summary": "run npm publish" });
        let a = l
            .request_action_approval(&t.id, "shell.exec", &payload, u64::MAX / 2, "agent:x")
            .unwrap();
        let b = l
            .request_action_approval(&t.id, "git.write", &payload, u64::MAX / 2, "agent:x")
            .unwrap();
        assert_eq!(
            l.task(&t.id).unwrap().unwrap().state,
            TaskState::AwaitingApproval
        );
        assert_eq!(l.pending_approvals().unwrap().len(), 2);

        let a = l
            .settle_approval(&a.id, ApprovalState::Approved, "owner", None)
            .unwrap();
        assert_eq!(a.resolved_by.as_deref(), Some("owner"));
        assert_eq!(
            l.task(&t.id).unwrap().unwrap().state,
            TaskState::AwaitingApproval,
            "another approval is still pending"
        );
        l.settle_approval(&b.id, ApprovalState::Rejected, "owner", Some("no"))
            .unwrap();
        assert_eq!(l.task(&t.id).unwrap().unwrap().state, TaskState::Running);
        assert!(matches!(
            l.settle_approval(&b.id, ApprovalState::Approved, "owner", None),
            Err(LedgerError::InvalidTransition { .. })
        ));
        assert_eq!(
            states(&l, &t.id),
            ["running", "awaitingApproval", "running"]
        );
        let settled = l.settled_approvals(10).unwrap();
        assert_eq!(settled.len(), 2);
        let events = l
            .events_of_types(&["approval.requested", "approval.resolved"], 10)
            .unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].payload["note"], "no");
        assert_eq!(events[3].payload["request"]["summary"], "run npm publish");
    }

    #[test]
    fn expiry_and_recovery() {
        let l = ledger();
        let t = running(&l);
        let a = l
            .request_action_approval(&t.id, "shell.exec", &json!({}), 1, "agent:x")
            .unwrap();
        let e = l
            .settle_approval(&a.id, ApprovalState::Expired, "plenipo", Some("no answer"))
            .unwrap();
        assert_eq!((e.state, e.resolved_by), (ApprovalState::Expired, None));
        assert_eq!(l.task(&t.id).unwrap().unwrap().state, TaskState::Running);

        // Left pending when Plenipo stopped: expired, the task left for the runtime to recover.
        l.request_action_approval(&t.id, "shell.exec", &json!({}), 5, "agent:x")
            .unwrap();
        assert_eq!(
            l.expire_pending_approvals("Plenipo restarted", "plenipo")
                .unwrap(),
            1
        );
        assert!(l.pending_approvals().unwrap().is_empty());
        assert_eq!(
            l.task(&t.id).unwrap().unwrap().state,
            TaskState::AwaitingApproval
        );
        l.transition_task(&t.id, TaskState::Failed, "plenipo", None)
            .unwrap();
    }

    #[test]
    fn only_a_running_task_can_wait() {
        let l = ledger();
        let t = task(&l, "queued");
        assert!(l
            .request_action_approval(&t.id, "x.y", &json!({}), 5, "a")
            .is_err());
        assert!(matches!(
            l.request_action_approval("missing", "x.y", &json!({}), 5, "a"),
            Err(LedgerError::NotFound(_))
        ));
        let r = running(&l);
        let a = l
            .request_action_approval(&r.id, "x.y", &json!({}), 5, "a")
            .unwrap();
        assert!(l
            .settle_approval(&a.id, ApprovalState::Pending, "o", None)
            .is_err());
    }
}
