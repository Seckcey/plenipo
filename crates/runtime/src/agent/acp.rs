//! The shared ACP driver (ADR-015): one task over the Agent Client Protocol.
//!
//! ACP is JSON-RPC 2.0, one message per line, on the AI tool's stdin and stdout. Plenipo is the
//! client. A task is one supervised process, and the whole exchange happens in it:
//!
//! 1. `initialize` (protocol version 1; Plenipo offers no terminal, and file access only when
//!    the adapter asks for it, below);
//! 2. `session/new`, or `session/resume` / `session/load` for a follow-up, with Plenipo's tool
//!    server when the worker has permissions;
//! 3. the adapter's session settings, if any (`session/set_config_option`, each checked);
//! 4. `session/prompt` with the task text, once the conversation ID is known; the tool streams
//!    `session/update` notifications and may ask `session/request_permission`;
//! 5. the `session/prompt` answer ends the task, and stdin is closed so the tool exits.
//!
//! Plenipo answers every permission request itself: a call to its own tool server is allowed
//! (Guard decides inside the call), anything else is refused. Plenipo knows a call is for its
//! tool server only from the tool name the AI tool itself sets (`_meta.toolName` or `toolName`
//! with the server's name before it, as in `mcp__plenipo__read_file`); the call's input
//! (`rawInput`) is text the model wrote, so what it says never decides. A shell command
//! (`execute`) is decided first: only Plenipo's own `run_command`, named that way, may run a
//! program. Each approval covers one action (ADR-013); Plenipo never chooses "allow for this
//! session". It never sends `authenticate`, which could start a sign-in in a browser; a tool
//! that is not signed in is reported as such. Unknown messages are ignored and counted
//! (ADR-007 §7).
//!
//! For an AI tool whose own tools cannot be switched off, an adapter turns on **file access
//! through Plenipo** (ADR-027, Kimi over ACP): `initialize` offers file reads and writes, and
//! each `fs/read_text_file` / `fs/write_text_file` becomes a [`FileRequest`] that Plenipo carries
//! out through Guard under the worker's permissions (a worker without them has every file
//! request refused). The tool's own file changes are allowed only for a worker that may change
//! files, since the change itself then comes to Plenipo; its own shell is refused whatever else
//! the request says, and so is everything else. Such a tool may name one of Plenipo's tools in
//! a request's title (`title_is_tool_name`): that counts, except for a shell command, and each
//! call allowed by its title is noted in the task's activity. The adapter can also require
//! modes: a mode outside them stops the task.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent::adapter::{
    cap, first_line, tool_summary, FileRequest, Parsed, ProcessEnd, ProviderSession, Stop,
    TurnParser, TurnState, MAX_EVENT_TEXT,
};
use crate::agent::dto::{AgentEvent, NoticeLevel, TurnOutcome, TurnResult};
use crate::agent::tools::{FileAccess, FileAnswer, ToolServer};
use crate::dto::TokenUsage;

/// The ACP protocol version Plenipo speaks.
pub const PROTOCOL_VERSION: u64 = 1;

/// Request IDs Plenipo uses; one of each per task.
const INITIALIZE: u64 = 1;
const OPEN: u64 = 2;
const PROMPT: u64 = 3;
/// Session settings are numbered from here, one ID each.
const SETTING: u64 = 10;
/// Most tool calls remembered by ID (a permission request may not repeat the call's kind).
const MAX_CALLS: usize = 256;
/// Most of the tool's own allowed file changes waiting for their write to come to Plenipo. One
/// more is refused (never an older one forgotten), so every allowed change is still checked.
const MAX_UNWRITTEN: usize = 64;
/// Plenipo's tool that writes files: only a worker whose grant offers it may let the AI tool
/// change files (ADR-027).
const WRITE_TOOL: &str = "write_file";
/// Plenipo's tool that runs programs: the only tool a shell command (`execute`) may be.
const RUN_TOOL: &str = "run_command";

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
    /// File access through Plenipo (ADR-027): see the module notes.
    pub file_access: bool,
    /// Settings sent with `session/set_config_option` after the conversation opens and before
    /// the prompt, in order, as (option ID, value). The option IDs `model` and `mode` are also
    /// what the task reports and checks.
    pub settings: Vec<(String, String)>,
    /// When not empty, the only modes the tool may be in once the prompt is sent; the task
    /// stops if it reports another (ADR-027 §4).
    pub allowed_modes: Vec<String>,
    /// Reopen a conversation with `session/load` even when the tool offers `session/resume`.
    pub load_to_resume: bool,
    /// The tool names the tool in a permission request's title (Kimi), so a title may name a
    /// tool of Plenipo's tool server: with the server's name before it, or bare. Never for a
    /// shell command (`execute`), and each call allowed by its title is noted in the activity.
    pub title_is_tool_name: bool,
    /// Refuse the task as soon as the tool answers `initialize`, before any conversation opens
    /// (for example a model Plenipo does not run on this tool).
    pub refusal: Option<Stop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before [`TurnParser::open`].
    Idle,
    Initializing,
    /// Opening or loading the conversation; updates now are history being replayed.
    Opening,
    /// Sending the session settings, one at a time.
    Configuring,
    Prompting,
    Done,
}

