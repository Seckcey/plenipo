//! Working copies (Phase 8, ADR-016): the branch and git worktree an objective's workers use.
//! Plenipo's capability broker makes them; this module records them, and the facts about each
//! branch (its commits and changed files) as they change, each with its `workspace.*` event in
//! the same transaction.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use crate::dto::{NewEvent, NewWorkspace, Workspace, WorkspaceFacts, WorkspaceState};
use crate::error::{LedgerError, Result};
use crate::rows::{parse_enum, u64_of};
use crate::{events, tasks, Ledger};

const COLS: &str = "id, project_id, correlation_id, root_task_id, parent_id, repository, \
    subfolder, path, branch, base_ref, base_commit, state, facts, created_at, updated_at, \
    removed_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: r.get(0)?,
        project_id: r.get(1)?,
        correlation_id: r.get(2)?,
        root_task_id: r.get(3)?,
        parent_id: r.get(4)?,
        repository: r.get(5)?,
        subfolder: r.get(6)?,
        path: r.get(7)?,
        branch: r.get(8)?,
        base_ref: r.get(9)?,
        base_commit: r.get(10)?,
        state: parse_enum(11, r.get(11)?, WorkspaceState::parse)?,
        facts: serde_json::from_str(&r.get::<_, String>(12)?).unwrap_or_default(),
        created_at: u64_of(r.get(13)?),
        updated_at: u64_of(r.get(14)?),
        removed_at: r.get::<_, Option<i64>>(15)?.map(u64_of),
    })
}

fn get(conn: &Connection, id: &str) -> Result<Option<Workspace>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM workspaces WHERE id = ?1"),
            [id],
            row,
        )
        .optional()?)
}

fn require(conn: &Connection, id: &str) -> Result<Workspace> {
    get(conn, id)?.ok_or_else(|| LedgerError::NotFound(format!("working copy {id}")))
}

fn many(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> Result<Vec<Workspace>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(args, row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn invalid(message: impl Into<String>) -> LedgerError {
    LedgerError::InvalidInput(message.into())
}

/// A branch name Plenipo makes: `plenipo/` then lower-case letters, digits, `-`, `/`, and `.`.
fn valid_branch(branch: &str) -> bool {
    branch.starts_with("plenipo/")
        && branch.len() <= 200
        && !branch.ends_with(['/', '.'])
        && !branch.contains("..")
        && !branch.contains("//")
        && branch
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '/' | '.'))
}

fn event(task_id: &str, actor: &str, event_type: &str, payload: serde_json::Value) -> NewEvent {
    NewEvent {
        task_id: Some(task_id.into()),
        source: actor.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    }
}

/// The facts that matter for a change (not when they were checked).
fn same_facts(a: &WorkspaceFacts, b: &WorkspaceFacts) -> bool {
    a.head == b.head
        && a.commits == b.commits
        && a.files == b.files
        && a.uncommitted == b.uncommitted
        && a.pushed == b.pushed
}

