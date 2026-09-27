//! Servers (Phase 11, ADR-025): the owner's list of servers workers may reach over SSH, what
//! kind of command each one is, and Guard's part of the decision about a worker using one.
//!
//! A server is set up by the owner in Settings → Servers: a friendly name, its address and port,
//! who Plenipo signs in as and how (a key or password kept in the operating system's protected
//! storage, or the owner's SSH agent), the identity (host key) pinned when it was set up, whether
//! it is a development, staging, or production server, which roles may use it, which kinds of
//! commands it allows, which folders commands may run and change things in, which ports may be
//! forwarded (none to start with), and when the owner is asked. Nothing here connects anywhere:
//! the capability broker does, after asking Guard.
//!
//! Commands are a program and its arguments, never a shell line, so every command can be put in
//! one of a few plain kinds ([`CommandClass`]) and checked word by word. Tools that would reach
//! other computers from a server, look for passwords, or watch the network are never allowed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::commands::CommandLine;
use crate::dto::{Layer, SensitiveKind};
use crate::paths::blocked_by;
use crate::sensitive;

/// Most servers in the list.
pub const MAX_SERVERS: usize = 50;
/// Most folders, forwarded ports, and roles per server.
pub const MAX_FOLDERS: usize = 20;
pub const MAX_FORWARDS: usize = 10;
pub const MAX_ROLES: usize = 100;
/// The standard SSH port.
pub const SSH_PORT: u16 = 22;

/// What a server is for. Production servers are always shown differently, and every command on
/// one waits for the owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Environment {
    #[default]
    Development,
    Staging,
    Production,
}

impl Environment {
    /// "production".
    pub fn word(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

/// How Plenipo signs in to a server. The key or password is kept in the operating system's
/// protected storage (Windows Credential Manager); the agent is the owner's own SSH agent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SignIn {
    /// A private key (and its passphrase, if it has one), kept by Plenipo's Vault.
    #[default]
    Key,
    /// A password, kept by Plenipo's Vault.
    Password,
    /// The owner's SSH agent (Windows' OpenSSH agent or Pageant). Never forwarded to the server.
    Agent,
}

/// The kinds of commands a server can allow. Every command is one of these (a command run as
/// administrator is also [`CommandClass::Admin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommandClass {
    /// Looks without changing anything: status, logs, listings, disk space.
    Look,
    /// Starts, stops, and restarts services and programs.
    Services,
    /// Installs, updates, deploys, and changes files and settings.
    Change,
    /// Deletes, wipes, or shuts down (destructive).
    Destroy,
    /// Runs as administrator (sudo and the like).
    Admin,
    /// Anything Plenipo does not recognize.
    Other,
}

impl CommandClass {
    pub const ALL: [Self; 6] = [
        Self::Look,
        Self::Services,
        Self::Change,
        Self::Destroy,
        Self::Admin,
        Self::Other,
    ];

    /// Plain name, e.g. "Look around".
    pub fn label(self) -> &'static str {
        match self {
            Self::Look => "Look around",
            Self::Services => "Start, stop, and restart services",
            Self::Change => "Install, deploy, and change files",
            Self::Destroy => "Delete, wipe, or shut down",
            Self::Admin => "Run as administrator",
            Self::Other => "Other commands",
        }
    }

    /// Examples shown in Settings.
    pub fn examples(self) -> &'static str {
        match self {
            Self::Look => {
                "ls, cat, tail, df, ps, systemctl status, journalctl, docker ps, git status, \
                 wp plugin list"
            }
            Self::Services => {
                "systemctl restart nginx, service apache2 reload, docker restart, pm2 restart"
            }
            Self::Change => {
                "git pull, apt install, npm ci, docker compose up, mkdir, cp, wp plugin update"
            }
            Self::Destroy => {
                "rm, docker rm, git reset --hard, apt remove, wp db reset, DROP TABLE, reboot"
            }
            Self::Admin => "sudo …, doas …",
            Self::Other => {
                "scripts, programming languages, database clients, curl, and anything Plenipo \
                 does not recognize"
            }
        }
    }
}

/// When the owner is asked about commands on a server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ServerApproval {
    /// Every command waits for the owner (always so on production servers).
    Every,
    /// Looking around runs at once; anything else waits for the owner.
    #[default]
    Changes,
    /// The kinds of commands the server allows run at once. Deleting, wiping, and shutting down,
    /// running as administrator, and other sensitive actions still ask.
    Allowed,
}

/// A server's identity as pinned by the owner: its host key's type and SHA-256 fingerprint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct HostKey {
    /// "ssh-ed25519".
    pub algorithm: String,
    /// "SHA256:…" (as `ssh-keygen -lf` shows it).
    pub fingerprint: String,
    #[ts(type = "number")]
    pub pinned_at: u64,
}

/// A server the owner set up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Server {
    pub id: String,
    /// The owner's name for it ("Website (production)").
    pub name: String,
    /// Host name or IP address.
    pub host: String,
    pub port: u16,
    /// Who Plenipo signs in as.
    pub user: String,
    pub environment: Environment,
    pub sign_in: SignIn,
    /// Pinned when it was set up; workers cannot use a server without one.
    #[ts(optional)]
    pub host_key: Option<HostKey>,
    /// Role IDs that may use it (none: no worker may).
    pub roles: Vec<String>,
    /// Kinds of commands it allows.
    pub classes: Vec<CommandClass>,
    pub approval: ServerApproval,
    /// Absolute folders commands run in and may change (none: the home folder of `user`).
    pub folders: Vec<String>,
    /// `host:port` destinations a worker may forward a port to, asking each time (none: off).
    pub forwards: Vec<String>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
}

impl Server {
    /// `deploy@web01.example.com:22`.
    pub fn address(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{}@{host}:{}", self.user, self.port)
    }

    /// "Website (production)".
    pub fn tag(&self) -> String {
        format!("{} ({})", self.name, self.environment.word())
    }

    pub fn allows(&self, class: CommandClass) -> bool {
        self.classes.contains(&class)
    }
}

/// A server's identity as the owner confirms it when pinning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct HostKeyInput {
    pub algorithm: String,
    pub fingerprint: String,
}

/// A server as the owner submits it (no `id`: a new one). The key, passphrase, and password are
/// sent once and kept only in the operating system's protected storage; leave them out to keep
/// what is stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct ServerInput {
    #[ts(optional)]
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub environment: Environment,
    pub sign_in: SignIn,
    #[ts(optional)]
    pub host_key: Option<HostKeyInput>,
    pub roles: Vec<String>,
    pub classes: Vec<CommandClass>,
    pub approval: ServerApproval,
    pub folders: Vec<String>,
    pub forwards: Vec<String>,
    /// A private key (OpenSSH or PEM), for [`SignIn::Key`].
    #[ts(optional)]
    pub key: Option<String>,
    /// The private key's passphrase, if it has one.
    #[ts(optional)]
    pub passphrase: Option<String>,
    /// The password, for [`SignIn::Password`].
    #[ts(optional)]
    pub password: Option<String>,
}

impl ServerInput {
    /// The same input without the secret values (what Guard's settings keep).
    pub fn without_secrets(&self) -> Self {
        Self {
            key: None,
            passphrase: None,
            password: None,
            ..self.clone()
        }
    }
}

/// The kinds of commands a new server allows to start with. Production servers start with
/// looking around and restarting services only: deleting, wiping, and shutting down are off.
pub fn default_classes(environment: Environment) -> Vec<CommandClass> {
    match environment {
        Environment::Production => vec![CommandClass::Look, CommandClass::Services],
        _ => vec![
            CommandClass::Look,
            CommandClass::Services,
            CommandClass::Change,
        ],
    }
}

// ---- Validation -------------------------------------------------------------------------------

fn one_line(what: &str, value: &str, max: usize) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() || v.chars().count() > max || v.chars().any(char::is_control) {
        return Err(format!("{what} must be one line of 1–{max} characters"));
    }
    Ok(v.to_owned())
}

