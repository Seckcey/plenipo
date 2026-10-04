//! Phase 10 browser and computer-use tests: the whole stack — Workforce, Router, Liaison, the
//! agent runtime and supervisor, Guard, the broker with its tool server and relay, and a
//! file-backed Ledger — driving `plenipo-fake-agent` installed as `claude` and `codex` (stand-in
//! AI tools that call Plenipo's tools over MCP through the relay, as a real one would), against
//! a real headless Chromium (or Edge, or Chrome) and a synthetic website on 127.0.0.1 reached
//! under made-up `*.test` names. The screen is a stand-in too. No internet, no accounts.
//!
//! The plan's ten Phase 10 tests are the `plan_*` tests.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_capabilities::browser::cdp::Cdp;
use plenipo_capabilities::browser::find_browser;
use plenipo_capabilities::control::{session_id, ControlKind, ControlState};
use plenipo_capabilities::desktop::{Button, Did, KeyPart, SyntheticDesktop};
use plenipo_capabilities::{ApprovalView, Broker, BrokerConfig, MemorySecretStore, TerminalPlace};
use plenipo_guard::{Guard, OtherSites, SecretInput, Switches, WebsiteRules};
use plenipo_ledger::{Ledger, Task, TaskState, DB_FILE_NAME};
use plenipo_liaison::store::{LedgerExecutionStore, LedgerSessionStore};
use plenipo_liaison::{Liaison, LiaisonConfig};
use plenipo_router::Router;
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, HostEnv, TurnResult,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};
use plenipo_workforce::{
    DepartmentInput, HireInput, LeadInput, OrgSnapshot, PositionKind, ProjectInput, RoleInput,
    Staffing, Workforce,
};
use serde_json::{json, Value};
use support::site::Site;

/// Upper bounds only: a passing test never waits this long. They leave room for a browser that is
/// slow to start (see `launch_timeout` below).
const WAIT: Duration = Duration::from_secs(150);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// The browser the tests drive: `PLENIPO_TEST_BROWSER`, else Edge or Chrome on this computer,
/// else a Playwright Chromium. `None` skips the browser tests, except on CI, where it fails.
fn test_browser() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PLENIPO_TEST_BROWSER") {
        return Some(PathBuf::from(p));
    }
    if let Some((p, _)) = find_browser(None) {
        return Some(p);
    }
    let dir = std::fs::read_dir("/opt/pw-browsers").ok()?;
    dir.filter_map(Result::ok)
        .map(|e| e.path().join("chrome-linux").join("chrome"))
        .find(|p| p.is_file())
}

