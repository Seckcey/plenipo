//! Settings → Devices, and the switch **Use Plenipo from another device** (Phase 14, ADR-141,
//! ADR-145). Organizations' windows only (`capabilities/default.json`): a pop-out, the sign, and
//! web pages cannot call them. Adding a phone, turning phone access on, and the approvals kept on
//! the PC widen who may connect or what a phone may answer, so they are the PC's alone (ADR-145
//! §3); a phone cannot reach any of these.

use std::sync::Arc;

use plenipo_core::CommandError;
use plenipo_guard::remote::KeptOnPc;
use plenipo_licensing::Limit;
use plenipo_remote::service::RemoteSettings;
use plenipo_remote::RemoteError;
use tauri::{AppHandle, Emitter as _, Runtime, State};

use crate::license_host::LicenseHost;
use crate::remote_host::{RemoteState, REMOTE_EVENT};

fn error(e: RemoteError) -> CommandError {
    match e {
        RemoteError::Invalid(m) | RemoteError::Refused(m) => CommandError::invalid_input(m),
        RemoteError::Store(m) => CommandError::internal(m),
    }
}

fn settings(state: &RemoteState, license: &LicenseHost) -> RemoteSettings {
    let pro = license
        .entitlements()
        .check(Limit::PhoneAccess)
        .is_allowed();
    state.settings(pro)
}

/// Run `work` off the window's thread (the Vault and the Ledger may take a moment), then give
/// Settings → Devices as it is.
async fn off_main<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, Arc<RemoteState>>,
    license: &State<'_, Arc<LicenseHost>>,
    work: impl FnOnce(&RemoteState) -> Result<(), CommandError> + Send + 'static,
) -> Result<RemoteSettings, CommandError> {
    let state = state.inner().clone();
    let license = license.inner().clone();
    let view = tauri::async_runtime::spawn_blocking(move || {
        work(&state)?;
        Ok::<_, CommandError>(settings(&state, &license))
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))??;
    let _ = app.emit(REMOTE_EVENT, "settings");
    Ok(view)
}

/// Settings → Devices: the switch, the phones, adding one, and the approvals kept on the PC.
#[tauri::command]
pub async fn get_remote(
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
) -> Result<RemoteSettings, CommandError> {
    let state = state.inner().clone();
    let license = license.inner().clone();
    tauri::async_runtime::spawn_blocking(move || settings(&state, &license))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))
}

/// Settings → Switches → **Use Plenipo from another device**. On is Pro only, and only once the
/// relay is live for this copy; off cuts every phone off at once (ADR-143 §7).
#[tauri::command]
pub async fn set_remote_switch<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    on: bool,
) -> Result<RemoteSettings, CommandError> {
    if on {
        license.allow(Limit::PhoneAccess)?;
        if !state.built.live {
            return Err(CommandError::invalid_input(
                "Using Plenipo from your phone is coming soon: 8 West's relay is not ready yet.",
            ));
        }
    }
    off_main(&app, &state, &license, move |s| {
        s.remote.set_switched_on(on).map_err(error)
    })
    .await
}

/// **Add a phone**: a new picture code and typed code, for 10 minutes (ADR-141).
#[tauri::command]
pub async fn start_phone_pairing<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
) -> Result<RemoteSettings, CommandError> {
    license.allow(Limit::PhoneAccess)?;
    off_main(&app, &state, &license, |s| {
        s.remote.start_pairing().map(drop).map_err(error)
    })
    .await
}

/// Stop adding a phone.
#[tauri::command]
pub async fn cancel_phone_pairing<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
) -> Result<RemoteSettings, CommandError> {
    off_main(&app, &state, &license, |s| {
        s.remote.cancel_pairing();
        Ok(())
    })
    .await
}

/// The owner's answer to "Is this your phone?": nothing is added until the owner says yes.
#[tauri::command]
pub async fn answer_phone_pairing<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    add: bool,
) -> Result<RemoteSettings, CommandError> {
    if add {
        license.allow(Limit::PhoneAccess)?;
    }
    off_main(&app, &state, &license, move |s| {
        s.remote.answer_pairing(add).map_err(error)
    })
    .await
}

fn check_device_id(id: &str) -> Result<(), CommandError> {
    if plenipo_remote::b64::is_id(id, 16) {
        Ok(())
    } else {
        Err(CommandError::invalid_input(
            "That is not a phone on your PC's list.",
        ))
    }
}

/// Rename a phone.
#[tauri::command]
pub async fn rename_device<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    id: String,
    name: String,
) -> Result<RemoteSettings, CommandError> {
    check_device_id(&id)?;
    if name.len() > 400 {
        return Err(CommandError::invalid_input("That name is too long."));
    }
    off_main(&app, &state, &license, move |s| {
        s.remote.rename(&id, &name).map_err(error)
    })
    .await
}

/// **Remove** a phone: cut off at once (ADR-143 §7).
#[tauri::command]
pub async fn remove_device<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    id: String,
) -> Result<RemoteSettings, CommandError> {
    check_device_id(&id)?;
    off_main(&app, &state, &license, move |s| {
        s.remote.remove(&id).map_err(error)
    })
    .await
}

/// Un-pause a phone paused after failed checks (ADR-142 §6).
#[tauri::command]
pub async fn unpause_device<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    id: String,
) -> Result<RemoteSettings, CommandError> {
    check_device_id(&id)?;
    off_main(&app, &state, &license, move |s| {
        s.remote.unpause(&id).map_err(error)
    })
    .await
}

/// **Keep these approvals on my PC only** (ADR-145 §5).
#[tauri::command]
pub async fn set_kept_on_pc<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RemoteState>>,
    license: State<'_, Arc<LicenseHost>>,
    kept: KeptOnPc,
) -> Result<RemoteSettings, CommandError> {
    if kept.kinds.len() > 64 {
        return Err(CommandError::invalid_input(
            "That is not a list Plenipo knows.",
        ));
    }
    off_main(&app, &state, &license, move |s| {
        s.remote.set_kept(kept).map(drop).map_err(error)
    })
    .await
}
