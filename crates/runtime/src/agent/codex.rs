//! Codex adapter (ADR-007).
//!
//! One turn = `codex exec --json --sandbox read-only --skip-git-repo-check [resume <thread>]`
//! with the objective on stdin — the same invocation OpenAI's Codex SDK uses. The thread ID
//! arrives in `thread.started`. Codex does not report its credential source in the stream, so
//! a subscription sign-in must be positively confirmed before every turn.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::adapter::{
    cap, first_line, Parsed, ProbeOutput, ProcessEnd, ProviderSession, RuntimeAdapter, TurnParser,
    TurnRequest, TurnState, MAX_EVENT_TEXT, MAX_SUMMARY, NETWORK_ENV,
};
use crate::agent::discovery::{npm_target_triple, HostEnv};
use crate::agent::dto::{
    AgentEvent, AuthState, AuthStatus, Effort, NoticeLevel, RuntimeCapabilities, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "codex";
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
            tool_posture: "Read-only sandbox: Codex may run read-only commands but cannot \
                           write files or use the network until Plenipo Guard grants \
                           capabilities (Phase 7)."
                .into(),
            // `codex exec -c model_reasoning_effort=<level>`.
            effort_levels: vec![
                Effort::Minimal,
                Effort::Low,
                Effort::Medium,
                Effort::High,
                Effort::XHigh,
            ],
        }
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

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let mut args: Vec<String> = [
            "exec",
            "--json",
            // Phase 3: no writes, no network (ADR-007 §5).
            "--sandbox",
            "read-only",
            // Each session runs in its own empty workspace, not a repository.
            "--skip-git-repo-check",
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
                summary: String::new(),
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
        }
    }

    fn feed(p: &mut dyn TurnParser, lines: &[Value]) -> Vec<AgentEvent> {
        lines
            .iter()
            .flat_map(|l| p.line(&l.to_string(), false).events)
            .collect()
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
                "--skip-git-repo-check"
            ]
        );
        let resume = Codex.turn_args(&TurnRequest {
            session: ProviderSession::Resume { id: "t-1".into() },
            model: Some("gpt-x".into()),
            effort: Some(Effort::Minimal),
            billing_confirmed: true,
        });
        assert_eq!(
            resume,
            [
                "exec",
                "--json",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--model",
                "gpt-x",
                "-c",
                "model_reasoning_effort=minimal",
                "resume",
                "t-1"
            ]
        );
        assert!(!Codex.preassigns_session_id());
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
