//! What a task cost (I2, the owner's "show the token cost after a task is complete"): its tokens
//! (pieces of words) over its runs of an AI tool, and its money on paid keys, for one task, or for
//! it and every task handed out beneath it. Read-only: it adds up what each run and each paid
//! request already recorded (`executions.usage_metadata`, the `spending` rows), with the tree from
//! the tasks' own `parent_task_id` and each task's position from its own record. Nothing new is
//! stored.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Ledger, LedgerError, Result};

/// The most tasks a tree's cost reads; past it, `more` says so.
pub const TREE_MAX: usize = 200;

/// How a task's money was priced, over its paid requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CostPricedBy {
    /// Each bill was the service's own (OpenRouter reports it).
    Service,
    /// Each was priced from Plenipo's dated price list.
    PriceList,
    /// Some one way, some the other.
    Both,
}

/// What one task cost, or a tree of tasks added up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskCost {
    pub task_id: String,
    /// Tokens it read (the reused ones included), over the runs that reported them.
    #[ts(type = "number")]
    pub read: u64,
    /// Of those read, how many came from the AI company's cache.
    #[ts(type = "number")]
    pub reused: u64,
    /// Tokens it wrote.
    #[ts(type = "number")]
    pub written: u64,
    /// Its runs of an AI tool (one a step), and how many of them reported tokens.
    pub runs: u32,
    pub counted: u32,
    /// A run ended early (stopped, failed, timed out) without reporting: it used at least these.
    pub at_least: bool,
    /// What its paid requests cost by their bills, in millionths of a dollar.
    #[ts(type = "number")]
    pub spent_micros: u64,
    /// Set aside for its paid requests still going (the most each could cost).
    #[ts(type = "number")]
    pub set_aside_micros: u64,
    /// Paid requests whose bill could not be read, counted at the most they could cost.
    pub not_priced: u32,
    #[ts(type = "number")]
    pub not_priced_micros: u64,
    /// How its money was priced; `None` without a paid request that was priced.
    pub priced_by: Option<CostPricedBy>,
    /// A run of it is still going.
    pub running: bool,
}

/// One task of a tree's cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskCostPart {
    pub task_id: String,
    /// The position that did it, from the task's own record; `None` for one outside the
    /// organization (or a position no longer in the Ledger).
    pub position_title: Option<String>,
    /// The AI tool of its newest run.
    pub runtime: Option<String>,
    pub cost: TaskCost,
}

/// A task and every task handed out beneath it, each one's cost and all of them added up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskTreeCost {
    pub task_id: String,
    /// Everything below added up.
    pub total: TaskCost,
    /// The task first, then those handed out beneath it, depth-first, in the order they began.
    pub parts: Vec<TaskCostPart>,
    /// It has more than `TREE_MAX` tasks: the rest are not counted.
    pub more: bool,
}

impl TaskCost {
    fn add(&mut self, other: &TaskCost) {
        self.read += other.read;
        self.reused += other.reused;
        self.written += other.written;
        self.runs += other.runs;
        self.counted += other.counted;
        self.at_least |= other.at_least;
        self.spent_micros += other.spent_micros;
        self.set_aside_micros += other.set_aside_micros;
        self.not_priced += other.not_priced;
        self.not_priced_micros += other.not_priced_micros;
        self.priced_by = joined(self.priced_by, other.priced_by);
        self.running |= other.running;
    }
}

fn joined(a: Option<CostPricedBy>, b: Option<CostPricedBy>) -> Option<CostPricedBy> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) if x == y => Some(x),
        _ => Some(CostPricedBy::Both),
    }
}

impl Ledger {
    /// What `task_id` cost: its own runs and paid requests (not the tasks it handed out).
    pub fn task_cost(&self, task_id: &str) -> Result<TaskCost> {
        self.read(|c| {
            exists(c, task_id)?;
            cost_of(c, task_id)
        })
    }

