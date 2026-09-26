//! Test double for the Claude Code and Codex CLIs (ADR-007). Never shipped.
//!
//! Copy or link this binary as `claude` / `codex` (`.exe` on Windows); it answers like the
//! real CLI named by its file stem: `--version`, the sign-in status command, and one turn in
//! the provider's JSON-lines stream format with the prompt read from stdin.
//!
//! State lives in `<HOME or USERPROFILE>/.plenipo-fake-agent/`:
//! - `auth` (optional): `subscription` (default), `api-key`, `signed-out`, `cloud`,
//!   `unknown-status`, or `stream-api-key` (status says subscription; Claude's stream reports
//!   an API key);
//! - `sessions/<id>.json`: prompts per session, so resume can be verified;
//! - `last-args.json`, `last-env.txt`: what the last turn received.
//!
//! Markers in the prompt pick a behavior: `[crash]`, `[malformed]`, `[usage-limit]`,
//! `[auth-expired]`, `[offline]`, `[slow]`, `[unknown]`, `[big]`.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let persona = args
        .first()
        .and_then(|a| Path::new(a).file_stem())
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let rest = &args[1..];
    let code = match persona.as_str() {
        "claude" => claude(rest),
        "codex" => codex(rest),
        other => {
            eprintln!(
                "plenipo-fake-agent: unknown persona {other:?} (name the file claude or codex)"
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

fn auth_mode() -> String {
    std::fs::read_to_string(state_dir().join("auth"))
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "subscription".into())
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

fn slow_ticks(mut tick: impl FnMut(u32)) {
    for i in 1..=300 {
        tick(i);
        std::thread::sleep(Duration::from_millis(200));
    }
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
    let (body, code) = match auth_mode().as_str() {
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

    if prompt.contains("[malformed]") {
        raw("<html>502 Bad Gateway</html>");
        raw("this is not json");
        return 0;
    }
    let (n, previous) = remember(&id, &prompt);
    let key_source = if auth_mode() == "stream-api-key" {
        "ANTHROPIC_API_KEY"
    } else {
        "none"
    };
    out(&json!({
        "type": "system", "subtype": "init", "session_id": id, "model": model,
        "apiKeySource": key_source, "tools": [], "cwd": cwd, "claude_code_version": "2.1.999"
    }));
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
    if prompt.contains("[crash]") {
        delta("Starting");
        eprintln!("fatal: simulated crash");
        return 70;
    }
    if prompt.contains("[usage-limit]") {
        return result_error("Claude AI usage limit reached|1760000000");
    }
    if prompt.contains("[auth-expired]") {
        return result_error("OAuth token has expired. Please run /login");
    }
    if prompt.contains("[offline]") {
        return result_error("API Error: Connection error.");
    }
    if prompt.contains("[slow]") {
        slow_ticks(|i| delta(&format!("tick {i} ")));
        return 0;
    }
    if prompt.contains("[unknown]") {
        out(&json!({ "type": "rate_limit_event", "info": {} }));
        out(&json!({ "type": "system", "subtype": "compact_boundary" }));
    }
    let text = if prompt.contains("[big]") {
        "B".repeat(1024 * 1024)
    } else {
        reply(n, &prompt, previous.as_deref())
    };
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
    match auth_mode().as_str() {
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
    if prompt.contains("[malformed]") {
        raw("Reading prompt from stdin...");
        raw("{not json at all");
        return 0;
    }
    let (n, previous) = remember(&id, &prompt);
    out(&json!({ "type": "thread.started", "thread_id": id }));
    out(&json!({ "type": "turn.started" }));
    let failed = |message: &str| {
        out(&json!({ "type": "error", "message": message }));
        out(&json!({ "type": "turn.failed", "error": { "message": message } }));
        1
    };
    if prompt.contains("[crash]") {
        eprintln!("thread 'main' panicked at codex-rs/core/src/fake.rs:1:1");
        return 101;
    }
    if prompt.contains("[usage-limit]") {
        return failed("You've hit your usage limit. Upgrade to Pro or try again later.");
    }
    if prompt.contains("[auth-expired]") {
        return failed(
            "unexpected status 401 Unauthorized: token expired, please run `codex login`",
        );
    }
    if prompt.contains("[offline]") {
        return failed("stream disconnected before completion: error sending request");
    }
    if prompt.contains("[slow]") {
        slow_ticks(|i| {
            out(&json!({ "type": "item.completed",
                         "item": { "id": format!("r{i}"), "type": "reasoning", "text": format!("tick {i}") } }))
        });
        return 0;
    }
    if prompt.contains("[unknown]") {
        out(&json!({ "type": "session.configured", "model": "x" }));
    }
    out(&json!({ "type": "item.started",
                 "item": { "id": "item_0", "type": "command_execution", "command": "bash -lc ls",
                           "aggregated_output": "", "exit_code": null, "status": "in_progress" } }));
    out(&json!({ "type": "item.completed",
                 "item": { "id": "item_0", "type": "command_execution", "command": "bash -lc ls",
                           "aggregated_output": "", "exit_code": 0, "status": "completed" } }));
    let text = if prompt.contains("[big]") {
        "B".repeat(1024 * 1024)
    } else {
        reply(n, &prompt, previous.as_deref())
    };
    out(&json!({ "type": "item.completed",
                 "item": { "id": "item_1", "type": "agent_message", "text": text } }));
    out(&json!({ "type": "turn.completed",
                 "usage": { "input_tokens": 20, "cached_input_tokens": 8, "output_tokens": 9 } }));
    0
}