/// A host name or IP address: letters, digits, `.`, `-`, `_`, or an IPv6 address (`:` and hex).
pub fn valid_host(host: &str) -> Result<String, String> {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']');
    let ok = (1..=253).contains(&h.len())
        && !h.starts_with(['-', '.'])
        && (h
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            || (h.contains(':')
                && h.chars()
                    .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')));
    if ok {
        Ok(h.to_ascii_lowercase())
    } else {
        Err(format!(
            "{:?} is not a server address: give its name (like web01.example.com) or IP address, \
             without ssh:// or a user name",
            host.trim()
        ))
    }
}

/// A user name to sign in as: 1–64 characters, no spaces, `@`, `:`, or `/`.
pub fn valid_user(user: &str) -> Result<String, String> {
    let u = user.trim();
    let ok = (1..=64).contains(&u.chars().count())
        && !u.starts_with('-')
        && u.chars()
            .all(|c| !c.is_whitespace() && !c.is_control() && !matches!(c, '@' | ':' | '/' | '\\'));
    if ok {
        Ok(u.to_owned())
    } else {
        Err(format!("{u:?} is not a user name to sign in as"))
    }
}

/// An absolute folder on a server, tidied (`/srv/app`); no `..`.
pub fn valid_folder(folder: &str) -> Result<String, String> {
    let f = folder.trim();
    if !f.starts_with('/') || f.chars().count() > 300 || f.chars().any(char::is_control) {
        return Err(format!(
            "{f:?}: a folder on the server is a full path starting with / (like /var/www/site)"
        ));
    }
    normalize(f).ok_or_else(|| format!("{f:?}: a folder cannot use .."))
}

/// A destination a port may be forwarded to: `host:port`.
pub fn valid_forward(to: &str) -> Result<String, String> {
    let t = to.trim();
    let parsed = t.rsplit_once(':').and_then(|(host, port)| {
        let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
        let host = valid_host(host).ok()?;
        Some(format!("{host}:{port}"))
    });
    parsed
        .ok_or_else(|| format!("{t:?}: a forwarded port is written host:port, like localhost:5432"))
}

/// A pinned fingerprint: `SHA256:` and 43 characters of base64.
pub fn valid_fingerprint(fp: &str) -> Result<String, String> {
    let f = fp.trim();
    let body = f.strip_prefix("SHA256:").unwrap_or("");
    let ok = (40..=64).contains(&body.len())
        && body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
    if ok {
        Ok(format!("SHA256:{}", body.trim_end_matches('=')))
    } else {
        Err(format!(
            "{f:?} is not a host key fingerprint (it looks like SHA256:… as ssh-keygen -lf shows)"
        ))
    }
}

/// Check a server as the owner submitted it, and make the stored form. `id` and the times are
/// the caller's.
pub fn clean(
    input: &ServerInput,
    id: &str,
    earlier: Option<&Server>,
    now: u64,
) -> Result<Server, String> {
    let name = one_line("the server's name", &input.name, 60)?;
    let host = valid_host(&input.host)?;
    let port = if input.port == 0 {
        SSH_PORT
    } else {
        input.port
    };
    let user = valid_user(&input.user)?;
    let environment = input.environment;
    let mut folders: Vec<String> = Vec::new();
    for f in input.folders.iter().filter(|f| !f.trim().is_empty()) {
        let f = valid_folder(f)?;
        if !folders.contains(&f) {
            folders.push(f);
        }
    }
    if folders.len() > MAX_FOLDERS {
        return Err(format!("at most {MAX_FOLDERS} folders per server"));
    }
    let mut forwards: Vec<String> = Vec::new();
    for f in input.forwards.iter().filter(|f| !f.trim().is_empty()) {
        let f = valid_forward(f)?;
        if !forwards.contains(&f) {
            forwards.push(f);
        }
    }
    if forwards.len() > MAX_FORWARDS {
        return Err(format!("at most {MAX_FORWARDS} forwarded ports per server"));
    }
    let mut roles: Vec<String> = Vec::new();
    for r in &input.roles {
        let r = r.trim();
        if r.is_empty() || r.len() > 64 || r.chars().any(char::is_control) {
            return Err("a role in the list is not valid".into());
        }
        if !roles.iter().any(|x| x == r) {
            roles.push(r.to_owned());
        }
    }
    if roles.len() > MAX_ROLES {
        return Err(format!("at most {MAX_ROLES} roles per server"));
    }
    let mut classes: Vec<CommandClass> = CommandClass::ALL
        .into_iter()
        .filter(|c| input.classes.contains(c))
        .collect();
    classes.dedup();
    // Production: every command asks, always.
    let approval = if environment == Environment::Production {
        ServerApproval::Every
    } else {
        input.approval
    };
    let host_key = match &input.host_key {
        Some(k) => {
            let fingerprint = valid_fingerprint(&k.fingerprint)?;
            let algorithm = one_line("the host key's type", &k.algorithm, 60)?;
            // Pinned again only when it changed.
            let pinned_at = earlier
                .and_then(|s| s.host_key.as_ref())
                .filter(|old| old.fingerprint == fingerprint)
                .map_or(now, |old| old.pinned_at);
            Some(HostKey {
                algorithm,
                fingerprint,
                pinned_at,
            })
        }
        None => None,
    };
    Ok(Server {
        id: id.to_owned(),
        name,
        host,
        port,
        user,
        environment,
        sign_in: input.sign_in,
        host_key,
        roles,
        classes,
        approval,
        folders,
        forwards,
        created_at: earlier.map_or(now, |s| s.created_at),
        updated_at: now,
    })
}

// ---- Remote paths -----------------------------------------------------------------------------

/// A POSIX path tidied: `//` and `.` removed, no trailing `/` (except `/`). `None` when it uses
/// `..`.
fn normalize(path: &str) -> Option<String> {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => return None,
            _ => parts.push(p),
        }
    }
    let joined = parts.join("/");
    Some(if absolute {
        format!("/{joined}")
    } else {
        joined
    })
}

/// `path` is `folder` or inside it.
fn inside(path: &str, folder: &str) -> bool {
    folder == "/" || path == folder || path.starts_with(&format!("{folder}/"))
}

/// Where a command runs on `server`: the folder a worker asked for (relative to the first
/// allowed folder, or a full path inside one), or the first allowed folder. `Ok(None)`: the home
/// folder of the user Plenipo signs in as (the server allows no folders).
pub fn resolve_cwd(server: &Server, asked: Option<&str>) -> Result<Option<String>, String> {
    let asked = asked.map(str::trim).filter(|a| !a.is_empty() && *a != ".");
    let Some(first) = server.folders.first() else {
        return match asked {
            None => Ok(None),
            Some(a) if a.starts_with('/') || a.starts_with('~') => Err(format!(
                "{} has no folders set up, so commands run in the home folder of {} (a folder \
                 inside it can be given as a relative path)",
                server.name, server.user
            )),
            Some(a) => normalize(a)
                .map(Some)
                .ok_or_else(|| format!("{a:?} cannot use ..")),
        };
    };
    let Some(asked) = asked else {
        return Ok(Some(first.clone()));
    };
    let full = if asked.starts_with('/') {
        asked.to_owned()
    } else {
        format!("{first}/{asked}")
    };
    let full = normalize(&full).ok_or_else(|| format!("{asked:?} cannot use .."))?;
    if server.folders.iter().any(|f| inside(&full, f)) {
        Ok(Some(full))
    } else {
        Err(format!(
            "{full} is not in the folders you allowed on {} ({})",
            server.name,
            server.folders.join(", ")
        ))
    }
}

/// The value of an option like `--target=/tmp` (else the argument itself).
fn value_of(arg: &str) -> &str {
    let arg = arg.trim_matches(['"', '\'']);
    match arg.split_once('=') {
        Some((k, v)) if k.starts_with('-') => v,
        _ => arg,
    }
}

