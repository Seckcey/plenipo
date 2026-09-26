//! The sensitive-action check: recognizes the plan's kinds of actions that need the owner's
//! approval by default in a command line or a script. It is a safety net, not the main
//! control — the main controls are the permission levels, the approved-command list (anything
//! not on it asks), and the blocked lists. It errs on the side of asking.

use std::path::Path;

use crate::commands::CommandLine;
use crate::dto::SensitiveKind;

/// The words of a command line or script, in lower case.
struct Words {
    /// The program (command lines only).
    program: Option<String>,
    list: Vec<String>,
    joined: String,
}

impl Words {
    fn command(cmd: &CommandLine) -> Self {
        let list = cmd.words_lower();
        Self {
            program: list.first().cloned(),
            joined: list.join(" "),
            list,
        }
    }

    fn script(text: &str) -> Self {
        let lower = text.to_lowercase();
        let list: Vec<String> = lower
            .split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':')))
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect();
        Self {
            program: None,
            joined: list.join(" "),
            list,
        }
    }

    /// The program is one of `names` (for a script: any word is).
    fn program_is(&self, names: &[&str]) -> bool {
        match &self.program {
            Some(p) => names.contains(&p.as_str()),
            None => self.list.iter().any(|w| names.contains(&w.as_str())),
        }
    }

    fn has(&self, word: &str) -> bool {
        self.list.iter().any(|w| w == word)
    }

    fn has_any(&self, words: &[&str]) -> bool {
        words.iter().any(|w| self.has(w))
    }

    /// Consecutive words, e.g. `drop table`.
    fn phrase(&self, phrase: &str) -> bool {
        let padded = format!(" {} ", self.joined);
        padded.contains(&format!(" {phrase} "))
    }

    fn contains(&self, text: &str) -> bool {
        self.joined.contains(text)
    }

    /// `program` followed (anywhere later) by one of `subcommands`.
    fn program_with(&self, programs: &[&str], subcommands: &[&str]) -> bool {
        self.program_is(programs) && self.has_any(subcommands)
    }
}

const CLOUD_CLIS: &[&str] = &[
    "aws",
    "az",
    "gcloud",
    "gsutil",
    "doctl",
    "heroku",
    "fly",
    "flyctl",
    "hcloud",
    "linode-cli",
    "oci",
    "ibmcloud",
];

