//! Settings → Community, and the switch **Community** (Phase 24, ADR-162, ADR-170).
//! Organizations' windows only (`capabilities/default.json`): a pop-out, the sign, and web pages
//! cannot call them. Turning Community on and signing in reach 8 West, so they happen only when
//! the owner presses them here (ADR-115, ADR-170 §2).

use std::sync::Arc;

use plenipo_community::profile::ProfileDraft;
use plenipo_community::service::{CommunityView, Refused, Stage};
use plenipo_core::CommandError;
use serde::Deserialize;
use tauri::{AppHandle, Runtime, State};

use crate::community_host::{self, CommunityState, TERMS_PAGE};

/// The longest Community name (contract §3).
const MOST_NAME_CHARS: usize = 30;
/// The longest terms version.
const MOST_TERMS_CHARS: usize = 40;

fn refused(r: Refused) -> CommandError {
    CommandError::invalid_input(r.0)
}

/// After a step: tell every window, and give Settings → Community as it is now.
fn after<R: Runtime>(app: &AppHandle<R>, state: &Arc<CommunityState>) -> CommunityView {
    community_host::changed(app);
    state.view()
}

/// Settings → Community, as it is.
#[tauri::command]
pub async fn get_community(
    state: State<'_, Arc<CommunityState>>,
) -> Result<CommunityView, CommandError> {
    Ok(state.view())
}

/// Settings → Switches → **Community**. On asks 8 West whether Community is open and, if it is,
/// shows a code to sign in with (ADR-170 §2, ADR-162 §2). Off is **Leave Community** (ADR-167 §15).
#[tauri::command]
pub async fn set_community_switch<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    on: bool,
) -> Result<CommunityView, CommandError> {
    let state = state.inner().clone();
    if on {
        state.community.turn_on().await.map_err(refused)?;
        if state.view().stage == Stage::SigningIn {
            community_host::ask_until_signed_in(&app, &state);
        }
    } else {
        state.community.leave().await.map_err(refused)?;
    }
    Ok(after(&app, &state))
}

/// **Check again** (after Coming soon) and **Sign in** (after signing out): the same as pressing
/// the switch.
#[tauri::command]
pub async fn check_community_again<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
) -> Result<CommunityView, CommandError> {
    set_community_switch(app, state, true).await
}

/// Stop signing in: nothing was kept.
#[tauri::command]
pub async fn cancel_community_sign_in<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
) -> Result<CommunityView, CommandError> {
    let state = state.inner().clone();
    state.community.cancel_sign_in();
    Ok(after(&app, &state))
}

/// Join Community: the Community name, the birth month and year, the terms version the owner
/// accepted on screen, and the profile with what people see (ADR-163 §2). Under 13 sends nothing
/// (ADR-162 §4).
#[tauri::command]
pub async fn join_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    name: String,
    birth_month: u8,
    birth_year: u16,
    terms: String,
    profile: ProfileDraft,
) -> Result<CommunityView, CommandError> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > MOST_NAME_CHARS {
        return Err(CommandError::invalid_input(
            "Choose a Community name of 3 to 30 letters, numbers, or dashes.",
        ));
    }
    if terms.is_empty()
        || terms.chars().count() > MOST_TERMS_CHARS
        || terms.chars().any(char::is_control)
    {
        return Err(CommandError::invalid_input(
            "Read and accept the Community terms.",
        ));
    }
    let state = state.inner().clone();
    let result = state
        .community
        .join(
            &name,
            birth_month,
            birth_year,
            &terms,
            &profile,
            &state.tile(),
        )
        .await;
    community_host::changed(&app);
    result.map_err(refused)?;
    Ok(state.view())
}

/// **Save** your profile and what people see (ADR-163 §2). An unticked part is hidden at once.
#[tauri::command]
pub async fn save_community_profile<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    profile: ProfileDraft,
) -> Result<CommunityView, CommandError> {
    let state = state.inner().clone();
    let tile = state.tile();
    let result = state.community.save_profile(&profile, &tile).await;
    community_host::changed(&app);
    result.map_err(refused)?;
    Ok(state.view())
}

/// **Appear offline**, or not (ADR-163 §5).
#[tauri::command]
pub async fn set_community_appear_offline<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    offline: bool,
) -> Result<CommunityView, CommandError> {
    let state = state.inner().clone();
    state
        .community
        .set_appear_offline(offline)
        .await
        .map_err(refused)?;
    Ok(after(&app, &state))
}

/// **Sign out of your account** on this PC. Membership stays.
#[tauri::command]
pub async fn sign_out_of_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
) -> Result<CommunityView, CommandError> {
    let state = state.inner().clone();
    state.community.sign_out().await.map_err(refused)?;
    Ok(after(&app, &state))
}

/// Which page to open in the owner's own browser.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommunityPage {
    /// The account site's page where the owner types the code.
    SignIn,
    /// The published Community terms.
    Terms,
}

/// **Open the sign-in page**, or the terms, in the owner's own browser: only these two
/// addresses, built into Plenipo, never one from an answer.
#[tauri::command]
pub async fn open_community_page(
    state: State<'_, Arc<CommunityState>>,
    page: CommunityPage,
) -> Result<(), CommandError> {
    let address = match page {
        CommunityPage::SignIn => state.sign_in_page(),
        CommunityPage::Terms => TERMS_PAGE.to_owned(),
    };
    state.opener.open(address).await.map_err(|why| {
        CommandError::invalid_input(format!("Plenipo couldn't open your web browser: {why}"))
    })
}
