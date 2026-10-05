//! The organization folder (Phase 25, ADR-205): where a new organization's folder would go,
//! choosing another place with the system's own folder chooser, this organization's folder, and
//! showing it in File Explorer. Each is an organization's window's alone
//! (`capabilities/default.json`), never a pop-out's, the sign's, or a web page's.
//!
//! The folder chooser is the system's own (Windows' folder picker, a Mac's, or GTK's), opened from
//! here only: the page can't open one or learn a path any other way.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plenipo_capabilities::org_folder;
use plenipo_core::{CommandError, FolderSync, OrgFolderInfo};
use plenipo_guard::places::{self, SyncedBy, SystemPlaces};
use plenipo_ledger::Ledger;
use plenipo_runtime::Supervisor;
use tauri::{AppHandle, Manager as _, Runtime, State, WebviewWindow};

use crate::commands::ledger_error;
use crate::files_commands::Outside;
use crate::org_commands::org_window;
use crate::orgs::{Org, MAX_NAME};
use crate::runtime_host::Persistence;

/// Where organizations' folders go by default (ADR-205 §2.1): `Documents\Plenipo`, where
/// ADR-201's folders already are; Plenipo's data folder's `files` where there is no Documents
/// folder; a temporary folder for the app's own tests.
pub fn base<R: Runtime>(app: &AppHandle<R>, persistence: Persistence) -> PathBuf {
    match persistence {
        Persistence::AppData => app
            .path()
            .document_dir()
            .map(|d| d.join("Plenipo"))
            .unwrap_or_else(|_| {
                app.path()
                    .app_local_data_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join("files")
            }),
        Persistence::InMemory => std::env::temp_dir()
            .join(format!("plenipo-{}", std::process::id()))
            .join("files"),
    }
}

/// This PC's places that can never be an organization's folder.
pub fn places<R: Runtime>(app: &AppHandle<R>) -> SystemPlaces {
    SystemPlaces::from_env(
        app.path().app_local_data_dir().ok(),
        app.path().document_dir().ok(),
    )
}

/// The system's own folders (the profile's and Documents), at and above which a junction is
/// not looked at.
pub fn trusted<R: Runtime>(app: &AppHandle<R>) -> Vec<PathBuf> {
    places(app).trusted()
}

/// What the page is told about a folder: where, whether it is there, and who syncs it.
pub fn info(path: Option<&Path>, problem: Option<String>) -> OrgFolderInfo {
    let Some(path) = path else {
        return OrgFolderInfo {
            path: None,
            exists: false,
            synced_by: None,
            kept_on_this_device: None,
            problem,
        };
    };
    let roots = places::onedrive_roots();
    let synced = places::synced_by(path, &roots);
    let kept = match synced {
        Some(SyncedBy::OneDrive) => places::onedrive_root_of(path, &roots)
            .and_then(|root| places::kept_on_this_device(path, &root)),
        _ => None,
    };
    OrgFolderInfo {
        path: Some(path.display().to_string()),
        exists: path.is_dir(),
        synced_by: synced.map(|s| match s {
            SyncedBy::OneDrive => FolderSync::OneDrive,
            SyncedBy::Other => FolderSync::Other,
        }),
        kept_on_this_device: kept,
        problem,
    }
}

/// An organization's name, as the folder is named after it (the same check as its name).
fn clean_name(name: &str) -> Result<String, CommandError> {
    if name.trim().is_empty() {
        return Ok(String::new());
    }
    plenipo_ledger::workforce::clean_line("the organization's name", name, MAX_NAME)
        .map_err(ledger_error)
}

/// Where a new organization's folder is (ADR-205 §2.1): `folder` when the owner chose one (the
/// folder itself when it is empty, otherwise a folder named after the organization inside it),
/// else the usual place. Checked with Guard's `place_problem`: the place as it really is, or why
/// it can't be used.
pub fn new_place<R: Runtime>(
    app: &AppHandle<R>,
    persistence: Persistence,
    name: &str,
    folder: Option<&str>,
) -> Result<PathBuf, String> {
    let target = match folder {
        Some(chosen) => {
            let chosen = places::place_problem(chosen, &places(app))?;
            if org_folder::free(&chosen) {
                chosen
            } else {
                org_folder::suggest(&chosen, name)
            }
        }
        None => org_folder::suggest(&base(app, persistence), name),
    };
    let real = places::place_problem(&target.display().to_string(), &places(app))?;
    if !org_folder::free(&real) {
        return Err(format!(
            "{} already has something in it. Choose an empty folder, or a new one.",
            real.display()
        ));
    }
    Ok(real)
}

