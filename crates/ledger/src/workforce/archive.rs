//! Archive, bring back, and delete for good (Phase 17, ADR-043, ADR-045).
//!
//! - **Archive:** a department takes everything in it along (its projects, each with its team,
//!   and every position under its manager); each archived item notes what it was archived with.
//! - **Bring back:** an item comes back as it was, with what was archived with it. A full-time
//!   position that had an agent gets a new one; the reviewer, QA, and security assignments that
//!   archiving ended come back when both sides are on the chart again.
//! - **Delete for good:** only archived items, never while anything has unfinished work. The row
//!   stays as a short record (ID, name, role, dates, "deleted by the owner") so older history still
//!   names it; its settings are cleared. A project or department takes along everything archived
//!   in it. Agents the owner chose to keep move to the Workforce instead (`saved.rs`).
//!
//! Each operation is one transaction; nothing on disk is touched.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use super::{
    all, archive, archived_with, get_position, hire, insert_oversight, invalid, now, org_event,
    position_row, refuse_if_busy, saved, specialties, unique, Org, POSITION_COLS,
};
use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::org::{dept_row, project_row, DEPT_COLS, PROJECT_COLS};
use crate::Ledger;

/// Runtime ID → its AI company (provider), recorded with a new agent.
pub type Providers = HashMap<String, String>;

pub(super) fn get_department(c: &Connection, id: &str) -> Result<Department> {
    c.query_row(
        &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
        [id],
        dept_row,
    )
    .optional()?
    .ok_or_else(|| LedgerError::NotFound(format!("department {id}")))
}

pub(super) fn get_project(c: &Connection, id: &str) -> Result<Project> {
    c.query_row(
        &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
        [id],
        project_row,
    )
    .optional()?
    .ok_or_else(|| LedgerError::NotFound(format!("project {id}")))
}

/// Every position, whatever its state, by ID.
fn every_position(c: &Connection) -> Result<HashMap<String, Position>> {
    Ok(all(
        c,
        &format!("SELECT {POSITION_COLS} FROM positions"),
        [],
        position_row,
    )?
    .into_iter()
    .map(|p| (p.id.clone(), p))
    .collect())
}

/// `root` (when archived) and every archived, not deleted position below it, parents first.
fn archived_below(every: &HashMap<String, Position>, root: &str) -> Vec<String> {
    let usable = |p: &Position| p.state == PositionState::Archived && !p.is_deleted();
    let mut out: Vec<String> = every
        .get(root)
        .filter(|p| usable(p))
        .map(|p| vec![p.id.clone()])
        .unwrap_or_default();
    let mut i = 0;
    while i < out.len() {
        let lead = out[i].clone();
        let mut children: Vec<&Position> = every
            .values()
            .filter(|p| p.reports_to.as_deref() == Some(lead.as_str()) && usable(p))
            .collect();
        children.sort_by_key(|p| (p.sort_key, p.created_at));
        out.extend(children.into_iter().map(|p| p.id.clone()));
        i += 1;
    }
    out
}

/// How many leads are above `id` (0: it reports to the owner).
fn depth(every: &HashMap<String, Position>, id: &str) -> usize {
    let mut n = 0;
    let mut seen = HashSet::new();
    let mut at = every.get(id).and_then(|p| p.reports_to.as_deref());
    while let Some(lead) = at {
        if !seen.insert(lead) {
            break;
        }
        n += 1;
        at = every.get(lead).and_then(|p| p.reports_to.as_deref());
    }
    n
}

/// Archived, not deleted positions whose archive note says they went with `id` (a project or a
/// department).
fn archived_with_item(c: &Connection, id: &str) -> Result<Vec<Position>> {
    all(
        c,
        &format!(
            "SELECT {POSITION_COLS} FROM positions
             WHERE state = 'archived' AND deleted_at IS NULL
               AND json_extract(metadata, '$.archive.with.id') = ?1"
        ),
        [id],
        position_row,
    )
}

