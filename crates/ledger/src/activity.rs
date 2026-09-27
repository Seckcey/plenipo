//! Activity over time: events counted in fixed time buckets (Phase 12A, ADR-029 §7).
//!
//! Downsampling: a series always has the number of buckets asked for (the UI asks for 96 over
//! 24 hours: 15 minutes each). A longer range makes each bucket wider and adds the counts up,
//! so nothing is dropped. Each bucket also counts problems (failures, blocks, refusals) and
//! requests for approval, so a strip can show where they happened.

use rusqlite::{params, Connection, OptionalExtension as _};

use crate::dto::{ActivityBucket, ActivityScope, ActivitySeries};
use crate::error::{LedgerError, Result};
use crate::Ledger;

/// The most buckets one series may have (5-minute buckets over a day).
pub const MAX_ACTIVITY_BUCKETS: u32 = 288;
/// The longest range one series may cover.
pub const MAX_ACTIVITY_RANGE_MS: u64 = 366 * 24 * 3_600_000;
/// The most scopes one request may ask for (one per card on screen).
pub const MAX_ACTIVITY_SCOPES: usize = 500;

/// An event that means something went wrong. Kept in one place so the UI and the Ledger agree.
const PROBLEM: &str = "(e.event_type LIKE '%failed'
     OR e.event_type LIKE '%rejected'
     OR e.event_type LIKE '%refused'
     OR e.event_type IN ('guard.denied', 'execution.timed_out', 'ssh.host_key_changed',
                         'approval.expired', 'diagnostic.failure')
     OR (e.event_type = 'task.state_changed'
         AND json_extract(e.payload, '$.to') IN ('failed', 'blocked')))";

/// A request for the owner's approval.
const WAITING: &str = "e.event_type = 'approval.requested'";

/// Where a scope's work is: the position at the top of its team, and its projects.
struct Reach {
    /// The team's lead; the team is it and everyone reporting to it, directly or not.
    lead: Option<String>,
    /// Projects whose tasks count.
    projects: Projects,
}

enum Projects {
    None,
    One(String),
    OfDepartment(String),
}

fn reach(c: &Connection, scope: &ActivityScope) -> Result<Option<Reach>> {
    let found = |sql: &str, id: &str| -> Result<Option<Option<String>>> {
        Ok(c.query_row(sql, [id], |r| r.get::<_, Option<String>>(0))
            .optional()?)
    };
    Ok(match scope {
        ActivityScope::All => None,
        ActivityScope::Department(id) => {
            let head = found("SELECT head_position_id FROM departments WHERE id = ?1", id)?
                .ok_or_else(|| LedgerError::NotFound(format!("department {id}")))?;
            Some(Reach {
                lead: head,
                projects: Projects::OfDepartment(id.clone()),
            })
        }
        ActivityScope::Project(id) => {
            let coordinator = found(
                "SELECT coordinator_position_id FROM projects WHERE id = ?1",
                id,
            )?
            .ok_or_else(|| LedgerError::NotFound(format!("project {id}")))?;
            Some(Reach {
                lead: coordinator,
                projects: Projects::One(id.clone()),
            })
        }
        ActivityScope::Position(id) => {
            found("SELECT id FROM positions WHERE id = ?1", id)?
                .ok_or_else(|| LedgerError::NotFound(format!("position {id}")))?;
            Some(Reach {
                lead: Some(id.clone()),
                projects: Projects::None,
            })
        }
    })
}

fn validate(from: u64, to: u64, buckets: u32) -> Result<u64> {
    if to <= from {
        return Err(LedgerError::InvalidInput(
            "an activity range must end after it starts".into(),
        ));
    }
    if to - from > MAX_ACTIVITY_RANGE_MS {
        return Err(LedgerError::InvalidInput(
            "an activity range can cover at most a year".into(),
        ));
    }
    if !(1..=MAX_ACTIVITY_BUCKETS).contains(&buckets) {
        return Err(LedgerError::InvalidInput(format!(
            "an activity series has 1–{MAX_ACTIVITY_BUCKETS} buckets"
        )));
    }
    Ok((to - from).div_ceil(u64::from(buckets)))
}

fn as_i64(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX)
}