/// A tool call the tool announced: what it calls it, and its kind (`read`, `edit`, `execute`, …).
#[derive(Debug, Clone, Default)]
struct Call {
    title: Option<String>,
    kind: Option<String>,
    /// The files the call names (`locations`), as the tool reported them with the call.
    paths: Vec<String>,
}

/// How a permission request was found to be for Plenipo's tool server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerCall<'a> {
    /// By the tool name the AI tool itself sets, with the server's name before it: this tool.
    Named(&'a str),
    /// By its title, which names one of the server's tools (`title_is_tool_name`).
    Titled,
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
    /// The next session setting to send (an index into `task.settings`).
    setting: usize,
    /// The tool's mode, as last reported.
    mode: Option<String>,
    /// Tool calls announced in this task, by ID.
    calls: HashMap<String, Call>,
    /// File requests waiting for Plenipo's answer: number → (JSON-RPC ID, the path when it is
    /// a write).
    files: HashMap<u64, (Value, Option<String>)>,
    next_file: u64,
    /// The tool's own file changes Plenipo allowed whose write has not come through Plenipo yet:
    /// the call's ID and the files it named (`locations`), oldest first, at most
    /// [`MAX_UNWRITTEN`]. A write Plenipo carried out clears the one change it is for
    /// ([`AcpTurn::written`]), never every change at once, so a second change that never comes
    /// is still caught.
    unwritten: Vec<(String, Vec<String>)>,
    /// The worker's activity already says its files are closed.
    files_refused_noted: bool,
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
            setting: 0,
            mode: None,
            calls: HashMap::new(),
            files: HashMap::new(),
            next_file: 0,
            unwritten: Vec::new(),
            files_refused_noted: false,
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

    /// Plenipo stops the task: the process is ended.
    fn stop(&mut self, outcome: TurnOutcome, reason: String) -> Parsed {
        let stop = Stop { outcome, reason };
        self.state.stop = Some(stop.clone());
        self.phase = Phase::Done;
        Parsed {
            stop: Some(stop),
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
            return self.stop(TurnOutcome::MalformedOutput, reason);
        }
        if let Some(refusal) = self.task.refusal.clone() {
            return self.stop(refusal.outcome, refusal.reason);
        }
        let caps = result.get("agentCapabilities");
        let can = |pointer: &str| {
            caps.and_then(|c| c.pointer(pointer))
                .is_some_and(|v| !v.is_null() && v != &json!(false))
        };
        let cwd = self.task.working_dir.display().to_string();
        let resume = !self.task.load_to_resume && can("/sessionCapabilities/resume");
        let (method, params) = match &self.task.session {
            ProviderSession::New { .. } => (
                "session/new",
                json!({ "cwd": cwd, "mcpServers": self.mcp_servers() }),
            ),
            // `session/resume` does not replay the history; `session/load` does.
            ProviderSession::Resume { id } if resume => (
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
        let options = result.get("configOptions");
        let model = ["/models/currentModelId", "/_meta/modelState/currentModelId"]
            .iter()
            .find_map(|p| result.pointer(p).and_then(Value::as_str))
            .map(str::to_owned)
            .or_else(|| options.and_then(|o| option_value(o, "model")));
        if model.is_some() {
            self.state.model = model;
        }
        if let Some(mode) = result
            .pointer("/modes/currentModeId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| options.and_then(|o| option_value(o, "mode")))
        {
            self.mode = Some(mode);
        }
        self.state.provider_session_id = Some(id);
        self.phase = Phase::Configuring;
        self.next_setting()
    }

    /// Send the next session setting, or the prompt once they are all set.
    fn next_setting(&mut self) -> Parsed {
        let Some((option, value)) = self.task.settings.get(self.setting).cloned() else {
            return self.send_prompt();
        };
        let session = self.state.provider_session_id.clone().unwrap_or_default();
        Parsed {
            send: vec![request(
                SETTING + self.setting as u64,
                "session/set_config_option",
                json!({ "sessionId": session, "configId": option, "value": value }),
            )],
            ..Parsed::none()
        }
    }

    /// The tool answered a setting: it must now hold the value asked for.
    fn setting_set(&mut self, result: &Value) -> Parsed {
        let (option, value) = self.task.settings[self.setting].clone();
        let reported = result
            .get("configOptions")
            .and_then(|o| option_value(o, &option));
        if let Some(now) = reported.filter(|now| *now != value) {
            return self.fail(format!(
                "{} did not take the setting {option} = {value} (it reports {now}).",
                self.label()
            ));
        }
        match option.as_str() {
            "model" => self.state.model = Some(value),
            "mode" => self.mode = Some(value),
            _ => {}
        }
        self.setting += 1;
        self.next_setting()
    }

    /// Whether the tool's `mode` is one the adapter allows.
    fn mode_allowed(&self, mode: &str) -> bool {
        self.task.allowed_modes.is_empty() || self.task.allowed_modes.iter().any(|m| m == mode)
    }

    fn mode_stop(&mut self, mode: &str) -> Parsed {
        let reason = format!(
            "{} switched to its \"{}\" mode, which Plenipo does not allow; the task was stopped.",
            self.label(),
            first_line(mode, 40)
        );
        self.stop(TurnOutcome::Failed, reason)
    }

    /// The tool reported its mode during the task.
    fn mode_reported(&mut self, mode: &str) -> Option<Parsed> {
        self.mode = Some(mode.to_owned());
        (!self.mode_allowed(mode)).then(|| self.mode_stop(mode))
    }

    fn send_prompt(&mut self) -> Parsed {
        if let Some(mode) = self.mode.clone().filter(|m| !self.mode_allowed(m)) {
            return self.mode_stop(&mode);
        }
        let id = self.state.provider_session_id.clone().unwrap_or_default();
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
        let setting = (self.phase == Phase::Configuring).then(|| SETTING + self.setting as u64);
        if let Some(error) = message.get("error") {
            let text = error_text(error);
            let ours = matches!(id, Some(INITIALIZE | OPEN | PROMPT)) || id == setting;
            if !ours {
                self.state.unknown += 1;
                return Parsed::none();
            }
            let mut parsed = self.fail(text);
            parsed.events.extend(self.flush_message());
            return parsed;
        }
        let result = message.get("result").cloned().unwrap_or(Value::Null);
        match (id, self.phase) {
            (Some(INITIALIZE), Phase::Initializing) => self.initialized(&result),
            (Some(OPEN), Phase::Opening) => self.opened(&result),
            (Some(n), Phase::Configuring) if Some(n) == setting => self.setting_set(&result),
            (Some(PROMPT), Phase::Prompting) => self.prompted(&result),
            _ => {
                self.state.unknown += 1;
                Parsed::none()
            }
        }
    }

    /// A request from the tool, which must be answered.
    fn request_from_tool(&mut self, id: &Value, method: &str, params: &Value) -> Parsed {
        match method {
            "session/request_permission" => self.permission(id, params),
            "fs/read_text_file" | "fs/write_text_file" if self.task.file_access => {
                self.file(id, method == "fs/write_text_file", params)
            }
            // Plenipo offers no terminal, and no other client methods.
            _ => Parsed {
                send: vec![reply_error(id, -32601, "Method not found")],
                ..Parsed::none()
            },
        }
    }

    /// Whether, and how, a permission request is for a tool of Plenipo's tool server. Only what
    /// the AI tool itself sets counts: the tool name with the server's name before it, or, for
    /// an adapter whose titles are tool names, the title. The call's input (`rawInput`) is text
    /// the model wrote and never decides. A title never counts for a shell command
    /// (`execute`): Plenipo's tools are not asked for that way (ADR-027).
    fn server_call<'a>(
        &self,
        call: &'a Value,
        kind: Option<&str>,
        server: &ToolServer,
    ) -> Option<ServerCall<'a>> {
        if let Some(tool) = named_server_tool(call, &server.name) {
            return Some(ServerCall::Named(tool));
        }
        let titled = self.task.title_is_tool_name
            && kind != Some("execute")
            && call
                .get("title")
                .and_then(Value::as_str)
                .is_some_and(|t| names_server_tool(t, server));
        titled.then_some(ServerCall::Titled)
    }

    /// The worker may change files: its grant offers Plenipo's `write_file`.
    fn can_write(&self) -> bool {
        self.task
            .tools
            .as_ref()
            .is_some_and(|t| t.tools.iter().any(|n| n == WRITE_TOOL))
    }

    /// A write for `path` came through Plenipo: it clears the oldest allowed change that named
    /// that file, or, failing that, the oldest that named no file. Never more than one.
    fn written(&mut self, path: &str) {
        let dir = &self.task.working_dir;
        let at = self
            .unwritten
            .iter()
            .position(|(_, paths)| paths.iter().any(|p| same_file(dir, p, path)))
            .or_else(|| {
                self.unwritten
                    .iter()
                    .position(|(_, paths)| paths.is_empty())
            });
        if let Some(at) = at {
            self.unwritten.remove(at);
        }
    }

    fn permission(&mut self, id: &Value, params: &Value) -> Parsed {
        let call = params.get("toolCall").unwrap_or(&Value::Null);
        let call_id = call
            .get("toolCallId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let kind = call
            .get("kind")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                let known = self.calls.get(call_id.as_deref()?)?;
                known.kind.clone()
            });
        let execute = kind.as_deref() == Some("execute");
        let what = call_name(call);
        let label = self.label();
        let server_call = self
            .task
            .tools
            .as_ref()
            .and_then(|t| self.server_call(call, kind.as_deref(), t));
        let own_edit =
            server_call.is_none() && self.task.file_access && kind.as_deref() == Some("edit");
        // Why the request is refused, as the task's activity says it; `None` allows it. A shell
        // command is decided first, whatever else the request says (ADR-027): with file access
        // through Plenipo the AI tool's own shell is always refused; otherwise only Plenipo's
        // own `run_command`, named by the AI tool itself, may run a program.
        let refusal = if execute
            && (self.task.file_access || !matches!(server_call, Some(ServerCall::Named(RUN_TOOL))))
        {
            Some(format!(
                "{label} asked to run a command with its own shell ({what}); Plenipo refused it. \
                 Workers run programs with Plenipo's run_command tool."
            ))
        } else if server_call.is_some() {
            None
        } else if !self.task.file_access {
            Some(format!(
                "{label} asked to use {what}; Plenipo refused it (only Plenipo's own tools are \
                 allowed)."
            ))
        } else if own_edit {
            if !self.can_write() {
                Some(format!(
                    "{label} asked to change a file ({what}); Plenipo refused it: this worker has \
                     no permission to change files."
                ))
            } else if call_id.is_none() {
                Some(format!(
                    "{label} asked to change a file ({what}) without naming the call; Plenipo \
                     refused it."
                ))
            } else if self.unwritten.len() >= MAX_UNWRITTEN {
                Some(format!(
                    "{label} asked to change a file ({what}); Plenipo refused it: {MAX_UNWRITTEN} \
                     changes it allowed have not come to Plenipo yet."
                ))
            } else {
                None
            }
        } else {
            Some(format!(
                "{label} asked to use {what}; Plenipo refused it (only Plenipo's own tools and \
                 file access through Plenipo are allowed)."
            ))
        };
        // Each approval covers one action (ADR-013): Plenipo never chooses "allow for this
        // session" (ADR-027).
        let wanted: &[&str] = if refusal.is_none() {
            &["allow_once"]
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
        if let (true, None, Some(_), Some(call_id)) = (own_edit, &refusal, &choice, call_id) {
            // The change itself must now come to Plenipo as `fs/write_text_file`, for one of
            // the files the call names (or, when it names none, for any file).
            let mut paths = call_paths(call);
            if paths.is_empty() {
                // Kimi names the files with the call, not in the permission request.
                paths = self
                    .calls
                    .get(&call_id)
                    .map(|c| c.paths.clone())
                    .unwrap_or_default();
            }
            self.unwritten.push((call_id, paths));
        }
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
        match refusal {
            Some(text) => parsed.events.push(AgentEvent::Notice {
                level: NoticeLevel::Info,
                text,
            }),
            // Allowed by its title alone: the activity says which tool and why, so the owner
            // can see the decision.
            None if server_call == Some(ServerCall::Titled) => {
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    text: format!(
                        "{label} asked to use {what}; Plenipo allowed it because that is one of \
                         Plenipo's own tools, named in the request (Plenipo's rules still apply \
                         inside the call)."
                    ),
                });
            }
            None => {}
        }
        parsed
    }

    /// The tool asks Plenipo to read or write a file for it (ADR-027).
    fn file(&mut self, id: &Value, write: bool, params: &Value) -> Parsed {
        let path = params
            .get("path")
            .and_then(Value::as_str)
            .filter(|p| !p.trim().is_empty());
        let content = params.get("content").and_then(Value::as_str);
        let same_session = match (
            params.get("sessionId").and_then(Value::as_str),
            &self.state.provider_session_id,
        ) {
            (Some(asked), Some(ours)) => asked == ours,
            _ => true,
        };
        let (Some(path), true, true) = (path, same_session, !write || content.is_some()) else {
            return Parsed {
                send: vec![reply_error(id, -32602, "Invalid params")],
                ..Parsed::none()
            };
        };
        if self.task.tools.is_none() {
            let mut parsed = Parsed {
                send: vec![reply_error(
                    id,
                    -32000,
                    "Not done: this worker has no permission to use files. Do not try to get \
                     around this; say in your answer what you needed.",
                )],
                ..Parsed::none()
            };
            if !std::mem::replace(&mut self.files_refused_noted, true) {
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    text: format!(
                        "{} asked to open a file; this worker has no permission to use files, \
                         so Plenipo refused.",
                        self.label()
                    ),
                });
            }
            return parsed;
        }
        let access = if write {
            // The allowed change this write is for is cleared once Plenipo carried it out
            // (`file_answered`).
            FileAccess::Write {
                path: path.to_owned(),
                content: content.unwrap_or_default().to_owned(),
            }
        } else {
            let number = |key: &str| {
                params
                    .get(key)
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
            };
            FileAccess::Read {
                path: path.to_owned(),
                line: number("line"),
                limit: number("limit"),
            }
        };
        self.next_file += 1;
        self.files
            .insert(self.next_file, (id.clone(), write.then(|| path.to_owned())));
        Parsed {
            files: vec![FileRequest {
                id: self.next_file,
                access,
            }],
            ..Parsed::none()
        }
    }

    fn update(&mut self, params: &Value) -> Parsed {
        let update = params.get("update").unwrap_or(&Value::Null);
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or("");
        if self.phase != Phase::Prompting {
            // History replayed while a conversation loads, the tool's answers to the settings,
            // or noise before the prompt.
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
                let text = |key: &str| update.get(key).and_then(Value::as_str).map(str::to_owned);
                if let Some(id) = text("toolCallId") {
                    if self.calls.len() >= MAX_CALLS {
                        self.calls.clear();
                    }
                    self.calls.insert(
                        id,
                        Call {
                            title: text("title"),
                            kind: text("kind"),
                            paths: call_paths(update),
                        },
                    );
                }
                let input = update.get("rawInput").unwrap_or(&Value::Null);
                parsed.events.push(AgentEvent::ToolUse {
                    tool: call_name(update),
                    summary: tool_summary(input),
                });
            }
            "tool_call_update" => return self.call_updated(update),
            "current_mode_update" => {
                if let Some(mode) = update.get("currentModeId").and_then(Value::as_str) {
                    return self.mode_reported(mode).unwrap_or_default();
                }
            }
            "config_option_update" => {
                if let Some(mode) = update
                    .get("configOptions")
                    .and_then(|o| option_value(o, "mode"))
                {
                    return self.mode_reported(&mode).unwrap_or_default();
                }
            }
            "agent_thought_chunk"
            | "user_message_chunk"
            | "plan"
            | "available_commands_update"
            | "session_info_update"
            | "usage_update" => {}
            _ => self.state.unknown += 1,
        }
        parsed
    }

    fn call_updated(&mut self, update: &Value) -> Parsed {
        let id = update.get("toolCallId").and_then(Value::as_str);
        if let Some(call) = id.and_then(|id| self.calls.get_mut(id)) {
            if let Some(kind) = update.get("kind").and_then(Value::as_str) {
                call.kind = Some(kind.to_owned());
            }
            let paths = call_paths(update);
            if !paths.is_empty() {
                call.paths = paths;
            }
        }
        let status = update.get("status").and_then(Value::as_str);
        if !matches!(status, Some("completed" | "failed")) {
            return Parsed::none();
        }
        let known = id.and_then(|i| self.calls.get(i));
        let tool = update
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| known.and_then(|c| c.title.clone()));
        let text = update
            .pointer("/content/0/content/text")
            .and_then(Value::as_str)
            .unwrap_or("");
        let result = AgentEvent::ToolResult {
            tool: tool.clone(),
            is_error: status == Some("failed"),
            summary: first_line(text, 200),
        };
        let unwritten = id.is_some_and(|i| {
            let at = self.unwritten.iter().position(|(call, _)| call == i);
            at.map(|at| self.unwritten.remove(at)).is_some()
        });
        if unwritten && status == Some("completed") {
            // An allowed change that never came to Plenipo: it happened outside Guard.
            let reason = format!(
                "{} reported a file change ({}) that did not go through Plenipo; the task was \
                 stopped.",
                self.label(),
                tool.as_deref().unwrap_or("a file tool")
            );
            let mut parsed = self.stop(TurnOutcome::Failed, reason);
            parsed.events.push(result);
            return parsed;
        }
        Parsed::one(result)
    }
}

