//! What is live in this organization now, for the Free edition's limits (Phase 11A): they count
//! live positions, never history. A department or project that is archived or deleted does not
//! count, and a project that finished a hundred tasks is still one project.

use crate::error::Result;
use crate::Ledger;

impl Ledger {
    /// Departments that are neither archived nor deleted.
    pub fn live_departments(&self) -> Result<u32> {
        self.read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM departments WHERE archived_at IS NULL AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )?)
        })
    }

    /// Projects that are neither archived nor deleted. Before 1.10.0, archiving a project set
    /// only its status (no `archived_at`), so the status counts too.
    pub fn live_projects(&self) -> Result<u32> {
        self.read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM projects
                 WHERE status <> 'archived' AND archived_at IS NULL AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )?)
        })
    }

    /// The tasks of the workers on the job now: running, or waiting for the owner's approval.
    pub fn tasks_on_the_job(&self) -> Result<Vec<String>> {
        self.read(|c| {
            let mut stmt = c.prepare(
                "SELECT id FROM tasks WHERE state IN ('running', 'awaitingApproval') ORDER BY rowid",
            )?;
            let ids = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(ids)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::dto::{NewTask, TaskState};
    use crate::Ledger;

    #[test]
    fn only_live_departments_and_projects_count() {
        let l = Ledger::open_in_memory().unwrap();
        assert_eq!(l.live_departments().unwrap(), 0);
        assert_eq!(l.live_projects().unwrap(), 0);
        let d = |n| l.create_department(n, "", None, "test").unwrap();
        let (dev, old, gone) = (d("Development"), d("Old"), d("Gone"));
        l.archive_department(&old.id, "test").unwrap();
        l.delete_department(&gone.id, "test").unwrap();
        let p = |n| {
            l.create_project(n, None, None, Some(dev.id.as_str()), "test")
                .unwrap()
        };
        let (_site, done) = (p("Site"), p("Done"));
        l.archive_project(&done.id, "test").unwrap();
        assert_eq!(l.live_departments().unwrap(), 1);
        assert_eq!(l.live_projects().unwrap(), 1);
    }

    /// Review finding: before 1.10.0, archiving a project set only its status. An owner who
    /// archived one then could neither make a project on Free nor bring that one back.
    #[test]
    fn a_project_archived_before_1_10_does_not_count() {
        let l = Ledger::open_in_memory().unwrap();
        let dev = l
            .create_department("Development", "", None, "test")
            .unwrap();
        let old = l
            .create_project("Old", None, None, Some(dev.id.as_str()), "test")
            .unwrap();
        l.write(|c, _| {
            c.execute(
                "UPDATE projects SET status = 'archived' WHERE id = ?1",
                [&old.id],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(l.live_projects().unwrap(), 0);
    }

    #[test]
    fn workers_on_the_job_are_running_or_waiting_for_an_approval() {
        let l = Ledger::open_in_memory().unwrap();
        let task = |path: &[TaskState]| {
            let t = l
                .create_task(
                    NewTask {
                        objective: "work".into(),
                        requested_by: "owner".into(),
                        ..NewTask::default()
                    },
                    "test",
                )
                .unwrap();
            for s in path {
                l.transition_task(&t.id, *s, "test", None).unwrap();
            }
            t.id
        };
        let running = task(&[TaskState::Running]);
        let asking = task(&[TaskState::Running, TaskState::AwaitingApproval]);
        task(&[]);
        task(&[TaskState::Running, TaskState::Blocked]);
        task(&[TaskState::Running, TaskState::Succeeded]);
        assert_eq!(l.tasks_on_the_job().unwrap(), vec![running, asking]);
    }
}
