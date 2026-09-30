//! Phase 19 tests (ADR-058 to ADR-060): the AI tools page's service through the real broker,
//! Guard, supervisor, agent runtime, adapters, and a file-backed Ledger, with
//! `plenipo-fake-agent` installed as every AI tool and a stand-in for the published release
//! lists on 127.0.0.1. No internet, no accounts.
//!
//! The plan's tests covered here:
//! - Sign in opens a terminal tab running exactly that tool's login command, and nothing else
//!   can be started that way.
//! - After the tab closes, the card re-checks and shows the new sign-in state.
//! - An update never starts while a task is using that tool; it waits.
//! - A failed update leaves the old version working and says so.
//! - After an update, the version, sign-in, and models are re-checked.
//! - Usage totals match the saved turns.
//! - The payment switch cannot be turned to a paid key before Phase 16.
//!
//! (A model reported but not checked, chosen in a menu: the router and the screen tests. The limit
//! and reset time on the card: the screen tests.)

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use plenipo_capabilities::ai_tools::{AiToolUpdateState, AiTools, PaymentMethod, UpdateBy};
use plenipo_capabilities::{
    Broker, BrokerConfig, MemorySecretStore, TerminalEvent, TerminalInfo, TerminalPlace,
};
use plenipo_guard::{Guard, OutboundRules};
use plenipo_ledger::{Ledger, DB_FILE_NAME};
use plenipo_liaison::store::{LedgerExecutionStore, LedgerSessionStore};
use plenipo_runtime::agent::{
    builtin_adapters, AccountAction, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, AuthState,
    Bridge, HostEnv, PlanReport,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};
use serde_json::Value;

const WAIT: Duration = Duration::from_secs(60);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn personas() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_plenipo-fake-agent-capabilities"))
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

fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ai-tools-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(env!("CARGO_BIN_EXE_plenipo-fake-agent-capabilities"), &path).unwrap();
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

/// A stand-in for the published release lists: npm's `latest` and GitHub's latest release,
/// answering every request with the version in `newest`.
struct Releases {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

fn releases(newest: &'static str) -> Releases {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            let _ = reader.read_line(&mut first);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let path = first
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_owned();
            seen.lock().unwrap().push(path.clone());
            let body = if path.contains("api.github.com") {
                format!(r#"{{"tag_name":"v{newest}"}}"#)
            } else {
                format!(r#"{{"name":"x","version":"{newest}"}}"#)
            };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    Releases { base, asked }
}

struct H {
    ledger: Arc<Ledger>,
    rt: AgentRuntime,
    broker: Broker,
    tools: AiTools,
    state: PathBuf,
    releases: Releases,
    _dir: tempfile::TempDir,
}

async fn harness(auth: &str) -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    let home = dir.path().join("home");
    let state = home.join(".plenipo-fake-agent");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("auth"), auth).unwrap();
    for stem in personas() {
        let (source, target) = (fake_clis().join(exe_name(stem)), bin.join(exe_name(stem)));
        if std::fs::hard_link(&source, &target).is_err() {
            std::fs::copy(&source, &target).unwrap();
        }
    }
    let ledger = Arc::new(Ledger::open(&dir.path().join("ledger").join(DB_FILE_NAME)).unwrap());
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(LedgerExecutionStore(Arc::clone(&ledger))),
        Arc::new(NoOutput),
        vec![],
    );
    let mut config = AgentConfig::new(dir.path().join("workspaces"));
    config.extra_env = vec![(HOME_VAR.into(), home.display().to_string())];
    config.bridge = Some(Bridge {
        executable: bin.join(exe_name("ollama")),
        args: vec!["--plenipo-ollama".into()],
    });
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup.clone(),
        Arc::new(LedgerSessionStore(Arc::clone(&ledger))),
        Arc::new(NoUpdates),
        HostEnv::new(Some(bin.into_os_string()), Some(home), None),
    );
    rt.refresh().await;
    let guard = Guard::new(Arc::clone(&ledger));
    let mut broker_config =
        BrokerConfig::new(PathBuf::from("unused-relay"), dir.path().join("tickets"));
    broker_config.terminal_refuses_administrator = false;
    let broker = Broker::new(
        guard,
        sup,
        Arc::new(MemorySecretStore::default()),
        broker_config,
    );
    rt.set_tools(Arc::new(broker.clone()));
    // Paid AI keys and the spending caps (ADR-085), as in the app.
    rt.set_paid_gate(plenipo_capabilities::paid::gate(&broker));
    let releases = releases("9.9.9");
    let tools = AiTools::new(
        rt.clone(),
        broker.clone(),
        OutboundRules::default(),
        Some(releases.base.clone()),
    );
    H {
        ledger,
        rt,
        broker,
        tools,
        state,
        releases,
        _dir: dir,
    }
}

