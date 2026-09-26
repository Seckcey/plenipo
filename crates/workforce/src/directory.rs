//! The organization's directory for Liaison (ADR-009 §5): a member's team, and where its
//! `role:<name>` requests go.

use std::sync::Arc;

use plenipo_ledger::{Ledger, NewWorker, Project, Task};
use plenipo_liaison::context::Destination;
use plenipo_liaison::{Directory, Placement, Team};
use plenipo_runtime::agent::{AgentRuntime, AgentRuntimeInfo};
use serde_json::{json, Value};

use crate::prompt::{member_identity, member_label, runtime_label, worker_identity};
use crate::service::org_name;
use crate::view::{OrgView, TeamMember};

pub struct WorkforceDirectory {
    pub(crate) ledger: Arc<Ledger>,
    pub(crate) runtime: AgentRuntime,
}

/// The project allows `runtime_id` (a position outside any project may use any runtime).
pub(crate) fn allowed(project: Option<&Project>, runtime_id: &str) -> bool {
    project.is_none_or(|p| p.allowed_runtimes.iter().any(|r| r == runtime_id))
}

fn ready(runtimes: &[AgentRuntimeInfo], runtime_id: &str) -> bool {
    runtimes.iter().any(|r| r.id == runtime_id && r.ready)
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
        let runtimes = self.runtime.runtimes();
        let lead = view.lead_of(position_id);
        let members = lead.map(|l| view.team(&l.id)).unwrap_or_default();
        // Work for a team belongs to the team's project, whose runtimes then apply.
        let project = lead.and_then(|l| view.project_of(&l.id));
        let destinations: Vec<Destination> = members
            .iter()
            .map(|m| Destination {
                address: format!("role:{}", m.position.title),
                label: member_label(&view, &runtimes, m),
                ready: ready(&runtimes, &m.position.runtime_id)
                    && allowed(project, &m.position.runtime_id),
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

    fn place(&self, workforce: &Value, _requester: &Task, name: &str) -> Result<Placement, String> {
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
        let runtimes = self.runtime.runtimes();
        let info = runtimes
            .iter()
            .find(|r| r.id == target.runtime_id)
            .ok_or_else(|| {
                format!(
                    "{}'s runtime ({}) is not available in this build",
                    target.title, target.runtime_id
                )
            })?;
        // The work is the lead's team's: its project, and that project's runtimes, apply
        // (also to an overseer from outside the project).
        let project = view.project_of(&lead.id);
        if !allowed(project, &target.runtime_id) {
            return Err(format!(
                "{} does not allow {} workers, so {} cannot take work; the owner can change the \
                 project's allowed runtimes or the position's runtime",
                project.map_or("the project", |p| p.name.as_str()),
                info.label,
                target.title
            ));
        }
        let agent_id = uuid::Uuid::new_v4().to_string();
        let project_id = project.map(|p| p.id.clone());
        Ok(Placement {
            address: format!("role:{}", target.title),
            label: format!(
                "{} ({})",
                target.title,
                runtime_label(&runtimes, &target.runtime_id)
            ),
            runtime_id: target.runtime_id.clone(),
            model: target.model.clone(),
            worker: NewWorker {
                agent_id: agent_id.clone(),
                position_id: target.id.clone(),
                role_id: target.role_id.clone(),
                runtime_id: target.runtime_id.clone(),
                runtime_provider: Some(info.provider.clone()),
                model: target.model.clone(),
                project_id: project_id.clone(),
            },
            workforce: json!({
                "positionId": target.id,
                "agentId": agent_id,
                "projectId": project_id,
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
}
