//! The organization's directory for Liaison (ADR-009 §5): a member's team, and where its
//! `role:<name>` requests go. An automatic position's worker gets the AI tool and model its
//! role's model policy picks at that moment (Phase 6, ADR-011); a fixed position's worker gets
//! the ones the owner set.

use std::sync::Arc;

use plenipo_ledger::{ChildConversation, Ledger, NewWorker, Position, Project, Task};
use plenipo_liaison::context::Destination;
use plenipo_liaison::protocol::PROTOCOL;
use plenipo_liaison::{Directory, MemberConversation, Placement, Team};
use plenipo_router::{Planner, RouteDecision, RouteRequest, Router};
use plenipo_runtime::agent::{SessionStart, WorkDoneBy};
use serde_json::{json, Value};

use crate::conversation;
use crate::prompt::{member_identity, member_label, member_reminder, worker_identity};
use crate::service::org_name;
use crate::view::{OrgView, TeamMember};

pub struct WorkforceDirectory {
    ledger: Arc<Ledger>,
    router: Router,
    /// The PC's Free or Pro: lessons are part of Pro (ADR-112).
    entitlements: crate::service::EntitlementsCell,
}

impl WorkforceDirectory {
    pub fn new(ledger: Arc<Ledger>, router: Router) -> Self {
        Self::with_entitlements(ledger, router, crate::service::EntitlementsCell::default())
    }

    pub fn with_entitlements(
        ledger: Arc<Ledger>,
        router: Router,
        entitlements: crate::service::EntitlementsCell,
    ) -> Self {
        Self {
            ledger,
            router,
            entitlements,
        }
    }

    /// What the position's role has learned, unless lessons are off for this copy (Free) or
    /// for it.
    fn learned(&self, view: &OrgView<'_>, p: &Position, project_id: Option<&str>) -> String {
        if !self
            .entitlements
            .get()
            .check(plenipo_licensing::Limit::Lessons)
            .is_allowed()
        {
            return String::new();
        }
        learned(&self.ledger, view, p, project_id)
    }

    /// Nobody in `lead`'s team or department does the job `name` (Phase 25, item 2.7). With the
    /// owner's switch on, an on-call worker for that role is hired onto `lead`'s team (`Ok`: the
    /// caller places the work again). Otherwise Plenipo asks the owner, once a day for the same
    /// lead and job, on Home's What's stuck, and says so.
    fn missing_job(
        &self,
        view: &OrgView<'_>,
        lead: &Position,
        requester: &Task,
        name: &str,
        why: String,
    ) -> Result<(), String> {
        let wanted = name.trim();
        let role = view.records.roles.iter().find(|r| {
            r.name.eq_ignore_ascii_case(wanted)
                && r.role_type == plenipo_ledger::RoleType::Worker
                && !r.persistent
        });
        let Some(role) = role else {
            return Err(format!(
                "{why}. Nobody in your department does this job either: ask the owner to hire \
                 one, or do this part yourself"
            ));
        };
        if hire_on_its_own(&self.ledger) {
            let new = plenipo_ledger::NewPosition {
                title: role.name.clone(),
                role_id: role.id.clone(),
                reports_to: Some(lead.id.clone()),
                runtime_id: None,
                runtime_provider: None,
                model: None,
                staffed: true,
                specialty_id: None,
                from_workforce: None,
            };
            return self
                .ledger
                .create_position(&new, "plenipo")
                .map(|_| ())
                .map_err(|e| format!("Plenipo could not hire a {}: {e}", role.name));
        }
        const DAY_MS: u64 = 24 * 3_600_000;
        let since = plenipo_ledger::now_ms().saturating_sub(DAY_MS);
        let asked = self
            .ledger
            .events_of_types(&["org.hire_needed"], 200)
            .unwrap_or_default()
            .into_iter()
            .any(|e| {
                e.created_at >= since
                    && e.payload["leadId"] == lead.id.as_str()
                    && e.payload["roleId"] == role.id.as_str()
            });
        if !asked {
            let _ = self.ledger.append_event(plenipo_ledger::NewEvent {
                task_id: Some(requester.id.clone()),
                source: format!("position:{}", lead.id),
                event_type: "org.hire_needed".into(),
                payload: json!({
                    "leadId": lead.id,
                    "lead": lead.title,
                    "roleId": role.id,
                    "role": role.name,
                    "projectId": view.project_of(&lead.id).map(|p| p.id.clone()),
                }),
                ..plenipo_ledger::NewEvent::default()
            });
        }
        Err(format!(
            "{why}. Nobody in your department is a {} either: Plenipo asked the owner whether to \
             hire one; meanwhile do this part yourself, or skip it and say so",
            role.name
        ))
    }

