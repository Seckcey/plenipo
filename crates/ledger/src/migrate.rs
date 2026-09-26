//! Schema migrations.
//!
//! Strategy (ADR-006): migrations are numbered and embedded. Each has an `up` and a `down`
//! script and is recorded with a checksum. Plenipo only ever migrates **forward**; before it
//! upgrades a database that already holds a schema, it takes a backup, which is the production
//! rollback path. `down` scripts exist for development and are tested round-trip. A database
//! with a schema newer than the app, an edited migration, or a gap in history is refused.

use rusqlite::{Connection, OptionalExtension as _};

use crate::error::{LedgerError, Result};

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up: &'static str,
    pub down: &'static str,
}

impl Migration {
    /// FNV-1a 64-bit hash of the `up` script: detects edits to applied migrations.
    pub fn checksum(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.up.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{hash:016x}")
    }
}

/// Every migration this build knows, in order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        up: include_str!("../migrations/0001_initial.up.sql"),
        down: include_str!("../migrations/0001_initial.down.sql"),
    },
    Migration {
        version: 2,
        name: "runtime_sessions",
        up: include_str!("../migrations/0002_runtime_sessions.up.sql"),
        down: include_str!("../migrations/0002_runtime_sessions.down.sql"),
    },
    Migration {
        version: 3,
        name: "liaison_messages",
        up: include_str!("../migrations/0003_liaison_messages.up.sql"),
        down: include_str!("../migrations/0003_liaison_messages.down.sql"),
    },
    Migration {
        version: 4,
        name: "workforce",
        up: include_str!("../migrations/0004_workforce.up.sql"),
        down: include_str!("../migrations/0004_workforce.down.sql"),
    },
];

/// Highest version in `migrations` (0 if none).
pub fn latest(migrations: &[Migration]) -> u32 {
    migrations.iter().map(|m| m.version).max().unwrap_or(0)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub from: u32,
    pub to: u32,
    pub applied: Vec<u32>,
}

fn ensure_history_table(conn: &Connection) -> Result<()> {
    // Check first (a read) so opening an up-to-date ledger never competes for the write lock.
    let exists: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_some() {
        return Ok(());
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT NOT NULL,
             checksum   TEXT NOT NULL,
             applied_at INTEGER NOT NULL
         );",
    )?;
    Ok(())
}

fn applied(conn: &Connection) -> Result<Vec<(u32, String)>> {
    let mut stmt =
        conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Current schema version (0 for an empty database).
pub fn current_version(conn: &Connection) -> Result<u32> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(0);
    }
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?)
}

/// Validate history and apply pending migrations, each in its own transaction.
/// `before_upgrade` runs once, before the first migration, when upgrading a database that
/// already has a schema (not when creating a new one) — used to take a backup.
pub fn migrate(
    conn: &Connection,
    migrations: &[Migration],
    before_upgrade: impl FnOnce(u32) -> Result<()>,
) -> Result<MigrationReport> {
    ensure_history_table(conn)?;
    let history = applied(conn)?;
    let app = latest(migrations);
    let from = history.iter().map(|(v, _)| *v).max().unwrap_or(0);

    for (version, checksum) in &history {
        match migrations.iter().find(|m| m.version == *version) {
            None if *version > app => return Err(LedgerError::NewerSchema { db: from, app }),
            None => {
                return Err(LedgerError::InconsistentMigrations(format!(
                    "applied migration {version} is unknown to this build"
                )))
            }
            Some(m) if m.checksum() != *checksum => {
                return Err(LedgerError::ModifiedMigration(*version))
            }
            Some(_) => {}
        }
    }
    for m in migrations.iter().filter(|m| m.version <= from) {
        if !history.iter().any(|(v, _)| *v == m.version) {
            return Err(LedgerError::InconsistentMigrations(format!(
                "migration {} was skipped",
                m.version
            )));
        }
    }

    let mut pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > from).collect();
    pending.sort_by_key(|m| m.version);
    if pending.is_empty() {
        return Ok(MigrationReport {
            from,
            to: from,
            applied: vec![],
        });
    }
    if from > 0 {
        before_upgrade(from)?;
    }
    let mut report = MigrationReport {
        from,
        to: from,
        applied: vec![],
    };
    for m in pending {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(m.up)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![m.version, m.name, m.checksum(), crate::now_ms() as i64],
        )?;
        tx.commit()?;
        report.to = m.version;
        report.applied.push(m.version);
    }
    Ok(report)
}

/// Roll back to `target` by running `down` scripts in reverse. Development and tests only;
/// production restores the pre-migration backup instead.
pub fn rollback_to(conn: &Connection, migrations: &[Migration], target: u32) -> Result<()> {
    let mut to_undo: Vec<&Migration> = migrations
        .iter()
        .filter(|m| m.version > target && m.version <= current_version(conn).unwrap_or(0))
        .collect();
    to_undo.sort_by_key(|m| std::cmp::Reverse(m.version));
    for m in to_undo {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(m.down)?;
        tx.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            [m.version],
        )?;
        tx.commit()?;
    }
    Ok(())
}
