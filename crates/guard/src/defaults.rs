//! What Plenipo starts with: built-in permission sets, command lists, blocked files, and the
//! permission set each built-in role template starts with. All of it is the owner's to change.

use std::collections::BTreeMap;

use crate::dto::{CommandRules, Level, PermissionSet};
use crate::registry::Capability;

fn set(id: &str, name: &str, description: &str, levels: &[(Capability, Level)]) -> PermissionSet {
    PermissionSet {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        levels: levels.iter().copied().collect::<BTreeMap<_, _>>(),
        built_in: true,
    }
}

/// The built-in permission sets.
pub fn builtin_sets() -> Vec<PermissionSet> {
    use Capability::*;
    use Level::*;
    vec![
        set(
            "read-only",
            "Read only",
            "Reads files and git history in the project folder. Changes nothing.",
            &[(FilesystemRead, Allowed), (GitRead, Allowed)],
        ),
        set(
            "developer",
            "Developer",
            "Reads and changes files, runs approved development commands, and commits. \
             Other programs, PowerShell scripts, and pushing ask you first.",
            &[
                (FilesystemRead, Allowed),
                (FilesystemWrite, Allowed),
                (ShellExec, Allowed),
                (PowershellExec, Ask),
                (GitRead, Allowed),
                (GitWrite, Allowed),
            ],
        ),
        set(
            "reviewer",
            "Reviewer",
            "Reads files and git history. Running a program asks you first.",
            &[
                (FilesystemRead, Allowed),
                (GitRead, Allowed),
                (ShellExec, Ask),
            ],
        ),
        set(
            "tester",
            "Tester",
            "Reads files and git history and runs approved test commands.",
            &[
                (FilesystemRead, Allowed),
                (GitRead, Allowed),
                (ShellExec, Allowed),
            ],
        ),
        set(
            "writer",
            "Writer",
            "Reads and changes files and reads git history. Runs nothing.",
            &[
                (FilesystemRead, Allowed),
                (FilesystemWrite, Allowed),
                (GitRead, Allowed),
            ],
        ),
        set(
            "researcher",
            "Researcher",
            "Visits websites (arrives in Phase 10). No access to project files.",
            &[(BrowserNavigate, Allowed)],
        ),
        set(
            "no-access",
            "No access",
            "Conversation only: no files, programs, or git.",
            &[],
        ),
    ]
}

/// The permission set each built-in role template starts with (by template name).
pub fn template_sets() -> &'static [(&'static str, &'static str)] {
    &[
        ("Supervisor", "read-only"),
        ("Senior Developer", "developer"),
        ("Code Reviewer", "reviewer"),
        ("Security Auditor", "reviewer"),
        ("QA Engineer", "tester"),
        ("Documentation Writer", "writer"),
        ("Designer", "writer"),
        ("Researcher", "researcher"),
    ]
}

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

/// Everyday build, test, and lint commands; programs that delete, download, reach other
/// computers, run a shell, or change the system.
pub fn default_commands() -> CommandRules {
    CommandRules {
        approved: list(&[
            "cargo build *",
            "cargo check *",
            "cargo test *",
            "cargo fmt *",
            "cargo clippy *",
            "cargo doc *",
            "cargo tree *",
            "npm test *",
            "npm run *",
            "npx tsc *",
            "npx eslint *",
            "npx prettier *",
            "npx vitest *",
            "npx jest *",
            "pnpm test *",
            "pnpm run *",
            "pnpm lint *",
            "pnpm build *",
            "pnpm typecheck *",
            "pnpm check *",
            "yarn test *",
            "yarn run *",
            "yarn build *",
            "yarn lint *",
            "tsc *",
            "eslint *",
            "prettier *",
            "vitest *",
            "jest *",
            "python -m pytest *",
            "python -m unittest *",
            "py -m pytest *",
            "pytest *",
            "ruff *",
            "black *",
            "mypy *",
            "go build *",
            "go test *",
            "go vet *",
            "gofmt *",
            "dotnet build *",
            "dotnet test *",
            "dotnet format *",
            "mvn test *",
            "mvn verify *",
            "mvn package *",
            "gradle test *",
            "gradle build *",
            "./gradlew test *",
            "./gradlew build *",
            "make test *",
            "make build *",
            "make lint *",
            "make check *",
        ]),
        ask: Vec::new(),
        blocked: list(&[
            "rm *",
            "rmdir *",
            "del *",
            "erase *",
            "rd *",
            "Remove-Item *",
            "format *",
            "diskpart *",
            "mkfs *",
            "dd *",
            "shutdown *",
            "reboot *",
            "Restart-Computer *",
            "Stop-Computer *",
            "reg *",
            "regedit *",
            "bcdedit *",
            "sudo *",
            "su *",
            "doas *",
            "runas *",
            "curl *",
            "wget *",
            "Invoke-WebRequest *",
            "Invoke-RestMethod *",
            "iwr *",
            "irm *",
            "ssh *",
            "scp *",
            "sftp *",
            "ftp *",
            "nc *",
            "ncat *",
            "telnet *",
            "chown *",
            "takeown *",
            "icacls *",
            "net *",
            "netsh *",
            "sc *",
            "schtasks *",
            "crontab *",
            "cmd *",
            "powershell *",
            "pwsh *",
            "bash *",
            "sh *",
            "zsh *",
            "wsl *",
        ]),
    }
}

/// Files no worker may open or change (gitignore-style; `!` makes an exception).
pub fn default_blocked_files() -> Vec<String> {
    list(&[
        ".env",
        ".env.*",
        "!.env.example",
        "!.env.sample",
        "!.env.template",
        "*.pem",
        "*.key",
        "*.pfx",
        "*.p12",
        "*.kdbx",
        "id_rsa*",
        "id_dsa*",
        "id_ecdsa*",
        "id_ed25519*",
        ".git-credentials",
        ".netrc",
        "_netrc",
        ".npmrc",
        ".pypirc",
        "credentials.json",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::valid_rule;
    use crate::paths::valid_pattern;

    #[test]
    fn defaults_are_valid() {
        let sets = builtin_sets();
        let mut ids: Vec<_> = sets.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), sets.len());
        for (_, id) in template_sets() {
            assert!(sets.iter().any(|s| s.id == *id), "{id}");
        }
        let c = default_commands();
        for r in c.approved.iter().chain(&c.blocked) {
            assert_eq!(&valid_rule(r).unwrap(), r);
        }
        for p in default_blocked_files() {
            assert_eq!(valid_pattern(&p).unwrap(), p);
        }
    }
}
