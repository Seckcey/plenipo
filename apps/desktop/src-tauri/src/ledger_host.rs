//! Opens the Plenipo Ledger for the desktop app, streams its events to the UI, and imports
//! Phase 1 execution history once. The Ledger-backed runtime stores live in `plenipo-liaison`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plenipo_ledger::{Ledger, LedgerEvent, DB_FILE_NAME};
use plenipo_liaison::store::to_row;
use plenipo_runtime::MetadataStore;
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::runtime_host::Persistence;

/// Tauri event name carrying each committed [`LedgerEvent`] to the main window.
pub const LEDGER_EVENT: &str = "plenipo://ledger";

/// Open the ledger. Never fails: if the on-disk ledger cannot be used, Plenipo runs on a
/// temporary in-memory ledger and says so prominently (nothing is silently ignored).
pub fn open<R: Runtime>(app: &AppHandle<R>, persistence: Persistence) -> Arc<Ledger> {
    let ledger = match persistence {
        Persistence::InMemory => in_memory(None),
        Persistence::AppData => match ledger_path(app) {
            Ok(path) => match Ledger::open(&path) {
                Ok(ledger) => ledger,
                Err(e) => in_memory(Some(format!(
                    "The ledger at {} could not be opened: {e}. Plenipo is using a temporary \
                     ledger for this session; nothing will be saved until this is resolved.",
                    path.display()
                ))),
            },
            Err(e) => in_memory(Some(format!(
                "The ledger location is unavailable ({e}). Plenipo is using a temporary ledger \
                 for this session; nothing will be saved."
            ))),
        },
    };
    let ledger = Arc::new(ledger);
    let handle = app.clone();
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if let Err(e) = handle.emit_to("main", LEDGER_EVENT, event) {
            log::warn!("failed to emit ledger event: {e}");
        }
    }));
    ledger
}

fn in_memory(notice: Option<String>) -> Ledger {
    let ledger = Ledger::open_in_memory().expect("in-memory SQLite is always available");
    if let Some(notice) = notice {
        log::warn!("{notice}");
        ledger.add_notice(notice);
    }
    ledger
}

pub fn ledger_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("ledger")
        .join(DB_FILE_NAME))
}

/// Before opening (Phase 13): carry out a restore chosen in Diagnostics, if one was.
pub fn apply_pending_restore(path: &Path) -> Option<plenipo_ledger::RestoreOutcome> {
    let outcome = plenipo_ledger::backups::apply_pending_restore(path)?;
    if outcome.restored.is_some() {
        log::warn!("{}", outcome.message);
    } else {
        log::error!("{}", outcome.message);
    }
    Some(outcome)
}

/// The Ledger at `path` is about to have its layout changed (a migration) when opened.
pub fn needs_layout_change(path: &Path) -> bool {
    plenipo_ledger::backups::layout_of_file(path)
        .is_some_and(|v| v > 0 && v < plenipo_ledger::migrate::latest(plenipo_ledger::MIGRATIONS))
}

/// Record a restore that happened at this start, and tell the owner.
pub fn record_restore(ledger: &Ledger, outcome: &plenipo_ledger::RestoreOutcome) {
    ledger.add_notice(outcome.message.clone());
    let event = plenipo_ledger::NewEvent {
        source: "owner".into(),
        event_type: if outcome.restored.is_some() {
            "ledger.restored".into()
        } else {
            "ledger.restore_failed".into()
        },
        payload: serde_json::json!({
            "backup": outcome.restored.as_ref().map(|b| &b.name),
            "keptAs": outcome.kept_as,
        }),
        ..plenipo_ledger::NewEvent::default()
    };
    if let Err(e) = ledger.append_event(event) {
        log::warn!("the restore could not be recorded: {e}");
    }
    // Paid spending since the backup was made still counts (Phase 16 Wave 3, ADR-085): the
    // Ledger as it was before the restore was kept as a backup, and its spending records come
    // back, so restoring never opens room under a spending cap.
    if let (Some(_), Some(kept)) = (&outcome.restored, &outcome.kept_as) {
        match ledger.carry_spending_from_backup(kept) {
            Ok(0) => {}
            Ok(n) => log::info!("{n} spending record(s) were kept through the restore"),
            Err(e) => log::warn!("spending records could not be kept through the restore: {e}"),
        }
    }
}

/// Move Phase 1's `executions.json` into the ledger (once), then rename the file.
pub fn import_phase1_history(ledger: &Ledger, json_path: &Path) {
    if !json_path.exists() {
        return;
    }
    let loaded = MetadataStore::file(json_path).load();
    for notice in loaded.notices {
        ledger.add_notice(notice);
    }
    let mut imported = 0;
    for record in &loaded.records {
        let already = ledger.execution(&record.id).ok().flatten().is_some();
        if !already {
            match ledger.upsert_execution(&to_row(record), "import") {
                Ok(()) => imported += 1,
                Err(e) => {
                    ledger.add_notice(format!("Could not import execution {}: {e}", record.id));
                    return; // keep the file for a later retry
                }
            }
        }
    }
    let done = json_path.with_extension(format!("json.imported-{}", plenipo_ledger::now_ms()));
    if std::fs::rename(json_path, &done).is_ok() && imported > 0 {
        ledger.add_notice(format!(
            "Imported {imported} execution(s) from the previous history file into the ledger."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_runtime::{ExecutionRecord, ExecutionState};

    fn record(id: &str, state: ExecutionState) -> ExecutionRecord {
        ExecutionRecord {
            id: id.into(),
            profile_id: "diagnostic.echo".into(),
            label: "Echo test".into(),
            executable: "/x".into(),
            args: vec!["--plenipo-diagnostic=echo".into()],
            working_dir: "/".into(),
            pid: Some(7),
            state,
            exit_code: Some(0),
            detail: None,
            started_at: 10,
            ended_at: Some(20),
            agent: None,
        }
    }

    #[test]
    fn phase1_history_is_imported_once() {
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join("executions.json");
        MetadataStore::file(&json)
            .save_all(&[
                record("a", ExecutionState::Succeeded),
                record("b", ExecutionState::Failed),
            ])
            .unwrap();
        let ledger = Ledger::open_in_memory().unwrap();
        import_phase1_history(&ledger, &json);
        assert!(!json.exists(), "renamed after import");
        assert_eq!(ledger.recent_executions(10).unwrap().len(), 2);
        let status = ledger.status().unwrap();
        assert!(status
            .notices
            .iter()
            .any(|n| n.contains("Imported 2 execution(s)")));
        // Running again is a no-op.
        import_phase1_history(&ledger, &json);
        assert_eq!(ledger.recent_executions(10).unwrap().len(), 2);
    }
}
