//! Ledger backups by kind, the list the owner restores from, and restoring one (Phase 13).
//!
//! Every backup is a verified `VACUUM INTO` snapshot in the backups folder next to the Ledger.
//! Its file name says what kind it is, so the list needs no index of its own:
//!
//! | Kind            | File name                            | Kept             |
//! | --------------- | ------------------------------------ | ---------------- |
//! | Manual          | `plenipo-backup-<ms>.db`             | newest 10        |
//! | Daily           | `daily-backup-<ms>.db`               | newest 7         |
//! | Before upgrade  | `pre-upgrade-<version>-<ms>.db`      | newest 5         |
//! | Before update   | `pre-update-<version>-<ms>.db`       | newest 5         |
//! | Before a layout change (migration) | `pre-migration-v<n>-<ms>.db` | always (ADR-006) |
//! | Before a restore | `before-restore-<ms>.db`            | newest 3         |
//!
//! **Restoring** never swaps the file under a running Plenipo: [`request_restore`] checks the
//! backup and leaves a request next to the Ledger; the next start calls
//! [`apply_pending_restore`] before opening the Ledger. The Ledger as it was is kept first (a
//! "before restore" backup), so a restore can itself be undone.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension as _};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{LedgerError, Result};
use crate::maintenance::{snapshot, verify};
use crate::{backups_dir, lock, migrate, BackupInfo, Ledger, MIGRATIONS};

/// The request a restore leaves next to the Ledger for the next start.
pub const RESTORE_REQUEST: &str = "restore-request.json";

/// What made a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BackupKind {
    /// The owner asked for it (Diagnostics → Create backup).
    Manual,
    /// Made once a day while Plenipo runs.
    Daily,
    /// Made when a new version of Plenipo first started.
    BeforeUpgrade,
    /// Made by the old version just before installing an update.
    BeforeUpdate,
    /// Made before the Ledger's layout was changed (a migration).
    BeforeMigration,
    /// The Ledger as it was just before a restore replaced it.
    BeforeRestore,
}

impl BackupKind {
    pub const ALL: [Self; 6] = [
        Self::Manual,
        Self::Daily,
        Self::BeforeUpgrade,
        Self::BeforeUpdate,
        Self::BeforeMigration,
        Self::BeforeRestore,
    ];

    /// The start of this kind's file names (before `-`).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Manual => "plenipo-backup",
            Self::Daily => "daily-backup",
            Self::BeforeUpgrade => "pre-upgrade",
            Self::BeforeUpdate => "pre-update",
            Self::BeforeMigration => "pre-migration",
            Self::BeforeRestore => "before-restore",
        }
    }

    /// How many of this kind are kept (`None`: all).
    pub fn keep(self) -> Option<usize> {
        match self {
            Self::Manual => Some(crate::maintenance::KEEP_BACKUPS),
            Self::Daily => Some(7),
            Self::BeforeUpgrade | Self::BeforeUpdate => Some(5),
            Self::BeforeMigration => None,
            Self::BeforeRestore => Some(3),
        }
    }

    fn of_file(name: &str) -> Option<Self> {
        if !name.ends_with(".db") {
            return None;
        }
        Self::ALL
            .into_iter()
            .find(|k| name.starts_with(&format!("{}-", k.prefix())))
    }
}

/// One backup the owner can restore (Diagnostics → Restore).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LedgerBackup {
    /// The file's name: how the owner picks it (never a path).
    pub name: String,
    pub kind: BackupKind,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// The Plenipo version named in it (before an upgrade: the version before; before an
    /// update: the version being installed).
    pub version: Option<String>,
    /// The Ledger layout version inside it, when it could be read.
    pub schema_version: Option<u32>,
    /// This version of Plenipo can restore it.
    pub restorable: bool,
    /// Why it cannot be restored, in plain words.
    pub problem: Option<String>,
}

/// The backups, and a restore waiting for the next start (Diagnostics → Restore).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LedgerBackups {
    /// Newest first.
    pub backups: Vec<LedgerBackup>,
    /// The backup that becomes the Ledger at the next start, if one was chosen.
    pub pending_restore: Option<String>,
    /// Where they are kept (shown, never opened from the screen).
    pub folder: Option<String>,
}

