//! Add-on tools the owner sets up (Phase 20 part 20C; ADR-066, ADR-071): another program that
//! offers tools over MCP, run as an approved program, **off** until the owner switches it on.
//!
//! What Guard keeps and checks here, in its settings (`addOns`), never a secret's value:
//! - the program (an installed program, never a shell or a program that downloads code each time
//!   it starts: the owner's choice 12), its arguments, and the names of the stored secrets it is
//!   given (ADR-048: only to that program);
//! - each of its tools as the program described it when the owner last looked, and the owner's
//!   mark for it: **Off**, **Reading** (goes ahead), or **Changing** (asks every time);
//! - **Who may use it**: roles and agents, **Read only** (its Reading tools) or **Read and write**
//!   (its Changing tools too). Empty to start (the owner's choice 13).
//!
//! Nothing here runs a program: the capability broker does, after asking Guard.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::connections::{Access, AccessLevel, Who};
use crate::service::{Guard, OWNER};

/// Most add-ons kept (ADR-066 §1).
pub const MAX_ADD_ONS: usize = 20;
/// Most tools kept for one add-on.
pub const MAX_TOOLS: usize = 100;
/// Most arguments, and the longest one.
pub const MAX_ARGS: usize = 30;
pub const MAX_ARG_CHARS: usize = 500;
/// Most stored secrets given to one add-on.
pub const MAX_SECRETS: usize = 8;
/// The longest description kept as the program gave it (the card shows it; a worker sees at most
/// [`MAX_DESCRIPTION_SHOWN`]).
pub const MAX_DESCRIPTION: usize = 2000;
pub const MAX_DESCRIPTION_SHOWN: usize = 300;
/// The biggest description of a tool's input kept.
pub const MAX_INPUT_BYTES: usize = 16 * 1024;
/// The longest name Plenipo gives a tool (`addon_<add-on>_<tool>`): AI tools add their own start
/// (`mcp__plenipo__`) and allow 64 characters in all.
pub const MAX_ALIAS: usize = 50;

/// The owner's mark for one of an add-on's tools (ADR-066 §2).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolMark {
    /// Not offered to anyone.
    #[default]
    Off,
    /// Goes ahead, for workers at **Read only** or more.
    Reading,
    /// Asks the owner every time, for workers at **Read and write**.
    Changing,
}

impl ToolMark {
    pub fn words(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Reading => "Reading",
            Self::Changing => "Changing",
        }
    }
}

/// One of an add-on's tools, as the program described it when the owner last looked (the
/// program's own words), and the owner's mark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddOnTool {
    /// The program's own name for it.
    pub name: String,
    /// Plenipo's name for it, offered to workers: `addon_<add-on>_<tool>`.
    pub alias: String,
    pub mark: ToolMark,
    /// Its description, as the program gave it.
    pub description: String,
    /// What it takes, as the program described it (a JSON Schema object).
    #[ts(type = "unknown")]
    pub input: Value,
    /// The program's own hints: shown as hints only, they never decide (ADR-066 §2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub read_only_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub destructive_hint: Option<bool>,
    /// Its description or input changed since the owner marked it: **Off** until the owner looks
    /// again.
    #[serde(default)]
    pub changed: bool,
}

/// A tool as the program lists it (`tools/list`), before the owner's marks are put on.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub name: String,
    pub description: String,
    pub input: Value,
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
}

/// One add-on program (ADR-066 §1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddOn {
    /// Plenipo's ID for it, from its name: small letters and digits, 1–12 (`notion`).
    pub id: String,
    /// The owner's name for it.
    pub name: String,
    /// The installed program, by its full path.
    pub program: String,
    /// Its arguments, one each (never a shell line).
    pub args: Vec<String>,
    /// The names of the stored secrets it is given (Settings → Secrets), each as its variable.
    pub secrets: Vec<String>,
    /// Switched on by the owner. Off to start.
    pub on: bool,
    pub tools: Vec<AddOnTool>,
    /// **Who may use it**: empty to start.
    pub access: Vec<Access>,
    /// When the owner last had Plenipo look at its tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub checked_at: Option<u64>,
    #[ts(type = "number")]
    pub added_at: u64,
}

