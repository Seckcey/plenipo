//! What the Phase 12 pages show that no earlier query gave: Home, and the pages of a department,
//! project, worker (position), and task.
//!
//! - A scope's events, newest first, a page at a time (a department's, project's, or position's
//!   history), and a task's events with those of the tasks under it.
//! - What is stuck, for Home: the newest problem of each piece of work still in trouble.
//! - The organization's objectives, newest first.
//! - A piece of work's pull requests, artifacts, and decisions (a project, or a task and the
//!   tasks under it).
//!
//! All read the existing tables (events by task and by time, tasks by position and parent), so
//! no migration is needed.

use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::activity::{reach, Org, Reach};
use crate::dto::*;
use crate::error::Result;
use crate::rows::{self, EVENT_COLUMNS, TASK_COLUMNS};
use crate::Ledger;

/// The most events one page of history holds.
pub const MAX_PAGE_EVENTS: u32 = 200;

/// Decisions: approvals answered or expired, refusals, handoffs refused, lessons kept or
/// discarded, the owner's stops and take-overs, and why each worker got its AI tool.
pub const DECISIONS: &[&str] = &[
    "approval.resolved",
    "approval.expired",
    "guard.denied",
    "liaison.handoff_rejected",
    "lesson.kept",
    "lesson.discarded",
    "control.taken_over",
    "control.stopped",
    "ssh.command_stop_requested",
    "org.worker_spawned",
];

/// Problems the owner may need to act on (What's stuck on Home).
const PROBLEMS: &[&str] = &[
    "task.state_changed",
    "guard.denied",
    "liaison.waiting_for_member",
    "liaison.dispatch_failed",
    "liaison.delivery_failed",
    "approval.expired",
    "ssh.host_key_changed",
];

/// The tasks of a project's work: every task under an objective that touched the project.
const PROJECT_TREE: &str = "WITH RECURSIVE up(id, parent, depth) AS (
         SELECT id, parent_task_id, 0 FROM tasks WHERE project_id = ?1
         UNION
         SELECT t.id, t.parent_task_id, up.depth + 1
         FROM tasks t JOIN up ON t.id = up.parent WHERE up.depth < 64
     ),
     down(id, depth) AS (
         SELECT id, 0 FROM up WHERE parent IS NULL
         UNION
         SELECT t.id, down.depth + 1
         FROM tasks t JOIN down ON t.parent_task_id = down.id WHERE down.depth < 64
     )";

/// The tasks of a task's work: the task and every task under it.
const TASK_TREE: &str = "WITH RECURSIVE down(id, depth) AS (
         SELECT id, 0 FROM tasks WHERE id = ?1
         UNION
         SELECT t.id, down.depth + 1
         FROM tasks t JOIN down ON t.parent_task_id = down.id WHERE down.depth < 64
     )";

/// Whose work a record covers.
#[derive(Debug, Clone, Copy)]
pub enum WorkOf<'a> {
    Project(&'a str),
    Task(&'a str),
}

impl WorkOf<'_> {
    fn tree(&self) -> (&'static str, &str) {
        match self {
            Self::Project(id) => (PROJECT_TREE, id),
            Self::Task(id) => (TASK_TREE, id),
        }
    }
}

fn before_seq(before: Option<u64>) -> i64 {
    before.map_or(i64::MAX, |b| i64::try_from(b).unwrap_or(i64::MAX))
}

