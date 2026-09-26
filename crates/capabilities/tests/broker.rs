//! Phase 7 capability broker and Guard tests: the real Guard, broker, tool server, relay,
//! Workforce, Router, Liaison, agent runtime, supervisor, adapters, and a file-backed Ledger,
//! driving `plenipo-fake-agent` installed as `claude` and `codex`, which calls Plenipo's tools
//! over MCP through the relay exactly as a real AI tool would. Every plan test is covered, and
//! the acceptance scenarios end to end. No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_capabilities::{
    ApprovalStatus, Broker, BrokerConfig, MemorySecretStore, SecretStore as _,
};
use plenipo_guard::{Capability, Guard, GuardOptions, PermissionSetInput, SecretInput};
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
    DepartmentInput, HireInput, LeadInput, OrgSnapshot, ProjectInput, Workforce,
};

const WAIT: Duration = Duration::from_secs(60);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
const VAULT_VALUE: &str = "vault-secret-value-42";

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

/// One copy of the fake CLIs per test process, ready to execute.
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("capabilities-fake-agents-{}", std::process::id()));
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

fn install_fake(bin: &Path, stem: &str) {
    let source = fake_clis().join(exe_name(stem));
    let target = bin.join(exe_name(stem));
    if std::fs::hard_link(&source, &target).is_err() {
        std::fs::copy(&source, &target).unwrap();
    }
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct NoUpdates;

impl AgentSink for NoUpdates {
    fn emit(&self, _: AgentUpdate) {}
}

struct H {
    ledger: Arc<Ledger>,
    rt: AgentRuntime,
    workforce: Workforce,
    guard: Guard,
    broker: Broker,
    store: Arc<MemorySecretStore>,
    run: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
    /// The project's folder.
    folder: PathBuf,
    supervisor: String,
    developer: String,
    reviewer: String,
    project: String,
}

impl Drop for H {
    fn drop(&mut self) {
        self.run.abort();
    }
}

fn lead(role_id: &str, title: &str, runtime: &str) -> LeadInput {
    LeadInput {
        role_id: role_id.into(),
        title: title.into(),
        runtime_id: Some(runtime.into()),
        model: None,
        vacant: None,
    }
}

/// Development → Website (its folder has a README, a source file, a `.env`, and a config file
/// holding secrets), with a Website Supervisor on Claude Code, a Backend Developer (Senior
/// Developer) on Codex, and a Reviewer (Code Reviewer) on Claude Code.
async fn harness() -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".plenipo-fake-agent")).unwrap();
    for stem in personas() {
        install_fake(&bin, stem);
    }
    let folder = dir.path().join("website");
    std::fs::create_dir_all(folder.join("src")).unwrap();
    std::fs::write(
        folder.join("README.md"),
        "# Website\nThe company website.\n",
    )
    .unwrap();
    std::fs::write(folder.join("src").join("app.txt"), "version = 1\n").unwrap();
    std::fs::write(folder.join(".env"), "DATABASE_URL=postgres://x\n").unwrap();
    std::fs::write(
        folder.join("config.txt"),
        format!("deploy key: {VAULT_VALUE}\nAPI_TOKEN=tok_12345678abcdef\n"),
    )
    .unwrap();
    std::fs::write(dir.path().join("outside.txt"), "not yours\n").unwrap();

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
    config.extra_env = vec![(
        HOME_VAR.into(),
        dir.path().join("home").display().to_string(),
    )];
    config.turn_timeout = Duration::from_secs(120);
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
    let workforce = Workforce::new(
        Arc::clone(&ledger),
        rt.clone(),
        liaison.clone(),
        router.clone(),
    );
    // Phase 7, as the desktop app wires it.
    let guard = Guard::new(Arc::clone(&ledger));
    guard.seed_template_roles().unwrap();
    let store = Arc::new(MemorySecretStore::default());
    let mut broker_config = BrokerConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_plenipo-tool-relay")),
        dir.path().join("tickets"),
    );
    // One "minute" of the approval window lasts a second here.
    broker_config.approval_minute = Duration::from_secs(1);
    let broker = Broker::new(guard.clone(), sup.clone(), store.clone(), broker_config);
    broker.start().await.unwrap();
    rt.set_tools(Arc::new(broker.clone()));
    rt.set_filter(broker.text_filter());
    let run = tokio::spawn(liaison.clone().run());

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
    let s = workforce
        .create_department(&DepartmentInput {
            name: "Development".into(),
            description: String::new(),
            head: Some(lead(&role("Manager"), "Development Manager", "claude-code")),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let department = s.departments[0].id.clone();
    let s = workforce
        .create_project(&ProjectInput {
            name: "Website".into(),
            description: String::new(),
            repository_url: None,
            local_path: Some(folder.display().to_string()),
            allowed_runtimes: vec!["claude-code".into(), "codex".into()],
            capability_profile: None,
            department_id: Some(department),
            coordinator: Some(lead(
                &role("Supervisor"),
                "Website Supervisor",
                "claude-code",
            )),
        })
        .unwrap();
    let project = s.projects[0].id.clone();
    let supervisor = s.projects[0].coordinator_position_id.clone().unwrap();
    let hire = |role_name: &str, title: &str, runtime: &str| -> String {
        let s: OrgSnapshot = workforce
            .hire(&HireInput {
                role_id: role(role_name),
                title: title.into(),
                reports_to: Some(supervisor.clone()),
                runtime_id: Some(runtime.into()),
                model: None,
                vacant: None,
            })
            .unwrap();
        s.positions
            .iter()
            .find(|p| p.title == title)
            .unwrap()
            .id
            .clone()
    };
    let developer = hire("Senior Developer", "Backend Developer", "codex");
    let reviewer = hire("Code Reviewer", "Reviewer", "claude-code");
    H {
        ledger,
        rt,
        workforce,
        guard,
        broker,
        store,
        run,
        folder,
        supervisor,
        developer,
        reviewer,
        project,
        dir,
    }
}

impl H {
    fn role(&self, name: &str) -> String {
        self.workforce
            .snapshot()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.name == name)
            .unwrap()
            .id
    }

    /// Give the supervisor an objective; returns its task.
    async fn objective(&self, objective: &str) -> String {
        let d = self
            .workforce
            .give_objective(&self.supervisor, objective)
            .await
            .unwrap();
        d.turns.last().unwrap().task_id.clone()
    }

    fn task(&self, id: &str) -> Task {
        self.ledger.task(id).unwrap().unwrap()
    }

    async fn finished(&self, id: &str) -> Task {
        let deadline = Instant::now() + WAIT;
        let task = loop {
            let task = self.task(id);
            if task.state.is_terminal() {
                break task;
            }
            assert!(
                Instant::now() < deadline,
                "task {id} never finished: {task:#?}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        if let Some(session) = task.metadata["sessionId"].as_str() {
            while let Ok(detail) = self.rt.session(session).await {
                if detail.session.active_task_id.as_deref() != Some(id) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "task {id} never released its session"
                );
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
        task
    }

    /// The first child task of `parent` (a handoff's worker), once it exists.
    async fn child(&self, parent: &str) -> Task {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(c) = self.ledger.child_tasks(parent).unwrap().into_iter().next() {
                return c;
            }
            assert!(Instant::now() < deadline, "no child task of {parent}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
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

    fn events(&self, id: &str, event_type: &str) -> Vec<serde_json::Value> {
        self.ledger
            .events_for_task(id)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.payload)
            .collect()
    }

    /// The next pending approval (waits for one).
    async fn pending(&self) -> plenipo_capabilities::ApprovalView {
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

    fn set_commands(&self, approved: &[&str]) {
        let mut rules = self.guard.config().unwrap().commands;
        rules.approved = approved.iter().map(|s| (*s).to_owned()).collect();
        self.guard.set_commands(&rules).unwrap();
    }

    /// Every stored text: events, tasks, and settings, as one string.
    fn everything_recorded(&self) -> String {
        let events = self.ledger.recent_events(10_000).unwrap();
        let tasks = self.ledger.list_tasks(1000).unwrap();
        format!(
            "{events:?}{tasks:?}{}",
            self.ledger
                .setting(plenipo_guard::SETTING)
                .unwrap()
                .unwrap()
        )
    }
}

fn tool(name: &str, args: serde_json::Value) -> String {
    format!("<<tool:{name} {args}>>")
}

/// A handoff to `role` whose objective is exactly `objective`.
fn handoff(role: &str, objective: &str) -> String {
    format!("{{{{handoff:role:{role}|{objective}}}}}")
}

fn lines_of(text: &str, prefix: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

// ---- Plan tests -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_allowed_read_and_denied_write() {
    let h = harness().await;
    // The Supervisor role starts with "Read only".
    let task = h
        .objective(&format!(
            "[tools-list] {} {}",
            tool("read_file", serde_json::json!({ "path": "README.md" })),
            tool(
                "write_file",
                serde_json::json!({ "path": "notes.txt", "content": "x" })
            ),
        ))
        .await;
    assert_eq!(h.finished(&task).await.state, TaskState::Succeeded);
    let text = h.text(&task);
    // Only the tools it may use are offered.
    let tools = lines_of(&text, "Tools:");
    assert_eq!(tools.len(), 1, "{text}");
    for offered in ["read_file", "list_directory", "search_text", "git_status"] {
        assert!(tools[0].contains(offered), "{text}");
    }
    for hidden in ["write_file", "run_command", "git_commit"] {
        assert!(!tools[0].contains(hidden), "{text}");
    }
    // Allowed read.
    assert!(
        text.contains("Tool read_file: README.md (2 lines)"),
        "{text}"
    );
    // Denied write, with the reason, and nothing written.
    let denied = lines_of(&text, "Tool write_file failed:");
    assert_eq!(denied.len(), 1, "{text}");
    assert!(
        denied[0].contains(
            "Blocked: the Read only set of the Supervisor role does not allow changing files"
        ),
        "{text}"
    );
    assert!(!h.folder.join("notes.txt").exists());
    // Recorded: the grant, the use, the refusal (visible), and the grant's end.
    assert_eq!(h.events(&task, "guard.grant_opened").len(), 1);
    let used = h.events(&task, "capability.used");
    assert_eq!(used.len(), 1);
    assert_eq!(used[0]["tool"], "read_file");
    let blocked = h.events(&task, "guard.denied");
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0]["layer"], "role");
    let closed = h.events(&task, "guard.grant_closed");
    assert_eq!(
        (closed[0]["used"].as_u64(), closed[0]["blocked"].as_u64()),
        (Some(1), Some(1))
    );
    let listed = h.broker.blocked(10).unwrap();
    assert_eq!(listed[0].worker, "Website Supervisor");
    assert_eq!(listed[0].summary, "write notes.txt");
    assert!(
        h.broker.grants().is_empty(),
        "the grant ended with the step"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn acceptance_a_development_worker_works_only_in_its_workspace() {
    let h = harness().await;
    h.set_commands(&["git --version *"]);
    let work = [
        tool("read_file", serde_json::json!({ "path": "README.md" })),
        tool("write_file", serde_json::json!({ "path": "src/new.txt", "content": "hello from the worker" })),
        tool("edit_file", serde_json::json!({ "path": "src/app.txt", "oldText": "version = 1", "newText": "version = 2" })),
        tool("run_command", serde_json::json!({ "program": "git", "args": ["--version"] })),
        tool("read_file", serde_json::json!({ "path": "../outside.txt" })),
        tool("write_file", serde_json::json!({ "path": "/tmp/plenipo-escape.txt", "content": "x" })),
        tool("read_file", serde_json::json!({ "path": ".env" })),
        tool("run_command", serde_json::json!({ "program": "curl", "args": ["https://example.com"] })),
        tool("write_file", serde_json::json!({ "path": ".git/config", "content": "x" })),
    ]
    .join(" ");
    let task = h.objective(&handoff("Backend Developer", &work)).await;
    let child = h.child(&task).await;
    assert_eq!(h.finished(&child.id).await.state, TaskState::Succeeded);
    assert_eq!(h.finished(&task).await.state, TaskState::Succeeded);
    let text = h.text(&child.id);
    // Reads and writes inside its folder, and runs an approved command.
    assert!(text.contains("Tool read_file: README.md"), "{text}");
    assert!(
        text.contains("Tool write_file: Created src/new.txt"),
        "{text}"
    );
    assert!(
        text.contains("Tool edit_file: Edited src/app.txt"),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(h.folder.join("src").join("new.txt")).unwrap(),
        "hello from the worker"
    );
    assert_eq!(
        std::fs::read_to_string(h.folder.join("src").join("app.txt")).unwrap(),
        "version = 2\n"
    );
    assert!(text.contains("Tool run_command: git version"), "{text}");
    // Everything outside the folder, blocked files, blocked commands, and git internals are
    // refused, and each refusal says why.
    let failed = lines_of(&text, "Tool ");
    let refusal = |needle: &str| {
        failed
            .iter()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no refusal mentioning {needle:?} in {text}"))
            .clone()
    };
    assert!(refusal("../outside.txt").contains("outside the project folder"));
    assert!(refusal("plenipo-escape").contains("outside the project folder"));
    assert!(refusal(".env").contains("is a blocked file"));
    assert!(refusal("curl").contains("blocked commands list"));
    assert!(refusal(".git").contains("git's own files"));
    assert!(!Path::new("/tmp/plenipo-escape.txt").exists() || cfg!(windows));
    assert_eq!(h.events(&child.id, "guard.denied").len(), 5);
    assert_eq!(h.events(&child.id, "capability.used").len(), 4);
    // The command ran as a recorded program run, linked from the event.
    let ran = h
        .events(&child.id, "capability.used")
        .into_iter()
        .find(|e| e["tool"] == "run_command")
        .unwrap();
    let execution = ran["executionId"].as_str().unwrap();
    let record = h.ledger.execution(execution).unwrap().unwrap();
    assert_eq!(record.profile_id.as_deref(), Some("capability.program"));
    assert!(record
        .label
        .starts_with("Backend Developer · run git --version"));
    // The grant snapshot names the worker, its folder, and its permissions.
    let opened = &h.events(&child.id, "guard.grant_opened")[0];
    assert_eq!(opened["worker"], "Backend Developer");
    assert_eq!(opened["role"], "Senior Developer");
    assert_eq!(opened["permissions"]["filesystem.write"], "allowed");
    assert_eq!(opened["permissions"]["powershell.exec"], "ask");
    let _ = &h.project;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_approval_required_accepted_rejected_and_expired() {
    let h = harness().await;
    // Nothing is on the approved list, so running a program asks.
    h.set_commands(&[]);
    h.guard
        .set_options(&GuardOptions {
            approval_minutes: 2,
        })
        .unwrap();
    let run = tool(
        "run_command",
        serde_json::json!({ "program": "git", "args": ["--version"] }),
    );
    let work = format!("{run} {run} {run}");
    let task = h.objective(&handoff("Backend Developer", &work)).await;
    let child = h.child(&task).await;

    // 1. Approval-required: the call pauses, the task waits, the card explains.
    let first = h.pending().await;
    assert_eq!(h.task(&child.id).state, TaskState::AwaitingApproval);
    assert_eq!(first.worker, "Backend Developer");
    assert_eq!(first.role, "Senior Developer");
    assert_eq!(first.project.as_deref(), Some("Website"));
    assert_eq!(first.summary, "run git --version");
    assert_eq!(first.detail, "git --version");
    assert_eq!(first.capability, Some(Capability::ShellExec));
    assert_eq!(first.risk_label, "Runs a program");
    assert!(
        first.reason.contains("not on your approved commands list"),
        "{}",
        first.reason
    );
    assert!(first.expires_at.unwrap() > first.requested_at);
    // 2. Accepted: it runs.
    let a = h.broker.resolve_approval(&first.id, true, "owner").unwrap();
    assert_eq!(a.status, ApprovalStatus::Approved);
    assert!(
        h.broker
            .resolve_approval(&first.id, false, "owner")
            .is_err(),
        "answered once"
    );
    // 3. Rejected: it does not run.
    let second = h.pending().await;
    assert_ne!(second.id, first.id);
    h.broker
        .resolve_approval(&second.id, false, "owner")
        .unwrap();
    // 4. Expired: nobody answers within the window.
    let third = h.pending().await;
    assert_ne!(third.id, second.id);

    assert_eq!(h.finished(&child.id).await.state, TaskState::Succeeded);
    let text = h.text(&child.id);
    let results = lines_of(&text, "Tool run_command");
    assert_eq!(results.len(), 3, "{text}");
    assert!(
        results[0].starts_with("Tool run_command: git version"),
        "{text}"
    );
    assert!(
        results[1].contains("Not done: the owner did not approve it"),
        "{text}"
    );
    assert!(
        results[2].contains("Not done: the owner did not answer in time"),
        "{text}"
    );
    let queue = h.broker.approvals().unwrap();
    assert!(queue.pending.is_empty());
    let status = |id: &str| queue.recent.iter().find(|a| a.id == id).unwrap().status;
    assert_eq!(status(&first.id), ApprovalStatus::Approved);
    assert_eq!(status(&second.id), ApprovalStatus::Rejected);
    assert_eq!(status(&third.id), ApprovalStatus::Expired);
    // The task paused and continued each time, in order.
    let states: Vec<String> = h
        .events(&child.id, "task.state_changed")
        .into_iter()
        .map(|e| e["to"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        states,
        [
            "running",
            "awaitingApproval",
            "running",
            "awaitingApproval",
            "running",
            "awaitingApproval",
            "running",
            "succeeded"
        ]
    );
    assert_eq!(h.events(&child.id, "capability.used").len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn acceptance_a_sensitive_request_waits_for_approval_even_when_allowed() {
    let h = harness().await;
    // "git push" is sensitive (it leaves the computer): Guard asks even though the developer
    // may save to git. Approving lets it run (here it fails: there is no remote), refusing not.
    let push = tool("git_push", serde_json::json!({}));
    let task = h.objective(&handoff("Backend Developer", &push)).await;
    let child = h.child(&task).await;
    let card = h.pending().await;
    assert_eq!(card.capability, Some(Capability::GitWrite));
    assert_eq!(card.summary, "git push origin");
    assert_eq!(
        card.sensitive_label.as_deref(),
        Some("Sending or publishing outside this computer")
    );
    assert!(
        card.reason
            .contains("needs your approval: it sends commits to a server"),
        "{}",
        card.reason
    );
    assert_eq!(h.task(&child.id).state, TaskState::AwaitingApproval);
    // Nothing happened yet.
    assert!(h.events(&child.id, "capability.used").is_empty());
    h.broker.resolve_approval(&card.id, true, "owner").unwrap();
    assert_eq!(h.finished(&child.id).await.state, TaskState::Succeeded);
    let used = h.events(&child.id, "capability.used");
    assert_eq!(used.len(), 1, "it ran only after the approval");
    assert_eq!(used[0]["approvalId"], card.id.as_str());
}

/// Make `link` lead to `target`; false if this computer does not allow it.
fn link_out(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).unwrap();
        true
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success())
            && link.exists()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_path_traversal_attempts_are_refused() {
    let h = harness().await;
    let outside = h.dir.path().join("outside.txt").display().to_string();
    let attempts = [
        "../outside.txt",
        "src/../../outside.txt",
        outside.as_str(),
        "src/..\\..\\outside.txt",
        "src/nul",
        "\\\\?\\C:\\outside.txt",
    ];
    let work = attempts
        .iter()
        .map(|p| tool("read_file", serde_json::json!({ "path": p })))
        .collect::<Vec<_>>()
        .join(" ");
    // A link inside the folder that leads out: a symbolic link, or on Windows a directory
    // junction (which needs no administrator rights).
    let linked = link_out(h.dir.path(), &h.folder.join("up"));
    let work = if linked {
        format!(
            "{work} {}",
            tool("read_file", serde_json::json!({ "path": "up/outside.txt" }))
        )
    } else {
        work
    };
    let task = h.objective(&work).await;
    assert_eq!(h.finished(&task).await.state, TaskState::Succeeded);
    let text = h.text(&task);
    assert!(
        !text.contains("not yours"),
        "nothing outside was read: {text}"
    );
    let refusals = lines_of(&text, "Tool read_file failed: Blocked");
    assert_eq!(
        refusals.len(),
        attempts.len() + usize::from(linked),
        "{text}"
    );
    for e in h.events(&task, "guard.denied") {
        assert_eq!(e["layer"], "target", "{e}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_command_allow_and_deny_behavior() {
    let h = harness().await;
    h.set_commands(&["git --version *"]);
    let mut rules = h.guard.config().unwrap().commands;
    rules.ask.push("git --version --build-options *".into());
    h.guard.set_commands(&rules).unwrap();
    let work = [
        // Approved: runs at once.
        tool(
            "run_command",
            serde_json::json!({ "program": "git", "args": ["--version"] }),
        ),
        // Blocked: never runs (even when not installed, the rule answers first).
        tool(
            "run_command",
            serde_json::json!({ "program": "rm", "args": ["-rf", "src"] }),
        ),
        tool(
            "run_command",
            serde_json::json!({ "program": "powershell", "args": ["-c", "x"] }),
        ),
        // No shell strings: a program name with spaces is refused before Guard.
        tool(
            "run_command",
            serde_json::json!({ "program": "git --version" }),
        ),
        // On the always-ask list: asks although approved.
        tool(
            "run_command",
            serde_json::json!({ "program": "git", "args": ["--version", "--build-options"] }),
        ),
    ]
    .join(" ");
    let task = h.objective(&handoff("Backend Developer", &work)).await;
    let child = h.child(&task).await;
    let card = h.pending().await;
    assert!(card.reason.contains("always-ask list"), "{}", card.reason);
    h.broker.resolve_approval(&card.id, false, "owner").unwrap();
    h.finished(&child.id).await;
    h.finished(&task).await;
    let text = h.text(&child.id);
    let results = lines_of(&text, "Tool run_command");
    assert_eq!(results.len(), 5, "{text}");
    assert!(
        results[0].starts_with("Tool run_command: git version"),
        "{text}"
    );
    assert!(
        results[1].contains("blocked commands list (\"rm *\")"),
        "{text}"
    );
    assert!(
        results[2].contains("blocked commands list (\"powershell *\")"),
        "{text}"
    );
    assert!(results[3].contains("without spaces"), "{text}");
    assert!(results[4].contains("did not approve"), "{text}");
    assert!(h.folder.join("src").exists());
    // A reviewer may only ask to run programs.
    let task = h
        .objective(&handoff(
            "Reviewer",
            &tool(
                "run_command",
                serde_json::json!({ "program": "git", "args": ["--version"] }),
            ),
        ))
        .await;
    let child = h.child(&task).await;
    let card = h.pending().await;
    assert!(
        card.reason
            .contains("Reviewer set of the Code Reviewer role asks you before running programs"),
        "{}",
        card.reason
    );
    h.broker.resolve_approval(&card.id, true, "owner").unwrap();
    h.finished(&child.id).await;
    assert!(h.text(&child.id).contains("Tool run_command: git version"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_secret_redaction() {
    let h = harness().await;
    h.broker
        .save_secret(&SecretInput {
            name: "Deploy key".into(),
            env_var: Some("PLENIPO_TEST_SECRET".into()),
            programs: vec!["printenv".into(), "cmd".into()],
            value: Some(VAULT_VALUE.into()),
            ..SecretInput::default()
        })
        .unwrap();
    let id = h.guard.config().unwrap().secrets[0].id.clone();
    assert_eq!(h.store.get(&id).unwrap().as_deref(), Some(VAULT_VALUE));
    let mut work = vec![
        tool("read_file", serde_json::json!({ "path": "config.txt" })),
        tool(
            "read_file",
            serde_json::json!({ "path": "config.txt", "offset": 2 }),
        ),
        tool("search_text", serde_json::json!({ "query": "deploy key" })),
        // Writing a hidden part back would destroy the secret: refused.
        tool(
            "write_file",
            serde_json::json!({ "path": "copy.txt", "content": "deploy key: [hidden by Plenipo: Deploy key]" }),
        ),
    ];
    if cfg!(unix) {
        h.set_commands(&["printenv *"]);
        work.push(tool(
            "run_command",
            serde_json::json!({ "program": "printenv", "args": ["PLENIPO_TEST_SECRET"] }),
        ));
    }
    let task = h
        .objective(&handoff("Backend Developer", &work.join(" ")))
        .await;
    let child = h.child(&task).await;
    h.finished(&child.id).await;
    let text = h.text(&child.id);
    let results = lines_of(&text, "Tool ");
    assert!(
        results[0].starts_with("Tool read_file: config.txt"),
        "{text}"
    );
    // Stored secrets and recognizable ones are hidden in what the worker reads and finds.
    assert!(
        text.contains("    deploy key: [hidden by Plenipo: Deploy key]"),
        "{text}"
    );
    assert!(
        text.contains("    API_TOKEN=[hidden by Plenipo: secret setting]"),
        "{text}"
    );
    assert!(
        text.contains("config.txt:1: deploy key: [hidden by Plenipo: Deploy key]"),
        "{text}"
    );
    assert!(
        results[3].contains("hid because it looked like a secret"),
        "{text}"
    );
    assert!(!h.folder.join("copy.txt").exists());
    if cfg!(unix) {
        // The program got the secret; the worker saw only its name.
        assert!(
            results[4].contains("Given the stored secret(s) Deploy key"),
            "{text}"
        );
        assert!(
            text.contains("    [hidden by Plenipo: Deploy key]"),
            "{text}"
        );
    }
    // Neither value reached the worker's answer or anything recorded.
    let recorded = h.everything_recorded();
    for secret in [VAULT_VALUE, "tok_12345678abcdef"] {
        assert!(!text.contains(secret), "{secret} in {text}");
        assert!(!recorded.contains(secret), "{secret} was recorded");
    }
    assert!(
        std::fs::read_to_string(h.folder.join("config.txt"))
            .unwrap()
            .contains(VAULT_VALUE),
        "the file itself is untouched"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_capability_revocation_during_execution() {
    let h = harness().await;
    h.set_commands(&[]);
    let run = tool(
        "run_command",
        serde_json::json!({ "program": "git", "args": ["--version"] }),
    );
    let read = tool("read_file", serde_json::json!({ "path": "README.md" }));
    let task = h
        .objective(&handoff(
            "Backend Developer",
            &format!("{read} {run} {read}"),
        ))
        .await;
    let child = h.child(&task).await;
    // While the worker waits for an approval, the owner revokes its permissions.
    let card = h.pending().await;
    let grants = h.broker.grants();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].worker, "Backend Developer");
    assert_eq!(grants[0].used, 1);
    let revoked = h.broker.revoke(&grants[0].grant_id, "owner").unwrap();
    assert!(revoked.revoked);
    h.finished(&child.id).await;
    let text = h.text(&child.id);
    let results = lines_of(&text, "Tool ");
    assert!(
        results[0].starts_with("Tool read_file: README.md"),
        "{text}"
    );
    assert!(
        results[1].contains("did not approve"),
        "the pending approval was refused: {text}"
    );
    assert!(
        results[2].contains("permissions were revoked"),
        "later calls are blocked: {text}"
    );
    let queue = h.broker.approvals().unwrap();
    let a = queue.recent.iter().find(|a| a.id == card.id).unwrap();
    assert_eq!(a.status, ApprovalStatus::Rejected);
    assert_eq!(a.note.as_deref(), Some("Permissions revoked."));
    assert_eq!(h.events(&child.id, "guard.grant_revoked").len(), 1);
    assert!(
        h.broker.revoke(&grants[0].grant_id, "owner").is_err(),
        "the grant has ended"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_settings_change_applies_to_the_next_call() {
    let h = harness().await;
    h.set_commands(&[]);
    let run = tool(
        "run_command",
        serde_json::json!({ "program": "git", "args": ["--version"] }),
    );
    let write = tool(
        "write_file",
        serde_json::json!({ "path": "later.txt", "content": "x" }),
    );
    let task = h
        .objective(&handoff("Backend Developer", &format!("{run} {write}")))
        .await;
    let child = h.child(&task).await;
    let card = h.pending().await;
    // Take "change files" away from the Developer set while the worker runs.
    let dev = h.guard.config().unwrap().set("developer").unwrap().clone();
    let mut levels = dev.levels.clone();
    levels.remove(&Capability::FilesystemWrite);
    h.guard
        .save_set(&PermissionSetInput {
            id: Some(dev.id.clone()),
            name: dev.name.clone(),
            description: dev.description.clone(),
            levels,
        })
        .unwrap();
    h.broker.resolve_approval(&card.id, true, "owner").unwrap();
    h.finished(&child.id).await;
    let text = h.text(&child.id);
    assert!(text.contains("Tool run_command: git version"), "{text}");
    assert!(
        text.contains("Tool write_file failed: Blocked: the Developer set of the Senior Developer role does not allow changing files"),
        "{text}"
    );
    assert!(!h.folder.join("later.txt").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_and_department_limits_narrow_a_role() {
    let h = harness().await;
    // Limit the Website project to "Read only": the developer can no longer write there.
    let snapshot = h.workforce.snapshot().unwrap();
    let p = snapshot
        .projects
        .iter()
        .find(|p| p.name == "Website")
        .unwrap();
    h.workforce
        .update_project(
            &p.id,
            &ProjectInput {
                name: p.name.clone(),
                description: p.description.clone(),
                repository_url: None,
                local_path: p.local_path.clone(),
                allowed_runtimes: p.allowed_runtimes.clone(),
                capability_profile: Some("read-only".into()),
                department_id: None,
                coordinator: None,
            },
        )
        .unwrap();
    let write = tool(
        "write_file",
        serde_json::json!({ "path": "x.txt", "content": "x" }),
    );
    let task = h.objective(&handoff("Backend Developer", &write)).await;
    let child = h.child(&task).await;
    h.finished(&child.id).await;
    h.finished(&task).await;
    assert!(
        h.text(&child.id).contains(
            "Blocked: the Website project's limit (Read only) does not allow changing files"
        ),
        "{}",
        h.text(&child.id)
    );
    // A role without a permission set gets no tools at all (conversation only).
    h.guard.assign_role(&h.role("Code Reviewer"), None).unwrap();
    let task = h
        .objective(&handoff(
            "Reviewer",
            &format!(
                "[tools-list] {}",
                tool("read_file", serde_json::json!({ "path": "README.md" }))
            ),
        ))
        .await;
    let child = h.child(&task).await;
    h.finished(&child.id).await;
    assert!(
        h.text(&child.id)
            .contains("Tool read_file failed: no Plenipo tools were given"),
        "{}",
        h.text(&child.id)
    );
    assert!(h.events(&child.id, "guard.grant_opened").is_empty());
    let _ = (&h.developer, &h.reviewer);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_without_a_folder_gives_no_file_tools_and_says_so() {
    let h = harness().await;
    let snapshot = h.workforce.snapshot().unwrap();
    let p = snapshot
        .projects
        .iter()
        .find(|p| p.name == "Website")
        .unwrap();
    h.workforce
        .update_project(
            &p.id,
            &ProjectInput {
                name: p.name.clone(),
                description: p.description.clone(),
                repository_url: None,
                local_path: None,
                allowed_runtimes: p.allowed_runtimes.clone(),
                capability_profile: None,
                department_id: None,
                coordinator: None,
            },
        )
        .unwrap();
    let task = h
        .objective(&tool(
            "read_file",
            serde_json::json!({ "path": "README.md" }),
        ))
        .await;
    h.finished(&task).await;
    let skipped = h.events(&task, "guard.grant_skipped");
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0]["reason"]
        .as_str()
        .unwrap()
        .contains("the Website project has no folder"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn approvals_left_waiting_expire_when_plenipo_starts_again_and_tickets_are_single_use() {
    let h = harness().await;
    // A running task with a pending approval, as if Plenipo had stopped meanwhile.
    let t = h
        .ledger
        .create_task(
            plenipo_ledger::NewTask {
                requested_by: "owner".into(),
                objective: "left behind".into(),
                priority: 2,
                ..plenipo_ledger::NewTask::default()
            },
            "owner",
        )
        .unwrap();
    h.ledger
        .transition_task(&t.id, TaskState::Running, "w", None)
        .unwrap();
    h.ledger
        .request_action_approval(
            &t.id,
            "shell.exec",
            &serde_json::json!({ "summary": "x" }),
            u64::MAX / 2,
            "w",
        )
        .unwrap();
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(LedgerExecutionStore(Arc::clone(&h.ledger))),
        Arc::new(NoOutput),
        vec![],
    );
    let restarted = Broker::new(
        h.guard.clone(),
        sup,
        Arc::new(MemorySecretStore::default()),
        BrokerConfig::new(PathBuf::from("relay"), h.dir.path().join("tickets2")),
    );
    assert!(restarted.approvals().unwrap().pending.is_empty());
    assert!(restarted
        .snapshot()
        .unwrap()
        .notices
        .iter()
        .any(|n| n.contains("marked expired")));
    // A ticket is valid only while its step runs.
    assert_eq!(h.broker.grant_for_ticket("not-a-ticket"), None);
    let task = h
        .objective(&tool(
            "read_file",
            serde_json::json!({ "path": "README.md" }),
        ))
        .await;
    h.finished(&task).await;
    let opened = &h.events(&task, "guard.grant_opened")[0];
    let grant_id = opened["grantId"].as_str().unwrap();
    assert!(!h
        .dir
        .path()
        .join("tickets")
        .join(format!("{grant_id}.ticket.json"))
        .exists());
    // Unknown tickets are turned away by the server without an answer.
    use std::io::{Read as _, Write as _};
    let port = h.broker.port().expect("the tool server runs");
    let mut conn = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    writeln!(conn, "{{\"ticket\":\"bogus\"}}").unwrap();
    writeln!(
        conn,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}}"
    )
    .unwrap();
    let mut answer = Vec::new();
    let n = conn.read_to_end(&mut answer).unwrap_or(0);
    assert_eq!(n, 0, "no answer: {}", String::from_utf8_lossy(&answer));
}
