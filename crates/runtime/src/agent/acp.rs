//! The shared ACP driver (ADR-015): one task over the Agent Client Protocol.
//!
//! ACP is JSON-RPC 2.0, one message per line, on the AI tool's stdin and stdout. Plenipo is the
//! client. A task is one supervised process, and the whole exchange happens in it:
//!
//! 1. `initialize` (protocol version 1; Plenipo offers no file or terminal access of its own);
//! 2. `session/new`, or `session/resume` / `session/load` for a follow-up, with Plenipo's tool
//!    server when the worker has permissions;
//! 3. `session/prompt` with the task text, once the conversation ID is known; the tool streams
//!    `session/update` notifications and may ask `session/request_permission`;
//! 4. the `session/prompt` answer ends the task, and stdin is closed so the tool exits.
//!
//! Plenipo answers every permission request itself: a call to its own tool server is allowed
//! (Guard decides inside the call), anything else is refused. It never sends `authenticate`,
//! which could start a sign-in in a browser; a tool that is not signed in is reported as such.
//! Unknown messages are ignored and counted (ADR-007 §7).

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::agent::adapter::{
    cap, first_line, tool_summary, Parsed, ProcessEnd, ProviderSession, Stop, TurnParser,
    TurnState, MAX_EVENT_TEXT,
};
use crate::agent::dto::{AgentEvent, NoticeLevel, TurnOutcome, TurnResult};
use crate::agent::tools::ToolServer;
use crate::dto::TokenUsage;

/// The ACP protocol version Plenipo speaks.
pub const PROTOCOL_VERSION: u64 = 1;

/// Request IDs Plenipo uses; one of each per task.
const INITIALIZE: u64 = 1;
const OPEN: u64 = 2;
const PROMPT: u64 = 3;

/// What an ACP adapter tells the driver about one task.
#[derive(Debug, Clone, Default)]
pub struct AcpTask {
    pub runtime_label: &'static str,
    pub session: ProviderSession,
    /// The conversation's folder (absolute), sent as the session's `cwd`.
    pub working_dir: PathBuf,
    /// Plenipo's tool server for this step (Phase 7), if any.
    pub tools: Option<ToolServer>,
    /// The model Plenipo asked for, reported until the tool names one.
    pub model: Option<String>,
    /// Extra `_meta` fields for opening the session (for example an agent profile).
    pub session_meta: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before [`TurnParser::open`].
    Idle,
    Initializing,
    /// Opening or loading the conversation; updates now are history being replayed.
    Opening,
    Prompting,
    Done,
}

/// One task over ACP.
pub struct AcpTurn {
    task: AcpTask,
    state: TurnState,
    phase: Phase,
    prompt: String,
    /// Text of the assistant message being streamed.
    message: String,
    /// The tool's reason for ending the prompt (`end_turn`, `cancelled`, …).
    stop_reason: Option<String>,
    /// Plenipo asked the tool to stop.
    cancel_asked: bool,
}

impl AcpTurn {
    pub fn new(task: AcpTask) -> Self {
        let mut state = TurnState::new(task.runtime_label);
        state.model = task.model.clone();
        Self {
            task,
            state,
            phase: Phase::Idle,
            prompt: String::new(),
            message: String::new(),
            stop_reason: None,
            cancel_asked: false,
        }
    }