impl AddOn {
    /// The tool Plenipo calls `alias`.
    pub fn tool(&self, alias: &str) -> Option<&AddOnTool> {
        self.tools.iter().find(|t| t.alias == alias)
    }

    /// The line on **Who may use it** that applies to an agent in a role: the agent's own, else
    /// its role's.
    pub fn line_for(&self, position_id: Option<&str>, role_id: &str) -> Option<&Access> {
        position_id
            .and_then(|p| {
                self.access
                    .iter()
                    .find(|a| matches!(&a.who, Who::Agent { id } if id == p))
            })
            .or_else(|| {
                self.access
                    .iter()
                    .find(|a| matches!(&a.who, Who::Role { id } if id == role_id))
            })
    }

    /// Whether a worker at `level` may use a tool marked `mark`.
    pub fn allows(level: AccessLevel, mark: ToolMark) -> bool {
        match mark {
            ToolMark::Off => false,
            ToolMark::Reading => true,
            ToolMark::Changing => level == AccessLevel::ReadWrite,
        }
    }
}

/// What the owner types to add a program. The program is its full path (the broker finds a bare
/// name on PATH first).
#[derive(Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddOnInput {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<String>,
}

impl std::fmt::Debug for AddOnInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddOnInput")
            .field("name", &self.name)
            .field("program", &self.program)
            .field("args", &self.args.len())
            .field("secrets", &self.secrets)
            .finish()
    }
}

/// A change to an add-on: only the fields given change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddOnChange {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub program: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub secrets: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub on: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub access: Option<Vec<Access>>,
}

// ---- Checking what the owner types ------------------------------------------------------------

/// Shells: a program that runs whatever line it is given (ADR-066 §1).
const SHELLS: [&str; 16] = [
    "cmd",
    "command",
    "powershell",
    "powershell_ise",
    "pwsh",
    "bash",
    "sh",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "wsl",
    "cscript",
    "wscript",
];
/// Programs that download code each time they start (the owner's choice 12).
const DOWNLOADERS: [&str; 5] = ["npx", "pnpx", "bunx", "uvx", "mshta"];
/// A program and the first argument that make it download and run code each time.
const DOWNLOADING: [(&str, &str); 7] = [
    ("npm", "exec"),
    ("npm", "x"),
    ("pnpm", "dlx"),
    ("yarn", "dlx"),
    ("bun", "x"),
    ("pipx", "run"),
    ("uv", "tool"),
];

/// A program's file name without its folder or Windows' run extensions, in small letters.
pub fn program_stem(program: &str) -> String {
    let file = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com", ".ps1", ".sh"] {
        if let Some(stem) = file.strip_suffix(ext) {
            return stem.to_owned();
        }
    }
    file
}