/// What one terminal tab sent to the screen.
#[derive(Default)]
struct Screen {
    bytes: Vec<u8>,
    ended: Option<(String, Option<i32>)>,
}

type Shared = Arc<Mutex<Screen>>;

fn sink(screen: &Shared) -> plenipo_capabilities::broker::TerminalSink {
    let screen = Arc::clone(screen);
    Arc::new(move |event: TerminalEvent| {
        let mut s = screen.lock().unwrap();
        match event {
            TerminalEvent::Output { data } => s.bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            ),
            TerminalEvent::Ended { why, code } => s.ended = Some((why, code)),
        }
    })
}

fn text(screen: &Shared) -> String {
    String::from_utf8_lossy(&screen.lock().unwrap().bytes).into_owned()
}

async fn until(what: &str, pred: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !pred() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

impl H {
    /// Open an AI tool's sign-in or sign-out tab, with a screen that answers the terminal's
    /// questions about the cursor, as the owner's screen (xterm.js) does: "top left". As it
    /// starts, Windows' pseudo console asks where the cursor is (ESC [ 6 n) and shows nothing,
    /// and runs nothing, until it hears back.
    async fn open_tab(&self, id: &str, action: AccountAction, screen: &Shared) -> TerminalInfo {
        let info = self
            .tools
            .open_account(id, action, 100, 30, sink(screen))
            .await
            .unwrap();
        let (broker, tab, screen) = (self.broker.clone(), info.id.clone(), Arc::clone(screen));
        tokio::spawn(async move {
            let mut answered = 0;
            loop {
                let (asked, ended) = {
                    let s = screen.lock().unwrap();
                    let asked = s.bytes.windows(4).filter(|w| *w == b"\x1b[6n").count();
                    (asked, s.ended.is_some())
                };
                if ended {
                    break;
                }
                while answered < asked {
                    let _ = broker.write_terminal(&tab, b"\x1b[1;1R");
                    answered += 1;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        info
    }

    fn info(&self, id: &str) -> plenipo_runtime::agent::AgentRuntimeInfo {
        self.rt.runtimes().into_iter().find(|r| r.id == id).unwrap()
    }

    fn events(&self, kind: &str) -> Vec<Value> {
        self.ledger
            .events_of_types(&[kind], 50)
            .unwrap()
            .into_iter()
            .map(|e| e.payload)
            .collect()
    }

    fn read(&self, file: &str) -> Option<String> {
        std::fs::read_to_string(self.state.join(file)).ok()
    }

    fn write(&self, file: &str, text: &str) {
        std::fs::write(self.state.join(file), text).unwrap();
    }

    /// Start a task that keeps `runtime` busy until cancelled; its session.
    async fn busy(&self, runtime: &str) -> String {
        let session = self
            .rt
            .start_session(runtime, "[slow]", None)
            .await
            .unwrap()
            .session
            .id;
        until("the task to start", || {
            !self.rt.tasks_using(runtime).is_empty()
        })
        .await;
        session
    }

    async fn free(&self, runtime: &str, session: &str) {
        self.rt.cancel_turn(session).await.unwrap();
        until("the task to end", || {
            self.rt.tasks_using(runtime).is_empty()
        })
        .await;
    }
}

// ---- Sign in (ADR-058) ------------------------------------------------------------------------------

#[tokio::test]
async fn sign_in_opens_a_tab_running_exactly_that_tools_login_command_and_nothing_else() {
    let h = harness("signed-out").await;
    let screen: Shared = Arc::default();
    let info = h.open_tab("codex", AccountAction::SignIn, &screen).await;
    assert_eq!(info.title, "Sign in · Codex");
    assert_eq!(info.detail, "codex login");
    assert_eq!(
        info.place,
        TerminalPlace::AiTool {
            runtime_id: "codex".into(),
            action: AccountAction::SignIn
        }
    );
    until("the sign-in program", || {
        text(&screen).contains("ABCD-1234")
    })
    .await;
    // Exactly its own command, with its tasks' environment and no key variables.
    let args: Vec<String> = serde_json::from_str(&h.read("last-args.json").unwrap()).unwrap();
    assert_eq!(args, ["login"]);
    let env = h.read("last-env.txt").unwrap();
    assert!(env.lines().any(|n| n == HOME_VAR), "{env}");
    for key in ["OPENAI_API_KEY", "CODEX_API_KEY", "ANTHROPIC_API_KEY"] {
        assert!(!env.lines().any(|n| n == key), "{key} reached the sign-in");
    }
    // Plenipo typed nothing into it.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.read("sign-in-input"), None);
    // Recorded: that it opened, for which tool — never what it showed.
    let opened = h.events("terminal.opened");
    assert_eq!(opened[0]["place"], "aiTool");
    assert_eq!(opened[0]["runtimeId"], "codex");
    assert_eq!(opened[0]["action"], "signIn");
    assert!(!opened[0].to_string().contains("ABCD"));
    h.broker.close_terminal(&info.id, "you closed it").unwrap();
    until("the tab to end", || screen.lock().unwrap().ended.is_some()).await;

    // Nothing else can be started that way.
    let refused = h
        .broker
        .open_terminal(
            &TerminalPlace::AiTool {
                runtime_id: "codex".into(),
                action: AccountAction::SignIn,
            },
            100,
            30,
            sink(&Shared::default()),
        )
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("AI tools page"), "{refused}");
    for bad in [
        r#"{"kind":"aiTool","runtimeId":"calc","action":"signIn"}"#,
        r#"{"kind":"aiTool","runtimeId":"codex","action":"update"}"#,
        r#"{"kind":"aiTool","runtimeId":"codex","action":"signIn","args":["--with-api-key"]}"#,
        r#"{"kind":"aiTool","runtimeId":"codex","action":"signIn","program":"calc.exe"}"#,
        r#"{"kind":"aiTool","runtimeId":"codex"}"#,
    ] {
        assert!(
            serde_json::from_str::<TerminalPlace>(bad).is_err(),
            "{bad} was read"
        );
    }
    // Kimi has no sign-out command: Guard says so, and records it.
    let err = h
        .tools
        .open_account(
            "kimi",
            AccountAction::SignOut,
            100,
            30,
            sink(&Shared::default()),
        )
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "Kimi has no sign-out command of its own.");
    assert_eq!(h.events("guard.ai_tool_refused")[0]["runtime"], "kimi");
}

#[tokio::test]
async fn after_the_tab_closes_the_card_checks_again_and_shows_the_new_sign_in() {
    let h = harness("signed-out").await;
    assert_eq!(h.info("grok").auth.state, AuthState::SignedOut);
    let screen: Shared = Arc::default();
    let info = h.open_tab("grok", AccountAction::SignIn, &screen).await;
    until("the sign-in program", || {
        text(&screen).contains("Press Enter")
    })
    .await;
    // While it runs, no new task starts on Grok.
    assert!(h.rt.held("grok"));
    // The owner signs in, in the tab.
    h.broker.write_terminal(&info.id, b"\r").unwrap();
    until("the tab to end", || screen.lock().unwrap().ended.is_some()).await;
    // The tab says the sign-in ended, not a shell.
    let (why, code) = screen.lock().unwrap().ended.clone().unwrap();
    assert_eq!((why.as_str(), code), ("Grok's sign-in ended", Some(0)));
    until("the check after it", || !h.rt.held("grok")).await;
    let grok = h.info("grok");
    assert_eq!(grok.auth.state, AuthState::Subscription);
    assert!(grok.ready);
    let changed = h.events("ai_tool.sign_in_changed");
    assert_eq!(changed[0]["runtime"], "grok");
    assert_eq!(changed[0]["from"], "signedOut");
    assert_eq!(changed[0]["to"], "subscription");
    assert!(!changed[0].to_string().contains('@'));
    let closed = h.events("terminal.closed");
    assert_eq!(closed[0]["runtimeId"], "grok");
    assert_eq!(closed[0]["exitCode"], 0);

    // Sign out, the same way: no key, the tab ends by itself.
    let screen: Shared = Arc::default();
    h.open_tab("grok", AccountAction::SignOut, &screen).await;
    until("the sign-out tab to end", || {
        screen.lock().unwrap().ended.is_some()
    })
    .await;
    until("the check after it", || !h.rt.held("grok")).await;
    assert_eq!(h.info("grok").auth.state, AuthState::SignedOut);
}

#[tokio::test]
async fn sign_out_waits_while_a_task_is_using_the_tool() {
    let h = harness("subscription").await;
    let session = h.busy("claude-code").await;
    let err = h
        .tools
        .open_account(
            "claude-code",
            AccountAction::SignOut,
            100,
            30,
            sink(&Shared::default()),
        )
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "1 task is using Claude Code. Plenipo waits until it finishes."
    );
    assert!(!h.rt.held("claude-code"));
    assert_eq!(h.read("sign-in-started"), None);
    h.free("claude-code", &session).await;
    let screen: Shared = Arc::default();
    h.open_tab("claude-code", AccountAction::SignOut, &screen)
        .await;
    until("the sign-out", || screen.lock().unwrap().ended.is_some()).await;
}

// ---- Updates (ADR-059) ------------------------------------------------------------------------------

#[tokio::test]
async fn an_update_never_starts_while_a_task_is_using_the_tool_it_waits() {
    let h = harness("subscription").await;
    h.write("newest-grok", "1.0.100");
    let session = h.busy("grok").await;
    let page = h.tools.update("grok", UpdateBy::Owner).unwrap();
    let grok = page.tools.iter().find(|t| t.runtime_id == "grok").unwrap();
    assert_eq!(grok.update.state, AiToolUpdateState::Waiting);
    until("the update to say what it waits for", || {
        let page = h.tools.page();
        page.tools
            .iter()
            .any(|t| t.runtime_id == "grok" && t.update.tasks_using == 1)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(h.read("update-log"), None, "it started while a task ran");
    h.free("grok", &session).await;
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated, "{done:?}");
    assert_eq!(done.to.as_deref(), Some("1.0.100"));
    assert!(h.read("update-log").unwrap().contains("grok update"));

    // A waiting update can be cancelled; it never runs.
    std::fs::remove_file(h.state.join("update-log")).unwrap();
    h.write("newest-grok", "1.0.101");
    let session = h.busy("grok").await;
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    h.tools.cancel_update("grok").unwrap();
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    h.free("grok", &session).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(h.read("update-log"), None);

    // Cancelled, then Update pressed again at once: the new press is not lost, and the update
    // runs once, when the tool is free.
    let session = h.busy("grok").await;
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    h.tools.cancel_update("grok").unwrap();
    let again = h.tools.update("grok", UpdateBy::Owner).unwrap();
    let grok = again.tools.iter().find(|t| t.runtime_id == "grok").unwrap();
    assert_eq!(grok.update.state, AiToolUpdateState::Waiting);
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    h.free("grok", &session).await;
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated, "{done:?}");
    let log = h.read("update-log").unwrap();
    assert_eq!(log.matches("grok update").count(), 1, "{log}");
    // A running update cannot be cancelled: Cancel says so, and nothing is left half done.
    assert!(h.tools.cancel_update("grok").is_err());
}

#[tokio::test]
async fn an_update_and_a_sign_in_tab_take_turns_on_one_tool() {
    let h = harness("subscription").await;
    h.write("newest-grok", "1.0.100");
    // The sign-in tab is open: the update waits until it closes.
    let screen = Shared::default();
    let info = h.open_tab("grok", AccountAction::SignIn, &screen).await;
    until("the sign-in program", || {
        text(&screen).contains("Press Enter")
    })
    .await;
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(
        h.read("update-log"),
        None,
        "it updated while the sign-in tab ran"
    );
    let waiting = h.tools.page();
    let grok = waiting
        .tools
        .iter()
        .find(|t| t.runtime_id == "grok")
        .unwrap();
    assert_eq!(grok.update.state, AiToolUpdateState::Waiting);
    h.broker.write_terminal(&info.id, b"\r").unwrap();
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated, "{done:?}");

    // While it updates, its sign-in tab does not open; Guard says why.
    h.write("newest-grok", "1.0.101");
    h.write("update-grok", "slow");
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    until("the update to run", || {
        h.tools
            .page()
            .tools
            .iter()
            .any(|t| t.runtime_id == "grok" && t.update.state == AiToolUpdateState::Updating)
    })
    .await;
    let refused = h
        .tools
        .open_account(
            "grok",
            AccountAction::SignOut,
            100,
            30,
            sink(&Shared::default()),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("Grok is being updated"), "{refused}");
    let reasons = h.events("guard.ai_tool_refused");
    assert!(
        reasons.iter().any(|e| e["action"] == "signOut"),
        "{reasons:?}"
    );
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated, "{done:?}");
}

