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

/// The most text Save takes (the broker checks the file's own limit too).
const MAX_SAVE_CHARS: usize = 6 * 1024 * 1024;

/// Opens a file with the program Windows uses for it, or shows it in its folder.
pub trait FileOpener: Send + Sync + 'static {
    fn open(
        &self,
        path: PathBuf,
        reveal: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// The opener the page's commands use (tests put a stand-in here).
pub struct Outside(pub Arc<dyn FileOpener>);

/// Windows' own File Explorer (`xdg-open` elsewhere, for development), started through the
/// supervisor with no shell. Opening a file lets Windows pick its program; Plenipo refuses
/// programs and scripts before this (the broker's `owner_file_path`).
pub struct SystemFileOpener {
    supervisor: Supervisor,
}

impl SystemFileOpener {
    pub fn new(supervisor: Supervisor) -> Self {
        Self { supervisor }
    }
}

impl FileOpener for SystemFileOpener {
    fn open(
        &self,
        path: PathBuf,
        reveal: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let supervisor = self.supervisor.clone();
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
pub async fn get_file_roots(broker: State<'_, Broker>) -> Result<FileRoots, CommandError> {
    blocking(&broker, Broker::file_roots).await
}

/// One folder's files and folders.
#[tauri::command]
pub async fn list_folder(
    broker: State<'_, Broker>,
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
    broker: State<'_, Broker>,
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
    broker: State<'_, Broker>,
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
    if base.as_ref().is_some_and(|b| {
        b.len() != 64 || !b.chars().all(|c| c.is_ascii_hexdigit())
    }) {
        return Err(CommandError::invalid_input("That is not a file's fingerprint."));
    }
    blocking(&broker, move |b| {
        b.save_file(&root, &path, &text, bom, line_ending, base.as_deref())
    })
    .await
}

/// Open a file with the program Windows uses for it (never a program or a script).
#[tauri::command]
pub async fn open_file_outside(
    broker: State<'_, Broker>,
    outside: State<'_, Outside>,
    root: String,
    path: String,
) -> Result<(), CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    let file = blocking(&broker, move |b| b.owner_file_path(&root, &path, true)).await?;
    outside
        .0
        .open(file, false)
        .await
        .map_err(|e| CommandError::internal(format!("Plenipo could not open it: {e}")))
}

/// Show a file in File Explorer, picked.
#[tauri::command]
pub async fn show_in_folder(
    broker: State<'_, Broker>,
    outside: State<'_, Outside>,
    root: String,
    path: String,
) -> Result<(), CommandError> {
    check_root(&root)?;
    check_path(&path)?;
    let file = blocking(&broker, move |b| b.owner_file_path(&root, &path, false)).await?;
    outside
        .0
        .open(file, true)
        .await
        .map_err(|e| CommandError::internal(format!("Plenipo could not show it: {e}")))
}

/// The files workers are changing now.
#[tauri::command]
pub fn get_changing_files(broker: State<'_, Broker>) -> Result<Vec<ChangingFile>, CommandError> {
    Ok(broker.changing_files())
}
