//! The evaluation engine (pure): a worker's permission level for a capability from its layers,
//! and Guard's decision about one action. The plan's layers, in order: role policy, project
//! policy, department policy, target resource, action risk class, explicit user approval rules.
//! The strictest answer wins.

use std::collections::BTreeMap;
use std::path::Path;

use crate::add_ons::{AddOn, AddOnCheck, ToolMark};
use crate::commands::{first_catch, first_match, rule_matches, CommandLine};
use crate::config::GuardConfig;
use crate::connections::{self, AccessLevel, Connection, ConnectionCheck, ConnectionVerdict};
use crate::dto::*;
use crate::paths::blocked_by;
use crate::registry::Capability;
use crate::sensitive;
use crate::servers::{self, ServerCheck, ServerVerdict};
use crate::websites::{self, Site, SiteVerdict};

/// Who is acting, as far as permissions go: the worker's role, and the project and department
/// its work belongs to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub role_id: String,
    pub role_name: String,
    /// The agent (its position), when known: a connection's own line for it wins over its
    /// role's (ADR-062 §3).
    pub position_id: Option<String>,
    pub project: Option<ScopeProject>,
    pub department: Option<ScopeUnit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeProject {
    pub id: String,
    pub name: String,
    /// The project's limit (a permission set ID); `None`: no limit.
    pub limit: Option<String>,
    /// The project's folder, as recorded.
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeUnit {
    pub id: String,
    pub name: String,
}

/// A worker's level for one capability, and each layer's note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelFor {
    pub level: Level,
    /// The strictest layer (the one that decided).
    pub layer: Layer,
    /// Why, as a clause: "the Developer set of the Senior Developer role allows it".
    pub reason: String,
    pub checks: Vec<Check>,
}

fn verdict_of(level: Level) -> Verdict {
    match level {
        Level::Allowed => Verdict::Allow,
        Level::Ask => Verdict::Ask,
        Level::Blocked => Verdict::Deny,
    }
}

fn says(level: Level, what: &str) -> String {
    match level {
        Level::Allowed => format!("allows {what}"),
        Level::Ask => format!("asks you before {what}"),
        Level::Blocked => format!("does not allow {what}"),
    }
}

/// Plain words for doing something with a capability ("changing files").
pub fn doing(c: Capability) -> &'static str {
    match c {
        Capability::FilesystemRead => "reading files",
        Capability::FilesystemWrite => "changing files",
        Capability::ShellExec => "running programs",
        Capability::PowershellExec => "running PowerShell scripts",
        Capability::GitRead => "reading git history",
        Capability::GitWrite => "saving to git",
        Capability::GithubRead => "reading GitHub",
        Capability::GithubWrite => "changing GitHub",
        Capability::SshConnect => "connecting to servers",
        Capability::BrowserNavigate => "visiting websites",
        Capability::BrowserAutomate => "using websites",
        Capability::ComputerObserve => "seeing the screen",
        Capability::ComputerControl => "using the mouse and keyboard",
        Capability::ConnectionsRead => "reading through this connection",
        Capability::ConnectionsWrite => "writing through this connection",
        Capability::McpInvoke => "using add-on tools",
        Capability::NetworkLocal => "reaching local services",
        Capability::ProcessManage => "managing running programs",
    }
}

/// The level `scope` has for `c` under `config`: the role's set grants; the project's and the
/// department's limits can only narrow it.
pub fn level_for(config: &GuardConfig, scope: &Scope, c: Capability) -> LevelFor {
    let what = doing(c);
    let mut checks = Vec::new();
    let role = format!("the {} role", scope.role_name);
    let (role_level, role_note) = match config.role_set(&scope.role_id) {
        None => (Level::Blocked, format!("{role} has no permission set")),
        Some(id) => match config.set(id) {
            None => (
                Level::Blocked,
                format!("{role}'s permission set no longer exists"),
            ),
            Some(set) => {
                let l = set.level(c);
                (
                    l,
                    format!("the {} set of {role} {}", set.name, says(l, what)),
                )
            }
        },
    };
    checks.push(Check {
        layer: Layer::Role,
        verdict: verdict_of(role_level),
        note: role_note.clone(),
    });
    narrowed(config, scope, c, role_level, Layer::Role, role_note, checks)
}

/// The level `scope` has for `c` (`connections.read` or `connections.write`) through
/// `connection`: the connection's **Who may use it** list grants — the agent's own line, else its
/// role's (ADR-062 §3); the project's and the department's limits can only narrow it.
pub fn level_for_connection(
    config: &GuardConfig,
    scope: &Scope,
    connection: &Connection,
    c: Capability,
) -> LevelFor {
    let what = doing(c);
    let name = connection.label();
    let role = format!("the {} role", scope.role_name);
    let (level, note) = match connection.line_for(scope.position_id.as_deref(), &scope.role_id) {
        None => (
            Level::Blocked,
            format!("{name}'s Who may use it list does not include this agent or {role}"),
        ),
        Some(line) => {
            let whose = match line.who {
                connections::Who::Agent { .. } => "this agent".to_owned(),
                connections::Who::Role { .. } => role,
            };
            let l = match (line.level, c) {
                (_, Capability::ConnectionsRead) => Level::Allowed,
                (AccessLevel::ReadWrite, Capability::ConnectionsWrite) => Level::Allowed,
                _ => Level::Blocked,
            };
            (
                l,
                format!(
                    "{name}'s Who may use it list gives {whose} {}, which {}",
                    line.level.words(),
                    says(l, what)
                ),
            )
        }
    };
    let checks = vec![Check {
        layer: Layer::Role,
        verdict: verdict_of(level),
        note: note.clone(),
    }];
    narrowed(config, scope, c, level, Layer::Role, note, checks)
}

/// The level `scope` has for `mcp.invoke` through `add_on` (ADR-066 §3): the add-on's **Who may
/// use it** list grants — the agent's own line, else its role's — **Read only** for its Reading
/// tools, **Read and write** for its Changing tools too (`changing`); the project's and the
/// department's limits can only narrow it.
pub fn level_for_add_on(
    config: &GuardConfig,
    scope: &Scope,
    add_on: &AddOn,
    changing: bool,
) -> LevelFor {
    let c = Capability::McpInvoke;
    let what = if changing {
        "using its Changing tools"
    } else {
        "using its Reading tools"
    };
    let name = format!("The add-on {}", add_on.name);
    let role = format!("the {} role", scope.role_name);
    let (level, note) = match add_on.line_for(scope.position_id.as_deref(), &scope.role_id) {
        None => (
            Level::Blocked,
            format!("{name}'s Who may use it list does not include this agent or {role}"),
        ),
        Some(line) => {
            let whose = match line.who {
                connections::Who::Agent { .. } => "this agent".to_owned(),
                connections::Who::Role { .. } => role,
            };
            let l = if !changing || line.level == AccessLevel::ReadWrite {
                Level::Allowed
            } else {
                Level::Blocked
            };
            (
                l,
                format!(
                    "{name}'s Who may use it list gives {whose} {}, which {}",
                    line.level.words(),
                    says(l, what)
                ),
            )
        }
    };
    let checks = vec![Check {
        layer: Layer::Role,
        verdict: verdict_of(level),
        note: note.clone(),
    }];
    narrowed(config, scope, c, level, Layer::Role, note, checks)
}