#[tokio::test]
async fn a_failed_update_leaves_the_old_version_working_and_says_so() {
    let h = harness("subscription").await;
    h.write("newest-grok", "1.0.100");
    h.write("update-grok", "fail");
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Failed);
    assert_eq!(done.old_still_works, Some(true));
    assert_eq!(done.to.as_deref(), Some("1.0.99"));
    assert!(done.message.unwrap().contains("download failed"));
    let failed = h.events("ai_tool.update_failed");
    assert_eq!(failed[0]["oldStillWorks"], true);
    assert_eq!(failed[0]["from"], "1.0.99");
    let grok = h.info("grok");
    assert_eq!(grok.installation.version.as_deref(), Some("1.0.99"));
    assert!(grok.ready, "the old version still takes tasks");
}

#[tokio::test]
async fn a_new_version_that_does_not_answer_is_put_back_or_given_no_tasks() {
    let h = harness("subscription").await;
    // Grok has its own command to go back: the old version returns.
    h.write("newest-grok", "1.0.100");
    h.write("update-grok", "broken");
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Failed, "{done:?}");
    assert_eq!(done.old_still_works, Some(true));
    assert!(h
        .read("update-log")
        .unwrap()
        .contains("grok update --version 1.0.99"));
    assert_eq!(h.events("ai_tool.put_back")[0]["version"], "1.0.99");
    assert!(h.info("grok").ready);

    // Kimi has none: it gets no tasks until it answers again, and says why.
    h.write("newest-kimi", "0.35.0");
    h.write("update-kimi", "broken");
    h.tools.update("kimi", UpdateBy::Owner).unwrap();
    let done = h.tools.settled("kimi", WAIT).await.unwrap();
    assert_eq!(done.old_still_works, Some(false));
    let kimi = h.info("kimi");
    assert!(!kimi.ready);
    let page = h.tools.page();
    let stopped = page.tools.iter().find(|t| t.runtime_id == "kimi").unwrap();
    assert!(stopped
        .out_of_service
        .as_deref()
        .unwrap()
        .contains("Install it again"));
    // No task starts on it (here its version check fails too, so it says Kimi is not
    // available, with how to install it again).
    let refused = h.rt.start_session("kimi", "hello", None).await.unwrap_err();
    let refused = refused.to_string();
    assert!(
        refused.contains("not giving Kimi tasks") || refused.contains("Kimi is not available"),
        "{refused}"
    );
    // Mended by hand, Check again gives it tasks again.
    std::fs::remove_file(h.state.join("broken-kimi")).unwrap();
    h.tools.check("kimi").await.unwrap();
    assert!(h.info("kimi").ready);
    assert!(h
        .tools
        .page()
        .tools
        .iter()
        .all(|t| t.out_of_service.is_none()));
}

