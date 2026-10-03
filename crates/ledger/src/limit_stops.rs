//! Objectives a plan's usage limit stopped (Phase 25, item 4.2; ADR-253). An objective the owner
//! gave whose last step ended at an AI tool's usage limit failed; Plenipo picks it back up once
//! the limit is over, unless the owner left it stopped. Each one is picked up, left stopped, or
//! run again by the owner at most once: the record says which.

use rusqlite::Connection;

use crate::dto::Task;
use crate::error::Result;
use crate::rows::u64_of;
use crate::Ledger;

/// Plenipo gave an objective a usage limit stopped to its worker again (payload: `runAgainAs`,
/// the new task, and `runtimeId`).
pub const PICKED_UP: &str = "work.picked_up";
/// The owner left an objective a usage limit stopped as it is.
pub const LEFT_STOPPED: &str = "work.left_stopped";
/// Plenipo tried to pick an objective back up and could not (payload: `reason`).
pub const NOT_PICKED_UP: &str = "work.not_picked_up";
/// The owner's **Run again** (Phase 13), recorded by the desktop app under this name.
const RUN_AGAIN: &str = "plenipo.run_again";
/// At most this many are looked at once.
const MAX_STOPS: u32 = 50;

/// An objective a usage limit stopped, still waiting.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitStop {
    pub task: Task,
    /// The AI tool that reached its limit.
    pub runtime: String,
    /// When the limit was reported (ms).
    pub at: u64,
}

impl Ledger {
    /// The owner's objectives that a usage limit stopped since `since` (ms) and that are still
    /// waiting: failed, their last step ended at a usage limit, not picked up, left stopped, or
    /// run again, and not given again by the owner since. Oldest first.
    pub fn limit_stops(&self, since: u64) -> Result<Vec<LimitStop>> {
        self.read(|c| stops(c, since))
    }
}

fn stops(c: &Connection, since: u64) -> Result<Vec<LimitStop>> {
    let mut stmt = c.prepare(
        "SELECT t.id, x.runtime, e.created_at
         FROM tasks t
         JOIN executions x ON x.task_id = t.id
         JOIN events e ON e.execution_id = x.id AND e.event_type = 'agent.result'
         WHERE t.parent_task_id IS NULL AND t.state = 'failed' AND t.updated_at >= ?1
           AND e.seq = (SELECT MAX(r.seq) FROM events r
                        JOIN executions rx ON r.execution_id = rx.id
                        WHERE rx.task_id = t.id AND r.event_type = 'agent.result')
           AND json_extract(e.payload, '$.outcome') = 'usageLimited'
           AND NOT EXISTS (SELECT 1 FROM events d WHERE d.task_id = t.id
                           AND d.event_type IN (?2, ?3, ?4, ?5))
           AND NOT EXISTS (SELECT 1 FROM tasks n WHERE n.parent_task_id IS NULL
                           AND n.objective = t.objective AND n.created_at > t.created_at)
         ORDER BY t.created_at, t.rowid
         LIMIT ?6",
    )?;
    let found = stmt
        .query_map(
            rusqlite::params![
                i64::try_from(since).unwrap_or(i64::MAX),
                PICKED_UP,
                LEFT_STOPPED,
                NOT_PICKED_UP,
                RUN_AGAIN,
                MAX_STOPS,
            ],
            |r| Ok((r.get::<_, String>(0)?, r.get(1)?, u64_of(r.get(2)?))),
        )?
        .collect::<rusqlite::Result<Vec<(String, String, u64)>>>()?;
    let mut out = Vec::with_capacity(found.len());
    for (id, runtime, at) in found {
        if let Some(task) = crate::tasks::get(c, &id)? {
            out.push(LimitStop { task, runtime, at });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dto::{ExecutionRow, NewEvent, NewTask, TaskState};

    fn stopped(l: &Ledger, objective: &str, outcome: &str) -> Task {
        let task = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: objective.into(),
                    metadata: json!({ "sessionId": "s-1", "workforce": { "positionId": "p-1" } }),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        l.transition_task(&task.id, TaskState::Running, "x", None)
            .unwrap();
        let x = format!("x-{}", task.id);
        l.upsert_execution(
            &ExecutionRow {
                id: x.clone(),
                task_id: Some(task.id.clone()),
                runtime: "codex".into(),
                provider: None,
                model: None,
                session_id: None,
                process_id: None,
                profile_id: None,
                label: "turn".into(),
                executable: None,
                args: vec![],
                working_dir: None,
                state: "failed".into(),
                exit_code: None,
                detail: None,
                started_at: task.created_at,
                ended_at: None,
                usage_metadata: json!({}),
            },
            "test",
        )
        .unwrap();
        l.append_event(NewEvent {
            execution_id: Some(x),
            source: "agent".into(),
            event_type: "agent.result".into(),
            payload: json!({ "outcome": outcome, "error": "usage limit" }),
            ..NewEvent::default()
        })
        .unwrap();
        l.transition_task(&task.id, TaskState::Failed, "x", None)
            .unwrap();
        l.task(&task.id).unwrap().unwrap()
    }

    fn ids(l: &Ledger) -> Vec<String> {
        l.limit_stops(0)
            .unwrap()
            .into_iter()
            .map(|s| s.task.objective)
            .collect()
    }

    #[test]
    fn objectives_a_limit_stopped_wait_until_picked_up_left_or_given_again() {
        let l = Ledger::open_in_memory().unwrap();
        let a = stopped(&l, "Build the page", "usageLimited");
        stopped(&l, "Crashed", "failed");
        let c = stopped(&l, "Write the docs", "usageLimited");
        let d = stopped(&l, "Fix the tests", "usageLimited");
        let stops = l.limit_stops(0).unwrap();
        assert_eq!(stops[0].runtime, "codex");
        assert_eq!(
            ids(&l),
            ["Build the page", "Write the docs", "Fix the tests"]
        );
        // Picked up, left stopped, and given again by the owner: none waits any more.
        for (task, kind) in [(&a, PICKED_UP), (&c, LEFT_STOPPED)] {
            l.append_event(NewEvent {
                task_id: Some(task.id.clone()),
                source: "plenipo".into(),
                event_type: kind.into(),
                ..NewEvent::default()
            })
            .unwrap();
        }
        assert_eq!(ids(&l), ["Fix the tests"]);
        l.create_task(
            NewTask {
                requested_by: "owner".into(),
                objective: d.objective.clone(),
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap();
        assert!(ids(&l).is_empty());
        // Too long ago: not looked at.
        let later = stopped(&l, "Plan the week", "usageLimited");
        assert!(l.limit_stops(later.updated_at + 1).unwrap().is_empty());
    }
}
