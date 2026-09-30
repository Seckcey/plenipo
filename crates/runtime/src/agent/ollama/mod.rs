//! Ollama adapter (ADR-017, Ollama's cloud models through its service on this PC).
//!
//! Ollama's program is found and its version read like any AI tool's, but the sign-in check and
//! every task run Plenipo's bridge ([`bridge`]): `auth`, and `chat --model M --session ID
//! [--resume] [--think LEVEL]` with the objective on stdin. The bridge talks to the Ollama
//! service on `127.0.0.1:11434` and prints one JSON object per line, read here. Plenipo chooses
//! the conversation ID, and the bridge keeps the conversation in the session's folder, because
//! Ollama keeps none. Conversation only: no tools yet (ADR-017 §4).

pub mod bridge;

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::adapter::{
    cap, model_name, NewestVersion, Parsed, ProbeOutput, ProcessEnd, ProviderSession,
    PublishedList, RuntimeAdapter, StatusCheck, TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AccountAction, AgentEvent, AuthState, AuthStatus, Effort, KnownModel, Maker,
    NoticeLevel, RuntimeCapabilities, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "ollama";
const LABEL: &str = "Ollama";
/// The model used when the worker names none: the cloud model checked on the owner's PC.
pub const DEFAULT_MODEL: &str = "gpt-oss:120b-cloud";
/// gpt-oss's thinking levels (`ollama show`: low, medium, high).
const GPT_OSS: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High];
/// Thinking levels of DeepSeek, GLM, and Kimi's cloud models (`ollama show`: low, high, max).
const TO_MAX: &[Effort] = &[Effort::Low, Effort::High, Effort::Max];
/// Every level any listed model takes, lowest first.
const ALL: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High, Effort::Max];

#[derive(Debug, Default, Clone, Copy)]
pub struct Ollama;

