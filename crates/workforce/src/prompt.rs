//! What members of the organization and their workers are told about themselves and their
//! team (written into their instructions by Liaison, ADR-009 §5).

use plenipo_ledger::{OversightKind, Position, Role};
use plenipo_runtime::agent::AgentRuntimeInfo;

use crate::view::{OrgView, TeamMember};

pub(crate) fn runtime_label(runtimes: &[AgentRuntimeInfo], id: &str) -> String {
    runtimes
        .iter()
        .find(|r| r.id == id)
        .map_or_else(|| id.to_owned(), |r| r.label.clone())
}

/// A role's purpose as one phrase list, or its description.
fn purpose(role: &Role) -> String {
    let items: Vec<&str> = role.metadata["purpose"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    if items.is_empty() {
        role.description.trim().trim_end_matches('.').to_owned()
    } else {
        items.join("; ")
    }
}

fn role_name<'a>(view: &OrgView<'a>, p: &Position) -> &'a str {
    view.role(p).map_or("team member", |r| r.name.as_str())
}

/// Who a persistent member is: position, department, project, supervisor, purpose.
pub fn member_identity(view: &OrgView<'_>, org: &str, me: &Position, has_team: bool) -> String {
    let mut s = format!("Your position: {}, the {}", me.title, role_name(view, me));
    if let Some(d) = view.department_of(&me.id) {
        s.push_str(&format!(" of the {} department", d.name));
    }
    s.push_str(&format!(" in {org}."));
    if let Some(p) = view.coordinates(&me.id) {
        s.push_str(&format!(" You lead the project {}", p.name));
        if let Some(repo) = &p.repository_url {
            s.push_str(&format!(" (repository {repo})"));
        }
        s.push('.');
    } else if let Some(p) = view.project_of(&me.id) {
        s.push_str(&format!(" You work on the project {}.", p.name));
    }
    match me.reports_to.as_deref().and_then(|b| view.position(b)) {
        Some(boss) => s.push_str(&format!(" You report to {}.", boss.title)),
        None => s.push_str(" You report to the owner."),
    }
    if let Some(role) = view.role(me) {
        let purpose = purpose(role);
        if !purpose.is_empty() {
            s.push_str(&format!("\nYour purpose: {purpose}."));
        }
    }
    if has_team {
        s.push_str(
            "\nGive each member of your team a short, specific task and combine their replies \
             into your answer; do a part yourself when no team member fits it.",
        );
    } else {
        s.push_str(
            "\nNo one is on your team yet, so do the work yourself; the owner can hire team \
             members in Plenipo.",
        );
    }
    s
}

/// Who a worker spawned by an on-demand position is; `serving` names the team it works for
/// through an oversight assignment.
pub fn worker_identity(
    view: &OrgView<'_>,
    org: &str,
    position: &Position,
    serving: Option<(&Position, OversightKind)>,
) -> String {
    let mut s = format!(
        "You are working as {} ({}) in {org}",
        position.title,
        role_name(view, position)
    );
    if let Some(lead) = position
        .reports_to
        .as_deref()
        .and_then(|b| view.position(b))
    {
        s.push_str(&format!(", on the team of {}", lead.title));
    }
    if let Some(p) = view.project_of(&position.id) {
        s.push_str(&format!(", project {}", p.name));
    } else if let Some(d) = view.department_of(&position.id) {
        s.push_str(&format!(", {} department", d.name));
    }
    s.push('.');
    if let Some((lead, kind)) = serving {
        s.push_str(&format!(
            " This task is for the team of {}, which you serve as its {}",
            lead.title,
            kind.label()
        ));
        if let Some(p) = view.project_of(&lead.id) {
            s.push_str(&format!(" (project {})", p.name));
        }
        s.push('.');
    }
    if let Some(role) = view.role(position) {
        let purpose = purpose(role);
        if !purpose.is_empty() {
            s.push_str(&format!(" Your purpose: {purpose}."));
        }
    }
    s
}

/// How a team member is listed in a worker's instructions, after its `role:<title>` address.
pub fn member_label(
    view: &OrgView<'_>,
    runtimes: &[AgentRuntimeInfo],
    m: &TeamMember<'_>,
) -> String {
    let base = format!(
        "{} on {}",
        role_name(view, m.position),
        runtime_label(runtimes, &m.position.runtime_id)
    );
    match m.oversight {
        Some(o) => format!("{base}, your team's {}", o.kind.label()),
        None => format!("{base}, a new worker for each request"),
    }
}
