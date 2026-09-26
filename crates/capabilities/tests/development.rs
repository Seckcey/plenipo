//! Phase 8 Development Department tests: the whole stack — Workforce, Router, Liaison, the
//! agent runtime and supervisor, Guard, the broker with its tool server and relay, and a
//! file-backed Ledger — driving `plenipo-fake-agent` installed as `claude` and `codex`, on a
//! real git repository. Workers follow scripts (`script.json`, one step per turn, by position
//! title), so every synthetic development scenario of the plan runs the same way every time.
//! No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_capabilities::{Broker, BrokerConfig, MemorySecretStore};
use plenipo_guard::Guard;
use plenipo_ledger::{Ledger, Task, TaskState, WorkspaceState, DB_FILE_NAME};
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
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(90);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

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

/// The other programs it stands in for (`--helpers`): GitHub's `gh`.
fn helpers() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_plenipo-fake-agent-capabilities"))
            .arg("--helpers")
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
            .join(format!("development-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas().iter().chain(helpers()) {
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

/// Run git for the test's own setup and checks; its output.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct NoUpdates;

impl AgentSink for NoUpdates {
    fn emit(&self, _: AgentUpdate) {}
}

/// Positions of the Development department.
struct Team {
    vp: String,
    supervisor: String,
    developer: String,
    frontend: String,
    reviewer: String,
    qa: String,
    writer: String,
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
    /// The project's folder: the owner's own git checkout.
    folder: PathBuf,
    project: String,
    team: Team,
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

/// Development, headed by the Development VP (Claude Code), runs the Website project: its
/// folder is a git repository on `main` with one commit, and its Website Supervisor (Claude
/// Code) leads a Backend Developer (Senior Developer, Codex), a Frontend Developer (Senior
/// Developer, Codex), a Reviewer (Code Reviewer, Claude Code), QA (QA Engineer, Claude Code),
/// and Docs (Documentation Writer, Claude Code).
async fn harness() -> H {
    harness_with(true).await
}

async fn harness_with(branch_per_objective: bool) -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".plenipo-fake-agent")).unwrap();
    for stem in personas().iter().chain(helpers()) {
        install_fake(&bin, stem);
    }
    let folder = dir.path().join("website");
    std::fs::create_dir_all(folder.join("src")).unwrap();
    std::fs::write(folder.join("README.md"), "# Website\n").unwrap();
    std::fs::write(folder.join("src").join("app.txt"), "version = 1\n").unwrap();
    std::fs::write(
        folder.join("check.sh"),
        "#!/bin/sh\n# The project's test: passes once src/fix.txt says fixed.\ngrep -q fixed src/fix.txt\n",
    )
    .unwrap();
    git(&folder, &["init", "-q", "-b", "main"]);
    git(&folder, &["config", "user.name", "Plenipo Test"]);
    git(&folder, &["config", "user.email", "test@example.com"]);
    git(&folder, &["add", "-A"]);
    git(&folder, &["commit", "-q", "-m", "Start"]);
    // Its remote server: a bare repository next to it.
    let origin = dir.path().join("origin.git");
    git(dir.path(), &["init", "-q", "--bare", "origin.git"]);
    git(
        &folder,
        &["remote", "add", "origin", &origin.display().to_string()],
    );
    git(&folder, &["push", "-q", "origin", "main"]);

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
    let guard = Guard::new(Arc::clone(&ledger));
    guard.seed_template_roles().unwrap();
    let store = Arc::new(MemorySecretStore::default());
    let mut broker_config = BrokerConfig::new(
        PathBuf::from(env!("CARGO_BIN_EXE_plenipo-tool-relay")),
        dir.path().join("tickets"),
    );
    broker_config.approval_minute = Duration::from_secs(1);
    // GitHub's gh is the stand-in in the test's own folder.
    broker_config.search_path = Some(bin.clone().into_os_string());
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
            head: Some(lead(&role("VP"), "Development VP", "claude-code")),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let department = s.departments[0].id.clone();
    let vp = s.departments[0].head_position_id.clone().unwrap();
    let s = workforce
        .create_project(&ProjectInput {
            name: "Website".into(),
            description: String::new(),
            repository_url: Some("https://github.com/example/website".into()),
            local_path: Some(folder.display().to_string()),
            allowed_runtimes: vec!["claude-code".into(), "codex".into()],
            capability_profile: None,
            branch_per_objective: Some(branch_per_objective),
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
    let team = Team {
        vp,
        developer: hire("Senior Developer", "Backend Developer", "codex"),
        frontend: hire("Senior Developer", "Frontend Developer", "codex"),
        reviewer: hire("Code Reviewer", "Reviewer", "claude-code"),
        qa: hire("QA Engineer", "QA", "claude-code"),
        writer: hire("Documentation Writer", "Docs", "claude-code"),
        supervisor,
    };
    H {
        ledger,
        rt,
        workforce,
        guard,
        broker,
        store,
        run,
        folder,
        project,
        team,
        dir,
    }
}

impl H {
    /// What each position does, turn by turn (see the fake CLI's scripts).
    fn script(&self, script: &Value) {
        let dir = self.dir.path().join("home").join(".plenipo-fake-agent");
        let _ = std::fs::remove_dir_all(dir.join("script-used"));
        std::fs::write(dir.join("script.json"), script.to_string()).unwrap();
    }

    /// Give `position` an objective; returns its task.
    async fn objective(&self, position: &str, objective: &str) -> String {
        let d = self
            .workforce
            .give_objective(position, objective, None)
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

    /// Every task of the objective `root`, with the position each worked as.
    fn tree(&self, root: &str) -> Vec<(Task, String)> {
        self.ledger
            .descendant_tasks(root)
            .unwrap()
            .into_iter()
            .map(|(t, _)| {
                let title = t.metadata["workforce"]["positionId"]
                    .as_str()
                    .and_then(|p| self.ledger.position(p).ok().flatten())
                    .map(|p| p.title)
                    .unwrap_or_default();
                (t, title)
            })
            .collect()
    }

    /// The tasks `title` worked on in the objective `root`.
    fn tasks_of(&self, root: &str, title: &str) -> Vec<Task> {
        self.tree(root)
            .into_iter()
            .filter(|(_, t)| t == title)
            .map(|(t, _)| t)
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
}

impl H {
    /// The fake gh's state (its pull requests, and the variables of its last run).
    fn gh_state(&self) -> PathBuf {
        self.dir.path().join("bin").join("gh-state")
    }

    fn pull_requests(&self) -> Vec<Value> {
        std::fs::read_to_string(self.gh_state().join("prs.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
}

fn tool(name: &str, args: Value) -> Value {
    json!([name, args])
}

fn to(role: &str, objective: &str) -> Value {
    json!({ "to": format!("role:{role}"), "objective": objective })
}

// ---- Working copies (a branch per objective) --------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_objective_gets_its_own_branch_and_working_copy_that_its_team_shares() {
    let h = harness().await;
    h.script(&json!({
        "Website Supervisor": [
            { "say": "Handing it to the developer.",
              "handoffs": [to("Backend Developer", "Add src/feature.txt and commit it.")] },
            { "say": "Now a review.",
              "handoffs": [to("Reviewer", "Review src/feature.txt.")] },
            { "say": "Feature added and reviewed." }
        ],
        "Backend Developer": [
            { "say": "Added the feature.",
              "tools": [
                  tool("write_file", json!({ "path": "src/feature.txt", "content": "feature on\n" })),
                  tool("git_add", json!({ "paths": ["src/feature.txt"] })),
                  tool("git_commit", json!({ "message": "Add the feature" })),
                  tool("git_status", json!({})),
              ] }
        ],
        "Reviewer": [
            { "say": "Looks right.",
              "tools": [tool("read_file", json!({ "path": "src/feature.txt" }))],
              "review": { "verdict": "approve", "findings": [] } }
        ]
    }));
    let root = h
        .objective(&h.team.supervisor, "Add the feature flag")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.text(&root), "Feature added and reviewed.");

    // The owner's own checkout is untouched: still on main, clean, no new file.
    assert!(!h.folder.join("src").join("feature.txt").exists());
    assert_eq!(git(&h.folder, &["branch", "--show-current"]), "main");
    assert_eq!(git(&h.folder, &["status", "--porcelain"]), "");

    // One working copy, on the objective's branch, recorded with what is on it.
    let correlation = h.task(&root).metadata["liaison"]["correlationId"]
        .as_str()
        .unwrap()
        .to_owned();
    let copies = h.ledger.workflow_workspaces(&correlation).unwrap();
    assert_eq!(copies.len(), 1, "{copies:#?}");
    let w = &copies[0];
    assert!(
        w.branch.starts_with("plenipo/add-the-feature-flag-"),
        "{}",
        w.branch
    );
    assert_eq!(w.base_ref.as_deref(), Some("main"));
    assert_eq!(w.root_task_id.as_deref(), Some(root.as_str()));
    assert_eq!(w.project_id, h.project);
    assert_eq!(
        std::fs::read_to_string(Path::new(&w.path).join("src").join("feature.txt")).unwrap(),
        "feature on\n"
    );
    assert_eq!(w.facts.commits.len(), 1);
    assert_eq!(w.facts.commits[0].subject, "Add the feature");
    assert!(
        w.facts
            .files
            .iter()
            .any(|f| f.path == "src/feature.txt" && f.committed && f.added == Some(1)),
        "{:#?}",
        w.facts
    );
    assert_eq!(w.facts.uncommitted, 0);
    // The branch is in the owner's repository, with the commit.
    assert_eq!(
        git(&h.folder, &["log", "--format=%s", "-n", "1", &w.branch]),
        "Add the feature"
    );

    // Both workers worked there: the reviewer read the developer's file.
    let dev = &h.tasks_of(&root, "Backend Developer")[0];
    let reviewer = &h.tasks_of(&root, "Reviewer")[0];
    for t in [dev, reviewer] {
        let opened = h.events(&t.id, "guard.grant_opened");
        assert_eq!(opened[0]["workspace"]["branch"], w.branch.as_str());
        assert_eq!(opened[0]["folder"], w.folder().display().to_string());
    }
    assert_eq!(
        h.events(&dev.id, "guard.grant_opened")[0]["workspace"]["changesFiles"],
        true
    );
    let read = h.text(&reviewer.id);
    assert!(
        read.contains("Tool read_file: src/feature.txt") && read.contains("feature on"),
        "{read}"
    );
    // In the trail: made for the first step that needed the folder (the supervisor's, which
    // may read files), and updated after the developer's commit.
    assert_eq!(h.events(&root, "workspace.created").len(), 1);
    assert!(!h.events(&dev.id, "workspace.updated").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workers_stay_on_their_objectives_branch() {
    let h = harness().await;
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [to("Backend Developer", "Try other branches.")] },
            { "say": "Done." }
        ],
        "Backend Developer": [
            { "say": "Tried.",
              "tools": [
                  tool("git_branch", json!({ "name": "main" })),
                  tool("git_branch", json!({ "name": "hotfix", "create": true })),
                  tool("git_push", json!({ "branch": "main" })),
                  tool("git_push", json!({})),
              ] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Branch games").await;
    // The push of its own branch still asks the owner: deny it.
    let approval = h.pending().await;
    assert!(
        approval
            .summary
            .starts_with("git push origin plenipo/branch-games-"),
        "{}",
        approval.summary
    );
    h.broker
        .resolve_approval(&approval.id, false, "owner")
        .unwrap();
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let dev = &h.tasks_of(&root, "Backend Developer")[0];
    let text = h.text(&dev.id);
    let refusals: Vec<&str> = text
        .lines()
        .filter(|l| {
            l.starts_with("Tool git_branch failed") || l.starts_with("Tool git_push failed")
        })
        .collect();
    assert_eq!(refusals.len(), 4, "{text}");
    assert!(refusals[0].contains("stays on its own branch"), "{text}");
    assert!(refusals[1].contains("stays on its own branch"), "{text}");
    assert!(
        refusals[2].contains("only this objective's branch"),
        "{text}"
    );
    assert!(refusals[3].contains("not approve"), "{text}");
    // Each refusal is recorded as blocked; nothing was pushed or switched.
    assert_eq!(h.events(&dev.id, "guard.denied").len(), 3);
    assert_eq!(git(&h.folder, &["branch", "--show-current"]), "main");
    assert!(git(&h.folder, &["branch", "--list", "hotfix"]).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_worker_changing_files_at_the_same_time_gets_its_own_working_copy() {
    let h = harness().await;
    h.script(&json!({
        "Website Supervisor": [
            { "say": "Two at once.",
              "handoffs": [
                  to("Backend Developer", "Write the API."),
                  to("Frontend Developer", "Write the page."),
              ] },
            { "say": "Both done." }
        ],
        "Backend Developer": [
            { "say": "API written.", "delay": 2500,
              "tools": [tool("write_file", json!({ "path": "src/api.txt", "content": "api\n" }))] }
        ],
        "Frontend Developer": [
            { "say": "Page written.", "delay": 2500,
              "tools": [tool("write_file", json!({ "path": "src/page.txt", "content": "page\n" }))] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Build both halves").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let correlation = h.task(&root).metadata["liaison"]["correlationId"]
        .as_str()
        .unwrap()
        .to_owned();
    let copies = h.ledger.workflow_workspaces(&correlation).unwrap();
    assert_eq!(copies.len(), 2, "{copies:#?}");
    let (first, second) = (&copies[0], &copies[1]);
    assert_eq!(second.parent_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(second.branch, format!("{}-2", first.branch));
    assert_eq!(second.base_ref.as_deref(), Some(first.branch.as_str()));
    // Each worker's file is in its own working copy only.
    let written = |w: &plenipo_ledger::Workspace, name: &str| {
        Path::new(&w.path).join("src").join(name).exists()
    };
    let (api_first, page_first) = (written(first, "api.txt"), written(first, "page.txt"));
    assert!(api_first ^ page_first, "one writer per working copy");
    assert_eq!(written(second, "api.txt"), !api_first);
    assert_eq!(written(second, "page.txt"), !page_first);
    // The worker in the second copy was told why, and to merge.
    let second_worker = h
        .tree(&root)
        .into_iter()
        .map(|(t, _)| t)
        .find(|t| {
            h.events(&t.id, "guard.grant_opened")
                .first()
                .is_some_and(|o| o["workspace"]["branch"] == second.branch.as_str())
        })
        .expect("a worker in the second working copy");
    let note = &h.events(&second_worker.id, "guard.grant_opened")[0]["note"];
    assert!(
        note.as_str().unwrap().contains("must be merged into"),
        "{note}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_can_work_in_its_folder_instead() {
    let h = harness_with(false).await;
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [to("Backend Developer", "Write it here.")] },
            { "say": "Done." }
        ],
        "Backend Developer": [
            { "tools": [tool("write_file", json!({ "path": "src/here.txt", "content": "here\n" }))] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Work in place").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(
        std::fs::read_to_string(h.folder.join("src").join("here.txt")).unwrap(),
        "here\n"
    );
    assert!(h
        .ledger
        .project_workspaces(&h.project, 10)
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removing_a_finished_objectives_working_copy_keeps_its_branch() {
    let h = harness().await;
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [to("Backend Developer", "Commit a note.")] },
            { "say": "Done." }
        ],
        "Backend Developer": [
            { "delay": 1500,
              "tools": [
                  tool("write_file", json!({ "path": "note.txt", "content": "note\n" })),
                  tool("git_add", json!({ "paths": ["note.txt"] })),
                  tool("git_commit", json!({ "message": "Add a note" })),
              ] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Leave a note").await;
    // While the objective runs, its working copy cannot be removed.
    let deadline = Instant::now() + WAIT;
    let w = loop {
        if let Some(w) = h
            .ledger
            .project_workspaces(&h.project, 1)
            .unwrap()
            .into_iter()
            .next()
        {
            break w;
        }
        assert!(Instant::now() < deadline, "no working copy was made");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(h.broker.remove_workspace(&w.id, "owner").is_err());
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let removed = h.broker.remove_workspace(&w.id, "owner").unwrap();
    assert_eq!(removed.state, WorkspaceState::Removed);
    assert!(!Path::new(&w.path).exists());
    assert_eq!(
        git(&h.folder, &["log", "--format=%s", "-n", "1", &w.branch]),
        "Add a note",
        "the branch stays"
    );
    assert_eq!(h.events(&root, "workspace.removed").len(), 1);
    // Removing it again changes nothing.
    assert_eq!(
        h.broker.remove_workspace(&w.id, "owner").unwrap().state,
        WorkspaceState::Removed
    );
}

// ---- GitHub ----------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_developer_opens_a_draft_pull_request_for_its_branch_only_after_approval() {
    let h = harness().await;
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [to("Backend Developer", "Build the login page and open a pull request.")] },
            { "say": "The pull request is open." }
        ],
        "Backend Developer": [
            { "say": "Opened the pull request.",
              "tools": [
                  tool("github_issue_view", json!({ "number": 12 })),
                  tool("write_file", json!({ "path": "src/login.txt", "content": "login\n" })),
                  tool("git_add", json!({ "paths": ["src/login.txt"] })),
                  tool("git_commit", json!({ "message": "Add the login page" })),
                  tool("github_pr_create", json!({
                      "title": "Add the login page",
                      "body": "Adds src/login.txt. Tested by hand."
                  })),
                  tool("github_pr_view", json!({})),
                  tool("github_pr_checks", json!({})),
                  tool("github_pr_list", json!({})),
              ] }
        ]
    }));
    let root = h
        .objective(&h.team.supervisor, "Build the login page")
        .await;
    // Publishing waits for the owner; nothing is pushed before.
    let approval = h.pending().await;
    assert!(
        approval.summary.starts_with(
            "open a draft pull request \"Add the login page\" from plenipo/build-the-login-page-"
        ),
        "{}",
        approval.summary
    );
    assert!(
        approval.summary.ends_with(" into main on example/website"),
        "{}",
        approval.summary
    );
    assert!(approval.detail.contains("git push -u origin plenipo/"));
    assert!(h.pull_requests().is_empty());
    assert!(git(
        &h.dir.path().join("origin.git"),
        &["branch", "--list", "plenipo/*"]
    )
    .is_empty());
    h.broker
        .resolve_approval(&approval.id, true, "owner")
        .unwrap();
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);

    let prs = h.pull_requests();
    assert_eq!(prs.len(), 1);
    let branch = prs[0]["headRefName"].as_str().unwrap().to_owned();
    assert!(
        branch.starts_with("plenipo/build-the-login-page-"),
        "{branch}"
    );
    assert_eq!(prs[0]["baseRefName"], "main");
    assert_eq!(prs[0]["isDraft"], true);
    // The branch reached the remote server, with the commit.
    assert_eq!(
        git(
            &h.dir.path().join("origin.git"),
            &["log", "--format=%s", "-n", "1", &branch]
        ),
        "Add the login page"
    );
    let dev = &h.tasks_of(&root, "Backend Developer")[0];
    let used = h.events(&dev.id, "capability.used");
    let created = used
        .iter()
        .find(|u| u["tool"] == "github_pr_create")
        .unwrap();
    assert_eq!(created["ok"], true);
    assert_eq!(
        created["pullRequest"]["url"],
        "https://github.com/example/website/pull/1"
    );
    assert_eq!(created["approvalId"], approval.id.as_str());
    let text = h.text(&dev.id);
    for line in [
        "Tool github_issue_view:",
        "Tool github_pr_create: https://github.com/example/website/pull/1",
        "Tool github_pr_view:",
        "Tool github_pr_checks:",
        "Tool github_pr_list:",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
    assert!(text.contains("\"bucket\":\"pass\""), "{text}");
    // The working copy knows its branch was pushed.
    let w = h
        .ledger
        .project_workspaces(&h.project, 1)
        .unwrap()
        .remove(0);
    assert!(w.facts.pushed, "{:#?}", w.facts);
    // gh never waits for input or prints colors.
    let env = std::fs::read_to_string(h.gh_state().join("last-env.txt")).unwrap();
    for var in ["GH_PROMPT_DISABLED", "NO_COLOR"] {
        assert!(env.lines().any(|l| l == var), "{var}: {env}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn github_tools_use_only_the_projects_repository_and_the_workers_permissions() {
    let h = harness().await;
    std::fs::create_dir_all(h.gh_state()).unwrap();
    std::fs::write(h.gh_state().join("signed-out"), "").unwrap();
    // The owner gives gh a GitHub token from Secrets; the worker never sees it.
    h.broker
        .save_secret(&plenipo_guard::SecretInput {
            name: "GitHub token".into(),
            env_var: Some("GH_TOKEN".into()),
            programs: vec!["gh".into()],
            value: Some("ghp_testtoken1234567890abcdefghijklmnop".into()),
            ..plenipo_guard::SecretInput::default()
        })
        .unwrap();
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [
                to("Reviewer", "Check the open pull requests."),
            ] },
            { "say": "Checked." }
        ],
        "Reviewer": [
            { "say": "Checked.",
              "tools": [
                  tool("github_pr_list", json!({ "state": "all" })),
                  tool("github_pr_create", json!({ "title": "Sneaky" })),
              ] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Check pull requests").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let reviewer = &h.tasks_of(&root, "Reviewer")[0];
    let text = h.text(&reviewer.id);
    // Reading works with the stored token (gh itself is signed out)...
    assert!(text.contains("Tool github_pr_list:"), "{text}");
    assert!(
        text.contains("Given the stored secret(s) GitHub token"),
        "{text}"
    );
    // ...but a reviewer may not open pull requests.
    assert!(text.contains("Tool github_pr_create failed"), "{text}");
    let denied = h.events(&reviewer.id, "guard.denied");
    assert_eq!(denied.len(), 1, "{denied:?}");
    assert_eq!(denied[0]["layer"], "role");
    assert!(h.pull_requests().is_empty());
    let recorded = format!("{:?}", h.ledger.events_for_task(&reviewer.id).unwrap());
    assert!(
        !recorded.contains("ghp_testtoken"),
        "the token is never recorded"
    );

    // Without a GitHub address for the project, GitHub tools refuse.
    let project = h.workforce.snapshot().unwrap().projects.remove(0);
    h.workforce
        .update_project(
            &h.project,
            &ProjectInput {
                name: project.name,
                description: project.description,
                repository_url: Some("https://gitlab.com/example/website".into()),
                local_path: project.local_path,
                allowed_runtimes: project.allowed_runtimes,
                capability_profile: None,
                branch_per_objective: None,
                department_id: None,
                coordinator: None,
            },
        )
        .unwrap();
    h.script(&json!({
        "Website Supervisor": [
            { "handoffs": [to("Reviewer", "Check again.")] },
            { "say": "Checked." }
        ],
        "Reviewer": [
            { "tools": [tool("github_pr_list", json!({}))] }
        ]
    }));
    let root = h.objective(&h.team.supervisor, "Check again").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let reviewer = &h.tasks_of(&root, "Reviewer")[0];
    assert!(
        h.text(&reviewer.id).contains("not on GitHub"),
        "{}",
        h.text(&reviewer.id)
    );
}