impl RuntimeAdapter for Ollama {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        "ollama"
    }

    fn provider_label(&self) -> &'static str {
        "Ollama"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            billing_checked_per_turn: false,
            tool_posture: "Conversation only: an Ollama worker can answer, write, and review \
                           text, but cannot read files or run programs yet."
                .into(),
            // `think` in `/api/chat`: every level one of these models accepts.
            effort_levels: ALL.to_vec(),
            // The cloud models the owner chose, checked on the owner's PC with `ollama show`
            // (Ollama 0.34.4, 2026-09-27), with the thinking levels each lists. MiniMax M3 lists
            // no levels and Nemotron 3 Ultra only on or off (on by default), so they have no
            // setting. On the free plan only gpt-oss and Nemotron answered; the others answered
            // "402 Payment Required", so their names say they need a paid Ollama plan. Other
            // cloud models can be named as `ollama list` shows them. Each says who made it
            // (ADR-081 §1): Ollama runs other companies' models, so a model not listed here is
            // made by someone Plenipo does not know.
            known_models: vec![
                KnownModel::new(DEFAULT_MODEL, "gpt-oss 120B", GPT_OSS).by(makers::OPENAI),
                KnownModel::new("nemotron-3-ultra:cloud", "Nemotron 3 Ultra", &[])
                    .by(makers::NVIDIA),
                KnownModel::new("kimi-k3:cloud", "Kimi K3 (paid plan)", TO_MAX)
                    .by(makers::MOONSHOT)
                    .same("kimi-k3"),
                KnownModel::new(
                    "deepseek-v4-pro:cloud",
                    "DeepSeek V4 Pro (paid plan)",
                    TO_MAX,
                )
                .by(makers::DEEPSEEK)
                .same("deepseek-v4-pro"),
                KnownModel::new(
                    "deepseek-v4.1-flash:cloud",
                    "DeepSeek V4.1 Flash (paid plan)",
                    TO_MAX,
                )
                .by(makers::DEEPSEEK)
                .same("deepseek-v4.1-flash"),
                KnownModel::new("glm-5.3:cloud", "GLM-5.3 (paid plan)", TO_MAX)
                    .by(makers::ZAI)
                    .same("glm-5.3"),
                KnownModel::new("glm-5.3-flash:cloud", "GLM-5.3 Flash (paid plan)", TO_MAX)
                    .by(makers::ZAI)
                    .same("glm-5.3-flash"),
                KnownModel::new("minimax-m3:cloud", "MiniMax M3 (paid plan)", &[])
                    .by(makers::MINIMAX)
                    .same("minimax-m3"),
            ],
            // Its default, gpt-oss 120B, is OpenAI's.
            default_maker: Some(Maker::new(makers::OPENAI.0, makers::OPENAI.1)),
            runs_other_makers: true,
        }
    }

    fn checked_version(&self) -> &'static str {
        "0.34.4"
    }

    fn install_hint(&self) -> &'static str {
        "Install Ollama from ollama.com/download, open it, then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Make sure Ollama is running, then open a terminal and run: ollama signin — and finish \
         signing in in your browser. Plenipo never asks for your password. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "ollama"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if cfg!(windows) {
            if let Some(home) = &host.home {
                out.push(
                    home.join("AppData")
                        .join("Local")
                        .join("Programs")
                        .join("Ollama")
                        .join("ollama.exe"),
                );
            }
        } else {
            out.extend(host.system_dirs().iter().map(|d| d.join("ollama")));
        }
        out
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["auth".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        // The bridge talks only to this PC's Ollama service: no proxies, no keys, and
        // `OLLAMA_HOST` is ignored.
        Vec::new()
    }

    fn preassigns_session_id(&self) -> bool {
        true
    }

    fn bridged(&self) -> bool {
        true
    }

    fn accepts_tools(&self) -> bool {
        false
    }

    /// Plenipo's own bridge says when it leaves earlier messages out (ADR-044 §2.5).
    fn reports_memory_shortened(&self) -> bool {
        true
    }

    /// The bridge keeps each conversation in the session's folder: whether it would leave
    /// earlier messages out is known before the step goes out.
    fn leaves_out(&self, request: &TurnRequest, prompt_bytes: usize) -> bool {
        match &request.session {
            ProviderSession::Resume { id } => {
                bridge::would_leave_out(&request.working_dir, id, prompt_bytes)
            }
            ProviderSession::New { .. } => false,
        }
    }

    /// `ollama signin` and `ollama signout` (Ollama's help, and both recorded on the owner's PC),
    /// run on the real `ollama` program, not Plenipo's bridge.
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        Some(match action {
            AccountAction::SignIn => vec!["signin".into()],
            AccountAction::SignOut => vec!["signout".into()],
        })
    }

    /// Ollama's own releases, on GitHub. Ollama has no update command: its tray app downloads a
    /// new version and asks the owner to restart it (ADR-059 §7).
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::Published(PublishedList::GitHub("ollama/ollama"))
    }

    /// The models on this PC, from the Ollama service (`GET /api/tags`, through the bridge).
    fn status_check(&self, _dir: &Path) -> StatusCheck {
        StatusCheck::Bridge(vec!["models".into()])
    }

    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        let last = out.stdout.lines().rev().find(|l| !l.trim().is_empty())?;
        let answer: Value = serde_json::from_str(last.trim()).ok()?;
        let list = answer.get("models")?.as_array()?;
        Some(
            list.iter()
                .filter_map(|m| m.get("name").and_then(Value::as_str).and_then(model_name))
                .map(|name| KnownModel {
                    label: name.clone(),
                    name,
                    effort_levels: Vec::new(),
                    maker: None,
                    points_to: None,
                    price: None,
                    same: None,
                })
                .collect(),
        )
    }

    /// Ollama lists the models downloaded to this PC, not every model it offers.
    fn reports_every_model(&self) -> bool {
        false
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
        let mut args = vec!["chat".into(), "--model".into(), model];
        match &request.session {
            ProviderSession::New { preassigned } => {
                let id = preassigned
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                args.extend(["--session".into(), id]);
            }
            ProviderSession::Resume { id } => {
                args.extend(["--session".into(), id.clone(), "--resume".into()]);
            }
        }
        if let Some(effort) = request.effort {
            args.extend(["--think".into(), effort.as_str().into()]);
        }
        args
    }

    fn parser(&self, _request: &TurnRequest) -> Box<dyn TurnParser> {
        Box::new(Parser {
            state: TurnState::new(LABEL),
        })
    }
}

fn parse_auth(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<String>, detail: Option<String>| AuthStatus {
        state,
        method,
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
    let answer = out
        .stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l.trim()).ok());
    let Some(v) = answer else {
        return status(
            AuthState::Unknown,
            None,
            Some("Ollama did not report its sign-in status.".into()),
        );
    };
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return status(AuthState::Unknown, None, Some(cap(e, 300)));
    }
    match v.get("signedIn").and_then(Value::as_bool) {
        Some(true) => {
            let plan = v
                .get("plan")
                .and_then(Value::as_str)
                .map(|p| {
                    p.chars()
                        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-'))
                        .take(32)
                        .collect::<String>()
                })
                .filter(|p| !p.trim().is_empty());
            let method = match plan {
                Some(plan) => format!("Ollama sign-in ({plan} plan)"),
                None => "Ollama sign-in".to_owned(),
            };
            status(AuthState::Subscription, Some(method), None)
        }
        Some(false) => status(AuthState::SignedOut, None, None),
        None => status(
            AuthState::Unknown,
            None,
            Some("Ollama did not report its sign-in status.".into()),
        ),
    }
}

