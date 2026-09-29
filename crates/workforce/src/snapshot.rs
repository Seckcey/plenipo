//! Building the organization snapshot and work views from Ledger records, open tasks, member
//! sessions, and runtime readiness. Pure functions: everything they need is passed in.

use std::collections::{HashMap, HashSet};

use plenipo_ledger::{
    AgentInstance, OrgRecords, OversightKind, Position, PositionState, RoleType, RuntimeSession,
    SavedAgent, Specialty, Task, TaskState, TilePlace,
};
use plenipo_router::Planner;
use serde_json::Value;

use crate::directory::{allowed, decide};
use crate::dto::*;
use crate::learning::LearningSettings;
use crate::templates::default_glyph;
use crate::view::OrgView;

pub(crate) struct Inputs<'a> {
    pub records: &'a OrgRecords,
    /// Unfinished organization tasks.
    pub open_tasks: &'a [Task],
    /// Organization tasks finished in the last 24 hours.
    pub finished_recent: &'a [Task],
    /// Open sessions of members and workers.
    pub sessions: &'a [RuntimeSession],
    /// Routing configuration and the AI tools' live state.
    pub planner: &'a Planner,
    /// Who learns (ADR-041).
    pub learning: &'a LearningSettings,
    pub name: String,
    pub titles: TitleTheme,
    pub notices: Vec<String>,
    /// The tiles the owner placed by hand (ADR-053).
    pub places: &'a [TilePlace],
    pub now: u64,
}

pub(crate) fn kind(t: RoleType) -> PositionKind {
    match t {
        RoleType::Superintendent => PositionKind::Superintendent,
        RoleType::DepartmentManager => PositionKind::DepartmentManager,
        RoleType::ProjectCoordinator => PositionKind::ProjectCoordinator,
        RoleType::Worker => PositionKind::Worker,
    }
}

pub(crate) fn oversight_role(k: OversightKind) -> OversightRole {
    match k {
        OversightKind::Review => OversightRole::Review,
        OversightKind::Qa => OversightRole::Qa,
        OversightKind::Security => OversightRole::Security,
    }
}

fn first_line(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        let mut out: String = line.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn position_of(task: &Task) -> Option<&str> {
    task.metadata["workforce"]["positionId"].as_str()
}

pub(crate) fn brief(task: &Task, titles: &HashMap<&str, &str>) -> TaskBrief {
    let position_id = position_of(task).map(str::to_owned);
    TaskBrief {
        id: task.id.clone(),
        objective: first_line(&task.objective, 200),
        state: task.state,
        position_title: position_id
            .as_deref()
            .and_then(|p| titles.get(p))
            .map(|t| (*t).to_owned()),
        position_id,
        project_id: task.project_id.clone(),
        parent_task_id: task.parent_task_id.clone(),
        session_id: task.metadata["sessionId"].as_str().map(str::to_owned),
        created_at: task.created_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
    }
}

/// A number from a JSON value (0 when absent).
fn count_of(v: &Value) -> u32 {
    u32::try_from(v.as_u64().unwrap_or(0)).unwrap_or(u32::MAX)
}

/// 10 for each kept lesson, 1 for each finished task (ADR-045).
pub(crate) fn score(kept_lessons: u32, tasks_done: u32) -> u32 {
    kept_lessons.saturating_mul(10).saturating_add(tasks_done)
}

/// An agent's experience, from the records and what it brought from the Workforce.
pub(crate) fn experience_counts(records: &OrgRecords, p: &Position) -> (u32, u32) {
    let live = records.experience.get(&p.id).copied().unwrap_or_default();
    let base = &p.metadata["experience"];
    (
        live.kept_lessons
            .saturating_add(count_of(&base["keptLessons"])),
        live.tasks_done.saturating_add(count_of(&base["tasksDone"])),
    )
}

/// The organization's average experience: the total of its agents' scores and how many agents
/// it is over (ADR-045: agents, not deleted, that finished a task).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Average {
    sum: u64,
    n: u64,
}

impl Average {
    /// The average, rounded, for the screen.
    pub(crate) fn shown(self) -> u32 {
        if self.n == 0 {
            return 0;
        }
        u32::try_from((self.sum + self.n / 2) / self.n).unwrap_or(u32::MAX)
    }

    /// Is `score` above the exact average (not the rounded one on screen)?
    fn exceeded_by(self, score: u32) -> bool {
        score > 0 && (self.n == 0 || u64::from(score) * self.n > self.sum)
    }
}

pub(crate) fn average_experience(records: &OrgRecords) -> Average {
    records
        .positions
        .iter()
        .filter(|p| !p.is_deleted())
        .map(|p| experience_counts(records, p))
        .filter(|(_, tasks)| *tasks > 0)
        .fold(Average::default(), |a, (kept, tasks)| Average {
            sum: a.sum + u64::from(score(kept, tasks)),
            n: a.n + 1,
        })
}

