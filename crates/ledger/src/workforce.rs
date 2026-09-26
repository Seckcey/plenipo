//! The Workforce's records (Phase 5, ADR-009): positions, the agents that fill them, oversight,
//! and the organization's structure rules.
//!
//! Every operation is one transaction that loads the organization as it is inside that
//! transaction, checks the rules against it, writes, and records its `org.*` events — so no
//! rule can be bypassed by a concurrent change. The Workforce service decides what to ask for
//! (runtimes, prompts, routing); this module keeps the organization consistent.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::events;
use crate::org::{
    agent_row, dept_row, project_row, role_row, AGENT_COLS, DEPT_COLS, PROJECT_COLS, ROLE_COLS,
};
use crate::rows::{json as parse_json, metadata_text, opt_u64, parse_enum, u64_of};
use crate::Ledger;

const POSITION_COLS: &str = "id, title, role_id, reports_to, runtime_id, model, state, sort_key, \
    metadata, created_at, updated_at, archived_at";

/// Stored as a position's `runtime_id` when the position is automatic: its role's model policy
/// chooses the runtime (Phase 6, ADR-011). No runtime may use this ID.
pub const AUTOMATIC: &str = "auto";

fn position_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Position> {
    let runtime: String = r.get(4)?;
    Ok(Position {
        id: r.get(0)?,
        title: r.get(1)?,
        role_id: r.get(2)?,
        reports_to: r.get(3)?,
        runtime_id: (runtime != AUTOMATIC).then_some(runtime),
        model: r.get(5)?,
        state: parse_enum(6, r.get(6)?, PositionState::parse)?,
        sort_key: r.get(7)?,
        metadata: parse_json(r.get(8)?),
        created_at: u64_of(r.get(9)?),
        updated_at: u64_of(r.get(10)?),
        archived_at: opt_u64(r.get(11)?),
    })
}

const OVERSIGHT_COLS: &str = "id, kind, overseer_id, target_id, state, created_at, ended_at";

fn oversight_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Oversight> {
    Ok(Oversight {
        id: r.get(0)?,
        kind: parse_enum(1, r.get(1)?, OversightKind::parse)?,
        overseer_id: r.get(2)?,
        target_id: r.get(3)?,
        active: r.get::<_, String>(4)? == "active",
        created_at: u64_of(r.get(5)?),
        ended_at: opt_u64(r.get(6)?),
    })
}

fn all<T, P: rusqlite::Params>(
    c: &Connection,
    sql: &str,
    p: P,
    map: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = c.prepare(sql)?;
    let rows = stmt.query_map(p, map)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn invalid(message: impl Into<String>) -> LedgerError {
    LedgerError::InvalidInput(message.into())
}

fn now() -> i64 {
    i64::try_from(crate::now_ms()).unwrap_or(i64::MAX)
}

/// A unique-constraint failure as a caller error with `message`.
fn unique(e: rusqlite::Error, message: &str) -> LedgerError {
    match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            invalid(message)
        }
        other => other.into(),
    }
}

// ---- Input validation --------------------------------------------------------------------

/// Trimmed, 1–`max` characters, one line, no control characters.
pub fn clean_line(what: &str, value: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(invalid(format!("{what} must be 1–{max} characters")));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid(format!("{what} must be a single line of text")));
    }
    Ok(value.to_owned())
}

/// Trimmed, at most `max` characters; line breaks and tabs allowed, other control characters
/// not.
fn clean_text(what: &str, value: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.chars().count() > max {
        return Err(invalid(format!("{what} must be at most {max} characters")));
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(invalid(format!("{what} contains control characters")));
    }
    Ok(value.to_owned())
}

/// `[a-z0-9][a-z0-9-]{0,31}`: a runtime (adapter) ID (never [`AUTOMATIC`]).
pub fn clean_runtime_id(id: &str) -> Result<String> {
    let id = id.trim();
    let mut chars = id.chars();
    let ok = (1..=32).contains(&id.len())
        && id != AUTOMATIC
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(id.to_owned())
    } else {
        Err(invalid(format!("invalid AI tool name {id:?}")))
    }
}

/// `[A-Za-z0-9][A-Za-z0-9._:\[\]-]{0,63}` — a model name, never a flag or a path (the same
/// rule the agent runtime applies).
pub fn clean_model(model: &str) -> Result<String> {
    let model = model.trim();
    let mut chars = model.chars();
    let ok = (1..=64).contains(&model.len())
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '[' | ']' | '-'));
    if ok {
        Ok(model.to_owned())
    } else {
        Err(invalid(format!("invalid model name {model:?}")))
    }
}

fn clean_optional_model(model: Option<&str>) -> Result<Option<String>> {
    model
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(clean_model)
        .transpose()
}

/// A repository location: an `https://`, `http://`, `ssh://`, or `git://` URL, or an SCP-style
/// `user@host:path`. Recorded only; Plenipo does not contact it in this phase.
fn clean_repository(url: &str) -> Result<String> {
    let url = url.trim();
    let schemes = ["https://", "http://", "ssh://", "git://"];
    let scp = url
        .split_once('@')
        .is_some_and(|(user, rest)| !user.is_empty() && rest.contains(':'));
    let ok = (1..=500).contains(&url.len())
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && (schemes
            .iter()
            .any(|s| url.starts_with(s) && url.len() > s.len())
            || scp);
    if ok {
        Ok(url.to_owned())
    } else {
        Err(invalid(
            "the repository must be an https://, ssh://, or git@host:path address",
        ))
    }
}

/// An absolute local folder path (Windows or Unix). Recorded only; workers are not given it
/// before Guard (Phase 7).
fn clean_local_path(path: &str) -> Result<String> {
    let path = path.trim();
    let bytes = path.as_bytes();
    let windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    let absolute = path.starts_with('/') || path.starts_with("\\\\") || windows_drive;
    if (1..=1000).contains(&path.len()) && absolute && !path.chars().any(char::is_control) {
        Ok(path.to_owned())
    } else {
        Err(invalid(
            "the local folder must be an absolute path, for example D:\\projects\\cloudline",
        ))
    }
}

/// A capability profile name: 1–64 of letters, digits, space, `.`, `_`, `-`.
fn clean_profile(name: &str) -> Result<String> {
    let name = name.trim();
    let ok = (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'));
    if ok {
        Ok(name.to_owned())
    } else {
        Err(invalid(format!("invalid capability profile name {name:?}")))
    }
}

fn clean_runtimes(list: &[String]) -> Result<Vec<String>> {
    if list.len() > 16 {
        return Err(invalid("at most 16 allowed AI tools"));
    }
    let mut out: Vec<String> = Vec::new();
    for id in list {
        let id = clean_runtime_id(id)?;
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Validated project settings.
fn clean_settings(s: &ProjectSettings) -> Result<ProjectSettings> {
    Ok(ProjectSettings {
        name: clean_line("the project name", &s.name, 120)?,
        description: clean_text("the description", &s.description, 2000)?,
        repository_url: s
            .repository_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(clean_repository)
            .transpose()?,
        local_path: s
            .local_path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(clean_local_path)
            .transpose()?,
        allowed_runtimes: clean_runtimes(&s.allowed_runtimes)?,
        capability_profile: s
            .capability_profile
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(clean_profile)
            .transpose()?,
    })
}

/// Validated input for a new position.
fn clean_position(new: &NewPosition) -> Result<NewPosition> {
    Ok(NewPosition {
        title: clean_line("the title", &new.title, 80)?,
        role_id: new.role_id.clone(),
        reports_to: new.reports_to.clone(),
        runtime_id: new
            .runtime_id
            .as_deref()
            .map(clean_runtime_id)
            .transpose()?,
        runtime_provider: new
            .runtime_provider
            .as_deref()
            .filter(|_| new.runtime_id.is_some())
            .map(|p| clean_line("the provider", p, 64))
            .transpose()?,
        model: fixed_model(new.runtime_id.as_deref(), new.model.as_deref())?,
        staffed: new.staffed,
    })
}

/// A position's model: only a fixed runtime has one (an automatic position's model comes from
/// its role's model policy).
fn fixed_model(runtime_id: Option<&str>, model: Option<&str>) -> Result<Option<String>> {
    let model = clean_optional_model(model)?;
    if runtime_id.is_none() && model.is_some() {
        return Err(invalid(
            "an automatic position gets its model from its role's model choices; choose an AI \
             tool to set a model",
        ));
    }
    Ok(model)
}

fn role_type_label(t: RoleType) -> &'static str {
    match t {
        RoleType::Superintendent => "VP",
        RoleType::DepartmentManager => "manager",
        RoleType::ProjectCoordinator => "supervisor",
        RoleType::Worker => "worker",
    }
}

// ---- The organization inside a transaction -------------------------------------------------

/// The active organization, loaded inside a write transaction so rules are checked against
/// exactly what the transaction will change.
#[derive(Clone)]
struct Org {
    /// Active positions.
    positions: HashMap<String, Position>,
    roles: HashMap<String, Role>,
    /// Head position → department.
    heads: HashMap<String, Department>,
    /// Coordinator position → project (active and archived projects).
    coordinators: HashMap<String, Project>,
    /// Active oversight assignments.
    oversight: Vec<Oversight>,
}

impl Org {
    fn load(c: &Connection) -> Result<Self> {
        let positions = all(
            c,
            &format!("SELECT {POSITION_COLS} FROM positions WHERE state = 'active'"),
            [],
            position_row,
        )?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
        let roles = all(c, &format!("SELECT {ROLE_COLS} FROM roles"), [], role_row)?
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let heads = all(
            c,
            &format!("SELECT {DEPT_COLS} FROM departments WHERE head_position_id IS NOT NULL"),
            [],
            dept_row,
        )?
        .into_iter()
        .filter_map(|d| d.head_position_id.clone().map(|h| (h, d)))
        .collect();
        let coordinators = all(
            c,
            &format!(
                "SELECT {PROJECT_COLS} FROM projects WHERE coordinator_position_id IS NOT NULL"
            ),
            [],
            project_row,
        )?
        .into_iter()
        .filter_map(|p| p.coordinator_position_id.clone().map(|c| (c, p)))
        .collect();
        let oversight = all(
            c,
            &format!("SELECT {OVERSIGHT_COLS} FROM oversight WHERE state = 'active'"),
            [],
            oversight_row,
        )?;
        Ok(Self {
            positions,
            roles,
            heads,
            coordinators,
            oversight,
        })
    }

    /// An active position, or why it cannot be used.
    fn position(&self, c: &Connection, id: &str) -> Result<&Position> {
        if let Some(p) = self.positions.get(id) {
            return Ok(p);
        }
        let archived: Option<String> = c
            .query_row("SELECT title FROM positions WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        Err(match archived {
            Some(title) => invalid(format!("{title} has been archived")),
            None => LedgerError::NotFound(format!("position {id}")),
        })
    }

    fn role(&self, role_id: &str) -> Result<&Role> {
        self.roles
            .get(role_id)
            .ok_or_else(|| LedgerError::NotFound(format!("role {role_id}")))
    }

    fn role_of(&self, p: &Position) -> Result<&Role> {
        self.role(&p.role_id)
    }

    /// Active positions reporting to `lead` (`None`: the owner).
    fn reports<'a>(&'a self, lead: Option<&'a str>) -> impl Iterator<Item = &'a Position> + 'a {
        self.positions
            .values()
            .filter(move |p| p.reports_to.as_deref() == lead)
    }

    /// Active positions overseeing the team led by `lead`.
    fn overseers(&self, lead: &str) -> Vec<&Position> {
        self.oversight
            .iter()
            .filter(|o| o.target_id == lead)
            .filter_map(|o| self.positions.get(&o.overseer_id))
            .collect()
    }

    /// `id` is `root` or reports to it, directly or indirectly.
    fn is_within(&self, id: &str, root: &str) -> bool {
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(at) = current {
            if at == root {
                return true;
            }
            if !seen.insert(at) {
                return false;
            }
            current = self.positions.get(at).and_then(|p| p.reports_to.as_deref());
        }
        false
    }

    /// `root` and every active position below it, parents before children.
    fn subtree(&self, root: &str) -> Vec<String> {
        let mut out = vec![root.to_owned()];
        let mut i = 0;
        while i < out.len() {
            let lead = out[i].clone();
            let mut children: Vec<&Position> = self.reports(Some(&lead)).collect();
            children.sort_by_key(|p| (p.sort_key, p.created_at));
            out.extend(children.into_iter().map(|p| p.id.clone()));
            i += 1;
        }
        out
    }

    /// The department of `id`: the nearest department head at or above it.
    fn department_of(&self, id: &str) -> Option<&Department> {
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(at) = current {
            if let Some(d) = self.heads.get(at) {
                return Some(d);
            }
            if !seen.insert(at) {
                return None;
            }
            current = self.positions.get(at).and_then(|p| p.reports_to.as_deref());
        }
        None
    }

    /// The project of `id`: the nearest coordinator at or above it.
    fn project_of(&self, id: &str) -> Option<&Project> {
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(at) = current {
            if let Some(p) = self.coordinators.get(at) {
                return Some(p);
            }
            if !seen.insert(at) {
                return None;
            }
            current = self.positions.get(at).and_then(|p| p.reports_to.as_deref());
        }
        None
    }

    fn lead_name(&self, lead: Option<&str>) -> String {
        lead.and_then(|l| self.positions.get(l))
            .map_or_else(|| "The owner".to_owned(), |p| p.title.clone())
    }

    /// `title` must not be the title of another member of `lead`'s team (its reports and its
    /// overseers), `me` excepted.
    fn check_title(&self, title: &str, lead: Option<&str>, me: Option<&str>) -> Result<()> {
        let key = title.to_lowercase();
        let overseers = lead.map(|l| self.overseers(l)).unwrap_or_default();
        let clash = self
            .reports(lead)
            .chain(overseers)
            .find(|p| Some(p.id.as_str()) != me && p.title.to_lowercase() == key);
        match clash {
            Some(p) => Err(invalid(format!(
                "{} already has a team member named \"{}\"; choose another title",
                self.lead_name(lead),
                p.title
            ))),
            None => Ok(()),
        }
    }

    /// May a position with `role` (itself `me`, when it exists) report to `to`?
    fn check_supervisor(
        &self,
        c: &Connection,
        role: &Role,
        me: Option<&str>,
        to: Option<&str>,
    ) -> Result<()> {
        let label = role_type_label(role.role_type);
        let Some(to) = to else {
            return match role.role_type {
                RoleType::Superintendent | RoleType::DepartmentManager => Ok(()),
                RoleType::ProjectCoordinator => Err(invalid(
                    "a supervisor reports to the manager of its project's department",
                )),
                RoleType::Worker => Err(invalid(
                    "a worker reports to a supervisor, manager, or other full-time position, \
                     not directly to you",
                )),
            };
        };
        let boss = self.position(c, to)?;
        let boss_role = self.role_of(boss)?;
        if !boss_role.persistent {
            return Err(invalid(format!(
                "{} is an on-call position; only full-time positions lead others",
                boss.title
            )));
        }
        if let Some(me) = me {
            if self.is_within(to, me) {
                return Err(invalid(format!(
                    "{} reports to this position, so this position cannot report to it",
                    boss.title
                )));
            }
        }
        match role.role_type {
            RoleType::Superintendent | RoleType::DepartmentManager
                if boss_role.role_type != RoleType::Superintendent =>
            {
                Err(invalid(format!("a {label} reports to you or to a VP")))
            }
            RoleType::ProjectCoordinator if !self.heads.contains_key(to) => {
                Err(invalid("a supervisor reports to a department's manager"))
            }
            _ => Ok(()),
        }
    }

    /// A fixed `runtime_id` must be allowed by `project` (when the position belongs to one). An
    /// automatic position is routed within the project's runtimes when it takes work.
    fn check_runtime(
        project: Option<&Project>,
        runtime_id: Option<&str>,
        title: &str,
    ) -> Result<()> {
        let Some(runtime_id) = runtime_id else {
            return Ok(());
        };
        match project {
            Some(p) if !p.allowed_runtimes.iter().any(|r| r == runtime_id) => {
                Err(invalid(format!(
                    "{} does not allow the {runtime_id} AI tool, so {title} cannot use it; change \
                     the project's allowed AI tools or choose another one",
                    p.name
                )))
            }
            _ => Ok(()),
        }
    }
}