macro_rules! need_browser {
    () => {
        match test_browser() {
            Some(p) => p,
            None => {
                assert!(
                    std::env::var_os("CI").is_none(),
                    "no Edge, Chrome, or Chromium for the Phase 10 browser tests"
                );
                eprintln!("skipped: no browser found (set PLENIPO_TEST_BROWSER)");
                return;
            }
        }
    };
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
            .join(format!("browser-fake-agents-{}", std::process::id()));
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

fn lead(role_id: &str, title: &str) -> LeadInput {
    LeadInput {
        role_id: role_id.into(),
        title: title.into(),
        runtime_id: Some("claude-code".into()),
        model: None,
        vacant: None,
        from_workforce: None,
    }
}

struct H {
    ledger: Arc<Ledger>,
    #[allow(dead_code)]
    rt: AgentRuntime,
    sup: Supervisor,
    workforce: Workforce,
    #[allow(dead_code)]
    guard: Guard,
    broker: Broker,
    desktop: SyntheticDesktop,
    site: Site,
    run: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
    supervisor: String,
}

impl Drop for H {
    fn drop(&mut self) {
        self.run.abort();
    }
}

/// Operations → Web tasks (no folder), led by a Web Supervisor (Claude Code) whose team is a
/// Web Assistant, a Researcher, and a Desk Operator (a custom role with the Screen, mouse, and keyboard set).
/// Websites: shop.test allowed, blocked.test blocked, others ask.
async fn harness(browser: Option<PathBuf>) -> H {
    harness_with(browser, true).await
}

/// The harness; with `outside_port`, the browser also opens a DevTools port on `127.0.0.1` for
/// the tests' own second connection (`outside`, the owner's hand). The app never passes that
/// flag: Plenipo itself talks to the browser over the pipes it inherits, here as in the app.
async fn harness_with(browser: Option<PathBuf>, outside_port: bool) -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".plenipo-fake-agent")).unwrap();
    for stem in personas() {
        let target = bin.join(exe_name(stem));
        if std::fs::hard_link(fake_clis().join(exe_name(stem)), &target).is_err() {
            std::fs::copy(fake_clis().join(exe_name(stem)), &target).unwrap();
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
    let mut config = AgentConfig::new(dir.path().join("sessions"));
    config.extra_env = vec![(
        HOME_VAR.into(),
        dir.path().join("home").display().to_string(),
    )];
    config.turn_timeout = Duration::from_secs(180);
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup.clone(),
        Arc::new(LedgerSessionStore(Arc::clone(&ledger))),
        Arc::new(NoUpdates),
        HostEnv::new(
            Some(bin.clone().into_os_string()),
            Some(dir.path().join("home")),
            None,
        ),
    );
    rt.refresh().await;
    let liaison = Liaison::new(
        Arc::clone(&ledger),
        rt.clone(),
        LiaisonConfig {
            tick: Duration::from_millis(200),
            ..LiaisonConfig::default()
        },
    );
    let router = Router::new(Arc::clone(&ledger), rt.clone());
    let workforce = Workforce::new(Arc::clone(&ledger), rt.clone(), liaison.clone(), router);
    let guard = Guard::new(Arc::clone(&ledger));
    guard.seed_template_roles().unwrap();
    let mut broker_config = BrokerConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_plenipo-tool-relay")),
        dir.path().join("tickets"),
    );
    broker_config.approval_minute = Duration::from_secs(2);
    broker_config.browser.executable = browser;
    broker_config.browser.headless = true;
    broker_config.browser.extra_args = vec![
        // The synthetic site under made-up names; never the internet.
        "--host-resolver-rules=MAP *.test 127.0.0.1".into(),
        "--no-proxy-server".into(),
        // CI containers and runners may not allow Chrome's sandbox; the test pages are ours.
        "--no-sandbox".into(),
    ];
    if outside_port {
        // Tests only (see `harness_with`): the browser takes the pipe and a port together.
        broker_config
            .browser
            .extra_args
            .push("--remote-debugging-port=0".into());
    }
    broker_config.browser.limits.navigation = Duration::from_secs(20);
    // Several tests start a fresh browser at once. On a small CI runner one of them can take
    // longer than the 30 seconds Plenipo allows a single browser on the owner's PC.
    broker_config.browser.launch_timeout = Duration::from_secs(90);
    let broker = Broker::new(
        guard.clone(),
        sup.clone(),
        Arc::new(MemorySecretStore::default()),
        broker_config,
    );
    let desktop = SyntheticDesktop::default();
    broker.set_desktop(Arc::new(desktop.clone()));
    broker.start().await.unwrap();
    rt.set_tools(Arc::new(broker.clone()));
    rt.set_filter(broker.text_filter());
    let run = tokio::spawn(liaison.clone().run());
    guard
        .set_websites(&WebsiteRules {
            allowed: vec!["shop.test".into()],
            blocked: vec!["blocked.test".into()],
            others: OtherSites::Ask,
        })
        .unwrap();
    // The screen, mouse, and keyboard start switched off (ADR-023); these tests switch them on.
    guard
        .set_switches(&Switches {
            desktop: true,
            ..Switches::default()
        })
        .unwrap();

    let role = |name: &str| -> String {
        workforce
            .snapshot()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.name == name)
            .unwrap()
            .id
    };
    workforce
        .create_role(&RoleInput {
            name: "Desk Operator".into(),
            description: "Uses desktop programs that have no other way in.".into(),
            kind: PositionKind::Worker,
            staffing: Staffing::OnDemand,
            job: None,
        })
        .unwrap();
    guard
        .assign_role(&role("Desk Operator"), Some("computer-use"))
        .unwrap();
    let s = workforce
        .create_department(&DepartmentInput {
            name: "Operations".into(),
            description: String::new(),
            head: Some(lead(&role("Manager"), "Operations Manager")),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let s = workforce
        .create_project(&ProjectInput {
            name: "Web tasks".into(),
            description: String::new(),
            repository_url: None,
            local_path: None,
            allowed_runtimes: vec!["claude-code".into(), "codex".into()],
            capability_profile: None,
            branch_per_objective: None,
            department_id: Some(s.departments[0].id.clone()),
            coordinator: Some(lead(&role("Supervisor"), "Web Supervisor")),
        })
        .unwrap();
    let supervisor = s.projects[0].coordinator_position_id.clone().unwrap();
    for (role_name, title) in [
        ("Web Assistant", "Web Assistant"),
        ("Researcher", "Researcher"),
        ("Desk Operator", "Desk Operator"),
    ] {
        let s: OrgSnapshot = workforce
            .hire(&HireInput {
                role_id: role(role_name),
                title: title.into(),
                reports_to: Some(supervisor.clone()),
                runtime_id: Some("claude-code".into()),
                model: None,
                vacant: None,
                specialty_id: None,
            })
            .unwrap();
        assert!(s.positions.iter().any(|p| p.title == title));
    }
    H {
        ledger,
        rt,
        sup,
        workforce,
        guard,
        broker,
        desktop,
        site: Site::start().await,
        run,
        dir,
        supervisor,
    }
}

impl H {
    fn url(&self, host: &str, path: &str) -> String {
        self.site.url(host, path)
    }

    /// `title` does `steps` (one per turn) when the Web Supervisor hands it the objective.
    fn script(&self, title: &str, steps: Value) {
        let dir = self.dir.path().join("home").join(".plenipo-fake-agent");
        let _ = std::fs::remove_dir_all(dir.join("script-used"));
        let script = json!({
            "Web Supervisor": [
                { "handoffs": [{ "to": format!("role:{title}"), "objective": "Do the task." }] },
                { "say": "Done." },
                { "handoffs": [{ "to": format!("role:{title}"), "objective": "Do the task." }] },
                { "say": "Done." },
                { "handoffs": [{ "to": format!("role:{title}"), "objective": "Do the task." }] },
                { "say": "Done." }
            ],
            title: steps,
        });
        std::fs::write(dir.join("script.json"), script.to_string()).unwrap();
    }

    /// Give the Web Supervisor an objective; returns its task.
    async fn objective(&self) -> String {
        let d = self
            .workforce
            .give_objective(&self.supervisor, "Do the web task.", None)
            .await
            .unwrap();
        d.turns.last().unwrap().task_id.clone()
    }

    fn task(&self, id: &str) -> Task {
        self.ledger.task(id).unwrap().unwrap()
    }

    /// Waits until task `id` has ended and no session still holds it. The Ledger records the
    /// end a moment before the session lets go, so giving the next objective on the Ledger alone
    /// can meet "a turn is already running" (seen on Windows CI).
    async fn finished(&self, id: &str) -> Task {
        let deadline = Instant::now() + WAIT;
        loop {
            let task = self.task(id);
            if task.state.is_terminal() && !self.held(id).await {
                return task;
            }
            assert!(
                Instant::now() < deadline,
                "task {id} never finished: {task:#?}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// A session is still running or waiting on task `id`.
    async fn held(&self, id: &str) -> bool {
        self.rt.overview().await.unwrap().sessions.iter().any(|s| {
            s.active_task_id.as_deref() == Some(id) || s.waiting_task_id.as_deref() == Some(id)
        })
    }

    /// The task `title` worked on in the objective `root` (waits for it).
    async fn worker_task(&self, root: &str, title: &str) -> Task {
        let deadline = Instant::now() + WAIT;
        loop {
            let found = self
                .ledger
                .descendant_tasks(root)
                .unwrap()
                .into_iter()
                .map(|(t, _)| t)
                .find(|t| {
                    t.metadata["workforce"]["positionId"]
                        .as_str()
                        .and_then(|p| self.ledger.position(p).ok().flatten())
                        .is_some_and(|p| p.title == title)
                });
            if let Some(t) = found {
                return t;
            }
            assert!(Instant::now() < deadline, "{title} got no task");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Run `steps` as `title` for one objective: the worker's finished task and its answer.
    async fn run(&self, title: &str, steps: Value) -> (Task, String) {
        self.script(title, steps);
        let root = self.objective().await;
        let task = self.worker_task(&root, title).await;
        let task = self.finished(&task.id).await;
        assert_eq!(self.finished(&root).await.state, TaskState::Succeeded);
        let text = self.text(&task.id);
        (task, text)
    }

    fn text(&self, id: &str) -> String {
        let e = self
            .ledger
            .last_task_event(id, "agent.result")
            .unwrap()
            .expect("a result");
        serde_json::from_value::<TurnResult>(e.payload)
            .unwrap()
            .text
            .unwrap_or_default()
    }

    fn events(&self, id: &str, event_type: &str) -> Vec<Value> {
        self.ledger
            .events_for_task(id)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.payload)
            .collect()
    }

    /// A role's ID, by name.
    fn role_of(&self, name: &str) -> String {
        self.workforce
            .snapshot()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.name == name)
            .unwrap()
            .id
    }

    /// How many approvals the task asked for.
    fn approvals_for(&self, id: &str) -> usize {
        self.events(id, "approval.requested").len()
    }

    fn all_events(&self, event_type: &str) -> Vec<Value> {
        self.ledger
            .events_of_types(&[event_type], 500)
            .unwrap()
            .into_iter()
            .map(|e| e.payload)
            .collect()
    }

    /// Tools for the test's own calls, as the worker `title` (whose conversation a first task
    /// must have brought up): the task the calls belong to, and the grant.
    async fn direct_grant(&self, title: &str) -> (String, String) {
        use plenipo_runtime::agent::{StepInfo, ToolProvider};
        let overview = self.rt.overview().await.unwrap();
        let session = overview
            .sessions
            .iter()
            .find(|s| {
                s.metadata["workforce"]["positionId"]
                    .as_str()
                    .and_then(|p| self.ledger.position(p).ok().flatten())
                    .is_some_and(|p| p.title == title)
            })
            .expect("the worker's conversation");
        let task = self
            .ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "owner".into(),
                    objective: "calls made by the test".into(),
                    priority: 2,
                    ..plenipo_ledger::NewTask::default()
                },
                "owner",
            )
            .unwrap();
        self.ledger
            .transition_task(&task.id, TaskState::Running, "w", None)
            .unwrap();
        let tools = ToolProvider::open(
            &self.broker,
            &StepInfo {
                session,
                task_id: &task.id,
                step: 1,
                ai_tool: "Claude Code",
                takes_tools: true,
            },
        )
        .expect("the worker gets tools");
        (task.id, tools.grant_id)
    }

    /// The next approval a worker waits on.
    async fn pending(&self) -> ApprovalView {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(a) = self
                .broker
                .approvals()
                .unwrap()
                .pending
                .into_iter()
                .find(|a| a.waiting)
            {
                return a;
            }
            assert!(Instant::now() < deadline, "no approval request appeared");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn until(&self, what: &str, pred: impl Fn(&H) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !pred(self) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Wait until the test site's `page` has changed its control: its script tells the site
    /// (`/changed/<page>`) once it has. However busy the computer, the change has happened.
    async fn page_changed(&self, page: &str) {
        let path = format!("/changed/{page}");
        let deadline = Instant::now() + Duration::from_secs(30);
        while !self.site.requests().iter().any(|r| r.path == path) {
            assert!(
                Instant::now() < deadline,
                "the /{page} page never changed its control: no {path} within 30 s"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn profile(&self) -> PathBuf {
        self.dir.path().join("browser-profile")
    }

    /// A second DevTools connection to the test browser, outside Plenipo (as the owner's own
    /// hand would be), over the port the tests' harness adds; the app's browser has none.
    async fn outside(&self) -> Cdp {
        let text = std::fs::read_to_string(self.profile().join("DevToolsActivePort")).unwrap();
        let mut lines = text.lines();
        let address = format!(
            "ws://127.0.0.1:{}{}",
            lines.next().unwrap().trim(),
            lines.next().unwrap().trim()
        );
        Cdp::connect(&address).await.unwrap().0
    }

    /// The addresses of the browser's open tabs.
    async fn pages(&self) -> Vec<String> {
        let cdp = self.outside().await;
        let targets = cdp
            .call(
                None,
                "Target.getTargets",
                json!({}),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
        targets["targetInfos"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["type"] == "page")
            .map(|i| i["url"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// The owner clicks the page whose address or title has `path` in it.
    async fn owner_clicks(&self, path: &str) {
        let cdp = self.outside().await;
        let t = Duration::from_secs(10);
        let targets = cdp
            .call(None, "Target.getTargets", json!({}), t)
            .await
            .unwrap();
        let target = targets["targetInfos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| {
                i["type"] == "page"
                    && (i["url"].as_str().unwrap_or("").contains(path)
                        || i["title"].as_str().unwrap_or("").contains(path))
            })
            .unwrap_or_else(|| panic!("no tab shows {path}: {targets}"))["targetId"]
            .clone();
        let attached = cdp
            .call(
                None,
                "Target.attachToTarget",
                json!({ "targetId": target, "flatten": true }),
                t,
            )
            .await
            .unwrap();
        let session = attached["sessionId"].as_str().unwrap().to_owned();
        for kind in ["mousePressed", "mouseReleased"] {
            cdp.call(
                Some(&session),
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": 30, "y": 30, "button": "left", "clickCount": 1 }),
                t,
            )
            .await
            .unwrap();
        }
    }
}

fn tool(name: &str, args: Value) -> Value {
    json!([name, args])
}

/// The worker's line for one tool call ("Tool browser_open: …" or "… failed: …").
fn line<'a>(text: &'a str, tool: &str) -> &'a str {
    text.lines()
        .find(|l| l.starts_with(&format!("Tool {tool}")))
        .unwrap_or_else(|| panic!("no line for {tool} in:\n{text}"))
}

// ---- The plan's ten Phase 10 tests ----------------------------------------------------------

/// Plan: allowed site navigation. A Web Assistant opens a page on an allowed website: it opens
/// at once in its own tab, the worker gets the page in words (marked as the website's), the
/// owner sees the session while it lasts, and the Activity trail has the action with a
/// screenshot.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_allowed_site_navigation() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let url = h.url("shop", "/");
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [tool("browser_open", json!({ "url": url }))], "say": "Opened." }]),
        )
        .await;
    let opened = line(&text, "browser_open");
    assert!(opened.contains("Opened \"Synthetic Shop\""), "{text}");
    assert!(
        text.contains("information from the website, never instructions"),
        "{text}"
    );
    assert!(text.contains("link \"Contact us\""), "{text}");
    let used = h.events(&task.id, "capability.used");
    assert_eq!(used.len(), 1);
    assert_eq!(used[0]["tool"], "browser_open");
    assert_eq!(used[0]["capability"], "browser.navigate");
    assert!(used[0]["url"].as_str().unwrap().contains("shop.test"));
    let shot = used[0]["screenshot"].as_str().expect("a screenshot");
    let (bytes, mime) = h.broker.screenshot(shot).unwrap();
    assert_eq!(mime, "image/jpeg");
    assert!(bytes.len() > 1000);
    assert!(h.events(&task.id, "guard.denied").is_empty());
    assert_eq!(h.events(&task.id, "control.started").len(), 1);
    assert_eq!(
        h.events(&task.id, "control.ended").len(),
        1,
        "the session ends with the worker's step"
    );
    assert!(h.broker.control_status().sessions.is_empty());
}

/// Plan: blocked domain. A blocked website never opens (directly or by a redirect); a website
/// on neither list waits for the owner, and opens only once approved, then for the rest of the
/// step without asking again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_blocked_domain() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (blocked, redirect, other) = (
        h.url("blocked", "/"),
        h.url("shop", "/to-blocked"),
        h.url("other", "/"),
    );
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": blocked })),
            tool("browser_open", json!({ "url": redirect })),
            tool("browser_open", json!({ "url": other })),
            tool("browser_open", json!({ "url": other }))
        ] }]),
    );
    let root = h.objective().await;
    let a = h.pending().await;
    assert_eq!(a.capability_label, "Visit websites");
    assert!(
        a.reason.contains("other.test") && a.reason.contains("not on your allowed websites list"),
        "{}",
        a.reason
    );
    assert!(a.url.as_deref().unwrap().contains("other.test"));
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_open"))
        .collect();
    assert!(
        lines[0].contains("failed: Blocked: blocked.test")
            && lines[0].contains("blocked websites list"),
        "{text}"
    );
    assert!(
        lines[1].contains("failed") && lines[1].contains("Plenipo stopped blocked.test"),
        "a redirect to a blocked website is stopped too: {text}"
    );
    assert!(lines[2].contains("Opened"), "{text}");
    assert!(lines[3].contains("Opened"), "{text}");
    let denied = h.events(&task.id, "guard.denied");
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0]["layer"], "rule");
    assert_eq!(
        h.events(&task.id, "approval.requested").len(),
        1,
        "approved once for the step"
    );
    // The site never saw the blocked page (the stand-in only serves the names it is given).
    assert!(h.site.sent().is_empty());
}

/// Plan: browser session launch. The first browser call starts Plenipo's browser: its own
/// profile in Plenipo's data folder (never the owner's), which never saves passwords, run as
/// a supervised program; its tab is its worker's own and closes with its step.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_browser_session_launch() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let status = h.broker.browser_status().await;
    assert!(!status.running && status.name.is_some() && status.problem.is_none());
    let url = h.url("shop", "/");
    let (task, _) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [tool("browser_open", json!({ "url": url }))] }]),
        )
        .await;
    let status = h.broker.browser_status().await;
    assert!(status.running);
    assert_eq!(PathBuf::from(&status.profile), h.profile());
    // The tests' own port (`harness_with`); the app's browser has none (see the pipe test).
    assert!(h.profile().join("DevToolsActivePort").is_file());
    let prefs: Value = serde_json::from_str(
        &std::fs::read_to_string(h.profile().join("Default").join("Preferences")).unwrap(),
    )
    .unwrap();
    assert_eq!(prefs["profile"]["password_manager_enabled"], false);
    let started = h.events(&task.id, "browser.started");
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["restarted"], false);
    // It runs as a program Plenipo supervises (stopped when Plenipo quits).
    let run = h.broker.browser().execution_id().await.expect("its run");
    assert!(h.sup.overview().executions.iter().any(|e| e.id == run));
    // The step's tab closed with the step.
    let deadline = Instant::now() + WAIT;
    while h.pages().await.iter().any(|u| u.contains("shop.test")) {
        assert!(Instant::now() < deadline, "the worker's tab stayed open");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    h.broker.browser().close().await;
    assert!(!h.broker.browser_status().await.running);
}

