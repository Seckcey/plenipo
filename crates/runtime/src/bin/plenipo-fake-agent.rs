//! Test double for the AI tools' CLIs (ADR-007, ADR-014). Never shipped.
//!
//! Copy or link this binary under a persona's name from `PERSONAS` (`claude`, `codex`;
//! `.exe` on Windows); it answers like the real CLI named by its file stem: `--version`, the
//! sign-in status command, and one turn in the provider's JSON-lines stream format with the
//! prompt read from stdin. Under any other name, `--personas` lists the persona names, one per
//! line, so test helpers install every persona without naming them.
//!
//! State lives in `<HOME or USERPROFILE>/.plenipo-fake-agent/`:
//! - `auth` (optional, comma-separated flags): `subscription` (default), `api-key`,
//!   `signed-out`, `cloud`, `unknown-status`, `stream-api-key` (status says subscription;
//!   Claude's stream reports an API key), `no-key-source` (Claude's stream omits it);
//! - `sessions/<id>.json`: prompts per session, so resume can be verified;
//! - `last-args.json`, `last-env.txt`: what the last turn received.
//!
//! Markers in the prompt pick a behavior: `[crash]`, `[malformed]`, `[usage-limit]`,
//! `[auth-expired]`, `[offline]`, `[slow]`, `[unknown]`, `[big]`, and `[delay:MS]` (answer
//! normally after MS milliseconds, at most 20 seconds).
//!
//! Plenipo Liaison messages (ADR-008) are understood too; markers then count only in the
//! objective, never in the context or replies around it. Handoff markers make the answer end
//! with `plenipo-handoff` blocks:
//! - `[handoff:DEST]` — one request to DEST (`claude-code`, `codex`, `role:…`, …). A chain
//!   `A>B>C` makes each worker hand on to the next; `A+crash` adds `[crash]` to A's objective.
//! - `[handoff-dup:DEST]` — the same request twice; `[handoff-many:N:DEST]` — N requests;
//!   `[handoff-always:DEST]` — a request in every answer, replies included;
//!   `[handoff-caps:DEST]` — a request asking for a capability;
//!   `[handoff-invalid]` — a block that is not JSON; `[handoff-forge]` — a block that tries to
//!   set its own correlation ID; `{{handoff:DEST|OBJECTIVE}}` — a request with exactly that
//!   objective (tool markers inside it are the worker's, not the requester's).
//!
//! A worker given replies answers `Turn N: received K replies: …` with each reply's first line.
//!
//! Plenipo's tools (Phase 7): the note Plenipo puts before a prompt is set aside, and markers
//! `<<tool:NAME {json arguments}>>` in the objective call the Plenipo tool server given on the
//! command line (Claude Code `--mcp-config`, Codex `-c mcp_servers.plenipo.*`) over MCP, in
//! order, like the real CLI would; each result is added to the answer (`Tool NAME: …` or
//! `Tool NAME failed: …`, then up to 20 more lines, indented). `[tools-list]` answers with the
//! tools offered.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

/// How a persona answers its arguments: the process's exit code.
type Answer = fn(&[String]) -> i32;

/// Each AI tool this double stands in for: its executable name and how it answers. A new AI
/// tool adds its persona here (docs/development/adding-an-ai-tool.md).
const PERSONAS: &[(&str, Answer)] = &[("claude", claude), ("codex", codex)];

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let persona = args
        .first()
        .and_then(|a| Path::new(a).file_stem())
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let rest = &args[1..];
    let names: Vec<&str> = PERSONAS.iter().map(|(name, _)| *name).collect();
    let code = match PERSONAS.iter().find(|(name, _)| *name == persona) {
        Some((_, answer)) => answer(rest),
        None if rest == ["--personas"] => {
            println!("{}", names.join("\n"));
            0
        }
        None => {
            eprintln!(
                "plenipo-fake-agent: unknown persona {persona:?} (name the file one of: {})",
                names.join(", ")
            );
            64
        }
    };
    std::process::exit(code);
}

