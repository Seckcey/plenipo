//! Organization entities: roles, departments, projects, agent instances.
//! Schema and repository only; the Workforce engine (Phase 5) builds on these.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::{json, Value};

use crate::dto::*;
use crate::error::{LedgerError, Result};
use crate::events;
use crate::rows::{json as parse_json, metadata_text, parse_enum, u64_of};
use crate::Ledger;

fn name_ok(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 200 {
        Err(LedgerError::InvalidInput(
            "name must be 1–200 characters".into(),
        ))
    } else {
        Ok(name.to_owned())
    }
}

fn org_event(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
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

/// Map "FOREIGN KEY constraint failed" on delete to a clear caller error.
fn in_use(e: rusqlite::Error, what: &str) -> LedgerError {
    if e.to_string().contains("FOREIGN KEY") {
        LedgerError::InvalidInput(format!("{what} is still referenced and cannot be deleted"))
    } else {
        e.into()
    }
}

fn role_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Role> {
    Ok(Role {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        role_type: parse_enum(3, r.get(3)?, RoleType::parse)?,
        persistent: r.get(4)?,
        model_policy_id: r.get(5)?,
        capability_profile_id: r.get(6)?,
        metadata: parse_json(r.get(7)?),
        created_at: u64_of(r.get(8)?),
    })
}
const ROLE_COLS: &str = "id, name, description, role_type, persistent, model_policy_id, capability_profile_id, metadata, created_at";

fn dept_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Department> {
    Ok(Department {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        manager_role_id: r.get(3)?,
        status: r.get(4)?,
        metadata: parse_json(r.get(5)?),
        created_at: u64_of(r.get(6)?),
    })
}
const DEPT_COLS: &str = "id, name, description, manager_role_id, status, metadata, created_at";

fn project_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        local_path: r.get(2)?,
        repository_url: r.get(3)?,
        department_id: r.get(4)?,
        metadata: parse_json(r.get(5)?),
        created_at: u64_of(r.get(6)?),
    })
}
const PROJECT_COLS: &str =
    "id, name, local_path, repository_url, department_id, metadata, created_at";

fn agent_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AgentInstance> {
    Ok(AgentInstance {
        id: r.get(0)?,
        role_id: r.get(1)?,
        runtime_provider: r.get(2)?,
        provider_session_id: r.get(3)?,
        project_id: r.get(4)?,
        lifecycle_state: parse_enum(5, r.get(5)?, AgentLifecycle::parse)?,
        metadata: parse_json(r.get(6)?),
        created_at: u64_of(r.get(7)?),
        last_seen_at: u64_of(r.get(8)?),
    })
}
const AGENT_COLS: &str = "id, role_id, runtime_provider, provider_session_id, project_id, lifecycle_state, metadata, created_at, last_seen_at";