/// What [`apply_pending_restore`] did at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOutcome {
    /// The backup that is now the Ledger (`None`: the restore could not be done).
    pub restored: Option<LedgerBackup>,
    /// The name of the backup of the Ledger as it was before.
    pub kept_as: Option<String>,
    /// A plain-words sentence for the owner.
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreRequest {
    backup: String,
    requested_at: u64,
}

/// The timestamp in a backup's name: the last run of 12 or more digits.
fn stamp_of(name: &str) -> Option<u64> {
    name.trim_end_matches(".db")
        .rsplit('-')
        .find(|part| part.len() >= 12 && part.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|p| p.parse().ok())
}

/// The version in `pre-upgrade-<version>-<ms>.db` / `pre-update-<version>-<ms>.db`.
fn version_of(kind: BackupKind, name: &str) -> Option<String> {
    if !matches!(kind, BackupKind::BeforeUpgrade | BackupKind::BeforeUpdate) {
        return None;
    }
    let rest = name.strip_prefix(&format!("{}-", kind.prefix()))?;
    let (version, _) = rest.split_once('-')?;
    (!version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.'))
    .then(|| version.to_owned())
}

/// A backup's Ledger layout version, read without changing the file.
fn schema_of(path: &Path) -> std::result::Result<u32, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let has_history: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if has_history.is_none() {
        return Err("it is not a Plenipo Ledger".into());
    }
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

/// The Ledger layout version of the database file at `path`, read without changing it
/// (`None`: missing, or not a Plenipo Ledger).
pub fn layout_of_file(path: &Path) -> Option<u32> {
    if !path.is_file() {
        return None;
    }
    schema_of(path).ok()
}

fn describe(dir: &Path, name: &str) -> Option<LedgerBackup> {
    let kind = BackupKind::of_file(name)?;
    let path = dir.join(name);
    let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
    let created_at = stamp_of(name).unwrap_or_else(|| {
        meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    });
    let latest = migrate::latest(MIGRATIONS);
    let (schema_version, problem) = match schema_of(&path) {
        Ok(v) if v > latest => (
            Some(v),
            Some(format!(
                "A newer version of Plenipo made it (Ledger layout {v}); this version reads up to \
                 layout {latest}."
            )),
        ),
        Ok(0) => (Some(0), Some("It is empty.".into())),
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(format!("It cannot be read ({e})."))),
    };
    Some(LedgerBackup {
        name: name.to_owned(),
        kind,
        created_at,
        size_bytes: meta.len(),
        version: version_of(kind, name),
        schema_version,
        restorable: problem.is_none(),
        problem,
    })
}

/// Every backup in `dir`, newest first.
pub fn list_backups(dir: &Path) -> Result<Vec<LedgerBackup>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut backups: Vec<LedgerBackup> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|name| describe(dir, &name))
        .collect();
    backups.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(backups)
}

/// Keep only the newest `kind` backups (see [`BackupKind::keep`]).
fn prune_kind(dir: &Path, kind: BackupKind) -> Result<()> {
    prune_kind_except(dir, kind, None)
}

/// Keep only the newest of `kind`, never deleting `except` (the backup being restored).
fn prune_kind_except(dir: &Path, kind: BackupKind, except: Option<&str>) -> Result<()> {
    let Some(keep) = kind.keep() else {
        return Ok(());
    };
    let mut of_kind: Vec<(u64, String)> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| BackupKind::of_file(n) == Some(kind) && Some(n.as_str()) != except)
        .map(|n| (stamp_of(&n).unwrap_or(0), n))
        .collect();
    of_kind.sort();
    let excess = of_kind.len().saturating_sub(keep);
    for (_, name) in of_kind.into_iter().take(excess) {
        std::fs::remove_file(dir.join(name))?;
    }
    Ok(())
}

/// A name that is one file in the backups folder: no folders, no way out of it.
fn plain_name(name: &str) -> Result<&str> {
    let ok = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && BackupKind::of_file(name).is_some();
    if ok {
        Ok(name)
    } else {
        Err(LedgerError::InvalidInput(
            "that is not one of the Ledger's backups".into(),
        ))
    }
}

