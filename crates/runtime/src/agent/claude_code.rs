//! Claude Code adapter (ADR-007).
//!
//! One turn = `claude -p --output-format stream-json …` with the objective on stdin. New
//! sessions get a Plenipo-chosen `--session-id`; follow-ups use `--resume`. It never gets its
//! built-in tools (`--tools ""`) or the owner's MCP servers (`--strict-mcp-config`); a worker with
//! permissions gets Plenipo's own tool server (Phase 7). The `init` event's credential source is
//! checked on every turn: anything but a subscription sign-in stops the turn.

use std::path::PathBuf;

use serde_json::Value;

use crate::agent::adapter::{
    cap, first_line, tool_summary, NewestVersion, Parsed, ProbeOutput, ProcessEnd, ProviderSession,
    PublishedList, RuntimeAdapter, Stop, TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT,
    NETWORK_ENV,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AccountAction, AgentEvent, AuthState, AuthStatus, Effort, KnownModel, NoticeLevel,
    PlanReport, PlanWindow, RuntimeCapabilities, StatusPhase, TurnOutcome, TurnResult,
};
use crate::agent::preview::{
    plenipo_write_tool, preview_of, PreviewPace, WriteTool, MAX_PREVIEW_JSON,
};
use crate::dto::TokenUsage;

pub const ID: &str = "claude-code";
/// `claude --effort`'s levels, which its current Fable, Opus, and Sonnet models all accept.
const FRONTIER_EFFORT: &[Effort] = &[
    Effort::Low,
    Effort::Medium,
    Effort::High,
    Effort::XHigh,
    Effort::Max,
];
const LABEL: &str = "Claude Code";

/// Credential source Claude Code reports when it uses a subscription (OAuth) sign-in.
const SUBSCRIPTION_SOURCE: &str = "none";

#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCode;

impl RuntimeAdapter for ClaudeCode {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        "anthropic"
    }

    fn provider_label(&self) -> &'static str {
        "Anthropic"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            billing_checked_per_turn: true,
            tool_posture: "No built-in tools and none of your MCP servers. A worker with \
                           permissions gets Plenipo's file, program, and git tools, each \
                           checked by Plenipo Guard."
                .into(),
            // `claude --effort <level>`.
            effort_levels: FRONTIER_EFFORT.to_vec(),
            // The model families `claude --model` accepts as aliases, each for its latest model,
            // and the exact model each points to (ADR-081 §8), both checked on the owner's PC
            // with Claude Code 2.1.284 and a Claude subscription (`system`/`init` reported the
            // exact name; each exact name ran by name). Haiku has no effort setting.
            known_models: vec![
                KnownModel::new("fable", "Fable", FRONTIER_EFFORT).now("claude-fable-5-1"),
                KnownModel::new("opus", "Opus", FRONTIER_EFFORT).now("claude-opus-5-5"),
                KnownModel::new("sonnet", "Sonnet", FRONTIER_EFFORT).now("claude-sonnet-5-5"),
                KnownModel::new("haiku", "Haiku", &[]).now("claude-haiku-4-5-20251001"),
                // The same models on your Anthropic key (Phase 25, item 4.4; ADR-254): an alias
                // is linked through the exact model it points to.
                KnownModel::new("claude-fable-5-1", "Fable 5.1", FRONTIER_EFFORT)
                    .same("claude-fable-5-1"),
                KnownModel::new("claude-opus-5-5", "Opus 5.5", FRONTIER_EFFORT)
                    .same("claude-opus-5-5"),
                KnownModel::new("claude-sonnet-5-5", "Sonnet 5.5", FRONTIER_EFFORT)
                    .same("claude-sonnet-5-5"),
                KnownModel::new("claude-haiku-4-5-20251001", "Haiku 4.5", &[])
                    .same("claude-haiku-4-5"),
            ]
            .into_iter()
            .map(|m| m.by(makers::ANTHROPIC))
            .collect(),
            default_maker: None,
            runs_other_makers: false,
        }
    }

    fn checked_version(&self) -> &'static str {
        "2.1.284"
    }

    fn install_hint(&self) -> &'static str {
        "Install the native Claude Code build. Windows (PowerShell): irm https://claude.ai/install.ps1 | iex — \
         macOS/Linux: curl -fsSL https://claude.ai/install.sh | bash. Then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Open a terminal, run: claude auth login — and sign in with your Claude subscription \
         account. Plenipo never asks for your password. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "claude"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(home) = &host.home {
            if cfg!(windows) {
                out.push(home.join(".local").join("bin").join("claude.exe"));
            } else {
                out.push(home.join(".local").join("bin").join("claude"));
                out.push(home.join(".claude").join("local").join("claude"));
            }
        }
        out.extend(host.system_dirs().iter().map(|d| d.join("claude")));
        out
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["auth".into(), "status".into(), "--json".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        ["CLAUDE_CONFIG_DIR", "CLAUDE_CODE_GIT_BASH_PATH"]
            .into_iter()
            .chain(NETWORK_ENV.iter().copied())
            .collect()
    }

    fn fixed_env(&self) -> Vec<(String, String)> {
        // Do not let the CLI replace itself in the middle of a Plenipo turn.
        vec![("DISABLE_AUTOUPDATER".into(), "1".into())]
    }

    fn preassigns_session_id(&self) -> bool {
        true
    }

    /// Its `compact_boundary` notice (ADR-044 §2.5).
    fn reports_memory_shortened(&self) -> bool {
        true
    }

    /// Anthropic's command-line reference: `claude auth login` and `claude auth logout`. Never
    /// `--console`, which signs in for pay-per-use API billing (ADR-058).
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        Some(match action {
            AccountAction::SignIn => vec!["auth".into(), "login".into()],
            AccountAction::SignOut => vec!["auth".into(), "logout".into()],
        })
    }

    /// Anthropic publishes each Claude Code release on npm, with the number its native build
    /// reports (2.1.283 on both, 2026-09-28).
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::Published(PublishedList::Npm("@anthropic-ai/claude-code"))
    }

    /// Anthropic's setup page: `claude update`. `DISABLE_AUTOUPDATER` stops only the background
    /// check, so it stays set.
    fn update_command(&self) -> Option<Vec<String>> {
        Some(vec!["update".into()])
    }

    /// `claude install <version>` installs that version of the native build again.
    fn put_back_command(&self, version: &str) -> Option<Vec<String>> {
        Some(vec!["install".into(), version.into()])
    }

    /// Documented: Claude Code's `rate_limit_event`; Codex's app server.
    fn reports_plan_left(&self) -> bool {
        true
    }

    fn update_by_hand(&self) -> Option<&'static str> {
        Some(
            "Claude Code installed with WinGet updates with WinGet: open a terminal and type \
             winget upgrade Anthropic.ClaudeCode",
        )
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let mut args: Vec<String> = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            // Phase 3: no built-in tools and no MCP servers (ADR-007 §5).
            "--tools",
            "",
            "--strict-mcp-config",
        ]
        .map(String::from)
        .to_vec();
        if let Some(model) = &request.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(effort) = request.effort {
            args.extend(["--effort".into(), effort.as_str().into()]);
        }
        // A summary of what the model thinks, so the owner sees it think (ADR-200). Only for a
        // version the option was checked on: an option an older one does not know stops it.
        if supports_thinking_display(request.cli_version.as_deref()) {
            args.extend(["--thinking-display".into(), "summarized".into()]);
        }
        if let Some(tools) = &request.tools {
            // Plenipo's tool server only (Phase 7): its tools need no prompt from Claude Code,
            // because Plenipo checks every call itself.
            args.extend([
                "--mcp-config".into(),
                tools.config_file.display().to_string(),
                "--allowedTools".into(),
                format!("mcp__{}", tools.name),
            ]);
        }
        match &request.session {
            ProviderSession::New {
                preassigned: Some(id),
            } => args.extend(["--session-id".into(), id.clone()]),
            ProviderSession::New { preassigned: None } => {}
            ProviderSession::Resume { id } => args.extend(["--resume".into(), id.clone()]),
        }
        args
    }

    fn turn_env(&self, request: &TurnRequest) -> Vec<(String, String)> {
        match &request.tools {
            // A tool call may wait for the owner's approval; the server starts quickly.
            Some(tools) => vec![
                (
                    "MCP_TOOL_TIMEOUT".into(),
                    tools.call_timeout.as_millis().max(1).to_string(),
                ),
                ("MCP_TIMEOUT".into(), "30000".into()),
            ],
            None => Vec::new(),
        }
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        let expected = match &request.session {
            ProviderSession::New { preassigned } => preassigned.clone(),
            ProviderSession::Resume { id } => Some(id.clone()),
        };
        Box::new(Parser {
            state: TurnState::new(LABEL),
            expected,
            billing_confirmed: request.billing_confirmed,
            init_seen: false,
            writing: std::collections::HashMap::new(),
            pace: PreviewPace::default(),
            rejected_reset: None,
        })
    }
}

