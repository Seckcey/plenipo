//! The provider-neutral runtime adapter contract (ADR-007) and helpers shared by adapters.
//!
//! Plan verbs → contract:
//!
//! | Plan                      | Here                                                        |
//! | ------------------------- | ----------------------------------------------------------- |
//! | detectInstallation        | [`RuntimeAdapter::executable_name`] + discovery, `version_args` |
//! | detectAuthentication      | [`RuntimeAdapter::auth_args`] + [`RuntimeAdapter::parse_auth`] |
//! | listRuntimeCapabilities   | [`RuntimeAdapter::capabilities`]                            |
//! | startSession / resumeSession / submitTask | [`RuntimeAdapter::turn_args`] ([`TurnRequest`]) |
//! | streamEvents              | [`TurnParser::line`]                                        |
//! | cancelExecution           | the supervisor's process-tree kill (common to all adapters) |
//! | closeSession              | Plenipo marks the session closed (the service)              |
//! | normalizeResult           | [`TurnParser::finish`]                                      |

use std::path::{Path, PathBuf};

use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    AgentEvent, AuthStatus, Effort, NoticeLevel, RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::dto::{ExecutionState, TokenUsage};

/// Longest final answer kept in a result.
pub const MAX_RESULT_TEXT: usize = 64 * 1024;
/// Longest text kept in one stored activity event or error.
pub const MAX_EVENT_TEXT: usize = 4 * 1024;
/// Longest tool summary.
pub const MAX_SUMMARY: usize = 200;
/// Lines of stderr kept to explain failures.
const STDERR_TAIL: usize = 20;

/// Captured output of a short probe command (version, sign-in status).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    /// The probe could not be started at all.
    pub spawn_error: Option<String>,
}

impl ProbeOutput {
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && self.spawn_error.is_none()
    }

    /// stdout followed by stderr.
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// Which provider session a turn runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSession {
    /// Start a new provider session. Adapters that let the caller choose the ID get one.
    New { preassigned: Option<String> },
    /// Continue the provider session with this (confirmed) ID.
    Resume { id: String },
}

/// Everything an adapter needs to build one turn's launch. The objective itself is not
/// here: it is always written to stdin, never placed in arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRequest {
    pub session: ProviderSession,
    /// Validated model name, or `None` for the runtime's default.
    pub model: Option<String>,
    /// One of the runtime's effort levels, or `None` for its default.
    pub effort: Option<Effort>,
    /// The sign-in check confirmed a subscription. When false, a runtime that checks billing
    /// per turn must see a subscription credential in the stream, or stop the turn.
    pub billing_confirmed: bool,
}

/// Why a parser asks Plenipo to stop the process immediately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub outcome: TurnOutcome,
    pub reason: String,
}

/// Result of parsing one stdout line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub events: Vec<AgentEvent>,
    pub stop: Option<Stop>,
}

impl Parsed {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn one(event: AgentEvent) -> Self {
        Self {
            events: vec![event],
            stop: None,
        }
    }
}

/// How the turn's process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEnd {
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    /// False when the process could not be started.
    pub started: bool,
    pub detail: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Turns one turn's output stream into normalized events and a result.
pub trait TurnParser: Send {
    /// One stdout line (JSON lines expected).
    fn line(&mut self, text: &str, truncated: bool) -> Parsed;
    /// One stderr line; kept to explain failures, never parsed as events.
    fn stderr(&mut self, text: &str);
    /// The process ended: produce the normalized result.
    fn finish(&mut self, end: &ProcessEnd) -> TurnResult;
}

