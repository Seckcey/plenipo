//! Add-on programs (Phase 20 part 20C; ADR-066, ADR-071): Plenipo's side of talking to a program
//! the owner added, which offers tools over MCP on its standard input and output.
//!
//! The program runs through the supervisor as an approved program (ADR-005, ADR-034): its own
//! process tree, a cleared environment plus only the stored secrets the owner named for it
//! (ADR-048), and no shell. Plenipo is the MCP client: `initialize` (30 seconds to answer), then
//! `tools/list` or `tools/call` (10 minutes), one JSON-RPC message per line. Anything the program
//! asks of Plenipo (to sample a model, to list folders, to ask the owner) is refused: it gets only
//! the call it was sent. What it answers is the program's words: size-capped here, then fenced
//! and redacted by the broker before a worker sees it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use plenipo_guard::add_ons::Listed;
use plenipo_runtime::{LaunchSpec, OutputLine, OutputStream, StdinFeed, Supervisor};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};

/// How long a program has to start and answer `initialize`.
pub const START_WAIT: Duration = Duration::from_secs(30);
/// How long a tool may take.
pub const CALL_WAIT: Duration = Duration::from_secs(10 * 60);
/// How long listing its tools may take.
const LIST_WAIT: Duration = Duration::from_secs(60);
/// The longest one message from the program may be.
const MAX_LINE: usize = 4 * 1024 * 1024;
/// The most text of one tool's answer a worker gets (ADR-066 §4).
pub const MAX_ANSWER_TEXT: usize = 64 * 1024;
/// The most a worker's arguments to a tool may be.
pub const MAX_ARGUMENTS: usize = 64 * 1024;
/// The most lines that are not the program's answers (its log lines on standard output) read
/// while waiting for one answer.
const MAX_OTHER_LINES: usize = 10_000;
/// The MCP version Plenipo asks for first.
pub const PROTOCOL: &str = "2025-06-18";
/// Versions Plenipo can speak (a program may answer with an older one).
const PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// The longest a session lives: a worker's step.
const MAX_LIFE: Duration = Duration::from_secs(12 * 60 * 60);
/// The most lines of the program's standard output waiting to be read; more are dropped (its
/// log lines on standard error never wait).
const MAX_WAITING_LINES: usize = 256;

/// A tool's answer, cut to size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    /// The program said the call failed.
    pub failed: bool,
    /// Pictures, sounds, or files it sent, which workers do not get from add-ons.
    pub left_out: usize,
    /// Its text was cut to [`MAX_ANSWER_TEXT`].
    pub cut: bool,
}

/// One running add-on program, for one worker's step (or one look at its tools).
pub struct Session {
    supervisor: Supervisor,
    execution: String,
    input: std::sync::Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    /// Held for a whole request, so two calls at once never read each other's answers.
    output: Mutex<mpsc::Receiver<OutputLine>>,
    next: AtomicU64,
    /// Its own folder, made for this session and removed when it stops.
    folder: PathBuf,
}

/// What starting a program needs.
pub struct Start<'a> {
    /// The add-on's name, for the supervisor's list of running programs.
    pub name: &'a str,
    pub program: &'a Path,
    pub args: &'a [String],
    /// Only the named stored secrets, as their variables.
    pub env: Vec<(String, String)>,
    pub working_dir: &'a Path,
}

/// A JSON-RPC error from a tool call, as a failed answer: its message is the program's words, so
/// it reaches the worker fenced like any answer, and is never recorded.
fn rpc_error(e: &Value) -> Value {
    let message: String = e["message"]
        .as_str()
        .unwrap_or("an error")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect();
    json!({ "isError": true, "content": [{ "type": "text", "text": message }] })
}

