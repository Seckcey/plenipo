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
use plenipo_runtime::agent::SessionStart;
use serde_json::{json, Value};

use crate::conversation;
use crate::prompt::{member_identity, member_label, worker_identity};
use crate::service::org_name;
use crate::view::{OrgView, TeamMember};

pub struct WorkforceDirectory {
    ledger: Arc<Ledger>,
    router: Router,
}

impl WorkforceDirectory {
    pub fn new(ledger: Arc<Ledger>, router: Router) -> Self {
        Self { ledger, router }
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
            identity: member_identity(view, &org_name(&self.ledger), target, has_team),
            project_id: plan.project_id,
        })
    }
}

/// The project allows `runtime_id` (a position outside any project may use any runtime).
pub(crate) fn allowed(project: Option<&Project>, runtime_id: &str) -> bool {
    project.is_none_or(|p| p.allowed_runtimes.iter().any(|r| r == runtime_id))
}

/// Where a new worker of `position` would go now, and why: the owner's fixed choice, or its
/// role's model policy within `project`'s AI tools. `reviewed`: the runtimes whose work it
/// would review.
pub(crate) fn decide(
    planner: &Planner,
    position: &Position,
    project: Option<&Project>,
    reviewed: &[String],
) -> RouteDecision {
    match &position.runtime_id {
        Some(runtime) => planner.fixed(&position.title, runtime, position.model.as_deref()),
        None => planner.route(&RouteRequest {
            role_id: &position.role_id,
            project: project.map(|p| (p.name.as_str(), p.allowed_runtimes.as_slice())),
            reviewed,
        }),
    }
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
        let members = lead.map(|l| view.team(&l.id)).unwrap_or_default();
        // Work for a team belongs to the team's project, whose runtimes then apply.
        let project = lead.and_then(|l| view.project_of(&l.id));
        let destinations: Vec<Destination> = members
            .iter()
            .map(|m| {
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
                let decision = decide(&planner, m.position, project, &[]);
                let ready = decision.choice.as_ref().is_some_and(|c| {
                    allowed(project, &c.runtime_id) && planner.unavailable(&c.runtime_id).is_none()
                });
                let tool = decision
                    .choice
                    .as_ref()
                    .filter(|_| decision.fixed)
                    .map(|c| c.runtime_label.as_str());
                Destination {
                    address: format!("role:{}", m.position.title),
                    label: member_label(&view, tool, m),
                    ready,
                }
            })
            .collect();
        let name = org_name(&self.ledger);
        let identity = if view.persistent(me) {
            member_identity(&view, &name, me, destinations.iter().any(|d| d.ready))
        } else {
            worker_identity(&view, &name, me, None)
        };
        Some(Team {
            identity,
            members: destinations,
        })
    }

    fn place(
        &self,
        workforce: &Value,
        _requester: &Task,
        name: &str,
        reviewed: &[String],
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
        let member = find(&view, &team, name)?;
        let target = member.position;
        let planner = self.router.planner().map_err(|e| e.to_string())?;
        if view.persistent(target) {
            return self.place_member(&view, &planner, target);
        }
        // The work is the lead's team's: its project, and that project's runtimes, apply
        // (also to an overseer from outside the project).
        let project = view.project_of(&lead.id);
        let decision = decide(&planner, target, project, reviewed);
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
                "routing": routing,
            }),
            identity: worker_identity(
                &view,
                &org_name(&self.ledger),
                target,
                member.oversight.map(|o| (lead, o.kind)),
            ),
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
