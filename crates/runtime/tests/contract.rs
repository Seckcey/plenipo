//! The contract every AI tool's adapter meets (ADR-014; walk-through in
//! `docs/development/adding-an-ai-tool.md`). Every test runs for each adapter in
//! `builtin_adapters()`, so registering a new AI tool there puts it under this suite. The last
//! tests start the tool's persona of `plenipo-fake-agent`. No network, no accounts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_runtime::agent::adapter::{find_version, ProcessEnd};
use plenipo_runtime::agent::service::validate_model;
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentEvent, AgentRuntime, AgentSink, AgentTurn, AgentUpdate,
    AuthState, Effort, HostEnv, InstallState, MemorySessionStore, ProviderSession, RuntimeAdapter,
    TurnOutcome, TurnRequest,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ExecutionState, MetadataStore, ProfileRegistry, RuntimeEvent,
    Supervisor, SupervisorConfig,
};

const SESSION_ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

// ---- Identity ---------------------------------------------------------------------------------

#[test]
fn every_ai_tool_is_named_and_distinct() {
    let adapters = builtin_adapters();
    assert!(!adapters.is_empty());
    let mut companies = BTreeMap::new();
    for a in &adapters {
        let id = a.id();
        for (what, value) in [
            ("label", a.label()),
            ("company ID", a.provider()),
            ("company name", a.provider_label()),
            ("executable name", a.executable_name()),
            ("install hint", a.install_hint()),
            ("sign-in hint", a.login_hint()),
        ] {
            assert!(!value.trim().is_empty(), "{id}: empty {what}");
        }
        // The ID is also a handoff address (`codex`, `role:…`) and part of a program's name.
        assert!(
            id.starts_with(|c: char| c.is_ascii_lowercase())
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{id:?}: use lowercase letters, digits, and dashes"
        );
        // One name per AI company.
        let name = companies.entry(a.provider()).or_insert(a.provider_label());
        assert_eq!(*name, a.provider_label(), "{id}: company {}", a.provider());
    }
    for (what, values) in [
        ("ID", adapters.iter().map(|a| a.id()).collect::<Vec<_>>()),
        ("label", adapters.iter().map(|a| a.label()).collect()),
        (
            "executable",
            adapters.iter().map(|a| a.executable_name()).collect(),
        ),
        ("company name", companies.values().copied().collect()),
    ] {
        let distinct: BTreeSet<_> = values.iter().collect();
        assert_eq!(distinct.len(), values.len(), "repeated {what}: {values:?}");
    }
}

// ---- Models and effort (the model capability discovery contract) ----------------------------

/// Lowest first, no repeats.
fn ascending(levels: &[Effort]) -> bool {
    levels.windows(2).all(|w| w[0] < w[1])
}

#[test]
fn known_models_are_valid_distinct_and_within_the_tools_effort_levels() {
    for a in builtin_adapters() {
        let id = a.id();
        let caps = a.capabilities();
        let checked = a.checked_version();
        assert_eq!(
            find_version(checked).as_deref(),
            Some(checked),
            "{id}: checked_version must be the CLI version, e.g. 2.1.283"
        );
        assert!(
            ascending(&caps.effort_levels),
            "{id}: effort levels lowest first, no repeats"
        );
        let mut names = BTreeSet::new();
        for m in &caps.known_models {
            assert_eq!(validate_model(&m.name).ok().as_ref(), Some(&m.name), "{id}");
            assert!(names.insert(&m.name), "{id}: {} listed twice", m.name);
            assert!(!m.label.trim().is_empty(), "{id}: {} has no label", m.name);
            assert!(ascending(&m.effort_levels), "{id}: {}", m.name);
            for e in &m.effort_levels {
                assert!(
                    caps.effort_levels.contains(e),
                    "{id}: {} takes {e:?}, which the AI tool does not list",
                    m.name
                );
            }
        }
    }
}

// ---- Launch: arguments and environment --------------------------------------------------------