/// Check that `name` can be restored over the Ledger at `db_path` (it exists, it is whole, and
/// this version can read it).
fn check_restorable(db_path: &Path, name: &str) -> Result<LedgerBackup> {
    let name = plain_name(name)?;
    let dir = backups_dir(db_path);
    let backup = describe(&dir, name).ok_or_else(|| {
        LedgerError::InvalidInput("that is not one of the Ledger's backups".into())
    })?;
    if let Some(problem) = &backup.problem {
        return Err(LedgerError::InvalidInput(format!(
            "That backup cannot be restored: {problem}"
        )));
    }
    if !verify(&dir.join(name)) {
        return Err(LedgerError::InvalidInput(
            "That backup cannot be restored: it failed its integrity check.".into(),
        ));
    }
    Ok(backup)
}

/// Ask for `name` to become the Ledger at the next start. Checks it first.
pub fn request_restore(db_path: &Path, name: &str) -> Result<LedgerBackup> {
    let backup = check_restorable(db_path, name)?;
    let request = RestoreRequest {
        backup: backup.name.clone(),
        requested_at: crate::now_ms(),
    };
    let path = restore_request_path(db_path);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&request)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(backup)
}

/// The backups of the Ledger file at `db_path`, and a restore waiting for the next start, read
/// from its folder alone. Plenipo uses it too when it could not open that file (one from a newer
/// Plenipo, after going back to an older version): its backups can still be restored.
pub fn overview_at(db_path: &Path) -> Result<LedgerBackups> {
    let dir = backups_dir(db_path);
    Ok(LedgerBackups {
        backups: list_backups(&dir)?,
        pending_restore: pending_restore(db_path),
        folder: Some(dir.display().to_string()),
    })
}

/// A restore waiting for the next start, if any.
pub fn pending_restore(db_path: &Path) -> Option<String> {
    let text = std::fs::read(restore_request_path(db_path)).ok()?;
    serde_json::from_slice::<RestoreRequest>(&text)
        .ok()
        .map(|r| r.backup)
}

/// Forget a restore that was asked for but has not happened.
pub fn cancel_restore(db_path: &Path) -> Result<()> {
    match std::fs::remove_file(restore_request_path(db_path)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

fn restore_request_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(RESTORE_REQUEST)
}

fn side_file(db_path: &Path, suffix: &str) -> PathBuf {
    let mut s = db_path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// How the Ledger as it was before a restore was kept.
enum Kept {
    /// A verified backup (listed in Diagnostics), by name.
    Backup(String),
    /// It could not be read, so its files were moved aside, untouched, under this name.
    MovedAside(String),
}

impl Kept {
    fn name(&self) -> &str {
        match self {
            Self::Backup(n) | Self::MovedAside(n) => n,
        }
    }

    fn words(&self) -> String {
        match self {
            Self::Backup(n) => format!(
                "The Ledger as it was before is kept as the backup {n}, so you can go back to it."
            ),
            Self::MovedAside(n) => format!(
                "The Ledger as it was before could not be read; its files were moved aside, \
                 untouched, as {n} next to the Ledger."
            ),
        }
    }
}

/// Whether the Ledger file at `db_path` can be read (a quick check of its pages).
fn readable(db_path: &Path) -> bool {
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .and_then(|c| c.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0)))
        .is_ok_and(|r| r == "ok")
}