/// A provider runtime. Implementations hold no per-turn state; they only describe how to
/// find, check, launch, and understand their CLI.
pub trait RuntimeAdapter: Send + Sync + 'static {
    /// Stable ID used in records and the UI, e.g. `claude-code`.
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Provider ID stored with executions, e.g. `anthropic`.
    fn provider(&self) -> &'static str;
    fn provider_label(&self) -> &'static str;
    fn capabilities(&self) -> RuntimeCapabilities;
    fn install_hint(&self) -> &'static str;
    fn login_hint(&self) -> &'static str;

    // ---- detectInstallation -------------------------------------------------------------

    /// File name searched on PATH (without extension).
    fn executable_name(&self) -> &'static str;
    /// Well-known install locations checked after PATH (full file paths).
    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf>;
    /// Map a found file to the executable Plenipo should run, or `None` if unusable.
    /// Receives Windows shims (`.cmd`/`.ps1`) and Unix scripts too. Default: Windows accepts
    /// only `.exe`; Unix accepts the file as is.
    fn resolve(&self, found: &Path) -> Option<PathBuf> {
        default_resolve(found)
    }
    fn version_args(&self) -> Vec<String> {
        vec!["--version".into()]
    }
    fn parse_version(&self, out: &ProbeOutput) -> Option<String> {
        find_version(&out.combined())
    }

    // ---- detectAuthentication -----------------------------------------------------------

    fn auth_args(&self) -> Vec<String>;
    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus;

    // ---- environment --------------------------------------------------------------------

    /// Variables passed through from Plenipo's own environment when set (names only).
    /// Never API keys or other credentials.
    fn passthrough_env(&self) -> Vec<&'static str>;
    /// Variables Plenipo sets for this runtime's processes.
    fn fixed_env(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    // ---- start/resume session + submit task, stream, normalize --------------------------

    /// Whether Plenipo chooses the provider session ID for new sessions.
    fn preassigns_session_id(&self) -> bool {
        false
    }
    fn turn_args(&self, request: &TurnRequest) -> Vec<String>;
    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser>;
}

/// Proxy and certificate settings every runtime may need on managed networks.
pub const NETWORK_ENV: &[&str] = &[
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
];

fn default_resolve(found: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let exe = found
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        exe.then(|| found.to_path_buf())
    } else {
        Some(found.to_path_buf())
    }
}

/// First `N.N[.N][-suffix]` token in `text`.
pub fn find_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
        .map(|t| t.trim_start_matches('v'))
        .find(|t| {
            let mut parts = t.split('.');
            let major = parts.next().unwrap_or("");
            let minor = parts.next().unwrap_or("");
            !major.is_empty()
                && major.chars().all(|c| c.is_ascii_digit())
                && minor.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|t| cap(t, 64))
}

/// Cut `text` to at most `max` bytes on a character boundary, marking the cut.
pub fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// First line of `text`, capped for one-line summaries.
pub fn first_line(text: &str, max: usize) -> String {
    cap(
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim(),
        max,
    )
}

/// Classify a provider error message. Order matters: usage limits often mention "limit"
/// and "429"; sign-in problems mention "login"/"401".
pub fn classify_error(message: &str) -> TurnOutcome {
    let m = message.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| m.contains(n));
    if any(&[
        "usage limit",
        "rate limit",
        "rate_limit",
        "hit your limit",
        "limit reached",
        "quota",
        "too many requests",
        "429",
    ]) {
        TurnOutcome::UsageLimited
    } else if any(&[
        "not logged in",
        "please run /login",
        "run `codex login`",
        "log in again",
        "login required",
        "invalid api key",
        "authentication",
        "unauthorized",
        "401",
        "oauth token",
        "token has expired",
        "token expired",
    ]) {
        TurnOutcome::AuthRequired
    } else if any(&[
        "connection error",
        "connection refused",
        "network",
        "enotfound",
        "econnrefused",
        "econnreset",
        "service unavailable",
        "overloaded",
        "503",
        "502",
        "stream disconnected",
    ]) {
        TurnOutcome::ProviderUnavailable
    } else {
        TurnOutcome::Failed
    }
}

/// State every turn parser keeps; adapters compose it.
#[derive(Debug, Default)]
pub struct TurnState {
    pub runtime_label: &'static str,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    /// Last complete assistant message.
    pub last_message: Option<String>,
    /// The provider's own final answer, when it reports one separately.
    pub final_text: Option<String>,
    /// The provider reported successful completion.
    pub completed: bool,
    /// The provider reported a terminal error.
    pub error: Option<String>,
    /// Last non-fatal error (e.g. a retry notice).
    pub last_warning: Option<String>,
    pub provider_duration_ms: Option<u64>,
    pub stop: Option<Stop>,
    pub understood: u32,
    pub malformed: u32,
    pub unknown: u32,
    stderr_tail: Vec<String>,
}

impl TurnState {
    pub fn new(runtime_label: &'static str) -> Self {
        Self {
            runtime_label,
            ..Self::default()
        }
    }

