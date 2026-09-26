//! Plenipo's tools for workers: their names, descriptions, input schemas, the capability each
//! needs, and how their arguments are read. Arguments come from an AI model and are untrusted:
//! everything is checked here for shape and size, then by Guard for permission.

use plenipo_guard::{Capability, Risk};
use serde_json::{json, Value};

/// Longest file content a worker may write in one call (bytes).
pub const MAX_WRITE_BYTES: usize = 1024 * 1024;
/// Most lines one read returns.
pub const MAX_READ_LINES: usize = 2000;
/// Longest commit message.
pub const MAX_MESSAGE_CHARS: usize = 5000;
/// Longest PowerShell script.
pub const MAX_SCRIPT_CHARS: usize = 20_000;
/// Most arguments for one program, and the longest argument.
pub const MAX_ARGS: usize = 64;
pub const MAX_ARG_CHARS: usize = 4000;

/// A tool Plenipo offers.
pub struct ToolDef {
    pub name: &'static str,
    pub capability: Capability,
    pub risk: Risk,
    pub description: &'static str,
    schema: fn() -> Value,
}

impl ToolDef {
    pub fn schema(&self) -> Value {
        (self.schema)()
    }
}

fn path_prop(what: &str) -> Value {
    json!({ "type": "string", "description": format!("{what}, relative to the project folder") })
}

pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "list_directory",
        capability: Capability::FilesystemRead,
        risk: Risk::Read,
        description: "List the files and folders in a folder of the project.",
        schema: || json!({ "type": "object", "properties": { "path": path_prop("Folder (default: the project folder)") } }),
    },
    ToolDef {
        name: "read_file",
        capability: Capability::FilesystemRead,
        risk: Risk::Read,
        description: "Read a text file of the project. Long files are read in parts: give the first line to read (offset, from 1) and how many lines (limit).",
        schema: || json!({ "type": "object", "properties": {
            "path": path_prop("File"),
            "offset": { "type": "integer", "minimum": 1, "description": "First line to read (default 1)" },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_READ_LINES, "description": "How many lines (default 2000)" }
        }, "required": ["path"] }),
    },
    ToolDef {
        name: "search_text",
        capability: Capability::FilesystemRead,
        risk: Risk::Read,
        description: "Find lines containing some text in the project's files (build folders such as node_modules and target are skipped).",
        schema: || json!({ "type": "object", "properties": {
            "query": { "type": "string", "description": "Text to find" },
            "path": path_prop("Folder or file to search (default: the project folder)"),
            "caseSensitive": { "type": "boolean" }
        }, "required": ["query"] }),
    },
    ToolDef {
        name: "write_file",
        capability: Capability::FilesystemWrite,
        risk: Risk::Change,
        description: "Create a file, or replace a file's whole content. Folders are created as needed.",
        schema: || json!({ "type": "object", "properties": {
            "path": path_prop("File"),
            "content": { "type": "string" }
        }, "required": ["path", "content"] }),
    },
    ToolDef {
        name: "edit_file",
        capability: Capability::FilesystemWrite,
        risk: Risk::Change,
        description: "Replace exact text in a file. oldText must appear exactly once unless replaceAll is true.",
        schema: || json!({ "type": "object", "properties": {
            "path": path_prop("File"),
            "oldText": { "type": "string" },
            "newText": { "type": "string" },
            "replaceAll": { "type": "boolean" }
        }, "required": ["path", "oldText", "newText"] }),
    },
    ToolDef {
        name: "move_path",
        capability: Capability::FilesystemWrite,
        risk: Risk::Change,
        description: "Move or rename a file or folder inside the project.",
        schema: || json!({ "type": "object", "properties": {
            "from": path_prop("File or folder"),
            "to": path_prop("New path (must not exist yet)")
        }, "required": ["from", "to"] }),
    },
    ToolDef {
        name: "delete_path",
        capability: Capability::FilesystemWrite,
        risk: Risk::Delete,
        description: "Delete a file, or an empty folder.",
        schema: || json!({ "type": "object", "properties": { "path": path_prop("File or empty folder") }, "required": ["path"] }),
    },
    ToolDef {
        name: "run_command",
        capability: Capability::ShellExec,
        risk: Risk::Run,
        description: "Run a program with arguments in the project folder, such as a build or test command (\"cargo\" with [\"test\"]). No shell: give the program and each argument separately. Approved commands run at once; others wait for the owner's approval.",
        schema: || json!({ "type": "object", "properties": {
            "program": { "type": "string", "description": "Program name, like cargo or npm (or ./script inside the project)" },
            "args": { "type": "array", "items": { "type": "string" } },
            "cwd": path_prop("Folder to run in (default: the project folder)"),
            "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": 1800 }
        }, "required": ["program"] }),
    },
    ToolDef {
        name: "run_powershell",
        capability: Capability::PowershellExec,
        risk: Risk::Run,
        description: "Run a PowerShell script in the project folder.",
        schema: || json!({ "type": "object", "properties": {
            "script": { "type": "string" },
            "cwd": path_prop("Folder to run in (default: the project folder)"),
            "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": 1800 }
        }, "required": ["script"] }),
    },
    ToolDef {
        name: "git_status",
        capability: Capability::GitRead,
        risk: Risk::Read,
        description: "Show the project's current branch and changed files (git status).",
        schema: || json!({ "type": "object", "properties": {} }),
    },
    ToolDef {
        name: "git_diff",
        capability: Capability::GitRead,
        risk: Risk::Read,
        description: "Show changes not yet committed (git diff); staged: true shows staged changes.",
        schema: || json!({ "type": "object", "properties": {
            "staged": { "type": "boolean" },
            "path": path_prop("Limit to this file or folder")
        } }),
    },
    ToolDef {
        name: "git_log",
        capability: Capability::GitRead,
        risk: Risk::Read,
        description: "Show recent commits (git log, one line each).",
        schema: || json!({ "type": "object", "properties": {
            "count": { "type": "integer", "minimum": 1, "maximum": 200 },
            "path": path_prop("Limit to this file or folder")
        } }),
    },
    ToolDef {
        name: "git_add",
        capability: Capability::GitWrite,
        risk: Risk::Change,
        description: "Stage files for the next commit (git add).",
        schema: || json!({ "type": "object", "properties": {
            "paths": { "type": "array", "items": { "type": "string" }, "description": "Files or folders, relative to the project folder" }
        }, "required": ["paths"] }),
    },
    ToolDef {
        name: "git_commit",
        capability: Capability::GitWrite,
        risk: Risk::Change,
        description: "Commit the staged changes with a message (git commit).",
        schema: || json!({ "type": "object", "properties": { "message": { "type": "string" } }, "required": ["message"] }),
    },
    ToolDef {
        name: "git_branch",
        capability: Capability::GitWrite,
        risk: Risk::Change,
        description: "Switch to a branch, or create it and switch to it (create: true).",
        schema: || json!({ "type": "object", "properties": {
            "name": { "type": "string" },
            "create": { "type": "boolean" }
        }, "required": ["name"] }),
    },
    ToolDef {
        name: "git_push",
        capability: Capability::GitWrite,
        risk: Risk::External,
        description: "Push commits to the remote server (git push). Always waits for the owner's approval.",
        schema: || json!({ "type": "object", "properties": {
            "remote": { "type": "string", "description": "Default: origin" },
            "branch": { "type": "string", "description": "Default: the current branch" }
        } }),
    },
];

pub fn find(name: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.name == name)
}

/// What a tool call asks for, with its arguments read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    List {
        path: String,
    },
    Read {
        path: String,
        offset: usize,
        limit: usize,
    },
    Search {
        query: String,
        path: String,
        case_sensitive: bool,
    },
    Write {
        path: String,
        content: String,
    },
    Edit {
        path: String,
        old: String,
        new: String,
        all: bool,
    },
    Move {
        from: String,
        to: String,
    },
    Delete {
        path: String,
    },
    Run {
        program: String,
        args: Vec<String>,
        cwd: String,
        timeout: Option<u64>,
    },
    Script {
        script: String,
        cwd: String,
        timeout: Option<u64>,
    },
    GitStatus,
    GitDiff {
        staged: bool,
        path: Option<String>,
    },
    GitLog {
        count: u32,
        path: Option<String>,
    },
    GitAdd {
        paths: Vec<String>,
    },
    GitCommit {
        message: String,
    },
    GitBranch {
        name: String,
        create: bool,
    },
    GitPush {
        remote: String,
        branch: Option<String>,
    },
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("\"{key}\" (text) is required"))
}

