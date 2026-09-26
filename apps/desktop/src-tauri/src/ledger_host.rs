//! Opens the Plenipo Ledger for the desktop app, streams its events to the UI, backs the
//! runtime supervisor with it, and imports Phase 1 execution history once.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plenipo_ledger::{ExecutionRow, Ledger, LedgerEvent, DB_FILE_NAME};
use plenipo_runtime::store::Loaded;
use plenipo_runtime::{
    AgentAttribution, ExecutionRecord, ExecutionState, ExecutionStore, MetadataStore, TokenUsage,
};
use serde_json::{json, Value};
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
            eprintln!("[plenipo] failed to emit ledger event: {e}");
        }
    }));
    ledger
}

fn in_memory(notice: Option<String>) -> Ledger {
    let ledger = Ledger::open_in_memory().expect("in-memory SQLite is always available");
    if let Some(notice) = notice {
        eprintln!("[plenipo] {notice}");
        ledger.add_notice(notice);
    }
    ledger
}

fn ledger_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("ledger")
        .join(DB_FILE_NAME))
}

/// Supervisor persistence backed by the ledger's `executions` table.
pub struct LedgerExecutionStore(pub Arc<Ledger>);

impl ExecutionStore for LedgerExecutionStore {
    fn load(&self, limit: usize) -> Loaded {
        match self
            .0
            .recent_executions(u32::try_from(limit).unwrap_or(u32::MAX))
        {
            Ok(rows) => {
                let mut notices = Vec::new();
                let mut records: Vec<ExecutionRecord> = rows
                    .into_iter()
                    .filter_map(|row| match to_record(row) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            notices.push(e);
                            None
                        }
                    })
                    .collect();
                records.reverse(); // oldest first
                Loaded { records, notices }
            }
            Err(e) => Loaded {
                records: vec![],
                notices: vec![format!(
                    "Could not read execution history from the ledger: {e}"
                )],
            },
        }
    }

    fn save(&self, record: &ExecutionRecord) -> Result<(), String> {
        self.0
            .upsert_execution(&to_row(record), "runtime")
            .map_err(|e| e.to_string())
    }
}

fn state_str(state: ExecutionState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// `runtime` value for plain supervised processes (no agent attribution).
const LOCAL_PROCESS: &str = "local-process";

pub fn to_row(r: &ExecutionRecord) -> ExecutionRow {
    let agent = r.agent.as_ref();
    ExecutionRow {
        id: r.id.clone(),
        task_id: agent.map(|a| a.task_id.clone()),
        runtime: agent.map_or_else(|| LOCAL_PROCESS.into(), |a| a.runtime_id.clone()),
        provider: agent.map(|a| a.provider.clone()),
        model: agent.and_then(|a| a.model.clone()),
        // Plenipo's session ID; the provider's own ID is kept with the usage metadata.
        session_id: agent.map(|a| a.session_id.clone()),
        process_id: r.pid,
        profile_id: Some(r.profile_id.clone()),
        label: r.label.clone(),
        executable: Some(r.executable.clone()),
        args: r.args.clone(),
        working_dir: Some(r.working_dir.clone()),
        state: state_str(r.state),
        exit_code: r.exit_code,
        detail: r.detail.clone(),
        started_at: r.started_at,
        ended_at: r.ended_at,
        usage_metadata: agent.map_or(
            Value::Null,
            |a| json!({ "providerSessionId": a.provider_session_id, "usage": a.usage }),
        ),
    }
}

fn to_attribution(row: &ExecutionRow) -> Option<Box<AgentAttribution>> {
    if row.runtime == LOCAL_PROCESS {
        return None;
    }
    let meta = &row.usage_metadata;
    Some(Box::new(AgentAttribution {
        runtime_id: row.runtime.clone(),
        provider: row.provider.clone()?,
        session_id: row.session_id.clone()?,
        task_id: row.task_id.clone()?,
        model: row.model.clone(),
        provider_session_id: meta
            .get("providerSessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        usage: meta
            .get("usage")
            .and_then(|u| serde_json::from_value::<TokenUsage>(u.clone()).ok()),
    }))
}

fn to_record(row: ExecutionRow) -> Result<ExecutionRecord, String> {
    let state: ExecutionState =
        serde_json::from_value(serde_json::Value::String(row.state.clone()))
            .map_err(|_| format!("Execution {} has an unknown state {:?}", row.id, row.state))?;
    let agent = to_attribution(&row);
    Ok(ExecutionRecord {
        id: row.id,
        profile_id: row.profile_id.unwrap_or_default(),
        label: row.label,
        executable: row.executable.unwrap_or_default(),
        args: row.args,
        working_dir: row.working_dir.unwrap_or_default(),
        pid: row.process_id,
        state,
        exit_code: row.exit_code,
        detail: row.detail,
        started_at: row.started_at,
        ended_at: row.ended_at,
        agent,
    })
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
    fn records_round_trip_through_the_ledger() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let store = LedgerExecutionStore(ledger.clone());
        let r = record("e1", ExecutionState::TimedOut);
        store.save(&r).unwrap();
        let loaded = store.load(10);
        assert_eq!(loaded.records, [r]);
        assert!(loaded.notices.is_empty());
        let events = ledger.events_for_execution("e1").unwrap();
        assert_eq!(events[0].event_type, "execution.timed_out");
    }

    #[test]
    fn agent_attribution_round_trips_through_the_ledger() {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let task = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    requested_by: "owner".into(),
                    objective: "Say hello".into(),
                    ..Default::default()
                },
                "owner",
            )
            .unwrap();
        let store = LedgerExecutionStore(ledger.clone());
        let mut r = record("e2", ExecutionState::Succeeded);
        r.agent = Some(Box::new(AgentAttribution {
            runtime_id: "claude-code".into(),
            provider: "anthropic".into(),
            session_id: "s-1".into(),
            task_id: task.id.clone(),
            model: Some("model-a".into()),
            provider_session_id: Some("p-9".into()),
            usage: Some(TokenUsage {
                input_tokens: 10,
                cached_input_tokens: 2,
                output_tokens: 5,
            }),
        }));
        store.save(&r).unwrap();
        assert_eq!(store.load(10).records, [r]);
        let row = ledger.execution("e2").unwrap().unwrap();
        assert_eq!(row.runtime, "claude-code");
        assert_eq!(row.provider.as_deref(), Some("anthropic"));
        assert_eq!(row.task_id.as_deref(), Some(task.id.as_str()));
        // The execution appears in its task's trail.
        let trail = ledger.events_for_task(&task.id).unwrap();
        assert!(trail.iter().any(|e| e.event_type == "execution.succeeded"));
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
