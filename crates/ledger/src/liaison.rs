//! Liaison messages (Phase 4, ADR-008): handoff requests between tasks and their replies.
//!
//! Liaison decides *what* happens; this module records it. Each operation is one transaction
//! that writes its `liaison.*` events on the affected tasks together with the task and message
//! changes, and every message state change is compare-and-set, so repeating an operation (a
//! replayed turn end, a second reconcile pass) changes nothing.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use crate::dto::{
    CancelOutcome, HandoffDecision, LedgerEvent, LiaisonMessage, MessageKind, MessageState,
    NewEvent, NewHandoffRequest, NewReply, OpenRequest, ReplyOutcome, Suspended, Task, TaskState,
};
use crate::error::{LedgerError, Result};
use crate::rows::{self, json as parse_json, parse_enum, u64_of, TASK_COLUMNS};
use crate::{events, tasks, Ledger};

const MESSAGE_COLUMNS: &str = "id, correlation_id, kind, in_reply_to, task_id, child_task_id, \
    source, destination, state, dedupe_key, envelope, created_at, updated_at";

fn message_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LiaisonMessage> {
    Ok(LiaisonMessage {
        id: r.get(0)?,
        correlation_id: r.get(1)?,
        kind: parse_enum(2, r.get(2)?, MessageKind::parse)?,
        in_reply_to: r.get(3)?,
        task_id: r.get(4)?,
        child_task_id: r.get(5)?,
        source: r.get(6)?,
        destination: r.get(7)?,
        state: parse_enum(8, r.get(8)?, MessageState::parse)?,
        dedupe_key: r.get(9)?,
        envelope: parse_json(r.get(10)?),
        created_at: u64_of(r.get(11)?),
        updated_at: u64_of(r.get(12)?),
    })
}

fn get(conn: &Connection, id: &str) -> Result<Option<LiaisonMessage>> {
    Ok(conn
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM liaison_messages WHERE id = ?1"),
            [id],
            message_row,
        )
        .optional()?)
}

fn require(conn: &Connection, id: &str) -> Result<LiaisonMessage> {
    get(conn, id)?.ok_or_else(|| LedgerError::NotFound(format!("liaison message {id}")))
}

fn by_dedupe(conn: &Connection, key: &str) -> Result<Option<LiaisonMessage>> {
    Ok(conn
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM liaison_messages WHERE dedupe_key = ?1"),
            [key],
            message_row,
        )
        .optional()?)
}

fn reply_to(conn: &Connection, request_id: &str) -> Result<Option<LiaisonMessage>> {
    Ok(conn
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM liaison_messages WHERE in_reply_to = ?1"),
            [request_id],
            message_row,
        )
        .optional()?)
}

fn request_for_child(conn: &Connection, child_task_id: &str) -> Result<Option<LiaisonMessage>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {MESSAGE_COLUMNS} FROM liaison_messages
                 WHERE kind = 'request' AND child_task_id = ?1"
            ),
            [child_task_id],
            message_row,
        )
        .optional()?)
}