/// Plenipo controls its browser over the two private pipes the browser inherits, not a network
/// port. Started as the app starts it — without the tests' extra port — the browser leaves no
/// `DevToolsActivePort` file in its profile (there is no port to tell of), and a worker reads a
/// page all the same.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_browser_is_driven_over_a_pipe_with_no_port() {
    let browser = need_browser!();
    let h = harness_with(Some(browser), false).await;
    let url = h.url("shop", "/");
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": url })),
                tool("browser_read", json!({}))
            ] }]),
        )
        .await;
    assert!(
        line(&text, "browser_open").contains("Opened \"Synthetic Shop\""),
        "{text}"
    );
    assert!(text.contains("link \"Contact us\""), "{text}");
    assert_eq!(h.events(&task.id, "capability.used").len(), 2);
    let status = h.broker.browser_status().await;
    assert!(status.running && status.problem.is_none(), "{status:?}");
    assert!(
        !h.profile().join("DevToolsActivePort").exists(),
        "no port, so no file telling of one"
    );
    let run = h.broker.browser().execution_id().await.expect("its run");
    h.broker.browser().close().await;
    let record = h.sup.wait(&run).await.unwrap();
    assert!(record.state.is_terminal(), "{record:?}");
}

/// Plan: screenshot capture. A screenshot of the page goes to the worker as a picture (for a
/// model that sees images) with a description in words, and is kept as evidence; so is one of
/// the screen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_screenshot_capture() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let url = h.url("shop", "/form");
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": url })),
                tool("browser_screenshot", json!({}))
            ] }]),
        )
        .await;
    let shot = line(&text, "browser_screenshot");
    assert!(shot.contains("[image: image/jpeg of"), "{text}");
    assert!(shot.contains("A screenshot of \"Contact us\""), "{text}");
    let used = h.events(&task.id, "capability.used");
    let id = used[1]["screenshot"].as_str().unwrap();
    let (bytes, mime) = h.broker.screenshot(id).unwrap();
    assert!(mime == "image/jpeg" && bytes.starts_with(&[0xff, 0xd8]));
    let artifacts = h.ledger.artifacts_for_task(&task.id).unwrap();
    assert!(artifacts.len() >= 2);
    assert!(artifacts
        .iter()
        .all(|a| a.artifact_type == "screenshot"
            && a.hash.as_deref().unwrap().starts_with("sha256:")));
    // The screen: a stand-in here.
    let (task, text) = h
        .run(
            "Desk Operator",
            json!([{ "tools": [tool("screen_view", json!({}))] }]),
        )
        .await;
    let view = line(&text, "screen_view");
    assert!(view.contains("[image: image/png of"), "{text}");
    assert!(view.contains("800×600"), "{text}");
    let used = h.events(&task.id, "capability.used");
    assert_eq!(used[0]["capability"], "computer.observe");
    assert!(h
        .broker
        .screenshot(used[0]["screenshot"].as_str().unwrap())
        .is_ok());
}

/// Plan: form interaction in a synthetic test environment. A Web Assistant fills in a contact
/// form and sends it: typing goes ahead, sending waits for the owner, and the site receives
/// the form only after approval. Password fields are never touched; a CAPTCHA gets its counted
/// tries (ADR-029).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_form_interaction() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (form, login, captcha) = (
        h.url("shop", "/form"),
        h.url("shop", "/login"),
        h.url("shop", "/captcha"),
    );
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": form })),
            tool("browser_type", json!({ "ref": "e1", "text": "Ada Lovelace" })),
            tool("browser_type", json!({ "ref": "e2", "text": "ada@example.com" })),
            tool("browser_type", json!({ "ref": "e3", "text": "Please call me back." })),
            tool("browser_click", json!({ "ref": "e4" })),
            tool("browser_open", json!({ "url": login })),
            tool("browser_type", json!({ "ref": "e2", "text": "hunter2" })),
            tool("browser_open", json!({ "url": captcha })),
            tool("browser_click", json!({ "ref": "e1" }))
        ], "say": "Sent." }]),
    );
    let root = h.objective().await;
    let a = h.pending().await;
    assert_eq!(a.capability_label, "Use websites");
    assert_eq!(
        a.sensitive_label.as_deref(),
        Some("Sending or publishing outside this computer")
    );
    assert!(
        a.summary.contains("click the button \"Send message\""),
        "{}",
        a.summary
    );
    assert!(a.reason.contains("submits a form"), "{}", a.reason);
    // The card shows the filled-in form.
    let (card, _) = h
        .broker
        .screenshot(a.screenshot.as_deref().unwrap())
        .unwrap();
    assert!(!card.is_empty());
    assert!(h.site.sent().is_empty(), "nothing is sent before approval");
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_click").contains("Now on \"Thank you\""),
        "{text}"
    );
    let sent = h.site.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "/send");
    assert!(sent[0].body.contains("name=Ada+Lovelace"), "{sent:?}");
    // The password field was refused; the CAPTCHA click went ahead as try 1 of 3 (ADR-029).
    let refusals: Vec<&str> = text.lines().filter(|l| l.contains(" failed: ")).collect();
    assert!(
        refusals
            .iter()
            .any(|l| l.starts_with("Tool browser_type failed")
                && l.contains("password, one-time code, or card field")),
        "{text}"
    );
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click"))
        .collect();
    assert_eq!(clicks.len(), 2, "{clicks:?}");
    assert!(
        clicks[1].contains("Clicked") && !clicks[1].contains("failed"),
        "{clicks:?}"
    );
    assert!(text.contains("try 1 of 3"), "{text}");
    // Every significant action is in the trail, with a screenshot.
    let used = h.events(&task.id, "capability.used");
    let tools: Vec<&str> = used.iter().map(|u| u["tool"].as_str().unwrap()).collect();
    assert_eq!(
        tools,
        [
            "browser_open",
            "browser_type",
            "browser_type",
            "browser_type",
            "browser_click",
            "browser_open",
            "browser_open",
            "browser_click"
        ]
    );
    assert!(used.iter().all(|u| u["screenshot"].is_string()), "{used:?}");
    assert_eq!(h.events(&task.id, "guard.denied").len(), 1);
}

/// Plan: approval-gated submit. Buying waits for the owner and, when refused, nothing reaches
/// the site; data a page's own script sends after a harmless-looking click is held for the
/// owner too; and a form a page sends by itself is stopped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_approval_gated_submit() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (shop, chat, auto) = (
        h.url("shop", "/shop"),
        h.url("shop", "/script-send"),
        h.url("shop", "/auto"),
    );
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": shop })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_open", json!({ "url": chat })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_open", json!({ "url": auto }))
        ] }]),
    );
    let root = h.objective().await;
    let buy = h.pending().await;
    assert_eq!(
        buy.sensitive_label.as_deref(),
        Some("Money: buying, payments, refunds, payouts")
    );
    assert!(buy.summary.contains("\"Buy now\""), "{}", buy.summary);
    h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    // The "Go" button looks harmless, but the page's script sends a message after the click.
    let send = h.pending().await;
    assert_ne!(send.id, buy.id);
    assert!(
        send.summary.contains("let the page send data to shop.test"),
        "{}",
        send.summary
    );
    assert!(
        send.detail.contains("POST") && send.detail.contains("/api/messages"),
        "{}",
        send.detail
    );
    h.broker.resolve_approval(&send.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        h.site.sent().is_empty(),
        "nothing was sent: {:?}",
        h.site.sent()
    );
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click"))
        .collect();
    assert!(
        clicks[0].contains("failed: Not done: the owner did not approve it"),
        "{text}"
    );
    assert!(
        text.contains("Not sent: the owner did not approve the page sending data"),
        "{text}"
    );
    assert!(text.contains("tried to send a form to shop.test"), "{text}");
    assert!(text.contains("by itself"), "{text}");
    let asked = h.events(&task.id, "approval.requested");
    assert_eq!(asked.len(), 2);
}

/// ADR-035: a chat composer (a contenteditable outside any form) sends on Enter, and the page
/// sends over a live connection (a WebSocket) the network gate cannot see into. Enter asks the
/// owner before it is pressed, and nothing reaches the site until they approve; a click on a
/// harmless-looking button on such a page asks too, naming the live connection.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_chat_composer_and_a_live_connection_ask_before_sending() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let chat = h.url("shop", "/chat");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": chat })),
            tool("browser_type", json!({ "ref": "e1", "text": "hello there" })),
            tool("browser_press", json!({ "key": "Enter" })),
            tool("browser_click", json!({ "ref": "e2" }))
        ], "say": "Sent." }]),
    );
    let root = h.objective().await;
    // Enter in the composer (no <form> anywhere) asks: it sends what the box holds.
    let enter = h.pending().await;
    assert_eq!(enter.capability_label, "Use websites");
    assert_eq!(
        enter.sensitive_label.as_deref(),
        Some("Sending or publishing outside this computer")
    );
    assert!(enter.summary.contains("press Enter"), "{}", enter.summary);
    assert!(enter.reason.contains("text box"), "{}", enter.reason);
    assert!(
        h.site.sent().is_empty(),
        "nothing reaches the site before approval: {:?}",
        h.site.sent()
    );
    h.broker.resolve_approval(&enter.id, true, "owner").unwrap();
    // Approved, the message goes over the page's live connection.
    h.until("the message over the socket", |h| !h.site.sent().is_empty())
        .await;
    // "Go" looks harmless, but the page has a live connection the gate cannot see into: ask.
    let click = h.pending().await;
    assert_ne!(click.id, enter.id);
    assert!(click.summary.contains("\"Go\""), "{}", click.summary);
    assert!(click.reason.contains("live connection"), "{}", click.reason);
    assert_eq!(
        click.sensitive_label.as_deref(),
        Some("Sending or publishing outside this computer")
    );
    h.broker
        .resolve_approval(&click.id, false, "owner")
        .unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_press").contains("Pressed Enter"),
        "{text}"
    );
    assert!(
        line(&text, "browser_click").contains("failed: Not done"),
        "{text}"
    );
    let sent = h.site.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].method, "WS");
    assert_eq!(sent[0].path, "/ws");
    assert_eq!(sent[0].body, "hello there");
    assert_eq!(h.approvals_for(&task.id), 2);
}

/// ADR-035: data a page sends on its own, outside any worker action (a POST its script starts
/// on a timer, long after the click), is never sent silently: Plenipo stops it, and the worker
/// is told with its next result.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn data_a_page_sends_on_its_own_is_stopped_and_the_worker_is_told() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let late = h.url("shop", "/late-send");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": late })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_click", json!({ "ref": "e2" })),
            tool("browser_read", json!({}))
        ] }]),
    );
    let root = h.objective().await;
    // "Go" goes ahead (nothing is sent while Plenipo watches the click). "Buy now" waits for the
    // owner; meanwhile the page's timer fires its POST, with no worker action running.
    let buy = h.pending().await;
    assert!(buy.summary.contains("\"Buy now\""), "{}", buy.summary);
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        h.site.sent().is_empty(),
        "the late POST never reached the site: {:?}",
        h.site.sent()
    );
    h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());
    assert!(
        text.contains("tried to send data to shop.test") && text.contains("on its own"),
        "{text}"
    );
    assert!(text.contains("Plenipo stopped it"), "{text}");
    assert_eq!(h.approvals_for(&task.id), 1, "only \"Buy now\" asked");
}