fn opt_text(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("\"{key}\" must be text")),
    }
}

fn flag(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("\"{key}\" must be true or false")),
    }
}

fn number(args: &Value, key: &str, min: u64, max: u64) -> Result<Option<u64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .filter(|n| (min..=max).contains(n))
            .map(Some)
            .ok_or_else(|| format!("\"{key}\" must be a whole number from {min} to {max}")),
    }
}

fn strings(args: &Value, key: &str, max: usize) -> Result<Vec<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            if items.len() > max {
                return Err(format!("at most {max} items in \"{key}\""));
            }
            items
                .iter()
                .map(|i| {
                    i.as_str()
                        .filter(|s| s.chars().count() <= MAX_ARG_CHARS && !s.contains('\0'))
                        .map(str::to_owned)
                        .ok_or_else(|| format!("each item of \"{key}\" must be text (at most {MAX_ARG_CHARS} characters)"))
                })
                .collect()
        }
        Some(_) => Err(format!("\"{key}\" must be a list of text")),
    }
}

/// A git name (branch or remote): letters, digits, `.`, `_`, `-`, `/`; not like an option,
/// no `..`, `//`, or a `.lock` ending.
fn git_name(what: &str, name: &str) -> Result<String, String> {
    let n = name.trim();
    let ok = (1..=100).contains(&n.len())
        && n.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        && !n.contains("..")
        && !n.contains("//")
        && !n.ends_with('/')
        && !n.ends_with(".lock");
    if ok {
        Ok(n.to_owned())
    } else {
        Err(format!("{n:?} is not a usable {what} name"))
    }
}

