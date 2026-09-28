//! Codex adapter (ADR-007).
//!
//! One turn = `codex exec --json --sandbox read-only --skip-git-repo-check
//! -c features.shell_tool=false -c features.view_image=false [resume <thread>]` with the
//! objective on stdin — the invocation OpenAI's Codex SDK uses, plus the two settings that
//! switch off Codex's own command tool and picture reader (ADR-051, Codex works through
//! Plenipo's tools). The thread ID arrives in `thread.started`. Codex does not report its
//! credential source in the stream, so a subscription sign-in must be positively confirmed
//! before every turn.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::adapter::{
    cap, first_line, model_name, plain_name, talk_answer, tool_summary, NewestVersion, Parsed,
    ProbeOutput, ProcessEnd, ProviderSession, PublishedList, RuntimeAdapter, StatusCheck,
    TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT, MAX_SUMMARY, NETWORK_ENV,
};
use crate::agent::claude_code::epoch_ms;
use crate::agent::discovery::{npm_target_triple, HostEnv};
use crate::agent::dto::{
    AccountAction, AgentEvent, AuthState, AuthStatus, Effort, KnownModel, NoticeLevel, PlanReport,
    PlanWindow, RuntimeCapabilities, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "codex";
/// Effort levels of Codex's models: up to extra high, max, or ultra.
const XHIGH: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High, Effort::XHigh];
const MAX: &[Effort] = &[
    Effort::Low,
    Effort::Medium,
    Effort::High,
    Effort::XHigh,
    Effort::Max,
];
const ULTRA: &[Effort] = &[
    Effort::Low,
    Effort::Medium,
    Effort::High,
    Effort::XHigh,
    Effort::Max,
    Effort::Ultra,
];
const LABEL: &str = "Codex";

#[derive(Debug, Default, Clone, Copy)]
pub struct Codex;

/// Native binary vendored inside the `@openai/codex` npm package rooted at `package`.
fn vendored_binary(package: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    let path = package
        .join("vendor")
        .join(npm_target_triple()?)
        .join("codex")
        .join(name);
    path.is_file().then_some(path)
}