/// Why a program and its arguments cannot be an add-on, in plain words, or `None`.
pub fn refused_program(program: &str, args: &[String]) -> Option<String> {
    let stem = program_stem(program);
    if SHELLS.contains(&stem.as_str()) {
        return Some(format!(
            "{stem} is a shell: it runs whatever it is given. Add the tool program itself."
        ));
    }
    let first = args
        .first()
        .map(|a| a.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let downloads = DOWNLOADERS.contains(&stem.as_str())
        || DOWNLOADING.iter().any(|(p, a)| *p == stem && *a == first)
        || (stem == "uv" && first == "run" && args.iter().any(|a| a.contains("--with")));
    if downloads {
        return Some(format!(
            "{stem} {}downloads code each time it starts, so what runs could change without you \
             seeing it. Install the program first (for example npm install -g <package>, or pipx \
             install <package>), then add the installed program.",
            if DOWNLOADERS.contains(&stem.as_str()) {
                String::new()
            } else {
                format!("{first} ")
            }
        ));
    }
    if stem == "deno"
        && args
            .iter()
            .any(|a| a.starts_with("http://") || a.starts_with("https://"))
    {
        return Some(
            "deno would download the program from the web each time it starts. Download it once \
             and add the file instead."
                .into(),
        );
    }
    None
}

/// An add-on's ID from the owner's name: its letters and digits, small, 1–12, not already taken
/// (a number is added: `notion2`).
pub fn id_for(name: &str, taken: &[&str]) -> String {
    let base: String = name
        .to_ascii_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(10)
        .collect();
    let base = if base.is_empty() {
        "addon".into()
    } else {
        base
    };
    if !taken.contains(&base.as_str()) {
        return base;
    }
    (2..100)
        .map(|n| format!("{base}{n}"))
        .find(|id| !taken.contains(&id.as_str()))
        .unwrap_or_else(|| format!("{base}x"))
}

/// A short check code for a name (FNV-1a, 32 bits), so a shortened tool name stays its own.
fn check_code(s: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{:04x}", h & 0xffff)
}

/// Plenipo's name for the program's tool `name`: `addon_<add-on>_<tool>`, in small letters,
/// digits, and `_`, at most [`MAX_ALIAS`] characters. A name that had to change is given a check
/// code, so two tools never share one.
pub fn alias_for(add_on: &str, name: &str) -> String {
    let clean: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let clean = clean.trim_matches('_').to_owned();
    let start = format!("addon_{add_on}_");
    let room = MAX_ALIAS - start.len();
    if !clean.is_empty() && clean == name && clean.len() <= room {
        return format!("{start}{clean}");
    }
    let code = check_code(name);
    let keep: String = clean.chars().take(room.saturating_sub(5)).collect();
    let keep = keep.trim_end_matches('_');
    if keep.is_empty() {
        format!("{start}{code}")
    } else {
        format!("{start}{keep}_{code}")
    }
}

/// The program's tools, with the owner's marks kept where nothing changed (ADR-066 §2): a new
/// tool is **Off**; one whose description or input changed goes back to **Off**, marked changed;
/// one the program no longer lists is dropped. At most [`MAX_TOOLS`].
pub fn merge_tools(add_on: &str, before: &[AddOnTool], listed: Vec<Listed>) -> Vec<AddOnTool> {
    let mut out: Vec<AddOnTool> = Vec::new();
    for l in listed.into_iter().take(MAX_TOOLS) {
        if l.name.is_empty() || out.iter().any(|t| t.name == l.name) {
            continue;
        }
        let description: String = l
            .description
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(MAX_DESCRIPTION)
            .collect();
        let input = if serde_json::to_vec(&l.input).map_or(usize::MAX, |b| b.len())
            <= MAX_INPUT_BYTES
            && l.input.is_object()
        {
            l.input
        } else {
            json!({ "type": "object" })
        };
        let mut alias = alias_for(add_on, &l.name);
        if out.iter().any(|t| t.alias == alias) {
            alias = alias_for(add_on, &format!("{}#{}", l.name, out.len()));
        }
        let (mark, changed) = match before.iter().find(|b| b.name == l.name) {
            Some(b) if b.description == description && b.input == input => (b.mark, b.changed),
            Some(b) => (ToolMark::Off, b.mark != ToolMark::Off || b.changed),
            None => (ToolMark::Off, false),
        };
        out.push(AddOnTool {
            name: l.name,
            alias,
            mark,
            description,
            input,
            read_only_hint: l.read_only_hint,
            destructive_hint: l.destructive_hint,
            changed,
        });
    }
    out
}

// ---- Guard's decision -------------------------------------------------------------------------

/// A worker's use of an add-on's tool, for Guard.
#[derive(Debug, Clone, Copy)]
pub struct AddOnCheck<'a> {
    pub add_on: &'a AddOn,
    pub tool: &'a AddOnTool,
}

// ---- The owner's changes, recorded ------------------------------------------------------------

