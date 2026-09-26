//! Status, integrity checks, backups, and export.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Map, Value};

use crate::dto::{BackupInfo, ExportInfo, IntegrityReport, LedgerStatus};
use crate::error::{LedgerError, Result};
use crate::{lock, migrate, Ledger};

/// Scheduled/manual backups kept per directory (pre-migration backups are never pruned).
pub const KEEP_BACKUPS: usize = 10;
const BACKUP_PREFIX: &str = "plenipo-backup-";

/// Tables included in a JSON export, in dependency order.
const EXPORT_TABLES: [&str; 15] = [
    "schema_migrations",
    "settings",
    "roles",
    "positions",
    "departments",
    "projects",
    "oversight",
    "runtime_sessions",
    "tasks",
    "agent_instances",
    "events",
    "executions",
    "approvals",
    "artifacts",
    "liaison_messages",
];

/// Write a consistent snapshot of `conn` to `dir/<prefix>-<timestamp>.db` and verify it.
pub(crate) fn snapshot(conn: &Connection, dir: &Path, prefix: &str) -> Result<BackupInfo> {
    std::fs::create_dir_all(dir)?;
    let created_at = crate::now_ms();
    let path = unique_path(dir, &format!("{prefix}-{created_at}"), "db");
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
    let verified = verify(&path);
    Ok(BackupInfo {
        size_bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        path: path.display().to_string(),
        created_at,
        verified,
    })
}

fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}.{ext}"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    path
}

/// Reopen a backup read-only and run an integrity check.
fn verify(path: &Path) -> bool {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|c| c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)))
        .is_ok_and(|r| r == "ok")
}

fn prune(dir: &Path, keep: usize) -> Result<()> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(BACKUP_PREFIX) && n.ends_with(".db"))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(keep);
    for old in backups.into_iter().take(excess) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

fn sqlite_value(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef::*;
    match v {
        Null => Value::Null,
        Integer(i) => json!(i),
        Real(f) => json!(f),
        Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        Blob(b) => json!({ "blobBytes": b.len() }),
    }
}

impl Ledger {
    pub fn schema_version(&self) -> Result<u32> {
        self.read(migrate::current_version)
    }

    pub fn status(&self) -> Result<LedgerStatus> {
        let (task_count, event_count, execution_count) = self.read(|c| {
            let count = |table: &str| -> Result<u64> {
                Ok(
                    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .map(crate::rows::u64_of)?,
                )
            };
            Ok((count("tasks")?, count("events")?, count("executions")?))
        })?;
        let size_bytes = self.path().map_or(0, |p| {
            let side = |suffix: &str| {
                let mut s = p.as_os_str().to_owned();
                s.push(suffix);
                std::fs::metadata(PathBuf::from(s))
                    .map(|m| m.len())
                    .unwrap_or(0)
            };
            std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) + side("-wal")
        });
        Ok(LedgerStatus {
            path: self.path().map(|p| p.display().to_string()),
            schema_version: self.schema_version()?,
            size_bytes,
            task_count,
            event_count,
            execution_count,
            notices: lock(&self.notices).clone(),
            last_integrity_check: lock(&self.last_integrity).clone(),
            last_backup: lock(&self.last_backup).clone(),
            persistent: self.path().is_some(),
        })
    }

    /// Full `PRAGMA integrity_check`. A failure is also added to the ledger notices.
    pub fn integrity_check(&self) -> Result<IntegrityReport> {
        let messages: Vec<String> = self.read(|c| {
            let mut stmt = c.prepare("PRAGMA integrity_check")?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })?;
        let fk_violations: i64 = self.read(|c| {
            let mut stmt = c.prepare("PRAGMA foreign_key_check")?;
            let n = stmt.query_map([], |_| Ok(()))?.count();
            Ok(i64::try_from(n).unwrap_or(i64::MAX))
        })?;
        let mut report = IntegrityReport {
            ok: messages == ["ok"] && fk_violations == 0,
            messages,
            checked_at: crate::now_ms(),
        };
        if fk_violations > 0 {
            report
                .messages
                .push(format!("{fk_violations} foreign key violation(s)"));
        }
        if !report.ok {
            self.add_notice(format!(
                "Ledger integrity check failed: {}. Create a backup and contact support before continuing.",
                report.messages.join("; ")
            ));
        }
        *lock(&self.last_integrity) = Some(report.clone());
        Ok(report)
    }

    /// Consistent, verified snapshot into `dir` (default: next to the database).
    /// Keeps the newest [`KEEP_BACKUPS`] backups in that directory.
    pub fn backup(&self, dir: Option<&Path>) -> Result<BackupInfo> {
        let dir = dir
            .map(Path::to_path_buf)
            .or_else(|| self.backups_dir())
            .ok_or_else(|| {
                LedgerError::InvalidInput("in-memory ledger needs a backup directory".into())
            })?;
        let info = self.read(|c| snapshot(c, &dir, BACKUP_PREFIX.trim_end_matches('-')))?;
        prune(&dir, KEEP_BACKUPS)?;
        *lock(&self.last_backup) = Some(info.clone());
        Ok(info)
    }

    /// Export every table as JSON to `dir` (default: next to the database).
    pub fn export_json(&self, dir: Option<&Path>) -> Result<ExportInfo> {
        let dir = dir
            .map(Path::to_path_buf)
            .or_else(|| self.backups_dir())
            .ok_or_else(|| {
                LedgerError::InvalidInput("in-memory ledger needs an export directory".into())
            })?;
        std::fs::create_dir_all(&dir)?;
        let created_at = crate::now_ms();
        let tables = self.read(|c| {
            let mut tables = Map::new();
            for table in EXPORT_TABLES {
                let mut stmt = c.prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))?;
                let names: Vec<String> =
                    stmt.column_names().into_iter().map(String::from).collect();
                let rows = stmt
                    .query_map([], |r| {
                        let mut obj = Map::new();
                        for (i, name) in names.iter().enumerate() {
                            obj.insert(name.clone(), sqlite_value(r.get_ref(i)?));
                        }
                        Ok(Value::Object(obj))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                tables.insert(table.into(), Value::Array(rows));
            }
            Ok(tables)
        })?;
        let doc = json!({
            "format": "plenipo-ledger-export",
            "formatVersion": 1,
            "schemaVersion": self.schema_version()?,
            "exportedAt": created_at,
            "tables": tables,
        });
        let path = unique_path(&dir, &format!("plenipo-export-{created_at}"), "json");
        std::fs::write(&path, serde_json::to_vec_pretty(&doc)?)?;
        Ok(ExportInfo {
            size_bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            path: path.display().to_string(),
            created_at,
        })
    }
}
