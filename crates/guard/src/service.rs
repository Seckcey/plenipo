//! The Guard service: reads and changes Guard's settings in the Ledger (one document, one
//! transaction and one `guard.*` event per change) and works out a worker's scope from the
//! organization's records.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use plenipo_ledger::{Ledger, LedgerError, OrgRecords};
use serde_json::{json, Value};

use crate::config::GuardConfig;
use crate::defaults::template_sets;
use crate::dto::*;
use crate::engine::{Scope, ScopeProject, ScopeUnit};
use crate::error::{GuardError, Result};
use crate::registry::Capability;

/// Ledger setting holding Guard's configuration.
pub const SETTING: &str = "guard";
pub const OWNER: &str = "owner";
pub const PLENIPO: &str = "plenipo";

struct Inner {
    ledger: Arc<Ledger>,
    notices: Mutex<Vec<String>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Guard {
    inner: Arc<Inner>,
}

fn to_ledger(e: GuardError) -> LedgerError {
    match e {
        GuardError::Ledger(e) => e,
        GuardError::Invalid(m) => LedgerError::InvalidInput(m),
    }
}

/// The department whose head is `id` or the nearest supervisor above it.
fn department_of<'a>(
    records: &'a OrgRecords,
    position_id: &str,
) -> Option<&'a plenipo_ledger::Department> {
    let positions: HashMap<&str, &plenipo_ledger::Position> = records
        .positions
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let mut seen = HashSet::new();
    let mut current = positions.get(position_id).copied();
    while let Some(p) = current {
        if !seen.insert(p.id.as_str()) {
            break;
        }
        if let Some(d) = records
            .departments
            .iter()
            .find(|d| d.head_position_id.as_deref() == Some(p.id.as_str()))
        {
            return Some(d);
        }
        current = p
            .reports_to
            .as_deref()
            .and_then(|s| positions.get(s).copied());
    }
    None
}

impl Guard {
    /// The service. Stores Plenipo's starting settings on first use and adds back any built-in
    /// permission set that is missing.
    pub fn new(ledger: Arc<Ledger>) -> Self {
        let this = Self {
            inner: Arc::new(Inner {
                ledger,
                notices: Mutex::new(Vec::new()),
            }),
        };
        if let Err(e) = this.ensure_defaults() {
            this.notice(format!("Could not store Guard's starting settings: {e}"));
        }
        this
    }

    fn notice(&self, notice: String) {
        let mut n = self.inner.notices.lock().unwrap_or_else(|p| p.into_inner());
        if !n.contains(&notice) {
            n.push(notice);
        }
    }