/// Positions a project archived before 1.10.0 took along: they have no archive note, so the
/// project's own event lists them.
fn legacy_team(c: &Connection, project: &Project) -> Result<Vec<Position>> {
    let listed: Option<String> = c
        .query_row(
            "SELECT payload FROM events
             WHERE event_type = 'org.project_archived' AND json_extract(payload, '$.id') = ?1
             ORDER BY seq DESC LIMIT 1",
            [&project.id],
            |r| r.get(0),
        )
        .optional()?;
    let ids: Vec<String> = listed
        .and_then(|p| serde_json::from_str::<Value>(&p).ok())
        .and_then(|v| serde_json::from_value(v["positions"].clone()).ok())
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in ids {
        if let Ok(p) = get_position(c, &id) {
            if p.state == PositionState::Archived
                && !p.is_deleted()
                && p.metadata.get("archive").is_none()
            {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// Unfinished tasks that belong to `project`.
fn project_unfinished(c: &Connection, project_id: &str) -> Result<u32> {
    Ok(c.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE project_id = ?1 AND state NOT IN ('succeeded', 'failed', 'cancelled')",
        [project_id],
        |r| r.get(0),
    )?)
}

fn refuse_if_project_busy(c: &Connection, project: &Project, doing: &str) -> Result<()> {
    match project_unfinished(c, &project.id)? {
        0 => Ok(()),
        n => Err(invalid(format!(
            "{} has {n} unfinished task{}; wait for {} or cancel {} before {doing}",
            project.name,
            if n == 1 { "" } else { "s" },
            if n == 1 { "it" } else { "them" },
            if n == 1 { "it" } else { "them" },
        ))),
    }
}

/// A name no other row of `table` has: "Website (deleted)", then "Website (deleted 2)", …
fn deleted_name(c: &Connection, table: &str, name: &str) -> Result<String> {
    let base: String = name.chars().take(180).collect();
    for n in 1..1000 {
        let candidate = if n == 1 {
            format!("{base} (deleted)")
        } else {
            format!("{base} (deleted {n})")
        };
        let taken: bool = c.query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM {table} WHERE lower(name) = lower(?1))"),
            [&candidate],
            |r| r.get(0),
        )?;
        if !taken {
            return Ok(candidate);
        }
    }
    Err(invalid("too many deleted items share this name"))
}

/// Leave only the short record of archived position `p`: its ID, title, role, and dates stay;
/// its AI tool, model, specialty, lead, and notes go. `note` says why ("deleted by the owner",
/// or moved to the Workforce).
pub(super) fn tombstone_position(c: &Connection, p: &Position, note: &Value) -> Result<()> {
    let at = now();
    c.execute(
        "UPDATE positions SET deleted_at = ?2, updated_at = ?2, runtime_id = ?3, model = NULL,
             specialty_id = NULL, reports_to = NULL, metadata = ?4
         WHERE id = ?1",
        params![
            p.id,
            at,
            super::AUTOMATIC,
            json!({ "deleted": note }).to_string()
        ],
    )?;
    Ok(())
}

/// Every position, project, and department that deleting one item for good takes along.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeletionPlan {
    /// Parents first.
    pub positions: Vec<Position>,
    pub projects: Vec<Project>,
    pub departments: Vec<Department>,
}

/// A deletion goes no further than what it names: a position in it that runs another department,
/// or supervises another project, that is not being deleted too stops it, with what to do.
fn refuse_other_leads(
    c: &Connection,
    every: &HashMap<String, Position>,
    ids: &[String],
    plan: &DeletionPlan,
) -> Result<()> {
    let leads = |sql: &str| -> Result<HashMap<String, (String, String)>> {
        Ok(all(c, sql, [], |r| {
            Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?)))
        })?
        .into_iter()
        .collect())
    };
    let heads = leads(
        "SELECT head_position_id, id, name FROM departments
         WHERE deleted_at IS NULL AND head_position_id IS NOT NULL",
    )?;
    let supervisors = leads(
        "SELECT coordinator_position_id, id, name FROM projects
         WHERE deleted_at IS NULL AND coordinator_position_id IS NOT NULL",
    )?;
    for pid in ids {
        let title = every.get(pid).map_or("An agent", |p| p.title.as_str());
        if let Some((dept, name)) = heads.get(pid) {
            if !plan.departments.iter().any(|d| &d.id == dept) {
                return Err(invalid(format!(
                    "{title} runs the {name} department, which reports to someone being deleted; \
                     delete {name} for good first, or bring it back and move {title}"
                )));
            }
        }
        if let Some((project, name)) = supervisors.get(pid) {
            if !plan.projects.iter().any(|p| &p.id == project) {
                return Err(invalid(format!(
                    "{title} supervises the {name} project, which reports to someone being \
                     deleted; delete {name} for good first, or bring it back and move {title}"
                )));
            }
        }
    }
    Ok(())
}