/// Every shape of turn: new (with and without an ID Plenipo chose) and resumed, with the
/// default model and each known model, at the default effort and each level it takes.
fn requests(a: &dyn RuntimeAdapter) -> Vec<TurnRequest> {
    let caps = a.capabilities();
    let sessions = [
        ProviderSession::New { preassigned: None },
        ProviderSession::New {
            preassigned: a.preassigns_session_id().then(|| SESSION_ID.to_owned()),
        },
        ProviderSession::Resume {
            id: SESSION_ID.into(),
        },
    ];
    let models =
        std::iter::once(None).chain(caps.known_models.iter().map(|m| Some(m.name.clone())));
    let mut out = Vec::new();
    for model in models {
        let efforts = caps.effort_levels_for(model.as_deref()).iter().copied();
        for effort in std::iter::once(None).chain(efforts.map(Some)) {
            for session in &sessions {
                out.push(TurnRequest {
                    session: session.clone(),
                    model: model.clone(),
                    effort,
                    ..TurnRequest::default()
                });
            }
        }
    }
    out
}

/// `value` is an argument of its own, or an option's value (`--flag=value`, `key=value`).
fn passes(args: &[String], value: &str) -> bool {
    args.iter().any(|a| {
        a == value
            || a.strip_suffix(value)
                .is_some_and(|flag| flag.ends_with('='))
    })
}

/// An argument that would carry a credential.
fn secret_bearing(arg: &str) -> bool {
    let a = arg.to_ascii_lowercase().replace('_', "-");
    let flag = a.split('=').next().unwrap_or("");
    [
        "api-key",
        "apikey",
        "auth-token",
        "access-token",
        "oauth-token",
        "password",
        "secret",
        "credential",
        "bearer",
    ]
    .iter()
    .any(|w| a.contains(w))
        || ["--token", "--key"].contains(&flag)
}

/// The messages a task that talks (ADR-015) sends when the AI tool answers every request
/// the way a willing tool would; empty for a one-way task.
fn messages(a: &dyn RuntimeAdapter, request: &TurnRequest) -> Vec<String> {
    let mut parser = a.parser(request);
    let Some(mut pending) = parser.open("Contract prompt") else {
        return Vec::new();
    };
    let mut sent = Vec::new();
    for _ in 0..8 {
        let mut next = Vec::new();
        for line in pending.drain(..) {
            sent.push(line.clone());
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let (Some(id), Some(method)) = (message.get("id"), message["method"].as_str()) else {
                continue;
            };
            let result = match method {
                "initialize" => serde_json::json!({ "protocolVersion": 1, "agentCapabilities": {
                    "loadSession": true, "sessionCapabilities": { "resume": {} } } }),
                "session/new" => serde_json::json!({ "sessionId": "contract-session" }),
                _ => serde_json::json!({}),
            };
            let answer = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
            next.extend(parser.line(&answer.to_string(), false).send);
        }
        pending = next;
    }
    sent
}

#[test]
fn turn_arguments_carry_the_choices_and_no_secrets() {
    for a in builtin_adapters() {
        let id = a.id();
        for request in requests(a.as_ref()) {
            let args = a.turn_args(&request);
            assert!(
                !args.iter().any(|arg| secret_bearing(arg)),
                "{id}: {args:?}"
            );
            let session = match &request.session {
                ProviderSession::Resume { id } => Some(id),
                ProviderSession::New { preassigned } => preassigned.as_ref(),
            };
            if let Some(session) = session {
                // In the arguments, or in the messages of a task that talks (ADR-015).
                let quoted = format!("\"{session}\"");
                assert!(
                    passes(&args, session)
                        || messages(a.as_ref(), &request)
                            .iter()
                            .any(|m| m.contains(&quoted)),
                    "{id}: session ID missing: {args:?}"
                );
            }
            if let Some(model) = &request.model {
                assert!(passes(&args, model), "{id}: model missing: {args:?}");
            }
            if let Some(effort) = request.effort {
                assert!(
                    passes(&args, effort.as_str()),
                    "{id}: effort missing: {args:?}"
                );
            }
        }
    }
}

