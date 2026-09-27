//! Lessons workers learn from their work (ADR-024). A worker ends a task with what would help
//! the next worker in its role; each lesson waits for the owner's Keep or Discard, unless the
//! owner lets that role learn on its own. Kept lessons go into the role's later workers'
//! instructions. Every change is a `lesson.*` event in the same transaction.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use crate::dto::{Lesson, LessonState, NewEvent, NewLessons};
use crate::error::{LedgerError, Result};
use crate::rows::{parse_enum, u64_of};
use crate::{events, Ledger};

/// Longest lesson, in characters (longer ones are cut, with "…").
pub const MAX_LESSON_CHARS: usize = 300;
/// Most lessons one task may add.
pub const MAX_LESSONS_PER_TASK: usize = 3;

const COLS: &str = "id, role_id, task_id, position_id, worker, text, state, from_web, created_at, \
    decided_at, decided_by";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Lesson> {
    Ok(Lesson {
        id: r.get(0)?,
        role_id: r.get(1)?,
        task_id: r.get(2)?,
        position_id: r.get(3)?,
        worker: r.get(4)?,
        text: r.get(5)?,
        state: parse_enum(6, r.get(6)?, LessonState::parse)?,
        from_web: r.get::<_, i64>(7)? != 0,
        created_at: u64_of(r.get(8)?),
        decided_at: r.get::<_, Option<i64>>(9)?.map(u64_of),
        decided_by: r.get(10)?,
    })
}

fn get(conn: &Connection, id: &str) -> Result<Option<Lesson>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM lessons WHERE id = ?1"),
            [id],
            row,
        )
        .optional()?)
}

fn require(conn: &Connection, id: &str) -> Result<Lesson> {
    get(conn, id)?.ok_or_else(|| LedgerError::NotFound(format!("lesson {id}")))
}

fn invalid(message: impl Into<String>) -> LedgerError {
    LedgerError::InvalidInput(message.into())
}

/// A lesson's text on one line: spaces collapsed, at most [`MAX_LESSON_CHARS`] characters.
/// `None` when nothing is left.
pub fn clean_lesson(text: &str) -> Option<String> {
    let line = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    if line.is_empty() {
        return None;
    }
    if line.chars().count() > MAX_LESSON_CHARS {
        let cut: String = line.chars().take(MAX_LESSON_CHARS - 1).collect();
        return Some(format!("{}…", cut.trim_end()));
    }
    Some(line)
}

fn lesson_event(lesson: &Lesson, actor: &str, event_type: &str) -> NewEvent {
    NewEvent {
        task_id: lesson.task_id.clone(),
        source: actor.into(),
        event_type: event_type.into(),
        payload: json!({
            "lessonId": lesson.id,
            "roleId": lesson.role_id,
            "worker": lesson.worker,
            "text": lesson.text,
            "state": lesson.state,
            "fromWeb": lesson.from_web,
        }),
        ..NewEvent::default()
    }
}