    fn label(&self) -> &'static str {
        self.task.runtime_label
    }

    /// The tool servers for the session: Plenipo's, when the worker has permissions.
    fn mcp_servers(&self) -> Value {
        match &self.task.tools {
            Some(t) => json!([{
                "name": t.name,
                "command": t.command.display().to_string(),
                "args": t.args,
                "env": [],
            }]),
            None => json!([]),
        }
    }

    fn with_meta(&self, mut params: Value) -> Value {
        if let Some(meta) = &self.task.session_meta {
            params["_meta"] = meta.clone();
        }
        params
    }

    /// End of the stream of the current message: record it.
    fn flush_message(&mut self) -> Option<AgentEvent> {
        let text = std::mem::take(&mut self.message);
        if text.trim().is_empty() {
            return None;
        }
        self.state.last_message = Some(text.clone());
        Some(AgentEvent::Message {
            text: cap(&text, MAX_EVENT_TEXT),
        })
    }

    /// The tool cannot go on: record why and let it exit.
    fn fail(&mut self, error: String) -> Parsed {
        self.state.error = Some(error);
        self.phase = Phase::Done;
        Parsed {
            close_input: true,
            ..Parsed::none()
        }
    }

    fn initialized(&mut self, result: &Value) -> Parsed {
        let version = result.get("protocolVersion").and_then(Value::as_u64);
        if version != Some(PROTOCOL_VERSION) {
            let reason = format!(
                "{} speaks ACP version {}; Plenipo speaks version {PROTOCOL_VERSION}.",
                self.label(),
                version.map_or_else(|| "(none)".to_owned(), |v| v.to_string())
            );
            let stop = Stop {
                outcome: TurnOutcome::MalformedOutput,
                reason,
            };
            self.state.stop = Some(stop.clone());
            self.phase = Phase::Done;
            return Parsed {
                stop: Some(stop),
                close_input: true,
                ..Parsed::none()
            };
        }
        let caps = result.get("agentCapabilities");
        let can = |pointer: &str| {
            caps.and_then(|c| c.pointer(pointer))
                .is_some_and(|v| !v.is_null() && v != &json!(false))
        };
        let cwd = self.task.working_dir.display().to_string();
        let (method, params) = match &self.task.session {
            ProviderSession::New { .. } => (
                "session/new",
                json!({ "cwd": cwd, "mcpServers": self.mcp_servers() }),
            ),
            // `session/resume` does not replay the history; `session/load` does.
            ProviderSession::Resume { id } if can("/sessionCapabilities/resume") => (
                "session/resume",
                json!({ "sessionId": id, "cwd": cwd, "mcpServers": self.mcp_servers() }),
            ),
            ProviderSession::Resume { id } if can("/loadSession") => (
                "session/load",
                json!({ "sessionId": id, "cwd": cwd, "mcpServers": self.mcp_servers() }),
            ),
            ProviderSession::Resume { .. } => {
                return self.fail(format!(
                    "{} cannot resume a conversation over ACP.",
                    self.label()
                ))
            }
        };
        self.phase = Phase::Opening;
        Parsed {
            send: vec![request(OPEN, method, self.with_meta(params))],
            ..Parsed::none()
        }
    }

    fn opened(&mut self, result: &Value) -> Parsed {
        let id = match &self.task.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { .. } => result
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        let Some(id) = id.filter(|i| !i.trim().is_empty()) else {
            return self.fail(format!(
                "{} did not say which conversation it opened.",
                self.label()
            ));
        };
        let model = ["/models/currentModelId", "/_meta/modelState/currentModelId"]
            .iter()
            .find_map(|p| result.pointer(p).and_then(Value::as_str))
            .map(str::to_owned);
        if model.is_some() {
            self.state.model = model;
        }
        self.state.provider_session_id = Some(id.clone());
        self.phase = Phase::Prompting;
        let prompt = request(
            PROMPT,
            "session/prompt",
            json!({ "sessionId": id, "prompt": [{ "type": "text", "text": self.prompt }] }),
        );
        Parsed {
            events: vec![AgentEvent::SessionStarted {
                provider_session_id: Some(id),
                model: self.state.model.clone(),
            }],
            send: vec![prompt],
            ..Parsed::none()
        }
    }

    fn prompted(&mut self, result: &Value) -> Parsed {
        let mut parsed = Parsed {
            close_input: true,
            ..Parsed::none()
        };
        parsed.events.extend(self.flush_message());
        let reason = result
            .get("stopReason")
            .and_then(Value::as_str)
            .unwrap_or("end_turn")
            .to_owned();
        if let Some(usage) = usage(result) {
            self.state.usage = Some(usage);
            parsed.events.push(AgentEvent::Usage { usage });
        }
        match reason.as_str() {
            "cancelled" => {}
            "refusal" => {
                self.state.error = Some(format!("{} refused the request.", self.label()));
            }
            "max_tokens" | "max_turn_requests" => {
                self.state.completed = true;
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: format!("{} stopped early ({reason}).", self.label()),
                });
            }
            _ => self.state.completed = true,
        }
        self.stop_reason = Some(reason);
        self.phase = Phase::Done;
        parsed
    }

    fn response(&mut self, id: Option<u64>, message: &Value) -> Parsed {
        if let Some(error) = message.get("error") {
            let text = error_text(error);
            return match id {
                Some(INITIALIZE | OPEN | PROMPT) => {
                    let mut parsed = self.fail(text);
                    parsed.events.extend(self.flush_message());
                    parsed
                }
                _ => {
                    self.state.unknown += 1;
                    Parsed::none()
                }
            };
        }
        let result = message.get("result").cloned().unwrap_or(Value::Null);
        match (id, self.phase) {
            (Some(INITIALIZE), Phase::Initializing) => self.initialized(&result),
            (Some(OPEN), Phase::Opening) => self.opened(&result),
            (Some(PROMPT), Phase::Prompting) => self.prompted(&result),
            _ => {
                self.state.unknown += 1;
                Parsed::none()
            }
        }
    }

    /// A request from the tool, which must be answered.
    fn request_from_tool(&mut self, id: &Value, method: &str, params: &Value) -> Parsed {
        if method != "session/request_permission" {
            // Plenipo offers no file, terminal, or other client methods.
            return Parsed {
                send: vec![json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                })
                .to_string()],
                ..Parsed::none()
            };
        }
        let call = params.get("toolCall").unwrap_or(&Value::Null);
        let allowed = self
            .task
            .tools
            .as_ref()
            .is_some_and(|t| is_tool_server_call(call, &t.name));
        let wanted: &[&str] = if allowed {
            &["allow_once", "allow_always"]
        } else {
            &["reject_once", "reject_always"]
        };
        let options = params.get("options").and_then(Value::as_array);
        let choice = wanted.iter().find_map(|kind| {
            options?
                .iter()
                .find(|o| o.get("kind").and_then(Value::as_str) == Some(kind))
                .and_then(|o| o.get("optionId").cloned())
        });
        let outcome = match choice {
            Some(option) => json!({ "outcome": "selected", "optionId": option }),
            None => json!({ "outcome": "cancelled" }),
        };
        let mut parsed = Parsed {
            send: vec![
                json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } }).to_string(),
            ],
            ..Parsed::none()
        };
        if !allowed {
            let what = call_name(call);
            parsed.events.push(AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: format!(
                    "{} asked to use {what}; Plenipo refused it (only Plenipo's own tools are \
                     allowed).",
                    self.label()
                ),
            });
        }
        parsed
    }

    fn update(&mut self, params: &Value) -> Parsed {
        let update = params.get("update").unwrap_or(&Value::Null);
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or("");
        if self.phase != Phase::Prompting {
            // History replayed while a conversation loads, or noise before the prompt.
            return Parsed::none();
        }
        let mut parsed = Parsed::none();
        match kind {
            "agent_message_chunk" => {
                if let Some(text) = update.pointer("/content/text").and_then(Value::as_str) {
                    self.message.push_str(text);
                    parsed
                        .events
                        .push(AgentEvent::TextDelta { text: text.into() });
                }
            }
            "tool_call" => {
                parsed.events.extend(self.flush_message());
                let input = update.get("rawInput").unwrap_or(&Value::Null);
                parsed.events.push(AgentEvent::ToolUse {
                    tool: call_name(update),
                    summary: tool_summary(input),
                });
            }
            "tool_call_update" => {
                let status = update.get("status").and_then(Value::as_str);
                if matches!(status, Some("completed" | "failed")) {
                    let text = update
                        .pointer("/content/0/content/text")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    parsed.events.push(AgentEvent::ToolResult {
                        tool: update
                            .get("title")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        is_error: status == Some("failed"),
                        summary: first_line(text, 200),
                    });
                }
            }
            "agent_thought_chunk"
            | "user_message_chunk"
            | "plan"
            | "available_commands_update"
            | "current_mode_update"
            | "config_option_update"
            | "session_info_update"
            | "usage_update" => {}
            _ => self.state.unknown += 1,
        }
        parsed
    }
}

