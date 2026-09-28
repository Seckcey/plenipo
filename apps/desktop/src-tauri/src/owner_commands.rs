//! Phase 17 commands: the owner's control over workers. Model and effort rules in layers and
//! learning in layers (ADR-041), specialties (ADR-042), archive, bring back, and delete for good
//! (ADR-043), and the Workforce (ADR-045). Every one is the main window's alone
//! (capabilities/default.json): the sign window and web pages are refused.
//!
//! None of them touches files, programs, the network, the browser, or the screen: deleting for
//! good forgets records and settings, and never deletes anything on disk.

use plenipo_core::CommandError;
use plenipo_guard::Guard;
use plenipo_router::{ModelRule, RoutingSnapshot, RuleTarget};
use plenipo_workforce::{
    DeletionPreview, LearningSnapshot, OrgSnapshot, SpecialtyInput, Workforce,
};
use serde::Deserialize;
use tauri::State;

use crate::commands::{
    bounded, bounded_optional, guard_error, validate_id, validate_job, validate_runtimes,
    with_workforce, workforce_error,
};

/// What can be deleted for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteKind {
    Position,
    Project,
    Department,
}

impl DeleteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Position => "position",
            Self::Project => "project",
            Self::Department => "department",
        }
    }
}

/// Most agents one deletion can move to the Workforce.
const MAX_SAVED: usize = 500;

fn validate_rule(rule: &ModelRule) -> Result<(), CommandError> {
    for id in rule.models.iter().chain(rule.efforts.keys()) {
        validate_id("model", id)?;
    }
    validate_runtimes(&rule.never_companies)
}

// ---- Model, effort, and learning in layers (ADR-041) -----------------------------------------

/// Set the organization's, a department's, or one agent's model and effort rule. An empty rule
/// for a department or an agent removes it. Open conversations take a new effort from their next
/// task; no agent is hired for it.
#[tauri::command]
pub async fn set_model_rule(
    workforce: State<'_, Workforce>,
    target: RuleTarget,
    rule: ModelRule,
) -> Result<RoutingSnapshot, CommandError> {
    match &target {
        RuleTarget::Organization => {}
        RuleTarget::Department(id) => validate_id("department", id)?,
        RuleTarget::Agent(id) => validate_id("position", id)?,
    }
    validate_rule(&rule)?;
    let workforce = workforce.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workforce.set_model_rule(&target, &rule))
        .await
        .map_err(|e| CommandError::internal(format!("workforce task failed: {e}")))?
        .map_err(workforce_error)
}

/// Learning on or off for a role (on unless turned off).
#[tauri::command]
pub async fn set_role_learns(
    workforce: State<'_, Workforce>,
    role_id: String,
    learns: bool,
) -> Result<LearningSnapshot, CommandError> {
    validate_id("role", &role_id)?;
    with_workforce(&workforce, move |w| w.set_role_learns(&role_id, learns)).await
}

/// Learning on or off for one agent; `learns` null: it follows its role.
#[tauri::command]
pub async fn set_agent_learning(
    workforce: State<'_, Workforce>,
    position_id: String,
    learns: Option<bool>,
) -> Result<LearningSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| {
        w.set_agent_learning(&position_id, learns)
    })
    .await
}

// ---- Specialties (ADR-042) -------------------------------------------------------------------

fn validate_specialty(input: &SpecialtyInput) -> Result<(), CommandError> {
    if let Some(role) = &input.role_id {
        validate_id("role", role)?;
    }
    bounded("the name", &input.name)?;
    bounded("the title", &input.title)?;
    validate_job(&input.job)?;
    for id in &input.suggest.models {
        validate_id("model", id)?;
    }
    if input.suggest.permissions.len() > 32 || input.suggest.models.len() > 32 {
        return Err(CommandError::invalid_input("too many suggestions"));
    }
    input
        .suggest
        .permissions
        .iter()
        .try_for_each(|p| bounded("a permission", p))
}

/// Add one of your own specialties to a role (a built-in role or one of yours).
#[tauri::command]
pub async fn create_specialty(
    workforce: State<'_, Workforce>,
    input: SpecialtyInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_specialty(&input)?;
    with_workforce(&workforce, move |w| w.create_specialty(&input)).await
}

/// Change one of your own specialties (built-in ones keep their lines).
#[tauri::command]
pub async fn update_specialty(
    workforce: State<'_, Workforce>,
    specialty_id: String,
    input: SpecialtyInput,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("specialty", &specialty_id)?;
    validate_specialty(&input)?;
    with_workforce(&workforce, move |w| {
        w.update_specialty(&specialty_id, &input)
    })
    .await
}