    /// A stdout line that is not JSON. Reported once, counted always.
    pub fn malformed_line(&mut self, truncated: bool) -> Parsed {
        self.malformed += 1;
        if self.malformed > 1 {
            return Parsed::none();
        }
        let why = if truncated {
            "an output line was too long to read"
        } else {
            "output that is not a recognized event"
        };
        Parsed::one(AgentEvent::Notice {
            level: NoticeLevel::Warning,
            text: format!("{} produced {why}; it was ignored.", self.runtime_label),
        })
    }

    pub fn stderr(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.stderr_tail.len() == STDERR_TAIL {
            self.stderr_tail.remove(0);
        }
        self.stderr_tail.push(cap(text, 1024));
    }

    fn stderr_text(&self) -> Option<String> {
        (!self.stderr_tail.is_empty()).then(|| self.stderr_tail.join("\n"))
    }

    /// Normalize the end of the turn (shared rules; see ADR-007 §7).
    pub fn finish(&mut self, end: &ProcessEnd) -> TurnResult {
        let label = self.runtime_label;
        let text = self
            .final_text
            .clone()
            .or_else(|| self.last_message.clone())
            .map(|t| cap(&t, MAX_RESULT_TEXT));
        let (outcome, summary, error) = if let Some(stop) = self.stop.clone() {
            (stop.outcome, stop.reason.clone(), Some(stop.reason))
        } else {
            match end.state {
                ExecutionState::Cancelled => {
                    let why = end.detail.clone().unwrap_or_else(|| "Cancelled".into());
                    (TurnOutcome::Cancelled, why, None)
                }
                ExecutionState::TimedOut => (
                    TurnOutcome::TimedOut,
                    "The turn exceeded its time limit and was stopped".into(),
                    end.detail.clone(),
                ),
                ExecutionState::Interrupted => (
                    TurnOutcome::Interrupted,
                    "Plenipo stopped while this turn was running".into(),
                    None,
                ),
                ExecutionState::Failed if !end.started => (
                    TurnOutcome::ProviderUnavailable,
                    format!("{label} could not be started"),
                    end.detail.clone(),
                ),
                _ => self.classify_exit(end),
            }
        };
        TurnResult {
            outcome,
            summary: first_line(&summary, 300),
            text,
            error: error.map(|e| cap(&e, MAX_EVENT_TEXT)),
            provider_session_id: self.provider_session_id.clone(),
            model: self.model.clone(),
            usage: self.usage,
            duration_ms: self.provider_duration_ms.or(end.duration_ms),
            ignored_lines: self.malformed + self.unknown,
        }
    }

    fn classify_exit(&self, end: &ProcessEnd) -> (TurnOutcome, String, Option<String>) {
        let label = self.runtime_label;
        if self.completed {
            let summary = self
                .final_text
                .as_deref()
                .or(self.last_message.as_deref())
                .map_or_else(|| "Completed".to_owned(), |t| first_line(t, 160));
            return (TurnOutcome::Completed, summary, None);
        }
        if let Some(error) = &self.error {
            let outcome = classify_error(error);
            let summary = match outcome {
                TurnOutcome::UsageLimited => {
                    format!("{label} reported a usage limit; resume this session later")
                }
                TurnOutcome::AuthRequired => format!("{label} needs you to sign in again"),
                TurnOutcome::ProviderUnavailable => format!("{label} could not reach its service"),
                _ => format!("{label} reported an error: {}", first_line(error, 200)),
            };
            return (outcome, summary, Some(error.clone()));
        }
        let stderr = self.stderr_text();
        if self.malformed > 0 && self.understood == 0 {
            return (
                TurnOutcome::MalformedOutput,
                format!("{label} produced output Plenipo could not understand"),
                stderr,
            );
        }
        match end.exit_code {
            Some(0) => (
                TurnOutcome::MalformedOutput,
                format!("{label} ended without reporting a result"),
                stderr,
            ),
            code => {
                let explained = stderr
                    .as_deref()
                    .map(classify_error)
                    .filter(|o| *o != TurnOutcome::Failed);
                let how = code.map_or_else(
                    || end.detail.clone().unwrap_or_else(|| "abnormally".into()),
                    |c| format!("with code {c}"),
                );
                match explained {
                    Some(outcome) => (
                        outcome,
                        format!("{label} exited {how}: {}", last_line(stderr.as_deref())),
                        stderr,
                    ),
                    None => (
                        TurnOutcome::Crashed,
                        format!("{label} exited {how} before reporting a result"),
                        stderr.or_else(|| self.last_warning.clone()),
                    ),
                }
            }
        }
    }
}