// ---- Writes used by several operations -------------------------------------------------------

fn org_event(
    out: &mut Vec<LedgerEvent>,
    tx: &Connection,
    actor: &str,
    kind: &str,
    payload: Value,
) -> Result<()> {
    out.push(events::insert(
        tx,
        NewEvent {
            source: actor.into(),
            event_type: format!("org.{kind}"),
            payload,
            ..NewEvent::default()
        },
    )?);
    Ok(())
}

fn get_position(c: &Connection, id: &str) -> Result<Position> {
    c.query_row(
        &format!("SELECT {POSITION_COLS} FROM positions WHERE id = ?1"),
        [id],
        position_row,
    )
    .optional()?
    .ok_or_else(|| LedgerError::NotFound(format!("position {id}")))
}

fn next_sort_key(c: &Connection, lead: Option<&str>) -> Result<i64> {
    Ok(c.query_row(
        "SELECT COALESCE(MAX(sort_key), 0) + 1 FROM positions WHERE reports_to IS ?1",
        [lead],
        |r| r.get(0),
    )?)
}

fn insert_position(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    new: &NewPosition,
    reports_to: Option<&str>,
    actor: &str,
) -> Result<Position> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now();
    tx.execute(
        "INSERT INTO positions (id, title, role_id, reports_to, runtime_id, model, state,
             sort_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8, ?8)",
        params![
            id,
            new.title,
            new.role_id,
            reports_to,
            new.runtime_id.as_deref().unwrap_or(AUTOMATIC),
            new.model,
            next_sort_key(tx, reports_to)?,
            now
        ],
    )
    .map_err(|e| unique(e, "that team already has a member with this title"))?;
    org_event(
        out,
        tx,
        actor,
        "position_created",
        json!({
            "positionId": id,
            "title": new.title,
            "roleId": new.role_id,
            "reportsTo": reports_to,
            "runtimeId": new.runtime_id,
            "automatic": new.runtime_id.is_none(),
            "model": new.model,
        }),
    )?;
    get_position(tx, &id)
}

/// The active incumbent of a persistent position.
fn incumbent(c: &Connection, position_id: &str) -> Result<Option<AgentInstance>> {
    Ok(c.query_row(
        &format!(
            "SELECT {AGENT_COLS} FROM agent_instances
             WHERE position_id = ?1 AND task_id IS NULL
               AND lifecycle_state NOT IN ('retired', 'failed')"
        ),
        [position_id],
        agent_row,
    )
    .optional()?)
}

/// Hire an incumbent into a persistent position. An automatic position's agent has no runtime
/// until its first conversation is routed ([`Ledger::route_agent`]).
fn hire(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    position: &Position,
    runtime_provider: Option<&str>,
    project_id: Option<&str>,
    actor: &str,
) -> Result<AgentInstance> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now();
    tx.execute(
        "INSERT INTO agent_instances (id, role_id, runtime_provider, project_id, lifecycle_state,
             created_at, last_seen_at, position_id, runtime_id, model)
         VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5, ?6, ?7, ?8)",
        params![
            id,
            position.role_id,
            runtime_provider.filter(|_| position.runtime_id.is_some()),
            project_id,
            now,
            position.id,
            position.runtime_id,
            position.model
        ],
    )
    .map_err(|e| unique(e, &format!("{} is already staffed", position.title)))?;
    org_event(
        out,
        tx,
        actor,
        "agent_hired",
        json!({
            "agentId": id,
            "positionId": position.id,
            "title": position.title,
            "runtimeId": position.runtime_id,
            "automatic": position.runtime_id.is_none(),
            "model": position.model,
        }),
    )?;
    tx.query_row(
        &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
        [&id],
        agent_row,
    )
    .map_err(Into::into)
}

/// Tasks of `position_id` (its incumbents' turns, or its spawned workers' tasks) that have not
/// finished.
fn unfinished_work(c: &Connection, position_id: &str) -> Result<u32> {
    Ok(c.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE json_extract(metadata, '$.workforce.positionId') = ?1
           AND state NOT IN ('succeeded', 'failed', 'cancelled')",
        [position_id],
        |r| r.get(0),
    )?)
}

fn refuse_if_busy(c: &Connection, position: &Position, doing: &str) -> Result<()> {
    match unfinished_work(c, &position.id)? {
        0 => Ok(()),
        n => Err(invalid(format!(
            "{} has {n} unfinished task{}; wait for {} or cancel {} before {doing}",
            position.title,
            if n == 1 { "" } else { "s" },
            if n == 1 { "it" } else { "them" },
            if n == 1 { "it" } else { "them" },
        ))),
    }
}

/// Retire the incumbent of `position`, if any.
fn retire_incumbent(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    position: &Position,
    reason: &str,
    actor: &str,
) -> Result<Option<AgentInstance>> {
    let Some(agent) = incumbent(tx, &position.id)? else {
        return Ok(None);
    };
    let now = now();
    tx.execute(
        "UPDATE agent_instances SET lifecycle_state = 'retired', retired_at = ?2, last_seen_at = ?2
         WHERE id = ?1",
        params![agent.id, now],
    )?;
    org_event(
        out,
        tx,
        actor,
        "agent_retired",
        json!({
            "agentId": agent.id,
            "positionId": position.id,
            "title": position.title,
            "reason": reason,
        }),
    )?;
    Ok(Some(tx.query_row(
        &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
        [&agent.id],
        agent_row,
    )?))
}

fn end_oversight_row(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    o: &Oversight,
    reason: &str,
    actor: &str,
) -> Result<()> {
    tx.execute(
        "UPDATE oversight SET state = 'ended', ended_at = ?2 WHERE id = ?1 AND state = 'active'",
        params![o.id, now()],
    )?;
    org_event(
        out,
        tx,
        actor,
        "oversight_ended",
        json!({
            "oversightId": o.id,
            "kind": o.kind,
            "overseerId": o.overseer_id,
            "targetId": o.target_id,
            "reason": reason,
        }),
    )
}

/// Archive `position` (already checked): retire its incumbent and end its oversight.
fn archive(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    org: &Org,
    position: &Position,
    reason: &str,
    actor: &str,
) -> Result<()> {
    retire_incumbent(tx, out, position, reason, actor)?;
    for o in org
        .oversight
        .iter()
        .filter(|o| o.overseer_id == position.id || o.target_id == position.id)
    {
        end_oversight_row(tx, out, o, reason, actor)?;
    }
    tx.execute(
        "UPDATE positions SET state = 'archived', archived_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![position.id, now()],
    )?;
    org_event(
        out,
        tx,
        actor,
        "position_archived",
        json!({ "positionId": position.id, "title": position.title, "reason": reason }),
    )
}

// ---- Worker lifecycle (called by the task state machine) -------------------------------------

/// A spawned worker's lifecycle follows its task (ADR-009 §4): it becomes active when the task
/// starts and leaves the workforce when the task ends — in the task's own transaction. The task
/// names its worker (`metadata.workforce.agentId`, checked when the worker is recorded).
pub(crate) fn follow_task(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    task: &Task,
    to: TaskState,
    actor: &str,
) -> Result<()> {
    let Some(agent_id) = task.metadata["workforce"]["agentId"].as_str() else {
        return Ok(());
    };
    // Only a worker spawned for this very task (an incumbent's turns name it too).
    let state: Option<String> = tx
        .query_row(
            "SELECT lifecycle_state FROM agent_instances WHERE id = ?1 AND task_id = ?2",
            [agent_id, task.id.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    let Some(state) = state else {
        return Ok(());
    };
    let now = now();
    let event = |event_type: &str, payload: Value| NewEvent {
        task_id: Some(task.id.clone()),
        source: actor.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    };
    if to == TaskState::Running && state == "starting" {
        tx.execute(
            "UPDATE agent_instances SET lifecycle_state = 'active', last_seen_at = ?2 WHERE id = ?1",
            params![agent_id, now],
        )?;
        out.push(events::insert(
            tx,
            event("org.worker_started", json!({ "agentId": agent_id })),
        )?);
    } else if to.is_terminal() && matches!(state.as_str(), "starting" | "active" | "idle") {
        let next = if to == TaskState::Failed {
            AgentLifecycle::Failed
        } else {
            AgentLifecycle::Retired
        };
        tx.execute(
            "UPDATE agent_instances SET lifecycle_state = ?2, retired_at = ?3, last_seen_at = ?3
             WHERE id = ?1",
            params![agent_id, next.as_str(), now],
        )?;
        out.push(events::insert(
            tx,
            event(
                "org.worker_retired",
                json!({ "agentId": agent_id, "outcome": to, "lifecycle": next }),
            ),
        )?);
    }
    Ok(())
}

/// Record the worker spawned for a delegated child task (called by Liaison's transaction).
pub(crate) fn insert_worker(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    worker: &NewWorker,
    task_id: &str,
    actor: &str,
) -> Result<()> {
    let task = crate::tasks::require(tx, task_id)?;
    let named = &task.metadata["workforce"];
    if named["agentId"].as_str() != Some(worker.agent_id.as_str())
        || named["positionId"].as_str() != Some(worker.position_id.as_str())
    {
        return Err(invalid(format!(
            "task {task_id} does not name this worker and position in its metadata"
        )));
    }
    let position = get_position(tx, &worker.position_id)?;
    if position.state != PositionState::Active {
        return Err(invalid(format!("{} has been archived", position.title)));
    }
    if worker.role_id != position.role_id {
        return Err(invalid("a worker's role must be its position's role"));
    }
    let runtime_id = clean_runtime_id(&worker.runtime_id)?;
    let model = clean_optional_model(worker.model.as_deref())?;
    let now = now();
    tx.execute(
        "INSERT INTO agent_instances (id, role_id, runtime_provider, project_id, lifecycle_state,
             created_at, last_seen_at, position_id, runtime_id, model, task_id)
         VALUES (?1, ?2, ?3, ?4, 'starting', ?5, ?5, ?6, ?7, ?8, ?9)",
        params![
            worker.agent_id,
            worker.role_id,
            worker.runtime_provider,
            worker.project_id,
            now,
            worker.position_id,
            runtime_id,
            model,
            task_id
        ],
    )
    .map_err(|e| unique(e, "a worker is already recorded for this task"))?;
    out.push(events::insert(
        tx,
        NewEvent {
            task_id: Some(task_id.into()),
            source: actor.into(),
            event_type: "org.worker_spawned".into(),
            payload: json!({
                "agentId": worker.agent_id,
                "positionId": worker.position_id,
                "title": position.title,
                "runtimeId": runtime_id,
                "model": model,
                "routing": worker.routing,
            }),
            ..NewEvent::default()
        },
    )?);
    Ok(())
}

/// A role template (seeded when missing).
#[derive(Debug, Clone, PartialEq)]
pub struct RoleTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub role_type: RoleType,
    pub persistent: bool,
    pub metadata: Value,
    /// Names the template was seeded under before: that role is renamed, not duplicated.
    pub formerly: &'static [&'static str],
}