impl TurnParser for AcpTurn {
    fn open(&mut self, prompt: &str) -> Option<Vec<String>> {
        self.prompt = prompt.to_owned();
        self.phase = Phase::Initializing;
        let files = self.task.file_access;
        Some(vec![request(
            INITIALIZE,
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "clientCapabilities": {
                    "fs": { "readTextFile": files, "writeTextFile": files },
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

    fn file_answered(&mut self, id: u64, answer: FileAnswer) -> Parsed {
        let Some((request, written)) = self.files.remove(&id) else {
            return Parsed::none();
        };
        if let (Ok(_), Some(path)) = (&answer, &written) {
            // The change came through Plenipo (Guard allowed it, and it was written).
            self.written(path);
        }
        let line = match answer {
            Ok(_) if written.is_some() => {
                json!({ "jsonrpc": "2.0", "id": request, "result": {} }).to_string()
            }
            Ok(content) => {
                json!({ "jsonrpc": "2.0", "id": request, "result": { "content": content } })
                    .to_string()
            }
            Err(why) => reply_error(&request, -32000, &why),
        };
        Parsed {
            send: vec![line],
            ..Parsed::none()
        }
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

/// One JSON-RPC error answer line.
fn reply_error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
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

/// The current value of a session setting in a `configOptions` list, found by its ID or its
/// category (`model`, `mode`, `thought_level`, …).
fn option_value(options: &Value, option: &str) -> Option<String> {
    options
        .as_array()?
        .iter()
        .find(|o| {
            o.get("id").and_then(Value::as_str) == Some(option)
                || o.get("category").and_then(Value::as_str) == Some(option)
        })?
        .get("currentValue")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// A tool call's name, as the tool reports it.
fn call_name(call: &Value) -> String {
    ["/_meta/toolName", "/toolName", "/title", "/kind"]
        .iter()
        .find_map(|p| call.pointer(p).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
        .map_or_else(|| "a tool".to_owned(), |s| first_line(s, 80))
}

/// The tool a name names once the name of the tool server `server` before it is taken off
/// (`mcp__plenipo__read`, `plenipo__read`, `plenipo:read`, `plenipo/read` → `read`); `None`
/// when the name does not carry it.
fn strip_server_prefix<'a>(name: &'a str, server: &str) -> Option<&'a str> {
    let rest = name
        .strip_prefix("mcp__")
        .unwrap_or(name)
        .strip_prefix(server)?;
    ["__", ":", "/"]
        .iter()
        .find_map(|sep| rest.strip_prefix(sep))
}

/// Whether `name` is a tool of `server`: with the server's name before it, or bare and one of
/// the tools the server offers.
fn names_server_tool(name: &str, server: &ToolServer) -> bool {
    strip_server_prefix(name, &server.name).is_some() || server.tools.iter().any(|t| t == name)
}

/// The files a tool call names (`locations`, set by the AI tool itself), as paths.
fn call_paths(call: &Value) -> Vec<String> {
    call.get("locations")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|x| x.get("path").and_then(Value::as_str))
                .filter(|p| !p.trim().is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// `a` and `b` name the same file: each taken from `dir` when it is relative, `.` and `..`
/// worked out, `\\` read as `/`, and letter case ignored on Windows. Nothing else counts as
/// the same (no matching by the end of a path).
fn same_file(dir: &Path, a: &str, b: &str) -> bool {
    let full = |p: &str| {
        let p = p.trim().replace('\\', "/");
        let joined = if Path::new(&p).is_absolute() || p.starts_with('/') {
            PathBuf::from(p)
        } else {
            dir.join(p)
        };
        let mut parts: Vec<String> = Vec::new();
        for c in joined.to_string_lossy().replace('\\', "/").split('/') {
            match c {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                c => parts.push(if cfg!(windows) {
                    c.to_lowercase()
                } else {
                    c.to_owned()
                }),
            }
        }
        parts
    };
    full(a) == full(b)
}

/// The tool of the server named `server` a permission request is for, from the tool name the
/// AI tool itself sets (`_meta.toolName` or `toolName`, as in `mcp__plenipo__read_file` →
/// `read_file`). Nothing else counts: the call's input (`rawInput`) is text the model wrote,
/// so a server named there proves nothing.
fn named_server_tool<'a>(call: &'a Value, server: &str) -> Option<&'a str> {
    ["/_meta/toolName", "/toolName"]
        .iter()
        .filter_map(|p| call.pointer(p).and_then(Value::as_str))
        .find_map(|n| strip_server_prefix(n, server))
}

/// Whether a permission request is for a tool of the server named `server`, by the tool name
/// the AI tool itself sets (see [`named_server_tool`]).
pub fn is_tool_server_call(call: &Value, server: &str) -> bool {
    named_server_tool(call, server).is_some()
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
                tools: vec!["read_file".into(), "write_file".into()],
            }),
            model: Some("grok-4.6".into()),
            session_meta: Some(json!({ "agentProfile": { "tools": [] } })),
            ..AcpTask::default()
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
        prompting_on(task(new_session(), tools))
    }

    fn prompting_on(task: AcpTask) -> AcpTurn {
        let mut t = AcpTurn::new(task);
        t.open("Say hi");
        t.line(grok_line(0), false);
        t.line(&answer(OPEN, json!({ "sessionId": "s-1" })), false);
        t
    }

    /// A Kimi-shaped task (ADR-027): file access through Plenipo, titles that are tool names,
    /// and Plenipo's `run_command` among the tools offered.
    fn kimi_task() -> AcpTask {
        let mut task = task(new_session(), true);
        task.runtime_label = "Kimi";
        task.file_access = true;
        task.title_is_tool_name = true;
        if let Some(server) = task.tools.as_mut() {
            server.tools.push("run_command".into());
        }
        task
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
        permission_on(task(new_session(), tools), tool_call, options)
    }

    fn permission_on(task: AcpTask, tool_call: Value, options: Value) -> (Parsed, AcpTurn) {
        let mut t = prompting_on(task);
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

    /// The option Plenipo chose in its answer.
    fn chosen(p: &Parsed) -> Value {
        sent(p)[0]["result"]["outcome"]["optionId"].clone()
    }

    #[test]
    fn only_plenipo_tool_calls_are_allowed() {
        // Plenipo's tool, named by the AI tool itself (`_meta.toolName`): allowed for this one
        // action only, although "always" is offered.
        let plenipo = json!({ "toolCallId": "c1", "title": "use_tool",
                              "_meta": { "toolName": "mcp__plenipo__read_file" },
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
        assert_eq!(chosen(&p), "no");

        // The tool's own tools, or another server's, are refused and noted, whatever the call's
        // input says: the model writes the input, so naming Plenipo there proves nothing.
        let own = json!({ "toolCallId": "c2", "title": "run_terminal_command",
                          "rawInput": { "command": "rm -rf /", "server": "plenipo" } });
        let (p, _) = permission(true, own, options());
        assert_eq!(chosen(&p), "no");
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
    fn tool_server_calls_are_recognized_by_the_tool_s_name_only() {
        for call in [
            json!({ "_meta": { "toolName": "mcp__plenipo__read_file" } }),
            json!({ "toolName": "plenipo__read_file" }),
            json!({ "toolName": "plenipo:read_file" }),
            json!({ "toolName": "plenipo/read_file" }),
        ] {
            assert!(is_tool_server_call(&call, "plenipo"), "{call}");
        }
        // The call's input is the model's own text: naming the server there proves nothing.
        for call in [
            json!({ "rawInput": { "server": "plenipo", "tool": "x" } }),
            json!({ "rawInput": { "server_name": "plenipo" } }),
            json!({ "rawInput": { "serverName": "plenipo", "mcpServer": "plenipo" } }),
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
    fn grok_s_own_tools_are_refused_whatever_their_input_says() {
        // Tools not on Grok's own-tools list (a new one, or the generic `use_tool`) are still
        // not Plenipo's when only the input, which the model writes, says so.
        for call in [
            json!({ "toolCallId": "c1", "title": "fetch_url", "kind": "fetch",
                    "rawInput": { "url": "https://example.com", "server": "plenipo" } }),
            json!({ "toolCallId": "c2", "title": "shell", "kind": "execute",
                    "rawInput": { "command": "cat secrets.txt", "serverName": "plenipo" } }),
            json!({ "toolCallId": "c3", "title": "use_tool", "kind": "other",
                    "rawInput": { "server": "plenipo", "tool": "read_file" } }),
        ] {
            let (p, _) = permission(true, call.clone(), options());
            assert_eq!(chosen(&p), "no", "{call}");
            assert!(
                matches!(
                    &p.events[..],
                    [AgentEvent::Notice { text, .. }] if text.contains("Plenipo refused it")
                ),
                "{call}: {:?}",
                p.events
            );
        }

        // A shell command is Plenipo's only as its own `run_command`, named by Grok itself.
        let run = |tool: &str| {
            json!({ "toolCallId": "c4", "title": "use_tool", "kind": "execute",
                    "_meta": { "toolName": format!("mcp__plenipo__{tool}") },
                    "rawInput": { "server": "plenipo", "tool": tool } })
        };
        let (p, _) = permission(true, run("run_command"), options());
        assert_eq!(chosen(&p), "yes", "once, never for the session");
        assert!(p.events.is_empty());
        let (p, _) = permission(true, run("read_file"), options());
        assert_eq!(chosen(&p), "no");
    }

    #[test]
    fn a_kimi_title_names_a_plenipo_tool_except_for_a_shell_command() {
        // Kimi (ADR-027) names Plenipo's tool bare in the title. A shell command is never one
        // of Plenipo's for it, however the request is named.
        for call in [
            json!({ "toolCallId": "c1", "title": "run_command", "kind": "execute" }),
            json!({ "toolCallId": "c2", "title": "mcp__plenipo__run_command", "kind": "execute" }),
            json!({ "toolCallId": "c3", "title": "Bash", "kind": "execute",
                    "_meta": { "toolName": "mcp__plenipo__run_command" } }),
        ] {
            let (p, _) = permission_on(kimi_task(), call.clone(), options());
            assert_eq!(chosen(&p), "no", "{call}");
            assert!(
                matches!(
                    &p.events[..],
                    [AgentEvent::Notice { text, .. }]
                        if text.contains("own shell") && text.contains("run_command")
                ),
                "{call}: {:?}",
                p.events
            );
        }

        // Any other kind: allowed for this one action, and the activity says which tool and why.
        let read = json!({ "toolCallId": "c4", "title": "read_file", "kind": "other" });
        let (p, _) = permission_on(kimi_task(), read, options());
        assert_eq!(chosen(&p), "yes", "once, never for the session");
        assert!(
            matches!(
                &p.events[..],
                [AgentEvent::Notice { level: NoticeLevel::Info, text }]
                    if text.contains("read_file") && text.contains("allowed")
            ),
            "{:?}",
            p.events
        );
    }

    /// An allowed change of the tool's own must come through Plenipo as a write for the file it
    /// named, carried out. A write clears only that change: a second allowed change that never
    /// comes is still caught when the tool reports it done, and the task stops. Only the same
    /// file counts (a file of the same name elsewhere does not), a refused write clears
    /// nothing, and when too many changes wait, the next one is refused rather than an older
    /// one forgotten.
    #[test]
    fn each_allowed_change_must_come_through_plenipo_on_its_own() {
        let edit = |id: &str, path: &str| {
            json!({ "toolCallId": id, "title": "Edit", "kind": "edit",
                    "locations": [{ "path": path }], "rawInput": { "path": path } })
        };
        let ask = |t: &mut AcpTurn, call: Value| {
            let p = t.line(
                &json!({ "jsonrpc": "2.0", "id": 77, "method": "session/request_permission",
                         "params": { "sessionId": "s-1", "toolCall": call, "options": options() } })
                .to_string(),
                false,
            );
            chosen(&p)
        };
        // The write comes to Plenipo, which answers with how it went.
        let write = |t: &mut AcpTurn, path: &str, answer: FileAnswer| {
            let p = t.line(
                &json!({ "jsonrpc": "2.0", "id": "w", "method": "fs/write_text_file",
                         "params": { "sessionId": "s-1", "path": path, "content": "x" } })
                .to_string(),
                false,
            );
            let id = p.files[0].id;
            t.file_answered(id, answer);
        };
        let done = |id: &str| {
            notify(
                json!({ "sessionUpdate": "tool_call_update", "toolCallId": id,
                           "title": "Edit", "status": "completed" }),
            )
        };
        let mut t = prompting_on(kimi_task());
        assert_eq!(ask(&mut t, edit("c1", "/work/conversation/a.txt")), "yes");
        assert_eq!(ask(&mut t, edit("c2", "/work/conversation/b.txt")), "yes");
        // The first comes through (a path relative to the folder is the same file).
        write(&mut t, "a.txt", Ok(String::new()));
        assert!(t.line(&done("c1"), false).stop.is_none());
        // The second never comes: the task stops when the tool says it is done.
        let stop = t.line(&done("c2"), false).stop.expect("the task stops");
        assert_eq!(stop.outcome, TurnOutcome::Failed);
        assert!(
            stop.reason.contains("did not go through Plenipo"),
            "{}",
            stop.reason
        );

        // A file of the same name in another folder, and a write Plenipo refused, clear nothing.
        let mut t = prompting_on(kimi_task());
        assert_eq!(
            ask(&mut t, edit("c3", "/work/conversation/sub/a.txt")),
            "yes"
        );
        write(&mut t, "a.txt", Ok(String::new()));
        write(
            &mut t,
            "sub/a.txt",
            Err("Blocked: a.txt is a blocked file".into()),
        );
        assert!(t.line(&done("c3"), false).stop.is_some());

        // A change that named no file is cleared by a write for any file; a write for a file no
        // change named clears one that named none, never a named one.
        let mut t = prompting_on(kimi_task());
        assert_eq!(ask(&mut t, edit("c4", "/work/conversation/d.txt")), "yes");
        assert_eq!(
            ask(
                &mut t,
                json!({ "toolCallId": "c5", "title": "Edit", "kind": "edit" })
            ),
            "yes"
        );
        write(&mut t, "/work/conversation/z.txt", Ok(String::new()));
        assert!(t.line(&done("c5"), false).stop.is_none());
        assert!(t.line(&done("c4"), false).stop.is_some());

        // Too many changes waiting: the next is refused, and the oldest is still checked.
        let mut t = prompting_on(kimi_task());
        for i in 0..MAX_UNWRITTEN {
            assert_eq!(
                ask(&mut t, edit(&format!("m{i}"), &format!("f{i}.txt"))),
                "yes"
            );
        }
        assert_eq!(ask(&mut t, edit("over", "over.txt")), "no");
        assert!(t.line(&done("m0"), false).stop.is_some());
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