/// Where a new organization named `name` would get its folder: in `folder` when the owner chose
/// one, otherwise `Documents\Plenipo`; with who syncs it, or why it can't be used.
#[tauri::command]
pub async fn suggest_org_folder<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    name: String,
    folder: Option<String>,
) -> Result<OrgFolderInfo, CommandError> {
    org_window(&window)?;
    let name = clean_name(&name)?;
    if folder
        .as_ref()
        .is_some_and(|f| f.chars().count() > places::MAX_PLACE_CHARS)
    {
        return Err(CommandError::invalid_input(
            "That folder's path is too long.",
        ));
    }
    let persistence = app.state::<crate::org_host::Defaults>().persistence;
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match new_place(&app2, persistence, &name, folder.as_deref()) {
            Ok(path) => info(Some(&path), None),
            Err(problem) => {
                let shown = folder
                    .map(PathBuf::from)
                    .unwrap_or_else(|| org_folder::suggest(&base(&app2, persistence), &name));
                info(Some(&shown), Some(problem))
            }
        }
    })
    .await
    .map_err(|e| CommandError::internal(format!("the check stopped unexpectedly: {e}")))
}

/// Choose a folder with the system's own folder chooser, over the window that asked. `None`
/// when the owner closes it without choosing. What comes back is only what the chooser gave,
/// with who syncs it, or why it can't be an organization's folder.
#[tauri::command]
pub async fn choose_folder<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<Option<OrgFolderInfo>, CommandError> {
    org_window(&window)?;
    // The app's own tests have no screen to show a chooser on.
    if app.state::<crate::org_host::Defaults>().persistence == Persistence::InMemory {
        return Err(CommandError::internal(
            "the folder chooser isn't available here",
        ));
    }
    let Some(picked) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Choose a folder")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let path = picked.path().to_path_buf();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        Some(
            match places::place_problem(&path.display().to_string(), &places(&app2)) {
                Ok(real) => info(Some(&real), None),
                Err(problem) => info(Some(&path), Some(problem)),
            },
        )
    })
    .await
    .map_err(|e| CommandError::internal(format!("the check stopped unexpectedly: {e}")))
}

/// This organization's folder: where it is, whether it is there, and who syncs it.
#[tauri::command]
pub async fn get_org_folder(ledger: Org<'_, Arc<Ledger>>) -> Result<OrgFolderInfo, CommandError> {
    let ledger = ledger.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let folder = ledger.organization_folder().map_err(ledger_error)?;
        Ok(info(folder.as_ref().map(|f| Path::new(&f.path)), None))
    })
    .await
    .map_err(|e| CommandError::internal(format!("the check stopped unexpectedly: {e}")))?
}

/// Open this organization's folder in File Explorer. Plenipo finds the folder in its own
/// record: the page names nothing, and only a folder is ever opened.
#[tauri::command]
pub async fn open_org_folder(
    ledger: Org<'_, Arc<Ledger>>,
    supervisor: Org<'_, Supervisor>,
    outside: State<'_, Outside>,
) -> Result<(), CommandError> {
    let folder = ledger
        .organization_folder()
        .map_err(ledger_error)?
        .ok_or_else(|| {
            CommandError::invalid_input("This organization has no organization folder yet.")
        })?;
    let path = PathBuf::from(&folder.path);
    // A recorded folder that became a junction or link leads somewhere else: not opened.
    let link = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink());
    if link || !path.is_dir() {
        return Err(CommandError::invalid_input(format!(
            "The folder {} is not there now.",
            folder.path
        )));
    }
    outside
        .0
        .open(supervisor.inner().clone(), path, false)
        .await
        .map_err(|e| CommandError::internal(format!("Plenipo could not open it: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_nothing_in_it_says_so() {
        let none = info(None, None);
        assert_eq!(none.path, None);
        assert!(!none.exists);
        let dir = tempfile::tempdir().unwrap();
        let here = info(Some(dir.path()), Some("why".into()));
        assert!(here.exists);
        assert_eq!(here.problem.as_deref(), Some("why"));
        assert_eq!(here.synced_by, None);
        assert_eq!(here.kept_on_this_device, None);
    }
}
