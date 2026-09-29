//! Antigravity adapter (Google's Antigravity CLI, `agy`), ADR-082 (Antigravity as an AI tool).
//!
//! Google's Gemini CLI no longer serves personal Google accounts
//! (`docs/phases/ai-tools-gemini-finding.md`); Antigravity CLI is its replacement, and runs
//! Google's Gemini models and some of Anthropic's and OpenAI's under one Google sign-in.
//!
//! One task is one process: `agy -p= --input-format stream-json --output-format stream-json`,
//! read-only (`--mode plan --sandbox`), the task's words written to stdin as one JSON message
//! and never on the command line. The answer streams back as JSON lines: `init` (the
//! conversation ID and the permission mode), `step_update` for each step (text as it is
//! written, tool steps, token counts), and one `result`.
//!
//! Least privilege (ADR-082 §4): Antigravity gets a home folder of its own
//! ([`RuntimeAdapter::own_home`]) with Plenipo's settings — strict permissions, every kind of
//! its own tool denied (programs, web pages, file reads and writes, add-on tools), paid AI
//! credits off — so none of the owner's own Antigravity settings, hooks, or add-ons apply. In
//! one-task mode Antigravity also refuses any tool that needs permission. A task whose `init`
//! does not report strict permissions (the settings did not load) is stopped at once, and so is
//! a task in which one of its own tools finished anyway. Workers on it are conversation only.
//!
//! Billing (ADR-007 §4): `agy models` lists models only when signed in; a Gemini API key needs
//! both a key variable (never passed) and a setting Plenipo never writes, and its list has only
//! Gemini's models, so a list without another company's model is taken for a key. Its
//! sign-in stays in the operating system's credential store, so the home folder of its own
//! keeps the owner's sign-in.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent::adapter::{
    cap, first_line, model_name, plain_name, ProbeOutput, ProcessEnd, ProviderSession,
    RuntimeAdapter, StatusCheck, Stop, TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT,
    MAX_RESULT_TEXT, MAX_SUMMARY, NETWORK_ENV,
};
use crate::agent::adapter::{NewestVersion, Parsed};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AccountAction, AgentEvent, AuthState, AuthStatus, KnownModel, NoticeLevel,
    RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "antigravity";
const LABEL: &str = "Antigravity";

/// Where Antigravity reads its settings, inside its home folder.
pub const SETTINGS: &str = ".gemini/antigravity-cli/settings.json";

/// The permission mode Plenipo's settings give it; `init` must report it.
const STRICT: &str = "strict";

/// Its own tools that change nothing and reach nothing: waiting, finishing, or asking (which
/// one-task mode refuses). Any other of its own tools that finishes stops the task.
const HARMLESS_TOOLS: &[&str] = &[
    "finish",
    "wait",
    "wait_5_seconds",
    "ask_question",
    "ask_permission",
    "ask_custom_permission",
    "list_permissions",
];

#[derive(Debug, Default, Clone, Copy)]
pub struct Antigravity;

/// Plenipo's settings for Antigravity (ADR-082 §4), exactly as Antigravity 1.2.13 writes the
/// file back after reading it (keys in order, two spaces), so the file is replaced only when
/// something changed it. Each permission rule kind was checked to load: Antigravity drops rule
/// kinds it does not know (`url(*)` and `file(*)` were dropped; these five were kept). Paid AI
/// credits stay at its default, off: it leaves `useG1Credits` out when it is off and keeps it
/// when it is on, so a file that turns them on is replaced before the next run.
pub const SETTINGS_TEXT: &str = "{
  \"permissions\": {
    \"deny\": [
      \"command(*)\",
      \"read_url(*)\",
      \"read_file(*)\",
      \"write_file(*)\",
      \"mcp(*)\"
    ]
  },
  \"toolPermission\": \"strict\"
}
";

