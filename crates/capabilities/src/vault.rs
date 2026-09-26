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
}

impl MemorySecretStore {
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

fn unavailable(label: &str, why: &str) -> BrokerError {
    BrokerError::Invalid(format!(
        "Plenipo could not use {label} to keep the secret: {why}"
    ))
}

fn check_value(value: &str) -> Result<()> {
    if value.is_empty() || value.chars().count() > MAX_SECRET_CHARS || value.contains('\0') {
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
                store
                    .set(id, v)
                    .map_err(|e| unavailable(store.label(), &e))?;
            }
            Ok(guard.save_secret(&reference)?)
        }
        None => {
            let info = guard.save_secret(&reference)?;
            if let Err(e) = store.set(&info.id, value.unwrap_or_default()) {
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
    store
        .delete(id)
        .map_err(|e| unavailable(store.label(), &e))?;
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
