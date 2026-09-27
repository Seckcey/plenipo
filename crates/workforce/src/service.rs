//! The Workforce service (ADR-009): the organization's snapshot and work views, the owner's
//! changes to it, and objectives for its persistent agents. Structure rules are enforced by
//! the Ledger in each change's own transaction; this service checks what only it knows —
//! which runtimes exist — and talks to Liaison and the agent runtime.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use plenipo_ledger::{
    Ledger, NewPosition, OversightKind, PositionPatch, ProjectSettings, RoleType, Task, TaskState,
};
use plenipo_liaison::Liaison;
use plenipo_router::Router;
use plenipo_runtime::agent::{
    AgentRuntime, AgentRuntimeInfo, AgentSessionDetail, InstallState, SessionStart,
};
use serde_json::{json, Value};

use crate::conversation::{self, ConversationPlan};
use crate::directory::WorkforceDirectory;
use crate::dto::*;
use crate::error::{Result, WorkforceError};
use crate::outcome::{self, ObjectiveReport};
use crate::snapshot::{self, Inputs};
use crate::templates::{self, default_glyph, role_templates, template_policies};
use crate::view::OrgView;

/// Actor recorded for the owner's changes.
pub const OWNER: &str = "owner";
/// Actor recorded for Plenipo's own changes (seeding role templates).
pub const PLENIPO: &str = "plenipo";
/// Settings key of the organization's name.
const ORGANIZATION: &str = "organization";
const DEFAULT_NAME: &str = "Organization";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// The organization's name (or a neutral default).
pub(crate) fn org_name(ledger: &Ledger) -> String {
    ledger
        .setting(ORGANIZATION)
        .ok()
        .flatten()
        .and_then(|v| v["name"].as_str().map(str::to_owned))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_NAME.to_owned())
}

/// What the owner chose to call the ranks (Business when unset or unknown).
fn org_titles(ledger: &Ledger) -> TitleTheme {
    ledger
        .setting(ORGANIZATION)
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v["titles"].clone()).ok())
        .unwrap_or_default()
}

fn invalid(message: impl Into<String>) -> WorkforceError {
    WorkforceError::Invalid(message.into())
}

struct Inner {
    ledger: Arc<Ledger>,
    runtime: AgentRuntime,
    liaison: Liaison,
    router: Router,
    notices: Mutex<Vec<String>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Workforce {
    inner: Arc<Inner>,
}

/// Most lines in each part of a role's working instructions, and the longest line.
pub const MAX_JOB_LINES: usize = 12;
pub const MAX_JOB_LINE_CHARS: usize = 300;

/// A role's working instructions as the owner wrote them (ADR-019): one item per line, list
/// markers ("- ", "* ", "• ") and blank lines removed.
pub fn clean_job(job: &RoleJob) -> Result<RoleJob> {
    let clean = |what: &str, items: &[String]| -> Result<Vec<String>> {
        let mut out = Vec::new();
        for line in items.iter().flat_map(|i| i.lines()) {
            let line = line
                .trim()
                .trim_start_matches(['-', '*', '•'])
                .trim()
                .to_owned();
            if line.is_empty() {
                continue;
            }
            if line.chars().count() > MAX_JOB_LINE_CHARS || line.chars().any(char::is_control) {
                return Err(invalid(format!(
                    "each line of {what} must be one line of at most {MAX_JOB_LINE_CHARS} \
                     characters"
                )));
            }
            out.push(line);
        }
        if out.len() > MAX_JOB_LINES {
            return Err(invalid(format!(
                "{what} can have at most {MAX_JOB_LINES} lines"
            )));
        }
        Ok(out)
    };
    Ok(RoleJob {
        duties: clean("what the role does", &job.duties)?,
        returns: clean("what it hands back", &job.returns)?,
        limits: clean("what it must not do", &job.limits)?,
        ask_lead: clean("when it asks its lead for help", &job.ask_lead)?,
    })
}

impl Workforce {
    /// Create the service: seed missing role templates (and their starting model policies)
    /// and install the organization's directory in Liaison, so members address their teams by
    /// role and each new worker is routed by its role's model policy.
    pub fn new(
        ledger: Arc<Ledger>,
        runtime: AgentRuntime,
        liaison: Liaison,
        router: Router,
    ) -> Self {
        let this = Self {
            inner: Arc::new(Inner {
                ledger: Arc::clone(&ledger),
                runtime,
                liaison: liaison.clone(),
                router: router.clone(),
                notices: Mutex::new(Vec::new()),
            }),
        };
        match ledger.ensure_roles(&role_templates(), PLENIPO) {
            Ok(roles) => {
                let defaults: Vec<_> = template_policies()
                    .into_iter()
                    .filter_map(|(name, policy)| {
                        roles
                            .iter()
                            .find(|r| r.name == name && r.metadata["template"] == true)
                            .map(|r| (r.id.clone(), policy))
                    })
                    .collect();
                if let Err(e) = router.seed_policies(&defaults) {
                    this.notice(format!(
                        "Could not add the built-in roles' model choices: {e}"
                    ));
                }
            }
            Err(e) => this.notice(format!("Could not add the built-in role templates: {e}")),
        }
        // Lessons from workers' answers (ADR-024).
        crate::learning::watch(&ledger);
        liaison.set_directory(Arc::new(WorkforceDirectory::new(ledger, router)));
        this
    }

