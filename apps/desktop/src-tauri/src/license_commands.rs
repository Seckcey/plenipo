//! Settings → License (Phase 11A, ADR-021, ADR-022): the license on this PC, entering and
//! removing a key, and checking with 8 West now. Organizations' windows only
//! (`capabilities/default.json`); a pop-out, the sign, and web pages cannot call them.
//!
//! The key is typed only here, kept in the Vault, and never shown again, sent, or recorded: the
//! screen and the Activity trail see its ID.

use std::sync::Arc;

use plenipo_core::CommandError;
use plenipo_licensing::LicenseView;
use tauri::{AppHandle, Runtime, State};

use crate::license_host::{self, LicenseHost, MAX_KEY_BYTES};
use crate::orgs::{OrgStack, Orgs};

fn first(orgs: &Orgs) -> Result<Arc<OrgStack>, CommandError> {
    orgs.first()
        .ok_or_else(|| CommandError::internal("the first organization is not open"))
}

/// The license on this PC: Free or Pro, why, and the last check (never the key).
#[tauri::command]
pub fn get_license(host: State<'_, Arc<LicenseHost>>) -> LicenseView {
    host.view()
}

/// Enter a license key: checked at once on this PC, kept in the Vault, and Pro from now. The
/// first check with 8 West starts by itself.
#[tauri::command]
pub async fn enter_license_key<R: Runtime>(
    app: AppHandle<R>,
    host: State<'_, Arc<LicenseHost>>,
    orgs: State<'_, Arc<Orgs>>,
    key: String,
) -> Result<LicenseView, CommandError> {
    if key.len() > MAX_KEY_BYTES {
        return Err(CommandError::invalid_input(
            "That is too long to be a Plenipo license key.",
        ));
    }
    let first = first(&orgs)?;
    let host = host.inner().clone();
    let view = tauri::async_runtime::spawn_blocking(move || host.enter(&key, &first.ledger))
        .await
        .map_err(|e| CommandError::internal(format!("keeping the key failed: {e}")))??;
    license_host::changed(&app);
    license_host::check_soon(&app);
    Ok(view)
}

/// Remove the license key: Free from now. Nothing else changes.
#[tauri::command]
pub async fn remove_license_key<R: Runtime>(
    app: AppHandle<R>,
    host: State<'_, Arc<LicenseHost>>,
    orgs: State<'_, Arc<Orgs>>,
) -> Result<LicenseView, CommandError> {
    let first = first(&orgs)?;
    let host = host.inner().clone();
    let view = tauri::async_runtime::spawn_blocking(move || host.remove(&first.ledger))
        .await
        .map_err(|e| CommandError::internal(format!("removing the key failed: {e}")))??;
    license_host::changed(&app);
    Ok(view)
}

/// Check with 8 West now. With no key, nothing is sent.
#[tauri::command]
pub async fn check_license_now<R: Runtime>(
    app: AppHandle<R>,
    host: State<'_, Arc<LicenseHost>>,
    orgs: State<'_, Arc<Orgs>>,
) -> Result<LicenseView, CommandError> {
    let first = first(&orgs)?;
    let view = host.check(&first.guard, &first.ledger).await;
    license_host::changed(&app);
    Ok(view)
}