impl TurnParser for AcpTurn {
    fn open(&mut self, prompt: &str) -> Option<Vec<String>> {
        self.prompt = prompt.to_owned();
        self.phase = Phase::Initializing;
        Some(vec![request(
            INITIALIZE,
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false,
                },
                "clientInfo": { "name": "plenipo", "version": env!("CARGO_PKG_VERSION") },
            }),
        )])
    }

    fn cancel(&mut self) -> Vec<String> {
        match (&self.phase, &self.state.provider_session_id) {
            (Phase::Prompting, Some(id)) => {
                self.cancel_asked = true;
                vec![json!({
                    "jsonrpc": "2.0", "method": "session/cancel",
                    "params": { "sessionId": id }
                })
                .to_string()]
            }
            _ => Vec::new(),
        }
    }

    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        if truncated {
            return self.state.malformed_line(true);
        }
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(false);
        };
        if !message.is_object() || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            self.state.unknown += 1;
            return Parsed::none();
        }
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id");
        let parsed = match (method, id) {
            (None, Some(id)) => self.response(id.as_u64(), &message),
            (Some(method), Some(id)) => {
                let params = message.get("params").unwrap_or(&Value::Null);
                self.request_from_tool(id, method, params)
            }
            (Some("session/update"), None) => {
                let params = message.get("params").unwrap_or(&Value::Null);
                self.update(params)
            }
            // The tool's own extensions (`_x.ai/…`, `x.ai/…`): progress and state, not events.
            (Some(m), None)
                if m.starts_with('_') || (m.contains('/') && !m.starts_with("session/")) =>
            {
                Parsed::none()
            }
            _ => {
                self.state.unknown += 1;
                return Parsed::none();
            }
        };
        self.state.understood += 1;
        parsed
    }

    fn stderr(&mut self, text: &str) {
        self.state.stderr(text);
    }

    fn finish(&mut self, end: &ProcessEnd) -> TurnResult {
        if !self.message.trim().is_empty() {
            self.state.last_message = Some(std::mem::take(&mut self.message));
        }
        let mut result = self.state.finish(end);
        let cancelled = self.stop_reason.as_deref() == Some("cancelled")
            || (self.cancel_asked && self.stop_reason.is_none());
        if cancelled
            && !matches!(
                result.outcome,
                TurnOutcome::TimedOut | TurnOutcome::Interrupted
            )
        {
            result.outcome = TurnOutcome::Cancelled;
            result.summary = "Cancelled".into();
            result.error = None;
        }
        result
    }
}