/// An argument that names a file or folder: a full path, a path in the home folder, or a
/// relative path with a folder in it. Addresses (`https://…`, `user@host:…`) are not.
fn path_like(arg: &str) -> Option<&str> {
    let v = value_of(arg);
    if v.is_empty() || v.contains("://") || (v.starts_with('-') && !v.contains('/')) {
        return None;
    }
    (v.starts_with('/') || v.starts_with('~') || v.contains('/') || v.starts_with('.')).then_some(v)
}

/// The full path an argument names on the server, run from `cwd` (`None`: the home folder).
/// `None` when it cannot be known (it climbs above where it starts, or starts in a home folder).
fn full_path(arg: &str, cwd: Option<&str>) -> Option<String> {
    if arg.starts_with('~') {
        return None;
    }
    if arg.starts_with('/') {
        return normalize(arg);
    }
    match cwd {
        Some(c) => normalize(&format!("{c}/{arg}")),
        None => normalize(arg).map(|rel| format!("~/{rel}")),
    }
}

/// Why a changing command's path argument is outside the server's folders, if it is.
fn outside_folders(server: &Server, arg: &str, cwd: Option<&str>) -> Option<String> {
    let path = path_like(arg)?;
    let full = full_path(path, cwd);
    let ok = match (&full, server.folders.is_empty()) {
        // A relative path inside the home folder, when no folders are set up.
        (Some(f), true) => f.starts_with("~/"),
        (Some(f), false) => server.folders.iter().any(|x| inside(f, x)),
        (None, _) => false,
    };
    if ok {
        return None;
    }
    Some(if server.folders.is_empty() {
        format!(
            "{path} is outside the home folder of {}, and {} has no other folders set up",
            server.user, server.name
        )
    } else {
        format!(
            "{path} is outside the folders you allowed on {} ({})",
            server.name,
            server.folders.join(", ")
        )
    })
}

// ---- Kinds of commands ------------------------------------------------------------------------

/// What kind of command a command line is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    /// Its kinds: run as administrator (if it is) and what the command itself does.
    pub classes: Vec<CommandClass>,
    /// Never allowed on a server, and why.
    pub never: Option<&'static str>,
    /// The command after `sudo` and wrappers like `nice` (what it really runs).
    pub inner: CommandLine,
}

impl Classified {
    /// It can change or delete things (its paths must be in the server's folders).
    pub fn changes(&self) -> bool {
        self.classes.iter().any(|c| {
            matches!(
                c,
                CommandClass::Change | CommandClass::Destroy | CommandClass::Other
            )
        })
    }

    /// Only looks.
    pub fn looks_only(&self) -> bool {
        self.classes.iter().all(|c| *c == CommandClass::Look)
    }

    pub fn is(&self, class: CommandClass) -> bool {
        self.classes.contains(&class)
    }
}

const HOP: &str = "Plenipo never lets a worker reach other computers from a server (ssh, scp, \
                   remote copies, and tunnels), scan or watch the network, or hunt for \
                   passwords: that is how access to one server spreads to others";

/// Programs that reach other computers, scan or watch the network, or crack passwords.
const NEVER: &[&str] = &[
    "ssh",
    "scp",
    "sftp",
    "slogin",
    "rsh",
    "rlogin",
    "rcp",
    "rexec",
    "telnet",
    "mosh",
    "sshpass",
    "ssh-copy-id",
    "ssh-keyscan",
    "ssh-add",
    "ssh-agent",
    "ssh-keygen",
    "nc",
    "ncat",
    "netcat",
    "socat",
    "ftp",
    "lftp",
    "tftp",
    "smbclient",
    "rdesktop",
    "xfreerdp",
    "vncviewer",
    "autossh",
    "sshuttle",
    "proxychains",
    "chisel",
    "ngrok",
    "nmap",
    "masscan",
    "zmap",
    "arp-scan",
    "netdiscover",
    "nbtscan",
    "fping",
    "hping3",
    "tcpdump",
    "tshark",
    "wireshark",
    "ettercap",
    "bettercap",
    "responder",
    "arpspoof",
    "hydra",
    "medusa",
    "ncrack",
    "john",
    "hashcat",
    "crackmapexec",
    "netexec",
    "mimikatz",
    "impacket",
    "secretsdump.py",
    "psexec.py",
    "linpeas.sh",
    "lazagne",
];

/// Programs that only look, whatever their arguments.
const LOOK: &[&str] = &[
    "ls",
    "ll",
    "dir",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "grep",
    "egrep",
    "fgrep",
    "zgrep",
    "rg",
    "ag",
    "stat",
    "file",
    "wc",
    "du",
    "df",
    "free",
    "uptime",
    "uname",
    "whoami",
    "id",
    "groups",
    "ps",
    "pstree",
    "pgrep",
    "pidof",
    "w",
    "who",
    "last",
    "lastlog",
    "printenv",
    "which",
    "whereis",
    "type",
    "echo",
    "printf",
    "pwd",
    "lsblk",
    "lscpu",
    "lsmem",
    "lsof",
    "lspci",
    "lsusb",
    "ss",
    "netstat",
    "ping",
    "ping6",
    "traceroute",
    "tracepath",
    "mtr",
    "dig",
    "nslookup",
    "host",
    "getent",
    "locale",
    "vmstat",
    "iostat",
    "mpstat",
    "sar",
    "nproc",
    "arch",
    "tree",
    "md5sum",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "cksum",
    "diff",
    "cmp",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "uniq",
    "cut",
    "tr",
    "column",
    "jq",
    "yq",
    "xxd",
    "hexdump",
    "od",
    "strings",
    "zcat",
    "bzcat",
    "xzcat",
    "tac",
    "nl",
    "fold",
    "locate",
    "test",
    "true",
    "false",
    "sleep",
    "ulimit",
    "getconf",
    "lsattr",
    "findmnt",
    "blkid",
    "numfmt",
    "expr",
    "seq",
    "cal",
    "tty",
];

fn is_version_only(args: &[String]) -> bool {
    matches!(
        args,
        [a] if matches!(a.as_str(), "--version" | "-v" | "-V" | "version" | "--help" | "-h")
    )
}

/// The first argument that is not an option.
fn sub(args: &[String]) -> Option<&str> {
    args.iter()
        .map(String::as_str)
        .find(|a| !a.starts_with('-'))
}

/// The first two arguments that are not options.
fn sub2(args: &[String]) -> (Option<&str>, Option<&str>) {
    let mut words = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'));
    (words.next(), words.next())
}

fn has(args: &[String], words: &[&str]) -> bool {
    args.iter().any(|a| words.contains(&a.as_str()))
}

/// A short option set such as `-rf` includes `c`.
fn short_flag(args: &[String], c: char) -> bool {
    args.iter()
        .any(|a| a.starts_with('-') && !a.starts_with("--") && a[1..].contains(c))
}

