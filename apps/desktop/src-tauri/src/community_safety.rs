//! **Block**, **Unblock**, **Report**, and **Delete my Community data from this PC** in the app
//! (Phase 24, ADR-167, ADR-168 §3). Organizations' windows only (`capabilities/default.json`).

use std::sync::Arc;

use plenipo_community::block_report::{BlockedPerson, ReportOf};
use plenipo_community::service::Refused;
use plenipo_community::wire::ReportReason;
use plenipo_core::CommandError;
use plenipo_ledger::Ledger;
use serde::Deserialize;
use tauri::{AppHandle, Runtime, State};

use crate::community_host::CommunityState;
use crate::community_messages::messages_changed;

fn refused(r: Refused) -> CommandError {
    CommandError::invalid_input(r.0)
}

fn ledger(state: &CommunityState) -> Result<Arc<Ledger>, CommandError> {
    state
        .ledger()
        .ok_or_else(|| CommandError::invalid_input("Plenipo is still starting."))
}

/// **What is wrong?**, as the screen sends it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Spam,
    Harassment,
    Scam,
    Hate,
    Sexual,
    Under13,
    YoungPersonRisk,
    Impersonation,
    Cheating,
    Other,
}

impl From<Reason> for ReportReason {
    fn from(reason: Reason) -> Self {
        match reason {
            Reason::Spam => Self::Spam,
            Reason::Harassment => Self::Harassment,
            Reason::Scam => Self::Scam,
            Reason::Hate => Self::Hate,
            Reason::Sexual => Self::Sexual,
            Reason::Under13 => Self::Under13,
            Reason::YoungPersonRisk => Self::YoungPersonRisk,
            Reason::Impersonation => Self::Impersonation,
            Reason::Cheating => Self::Cheating,
            Reason::Other => Self::Other,
        }
    }
}

/// **Block** someone. They are not told.
#[tauri::command]
pub async fn block_in_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
    name: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let done = state.community.block(&ledger, &member_id, &name).await;
    messages_changed(&app);
    done.map_err(refused)
}

/// **Unblock** someone.
#[tauri::command]
pub async fn unblock_in_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let done = state.community.unblock(&ledger, &member_id).await;
    messages_changed(&app);
    done.map_err(refused)
}

/// Settings → Community → **Blocked**.
#[tauri::command]
pub async fn community_blocked(
    state: State<'_, Arc<CommunityState>>,
) -> Result<Vec<BlockedPerson>, CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    state.community.blocked(&ledger).await.map_err(refused)
}

/// **Report** a person, their profile, or messages they sent you (up to 20, with their proof).
/// `block`: block them too (ADR-167 §5), after the report is made.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn report_in_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
    name: String,
    of: ReportOf,
    reason: Reason,
    note: String,
    block: bool,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    state
        .community
        .report(&ledger, &member_id, &of, reason.into(), &note)
        .await
        .map_err(refused)?;
    if block {
        let done = state.community.block(&ledger, &member_id, &name).await;
        messages_changed(&app);
        done.map_err(refused)?;
    }
    Ok(())
}

/// **Delete my Community data from this PC**: every conversation and message here. The screen
/// asks first. Nothing is sent.
#[tauri::command]
pub async fn delete_my_community_data<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    state.community.delete_my_data(&ledger).map_err(refused)?;
    messages_changed(&app);
    Ok(())
}
