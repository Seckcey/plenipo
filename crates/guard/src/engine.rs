//! The evaluation engine (pure): a worker's permission level for a capability from its layers,
//! and Guard's decision about one action. The plan's layers, in order: role policy, project
//! policy, department policy, target resource, action risk class, explicit user approval rules.
//! The strictest answer wins.

use std::collections::BTreeMap;
use std::path::Path;

use crate::commands::{first_match, CommandLine};
use crate::config::GuardConfig;
use crate::dto::*;
use crate::paths::blocked_by;
use crate::registry::Capability;
use crate::sensitive;

/// Who is acting, as far as permissions go: the worker's role, and the project and department
/// its work belongs to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub role_id: String,
    pub role_name: String,
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
    let mut level = role_level;
    let mut layer = Layer::Role;
    let mut reason = role_note;

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
    LevelFor {
        level,
        layer,
        reason,
        checks,
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
    pub workspace: &'a Path,
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
    // Layer 4: the target.
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
    if let Some(cmd) = request.command {
        if let Some(rule) = first_match(&config.commands.blocked, cmd) {
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
    let found = request
        .inherent
        .or_else(|| {
            request
                .command
                .and_then(|c| sensitive::command(c, request.workspace))
        })
        .or_else(|| request.script.and_then(sensitive::script));
    if let Some((kind, because)) = found {
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
    if let Some(cmd) = request.command {
        if let Some(rule) = first_match(&config.commands.ask, cmd) {
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
    if let Some(cmd) = request.command {
        if first_match(&config.commands.approved, cmd).is_none() {
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
    checks.push(Check {
        layer: Layer::Target,
        verdict: Verdict::Allow,
        note: "inside the project folder".into(),
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
            workspace: Path::new("/w"),
        }
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