/// What one program (already without `sudo` and wrappers) does.
fn class_of(program: &str, args: &[String]) -> Result<CommandClass, &'static str> {
    use CommandClass::*;
    if NEVER.contains(&program) {
        return Err(HOP);
    }
    // rsync or a remote copy to another computer (`host:path`).
    if program == "rsync" {
        let remote = args.iter().any(|a| {
            !a.starts_with('-')
                && a.split_once(':')
                    .is_some_and(|(h, _)| !h.is_empty() && !h.contains('/'))
        }) || has(args, &["-e", "--rsh"])
            || args
                .iter()
                .any(|a| a.starts_with("--rsh=") || a.starts_with("-e"));
        if remote {
            return Err(HOP);
        }
        return Ok(if args.iter().any(|a| a.starts_with("--delete")) {
            Destroy
        } else {
            Change
        });
    }
    if is_version_only(args) {
        return Ok(Look);
    }
    if LOOK.contains(&program) {
        return Ok(Look);
    }
    let s = sub(args);
    let (_, s2) = sub2(args);
    Ok(match program {
        "date" => {
            if has(args, &["-s", "--set"]) || args.iter().any(|a| !a.starts_with(['-', '+'])) {
                Other
            } else {
                Look
            }
        }
        "hostname" => {
            if args.iter().all(|a| a.starts_with('-'))
                && !has(args, &["-F", "--file", "-b", "--boot"])
            {
                Look
            } else {
                Other
            }
        }
        "hostnamectl" | "timedatectl" => match s {
            None | Some("status" | "show" | "list-timezones") => Look,
            Some(_) => Other,
        },
        "find" => {
            if has(args, &["-delete"]) {
                Destroy
            } else if has(
                args,
                &[
                    "-exec", "-execdir", "-ok", "-okdir", "-fprint", "-fprint0", "-fprintf", "-fls",
                ],
            ) {
                Other
            } else {
                Look
            }
        }
        "sed" => {
            if args.iter().any(|a| {
                a == "-i"
                    || a.starts_with("-i")
                    || a == "--in-place"
                    || a.starts_with("--in-place=")
            }) {
                Change
            } else {
                Look
            }
        }
        "sort" => {
            if short_flag(args, 'o') || args.iter().any(|a| a.starts_with("--output")) {
                Change
            } else {
                Look
            }
        }
        "awk" | "gawk" | "mawk" => {
            if args.iter().any(|a| {
                a.contains("system") || a.contains('>') || a.contains('|') || a.contains("getline")
            }) {
                Other
            } else {
                Look
            }
        }
        "top" | "htop" => {
            if short_flag(args, 'b') {
                Look
            } else {
                Other
            }
        }
        "journalctl" => {
            if args.iter().any(|a| a.starts_with("--vacuum")) {
                Destroy
            } else if has(args, &["--rotate", "--flush", "--relinquish-var", "--sync"]) {
                Change
            } else {
                Look
            }
        }
        "dmesg" => {
            if short_flag(args, 'c')
                || short_flag(args, 'C')
                || has(args, &["--clear", "--read-clear"])
            {
                Destroy
            } else {
                Look
            }
        }
        "ip" => match s {
            None => Look,
            Some(_)
                if has(
                    args,
                    &[
                        "add", "del", "delete", "change", "replace", "set", "flush", "append",
                    ],
                ) =>
            {
                Other
            }
            Some(_) => Look,
        },
        "ifconfig" | "sysctl" => {
            if args.is_empty() || has(args, &["-a"]) {
                Look
            } else {
                Other
            }
        }
        "systemctl" => match s {
            None
            | Some(
                "status" | "is-active" | "is-enabled" | "is-failed" | "list-units"
                | "list-unit-files" | "list-timers" | "list-sockets" | "list-jobs" | "show" | "cat"
                | "list-dependencies" | "get-default",
            ) => Look,
            Some(
                "start"
                | "stop"
                | "restart"
                | "reload"
                | "try-restart"
                | "reload-or-restart"
                | "try-reload-or-restart"
                | "enable"
                | "disable"
                | "daemon-reload"
                | "reset-failed"
                | "kill"
                | "mask"
                | "unmask",
            ) => Services,
            Some(
                "reboot" | "poweroff" | "halt" | "suspend" | "hibernate" | "kexec" | "emergency"
                | "rescue" | "isolate" | "default" | "soft-reboot",
            ) => Destroy,
            Some(
                "edit" | "set-property" | "link" | "preset" | "preset-all" | "revert"
                | "set-default",
            ) => Change,
            Some(_) => Other,
        },
        "service" => {
            if has(args, &["--status-all"]) || s2 == Some("status") {
                Look
            } else if matches!(
                s2,
                Some("start" | "stop" | "restart" | "reload" | "force-reload" | "try-restart")
            ) {
                Services
            } else {
                Other
            }
        }
        "docker" | "podman" => docker(args),
        "docker-compose" | "podman-compose" => compose(args),
        "kubectl" | "oc" => match s {
            Some(
                "get" | "describe" | "logs" | "top" | "version" | "explain" | "api-resources"
                | "api-versions" | "cluster-info" | "auth" | "diff",
            ) => Look,
            Some("config") => {
                if matches!(
                    s2,
                    Some("view" | "get-contexts" | "current-context" | "get-clusters")
                ) {
                    Look
                } else {
                    Other
                }
            }
            Some("rollout") => match s2 {
                Some("status" | "history") => Look,
                Some("restart" | "pause" | "resume") => Services,
                Some("undo") => Change,
                _ => Other,
            },
            Some("scale" | "cordon" | "uncordon") => Services,
            Some(
                "apply" | "create" | "patch" | "set" | "replace" | "edit" | "label" | "annotate"
                | "expose" | "autoscale",
            ) => Change,
            Some("delete" | "drain" | "taint") => Destroy,
            _ => Other,
        },
        "git" => git(args),
        "apt" | "apt-get" | "aptitude" | "yum" | "dnf" | "apk" | "zypper" | "snap" | "brew" => {
            match s {
                None => Other,
                Some(
                    "list" | "search" | "show" | "policy" | "info" | "madison" | "depends"
                    | "rdepends" | "check-update" | "history" | "repolist" | "list-installed"
                    | "provides" | "whatprovides",
                ) => Look,
                Some(
                    "update" | "upgrade" | "install" | "reinstall" | "dist-upgrade"
                    | "full-upgrade" | "refresh" | "makecache" | "add" | "fix" | "download"
                    | "source" | "build-dep",
                ) => Change,
                Some(
                    "remove" | "purge" | "autoremove" | "erase" | "del" | "clean" | "autoclean"
                    | "uninstall",
                ) => Destroy,
                Some(_) => Other,
            }
        }
        "dpkg" | "rpm" => {
            if short_flag(args, 'l')
                || short_flag(args, 'L')
                || short_flag(args, 's')
                || short_flag(args, 'q')
                || has(args, &["--list", "--status", "--listfiles", "--query"])
            {
                Look
            } else if short_flag(args, 'r')
                || short_flag(args, 'P')
                || short_flag(args, 'e')
                || has(args, &["--remove", "--purge", "--erase"])
            {
                Destroy
            } else if short_flag(args, 'i')
                || short_flag(args, 'U')
                || has(args, &["--install", "--upgrade", "--configure"])
            {
                Change
            } else {
                Other
            }
        }
        "npm" | "pnpm" | "yarn" => match s {
            Some("ls" | "list" | "view" | "info" | "outdated" | "why" | "explain" | "config")
                if !has(args, &["set", "delete"]) =>
            {
                Look
            }
            Some("audit") if !has(args, &["fix"]) => Look,
            Some(
                "install" | "i" | "ci" | "add" | "update" | "up" | "upgrade" | "run" | "run-script"
                | "start" | "build" | "test" | "rebuild" | "uninstall" | "remove" | "rm" | "un"
                | "prune" | "dedupe" | "restart" | "stop" | "audit" | "publish",
            ) => Change,
            None => Change,
            Some(_) => Other,
        },
        "pip" | "pip3" | "pipx" => match s {
            Some("list" | "show" | "freeze" | "check" | "index") => Look,
            Some("install" | "download" | "wheel" | "uninstall") => Change,
            _ => Other,
        },
        "composer" => match s {
            Some(
                "show" | "info" | "outdated" | "validate" | "diagnose" | "licenses" | "depends"
                | "why" | "prohibits" | "why-not" | "status",
            ) => Look,
            Some(
                "install" | "update" | "upgrade" | "require" | "remove" | "dump-autoload"
                | "dumpautoload" | "run-script" | "run" | "clear-cache",
            ) => Change,
            _ => Other,
        },
        "pm2" => match s {
            Some(
                "list" | "ls" | "l" | "status" | "show" | "describe" | "info" | "logs" | "jlist"
                | "prettylist" | "env" | "ping" | "report",
            ) => Look,
            Some(
                "start" | "stop" | "restart" | "reload" | "startOrRestart" | "startOrReload"
                | "gracefulReload" | "save" | "resurrect" | "scale" | "reset" | "dump",
            ) => Services,
            Some("delete" | "del" | "kill" | "flush" | "uninstall" | "unstartup") => Destroy,
            _ => Other,
        },
        "supervisorctl" => match s {
            Some("status" | "avail" | "pid" | "tail" | "version" | "help") => Look,
            Some("start" | "stop" | "restart" | "reread" | "update" | "reload" | "signal") => {
                Services
            }
            Some("remove" | "clear" | "shutdown") => Destroy,
            _ => Other,
        },
        "nginx" => {
            if has(args, &["-t", "-T", "-v", "-V"]) {
                Look
            } else if has(args, &["-s"]) || args.is_empty() {
                Services
            } else {
                Other
            }
        }
        "apachectl" | "apache2ctl" | "httpd" => match s {
            _ if has(args, &["-t", "-S", "-M", "-v", "-V", "-l", "-L"]) => Look,
            Some("configtest" | "status" | "fullstatus") => Look,
            Some("graceful" | "restart" | "start" | "stop" | "graceful-stop") => Services,
            _ => Other,
        },
        "wp" => wp(args),
        "crontab" => {
            if short_flag(args, 'l') {
                Look
            } else if short_flag(args, 'r') {
                Destroy
            } else {
                Change
            }
        }
        "certbot" => match s {
            Some("certificates" | "plugins") => Look,
            Some("renew" | "certonly" | "install" | "run" | "reconfigure" | "enhance") => Change,
            Some("revoke" | "delete" | "unregister") => Destroy,
            _ => Other,
        },
        "ufw" | "iptables" | "ip6tables" | "nft" | "firewall-cmd" => {
            let lists = (program == "ufw" && s == Some("status"))
                || (program.starts_with("ip") && short_flag(args, 'L'))
                || (program == "nft" && s == Some("list"))
                || has(
                    args,
                    &["--list-all", "--state", "--list-services", "--list-ports"],
                );
            if lists {
                Look
            } else {
                Other
            }
        }
        "kill" | "pkill" | "killall" => Services,
        "mkdir" | "touch" | "cp" | "mv" | "ln" | "chmod" | "chown" | "chgrp" | "tee"
        | "install" | "patch" | "unzip" | "gunzip" | "gzip" | "bzip2" | "bunzip2" | "xz"
        | "unxz" | "zip" | "tar" | "make" | "cmake" | "wget" | "rename" | "setfacl"
        | "sudoedit" => {
            if program == "wget" {
                Other
            } else {
                Change
            }
        }
        "rm" | "rmdir" | "unlink" | "shred" | "dd" | "wipefs" | "fdisk" | "sfdisk" | "parted"
        | "gdisk" | "sgdisk" | "mkswap" | "swapoff" | "truncate" | "fallocate" | "reboot"
        | "shutdown" | "poweroff" | "halt" | "init" | "telinit" | "userdel" | "groupdel"
        | "deluser" | "delgroup" | "kexec" => Destroy,
        p if p.starts_with("mkfs") => Destroy,
        // Database clients, curl, programs that run code (shells, Python, PHP), and anything
        // unknown.
        _ => Other,
    })
}

