//! Phase 3 runtime adapter tests: the real session service, supervisor, and adapters driving
//! `plenipo-fake-agent` installed as `claude` and `codex`. Each plan test runs for both
//! runtimes. No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentEvent, AgentRuntime, AgentSessionDetail, AgentSink,
    AgentTurn, AgentUpdate, AuthState, HostEnv, InstallState, MemorySessionStore, SessionState,
    SessionStore, TurnOutcome,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ExecutionState, MetadataStore, ProfileRegistry, RuntimeError,
    RuntimeEvent, Supervisor, SupervisorConfig,
};

const RUNTIMES: [&str; 2] = ["claude-code", "codex"];
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

    fn last_env(&self) -> Vec<String> {
        std::fs::read_to_string(self.state().join("last-env.txt"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn harness_with(installed: &[&str], auth: Option<&str>) -> H {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    for stem in installed {
        std::fs::copy(fake_exe(), bin.join(exe_name(stem))).unwrap();
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
    harness_with(&["claude", "codex"], None)
}

/// Wait until the session has `turns` turns and none is running.
async fn settled(rt: &AgentRuntime, session_id: &str, turns: usize) -> AgentSessionDetail {
    let deadline = Instant::now() + WAIT;
    loop {
        let detail = rt.session(session_id).await.unwrap();
        if detail.turns.len() >= turns && detail.turns.iter().all(|t| !t.running) {
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
    assert_eq!(runtimes.len(), 2);
    for (info, version) in runtimes.iter().zip(["2.1.999", "0.99.0"]) {
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
    // No account identifier from the status output is kept.
    assert!(!format!("{runtimes:?}").contains("owner@example.com"));
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
async fn missing_runtimes_are_reported_not_installed() {
    let h = harness_with(&[], None);
    let before = h.rt.runtimes();
    assert!(before
        .iter()
        .all(|r| r.installation.state == InstallState::Checking));
    for info in h.rt.refresh().await {
        // A machine-wide install in a well-known location would legitimately be found.
        if info.installation.state == InstallState::Installed {
            continue;
        }
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
    if info.installation.state != InstallState::Installed {
        assert_eq!(info.installation.state, InstallState::Unsupported);
        assert!(info.installation.detail.unwrap().contains("claude.cmd"));
    }
}

#[tokio::test]
async fn unauthenticated_runtimes_refuse_work_with_login_guidance() {
    for runtime in RUNTIMES {
        let h = harness_with(&["claude", "codex"], Some("signed-out"));
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
        ("cloud", "claude-code", AuthState::ThirdPartyCloud),
    ] {
        let h = harness_with(&["claude", "codex"], Some(mode));
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
    let h = harness_with(&["claude", "codex"], Some("unknown-status"));
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
        assert!(result.usage.is_some_and(|u| u.output_tokens > 0));
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
        if runtime == "claude-code" {
            let deltas = live
                .iter()
                .filter(|e| matches!(e, AgentEvent::TextDelta { .. }))
                .count();
            assert!(deltas > 1, "text streams incrementally ({deltas})");
        } else {
            assert!(live.iter().any(|e| matches!(e, AgentEvent::ToolUse { .. })));
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
        assert!(agent.usage.is_some());
        let args = h.last_args();
        assert!(!args.iter().any(|a| a.contains("hello")), "{args:?}");
        assert_eq!(exec.args, args);
        // The working directory is the session's own workspace.
        assert!(Path::new(&exec.working_dir).ends_with(&session.id));
        // No credentials reached the CLI.
        let env = h.last_env();
        for secret in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CODEX_API_KEY"] {
            assert!(!env.iter().any(|n| n == secret), "{secret}");
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
        let args = h.last_args();
        let resume_flag = if runtime == "claude-code" {
            "--resume"
        } else {
            "resume"
        };
        let i = args
            .iter()
            .position(|a| a == resume_flag)
            .expect("resume argument");
        assert_eq!(args[i + 1], provider_id);
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
        assert_eq!(exec.state, ExecutionState::Cancelled);
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
        assert!(h.rt.refresh().await.iter().all(|r| r.ready));
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
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemorySessionStore::default());
    store.insert_turn(AgentTurn {
        task_id: "t-1".into(),
        session_id: "s-1".into(),
        number: 1,
        objective: "left running".into(),
        execution_id: Some("e-1".into()),
        running: true,
        result: None,
        started_at: 1,
        ended_at: None,
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
    let turn = store.turns("s-1").unwrap().remove(0);
    assert!(!turn.running);
    assert_eq!(outcome(&turn), TurnOutcome::Interrupted);
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
