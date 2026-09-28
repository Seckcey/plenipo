//! The Workforce (Phase 17, ADR-045): agents the owner saved to hire again. Saving copies what an
//! archived agent is (title, role, specialty, AI settings, learning setting), what it did (its
//! experience), and the lessons it wrote that the owner keeps, then leaves a short record in its
//! place on the chart. Hiring it again (a new position with `from_workforce`) carries its
//! experience on, restores any of its lessons its role no longer has, and takes it out of the
//! Workforce.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use super::archive::tombstone_position;
use super::{all, experience, get_position, invalid, now, org_event, refuse_if_busy};
use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::rows::{json as parse_json, u64_of};
use crate::Ledger;

pub(crate) const SAVED_COLS: &str =
    "id, title, role_id, specialty_id, from_position, settings, experience, lessons, saved_at";

pub(crate) fn saved_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SavedAgent> {
    Ok(SavedAgent {
        id: r.get(0)?,
        title: r.get(1)?,
        role_id: r.get(2)?,
        specialty_id: r.get(3)?,
        from_position: r.get(4)?,
        settings: parse_json(r.get(5)?),
        experience: parse_json(r.get(6)?),
        lessons: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
        saved_at: u64_of(r.get(8)?),
    })
}

fn get(c: &Connection, id: &str) -> Result<Option<SavedAgent>> {
    Ok(c.query_row(
        &format!("SELECT {SAVED_COLS} FROM saved_agents WHERE id = ?1"),
        [id],
        saved_row,
    )
    .optional()?)
}

/// The project and department `p` worked in, by name (nearest first).
fn places(c: &Connection, p: &Position) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut at = Some(p.id.clone());
    let mut hops = 0;
    while let Some(id) = at {
        hops += 1;
        if hops > 64 {
            break;
        }
        let project: Option<String> = c
            .query_row(
                "SELECT name FROM projects WHERE coordinator_position_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(name) = project {
            if !out.contains(&name) {
                out.push(name);
            }
        }
        let department: Option<String> = c
            .query_row(
                "SELECT name FROM departments WHERE head_position_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(name) = department {
            if !out.contains(&name) {
                out.push(name);
            }
            break;
        }
        at = c
            .query_row(
                "SELECT reports_to FROM positions WHERE id = ?1",
                [&id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
    }
    Ok(out)
}

/// A number from `v`, 0 when absent.
fn num(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

/// Move archived position `p` to the Workforce, with `settings` (its AI settings and learning
/// setting, from the Workforce service). Its place on the chart becomes a short record.
pub(super) fn save(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    p: &Position,
    settings: &Value,
    actor: &str,
) -> Result<SavedAgent> {
    let counts = experience::counts_for(tx, &p.id)?;
    let before = &p.metadata["experience"];
    let mut lessons: Vec<String> =
        serde_json::from_value(before["lessons"].clone()).unwrap_or_default();
    for text in experience::kept_lessons(tx, &p.id)? {
        if !lessons.contains(&text) {
            lessons.push(text);
        }
    }
    let first = [before["firstWorked"].as_u64(), counts.first_task_at]
        .into_iter()
        .flatten()
        .min();
    let last = [before["lastWorked"].as_u64(), counts.last_task_at]
        .into_iter()
        .flatten()
        .max();
    let mut worked = places(tx, p)?;
    for place in before["places"].as_array().into_iter().flatten() {
        if let Some(name) = place.as_str() {
            if !worked.iter().any(|w| w == name) {
                worked.push(name.to_owned());
            }
        }
    }
    let experience = json!({
        "keptLessons": num(&before["keptLessons"]) + u64::from(counts.kept_lessons),
        "tasksDone": num(&before["tasksDone"]) + u64::from(counts.tasks_done),
        "firstWorked": first,
        "lastWorked": last,
        "places": worked,
    });
    let specialty = super::specialties::current(tx, p.specialty_id.as_deref())?;
    let id = uuid::Uuid::new_v4().to_string();
    let at = now();
    tx.execute(
        "INSERT INTO saved_agents (id, title, role_id, specialty_id, from_position, settings,
             experience, lessons, saved_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            p.title,
            p.role_id,
            specialty.as_ref().map(|s| &s.id),
            p.id,
            settings.to_string(),
            experience.to_string(),
            serde_json::to_string(&lessons).unwrap_or_else(|_| "[]".into()),
            at
        ],
    )?;
    tombstone_position(
        tx,
        p,
        &json!({ "by": actor, "at": at, "movedTo": "workforce", "savedId": id }),
    )?;
    org_event(
        out,
        tx,
        actor,
        "agent_saved",
        json!({
            "savedId": id,
            "positionId": p.id,
            "title": p.title,
            "roleId": p.role_id,
            "experience": experience,
        }),
    )?;
    get(tx, &id)?.ok_or_else(|| LedgerError::NotFound(format!("saved agent {id}")))
}

/// Position `position_id` (just created) hires saved agent `saved_id` again: its experience
/// carries on, its lessons its role no longer has come back as kept, and it leaves the Workforce.
pub(super) fn adopt(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    saved_id: &str,
    position_id: &str,
    role_id: &str,
    title: &str,
    actor: &str,
) -> Result<()> {
    let s =
        get(tx, saved_id)?.ok_or_else(|| invalid("that agent is no longer in your Workforce"))?;
    if s.role_id != role_id {
        return Err(invalid(format!(
            "{} keeps its role when hired again from your Workforce",
            s.title
        )));
    }
    let mut base = s.experience.clone();
    if !base.is_object() {
        base = json!({});
    }
    base["lessons"] = json!(s.lessons);
    base["savedId"] = json!(s.id);
    tx.execute(
        "UPDATE positions SET metadata = json_set(metadata, '$.experience', json(?2))
         WHERE id = ?1",
        params![position_id, base.to_string()],
    )?;
    let at = now();
    let mut restored = 0;
    for text in &s.lessons {
        // Only a lesson its role has never had: one the owner removed or discarded stays so.
        let known: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM lessons WHERE role_id = ?1 AND text = ?2)",
            params![role_id, text],
            |r| r.get(0),
        )?;
        if known {
            continue;
        }
        let lesson_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO lessons (id, role_id, task_id, position_id, worker, text, state,
                 from_web, created_at, decided_at, decided_by)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, 'kept', 0, ?6, ?6, ?7)",
            params![lesson_id, role_id, s.from_position, title, text, at, actor],
        )?;
        restored += 1;
    }
    tx.execute("DELETE FROM saved_agents WHERE id = ?1", [saved_id])?;
    org_event(
        out,
        tx,
        actor,
        "agent_hired_from_workforce",
        json!({
            "savedId": s.id,
            "positionId": position_id,
            "title": title,
            "roleId": role_id,
            // The saved row goes; its record stays here: what it came back with.
            "settings": s.settings,
            "experience": s.experience,
            "lessonsRestored": restored,
        }),
    )
}

