//! Creates the runtime supervisor for the desktop app and bridges its events to the UI.

use std::path::PathBuf;
use std::sync::Arc;

use plenipo_ledger::Ledger;
use plenipo_runtime::profile::diagnostic_profiles;
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};

use crate::ledger_host;
use plenipo_liaison::store::LedgerExecutionStore;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

/// Tauri event name carrying [`RuntimeEvent`] payloads to the main window.
pub const RUNTIME_EVENT: &str = "plenipo://runtime";

/// Where runtime state lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persistence {
    /// `<app local data>/runtime/` — the real app.
    AppData,
    /// Memory only — tests.
    InMemory,
}

struct TauriSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> EventSink for TauriSink<R> {
    fn emit(&self, event: RuntimeEvent) {
        let lifecycle = matches!(event, RuntimeEvent::Lifecycle(_));
        if let Err(e) = self.app.emit_to("main", RUNTIME_EVENT, &event) {
            eprintln!("[plenipo] failed to emit runtime event: {e}");
        }
        if lifecycle {
            crate::tray::refresh(&self.app);
        }
    }
}

/// Build the supervisor, persisting executions in the ledger. Never fails: problems become
/// notices shown in the UI, and the app falls back to no launch profiles rather than refusing
/// to start.
pub fn create_supervisor<R: Runtime>(
    app: &AppHandle<R>,
    persistence: Persistence,
    ledger: Arc<Ledger>,
) -> Supervisor {
    let mut notices = Vec::new();
    let work_dir = match persistence {
        Persistence::AppData => match prepare_dirs(app) {
            Ok((legacy_history, work_dir)) => {
                ledger_host::import_phase1_history(&ledger, &legacy_history);
                work_dir
            }
            Err(e) => {
                notices.push(format!("Runtime working directory unavailable ({e})."));
                std::env::temp_dir()
            }
        },
        Persistence::InMemory => std::env::temp_dir(),
    };
    let store = Arc::new(LedgerExecutionStore(ledger));

    // Phase 1 allows exactly one executable: Plenipo itself, in diagnostic mode.
    let (policy, profiles) = match std::env::current_exe() {
        Ok(exe) => match ExecutablePolicy::new([&exe]) {
            Ok(policy) => {
                let (registry, rejected) =
                    ProfileRegistry::new(diagnostic_profiles(&exe, &work_dir), &policy);
                notices.extend(rejected);
                (policy, registry)
            }
            Err(e) => {
                notices.push(format!("Launch profiles disabled: {e}"));
                (ExecutablePolicy::default(), ProfileRegistry::default())
            }
        },
        Err(e) => {
            notices.push(format!(
                "Launch profiles disabled: cannot locate Plenipo ({e})"
            ));
            (ExecutablePolicy::default(), ProfileRegistry::default())
        }
    };

    Supervisor::new(
        SupervisorConfig::default(),
        policy,
        profiles,
        store,
        Arc::new(TauriSink { app: app.clone() }),
        notices,
    )
}

fn prepare_dirs<R: Runtime>(app: &AppHandle<R>) -> Result<(PathBuf, PathBuf), String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("runtime");
    let work_dir = base.join("diagnostics-workspace");
    std::fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
    Ok((base.join("executions.json"), work_dir))
}