/// Remove one of your own specialties (refused while an agent on the chart has it).
#[tauri::command]
pub async fn remove_specialty(
    workforce: State<'_, Workforce>,
    specialty_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("specialty", &specialty_id)?;
    with_workforce(&workforce, move |w| w.remove_specialty(&specialty_id)).await
}

// ---- Archive, bring back, delete for good (ADR-043) -----------------------------------------

/// Archive a department with everything in it, once nothing in it has unfinished work.
#[tauri::command]
pub async fn archive_department(
    workforce: State<'_, Workforce>,
    department_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("department", &department_id)?;
    with_workforce(&workforce, move |w| w.archive_department(&department_id)).await
}

/// Bring an archived agent back as it was.
#[tauri::command]
pub async fn bring_back_position(
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.bring_back_position(&position_id)).await
}

/// Bring an archived project back with its team.
#[tauri::command]
pub async fn bring_back_project(
    workforce: State<'_, Workforce>,
    project_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("project", &project_id)?;
    with_workforce(&workforce, move |w| w.bring_back_project(&project_id)).await
}

/// Bring an archived department back with everything archived with it.
#[tauri::command]
pub async fn bring_back_department(
    workforce: State<'_, Workforce>,
    department_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("department", &department_id)?;
    with_workforce(&workforce, move |w| w.bring_back_department(&department_id)).await
}

/// What deleting an archived item for good would take along, each agent's experience, and the
/// organization's average (for the confirmation, which asks first).
#[tauri::command]
pub async fn preview_delete_for_good(
    workforce: State<'_, Workforce>,
    kind: DeleteKind,
    id: String,
) -> Result<DeletionPreview, CommandError> {
    validate_id(kind.as_str(), &id)?;
    with_workforce(&workforce, move |w| w.preview_delete(kind.as_str(), &id)).await
}

/// Delete an archived agent, project, or department for good, after the owner confirmed. The
/// agents in `save` move to the Workforce instead. Refused while anything has unfinished work. A
/// short record stays in the Ledger; nothing on disk is touched.
#[tauri::command]
pub async fn delete_for_good(
    workforce: State<'_, Workforce>,
    guard: State<'_, Guard>,
    kind: DeleteKind,
    id: String,
    save: Vec<String>,
) -> Result<OrgSnapshot, CommandError> {
    validate_id(kind.as_str(), &id)?;
    if save.len() > MAX_SAVED {
        return Err(CommandError::invalid_input("too many agents to save"));
    }
    save.iter().try_for_each(|p| validate_id("position", p))?;
    let guard = guard.inner().clone();
    let workforce = workforce.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (snapshot, deleted) = workforce
            .delete_for_good(kind.as_str(), &id, &save)
            .map_err(workforce_error)?;
        // A deleted department's permission limit goes with it (ADR-043 §10).
        for department in &deleted.departments {
            let limited = guard
                .config()
                .map_err(guard_error)?
                .departments
                .contains_key(department);
            if limited {
                guard
                    .assign_department(department, None)
                    .map_err(guard_error)?;
            }
        }
        Ok(snapshot)
    })
    .await
    .map_err(|e| CommandError::internal(format!("workforce task failed: {e}")))?
}

// ---- The Workforce (ADR-045) ---------------------------------------------------------------

/// Save an archived agent to your Workforce.
#[tauri::command]
pub async fn save_to_workforce(
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.save_to_workforce(&position_id)).await
}

/// Hire an agent from your Workforce into a team (`reportsTo`; null: you), with its settings,
/// experience, and lessons.
#[tauri::command]
pub async fn hire_from_workforce(
    workforce: State<'_, Workforce>,
    saved_id: String,
    reports_to: Option<String>,
    title: Option<String>,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("saved agent", &saved_id)?;
    if let Some(lead) = &reports_to {
        validate_id("position", lead)?;
    }
    bounded_optional("the title", title.as_deref())?;
    with_workforce(&workforce, move |w| {
        w.hire_from_workforce(&saved_id, reports_to.as_deref(), title.as_deref())
    })
    .await
}

/// Delete an agent in your Workforce for good.
#[tauri::command]
pub async fn delete_saved_agent(
    workforce: State<'_, Workforce>,
    saved_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("saved agent", &saved_id)?;
    with_workforce(&workforce, move |w| w.delete_saved_agent(&saved_id)).await
}