/// Keep only characters safe to show as a short label (never an account identifier).
fn label_value(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'))
        .take(40)
        .collect()
}

fn first_str<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str))
}

/// The JSON object printed by `claude auth status --json`, tolerating extra text around it.
fn status_json(stdout: &str) -> Option<Value> {
    serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .or_else(|| {
            let start = stdout.find('{')?;
            let end = stdout.rfind('}')?;
            serde_json::from_str(stdout.get(start..=end)?).ok()
        })
        .filter(Value::is_object)
}

fn parse_auth(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<String>, detail: Option<&str>| AuthStatus {
        state,
        method,
        detail: detail.map(str::to_owned),
    };
    if let Some(e) = &out.spawn_error {
        return status(
            AuthState::Unknown,
            None,
            Some(&format!("The sign-in check could not start: {e}")),
        );
    }
    if out.timed_out {
        return status(
            AuthState::Unknown,
            None,
            Some("The sign-in check timed out."),
        );
    }
    let Some(json) = status_json(&out.stdout) else {
        let lower = out.combined().to_ascii_lowercase();
        if lower.contains("not logged in") || lower.contains("not signed in") {
            return status(AuthState::SignedOut, None, None);
        }
        return status(
            AuthState::Unknown,
            None,
            Some("This Claude Code version did not report its sign-in status; it will be checked when a task starts."),
        );
    };
    let logged_in = ["loggedIn", "logged_in", "isLoggedIn", "authenticated"]
        .iter()
        .find_map(|k| json.get(*k).and_then(Value::as_bool));
    if logged_in == Some(false) {
        return status(AuthState::SignedOut, None, None);
    }
    let provider = first_str(&json, &["apiProvider", "api_provider", "provider"])
        .unwrap_or("")
        .to_ascii_lowercase();
    if ["bedrock", "vertex", "foundry"]
        .iter()
        .any(|p| provider.contains(p))
    {
        return status(
            AuthState::ThirdPartyCloud,
            Some(label_value(&provider)),
            Some("Claude Code is configured for a third-party cloud, which bills that account."),
        );
    }
    let method = first_str(&json, &["authMethod", "auth_method", "method"]).unwrap_or("");
    let m = method.to_ascii_lowercase();
    let plan = first_str(&json, &["subscriptionType", "subscription_type"])
        .filter(|s| !s.is_empty() && *s != "null");
    if (m.contains("api") && m.contains("key")) || m.contains("console") || m == "apikey" {
        return status(
            AuthState::ApiKey,
            Some(label_value(method)),
            Some("Signed in with an API key: usage would be billed to the API."),
        );
    }
    if plan.is_some()
        || m.contains("claude.ai")
        || m.contains("oauth")
        || m.contains("subscription")
    {
        let label = match plan {
            Some(plan) => format!("Claude subscription ({})", label_value(plan)),
            None => "Claude subscription".into(),
        };
        return status(AuthState::Subscription, Some(label), None);
    }
    if logged_in == Some(true) {
        return status(
            AuthState::Unverified,
            (!method.is_empty()).then(|| label_value(method)),
            Some("Signed in; the billing method was not recognized. Each turn is checked before it runs."),
        );
    }
    status(AuthState::Unknown, None, None)
}

struct Parser {
    state: TurnState,
    /// The session ID Plenipo asked for (new or resumed).
    expected: Option<String>,
    /// The sign-in check confirmed a subscription before the turn.
    billing_confirmed: bool,
    init_seen: bool,
    /// Plenipo file changes the model is writing now, by content block (Watch, ADR-055): the
    /// tool call's ID, which change, and its arguments so far.
    writing: std::collections::HashMap<u64, (String, WriteTool, String)>,
    pace: PreviewPace,
    /// When the limit Claude Code last said was reached starts again (ms): added to the turn's
    /// error, so the hold waits for it (Phase 25, item 4.3).
    rejected_reset: Option<u64>,
}

impl Parser {
    /// Stop the turn: billing could not be confirmed before or during it (ADR-007 §4).
    fn unconfirmed_billing(&mut self) -> Parsed {
        let stop = Stop {
            outcome: TurnOutcome::BillingNotAllowed,
            reason: "Claude Code did not report which credential it uses, and its sign-in could \
                     not be confirmed as a subscription, so the turn was stopped (API billing is \
                     disabled). Check `claude auth status`."
                .into(),
        };
        self.state.stop = Some(stop.clone());
        Parsed {
            stop: Some(stop),
            ..Parsed::none()
        }
    }