pub(crate) fn experience_info(
    kept_lessons: u32,
    tasks_done: u32,
    average: Average,
) -> ExperienceInfo {
    let score = score(kept_lessons, tasks_done);
    ExperienceInfo {
        score,
        kept_lessons,
        tasks_done,
        experienced: average.exceeded_by(score),
    }
}

pub(crate) fn specialty_info(s: &Specialty) -> SpecialtyInfo {
    SpecialtyInfo {
        id: s.id.clone(),
        role_id: s.role_id.clone(),
        name: s.name.clone(),
        title: s.title.clone(),
        built_in: s.built_in(),
        job: crate::prompt::specialty_job(s),
        suggest: serde_json::from_value(s.metadata["suggest"].clone()).unwrap_or_default(),
    }
}

fn archived_with(v: &Value) -> Option<ArchivedWith> {
    Some(ArchivedWith {
        kind: v["kind"].as_str()?.to_owned(),
        id: v["id"].as_str()?.to_owned(),
        name: v["name"].as_str()?.to_owned(),
    })
}

pub(crate) fn saved_info(records: &OrgRecords, s: &SavedAgent, average: Average) -> SavedAgentInfo {
    let e = &s.experience;
    let specialty = s
        .specialty_id
        .as_deref()
        .and_then(|id| records.specialties.iter().find(|x| x.id == id));
    SavedAgentInfo {
        id: s.id.clone(),
        title: s.title.clone(),
        role_id: s.role_id.clone(),
        role_name: records
            .roles
            .iter()
            .find(|r| r.id == s.role_id)
            .map_or_else(String::new, |r| r.name.clone()),
        specialty_id: specialty.map(|x| x.id.clone()),
        specialty: specialty.map(|x| x.name.clone()),
        experience: experience_info(
            count_of(&e["keptLessons"]),
            count_of(&e["tasksDone"]),
            average,
        ),
        places: serde_json::from_value(e["places"].clone()).unwrap_or_default(),
        first_worked: e["firstWorked"].as_u64(),
        last_worked: e["lastWorked"].as_u64(),
        lessons: s.lessons.clone(),
        runtime_id: s.settings["runtimeId"].as_str().map(str::to_owned),
        model: s.settings["model"].as_str().map(str::to_owned),
        saved_at: s.saved_at,
    }
}

fn counts(tasks: &[&Task]) -> WorkCounts {
    let mut c = WorkCounts::default();
    for t in tasks {
        match t.state {
            TaskState::Running => c.working += 1,
            TaskState::Blocked | TaskState::AwaitingApproval => c.waiting += 1,
            TaskState::Queued => c.queued += 1,
            _ => {}
        }
    }
    c
}