#[test]
fn no_api_key_or_cloud_variables_are_passed() {
    for a in builtin_adapters() {
        let fixed = a.fixed_env();
        // A fixed switch Plenipo sets to turn key sign-in off (`GROK_DISABLE_API_KEY_AUTH=1`,
        // ADR-015 §6) carries no credential; everything else is checked by name.
        let switch_off = |(k, v): &&(String, String)| {
            k.contains("DISABLE") && ["1", "true"].contains(&v.as_str())
        };
        let names = a.passthrough_env().into_iter().chain(
            fixed
                .iter()
                .filter(|kv| !switch_off(kv))
                .map(|(k, _)| k.as_str()),
        );
        for name in names {
            let upper = name.to_ascii_uppercase();
            assert!(
                ![
                    "KEY",
                    "TOKEN",
                    "SECRET",
                    "PASSWORD",
                    "CREDENTIAL",
                    "BEDROCK",
                    "VERTEX",
                    "FOUNDRY",
                    "BASE_URL",
                ]
                .iter()
                .any(|w| upper.contains(w)),
                "{}: {name} could carry a credential or switch billing",
                a.id()
            );
        }
    }
}

// ---- Output: the parser and the normalized result ---------------------------------------------

fn end(state: ExecutionState, exit_code: Option<i32>, started: bool) -> ProcessEnd {
    ProcessEnd {
        state,
        exit_code,
        started,
        detail: None,
        duration_ms: Some(1),
    }
}

#[test]
fn parsers_ignore_malformed_lines_and_finish_with_one_result() {
    use ExecutionState::*;
    let ends = [
        (end(Succeeded, Some(0), true), None),
        (end(Failed, Some(1), true), None),
        (
            end(Failed, None, false),
            Some(TurnOutcome::ProviderUnavailable),
        ),
        (end(Cancelled, None, true), Some(TurnOutcome::Cancelled)),
        (end(TimedOut, None, true), Some(TurnOutcome::TimedOut)),
        (end(Interrupted, None, true), Some(TurnOutcome::Interrupted)),
    ];
    let junk = [
        "<html>502 Bad Gateway</html>",
        "{not json",
        "[1, 2]",
        r#"{"type":"plenipo.contract.unknown"}"#,
    ];
    for a in builtin_adapters() {
        let id = a.id();
        for fed in [&junk[..0], &junk[..]] {
            for (process, want) in &ends {
                let request = TurnRequest {
                    billing_confirmed: true,
                    ..TurnRequest::default()
                };
                let mut parser = a.parser(&request);
                for line in fed {
                    let parsed = parser.line(line, false);
                    assert!(parsed.stop.is_none(), "{id}: {line:?} stopped the turn");
                    assert!(
                        parsed
                            .events
                            .iter()
                            .all(|e| matches!(e, AgentEvent::Notice { .. })),
                        "{id}: {line:?} → {:?}",
                        parsed.events
                    );
                }
                parser.line(&"x".repeat(64), true);
                let result = parser.finish(process);
                let at = format!("{id}, {:?} after {} lines", process.state, fed.len());
                assert_ne!(result.outcome, TurnOutcome::Completed, "{at}");
                assert!(!result.summary.trim().is_empty(), "{at}");
                assert_eq!(result.ignored_lines as usize, fed.len() + 1, "{at}");
                if let Some(want) = want {
                    assert_eq!(result.outcome, *want, "{at}");
                }
            }
        }
    }
}

// ---- Against the fake CLI --------------------------------------------------------------------

const WAIT: Duration = Duration::from_secs(30);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plenipo-fake-agent"))
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// Every AI tool the fake CLI stands in for (`plenipo-fake-agent --personas`).
fn personas() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = std::process::Command::new(fake_exe())
            .arg("--personas")
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .leak()
            .lines()
            .collect()
    })
}

/// One copy of each persona per test process, ready to execute; harnesses get hard links
/// (see `agents.rs` for why).
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("contract-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(fake_exe(), &path).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while let Err(e) = std::process::Command::new(&path).arg("--version").output() {
                assert!(Instant::now() < deadline, "fake CLI never runnable: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        dir
    })
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct NoUpdates;

impl AgentSink for NoUpdates {
    fn emit(&self, _: AgentUpdate) {}
}

/// Every AI tool, played by its persona, signed in as `auth` says.
struct Fakes {
    rt: AgentRuntime,
    dir: tempfile::TempDir,
}