    // ---- Learning (ADR-024) ----------------------------------------------------------------

    /// Learning's settings and the lessons waiting and kept.
    pub fn learning(&self) -> Result<crate::learning::LearningSnapshot> {
        crate::learning::snapshot(&self.inner.ledger)
    }

    /// Worker learning on or off (Settings → Switches).
    pub fn set_learning(&self, enabled: bool) -> Result<crate::learning::LearningSnapshot> {
        crate::learning::set_enabled(&self.inner.ledger, enabled)?;
        self.learning()
    }

    /// Whether a role learns on its own (its lessons kept without asking).
    pub fn set_role_learning(
        &self,
        role_id: &str,
        auto: bool,
    ) -> Result<crate::learning::LearningSnapshot> {
        crate::learning::set_role(&self.inner.ledger, role_id, auto)?;
        self.learning()
    }

    /// Keep (in the owner's wording, when given) or discard a waiting lesson.
    pub fn decide_lesson(
        &self,
        lesson_id: &str,
        keep: bool,
        text: Option<&str>,
    ) -> Result<crate::learning::LearningSnapshot> {
        self.inner
            .ledger
            .decide_lesson(lesson_id, keep, text, OWNER)?;
        self.learning()
    }

    /// Remove a kept lesson: the role's later workers no longer get it.
    pub fn remove_lesson(&self, lesson_id: &str) -> Result<crate::learning::LearningSnapshot> {
        self.inner.ledger.remove_lesson(lesson_id, OWNER)?;
        self.learning()
    }

    fn notices(&self) -> MutexGuard<'_, Vec<String>> {
        self.inner.notices.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn notice(&self, notice: String) {
        let mut notices = self.notices();
        if !notices.contains(&notice) {
            notices.push(notice);
        }
    }

    fn ledger(&self) -> &Ledger {
        &self.inner.ledger
    }

    fn runtimes(&self) -> Vec<AgentRuntimeInfo> {
        self.inner.runtime.runtimes()
    }

    /// A runtime this build has, with its provider.
    fn runtime(&self, id: &str) -> Result<AgentRuntimeInfo> {
        let id = id.trim();
        self.runtimes()
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| invalid(format!("there is no AI tool named {id:?}")))
    }

    // ---- Reads --------------------------------------------------------------------------

    /// The whole organization, with live status (blocking: reads the Ledger).
    pub fn snapshot(&self) -> Result<OrgSnapshot> {
        let l = self.ledger();
        let now = plenipo_ledger::now_ms();
        let records = l.org_records()?;
        let open_tasks = l.open_workforce_tasks()?;
        let finished_recent = l.finished_workforce_tasks(now.saturating_sub(DAY_MS), 1000)?;
        let sessions = l.open_workforce_sessions()?;
        let planner = self.inner.router.planner()?;
        Ok(snapshot::build(&Inputs {
            records: &records,
            open_tasks: &open_tasks,
            finished_recent: &finished_recent,
            sessions: &sessions,
            planner: &planner,
            name: org_name(l),
            titles: org_titles(l),
            notices: self.notices().clone(),
            now,
        }))
    }

