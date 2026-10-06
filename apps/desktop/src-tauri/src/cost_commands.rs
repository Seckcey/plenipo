//! I2 commands: what a task cost (the owner's "show the token cost after a task is complete"):
//! its tokens (pieces of words) over its runs, and its money on paid keys, alone or with every
//! task handed out beneath it. Both are the organization's window's alone
//! (capabilities/default.json): the sign window, a pop-out, and web pages are refused.
//!
//! They only read what each run and each paid request already recorded in the organization's
//! own Ledger. Nothing is stored, sent, or started.

use std::sync::Arc;

use plenipo_core::CommandError;
use plenipo_ledger::{Ledger, TaskCost, TaskTreeCost};

use crate::commands::{validate_task_id, with_ledger};
use crate::orgs::Org;

/// What one task cost: its own runs and paid requests.
#[tauri::command]
pub async fn get_task_cost(
    ledger: Org<'_, Arc<Ledger>>,
    task_id: String,
) -> Result<TaskCost, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| l.task_cost(&task_id)).await
}

/// What a task and every task handed out beneath it cost, each one and in all (at most
/// `TREE_MAX` tasks; `more` says when there were more).
#[tauri::command]
pub async fn get_task_tree_cost(
    ledger: Org<'_, Arc<Ledger>>,
    task_id: String,
) -> Result<TaskTreeCost, CommandError> {
    validate_task_id(&task_id)?;
    with_ledger(&ledger, move |l| l.task_tree_cost(&task_id)).await
}