impl RuntimeAdapter for Codex {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        "openai"
    }

    fn provider_label(&self) -> &'static str {
        "OpenAI"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: false,
            resume: true,
            cancel: true,
            structured_results: true,
            billing_checked_per_turn: false,
            // ADR-051 (Codex works through Plenipo's tools).
            tool_posture: "Codex's own commands are off: it cannot run commands or read files \
                           on its own, and its read-only sandbox allows no writes and no \
                           network. A worker with permissions gets Plenipo's file, program, \
                           and git tools, each checked by Plenipo Guard."
                .into(),
            // `codex exec -c model_reasoning_effort=<level>`: every level one of its models
            // accepts. Codex passes any value on, so Plenipo keeps to these.
            effort_levels: ULTRA.to_vec(),
            // The models Codex's own model picker lists (Codex 0.157.1), in its order, with the
            // effort levels each accepts.
            known_models: vec![
                KnownModel::new("gpt-6-astra", "GPT-6-Astra", ULTRA),
                KnownModel::new("gpt-6-sol", "GPT-6-Sol", ULTRA),
                KnownModel::new("gpt-6-luna", "GPT-6-Luna", MAX),
                KnownModel::new("gpt-5.6-sol", "GPT-5.6-Sol", ULTRA),
                KnownModel::new("gpt-5.6-terra", "GPT-5.6-Terra", ULTRA),
                KnownModel::new("gpt-5.6-luna", "GPT-5.6-Luna", MAX),
                KnownModel::new("gpt-5.5", "GPT-5.5", XHIGH),
            ],
        }
    }

    fn checked_version(&self) -> &'static str {
        "0.157.1"
    }

    fn install_hint(&self) -> &'static str {
        "Install the Codex CLI: npm install -g @openai/codex (requires Node.js). Then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Open a terminal, run: codex login — and choose Sign in with ChatGPT. Plenipo never asks \
         for your password. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "codex"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if cfg!(windows) {
            if let Some(appdata) = &host.appdata {
                let package = appdata
                    .join("npm")
                    .join("node_modules")
                    .join("@openai")
                    .join("codex");
                out.extend(vendored_binary(&package));
            }
            // OpenAI's standalone installer for Windows (its install page), which `codex update`
            // keeps up to date.
            if let Some(local) = host.var("LOCALAPPDATA") {
                out.push(
                    PathBuf::from(local)
                        .join("Programs")
                        .join("OpenAI")
                        .join("Codex")
                        .join("bin")
                        .join("codex.exe"),
                );
            }
        } else {
            if let Some(home) = &host.home {
                out.push(home.join(".local").join("bin").join("codex"));
            }
            out.extend(host.system_dirs().iter().map(|d| d.join("codex")));
        }
        out
    }

    fn resolve(&self, found: &Path) -> Option<PathBuf> {
        let ext = found
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        if cfg!(windows) {
            return match ext.as_deref() {
                Some("exe") => Some(found.to_path_buf()),
                // npm shim: run the native binary it would start, as the Codex SDK does.
                Some("cmd" | "ps1" | "bat") => {
                    let package = found
                        .parent()?
                        .join("node_modules")
                        .join("@openai")
                        .join("codex");
                    vendored_binary(&package)
                }
                _ => None,
            };
        }
        // npm links `codex` to the package's `bin/codex.js` launcher; prefer its native binary.
        let canonical = dunce::canonicalize(found).ok()?;
        if canonical.file_name().is_some_and(|n| n == "codex.js") {
            if let Some(native) = canonical
                .parent()
                .and_then(Path::parent)
                .and_then(vendored_binary)
            {
                return Some(native);
            }
        }
        Some(found.to_path_buf())
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["login".into(), "status".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        ["CODEX_HOME"]
            .into_iter()
            .chain(NETWORK_ENV.iter().copied())
            .collect()
    }

    /// OpenAI's command reference: `codex login` (with no flags, the ChatGPT sign-in in the
    /// browser) and `codex logout`. Never `--with-api-key` or `--with-access-token` (ADR-058).
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        Some(match action {
            AccountAction::SignIn => vec!["login".into()],
            AccountAction::SignOut => vec!["logout".into()],
        })
    }

    /// OpenAI publishes each Codex release on npm, one of its documented install and update
    /// paths, with the number `codex --version` reports.
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::Published(PublishedList::Npm("@openai/codex"))
    }

    /// OpenAI's command reference: `codex update`, "when the installed release supports
    /// self-update".
    fn update_command(&self) -> Option<Vec<String>> {
        Some(vec!["update".into()])
    }

    /// OpenAI's installer settings: prompts take their default answer.
    fn update_env(&self) -> Vec<(String, String)> {
        vec![("CODEX_NON_INTERACTIVE".into(), "1".into())]
    }

    fn update_by_hand(&self) -> Option<&'static str> {
        Some(
            "Codex installed with npm updates with npm: open a terminal and type \
             npm install -g @openai/codex",
        )
    }

    /// Codex's app server (OpenAI's documented way for apps to read an account's limits and
    /// models, ADR-060 §3): `initialize`, `account/read`, `account/rateLimits/read`, and
    /// `model/list`, with no conversation and no task.
    fn status_check(&self, _dir: &Path) -> StatusCheck {
        let version = env!("CARGO_PKG_VERSION");
        let lines = [
            serde_json::json!({ "method": "initialize", "id": 1, "params": {
                "clientInfo": { "name": "plenipo", "title": "Plenipo", "version": version } } }),
            serde_json::json!({ "method": "initialized" }),
            serde_json::json!({ "method": "account/read", "id": 2,
                                "params": { "refreshToken": false } }),
            serde_json::json!({ "method": "account/rateLimits/read", "id": 3 }),
            serde_json::json!({ "method": "model/list", "id": 4,
                                "params": { "limit": 100, "includeHidden": false } }),
        ];
        StatusCheck::Talk {
            args: vec![
                "-c".into(),
                "check_for_update_on_startup=false".into(),
                "app-server".into(),
            ],
            lines: lines.iter().map(ToString::to_string).collect(),
            answers: vec![1, 2, 3, 4],
        }
    }

    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        parse_models(out)
    }

    fn parse_plan(&self, out: &ProbeOutput) -> Option<PlanReport> {
        parse_plan(out, crate::now_ms())
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let mut args: Vec<String> = [
            "exec",
            "--json",
            // Phase 3: no writes, no network (ADR-007 §5).
            "--sandbox",
            "read-only",
            // Each session runs in its own empty workspace, not a repository.
            "--skip-git-repo-check",
            // ADR-051 (Codex works through Plenipo's tools): Codex's own command tool
            // (`exec_command`, `write_stdin`) and its picture reader (`view_image`) are off, so
            // it reads files only through Plenipo's tools, where Guard decides and the use is
            // recorded. Both are stable `features` settings of Codex 0.157.1; the read-only
            // sandbox stays as a second wall.
            "-c",
            "features.shell_tool=false",
            "-c",
            "features.view_image=false",
            // ADR-059 §9: Codex does not look for its own updates during a task; Plenipo updates
            // it between tasks (OpenAI's documented `check_for_update_on_startup`).
            "-c",
            "check_for_update_on_startup=false",
        ]
        .map(String::from)
        .to_vec();
        if let Some(model) = &request.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(effort) = request.effort {
            args.extend([
                "-c".into(),
                format!("model_reasoning_effort={}", effort.as_str()),
            ]);
        }
        if let Some(tools) = &request.tools {
            // Plenipo's tool server (Phase 7), as TOML values Codex parses.
            let key = format!("mcp_servers.{}", tools.name);
            let list = tools
                .args
                .iter()
                .map(|a| toml_string(a))
                .collect::<Vec<_>>()
                .join(", ");
            args.extend([
                "-c".into(),
                format!(
                    "{key}.command={}",
                    toml_string(&tools.command.display().to_string())
                ),
                "-c".into(),
                format!("{key}.args=[{list}]"),
                "-c".into(),
                format!("{key}.startup_timeout_sec=30"),
                "-c".into(),
                format!(
                    "{key}.tool_timeout_sec={}",
                    tools.call_timeout.as_secs().max(1)
                ),
            ]);
        }
        if let ProviderSession::Resume { id } = &request.session {
            args.extend(["resume".into(), id.clone()]);
        }
        args
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        let expected = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { .. } => None,
        };
        Box::new(Parser {
            state: TurnState::new(LABEL),
            expected,
        })
    }
}

