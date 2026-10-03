//! What Plenipo starts with: built-in permission sets, command lists, blocked files, and the
//! permission set each built-in role template starts with. All of it is the owner's to change.

use std::collections::BTreeMap;

use crate::dto::{CommandRules, Level, PermissionSet};
use crate::registry::Capability;
use crate::websites::{OtherSites, WebsiteRules};

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
            "Reads files and git history in the project folder, and the project on GitHub. \
             Changes nothing.",
            &[
                (FilesystemRead, Allowed),
                (GitRead, Allowed),
                (GithubRead, Allowed),
            ],
        ),
        set(
            "developer",
            "Developer",
            "Reads and changes files, runs approved development commands, commits, and reads \
             GitHub. Other programs, PowerShell scripts, pushing, and opening pull requests ask \
             you first.",
            &[
                (FilesystemRead, Allowed),
                (FilesystemWrite, Allowed),
                (ShellExec, Allowed),
                (PowershellExec, Ask),
                (GitRead, Allowed),
                (GitWrite, Allowed),
                (GithubRead, Allowed),
                (GithubWrite, Allowed),
            ],
        ),
        set(
            "reviewer",
            "Reviewer",
            "Reads files, git history, and GitHub. Running a program asks you first.",
            &[
                (FilesystemRead, Allowed),
                (GitRead, Allowed),
                (ShellExec, Ask),
                (GithubRead, Allowed),
            ],
        ),
        set(
            "tester",
            "Tester",
            "Reads files, git history, and GitHub checks, and runs approved test commands.",
            &[
                (FilesystemRead, Allowed),
                (GitRead, Allowed),
                (ShellExec, Allowed),
                (GithubRead, Allowed),
            ],
        ),
        set(
            "writer",
            "Writer",
            "Reads and changes files, reads git history, and commits. Runs nothing; pushing \
             asks you first.",
            &[
                (FilesystemRead, Allowed),
                (FilesystemWrite, Allowed),
                (GitRead, Allowed),
                (GitWrite, Allowed),
            ],
        ),
        set(
            "researcher",
            "Researcher",
            "Opens and reads web pages and takes screenshots on the websites your lists allow. \
             Clicks and types nothing. No access to project files.",
            &[(BrowserNavigate, Allowed)],
        ),
        set(
            "web-assistant",
            "Web assistant",
            "Opens, reads, and uses web pages on the websites your lists allow: clicks, types, \
             and chooses. Submitting forms, buying, signing in, and sending always ask you \
             first. No access to project files.",
            &[(BrowserNavigate, Allowed), (BrowserAutomate, Allowed)],
        ),
        set(
            "computer-use",
            "Screen, mouse, and keyboard",
            "Sees the screen and, as a last resort, uses the mouse and keyboard. Taking control \
             always asks you first, with the worker's reason.",
            &[(ComputerObserve, Allowed), (ComputerControl, Allowed)],
        ),
        set(
            "servers",
            "Servers",
            "Runs commands over SSH on the servers you set up, as each server allows: which \
             roles may use it, what kinds of commands, and in which folders. On production \
             servers every command asks you first. No access to project files.",
            &[(SshConnect, Allowed)],
        ),
        set(
            "no-access",
            "No access",
            "Conversation only: no files, programs, or git.",
            &[],
        ),
    ]
}

/// Built-in sets as earlier versions of Plenipo made them (before GitHub tools, Phase 8; before
/// the Writer set could commit and websites had tools, v1.3): a set the owner never changed is
/// brought up to date; a changed one is left alone.
pub fn earlier_sets() -> Vec<PermissionSet> {
    use Capability::*;
    use Level::*;
    vec![
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
        ("Web Assistant", "web-assistant"),
        ("Operations Engineer", "servers"),
    ]
}

/// The websites workers start without: sites whose terms forbid automated use (the owner can
/// remove one after checking its terms, ADR-020). Every other website asks the first time.
pub fn default_websites() -> WebsiteRules {
    WebsiteRules {
        allowed: Vec::new(),
        blocked: list(&[
            "linkedin.com",
            "facebook.com",
            "instagram.com",
            "x.com",
            "twitter.com",
            "tiktok.com",
            "amazon.com",
        ]),
        others: OtherSites::Ask,
    }
}

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