fn docker(args: &[String]) -> CommandClass {
    use CommandClass::*;
    let (s1, s2) = sub2(args);
    match s1 {
        Some(
            "ps" | "images" | "logs" | "inspect" | "stats" | "top" | "version" | "info" | "port"
            | "diff" | "history" | "events" | "search",
        ) => Look,
        Some("start" | "stop" | "restart" | "pause" | "unpause" | "kill") => Services,
        Some(
            "pull" | "build" | "run" | "create" | "tag" | "cp" | "commit" | "load" | "update"
            | "save" | "import",
        ) => Change,
        Some("rm" | "rmi" | "prune") => Destroy,
        Some("compose") => compose(
            &args[args
                .iter()
                .position(|a| a == "compose")
                .map_or(args.len(), |i| i + 1)..],
        ),
        Some(
            "container" | "image" | "volume" | "network" | "system" | "context" | "builder"
            | "buildx",
        ) => match s2 {
            Some(
                "ls" | "list" | "inspect" | "logs" | "top" | "stats" | "port" | "df" | "info"
                | "history" | "diff",
            ) => Look,
            Some("start" | "stop" | "restart" | "kill" | "pause" | "unpause") => Services,
            Some("rm" | "prune" | "remove") => Destroy,
            Some(
                "create" | "connect" | "disconnect" | "pull" | "build" | "tag" | "use" | "load"
                | "save" | "update",
            ) => Change,
            _ => Other,
        },
        _ => Other,
    }
}

fn compose(args: &[String]) -> CommandClass {
    use CommandClass::*;
    match sub(args) {
        Some(
            "ps" | "logs" | "config" | "ls" | "top" | "images" | "version" | "events" | "port",
        ) => Look,
        Some("start" | "stop" | "restart" | "pause" | "unpause" | "kill") => Services,
        Some("down") => {
            if has(args, &["-v", "--volumes", "--rmi"]) {
                Destroy
            } else {
                Services
            }
        }
        Some("up" | "pull" | "build" | "create" | "cp") => Change,
        Some("rm") => Destroy,
        _ => Other,
    }
}

fn git(args: &[String]) -> CommandClass {
    use CommandClass::*;
    // Options that make git run other programs.
    if args.iter().any(|a| {
        a.contains("sshCommand")
            || a.contains("core.pager")
            || a.starts_with("--upload-pack")
            || a.starts_with("--exec")
    }) {
        return Other;
    }
    let (s1, s2) = sub2(args);
    let rest = || {
        args.iter()
            .skip_while(|a| Some(a.as_str()) != s1)
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
    };
    match s1 {
        None => Look,
        Some(
            "status" | "log" | "diff" | "show" | "rev-parse" | "describe" | "ls-files" | "ls-tree"
            | "blame" | "shortlog" | "whatchanged" | "grep" | "cat-file" | "count-objects"
            | "rev-list" | "name-rev" | "for-each-ref" | "show-ref" | "merge-base" | "cherry"
            | "ls-remote" | "help",
        ) => Look,
        Some("branch") => {
            let r = rest();
            if short_flag(&r, 'D') || short_flag(&r, 'd') || has(&r, &["--delete"]) {
                Destroy
            } else if r.iter().all(|a| a.starts_with('-')) {
                Look
            } else {
                Change
            }
        }
        Some("tag") => {
            let r = rest();
            if short_flag(&r, 'd') || has(&r, &["--delete"]) {
                Destroy
            } else if r
                .iter()
                .all(|a| matches!(a.as_str(), "-l" | "--list" | "-n"))
            {
                Look
            } else {
                Change
            }
        }
        Some("remote") => match s2 {
            None | Some("show" | "get-url") => Look,
            Some("remove" | "rm") => Destroy,
            Some(_) => Change,
        },
        Some("config") => {
            if has(
                args,
                &[
                    "--get",
                    "--get-all",
                    "--list",
                    "-l",
                    "--get-regexp",
                    "--show-origin",
                ],
            ) {
                Look
            } else {
                Change
            }
        }
        Some("stash") => match s2 {
            Some("list" | "show") => Look,
            Some("drop" | "clear") => Destroy,
            _ => Change,
        },
        Some("reflog") => match s2 {
            Some("expire" | "delete") => Destroy,
            _ => Look,
        },
        Some("reset") => {
            if has(args, &["--hard", "--merge", "--keep"]) {
                Destroy
            } else {
                Change
            }
        }
        Some("clean") => {
            if short_flag(args, 'n') || has(args, &["--dry-run"]) {
                Look
            } else {
                Destroy
            }
        }
        Some("push") => {
            if short_flag(&rest(), 'f')
                || args.iter().any(|a| {
                    a.starts_with("--force")
                        || a.starts_with("--delete")
                        || a.starts_with("--mirror")
                        || a.starts_with('+')
                })
            {
                Destroy
            } else {
                Change
            }
        }
        Some("gc" | "prune" | "filter-branch" | "filter-repo") => Destroy,
        Some(
            "pull" | "fetch" | "checkout" | "switch" | "restore" | "merge" | "rebase" | "add"
            | "commit" | "clone" | "init" | "submodule" | "cherry-pick" | "revert" | "mv" | "rm"
            | "apply" | "am" | "worktree" | "sparse-checkout" | "lfs" | "notes",
        ) => Change,
        Some(_) => Other,
    }
}