    pub fn notices(&self) -> Vec<String> {
        self.inner
            .notices
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn ledger(&self) -> &Arc<Ledger> {
        &self.inner.ledger
    }

    /// The stored configuration.
    pub fn config(&self) -> Result<GuardConfig> {
        let value = self.ledger().setting(SETTING)?.unwrap_or(Value::Null);
        GuardConfig::from_value(value).map_err(|e| {
            GuardError::Invalid(format!(
                "the saved permission settings could not be read: {e}"
            ))
        })
    }

    /// Change the configuration in one Ledger transaction, recording `event` with the payload
    /// `change` returns (`None`: nothing to record, nothing written).
    fn update<T>(
        &self,
        event: &str,
        actor: &str,
        change: impl FnOnce(&mut GuardConfig) -> Result<Option<(Value, T)>>,
    ) -> Result<Option<T>> {
        let mut out = None;
        let mut skipped = false;
        let result = self
            .ledger()
            .update_setting(SETTING, event, actor, |value| {
                let mut config = GuardConfig::from_value(value).map_err(|e| {
                    LedgerError::InvalidInput(format!(
                        "the saved permission settings could not be read: {e}"
                    ))
                })?;
                match change(&mut config).map_err(to_ledger)? {
                    Some((payload, value)) => {
                        out = Some(value);
                        Ok((config.to_value(), payload))
                    }
                    None => {
                        skipped = true;
                        Err(LedgerError::InvalidInput(String::new()))
                    }
                }
            });
        match result {
            Ok(_) => Ok(out),
            Err(_) if skipped => Ok(None),
            // A refusal keeps its plain message (no "invalid input:" in front of it).
            Err(LedgerError::InvalidInput(m)) => Err(GuardError::Invalid(m)),
            Err(e) => Err(e.into()),
        }
    }

    fn ensure_defaults(&self) -> Result<()> {
        if self.ledger().setting(SETTING)?.is_none() {
            self.update("guard.defaults_added", PLENIPO, |c| {
                *c = GuardConfig::with_defaults();
                Ok(Some((
                    json!({ "sets": c.sets.iter().map(|s| &s.id).collect::<Vec<_>>() }),
                    (),
                )))
            })?;
            return Ok(());
        }
        let config = self.config()?;
        let mut probe = config.clone();
        if probe.add_missing_builtins().is_empty() {
            return Ok(());
        }
        self.update("guard.sets_added", PLENIPO, |c| {
            let added = c.add_missing_builtins();
            Ok((!added.is_empty()).then(|| (json!({ "sets": added }), ())))
        })?;
        Ok(())
    }

    /// Give each built-in role template its starting permission set, once (a role the owner
    /// has decided about — even "none" — is left alone).
    pub fn seed_template_roles(&self) -> Result<()> {
        let roles = self.ledger().list_roles()?;
        let config = self.config()?;
        let wanted: Vec<(String, String)> = roles
            .iter()
            .filter(|r| r.metadata["template"] == true && !config.roles.contains_key(&r.id))
            .filter_map(|r| {
                template_sets()
                    .iter()
                    .find(|(name, _)| *name == r.name)
                    .filter(|(_, set)| config.set(set).is_some())
                    .map(|(_, set)| (r.id.clone(), (*set).to_owned()))
            })
            .collect();
        if wanted.is_empty() {
            return Ok(());
        }
        self.update("guard.roles_seeded", PLENIPO, |c| {
            let mut added = serde_json::Map::new();
            for (role, set) in &wanted {
                if !c.roles.contains_key(role) {
                    c.roles.insert(role.clone(), Some(set.clone()));
                    added.insert(role.clone(), json!(set));
                }
            }
            Ok((!added.is_empty()).then(|| (json!({ "roles": added }), ())))
        })?;
        Ok(())
    }

    // ---- The owner's changes ----------------------------------------------------------------

    pub fn save_set(&self, input: &PermissionSetInput) -> Result<PermissionSet> {
        let event = if input.id.is_some() {
            "guard.set_changed"
        } else {
            "guard.set_added"
        };
        self.update(event, OWNER, |c| {
            let set = c.save_set(input)?;
            Ok(Some((json!({ "set": set }), set)))
        })?
        .ok_or_else(|| GuardError::Invalid("nothing changed".into()))
    }

    pub fn remove_set(&self, id: &str) -> Result<()> {
        let projects: Vec<String> = self
            .ledger()
            .list_projects()?
            .into_iter()
            .filter(|p| p.status == "active" && p.capability_profile.as_deref() == Some(id))
            .map(|p| p.name)
            .collect();
        self.update("guard.set_removed", OWNER, |c| {
            let set = c.remove_set(id, &projects)?;
            Ok(Some((json!({ "id": set.id, "name": set.name }), ())))
        })?;
        Ok(())
    }

    pub fn assign_role(&self, role_id: &str, set_id: Option<&str>) -> Result<()> {
        let role = self
            .ledger()
            .list_roles()?
            .into_iter()
            .find(|r| r.id == role_id)
            .ok_or_else(|| GuardError::Invalid("that role no longer exists".into()))?;
        self.update("guard.role_assigned", OWNER, |c| {
            c.assign_role(&role.id, set_id)?;
            Ok(Some((
                json!({ "roleId": role.id, "role": role.name, "setId": set_id }),
                (),
            )))
        })?;
        Ok(())
    }

    pub fn assign_department(&self, department_id: &str, set_id: Option<&str>) -> Result<()> {
        let department = self
            .ledger()
            .list_departments()?
            .into_iter()
            .find(|d| d.id == department_id)
            .ok_or_else(|| GuardError::Invalid("that department no longer exists".into()))?;
        self.update("guard.department_limited", OWNER, |c| {
            c.assign_department(&department.id, set_id)?;
            Ok(Some((
                json!({ "departmentId": department.id, "department": department.name, "setId": set_id }),
                (),
            )))
        })?;
        Ok(())
    }

    /// Check that `set_id` can be a project's limit (the project's own field holds it).
    pub fn check_project_limit(&self, set_id: Option<&str>) -> Result<()> {
        match set_id.map(str::trim).filter(|s| !s.is_empty()) {
            Some(id) if self.config()?.set(id).is_none() => Err(GuardError::Invalid(format!(
                "\"{id}\" is not one of your permission sets"
            ))),
            _ => Ok(()),
        }
    }

    pub fn set_commands(&self, rules: &CommandRules) -> Result<()> {
        self.update("guard.commands_changed", OWNER, |c| {
            c.set_commands(rules)?;
            Ok(Some((json!({ "commands": c.commands }), ())))
        })?;
        Ok(())
    }

    pub fn set_blocked_files(&self, patterns: &[String]) -> Result<()> {
        self.update("guard.files_changed", OWNER, |c| {
            c.set_blocked_files(patterns)?;
            Ok(Some((json!({ "blockedFiles": c.blocked_files }), ())))
        })?;
        Ok(())
    }

    pub fn set_sensitive(&self, kind: SensitiveKind, rule: SensitiveRule) -> Result<()> {
        self.update("guard.sensitive_changed", OWNER, |c| {
            c.set_sensitive(kind, rule);
            Ok(Some((json!({ "kind": kind, "rule": rule }), ())))
        })?;
        Ok(())
    }

    pub fn set_options(&self, options: &GuardOptions) -> Result<()> {
        self.update("guard.options_changed", OWNER, |c| {
            c.set_options(options)?;
            Ok(Some((json!({ "options": c.options }), ())))
        })?;
        Ok(())
    }

    /// Record a secret's reference (the Vault stores its value). Returns it.
    pub fn save_secret(&self, input: &SecretInput) -> Result<SecretInfo> {
        let now = plenipo_ledger::now_ms();
        let event = if input.id.is_some() {
            "vault.secret_changed"
        } else {
            "vault.secret_added"
        };
        self.update(event, OWNER, |c| {
            let s = c.save_secret(input, now)?;
            // Only the reference: never the value.
            Ok(Some((
                json!({ "secretId": s.id, "name": s.name, "envVar": s.env_var, "programs": s.programs }),
                s,
            )))
        })?
        .ok_or_else(|| GuardError::Invalid("nothing changed".into()))
    }

    pub fn remove_secret(&self, id: &str) -> Result<SecretInfo> {
        self.update("vault.secret_removed", OWNER, |c| {
            let s = c.remove_secret(id)?;
            Ok(Some((json!({ "secretId": s.id, "name": s.name }), s)))
        })?
        .ok_or_else(|| GuardError::Invalid("nothing changed".into()))
    }

    // ---- Reading ----------------------------------------------------------------------------

    /// The scope of an organization member's work, from its session's `workforce` metadata
    /// (`positionId`, and `projectId` for the project the work belongs to). `None` when it is
    /// not an organization member's.
    pub fn scope_for(&self, workforce: &Value) -> Result<Option<Scope>> {
        let Some(position_id) = workforce["positionId"].as_str() else {
            return Ok(None);
        };
        let records = self.ledger().org_records()?;
        Ok(scope_in(
            &records,
            position_id,
            workforce["projectId"].as_str(),
        ))
    }

    /// Everything Settings → Permissions shows.
    pub fn settings(&self) -> Result<GuardSettings> {
        let config = self.config()?;
        let roles = self.ledger().list_roles()?;
        let departments = self.ledger().list_departments()?;
        let projects = self.ledger().list_projects()?;
        Ok(GuardSettings {
            capabilities: Capability::ALL
                .iter()
                .map(|c| {
                    let i = c.info();
                    CapabilityInfo {
                        id: *c,
                        label: i.label.into(),
                        description: i.description.into(),
                        tools: i.tools,
                        arrives: i.arrives.map(str::to_owned),
                    }
                })
                .collect(),
            roles: roles
                .iter()
                .map(|r| RolePermissions {
                    role_id: r.id.clone(),
                    role_name: r.name.clone(),
                    full_time: r.persistent,
                    set_id: config.role_set(&r.id).map(str::to_owned),
                })
                .collect(),
            departments: departments
                .iter()
                .filter(|d| d.status == "active")
                .map(|d| UnitLimit {
                    id: d.id.clone(),
                    name: d.name.clone(),
                    set_id: config.departments.get(&d.id).cloned(),
                    folder: None,
                    problem: None,
                })
                .collect(),
            projects: projects
                .iter()
                .filter(|p| p.status == "active")
                .map(|p| UnitLimit {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    set_id: p.capability_profile.clone(),
                    folder: p.local_path.clone(),
                    problem: project_problem(&config, p),
                })
                .collect(),
            commands: config.commands.clone(),
            blocked_files: config.blocked_files.clone(),
            sensitive: SensitiveKind::ALL
                .iter()
                .map(|k| SensitiveInfo {
                    kind: *k,
                    label: k.label().into(),
                    examples: k.examples().into(),
                    rule: config.sensitive_rule(*k),
                })
                .collect(),
            options: config.options.clone(),
            secrets: config.secrets.clone(),
            sets: config.sets,
        })
    }
}

/// What the owner should know about a project's permissions, if anything.
fn project_problem(config: &GuardConfig, p: &plenipo_ledger::Project) -> Option<String> {
    if let Some(limit) = &p.capability_profile {
        if config.set(limit).is_none() {
            return Some(format!(
                "Its limit \"{limit}\" is not one of your permission sets, so its workers get no \
                 permissions. Edit the project and choose a set (or no limit)."
            ));
        }
    }
    match &p.local_path {
        None => Some(
            "It has no folder, so its workers cannot use files, programs, or git. Edit the \
             project to add one."
                .into(),
        ),
        Some(path) if !std::path::Path::new(path).is_dir() => Some(format!(
            "Its folder {path} does not exist on this computer."
        )),
        _ => None,
    }
}

/// The scope of `position_id`'s work (`project_id`: the project the work belongs to).
pub fn scope_in(
    records: &OrgRecords,
    position_id: &str,
    project_id: Option<&str>,
) -> Option<Scope> {
    let position = records.positions.iter().find(|p| p.id == position_id)?;
    let role = records.roles.iter().find(|r| r.id == position.role_id)?;
    let project = project_id.and_then(|id| records.projects.iter().find(|p| p.id == id));
    let department = project
        .and_then(|p| p.department_id.as_deref())
        .and_then(|d| records.departments.iter().find(|x| x.id == d))
        .or_else(|| department_of(records, position_id));
    Some(Scope {
        role_id: role.id.clone(),
        role_name: role.name.clone(),
        project: project.map(|p| ScopeProject {
            id: p.id.clone(),
            name: p.name.clone(),
            limit: p.capability_profile.clone(),
            folder: p.local_path.clone(),
        }),
        department: department.map(|d| ScopeUnit {
            id: d.id.clone(),
            name: d.name.clone(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::{RoleTemplate, RoleType};

    fn ledger() -> Arc<Ledger> {
        let l = Arc::new(Ledger::open_in_memory().unwrap());
        l.ensure_roles(
            &[
                RoleTemplate {
                    name: "Senior Developer",
                    description: "",
                    role_type: RoleType::Worker,
                    persistent: false,
                    metadata: json!({ "template": true }),
                    formerly: &[],
                },
                RoleTemplate {
                    name: "Code Reviewer",
                    description: "",
                    role_type: RoleType::Worker,
                    persistent: false,
                    metadata: json!({ "template": true }),
                    formerly: &[],
                },
            ],
            "test",
        )
        .unwrap();
        l
    }

    #[test]
    fn starting_settings_are_stored_once_and_roles_seeded_once() {
        let l = ledger();
        let g = Guard::new(l.clone());
        let c = g.config().unwrap();
        assert!(c.set("developer").is_some());
        assert!(!c.commands.approved.is_empty());
        g.seed_template_roles().unwrap();
        let roles = l.list_roles().unwrap();
        let dev = roles.iter().find(|r| r.name == "Senior Developer").unwrap();
        let rev = roles.iter().find(|r| r.name == "Code Reviewer").unwrap();
        assert_eq!(g.config().unwrap().role_set(&dev.id), Some("developer"));
        // The owner's choice — even none — is never replaced.
        g.assign_role(&rev.id, None).unwrap();
        g.seed_template_roles().unwrap();
        assert_eq!(g.config().unwrap().role_set(&rev.id), None);
        assert!(g.config().unwrap().roles.contains_key(&rev.id));
        // A second start changes nothing.
        let before = l.recent_events(100).unwrap().len();
        let g2 = Guard::new(l.clone());
        g2.seed_template_roles().unwrap();
        assert_eq!(l.recent_events(100).unwrap().len(), before);
        let types: Vec<String> = l
            .recent_events(100)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
        assert!(types.contains(&"guard.defaults_added".to_owned()));
        assert!(types.contains(&"guard.roles_seeded".to_owned()));
        assert!(types.contains(&"guard.role_assigned".to_owned()));
    }

    #[test]
    fn changes_are_validated_recorded_and_secret_values_never_stored() {
        let l = ledger();
        let g = Guard::new(l.clone());
        let set = g
            .save_set(&PermissionSetInput {
                name: "Docs".into(),
                levels: [(Capability::FilesystemRead, Level::Allowed)]
                    .into_iter()
                    .collect(),
                ..PermissionSetInput::default()
            })
            .unwrap();
        assert!(g.check_project_limit(Some(&set.id)).is_ok());
        assert!(g.check_project_limit(Some("nope")).is_err());
        assert!(g.check_project_limit(None).is_ok());
        assert!(g.assign_role("missing-role", Some(&set.id)).is_err());
        let refused = g
            .set_options(&GuardOptions {
                approval_minutes: 99,
            })
            .unwrap_err();
        assert!(refused.is_caller_error());
        assert!(
            !refused.to_string().starts_with("invalid input"),
            "refusals read plainly: {refused}"
        );
        let s = g
            .save_secret(&SecretInput {
                name: "Deploy token".into(),
                value: Some("super-secret-value".into()),
                ..SecretInput::default()
            })
            .unwrap();
        let everything = format!(
            "{}{:?}",
            l.setting(SETTING).unwrap().unwrap(),
            l.recent_events(100).unwrap()
        );
        assert!(!everything.contains("super-secret-value"));
        g.remove_secret(&s.id).unwrap();
        g.remove_set(&set.id).unwrap();
        let settings = g.settings().unwrap();
        assert_eq!(settings.capabilities.len(), 16);
        assert_eq!(settings.sensitive.len(), SensitiveKind::ALL.len());
        assert!(settings
            .roles
            .iter()
            .any(|r| r.role_name == "Code Reviewer"));
    }
}
