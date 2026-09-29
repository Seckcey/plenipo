//! Guard's configuration: permission sets, who has which, the owner's rules, and the Vault's
//! secret references, kept in the Ledger's `guard` setting as one document. Every change is
//! validated here before it is written.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::commands::valid_rule;
use crate::connections::{
    self, Access, Account, AccountKind, Connection, ConnectionState, OwnApp, Part, PartLevel, Who,
    MAX_ACCESS, MAX_CONNECTIONS, MAX_SEND_LIST,
};
use crate::defaults;
use crate::dto::*;
use crate::error::{GuardError, Result};
use crate::paths::valid_pattern;
use crate::servers::{self, Server, ServerInput, MAX_SERVERS};
use crate::websites::{self, WebsiteRules};

pub const MAX_SETS: usize = 64;
pub const MAX_RULES: usize = 300;
pub const MAX_SECRETS: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuardConfig {
    pub sets: Vec<PermissionSet>,
    /// Role ID → its permission set (`None`: the owner chose none). A role not listed has none.
    pub roles: BTreeMap<String, Option<String>>,
    /// Department ID → the permission set that limits its work.
    pub departments: BTreeMap<String, String>,
    pub commands: CommandRules,
    pub blocked_files: Vec<String>,
    /// Kinds not listed ask.
    pub sensitive: BTreeMap<SensitiveKind, SensitiveRule>,
    pub options: GuardOptions,
    /// References to secrets in the operating system's protected storage (never values).
    pub secrets: Vec<SecretInfo>,
    /// Which websites workers may open in Plenipo's browser (Phase 10, ADR-020).
    pub websites: WebsiteRules,
    /// The servers workers may use over SSH (Phase 11, ADR-025).
    pub servers: Vec<Server>,
    /// The owner's on/off switches (ADR-023). Missing in older documents: the defaults.
    pub switches: Switches,
    /// Which browser is Plenipo's browser (ADR-028). Missing in older documents: Automatic.
    pub browser_choice: BrowserChoice,
    /// The owner's connections (Phase 20, ADR-062): never their sign-ins, which only the Vault
    /// keeps. Missing in older documents: none.
    pub connections: Vec<Connection>,
}

fn invalid(message: impl Into<String>) -> GuardError {
    GuardError::Invalid(message.into())
}

/// Trimmed, 1–`max` characters, one line.
fn line(what: &str, value: &str, max: usize) -> Result<String> {
    let v = value.trim();
    if v.is_empty() || v.chars().count() > max || v.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{what} must be one line of 1–{max} characters"
        )));
    }
    Ok(v.to_owned())
}

/// A slug from a name: lower-case letters, digits, and dashes.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "set".into()
    } else {
        out.chars().take(40).collect()
    }
}

/// An environment variable name: letters, digits, `_`, not starting with a digit.
fn env_name(name: &str) -> Result<String> {
    let n = name.trim();
    let ok = (1..=64).contains(&n.len())
        && n.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    // Never let a secret replace what programs need to run.
    let reserved = [
        "PATH",
        "PATHEXT",
        "HOME",
        "USERPROFILE",
        "SYSTEMROOT",
        "COMSPEC",
        "TEMP",
        "TMP",
    ];
    if !ok || reserved.contains(&n.to_ascii_uppercase().as_str()) {
        return Err(invalid(format!(
            "{n:?} cannot be used as the environment variable name"
        )));
    }
    Ok(n.to_owned())
}

/// A program name a secret may be given to (`gh`, `npm`); no paths.
fn program_name(name: &str) -> Result<String> {
    let n = name.trim().to_lowercase();
    let ok = (1..=64).contains(&n.len())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !n.starts_with('.');
    if !ok {
        return Err(invalid(format!(
            "{name:?} is not a program name (use a name like gh or npm)"
        )));
    }
    Ok(n)
}

impl GuardConfig {
    /// Everything Plenipo starts with.
    pub fn with_defaults() -> Self {
        Self {
            sets: defaults::builtin_sets(),
            commands: defaults::default_commands(),
            blocked_files: defaults::default_blocked_files(),
            websites: defaults::default_websites(),
            ..Self::default()
        }
    }

    /// Read the stored document (`Null`: nothing stored yet).
    pub fn from_value(value: serde_json::Value) -> std::result::Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn set(&self, id: &str) -> Option<&PermissionSet> {
        self.sets.iter().find(|s| s.id == id)
    }

    /// Built-in sets missing from the list (added back; returns them).
    pub fn add_missing_builtins(&mut self) -> Vec<PermissionSet> {
        let mut added = Vec::new();
        for s in defaults::builtin_sets() {
            if self.set(&s.id).is_none() {
                self.sets.push(s.clone());
                added.push(s);
            }
        }
        added
    }

    /// Bring built-in sets the owner never changed up to date with this version's defaults
    /// (their levels and description exactly as an earlier version made them). Returns the IDs
    /// updated.
    pub fn upgrade_builtins(&mut self) -> Vec<String> {
        let current = defaults::builtin_sets();
        let mut updated = Vec::new();
        for earlier in defaults::earlier_sets() {
            let Some(now) = current.iter().find(|s| s.id == earlier.id) else {
                continue;
            };
            if let Some(s) = self.sets.iter_mut().find(|s| {
                s.id == earlier.id
                    && s.built_in
                    && s.levels == earlier.levels
                    && s.description == earlier.description
            }) {
                s.levels = now.levels.clone();
                s.description = now.description.clone();
                updated.push(s.id.clone());
            }
        }
        updated
    }