struct Parser {
    state: TurnState,
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

impl TurnParser for Parser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let parsed = match v.get("type").and_then(Value::as_str) {
            Some("session") => {
                let id = Some(cap(str_of(&v, "id"), 128)).filter(|s| !s.is_empty());
                let model = Some(cap(str_of(&v, "model"), 128)).filter(|s| !s.is_empty());
                self.state.provider_session_id.clone_from(&id);
                self.state.model.clone_from(&model);
                Parsed::one(AgentEvent::SessionStarted {
                    provider_session_id: id,
                    model,
                })
            }
            Some("thinking") => Parsed::one(AgentEvent::Reasoning {
                text: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
            }),
            Some("text") => Parsed::one(AgentEvent::TextDelta {
                text: str_of(&v, "text").to_owned(),
            }),
            Some("answer") => {
                let text = str_of(&v, "text");
                if text.trim().is_empty() {
                    Parsed::none()
                } else {
                    self.state.last_message = Some(text.to_owned());
                    Parsed::one(AgentEvent::Message {
                        text: text.to_owned(),
                    })
                }
            }
            Some("done") => {
                self.state.completed = true;
                let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
                self.state.provider_duration_ms = v.get("durationMs").and_then(Value::as_u64);
                let usage = TokenUsage {
                    input_tokens: n("inputTokens"),
                    cached_input_tokens: n("cachedTokens"),
                    output_tokens: n("outputTokens"),
                };
                self.state.usage = Some(usage);
                let mut parsed = Parsed::one(AgentEvent::Usage { usage });
                if str_of(&v, "reason") == "length" {
                    parsed.events.push(AgentEvent::Notice {
                        level: NoticeLevel::Warning,
                        text: "The answer was cut off at the model's length limit.".into(),
                    });
                }
                parsed
            }
            // Plenipo's bridge left earlier messages out: the model's memory of the conversation
            // is shorter (ADR-044 §2.5).
            Some("notice") if v.get("leftOut").and_then(Value::as_u64).unwrap_or(0) > 0 => {
                Parsed::one(AgentEvent::MemoryShortened {
                    detail: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
                })
            }
            Some("notice") => Parsed::one(AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
            }),
            Some("error") => {
                self.state.error = Some(cap(str_of(&v, "message"), MAX_EVENT_TEXT));
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
        self.state.finish(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::dto::TurnOutcome;
    use crate::dto::ExecutionState;
    use serde_json::json;

    /// Who made "its default" (ADR-081 §2) is who made the model Plenipo runs when none is named.
    #[test]
    fn its_default_is_made_by_whoever_made_its_default_model() {
        let caps = Ollama.capabilities();
        let listed = caps
            .known_models
            .iter()
            .find(|m| m.name == DEFAULT_MODEL)
            .expect("its default model is listed");
        assert_eq!(caps.default_maker, listed.maker);
        assert!(caps.default_maker.is_some());
    }

    fn probe(stdout: &str) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
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
            duration_ms: Some(9),
        }
    }

    fn feed(p: &mut dyn TurnParser, lines: &[Value]) -> Vec<AgentEvent> {
        lines
            .iter()
            .flat_map(|l| p.line(&l.to_string(), false).events)
            .collect()
    }

    #[test]
    fn turn_arguments_name_the_model_and_conversation() {
        let new = Ollama.turn_args(&TurnRequest {
            session: ProviderSession::New {
                preassigned: Some("c-1".into()),
            },
            ..TurnRequest::default()
        });
        assert_eq!(new, ["chat", "--model", DEFAULT_MODEL, "--session", "c-1"]);
        let resume = Ollama.turn_args(&TurnRequest {
            session: ProviderSession::Resume { id: "c-1".into() },
            model: Some("glm-x:cloud".into()),
            effort: Some(Effort::High),
            ..TurnRequest::default()
        });
        assert_eq!(
            resume,
            [
                "chat",
                "--model",
                "glm-x:cloud",
                "--session",
                "c-1",
                "--resume",
                "--think",
                "high"
            ]
        );
        assert!(Ollama.preassigns_session_id() && Ollama.bridged() && !Ollama.accepts_tools());
    }

    #[test]
    fn known_models_are_valid_names() {
        let caps = Ollama.capabilities();
        assert_eq!(caps.known_models.len(), 8);
        assert_eq!(caps.effort_levels_for(Some("kimi-k3:cloud")), TO_MAX);
        assert!(caps
            .effort_levels_for(Some("nemotron-3-ultra:cloud"))
            .is_empty());
        for m in &caps.known_models {
            assert_eq!(
                crate::agent::service::validate_model(&m.name).unwrap(),
                m.name
            );
        }
    }