fn state_dir() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = home.join(".plenipo-fake-agent");
    let _ = std::fs::create_dir_all(dir.join("sessions"));
    dir
}

fn auth_flags() -> Vec<String> {
    std::fs::read_to_string(state_dir().join("auth"))
        .map(|s| s.split(',').map(|f| f.trim().to_owned()).collect())
        .unwrap_or_default()
}

fn auth_has(flag: &str) -> bool {
    auth_flags().iter().any(|f| f == flag)
}

/// The sign-in the status command reports.
fn auth_mode() -> &'static str {
    ["signed-out", "api-key", "cloud", "unknown-status"]
        .into_iter()
        .find(|m| auth_has(m))
        .unwrap_or("subscription")
}

fn out(v: &Value) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{v}");
    let _ = stdout.flush();
}

fn raw(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn read_prompt() -> String {
    let mut prompt = String::new();
    let _ = std::io::stdin().read_to_string(&mut prompt);
    prompt.trim().to_owned()
}

/// Record what this turn received so tests can check it.
fn record_invocation(args: &[String]) {
    let dir = state_dir();
    let _ = std::fs::write(dir.join("last-args.json"), json!(args).to_string());
    let mut names: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    names.sort();
    let _ = std::fs::write(dir.join("last-env.txt"), names.join("\n"));
}

fn session_path(id: &str) -> PathBuf {
    state_dir().join("sessions").join(format!("{id}.json"))
}

fn load_session(id: &str) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(session_path(id)).ok()?).ok()
}

/// The first prompt remembered for a session.
fn first_prompt(id: &str) -> Option<String> {
    load_session(id)?["prompts"][0].as_str().map(str::to_owned)
}

/// Append the prompt; return (turn number, previous prompt).
fn remember(id: &str, prompt: &str) -> (usize, Option<String>) {
    let cwd = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let mut session = load_session(id).unwrap_or_else(|| json!({ "cwd": cwd, "prompts": [] }));
    let prompts = session["prompts"].as_array_mut().expect("prompts");
    let previous = prompts.last().and_then(Value::as_str).map(str::to_owned);
    prompts.push(json!(prompt));
    let n = prompts.len();
    let _ = std::fs::write(session_path(id), session.to_string());
    (n, previous)
}

fn reply(n: usize, prompt: &str, previous: Option<&str>) -> String {
    format!("Turn {n}: you said {prompt:?}. Previous: {previous:?}.")
}

// ---- Plenipo Liaison messages ---------------------------------------------------------------

const ROOT_HEADER: &str = "[Plenipo Liaison — instructions]";
const REQUEST_HEADER: &str = "[Plenipo Liaison — handoff request]";
const REPLIES_HEADER: &str = "[Plenipo Liaison — handoff replies]";
const FOOTER: &str = "[End of Plenipo instructions]";

/// What kind of message the prompt is.
enum Mode {
    Plain,
    /// The owner's objective with Liaison's instructions.
    Root,
    /// A handoff request; its first context block's first line and whether Plenipo gave it
    /// tools.
    Worker {
        context: Option<String>,
        granted: bool,
    },
    /// Replies to earlier requests: one line per reply.
    Replies(Vec<String>),
}

