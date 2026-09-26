//! Durable execution metadata (JSON file, atomic writes).
//!
//! Interim store until the SQLite Ledger (Phase 2). Holds metadata only — never output
//! or environment values. A corrupt file is quarantined and reported, never silently
//! discarded.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dto::ExecutionRecord;

/// Maximum number of records kept on disk (oldest finished records are dropped first).
pub const MAX_HISTORY: usize = 200;
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    executions: Vec<ExecutionRecord>,
}

/// Where execution metadata is persisted. `None` keeps it in memory only.
#[derive(Debug, Clone, Default)]
pub struct MetadataStore {
    path: Option<PathBuf>,
}

/// Result of loading the store.
#[derive(Debug, Default)]
pub struct Loaded {
    pub records: Vec<ExecutionRecord>,
    pub notices: Vec<String>,
}

impl MetadataStore {
    pub fn in_memory() -> Self {
        Self { path: None }
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn load(&self) -> Loaded {
        let Some(path) = &self.path else {
            return Loaded::default();
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::default(),
            Err(e) => {
                return Loaded {
                    records: vec![],
                    notices: vec![format!(
                        "Could not read execution history ({}): {e}",
                        path.display()
                    )],
                }
            }
        };
        match serde_json::from_str::<FileFormat>(&text) {
            Ok(f) if f.version == FORMAT_VERSION => Loaded {
                records: f.executions,
                notices: vec![],
            },
            Ok(f) => Loaded {
                records: vec![],
                notices: vec![
                    self.quarantine(path, &format!("unsupported format version {}", f.version))
                ],
            },
            Err(e) => Loaded {
                records: vec![],
                notices: vec![self.quarantine(path, &e.to_string())],
            },
        }
    }

    /// Move an unreadable file aside so it can be inspected, and describe what happened.
    fn quarantine(&self, path: &Path, reason: &str) -> String {
        let aside = path.with_extension(format!("corrupt-{}.json", crate::now_ms()));
        match fs::rename(path, &aside) {
            Ok(()) => format!(
                "Execution history was unreadable ({reason}). It was moved to {} and a new history was started.",
                aside.display()
            ),
            Err(e) => format!(
                "Execution history was unreadable ({reason}) and could not be moved aside: {e}"
            ),
        }
    }

    /// Atomically replace the stored records (write temp file, then rename).
    pub fn save(&self, records: &[ExecutionRecord]) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(&FileFormat {
            version: FORMAT_VERSION,
            executions: records.to_vec(),
        })
        .map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&data)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ExecutionState;

    pub(crate) fn record(id: &str, state: ExecutionState) -> ExecutionRecord {
        ExecutionRecord {
            id: id.into(),
            profile_id: "diagnostic.echo".into(),
            label: "Echo".into(),
            executable: "/x".into(),
            args: vec![],
            working_dir: "/".into(),
            pid: Some(1),
            state,
            exit_code: None,
            detail: None,
            started_at: 1,
            ended_at: None,
        }
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = MetadataStore::file(dir.path().join("x.json")).load();
        assert!(loaded.records.is_empty() && loaded.notices.is_empty());
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::file(dir.path().join("nested/executions.json"));
        let records = vec![
            record("a", ExecutionState::Succeeded),
            record("b", ExecutionState::Running),
        ];
        store.save(&records).unwrap();
        assert_eq!(store.load().records, records);
        assert!(!dir.path().join("nested/executions.json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_quarantined_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("executions.json");
        fs::write(&path, b"{ not json").unwrap();
        let loaded = MetadataStore::file(&path).load();
        assert!(loaded.records.is_empty());
        assert_eq!(loaded.notices.len(), 1);
        assert!(loaded.notices[0].contains("unreadable"));
        assert!(!path.exists(), "corrupt file must be moved aside");
        let quarantined: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt-"))
            .collect();
        assert_eq!(quarantined.len(), 1);
    }

    #[test]
    fn unknown_version_is_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("executions.json");
        fs::write(&path, br#"{"version":99,"executions":[]}"#).unwrap();
        let loaded = MetadataStore::file(&path).load();
        assert!(loaded.notices[0].contains("unsupported format version 99"));
    }
}