/// WP-CLI (WordPress and WooCommerce).
fn wp(args: &[String]) -> CommandClass {
    use CommandClass::*;
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect();
    if args.iter().any(|a| a == "--info") && words.is_empty() {
        return Look;
    }
    let (group, action) = (words.first().copied(), words.get(1).copied());
    let action = if group == Some("wc") {
        words
            .iter()
            .rev()
            .find(|w| {
                matches!(
                    **w,
                    "list" | "get" | "create" | "update" | "delete" | "batch" | "run"
                )
            })
            .copied()
    } else {
        action
    };
    match (group, action) {
        (Some("eval" | "eval-file" | "shell" | "package"), _)
        | (Some("db"), Some("query" | "cli")) => Other,
        (Some("search-replace"), _) => {
            if has(args, &["--dry-run"]) {
                Look
            } else {
                Change
            }
        }
        (Some("db"), Some("reset" | "drop" | "clean" | "import")) => Destroy,
        (Some("db"), Some("check" | "size" | "tables" | "prefix" | "columns")) => Look,
        (Some("db"), Some("export" | "optimize" | "repair")) => Change,
        (Some("site"), Some("empty" | "delete" | "archive" | "deactivate" | "spam")) => Destroy,
        (Some("user"), Some("list" | "get" | "meta" | "list-caps")) => Look,
        (Some("user"), Some("delete")) => Destroy,
        (Some("user"), Some(_)) => Other,
        (_, Some("delete" | "uninstall" | "destroy" | "trash" | "empty" | "flush-group")) => {
            Destroy
        }
        (Some("cache"), Some("flush")) | (Some("rewrite"), Some("flush")) => Change,
        (
            _,
            Some(
                "list" | "get" | "status" | "is-active" | "is-installed" | "search" | "version"
                | "check-update" | "verify-checksums" | "path" | "type" | "info" | "count"
                | "exists" | "pluck" | "has" | "is-enabled",
            ),
        ) => Look,
        (
            _,
            Some(
                "install" | "activate" | "deactivate" | "update" | "toggle" | "auto-updates"
                | "add" | "patch" | "set" | "create" | "download" | "update-db" | "run"
                | "regenerate" | "generate" | "import" | "enable" | "disable" | "flush" | "batch"
                | "schedule" | "unschedule" | "reset",
            ),
        ) => Change,
        (Some("maintenance-mode"), _) => Change,
        _ => Other,
    }
}

/// A program's bare name as the server would find it: `/usr/bin/Systemctl` → `systemctl`.
fn bare(program: &str) -> String {
    program
        .trim()
        .rsplit('/')
        .next()
        .unwrap_or(program)
        .to_lowercase()
}

/// Options of `sudo` that take a value.
const SUDO_VALUE_OPTIONS: &[&str] = &[
    "-u", "-g", "-p", "-C", "-D", "-h", "-r", "-t", "-U", "-T", "-R",
];

/// Put a command line in its kinds. `sudo`, `doas`, and wrappers like `nice` and `timeout` are
/// looked through: what they run decides the kind (and `sudo` adds "Run as administrator").
pub fn classify(cmd: &CommandLine) -> Classified {
    let mut raw = cmd.program.trim().to_owned();
    let mut program = bare(&raw);
    let mut args: Vec<String> = cmd.args.clone();
    let mut classes = Vec::new();
    for _ in 0..8 {
        match program.as_str() {
            "sudo" | "doas" | "pkexec" | "runuser" => {
                if !classes.contains(&CommandClass::Admin) {
                    classes.push(CommandClass::Admin);
                }
                let mut i = 0;
                let mut shell = false;
                while i < args.len() && args[i].starts_with('-') {
                    let a = args[i].as_str();
                    if matches!(a, "-s" | "-i" | "--shell" | "--login" | "-l") {
                        shell = true;
                    }
                    if a == "--" {
                        i += 1;
                        break;
                    }
                    i += if SUDO_VALUE_OPTIONS.contains(&a) {
                        2
                    } else {
                        1
                    };
                }
                if shell || i >= args.len() {
                    classes.push(CommandClass::Other);
                    return Classified {
                        classes,
                        never: None,
                        inner: CommandLine::new(program, &[]),
                    };
                }
                raw = args[i].clone();
                program = bare(&raw);
                args = args[i + 1..].to_vec();
            }
            "env" | "nice" | "ionice" | "time" | "stdbuf" | "chrt" | "taskset" | "timeout" => {
                // Skip options, `NAME=value` settings, and the values the options take.
                let mut i = 0;
                while i < args.len() {
                    let a = args[i].as_str();
                    let takes_value = matches!(
                        (program.as_str(), a),
                        ("nice", "-n")
                            | ("ionice", "-c" | "-n" | "-p")
                            | ("timeout", "-s" | "-k")
                            | ("chrt", "-p")
                            | ("env", "-u" | "-C" | "-S")
                    );
                    if takes_value {
                        i += 2;
                    } else if a.starts_with('-') || (program == "env" && a.contains('=')) {
                        i += 1;
                    } else {
                        break;
                    }
                }
                // `timeout` and `taskset`/`chrt` take one more word (the time, the mask).
                if matches!(program.as_str(), "timeout" | "taskset" | "chrt") && i < args.len() {
                    i += 1;
                }
                if i >= args.len() {
                    classes.push(if program == "env" {
                        CommandClass::Look
                    } else {
                        CommandClass::Other
                    });
                    return Classified {
                        classes,
                        never: None,
                        inner: CommandLine::new(program, &[]),
                    };
                }
                raw = args[i].clone();
                program = bare(&raw);
                args = args[i + 1..].to_vec();
            }
            "su" => {
                if !classes.contains(&CommandClass::Admin) {
                    classes.push(CommandClass::Admin);
                }
                classes.push(CommandClass::Other);
                return Classified {
                    classes,
                    never: None,
                    inner: CommandLine { program, args },
                };
            }
            _ => break,
        }
    }
    let inner = CommandLine {
        program: program.clone(),
        args: args.clone(),
    };
    // A program named by a path outside the system's folders (`./deploy.sh`, `/opt/app/run`)
    // runs code Plenipo cannot see.
    let system = ["/usr/", "/bin/", "/sbin/"]
        .iter()
        .any(|d| raw.starts_with(d));
    let class = if raw.contains('/') && !system {
        if NEVER.contains(&program.as_str()) {
            Err(HOP)
        } else {
            Ok(CommandClass::Other)
        }
    } else {
        class_of(&program, &args)
    };
    match class {
        Ok(c) => {
            // A database command that deletes or wipes data is destructive whatever the client.
            let wipes = matches!(
                sensitive::command(&inner, std::path::Path::new("/")),
                Some((SensitiveKind::Database | SensitiveKind::CloudDelete, _))
            );
            let c = if wipes { CommandClass::Destroy } else { c };
            if !classes.contains(&c) {
                classes.push(c);
            }
            Classified {
                classes,
                never: None,
                inner,
            }
        }
        Err(never) => {
            classes.push(CommandClass::Other);
            Classified {
                classes,
                never: Some(never),
                inner,
            }
        }
    }
}

// ---- Guard's decision -------------------------------------------------------------------------

/// What a worker wants to do with a server.
#[derive(Debug, Clone, Copy)]
pub enum ServerUse<'a> {
    /// Connect to it (or list it).
    Connect,
    /// Run a command, in the folder given (`None`: the home folder).
    Run {
        classified: &'a Classified,
        cwd: Option<&'a str>,
    },
    /// Forward a port to `to` (`host:port`) through it.
    Forward { to: &'a str },
}

/// A worker's use of a server, for Guard.
#[derive(Debug, Clone, Copy)]
pub struct ServerCheck<'a> {
    pub server: &'a Server,
    pub role_id: &'a str,
    pub role_name: &'a str,
    pub what: ServerUse<'a>,
}

/// Guard's answer about the server part of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerVerdict {
    /// Never, and why (a clause after "Blocked: ").
    Deny { layer: Layer, reason: String },
    /// Allowed as far as the server goes. `ask`: it still waits for the owner, and why;
    /// `sensitive`: a sensitive kind of action the command is.
    Go {
        ask: Option<String>,
        sensitive: Option<(SensitiveKind, &'static str)>,
    },
}

