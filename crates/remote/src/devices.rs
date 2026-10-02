//! The phones the PC knows, and where they are kept (ADR-141 §8).
//!
//! The PC's keys and each phone's record are kept in the Vault (the first organization's, under
//! its own name, like the license key, ADR-110): its name, its long-term public key, its passkey,
//! when it was added and last seen, whether it is paused, and, after part 14C, where to send its
//! notices. The switch and the approvals kept on the PC are not secret: they are kept in
//! `remote.json` in Plenipo's data folder. Nothing here is ever written to a log.

use plenipo_guard::remote::KeptOnPc;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::keys::PcKeys;
use crate::protocol::Subscription;
use crate::webauthn::Passkey;
use crate::{RemoteError, Result};

/// The PC's keys, in the Vault.
pub const KEYS_ID: &str = "plenipo-remote-pc";
/// The list of the phones' IDs, in the Vault.
pub const LIST_ID: &str = "plenipo-remote-devices";
/// The Vault ID of a phone's record starts with this.
pub const DEVICE_PREFIX: &str = "plenipo-remote-device-";

/// Where secret-ish values are kept: the Vault (the app's), or memory (the tests).
pub trait KeyStore: Send + Sync + 'static {
    fn get(&self, id: &str) -> std::result::Result<Option<String>, String>;
    fn set(&self, id: &str, value: &str) -> std::result::Result<(), String>;
    fn delete(&self, id: &str) -> std::result::Result<(), String>;
}

/// Where the switch and the approvals kept on the PC are kept: `remote.json` (the app's), or
/// memory (the tests).
pub trait ConfigFile: Send + Sync + 'static {
    fn read(&self) -> Option<String>;
    fn write(&self, text: &str) -> std::result::Result<(), String>;
}

/// A store in memory (the tests).
#[derive(Default)]
pub struct MemoryStore {
    values: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
    /// When set, every write fails (a Vault that cannot be used).
    pub broken: std::sync::atomic::AtomicBool,
}

impl MemoryStore {
    pub fn ids(&self) -> Vec<String> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }
}

impl KeyStore for MemoryStore {
    fn get(&self, id: &str) -> std::result::Result<Option<String>, String> {
        Ok(self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned())
    }

    fn set(&self, id: &str, value: &str) -> std::result::Result<(), String> {
        if self.broken.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("the store is not available".into());
        }
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, id: &str) -> std::result::Result<(), String> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
        Ok(())
    }
}

/// A config file in memory (the tests).
#[derive(Default)]
pub struct MemoryConfig(pub std::sync::Mutex<Option<String>>);

impl ConfigFile for MemoryConfig {
    fn read(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn write(&self, text: &str) -> std::result::Result<(), String> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text.to_owned());
        Ok(())
    }
}

/// The switch and the approvals kept on the PC (`remote.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Config {
    /// Settings → Switches → **Use Plenipo from another device** (off to begin with).
    pub switched_on: bool,
    /// Settings → Devices → **Keep these approvals on my PC only** (none to begin with).
    pub kept: KeptOnPc,
}

impl Config {
    pub fn read(file: &dyn ConfigFile) -> Self {
        file.read()
            .and_then(|t| serde_json::from_str::<Self>(&t).ok())
            .map(|c| Self {
                kept: c.kept.tidy(),
                ..c
            })
            .unwrap_or_default()
    }

    pub fn write(&self, file: &dyn ConfigFile) -> Result<()> {
        let text = serde_json::to_string_pretty(self).expect("the config is JSON");
        file.write(&text).map_err(|e| {
            RemoteError::Store(format!("Plenipo couldn't keep the phone settings: {e}"))
        })
    }
}

/// A phone the PC knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Device {
    /// Its ID on the PC (16 random bytes, base64url).
    pub id: String,
    /// Its name ("Frank's iPhone").
    pub name: String,
    /// What it is ("Safari on iPhone").
    pub browser: String,
    /// Its ID at the relay (16 random bytes, base64url): what its pass names.
    pub phone: String,
    /// Its long-term Noise public key (base64url).
    pub key: String,
    pub passkey: Passkey,
    /// Unix milliseconds.
    pub added_at: u64,
    #[serde(default)]
    pub last_seen_at: Option<u64>,
    /// Paused after too many failed checks (ADR-142 §6), until the owner un-pauses it.
    #[serde(default)]
    pub paused: bool,
    /// Where its notices go (part 14C).
    #[serde(default)]
    pub notices: Option<Subscription>,
}

impl Device {
    /// Its long-term Noise public key.
    pub fn noise_key(&self) -> Option<[u8; 32]> {
        crate::b64::decode_exact(&self.key)
    }
}

/// A phone, as Settings → Devices shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub browser: String,
    #[ts(type = "number")]
    pub added_at: u64,
    #[ts(type = "number | null")]
    pub last_seen_at: Option<u64>,
    pub paused: bool,
    /// Signed in now.
    pub signed_in: bool,
    /// It gets notices (part 14C).
    pub notices: bool,
}

/// A name, made safe to show: one line, no control characters, at most 40 characters.
pub fn clean_name(name: &str, fallback: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let joined: String = words
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(crate::protocol::MAX_NAME)
        .collect();
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The Vault's view of the PC's keys and phones.
pub struct Kept<'a> {
    pub store: &'a dyn KeyStore,
}