impl Fakes {
    fn new(auth: &str) -> Self {
        let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        let bin = dir.path().join("bin");
        let home = dir.path().join("home");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(home.join(".plenipo-fake-agent")).unwrap();
        std::fs::write(home.join(".plenipo-fake-agent").join("auth"), auth).unwrap();
        for stem in personas() {
            let (source, target) = (fake_clis().join(exe_name(stem)), bin.join(exe_name(stem)));
            if std::fs::hard_link(&source, &target).is_err() {
                std::fs::copy(&source, &target).unwrap();
            }
        }
        let sup = Supervisor::new(
            SupervisorConfig::default(),
            ExecutablePolicy::default(),
            ProfileRegistry::default(),
            Arc::new(MetadataStore::in_memory()),
            Arc::new(NoOutput),
            vec![],
        );
        let mut config = AgentConfig::new(dir.path().join("workspaces"));
        config.extra_env = vec![(HOME_VAR.into(), home.display().to_string())];
        let rt = AgentRuntime::new(
            config,
            builtin_adapters(),
            sup,
            Arc::new(MemorySessionStore::default()),
            Arc::new(NoUpdates),
            HostEnv::new(Some(bin.into_os_string()), Some(home), None),
        );
        Self { rt, dir }
    }

    /// Run one task on `runtime` to its end; the task and the arguments the CLI received.
    async fn run(&self, runtime: &str, objective: &str) -> (AgentTurn, Vec<String>) {
        let started = self.rt.start_session(runtime, objective, None).await;
        let id = started.unwrap().session.id;
        let deadline = Instant::now() + WAIT;
        loop {
            let detail = self.rt.session(&id).await.unwrap();
            if detail.turns.iter().all(|t| t.result.is_some())
                && detail.session.active_task_id.is_none()
            {
                let state = self.dir.path().join("home").join(".plenipo-fake-agent");
                let args = std::fs::read_to_string(state.join("last-args.json")).unwrap();
                return (
                    detail.turns[0].clone(),
                    serde_json::from_str(&args).unwrap(),
                );
            }
            assert!(Instant::now() < deadline, "{runtime}: {detail:#?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[tokio::test]
async fn sign_in_check_tells_a_subscription_from_an_api_key() {
    for a in builtin_adapters() {
        assert!(
            personas().contains(&a.executable_name()),
            "{}: add a `{}` persona to plenipo-fake-agent",
            a.id(),
            a.executable_name()
        );
    }
    for (auth, state, ready) in [
        ("subscription", AuthState::Subscription, true),
        ("api-key", AuthState::ApiKey, false),
        ("signed-out", AuthState::SignedOut, false),
    ] {
        let fakes = Fakes::new(auth);
        for info in fakes.rt.refresh().await {
            let at = format!("{} ({auth})", info.id);
            assert_eq!(info.installation.state, InstallState::Installed, "{at}");
            assert!(info.installation.version.is_some(), "{at}");
            assert_eq!((info.auth.state, info.ready), (state, ready), "{at}");
        }
    }
}

#[tokio::test]
async fn prompts_go_on_stdin_and_limits_and_sign_in_errors_are_normalized() {
    let fakes = Fakes::new("subscription");
    fakes.rt.refresh().await;
    for a in builtin_adapters() {
        let id = a.id();
        let (turn, args) = fakes.run(id, "Contract check 7c1e").await;
        let result = turn.result.unwrap();
        assert_eq!(result.outcome, TurnOutcome::Completed, "{id}: {result:#?}");
        assert!(
            result
                .text
                .unwrap_or_default()
                .contains("Contract check 7c1e"),
            "{id}: the prompt reached the CLI on stdin"
        );
        assert!(!args.iter().any(|a| a.contains("7c1e")), "{id}: {args:?}");
        for (marker, outcome) in [
            ("[usage-limit]", TurnOutcome::UsageLimited),
            ("[auth-expired]", TurnOutcome::AuthRequired),
        ] {
            let (turn, _) = fakes.run(id, marker).await;
            assert_eq!(turn.result.unwrap().outcome, outcome, "{id}: {marker}");
        }
    }
}