fn deny(layer: Layer, reason: String) -> ServerVerdict {
    ServerVerdict::Deny { layer, reason }
}

/// Check a worker's use of a server against the owner's settings for it: who may use it, its
/// pinned identity, the kinds of commands it allows, its folders, blocked files, forwarded
/// ports, and when the owner is asked (every command on a production server).
pub fn check(blocked_files: &[String], c: &ServerCheck<'_>) -> ServerVerdict {
    let s = c.server;
    let name = &s.name;
    if !s.roles.iter().any(|r| r == c.role_id) {
        return deny(
            Layer::Rule,
            format!(
                "the {} role may not use {name} (Settings → Servers lists the roles that may)",
                c.role_name
            ),
        );
    }
    if s.host_key.is_none() {
        return deny(
            Layer::Target,
            format!(
                "{name}'s identity (host key) has not been checked and pinned yet, in Settings → \
                 Servers"
            ),
        );
    }
    match c.what {
        ServerUse::Connect => ServerVerdict::Go {
            ask: None,
            sensitive: None,
        },
        ServerUse::Forward { to } => {
            if s.forwards.iter().any(|f| f == to) {
                ServerVerdict::Go {
                    ask: Some(format!(
                        "forwarding a port through {name} always waits for your approval"
                    )),
                    sensitive: None,
                }
            } else {
                deny(
                    Layer::Rule,
                    if s.forwards.is_empty() {
                        format!("port forwarding is off for {name} (it is off unless you list a port in its settings)")
                    } else {
                        format!(
                            "{to} is not one of the ports you allowed to be forwarded through \
                             {name} ({})",
                            s.forwards.join(", ")
                        )
                    },
                )
            }
        }
        ServerUse::Run { classified, cwd } => run(blocked_files, s, classified, cwd),
    }
}