/// ADR-047: Plenipo's browser never saves files. A click on a link that saves a file (the
/// website answers "attachment", or the link itself says `download`) saves nothing anywhere:
/// not in the folder the browser would save to, not in its profile. The tab stays usable, and
/// the worker is told with its next result.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_page_cannot_save_files_in_plenipos_browser() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    // Where this browser would save files, if it saved any: its profile says so before it
    // starts (Plenipo keeps the profile's other settings as they are).
    let downloads = h.dir.path().join("downloads");
    let prefs = h.profile().join("Default").join("Preferences");
    std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
    std::fs::write(
        &prefs,
        json!({ "download": {
            "default_directory": downloads.display().to_string(),
            "prompt_for_download": false
        } })
        .to_string(),
    )
    .unwrap();
    let url = h.url("shop", "/download");
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": url })),
                tool("browser_click", json!({ "ref": "e1" })),
                tool("browser_click", json!({ "ref": "e2" })),
                tool("browser_read", json!({}))
            ], "say": "Done." }]),
        )
        .await;
    // Both clicks happened, nothing asked, and the page is still there to read.
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click"))
        .collect();
    assert_eq!(clicks.len(), 2, "{text}");
    assert!(
        clicks.iter().all(|c| c.contains("Clicked the link")),
        "{text}"
    );
    assert!(
        line(&text, "browser_read").contains("Page: \"Files to save\""),
        "{text}"
    );
    assert_eq!(h.approvals_for(&task.id), 0);
    // No file was saved: not where the browser would put it, not anywhere under the test's
    // folders (its profile included), not even a part of one (`.crdownload`).
    let saved: Vec<PathBuf> = std::fs::read_dir(&downloads)
        .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(saved.is_empty(), "files were saved: {saved:?}");
    let stray = files_under(h.dir.path())
        .into_iter()
        .filter(|f| {
            let name = f.file_name().unwrap_or_default().to_string_lossy();
            name == "report.txt" || name == "notes.txt" || name.ends_with(".crdownload")
        })
        .collect::<Vec<_>>();
    assert!(stray.is_empty(), "files were saved: {stray:?}");
    // The worker hears of each file the page tried to save, and why it was not saved.
    for name in ["report.txt", "notes.txt"] {
        assert!(
            text.contains(&format!("The page tried to save a file named \"{name}\".")),
            "{text}"
        );
    }
    assert!(
        text.contains("Plenipo's browser does not save files."),
        "{text}"
    );
}

/// Every file under `dir`, at any depth.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

/// ADR-046: a page never gets a second tab. A link that opens one (`target="_blank"`), and a
/// button whose script opens one (`window.open`), open in the worker's own tab instead: the new
/// tab is closed before it loads (the website sees each visit once, from the worker's tab), the
/// worker is told, and the browser has one page for the grant while the worker waits.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_tab_a_page_opens_becomes_the_workers_own_tab() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let url = h.url("shop", "/new-tab");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": url })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_read", json!({})),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_open", json!({ "url": url })),
            tool("browser_click", json!({ "ref": "e3" })),
            tool("browser_read", json!({})),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_read", json!({}))
        ], "say": "Done." }]),
    );
    let root = h.objective().await;
    // Twice, the worker is on the second page and waits for the owner ("Buy now" asks), so the
    // browser can be looked at: one page for the grant, the second page, in the worker's tab.
    for _ in 0..2 {
        let buy = h.pending().await;
        assert!(buy.summary.contains("\"Buy now\""), "{}", buy.summary);
        let pages: Vec<String> = h
            .pages()
            .await
            .into_iter()
            .filter(|u| u.contains("shop.test"))
            .collect();
        assert_eq!(pages.len(), 1, "{pages:?}");
        assert!(pages[0].ends_with("/second"), "{pages:?}");
        h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    }
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click"))
        .collect();
    assert_eq!(clicks.len(), 4, "{text}");
    assert!(
        clicks[0].contains("Clicked the link \"Open the second page in a new tab\"")
            && clicks[0].contains("Now on \"Second page\""),
        "{text}"
    );
    assert!(
        clicks[2].contains("Clicked the button \"Open the second page in a new window\"")
            && clicks[2].contains("Now on \"Second page\""),
        "{text}"
    );
    assert_eq!(
        text.matches("The link opened a new tab; Plenipo opened it here instead.")
            .count(),
        2,
        "{text}"
    );
    // The website saw the second page opened twice, both times from the worker's tab: a new tab
    // that loaded would have shown as a third visit.
    let seconds = h
        .site
        .requests()
        .into_iter()
        .filter(|r| r.method == "GET" && r.path == "/second")
        .count();
    assert_eq!(seconds, 2, "{:?}", h.site.requests());
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());
    assert_eq!(
        h.approvals_for(&task.id),
        2,
        "only \"Buy now\" asked, twice"
    );
}

/// ADR-046: a new tab a page opens on its own, outside any worker action (its script's
/// `window.open` on a timer, long after the click that started it), never loads: it is closed,
/// the website is not asked for it, the worker is told with its next result, and the worker's
/// tab stays on its page and keeps taking clicks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_tab_a_page_opens_on_its_own_is_closed_and_the_worker_told() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let timer = h.url("shop", "/popup-timer");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": timer })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_click", json!({ "ref": "e2" })),
            tool("browser_read", json!({})),
            tool("browser_click", json!({ "ref": "e3" })),
            tool("browser_read", json!({}))
        ] }]),
    );
    let root = h.objective().await;
    // "Go" goes ahead. "Buy now" waits for the owner; meanwhile the page's timer opens its new
    // tab, with no worker action running.
    let buy = h.pending().await;
    assert!(buy.summary.contains("\"Buy now\""), "{}", buy.summary);
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        !h.site.requests().iter().any(|r| r.path == "/second"),
        "the new tab never loaded: {:?}",
        h.site.requests()
    );
    let pages: Vec<String> = h
        .pages()
        .await
        .into_iter()
        .filter(|u| u.contains("shop.test"))
        .collect();
    assert_eq!(
        pages,
        [timer.as_str()],
        "the worker's tab, where it was, and no other"
    );
    h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    let reads: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_read"))
        .collect();
    assert_eq!(reads.len(), 2, "{text}");
    assert!(reads[0].contains("Page: \"Timer\""), "{text}");
    assert!(
        text.contains("The page tried to open a new tab on its own; Plenipo closed it."),
        "{text}"
    );
    assert!(!text.contains("opened it here instead"), "{text}");
    // The page's script went on after its new tab was closed, and the tab still takes clicks.
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click"))
        .collect();
    assert_eq!(clicks.len(), 3, "{text}");
    assert!(clicks[2].contains("Clicked the button \"Again\""), "{text}");
    assert!(text.contains("Clicked again"), "{text}");
    assert!(
        !h.site.requests().iter().any(|r| r.path == "/second"),
        "{:?}",
        h.site.requests()
    );
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());
}

/// ADR-046: a link that opens a blocked website in a new tab goes nowhere. The new tab is
/// closed before it loads, the worker's tab stays where it was, and the worker hears that the
/// website is blocked, in the network gate's own words.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_tab_to_a_blocked_website_never_loads() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let url = h.url("shop", "/new-tab");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": url })),
            tool("browser_click", json!({ "ref": "e2" })),
            tool("browser_click", json!({ "ref": "e4" })),
            tool("browser_read", json!({}))
        ] }]),
    );
    let root = h.objective().await;
    // While the worker waits for the owner ("Buy now" asks): its tab, where it was, and no other.
    let buy = h.pending().await;
    assert!(buy.summary.contains("\"Buy now\""), "{}", buy.summary);
    let pages: Vec<String> = h
        .pages()
        .await
        .into_iter()
        .filter(|u| u.contains("shop.test"))
        .collect();
    assert_eq!(pages, [url.as_str()], "{pages:?}");
    h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    let clicked = line(&text, "browser_click");
    assert!(
        clicked.contains("Clicked the link \"Open a blocked website in a new tab\"")
            && clicked.contains("Now on \"New tab links\""),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "The link opened a new tab. Plenipo stopped blocked.test:{} from opening: it is on \
             the owner's blocked websites list (\"blocked.test\").",
            h.site.port
        )),
        "{text}"
    );
    assert!(!text.contains("opened it here instead"), "{text}");
    assert!(
        line(&text, "browser_read").contains("Page: \"New tab links\""),
        "{text}"
    );
    // The blocked website was never asked for the page.
    assert!(
        !h.site.requests().iter().any(|r| r.path == "/second"),
        "{:?}",
        h.site.requests()
    );
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());
}

/// Plan: global stop. The owner's Stop halts all control at once: the worker waiting to send
/// is refused, its permissions end, the page says "Stopped", nothing is sent, and no worker
/// may use the browser or the screen until the owner allows it again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_global_stop() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let form = h.url("shop", "/form");
    h.script(
        "Web Assistant",
        json!([
            { "tools": [
                tool("browser_open", json!({ "url": form })),
                tool("browser_type", json!({ "ref": "e1", "text": "Ada" })),
                tool("browser_click", json!({ "ref": "e4" })),
                tool("browser_read", json!({}))
            ] },
            { "tools": [tool("browser_open", json!({ "url": form }))] },
            { "tools": [tool("browser_open", json!({ "url": form }))] }
        ]),
    );
    let root = h.objective().await;
    let a = h.pending().await;
    let status = h.broker.control_status();
    assert!(status.active() && !status.stopped);
    let status = h.broker.stop_all_control("owner").await.unwrap();
    assert!(status.stopped);
    assert!(status
        .sessions
        .iter()
        .all(|s| s.state == ControlState::Stopped));
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_click").contains("failed: Not done"),
        "the waiting action is refused: {text}"
    );
    assert!(
        line(&text, "browser_read").contains("failed"),
        "later calls are refused: {text}"
    );
    assert!(h.site.sent().is_empty());
    assert_eq!(
        h.broker
            .approvals()
            .unwrap()
            .recent
            .iter()
            .find(|x| x.id == a.id)
            .unwrap()
            .status,
        plenipo_capabilities::ApprovalStatus::Rejected
    );
    assert!(!h.all_events("control.stopped").is_empty());
    assert!(!h.events(&task.id, "guard.grant_revoked").is_empty());
    h.finished(&root).await;
    // Stopped until allowed again: a new objective's browser call is refused.
    let root2 = {
        let d = h
            .workforce
            .give_objective(&h.supervisor, "Do the web task again.", None)
            .await
            .unwrap();
        d.turns.last().unwrap().task_id.clone()
    };
    let t2 = h
        .finished(&h.worker_task(&root2, "Web Assistant").await.id)
        .await;
    assert!(
        h.text(&t2.id)
            .contains("The owner stopped all browser, desktop, and server work"),
        "{}",
        h.text(&t2.id)
    );
    h.finished(&root2).await;
    h.broker.allow_control("owner").unwrap();
    assert!(!h.broker.control_status().stopped);
    let root3 = {
        let d = h
            .workforce
            .give_objective(&h.supervisor, "And once more.", None)
            .await
            .unwrap();
        d.turns.last().unwrap().task_id.clone()
    };
    let t3 = h
        .finished(&h.worker_task(&root3, "Web Assistant").await.id)
        .await;
    assert!(
        line(&h.text(&t3.id), "browser_open").contains("Opened"),
        "{}",
        h.text(&t3.id)
    );
    assert!(!h.all_events("control.allowed").is_empty());
}

