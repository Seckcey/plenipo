//! Phase 3 runtime adapter tests: the real session service, supervisor, and adapters driving
//! `plenipo-fake-agent` installed as each AI tool (`claude`, `codex`, `grok`, `kimi`, and
//! `ollama`). Each plan test runs for every tool it applies to. No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentEvent, AgentRuntime, AgentSessionDetail, AgentSink,
    AgentTurn, AgentUpdate, AuthState, Bridge, BriefInput, Effort, HoldFor, HostEnv, InstallState,
    MemorySessionStore, SessionStart, SessionState, SessionStore, StepInfo, StepNote, StepTools,
    ToolProvider, TurnDisposition, TurnEnd, TurnHook, TurnInput, TurnOutcome, TurnRef, TurnTask,
    STEP_SEQ,
};
use plenipo_runtime::{
    BriefKind, BriefWhy, EventSink, ExecutablePolicy, ExecutionState, MetadataStore, NoteKind,
    ProfileRegistry, PromptSize, RuntimeError, RuntimeEvent, Supervisor, SupervisorConfig,
};

const RUNTIMES: [&str; 4] = ["claude-code", "codex", "grok", "kimi"];
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

/// Test scratch space on the same filesystem as the shared fake CLIs (for hard links).
fn scratch() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
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

/// One copy of the fake CLIs per test process, ready to execute.
///
/// Copying an executable while other tests fork is racy on Linux: a forked child can briefly
/// inherit the copy's write handle, and exec then fails with ETXTBSY. So copy once, wait until
/// the copies run, and give each harness hard links (which never open a write handle).
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(fake_exe(), &path).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while let Err(e) = std::process::Command::new(&path).arg("--version").output() {
                assert!(
                    Instant::now() < deadline,
                    "fake CLI never became runnable: {e}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        dir
    })
}

fn install_fake(bin: &Path, stem: &str) {
    let source = fake_clis().join(exe_name(stem));
    let target = bin.join(exe_name(stem));
    if std::fs::hard_link(&source, &target).is_err() {
        std::fs::copy(&source, &target).unwrap();
    }
}

#[derive(Default)]
struct Updates(Mutex<Vec<AgentUpdate>>);

impl AgentSink for Updates {
    fn emit(&self, update: AgentUpdate) {
        self.0.lock().unwrap().push(update);
    }
}

impl Updates {
    fn activity(&self, task_id: &str) -> Vec<AgentEvent> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|u| match u {
                AgentUpdate::Activity(a) if a.task_id == task_id => Some(a.event.clone()),
                _ => None,
            })
            .collect()
    }
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct H {
    rt: AgentRuntime,
    sup: Supervisor,
    store: Arc<MemorySessionStore>,
    updates: Arc<Updates>,
    dir: tempfile::TempDir,
}

impl H {
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    fn state(&self) -> PathBuf {
        self.dir.path().join("home").join(".plenipo-fake-agent")
    }

    fn set_auth(&self, mode: &str) {
        std::fs::create_dir_all(self.state()).unwrap();
        std::fs::write(self.state().join("auth"), mode).unwrap();
    }

    fn last_args(&self) -> Vec<String> {
        serde_json::from_str(&std::fs::read_to_string(self.state().join("last-args.json")).unwrap())
            .unwrap()
    }

