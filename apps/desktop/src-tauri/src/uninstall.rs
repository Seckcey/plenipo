//! What the uninstaller asks of Plenipo (Phase 13). When the owner ticks "delete my Plenipo
//! data", the uninstaller runs `plenipo-desktop.exe --plenipo-forget-secrets` before it deletes
//! Plenipo's folder: the secrets Plenipo kept in Windows Credential Manager (the owner's, and
//! servers' sign-ins, and the license key) are removed too, so nothing of the owner's is left
//! behind. Nothing else happens in this mode: no window, no tray, no work.

use std::path::{Path, PathBuf};

use plenipo_capabilities::vault::{self, SecretStore};
use plenipo_ledger::DB_FILE_NAME;

/// The argument the uninstaller passes.
pub const FORGET_SECRETS_ARG: &str = "--plenipo-forget-secrets";
/// Plenipo's identifier: its folder's name and its name in Credential Manager.
pub const IDENTIFIER: &str = "com.eightwest.plenipo";

/// Plenipo's own folder, as Tauri places it: `%LOCALAPPDATA%\com.eightwest.plenipo` on Windows,
/// `~/Library/Application Support/com.eightwest.plenipo` on a Mac (Phase 23), and
/// `~/.local/share/com.eightwest.plenipo` on Linux.
fn data_dir() -> Option<PathBuf> {
    data_dir_with(|name| std::env::var_os(name))
}

/// [`data_dir`], reading the settings it needs through `var` (only full paths count).
fn data_dir_with(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let path = |name: &str| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = if cfg!(windows) {
        path("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        path("HOME").map(|home| home.join("Library").join("Application Support"))
    } else {
        path("XDG_DATA_HOME").or_else(|| path("HOME").map(|home| home.join(".local").join("share")))
    }?;
    Some(base.join(IDENTIFIER))
}

/// Remove every secret Plenipo keeps for the Ledger in `data`. Returns how many were removed.
pub fn forget_secrets(data: &Path, store: &dyn SecretStore) -> Result<usize, String> {
    let ids = saved_ids(data)?;
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

/// The names of the secrets the Ledger in `data` says Plenipo kept (none without a Ledger).
fn saved_ids(data: &Path) -> Result<Vec<String>, String> {
    let db = data.join("ledger").join(DB_FILE_NAME);
    if !db.exists() {
        return Ok(Vec::new());
    }
    // Read straight from the file: a Ledger from a newer Plenipo, or with damaged settings, must
    // still give up the names of the secrets Plenipo kept.
    match plenipo_ledger::setting_in_file(&db, plenipo_guard::SETTING) {
        Ok(Some(settings)) => Ok(vault::stored_ids_in(&settings)),
        Ok(None) => Ok(Vec::new()),
        Err(e) => Err(format!("the Ledger could not be read ({e})")),
    }
}

/// Whether any organization in `data` says it saved a secret (a Ledger that cannot be read
/// might have: it counts as yes).
pub fn any_saved_ids(data: &Path) -> bool {
    organization_folders(data)
        .iter()
        .any(|(_, folder)| saved_ids(folder).map_or(true, |ids| !ids.is_empty()))
}

/// Every organization's folder in `data` (Phase 21, ADR-094 §9): the first one's is `data`
/// itself; the others are in `organizations/`, each with its own name in the Vault.
pub fn organization_folders(data: &Path) -> Vec<(String, PathBuf)> {
    let mut all = vec![(crate::orgs::FIRST.to_owned(), data.to_path_buf())];
    if let Ok(entries) = std::fs::read_dir(data.join(crate::orgs::FOLDER)) {
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            if crate::orgs::is_org_id(&name) && name != crate::orgs::FIRST && e.path().is_dir() {
                all.push((name, e.path()));
            }
        }
    }
    all
}

/// Remove every secret each organization in `data` kept, each from its own name in the Vault
/// (`store_for`). Returns how many were removed.
pub fn forget_every_organizations_secrets(
    data: &Path,
    store_for: impl Fn(&str) -> Box<dyn SecretStore>,
) -> Result<usize, String> {
    let mut removed = 0;
    let mut problems = Vec::new();
    for (id, folder) in organization_folders(data) {
        let store = store_for(&id);
        match forget_secrets(&folder, store.as_ref()) {
            Ok(n) => removed += n,
            Err(e) => problems.push(e),
        }
        // The PC's license key (Phase 11A), kept under the first organization's name.
        if id == crate::orgs::FIRST {
            match forget_one(store.as_ref(), crate::license_host::VAULT_ID) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(e) => problems.push(format!("the license key: {e}")),
            }
            // The license record's copy, next to the key (P-DESK-1).
            match forget_one(store.as_ref(), crate::license_host::RECORD_COPY_ID) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(e) => problems.push(format!("the license record's copy: {e}")),
            }
            // Community's keys for this PC (Phase 24), under the same name.
            match forget_one(store.as_ref(), plenipo_community::keys::KEYS_ID) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(e) => problems.push(format!("Community's keys: {e}")),
            }
            // Phone access's keys and phones (Phase 14), under the same name.
            match forget_phone_access(store.as_ref()) {
                Ok(n) => removed += n,
                Err(e) => problems.push(format!("phone access: {e}")),
            }
        }
    }
    if problems.is_empty() {
        Ok(removed)
    } else {
        Err(format!("{removed} removed; {}", problems.join("; ")))
    }
}