impl Ledger {
    /// The agents in the Workforce, most recently saved first.
    pub fn saved_agents(&self) -> Result<Vec<SavedAgent>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {SAVED_COLS} FROM saved_agents ORDER BY saved_at DESC, rowid DESC"
                ),
                [],
                saved_row,
            )
        })
    }

    pub fn saved_agent(&self, id: &str) -> Result<Option<SavedAgent>> {
        self.read(|c| get(c, id))
    }

    /// Save an archived position to the Workforce on its own (not while deleting for good). A
    /// supervisor or manager is saved when its project or department is deleted for good.
    pub fn save_to_workforce(&self, save_agent: &SaveAgent, actor: &str) -> Result<SavedAgent> {
        self.write(|tx, out| {
            let p = get_position(tx, &save_agent.position_id)?;
            if p.is_deleted() {
                return Err(invalid(format!("{} is no longer on the chart", p.title)));
            }
            if p.state != PositionState::Archived {
                return Err(invalid(format!(
                    "{} is on the chart; archive it first, then save it to your Workforce",
                    p.title
                )));
            }
            let leads: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM departments WHERE head_position_id = ?1
                                  AND deleted_at IS NULL)
                     OR EXISTS (SELECT 1 FROM projects WHERE coordinator_position_id = ?1
                                  AND deleted_at IS NULL)",
                [&p.id],
                |r| r.get(0),
            )?;
            if leads {
                return Err(invalid(format!(
                    "{} leads a project or department; save it when you delete that for good",
                    p.title
                )));
            }
            let below: Vec<String> = all(
                tx,
                "SELECT title FROM positions WHERE reports_to = ?1 AND deleted_at IS NULL
                 ORDER BY title",
                [&p.id],
                |r| r.get(0),
            )?;
            if !below.is_empty() {
                return Err(invalid(format!(
                    "{} leads archived agents ({}); bring them back or delete them for good first",
                    p.title,
                    below.join(", ")
                )));
            }
            refuse_if_busy(tx, &p, "saving it to your Workforce")?;
            save(tx, out, &p, &save_agent.settings, actor)
        })
    }

    /// Delete an agent in the Workforce for good. The lessons its role keeps stay with the role.
    pub fn delete_saved_agent(&self, id: &str, actor: &str) -> Result<SavedAgent> {
        self.write(|tx, out| {
            let s =
                get(tx, id)?.ok_or_else(|| LedgerError::NotFound(format!("saved agent {id}")))?;
            tx.execute("DELETE FROM saved_agents WHERE id = ?1", [id])?;
            org_event(
                out,
                tx,
                actor,
                "saved_agent_deleted",
                json!({ "savedId": id, "title": s.title, "roleId": s.role_id }),
            )?;
            Ok(s)
        })
    }
}