    /// What Plenipo last sent to open an ACP session (ADR-015).
    fn last_acp(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.state().join("last-acp.json")).unwrap())
            .unwrap()
    }

    /// The provider session a follow-up resumed: from the arguments, or, for Grok and Kimi
    /// (ACP, ADR-015), from the message that opened the session.
    fn resumed(&self, runtime: &str) -> String {
        if matches!(runtime, "grok" | "kimi") {
            let acp = self.last_acp();
            // Grok resumes; Kimi loads, the way checked on the real CLI (ADR-027).
            let method = if runtime == "grok" {
                "session/resume"
            } else {
                "session/load"
            };
            assert_eq!(acp["method"], method, "{acp}");
            return acp["params"]["sessionId"].as_str().unwrap().to_owned();
        }
        let flag = if runtime == "claude-code" {
            "--resume"
        } else {
            "resume"
        };
        let args = self.last_args();
        let i = args
            .iter()
            .position(|a| a == flag)
            .expect("resume argument");
        args[i + 1].clone()
    }

    fn last_env(&self) -> Vec<String> {
        std::fs::read_to_string(self.state().join("last-env.txt"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn harness_with(installed: &[&str], auth: Option<&str>) -> H {
    harness_config(installed, auth, |_| {})
}

fn harness_config(
    installed: &[&str],
    auth: Option<&str>,
    tune: impl FnOnce(&mut AgentConfig),
) -> H {
    let dir = scratch();
    let bin = dir.path().join("bin");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    for stem in installed {
        install_fake(&bin, stem);
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
    config.turn_timeout = Duration::from_secs(120);
    // The `ollama` persona also plays Plenipo's Ollama bridge (ADR-017).
    config.bridge = Some(Bridge {
        executable: bin.join(exe_name("ollama")),
        args: vec!["--plenipo-ollama".into()],
    });
    tune(&mut config);
    let store = Arc::new(MemorySessionStore::default());
    let updates = Arc::new(Updates::default());
    let host = HostEnv::new(Some(bin.clone().into_os_string()), Some(home), None);
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup.clone(),
        store.clone(),
        updates.clone(),
        host,
    );
    // The `ollama` persona plays Plenipo's paid helper too (ADR-085), with a test key.
    rt.set_paid_gate(Arc::new(
        plenipo_runtime::agent::paid::MemoryPaidGate::with_key_for(&["openrouter"]),
    ));
    let h = H {
        rt,
        sup,
        store,
        updates,
        dir,
    };
    if let Some(mode) = auth {
        h.set_auth(mode);
    }
    h
}

fn harness() -> H {
    harness_with(personas(), None)
}

/// Wait until the session has `turns` turns, none is running, and the runtime has released the
/// session. A turn reads as finished as soon as its result is recorded, a moment before its slot
/// is released; a follow-up sent in that moment is refused as "already running".
async fn settled(rt: &AgentRuntime, session_id: &str, turns: usize) -> AgentSessionDetail {
    let deadline = Instant::now() + WAIT;
    loop {
        let detail = rt.session(session_id).await.unwrap();
        if detail.turns.len() >= turns
            && detail.turns.iter().all(|t| !t.running)
            && detail.session.active_task_id.is_none()
        {
            return detail;
        }
        assert!(
            Instant::now() < deadline,
            "turns did not finish: {detail:#?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn run(h: &H, runtime: &str, objective: &str) -> (AgentSessionDetail, AgentTurn) {
    let started = h.rt.start_session(runtime, objective, None).await.unwrap();
    let detail = settled(&h.rt, &started.session.id, 1).await;
    let turn = detail.turns.last().unwrap().clone();
    (detail, turn)
}

fn outcome(turn: &AgentTurn) -> TurnOutcome {
    turn.result
        .as_ref()
        .expect("finished turn has a result")
        .outcome
}

// ---- Installation and sign-in ---------------------------------------------------------------

#[tokio::test]
async fn installation_detection() {
    let h = harness();
    let runtimes = h.rt.refresh().await;
    // Seven AI tools with subscriptions, OpenRouter, and ten AI companies' own services with a
    // key (ADR-087).
    assert_eq!(runtimes.len(), 18);
    // Each company's own service comes with Plenipo too; without its key it is not ready.
    let (runtimes, direct) = runtimes.split_at(8);
    for info in direct {
        assert!(info.id.ends_with("-key"), "{}", info.id);
        assert_eq!(
            info.installation.state,
            InstallState::Installed,
            "{info:#?}"
        );
        assert_eq!(info.auth.state, AuthState::SignedOut, "{info:#?}");
        assert!(!info.ready);
    }
    // OpenRouter (ADR-086) is Plenipo's own helper: its version is Plenipo's, checked below.
    let (runtimes, paid) = runtimes.split_at(7);
    assert_eq!(paid[0].id, "openrouter");
    assert_eq!(paid[0].installation.state, InstallState::Installed);
    assert_eq!(
        paid[0].installation.version.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(paid[0].auth.state, AuthState::PaidKey, "{:#?}", paid[0]);
    assert!(paid[0].ready);
    for (info, version) in runtimes.iter().zip([
        "2.1.999", "0.99.0", "1.0.99", "0.34.99", "0.34.4", "1.2.99", "1.0.99",
    ]) {
        assert_eq!(
            info.installation.state,
            InstallState::Installed,
            "{info:#?}"
        );
        assert_eq!(info.installation.version.as_deref(), Some(version));
        let exe = info.installation.executable.as_deref().unwrap();
        assert!(
            Path::new(exe).starts_with(dunce::canonicalize(h.bin()).unwrap()),
            "{exe}"
        );
        assert_eq!(info.auth.state, AuthState::Subscription, "{info:#?}");
        assert!(info.ready);
        assert!(info.checked_at.is_some());
    }
    assert_eq!(
        runtimes[0].auth.method.as_deref(),
        Some("Claude subscription (max)")
    );
    assert_eq!(runtimes[1].auth.method.as_deref(), Some("ChatGPT sign-in"));
    assert_eq!(runtimes[2].auth.method.as_deref(), Some("grok.com sign-in"));
    assert_eq!(runtimes[3].auth.method.as_deref(), Some("Kimi sign-in"));
    assert_eq!(
        runtimes[4].auth.method.as_deref(),
        Some("Ollama sign-in (free plan)")
    );
    assert_eq!(runtimes[5].auth.method.as_deref(), Some("Google sign-in"));
    assert_eq!(runtimes[6].auth.method.as_deref(), Some("Copilot sign-in"));
    // No account identifier from the status output is kept.
    assert!(!format!("{runtimes:?}").contains("owner@example.com"));
    assert!(!format!("{runtimes:?}").contains("octo-owner"));
    // The UI was told.
    assert!(h
        .updates
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|u| matches!(u, AgentUpdate::Runtimes(_))));
}

#[tokio::test]
async fn ollama_runs_through_the_bridge_and_keeps_the_conversation() {
    let h = harness();
    h.rt.refresh().await;
    let (detail, turn) = run(&h, "ollama", "Hello Ollama 5b2d").await;
    assert_eq!(outcome(&turn), TurnOutcome::Completed, "{turn:#?}");
    let args = h.last_args();
    assert_eq!(args[..3], ["chat", "--model", "gpt-oss:120b-cloud"]);
    assert!(!args.iter().any(|a| a.contains("5b2d")), "{args:?}");
    h.rt.resume_session(&detail.session.id, "And again")
        .await
        .unwrap();
    let detail = settled(&h.rt, &detail.session.id, 2).await;
    let text = detail.turns[1].result.as_ref().unwrap().text.clone();
    assert!(
        text.unwrap_or_default().starts_with("Turn 2: "),
        "{detail:#?}"
    );
    let args = h.last_args();
    assert!(args.contains(&"--resume".to_owned()), "{args:?}");
}

#[tokio::test]
async fn ollama_without_plenipos_bridge_is_not_usable() {
    let h = harness_config(personas(), None, |c| c.bridge = None);
    let info =
        h.rt.refresh()
            .await
            .into_iter()
            .find(|r| r.id == "ollama")
            .unwrap();
    assert_eq!(info.installation.state, InstallState::Broken, "{info:#?}");
    assert!(!info.ready);
    assert!(info
        .installation
        .detail
        .unwrap_or_default()
        .contains("helper is not set up"));
}

#[tokio::test]
async fn missing_runtimes_are_reported_not_installed() {
    let h = harness_with(&[], None);
    let before = h.rt.runtimes();
    assert!(before
        .iter()
        .all(|r| r.installation.state == InstallState::Checking));
    for info in h.rt.refresh().await {
        assert_eq!(
            info.installation.state,
            InstallState::NotInstalled,
            "{info:#?}"
        );
        assert!(!info.ready);
        assert!(!info.install_hint.is_empty());
    }
}

#[cfg(windows)]
#[tokio::test]
async fn windows_npm_shims_are_not_run() {
    let h = harness_with(&[], None);
    std::fs::write(h.bin().join("claude.cmd"), "@echo off\r\n").unwrap();
    let info = h.rt.refresh().await.remove(0);
    assert_eq!(info.installation.state, InstallState::Unsupported);
    assert!(info.installation.detail.unwrap().contains("claude.cmd"));
}

#[tokio::test]
async fn unauthenticated_runtimes_refuse_work_with_login_guidance() {
    for runtime in RUNTIMES {
        let h = harness_with(personas(), Some("signed-out"));
        let info =
            h.rt.refresh()
                .await
                .into_iter()
                .find(|r| r.id == runtime)
                .unwrap();
        assert_eq!(info.auth.state, AuthState::SignedOut, "{runtime}");
        assert!(!info.ready);
        let err =
            h.rt.start_session(runtime, "hello", None)
                .await
                .expect_err(runtime);
        assert!(matches!(err, RuntimeError::NotReady(_)), "{err}");
        assert!(err.to_string().contains("not signed in"), "{err}");
        assert!(
            err.to_string().contains("login"),
            "login command given: {err}"
        );
        assert!(err.is_caller_error());
        // Nothing ran and nothing was recorded.
        assert!(h.store.sessions(10).unwrap().is_empty());
        assert!(h.sup.overview().executions.is_empty());
    }
}

#[tokio::test]
async fn api_key_and_cloud_sign_ins_are_refused() {
    for (mode, runtime, state) in [
        ("api-key", "claude-code", AuthState::ApiKey),
        ("api-key", "codex", AuthState::ApiKey),
        // GitHub Copilot signed in with a token in a variable (ADR-083).
        ("api-key", "copilot", AuthState::ApiKey),
        ("cloud", "claude-code", AuthState::ThirdPartyCloud),
    ] {
        let h = harness_with(personas(), Some(mode));
        let info =
            h.rt.refresh()
                .await
                .into_iter()
                .find(|r| r.id == runtime)
                .unwrap();
        assert_eq!(info.auth.state, state, "{runtime} {mode}");
        assert!(!info.ready);
        assert!(
            !format!("{info:?}").contains("ABCDE"),
            "masked key never kept"
        );
        let err =
            h.rt.start_session(runtime, "hello", None)
                .await
                .unwrap_err();
        assert!(matches!(err, RuntimeError::NotReady(_)), "{err}");
        assert!(h.sup.overview().executions.is_empty(), "no API-billed run");
    }
}

#[tokio::test]
async fn unverifiable_sign_in_is_allowed_only_with_a_per_turn_billing_check() {
    let h = harness_with(personas(), Some("unknown-status"));
    let runtimes = h.rt.refresh().await;
    assert_eq!(runtimes[0].auth.state, AuthState::Unknown);
    assert!(
        runtimes[0].ready,
        "Claude Code re-checks billing in every turn"
    );
    assert_eq!(runtimes[1].auth.state, AuthState::Unknown);
    assert!(
        !runtimes[1].ready,
        "Codex must confirm a ChatGPT sign-in first"
    );
}

#[tokio::test]
async fn claude_code_turn_is_stopped_when_it_reports_api_billing() {
    let h = harness_with(&["claude"], Some("stream-api-key"));
    let started = Instant::now();
    let (_, turn) = run(&h, "claude-code", "hello").await;
    assert_eq!(outcome(&turn), TurnOutcome::BillingNotAllowed);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "stopped promptly, not after the fake's 60 s wait"
    );
    let result = turn.result.unwrap();
    assert!(
        result.summary.contains("ANTHROPIC_API_KEY"),
        "{}",
        result.summary
    );
    let exec = h.sup.record(turn.execution_id.as_deref().unwrap()).unwrap();
    assert_eq!(exec.state, ExecutionState::Cancelled);
    assert_eq!(exec.detail.as_deref(), Some("Stopped by Plenipo policy"));
}

#[tokio::test]
async fn unconfirmed_sign_in_without_a_reported_credential_source_is_stopped() {
    // `auth status` gives nothing usable and the stream does not say which credential it uses:
    // nothing confirms a subscription, so the turn must not run (ADR-007 §4).
    let h = harness_with(&["claude"], Some("unknown-status,no-key-source"));
    let (_, turn) = run(&h, "claude-code", "hello").await;
    assert_eq!(outcome(&turn), TurnOutcome::BillingNotAllowed);
    assert!(turn.result.unwrap().summary.contains("did not report"));

    // A subscription confirmed by `auth status` does not depend on the stream reporting it.
    let h = harness_with(&["claude"], Some("no-key-source"));
    let (_, turn) = run(&h, "claude-code", "hello").await;
    assert_eq!(outcome(&turn), TurnOutcome::Completed);
}

// ---- Turns ----------------------------------------------------------------------------------

#[tokio::test]
async fn new_session_streams_activity_and_returns_a_normalized_result() {
    for runtime in RUNTIMES {
        let h = harness();
        let (detail, turn) = run(&h, runtime, "hello there").await;
        let result = turn.result.clone().unwrap();
        assert_eq!(
            result.outcome,
            TurnOutcome::Completed,
            "{runtime}: {result:#?}"
        );
        assert_eq!(
            result.text.as_deref(),
            Some("Turn 1: you said \"hello there\". Previous: None.")
        );
        // Kimi 0.34.0 reports no token counts over ACP (only its context size).
        let counts_tokens = runtime != "kimi";
        assert_eq!(
            result.usage.is_some_and(|u| u.output_tokens > 0),
            counts_tokens,
            "{runtime}"
        );
        assert_eq!(turn.number, 1);
        assert_eq!(turn.objective, "hello there");

        // Session: provider session confirmed and preserved.
        let session = &detail.session;
        assert_eq!(session.runtime_id, runtime);
        assert_eq!(session.state, SessionState::Open);
        assert!(session.provider_session_confirmed);
        let provider_id = session.provider_session_id.clone().unwrap();
        assert_eq!(
            result.provider_session_id.as_deref(),
            Some(provider_id.as_str())
        );
        assert_eq!(session.turn_count, 1);
        assert!(session.active_task_id.is_none());

        // Streamed activity: live events arrived, and the durable subset was recorded.
        let live = h.updates.activity(&turn.task_id);
        assert!(
            matches!(live[0], AgentEvent::SessionStarted { .. }),
            "{live:?}"
        );
        assert!(live.iter().any(|e| matches!(e, AgentEvent::Message { .. })));
        let deltas = live
            .iter()
            .filter(|e| matches!(e, AgentEvent::TextDelta { .. }))
            .count();
        match runtime {
            "claude-code" => assert!(deltas > 1, "text streams incrementally ({deltas})"),
            "codex" => assert!(live.iter().any(|e| matches!(e, AgentEvent::ToolUse { .. }))),
            _ => assert!(deltas > 0, "text streams ({deltas})"),
        }
        let stored = h.store.activity(&turn.task_id);
        assert!(stored
            .iter()
            .any(|e| matches!(e, AgentEvent::Message { .. })));
        assert!(stored
            .iter()
            .all(|e| !matches!(e, AgentEvent::TextDelta { .. } | AgentEvent::Usage { .. })));
        assert_eq!(
            detail.activity.len(),
            h.rt.session(&session.id).await.unwrap().activity.len()
        );

        // Execution: supervised, attributed, and the prompt never in argv.
        let exec = h.sup.record(turn.execution_id.as_deref().unwrap()).unwrap();
        assert_eq!(exec.state, ExecutionState::Succeeded);
        assert_eq!(exec.profile_id, format!("agent.{runtime}"));
        let agent = exec.agent.unwrap();
        assert_eq!(agent.runtime_id, runtime);
        assert_eq!(agent.task_id, turn.task_id);
        assert_eq!(agent.session_id, session.id);
        assert_eq!(
            agent.provider_session_id.as_deref(),
            Some(provider_id.as_str())
        );
        assert_eq!(agent.usage.is_some(), counts_tokens, "{runtime}");
        let args = h.last_args();
        assert!(!args.iter().any(|a| a.contains("hello")), "{args:?}");
        assert_eq!(exec.args, args);
        // The working directory is the session's own workspace.
        assert!(Path::new(&exec.working_dir).ends_with(&session.id));
        // No credentials reached the CLI.
        let env = h.last_env();
        for secret in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "CODEX_API_KEY",
            "XAI_API_KEY",
            "GROK_CODE_XAI_API_KEY",
        ] {
            assert!(!env.iter().any(|n| n == secret), "{secret}");
        }
        if runtime == "grok" {
            // API-key sign-in refused by Grok itself; Plenipo never asked it to sign in.
            assert!(env.iter().any(|n| n == "GROK_DISABLE_API_KEY_AUTH"));
            assert!(env.iter().any(|n| n == "GROK_DISABLE_AUTOUPDATER"));
            // No web or X search on xAI's side.
            assert!(env.iter().any(|n| n == "GROK_BACKEND_SEARCH"));
            assert!(!h.state().join("authenticate-called").exists());
            assert_eq!(args.last().map(String::as_str), Some("stdio"));
            assert!(args.contains(&"--no-leader".to_owned()), "{args:?}");
        }
        if runtime == "claude-code" {
            assert!(env.iter().any(|n| n == "DISABLE_AUTOUPDATER"));
            let i = args.iter().position(|a| a == "--session-id").unwrap();
            assert_eq!(args[i + 1], provider_id, "Plenipo chose the session ID");
        }
    }
}

#[tokio::test]
async fn resume_continues_the_same_provider_session() {
    for runtime in RUNTIMES {
        let h = harness();
        let (first, _) = run(&h, runtime, "remember this").await;
        let id = first.session.id.clone();
        let provider_id = first.session.provider_session_id.clone().unwrap();

        let resumed = h.rt.resume_session(&id, "and now?").await.unwrap();
        assert_eq!(resumed.turns.len(), 2);
        let detail = settled(&h.rt, &id, 2).await;
        let turn = &detail.turns[1];
        assert_eq!(turn.number, 2);
        let result = turn.result.clone().unwrap();
        assert_eq!(
            result.outcome,
            TurnOutcome::Completed,
            "{runtime}: {result:#?}"
        );
        assert_eq!(
            result.text.as_deref(),
            Some("Turn 2: you said \"and now?\". Previous: Some(\"remember this\").")
        );
        assert_eq!(
            detail.session.provider_session_id.as_deref(),
            Some(provider_id.as_str())
        );
        assert_eq!(detail.session.turn_count, 2);
        assert_eq!(h.resumed(runtime), provider_id);
        // Two executions, both in the same Plenipo session.
        let execs: Vec<_> = h
            .sup
            .overview()
            .executions
            .into_iter()
            .filter_map(|e| e.agent)
            .collect();
        assert_eq!(execs.len(), 2);
        assert!(execs.iter().all(|a| a.session_id == id));
    }
}

#[tokio::test]
async fn cancellation_stops_the_turn_and_the_session_stays_resumable() {
    for runtime in RUNTIMES {
        let h = harness();
        let started =
            h.rt.start_session(runtime, "work [slow]", None)
                .await
                .unwrap();
        let id = started.session.id.clone();
        let task = started.turns[0].task_id.clone();
        assert!(started.turns[0].running);
        assert_eq!(
            started.session.active_task_id.as_deref(),
            Some(task.as_str())
        );
        // Wait for live activity, then cancel.
        let deadline = Instant::now() + WAIT;
        while h.updates.activity(&task).len() < 3 {
            assert!(Instant::now() < deadline, "{runtime}: no live activity");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let detail = h.rt.cancel_turn(&id).await.unwrap();
        let turn = &detail.turns[0];
        assert!(!turn.running, "{runtime}: recorded before cancel returns");
        assert_eq!(outcome(turn), TurnOutcome::Cancelled);
        let exec = h.sup.record(turn.execution_id.as_deref().unwrap()).unwrap();
        // A tool that talks (Grok and Kimi, ADR-015) is asked to stop and ends by itself; the
        // others are ended by Plenipo.
        let ended = if matches!(runtime, "grok" | "kimi") {
            ExecutionState::Succeeded
        } else {
            ExecutionState::Cancelled
        };
        assert_eq!(exec.state, ended, "{runtime}");
        assert!(detail.session.active_task_id.is_none());

        h.rt.resume_session(&id, "after cancel").await.unwrap();
        let detail = settled(&h.rt, &id, 2).await;
        let result = detail.turns[1].result.clone().unwrap();
        assert_eq!(
            result.outcome,
            TurnOutcome::Completed,
            "{runtime}: {result:#?}"
        );
        assert!(
            result.text.unwrap().starts_with("Turn 2:"),
            "same provider session"
        );
    }
}

#[tokio::test]
async fn failures_are_normalized() {
    for runtime in RUNTIMES {
        for (marker, want) in [
            ("[crash]", TurnOutcome::Crashed),
            ("[usage-limit]", TurnOutcome::UsageLimited),
            ("[malformed]", TurnOutcome::MalformedOutput),
            ("[auth-expired]", TurnOutcome::AuthRequired),
            ("[offline]", TurnOutcome::ProviderUnavailable),
        ] {
            let h = harness();
            let (detail, turn) = run(&h, runtime, &format!("please {marker}")).await;
            let result = turn.result.clone().unwrap();
            assert_eq!(result.outcome, want, "{runtime} {marker}: {result:#?}");
            assert!(!result.summary.is_empty());
            // A failed turn never closes the session or switches provider.
            assert_eq!(detail.session.state, SessionState::Open);
            assert_eq!(detail.session.runtime_id, runtime);
            if marker == "[crash]" {
                assert!(
                    result.error.as_deref().unwrap_or("").contains("panicked")
                        || result
                            .error
                            .as_deref()
                            .unwrap_or("")
                            .contains("simulated crash")
                );
            }
        }
    }
}

#[tokio::test]
async fn usage_limited_session_can_be_resumed_later() {
    for runtime in RUNTIMES {
        let h = harness();
        let (detail, turn) = run(&h, runtime, "go [usage-limit]").await;
        assert_eq!(outcome(&turn), TurnOutcome::UsageLimited);
        h.rt.resume_session(&detail.session.id, "try again")
            .await
            .unwrap();
        let detail = settled(&h.rt, &detail.session.id, 2).await;
        assert_eq!(
            outcome(&detail.turns[1]),
            TurnOutcome::Completed,
            "{runtime}"
        );
    }
}

#[tokio::test]
async fn provider_unavailable_after_detection_is_refused() {
    for (runtime, stem) in [("claude-code", "claude"), ("codex", "codex")] {
        let h = harness();
        // Every AI tool but the companies' own services, which have no key here.
        assert!(h
            .rt
            .refresh()
            .await
            .iter()
            .filter(|r| !r.id.ends_with("-key"))
            .all(|r| r.ready));
        std::fs::remove_file(h.bin().join(exe_name(stem))).unwrap();
        let err =
            h.rt.start_session(runtime, "hello", None)
                .await
                .unwrap_err();
        assert!(matches!(err, RuntimeError::NotReady(_)), "{runtime}: {err}");
        let info =
            h.rt.runtimes()
                .into_iter()
                .find(|r| r.id == runtime)
                .unwrap();
        assert!(!info.ready);
        assert!(h.sup.overview().executions.is_empty());
    }
}

#[tokio::test]
async fn large_and_unknown_output_is_handled() {
    for runtime in RUNTIMES {
        let h = harness();
        let (_, turn) = run(&h, runtime, "[big]").await;
        let result = turn.result.unwrap();
        assert_eq!(
            result.outcome,
            TurnOutcome::Completed,
            "{runtime}: 1 MiB lines parse"
        );
        assert!(
            result.text.unwrap().len() <= 64 * 1024,
            "result text is capped"
        );

        let (_, turn) = run(&h, runtime, "[unknown]").await;
        let result = turn.result.unwrap();
        assert_eq!(result.outcome, TurnOutcome::Completed, "{runtime}");
        assert!(
            result.ignored_lines >= 1,
            "unknown events counted, not fatal"
        );
    }
}

// ---- Sessions -------------------------------------------------------------------------------

#[tokio::test]
async fn one_turn_per_session_and_closing() {
    let h = harness();
    let started =
        h.rt.start_session("claude-code", "long [slow]", None)
            .await
            .unwrap();
    let id = started.session.id.clone();
    let busy = h.rt.resume_session(&id, "second").await.unwrap_err();
    assert!(busy.to_string().contains("already running"), "{busy}");
    assert!(matches!(busy, RuntimeError::SessionBusy(_)), "{busy:?}");
    let close = h.rt.close_session(&id).await.unwrap_err();
    assert!(close.to_string().contains("Cancel it first"), "{close}");

    h.rt.cancel_turn(&id).await.unwrap();
    let closed = h.rt.close_session(&id).await.unwrap();
    assert_eq!(closed.state, SessionState::Closed);
    let err = h.rt.resume_session(&id, "more").await.unwrap_err();
    assert!(err.to_string().contains("closed"), "{err}");
    assert!(h.rt.cancel_turn(&id).await.is_err());
    // Closing twice is harmless.
    assert_eq!(
        h.rt.close_session(&id).await.unwrap().state,
        SessionState::Closed
    );
    let overview = h.rt.overview().await.unwrap();
    assert_eq!(overview.sessions.len(), 1);
    assert_eq!(overview.sessions[0].state, SessionState::Closed);
}

/// A member of the organization keeps one conversation for all its tasks, so stopping one
/// task must never stop another (Phase 8).
#[tokio::test]
async fn cancel_task_stops_only_the_named_turn() {
    let h = harness();
    let started =
        h.rt.start_session("claude-code", "long [slow]", None)
            .await
            .unwrap();
    let id = started.session.id.clone();
    let deadline = Instant::now() + WAIT;
    let task = loop {
        let detail = h.rt.session(&id).await.unwrap();
        if let Some(t) = detail.session.active_task_id.clone() {
            break t;
        }
        assert!(Instant::now() < deadline, "the turn never started");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let other =
        h.rt.cancel_task(&id, "7c9e6679-7425-40de-944b-e07fc1f90ae7")
            .await
            .unwrap_err();
    assert!(other.to_string().contains("is not the turn"), "{other}");
    assert!(h.rt.session(&id).await.unwrap().turns[0].running);
    h.rt.cancel_task(&id, &task).await.unwrap();
    let detail = settled(&h.rt, &id, 1).await;
    assert_eq!(outcome(&detail.turns[0]), TurnOutcome::Cancelled);
    assert!(
        h.rt.cancel_task(&id, &task).await.is_err(),
        "nothing left to stop"
    );
}

#[tokio::test]
async fn close_and_follow_up_never_interleave() {
    for _ in 0..10 {
        let h = harness();
        let (detail, _) = run(&h, "codex", "first").await;
        let id = detail.session.id.clone();
        let (closed, resumed) =
            tokio::join!(h.rt.close_session(&id), h.rt.resume_session(&id, "second"));
        match (closed, resumed) {
            // The close won: the follow-up was refused and nothing ran.
            (Ok(s), Err(e)) => {
                assert_eq!(s.state, SessionState::Closed);
                assert!(matches!(e, RuntimeError::NotReady(_)), "{e}");
                assert_eq!(h.store.turns(&id).unwrap().len(), 1);
            }
            // The follow-up won: the close was refused while it runs.
            (Err(e), Ok(_)) => {
                assert!(e.to_string().contains("Cancel it first"), "{e}");
                let detail = settled(&h.rt, &id, 2).await;
                assert_eq!(detail.session.state, SessionState::Open);
            }
            other => panic!("exactly one must win: {other:?}"),
        }
    }
}

#[tokio::test]
async fn invalid_requests_are_rejected() {
    let h = harness();
    for (runtime, objective, model) in [
        ("claude-code", "   ", None),
        (
            "claude-code",
            "hello",
            Some("--dangerously-skip-permissions"),
        ),
        ("claude-code", "hello", Some("../../bin/sh")),
        ("gemini", "hello", None),
    ] {
        let err =
            h.rt.start_session(runtime, objective, model)
                .await
                .unwrap_err();
        assert!(matches!(err, RuntimeError::InvalidInput(_)), "{err}");
    }
    let unknown = "00000000-0000-4000-8000-000000000000";
    assert!(matches!(
        h.rt.resume_session(unknown, "hi").await,
        Err(RuntimeError::UnknownSession(_))
    ));
    assert!(h.rt.session(unknown).await.is_err());
    assert!(h.rt.close_session(unknown).await.is_err());
    assert!(h.sup.overview().executions.is_empty());
}

#[tokio::test]
async fn model_choice_is_passed_and_recorded() {
    let h = harness();
    let started =
        h.rt.start_session("claude-code", "hello", Some("opus[1m]"))
            .await
            .unwrap();
    let detail = settled(&h.rt, &started.session.id, 1).await;
    let args = h.last_args();
    let i = args.iter().position(|a| a == "--model").unwrap();
    assert_eq!(args[i + 1], "opus[1m]");
    // The provider-reported model replaces the requested one.
    assert_eq!(detail.session.model.as_deref(), Some("opus[1m]"));
}

#[tokio::test]
async fn restart_marks_unfinished_turns_interrupted() {
    let dir = scratch();
    let store = Arc::new(MemorySessionStore::default());
    store.insert_turn(AgentTurn {
        execution_id: Some("e-1".into()),
        running: true,
        ..unstarted("t-1", "s-1", "left running")
    });
    store.insert_turn(AgentTurn {
        waiting: true,
        ..unstarted("t-2", "s-2", "left waiting")
    });
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(MetadataStore::in_memory()),
        Arc::new(NoOutput),
        vec![],
    );
    let rt = AgentRuntime::new(
        AgentConfig::new(dir.path().to_path_buf()),
        builtin_adapters(),
        sup,
        store.clone(),
        Arc::new(Updates::default()),
        HostEnv::new(None, None, None),
    );
    for session in ["s-1", "s-2"] {
        let turn = store.turns(session).unwrap().remove(0);
        assert!(!turn.running && !turn.waiting, "{session}");
        assert_eq!(outcome(&turn), TurnOutcome::Interrupted);
    }
    assert!(store.unfinished_turns().unwrap().is_empty());
    let overview = rt.overview().await.unwrap();
    assert!(
        overview.notices[0].contains("interrupted"),
        "{:?}",
        overview.notices
    );
}

#[tokio::test]
async fn shutdown_stops_running_turns_and_records_them() {
    let h = harness();
    let started =
        h.rt.start_session("codex", "long [slow]", None)
            .await
            .unwrap();
    let id = started.session.id.clone();
    let stopped = h.rt.shutdown(Duration::from_secs(10)).await;
    assert_eq!(stopped, 1);
    let turn = h.store.turns(&id).unwrap().remove(0);
    assert!(!turn.running, "recorded before shutdown returned");
    assert_eq!(outcome(&turn), TurnOutcome::Cancelled);
    assert!(matches!(
        h.rt.start_session("codex", "more", None).await,
        Err(RuntimeError::ShuttingDown)
    ));
}

// ---- Steps and waiting (extension points used by Liaison, ADR-008) ---------------------------

/// A turn recorded but not started (a task a later turn adopts, or one left from before).
fn unstarted(task_id: &str, session_id: &str, objective: &str) -> AgentTurn {
    AgentTurn {
        task_id: task_id.into(),
        session_id: session_id.into(),
        number: 1,
        objective: objective.into(),
        execution_id: None,
        running: false,
        waiting: false,
        result: None,
        steps: Vec::new(),
        started_at: 1,
        ended_at: None,
    }
}

/// Stands in for Liaison: keeps a turn waiting when its first answer mentions `[wait]`.
struct WaitHook {
    store: Arc<MemorySessionStore>,
    released: AtomicUsize,
}

impl TurnHook for WaitHook {
    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition {
        let wants = end.step == 1
            && end.result.outcome == TurnOutcome::Completed
            && end
                .result
                .text
                .as_deref()
                .is_some_and(|t| t.contains("[wait]"));
        if !wants {
            return TurnDisposition::Finish;
        }
        let turn = TurnRef {
            session_id: &end.session.id,
            task_id: &end.task_id,
            execution_id: end.execution_id.as_deref(),
            step: Some(end.step),
            actor: "test",
        };
        self.store.suspend(&turn, &end.result).unwrap();
        TurnDisposition::Suspended {
            reason: "waiting for replies".into(),
        }
    }

    fn released(&self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

/// A [`WaitHook`] that also reads the session right after recording the wait, while the
/// runtime still holds the turn's slot (the window a UI snapshot can land in).
struct SnapshotHook {
    wait: WaitHook,
    rt: OnceLock<AgentRuntime>,
    seen: Mutex<Option<AgentTurn>>,
}

impl TurnHook for SnapshotHook {
    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition {
        let disposition = self.wait.turn_ended(end);
        if disposition != TurnDisposition::Finish {
            let rt = self.rt.get().expect("runtime set").clone();
            let id = end.session.id.clone();
            let detail = tokio::runtime::Handle::current()
                .block_on(async move { rt.session(&id).await })
                .unwrap();
            *self.seen.lock().unwrap() = detail.turns.first().cloned();
        }
        disposition
    }
}

/// A [`WaitHook`] that cancels the turn from inside the hook: after its wait is recorded (the
/// turn already reads as waiting) and before the runtime moves its slot from running to waiting.
struct CancelHook {
    wait: WaitHook,
    rt: OnceLock<AgentRuntime>,
    cancel: Mutex<Option<tokio::task::JoinHandle<Result<AgentSessionDetail, RuntimeError>>>>,
}

impl TurnHook for CancelHook {
    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition {
        let disposition = self.wait.turn_ended(end);
        if disposition != TurnDisposition::Finish {
            let rt = self.rt.get().expect("runtime set").clone();
            let id = end.session.id.clone();
            let cancel =
                tokio::runtime::Handle::current().spawn(async move { rt.cancel_turn(&id).await });
            *self.cancel.lock().unwrap() = Some(cancel);
            // Let the cancel find the turn still holding its slot as running.
            std::thread::sleep(Duration::from_millis(200));
        }
        disposition
    }

    fn released(&self) {
        self.wait.released();
    }
}

fn with_hook(h: &H) -> Arc<WaitHook> {
    let hook = Arc::new(WaitHook {
        store: h.store.clone(),
        released: AtomicUsize::new(0),
    });
    h.rt.set_hook(hook.clone());
    hook
}

/// Wait until the session's turn `n` satisfies `predicate`.
async fn turn_where(
    rt: &AgentRuntime,
    session_id: &str,
    n: usize,
    predicate: impl Fn(&AgentTurn) -> bool,
) -> AgentSessionDetail {
    let deadline = Instant::now() + WAIT;
    loop {
        let detail = rt.session(session_id).await.unwrap();
        if detail.turns.get(n - 1).is_some_and(&predicate) {
            return detail;
        }
        assert!(
            Instant::now() < deadline,
            "turn {n} never matched: {detail:#?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Start a turn that waits, and wait until the runtime holds it as waiting. It reads as waiting as
/// soon as its wait is recorded, a moment before the runtime moves its slot there; a continuation
/// sent in that moment is told to try again.
async fn waiting_turn(h: &H, runtime: &str) -> (String, String) {
    let started =
        h.rt.start_session(runtime, "plan it [wait]", None)
            .await
            .unwrap();
    let id = started.session.id.clone();
    let deadline = Instant::now() + WAIT;
    loop {
        let detail = h.rt.session(&id).await.unwrap();
        if let Some(task) = detail.session.waiting_task_id.clone() {
            return (id, task);
        }
        assert!(
            Instant::now() < deadline,
            "the turn never waited: {detail:#?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_reads_as_waiting_as_soon_as_its_wait_is_recorded() {
    let h = harness();
    let hook = Arc::new(SnapshotHook {
        wait: WaitHook {
            store: h.store.clone(),
            released: AtomicUsize::new(0),
        },
        rt: OnceLock::new(),
        seen: Mutex::new(None),
    });
    assert!(hook.rt.set(h.rt.clone()).is_ok());
    h.rt.set_hook(hook.clone());
    let started =
        h.rt.start_session("codex", "plan it [wait]", None)
            .await
            .unwrap();
    turn_where(&h.rt, &started.session.id, 1, |t| t.waiting).await;
    // Read before the runtime released the step: the recorded wait already shows, never a
    // running turn whose only step has finished. This test can see the wait before the hook's
    // own read has finished, so wait for that read.
    let deadline = Instant::now() + WAIT;
    let seen = loop {
        if let Some(seen) = hook.seen.lock().unwrap().clone() {
            break seen;
        }
        assert!(Instant::now() < deadline, "no read from inside the hook");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(seen.waiting && !seen.running, "{seen:#?}");
    assert!(seen.steps.iter().all(|s| !s.running), "{seen:#?}");
}

#[tokio::test]
async fn a_continuing_task_waiting_for_its_ai_tool_is_not_counted_as_using_it() {
    let h = harness();
    let _hook = with_hook(&h);
    let (id, task) = waiting_turn(&h, "codex").await;
    // Codex is being updated: the next step waits, and does not count as using Codex (so the
    // owner's sign-in tab or another update is not refused because of it).
    let hold = h.rt.hold_if_free("codex", HoldFor::Update).unwrap();
    let rt = h.rt.clone();
    let (sid, tid) = (id.clone(), task.clone());
    let next = tokio::spawn(async move {
        rt.continue_turn(&sid, &tid, "here are the replies", StepNote::default())
            .await
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!next.is_finished(), "the step waits for the update");
    assert!(h.rt.tasks_using("codex").is_empty());
    drop(hold);
    next.await.unwrap().unwrap();
    let detail = turn_where(&h.rt, &id, 1, |t| t.result.is_some()).await;
    assert_eq!(
        detail.turns[0].result.as_ref().unwrap().outcome,
        TurnOutcome::Completed
    );
}

#[tokio::test]
async fn the_owner_can_stop_a_task_that_waits_for_its_ai_tool() {
    let h = harness();
    let _hook = with_hook(&h);
    let (id, task) = waiting_turn(&h, "codex").await;
    let hold = h.rt.hold_if_free("codex", HoldFor::Update).unwrap();
    let rt = h.rt.clone();
    let (sid, tid) = (id.clone(), task.clone());
    let next = tokio::spawn(async move {
        rt.continue_turn(&sid, &tid, "here are the replies", StepNote::default())
            .await
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!next.is_finished(), "the step waits for the update");
    // Stop works while it waits: the task ends as stopped, and never starts on Codex.
    tokio::time::timeout(Duration::from_secs(10), h.rt.cancel_task(&id, &task))
        .await
        .expect("stopping does not wait for the update")
        .unwrap();
    let detail = turn_where(&h.rt, &id, 1, |t| t.result.is_some()).await;
    assert_eq!(
        detail.turns[0].result.as_ref().unwrap().outcome,
        TurnOutcome::Cancelled
    );
    assert!(next.await.unwrap().is_err());
    assert!(h.rt.held("codex"), "the update goes on");
    drop(hold);
}

/// Stop all work (Phase 25, item 3.4; ADR-199): every turn running now stops, a new one waits —
/// and can still be stopped — and Allow again lets the one that waited start.
#[tokio::test]
async fn stop_all_work_stops_what_runs_and_holds_new_work_until_allowed_again() {
    let h = harness();
    let busy =
        h.rt.start_session("claude-code", "long [slow]", None)
            .await
            .unwrap();
    let busy_id = busy.session.id.clone();
    let deadline = Instant::now() + WAIT;
    while h
        .rt
        .session(&busy_id)
        .await
        .unwrap()
        .session
        .active_task_id
        .is_none()
    {
        assert!(Instant::now() < deadline, "the turn never started");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    h.rt.hold_all_work();
    assert_eq!(h.rt.stop_all_turns().await, 1);
    let stopped = settled(&h.rt, &busy_id, 1).await;
    assert_eq!(outcome(&stopped.turns[0]), TurnOutcome::Cancelled);
    assert!(h.rt.work_held());
    assert!(h
        .rt
        .runtimes()
        .iter()
        .all(|r| r.held == Some(HoldFor::StopAll)));

    // New work waits while it is held.
    let rt = h.rt.clone();
    let next = tokio::spawn(async move { rt.start_session("codex", "hello", None).await });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!next.is_finished(), "new work waits until Allow again");

    // Allow again: it starts and finishes.
    h.rt.allow_work();
    assert!(!h.rt.work_held());
    let started = tokio::time::timeout(WAIT, next)
        .await
        .expect("it starts once allowed")
        .unwrap()
        .unwrap();
    let done = settled(&h.rt, &started.session.id, 1).await;
    assert_eq!(outcome(&done.turns[0]), TurnOutcome::Completed);
    assert!(h.rt.runtimes().iter().all(|r| r.held.is_none()));
}

#[tokio::test]
async fn a_waiting_turn_continues_as_a_new_step_in_the_same_provider_session() {
    for runtime in RUNTIMES {
        let h = harness();
        let hook = with_hook(&h);
        let (id, task) = waiting_turn(&h, runtime).await;
        let detail = h.rt.session(&id).await.unwrap();
        let turn = &detail.turns[0];
        assert!(
            !turn.running && turn.result.is_none(),
            "{runtime}: {turn:#?}"
        );
        assert_eq!(turn.steps.len(), 1);
        assert_eq!(
            turn.steps[0].result.as_ref().unwrap().outcome,
            TurnOutcome::Completed
        );
        assert_eq!(
            detail.session.waiting_task_id.as_deref(),
            Some(task.as_str())
        );
        assert!(detail.session.active_task_id.is_none());
        let provider_id = detail.session.provider_session_id.clone().unwrap();

        // The session takes no other work while it waits.
        let busy =
            h.rt.resume_session(&id, "something else")
                .await
                .unwrap_err();
        assert!(busy.to_string().contains("waiting to continue"), "{busy}");
        assert!(h.rt.close_session(&id).await.is_err());

        let note = StepNote {
            reason: "replies arrived".into(),
            data: serde_json::json!({ "deliver": ["r-1"] }),
            passed_bytes: 0,
        };
        h.rt.continue_turn(&id, &task, "here are the replies", note.clone())
            .await
            .unwrap();
        let detail = turn_where(&h.rt, &id, 1, |t| t.result.is_some()).await;
        let turn = &detail.turns[0];
        let result = turn.result.clone().unwrap();
        assert_eq!(
            result.outcome,
            TurnOutcome::Completed,
            "{runtime}: {result:#?}"
        );
        assert_eq!(
            result.text.as_deref(),
            Some("Turn 2: you said \"here are the replies\". Previous: Some(\"plan it [wait]\").")
        );
        assert_eq!(turn.steps.len(), 2);
        assert_ne!(turn.steps[0].execution_id, turn.steps[1].execution_id);
        assert_eq!(turn.execution_id, turn.steps[1].execution_id);
        assert_eq!(detail.session.turn_count, 1, "a step is not a new turn");
        assert!(detail.session.waiting_task_id.is_none());
        assert_eq!(h.store.notes(&task), [note]);
        // Same provider session, resumed.
        assert_eq!(h.resumed(runtime), provider_id);
        // Step 2's live activity is numbered after step 1's.
        let seqs: Vec<u64> = h
            .updates
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|u| match u {
                AgentUpdate::Activity(a) if a.task_id == task => Some(a.seq),
                _ => None,
            })
            .collect();
        assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
        assert!(seqs.iter().any(|s| *s > STEP_SEQ) && seqs[0] < STEP_SEQ);
        let exec = h.sup.record(turn.execution_id.as_deref().unwrap()).unwrap();
        assert!(exec.label.ends_with("task 1 · step 2"), "{}", exec.label);
        // The hook hears of each release once the slot is freed, just after the result is recorded.
        let deadline = Instant::now() + WAIT;
        while hook.released.load(Ordering::SeqCst) < 2 {
            assert!(
                Instant::now() < deadline,
                "{runtime}: a release was never reported"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[tokio::test]
async fn cancelling_a_waiting_turn_ends_it_and_frees_the_session() {
    let h = harness();
    let hook = with_hook(&h);
    let (id, task) = waiting_turn(&h, "codex").await;
    let before = hook.released.load(Ordering::SeqCst);
    let detail = h.rt.cancel_turn(&id).await.unwrap();
    let turn = &detail.turns[0];
    assert!(!turn.waiting && !turn.running);
    let result = turn.result.clone().unwrap();
    assert_eq!(result.outcome, TurnOutcome::Cancelled);
    assert!(result.summary.contains("waiting"), "{}", result.summary);
    assert_eq!(turn.steps.len(), 1, "cancelling runs no step");
    assert!(detail.session.waiting_task_id.is_none());
    assert!(hook.released.load(Ordering::SeqCst) > before);
    // It cannot be continued any more, and the session takes new work.
    let err =
        h.rt.continue_turn(&id, &task, "late replies", StepNote::default())
            .await
            .unwrap_err();
    assert!(matches!(err, RuntimeError::NotWaiting(_)), "{err}");
    h.rt.resume_session(&id, "next").await.unwrap();
    settled(&h.rt, &id, 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_as_a_turn_starts_waiting_ends_the_wait() {
    let h = harness();
    let hook = Arc::new(CancelHook {
        wait: WaitHook {
            store: h.store.clone(),
            released: AtomicUsize::new(0),
        },
        rt: OnceLock::new(),
        cancel: Mutex::new(None),
    });
    assert!(hook.rt.set(h.rt.clone()).is_ok());
    h.rt.set_hook(hook.clone());
    let started =
        h.rt.start_session("codex", "plan it [wait]", None)
            .await
            .unwrap();
    let id = started.session.id.clone();
    // The cancel came while the turn was moving into its wait: it ends the wait, not just the
    // step that had already finished.
    let deadline = Instant::now() + WAIT;
    let cancel = loop {
        if let Some(cancel) = hook.cancel.lock().unwrap().take() {
            break cancel;
        }
        assert!(Instant::now() < deadline, "the hook never cancelled");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let detail = cancel.await.unwrap().unwrap();
    let turn = &detail.turns[0];
    assert!(!turn.waiting && !turn.running, "{turn:#?}");
    assert_eq!(outcome(turn), TurnOutcome::Cancelled);
    assert!(detail.session.waiting_task_id.is_none());
    assert!(detail.session.active_task_id.is_none());
    // The session takes new work.
    h.rt.resume_session(&id, "next").await.unwrap();
    settled(&h.rt, &id, 2).await;
}

#[tokio::test]
async fn continuing_needs_a_waiting_turn_and_a_free_worker_slot() {
    let h = harness_config(personas(), None, |c| c.max_active_turns = 1);
    with_hook(&h);
    let (id, task) = waiting_turn(&h, "codex").await;
    // Only waiting turns continue.
    let err =
        h.rt.continue_turn(&id, "another-task", "x", StepNote::default())
            .await
            .unwrap_err();
    assert!(matches!(err, RuntimeError::NotWaiting(_)), "{err}");
    let unknown = "00000000-0000-4000-8000-000000000000";
    assert!(matches!(
        h.rt.continue_turn(unknown, &task, "x", StepNote::default())
            .await,
        Err(RuntimeError::NotWaiting(_))
    ));
    // A waiting turn holds no worker slot, but a continuation needs one.
    let slow =
        h.rt.start_session("claude-code", "busy [slow]", None)
            .await
            .unwrap();
    let err =
        h.rt.continue_turn(&id, &task, "replies", StepNote::default())
            .await
            .unwrap_err();
    assert!(matches!(err, RuntimeError::Busy(_)), "{err}");
    assert!(err.is_caller_error());
    let detail = h.rt.session(&id).await.unwrap();
    assert!(
        detail.turns[0].waiting,
        "still waiting after a busy refusal"
    );
    // The global cap refuses new sessions the same way.
    assert!(matches!(
        h.rt.start_session("codex", "one more", None).await,
        Err(RuntimeError::Busy(_))
    ));
    h.rt.cancel_turn(&slow.session.id).await.unwrap();
    h.rt.continue_turn(&id, &task, "replies", StepNote::default())
        .await
        .unwrap();
    let detail = turn_where(&h.rt, &id, 1, |t| t.result.is_some()).await;
    assert_eq!(outcome(&detail.turns[0]), TurnOutcome::Completed);
}

#[tokio::test]
async fn a_turn_that_cannot_continue_ends_with_the_reason() {
    let h = harness();
    with_hook(&h);
    let (id, task) = waiting_turn(&h, "codex").await;
    h.set_auth("signed-out");
    let err =
        h.rt.continue_turn(&id, &task, "replies", StepNote::default())
            .await
            .unwrap_err();
    assert!(matches!(err, RuntimeError::NotReady(_)), "{err}");
    let detail = h.rt.session(&id).await.unwrap();
    let turn = &detail.turns[0];
    let result = turn.result.clone().unwrap();
    assert_eq!(result.outcome, TurnOutcome::AuthRequired);
    assert!(
        result.summary.starts_with("Could not continue"),
        "{}",
        result.summary
    );
    assert!(!turn.waiting && detail.session.waiting_task_id.is_none());
    assert_eq!(turn.steps.len(), 1);
}

#[tokio::test]
async fn sessions_start_with_a_chosen_id_metadata_and_prompt() {
    let h = harness();
    let id = "5d0b2c6e-8d7a-4f3e-9a51-0c2b8e4f6a7d";
    let started =
        h.rt.start_session_with(
            SessionStart {
                id: Some(id.into()),
                runtime_id: "codex".into(),
                model: None,
                effort: Some(Effort::Ultra),
                title: Some("Chosen title\nsecond line".into()),
                metadata: serde_json::json!({ "origin": "test" }),
            },
            TurnInput {
                objective: "the recorded objective".into(),
                prompt: Some("the prompt that is sent".into()),
                brief: None,
                task: TurnTask::New {
                    requested_by: "agent:tester".into(),
                    metadata: serde_json::json!({ "extra": 1 }),
                    project_id: None,
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(started.session.id, id);
    assert_eq!(started.session.title, "Chosen title");
    assert_eq!(started.session.effort, Some(Effort::Ultra));
    assert_eq!(started.session.metadata["origin"], "test");
    let detail = settled(&h.rt, id, 1).await;
    let turn = &detail.turns[0];
    assert_eq!(turn.objective, "the recorded objective");
    assert_eq!(
        turn.result.as_ref().unwrap().text.as_deref(),
        Some("Turn 1: you said \"the prompt that is sent\". Previous: None.")
    );
    // Every turn of the conversation runs at its effort level.
    let effort = "model_reasoning_effort=ultra".to_owned();
    assert!(h.last_args().contains(&effort), "{:?}", h.last_args());
    h.rt.resume_session(id, "and again").await.unwrap();
    settled(&h.rt, id, 2).await;
    assert!(h.last_args().contains(&effort), "{:?}", h.last_args());
    // The same ID cannot be opened twice; bad input is refused before anything runs.
    let again =
        h.rt.start_session_with(
            SessionStart {
                id: Some(id.into()),
                runtime_id: "codex".into(),
                ..SessionStart::default()
            },
            TurnInput::owner("again"),
        )
        .await;
    assert!(again.is_err());
    for (start, input) in [
        (
            SessionStart {
                id: Some("../escape".into()),
                runtime_id: "codex".into(),
                ..SessionStart::default()
            },
            TurnInput::owner("x"),
        ),
        (
            SessionStart {
                runtime_id: "codex".into(),
                metadata: serde_json::json!(["not", "an", "object"]),
                ..SessionStart::default()
            },
            TurnInput::owner("x"),
        ),
        (
            SessionStart {
                runtime_id: "codex".into(),
                ..SessionStart::default()
            },
            TurnInput {
                prompt: Some(" ".into()),
                ..TurnInput::owner("x")
            },
        ),
        (
            // No Codex model has a "minimal" effort level.
            SessionStart {
                runtime_id: "codex".into(),
                effort: Some(Effort::Minimal),
                ..SessionStart::default()
            },
            TurnInput::owner("x"),
        ),
    ] {
        let err = h.rt.start_session_with(start, input).await.unwrap_err();
        assert!(matches!(err, RuntimeError::InvalidInput(_)), "{err}");
    }
    assert_eq!(h.store.sessions(10).unwrap().len(), 1);
}

#[tokio::test]
async fn a_recorded_task_can_be_adopted_as_a_turn() {
    let h = harness();
    let id = "7a1c3e5f-2b4d-4c6e-8f0a-1b3d5f7a9c2e";
    h.store.insert_turn(unstarted("t-adopt", id, "review this"));
    h.rt.start_session_with(
        SessionStart {
            id: Some(id.into()),
            runtime_id: "claude-code".into(),
            ..SessionStart::default()
        },
        TurnInput {
            objective: "review this".into(),
            prompt: Some("Please review this".into()),
            brief: None,
            task: TurnTask::Existing {
                task_id: "t-adopt".into(),
            },
        },
    )
    .await
    .unwrap();
    let detail = settled(&h.rt, id, 1).await;
    assert_eq!(detail.turns.len(), 1);
    assert_eq!(detail.turns[0].task_id, "t-adopt");
    assert_eq!(outcome(&detail.turns[0]), TurnOutcome::Completed);
    // A task that already ran cannot be adopted again.
    let err =
        h.rt.start_session_with(
            SessionStart {
                runtime_id: "codex".into(),
                ..SessionStart::default()
            },
            TurnInput {
                objective: "again".into(),
                prompt: None,
                brief: None,
                task: TurnTask::Existing {
                    task_id: "t-adopt".into(),
                },
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RuntimeError::Store(_)), "{err}");
}

// ---- Kimi (ADR-027: Kimi over ACP, with its file reads and writes going through Plenipo) ------

async fn run_kimi(
    h: &H,
    objective: &str,
    model: Option<&str>,
    effort: Option<Effort>,
) -> (AgentSessionDetail, AgentTurn) {
    let started =
        h.rt.start_session_with(
            SessionStart {
                runtime_id: "kimi".into(),
                model: model.map(str::to_owned),
                effort,
                ..SessionStart::default()
            },
            TurnInput::owner(objective),
        )
        .await
        .unwrap();
    let detail = settled(&h.rt, &started.session.id, 1).await;
    let turn = detail.turns.last().unwrap().clone();
    (detail, turn)
}

#[tokio::test]
async fn kimi_takes_its_mode_model_and_thinking_level_before_the_prompt() {
    let h = harness();
    let (detail, turn) = run_kimi(
        &h,
        "hello [settings]",
        Some("kimi-code/kimi-for-coding-highspeed"),
        Some(Effort::Low),
    )
    .await;
    let result = turn.result.unwrap();
    assert_eq!(result.outcome, TurnOutcome::Completed, "{result:#?}");
    // A worker without Plenipo's tools runs in Kimi's read-only mode.
    assert!(
        result.text.as_deref().unwrap().contains(
            "Settings: model kimi-code/kimi-for-coding-highspeed, thinking low, mode plan."
        ),
        "{result:#?}"
    );
    assert_eq!(
        detail.session.model.as_deref(),
        Some("kimi-code/kimi-for-coding-highspeed")
    );
    let acp = h.last_acp();
    assert_eq!(
        acp["settings"],
        serde_json::json!([
            ["mode", "plan"],
            ["model", "kimi-code/kimi-for-coding-highspeed"],
            ["thinking", "low"]
        ])
    );
    // File access through Plenipo; no terminal; the prompt only on stdin.
    let caps = &acp["initialize"]["clientCapabilities"];
    assert_eq!(caps["fs"]["readTextFile"], true);
    assert_eq!(caps["terminal"], false);
    assert_eq!(h.last_args(), ["acp"]);
    assert!(!h.state().join("authenticate-called").exists());

    // No model named: Kimi's default is named anyway, never Kimi's own setting.
    let h = harness();
    let (_, turn) = run_kimi(&h, "hi [settings]", None, None).await;
    assert!(turn
        .result
        .unwrap()
        .text
        .unwrap()
        .contains("model kimi-code/k3,"));
    assert_eq!(
        h.last_acp()["settings"],
        serde_json::json!([["mode", "plan"], ["model", "kimi-code/k3"]])
    );
}

#[tokio::test]
async fn kimi_without_permissions_reads_changes_and_runs_nothing() {
    let h = harness();
    let secret = h.dir.path().join("secret.txt");
    std::fs::write(&secret, "do not read").unwrap();
    let objective = format!(
        "try [own-read:{}] [own-write:made.txt|x] [own-shell]",
        secret.display()
    );
    let (_, turn) = run_kimi(&h, &objective, None, None).await;
    let result = turn.result.unwrap();
    assert_eq!(result.outcome, TurnOutcome::Completed, "{result:#?}");
    let text = result.text.unwrap();
    assert!(
        text.contains("failed: Not done: this worker has no permission"),
        "{text}"
    );
    assert!(!text.contains("do not read"), "{text}");
    assert!(text.contains("Write made.txt answer: reject."), "{text}");
    assert!(text.contains("Shell answer: reject."), "{text}");
    let activity = h.store.activity(&turn.task_id);
    let notices: Vec<String> = activity
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Notice { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        notices.iter().any(|n| n.contains("run_command")),
        "{notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|n| n.contains("no permission to use files")),
        "{notices:?}"
    );
}

#[tokio::test]
async fn kimi_is_stopped_when_it_leaves_the_modes_plenipo_allows() {
    let h = harness();
    let (_, turn) = run_kimi(&h, "go [yolo] [delay:2000]", None, None).await;
    let result = turn.result.unwrap();
    assert_eq!(result.outcome, TurnOutcome::Failed, "{result:#?}");
    assert!(result.summary.contains("\"yolo\""), "{}", result.summary);
}

#[tokio::test]
async fn kimi_runs_only_the_subscription_models() {
    let h = harness();
    let (_, turn) = run_kimi(&h, "hi", Some("moonshot/kimi-k2"), None).await;
    let result = turn.result.unwrap();
    assert_eq!(
        result.outcome,
        TurnOutcome::BillingNotAllowed,
        "{result:#?}"
    );
    // Refused before any conversation was opened.
    assert!(h.last_acp().get("method").is_none());
}

// ---- Prompts sized to the job (ADR-044) --------------------------------------------------------

/// Liaison's message for `objective`: the full instructions (`hash` names them), and a short
/// reminder instead of them.
fn briefed(objective: &str, hash: u64) -> TurnInput {
    let full = format!("[instructions]\nWho you are, your job, your team.\n[end]\n\n{objective}");
    let reminder = format!("[instructions]\nThey still apply.\n[end]\n\n{objective}");
    TurnInput {
        objective: objective.into(),
        prompt: None,
        brief: Some(BriefInput {
            full,
            reminder: Some(reminder),
            passed_bytes: objective.len(),
            reminder_passed_bytes: objective.len(),
            hash,
            large: false,
        }),
        task: TurnTask::New {
            requested_by: "owner".into(),
            metadata: serde_json::Value::Null,
            project_id: None,
        },
    }
}

/// Give a session its next briefed objective and wait for it; returns its step sizes.
async fn briefed_turn(h: &H, id: &str, input: TurnInput) -> Vec<PromptSize> {
    let turns = h.rt.session(id).await.unwrap().turns.len();
    h.rt.resume_session_with(id, input).await.unwrap();
    let detail = settled(&h.rt, id, turns + 1).await;
    detail.turns[turns]
        .steps
        .iter()
        .map(|s| s.result.as_ref().unwrap().prompt.expect("a size"))
        .collect()
}

/// ADR-044 §2.5: every AI tool that says it shortened its memory is heard — Claude Code's
/// "compacted" notice, a drop in the context Grok and Kimi report in use, the Ollama bridge
/// leaving earlier messages out — and the next task gets the full instructions again.
#[tokio::test]
async fn a_shortened_memory_is_heard_and_the_next_task_gets_the_full_instructions() {
    for runtime in ["claude-code", "grok", "kimi", "ollama"] {
        let h = harness();
        let started =
            h.rt.start_session_with(
                SessionStart {
                    runtime_id: runtime.into(),
                    ..SessionStart::default()
                },
                briefed("one", 7),
            )
            .await
            .unwrap();
        let id = started.session.id.clone();
        let detail = settled(&h.rt, &id, 1).await;
        let first = detail.turns[0].steps[0]
            .result
            .clone()
            .unwrap()
            .prompt
            .unwrap();
        assert_eq!(
            (first.brief, first.why),
            (BriefKind::Full, Some(BriefWhy::First))
        );
        briefed_turn(&h, &id, briefed("two", 7)).await;
        // What was sent so far is still in the conversation: its mark holds (ADR-044 §4.13).
        // Records are named by ID only where the AI tool itself says when it shortens its
        // memory: Grok and Kimi report the context in use only after each answer, which can miss
        // a shortening in the middle of a step, so their records are pasted, as for Codex.
        let names_records = matches!(runtime, "claude-code" | "ollama");
        let mark = h.rt.memory_mark(&id);
        assert_eq!(mark.is_some(), names_records, "{runtime}");
        briefed_turn(&h, &id, briefed("three [compact]", 7)).await;
        let moved = h.rt.memory_mark(&id);
        if names_records {
            assert!(
                moved.is_some() && moved != mark,
                "{runtime}: {mark:?} {moved:?}"
            );
        } else {
            assert_eq!(moved, None, "{runtime}");
        }
        let detail = h.rt.session(&id).await.unwrap();
        let shortened: Vec<AgentEvent> = h
            .store
            .activity(&detail.turns[2].task_id)
            .into_iter()
            .filter(|e| matches!(e, AgentEvent::MemoryShortened { .. }))
            .collect();
        assert_eq!(shortened.len(), 1, "{runtime}: {shortened:?}");
        let next = briefed_turn(&h, &id, briefed("four", 7)).await;
        assert_eq!(
            (next[0].brief, next[0].why),
            (BriefKind::Full, Some(BriefWhy::MemoryShortened)),
            "{runtime}"
        );
        // Once sent, the full instructions are what the conversation has again.
        let after = briefed_turn(&h, &id, briefed("five", 7)).await;
        assert_eq!(
            (after[0].brief, after[0].why),
            (BriefKind::Reminder, Some(BriefWhy::Routine)),
            "{runtime}"
        );
        assert_eq!(h.rt.memory_mark(&id), moved, "{runtime}");
    }
}

/// ADR-044 §2: a task that sent other instructions in full and then did not finish leaves them
/// in doubt; the next task sends its instructions in full, even when they are the earlier ones.
#[tokio::test]
async fn instructions_a_failed_task_sent_go_out_in_full_again() {
    let h = harness();
    let id = briefed_session(&h, "claude-code", "one").await;
    // Changed instructions go out in full, and the task stops at a usage limit.
    let failed = briefed_turn(&h, &id, briefed("two [usage-limit]", 8)).await[0];
    assert_eq!(
        (failed.brief, failed.why),
        (BriefKind::Full, Some(BriefWhy::Changed))
    );
    // Back to the first instructions: the AI tool may have either, so they go out in full.
    let back = briefed_turn(&h, &id, briefed("three", 7)).await[0];
    assert_eq!(
        (back.brief, back.why),
        (BriefKind::Full, Some(BriefWhy::Changed))
    );
    let after = briefed_turn(&h, &id, briefed("four", 7)).await[0];
    assert_eq!(
        (after.brief, after.why),
        (BriefKind::Reminder, Some(BriefWhy::Routine))
    );
}

/// ADR-044 §2.5: when the Ollama bridge would have to leave the start of a conversation out of
/// the next task, Plenipo knows before it sends it, and the task goes out with the full
/// instructions instead of a reminder of what the model would no longer see.
#[tokio::test]
async fn an_ollama_conversation_too_long_to_send_whole_gets_the_full_instructions() {
    let h = harness();
    let id = briefed_session(&h, "ollama", "one").await;
    let routine = briefed_turn(&h, &id, briefed("two", 7)).await[0];
    assert_eq!(routine.brief, BriefKind::Reminder);
    // The conversation the bridge keeps grows past what it sends at once.
    let session = h.rt.session(&id).await.unwrap().session;
    let provider = session.provider_session_id.unwrap();
    let history = serde_json::json!({ "model": "m", "messages": [
        { "role": "user", "content": "x".repeat(450_000) },
        { "role": "assistant", "content": "ok" },
    ]});
    std::fs::write(
        Path::new(&session.working_dir).join(format!(".plenipo-ollama-{provider}.json")),
        history.to_string(),
    )
    .unwrap();
    let next = briefed_turn(&h, &id, briefed("three", 7)).await[0];
    assert_eq!(
        (next.brief, next.why),
        (BriefKind::Full, Some(BriefWhy::MemoryShortened))
    );
}

/// What the fake AI tool received in a provider conversation: each message (without Plenipo's
/// tools note) and its whole size.
fn received(h: &H, provider_session: &str) -> (Vec<String>, Vec<u32>) {
    let file = h
        .state()
        .join("sessions")
        .join(format!("{provider_session}.json"));
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let strings = |key: &str| -> Vec<serde_json::Value> { v[key].as_array().unwrap().clone() };
    (
        strings("prompts")
            .iter()
            .map(|p| p.as_str().unwrap().to_owned())
            .collect(),
        strings("sizes")
            .iter()
            .map(|s| u32::try_from(s.as_u64().unwrap()).unwrap())
            .collect(),
    )
}

async fn briefed_session(h: &H, runtime: &str, objective: &str) -> String {
    let started =
        h.rt.start_session_with(
            SessionStart {
                runtime_id: runtime.into(),
                ..SessionStart::default()
            },
            briefed(objective, 7),
        )
        .await
        .unwrap();
    settled(&h.rt, &started.session.id, 1).await;
    started.session.id
}

/// ADR-044 §2–§3: a routine objective gets a short reminder; the first one, changed
/// instructions, a large job, the 10th objective after the full instructions, and a message
/// without a short form get the full instructions.
#[tokio::test]
async fn routine_objectives_get_a_short_reminder_and_the_rest_the_full_instructions() {
    let h = harness();
    let id = briefed_session(&h, "codex", "one").await;
    // Codex does not say when it shortens its memory: Plenipo cannot tell what the
    // conversation still has, so saved records are pasted again every time (ADR-044 §4.13).
    assert_eq!(h.rt.memory_mark(&id), None);
    let routine = briefed_turn(&h, &id, briefed("two", 7)).await[0];
    assert_eq!(
        (routine.brief, routine.why),
        (BriefKind::Reminder, Some(BriefWhy::Routine))
    );
    assert!(routine.own_bytes < routine.full_own_bytes, "{routine:?}");
    // What the AI tool really received: the reminder, then the objective.
    let provider =
        h.rt.session(&id)
            .await
            .unwrap()
            .session
            .provider_session_id
            .unwrap();
    let (prompts, sizes) = received(&h, &provider);
    assert!(
        prompts[0].starts_with("[instructions]\nWho you are"),
        "{prompts:?}"
    );
    assert_eq!(
        prompts[1],
        "[instructions]\nThey still apply.\n[end]\n\ntwo"
    );
    assert_eq!(sizes[1], routine.bytes, "the recorded size is what arrived");

    let why = |sizes: Vec<PromptSize>| (sizes[0].brief, sizes[0].why.unwrap());
    assert_eq!(
        why(briefed_turn(&h, &id, briefed("new team", 8)).await),
        (BriefKind::Full, BriefWhy::Changed)
    );
    let mut large = briefed("a large job", 8);
    if let Some(b) = large.brief.as_mut() {
        b.large = true;
    }
    assert_eq!(
        why(briefed_turn(&h, &id, large).await),
        (BriefKind::Full, BriefWhy::LargeJob)
    );
    // Nine reminders, then the 10th objective after the full instructions gets them again.
    for n in 1..=9 {
        let (kind, _) = why(briefed_turn(&h, &id, briefed(&format!("routine {n}"), 8)).await);
        assert_eq!(kind, BriefKind::Reminder, "routine {n}");
    }
    assert_eq!(
        why(briefed_turn(&h, &id, briefed("the tenth", 8)).await),
        (BriefKind::Full, BriefWhy::EveryTenth)
    );
    let mut plain = briefed("no short form", 8);
    if let Some(b) = plain.brief.as_mut() {
        b.reminder = None;
    }
    assert_eq!(
        why(briefed_turn(&h, &id, plain).await),
        (BriefKind::Full, BriefWhy::NoReminder)
    );
    // An objective without a brief is sent as it is.
    let bare = briefed_turn(&h, &id, TurnInput::owner("just this")).await[0];
    assert_eq!((bare.brief, bare.why), (BriefKind::Plain, None));
    assert_eq!(bare.own_bytes, 0, "the owner's objective alone");
}

/// Plenipo started again: a new runtime over the same records, with the same AI tools.
fn restarted(h: &H) -> AgentRuntime {
    let home = h.dir.path().join("home");
    let mut config = AgentConfig::new(h.dir.path().join("workspaces"));
    config.extra_env = vec![(HOME_VAR.into(), home.display().to_string())];
    config.turn_timeout = Duration::from_secs(120);
    AgentRuntime::new(
        config,
        builtin_adapters(),
        h.sup.clone(),
        h.store.clone(),
        h.updates.clone(),
        HostEnv::new(Some(h.bin().into_os_string()), Some(home), None),
    )
}

/// ADR-044 §2.4: after Plenipo starts again it cannot know what the AI tool kept, so the next
/// task gets the full instructions.
#[tokio::test]
async fn after_a_restart_the_next_task_gets_the_full_instructions() {
    let h = harness();
    let id = briefed_session(&h, "claude-code", "one").await;
    let before = briefed_turn(&h, &id, briefed("two", 7)).await[0];
    assert_eq!(before.brief, BriefKind::Reminder);
    let rt = restarted(&h);
    // Nothing is known of the conversation after a restart.
    assert_eq!(rt.memory_mark(&id), None);
    assert!(h.rt.memory_mark(&id).is_some());
    rt.resume_session_with(&id, briefed("three", 7))
        .await
        .unwrap();
    let detail = settled(&rt, &id, 3).await;
    let after = detail.turns[2].steps[0]
        .result
        .clone()
        .unwrap()
        .prompt
        .unwrap();
    assert_eq!(
        (after.brief, after.why),
        (BriefKind::Full, Some(BriefWhy::AfterRestart))
    );
    rt.resume_session_with(&id, briefed("four", 7))
        .await
        .unwrap();
    let detail = settled(&rt, &id, 4).await;
    let next = detail.turns[3].steps[0]
        .result
        .clone()
        .unwrap()
        .prompt
        .unwrap();
    assert_eq!(next.brief, BriefKind::Reminder);
}

/// Gives every step a permissions note that a test can change.
struct Notes(Mutex<String>);

impl ToolProvider for Notes {
    fn open(&self, _: &StepInfo<'_>) -> Option<StepTools> {
        None
    }

    fn note_without_tools(&self, _: &StepInfo<'_>) -> Option<String> {
        Some(self.0.lock().unwrap().clone())
    }

    fn close(&self, _: &str) {}
}

/// ADR-044 §3: the permissions note goes out in full the first time, with the full
/// instructions, when it changed, and after a shortened memory; otherwise a short note that
/// keeps the safety rules in view — also for the step that delivers replies.
#[tokio::test]
async fn the_permissions_note_goes_out_in_full_only_when_needed() {
    let h = harness();
    // As long as a real note: longer than the short one that stands in for it.
    let note_text = |what: &str| {
        format!(
            "You can use Plenipo's tools for the Website project. You may: {what}. Every use is \
             checked and recorded.\n"
        )
        .repeat(4)
    };
    let notes = Arc::new(Notes(Mutex::new(note_text(
        "reading files; changing files",
    ))));
    h.rt.set_tools(notes.clone());
    with_hook(&h);
    let id = briefed_session(&h, "claude-code", "one").await;
    let first = h.rt.session(&id).await.unwrap().turns[0].steps[0]
        .result
        .clone()
        .unwrap()
        .prompt
        .unwrap();
    assert_eq!(first.note, NoteKind::Full);
    let second = briefed_turn(&h, &id, briefed("two", 7)).await[0];
    assert_eq!(
        (second.brief, second.note),
        (BriefKind::Reminder, NoteKind::Reminder)
    );
    // The short note is shorter, and the size says what the full one would have been.
    assert!(second.own_bytes < second.full_own_bytes, "{second:?}");

    // A step that delivers replies carries the short note too.
    h.rt.resume_session_with(&id, briefed("plan it [wait]", 7))
        .await
        .unwrap();
    let detail = turn_where(&h.rt, &id, 3, |t| t.waiting).await;
    let task = detail.turns[2].task_id.clone();
    let deadline = Instant::now() + WAIT;
    while h
        .rt
        .session(&id)
        .await
        .unwrap()
        .session
        .waiting_task_id
        .is_none()
    {
        assert!(Instant::now() < deadline, "the turn never waited");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let note = StepNote {
        reason: "replies arrived".into(),
        data: serde_json::Value::Null,
        passed_bytes: "the replies".len(),
    };
    h.rt.continue_turn(&id, &task, "the replies", note)
        .await
        .unwrap();
    // Done and free: the result is recorded a moment before the turn stops running, and the
    // next objective below would be refused as busy in between.
    let detail = settled(&h.rt, &id, 3).await;
    let replies = detail.turns[2].steps[1]
        .result
        .clone()
        .unwrap()
        .prompt
        .unwrap();
    assert_eq!(
        (replies.brief, replies.note, replies.why),
        (BriefKind::Replies, NoteKind::Reminder, None)
    );

    // New permissions: the note goes out in full, though the instructions are a reminder.
    *notes.0.lock().unwrap() = note_text("reading files");
    let changed = briefed_turn(&h, &id, briefed("four", 7)).await[0];
    assert_eq!(
        (changed.brief, changed.note),
        (BriefKind::Reminder, NoteKind::Full)
    );
    let same = briefed_turn(&h, &id, briefed("five", 7)).await[0];
    assert_eq!(same.note, NoteKind::Reminder);
    // After the AI tool shortened its memory, everything goes out in full.
    briefed_turn(&h, &id, briefed("six [compact]", 7)).await;
    let after = briefed_turn(&h, &id, briefed("seven", 7)).await[0];
    assert_eq!((after.brief, after.note), (BriefKind::Full, NoteKind::Full));
    // What arrived is what was recorded, note included.
    let provider =
        h.rt.session(&id)
            .await
            .unwrap()
            .session
            .provider_session_id
            .unwrap();
    let (_, sizes) = received(&h, &provider);
    assert_eq!(sizes[1], second.bytes);
    // A note no longer than the short one always goes out as it is.
    *notes.0.lock().unwrap() = "You have no Plenipo tools in this task.".into();
    for objective in ["eight", "nine"] {
        let size = briefed_turn(&h, &id, briefed(objective, 7)).await[0];
        assert_eq!(size.note, NoteKind::Full, "{objective}");
    }
}