/// Everyday build, test, and lint commands; programs that delete, download, reach other
/// computers, run a shell, or change the system. Script runners (`npm run`, `make test`) run
/// a project's own scripts with the owner's account, so they are not approved for every
/// project: the owner approves them where the project is trusted (ADR-034).
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
            "npx tsc *",
            "npx eslint *",
            "npx prettier *",
            "npx vitest *",
            "npx jest *",
            "pnpm test *",
            "pnpm lint *",
            "pnpm build *",
            "pnpm typecheck *",
            "pnpm check *",
            "yarn test *",
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
            "make lint *",
            "make check *",
        ]),
        // Programs that open a file, a web page, or another app the way the owner's desktop
        // would (Phase 23, ADR-150): a worker asks first. Linux desktops have several.
        ask: list(&[
            "open *",
            "xdg-open *",
            "gio *",
            "gnome-open *",
            "gvfs-open *",
            "kde-open *",
            "kde-open5 *",
            "exo-open *",
            "sensible-browser *",
            "x-www-browser *",
            "www-browser *",
        ]),
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
            "fish *",
            "wsl *",
            // A Mac's and Linux's own system tools (Phase 23, ADR-150): AppleScript, which can
            // drive any app; the Keychain; background services and settings; disks; and the
            // Mac's privacy permissions and app checks.
            "osascript *",
            "security *",
            "launchctl *",
            "defaults *",
            "diskutil *",
            "csrutil *",
            "tccutil *",
            "spctl *",
            "systemsetup *",
            "systemctl *",
            "pkexec *",
            // The Phase 23 Guard review: a Mac's and Linux's tools that would reach past Guard.
            // Moving the mouse and typing, or seeing the screen, outside computer use (which asks
            // every step and shows the sign, ADR-049); reading the clipboard; reading the
            // password store, where Plenipo's own keys are kept on Linux.
            "xdotool *",
            "ydotool *",
            "wtype *",
            "xte *",
            "cliclick *",
            "screencapture *",
            "scrot *",
            "grim *",
            "gnome-screenshot *",
            "spectacle *",
            "import *",
            "xwd *",
            "pbpaste *",
            "xclip *",
            "xsel *",
            "wl-paste *",
            "secret-tool *",
            "kwallet-query *",
            "kwalletcli *",
            "keyring *",
            "pass *",
            // Starting programs outside Plenipo's reach, which Stop would not end (ADR-158).
            "setsid *",
            "systemd-run *",
            "at *",
            "batch *",
            "loginctl *",
            "daemonize *",
            "start-stop-daemon *",
            // The desktop's own services, which can do most of the above; a Mac's automations.
            "dbus-send *",
            "gdbus *",
            "busctl *",
            "qdbus *",
            "shortcuts *",
            "automator *",
            // Settings, as `defaults` and `reg` are: the desktop's, the network's, which program
            // opens which file.
            "gsettings *",
            "dconf *",
            "kwriteconfig5 *",
            "kwriteconfig6 *",
            "xdg-settings *",
            "xdg-mime *",
            "networksetup *",
            "scutil *",
            "nmcli *",
            // Turning the computer off, and ending programs by name (Plenipo's own among them).
            "poweroff *",
            "halt *",
            "pkill *",
            "killall *",
            // A network tool, and more shells.
            "socat *",
            "dash *",
            "ksh *",
            "csh *",
            "tcsh *",
            "mksh *",
            "busybox *",
            "nu *",
        ]),
        // No program is given a stored secret without asking until the owner says so
        // (ADR-048).
        with_secrets: Vec::new(),
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
        let w = default_websites();
        assert_eq!(crate::websites::clean(&w).unwrap(), w);
    }

    /// The Phase 23 Guard review: a Mac's and Linux's tools that would reach past Guard are
    /// blocked however they are written, and the desktop's other "open" programs ask.
    #[test]
    fn mac_and_linux_tools_that_reach_past_guard_are_blocked() {
        use crate::commands::{first_catch, CommandLine};
        let c = default_commands();
        for line in [
            "xdotool type hello",
            "screencapture -x shot.png",
            "pbpaste",
            "secret-tool search --all service plenipo",
            "setsid sleep 100",
            "systemd-run --user sleep 100",
            "busctl --user list",
            "shortcuts run Anything",
            "gsettings set org.gnome.system.proxy mode manual",
            "pkill -f plenipo",
            "KILLALL Plenipo",
            "osascript -e beep",
        ] {
            let mut words = line.split_whitespace();
            let program = words.next().unwrap();
            let args: Vec<&str> = words.collect();
            let cmd = CommandLine::new(program, &args);
            assert!(first_catch(&c.blocked, &cmd).is_some(), "{line}");
        }
        for line in [
            "gio open notes.pdf",
            "kde-open notes.pdf",
            "xdg-open notes.pdf",
        ] {
            let mut words = line.split_whitespace();
            let program = words.next().unwrap();
            let args: Vec<&str> = words.collect();
            let cmd = CommandLine::new(program, &args);
            assert!(first_catch(&c.ask, &cmd).is_some(), "{line}");
            assert!(first_catch(&c.blocked, &cmd).is_none(), "{line}");
        }
        for r in &c.ask {
            assert_eq!(&valid_rule(r).unwrap(), r);
        }
    }

    /// Script runners run a project's own scripts with the owner's account, so they are
    /// approved per project, never for everyone (ADR-034).
    #[test]
    fn script_runners_are_not_approved_by_default() {
        let c = default_commands();
        for runner in [
            "npm run *",
            "pnpm run *",
            "yarn run *",
            "make test *",
            "make build *",
        ] {
            assert!(!c.approved.contains(&runner.to_owned()), "{runner}");
        }
        for kept in [
            "cargo test *",
            "cargo build *",
            "python -m pytest *",
            "./gradlew test *",
        ] {
            assert!(c.approved.contains(&kept.to_owned()), "{kept}");
        }
    }
}