    fn init(&mut self, v: &Value) -> Parsed {
        self.init_seen = true;
        let session = v
            .get("session_id")
            .and_then(Value::as_str)
            .map(|s| cap(s, 128));
        let model = v.get("model").and_then(Value::as_str).map(|s| cap(s, 128));
        let mut parsed = Parsed::one(AgentEvent::SessionStarted {
            provider_session_id: session.clone(),
            model: model.clone(),
        });
        if let (Some(expected), Some(reported)) = (&self.expected, &session) {
            if expected != reported {
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: format!(
                        "Claude Code reported session {reported} instead of {expected}; Plenipo will resume {reported}."
                    ),
                });
            }
        }
        self.state.provider_session_id = session.or(self.state.provider_session_id.take());
        self.state.model = model.or(self.state.model.take());
        let source = v.get("apiKeySource").and_then(Value::as_str);
        if source.is_none() && !self.billing_confirmed {
            let mut stopped = self.unconfirmed_billing();
            stopped.events.append(&mut parsed.events);
            return stopped;
        }
        if let Some(source) = source {
            if source != SUBSCRIPTION_SOURCE {
                let stop = Stop {
                    outcome: TurnOutcome::BillingNotAllowed,
                    reason: format!(
                        "Claude Code reported the credential source {:?} instead of a subscription sign-in, so the turn was stopped (API billing is disabled).",
                        cap(source, 64)
                    ),
                };
                self.state.stop = Some(stop.clone());
                parsed.stop = Some(stop);
            }
        }
        parsed
    }

    /// A Plenipo file change the model is still writing (Watch, ADR-055): Claude Code streams
    /// each tool call's arguments (`--include-partial-messages`) as `input_json_delta` pieces
    /// between `content_block_start` and `content_block_stop`.
    fn tool_input(&mut self, v: &Value) -> Parsed {
        let event = v.get("event").unwrap_or(&Value::Null);
        let index = event.get("index").and_then(Value::as_u64);
        let mut parsed = Parsed::none();
        match (event.get("type").and_then(Value::as_str), index) {
            // A new message: no block of an earlier one is still being written.
            (Some("message_start" | "message_stop"), _) => self.writing.clear(),
            (Some("content_block_start"), Some(index)) => {
                // Whatever this block is, an earlier block at the same place is over.
                self.writing.remove(&index);
                let block = event.get("content_block").unwrap_or(&Value::Null);
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    if let Some(tool) = block
                        .get("name")
                        .and_then(Value::as_str)
                        .and_then(plenipo_write_tool)
                    {
                        let id = block
                            .get("id")
                            .and_then(Value::as_str)
                            .map_or_else(|| format!("block-{index}"), |id| cap(id, 128));
                        self.writing.insert(index, (id, tool, String::new()));
                    }
                }
            }
            (Some("content_block_delta"), Some(index)) => {
                let piece = event
                    .pointer("/delta/partial_json")
                    .and_then(Value::as_str)
                    .filter(|_| {
                        event.pointer("/delta/type").and_then(Value::as_str)
                            == Some("input_json_delta")
                    });
                if let (Some(piece), Some((id, tool, json))) = (piece, self.writing.get_mut(&index))
                {
                    if json.len() < MAX_PREVIEW_JSON {
                        json.push_str(piece);
                    }
                    // Read the arguments so far only when a preview may be due (reading them
                    // on every piece would grow with the square of their size).
                    if self.pace.ready(id) {
                        if let Some(p) = preview_of(id, *tool, json, false) {
                            if self.pace.due(&p) {
                                parsed.previews.push(p);
                            }
                        }
                    }
                }
            }
            (Some("content_block_stop"), Some(index)) => {
                if let Some((id, tool, json)) = self.writing.remove(&index) {
                    if let Some(p) = preview_of(&id, tool, &json, true) {
                        self.pace.due(&p);
                        parsed.previews.push(p);
                    }
                }
            }
            _ => {}
        }
        parsed
    }

    fn assistant(&mut self, v: &Value) -> Parsed {
        let mut parsed = Parsed::none();
        let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else {
            return parsed;
        };
        let mut text = String::new();
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                    }
                }
                // Claude Code's to-dos are its plan (Phase 25, item 3.1).
                Some("tool_use")
                    if block.get("name").and_then(Value::as_str) == Some("TodoWrite") =>
                {
                    let steps = super::dto::plan_steps(
                        block.pointer("/input/todos").unwrap_or(&Value::Null),
                        &["content", "activeForm"],
                    );
                    if !steps.is_empty() {
                        parsed.events.push(AgentEvent::Plan { steps });
                    }
                }
                Some("tool_use") => parsed.events.push(AgentEvent::ToolUse {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .map(|id| cap(id, 128)),
                    tool: cap(
                        block.get("name").and_then(Value::as_str).unwrap_or("tool"),
                        80,
                    ),
                    summary: tool_summary(block.get("input").unwrap_or(&Value::Null)),
                }),
                _ => {}
            }
        }
        if !text.trim().is_empty() {
            self.state.last_message = Some(text.clone());
            parsed.events.insert(0, AgentEvent::Message { text });
        }
        parsed
    }

    fn user(v: &Value) -> Parsed {
        let mut parsed = Parsed::none();
        let Some(content) = v.pointer("/message/content").and_then(Value::as_array) else {
            return parsed;
        };
        for block in content {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let body = match block.get("content") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(parts)) => parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => String::new(),
            };
            parsed.events.push(AgentEvent::ToolResult {
                id: block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .map(|id| cap(id, 128)),
                tool: None,
                is_error: block
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                summary: first_line(&body, 200),
            });
        }
        parsed
    }

    fn result(&mut self, v: &Value) -> Parsed {
        let mut parsed = Parsed::none();
        let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
        let subtype = v.get("subtype").and_then(Value::as_str).unwrap_or("");
        let text = v.get("result").and_then(Value::as_str);
        if let Some(session) = v.get("session_id").and_then(Value::as_str) {
            self.state.provider_session_id = Some(cap(session, 128));
        }
        self.state.provider_duration_ms = v.get("duration_ms").and_then(Value::as_u64);
        if let Some(usage) = v.get("usage").filter(|u| u.is_object()) {
            let n = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
            let cached = n("cache_read_input_tokens");
            let usage = TokenUsage {
                input_tokens: n("input_tokens") + n("cache_creation_input_tokens") + cached,
                cached_input_tokens: cached,
                output_tokens: n("output_tokens"),
            };
            self.state.usage = Some(usage);
            parsed.events.push(AgentEvent::Usage { usage });
        }
        if !is_error && subtype == "success" {
            self.state.completed = true;
            self.state.final_text = text.map(str::to_owned);
        } else {
            let mut detail = text.filter(|t| !t.trim().is_empty()).map_or_else(
                || format!("Claude Code ended with {subtype:?}"),
                str::to_owned,
            );
            // The reset time its plan report gave, in the form the Router reads, unless the
            // message carries one already.
            if let Some(reset) = self.rejected_reset {
                if !detail.contains('|') {
                    detail.truncate(MAX_EVENT_TEXT.saturating_sub(24));
                    detail.push_str(&format!("|{reset}"));
                }
            }
            self.state.error = Some(cap(&detail, MAX_EVENT_TEXT));
        }
        parsed
    }
}