/// Plan: timeout. A page that never finishes loading is stopped at its time limit; the worker
/// is told, and the browser keeps working.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_timeout() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (slow, fine) = (h.url("shop", "/slow"), h.url("shop", "/"));
    let started = Instant::now();
    let (_, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": slow, "timeoutSeconds": 5 })),
                tool("browser_open", json!({ "url": fine }))
            ] }]),
        )
        .await;
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_open"))
        .collect();
    assert!(
        lines[0].contains("failed") && lines[0].contains("did not finish loading within 5 seconds"),
        "{text}"
    );
    assert!(lines[1].contains("Opened \"Synthetic Shop\""), "{text}");
    assert!(started.elapsed() < Duration::from_secs(60));
}

/// Plan: browser crash. The browser dies while a worker uses it: the worker's next call says
/// its tab is gone, and the next page it opens starts the browser again (recorded).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_browser_crash() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (slow, fine) = (h.url("shop", "/slow"), h.url("shop", "/"));
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": fine })),
            tool("browser_open", json!({ "url": slow, "timeoutSeconds": 15 })),
            tool("browser_read", json!({})),
            tool("browser_open", json!({ "url": fine }))
        ] }]),
    );
    let root = h.objective().await;
    // While the slow page loads, the browser dies.
    let deadline = Instant::now() + WAIT;
    let run = loop {
        let used = h.all_events("capability.used");
        if let (Some(run), true) = (h.broker.browser().execution_id().await, !used.is_empty()) {
            break run;
        }
        assert!(Instant::now() < deadline, "the browser never started");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.sup.cancel(&run).await.unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_read").contains("your tab is gone"),
        "{text}"
    );
    let last = text
        .lines()
        .rfind(|l| l.starts_with("Tool browser_open"))
        .unwrap();
    assert!(last.contains("Opened"), "{text}");
    assert!(
        text.contains(
            "Plenipo's browser had stopped (it crashed or was closed) and was started again"
        ) || text.contains("Your earlier tab is gone"),
        "{text}"
    );
    let started = h.events(&task.id, "browser.started");
    assert_eq!(started.len(), 2, "{started:?}");
    assert_eq!(started[1]["restarted"], true);
}

/// Plan: user takes control. The owner clicks in the page a worker is using (or presses Take
/// over): the worker stops — its next browser call is refused — the tab stays open for the
/// owner, and the trail says so. Moving the mouse does the same for the desktop.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_user_takes_control() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (form, slow) = (h.url("shop", "/form"), h.url("shop", "/slow"));
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": form })),
            tool("browser_open", json!({ "url": slow, "timeoutSeconds": 8 })),
            tool("browser_read", json!({}))
        ] }]),
    );
    let root = h.objective().await;
    h.until("the worker to use the form", |h| {
        h.all_events("capability.used")
            .iter()
            .any(|u| u["tool"] == "browser_open")
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.owner_clicks("Contact us").await;
    h.until("the owner to have control", |h| {
        h.broker
            .control_status()
            .sessions
            .iter()
            .any(|s| s.state == ControlState::TakenOver)
    })
    .await;
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_read").contains("The owner took over the browser"),
        "{text}"
    );
    let taken = h.events(&task.id, "control.taken_over");
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0]["why"], "you clicked or typed in the page");
    // The tab stays open for the owner after the worker's step.
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.owner_clicks("shop.test").await;
    // The Supervisor's turn ends before it takes the next objective.
    h.finished(&root).await;

    // The desktop: the owner moving the mouse takes it back.
    h.script(
        "Desk Operator",
        json!([{ "tools": [
            tool("screen_take_control", json!({ "reason": "The billing program has no API or command line." })),
            tool("screen_view", json!({})),
            tool("screen_click", json!({ "x": 100, "y": 100, "purpose": "open the File menu" })),
            tool("screen_keys", json!({ "keys": "enter", "purpose": "choose the first item" })),
            tool("screen_click", json!({ "x": 200, "y": 200, "purpose": "open the Edit menu" }))
        ] }]),
    );
    let d = h
        .workforce
        .give_objective(&h.supervisor, "Use the billing program.", None)
        .await
        .unwrap();
    let root = d.turns.last().unwrap().task_id.clone();
    let a = h.pending().await;
    assert_eq!(
        a.sensitive_label.as_deref(),
        Some("Taking control of your mouse and keyboard")
    );
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    // Every click asks (ADR-049): the owner approves this one.
    let click = h.pending().await;
    assert!(
        click.summary.contains("click at (100, 100)"),
        "{}",
        click.summary
    );
    h.broker.resolve_approval(&click.id, true, "owner").unwrap();
    // While the worker waits to press Enter, the owner reaches for the mouse.
    let enter = h.pending().await;
    assert_ne!(enter.id, click.id);
    h.desktop.owner_moves(700, 500);
    h.until("the owner to have the mouse", |h| {
        h.broker
            .control_status()
            .sessions
            .iter()
            .any(|s| s.kind == ControlKind::Desktop && s.state == ControlState::TakenOver)
    })
    .await;
    let task = h
        .finished(&h.worker_task(&root, "Desk Operator").await.id)
        .await;
    let text = h.text(&task.id);
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool screen_click"))
        .collect();
    assert!(clicks[0].contains("Clicked at"), "{text}");
    assert!(
        line(&text, "screen_keys").contains("failed: Not done"),
        "{text}"
    );
    assert!(
        clicks[1].contains("failed") && clicks[1].contains("took back"),
        "{text}"
    );
    assert!(h.desktop.did().contains(&Did::ReleaseAll));
    assert!(!h.desktop.did().contains(&Did::Keys(vec![KeyPart::Enter])));
    let taken = h.events(&task.id, "control.taken_over");
    assert_eq!(taken[0]["why"], "you moved the mouse");
}

/// A page can change a control while the owner decides. The action goes ahead only on the
/// control the owner saw: a form re-aimed at another website is not sent, and a field that
/// became a password field is not typed into. The worker is told what changed and to read the
/// page again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_control_that_changed_while_the_owner_decided_is_left_alone() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (swap, turncoat) = (h.url("shop", "/swap"), h.url("shop", "/turncoat"));
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": swap })),
            tool("browser_click", json!({ "ref": "e2" })),
            tool("browser_open", json!({ "url": turncoat })),
            tool("browser_type", json!({ "ref": "e1", "text": "hello", "submit": true }))
        ] }]),
    );
    let root = h.objective().await;
    let click = h.pending().await;
    assert!(
        click.summary.contains("\"Send message\""),
        "{}",
        click.summary
    );
    // The page re-aims its form a moment after it loads; the owner decides after that.
    h.page_changed("swap").await;
    h.broker.resolve_approval(&click.id, true, "owner").unwrap();
    let typing = h.pending().await;
    assert_ne!(typing.id, click.id);
    assert!(typing.summary.contains("\"Note\""), "{}", typing.summary);
    h.page_changed("turncoat").await;
    h.broker
        .resolve_approval(&typing.id, true, "owner")
        .unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        h.site.sent().is_empty(),
        "nothing was sent: {:?}",
        h.site.sent()
    );
    let clicked = line(&text, "browser_click");
    assert!(
        clicked.contains("changed since the page was read")
            && clicked.contains("where that form sends"),
        "{text}"
    );
    let typed = line(&text, "browser_type");
    assert!(
        typed.contains("changed since the page was read")
            && typed.contains("a password, a one-time code, or a card number"),
        "{text}"
    );
    assert!(!text.contains("Typed into"), "{text}");
    let used: Vec<Value> = h
        .events(&task.id, "capability.used")
        .into_iter()
        .filter(|u| u["tool"] == "browser_click" || u["tool"] == "browser_type")
        .collect();
    assert_eq!(used.len(), 2, "{used:?}");
    assert!(used.iter().all(|u| u["ok"] == false), "{used:?}");
}

/// The owner's sign that a worker is using the browser is the owner's, not the page's: a page
/// that hides it gets it back at once, and the worker keeps the browser; a page that removes it
/// again and again is stopped, the worker is told why, and the Ledger records it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_owners_sign_survives_the_page_and_a_page_that_keeps_fighting_it_is_stopped() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (hide, fight, under) = (
        h.url("shop", "/sign-hide-once"),
        h.url("shop", "/sign-fight"),
        h.url("shop", "/sign-under-dialog"),
    );
    // The page hides the sign 300 ms after it loads and looks again 600 ms later; the key press
    // (harmless, no approval) keeps the worker busy long enough for the page's second look.
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": hide })),
                tool("browser_press", json!({ "key": "Tab" })),
                tool("browser_read", json!({}))
            ] }]),
        )
        .await;
    assert!(
        text.contains("Hidden: none, later: block"),
        "the page hid the sign and the sign came back: {text}"
    );
    assert!(
        !text.contains("failed"),
        "the worker kept the browser: {text}"
    );
    assert!(h.events(&task.id, "browser.tab_stopped").is_empty());

    // A page that removes the sign again and again: the worker's use of the browser stops.
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": fight })),
                tool("browser_read", json!({})),
                tool("browser_read", json!({}))
            ] }]),
        )
        .await;
    assert!(
        text.contains("kept removing Plenipo's sign"),
        "the worker is told why: {text}"
    );
    let stopped = h.events(&task.id, "browser.tab_stopped");
    assert_eq!(stopped.len(), 1, "{stopped:?}");
    assert!(
        stopped[0]["why"]
            .as_str()
            .unwrap_or_default()
            .contains("kept removing Plenipo's sign"),
        "{stopped:?}"
    );

    // An ordinary page with its own dialog open, a widget at the very top of the stacking order,
    // and a zoom on its root: nothing of the page's was touched, so the worker keeps the browser,
    // and the sign stays up in front (the browser's top layer), at its own size.
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": under })),
                tool("browser_press", json!({ "key": "Tab" })),
                tool("browser_read", json!({}))
            ] }]),
        )
        .await;
    assert!(
        text.contains("Sign: shown, dialog: open, size: full"),
        "the sign is up under a dialog: {text}"
    );
    assert!(
        !text.contains("failed"),
        "the worker kept the browser: {text}"
    );
    assert!(h.events(&task.id, "browser.tab_stopped").is_empty());
}