    /// The role's permission set ID (`None`: none).
    pub fn role_set(&self, role_id: &str) -> Option<&str> {
        self.roles.get(role_id).and_then(|s| s.as_deref())
    }

    pub fn sensitive_rule(&self, kind: SensitiveKind) -> SensitiveRule {
        self.sensitive.get(&kind).copied().unwrap_or_default()
    }

    /// Add or change a permission set. Returns it.
    pub fn save_set(&mut self, input: &PermissionSetInput) -> Result<PermissionSet> {
        let name = line("the permission set's name", &input.name, 60)?;
        let description = input.description.trim().to_owned();
        if description.chars().count() > 500 {
            return Err(invalid("the description is limited to 500 characters"));
        }
        let existing = match &input.id {
            Some(id) => Some(
                self.sets
                    .iter()
                    .position(|s| &s.id == id)
                    .ok_or_else(|| invalid("that permission set no longer exists"))?,
            ),
            None => None,
        };
        if self
            .sets
            .iter()
            .enumerate()
            .any(|(i, s)| Some(i) != existing && s.name.eq_ignore_ascii_case(&name))
        {
            return Err(invalid(format!(
                "a permission set named \"{name}\" already exists"
            )));
        }
        // Only the levels that are not "blocked" are kept (blocked is the default).
        let levels = input
            .levels
            .iter()
            .filter(|(_, l)| **l != Level::Blocked)
            .map(|(c, l)| (*c, *l))
            .collect();
        let saved = match existing {
            Some(i) => {
                let s = &mut self.sets[i];
                s.name = name;
                s.description = description;
                s.levels = levels;
                s.clone()
            }
            None => {
                if self.sets.len() >= MAX_SETS {
                    return Err(invalid(format!("at most {MAX_SETS} permission sets")));
                }
                let base = slug(&name);
                let mut id = base.clone();
                let mut n = 2;
                while self.set(&id).is_some() {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                let s = PermissionSet {
                    id,
                    name,
                    description,
                    levels,
                    built_in: false,
                };
                self.sets.push(s.clone());
                s
            }
        };
        Ok(saved)
    }

    /// Remove a permission set nothing uses. `projects`: names of projects whose limit it is.
    pub fn remove_set(&mut self, id: &str, projects: &[String]) -> Result<PermissionSet> {
        let i = self
            .sets
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| invalid("that permission set no longer exists"))?;
        if self.sets[i].built_in {
            return Err(invalid(
                "built-in permission sets can be changed but not removed",
            ));
        }
        let roles = self
            .roles
            .values()
            .filter(|s| s.as_deref() == Some(id))
            .count();
        let departments = self.departments.values().filter(|s| *s == id).count();
        if roles + departments + projects.len() > 0 {
            let mut users = Vec::new();
            if roles > 0 {
                users.push(format!("{roles} role(s)"));
            }
            if departments > 0 {
                users.push(format!("{departments} department(s)"));
            }
            if !projects.is_empty() {
                users.push(format!("the project(s) {}", projects.join(", ")));
            }
            return Err(invalid(format!(
                "{} is still used by {}; choose another set for them first",
                self.sets[i].name,
                users.join(" and ")
            )));
        }
        Ok(self.sets.remove(i))
    }

    /// Give a role a permission set (`None`: none).
    pub fn assign_role(&mut self, role_id: &str, set_id: Option<&str>) -> Result<()> {
        if let Some(id) = set_id {
            self.set(id)
                .ok_or_else(|| invalid("that permission set no longer exists"))?;
        }
        self.roles
            .insert(role_id.to_owned(), set_id.map(str::to_owned));
        Ok(())
    }

    /// Limit a department's work to a permission set (`None`: no limit).
    pub fn assign_department(&mut self, department_id: &str, set_id: Option<&str>) -> Result<()> {
        match set_id {
            Some(id) => {
                self.set(id)
                    .ok_or_else(|| invalid("that permission set no longer exists"))?;
                self.departments
                    .insert(department_id.to_owned(), id.to_owned());
            }
            None => {
                self.departments.remove(department_id);
            }
        }
        Ok(())
    }

    pub fn set_commands(&mut self, rules: &CommandRules) -> Result<()> {
        let clean = |what: &str, list: &[String]| -> Result<Vec<String>> {
            if list.len() > MAX_RULES {
                return Err(invalid(format!("at most {MAX_RULES} {what}")));
            }
            let mut out: Vec<String> = Vec::new();
            for r in list.iter().filter(|r| !r.trim().is_empty()) {
                let r = valid_rule(r).map_err(invalid)?;
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            Ok(out)
        };
        // ADR-048: a rule that names a program and the stored secrets it may be given.
        if rules.with_secrets.len() > MAX_RULES {
            return Err(invalid(format!(
                "at most {MAX_RULES} commands given stored secrets"
            )));
        }
        let mut with_secrets: Vec<SecretRule> = Vec::new();
        for r in &rules.with_secrets {
            let rule = valid_rule(&r.rule).map_err(invalid)?;
            let mut secrets: Vec<String> = Vec::new();
            for name in r.secrets.iter().filter(|n| !n.trim().is_empty()) {
                let name = line("a secret's name", name, 60)?;
                if !secrets.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                    secrets.push(name);
                }
            }
            if secrets.is_empty() {
                return Err(invalid(format!(
                    "\"{rule}\" names no stored secret; name the secret it may be given, or \
                     put it on the approved list instead"
                )));
            }
            let entry = SecretRule { rule, secrets };
            if !with_secrets.contains(&entry) {
                with_secrets.push(entry);
            }
        }
        self.commands = CommandRules {
            approved: clean("approved commands", &rules.approved)?,
            ask: clean("always-ask commands", &rules.ask)?,
            blocked: clean("blocked commands", &rules.blocked)?,
            with_secrets,
        };
        Ok(())
    }