impl RuntimeAdapter for Antigravity {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        makers::GOOGLE.0
    }

    fn provider_label(&self) -> &'static str {
        makers::GOOGLE.1
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        let google = |name: &str, label: &str| KnownModel::new(name, label, &[]).by(makers::GOOGLE);
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            // Its stream never names the sign-in in use; `agy models` must show a Google
            // sign-in before each task.
            billing_checked_per_turn: false,
            tool_posture: "Conversation only: Antigravity's own tools are off. It cannot run \
                           programs, read or change files, open web pages, or use add-ons, and \
                           none of your own Antigravity settings apply. Paid AI credits stay \
                           off. If it uses one of its own tools anyway (its web search, for \
                           example), Plenipo stops the task."
                .into(),
            // Each model's name carries its thinking level ("Gemini 3.8 Flash (High)"), so
            // Plenipo sets no effort of its own (ADR-082 §5).
            effort_levels: Vec::new(),
            // `agy models` on the owner's PC, signed in with a Google plan (1.2.13,
            // 2026-09-29), in its order.
            known_models: vec![
                google("gemini-3.8-flash-high", "Gemini 3.8 Flash (High)"),
                google("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium)"),
                google("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low)"),
                google("gemini-3.7-flash-high", "Gemini 3.7 Flash (High)"),
                google("gemini-3.7-flash-medium", "Gemini 3.7 Flash (Medium)"),
                google("gemini-3.7-flash-low", "Gemini 3.7 Flash (Low)"),
                google("gemini-3.6-flash-high", "Gemini 3.6 Flash (High)"),
                google("gemini-3.6-flash-medium", "Gemini 3.6 Flash (Medium)"),
                google("gemini-3.6-flash-low", "Gemini 3.6 Flash (Low)"),
                google("gemini-3.1-pro-high", "Gemini 3.1 Pro (High)"),
                google("gemini-3.1-pro-low", "Gemini 3.1 Pro (Low)"),
                KnownModel::new("claude-sonnet-4-6", "Claude Sonnet 4.6 (Thinking)", &[])
                    .by(makers::ANTHROPIC),
                KnownModel::new(
                    "claude-opus-4-6-thinking",
                    "Claude Opus 4.6 (Thinking)",
                    &[],
                )
                .by(makers::ANTHROPIC),
                KnownModel::new("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)", &[])
                    .by(makers::OPENAI),
            ],
            // Which model runs when none is named is not reported anywhere Plenipo reads, so
            // who made it is not known and review plays safe (ADR-081 §2).
            default_maker: None,
            runs_other_makers: true,
        }
    }

    fn checked_version(&self) -> &'static str {
        "1.2.13"
    }

    fn install_hint(&self) -> &'static str {
        "Install Antigravity, Google's command-line AI tool. Windows (PowerShell): irm \
         https://antigravity.google/cli/install.ps1 | iex — macOS/Linux: curl -fsSL \
         https://antigravity.google/cli/install.sh | bash. Then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Choose Sign in: Antigravity opens in a terminal tab. Sign in with the Google account \
         that has your Google AI plan, then type /exit. Plenipo never asks for your password or \
         an API key. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "agy"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(home) = &host.home {
            // Google's installers: %LOCALAPPDATA%\agy\bin\agy.exe on Windows, ~/.local/bin/agy
            // elsewhere.
            if cfg!(windows) {
                out.push(
                    home.join("AppData")
                        .join("Local")
                        .join("agy")
                        .join("bin")
                        .join("agy.exe"),
                );
            } else {
                out.push(home.join(".local").join("bin").join("agy"));
            }
        }
        out.extend(host.system_dirs().iter().map(|d| d.join("agy")));
        out
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["models".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        NETWORK_ENV.to_vec()
    }

    fn fixed_env(&self) -> Vec<(String, String)> {
        // Do not let it replace itself in the middle of a Plenipo task (ADR-059): Plenipo
        // updates it between tasks. Only `true` turns its background updates off (1.2.13).
        vec![("AGY_CLI_DISABLE_AUTO_UPDATE".into(), "true".into())]
    }

    fn own_home(&self) -> Vec<(&'static str, String)> {
        vec![(SETTINGS, SETTINGS_TEXT.to_owned())]
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        // `-p=`: one task, its words from stdin (a bare `-p` would take the next flag as them).
        let mut args: Vec<String> = [
            "-p=",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--mode",
            "plan",
            "--sandbox",
        ]
        .map(String::from)
        .to_vec();
        if let Some(model) = &request.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let ProviderSession::Resume { id } = &request.session {
            args.extend(["--conversation".into(), id.clone()]);
        }
        args
    }

    fn accepts_tools(&self) -> bool {
        false
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        let expected = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { .. } => None,
        };
        Box::new(Parser {
            state: TurnState::new(LABEL),
            expected,
            answer: String::new(),
            model: request.model.clone(),
            verified: false,
        })
    }

    // ---- The AI tools page (Phase 19) ----------------------------------------------------

    /// Antigravity has no sign-in command: started on its own, it asks you to sign in ("Launch
    /// the CLI without arguments to sign in", its own words). Signing out is done inside it.
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        match action {
            AccountAction::SignIn => Some(Vec::new()),
            AccountAction::SignOut => None,
        }
    }

    /// `agy update` checks and installs in one step (it has no check-only form), with
    /// standard input closed and without a sign-in.
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::None
    }

    fn update_command(&self) -> Option<Vec<String>> {
        Some(vec!["update".into()])
    }

    fn status_check(&self, _dir: &Path) -> StatusCheck {
        StatusCheck::Command(vec!["models".into()])
    }

    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        out.succeeded().then(|| listed_models(&out.stdout))?
    }
}

