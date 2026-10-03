//! Rewards for taking part in the app (Phase 24, ADR-169, ADR-172): your points, the
//! leaderboard, and **Getting started**. Organizations' windows only
//! (`capabilities/default.json`).

use std::sync::Arc;

use plenipo_community::rewards::{GettingStarted, LeaderboardView, PointsView};
use plenipo_core::CommandError;
use tauri::State;

use crate::community_host::CommunityState;

/// Your points, places, badges, and "Thanked by" count.
#[tauri::command]
pub async fn community_points(
    state: State<'_, Arc<CommunityState>>,
) -> Result<PointsView, CommandError> {
    let state = state.inner().clone();
    state
        .community
        .points()
        .await
        .map_err(|r| CommandError::invalid_input(r.0))
}

/// The leaderboard: **This week** or **All time**.
#[tauri::command]
pub async fn community_leaderboard(
    state: State<'_, Arc<CommunityState>>,
    all_time: bool,
) -> Result<LeaderboardView, CommandError> {
    let state = state.inner().clone();
    state
        .community
        .leaderboard(all_time)
        .await
        .map_err(|r| CommandError::invalid_input(r.0))
}

/// **Getting started**, as this PC knows it. Nothing is sent.
#[tauri::command]
pub async fn community_getting_started(
    state: State<'_, Arc<CommunityState>>,
) -> Result<GettingStarted, CommandError> {
    Ok(state.community.getting_started())
}

/// Close **Getting started**. Nothing is sent.
#[tauri::command]
pub async fn close_community_getting_started(
    state: State<'_, Arc<CommunityState>>,
) -> Result<GettingStarted, CommandError> {
    state.community.close_getting_started();
    Ok(state.community.getting_started())
}
