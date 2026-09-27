//! Activity over time: events counted in fixed time buckets (Phase 12A, ADR-030 §7).
//!
//! Downsampling: a series always has the number of buckets asked for (the UI asks for 96 over
//! 24 hours: 15 minutes each). A longer range makes each bucket wider and adds the counts up,
//! so nothing is dropped. Each bucket also counts problems (failures and refusals) and requests
//! for approval, so a strip can show where they happened.
//!
//! One request reads the time window once, grouped by bucket, position, and project, and then
//! counts those groups for every scope asked for. So a page of cards costs one pass over the
//! window, however many cards it has, and the Ledger is held only for that pass.

use std::collections::{HashMap, HashSet};

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

/// An event that means something went wrong. A task that is `blocked` is waiting on its team's
/// replies (normal delegation), so it is not a problem. Execution events are named for their
/// end state (`execution.failed`, `execution.timed_out`, `execution.interrupted`).
const PROBLEM: &str = "(e.event_type LIKE '%failed'
     OR e.event_type LIKE '%rejected'
     OR e.event_type LIKE '%refused'
     OR e.event_type IN ('guard.denied', 'execution.timed_out', 'execution.interrupted',
                         'ssh.host_key_changed', 'approval.expired', 'browser.tab_lost',
                         'diagnostic.failure')
     OR (e.event_type = 'task.state_changed'
         AND json_extract(e.payload, '$.to') = 'failed'))";

/// A request for the owner's approval.
const WAITING: &str = "e.event_type = 'approval.requested'";

/// The one pass over the window: events grouped by bucket, the task's position, and its project.
/// Events without a task count only for the whole organization.
fn scan_sql() -> String {
    format!(
        "SELECT (e.created_at - ?1) / ?3 AS b,
                json_extract(t.metadata, '$.workforce.positionId') AS pos,
                t.project_id AS proj,
                COUNT(*),
                SUM(CASE WHEN {PROBLEM} THEN 1 ELSE 0 END),
                SUM(CASE WHEN {WAITING} THEN 1 ELSE 0 END)
         FROM events e LEFT JOIN tasks t ON t.id = e.task_id
         WHERE e.created_at >= ?1 AND e.created_at < ?2
         GROUP BY b, pos, proj"
    )
}

/// The organization's shape, read once per request.
struct Org {
    /// Positions reporting to each position (archived ones too, so past work still counts).
    reports: HashMap<String, Vec<String>>,
    /// Positions that head a department.
    heads: HashSet<String>,
    /// Positions that lead a project.
    leads: HashSet<String>,
}

impl Org {
    fn read(c: &Connection) -> Result<Self> {
        let mut reports: HashMap<String, Vec<String>> = HashMap::new();
        let mut stmt = c.prepare("SELECT id, reports_to FROM positions")?;
        for row in stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })? {
            let (id, up) = row?;
            if let Some(up) = up {
                reports.entry(up).or_default().push(id);
            }
        }
        let ids = |sql: &str| -> Result<HashSet<String>> {
            let mut stmt = c.prepare(sql)?;
            let out = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(out)
        };
        Ok(Self {
            reports,
            heads: ids(
                "SELECT head_position_id FROM departments WHERE head_position_id IS NOT NULL",
            )?,
            leads: ids("SELECT coordinator_position_id FROM projects \
                 WHERE coordinator_position_id IS NOT NULL")?,
        })
    }

    /// `lead` and everyone below it, not going past a position in `stop` (another department's
    /// head, or another project's lead), so a department counts its own work only.
    fn team(&self, lead: &str, stop: &HashSet<String>) -> HashSet<String> {
        let mut team = HashSet::from([lead.to_owned()]);
        let mut next = vec![lead.to_owned()];
        while let Some(at) = next.pop() {
            for child in self.reports.get(&at).into_iter().flatten() {
                if stop.contains(child) || !team.insert(child.clone()) {
                    continue;
                }
                next.push(child.clone());
            }
        }
        team
    }
}

/// What a scope counts: every event, or the tasks of its team's positions and its projects.
enum Reach {
    All,
    Some {
        positions: HashSet<String>,
        projects: HashSet<String>,
    },
}

impl Reach {
    fn counts(&self, pos: Option<&str>, proj: Option<&str>) -> bool {
        match self {
            Self::All => true,
            Self::Some {
                positions,
                projects,
            } => {
                pos.is_some_and(|p| positions.contains(p))
                    || proj.is_some_and(|p| projects.contains(p))
            }
        }
    }
}