pub(crate) fn build(inputs: &Inputs<'_>) -> OrgSnapshot {
    let records = inputs.records;
    let view = OrgView::new(records);
    let planner = inputs.planner;
    let average = average_experience(records);
    let titles: HashMap<&str, &str> = records
        .positions
        .iter()
        .map(|p| (p.id.as_str(), p.title.as_str()))
        .collect();
    let mut by_position: HashMap<&str, Vec<&Task>> = HashMap::new();
    let tasks: HashMap<&str, &Task> = inputs
        .open_tasks
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    for t in inputs.open_tasks {
        if let Some(p) = position_of(t) {
            by_position.entry(p).or_default().push(t);
        }
    }
    let session_of = |agent: &AgentInstance| {
        inputs
            .sessions
            .iter()
            .find(|s| {
                s.metadata["workforce"]["agentId"].as_str() == Some(agent.id.as_str())
                    && agent.runtime_id.as_deref().is_none_or(|r| r == s.runtime)
            })
            .map(|s| s.id.clone())
    };

    let info = |p: &Position| -> PositionInfo {
        let role = view.role(p);
        let persistent = role.is_some_and(|r| r.persistent);
        let project = view.project_of(&p.id);
        // Where it works now: the team it is lent to, or its own (ADR-054).
        let work = view.work_project_of(&p.id);
        let own: Vec<&Task> = by_position.get(p.id.as_str()).cloned().unwrap_or_default();
        let agent = if persistent {
            records
                .agents
                .iter()
                .find(|a| a.position_id.as_deref() == Some(p.id.as_str()) && a.task_id.is_none())
        } else {
            None
        };
        let workers: Vec<WorkerInfo> = if persistent {
            Vec::new()
        } else {
            records
                .agents
                .iter()
                .filter(|a| a.position_id.as_deref() == Some(p.id.as_str()))
                .filter_map(|a| {
                    let task = tasks.get(a.task_id.as_deref()?)?;
                    Some(WorkerInfo {
                        agent_id: a.id.clone(),
                        task_id: task.id.clone(),
                        objective: first_line(&task.objective, 200),
                        state: task.state,
                        session_id: task.metadata["sessionId"].as_str().map(str::to_owned),
                        runtime_id: a
                            .runtime_id
                            .clone()
                            .or_else(|| p.runtime_id.clone())
                            .unwrap_or_default(),
                        model: a.model.clone(),
                        routing: task.metadata["workforce"]["routing"]["reason"]
                            .as_str()
                            .map(str::to_owned),
                        parent_task_id: task.parent_task_id.clone(),
                        spawned_at: a.created_at,
                        started_at: task.started_at,
                    })
                })
                .collect()
        };
        let c = counts(&own);
        let active = p.state == PositionState::Active;
        // Where its next worker (or a new agent) would go.
        let route = active.then(|| decide(planner, &view, p, work, &[]));
        // A full-time agent keeps the AI tool of the conversation it has.
        let conversation = agent
            .filter(|a| session_of(a).is_some())
            .and_then(|a| a.runtime_id.clone().map(|r| (r, a.model.clone())));
        let (runtime_id, model) = match (&conversation, &route) {
            (Some((r, m)), _) => (Some(r.clone()), m.clone()),
            _ if p.runtime_id.is_some() => (p.runtime_id.clone(), p.model.clone()),
            (None, Some(d)) => d.choice.as_ref().map_or((None, None), |c| {
                (Some(c.runtime_id.clone()), c.model.clone())
            }),
            (None, None) => (None, None),
        };
        let tool_label = |id: &str| {
            planner
                .tool(id)
                .map_or_else(|| id.to_owned(), |t| t.info.label.clone())
        };
        let detail = match (&runtime_id, &route) {
            (Some(r), _) if !allowed(work, r) => Some(format!(
                "{} does not allow {}",
                work.map_or("The project", |x| x.name.as_str()),
                tool_label(r)
            )),
            // A fixed AI tool whose AI company a rule never uses cannot start a new
            // conversation (an open one carries on).
            (Some(_), Some(d)) if conversation.is_none() && d.choice.is_none() => {
                Some(d.reason.clone())
            }
            (Some(r), _) => planner.unavailable(r),
            (None, Some(d)) => Some(d.reason.clone()),
            (None, None) => None,
        };
        let status = if p.state == PositionState::Archived {
            PositionStatus::Archived
        } else if persistent && agent.is_none() {
            PositionStatus::Vacant
        } else if c.working > 0 {
            PositionStatus::Working
        } else if c.waiting > 0 {
            PositionStatus::Waiting
        } else if c.queued > 0 {
            PositionStatus::Queued
        } else if detail.is_some() {
            PositionStatus::Unavailable
        } else {
            PositionStatus::Idle
        };
        let current_task = if persistent {
            own.iter()
                .filter(|t| matches!(t.state, TaskState::Running | TaskState::Blocked))
                .max_by_key(|t| (t.created_at, t.id.clone()))
                .map(|t| brief(t, &titles))
        } else {
            None
        };
        let former = records
            .former_agents
            .get(&p.id)
            .copied()
            .unwrap_or_default();
        PositionInfo {
            id: p.id.clone(),
            title: p.title.clone(),
            role_id: p.role_id.clone(),
            role_name: role.map_or_else(String::new, |r| r.name.clone()),
            kind: kind(view.kind(p)),
            staffing: if persistent {
                Staffing::Persistent
            } else {
                Staffing::OnDemand
            },
            reports_to: p.reports_to.clone(),
            department_id: view.department_of(&p.id).map(|d| d.id.clone()),
            project_id: project.map(|x| x.id.clone()),
            heads_department_id: view.heads(&p.id).map(|d| d.id.clone()),
            coordinates_project_id: view.coordinates(&p.id).map(|x| x.id.clone()),
            runtime_id,
            model,
            automatic: p.runtime_id.is_none(),
            route,
            active,
            sort_key: p.sort_key,
            agent: agent.map(|a| AgentInfo {
                id: a.id.clone(),
                runtime_id: a.runtime_id.clone().or_else(|| p.runtime_id.clone()),
                model: a.model.clone(),
                session_id: session_of(a),
                hired_at: a.created_at,
            }),
            workers,
            status,
            status_detail: if status == PositionStatus::Archived {
                None
            } else {
                detail
            },
            current_task,
            counts: c,
            history: PositionHistory {
                retired: former.retired,
                failed: former.failed,
                last_retired_at: former.last_retired_at,
            },
            created_at: p.created_at,
            archived_at: p.archived_at,
            specialty_id: p.specialty_id.clone(),
            specialty: p.specialty_id.as_deref().and_then(|id| {
                records
                    .specialties
                    .iter()
                    .find(|x| x.id == id)
                    .map(|x| x.name.clone())
            }),
            experience: {
                let (kept, tasks) = experience_counts(records, p);
                experience_info(kept, tasks, average)
            },
            learning: {
                let (learns, from) = crate::learning::learns(inputs.learning, p);
                LearningInfo {
                    learns,
                    from,
                    own: inputs.learning.agents.get(&p.id).copied(),
                }
            },
            own_rule: planner.config.positions.get(&p.id).cloned(),
            archived_with: archived_with(&p.metadata["archive"]["with"]),
            deleted: p.is_deleted(),
            in_workforce: p.metadata["deleted"]["movedTo"] == "workforce",
            loan: view.loan(&p.id).map(|l| LoanInfo {
                to_lead_id: l.to_lead_id.clone(),
                to: titles
                    .get(l.to_lead_id.as_str())
                    .map_or_else(|| "another team".into(), |t| (*t).to_owned()),
                project: view.project_of(&l.to_lead_id).map(|x| x.name.clone()),
                until: l.until,
                objective_task_id: l.objective_task_id.clone(),
                going_home: l.going_home,
                since: l.started_at,
            }),
        }
    };

    let mut positions: Vec<PositionInfo> = view.tree_order().into_iter().map(info).collect();
    let mut archived: Vec<&Position> = records
        .positions
        .iter()
        .filter(|p| p.state == PositionState::Archived && !p.is_deleted())
        .collect();
    archived.sort_by_key(|p| std::cmp::Reverse(p.archived_at));
    positions.extend(archived.into_iter().map(info));
    let mut deleted: Vec<&Position> = records
        .positions
        .iter()
        .filter(|p| p.is_deleted())
        .collect();
    deleted.sort_by_key(|p| std::cmp::Reverse(p.deleted_at));
    positions.extend(deleted.into_iter().map(info));

    let active: Vec<&PositionInfo> = positions.iter().filter(|p| p.active).collect();
    let mut stats = OrgStats {
        departments: u32::try_from(
            records
                .departments
                .iter()
                .filter(|d| d.archived_at.is_none() && d.deleted_at.is_none())
                .count(),
        )
        .unwrap_or(u32::MAX),
        projects: u32::try_from(
            records
                .projects
                .iter()
                .filter(|p| p.status == "active")
                .count(),
        )
        .unwrap_or(u32::MAX),
        positions: u32::try_from(active.len()).unwrap_or(u32::MAX),
        ..OrgStats::default()
    };
    for p in &active {
        if p.staffing == Staffing::Persistent {
            if p.agent.is_some() {
                stats.staffed += 1;
            } else {
                stats.vacant += 1;
            }
        }
        stats.active_workers += u32::try_from(p.workers.len()).unwrap_or(u32::MAX);
    }
    let all_open: Vec<&Task> = inputs.open_tasks.iter().collect();
    let c = counts(&all_open);
    (stats.working, stats.waiting, stats.queued) = (c.working, c.waiting, c.queued);
    for t in inputs.finished_recent {
        match t.state {
            TaskState::Succeeded => stats.completed_24h += 1,
            TaskState::Failed => stats.failed_24h += 1,
            _ => {}
        }
    }

    OrgSnapshot {
        name: inputs.name.clone(),
        titles: inputs.titles,
        roles: records
            .roles
            .iter()
            .map(|r| {
                let strings = |key: &str| -> Vec<String> {
                    r.metadata[key]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                RoleInfo {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    description: r.description.clone(),
                    kind: kind(r.role_type),
                    staffing: if r.persistent {
                        Staffing::Persistent
                    } else {
                        Staffing::OnDemand
                    },
                    template: r.metadata["template"] == true,
                    glyph: r.metadata["glyph"]
                        .as_str()
                        .unwrap_or_else(|| default_glyph(r.role_type))
                        .to_owned(),
                    purpose: strings("purpose"),
                    default_capabilities: strings("defaultCapabilities"),
                    job: crate::templates::job_of(r),
                    specialties: records
                        .specialties
                        .iter()
                        .filter(|x| x.role_id == r.id && x.removed_at.is_none())
                        .map(specialty_info)
                        .collect(),
                    learns: !inputs.learning.off_roles.contains(&r.id),
                    learns_on_its_own: inputs.learning.auto_roles.contains(&r.id),
                }
            })
            .collect(),
        departments: records
            .departments
            .iter()
            .map(|d| DepartmentInfo {
                id: d.id.clone(),
                name: d.name.clone(),
                description: d.description.clone(),
                active: d.status == "active" && d.archived_at.is_none() && d.deleted_at.is_none(),
                head_position_id: d.head_position_id.clone(),
                project_ids: records
                    .projects
                    .iter()
                    .filter(|p| p.department_id.as_deref() == Some(d.id.as_str()))
                    .map(|p| p.id.clone())
                    .collect(),
                created_at: d.created_at,
                archived_at: d.archived_at,
                deleted: d.deleted_at.is_some(),
            })
            .collect(),
        projects: records
            .projects
            .iter()
            .map(|p| ProjectInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                department_id: p.department_id.clone(),
                repository_url: p.repository_url.clone(),
                local_path: p.local_path.clone(),
                allowed_runtimes: p.allowed_runtimes.clone(),
                capability_profile: p.capability_profile.clone(),
                coordinator_position_id: p.coordinator_position_id.clone(),
                active: p.status == "active",
                branch_per_objective: p.branch_per_objective,
                created_at: p.created_at,
                archived_at: p.archived_at,
                archived_with: archived_with(&p.metadata["archive"]["with"]),
                deleted: p.deleted_at.is_some(),
            })
            .collect(),
        positions,
        workforce: records
            .saved_agents
            .iter()
            .map(|s| saved_info(records, s, average))
            .collect(),
        average_experience: average.shown(),
        oversight: records
            .oversight
            .iter()
            .filter(|o| o.active)
            .map(|o| OversightInfo {
                id: o.id.clone(),
                role: oversight_role(o.kind),
                overseer_id: o.overseer_id.clone(),
                target_id: o.target_id.clone(),
                created_at: o.created_at,
            })
            .collect(),
        stats,
        runtimes: planner
            .tools
            .iter()
            .map(|t| RuntimeBrief {
                id: t.info.id.clone(),
                label: t.info.label.clone(),
                ready: t.info.ready,
                company: t.info.provider_label.clone(),
            })
            .collect(),
        notices: inputs.notices.clone(),
        places: inputs.places.to_vec(),
        generated_at: inputs.now,
    }
}