/// Keep the Ledger as it is now before a restore replaces it. One that can be read is backed
/// up and checked, or the restore stops (a full disk must never cost the owner their Ledger);
/// its write-ahead log is emptied into it first, so nothing is only in that log. One that
/// cannot be read has its files moved aside, untouched.
fn keep_current(db_path: &Path) -> std::result::Result<Option<Kept>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    if readable(db_path) {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .map_err(|e| e.to_string())?;
        let info = snapshot(
            &conn,
            &backups_dir(db_path),
            BackupKind::BeforeRestore.prefix(),
        )
        .map_err(|e| format!("it could not be backed up first ({e})"))?;
        if !info.verified {
            let _ = std::fs::remove_file(&info.path);
            return Err("its backup failed its integrity check".into());
        }
        drop(conn);
        let name = Path::new(&info.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(Some(Kept::Backup(name)));
    }
    let stamp = crate::now_ms();
    let name = db_path
        .file_name()
        .map_or_else(|| "plenipo.db".into(), |n| n.to_string_lossy().into_owned());
    for suffix in ["", "-wal", "-shm"] {
        let from = side_file(db_path, suffix);
        if from.exists() {
            let to = db_path.with_file_name(format!("{name}{suffix}.replaced-{stamp}"));
            std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(Some(Kept::MovedAside(format!("{name}.replaced-{stamp}"))))
}

/// Before the Ledger at `db_path` is opened: carry out a restore that was asked for. Returns
/// `None` when none was asked for. On any problem the Ledger is left as it was.
///
/// Nothing is changed until the backup is copied next to the Ledger and checked, and the
/// Ledger as it is now is kept; then the copy takes the Ledger's place in one rename, so a
/// restore stopped at any point leaves either the old Ledger or the restored one, never a
/// missing or half-written file.
pub fn apply_pending_restore(db_path: &Path) -> Option<RestoreOutcome> {
    let request_path = restore_request_path(db_path);
    let text = std::fs::read(&request_path).ok()?;
    let failed = |why: String| RestoreOutcome {
        restored: None,
        kept_as: None,
        message: format!("The Ledger was not restored: {why} Your Ledger was not changed."),
    };
    // Remove the request first: a restore that fails, or Plenipo stopping halfway, must not be
    // tried again at every start (and a request that cannot be removed would restore again at
    // every start, undoing what was done since).
    if let Err(e) = std::fs::remove_file(&request_path) {
        return Some(failed(format!(
            "the note asking for it could not be removed ({e}), and it would restore again at \
             every start."
        )));
    }
    let Ok(request) = serde_json::from_slice::<RestoreRequest>(&text) else {
        return Some(failed(
            "the note asking for it was damaged, so Plenipo could not tell which backup to use."
                .into(),
        ));
    };
    let backup = match check_restorable(db_path, &request.backup) {
        Ok(b) => b,
        Err(e) => return Some(failed(format!("{}.", plain(&e)))),
    };
    let staged = side_file(db_path, ".restoring");
    let _ = std::fs::remove_file(&staged);
    let copied = std::fs::copy(backups_dir(db_path).join(&backup.name), &staged);
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&staged);
        return Some(failed(format!("the backup could not be copied ({e}).")));
    }
    if !verify(&staged) {
        let _ = std::fs::remove_file(&staged);
        return Some(failed(
            "the copy of the backup failed its integrity check.".into(),
        ));
    }
    let kept = match keep_current(db_path) {
        Ok(kept) => kept,
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Some(failed(format!(
                "the Ledger as it is now could not be kept first: {e}."
            )));
        }
    };
    let kept_as = kept.as_ref().map(|k| k.name().to_owned());
    let not_replaced = |e: std::io::Error| {
        let _ = std::fs::remove_file(&staged);
        RestoreOutcome {
            restored: None,
            kept_as: kept_as.clone(),
            message: format!(
                "The Ledger was not restored: its file could not be replaced ({e}). {}",
                kept.as_ref().map_or_else(String::new, Kept::words)
            ),
        }
    };
    // The restored file must not meet the old one's write-ahead log (emptied above).
    for suffix in ["-wal", "-shm"] {
        let path = side_file(db_path, suffix);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                return Some(not_replaced(e));
            }
        }
    }
    if let Err(e) = std::fs::rename(&staged, db_path) {
        return Some(not_replaced(e));
    }
    let _ = prune_kind_except(
        &backups_dir(db_path),
        BackupKind::BeforeRestore,
        Some(&backup.name),
    );
    let message = match &kept {
        Some(k) => format!(
            "The Ledger was restored from the backup {}. {}",
            backup.name,
            k.words()
        ),
        None => format!("The Ledger was restored from the backup {}.", backup.name),
    };
    Some(RestoreOutcome {
        restored: Some(backup),
        kept_as,
        message,
    })
}