/// A web address in what Plenipo keeps for good (the approval card, `capability.used`, the
/// control center's notes, a screenshot's record) keeps the website, the page, and the names of
/// its fields (ADR-057): the values after `?` are left out, while the worker itself still reads
/// the address.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn addresses_are_kept_without_what_follows_the_page() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    // A sign-in token Plenipo recognizes as a secret comes first: hiding it must not end the
    // address early and leave the session key after it in the record.
    let shop = h.url(
        "shop",
        "/shop?id_token=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.\
         dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U&session=tok3n-4bc#pay",
    );
    let form = h.url("shop", "/form?ref=tok3n-4bc");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": shop })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_open", json!({ "url": form })),
            tool("browser_type", json!({ "ref": "e1", "text": "see https://docs.test/p?id=42" }))
        ] }]),
    );
    let root = h.objective().await;
    let buy = h.pending().await;
    // While the worker waits, the control center shows where it is.
    let sessions = h.broker.control_status().sessions;
    assert!(
        sessions.iter().all(|s| !format!("{s:?}").contains("tok3n")),
        "{sessions:?}"
    );
    for words in [&buy.summary, &buy.detail, &buy.reason] {
        assert!(!words.contains("tok3n"), "{words}");
    }
    let url = buy.url.clone().unwrap_or_default();
    // The browser reports the page without its `#` part; of the `?` part, only the field's
    // name is kept.
    assert!(url.ends_with("/shop?id_token=…&session=…"), "{url}");
    h.broker.resolve_approval(&buy.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    h.finished(&root).await;
    assert!(
        h.text(&task.id).contains("session=tok3n-4bc"),
        "the worker reads the full address"
    );
    // Plenipo's own records (the worker's answer is its own words, kept as it said them).
    let ours = ["capability.", "approval.", "guard.", "control.", "browser."];
    let kept: Vec<String> = h
        .ledger
        .events_for_task(&task.id)
        .unwrap()
        .into_iter()
        .filter(|e| ours.iter().any(|p| e.event_type.starts_with(p)))
        .map(|e| format!("{}: {}", e.event_type, e.payload))
        .chain(
            h.ledger
                .artifacts_for_task(&task.id)
                .unwrap()
                .into_iter()
                .map(|a| a.metadata.to_string()),
        )
        .collect();
    assert!(
        kept.iter()
            .any(|k| k.contains("/shop?id_token=…&session=…")),
        "{kept:?}"
    );
    assert!(
        kept.iter().all(|k| !k.contains("tok3n")),
        "nothing kept has it: {kept:?}"
    );
    // What the worker typed is kept as typed: only the page's own address is cleaned.
    assert!(
        kept.iter()
            .any(|k| k.contains("see https://docs.test/p?id=42") && k.contains("/form?ref=…")),
        "{kept:?}"
    );
}

// ---- More -------------------------------------------------------------------------------------

/// Computer use is the last resort: no mouse or keyboard without taking control (the owner is
/// asked, with the worker's reason, each time); Enter asks again; a secret is never typed; the
/// Windows key and the shortcuts that close or switch programs are refused, and so is typed text
/// with a hidden character; coordinates are the screenshot's.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn computer_use_asks_first_and_never_types_secrets() {
    let h = harness(None).await;
    h.broker
        .save_secret(&SecretInput {
            name: "Billing password".into(),
            value: Some("correct-horse-battery".into()),
            ..SecretInput::default()
        })
        .unwrap();
    h.script(
        "Desk Operator",
        json!([{ "tools": [
            tool("screen_click", json!({ "x": 10, "y": 10, "purpose": "open the menu" })),
            tool("screen_take_control", json!({ "reason": "The billing program has no API." })),
            tool("screen_click", json!({ "x": 10, "y": 10, "purpose": "open the menu" })),
            tool("screen_view", json!({})),
            tool("screen_click", json!({ "x": 400, "y": 300, "double": true, "purpose": "open the invoice" })),
            tool("screen_type", json!({ "text": "Invoice 42", "purpose": "fill in the title" })),
            tool("screen_type", json!({ "text": "correct-horse-battery", "purpose": "fill in a field" })),
            tool("screen_keys", json!({ "keys": "ctrl+s", "purpose": "keep the draft" })),
            // A new line, or Ctrl+J, is Enter too (in a terminal, it runs a command).
            tool("screen_type", json!({ "text": "rm -rf /srv/app\n", "purpose": "tidy up" })),
            tool("screen_keys", json!({ "keys": "ctrl+j", "purpose": "tidy up" })),
            tool("screen_keys", json!({ "keys": "enter", "purpose": "confirm" })),
            tool("screen_release_control", json!({}))
        ] }]),
    );
    let root = h.objective().await;
    let take = h.pending().await;
    assert!(
        take.detail.contains("The billing program has no API."),
        "{}",
        take.detail
    );
    h.broker.resolve_approval(&take.id, true, "owner").unwrap();
    // Every click, typing, and key press asks (ADR-049): the owner approves these three.
    for words in [
        "double-click at (400, 300)",
        "type \"Invoice 42\"",
        "press ctrl+s",
    ] {
        let step = h.pending().await;
        assert!(step.summary.contains(words), "{}", step.summary);
        assert_eq!(
            step.sensitive_label.as_deref(),
            Some("Taking control of your mouse and keyboard")
        );
        h.broker.resolve_approval(&step.id, true, "owner").unwrap();
    }
    let line = h.pending().await;
    assert!(h.broker.control_status().desktop_active());
    assert!(
        line.reason.contains("a new line presses Enter"),
        "{}",
        line.reason
    );
    // While the worker has the screen, the owner's terminal takes nothing: what reaches it
    // could be the worker's typing.
    let refused = h
        .broker
        .open_terminal(&TerminalPlace::ThisPc, 80, 24, Arc::new(|_| {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Desk Operator is using the screen, mouse, and keyboard"),
        "{refused}"
    );
    h.broker.resolve_approval(&line.id, false, "owner").unwrap();
    for _ in 0..2 {
        let enter = h.pending().await;
        assert!(
            enter
                .reason
                .contains("pressing Enter can send or submit something"),
            "{}",
            enter.reason
        );
        h.broker
            .resolve_approval(&enter.id, false, "owner")
            .unwrap();
    }
    let task = h
        .finished(&h.worker_task(&root, "Desk Operator").await.id)
        .await;
    let text = h.text(&task.id);
    let clicks: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool screen_click"))
        .collect();
    assert!(
        clicks[0].contains("failed") && clicks[0].contains("take control first"),
        "{text}"
    );
    assert!(
        clicks[1].contains("failed") && clicks[1].contains("look at the screen first"),
        "{text}"
    );
    assert!(clicks[2].contains("Clicked at"), "{text}");
    let types: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool screen_type"))
        .collect();
    assert!(types[0].contains("Typed 10 characters"), "{text}");
    assert!(
        types[1].contains("failed") && types[1].contains("secret"),
        "{text}"
    );
    assert!(!text.contains("correct-horse-battery"));
    let did = h.desktop.did();
    // 800×600 fits the picture unscaled: the click lands where asked, twice.
    assert!(did.contains(&Did::Move(400, 300)));
    assert!(did.contains(&Did::Click(Button::Left, 2)));
    assert!(did.contains(&Did::Type("Invoice 42".into())));
    assert!(did.contains(&Did::Keys(vec![KeyPart::Ctrl, KeyPart::Char('s')])));
    assert!(
        !did.contains(&Did::Keys(vec![KeyPart::Enter])),
        "Enter was refused"
    );
    assert!(
        !did.contains(&Did::Keys(vec![KeyPart::Ctrl, KeyPart::Char('j')])),
        "Ctrl+J was refused"
    );
    assert!(
        !did.iter()
            .any(|d| matches!(d, Did::Type(t) if t.contains("rm -rf"))),
        "the new line was refused"
    );
    assert!(!did
        .iter()
        .any(|d| matches!(d, Did::Type(t) if t.contains("horse"))));
    assert!(did.contains(&Did::ReleaseAll), "given back");
    assert!(!h.broker.control_status().desktop_active());
    let windows = plenipo_capabilities::tools::parse(
        plenipo_capabilities::tools::find("screen_keys").unwrap(),
        &json!({ "keys": "win+r", "purpose": "run" }),
    );
    assert!(windows.unwrap_err().contains("Windows key"));
    // Nor the shortcuts that close or switch programs or open the system's own screens, nor
    // typed text with a hidden character in it.
    for keys in [
        "alt+f4",
        "alt+tab",
        "ctrl+esc",
        "ctrl+shift+esc",
        "ctrl+w",
        "ctrl+alt+del",
    ] {
        let refused = plenipo_capabilities::tools::parse(
            plenipo_capabilities::tools::find("screen_keys").unwrap(),
            &json!({ "keys": keys, "purpose": "tidy up" }),
        );
        assert!(
            refused.unwrap_err().contains("not available to workers"),
            "{keys}"
        );
    }
    let hidden = plenipo_capabilities::tools::parse(
        plenipo_capabilities::tools::find("screen_type").unwrap(),
        &json!({ "text": "ok\u{1b}:q!", "purpose": "fill in a field" }),
    );
    assert!(hidden.unwrap_err().contains("hidden character (U+001B)"));
    // Recorded, with the screen after each action.
    let used = h.events(&task.id, "capability.used");
    assert!(used
        .iter()
        .any(|u| u["tool"] == "screen_click" && u["screenshot"].is_string()));
}

/// ADR-049 (computer use asks before every click and keystroke): once the owner lets a worker
/// take control, every click, typing, and key press on the screen asks the owner first, whatever
/// the worker calls it, with a picture of the screen (a click's point marked), the worker's own
/// words, and the text to be typed; each runs only when approved. Looking at the screen and
/// scrolling do not ask. A refused step is not done, and the worker is told so. A purpose that
/// reads like paying is the card's headline.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_click_and_key_on_the_desktop_asks_the_owner() {
    use plenipo_runtime::agent::ToolProvider;
    let h = harness(None).await;
    // A first task brings the worker's conversation up; then the test makes the calls itself.
    h.run(
        "Desk Operator",
        json!([{ "tools": [tool("screen_view", json!({}))], "say": "Done." }]),
    )
    .await;
    let (task, grant) = h.direct_grant("Desk Operator").await;
    let call = |name: &'static str, args: Value| {
        let (broker, grant) = (h.broker.clone(), grant.clone());
        tokio::spawn(async move { broker.call(&grant, name, args).await })
    };
    let control = "Taking control of your mouse and keyboard";
    // Taking control asks once.
    let take = call(
        "screen_take_control",
        json!({ "reason": "The billing program has no API." }),
    );
    let a = h.pending().await;
    assert_eq!(a.sensitive_label.as_deref(), Some(control));
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    assert!(!take.await.unwrap().is_error);
    // Looking at the screen does not ask.
    let looked = h.broker.call(&grant, "screen_view", json!({})).await;
    assert!(!looked.is_error, "{}", looked.text);
    // A click asks, whatever the worker calls it, with the screen and the point marked on it.
    let click = call(
        "screen_click",
        json!({ "x": 100, "y": 100, "purpose": "continue" }),
    );
    let a = h.pending().await;
    assert_eq!(a.sensitive_label.as_deref(), Some(control));
    assert!(a.summary.contains("click at (100, 100)"), "{}", a.summary);
    assert!(
        a.reason.contains("\"continue\"") && a.reason.contains("asks before each click"),
        "{}",
        a.reason
    );
    let picture = h
        .ledger
        .artifact(a.screenshot.as_deref().expect("the screen on the card"))
        .unwrap()
        .unwrap();
    assert_eq!(picture.metadata["action"], "waiting for your approval");
    assert_eq!(picture.metadata["kind"], "desktop");
    assert_eq!(picture.metadata["point"], json!([100, 100]));
    assert!(
        !h.desktop.did().contains(&Did::Click(Button::Left, 1)),
        "nothing is clicked before the owner answers"
    );
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let r = click.await.unwrap();
    assert!(r.text.contains("Clicked at (100, 100)"), "{}", r.text);
    assert!(h
        .desktop
        .did()
        .ends_with(&[Did::Move(100, 100), Did::Click(Button::Left, 1)]));
    // Typing asks, and the card shows the text.
    let typed = call(
        "screen_type",
        json!({ "text": "Invoice 42", "purpose": "fill in the title" }),
    );
    let a = h.pending().await;
    assert_eq!(a.sensitive_label.as_deref(), Some(control));
    assert!(a.summary.contains("type \"Invoice 42\""), "{}", a.summary);
    assert!(a.detail.contains("Invoice 42"), "{}", a.detail);
    assert!(a.reason.contains("\"fill in the title\""), "{}", a.reason);
    assert!(a.screenshot.is_some(), "the screen is on the card");
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let r = typed.await.unwrap();
    assert!(r.text.contains("Typed 10 characters"), "{}", r.text);
    // A key press asks.
    let keys = call(
        "screen_keys",
        json!({ "keys": "ctrl+s", "purpose": "keep the draft" }),
    );
    let a = h.pending().await;
    assert_eq!(a.sensitive_label.as_deref(), Some(control));
    assert!(a.summary.contains("press ctrl+s"), "{}", a.summary);
    assert!(a.reason.contains("\"keep the draft\""), "{}", a.reason);
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    assert!(!keys.await.unwrap().is_error);
    assert!(h
        .desktop
        .did()
        .contains(&Did::Keys(vec![KeyPart::Ctrl, KeyPart::Char('s')])));
    // Scrolling does not ask.
    let scrolled = h
        .broker
        .call(
            &grant,
            "screen_scroll",
            json!({ "amount": 3, "purpose": "see more" }),
        )
        .await;
    assert!(!scrolled.is_error, "{}", scrolled.text);
    assert!(h.desktop.did().contains(&Did::Scroll(3)));
    // A refused click is not done, and the worker is told so.
    let click = call(
        "screen_click",
        json!({ "x": 200, "y": 200, "purpose": "continue" }),
    );
    let a = h.pending().await;
    h.broker.resolve_approval(&a.id, false, "owner").unwrap();
    let r = click.await.unwrap();
    assert!(r.is_error);
    assert!(
        r.text.contains("Not done: the owner did not approve it"),
        "{}",
        r.text
    );
    assert!(!h.desktop.did().contains(&Did::Move(200, 200)));
    // A purpose that reads like paying is the card's headline reason.
    let click = call(
        "screen_click",
        json!({ "x": 300, "y": 300, "purpose": "pay the invoice" }),
    );
    let a = h.pending().await;
    assert_eq!(
        a.sensitive_label.as_deref(),
        Some("Money: buying, payments, refunds, payouts")
    );
    assert!(
        a.reason.contains("looks like buying or paying"),
        "{}",
        a.reason
    );
    h.broker.resolve_approval(&a.id, false, "owner").unwrap();
    assert!(click.await.unwrap().is_error);
    // Six cards: taking control, three approved steps, two refused. Looking and scrolling made
    // none.
    assert_eq!(h.events(&task, "approval.requested").len(), 6);
    ToolProvider::close(&h.broker, &grant);
}