/// `agy models`: one `name<TAB>Label` line per model, after "Fetching available models...".
/// `None` when no line is a model.
fn listed_models(stdout: &str) -> Option<Vec<KnownModel>> {
    let models: Vec<KnownModel> = stdout
        .lines()
        .filter_map(|line| {
            let (name, label) = line.trim().split_once('\t')?;
            let name = model_name(name)?;
            let label = plain_name(label, 64).unwrap_or_else(|| name.clone());
            Some(KnownModel::new(&name, &label, &[]))
        })
        .collect();
    (!models.is_empty()).then_some(models)
}

/// `agy models` lists models only when signed in to Google; signed out it says "Please sign
/// in" and exits with 1.
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
    let all = out.combined().to_ascii_lowercase();
    if all.contains("sign in") || all.contains("log in") || all.contains("not logged in") {
        return status(AuthState::SignedOut, None, None);
    }
    if let (Some(0), Some(models)) = (out.exit_code, listed_models(&out.stdout)) {
        // Signed in to Google, it also offers other companies' models; with a Gemini API key
        // (a setting Plenipo never writes, and a variable it never passes) it lists only
        // Gemini's (both recorded, 1.2.13). A Google sign-in is recognized only by a model
        // Plenipo's own list says another company made.
        let known = Antigravity.capabilities().known_models;
        let another_company = models.iter().any(|m| {
            known.iter().any(|k| {
                k.name == m.name && k.maker.as_ref().is_some_and(|w| w.id != makers::GOOGLE.0)
            })
        });
        if another_company {
            return status(AuthState::Subscription, Some("Google sign-in"), None);
        }
        if models.iter().all(|m| m.name.starts_with("gemini-")) {
            return status(
                AuthState::ApiKey,
                Some("Gemini API key"),
                Some(
                    "Antigravity listed only Gemini's models, as it does when it uses a Gemini \
                     API key (billed per use), not your Google sign-in."
                        .into(),
                ),
            );
        }
        return status(
            AuthState::Unverified,
            None,
            Some(
                "Antigravity listed models Plenipo cannot place, so it cannot tell whether it is \
                 using your Google sign-in or a key billed per use."
                    .into(),
            ),
        );
    }
    let said = first_line(
        out.combined()
            .lines()
            .find(|l| !l.trim().is_empty() && !l.contains("Fetching available models"))
            .unwrap_or(""),
        120,
    );
    status(
        AuthState::Unknown,
        None,
        Some(if said.is_empty() {
            "Antigravity did not list its models, so Plenipo cannot tell how it is signed in."
                .into()
        } else {
            format!("Antigravity did not list its models: {said}")
        }),
    )
}

/// One task's JSON lines (Antigravity 1.2.13, recorded on the owner's PC).
struct Parser {
    state: TurnState,
    /// The conversation Plenipo asked to continue.
    expected: Option<String>,
    /// The answer being written, from `text_delta`s (at most [`MAX_RESULT_TEXT`] kept).
    answer: String,
    model: Option<String>,
    /// Its `init` reported strict permissions: it read Plenipo's settings. Nothing it does
    /// before that is accepted.
    verified: bool,
}

impl Parser {
    fn stop(&mut self, reason: String) -> Parsed {
        let stop = Stop {
            outcome: TurnOutcome::Failed,
            reason,
        };
        self.state.stop = Some(stop.clone());
        Parsed {
            stop: Some(stop),
            ..Parsed::none()
        }
    }

    fn init(&mut self, v: &Value) -> Parsed {
        let id = v
            .get("conversation_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| cap(s, 128));
        // Plenipo's settings were read: they are what makes its permissions strict.
        let mode = v.pointer("/init/permission_mode").and_then(Value::as_str);
        if mode != Some(STRICT) {
            return self.stop(format!(
                "Antigravity did not start with Plenipo's settings for it (its permissions were \
                 {:?}, not strict), so Plenipo stopped the task before it began.",
                mode.unwrap_or("not reported")
            ));
        }
        self.verified = true;
        let mut parsed = Parsed::one(AgentEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: self.model.clone(),
        });
        if let (Some(expected), Some(reported)) = (&self.expected, &id) {
            if expected != reported {
                parsed.events.push(AgentEvent::Notice {
                    level: NoticeLevel::Warning,
                    text: format!(
                        "Antigravity reported conversation {reported} instead of {expected}; \
                         Plenipo will continue {reported}."
                    ),
                });
            }
        }
        if id.is_some() {
            self.state.provider_session_id = id;
        }
        self.state.model = self.model.clone();
        parsed
    }