    pub fn set_blocked_files(&mut self, patterns: &[String]) -> Result<()> {
        if patterns.len() > MAX_RULES {
            return Err(invalid(format!("at most {MAX_RULES} file patterns")));
        }
        let mut out: Vec<String> = Vec::new();
        for p in patterns.iter().filter(|p| !p.trim().is_empty()) {
            let p = valid_pattern(p).map_err(invalid)?;
            if !out.contains(&p) {
                out.push(p);
            }
        }
        self.blocked_files = out;
        Ok(())
    }

    pub fn set_websites(&mut self, rules: &WebsiteRules) -> Result<()> {
        self.websites = websites::clean(rules).map_err(invalid)?;
        Ok(())
    }

    pub fn set_switches(&mut self, switches: &Switches) {
        self.switches = switches.clone();
    }

    pub fn set_browser_choice(&mut self, choice: BrowserChoice) {
        self.browser_choice = choice;
    }

    pub fn set_sensitive(&mut self, kind: SensitiveKind, rule: SensitiveRule) {
        match rule {
            SensitiveRule::Ask => self.sensitive.remove(&kind),
            SensitiveRule::Block => self.sensitive.insert(kind, rule),
        };
    }

    pub fn set_options(&mut self, options: &GuardOptions) -> Result<()> {
        if !(1..=MAX_APPROVAL_MINUTES).contains(&options.approval_minutes) {
            return Err(invalid(format!(
                "approvals can wait 1–{MAX_APPROVAL_MINUTES} minutes"
            )));
        }
        self.options = options.clone();
        Ok(())
    }

    /// Add or change a secret's reference (never its value). Returns it.
    pub fn save_secret(&mut self, input: &SecretInput, now: u64) -> Result<SecretInfo> {
        let name = line("the secret's name", &input.name, 60)?;
        let env_var = input
            .env_var
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(env_name)
            .transpose()?;
        let mut programs: Vec<String> = Vec::new();
        for p in input.programs.iter().filter(|p| !p.trim().is_empty()) {
            let p = program_name(p)?;
            if !programs.contains(&p) {
                programs.push(p);
            }
        }
        if programs.len() > 16 {
            return Err(invalid("a secret can go to at most 16 programs"));
        }
        if env_var.is_some() != !programs.is_empty() {
            return Err(invalid(
                "to give a secret to programs, name both the environment variable and the programs",
            ));
        }
        let existing = match &input.id {
            Some(id) => Some(
                self.secrets
                    .iter()
                    .position(|s| &s.id == id)
                    .ok_or_else(|| invalid("that secret no longer exists"))?,
            ),
            None => None,
        };
        if self
            .secrets
            .iter()
            .enumerate()
            .any(|(i, s)| Some(i) != existing && s.name.eq_ignore_ascii_case(&name))
        {
            return Err(invalid(format!("a secret named \"{name}\" already exists")));
        }
        let saved = match existing {
            Some(i) => {
                let s = &mut self.secrets[i];
                s.name = name;
                s.env_var = env_var;
                s.programs = programs;
                s.updated_at = now;
                s.clone()
            }
            None => {
                if self.secrets.len() >= MAX_SECRETS {
                    return Err(invalid(format!("at most {MAX_SECRETS} secrets")));
                }
                let s = SecretInfo {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    env_var,
                    programs,
                    created_at: now,
                    updated_at: now,
                };
                self.secrets.push(s.clone());
                s
            }
        };
        Ok(saved)
    }