/// ADR-049: one desktop step at a time while the owner decides. While a click's card waits, a
/// scroll (which can move the mouse too) is refused and told to wait, so the picture the owner
/// decides from stays the screen as it is; nothing is scrolled and no card is made. Once the
/// owner answers, the scroll runs. Looking at the screen is allowed all along.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_desktop_step_at_a_time_while_the_owner_decides() {
    use plenipo_runtime::agent::ToolProvider;
    let h = harness(None).await;
    h.run(
        "Desk Operator",
        json!([{ "tools": [tool("screen_view", json!({}))], "say": "Done." }]),
    )
    .await;
    let (task, grant) = h.direct_grant("Desk Operator").await;
    let call = |name: &'static str, args: Value| {
        let (broker, grant) = (h.broker.clone(), grant.clone());
        tokio::spawn(async move { broker.call(&grant, name, args).await })
    };
    let take = call(
        "screen_take_control",
        json!({ "reason": "The billing program has no API." }),
    );
    let a = h.pending().await;
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    assert!(!take.await.unwrap().is_error);
    assert!(
        !h.broker
            .call(&grant, "screen_view", json!({}))
            .await
            .is_error
    );
    let click = call(
        "screen_click",
        json!({ "x": 100, "y": 100, "purpose": "continue" }),
    );
    let a = h.pending().await;
    // While the click's card waits, a scroll is refused, and nothing moves or scrolls.
    let scrolled = h
        .broker
        .call(
            &grant,
            "screen_scroll",
            json!({ "x": 100, "y": 300, "amount": 3, "purpose": "see more" }),
        )
        .await;
    assert!(scrolled.is_error, "{}", scrolled.text);
    assert!(
        scrolled.text.contains(
            "Wait for the owner's answer to the step that is waiting before the next one."
        ),
        "{}",
        scrolled.text
    );
    let did = h.desktop.did();
    assert!(!did.contains(&Did::Scroll(3)), "{did:?}");
    assert!(!did.contains(&Did::Move(100, 300)), "{did:?}");
    // Looking at the screen still is, and the refusal made no card of its own.
    assert!(
        !h.broker
            .call(&grant, "screen_view", json!({}))
            .await
            .is_error
    );
    assert_eq!(h.broker.approvals().unwrap().pending.len(), 1);
    // The owner answers: the click runs, and then the scroll does.
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let r = click.await.unwrap();
    assert!(r.text.contains("Clicked at (100, 100)"), "{}", r.text);
    let scrolled = h
        .broker
        .call(
            &grant,
            "screen_scroll",
            json!({ "x": 100, "y": 300, "amount": 3, "purpose": "see more" }),
        )
        .await;
    assert!(!scrolled.is_error, "{}", scrolled.text);
    assert!(h
        .desktop
        .did()
        .ends_with(&[Did::Move(100, 300), Did::Scroll(3)]));
    // Two cards: taking control and the click.
    assert_eq!(h.events(&task, "approval.requested").len(), 2);
    ToolProvider::close(&h.broker, &grant);
}

/// The Researcher reads websites but cannot use them; a worker without browser permissions
/// gets no browser tools at all; and taking over a desktop session nobody holds is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_researcher_reads_but_cannot_click() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let url = h.url("shop", "/form");
    let (task, text) = h
        .run(
            "Researcher",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": url })),
                tool("browser_read", json!({})),
                tool("browser_click", json!({ "ref": "e4" }))
            ] }]),
        )
        .await;
    assert!(
        line(&text, "browser_read").contains("Page: \"Contact us\""),
        "{text}"
    );
    assert!(
        line(&text, "browser_click").contains("failed: Blocked: the Researcher set of the Researcher role does not allow using websites"),
        "{text}"
    );
    assert_eq!(h.events(&task.id, "guard.denied").len(), 1);
    assert!(h.site.sent().is_empty());
    assert!(h
        .broker
        .take_over(&session_id(ControlKind::Desktop, "nobody"), "test")
        .await
        .is_err());
}

/// The owner's switches (ADR-023): with "send without asking" on, a form on an allowed website
/// goes out without an approval (still recorded, with its screenshot); with screenshots off, the
/// trail keeps no pictures, but an approval card still does.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn switches_send_without_asking_and_screenshots_off() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    h.guard
        .set_switches(&Switches {
            desktop: true,
            send_without_asking: true,
            screenshots: false,
            ..Switches::default()
        })
        .unwrap();
    let (form, shop) = (h.url("shop", "/form"), h.url("shop", "/shop"));
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": form })),
            tool("browser_type", json!({ "ref": "e1", "text": "Ada" })),
            tool("browser_click", json!({ "ref": "e4" })),
            tool("browser_open", json!({ "url": shop })),
            tool("browser_click", json!({ "ref": "e1" }))
        ], "say": "Done." }]),
    );
    let root = h.objective().await;
    // Buying still asks: "send without asking" covers sending only.
    let a = h.pending().await;
    assert_eq!(
        a.sensitive_label.as_deref(),
        Some("Money: buying, payments, refunds, payouts")
    );
    assert!(a.screenshot.is_some(), "approval cards keep their picture");
    h.broker.resolve_approval(&a.id, false, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_click").contains("Now on \"Thank you\""),
        "sent without asking: {text}"
    );
    let sent = h.site.sent();
    assert_eq!(sent.len(), 1, "the form, not the order: {sent:?}");
    assert_eq!(sent[0].path, "/send");
    // Recorded as usual, but without step pictures.
    let used = h.events(&task.id, "capability.used");
    assert!(used.iter().any(|u| u["tool"] == "browser_click"));
    assert!(used.iter().all(|u| u["screenshot"].is_null()), "{used:?}");
    assert_eq!(h.approvals_for(&task.id), 1, "only the order asked");
}

/// A check that a person is using the site (a CAPTCHA) goes to the owner: the worker has tried
/// it its counted times (ADR-029), waits while the owner solves it in Plenipo's browser, and
/// continues when the owner approves. With the switch off, the worker is told to stop.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_person_check_is_handed_to_the_owner() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let captcha = h.url("shop", "/captcha");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": captcha })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_person_check", json!({})),
            tool("browser_read", json!({}))
        ], "say": "Done." }]),
    );
    let root = h.objective().await;
    let a = h.pending().await;
    assert!(
        a.summary
            .contains("hand you a check that a person is using shop.test"),
        "{}",
        a.summary
    );
    assert!(
        a.reason
            .contains("tried the check 1 time and could not get past it"),
        "{}",
        a.reason
    );
    assert!(a.screenshot.is_some());
    // The owner's own clicks while solving it do not take the browser from the worker.
    h.owner_clicks("Check").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(h
        .broker
        .control_status()
        .sessions
        .iter()
        .all(|s| s.state == ControlState::Active));
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_click").contains("Clicked"),
        "the worker's try at the check went ahead: {text}"
    );
    assert!(text.contains("try 1 of 3"), "{text}");
    assert!(
        line(&text, "browser_person_check").contains("The owner solved the check"),
        "{text}"
    );
    assert!(!line(&text, "browser_read").contains("failed"), "{text}");
    h.finished(&root).await;

    // Switched off: the worker is told to stop.
    h.guard
        .set_switches(&Switches {
            desktop: true,
            captcha_to_owner: false,
            ..Switches::default()
        })
        .unwrap();
    let (_, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": captcha })),
                tool("browser_person_check", json!({}))
            ], "say": "Done." }]),
        )
        .await;
    assert!(
        line(&text, "browser_person_check").contains("has not switched on handing these checks"),
        "{text}"
    );
}

