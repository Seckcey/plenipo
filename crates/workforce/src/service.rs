//! The Workforce service (ADR-009): the organization's snapshot and work views, the owner's
//! changes to it, and objectives for its persistent agents. Structure rules are enforced by
//! the Ledger in each change's own transaction; this service checks what only it knows —
//! which runtimes exist — and talks to Liaison and the agent runtime.

use std::sync::{Arc, Mutex, MutexGuard};

use plenipo_ledger::{
    Ledger, NewPosition, OversightKind, PositionPatch, PositionState, ProjectSettings, RoleType,
};
use plenipo_liaison::Liaison;
use plenipo_runtime::agent::{AgentRuntime, AgentRuntimeInfo, AgentSessionDetail, SessionStart};
use serde_json::{json, Value};

use crate::directory::{allowed, WorkforceDirectory};
use crate::dto::*;
use crate::error::{Result, WorkforceError};
use crate::snapshot::{self, Inputs};
use crate::templates::{default_glyph, role_templates};
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
    notices: Mutex<Vec<String>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Workforce {
    inner: Arc<Inner>,
}

impl Workforce {
    /// Create the service: seed missing role templates and install the organization's
    /// directory in Liaison, so members address their teams by role.
    pub fn new(ledger: Arc<Ledger>, runtime: AgentRuntime, liaison: Liaison) -> Self {
        let this = Self {
            inner: Arc::new(Inner {
                ledger: Arc::clone(&ledger),
                runtime: runtime.clone(),
                liaison: liaison.clone(),
                notices: Mutex::new(Vec::new()),
            }),
        };
        if let Err(e) = ledger.ensure_roles(&role_templates(), PLENIPO) {
            this.notice(format!("Could not add the built-in role templates: {e}"));
        }
        liaison.set_directory(Arc::new(WorkforceDirectory { ledger, runtime }));
        this
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
        let runtimes = self.runtimes();
        Ok(snapshot::build(&Inputs {
            records: &records,
            open_tasks: &open_tasks,
            finished_recent: &finished_recent,
            sessions: &sessions,
            runtimes: &runtimes,
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
        self.ledger().create_role(
            &name,
            description,
            role_type,
            persistent,
            &json!({ "glyph": default_glyph(role_type), "purpose": purpose }),
            OWNER,
        )?;
        self.snapshot()
    }

    fn lead(&self, input: &LeadInput, reports_to: Option<String>) -> Result<NewPosition> {
        let runtime = self.runtime(&input.runtime_id)?;
        Ok(NewPosition {
            title: input.title.clone(),
            role_id: input.role_id.clone(),
            reports_to,
            runtime_id: runtime.id,
            runtime_provider: Some(runtime.provider),
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
        let runtime = self.runtime(&input.runtime_id)?;
        self.ledger().create_position(
            &NewPosition {
                title: input.title.clone(),
                role_id: input.role_id.clone(),
                reports_to: input.reports_to.clone(),
                runtime_id: runtime.id,
                runtime_provider: Some(runtime.provider),
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
        let runtime = self.runtime(&position.runtime_id)?;
        self.ledger()
            .fill_position(position_id, Some(&runtime.provider), OWNER)?;
        self.snapshot()
    }

    /// Let a persistent position's agent go, leaving the position vacant.
    pub fn vacate(&self, position_id: &str) -> Result<OrgSnapshot> {
        self.ledger().vacate_position(position_id, OWNER)?;
        self.snapshot()
    }

    pub fn update_position(&self, id: &str, input: &PositionPatchInput) -> Result<OrgSnapshot> {
        let runtime = input
            .runtime_id
            .as_deref()
            .map(|r| self.runtime(r))
            .transpose()?;
        let model = input.model.as_deref().map(|m| {
            let m = m.trim();
            (!m.is_empty()).then(|| m.to_owned())
        });
        self.ledger().update_position(
            id,
            &PositionPatch {
                title: input.title.clone(),
                runtime: runtime.map(|r| (r.id, Some(r.provider))),
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

    /// Give a staffed persistent position's agent an objective: its session continues (or
    /// starts), with its team named in its instructions. Workers it delegates to appear under
    /// its team's positions and leave the workforce when they finish.
    pub async fn give_objective(
        &self,
        position_id: &str,
        objective: &str,
    ) -> Result<AgentSessionDetail> {
        let this = self.clone();
        let id = position_id.to_owned();
        let plan = tokio::task::spawn_blocking(move || this.plan_objective(&id))
            .await
            .map_err(|e| WorkforceError::Internal(e.to_string()))??;
        let liaison = &self.inner.liaison;
        let detail = match plan.session_id {
            Some(session_id) => {
                liaison
                    .resume_member_session(&session_id, objective, plan.workforce, plan.project_id)
                    .await?
            }
            None => {
                liaison
                    .start_member_session(
                        SessionStart {
                            id: None,
                            runtime_id: plan.runtime_id,
                            model: plan.model,
                            title: Some(plan.title),
                            metadata: Value::Null,
                        },
                        objective,
                        plan.workforce,
                        plan.project_id,
                    )
                    .await?
            }
        };
        Ok(detail)
    }

    /// Check that `position_id` can take an objective and find its agent's session.
    fn plan_objective(&self, position_id: &str) -> Result<ObjectivePlan> {
        let l = self.ledger();
        let records = l.org_records()?;
        let view = OrgView::new(&records);
        let position = view.position(position_id).ok_or_else(|| {
            WorkforceError::Ledger(plenipo_ledger::LedgerError::NotFound(format!(
                "position {position_id}"
            )))
        })?;
        if position.state != PositionState::Active {
            return Err(invalid(format!("{} has been archived", position.title)));
        }
        if !view.persistent(position) {
            return Err(invalid(format!(
                "{} is an on-call position: it takes tasks handed to it by its team's lead",
                position.title
            )));
        }
        let agent = l.position_incumbent(position_id)?.ok_or_else(|| {
            invalid(format!(
                "{} is vacant; hire an agent into it first",
                position.title
            ))
        })?;
        let runtime_id = agent
            .runtime_id
            .clone()
            .unwrap_or_else(|| position.runtime_id.clone());
        let project = view.project_of(position_id);
        if !allowed(project, &runtime_id) {
            return Err(invalid(format!(
                "{} does not allow the {runtime_id} runtime; change the project's allowed \
                 runtimes or the position's runtime",
                project.map_or("The project", |p| p.name.as_str())
            )));
        }
        let session_id = l
            .agent_sessions(&agent.id)?
            .into_iter()
            .find(|s| {
                s.state == plenipo_ledger::RuntimeSessionState::Open && s.runtime == runtime_id
            })
            .map(|s| s.id);
        let project_id = project.map(|p| p.id.clone());
        Ok(ObjectivePlan {
            session_id,
            runtime_id,
            model: agent.model.clone(),
            title: position.title.clone(),
            workforce: json!({
                "positionId": position_id,
                "agentId": agent.id,
                "projectId": project_id,
            }),
            project_id,
        })
    }
}

struct ObjectivePlan {
    session_id: Option<String>,
    runtime_id: String,
    model: Option<String>,
    title: String,
    workforce: Value,
    project_id: Option<String>,
}