impl Ledger {
    /// Record what one task's worker learned: kept at once when `new.keep`, otherwise waiting
    /// for the owner. At most [`MAX_LESSONS_PER_TASK`]; blanks and lessons the role already has
    /// are left out. Each gets `lesson.added`.
    pub fn add_lessons(&self, new: &NewLessons, actor: &str) -> Result<Vec<Lesson>> {
        let state = if new.keep {
            LessonState::Kept
        } else {
            LessonState::Waiting
        };
        self.write(|tx, out| {
            let mut added = Vec::new();
            for text in new.texts.iter().filter_map(|t| clean_lesson(t)) {
                if added.len() == MAX_LESSONS_PER_TASK {
                    break;
                }
                let known: bool = tx.query_row(
                    "SELECT EXISTS (SELECT 1 FROM lessons WHERE role_id = ?1 AND text = ?2 \
                     AND state IN ('waiting', 'kept'))",
                    params![new.role_id, text],
                    |r| r.get(0),
                )?;
                if known {
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                let now = crate::now_ms() as i64;
                tx.execute(
                    "INSERT INTO lessons (id, role_id, task_id, position_id, worker, text, state, \
                     from_web, created_at, decided_at, decided_by) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        id,
                        new.role_id,
                        new.task_id,
                        new.position_id,
                        new.worker.chars().take(200).collect::<String>(),
                        text,
                        state.as_str(),
                        i64::from(new.from_web),
                        now,
                        new.keep.then_some(now),
                        new.keep.then_some(actor),
                    ],
                )
                .map_err(|e| match e {
                    rusqlite::Error::SqliteFailure(f, _)
                        if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        invalid("a lesson needs a role and a task that exist")
                    }
                    e => e.into(),
                })?;
                let lesson = require(tx, &id)?;
                out.push(events::insert(
                    tx,
                    lesson_event(&lesson, actor, "lesson.added"),
                )?);
                added.push(lesson);
            }
            Ok(added)
        })
    }

    /// The owner's answer to a waiting lesson: keep it (with the owner's wording when `text` is
    /// given) or discard it.
    pub fn decide_lesson(
        &self,
        id: &str,
        keep: bool,
        text: Option<&str>,
        actor: &str,
    ) -> Result<Lesson> {
        let text = match text {
            Some(t) => Some(clean_lesson(t).ok_or_else(|| invalid("a lesson cannot be empty"))?),
            None => None,
        };
        self.write(|tx, out| {
            let lesson = require(tx, id)?;
            if lesson.state != LessonState::Waiting {
                return Err(invalid("that lesson was already answered"));
            }
            let (state, event_type) = if keep {
                (LessonState::Kept, "lesson.kept")
            } else {
                (LessonState::Discarded, "lesson.discarded")
            };
            tx.execute(
                "UPDATE lessons SET state = ?2, text = COALESCE(?3, text), decided_at = ?4, \
                 decided_by = ?5 WHERE id = ?1",
                params![
                    id,
                    state.as_str(),
                    text.filter(|_| keep),
                    crate::now_ms() as i64,
                    actor
                ],
            )?;
            let lesson = require(tx, id)?;
            out.push(events::insert(
                tx,
                lesson_event(&lesson, actor, event_type),
            )?);
            Ok(lesson)
        })
    }

    /// The owner removes a kept lesson: the role's later workers no longer get it.
    pub fn remove_lesson(&self, id: &str, actor: &str) -> Result<Lesson> {
        self.write(|tx, out| {
            let lesson = require(tx, id)?;
            if lesson.state != LessonState::Kept {
                return Err(invalid("only a kept lesson can be removed"));
            }
            tx.execute(
                "UPDATE lessons SET state = 'removed', decided_at = ?2, decided_by = ?3 \
                 WHERE id = ?1",
                params![id, crate::now_ms() as i64, actor],
            )?;
            let lesson = require(tx, id)?;
            out.push(events::insert(
                tx,
                lesson_event(&lesson, actor, "lesson.removed"),
            )?);
            Ok(lesson)
        })
    }

    pub fn lesson(&self, id: &str) -> Result<Option<Lesson>> {
        self.read(|c| get(c, id))
    }

    /// Lessons in `state`, newest first; for one role when `role_id` is given.
    pub fn lessons(
        &self,
        state: LessonState,
        role_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Lesson>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {COLS} FROM lessons WHERE state = ?1 AND (?2 IS NULL OR role_id = ?2) \
                 ORDER BY created_at DESC, rowid DESC LIMIT ?3"
            ))?;
            let rows = stmt
                .query_map(params![state.as_str(), role_id, i64::from(limit)], row)?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// The task, or any task handed on from it, used Plenipo's browser, saw the screen, or ran
    /// commands on a server (Phase 11). Its lessons may carry what a website, another program, or
    /// a server said, so they always wait for the owner.
    pub fn task_used_web_screen_or_servers(&self, task_id: &str) -> Result<bool> {
        self.read(|c| {
            Ok(c.query_row(
                "WITH RECURSIVE tree(id) AS ( \
                   SELECT ?1 UNION ALL \
                   SELECT t.id FROM tasks t JOIN tree ON t.parent_task_id = tree.id) \
                 SELECT EXISTS (SELECT 1 FROM events JOIN tree ON events.task_id = tree.id \
                   WHERE (event_type = 'capability.used' \
                     AND (json_extract(payload, '$.capability') LIKE 'browser.%' \
                       OR json_extract(payload, '$.capability') LIKE 'computer.%' \
                       OR json_extract(payload, '$.capability') LIKE 'ssh.%')) \
                   OR event_type = 'control.started' \
                   OR event_type LIKE 'ssh.%')",
                [task_id],
                |r| r.get(0),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{NewTask, RoleType};

    fn ledger() -> (tempfile::TempDir, Ledger) {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::open(&dir.path().join("l.db")).unwrap();
        (dir, l)
    }

    fn role(l: &Ledger) -> String {
        l.create_role(
            "Scout",
            "Finds suppliers.",
            RoleType::Worker,
            false,
            &json!({}),
            "owner",
        )
        .unwrap()
        .id
    }

    fn task(l: &Ledger) -> String {
        l.create_task(
            NewTask {
                objective: "Find suppliers".into(),
                requested_by: "owner".into(),
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
        .id
    }

    fn new(role_id: &str, task_id: &str, texts: &[&str], keep: bool) -> NewLessons {
        NewLessons {
            role_id: role_id.into(),
            task_id: task_id.into(),
            position_id: None,
            worker: "Scout".into(),
            texts: texts.iter().map(|t| (*t).to_owned()).collect(),
            from_web: false,
            keep,
        }
    }

    #[test]
    fn lessons_wait_for_the_owner_and_can_be_kept_edited_or_removed() {
        let (_d, l) = ledger();
        let (r, t) = (role(&l), task(&l));
        let long = "x".repeat(500);
        let added = l
            .add_lessons(
                &new(
                    &r,
                    &t,
                    &[
                        "  Check the Orders page\n first. ",
                        "",
                        &long,
                        "Three",
                        "Four",
                    ],
                    false,
                ),
                "agent:claude-code",
            )
            .unwrap();
        assert_eq!(
            added.len(),
            MAX_LESSONS_PER_TASK,
            "blanks dropped, three at most"
        );
        assert_eq!(added[0].text, "Check the Orders page first.");
        assert_eq!(added[1].text.chars().count(), MAX_LESSON_CHARS);
        assert!(added[1].text.ends_with('…'));
        assert!(added.iter().all(|a| a.state == LessonState::Waiting));
        // The same lesson again is left out.
        assert!(l
            .add_lessons(&new(&r, &t, &["Check the Orders page first."], false), "a")
            .unwrap()
            .is_empty());
        let kept = l
            .decide_lesson(&added[0].id, true, Some("Open Orders first."), "owner")
            .unwrap();
        assert_eq!(
            (kept.state, kept.text.as_str()),
            (LessonState::Kept, "Open Orders first.")
        );
        assert!(l.decide_lesson(&added[0].id, false, None, "owner").is_err());
        let gone = l.decide_lesson(&added[1].id, false, None, "owner").unwrap();
        assert_eq!(gone.state, LessonState::Discarded);
        assert!(l.remove_lesson(&added[1].id, "owner").is_err(), "not kept");
        assert_eq!(l.lessons(LessonState::Kept, Some(&r), 20).unwrap().len(), 1);
        assert_eq!(
            l.remove_lesson(&kept.id, "owner").unwrap().state,
            LessonState::Removed
        );
        assert!(l
            .lessons(LessonState::Kept, Some(&r), 20)
            .unwrap()
            .is_empty());
        assert_eq!(l.lessons(LessonState::Waiting, None, 20).unwrap().len(), 1);
        let kinds: Vec<String> = l
            .events_for_task(&t)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .filter(|k| k.starts_with("lesson."))
            .collect();
        assert_eq!(
            kinds,
            [
                "lesson.added",
                "lesson.added",
                "lesson.added",
                "lesson.kept",
                "lesson.discarded",
                "lesson.removed"
            ]
        );
    }

    #[test]
    fn a_role_that_learns_on_its_own_keeps_them_at_once() {
        let (_d, l) = ledger();
        let (r, t) = (role(&l), task(&l));
        let added = l
            .add_lessons(&new(&r, &t, &["Ask for the order number."], true), "a")
            .unwrap();
        assert_eq!(added[0].state, LessonState::Kept);
        assert!(added[0].decided_at.is_some());
        assert!(!l.task_used_web_screen_or_servers(&t).unwrap());
        assert!(l
            .add_lessons(&new("no-such-role", &t, &["x"], false), "a")
            .is_err());
    }

    #[test]
    fn web_screen_or_server_use_anywhere_below_a_task_counts() {
        let (_d, l) = ledger();
        let (parent, other) = (task(&l), task(&l));
        let child = l
            .create_task(
                NewTask {
                    objective: "Look it up".into(),
                    requested_by: "owner".into(),
                    parent_task_id: Some(parent.clone()),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap()
            .id;
        let used = |task: &str, capability: &str| {
            l.append_event(crate::dto::NewEvent {
                task_id: Some(task.to_owned()),
                execution_id: None,
                source: "guard".into(),
                destination: None,
                event_type: "capability.used".into(),
                payload: serde_json::json!({ "capability": capability }),
            })
            .unwrap();
        };
        used(&parent, "files.read");
        assert!(!l.task_used_web_screen_or_servers(&parent).unwrap());
        // A website read by a task handed on from it counts for the task that handed it on.
        used(&child, "browser.navigate");
        assert!(l.task_used_web_screen_or_servers(&parent).unwrap());
        assert!(l.task_used_web_screen_or_servers(&child).unwrap());
        // So does seeing the screen.
        used(&other, "computer.observe");
        assert!(l.task_used_web_screen_or_servers(&other).unwrap());
        // And running commands on a server (Phase 11): through the permission, or any of the
        // server events (a connection alone brings in what the server says).
        let (ssh, connected) = (task(&l), task(&l));
        used(&ssh, "ssh.connect");
        assert!(l.task_used_web_screen_or_servers(&ssh).unwrap());
        l.append_event(crate::dto::NewEvent {
            task_id: Some(connected.clone()),
            execution_id: None,
            source: "capabilities".into(),
            destination: None,
            event_type: "ssh.command_finished".into(),
            payload: serde_json::json!({ "server": "Shop" }),
        })
        .unwrap();
        assert!(l.task_used_web_screen_or_servers(&connected).unwrap());
        assert!(!l.task_used_web_screen_or_servers(&task(&l)).unwrap());
    }
}
