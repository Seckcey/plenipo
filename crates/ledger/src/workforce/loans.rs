//! Agents lent to another team (Phase 18, ADR-054): lending, the objective a loan joins, and
//! going home — when the objective is done, when the owner sends it home, or when the team it
//! helps is archived. Each change is one transaction with its `org.*` event.

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use super::{all, get_position, invalid, now, org_event, refuse_if_busy, unfinished_work, Org};
use crate::dto::*;
use crate::error::Result;
use crate::rows::{opt_u64, parse_enum, u64_of};
use crate::Ledger;

pub(crate) const LOAN_COLS: &str = "id, position_id, from_lead_id, to_lead_id, to_project_id, \
    to_department_id, until, objective_task_id, state, going_home, started_at, ended_at, end_reason";

pub(crate) fn loan_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Loan> {
    Ok(Loan {
        id: r.get(0)?,
        position_id: r.get(1)?,
        from_lead_id: r.get(2)?,
        to_lead_id: r.get(3)?,
        to_project_id: r.get(4)?,
        to_department_id: r.get(5)?,
        until: parse_enum(6, r.get(6)?, LoanUntil::parse)?,
        objective_task_id: r.get(7)?,
        active: r.get::<_, String>(8)? == "active",
        going_home: r.get::<_, i64>(9)? != 0,
        started_at: u64_of(r.get(10)?),
        ended_at: opt_u64(r.get(11)?),
        end_reason: r.get(12)?,
    })
}

/// Every loan in effect now.
pub(crate) fn active_loans(c: &Connection) -> Result<Vec<Loan>> {
    all(
        c,
        &format!("SELECT {LOAN_COLS} FROM loans WHERE state = 'active' ORDER BY started_at"),
        [],
        loan_row,
    )
}

/// The loan in effect for `position_id`, if it is lent.
pub(crate) fn active_loan(c: &Connection, position_id: &str) -> Result<Option<Loan>> {
    Ok(c.query_row(
        &format!("SELECT {LOAN_COLS} FROM loans WHERE position_id = ?1 AND state = 'active'"),
        [position_id],
        loan_row,
    )
    .optional()?)
}

