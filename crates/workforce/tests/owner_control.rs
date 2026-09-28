//! Phase 17 through the real Workforce, Router, Liaison, agent runtime, supervisor, adapters, and
//! a file-backed Ledger, driving `plenipo-fake-agent` installed as the AI tools: effort alone
//! keeps the agent and its conversation (ADR-041); learning follows the closest setting under the
//! main switch (ADR-041); a Senior Developer with the Database specialty (ADR-042); archive, bring
//! back, and delete for good (ADR-043); experience and the Workforce (ADR-045).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_ledger::{Ledger, LessonState, Task, DB_FILE_NAME};
use plenipo_liaison::store::{LedgerExecutionStore, LedgerSessionStore};
use plenipo_liaison::Directory as _;
use plenipo_liaison::{Liaison, LiaisonConfig};
use plenipo_router::{ModelRule, Router, RuleTarget};
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, Effort, HostEnv,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};
use plenipo_workforce::directory::WorkforceDirectory;
use plenipo_workforce::learning::{self, LearningFrom};
use plenipo_workforce::{
    DepartmentInput, HireInput, LeadInput, OrgSnapshot, PositionInfo, PositionPatchInput,
    PositionStatus, ProjectInput, RoleJob, SpecialtyInput, SpecialtySuggest, Workforce,
};
use serde_json::json;

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

fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("owner-control-fake-agents-{}", std::process::id()));
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
    router: Router,
    workforce: Workforce,
    _run: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
}