    /// The work a position owns and its team's unfinished work; `None`: the whole
    /// organization's unfinished and recent work.
    pub fn work(&self, position_id: Option<&str>) -> Result<WorkView> {
        let l = self.ledger();
        let records = l.org_records()?;
        let open_tasks = l.open_workforce_tasks()?;
        let own = match position_id {
            Some(id) => {
                if !records.positions.iter().any(|p| p.id == id) {
                    return Err(WorkforceError::Ledger(
                        plenipo_ledger::LedgerError::NotFound(format!("position {id}")),
                    ));
                }
                l.position_tasks(id, 200)?
            }
            None => {
                let mut all = open_tasks.clone();
                let week = plenipo_ledger::now_ms().saturating_sub(7 * DAY_MS);
                all.extend(l.finished_workforce_tasks(week, 50)?);
                all
            }
        };
        Ok(snapshot::work_view(
            &records,
            position_id,
            &own,
            &open_tasks,
        ))
    }

    /// Plenipo's record of the objective `task_id` belongs to: every task, worker, file, test,
    /// branch, pull request, finding, and approval of it (Phase 8, ADR-016). Blocking: reads
    /// the Ledger.
    pub fn objective_report(&self, task_id: &str) -> Result<ObjectiveReport> {
        let l = self.ledger();
        let root = l.task_root(task_id)?;
        let descendants = l.descendant_tasks(&root.id)?;
        let mut events = HashMap::new();
        let mut approvals = HashMap::new();
        for t in std::iter::once(&root).chain(descendants.iter().map(|(t, _)| t)) {
            events.insert(t.id.clone(), l.events_for_task(&t.id)?);
            approvals.insert(t.id.clone(), l.approvals_for_task(&t.id)?);
        }
        let workspaces = match root.metadata["liaison"]["correlationId"].as_str() {
            Some(c) => l.workflow_workspaces(c)?,
            None => Vec::new(),
        };
        let records = l.org_records()?;
        let view = OrgView::new(&records);
        let runtime_labels: HashMap<String, String> = self
            .runtimes()
            .into_iter()
            .map(|r| (r.id, r.label))
            .collect();
        Ok(outcome::build(&outcome::Inputs {
            root: &root,
            descendants: &descendants,
            events: &events,
            approvals: &approvals,
            workspaces: &workspaces,
            view: &view,
            runtime_labels: &runtime_labels,
        }))
    }

    /// A project's recent objectives and its working copies (Phase 8: the Projects page).
    pub fn project_work(&self, project_id: &str) -> Result<ProjectWork> {
        let l = self.ledger();
        if l.project(project_id)?.is_none() {
            return Err(WorkforceError::Ledger(
                plenipo_ledger::LedgerError::NotFound(format!("project {project_id}")),
            ));
        }
        let records = l.org_records()?;
        let view = OrgView::new(&records);
        let pending: Vec<String> = l
            .pending_approvals()?
            .into_iter()
            .map(|a| a.task_id)
            .collect();
        let objectives = l
            .project_objectives(project_id, 20)?
            .into_iter()
            .map(|root| brief(l, &view, &pending, root, Some(project_id)))
            .collect::<Result<Vec<_>>>()?;
        Ok(ProjectWork {
            project_id: project_id.to_owned(),
            objectives,
            working_copies: l.project_workspaces(project_id, 20)?,
        })
    }