fn last_line(text: Option<&str>) -> String {
    text.and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()))
        .map_or_else(String::new, |l| cap(l.trim(), 200))
}

/// A short, single-line description of a tool call's target (path, command, query, …).
pub fn tool_summary(input: &serde_json::Value) -> String {
    const KEYS: &[&str] = &[
        "file_path",
        "path",
        "command",
        "pattern",
        "url",
        "query",
        "description",
        "prompt",
    ];
    let found = KEYS
        .iter()
        .find_map(|k| input.get(*k).and_then(serde_json::Value::as_str));
    found.map_or_else(String::new, |s| {
        first_line(&s.replace(['\r', '\n'], " "), MAX_SUMMARY)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(5),
        }
    }

    #[test]
    fn versions() {
        assert_eq!(
            find_version("2.1.283 (Claude Code)").as_deref(),
            Some("2.1.283")
        );
        assert_eq!(
            find_version("codex-cli 0.50.0\n").as_deref(),
            Some("0.50.0")
        );
        assert_eq!(find_version("v1.2-beta.1").as_deref(), Some("1.2-beta.1"));
        assert_eq!(find_version("no version here 3"), None);
    }

    #[test]
    fn caps_on_char_boundaries() {
        assert_eq!(cap("hello", 10), "hello");
        let capped = cap(&"é".repeat(100), 11);
        assert!(capped.len() <= 11 && capped.ends_with('…'), "{capped}");
        assert_eq!(first_line("\n\n  first \nsecond", 50), "first");
    }

    #[test]
    fn error_classification() {
        use TurnOutcome::*;
        for (msg, want) in [
            ("Claude AI usage limit reached|1760000000", UsageLimited),
            ("You've hit your limit · resets 3pm", UsageLimited),
            ("stream error: 429 Too Many Requests", UsageLimited),
            ("Invalid API key · Please run /login", AuthRequired),
            ("OAuth token has expired", AuthRequired),
            ("Not logged in", AuthRequired),
            ("API Error: Connection error.", ProviderUnavailable),
            ("503 Service Unavailable", ProviderUnavailable),
            ("Something unexpected", Failed),
        ] {
            assert_eq!(classify_error(msg), want, "{msg}");
        }
    }

    #[test]
    fn finish_rules() {
        let mut s = TurnState::new("Test");
        s.completed = true;
        s.last_message = Some("Answer line\nmore".into());
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(
            (r.outcome, r.summary.as_str()),
            (TurnOutcome::Completed, "Answer line")
        );
        assert_eq!(r.text.as_deref(), Some("Answer line\nmore"));

        let mut s = TurnState::new("Test");
        let r = s.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::Cancelled);

        let mut s = TurnState::new("Test");
        s.stderr("boom: segfault");
        let r = s.finish(&end(ExecutionState::Failed, Some(139)));
        assert_eq!(r.outcome, TurnOutcome::Crashed);
        assert_eq!(r.error.as_deref(), Some("boom: segfault"));

        let mut s = TurnState::new("Test");
        s.stderr("Error: Not logged in");
        let r = s.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::AuthRequired);

        let mut s = TurnState::new("Test");
        s.malformed_line(false);
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::MalformedOutput);
        assert_eq!(r.ignored_lines, 1);

        let mut s = TurnState::new("Test");
        let r = s.finish(&ProcessEnd {
            started: false,
            ..end(ExecutionState::Failed, None)
        });
        assert_eq!(r.outcome, TurnOutcome::ProviderUnavailable);

        let mut s = TurnState::new("Test");
        s.error = Some("usage limit reached".into());
        s.completed = false;
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::UsageLimited);

        let mut s = TurnState::new("Test");
        s.stop = Some(Stop {
            outcome: TurnOutcome::BillingNotAllowed,
            reason: "nope".into(),
        });
        let r = s.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);
    }

    #[test]
    fn malformed_is_reported_once() {
        let mut s = TurnState::new("Test");
        assert_eq!(s.malformed_line(false).events.len(), 1);
        assert!(s.malformed_line(false).events.is_empty());
        assert_eq!(s.malformed, 2);
    }

    #[test]
    fn tool_summaries() {
        let v = serde_json::json!({ "command": "ls -la\nrm x", "other": 1 });
        assert_eq!(tool_summary(&v), "ls -la rm x");
        assert_eq!(tool_summary(&serde_json::json!({})), "");
    }
}