    pub fn server(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// A server by its name (as workers give it), ignoring case.
    pub fn server_named(&self, name: &str) -> Option<&Server> {
        let name = name.trim();
        self.servers
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Add or change a server (never its secret values). `roles`: the IDs of roles that exist.
    /// Returns it, and the one it replaced.
    pub fn save_server(
        &mut self,
        input: &ServerInput,
        roles: &[String],
        now: u64,
    ) -> Result<(Server, Option<Server>)> {
        let existing = match &input.id {
            Some(id) => Some(
                self.servers
                    .iter()
                    .position(|s| &s.id == id)
                    .ok_or_else(|| invalid("that server is no longer in the list"))?,
            ),
            None => None,
        };
        if let Some(r) = input.roles.iter().find(|r| !roles.contains(r)) {
            return Err(invalid(format!("the role {r:?} no longer exists")));
        }
        let earlier = existing.map(|i| self.servers[i].clone());
        let id = earlier
            .as_ref()
            .map_or_else(|| uuid::Uuid::new_v4().to_string(), |s| s.id.clone());
        let server = servers::clean(input, &id, earlier.as_ref(), now).map_err(invalid)?;
        if self
            .servers
            .iter()
            .enumerate()
            .any(|(i, s)| Some(i) != existing && s.name.eq_ignore_ascii_case(&server.name))
        {
            return Err(invalid(format!(
                "a server named \"{}\" is already in the list",
                server.name
            )));
        }
        match existing {
            Some(i) => self.servers[i] = server.clone(),
            None => {
                if self.servers.len() >= MAX_SERVERS {
                    return Err(invalid(format!("at most {MAX_SERVERS} servers")));
                }
                self.servers.push(server.clone());
            }
        }
        Ok((server, earlier))
    }

    pub fn remove_server(&mut self, id: &str) -> Result<Server> {
        let i = self
            .servers
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| invalid("that server is no longer in the list"))?;
        Ok(self.servers.remove(i))
    }

    // ---- Connections (Phase 20) ---------------------------------------------------------------

    pub fn connection(&self, id: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// The connection `id`, or a new one when `id` belongs to a service Plenipo knows.
    pub fn connection_or_new(&self, id: &str) -> Result<Connection> {
        if let Some(c) = self.connection(id) {
            return Ok(c.clone());
        }
        let service = connections::service_of(id)
            .ok_or_else(|| invalid(format!("Plenipo has no connection called {id:?}")))?;
        Ok(Connection::new(id, service))
    }

    fn put_connection(&mut self, c: Connection) -> Result<Connection> {
        match self.connections.iter_mut().find(|x| x.id == c.id) {
            Some(x) => *x = c.clone(),
            None => {
                if self.connections.len() >= MAX_CONNECTIONS {
                    return Err(invalid(format!("at most {MAX_CONNECTIONS} connections")));
                }
                self.connections.push(c.clone());
            }
        }
        Ok(c)
    }

    /// Set the levels of a connection's parts (parts not given keep theirs).
    pub fn set_connection_parts(
        &mut self,
        id: &str,
        parts: &BTreeMap<Part, PartLevel>,
    ) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        for (part, level) in parts {
            if !c.service.parts().contains(part) {
                return Err(invalid(format!(
                    "{} has no part called {}",
                    c.label(),
                    part.label()
                )));
            }
            if *level == PartLevel::FullAccess && !part.has_full_access() {
                return Err(invalid(format!(
                    "{}'s {} only reads: it is Off or Read only",
                    c.label(),
                    part.label()
                )));
            }
            c.parts.insert(*part, *level);
        }
        self.put_connection(c)
    }

    /// Set a connection's **Who may use it** list. `roles` and `agents`: the IDs that exist.
    pub fn set_connection_access(
        &mut self,
        id: &str,
        access: &[Access],
        roles: &[String],
        agents: &[String],
    ) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        if access.len() > MAX_ACCESS {
            return Err(invalid(format!(
                "at most {MAX_ACCESS} lines on Who may use it"
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for a in access {
            // A line already on the list stays even when its role or agent is gone (it gives
            // nobody anything, and the owner can remove it); a new one must exist.
            let kept = c.access.iter().any(|x| x.who == a.who);
            match &a.who {
                Who::Role { id } if !kept && !roles.contains(id) => {
                    return Err(invalid("a role you added no longer exists"))
                }
                Who::Agent { id } if !kept && !agents.contains(id) => {
                    return Err(invalid("an agent you added no longer exists"))
                }
                _ => {}
            }
            if !seen.insert(&a.who) {
                return Err(invalid("a role or an agent is on Who may use it only once"));
            }
        }
        c.access = access.to_vec();
        self.put_connection(c)
    }

    /// Set a connection's **Send without asking to** list.
    pub fn set_connection_send_list(&mut self, id: &str, list: &[String]) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        if list.len() > MAX_SEND_LIST {
            return Err(invalid(format!(
                "at most {MAX_SEND_LIST} entries on Send without asking to"
            )));
        }
        let mut entries: Vec<String> = Vec::new();
        for e in list {
            let e = connections::send_entry(e, c.service).map_err(invalid)?;
            if !entries.iter().any(|x| x.eq_ignore_ascii_case(&e)) {
                entries.push(e);
            }
        }
        c.send_list = entries;
        self.put_connection(c)
    }

    /// Use an organization's own app instead of 8 West's (`None`: 8 West's). Only while the
    /// connection is not connected: its sign-in belongs to the app it was made with.
    pub fn set_connection_own_app(&mut self, id: &str, app: Option<&OwnApp>) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        if c.service != connections::Service::Microsoft365 {
            return Err(invalid(format!(
                "{} does not take an organization's own app yet",
                c.label()
            )));
        }
        if c.state != ConnectionState::NotConnected {
            return Err(invalid(format!(
                "Disconnect {} first: its sign-in belongs to the app it was made with",
                c.label()
            )));
        }
        c.own_app = match app {
            None => None,
            Some(a) => {
                let app_id = a.app_id.trim().to_lowercase();
                if !connections::is_guid(&app_id) {
                    return Err(invalid(
                        "the app ID must look like 12345678-abcd-4ef0-9abc-0123456789ab",
                    ));
                }
                let tenant = a
                    .tenant
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                if !(connections::is_guid(&tenant) || connections::is_domain(&tenant)) {
                    return Err(invalid(
                        "the organization must be its domain (contoso.com) or its ID",
                    ));
                }
                if a.secret_kept {
                    return Err(invalid("a Microsoft app needs no secret"));
                }
                Some(OwnApp {
                    app_id,
                    tenant: Some(tenant),
                    secret_kept: false,
                })
            }
        };
        self.put_connection(c)
    }

    /// Use the owner's own Slack or Google app (`None`: none — 8 West's for Slack; Google has no
    /// other), by its client ID. `secret_kept`: a Google app's secret is already in the Vault
    /// (ADR-069 §4). Only while the connection is not connected.
    pub fn set_connection_client_app(
        &mut self,
        id: &str,
        client_id: Option<&str>,
        secret_kept: bool,
    ) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        if c.state != ConnectionState::NotConnected {
            return Err(invalid(format!(
                "Disconnect {} first: its sign-in belongs to the app it was made with",
                c.label()
            )));
        }
        c.own_app = match (c.service, client_id) {
            (connections::Service::Slack | connections::Service::Google, None) => None,
            (connections::Service::Slack, Some(id)) => {
                let id = id.trim();
                if !connections::is_slack_client_id(id) {
                    return Err(invalid(
                        "a Slack app's client ID looks like 1234567890.9876543210",
                    ));
                }
                if secret_kept {
                    return Err(invalid("a Slack app signs in with no secret"));
                }
                Some(OwnApp {
                    app_id: id.to_owned(),
                    tenant: None,
                    secret_kept: false,
                })
            }
            (connections::Service::Google, Some(id)) => {
                let id = id.trim().to_lowercase();
                if !connections::is_google_client_id(&id) {
                    return Err(invalid(
                        "a Google app's client ID looks like \
                         1234567890-abc123.apps.googleusercontent.com",
                    ));
                }
                if !secret_kept {
                    return Err(invalid("a Google app needs its client secret too"));
                }
                Some(OwnApp {
                    app_id: id,
                    tenant: None,
                    secret_kept: true,
                })
            }
            _ => {
                return Err(invalid(format!(
                    "{} does not take a client ID here",
                    c.label()
                )))
            }
        };
        self.put_connection(c)
    }

    /// Add another account of a service that may have more than one (Slack's workspaces): its
    /// next free ID (`slack-2`, `slack-3`, …). The first one (`slack`), shown before anything was
    /// kept, is kept first, so the new card is always a second one on screen.
    pub fn add_connection(&mut self, service: connections::Service) -> Result<Connection> {
        if !service.many() {
            return Err(invalid(format!(
                "Plenipo keeps one {} account",
                service.label()
            )));
        }
        let kept = |cfg: &Self, id: &str| cfg.connection(id).is_some();
        if !self.connections.iter().any(|c| c.service == service) {
            self.put_connection(Connection::new(service.id(), service))?;
        }
        let id = std::iter::once(service.id().to_owned())
            .chain((2..=999).map(|n| format!("{}-{n}", service.id())))
            .find(|id| !kept(self, id))
            .ok_or_else(|| invalid(format!("at most 999 {} accounts", service.label())))?;
        self.put_connection(Connection::new(&id, service))
    }

    /// Remove a card that is not connected (a Slack workspace the owner no longer wants).
    pub fn remove_connection(&mut self, id: &str) -> Result<Connection> {
        let c = self.connection_or_new(id)?;
        if !c.service.many() {
            return Err(invalid(format!(
                "{} has one card; disconnect it instead",
                c.label()
            )));
        }
        if c.state != ConnectionState::NotConnected {
            return Err(invalid(format!(
                "Disconnect this {} workspace first",
                c.label()
            )));
        }
        self.connections.retain(|x| x.id != id);
        Ok(c)
    }

    /// A sign-in finished: the account, and what the service granted.
    pub fn connection_connected(
        &mut self,
        id: &str,
        kind: Option<AccountKind>,
        account: Account,
        granted: &[String],
        now: u64,
    ) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        c.state = ConnectionState::Connected;
        c.account_kind = kind;
        c.account = Some(account);
        c.granted = granted.to_vec();
        c.connected_at = Some(now);
        self.put_connection(c)
    }