    /// Home (Phase 12): the objectives still going, those finished in the last week with their
    /// answers, and what is stuck (the last week).
    pub fn home(&self) -> Result<HomeView> {
        const DAY_MS: u64 = 24 * 3_600_000;
        const WEEK_MS: u64 = 7 * DAY_MS;
        let l = self.ledger();
        let records = l.org_records()?;
        let view = OrgView::new(&records);
        let pending: Vec<String> = l
            .pending_approvals()?
            .into_iter()
            .map(|a| a.task_id)
            .collect();
        let now = plenipo_ledger::now_ms();
        let since = now.saturating_sub(WEEK_MS);
        let (going, finished_day) = l.objective_counts(now.saturating_sub(DAY_MS))?;
        let current = l
            .open_objectives(50)?
            .into_iter()
            .map(|root| brief(l, &view, &pending, root, None))
            .collect::<Result<Vec<_>>>()?;
        // The last to finish first.
        let finished = l
            .finished_objectives(since, 20)?
            .into_iter()
            .map(|root| brief(l, &view, &pending, root, None))
            .collect::<Result<Vec<_>>>()?;
        let titles: HashMap<&str, &str> = records
            .positions
            .iter()
            .map(|p| (p.id.as_str(), p.title.as_str()))
            .collect();
        let mut stuck = Vec::new();
        for event in l.problems(since, 30)? {
            let task = match event.task_id.as_deref() {
                Some(id) => l.task(id)?.map(|t| snapshot::brief(&t, &titles)),
                None => None,
            };
            stuck.push(StuckItem { event, task });
        }
        Ok(HomeView {
            current,
            finished,
            stuck,
            going,
            finished_day,
        })
    }

    // ---- The organization ---------------------------------------------------------------

    pub fn rename(&self, name: &str) -> Result<OrgSnapshot> {
        let name = plenipo_ledger::workforce::clean_line("the organization's name", name, 80)?;
        self.ledger()
            .merge_setting(ORGANIZATION, &json!({ "name": name }), OWNER)?;
        self.snapshot()
    }

    /// Choose what the app calls the ranks. Display only: agents keep the plain titles.
    pub fn set_titles(&self, titles: TitleTheme) -> Result<OrgSnapshot> {
        self.ledger()
            .merge_setting(ORGANIZATION, &json!({ "titles": titles }), OWNER)?;
        self.snapshot()
    }

    pub fn create_role(&self, input: &RoleInput) -> Result<OrgSnapshot> {
        let name = plenipo_ledger::workforce::clean_line("the role name", &input.name, 80)?;
        let description = input.description.trim();
        if description.chars().count() > 2000 {
            return Err(invalid("the description must be at most 2000 characters"));
        }
        let role_type = match input.kind {
            PositionKind::Superintendent => RoleType::Superintendent,
            PositionKind::DepartmentManager => RoleType::DepartmentManager,
            PositionKind::ProjectCoordinator => RoleType::ProjectCoordinator,
            PositionKind::Worker => RoleType::Worker,
        };
        let persistent = input.staffing == Staffing::Persistent;
        if role_type != RoleType::Worker && !persistent {
            return Err(invalid("VPs, managers, and supervisors are full-time"));
        }
        if self
            .ledger()
            .list_roles()?
            .iter()
            .any(|r| r.name.eq_ignore_ascii_case(&name))
        {
            return Err(invalid(format!("a role named \"{name}\" already exists")));
        }
        let purpose: Vec<&str> = if description.is_empty() {
            Vec::new()
        } else {
            vec![description.trim_end_matches('.')]
        };
        let job = clean_job(&input.job.clone().unwrap_or_default())?;
        self.ledger().create_role(
            &name,
            description,
            role_type,
            persistent,
            &json!({ "glyph": default_glyph(role_type), "purpose": purpose, "job": job }),
            OWNER,
        )?;
        self.snapshot()
    }

    /// Change a role the owner created: its name, description, and working instructions
    /// (ADR-019). Built-in roles keep theirs.
    pub fn update_role(&self, role_id: &str, input: &RoleUpdate) -> Result<OrgSnapshot> {
        let name = plenipo_ledger::workforce::clean_line("the role name", &input.name, 80)?;
        let description = input.description.trim();
        if description.chars().count() > 2000 {
            return Err(invalid("the description must be at most 2000 characters"));
        }
        let roles = self.ledger().list_roles()?;
        let role = roles
            .iter()
            .find(|r| r.id == role_id)
            .ok_or_else(|| invalid("that role no longer exists"))?;
        if role.metadata["template"] == true {
            return Err(invalid(
                "built-in roles keep their instructions; create a role of your own to write \
                 different ones",
            ));
        }
        if roles
            .iter()
            .any(|r| r.id != role_id && r.name.eq_ignore_ascii_case(&name))
        {
            return Err(invalid(format!("a role named \"{name}\" already exists")));
        }
        let mut metadata = role.metadata.clone();
        if !metadata.is_object() {
            metadata = json!({});
        }
        metadata["purpose"] = if description.is_empty() {
            json!([])
        } else {
            json!([description.trim_end_matches('.')])
        };
        metadata["job"] = json!(clean_job(&input.job)?);
        self.ledger()
            .update_role(role_id, &name, description, &metadata, OWNER)?;
        self.snapshot()
    }