    #[test]
    fn sign_in_check_reports_the_plan() {
        let s = parse_auth(&probe(r#"{"signedIn":true,"plan":"free"}"#));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("Ollama sign-in (free plan)"));
        assert_eq!(
            parse_auth(&probe(r#"{"signedIn":false}"#)).state,
            AuthState::SignedOut
        );
        let s = parse_auth(&probe(r#"{"error":"Ollama is not running on this PC"}"#));
        assert_eq!(s.state, AuthState::Unknown);
        assert!(s.detail.unwrap().contains("not running"));
        assert_eq!(parse_auth(&probe("nonsense")).state, AuthState::Unknown);
    }

    #[test]
    fn environment_passes_nothing_through() {
        let host = HostEnv::default().with_vars(vec![
            ("OLLAMA_API_KEY".into(), "k".into()),
            ("OLLAMA_HOST".into(), "0.0.0.0".into()),
            ("HTTPS_PROXY".into(), "http://x".into()),
        ]);
        assert!(crate::agent::discovery::runtime_env(&Ollama, &host).is_empty());
    }

    #[test]
    fn a_streamed_answer_is_normalized() {
        let mut p = Ollama.parser(&TurnRequest::default());
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"session","id":"c-1","model":DEFAULT_MODEL}),
                json!({"type":"thinking","text":"Say hello."}),
                json!({"type":"text","text":"hello "}),
                json!({"type":"text","text":"there"}),
                json!({"type":"answer","text":"hello there"}),
                json!({"type":"done","reason":"stop","inputTokens":75,"cachedTokens":48,"outputTokens":55,"durationMs":1200}),
            ],
        );
        let usage = TokenUsage {
            input_tokens: 75,
            cached_input_tokens: 48,
            output_tokens: 55,
        };
        assert_eq!(
            events,
            [
                AgentEvent::SessionStarted {
                    provider_session_id: Some("c-1".into()),
                    model: Some(DEFAULT_MODEL.into())
                },
                AgentEvent::Reasoning {
                    text: "Say hello.".into()
                },
                AgentEvent::TextDelta {
                    text: "hello ".into()
                },
                AgentEvent::TextDelta {
                    text: "there".into()
                },
                AgentEvent::Message {
                    text: "hello there".into()
                },
                AgentEvent::Usage { usage },
            ]
        );
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("hello there"));
        assert_eq!(r.provider_session_id.as_deref(), Some("c-1"));
        assert_eq!(r.duration_ms, Some(1200));
    }

    /// ADR-044 §2.5: when the bridge leaves earlier messages out, the model's memory of the
    /// conversation is shorter; other notices stay notices.
    #[test]
    fn messages_left_out_mean_a_shortened_memory() {
        let mut p = Ollama.parser(&TurnRequest::default());
        let text = "3 earlier message(s) were left out: the conversation is longer than Plenipo \
                    sends at once.";
        let events = feed(
            p.as_mut(),
            &[
                json!({"type":"notice","leftOut":3,"text":text}),
                json!({"type":"notice","text":"The conversation could not be saved."}),
            ],
        );
        assert_eq!(
            events,
            [
                AgentEvent::MemoryShortened {
                    detail: text.into()
                },
                AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    text: "The conversation could not be saved.".into()
                },
            ]
        );
    }

    #[test]
    fn errors_are_classified() {
        for (message, outcome) in [
            (
                "429 usage limit: hourly limit reached",
                TurnOutcome::UsageLimited,
            ),
            (
                "401 unauthorized: sign in to Ollama (ollama signin).",
                TurnOutcome::AuthRequired,
            ),
            (
                "Ollama is not running on this PC (connection refused: x). Start Ollama.",
                TurnOutcome::ProviderUnavailable,
            ),
            ("Ollama answered 404: model not found", TurnOutcome::Failed),
            (
                "This model needs a paid Ollama plan (402 payment required). Choose a model \
                 your plan includes, or change your plan at ollama.com.",
                TurnOutcome::Failed,
            ),
        ] {
            let mut p = Ollama.parser(&TurnRequest::default());
            feed(p.as_mut(), &[json!({"type":"error","message":message})]);
            let r = p.finish(&end(ExecutionState::Failed, Some(1)));
            assert_eq!(r.outcome, outcome, "{message}");
        }
    }

    #[test]
    fn the_bridge_output_is_what_the_parser_reads() {
        // A real bridge run, against a stand-in service, read by the parser.
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{BufRead, Write};
            let (mut s, _) = listener.accept().unwrap();
            let mut r = std::io::BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).is_ok() && line != "\r\n" {
                line.clear();
            }
            let answer = r#"{"id":"x","email":"a@b.c","plan":"pro"}"#;
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{answer}",
                answer.len()
            )
            .unwrap();
        });
        let mut out = Vec::new();
        let args = ["auth".to_owned(), "--port".into(), port.to_string()];
        assert_eq!(bridge::run(&args, &mut &b""[..], &mut out), 0);
        let s = parse_auth(&probe(&String::from_utf8(out).unwrap()));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("Ollama sign-in (pro plan)"));
    }
}
