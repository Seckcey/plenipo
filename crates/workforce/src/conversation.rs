//! A full-time member's conversation (ADR-009 §2, ADR-016): the one runtime session its agent
//! keeps across objectives, whether the owner gives the objective or the member's lead hands it
//! over. It continues on the AI tool it started on; a new one starts on the owner's fixed choice
//! or on what the role's model policy picks now (Phase 6).

use plenipo_ledger::{
    AgentInstance, AgentRoute, Ledger, Position, PositionState, RuntimeSessionState,
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
    })
}