fn series(
    c: &Connection,
    scope: &ActivityScope,
    from: u64,
    to: u64,
    buckets: u32,
) -> Result<ActivitySeries> {
    let width = validate(from, to, buckets)?;
    let head = format!(
        "SELECT (e.created_at - ?1) / ?3 AS b, COUNT(*),
                SUM(CASE WHEN {PROBLEM} THEN 1 ELSE 0 END),
                SUM(CASE WHEN {WAITING} THEN 1 ELSE 0 END)"
    );
    let range = "e.created_at >= ?1 AND e.created_at < ?2";
    let mut out = vec![ActivityBucket::default(); buckets as usize];
    let mut add = |b: i64, events: i64, problems: i64, waiting: i64| {
        let Some(slot) = usize::try_from(b).ok().and_then(|i| out.get_mut(i)) else {
            return;
        };
        let n = |v: i64| u32::try_from(v).unwrap_or(u32::MAX);
        slot.events = n(events);
        slot.problems = n(problems);
        slot.waiting = n(waiting);
    };
    let row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, i64, i64, i64)> {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    };
    let (from_i, to_i, width_i) = (as_i64(from), as_i64(to), as_i64(width));

    match reach(c, scope)? {
        None => {
            let sql = format!("{head} FROM events e WHERE {range} GROUP BY b");
            let mut stmt = c.prepare(&sql)?;
            for r in stmt.query_map(params![from_i, to_i, width_i], row)? {
                let (b, e, p, w) = r?;
                add(b, e, p, w);
            }
        }
        Some(reach) => {
            let (projects, project_param) = match &reach.projects {
                // Always bind ?5, so every scope uses the same parameters.
                Projects::None => ("(0 AND ?5 IS NULL)", None),
                Projects::One(id) => ("t.project_id = ?5", Some(id.as_str())),
                Projects::OfDepartment(id) => (
                    "t.project_id IN (SELECT id FROM projects WHERE department_id = ?5)",
                    Some(id.as_str()),
                ),
            };
            // The team: the lead and everyone below it (archived positions keep their place,
            // so past work still counts). UNION stops at a cycle.
            let sql = format!(
                "WITH RECURSIVE team(id) AS (
                     SELECT ?4 WHERE ?4 IS NOT NULL
                     UNION
                     SELECT p.id FROM positions p JOIN team ON p.reports_to = team.id
                 )
                 {head}
                 FROM events e JOIN tasks t ON t.id = e.task_id
                 WHERE {range}
                   AND (json_extract(t.metadata, '$.workforce.positionId') IN (SELECT id FROM team)
                        OR {projects})
                 GROUP BY b"
            );
            let mut stmt = c.prepare(&sql)?;
            for r in stmt.query_map(
                params![from_i, to_i, width_i, reach.lead, project_param],
                row,
            )? {
                let (b, e, p, w) = r?;
                add(b, e, p, w);
            }
        }
    }
    Ok(ActivitySeries {
        from,
        to,
        bucket_ms: width,
        buckets: out,
    })
}

