//! A full-time member's conversation (ADR-009 §2, ADR-016): the one runtime session its agent
//! keeps across objectives, whether the owner gives the objective or the member's lead hands it
//! over. It continues on the AI tool it started on; a new one starts on the owner's fixed choice
//! or on what the role's model policy picks now (Phase 6).

use plenipo_ledger::{
    AgentInstance, AgentRoute, Ledger, NewWorker, Position, PositionState, RuntimeSessionState,
};
use plenipo_router::Planner;
use plenipo_runtime::agent::Effort;
use serde_json::{json, Value};

use crate::directory::{allowed, decide};
use crate::error::{Result, WorkforceError};
use crate::service::PLENIPO;
use crate::view::OrgView;

/// Where a full-time member's next task runs.
#[derive(Debug, Clone)]
pub(crate) struct ConversationPlan {
    /// The open conversation's session ID, or the ID to start a new one with.
    pub session_id: String,
    /// The conversation is open already (resume it); otherwise start it.
    pub existing: bool,
    pub runtime_id: String,
    pub model: Option<String>,
    /// For a new conversation (an open one keeps its own).
    pub effort: Option<Effort>,
    /// The member's record for its session and tasks (`positionId`, `agentId`, `projectId`,
    /// and `routing` for a new conversation).
    pub workforce: Value,
    pub project_id: Option<String>,
    pub title: String,
    /// For an on-call position's direct chat (ADR-208): the worker staffed for this message,
    /// recorded with its task.
    pub worker: Option<NewWorker>,
}

fn invalid(message: impl Into<String>) -> WorkforceError {
    WorkforceError::Invalid(message.into())
}

/// The session ID for a member's new conversation: its agent's ID for the first one (so two
/// callers starting it at the same moment start the same one; the second is refused as busy),
/// a fresh ID after that.
fn new_session_id(ledger: &Ledger, agent: &AgentInstance) -> Result<String> {
    Ok(if ledger.runtime_session(&agent.id)?.is_none() {
        agent.id.clone()
    } else {
        uuid::Uuid::new_v4().to_string()
    })
}

/// Check that `position` can take a task now and find its conversation.
pub(crate) fn plan(
    ledger: &Ledger,
    planner: &Planner,
    view: &OrgView<'_>,
    position: &Position,
) -> Result<ConversationPlan> {
    if position.state != PositionState::Active {
        return Err(invalid(format!("{} has been archived", position.title)));
    }
    if !view.persistent(position) {
        return Err(invalid(format!(
            "{} is an on-call position: it takes tasks handed to it by its team's lead",
            position.title
        )));
    }
    let agent = ledger.position_incumbent(&position.id)?.ok_or_else(|| {
        invalid(format!(
            "{} is vacant; hire an agent into it first",
            position.title
        ))
    })?;
    let project = view.project_of(&position.id);
    let project_id = project.map(|p| p.id.clone());
    // Each task carries its team's project and department, which Guard reads (ADR-054 §9).
    let mut workforce = json!({
        "positionId": position.id,
        "agentId": agent.id,
        "projectId": project_id,
        "departmentId": view.department_of(&position.id).map(|d| d.id.clone()),
    });
    let not_allowed = |runtime_id: &str| {
        invalid(format!(
            "{} does not allow the {runtime_id} AI tool; change the project's allowed AI tools or \
             the position's AI tool",
            project.map_or("The project", |p| p.name.as_str())
        ))
    };
    // Its conversation continues on the AI tool it started on.
    let open = match agent.runtime_id.as_deref() {
        Some(runtime_id) => ledger
            .agent_sessions(&agent.id)?
            .into_iter()
            .find(|s| s.state == RuntimeSessionState::Open && s.runtime == runtime_id),
        None => None,
    };
    if let Some(session) = open {
        if !allowed(project, &session.runtime) {
            return Err(not_allowed(&session.runtime));
        }
        if let Some((t, limit)) = planner
            .tool(&session.runtime)
            .and_then(|t| t.limit.as_ref().map(|l| (t, l)))
        {
            return Err(invalid(format!(
                "{}'s conversation is on {}, which {}. Give the objective again then, or hire a \
                 new agent into the position to use another model",
                position.title,
                t.info.label,
                plenipo_router::engine::limit_words(limit, planner.now)
            )));
        }
        return Ok(ConversationPlan {
            session_id: session.id,
            existing: true,
            runtime_id: session.runtime,
            model: session.model.or_else(|| agent.model.clone()),
            effort: None,
            workforce,
            project_id,
            title: position.title.clone(),
            worker: None,
        });
    }
    // A new conversation: the owner's fixed choice, or what the role's policy picks now.
    let decision = decide(planner, view, position, project, &[]);
    let Some(choice) = decision.choice.clone() else {
        return Err(invalid(format!(
            "{} cannot start: {}",
            position.title, decision.reason
        )));
    };
    if !allowed(project, &choice.runtime_id) {
        return Err(not_allowed(&choice.runtime_id));
    }
    let routing = json!(decision);
    let routed = agent.runtime_id.as_deref() == Some(choice.runtime_id.as_str())
        && agent.model == choice.model;
    if position.runtime_id.is_none() && !routed {
        ledger.route_agent(
            &agent.id,
            &AgentRoute {
                runtime_id: choice.runtime_id.clone(),
                runtime_provider: Some(choice.company.clone()).filter(|c| !c.is_empty()),
                model: choice.model.clone(),
                routing: routing.clone(),
            },
            PLENIPO,
        )?;
    }
    workforce["routing"] = routing;
    Ok(ConversationPlan {
        session_id: new_session_id(ledger, &agent)?,
        existing: false,
        runtime_id: choice.runtime_id,
        model: choice.model,
        effort: choice.effort,
        workforce,
        project_id,
        title: position.title.clone(),
        worker: None,
    })
}