fn reach(c: &Connection, org: &Org, scope: &ActivityScope) -> Result<Reach> {
    let lookup = |sql: &str, id: &str| -> Result<Option<Option<String>>> {
        Ok(c.query_row(sql, [id], |r| r.get::<_, Option<String>>(0))
            .optional()?)
    };
    let without = |set: &HashSet<String>, keep: Option<&String>| -> HashSet<String> {
        set.iter().filter(|p| Some(*p) != keep).cloned().collect()
    };
    Ok(match scope {
        ActivityScope::All => Reach::All,
        ActivityScope::Department(id) => {
            let head = lookup("SELECT head_position_id FROM departments WHERE id = ?1", id)?
                .ok_or_else(|| LedgerError::NotFound(format!("department {id}")))?;
            let mut stmt = c.prepare("SELECT id FROM projects WHERE department_id = ?1")?;
            let projects = stmt
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            Reach::Some {
                positions: head
                    .as_ref()
                    .map(|h| org.team(h, &without(&org.heads, Some(h))))
                    .unwrap_or_default(),
                projects,
            }
        }
        ActivityScope::Project(id) => {
            let lead = lookup(
                "SELECT coordinator_position_id FROM projects WHERE id = ?1",
                id,
            )?
            .ok_or_else(|| LedgerError::NotFound(format!("project {id}")))?;
            let stop: HashSet<String> = without(&org.leads, lead.as_ref())
                .union(&org.heads)
                .cloned()
                .collect();
            Reach::Some {
                positions: lead
                    .as_ref()
                    .map(|l| org.team(l, &stop))
                    .unwrap_or_default(),
                projects: HashSet::from([id.clone()]),
            }
        }
        ActivityScope::Position(id) => {
            lookup("SELECT id FROM positions WHERE id = ?1", id)?
                .ok_or_else(|| LedgerError::NotFound(format!("position {id}")))?;
            Reach::Some {
                positions: org.team(id, &HashSet::new()),
                projects: HashSet::new(),
            }
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

/// One group from the scan: its bucket, position, project, and counts.
struct Group {
    bucket: usize,
    pos: Option<String>,
    proj: Option<String>,
    counts: ActivityBucket,
}

fn scan(c: &Connection, from: u64, to: u64, width: u64) -> Result<Vec<Group>> {
    let n = |v: i64| u32::try_from(v).unwrap_or(u32::MAX);
    let mut stmt = c.prepare(&scan_sql())?;
    let rows = stmt
        .query_map(params![as_i64(from), as_i64(to), as_i64(width)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(b, pos, proj, events, problems, waiting)| {
            Some(Group {
                bucket: usize::try_from(b).ok()?,
                pos,
                proj,
                counts: ActivityBucket {
                    events: n(events),
                    problems: n(problems),
                    waiting: n(waiting),
                },
            })
        })
        .collect())
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
        let width = validate(from, to, buckets)?;
        if scopes.is_empty() {
            return Ok(Vec::new());
        }
        let (reaches, groups) = self.read(|c| {
            let org = Org::read(c)?;
            let reaches = scopes
                .iter()
                .map(|s| reach(c, &org, s))
                .collect::<Result<Vec<_>>>()?;
            Ok((reaches, scan(c, from, to, width)?))
        })?;
        Ok(reaches
            .iter()
            .map(|reach| {
                let mut out = vec![ActivityBucket::default(); buckets as usize];
                for g in &groups {
                    if !reach.counts(g.pos.as_deref(), g.proj.as_deref()) {
                        continue;
                    }
                    if let Some(slot) = out.get_mut(g.bucket) {
                        slot.events = slot.events.saturating_add(g.counts.events);
                        slot.problems = slot.problems.saturating_add(g.counts.problems);
                        slot.waiting = slot.waiting.saturating_add(g.counts.waiting);
                    }
                }
                ActivitySeries {
                    from,
                    to,
                    bucket_ms: width,
                    buckets: out,
                }
            })
            .collect())
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

    fn event(
        l: &Ledger,
        task: Option<&str>,
        event_type: &str,
        payload: serde_json::Value,
        at: u64,
    ) {
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
            event(
                &l,
                None,
                "diagnostic.echo",
                json!({}),
                from + i * 2 * HOUR + 1,
            );
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
            ("execution.failed", json!({})),
            ("execution.interrupted", json!({})),
            ("browser.tab_lost", json!({})),
            ("liaison.dispatch_failed", json!({})),
            ("task.transition_rejected", json!({})),
            ("liaison.reply_refused", json!({})),
            ("guard.denied", json!({})),
            ("execution.timed_out", json!({})),
            ("ssh.host_key_changed", json!({})),
            ("approval.expired", json!({})),
            (
                "task.state_changed",
                json!({ "from": "running", "to": "failed" }),
            ),
        ] {
            event(&l, None, t, payload, at);
        }
        // Not problems: waiting on the team's replies (normal delegation), a success, and the
        // owner's answer.
        event(
            &l,
            None,
            "task.state_changed",
            json!({ "from": "running", "to": "blocked" }),
            at,
        );
        event(
            &l,
            None,
            "task.state_changed",
            json!({ "from": "running", "to": "succeeded" }),
            at,
        );
        event(
            &l,
            None,
            "approval.resolved",
            json!({ "approved": false }),
            at,
        );
        event(&l, None, "approval.requested", json!({}), at);
        let s = &l.activity(&[ActivityScope::All], from, NOW, 24).unwrap()[0];
        assert_eq!(
            s.buckets[1],
            ActivityBucket {
                events: 15,
                problems: 11,
                waiting: 1
            }
        );
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
    fn a_department_counts_its_own_work_not_a_department_below_it() {
        let l = ledger();
        org(&l);
        // The VP heads Leadership; the Development Manager (D) reports to the VP.
        crate::lock(&l.conn)
            .execute_batch(
                "INSERT INTO roles (id, name, role_type, persistent, created_at) VALUES
                     ('r-vp', 'Test VP', 'superintendent', 1, 0);
                 INSERT INTO positions (id, title, role_id, reports_to, runtime_id, state, created_at, updated_at) VALUES
                     ('V', 'VP', 'r-vp', NULL, 'claude', 'active', 0, 0);
                 UPDATE positions SET reports_to = 'V' WHERE id = 'D';
                 INSERT INTO departments (id, name, created_at, head_position_id) VALUES
                     ('lead', 'Leadership', 0, 'V');",
            )
            .unwrap();
        let at = NOW - HOUR;
        task_for(&l, "t-vp", None, Some("V")); // the VP's own work
        task_for(&l, "t-sd", None, Some("S")); // Development's worker
        task_for(&l, "t-web", Some("web"), None); // a Development project
        for t in ["t-vp", "t-sd", "t-web"] {
            event(&l, Some(t), "diagnostic.echo", json!({}), at);
        }
        let got: Vec<u32> = l
            .activity(
                &[
                    ActivityScope::Department("lead".into()),
                    ActivityScope::Department("dev".into()),
                    ActivityScope::Position("V".into()),
                ],
                NOW - DAY,
                NOW,
                96,
            )
            .unwrap()
            .iter()
            .map(|s| totals(s).0)
            .collect();
        // Leadership: only the VP's own work. Development: its worker and its project. The VP's
        // position: the work of the VP and everyone under it (a project task that no position
        // took counts for its project and department, not for a position).
        assert_eq!(got, vec![1, 2, 2]);
    }

    #[test]
    fn refuses_bad_requests() {
        let l = ledger();
        org(&l);
        let bad = |scopes: &[ActivityScope], from: u64, to: u64, buckets: u32| {
            l.activity(scopes, from, to, buckets).unwrap_err()
        };
        assert!(matches!(
            bad(&[ActivityScope::All], NOW, NOW, 96),
            LedgerError::InvalidInput(_)
        ));
        assert!(matches!(
            bad(&[ActivityScope::All], NOW - DAY, NOW, 0),
            LedgerError::InvalidInput(_)
        ));
        assert!(matches!(
            bad(&[ActivityScope::All], NOW - DAY, NOW, 289),
            LedgerError::InvalidInput(_)
        ));
        assert!(matches!(
            bad(&[ActivityScope::All], NOW - 400 * DAY, NOW, 96),
            LedgerError::InvalidInput(_)
        ));
        assert!(matches!(
            bad(&[ActivityScope::Project("nope".into())], NOW - DAY, NOW, 96),
            LedgerError::NotFound(_)
        ));
        let many = vec![ActivityScope::All; MAX_ACTIVITY_SCOPES + 1];
        assert!(matches!(
            bad(&many, NOW - DAY, NOW, 96),
            LedgerError::InvalidInput(_)
        ));
        assert!(l.activity(&[], NOW - DAY, NOW, 96).unwrap().is_empty());
    }

    #[test]
    fn uses_the_time_index() {
        let l = ledger();
        let conn = crate::lock(&l.conn);
        // The real query, as each request runs it once.
        let plan: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {}", scan_sql()))
            .unwrap()
            .query_map(params![1_i64, 2_i64, 1_i64], |r| r.get::<_, String>(3))
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