    /// Stop the task: a step or a finished answer came before `init` said the permissions
    /// were strict.
    fn unverified(&mut self) -> Parsed {
        self.stop(
            "Antigravity did not say it started with Plenipo's settings for it before it began, \
             so Plenipo stopped the task."
                .into(),
        )
    }

    fn step(&mut self, step: &Value) -> Parsed {
        if !self.verified {
            return self.unverified();
        }
        let kind = step.get("step_type").and_then(Value::as_str).unwrap_or("");
        let state = step.get("state").and_then(Value::as_str).unwrap_or("");
        // A step that names a tool is a tool, whatever kind it says it is.
        let names_a_tool = step.get("tool_name").is_some() || step.get("tool_info").is_some();
        match kind {
            "agent_response" if !names_a_tool => {
                let mut parsed = Parsed::none();
                if let Some(delta) = step.get("text_delta").and_then(Value::as_str) {
                    if !delta.is_empty() {
                        if self.answer.len() < MAX_RESULT_TEXT {
                            self.answer.push_str(delta);
                        }
                        parsed.events.push(AgentEvent::TextDelta {
                            text: delta.to_owned(),
                        });
                    }
                }
                if state == "DONE" {
                    let text = std::mem::take(&mut self.answer);
                    if !text.trim().is_empty() {
                        self.state.last_message = Some(text.clone());
                        parsed.events.push(AgentEvent::Message { text });
                    }
                }
                parsed
            }
            _ if kind == "tool" || names_a_tool => self.tool(step, state),
            _ => Parsed::none(),
        }
    }