fn title_of(c: &Connection, id: &str) -> Result<String> {
    Ok(
        c.query_row("SELECT title FROM positions WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or_else(|| "a former position".into()),
    )
}

/// End `loan`: the agent is back on its home team. `why` says why, in plain words.
pub(crate) fn end_loan(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    loan: &Loan,
    why: &str,
    actor: &str,
) -> Result<()> {
    let ended = tx.execute(
        "UPDATE loans SET state = 'ended', ended_at = ?2, end_reason = ?3
         WHERE id = ?1 AND state = 'active'",
        params![loan.id, now(), why],
    )?;
    if ended == 0 {
        return Ok(());
    }
    org_event(
        out,
        tx,
        actor,
        "agent_returned",
        json!({
            "loanId": loan.id,
            "positionId": loan.position_id,
            "title": title_of(tx, &loan.position_id)?,
            "toLeadId": loan.to_lead_id,
            "to": title_of(tx, &loan.to_lead_id)?,
            "objectiveTaskId": loan.objective_task_id,
            "reason": why,
        }),
    )
}

/// The objective `task_id` belongs to: the task at the top of its chain.
pub(crate) fn objective_root(c: &Connection, task_id: &str) -> Result<String> {
    let mut current = task_id.to_owned();
    for _ in 0..64 {
        let parent: Option<String> = c
            .query_row(
                "SELECT parent_task_id FROM tasks WHERE id = ?1",
                [&current],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        match parent {
            Some(p) => current = p,
            None => return Ok(current),
        }
    }
    Ok(current)
}

/// Why a lent `position` may not take the job of task `task_id`, handed over by `lead_id`'s
/// team; `None` when it may. A loan for one objective joins the first objective that hands it a
/// job (called in the transaction that records the worker).
pub(crate) fn join_loan(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    position: &Position,
    lead_id: Option<&str>,
    task_id: &str,
    actor: &str,
) -> Result<()> {
    let Some(loan) = active_loan(tx, &position.id)? else {
        // Not lent: it works for its own team, or a team it oversees (the directory decides).
        return Ok(());
    };
    let helps = title_of(tx, &loan.to_lead_id)?;
    if lead_id != Some(loan.to_lead_id.as_str()) {
        return Err(invalid(format!(
            "{} is lent to {helps}'s team and takes work only from that team until it comes back",
            position.title
        )));
    }
    // Going home: the directory refuses new work for it; a job that was placed just before
    // Send home is kept (refusing here would drop every hand-off of the requester's turn), and
    // the loan ends when its last task ends.
    if loan.going_home || loan.until != LoanUntil::Objective {
        return Ok(());
    }
    let root = objective_root(tx, task_id)?;
    match loan.objective_task_id.as_deref() {
        Some(joined) if joined == root => Ok(()),
        Some(_) => Err(invalid(format!(
            "{} is lent to {helps}'s team for another objective; it comes back when that one is \
             done",
            position.title
        ))),
        None => {
            tx.execute(
                "UPDATE loans SET objective_task_id = ?2 WHERE id = ?1",
                params![loan.id, root],
            )?;
            org_event(
                out,
                tx,
                actor,
                "agent_joined_objective",
                json!({
                    "loanId": loan.id,
                    "positionId": position.id,
                    "title": position.title,
                    "objectiveTaskId": root,
                }),
            )
        }
    }
}

/// A task ended (`to` is final): a loan whose objective it was, or whose agent was going home
/// after it, ends — once the lent agent has nothing unfinished (otherwise it goes home when its
/// own task ends). Called in the task's own transaction, after its state changed.
pub(crate) fn follow_loans(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    task: &Task,
    to: TaskState,
    actor: &str,
) -> Result<()> {
    // Only the organization's tasks can be a loan's objective or a lent agent's job.
    if !to.is_terminal() || !task.metadata["workforce"].is_object() {
        return Ok(());
    }
    let done: Vec<Loan> = all(
        tx,
        &format!("SELECT {LOAN_COLS} FROM loans WHERE state = 'active' AND objective_task_id = ?1"),
        [&task.id],
        loan_row,
    )?;
    for loan in done {
        if unfinished_work(tx, &loan.position_id)? == 0 {
            end_loan(tx, out, &loan, "its objective is done", actor)?;
        } else if !loan.going_home {
            tx.execute(
                "UPDATE loans SET going_home = 1 WHERE id = ?1",
                params![loan.id],
            )?;
        }
    }
    if let Some(position_id) = task.metadata["workforce"]["positionId"].as_str() {
        if let Some(loan) = active_loan(tx, position_id)? {
            // Its objective may have ended before this task did (a job handed over while the
            // objective was being stopped): it goes home once its own work is done, too.
            let objective_done = loan
                .objective_task_id
                .as_deref()
                .is_some_and(|o| task_done(tx, o));
            if (loan.going_home || objective_done) && unfinished_work(tx, position_id)? == 0 {
                let why = if objective_done {
                    "its objective is done"
                } else {
                    "you sent it home"
                };
                end_loan(tx, out, &loan, why, actor)?;
            }
        }
    }
    Ok(())
}

fn task_done(c: &Connection, id: &str) -> bool {
    c.query_row("SELECT state FROM tasks WHERE id = ?1", [id], |r| {
        r.get::<_, String>(0)
    })
    .is_ok_and(|s| matches!(s.as_str(), "succeeded" | "failed" | "cancelled"))
}

/// Archiving `position` ends the loans it is part of: its own (it goes home first, and leaves
/// with its team), and those lent to its team (they go home). An agent lent to its team that
/// still works for it stops the archive, like any team member with unfinished work.
pub(super) fn end_loans_on_archive(
    tx: &Connection,
    out: &mut Vec<LedgerEvent>,
    position: &Position,
    actor: &str,
) -> Result<()> {
    if let Some(loan) = active_loan(tx, &position.id)? {
        refuse_if_busy(tx, position, "archiving it")?;
        end_loan(tx, out, &loan, "its own team was archived", actor)?;
    }
    let lent_in: Vec<Loan> = all(
        tx,
        &format!("SELECT {LOAN_COLS} FROM loans WHERE state = 'active' AND to_lead_id = ?1"),
        [&position.id],
        loan_row,
    )?;
    for loan in lent_in {
        let agent = get_position(tx, &loan.position_id)?;
        refuse_if_busy(tx, &agent, &format!("archiving {}", position.title))?;
        end_loan(tx, out, &loan, "the team it helped was archived", actor)?;
    }
    Ok(())
}

impl Org {
    /// The loan in effect for `id`, if it is lent.
    pub(super) fn loan_of(&self, id: &str) -> Option<&Loan> {
        self.loans.iter().find(|l| l.position_id == id)
    }

    /// Active positions lent to the team led by `lead`.
    pub(super) fn lent_to(&self, lead: &str) -> Vec<&Position> {
        self.loans
            .iter()
            .filter(|l| l.to_lead_id == lead)
            .filter_map(|l| self.positions.get(&l.position_id))
            .collect()
    }
}

impl Ledger {
    /// The objective task `task_id` belongs to: the task at the top of its chain.
    pub fn objective_of(&self, task_id: &str) -> Result<String> {
        self.read(|c| objective_root(c, task_id))
    }

    /// Loans in effect now.
    pub fn loans(&self) -> Result<Vec<Loan>> {
        self.read(active_loans)
    }

    /// The loans of `position_id`, newest first, ended ones included.
    pub fn loans_of(&self, position_id: &str, limit: u32) -> Result<Vec<Loan>> {
        self.read(|c| {
            all(
                c,
                &format!(
                    "SELECT {LOAN_COLS} FROM loans WHERE position_id = ?1
                     ORDER BY started_at DESC, rowid DESC LIMIT ?2"
                ),
                params![position_id, limit],
                loan_row,
            )
        })
    }

    /// Lend on-call position `id` to the team led by `to_lead` (ADR-054), for one objective of
    /// that team or until the owner sends it home.
    pub fn lend_position(
        &self,
        id: &str,
        to_lead: &str,
        until: LoanUntil,
        actor: &str,
    ) -> Result<Loan> {
        self.write(|tx, out| {
            let org = Org::load(tx)?;
            let position = org.position(tx, id)?.clone();
            if org.role_of(&position)?.persistent {
                return Err(invalid(format!(
                    "{} is a full-time position: its conversation remembers its own project's \
                     work, so move it instead of lending it",
                    position.title
                )));
            }
            if org.loan_of(id).is_some() {
                return Err(invalid(format!(
                    "{} is already lent; send it home first",
                    position.title
                )));
            }
            let lead = org.position(tx, to_lead)?.clone();
            if !org.role_of(&lead)?.persistent {
                return Err(invalid(format!(
                    "{} is an on-call position and has no team to lend to",
                    lead.title
                )));
            }
            if position.reports_to.as_deref() == Some(to_lead) {
                return Err(invalid(format!(
                    "{} is already on {}'s team",
                    position.title, lead.title
                )));
            }
            if let Some(o) = org
                .oversight
                .iter()
                .find(|o| o.overseer_id == id && o.target_id == to_lead)
            {
                return Err(invalid(format!(
                    "{} already serves {}'s team as its {}",
                    position.title,
                    lead.title,
                    o.kind.label()
                )));
            }
            refuse_if_busy(tx, &position, "lending it")?;
            org.check_title(&position.title, Some(to_lead), Some(id))?;
            let project = org.project_of(to_lead);
            Org::check_runtime(project, position.runtime_id.as_deref(), &position.title)?;
            let department = org.department_of(to_lead);
            let loan_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO loans (id, position_id, from_lead_id, to_lead_id, to_project_id,
                     to_department_id, until, state, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8)",
                params![
                    loan_id,
                    id,
                    position.reports_to,
                    to_lead,
                    project.map(|p| p.id.clone()),
                    department.map(|d| d.id.clone()),
                    until.as_str(),
                    now()
                ],
            )?;
            org_event(
                out,
                tx,
                actor,
                "agent_lent",
                json!({
                    "loanId": loan_id,
                    "positionId": id,
                    "title": position.title,
                    "fromLeadId": position.reports_to,
                    "from": org.lead_name(position.reports_to.as_deref()),
                    "toLeadId": to_lead,
                    "to": lead.title,
                    "projectId": project.map(|p| p.id.clone()),
                    "project": project.map(|p| p.name.clone()),
                    "departmentId": department.map(|d| d.id.clone()),
                    "until": until,
                }),
            )?;
            Ok(tx.query_row(
                &format!("SELECT {LOAN_COLS} FROM loans WHERE id = ?1"),
                [&loan_id],
                loan_row,
            )?)
        })
    }

    /// Send lent position `id` home (ADR-054 §6): now, or — while it works — when its task ends.
    pub fn send_home(&self, id: &str, actor: &str) -> Result<Loan> {
        self.write(|tx, out| {
            let position = get_position(tx, id)?;
            let loan = active_loan(tx, id)?.ok_or_else(|| {
                invalid(format!("{} is not lent to another team", position.title))
            })?;
            if unfinished_work(tx, id)? == 0 {
                end_loan(tx, out, &loan, "you sent it home", actor)?;
            } else if !loan.going_home {
                tx.execute(
                    "UPDATE loans SET going_home = 1 WHERE id = ?1",
                    params![loan.id],
                )?;
                org_event(
                    out,
                    tx,
                    actor,
                    "agent_going_home",
                    json!({
                        "loanId": loan.id,
                        "positionId": id,
                        "title": position.title,
                        "toLeadId": loan.to_lead_id,
                    }),
                )?;
            }
            Ok(tx.query_row(
                &format!("SELECT {LOAN_COLS} FROM loans WHERE id = ?1"),
                [&loan.id],
                loan_row,
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::super::insert_worker;
    use crate::dto::*;
    use crate::error::LedgerError;
    use crate::test_support::ledger;
    use crate::workforce::RoleTemplate;
    use crate::Ledger;

    const OWNER: &str = "owner";

    fn template(name: &'static str, role_type: RoleType, persistent: bool) -> RoleTemplate {
        RoleTemplate {
            name,
            description: "",
            role_type,
            persistent,
            metadata: Value::Null,
            formerly: &[],
        }
    }

    fn position(role: &str, title: &str, reports_to: Option<&str>) -> NewPosition {
        NewPosition {
            title: title.into(),
            role_id: role.into(),
            reports_to: reports_to.map(str::to_owned),
            staffed: true,
            ..NewPosition::default()
        }
    }

    /// Development runs Website, with a Security Auditor; Marketing runs Shop, with a developer.
    struct World {
        l: Ledger,
        auditor: String,
        auditor_role: String,
        staff: String,
        website: String,
        website_lead: String,
        shop: String,
        shop_lead: String,
        marketing: String,
    }

    fn world() -> World {
        let l = ledger();
        let roles = l
            .ensure_roles(
                &[
                    template("Department Manager", RoleType::DepartmentManager, true),
                    template("Project Coordinator", RoleType::ProjectCoordinator, true),
                    template("Security Auditor", RoleType::Worker, false),
                    template("Senior Developer", RoleType::Worker, false),
                    template("Staff Engineer", RoleType::Worker, true),
                ],
                "plenipo",
            )
            .unwrap();
        let role = |n: &str| roles.iter().find(|r| r.name == n).unwrap().id.clone();
        let project = |name: &str, dept: &str| {
            l.create_project_with_coordinator(
                dept,
                &ProjectSettings {
                    name: name.into(),
                    description: String::new(),
                    allowed_runtimes: vec!["claude-code".into()],
                    ..ProjectSettings::default()
                },
                &position(
                    &role("Project Coordinator"),
                    &format!("{name} Supervisor"),
                    None,
                ),
                OWNER,
            )
            .unwrap()
        };
        let (development, _) = l
            .create_department_with_head(
                "Development",
                "",
                &position(&role("Department Manager"), "Development Manager", None),
                OWNER,
            )
            .unwrap();
        let (marketing, _) = l
            .create_department_with_head(
                "Marketing",
                "",
                &position(&role("Department Manager"), "Marketing Manager", None),
                OWNER,
            )
            .unwrap();
        let (website, website_lead) = project("Website", &development.id);
        let (shop, shop_lead) = project("Shop", &marketing.id);
        let (auditor, _) = l
            .create_position(
                &position(
                    &role("Security Auditor"),
                    "Security Auditor",
                    Some(&website_lead.id),
                ),
                OWNER,
            )
            .unwrap();
        let (staff, _) = l
            .create_position(
                &position(
                    &role("Staff Engineer"),
                    "Staff Engineer",
                    Some(&website_lead.id),
                ),
                OWNER,
            )
            .unwrap();
        l.create_position(
            &position(
                &role("Senior Developer"),
                "Shop Developer",
                Some(&shop_lead.id),
            ),
            OWNER,
        )
        .unwrap();
        World {
            auditor_role: auditor.role_id.clone(),
            auditor: auditor.id,
            staff: staff.id,
            website: website.id,
            website_lead: website_lead.id,
            shop: shop.id,
            shop_lead: shop_lead.id,
            marketing: marketing.id,
            l,
        }
    }

    fn err<T: std::fmt::Debug>(r: crate::Result<T>) -> String {
        match r {
            Err(LedgerError::InvalidInput(m)) => m,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn types(l: &Ledger) -> Vec<String> {
        l.recent_events(500)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect()
    }

    /// The objective a lead works on (its root task).
    fn objective(l: &Ledger, lead: &str, project: &str) -> String {
        l.create_task(
            NewTask {
                requested_by: OWNER.into(),
                objective: "An objective".into(),
                project_id: Some(project.into()),
                metadata: json!({ "workforce": { "positionId": lead } }),
                ..NewTask::default()
            },
            OWNER,
        )
        .unwrap()
        .id
    }

    /// A job handed to `w.auditor` by `lead`'s team, in objective `parent`.
    fn job(w: &World, parent: &str, lead: &str, project: &str) -> crate::Result<String> {
        let agent_id = uuid::Uuid::new_v4().to_string();
        let new = NewTask {
            parent_task_id: Some(parent.into()),
            requested_by: "agent:claude-code".into(),
            objective: "Check it".into(),
            project_id: Some(project.into()),
            metadata: json!({ "workforce": {
                "positionId": w.auditor, "agentId": agent_id, "leadId": lead,
            } }),
            ..NewTask::default()
        };
        let worker = NewWorker {
            agent_id,
            position_id: w.auditor.clone(),
            role_id: w.auditor_role.clone(),
            runtime_id: "claude-code".into(),
            runtime_provider: Some("anthropic".into()),
            model: None,
            project_id: Some(project.into()),
            routing: Value::Null,
        };
        // The task and its worker are recorded together, as Liaison does.
        w.l.write(|tx, out| {
            let task = crate::tasks::insert(tx, out, new.clone(), "liaison")?;
            insert_worker(tx, out, &worker, &task.id, "liaison")?;
            Ok(task.id)
        })
    }

    fn finish(l: &Ledger, task: &str) {
        l.transition_task(task, TaskState::Running, "agent:claude-code", None)
            .unwrap();
        l.transition_task(task, TaskState::Succeeded, "agent:claude-code", None)
            .unwrap();
    }

    fn loan(l: &Ledger, position: &str) -> Loan {
        l.loans_of(position, 1).unwrap().remove(0)
    }

    #[test]
    fn a_lent_agent_works_for_one_objective_of_the_other_team_then_comes_home() {
        let w = world();
        let lent =
            w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Objective, OWNER)
                .unwrap();
        assert!(lent.active);
        assert_eq!(lent.to_project_id.as_deref(), Some(w.shop.as_str()));
        assert_eq!(lent.to_department_id.as_deref(), Some(w.marketing.as_str()));
        assert_eq!(lent.from_lead_id.as_deref(), Some(w.website_lead.as_str()));
        let event = w.l.recent_events(50).unwrap();
        let lent_event = event
            .iter()
            .find(|e| e.event_type == "org.agent_lent")
            .unwrap();
        assert_eq!(lent_event.payload["to"], "Shop Supervisor");
        assert_eq!(lent_event.payload["until"], "objective");

        // While lent it stays where it is, and cannot be moved, archived, or lent again.
        assert!(
            err(w.l.move_position(&w.auditor, Some(&w.shop_lead), OWNER))
                .contains("send it home first")
        );
        assert!(err(w.l.archive_position(&w.auditor, OWNER)).contains("send it home first"));
        assert!(err(w
            .l
            .lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER))
        .contains("already lent"));

        // It takes work only from the team it helps.
        let home = objective(&w.l, &w.website_lead, &w.website);
        assert!(err(job(&w, &home, &w.website_lead, &w.website))
            .contains("lent to Shop Supervisor's team"));

        // The first job ties the loan to that team's objective; a second check in it is fine.
        let shop = objective(&w.l, &w.shop_lead, &w.shop);
        let first = job(&w, &shop, &w.shop_lead, &w.shop).unwrap();
        assert_eq!(
            loan(&w.l, &w.auditor).objective_task_id.as_deref(),
            Some(shop.as_str())
        );
        assert!(types(&w.l).contains(&"org.agent_joined_objective".to_owned()));
        let other = objective(&w.l, &w.shop_lead, &w.shop);
        assert!(err(job(&w, &other, &w.shop_lead, &w.shop)).contains("another objective"));
        finish(&w.l, &first);
        let second = job(&w, &shop, &w.shop_lead, &w.shop).unwrap();
        finish(&w.l, &second);
        assert!(
            loan(&w.l, &w.auditor).active,
            "home when the objective is done"
        );

        // The objective is done: it goes home by itself.
        finish(&w.l, &shop);
        let back = loan(&w.l, &w.auditor);
        assert!(!back.active);
        assert_eq!(back.end_reason.as_deref(), Some("its objective is done"));
        let returned =
            w.l.recent_events(50)
                .unwrap()
                .into_iter()
                .find(|e| e.event_type == "org.agent_returned")
                .unwrap();
        assert_eq!(returned.payload["reason"], "its objective is done");
        assert_eq!(returned.payload["objectiveTaskId"], shop);
        // Home again, it works for its own team.
        job(&w, &home, &w.website_lead, &w.website).unwrap();
    }

    #[test]
    fn lending_follows_the_rules() {
        let w = world();
        assert!(err(w
            .l
            .lend_position(&w.staff, &w.shop_lead, LoanUntil::Objective, OWNER))
        .contains("move it instead"));
        assert!(err(w
            .l
            .lend_position(&w.auditor, &w.website_lead, LoanUntil::Objective, OWNER))
        .contains("already on"));
        let developer =
            w.l.org_records()
                .unwrap()
                .positions
                .into_iter()
                .find(|p| p.title == "Shop Developer")
                .unwrap();
        assert!(err(w
            .l
            .lend_position(&w.auditor, &developer.id, LoanUntil::Objective, OWNER))
        .contains("no team to lend to"));
        // The same title on that team would make hand-offs ambiguous.
        w.l.update_position(
            &developer.id,
            &PositionPatch {
                title: Some("Security Auditor".into()),
                ..PositionPatch::default()
            },
            OWNER,
        )
        .unwrap();
        assert!(err(w
            .l
            .lend_position(&w.auditor, &w.shop_lead, LoanUntil::Objective, OWNER))
        .contains("already has a team member named"));
        w.l.update_position(
            &developer.id,
            &PositionPatch {
                title: Some("Shop Developer".into()),
                ..PositionPatch::default()
            },
            OWNER,
        )
        .unwrap();
        // Refused while it has unfinished work.
        let home = objective(&w.l, &w.website_lead, &w.website);
        let busy = job(&w, &home, &w.website_lead, &w.website).unwrap();
        assert!(err(w
            .l
            .lend_position(&w.auditor, &w.shop_lead, LoanUntil::Objective, OWNER))
        .contains("unfinished"));
        finish(&w.l, &busy);
        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER)
            .unwrap();
        // A new team member with its title is refused while it is lent there.
        assert!(err(w.l.update_position(
            &developer.id,
            &PositionPatch {
                title: Some("Security Auditor".into()),
                ..PositionPatch::default()
            },
            OWNER,
        ))
        .contains("Security Auditor"));
        // Nor can the lent agent take a title of the team it helps, or an AI tool that team's
        // project does not allow (hand-offs there would fail).
        assert!(err(w.l.update_position(
            &w.auditor,
            &PositionPatch {
                title: Some("Shop Developer".into()),
                ..PositionPatch::default()
            },
            OWNER,
        ))
        .contains("Shop Developer"));
        assert!(err(w.l.update_position(
            &w.auditor,
            &PositionPatch {
                runtime: Some(Some(("codex".into(), Some("openai".into())))),
                ..PositionPatch::default()
            },
            OWNER,
        ))
        .contains("codex"));
    }

    #[test]
    fn sent_home_now_or_when_its_task_ends() {
        let w = world();
        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER)
            .unwrap();
        let idle = w.l.send_home(&w.auditor, OWNER).unwrap();
        assert!(!idle.active);
        assert_eq!(idle.end_reason.as_deref(), Some("you sent it home"));
        assert!(err(w.l.send_home(&w.auditor, OWNER)).contains("not lent"));

        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER)
            .unwrap();
        let shop = objective(&w.l, &w.shop_lead, &w.shop);
        let running = job(&w, &shop, &w.shop_lead, &w.shop).unwrap();
        // Until returned: any objective of that team.
        let other = objective(&w.l, &w.shop_lead, &w.shop);
        let more = job(&w, &other, &w.shop_lead, &w.shop).unwrap();
        finish(&w.l, &more);
        let going = w.l.send_home(&w.auditor, OWNER).unwrap();
        assert!(going.active && going.going_home);
        assert!(types(&w.l).contains(&"org.agent_going_home".to_owned()));
        // The directory offers it no new work; a job placed just before Send home is kept (the
        // requester's other hand-offs are not dropped), and it goes home after its last task.
        let late = job(&w, &shop, &w.shop_lead, &w.shop).unwrap();
        finish(&w.l, &running);
        assert!(loan(&w.l, &w.auditor).active, "one task still to finish");
        finish(&w.l, &late);
        let back = loan(&w.l, &w.auditor);
        assert!(!back.active);
        assert_eq!(back.end_reason.as_deref(), Some("you sent it home"));
    }

    #[test]
    fn a_job_that_joins_an_objective_already_ended_still_brings_it_home() {
        let w = world();
        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Objective, OWNER)
            .unwrap();
        // The objective is stopped while one of its steps is still running (Liaison stops a
        // running step later); that step hands the lent agent a job in the meantime.
        let shop = objective(&w.l, &w.shop_lead, &w.shop);
        w.l.transition_task(&shop, TaskState::Running, "agent:claude-code", None)
            .unwrap();
        let step =
            w.l.create_task(
                NewTask {
                    parent_task_id: Some(shop.clone()),
                    requested_by: "liaison".into(),
                    objective: "A step".into(),
                    project_id: Some(w.shop.clone()),
                    metadata: json!({ "workforce": { "positionId": w.shop_lead } }),
                    ..NewTask::default()
                },
                OWNER,
            )
            .unwrap()
            .id;
        w.l.transition_task(&step, TaskState::Running, "agent:claude-code", None)
            .unwrap();
        w.l.transition_task(&shop, TaskState::Cancelled, OWNER, None)
            .unwrap();
        let late = job(&w, &step, &w.shop_lead, &w.shop).unwrap();
        assert_eq!(
            loan(&w.l, &w.auditor).objective_task_id.as_deref(),
            Some(shop.as_str())
        );
        finish(&w.l, &late);
        let back = loan(&w.l, &w.auditor);
        assert!(
            !back.active,
            "it does not stay lent to an objective that is over"
        );
        assert_eq!(back.end_reason.as_deref(), Some("its objective is done"));
    }

    #[test]
    fn archiving_the_team_it_helps_sends_it_home_and_archiving_its_own_takes_it_along() {
        let w = world();
        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER)
            .unwrap();
        let shop = objective(&w.l, &w.shop_lead, &w.shop);
        let running = job(&w, &shop, &w.shop_lead, &w.shop).unwrap();
        assert!(err(w.l.archive_project(&w.shop, OWNER)).contains("unfinished"));
        finish(&w.l, &running);
        finish(&w.l, &shop);
        w.l.archive_project(&w.shop, OWNER).unwrap();
        let back = loan(&w.l, &w.auditor);
        assert_eq!(
            back.end_reason.as_deref(),
            Some("the team it helped was archived")
        );
        assert_eq!(
            w.l.position(&w.auditor).unwrap().unwrap().state,
            PositionState::Active
        );

        let w = world();
        w.l.lend_position(&w.auditor, &w.shop_lead, LoanUntil::Returned, OWNER)
            .unwrap();
        w.l.archive_project(&w.website, OWNER).unwrap();
        assert_eq!(
            loan(&w.l, &w.auditor).end_reason.as_deref(),
            Some("its own team was archived")
        );
        assert_eq!(
            w.l.position(&w.auditor).unwrap().unwrap().state,
            PositionState::Archived
        );
    }
}