/// The owner's direct chat with an on-call position (ADR-208): its one open conversation, which
/// continues on the AI tool it started on (the same runtime session resumes, so it remembers the
/// earlier messages), or a new one on what the role's policy picks now; and a worker staffed for
/// this message, which leaves when its answer is done, as one staffed for a hand-off does.
pub(crate) fn direct_plan(
    ledger: &Ledger,
    planner: &Planner,
    view: &OrgView<'_>,
    position: &Position,
) -> Result<ConversationPlan> {
    if position.state != PositionState::Active {
        return Err(invalid(format!("{} has been archived", position.title)));
    }
    // The team it works for: the one it is lent to (ADR-054), as for a hand-off. Its project and
    // department are what Guard, the allowed AI tools, and paid keys' limits read.
    let project = view.work_project_of(&position.id);
    let project_id = project.map(|p| p.id.clone());
    let not_allowed = |runtime_id: &str| {
        invalid(format!(
            "{} does not allow the {runtime_id} AI tool; change the project's allowed AI tools or \
             the position's AI tool",
            project.map_or("The project", |p| p.name.as_str())
        ))
    };
    let (session_id, existing, runtime_id, model, effort, routing) =
        match ledger.open_direct_session(&position.id)? {
            Some(session) => {
                if !allowed(project, &session.runtime) {
                    return Err(not_allowed(&session.runtime));
                }
                if let Some((t, limit)) = planner
                    .tool(&session.runtime)
                    .and_then(|t| t.limit.as_ref().map(|l| (t, l)))
                {
                    return Err(invalid(format!(
                        "{}'s conversation is on {}, which {}. Write again then, or end this chat \
                         to start one on another AI tool",
                        position.title,
                        t.info.label,
                        plenipo_router::engine::limit_words(limit, planner.now)
                    )));
                }
                let routing = session.metadata["workforce"]["routing"].clone();
                (
                    session.id,
                    true,
                    session.runtime,
                    session.model,
                    None,
                    routing,
                )
            }
            None => {
                let decision = decide(planner, view, position, project, &[]);
                let Some(choice) = decision.choice.clone() else {
                    return Err(invalid(format!(
                        "{} cannot start: {}",
                        position.title, decision.reason
                    )));
                };
                if !allowed(project, &choice.runtime_id) {
                    return Err(not_allowed(&choice.runtime_id));
                }
                // Its first conversation takes the position's ID, so two first messages at the
                // same moment start the same one (the second is refused as busy), as a member's
                // takes its agent's; a fresh ID after that.
                let session_id = if ledger.runtime_session(&position.id)?.is_none() {
                    position.id.clone()
                } else {
                    uuid::Uuid::new_v4().to_string()
                };
                (
                    session_id,
                    false,
                    choice.runtime_id,
                    choice.model,
                    choice.effort,
                    json!(decision),
                )
            }
        };
    let tool = planner.tool(&runtime_id).ok_or_else(|| {
        invalid(format!(
            "{}'s AI tool ({runtime_id}) is not available in this version of Plenipo",
            position.title
        ))
    })?;
    let agent_id = uuid::Uuid::new_v4().to_string();
    let workforce = json!({
        "positionId": position.id,
        "agentId": agent_id,
        "projectId": project_id,
        "departmentId": view.work_department_of(&position.id).map(|d| d.id.clone()),
        "leadId": view.lead_of(&position.id).map(|l| l.id.clone()),
        "routing": routing,
    });
    Ok(ConversationPlan {
        session_id,
        existing,
        runtime_id: runtime_id.clone(),
        model: model.clone(),
        effort,
        workforce,
        project_id: project_id.clone(),
        title: position.title.clone(),
        worker: Some(NewWorker {
            agent_id,
            position_id: position.id.clone(),
            role_id: position.role_id.clone(),
            runtime_id,
            runtime_provider: Some(tool.info.provider.clone()),
            model,
            project_id,
            routing,
        }),
    })
}