    fn tool(&mut self, step: &Value, state: &str) -> Parsed {
        let name = step
            .get("tool_name")
            .and_then(Value::as_str)
            .map_or_else(|| "a tool".to_owned(), |n| cap(n, 80));
        match state {
            "DONE" if !HARMLESS_TOOLS.contains(&name.as_str()) => self.stop(format!(
                "Antigravity used its own tool {name}, which Plenipo does not allow, so Plenipo \
                 stopped the task."
            )),
            "DONE" => Parsed::one(AgentEvent::ToolResult {
                tool: Some(name),
                is_error: false,
                summary: "done".into(),
            }),
            "ERROR" => {
                let message = step
                    .pointer("/tool_info/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                // Refused for want of permission (recorded: "permission check failed for
                // command …: user denied permission to run command"). A tool that failed for any
                // other reason may have run, so the task stops.
                let lower = message.to_ascii_lowercase();
                if !(lower.contains("permission check failed")
                    || lower.contains("denied permission"))
                {
                    return self.stop(format!(
                        "Antigravity's own tool {name} ran and failed; Plenipo does not allow its \
                         own tools, so it stopped the task."
                    ));
                }
                Parsed::one(AgentEvent::ToolResult {
                    tool: Some(name),
                    is_error: true,
                    summary: first_line(message, MAX_SUMMARY),
                })
            }
            _ => Parsed::one(AgentEvent::ToolUse {
                tool: name,
                summary: "asked to use one of its own tools".into(),
            }),
        }
    }

    fn result(&mut self, r: &Value) -> Parsed {
        let mut parsed = Parsed::none();
        if let Some(id) = r
            .get("conversation_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            self.state.provider_session_id = Some(cap(id, 128));
        }
        if let Some(u) = r.get("usage").filter(|u| u.is_object()) {
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            let usage = TokenUsage {
                input_tokens: n("input_tokens"),
                cached_input_tokens: n("cache_read_tokens"),
                output_tokens: n("output_tokens"),
            };
            self.state.usage = Some(usage);
            parsed.events.push(AgentEvent::Usage { usage });
        }
        if let Some(seconds) = r.get("duration_seconds").and_then(Value::as_f64) {
            if seconds.is_finite() && seconds >= 0.0 {
                self.state.provider_duration_ms = Some((seconds * 1000.0) as u64);
            }
        }
        let refused: Vec<String> = r
            .get("denied_actions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|a| {
                        a.get("display_name")
                            .or_else(|| a.get("action"))
                            .and_then(Value::as_str)
                            .map(|s| cap(s, 60))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !refused.is_empty() {
            parsed.events.push(AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: format!(
                    "Antigravity asked to use its own tools ({}); they are off for Plenipo's \
                     tasks, so it went on without them.",
                    refused.join(", ")
                ),
            });
        }
        match r.get("status").and_then(Value::as_str) {
            Some("SUCCESS") if !self.verified => {
                let stopped = self.unverified();
                parsed.stop = stopped.stop;
            }
            Some("SUCCESS") => {
                self.state.completed = true;
                let text = r.get("response").and_then(Value::as_str).unwrap_or("");
                if !text.trim().is_empty() {
                    self.state.final_text = Some(text.to_owned());
                }
            }
            _ => {
                let error = r
                    .get("error")
                    .and_then(Value::as_str)
                    .filter(|e| !e.trim().is_empty())
                    .unwrap_or("Antigravity reported that the task failed");
                self.state.error = Some(cap(error, MAX_EVENT_TEXT));
            }
        }
        parsed
    }
}

impl TurnParser for Parser {
    /// The task's words as the one JSON message Antigravity reads from stdin.
    fn input(&mut self, prompt: String) -> String {
        let message = json!({ "event": "user", "message": { "role": "user", "content": prompt } });
        format!("{message}\n")
    }

    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let parsed = match v.get("event").and_then(Value::as_str) {
            Some("init") => self.init(&v),
            Some("step_update") => match v.get("step_update") {
                Some(step) => self.step(step),
                None => Parsed::none(),
            },
            Some("result") => match v.get("result") {
                Some(r) => self.result(r),
                None => Parsed::none(),
            },
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
    use crate::agent::dto::Effort;
    use crate::dto::ExecutionState;

    fn probe(stdout: &str, stderr: &str, code: i32) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
            ..ProbeOutput::default()
        }
    }

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(10),
        }
    }

    fn request(session: ProviderSession, model: Option<&str>) -> TurnRequest {
        TurnRequest {
            session,
            model: model.map(str::to_owned),
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
        }
    }

    /// `agy models` on the owner's PC (1.2.13), signed in.
    const MODELS: &str = "Fetching available models...\n\
        gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
        gemini-3.1-pro-low\tGemini 3.1 Pro (Low)\n\
        claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n\
        gpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n";

    #[test]
    fn the_sign_in_check_tells_a_google_sign_in_from_signed_out() {
        let s = parse_auth(&probe(MODELS, "", 0));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("Google sign-in"));
        let out = probe(
            "Fetching available models...\n",
            "Error: Please sign in to view available models. Launch the CLI without arguments \
             to sign in.\n",
            1,
        );
        assert_eq!(parse_auth(&out).state, AuthState::SignedOut);
        // Only Gemini's models: the list a Gemini API key gives.
        let key = parse_auth(&probe(
            "Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\n",
            "",
            0,
        ));
        assert_eq!(key.state, AuthState::ApiKey);
        // Nothing listed and nothing understood: never a subscription.
        let odd = parse_auth(&probe("Fetching available models...\n", "boom\n", 2));
        assert_eq!(odd.state, AuthState::Unknown);
        assert!(odd.detail.unwrap().contains("boom"));
        assert_eq!(
            parse_auth(&probe("Fetching available models...\n", "", 0)).state,
            AuthState::Unknown
        );
        let slow = ProbeOutput {
            timed_out: true,
            ..ProbeOutput::default()
        };
        assert_eq!(parse_auth(&slow).state, AuthState::Unknown);
    }

    #[test]
    fn a_google_sign_in_is_recognized_only_by_another_companys_model() {
        // Google's other models (as a Gemini API key's list could have): not recognized.
        let odd = parse_auth(&probe(
            "Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
             gemma-3-27b\tGemma 3 27B\n",
            "",
            0,
        ));
        assert_eq!(odd.state, AuthState::Unverified);
        assert!(
            !Antigravity.capabilities().billing_checked_per_turn,
            "so never ready"
        );
        // A model Plenipo does not know by that name is not enough either.
        let unknown = parse_auth(&probe(
            "Fetching available models...\nclaude-9\tClaude 9\n",
            "",
            0,
        ));
        assert_eq!(unknown.state, AuthState::Unverified);
        // One Plenipo knows another company made: a Google sign-in.
        let signed_in = parse_auth(&probe(
            "Fetching available models...\ngpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n",
            "",
            0,
        ));
        assert_eq!(signed_in.state, AuthState::Subscription);
    }

    #[test]
    fn its_models_are_read_from_its_own_list() {
        let models = Antigravity.parse_models(&probe(MODELS, "", 0)).unwrap();
        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "gemini-3.8-flash-high",
                "gemini-3.1-pro-low",
                "claude-sonnet-4-6",
                "gpt-oss-120b-medium"
            ]
        );
        assert_eq!(models[2].label, "Claude Sonnet 4.6 (Thinking)");
        // Who made a reported model comes from Plenipo's own list, never from the tool.
        assert!(models.iter().all(|m| m.maker.is_none()));
        assert_eq!(
            Antigravity.parse_models(&probe("Fetching available models...\n", "", 0)),
            None
        );
        assert_eq!(Antigravity.parse_models(&probe(MODELS, "", 1)), None);
    }

    #[test]
    fn it_lists_three_companies_models_and_says_who_made_each() {
        let caps = Antigravity.capabilities();
        assert!(caps.runs_other_makers);
        assert!(caps.effort_levels.is_empty());
        let by = |id: &str| {
            caps.known_models
                .iter()
                .filter(|m| m.maker.as_ref().map(|k| k.id.as_str()) == Some(id))
                .count()
        };
        assert_eq!((by("google"), by("anthropic"), by("openai")), (11, 2, 1));
        assert_eq!(caps.known_models.len(), 14);
        assert!(caps
            .effort_levels_for(Some("gemini-3.1-pro-high"))
            .is_empty());
        assert!(!caps.effort_levels.contains(&Effort::Max));
    }

    #[test]
    fn a_task_is_read_only_its_words_never_in_the_arguments() {
        let new = Antigravity.turn_args(&request(ProviderSession::default(), None));
        assert_eq!(
            new,
            [
                "-p=",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--mode",
                "plan",
                "--sandbox"
            ]
        );
        let again = Antigravity.turn_args(&request(
            ProviderSession::Resume { id: "c-1".into() },
            Some("gemini-3.1-pro-high"),
        ));
        assert_eq!(
            &again[8..],
            ["--model", "gemini-3.1-pro-high", "--conversation", "c-1"]
        );
        // The words go in on stdin, as one JSON message.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        assert_eq!(p.open("hi"), None);
        let line = p.input("Say \"OK\"\nplease".into());
        assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
        let v: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(
            v,
            json!({"event": "user", "message": {"role": "user", "content": "Say \"OK\"\nplease"}})
        );
    }

    #[test]
    fn its_own_settings_turn_its_tools_and_paid_credits_off() {
        let files = Antigravity.own_home();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, SETTINGS);
        let s: Value = serde_json::from_str(&files[0].1).unwrap();
        // Paid AI credits at its default, off: never turned on.
        assert!(s.get("useG1Credits").is_none());
        assert_eq!(s["toolPermission"], json!("strict"));
        // Exactly the file Antigravity writes back (keys in order, two spaces, a last line),
        // so it is replaced only when something changed it.
        assert_eq!(
            files[0].1,
            serde_json::to_string_pretty(&s).unwrap() + "\n",
            "Antigravity's own layout"
        );
        assert_eq!(
            s.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["permissions", "toolPermission"]
        );
        assert_eq!(
            s["permissions"]["deny"],
            json!([
                "command(*)",
                "read_url(*)",
                "read_file(*)",
                "write_file(*)",
                "mcp(*)"
            ])
        );
        assert!(s.get("modelProvider").is_none(), "never a Gemini API key");
        assert_eq!(
            Antigravity.fixed_env(),
            [("AGY_CLI_DISABLE_AUTO_UPDATE".to_owned(), "true".to_owned())]
        );
        assert!(!Antigravity.accepts_tools());
    }

    /// The owner's first task, recorded (1.2.13).
    const TASK: &[&str] = &[
        r#"{"event":"init","conversation_id":"b767","init":{"cwd":"x","tools":["run_command"],"permission_mode":"strict"}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"b767","step_index":0,"state":"DONE","step_type":"user_input"}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"b767","step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"OK"}}"#,
        r#"{"event":"step_update","step_update":{"conversation_id":"b767","step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":5.3,"usage":{"input_tokens":13685,"output_tokens":979,"thinking_tokens":978,"cache_read_tokens":0,"total_tokens":14664}}}"#,
        r#"{"event":"result","result":{"conversation_id":"b767","status":"SUCCESS","response":"OK\n","duration_seconds":5.4362304,"num_turns":1,"usage":{"input_tokens":13685,"output_tokens":979,"thinking_tokens":978,"cache_read_tokens":7,"total_tokens":14664}}}"#,
    ];

    fn run(p: &mut Box<dyn TurnParser>, lines: &[&str]) -> Vec<Parsed> {
        lines.iter().map(|l| p.line(l, false)).collect()
    }

    #[test]
    fn a_task_streams_its_answer_and_ends_with_its_result() {
        let mut p = Antigravity.parser(&request(
            ProviderSession::default(),
            Some("gemini-3.8-flash-high"),
        ));
        let out = run(&mut p, TASK);
        assert_eq!(
            out[0].events,
            [AgentEvent::SessionStarted {
                provider_session_id: Some("b767".into()),
                model: Some("gemini-3.8-flash-high".into()),
            }]
        );
        assert_eq!(out[2].events, [AgentEvent::TextDelta { text: "OK".into() }]);
        assert_eq!(
            out[3].events,
            [
                AgentEvent::TextDelta { text: "\n".into() },
                AgentEvent::Message {
                    text: "OK\n".into()
                }
            ]
        );
        let usage = TokenUsage {
            input_tokens: 13685,
            cached_input_tokens: 7,
            output_tokens: 979,
        };
        assert_eq!(out[4].events, [AgentEvent::Usage { usage }]);
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("OK\n"));
        assert_eq!(r.provider_session_id.as_deref(), Some("b767"));
        assert_eq!(r.usage, Some(usage));
        assert_eq!(r.duration_ms, Some(5436));
        assert_eq!(r.ignored_lines, 0);
    }

    #[test]
    fn continuing_a_conversation_names_it_and_warns_when_another_comes_back() {
        let mut p = Antigravity.parser(&request(
            ProviderSession::Resume { id: "b767".into() },
            None,
        ));
        assert_eq!(run(&mut p, &TASK[..1])[0].events.len(), 1);
        let mut p =
            Antigravity.parser(&request(ProviderSession::Resume { id: "zzz".into() }, None));
        let out = run(&mut p, &TASK[..1]);
        assert!(matches!(
            &out[0].events[1],
            AgentEvent::Notice { level: NoticeLevel::Warning, text } if text.contains("instead of zzz")
        ));
    }

    #[test]
    fn a_task_without_plenipos_settings_is_stopped_before_it_begins() {
        for init in [
            r#"{"event":"init","conversation_id":"c","init":{"permission_mode":"request-review"}}"#,
            r#"{"event":"init","conversation_id":"c","init":{}}"#,
        ] {
            let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
            let parsed = p.line(init, false);
            let stop = parsed.stop.expect("stopped");
            assert_eq!(stop.outcome, TurnOutcome::Failed);
            assert!(stop
                .reason
                .contains("did not start with Plenipo's settings"));
            let r = p.finish(&end(ExecutionState::Cancelled, None));
            assert_eq!(r.outcome, TurnOutcome::Failed);
        }
    }

    #[test]
    fn nothing_is_accepted_before_it_says_it_read_plenipos_settings() {
        // A step first: stopped.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        let stop = p.line(TASK[2], false).stop.expect("stopped");
        assert!(stop
            .reason
            .contains("did not say it started with Plenipo's settings"));
        assert_eq!(
            p.finish(&end(ExecutionState::Cancelled, None)).outcome,
            TurnOutcome::Failed
        );
        // A finished answer with no `init`: stopped, never Completed.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        assert!(p.line(TASK[4], false).stop.is_some());
        assert_eq!(
            p.finish(&end(ExecutionState::Succeeded, Some(0))).outcome,
            TurnOutcome::Failed
        );
        // An error with no `init` (signed out, recorded) keeps its own meaning.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        let signed_out = r#"{"event":"result","result":{"conversation_id":"","status":"ERROR","response":"","error":"authentication failed or timed out"}}"#;
        assert!(p.line(signed_out, false).stop.is_none());
        assert_eq!(
            p.finish(&end(ExecutionState::Failed, Some(1))).outcome,
            TurnOutcome::AuthRequired
        );
    }

    #[test]
    fn a_tool_that_failed_for_another_reason_or_under_another_kind_stops_the_task() {
        for step in [
            // Failed, but not for want of permission: it may have run.
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ERROR","step_type":"tool","tool_name":"search_web","tool_info":{"name":"search_web","error":{"type":"TOOL_ERROR","message":"request timed out"}}}}"#,
            // A tool reported under a step kind Plenipo does not know.
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"browser_action","tool_name":"open_browser_url"}}"#,
        ] {
            let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
            p.line(TASK[0], false);
            let stop = p.line(step, false).stop.expect(step);
            assert!(stop.reason.contains("Plenipo does not allow"), "{stop:?}");
        }
        // A step kind it does not know, naming no tool, is ignored.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        p.line(TASK[0], false);
        let other = r#"{"event":"step_update","step_update":{"step_index":3,"state":"DONE","step_type":"error_message"}}"#;
        assert!(p.line(other, false).stop.is_none());
    }

    #[test]
    fn a_very_long_answer_is_kept_to_the_result_limit() {
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        p.line(TASK[0], false);
        let piece = "x".repeat(MAX_RESULT_TEXT);
        for state in ["ACTIVE", "ACTIVE", "DONE"] {
            let line = json!({"event": "step_update", "step_update": {"state": state, "step_type": "agent_response", "text_delta": piece}}).to_string();
            p.line(&line, false);
        }
        p.line(r#"{"event":"result","result":{"conversation_id":"b767","status":"SUCCESS","response":""}}"#, false);
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert!(r.text.unwrap().len() <= MAX_RESULT_TEXT);
    }

    /// The owner's E9 check: asked to write a file, it tried to run a program; refused.
    #[test]
    fn a_refused_tool_is_shown_and_one_that_ran_stops_the_task() {
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        let out = run(
            &mut p,
            &[
                TASK[0],
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"Get-ChildItem"}}}}"#,
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"ERROR","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","error":{"type":"TOOL_ERROR","message":"permission check failed for command \"Get-ChildItem\": user denied permission to run command:\nGet-ChildItem"}}}}"#,
                r#"{"event":"result","result":{"conversation_id":"b767","status":"SUCCESS","response":"","num_turns":1,"denied_actions":[{"action":"command","display_name":"RunCommand"}]}}"#,
            ],
        );
        assert!(
            matches!(&out[1].events[0], AgentEvent::ToolUse { tool, .. } if tool == "run_command")
        );
        assert!(matches!(
            &out[2].events[0],
            AgentEvent::ToolResult { is_error: true, summary, .. } if summary.starts_with("permission check failed")
        ));
        assert!(out[2].stop.is_none());
        assert!(matches!(
            &out[3].events[0],
            AgentEvent::Notice { text, .. } if text.contains("(RunCommand)")
        ));
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);

        // One of its own tools that finished: stopped at once.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        let out = run(
            &mut p,
            &[
                TASK[0],
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"tool","tool_name":"search_web"}}"#,
            ],
        );
        let stop = out[1].stop.clone().expect("stopped");
        assert!(stop.reason.contains("its own tool search_web"));
        assert_eq!(
            p.finish(&end(ExecutionState::Cancelled, None)).outcome,
            TurnOutcome::Failed
        );
        // Finishing or waiting changes nothing.
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        let out = run(
            &mut p,
            &[
                TASK[0],
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"tool","tool_name":"finish"}}"#,
            ],
        );
        assert!(out[1].stop.is_none());
    }

    #[test]
    fn its_errors_are_classified() {
        for (error, outcome) in [
            ("authentication failed or timed out", TurnOutcome::AuthRequired),
            (
                "Error 429, Message: Resource has been exhausted (e.g. check quota)., Status: RESOURCE_EXHAUSTED",
                TurnOutcome::UsageLimited,
            ),
            ("model exploded", TurnOutcome::Failed),
        ] {
            let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
            let line = json!({"event": "result", "result": {"conversation_id": "", "status": "ERROR", "response": "", "error": error}}).to_string();
            p.line(&line, false);
            let r = p.finish(&end(ExecutionState::Failed, Some(3)));
            assert_eq!(r.outcome, outcome, "{error}");
        }
    }

    #[test]
    fn junk_is_ignored_and_counted() {
        let mut p = Antigravity.parser(&request(ProviderSession::default(), None));
        assert_eq!(p.line("<html>", false).events.len(), 1);
        assert!(p.line(r#"{"event":"mystery"}"#, false).events.is_empty());
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::MalformedOutput);
        assert_eq!(r.ignored_lines, 2);
    }

    #[test]
    fn it_signs_in_by_starting_on_its_own_and_updates_itself() {
        assert_eq!(
            Antigravity.account_command(AccountAction::SignIn),
            Some(Vec::new())
        );
        assert_eq!(Antigravity.account_command(AccountAction::SignOut), None);
        assert_eq!(
            Antigravity.update_command(),
            Some(vec!["update".to_owned()])
        );
        assert_eq!(Antigravity.newest_version(), NewestVersion::None);
        let host = HostEnv::new(None, Some(PathBuf::from("/h")), None);
        let first = Antigravity.known_locations(&host)[0].clone();
        assert!(first.ends_with(if cfg!(windows) {
            "AppData/Local/agy/bin/agy.exe"
        } else {
            ".local/bin/agy"
        }));
    }
}