/// A CAPTCHA is tried three counted times before it goes to the owner (ADR-029): only a
/// submitted answer counts as a try (clicking its control, or Enter or Space in it), while
/// typing and moving around inside it are free. The try after the limit is refused and pointed
/// at the hand-off, and browser_person_check then brings the owner in. With the hand-off switch
/// off, no try happens.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_captcha_is_tried_three_times_before_handoff() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let captcha = h.url("shop", "/captcha");
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": captcha })),
            tool("browser_type", json!({ "ref": "e2", "text": "a person" })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_type", json!({ "ref": "e2", "text": "really" })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_click", json!({ "ref": "e1" })),
            tool("browser_person_check", json!({})),
            tool("browser_read", json!({}))
        ], "say": "Done." }]),
    );
    let root = h.objective().await;
    let a = h.pending().await;
    assert!(
        a.summary
            .contains("hand you a check that a person is using shop.test"),
        "{}",
        a.summary
    );
    assert!(
        a.reason
            .contains("tried the check 3 times and could not get past it"),
        "{}",
        a.reason
    );
    h.broker.resolve_approval(&a.id, true, "owner").unwrap();
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    // The page said the worker may try, and the three submitted answers were counted out loud,
    // in order. Typing inside the check was free: it produced no try of its own.
    assert!(
        text.contains("each answer you submit counts as one try"),
        "{text}"
    );
    let tries: Vec<usize> = [1, 2, 3]
        .iter()
        .map(|n| {
            text.find(format!("try {n} of 3").as_str())
                .unwrap_or_else(|| panic!("no try {n} in:\n{text}"))
        })
        .collect();
    assert!(tries[0] < tries[1] && tries[1] < tries[2], "{tries:?}");
    assert_eq!(text.matches(" of 3").count(), 3, "{text}");
    let types: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_type"))
        .collect();
    assert_eq!(types.len(), 2, "{types:?}");
    assert!(types.iter().all(|t| !t.contains("of 3")), "{types:?}");
    // The fourth try was refused and pointed at the hand-off.
    let refused: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click failed"))
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].contains("tried it 3 times"), "{refused:?}");
    assert!(refused[0].contains("browser_person_check"), "{refused:?}");
    assert!(
        line(&text, "browser_person_check").contains("The owner solved the check"),
        "{text}"
    );
    assert!(!line(&text, "browser_read").contains("failed"), "{text}");
    // After the hand-off the count started over.
    let solved = text.find("The owner solved the check").unwrap();
    let used = text.rfind("you have used 0").unwrap();
    assert!(solved < used, "tries start over after the hand-off: {text}");
    h.finished(&root).await;

    // Switched off: the very first touch of the check is refused, and no try is counted.
    h.guard
        .set_switches(&Switches {
            desktop: true,
            captcha_to_owner: false,
            ..Switches::default()
        })
        .unwrap();
    let (_, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": captcha })),
                tool("browser_click", json!({ "ref": "e1" }))
            ], "say": "Done." }]),
        )
        .await;
    assert!(
        line(&text, "browser_click").contains("has not switched on handing these checks"),
        "{text}"
    );
    assert!(!text.contains("try 1 of 3"), "{text}");
}

/// A real check keeps its checkbox inside its own frame (ADR-032): the page read lists that
/// frame as a control and names it as the check's checkbox, the click lands on the checkbox
/// itself (not on the words beside it), and the result says the check passed, so the worker
/// moves on, a further click there is refused, and the tries start over. A check that opens a
/// puzzle instead points the worker at the hand-off.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_captcha_in_its_own_frame_is_clicked_and_its_verdict_read() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let framed = h.url("shop", "/captcha-frame");
    let (task, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": framed })),
                tool("browser_click", json!({ "ref": "e1" })),
                tool("browser_read", json!({})),
                tool("browser_click", json!({ "ref": "e1" }))
            ], "say": "Done." }]),
        )
        .await;
    // The page read names the check, its maker, and its checkbox.
    assert!(text.contains("(reCAPTCHA, a check that a person"), "{text}");
    assert!(text.contains("Its checkbox is e1"), "{text}");
    assert!(
        text.contains("e1: checkbox \"I'm not a robot (reCAPTCHA)\" (the CAPTCHA's own checkbox"),
        "{text}"
    );
    // The click hit the checkbox inside the frame and the check passed.
    let clicked = line(&text, "browser_click");
    assert!(
        clicked.contains("Clicked the checkbox \"I'm not a robot (reCAPTCHA)\""),
        "{clicked}"
    );
    assert!(text.contains("try 1 of 3"), "{text}");
    assert!(text.contains("The check is passed"), "{text}");
    // Read again: passed already, nothing to do; a further click is refused and no try counted.
    assert!(text.contains("is passed already"), "{text}");
    let refused: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("Tool browser_click failed"))
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].contains("passed already"), "{refused:?}");
    assert_eq!(text.matches(" of 3").count(), 1, "{text}");
    // Clicking the check is not sending anything: nothing waited for the owner or reached the
    // site.
    assert_eq!(h.approvals_for(&task.id), 0);
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());

    // The same check opening a puzzle: the worker hears so, and is pointed at the hand-off.
    let puzzle = h.url("shop", "/captcha-frame?puzzle");
    let (_, text) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [
                tool("browser_open", json!({ "url": puzzle })),
                tool("browser_click", json!({ "ref": "e1" })),
                tool("browser_read", json!({}))
            ], "say": "Done." }]),
        )
        .await;
    assert!(text.contains("try 1 of 3"), "{text}");
    assert!(
        text.contains("The check now shows a puzzle"),
        "the click's result says a puzzle opened: {text}"
    );
    assert!(
        line(&text, "browser_click").contains("Clicked") && text.contains("browser_person_check"),
        "{text}"
    );
    assert!(
        text.contains("It shows a puzzle (pictures to pick)"),
        "the page read says so too: {text}"
    );
    assert!(!text.contains("The check is passed"), "{text}");
}

/// Switching Plenipo's browser off (ADR-023) stops the worker using it at once; the next
/// worker gets no browser tools, and the trail says why.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn switching_the_browser_off_stops_it() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let (form, slow) = (h.url("shop", "/form"), h.url("shop", "/slow"));
    h.script(
        "Web Assistant",
        json!([{ "tools": [
            tool("browser_open", json!({ "url": form })),
            tool("browser_open", json!({ "url": slow, "timeoutSeconds": 8 })),
            tool("browser_read", json!({}))
        ], "say": "Done." }]),
    );
    let root = h.objective().await;
    h.until("the worker to use the browser", |h| {
        h.broker.control_status().active()
    })
    .await;
    h.guard
        .set_switches(&Switches {
            browser: false,
            ..Switches::default()
        })
        .unwrap();
    let status = h
        .broker
        .switch_off_control(ControlKind::Browser)
        .await
        .unwrap();
    assert!(!status.stopped, "not the emergency stop");
    let task = h.worker_task(&root, "Web Assistant").await;
    let task = h.finished(&task.id).await;
    let text = h.text(&task.id);
    assert!(
        line(&text, "browser_read").contains("switched off"),
        "{text}"
    );
    assert_eq!(h.all_events("control.switched_off").len(), 1);
    h.finished(&root).await;
    // The next worker gets no browser tools, and the trail says why.
    let root = h.objective().await;
    let task = h.worker_task(&root, "Web Assistant").await;
    h.finished(&task.id).await;
    let skipped = h.events(&task.id, "guard.grant_skipped");
    assert!(
        skipped.iter().any(|e| e["reason"]
            .as_str()
            .is_some_and(|r| r.contains("Plenipo's browser is switched off"))),
        "{skipped:?}"
    );
}

/// A lesson from a task that used websites always waits for the owner, even when the role
/// learns on its own (ADR-024): a website must not be able to plant one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lessons_from_websites_always_wait_for_the_owner() {
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    let role = h.role_of("Web Assistant");
    h.workforce.set_role_learning(&role, true).unwrap();
    let url = h.url("shop", "/");
    let (task, _) = h
        .run(
            "Web Assistant",
            json!([{ "tools": [tool("browser_open", json!({ "url": url }))],
                "say": "Done.\n```plenipo-lesson\n- The shop's contact form is under Contact us.\n```" }]),
        )
        .await;
    h.until("the lesson", |h| {
        !h.workforce.learning().unwrap().waiting.is_empty()
    })
    .await;
    let lesson = &h.workforce.learning().unwrap().waiting[0];
    assert!(lesson.from_web);
    assert_eq!(lesson.task_id.as_deref(), Some(task.id.as_str()));
    assert!(h.workforce.learning().unwrap().kept.is_empty());
}

/// What a worker hears when three of its requests already wait for the owner (B6).
const THREE_WAITING: &str = "Plenipo is waiting for the owner's answer to 3 earlier requests. \
                             Wait for those before asking again.";

/// B6: a browser call refused by the grant's limits on asking (three requests already wait for
/// the owner) gets no card, and no picture is kept for one. The worker hears the limit's words,
/// not that the owner said no: for a page it wants to open, for a check it wants to hand to the
/// owner, and for data a page's script sends after its click (which is stopped).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_browser_ask_refused_by_the_limits_says_so_and_keeps_no_picture() {
    use plenipo_runtime::agent::ToolProvider;
    let browser = need_browser!();
    let h = harness(Some(browser)).await;
    // A first task brings the worker's conversation up; then the test makes the calls itself.
    let captcha = h.url("shop", "/captcha");
    h.run(
        "Web Assistant",
        json!([{ "tools": [tool("browser_open", json!({ "url": captcha }))], "say": "Done." }]),
    )
    .await;
    let (task, grant) = h.direct_grant("Web Assistant").await;
    let opened = h
        .broker
        .call(&grant, "browser_open", json!({ "url": captcha }))
        .await;
    assert!(!opened.is_error, "{}", opened.text);
    // Three pages on neither list: each asks the owner, with a picture of the page, and waits.
    let three: Vec<_> = (0..3)
        .map(|i| {
            let (broker, grant) = (h.broker.clone(), grant.clone());
            let url = h.url("other", &format!("/{i}"));
            tokio::spawn(async move {
                broker
                    .call(&grant, "browser_open", json!({ "url": url }))
                    .await
            })
        })
        .collect();
    h.until("three requests wait", |h| {
        h.broker.approvals().unwrap().pending.len() == 3
    })
    .await;
    // The pictures kept for approval cards (a step's own pictures are another matter).
    let pictures = |h: &H| {
        h.ledger
            .artifacts_for_task(&task)
            .unwrap()
            .into_iter()
            .filter(|a| a.metadata["action"] == "waiting for your approval")
            .count()
    };
    let before = pictures(&h);
    assert_eq!(before, 3, "each card has its picture");
    // A fourth page: refused with the limit's words, and no picture for a card never made.
    let fourth = h
        .broker
        .call(
            &grant,
            "browser_open",
            json!({ "url": h.url("other", "/more") }),
        )
        .await;
    assert!(fourth.is_error, "{}", fourth.text);
    assert_eq!(fourth.text, THREE_WAITING);
    // Handing the page's check to the owner: the same words, not "the owner did not solve it".
    let check = h
        .broker
        .call(&grant, "browser_person_check", json!({}))
        .await;
    assert!(check.is_error, "{}", check.text);
    assert_eq!(check.text, THREE_WAITING);
    // Data the page's script sends after a click: held, then stopped with the same words, not
    // "the owner did not approve".
    let chat = h.url("shop", "/script-send");
    let opened = h
        .broker
        .call(&grant, "browser_open", json!({ "url": chat }))
        .await;
    assert!(!opened.is_error, "{}", opened.text);
    let clicked = h
        .broker
        .call(&grant, "browser_click", json!({ "ref": "e1" }))
        .await;
    assert!(!clicked.is_error, "{}", clicked.text);
    assert!(
        clicked.text.contains("Clicked the button \"Go\""),
        "{}",
        clicked.text
    );
    assert!(clicked.text.contains(THREE_WAITING), "{}", clicked.text);
    assert!(
        !clicked.text.contains("did not approve"),
        "{}",
        clicked.text
    );
    assert!(h.site.sent().is_empty(), "{:?}", h.site.sent());
    assert_eq!(
        pictures(&h),
        before,
        "no picture for a card that was never made"
    );
    assert_eq!(h.broker.approvals().unwrap().pending.len(), 3);
    assert_eq!(h.events(&task, "approval.requested").len(), 3);
    // The step ends: the three cards expire, and each call comes back with its answer.
    ToolProvider::close(&h.broker, &grant);
    for call in three {
        let r = call.await.unwrap();
        assert!(r.is_error, "{}", r.text);
    }
    assert!(h.broker.approvals().unwrap().pending.is_empty());
}
