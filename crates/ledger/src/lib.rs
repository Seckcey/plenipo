//! Plenipo Ledger — the durable local system of record.
//!
//! One SQLite database (WAL, `synchronous=FULL`, foreign keys on) holds organization,
//! tasks, executions, approvals, artifacts, and an **append-only** event trail. Every
//! mutation that matters writes its event in the same transaction, so the trail is always
//! complete and ordered. See ADR-006.

pub mod dto;
pub mod error;
mod events;
mod liaison;
mod maintenance;
pub mod migrate;
mod org;
mod records;
mod rows;
mod sessions;
mod tasks;
pub mod workforce;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode};

pub use dto::*;
pub use error::{LedgerError, Result};
pub use migrate::{Migration, MIGRATIONS};
pub use workforce::RoleTemplate;

/// Called after every committed event (e.g. to stream it to the UI).
pub type Listener = Arc<dyn Fn(&LedgerEvent) + Send + Sync>;

pub const DB_FILE_NAME: &str = "plenipo.db";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Total time a write waits for the write lock. SQLite's busy handler is not fair between
/// connections, so a writer can lose several 5 s rounds under contention (notably on Windows,
/// where every `synchronous=FULL` commit flushes to disk). Keep retrying, then give up loudly.
const WRITE_LOCK_WAIT: Duration = Duration::from_secs(60);

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub struct Ledger {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
    listeners: RwLock<Vec<Listener>>,
    notices: Mutex<Vec<String>>,
    last_integrity: Mutex<Option<IntegrityReport>>,
    last_backup: Mutex<Option<BackupInfo>>,
}

impl Ledger {
    /// Open (or create) the ledger at `path` with the built-in migrations.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, MIGRATIONS)
    }

    /// Open with an explicit migration list (tests use this to simulate upgrades).
    ///
    /// A file that fails SQLite's `quick_check` (or is not a database at all) is quarantined
    /// next to the original and a new ledger is started; a notice explains what happened.
    /// A database whose schema is newer than this build is refused with
    /// [`LedgerError::NewerSchema`] and left untouched.
    pub fn open_with(path: &Path, migrations: &[Migration]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut notices = Vec::new();
        let existed = path.exists();
        let conn = match open_checked(path) {
            Ok(conn) => conn,
            Err(reason) if existed => {
                let moved = quarantine(path)?;
                notices.push(format!(
                    "The ledger database failed its integrity check ({reason}). It was moved to \
                     {} and a new, empty ledger was started. Backups are in {}.",
                    moved.display(),
                    backups_dir(path).display()
                ));
                open_checked(path).map_err(LedgerError::InvalidInput)?
            }
            Err(reason) => return Err(LedgerError::InvalidInput(reason)),
        };
        configure(&conn, true)?;
        let backup_dir = backups_dir(path);
        let report = migrate::migrate(&conn, migrations, |from| {
            let info =
                maintenance::snapshot(&conn, &backup_dir, &format!("pre-migration-v{from}"))?;
            notices.push(format!(
                "The ledger schema was upgraded from version {from}. A backup of the previous \
                 version was saved to {}.",
                info.path
            ));
            Ok(())
        })?;
        let _ = report;
        Ok(Self::from_parts(conn, Some(path.to_path_buf()), notices))
    }

    /// A private, non-persistent ledger (tests, or fallback when the file cannot be opened).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        configure(&conn, false)?;
        migrate::migrate(&conn, MIGRATIONS, |_| Ok(()))?;
        Ok(Self::from_parts(conn, None, vec![]))
    }

    fn from_parts(conn: Connection, path: Option<PathBuf>, notices: Vec<String>) -> Self {
        Self {
            conn: Mutex::new(conn),
            path,
            listeners: RwLock::new(Vec::new()),
            notices: Mutex::new(notices),
            last_integrity: Mutex::new(None),
            last_backup: Mutex::new(None),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Default location for backups and exports of this ledger.
    pub fn backups_dir(&self) -> Option<PathBuf> {
        self.path.as_deref().map(backups_dir)
    }

    /// Add a listener called after every committed event, in commit order. Listeners run on
    /// the writing thread after the transaction and its lock are released: keep them quick.
    pub fn add_listener(&self, listener: Listener) {
        self.listeners
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .push(listener);
    }

    /// Add a notice for the user (shown with the ledger status).
    pub fn add_notice(&self, notice: impl Into<String>) {
        let notice = notice.into();
        let mut notices = lock(&self.notices);
        if !notices.contains(&notice) {
            notices.push(notice);
        }
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        lock(&self.conn)
    }

    /// Run `f` in an IMMEDIATE transaction; notify the listeners of its events after commit.
    /// Any error rolls the whole transaction back.
    fn write<T>(
        &self,
        f: impl FnOnce(&Connection, &mut Vec<LedgerEvent>) -> Result<T>,
    ) -> Result<T> {
        let mut events = Vec::new();
        let value = {
            let conn = self.conn();
            begin_immediate(&conn)?;
            let outcome = f(&conn, &mut events)
                .and_then(|v| conn.execute_batch("COMMIT").map(|()| v).map_err(Into::into));
            match outcome {
                Ok(value) => value,
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        };
        let listeners = self
            .listeners
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for event in &events {
            for listener in &listeners {
                listener(event);
            }
        }
        Ok(value)
    }

    fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        f(&self.conn())
    }
}

/// Take the write lock, retrying with backoff while another connection holds it.
fn begin_immediate(conn: &Connection) -> Result<()> {
    // A panic inside a previous write could have left a transaction open on this connection.
    if !conn.is_autocommit() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let deadline = Instant::now() + WRITE_LOCK_WAIT;
    let mut attempt: u64 = 0;
    loop {
        match conn.execute_batch("BEGIN IMMEDIATE") {
            Ok(()) => return Ok(()),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == ErrorCode::DatabaseBusy && Instant::now() < deadline =>
            {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(10 * attempt.min(20)));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn backups_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map_or_else(|| PathBuf::from("backups"), |p| p.join("backups"))
}

/// Open and run `quick_check`. Any failure is returned as a human-readable reason.
fn open_checked(path: &Path) -> std::result::Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.busy_timeout(BUSY_TIMEOUT).map_err(|e| e.to_string())?;
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if result == "ok" {
        Ok(conn)
    } else {
        Err(result)
    }
}

fn configure(conn: &Connection, on_disk: bool) -> Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    if on_disk {
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(LedgerError::InvalidInput(format!(
                "could not enable WAL journaling (got {mode})"
            )));
        }
    }
    // Durability over speed: this is an audit ledger.
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")?;
    Ok(())
}

/// Move a damaged database (and its WAL/SHM files) aside. Returns the new main-file path.
fn quarantine(path: &Path) -> Result<PathBuf> {
    let stamp = now_ms();
    let name = path
        .file_name()
        .map_or_else(|| "ledger".into(), |n| n.to_string_lossy().into_owned());
    let target = path.with_file_name(format!("{name}.corrupt-{stamp}"));
    std::fs::rename(path, &target)?;
    for suffix in ["-wal", "-shm"] {
        let side = path.with_file_name(format!("{name}{suffix}"));
        if side.exists() {
            let _ = std::fs::rename(
                &side,
                path.with_file_name(format!("{name}{suffix}.corrupt-{stamp}")),
            );
        }
    }
    Ok(target)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub fn ledger() -> Ledger {
        Ledger::open_in_memory().unwrap()
    }

    pub fn task(ledger: &Ledger, objective: &str) -> Task {
        ledger
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: objective.into(),
                    priority: 2,
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap()
    }
}