#[tokio::test]
async fn after_an_update_the_version_sign_in_and_models_are_checked_again() {
    let h = harness("subscription").await;
    h.tools.check("grok").await.unwrap();
    h.write("newest-grok", "1.0.100");
    h.write("models-grok", "grok-5");
    h.tools.update("grok", UpdateBy::Owner).unwrap();
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated);
    let grok = h.info("grok");
    assert_eq!(grok.installation.version.as_deref(), Some("1.0.100"));
    assert_eq!(grok.auth.state, AuthState::Subscription);
    let reported = grok.reported_models.unwrap();
    assert!(reported.models.iter().any(|m| m.name == "grok-5"));
    assert!(!grok
        .capabilities
        .known_models
        .iter()
        .any(|m| m.name == "grok-5"));
    let changed = h.events("ai_tool.models_changed");
    assert_eq!(changed[0]["added"][0], "grok-5");
    let updated = h.events("ai_tool.updated");
    assert_eq!(updated[0]["from"], "1.0.99");
    assert_eq!(updated[0]["to"], "1.0.100");
    // Asked with no task: no conversation.
    assert!(h.rt.overview().await.unwrap().sessions.is_empty());
    // A tool installed another way says what to type.
    h.write("newest-codex", "0.200.0");
    h.write("update-codex", "by-hand");
    h.tools.look_for_new_versions(UpdateBy::Owner).await;
    h.tools.update("codex", UpdateBy::Owner).unwrap();
    let done = h.tools.settled("codex", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::ByHand, "{done:?}");
    assert!(done
        .message
        .unwrap()
        .contains("npm install -g @openai/codex"));
}