/// Claude Code's `rate_limit_event`, documented as the Agent SDK's `SDKRateLimitEvent`: whether
/// the plan's limit was reached or is near, the share of it used (`utilization`, 0 to 1), when it
/// resets (`resetsAt`, seconds), and which limit it is (`rateLimitType`: five hours, the week, or
/// the week's for Opus or Sonnet; Phase 25, item 4.3). Only those documented fields are read; the
/// share and the reset time are optional, and a missing one is left out, never worked out (ADR-060
/// §3). A report about extra paid usage (`overage`) is not a plan window, and is left out.
fn plan_report(v: &Value, now: u64) -> Option<PlanReport> {
    let info = v.get("rate_limit_info")?;
    let (limited, warning) = match info.get("status").and_then(Value::as_str)? {
        "allowed" => (false, false),
        "allowed_warning" => (false, true),
        "rejected" => (true, false),
        _ => return None,
    };
    const WEEK: u64 = 7 * 24 * 60;
    let (minutes, models) = match info.get("rateLimitType").and_then(Value::as_str) {
        Some("five_hour") => (Some(300), None),
        Some("seven_day" | "seven_day_overage_included") => (Some(WEEK), None),
        Some("seven_day_opus") => (Some(WEEK), Some("Opus".to_owned())),
        Some("seven_day_sonnet") => (Some(WEEK), Some("Sonnet".to_owned())),
        Some("overage") => return None,
        _ => (None, None),
    };
    let used_percent = info
        .get("utilization")
        .and_then(Value::as_f64)
        .filter(|u| u.is_finite() && *u >= 0.0)
        .map(|u| (u * 100.0).round().min(100.0) as u8);
    let resets_at = info
        .get("resetsAt")
        .and_then(Value::as_f64)
        .and_then(epoch_ms);
    let windows = if used_percent.is_some() || resets_at.is_some() {
        vec![PlanWindow {
            minutes,
            used_percent,
            resets_at,
            models,
        }]
    } else {
        Vec::new()
    };
    Some(PlanReport {
        windows,
        limited,
        warning,
        plan: None,
        reported_at: now,
        key_limit: None,
    })
}

/// A time in seconds (or, if it is that large, milliseconds) since 1970, as milliseconds.
pub(crate) fn epoch_ms(value: f64) -> Option<u64> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let ms = if value < 1e12 { value * 1000.0 } else { value };
    (ms < 1e15).then_some(ms as u64)
}

/// The first Claude Code version `--thinking-display` was checked on, read from the program
/// itself (2.1.288).
const THINKING_DISPLAY_SINCE: (u64, u64, u64) = (2, 1, 288);

/// Whether an installed version (`2.1.288`, or `2.1.288 (Claude Code)`) is one the option was
/// checked on. An unknown version is not.
fn supports_thinking_display(version: Option<&str>) -> bool {
    let Some(first) = version.and_then(|v| v.split_whitespace().next()) else {
        return false;
    };
    let mut parts = first.split('.').map(|p| p.parse::<u64>().ok());
    match (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    ) {
        (Some(a), Some(b), Some(c)) => (a, b, c) >= THINKING_DISPLAY_SINCE,
        _ => false,
    }
}

