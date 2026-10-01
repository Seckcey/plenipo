//! Phase 16 Wave 3 commands: the Spending caps page (ADR-085, paid AI keys with spending caps).
//! Every one is an organization's window's alone (capabilities/default.json): the sign window
//! and web pages are refused. Each organization has its own caps and spending, in its own
//! Ledger (Phase 21, ADR-094).
//!
//! They read the month's spending and set or remove the owner's monthly caps. Nothing here
//! spends money, takes a key, or starts work: a paid task sets money aside itself, through the
//! Ledger, when it starts.

use std::sync::Arc;

use plenipo_core::CommandError;
use plenipo_ledger::{now_ms, CapCovers, Ledger, SpendingPage};
use plenipo_runtime::agent::AgentRuntime;

use crate::commands::with_ledger;
use crate::orgs::Org;

/// Who changes a cap from this window.
const OWNER: &str = "owner";

/// This month's spending, each cap, and the month's latest paid tasks.
#[tauri::command]
pub async fn get_spending(ledger: Org<'_, Arc<Ledger>>) -> Result<SpendingPage, CommandError> {
    with_ledger(&ledger, |l| l.spending_page(now_ms())).await
}

/// Set (or change) the monthly cap for the business, a department, or one position, in
/// millionths of a dollar. A cap is the owner's choice: none is needed for a paid key.
#[tauri::command]
pub async fn set_spending_cap(
    ledger: Org<'_, Arc<Ledger>>,
    covers: CapCovers,
    monthly_micros: u64,
) -> Result<SpendingPage, CommandError> {
    with_ledger(&ledger, move |l| {
        let now = now_ms();
        l.set_spending_cap(&covers, monthly_micros, OWNER, now)?;
        l.spending_page(now)
    })
    .await
}

/// Check the paid AI tools again, in the background, after the paid-keys switch changed: their
/// cards and the Router say so at once (ADR-085). A check while paid keys cannot be used starts
/// nothing.
pub(crate) fn recheck_paid_tools(agents: &AgentRuntime) {
    for adapter in plenipo_runtime::agent::builtin_adapters() {
        if adapter.paid() {
            let agents = agents.clone();
            let id = adapter.id().to_owned();
            tauri::async_runtime::spawn(async move {
                let _ = agents.recheck(&id).await;
            });
        }
    }
}

/// Remove a cap, the business's included: with no cap, paid work has no dollar limit.
#[tauri::command]
pub async fn remove_spending_cap(
    ledger: Org<'_, Arc<Ledger>>,
    cap_id: String,
) -> Result<SpendingPage, CommandError> {
    if cap_id.is_empty() || cap_id.len() > 64 {
        return Err(CommandError::invalid_input("invalid spending cap"));
    }
    with_ledger(&ledger, move |l| {
        l.remove_spending_cap(&cap_id, OWNER)?;
        l.spending_page(now_ms())
    })
    .await
}
