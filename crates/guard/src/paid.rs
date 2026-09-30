//! Paid AI services (Phase 16 Wave 3, ADR-085 to ADR-087): which services a paid AI key may
//! reach, the only addresses each may use, and the references to the owner's saved keys (never
//! the keys, which only the Vault keeps).
//!
//! A paid task runs Plenipo's own helper (`plenipo-desktop --plenipo-paid <service>`), which
//! reaches its service's fixed addresses only, each checked by [`crate::outbound`]'s rules for
//! Plenipo's own requests (`Purpose::PaidAi`) before it connects.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// At most this many paid keys (one per paid AI tool, with room to spare).
pub const MAX_PAID_KEYS: usize = 32;

/// A paid AI service Plenipo can reach with the owner's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PaidService {
    /// OpenRouter: one key, hundreds of models from dozens of companies (ADR-086).
    OpenRouter,
}

/// How a service is talked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaidProtocol {
    /// OpenAI's chat completions, which most services offer (OpenRouter's too).
    OpenAiChat,
    /// Anthropic's own messages (ADR-087).
    Anthropic,
}

impl PaidService {
    pub const ALL: [Self; 1] = [Self::OpenRouter];

    /// The service's ID: also its paid AI tool's ID ("openrouter").
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Its name on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
        }
    }

    /// The only hosts the service's helper reaches.
    pub fn hosts(self) -> &'static [&'static str] {
        match self {
            Self::OpenRouter => &["openrouter.ai"],
        }
    }

    /// Where its requests go.
    pub fn base_url(self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    pub fn protocol(self) -> PaidProtocol {
        match self {
            Self::OpenRouter => PaidProtocol::OpenAiChat,
        }
    }

    /// A cheap read that tells whether the key works (OpenRouter's `/key` also gives the key's
    /// own limit).
    pub fn key_check_path(self) -> &'static str {
        match self {
            Self::OpenRouter => "/key",
        }
    }

    /// Where its models and their prices are listed, if it lists them.
    pub fn models_path(self) -> Option<&'static str> {
        match self {
            Self::OpenRouter => Some("/models"),
        }
    }

    /// Where a task's request goes.
    pub fn chat_path(self) -> &'static str {
        match self.protocol() {
            PaidProtocol::OpenAiChat => "/chat/completions",
            PaidProtocol::Anthropic => "/messages",
        }
    }
}

/// A saved paid key, by reference: which AI tool it is for and the name the owner gave it. The
/// key itself is only in the Vault, under `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaidKeyInfo {
    /// The Vault's name for the key (`paid-key-…`).
    pub id: String,
    /// The paid AI tool the key is for ("openrouter"); it reaches no other.
    pub runtime_id: String,
    /// The owner's name for it ("Office key").
    pub name: String,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    /// The key this one replaces, kept in the Vault until this one's check passes, then erased
    /// (a key is never left in the Vault with nothing pointing to it, even if Plenipo stops
    /// during the check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub replaces: Option<String>,
}

impl PaidKeyInfo {
    /// Every Vault name this reference holds: the key, and the one it replaces, if any.
    pub fn vault_ids(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.id).chain(self.replaces.iter())
    }
}

/// The Vault's name for a new paid key.
pub fn new_key_id() -> String {
    format!("paid-key-{}", uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_service_has_its_own_id_https_address_and_hosts() {
        for s in PaidService::ALL {
            assert_eq!(PaidService::from_id(s.id()), Some(s));
            assert!(s.base_url().starts_with("https://"), "{s:?}");
            let host = s.base_url()["https://".len()..].split('/').next().unwrap();
            assert!(s.hosts().contains(&host), "{s:?}: {host}");
            assert!(s.key_check_path().starts_with('/'));
            assert!(s.chat_path().starts_with('/'));
        }
        assert_eq!(PaidService::from_id("nope"), None);
    }

    #[test]
    fn a_key_id_names_a_paid_key() {
        let id = new_key_id();
        assert!(id.starts_with("paid-key-") && id.len() <= 64, "{id}");
        assert_ne!(id, new_key_id());
    }
}