/// A Ledger error in plain words (without the "invalid input:" prefix).
fn plain(e: &LedgerError) -> String {
    match e {
        LedgerError::InvalidInput(m) | LedgerError::NotFound(m) => {
            m.trim_end_matches('.').to_owned()
        }
        other => other.to_string(),
    }
}

impl Ledger {
    /// A verified backup of `kind` in the backups folder, keeping only the newest of that kind.
    /// `version` names the Plenipo version for [`BackupKind::BeforeUpgrade`] and
    /// [`BackupKind::BeforeUpdate`].
    pub fn backup_of_kind(&self, kind: BackupKind, version: Option<&str>) -> Result<BackupInfo> {
        let dir = self
            .backups_dir()
            .ok_or_else(|| LedgerError::InvalidInput("a temporary Ledger has no backups".into()))?;
        let prefix = match (kind, version) {
            (BackupKind::BeforeUpgrade | BackupKind::BeforeUpdate, Some(v)) => {
                let v: String = v
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
                    .take(40)
                    .collect();
                format!(
                    "{}-{}",
                    kind.prefix(),
                    if v.is_empty() { "unknown" } else { &v }
                )
            }
            _ => kind.prefix().to_owned(),
        };
        let info = self.read(|c| snapshot(c, &dir, &prefix))?;
        if !info.verified {
            let _ = std::fs::remove_file(&info.path);
            return Err(LedgerError::InvalidInput(
                "the backup was made but failed its check, so it was not kept".into(),
            ));
        }
        prune_kind(&dir, kind)?;
        *lock(&self.last_backup) = Some(info.clone());
        Ok(info)
    }

    /// Every backup of this Ledger, newest first (none for a temporary Ledger).
    pub fn backups(&self) -> Result<Vec<LedgerBackup>> {
        match self.backups_dir() {
            Some(dir) => list_backups(&dir),
            None => Ok(Vec::new()),
        }
    }

    /// The backups, and the restore waiting for the next start.
    pub fn backups_overview(&self) -> Result<LedgerBackups> {
        match self.path() {
            Some(path) => overview_at(path),
            None => Ok(LedgerBackups {
                backups: Vec::new(),
                pending_restore: None,
                folder: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_come_from_file_names() {
        let cases = [
            ("plenipo-backup-1790000000000.db", Some(BackupKind::Manual)),
            ("daily-backup-1790000000000.db", Some(BackupKind::Daily)),
            (
                "pre-upgrade-1.8.0-1790000000000.db",
                Some(BackupKind::BeforeUpgrade),
            ),
            (
                "pre-update-1.10.0-1790000000000.db",
                Some(BackupKind::BeforeUpdate),
            ),
            (
                "pre-migration-v8-1790000000000.db",
                Some(BackupKind::BeforeMigration),
            ),
            (
                "before-restore-1790000000000.db",
                Some(BackupKind::BeforeRestore),
            ),
            ("plenipo-export-1790000000000.json", None),
            ("plenipo.db", None),
            ("something-else.db", None),
        ];
        for (name, kind) in cases {
            assert_eq!(BackupKind::of_file(name), kind, "{name}");
        }
        assert_eq!(
            stamp_of("daily-backup-1790000000000-1.db"),
            Some(1790000000000)
        );
        assert_eq!(
            version_of(
                BackupKind::BeforeUpgrade,
                "pre-upgrade-1.8.0-1790000000000.db"
            ),
            Some("1.8.0".into())
        );
        assert_eq!(version_of(BackupKind::Daily, "daily-backup-1.db"), None);
    }

    #[test]
    fn names_with_folders_or_other_files_are_refused() {
        for bad in [
            "../plenipo.db",
            "..\\plenipo.db",
            "C:/Windows/evil.db",
            "backups/daily-backup-1790000000000.db",
            "plenipo.db",
            ".daily-backup-1790000000000.db",
            "",
        ] {
            assert!(plain_name(bad).is_err(), "{bad}");
        }
        assert!(plain_name("daily-backup-1790000000000.db").is_ok());
    }
}