/// Thinking as the model streams it (ADR-200): the words of its summary as they come, and a
/// sign that it began even when the words are left out. `None` for any other stream event.
fn thinking(v: &Value) -> Option<Parsed> {
    let event = v.get("event")?;
    match event.get("type").and_then(Value::as_str)? {
        "content_block_start" => {
            let block = event.get("content_block")?;
            (block.get("type").and_then(Value::as_str) == Some("thinking")).then(|| {
                Parsed::one(AgentEvent::Status {
                    phase: StatusPhase::Thinking,
                    text: "Thinking".into(),
                })
            })
        }
        "content_block_delta" => {
            let delta = event.get("delta")?;
            match delta.get("type").and_then(Value::as_str)? {
                "thinking_delta" => Some(match delta.get("thinking").and_then(Value::as_str) {
                    Some(text) if !text.is_empty() => Parsed::one(AgentEvent::Reasoning {
                        text: text.to_owned(),
                    }),
                    _ => Parsed::none(),
                }),
                "signature_delta" => Some(Parsed::none()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Claude Code's `system` line `api_retry` in plain words: why it is waiting, how long, and
/// which try it is on. The fields are the ones Claude Code's own output schema names
/// (`attempt`, `max_retries`, `retry_delay_ms`, `error_status`, `error`, `no_response`).
fn retry_words(v: &Value) -> String {
    let number = |k: &str| v.get(k).and_then(Value::as_u64);
    let attempt = number("attempt").unwrap_or(1);
    let most = number("max_retries").unwrap_or(10).max(attempt);
    let wait = number("retry_delay_ms").map_or(0, |ms| ms.div_ceil(1000));
    let status = v.get("error_status").and_then(Value::as_u64);
    let error = v.get("error").and_then(Value::as_str).unwrap_or("");
    let waited = v
        .pointer("/no_response/waited_ms")
        .and_then(Value::as_u64)
        .map(|ms| ms / 1000);
    let why = match (status, error) {
        (Some(429), _) | (_, "rate_limit") => "Anthropic asked Plenipo to slow down".to_owned(),
        (Some(529), _) | (_, "overloaded" | "overloaded_error") => {
            "Anthropic's servers are busy".to_owned()
        }
        (Some(code), _) if code >= 500 => "Anthropic had a problem on its side".to_owned(),
        (None, _) if waited.is_some() => format!(
            "Anthropic has not answered for {} s",
            waited.unwrap_or_default()
        ),
        _ => "Anthropic could not answer yet".to_owned(),
    };
    format!("{why}. Asking again in {wait} s (try {attempt} of {most}).")
}

impl TurnParser for Parser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let kind = v.get("type").and_then(Value::as_str);
        let is_init =
            kind == Some("system") && v.get("subtype").and_then(Value::as_str) == Some("init");
        // Without a confirmed subscription, nothing may happen before the credential check.
        if !is_init && !self.init_seen && !self.billing_confirmed && self.state.stop.is_none() {
            self.state.understood += 1;
            return self.unconfirmed_billing();
        }
        let parsed = match kind {
            Some("system") if is_init => self.init(&v),
            Some("stream_event") => {
                let delta = v.pointer("/event/delta");
                match (
                    v.pointer("/event/type").and_then(Value::as_str),
                    delta.and_then(|d| d.get("type")).and_then(Value::as_str),
                    delta.and_then(|d| d.get("text")).and_then(Value::as_str),
                ) {
                    (Some("content_block_delta"), Some("text_delta"), Some(t)) => {
                        Parsed::one(AgentEvent::TextDelta { text: t.to_owned() })
                    }
                    _ => match thinking(&v) {
                        Some(parsed) => parsed,
                        None => self.tool_input(&v),
                    },
                }
            }
            Some("assistant") => self.assistant(&v),
            Some("user") => Self::user(&v),
            Some("result") => self.result(&v),
            // How much of the plan is used (ADR-060 §3).
            Some("rate_limit_event") => {
                let plan = plan_report(&v, crate::now_ms());
                if let Some(p) = plan.as_ref().filter(|p| p.limited) {
                    self.rejected_reset = p.windows.iter().filter_map(|w| w.resets_at).max();
                }
                Parsed {
                    plan,
                    ..Parsed::none()
                }
            }
            // Claude Code compacted the conversation: it keeps a summary of the earlier part
            // (ADR-044 §2.5).
            Some("system")
                if v.get("subtype").and_then(Value::as_str) == Some("compact_boundary") =>
            {
                Parsed::one(AgentEvent::MemoryShortened {
                    detail: "Claude Code shortened its memory of this conversation: it keeps a \
                             summary of the earlier part."
                        .into(),
                })
            }
            // The AI company was busy or slow and Claude Code is asking again: said at once,
            // not after minutes of silence (ADR-200).
            Some("system") if v.get("subtype").and_then(Value::as_str) == Some("api_retry") => {
                Parsed::one(AgentEvent::Status {
                    phase: StatusPhase::Waiting,
                    text: retry_words(&v),
                })
            }
            // The request went out and the first words have not come yet.
            Some("system")
                if v.get("subtype").and_then(Value::as_str) == Some("status")
                    && v.get("status").and_then(Value::as_str) == Some("requesting") =>
            {
                Parsed::one(AgentEvent::Status {
                    phase: StatusPhase::Waiting,
                    text: "Asked Anthropic. Waiting for the first words.".into(),
                })
            }
            Some("system") => Parsed::none(),
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
        self.state.finish(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ExecutionState;
    use serde_json::json;

    fn probe(stdout: &str, code: i32) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(code),
            stdout: stdout.into(),
            ..ProbeOutput::default()
        }
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

    fn feed(p: &mut dyn TurnParser, lines: &[Value]) -> Vec<AgentEvent> {
        lines
            .iter()
            .flat_map(|l| p.line(&l.to_string(), false).events)
            .collect()
    }

    fn new_request() -> TurnRequest {
        TurnRequest {
            session: ProviderSession::New {
                preassigned: Some("11111111-1111-4111-8111-111111111111".into()),
            },
            model: None,
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
            cli_version: None,
        }
    }

    #[test]
    fn thinking_summaries_are_asked_for_only_on_a_version_they_were_checked_on() {
        for (version, yes) in [
            (Some("2.1.288"), true),
            (Some("2.1.288 (Claude Code)"), true),
            (Some("2.1.300"), true),
            (Some("2.2.0"), true),
            (Some("3.0.0"), true),
            (Some("2.1.287"), false),
            (Some("2.0.999"), false),
            (Some("1.9.9"), false),
            (Some("garbage"), false),
            (Some(""), false),
            (None, false),
        ] {
            assert_eq!(supports_thinking_display(version), yes, "{version:?}");
            let request = TurnRequest {
                cli_version: version.map(str::to_owned),
                ..new_request()
            };
            let args = ClaudeCode.turn_args(&request);
            let asked = args
                .windows(2)
                .any(|w| w == ["--thinking-display", "summarized"]);
            assert_eq!(asked, yes, "{version:?}: {args:?}");
        }
    }

    #[test]
    fn thinking_streams_as_reasoning_and_a_start_sign_even_when_the_words_are_left_out() {
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({ "type": "system", "subtype": "init", "session_id":
                    "11111111-1111-4111-8111-111111111111", "model": "claude-sonnet-5-5",
                    "apiKeySource": "none" }),
                json!({ "type": "stream_event", "event": { "type": "content_block_start",
                    "index": 0, "content_block": { "type": "thinking", "thinking": "" } } }),
                json!({ "type": "stream_event", "event": { "type": "content_block_delta",
                    "index": 0, "delta": { "type": "thinking_delta", "thinking": "The owner " } } }),
                json!({ "type": "stream_event", "event": { "type": "content_block_delta",
                    "index": 0, "delta": { "type": "thinking_delta", "thinking": "wants a script." } } }),
                json!({ "type": "stream_event", "event": { "type": "content_block_delta",
                    "index": 0, "delta": { "type": "thinking_delta", "thinking": "" } } }),
                json!({ "type": "stream_event", "event": { "type": "content_block_delta",
                    "index": 0, "delta": { "type": "signature_delta", "signature": "abc" } } }),
            ],
        );
        assert!(matches!(
            &events[1],
            AgentEvent::Status { phase: StatusPhase::Thinking, text } if text == "Thinking"
        ));
        let thoughts: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::Reasoning { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(thoughts, ["The owner ", "wants a script."]);
        // An empty piece and a signature say nothing.
        assert_eq!(events.len(), 4, "{events:?}");
    }

    #[test]
    fn waiting_and_asking_again_are_said_at_once_in_plain_words() {
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({ "type": "system", "subtype": "init", "session_id":
                    "11111111-1111-4111-8111-111111111111", "apiKeySource": "none" }),
                json!({ "type": "system", "subtype": "status", "status": "requesting" }),
                json!({ "type": "system", "subtype": "api_retry", "attempt": 3, "max_retries": 10,
                    "retry_delay_ms": 4400, "error_status": 529, "error": "overloaded" }),
                json!({ "type": "system", "subtype": "api_retry", "attempt": 1, "max_retries": 10,
                    "retry_delay_ms": 500, "error_status": 429, "error": "rate_limit" }),
                json!({ "type": "system", "subtype": "api_retry", "attempt": 2, "max_retries": 10,
                    "retry_delay_ms": 1100, "error_status": null, "error": "unknown",
                    "no_response": { "waited_ms": 45000, "retry_wait_ms": 1100 } }),
                json!({ "type": "system", "subtype": "api_retry", "attempt": 4, "max_retries": 10,
                    "retry_delay_ms": 8500, "error_status": 503, "error": "server_error" }),
                // Other system lines still say nothing.
                json!({ "type": "system", "subtype": "thinking_tokens", "tokens": 5 }),
            ],
        );
        let said: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::Status {
                    phase: StatusPhase::Waiting,
                    text,
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            said,
            [
                "Asked Anthropic. Waiting for the first words.",
                "Anthropic's servers are busy. Asking again in 5 s (try 3 of 10).",
                "Anthropic asked Plenipo to slow down. Asking again in 1 s (try 1 of 10).",
                "Anthropic has not answered for 45 s. Asking again in 2 s (try 2 of 10).",
                "Anthropic had a problem on its side. Asking again in 9 s (try 4 of 10).",
            ]
        );
    }

    #[test]
    fn a_tool_call_and_its_result_carry_the_same_id() {
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({ "type": "system", "subtype": "init", "session_id":
                    "11111111-1111-4111-8111-111111111111", "apiKeySource": "none" }),
                json!({ "type": "assistant", "message": { "content": [
                    { "type": "tool_use", "id": "toolu_01", "name": "mcp__plenipo__write_file",
                      "input": { "path": "clear-temp.ps1", "content": "x" } } ] } }),
                json!({ "type": "user", "message": { "content": [
                    { "type": "tool_result", "tool_use_id": "toolu_01", "content": "Created", 
                      "is_error": false } ] } }),
            ],
        );
        match (&events[1], &events[2]) {
            (
                AgentEvent::ToolUse {
                    id: Some(a),
                    tool,
                    summary,
                },
                AgentEvent::ToolResult {
                    id: Some(b),
                    is_error: false,
                    ..
                },
            ) => {
                assert_eq!((a.as_str(), b.as_str()), ("toolu_01", "toolu_01"));
                assert_eq!(tool, "mcp__plenipo__write_file");
                assert_eq!(summary, "clear-temp.ps1");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn known_models_are_valid_names_with_their_own_effort_levels() {
        let caps = ClaudeCode.capabilities();
        let names: Vec<&str> = caps.known_models.iter().map(|m| m.name.as_str()).collect();
        // The short names first, then the exact versions they point to today (ADR-081 §6).
        assert_eq!(
            names,
            [
                "fable",
                "opus",
                "sonnet",
                "haiku",
                "claude-fable-5-1",
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "claude-haiku-4-5-20251001"
            ]
        );
        for short in &caps.known_models[..4] {
            let exact = short
                .points_to
                .as_deref()
                .expect("a short name says what it is now");
            let target = caps.known_models.iter().find(|m| m.name == exact).unwrap();
            assert_eq!(target.effort_levels, short.effort_levels, "{exact}");
            assert_eq!(target.maker, short.maker, "{exact}");
        }
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
        assert_eq!(caps.effort_levels_for(Some("fable")), FRONTIER_EFFORT);
        assert!(caps.effort_levels_for(Some("haiku")).is_empty());
        // A model it does not know, or its default model: the CLI's own levels.
        assert_eq!(caps.effort_levels_for(Some("claude-x")), FRONTIER_EFFORT);
        assert_eq!(caps.effort_levels_for(None), FRONTIER_EFFORT);
    }

    #[test]
    fn turn_arguments_new_and_resume() {
        let args = ClaudeCode.turn_args(&new_request());
        assert_eq!(&args[..3], ["-p", "--output-format", "stream-json"]);
        let tools = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[tools + 1], "", "no built-in tools in Phase 3");
        assert!(args.contains(&"--strict-mcp-config".to_owned()));
        assert!(args.ends_with(&[
            "--session-id".into(),
            "11111111-1111-4111-8111-111111111111".into()
        ]));
        for forbidden in [
            "--dangerously-skip-permissions",
            "--bare",
            "--no-session-persistence",
        ] {
            assert!(!args.iter().any(|a| a == forbidden), "{forbidden}");
        }

        let resume = ClaudeCode.turn_args(&TurnRequest {
            session: ProviderSession::Resume { id: "abc".into() },
            model: Some("sonnet".into()),
            effort: Some(Effort::XHigh),
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
            cli_version: None,
        });
        assert!(resume.ends_with(&["--resume".into(), "abc".into()]));
        let m = resume.iter().position(|a| a == "--model").unwrap();
        assert_eq!(resume[m + 1], "sonnet");
        let e = resume.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(resume[e + 1], "xhigh");
        assert!(!args.iter().any(|a| a == "--effort"));
    }

    #[test]
    fn plenipo_tools_come_only_from_plenipo() {
        use crate::agent::tools::ToolServer;
        let tools = ToolServer {
            name: "plenipo".into(),
            command: "/opt/plenipo/plenipo-desktop".into(),
            args: vec!["--plenipo-tools=/data/t.json".into()],
            config_file: "/data/t.mcp.json".into(),
            call_timeout: std::time::Duration::from_secs(3600),
            tools: Vec::new(),
        };
        let request = TurnRequest {
            tools: Some(tools),
            working_dir: PathBuf::new(),
            ..new_request()
        };
        let args = ClaudeCode.turn_args(&request);
        // Still no built-in tools and none of the owner's MCP servers…
        let t = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[t + 1], "");
        assert!(args.contains(&"--strict-mcp-config".to_owned()));
        // …only Plenipo's server, whose tools need no prompt from Claude Code.
        let m = args.iter().position(|a| a == "--mcp-config").unwrap();
        assert_eq!(args[m + 1], "/data/t.mcp.json");
        let a = args.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(args[a + 1], "mcp__plenipo");
        assert!(!args
            .iter()
            .any(|a| a.contains("dangerously") || a == "--permission-mode"));
        let env = ClaudeCode.turn_env(&request);
        assert!(env.contains(&("MCP_TOOL_TIMEOUT".into(), "3600000".into())));
        assert!(ClaudeCode.turn_env(&new_request()).is_empty());
        assert!(!ClaudeCode
            .turn_args(&new_request())
            .contains(&"--mcp-config".to_owned()));
    }

    #[test]
    fn auth_status_classification() {
        let s = parse_auth(&probe(
            r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"a@b.c","subscriptionType":"max"}"#,
            0,
        ));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("Claude subscription (max)"));
        assert!(
            !format!("{s:?}").contains("a@b.c"),
            "no account identifiers"
        );

        let s = parse_auth(&probe(r#"{"loggedIn":false}"#, 1));
        assert_eq!(s.state, AuthState::SignedOut);
        let s = parse_auth(&probe(
            r#"{"loggedIn":true,"authMethod":"api_key","apiProvider":"firstParty"}"#,
            0,
        ));
        assert_eq!(s.state, AuthState::ApiKey);
        let s = parse_auth(&probe(r#"{"loggedIn":true,"apiProvider":"bedrock"}"#, 0));
        assert_eq!(s.state, AuthState::ThirdPartyCloud);
        let s = parse_auth(&probe(
            r#"{"loggedIn":true,"authMethod":"something-new"}"#,
            0,
        ));
        assert_eq!(s.state, AuthState::Unverified);
        let s = parse_auth(&probe("error: unknown command 'auth'", 1));
        assert_eq!(s.state, AuthState::Unknown);
        let s = parse_auth(&probe("Not logged in", 1));
        assert_eq!(s.state, AuthState::SignedOut);
        let s = parse_auth(&ProbeOutput {
            timed_out: true,
            ..ProbeOutput::default()
        });
        assert_eq!(s.state, AuthState::Unknown);
    }

    /// Phase 25, item 3.1: Claude Code's to-dos become its plan, not a step of their own.
    #[test]
    fn its_to_dos_are_its_plan() {
        use super::super::dto::{PlanStatus, PlanStep};
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"TodoWrite","input":{"todos":[{"content":"Read the code","status":"completed","activeForm":"Reading the code"},{"content":"Run the tests","status":"in_progress","activeForm":"Running the tests"},{"content":"Write it up","status":"pending","activeForm":"Writing it up"}]}}]}}),
            ],
        );
        let step = |text: &str, status| PlanStep {
            text: text.into(),
            status,
        };
        assert_eq!(
            events,
            [AgentEvent::Plan {
                steps: vec![
                    step("Read the code", PlanStatus::Done),
                    step("Run the tests", PlanStatus::InProgress),
                    step("Write it up", PlanStatus::Pending),
                ]
            }]
        );
    }

    #[test]
    fn successful_stream_is_normalized() {
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"system","subtype":"init","session_id":"11111111-1111-4111-8111-111111111111","model":"model-x","apiKeySource":"none","tools":[]}),
                json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}}),
                json!({"type":"stream_event","event":{"type":"message_start"}}),
                json!({"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","name":"Read","input":{"file_path":"/tmp/x"}}]}}),
                json!({"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"denied"}]}}),
                json!({"type":"brand_new_event"}),
                json!({"type":"result","subtype":"success","is_error":false,"result":"Hello","session_id":"11111111-1111-4111-8111-111111111111","duration_ms":42,"usage":{"input_tokens":3,"cache_creation_input_tokens":2,"cache_read_input_tokens":5,"output_tokens":7}}),
            ],
        );
        assert_eq!(
            events,
            [
                AgentEvent::SessionStarted {
                    provider_session_id: Some("11111111-1111-4111-8111-111111111111".into()),
                    model: Some("model-x".into())
                },
                AgentEvent::TextDelta { text: "Hel".into() },
                AgentEvent::Message {
                    text: "Hello".into()
                },
                AgentEvent::ToolUse {
                    id: None,
                    tool: "Read".into(),
                    summary: "/tmp/x".into()
                },
                AgentEvent::ToolResult {
                    id: None,
                    tool: None,
                    is_error: true,
                    summary: "denied".into()
                },
                AgentEvent::Usage {
                    usage: TokenUsage {
                        input_tokens: 10,
                        cached_input_tokens: 5,
                        output_tokens: 7
                    }
                },
            ]
        );
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("Hello"));
        assert_eq!(r.model.as_deref(), Some("model-x"));
        assert_eq!(r.duration_ms, Some(42));
        assert_eq!(r.ignored_lines, 1, "unknown event counted, not fatal");
    }

    /// ADR-055: a Plenipo file change streamed while the model writes it becomes previews — the
    /// path once complete, the text so far, and the end — and other tool calls do not.
    #[test]
    fn a_file_change_being_written_becomes_previews() {
        let mut p = ClaudeCode.parser(&new_request());
        let init = json!({"type":"system","subtype":"init","session_id":"11111111-1111-4111-8111-111111111111","model":"m","apiKeySource":"none","tools":[]});
        p.line(&init.to_string(), false);
        let start = |index: u64, name: &str| {
            json!({"type":"stream_event","event":{"type":"content_block_start","index":index,
                "content_block":{"type":"tool_use","id":format!("toolu_{index}"),"name":name,"input":{}}}})
        };
        let delta = |index: u64, piece: &str| {
            json!({"type":"stream_event","event":{"type":"content_block_delta","index":index,
                "delta":{"type":"input_json_delta","partial_json":piece}}})
        };
        let stop = |index: u64| json!({"type":"stream_event","event":{"type":"content_block_stop","index":index}});
        let mut previews = Vec::new();
        for line in [
            start(1, "mcp__plenipo__write_file"),
            start(2, "mcp__plenipo__read_file"),
            delta(1, "{\"path\": \"src/a"),
            delta(2, "{\"path\": \"secret"),
            delta(1, ".rs\", \"content\": \"fn main"),
            delta(1, "() {}\\n\"}"),
            stop(1),
            stop(2),
        ] {
            let parsed = p.line(&line.to_string(), false);
            assert!(parsed.events.is_empty(), "previews are not activity");
            previews.extend(parsed.previews);
        }
        assert!(
            previews.iter().all(|x| x.call == "toolu_1"),
            "{previews:#?}"
        );
        let last = previews.last().unwrap();
        assert!(last.done);
        assert_eq!(last.tool, WriteTool::Write);
        assert_eq!(last.path.as_deref(), Some("src/a.rs"));
        assert_eq!(last.text, "fn main() {}\n");
        assert!(previews.iter().any(|x| !x.done && x.path.is_some()));

        // A block left open (a stream cut off) is forgotten when a new message starts, so
        // another tool's arguments at the same place never join it.
        let message = json!({"type":"stream_event","event":{"type":"message_start"}});
        let mut later = Vec::new();
        for line in [
            start(3, "mcp__plenipo__write_file"),
            delta(3, "{\"path\": \"src/b.rs\", \"content\": \"x"),
            message,
            start(3, "mcp__plenipo__read_file"),
            delta(3, "{\"path\": \"other.txt\"}"),
            stop(3),
        ] {
            later.extend(p.line(&line.to_string(), false).previews);
        }
        assert!(
            later.iter().all(|x| !x.text.contains("other") && !x.done),
            "{later:#?}"
        );
    }

    /// ADR-044 §2.5: Claude Code's "compacted" notice tells Plenipo it shortened its memory.
    #[test]
    fn a_compacted_conversation_is_reported() {
        let mut p = ClaudeCode.parser(&new_request());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"system","subtype":"init","apiKeySource":"none",
                       "session_id":"11111111-1111-4111-8111-111111111111"}),
                json!({"type":"system","subtype":"compact_boundary",
                       "session_id":"11111111-1111-4111-8111-111111111111",
                       "compact_metadata":{"trigger":"auto","pre_tokens":155000}}),
                json!({"type":"system","subtype":"status"}),
            ],
        );
        assert!(matches!(
            &events[1..],
            [AgentEvent::MemoryShortened { detail }] if detail.contains("shortened its memory")
        ));
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.ignored_lines, 0, "understood, not ignored");
    }

    #[test]
    fn api_key_credential_source_stops_the_turn() {
        let mut p = ClaudeCode.parser(&new_request());
        let parsed = p.line(
            &json!({"type":"system","subtype":"init","session_id":"s","apiKeySource":"ANTHROPIC_API_KEY"})
                .to_string(),
            false,
        );
        let stop = parsed.stop.expect("must stop");
        assert_eq!(stop.outcome, TurnOutcome::BillingNotAllowed);
        let r = p.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);
        assert!(r.summary.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn missing_credential_source_needs_a_confirmed_subscription() {
        let init = json!({"type":"system","subtype":"init","session_id":"s"}).to_string();
        // Confirmed by `auth status`: fine.
        let mut p = ClaudeCode.parser(&new_request());
        assert!(p.line(&init, false).stop.is_none());

        // Unconfirmed sign-in and no credential source in the stream: stop.
        let unconfirmed = TurnRequest {
            billing_confirmed: false,
            tools: None,
            ..new_request()
        };
        let mut p = ClaudeCode.parser(&unconfirmed);
        let parsed = p.line(&init, false);
        assert_eq!(parsed.stop.unwrap().outcome, TurnOutcome::BillingNotAllowed);
        let r = p.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);

        // Unconfirmed, but the stream reports a subscription credential: fine.
        let mut p = ClaudeCode.parser(&unconfirmed);
        let ok = json!({"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"});
        assert!(p.line(&ok.to_string(), false).stop.is_none());

        // Unconfirmed and output before any credential report: stop before it counts.
        let mut p = ClaudeCode.parser(&unconfirmed);
        let early = json!({"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}});
        let parsed = p.line(&early.to_string(), false);
        assert!(parsed.stop.is_some());
        assert!(parsed.events.is_empty());
    }

    #[test]
    fn error_results_are_classified() {
        for (text, want) in [
            (
                "Claude AI usage limit reached|1760000000",
                TurnOutcome::UsageLimited,
            ),
            (
                "Invalid API key · Please run /login",
                TurnOutcome::AuthRequired,
            ),
            ("Something else broke", TurnOutcome::Failed),
        ] {
            let mut p = ClaudeCode.parser(&new_request());
            feed(
                p.as_mut(),
                &[json!({"type":"result","subtype":"success","is_error":true,"result":text})],
            );
            let r = p.finish(&end(ExecutionState::Failed, Some(1)));
            assert_eq!(r.outcome, want, "{text}");
            assert_eq!(r.error.as_deref(), Some(text));
        }
    }

    #[test]
    fn crash_and_malformed_output() {
        let mut p = ClaudeCode.parser(&new_request());
        feed(
            p.as_mut(),
            &[json!({"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"})],
        );
        p.stderr("fatal: something died");
        let r = p.finish(&end(ExecutionState::Failed, Some(134)));
        assert_eq!(r.outcome, TurnOutcome::Crashed);
        assert_eq!(r.provider_session_id.as_deref(), Some("s"));

        let mut p = ClaudeCode.parser(&new_request());
        assert_eq!(p.line("<html>oops</html>", false).events.len(), 1);
        assert!(p.line("more junk", false).events.is_empty());
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::MalformedOutput);
        assert_eq!(r.ignored_lines, 2);
    }

    #[test]
    fn session_mismatch_is_reported() {
        let mut p = ClaudeCode.parser(&TurnRequest {
            session: ProviderSession::Resume { id: "old".into() },
            model: None,
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
            cli_version: None,
        });
        let events = feed(
            p.as_mut(),
            &[json!({"type":"system","subtype":"init","session_id":"new","apiKeySource":"none"})],
        );
        assert!(
            matches!(&events[1], AgentEvent::Notice { level: NoticeLevel::Warning, text } if text.contains("new"))
        );
    }

    #[test]
    fn environment_passes_no_credentials() {
        let host = HostEnv::default().with_vars(vec![
            ("ANTHROPIC_API_KEY".into(), "sk-secret".into()),
            ("ANTHROPIC_AUTH_TOKEN".into(), "t".into()),
            ("CLAUDE_CODE_OAUTH_TOKEN".into(), "t".into()),
            ("CLAUDE_CODE_USE_BEDROCK".into(), "1".into()),
            ("HTTPS_PROXY".into(), "http://proxy:8080".into()),
            ("CLAUDE_CONFIG_DIR".into(), "/cfg".into()),
        ]);
        let env = crate::agent::discovery::runtime_env(&ClaudeCode, &host);
        let names: Vec<_> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            ["DISABLE_AUTOUPDATER", "CLAUDE_CONFIG_DIR", "HTTPS_PROXY"]
        );
    }

    /// Phase 25, item 4.3: each report says which limit it is, and a limit reached in a turn
    /// waits for the reset time its report gave.
    #[test]
    fn each_report_names_its_limit_and_a_reached_one_gives_its_reset() {
        let report = |kind: &str| {
            plan_report(
                &json!({ "type": "rate_limit_event", "rate_limit_info": {
                    "status": "allowed", "rateLimitType": kind, "utilization": 0.4,
                    "resetsAt": 1_790_578_200u64 } }),
                1,
            )
        };
        let five = report("five_hour").unwrap();
        assert_eq!(
            (five.windows[0].minutes, five.windows[0].models.as_deref()),
            (Some(300), None)
        );
        let week = report("seven_day").unwrap();
        assert_eq!(week.windows[0].minutes, Some(7 * 24 * 60));
        let opus = report("seven_day_opus").unwrap();
        assert_eq!(opus.windows[0].models.as_deref(), Some("Opus"));
        assert_eq!(
            report("seven_day_sonnet").unwrap().windows[0]
                .models
                .as_deref(),
            Some("Sonnet")
        );
        // Extra paid usage is not a plan window.
        assert_eq!(report("overage"), None);

        let mut p = ClaudeCode.parser(&new_request());
        feed(
            p.as_mut(),
            &[
                json!({ "type": "system", "subtype": "init", "session_id": "s",
                        "apiKeySource": "none" }),
                json!({ "type": "rate_limit_event", "rate_limit_info": {
                    "status": "rejected", "rateLimitType": "five_hour",
                    "resetsAt": 1_790_578_200u64 } }),
                json!({ "type": "result", "subtype": "success", "is_error": true,
                        "result": "You've hit your limit · resets 3pm" }),
            ],
        );
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::UsageLimited);
        assert_eq!(
            r.error.as_deref(),
            Some("You've hit your limit · resets 3pm|1790578200000")
        );
    }
}