impl Kept<'_> {
    fn err(e: String) -> RemoteError {
        RemoteError::Store(format!(
            "Plenipo couldn't use the Vault for your phones: {e}"
        ))
    }

    /// The PC's keys, made and kept the first time.
    pub fn keys(&self) -> Result<PcKeys> {
        if let Some(text) = self.store.get(KEYS_ID).map_err(Self::err)? {
            if let Some(keys) = PcKeys::read(&text) {
                return Ok(keys);
            }
            return Err(RemoteError::Store(
                "The phone keys in the Vault could not be read. Remove every phone in Settings \
                 → Devices and add them again."
                    .into(),
            ));
        }
        let keys = PcKeys::new();
        self.store.set(KEYS_ID, &keys.write()).map_err(Self::err)?;
        Ok(keys)
    }

    /// Every phone kept (one that cannot be read is skipped, and said in the log without its
    /// contents).
    pub fn devices(&self) -> Result<Vec<Device>> {
        let Some(list) = self.store.get(LIST_ID).map_err(Self::err)? else {
            return Ok(Vec::new());
        };
        let ids: Vec<String> = serde_json::from_str(&list).unwrap_or_default();
        let mut devices = Vec::new();
        for id in ids {
            match self.store.get(&format!("{DEVICE_PREFIX}{id}")) {
                Ok(Some(text)) => match serde_json::from_str::<Device>(&text) {
                    Ok(d) if d.id == id => devices.push(d),
                    _ => log::warn!("a phone's record in the Vault could not be read"),
                },
                Ok(None) => {}
                Err(e) => return Err(Self::err(e)),
            }
        }
        Ok(devices)
    }

    /// Keep one phone's record (and the list).
    pub fn put(&self, device: &Device, all_ids: &[String]) -> Result<()> {
        let text = serde_json::to_string(device).expect("a device is JSON");
        self.store
            .set(&format!("{DEVICE_PREFIX}{}", device.id), &text)
            .map_err(Self::err)?;
        self.store
            .set(
                LIST_ID,
                &serde_json::to_string(all_ids).expect("ids are JSON"),
            )
            .map_err(Self::err)
    }

    /// Forget one phone.
    pub fn remove(&self, id: &str, all_ids: &[String]) -> Result<()> {
        self.store
            .set(
                LIST_ID,
                &serde_json::to_string(all_ids).expect("ids are JSON"),
            )
            .map_err(Self::err)?;
        self.store
            .delete(&format!("{DEVICE_PREFIX}{id}"))
            .map_err(Self::err)
    }

    /// Every Vault ID phone access uses (for uninstalling with "delete my data").
    pub fn all_ids(&self) -> Vec<String> {
        let mut ids = vec![KEYS_ID.to_owned(), LIST_ID.to_owned()];
        if let Ok(Some(list)) = self.store.get(LIST_ID) {
            let devices: Vec<String> = serde_json::from_str(&list).unwrap_or_default();
            ids.extend(devices.into_iter().map(|d| format!("{DEVICE_PREFIX}{d}")));
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> Device {
        Device {
            id: id.into(),
            name: "Frank's iPhone".into(),
            browser: "Safari on iPhone".into(),
            phone: crate::b64::encode(&[2u8; 16]),
            key: crate::b64::encode(&[3u8; 32]),
            passkey: Passkey {
                id: crate::b64::encode(&[4u8; 64]),
                algorithm: crate::webauthn::ES256,
                public: crate::b64::encode(&[5u8; 65]),
                counter: 7,
            },
            added_at: 1,
            last_seen_at: None,
            paused: false,
            notices: Some(Subscription {
                endpoint: format!("https://web.push.apple.com/{}", "x".repeat(200)),
                p256dh: crate::b64::encode(&[4u8; 65]),
                auth: crate::b64::encode(&[1u8; 16]),
            }),
        }
    }

    #[test]
    fn phones_are_kept_in_the_vault_and_read_back() {
        let store = MemoryStore::default();
        let kept = Kept { store: &store };
        let keys = kept.keys().unwrap();
        // Made once.
        assert_eq!(kept.keys().unwrap().fingerprint(), keys.fingerprint());
        let a = device("a");
        let b = device("b");
        kept.put(&a, &["a".into()]).unwrap();
        kept.put(&b, &["a".into(), "b".into()]).unwrap();
        assert_eq!(kept.devices().unwrap(), [a.clone(), b]);
        kept.remove("b", &["a".into()]).unwrap();
        assert_eq!(kept.devices().unwrap(), [a]);
        assert_eq!(
            kept.all_ids(),
            [KEYS_ID, LIST_ID, "plenipo-remote-device-a"]
        );
    }

    #[test]
    fn a_record_fits_one_vault_value() {
        // A record with the longest name and a notice address stays well under the Vault's
        // 10,000 characters.
        let mut d = device("a");
        d.name = "x".repeat(40);
        let text = serde_json::to_string(&d).unwrap();
        assert!(text.len() < 2_000, "{}", text.len());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Frank's\n iPhone ", "Phone"), "Frank's iPhone");
        assert_eq!(clean_name("\u{7}\u{1b}", "Phone"), "Phone");
        assert_eq!(clean_name(&"y".repeat(100), "Phone").len(), 40);
    }

    #[test]
    fn the_config_reads_back_and_starts_off() {
        let file = MemoryConfig::default();
        assert_eq!(Config::read(&file), Config::default());
        assert!(!Config::read(&file).switched_on);
        let c = Config {
            switched_on: true,
            kept: KeptOnPc {
                every: false,
                production_servers: true,
                kinds: vec![],
            },
        };
        c.write(&file).unwrap();
        assert_eq!(Config::read(&file), c);
        *file.0.lock().unwrap() = Some("not json".into());
        assert_eq!(Config::read(&file), Config::default());
    }
}