    /// A fixed runtime and its provider, or `(None, None)`: automatic.
    fn fixed_runtime(&self, id: Option<&str>) -> Result<(Option<String>, Option<String>)> {
        match id {
            Some(id) => {
                let r = self.runtime(id)?;
                Ok((Some(r.id), Some(r.provider)))
            }
            None => Ok((None, None)),
        }
    }

    fn lead(&self, input: &LeadInput, reports_to: Option<String>) -> Result<NewPosition> {
        let (runtime_id, runtime_provider) = self.fixed_runtime(input.runtime_id.as_deref())?;
        Ok(NewPosition {
            title: input.title.clone(),
            role_id: input.role_id.clone(),
            reports_to,
            runtime_id,
            runtime_provider,
            model: input.model.clone(),
            staffed: !input.vacant.unwrap_or(false),
        })
    }

    pub fn create_department(&self, input: &DepartmentInput) -> Result<OrgSnapshot> {
        let head = input
            .head
            .as_ref()
            .ok_or_else(|| invalid("a new department needs a head position"))?;
        let head = self.lead(head, input.reports_to.clone())?;
        self.ledger()
            .create_department_with_head(&input.name, &input.description, &head, OWNER)?;
        self.snapshot()
    }

    pub fn update_department(&self, id: &str, input: &DepartmentInput) -> Result<OrgSnapshot> {
        let current = self.ledger().department(id)?.ok_or_else(|| {
            WorkforceError::Ledger(plenipo_ledger::LedgerError::NotFound(format!(
                "department {id}"
            )))
        })?;
        let active = input.active.unwrap_or(current.status == "active");
        self.ledger().update_department_details(
            id,
            &input.name,
            &input.description,
            active,
            OWNER,
        )?;
        self.snapshot()
    }

    pub fn remove_department(&self, id: &str) -> Result<OrgSnapshot> {
        self.ledger().remove_department(id, OWNER)?;
        self.snapshot()
    }

    fn settings(&self, input: &ProjectInput) -> Result<ProjectSettings> {
        for r in &input.allowed_runtimes {
            self.runtime(r)?;
        }
        Ok(ProjectSettings {
            name: input.name.clone(),
            description: input.description.clone(),
            repository_url: input.repository_url.clone(),
            local_path: input.local_path.clone(),
            allowed_runtimes: input.allowed_runtimes.clone(),
            capability_profile: input.capability_profile.clone(),
            branch_per_objective: input.branch_per_objective,
        })
    }

    pub fn create_project(&self, input: &ProjectInput) -> Result<OrgSnapshot> {
        let department_id = input
            .department_id
            .as_deref()
            .ok_or_else(|| invalid("a new project belongs to a department"))?;
        let coordinator = input
            .coordinator
            .as_ref()
            .ok_or_else(|| invalid("a new project needs a supervisor position"))?;
        let settings = self.settings(input)?;
        let coordinator = self.lead(coordinator, None)?;
        self.ledger().create_project_with_coordinator(
            department_id,
            &settings,
            &coordinator,
            OWNER,
        )?;
        self.snapshot()
    }

