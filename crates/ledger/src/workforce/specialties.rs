//! Specialties under each role (Phase 17, ADR-042). Plenipo seeds the built-in ones and brings
//! them up to date at every start, like built-in roles; the owner adds their own to any role,
//! and can change or remove those. A removed specialty keeps its row, so positions and saved
//! agents that had it still name it. Every change is an `org.specialty_*` event in the same
//! transaction.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use super::{all, invalid, now, org_event};
use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::org::{role_row, ROLE_COLS};
use crate::rows::{json as parse_json, metadata_text, opt_u64, u64_of};
use crate::Ledger;

pub(crate) const SPECIALTY_COLS: &str =
    "id, role_id, name, title, metadata, created_at, updated_at, removed_at";

pub(crate) fn specialty_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Specialty> {
    Ok(Specialty {
        id: r.get(0)?,
        role_id: r.get(1)?,
        name: r.get(2)?,
        title: r.get(3)?,
        metadata: parse_json(r.get(4)?),
        created_at: u64_of(r.get(5)?),
        updated_at: u64_of(r.get(6)?),
        removed_at: opt_u64(r.get(7)?),
    })
}

fn get(c: &Connection, id: &str) -> Result<Option<Specialty>> {
    Ok(c.query_row(
        &format!("SELECT {SPECIALTY_COLS} FROM specialties WHERE id = ?1"),
        [id],
        specialty_row,
    )
    .optional()?)
}

fn require(c: &Connection, id: &str) -> Result<Specialty> {
    get(c, id)?.ok_or_else(|| LedgerError::NotFound(format!("specialty {id}")))
}

/// The specialty `id` for a position of `role_id`: it exists, belongs to that role, and has not
/// been removed. `None` stays `None`.
pub(super) fn check(c: &Connection, role_id: &str, id: Option<&str>) -> Result<Option<Specialty>> {
    let Some(id) = id else {
        return Ok(None);
    };
    let s = get(c, id)?.ok_or_else(|| invalid("that specialty no longer exists"))?;
    if s.role_id != role_id {
        return Err(invalid(format!(
            "{} is a specialty of another role; choose one of this role's",
            s.name
        )));
    }
    if s.removed_at.is_some() {
        return Err(invalid(format!("the {} specialty was removed", s.name)));
    }
    Ok(Some(s))
}

/// The specialty `id`, when it is still in use (not removed); `None` otherwise.
pub(super) fn current(c: &Connection, id: Option<&str>) -> Result<Option<Specialty>> {
    Ok(match id {
        Some(id) => get(c, id)?.filter(|s| s.removed_at.is_none()),
        None => None,
    })
}

fn unique_name(e: rusqlite::Error, name: &str) -> LedgerError {
    if e.to_string().contains("UNIQUE") {
        invalid(format!(
            "this role already has a specialty named \"{name}\""
        ))
    } else {
        e.into()
    }
}

/// `metadata` with `template` set as given (the owner cannot mark a specialty as built in).
fn with_template(metadata: &Value, template: bool) -> Value {
    let mut m = if metadata.is_object() {
        metadata.clone()
    } else {
        json!({})
    };
    m["template"] = json!(template);
    m
}

impl Ledger {
    /// Every specialty, removed ones included, by role and name.
    pub fn list_specialties(&self) -> Result<Vec<Specialty>> {
        self.read(|c| {
            all(
                c,
                &format!("SELECT {SPECIALTY_COLS} FROM specialties ORDER BY role_id, lower(name)"),
                [],
                specialty_row,
            )
        })
    }

    pub fn specialty(&self, id: &str) -> Result<Option<Specialty>> {
        self.read(|c| get(c, id))
    }