/// The prompt's mode and the text that counts: the objective (or the whole plain prompt).
fn view(prompt: &str) -> (Mode, String) {
    if prompt.starts_with(REPLIES_HEADER) {
        let items: Vec<String> = prompt
            .split("\n## Reply ")
            .skip(1)
            .map(|section| {
                let mut lines = section.lines();
                let header = lines.next().unwrap_or("");
                let who = header.split_once("— ").map_or(header, |(_, w)| w).trim();
                let body: Vec<&str> = lines.collect();
                let first = match body.iter().position(|l| l.starts_with("--- begin reply")) {
                    Some(i) => body.get(i + 1).copied().unwrap_or(""),
                    None => body
                        .iter()
                        .find(|l| l.starts_with("Reason: ") || l.starts_with("Summary: "))
                        .copied()
                        .unwrap_or(""),
                };
                format!("{who}: {}", first.trim())
            })
            .collect();
        let said = format!("{} replies", items.len());
        return (Mode::Replies(items), said);
    }
    if prompt.starts_with(REQUEST_HEADER) {
        let objective = prompt
            .split_once("## Objective\n")
            .and_then(|(_, rest)| rest.split_once("\n\n## Acceptance criteria"))
            .map_or("", |(o, _)| o)
            .trim()
            .to_owned();
        let context = prompt
            .lines()
            .skip_while(|l| !l.starts_with("--- begin context"))
            .nth(1)
            .map(str::to_owned);
        // Permissions come with Plenipo's tools note (set aside before this), never with
        // the request; the caller fills this in.
        return (
            Mode::Worker {
                context,
                granted: false,
            },
            objective,
        );
    }
    if prompt.starts_with(ROOT_HEADER) {
        if let Some((_, objective)) = prompt.split_once(FOOTER) {
            return (Mode::Root, objective.trim().to_owned());
        }
    }
    (Mode::Plain, prompt.to_owned())
}

/// Arguments of every `[name:ARG]` marker in `text`.
fn markers<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("[{name}:");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(end) = after.find(']') else { break };
        out.push(&after[..end]);
        rest = &after[end..];
    }
    out
}

/// Contents of every `{{name:…}}` marker in `text`.
fn braced<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("{{{{{name}:");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(end) = after.find("}}") else { break };
        out.push(&after[..end]);
        rest = &after[end + 2..];
    }
    out
}