/// Remove phone access's keys and phones (Phase 14, ADR-141 §8), kept under the first
/// organization's name. How many were removed.
fn forget_phone_access(store: &dyn SecretStore) -> Result<usize, String> {
    use plenipo_remote::devices::{DEVICE_PREFIX, KEYS_ID, LIST_ID};
    let mut ids = vec![KEYS_ID.to_owned(), LIST_ID.to_owned()];
    if let Some(list) = vault::read(store, LIST_ID)? {
        let phones: Vec<String> = serde_json::from_str(&list).unwrap_or_default();
        ids.extend(phones.into_iter().map(|p| format!("{DEVICE_PREFIX}{p}")));
    }
    let (removed, problems) = vault::forget_ids(store, &ids);
    if problems.is_empty() {
        Ok(removed)
    } else {
        Err(problems.join("; "))
    }
}

/// Remove one secret (the license key, Community's keys), when it is kept. True when it was.
fn forget_one(store: &dyn SecretStore, id: &str) -> Result<bool, String> {
    if vault::read(store, id)?.is_none() {
        return Ok(false);
    }
    vault::erase(store, id).map(|()| true)
}

/// Delete Plenipo's own folders (Phase 23: "Delete my Plenipo data" on a Mac and Linux, where
/// removing Plenipo never touches the owner's home folder). A folder is deleted only when its
/// name is Plenipo's own identifier; one already gone is fine.
pub fn delete_folders(folders: &[PathBuf]) -> Result<(), String> {
    let mut problems = Vec::new();
    for folder in folders {
        if folder.file_name().and_then(|n| n.to_str()) != Some(IDENTIFIER) {
            problems.push(format!(
                "{} is not Plenipo's own folder, so it was left alone",
                folder.display()
            ));
            continue;
        }
        match std::fs::remove_dir_all(folder) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => problems.push(format!("{} ({e})", folder.display())),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// What a Mac keeps for Plenipo outside Plenipo's own folders (Phase 23): its settings file and
/// its saved window state, each named for Plenipo. "Delete my Plenipo data" removes them too.
pub fn mac_leftovers(home: &Path) -> [PathBuf; 2] {
    let library = home.join("Library");
    [
        library
            .join("Preferences")
            .join(format!("{IDENTIFIER}.plist")),
        library
            .join("Saved Application State")
            .join(format!("{IDENTIFIER}.savedState")),
    ]
}

/// Delete each of `paths` (a file or a folder) only when it is one of [`mac_leftovers`]' names;
/// one already gone is fine, and a link is removed, never followed.
pub fn delete_leftovers(paths: &[PathBuf]) -> Result<(), String> {
    let allowed = [
        format!("{IDENTIFIER}.plist"),
        format!("{IDENTIFIER}.savedState"),
    ];
    let mut problems = Vec::new();
    for path in paths {
        let named = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| allowed.iter().any(|a| a == n));
        if !named {
            problems.push(format!(
                "{} is not one of Plenipo's own files, so it was left alone",
                path.display()
            ));
            continue;
        }
        let removed = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
            Ok(_) => std::fs::remove_file(path),
            Err(e) => Err(e),
        };
        match removed {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => problems.push(format!("{} ({e})", path.display())),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
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
    let forgotten = forget_every_organizations_secrets(&data, |id| {
        Box::new(plenipo_capabilities::OsSecretStore::new(
            crate::orgs::vault_name(IDENTIFIER, id),
        ))
    });
    Some(match forgotten {
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
    fn deleting_my_data_forgets_every_organizations_secrets_under_its_own_name() {
        let dir = tempfile::tempdir().unwrap();
        let other = crate::orgs::new_id();
        let stores: std::collections::HashMap<String, Arc<MemorySecretStore>> = [
            (
                crate::orgs::FIRST.to_owned(),
                Arc::new(MemorySecretStore::default()),
            ),
            (other.clone(), Arc::new(MemorySecretStore::default())),
        ]
        .into_iter()
        .collect();
        for (id, store) in &stores {
            let folder = crate::orgs::folder_of(dir.path(), id);
            let ledger = Arc::new(Ledger::open(&folder.join("ledger").join(DB_FILE_NAME)).unwrap());
            let guard = Guard::new(ledger);
            vault::save(
                &guard,
                store.as_ref(),
                &SecretInput {
                    id: None,
                    name: format!("Token of {id}"),
                    env_var: None,
                    programs: Vec::new(),
                    value: Some("s3cret".into()),
                },
            )
            .unwrap();
            assert_eq!(store.stored(), 1);
        }
        // The PC's license key (Phase 11A), under the first organization's name.
        vault::put(
            stores[crate::orgs::FIRST].as_ref(),
            crate::license_host::VAULT_ID,
            "plenipo1.key",
        )
        .unwrap();
        // The license record's copy (P-DESK-1), next to the key.
        vault::put(
            stores[crate::orgs::FIRST].as_ref(),
            crate::license_host::RECORD_COPY_ID,
            "{\"keyId\":\"lk_x\"}",
        )
        .unwrap();
        // Community's keys for this PC (Phase 24), under the same name.
        vault::put(
            stores[crate::orgs::FIRST].as_ref(),
            plenipo_community::keys::KEYS_ID,
            "{\"kept\":1}",
        )
        .unwrap();
        // A folder that is not an organization's is left alone.
        std::fs::create_dir_all(dir.path().join(crate::orgs::FOLDER).join("notes")).unwrap();
        assert!(any_saved_ids(dir.path()));
        let removed = forget_every_organizations_secrets(dir.path(), |id| {
            Box::new(Shared(Arc::clone(&stores[id]))) as Box<dyn SecretStore>
        })
        .unwrap();
        assert_eq!(removed, 5);
        assert!(stores.values().all(|s| s.stored() == 0));
        // A PC where nothing was ever saved says so.
        let empty = tempfile::tempdir().unwrap();
        assert!(!any_saved_ids(empty.path()));
    }

    /// One organization's store, shared with the test.
    struct Shared(Arc<MemorySecretStore>);

    impl SecretStore for Shared {
        fn label(&self) -> &str {
            self.0.label()
        }
        fn check(&self) -> std::result::Result<(), String> {
            self.0.check()
        }
        fn set(&self, id: &str, value: &str) -> std::result::Result<(), String> {
            self.0.set(id, value)
        }
        fn get(&self, id: &str) -> std::result::Result<Option<String>, String> {
            self.0.get(id)
        }
        fn delete(&self, id: &str) -> std::result::Result<(), String> {
            self.0.delete(id)
        }
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

    /// Phase 23: only Plenipo's own folders are deleted, and one already gone is fine.
    #[test]
    fn only_plenipos_own_folders_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let own = dir.path().join(IDENTIFIER);
        std::fs::create_dir_all(own.join("ledger")).unwrap();
        std::fs::write(own.join("ledger").join("plenipo.db"), b"x").unwrap();
        let other = dir.path().join("Documents");
        std::fs::create_dir_all(&other).unwrap();
        let gone = dir.path().join("cache").join(IDENTIFIER);
        let why = delete_folders(&[own.clone(), other.clone(), gone]).unwrap_err();
        assert!(!own.exists());
        assert!(other.exists(), "never another folder");
        assert!(why.contains("not Plenipo's own folder"), "{why}");
        assert!(delete_folders(&[own]).is_ok(), "already gone");
    }

    /// Phase 23: the data folder is where Tauri puts it on each system (on a Mac, Application
    /// Support, not Linux's folder).
    #[test]
    fn the_data_folder_is_each_systems_own() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let vars = |name: &str| match name {
            "HOME" => Some(home.clone().into_os_string()),
            "LOCALAPPDATA" => Some(home.join("AppData").join("Local").into_os_string()),
            _ => None,
        };
        let expected = if cfg!(windows) {
            home.join("AppData").join("Local")
        } else if cfg!(target_os = "macos") {
            home.join("Library").join("Application Support")
        } else {
            home.join(".local").join("share")
        };
        assert_eq!(data_dir_with(vars).unwrap(), expected.join(IDENTIFIER));
        assert_eq!(data_dir_with(|_| Some("relative".into())), None);
    }

    /// Phase 23: a Mac's settings file and saved window state go too, and nothing else.
    #[test]
    fn only_plenipos_own_mac_files_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let [plist, saved] = mac_leftovers(dir.path());
        assert!(plist.ends_with("Library/Preferences/com.eightwest.plenipo.plist"));
        assert!(saved.ends_with("Library/Saved Application State/com.eightwest.plenipo.savedState"));
        std::fs::create_dir_all(plist.parent().unwrap()).unwrap();
        std::fs::write(&plist, b"<plist/>").unwrap();
        std::fs::create_dir_all(saved.join("windows")).unwrap();
        let other = plist.with_file_name("com.apple.finder.plist");
        std::fs::write(&other, b"<plist/>").unwrap();
        let why = delete_leftovers(&[plist.clone(), saved.clone(), other.clone()]).unwrap_err();
        assert!(!plist.exists() && !saved.exists());
        assert!(other.exists(), "never another program's settings");
        assert!(why.contains("not one of Plenipo's own files"), "{why}");
        assert!(
            delete_leftovers(&mac_leftovers(dir.path())).is_ok(),
            "already gone"
        );
    }
}