/// One JSON-RPC request line.
fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// A JSON-RPC error as one line: its message, and its data when that is text.
fn error_text(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("error");
    match error.get("data") {
        Some(Value::String(data)) if !data.trim().is_empty() => format!("{message}: {data}"),
        _ => message.to_owned(),
    }
}

/// A tool call's name, as the tool reports it.
fn call_name(call: &Value) -> String {
    ["/_meta/toolName", "/toolName", "/title", "/kind"]
        .iter()
        .find_map(|p| call.pointer(p).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
        .map_or_else(|| "a tool".to_owned(), |s| first_line(s, 80))
}

/// Whether a permission request is for a tool of the server named `server`: its tool name
/// carries the server's name as a prefix (`mcp__plenipo__read`, `plenipo__read`,
/// `plenipo:read`), or its input names the server (a generic "use a tool server's tool" call).
pub fn is_tool_server_call(call: &Value, server: &str) -> bool {
    let prefixed = |name: &str| {
        let name = name.strip_prefix("mcp__").unwrap_or(name);
        [
            format!("{server}__"),
            format!("{server}:"),
            format!("{server}/"),
        ]
        .iter()
        .any(|p| name.starts_with(p.as_str()))
    };
    let names = ["/_meta/toolName", "/toolName"]
        .iter()
        .filter_map(|p| call.pointer(p).and_then(Value::as_str));
    let input_server = ["server", "serverName", "server_name", "mcpServer"]
        .iter()
        .filter_map(|k| {
            call.pointer(&format!("/rawInput/{k}"))
                .and_then(Value::as_str)
        })
        .any(|s| s == server);
    input_server || names.into_iter().any(prefixed)
}

/// Token usage from a `session/prompt` answer (`usage` or `_meta.usage`, either naming).
fn usage(result: &Value) -> Option<TokenUsage> {
    let usage = result
        .get("usage")
        .or_else(|| result.pointer("/_meta/usage"))
        .filter(|u| u.is_object())?;
    let n = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| usage.get(*k).and_then(Value::as_u64))
            .unwrap_or(0)
    };
    let cached = n(&[
        "cachedReadTokens",
        "cacheReadInputTokens",
        "cache_read_input_tokens",
    ]);
    // Plenipo counts the whole prompt, cached parts included (as for Claude Code). ACP's
    // `inputTokens` already is that; a snake_case `input_tokens` is the uncached part only.
    let input = match usage.get("inputTokens").and_then(Value::as_u64) {
        Some(total) => total,
        None => n(&["input_tokens"]) + n(&["cache_creation_input_tokens"]) + cached,
    };
    Some(TokenUsage {
        input_tokens: input,
        cached_input_tokens: cached,
        output_tokens: n(&["outputTokens", "output_tokens"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ExecutionState;

    /// What Grok 1.0.41 answered over ACP, signed out (redacted), in order.
    const GROK_SIGNED_OUT: &str =
        include_str!("../../../../docs/phases/evidence/ai-tools-grok/acp-signed-out.agent.jsonl");

    fn grok_line(n: usize) -> &'static str {
        GROK_SIGNED_OUT.lines().nth(n).unwrap()
    }

    fn task(session: ProviderSession, tools: bool) -> AcpTask {
        AcpTask {
            runtime_label: "Grok",
            session,
            working_dir: PathBuf::from("/work/conversation"),
            tools: tools.then(|| ToolServer {
                name: "plenipo".into(),
                command: "/app/plenipo-desktop".into(),
                args: vec!["--plenipo-tools=ticket".into()],
                config_file: "/app/tools.json".into(),
                call_timeout: std::time::Duration::from_secs(60),
            }),
            model: Some("grok-4.6".into()),
            session_meta: Some(json!({ "agentProfile": { "tools": [] } })),
        }
    }

    fn new_session() -> ProviderSession {
        ProviderSession::New { preassigned: None }
    }

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(1),
        }
    }

    fn sent(parsed: &Parsed) -> Vec<Value> {
        parsed
            .send
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn notify(update: Value) -> String {
        json!({ "jsonrpc": "2.0", "method": "session/update",
                "params": { "sessionId": "s-1", "update": update } })
        .to_string()
    }

    fn answer(id: u64, result: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
    }

    /// Open a new session and get to the prompt (Grok's real `initialize` answer).
    fn prompting(tools: bool) -> AcpTurn {
        let mut t = AcpTurn::new(task(new_session(), tools));
        t.open("Say hi");
        t.line(grok_line(0), false);
        t.line(&answer(OPEN, json!({ "sessionId": "s-1" })), false);
        t
    }

    #[test]
    fn a_new_task_initializes_opens_the_session_then_sends_the_prompt() {
        let mut t = AcpTurn::new(task(new_session(), false));
        let opening: Vec<Value> = t
            .open("Say hi")
            .unwrap()
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(opening.len(), 1);
        assert_eq!(opening[0]["method"], "initialize");
        assert_eq!(opening[0]["params"]["protocolVersion"], 1);
        assert_eq!(
            opening[0]["params"]["clientCapabilities"]["terminal"],
            false
        );
        assert_eq!(
            opening[0]["params"]["clientCapabilities"]["fs"]["writeTextFile"],
            false
        );
        assert!(
            !opening[0].to_string().contains("Say hi"),
            "not before the session"
        );

        // Grok 1.0.41's real answer to `initialize`.
        let p = t.line(grok_line(0), false);
        let open = sent(&p);
        assert_eq!(open[0]["method"], "session/new");
        assert_eq!(open[0]["params"]["cwd"], "/work/conversation");
        assert_eq!(open[0]["params"]["mcpServers"], json!([]));
        assert_eq!(
            open[0]["params"]["_meta"]["agentProfile"]["tools"],
            json!([])
        );

        // Grok's own progress notifications are neither events nor unknown.
        for n in 1..=6 {
            let p = t.line(grok_line(n), false);
            assert_eq!(p, Parsed::none(), "{}", grok_line(n));
        }

        let p = t.line(
            &answer(
                OPEN,
                json!({ "sessionId": "s-1", "models": { "currentModelId": "grok-4.6" } }),
            ),
            false,
        );
        assert_eq!(
            p.events,
            [AgentEvent::SessionStarted {
                provider_session_id: Some("s-1".into()),
                model: Some("grok-4.6".into())
            }]
        );
        let prompt = sent(&p);
        assert_eq!(prompt[0]["method"], "session/prompt");
        assert_eq!(prompt[0]["params"]["sessionId"], "s-1");
        assert_eq!(
            prompt[0]["params"]["prompt"],
            json!([{ "type": "text", "text": "Say hi" }])
        );

        let chunk = |text: &str| {
            notify(json!({ "sessionUpdate": "agent_message_chunk",
                           "content": { "type": "text", "text": text } }))
        };
        assert_eq!(
            t.line(&chunk("Let me "), false).events,
            [AgentEvent::TextDelta {
                text: "Let me ".into()
            }]
        );
        t.line(&chunk("look."), false);
        let p = t.line(
            &notify(
                json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "Read",
                            "kind": "read", "status": "pending",
                            "rawInput": { "path": "src/main.rs" } }),
            ),
            false,
        );
        assert_eq!(
            p.events,
            [
                AgentEvent::Message {
                    text: "Let me look.".into()
                },
                AgentEvent::ToolUse {
                    tool: "Read".into(),
                    summary: "src/main.rs".into()
                }
            ]
        );
        let p = t.line(
            &notify(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1",
                            "title": "Read", "status": "failed",
                            "content": [{ "type": "content",
                                          "content": { "type": "text", "text": "denied\nmore" } }] })),
            false,
        );
        assert_eq!(
            p.events,
            [AgentEvent::ToolResult {
                tool: Some("Read".into()),
                is_error: true,
                summary: "denied".into()
            }]
        );
        t.line(
            &notify(json!({ "sessionUpdate": "agent_thought_chunk",
                            "content": { "type": "text", "text": "hmm" } })),
            false,
        );
        t.line(&chunk("Hi!"), false);

        let p = t.line(
            &answer(
                PROMPT,
                json!({ "stopReason": "end_turn",
                        "_meta": { "usage": { "inputTokens": 50, "cachedReadTokens": 40,
                                              "outputTokens": 7 } } }),
            ),
            false,
        );
        assert!(p.close_input);
        let usage = TokenUsage {
            input_tokens: 50,
            cached_input_tokens: 40,
            output_tokens: 7,
        };
        assert_eq!(
            p.events,
            [
                AgentEvent::Message { text: "Hi!".into() },
                AgentEvent::Usage { usage }
            ]
        );

        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("Hi!"));
        assert_eq!(r.summary, "Hi!");
        assert_eq!(r.provider_session_id.as_deref(), Some("s-1"));
        assert_eq!(r.model.as_deref(), Some("grok-4.6"));
        assert_eq!(r.usage, Some(usage));
        assert_eq!(r.ignored_lines, 0);
    }

    #[test]
    fn signed_out_is_reported_as_a_sign_in_problem_without_signing_in() {
        let mut t = AcpTurn::new(task(new_session(), false));
        t.open("Say hi");
        t.line(grok_line(0), false);
        let mut closed = false;
        for n in 1..GROK_SIGNED_OUT.lines().count() {
            let p = t.line(grok_line(n), false);
            // Never answered with `authenticate`: that could start a sign-in in a browser.
            assert!(p.send.iter().all(|l| !l.contains("authenticate")), "{p:?}");
            closed |= p.close_input;
        }
        assert!(
            closed,
            "the tool must be let go once it cannot open a session"
        );
        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::AuthRequired, "{r:#?}");
        assert!(r.error.unwrap().contains("Authentication required"));
    }

    #[test]
    fn a_follow_up_resumes_loads_or_says_it_cannot() {
        let resume = || ProviderSession::Resume { id: "s-9".into() };
        // Grok offers `session/resume` (no history replay).
        let mut t = AcpTurn::new(task(resume(), false));
        t.open("Again");
        let open = sent(&t.line(grok_line(0), false));
        assert_eq!(open[0]["method"], "session/resume");
        assert_eq!(open[0]["params"]["sessionId"], "s-9");
        assert_eq!(open[0]["params"]["cwd"], "/work/conversation");

        // A tool that can only load replays the history first; none of it is recorded.
        let init = |caps: Value| {
            answer(
                INITIALIZE,
                json!({ "protocolVersion": 1, "agentCapabilities": caps }),
            )
        };
        let mut t = AcpTurn::new(task(resume(), false));
        t.open("Again");
        let open = sent(&t.line(&init(json!({ "loadSession": true })), false));
        assert_eq!(open[0]["method"], "session/load");
        let replay = notify(json!({ "sessionUpdate": "agent_message_chunk",
                                    "content": { "type": "text", "text": "old answer" } }));
        assert_eq!(t.line(&replay, false), Parsed::none());
        let p = t.line(&answer(OPEN, Value::Null), false);
        assert_eq!(sent(&p)[0]["params"]["sessionId"], "s-9");
        t.line(&answer(PROMPT, json!({ "stopReason": "end_turn" })), false);
        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text, None, "the replayed answer is not this task's");
        assert_eq!(r.provider_session_id.as_deref(), Some("s-9"));

        let mut t = AcpTurn::new(task(resume(), false));
        t.open("Again");
        let p = t.line(&init(json!({ "loadSession": false })), false);
        assert!(p.close_input && p.send.is_empty());
        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Failed);
        assert!(r.summary.contains("cannot resume"), "{}", r.summary);
    }

    fn permission(tools: bool, tool_call: Value, options: Value) -> (Parsed, AcpTurn) {
        let mut t = prompting(tools);
        let p = t.line(
            &json!({ "jsonrpc": "2.0", "id": 77, "method": "session/request_permission",
                     "params": { "sessionId": "s-1", "toolCall": tool_call, "options": options } })
            .to_string(),
            false,
        );
        (p, t)
    }

    fn options() -> Value {
        json!([
            { "optionId": "yes", "name": "Allow", "kind": "allow_once" },
            { "optionId": "always", "name": "Always", "kind": "allow_always" },
            { "optionId": "no", "name": "Reject", "kind": "reject_once" }
        ])
    }

    #[test]
    fn only_plenipo_tool_calls_are_allowed() {
        let plenipo = json!({ "toolCallId": "c1", "title": "use_tool",
                              "rawInput": { "server": "plenipo", "tool": "read_file" } });
        let (p, _) = permission(true, plenipo.clone(), options());
        let reply = &sent(&p)[0];
        assert_eq!(reply["id"], 77);
        assert_eq!(
            reply["result"]["outcome"],
            json!({ "outcome": "selected", "optionId": "yes" })
        );
        assert!(p.events.is_empty());

        // Without Plenipo's tools, the same call is refused.
        let (p, _) = permission(false, plenipo, options());
        assert_eq!(sent(&p)[0]["result"]["outcome"]["optionId"], "no");

        // The tool's own tools, or another server's, are refused and noted.
        let own = json!({ "toolCallId": "c2", "title": "run_terminal_command",
                          "rawInput": { "command": "rm -rf /" } });
        let (p, _) = permission(true, own, options());
        assert_eq!(sent(&p)[0]["result"]["outcome"]["optionId"], "no");
        assert!(matches!(
            &p.events[..],
            [AgentEvent::Notice { text, .. }] if text.contains("run_terminal_command")
        ));

        // No option to refuse with: the request is cancelled rather than allowed.
        let (p, _) = permission(
            true,
            json!({ "title": "write" }),
            json!([{ "optionId": "yes", "kind": "allow_once" }]),
        );
        assert_eq!(
            sent(&p)[0]["result"]["outcome"],
            json!({ "outcome": "cancelled" })
        );
    }

    #[test]
    fn tool_server_calls_are_recognized_by_name_or_by_server() {
        for call in [
            json!({ "rawInput": { "server": "plenipo", "tool": "x" } }),
            json!({ "rawInput": { "server_name": "plenipo" } }),
            json!({ "_meta": { "toolName": "mcp__plenipo__read_file" } }),
            json!({ "toolName": "plenipo__read_file" }),
            json!({ "toolName": "plenipo:read_file" }),
        ] {
            assert!(is_tool_server_call(&call, "plenipo"), "{call}");
        }
        for call in [
            json!({ "rawInput": { "server": "github" } }),
            json!({ "toolName": "mcp__plenipoevil__x" }),
            json!({ "toolName": "read_file", "title": "plenipo__read_file" }),
            json!({ "rawInput": { "path": "plenipo" } }),
            json!(null),
        ] {
            assert!(!is_tool_server_call(&call, "plenipo"), "{call}");
        }
    }

    #[test]
    fn client_methods_plenipo_does_not_offer_are_refused() {
        let mut t = prompting(false);
        let p = t.line(
            &json!({ "jsonrpc": "2.0", "id": "r1", "method": "fs/write_text_file",
                     "params": { "path": "/etc/passwd", "content": "" } })
            .to_string(),
            false,
        );
        let reply = &sent(&p)[0];
        assert_eq!(reply["id"], "r1");
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[test]
    fn cancel_asks_the_tool_to_stop_the_prompt() {
        let mut t = AcpTurn::new(task(new_session(), false));
        t.open("Long job");
        assert!(t.cancel().is_empty(), "nothing to cancel before the prompt");
        t.line(grok_line(0), false);
        t.line(&answer(OPEN, json!({ "sessionId": "s-1" })), false);
        let lines = t.cancel();
        let cancel: Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(cancel["method"], "session/cancel");
        assert_eq!(cancel["params"]["sessionId"], "s-1");
        assert!(cancel.get("id").is_none(), "a notification");
        let p = t.line(&answer(PROMPT, json!({ "stopReason": "cancelled" })), false);
        assert!(p.close_input);
        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Cancelled);
    }

    #[test]
    fn prompt_errors_are_classified() {
        for (message, outcome) in [
            (
                "You have reached your usage limit",
                TurnOutcome::UsageLimited,
            ),
            ("Authentication required", TurnOutcome::AuthRequired),
            ("Model overloaded", TurnOutcome::ProviderUnavailable),
            ("Something else", TurnOutcome::Failed),
        ] {
            let mut t = prompting(false);
            let p = t.line(
                &json!({ "jsonrpc": "2.0", "id": PROMPT,
                         "error": { "code": -32603, "message": message } })
                .to_string(),
                false,
            );
            assert!(p.close_input);
            let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
            assert_eq!(r.outcome, outcome, "{message}");
        }
    }

    #[test]
    fn another_protocol_version_stops_the_task() {
        let mut t = AcpTurn::new(task(new_session(), false));
        t.open("x");
        let p = t.line(&answer(INITIALIZE, json!({ "protocolVersion": 2 })), false);
        let stop = p.stop.unwrap();
        assert_eq!(stop.outcome, TurnOutcome::MalformedOutput);
        assert!(stop.reason.contains("version 2"), "{}", stop.reason);
    }

    #[test]
    fn usage_counts_the_whole_prompt_in_either_naming() {
        assert_eq!(
            usage(
                &json!({ "usage": { "input_tokens": 10, "cache_read_input_tokens": 30,
                                      "cache_creation_input_tokens": 5, "output_tokens": 2 } })
            ),
            Some(TokenUsage {
                input_tokens: 45,
                cached_input_tokens: 30,
                output_tokens: 2
            })
        );
        assert_eq!(usage(&json!({ "stopReason": "end_turn" })), None);
    }

    #[test]
    fn unknown_messages_are_ignored_and_counted() {
        let mut t = prompting(false);
        for line in [
            r#"{"jsonrpc":"2.0","method":"session/something_new","params":{}}"#,
            &notify(json!({ "sessionUpdate": "brand_new_update" })),
            r#"{"type":"not json-rpc"}"#,
            &answer(99, json!({})),
        ] {
            let p = t.line(line, false);
            assert!(
                p.events.is_empty() && p.send.is_empty() && p.stop.is_none(),
                "{line}"
            );
        }
        t.line(&answer(PROMPT, json!({ "stopReason": "end_turn" })), false);
        let r = t.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.ignored_lines, 4);
    }
}