    /// A request to a full-time member (a VP's Supervisor, a manager's Supervisor): the member
    /// does it in its own conversation, which keeps its memory, and no worker is brought in
    /// (ADR-016). The member works on its own project, whose AI tools apply.
    fn place_member(
        &self,
        view: &OrgView<'_>,
        planner: &Planner,
        target: &Position,
    ) -> Result<Placement, String> {
        let plan = conversation::plan(&self.ledger, planner, view, target).map_err(|e| {
            format!("{e}. The owner can change this in Plenipo; do this part yourself meanwhile")
        })?;
        let label = planner
            .tool(&plan.runtime_id)
            .map_or_else(|| plan.runtime_id.clone(), |t| t.info.label.clone());
        let mut workforce = plan.workforce.clone();
        workforce["fullTime"] = json!(true);
        let has_team = !view.team(&target.id).is_empty();
        Ok(Placement {
            address: format!("role:{}", target.title),
            label: format!("{} ({label})", target.title),
            runtime_id: plan.runtime_id.clone(),
            model: plan.model.clone(),
            effort: plan.effort,
            worker: None,
            conversation: Some(ChildConversation {
                session_id: plan.session_id.clone(),
                runtime_id: plan.runtime_id.clone(),
                model: plan.model.clone(),
                effort: plan.effort.map(|e| e.as_str().to_owned()),
            }),
            workforce,
            identity: member_identity(view, &org_name(&self.ledger), target, has_team)
                + &self.learned(view, target, plan.project_id.as_deref()),
            project_id: plan.project_id,
        })
    }
}

/// Whether the owner let leads hire missing workers on their own (Settings → Switches, Phase 25,
/// item 2.7). Read from Guard's saved settings; off when they can't be read.
fn hire_on_its_own(ledger: &Ledger) -> bool {
    ledger
        .setting("guard")
        .ok()
        .flatten()
        .is_some_and(|g| g["switches"]["hireOnItsOwn"] == true)
}

/// What the position's role has learned, for a worker on `project_id`, and how to write down a
/// lesson (ADR-024; ADR-050: notes in a fence, from this project or from none), unless learning
/// is off for it (ADR-041).
fn learned(ledger: &Ledger, view: &OrgView<'_>, p: &Position, project_id: Option<&str>) -> String {
    view.role(p).map_or_else(String::new, |r| {
        crate::learning::instructions(ledger, p, &r.name, project_id)
    })
}

/// The project allows `runtime_id` (a position outside any project may use any runtime).
pub(crate) fn allowed(project: Option<&Project>, runtime_id: &str) -> bool {
    project.is_none_or(|p| p.allowed_runtimes.iter().any(|r| r == runtime_id))
}

/// What a decision about `position` depends on: its role, its own rule, its department's rule
/// (ADR-041), `project`'s AI tools, and `reviewed`, the work it would review (its AI tools and
/// models, ADR-081).
pub(crate) fn request<'a>(
    view: &OrgView<'a>,
    position: &'a Position,
    project: Option<&'a Project>,
    reviewed: &'a [WorkDoneBy],
) -> RouteRequest<'a> {
    RouteRequest {
        role_id: &position.role_id,
        position_id: Some(&position.id),
        // The department it works in: the team it is lent to, or its own (ADR-041 §5, ADR-054).
        department: view
            .work_department_of(&position.id)
            .map(|d| (d.id.as_str(), d.name.as_str())),
        project: project.map(|p| (p.name.as_str(), p.allowed_runtimes.as_slice())),
        reviewed,
    }
}