    /// Set up a software project from the Development template (Phase 8): the Development
    /// department with its VP when there is none, then the project with its supervisor and the
    /// standard team (on call, each routed by its role's model choices). Everything is checked
    /// first; each change is recorded as the owner's.
    pub fn set_up_development(&self, input: &DevelopmentInput) -> Result<OrgSnapshot> {
        let t = &templates::DEVELOPMENT;
        let roles = self.ledger().list_roles()?;
        let role = |name: &str| {
            roles
                .iter()
                .find(|r| r.name == name && r.metadata["template"] == true)
                .map(|r| r.id.clone())
                .ok_or_else(|| invalid(format!("the built-in role {name} is missing")))
        };
        let head_role = role(t.head.1)?;
        let supervisor_role = role(t.supervisor_role)?;
        let team: Vec<(&str, String)> = t
            .team
            .iter()
            .map(|(title, r)| Ok((*title, role(r)?)))
            .collect::<Result<_>>()?;
        let settings = self.settings(&input.project)?;
        let name = plenipo_ledger::workforce::clean_line("the project name", &settings.name, 200)?;
        if self
            .ledger()
            .list_projects()?
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&name))
        {
            return Err(invalid(format!(
                "a project named \"{name}\" already exists"
            )));
        }
        let runtime = input
            .runtime_id
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty());
        if let Some(r) = runtime {
            self.runtime(r)?;
            if !settings.allowed_runtimes.iter().any(|a| a == r) {
                return Err(invalid(
                    "the supervisor's AI tool must be one of the project's allowed AI tools",
                ));
            }
        }
        let lead = |role_id: &str, title: String| LeadInput {
            role_id: role_id.to_owned(),
            title,
            runtime_id: runtime.map(str::to_owned),
            model: None,
            vacant: None,
        };
        let department = match self
            .ledger()
            .list_departments()?
            .into_iter()
            .find(|d| d.status == "active" && d.name.eq_ignore_ascii_case(t.department))
        {
            Some(d) => d.id,
            None => {
                let s = self.create_department(&DepartmentInput {
                    name: t.department.into(),
                    description: t.description.into(),
                    head: Some(lead(&head_role, t.head.0.into())),
                    reports_to: None,
                    active: None,
                })?;
                s.departments
                    .into_iter()
                    .find(|d| d.name == t.department)
                    .map(|d| d.id)
                    .ok_or_else(|| {
                        WorkforceError::Internal("the new department is missing".into())
                    })?
            }
        };
        let s = self.create_project(&ProjectInput {
            department_id: Some(department),
            coordinator: Some(lead(&supervisor_role, format!("{name} Supervisor"))),
            ..input.project.clone()
        })?;
        let supervisor = s
            .projects
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.coordinator_position_id.clone())
            .ok_or_else(|| WorkforceError::Internal("the new project is missing".into()))?;
        for (title, role_id) in team {
            self.hire(&HireInput {
                role_id,
                title: title.into(),
                reports_to: Some(supervisor.clone()),
                runtime_id: None,
                model: None,
                vacant: None,
            })?;
        }
        self.snapshot()
    }

    pub fn update_project(&self, id: &str, input: &ProjectInput) -> Result<OrgSnapshot> {
        let settings = self.settings(input)?;
        self.ledger()
            .update_project_settings(id, &settings, OWNER)?;
        self.snapshot()
    }

    pub fn archive_project(&self, id: &str) -> Result<OrgSnapshot> {
        self.ledger().archive_project(id, OWNER)?;
        self.snapshot()
    }

    // ---- Positions ----------------------------------------------------------------------

    /// Hire into a team: a new position (and, for a persistent one, its agent).
    pub fn hire(&self, input: &HireInput) -> Result<OrgSnapshot> {
        let (runtime_id, runtime_provider) = self.fixed_runtime(input.runtime_id.as_deref())?;
        self.ledger().create_position(
            &NewPosition {
                title: input.title.clone(),
                role_id: input.role_id.clone(),
                reports_to: input.reports_to.clone(),
                runtime_id,
                runtime_provider,
                model: input.model.clone(),
                staffed: !input.vacant.unwrap_or(false),
            },
            OWNER,
        )?;
        self.snapshot()
    }

    /// Hire an agent into a vacant persistent position.
    pub fn fill(&self, position_id: &str) -> Result<OrgSnapshot> {
        let position = self.ledger().position(position_id)?.ok_or_else(|| {
            WorkforceError::Ledger(plenipo_ledger::LedgerError::NotFound(format!(
                "position {position_id}"
            )))
        })?;
        let (_, provider) = self.fixed_runtime(position.runtime_id.as_deref())?;
        self.ledger()
            .fill_position(position_id, provider.as_deref(), OWNER)?;
        self.snapshot()
    }

    /// Let a persistent position's agent go, leaving the position vacant.
    pub fn vacate(&self, position_id: &str) -> Result<OrgSnapshot> {
        self.ledger().vacate_position(position_id, OWNER)?;
        self.snapshot()
    }

    pub fn update_position(&self, id: &str, input: &PositionPatchInput) -> Result<OrgSnapshot> {
        // An empty runtime makes the position automatic.
        let runtime = input
            .runtime_id
            .as_deref()
            .map(|r| match r.trim() {
                "" => Ok(None),
                r => self.runtime(r).map(|r| Some((r.id, Some(r.provider)))),
            })
            .transpose()?;
        let model = input.model.as_deref().map(|m| {
            let m = m.trim();
            (!m.is_empty()).then(|| m.to_owned())
        });
        self.ledger().update_position(
            id,
            &PositionPatch {
                title: input.title.clone(),
                runtime,
                model,
            },
            OWNER,
        )?;
        self.snapshot()
    }

    /// Make a position report to `reports_to` (`None`: the owner).
    pub fn move_position(&self, id: &str, reports_to: Option<&str>) -> Result<OrgSnapshot> {
        self.ledger().move_position(id, reports_to, OWNER)?;
        self.snapshot()
    }

    pub fn archive_position(&self, id: &str) -> Result<OrgSnapshot> {
        self.ledger().archive_position(id, OWNER)?;
        self.snapshot()
    }

    pub fn assign_oversight(
        &self,
        overseer_id: &str,
        target_id: &str,
        role: OversightRole,
    ) -> Result<OrgSnapshot> {
        let kind = match role {
            OversightRole::Review => OversightKind::Review,
            OversightRole::Qa => OversightKind::Qa,
            OversightRole::Security => OversightKind::Security,
        };
        self.ledger()
            .assign_oversight(kind, overseer_id, target_id, OWNER)?;
        self.snapshot()
    }

    pub fn end_oversight(&self, id: &str) -> Result<OrgSnapshot> {
        self.ledger().end_oversight(id, OWNER)?;
        self.snapshot()
    }

    // ---- Objectives ---------------------------------------------------------------------

    /// Give a staffed persistent position's agent an objective: its conversation continues (or
    /// starts), with its team named in its instructions. Workers it delegates to appear under
    /// its team's positions and leave the workforce when they finish; full-time members it
    /// hands work to do it in their own conversations (Phase 8). An agent starting a
    /// conversation gets the AI tool and model its position's role policy picks now (Phase 6);
    /// it keeps them for the whole conversation. `project_id` names the project the objective
    /// is about (for a VP or manager: one of the projects its team runs).
    pub async fn give_objective(
        &self,
        position_id: &str,
        objective: &str,
        project_id: Option<&str>,
    ) -> Result<AgentSessionDetail> {
        // Routing needs to know which AI tools are ready: finish detecting them first.
        let runtime = &self.inner.runtime;
        if runtime
            .runtimes()
            .iter()
            .any(|r| r.installation.state == InstallState::Checking)
        {
            runtime.refresh().await;
        }
        let this = self.clone();
        let (id, project) = (position_id.to_owned(), project_id.map(str::to_owned));
        let (plan, project_note) =
            tokio::task::spawn_blocking(move || this.plan_objective(&id, project.as_deref()))
                .await
                .map_err(|e| WorkforceError::Internal(e.to_string()))??;
        let objective = match project_note {
            Some(note) => format!("{}\n\n{note}", objective.trim()),
            None => objective.to_owned(),
        };
        let liaison = &self.inner.liaison;
        let detail = if plan.existing {
            liaison
                .resume_member_session(
                    &plan.session_id,
                    &objective,
                    plan.workforce,
                    plan.project_id,
                )
                .await?
        } else {
            liaison
                .start_member_session(
                    SessionStart {
                        id: Some(plan.session_id),
                        runtime_id: plan.runtime_id,
                        model: plan.model,
                        effort: plan.effort,
                        title: Some(plan.title),
                        metadata: Value::Null,
                    },
                    &objective,
                    plan.workforce,
                    plan.project_id,
                )
                .await?
        };
        Ok(detail)
    }

    /// Check that `position_id` can take an objective and find its agent's conversation — or,
    /// for a new one, the AI tool and model to start it on. With `project_id`, the objective
    /// belongs to that project, which must be run by the position's team; the second value is
    /// the line that names it for the agent.
    fn plan_objective(
        &self,
        position_id: &str,
        project_id: Option<&str>,
    ) -> Result<(ConversationPlan, Option<String>)> {
        let l = self.ledger();
        let records = l.org_records()?;
        let view = OrgView::new(&records);
        let position = view.position(position_id).ok_or_else(|| {
            WorkforceError::Ledger(plenipo_ledger::LedgerError::NotFound(format!(
                "position {position_id}"
            )))
        })?;
        let planner = self.inner.router.planner()?;
        let mut plan = conversation::plan(l, &planner, &view, position)?;
        let mut note = None;
        if let Some(project_id) = project_id {
            let project = records
                .projects
                .iter()
                .find(|p| p.id == project_id && p.status == "active")
                .ok_or_else(|| invalid("that project does not exist or has been archived"))?;
            let supervisor = project
                .coordinator_position_id
                .as_deref()
                .and_then(|c| view.active(c));
            let runs_it = supervisor
                .is_some_and(|s| s.id == position.id || view.chain_contains(&s.id, &position.id));
            if !runs_it {
                return Err(invalid(format!(
                    "{} is not run by {}'s team",
                    project.name, position.title
                )));
            }
            if plan.project_id.as_deref() != Some(project_id) {
                plan.project_id = Some(project_id.to_owned());
                note = Some(match supervisor.filter(|s| s.id != position.id) {
                    Some(s) => format!(
                        "Project: {} (its supervisor: role:{}).",
                        project.name, s.title
                    ),
                    None => format!("Project: {}.", project.name),
                });
            }
        }
        Ok((plan, note))
    }
}