#[tokio::test]
async fn new_versions_come_from_each_tools_own_check_or_its_makers_list() {
    let h = harness("subscription").await;
    h.write("newest-grok", "1.0.100");
    let page = h.tools.look_for_new_versions(UpdateBy::Owner).await;
    let newest = |id: &str| {
        page.tools
            .iter()
            .find(|t| t.runtime_id == id)
            .unwrap()
            .newest
            .clone()
    };
    assert_eq!(newest("grok").as_deref(), Some("1.0.100"));
    assert_eq!(newest("claude-code").as_deref(), Some("9.9.9"));
    assert_eq!(newest("codex").as_deref(), Some("9.9.9"));
    assert_eq!(newest("ollama").as_deref(), Some("9.9.9"));
    assert_eq!(newest("copilot").as_deref(), Some("9.9.9"));
    assert_eq!(newest("kimi"), None);
    let asked = h.releases.asked.lock().unwrap().clone();
    assert!(asked.contains(&"/registry.npmjs.org/@anthropic-ai/claude-code/latest".to_owned()));
    assert!(asked.contains(&"/registry.npmjs.org/@openai/codex/latest".to_owned()));
    assert!(asked.contains(&"/registry.npmjs.org/@github/copilot/latest".to_owned()));
    assert!(asked.contains(&"/api.github.com/repos/ollama/ollama/releases/latest".to_owned()));
    assert_eq!(asked.len(), 4);
    // Told once per version; nothing is installed while it asks first.
    let told = h.events("ai_tool.update_available");
    assert_eq!(told.len(), 5);
    h.tools.look_for_new_versions(UpdateBy::Owner).await;
    assert_eq!(h.events("ai_tool.update_available").len(), 5);
    assert_eq!(h.read("update-log"), None);
    assert!(!h.tools.page().auto_update);

    // With the switch on, a tool with a new version is updated by itself.
    h.tools.set_auto_update(true).unwrap();
    assert_eq!(h.events("ai_tools.auto_update_switched")[0]["on"], true);
    h.write("newest-grok", "1.0.101");
    h.tools.look_for_new_versions(UpdateBy::Automatic).await;
    let done = h.tools.settled("grok", WAIT).await.unwrap();
    assert_eq!(done.state, AiToolUpdateState::Updated);
    assert!(done.automatic);
}

