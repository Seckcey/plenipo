//! Plenipo Vault: secret values live in the operating system's protected storage (Windows
//! Credential Manager, the macOS Keychain, the Linux kernel keyring); Plenipo keeps only a
//! reference — the secret's name, and which programs get it as which environment variable
//! (the secret-reference model, in Guard's settings). A value is written once, read only to
//! give it to an allowed program or to hide it in text, and never shown or recorded.

use std::collections::HashMap;
use std::sync::Mutex;

use plenipo_guard::{Guard, GuardError, SecretInfo, SecretInput};

use crate::error::{BrokerError, Result};

/// Longest secret value accepted.
pub const MAX_SECRET_CHARS: usize = 10_000;
/// Longest piece kept in one entry. Windows Credential Manager holds at most 2,560 bytes per
/// entry, stored as UTF-16 (1,280 characters), so a longer value — such as an RSA private key —
/// is kept in pieces (Phase 11).
pub const PIECE_CHARS: usize = 1_000;
/// What the first entry of a value kept in pieces holds, before the number of pieces.
const PIECES: &str = "plenipo-pieces/v1:";

/// Where secret values are kept.
pub trait SecretStore: Send + Sync + 'static {
    /// What it is called on this computer ("Windows Credential Manager").
    fn label(&self) -> &str;
    /// Whether it can be used here (`Err`: why not).
    fn check(&self) -> std::result::Result<(), String>;
    fn set(&self, id: &str, value: &str) -> std::result::Result<(), String>;
    fn get(&self, id: &str) -> std::result::Result<Option<String>, String>;
    fn delete(&self, id: &str) -> std::result::Result<(), String>;
}

/// The operating system's store, under Plenipo's own service name.
pub struct OsSecretStore {
    service: String,
}

impl OsSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, id: &str) -> std::result::Result<keyring::Entry, String> {
        keyring::Entry::new(&self.service, id).map_err(|e| e.to_string())
    }
}

impl SecretStore for OsSecretStore {
    fn label(&self) -> &str {
        if cfg!(windows) {
            "Windows Credential Manager"
        } else if cfg!(target_os = "macos") {
            "the macOS Keychain"
        } else {
            "the Linux kernel keyring"
        }
    }