    /// What `task_id` and every task handed out beneath it cost, each one and in all.
    pub fn task_tree_cost(&self, task_id: &str) -> Result<TaskTreeCost> {
        self.read(|c| {
            exists(c, task_id)?;
            let (ids, more) = tree(c, task_id)?;
            let mut total = TaskCost {
                task_id: task_id.to_owned(),
                ..TaskCost::default()
            };
            let mut parts = Vec::with_capacity(ids.len());
            for id in ids {
                let cost = cost_of(c, &id)?;
                total.add(&cost);
                parts.push(TaskCostPart {
                    position_title: title_of(c, &id)?,
                    runtime: runtime_of(c, &id)?,
                    task_id: id,
                    cost,
                });
            }
            Ok(TaskTreeCost {
                task_id: task_id.to_owned(),
                total,
                parts,
                more,
            })
        })
    }
}

fn exists(c: &Connection, task_id: &str) -> Result<()> {
    let found: Option<String> = c
        .query_row("SELECT id FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })
        .optional()?;
    found
        .map(|_| ())
        .ok_or_else(|| LedgerError::NotFound(format!("task {task_id}")))
}

/// A run's state that ended it early: what it used may not have been reported.
fn ended_early(state: &str) -> bool {
    matches!(state, "cancelled" | "interrupted" | "timedOut" | "failed")
}

fn count(n: Option<i64>) -> u64 {
    n.and_then(|v| u64::try_from(v).ok()).unwrap_or(0)
}

fn cost_of(c: &Connection, task_id: &str) -> Result<TaskCost> {
    let mut cost = TaskCost {
        task_id: task_id.to_owned(),
        ..TaskCost::default()
    };
    // An AI tool's run records `usage` (an object, or null when it reported none); a local
    // program's run records no `usage` at all, and is not counted.
    let mut runs = c.prepare(
        "SELECT state, json_type(usage_metadata, '$.usage'),
                json_extract(usage_metadata, '$.usage.inputTokens'),
                json_extract(usage_metadata, '$.usage.cachedInputTokens'),
                json_extract(usage_metadata, '$.usage.outputTokens')
         FROM executions WHERE task_id = ?1",
    )?;
    let rows = runs.query_map([task_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut early_without = false;
    for row in rows {
        let (state, kind, read, reused, written) = row?;
        if state == "starting" || state == "running" {
            cost.running = true;
        }
        let Some(kind) = kind else { continue };
        cost.runs += 1;
        if kind == "object" {
            cost.counted += 1;
            cost.read += count(read);
            cost.reused += count(reused);
            cost.written += count(written);
        } else if ended_early(&state) {
            early_without = true;
        }
    }
    cost.at_least = early_without && cost.counted > 0;

    let mut paid = c.prepare(
        "SELECT state, set_aside_micros, spent_micros, priced_by FROM spending WHERE task_id = ?1",
    )?;
    let rows = paid.query_map([task_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (state, set_aside, spent, priced_by) = row?;
        match state.as_str() {
            "spent" => {
                cost.spent_micros += count(spent);
                let by = match priced_by.as_deref() {
                    Some("service") => Some(CostPricedBy::Service),
                    Some("priceList") => Some(CostPricedBy::PriceList),
                    _ => None,
                };
                cost.priced_by = joined(cost.priced_by, by);
            }
            "setAside" => cost.set_aside_micros += count(Some(set_aside)),
            "notPriced" => {
                cost.not_priced += 1;
                cost.not_priced_micros += count(Some(set_aside));
            }
            // Released: the request was never sent, so nothing was spent.
            _ => {}
        }
    }
    Ok(cost)
}

/// The task and those handed out beneath it, depth-first in the order they began; at most
/// `TREE_MAX`, and whether there were more.
fn tree(c: &Connection, root: &str) -> Result<(Vec<String>, bool)> {
    let mut children =
        c.prepare("SELECT id FROM tasks WHERE parent_task_id = ?1 ORDER BY created_at, id")?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![root.to_owned()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if out.len() == TREE_MAX {
            return Ok((out, true));
        }
        let kids = children
            .query_map([&id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        out.push(id);
        stack.extend(kids.into_iter().rev());
    }
    Ok((out, false))
}

/// The title of the position a task's own record names (`metadata.workforce.positionId`).
fn title_of(c: &Connection, task_id: &str) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT p.title FROM tasks t
         JOIN positions p ON p.id = json_extract(t.metadata, '$.workforce.positionId')
         WHERE t.id = ?1",
        [task_id],
        |r| r.get(0),
    )
    .optional()?)
}

/// The AI tool of a task's newest run.
fn runtime_of(c: &Connection, task_id: &str) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT runtime FROM executions WHERE task_id = ?1 ORDER BY started_at DESC, id DESC
         LIMIT 1",
        [task_id],
        |r| r.get(0),
    )
    .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn ledger() -> Ledger {
        Ledger::open_in_memory().unwrap()
    }

    fn task(l: &Ledger, id: &str, parent: Option<&str>, at: i64, metadata: Value) {
        l.conn()
            .execute(
                "INSERT INTO tasks (id, parent_task_id, requested_by, objective, state, metadata,
                                    created_at, updated_at)
                 VALUES (?1, ?2, 'owner', 'Do it', 'succeeded', ?3, ?4, ?4)",
                rusqlite::params![id, parent, metadata.to_string(), at],
            )
            .unwrap();
    }

    /// A run of an AI tool (`usage`: its report, or null), or a local program's (`None`).
    fn run(l: &Ledger, id: &str, task: &str, state: &str, usage: Option<Value>, at: i64) {
        let meta = match usage {
            Some(usage) => json!({ "providerSessionId": "p", "usage": usage }),
            None => Value::Null,
        };
        l.conn()
            .execute(
                "INSERT INTO executions (id, task_id, runtime, state, started_at, usage_metadata)
                 VALUES (?1, ?2, 'codex', ?3, ?4, ?5)",
                rusqlite::params![id, task, state, at, meta.to_string()],
            )
            .unwrap();
    }

    fn used(read: u64, reused: u64, written: u64) -> Value {
        json!({ "inputTokens": read, "cachedInputTokens": reused, "outputTokens": written })
    }

    fn paid(
        l: &Ledger,
        id: &str,
        task: &str,
        state: &str,
        set_aside: i64,
        spent: Option<(i64, &str)>,
    ) {
        let settled = (state != "setAside").then_some(5_i64);
        l.conn()
            .execute(
                "INSERT INTO spending (id, task_id, runtime, model, month, state, set_aside_micros,
                                       spent_micros, priced_by, created_at, settled_at)
                 VALUES (?1, ?2, 'openrouter', 'm', '2026-10', ?3, ?4, ?5, ?6, 1, ?7)",
                rusqlite::params![
                    id,
                    task,
                    state,
                    set_aside,
                    spent.map(|(s, _)| s),
                    spent.map(|(_, by)| by),
                    settled
                ],
            )
            .unwrap();
    }

    #[test]
    fn a_tasks_cost_adds_up_its_runs_and_its_paid_requests() {
        let l = ledger();
        task(&l, "t", None, 1, json!({}));
        run(&l, "e1", "t", "succeeded", Some(used(1_000, 600, 100)), 1);
        run(&l, "e2", "t", "succeeded", Some(used(500, 0, 50)), 2);
        // A local program's run is not an AI tool's: not counted.
        run(&l, "e3", "t", "succeeded", None, 3);
        paid(&l, "s1", "t", "spent", 9_000, Some((1_200, "priceList")));
        paid(&l, "s2", "t", "spent", 9_000, Some((800, "service")));
        paid(&l, "s3", "t", "released", 9_000, None);
        let cost = l.task_cost("t").unwrap();
        assert_eq!((cost.read, cost.reused, cost.written), (1_500, 600, 150));
        assert_eq!((cost.runs, cost.counted), (2, 2));
        assert!(!cost.at_least && !cost.running);
        assert_eq!(cost.spent_micros, 2_000);
        assert_eq!(cost.priced_by, Some(CostPricedBy::Both));
        assert_eq!((cost.set_aside_micros, cost.not_priced), (0, 0));
    }

    #[test]
    fn a_run_stopped_before_it_reported_makes_it_at_least_and_one_running_says_so() {
        let l = ledger();
        task(&l, "t", None, 1, json!({}));
        run(&l, "e1", "t", "succeeded", Some(used(100, 0, 10)), 1);
        run(&l, "e2", "t", "cancelled", Some(Value::Null), 2);
        run(&l, "e3", "t", "running", Some(Value::Null), 3);
        paid(&l, "s1", "t", "setAside", 4_000, None);
        paid(&l, "s2", "t", "notPriced", 3_000, None);
        let cost = l.task_cost("t").unwrap();
        assert!(cost.at_least && cost.running);
        assert_eq!((cost.runs, cost.counted), (3, 1));
        assert_eq!(cost.set_aside_micros, 4_000);
        assert_eq!((cost.not_priced, cost.not_priced_micros), (1, 3_000));
        assert_eq!(cost.priced_by, None);
        // A tool that reports no tokens at all (Kimi): nothing counted, and not "at least".
        task(&l, "k", None, 1, json!({}));
        run(&l, "e4", "k", "succeeded", Some(Value::Null), 1);
        let kimi = l.task_cost("k").unwrap();
        assert_eq!((kimi.runs, kimi.counted, kimi.at_least), (1, 0, false));
    }

    #[test]
    fn a_trees_cost_walks_the_tasks_handed_out_with_each_ones_position() {
        let l = ledger();
        l.conn()
            .execute_batch(
                "INSERT INTO roles (id, name, role_type, persistent, created_at)
                     VALUES ('r', 'Developer', 'worker', 0, 1);
                 INSERT INTO positions (id, title, role_id, runtime_id, state, created_at, updated_at)
                     VALUES ('p-dev', 'Senior Developer', 'r', 'codex', 'active', 1, 1);",
            )
            .unwrap();
        task(&l, "lead", None, 1, json!({}));
        task(
            &l,
            "a",
            Some("lead"),
            2,
            json!({ "workforce": { "positionId": "p-dev" } }),
        );
        task(&l, "a1", Some("a"), 3, json!({}));
        task(&l, "b", Some("lead"), 4, json!({}));
        run(&l, "e1", "lead", "succeeded", Some(used(100, 0, 10)), 1);
        run(&l, "e2", "a", "succeeded", Some(used(200, 50, 20)), 2);
        run(&l, "e3", "a1", "succeeded", Some(used(300, 0, 30)), 3);
        run(&l, "e4", "b", "succeeded", Some(used(400, 0, 40)), 4);
        paid(&l, "s1", "b", "spent", 9_000, Some((700, "service")));

        let tree = l.task_tree_cost("lead").unwrap();
        // Depth-first, in the order they began.
        let order: Vec<_> = tree.parts.iter().map(|p| p.task_id.as_str()).collect();
        assert_eq!(order, ["lead", "a", "a1", "b"]);
        assert_eq!(
            tree.parts[1].position_title.as_deref(),
            Some("Senior Developer")
        );
        assert_eq!(tree.parts[0].position_title, None);
        assert_eq!(tree.parts[0].runtime.as_deref(), Some("codex"));
        assert_eq!(
            (tree.total.read, tree.total.reused, tree.total.written),
            (1_000, 50, 100)
        );
        assert_eq!(tree.total.spent_micros, 700);
        assert_eq!(tree.total.priced_by, Some(CostPricedBy::Service));
        assert!(!tree.more);
        // A task alone is its own runs only.
        assert_eq!(l.task_cost("lead").unwrap().read, 100);
        assert!(matches!(l.task_cost("nope"), Err(LedgerError::NotFound(_))));
        assert!(matches!(
            l.task_tree_cost("nope"),
            Err(LedgerError::NotFound(_))
        ));
    }

    #[test]
    fn a_tree_past_its_limit_says_there_is_more() {
        let l = ledger();
        task(&l, "root", None, 0, json!({}));
        for i in 0..TREE_MAX {
            let id = format!("c{i:03}");
            task(&l, &id, Some("root"), 1 + i as i64, json!({}));
        }
        let tree = l.task_tree_cost("root").unwrap();
        assert_eq!(tree.parts.len(), TREE_MAX);
        assert!(tree.more);
    }
}