    /// Seed the built-in specialties that are missing and bring the others up to date (their
    /// title and lines). A template whose role is not a built-in role, or whose name the owner
    /// already gave a specialty of their own on that role, is left out. Returns every specialty.
    pub fn ensure_specialties(
        &self,
        templates: &[SpecialtyTemplate],
        actor: &str,
    ) -> Result<Vec<Specialty>> {
        self.write(|tx, out| {
            for t in templates {
                let role: Option<Role> = tx
                    .query_row(
                        &format!(
                            "SELECT {ROLE_COLS} FROM roles
                             WHERE name = ?1 AND json_extract(metadata, '$.template') = 1"
                        ),
                        [t.role],
                        role_row,
                    )
                    .optional()?;
                let Some(role) = role else {
                    continue;
                };
                let metadata = with_template(&t.metadata, true);
                let existing: Option<Specialty> = tx
                    .query_row(
                        &format!(
                            "SELECT {SPECIALTY_COLS} FROM specialties
                             WHERE role_id = ?1 AND lower(name) = lower(?2) AND removed_at IS NULL"
                        ),
                        params![role.id, t.name],
                        specialty_row,
                    )
                    .optional()?;
                match existing {
                    Some(s) if !s.built_in() => {}
                    Some(s) => {
                        if s.title != t.title || s.metadata != metadata || s.name != t.name {
                            tx.execute(
                                "UPDATE specialties SET name = ?2, title = ?3, metadata = ?4,
                                     updated_at = ?5
                                 WHERE id = ?1",
                                params![s.id, t.name, t.title, metadata_text(&metadata)?, now()],
                            )?;
                            org_event(
                                out,
                                tx,
                                actor,
                                "specialty_updated",
                                json!({
                                    "id": s.id,
                                    "roleId": role.id,
                                    "role": role.name,
                                    "name": t.name,
                                    "template": true,
                                }),
                            )?;
                        }
                    }
                    None => {
                        let id = uuid::Uuid::new_v4().to_string();
                        let at = now();
                        tx.execute(
                            "INSERT INTO specialties (id, role_id, name, title, metadata,
                                 created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                            params![id, role.id, t.name, t.title, metadata_text(&metadata)?, at],
                        )?;
                        org_event(
                            out,
                            tx,
                            actor,
                            "specialty_created",
                            json!({
                                "id": id,
                                "roleId": role.id,
                                "role": role.name,
                                "name": t.name,
                                "template": true,
                            }),
                        )?;
                    }
                }
            }
            all(
                tx,
                &format!("SELECT {SPECIALTY_COLS} FROM specialties ORDER BY role_id, lower(name)"),
                [],
                specialty_row,
            )
        })
    }

    /// Add one of the owner's specialties to `role_id` (a built-in role or the owner's own).
    pub fn create_specialty(
        &self,
        role_id: &str,
        fields: &SpecialtyFields,
        actor: &str,
    ) -> Result<Specialty> {
        self.write(|tx, out| {
            let role = tx
                .query_row(
                    &format!("SELECT {ROLE_COLS} FROM roles WHERE id = ?1"),
                    [role_id],
                    role_row,
                )
                .optional()?
                .ok_or_else(|| invalid("that role no longer exists"))?;
            let id = uuid::Uuid::new_v4().to_string();
            let at = now();
            let metadata = with_template(&fields.metadata, false);
            tx.execute(
                "INSERT INTO specialties (id, role_id, name, title, metadata, created_at,
                     updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    id,
                    role.id,
                    fields.name,
                    fields.title,
                    metadata_text(&metadata)?,
                    at
                ],
            )
            .map_err(|e| unique_name(e, &fields.name))?;
            org_event(
                out,
                tx,
                actor,
                "specialty_created",
                json!({ "id": id, "roleId": role.id, "role": role.name, "name": fields.name }),
            )?;
            require(tx, &id)
        })
    }

    /// Change one of the owner's specialties (built-in ones keep their lines).
    pub fn update_specialty(
        &self,
        id: &str,
        fields: &SpecialtyFields,
        actor: &str,
    ) -> Result<Specialty> {
        self.write(|tx, out| {
            let s = require(tx, id)?;
            if s.built_in() {
                return Err(invalid(
                    "built-in specialties keep their lines; add a specialty of your own to write \
                     different ones",
                ));
            }
            if s.removed_at.is_some() {
                return Err(invalid(format!("the {} specialty was removed", s.name)));
            }
            let metadata = with_template(&fields.metadata, false);
            tx.execute(
                "UPDATE specialties SET name = ?2, title = ?3, metadata = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![
                    id,
                    fields.name,
                    fields.title,
                    metadata_text(&metadata)?,
                    now()
                ],
            )
            .map_err(|e| unique_name(e, &fields.name))?;
            org_event(
                out,
                tx,
                actor,
                "specialty_updated",
                json!({
                    "id": id,
                    "roleId": s.role_id,
                    "name": fields.name,
                    "formerly": s.name,
                }),
            )?;
            require(tx, id)
        })
    }

    /// Remove one of the owner's specialties. Refused while an active position has it; archived
    /// positions that had it come back without it.
    pub fn remove_specialty(&self, id: &str, actor: &str) -> Result<Specialty> {
        self.write(|tx, out| {
            let s = require(tx, id)?;
            if s.built_in() {
                return Err(invalid("built-in specialties cannot be removed"));
            }
            if s.removed_at.is_some() {
                return Ok(s);
            }
            let holders: Vec<String> = all(
                tx,
                "SELECT title FROM positions WHERE specialty_id = ?1 AND state = 'active'
                 ORDER BY title",
                [id],
                |r| r.get(0),
            )?;
            if !holders.is_empty() {
                return Err(invalid(format!(
                    "{} is the specialty of {}; give {} another specialty first",
                    s.name,
                    holders.join(", "),
                    if holders.len() == 1 { "it" } else { "them" }
                )));
            }
            tx.execute(
                "UPDATE specialties SET removed_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id, now()],
            )?;
            org_event(
                out,
                tx,
                actor,
                "specialty_removed",
                json!({ "id": id, "roleId": s.role_id, "name": s.name }),
            )?;
            require(tx, id)
        })
    }
}
