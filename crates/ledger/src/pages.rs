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

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::activity::{reach, Org, Reach};
use crate::dto::*;
use crate::error::Result;
use crate::rows::{self, EVENT_COLUMNS, TASK_COLUMNS};
use crate::Ledger;

/// The most events one page of history holds.
pub const MAX_PAGE_EVENTS: u32 = 200;

/// How many of the newest events a page of a scope's history reads in order before it looks
/// further back through the task index.
const HISTORY_STRETCH: i64 = 20_000;

/// Decisions: approvals answered or expired, refusals, handoffs refused, lessons kept or
/// discarded, the owner's stops and take-overs and Plenipo's own stops of a browser tab, and
/// why each worker got its AI tool.
pub const DECISIONS: &[&str] = &[
    "approval.resolved",
    "approval.expired",
    "guard.denied",
    "liaison.handoff_rejected",
    "lesson.kept",
    "lesson.discarded",
    "control.taken_over",
    "control.stopped",
    "browser.tab_stopped",
    "ssh.command_stop_requested",
    "org.worker_spawned",
];

/// Problems the owner may need to act on (What's stuck on Home). A task waiting for its
/// full-time worker to finish another task is ordinary queueing, not one of them.
const PROBLEMS: &[&str] = &[
    "task.state_changed",
    "guard.denied",
    "liaison.dispatch_failed",
    "liaison.delivery_failed",
    "approval.expired",
    "ssh.host_key_changed",
    // A lead needs a job nobody in its department does (Phase 25, item 2.7).
    "org.hire_needed",
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
                } => {
                    let (positions, projects) = (json_list(&positions), json_list(&projects));
                    let query = |range: &str, limit: u32, plus: &str| {
                        events(
                            c,
                            &format!(
                                "WITH scoped(id) AS (
                                     SELECT id FROM tasks
                                     WHERE json_extract(metadata, '$.workforce.positionId')
                                           IN (SELECT value FROM json_each(?1))
                                        OR project_id IN (SELECT value FROM json_each(?2))
                                 )
                                 SELECT {} FROM events e
                                 WHERE {range}
                                   AND ({plus}e.task_id IN (SELECT id FROM scoped)
                                        OR ({plus}e.task_id IS NULL
                                            AND json_extract(e.payload, '$.positionId')
                                                IN (SELECT value FROM json_each(?1))))
                                 ORDER BY e.seq DESC LIMIT ?4",
                                rows::prefixed(EVENT_COLUMNS, "e")
                            ),
                            params![positions, projects, before, limit],
                        )
                    };
                    // The newest stretch of the Ledger first, read in order (fast when the
                    // scope is busy); then, only if the page is not full, the scope's older
                    // events through the task index (fast when the scope is quiet).
                    let newest: i64 =
                        c.query_row("SELECT coalesce(max(seq), 0) FROM events", [], |r| r.get(0))?;
                    let upper = before.min(newest.saturating_add(1));
                    let floor = upper.saturating_sub(HISTORY_STRETCH).max(0);
                    let mut page = query(
                        &format!("e.seq < ?3 AND e.seq < {upper} AND e.seq >= {floor}"),
                        limit,
                        // `+` keeps SQLite reading the stretch in order, not through the index.
                        "+",
                    )?;
                    let got = u32::try_from(page.len()).unwrap_or(limit);
                    if got < limit && floor > 0 {
                        page.extend(query(&format!("e.seq < {floor}"), limit - got, "")?);
                    }
                    Ok(page)
                }
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

    /// The organization's objectives (tasks with no parent that a position was given) still
    /// going, newest first.
    pub fn open_objectives(&self, limit: u32) -> Result<Vec<Task>> {
        self.objectives(
            "state NOT IN ('succeeded', 'failed', 'cancelled')
             ORDER BY created_at DESC, rowid DESC LIMIT ?1",
            params![limit.clamp(1, 1000)],
        )
    }

    /// The organization's objectives that finished (succeeded, failed, or were cancelled)
    /// since `since`, the last to finish first.
    pub fn finished_objectives(&self, since: u64, limit: u32) -> Result<Vec<Task>> {
        self.objectives(
            "state IN ('succeeded', 'failed', 'cancelled')
               AND coalesce(completed_at, updated_at) >= ?1
             ORDER BY coalesce(completed_at, updated_at) DESC, rowid DESC LIMIT ?2",
            params![
                i64::try_from(since).unwrap_or(i64::MAX),
                limit.clamp(1, 1000)
            ],
        )
    }

    /// How many of the organization's objectives are going, and how many finished since
    /// `since` (all of them, not a page).
    pub fn objective_counts(&self, since: u64) -> Result<(u32, u32)> {
        self.read(|c| {
            let (going, finished) = c.query_row(
                "SELECT
                     coalesce(sum(state NOT IN ('succeeded', 'failed', 'cancelled')), 0),
                     coalesce(sum(state IN ('succeeded', 'failed', 'cancelled')
                                  AND coalesce(completed_at, updated_at) >= ?1), 0)
                 FROM tasks
                 WHERE parent_task_id IS NULL
                   AND json_extract(metadata, '$.workforce.positionId') IS NOT NULL",
                [i64::try_from(since).unwrap_or(i64::MAX)],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )?;
            Ok((
                u32::try_from(going).unwrap_or(u32::MAX),
                u32::try_from(finished).unwrap_or(u32::MAX),
            ))
        })
    }

    fn objectives(&self, rest: &str, args: impl rusqlite::Params) -> Result<Vec<Task>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE parent_task_id IS NULL
                   AND json_extract(metadata, '$.workforce.positionId') IS NOT NULL
                   AND {rest}"
            ))?;
            let rows = stmt
                .query_map(args, rows::task)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// What is stuck (Home): the newest problem of each piece of work still in trouble since
    /// `since`, newest first. Work is the company's: an objective given to a position and the
    /// tasks under it (not Diagnostics' test tasks). A failed task counts until its objective
    /// succeeds; a refusal, an expired approval, or a handoff that could not start or be
    /// delivered while its task still runs. A server whose ID changed counts until it is set
    /// right, as Settings → Servers decides: a new ID pinned, a later connection or test that
    /// succeeds, or the server removed.
    ///
    /// Reads only the events since `since` (found through the time index), and asks about each
    /// piece of work once.
    pub fn problems(&self, since: u64, limit: u32) -> Result<Vec<LedgerEvent>> {
        let limit = usize::try_from(limit.clamp(1, 200)).unwrap_or(200);
        let since = i64::try_from(since).unwrap_or(i64::MAX);
        self.read(|c| {
            // Events are numbered in the order they happen: the first one since `since`
            // bounds the scan.
            let Some(first) = c
                .query_row(
                    "SELECT seq FROM events WHERE created_at >= ?1
                     ORDER BY created_at, seq LIMIT 1",
                    [since],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
            else {
                return Ok(Vec::new());
            };
            let marks = (1..=PROBLEMS.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut args: Vec<rusqlite::types::Value> = PROBLEMS
                .iter()
                .map(|t| rusqlite::types::Value::Text((*t).to_owned()))
                .collect();
            args.push(rusqlite::types::Value::Integer(first));
            args.push(rusqlite::types::Value::Integer(since));
            let n = PROBLEMS.len();
            let mut candidates = c.prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM events
                 WHERE seq >= ?{} AND event_type IN ({marks}) AND created_at >= ?{}
                   AND (event_type <> 'task.state_changed'
                        OR json_extract(payload, '$.to') = 'failed')
                 ORDER BY seq DESC",
                n + 1,
                n + 2
            ))?;
            let mut rows = candidates.query(rusqlite::params_from_iter(args))?;
            // The objective a task belongs to: whether it is the company's work, and its state.
            let objective = |id: &str| -> Option<(bool, TaskState)> {
                c.query_row(
                    "WITH RECURSIVE up(id, parent, depth) AS (
                         SELECT id, parent_task_id, 0 FROM tasks WHERE id = ?1
                         UNION ALL
                         SELECT t.id, t.parent_task_id, up.depth + 1
                         FROM tasks t JOIN up ON t.id = up.parent WHERE up.depth < 64
                     )
                     SELECT json_extract(t.metadata, '$.workforce.positionId') IS NOT NULL
                            AND coalesce(json_extract(t.metadata, '$.synthetic'), 0) = 0,
                            t.state
                     FROM tasks t
                     WHERE t.id = (SELECT id FROM up WHERE parent IS NULL)",
                    [id],
                    |r| Ok((r.get::<_, bool>(0)?, r.get::<_, String>(1)?)),
                )
                .ok()
                .and_then(|(company, state)| Some((company, TaskState::parse(&state)?)))
            };
            let running = |id: &str| -> bool {
                c.query_row("SELECT state FROM tasks WHERE id = ?1", [id], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
                .and_then(|s| TaskState::parse(&s))
                .is_some_and(|s| !s.is_terminal())
            };
            let set_right = |server: &str, after: u64| -> bool {
                c.query_row(
                    "SELECT 1 FROM events
                     WHERE seq > ?2
                       AND event_type IN ('guard.server_changed', 'guard.server_removed',
                                          'ssh.connected', 'ssh.tested')
                       AND json_extract(payload, '$.serverId') = ?1
                       AND (event_type IN ('guard.server_removed', 'ssh.connected')
                            OR (event_type = 'guard.server_changed'
                                AND json_extract(payload, '$.pinned') = 1)
                            OR (event_type = 'ssh.tested'
                                AND json_extract(payload, '$.ok') = 1))
                     LIMIT 1",
                    params![server, i64::try_from(after).unwrap_or(i64::MAX)],
                    |_| Ok(()),
                )
                .is_ok()
            };
            // Each piece of work is decided by its newest problem, once.
            let mut decided: HashSet<String> = HashSet::new();
            let mut out = Vec::new();
            while out.len() < limit {
                let Some(row) = rows.next()? else { break };
                let e = rows::event(row)?;
                let p = &e.payload;
                let key = match (&e.task_id, e.event_type.as_str()) {
                    (_, "ssh.host_key_changed") => match text(&p["serverId"]) {
                        Some(server) => format!("server:{server}"),
                        None => continue,
                    },
                    // A lead needs a worker for a job: one question per lead and job (Phase 25,
                    // item 2.7), until the team or department has one (the caller checks).
                    (_, "org.hire_needed") => match (text(&p["leadId"]), text(&p["roleId"])) {
                        (Some(lead), Some(role)) => format!("hire:{lead}:{role}"),
                        _ => continue,
                    },
                    (Some(t), _) => format!("task:{t}"),
                    (None, _) => continue,
                };
                if !decided.insert(key) {
                    continue;
                }
                let still = match (e.event_type.as_str(), e.task_id.as_deref()) {
                    ("ssh.host_key_changed", _) => {
                        text(&p["serverId"]).is_some_and(|s| !set_right(&s, e.seq))
                    }
                    ("org.hire_needed", _) => true,
                    ("task.state_changed", Some(t)) => objective(t)
                        .is_some_and(|(company, state)| company && state != TaskState::Succeeded),
                    (_, Some(t)) => objective(t).is_some_and(|(company, _)| company) && running(t),
                    (_, None) => false,
                };
                if still {
                    out.push(e);
                }
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

    /// An objective given to a position: the company's work.
    fn objective(l: &Ledger, text: &str) -> Task {
        l.create_task(
            NewTask {
                requested_by: "owner".into(),
                objective: text.into(),
                priority: 2,
                metadata: json!({ "workforce": { "positionId": "pos-shop" } }),
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
    }

    #[test]
    fn objectives_going_are_newest_first_and_finished_ones_last_to_finish_first() {
        let l = ledger();
        let older = objective(&l, "Order stock");
        let newer = objective(&l, "Write the post");
        let going = objective(&l, "Plan the week");
        // Not the organization's: no position was given it.
        let other = task(&l, "Try the AI tool");
        for t in [&older, &newer, &going, &other] {
            l.transition_task(&t.id, TaskState::Running, "w", None)
                .unwrap();
        }
        // The newer one finishes first, the older one last.
        l.transition_task(&newer.id, TaskState::Succeeded, "w", None)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        l.transition_task(&older.id, TaskState::Failed, "w", None)
            .unwrap();
        let ids = |tasks: Vec<Task>| tasks.into_iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(l.open_objectives(50).unwrap()), [going.id]);
        assert_eq!(
            ids(l.finished_objectives(0, 20).unwrap()),
            [older.id.clone(), newer.id.clone()]
        );
        assert_eq!(ids(l.finished_objectives(0, 1).unwrap()), [older.id]);
        // Counted whole, not a page.
        assert_eq!(l.objective_counts(0).unwrap(), (1, 2));
        assert_eq!(
            l.objective_counts(crate::now_ms() + 60_000).unwrap(),
            (1, 0)
        );
        assert!(l
            .finished_objectives(crate::now_ms() + 60_000, 20)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn what_is_stuck_is_one_problem_per_piece_of_work_still_in_trouble() {
        let l = ledger();
        // A failed objective: stuck.
        let failed = objective(&l, "Order stock");
        fail(&l, &failed);
        // A failed step whose objective then succeeded: not stuck.
        let done = objective(&l, "Write the post");
        let step = child(&l, &done, "Draft", json!({}));
        fail(&l, &step);
        l.transition_task(&done.id, TaskState::Running, "w", None)
            .unwrap();
        l.transition_task(&done.id, TaskState::Succeeded, "w", None)
            .unwrap();
        // Not the company's work: a task nobody in the organization was given, and
        // Diagnostics' test task.
        fail(&l, &task(&l, "Try the AI tool"));
        let test = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "Synthetic diagnostic task".into(),
                    priority: 2,
                    metadata: json!({ "synthetic": true }),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        fail(&l, &test);
        // A refusal while its task still runs: stuck (once, however many).
        let running = objective(&l, "Clean the server");
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
        // A task waiting for its full-time worker to finish another task: ordinary queueing.
        let queued = objective(&l, "Next in line");
        event(
            &l,
            Some(&queued.id),
            "liaison.waiting_for_member",
            json!({ "reason": "waiting for Senior Developer to finish its current task" }),
        );
        // A changed server ID: stuck until it is set right, as Settings → Servers decides.
        for (server, fixed) in [
            ("s1", None),
            (
                "s2",
                Some(("guard.server_changed", json!({ "pinned": true }))),
            ),
            ("s3", Some(("ssh.tested", json!({ "ok": true })))),
            (
                "s4",
                Some(("ssh.connected", json!({ "worker": "Operations Engineer" }))),
            ),
            ("s5", Some(("guard.server_removed", json!({})))),
            // Saved again without a new ID, or a test that failed: still stuck.
            (
                "s6",
                Some(("guard.server_changed", json!({ "pinned": false }))),
            ),
            ("s7", Some(("ssh.tested", json!({ "ok": false })))),
        ] {
            event(
                &l,
                None,
                "ssh.host_key_changed",
                json!({ "serverId": server, "server": server }),
            );
            if let Some((kind, mut payload)) = fixed {
                payload["serverId"] = json!(server);
                event(&l, None, kind, payload);
            }
        }
        // A busy week of ordinary work does not push older problems out.
        for _ in 0..2_100 {
            event(
                &l,
                Some(&running.id),
                "task.state_changed",
                json!({ "from": "running", "to": "blocked" }),
            );
        }
        let stuck = l.problems(0, 50).unwrap();
        let kinds: Vec<(&str, Option<&str>, Option<&str>)> = stuck
            .iter()
            .map(|e| {
                (
                    e.event_type.as_str(),
                    e.task_id.as_deref(),
                    e.payload["serverId"].as_str(),
                )
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ("ssh.host_key_changed", None, Some("s7")),
                ("ssh.host_key_changed", None, Some("s6")),
                ("ssh.host_key_changed", None, Some("s1")),
                ("guard.denied", Some(running.id.as_str()), None),
                ("task.state_changed", Some(failed.id.as_str()), None),
            ]
        );
        assert_eq!(stuck[3].payload["summary"], "run rm -rf", "the newest");
        // At most `limit`, newest first; nothing from before `since`.
        assert_eq!(l.problems(0, 2).unwrap().len(), 2);
        assert!(l.problems(crate::now_ms() + 60_000, 50).unwrap().is_empty());
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
        // And all of it, page by page: the newest stretch holds none of it, so each page looks
        // further back; every event comes once, in order.
        let mut seqs: Vec<u64> = Vec::new();
        timed(
            "the department's whole history, a page at a time",
            &mut || {
                let mut before = None;
                loop {
                    let page = l
                        .scope_events(
                            &ActivityScope::Department("dev".into()),
                            before,
                            MAX_PAGE_EVENTS,
                        )
                        .unwrap();
                    let Some(last) = page.last() else { break };
                    before = Some(last.seq);
                    seqs.extend(page.iter().map(|e| e.seq));
                }
                seqs.len()
            },
        );
        assert_eq!(seqs.len(), 3_000, "every event, each once");
        assert!(seqs.windows(2).all(|w| w[0] > w[1]), "newest first");
        let res = timed("the other department's first page", &mut || {
            l.scope_events(&ActivityScope::Department("res".into()), None, 50)
                .unwrap()
                .len()
        });
        assert_eq!(res, 50);
        timed("what is stuck", &mut || l.problems(0, 30).unwrap().len());
    }
}