/// Longest answer an objective's summary keeps (characters).
const BRIEF_ANSWER_CHARS: usize = 600;

/// An objective in a line: who has it, its state, how its tasks are going, its branch, its
/// project, and the start of its answer.
fn brief(
    l: &plenipo_ledger::Ledger,
    view: &OrgView<'_>,
    pending: &[String],
    root: Task,
    project_id: Option<&str>,
) -> Result<ObjectiveBrief> {
    let tree = l.descendant_tasks(&root.id)?;
    let all: Vec<&Task> = std::iter::once(&root)
        .chain(tree.iter().map(|(t, _)| t))
        .collect();
    let count = |f: &dyn Fn(&Task) -> bool| {
        u32::try_from(all.iter().filter(|t| f(t)).count()).unwrap_or(u32::MAX)
    };
    let project_id = project_id
        .map(str::to_owned)
        .or_else(|| all.iter().find_map(|t| t.project_id.clone()));
    let branch = match (
        root.metadata["liaison"]["correlationId"].as_str(),
        &project_id,
    ) {
        (Some(c), Some(p)) => l
            .workflow_workspaces(c)?
            .into_iter()
            .find(|w| &w.project_id == p && w.parent_id.is_none())
            .map(|w| w.branch),
        _ => None,
    };
    let answer = l
        .last_task_event(&root.id, "agent.result")?
        .and_then(|e| e.payload["text"].as_str().map(str::trim).map(str::to_owned))
        .filter(|t| !t.is_empty())
        .map(|t| {
            if t.chars().count() > BRIEF_ANSWER_CHARS {
                let cut: String = t.chars().take(BRIEF_ANSWER_CHARS).collect();
                format!("{cut}…")
            } else {
                t
            }
        });
    Ok(ObjectiveBrief {
        objective: root
            .objective
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_owned(),
        position_title: root.metadata["workforce"]["positionId"]
            .as_str()
            .and_then(|p| view.position(p))
            .map(|p| p.title.clone()),
        state: root.state,
        created_at: root.created_at,
        completed_at: root.completed_at,
        tasks: count(&|_| true),
        active: count(&|t| !t.state.is_terminal() && t.state != TaskState::Queued),
        failed: count(&|t| t.state == TaskState::Failed),
        waiting_approvals: count(&|t| pending.contains(&t.id)),
        branch,
        project_id,
        answer,
        root_task_id: root.id,
    })
}