async fn harness() -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".plenipo-fake-agent")).unwrap();
    for stem in personas() {
        let source = fake_clis().join(exe_name(stem));
        let target = bin.join(exe_name(stem));
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
    config.extra_env = vec![(
        HOME_VAR.into(),
        dir.path().join("home").display().to_string(),
    )];
    config.turn_timeout = Duration::from_secs(120);
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup,
        Arc::new(LedgerSessionStore(Arc::clone(&ledger))),
        Arc::new(NoUpdates),
        HostEnv::new(
            Some(bin.into_os_string()),
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
    let run = tokio::spawn(liaison.run());
    H {
        ledger,
        rt,
        router,
        workforce,
        _run: run,
        dir,
    }
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

fn project_input(name: &str) -> ProjectInput {
    ProjectInput {
        name: name.into(),
        description: format!("The {name} product"),
        repository_url: None,
        local_path: None,
        allowed_runtimes: vec!["claude-code".into(), "codex".into()],
        capability_profile: None,
        branch_per_objective: None,
        department_id: None,
        coordinator: None,
    }
}

/// Development (manager on Claude Code), Cloudline (supervisor on Claude Code), and an on-call
/// Senior Developer on Codex.
struct Org {
    department: String,
    project: String,
    head: String,
    coordinator: String,
    developer: String,
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
            .unwrap()
            .id
    }

    fn position(&self, id: &str) -> PositionInfo {
        self.snapshot()
            .positions
            .into_iter()
            .find(|p| p.id == id)
            .unwrap()
    }

    fn development(&self) -> Org {
        let s = self
            .workforce
            .create_department(&DepartmentInput {
                name: "Development".into(),
                description: String::new(),
                head: Some(lead(
                    &self.role("Manager"),
                    "Development Manager",
                    "claude-code",
                )),
                reports_to: None,
                active: None,
            })
            .unwrap();
        let d = s
            .departments
            .iter()
            .find(|d| d.name == "Development")
            .unwrap();
        let (department, head) = (d.id.clone(), d.head_position_id.clone().unwrap());
        let s = self
            .workforce
            .create_project(&ProjectInput {
                department_id: Some(department.clone()),
                coordinator: Some(lead(
                    &self.role("Supervisor"),
                    "Cloudline Coordinator",
                    "claude-code",
                )),
                ..project_input("Cloudline")
            })
            .unwrap();
        let project = s.projects.iter().find(|p| p.name == "Cloudline").unwrap();
        let coordinator = project.coordinator_position_id.clone().unwrap();
        let s = self
            .workforce
            .hire(&HireInput {
                role_id: self.role("Senior Developer"),
                title: "Senior Developer".into(),
                reports_to: Some(coordinator.clone()),
                runtime_id: Some("codex".into()),
                model: None,
                vacant: None,
                specialty_id: None,
            })
            .unwrap();
        let developer = s
            .positions
            .iter()
            .find(|p| p.title == "Senior Developer" && p.active)
            .unwrap()
            .id
            .clone();
        Org {
            department,
            project: project.id.clone(),
            head,
            coordinator,
            developer,
        }
    }

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
            assert!(Instant::now() < deadline, "task {id} never finished");
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        if let Some(session) = task.metadata["sessionId"].as_str() {
            while let Ok(detail) = self.rt.session(session).await {
                if detail.session.active_task_id.as_deref() != Some(id) {
                    break;
                }
                assert!(Instant::now() < deadline, "task {id} held its session");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
        task
    }

    async fn done(&self, position: &str, objective: &str) {
        let task = self.objective(position, objective).await;
        self.finished(&task).await;
    }

    fn fake_dir(&self) -> PathBuf {
        self.dir.path().join("home").join(".plenipo-fake-agent")
    }

    /// What the last turn's AI tool was started with.
    fn last_args(&self) -> Vec<String> {
        let text = std::fs::read_to_string(self.fake_dir().join("last-args.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// Every position titled here answers with `lesson` in a lesson block.
    fn answer_with_lesson(&self, titles: &[&str], lesson: &str) {
        let mut script = serde_json::Map::new();
        for t in titles {
            script.insert(
                (*t).to_owned(),
                json!([{ "say": format!("Done.\n```plenipo-lesson\n- {lesson}\n```") }]),
            );
        }
        let _ = std::fs::remove_dir_all(self.fake_dir().join("script-used"));
        std::fs::write(
            self.fake_dir().join("script.json"),
            serde_json::Value::Object(script).to_string(),
        )
        .unwrap();
    }

    fn lessons(&self, role: &str, text: &str) -> usize {
        [LessonState::Waiting, LessonState::Kept]
            .into_iter()
            .flat_map(|state| self.ledger.lessons(state, Some(role), 50).unwrap())
            .filter(|l| l.text == text)
            .count()
    }

    async fn until(&self, what: &str, pred: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT;
        while !pred() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

/// ADR-041 §7: an open conversation takes the effort set for its listed model, even when the AI
/// tool reported another name for it (the fake Claude Code calls its default model
/// "fake-claude-model").
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_conversation_takes_the_effort_set_for_its_listed_model() {
    let h = harness().await;
    let org = h.development();
    let first = h.objective(&org.coordinator, "Plan the release.").await;
    h.finished(&first).await;
    let session = h.task(&first).metadata["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let open = h.ledger.runtime_session(&session).unwrap().unwrap();
    assert_eq!(open.model.as_deref(), Some("fake-claude-model"));
    assert_eq!(open.effort, None);
    let default_model = h
        .router
        .snapshot()
        .unwrap()
        .models
        .into_iter()
        .find(|m| m.built_in && m.runtime_id == "claude-code")
        .unwrap()
        .id;
    h.workforce
        .set_model_rule(
            &RuleTarget::Organization,
            &ModelRule {
                efforts: [(default_model, Effort::Low)].into_iter().collect(),
                ..ModelRule::default()
            },
        )
        .unwrap();
    assert_eq!(
        h.ledger
            .runtime_session(&session)
            .unwrap()
            .unwrap()
            .effort
            .as_deref(),
        Some("low")
    );
}

/// ADR-041 §7: moved to another department, an agent's open conversation takes that
/// department's effort from its next task.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moved_agent_takes_its_new_departments_effort() {
    let h = harness().await;
    let org = h.development();
    let s = h
        .workforce
        .create_department(&DepartmentInput {
            name: "Operations".into(),
            description: String::new(),
            head: Some(lead(
                &h.role("Manager"),
                "Operations Manager",
                "claude-code",
            )),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let ops = s
        .departments
        .iter()
        .find(|d| d.name == "Operations")
        .unwrap();
    let (ops_id, ops_head) = (ops.id.clone(), ops.head_position_id.clone().unwrap());
    h.workforce
        .set_model_rule(
            &RuleTarget::Department(ops_id),
            &ModelRule {
                effort: Some(Effort::Medium),
                ..ModelRule::default()
            },
        )
        .unwrap();
    let first = h.objective(&org.coordinator, "Plan the release.").await;
    h.finished(&first).await;
    let session = h.task(&first).metadata["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let effort = |h: &H| h.ledger.runtime_session(&session).unwrap().unwrap().effort;
    assert_eq!(effort(&h), None);
    h.workforce
        .move_position(&org.coordinator, Some(&ops_head))
        .unwrap();
    assert_eq!(effort(&h).as_deref(), Some("medium"));
}

/// ADR-041 §3–§4: an AI company a rule never uses cannot be fixed for an agent under that rule;
/// one fixed before the rule shows it cannot start, and why.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_fixed_ai_tool_a_rule_never_uses_is_refused_in_plain_words() {
    let h = harness().await;
    let org = h.development();
    let openai = h
        .router
        .snapshot()
        .unwrap()
        .tools
        .into_iter()
        .find(|t| t.runtime_id == "codex")
        .unwrap()
        .company;
    h.workforce
        .set_model_rule(
            &RuleTarget::Department(org.department.clone()),
            &ModelRule {
                never_companies: vec![openai],
                ..ModelRule::default()
            },
        )
        .unwrap();
    // The developer was fixed on Codex before the rule: it cannot start, and says why.
    let developer = h.position(&org.developer);
    assert_eq!(developer.status, PositionStatus::Unavailable);
    let why = developer.status_detail.unwrap();
    assert!(why.contains("never uses"), "{why}");
    // Hiring, or changing an agent, onto Codex under that rule is refused.
    let refused = h
        .workforce
        .hire(&HireInput {
            role_id: h.role("Senior Developer"),
            title: "Second Developer".into(),
            reports_to: Some(org.coordinator.clone()),
            runtime_id: Some("codex".into()),
            model: None,
            vacant: None,
            specialty_id: None,
        })
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Second Developer") && refused.contains("never uses"),
        "{refused}"
    );
    let refused = h
        .workforce
        .update_position(
            &org.coordinator,
            &PositionPatchInput {
                runtime_id: Some("codex".into()),
                ..PositionPatchInput::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(refused.contains("never uses"), "{refused}");
    assert_eq!(
        h.position(&org.coordinator).runtime_id.as_deref(),
        Some("claude-code")
    );
    // Outside the department the rule does not apply.
    let s = h
        .workforce
        .create_department(&DepartmentInput {
            name: "Operations".into(),
            description: String::new(),
            head: Some(lead(&h.role("Manager"), "Operations Manager", "codex")),
            reports_to: None,
            active: None,
        })
        .unwrap();
    let ops = s
        .departments
        .iter()
        .find(|d| d.name == "Operations")
        .unwrap();
    assert!(ops.head_position_id.is_some());
}

/// ADR-041 §7: changing only effort keeps the agent and its conversation; the next task runs at
/// the new level, and the reason names the layer. Changing the model still hires a new agent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changing_only_effort_keeps_the_agent_and_its_conversation() {
    let h = harness().await;
    let org = h.development();
    let first = h.objective(&org.coordinator, "Plan the release.").await;
    h.finished(&first).await;
    let agent = h
        .ledger
        .position_incumbent(&org.coordinator)
        .unwrap()
        .unwrap();
    let session = h.task(&first).metadata["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    h.workforce
        .set_model_rule(
            &RuleTarget::Agent(org.coordinator.clone()),
            &ModelRule {
                effort: Some(Effort::High),
                ..ModelRule::default()
            },
        )
        .unwrap();
    assert_eq!(
        h.ledger
            .position_incumbent(&org.coordinator)
            .unwrap()
            .unwrap()
            .id,
        agent.id,
        "no new agent"
    );
    assert_eq!(
        h.ledger
            .runtime_session(&session)
            .unwrap()
            .unwrap()
            .effort
            .as_deref(),
        Some("high")
    );
    let second = h
        .objective(&org.coordinator, "Plan the next release.")
        .await;
    h.finished(&second).await;
    assert_eq!(h.task(&second).metadata["sessionId"], session.as_str());
    let args = h.last_args();
    assert!(
        args.windows(2)
            .any(|w| w[0] == "--effort" && w[1] == "high"),
        "{args:?}"
    );
    let route = h.position(&org.coordinator).route.unwrap();
    assert!(
        route
            .reason
            .ends_with("It runs at high effort, from this agent's own setting."),
        "{}",
        route.reason
    );
    // A model the AI tool does not take at that level is refused in plain words.
    let refused = h
        .workforce
        .set_model_rule(
            &RuleTarget::Agent(org.coordinator.clone()),
            &ModelRule {
                effort: Some(Effort::Ultra),
                ..ModelRule::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(refused.contains("does not take ultra effort"), "{refused}");
    // Changing the model still hires a new agent.
    h.workforce
        .update_position(
            &org.coordinator,
            &PositionPatchInput {
                model: Some("opus".into()),
                ..PositionPatchInput::default()
            },
        )
        .unwrap();
    assert_ne!(
        h.ledger
            .position_incumbent(&org.coordinator)
            .unwrap()
            .unwrap()
            .id,
        agent.id
    );
}

/// ADR-041 §10–§12: learning off at the organization stops all learning; off for one agent stops
/// only that agent; an agent set on inside a role that is off learns (the closest wins).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn learning_follows_the_closest_setting_under_the_main_switch() {
    let h = harness().await;
    let org = h.development();
    let supervisor = h.role("Supervisor");
    let manager = h.role("Manager");
    let coordinator = || h.ledger.position(&org.coordinator).unwrap().unwrap();
    let head = || h.ledger.position(&org.head).unwrap().unwrap();

    // Off for one agent: only that agent stops learning.
    h.workforce
        .set_agent_learning(&org.coordinator, Some(false))
        .unwrap();
    assert!(learning::instructions(&h.ledger, &coordinator(), "Supervisor").is_empty());
    assert!(!learning::instructions(&h.ledger, &head(), "Manager").is_empty());
    let p = h.position(&org.coordinator);
    assert_eq!(
        (p.learning.learns, p.learning.from, p.learning.own),
        (false, LearningFrom::Agent, Some(false))
    );
    h.answer_with_lesson(
        &["Cloudline Coordinator", "Development Manager"],
        "Check the release notes.",
    );
    h.done(&org.coordinator, "Plan the release.").await;
    h.done(&org.head, "Plan the quarter.").await;
    h.until("the manager's lesson", || {
        h.lessons(&manager, "Check the release notes.") == 1
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.lessons(&supervisor, "Check the release notes."), 0);

    // The role off, the agent on: the agent learns.
    h.workforce.set_role_learns(&supervisor, false).unwrap();
    h.workforce
        .set_agent_learning(&org.coordinator, Some(true))
        .unwrap();
    assert!(!learning::instructions(&h.ledger, &coordinator(), "Supervisor").is_empty());
    h.answer_with_lesson(&["Cloudline Coordinator"], "Ask QA first.");
    h.done(&org.coordinator, "Plan the next release.").await;
    h.until("the supervisor's lesson", || {
        h.lessons(&supervisor, "Ask QA first.") == 1
    })
    .await;
    // Following its role again: off.
    h.workforce
        .set_agent_learning(&org.coordinator, None)
        .unwrap();
    let p = h.position(&org.coordinator);
    assert_eq!(
        (p.learning.learns, p.learning.from),
        (false, LearningFrom::Role)
    );
    assert!(learning::instructions(&h.ledger, &coordinator(), "Supervisor").is_empty());

    // The main switch off stops all learning, even an agent set on.
    h.workforce
        .set_agent_learning(&org.coordinator, Some(true))
        .unwrap();
    h.workforce.set_learning(false).unwrap();
    let p = h.position(&org.coordinator);
    assert_eq!(
        (p.learning.learns, p.learning.from),
        (false, LearningFrom::Organization)
    );
    h.answer_with_lesson(
        &["Cloudline Coordinator", "Development Manager"],
        "Nothing now.",
    );
    h.done(&org.coordinator, "Plan again.").await;
    h.done(&org.head, "Plan again.").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.lessons(&supervisor, "Nothing now."), 0);
    assert_eq!(h.lessons(&manager, "Nothing now."), 0);
    assert!(learning::instructions(&h.ledger, &head(), "Manager").is_empty());
}

/// ADR-042: hire a Senior Developer with the Database specialty; its lines reach its
/// instructions; the owner adds a specialty of their own; changing it keeps the agent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_senior_developer_with_the_database_specialty() {
    let h = harness().await;
    let org = h.development();
    let s = h.snapshot();
    let senior = s
        .roles
        .iter()
        .find(|r| r.name == "Senior Developer")
        .unwrap();
    let names: Vec<&str> = senior.specialties.iter().map(|x| x.name.as_str()).collect();
    for name in [
        "Front-end",
        "Back-end",
        "Database",
        "UX/UI",
        "Mobile",
        "DevOps",
        "Data",
    ] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }
    let db = senior
        .specialties
        .iter()
        .find(|x| x.name == "Database")
        .unwrap();
    assert!(db.built_in);
    let s = h
        .workforce
        .hire(&HireInput {
            role_id: senior.id.clone(),
            title: db.title.clone(),
            reports_to: Some(org.coordinator.clone()),
            runtime_id: Some("codex".into()),
            model: None,
            vacant: None,
            specialty_id: Some(db.id.clone()),
        })
        .unwrap();
    let dba = s
        .positions
        .iter()
        .find(|p| p.title == "Database Developer")
        .unwrap();
    assert_eq!(dba.specialty.as_deref(), Some("Database"));
    let directory = WorkforceDirectory::new(Arc::clone(&h.ledger), h.router.clone());
    let team = directory
        .team(&json!({ "positionId": dba.id }))
        .expect("its team");
    assert!(team
        .identity
        .contains("Your job as Senior Developer (Database):"));
    assert!(team
        .identity
        .contains("never delete or rewrite real data: work on test data or a copy"));
    // Its lead's team lists it with its specialty.
    let lead_team = directory
        .team(&json!({ "positionId": org.coordinator }))
        .unwrap();
    assert!(lead_team
        .members
        .iter()
        .any(|m| m.label.starts_with("Senior Developer (Database)")));
    // The owner's own specialty, on a role of any kind; changing it keeps the agent.
    let supervisor = h.role("Supervisor");
    let s = h
        .workforce
        .create_specialty(&SpecialtyInput {
            role_id: Some(supervisor.clone()),
            name: "Releases".into(),
            title: "Release Supervisor".into(),
            job: RoleJob {
                duties: vec!["plan each release with its checklist".into()],
                ..RoleJob::default()
            },
            suggest: SpecialtySuggest::default(),
        })
        .unwrap();
    let releases = s
        .roles
        .iter()
        .find(|r| r.id == supervisor)
        .unwrap()
        .specialties
        .iter()
        .find(|x| x.name == "Releases")
        .unwrap()
        .clone();
    assert!(!releases.built_in);
    let agent = h
        .ledger
        .position_incumbent(&org.coordinator)
        .unwrap()
        .unwrap();
    h.workforce
        .update_position(
            &org.coordinator,
            &PositionPatchInput {
                specialty_id: Some(releases.id.clone()),
                ..PositionPatchInput::default()
            },
        )
        .unwrap();
    assert_eq!(
        h.ledger
            .position_incumbent(&org.coordinator)
            .unwrap()
            .unwrap()
            .id,
        agent.id
    );
    assert_eq!(
        h.position(&org.coordinator).specialty.as_deref(),
        Some("Releases")
    );
    // A suggested model that has left the owner's list is dropped, not refused.
    let s = h
        .workforce
        .update_specialty(
            &releases.id,
            &SpecialtyInput {
                role_id: None,
                name: "Releases".into(),
                title: "Release Supervisor".into(),
                job: RoleJob {
                    duties: vec!["plan each release with its checklist".into()],
                    ..RoleJob::default()
                },
                suggest: SpecialtySuggest {
                    models: vec!["a-model-removed-since".into()],
                    ..SpecialtySuggest::default()
                },
            },
        )
        .unwrap();
    let kept = s
        .roles
        .iter()
        .flat_map(|r| &r.specialties)
        .find(|x| x.id == releases.id)
        .unwrap();
    assert!(kept.suggest.models.is_empty());
    // A built-in specialty cannot be changed; a suggested permission must be a real one.
    assert!(h
        .workforce
        .update_specialty(
            &db.id,
            &SpecialtyInput {
                role_id: None,
                name: "Data stores".into(),
                title: String::new(),
                job: RoleJob::default(),
                suggest: SpecialtySuggest::default(),
            }
        )
        .is_err());
    assert!(h
        .workforce
        .create_specialty(&SpecialtyInput {
            role_id: Some(supervisor),
            name: "Anything".into(),
            title: String::new(),
            job: RoleJob::default(),
            suggest: SpecialtySuggest {
                permissions: vec!["everything".into()],
                ..SpecialtySuggest::default()
            },
        })
        .is_err());
}

/// ADR-043 and ADR-045: archive, bring back, archive again, delete for good, and older work still
/// names it; deleting a project offers its experienced agents to the Workforce; one saved is hired
/// again with its experience, rule, and learning setting.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn archive_bring_back_delete_and_the_workforce() {
    let h = harness().await;
    let org = h.development();
    // Experience: the supervisor finishes three tasks, the manager one.
    for n in 0..3 {
        h.done(&org.coordinator, &format!("Plan release {n}."))
            .await;
    }
    h.done(&org.head, "Plan the quarter.").await;
    let s = h.snapshot();
    assert_eq!(s.average_experience, 2);
    let c = s
        .positions
        .iter()
        .find(|p| p.id == org.coordinator)
        .unwrap();
    assert_eq!((c.experience.tasks_done, c.experience.score), (3, 3));
    assert!(c.experience.experienced);
    let m = s.positions.iter().find(|p| p.id == org.head).unwrap();
    assert!(!m.experience.experienced);

    // An agent on its own: archive, bring back, archive, delete for good.
    let s = h
        .workforce
        .hire(&HireInput {
            role_id: h.role("Code Reviewer"),
            title: "Cloudline Reviewer".into(),
            reports_to: Some(org.coordinator.clone()),
            runtime_id: Some("codex".into()),
            model: None,
            vacant: None,
            specialty_id: None,
        })
        .unwrap();
    let reviewer = s
        .positions
        .iter()
        .find(|p| p.title == "Cloudline Reviewer")
        .unwrap()
        .id
        .clone();
    h.workforce.archive_position(&reviewer).unwrap();
    let s = h.workforce.bring_back_position(&reviewer).unwrap();
    assert!(s.positions.iter().any(|p| p.id == reviewer && p.active));
    h.workforce.archive_position(&reviewer).unwrap();
    let preview = h.workforce.preview_delete("position", &reviewer).unwrap();
    assert_eq!(preview.agents.len(), 1);
    let (s, _) = h
        .workforce
        .delete_for_good("position", &reviewer, &[])
        .unwrap();
    let record = s.positions.iter().find(|p| p.id == reviewer).unwrap();
    assert!(record.deleted && !record.active);
    assert_eq!(record.title, "Cloudline Reviewer");

    // The supervisor's own rule and learning setting travel with it.
    h.workforce
        .set_model_rule(
            &RuleTarget::Agent(org.coordinator.clone()),
            &ModelRule {
                effort: Some(Effort::Low),
                ..ModelRule::default()
            },
        )
        .unwrap();
    h.workforce
        .set_agent_learning(&org.coordinator, Some(false))
        .unwrap();

    // Deleting the project for good offers its agents, the experienced ones marked.
    h.workforce.archive_project(&org.project).unwrap();
    let preview = h.workforce.preview_delete("project", &org.project).unwrap();
    assert_eq!(preview.projects, ["Cloudline"]);
    let offered: Vec<(&str, bool)> = preview
        .agents
        .iter()
        .map(|a| (a.title.as_str(), a.experience.experienced))
        .collect();
    assert_eq!(
        offered,
        [("Cloudline Coordinator", true), ("Senior Developer", false)]
    );
    let (s, deleted) = h
        .workforce
        .delete_for_good(
            "project",
            &org.project,
            std::slice::from_ref(&org.coordinator),
        )
        .unwrap();
    assert_eq!(deleted.saved.len(), 1);
    assert_eq!(s.workforce.len(), 1);
    let saved = &s.workforce[0];
    assert_eq!(saved.title, "Cloudline Coordinator");
    assert_eq!(saved.experience.tasks_done, 3);
    assert_eq!(saved.places, ["Cloudline", "Development"]);
    let record = s
        .positions
        .iter()
        .find(|p| p.id == org.coordinator)
        .unwrap();
    assert!(record.deleted && record.in_workforce);
    let short = s.positions.iter().find(|p| p.id == org.developer).unwrap();
    assert!(short.deleted && !short.in_workforce);
    // Older work still shows who did it.
    let home = h.workforce.home().unwrap();
    assert!(home
        .finished
        .iter()
        .any(|o| o.position_title.as_deref() == Some("Cloudline Coordinator")));
    // Its rules were forgotten on the chart, and kept with it in the Workforce.
    assert!(h.router.rule_of(&org.coordinator).unwrap().is_none());

    // Hired again as the supervisor of a new project: its experience and settings come back.
    let s = h
        .workforce
        .create_project(&ProjectInput {
            department_id: Some(org.department.clone()),
            coordinator: Some(LeadInput {
                from_workforce: Some(saved.id.clone()),
                ..lead(&h.role("Supervisor"), "Mobile Supervisor", "claude-code")
            }),
            ..project_input("Mobile")
        })
        .unwrap();
    assert!(s.workforce.is_empty());
    let back = s
        .positions
        .iter()
        .find(|p| p.title == "Mobile Supervisor" && p.active)
        .unwrap();
    assert_eq!(back.experience.tasks_done, 3);
    assert_eq!(back.own_rule.as_ref().unwrap().effort, Some(Effort::Low));
    assert_eq!(back.learning.own, Some(false));
    // Its lead's department is still here; the project it came from is a short record.
    let old = s.projects.iter().find(|p| p.id == org.project).unwrap();
    assert!(old.deleted);
    assert_eq!(old.name, "Cloudline (deleted)");
}
