//! Phase 5 Workforce and Phase 6 routing tests: the real Workforce, Router, Liaison, agent
//! runtime, supervisor, adapters, and a file-backed Ledger, driving `plenipo-fake-agent`
//! installed as `claude` and `codex`. Every plan test is covered, and the acceptance scenarios
//! end to end. No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_ledger::{AgentLifecycle, Ledger, Task, TaskState, DB_FILE_NAME};
use plenipo_liaison::store::{LedgerExecutionStore, LedgerSessionStore};
use plenipo_liaison::Directory as _;
use plenipo_liaison::{Liaison, LiaisonConfig};
use plenipo_router::{CrossCompany, LimitBehavior, ModelInput, Router, RoutingOptions};
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, Effort, HostEnv,
    TurnResult,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};
use plenipo_workforce::directory::WorkforceDirectory;
use plenipo_workforce::{
    DepartmentInput, HireInput, LeadInput, OrgSnapshot, OversightRole, PositionInfo, PositionKind,
    PositionStatus, ProjectInput, RoleInput, RoleJob, RoleUpdate, Staffing, Workforce,
    WorkforceError,
};

const WAIT: Duration = Duration::from_secs(60);
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
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_plenipo-fake-agent-workforce"))
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

/// One copy of the fake CLIs per test process, ready to execute (hard links later never open
/// a write handle, which avoids ETXTBSY while other tests fork).
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("workforce-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(env!("CARGO_BIN_EXE_plenipo-fake-agent-workforce"), &path).unwrap();
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
    sup: Supervisor,
    liaison: Liaison,
    router: Router,
    workforce: Workforce,
    run: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
}

/// Everything above the Ledger, as the desktop app wires it.
async fn stack(
    dir: &Path,
) -> (
    Arc<Ledger>,
    AgentRuntime,
    Supervisor,
    Liaison,
    Router,
    Workforce,
) {
    let ledger = Arc::new(Ledger::open(&dir.join("ledger").join(DB_FILE_NAME)).unwrap());
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(LedgerExecutionStore(Arc::clone(&ledger))),
        Arc::new(NoOutput),
        vec![],
    );
    let mut config = AgentConfig::new(dir.join("workspaces"));
    config.extra_env = vec![(HOME_VAR.into(), dir.join("home").display().to_string())];
    config.turn_timeout = Duration::from_secs(120);
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup.clone(),
        Arc::new(LedgerSessionStore(Arc::clone(&ledger))),
        Arc::new(NoUpdates),
        HostEnv::new(
            Some(dir.join("bin").into_os_string()),
            Some(dir.join("home")),
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
    (ledger, rt, sup, liaison, router, workforce)
}

async fn harness() -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".plenipo-fake-agent")).unwrap();
    for stem in personas() {
        install_fake(&bin, stem);
    }
    let (ledger, rt, sup, liaison, router, workforce) = stack(dir.path()).await;
    let run = tokio::spawn(liaison.clone().run());
    H {
        ledger,
        rt,
        sup,
        liaison,
        router,
        workforce,
        run,
        dir,
    }
}

/// The organization most tests use: Development (headed by a Development Manager on Claude
/// Code) with the project Cloudline (coordinator on Claude Code), a Senior Developer on Codex
/// on the coordinator's team, and a QA Engineer on Claude Code under the manager, assigned as
/// Cloudline's QA evaluator.
struct Org {
    department: String,
    project: String,
    head: String,
    coordinator: String,
    developer: String,
    qa: String,
}

fn lead(role_id: &str, title: &str, runtime: &str) -> LeadInput {
    LeadInput {
        role_id: role_id.into(),
        title: title.into(),
        runtime_id: Some(runtime.into()),
        model: None,
        vacant: None,
        from_workforce: None,
    }
}

fn project_input(name: &str, runtimes: &[&str]) -> ProjectInput {
    ProjectInput {
        name: name.into(),
        description: format!("The {name} product"),
        repository_url: Some(format!(
            "https://github.com/example/{}",
            name.to_lowercase()
        )),
        local_path: Some(format!("D:\\projects\\{}", name.to_lowercase())),
        allowed_runtimes: runtimes.iter().map(|r| (*r).to_owned()).collect(),
        capability_profile: Some("development".into()),
        branch_per_objective: None,
        department_id: None,
        coordinator: None,
    }
}

impl H {
    fn snapshot(&self) -> OrgSnapshot {
        self.workforce.snapshot().unwrap()
    }

    fn role(&self, name: &str) -> String {
        self.snapshot()
            .roles
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no role {name}"))
            .id
    }

    fn position(&self, id: &str) -> PositionInfo {
        self.snapshot()
            .positions
            .into_iter()
            .find(|p| p.id == id)
            .unwrap()
    }

    fn id_of(s: &OrgSnapshot, title: &str) -> String {
        s.positions
            .iter()
            .find(|p| p.title == title && p.active)
            .unwrap_or_else(|| panic!("no position {title}"))
            .id
            .clone()
    }

    fn hire(&self, role: &str, title: &str, reports_to: &str, runtime: &str) -> String {
        let s = self
            .workforce
            .hire(&HireInput {
                role_id: self.role(role),
                title: title.into(),
                reports_to: Some(reports_to.into()),
                runtime_id: Some(runtime.into()),
                model: None,
                vacant: None,
                specialty_id: None,
            })
            .unwrap();
        Self::id_of(&s, title)
    }

    fn department(&self, name: &str, head_title: &str, runtime: &str) -> (String, String) {
        let s = self
            .workforce
            .create_department(&DepartmentInput {
                name: name.into(),
                description: format!("{name} work"),
                head: Some(lead(&self.role("Manager"), head_title, runtime)),
                reports_to: None,
                active: None,
            })
            .unwrap();
        let d = s.departments.iter().find(|d| d.name == name).unwrap();
        (d.id.clone(), d.head_position_id.clone().unwrap())
    }

    fn development(&self) -> Org {
        let (department, head) =
            self.department("Development", "Development Manager", "claude-code");
        let s = self
            .workforce
            .create_project(&ProjectInput {
                department_id: Some(department.clone()),
                coordinator: Some(lead(
                    &self.role("Supervisor"),
                    "Cloudline Coordinator",
                    "claude-code",
                )),
                ..project_input("Cloudline", &["claude-code", "codex"])
            })
            .unwrap();
        let project = s.projects.iter().find(|p| p.name == "Cloudline").unwrap();
        let coordinator = project.coordinator_position_id.clone().unwrap();
        let developer = self.hire(
            "Senior Developer",
            "Senior Developer",
            &coordinator,
            "codex",
        );
        let qa = self.hire("QA Engineer", "QA Engineer", &head, "claude-code");
        self.workforce
            .assign_oversight(&qa, &coordinator, OversightRole::Qa)
            .unwrap();
        Org {
            department,
            project: project.id.clone(),
            head,
            coordinator,
            developer,
            qa,
        }
    }

    /// Give `position` an objective; returns the turn's task ID.
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

    /// Wait until task `id` has finished and its session no longer holds it. A turn's task is
    /// recorded as finished a moment before the runtime releases the session; a follow-up sent
    /// in that moment is refused as "already running".
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