    fn check(&self) -> std::result::Result<(), String> {
        match self.entry("plenipo-availability-check")?.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, id: &str, value: &str) -> std::result::Result<(), String> {
        self.entry(id)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn get(&self, id: &str) -> std::result::Result<Option<String>, String> {
        match self.entry(id)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn delete(&self, id: &str) -> std::result::Result<(), String> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// A store in memory (tests, and a fallback where the operating system has none).
#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
    /// When set, every operation fails with this (to test an unavailable store).
    pub broken: Option<String>,
    /// When set, longer values are refused, as Windows Credential Manager refuses more than
    /// 1,280 characters.
    pub limit: Option<usize>,
}

impl MemorySecretStore {
    /// A store that, like Windows Credential Manager, refuses values longer than `chars`.
    pub fn with_limit(chars: usize) -> Self {
        Self {
            limit: Some(chars),
            ..Self::default()
        }
    }

    /// How many values it holds (tests).
    pub fn stored(&self) -> usize {
        self.values.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    fn map(
        &self,
    ) -> std::result::Result<std::sync::MutexGuard<'_, HashMap<String, String>>, String> {
        if let Some(why) = &self.broken {
            return Err(why.clone());
        }
        Ok(self.values.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

impl SecretStore for MemorySecretStore {
    fn label(&self) -> &str {
        "memory (test)"
    }

    fn check(&self) -> std::result::Result<(), String> {
        self.map().map(|_| ())
    }

    fn set(&self, id: &str, value: &str) -> std::result::Result<(), String> {
        if self.limit.is_some_and(|l| value.chars().count() > l) {
            return Err("the value is too long for one entry".into());
        }
        self.map()?.insert(id.to_owned(), value.to_owned());
        Ok(())
    }

    fn get(&self, id: &str) -> std::result::Result<Option<String>, String> {
        Ok(self.map()?.get(id).cloned())
    }

    fn delete(&self, id: &str) -> std::result::Result<(), String> {
        self.map()?.remove(id);
        Ok(())
    }
}

fn piece_id(id: &str, n: usize) -> String {
    format!("{id}.piece{n}")
}

/// Keep `value` under `id`: in one entry, or in pieces when it is longer than [`PIECE_CHARS`].
/// Pieces from an earlier, longer value are removed.
pub fn put(store: &dyn SecretStore, id: &str, value: &str) -> std::result::Result<(), String> {
    let chars: Vec<char> = value.chars().collect();
    let earlier = pieces_of(store, id)?;
    if chars.len() <= PIECE_CHARS {
        store.set(id, value)?;
    } else {
        let pieces: Vec<String> = chars
            .chunks(PIECE_CHARS)
            .map(|c| c.iter().collect())
            .collect();
        for (n, piece) in pieces.iter().enumerate() {
            store.set(&piece_id(id, n + 1), piece)?;
        }
        store.set(id, &format!("{PIECES}{}", pieces.len()))?;
        for n in pieces.len() + 1..=earlier {
            store.delete(&piece_id(id, n))?;
        }
        return Ok(());
    }
    for n in 1..=earlier {
        store.delete(&piece_id(id, n))?;
    }
    Ok(())
}

/// How many pieces the value under `id` is kept in (0: one entry, or none).
fn pieces_of(store: &dyn SecretStore, id: &str) -> std::result::Result<usize, String> {
    Ok(store
        .get(id)?
        .and_then(|v| v.strip_prefix(PIECES).and_then(|n| n.parse().ok()))
        .unwrap_or(0))
}

/// The value kept under `id` (joined again if it is kept in pieces).
pub fn read(store: &dyn SecretStore, id: &str) -> std::result::Result<Option<String>, String> {
    let Some(first) = store.get(id)? else {
        return Ok(None);
    };
    let Some(n) = first
        .strip_prefix(PIECES)
        .and_then(|n| n.parse::<usize>().ok())
    else {
        return Ok(Some(first));
    };
    let mut value = String::new();
    for i in 1..=n {
        match store.get(&piece_id(id, i))? {
            Some(piece) => value.push_str(&piece),
            None => return Err(format!("part {i} of {n} of the stored value is missing")),
        }
    }
    Ok(Some(value))
}

/// Remove the value kept under `id`, with its pieces.
pub fn erase(store: &dyn SecretStore, id: &str) -> std::result::Result<(), String> {
    for n in 1..=pieces_of(store, id)? {
        store.delete(&piece_id(id, n))?;
    }
    store.delete(id)
}

/// Every value Plenipo keeps in `store` for `config`: the owner's secrets, the servers'
/// sign-ins, and the connections' sign-ins (Phase 20). Uninstalling with "delete my data"
/// removes them all (Phase 13).
pub fn stored_ids(config: &plenipo_guard::GuardConfig) -> Vec<String> {
    config
        .secrets
        .iter()
        .map(|s| s.id.clone())
        .chain(
            config
                .servers
                .iter()
                .flat_map(|s| crate::broker::servers::vault_ids(&s.id)),
        )
        .chain(crate::connections::Connections::vault_ids(config))
        .collect()
}

/// The IDs [`stored_ids`] gives, read leniently from the permission settings as stored (JSON),
/// so settings that no longer read as a whole still give up the secrets' names.
pub fn stored_ids_in(settings: &serde_json::Value) -> Vec<String> {
    let ids = |list: &str| -> Vec<String> {
        settings
            .get(list)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|i| i.get("id").and_then(|id| id.as_str()).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    ids("secrets")
        .into_iter()
        .chain(
            ids("servers")
                .iter()
                .flat_map(|s| crate::broker::servers::vault_ids(s)),
        )
        .chain(
            ids("connections")
                .iter()
                .map(|c| crate::connections::vault_id(c)),
        )
        .collect()
}

/// Remove every value Plenipo keeps for `config` (with their pieces). Returns how many were
/// removed and what could not be.
pub fn forget_all(
    store: &dyn SecretStore,
    config: &plenipo_guard::GuardConfig,
) -> (usize, Vec<String>) {
    forget_ids(store, &stored_ids(config))
}

/// Remove the values kept under `ids` (with their pieces).
pub fn forget_ids(store: &dyn SecretStore, ids: &[String]) -> (usize, Vec<String>) {
    let mut removed = 0;
    let mut problems = Vec::new();
    for id in ids {
        let there = matches!(store.get(id), Ok(Some(_)));
        match erase(store, id) {
            Ok(()) if there => removed += 1,
            Ok(()) => {}
            Err(e) => problems.push(format!("{id}: {e}")),
        }
    }
    (removed, problems)
}

fn unavailable(label: &str, why: &str) -> BrokerError {
    BrokerError::Invalid(format!(
        "Plenipo could not use {label} to keep the secret: {why}"
    ))
}

fn check_value(value: &str) -> Result<()> {
    if value.is_empty()
        || value.chars().count() > MAX_SECRET_CHARS
        || value.contains('\0')
        || value.starts_with(PIECES)
    {
        return Err(BrokerError::Invalid(format!(
            "a secret's value must be 1–{MAX_SECRET_CHARS} characters"
        )));
    }
    Ok(())
}

/// Save a secret: its value to `store`, its reference to Guard's settings. A new secret needs
/// a value; a changed one keeps its stored value unless a new one is given.
pub fn save(guard: &Guard, store: &dyn SecretStore, input: &SecretInput) -> Result<SecretInfo> {
    let value = input.value.as_deref().filter(|v| !v.is_empty());
    if let Some(v) = value {
        check_value(v)?;
    } else if input.id.is_none() {
        return Err(BrokerError::Invalid("enter the secret's value".into()));
    }
    // Never pass the value on with the reference.
    let reference = SecretInput {
        value: None,
        ..input.clone()
    };
    match &input.id {
        Some(id) => {
            if let Some(v) = value {
                put(store, id, v).map_err(|e| unavailable(store.label(), &e))?;
            }
            Ok(guard.save_secret(&reference)?)
        }
        None => {
            let info = guard.save_secret(&reference)?;
            if let Err(e) = put(store, &info.id, value.unwrap_or_default()) {
                // Keep no reference to a value that was not stored.
                let _ = guard.remove_secret(&info.id);
                return Err(unavailable(store.label(), &e));
            }
            Ok(info)
        }
    }
}

/// Remove a secret's value and its reference.
pub fn remove(guard: &Guard, store: &dyn SecretStore, id: &str) -> Result<SecretInfo> {
    erase(store, id).map_err(|e| unavailable(store.label(), &e))?;
    guard.remove_secret(id).map_err(|e| match e {
        GuardError::Invalid(m) => BrokerError::Invalid(m),
        other => other.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn guard() -> Guard {
        Guard::new(Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap()))
    }

    #[test]
    fn values_go_to_the_store_and_references_to_the_settings() {
        let g = guard();
        let store = MemorySecretStore::default();
        assert!(save(
            &g,
            &store,
            &SecretInput {
                name: "No value".into(),
                ..SecretInput::default()
            }
        )
        .is_err());
        let info = save(
            &g,
            &store,
            &SecretInput {
                name: "GitHub token".into(),
                env_var: Some("GH_TOKEN".into()),
                programs: vec!["gh".into()],
                value: Some("ghp_supersecretvalue".into()),
                ..SecretInput::default()
            },
        )
        .unwrap();
        assert_eq!(
            store.get(&info.id).unwrap().as_deref(),
            Some("ghp_supersecretvalue")
        );
        let setting = g.ledger().setting(plenipo_guard::SETTING).unwrap().unwrap();
        assert!(!setting.to_string().contains("ghp_supersecretvalue"));
        // Renaming keeps the value.
        save(
            &g,
            &store,
            &SecretInput {
                id: Some(info.id.clone()),
                name: "GitHub".into(),
                env_var: Some("GH_TOKEN".into()),
                programs: vec!["gh".into()],
                value: None,
            },
        )
        .unwrap();
        assert!(store.get(&info.id).unwrap().is_some());
        remove(&g, &store, &info.id).unwrap();
        assert!(store.get(&info.id).unwrap().is_none());
        assert!(g.config().unwrap().secrets.is_empty());
    }

    #[test]
    fn long_values_are_kept_in_pieces() {
        let store = MemorySecretStore {
            limit: Some(1_280),
            ..MemorySecretStore::default()
        };
        let long: String = (0..3_400)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        assert!(store.set("k", &long).is_err(), "too long for one entry");
        put(&store, "k", &long).unwrap();
        assert_eq!(read(&store, "k").unwrap().as_deref(), Some(long.as_str()));
        assert_eq!(pieces_of(&store, "k").unwrap(), 4);
        // A shorter value replaces it, and the pieces go.
        put(&store, "k", "short value").unwrap();
        assert_eq!(read(&store, "k").unwrap().as_deref(), Some("short value"));
        assert!(store.get("k.piece1").unwrap().is_none());
        put(&store, "k", &long).unwrap();
        erase(&store, "k").unwrap();
        assert!(read(&store, "k").unwrap().is_none());
        assert!(store.get("k.piece4").unwrap().is_none());
        // Secrets use it too.
        let g = guard();
        let info = save(
            &g,
            &store,
            &SecretInput {
                name: "Long token".into(),
                value: Some(long.clone()),
                ..SecretInput::default()
            },
        )
        .unwrap();
        assert_eq!(
            read(&store, &info.id).unwrap().as_deref(),
            Some(long.as_str())
        );
        remove(&g, &store, &info.id).unwrap();
        assert!(store.get(&piece_id(&info.id, 1)).unwrap().is_none());
    }

    #[test]
    fn an_unavailable_store_keeps_no_reference() {
        let g = guard();
        let store = MemorySecretStore {
            broken: Some("no keyring".into()),
            ..MemorySecretStore::default()
        };
        let e = save(
            &g,
            &store,
            &SecretInput {
                name: "Key".into(),
                value: Some("value-123".into()),
                ..SecretInput::default()
            },
        )
        .unwrap_err();
        assert!(e.to_string().contains("no keyring"));
        assert!(g.config().unwrap().secrets.is_empty());
    }
}