/// Where a new worker of `position` would go now, and why: the owner's fixed choice, or the
/// closest rule's models within `project`'s AI tools (ADR-041).
pub(crate) fn decide(
    planner: &Planner,
    view: &OrgView<'_>,
    position: &Position,
    project: Option<&Project>,
    reviewed: &[WorkDoneBy],
) -> RouteDecision {
    let request = request(view, position, project, reviewed);
    match &position.runtime_id {
        Some(runtime) => planner.fixed(
            &request,
            &position.title,
            runtime,
            position.model.as_deref(),
        ),
        None => planner.route(&request),
    }
}

/// Whether the routed model is known to see images, from who made it (Phase 25, item 2.4).
fn sees_images(maker: Option<&str>, model: Option<&str>) -> bool {
    plenipo_router::makers::sees_images(maker, model) == Some(true)
}

/// Why `name` is not on `lead`'s team when it is one of the team's agents lent to another team
/// (ADR-054).
fn away(view: &OrgView<'_>, lead: &str, name: &str) -> Option<String> {
    let wanted = name.trim().to_lowercase();
    view.reports(Some(lead)).into_iter().find_map(|p| {
        let named = p.title.to_lowercase() == wanted
            || view
                .role(p)
                .is_some_and(|r| r.name.to_lowercase() == wanted);
        let loan = view.loan(&p.id).filter(|_| named)?;
        let to = view
            .position(&loan.to_lead_id)
            .map_or("another team", |l| l.title.as_str());
        let until = match loan.until {
            plenipo_ledger::LoanUntil::Objective => "until its objective there is done",
            plenipo_ledger::LoanUntil::Returned => "until the owner sends it home",
        };
        Some(format!(
            "{} is lent to {to}'s team {until}; do this part yourself",
            p.title
        ))
    })
}