/// A worker's level after the project's and department's limits and the owner's switches,
/// starting from what its grant gives (`granted`, decided by `granted_by`, `because` …).
fn narrowed(
    config: &GuardConfig,
    scope: &Scope,
    c: Capability,
    granted: Level,
    granted_by: Layer,
    because: String,
    mut checks: Vec<Check>,
) -> LevelFor {
    let what = doing(c);
    let mut level = granted;
    let mut layer = granted_by;
    let mut reason = because;

    let mut limit =
        |layer_kind: Layer, owner: String, set_id: Option<&str>, checks: &mut Vec<Check>| {
            let (l, note) = match set_id {
                None => (Level::Allowed, format!("{owner} sets no limit")),
                Some(id) => match config.set(id) {
                    None => (
                        Level::Blocked,
                        format!(
                        "{owner}'s limit \"{id}\" is not one of your permission sets, so nothing \
                         is allowed there"
                    ),
                    ),
                    Some(set) => {
                        let l = set.level(c);
                        (
                            l,
                            format!("{owner}'s limit ({}) {}", set.name, says(l, what)),
                        )
                    }
                },
            };
            checks.push(Check {
                layer: layer_kind,
                verdict: verdict_of(l),
                note: note.clone(),
            });
            if l < level {
                level = l;
                layer = layer_kind;
                reason = note;
            }
        };
    if let Some(p) = &scope.project {
        limit(
            Layer::Project,
            format!("the {} project", p.name),
            p.limit.as_deref(),
            &mut checks,
        );
    }
    if let Some(d) = &scope.department {
        limit(
            Layer::Department,
            format!("the {} department", d.name),
            config.departments.get(&d.id).map(String::as_str),
            &mut checks,
        );
    }
    // The owner's switches (ADR-023): a feature turned off is off for everyone.
    if let Some(off) = switched_off(config, c) {
        checks.push(Check {
            layer: Layer::Rule,
            verdict: Verdict::Deny,
            note: off.to_owned(),
        });
        if level > Level::Blocked {
            level = Level::Blocked;
            layer = Layer::Rule;
            reason = off.to_owned();
        }
    }
    LevelFor {
        level,
        layer,
        reason,
        checks,
    }
}

/// Why `c` is off for every worker, when a switch turned its feature off (ADR-023).
pub fn switched_off(config: &GuardConfig, c: Capability) -> Option<&'static str> {
    match c {
        Capability::BrowserNavigate | Capability::BrowserAutomate if !config.switches.browser => {
            Some("Plenipo's browser is switched off (Settings → Switches)")
        }
        Capability::ComputerObserve | Capability::ComputerControl if !config.switches.desktop => {
            Some("the screen, mouse, and keyboard are switched off (Settings → Switches)")
        }
        Capability::SshConnect if !config.switches.servers => {
            Some("remote computers (SSH) are switched off (Settings → Switches)")
        }
        _ => None,
    }
}

/// Levels for every capability (what a grant snapshots).
pub fn levels_for(config: &GuardConfig, scope: &Scope) -> BTreeMap<Capability, Level> {
    Capability::ALL
        .iter()
        .map(|c| (*c, level_for(config, scope, *c).level))
        .collect()
}

/// One action a worker asks for, already confined to its folder by the caller.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    pub capability: Capability,
    pub risk: Risk,
    /// What it does, for the reason ("read README.md", "run cargo test").
    pub summary: &'a str,
    /// Files it touches, relative to the folder.
    pub files: &'a [String],
    /// It writes into a `.git` folder.
    pub writes_git_dir: bool,
    pub command: Option<&'a CommandLine>,
    pub script: Option<&'a str>,
    /// Sensitive on its own (e.g. pushing to a server).
    pub inherent: Option<(SensitiveKind, &'a str)>,
    /// Names of the stored secrets the program would be given (ADR-048): it asks unless one of
    /// the owner's rules names both the program and each secret.
    pub secrets: &'a [String],
    pub workspace: &'a Path,
    /// The website it opens or acts on (Phase 10), checked against the owner's website lists.
    pub site: Option<SiteCheck<'a>>,
    /// The server it uses (Phase 11), checked against the owner's settings for that server.
    pub server: Option<ServerCheck<'a>>,
    /// The connection it uses (Phase 20), checked against the owner's settings for it.
    pub connection: Option<ConnectionCheck<'a>>,
    /// The add-on tool it uses (Phase 20 part 20C, ADR-066), checked against the owner's marks.
    pub add_on: Option<AddOnCheck<'a>>,
}

/// A website an action opens or acts on.
#[derive(Debug, Clone, Copy)]
pub struct SiteCheck<'a> {
    pub site: &'a Site,
    /// The owner already approved opening this website for this worker's step.
    pub approved: bool,
}

/// The level the grant allowed when it was issued, and whether it still stands.
#[derive(Debug, Clone, Copy)]
pub struct GrantState {
    pub level: Level,
    pub revoked: bool,
}