/// Phase 19: the AI tools page (ADR-058 to ADR-060).
#[cfg(test)]
mod ai_tools_page_tests {
    use super::*;
    use crate::agent::adapter::NewestVersion;
    use serde_json::json;

    fn request() -> TurnRequest {
        TurnRequest {
            session: ProviderSession::New { preassigned: None },
            model: None,
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
            cli_version: None,
        }
    }

    #[test]
    fn signs_in_and_out_and_updates_with_its_own_commands() {
        assert_eq!(
            ClaudeCode.account_command(AccountAction::SignIn),
            Some(vec!["auth".to_owned(), "login".to_owned()])
        );
        assert_eq!(
            ClaudeCode.account_command(AccountAction::SignOut),
            Some(vec!["auth".to_owned(), "logout".to_owned()])
        );
        assert_eq!(ClaudeCode.update_command(), Some(vec!["update".to_owned()]));
        assert_eq!(
            ClaudeCode.put_back_command("2.1.283"),
            Some(vec!["install".to_owned(), "2.1.283".to_owned()])
        );
        assert_eq!(
            ClaudeCode.newest_version(),
            NewestVersion::Published(PublishedList::Npm("@anthropic-ai/claude-code"))
        );
        assert!(ClaudeCode
            .update_by_hand()
            .unwrap()
            .contains("winget upgrade"));
        // Its updater stays off during tasks; `claude update` still works with it set.
        assert!(ClaudeCode
            .fixed_env()
            .contains(&("DISABLE_AUTOUPDATER".into(), "1".into())));
    }