fn all(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<LiaisonMessage>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(params, message_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// `[0-9A-Za-z._:-]{1,64}`: message and correlation IDs.
fn validate_id(what: &str, id: &str) -> Result<()> {
    let ok = (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if ok {
        Ok(())
    } else {
        Err(LedgerError::InvalidInput(format!("invalid {what} {id:?}")))
    }
}

fn validate_text(what: &str, value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max {
        Err(LedgerError::InvalidInput(format!(
            "{what} must be 1–{max} characters"
        )))
    } else {
        Ok(())
    }
}

fn object(what: &str, value: &Value) -> Result<String> {
    if value.is_object() {
        Ok(value.to_string())
    } else {
        Err(LedgerError::InvalidInput(format!(
            "{what} must be a JSON object"
        )))
    }
}

/// `base` (a JSON object, or null) with `extra`'s fields added.
fn payload(base: &Value, extra: Value) -> Value {
    let mut out = base.as_object().cloned().unwrap_or_default();
    if let Value::Object(extra) = extra {
        out.extend(extra);
    }
    Value::Object(out)
}

fn event(task_id: &str, actor: &str, event_type: &str, payload: Value) -> NewEvent {
    NewEvent {
        task_id: Some(task_id.into()),
        source: actor.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_message(
    tx: &Connection,
    id: &str,
    correlation_id: &str,
    kind: MessageKind,
    in_reply_to: Option<&str>,
    task_id: &str,
    child_task_id: Option<&str>,
    source: &str,
    destination: &str,
    state: MessageState,
    dedupe_key: &str,
    envelope: &Value,
) -> Result<LiaisonMessage> {
    validate_id("message id", id)?;
    validate_id("correlation id", correlation_id)?;
    validate_text("source", source, 200)?;
    validate_text("destination", destination, 200)?;
    validate_text("dedupe key", dedupe_key, 200)?;
    let envelope = object("envelope", envelope)?;
    let now = crate::now_ms() as i64;
    tx.execute(
        "INSERT INTO liaison_messages (id, correlation_id, kind, in_reply_to, task_id,
             child_task_id, source, destination, state, dedupe_key, envelope, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            id,
            correlation_id,
            kind.as_str(),
            in_reply_to,
            task_id,
            child_task_id,
            source,
            destination,
            state.as_str(),
            dedupe_key,
            envelope,
            now
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            LedgerError::InvalidInput(format!("liaison message rejected: {e}"))
        }
        other => other.into(),
    })?;
    require(tx, id)
}

/// Compare-and-set a message's state.
fn set_state(
    tx: &Connection,
    id: &str,
    from: &[MessageState],
    to: MessageState,
) -> Result<LiaisonMessage> {
    let current = require(tx, id)?;
    if !from.contains(&current.state) {
        return Err(LedgerError::InvalidTransition {
            entity: "liaison message",
            id: id.into(),
            from: current.state.as_str().into(),
            to: to.as_str().into(),
        });
    }
    tx.execute(
        "UPDATE liaison_messages SET state = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, to.as_str(), crate::now_ms() as i64],
    )?;
    require(tx, id)
}

/// Record a reply to `request` (inside a transaction), after checking that its claims match.
fn insert_reply(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    request: &LiaisonMessage,
    reply: &NewReply,
    actor: &str,
) -> Result<LiaisonMessage> {
    if reply.in_reply_to != request.id {
        return Err(LedgerError::InvalidInput(format!(
            "reply {} names request {:?}, not {}",
            reply.message_id, reply.in_reply_to, request.id
        )));
    }
    if reply.correlation_id != request.correlation_id {
        return Err(LedgerError::InvalidInput(format!(
            "reply {} carries correlation {:?}, but request {} belongs to {:?}",
            reply.message_id, reply.correlation_id, request.id, request.correlation_id
        )));
    }
    if reply.child_task_id.is_some() && reply.child_task_id != request.child_task_id {
        return Err(LedgerError::InvalidInput(format!(
            "reply {} comes from task {:?}, not from the request's child task",
            reply.message_id, reply.child_task_id
        )));
    }
    let message = insert_message(
        tx,
        &reply.message_id,
        &request.correlation_id,
        MessageKind::Reply,
        Some(&request.id),
        &request.task_id,
        reply.child_task_id.as_deref(),
        &reply.source,
        &request.source,
        MessageState::Pending,
        &format!("reply:{}", request.id),
        &reply.envelope,
    )?;
    let ids = json!({
        "messageId": message.id,
        "inReplyTo": request.id,
        "correlationId": request.correlation_id,
        "childTaskId": reply.child_task_id,
    });
    if let Some(child) = &reply.child_task_id {
        out.push(events::insert(
            tx,
            event(
                child,
                actor,
                "liaison.reply_sent",
                payload(&reply.summary, ids.clone()),
            ),
        )?);
    }
    out.push(events::insert(
        tx,
        event(
            &request.task_id,
            actor,
            "liaison.reply_received",
            payload(&reply.summary, ids),
        ),
    )?);
    Ok(message)
}

impl Ledger {
    /// Record a step's handoff requests and move the requesting task from `running` to
    /// `blocked` — one transaction: `step_result` (the step's `agent.result`), then per request
    /// `liaison.handoff_requested` and either a queued child task (`liaison.handoff_received` on
    /// it) or a refusal (`liaison.handoff_rejected` and a pending reply), then the state change.
    ///
    /// Requests whose dedupe key is already recorded are not recorded again; when all of them
    /// are, nothing changes and `replayed` is set.
    pub fn suspend_for_handoffs(
        &self,
        task_id: &str,
        step_result: NewEvent,
        requests: Vec<NewHandoffRequest>,
        reason: &str,
        actor: &str,
    ) -> Result<Suspended> {
        if requests.is_empty() {
            return Err(LedgerError::InvalidInput("no handoff requests".into()));
        }
        let result = self.write(|tx, out| {
            let task = tasks::require(tx, task_id)?;
            let mut known = Vec::new();
            for r in &requests {
                if let Some(existing) = by_dedupe(tx, &r.dedupe_key)? {
                    if existing.task_id != task_id || existing.kind != MessageKind::Request {
                        return Err(LedgerError::InvalidInput(format!(
                            "dedupe key {:?} belongs to another request",
                            r.dedupe_key
                        )));
                    }
                    known.push(existing);
                }
            }
            if known.len() == requests.len() {
                return Ok(Suspended {
                    task,
                    requests: known,
                    children: Vec::new(),
                    replayed: true,
                });
            }
            if task.state != TaskState::Running {
                return Err(LedgerError::InvalidTransition {
                    entity: "task",
                    id: task_id.into(),
                    from: task.state.as_str().into(),
                    to: TaskState::Blocked.as_str().into(),
                });
            }
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(task_id.into()),
                    ..step_result
                },
            )?);
            let mut recorded = Vec::new();
            let mut children = Vec::new();
            for r in requests {
                if let Some(existing) = by_dedupe(tx, &r.dedupe_key)? {
                    recorded.push(existing);
                    continue;
                }
                object("handoff summary", &r.summary)?;
                let ids = json!({
                    "messageId": r.message_id,
                    "correlationId": r.correlation_id,
                    "destination": r.destination,
                });
                out.push(events::insert(
                    tx,
                    event(
                        task_id,
                        actor,
                        "liaison.handoff_requested",
                        payload(&r.summary, ids.clone()),
                    ),
                )?);
                match r.decision {
                    HandoffDecision::Accept { child, received } => {
                        if child.parent_task_id.as_deref() != Some(task_id) {
                            return Err(LedgerError::InvalidInput(
                                "a handoff's child task must name the requesting task as its parent"
                                    .into(),
                            ));
                        }
                        let child = tasks::insert(tx, out, child, actor)?;
                        let message = insert_message(
                            tx,
                            &r.message_id,
                            &r.correlation_id,
                            MessageKind::Request,
                            None,
                            task_id,
                            Some(&child.id),
                            &r.source,
                            &r.destination,
                            MessageState::Accepted,
                            &r.dedupe_key,
                            &r.envelope,
                        )?;
                        out.push(events::insert(
                            tx,
                            event(
                                &child.id,
                                actor,
                                "liaison.handoff_received",
                                payload(
                                    &received,
                                    json!({
                                        "messageId": r.message_id,
                                        "correlationId": r.correlation_id,
                                        "parentTaskId": task_id,
                                        "source": r.source,
                                    }),
                                ),
                            ),
                        )?);
                        recorded.push(message);
                        children.push(tasks::require(tx, &child.id)?);
                    }
                    HandoffDecision::Reject { reason, reply } => {
                        let message = insert_message(
                            tx,
                            &r.message_id,
                            &r.correlation_id,
                            MessageKind::Request,
                            None,
                            task_id,
                            None,
                            &r.source,
                            &r.destination,
                            MessageState::Rejected,
                            &r.dedupe_key,
                            &r.envelope,
                        )?;
                        out.push(events::insert(
                            tx,
                            event(
                                task_id,
                                actor,
                                "liaison.handoff_rejected",
                                payload(&ids, json!({ "reason": reason })),
                            ),
                        )?);
                        insert_reply(tx, out, &message, &reply, actor)?;
                        recorded.push(message);
                    }
                }
            }
            let task =
                tasks::transition(tx, out, task_id, TaskState::Blocked, actor, Some(reason))?;
            Ok(Suspended {
                task,
                requests: recorded,
                children,
                replayed: false,
            })
        });
        self.record_rejection(task_id, actor, Some(reason), &result);
        result
    }

    /// A worker session starts the child task of an accepted request: the request becomes
    /// `dispatched` and the child `running`, with `liaison.dispatched` on the child.
    pub fn begin_handoff_turn(
        &self,
        child_task_id: &str,
        session_id: &str,
        actor: &str,
    ) -> Result<Task> {
        let result = self.write(|tx, out| {
            let child = tasks::require(tx, child_task_id)?;
            if child.metadata.get("sessionId").and_then(Value::as_str) != Some(session_id) {
                return Err(LedgerError::InvalidInput(format!(
                    "task {child_task_id} is not assigned to session {session_id}"
                )));
            }
            let request = request_for_child(tx, child_task_id)?.ok_or_else(|| {
                LedgerError::NotFound(format!("handoff request for task {child_task_id}"))
            })?;
            if request.state != MessageState::Accepted {
                return Err(LedgerError::InvalidInput(format!(
                    "the handoff for task {child_task_id} is {}",
                    request.state.as_str()
                )));
            }
            set_state(
                tx,
                &request.id,
                &[MessageState::Accepted],
                MessageState::Dispatched,
            )?;
            out.push(events::insert(
                tx,
                event(
                    child_task_id,
                    actor,
                    "liaison.dispatched",
                    json!({
                        "messageId": request.id,
                        "correlationId": request.correlation_id,
                        "sessionId": session_id,
                    }),
                ),
            )?);
            tasks::transition(
                tx,
                out,
                child_task_id,
                TaskState::Running,
                actor,
                Some("handoff dispatched"),
            )
        });
        self.record_rejection(child_task_id, actor, Some("handoff dispatched"), &result);
        result
    }

    /// The child of an accepted request could not be started: the child fails with `reason`
    /// (`liaison.dispatch_failed` on it). The request stays open until it is answered.
    pub fn fail_handoff_dispatch(
        &self,
        request_id: &str,
        reason: &str,
        detail: Value,
        actor: &str,
    ) -> Result<Task> {
        self.write(|tx, out| {
            let request = require(tx, request_id)?;
            let child = match (&request.kind, request.state, &request.child_task_id) {
                (MessageKind::Request, MessageState::Accepted, Some(child)) => child.clone(),
                _ => {
                    return Err(LedgerError::InvalidInput(format!(
                        "handoff {request_id} is not waiting to be dispatched"
                    )))
                }
            };
            out.push(events::insert(
                tx,
                event(
                    &child,
                    actor,
                    "liaison.dispatch_failed",
                    payload(
                        &detail,
                        json!({
                            "messageId": request.id,
                            "correlationId": request.correlation_id,
                            "reason": reason,
                        }),
                    ),
                ),
            )?);
            tasks::transition(tx, out, &child, TaskState::Failed, actor, Some(reason))
        })
    }

    /// Answer an open request. The request becomes `answered` and the reply `pending`, with
    /// `liaison.reply_sent` on the child and `liaison.reply_received` on the requester. A reply
    /// whose claims do not match the request is refused and recorded as
    /// `liaison.reply_refused` on the requesting task.
    pub fn answer_request(&self, reply: NewReply, actor: &str) -> Result<ReplyOutcome> {
        let result = self.write(|tx, out| {
            let request = require(tx, &reply.in_reply_to)?;
            if request.kind != MessageKind::Request {
                return Err(LedgerError::InvalidInput(format!(
                    "{} is a reply, not a request",
                    request.id
                )));
            }
            if let Some(existing) = reply_to(tx, &request.id)? {
                return Ok(ReplyOutcome::AlreadyAnswered(existing));
            }
            if !request.state.is_open() {
                return Ok(ReplyOutcome::NotOpen(request.state));
            }
            if let Some(child) = &request.child_task_id {
                let child = tasks::require(tx, child)?;
                if !child.state.is_terminal() {
                    return Err(LedgerError::InvalidInput(format!(
                        "task {} has not finished; it cannot answer yet",
                        child.id
                    )));
                }
            }
            let message = insert_reply(tx, out, &request, &reply, actor)?;
            set_state(
                tx,
                &request.id,
                &[MessageState::Accepted, MessageState::Dispatched],
                MessageState::Answered,
            )?;
            Ok(ReplyOutcome::Recorded(message))
        });
        if let Err(e) = &result {
            if e.is_caller_error() {
                if let Ok(Some(request)) = self.read(|c| get(c, &reply.in_reply_to)) {
                    let _ = self.append_event(event(
                        &request.task_id,
                        actor,
                        "liaison.reply_refused",
                        json!({
                            "messageId": reply.message_id,
                            "inReplyTo": reply.in_reply_to,
                            "correlationId": reply.correlation_id,
                            "reason": e.to_string(),
                        }),
                    ));
                }
            }
        }
        result
    }

    /// Resume a waiting task with its pending replies: they become `delivered`
    /// (`liaison.replies_delivered`) and the task moves from `blocked` to `running`.
    pub fn resume_with_replies(
        &self,
        task_id: &str,
        reply_ids: &[String],
        reason: &str,
        actor: &str,
    ) -> Result<Task> {
        let result = self.write(|tx, out| {
            let mut correlation = None;
            for id in reply_ids {
                let reply = require(tx, id)?;
                if reply.kind != MessageKind::Reply || reply.task_id != task_id {
                    return Err(LedgerError::InvalidInput(format!(
                        "{id} is not a reply to task {task_id}"
                    )));
                }
                set_state(tx, id, &[MessageState::Pending], MessageState::Delivered)?;
                correlation.get_or_insert(reply.correlation_id);
            }
            if !reply_ids.is_empty() {
                out.push(events::insert(
                    tx,
                    event(
                        task_id,
                        actor,
                        "liaison.replies_delivered",
                        json!({ "messageIds": reply_ids, "correlationId": correlation }),
                    ),
                )?);
            }
            tasks::transition(tx, out, task_id, TaskState::Running, actor, Some(reason))
        });
        self.record_rejection(task_id, actor, Some(reason), &result);
        result
    }

    /// Discard the pending replies to `task_id` (it no longer waits for them).
    pub fn discard_replies(&self, task_id: &str, reason: &str, actor: &str) -> Result<Vec<String>> {
        self.write(|tx, out| {
            let pending = all(
                tx,
                &format!(
                    "SELECT {MESSAGE_COLUMNS} FROM liaison_messages
                     WHERE kind = 'reply' AND state = 'pending' AND task_id = ?1
                     ORDER BY created_at, rowid"
                ),
                [task_id],
            )?;
            let ids: Vec<String> = pending.iter().map(|m| m.id.clone()).collect();
            for id in &ids {
                set_state(tx, id, &[MessageState::Pending], MessageState::Discarded)?;
            }
            if let Some(first) = pending.first() {
                out.push(events::insert(
                    tx,
                    event(
                        task_id,
                        actor,
                        "liaison.reply_discarded",
                        json!({
                            "messageIds": ids,
                            "correlationId": first.correlation_id,
                            "reason": reason,
                        }),
                    ),
                )?);
            }
            Ok(ids)
        })
    }

    /// Cancel an open request (`liaison.handoff_cancelled` on both tasks). A child that has not
    /// started is cancelled in the same transaction; a running or waiting child must be stopped
    /// by its runtime.
    pub fn cancel_request(
        &self,
        request_id: &str,
        reason: &str,
        actor: &str,
    ) -> Result<CancelOutcome> {
        self.write(|tx, out| {
            let request = require(tx, request_id)?;
            if request.kind != MessageKind::Request || !request.state.is_open() {
                return Ok(CancelOutcome::NotOpen);
            }
            set_state(
                tx,
                request_id,
                &[MessageState::Accepted, MessageState::Dispatched],
                MessageState::Cancelled,
            )?;
            let detail = json!({
                "messageId": request.id,
                "correlationId": request.correlation_id,
                "childTaskId": request.child_task_id,
                "reason": reason,
            });
            out.push(events::insert(
                tx,
                event(
                    &request.task_id,
                    actor,
                    "liaison.handoff_cancelled",
                    detail.clone(),
                ),
            )?);
            let Some(child) = &request.child_task_id else {
                return Ok(CancelOutcome::Cancelled { child: None });
            };
            out.push(events::insert(
                tx,
                event(child, actor, "liaison.handoff_cancelled", detail),
            )?);
            let task = tasks::require(tx, child)?;
            let task = if task.state == TaskState::Queued {
                tasks::transition(tx, out, child, TaskState::Cancelled, actor, Some(reason))?
            } else {
                task
            };
            Ok(CancelOutcome::Cancelled {
                child: Some(Box::new(task)),
            })
        })
    }

    // ---- Queries ------------------------------------------------------------------------

    pub fn liaison_message(&self, id: &str) -> Result<Option<LiaisonMessage>> {
        self.read(|c| get(c, id))
    }

    /// Messages of `task_id` as requester (its requests and the replies to it), oldest first.
    pub fn liaison_messages_for_task(&self, task_id: &str) -> Result<Vec<LiaisonMessage>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {MESSAGE_COLUMNS} FROM liaison_messages WHERE task_id = ?1
                     ORDER BY created_at, rowid"
                ),
                [task_id],
            )
        })
    }

    /// The request that created `child_task_id`, if it was created by a handoff.
    pub fn liaison_request_for_child(&self, child_task_id: &str) -> Result<Option<LiaisonMessage>> {
        self.read(|c| request_for_child(c, child_task_id))
    }

    /// The reply to `request_id`, if any.
    pub fn liaison_reply_to(&self, request_id: &str) -> Result<Option<LiaisonMessage>> {
        self.read(|c| reply_to(c, request_id))
    }

    /// Every message of one workflow, oldest first.
    pub fn liaison_messages_for_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<LiaisonMessage>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {MESSAGE_COLUMNS} FROM liaison_messages WHERE correlation_id = ?1
                     ORDER BY created_at, rowid"
                ),
                [correlation_id],
            )
        })
    }

    /// Requests not yet refused in a workflow (accepted, dispatched, answered, or cancelled).
    pub fn liaison_handoff_count(&self, correlation_id: &str) -> Result<u32> {
        self.read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM liaison_messages
                 WHERE correlation_id = ?1 AND kind = 'request' AND state != 'rejected'",
                [correlation_id],
                |r| r.get(0),
            )?)
        })
    }

    /// Open requests (accepted or dispatched) with the states of their tasks, oldest first.
    pub fn liaison_open_requests(&self) -> Result<Vec<OpenRequest>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {}, p.state, {}
                 FROM liaison_messages m JOIN tasks p ON p.id = m.task_id
                 LEFT JOIN tasks t ON t.id = m.child_task_id
                 WHERE m.kind = 'request' AND m.state IN ('accepted', 'dispatched')
                 ORDER BY m.created_at, m.rowid",
                rows::prefixed(MESSAGE_COLUMNS, "m"),
                rows::prefixed(TASK_COLUMNS, "t"),
            ))?;
            let rows = stmt
                .query_map([], |r| {
                    let message = message_row(r)?;
                    let parent_state = parse_enum(13, r.get(13)?, TaskState::parse)?;
                    let child = match r.get::<_, Option<String>>(14)? {
                        Some(_) => Some(rows::task_at(r, 14)?),
                        None => None,
                    };
                    Ok(OpenRequest {
                        message,
                        parent_state,
                        child,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Cancelled requests whose child task is still unfinished (it must be stopped).
    pub fn liaison_cancelled_live_children(&self) -> Result<Vec<Task>> {
        self.read(|c| {
            let columns = rows::prefixed(TASK_COLUMNS, "t");
            let mut stmt = c.prepare(&format!(
                "SELECT {columns} FROM liaison_messages m JOIN tasks t ON t.id = m.child_task_id
                 WHERE m.kind = 'request' AND m.state = 'cancelled'
                   AND t.state NOT IN ('succeeded', 'failed', 'cancelled')
                 ORDER BY m.created_at, m.rowid"
            ))?;
            let rows = stmt
                .query_map([], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Waiting (`blocked`) tasks with pending replies and no open request: ready to resume.
    pub fn liaison_ready_deliveries(&self) -> Result<Vec<String>> {
        self.read(|c| {
            let mut stmt = c.prepare(
                "SELECT r.task_id, MIN(r.created_at) AS first FROM liaison_messages r
                 JOIN tasks t ON t.id = r.task_id
                 WHERE r.kind = 'reply' AND r.state = 'pending' AND t.state = 'blocked'
                   AND NOT EXISTS (SELECT 1 FROM liaison_messages q
                                   WHERE q.kind = 'request' AND q.task_id = r.task_id
                                     AND q.state IN ('accepted', 'dispatched'))
                 GROUP BY r.task_id ORDER BY first",
            )?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Pending replies to `task_id`, oldest first.
    pub fn liaison_pending_replies(&self, task_id: &str) -> Result<Vec<LiaisonMessage>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {MESSAGE_COLUMNS} FROM liaison_messages
                     WHERE kind = 'reply' AND state = 'pending' AND task_id = ?1
                     ORDER BY created_at, rowid"
                ),
                [task_id],
            )
        })
    }

    /// Tasks that finished while replies to them were still pending.
    pub fn liaison_stale_replies(&self) -> Result<Vec<String>> {
        self.read(|c| {
            let mut stmt = c.prepare(
                "SELECT DISTINCT r.task_id FROM liaison_messages r JOIN tasks t ON t.id = r.task_id
                 WHERE r.kind = 'reply' AND r.state = 'pending'
                   AND t.state IN ('succeeded', 'failed', 'cancelled')",
            )?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::NewTask;
    use crate::test_support::*;

    const CORRELATION: &str = "c0ffee00-0000-4000-8000-000000000001";

    fn running(l: &Ledger, objective: &str) -> Task {
        let t = task(l, objective);
        l.transition_task(&t.id, TaskState::Running, "agent:codex", None)
            .unwrap()
    }

    fn step_result() -> NewEvent {
        NewEvent {
            source: "agent:codex".into(),
            event_type: "agent.result".into(),
            payload: json!({ "outcome": "completed", "step": 1 }),
            ..NewEvent::default()
        }
    }

    fn accept(parent: &str, id: &str, key: &str) -> NewHandoffRequest {
        NewHandoffRequest {
            message_id: id.into(),
            correlation_id: CORRELATION.into(),
            dedupe_key: key.into(),
            source: "session:s-parent".into(),
            destination: "runtime:claude-code".into(),
            envelope: json!({ "objective": "Review it" }),
            summary: json!({ "objective": "Review it" }),
            decision: HandoffDecision::Accept {
                child: NewTask {
                    parent_task_id: Some(parent.into()),
                    requested_by: "agent:codex".into(),
                    assigned_to: Some("claude-code".into()),
                    objective: "Review it".into(),
                    priority: 2,
                    metadata: json!({ "sessionId": format!("s-{id}") }),
                    ..NewTask::default()
                },
                received: json!({ "depth": 1 }),
            },
        }
    }

    fn reject(id: &str, key: &str) -> NewHandoffRequest {
        NewHandoffRequest {
            message_id: id.into(),
            correlation_id: CORRELATION.into(),
            dedupe_key: key.into(),
            source: "session:s-parent".into(),
            destination: "runtime:gemini".into(),
            envelope: json!({ "objective": "Ask gemini" }),
            summary: json!({ "objective": "Ask gemini" }),
            decision: HandoffDecision::Reject {
                reason: "unknown destination".into(),
                reply: NewReply {
                    message_id: format!("{id}-reply"),
                    correlation_id: CORRELATION.into(),
                    in_reply_to: id.into(),
                    child_task_id: None,
                    source: "liaison".into(),
                    envelope: json!({ "outcome": "rejected" }),
                    summary: json!({ "outcome": "rejected" }),
                },
            },
        }
    }

    fn reply(request: &LiaisonMessage, id: &str) -> NewReply {
        NewReply {
            message_id: id.into(),
            correlation_id: request.correlation_id.clone(),
            in_reply_to: request.id.clone(),
            child_task_id: request.child_task_id.clone(),
            source: "session:s-child".into(),
            envelope: json!({ "outcome": "completed", "text": "Looks right." }),
            summary: json!({ "outcome": "completed" }),
        }
    }

    fn types(l: &Ledger, task_id: &str) -> Vec<String> {
        l.events_for_task(task_id)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect()
    }

    #[test]
    fn suspension_records_requests_children_and_the_wait_together() {
        let l = ledger();
        let parent = running(&l, "write a parser");
        let s = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![accept(&parent.id, "m-1", "k-1"), reject("m-2", "k-2")],
                "waiting for 1 handoff reply",
                "liaison",
            )
            .unwrap();
        assert!(!s.replayed);
        assert_eq!(s.task.state, TaskState::Blocked);
        assert_eq!(s.children.len(), 1);
        let child = &s.children[0];
        assert_eq!(child.state, TaskState::Queued);
        assert_eq!(child.parent_task_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(s.requests[0].state, MessageState::Accepted);
        assert_eq!(
            s.requests[0].child_task_id.as_deref(),
            Some(child.id.as_str())
        );
        assert_eq!(s.requests[1].state, MessageState::Rejected);
        // The refusal is already answered, pending delivery.
        let refusal = l.liaison_reply_to("m-2").unwrap().unwrap();
        assert_eq!(refusal.state, MessageState::Pending);
        assert_eq!(refusal.destination, "session:s-parent");
        assert_eq!(refusal.correlation_id, CORRELATION);

        assert_eq!(
            types(&l, &parent.id),
            [
                "task.created",
                "task.state_changed",
                "agent.result",
                "liaison.handoff_requested",
                "task.child_created",
                "liaison.handoff_requested",
                "liaison.handoff_rejected",
                "liaison.reply_received",
                "task.state_changed",
            ]
        );
        assert_eq!(
            types(&l, &child.id),
            ["task.created", "liaison.handoff_received"]
        );
        let received = l.events_for_task(&child.id).unwrap().pop().unwrap();
        assert_eq!(received.payload["correlationId"], CORRELATION);
        assert_eq!(received.payload["depth"], 1);
        assert_eq!(received.payload["parentTaskId"], json!(parent.id));

        // The whole workflow is found by its correlation ID.
        assert_eq!(
            l.liaison_messages_for_correlation(CORRELATION)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(l.liaison_handoff_count(CORRELATION).unwrap(), 1);
    }

    #[test]
    fn replayed_suspensions_change_nothing() {
        let l = ledger();
        let parent = running(&l, "p");
        let batch = || vec![accept(&parent.id, "m-1", "k-1")];
        l.suspend_for_handoffs(&parent.id, step_result(), batch(), "waiting", "liaison")
            .unwrap();
        let before = l.events_for_task(&parent.id).unwrap().len();
        // Same dedupe key, even with a new message ID: nothing new.
        let mut again = batch();
        again[0].message_id = "m-9".into();
        let s = l
            .suspend_for_handoffs(&parent.id, step_result(), again, "waiting", "liaison")
            .unwrap();
        assert!(s.replayed);
        assert_eq!(s.requests[0].id, "m-1");
        assert_eq!(l.events_for_task(&parent.id).unwrap().len(), before);
        assert_eq!(l.child_tasks(&parent.id).unwrap().len(), 1);
        assert!(l.liaison_message("m-9").unwrap().is_none());
    }

    #[test]
    fn suspension_needs_a_running_task_and_rolls_back_as_a_whole() {
        let l = ledger();
        let queued = task(&l, "never started");
        let err = l
            .suspend_for_handoffs(
                &queued.id,
                step_result(),
                vec![accept(&queued.id, "m-1", "k-1")],
                "waiting",
                "liaison",
            )
            .unwrap_err();
        assert!(matches!(err, LedgerError::InvalidTransition { .. }));
        assert!(l.child_tasks(&queued.id).unwrap().is_empty());
        assert!(l.liaison_message("m-1").unwrap().is_none());
        assert_eq!(
            types(&l, &queued.id).last().unwrap(),
            "task.transition_rejected"
        );

        // A bad request (child under another parent) rolls everything back.
        let parent = running(&l, "p");
        let other = task(&l, "other");
        let err = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![
                    accept(&parent.id, "m-2", "k-2"),
                    accept(&other.id, "m-3", "k-3"),
                ],
                "waiting",
                "liaison",
            )
            .unwrap_err();
        assert!(matches!(err, LedgerError::InvalidInput(_)));
        assert_eq!(
            l.task(&parent.id).unwrap().unwrap().state,
            TaskState::Running
        );
        assert!(l.liaison_message("m-2").unwrap().is_none());
        assert!(l.child_tasks(&parent.id).unwrap().is_empty());
    }

    #[test]
    fn dispatch_answer_and_delivery() {
        let l = ledger();
        let parent = running(&l, "p");
        let s = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![accept(&parent.id, "m-1", "k-1")],
                "waiting",
                "liaison",
            )
            .unwrap();
        let child = s.children[0].clone();
        // Only the assigned session may start it.
        assert!(l
            .begin_handoff_turn(&child.id, "s-other", "agent:claude-code")
            .is_err());
        let started = l
            .begin_handoff_turn(&child.id, "s-m-1", "agent:claude-code")
            .unwrap();
        assert_eq!(started.state, TaskState::Running);
        let request = l.liaison_message("m-1").unwrap().unwrap();
        assert_eq!(request.state, MessageState::Dispatched);
        // Not twice.
        assert!(l
            .begin_handoff_turn(&child.id, "s-m-1", "agent:claude-code")
            .is_err());

        // Nothing to deliver while the request is open, and no reply before the child ends.
        assert!(l.liaison_ready_deliveries().unwrap().is_empty());
        assert!(l.answer_request(reply(&request, "r-1"), "liaison").is_err());
        let open = l.liaison_open_requests().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].parent_state, TaskState::Blocked);
        assert_eq!(open[0].child.as_ref().unwrap().state, TaskState::Running);

        l.transition_task(&child.id, TaskState::Succeeded, "agent:claude-code", None)
            .unwrap();
        let outcome = l.answer_request(reply(&request, "r-1"), "liaison").unwrap();
        let ReplyOutcome::Recorded(answer) = outcome else {
            panic!("expected a new reply: {outcome:?}");
        };
        assert_eq!(answer.state, MessageState::Pending);
        assert_eq!(answer.task_id, parent.id);
        assert_eq!(answer.destination, "session:s-parent");
        assert_eq!(
            l.liaison_message("m-1").unwrap().unwrap().state,
            MessageState::Answered
        );
        // Answering again returns the same reply; nothing is recorded twice.
        assert_eq!(
            l.answer_request(reply(&request, "r-2"), "liaison").unwrap(),
            ReplyOutcome::AlreadyAnswered(answer.clone())
        );
        assert!(l.liaison_message("r-2").unwrap().is_none());

        assert_eq!(l.liaison_ready_deliveries().unwrap(), [parent.id.as_str()]);
        let resumed = l
            .resume_with_replies(&parent.id, &["r-1".into()], "1 reply delivered", "liaison")
            .unwrap();
        assert_eq!(resumed.state, TaskState::Running);
        assert_eq!(
            l.liaison_message("r-1").unwrap().unwrap().state,
            MessageState::Delivered
        );
        assert!(l.liaison_ready_deliveries().unwrap().is_empty());
        // A delivered reply cannot be delivered again.
        l.transition_task(&parent.id, TaskState::Blocked, "liaison", None)
            .unwrap();
        assert!(l
            .resume_with_replies(&parent.id, &["r-1".into()], "again", "liaison")
            .is_err());

        assert_eq!(
            types(&l, &child.id),
            [
                "task.created",
                "liaison.handoff_received",
                "liaison.dispatched",
                "task.state_changed",
                "task.state_changed",
                "liaison.reply_sent",
            ]
        );
        let parent_types = types(&l, &parent.id);
        assert!(parent_types
            .windows(2)
            .any(|w| w == ["liaison.replies_delivered", "task.state_changed"]));
        assert!(parent_types.contains(&"liaison.reply_received".to_owned()));
    }

    #[test]
    fn forged_replies_are_refused_and_recorded() {
        let l = ledger();
        let parent = running(&l, "p");
        let s = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![accept(&parent.id, "m-1", "k-1")],
                "waiting",
                "liaison",
            )
            .unwrap();
        let child = &s.children[0];
        l.begin_handoff_turn(&child.id, "s-m-1", "agent:claude-code")
            .unwrap();
        l.transition_task(&child.id, TaskState::Failed, "agent:claude-code", None)
            .unwrap();
        let request = l.liaison_message("m-1").unwrap().unwrap();
        let intruder = task(&l, "unrelated");

        let mut wrong_correlation = reply(&request, "r-x");
        wrong_correlation.correlation_id = "another-workflow".into();
        let mut wrong_child = reply(&request, "r-y");
        wrong_child.child_task_id = Some(intruder.id.clone());
        for forged in [wrong_correlation, wrong_child] {
            let err = l.answer_request(forged, "liaison").unwrap_err();
            assert!(matches!(err, LedgerError::InvalidInput(_)), "{err}");
        }
        assert!(l.liaison_reply_to("m-1").unwrap().is_none());
        assert_eq!(
            l.liaison_message("m-1").unwrap().unwrap().state,
            MessageState::Dispatched
        );
        let refused = l
            .events_for_task(&parent.id)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == "liaison.reply_refused")
            .count();
        assert_eq!(refused, 2);
        // A reply cannot answer a reply.
        let ReplyOutcome::Recorded(real) =
            l.answer_request(reply(&request, "r-1"), "liaison").unwrap()
        else {
            panic!("expected a reply");
        };
        let mut chained = reply(&request, "r-2");
        chained.in_reply_to = real.id;
        assert!(l.answer_request(chained, "liaison").is_err());
    }

    #[test]
    fn cancellation_stops_queued_children_and_leaves_running_ones_to_their_runtime() {
        let l = ledger();
        let parent = running(&l, "p");
        let s = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![
                    accept(&parent.id, "m-1", "k-1"),
                    accept(&parent.id, "m-2", "k-2"),
                ],
                "waiting",
                "liaison",
            )
            .unwrap();
        let (queued, started) = (&s.children[0], &s.children[1]);
        l.begin_handoff_turn(&started.id, "s-m-2", "agent:claude-code")
            .unwrap();
        l.transition_task(&parent.id, TaskState::Cancelled, "owner", None)
            .unwrap();
        let open = l.liaison_open_requests().unwrap();
        assert!(open.iter().all(|o| o.parent_state == TaskState::Cancelled));

        let child = |outcome: CancelOutcome| match outcome {
            CancelOutcome::Cancelled { child: Some(child) } => *child,
            other => panic!("expected a cancelled child: {other:?}"),
        };
        let first = child(
            l.cancel_request("m-1", "parent cancelled", "liaison")
                .unwrap(),
        );
        assert_eq!(
            first.state,
            TaskState::Cancelled,
            "a child that never started is cancelled right away"
        );
        let second = child(
            l.cancel_request("m-2", "parent cancelled", "liaison")
                .unwrap(),
        );
        assert_eq!(second.state, TaskState::Running);
        let live: Vec<_> = l
            .liaison_cancelled_live_children()
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(live, [started.id.as_str()], "its runtime must stop it");
        // Cancelling twice does nothing; a cancelled request takes no reply.
        assert_eq!(
            l.cancel_request("m-1", "again", "liaison").unwrap(),
            CancelOutcome::NotOpen
        );
        l.transition_task(&started.id, TaskState::Cancelled, "agent:claude-code", None)
            .unwrap();
        let request = l.liaison_message("m-2").unwrap().unwrap();
        assert_eq!(
            l.answer_request(reply(&request, "r-2"), "liaison").unwrap(),
            ReplyOutcome::NotOpen(MessageState::Cancelled)
        );
        assert!(l.liaison_cancelled_live_children().unwrap().is_empty());
        assert!(types(&l, &queued.id).contains(&"liaison.handoff_cancelled".to_owned()));
    }

    #[test]
    fn failed_dispatch_fails_the_child_and_stale_replies_are_discarded() {
        let l = ledger();
        let parent = running(&l, "p");
        let s = l
            .suspend_for_handoffs(
                &parent.id,
                step_result(),
                vec![accept(&parent.id, "m-1", "k-1"), reject("m-2", "k-2")],
                "waiting",
                "liaison",
            )
            .unwrap();
        let failed = l
            .fail_handoff_dispatch(
                "m-1",
                "Claude Code is not signed in.",
                json!({ "outcome": "authRequired" }),
                "liaison",
            )
            .unwrap();
        assert_eq!(failed.state, TaskState::Failed);
        assert_eq!(failed.id, s.children[0].id);
        // Only an accepted request can fail to dispatch.
        assert!(l
            .fail_handoff_dispatch("m-1", "again", Value::Null, "liaison")
            .is_err());
        let event = l.events_for_task(&failed.id).unwrap();
        let dispatch_failed = event
            .iter()
            .find(|e| e.event_type == "liaison.dispatch_failed")
            .unwrap();
        assert_eq!(dispatch_failed.payload["outcome"], "authRequired");

        // The requester stops waiting (cancelled by the owner): its pending replies are stale.
        l.transition_task(&parent.id, TaskState::Cancelled, "owner", None)
            .unwrap();
        assert_eq!(l.liaison_stale_replies().unwrap(), [parent.id.as_str()]);
        let discarded = l
            .discard_replies(&parent.id, "the task is no longer waiting", "liaison")
            .unwrap();
        assert_eq!(discarded, ["m-2-reply"]);
        assert!(l.liaison_stale_replies().unwrap().is_empty());
        assert!(l
            .discard_replies(&parent.id, "again", "liaison")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn messages_are_immutable_and_never_deleted() {
        let l = ledger();
        let parent = running(&l, "p");
        l.suspend_for_handoffs(
            &parent.id,
            step_result(),
            vec![accept(&parent.id, "m-1", "k-1")],
            "waiting",
            "liaison",
        )
        .unwrap();
        let c = l.conn();
        for sql in [
            "UPDATE liaison_messages SET envelope = '{}' WHERE id = 'm-1'",
            "UPDATE liaison_messages SET correlation_id = 'x' WHERE id = 'm-1'",
            "UPDATE liaison_messages SET task_id = 'x' WHERE id = 'm-1'",
            "DELETE FROM liaison_messages",
        ] {
            let err = c.execute(sql, []).unwrap_err().to_string();
            assert!(err.contains("liaison messages"), "{sql}: {err}");
        }
        // States are constrained per kind.
        assert!(c
            .execute(
                "UPDATE liaison_messages SET state = 'delivered' WHERE id = 'm-1'",
                []
            )
            .is_err());
    }

    #[test]
    fn task_roots() {
        let l = ledger();
        let root = task(&l, "root");
        let child = l
            .create_task(
                NewTask {
                    parent_task_id: Some(root.id.clone()),
                    requested_by: "x".into(),
                    objective: "child".into(),
                    ..NewTask::default()
                },
                "x",
            )
            .unwrap();
        let grandchild = l
            .create_task(
                NewTask {
                    parent_task_id: Some(child.id.clone()),
                    requested_by: "x".into(),
                    objective: "grandchild".into(),
                    ..NewTask::default()
                },
                "x",
            )
            .unwrap();
        for id in [&root.id, &child.id, &grandchild.id] {
            assert_eq!(l.task_root(id).unwrap().id, root.id);
        }
        assert!(matches!(
            l.task_root("missing"),
            Err(LedgerError::NotFound(_))
        ));
    }
}
