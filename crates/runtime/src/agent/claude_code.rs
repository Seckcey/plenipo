//! Claude Code adapter (ADR-007).
//!
//! One turn = `claude -p --output-format stream-json …` with the objective on stdin. New
//! sessions get a Plenipo-chosen `--session-id`; follow-ups use `--resume`. Phase 3 grants no
//! tools (`--tools ""`) and no MCP servers. The `init` event's credential source is checked on
//! every turn: anything but a subscription sign-in stops the turn.

use std::path::PathBuf;

use serde_json::Value;

use crate::agent::adapter::{
    cap, first_line, tool_summary, Parsed, ProbeOutput, ProcessEnd, ProviderSession,
    RuntimeAdapter, Stop, TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT, NETWORK_ENV,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    AgentEvent, AuthState, AuthStatus, NoticeLevel, RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "claude-code";
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
            tool_posture: "Conversation only: no built-in tools and no MCP servers until \
                           Plenipo Guard grants capabilities (Phase 7)."
                .into(),
        }
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
        match &request.session {
            ProviderSession::New {
                preassigned: Some(id),
            } => args.extend(["--session-id".into(), id.clone()]),
            ProviderSession::New { preassigned: None } => {}
            ProviderSession::Resume { id } => args.extend(["--resume".into(), id.clone()]),
        }
        args
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
            events: Vec::new(),
            stop: Some(stop),
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
                Some("tool_use") => parsed.events.push(AgentEvent::ToolUse {
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
            let detail = text.filter(|t| !t.trim().is_empty()).map_or_else(
                || format!("Claude Code ended with {subtype:?}"),
                str::to_owned,
            );
            self.state.error = Some(cap(&detail, MAX_EVENT_TEXT));
        }
        parsed
    }
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
                    _ => Parsed::none(),
                }
            }
            Some("assistant") => self.assistant(&v),
            Some("user") => Self::user(&v),
            Some("result") => self.result(&v),
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
            billing_confirmed: true,
        }
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
            billing_confirmed: true,
        });
        assert!(resume.ends_with(&["--resume".into(), "abc".into()]));
        let m = resume.iter().position(|a| a == "--model").unwrap();
        assert_eq!(resume[m + 1], "sonnet");
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
                    tool: "Read".into(),
                    summary: "/tmp/x".into()
                },
                AgentEvent::ToolResult {
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
            billing_confirmed: true,
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
}