    #[test]
    fn reads_the_plan_only_from_its_documented_rate_limit_event() {
        let event = json!({ "type": "rate_limit_event", "uuid": "u", "session_id": "s",
            "rate_limit_info": { "status": "allowed_warning", "resetsAt": 1_790_578_200u64,
                                 "utilization": 0.914 } });
        let report = plan_report(&event, 7).unwrap();
        assert_eq!(
            report.windows,
            vec![PlanWindow {
                minutes: None,
                used_percent: Some(91),
                resets_at: Some(1_790_578_200_000),
                models: None,
            }]
        );
        assert!(report.warning && !report.limited);
        assert_eq!(report.reported_at, 7);
        // The share is optional: without it, only what was reported.
        let reached = json!({ "type": "rate_limit_event",
            "rate_limit_info": { "status": "rejected", "resetsAt": 1_790_578_200u64 } });
        let report = plan_report(&reached, 1).unwrap();
        assert!(report.limited);
        assert_eq!(report.windows[0].used_percent, None);
        // Nothing reported: no window, never a share worked out.
        let bare =
            json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "allowed" } });
        assert!(plan_report(&bare, 1).unwrap().windows.is_empty());
        // Undocumented statuses and shapes are not read.
        let odd = json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "maybe" } });
        assert_eq!(plan_report(&odd, 1), None);
        assert_eq!(
            plan_report(&json!({ "type": "rate_limit_event", "info": {} }), 1),
            None
        );
    }

    #[test]
    fn the_plan_comes_through_the_stream_plenipo_already_reads() {
        let mut parser = ClaudeCode.parser(&request());
        let init = json!({ "type": "system", "subtype": "init", "session_id": "s",
                           "apiKeySource": "none" });
        let _ = parser.line(&init.to_string(), false);
        let event = json!({ "type": "rate_limit_event", "uuid": "u", "session_id": "s",
            "rate_limit_info": { "status": "allowed", "resetsAt": 1_790_578_200u64,
                                 "utilization": 0.09 } });
        let parsed = parser.line(&event.to_string(), false);
        assert_eq!(parsed.plan.unwrap().windows[0].used_percent, Some(9));
        assert!(parsed.events.is_empty() && parsed.stop.is_none());
    }
}
