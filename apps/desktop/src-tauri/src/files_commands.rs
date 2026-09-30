//! The owner's files (Phase 21, ADR-093): the file view, the editor, Save, and opening a file in
//! another program or in its folder. Every command names a known top folder (a project's folder
//! or a working copy) and a path inside it; the broker finds the folder in the Ledger and checks
//! the path with Guard's path checker. Each is an organization's window's alone
//! (`capabilities/default.json`), never a pop-out's, the sign's, or a web page's.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use plenipo_capabilities::dto::{
    ChangingFile, FileRoots, FileView, FolderListing, LineEnding, SaveOutcome,
};
use plenipo_capabilities::Broker;
use plenipo_core::CommandError;
use plenipo_runtime::Supervisor;
use tauri::State;

use crate::commands::broker_error;
use crate::orgs::Org;

/// The most text Save takes (the broker checks the file's own limit too).
const MAX_SAVE_CHARS: usize = 6 * 1024 * 1024;

/// Opens a file with the program Windows uses for it, or shows it in its folder, through the
/// supervisor of the organization whose window asked (recorded in its own Ledger).
pub trait FileOpener: Send + Sync + 'static {
    fn open(
        &self,
        supervisor: Supervisor,
        path: PathBuf,
        reveal: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// The opener the page's commands use (tests put a stand-in here).
pub struct Outside(pub Arc<dyn FileOpener>);

/// Windows' own File Explorer (`xdg-open` elsewhere, for development), started through the
/// supervisor with no shell. Opening a file lets Windows pick its program; Plenipo refuses
/// programs and scripts before this (the broker's `owner_file_path`).
pub struct SystemFileOpener;

impl FileOpener for SystemFileOpener {
    fn open(
        &self,
        supervisor: Supervisor,
        path: PathBuf,
        reveal: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async move {
            let (executable, args) = if cfg!(windows) {
                let root = std::env::var_os("SystemRoot")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "C:\\Windows".into());
                let shown = path.display().to_string();
                (
                    root.join("explorer.exe"),
                    vec![if reveal {
                        format!("/select,{shown}")
                    } else {
                        shown
                    }],
                )
            } else {
                let target = if reveal {
                    path.parent().map_or(path.clone(), PathBuf::from)
                } else {
                    path.clone()
                };
                (
                    plenipo_capabilities::programs::find_on_path("xdg-open")
                        .ok_or_else(|| "xdg-open is not installed".to_owned())?,
                    vec![target.display().to_string()],
                )
            };
            let dir = std::env::temp_dir();
            plenipo_capabilities::programs::run(
                &supervisor,
                plenipo_capabilities::programs::Run {
                    label: if reveal {
                        "Show a file in its folder".into()
                    } else {
                        "Open a file in another program".into()
                    },
                    executable,
                    args,
                    working_dir: &dir,
                    env: Vec::new(),
                    stdin: None,
                    timeout: Duration::from_secs(30),
                },
                |_| {},
            )
            .await
            // File Explorer ends with a code of its own even when it worked.
            .map(|_| ())
        })
    }
}

fn check_path(path: &str) -> Result<(), CommandError> {
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err(CommandError::invalid_input("That is not a file name."));
    }
    Ok(())
}

fn check_root(root: &str) -> Result<(), CommandError> {
    if root.is_empty() || root.len() > 160 {
        return Err(CommandError::invalid_input(
            "That is not a folder Plenipo knows.",
        ));
    }
    Ok(())
}

/// Run broker file work off the async threads (it reads the disk).
async fn blocking<T: Send + 'static>(
    broker: &Broker,
    f: impl FnOnce(&Broker) -> plenipo_capabilities::Result<T> + Send + 'static,
) -> Result<T, CommandError> {
    let broker = broker.clone();
    tauri::async_runtime::spawn_blocking(move || f(&broker).map_err(broker_error))
        .await
        .map_err(|e| CommandError::internal(format!("the work stopped unexpectedly: {e}")))?
}

/// The folders Plenipo knows: each project's folder and its working copies, and who writes in
/// each now.
#[tauri::command]
pub async fn get_file_roots(broker: Org<'_, Broker>) -> Result<FileRoots, CommandError> {
    blocking(&broker, Broker::file_roots).await
}

/// One folder's files and folders.
#[tauri::command]
pub async fn list_folder(
    broker: Org<'_, Broker>,
    root: String,
    path: String,
) -> Result<FolderListing, CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    blocking(&broker, move |b| b.list_folder(&root, &path)).await
}

/// Open a file in Plenipo.
#[tauri::command]
pub async fn read_file(
    broker: Org<'_, Broker>,
    root: String,
    path: String,
) -> Result<FileView, CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    blocking(&broker, move |b| b.read_file(&root, &path)).await
}