#[tokio::test]
async fn with_the_switch_on_the_owner_still_hears_what_plenipo_cannot_update() {
    let h = harness("subscription").await;
    h.tools.set_auto_update(true).unwrap();
    h.tools.look_for_new_versions(UpdateBy::Automatic).await;
    let told = h.events("ai_tool.update_available");
    let automatic = |id: &str| {
        told.iter()
            .find(|e| e["runtime"] == id)
            .map(|e| e["automatic"].clone())
    };
    // Ollama updates from its own tray app: the owner is told, as with the switch off.
    assert_eq!(automatic("ollama"), Some(serde_json::json!(false)));
    assert_eq!(
        automatic("grok"),
        None,
        "Grok's own check has nothing newer"
    );
    assert_eq!(automatic("codex"), Some(serde_json::json!(true)));
    // Codex, Claude Code, and GitHub Copilot, installed here another way, cannot reach 9.9.9 by
    // themselves: the owner is told what to type, once.
    for id in ["codex", "claude-code", "copilot"] {
        let done = h.tools.settled(id, WAIT).await.unwrap();
        assert_eq!(done.state, AiToolUpdateState::ByHand, "{id}: {done:?}");
    }
    let by_hand = h.events("ai_tool.update_by_hand");
    assert_eq!(by_hand.len(), 3, "{by_hand:?}");
    assert!(by_hand.iter().all(|e| e["automatic"] == true));
    // The next day's look does not try the same version again (Kimi, which has no list of its
    // versions, asks its own upgrade once a day).
    let runs = || {
        h.read("update-log")
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("codex ") || l.starts_with("claude "))
            .count()
    };
    let before = runs();
    assert_eq!(before, 2);
    h.tools.look_for_new_versions(UpdateBy::Automatic).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(runs(), before);
    assert_eq!(h.events("ai_tool.update_by_hand").len(), 3);
}

// ---- Usage and payment (ADR-060) ------------------------------------------------------------------

#[tokio::test]
async fn usage_totals_match_the_saved_turns() {
    let h = harness("subscription").await;
    let start = plenipo_ledger::now_ms() - 60_000;
    for objective in ["one", "two"] {
        let id =
            h.rt.start_session("codex", objective, None)
                .await
                .unwrap()
                .session
                .id;
        until("the task to finish", || {
            h.rt.tasks_using("codex").is_empty()
        })
        .await;
        let _ = id;
    }
    until("both steps saved", || {
        h.ledger
            .token_steps("codex", start, start + 3_600_000)
            .unwrap()
            .iter()
            .filter(|s| s.read.is_some())
            .count()
            == 2
    })
    .await;
    let usage = h.tools.usage("codex", &[start, start + 3_600_000]).unwrap();
    let day = &usage.days[0];
    let total: (u64, u64, u64, u32) = day.models.iter().fold((0, 0, 0, 0), |t, m| {
        (t.0 + m.read, t.1 + m.reused, t.2 + m.written, t.3 + m.tasks)
    });
    // The fake Codex reports 20 read (8 reused) and 9 written per step.
    assert_eq!(total, (40, 16, 18, 2));
    // Nothing on another tool, and the days asked for are checked.
    assert!(h
        .tools
        .usage("claude-code", &[start, start + 3_600_000])
        .unwrap()
        .days[0]
        .models
        .is_empty());
    assert!(h.tools.usage("codex", &[start]).is_err());
    assert!(h.tools.usage("codex", &[start, start]).is_err());
    assert!(h.tools.usage("nope", &[start, start + 1]).is_err());
}