/// The team member `name` refers to: by title, or by role name when that is unambiguous.
fn find<'a>(
    view: &OrgView<'_>,
    team: &'a [TeamMember<'a>],
    name: &str,
) -> Result<&'a TeamMember<'a>, String> {
    let wanted = name.trim().to_lowercase();
    let listed = || {
        team.iter()
            .map(|m| format!("role:{}", m.position.title))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let by_title: Vec<&TeamMember<'_>> = team
        .iter()
        .filter(|m| m.position.title.to_lowercase() == wanted)
        .collect();
    if let [m] = by_title.as_slice() {
        return Ok(m);
    }
    let by_role: Vec<&TeamMember<'_>> = team
        .iter()
        .filter(|m| {
            view.role(m.position)
                .is_some_and(|r| r.name.to_lowercase() == wanted)
        })
        .collect();
    match by_role.as_slice() {
        [m] => Ok(m),
        [] if team.is_empty() => Err(format!(
            "\"{}\" is not on your team: no one is yet, so do this part yourself (the owner can \
             hire team members in Plenipo)",
            name.trim()
        )),
        [] => Err(format!(
            "\"{}\" is not on your team; address one of: {}",
            name.trim(),
            listed()
        )),
        many => Err(format!(
            "several members of your team have the role \"{}\" ({}); address one by its title",
            name.trim(),
            many.iter()
                .map(|m| format!("role:{}", m.position.title))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

impl Directory for WorkforceDirectory {
    fn team(&self, workforce: &Value) -> Option<Team> {
        let position_id = workforce["positionId"].as_str()?;
        let records = self.ledger.org_records().ok()?;
        let view = OrgView::new(&records);
        let me = view.position(position_id)?;
        let planner = self.router.planner().ok()?;
        let lead = view.lead_of(position_id);
        let mut members = lead.map(|l| view.team(&l.id)).unwrap_or_default();
        // The department's other workers come after the team (Phase 25, item 2.7).
        let shared = members.len();
        if let Some(l) = lead.filter(|_| view.persistent(me)) {
            members.extend(view.department_pool(&l.id));
        }
        // Work for a team belongs to the team's project, whose runtimes then apply.
        let project = lead.and_then(|l| view.project_of(&l.id));
        let destinations: Vec<Destination> = members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let from_department = i >= shared;
                if view.persistent(m.position) {
                    // A full-time member: ready when it is staffed and its conversation can
                    // take work (it may have to finish a current task first).
                    let ready = conversation::plan(&self.ledger, &planner, &view, m.position)
                        .map(|c| planner.unavailable(&c.runtime_id).is_none())
                        .unwrap_or(false);
                    return Destination {
                        address: format!("role:{}", m.position.title),
                        label: member_label(&view, None, m),
                        ready,
                    };
                }
                let decision = decide(&planner, &view, m.position, project, &[]);
                let ready = decision.choice.as_ref().is_some_and(|c| {
                    allowed(project, &c.runtime_id) && planner.unavailable(&c.runtime_id).is_none()
                });
                let tool = decision
                    .choice
                    .as_ref()
                    .filter(|_| decision.fixed)
                    .map(|c| c.runtime_label.as_str());
                let label = member_label(&view, tool, m);
                Destination {
                    address: format!("role:{}", m.position.title),
                    label: if from_department {
                        format!("{label} — from your department, when your team has no one for it")
                    } else {
                        label
                    },
                    ready,
                }
            })
            .collect();
        let name = org_name(&self.ledger);
        let identity = if view.persistent(me) {
            member_identity(&view, &name, me, destinations.iter().any(|d| d.ready))
        } else {
            let choice = &workforce["routing"]["choice"];
            let sees = sees_images(choice["maker"]["id"].as_str(), choice["model"].as_str());
            worker_identity(&view, &name, me, None, sees)
        } + &self.learned(&view, me, workforce["projectId"].as_str());
        Some(Team {
            identity,
            reminder: view
                .persistent(me)
                .then(|| member_reminder(&view, &name, me)),
            members: destinations,
        })
    }

    fn place(
        &self,
        workforce: &Value,
        requester: &Task,
        name: &str,
        reviewed: &[WorkDoneBy],
    ) -> Result<Placement, String> {
        let position_id = workforce["positionId"]
            .as_str()
            .ok_or_else(|| "this worker is not part of the organization".to_owned())?;
        let records = self.ledger.org_records().map_err(|e| e.to_string())?;
        let view = OrgView::new(&records);
        let me = view
            .position(position_id)
            .ok_or_else(|| "this worker is no longer part of the organization".to_owned())?;
        let lead = view
            .lead_of(position_id)
            .ok_or_else(|| format!("{} has no team to hand work to", me.title))?;
        let team = view.team(&lead.id);
        // The department's other workers, after the team (Phase 25, item 2.7).
        let pool = view.department_pool(&lead.id);
        let answers = |members: &[TeamMember<'_>]| {
            let wanted = name.trim().to_lowercase();
            members.iter().any(|m| {
                m.position.title.to_lowercase() == wanted
                    || view
                        .role(m.position)
                        .is_some_and(|r| r.name.to_lowercase() == wanted)
            })
        };
        let from_pool = !answers(&team) && answers(&pool);
        let searched = if from_pool { &pool } else { &team };
        let member = match find(&view, searched, name) {
            Ok(m) => m,
            // Lent away is the reason only when nobody on the team answers to that name (else
            // `find`'s own reason, such as two members with that role, is the useful one).
            Err(e) if answers(&team) || answers(&pool) => return Err(e),
            Err(e) => {
                if let Some(why) = away(&view, &lead.id, name) {
                    return Err(why);
                }
                // Nobody in the department does the job: hire one when the owner let leads
                // do that, else ask the owner (Phase 25, item 2.7).
                self.missing_job(&view, lead, requester, name, e)?;
                return self.place(workforce, requester, name, reviewed);
            }
        };
        let target = member.position;
        if let Some(loan) = member.lent {
            // Lent for one objective: only that objective's work (ADR-054).
            if let Some(joined) = loan.objective_task_id.as_deref() {
                let objective = self
                    .ledger
                    .objective_of(&requester.id)
                    .map_err(|e| e.to_string())?;
                if objective != joined {
                    return Err(format!(
                        "{} is lent to your team for another objective and goes home when that one is \
                         done; do this part yourself",
                        target.title
                    ));
                }
            }
            if loan.going_home {
                return Err(format!(
                    "{} is going home after its current task; do this part yourself",
                    target.title
                ));
            }
        }
        let planner = self.router.planner().map_err(|e| e.to_string())?;
        if view.persistent(target) {
            return self.place_member(&view, &planner, target);
        }
        // The work is the lead's team's: its project, and that project's runtimes, apply
        // (also to an overseer from outside the project).
        let project = view.project_of(&lead.id);
        let decision = decide(&planner, &view, target, project, reviewed);
        let Some(choice) = decision.choice.clone() else {
            return Err(format!(
                "{} cannot take work now: {} The owner can change this in Plenipo's settings",
                target.title, decision.reason
            ));
        };
        let tool = planner.tool(&choice.runtime_id).ok_or_else(|| {
            format!(
                "{}'s AI tool ({}) is not available in this version of Plenipo",
                target.title, choice.runtime_id
            )
        })?;
        if !allowed(project, &choice.runtime_id) {
            return Err(format!(
                "{} does not allow {} workers, so {} cannot take work; the owner can change the \
                 project's allowed AI tools or the position's AI tool",
                project.map_or("the project", |p| p.name.as_str()),
                tool.info.label,
                target.title
            ));
        }
        let agent_id = uuid::Uuid::new_v4().to_string();
        let project_id = project.map(|p| p.id.clone());
        // The team's department decides the department permission limit (ADR-054 §9).
        let department_id = view.department_of(&lead.id).map(|d| d.id.clone());
        let routing = json!(decision);
        Ok(Placement {
            address: format!("role:{}", target.title),
            label: format!("{} ({})", target.title, choice.label),
            runtime_id: choice.runtime_id.clone(),
            model: choice.model.clone(),
            effort: choice.effort,
            conversation: None,
            worker: Some(NewWorker {
                agent_id: agent_id.clone(),
                position_id: target.id.clone(),
                role_id: target.role_id.clone(),
                runtime_id: choice.runtime_id.clone(),
                runtime_provider: Some(tool.info.provider.clone()),
                model: choice.model.clone(),
                project_id: project_id.clone(),
                routing: routing.clone(),
            }),
            workforce: json!({
                "positionId": target.id,
                "agentId": agent_id,
                "projectId": project_id,
                "departmentId": department_id,
                "leadId": lead.id,
                "routing": routing,
            }),
            identity: worker_identity(
                &view,
                &org_name(&self.ledger),
                target,
                member.oversight.map(|o| (lead, o.kind)),
                sees_images(
                    choice.maker.as_ref().map(|m| m.id.as_str()),
                    choice.model.as_deref(),
                ),
            ) + &self.learned(&view, target, project_id.as_deref()),
            project_id,
        })
    }

    fn conversation(&self, workforce: &Value) -> Result<MemberConversation, String> {
        let (Some(position_id), Some(agent_id)) = (
            workforce["positionId"].as_str(),
            workforce["agentId"].as_str(),
        ) else {
            return Err("this task does not name a member of the organization".into());
        };
        let records = self.ledger.org_records().map_err(|e| e.to_string())?;
        let view = OrgView::new(&records);
        let position = view
            .position(position_id)
            .ok_or_else(|| "that position is no longer part of the organization".to_owned())?;
        let planner = self.router.planner().map_err(|e| e.to_string())?;
        let plan = conversation::plan(&self.ledger, &planner, &view, position)
            .map_err(|e| e.to_string())?;
        if plan.workforce["agentId"].as_str() != Some(agent_id) {
            return Err(format!(
                "{} has a new agent since this task was handed over; hand it over again",
                position.title
            ));
        }
        let conversation = ChildConversation {
            session_id: plan.session_id.clone(),
            runtime_id: plan.runtime_id.clone(),
            model: plan.model.clone(),
            effort: plan.effort.map(|e| e.as_str().to_owned()),
        };
        let start = (!plan.existing).then(|| SessionStart {
            id: Some(plan.session_id.clone()),
            runtime_id: plan.runtime_id.clone(),
            model: plan.model.clone(),
            effort: plan.effort,
            title: Some(plan.title.clone()),
            metadata: json!({
                "liaison": { "enabled": true, "origin": "member", "protocol": PROTOCOL },
                "workforce": plan.workforce,
            }),
        });
        Ok(MemberConversation {
            conversation,
            start,
        })
    }
}