fn decision(
    verdict: Verdict,
    layer: Layer,
    reason: String,
    risk: Risk,
    sensitive: Option<SensitiveKind>,
    mut checks: Vec<Check>,
) -> Decision {
    checks.push(Check {
        layer,
        verdict,
        note: reason.clone(),
    });
    Decision {
        verdict,
        reason,
        layer,
        risk,
        sensitive,
        checks,
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

/// Guard's decision about `request` for a worker with `current` levels (from the settings as
/// they are now) and `grant` (what it was given when its step started).
pub fn evaluate(
    config: &GuardConfig,
    request: &Request<'_>,
    current: &LevelFor,
    grant: GrantState,
) -> Decision {
    let risk = request.risk;
    let mut checks = current.checks.clone();
    if grant.revoked {
        return decision(
            Verdict::Deny,
            Layer::Grant,
            "This worker's permissions were revoked.".into(),
            risk,
            None,
            checks,
        );
    }
    let what = doing(request.capability);
    // Layers 1–3: role, project, department (and the grant, which never widens).
    let (level, layer, why) = if grant.level < current.level {
        (
            grant.level,
            Layer::Grant,
            format!(
                "this worker's starting permission {}",
                says(grant.level, what)
            ),
        )
    } else {
        (current.level, current.layer, current.reason.clone())
    };
    if level == Level::Blocked {
        return decision(
            Verdict::Deny,
            layer,
            format!("Blocked: {}.", why),
            risk,
            None,
            checks,
        );
    }
    // Layer 4: the target. A server (Phase 11): who may use it, its pinned identity, its kinds
    // of commands, folders, and blocked files; production servers ask for every command.
    let mut server_asks = None;
    let mut server_sensitive = Vec::new();
    if let Some(check) = &request.server {
        match servers::check(&config.blocked_files, check) {
            ServerVerdict::Deny { layer, reason } => {
                return decision(
                    Verdict::Deny,
                    layer,
                    format!("Blocked: {reason}."),
                    risk,
                    None,
                    checks,
                )
            }
            ServerVerdict::Go { ask, sensitive } => {
                server_asks = ask;
                server_sensitive = sensitive;
            }
        }
    }
    // A connection (Phase 20): connected, and the part on at a level that allows the tool.
    let mut connection_sensitive = None;
    let mut connection_all_listed = false;
    if let Some(check) = &request.connection {
        // A tool that writes, sends, deletes, or pays needs the writing permission, whatever it
        // is called (defense in depth: the tool tables keep these in line).
        if check.kind.writes() && request.capability != Capability::ConnectionsWrite {
            return decision(
                Verdict::Deny,
                Layer::Target,
                "Blocked: this changes something, so it needs Write through Connections.".into(),
                risk,
                None,
                checks,
            );
        }
        match connections::check(check) {
            ConnectionVerdict::Deny { layer, reason } => {
                return decision(
                    Verdict::Deny,
                    layer,
                    format!("Blocked: {reason}."),
                    risk,
                    None,
                    checks,
                )
            }
            ConnectionVerdict::Go {
                sensitive,
                all_listed,
            } => {
                connection_sensitive = sensitive;
                connection_all_listed = all_listed;
            }
        }
    }
    // An add-on's tool (ADR-066): the add-on on, the tool marked Reading or Changing. A Changing
    // tool always asks (below): no switch lets an add-on change anything without asking, because
    // Plenipo cannot see what the program does with the call (ADR-066 §4).
    let mut add_on_asks = None;
    if let Some(check) = &request.add_on {
        let (a, t) = (check.add_on, check.tool);
        if !a.on {
            return decision(
                Verdict::Deny,
                Layer::Target,
                format!(
                    "Blocked: the add-on {} is off (Settings → Connections → Add-on tools).",
                    a.name
                ),
                risk,
                None,
                checks,
            );
        }
        match t.mark {
            ToolMark::Off => {
                let why = if t.changed {
                    "changed since the owner last looked, so it is Off until they look again"
                } else {
                    "Off"
                };
                return decision(
                    Verdict::Deny,
                    Layer::Rule,
                    format!(
                        "Blocked: the add-on {}'s tool {} is {why} (Settings → Connections → \
                         Add-on tools).",
                        a.name, t.name
                    ),
                    risk,
                    None,
                    checks,
                );
            }
            ToolMark::Changing => add_on_asks = Some(format!("{} from {}", t.name, a.name)),
            ToolMark::Reading => {}
        }
    }
    if request.writes_git_dir {
        return decision(
            Verdict::Deny,
            Layer::Target,
            "Blocked: git's own files (.git) change only through the git tools.".into(),
            risk,
            None,
            checks,
        );
    }
    for f in request.files {
        if let Some(rule) = blocked_by(&config.blocked_files, f) {
            return decision(
                Verdict::Deny,
                Layer::Rule,
                format!("Blocked: {f} is a blocked file (your rule \"{rule}\")."),
                risk,
                None,
                checks,
            );
        }
    }
    // The website (Phase 10): blocked ones never open; local addresses only when allowed by name.
    let mut site_asks = None;
    let mut site_allowed = false;
    if let Some(check) = request.site {
        let shown = check.site.shown();
        match websites::check(&config.websites, check.site) {
            SiteVerdict::Blocked(rule) => {
                return decision(
                    Verdict::Deny,
                    Layer::Rule,
                    format!("Blocked: {shown} is on your blocked websites list (\"{rule}\")."),
                    risk,
                    None,
                    checks,
                )
            }
            SiteVerdict::Local => {
                return decision(
                    Verdict::Deny,
                    Layer::Target,
                    format!(
                        "Blocked: {shown} is an address on this computer or your local network, \
                         and those open only when your allowed websites list names them."
                    ),
                    risk,
                    None,
                    checks,
                )
            }
            SiteVerdict::Other => {
                return decision(
                    Verdict::Deny,
                    Layer::Rule,
                    format!(
                        "Blocked: {shown} is not on your allowed websites list, and you set other \
                         websites to blocked."
                    ),
                    risk,
                    None,
                    checks,
                )
            }
            SiteVerdict::Ask if !check.approved => site_asks = Some(shown),
            SiteVerdict::Allowed(_) => site_allowed = true,
            SiteVerdict::Ask | SiteVerdict::Blank => {}
        }
    }
    if let Some(cmd) = request.command {
        // Blocking catches a name however it is written, on every system (ADR-150).
        if let Some(rule) = first_catch(&config.commands.blocked, cmd) {
            return decision(
                Verdict::Deny,
                Layer::Rule,
                format!(
                    "Blocked: {} is on your blocked commands list (\"{rule}\").",
                    cmd.program_name()
                ),
                risk,
                None,
                checks,
            );
        }
    }
    // Layer 5: the action's risk.
    // On a server, the kind the owner blocked wins; otherwise the most specific one.
    let server_found = server_sensitive
        .iter()
        .copied()
        .find(|(kind, _)| config.sensitive_rule(*kind) == SensitiveRule::Block)
        .or_else(|| server_sensitive.first().copied());
    let found = request
        .inherent
        .or(connection_sensitive)
        .or(server_found)
        .or_else(|| {
            request
                .command
                .and_then(|c| sensitive::command(c, request.workspace))
        })
        .or_else(|| request.script.and_then(sensitive::script));
    // The owner's switches (ADR-023): on a website on the allowed list, sending, buying, or
    // signing in may go ahead without asking; through a connection, only sending, and only when
    // every recipient is on the connection's list (ADR-062 §5) — money never goes ahead without
    // asking there. A kind set to blocked stays blocked, and the role's own "ask me" level still
    // asks (below).
    let on_allowed_site = request.capability == Capability::BrowserAutomate && site_allowed;
    let to_listed_recipients = |kind: SensitiveKind| {
        request.capability == Capability::ConnectionsWrite
            && kind == SensitiveKind::Outbound
            && connection_all_listed
    };
    let without_asking = |kind: SensitiveKind| {
        (on_allowed_site || to_listed_recipients(kind))
            && config.sensitive_rule(kind) == SensitiveRule::Ask
            && config.switches.without_asking(kind) == Some(true)
    };
    if let Some((kind, because)) = found.filter(|(kind, _)| without_asking(*kind)) {
        let note = if to_listed_recipients(kind) {
            format!(
                "{because} ({}); you let workers send to the addresses on this connection's list \
                 without asking (Settings → Switches and Settings → Connections)",
                kind.label()
            )
        } else {
            format!(
                "{because} ({}); you let workers do this on your allowed websites without asking \
                 (Settings → Switches)",
                kind.label()
            )
        };
        checks.push(Check {
            layer: Layer::Risk,
            verdict: Verdict::Allow,
            note,
        });
    }
    if let Some((kind, because)) = found.filter(|(kind, _)| !without_asking(*kind)) {
        let (verdict, reason) = match config.sensitive_rule(kind) {
            SensitiveRule::Ask => (
                Verdict::Ask,
                format!(
                    "{} needs your approval: {because} ({}).",
                    capitalized(request.summary),
                    kind.label()
                ),
            ),
            SensitiveRule::Block => (
                Verdict::Deny,
                format!(
                    "Blocked: {because}, and you set \"{}\" to blocked.",
                    kind.label()
                ),
            ),
        };
        return decision(verdict, Layer::Risk, reason, risk, Some(kind), checks);
    }
    // Layer 6: the owner's explicit rules.
    if let Some(tool) = add_on_asks {
        return decision(
            Verdict::Ask,
            Layer::Rule,
            format!(
                "{} needs your approval: you marked the add-on tool {tool} Changing, so it asks \
                 you every time.",
                capitalized(request.summary)
            ),
            risk,
            None,
            checks,
        );
    }
    if let Some(why) = server_asks {
        return decision(
            Verdict::Ask,
            Layer::Rule,
            format!(
                "{} needs your approval: {why}.",
                capitalized(request.summary)
            ),
            risk,
            None,
            checks,
        );
    }
    if let Some(shown) = site_asks {
        return decision(
            Verdict::Ask,
            Layer::Rule,
            format!(
                "{} needs your approval: {shown} is not on your allowed websites list (once you \
                 approve, this worker may use it until its step ends).",
                capitalized(request.summary)
            ),
            risk,
            None,
            checks,
        );
    }
    if let Some(cmd) = request.command {
        if let Some(rule) = first_catch(&config.commands.ask, cmd) {
            return decision(
                Verdict::Ask,
                Layer::Rule,
                format!(
                    "{} needs your approval: it is on your always-ask list (\"{rule}\").",
                    capitalized(request.summary)
                ),
                risk,
                None,
                checks,
            );
        }
    }
    if level == Level::Ask {
        return decision(
            Verdict::Ask,
            layer,
            format!(
                "{} needs your approval: {}.",
                capitalized(request.summary),
                why
            ),
            risk,
            None,
            checks,
        );
    }
    // A rule that names a program and its secrets (ADR-048) approves the command too.
    let names_program = |r: &SecretRule| {
        request
            .command
            .is_some_and(|cmd| rule_matches(&r.rule, cmd))
    };
    if let Some(cmd) = request.command {
        if first_match(&config.commands.approved, cmd).is_none()
            && !config.commands.with_secrets.iter().any(names_program)
        {
            return decision(
                Verdict::Ask,
                Layer::Rule,
                format!(
                    "{} needs your approval: it is not on your approved commands list.",
                    capitalized(request.summary)
                ),
                risk,
                None,
                checks,
            );
        }
    }
    // ADR-048 (secrets reach only the programs they are for): a program is given a stored
    // secret without asking only when one of the owner's rules names both the program and that
    // secret. A script has no command a rule could name, so it always asks.
    if !request.secrets.is_empty() {
        let named = |secret: &String| {
            config
                .commands
                .with_secrets
                .iter()
                .filter(|r| names_program(r))
                .any(|r| r.secrets.iter().any(|n| n.eq_ignore_ascii_case(secret)))
        };
        let what = format!(
            "the stored secret{} {}",
            if request.secrets.len() == 1 { "" } else { "s" },
            request.secrets.join(", ")
        );
        if !request.secrets.iter().all(named) {
            return decision(
                Verdict::Ask,
                Layer::Rule,
                format!(
                    "{} needs your approval: it would be given {what}.",
                    capitalized(request.summary)
                ),
                risk,
                None,
                checks,
            );
        }
        checks.push(Check {
            layer: Layer::Rule,
            verdict: Verdict::Allow,
            note: format!("your rule gives this program {what}"),
        });
    }
    checks.push(Check {
        layer: Layer::Target,
        verdict: Verdict::Allow,
        note: match (&request.connection, &request.add_on) {
            (Some(c), _) => format!("through {}", c.connection.label()),
            (None, Some(a)) => format!("through the add-on {}", a.add_on.name),
            (None, None) => "inside the project folder".into(),
        },
    });
    decision(
        Verdict::Allow,
        layer,
        format!("Allowed: {why}."),
        risk,
        None,
        checks,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::PermissionSetInput;
    use crate::websites::{OtherSites, WebsiteRules};

    fn config() -> GuardConfig {
        let mut c = GuardConfig::with_defaults();
        c.assign_role("dev", Some("developer")).unwrap();
        c.assign_role("rev", Some("reviewer")).unwrap();
        c
    }

    fn scope(role: &str, project_limit: Option<&str>) -> Scope {
        Scope {
            role_id: role.into(),
            role_name: if role == "dev" {
                "Senior Developer".into()
            } else {
                "Code Reviewer".into()
            },
            project: Some(ScopeProject {
                id: "p".into(),
                name: "Website".into(),
                limit: project_limit.map(str::to_owned),
                folder: Some("/w".into()),
            }),
            department: Some(ScopeUnit {
                id: "d".into(),
                name: "Development".into(),
            }),
            ..Scope::default()
        }
    }

    fn request<'a>(
        capability: Capability,
        files: &'a [String],
        command: Option<&'a CommandLine>,
    ) -> Request<'a> {
        Request {
            capability,
            risk: Risk::Read,
            summary: "do it",
            files,
            writes_git_dir: false,
            command,
            script: None,
            inherent: None,
            secrets: &[],
            workspace: Path::new("/w"),
            site: None,
            server: None,
            connection: None,
            add_on: None,
        }
    }

    /// Microsoft 365, connected: Mail at Full access, Calendar at Read only; the Documentation
    /// Writer role may read and write, and one agent in it may only read.
    fn microsoft() -> Connection {
        use crate::connections::{Access, AccountKind, ConnectionState, Part, PartLevel, Service};
        let mut m = Connection::new("microsoft365", Service::Microsoft365);
        m.state = ConnectionState::Connected;
        m.account_kind = Some(AccountKind::Work);
        m.parts.insert(Part::Mail, PartLevel::FullAccess);
        m.access = vec![
            Access {
                who: connections::Who::Role {
                    id: "writer".into(),
                },
                level: AccessLevel::ReadWrite,
            },
            Access {
                who: connections::Who::Agent {
                    id: "pos-read".into(),
                },
                level: AccessLevel::ReadOnly,
            },
        ];
        m.send_list = vec!["@8westit.com".into()];
        m
    }

    fn writer(position: &str, limit: Option<&str>) -> Scope {
        Scope {
            role_id: "writer".into(),
            role_name: "Documentation Writer".into(),
            position_id: Some(position.into()),
            project: Some(ScopeProject {
                id: "p".into(),
                name: "Website".into(),
                limit: limit.map(str::to_owned),
                folder: None,
            }),
            ..Scope::default()
        }
    }

    fn connection_eval(
        c: &GuardConfig,
        s: &Scope,
        m: &Connection,
        kind: crate::connections::ToolKind,
        to: &[String],
    ) -> Decision {
        use crate::connections::{Part, ToolKind};
        let capability = if kind == ToolKind::Read {
            Capability::ConnectionsRead
        } else {
            Capability::ConnectionsWrite
        };
        let now = level_for_connection(c, s, m, capability);
        let mut r = request(capability, &[], None);
        r.summary = "send the reply";
        r.connection = Some(ConnectionCheck {
            connection: m,
            part: Part::Mail,
            kind,
            recipients: to,
        });
        evaluate(
            c,
            &r,
            &now,
            GrantState {
                level: now.level,
                revoked: false,
            },
        )
    }

    /// An add-on with one Reading and one Changing tool, on; the Documentation Writer role may
    /// use its Reading tools, and one agent may use its Changing tools too.
    fn add_on() -> AddOn {
        use crate::add_ons::AddOnTool;
        use crate::connections::Access;
        let tool = |name: &str, mark| AddOnTool {
            name: name.into(),
            alias: format!("addon_tickets_{name}"),
            mark,
            description: String::new(),
            input: serde_json::json!({ "type": "object" }),
            read_only_hint: None,
            destructive_hint: None,
            changed: false,
        };
        AddOn {
            id: "tickets".into(),
            name: "Tickets".into(),
            program: "/usr/local/bin/tickets-mcp".into(),
            args: Vec::new(),
            secrets: Vec::new(),
            on: true,
            tools: vec![
                tool("lookup", ToolMark::Reading),
                tool("create", ToolMark::Changing),
                tool("purge", ToolMark::Off),
            ],
            access: vec![
                Access {
                    who: connections::Who::Role {
                        id: "writer".into(),
                    },
                    level: AccessLevel::ReadOnly,
                },
                Access {
                    who: connections::Who::Agent {
                        id: "pos-rw".into(),
                    },
                    level: AccessLevel::ReadWrite,
                },
            ],
            checked_at: Some(1),
            added_at: 1,
        }
    }

    fn add_on_eval(c: &GuardConfig, s: &Scope, a: &AddOn, tool: &str) -> Decision {
        let t = a.tools.iter().find(|t| t.name == tool).unwrap();
        let now = level_for_add_on(c, s, a, t.mark == ToolMark::Changing);
        let mut r = request(Capability::McpInvoke, &[], None);
        r.summary = "use the add-on tool";
        r.add_on = Some(AddOnCheck { add_on: a, tool: t });
        evaluate(
            c,
            &r,
            &now,
            GrantState {
                level: now.level,
                revoked: false,
            },
        )
    }

    /// ADR-066 §3–§4: an add-on's list grants; Reading tools go ahead, Changing tools need Read
    /// and write and ask every time, whatever the switches say; an Off tool, an add-on that is
    /// off, or a project limit without add-on tools refuses.
    #[test]
    fn add_on_tools_follow_their_list_and_changing_ones_always_ask() {
        let mut c = config();
        let mut a = add_on();
        let reader = writer("pos-1", None);
        let d = add_on_eval(&c, &reader, &a, "lookup");
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        let d = add_on_eval(&c, &reader, &a, "create");
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("Read only"), "{}", d.reason);
        let rw = writer("pos-rw", None);
        c.switches.send_without_asking = true;
        c.switches.buy_without_asking = true;
        let d = add_on_eval(&c, &rw, &a, "create");
        assert_eq!(d.verdict, Verdict::Ask, "{}", d.reason);
        assert!(d.reason.contains("asks you every time"), "{}", d.reason);
        let d = add_on_eval(&c, &rw, &a, "purge");
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("is Off"), "{}", d.reason);
        a.tools[2].changed = true;
        let d = add_on_eval(&c, &rw, &a, "purge");
        assert!(d.reason.contains("look again"), "{}", d.reason);
        // Nobody else, and nothing while the add-on is off.
        let dev = Scope {
            position_id: Some("pos-dev".into()),
            ..scope("dev", None)
        };
        let d = add_on_eval(&c, &dev, &a, "lookup");
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("Who may use it"), "{}", d.reason);
        a.on = false;
        let d = add_on_eval(&c, &rw, &a, "lookup");
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("is off"), "{}", d.reason);
        a.on = true;
        // A project limited to a set without add-on tools: none there.
        let d = add_on_eval(&c, &writer("pos-rw", Some("developer")), &a, "lookup");
        assert_eq!(d.verdict, Verdict::Deny);
        assert_eq!(d.layer, Layer::Project);
    }

    /// Money through Stripe (or a store refund) always asks: with both switches on, the
    /// customer on the list, and the worker at Read and write (the owner's rule, ADR-071 §6.3).
    #[test]
    fn stripe_money_always_asks_whatever_the_switches_and_lists_say() {
        use crate::connections::{ConnectionState, Part, PartLevel, Service, ToolKind};
        let mut c = config();
        c.switches.send_without_asking = true;
        c.switches.buy_without_asking = true;
        let mut m = microsoft();
        m.id = "stripe".into();
        m.service = Service::Stripe;
        m.parts = Service::Stripe.starting_parts();
        m.parts.insert(Part::Payments, PartLevel::FullAccess);
        m.state = ConnectionState::Connected;
        m.account_kind = None;
        m.send_list = vec!["alex@8westit.com".into()];
        let s = writer("pos-1", None);
        let to = vec!["alex@8westit.com".to_owned()];
        let now = level_for_connection(&c, &s, &m, Capability::ConnectionsWrite);
        let mut r = request(Capability::ConnectionsWrite, &[], None);
        r.summary = "refund USD 25.00";
        r.connection = Some(ConnectionCheck {
            connection: &m,
            part: Part::Payments,
            kind: ToolKind::Pay,
            recipients: &to,
        });
        let grant = GrantState {
            level: now.level,
            revoked: false,
        };
        let d = evaluate(&c, &r, &now, grant);
        assert_eq!(d.verdict, Verdict::Ask, "{}", d.reason);
        assert_eq!(d.sensitive, Some(SensitiveKind::Payment));
        // The owner's Blocked rule for money refuses outright.
        c.set_sensitive(SensitiveKind::Payment, SensitiveRule::Block);
        assert_eq!(evaluate(&c, &r, &now, grant).verdict, Verdict::Deny);
    }

    /// ADR-062 §3: the connection's list grants — the agent's own line, else its role's — and a
    /// project's limit only narrows it.
    #[test]
    fn a_connections_list_grants_and_limits_narrow() {
        use crate::connections::ToolKind;
        let c = config();
        let m = microsoft();
        let d = connection_eval(&c, &writer("pos-1", None), &m, ToolKind::Read, &[]);
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        let d = connection_eval(&c, &writer("pos-1", None), &m, ToolKind::Write, &[]);
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        // The agent's own line (Read only) wins over its role's.
        let d = connection_eval(&c, &writer("pos-read", None), &m, ToolKind::Write, &[]);
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(
            d.reason.contains("gives this agent Read only"),
            "{}",
            d.reason
        );
        // A role not on the list gets nothing, whatever its permission set.
        let dev = Scope {
            position_id: Some("pos-dev".into()),
            ..scope("dev", None)
        };
        let d = connection_eval(&c, &dev, &m, ToolKind::Read, &[]);
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("Who may use it"), "{}", d.reason);
        // A project limited to a set without Connections: none there.
        let d = connection_eval(
            &c,
            &writer("pos-1", Some("developer")),
            &m,
            ToolKind::Read,
            &[],
        );
        assert_eq!(d.verdict, Verdict::Deny);
        assert_eq!(d.layer, Layer::Project);
    }

    /// ADR-062 §5: sending asks — unless the switch is on and every recipient is listed; the
    /// owner's Blocked rule and an "ask" limit still win; deleting asks; paying always asks.
    #[test]
    fn sending_asks_unless_switched_on_for_listed_recipients_and_paying_always_asks() {
        use crate::connections::ToolKind;
        let mut c = config();
        let m = microsoft();
        let s = writer("pos-1", None);
        let team = vec!["alex@8westit.com".to_owned()];
        let outside = vec!["alex@8westit.com".to_owned(), "x@evil.test".to_owned()];
        let d = connection_eval(&c, &s, &m, ToolKind::Send, &team);
        assert_eq!(d.verdict, Verdict::Ask);
        assert_eq!(d.sensitive, Some(SensitiveKind::Outbound));
        c.switches.send_without_asking = true;
        let d = connection_eval(&c, &s, &m, ToolKind::Send, &team);
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        assert!(d
            .checks
            .iter()
            .any(|k| k.note.contains("this connection's list")));
        // One recipient off the list: it asks.
        let d = connection_eval(&c, &s, &m, ToolKind::Send, &outside);
        assert_eq!(d.verdict, Verdict::Ask);
        // Sending set to Blocked: never.
        c.set_sensitive(SensitiveKind::Outbound, SensitiveRule::Block);
        let d = connection_eval(&c, &s, &m, ToolKind::Send, &team);
        assert_eq!(d.verdict, Verdict::Deny);
        c.set_sensitive(SensitiveKind::Outbound, SensitiveRule::Ask);
        // Deleting asks.
        let d = connection_eval(&c, &s, &m, ToolKind::Delete, &[]);
        assert_eq!(d.verdict, Verdict::Ask);
        assert_eq!(d.sensitive, Some(SensitiveKind::CloudDelete));
        // Paying always asks, even with "Buying and paying (without asking)" on.
        c.switches.buy_without_asking = true;
        let d = connection_eval(&c, &s, &m, ToolKind::Pay, &team);
        assert_eq!(d.verdict, Verdict::Ask);
        assert_eq!(d.sensitive, Some(SensitiveKind::Payment));
        // A draft stays in the owner's account: no question.
        let d = connection_eval(&c, &s, &m, ToolKind::Write, &[]);
        assert_eq!(d.verdict, Verdict::Allow);
    }

    /// The switch reaches sending only: never deleting or paying, even to listed recipients; a
    /// limit that asks still asks; and a tool that changes something needs the writing
    /// permission whatever it says it is.
    #[test]
    fn the_send_switch_reaches_sending_only_and_a_limit_that_asks_still_asks() {
        use crate::connections::{Part, ToolKind};
        let mut c = config();
        c.switches.send_without_asking = true;
        c.switches.buy_without_asking = true;
        let m = microsoft();
        let s = writer("pos-1", None);
        let team = vec!["alex@8westit.com".to_owned()];
        for kind in [ToolKind::Delete, ToolKind::Pay] {
            let d = connection_eval(&c, &s, &m, kind, &team);
            assert_eq!(d.verdict, Verdict::Ask, "{kind:?}: {}", d.reason);
        }
        // A project limit whose set says "ask me" for writing: a listed send still asks.
        let asks = c
            .save_set(&crate::dto::PermissionSetInput {
                id: None,
                name: "Asks first".into(),
                description: String::new(),
                levels: [
                    (Capability::ConnectionsRead, Level::Allowed),
                    (Capability::ConnectionsWrite, Level::Ask),
                ]
                .into_iter()
                .collect(),
            })
            .unwrap();
        let limited = writer("pos-1", Some(&asks.id));
        let d = connection_eval(&c, &limited, &m, ToolKind::Send, &team);
        assert_eq!(d.verdict, Verdict::Ask, "{}", d.reason);
        // A department limit narrows too.
        let mut in_dept = writer("pos-1", None);
        in_dept.department = Some(ScopeUnit {
            id: "d".into(),
            name: "Development".into(),
        });
        c.assign_department("d", Some("read-only")).unwrap();
        let d = connection_eval(&c, &in_dept, &m, ToolKind::Read, &[]);
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Department));
        // A tool that sends but asks only for reading is refused.
        let now = level_for_connection(&c, &s, &m, Capability::ConnectionsRead);
        let mut r = request(Capability::ConnectionsRead, &[], None);
        r.connection = Some(ConnectionCheck {
            connection: &m,
            part: Part::Mail,
            kind: ToolKind::Send,
            recipients: &team,
        });
        let d = evaluate(
            &c,
            &r,
            &now,
            GrantState {
                level: now.level,
                revoked: false,
            },
        );
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(
            d.reason.contains("Write through Connections"),
            "{}",
            d.reason
        );
        // A step's grant stricter than the settings now: the stricter wins.
        let now = level_for_connection(&c, &s, &m, Capability::ConnectionsWrite);
        let mut r = request(Capability::ConnectionsWrite, &[], None);
        r.connection = Some(ConnectionCheck {
            connection: &m,
            part: Part::Mail,
            kind: ToolKind::Write,
            recipients: &[],
        });
        let d = evaluate(
            &c,
            &r,
            &now,
            GrantState {
                level: Level::Blocked,
                revoked: false,
            },
        );
        assert_eq!(d.verdict, Verdict::Deny, "{}", d.reason);
    }

    fn eval(c: &GuardConfig, s: &Scope, r: &Request<'_>) -> Decision {
        let now = level_for(c, s, r.capability);
        evaluate(
            c,
            r,
            &now,
            GrantState {
                level: now.level,
                revoked: false,
            },
        )
    }

    #[test]
    fn layers_narrow_and_explain() {
        let c = config();
        let dev = level_for(&c, &scope("dev", None), Capability::FilesystemWrite);
        assert_eq!(dev.level, Level::Allowed);
        assert!(dev
            .reason
            .contains("Developer set of the Senior Developer role allows"));
        assert_eq!(dev.checks.len(), 3, "role, project, department");

        let limited = level_for(
            &c,
            &scope("dev", Some("read-only")),
            Capability::FilesystemWrite,
        );
        assert_eq!(
            (limited.level, limited.layer),
            (Level::Blocked, Layer::Project)
        );
        assert!(limited
            .reason
            .contains("Website project's limit (Read only)"));

        let mut c2 = c.clone();
        c2.assign_department("d", Some("reviewer")).unwrap();
        let dept = level_for(&c2, &scope("dev", None), Capability::ShellExec);
        assert_eq!((dept.level, dept.layer), (Level::Ask, Layer::Department));

        let unknown = level_for(
            &c,
            &scope("dev", Some("development")),
            Capability::FilesystemRead,
        );
        assert_eq!(
            unknown.level,
            Level::Blocked,
            "an unknown limit fails closed"
        );
        let nobody = level_for(&c, &scope("other", None), Capability::FilesystemRead);
        assert_eq!(nobody.level, Level::Blocked);
        assert!(nobody.reason.contains("has no permission set"));

        let all = levels_for(&c, &scope("rev", None));
        assert_eq!(all[&Capability::FilesystemRead], Level::Allowed);
        assert_eq!(all[&Capability::FilesystemWrite], Level::Blocked);
        assert_eq!(all[&Capability::ShellExec], Level::Ask);
    }

    #[test]
    fn allowed_read_and_denied_write() {
        let c = config();
        let files = vec!["README.md".to_owned()];
        let d = eval(
            &c,
            &scope("rev", None),
            &request(Capability::FilesystemRead, &files, None),
        );
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        let d = eval(
            &c,
            &scope("rev", None),
            &request(Capability::FilesystemWrite, &files, None),
        );
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Role));
        assert!(
            d.reason.starts_with(
                "Blocked: the Reviewer set of the Code Reviewer role does not allow changing files"
            ),
            "{}",
            d.reason
        );
    }

    #[test]
    fn blocked_files_and_git_internals() {
        let c = config();
        let files = vec!["config/.env".to_owned()];
        let d = eval(
            &c,
            &scope("dev", None),
            &request(Capability::FilesystemRead, &files, None),
        );
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Rule));
        let ok = vec![".env.example".to_owned()];
        let d = eval(
            &c,
            &scope("dev", None),
            &request(Capability::FilesystemRead, &ok, None),
        );
        assert_eq!(d.verdict, Verdict::Allow);
        let mut r = request(Capability::FilesystemWrite, &ok, None);
        r.writes_git_dir = true;
        assert_eq!(eval(&c, &scope("dev", None), &r).layer, Layer::Target);
    }

    #[test]
    fn command_allow_ask_and_deny() {
        let c = config();
        let s = scope("dev", None);
        let run = |line: &str| {
            let mut w = line.split_whitespace();
            let cmd = CommandLine {
                program: w.next().unwrap().to_owned(),
                args: w.map(str::to_owned).collect(),
            };
            eval(&c, &s, &request(Capability::ShellExec, &[], Some(&cmd)))
        };
        assert_eq!(run("cargo test --workspace").verdict, Verdict::Allow);
        let unknown = run("cargo run");
        assert_eq!(
            (unknown.verdict, unknown.layer),
            (Verdict::Ask, Layer::Rule)
        );
        assert!(unknown
            .reason
            .contains("not on your approved commands list"));
        let blocked = run("curl https://example.com");
        assert_eq!(
            (blocked.verdict, blocked.layer),
            (Verdict::Deny, Layer::Rule)
        );
        assert!(blocked.reason.contains("\"curl *\""));
        // Approved, but sensitive: asks anyway.
        let deploy = run("npm run deploy");
        assert_eq!((deploy.verdict, deploy.layer), (Verdict::Ask, Layer::Risk));
        assert_eq!(deploy.sensitive, Some(SensitiveKind::Production));
        // The owner can block a sensitive kind outright, and add always-ask rules.
        let mut c2 = c.clone();
        c2.set_sensitive(SensitiveKind::Production, SensitiveRule::Block);
        c2.commands.ask.push("cargo test *".into());
        let cmd = CommandLine::new("npm", &["run", "deploy"]);
        let d = eval(&c2, &s, &request(Capability::ShellExec, &[], Some(&cmd)));
        assert_eq!(d.verdict, Verdict::Deny);
        let cmd = CommandLine::new("cargo", &["test"]);
        let d = eval(&c2, &s, &request(Capability::ShellExec, &[], Some(&cmd)));
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Rule));
        // Reviewers must ask for any program, and cannot run blocked ones at all.
        let rev = scope("rev", None);
        let cmd = CommandLine::new("cargo", &["test"]);
        let d = eval(&c, &rev, &request(Capability::ShellExec, &[], Some(&cmd)));
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Role));
    }

    /// ADR-048: a program that would be given a stored secret asks first, even when its command
    /// is approved, unless one of the owner's rules names both the program and the secret.
    #[test]
    fn a_stored_secret_asks_unless_a_rule_names_program_and_secret() {
        let mut c = config();
        let s = scope("dev", None);
        let given = ["Deploy key".to_owned()];
        let cmd = CommandLine::new("cargo", &["test"]);
        let mut r = request(Capability::ShellExec, &[], Some(&cmd));
        r.summary = "run cargo test";
        assert_eq!(eval(&c, &s, &r).verdict, Verdict::Allow);
        r.secrets = &given;
        let d = eval(&c, &s, &r);
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Rule));
        assert_eq!(
            d.reason,
            "Run cargo test needs your approval: it would be given the stored secret Deploy key."
        );
        // A rule for another program, or another secret, does not cover it.
        c.commands.with_secrets.push(SecretRule {
            rule: "gh *".into(),
            secrets: vec!["Deploy key".into()],
        });
        c.commands.with_secrets.push(SecretRule {
            rule: "cargo test *".into(),
            secrets: vec!["Other".into()],
        });
        assert_eq!(eval(&c, &s, &r).verdict, Verdict::Ask);
        // One naming the program and the secret does; names are compared ignoring case.
        c.commands.with_secrets.push(SecretRule {
            rule: "cargo test *".into(),
            secrets: vec!["deploy KEY".into()],
        });
        let d = eval(&c, &s, &r);
        assert_eq!(d.verdict, Verdict::Allow, "{}", d.reason);
        assert!(
            d.checks
                .iter()
                .any(|k| k.layer == Layer::Rule && k.note.contains("Deploy key")),
            "{:?}",
            d.checks
        );
        // Every secret given must be named by a rule that matches the command.
        let two = ["Deploy key".to_owned(), "Third".to_owned()];
        r.secrets = &two;
        let d = eval(&c, &s, &r);
        assert_eq!(d.verdict, Verdict::Ask);
        assert!(
            d.reason.contains("stored secrets Deploy key, Third"),
            "{}",
            d.reason
        );
        // Such a rule is an approved command too: one line does both.
        let cmd = CommandLine::new("gh", &["pr", "list"]);
        let mut r = request(Capability::ShellExec, &[], Some(&cmd));
        r.summary = "run gh pr list";
        r.secrets = &given;
        assert_eq!(eval(&c, &s, &r).verdict, Verdict::Allow);
        // The always-ask list and blocked list still win over it.
        c.commands.ask.push("gh pr *".into());
        assert_eq!(eval(&c, &s, &r).verdict, Verdict::Ask);
        // A script has no command a rule could name: it always asks when given a secret.
        for set in c.sets.iter_mut().filter(|s| s.id == "developer") {
            set.levels
                .insert(Capability::PowershellExec, Level::Allowed);
        }
        let mut script = request(Capability::PowershellExec, &[], None);
        script.summary = "run a PowerShell script";
        script.script = Some("Get-Date");
        assert_eq!(eval(&c, &s, &script).verdict, Verdict::Allow);
        script.secrets = &given;
        let d = eval(&c, &s, &script);
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Rule));
        assert!(
            d.reason.contains("stored secret Deploy key"),
            "{}",
            d.reason
        );
    }

    #[test]
    fn inherent_risk_and_scripts() {
        let c = config();
        let s = scope("dev", None);
        let mut push = request(Capability::GitWrite, &[], None);
        push.summary = "push to origin";
        push.inherent = Some((SensitiveKind::Outbound, "it sends commits to a server"));
        let d = eval(&c, &s, &push);
        assert_eq!(d.verdict, Verdict::Ask);
        assert!(
            d.reason.starts_with("Push to origin needs your approval"),
            "{}",
            d.reason
        );
        let mut script = request(Capability::PowershellExec, &[], None);
        script.script = Some("Get-ChildItem");
        assert_eq!(
            eval(&c, &s, &script).verdict,
            Verdict::Ask,
            "developers ask for scripts"
        );
    }

    #[test]
    fn websites_are_checked_against_the_lists() {
        let mut c = config();
        c.assign_role("web", Some("web-assistant")).unwrap();
        let s = Scope {
            role_id: "web".into(),
            role_name: "Web Assistant".into(),
            ..Scope::default()
        };
        let visit = |c: &GuardConfig, address: &str, approved: bool| {
            let site = Site::parse(address).unwrap();
            let mut r = request(Capability::BrowserNavigate, &[], None);
            r.summary = "open the page";
            r.site = Some(SiteCheck {
                site: &site,
                approved,
            });
            eval(c, &s, &r)
        };
        let blocked = visit(&c, "https://www.linkedin.com/feed", false);
        assert_eq!(
            (blocked.verdict, blocked.layer),
            (Verdict::Deny, Layer::Rule)
        );
        assert!(
            blocked.reason.contains("blocked websites list"),
            "{}",
            blocked.reason
        );
        let unknown = visit(&c, "https://example.org/", false);
        assert_eq!(unknown.verdict, Verdict::Ask);
        assert!(unknown.reason.contains("not on your allowed websites list"));
        assert_eq!(
            visit(&c, "https://example.org/", true).verdict,
            Verdict::Allow,
            "approved for this step"
        );
        let local = visit(&c, "http://192.168.1.1/", true);
        assert_eq!((local.verdict, local.layer), (Verdict::Deny, Layer::Target));
        c.set_websites(&WebsiteRules {
            allowed: vec!["example.org".into(), "192.168.1.1".into()],
            blocked: vec![],
            others: OtherSites::Block,
        })
        .unwrap();
        assert_eq!(
            visit(&c, "https://www.example.org/", false).verdict,
            Verdict::Allow
        );
        assert_eq!(
            visit(&c, "http://192.168.1.1/", false).verdict,
            Verdict::Allow
        );
        assert_eq!(
            visit(&c, "https://other.example/", true).verdict,
            Verdict::Deny,
            "other websites blocked"
        );
        // A sensitive action on an allowed website still asks.
        let site = Site::parse("https://example.org/checkout").unwrap();
        let mut buy = request(Capability::BrowserAutomate, &[], None);
        buy.summary = "click \"Place order\"";
        buy.site = Some(SiteCheck {
            site: &site,
            approved: false,
        });
        buy.inherent = Some((SensitiveKind::Payment, "it looks like buying something"));
        let d = eval(&c, &s, &buy);
        assert_eq!(
            (d.verdict, d.sensitive),
            (Verdict::Ask, Some(SensitiveKind::Payment))
        );
        // The Researcher set may visit but not use websites.
        c.assign_role("res", Some("researcher")).unwrap();
        let r = Scope {
            role_id: "res".into(),
            role_name: "Researcher".into(),
            ..Scope::default()
        };
        assert_eq!(eval(&c, &r, &buy).verdict, Verdict::Deny);
    }

    /// The owner's switches (ADR-023): a feature switched off is off for every worker; the
    /// website switches let sending, buying, and signing in go ahead without asking, but only
    /// on allowed websites, never over a "blocked" rule, and never past a role's "ask me".
    #[test]
    fn switches_turn_features_off_and_let_website_actions_go_ahead() {
        let mut c = config();
        c.assign_role("web", Some("web-assistant")).unwrap();
        c.set_websites(&WebsiteRules {
            allowed: vec!["shop.example".into()],
            blocked: vec![],
            others: OtherSites::Ask,
        })
        .unwrap();
        let s = Scope {
            role_id: "web".into(),
            role_name: "Web Assistant".into(),
            ..Scope::default()
        };
        // Defaults: the browser on, the screen off, every website action asks.
        assert!(!Switches::default().desktop);
        assert_eq!(
            level_for(&c, &s, Capability::BrowserAutomate).level,
            Level::Allowed
        );
        let act = |c: &GuardConfig, address: &str, kind: SensitiveKind| {
            let site = Site::parse(address).unwrap();
            let mut r = request(Capability::BrowserAutomate, &[], None);
            r.summary = "click \"Send\"";
            r.site = Some(SiteCheck {
                site: &site,
                approved: true,
            });
            r.inherent = Some((kind, "it submits a form"));
            eval(c, &s, &r)
        };
        let shop = "https://shop.example/form";
        assert_eq!(act(&c, shop, SensitiveKind::Outbound).verdict, Verdict::Ask);
        c.set_switches(&Switches {
            send_without_asking: true,
            ..Switches::default()
        });
        let sent = act(&c, shop, SensitiveKind::Outbound);
        assert_eq!(sent.verdict, Verdict::Allow);
        assert!(sent
            .checks
            .iter()
            .any(|k| k.note.contains("without asking (Settings → Switches)")));
        // Only sending: buying still asks; and only on allowed websites.
        assert_eq!(act(&c, shop, SensitiveKind::Payment).verdict, Verdict::Ask);
        assert_eq!(
            act(&c, "https://other.example/form", SensitiveKind::Outbound).verdict,
            Verdict::Ask,
            "a website you approved once is not on the allowed list"
        );
        // A "blocked" rule wins over the switch.
        c.set_sensitive(SensitiveKind::Outbound, SensitiveRule::Block);
        assert_eq!(
            act(&c, shop, SensitiveKind::Outbound).verdict,
            Verdict::Deny
        );
        c.set_sensitive(SensitiveKind::Outbound, SensitiveRule::Ask);
        // Buying and signing in have their own switches.
        c.set_switches(&Switches {
            buy_without_asking: true,
            sign_in_without_asking: true,
            ..Switches::default()
        });
        assert_eq!(
            act(&c, shop, SensitiveKind::Payment).verdict,
            Verdict::Allow
        );
        assert_eq!(act(&c, shop, SensitiveKind::SignIn).verdict, Verdict::Allow);
        assert_eq!(act(&c, shop, SensitiveKind::Outbound).verdict, Verdict::Ask);
        // Switching the browser off blocks both browser permissions, whatever the role's set.
        c.set_switches(&Switches {
            browser: false,
            ..Switches::default()
        });
        let off = act(&c, shop, SensitiveKind::Outbound);
        assert_eq!((off.verdict, off.layer), (Verdict::Deny, Layer::Rule));
        assert!(
            off.reason.contains("Plenipo's browser is switched off"),
            "{}",
            off.reason
        );
        assert_eq!(
            levels_for(&c, &s)[&Capability::BrowserNavigate],
            Level::Blocked
        );
        // The screen, mouse, and keyboard stay off until switched on.
        c.assign_role("desk", Some("computer-use")).unwrap();
        let desk = Scope {
            role_id: "desk".into(),
            role_name: "Desk Operator".into(),
            ..Scope::default()
        };
        assert_eq!(
            level_for(&c, &desk, Capability::ComputerObserve).level,
            Level::Blocked
        );
        c.set_switches(&Switches {
            desktop: true,
            ..Switches::default()
        });
        assert_eq!(
            level_for(&c, &desk, Capability::ComputerObserve).level,
            Level::Allowed
        );
    }

    #[test]
    fn servers_are_checked_with_their_settings() {
        use crate::servers::*;
        let mut c = config();
        // Remote computers (SSH) start switched off (ADR-023).
        assert!(!c.switches.servers);
        c.set_switches(&Switches {
            servers: true,
            ..Switches::default()
        });
        c.assign_role("ops", Some("servers")).unwrap();
        let s = Scope {
            role_id: "ops".into(),
            role_name: "Operations Engineer".into(),
            ..Scope::default()
        };
        let (server, _) = c
            .save_server(
                &ServerInput {
                    name: "Shop".into(),
                    host: "shop.example.com".into(),
                    user: "deploy".into(),
                    environment: Environment::Production,
                    roles: vec!["ops".into()],
                    classes: default_classes(Environment::Production),
                    host_key: Some(HostKeyInput {
                        algorithm: "ssh-ed25519".into(),
                        fingerprint: format!("SHA256:{}", "d".repeat(43)),
                    }),
                    ..ServerInput::default()
                },
                &["ops".to_owned()],
                1,
            )
            .unwrap();
        let run = |c: &GuardConfig, role: &Scope, line: &str| {
            let mut w = line.split_whitespace();
            let cmd = CommandLine {
                program: w.next().unwrap().to_owned(),
                args: w.map(str::to_owned).collect(),
            };
            let k = classify(&cmd);
            let mut r = request(Capability::SshConnect, &[], None);
            r.summary = "run it on Shop";
            r.server = Some(ServerCheck {
                server: &server,
                role_id: &role.role_id,
                role_name: &role.role_name,
                what: ServerUse::Run {
                    classified: &k,
                    cwd: None,
                },
            });
            eval(c, role, &r)
        };
        let d = run(&c, &s, "uptime");
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Rule));
        assert!(d.reason.contains("production server"), "{}", d.reason);
        let d = run(&c, &s, "rm -rf /tmp/x");
        assert_eq!(d.verdict, Verdict::Deny);
        // A role without the Servers set never gets as far as the server.
        let d = run(&c, &scope("rev", None), "uptime");
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Role));
        // The owner's rule for running as administrator applies on servers too.
        let mut c2 = c.clone();
        c2.servers[0].classes.push(CommandClass::Admin);
        c2.set_sensitive(SensitiveKind::Privilege, SensitiveRule::Block);
        let server2 = c2.servers[0].clone();
        let cmd = CommandLine::new("sudo", &["systemctl", "restart", "nginx"]);
        let k = classify(&cmd);
        let mut r = request(Capability::SshConnect, &[], None);
        r.summary = "restart nginx";
        r.server = Some(ServerCheck {
            server: &server2,
            role_id: "ops",
            role_name: "Operations Engineer",
            what: ServerUse::Run {
                classified: &k,
                cwd: None,
            },
        });
        let d = eval(&c2, &s, &r);
        assert_eq!(
            (d.verdict, d.sensitive),
            (Verdict::Deny, Some(SensitiveKind::Privilege))
        );
        // A change on a production server is also "deploying or changing live systems": it asks
        // with that kind, and the owner's "Blocked" for it blocks every change on production
        // servers, even one that is also another kind. Looking around still only asks.
        let d = run(&c, &s, "systemctl restart nginx");
        assert_eq!(
            (d.verdict, d.layer, d.sensitive),
            (Verdict::Ask, Layer::Risk, Some(SensitiveKind::Production))
        );
        assert!(
            d.reason.contains("it changes a production server"),
            "{}",
            d.reason
        );
        let mut c3 = c2.clone();
        c3.set_sensitive(SensitiveKind::Privilege, SensitiveRule::Ask);
        c3.set_sensitive(SensitiveKind::Production, SensitiveRule::Block);
        let d = run(&c3, &s, "systemctl restart nginx");
        assert_eq!(
            (d.verdict, d.sensitive),
            (Verdict::Deny, Some(SensitiveKind::Production))
        );
        let d = eval(&c3, &s, &r);
        assert_eq!(
            (d.verdict, d.sensitive),
            (Verdict::Deny, Some(SensitiveKind::Production)),
            "sudo on production: the blocked kind wins over running as administrator"
        );
        let d = run(&c3, &s, "uptime");
        assert_eq!((d.verdict, d.sensitive), (Verdict::Ask, None));
        // Switched off: blocked for every role, whatever the server says.
        c.set_switches(&Switches::default());
        let d = run(&c, &s, "uptime");
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Rule));
        assert!(
            d.reason.contains("remote computers (SSH) are switched off"),
            "{}",
            d.reason
        );
        assert_eq!(levels_for(&c, &s)[&Capability::SshConnect], Level::Blocked);
    }

    #[test]
    fn revoked_and_narrowed_grants() {
        let c = config();
        let s = scope("dev", None);
        let files = vec!["a.txt".to_owned()];
        let r = request(Capability::FilesystemWrite, &files, None);
        let now = level_for(&c, &s, r.capability);
        let d = evaluate(
            &c,
            &r,
            &now,
            GrantState {
                level: Level::Allowed,
                revoked: true,
            },
        );
        assert_eq!((d.verdict, d.layer), (Verdict::Deny, Layer::Grant));
        // Settings changed after the grant: the stricter of the two applies.
        let mut c2 = c.clone();
        c2.save_set(&PermissionSetInput {
            id: Some("developer".into()),
            name: "Developer".into(),
            description: String::new(),
            levels: [(Capability::FilesystemRead, Level::Allowed)]
                .into_iter()
                .collect(),
        })
        .unwrap();
        let now = level_for(&c2, &s, r.capability);
        let d = evaluate(
            &c2,
            &r,
            &now,
            GrantState {
                level: Level::Allowed,
                revoked: false,
            },
        );
        assert_eq!(d.verdict, Verdict::Deny);
        // …and a grant never widens: a worker that started with "ask" keeps asking.
        let now = level_for(&c, &s, r.capability);
        let d = evaluate(
            &c,
            &r,
            &now,
            GrantState {
                level: Level::Ask,
                revoked: false,
            },
        );
        assert_eq!((d.verdict, d.layer), (Verdict::Ask, Layer::Grant));
    }
}