fn json_list(items: &HashSet<String>) -> String {
    serde_json::to_string(&items.iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into())
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn events(c: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<LedgerEvent>> {
    let mut stmt = c.prepare(sql)?;
    let rows = stmt
        .query_map(params, rows::event)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

impl Ledger {
    /// A scope's events, newest first, before event `before` (the next page): the events of
    /// its team's tasks and its projects' tasks, and the organization's own events about its
    /// positions. The whole organization's are simply the newest events.
    pub fn scope_events(
        &self,
        scope: &ActivityScope,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<LedgerEvent>> {
        let limit = limit.clamp(1, MAX_PAGE_EVENTS);
        let before = before_seq(before);
        self.read(|c| {
            let org = Org::read(c)?;
            match reach(c, &org, scope)? {
                Reach::All => events(
                    c,
                    &format!(
                        "SELECT {EVENT_COLUMNS} FROM events WHERE seq < ?1
                         ORDER BY seq DESC LIMIT ?2"
                    ),
                    params![before, limit],
                ),
                Reach::Some {
                    positions,
                    projects,
                } => events(
                    c,
                    &format!(
                        "WITH scoped(id) AS (
                             SELECT id FROM tasks
                             WHERE json_extract(metadata, '$.workforce.positionId')
                                   IN (SELECT value FROM json_each(?1))
                                OR project_id IN (SELECT value FROM json_each(?2))
                         )
                         SELECT {} FROM events e
                         WHERE e.seq < ?3
                           AND (e.task_id IN (SELECT id FROM scoped)
                                OR (e.task_id IS NULL
                                    AND json_extract(e.payload, '$.positionId')
                                        IN (SELECT value FROM json_each(?1))))
                         ORDER BY e.seq DESC LIMIT ?4",
                        rows::prefixed(EVENT_COLUMNS, "e")
                    ),
                    params![json_list(&positions), json_list(&projects), before, limit],
                ),
            }
        })
    }

    /// A task's events with those of every task under it, newest first, before event
    /// `before` (the next page).
    pub fn tree_events(
        &self,
        task_id: &str,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<LedgerEvent>> {
        let limit = limit.clamp(1, MAX_PAGE_EVENTS);
        self.read(|c| {
            events(
                c,
                &format!(
                    "{TASK_TREE}
                     SELECT {EVENT_COLUMNS} FROM events
                     WHERE task_id IN (SELECT id FROM down) AND seq < ?2
                     ORDER BY seq DESC LIMIT ?3"
                ),
                params![task_id, before_seq(before), limit],
            )
        })
    }

    /// The organization's objectives (tasks with no parent that a position was given), newest
    /// first.
    pub fn org_objectives(&self, limit: u32) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE parent_task_id IS NULL
                   AND json_extract(metadata, '$.workforce.positionId') IS NOT NULL
                 ORDER BY created_at DESC, rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit.clamp(1, 1000)], rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// What is stuck (Home): the newest problem of each piece of work still in trouble since
    /// `since`, newest first. A failed task counts until its objective succeeds; a refusal, an
    /// expired approval, or a failed handoff while its task still runs; a handoff waiting for
    /// a position nobody fills while it waits; a server whose ID changed until it is pinned
    /// again.
    pub fn problems(&self, since: u64, limit: u32) -> Result<Vec<LedgerEvent>> {
        let limit = usize::try_from(limit.clamp(1, 200)).unwrap_or(200);
        let since = i64::try_from(since).unwrap_or(i64::MAX);
        self.read(|c| {
            let marks = vec!["?"; PROBLEMS.len()].join(", ");
            let mut args: Vec<rusqlite::types::Value> = PROBLEMS
                .iter()
                .map(|t| rusqlite::types::Value::Text((*t).to_owned()))
                .collect();
            args.push(rusqlite::types::Value::Integer(since));
            let candidates = events(
                c,
                &format!(
                    "SELECT {EVENT_COLUMNS} FROM events
                     WHERE event_type IN ({marks}) AND created_at >= ?{}
                     ORDER BY seq DESC LIMIT 2000",
                    PROBLEMS.len() + 1
                ),
                rusqlite::params_from_iter(args),
            )?;
            let state = |id: &str| -> Result<Option<TaskState>> {
                Ok(
                    c.query_row("SELECT state FROM tasks WHERE id = ?1", [id], |r| {
                        r.get::<_, String>(0)
                    })
                    .ok()
                    .and_then(|s| TaskState::parse(&s)),
                )
            };
            let root_state = |id: &str| -> Result<Option<TaskState>> {
                Ok(c.query_row(
                    "WITH RECURSIVE up(id, parent, depth) AS (
                         SELECT id, parent_task_id, 0 FROM tasks WHERE id = ?1
                         UNION ALL
                         SELECT t.id, t.parent_task_id, up.depth + 1
                         FROM tasks t JOIN up ON t.id = up.parent WHERE up.depth < 64
                     )
                     SELECT t.state FROM tasks t
                     WHERE t.id = (SELECT id FROM up WHERE parent IS NULL)",
                    [id],
                    |r| r.get::<_, String>(0),
                )
                .ok()
                .and_then(|s| TaskState::parse(&s)))
            };
            let repinned = |server: &str, after: u64| -> Result<bool> {
                Ok(c.query_row(
                    "SELECT 1 FROM events WHERE event_type = 'guard.server_changed'
                       AND json_extract(payload, '$.serverId') = ?1
                       AND json_extract(payload, '$.pinned') = 1 AND created_at >= ?2 LIMIT 1",
                    params![server, i64::try_from(after).unwrap_or(i64::MAX)],
                    |_| Ok(()),
                )
                .is_ok())
            };
            let mut seen: HashSet<String> = HashSet::new();
            let mut out = Vec::new();
            for e in candidates {
                if out.len() >= limit {
                    break;
                }
                let p = &e.payload;
                // One problem per piece of work (a task, or a server).
                let key = match (&e.task_id, e.event_type.as_str()) {
                    (_, "ssh.host_key_changed") => {
                        format!("server:{}", text(&p["serverId"]).unwrap_or_default())
                    }
                    (Some(t), _) => format!("task:{t}"),
                    (None, _) => continue,
                };
                if seen.contains(&key) {
                    continue;
                }
                let still = match e.event_type.as_str() {
                    "task.state_changed" => {
                        p["to"] == "failed"
                            && e.task_id.as_deref().is_some_and(|t| {
                                root_state(t)
                                    .ok()
                                    .flatten()
                                    .is_none_or(|s| s != TaskState::Succeeded)
                            })
                    }
                    "ssh.host_key_changed" => text(&p["serverId"])
                        .is_some_and(|s| !repinned(&s, e.created_at).unwrap_or(false)),
                    _ => e
                        .task_id
                        .as_deref()
                        .is_some_and(|t| state(t).ok().flatten().is_some_and(|s| !s.is_terminal())),
                };
                if !still {
                    continue;
                }
                seen.insert(key);
                out.push(e);
            }
            Ok(out)
        })
    }

    /// A piece of work's pull requests, artifacts, and decisions, newest first (at most
    /// `limit` of each).
    pub fn work_record(&self, of: WorkOf<'_>, limit: u32) -> Result<WorkRecord> {
        let limit = limit.clamp(1, MAX_PAGE_EVENTS);
        let (tree, id) = of.tree();
        self.read(|c| {
            let prs = events(
                c,
                &format!(
                    "{tree}
                     SELECT {EVENT_COLUMNS} FROM events
                     WHERE task_id IN (SELECT id FROM down) AND event_type = 'capability.used'
                       AND json_extract(payload, '$.pullRequest.url') IS NOT NULL
                     ORDER BY seq DESC LIMIT ?2"
                ),
                params![id, limit],
            )?
            .into_iter()
            .filter_map(|e| {
                let pr = &e.payload["pullRequest"];
                Some(PullRequestRef {
                    url: text(&pr["url"])?,
                    number: pr["number"].as_u64(),
                    task_id: e.task_id.clone()?,
                    worker: text(&e.payload["worker"]),
                    created_at: e.created_at,
                })
            })
            .collect();
            let mut stmt = c.prepare(&format!(
                "{tree}
                 SELECT id, task_id, artifact_type, metadata, created_at FROM artifacts
                 WHERE task_id IN (SELECT id FROM down)
                 ORDER BY created_at DESC, rowid DESC LIMIT ?2"
            ))?;
            let artifacts = stmt
                .query_map(params![id, limit], |r| {
                    let metadata = rows::json(r.get::<_, String>(3)?);
                    Ok(ArtifactView {
                        id: r.get(0)?,
                        task_id: r.get(1)?,
                        kind: r.get(2)?,
                        label: text(&metadata["title"])
                            .or_else(|| text(&metadata["url"]))
                            .or_else(|| text(&metadata["caption"])),
                        created_at: rows::u64_of(r.get(4)?),
                    })
                })?
                .collect::<rusqlite::Result<_>>()?;
            let marks = (0..DECISIONS.len())
                .map(|i| format!("?{}", i + 3))
                .collect::<Vec<_>>()
                .join(", ");
            let mut args: Vec<rusqlite::types::Value> = vec![
                rusqlite::types::Value::Text(id.to_owned()),
                rusqlite::types::Value::Integer(i64::from(limit)),
            ];
            args.extend(
                DECISIONS
                    .iter()
                    .map(|t| rusqlite::types::Value::Text((*t).to_owned())),
            );
            let decisions = events(
                c,
                &format!(
                    "{tree}
                     SELECT {EVENT_COLUMNS} FROM events
                     WHERE task_id IN (SELECT id FROM down) AND event_type IN ({marks})
                     ORDER BY seq DESC LIMIT ?2"
                ),
                rusqlite::params_from_iter(args),
            )?;
            Ok(WorkRecord {
                pull_requests: prs,
                artifacts,
                decisions,
            })
        })
    }

    /// The IDs of a task and every task under it (for a task's approvals).
    pub fn tree_task_ids(&self, task_id: &str) -> Result<Vec<String>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!("{TASK_TREE} SELECT id FROM down"))?;
            let rows = stmt
                .query_map([task_id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::*;

    fn child(l: &Ledger, parent: &Task, objective: &str, metadata: Value) -> Task {
        l.create_task(
            NewTask {
                parent_task_id: Some(parent.id.clone()),
                requested_by: "worker".into(),
                objective: objective.into(),
                priority: 2,
                metadata,
                ..NewTask::default()
            },
            "worker",
        )
        .unwrap()
    }

    fn event(l: &Ledger, task: Option<&str>, event_type: &str, payload: Value) -> LedgerEvent {
        l.append_event(NewEvent {
            task_id: task.map(str::to_owned),
            source: "guard".into(),
            event_type: event_type.into(),
            payload,
            ..NewEvent::default()
        })
        .unwrap()
    }

    fn fail(l: &Ledger, t: &Task) {
        l.transition_task(&t.id, TaskState::Running, "w", None)
            .unwrap();
        l.transition_task(&t.id, TaskState::Failed, "w", None)
            .unwrap();
    }

    #[test]
    fn a_scopes_history_is_its_teams_and_projects_events_and_its_own_org_events() {
        use crate::activity::tests::{org, task_for};
        let l = ledger();
        org(&l);
        task_for(&l, "t-web", Some("web"), Some("W"));
        task_for(&l, "t-dev", None, Some("S"));
        task_for(&l, "t-res", None, Some("R"));
        let web = event(
            &l,
            Some("t-web"),
            "agent.message",
            json!({ "text": "website" }),
        );
        let dev = event(
            &l,
            Some("t-dev"),
            "agent.message",
            json!({ "text": "developer" }),
        );
        event(
            &l,
            Some("t-res"),
            "agent.message",
            json!({ "text": "research" }),
        );
        let hired = event(
            &l,
            None,
            "org.position_updated",
            json!({ "positionId": "S" }),
        );
        event(
            &l,
            None,
            "org.position_updated",
            json!({ "positionId": "R" }),
        );
        let seqs = |scope: ActivityScope| -> Vec<u64> {
            l.scope_events(&scope, None, 50)
                .unwrap()
                .into_iter()
                .map(|e| e.seq)
                .collect()
        };
        // Development: its project's work and everyone under its head; not Research.
        assert_eq!(
            seqs(ActivityScope::Department("dev".into())),
            [hired.seq, dev.seq, web.seq]
        );
        // The worker: its own tasks and the organization's events about it.
        assert_eq!(
            seqs(ActivityScope::Position("S".into())),
            [hired.seq, dev.seq]
        );
        assert_eq!(
            seqs(ActivityScope::Project("web".into())),
            [hired.seq, dev.seq, web.seq]
        );
        // Everything, a page at a time.
        let all = l.scope_events(&ActivityScope::All, None, 2).unwrap();
        assert_eq!(all.len(), 2);
        let rest = l
            .scope_events(&ActivityScope::All, Some(all[1].seq), 50)
            .unwrap();
        assert!(rest.iter().all(|e| e.seq < all[1].seq));
        assert!(l
            .scope_events(&ActivityScope::Department("nope".into()), None, 5)
            .is_err());
    }

    #[test]
    fn a_task_tree_has_its_events_newest_first_a_page_at_a_time() {
        let l = ledger();
        let root = task(&l, "Ship the site");
        let kid = child(&l, &root, "Build it", json!({}));
        let other = task(&l, "Something else");
        for i in 0..5 {
            event(
                &l,
                Some(&kid.id),
                "agent.message",
                json!({ "text": format!("k{i}") }),
            );
        }
        event(
            &l,
            Some(&other.id),
            "agent.message",
            json!({ "text": "elsewhere" }),
        );
        let page = l.tree_events(&root.id, None, 3).unwrap();
        assert_eq!(page.len(), 3);
        assert!(page.windows(2).all(|w| w[0].seq > w[1].seq), "newest first");
        assert!(page
            .iter()
            .all(|e| e.task_id.as_deref() != Some(other.id.as_str())));
        let next = l
            .tree_events(&root.id, Some(page.last().unwrap().seq), 50)
            .unwrap();
        assert!(next.iter().all(|e| e.seq < page.last().unwrap().seq));
        let ids = l.tree_task_ids(&root.id).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn what_is_stuck_is_one_problem_per_piece_of_work_still_in_trouble() {
        let l = ledger();
        // A failed objective: stuck.
        let failed = task(&l, "Order stock");
        fail(&l, &failed);
        // A failed step whose objective then succeeded: not stuck.
        let done = task(&l, "Write the post");
        let step = child(&l, &done, "Draft", json!({}));
        fail(&l, &step);
        l.transition_task(&done.id, TaskState::Running, "w", None)
            .unwrap();
        l.transition_task(&done.id, TaskState::Succeeded, "w", None)
            .unwrap();
        // A refusal while its task still runs: stuck (once, however many).
        let running = task(&l, "Clean the server");
        l.transition_task(&running.id, TaskState::Running, "w", None)
            .unwrap();
        event(
            &l,
            Some(&running.id),
            "guard.denied",
            json!({ "summary": "run rm" }),
        );
        event(
            &l,
            Some(&running.id),
            "guard.denied",
            json!({ "summary": "run rm -rf" }),
        );
        // A changed server ID: stuck until pinned again.
        event(
            &l,
            None,
            "ssh.host_key_changed",
            json!({ "serverId": "s1", "server": "Shop" }),
        );
        event(
            &l,
            None,
            "ssh.host_key_changed",
            json!({ "serverId": "s2", "server": "Dev" }),
        );
        event(
            &l,
            None,
            "guard.server_changed",
            json!({ "serverId": "s2", "pinned": true }),
        );
        let stuck = l.problems(0, 50).unwrap();
        let kinds: Vec<(&str, Option<&str>)> = stuck
            .iter()
            .map(|e| (e.event_type.as_str(), e.task_id.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("ssh.host_key_changed", None),
                ("guard.denied", Some(running.id.as_str())),
                ("task.state_changed", Some(failed.id.as_str())),
            ]
        );
        assert_eq!(stuck[1].payload["summary"], "run rm -rf", "the newest");
        assert_eq!(stuck[0].payload["serverId"], "s1");
    }

    #[test]
    fn a_projects_record_has_its_pull_requests_artifacts_and_decisions() {
        let l = ledger();
        let root = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "Fix the checkout".into(),
                    priority: 2,
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        let step = l
            .create_task(
                NewTask {
                    parent_task_id: Some(root.id.clone()),
                    requested_by: "worker".into(),
                    project_id: None,
                    objective: "Open the pull request".into(),
                    priority: 2,
                    ..NewTask::default()
                },
                "worker",
            )
            .unwrap();
        // The project is named on the step (as Workforce does); the record follows the tree.
        let project = {
            let d = l.create_department("Web", "", None, "owner").unwrap();
            l.create_project("Shop", None, None, Some(&d.id), "owner")
                .unwrap()
                .id
        };
        let tagged = l
            .create_task(
                NewTask {
                    parent_task_id: Some(step.id.clone()),
                    requested_by: "worker".into(),
                    project_id: Some(project.clone()),
                    objective: "Review".into(),
                    priority: 2,
                    ..NewTask::default()
                },
                "worker",
            )
            .unwrap();
        event(
            &l,
            Some(&step.id),
            "capability.used",
            json!({ "worker": "Senior Developer", "pullRequest": { "url": "https://github.com/o/r/pull/7", "number": 7 } }),
        );
        event(
            &l,
            Some(&step.id),
            "capability.used",
            json!({ "worker": "x" }),
        );
        l.record_artifact(
            Some(&tagged.id),
            "screenshot",
            Some("/tmp/s.png"),
            None,
            None,
            &json!({ "title": "Checkout page" }),
            "guard",
        )
        .unwrap();
        event(
            &l,
            Some(&tagged.id),
            "approval.resolved",
            json!({ "state": "approved" }),
        );
        event(
            &l,
            Some(&tagged.id),
            "agent.message",
            json!({ "text": "not a decision" }),
        );
        let record = l.work_record(WorkOf::Project(&project), 50).unwrap();
        assert_eq!(record.pull_requests.len(), 1);
        assert_eq!(record.pull_requests[0].number, Some(7));
        assert_eq!(
            record.pull_requests[0].worker.as_deref(),
            Some("Senior Developer")
        );
        assert_eq!(record.artifacts.len(), 1);
        assert_eq!(record.artifacts[0].label.as_deref(), Some("Checkout page"));
        assert_eq!(record.decisions.len(), 1);
        assert_eq!(record.decisions[0].event_type, "approval.resolved");
        // The same, from the task's page (the task and the tasks under it).
        let from_step = l.work_record(WorkOf::Task(&step.id), 50).unwrap();
        assert_eq!(from_step, record);
        let from_leaf = l.work_record(WorkOf::Task(&tagged.id), 50).unwrap();
        assert!(from_leaf.pull_requests.is_empty());
    }

    /// The plan's "large task history" test: a long history still opens a page at a time,
    /// quickly, for a task tree and for a department, among many other events.
    #[test]
    fn a_large_history_opens_a_page_at_a_time_quickly() {
        use crate::activity::tests::{org, task_for};
        use std::time::{Duration, Instant};
        let l = ledger();
        org(&l);
        task_for(&l, "big", Some("web"), Some("W"));
        {
            let conn = crate::lock(&l.conn);
            let tx = conn.unchecked_transaction().unwrap();
            let mut child = tx
                .prepare(
                    "INSERT INTO tasks (id, parent_task_id, requested_by, project_id, objective,
                                        state, metadata, created_at, updated_at)
                     VALUES (?1, ?2, 'owner', ?3, 'test', 'running', ?4, 0, 0)",
                )
                .unwrap();
            let mut add = tx
                .prepare(
                    "INSERT INTO events (id, task_id, source, event_type, payload, created_at)
                     VALUES (?1, ?2, 'test', 'agent.message', '{}', ?3)",
                )
                .unwrap();
            // The objective and 20 tasks under it: 3,000 events.
            for k in 0..20 {
                child
                    .execute(params![
                        format!("big-{k}"),
                        "big",
                        "web",
                        json!({ "workforce": { "positionId": "S" } }).to_string()
                    ])
                    .unwrap();
            }
            let mut at: i64 = 0;
            for i in 0..3_000 {
                at += 1;
                let task = if i % 3 == 0 {
                    "big".to_owned()
                } else {
                    format!("big-{}", i % 20)
                };
                add.execute(params![uuid::Uuid::new_v4().to_string(), task, at])
                    .unwrap();
            }
            // 30,000 events of Research's 200 tasks, most of them newer.
            for t in 0..200 {
                child
                    .execute(params![
                        format!("res-{t}"),
                        Option::<String>::None,
                        Option::<String>::None,
                        json!({ "workforce": { "positionId": "R" } }).to_string()
                    ])
                    .unwrap();
                for _ in 0..150 {
                    at += 1;
                    add.execute(params![
                        uuid::Uuid::new_v4().to_string(),
                        format!("res-{t}"),
                        at
                    ])
                    .unwrap();
                }
            }
            drop(child);
            drop(add);
            tx.commit().unwrap();
        }
        let timed = |what: &str, f: &mut dyn FnMut() -> usize| {
            let started = Instant::now();
            let n = f();
            let took = started.elapsed();
            assert!(
                took < Duration::from_millis(1_500),
                "{what} took {took:?} for {n} events"
            );
            n
        };
        // The objective's page: its newest 50, then page by page to the oldest.
        let first = l.tree_events("big", None, 50).unwrap();
        assert_eq!(first.len(), 50);
        let mut before = first.last().map(|e| e.seq);
        let mut seen = first.len();
        timed("the whole task tree, a page at a time", &mut || {
            while let Some(b) = before {
                let page = l.tree_events("big", Some(b), MAX_PAGE_EVENTS).unwrap();
                seen += page.len();
                before = page.last().map(|e| e.seq);
            }
            seen
        });
        assert_eq!(seen, 3_000, "every event, each once");
        // The department's newest page, found among 30,000 newer events of another department.
        let dev = timed("the department's first page", &mut || {
            l.scope_events(&ActivityScope::Department("dev".into()), None, 50)
                .unwrap()
                .len()
        });
        assert_eq!(dev, 50);
        let res = timed("the other department's first page", &mut || {
            l.scope_events(&ActivityScope::Department("res".into()), None, 50)
                .unwrap()
                .len()
        });
        assert_eq!(res, 50);
        timed("what is stuck", &mut || l.problems(0, 30).unwrap().len());
    }
}