/// Save a text file the owner edited (`base`: its fingerprint when opened; none: Save anyway).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn save_file(
    broker: Org<'_, Broker>,
    root: String,
    path: String,
    text: String,
    bom: bool,
    line_ending: LineEnding,
    base: Option<String>,
) -> Result<SaveOutcome, CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    if text.len() > MAX_SAVE_CHARS {
        return Err(CommandError::invalid_input(
            "Plenipo saves text files up to 5 MB.",
        ));
    }
    if base
        .as_ref()
        .is_some_and(|b| b.len() != 64 || !b.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(CommandError::invalid_input(
            "That is not a file's fingerprint.",
        ));
    }
    blocking(&broker, move |b| {
        b.save_file(&root, &path, &text, bom, line_ending, base.as_deref())
    })
    .await
}

/// Open a file with the program Windows uses for it (never a program or a script).
#[tauri::command]
pub async fn open_file_outside(
    broker: Org<'_, Broker>,
    supervisor: Org<'_, Supervisor>,
    outside: State<'_, Outside>,
    root: String,
    path: String,
) -> Result<(), CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    let file = blocking(&broker, move |b| b.owner_file_path(&root, &path, true)).await?;
    outside
        .0
        .open(supervisor.inner().clone(), file, false)
        .await
        .map_err(|e| CommandError::internal(format!("Plenipo could not open it: {e}")))
}

/// Show a file in File Explorer, picked.
#[tauri::command]
pub async fn show_in_folder(
    broker: Org<'_, Broker>,
    supervisor: Org<'_, Supervisor>,
    outside: State<'_, Outside>,
    root: String,
    path: String,
) -> Result<(), CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    let file = blocking(&broker, move |b| b.owner_file_path(&root, &path, false)).await?;
    outside
        .0
        .open(supervisor.inner().clone(), file, true)
        .await
        .map_err(|e| CommandError::internal(format!("Plenipo could not show it: {e}")))
}

/// The files workers are changing now.
#[tauri::command]
pub fn get_changing_files(broker: Org<'_, Broker>) -> Result<Vec<ChangingFile>, CommandError> {
    Ok(broker.changing_files())
}

// ---- Files dropped from File Explorer (ADR-093 §20) --------------------------------------------

/// The event an organization's window hears when files are dropped on it.
pub const DROP_EVENT: &str = "plenipo://drop";
/// A drop's ticket lasts this long (the owner may take a while to give the objective).
const DROP_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// Drops kept at once.
const MAX_DROPS: usize = 64;

struct Dropped {
    window: String,
    paths: Vec<PathBuf>,
    at: std::time::Instant,
}

/// Files dropped on Plenipo's windows from File Explorer, by ticket. The page never sees where
/// they are on the PC, only their names and the ticket; a file reaches an objective only through
/// a real drop on that same window.
#[derive(Default)]
pub struct Drops(std::sync::Mutex<std::collections::HashMap<String, Dropped>>);

impl Drops {
    /// Keep a drop on `window`; its ticket.
    pub fn add(&self, window: &str, paths: Vec<PathBuf>) -> String {
        let mut all = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        all.retain(|_, d| d.at.elapsed() < DROP_LIFETIME);
        while all.len() >= MAX_DROPS {
            let Some(oldest) = all.iter().min_by_key(|(_, d)| d.at).map(|(k, _)| k.clone()) else {
                break;
            };
            all.remove(&oldest);
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        all.insert(
            id.clone(),
            Dropped {
                window: window.to_owned(),
                paths,
                at: std::time::Instant::now(),
            },
        );
        id
    }

    /// One dropped file, for the window it was dropped on.
    pub fn path(&self, window: &str, drop: &str, index: u32) -> Result<PathBuf, CommandError> {
        let all = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        all.get(drop)
            .filter(|d| d.window == window && d.at.elapsed() < DROP_LIFETIME)
            .and_then(|d| d.paths.get(index as usize).cloned())
            .ok_or_else(|| CommandError::invalid_input("Drop that file on the objective again."))
    }
}

/// Files were dropped on an organization's window: keep them, and tell its page their names and
/// where they landed.
pub fn dropped<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    paths: Vec<PathBuf>,
    position: tauri::PhysicalPosition<f64>,
) {
    use tauri::{Emitter as _, Manager as _};
    if !crate::workspace_windows::is_org_window(window.label()) || paths.is_empty() {
        return;
    }
    let Some(drops) = window.try_state::<Drops>() else {
        return;
    };
    let files = paths
        .iter()
        .map(|p| {
            let meta = std::fs::metadata(p).ok();
            plenipo_capabilities::dto::DroppedFile {
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                folder: meta.as_ref().is_some_and(std::fs::Metadata::is_dir),
                size: meta.filter(std::fs::Metadata::is_file).map(|m| m.len()),
            }
        })
        .collect();
    let scale = window.scale_factor().unwrap_or(1.0);
    let at = position.to_logical::<f64>(scale);
    let drop = drops.add(window.label(), paths);
    let payload = plenipo_capabilities::dto::DroppedFiles {
        drop,
        files,
        x: at.x,
        y: at.y,
    };
    if let Err(e) = window.emit_to(window.label(), DROP_EVENT, &payload) {
        log::warn!("could not tell the window about the dropped files: {e}");
    }
}