fn classify(w: &Words) -> Option<(SensitiveKind, &'static str)> {
    use SensitiveKind::*;
    // Privilege escalation.
    if w.program_is(&["sudo", "doas", "su", "runas", "pkexec", "gsudo", "psexec"])
        || w.phrase("-verb runas")
        || (w.program.is_none() && w.has("runas"))
    {
        return Some((Privilege, "it runs a program as administrator"));
    }
    // Credentials.
    if w.program_is(&["passwd", "chpasswd", "cmdkey", "ssh-keygen", "ssh-add"])
        || w.phrase("net user")
        || w.program_with(&["gh"], &["auth"])
        || w.program_with(&["docker", "podman"], &["login"])
        || w.program_with(&["npm", "pnpm", "yarn"], &["token", "login", "adduser"])
        || w.phrase("git credential")
        || (w.program_is(&["git"]) && w.has("config") && w.contains("credential"))
        || (w.program_is(&["aws"])
            && w.list.iter().any(|x| {
                x.ends_with("access-key") || x.ends_with("login-profile") || x == "secretsmanager"
            }))
        || (w.program_is(&["az"]) && (w.has("credential") || w.phrase("keyvault secret")))
        || (w.program_is(&["gcloud"]) && w.has("keys") && w.has("iam"))
        || w.phrase("create secret")
        || w.program_with(
            &["security"],
            &["add-generic-password", "delete-generic-password"],
        )
        || w.has_any(&[
            "set-secret",
            "remove-secret",
            "new-localuser",
            "set-localuser",
        ])
        || w.program_with(&["vault"], &["put", "delete", "write"])
        || (w.program_is(&["op"]) && w.has("item"))
    {
        return Some((
            Credentials,
            "it changes a password, key, or other credential",
        ));
    }
    // DNS.
    if w.has("route53")
        || w.phrase("network dns")
        || (w.program_is(&["gcloud"]) && w.has("dns"))
        || w.program_is(&["nsupdate"])
        || w.list
            .iter()
            .any(|x| x.contains("dnsserverresourcerecord") || x.contains("dnsclientserveraddress"))
        || w.phrase("set dns")
        || w.phrase("compute domain")
    {
        return Some((Dns, "it changes DNS records or settings"));
    }
    // Destructive database operations.
    if w.phrase("drop table")
        || w.phrase("drop database")
        || w.phrase("drop schema")
        || w.phrase("truncate table")
        || w.phrase("delete from")
        || w.has_any(&[
            "flushall",
            "flushdb",
            "dropdb",
            "db:drop",
            "db:reset",
            "dropdatabase",
        ])
        || w.contains("dropdatabase")
        || w.phrase("migrate reset")
        || w.phrase("manage.py flush")
    {
        return Some((Database, "it deletes or wipes database data"));
    }
    // Cloud resource deletion.
    if (w.program_is(CLOUD_CLIS)
        && w.list.iter().any(|x| {
            x.starts_with("delete")
                || x.starts_with("terminate")
                || x == "destroy"
                || x == "apps:destroy"
                || x == "rb"
        }))
        || w.program_with(&["terraform", "pulumi", "cdk", "tofu"], &["destroy"])
        || w.program_with(&["kubectl", "oc"], &["delete"])
        || w.program_with(&["helm"], &["uninstall", "delete"])
        || w.program_with(&["sls", "serverless"], &["remove"])
        || (w.program_is(&["gsutil"]) && w.has("rm"))
    {
        return Some((CloudDelete, "it deletes cloud resources"));
    }
    // Deploying to or changing live systems.
    if w.program_with(&["terraform", "tofu"], &["apply", "import"])
        || w.program_with(&["pulumi"], &["up", "update"])
        || w.program_with(
            &["kubectl", "oc"],
            &[
                "apply", "rollout", "scale", "set", "patch", "replace", "edit", "create",
            ],
        )
        || w.program_with(&["helm"], &["install", "upgrade", "rollback"])
        || w.program_is(&["ansible-playbook", "kamal", "cap"])
        || w.has_any(&["deploy", "--prod", "--production"])
        || w.list.iter().any(|x| x.starts_with("deploy:"))
        || w.program_with(&["railway"], &["up"])
        || w.program_with(&["az"], &["up"])
    {
        return Some((Production, "it deploys to or changes a live system"));
    }
    // Money.
    if w.program_is(&["stripe", "paypal", "braintree"])
        && !w.list.get(1).is_some_and(|a| {
            [
                "--version",
                "-v",
                "help",
                "--help",
                "logs",
                "listen",
                "get",
                "list",
            ]
            .contains(&a.as_str())
        })
        || w.has_any(&[
            "payouts",
            "refunds",
            "payment_intents",
            "charges",
            "transfers",
        ])
    {
        return Some((Payment, "it can move money"));
    }
    // Outbound: publishing, pushing, messages.
    if w.program_with(&["git"], &["push"])
        || w.program_with(
            &[
                "npm", "pnpm", "yarn", "cargo", "gem", "nuget", "twine", "poetry", "flit",
            ],
            &["publish", "push", "upload"],
        )
        || w.phrase("nuget push")
        || w.program_with(&["docker", "podman", "buildah"], &["push"])
        || (w.program_is(&["gh"])
            && (w.phrase("pr create")
                || w.phrase("pr merge")
                || w.phrase("pr comment")
                || w.phrase("pr review")
                || w.phrase("issue create")
                || w.phrase("issue comment")
                || w.phrase("release create")
                || w.phrase("repo create")
                || (w.has("api") && w.has_any(&["-x", "--method", "-f", "--field"]))))
        || w.program_is(&["sendmail", "mail", "mailx", "mutt", "twilio"])
        || w.has("send-mailmessage")
        || (w.program_is(&["curl", "wget", "invoke-webrequest", "invoke-restmethod"])
            && w.has_any(&[
                "-d",
                "--data",
                "--data-raw",
                "--data-binary",
                "-f",
                "--form",
                "-t",
                "--upload-file",
                "--post-data",
                "-body",
            ]))
        || (w.program_is(&["aws"]) && w.phrase("sns publish"))
        || (w.program_is(&["aws"]) && w.has("ses") && w.has_any(&["send-email", "send-raw-email"]))
    {
        return Some((
            Outbound,
            "it sends or publishes something outside this computer",
        ));
    }
    None
}

/// Programs that delete, move, or overwrite files.
const DESTRUCTIVE: &[&str] = &[
    "rm",
    "rmdir",
    "del",
    "erase",
    "rd",
    "remove-item",
    "mv",
    "move",
    "move-item",
    "cp",
    "copy",
    "copy-item",
    "xcopy",
    "robocopy",
    "rsync",
    "dd",
    "truncate",
    "shred",
    "set-content",
    "out-file",
];