/// A TOML basic string (quoted, with `\\`, `"`, and control characters escaped).
fn toml_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Codex's app server's `model/list` answer (`data`: each model's `model`, `displayName`, and
/// `supportedReasoningEfforts`).
fn parse_models(out: &ProbeOutput) -> Option<Vec<KnownModel>> {
    let list = talk_answer(out, 4)?;
    let data = list.get("data")?.as_array()?;
    Some(
        data.iter()
            .filter(|m| !m.get("hidden").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|m| {
                let name = m
                    .get("model")
                    .or_else(|| m.get("id"))
                    .and_then(Value::as_str)
                    .and_then(model_name)?;
                let label = m
                    .get("displayName")
                    .and_then(Value::as_str)
                    .and_then(|l| plain_name(l, 64))
                    .unwrap_or_else(|| name.clone());
                let mut efforts: Vec<Effort> = m
                    .get("supportedReasoningEfforts")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(|e| {
                                e.get("reasoningEffort")
                                    .or(Some(e))
                                    .and_then(Value::as_str)
                                    .and_then(Effort::parse)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                efforts.sort();
                efforts.dedup();
                Some(KnownModel {
                    name,
                    label,
                    effort_levels: efforts,
                })
            })
            .collect(),
    )
}

/// Codex's app server's `account/rateLimits/read` answer: each window's share used, length, and
/// reset time, as Codex reports them, and the plan's name from `account/read` (never the
/// email it also carries).
fn parse_plan(out: &ProbeOutput, now: u64) -> Option<PlanReport> {
    let limits = talk_answer(out, 3)?;
    let limits = limits.get("rateLimits")?;
    let windows: Vec<PlanWindow> = ["primary", "secondary"]
        .iter()
        .filter_map(|key| limits.get(*key).filter(|w| w.is_object()))
        .map(|w| PlanWindow {
            minutes: w.get("windowDurationMins").and_then(Value::as_u64),
            used_percent: w
                .get("usedPercent")
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite() && *p >= 0.0)
                .map(|p| p.round().min(100.0) as u8),
            resets_at: w.get("resetsAt").and_then(Value::as_f64).and_then(epoch_ms),
        })
        .collect();
    let plan = talk_answer(out, 2)
        .and_then(|a| {
            a.pointer("/account/planType")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            limits
                .get("planType")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|p| plain_name(&p, 32));
    let limited = limits
        .get("rateLimitReachedType")
        .is_some_and(|t| !t.is_null());
    Some(PlanReport {
        windows,
        limited,
        warning: false,
        plan,
        reported_at: now,
    })
}

fn parse_auth(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<&str>, detail: Option<String>| AuthStatus {
        state,
        method: method.map(str::to_owned),
        detail,
    };
    if let Some(e) = &out.spawn_error {
        return status(
            AuthState::Unknown,
            None,
            Some(format!("The sign-in check could not start: {e}")),
        );
    }
    if out.timed_out {
        return status(
            AuthState::Unknown,
            None,
            Some("The sign-in check timed out.".into()),
        );
    }
    // The output of `codex login status` can include a masked key: classify it, never keep it.
    let text = out.combined().to_ascii_lowercase();
    if text.contains("not logged in") || text.contains("not signed in") {
        status(AuthState::SignedOut, None, None)
    } else if text.contains("chatgpt") {
        status(AuthState::Subscription, Some("ChatGPT sign-in"), None)
    } else if text.contains("api key") {
        status(
            AuthState::ApiKey,
            Some("OpenAI API key"),
            Some("Signed in with an API key: usage would be billed to the API.".into()),
        )
    } else if out.exit_code == Some(0) {
        status(
            AuthState::Unverified,
            None,
            Some("Signed in, but the billing method was not recognized.".into()),
        )
    } else {
        status(
            AuthState::Unknown,
            None,
            Some("Codex did not report its sign-in status.".into()),
        )
    }
}

struct Parser {
    state: TurnState,
    expected: Option<String>,
}

fn item_type(item: &Value) -> Option<&str> {
    item.get("type")
        .or_else(|| item.get("item_type"))
        .and_then(Value::as_str)
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Codex passes some service errors on as the service's own JSON body, for example
/// `{"type":"error","status":400,"error":{"message":"The 'x' model is not supported…"}}`,
/// sometimes with text before or after it. Show the sentence inside it instead of the raw JSON.
fn readable(message: &str) -> String {
    let inner = message.find('{').and_then(|start| {
        // The first JSON value from the brace on; anything after it is ignored.
        let v = serde_json::Deserializer::from_str(&message[start..])
            .into_iter::<Value>()
            .next()?
            .ok()?;
        ["/error/message", "/message", "/detail"]
            .iter()
            .find_map(|p| v.pointer(p).and_then(Value::as_str).map(str::to_owned))
    });
    match inner {
        Some(m) if !m.trim().is_empty() => readable(&m),
        _ => message.to_owned(),
    }
}

impl Parser {
    fn thread_started(&mut self, v: &Value) -> Parsed {
        let id = v
            .get("thread_id")
            .and_then(Value::as_str)
            .map(|s| cap(s, 128));
        let mut parsed = Parsed::one(AgentEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: None,
        });
        if let (Some(expected), Some(reported)) = (&self.expected, &id) {
            if expected != reported {
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: format!(
                        "Codex reported thread {reported} instead of {expected}; Plenipo will resume {reported}."
                    ),
                });
            }
        }
        if id.is_some() {
            self.state.provider_session_id = id;
        }
        parsed
    }

    fn item_started(item: &Value) -> Parsed {
        match item_type(item) {
            Some("command_execution") => Parsed::one(AgentEvent::ToolUse {
                tool: "shell".into(),
                summary: first_line(&str_of(item, "command").replace('\n', " "), MAX_SUMMARY),
            }),
            Some("mcp_tool_call") => Parsed::one(AgentEvent::ToolUse {
                tool: cap(
                    &format!("{}/{}", str_of(item, "server"), str_of(item, "tool")),
                    80,
                ),
                summary: tool_summary(item.get("arguments").unwrap_or(&Value::Null)),
            }),
            _ => Parsed::none(),
        }
    }

    fn item_completed(&mut self, item: &Value) -> Parsed {
        let status = str_of(item, "status");
        match item_type(item) {
            Some("agent_message") => {
                let text = str_of(item, "text");
                if text.trim().is_empty() {
                    return Parsed::none();
                }
                self.state.last_message = Some(text.to_owned());
                Parsed::one(AgentEvent::Message {
                    text: text.to_owned(),
                })
            }
            Some("reasoning") => Parsed::one(AgentEvent::Reasoning {
                text: cap(str_of(item, "text"), MAX_EVENT_TEXT),
            }),
            Some("command_execution") => {
                let code = item.get("exit_code").and_then(Value::as_i64);
                Parsed::one(AgentEvent::ToolResult {
                    tool: Some("shell".into()),
                    is_error: code != Some(0) || matches!(status, "failed" | "declined"),
                    summary: code.map_or_else(|| status.to_owned(), |c| format!("exit {c}")),
                })
            }
            Some("mcp_tool_call") => Parsed::one(AgentEvent::ToolResult {
                tool: Some(cap(
                    &format!("{}/{}", str_of(item, "server"), str_of(item, "tool")),
                    80,
                )),
                is_error: status == "failed",
                summary: status.to_owned(),
            }),
            Some("file_change") => {
                let paths: Vec<&str> = item
                    .get("changes")
                    .and_then(Value::as_array)
                    .map(|c| c.iter().map(|c| str_of(c, "path")).collect())
                    .unwrap_or_default();
                Parsed::one(AgentEvent::ToolUse {
                    tool: "file change".into(),
                    summary: first_line(&paths.join(", "), MAX_SUMMARY),
                })
            }
            Some("web_search") => Parsed::one(AgentEvent::ToolUse {
                tool: "web search".into(),
                summary: first_line(str_of(item, "query"), MAX_SUMMARY),
            }),
            Some("todo_list") => {
                let n = item
                    .get("items")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                Parsed::one(AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    text: format!("Codex updated its plan ({n} steps)"),
                })
            }
            Some("error") => {
                let message = cap(&readable(str_of(item, "message")), MAX_EVENT_TEXT);
                self.state.last_warning = Some(message.clone());
                Parsed::one(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: message,
                })
            }
            _ => Parsed::none(),
        }
    }
}

