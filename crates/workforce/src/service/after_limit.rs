//! When a plan runs out (Phase 25, item 4.2; ADR-253). An objective the owner gave whose AI tool
//! reached its usage limit failed. It waits, and Plenipo gives it to the same worker again once
//! the limit is over (the reset time the AI tool reported, an hour when it reported none, or the
//! owner's **Try again now**), unless the owner left it stopped. Plenipo never uses a usage reset
//! or buys anything: **Use a reset** opens the company's own page.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use plenipo_ledger::limit_stops::{LEFT_STOPPED, NOT_PICKED_UP, PICKED_UP};
use plenipo_ledger::{LimitStop, NewEvent, Task};
use plenipo_router::limits::LOOKBACK_MS;
use serde_json::json;

use super::{invalid, Workforce, PLENIPO};
use crate::dto::{LimitWait, LimitWaitWork};
use crate::error::{Result, WorkforceError};

/// The company whose plan an AI tool uses, when that company gives usage resets, and its own
/// page where the owner can use one. Only these two; Plenipo never opens another address here.
pub fn reset_page(runtime_id: &str) -> Option<(&'static str, &'static str)> {
    match runtime_id {
        "claude-code" => Some(("Anthropic", "https://claude.ai/settings/usage")),
        "codex" => Some(("OpenAI", "https://chatgpt.com/codex/settings/usage")),
        _ => None,
    }
}

/// How a stopped objective is given again, like the owner's **Run again** (Phase 13).
enum Again {
    /// The same position, with the objective as the owner gave it.
    Position {
        position_id: String,
        project_id: Option<String>,
        objective: String,
    },
    /// The same conversation (Workers).
    Conversation {
        session_id: String,
        objective: String,
    },
}

/// The objective as the owner gave it: without the project line Plenipo adds for the agent.
fn owners_objective(objective: &str) -> String {
    match objective.rsplit_once("\n\nProject: ") {
        Some((before, project)) if !project.contains('\n') => before.trim().to_owned(),
        _ => objective.trim().to_owned(),
    }
}

/// How `task` is given again; `None` for work that is not picked up by itself (a side chat, or
/// work Plenipo cannot place).
fn again(task: &Task) -> Option<Again> {
    let m = &task.metadata;
    if !m["sideChat"].is_null() || m["liaison"]["origin"].as_str() == Some("handoff") {
        return None;
    }
    let objective = owners_objective(&task.objective);
    if objective.is_empty() {
        return None;
    }
    if let Some(position_id) = m["workforce"]["positionId"].as_str() {
        return Some(Again::Position {
            position_id: position_id.to_owned(),
            project_id: task.project_id.clone(),
            objective,
        });
    }
    m["sessionId"]
        .as_str()
        .map(|session_id| Again::Conversation {
            session_id: session_id.to_owned(),
            objective,
        })
}

/// Clears the "picking up" flag when a pick-up ends, however it ends.
struct PickingUp<'a>(&'a std::sync::atomic::AtomicBool);

impl Drop for PickingUp<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Workforce {
    /// The objectives usage limits stopped and that wait, with what their AI tool's limit says.
    fn waiting(&self) -> Result<Vec<(LimitStop, Option<plenipo_router::UsageLimit>, String)>> {
        let now = plenipo_ledger::now_ms();
        let stops = self.ledger().limit_stops(now.saturating_sub(LOOKBACK_MS))?;
        if stops.is_empty() {
            return Ok(Vec::new());
        }
        let planner = self.inner.router.planner()?;
        Ok(stops
            .into_iter()
            .filter(|s| again(&s.task).is_some())
            .map(|s| {
                let tool = planner.tool(&s.runtime);
                let limit = tool.and_then(|t| t.limit.clone());
                let label = tool.map_or_else(|| s.runtime.clone(), |t| t.info.label.clone());
                (s, limit, label)
            })
            .collect())
    }

    /// The work usage limits stopped, for each AI tool: the notice with the owner's choices.
    pub fn limit_waits(&self) -> Result<Vec<LimitWait>> {
        let waiting = self.waiting()?;
        if waiting.is_empty() {
            return Ok(Vec::new());
        }
        let records = self.ledger().org_records()?;
        let mut by_tool: BTreeMap<String, LimitWait> = BTreeMap::new();
        for (stop, limit, label) in waiting {
            let who = stop.task.metadata["workforce"]["positionId"]
                .as_str()
                .and_then(|id| records.positions.iter().find(|p| p.id == id))
                .map(|p| p.title.clone());
            let wait = by_tool
                .entry(stop.runtime.clone())
                .or_insert_with(|| LimitWait {
                    runtime_id: stop.runtime.clone(),
                    label,
                    reset_company: reset_page(&stop.runtime).map(|(c, _)| c.to_owned()),
                    since: limit.as_ref().map_or(stop.at, |l| l.since),
                    until: limit.as_ref().map(|l| l.until),
                    reported: limit.as_ref().is_some_and(|l| l.resets_at.is_some()),
                    work: Vec::new(),
                });
            wait.work.push(LimitWaitWork {
                task_id: stop.task.id.clone(),
                objective: owners_objective(&stop.task.objective),
                who,
            });
        }
        Ok(by_tool.into_values().collect())
    }

