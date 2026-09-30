//! Phase 16 Wave 3 commands: the Spending caps page (ADR-085, paid AI keys with spending caps).
//! Every one is the main window's alone (capabilities/default.json): the sign window and web
//! pages are refused.
//!
//! They read the month's spending and set or remove the owner's monthly caps. Nothing here
//! spends money, takes a key, or starts work: a paid task sets money aside itself, through the
//! Ledger, when it starts.

use std::sync::Arc;

use plenipo_capabilities::broker::Broker;
use plenipo_core::CommandError;
use plenipo_ledger::{now_ms, CapCovers, Ledger, SpendingPage};
use tauri::State;

use crate::commands::with_ledger;

/// Who changes a cap from this window.
const OWNER: &str = "owner";

/// This month's spending, each cap, and the month's latest paid tasks.
#[tauri::command]
pub async fn get_spending(ledger: State<'_, Arc<Ledger>>) -> Result<SpendingPage, CommandError> {
    with_ledger(&ledger, |l| l.spending_page(now_ms())).await
}

/// Set (or change) the monthly cap for the business, a department, or one position, in
/// millionths of a dollar.
#[tauri::command]
pub async fn set_spending_cap(
    ledger: State<'_, Arc<Ledger>>,
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

/// Remove a cap. The business's cap stays while a paid key is saved (ADR-085 §2.4).
#[tauri::command]
pub async fn remove_spending_cap(
    ledger: State<'_, Arc<Ledger>>,
    broker: State<'_, Broker>,
    cap_id: String,
) -> Result<SpendingPage, CommandError> {
    if cap_id.is_empty() || cap_id.len() > 64 {
        return Err(CommandError::invalid_input("invalid spending cap"));
    }
    let keys_saved = plenipo_capabilities::paid::any_key(&broker);
    with_ledger(&ledger, move |l| {
        l.remove_spending_cap(&cap_id, keys_saved, OWNER)?;
        l.spending_page(now_ms())
    })
    .await
}