#[tokio::test]
async fn a_task_that_runs_past_midnight_on_two_models_is_counted_once() {
    let h = harness("subscription").await;
    let task = h
        .ledger
        .create_task(
            plenipo_ledger::NewTask {
                parent_task_id: None,
                requested_by: "owner".into(),
                assigned_to: None,
                project_id: None,
                objective: "Late night".into(),
                acceptance_criteria: String::new(),
                priority: 3,
                metadata: serde_json::Value::Null,
            },
            "owner",
        )
        .unwrap();
    let day = 86_400_000;
    let midnight = 20_000 * day;
    let counts = serde_json::json!({ "usage": { "inputTokens": 10, "cachedInputTokens": 4,
                                                "outputTokens": 5 } });
    for (id, model, at) in [
        ("step-1", "gpt-6-sol", midnight - 600_000),
        ("step-2", "gpt-5.5", midnight + 600_000),
    ] {
        h.ledger
            .upsert_execution(
                &plenipo_ledger::ExecutionRow {
                    id: id.into(),
                    task_id: Some(task.id.clone()),
                    runtime: "codex".into(),
                    provider: None,
                    model: Some(model.into()),
                    session_id: None,
                    process_id: None,
                    profile_id: None,
                    label: "turn".into(),
                    executable: None,
                    args: vec![],
                    working_dir: None,
                    state: "succeeded".into(),
                    exit_code: Some(0),
                    detail: None,
                    started_at: at,
                    ended_at: Some(at + 1),
                    usage_metadata: counts.clone(),
                },
                "test",
            )
            .unwrap();
    }
    let usage = h
        .tools
        .usage("codex", &[midnight - day, midnight, midnight + day])
        .unwrap();
    let all: Vec<_> = usage.days.iter().flat_map(|d| d.models.iter()).collect();
    // Both steps' tokens count, and the task once: on its first day, under its first model.
    assert_eq!(all.iter().map(|m| m.read).sum::<u64>(), 20);
    assert_eq!(all.iter().map(|m| m.steps).sum::<u32>(), 2);
    assert_eq!(all.iter().map(|m| m.tasks).sum::<u32>(), 1);
    assert_eq!(usage.days[0].models[0].tasks, 1);
    assert_eq!(usage.days[0].models[0].model.as_deref(), Some("gpt-6-sol"));
    // A "day" longer than 25 hours is refused.
    assert!(h.tools.usage("codex", &[0, 9_000_000_000_000_000]).is_err());
}

#[tokio::test]
async fn the_payment_switch_cannot_be_turned_to_a_paid_key_and_plans_come_only_as_reported() {
    let h = harness("subscription").await;
    // Every AI tool that signs in with a subscription uses it; a paid AI tool (OpenRouter,
    // ADR-085) uses its key and has none saved yet.
    let page = h.tools.page();
    assert!(page
        .tools
        .iter()
        .filter(|t| t.runtime_id != "openrouter")
        .all(|t| t.payment == PaymentMethod::Subscription));
    let openrouter = page
        .tools
        .iter()
        .find(|t| t.runtime_id == "openrouter")
        .unwrap();
    assert_eq!(openrouter.payment, PaymentMethod::PaidKey);
    assert!(openrouter.paid_key.is_none());
    assert!(openrouter
        .paid_blocked
        .as_deref()
        .unwrap()
        .contains("switched off"));
    assert!(h
        .tools
        .set_payment("openrouter", PaymentMethod::Subscription)
        .is_err());
    let err = h
        .tools
        .set_payment("codex", PaymentMethod::PaidKey)
        .unwrap_err();
    assert!(err.to_string().contains("spending caps"), "{err}");
    assert!(h
        .tools
        .set_payment("codex", PaymentMethod::Subscription)
        .is_ok());
    // Codex reports its plan through its app server; Grok reports none.
    h.tools.check("codex").await.unwrap();
    let page = h.tools.page();
    let codex = page.tools.iter().find(|t| t.runtime_id == "codex").unwrap();
    let plan = codex.plan.as_ref().unwrap();
    assert_eq!(plan.windows[0].used_percent, Some(25));
    assert_eq!(plan.plan.as_deref(), Some("plus"));
    let grok = page.tools.iter().find(|t| t.runtime_id == "grok").unwrap();
    assert!(!grok.reports_plan_left && grok.plan.is_none());
    assert!(codex.reports_plan_left);
    // GitHub Copilot (ADR-083): no paid key either, and its plan left comes from its own check
    // (chat only: code suggestions are not Plenipo's, and nothing is included in premium requests).
    let err = h
        .tools
        .set_payment("copilot", PaymentMethod::PaidKey)
        .unwrap_err();
    assert!(err.to_string().contains("spending caps"), "{err}");
    h.tools.check("copilot").await.unwrap();
    let page = h.tools.page();
    let copilot = page
        .tools
        .iter()
        .find(|t| t.runtime_id == "copilot")
        .unwrap();
    assert_eq!(copilot.payment, PaymentMethod::Subscription);
    assert!(copilot.reports_plan_left);
    let plan = copilot.plan.as_ref().unwrap();
    assert_eq!(plan.windows.len(), 1);
    assert_eq!(plan.windows[0].used_percent, Some(25));
}