/// A department or project comes back only with its lead on the chart (a safety net: deleting
/// for good never takes a lead without its department or project).
fn lead_is_back(c: &Connection, lead: Option<&str>, name: &str, what: &str) -> Result<()> {
    if let Some(id) = lead {
        let p = get_position(c, id)?;
        if p.state != PositionState::Active {
            let gone = if p.is_deleted() {
                "was deleted for good"
            } else {
                "is still archived"
            };
            return Err(invalid(format!(
                "{name} cannot come back: its {what}, {}, {gone}",
                p.title
            )));
        }
    }
    Ok(())
}

/// What `kind` (`position`, `project`, or `department`) `id` would take along when deleted for
/// good, or why it cannot be deleted.
fn plan(c: &Connection, kind: &str, id: &str) -> Result<DeletionPlan> {
    let every = every_position(c)?;
    let mut plan = DeletionPlan::default();
    let mut ids: Vec<String> = Vec::new();
    match kind {
        "position" => {
            let p = every
                .get(id)
                .ok_or_else(|| LedgerError::NotFound(format!("position {id}")))?;
            if p.is_deleted() {
                return Err(invalid(format!("{} was already deleted for good", p.title)));
            }
            if p.state != PositionState::Archived {
                return Err(invalid(format!(
                    "{} is on the chart; archive it first, then delete it for good",
                    p.title
                )));
            }
            let head_of: Option<String> = c
                .query_row(
                    "SELECT name FROM departments WHERE head_position_id = ?1 AND deleted_at IS NULL",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(d) = head_of {
                return Err(invalid(format!(
                    "{} is the manager of the {d} department; delete the department for good \
                     instead",
                    p.title
                )));
            }
            let lead_of: Option<String> = c
                .query_row(
                    "SELECT name FROM projects WHERE coordinator_position_id = ?1 AND deleted_at IS NULL",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(pr) = lead_of {
                return Err(invalid(format!(
                    "{} is the supervisor of the {pr} project; delete the project for good instead",
                    p.title
                )));
            }
            ids = archived_below(&every, id);
        }
        "project" => {
            let pr = get_project(c, id)?;
            if pr.deleted_at.is_some() {
                return Err(invalid(format!("{} was already deleted for good", pr.name)));
            }
            if pr.status != "archived" {
                return Err(invalid(format!(
                    "{} is active; archive it first, then delete it for good",
                    pr.name
                )));
            }
            if let Some(lead) = &pr.coordinator_position_id {
                ids = archived_below(&every, lead);
            }
            plan.projects.push(pr);
        }
        "department" => {
            let d = get_department(c, id)?;
            if d.deleted_at.is_some() {
                return Err(invalid(format!("{} was already deleted for good", d.name)));
            }
            if d.archived_at.is_none() {
                return Err(invalid(format!(
                    "{} is not archived; archive it first, then delete it for good",
                    d.name
                )));
            }
            let projects = all(
                c,
                &format!(
                    "SELECT {PROJECT_COLS} FROM projects
                     WHERE department_id = ?1 AND deleted_at IS NULL ORDER BY name"
                ),
                [id],
                project_row,
            )?;
            if let Some(active) = projects.iter().find(|p| p.status != "archived") {
                return Err(invalid(format!(
                    "{} is still active in {}; archive it first",
                    active.name, d.name
                )));
            }
            if let Some(head) = &d.head_position_id {
                ids = archived_below(&every, head);
            }
            for pr in &projects {
                if let Some(lead) = &pr.coordinator_position_id {
                    for pid in archived_below(&every, lead) {
                        if !ids.contains(&pid) {
                            ids.push(pid);
                        }
                    }
                }
            }
            plan.projects = projects;
            plan.departments.push(d);
        }
        other => {
            return Err(invalid(format!(
                "there is nothing called a {other} to delete"
            )))
        }
    }
    refuse_other_leads(c, &every, &ids, &plan)?;
    ids.sort_by_key(|pid| depth(&every, pid));
    plan.positions = ids
        .iter()
        .filter_map(|pid| every.get(pid).cloned())
        .collect();
    for p in &plan.positions {
        refuse_if_busy(c, p, "deleting it for good")?;
    }
    for pr in &plan.projects {
        refuse_if_project_busy(c, pr, "deleting it for good")?;
    }
    Ok(plan)
}

/// Delete `plan` for good, moving the positions in `save` to the Workforce instead.
fn delete_plan(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    plan: &DeletionPlan,
    save: &[SaveAgent],
    actor: &str,
) -> Result<Deleted> {
    for s in save {
        if !plan.positions.iter().any(|p| p.id == s.position_id) {
            return Err(invalid(
                "an agent chosen for your Workforce is not part of what is being deleted",
            ));
        }
    }
    let at = now();
    let mut deleted = Deleted::default();
    let role_name = |role_id: &str| -> Result<Option<String>> {
        Ok(tx
            .query_row("SELECT name FROM roles WHERE id = ?1", [role_id], |r| {
                r.get(0)
            })
            .optional()?)
    };
    // Positions first: the Workforce reads where they worked from their project and department.
    for p in plan.positions.iter().rev() {
        if let Some(s) = save.iter().find(|s| s.position_id == p.id) {
            let saved = saved::save(tx, out, p, &s.settings, actor)?;
            deleted.saved.push((p.id.clone(), saved.id));
            continue;
        }
        let note = json!({ "by": actor, "at": at });
        tombstone_position(tx, p, &note)?;
        org_event(
            out,
            tx,
            actor,
            "position_deleted",
            json!({
                "positionId": p.id,
                "title": p.title,
                "roleId": p.role_id,
                "role": role_name(&p.role_id)?,
                "createdAt": p.created_at,
                "archivedAt": p.archived_at,
                "deletedAt": at,
            }),
        )?;
        deleted.positions.push(p.id.clone());
    }
    for pr in &plan.projects {
        let name = deleted_name(tx, "projects", &pr.name)?;
        let note = json!({ "deleted": { "by": actor, "at": at, "name": pr.name } });
        tx.execute(
            "UPDATE projects SET name = ?2, description = '', local_path = NULL,
                 repository_url = NULL, allowed_runtimes = '[]', capability_profile = NULL,
                 coordinator_position_id = NULL, metadata = ?3, deleted_at = ?4,
                 archived_at = COALESCE(archived_at, ?4)
             WHERE id = ?1",
            params![pr.id, name, note.to_string(), at],
        )?;
        org_event(
            out,
            tx,
            actor,
            "project_deleted",
            json!({
                "id": pr.id,
                "name": pr.name,
                "shortName": name,
                "createdAt": pr.created_at,
                "archivedAt": pr.archived_at,
                "deletedAt": at,
            }),
        )?;
        deleted.projects.push(pr.id.clone());
    }
    for d in &plan.departments {
        let name = deleted_name(tx, "departments", &d.name)?;
        let note = json!({ "deleted": { "by": actor, "at": at, "name": d.name } });
        tx.execute(
            "UPDATE departments SET name = ?2, description = '', manager_role_id = NULL,
                 head_position_id = NULL, status = 'inactive', metadata = ?3, deleted_at = ?4
             WHERE id = ?1",
            params![d.id, name, note.to_string(), at],
        )?;
        org_event(
            out,
            tx,
            actor,
            "department_deleted",
            json!({
                "id": d.id,
                "name": d.name,
                "shortName": name,
                "forGood": true,
                "projects": plan.projects.iter().map(|p| &p.name).collect::<Vec<_>>(),
                "createdAt": d.created_at,
                "archivedAt": d.archived_at,
                "deletedAt": at,
            }),
        )?;
        deleted.departments.push(d.id.clone());
    }
    Ok(deleted)
}

/// Put archived `p` back on the chart as it was (checked against the chart as it is now).
fn restore(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    p: &Position,
    providers: &Providers,
    actor: &str,
) -> Result<Position> {
    let org = Org::load(tx)?;
    let role = org.role_of(p)?.clone();
    if let Some(lead) = p.reports_to.as_deref() {
        if !org.positions.contains_key(lead) {
            let title = get_position(tx, lead).map_or_else(|_| "its lead".to_owned(), |l| l.title);
            return Err(invalid(format!(
                "{} reports to {title}, which is not on the chart; bring {title} back first",
                p.title
            )));
        }
    }
    org.check_title(&p.title, p.reports_to.as_deref(), Some(&p.id))?;
    let project = org
        .coordinators
        .get(&p.id)
        .or_else(|| p.reports_to.as_deref().and_then(|l| org.project_of(l)));
    Org::check_runtime(project, p.runtime_id.as_deref(), &p.title)?;
    let specialty = specialties::current(tx, p.specialty_id.as_deref())?;
    let note = p.metadata.get("archive").cloned().unwrap_or(Value::Null);
    tx.execute(
        "UPDATE positions SET state = 'active', archived_at = NULL, updated_at = ?2,
             specialty_id = ?3, metadata = json_remove(metadata, '$.archive')
         WHERE id = ?1",
        params![p.id, now(), specialty.as_ref().map(|s| &s.id)],
    )
    .map_err(|e| unique(e, "that team already has a member with this title"))?;
    let restored = get_position(tx, &p.id)?;
    // A full-time position gets a new agent if it had one (positions archived before 1.10.0
    // did not record it; a full-time position normally has one).
    let staffed = note["staffed"].as_bool().unwrap_or(true);
    let agent = if role.persistent && staffed {
        let provider = restored
            .runtime_id
            .as_deref()
            .and_then(|r| providers.get(r))
            .map(String::as_str);
        Some(hire(
            tx,
            out,
            &restored,
            provider,
            project.map(|pr| pr.id.as_str()),
            actor,
        )?)
    } else {
        None
    };
    org_event(
        out,
        tx,
        actor,
        "position_restored",
        json!({
            "positionId": p.id,
            "title": p.title,
            "agentId": agent.as_ref().map(|a| &a.id),
            "specialtyDropped": p.specialty_id.is_some() && specialty.is_none(),
        }),
    )?;
    Ok(restored)
}

/// Bring back the reviewer, QA, and security assignments archiving ended for `restored`, when
/// both sides are on the chart again. Returns the ones that could not come back, and why.
fn restore_oversight(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    restored: &[Position],
    actor: &str,
) -> Result<Vec<Value>> {
    let mut wanted: Vec<(OversightKind, String, String)> = Vec::new();
    for p in restored {
        let list = p.metadata["archive"]["oversight"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for o in list {
            let (Some(kind), Some(overseer), Some(target)) = (
                o["kind"].as_str().and_then(OversightKind::parse),
                o["overseerId"].as_str(),
                o["targetId"].as_str(),
            ) else {
                continue;
            };
            let item = (kind, overseer.to_owned(), target.to_owned());
            if !wanted.contains(&item) {
                wanted.push(item);
            }
        }
    }
    let mut skipped = Vec::new();
    for (kind, overseer, target) in wanted {
        let org = Org::load(tx)?;
        if !org.positions.contains_key(&overseer) || !org.positions.contains_key(&target) {
            continue;
        }
        if org
            .oversight
            .iter()
            .any(|o| o.kind == kind && o.overseer_id == overseer && o.target_id == target)
        {
            continue;
        }
        if let Err(e) = insert_oversight(tx, out, &org, kind, &overseer, &target, actor) {
            skipped.push(json!({
                "kind": kind,
                "overseerId": overseer,
                "targetId": target,
                "why": e.to_string(),
            }));
        }
    }
    Ok(skipped)
}

/// Restore `positions` parents first, then their assignments.
fn restore_all(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    positions: Vec<Position>,
    providers: &Providers,
    actor: &str,
) -> Result<(Vec<String>, Vec<Value>)> {
    let every = every_position(tx)?;
    let mut positions = positions;
    positions.sort_by_key(|p| depth(&every, &p.id));
    let mut done = Vec::new();
    for p in &positions {
        restore(tx, out, p, providers, actor)?;
        done.push(p.clone());
    }
    let skipped = restore_oversight(tx, out, &done, actor)?;
    Ok((done.into_iter().map(|p| p.id).collect(), skipped))
}

impl Ledger {
    /// Archive a department with everything in it (ADR-043): its projects, each with its team,
    /// and every position under its manager, once none of it has unfinished work.
    pub fn archive_department(&self, id: &str, actor: &str) -> Result<Department> {
        self.write(|tx, out| {
            let d = get_department(tx, id)?;
            if d.deleted_at.is_some() {
                return Err(invalid(format!("{} was deleted for good", d.name)));
            }
            if d.archived_at.is_some() {
                return Ok(d);
            }
            let org = Org::load(tx)?;
            let projects = all(
                tx,
                &format!(
                    "SELECT {PROJECT_COLS} FROM projects
                     WHERE department_id = ?1 AND status = 'active' ORDER BY name"
                ),
                [id],
                project_row,
            )?;
            let team: Vec<String> = d
                .head_position_id
                .as_deref()
                .filter(|h| org.positions.contains_key(*h))
                .map(|h| org.subtree(h))
                .unwrap_or_default();
            // Everything in the department goes with it, and nothing of another: a manager of
            // another department, or a supervisor of another department's project, who reports to
            // someone in this one must be moved first (ADR-043 §7: refused, with what to do).
            for pid in &team {
                let Some(p) = org.positions.get(pid) else {
                    continue;
                };
                if let Some(other) = org
                    .heads
                    .get(pid)
                    .filter(|o| o.id != d.id && o.deleted_at.is_none())
                {
                    return Err(invalid(format!(
                        "{} runs the {} department and reports to someone in {}; move {} to report \
                         to you or another VP first, or archive {} on its own",
                        p.title, other.name, d.name, p.title, other.name
                    )));
                }
                if let Some(pr) = org.coordinators.get(pid).filter(|pr| {
                    pr.deleted_at.is_none() && pr.department_id.as_deref() != Some(d.id.as_str())
                }) {
                    return Err(invalid(format!(
                        "{} supervises the {} project, which is not in {}; move {} first, or \
                         archive {} on its own",
                        p.title, pr.name, d.name, p.title, pr.name
                    )));
                }
            }
            for pid in &team {
                if let Some(p) = org.positions.get(pid) {
                    refuse_if_busy(tx, p, "archiving the department")?;
                }
            }
            for pr in &projects {
                refuse_if_project_busy(tx, pr, "archiving the department")?;
            }
            let with_department = archived_with("department", &d.id, &d.name);
            let mut done: HashSet<String> = HashSet::new();
            let at = now();
            for pr in &projects {
                let with_project = archived_with("project", &pr.id, &pr.name);
                let project_team: Vec<String> = pr
                    .coordinator_position_id
                    .as_deref()
                    .filter(|c| org.positions.contains_key(*c))
                    .map(|c| org.subtree(c))
                    .unwrap_or_default();
                for pid in project_team.iter().rev() {
                    if let Some(p) = org.positions.get(pid) {
                        archive(
                            tx,
                            out,
                            &org,
                            p,
                            Some(&with_project),
                            "department archived",
                            actor,
                        )?;
                        done.insert(pid.clone());
                    }
                }
                let note = json!({ "with": with_department, "positions": project_team });
                tx.execute(
                    "UPDATE projects SET status = 'archived', archived_at = ?2,
                         metadata = json_set(metadata, '$.archive', json(?3))
                     WHERE id = ?1",
                    params![pr.id, at, note.to_string()],
                )?;
                org_event(
                    out,
                    tx,
                    actor,
                    "project_archived",
                    json!({
                        "id": pr.id,
                        "name": pr.name,
                        "positions": project_team,
                        "department": d.name,
                    }),
                )?;
            }
            let rest: Vec<String> = team.into_iter().filter(|p| !done.contains(p)).collect();
            for pid in rest.iter().rev() {
                if let Some(p) = org.positions.get(pid) {
                    archive(
                        tx,
                        out,
                        &org,
                        p,
                        Some(&with_department),
                        "department archived",
                        actor,
                    )?;
                }
            }
            let project_ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
            let note = json!({ "status": d.status, "projects": project_ids, "positions": rest });
            tx.execute(
                "UPDATE departments SET status = 'inactive', archived_at = ?2,
                     metadata = json_set(metadata, '$.archive', json(?3))
                 WHERE id = ?1",
                params![id, at, note.to_string()],
            )?;
            org_event(
                out,
                tx,
                actor,
                "department_archived",
                json!({
                    "id": id,
                    "name": d.name,
                    "projects": projects.iter().map(|p| &p.name).collect::<Vec<_>>(),
                    "positions": rest,
                }),
            )?;
            get_department(tx, id)
        })
    }

    /// Bring an archived position back on its own. One archived with a project or department
    /// comes back with it instead.
    pub fn bring_back_position(
        &self,
        id: &str,
        providers: &Providers,
        actor: &str,
    ) -> Result<Position> {
        self.write(|tx, out| {
            let p = get_position(tx, id)?;
            if p.is_deleted() {
                return Err(invalid(if p.metadata["deleted"]["savedId"].is_string() {
                    format!("{} is in your Workforce; hire it from there", p.title)
                } else {
                    format!("{} was deleted for good", p.title)
                }));
            }
            if p.state == PositionState::Active {
                return Err(invalid(format!("{} is not archived", p.title)));
            }
            let with = &p.metadata["archive"]["with"];
            if let (Some(kind), Some(name)) = (with["kind"].as_str(), with["name"].as_str()) {
                return Err(invalid(format!(
                    "{} was archived with the {name} {kind}; bring the {kind} back, and it comes \
                     back too",
                    p.title
                )));
            }
            let org = Org::load(tx)?;
            if let Some(d) = org.heads.get(id) {
                return Err(invalid(format!(
                    "{} is the manager of the {} department; bring the department back instead",
                    p.title, d.name
                )));
            }
            if let Some(pr) = org.coordinators.get(id) {
                return Err(invalid(format!(
                    "{} is the supervisor of the {} project; bring the project back instead",
                    p.title, pr.name
                )));
            }
            let role = org.role_of(&p)?;
            if role.role_type == RoleType::DepartmentManager {
                return Err(invalid(format!(
                    "{} was a department's manager, and its department is gone; a manager comes \
                     back only with its department",
                    p.title
                )));
            }
            let (_, skipped) = restore_all(tx, out, vec![p], providers, actor)?;
            if !skipped.is_empty() {
                org_event(
                    out,
                    tx,
                    actor,
                    "oversight_not_restored",
                    json!({ "positionId": id, "skipped": skipped }),
                )?;
            }
            get_position(tx, id)
        })
    }

    /// Bring back an archived project with the team archived with it.
    pub fn bring_back_project(
        &self,
        id: &str,
        providers: &Providers,
        actor: &str,
    ) -> Result<Project> {
        self.write(|tx, out| {
            let pr = get_project(tx, id)?;
            if pr.deleted_at.is_some() {
                return Err(invalid(format!("{} was deleted for good", pr.name)));
            }
            if pr.status != "archived" {
                return Err(invalid(format!("{} is not archived", pr.name)));
            }
            let with = &pr.metadata["archive"]["with"];
            if let Some(dept) = with["id"].as_str() {
                let d = get_department(tx, dept)?;
                if d.archived_at.is_some() && d.deleted_at.is_none() {
                    return Err(invalid(format!(
                        "{} was archived with the {} department; bring the department back, and \
                         it comes back too",
                        pr.name, d.name
                    )));
                }
            }
            if let Some(dept) = &pr.department_id {
                let d = get_department(tx, dept)?;
                if d.deleted_at.is_some() {
                    return Err(invalid(format!(
                        "{}'s department was deleted for good, so it cannot come back",
                        pr.name
                    )));
                }
                if d.archived_at.is_some() {
                    return Err(invalid(format!(
                        "{} belongs to the {} department, which is archived; bring the department \
                         back first",
                        pr.name, d.name
                    )));
                }
            }
            let mut team = archived_with_item(tx, id)?;
            if pr.metadata.get("archive").is_none() {
                team.extend(legacy_team(tx, &pr)?);
            }
            tx.execute(
                "UPDATE projects SET status = 'active', archived_at = NULL,
                     metadata = json_remove(metadata, '$.archive')
                 WHERE id = ?1",
                [id],
            )?;
            let (positions, skipped) = restore_all(tx, out, team, providers, actor)?;
            lead_is_back(
                tx,
                pr.coordinator_position_id.as_deref(),
                &pr.name,
                "supervisor",
            )?;
            org_event(
                out,
                tx,
                actor,
                "project_restored",
                json!({
                    "id": id,
                    "name": pr.name,
                    "positions": positions,
                    "oversightSkipped": skipped,
                }),
            )?;
            get_project(tx, id)
        })
    }

    /// Bring back an archived department with everything archived with it.
    pub fn bring_back_department(
        &self,
        id: &str,
        providers: &Providers,
        actor: &str,
    ) -> Result<Department> {
        self.write(|tx, out| {
            let d = get_department(tx, id)?;
            if d.deleted_at.is_some() {
                return Err(invalid(format!("{} was deleted for good", d.name)));
            }
            if d.archived_at.is_none() {
                return Err(invalid(format!("{} is not archived", d.name)));
            }
            let status = match d.metadata["archive"]["status"].as_str() {
                Some("inactive") => "inactive",
                _ => "active",
            };
            tx.execute(
                "UPDATE departments SET status = ?2, archived_at = NULL,
                     metadata = json_remove(metadata, '$.archive')
                 WHERE id = ?1",
                params![id, status],
            )?;
            let projects = all(
                tx,
                &format!(
                    "SELECT {PROJECT_COLS} FROM projects
                     WHERE department_id = ?1 AND status = 'archived' AND deleted_at IS NULL
                       AND json_extract(metadata, '$.archive.with.id') = ?1"
                ),
                [id],
                project_row,
            )?;
            let mut team = archived_with_item(tx, id)?;
            for pr in &projects {
                tx.execute(
                    "UPDATE projects SET status = 'active', archived_at = NULL,
                         metadata = json_remove(metadata, '$.archive')
                     WHERE id = ?1",
                    [&pr.id],
                )?;
                team.extend(archived_with_item(tx, &pr.id)?);
            }
            let (positions, skipped) = restore_all(tx, out, team, providers, actor)?;
            lead_is_back(tx, d.head_position_id.as_deref(), &d.name, "manager")?;
            for pr in &projects {
                lead_is_back(
                    tx,
                    pr.coordinator_position_id.as_deref(),
                    &pr.name,
                    "supervisor",
                )?;
            }
            org_event(
                out,
                tx,
                actor,
                "department_restored",
                json!({
                    "id": id,
                    "name": d.name,
                    "projects": projects.iter().map(|p| &p.name).collect::<Vec<_>>(),
                    "positions": positions,
                    "oversightSkipped": skipped,
                }),
            )?;
            get_department(tx, id)
        })
    }

    /// What deleting `kind` (`position`, `project`, `department`) `id` for good would take along,
    /// or why it cannot be deleted now. For the confirmation.
    pub fn deletion_plan(&self, kind: &str, id: &str) -> Result<DeletionPlan> {
        self.read(|c| plan(c, kind, id))
    }

    /// Delete an archived position for good, with the archived positions under it; those in
    /// `save` move to the Workforce instead.
    pub fn delete_position_for_good(
        &self,
        id: &str,
        save: &[SaveAgent],
        actor: &str,
    ) -> Result<Deleted> {
        self.write(|tx, out| {
            let plan = plan(tx, "position", id)?;
            delete_plan(tx, out, &plan, save, actor)
        })
    }

    /// Delete an archived project for good, with its archived team.
    pub fn delete_project_for_good(
        &self,
        id: &str,
        save: &[SaveAgent],
        actor: &str,
    ) -> Result<Deleted> {
        self.write(|tx, out| {
            let plan = plan(tx, "project", id)?;
            delete_plan(tx, out, &plan, save, actor)
        })
    }

    /// Delete an archived department for good, with everything archived in it.
    pub fn delete_department_for_good(
        &self,
        id: &str,
        save: &[SaveAgent],
        actor: &str,
    ) -> Result<Deleted> {
        self.write(|tx, out| {
            let plan = plan(tx, "department", id)?;
            delete_plan(tx, out, &plan, save, actor)
        })
    }
}