/// Read a call's arguments.
pub fn parse(tool: &ToolDef, args: &Value) -> Result<Action, String> {
    let args = if args.is_null() { &Value::Null } else { args };
    if !args.is_null() && !args.is_object() {
        return Err("the arguments must be an object".into());
    }
    let path_or_root = |key: &str| opt_text(args, key).map(|p| p.unwrap_or_else(|| ".".into()));
    Ok(match tool.name {
        "list_directory" => Action::List {
            path: path_or_root("path")?,
        },
        "read_file" => Action::Read {
            path: text(args, "path")?.to_owned(),
            offset: number(args, "offset", 1, u64::from(u32::MAX))?.unwrap_or(1) as usize,
            limit: number(args, "limit", 1, MAX_READ_LINES as u64)?.unwrap_or(MAX_READ_LINES as u64)
                as usize,
        },
        "search_text" => {
            let query = text(args, "query")?;
            if query.is_empty() || query.chars().count() > 500 || query.contains('\n') {
                return Err("\"query\" must be one line of 1–500 characters".into());
            }
            Action::Search {
                query: query.to_owned(),
                path: path_or_root("path")?,
                case_sensitive: flag(args, "caseSensitive")?,
            }
        }
        "write_file" => {
            let content = text(args, "content")?;
            if content.len() > MAX_WRITE_BYTES {
                return Err(format!(
                    "files written in one call are limited to {MAX_WRITE_BYTES} bytes"
                ));
            }
            Action::Write {
                path: text(args, "path")?.to_owned(),
                content: content.to_owned(),
            }
        }
        "edit_file" => {
            let old = text(args, "oldText")?;
            if old.is_empty() {
                return Err("\"oldText\" cannot be empty".into());
            }
            let new = text(args, "newText")?;
            if old.len() + new.len() > MAX_WRITE_BYTES {
                return Err("the edit is too large".into());
            }
            Action::Edit {
                path: text(args, "path")?.to_owned(),
                old: old.to_owned(),
                new: new.to_owned(),
                all: flag(args, "replaceAll")?,
            }
        }
        "move_path" => Action::Move {
            from: text(args, "from")?.to_owned(),
            to: text(args, "to")?.to_owned(),
        },
        "delete_path" => Action::Delete {
            path: text(args, "path")?.to_owned(),
        },
        "run_command" => {
            let program = text(args, "program")?.trim();
            if program.is_empty()
                || program.chars().count() > 200
                || program.chars().any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(
                    "\"program\" must be a program name without spaces; give arguments in \"args\""
                        .into(),
                );
            }
            Action::Run {
                program: program.to_owned(),
                args: strings(args, "args", MAX_ARGS)?,
                cwd: path_or_root("cwd")?,
                timeout: number(args, "timeoutSeconds", 1, 1800)?,
            }
        }
        "run_powershell" => {
            let script = text(args, "script")?;
            if script.trim().is_empty() || script.chars().count() > MAX_SCRIPT_CHARS {
                return Err(format!(
                    "\"script\" must be 1–{MAX_SCRIPT_CHARS} characters"
                ));
            }
            Action::Script {
                script: script.to_owned(),
                cwd: path_or_root("cwd")?,
                timeout: number(args, "timeoutSeconds", 1, 1800)?,
            }
        }
        "git_status" => Action::GitStatus,
        "git_diff" => Action::GitDiff {
            staged: flag(args, "staged")?,
            path: opt_text(args, "path")?,
        },
        "git_log" => Action::GitLog {
            count: number(args, "count", 1, 200)?.unwrap_or(20) as u32,
            path: opt_text(args, "path")?,
        },
        "git_add" => {
            let paths = strings(args, "paths", MAX_ARGS)?;
            if paths.is_empty() {
                return Err("name at least one path to stage".into());
            }
            Action::GitAdd { paths }
        }
        "git_commit" => {
            let message = text(args, "message")?.trim();
            if message.is_empty()
                || message.chars().count() > MAX_MESSAGE_CHARS
                || message.contains('\0')
            {
                return Err(format!(
                    "the commit message must be 1–{MAX_MESSAGE_CHARS} characters"
                ));
            }
            Action::GitCommit {
                message: message.to_owned(),
            }
        }
        "git_branch" => Action::GitBranch {
            name: git_name("branch", text(args, "name")?)?,
            create: flag(args, "create")?,
        },
        "git_push" => Action::GitPush {
            remote: match opt_text(args, "remote")? {
                Some(r) => git_name("remote", &r)?,
                None => "origin".into(),
            },
            branch: opt_text(args, "branch")?
                .map(|b| git_name("branch", &b))
                .transpose()?,
        },
        other => return Err(format!("unknown tool {other}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: Value) -> Result<Action, String> {
        parse(find(name).unwrap(), &args)
    }

    #[test]
    fn every_tool_has_a_schema_and_a_known_capability() {
        let mut names: Vec<_> = TOOLS.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
        for t in TOOLS {
            assert_eq!(t.schema()["type"], "object", "{}", t.name);
            assert!(t.capability.has_tools(), "{}", t.name);
        }
    }

    #[test]
    fn arguments_are_checked() {
        assert_eq!(
            call("read_file", json!({ "path": "a.txt" })).unwrap(),
            Action::Read {
                path: "a.txt".into(),
                offset: 1,
                limit: MAX_READ_LINES
            }
        );
        assert!(call("read_file", json!({})).is_err());
        assert!(call("read_file", json!({ "path": "a", "limit": 0 })).is_err());
        assert!(call("read_file", json!("not an object")).is_err());
        assert_eq!(
            call("list_directory", Value::Null).unwrap(),
            Action::List { path: ".".into() }
        );
        assert_eq!(
            call(
                "run_command",
                json!({ "program": "cargo", "args": ["test", "-q"] })
            )
            .unwrap(),
            Action::Run {
                program: "cargo".into(),
                args: vec!["test".into(), "-q".into()],
                cwd: ".".into(),
                timeout: None
            }
        );
        assert!(
            call("run_command", json!({ "program": "cargo test" })).is_err(),
            "no shell strings"
        );
        assert!(call("run_command", json!({ "program": "x", "args": [1] })).is_err());
        assert!(call("git_branch", json!({ "name": "--force" })).is_err());
        assert!(call("git_branch", json!({ "name": "a..b" })).is_err());
        assert!(call("git_branch", json!({ "name": "feature/login" })).is_ok());
        assert_eq!(
            call("git_push", json!({})).unwrap(),
            Action::GitPush {
                remote: "origin".into(),
                branch: None
            }
        );
        assert!(call("git_commit", json!({ "message": "  " })).is_err());
        assert!(call(
            "edit_file",
            json!({ "path": "a", "oldText": "", "newText": "x" })
        )
        .is_err());
        assert!(call(
            "write_file",
            json!({ "path": "a", "content": "x".repeat(MAX_WRITE_BYTES + 1) })
        )
        .is_err());
        assert!(call("search_text", json!({ "query": "a\nb" })).is_err());
    }
}