/// What an add-on change records: never a secret's value, an argument's text, or a tool's words.
fn change_payload(a: &AddOn, what: &str) -> Value {
    json!({
        "addOnId": a.id,
        "name": a.name,
        "what": what,
        "on": a.on,
        "program": program_stem(&a.program),
        "arguments": a.args.len(),
        "secrets": a.secrets,
        "tools": a.tools.iter().map(|t| json!({ "tool": t.name, "mark": t.mark.words() })).collect::<Vec<_>>(),
        "access": a.access.iter().map(|x| json!({ "who": x.who, "level": x.level.words() })).collect::<Vec<_>>(),
    })
}

impl Guard {
    /// Every add-on kept.
    pub fn add_ons(&self) -> crate::Result<Vec<AddOn>> {
        Ok(self.config()?.add_ons)
    }

    /// Add a program, **off** (ADR-066 §1). `program`: its full path, found by the broker.
    /// Records `guard.add_on_added`.
    pub fn add_add_on(&self, input: &AddOnInput) -> crate::Result<AddOn> {
        let now = plenipo_ledger::now_ms();
        self.update("guard.add_on_added", OWNER, |c| {
            let a = c.add_add_on(input, now)?;
            Ok(Some((change_payload(&a, "added"), a)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Change an add-on (its name, program, arguments, secrets, on or off, or who may use it).
    /// Records `guard.add_on_changed`.
    pub fn change_add_on(&self, id: &str, change: &AddOnChange) -> crate::Result<AddOn> {
        let records = self.ledger().org_records()?;
        let roles: Vec<String> = records.roles.iter().map(|r| r.id.clone()).collect();
        let agents: Vec<String> = records
            .positions
            .iter()
            .filter(|p| !p.is_deleted())
            .map(|p| p.id.clone())
            .collect();
        self.update("guard.add_on_changed", OWNER, |c| {
            let a = c.change_add_on(id, change, &roles, &agents)?;
            Ok(Some((change_payload(&a, "changed"), a)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Keep the tools the program listed, with the owner's marks where nothing changed.
    /// Records `guard.add_on_changed`.
    pub fn add_on_tools_listed(&self, id: &str, listed: Vec<Listed>) -> crate::Result<AddOn> {
        let now = plenipo_ledger::now_ms();
        self.update("guard.add_on_changed", OWNER, |c| {
            let a = c.add_on_tools_listed(id, listed, now)?;
            Ok(Some((change_payload(&a, "tools listed"), a)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Mark tools **Off**, **Reading**, or **Changing** (by the program's names for them).
    /// Records `guard.add_on_changed`.
    pub fn set_add_on_tools(
        &self,
        id: &str,
        marks: &std::collections::BTreeMap<String, ToolMark>,
    ) -> crate::Result<AddOn> {
        self.update("guard.add_on_changed", OWNER, |c| {
            let a = c.set_add_on_tools(id, marks)?;
            Ok(Some((change_payload(&a, "marks"), a)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Remove an add-on. Records `guard.add_on_removed`.
    pub fn remove_add_on(&self, id: &str) -> crate::Result<AddOn> {
        self.update("guard.add_on_removed", OWNER, |c| {
            let a = c.remove_add_on(id)?;
            Ok(Some((json!({ "addOnId": a.id, "name": a.name }), a)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(name: &str, description: &str) -> Listed {
        Listed {
            name: name.into(),
            description: description.into(),
            input: json!({ "type": "object", "properties": {} }),
            read_only_hint: None,
            destructive_hint: None,
        }
    }

    #[test]
    fn shells_and_downloading_programs_are_refused() {
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        for (program, a) in [
            (
                "C:\\Windows\\System32\\cmd.exe",
                args(&["/c", "notion-mcp"]),
            ),
            ("/usr/bin/bash", args(&["-c", "x"])),
            ("C:\\Program Files\\PowerShell\\7\\pwsh.exe", args(&[])),
            (
                "C:\\nodejs\\npx.cmd",
                args(&["-y", "@notionhq/notion-mcp-server"]),
            ),
            ("/home/alex/.local/bin/uvx", args(&["mcp-server-fetch"])),
            ("/usr/local/bin/bunx", args(&["x"])),
            ("C:\\nodejs\\npm.cmd", args(&["exec", "x"])),
            ("/usr/bin/pnpm", args(&["dlx", "x"])),
            ("/usr/bin/yarn", args(&["dlx", "x"])),
            ("/usr/bin/pipx", args(&["run", "x"])),
            ("/usr/bin/uv", args(&["tool", "run", "x"])),
            (
                "/usr/bin/deno",
                args(&["run", "https://example.com/server.ts"]),
            ),
        ] {
            assert!(refused_program(program, &a).is_some(), "{program} {a:?}");
        }
        for (program, a) in [
            (
                "C:\\Users\\alex\\AppData\\Roaming\\npm\\notion-mcp-server.cmd",
                args(&[]),
            ),
            ("/usr/local/bin/stripe-mcp", args(&["--tools=all"])),
            ("/usr/bin/python3", args(&["-m", "mcp_server_git"])),
            (
                "/usr/bin/node",
                args(&["/opt/server/index.js", "--url", "https://api.example.com"]),
            ),
            ("/usr/bin/npm", args(&["run", "start"])),
        ] {
            assert_eq!(refused_program(program, &a), None, "{program} {a:?}");
        }
        assert!(refused_program("npx", &[])
            .unwrap()
            .contains("Install the program first"));
    }

    #[test]
    fn tool_names_are_short_plain_and_their_own() {
        assert_eq!(alias_for("notion", "search"), "addon_notion_search");
        assert_eq!(alias_for("notion", "get_page"), "addon_notion_get_page");
        // Changed names get a check code, so two tools never share one.
        let a = alias_for("notion", "Get-Page");
        let b = alias_for("notion", "get page");
        assert!(a.starts_with("addon_notion_get_page_"), "{a}");
        assert_ne!(a, b);
        let long = alias_for("addon12345", &"x".repeat(80));
        assert!(long.len() <= MAX_ALIAS, "{long}");
        assert!(long
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
        assert_eq!(alias_for("n", "!!!").len(), "addon_n_".len() + 4);
        assert_eq!(id_for("Notion (8 West)", &[]), "notion8wes");
        assert_eq!(id_for("Notion", &["notion"]), "notion2");
        assert_eq!(id_for("—", &[]), "addon");
    }

    #[test]
    fn a_new_or_changed_tool_starts_off() {
        let first = merge_tools("t", &[], vec![listed("lookup", "Looks up an order")]);
        assert_eq!(first[0].mark, ToolMark::Off);
        assert!(!first[0].changed);
        let mut marked = first.clone();
        marked[0].mark = ToolMark::Reading;
        // Nothing changed: the mark stays.
        let again = merge_tools("t", &marked, vec![listed("lookup", "Looks up an order")]);
        assert_eq!(again[0].mark, ToolMark::Reading);
        // Its description changed: back to Off, marked changed; a new tool starts Off.
        let changed = merge_tools(
            "t",
            &marked,
            vec![
                listed("lookup", "Looks up an order, and refunds it"),
                listed("create_ticket", "Creates a ticket"),
            ],
        );
        assert_eq!(changed[0].mark, ToolMark::Off);
        assert!(changed[0].changed);
        assert_eq!(changed[1].mark, ToolMark::Off);
        assert!(!changed[1].changed);
        // A tool no longer listed is dropped; the same name twice is kept once.
        let fewer = merge_tools(
            "t",
            &changed,
            vec![
                listed("create_ticket", "Creates a ticket"),
                listed("create_ticket", "x"),
            ],
        );
        assert_eq!(fewer.len(), 1);
    }

    #[test]
    fn reading_goes_to_read_only_and_changing_needs_read_and_write() {
        assert!(AddOn::allows(AccessLevel::ReadOnly, ToolMark::Reading));
        assert!(!AddOn::allows(AccessLevel::ReadOnly, ToolMark::Changing));
        assert!(AddOn::allows(AccessLevel::ReadWrite, ToolMark::Changing));
        assert!(!AddOn::allows(AccessLevel::ReadWrite, ToolMark::Off));
    }
}