impl TurnParser for Parser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let parsed = match v.get("type").and_then(Value::as_str) {
            Some("thread.started") => self.thread_started(&v),
            Some("turn.started") | Some("item.updated") => Parsed::none(),
            Some("item.started") => Self::item_started(v.get("item").unwrap_or(&Value::Null)),
            Some("item.completed") => self.item_completed(v.get("item").unwrap_or(&Value::Null)),
            Some("turn.completed") => {
                self.state.completed = true;
                match v.get("usage").filter(|u| u.is_object()) {
                    Some(u) => {
                        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
                        let usage = TokenUsage {
                            input_tokens: n("input_tokens"),
                            cached_input_tokens: n("cached_input_tokens"),
                            output_tokens: n("output_tokens"),
                        };
                        self.state.usage = Some(usage);
                        Parsed::one(AgentEvent::Usage { usage })
                    }
                    None => Parsed::none(),
                }
            }
            Some("turn.failed") => {
                let message = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map_or_else(|| "Codex reported that the task failed".into(), readable);
                self.state.error = Some(cap(&message, MAX_EVENT_TEXT));
                Parsed::none()
            }
            Some("error") => {
                let message = cap(&readable(str_of(&v, "message")), MAX_EVENT_TEXT);
                self.state.last_warning = Some(message.clone());
                Parsed::one(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: message,
                })
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
        // A stream-level error with no turn outcome explains the failure.
        if !self.state.completed && self.state.error.is_none() {
            self.state.error = self.state.last_warning.take();
        }
        self.state.finish(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::dto::TurnOutcome;
    use crate::dto::ExecutionState;
    use serde_json::json;

    fn probe(text: &str, code: i32) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(code),
            stderr: text.into(),
            ..ProbeOutput::default()
        }
    }

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(9),
        }
    }

    fn new_request() -> TurnRequest {
        TurnRequest {
            session: ProviderSession::New { preassigned: None },
            model: None,
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
        }
    }

    fn feed(p: &mut dyn TurnParser, lines: &[Value]) -> Vec<AgentEvent> {
        lines
            .iter()
            .flat_map(|l| p.line(&l.to_string(), false).events)
            .collect()
    }

    #[test]
    fn plenipo_tools_are_passed_as_codex_settings() {
        use crate::agent::tools::ToolServer;
        let tools = ToolServer {
            name: "plenipo".into(),
            command: r#"C:\Program Files\Plenipo "x"\plenipo.exe"#.into(),
            args: vec![r#"--plenipo-tools=C:\Users\me\t.json"#.into()],
            config_file: "unused".into(),
            call_timeout: std::time::Duration::from_secs(3600),
            tools: Vec::new(),
        };
        let args = Codex.turn_args(&TurnRequest {
            session: ProviderSession::Resume { id: "t1".into() },
            model: None,
            effort: None,
            billing_confirmed: true,
            tools: Some(tools),
            working_dir: PathBuf::new(),
        });
        let settings: Vec<&String> = args
            .iter()
            .enumerate()
            .filter(|(i, _)| *i > 0 && args[i - 1] == "-c")
            .map(|(_, a)| a)
            .collect();
        assert!(
            settings.contains(
                &&r#"mcp_servers.plenipo.command="C:\\Program Files\\Plenipo \"x\"\\plenipo.exe""#
                    .to_owned()
            ),
            "{settings:?}"
        );
        assert!(
            settings.contains(
                &&r#"mcp_servers.plenipo.args=["--plenipo-tools=C:\\Users\\me\\t.json"]"#
                    .to_owned()
            ),
            "{settings:?}"
        );
        assert!(settings.contains(&&"mcp_servers.plenipo.tool_timeout_sec=3600".to_owned()));
        // Still its read-only sandbox, and the session comes last.
        let s = args.iter().position(|a| a == "--sandbox").unwrap();
        assert_eq!(args[s + 1], "read-only");
        assert!(args.ends_with(&["resume".into(), "t1".into()]));
        assert_eq!(toml_string("a\u{1}b"), "\"a\\u0001b\"");
    }
    #[test]
    fn turn_arguments_match_the_sdk_shape() {
        assert_eq!(
            Codex.turn_args(&new_request()),
            [
                "exec",
                "--json",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "-c",
                "features.shell_tool=false",
                "-c",
                "features.view_image=false",
                "-c",
                "check_for_update_on_startup=false"
            ]
        );
        let resume = Codex.turn_args(&TurnRequest {
            session: ProviderSession::Resume { id: "t-1".into() },
            model: Some("gpt-x".into()),
            effort: Some(Effort::Ultra),
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
        });
        assert_eq!(
            resume,
            [
                "exec",
                "--json",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "-c",
                "features.shell_tool=false",
                "-c",
                "features.view_image=false",
                "-c",
                "check_for_update_on_startup=false",
                "--model",
                "gpt-x",
                "-c",
                "model_reasoning_effort=ultra",
                "resume",
                "t-1"
            ]
        );
        assert!(!Codex.preassigns_session_id());
    }

    #[test]
    fn codex_own_commands_are_switched_off() {
        // ADR-051: Codex's own command tool and its own picture reader are off in every turn,
        // as settings Codex reads before it starts (`features.shell_tool`, `features.view_image`).
        for request in [
            new_request(),
            TurnRequest {
                session: ProviderSession::Resume { id: "t-1".into() },
                model: Some("gpt-x".into()),
                effort: Some(Effort::High),
                billing_confirmed: true,
                tools: None,
                working_dir: PathBuf::new(),
            },
        ] {
            let args = Codex.turn_args(&request);
            let settings: Vec<&str> = args
                .iter()
                .enumerate()
                .filter(|(i, _)| *i > 0 && args[i - 1] == "-c")
                .map(|(_, a)| a.as_str())
                .collect();
            assert!(settings.contains(&"features.shell_tool=false"), "{args:?}");
            assert!(settings.contains(&"features.view_image=false"), "{args:?}");
            // Part of the fixed prefix: before the model and before the session.
            let shell = args
                .iter()
                .position(|a| a == "features.shell_tool=false")
                .unwrap();
            if let Some(model) = args.iter().position(|a| a == "--model") {
                assert!(shell < model, "{args:?}");
            }
            if let Some(resume) = args.iter().position(|a| a == "resume") {
                assert!(shell < resume, "{args:?}");
            }
        }
        // The posture the owner reads says so.
        let posture = Codex.capabilities().tool_posture;
        assert!(posture.contains("own commands are off"), "{posture}");
        assert!(
            posture.contains("Plenipo's file, program, and git tools"),
            "{posture}"
        );
    }

    #[test]
    fn known_models_are_valid_names_within_codex_effort_levels() {
        let caps = Codex.capabilities();
        assert_eq!(caps.known_models[0].name, "gpt-6-astra");
        for m in &caps.known_models {
            assert_eq!(
                crate::agent::service::validate_model(&m.name).unwrap(),
                m.name
            );
            assert!(m
                .effort_levels
                .iter()
                .all(|e| caps.effort_levels.contains(e)));
        }
        // No Codex model takes "minimal"; only some take "ultra".
        assert!(!caps.effort_levels.contains(&Effort::Minimal));
        assert!(!caps
            .effort_levels_for(Some("gpt-6-luna"))
            .contains(&Effort::Ultra));
    }

    #[test]
    fn login_status_classification() {
        let s = parse_auth(&probe("Logged in using ChatGPT", 0));
        assert_eq!(s.state, AuthState::Subscription);
        let s = parse_auth(&probe("Logged in using an API key - sk-proj-***ABCDE", 0));
        assert_eq!(s.state, AuthState::ApiKey);
        assert!(!format!("{s:?}").contains("ABCDE"), "masked key never kept");
        assert_eq!(
            parse_auth(&probe("Not logged in", 1)).state,
            AuthState::SignedOut
        );
        assert_eq!(
            parse_auth(&probe("Logged in", 0)).state,
            AuthState::Unverified
        );
        assert_eq!(
            parse_auth(&probe("error: unexpected argument", 2)).state,
            AuthState::Unknown
        );
    }

    #[test]
    fn plenipo_tool_calls_say_what_they_do() {
        let mut p = Codex.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"item.started","item":{"id":"m1","type":"mcp_tool_call","server":"plenipo","tool":"run_command","arguments":{"program":"git","args":["--version"]},"status":"in_progress"}}),
            ],
        );
        assert_eq!(
            events,
            [AgentEvent::ToolUse {
                tool: "plenipo/run_command".into(),
                summary: "git --version".into()
            }]
        );
    }

    #[test]
    fn successful_stream_is_normalized() {
        let mut p = Codex.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"thread.started","thread_id":"t-1"}),
                json!({"type":"turn.started"}),
                json!({"type":"item.started","item":{"id":"i1","type":"command_execution","command":"bash -lc ls","status":"in_progress"}}),
                json!({"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"bash -lc ls","exit_code":0,"status":"completed"}}),
                json!({"type":"item.completed","item":{"id":"i2","type":"reasoning","text":"thinking"}}),
                json!({"type":"item.completed","item":{"id":"i3","type":"agent_message","text":"Done."}}),
                json!({"type":"something.new"}),
                json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":12}}),
            ],
        );
        assert_eq!(
            events,
            [
                AgentEvent::SessionStarted {
                    provider_session_id: Some("t-1".into()),
                    model: None
                },
                AgentEvent::ToolUse {
                    tool: "shell".into(),
                    summary: "bash -lc ls".into()
                },
                AgentEvent::ToolResult {
                    tool: Some("shell".into()),
                    is_error: false,
                    summary: "exit 0".into()
                },
                AgentEvent::Reasoning {
                    text: "thinking".into()
                },
                AgentEvent::Message {
                    text: "Done.".into()
                },
                AgentEvent::Usage {
                    usage: TokenUsage {
                        input_tokens: 100,
                        cached_input_tokens: 40,
                        output_tokens: 12
                    }
                },
            ]
        );
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("Done."));
        assert_eq!(r.provider_session_id.as_deref(), Some("t-1"));
        assert_eq!(r.ignored_lines, 1);
    }

    #[test]
    fn failures_are_classified() {
        let mut p = Codex.parser(&new_request());
        feed(
            p.as_mut(),
            &[
                json!({"type":"thread.started","thread_id":"t-1"}),
                json!({"type":"turn.failed","error":{"message":"You've hit your usage limit. Try again later."}}),
            ],
        );
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::UsageLimited);

        // A stream error without a turn outcome explains the failure.
        let mut p = Codex.parser(&new_request());
        feed(
            p.as_mut(),
            &[json!({"type":"error","message":"stream disconnected before completion"})],
        );
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::ProviderUnavailable);

        let mut p = Codex.parser(&new_request());
        p.line("{not json", true);
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::MalformedOutput);
    }

    #[test]
    fn service_errors_read_as_plain_sentences() {
        // As the real Codex CLI reported an unsupported model (owner check, 2026-09-26).
        let body = r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-x' model is not supported when using Codex with a ChatGPT account."}}"#;
        let plain = "The 'gpt-x' model is not supported when using Codex with a ChatGPT account.";
        let mut p = Codex.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"thread.started","thread_id":"t-1"}),
                json!({"type":"turn.started"}),
                json!({"type":"error","message":body}),
                json!({"type":"turn.failed","error":{"message":body}}),
            ],
        );
        assert_eq!(
            events.last(),
            Some(&AgentEvent::Notice {
                level: NoticeLevel::Warning,
                text: plain.into()
            })
        );
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::Failed);
        assert_eq!(r.summary, format!("Codex reported an error: {plain}"));
        assert_eq!(r.error.as_deref(), Some(plain));

        // Text around the body does not hide the sentence.
        for wrapped in [
            format!("unexpected status 400 Bad Request: {body}"),
            format!("{body} (request id: req_123)"),
            format!("{body}\nretrying"),
        ] {
            assert_eq!(readable(&wrapped), plain, "{wrapped}");
        }

        // Plain messages and JSON without a message stay as they are.
        assert_eq!(readable("stream disconnected"), "stream disconnected");
        assert_eq!(readable(r#"{"status":500}"#), r#"{"status":500}"#);
        assert_eq!(readable("expected '{' at line 3"), "expected '{' at line 3");
    }

    #[test]
    fn environment_passes_no_credentials() {
        let host = HostEnv::default().with_vars(vec![
            ("OPENAI_API_KEY".into(), "sk".into()),
            ("CODEX_API_KEY".into(), "sk".into()),
            ("OPENAI_BASE_URL".into(), "http://x".into()),
            ("CODEX_HOME".into(), "/codex".into()),
        ]);
        let env = crate::agent::discovery::runtime_env(&Codex, &host);
        assert_eq!(env, [("CODEX_HOME".to_owned(), "/codex".to_owned())]);
    }

    #[cfg(unix)]
    #[test]
    fn npm_launcher_resolves_to_the_vendored_binary() {
        use std::os::unix::fs::PermissionsExt as _;
        let Some(triple) = npm_target_triple() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("lib/node_modules/@openai/codex");
        std::fs::create_dir_all(package.join("bin")).unwrap();
        let launcher = package.join("bin/codex.js");
        std::fs::write(&launcher, "#!/usr/bin/env node\n").unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink(&launcher, bin.join("codex")).unwrap();
        // Without a vendored binary, the launcher itself is used.
        assert_eq!(Codex.resolve(&bin.join("codex")), Some(bin.join("codex")));
        let native = package.join("vendor").join(triple).join("codex/codex");
        std::fs::create_dir_all(native.parent().unwrap()).unwrap();
        std::fs::write(&native, "").unwrap();
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            Codex.resolve(&bin.join("codex")),
            Some(
                dunce::canonicalize(&package)
                    .unwrap()
                    .join("vendor")
                    .join(triple)
                    .join("codex/codex")
            )
        );
    }
}

