//! What members of the organization and their workers are told about themselves and their
//! team (written into their instructions by Liaison, ADR-009 §5).

use plenipo_ledger::{OversightKind, Position, Role};

use crate::view::{OrgView, TeamMember};

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

/// How a lead of leads (a VP, a manager) works with the full-time members on its team (Phase 8,
/// ADR-016).
const DELEGATION: &str = "Hand each objective to the member of your team who leads the work it \
    concerns (for a project, its supervisor): the whole objective, the project, and what to send \
    back, in a few sentences. When the reply comes, report to the owner in a few short lines: \
    what was done and by whom, the files changed, the tests run and their results, the branch \
    and any pull request, review findings still open, and approvals still needed. Say plainly \
    what is not finished.";

/// How a project's supervisor runs a development objective with its team (Phase 8, ADR-016).
const PLAYBOOK: &str = "How to run the objective: 1) Break it into small, bounded tasks. \
    2) Have a developer do each one on the objective's branch and commit it. 3) Have your \
    reviewer review the work, passing the developer's task as context ({\"kind\": \"task\", \
    \"taskId\": \"...\"}); if it requests changes, send the findings to a developer and have it \
    reviewed again. 4) Have QA run the project's tests and check the acceptance criteria; if \
    they fail, have a developer fix it and QA check again. 5) If the change affects \
    documentation, have your documentation writer update it. 6) Answer with a short report: \
    what changed, tests and results, the review verdict and any open findings, the branch, \
    and approvals still needed. Open a pull request only when the objective asks for one (it \
    waits for the owner's approval). Skip a step your team has no one for, and say so.";

/// A reviewer's, QA's, or security auditor's verdict, which Plenipo reads for the objective's
/// result (Phase 8).
const VERDICT: &str = "End your answer with your verdict in a fenced block:\n```plenipo-review\n\
    {\"verdict\": \"approve\", \"findings\": [{\"severity\": \"major\", \"file\": \
    \"src/app.rs\", \"summary\": \"one line\"}]}\n```\n\"verdict\" is \"approve\" or \
    \"request-changes\" (use it when a blocker or major finding must be fixed); \"severity\" is \
    \"blocker\", \"major\", or \"minor\". List only real problems; no findings is fine.";

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
        let team = view.team(&me.id);
        if team.iter().any(|m| view.persistent(m.position)) {
            s.push('\n');
            s.push_str(DELEGATION);
        }
        if view.coordinates(&me.id).is_some() && team.iter().any(|m| !view.persistent(m.position)) {
            s.push('\n');
            s.push_str(PLAYBOOK);
        }
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
    let reviews = serving.is_some() || view.role(position).is_some_and(gives_verdict);
    if reviews {
        s.push('\n');
        s.push_str(VERDICT);
    }
    s
}

/// Reviewers, QA, and security auditors end their answers with a verdict (Phase 8): the
/// built-in Code Reviewer, QA Engineer, and Security Auditor roles, and custom roles marked so.
fn gives_verdict(role: &Role) -> bool {
    role.metadata["verdict"] == true
        || (role.metadata["template"] == true
            && crate::templates::VERDICT_ROLES.contains(&role.name.as_str()))
}

/// How a team member is listed in a worker's instructions, after its `role:<title>` address.
/// `tool` names a fixed position's AI tool; an automatic position's is left out, since its
/// role's model policy picks one for each worker (so the instructions stay the same when the
/// policy changes).
pub fn member_label(view: &OrgView<'_>, tool: Option<&str>, m: &TeamMember<'_>) -> String {
    if view.persistent(m.position) {
        // A full-time member does the task in its own conversation, with its own team (ADR-016).
        let what = if let Some(p) = view.coordinates(&m.position.id) {
            format!(", leads the project {}", p.name)
        } else if let Some(d) = view.heads(&m.position.id) {
            format!(", runs the {} department", d.name)
        } else {
            String::new()
        };
        return format!(
            "{}{what}; full-time, it works on the task with its own team",
            role_name(view, m.position)
        );
    }
    let base = match tool {
        Some(tool) => format!("{} on {tool}", role_name(view, m.position)),
        None => role_name(view, m.position).to_owned(),
    };
    match m.oversight {
        Some(o) => format!("{base}, your team's {}", o.kind.label()),
        None => format!("{base}, a new worker for each request"),
    }
}