impl Session {
    /// Start the program and say hello (`initialize`), within [`START_WAIT`].
    pub async fn start(supervisor: &Supervisor, s: Start<'_>) -> Result<Self, String> {
        let executable: PathBuf = supervisor
            .allow_executable(s.program)
            .map_err(|e| format!("{} cannot be run: {e}", s.program.display()))?;
        let (input, feed) = StdinFeed::new();
        let (tx, mut all) = mpsc::unbounded_channel::<OutputLine>();
        // Only standard output waits to be read, and only so much: a program that prints while
        // idle cannot fill Plenipo's memory.
        let (keep, rx) = mpsc::channel(MAX_WAITING_LINES);
        tokio::spawn(async move {
            while let Some(line) = all.recv().await {
                if line.stream == OutputStream::Stdout {
                    let _ = keep.try_send(line);
                }
            }
        });
        let record = supervisor
            .launch(LaunchSpec {
                profile_id: "capability.add-on".into(),
                label: format!("Add-on tools: {}", s.name),
                executable,
                args: s.args.to_vec(),
                env: s.env,
                working_dir: s.working_dir.to_path_buf(),
                max_runtime: MAX_LIFE,
                stdin: None,
                stdin_feed: Some(feed),
                max_line_bytes: Some(MAX_LINE),
                observer: Some(tx),
                agent: None,
                extra_pipes: None,
            })
            .await
            .map_err(|e| format!("the program could not be started: {e}"))?;
        let session = Self {
            supervisor: supervisor.clone(),
            execution: record.id,
            input: std::sync::Mutex::new(Some(input)),
            output: Mutex::new(rx),
            next: AtomicU64::new(1),
            folder: s.working_dir.to_path_buf(),
        };
        let hello = session
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL,
                    "capabilities": {},
                    "clientInfo": { "name": "Plenipo", "version": env!("CARGO_PKG_VERSION") },
                }),
                START_WAIT,
            )
            .await;
        let hello = match hello {
            Ok(h) => h,
            Err(e) => {
                session.stop().await;
                return Err(e);
            }
        };
        let version = hello["protocolVersion"].as_str().unwrap_or_default();
        if !PROTOCOLS.contains(&version) {
            session.stop().await;
            return Err(format!(
                "the program speaks a version of MCP Plenipo does not know ({})",
                version.chars().take(20).collect::<String>()
            ));
        }
        if hello["capabilities"]["tools"].is_null() {
            session.stop().await;
            return Err("the program offers no tools".into());
        }
        session.notify("notifications/initialized", json!({}));
        Ok(session)
    }

    fn send(&self, message: &Value) -> Result<(), String> {
        let mut line = serde_json::to_vec(message).map_err(|e| e.to_string())?;
        line.push(b'\n');
        self.input
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .ok_or("the program was stopped")?
            .send(line)
            .map_err(|_| "the program stopped".to_owned())
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// One request, and its answer within `wait`. What the program asks of Plenipo meanwhile is
    /// refused; its notifications and log lines are passed over.
    async fn request(&self, method: &str, params: Value, wait: Duration) -> Result<Value, String> {
        // The wait covers waiting for another call to finish too.
        let read = async {
            let mut output = self.output.lock().await;
            let id = self.next.fetch_add(1, Ordering::Relaxed);
            self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
            let mut other = 0usize;
            loop {
                let Some(line) = output.recv().await else {
                    return Err("the program stopped before it answered".to_owned());
                };
                if line.stream != OutputStream::Stdout {
                    continue;
                }
                if line.truncated {
                    return Err("the program's answer was too big".to_owned());
                }
                let Ok(message) = serde_json::from_str::<Value>(line.text.trim()) else {
                    other += 1;
                    if other > MAX_OTHER_LINES {
                        return Err("the program wrote too much that was not MCP".to_owned());
                    }
                    continue;
                };
                let has_method = message["method"].is_string();
                match (&message["id"], has_method) {
                    // The program asks Plenipo something: refused, whatever it is.
                    (asked, true) if !asked.is_null() => {
                        let _ = self.send(&json!({
                            "jsonrpc": "2.0",
                            "id": asked,
                            "error": { "code": -32601, "message": "Plenipo does not offer this to add-on programs." },
                        }));
                    }
                    // A notification: passed over.
                    (_, true) => {}
                    (answer, false) if answer.as_u64() == Some(id) => {
                        if !message["error"].is_null() {
                            return if method == "tools/call" {
                                Ok(rpc_error(&message["error"]))
                            } else {
                                Err("the program answered with an error".to_owned())
                            };
                        }
                        return Ok(message["result"].clone());
                    }
                    _ => {}
                }
            }
        };
        match tokio::time::timeout(wait, read).await {
            Ok(r) => r,
            Err(_) => Err(format!(
                "the program did not answer within {} seconds",
                wait.as_secs()
            )),
        }
    }

    /// Its tools (`tools/list`, page by page, at most 5 pages).
    pub async fn tools(&self) -> Result<Vec<Listed>, String> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..5 {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let page = self.request("tools/list", params, LIST_WAIT).await?;
            for t in page["tools"].as_array().cloned().unwrap_or_default() {
                let Some(name) = t["name"].as_str() else {
                    continue;
                };
                // MCP's own rule for a tool's name (it is shown in Plenipo's own lines), and an
                // input small enough to be kept whole and compared: other tools are left out.
                if name.is_empty()
                    || name.len() > 128
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    || serde_json::to_vec(&t["inputSchema"]).map_or(usize::MAX, |b| b.len())
                        > plenipo_guard::add_ons::MAX_INPUT_BYTES
                {
                    continue;
                }
                let hint = |k: &str| t["annotations"][k].as_bool();
                out.push(Listed {
                    name: name.to_owned(),
                    description: t["description"].as_str().unwrap_or_default().to_owned(),
                    input: t["inputSchema"].clone(),
                    read_only_hint: hint("readOnlyHint"),
                    destructive_hint: hint("destructiveHint"),
                });
            }
            cursor = page["nextCursor"]
                .as_str()
                .filter(|c| !c.is_empty() && c.len() <= 1000)
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    /// Call one of its tools (`tools/call`) with the worker's arguments, within [`CALL_WAIT`].
    pub async fn call(&self, name: &str, arguments: Value) -> Result<Answer, String> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
                CALL_WAIT,
            )
            .await?;
        Ok(answer_of(&result))
    }

    /// Stop the program: its input closes, then its process tree ends, and its folder goes.
    pub async fn stop(&self) {
        self.input.lock().unwrap_or_else(|p| p.into_inner()).take();
        let _ = self.supervisor.terminate(&self.execution).await;
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

/// A session dropped without [`Session::stop`] (its step ended while it was starting) still ends
/// its program and removes its folder.
impl Drop for Session {
    fn drop(&mut self) {
        if self
            .input
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .is_none()
        {
            return;
        }
        let supervisor = self.supervisor.clone();
        let execution = self.execution.clone();
        let folder = self.folder.clone();
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            rt.spawn(async move {
                let _ = supervisor.terminate(&execution).await;
                let _ = std::fs::remove_dir_all(folder);
            });
        }
    }
}