#[tokio::test]
async fn what_plenipo_keeps_but_cannot_read_is_never_written_over() {
    let h = harness("subscription").await;
    // Kept by a version of Plenipo that wrote it differently: this one cannot read it.
    let odd =
        serde_json::json!({ "autoUpdate": "yes", "tools": { "grok": { "outOfService": 7 } } });
    h.ledger
        .change_setting("ai_tools", |_| Ok(odd.clone()))
        .unwrap();
    // A plan report arrives: it is not kept, and what was there stays as it was.
    h.tools.plan_reported(
        "claude-code",
        PlanReport {
            windows: vec![],
            limited: false,
            warning: false,
            plan: None,
            reported_at: 1,
        },
    );
    assert_eq!(h.ledger.setting("ai_tools").unwrap().unwrap(), odd);
    // Nor does the switch write over it; it says it could not.
    assert!(h.tools.set_auto_update(true).is_err());
    assert_eq!(h.ledger.setting("ai_tools").unwrap().unwrap(), odd);
}

/// A paid key (ADR-085) is saved only with the switch on and the business's cap set, checked
/// once, kept only in the Vault, hidden everywhere, and removed from the Vault when removed. A
/// key the service refuses changes nothing.
#[tokio::test]
async fn a_paid_key_is_kept_only_in_the_vault_and_only_with_the_switch_and_the_business_cap() {
    let h = harness("subscription").await;
    let key = "sk-or-v1-test-key-not-real-abcdef012345";
    let card = |page: &plenipo_capabilities::ai_tools::AiToolsPage| {
        page.tools
            .iter()
            .find(|t| t.runtime_id == "openrouter")
            .cloned()
            .unwrap()
    };
    // Paid keys switched off: refused, and nothing is kept.
    let err = h
        .tools
        .save_paid_key("openrouter", "Office key", key)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("switched off"), "{err}");
    h.broker
        .guard()
        .set_switches(&plenipo_guard::dto::Switches {
            paid_ai_keys: true,
            ..plenipo_guard::dto::Switches::default()
        })
        .unwrap();
    // No business cap yet: refused.
    let err = h
        .tools
        .save_paid_key("openrouter", "Office key", key)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("business's monthly spending cap"),
        "{err}"
    );
    h.ledger
        .set_spending_cap(
            &plenipo_ledger::CapCovers::Business,
            50_000_000,
            "owner",
            plenipo_ledger::now_ms(),
        )
        .unwrap();
    // A key the service refuses: nothing changes.
    std::fs::write(h.state.join("auth"), "signed-out").unwrap();
    let err = h
        .tools
        .save_paid_key("openrouter", "Office key", key)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("did not accept the key"), "{err}");
    assert!(!err.to_string().contains(key));
    assert!(card(&h.tools.page()).paid_key.is_none());
    assert!(h.broker.guard().config().unwrap().paid_keys.is_empty());
    // Accepted: saved by name, and OpenRouter is ready.
    std::fs::write(h.state.join("auth"), "subscription").unwrap();
    let page = h
        .tools
        .save_paid_key("openrouter", "Office key", key)
        .await
        .unwrap();
    let saved = card(&page);
    assert_eq!(saved.paid_key.as_ref().unwrap().name, "Office key");
    assert!(saved.paid_blocked.is_none());
    let info =
        h.rt.runtimes()
            .into_iter()
            .find(|r| r.id == "openrouter")
            .unwrap();
    assert!(info.ready, "{info:#?}");
    // The key is in the Vault only: never in the settings, the Ledger, the page, or a log line.
    let config = h.broker.guard().config().unwrap();
    let stored = saved.paid_key.clone().unwrap().id;
    assert!(plenipo_capabilities::vault::stored_ids(&config).contains(&stored));
    assert_eq!(config.paid_keys.len(), 1);
    assert_eq!(
        plenipo_capabilities::vault::read(h.broker.secret_store().as_ref(), &stored)
            .unwrap()
            .as_deref(),
        Some(key)
    );
    let guard_setting = h.ledger.setting("guard").unwrap().unwrap().to_string();
    assert!(!guard_setting.contains(key));
    let events = serde_json::to_string(&h.ledger.recent_events(1_000).unwrap()).unwrap();
    assert!(!events.contains(key) && !events.contains("abcdef012345"));
    assert!(!format!("{page:?}").contains(key));
    let filter = h.broker.text_filter();
    assert!(!filter(&format!("the service said {key}")).contains(key));
    // While a key is saved, the business's cap stays (ADR-085 §2.4).
    assert!(plenipo_capabilities::paid::any_key(&h.broker));
    // Removed: gone from the Vault too, and OpenRouter is not ready.
    let page = h.tools.remove_paid_key("openrouter").await.unwrap();
    assert!(card(&page).paid_key.is_none());
    assert!(
        plenipo_capabilities::vault::read(h.broker.secret_store().as_ref(), &stored)
            .unwrap()
            .is_none()
    );
    assert!(!plenipo_capabilities::paid::any_key(&h.broker));
    let info =
        h.rt.runtimes()
            .into_iter()
            .find(|r| r.id == "openrouter")
            .unwrap();
    assert!(!info.ready);
}