/// The work a position owns (`own`: its tasks, newest first) and its team's unfinished tasks;
/// for the whole organization (`position_id` = `None`), `own` is every recent task.
pub(crate) fn work_view(
    records: &OrgRecords,
    position_id: Option<&str>,
    own: &[Task],
    open_tasks: &[Task],
) -> WorkView {
    let view = OrgView::new(records);
    let titles: HashMap<&str, &str> = records
        .positions
        .iter()
        .map(|p| (p.id.as_str(), p.title.as_str()))
        .collect();
    let of = |state: fn(TaskState) -> bool| -> Vec<TaskBrief> {
        own.iter()
            .filter(|t| state(t.state))
            .map(|t| brief(t, &titles))
            .collect()
    };
    let mut recent: Vec<&Task> = own.iter().filter(|t| t.state.is_terminal()).collect();
    recent.sort_by_key(|t| std::cmp::Reverse((t.completed_at, t.created_at)));
    let team: Vec<TaskBrief> = match position_id {
        Some(root) => {
            let mut below: HashSet<&str> = HashSet::new();
            let mut stack = vec![root.to_owned()];
            while let Some(lead) = stack.pop() {
                for p in view.reports(Some(&lead)) {
                    if below.insert(p.id.as_str()) {
                        stack.push(p.id.clone());
                    }
                }
            }
            open_tasks
                .iter()
                .filter(|t| position_of(t).is_some_and(|p| below.contains(p)))
                .map(|t| brief(t, &titles))
                .collect()
        }
        None => Vec::new(),
    };
    WorkView {
        position_id: position_id.map(str::to_owned),
        running: of(|s| s == TaskState::Running),
        waiting: of(|s| matches!(s, TaskState::Blocked | TaskState::AwaitingApproval)),
        queued: of(|s| s == TaskState::Queued),
        recent: recent
            .into_iter()
            .take(20)
            .map(|t| brief(t, &titles))
            .collect(),
        team,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::{Ledger, NewPosition, NewTask, OversightKind, ProjectSettings};
    use plenipo_router::Router;
    use plenipo_runtime::agent::{
        AgentRuntimeInfo, AuthState, AuthStatus, InstallState, Installation, RuntimeCapabilities,
    };
    use serde_json::json;
    use std::sync::Arc;

    fn runtime(id: &str, ready: bool) -> AgentRuntimeInfo {
        AgentRuntimeInfo {
            id: id.into(),
            label: if id == "codex" {
                "Codex"
            } else {
                "Claude Code"
            }
            .into(),
            provider: "provider".into(),
            provider_label: "Provider".into(),
            installation: Installation {
                state: InstallState::Installed,
                executable: None,
                version: None,
                detail: None,
            },
            auth: AuthStatus {
                state: if ready {
                    AuthState::Subscription
                } else {
                    AuthState::SignedOut
                },
                method: None,
                detail: None,
            },
            capabilities: RuntimeCapabilities {
                streaming_text: true,
                resume: true,
                cancel: true,
                structured_results: true,
                billing_checked_per_turn: true,
                tool_posture: String::new(),
                effort_levels: Vec::new(),
                known_models: Vec::new(),
                default_maker: None,
                runs_other_makers: false,
            },
            install_hint: String::new(),
            login_hint: String::new(),
            ready,
            checked_at: None,
            checked_version: String::new(),
            account: Default::default(),
            reported_models: None,
            held: None,
        }
    }

    struct Org {
        l: Arc<Ledger>,
        head: String,
        coordinator: String,
        developer: String,
        qa: String,
        project: String,
    }

    /// Development (staffed manager) → Cloudline coordinator (staffed) → Senior Developer (on
    /// demand, Codex); a QA Engineer under the manager oversees Cloudline's team.
    fn org() -> Org {
        let l = Arc::new(Ledger::open_in_memory().unwrap());
        let roles = l
            .ensure_roles(&crate::templates::role_templates(), "plenipo")
            .unwrap();
        let role = |n: &str| roles.iter().find(|r| r.name == n).unwrap().id.clone();
        let hire = |role_id: String, title: &str, to: Option<&str>, runtime: &str| NewPosition {
            title: title.into(),
            role_id,
            reports_to: to.map(str::to_owned),
            runtime_id: Some(runtime.into()),
            staffed: true,
            ..NewPosition::default()
        };
        let (dept, head) = l
            .create_department_with_head(
                "Development",
                "",
                &hire(role("Manager"), "Development Manager", None, "claude-code"),
                "owner",
            )
            .unwrap();
        let (project, coordinator) = l
            .create_project_with_coordinator(
                &dept.id,
                &ProjectSettings {
                    name: "Cloudline".into(),
                    allowed_runtimes: vec!["claude-code".into(), "codex".into()],
                    ..ProjectSettings::default()
                },
                &hire(
                    role("Supervisor"),
                    "Cloudline Coordinator",
                    None,
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        let (developer, _) = l
            .create_position(
                &hire(
                    role("Senior Developer"),
                    "Senior Developer",
                    Some(&coordinator.id),
                    "codex",
                ),
                "owner",
            )
            .unwrap();
        let (qa, _) = l
            .create_position(
                &hire(
                    role("QA Engineer"),
                    "QA Engineer",
                    Some(&head.id),
                    "claude-code",
                ),
                "owner",
            )
            .unwrap();
        l.assign_oversight(OversightKind::Qa, &qa.id, &coordinator.id, "owner")
            .unwrap();
        Org {
            l,
            head: head.id,
            coordinator: coordinator.id,
            developer: developer.id,
            qa: qa.id,
            project: project.id,
        }
    }

    fn task(l: &Ledger, position: &str, agent: Option<&str>, objective: &str) -> Task {
        let mut workforce = json!({ "positionId": position });
        if let Some(a) = agent {
            workforce["agentId"] = json!(a);
        }
        l.create_task(
            NewTask {
                requested_by: "owner".into(),
                objective: objective.into(),
                metadata: json!({ "workforce": workforce, "sessionId": "s-1" }),
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
    }

    /// Delegate from the running `turn` to the Senior Developer position, recording the worker
    /// with the child task as Liaison does (the turn then waits).
    fn delegate(o: &Org, turn: &str, objective: &str) -> Task {
        use plenipo_ledger::{HandoffDecision, NewEvent, NewHandoffRequest, NewWorker};
        let records = o.l.org_records().unwrap();
        let dev = records
            .positions
            .iter()
            .find(|p| p.id == o.developer)
            .unwrap();
        let agent_id = uuid::Uuid::new_v4().to_string();
        let message_id = uuid::Uuid::new_v4().to_string();
        let suspended =
            o.l.suspend_for_handoffs(
                turn,
                NewEvent {
                    source: "agent:claude-code".into(),
                    event_type: "agent.result".into(),
                    payload: json!({ "step": 1 }),
                    ..NewEvent::default()
                },
                vec![NewHandoffRequest {
                    message_id: message_id.clone(),
                    correlation_id: "c-1".into(),
                    dedupe_key: format!("{turn}:1:x"),
                    source: "session:s-1".into(),
                    destination: "role:Senior Developer".into(),
                    envelope: json!({ "objective": objective }),
                    summary: json!({ "objective": objective }),
                    decision: HandoffDecision::Accept {
                        child: NewTask {
                            parent_task_id: Some(turn.into()),
                            requested_by: "agent:claude-code".into(),
                            assigned_to: Some("codex".into()),
                            objective: objective.into(),
                            metadata: json!({
                                "sessionId": "s-2",
                                "workforce": {
                                    "positionId": dev.id,
                                    "agentId": agent_id,
                                    "routing": { "reason": "Codex is the first choice." },
                                },
                            }),
                            ..NewTask::default()
                        },
                        received: json!({}),
                        worker: Some(Box::new(NewWorker {
                            agent_id,
                            position_id: dev.id.clone(),
                            role_id: dev.role_id.clone(),
                            runtime_id: "codex".into(),
                            runtime_provider: None,
                            model: None,
                            project_id: Some(o.project.clone()),
                            routing: json!({ "reason": "Codex is the first choice." }),
                        })),
                    },
                }],
                "waiting for 1 handoff reply",
                "liaison",
            )
            .unwrap();
        suspended.children[0].clone()
    }

    fn snapshot(l: &Arc<Ledger>, runtimes: &[AgentRuntimeInfo]) -> OrgSnapshot {
        let tools = runtimes.to_vec();
        let planner = Router::with_tools(Arc::clone(l), Arc::new(move || tools.clone()))
            .planner()
            .unwrap();
        let records = l.org_records().unwrap();
        let open = l.open_workforce_tasks().unwrap();
        let finished = l.finished_workforce_tasks(0, 1000).unwrap();
        build(&Inputs {
            records: &records,
            open_tasks: &open,
            finished_recent: &finished,
            sessions: &[],
            planner: &planner,
            learning: &crate::learning::LearningSettings::default(),
            name: "8 West".into(),
            titles: TitleTheme::Army,
            notices: vec![],
            places: &[],
            now: 1,
        })
    }

    fn position<'a>(s: &'a OrgSnapshot, id: &str) -> &'a PositionInfo {
        s.positions.iter().find(|p| p.id == id).unwrap()
    }

    #[test]
    fn the_tree_gives_every_position_its_department_project_and_team() {
        let o = org();
        let records = o.l.org_records().unwrap();
        let view = OrgView::new(&records);
        let titles = |ps: Vec<&Position>| ps.iter().map(|p| p.title.clone()).collect::<Vec<_>>();
        assert_eq!(
            titles(view.tree_order()),
            [
                "Development Manager",
                "Cloudline Coordinator",
                "Senior Developer",
                "QA Engineer"
            ]
        );
        assert_eq!(
            view.department_of(&o.developer).unwrap().name,
            "Development"
        );
        assert_eq!(view.project_of(&o.developer).unwrap().name, "Cloudline");
        assert!(
            view.project_of(&o.qa).is_none(),
            "the QA engineer is not in the project"
        );
        // The coordinator's team: its on-demand report and its QA overseer.
        let team: Vec<(String, bool)> = view
            .team(&o.coordinator)
            .iter()
            .map(|m| (m.position.title.clone(), m.oversight.is_some()))
            .collect();
        assert_eq!(
            team,
            [
                ("Senior Developer".into(), false),
                ("QA Engineer".into(), true)
            ]
        );
        // A worker's lead is its supervisor; a persistent position leads itself.
        assert_eq!(view.lead_of(&o.developer).unwrap().id, o.coordinator);
        assert_eq!(view.lead_of(&o.coordinator).unwrap().id, o.coordinator);
        // The manager's team: its on-demand QA engineer, then its full-time coordinator, which
        // takes work in its own conversation (Phase 8).
        assert_eq!(
            titles(view.team(&o.head).iter().map(|m| m.position).collect()),
            ["QA Engineer", "Cloudline Coordinator"]
        );
    }

    #[test]
    fn statuses_counts_and_workers_come_from_open_tasks() {
        let o = org();
        let runtimes = [runtime("claude-code", true), runtime("codex", true)];
        let idle = snapshot(&o.l, &runtimes);
        assert_eq!(idle.name, "8 West");
        assert_eq!(position(&idle, &o.coordinator).status, PositionStatus::Idle);
        assert_eq!(position(&idle, &o.developer).status, PositionStatus::Idle);
        assert_eq!(position(&idle, &o.developer).staffing, Staffing::OnDemand);
        assert_eq!(
            (
                idle.stats.departments,
                idle.stats.projects,
                idle.stats.positions
            ),
            (1, 1, 4)
        );
        assert_eq!((idle.stats.staffed, idle.stats.vacant), (2, 0));
        assert_eq!(idle.oversight.len(), 1);
        assert_eq!(idle.oversight[0].role, OversightRole::Qa);
        let coordinator = position(&idle, &o.coordinator);
        assert_eq!(coordinator.kind, PositionKind::ProjectCoordinator);
        assert_eq!(
            coordinator.coordinates_project_id.as_deref(),
            Some(o.project.as_str())
        );
        assert_eq!(coordinator.project_id.as_deref(), Some(o.project.as_str()));
        assert!(coordinator.agent.is_some());
        let dev_role = idle
            .roles
            .iter()
            .find(|r| r.name == "Senior Developer")
            .unwrap();
        assert!(dev_role.template && dev_role.glyph == "code" && !dev_role.purpose.is_empty());

        // The coordinator works, then delegates to a worker (recorded the way Liaison does):
        // it waits while the worker is queued, then running.
        let agent = position(&idle, &o.coordinator).agent.clone().unwrap();
        let turn = task(&o.l, &o.coordinator, Some(&agent.id), "Ship feature X");
        o.l.transition_task(&turn.id, TaskState::Running, "agent", None)
            .unwrap();
        let working = snapshot(&o.l, &runtimes);
        let coordinator = position(&working, &o.coordinator);
        assert_eq!(coordinator.status, PositionStatus::Working);
        assert_eq!(coordinator.current_task.as_ref().unwrap().id, turn.id);
        let child = delegate(&o, &turn.id, "Implement it\nwith tests");
        let busy = snapshot(&o.l, &runtimes);
        assert_eq!(
            position(&busy, &o.coordinator).status,
            PositionStatus::Waiting
        );
        let dev = position(&busy, &o.developer);
        assert_eq!(dev.status, PositionStatus::Queued);
        assert_eq!(dev.workers.len(), 1);
        assert_eq!(dev.workers[0].task_id, child.id);
        assert_eq!(dev.workers[0].objective, "Implement it", "first line");
        assert_eq!(
            dev.workers[0].parent_task_id.as_deref(),
            Some(turn.id.as_str())
        );
        assert_eq!(busy.stats.active_workers, 1);
        assert_eq!((busy.stats.waiting, busy.stats.queued), (1, 1));
        o.l.transition_task(&child.id, TaskState::Running, "agent", None)
            .unwrap();
        let running = snapshot(&o.l, &runtimes);
        assert_eq!(
            position(&running, &o.developer).status,
            PositionStatus::Working
        );
        assert_eq!(
            position(&running, &o.developer).workers[0].state,
            TaskState::Running
        );

        // When the worker's task ends, it leaves: history remains.
        o.l.transition_task(&child.id, TaskState::Succeeded, "agent", None)
            .unwrap();
        let after = snapshot(&o.l, &runtimes);
        let dev = position(&after, &o.developer);
        assert!(dev.workers.is_empty());
        assert_eq!(dev.history.retired, 1);
        assert_eq!(dev.status, PositionStatus::Idle);
        assert_eq!(after.stats.active_workers, 0);
        assert_eq!(after.stats.completed_24h, 1);
        assert_eq!(
            position(&after, &o.coordinator).status,
            PositionStatus::Waiting
        );
        let records = o.l.org_records().unwrap();

        // The work views: the coordinator's own task, and its team's (none unfinished now).
        let own = o.l.position_tasks(&o.coordinator, 100).unwrap();
        let open = o.l.open_workforce_tasks().unwrap();
        let work = work_view(&records, Some(&o.coordinator), &own, &open);
        assert_eq!(work.waiting.len(), 1);
        assert!(work.running.is_empty() && work.team.is_empty());
        let dev_own = o.l.position_tasks(&o.developer, 100).unwrap();
        let dev_work = work_view(&records, Some(&o.developer), &dev_own, &open);
        assert_eq!(dev_work.recent.len(), 1);
        assert_eq!(
            dev_work.recent[0].position_title.as_deref(),
            Some("Senior Developer")
        );
    }

    #[test]
    fn vacancies_unready_runtimes_and_project_policy_show_on_the_nodes() {
        let o = org();
        let runtimes = [runtime("claude-code", true), runtime("codex", false)];
        o.l.vacate_position(&o.coordinator, "owner").unwrap();
        let s = snapshot(&o.l, &runtimes);
        assert_eq!(position(&s, &o.coordinator).status, PositionStatus::Vacant);
        assert_eq!(s.stats.vacant, 1);
        let dev = position(&s, &o.developer);
        assert_eq!(dev.status, PositionStatus::Unavailable);
        assert!(dev
            .status_detail
            .as_deref()
            .unwrap()
            .contains("Codex is not signed in"));
        // The project no longer allows Codex: that is shown too, before readiness.
        o.l.update_project_settings(
            &o.project,
            &ProjectSettings {
                name: "Cloudline".into(),
                allowed_runtimes: vec!["claude-code".into()],
                ..ProjectSettings::default()
            },
            "owner",
        )
        .unwrap();
        let s = snapshot(
            &o.l,
            &[runtime("claude-code", true), runtime("codex", true)],
        );
        let dev = position(&s, &o.developer);
        assert_eq!(dev.status, PositionStatus::Unavailable);
        assert_eq!(
            dev.status_detail.as_deref(),
            Some("Cloudline does not allow Codex")
        );
        // Archived positions come last and say so.
        o.l.end_oversight(&o.l.org_records().unwrap().oversight[0].id, "owner")
            .unwrap();
        o.l.archive_position(&o.qa, "owner").unwrap();
        let s = snapshot(&o.l, &runtimes);
        let last = s.positions.last().unwrap();
        assert_eq!(
            (last.id.as_str(), last.status),
            (o.qa.as_str(), PositionStatus::Archived)
        );
        assert!(!last.active && last.archived_at.is_some());
        assert_eq!(s.stats.positions, 3);
    }

    #[test]
    fn experienced_means_above_the_exact_average_not_the_rounded_one() {
        // Scores 10 and 11: the average is 10.5, shown as 11.
        let average = Average { sum: 21, n: 2 };
        assert_eq!(average.shown(), 11);
        assert!(
            experience_info(1, 1, average).experienced,
            "11 is above 10.5"
        );
        assert!(!experience_info(1, 0, average).experienced, "10 is not");
        // Nobody has finished a task yet: any experience at all counts.
        assert!(experience_info(1, 0, Average::default()).experienced);
        assert!(!experience_info(0, 0, Average::default()).experienced);
    }
}