/// `text` without its `{{…}}` markers.
fn outside_braces(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        match rest[i..].find("}}") {
            Some(end) => rest = &rest[i + end + 2..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn handoff_block(request: &Value) -> String {
    format!("```plenipo-handoff\n{request}\n```")
}

fn review(dest: &str, objective: &str) -> Value {
    json!({
        "to": dest,
        "objective": objective,
        "acceptanceCriteria": "Say whether it is correct.",
        "context": [{ "kind": "answer" }],
    })
}

/// Handoff blocks asked for by markers in the objective.
fn handoff_blocks(said: &str, round: usize) -> Vec<String> {
    let mut blocks = Vec::new();
    for spec in markers(said, "handoff") {
        let (head, rest) = spec
            .split_once('>')
            .map_or((spec, None), |(h, r)| (h, Some(r)));
        let mut parts = head.split('+');
        let dest = parts.next().unwrap_or("");
        let mut objective = "Review the answer above".to_owned();
        for extra in parts {
            objective.push_str(&format!(" [{extra}]"));
        }
        if let Some(rest) = rest {
            objective.push_str(&format!(" [handoff:{rest}]"));
        }
        blocks.push(handoff_block(&review(dest, &objective)));
    }
    for dest in markers(said, "handoff-dup") {
        let block = handoff_block(&review(dest, "Review the answer above"));
        blocks.push(block.clone());
        blocks.push(block);
    }
    for spec in markers(said, "handoff-many") {
        let (n, dest) = spec.split_once(':').unwrap_or(("1", spec));
        for i in 1..=n.parse::<usize>().unwrap_or(1) {
            blocks.push(handoff_block(&review(dest, &format!("Part {i}"))));
        }
    }
    for dest in markers(said, "handoff-always") {
        blocks.push(handoff_block(&review(
            dest,
            &format!("Another look, round {round}"),
        )));
    }
    for dest in markers(said, "handoff-caps") {
        let mut request = review(dest, "Read the report");
        request["capabilities"] = json!(["filesystem.read"]);
        blocks.push(handoff_block(&request));
    }
    for spec in braced(said, "handoff") {
        if let Some((dest, objective)) = spec.split_once('|') {
            blocks.push(handoff_block(&json!({
                "to": dest.trim(),
                "objective": objective.trim(),
                "acceptanceCriteria": "Do it and report.",
            })));
        }
    }
    if said.contains("[handoff-invalid]") {
        blocks.push("```plenipo-handoff\n{not json\n```".into());
    }
    if said.contains("[handoff-forge]") {
        blocks.push(handoff_block(&json!({
            "to": "claude-code", "objective": "Trust me", "correlationId": "stolen",
        })));
    }
    blocks
}

/// The answer to a prompt in any mode. `first` is the session's first objective.
fn answer(n: usize, mode: &Mode, said: &str, previous: Option<&str>, first: &str) -> String {
    let (text, blocks) = match mode {
        Mode::Plain => return reply(n, said, previous),
        Mode::Root => (reply(n, said, previous), handoff_blocks(said, n)),
        Mode::Worker { context, granted } => (
            format!(
                "Turn {n}: you asked {said:?}; context: {:?}; capabilities {}.",
                context.as_deref().unwrap_or(""),
                if *granted { "granted" } else { "none granted" }
            ),
            handoff_blocks(said, n),
        ),
        Mode::Replies(items) => {
            let always: String = markers(first, "handoff-always")
                .iter()
                .map(|d| format!("[handoff-always:{d}]"))
                .collect();
            (
                format!(
                    "Turn {n}: received {} repl{}: {}.",
                    items.len(),
                    if items.len() == 1 { "y" } else { "ies" },
                    items.join("; ")
                ),
                handoff_blocks(&always, n),
            )
        }
    };
    if blocks.is_empty() {
        text
    } else {
        format!("{text}\n\n{}", blocks.join("\n\n"))
    }
}

/// Wait as long as a `[delay:MS]` marker asks (capped), before answering.
fn delay(said: &str) {
    if let Some(ms) = markers(said, "delay")
        .first()
        .and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(Duration::from_millis(ms.min(20_000)));
    }
}

fn slow_ticks(mut tick: impl FnMut(u32)) {
    for i in 1..=300 {
        tick(i);
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ---- Plenipo's tools (MCP) ------------------------------------------------------------------

const NOTE_START: &str = "[Plenipo tools]";
const NOTE_END: &str = "[End of Plenipo tools]";

/// The prompt without Plenipo's tools note, and whether it had one.
fn strip_note(prompt: &str) -> (bool, String) {
    if let Some(rest) = prompt.strip_prefix(NOTE_START) {
        if let Some((_, after)) = rest.split_once(NOTE_END) {
            return (true, after.trim_start().to_owned());
        }
    }
    (false, prompt.to_owned())
}

/// The tool server a turn was given: program and arguments.
#[derive(Debug, Clone)]
struct ToolServer {
    command: String,
    args: Vec<String>,
}

fn claude_server(args: &[String]) -> Option<ToolServer> {
    let path = flag(args, "--mcp-config")?;
    let config: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let s = &config["mcpServers"]["plenipo"];
    Some(ToolServer {
        command: s["command"].as_str()?.to_owned(),
        args: s["args"]
            .as_array()?
            .iter()
            .filter_map(|a| a.as_str().map(str::to_owned))
            .collect(),
    })
}

/// A TOML basic string (`"…"`), unescaped.
fn toml_string(value: &str) -> Option<(String, &str)> {
    let rest = value.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &rest[i + 1..])),
            '\\' => match chars.next()?.1 {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = (0..4).filter_map(|_| chars.next().map(|x| x.1)).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}

fn toml_array(value: &str) -> Option<Vec<String>> {
    let mut rest = value.trim().strip_prefix('[')?;
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix(']') {
            return r.trim().is_empty().then_some(out);
        }
        let (item, after) = toml_string(rest)?;
        out.push(item);
        rest = after.trim_start();
        rest = rest.strip_prefix(',').unwrap_or(rest);
    }
}

fn codex_server(args: &[String]) -> Option<ToolServer> {
    let mut command = None;
    let mut list = None;
    for (i, a) in args.iter().enumerate() {
        if a != "-c" {
            continue;
        }
        let Some((key, value)) = args.get(i + 1).and_then(|v| v.split_once('=')) else {
            continue;
        };
        match key {
            "mcp_servers.plenipo.command" => command = toml_string(value).map(|v| v.0),
            "mcp_servers.plenipo.args" => list = toml_array(value),
            _ => {}
        }
    }
    Some(ToolServer {
        command: command?,
        args: list.unwrap_or_default(),
    })
}

/// `<<tool:NAME {json}>>` markers, in order (not those inside a `{{…}}` marker).
fn tool_calls(text: &str) -> Vec<(String, Value)> {
    let text = outside_braces(text);
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("<<tool:") {
        let after = &rest[i + 7..];
        let Some(end) = after.find(">>") else { break };
        let body = &after[..end];
        let (name, args) = body.split_once(' ').unwrap_or((body, "{}"));
        out.push((
            name.trim().to_owned(),
            serde_json::from_str(args.trim()).unwrap_or(Value::Null),
        ));
        rest = &after[end + 2..];
    }
    out
}

/// A running MCP client connection to the tool server.
struct Mcp {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: std::io::BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(server: &ToolServer) -> Result<Self, String> {
        let mut child = std::process::Command::new(&server.command)
            .args(&server.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| format!("could not start the tool server: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = std::io::BufReader::new(child.stdout.take().ok_or("no stdout")?);
        let mut mcp = Self {
            child,
            stdin,
            stdout,
            next: 0,
        };
        mcp.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "fake-agent", "version": "1" } }),
        )?;
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        Ok(mcp)
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.stdin, "{message}")
            .and_then(|()| self.stdin.flush())
            .map_err(|e| format!("the tool server went away: {e}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        use std::io::BufRead as _;
        self.next += 1;
        let id = self.next;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            let mut line = String::new();
            let n = self
                .stdout
                .read_line(&mut line)
                .map_err(|e| format!("reading the tool server: {e}"))?;
            if n == 0 {
                return Err("the tool server closed the connection".into());
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if v["id"] == json!(id) {
                if let Some(e) = v.get("error") {
                    return Err(format!("error {}: {}", e["code"], e["message"]));
                }
                return Ok(v["result"].clone());
            }
        }
    }

    /// Close the connection like a real AI tool: end the server's input, give it a moment to
    /// exit, then stop it (never wait on it forever).
    fn finish(mut self) {
        drop(self.stdin);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One tool call's outcome: (name, arguments, text, is_error).
type ToolOutcome = (String, Value, String, bool);

/// Run the marked tool calls (and a listing when asked) against `server`.
fn use_tools(
    server: Option<&ToolServer>,
    calls: &[(String, Value)],
    list: bool,
) -> (Vec<String>, Vec<ToolOutcome>) {
    let Some(server) = server else {
        let failed = calls
            .iter()
            .map(|(n, a)| {
                (
                    n.clone(),
                    a.clone(),
                    "no Plenipo tools were given".to_owned(),
                    true,
                )
            })
            .collect();
        return (Vec::new(), failed);
    };
    let mut mcp = match Mcp::start(server) {
        Ok(m) => m,
        Err(e) => {
            let failed = calls
                .iter()
                .map(|(n, a)| (n.clone(), a.clone(), e.clone(), true))
                .collect();
            return (Vec::new(), failed);
        }
    };
    let names = if list {
        mcp.request("tools/list", json!({}))
            .ok()
            .and_then(|r| {
                r["tools"].as_array().map(|t| {
                    t.iter()
                        .filter_map(|t| t["name"].as_str().map(str::to_owned))
                        .collect()
                })
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut outcomes = Vec::new();
    for (name, args) in calls {
        let outcome = match mcp.request("tools/call", json!({ "name": name, "arguments": args })) {
            Ok(r) => (
                r["content"][0]["text"].as_str().unwrap_or("").to_owned(),
                r["isError"].as_bool().unwrap_or(false),
            ),
            Err(e) => (e, true),
        };
        outcomes.push((name.clone(), args.clone(), outcome.0, outcome.1));
    }
    mcp.finish();
    (names, outcomes)
}

/// Lines added to the answer for the tools used.
fn tool_lines(names: &[String], list: bool, outcomes: &[ToolOutcome]) -> Vec<String> {
    let mut out = Vec::new();
    if list {
        out.push(format!("Tools: {}.", names.join(", ")));
    }
    for (name, _, text, is_error) in outcomes {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let first = lines.next().unwrap_or("").trim();
        out.push(if *is_error {
            format!("Tool {name} failed: {first}")
        } else {
            format!("Tool {name}: {first}")
        });
        // The rest of the result (a little of it), indented.
        out.extend(lines.take(20).map(|l| format!("    {}", l.trim_end())));
    }
    out
}

// ---- Claude Code ------------------------------------------------------------------------

fn claude(args: &[String]) -> i32 {
    if args.first().map(String::as_str) == Some("--version") {
        println!("2.1.999 (Claude Code)");
        return 0;
    }
    if args.len() >= 2 && args[0] == "auth" && args[1] == "status" {
        return claude_auth();
    }
    if args.iter().any(|a| a == "-p") {
        return claude_turn(args);
    }
    eprintln!("fake claude: unsupported arguments {args:?}");
    2
}

fn claude_auth() -> i32 {
    let (body, code) = match auth_mode() {
        "signed-out" => (json!({ "loggedIn": false }), 1),
        "api-key" => (
            json!({ "loggedIn": true, "authMethod": "api_key", "apiProvider": "firstParty" }),
            0,
        ),
        "cloud" => (json!({ "loggedIn": true, "apiProvider": "bedrock" }), 0),
        "unknown-status" => {
            eprintln!("error: unknown command 'auth'");
            return 1;
        }
        _ => (
            json!({
                "loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty",
                "email": "owner@example.com", "orgName": "Example", "subscriptionType": "max"
            }),
            0,
        ),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&body).unwrap_or_default()
    );
    code
}

fn claude_turn(args: &[String]) -> i32 {
    record_invocation(args);
    if flag(args, "--output-format").as_deref() != Some("stream-json") {
        eprintln!("fake claude: expected --output-format stream-json");
        return 2;
    }
    // Like the real CLI, which lists its choices.
    if let Some(effort) = flag(args, "--effort") {
        if !["low", "medium", "high", "xhigh", "max"].contains(&effort.as_str()) {
            eprintln!("error: option '--effort <level>' argument '{effort}' is invalid.");
            return 1;
        }
    }
    let prompt = read_prompt();
    let cwd = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let id = if let Some(id) = flag(args, "--resume") {
        match load_session(&id) {
            Some(s) if s["cwd"] == json!(cwd) => id,
            _ => {
                eprintln!("No conversation found with session ID: {id}");
                return 1;
            }
        }
    } else if let Some(id) = flag(args, "--session-id") {
        if load_session(&id).is_some() {
            eprintln!("Error: Session ID {id} is already in use.");
            return 1;
        }
        id
    } else {
        format!("{:032x}", std::process::id())
    };
    let model = flag(args, "--model").unwrap_or_else(|| "fake-claude-model".into());
    let (noted, prompt) = strip_note(&prompt);
    let (mut mode, said) = view(&prompt);
    if let Mode::Worker { granted, .. } = &mut mode {
        *granted = noted;
    }

    if said.contains("[malformed]") {
        raw("<html>502 Bad Gateway</html>");
        raw("this is not json");
        return 0;
    }
    let first = first_prompt(&id).unwrap_or_else(|| said.clone());
    let (n, previous) = remember(&id, &said);
    let key_source = if auth_has("stream-api-key") {
        "ANTHROPIC_API_KEY"
    } else {
        "none"
    };
    let mut init = json!({
        "type": "system", "subtype": "init", "session_id": id, "model": model,
        "apiKeySource": key_source, "tools": [], "cwd": cwd, "claude_code_version": "2.1.999"
    });
    if auth_has("no-key-source") {
        init.as_object_mut().map(|o| o.remove("apiKeySource"));
    }
    out(&init);
    let delta = |text: &str| {
        out(&json!({
            "type": "stream_event", "session_id": id,
            "event": { "type": "content_block_delta", "index": 0,
                       "delta": { "type": "text_delta", "text": text } }
        }))
    };
    let result_error = |text: &str| {
        out(&json!({
            "type": "result", "subtype": "success", "is_error": true, "result": text,
            "session_id": id, "duration_ms": 5, "num_turns": 1
        }));
        1
    };
    if key_source != "none" {
        // A real CLI would now call the API; wait so Plenipo has to stop it.
        slow_ticks(|_| {});
        return 0;
    }
    if said.contains("[crash]") {
        delta("Starting");
        eprintln!("fatal: simulated crash");
        return 70;
    }
    if said.contains("[usage-limit]") {
        return result_error("Claude AI usage limit reached|1760000000");
    }
    if said.contains("[auth-expired]") {
        return result_error("OAuth token has expired. Please run /login");
    }
    if said.contains("[offline]") {
        return result_error("API Error: Connection error.");
    }
    if said.contains("[slow]") {
        slow_ticks(|i| delta(&format!("tick {i} ")));
        return 0;
    }
    if said.contains("[unknown]") {
        out(&json!({ "type": "rate_limit_event", "info": {} }));
        out(&json!({ "type": "system", "subtype": "compact_boundary" }));
    }
    delay(&said);
    let calls = tool_calls(&said);
    let list = said.contains("[tools-list]");
    let mut used = Vec::new();
    if !calls.is_empty() || list {
        // Like the real CLI: only servers from --mcp-config, and only tools --allowedTools allows.
        let allowed = flag(args, "--allowedTools").as_deref() == Some("mcp__plenipo");
        let server = claude_server(args).filter(|_| allowed);
        let (names, outcomes) = use_tools(server.as_ref(), &calls, list);
        for (i, (name, input, text, is_error)) in outcomes.iter().enumerate() {
            let tool_id = format!("toolu_{i}");
            out(&json!({
                "type": "assistant", "session_id": id,
                "message": { "model": model, "role": "assistant", "content": [
                    { "type": "tool_use", "id": tool_id, "name": format!("mcp__plenipo__{name}"), "input": input }
                ] }
            }));
            out(&json!({
                "type": "user", "session_id": id,
                "message": { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": tool_id, "is_error": is_error, "content": text }
                ] }
            }));
        }
        used = tool_lines(&names, list, &outcomes);
    }
    let mut text = if said.contains("[big]") {
        "B".repeat(1024 * 1024)
    } else {
        answer(n, &mode, &said, previous.as_deref(), &first)
    };
    if !used.is_empty() {
        text = format!("{text}\n{}", used.join("\n"));
    }
    for chunk in text.as_bytes().chunks(16).take(8) {
        delta(&String::from_utf8_lossy(chunk));
        std::thread::sleep(Duration::from_millis(20));
    }
    out(&json!({
        "type": "assistant", "session_id": id,
        "message": { "model": model, "role": "assistant",
                     "content": [{ "type": "text", "text": text }] }
    }));
    out(&json!({
        "type": "result", "subtype": "success", "is_error": false, "result": text,
        "session_id": id, "duration_ms": 1234, "num_turns": 1, "total_cost_usd": 0.01,
        "usage": { "input_tokens": 12, "cache_creation_input_tokens": 3,
                   "cache_read_input_tokens": 5, "output_tokens": 7 }
    }));
    0
}

// ---- Codex --------------------------------------------------------------------------------

fn codex(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("codex-cli 0.99.0");
            0
        }
        Some("login") if args.get(1).map(String::as_str) == Some("status") => codex_auth(),
        Some("exec") => codex_turn(args),
        _ => {
            eprintln!("fake codex: unsupported arguments {args:?}");
            2
        }
    }
}

fn codex_auth() -> i32 {
    match auth_mode() {
        "signed-out" => {
            eprintln!("Not logged in");
            1
        }
        "api-key" => {
            eprintln!("Logged in using an API key - sk-proj-***ABCDE");
            0
        }
        "unknown-status" => {
            eprintln!("error: unrecognized subcommand 'status'");
            2
        }
        _ => {
            eprintln!("Logged in using ChatGPT");
            0
        }
    }
}

fn codex_turn(args: &[String]) -> i32 {
    record_invocation(args);
    if !args.iter().any(|a| a == "--json")
        || flag(args, "--sandbox").as_deref() != Some("read-only")
    {
        eprintln!("fake codex: expected --json and --sandbox read-only");
        return 2;
    }
    let prompt = read_prompt();
    let id = match args.iter().position(|a| a == "resume") {
        Some(i) => {
            let Some(id) = args.get(i + 1) else {
                eprintln!("fake codex: resume needs a thread id");
                return 2;
            };
            if load_session(id).is_none() {
                eprintln!("Error: thread not found: {id}");
                return 1;
            }
            id.clone()
        }
        None => format!("thread-{:08x}-{}", std::process::id(), prompt.len()),
    };
    let (noted, prompt) = strip_note(&prompt);
    let (mut mode, said) = view(&prompt);
    if let Mode::Worker { granted, .. } = &mut mode {
        *granted = noted;
    }
    if said.contains("[malformed]") {
        raw("Reading prompt from stdin...");
        raw("{not json at all");
        return 0;
    }
    let first = first_prompt(&id).unwrap_or_else(|| said.clone());
    let (n, previous) = remember(&id, &said);
    out(&json!({ "type": "thread.started", "thread_id": id }));
    out(&json!({ "type": "turn.started" }));
    let failed = |message: &str| {
        out(&json!({ "type": "error", "message": message }));
        out(&json!({ "type": "turn.failed", "error": { "message": message } }));
        1
    };
    if said.contains("[crash]") {
        eprintln!("thread 'main' panicked at codex-rs/core/src/fake.rs:1:1");
        return 101;
    }
    if said.contains("[usage-limit]") {
        return failed("You've hit your usage limit. Upgrade to Pro or try again later.");
    }
    if said.contains("[auth-expired]") {
        return failed(
            "unexpected status 401 Unauthorized: token expired, please run `codex login`",
        );
    }
    if said.contains("[offline]") {
        return failed("stream disconnected before completion: error sending request");
    }
    if said.contains("[slow]") {
        slow_ticks(|i| {
            out(&json!({ "type": "item.completed",
                         "item": { "id": format!("r{i}"), "type": "reasoning", "text": format!("tick {i}") } }))
        });
        return 0;
    }
    if said.contains("[unknown]") {
        out(&json!({ "type": "session.configured", "model": "x" }));
    }
    delay(&said);
    out(&json!({ "type": "item.started",
                 "item": { "id": "item_0", "type": "command_execution", "command": "bash -lc ls",
                           "aggregated_output": "", "exit_code": null, "status": "in_progress" } }));
    out(&json!({ "type": "item.completed",
                 "item": { "id": "item_0", "type": "command_execution", "command": "bash -lc ls",
                           "aggregated_output": "", "exit_code": 0, "status": "completed" } }));
    let calls = tool_calls(&said);
    let list = said.contains("[tools-list]");
    let mut used = Vec::new();
    if !calls.is_empty() || list {
        let server = codex_server(args);
        let (names, outcomes) = use_tools(server.as_ref(), &calls, list);
        for (i, (name, input, text, is_error)) in outcomes.iter().enumerate() {
            let item = |status: &str| {
                json!({
                    "id": format!("mcp_{i}"), "type": "mcp_tool_call", "server": "plenipo",
                    "tool": name, "arguments": input, "status": status,
                    "result": { "content": [{ "type": "text", "text": text }] }
                })
            };
            out(&json!({ "type": "item.started", "item": item("in_progress") }));
            out(&json!({ "type": "item.completed",
                         "item": item(if *is_error { "failed" } else { "completed" }) }));
        }
        used = tool_lines(&names, list, &outcomes);
    }
    let mut text = if said.contains("[big]") {
        "B".repeat(1024 * 1024)
    } else {
        answer(n, &mode, &said, previous.as_deref(), &first)
    };
    if !used.is_empty() {
        text = format!("{text}\n{}", used.join("\n"));
    }
    out(&json!({ "type": "item.completed",
                 "item": { "id": "item_1", "type": "agent_message", "text": text } }));
    out(&json!({ "type": "turn.completed",
                 "usage": { "input_tokens": 20, "cached_input_tokens": 8, "output_tokens": 9 } }));
    0
}