/// A `tools/call` result as text for the worker: its text parts, in order, cut to size; any
/// picture, sound, or file only counted.
pub fn answer_of(result: &Value) -> Answer {
    let mut text = String::new();
    let mut left_out = 0;
    for part in result["content"].as_array().cloned().unwrap_or_default() {
        match part["type"].as_str() {
            Some("text") => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(part["text"].as_str().unwrap_or_default());
            }
            _ => left_out += 1,
        }
    }
    if text.is_empty() && result["structuredContent"].is_object() {
        text = serde_json::to_string_pretty(&result["structuredContent"]).unwrap_or_default();
    }
    let cut = text.len() > MAX_ANSWER_TEXT;
    if cut {
        let mut end = MAX_ANSWER_TEXT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    Answer {
        text,
        failed: result["isError"].as_bool() == Some(true),
        left_out,
        cut,
    }
}

/// A worker's arguments to an add-on tool, checked: an object (or nothing), at most
/// [`MAX_ARGUMENTS`] bytes, at most 20 levels deep, and only the names the tool's input lists
/// when it lists them and allows no others.
pub fn check_arguments(args: &Value, input: &Value) -> Result<Value, String> {
    let args = match args {
        Value::Null => json!({}),
        Value::Object(_) => args.clone(),
        _ => return Err("the arguments must be an object".into()),
    };
    if serde_json::to_vec(&args).map_or(usize::MAX, |b| b.len()) > MAX_ARGUMENTS {
        return Err(format!("the arguments are over {MAX_ARGUMENTS} bytes"));
    }
    fn depth(v: &Value) -> usize {
        match v {
            Value::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
            Value::Object(o) => 1 + o.values().map(depth).max().unwrap_or(0),
            _ => 0,
        }
    }
    if depth(&args) > 20 {
        return Err("the arguments are too deep".into());
    }
    if input["additionalProperties"] == json!(false) {
        if let (Some(given), Some(known)) = (args.as_object(), input["properties"].as_object()) {
            if let Some(k) = given.keys().find(|k| !known.contains_key(*k)) {
                return Err(format!("\"{k}\" is not one of this tool's arguments"));
            }
        }
    }
    if let (Some(required), Some(given)) = (input["required"].as_array(), args.as_object()) {
        if let Some(missing) = required
            .iter()
            .filter_map(Value::as_str)
            .find(|r| !given.contains_key(*r))
        {
            return Err(format!("\"{missing}\" is required"));
        }
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_text_only_and_cut_to_size() {
        let a = answer_of(&json!({ "content": [
            { "type": "text", "text": "Order 1042 is shipped." },
            { "type": "image", "data": "iVBOR", "mimeType": "image/png" },
            { "type": "text", "text": "Tracking: 1Z999" }
        ] }));
        assert_eq!(a.text, "Order 1042 is shipped.\nTracking: 1Z999");
        assert_eq!(a.left_out, 1);
        assert!(!a.failed && !a.cut);
        let big = answer_of(
            &json!({ "content": [{ "type": "text", "text": "é".repeat(MAX_ANSWER_TEXT) }], "isError": true }),
        );
        assert!(big.cut && big.failed);
        assert!(big.text.len() <= MAX_ANSWER_TEXT);
        let structured = answer_of(&json!({ "content": [], "structuredContent": { "ok": true } }));
        assert!(structured.text.contains("\"ok\": true"));
    }

    #[test]
    fn arguments_are_checked_against_the_tools_input() {
        let input = json!({ "type": "object", "properties": { "order": { "type": "string" } },
            "required": ["order"], "additionalProperties": false });
        assert!(check_arguments(&json!({ "order": "1042" }), &input).is_ok());
        assert!(
            check_arguments(&json!({ "order": "1", "all": true }), &input)
                .unwrap_err()
                .contains("\"all\"")
        );
        assert!(check_arguments(&json!({}), &input)
            .unwrap_err()
            .contains("required"));
        assert!(check_arguments(&json!(["x"]), &input).is_err());
        let loose = json!({ "type": "object" });
        assert_eq!(check_arguments(&Value::Null, &loose).unwrap(), json!({}));
        let mut deep = json!(1);
        for _ in 0..25 {
            deep = json!([deep]);
        }
        assert!(check_arguments(&json!({ "x": deep }), &loose).is_err());
        let huge = json!({ "x": "a".repeat(MAX_ARGUMENTS) });
        assert!(check_arguments(&huge, &loose).is_err());
    }
}