impl Ledger {
    // ---- Reads ----------------------------------------------------------------------------

    /// The whole organization in one consistent read.
    pub fn org_records(&self) -> Result<OrgRecords> {
        self.read(|c| {
            let mut former = HashMap::new();
            let mut stmt = c.prepare(
                "SELECT position_id,
                        SUM(lifecycle_state = 'retired'), SUM(lifecycle_state = 'failed'),
                        MAX(retired_at)
                 FROM agent_instances
                 WHERE position_id IS NOT NULL AND lifecycle_state IN ('retired', 'failed')
                 GROUP BY position_id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    FormerAgents {
                        retired: r.get(1)?,
                        failed: r.get(2)?,
                        last_retired_at: opt_u64(r.get(3)?),
                    },
                ))
            })?;
            for row in rows {
                let (id, f) = row?;
                former.insert(id, f);
            }
            Ok(OrgRecords {
                roles: all(c, &format!("SELECT {ROLE_COLS} FROM roles ORDER BY name"), [], role_row)?,
                departments: all(
                    c,
                    &format!("SELECT {DEPT_COLS} FROM departments ORDER BY name"),
                    [],
                    dept_row,
                )?,
                projects: all(
                    c,
                    &format!("SELECT {PROJECT_COLS} FROM projects ORDER BY name"),
                    [],
                    project_row,
                )?,
                positions: all(
                    c,
                    &format!(
                        "SELECT {POSITION_COLS} FROM positions ORDER BY reports_to, sort_key, created_at"
                    ),
                    [],
                    position_row,
                )?,
                agents: all(
                    c,
                    &format!(
                        "SELECT {AGENT_COLS} FROM agent_instances
                         WHERE lifecycle_state NOT IN ('retired', 'failed') ORDER BY created_at"
                    ),
                    [],
                    agent_row,
                )?,
                oversight: all(
                    c,
                    &format!(
                        "SELECT {OVERSIGHT_COLS} FROM oversight WHERE state = 'active' ORDER BY created_at"
                    ),
                    [],
                    oversight_row,
                )?,
                former_agents: former,
            })
        })
    }

    pub fn position(&self, id: &str) -> Result<Option<Position>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!("SELECT {POSITION_COLS} FROM positions WHERE id = ?1"),
                [id],
                position_row,
            )
            .optional()?)
        })
    }

    /// The active incumbent of a persistent position.
    pub fn position_incumbent(&self, position_id: &str) -> Result<Option<AgentInstance>> {
        self.read(|c| incumbent(c, position_id))
    }

    /// Every agent that has filled `position_id`, newest first.
    pub fn position_agents(&self, position_id: &str, limit: u32) -> Result<Vec<AgentInstance>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {AGENT_COLS} FROM agent_instances WHERE position_id = ?1
                     ORDER BY created_at DESC, rowid DESC LIMIT ?2"
                ),
                params![position_id, limit.clamp(1, 1000)],
                agent_row,
            )
        })
    }

    /// Tasks of a position (its incumbents' turns or its workers' tasks), newest first.
    pub fn position_tasks(&self, position_id: &str, limit: u32) -> Result<Vec<Task>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {} FROM tasks WHERE json_extract(metadata, '$.workforce.positionId') = ?1
                     ORDER BY created_at DESC, rowid DESC LIMIT ?2",
                    crate::rows::TASK_COLUMNS
                ),
                params![position_id, limit.clamp(1, 1000)],
                crate::rows::task,
            )
        })
    }

    /// Unfinished tasks anywhere in the organization, oldest first.
    pub fn open_workforce_tasks(&self) -> Result<Vec<Task>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {} FROM tasks
                     WHERE json_extract(metadata, '$.workforce.positionId') IS NOT NULL
                       AND state NOT IN ('succeeded', 'failed', 'cancelled')
                     ORDER BY created_at, rowid",
                    crate::rows::TASK_COLUMNS
                ),
                [],
                crate::rows::task,
            )
        })
    }

    /// Organization tasks that finished at or after `since` (ms), newest first.
    pub fn finished_workforce_tasks(&self, since: u64, limit: u32) -> Result<Vec<Task>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {} FROM tasks
                     WHERE json_extract(metadata, '$.workforce.positionId') IS NOT NULL
                       AND state IN ('succeeded', 'failed', 'cancelled') AND completed_at >= ?1
                     ORDER BY completed_at DESC, rowid DESC LIMIT ?2",
                    crate::rows::TASK_COLUMNS
                ),
                params![
                    i64::try_from(since).unwrap_or(i64::MAX),
                    limit.clamp(1, 1000)
                ],
                crate::rows::task,
            )
        })
    }

    pub fn setting(&self, key: &str) -> Result<Option<Value>> {
        self.read(|c| {
            Ok(
                c.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
                .map(parse_json),
            )
        })
    }

    // ---- Settings and roles -----------------------------------------------------------------

    pub fn put_setting(&self, key: &str, value: &Value, actor: &str) -> Result<()> {
        let key = clean_line("the setting key", key, 64)?;
        self.write(|tx, out| {
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value.to_string(), now()],
            )?;
            org_event(out, tx, actor, "settings_changed", json!({ "key": key }))
        })
    }

    /// Set `fields` in the object stored under `key`, keeping its other fields; returns the
    /// whole object. One transaction, so two changes to different fields never undo each other.
    pub fn merge_setting(&self, key: &str, fields: &Value, actor: &str) -> Result<Value> {
        let key = clean_line("the setting key", key, 64)?;
        let Some(fields) = fields.as_object() else {
            return Err(invalid("setting fields must be an object"));
        };
        self.write(|tx, out| {
            let mut value = tx
                .query_row("SELECT value FROM settings WHERE key = ?1", [&key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
                .map(parse_json)
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            for (field, v) in fields {
                value[field] = v.clone();
            }
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value.to_string(), now()],
            )?;
            let changed: Vec<&String> = fields.keys().collect();
            org_event(
                out,
                tx,
                actor,
                "settings_changed",
                json!({ "key": key, "fields": changed }),
            )?;
            Ok(value)
        })
    }

    /// Insert the templates that no role is named after yet, renaming a template's role seeded
    /// under one of its former names instead; returns every role.
    pub fn ensure_roles(&self, templates: &[RoleTemplate], actor: &str) -> Result<Vec<Role>> {
        self.write(|tx, out| {
            for t in templates {
                let exists: Option<String> = tx
                    .query_row("SELECT id FROM roles WHERE name = ?1", [t.name], |r| r.get(0))
                    .optional()?;
                if exists.is_some() {
                    continue;
                }
                let mut former = None;
                for old in t.formerly {
                    former = tx
                        .query_row(
                            "SELECT id FROM roles
                             WHERE name = ?1 AND json_extract(metadata, '$.template') = 1",
                            [old],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()?
                        .map(|id| (id, *old));
                    if former.is_some() {
                        break;
                    }
                }
                if let Some((id, old)) = former {
                    // Same role, so every position holding it keeps it.
                    tx.execute(
                        "UPDATE roles SET name = ?2, description = ?3, metadata = ?4 WHERE id = ?1",
                        params![id, t.name, t.description, metadata_text(&t.metadata)?],
                    )?;
                    org_event(
                        out,
                        tx,
                        actor,
                        "role_renamed",
                        json!({ "id": id, "name": t.name, "formerly": old, "template": true }),
                    )?;
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO roles (id, name, description, role_type, persistent, metadata, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        id,
                        t.name,
                        t.description,
                        t.role_type.as_str(),
                        t.persistent,
                        metadata_text(&t.metadata)?,
                        now()
                    ],
                )?;
                org_event(
                    out,
                    tx,
                    actor,
                    "role_created",
                    json!({ "id": id, "name": t.name, "roleType": t.role_type, "template": true }),
                )?;
            }
            all(tx, &format!("SELECT {ROLE_COLS} FROM roles ORDER BY name"), [], role_row)
        })
    }

    // ---- Departments --------------------------------------------------------------------------

    /// Create a department and the position that heads it (hired now when `head.staffed`).
    pub fn create_department_with_head(
        &self,
        name: &str,
        description: &str,
        head: &NewPosition,
        actor: &str,
    ) -> Result<(Department, Position)> {
        let name = clean_line("the department name", name, 120)?;
        let description = clean_text("the description", description, 2000)?;
        let head = clean_position(head)?;
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let role = org.role(&head.role_id)?;
            if !matches!(
                role.role_type,
                RoleType::Superintendent | RoleType::DepartmentManager
            ) || !role.persistent
            {
                return Err(invalid(format!(
                    "{} cannot run a department: choose a VP or manager role",
                    role.name
                )));
            }
            org.check_supervisor(tx, role, None, head.reports_to.as_deref())?;
            org.check_title(&head.title, head.reports_to.as_deref(), None)?;
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO departments (id, name, description, manager_role_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, name, description, role.id, now()],
            )
            .map_err(|e| unique(e, &format!("a department named \"{name}\" already exists")))?;
            let position = insert_position(tx, out, &head, head.reports_to.as_deref(), actor)?;
            tx.execute(
                "UPDATE departments SET head_position_id = ?2 WHERE id = ?1",
                params![id, position.id],
            )?;
            org_event(
                out,
                tx,
                actor,
                "department_created",
                json!({ "id": id, "name": name, "headPositionId": position.id }),
            )?;
            if head.staffed {
                hire(
                    tx,
                    out,
                    &position,
                    head.runtime_provider.as_deref(),
                    None,
                    actor,
                )?;
            }
            let department = tx.query_row(
                &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                [&id],
                dept_row,
            )?;
            Ok((department, position))
        })
    }

    /// Rename a department, change its description, or mark it active or inactive.
    pub fn update_department_details(
        &self,
        id: &str,
        name: &str,
        description: &str,
        active: bool,
        actor: &str,
    ) -> Result<Department> {
        let name = clean_line("the department name", name, 120)?;
        let description = clean_text("the description", description, 2000)?;
        self.write(|tx, out| {
            let n = tx
                .execute(
                    "UPDATE departments SET name = ?2, description = ?3, status = ?4 WHERE id = ?1",
                    params![
                        id,
                        name,
                        description,
                        if active { "active" } else { "inactive" }
                    ],
                )
                .map_err(|e| unique(e, &format!("a department named \"{name}\" already exists")))?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("department {id}")));
            }
            org_event(
                out,
                tx,
                actor,
                "department_updated",
                json!({ "id": id, "name": name, "active": active }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                [id],
                dept_row,
            )?)
        })
    }

    /// Delete a department that has no projects and whose head has no team: the head position
    /// is archived (its incumbent retired) in the same transaction.
    pub fn remove_department(&self, id: &str, actor: &str) -> Result<()> {
        self.write(|tx, out| {
            let department = tx
                .query_row(
                    &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                    [id],
                    dept_row,
                )
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("department {id}")))?;
            let projects: u32 = tx.query_row(
                "SELECT COUNT(*) FROM projects WHERE department_id = ?1",
                [id],
                |r| r.get(0),
            )?;
            if projects > 0 {
                return Err(invalid(format!(
                    "{} still has {projects} project{} (archived projects count too); mark the \
                     department inactive instead",
                    department.name,
                    if projects == 1 { "" } else { "s" }
                )));
            }
            let org = Org::load(tx)?;
            if let Some(head_id) = &department.head_position_id {
                if let Some(head) = org.positions.get(head_id) {
                    let reports = org.reports(Some(head_id)).count();
                    if reports > 0 {
                        return Err(invalid(format!(
                            "{} leads {reports} position{}; move or archive {} first",
                            head.title,
                            if reports == 1 { "" } else { "s" },
                            if reports == 1 { "it" } else { "them" }
                        )));
                    }
                    refuse_if_busy(tx, head, "removing the department")?;
                    archive(tx, out, &org, head, "department removed", actor)?;
                }
            }
            tx.execute("DELETE FROM departments WHERE id = ?1", [id])?;
            org_event(
                out,
                tx,
                actor,
                "department_deleted",
                json!({ "id": id, "name": department.name }),
            )
        })
    }

    // ---- Projects -----------------------------------------------------------------------------

    /// Create a project in `department_id` with its coordinator position, which reports to the
    /// department's head (hired now when `coordinator.staffed`).
    pub fn create_project_with_coordinator(
        &self,
        department_id: &str,
        settings: &ProjectSettings,
        coordinator: &NewPosition,
        actor: &str,
    ) -> Result<(Project, Position)> {
        let settings = clean_settings(settings)?;
        let coordinator = clean_position(coordinator)?;
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let department = tx
                .query_row(
                    &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                    [department_id],
                    dept_row,
                )
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("department {department_id}")))?;
            if department.status != "active" {
                return Err(invalid(format!("{} is inactive", department.name)));
            }
            let head = department
                .head_position_id
                .as_deref()
                .and_then(|h| org.positions.get(h))
                .ok_or_else(|| invalid(format!("{} has no head position", department.name)))?;
            let role = org.role(&coordinator.role_id)?;
            if role.role_type != RoleType::ProjectCoordinator || !role.persistent {
                return Err(invalid(format!(
                    "{} cannot lead a project: choose a supervisor role",
                    role.name
                )));
            }
            org.check_title(&coordinator.title, Some(&head.id), None)?;
            if let Some(runtime) = coordinator
                .runtime_id
                .as_ref()
                .filter(|r| !settings.allowed_runtimes.contains(r))
            {
                return Err(invalid(format!(
                    "the supervisor's AI tool ({runtime}) must be one of the project's allowed AI \
                     tools"
                )));
            }
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO projects (id, name, local_path, repository_url, department_id,
                     created_at, description, allowed_runtimes, capability_profile, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active')",
                params![
                    id,
                    settings.name,
                    settings.local_path,
                    settings.repository_url,
                    department.id,
                    now(),
                    settings.description,
                    serde_json::to_string(&settings.allowed_runtimes)?,
                    settings.capability_profile
                ],
            )
            .map_err(|e| {
                unique(
                    e,
                    &format!("a project named \"{}\" already exists", settings.name),
                )
            })?;
            let position = insert_position(tx, out, &coordinator, Some(&head.id), actor)?;
            tx.execute(
                "UPDATE projects SET coordinator_position_id = ?2 WHERE id = ?1",
                params![id, position.id],
            )?;
            org_event(
                out,
                tx,
                actor,
                "project_created",
                json!({
                    "id": id,
                    "name": settings.name,
                    "departmentId": department.id,
                    "coordinatorPositionId": position.id,
                    "allowedRuntimes": settings.allowed_runtimes,
                    "capabilityProfile": settings.capability_profile,
                }),
            )?;
            if coordinator.staffed {
                hire(
                    tx,
                    out,
                    &position,
                    coordinator.runtime_provider.as_deref(),
                    Some(&id),
                    actor,
                )?;
            }
            let project = tx.query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                [&id],
                project_row,
            )?;
            Ok((project, position))
        })
    }

    /// Change a project's name, description, repository, folder, allowed runtimes, or
    /// capability profile. Positions on a runtime the project no longer allows stay, but can no
    /// longer take work until their runtime is changed.
    pub fn update_project_settings(
        &self,
        id: &str,
        settings: &ProjectSettings,
        actor: &str,
    ) -> Result<Project> {
        let settings = clean_settings(settings)?;
        self.write(|tx, out| {
            let status: Option<String> = tx
                .query_row("SELECT status FROM projects WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .optional()?;
            match status.as_deref() {
                None => return Err(LedgerError::NotFound(format!("project {id}"))),
                Some("archived") => return Err(invalid("an archived project cannot change")),
                _ => {}
            }
            tx.execute(
                "UPDATE projects SET name = ?2, description = ?3, repository_url = ?4,
                     local_path = ?5, allowed_runtimes = ?6, capability_profile = ?7
                 WHERE id = ?1",
                params![
                    id,
                    settings.name,
                    settings.description,
                    settings.repository_url,
                    settings.local_path,
                    serde_json::to_string(&settings.allowed_runtimes)?,
                    settings.capability_profile
                ],
            )
            .map_err(|e| {
                unique(
                    e,
                    &format!("a project named \"{}\" already exists", settings.name),
                )
            })?;
            org_event(
                out,
                tx,
                actor,
                "project_updated",
                json!({
                    "id": id,
                    "name": settings.name,
                    "allowedRuntimes": settings.allowed_runtimes,
                    "capabilityProfile": settings.capability_profile,
                }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                [id],
                project_row,
            )?)
        })
    }

    /// Archive a project and its whole team (the coordinator and every position below it),
    /// once none of them has unfinished work.
    pub fn archive_project(&self, id: &str, actor: &str) -> Result<Project> {
        self.write(|tx, out| {
            let project = tx
                .query_row(
                    &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                    [id],
                    project_row,
                )
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("project {id}")))?;
            if project.status == "archived" {
                return Ok(project);
            }
            let org = Org::load(tx)?;
            let team: Vec<String> = project
                .coordinator_position_id
                .as_deref()
                .filter(|c| org.positions.contains_key(*c))
                .map(|c| org.subtree(c))
                .unwrap_or_default();
            for pid in &team {
                if let Some(p) = org.positions.get(pid) {
                    refuse_if_busy(tx, p, "archiving the project")?;
                }
            }
            // Deepest first, so no position is ever left reporting to an archived one.
            for pid in team.iter().rev() {
                if let Some(p) = org.positions.get(pid) {
                    archive(tx, out, &org, p, "project archived", actor)?;
                }
            }
            tx.execute(
                "UPDATE projects SET status = 'archived' WHERE id = ?1",
                [id],
            )?;
            org_event(
                out,
                tx,
                actor,
                "project_archived",
                json!({ "id": id, "name": project.name, "positions": team }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                [id],
                project_row,
            )?)
        })
    }

    // ---- Positions ----------------------------------------------------------------------------

    /// Hire into a team: a new position reporting to `new.reports_to` (a worker role, or a
    /// superintendent). Department heads come with their department and coordinators with
    /// their project.
    pub fn create_position(
        &self,
        new: &NewPosition,
        actor: &str,
    ) -> Result<(Position, Option<AgentInstance>)> {
        let new = clean_position(new)?;
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let role = org.role(&new.role_id)?;
            match role.role_type {
                RoleType::DepartmentManager => {
                    return Err(invalid(
                        "a manager comes with a department: create a department instead",
                    ))
                }
                RoleType::ProjectCoordinator => {
                    return Err(invalid(
                        "a supervisor comes with a project: create a project instead",
                    ))
                }
                RoleType::Superintendent | RoleType::Worker => {}
            }
            org.check_supervisor(tx, role, None, new.reports_to.as_deref())?;
            org.check_title(&new.title, new.reports_to.as_deref(), None)?;
            let project = new.reports_to.as_deref().and_then(|l| org.project_of(l));
            Org::check_runtime(project, new.runtime_id.as_deref(), &new.title)?;
            let position = insert_position(tx, out, &new, new.reports_to.as_deref(), actor)?;
            let agent = if role.persistent && new.staffed {
                Some(hire(
                    tx,
                    out,
                    &position,
                    new.runtime_provider.as_deref(),
                    project.map(|p| p.id.as_str()),
                    actor,
                )?)
            } else {
                None
            };
            Ok((position, agent))
        })
    }

    /// Hire an incumbent into a vacant persistent position.
    pub fn fill_position(
        &self,
        id: &str,
        runtime_provider: Option<&str>,
        actor: &str,
    ) -> Result<AgentInstance> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?;
            if !org.role_of(position)?.persistent {
                return Err(invalid(format!(
                    "{} is an on-call position: it gets a new worker for every task",
                    position.title
                )));
            }
            if incumbent(tx, id)?.is_some() {
                return Err(invalid(format!("{} is already staffed", position.title)));
            }
            let project = org.project_of(id);
            Org::check_runtime(project, position.runtime_id.as_deref(), &position.title)?;
            hire(
                tx,
                out,
                position,
                runtime_provider,
                project.map(|p| p.id.as_str()),
                actor,
            )
        })
    }

    /// Retire the incumbent of a persistent position, leaving it vacant.
    pub fn vacate_position(&self, id: &str, actor: &str) -> Result<AgentInstance> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?;
            refuse_if_busy(tx, position, "letting its agent go")?;
            retire_incumbent(tx, out, position, "position vacated", actor)?
                .ok_or_else(|| invalid(format!("{} is already vacant", position.title)))
        })
    }

    /// Rename a position, or change its runtime or model. A staffed persistent position gets a
    /// new incumbent for a new runtime or model (its conversation starts over).
    pub fn update_position(
        &self,
        id: &str,
        patch: &PositionPatch,
        actor: &str,
    ) -> Result<(Position, Option<AgentInstance>)> {
        let title = patch
            .title
            .as_deref()
            .map(|t| clean_line("the title", t, 80))
            .transpose()?;
        let runtime = patch
            .runtime
            .as_ref()
            .map(|r| -> Result<Option<(String, Option<String>)>> {
                r.as_ref()
                    .map(|(r, p)| -> Result<(String, Option<String>)> {
                        Ok((
                            clean_runtime_id(r)?,
                            p.as_deref()
                                .map(|p| clean_line("the provider", p, 64))
                                .transpose()?,
                        ))
                    })
                    .transpose()
            })
            .transpose()?;
        let model = patch
            .model
            .as_ref()
            .map(|m| clean_optional_model(m.as_deref()))
            .transpose()?;
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?.clone();
            let role = org.role_of(&position)?.clone();
            let mut changes = serde_json::Map::new();
            if let Some(title) = &title {
                if *title != position.title {
                    org.check_title(title, position.reports_to.as_deref(), Some(id))?;
                    for o in org.oversight.iter().filter(|o| o.overseer_id == id) {
                        org.check_title(title, Some(&o.target_id), Some(id))?;
                    }
                    changes.insert("title".into(), json!(title));
                }
            }
            let new_runtime = runtime
                .as_ref()
                .map(|r| r.as_ref().map(|(r, _)| r.clone()))
                .filter(|r| *r != position.runtime_id);
            let runtime_after = new_runtime
                .clone()
                .unwrap_or_else(|| position.runtime_id.clone());
            let mut new_model = model.clone().filter(|m| *m != position.model);
            if runtime_after.is_none() {
                // An automatic position has no model of its own.
                fixed_model(None, new_model.clone().flatten().as_deref())?;
                new_model = position.model.is_some().then_some(None);
            }
            if let Some(r) = &new_runtime {
                Org::check_runtime(org.project_of(id), r.as_deref(), &position.title)?;
                changes.insert("runtimeId".into(), json!(r));
                changes.insert("automatic".into(), json!(r.is_none()));
            }
            if let Some(m) = &new_model {
                changes.insert("model".into(), json!(m));
            }
            if changes.is_empty() {
                return Ok((position, None));
            }
            let replace = role.persistent
                && (new_runtime.is_some() || new_model.is_some())
                && incumbent(tx, id)?.is_some();
            if replace {
                refuse_if_busy(tx, &position, "changing its AI tool or model")?;
            }
            tx.execute(
                "UPDATE positions SET title = ?2, runtime_id = ?3, model = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![
                    id,
                    title.as_ref().unwrap_or(&position.title),
                    runtime_after.as_deref().unwrap_or(AUTOMATIC),
                    new_model.as_ref().unwrap_or(&position.model),
                    now()
                ],
            )
            .map_err(|e| unique(e, "that team already has a member with this title"))?;
            changes.insert("positionId".into(), json!(id));
            org_event(out, tx, actor, "position_updated", Value::Object(changes))?;
            let updated = get_position(tx, id)?;
            let agent = if replace {
                retire_incumbent(tx, out, &updated, "AI tool or model changed", actor)?;
                let provider = runtime.as_ref().and_then(|r| r.as_ref()?.1.as_deref());
                Some(hire(
                    tx,
                    out,
                    &updated,
                    provider,
                    org.project_of(id).map(|p| p.id.as_str()),
                    actor,
                )?)
            } else {
                None
            };
            Ok((updated, agent))
        })
    }

    /// Make a position report to `to` (`None`: the owner). Moving a coordinator under another
    /// department's head moves its project to that department, in the same transaction.
    pub fn move_position(&self, id: &str, to: Option<&str>, actor: &str) -> Result<Position> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?.clone();
            if position.reports_to.as_deref() == to {
                return Ok(position);
            }
            let role = org.role_of(&position)?;
            org.check_supervisor(tx, role, Some(id), to)?;
            org.check_title(&position.title, to, Some(id))?;
            if let Some(to) = to {
                if let Some(o) = org
                    .oversight
                    .iter()
                    .find(|o| o.overseer_id == id && o.target_id == to)
                {
                    return Err(invalid(format!(
                        "{} is already assigned to that team as its {}; end that assignment first",
                        position.title,
                        o.kind.label()
                    )));
                }
            }
            // Check every moved position's runtime against the project it lands in.
            let mut moved = org.clone();
            if let Some(p) = moved.positions.get_mut(id) {
                p.reports_to = to.map(str::to_owned);
            }
            for pid in moved.subtree(id) {
                if let Some(p) = moved.positions.get(&pid) {
                    Org::check_runtime(moved.project_of(&pid), p.runtime_id.as_deref(), &p.title)?;
                }
            }
            tx.execute(
                "UPDATE positions SET reports_to = ?2, sort_key = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, to, next_sort_key(tx, to)?, now()],
            )
            .map_err(|e| unique(e, "that team already has a member with this title"))?;
            org_event(
                out,
                tx,
                actor,
                "position_moved",
                json!({
                    "positionId": id,
                    "title": position.title,
                    "from": position.reports_to,
                    "to": to,
                }),
            )?;
            if let Some(project) = org.coordinators.get(id) {
                let department = to.and_then(|t| moved.department_of(t)).map(|d| d.id.clone());
                if department != project.department_id {
                    tx.execute(
                        "UPDATE projects SET department_id = ?2 WHERE id = ?1",
                        params![project.id, department],
                    )?;
                    org_event(
                        out,
                        tx,
                        actor,
                        "project_reassigned",
                        json!({
                            "id": project.id,
                            "name": project.name,
                            "from": project.department_id,
                            "to": department,
                        }),
                    )?;
                }
            }
            get_position(tx, id)
        })
    }

    /// Archive a position (orphan prevention: it must not lead anyone, head a department,
    /// coordinate an active project, or have unfinished work). Its incumbent is retired and its
    /// oversight assignments end, in the same transaction.
    pub fn archive_position(&self, id: &str, actor: &str) -> Result<Position> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?;
            if let Some(d) = org.heads.get(id) {
                return Err(invalid(format!(
                    "{} heads {}; remove the department instead, or give the position a new agent",
                    position.title, d.name
                )));
            }
            if let Some(p) = org.coordinators.get(id) {
                if p.status == "active" {
                    return Err(invalid(format!(
                        "{} coordinates {}; archive the project instead, or give the position a \
                         new agent",
                        position.title, p.name
                    )));
                }
            }
            let reports: Vec<&str> = org.reports(Some(id)).map(|p| p.title.as_str()).collect();
            if !reports.is_empty() {
                return Err(invalid(format!(
                    "{} leads {} ({}); move or archive them first so no one is left without a \
                     supervisor",
                    position.title,
                    if reports.len() == 1 {
                        "1 position".to_owned()
                    } else {
                        format!("{} positions", reports.len())
                    },
                    reports.join(", ")
                )));
            }
            refuse_if_busy(tx, position, "archiving it")?;
            archive(tx, out, &org, position, "position archived", actor)?;
            get_position(tx, id)
        })
    }

    // ---- Oversight ----------------------------------------------------------------------------

    /// Assign `overseer` (an on-demand position) as the reviewer, QA evaluator, or security
    /// auditor of the team led by `target` (a persistent position).
    pub fn assign_oversight(
        &self,
        kind: OversightKind,
        overseer_id: &str,
        target_id: &str,
        actor: &str,
    ) -> Result<Oversight> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let overseer = org.position(tx, overseer_id)?;
            let target = org.position(tx, target_id)?;
            if overseer_id == target_id {
                return Err(invalid("a position cannot oversee its own team"));
            }
            if org.role_of(overseer)?.persistent {
                return Err(invalid(format!(
                    "{} is a full-time position; for now only on-call positions can be assigned \
                     to review, QA, or audit a team",
                    overseer.title
                )));
            }
            if !org.role_of(target)?.persistent {
                return Err(invalid(format!(
                    "{} is an on-call position and has no team to oversee",
                    target.title
                )));
            }
            if overseer.reports_to.as_deref() == Some(target_id) {
                return Err(invalid(format!(
                    "{} is already on {}'s team",
                    overseer.title, target.title
                )));
            }
            if org
                .oversight
                .iter()
                .any(|o| o.kind == kind && o.overseer_id == overseer_id && o.target_id == target_id)
            {
                return Err(invalid(format!(
                    "{} is already the {} for {}'s team",
                    overseer.title,
                    kind.label(),
                    target.title
                )));
            }
            org.check_title(&overseer.title, Some(target_id), Some(overseer_id))?;
            Org::check_runtime(
                org.project_of(target_id),
                overseer.runtime_id.as_deref(),
                &overseer.title,
            )?;
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO oversight (id, kind, overseer_id, target_id, state, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
                params![id, kind.as_str(), overseer_id, target_id, now()],
            )?;
            org_event(
                out,
                tx,
                actor,
                "oversight_assigned",
                json!({
                    "oversightId": id,
                    "kind": kind,
                    "overseerId": overseer_id,
                    "targetId": target_id,
                    "overseer": overseer.title,
                    "target": target.title,
                }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {OVERSIGHT_COLS} FROM oversight WHERE id = ?1"),
                [&id],
                oversight_row,
            )?)
        })
    }

    pub fn end_oversight(&self, id: &str, actor: &str) -> Result<Oversight> {
        self.write(|tx, out| {
            let o = tx
                .query_row(
                    &format!("SELECT {OVERSIGHT_COLS} FROM oversight WHERE id = ?1"),
                    [id],
                    oversight_row,
                )
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("oversight assignment {id}")))?;
            if !o.active {
                return Ok(o);
            }
            end_oversight_row(tx, out, &o, "assignment ended", actor)?;
            Ok(tx.query_row(
                &format!("SELECT {OVERSIGHT_COLS} FROM oversight WHERE id = ?1"),
                [id],
                oversight_row,
            )?)
        })
    }

    // ---- Routing (Phase 6, ADR-011) -------------------------------------------------------------

    /// Record the runtime and model an automatic position's agent starts its conversation on,
    /// with the Router's reasons (`org.agent_routed`). The agent must be the active incumbent of
    /// an active, automatic position.
    pub fn route_agent(
        &self,
        agent_id: &str,
        route: &AgentRoute,
        actor: &str,
    ) -> Result<AgentInstance> {
        let runtime_id = clean_runtime_id(&route.runtime_id)?;
        let model = clean_optional_model(route.model.as_deref())?;
        let provider = route
            .runtime_provider
            .as_deref()
            .map(|p| clean_line("the provider", p, 64))
            .transpose()?;
        self.write(|tx, out| {
            let agent = tx
                .query_row(
                    &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
                    [agent_id],
                    agent_row,
                )
                .optional()?
                .ok_or_else(|| LedgerError::NotFound(format!("agent {agent_id}")))?;
            let position_id = agent
                .position_id
                .as_deref()
                .filter(|_| agent.task_id.is_none() && agent.is_active())
                .ok_or_else(|| invalid("only a position's current agent can be routed"))?;
            let position = get_position(tx, position_id)?;
            if position.state != PositionState::Active || position.runtime_id.is_some() {
                return Err(invalid(format!(
                    "{} does not choose its AI tool automatically",
                    position.title
                )));
            }
            tx.execute(
                "UPDATE agent_instances SET runtime_id = ?2, runtime_provider = ?3, model = ?4,
                     last_seen_at = ?5
                 WHERE id = ?1",
                params![agent_id, runtime_id, provider, model, now()],
            )?;
            org_event(
                out,
                tx,
                actor,
                "agent_routed",
                json!({
                    "agentId": agent_id,
                    "positionId": position.id,
                    "title": position.title,
                    "runtimeId": runtime_id,
                    "model": model,
                    "routing": route.routing,
                }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
                [agent_id],
                agent_row,
            )?)
        })
    }

    /// Replace the setting `key` with what `change` makes of its current value (`Null` when
    /// unset), in one transaction, and record `event_type` with the payload `change` returns.
    /// `change` refusing leaves the setting as it was. Returns the new value.
    pub fn update_setting(
        &self,
        key: &str,
        event_type: &str,
        actor: &str,
        change: impl FnOnce(Value) -> Result<(Value, Value)>,
    ) -> Result<Value> {
        let key = clean_line("the setting key", key, 64)?;
        self.write(|tx, out| {
            let current = tx
                .query_row("SELECT value FROM settings WHERE key = ?1", [&key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
                .map_or(Value::Null, parse_json);
            let (value, payload) = change(current)?;
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value.to_string(), now()],
            )?;
            out.push(events::insert(
                tx,
                NewEvent {
                    source: actor.into(),
                    event_type: event_type.into(),
                    payload,
                    ..NewEvent::default()
                },
            )?);
            Ok(value)
        })
    }

    /// Agent turn results that hit a usage limit or completed, for runs started at or after
    /// `since` (ms), newest first.
    pub fn recent_turn_outcomes(&self, since: u64) -> Result<Vec<TurnOutcomeRecord>> {
        self.read(|c| {
            all(
                c,
                "SELECT x.runtime, x.model, json_extract(e.payload, '$.outcome'),
                        json_extract(e.payload, '$.error'), json_extract(e.payload, '$.summary'),
                        e.created_at
                 FROM executions x JOIN events e ON e.execution_id = x.id
                 WHERE x.started_at >= ?1 AND e.event_type = 'agent.result'
                   AND json_extract(e.payload, '$.outcome') IN ('usageLimited', 'completed')
                 ORDER BY e.seq DESC",
                [i64::try_from(since).unwrap_or(i64::MAX)],
                |r| {
                    Ok(TurnOutcomeRecord {
                        runtime: r.get(0)?,
                        model: r.get(1)?,
                        outcome: r.get(2)?,
                        error: r.get(3)?,
                        summary: r.get(4)?,
                        at: u64_of(r.get(5)?),
                    })
                },
            )
        })
    }

    /// Models runtimes ran (as reported by the provider, or as asked), most recently used
    /// first.
    pub fn models_seen(&self, limit: u32) -> Result<Vec<SeenModel>> {
        self.read(|c| {
            all(
                c,
                "SELECT runtime, model, COUNT(*), MAX(started_at) FROM executions
                 WHERE model IS NOT NULL AND model != ''
                 GROUP BY runtime, model ORDER BY MAX(started_at) DESC LIMIT ?1",
                [limit.clamp(1, 500)],
                |r| {
                    Ok(SeenModel {
                        runtime: r.get(0)?,
                        model: r.get(1)?,
                        runs: r.get(2)?,
                        last_used: u64_of(r.get(3)?),
                    })
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn template(name: &'static str, role_type: RoleType, persistent: bool) -> RoleTemplate {
        RoleTemplate {
            name,
            description: "",
            role_type,
            persistent,
            metadata: Value::Null,
            formerly: &[],
        }
    }

    /// A ledger with the roles the tests use, by name.
    fn setup() -> (Ledger, HashMap<&'static str, String>) {
        let l = ledger();
        let roles = l
            .ensure_roles(
                &[
                    template("Superintendent", RoleType::Superintendent, true),
                    template("Department Manager", RoleType::DepartmentManager, true),
                    template("Project Coordinator", RoleType::ProjectCoordinator, true),
                    template("Senior Developer", RoleType::Worker, false),
                    template("QA Engineer", RoleType::Worker, false),
                    template("Staff Engineer", RoleType::Worker, true),
                ],
                "plenipo",
            )
            .unwrap();
        let ids = [
            "Superintendent",
            "Department Manager",
            "Project Coordinator",
            "Senior Developer",
            "QA Engineer",
            "Staff Engineer",
        ]
        .into_iter()
        .map(|n| (n, roles.iter().find(|r| r.name == n).unwrap().id.clone()))
        .collect();
        (l, ids)
    }

    fn position(role: &str, title: &str, reports_to: Option<&str>, runtime: &str) -> NewPosition {
        NewPosition {
            title: title.into(),
            role_id: role.into(),
            reports_to: reports_to.map(str::to_owned),
            runtime_id: Some(runtime.into()),
            runtime_provider: Some("provider".into()),
            model: None,
            staffed: true,
        }
    }

    fn settings(name: &str, runtimes: &[&str]) -> ProjectSettings {
        ProjectSettings {
            name: name.into(),
            description: "A project".into(),
            repository_url: Some("https://github.com/example/cloudline".into()),
            local_path: Some("D:\\projects\\cloudline".into()),
            allowed_runtimes: runtimes.iter().map(|r| (*r).to_owned()).collect(),
            capability_profile: Some("development".into()),
        }
    }

    /// Development (headed by a staffed department manager) with project Cloudline and its
    /// staffed coordinator. Returns (department, head, project, coordinator).
    fn development(
        l: &Ledger,
        roles: &HashMap<&'static str, String>,
    ) -> (Department, Position, Project, Position) {
        let (dept, head) = l
            .create_department_with_head(
                "Development",
                "Builds things",
                &position(
                    roles["Department Manager"].as_str(),
                    "Development Manager",
                    None,
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        let (project, coordinator) = l
            .create_project_with_coordinator(
                &dept.id,
                &settings("Cloudline", &["claude-code", "codex"]),
                &position(
                    roles["Project Coordinator"].as_str(),
                    "Cloudline Coordinator",
                    None,
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        (dept, head, project, coordinator)
    }

    fn types(l: &Ledger) -> Vec<String> {
        l.recent_events(500)
            .unwrap()
            .into_iter()
            .rev()
            .map(|e| e.event_type)
            .collect()
    }

    fn err(r: Result<impl std::fmt::Debug>) -> String {
        match r {
            Err(LedgerError::InvalidInput(m)) => m,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_department_comes_with_its_head_and_a_project_with_its_coordinator() {
        let (l, roles) = setup();
        let (dept, head, project, coordinator) = development(&l, &roles);
        assert_eq!(dept.head_position_id.as_deref(), Some(head.id.as_str()));
        assert_eq!(
            dept.manager_role_id.as_deref(),
            Some(roles["Department Manager"].as_str())
        );
        assert_eq!(head.reports_to, None, "the head reports to the owner");
        assert_eq!(project.department_id.as_deref(), Some(dept.id.as_str()));
        assert_eq!(
            project.coordinator_position_id.as_deref(),
            Some(coordinator.id.as_str())
        );
        assert_eq!(coordinator.reports_to.as_deref(), Some(head.id.as_str()));
        assert_eq!(project.allowed_runtimes, ["claude-code", "codex"]);
        assert_eq!(project.capability_profile.as_deref(), Some("development"));
        assert_eq!(
            project.local_path.as_deref(),
            Some("D:\\projects\\cloudline")
        );
        // Both are staffed: one active incumbent each.
        for p in [&head, &coordinator] {
            let agent = l.position_incumbent(&p.id).unwrap().unwrap();
            assert_eq!(agent.lifecycle_state, AgentLifecycle::Active);
            assert_eq!(agent.runtime_id.as_deref(), Some("claude-code"));
            assert_eq!(agent.task_id, None);
        }
        let coordinator_agent = l.position_incumbent(&coordinator.id).unwrap().unwrap();
        assert_eq!(
            coordinator_agent.project_id.as_deref(),
            Some(project.id.as_str())
        );
        let t = types(&l);
        for expected in [
            "org.department_created",
            "org.project_created",
            "org.position_created",
            "org.agent_hired",
        ] {
            assert!(t.iter().any(|x| x == expected), "{expected} in {t:?}");
        }

        // Names are unique; heads and coordinators need the right kind of role.
        let dup = l.create_department_with_head(
            "Development",
            "",
            &position(
                &roles["Department Manager"],
                "Another Manager",
                None,
                "codex",
            ),
            "owner",
        );
        assert!(err(dup).contains("already exists"));
        let worker_head = l.create_department_with_head(
            "Operations",
            "",
            &position(&roles["Senior Developer"], "Ops Lead", None, "codex"),
            "owner",
        );
        assert!(err(worker_head).contains("cannot run a department"));
        let not_allowed = l.create_project_with_coordinator(
            &dept.id,
            &settings("Milepost", &["claude-code"]),
            &position(
                &roles["Project Coordinator"],
                "Milepost Coordinator",
                None,
                "codex",
            ),
            "owner",
        );
        assert!(err(not_allowed).contains("allowed AI tools"));
        let bad_path = l.create_project_with_coordinator(
            &dept.id,
            &ProjectSettings {
                local_path: Some("relative/path".into()),
                ..settings("Waypoint", &["codex"])
            },
            &position(
                &roles["Project Coordinator"],
                "Waypoint Coordinator",
                None,
                "codex",
            ),
            "owner",
        );
        assert!(err(bad_path).contains("absolute path"));
        let bad_repo = l.create_project_with_coordinator(
            &dept.id,
            &ProjectSettings {
                repository_url: Some("file:///etc/passwd".into()),
                ..settings("Waypoint", &["codex"])
            },
            &position(
                &roles["Project Coordinator"],
                "Waypoint Coordinator",
                None,
                "codex",
            ),
            "owner",
        );
        assert!(err(bad_repo).contains("repository"));
        // A vacant head: the department exists before anyone is hired.
        let (ops, ops_head) = l
            .create_department_with_head(
                "Operations",
                "",
                &NewPosition {
                    staffed: false,
                    ..position(
                        &roles["Department Manager"],
                        "Operations Manager",
                        None,
                        "codex",
                    )
                },
                "owner",
            )
            .unwrap();
        assert_eq!(ops.head_position_id.as_deref(), Some(ops_head.id.as_str()));
        assert!(l.position_incumbent(&ops_head.id).unwrap().is_none());
        // Assign a manager: hire into the vacant head position.
        let hired = l
            .fill_position(&ops_head.id, Some("openai"), "owner")
            .unwrap();
        assert_eq!(hired.position_id.as_deref(), Some(ops_head.id.as_str()));
        assert!(err(l.fill_position(&ops_head.id, None, "owner")).contains("already staffed"));
    }

    #[test]
    fn hiring_into_a_team_follows_the_structure_rules() {
        let (l, roles) = setup();
        let (_, head, _, coordinator) = development(&l, &roles);
        let dev = |title: &str, to: Option<&str>| {
            l.create_position(
                &position(&roles["Senior Developer"], title, to, "codex"),
                "owner",
            )
        };
        assert!(err(dev("Loose Developer", None)).contains("not directly to you"));
        let (developer, agent) = dev("Senior Developer", Some(&coordinator.id)).unwrap();
        assert!(agent.is_none(), "on-demand positions have no incumbent");
        assert!(err(dev("Helper", Some(&developer.id))).contains("only full-time positions lead"));
        assert!(err(dev("senior developer", Some(&coordinator.id)))
            .contains("already has a team member"));
        // Other teams may reuse a title.
        dev("Senior Developer", Some(&head.id)).unwrap();
        let manager = l.create_position(
            &position(
                &roles["Department Manager"],
                "Floating Manager",
                None,
                "codex",
            ),
            "owner",
        );
        assert!(err(manager).contains("create a department instead"));
        let coord = l.create_position(
            &position(
                &roles["Project Coordinator"],
                "Floating Coordinator",
                Some(&head.id),
                "codex",
            ),
            "owner",
        );
        assert!(err(coord).contains("create a project instead"));
        // A superintendent may sit at the top without a department.
        let (chief, chief_agent) = l
            .create_position(
                &position(&roles["Superintendent"], "Chief", None, "claude-code"),
                "owner",
            )
            .unwrap();
        assert!(chief_agent.is_some());
        assert_eq!(chief.reports_to, None);
        // The project's allowed runtimes bind everyone in it.
        l.update_project_settings(
            &l.org_records().unwrap().projects[0].id,
            &settings("Cloudline", &["claude-code"]),
            "owner",
        )
        .unwrap();
        assert!(err(dev("Codex Developer", Some(&coordinator.id)))
            .contains("does not allow the codex AI tool"));
        // Bad input never reaches the database.
        for bad in [
            NewPosition {
                title: " ".into(),
                ..position(
                    &roles["QA Engineer"],
                    "x",
                    Some(&coordinator.id),
                    "claude-code",
                )
            },
            NewPosition {
                title: "two\nlines".into(),
                ..position(
                    &roles["QA Engineer"],
                    "x",
                    Some(&coordinator.id),
                    "claude-code",
                )
            },
            position(
                &roles["QA Engineer"],
                "QA",
                Some(&coordinator.id),
                "Claude Code",
            ),
            NewPosition {
                model: Some("--yolo".into()),
                ..position(
                    &roles["QA Engineer"],
                    "QA",
                    Some(&coordinator.id),
                    "claude-code",
                )
            },
        ] {
            assert!(l.create_position(&bad, "owner").is_err(), "{bad:?}");
        }
    }

    #[test]
    fn moves_prevent_cycles_and_reassign_projects() {
        let (l, roles) = setup();
        let (chief, _) = l
            .create_position(
                &position(&roles["Superintendent"], "Chief", None, "claude-code"),
                "owner",
            )
            .unwrap();
        let (dev, dev_head) = l
            .create_department_with_head(
                "Development",
                "",
                &position(
                    &roles["Department Manager"],
                    "Development Manager",
                    Some(&chief.id),
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        let (ops, ops_head) = l
            .create_department_with_head(
                "Operations",
                "",
                &position(
                    &roles["Department Manager"],
                    "Operations Manager",
                    Some(&chief.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        let (project, coordinator) = l
            .create_project_with_coordinator(
                &dev.id,
                &settings("Cloudline", &["claude-code", "codex"]),
                &position(
                    &roles["Project Coordinator"],
                    "Cloudline Coordinator",
                    None,
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        let (developer, _) = l
            .create_position(
                &position(
                    &roles["Senior Developer"],
                    "Senior Developer",
                    Some(&coordinator.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();

        // A manager reports to the owner or a VP, and never below itself.
        assert!(
            err(l.move_position(&ops_head.id, Some(&dev_head.id), "owner")).contains("to a VP")
        );
        assert!(err(l.move_position(&chief.id, Some(&dev_head.id), "owner"))
            .contains("cannot report to it"));
        assert!(l
            .move_position(&chief.id, Some(&chief.id), "owner")
            .is_err());
        assert!(
            err(l.move_position(&coordinator.id, Some(&chief.id), "owner"))
                .contains("a department's manager")
        );
        assert!(
            err(l.move_position(&coordinator.id, Some(&developer.id), "owner")).contains("on-call")
        );
        assert!(err(l.move_position(&developer.id, None, "owner")).contains("not directly to you"));

        // Department/project reassignment: the coordinator moves to Operations' head.
        let moved = l
            .move_position(&coordinator.id, Some(&ops_head.id), "owner")
            .unwrap();
        assert_eq!(moved.reports_to.as_deref(), Some(ops_head.id.as_str()));
        let records = l.org_records().unwrap();
        let project = records
            .projects
            .iter()
            .find(|p| p.id == project.id)
            .unwrap();
        assert_eq!(project.department_id.as_deref(), Some(ops.id.as_str()));
        let reassigned = l
            .recent_events(50)
            .unwrap()
            .into_iter()
            .find(|e| e.event_type == "org.project_reassigned")
            .unwrap();
        assert_eq!(reassigned.payload["from"], json!(dev.id));
        assert_eq!(reassigned.payload["to"], json!(ops.id));
        // Moving again to the same place changes nothing.
        let before = l.recent_events(1).unwrap()[0].seq;
        l.move_position(&coordinator.id, Some(&ops_head.id), "owner")
            .unwrap();
        assert_eq!(l.recent_events(1).unwrap()[0].seq, before);

        // A worker moves between teams; its runtime must be allowed where it lands.
        l.move_position(&developer.id, Some(&dev_head.id), "owner")
            .unwrap();
        l.update_project_settings(
            &project.id,
            &settings("Cloudline", &["claude-code"]),
            "owner",
        )
        .unwrap();
        assert!(
            err(l.move_position(&developer.id, Some(&coordinator.id), "owner"))
                .contains("does not allow")
        );
    }

    #[test]
    fn orphans_are_prevented() {
        let (l, roles) = setup();
        let (dept, head, project, coordinator) = development(&l, &roles);
        let (developer, _) = l
            .create_position(
                &position(
                    &roles["Senior Developer"],
                    "Senior Developer",
                    Some(&coordinator.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        assert!(err(l.archive_position(&head.id, "owner")).contains("heads Development"));
        assert!(err(l.archive_position(&coordinator.id, "owner"))
            .contains("archive the project instead"));
        let (staff, _) = l
            .create_position(
                &position(
                    &roles["Staff Engineer"],
                    "Staff Engineer",
                    Some(&head.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        let (intern, _) = l
            .create_position(
                &position(
                    &roles["Senior Developer"],
                    "Junior Developer",
                    Some(&staff.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        let refused = err(l.archive_position(&staff.id, "owner"));
        assert!(refused.contains("Junior Developer") && refused.contains("without a supervisor"));
        assert!(err(l.remove_department(&dept.id, "owner")).contains("project"));

        // Unfinished work blocks archiving and replacing.
        let busy = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "work".into(),
                    metadata: json!({ "workforce": { "positionId": intern.id } }),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        assert!(err(l.archive_position(&intern.id, "owner")).contains("unfinished task"));
        l.transition_task(&busy.id, TaskState::Cancelled, "owner", None)
            .unwrap();
        let archived = l.archive_position(&intern.id, "owner").unwrap();
        assert_eq!(archived.state, PositionState::Archived);
        assert!(archived.archived_at.is_some());
        l.archive_position(&staff.id, "owner").unwrap();
        // Archived positions are kept and frozen.
        assert!(err(l.move_position(&staff.id, Some(&head.id), "owner")).contains("archived"));
        {
            let c = l.conn();
            assert!(c
                .execute("DELETE FROM positions WHERE id = ?1", [&staff.id])
                .is_err());
            assert!(c
                .execute(
                    "UPDATE positions SET title = 'x' WHERE id = ?1",
                    [&staff.id]
                )
                .is_err());
        }

        // Archiving the project archives its whole team, deepest first.
        let archived = l.archive_project(&project.id, "owner").unwrap();
        assert_eq!(archived.status, "archived");
        let records = l.org_records().unwrap();
        for id in [&coordinator.id, &developer.id] {
            let p = records.positions.iter().find(|p| &p.id == id).unwrap();
            assert_eq!(p.state, PositionState::Archived, "{}", p.title);
        }
        assert!(l.position_incumbent(&coordinator.id).unwrap().is_none());
        assert!(err(l.update_project_settings(
            &project.id,
            &settings("Cloudline", &["codex"]),
            "owner"
        ))
        .contains("archived"));
        // An archived project still counts: mark the department inactive instead.
        assert!(err(l.remove_department(&dept.id, "owner")).contains("inactive instead"));
        let inactive = l
            .update_department_details(&dept.id, "Development", "", false, "owner")
            .unwrap();
        assert_eq!(inactive.status, "inactive");

        // A department without projects can be removed; its head is archived.
        let (ops, ops_head) = l
            .create_department_with_head(
                "Operations",
                "",
                &position(
                    &roles["Department Manager"],
                    "Operations Manager",
                    None,
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        l.remove_department(&ops.id, "owner").unwrap();
        assert!(l.department(&ops.id).unwrap().is_none());
        assert_eq!(
            l.position(&ops_head.id).unwrap().unwrap().state,
            PositionState::Archived
        );
        let t = types(&l);
        assert!(t.iter().any(|x| x == "org.department_deleted"));
        assert!(t.iter().any(|x| x == "org.project_archived"));
    }

    #[test]
    fn oversight_makes_an_on_demand_position_serve_another_team() {
        let (l, roles) = setup();
        let (_, head, _, coordinator) = development(&l, &roles);
        let (qa, _) = l
            .create_position(
                &position(
                    &roles["QA Engineer"],
                    "QA Engineer",
                    Some(&head.id),
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        let o = l
            .assign_oversight(OversightKind::Qa, &qa.id, &coordinator.id, "owner")
            .unwrap();
        assert!(o.active);
        assert_eq!(
            (o.overseer_id.as_str(), o.target_id.as_str()),
            (qa.id.as_str(), coordinator.id.as_str())
        );
        assert!(
            err(l.assign_oversight(OversightKind::Qa, &qa.id, &coordinator.id, "owner"))
                .contains("already the QA evaluator")
        );
        // The same position may also review the team.
        l.assign_oversight(OversightKind::Review, &qa.id, &coordinator.id, "owner")
            .unwrap();
        assert!(err(l.assign_oversight(
            OversightKind::Security,
            &coordinator.id,
            &head.id,
            "owner"
        ))
        .contains("full-time position"));
        assert!(
            err(l.assign_oversight(OversightKind::Security, &qa.id, &qa.id, "owner"))
                .contains("own team")
        );
        let (dev, _) = l
            .create_position(
                &position(
                    &roles["Senior Developer"],
                    "Senior Developer",
                    Some(&coordinator.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        assert!(
            err(l.assign_oversight(OversightKind::Security, &qa.id, &dev.id, "owner"))
                .contains("no team")
        );
        assert!(err(l.assign_oversight(
            OversightKind::Security,
            &dev.id,
            &coordinator.id,
            "owner"
        ))
        .contains("already on"));
        // Overseers count as team members for titles, and cannot also join the team.
        let clash = l.create_position(
            &position(
                &roles["QA Engineer"],
                "qa engineer",
                Some(&coordinator.id),
                "codex",
            ),
            "owner",
        );
        assert!(err(clash).contains("already has a team member"));
        assert!(err(l.move_position(&qa.id, Some(&coordinator.id), "owner"))
            .contains("end that assignment first"));
        // Ending is recorded once; ended assignments are frozen and kept.
        let ended = l.end_oversight(&o.id, "owner").unwrap();
        assert!(!ended.active && ended.ended_at.is_some());
        let again = l.end_oversight(&o.id, "owner").unwrap();
        assert_eq!(again, ended);
        {
            let c = l.conn();
            assert!(c
                .execute("DELETE FROM oversight WHERE id = ?1", [&o.id])
                .is_err());
            assert!(c
                .execute(
                    "UPDATE oversight SET target_id = 'x' WHERE id = ?1",
                    [&o.id]
                )
                .is_err());
        }
        // Archiving an overseer ends its remaining assignments.
        l.archive_position(&qa.id, "owner").unwrap();
        assert!(l.org_records().unwrap().oversight.is_empty());
        let t = types(&l);
        assert!(t.iter().filter(|x| *x == "org.oversight_ended").count() == 2);
    }

    #[test]
    fn a_spawned_worker_starts_and_retires_with_its_task() {
        let (l, roles) = setup();
        let (_, _, project, coordinator) = development(&l, &roles);
        let (dev, _) = l
            .create_position(
                &position(
                    &roles["Senior Developer"],
                    "Senior Developer",
                    Some(&coordinator.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        let spawn = |outcome: TaskState| {
            let agent_id = uuid::Uuid::new_v4().to_string();
            let task = l
                .create_task(
                    NewTask {
                        requested_by: "agent:claude-code".into(),
                        objective: "Implement it".into(),
                        project_id: Some(project.id.clone()),
                        metadata: json!({ "workforce": { "positionId": dev.id, "agentId": agent_id } }),
                        ..NewTask::default()
                    },
                    "liaison",
                )
                .unwrap();
            let worker = NewWorker {
                agent_id,
                position_id: dev.id.clone(),
                role_id: dev.role_id.clone(),
                runtime_id: "codex".into(),
                runtime_provider: Some("openai".into()),
                model: None,
                project_id: Some(project.id.clone()),
                routing: json!({ "reason": "test" }),
            };
            l.write(|tx, out| insert_worker(tx, out, &worker, &task.id, "liaison"))
                .unwrap();
            let active = |l: &Ledger| {
                l.org_records()
                    .unwrap()
                    .agents
                    .iter()
                    .any(|a| a.id == worker.agent_id)
            };
            let state = |l: &Ledger| {
                l.position_agents(&dev.id, 100)
                    .unwrap()
                    .into_iter()
                    .find(|a| a.id == worker.agent_id)
                    .unwrap()
            };
            assert_eq!(state(&l).lifecycle_state, AgentLifecycle::Starting);
            assert!(active(&l), "a queued worker is in the active workforce");
            if outcome != TaskState::Cancelled {
                l.transition_task(&task.id, TaskState::Running, "agent:codex", None)
                    .unwrap();
                assert_eq!(state(&l).lifecycle_state, AgentLifecycle::Active);
                l.transition_task(&task.id, TaskState::Blocked, "liaison", None)
                    .unwrap();
                assert_eq!(
                    state(&l).lifecycle_state,
                    AgentLifecycle::Active,
                    "waiting is still working"
                );
                l.transition_task(&task.id, TaskState::Running, "liaison", None)
                    .unwrap();
            }
            l.transition_task(&task.id, outcome, "agent:codex", None)
                .unwrap();
            let done = state(&l);
            assert!(!active(&l), "it left the active workforce");
            assert!(done.retired_at.is_some());
            let trail: Vec<String> = l
                .events_for_task(&task.id)
                .unwrap()
                .into_iter()
                .map(|e| e.event_type)
                .filter(|t| t.starts_with("org."))
                .collect();
            (done, trail, task)
        };
        let (done, trail, _) = spawn(TaskState::Succeeded);
        assert_eq!(done.lifecycle_state, AgentLifecycle::Retired);
        assert_eq!(
            trail,
            [
                "org.worker_spawned",
                "org.worker_started",
                "org.worker_retired"
            ]
        );
        let (failed, _, _) = spawn(TaskState::Failed);
        assert_eq!(failed.lifecycle_state, AgentLifecycle::Failed);
        let (cancelled, trail, task) = spawn(TaskState::Cancelled);
        assert_eq!(cancelled.lifecycle_state, AgentLifecycle::Retired);
        assert_eq!(trail, ["org.worker_spawned", "org.worker_retired"]);
        // History remains: every worker, and its task, is still recorded.
        assert_eq!(l.position_agents(&dev.id, 100).unwrap().len(), 3);
        assert_eq!(l.position_tasks(&dev.id, 100).unwrap().len(), 3);
        let former = l.org_records().unwrap().former_agents[&dev.id];
        assert_eq!((former.retired, former.failed), (2, 1));
        // A task must name its worker, and has at most one.
        let stranger = NewWorker {
            agent_id: uuid::Uuid::new_v4().to_string(),
            position_id: dev.id.clone(),
            role_id: dev.role_id.clone(),
            runtime_id: "codex".into(),
            runtime_provider: None,
            model: None,
            project_id: None,
            routing: Value::Null,
        };
        assert!(l
            .write(|tx, out| insert_worker(tx, out, &stranger, &task.id, "liaison"))
            .is_err());
        let again = NewWorker {
            agent_id: task.metadata["workforce"]["agentId"]
                .as_str()
                .unwrap()
                .to_owned(),
            position_id: dev.id.clone(),
            role_id: dev.role_id.clone(),
            runtime_id: "codex".into(),
            runtime_provider: None,
            model: None,
            project_id: None,
            routing: Value::Null,
        };
        assert!(l
            .write(|tx, out| insert_worker(tx, out, &again, &task.id, "liaison"))
            .is_err());
    }

    #[test]
    fn a_persistent_position_keeps_one_incumbent_at_a_time() {
        let (l, roles) = setup();
        let (_, _, _, coordinator) = development(&l, &roles);
        let first = l.position_incumbent(&coordinator.id).unwrap().unwrap();
        // Changing the runtime replaces the incumbent (a session belongs to one runtime).
        let (updated, replacement) = l
            .update_position(
                &coordinator.id,
                &PositionPatch {
                    title: Some("Cloudline Lead".into()),
                    runtime: Some(Some(("codex".into(), Some("openai".into())))),
                    model: Some(Some("model-x".into())),
                },
                "owner",
            )
            .unwrap();
        assert_eq!(updated.title, "Cloudline Lead");
        assert_eq!(updated.runtime_id.as_deref(), Some("codex"));
        assert_eq!(updated.model.as_deref(), Some("model-x"));
        let replacement = replacement.unwrap();
        assert_ne!(replacement.id, first.id);
        assert_eq!(replacement.runtime_id.as_deref(), Some("codex"));
        let agents = l.position_agents(&coordinator.id, 10).unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[1].lifecycle_state, AgentLifecycle::Retired);
        // A rename alone keeps the incumbent.
        let (_, none) = l
            .update_position(
                &coordinator.id,
                &PositionPatch {
                    title: Some("Cloudline Coordinator".into()),
                    ..PositionPatch::default()
                },
                "owner",
            )
            .unwrap();
        assert!(none.is_none());
        // Unfinished work blocks letting the agent go.
        let turn = l
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "coordinate".into(),
                    metadata: json!({ "workforce": { "positionId": coordinator.id } }),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        l.transition_task(&turn.id, TaskState::Running, "agent:codex", None)
            .unwrap();
        assert!(err(l.vacate_position(&coordinator.id, "owner")).contains("unfinished task"));
        assert!(err(l.update_position(
            &coordinator.id,
            &PositionPatch {
                runtime: Some(Some(("claude-code".into(), None))),
                ..PositionPatch::default()
            },
            "owner"
        ))
        .contains("unfinished task"));
        l.transition_task(&turn.id, TaskState::Succeeded, "agent:codex", None)
            .unwrap();
        let gone = l.vacate_position(&coordinator.id, "owner").unwrap();
        assert_eq!(gone.lifecycle_state, AgentLifecycle::Retired);
        assert!(err(l.vacate_position(&coordinator.id, "owner")).contains("already vacant"));
        // The database also refuses a second incumbent.
        l.fill_position(&coordinator.id, None, "owner").unwrap();
        let c = l.conn();
        let dup = c.execute(
            "INSERT INTO agent_instances (id, role_id, lifecycle_state, created_at, last_seen_at, position_id)
             VALUES ('x', ?1, 'active', 1, 1, ?2)",
            params![coordinator.role_id, coordinator.id],
        );
        assert!(dup.is_err());
    }

    #[test]
    fn settings_and_role_templates() {
        let (l, roles) = setup();
        assert_eq!(l.setting("organization").unwrap(), None);
        l.put_setting(
            "organization",
            &json!({ "name": "8 West Ventures" }),
            "owner",
        )
        .unwrap();
        l.put_setting("organization", &json!({ "name": "8 West" }), "owner")
            .unwrap();
        assert_eq!(
            l.setting("organization").unwrap(),
            Some(json!({ "name": "8 West" }))
        );
        // Seeding again adds nothing.
        let before = l.list_roles().unwrap().len();
        l.ensure_roles(
            &[template("QA Engineer", RoleType::Worker, false)],
            "plenipo",
        )
        .unwrap();
        assert_eq!(l.list_roles().unwrap().len(), before);
        // A role's class is fixed.
        let c = l.conn();
        assert!(c
            .execute(
                "UPDATE roles SET persistent = 1 WHERE id = ?1",
                [&roles["QA Engineer"]]
            )
            .is_err());
        assert!(c
            .execute(
                "UPDATE roles SET role_type = 'superintendent' WHERE id = ?1",
                [&roles["QA Engineer"]]
            )
            .is_err());
        assert!(c
            .execute(
                "UPDATE roles SET description = 'tests' WHERE id = ?1",
                [&roles["QA Engineer"]]
            )
            .is_ok());
    }

    #[test]
    fn merging_a_setting_keeps_its_other_fields() {
        let l = ledger();
        let v = l
            .merge_setting("organization", &json!({ "name": "8 West" }), "owner")
            .unwrap();
        assert_eq!(v, json!({ "name": "8 West" }));
        let v = l
            .merge_setting("organization", &json!({ "titles": "army" }), "owner")
            .unwrap();
        assert_eq!(v, json!({ "name": "8 West", "titles": "army" }));
        l.merge_setting(
            "organization",
            &json!({ "name": "8 West Ventures" }),
            "owner",
        )
        .unwrap();
        assert_eq!(
            l.setting("organization").unwrap(),
            Some(json!({ "name": "8 West Ventures", "titles": "army" }))
        );
        // A value that is not an object is replaced; fields must be an object.
        l.put_setting("organization", &json!("old"), "owner")
            .unwrap();
        let v = l
            .merge_setting("organization", &json!({ "titles": "navy" }), "owner")
            .unwrap();
        assert_eq!(v, json!({ "titles": "navy" }));
        assert!(l
            .merge_setting("organization", &json!("navy"), "owner")
            .is_err());
        let changed = l
            .recent_events(10)
            .unwrap()
            .into_iter()
            .find(|e| e.event_type == "org.settings_changed")
            .unwrap();
        assert_eq!(
            changed.payload,
            json!({ "key": "organization", "fields": ["titles"] })
        );
    }

    #[test]
    fn a_renamed_template_keeps_its_role_and_positions() {
        let l = ledger();
        let old = RoleTemplate {
            name: "Project Coordinator",
            description: "Coordinates a project.",
            role_type: RoleType::ProjectCoordinator,
            persistent: true,
            metadata: json!({ "template": true, "purpose": ["coordinate"] }),
            formerly: &[],
        };
        let custom = RoleTemplate {
            name: "Superintendent",
            description: "The owner's own role, not a template.",
            role_type: RoleType::Superintendent,
            persistent: true,
            metadata: json!({ "template": false }),
            formerly: &[],
        };
        let roles = l.ensure_roles(&[old, custom], "plenipo").unwrap();
        let id = |name: &str| roles.iter().find(|r| r.name == name).unwrap().id.clone();
        let coordinator = id("Project Coordinator");
        let (held, _) = l
            .create_position(
                &position(&id("Superintendent"), "Chief", None, "codex"),
                "owner",
            )
            .unwrap();

        let renamed = [
            RoleTemplate {
                name: "Supervisor",
                description: "Leads a project team.",
                role_type: RoleType::ProjectCoordinator,
                persistent: true,
                metadata: json!({ "template": true, "purpose": ["lead the team"] }),
                formerly: &["Project Coordinator"],
            },
            RoleTemplate {
                name: "VP",
                description: "Runs the organization for the owner.",
                role_type: RoleType::Superintendent,
                persistent: true,
                metadata: json!({ "template": true }),
                formerly: &["Superintendent"],
            },
        ];
        let roles = l.ensure_roles(&renamed, "plenipo").unwrap();
        // The template's role is renamed in place: same ID, new wording.
        let supervisor = roles.iter().find(|r| r.name == "Supervisor").unwrap();
        assert_eq!(supervisor.id, coordinator);
        assert_eq!(supervisor.description, "Leads a project team.");
        assert_eq!(supervisor.metadata["purpose"], json!(["lead the team"]));
        assert!(roles.iter().all(|r| r.name != "Project Coordinator"));
        let renamed_event = l
            .recent_events(20)
            .unwrap()
            .into_iter()
            .find(|e| e.event_type == "org.role_renamed")
            .unwrap();
        assert_eq!(renamed_event.payload["formerly"], "Project Coordinator");
        assert_eq!(renamed_event.payload["name"], "Supervisor");
        // An owner's role that merely shares a former name is left alone; the template is
        // added next to it, and the position holding the owner's role keeps it.
        assert!(roles.iter().any(|r| r.name == "Superintendent"));
        assert!(roles.iter().any(|r| r.name == "VP"));
        assert_eq!(
            l.org_records()
                .unwrap()
                .positions
                .iter()
                .find(|p| p.id == held.id)
                .unwrap()
                .role_id,
            id("Superintendent")
        );
        // Seeding again changes nothing.
        let before = l.list_roles().unwrap();
        assert_eq!(l.ensure_roles(&renamed, "plenipo").unwrap(), before);
    }

    fn automatic(role: &str, title: &str, reports_to: Option<&str>) -> NewPosition {
        NewPosition {
            runtime_id: None,
            runtime_provider: None,
            ..position(role, title, reports_to, "unused")
        }
    }

    #[test]
    fn automatic_positions_are_routed_not_fixed() {
        let (l, roles) = setup();
        let (_, _, project, coordinator) = development(&l, &roles);
        // No runtime may use the automatic marker as its ID.
        assert!(clean_runtime_id(AUTOMATIC).is_err());
        // An automatic worker position is not bound by the project's runtimes when hired (the
        // Router routes within them when it takes work), and has no model of its own.
        let (dev, agent) = l
            .create_position(
                &automatic(
                    roles["Senior Developer"].as_str(),
                    "Senior Developer",
                    Some(&coordinator.id),
                ),
                "owner",
            )
            .unwrap();
        assert!(agent.is_none());
        assert_eq!(
            (dev.runtime_id.as_deref(), dev.model.as_deref()),
            (None, None)
        );
        let stored: String = l
            .conn()
            .query_row(
                "SELECT runtime_id FROM positions WHERE id = ?1",
                [&dev.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, AUTOMATIC);
        let with_model = NewPosition {
            model: Some("model-x".into()),
            ..automatic(roles["QA Engineer"].as_str(), "QA", Some(&coordinator.id))
        };
        assert!(err(l.create_position(&with_model, "owner")).contains("automatic position"));
        l.update_project_settings(&project.id, &settings("Cloudline", &["codex"]), "owner")
            .unwrap();
        assert!(l.position(&dev.id).unwrap().unwrap().runtime_id.is_none());

        // An automatic full-time position's agent has no runtime until its conversation is
        // routed; routing records why, once per call, only for an automatic incumbent.
        let (staff, agent) = l
            .create_position(
                &automatic(roles["Superintendent"].as_str(), "VP", None),
                "owner",
            )
            .unwrap();
        let agent = agent.unwrap();
        assert_eq!(
            (
                agent.runtime_id.as_deref(),
                agent.runtime_provider.as_deref()
            ),
            (None, None)
        );
        let route = AgentRoute {
            runtime_id: "codex".into(),
            runtime_provider: Some("openai".into()),
            model: Some("model-x".into()),
            routing: json!({ "reason": "first choice" }),
        };
        let routed = l.route_agent(&agent.id, &route, "owner").unwrap();
        assert_eq!(routed.runtime_id.as_deref(), Some("codex"));
        assert_eq!(routed.model.as_deref(), Some("model-x"));
        assert_eq!(routed.runtime_provider.as_deref(), Some("openai"));
        let event = l.recent_events(1).unwrap().remove(0);
        assert_eq!(event.event_type, "org.agent_routed");
        assert_eq!(event.payload["positionId"], json!(staff.id));
        assert_eq!(event.payload["routing"]["reason"], "first choice");
        let fixed = l.position_incumbent(&coordinator.id).unwrap().unwrap();
        assert!(err(l.route_agent(&fixed.id, &route, "owner")).contains("automatically"));
        l.vacate_position(&staff.id, "owner").unwrap();
        assert!(err(l.route_agent(&agent.id, &route, "owner")).contains("current agent"));
        assert!(l
            .route_agent(
                &agent.id,
                &AgentRoute {
                    runtime_id: AUTOMATIC.into(),
                    ..route
                },
                "owner"
            )
            .is_err());
    }

    #[test]
    fn a_position_switches_between_fixed_and_automatic() {
        let (l, roles) = setup();
        let (_, _, _, coordinator) = development(&l, &roles);
        l.update_position(
            &coordinator.id,
            &PositionPatch {
                model: Some(Some("model-x".into())),
                ..PositionPatch::default()
            },
            "owner",
        )
        .unwrap();
        let first = l.position_incumbent(&coordinator.id).unwrap().unwrap();
        // Automatic: the model goes with the fixed runtime, and a new agent is hired.
        let (updated, replacement) = l
            .update_position(
                &coordinator.id,
                &PositionPatch {
                    runtime: Some(None),
                    ..PositionPatch::default()
                },
                "owner",
            )
            .unwrap();
        assert_eq!((updated.runtime_id, updated.model), (None, None));
        let replacement = replacement.unwrap();
        assert_ne!(replacement.id, first.id);
        assert_eq!(replacement.runtime_id, None);
        let event = l
            .recent_events(10)
            .unwrap()
            .into_iter()
            .find(|e| e.event_type == "org.position_updated")
            .unwrap();
        assert_eq!(event.payload["automatic"], true);
        // An automatic position takes no model.
        assert!(err(l.update_position(
            &coordinator.id,
            &PositionPatch {
                model: Some(Some("model-y".into())),
                ..PositionPatch::default()
            },
            "owner"
        ))
        .contains("automatic position"));
        // Back to fixed: the runtime must be one the project allows.
        let (fixed, _) = l
            .update_position(
                &coordinator.id,
                &PositionPatch {
                    runtime: Some(Some(("codex".into(), Some("openai".into())))),
                    model: Some(Some("model-y".into())),
                    ..PositionPatch::default()
                },
                "owner",
            )
            .unwrap();
        assert_eq!(fixed.runtime_id.as_deref(), Some("codex"));
        assert_eq!(fixed.model.as_deref(), Some("model-y"));
        assert!(err(l.update_position(
            &coordinator.id,
            &PositionPatch {
                runtime: Some(Some(("other-tool".into(), None))),
                ..PositionPatch::default()
            },
            "owner"
        ))
        .contains("does not allow"));
    }

    #[test]
    fn settings_update_atomically_with_their_event() {
        let l = ledger();
        let add = |n: i64| {
            move |v: Value| -> Result<(Value, Value)> {
                let total = v["total"].as_i64().unwrap_or(0) + n;
                Ok((json!({ "total": total }), json!({ "added": n })))
            }
        };
        assert_eq!(
            l.update_setting("counter", "router.test", "owner", add(2))
                .unwrap()["total"],
            2
        );
        assert_eq!(
            l.update_setting("counter", "router.test", "owner", add(3))
                .unwrap()["total"],
            5
        );
        let event = l.recent_events(1).unwrap().remove(0);
        assert_eq!(
            (event.event_type.as_str(), &event.payload),
            ("router.test", &json!({ "added": 3 }))
        );
        // A refusal changes nothing and records nothing.
        let before = l.recent_events(10).unwrap().len();
        assert!(l
            .update_setting("counter", "router.test", "owner", |_| Err(invalid("no")))
            .is_err());
        assert_eq!(l.setting("counter").unwrap().unwrap()["total"], 5);
        assert_eq!(l.recent_events(10).unwrap().len(), before);
    }

    #[test]
    fn turn_outcomes_and_models_seen_come_from_the_executions() {
        let l = ledger();
        let run = |id: &str, runtime: &str, model: Option<&str>, at: u64, outcome: Option<&str>| {
            l.upsert_execution(
                &ExecutionRow {
                    id: id.into(),
                    task_id: None,
                    runtime: runtime.into(),
                    provider: None,
                    model: model.map(str::to_owned),
                    session_id: None,
                    process_id: None,
                    profile_id: None,
                    label: "turn".into(),
                    executable: None,
                    args: vec![],
                    working_dir: None,
                    state: "succeeded".into(),
                    exit_code: Some(0),
                    detail: None,
                    started_at: at,
                    ended_at: Some(at + 1),
                    usage_metadata: json!({}),
                },
                "test",
            )
            .unwrap();
            if let Some(outcome) = outcome {
                l.append_event(NewEvent {
                    execution_id: Some(id.into()),
                    source: format!("agent:{runtime}"),
                    event_type: "agent.result".into(),
                    payload: json!({ "outcome": outcome, "error": "limit|1760000000", "summary": "s" }),
                    ..NewEvent::default()
                })
                .unwrap();
            }
        };
        run("a", "claude-code", Some("opus"), 1_000, Some("completed"));
        run(
            "b",
            "claude-code",
            Some("opus"),
            2_000,
            Some("usageLimited"),
        );
        run("c", "codex", None, 3_000, Some("failed"));
        run("d", "codex", Some("gpt-x"), 50, Some("usageLimited"));
        let outcomes = l.recent_turn_outcomes(500).unwrap();
        let seen: Vec<(&str, &str)> = outcomes
            .iter()
            .map(|o| (o.runtime.as_str(), o.outcome.as_str()))
            .collect();
        // Newest first; other outcomes and older runs are left out.
        assert_eq!(
            seen,
            [
                ("claude-code", "usageLimited"),
                ("claude-code", "completed")
            ]
        );
        assert_eq!(outcomes[0].error.as_deref(), Some("limit|1760000000"));
        assert_eq!(outcomes[0].model.as_deref(), Some("opus"));
        let models = l.models_seen(10).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(
            (
                models[0].model.as_str(),
                models[0].runs,
                models[0].last_used
            ),
            ("opus", 2, 2_000)
        );
        assert_eq!(models[1].model, "gpt-x");
    }
}