fn one<T>(
    c: &Connection,
    sql: &str,
    id: &str,
    map: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Option<T>> {
    Ok(c.query_row(sql, [id], map).optional()?)
}

fn all<T>(
    c: &Connection,
    sql: &str,
    map: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = c.prepare(sql)?;
    let rows = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

impl Ledger {
    // ---- Roles --------------------------------------------------------------------------

    pub fn create_role(
        &self,
        name: &str,
        description: &str,
        role_type: RoleType,
        persistent: bool,
        metadata: &Value,
        actor: &str,
    ) -> Result<Role> {
        let name = name_ok(name)?;
        let metadata = metadata_text(metadata)?;
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO roles (id, name, description, role_type, persistent, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, name, description, role_type.as_str(), persistent, metadata, crate::now_ms() as i64],
            )?;
            org_event(tx, out, actor, "role_created", json!({ "id": id, "name": name, "roleType": role_type }))?;
            one(tx, &format!("SELECT {ROLE_COLS} FROM roles WHERE id = ?1"), &id, role_row)?
                .ok_or_else(|| LedgerError::NotFound(id))
        })
    }

    pub fn role(&self, id: &str) -> Result<Option<Role>> {
        self.read(|c| {
            one(
                c,
                &format!("SELECT {ROLE_COLS} FROM roles WHERE id = ?1"),
                id,
                role_row,
            )
        })
    }

    pub fn list_roles(&self) -> Result<Vec<Role>> {
        self.read(|c| {
            all(
                c,
                &format!("SELECT {ROLE_COLS} FROM roles ORDER BY name"),
                role_row,
            )
        })
    }

    /// Point a role at model-policy / capability-profile records (Phases 6–7).
    pub fn set_role_policies(
        &self,
        id: &str,
        model_policy_id: Option<&str>,
        capability_profile_id: Option<&str>,
        actor: &str,
    ) -> Result<Role> {
        self.write(|tx, out| {
            let n = tx.execute(
                "UPDATE roles SET model_policy_id = ?2, capability_profile_id = ?3 WHERE id = ?1",
                params![id, model_policy_id, capability_profile_id],
            )?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("role {id}")));
            }
            org_event(
                tx,
                out,
                actor,
                "role_updated",
                json!({ "id": id, "modelPolicyId": model_policy_id, "capabilityProfileId": capability_profile_id }),
            )?;
            one(tx, &format!("SELECT {ROLE_COLS} FROM roles WHERE id = ?1"), id, role_row)?
                .ok_or_else(|| LedgerError::NotFound(id.into()))
        })
    }

    pub fn delete_role(&self, id: &str, actor: &str) -> Result<()> {
        self.write(|tx, out| {
            let n = tx
                .execute("DELETE FROM roles WHERE id = ?1", [id])
                .map_err(|e| in_use(e, "role"))?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("role {id}")));
            }
            org_event(tx, out, actor, "role_deleted", json!({ "id": id }))
        })
    }

    // ---- Departments --------------------------------------------------------------------

    pub fn create_department(
        &self,
        name: &str,
        description: &str,
        manager_role_id: Option<&str>,
        actor: &str,
    ) -> Result<Department> {
        let name = name_ok(name)?;
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO departments (id, name, description, manager_role_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    name,
                    description,
                    manager_role_id,
                    crate::now_ms() as i64
                ],
            )?;
            org_event(
                tx,
                out,
                actor,
                "department_created",
                json!({ "id": id, "name": name }),
            )?;
            one(
                tx,
                &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                &id,
                dept_row,
            )?
            .ok_or_else(|| LedgerError::NotFound(id))
        })
    }

    pub fn department(&self, id: &str) -> Result<Option<Department>> {
        self.read(|c| {
            one(
                c,
                &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"),
                id,
                dept_row,
            )
        })
    }

    pub fn list_departments(&self) -> Result<Vec<Department>> {
        self.read(|c| {
            all(
                c,
                &format!("SELECT {DEPT_COLS} FROM departments ORDER BY name"),
                dept_row,
            )
        })
    }

    pub fn update_department(
        &self,
        id: &str,
        name: &str,
        description: &str,
        manager_role_id: Option<&str>,
        active: bool,
        actor: &str,
    ) -> Result<Department> {
        let name = name_ok(name)?;
        self.write(|tx, out| {
            let n = tx.execute(
                "UPDATE departments SET name = ?2, description = ?3, manager_role_id = ?4, status = ?5 WHERE id = ?1",
                params![id, name, description, manager_role_id, if active { "active" } else { "inactive" }],
            )?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("department {id}")));
            }
            org_event(tx, out, actor, "department_updated", json!({ "id": id, "name": name, "active": active }))?;
            one(tx, &format!("SELECT {DEPT_COLS} FROM departments WHERE id = ?1"), id, dept_row)?
                .ok_or_else(|| LedgerError::NotFound(id.into()))
        })
    }

    pub fn delete_department(&self, id: &str, actor: &str) -> Result<()> {
        self.write(|tx, out| {
            let n = tx
                .execute("DELETE FROM departments WHERE id = ?1", [id])
                .map_err(|e| in_use(e, "department"))?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("department {id}")));
            }
            org_event(tx, out, actor, "department_deleted", json!({ "id": id }))
        })
    }

    // ---- Projects -----------------------------------------------------------------------

    pub fn create_project(
        &self,
        name: &str,
        local_path: Option<&str>,
        repository_url: Option<&str>,
        department_id: Option<&str>,
        actor: &str,
    ) -> Result<Project> {
        let name = name_ok(name)?;
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO projects (id, name, local_path, repository_url, department_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, local_path, repository_url, department_id, crate::now_ms() as i64],
            )?;
            org_event(tx, out, actor, "project_created", json!({ "id": id, "name": name, "departmentId": department_id }))?;
            one(tx, &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"), &id, project_row)?
                .ok_or_else(|| LedgerError::NotFound(id))
        })
    }

    pub fn project(&self, id: &str) -> Result<Option<Project>> {
        self.read(|c| {
            one(
                c,
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                id,
                project_row,
            )
        })
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.read(|c| {
            all(
                c,
                &format!("SELECT {PROJECT_COLS} FROM projects ORDER BY name"),
                project_row,
            )
        })
    }

    /// Move a project to another department (or none).
    pub fn reassign_project(
        &self,
        id: &str,
        department_id: Option<&str>,
        actor: &str,
    ) -> Result<Project> {
        self.write(|tx, out| {
            let n = tx.execute(
                "UPDATE projects SET department_id = ?2 WHERE id = ?1",
                params![id, department_id],
            )?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("project {id}")));
            }
            org_event(
                tx,
                out,
                actor,
                "project_reassigned",
                json!({ "id": id, "departmentId": department_id }),
            )?;
            one(
                tx,
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                id,
                project_row,
            )?
            .ok_or_else(|| LedgerError::NotFound(id.into()))
        })
    }

    pub fn delete_project(&self, id: &str, actor: &str) -> Result<()> {
        self.write(|tx, out| {
            let n = tx
                .execute("DELETE FROM projects WHERE id = ?1", [id])
                .map_err(|e| in_use(e, "project"))?;
            if n == 0 {
                return Err(LedgerError::NotFound(format!("project {id}")));
            }
            org_event(tx, out, actor, "project_deleted", json!({ "id": id }))
        })
    }

    // ---- Agent instances ----------------------------------------------------------------

    pub fn create_agent_instance(
        &self,
        role_id: &str,
        runtime_provider: Option<&str>,
        project_id: Option<&str>,
        actor: &str,
    ) -> Result<AgentInstance> {
        self.write(|tx, out| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = crate::now_ms() as i64;
            tx.execute(
                "INSERT INTO agent_instances (id, role_id, runtime_provider, project_id, lifecycle_state, created_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, 'starting', ?5, ?5)",
                params![id, role_id, runtime_provider, project_id, now],
            )?;
            org_event(tx, out, actor, "agent_created", json!({ "id": id, "roleId": role_id }))?;
            one(tx, &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"), &id, agent_row)?
                .ok_or_else(|| LedgerError::NotFound(id))
        })
    }

    pub fn agent_instance(&self, id: &str) -> Result<Option<AgentInstance>> {
        self.read(|c| {
            one(
                c,
                &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
                id,
                agent_row,
            )
        })
    }

    /// Agents that are not retired or failed.
    pub fn active_agent_instances(&self) -> Result<Vec<AgentInstance>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {AGENT_COLS} FROM agent_instances
                     WHERE lifecycle_state NOT IN ('retired', 'failed') ORDER BY created_at"
                ),
                agent_row,
            )
        })
    }

    pub fn set_agent_lifecycle(
        &self,
        id: &str,
        next: AgentLifecycle,
        actor: &str,
    ) -> Result<AgentInstance> {
        self.write(|tx, out| {
            let agent = one(
                tx,
                &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
                id,
                agent_row,
            )?
            .ok_or_else(|| LedgerError::NotFound(format!("agent {id}")))?;
            if !agent.lifecycle_state.can_transition_to(next) {
                return Err(LedgerError::InvalidTransition {
                    entity: "agent",
                    id: id.into(),
                    from: agent.lifecycle_state.as_str().into(),
                    to: next.as_str().into(),
                });
            }
            tx.execute(
                "UPDATE agent_instances SET lifecycle_state = ?2, last_seen_at = ?3 WHERE id = ?1",
                params![id, next.as_str(), crate::now_ms() as i64],
            )?;
            org_event(
                tx,
                out,
                actor,
                "agent_lifecycle_changed",
                json!({ "id": id, "from": agent.lifecycle_state, "to": next }),
            )?;
            one(
                tx,
                &format!("SELECT {AGENT_COLS} FROM agent_instances WHERE id = ?1"),
                id,
                agent_row,
            )?
            .ok_or_else(|| LedgerError::NotFound(id.into()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn role_crud() {
        let l = ledger();
        let r = l
            .create_role(
                "Senior Developer",
                "implements",
                RoleType::Worker,
                false,
                &Value::Null,
                "owner",
            )
            .unwrap();
        assert_eq!(l.role(&r.id).unwrap().unwrap(), r);
        let r2 = l
            .set_role_policies(&r.id, Some("policy-1"), None, "owner")
            .unwrap();
        assert_eq!(r2.model_policy_id.as_deref(), Some("policy-1"));
        assert!(
            l.create_role(
                "Senior Developer",
                "",
                RoleType::Worker,
                false,
                &Value::Null,
                "owner"
            )
            .is_err(),
            "names are unique"
        );
        l.delete_role(&r.id, "owner").unwrap();
        assert!(l.role(&r.id).unwrap().is_none());
        assert!(matches!(
            l.delete_role(&r.id, "owner"),
            Err(LedgerError::NotFound(_))
        ));
    }

    #[test]
    fn department_and_project_crud_with_reassignment() {
        let l = ledger();
        let mgr = l
            .create_role(
                "Development Superintendent",
                "",
                RoleType::Superintendent,
                true,
                &Value::Null,
                "owner",
            )
            .unwrap();
        let dev = l
            .create_department("Development", "builds things", Some(&mgr.id), "owner")
            .unwrap();
        let ops = l
            .create_department("Operations", "", None, "owner")
            .unwrap();
        let p = l
            .create_project(
                "Cloudline",
                Some("C:/src/cloudline"),
                None,
                Some(&dev.id),
                "owner",
            )
            .unwrap();
        assert_eq!(p.department_id.as_deref(), Some(dev.id.as_str()));

        let moved = l.reassign_project(&p.id, Some(&ops.id), "owner").unwrap();
        assert_eq!(moved.department_id.as_deref(), Some(ops.id.as_str()));

        let renamed = l
            .update_department(
                &dev.id,
                "Engineering",
                "renamed",
                Some(&mgr.id),
                false,
                "owner",
            )
            .unwrap();
        assert_eq!(
            (renamed.name.as_str(), renamed.status.as_str()),
            ("Engineering", "inactive")
        );

        // Referenced rows cannot be deleted.
        assert!(matches!(
            l.delete_department(&ops.id, "owner"),
            Err(LedgerError::InvalidInput(_))
        ));
        assert!(matches!(
            l.delete_role(&mgr.id, "owner"),
            Err(LedgerError::InvalidInput(_))
        ));
        l.delete_project(&p.id, "owner").unwrap();
        l.delete_department(&ops.id, "owner").unwrap();
        assert_eq!(l.list_departments().unwrap().len(), 1);
        assert_eq!(l.list_projects().unwrap().len(), 0);

        let org_events = l.recent_events(50).unwrap();
        assert!(org_events
            .iter()
            .any(|e| e.event_type == "org.project_reassigned"));
        assert!(org_events
            .iter()
            .any(|e| e.event_type == "org.department_deleted"));
    }

    #[test]
    fn agent_lifecycle_is_enforced() {
        let l = ledger();
        let role = l
            .create_role(
                "Reviewer",
                "",
                RoleType::Worker,
                false,
                &Value::Null,
                "owner",
            )
            .unwrap();
        let a = l
            .create_agent_instance(&role.id, Some("local"), None, "coordinator")
            .unwrap();
        assert_eq!(a.lifecycle_state, AgentLifecycle::Starting);
        l.set_agent_lifecycle(&a.id, AgentLifecycle::Active, "coordinator")
            .unwrap();
        assert_eq!(l.active_agent_instances().unwrap().len(), 1);
        l.set_agent_lifecycle(&a.id, AgentLifecycle::Retired, "coordinator")
            .unwrap();
        assert!(l.active_agent_instances().unwrap().is_empty());
        assert!(matches!(
            l.set_agent_lifecycle(&a.id, AgentLifecycle::Active, "coordinator"),
            Err(LedgerError::InvalidTransition { .. })
        ));
        // History remains.
        assert!(l.agent_instance(&a.id).unwrap().is_some());
        assert!(l
            .create_agent_instance("no-such-role", None, None, "x")
            .is_err());
    }
}
