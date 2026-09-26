//! Tasks: creation, state machine, parent/child relations, timelines.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use crate::dto::{LedgerEvent, NewEvent, NewTask, Task, TaskState, TaskTimeline};
use crate::error::{LedgerError, Result};
use crate::events;
use crate::rows::{self, TASK_COLUMNS};
use crate::Ledger;

pub(crate) fn get(conn: &Connection, id: &str) -> Result<Option<Task>> {
    Ok(conn
        .query_row(
            &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
            [id],
            rows::task,
        )
        .optional()?)
}

pub(crate) fn require(conn: &Connection, id: &str) -> Result<Task> {
    get(conn, id)?.ok_or_else(|| LedgerError::NotFound(format!("task {id}")))
}

/// Create a task in `queued` inside an open transaction, recording `task.created` (and
/// `task.child_created` on its parent).
pub(crate) fn insert(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    new: NewTask,
    actor: &str,
) -> Result<Task> {
    let objective = new.objective.trim().to_owned();
    if objective.is_empty() || objective.len() > 10_000 {
        return Err(LedgerError::InvalidInput(
            "objective must be 1–10,000 characters".into(),
        ));
    }
    if new.requested_by.trim().is_empty() {
        return Err(LedgerError::InvalidInput("requested_by is required".into()));
    }
    if new.priority > 4 {
        return Err(LedgerError::InvalidInput("priority must be 0–4".into()));
    }
    let metadata = rows::metadata_text(&new.metadata)?;
    if let Some(parent_id) = &new.parent_task_id {
        let parent = require(tx, parent_id)?;
        if parent.state.is_terminal() {
            return Err(LedgerError::InvalidInput(format!(
                "parent task {parent_id} is already {}",
                parent.state.as_str()
            )));
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::now_ms() as i64;
    tx.execute(
        "INSERT INTO tasks (id, parent_task_id, requested_by, assigned_to, project_id,
             objective, acceptance_criteria, priority, state, metadata, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?10, ?10)",
        params![
            id,
            new.parent_task_id,
            new.requested_by,
            new.assigned_to,
            new.project_id,
            objective,
            new.acceptance_criteria,
            new.priority,
            metadata,
            now
        ],
    )?;
    out.push(events::insert(
        tx,
        NewEvent {
            task_id: Some(id.clone()),
            source: actor.into(),
            event_type: "task.created".into(),
            payload: json!({
                "objective": objective,
                "parentTaskId": new.parent_task_id,
                "requestedBy": new.requested_by,
                "assignedTo": new.assigned_to,
                "priority": new.priority,
            }),
            ..NewEvent::default()
        },
    )?);
    if let Some(parent_id) = &new.parent_task_id {
        out.push(events::insert(
            tx,
            NewEvent {
                task_id: Some(parent_id.clone()),
                source: actor.into(),
                event_type: "task.child_created".into(),
                payload: json!({ "childTaskId": id, "objective": objective }),
                ..NewEvent::default()
            },
        )?);
    }
    require(tx, &id)
}

/// Move a task to `to` inside an open transaction and record `task.state_changed`. An
/// illegal transition returns [`LedgerError::InvalidTransition`] and writes nothing.
pub(crate) fn transition(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    id: &str,
    to: TaskState,
    actor: &str,
    reason: Option<&str>,
) -> Result<Task> {
    let task = require(tx, id)?;
    if !task.state.can_transition_to(to) {
        return Err(LedgerError::InvalidTransition {
            entity: "task",
            id: id.to_owned(),
            from: task.state.as_str().into(),
            to: to.as_str().into(),
        });
    }
    let now = crate::now_ms() as i64;
    tx.execute(
        "UPDATE tasks SET state = ?2, updated_at = ?3,
             started_at = CASE WHEN ?2 = 'running' AND started_at IS NULL THEN ?3 ELSE started_at END,
             completed_at = CASE WHEN ?4 THEN ?3 ELSE completed_at END
         WHERE id = ?1",
        params![id, to.as_str(), now, to.is_terminal()],
    )?;
    out.push(events::insert(
        tx,
        NewEvent {
            task_id: Some(id.into()),
            source: actor.into(),
            event_type: "task.state_changed".into(),
            payload: json!({ "from": task.state, "to": to, "reason": reason }),
            ..NewEvent::default()
        },
    )?);
    require(tx, id)
}

impl Ledger {
    /// Create a task in `queued` and record `task.created`.
    pub fn create_task(&self, new: NewTask, actor: &str) -> Result<Task> {
        self.write(|tx, out| insert(tx, out, new, actor))
    }

    pub fn task(&self, id: &str) -> Result<Option<Task>> {
        self.read(|c| get(c, id))
    }

    /// Newest first.
    pub fn list_tasks(&self, limit: u32) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks ORDER BY created_at DESC, rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 1000)], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Direct children, oldest first.
    pub fn child_tasks(&self, parent_id: &str) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks WHERE parent_task_id = ?1 ORDER BY created_at, rowid"
            ))?;
            let rows = stmt
                .query_map([parent_id], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// The whole subtree under `root_id` (excluding the root) with depth (children = 1).
    pub fn descendant_tasks(&self, root_id: &str) -> Result<Vec<(Task, u32)>> {
        self.read(|c| {
            let prefixed = rows::prefixed(TASK_COLUMNS, "t");
            let mut stmt = c.prepare(&format!(
                "WITH RECURSIVE tree(id, depth) AS (
                     SELECT id, 1 FROM tasks WHERE parent_task_id = ?1
                     UNION ALL
                     SELECT t.id, tree.depth + 1 FROM tasks t JOIN tree ON t.parent_task_id = tree.id
                 )
                 SELECT {prefixed}, tree.depth FROM tasks t JOIN tree ON t.id = tree.id
                 ORDER BY tree.depth, t.created_at, t.rowid"
            ))?;
            let rows = stmt
                .query_map([root_id], |r| Ok((rows::task(r)?, r.get::<_, u32>(14)?)))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// The top of `id`'s parent chain (the task itself when it has no parent).
    pub fn task_root(&self, id: &str) -> Result<Task> {
        self.read(|c| {
            let root: Option<String> = c
                .query_row(
                    "WITH RECURSIVE up(id, parent, depth) AS (
                         SELECT id, parent_task_id, 0 FROM tasks WHERE id = ?1
                         UNION ALL
                         SELECT t.id, t.parent_task_id, up.depth + 1
                         FROM tasks t JOIN up ON t.id = up.parent WHERE up.depth < 10000
                     )
                     SELECT id FROM up ORDER BY depth DESC LIMIT 1",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            let root = root.ok_or_else(|| LedgerError::NotFound(format!("task {id}")))?;
            require(c, &root)
        })
    }

    /// Move a task to `to`. Illegal transitions are rejected **and** recorded as
    /// `task.transition_rejected` so the attempt is visible in the trail.
    pub fn transition_task(
        &self,
        id: &str,
        to: TaskState,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<Task> {
        self.transition_with(id, to, actor, reason, None)
    }

    /// Record `event` on the task and move it to `to` in **one** transaction (e.g. an agent
    /// turn's result and its final state). An illegal transition writes neither and is
    /// recorded as `task.transition_rejected`.
    pub fn complete_task(
        &self,
        id: &str,
        to: TaskState,
        actor: &str,
        reason: Option<&str>,
        event: NewEvent,
    ) -> Result<Task> {
        self.transition_with(id, to, actor, reason, Some(event))
    }

    fn transition_with(
        &self,
        id: &str,
        to: TaskState,
        actor: &str,
        reason: Option<&str>,
        before: Option<NewEvent>,
    ) -> Result<Task> {
        let result = self.write(|tx, out| {
            let task = require(tx, id)?;
            if !task.state.can_transition_to(to) {
                return Err(LedgerError::InvalidTransition {
                    entity: "task",
                    id: id.to_owned(),
                    from: task.state.as_str().into(),
                    to: to.as_str().into(),
                });
            }
            if let Some(event) = before {
                out.push(events::insert(
                    tx,
                    NewEvent {
                        task_id: Some(id.into()),
                        ..event
                    },
                )?);
            }
            transition(tx, out, id, to, actor, reason)
        });
        self.record_rejection(id, actor, reason, &result);
        result
    }

    /// Best effort: a rejected transition is itself part of the audit trail.
    pub(crate) fn record_rejection<T>(
        &self,
        id: &str,
        actor: &str,
        reason: Option<&str>,
        result: &Result<T>,
    ) {
        if let Err(LedgerError::InvalidTransition {
            entity: "task",
            id: task_id,
            from,
            to,
        }) = result
        {
            if task_id == id {
                let _ = self.append_event(NewEvent {
                    task_id: Some(id.into()),
                    source: actor.into(),
                    event_type: "task.transition_rejected".into(),
                    payload: json!({ "from": from, "to": to, "reason": reason }),
                    ..NewEvent::default()
                });
            }
        }
    }

    /// Assign (or unassign) a task and record `task.assigned`.
    pub fn assign_task(&self, id: &str, assignee: Option<&str>, actor: &str) -> Result<Task> {
        self.write(|tx, out| {
            let task = require(tx, id)?;
            if task.state.is_terminal() {
                return Err(LedgerError::InvalidInput(format!(
                    "task {id} is already {}",
                    task.state.as_str()
                )));
            }
            tx.execute(
                "UPDATE tasks SET assigned_to = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, assignee, crate::now_ms() as i64],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    task_id: Some(id.into()),
                    source: actor.into(),
                    event_type: "task.assigned".into(),
                    payload: json!({ "from": task.assigned_to, "to": assignee }),
                    ..NewEvent::default()
                },
            )?);
            require(tx, id)
        })
    }

    /// A task with its complete ordered trail and direct children.
    pub fn task_timeline(&self, id: &str) -> Result<TaskTimeline> {
        let task = self
            .task(id)?
            .ok_or_else(|| LedgerError::NotFound(format!("task {id}")))?;
        Ok(TaskTimeline {
            events: self.events_for_task(id)?,
            children: self.child_tasks(id)?,
            task,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use TaskState::*;

    #[test]
    fn create_read_and_list() {
        let l = ledger();
        let a = task(&l, "first");
        let b = task(&l, "second");
        assert_eq!(a.state, Queued);
        assert_eq!(a.started_at, None);
        assert_eq!(l.task(&a.id).unwrap().unwrap(), a);
        let listed: Vec<_> = l
            .list_tasks(10)
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(listed, [b.id, a.id]);
        assert!(l.task("missing").unwrap().is_none());
    }

    #[test]
    fn creation_validates_input() {
        let l = ledger();
        let base = NewTask {
            requested_by: "owner".into(),
            objective: "x".into(),
            ..NewTask::default()
        };
        for bad in [
            NewTask {
                objective: "   ".into(),
                ..base.clone()
            },
            NewTask {
                objective: "x".repeat(10_001),
                ..base.clone()
            },
            NewTask {
                requested_by: "".into(),
                ..base.clone()
            },
            NewTask {
                priority: 9,
                ..base.clone()
            },
            NewTask {
                metadata: serde_json::json!([1]),
                ..base.clone()
            },
            NewTask {
                parent_task_id: Some("missing".into()),
                ..base.clone()
            },
        ] {
            assert!(l.create_task(bad, "owner").is_err());
        }
        assert_eq!(
            l.list_tasks(10).unwrap().len(),
            0,
            "failed creates leave nothing behind"
        );
    }

    #[test]
    fn full_lifecycle_records_every_step_in_order() {
        let l = ledger();
        let t = task(&l, "lifecycle");
        let running = l
            .transition_task(&t.id, Running, "worker", Some("go"))
            .unwrap();
        assert!(running.started_at.is_some());
        l.transition_task(&t.id, AwaitingApproval, "worker", None)
            .unwrap();
        l.transition_task(&t.id, Running, "owner", Some("approved"))
            .unwrap();
        l.transition_task(&t.id, Blocked, "worker", Some("waiting on input"))
            .unwrap();
        l.transition_task(&t.id, Running, "worker", None).unwrap();
        let done = l.transition_task(&t.id, Succeeded, "worker", None).unwrap();
        assert!(done.completed_at.is_some());
        assert_eq!(
            done.started_at, running.started_at,
            "started_at is set once"
        );

        let trail = l.events_for_task(&t.id).unwrap();
        let steps: Vec<(String, Option<&str>)> = trail
            .iter()
            .map(|e| (e.event_type.clone(), e.payload["to"].as_str()))
            .collect();
        assert_eq!(
            steps,
            [
                ("task.created".into(), None),
                ("task.state_changed".into(), Some("running")),
                ("task.state_changed".into(), Some("awaitingApproval")),
                ("task.state_changed".into(), Some("running")),
                ("task.state_changed".into(), Some("blocked")),
                ("task.state_changed".into(), Some("running")),
                ("task.state_changed".into(), Some("succeeded")),
            ]
        );
        assert!(trail.windows(2).all(|w| w[0].seq < w[1].seq));
        assert_eq!(trail[1].payload["reason"], "go");
    }

    #[test]
    fn invalid_transitions_are_rejected_and_recorded() {
        let l = ledger();
        let t = task(&l, "strict");
        let err = l
            .transition_task(&t.id, Succeeded, "worker", None)
            .unwrap_err();
        assert!(matches!(err, LedgerError::InvalidTransition { .. }));
        assert_eq!(
            l.task(&t.id).unwrap().unwrap().state,
            Queued,
            "state unchanged"
        );

        l.transition_task(&t.id, Cancelled, "owner", None).unwrap();
        for to in TaskState::ALL {
            assert!(
                l.transition_task(&t.id, to, "worker", None).is_err(),
                "cancelled -> {to:?}"
            );
        }
        let rejected = l
            .events_for_task(&t.id)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == "task.transition_rejected")
            .count();
        assert_eq!(rejected, 1 + TaskState::ALL.len());
        assert!(matches!(
            l.transition_task("missing", Running, "x", None),
            Err(LedgerError::NotFound(_))
        ));
    }

    #[test]
    fn parent_child_relations() {
        let l = ledger();
        let root = task(&l, "root");
        let child = |parent: &str, name: &str| {
            l.create_task(
                NewTask {
                    parent_task_id: Some(parent.into()),
                    requested_by: "coordinator".into(),
                    objective: name.into(),
                    ..NewTask::default()
                },
                "coordinator",
            )
            .unwrap()
        };
        let a = child(&root.id, "a");
        let b = child(&root.id, "b");
        let a1 = child(&a.id, "a1");

        let kids: Vec<_> = l
            .child_tasks(&root.id)
            .unwrap()
            .into_iter()
            .map(|t| t.objective)
            .collect();
        assert_eq!(kids, ["a", "b"]);
        let tree: Vec<_> = l
            .descendant_tasks(&root.id)
            .unwrap()
            .into_iter()
            .map(|(t, d)| (t.objective, d))
            .collect();
        assert_eq!(tree, [("a".into(), 1), ("b".into(), 1), ("a1".into(), 2)]);
        assert_eq!(a1.parent_task_id.as_deref(), Some(a.id.as_str()));

        // The parent's trail records each child.
        let parent_trail = l.task_timeline(&root.id).unwrap();
        assert_eq!(parent_trail.children.len(), 2);
        assert_eq!(
            parent_trail
                .events
                .iter()
                .filter(|e| e.event_type == "task.child_created")
                .count(),
            2
        );

        // No children under finished tasks; tasks with children cannot be deleted.
        l.transition_task(&b.id, Cancelled, "owner", None).unwrap();
        assert!(l
            .create_task(
                NewTask {
                    parent_task_id: Some(b.id.clone()),
                    requested_by: "x".into(),
                    objective: "late".into(),
                    ..NewTask::default()
                },
                "x"
            )
            .is_err());
        let c = l.conn();
        assert!(c
            .execute("DELETE FROM tasks WHERE id = ?1", [&root.id])
            .is_err());
    }

    #[test]
    fn assignment_is_recorded() {
        let l = ledger();
        let t = task(&l, "assign");
        let t = l
            .assign_task(&t.id, Some("agent-1"), "coordinator")
            .unwrap();
        assert_eq!(t.assigned_to.as_deref(), Some("agent-1"));
        let last = l.events_for_task(&t.id).unwrap().pop().unwrap();
        assert_eq!(last.event_type, "task.assigned");
        assert_eq!(last.payload["to"], "agent-1");
    }

    #[test]
    fn state_check_constraint_backs_up_the_state_machine() {
        let l = ledger();
        let t = task(&l, "constraint");
        let c = l.conn();
        assert!(c
            .execute("UPDATE tasks SET state = 'exploded' WHERE id = ?1", [&t.id])
            .is_err());
    }

    #[test]
    fn complete_task_records_the_event_and_state_together() {
        let l = ledger();
        let t = task(&l, "turn");
        l.transition_task(&t.id, Running, "w", None).unwrap();
        let result = |text: &str| NewEvent {
            source: "agent:test".into(),
            event_type: "agent.result".into(),
            payload: json!({ "summary": text }),
            ..NewEvent::default()
        };
        let done = l
            .complete_task(&t.id, Succeeded, "w", Some("done"), result("ok"))
            .unwrap();
        assert_eq!(done.state, Succeeded);
        let trail: Vec<_> = l
            .events_for_task(&t.id)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert_eq!(
            &trail[trail.len() - 2..],
            ["agent.result", "task.state_changed"]
        );
        // An illegal transition writes neither the event nor the state.
        let before = l.events_for_task(&t.id).unwrap().len();
        assert!(l
            .complete_task(&t.id, Failed, "w", None, result("again"))
            .is_err());
        let after = l.events_for_task(&t.id).unwrap();
        assert_eq!(after.len(), before + 1);
        assert_eq!(after.last().unwrap().event_type, "task.transition_rejected");
        assert!(!after.iter().any(|e| e.payload["summary"] == "again"));
    }
}