    /// Wait until the snapshot satisfies `pred`; returns that snapshot.
    async fn until(&self, what: &str, pred: impl Fn(&OrgSnapshot) -> bool) -> OrgSnapshot {
        let deadline = Instant::now() + WAIT;
        loop {
            let s = self.snapshot();
            if pred(&s) {
                return s;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
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

    fn types(&self, id: &str) -> Vec<String> {
        self.ledger
            .events_for_task(id)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect()
    }

    fn rejections(&self, task_id: &str) -> Vec<String> {
        self.ledger
            .liaison_messages_for_task(task_id)
            .unwrap()
            .into_iter()
            .filter_map(|m| m.envelope["rejection"].as_str().map(str::to_owned))
            .collect()
    }
}

fn refusal<T: std::fmt::Debug>(r: Result<T, WorkforceError>) -> String {
    match r {
        Err(e) if e.is_caller_error() => e.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `expected` occurs in `types` in this order (other events may come between).
fn assert_in_order(types: &[String], expected: &[&str]) {
    let mut rest = types.iter();
    for want in expected {
        assert!(
            rest.any(|t| t == want),
            "{want} missing or out of order in {types:#?}"
        );
    }
}

// ---- Plan tests -----------------------------------------------------------------------------

/// A custom role's working instructions in the owner's own words (ADR-019): cleaned, shown in
/// the organization, changeable, and used in its workers' instructions like a built-in role's.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_custom_role_gets_its_owners_working_instructions() {
    let h = harness().await;
    let lines = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let s = h
        .workforce
        .create_role(&RoleInput {
            name: "Bookkeeper".into(),
            description: "Keeps the books.".into(),
            kind: PositionKind::Worker,
            staffing: Staffing::OnDemand,
            job: Some(RoleJob {
                duties: lines(&["- enter the month's receipts\n* match them to the bank", ""]),
                returns: lines(&["a list of what was entered"]),
                limits: lines(&["never pay a bill"]),
                ask_lead: lines(&["a receipt is unreadable"]),
            }),
        })
        .unwrap();
    let role = s.roles.iter().find(|r| r.name == "Bookkeeper").unwrap();
    assert_eq!(
        role.job.duties,
        ["enter the month's receipts", "match them to the bank"],
        "one item per line, list markers and blank lines removed"
    );
    assert_eq!(role.job.limits, ["never pay a bill"]);
    // Built-in roles show their instructions too.
    let dev = s
        .roles
        .iter()
        .find(|r| r.name == "Senior Developer")
        .unwrap();
    assert!(!dev.job.duties.is_empty() && !dev.job.limits.is_empty());

    let changed = h
        .workforce
        .update_role(
            &role.id,
            &RoleUpdate {
                name: "Bookkeeper".into(),
                description: "Keeps the books and files receipts.".into(),
                job: RoleJob {
                    duties: lines(&["file receipts by month"]),
                    ..role.job.clone()
                },
            },
        )
        .unwrap();
    let role = changed
        .roles
        .iter()
        .find(|r| r.name == "Bookkeeper")
        .unwrap();
    assert_eq!(role.job.duties, ["file receipts by month"]);
    assert_eq!(role.description, "Keeps the books and files receipts.");
    // Built-in roles keep theirs; lines are limited.
    assert!(refusal(h.workforce.update_role(
        &h.role("Senior Developer"),
        &RoleUpdate {
            name: "Senior Developer".into(),
            description: String::new(),
            job: RoleJob::default(),
        },
    ))
    .contains("built-in roles keep their instructions"));
    let long = RoleJob {
        duties: vec!["x".repeat(400)],
        ..RoleJob::default()
    };
    assert!(refusal(h.workforce.update_role(
        &role.id,
        &RoleUpdate {
            name: "Bookkeeper".into(),
            description: String::new(),
            job: long,
        },
    ))
    .contains("at most 300 characters"));
    let many = RoleJob {
        limits: (0..13).map(|i| format!("rule {i}")).collect(),
        ..RoleJob::default()
    };
    assert!(refusal(h.workforce.create_role(&RoleInput {
        name: "Clerk".into(),
        description: String::new(),
        kind: PositionKind::Worker,
        staffing: Staffing::OnDemand,
        job: Some(many),
    }))
    .contains("at most 12 lines"));
}

/// Workers learn from their work (ADR-024): a lesson in a worker's answer waits for the owner;
/// kept, it is in the instructions of the role's later workers; a role that learns on its own
/// keeps them at once; with learning switched off, nothing is recorded or used.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workers_learn_lessons_the_owner_keeps() {
    use plenipo_ledger::LessonState;
    use plenipo_workforce::learning;
    let h = harness().await;
    let org = h.development();
    let supervisor = h.role("Supervisor");
    let answer = |lesson: &str| {
        let script = serde_json::json!({
            "Cloudline Coordinator": [
                { "say": format!("Done.\n```plenipo-lesson\n- {lesson}\n```") }
            ]
        });
        let dir = h.dir.path().join("home").join(".plenipo-fake-agent");
        // A new script starts from its first step.
        let _ = std::fs::remove_dir_all(dir.join("script-used"));
        std::fs::write(dir.join("script.json"), script.to_string()).unwrap();
    };
    let lessons = |state: LessonState| {
        h.ledger
            .lessons(state, Some(&supervisor), 20)
            .unwrap()
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
    };
    let until = |what: &str, pred: &dyn Fn() -> bool| {
        let deadline = Instant::now() + WAIT;
        while !pred() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    // The instructions ask for lessons, and there are none yet.
    let told = learning::instructions(
        &h.ledger,
        &h.ledger.position(&org.coordinator).unwrap().unwrap(),
        "Supervisor",
    );
    assert!(told.contains("```plenipo-lesson"), "{told}");
    assert!(!told.contains("have learned"));

    answer("Read the release notes before planning.");
    let task = h.objective(&org.coordinator, "Plan the release.").await;
    h.finished(&task).await;
    until("the lesson", &|| !lessons(LessonState::Waiting).is_empty());
    let snap = h.workforce.learning().unwrap();
    let waiting = &snap.waiting[0];
    assert_eq!(waiting.text, "Read the release notes before planning.");
    assert_eq!(waiting.worker, "Cloudline Coordinator");
    assert!(!waiting.from_web);
    assert!(!learning::instructions(
        &h.ledger,
        &h.ledger.position(&org.coordinator).unwrap().unwrap(),
        "Supervisor"
    )
    .contains("Read the release notes"));
    // Kept in the owner's words, it is in the next workers' instructions.
    h.workforce
        .decide_lesson(&waiting.id, true, Some("Read the release notes first."))
        .unwrap();
    let told = learning::instructions(
        &h.ledger,
        &h.ledger.position(&org.coordinator).unwrap().unwrap(),
        "Supervisor",
    );
    assert!(told.contains("- Read the release notes first."), "{told}");

    // A role that learns on its own keeps them at once.
    h.workforce.set_role_learning(&supervisor, true).unwrap();
    answer("Ask QA before the release.");
    let task = h
        .objective(&org.coordinator, "Plan the next release.")
        .await;
    h.finished(&task).await;
    until("the kept lesson", &|| lessons(LessonState::Kept).len() == 2);
    assert!(lessons(LessonState::Waiting).is_empty());

    // Switched off: no lessons recorded, none in the instructions.
    let snap = h.workforce.set_learning(false).unwrap();
    assert!(!snap.enabled && snap.auto_roles == [supervisor.clone()]);
    answer("Something else.");
    let task = h.objective(&org.coordinator, "Plan another release.").await;
    h.finished(&task).await;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(lessons(LessonState::Kept).len(), 2);
    assert!(learning::instructions(
        &h.ledger,
        &h.ledger.position(&org.coordinator).unwrap().unwrap(),
        "Supervisor"
    )
    .is_empty());
    // Removing a kept lesson.
    h.workforce.set_learning(true).unwrap();
    let kept = h.workforce.learning().unwrap().kept;
    let snap = h.workforce.remove_lesson(&kept[0].id).unwrap();
    assert_eq!(snap.kept.len(), 1);
    assert!(h.workforce.set_role_learning("no-such-role", true).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_create_department_role_manager_and_project_coordinator() {
    let h = harness().await;
    // Create role: a custom, on-demand worker role next to the seeded templates.
    let s = h
        .workforce
        .create_role(&RoleInput {
            name: "Release Manager".into(),
            description: "Runs releases.".into(),
            kind: PositionKind::Worker,
            staffing: Staffing::OnDemand,
            job: None,
        })
        .unwrap();
    let role = s
        .roles
        .iter()
        .find(|r| r.name == "Release Manager")
        .unwrap();
    assert!(!role.template && role.staffing == Staffing::OnDemand);
    assert_eq!(role.purpose, ["Runs releases"]);
    assert!(s.roles.iter().filter(|r| r.template).count() >= 10);
    assert!(refusal(h.workforce.create_role(&RoleInput {
        name: "Floating Manager".into(),
        description: String::new(),
        kind: PositionKind::DepartmentManager,
        staffing: Staffing::OnDemand,
        job: None,
    }))
    .contains("full-time"));

    // Create department, with its head position still vacant.
    let s = h
        .workforce
        .create_department(&DepartmentInput {
            name: "Development".into(),
            description: "Builds the products".into(),
            head: Some(LeadInput {
                vacant: Some(true),
                ..lead(&h.role("Manager"), "Development Manager", "claude-code")
            }),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let dept = s
        .departments
        .iter()
        .find(|d| d.name == "Development")
        .unwrap();
    let head_id = dept.head_position_id.clone().unwrap();
    let head = h.position(&head_id);
    assert_eq!(head.status, PositionStatus::Vacant);
    assert_eq!(head.kind, PositionKind::DepartmentManager);
    assert_eq!(head.heads_department_id.as_deref(), Some(dept.id.as_str()));

    // Assign manager: hire an agent into the head position.
    let s = h.workforce.fill(&head_id).unwrap();
    let head = s.positions.iter().find(|p| p.id == head_id).unwrap();
    assert_eq!(head.status, PositionStatus::Idle);
    let agent = head.agent.clone().unwrap();
    assert_eq!(agent.runtime_id.as_deref(), Some("claude-code"));
    assert!(agent.session_id.is_none(), "no objective yet");

    // Create project coordinator: the project comes with its coordinator under the head.
    let s = h
        .workforce
        .create_project(&ProjectInput {
            department_id: Some(dept.id.clone()),
            coordinator: Some(lead(
                &h.role("Supervisor"),
                "Cloudline Coordinator",
                "claude-code",
            )),
            ..project_input("Cloudline", &["claude-code", "codex"])
        })
        .unwrap();
    let project = s.projects.iter().find(|p| p.name == "Cloudline").unwrap();
    assert_eq!(project.department_id.as_deref(), Some(dept.id.as_str()));
    assert_eq!(project.allowed_runtimes, ["claude-code", "codex"]);
    assert_eq!(project.capability_profile.as_deref(), Some("development"));
    assert_eq!(
        project.local_path.as_deref(),
        Some("D:\\projects\\cloudline")
    );
    let coordinator = s
        .positions
        .iter()
        .find(|p| Some(&p.id) == project.coordinator_position_id.as_ref())
        .unwrap();
    assert_eq!(coordinator.reports_to.as_deref(), Some(head_id.as_str()));
    assert_eq!(coordinator.department_id.as_deref(), Some(dept.id.as_str()));
    assert_eq!(coordinator.status, PositionStatus::Idle);
    assert_eq!(
        (s.stats.departments, s.stats.projects, s.stats.staffed),
        (1, 1, 2)
    );
    // Only runtimes this build has are accepted.
    assert!(refusal(h.workforce.hire(&HireInput {
        role_id: h.role("Senior Developer"),
        title: "Gemini Developer".into(),
        reports_to: Some(coordinator.id.clone()),
        runtime_id: Some("gemini".into()),
        model: None,
        vacant: None,
        specialty_id: None,
    }))
    .contains("no AI tool named"));
    let events: Vec<String> = h
        .ledger
        .recent_events(200)
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    for want in [
        "org.role_created",
        "org.department_created",
        "org.agent_hired",
        "org.project_created",
    ] {
        assert!(events.iter().any(|e| e == want), "{want}");
    }
}

/// The Phase 5 acceptance criterion: view Development, select a project, give its coordinator
/// an objective, and watch workers appear under the coordinator and leave the active
/// workforce when they finish, while their history remains.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn acceptance_workers_appear_under_the_coordinator_and_leave_when_done() {
    let h = harness().await;
    let o = h.development();
    let s = h.snapshot();
    let development = s
        .departments
        .iter()
        .find(|d| d.name == "Development")
        .unwrap();
    assert_eq!(development.project_ids, std::slice::from_ref(&o.project));
    let cloudline = s.projects.iter().find(|p| p.id == o.project).unwrap();
    assert_eq!(
        cloudline.coordinator_position_id.as_deref(),
        Some(o.coordinator.as_str())
    );

    let root = h
        .objective(
            &o.coordinator,
            "Ship the Cloudline sign-in page [handoff:role:Senior Developer+delay:2000] \
             [handoff:role:QA Engineer+delay:2000]",
        )
        .await;

    // Two workers appear: one under each position of the coordinator's team.
    let busy = h
        .until("both workers to appear", |s| {
            let workers = |id: &str| {
                s.positions
                    .iter()
                    .find(|p| p.id == id)
                    .map_or(0, |p| p.workers.len())
            };
            workers(&o.developer) == 1 && workers(&o.qa) == 1
        })
        .await;
    assert_eq!(busy.stats.active_workers, 2);
    let position = |id: &str| busy.positions.iter().find(|p| p.id == id).unwrap().clone();
    let coordinator = position(&o.coordinator);
    assert_eq!(coordinator.status, PositionStatus::Waiting);
    assert_eq!(coordinator.current_task.as_ref().unwrap().id, root);
    for id in [&o.developer, &o.qa] {
        let worker = &position(id).workers[0];
        assert_eq!(worker.parent_task_id.as_deref(), Some(root.as_str()));
        assert_eq!(worker.objective, "Review the answer above [delay:2000]");
    }
    assert_eq!(position(&o.developer).workers[0].runtime_id, "codex");
    assert_eq!(position(&o.qa).workers[0].runtime_id, "claude-code");
    let children: Vec<String> = [&o.developer, &o.qa]
        .iter()
        .map(|id| position(id).workers[0].task_id.clone())
        .collect();

    // The coordinator continues with both replies and finishes.
    let done = h.finished(&root).await;
    assert_eq!(done.state, TaskState::Succeeded, "{:#?}", h.types(&root));
    assert!(
        h.text(&root).starts_with("Turn 2: received 2 replies:"),
        "{}",
        h.text(&root)
    );

    // The workers left the active workforce...
    let after = h
        .until("the workers to leave", |s| s.stats.active_workers == 0)
        .await;
    for id in [&o.developer, &o.qa] {
        let p = after.positions.iter().find(|p| &p.id == id).unwrap();
        assert!(p.workers.is_empty());
        assert_eq!(p.history.retired, 1, "{}", p.title);
        assert!(p.history.last_retired_at.is_some());
    }
    assert!(after.stats.completed_24h >= 3);

    // ...while their history remains: agents, tasks, and trails.
    for (position_id, child) in [(&o.developer, &children[0]), (&o.qa, &children[1])] {
        let agents = h.ledger.position_agents(position_id, 10).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].lifecycle_state, AgentLifecycle::Retired);
        assert_eq!(agents[0].task_id.as_deref(), Some(child.as_str()));
        let task = h.task(child);
        assert_eq!(task.state, TaskState::Succeeded);
        assert_eq!(task.project_id.as_deref(), Some(o.project.as_str()));
        assert_in_order(
            &h.types(child),
            &[
                "task.created",
                "org.worker_spawned",
                "liaison.dispatched",
                "org.worker_started",
                "agent.result",
                "org.worker_retired",
                "liaison.reply_sent",
            ],
        );
        let work = h.workforce.work(Some(position_id)).unwrap();
        assert_eq!(work.recent.len(), 1);
        assert_eq!(&work.recent[0].id, child);
    }
    assert_eq!(done.project_id.as_deref(), Some(o.project.as_str()));
    let coordinator_work = h.workforce.work(Some(&o.coordinator)).unwrap();
    assert_eq!(coordinator_work.recent[0].id, root);
    assert!(coordinator_work.team.is_empty(), "no team work left");
    let org_work = h.workforce.work(None).unwrap();
    assert!(org_work.recent.iter().any(|t| t.id == root));
    assert!(org_work.running.is_empty() && org_work.queued.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_persistent_coordinator_survives_restart() {
    let h = harness().await;
    let o = h.development();
    let first = h.objective(&o.coordinator, "Remember the plan").await;
    assert_eq!(h.finished(&first).await.state, TaskState::Succeeded);
    let before = h.position(&o.coordinator).agent.unwrap();
    let session = before.session_id.clone().expect("its session");

    // Restart: everything above the Ledger stops; a new stack opens the same Ledger file.
    h.liaison.shutdown();
    h.run.abort();
    h.rt.shutdown(Duration::from_secs(10)).await;
    h.sup.shutdown(Duration::from_secs(10)).await;
    let (ledger, rt, sup, liaison, router, workforce) = stack(h.dir.path()).await;
    let run = tokio::spawn(liaison.clone().run());
    let h = H {
        ledger,
        rt,
        sup,
        liaison,
        router,
        workforce,
        run,
        dir: h.dir,
    };
    let coordinator = h.position(&o.coordinator);
    let after = coordinator.agent.clone().unwrap();
    assert_eq!(after.id, before.id, "the same agent holds the position");
    assert_eq!(after.session_id.as_deref(), Some(session.as_str()));
    assert_eq!(coordinator.status, PositionStatus::Idle);
    assert_eq!(coordinator.history.retired, 0);

    // Its conversation continues in the same provider session.
    let second = h.objective(&o.coordinator, "What was the plan?").await;
    assert_eq!(h.finished(&second).await.state, TaskState::Succeeded);
    assert_eq!(
        h.task(&second).metadata["sessionId"],
        serde_json::json!(session)
    );
    let text = h.text(&second);
    assert!(
        text.starts_with(
            "Turn 2: you said \"What was the plan?\". Previous: Some(\"Remember the plan\")"
        ),
        "{text}"
    );
    h.run.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_department_and_project_reassignment() {
    let h = harness().await;
    let o = h.development();
    let (operations, ops_head) = h.department("Operations", "Operations Manager", "codex");
    // Moving the coordinator under Operations' head moves the project to Operations.
    let s = h
        .workforce
        .move_position(&o.coordinator, Some(&ops_head))
        .unwrap();
    let project = s.projects.iter().find(|p| p.id == o.project).unwrap();
    assert_eq!(project.department_id.as_deref(), Some(operations.as_str()));
    let dept_of = |s: &OrgSnapshot, id: &str| {
        s.positions
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .department_id
            .clone()
    };
    assert_eq!(
        dept_of(&s, &o.coordinator).as_deref(),
        Some(operations.as_str())
    );
    assert_eq!(
        dept_of(&s, &o.developer).as_deref(),
        Some(operations.as_str()),
        "its team moves with it"
    );
    let development = s.departments.iter().find(|d| d.id == o.department).unwrap();
    assert!(development.project_ids.is_empty());
    // A worker moves to another team: out of the project, into that department.
    let s = h
        .workforce
        .move_position(&o.developer, Some(&o.head))
        .unwrap();
    let dev = s.positions.iter().find(|p| p.id == o.developer).unwrap();
    assert_eq!(dev.project_id, None);
    assert_eq!(dev.department_id.as_deref(), Some(o.department.as_str()));
    let events: Vec<String> = h
        .ledger
        .recent_events(100)
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    assert!(events.iter().any(|e| e == "org.project_reassigned"));
    assert!(events.iter().filter(|e| *e == "org.position_moved").count() == 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_orphan_prevention() {
    let h = harness().await;
    let o = h.development();
    assert!(refusal(h.workforce.archive_position(&o.coordinator)).contains("archive the project"));
    assert!(refusal(h.workforce.archive_position(&o.head)).contains("heads Development"));
    // A persistent specialist with an on-demand report cannot leave it without a supervisor.
    h.workforce
        .create_role(&RoleInput {
            name: "Tech Lead".into(),
            description: "Leads implementation.".into(),
            kind: PositionKind::Worker,
            staffing: Staffing::Persistent,
            job: None,
        })
        .unwrap();
    let tech_lead = h.hire("Tech Lead", "Tech Lead", &o.coordinator, "claude-code");
    let intern = h.hire("Senior Developer", "Junior Developer", &tech_lead, "codex");
    assert!(refusal(h.workforce.archive_position(&tech_lead)).contains("without a supervisor"));
    // No cycles; only persistent positions supervise; heads report to the owner or a
    // superintendent.
    assert!(
        refusal(h.workforce.move_position(&o.head, Some(&o.coordinator)))
            .contains("cannot report to it")
    );
    let (_, ops_head) = h.department("Operations", "Operations Manager", "codex");
    assert!(refusal(h.workforce.move_position(&ops_head, Some(&o.head))).contains("to a VP"));
    assert!(refusal(h.workforce.move_position(&tech_lead, Some(&intern))).contains("on-call"));
    assert!(refusal(
        h.workforce
            .move_position(&o.coordinator, Some(&o.developer))
    )
    .contains("on-call"));
    assert!(refusal(
        h.workforce
            .delete_for_good("department", &o.department, &[])
            .map(|(s, _)| s)
    )
    .contains("not archived"));

    // An agent with unfinished work cannot be let go.
    let busy = h
        .objective(&o.coordinator, "Take your time [delay:3000]")
        .await;
    h.until("the coordinator to work", |s| {
        s.positions
            .iter()
            .any(|p| p.id == o.coordinator && p.status == PositionStatus::Working)
    })
    .await;
    assert!(refusal(h.workforce.vacate(&o.coordinator)).contains("unfinished task"));
    assert!(refusal(h.workforce.archive_project(&o.project)).contains("unfinished task"));
    h.finished(&busy).await;
    let s = h.workforce.vacate(&o.coordinator).unwrap();
    let coordinator = s.positions.iter().find(|p| p.id == o.coordinator).unwrap();
    assert_eq!(coordinator.status, PositionStatus::Vacant);
    assert_eq!(coordinator.history.retired, 1);
    assert!(refusal(
        h.workforce
            .give_objective(&o.coordinator, "hello", None)
            .await
    )
    .contains("vacant"));

    // Archiving the project archives its whole team and ends the QA assignment for it.
    let s = h.workforce.archive_project(&o.project).unwrap();
    for id in [&o.coordinator, &o.developer, &tech_lead, &intern] {
        let p = s.positions.iter().find(|p| &p.id == id).unwrap();
        assert!(!p.active, "{} archived", p.title);
    }
    assert!(s.oversight.is_empty());
    assert!(
        s.positions.iter().any(|p| p.id == o.qa && p.active),
        "the QA engineer stays"
    );
    assert!(
        !s.projects
            .iter()
            .find(|p| p.id == o.project)
            .unwrap()
            .active
    );
}

// ---- Routing -------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_outside_the_team_are_refused_and_explained() {
    let h = harness().await;
    let o = h.development();
    let root = h
        .objective(
            &o.coordinator,
            "Ask around [handoff:role:Designer] [handoff:codex]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(
        h.ledger.child_tasks(&root).unwrap().is_empty(),
        "nothing started"
    );
    let reasons = h.rejections(&root);
    assert!(
        reasons[0].contains(
            "\"Designer\" is not on your team; address one of: role:Senior Developer, role:QA Engineer"
        ),
        "{reasons:?}"
    );
    assert!(
        reasons[1].contains("hand work to a member of your team, not to an AI tool"),
        "{reasons:?}"
    );
    // A runtime the project no longer allows is refused, never switched.
    let mut settings = project_input("Cloudline", &["claude-code"]);
    settings.description = "Claude Code only".into();
    h.workforce.update_project(&o.project, &settings).unwrap();
    let root = h
        .objective(&o.coordinator, "Build it [handoff:role:Senior Developer]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.ledger.child_tasks(&root).unwrap().is_empty());
    let reasons = h.rejections(&root);
    assert!(
        reasons[0].contains("Cloudline does not allow Codex workers"),
        "{reasons:?}"
    );
    assert_eq!(
        h.position(&o.developer).status,
        PositionStatus::Unavailable,
        "the canvas says why"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn objectives_go_only_to_staffed_persistent_positions() {
    let h = harness().await;
    let o = h.development();
    assert!(refusal(h.workforce.give_objective(&o.developer, "x", None).await).contains("on-call"));
    assert!(h
        .workforce
        .give_objective("0f8fad5b-d9cb-469f-a165-70867728950e", "x", None)
        .await
        .is_err());
    // The project's allowed runtimes bind its coordinator too.
    h.workforce
        .update_project(&o.project, &project_input("Cloudline", &["codex"]))
        .unwrap();
    assert!(
        refusal(h.workforce.give_objective(&o.coordinator, "x", None).await)
            .contains("does not allow")
    );
    // A member session cannot be continued from the Workers view.
    h.workforce
        .update_project(
            &o.project,
            &project_input("Cloudline", &["claude-code", "codex"]),
        )
        .unwrap();
    let t = h.objective(&o.coordinator, "Hello").await;
    h.finished(&t).await;
    let session = h
        .position(&o.coordinator)
        .agent
        .unwrap()
        .session_id
        .unwrap();
    assert!(h
        .liaison
        .resume_session(&session, "sneak in")
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_worker_that_fails_leaves_as_failed_and_the_coordinator_carries_on() {
    let h = harness().await;
    let o = h.development();
    let root = h
        .objective(
            &o.coordinator,
            "Build it [handoff:role:Senior Developer+crash]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(
        h.text(&root)
            .starts_with("Turn 2: received 1 reply: Senior Developer (Codex): crashed"),
        "{}",
        h.text(&root)
    );
    let s = h
        .until("the worker to leave", |s| s.stats.active_workers == 0)
        .await;
    let dev = s.positions.iter().find(|p| p.id == o.developer).unwrap();
    assert_eq!((dev.history.retired, dev.history.failed), (0, 1));
    let agents = h.ledger.position_agents(&o.developer, 10).unwrap();
    assert_eq!(agents[0].lifecycle_state, AgentLifecycle::Failed);
    assert!(s.stats.failed_24h >= 1);

    // Home: the objective is finished, with its answer; the worker's failure is not stuck,
    // because its objective succeeded anyway.
    let home = h.workforce.home().unwrap();
    let done = home
        .finished
        .iter()
        .find(|b| b.root_task_id == root)
        .expect("the objective is on Home");
    assert!(done.answer.as_deref().unwrap_or("").starts_with("Turn 2"));
    assert!(home.current.iter().all(|b| b.root_task_id != root));
    assert!(home.stuck.is_empty(), "{:#?}", home.stuck);

    // An objective that fails is stuck, named with its task and who has it, until it is
    // done another way.
    let failed = h.objective(&o.coordinator, "Ship it [crash]").await;
    assert_eq!(h.finished(&failed).await.state, TaskState::Failed);
    let home = h.workforce.home().unwrap();
    let stuck = &home.stuck[0];
    assert_eq!(stuck.event.event_type, "task.state_changed");
    let task = stuck.task.as_ref().expect("its task");
    assert_eq!(task.id, failed);
    assert_eq!(task.objective, "Ship it [crash]");
    assert_eq!(task.position_id.as_deref(), Some(o.coordinator.as_str()));
    assert!(task.position_title.is_some());
    assert!(home.finished.iter().any(|b| b.root_task_id == failed));
}

// ---- Phase 6: model policy and role routing -------------------------------------------------

impl H {
    /// Hire an automatic position (its role's model policy picks each worker's AI tool).
    fn hire_auto(&self, role: &str, title: &str, reports_to: &str) -> String {
        let s = self
            .workforce
            .hire(&HireInput {
                role_id: self.role(role),
                title: title.into(),
                reports_to: Some(reports_to.into()),
                runtime_id: None,
                model: None,
                vacant: None,
                specialty_id: None,
            })
            .unwrap();
        Self::id_of(&s, title)
    }

    fn model(&self, label: &str) -> String {
        self.router
            .snapshot()
            .unwrap()
            .models
            .into_iter()
            .find(|m| m.label == label)
            .unwrap_or_else(|| panic!("no model {label}"))
            .id
    }

    /// Set a role's ordered model list, keeping the rest of its policy.
    fn prefer(&self, role: &str, labels: &[&str]) {
        let mut policy = self.policy(role);
        policy.models = labels.iter().map(|l| self.model(l)).collect();
        self.router.set_policy(&self.role(role), &policy).unwrap();
    }

    fn policy(&self, role: &str) -> plenipo_router::RolePolicy {
        let role = self.role(role);
        self.router
            .snapshot()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.role_id == role)
            .unwrap()
            .policy
    }

    /// The only child task `root` created in its latest round.
    fn last_child(&self, root: &str) -> Task {
        self.ledger
            .child_tasks(root)
            .unwrap()
            .into_iter()
            .max_by_key(|t| t.created_at)
            .expect("a child task")
    }

    /// What Liaison writes into a member's instructions: who it is and its team.
    fn briefing(&self, position: &str) -> (String, Vec<(String, String, bool)>) {
        let team = WorkforceDirectory::new(Arc::clone(&self.ledger), self.router.clone())
            .team(&serde_json::json!({ "positionId": position }))
            .unwrap();
        let members = team
            .members
            .into_iter()
            .map(|d| (d.address, d.label, d.ready))
            .collect();
        (team.identity, members)
    }
}

fn reason(task: &Task) -> String {
    task.metadata["workforce"]["routing"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// The Phase 6 acceptance criteria: changing a role's model preference in Settings changes the
/// next worker Plenipo launches, without touching the coordinator or its instructions, and
/// every choice is explained.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn acceptance_a_roles_model_choices_decide_its_next_worker() {
    let h = harness().await;
    let o = h.development();
    let backend = h.hire_auto("Senior Developer", "Backend Developer", &o.coordinator);
    let s = h
        .router
        .save_model(&ModelInput {
            id: None,
            runtime_id: "claude-code".into(),
            name: Some("fake-fast".into()),
            label: "Fast".into(),
            features: vec![],
            context_tokens: None,
            cost: plenipo_router::CostClass::Economical,
            effort: Some(Effort::High),
        })
        .unwrap();
    assert_eq!(
        s.models.len(),
        plenipo_runtime::agent::builtin_adapters().len() + 1,
        "each AI tool's built-in default and the owner's model"
    );

    // Senior Developer: Codex's default model first.
    h.prefer("Senior Developer", &["Codex (default model)"]);
    let p = h.position(&backend);
    assert!(p.automatic);
    assert_eq!(
        p.runtime_id.as_deref(),
        Some("codex"),
        "the next worker's AI tool"
    );
    assert_eq!(
        p.route.as_ref().unwrap().reason,
        "Codex (default model) is Senior Developer's first choice and is ready."
    );
    let first = h
        .objective(&o.coordinator, "Build it [handoff:role:Backend Developer]")
        .await;
    assert_eq!(h.finished(&first).await.state, TaskState::Succeeded);
    let child = h.last_child(&first);
    assert_eq!(child.assigned_to.as_deref(), Some("codex"));
    assert_eq!(
        reason(&child),
        "Codex (default model) is Senior Developer's first choice and is ready."
    );
    let spawned = h
        .ledger
        .events_for_task(&child.id)
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "org.worker_spawned")
        .unwrap();
    assert_eq!(spawned.payload["runtimeId"], "codex");
    assert_eq!(spawned.payload["routing"]["rank"], 1);
    assert!(
        h.text(&first).contains("(Codex): completed"),
        "{}",
        h.text(&first)
    );

    // The owner changes the preference in Settings: the next worker uses the new first choice.
    h.prefer("Senior Developer", &["Fast", "Codex (default model)"]);
    let second = h
        .objective(
            &o.coordinator,
            "Now the API [handoff:role:Backend Developer]",
        )
        .await;
    assert_eq!(h.finished(&second).await.state, TaskState::Succeeded);
    let child = h.last_child(&second);
    assert_eq!(child.assigned_to.as_deref(), Some("claude-code"));
    assert_eq!(
        reason(&child),
        "Fast (Claude Code) is Senior Developer's first choice and is ready. It runs at high \
         effort, from the model's own setting."
    );
    let worker =
        h.rt.session(child.metadata["sessionId"].as_str().unwrap())
            .await
            .unwrap()
            .session;
    assert_eq!(
        worker.model.as_deref(),
        Some("fake-fast"),
        "the chosen model"
    );
    assert_eq!(worker.effort, Some(Effort::High), "the model's effort");
    let history = h.ledger.position_agents(&backend, 10).unwrap();
    let runtimes: Vec<Option<&str>> = history.iter().map(|a| a.runtime_id.as_deref()).collect();
    assert_eq!(
        runtimes,
        [Some("claude-code"), Some("codex")],
        "newest first"
    );

    // A position the owner fixed keeps its AI tool whatever the role's policy says.
    let third = h
        .objective(&o.coordinator, "Review [handoff:role:Senior Developer]")
        .await;
    assert_eq!(h.finished(&third).await.state, TaskState::Succeeded);
    let child = h.last_child(&third);
    assert_eq!(child.assigned_to.as_deref(), Some("codex"));
    assert_eq!(
        reason(&child),
        "You set Senior Developer to always use Codex (default model)."
    );

    // Nothing about the coordinator changed: same position, same conversation, and the same
    // instructions (the team list names no AI tool for automatic members).
    let events: Vec<String> = h
        .ledger
        .recent_events(1000)
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    assert!(!events.iter().any(|e| e == "org.position_updated"));
    let turns = h.ledger.position_tasks(&o.coordinator, 10).unwrap();
    assert!(turns
        .iter()
        .all(|t| t.metadata["sessionId"] == turns[0].metadata["sessionId"]));
    let (identity, members) = h.briefing(&o.coordinator);
    let backend_member = members
        .iter()
        .find(|(address, _, _)| address == "role:Backend Developer")
        .unwrap();
    assert_eq!(
        backend_member.1,
        "Senior Developer, a new worker for each request"
    );
    h.prefer("Senior Developer", &["Codex (default model)"]);
    assert_eq!(h.briefing(&o.coordinator), (identity, members));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_usage_limit_holds_work_back_or_moves_it_on_as_the_owner_chose() {
    let h = harness().await;
    let o = h.development();
    let backend = h.hire_auto("Senior Developer", "Backend Developer", &o.coordinator);
    h.prefer(
        "Senior Developer",
        &["Codex (default model)", "Claude Code (default model)"],
    );
    // The worker on Codex reports a usage limit.
    let root = h
        .objective(
            &o.coordinator,
            "Go [handoff:role:Backend Developer+usage-limit]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.text(&root).contains("usageLimited"), "{}", h.text(&root));
    let tools = h.router.snapshot().unwrap().tools;
    let codex = tools.iter().find(|t| t.runtime_id == "codex").unwrap();
    assert!(codex.ready && !codex.available);
    let limit = codex.usage_limit.as_ref().unwrap();
    assert!(limit.detail.contains("usage limit"), "{limit:?}");
    assert_eq!(limit.until, limit.since + plenipo_router::limits::HOLD_MS);
    let p = h.position(&backend);
    assert_eq!(p.status, PositionStatus::Unavailable);
    assert!(
        p.status_detail
            .as_deref()
            .unwrap()
            .contains("waits for it rather than moving work to another AI company"),
        "{p:?}"
    );

    // Wait (the default): the next request is refused and explained; nothing switches.
    let held = h
        .objective(&o.coordinator, "Again [handoff:role:Backend Developer]")
        .await;
    assert_eq!(h.finished(&held).await.state, TaskState::Succeeded);
    assert!(h.ledger.child_tasks(&held).unwrap().is_empty());
    let why = h.rejections(&held);
    assert!(
        why[0].contains("Codex reached its usage limit, and Senior Developer waits for it"),
        "{why:?}"
    );

    // Next choice: the owner allows moving on, and says so in the explanation.
    h.router
        .set_options(RoutingOptions {
            on_usage_limit: LimitBehavior::NextChoice,
        })
        .unwrap();
    let moved = h
        .objective(&o.coordinator, "Once more [handoff:role:Backend Developer]")
        .await;
    assert_eq!(h.finished(&moved).await.state, TaskState::Succeeded);
    let child = h.last_child(&moved);
    assert_eq!(child.assigned_to.as_deref(), Some("claude-code"));
    assert!(
        reason(&child).starts_with(
            "Claude Code (default model) is Senior Developer's second choice: Codex (default \
             model) was skipped because Codex reached its usage limit"
        ),
        "{}",
        reason(&child)
    );

    // The owner tries Codex again: the first choice is back.
    h.router.clear_limit("codex").unwrap();
    let back = h
        .objective(&o.coordinator, "Last [handoff:role:Backend Developer]")
        .await;
    assert_eq!(h.finished(&back).await.state, TaskState::Succeeded);
    assert_eq!(h.last_child(&back).assigned_to.as_deref(), Some("codex"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_time_agent_is_routed_when_its_conversation_starts_and_keeps_it() {
    let h = harness().await;
    let s = h
        .workforce
        .create_department(&DepartmentInput {
            name: "Research".into(),
            description: String::new(),
            head: Some(LeadInput {
                runtime_id: None,
                ..lead(&h.role("Manager"), "Research Manager", "unused")
            }),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let head = H::id_of(&s, "Research Manager");
    let p = h.position(&head);
    assert!(p.automatic);
    assert_eq!(p.agent.as_ref().unwrap().runtime_id, None, "not routed yet");
    assert_eq!(p.status, PositionStatus::Idle);
    h.prefer("Manager", &["Codex (default model)"]);
    // The Manager runs Codex's default model at low effort.
    let mut policy = h.policy("Manager");
    policy
        .efforts
        .insert(h.model("Codex (default model)"), Effort::Low);
    h.router.set_policy(&h.role("Manager"), &policy).unwrap();

    let first = h.objective(&head, "Plan the quarter").await;
    assert_eq!(h.finished(&first).await.state, TaskState::Succeeded);
    let turn = h.task(&first);
    assert_eq!(turn.assigned_to.as_deref(), Some("codex"));
    assert_eq!(
        reason(&turn),
        "Codex (default model) is Manager's first choice and is ready. It runs at low \
         effort, from Manager's rule."
    );
    let conversation = turn.metadata["sessionId"].as_str().unwrap().to_owned();
    assert_eq!(
        h.rt.session(&conversation).await.unwrap().session.effort,
        Some(Effort::Low)
    );
    let routed = h
        .ledger
        .recent_events(200)
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "org.agent_routed")
        .unwrap();
    assert_eq!(routed.payload["runtimeId"], "codex");
    let p = h.position(&head);
    let agent = p.agent.clone().unwrap();
    assert_eq!(agent.runtime_id.as_deref(), Some("codex"));
    let session = agent.session_id.unwrap();

    // A new preference does not move an ongoing conversation; a new agent follows it.
    h.prefer("Manager", &["Claude Code (default model)"]);
    let p = h.position(&head);
    assert_eq!(
        p.runtime_id.as_deref(),
        Some("codex"),
        "its conversation's AI tool"
    );
    assert_eq!(
        p.route.unwrap().choice.unwrap().runtime_id,
        "claude-code",
        "what a new agent would get"
    );
    let second = h.objective(&head, "And next quarter?").await;
    h.finished(&second).await;
    assert_eq!(h.task(&second).metadata["sessionId"], session.as_str());
    h.workforce.vacate(&head).unwrap();
    h.workforce.fill(&head).unwrap();
    let third = h.objective(&head, "Start fresh").await;
    assert_eq!(h.finished(&third).await.state, TaskState::Succeeded);
    assert_eq!(h.task(&third).assigned_to.as_deref(), Some("claude-code"));
    assert_ne!(h.task(&third).metadata["sessionId"], session.as_str());

    // No model can take the work: the objective is refused with the reason.
    h.prefer("Manager", &[]);
    let mut policy = h.policy("Manager");
    policy.needs = vec![plenipo_router::ModelFeature::ComputerUse];
    h.router.set_policy(&h.role("Manager"), &policy).unwrap();
    h.workforce.vacate(&head).unwrap();
    h.workforce.fill(&head).unwrap();
    let why = refusal(h.workforce.give_objective(&head, "Anything", None).await);
    assert!(
        why.starts_with("Research Manager cannot start: No model can take Manager's work now"),
        "{why}"
    );
    assert!(
        why.contains("not marked as able to use a computer"),
        "{why}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reviewers_come_from_another_ai_company_and_unfit_roles_are_explained() {
    let h = harness().await;
    let o = h.development();
    // The coordinator works on Claude Code (Anthropic). The Code Reviewer template prefers
    // another AI company than the work it reviews.
    let reviewer = h.hire_auto("Code Reviewer", "Reviewer", &o.coordinator);
    h.prefer(
        "Code Reviewer",
        &["Claude Code (default model)", "Codex (default model)"],
    );
    let policy = h
        .router
        .snapshot()
        .unwrap()
        .roles
        .into_iter()
        .find(|r| r.role_name == "Code Reviewer")
        .unwrap()
        .policy;
    assert_eq!(
        policy.cross_company,
        CrossCompany::Prefer,
        "the template's default"
    );
    // Nothing reviewed yet: the list order.
    assert_eq!(
        h.position(&reviewer).runtime_id.as_deref(),
        Some("claude-code")
    );
    let root = h
        .objective(&o.coordinator, "Check my plan [handoff:role:Reviewer]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.last_child(&root);
    assert_eq!(child.assigned_to.as_deref(), Some("codex"));
    assert!(
        reason(&child).ends_with(
            "It comes from a different AI company than the work it reviews (Anthropic)."
        ),
        "{}",
        reason(&child)
    );

    // The Designer template needs a model that sees and makes images; none is marked so.
    h.hire_auto("Designer", "Designer", &o.coordinator);
    let root = h
        .objective(&o.coordinator, "A logo [handoff:role:Designer]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.ledger.child_tasks(&root).unwrap().is_empty());
    let why = h.rejections(&root);
    assert!(
        why[0].starts_with("Designer cannot take work now: No model can take Designer's work now"),
        "{why:?}"
    );
    assert!(
        why[0].contains("not marked as able to see images"),
        "{why:?}"
    );
}

// ---- Phase 8: leads hand work to full-time members (ADR-016) -------------------------------

fn session_of(task: &Task) -> String {
    task.metadata["sessionId"]
        .as_str()
        .expect("a session")
        .to_owned()
}

impl H {
    /// The first child of `parent`, once it exists.
    async fn child_of(&self, parent: &str) -> Task {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(child) = self.ledger.child_tasks(parent).unwrap().into_iter().next() {
                return child;
            }
            assert!(Instant::now() < deadline, "no child task of {parent}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Wait until task `id` has an event of `event_type`.
    async fn event_on(&self, id: &str, event_type: &str) {
        let deadline = Instant::now() + WAIT;
        while !self.types(id).iter().any(|t| t == event_type) {
            assert!(
                Instant::now() < deadline,
                "task {id} never had {event_type}: {:#?}",
                self.types(id)
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_manager_hands_an_objective_to_its_supervisor_who_does_it_in_its_own_conversation() {
    let h = harness().await;
    let o = h.development();
    // The supervisor already has a conversation with the owner.
    let first = h.objective(&o.coordinator, "Hello").await;
    h.finished(&first).await;
    let conversation = session_of(&h.task(&first));
    // The manager's team names its supervisor, a full-time member.
    let (_, team) = h.briefing(&o.head);
    let supervisor = team
        .iter()
        .find(|(address, _, _)| address == "role:Cloudline Coordinator")
        .expect("the supervisor is on the manager's team");
    assert!(supervisor.2, "ready: {team:?}");
    assert!(
        supervisor.1.contains("leads the project Cloudline"),
        "{team:?}"
    );

    let root = h
        .objective(
            &o.head,
            "Ship it {{handoff:role:Cloudline Coordinator|Build it [handoff:role:Senior Developer]}}",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.last_child(&root);
    assert_eq!(child.state, TaskState::Succeeded);
    assert_eq!(
        session_of(&child),
        conversation,
        "the supervisor's own conversation"
    );
    assert_eq!(child.metadata["workforce"]["fullTime"], true);
    assert_eq!(child.metadata["workforce"]["positionId"], o.coordinator);
    assert_eq!(child.project_id.as_deref(), Some(o.project.as_str()));
    assert_eq!(
        child.metadata["turn"], 2,
        "its second task in that conversation"
    );
    assert!(
        !h.types(&child.id).iter().any(|t| t == "org.worker_spawned"),
        "no worker is brought in for a full-time member"
    );
    // The supervisor handed on to its developer, a worker brought in for the task.
    let grandchild = h.last_child(&child.id);
    assert_eq!(grandchild.metadata["workforce"]["positionId"], o.developer);
    assert!(h
        .types(&grandchild.id)
        .iter()
        .any(|t| t == "org.worker_spawned"));
    // One workflow from the owner's objective down.
    let correlation = h.task(&root).metadata["liaison"]["correlationId"].clone();
    assert_eq!(child.metadata["liaison"]["correlationId"], correlation);
    assert_eq!(grandchild.metadata["liaison"]["correlationId"], correlation);
    // Replies are named by position.
    assert!(
        h.text(&child.id)
            .starts_with("Turn 3: received 1 reply: Senior Developer (Codex): completed"),
        "{}",
        h.text(&child.id)
    );
    assert!(
        h.text(&root).starts_with(
            "Turn 2: received 1 reply: Cloudline Coordinator (Claude Code): completed: Turn 3"
        ),
        "{}",
        h.text(&root)
    );
    // Its conversation stays open, and the owner's next objective continues it.
    let next = h.objective(&o.coordinator, "And now?").await;
    h.finished(&next).await;
    assert_eq!(session_of(&h.task(&next)), conversation);
    let session = h.ledger.runtime_session(&conversation).unwrap().unwrap();
    assert_eq!(session.state, plenipo_ledger::RuntimeSessionState::Open);
    assert_eq!(session.turn_count, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_busy_supervisor_takes_a_handed_over_objective_when_it_is_free() {
    let h = harness().await;
    let o = h.development();
    let busy = h.objective(&o.coordinator, "A long job [delay:3000]").await;
    let root = h
        .objective(
            &o.head,
            "Ship it {{handoff:role:Cloudline Coordinator|Summarize the status}}",
        )
        .await;
    let child = h.child_of(&root).await;
    h.event_on(&child.id, "liaison.waiting_for_member").await;
    assert_eq!(
        h.task(&child.id).state,
        TaskState::Queued,
        "it waits for the supervisor"
    );
    assert_eq!(h.finished(&busy).await.state, TaskState::Succeeded);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.task(&child.id);
    assert_eq!(child.state, TaskState::Succeeded);
    assert!(child.started_at >= h.task(&busy).completed_at);
    assert_eq!(session_of(&child), session_of(&h.task(&busy)));
    // The wait was recorded once, with the reason.
    let waits: Vec<_> = h
        .ledger
        .events_for_task(&child.id)
        .unwrap()
        .into_iter()
        .filter(|e| e.event_type == "liaison.waiting_for_member")
        .collect();
    assert_eq!(waits.len(), 1);
    assert!(waits[0].payload["reason"]
        .as_str()
        .unwrap()
        .starts_with("waiting for Cloudline Coordinator"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_handover_starts_a_supervisors_first_conversation_and_work_only_goes_down() {
    let h = harness().await;
    let o = h.development();
    let agent = h.position(&o.coordinator).agent.unwrap().id;
    let root = h
        .objective(&o.head, "{{handoff:role:Cloudline Coordinator|Plan it}}")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.last_child(&root);
    assert_eq!(child.state, TaskState::Succeeded);
    assert_eq!(
        session_of(&child),
        agent,
        "its first conversation is named after its agent"
    );
    let session = h.ledger.runtime_session(&agent).unwrap().unwrap();
    assert_eq!(session.metadata["liaison"]["origin"], "member");
    assert_eq!(session.metadata["workforce"]["positionId"], o.coordinator);
    // The owner's next objective continues that conversation.
    let next = h.objective(&o.coordinator, "Next").await;
    h.finished(&next).await;
    assert_eq!(session_of(&h.task(&next)), agent);

    // Work only goes down: the supervisor cannot hand work to its manager.
    let up = h
        .objective(
            &o.coordinator,
            "Escalate [handoff:role:Development Manager]",
        )
        .await;
    assert_eq!(h.finished(&up).await.state, TaskState::Succeeded);
    assert!(h.ledger.child_tasks(&up).unwrap().is_empty());
    let why = h.rejections(&up);
    assert!(
        why[0].contains("\"Development Manager\" is not on your team"),
        "{why:?}"
    );

    // A vacant supervisor cannot take work, and the manager is told why.
    h.workforce.vacate(&o.coordinator).unwrap();
    let root = h
        .objective(&o.head, "{{handoff:role:Cloudline Coordinator|Plan it}}")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.ledger.child_tasks(&root).unwrap().is_empty());
    let why = h.rejections(&root);
    assert!(why[0].contains("vacant"), "{why:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_a_handed_over_objective_stops_only_that_task() {
    let h = harness().await;
    let o = h.development();
    let root = h
        .objective(
            &o.head,
            "{{handoff:role:Cloudline Coordinator|A long job [delay:15000]}}",
        )
        .await;
    let child = h.child_of(&root).await;
    let deadline = Instant::now() + WAIT;
    while h.task(&child.id).state != TaskState::Running {
        assert!(Instant::now() < deadline, "the supervisor never started");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // The owner cancels the manager's objective: its wait ends, and the supervisor's task
    // stops.
    let manager_session = session_of(&h.task(&root));
    h.rt.cancel_turn(&manager_session).await.unwrap();
    assert_eq!(h.finished(&root).await.state, TaskState::Cancelled);
    assert_eq!(h.finished(&child.id).await.state, TaskState::Cancelled);
    // The supervisor's conversation carries on with its next objective.
    let next = h.objective(&o.coordinator, "Still there?").await;
    assert_eq!(h.finished(&next).await.state, TaskState::Succeeded);
    assert_eq!(session_of(&h.task(&next)), session_of(&child));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_development_template_sets_up_a_department_project_and_team() {
    let h = harness().await;
    let input = |name: &str| plenipo_workforce::DevelopmentInput {
        project: ProjectInput {
            capability_profile: None,
            ..project_input(name, &["claude-code", "codex"])
        },
        runtime_id: Some("claude-code".into()),
    };
    let s = h.workforce.set_up_development(&input("Website")).unwrap();
    assert_eq!(s.departments.len(), 1);
    let dept = &s.departments[0];
    assert_eq!(dept.name, "Development");
    let vp = s
        .positions
        .iter()
        .find(|p| Some(&p.id) == dept.head_position_id.as_ref())
        .unwrap();
    assert_eq!(
        (vp.title.as_str(), vp.kind),
        ("Development VP", PositionKind::Superintendent)
    );
    assert_eq!(vp.reports_to, None, "it reports to the owner");
    assert_eq!(vp.runtime_id.as_deref(), Some("claude-code"));
    let project = s.projects.iter().find(|p| p.name == "Website").unwrap();
    assert!(project.branch_per_objective, "on by default");
    let supervisor = project.coordinator_position_id.clone().unwrap();
    assert_eq!(H::id_of(&s, "Website Supervisor"), supervisor);
    let team: Vec<(String, bool)> = s
        .positions
        .iter()
        .filter(|p| p.reports_to.as_deref() == Some(supervisor.as_str()))
        .map(|p| (p.title.clone(), p.automatic))
        .collect();
    assert_eq!(
        team,
        [
            ("Senior Developer".to_owned(), true),
            ("Code Reviewer".to_owned(), true),
            ("QA Engineer".to_owned(), true),
            ("Documentation Writer".to_owned(), true),
        ]
    );
    // The VP's team is its supervisor, which it hands objectives to.
    let (_, briefing) = h.briefing(&vp.id);
    assert_eq!(briefing[0].0, "role:Website Supervisor");

    // A second project joins the same department; a name is used once.
    let s = h.workforce.set_up_development(&input("Cloudline")).unwrap();
    assert_eq!(s.departments.len(), 1);
    assert_eq!(s.projects.len(), 2);
    assert!(refusal(h.workforce.set_up_development(&input("website"))).contains("already exists"));
    // The supervisor's AI tool must be allowed in the project.
    let mut bad = input("Waypoint");
    bad.project.allowed_runtimes = vec!["codex".into()];
    assert!(refusal(h.workforce.set_up_development(&bad)).contains("allowed AI tools"));
}