    /// Give each waiting objective whose AI tool's limit is over to its worker again, and record
    /// it; one that cannot be given again is recorded with the reason and not tried again.
    /// Nothing while Stop all work holds the work, or while another pick-up runs. Returns how
    /// many were picked up.
    pub async fn pick_up_after_limits(&self) -> Result<usize> {
        if self.inner.runtime.work_held()
            || self
                .inner
                .picking_up
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return Ok(0);
        }
        let _done = PickingUp(&self.inner.picking_up);
        let this = self.clone();
        let due: Vec<(Task, String, String)> = tokio::task::spawn_blocking(move || {
            this.waiting().map(|w| {
                w.into_iter()
                    .filter(|(_, limit, _)| limit.is_none())
                    .map(|(s, _, label)| (s.task, s.runtime, label))
                    .collect()
            })
        })
        .await
        .map_err(|e| WorkforceError::Internal(e.to_string()))??;
        let mut picked = 0;
        for (task, runtime, label) in due {
            if self.inner.runtime.work_held() {
                break;
            }
            let Some(how) = again(&task) else { continue };
            let given = match how {
                Again::Position {
                    position_id,
                    project_id,
                    objective,
                } => {
                    self.give_objective(&position_id, &objective, project_id.as_deref())
                        .await
                }
                Again::Conversation {
                    session_id,
                    objective,
                } => self
                    .inner
                    .liaison
                    .resume_session(&session_id, &objective)
                    .await
                    .map_err(WorkforceError::from),
            };
            let (kind, payload) = match given {
                Ok(detail) => {
                    picked += 1;
                    (
                        PICKED_UP,
                        json!({
                            "runAgainAs": detail.turns.last().map(|t| t.task_id.clone()),
                            "runtimeId": runtime,
                            "label": label,
                        }),
                    )
                }
                Err(e) => (
                    NOT_PICKED_UP,
                    json!({ "reason": e.to_string(), "runtimeId": runtime, "label": label }),
                ),
            };
            self.ledger().append_event(NewEvent {
                task_id: Some(task.id.clone()),
                source: PLENIPO.into(),
                event_type: kind.into(),
                payload,
                ..NewEvent::default()
            })?;
        }
        Ok(picked)
    }

    /// The owner used a usage reset (or wants to try now): `runtime_id`'s limit is cleared, like
    /// **Try again now**, and its waiting work is picked up at once.
    pub async fn pick_up_now(&self, runtime_id: &str) -> Result<usize> {
        self.inner.router.clear_limit(runtime_id)?;
        self.pick_up_after_limits().await
    }

    /// The owner left these waiting objectives stopped: they are never picked up by themselves.
    /// Only work that is waiting can be.
    pub fn leave_stopped(&self, task_ids: &[String]) -> Result<Vec<LimitWait>> {
        let waiting: Vec<String> = self
            .waiting()?
            .into_iter()
            .map(|(s, _, _)| s.task.id)
            .collect();
        if let Some(id) = task_ids.iter().find(|id| !waiting.contains(id)) {
            return Err(invalid(format!(
                "{id} is not waiting for a usage limit; nothing was changed"
            )));
        }
        for id in task_ids {
            self.ledger().append_event(NewEvent {
                task_id: Some(id.clone()),
                source: super::OWNER.into(),
                event_type: LEFT_STOPPED.into(),
                ..NewEvent::default()
            })?;
        }
        self.limit_waits()
    }
}

#[cfg(test)]
mod tests {
    use plenipo_ledger::TaskState;

    use super::*;

    fn task(objective: &str, metadata: serde_json::Value) -> Task {
        Task {
            id: "t-1".into(),
            parent_task_id: None,
            requested_by: "owner".into(),
            assigned_to: None,
            project_id: Some("pr-1".into()),
            objective: objective.into(),
            acceptance_criteria: String::new(),
            priority: 2,
            state: TaskState::Failed,
            metadata,
            created_at: 1,
            updated_at: 1,
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn stopped_work_goes_back_to_the_same_worker_but_never_a_side_chat() {
        let member = task(
            "Fix the login page\n\nProject: Website (its supervisor: role:Web Supervisor).",
            json!({ "sessionId": "s-1", "workforce": { "positionId": "p-1" } }),
        );
        match again(&member) {
            Some(Again::Position {
                position_id,
                project_id,
                objective,
            }) => {
                assert_eq!(position_id, "p-1");
                assert_eq!(project_id.as_deref(), Some("pr-1"));
                assert_eq!(objective, "Fix the login page");
            }
            _ => panic!("a member's objective goes back to its position"),
        }
        assert!(matches!(
            again(&task("Say hi", json!({ "sessionId": "s-2" }))),
            Some(Again::Conversation { .. })
        ));
        let side = json!({ "sessionId": "s-3", "sideChat": { "positionId": "p-1" } });
        assert!(again(&task("A question", side)).is_none());
        assert!(again(&task("Lost", json!({}))).is_none());
        assert_eq!(reset_page("claude-code").unwrap().0, "Anthropic");
        assert_eq!(reset_page("codex").unwrap().0, "OpenAI");
        assert!(reset_page("grok").is_none());
    }
}