fn run(blocked_files: &[String], s: &Server, k: &Classified, cwd: Option<&str>) -> ServerVerdict {
    let name = &s.name;
    if let Some(never) = k.never {
        return deny(Layer::Rule, never.to_owned());
    }
    // Blocked files: the owner's patterns apply on servers too.
    for arg in &k.inner.args {
        let v = value_of(arg);
        if v.is_empty() || v.starts_with('-') || v.contains("://") {
            continue;
        }
        let rel = v.trim_start_matches("~/").trim_start_matches('/');
        if let Some(rule) = blocked_by(blocked_files, rel) {
            return deny(
                Layer::Rule,
                format!("{v} is a blocked file (your rule \"{rule}\")"),
            );
        }
    }
    if k.changes() {
        for arg in &k.inner.args {
            if let Some(why) = outside_folders(s, arg, cwd) {
                return deny(Layer::Target, why);
            }
        }
    }
    for class in &k.classes {
        if !s.allows(*class) {
            let production =
                if *class == CommandClass::Destroy && s.environment == Environment::Production {
                    " (on a production server they are off unless you turn them on)"
                } else {
                    ""
                };
            return deny(
                Layer::Rule,
                format!(
                    "{name} does not allow \"{}\" commands{production}; you can allow them in \
                     Settings → Servers",
                    class.label()
                ),
            );
        }
    }
    // Running as administrator is the sensitive kind of a `sudo` command (so the owner's rule
    // for it applies on servers too); otherwise what the command itself does.
    let sensitive = if k.is(CommandClass::Admin) {
        Some((
            SensitiveKind::Privilege,
            "it runs a program as administrator",
        ))
    } else {
        sensitive::command(&k.inner, std::path::Path::new("/"))
            .filter(|(kind, _)| *kind != SensitiveKind::OutsideWorkspace)
    };
    let ask = if s.environment == Environment::Production {
        Some(format!(
            "{name} is a production server: every command there waits for your approval"
        ))
    } else if k.is(CommandClass::Destroy) {
        Some("deleting, wiping, or shutting down always waits for your approval".to_owned())
    } else if k.is(CommandClass::Admin) {
        Some("running as administrator always waits for your approval".to_owned())
    } else {
        match s.approval {
            ServerApproval::Every => Some(format!("you asked to approve every command on {name}")),
            ServerApproval::Changes if !k.looks_only() => Some(format!(
                "{name} asks you before anything that is not looking around"
            )),
            _ => None,
        }
    };
    ServerVerdict::Go { ask, sensitive }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CommandClass::*;

    fn cmd(line: &str) -> CommandLine {
        let mut w = line.split_whitespace();
        CommandLine {
            program: w.next().unwrap().to_owned(),
            args: w.map(str::to_owned).collect(),
        }
    }

    fn classes(line: &str) -> Vec<CommandClass> {
        classify(&cmd(line)).classes
    }

    #[test]
    fn everyday_commands_have_their_kind() {
        for (line, want) in [
            ("ls -la /var/www", vec![Look]),
            ("tail -n 100 /var/log/nginx/error.log", vec![Look]),
            ("df -h", vec![Look]),
            ("systemctl status nginx", vec![Look]),
            ("journalctl -u nginx -n 50", vec![Look]),
            ("docker ps -a", vec![Look]),
            ("docker compose logs web", vec![Look]),
            ("git status", vec![Look]),
            ("git log --oneline -5", vec![Look]),
            ("wp plugin list", vec![Look]),
            ("wp core version", vec![Look]),
            ("wp wc product list --user=1", vec![Look]),
            ("find /var/www -name *.php", vec![Look]),
            ("nginx -t", vec![Look]),
            ("uptime", vec![Look]),
            ("systemctl restart nginx", vec![Services]),
            ("service apache2 reload", vec![Services]),
            ("docker restart web", vec![Services]),
            ("pm2 restart all", vec![Services]),
            ("docker compose down", vec![Services]),
            ("git pull", vec![Change]),
            ("apt install -y nginx", vec![Change]),
            ("npm ci", vec![Change]),
            ("docker compose up -d", vec![Change]),
            ("wp plugin update --all", vec![Change]),
            ("mkdir -p releases/42", vec![Change]),
            ("sed -i s/a/b/ config.php", vec![Change]),
            ("rm -rf /var/www/old", vec![Destroy]),
            ("git reset --hard origin/main", vec![Destroy]),
            ("git push --force", vec![Destroy]),
            ("docker system prune -a", vec![Destroy]),
            ("docker compose down -v", vec![Destroy]),
            ("apt purge nginx", vec![Destroy]),
            ("wp db reset --yes", vec![Destroy]),
            ("wp plugin delete akismet", vec![Destroy]),
            ("reboot", vec![Destroy]),
            ("systemctl reboot", vec![Destroy]),
            ("find . -name *.log -delete", vec![Destroy]),
            ("mysql -e DROP TABLE users", vec![Destroy]),
            ("kubectl delete pod web-1", vec![Destroy]),
            ("sudo systemctl restart nginx", vec![Admin, Services]),
            ("sudo -u www-data wp cache flush", vec![Admin, Change]),
            ("sudo -i", vec![Admin, Other]),
            ("nice -n 10 tar czf backup.tgz site", vec![Change]),
            ("timeout 30 curl https://example.com", vec![Other]),
            ("python3 manage.py migrate", vec![Other]),
            ("bash -c ls", vec![Other]),
            ("./deploy.sh", vec![Other]),
            ("mysql -e SELECT 1", vec![Other]),
            ("frobnicate --now", vec![Other]),
            ("env", vec![Look]),
            ("/usr/bin/systemctl status nginx", vec![Look]),
        ] {
            assert_eq!(classes(line), want, "{line}");
        }
    }

    #[test]
    fn reaching_other_computers_is_never_allowed() {
        for line in [
            "ssh other-server",
            "scp file other:/tmp",
            "sudo ssh root@db",
            "rsync -a site/ backup@nas:/srv",
            "rsync -e ssh a b",
            "nc -lvp 4444",
            "nmap -sP 10.0.0.0/24",
            "tcpdump -i eth0",
            "ssh-keygen -t ed25519",
            "env FOO=1 ssh db",
            "nice -n 5 socat - TCP:db:5432",
        ] {
            assert!(classify(&cmd(line)).never.is_some(), "{line}");
        }
        assert!(classify(&cmd("rsync -a releases/ current/"))
            .never
            .is_none());
        assert_eq!(classes("rsync -a --delete a/ b/"), vec![Destroy]);
    }

    fn server() -> Server {
        clean(
            &ServerInput {
                name: "Dev box".into(),
                host: "dev.example.com".into(),
                port: 0,
                user: "deploy".into(),
                environment: Environment::Development,
                sign_in: SignIn::Password,
                host_key: Some(HostKeyInput {
                    algorithm: "ssh-ed25519".into(),
                    fingerprint: format!("SHA256:{}", "A".repeat(43)),
                }),
                roles: vec!["ops".into()],
                classes: default_classes(Environment::Development),
                approval: ServerApproval::Changes,
                folders: vec!["/srv/app".into(), "/var/www/site/".into()],
                forwards: vec!["localhost:5432".into()],
                ..ServerInput::default()
            },
            "s1",
            None,
            1,
        )
        .unwrap()
    }

    fn run_check(s: &Server, line: &str, cwd: Option<&str>) -> ServerVerdict {
        let k = classify(&cmd(line));
        check(
            &crate::defaults::default_blocked_files(),
            &ServerCheck {
                server: s,
                role_id: "ops",
                role_name: "Operations Engineer",
                what: ServerUse::Run {
                    classified: &k,
                    cwd,
                },
            },
        )
    }

    fn asks(v: &ServerVerdict) -> Option<&str> {
        match v {
            ServerVerdict::Go { ask, .. } => ask.as_deref(),
            ServerVerdict::Deny { reason, .. } => panic!("denied: {reason}"),
        }
    }

    fn denied(v: &ServerVerdict) -> &str {
        match v {
            ServerVerdict::Deny { reason, .. } => reason,
            other => panic!("not denied: {other:?}"),
        }
    }

    #[test]
    fn servers_are_validated_and_production_always_asks() {
        let s = server();
        assert_eq!(s.port, 22);
        assert_eq!(s.folders, ["/srv/app", "/var/www/site"]);
        assert_eq!(s.address(), "deploy@dev.example.com:22");
        for bad in ["ssh://x", "user@x", "a b", "-oProxyCommand=x", ""] {
            assert!(valid_host(bad).is_err(), "{bad}");
        }
        assert_eq!(valid_host("[2001:db8::1]").unwrap(), "2001:db8::1");
        assert!(valid_folder("srv/app").is_err());
        assert!(valid_folder("/srv/../etc").is_err());
        assert!(valid_forward("localhost").is_err());
        assert!(valid_fingerprint("MD5:aa:bb").is_err());
        let mut input = ServerInput {
            name: "Shop".into(),
            host: "10.0.0.5".into(),
            user: "root".into(),
            environment: Environment::Production,
            approval: ServerApproval::Allowed,
            ..ServerInput::default()
        };
        let p = clean(&input, "p", None, 5).unwrap();
        assert_eq!(
            p.approval,
            ServerApproval::Every,
            "production asks for every command"
        );
        assert!(!default_classes(Environment::Production).contains(&Destroy));
        input.user = "bad user".into();
        assert!(clean(&input, "p", None, 5).is_err());
    }

    #[test]
    fn development_server_policy() {
        let s = server();
        assert_eq!(asks(&run_check(&s, "systemctl status nginx", None)), None);
        assert!(asks(&run_check(&s, "systemctl restart nginx", None))
            .unwrap()
            .contains("not looking around"));
        let mut allowed = s.clone();
        allowed.approval = ServerApproval::Allowed;
        assert_eq!(
            asks(&run_check(&allowed, "git pull", Some("/srv/app"))),
            None
        );
        assert!(denied(&run_check(&s, "rm -rf build", Some("/srv/app"))).contains("Delete, wipe"));
        let mut destroy = allowed.clone();
        destroy.classes.push(Destroy);
        assert!(asks(&run_check(&destroy, "rm -rf build", Some("/srv/app")))
            .unwrap()
            .contains("always waits"));
        // Changing paths only inside the folders.
        assert!(denied(&run_check(
            &allowed,
            "cp a /etc/nginx/nginx.conf",
            Some("/srv/app")
        ))
        .contains("outside the folders"));
        assert!(
            denied(&run_check(&allowed, "mv x ../../etc/y", Some("/srv/app")))
                .contains("outside the folders")
        );
        assert_eq!(
            asks(&run_check(
                &allowed,
                "cp a /var/www/site/b",
                Some("/srv/app")
            )),
            None
        );
        // Reading anywhere is fine, but never a blocked file.
        assert_eq!(asks(&run_check(&s, "cat /etc/os-release", None)), None);
        assert!(denied(&run_check(&s, "cat /srv/app/.env", None)).contains("blocked file"));
        assert!(denied(&run_check(&s, "cat ~/.ssh/id_ed25519", None)).contains("blocked file"));
        // Never another computer.
        assert!(denied(&run_check(&s, "ssh db", None)).contains("never lets a worker"));
        // Not an allowed role.
        let k = classify(&cmd("uptime"));
        let v = check(
            &[],
            &ServerCheck {
                server: &s,
                role_id: "writer",
                role_name: "Documentation Writer",
                what: ServerUse::Run {
                    classified: &k,
                    cwd: None,
                },
            },
        );
        assert!(denied(&v).contains("Documentation Writer role may not use Dev box"));
        // sudo asks, and is a sensitive action.
        let mut admin = allowed.clone();
        admin.classes.push(Admin);
        match run_check(&admin, "sudo systemctl restart nginx", None) {
            ServerVerdict::Go { ask, sensitive } => {
                assert!(ask.unwrap().contains("administrator"));
                assert_eq!(sensitive.map(|s| s.0), Some(SensitiveKind::Privilege));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn production_asks_for_every_command_and_blocks_destroying() {
        let mut s = server();
        s.environment = Environment::Production;
        s.classes = default_classes(Environment::Production);
        assert!(asks(&run_check(&s, "uptime", None))
            .unwrap()
            .contains("production server: every command"));
        assert!(denied(&run_check(&s, "reboot", None)).contains("production server"));
        s.classes.push(Destroy);
        assert!(asks(&run_check(&s, "reboot", None))
            .unwrap()
            .contains("production"));
    }

    #[test]
    fn identity_forwards_and_folders() {
        let mut s = server();
        let connect = |s: &Server, what| {
            check(
                &[],
                &ServerCheck {
                    server: s,
                    role_id: "ops",
                    role_name: "Ops",
                    what,
                },
            )
        };
        assert!(asks(&connect(
            &s,
            ServerUse::Forward {
                to: "localhost:5432"
            }
        ))
        .unwrap()
        .contains("always waits"));
        assert!(denied(&connect(&s, ServerUse::Forward { to: "db:5432" })).contains("not one of"));
        s.host_key = None;
        assert!(denied(&connect(&s, ServerUse::Connect)).contains("not been checked and pinned"));
        let s = server();
        assert_eq!(resolve_cwd(&s, None).unwrap().as_deref(), Some("/srv/app"));
        assert_eq!(
            resolve_cwd(&s, Some("current/public")).unwrap().as_deref(),
            Some("/srv/app/current/public")
        );
        assert_eq!(
            resolve_cwd(&s, Some("/var/www/site/wp"))
                .unwrap()
                .as_deref(),
            Some("/var/www/site/wp")
        );
        assert!(resolve_cwd(&s, Some("/etc")).is_err());
        assert!(resolve_cwd(&s, Some("../x")).is_err());
        let mut home = s.clone();
        home.folders.clear();
        assert_eq!(resolve_cwd(&home, None).unwrap(), None);
        assert!(resolve_cwd(&home, Some("/srv")).is_err());
        assert_eq!(
            resolve_cwd(&home, Some("site")).unwrap().as_deref(),
            Some("site")
        );
    }
}
