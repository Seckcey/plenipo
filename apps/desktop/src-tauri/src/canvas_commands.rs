//! Phase 18 commands: the organization canvas — tile places, Tidy up, moving an oversight line's
//! end, and the live view (ADR-053) — move or lend (ADR-054), Watch (ADR-055), and the owner's
//! tile (ADR-056). Every one is the main window's alone (capabilities/default.json): the sign
//! window and web pages are refused.
//!
//! None of them touches files, programs, the network, the browser, or the screen. Watch only
//! reads what the capability broker already carried out for workers, after Guard; nothing it
//! offers writes to a working copy. The owner's picture arrives already shrunk by the window;
//! Plenipo never opens a file for it.

use plenipo_capabilities::watch::{WatchFileView, WatchView};
use plenipo_capabilities::{Broker, LiveView};
use plenipo_core::CommandError;
use plenipo_ledger::workforce::{MAX_PLACES, ORGANIZATION_TILE, OWNER_TILE};
use plenipo_ledger::{LoanUntil, TilePlace};
use plenipo_workforce::{OrgSnapshot, OwnerProfile, OwnerProfileInput, Workforce};
use tauri::State;

use crate::commands::{validate_id, validate_optional_id, with_broker, with_workforce};

fn validate_tile(id: &str) -> Result<(), CommandError> {
    if id == OWNER_TILE || id == ORGANIZATION_TILE {
        Ok(())
    } else {
        validate_id("tile", id)
    }
}

// ---- The canvas (ADR-053) -------------------------------------------------------------------

/// Save where the owner put these tiles (a whole team moved at once, at most 500).
#[tauri::command]
pub async fn place_tiles(
    workforce: State<'_, Workforce>,
    places: Vec<TilePlace>,
) -> Result<(), CommandError> {
    if places.len() > MAX_PLACES {
        return Err(CommandError::invalid_input(format!(
            "at most {MAX_PLACES} tiles can be placed at once"
        )));
    }
    places.iter().try_for_each(|p| validate_tile(&p.tile_id))?;
    with_workforce(&workforce, move |w| w.place_tiles(&places)).await
}

/// Tidy up: forget every place, so the automatic layout comes back. Returns what it forgot, for
/// Undo.
#[tauri::command]
pub async fn tidy_up(workforce: State<'_, Workforce>) -> Result<Vec<TilePlace>, CommandError> {
    with_workforce(&workforce, Workforce::tidy_up).await
}

/// Move one end of an oversight line: another overseer, or another team (never both unchanged).
#[tauri::command]
pub async fn retarget_oversight(
    workforce: State<'_, Workforce>,
    oversight_id: String,
    overseer_id: Option<String>,
    target_id: Option<String>,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("oversight assignment", &oversight_id)?;
    validate_optional_id("position", overseer_id.as_deref())?;
    validate_optional_id("position", target_id.as_deref())?;
    if overseer_id.is_none() && target_id.is_none() {
        return Err(CommandError::invalid_input(
            "name the new overseer or the new team",
        ));
    }
    with_workforce(&workforce, move |w| {
        w.retarget_oversight(&oversight_id, overseer_id.as_deref(), target_id.as_deref())
    })
    .await
}

/// Who is working where now, and the hand-offs of the last few minutes.
#[tauri::command]
pub async fn get_live_view(broker: State<'_, Broker>) -> Result<LiveView, CommandError> {
    with_broker(&broker, |b| Ok(b.live_view())).await
}

// ---- Move or lend (ADR-054) -----------------------------------------------------------------

/// Lend an on-call agent to another team's lead, for one objective or until it is sent home.
#[tauri::command]
pub async fn lend_agent(
    workforce: State<'_, Workforce>,
    position_id: String,
    to_lead_id: String,
    until: LoanUntil,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    validate_id("position", &to_lead_id)?;
    with_workforce(&workforce, move |w| {
        w.lend_agent(&position_id, &to_lead_id, until)
    })
    .await
}

/// Send a lent agent home: now, or when the task it is on ends.
#[tauri::command]
pub async fn send_home(
    workforce: State<'_, Workforce>,
    position_id: String,
) -> Result<OrgSnapshot, CommandError> {
    validate_id("position", &position_id)?;
    with_workforce(&workforce, move |w| w.send_home(&position_id)).await
}

// ---- Watch (ADR-055) — read-only ------------------------------------------------------------

/// What an agent's workers changed in its latest objective, each file once, newest first.
#[tauri::command]
pub async fn get_watch(
    broker: State<'_, Broker>,
    position_id: String,
) -> Result<WatchView, CommandError> {
    validate_id("position", &position_id)?;
    with_broker(&broker, move |b| Ok(b.watch_view(&position_id))).await
}

/// One change's file, its new and changed lines marked, or a summary; `null` when Plenipo no
/// longer has it.
#[tauri::command]
pub async fn get_watch_change(
    broker: State<'_, Broker>,
    change_id: String,
) -> Result<Option<WatchFileView>, CommandError> {
    validate_id("change", &change_id)?;
    with_broker(&broker, move |b| Ok(b.watch_change(&change_id))).await
}

// ---- The owner's tile (ADR-056) -------------------------------------------------------------

#[tauri::command]
pub async fn get_owner_profile(
    workforce: State<'_, Workforce>,
) -> Result<OwnerProfile, CommandError> {
    with_workforce(&workforce, |w| w.owner_profile()).await
}

/// Change the owner's status, mood, message, and picture (a PNG of at most 256 × 256 and 256 KB,
/// shrunk by the window).
#[tauri::command]
pub async fn set_owner_profile(
    workforce: State<'_, Workforce>,
    input: OwnerProfileInput,
) -> Result<OwnerProfile, CommandError> {
    with_workforce(&workforce, move |w| w.set_owner_profile(&input)).await
}
