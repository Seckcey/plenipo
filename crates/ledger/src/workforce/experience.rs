//! What each agent has done, for its experience (Phase 17, ADR-045): the lessons it wrote that
//! the owner still keeps, and its tasks that finished. Worked out from the records each time, so
//! nothing is stored for it and it cannot drift. An agent hired from the Workforce also carries
//! what it had done before (`metadata.experience` on its position).

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::dto::ExperienceCounts;
use crate::error::Result;
use crate::rows::opt_u64;
use crate::Ledger;

/// Each position's counts, for every position that wrote a kept lesson or finished a task.
pub(crate) fn counts(c: &Connection) -> Result<HashMap<String, ExperienceCounts>> {
    let mut out: HashMap<String, ExperienceCounts> = HashMap::new();
    let mut stmt = c.prepare(
        "SELECT position_id, COUNT(*) FROM lessons
         WHERE state = 'kept' AND position_id IS NOT NULL GROUP BY position_id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?;
    for row in rows {
        let (id, n) = row?;
        out.entry(id).or_default().kept_lessons = n;
    }
    let mut stmt = c.prepare(
        "SELECT json_extract(metadata, '$.workforce.positionId') AS position, COUNT(*),
                MIN(created_at), MAX(COALESCE(completed_at, updated_at))
         FROM tasks
         WHERE json_extract(metadata, '$.workforce.positionId') IS NOT NULL
           AND state = 'succeeded'
         GROUP BY position",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, u32>(1)?,
            opt_u64(r.get(2)?),
            opt_u64(r.get(3)?),
        ))
    })?;
    for row in rows {
        let (id, n, first, last) = row?;
        let e = out.entry(id).or_default();
        e.tasks_done = n;
        e.first_task_at = first;
        e.last_task_at = last;
    }
    Ok(out)
}

/// One position's counts.
pub(crate) fn counts_for(c: &Connection, position_id: &str) -> Result<ExperienceCounts> {
    let kept_lessons: u32 = c.query_row(
        "SELECT COUNT(*) FROM lessons WHERE state = 'kept' AND position_id = ?1",
        [position_id],
        |r| r.get(0),
    )?;
    let (tasks_done, first_task_at, last_task_at) = c.query_row(
        "SELECT COUNT(*), MIN(created_at), MAX(COALESCE(completed_at, updated_at)) FROM tasks
         WHERE json_extract(metadata, '$.workforce.positionId') = ?1 AND state = 'succeeded'",
        [position_id],
        |r| Ok((r.get::<_, u32>(0)?, opt_u64(r.get(1)?), opt_u64(r.get(2)?))),
    )?;
    Ok(ExperienceCounts {
        kept_lessons,
        tasks_done,
        first_task_at,
        last_task_at,
    })
}

/// The lessons `position_id` wrote that are kept, oldest first: each text, and whether the owner
/// kept it (a lesson its role kept on its own was never reviewed; ADR-050).
pub(crate) fn kept_lessons(c: &Connection, position_id: &str) -> Result<Vec<(String, bool)>> {
    super::all(
        c,
        "SELECT text, COALESCE(decided_by = 'owner', 0) FROM lessons
         WHERE position_id = ?1 AND state = 'kept'
         ORDER BY created_at, rowid",
        params![position_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

impl Ledger {
    /// Each position's counts (ADR-045), from the records.
    pub fn experience_counts(&self) -> Result<HashMap<String, ExperienceCounts>> {
        self.read(counts)
    }
}