impl Ledger {
    /// Events in `[from, to)` counted into `buckets` equal buckets, for each scope.
    pub fn activity(
        &self,
        scopes: &[ActivityScope],
        from: u64,
        to: u64,
        buckets: u32,
    ) -> Result<Vec<ActivitySeries>> {
        if scopes.len() > MAX_ACTIVITY_SCOPES {
            return Err(LedgerError::InvalidInput(format!(
                "at most {MAX_ACTIVITY_SCOPES} activity series at a time"
            )));
        }
        validate(from, to, buckets)?;
        self.read(|c| {
            scopes
                .iter()
                .map(|s| series(c, s, from, to, buckets))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use serde_json::json;

    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 24 * HOUR;
    /// A fixed "now" so buckets are predictable.
    const NOW: u64 = 1_790_000_000_000;

    /// A tiny organization: Development (head D), its project Website (supervisor W, with a
    /// worker S under it), and Research (head R) with nothing under it.
    fn org(l: &Ledger) {
        let conn = crate::lock(&l.conn);
        conn.execute_batch(
            "INSERT INTO roles (id, name, role_type, persistent, created_at) VALUES
                 ('r-m', 'Test Manager', 'department_manager', 1, 0),
                 ('r-s', 'Test Supervisor', 'project_coordinator', 1, 0),
                 ('r-w', 'Test Worker', 'worker', 0, 0);
             INSERT INTO positions (id, title, role_id, reports_to, runtime_id, state, created_at, updated_at) VALUES
                 ('D', 'Development Manager', 'r-m', NULL, 'claude', 'active', 0, 0),
                 ('W', 'Website Supervisor', 'r-s', 'D', 'claude', 'active', 0, 0),
                 ('S', 'Senior Developer', 'r-w', 'W', 'claude', 'active', 0, 0),
                 ('R', 'Research Manager', 'r-m', NULL, 'claude', 'active', 0, 0);
             INSERT INTO departments (id, name, created_at, head_position_id) VALUES
                 ('dev', 'Development', 0, 'D'),
                 ('res', 'Research', 0, 'R'),
                 ('empty', 'No head', 0, NULL);
             INSERT INTO projects (id, name, department_id, created_at, coordinator_position_id) VALUES
                 ('web', 'Website', 'dev', 0, 'W'),
                 ('lone', 'Lone project', 'dev', 0, NULL);",
        )
        .unwrap();
    }

    fn task_for(l: &Ledger, id: &str, project: Option<&str>, position: Option<&str>) {
        let metadata = match position {
            Some(p) => json!({ "workforce": { "positionId": p } }),
            None => json!({}),
        };
        crate::lock(&l.conn)
            .execute(
                "INSERT INTO tasks (id, requested_by, project_id, objective, state, metadata, created_at, updated_at)
                 VALUES (?1, 'owner', ?2, 'test', 'running', ?3, 0, 0)",
                params![id, project, metadata.to_string()],
            )
            .unwrap();
    }

    fn event(l: &Ledger, task: Option<&str>, event_type: &str, payload: serde_json::Value, at: u64) {
        crate::lock(&l.conn)
            .execute(
                "INSERT INTO events (id, task_id, source, event_type, payload, created_at)
                 VALUES (?1, ?2, 'test', ?3, ?4, ?5)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    task,
                    event_type,
                    payload.to_string(),
                    as_i64(at)
                ],
            )
            .unwrap();
    }

    fn totals(s: &ActivitySeries) -> (u32, u32, u32) {
        s.buckets.iter().fold((0, 0, 0), |t, b| {
            (t.0 + b.events, t.1 + b.problems, t.2 + b.waiting)
        })
    }

    #[test]
    fn counts_into_fixed_buckets() {
        let l = ledger();
        let from = NOW - DAY;
        event(&l, None, "diagnostic.echo", json!({}), from);
        event(&l, None, "diagnostic.echo", json!({}), from + 14 * 60_000);
        event(&l, None, "diagnostic.echo", json!({}), from + 15 * 60_000);
        event(&l, None, "diagnostic.echo", json!({}), NOW - 1);
        event(&l, None, "diagnostic.echo", json!({}), NOW); // the end is not included
        event(&l, None, "diagnostic.echo", json!({}), from - 1); // before the start
        let s = &l.activity(&[ActivityScope::All], from, NOW, 96).unwrap()[0];
        assert_eq!(s.buckets.len(), 96);
        assert_eq!(s.bucket_ms, 15 * 60_000);
        assert_eq!(s.buckets[0].events, 2);
        assert_eq!(s.buckets[1].events, 1);
        assert_eq!(s.buckets[95].events, 1);
        assert_eq!(totals(s), (4, 0, 0));
    }

    #[test]
    fn a_longer_range_widens_buckets_and_drops_nothing() {
        let l = ledger();
        let from = NOW - 7 * DAY;
        for i in 0..70 {
            event(&l, None, "diagnostic.echo", json!({}), from + i * 2 * HOUR + 1);
        }
        let s = &l.activity(&[ActivityScope::All], from, NOW, 96).unwrap()[0];
        assert_eq!(s.bucket_ms, (7 * DAY).div_ceil(96));
        assert_eq!(totals(s).0, 70);
    }

    #[test]
    fn counts_problems_and_approvals() {
        let l = ledger();
        let from = NOW - DAY;
        let at = from + HOUR;
        for (t, payload) in [
            ("turn.failed", json!({})),
            ("liaison.dispatch_failed", json!({})),
            ("task.transition_rejected", json!({})),
            ("liaison.reply_refused", json!({})),
            ("guard.denied", json!({})),
            ("execution.timed_out", json!({})),
            ("ssh.host_key_changed", json!({})),
            ("approval.expired", json!({})),
            ("task.state_changed", json!({ "from": "running", "to": "failed" })),
            ("task.state_changed", json!({ "from": "running", "to": "blocked" })),
        ] {
            event(&l, None, t, payload, at);
        }
        // Not problems:
        event(&l, None, "task.state_changed", json!({ "from": "running", "to": "succeeded" }), at);
        event(&l, None, "approval.resolved", json!({ "approved": false }), at);
        event(&l, None, "approval.requested", json!({}), at);
        let s = &l.activity(&[ActivityScope::All], from, NOW, 24).unwrap()[0];
        assert_eq!(s.buckets[1], ActivityBucket { events: 13, problems: 10, waiting: 1 });
    }

    #[test]
    fn scopes_follow_the_organization() {
        let l = ledger();
        org(&l);
        let at = NOW - HOUR;
        task_for(&l, "t-web", Some("web"), None); // a Website task
        task_for(&l, "t-sd", None, Some("S")); // work of a worker in the Website team
        task_for(&l, "t-lone", Some("lone"), None); // another Development project
        task_for(&l, "t-res", None, Some("R")); // Research's own work
        task_for(&l, "t-none", None, None); // nobody's
        for t in ["t-web", "t-sd", "t-lone", "t-res", "t-none"] {
            event(&l, Some(t), "diagnostic.echo", json!({}), at);
        }
        event(&l, None, "org.settings_changed", json!({}), at);

        let scopes = [
            ActivityScope::All,
            ActivityScope::Department("dev".into()),
            ActivityScope::Project("web".into()),
            ActivityScope::Position("S".into()),
            ActivityScope::Position("W".into()),
            ActivityScope::Department("res".into()),
            ActivityScope::Department("empty".into()),
            ActivityScope::Project("lone".into()),
        ];
        let got: Vec<u32> = l
            .activity(&scopes, NOW - DAY, NOW, 96)
            .unwrap()
            .iter()
            .map(|s| totals(s).0)
            .collect();
        assert_eq!(got, vec![6, 3, 2, 1, 1, 1, 0, 1]);
    }

    #[test]
    fn refuses_bad_requests() {
        let l = ledger();
        org(&l);
        let bad = |scopes: &[ActivityScope], from: u64, to: u64, buckets: u32| {
            l.activity(scopes, from, to, buckets).unwrap_err()
        };
        assert!(matches!(bad(&[ActivityScope::All], NOW, NOW, 96), LedgerError::InvalidInput(_)));
        assert!(matches!(bad(&[ActivityScope::All], NOW - DAY, NOW, 0), LedgerError::InvalidInput(_)));
        assert!(matches!(bad(&[ActivityScope::All], NOW - DAY, NOW, 289), LedgerError::InvalidInput(_)));
        assert!(matches!(
            bad(&[ActivityScope::All], NOW - 400 * DAY, NOW, 96),
            LedgerError::InvalidInput(_)
        ));
        assert!(matches!(
            bad(&[ActivityScope::Project("nope".into())], NOW - DAY, NOW, 96),
            LedgerError::NotFound(_)
        ));
        let many = vec![ActivityScope::All; MAX_ACTIVITY_SCOPES + 1];
        assert!(matches!(bad(&many, NOW - DAY, NOW, 96), LedgerError::InvalidInput(_)));
        assert!(l.activity(&[], NOW - DAY, NOW, 96).unwrap().is_empty());
    }

    #[test]
    fn uses_the_time_index() {
        let l = ledger();
        let conn = crate::lock(&l.conn);
        let plan: Vec<String> = conn
            .prepare(
                "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM events e
                 WHERE e.created_at >= 1 AND e.created_at < 2",
            )
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            plan.iter().any(|p| p.contains("events_by_created")),
            "{plan:?}"
        );
    }

    #[test]
    fn scopes_are_tagged_for_the_ui() {
        assert_eq!(
            serde_json::to_value(ActivityScope::Project("web".into())).unwrap(),
            json!({ "kind": "project", "id": "web" })
        );
        assert_eq!(
            serde_json::to_value(ActivityScope::All).unwrap(),
            json!({ "kind": "all" })
        );
    }
}