/// An argument that names a place outside `root`: an absolute path elsewhere, or `..` that
/// climbs out (relative arguments are taken from `root`).
fn escapes(arg: &str, root: &Path) -> bool {
    let arg = arg.trim_matches(['"', '\'']);
    let arg = arg
        .split_once('=')
        .filter(|(k, _)| k.starts_with('-'))
        .map_or(arg, |(_, v)| v);
    if arg.is_empty() || (arg.starts_with('-') && !arg.contains(['/', '\\'])) {
        return false;
    }
    let b = arg.as_bytes();
    let absolute = arg.starts_with('/')
        || arg.starts_with('\\')
        || arg.starts_with('~')
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':');
    if absolute {
        let lower = arg.replace('\\', "/").to_lowercase();
        let root = root.display().to_string().replace('\\', "/").to_lowercase();
        return !(lower == root || lower.starts_with(&format!("{root}/")));
    }
    if !arg.contains("..") {
        return false;
    }
    let mut depth: i64 = 0;
    for part in arg.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => depth += 1,
        }
    }
    false
}

/// The sensitive kind of running `cmd` in `workspace`, with a short reason ("it deletes cloud
/// resources").
pub fn command(cmd: &CommandLine, workspace: &Path) -> Option<(SensitiveKind, &'static str)> {
    let words = Words::command(cmd);
    if let Some(hit) = classify(&words) {
        return Some(hit);
    }
    if words.program_is(DESTRUCTIVE) && cmd.args.iter().any(|a| escapes(a, workspace)) {
        return Some((
            SensitiveKind::OutsideWorkspace,
            "it deletes, moves, or overwrites files outside the project folder",
        ));
    }
    None
}

/// The sensitive kind of a script (best effort over its words).
pub fn script(text: &str) -> Option<(SensitiveKind, &'static str)> {
    classify(&Words::script(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use SensitiveKind::*;

    fn kind(line: &str) -> Option<SensitiveKind> {
        let mut w = line.split_whitespace();
        let cmd = CommandLine {
            program: w.next().unwrap().to_owned(),
            args: w.map(str::to_owned).collect(),
        };
        command(&cmd, Path::new("/home/me/proj")).map(|(k, _)| k)
    }

    #[test]
    fn the_plans_sensitive_actions_are_recognized() {
        for (line, want) in [
            ("vercel --prod", Production),
            ("terraform apply -auto-approve", Production),
            ("kubectl apply -f deploy.yaml", Production),
            ("npm run deploy", Production),
            ("firebase deploy", Production),
            ("aws route53 change-resource-record-sets --x", Dns),
            ("az network dns record-set a add-record", Dns),
            ("passwd", Credentials),
            ("gh auth login", Credentials),
            ("aws iam create-access-key", Credentials),
            ("docker login", Credentials),
            ("psql -c DROP TABLE users", Database),
            ("redis-cli flushall", Database),
            ("npx prisma migrate reset", Database),
            ("aws ec2 terminate-instances --ids i-1", CloudDelete),
            ("az group delete -n rg", CloudDelete),
            ("terraform destroy", CloudDelete),
            ("kubectl delete pod x", CloudDelete),
            ("stripe refunds create --charge ch_1", Payment),
            ("git push origin main", Outbound),
            ("npm publish", Outbound),
            ("cargo publish", Outbound),
            ("docker push me/app", Outbound),
            ("gh pr create --fill", Outbound),
            ("curl -X POST -d x https://example.com", Outbound),
            ("sudo apt install x", Privilege),
            ("runas /user:admin cmd", Privilege),
            ("rm -rf /home/me/other", OutsideWorkspace),
            ("rm -rf ../other", OutsideWorkspace),
            ("del C:\\Windows\\x", OutsideWorkspace),
            ("mv --target-directory=/tmp x", OutsideWorkspace),
        ] {
            assert_eq!(kind(line), Some(want), "{line}");
        }
    }

    #[test]
    fn everyday_development_commands_are_not_sensitive() {
        for line in [
            "cargo test --workspace",
            "npm test",
            "npm run build",
            "pnpm lint",
            "python -m pytest -q",
            "git status",
            "go test ./...",
            "rm -rf target",
            "rm src/old.rs",
            "rm /home/me/proj/tmp.txt",
            "stripe --version",
            "gh pr list",
            "curl https://example.com",
        ] {
            assert_eq!(kind(line), None, "{line}");
        }
    }

    #[test]
    fn scripts_are_checked_too() {
        assert_eq!(
            script("Start-Process powershell -Verb RunAs").map(|k| k.0),
            Some(Privilege)
        );
        assert_eq!(
            script("Invoke-Sqlcmd -Query 'DROP TABLE Customers'").map(|k| k.0),
            Some(Database)
        );
        assert_eq!(
            script("Send-MailMessage -To a@b.c").map(|k| k.0),
            Some(Outbound)
        );
        assert_eq!(script("Get-ChildItem | Measure-Object").map(|k| k.0), None);
    }
}