    /// The service refused the connection's sign-in. `None`: it was not connected.
    pub fn connection_needs_sign_in(&mut self, id: &str) -> Result<Option<Connection>> {
        let Some(mut c) = self.connection(id).cloned() else {
            return Ok(None);
        };
        if c.state != ConnectionState::Connected {
            return Ok(None);
        }
        c.state = ConnectionState::NeedsSignIn;
        self.put_connection(c).map(Some)
    }

    /// Disconnected: the account and what was granted are forgotten; its parts, who may use
    /// it, and its list stay, so connecting again restores them (ADR-063 §5).
    pub fn connection_disconnected(&mut self, id: &str) -> Result<Connection> {
        let mut c = self.connection_or_new(id)?;
        c.state = ConnectionState::NotConnected;
        c.account_kind = None;
        c.account = None;
        c.granted = Vec::new();
        c.connected_at = None;
        self.put_connection(c)
    }

    pub fn remove_secret(&mut self, id: &str) -> Result<SecretInfo> {
        let i = self
            .secrets
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| invalid("that secret no longer exists"))?;
        Ok(self.secrets.remove(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Capability;

    fn input(name: &str, levels: &[(Capability, Level)]) -> PermissionSetInput {
        PermissionSetInput {
            id: None,
            name: name.into(),
            description: String::new(),
            levels: levels.iter().copied().collect(),
        }
    }

    #[test]
    fn unchanged_builtin_sets_gain_github_and_changed_ones_are_left_alone() {
        let mut c = GuardConfig::with_defaults();
        // The sets as an earlier version stored them; the owner changed the Reviewer set.
        for earlier in defaults::earlier_sets() {
            let s = c.sets.iter_mut().find(|s| s.id == earlier.id).unwrap();
            *s = earlier;
        }
        c.sets
            .iter_mut()
            .find(|s| s.id == "reviewer")
            .unwrap()
            .levels
            .insert(Capability::ShellExec, Level::Allowed);
        assert_eq!(
            c.upgrade_builtins(),
            ["writer", "researcher", "read-only", "developer", "tester"]
        );
        let writer = c.set("writer").unwrap();
        assert_eq!(
            writer.levels.get(&Capability::GitWrite),
            Some(&Level::Allowed),
            "the Writer set commits (ADR-019)"
        );
        let developer = c.set("developer").unwrap();
        assert_eq!(
            developer.levels.get(&Capability::GithubWrite),
            Some(&Level::Allowed)
        );
        assert!(developer.description.contains("pull requests"));
        assert!(!c
            .set("reviewer")
            .unwrap()
            .levels
            .contains_key(&Capability::GithubRead));
        assert!(c.upgrade_builtins().is_empty(), "only once");
        assert_eq!(
            c.sets,
            {
                let mut fresh = GuardConfig::with_defaults();
                let r = fresh.sets.iter_mut().find(|s| s.id == "reviewer").unwrap();
                *r = c.set("reviewer").unwrap().clone();
                fresh.sets
            },
            "the others match this version's defaults"
        );
    }

    #[test]
    fn permission_sets_are_added_changed_and_removed() {
        let mut c = GuardConfig::with_defaults();
        let s = c
            .save_set(&input(
                "Docs & Tests",
                &[
                    (Capability::FilesystemRead, Level::Allowed),
                    (Capability::ShellExec, Level::Blocked),
                ],
            ))
            .unwrap();
        assert_eq!(s.id, "docs-tests");
        assert_eq!(s.levels.len(), 1, "blocked is the default, not stored");
        assert!(
            c.save_set(&input("docs & TESTS", &[])).is_err(),
            "unique names"
        );
        let again = c.save_set(&input("Docs and tests", &[])).unwrap();
        assert_eq!(again.id, "docs-and-tests");
        let mut change = input("Docs", &[]);
        change.id = Some(s.id.clone());
        assert_eq!(c.save_set(&change).unwrap().name, "Docs");

        c.assign_role("role-1", Some("docs-tests")).unwrap();
        assert!(c.remove_set("docs-tests", &[]).is_err(), "used by a role");
        c.assign_role("role-1", None).unwrap();
        assert_eq!(c.role_set("role-1"), None);
        assert!(c.remove_set("docs-tests", &["Website".into()]).is_err());
        assert!(c.remove_set("docs-tests", &[]).is_ok());
        assert!(c.remove_set("developer", &[]).is_err(), "built-in");
        assert!(c.assign_role("r", Some("missing")).is_err());
        c.assign_department("d", Some("read-only")).unwrap();
        assert_eq!(c.departments["d"], "read-only");
        c.assign_department("d", None).unwrap();
        assert!(c.departments.is_empty());
    }

    #[test]
    fn builtins_come_back() {
        let mut c = GuardConfig::default();
        assert_eq!(
            c.add_missing_builtins().len(),
            defaults::builtin_sets().len()
        );
        assert!(c.add_missing_builtins().is_empty());
    }

    #[test]
    fn rules_are_validated() {
        let mut c = GuardConfig::default();
        c.set_commands(&CommandRules {
            approved: vec!["cargo  test *".into(), "cargo test *".into(), " ".into()],
            ask: vec![],
            blocked: vec!["rm *".into()],
            with_secrets: vec![],
        })
        .unwrap();
        assert_eq!(c.commands.approved, ["cargo test *"]);
        // ADR-048: a rule naming a program and its secrets is cleaned like the lists, and
        // must name at least one secret.
        c.set_commands(&CommandRules {
            with_secrets: vec![
                SecretRule {
                    rule: " gh  pr * ".into(),
                    secrets: vec![" GitHub token ".into(), "github TOKEN".into(), "".into()],
                },
                SecretRule {
                    rule: "gh pr *".into(),
                    secrets: vec!["GitHub token".into()],
                },
            ],
            ..CommandRules::default()
        })
        .unwrap();
        assert_eq!(
            c.commands.with_secrets,
            [SecretRule {
                rule: "gh pr *".into(),
                secrets: vec!["GitHub token".into()],
            }]
        );
        for bad in [
            SecretRule {
                rule: "gh *".into(),
                secrets: vec![],
            },
            SecretRule {
                rule: "/usr/bin/gh *".into(),
                secrets: vec!["GitHub token".into()],
            },
            SecretRule {
                rule: "gh *".into(),
                secrets: vec!["x".repeat(61)],
            },
        ] {
            assert!(
                c.set_commands(&CommandRules {
                    with_secrets: vec![bad.clone()],
                    ..CommandRules::default()
                })
                .is_err(),
                "{bad:?}"
            );
        }
        assert!(c
            .set_commands(&CommandRules {
                approved: vec!["/bin/rm *".into()],
                ..CommandRules::default()
            })
            .is_err());
        c.set_blocked_files(&["*.pem".into(), "!".into()])
            .unwrap_err();
        c.set_blocked_files(&["*.pem".into()]).unwrap();
        c.set_sensitive(SensitiveKind::Dns, SensitiveRule::Block);
        assert_eq!(c.sensitive_rule(SensitiveKind::Dns), SensitiveRule::Block);
        c.set_sensitive(SensitiveKind::Dns, SensitiveRule::Ask);
        assert!(c.sensitive.is_empty());
        assert!(c
            .set_options(&GuardOptions {
                approval_minutes: 0
            })
            .is_err());
        c.set_options(&GuardOptions {
            approval_minutes: 5,
        })
        .unwrap();
    }

    #[test]
    fn connections_keep_settings_never_sign_ins() {
        use crate::connections::*;
        let mut c = GuardConfig::with_defaults();
        let roles = vec!["writer".to_owned()];
        let agents = vec!["pos-1".to_owned()];
        // A known service's connection starts with Mail and Calendar at Read only.
        let m = c.connection_or_new("microsoft365").unwrap();
        assert_eq!(m.part(Part::Mail), PartLevel::ReadOnly);
        assert_eq!(m.part(Part::Teams), PartLevel::Off);
        assert!(c.connection_or_new("myspace").is_err());
        // Parts.
        let mut parts = BTreeMap::new();
        parts.insert(Part::Teams, PartLevel::FullAccess);
        let m = c.set_connection_parts("microsoft365", &parts).unwrap();
        assert_eq!(m.part(Part::Teams), PartLevel::FullAccess);
        assert_eq!(
            m.part(Part::Mail),
            PartLevel::ReadOnly,
            "untouched parts keep their level"
        );
        // Who may use it: known roles and agents, once each.
        let line = |who: Who| Access {
            who,
            level: AccessLevel::ReadOnly,
        };
        assert!(c
            .set_connection_access(
                "microsoft365",
                &[line(Who::Role {
                    id: "nobody".into()
                })],
                &roles,
                &agents
            )
            .is_err());
        assert!(c
            .set_connection_access(
                "microsoft365",
                &[
                    line(Who::Role {
                        id: "writer".into()
                    }),
                    line(Who::Role {
                        id: "writer".into()
                    })
                ],
                &roles,
                &agents
            )
            .is_err());
        c.set_connection_access(
            "microsoft365",
            &[
                line(Who::Role {
                    id: "writer".into(),
                }),
                line(Who::Agent { id: "pos-1".into() }),
            ],
            &roles,
            &agents,
        )
        .unwrap();
        // The agent is removed later: its line stays (it gives nobody anything), and the owner
        // can still change the other lines, or take it off; a new unknown one is refused, in
        // words without its ID.
        let gone: Vec<String> = Vec::new();
        c.set_connection_access(
            "microsoft365",
            &[
                Access {
                    who: Who::Role {
                        id: "writer".into(),
                    },
                    level: AccessLevel::ReadWrite,
                },
                line(Who::Agent { id: "pos-1".into() }),
            ],
            &roles,
            &gone,
        )
        .unwrap();
        c.set_connection_access(
            "microsoft365",
            &[line(Who::Role {
                id: "writer".into(),
            })],
            &roles,
            &gone,
        )
        .unwrap();
        let err = c
            .set_connection_access(
                "microsoft365",
                &[line(Who::Agent { id: "pos-2".into() })],
                &roles,
                &gone,
            )
            .unwrap_err()
            .to_string();
        assert!(err.contains("an agent you added no longer exists"), "{err}");
        assert!(!err.contains("pos-2"), "{err}");
        // The list: checked, lower case, once each.
        assert!(c
            .set_connection_send_list("microsoft365", &["not an address".into()])
            .is_err());
        let m = c
            .set_connection_send_list(
                "microsoft365",
                &["@8WestIT.com".into(), "@8westit.com".into()],
            )
            .unwrap();
        assert_eq!(m.send_list, vec!["@8westit.com".to_owned()]);
        // An organization's own app: a GUID and a domain, and only while not connected.
        assert!(c
            .set_connection_own_app(
                "microsoft365",
                Some(&OwnApp {
                    app_id: "nope".into(),
                    tenant: Some("contoso.com".into()),
                    secret_kept: false,
                })
            )
            .is_err());
        let app = OwnApp {
            app_id: "12345678-ABCD-4ef0-9abc-0123456789ab".into(),
            tenant: Some("Contoso.com".into()),
            secret_kept: false,
        };
        // No organization, or a secret: refused.
        for bad in [
            OwnApp {
                tenant: None,
                ..app.clone()
            },
            OwnApp {
                secret_kept: true,
                ..app.clone()
            },
        ] {
            assert!(c
                .set_connection_own_app("microsoft365", Some(&bad))
                .is_err());
        }
        let m = c
            .set_connection_own_app("microsoft365", Some(&app))
            .unwrap();
        assert_eq!(
            m.own_app.unwrap().app_id,
            "12345678-abcd-4ef0-9abc-0123456789ab"
        );
        // Connected, then disconnected: the account goes; the settings stay.
        let account = Account {
            name: "Frankie".into(),
            address: "frankie@8westit.com".into(),
            organization: Some("8 West IT".into()),
            tenant: None,
        };
        c.connection_connected(
            "microsoft365",
            Some(AccountKind::Work),
            account,
            &["Mail.Read".into()],
            1,
        )
        .unwrap();
        assert!(c.set_connection_own_app("microsoft365", None).is_err());
        assert!(c
            .connection_needs_sign_in("microsoft365")
            .unwrap()
            .is_some());
        let m = c.connection_disconnected("microsoft365").unwrap();
        assert_eq!(m.state, ConnectionState::NotConnected);
        assert_eq!(m.account, None);
        assert!(m.granted.is_empty());
        assert_eq!(m.access.len(), 1);
        assert_eq!(m.send_list.len(), 1);
        // Nothing like a token is ever part of the document.
        let doc = c.to_value().to_string();
        assert!(!doc.to_lowercase().contains("token"), "{doc}");
    }

    #[test]
    fn slack_workspaces_and_the_owners_own_apps() {
        use crate::connections::*;
        let mut c = GuardConfig::default();
        // Slack starts with Channels at Read only; Google with Gmail and Calendar.
        let s = c.connection_or_new("slack").unwrap();
        assert_eq!(s.part(Part::Channels), PartLevel::ReadOnly);
        assert_eq!(s.part(Part::DirectMessages), PartLevel::Off);
        let g = c.connection_or_new("google").unwrap();
        assert_eq!(g.part(Part::Gmail), PartLevel::ReadOnly);
        assert_eq!(g.part(Part::Calendar), PartLevel::ReadOnly);
        assert_eq!(g.part(Part::Drive), PartLevel::Off);
        // Search only reads; a part of another service is refused.
        let mut full = BTreeMap::new();
        full.insert(Part::Search, PartLevel::FullAccess);
        assert!(c.set_connection_parts("slack", &full).is_err());
        let mut mail = BTreeMap::new();
        mail.insert(Part::Mail, PartLevel::ReadOnly);
        assert!(c.set_connection_parts("google", &mail).is_err());
        // Adding a workspace keeps the first card and makes the next free one.
        assert_eq!(c.add_connection(Service::Slack).unwrap().id, "slack-2");
        assert!(c.connection("slack").is_some());
        assert_eq!(c.add_connection(Service::Slack).unwrap().id, "slack-3");
        assert!(c.add_connection(Service::Google).is_err());
        // Removing a card that is not connected; then its ID is free again.
        c.remove_connection("slack-2").unwrap();
        assert_eq!(c.add_connection(Service::Slack).unwrap().id, "slack-2");
        assert!(c.remove_connection("google").is_err());
        let account = Account {
            name: "Frankie".into(),
            address: "frankie@8westit.com".into(),
            organization: Some("8 West IT".into()),
            tenant: Some("T0800000001".into()),
        };
        c.connection_connected("slack-3", None, account, &["channels:read".into()], 1)
            .unwrap();
        assert!(c.remove_connection("slack-3").is_err());
        // A Slack channel can be on a Slack list by its ID; never on Microsoft's or Google's.
        let l = c
            .set_connection_send_list("slack", &["c0100000001".into(), "@8westit.com".into()])
            .unwrap();
        assert_eq!(l.send_list, ["C0100000001", "@8westit.com"]);
        assert!(c
            .set_connection_send_list("slack", &["#general".into()])
            .is_err());
        assert!(c
            .set_connection_send_list("google", &["C0100000001".into()])
            .is_err());
        assert!(c
            .set_connection_send_list("microsoft365", &["C0100000001".into()])
            .is_err());
        // The owner's own apps: Slack's client ID with no secret; Google's with its secret kept.
        let own = c
            .set_connection_client_app("slack", Some("1234567890.9876543210"), false)
            .unwrap();
        assert_eq!(own.own_app.unwrap().tenant, None);
        assert!(c
            .set_connection_client_app("slack", Some("1234567890.9876543210"), true)
            .is_err());
        assert!(c
            .set_connection_client_app("slack", Some("not-an-id"), false)
            .is_err());
        let gid = "123456789012-abcdef123.apps.googleusercontent.com";
        assert!(c
            .set_connection_client_app("google", Some(gid), false)
            .is_err());
        let own = c
            .set_connection_client_app("google", Some(gid), true)
            .unwrap();
        assert!(own.own_app.unwrap().secret_kept);
        assert!(c
            .set_connection_client_app("google", Some("evil.example"), true)
            .is_err());
        assert!(c
            .set_connection_client_app("microsoft365", Some(gid), true)
            .is_err());
        assert!(c
            .set_connection_client_app("slack-3", Some("1.2"), false)
            .is_err());
        assert_eq!(
            c.set_connection_client_app("google", None, false)
                .unwrap()
                .own_app,
            None
        );
    }

    #[test]
    fn secrets_are_references_only() {
        let mut c = GuardConfig::default();
        let s = c
            .save_secret(
                &SecretInput {
                    id: None,
                    name: "GitHub token".into(),
                    env_var: Some("GH_TOKEN".into()),
                    programs: vec!["GH".into(), "gh".into()],
                    value: Some("ghp_x".into()),
                },
                1,
            )
            .unwrap();
        assert_eq!(s.programs, ["gh"]);
        assert!(!serde_json::to_string(&c).unwrap().contains("ghp_x"));
        for bad in [
            SecretInput {
                name: "x".into(),
                env_var: Some("PATH".into()),
                programs: vec!["gh".into()],
                ..SecretInput::default()
            },
            SecretInput {
                name: "y".into(),
                env_var: Some("TOKEN".into()),
                ..SecretInput::default()
            },
            SecretInput {
                name: "z".into(),
                env_var: Some("TOKEN".into()),
                programs: vec!["/bin/gh".into()],
                ..SecretInput::default()
            },
            SecretInput {
                name: "github TOKEN".into(),
                ..SecretInput::default()
            },
        ] {
            assert!(c.save_secret(&bad, 2).is_err(), "{bad:?}");
        }
        assert!(c.remove_secret(&s.id).is_ok());
        assert!(c.remove_secret(&s.id).is_err());
    }
}