impl Ledger {
    /// Record a working copy made for an objective, with `workspace.created` on `task_id` (the
    /// task whose step needed it).
    pub fn create_workspace(
        &self,
        new: &NewWorkspace,
        task_id: &str,
        actor: &str,
    ) -> Result<Workspace> {
        if !valid_branch(&new.branch) {
            return Err(invalid(format!(
                "{:?} is not a branch Plenipo makes",
                new.branch
            )));
        }
        for (what, value) in [
            ("repository", &new.repository),
            ("path", &new.path),
            ("base commit", &new.base_commit),
        ] {
            if value.trim().is_empty() {
                return Err(invalid(format!("a working copy needs its {what}")));
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.write(|tx, out| {
            tasks::require(tx, task_id)?;
            let project: Option<String> = tx
                .query_row(
                    "SELECT status FROM projects WHERE id = ?1",
                    [&new.project_id],
                    |r| r.get(0),
                )
                .optional()?;
            if project.is_none() {
                return Err(LedgerError::NotFound(format!("project {}", new.project_id)));
            }
            if let Some(parent) = &new.parent_id {
                let parent = require(tx, parent)?;
                if parent.correlation_id != new.correlation_id
                    || parent.project_id != new.project_id
                {
                    return Err(invalid(
                        "a second working copy belongs to the same objective and project",
                    ));
                }
            }
            let now = crate::now_ms() as i64;
            tx.execute(
                "INSERT INTO workspaces (id, project_id, correlation_id, root_task_id, parent_id,
                     repository, subfolder, path, branch, base_ref, base_commit, state, facts,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'active', '{}', ?12, ?12)",
                params![
                    id,
                    new.project_id,
                    new.correlation_id,
                    new.root_task_id,
                    new.parent_id,
                    new.repository,
                    new.subfolder,
                    new.path,
                    new.branch,
                    new.base_ref,
                    new.base_commit,
                    now
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _)
                    if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    invalid("this objective already has its working copy in this project")
                }
                e => e.into(),
            })?;
            out.push(events::insert(
                tx,
                event(
                    task_id,
                    actor,
                    "workspace.created",
                    json!({
                        "workspaceId": id,
                        "projectId": new.project_id,
                        "correlationId": new.correlation_id,
                        "branch": new.branch,
                        "baseRef": new.base_ref,
                        "baseCommit": new.base_commit,
                        "path": new.path,
                        "parentId": new.parent_id,
                    }),
                ),
            )?);
            require(tx, &id)
        })
    }

    pub fn workspace(&self, id: &str) -> Result<Option<Workspace>> {
        self.read(|c| get(c, id))
    }

    /// The objective's working copy in a project (its first; not a second one).
    pub fn objective_workspace(
        &self,
        project_id: &str,
        correlation_id: &str,
    ) -> Result<Option<Workspace>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {COLS} FROM workspaces
                     WHERE project_id = ?1 AND correlation_id = ?2 AND parent_id IS NULL"
                ),
                [project_id, correlation_id],
                row,
            )
            .optional()?)
        })
    }

    /// Every working copy of an objective (its workflow), oldest first.
    pub fn workflow_workspaces(&self, correlation_id: &str) -> Result<Vec<Workspace>> {
        self.read(|c| {
            many(
                c,
                &format!(
                    "SELECT {COLS} FROM workspaces WHERE correlation_id = ?1
                     ORDER BY created_at, rowid"
                ),
                [correlation_id],
            )
        })
    }

    /// A project's working copies, newest first.
    pub fn project_workspaces(&self, project_id: &str, limit: u32) -> Result<Vec<Workspace>> {
        self.read(|c| {
            many(
                c,
                &format!(
                    "SELECT {COLS} FROM workspaces WHERE project_id = ?1
                     ORDER BY created_at DESC, rowid DESC LIMIT ?2"
                ),
                params![project_id, limit],
            )
        })
    }

    /// Record what is on a working copy's branch now. `workspace.updated` goes on `task_id`
    /// (the task whose step changed it) only when something changed.
    pub fn update_workspace_facts(
        &self,
        id: &str,
        facts: &WorkspaceFacts,
        task_id: &str,
        actor: &str,
    ) -> Result<Workspace> {
        self.write(|tx, out| {
            let current = require(tx, id)?;
            let changed = !same_facts(&current.facts, facts);
            tx.execute(
                "UPDATE workspaces SET facts = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, serde_json::to_string(facts)?, crate::now_ms() as i64],
            )?;
            if changed && tasks::get(tx, task_id)?.is_some() {
                out.push(events::insert(
                    tx,
                    event(
                        task_id,
                        actor,
                        "workspace.updated",
                        json!({
                            "workspaceId": id,
                            "branch": current.branch,
                            "head": facts.head,
                            "commits": facts.commits.len(),
                            "files": facts.files.len(),
                            "uncommitted": facts.uncommitted,
                            "pushed": facts.pushed,
                        }),
                    ),
                )?);
            }
            require(tx, id)
        })
    }

    /// Mark a working copy removed (its folder is gone; the branch stays), with
    /// `workspace.removed` on its objective's task.
    pub fn remove_workspace(&self, id: &str, actor: &str) -> Result<Workspace> {
        self.write(|tx, out| {
            let current = require(tx, id)?;
            if current.state == WorkspaceState::Removed {
                return Ok(current);
            }
            let now = crate::now_ms() as i64;
            tx.execute(
                "UPDATE workspaces SET state = 'removed', removed_at = ?2, updated_at = ?2
                 WHERE id = ?1",
                params![id, now],
            )?;
            if let Some(task) = &current.root_task_id {
                out.push(events::insert(
                    tx,
                    event(
                        task,
                        actor,
                        "workspace.removed",
                        json!({
                            "workspaceId": id,
                            "branch": current.branch,
                            "path": current.path,
                        }),
                    ),
                )?);
            }
            require(tx, id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{CommitInfo, FileChange, NewTask};
    use crate::{Ledger, DB_FILE_NAME};

    fn ledger() -> (tempfile::TempDir, Ledger) {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::open(&dir.path().join(DB_FILE_NAME)).unwrap();
        (dir, l)
    }

    fn project(l: &Ledger) -> String {
        l.write(|tx, _| {
            tx.execute(
                "INSERT INTO projects (id, name, created_at) VALUES ('p-1', 'Website', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        "p-1".into()
    }

    fn new(project: &str, task: &str, branch: &str, path: &str) -> NewWorkspace {
        NewWorkspace {
            project_id: project.into(),
            correlation_id: "wf-1".into(),
            root_task_id: Some(task.into()),
            parent_id: None,
            repository: "/repo".into(),
            subfolder: "apps/web".into(),
            path: path.into(),
            branch: branch.into(),
            base_ref: Some("main".into()),
            base_commit: "abc1234".into(),
        }
    }

    #[test]
    fn one_working_copy_per_objective_facts_and_removal_are_recorded() {
        let (_dir, l) = ledger();
        let p = project(&l);
        let task = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "Add a login page".into(),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        let w = l
            .create_workspace(
                &new(&p, &task.id, "plenipo/add-a-login-page-1a2b3c4d", "/ws/1"),
                &task.id,
                "guard",
            )
            .unwrap();
        assert_eq!(w.state, WorkspaceState::Active);
        assert_eq!(w.folder(), std::path::PathBuf::from("/ws/1/apps/web"));
        assert_eq!(l.objective_workspace(&p, "wf-1").unwrap().unwrap().id, w.id);
        // A second one for the same objective must name the first.
        let again = l.create_workspace(
            &new(&p, &task.id, "plenipo/add-a-login-page-1a2b3c4d-2", "/ws/2"),
            &task.id,
            "guard",
        );
        assert!(again.unwrap_err().to_string().contains("already has"));
        let side = l
            .create_workspace(
                &NewWorkspace {
                    parent_id: Some(w.id.clone()),
                    ..new(&p, &task.id, "plenipo/add-a-login-page-1a2b3c4d-2", "/ws/2")
                },
                &task.id,
                "guard",
            )
            .unwrap();
        assert_eq!(
            l.workflow_workspaces("wf-1")
                .unwrap()
                .iter()
                .map(|w| w.id.clone())
                .collect::<Vec<_>>(),
            [w.id.clone(), side.id.clone()]
        );
        // Branch names are Plenipo's own.
        for bad in ["main", "plenipo/../x", "plenipo/Upper", "plenipo/x/"] {
            assert!(l
                .create_workspace(&new(&p, &task.id, bad, "/ws/x"), &task.id, "guard")
                .is_err());
        }

        let facts = WorkspaceFacts {
            head: Some("def5678".into()),
            commits: vec![CommitInfo {
                hash: "def5678".into(),
                subject: "Add the login page".into(),
            }],
            files: vec![FileChange {
                path: "apps/web/login.html".into(),
                added: Some(20),
                removed: Some(0),
                committed: true,
            }],
            uncommitted: 0,
            pushed: false,
            checked_at: Some(1),
        };
        l.update_workspace_facts(&w.id, &facts, &task.id, "guard")
            .unwrap();
        // Checking again with nothing changed records nothing new.
        let later = WorkspaceFacts {
            checked_at: Some(2),
            ..facts.clone()
        };
        let w2 = l
            .update_workspace_facts(&w.id, &later, &task.id, "guard")
            .unwrap();
        assert_eq!(w2.facts, later);
        let types: Vec<String> = l
            .events_for_task(&task.id)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .filter(|t| t.starts_with("workspace."))
            .collect();
        assert_eq!(
            types,
            [
                "workspace.created",
                "workspace.created",
                "workspace.updated"
            ]
        );
        let removed = l.remove_workspace(&w.id, "owner").unwrap();
        assert_eq!(removed.state, WorkspaceState::Removed);
        assert!(removed.removed_at.is_some());
        assert_eq!(l.project_workspaces(&p, 10).unwrap().len(), 2);
        assert_eq!(
            l.last_task_event(&task.id, "workspace.removed")
                .unwrap()
                .unwrap()
                .payload["branch"],
            "plenipo/add-a-login-page-1a2b3c4d"
        );
    }
}
