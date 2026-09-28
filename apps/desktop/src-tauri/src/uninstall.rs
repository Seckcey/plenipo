//! What the uninstaller asks of Plenipo (Phase 13). When the owner ticks "delete my Plenipo
//! data", the uninstaller runs `plenipo-desktop.exe --plenipo-forget-secrets` before it deletes
//! Plenipo's folder: the secrets Plenipo kept in Windows Credential Manager (the owner's, and
//! servers' sign-ins) are removed too, so nothing of the owner's is left behind. Nothing else
//! happens in this mode: no window, no tray, no work.

use std::path::{Path, PathBuf};

use plenipo_capabilities::vault::{self, SecretStore};
use plenipo_ledger::DB_FILE_NAME;

/// The argument the uninstaller passes.
pub const FORGET_SECRETS_ARG: &str = "--plenipo-forget-secrets";
/// Plenipo's identifier: its folder's name and its name in Credential Manager.
pub const IDENTIFIER: &str = "com.eightwest.plenipo";

/// Plenipo's own folder, as Tauri places it (`%LOCALAPPDATA%\com.eightwest.plenipo`).
fn data_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }?;
    Some(base.join(IDENTIFIER))
}

/// Remove every secret Plenipo keeps for the Ledger in `data`. Returns how many were removed.
pub fn forget_secrets(data: &Path, store: &dyn SecretStore) -> Result<usize, String> {
    let db = data.join("ledger").join(DB_FILE_NAME);
    if !db.exists() {
        return Ok(0);
    }
    // Read straight from the file: a Ledger from a newer Plenipo, or with damaged settings, must
    // still give up the names of the secrets Plenipo kept.
    let ids = match plenipo_ledger::setting_in_file(&db, plenipo_guard::SETTING) {
        Ok(Some(settings)) => vault::stored_ids_in(&settings),
        Ok(None) => Vec::new(),
        Err(e) => return Err(format!("the Ledger could not be read ({e})")),
    };
    let (removed, problems) = vault::forget_ids(store, &ids);
    if problems.is_empty() {
        Ok(removed)
    } else {
        Err(format!(
            "{removed} removed; these could not be: {}",
            problems.join("; ")
        ))
    }
}

/// Run the mode when asked for (in `main`, before anything else). Returns the exit code.
pub fn maybe_run_from_args(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    if args.next().as_deref() != Some(FORGET_SECRETS_ARG) {
        return None;
    }
    let Some(data) = data_dir() else {
        return Some(0);
    };
    let store = plenipo_capabilities::OsSecretStore::new(IDENTIFIER);
    Some(match forget_secrets(&data, &store) {
        Ok(n) => {
            println!("Removed {n} secret(s) Plenipo kept in Windows Credential Manager.");
            0
        }
        Err(e) => {
            eprintln!("Some secrets could not be removed: {e}");
            1
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use plenipo_capabilities::MemorySecretStore;
    use plenipo_guard::{Guard, SecretInput};
    use plenipo_ledger::Ledger;

    #[test]
    fn deleting_my_data_forgets_every_secret_plenipo_kept() {
        // The owner's secrets, and a connection's sign-in (Phase 20).
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("ledger").join(DB_FILE_NAME);
        // Like Windows Credential Manager: long values are kept in pieces.
        let store = MemorySecretStore::with_limit(1_280);
        {
            let ledger = Arc::new(Ledger::open(&db).unwrap());
            let guard = Guard::new(ledger);
            // A secret long enough to be kept in pieces, and a short one.
            vault::save(
                &guard,
                &store,
                &SecretInput {
                    id: None,
                    name: "GitHub token".into(),
                    env_var: Some("GH_TOKEN".into()),
                    programs: vec!["gh".into()],
                    value: Some("x".repeat(2_500)),
                },
            )
            .unwrap();
            vault::save(
                &guard,
                &store,
                &SecretInput {
                    id: None,
                    name: "Other".into(),
                    env_var: Some("TOOL_KEY".into()),
                    programs: vec!["tool".into()],
                    value: Some("short-secret-value".into()),
                },
            )
            .unwrap();
            // A connection's sign-in (Phase 20), long enough to be kept in pieces too.
            guard
                .connection_connected(
                    "microsoft365",
                    Some(plenipo_guard::AccountKind::Work),
                    plenipo_guard::Account::default(),
                    &[],
                )
                .unwrap();
            vault::put(
                &store,
                &plenipo_capabilities::connections::vault_id("microsoft365"),
                &"r".repeat(3_000),
            )
            .unwrap();
            let config = guard.config().unwrap();
            assert_eq!(config.secrets.len(), 2);
        }
        assert!(store.stored() > 2, "the long one is kept in pieces");
        // A later Plenipo changed the Ledger's layout (this one would refuse to open it) and
        // its permission settings no longer read as a whole: the secrets' names still do.
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at) \
                 VALUES (999, 'future', 'x', 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE settings SET value = json_set(value, '$.sets', 'not a list') \
                 WHERE key = 'guard'",
                [],
            )
            .unwrap();
        }
        assert!(Ledger::open(&db).is_err(), "a newer layout is refused");
        assert_eq!(forget_secrets(dir.path(), &store).unwrap(), 3);
        assert_eq!(store.stored(), 0, "nothing is left behind");
        // Again, or with no Ledger at all: nothing to do.
        assert_eq!(forget_secrets(dir.path(), &store).unwrap(), 0);
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(forget_secrets(empty.path(), &store).unwrap(), 0);
    }

    #[test]
    fn the_mode_is_only_for_its_own_argument() {
        let args = |a: &[&str]| {
            a.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert_eq!(maybe_run_from_args(args(&["plenipo.exe"])), None);
        assert_eq!(
            maybe_run_from_args(args(&["plenipo.exe", "--in-tray"])),
            None
        );
        assert_eq!(
            maybe_run_from_args(args(&["plenipo.exe", "x", FORGET_SECRETS_ARG])),
            None
        );
    }
}