/// Phase 19: the AI tools page (ADR-058 to ADR-060).
#[cfg(test)]
mod ai_tools_page_tests {
    use super::*;
    use crate::agent::adapter::StatusCheck;

    /// What Codex's app server answers, as OpenAI's app server page shows it.
    fn talk() -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
            stdout: [
                r#"{"id":1,"result":{"userAgent":"codex"}}"#,
                r#"{"id":2,"result":{"account":{"type":"chatgpt","email":"owner@example.com","planType":"pro"},"requiresOpenaiAuth":true}}"#,
                r#"{"method":"account/rateLimits/updated","params":{}}"#,
                r#"{"id":3,"result":{"rateLimits":{"limitId":"codex","limitName":null,"primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":1730947200},"secondary":null,"rateLimitReachedType":null}}}"#,
                r#"{"id":4,"result":{"data":[{"id":"gpt-6-sol","model":"gpt-6-sol","displayName":"GPT-6 Sol","hidden":false,"supportedReasoningEfforts":[{"reasoningEffort":"medium","description":"x"},{"reasoningEffort":"low"}],"isDefault":true},{"id":"gpt-hidden","model":"gpt-hidden","displayName":"Hidden","hidden":true},{"id":"bad name!","model":"bad name!","displayName":"Bad"}],"nextCursor":null}}"#,
            ]
            .join("\n"),
            ..ProbeOutput::default()
        }
    }

    #[test]
    fn signs_in_and_out_and_updates_with_its_own_commands() {
        assert_eq!(
            Codex.account_command(AccountAction::SignIn),
            Some(vec!["login".to_owned()])
        );
        assert_eq!(
            Codex.account_command(AccountAction::SignOut),
            Some(vec!["logout".to_owned()])
        );
        assert_eq!(Codex.update_command(), Some(vec!["update".to_owned()]));
        assert_eq!(
            Codex.update_env(),
            vec![("CODEX_NON_INTERACTIVE".to_owned(), "1".to_owned())]
        );
        assert_eq!(Codex.put_back_command("0.157.1"), None);
        assert!(Codex
            .update_by_hand()
            .unwrap()
            .contains("npm install -g @openai/codex"));
    }

    #[test]
    fn its_app_server_is_asked_for_the_plan_and_the_models_with_no_task() {
        let StatusCheck::Talk {
            args,
            lines,
            answers,
        } = Codex.status_check(Path::new("/checks"))
        else {
            panic!("Codex talks to its app server");
        };
        assert_eq!(
            args,
            ["-c", "check_for_update_on_startup=false", "app-server"]
        );
        let methods: Vec<String> = lines
            .iter()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["method"].to_string())
            .collect();
        assert_eq!(
            methods,
            [
                "\"initialize\"",
                "\"initialized\"",
                "\"account/read\"",
                "\"account/rateLimits/read\"",
                "\"model/list\""
            ]
        );
        assert!(!lines
            .iter()
            .any(|l| l.contains("thread/") || l.contains("turn/")));
        assert_eq!(answers, [1, 2, 3, 4]);
    }

    #[test]
    fn reads_the_plan_and_the_models_it_reports_never_the_email() {
        let plan = Codex.parse_plan(&talk()).unwrap();
        assert_eq!(
            plan.windows,
            vec![PlanWindow {
                minutes: Some(300),
                used_percent: Some(25),
                resets_at: Some(1_730_947_200_000),
            }]
        );
        assert_eq!(plan.plan.as_deref(), Some("pro"));
        assert!(!plan.limited);
        assert!(!serde_json::to_string(&plan).unwrap().contains("owner@"));
        let models = Codex.parse_models(&talk()).unwrap();
        assert_eq!(
            models,
            vec![KnownModel::new(
                "gpt-6-sol",
                "GPT-6 Sol",
                &[Effort::Low, Effort::Medium]
            )]
        );
        assert_eq!(Codex.parse_models(&ProbeOutput::default()), None);
    }
}
