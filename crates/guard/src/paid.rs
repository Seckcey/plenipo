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

/// A paid AI service Plenipo can reach with the owner's key: OpenRouter (ADR-086), and each AI
/// company's own service (ADR-087). Each address was checked against the company's own
/// documentation on 2026-09-30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PaidService {
    /// OpenRouter: one key, hundreds of models from dozens of companies (ADR-086).
    OpenRouter,
    /// Anthropic's own service (Claude models).
    Anthropic,
    /// OpenAI's own service (GPT models).
    OpenAi,
    /// xAI's own service (Grok models).
    Xai,
    /// Moonshot AI's own service (Kimi models).
    Moonshot,
    /// Google's Gemini service, through its OpenAI-style chat.
    Google,
    /// DeepSeek's own service.
    DeepSeek,
    /// Z.ai's own service (GLM models), international.
    Zai,
    /// MiniMax's own service, international.
    MiniMax,
    /// Mistral's own service.
    Mistral,
    /// Alibaba Cloud Model Studio (Qwen models), international (Singapore).
    Alibaba,
}

/// How a service is talked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaidProtocol {
    /// OpenAI's chat completions, which most services offer (OpenRouter's too).
    OpenAiChat,
    /// Anthropic's own messages (ADR-087).
    Anthropic,
}

/// How a request carries the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaidAuth {
    /// `Authorization: Bearer <key>`.
    Bearer,
    /// Anthropic's `x-api-key` and `anthropic-version`.
    AnthropicKey,
    /// Google's `x-goog-api-key`.
    GoogleKey,
}

impl PaidService {
    pub const ALL: [Self; 11] = [
        Self::OpenRouter,
        Self::Anthropic,
        Self::OpenAi,
        Self::Xai,
        Self::Moonshot,
        Self::Google,
        Self::DeepSeek,
        Self::Zai,
        Self::MiniMax,
        Self::Mistral,
        Self::Alibaba,
    ];

    /// The service's ID: also its paid AI tool's ID ("openrouter", "anthropic-key").
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic-key",
            Self::OpenAi => "openai-key",
            Self::Xai => "xai-key",
            Self::Moonshot => "moonshot-key",
            Self::Google => "google-key",
            Self::DeepSeek => "deepseek-key",
            Self::Zai => "zai-key",
            Self::MiniMax => "minimax-key",
            Self::Mistral => "mistral-key",
            Self::Alibaba => "alibaba-key",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Its name on screen and in its own words ("OpenRouter refused the key").
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Xai => "xAI",
            Self::Moonshot => "Moonshot AI",
            Self::Google => "Google",
            Self::DeepSeek => "DeepSeek",
            Self::Zai => "Z.ai",
            Self::MiniMax => "MiniMax",
            Self::Mistral => "Mistral",
            Self::Alibaba => "Alibaba Cloud",
        }
    }

    /// The only hosts the service's helper reaches.
    pub fn hosts(self) -> &'static [&'static str] {
        match self {
            Self::OpenRouter => &["openrouter.ai"],
            Self::Anthropic => &["api.anthropic.com"],
            Self::OpenAi => &["api.openai.com"],
            Self::Xai => &["api.x.ai"],
            Self::Moonshot => &["api.moonshot.ai"],
            Self::Google => &["generativelanguage.googleapis.com"],
            Self::DeepSeek => &["api.deepseek.com"],
            Self::Zai => &["api.z.ai"],
            Self::MiniMax => &["api.minimax.io"],
            Self::Mistral => &["api.mistral.ai"],
            Self::Alibaba => &["dashscope-intl.aliyuncs.com"],
        }
    }

    /// Where its requests go.
    pub fn base_url(self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::Moonshot => "https://api.moonshot.ai/v1",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta",
            Self::DeepSeek => "https://api.deepseek.com",
            Self::Zai => "https://api.z.ai/api/paas/v4",
            Self::MiniMax => "https://api.minimax.io/v1",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::Alibaba => "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        }
    }

    pub fn protocol(self) -> PaidProtocol {
        match self {
            Self::Anthropic => PaidProtocol::Anthropic,
            _ => PaidProtocol::OpenAiChat,
        }
    }

    pub fn auth(self) -> PaidAuth {
        match self {
            Self::Anthropic => PaidAuth::AnthropicKey,
            Self::Google => PaidAuth::GoogleKey,
            _ => PaidAuth::Bearer,
        }
    }

    /// A cheap read that tells whether the key works: OpenRouter's `/key` (with the key's own
    /// limit); the others' list of models, which costs nothing.
    pub fn key_check_path(self) -> &'static str {
        match self {
            Self::OpenRouter => "/key",
            // Google's own list (its OpenAI-style one has none), every model on one page.
            Self::Google => "/models?pageSize=1000",
            _ => "/models",
        }
    }

    /// Where its models are listed (OpenRouter's with their prices), if it lists them.
    pub fn models_path(self) -> Option<&'static str> {
        match self {
            Self::OpenRouter => Some("/models"),
            other => Some(other.key_check_path()),
        }
    }

    /// Where a task's request goes.
    pub fn chat_path(self) -> &'static str {
        match self {
            Self::Google => "/openai/chat/completions",
            other => match other.protocol() {
                PaidProtocol::OpenAiChat => "/chat/completions",
                PaidProtocol::Anthropic => "/messages",
            },
        }
    }

    /// The request's field for the longest answer: `max_completion_tokens` where the service
    /// takes only that (its reasoning models), `max_tokens` elsewhere. Either way the thinking
    /// counts inside it, where the service says so.
    pub fn max_tokens_field(self) -> &'static str {
        match self {
            Self::OpenAi | Self::Moonshot | Self::MiniMax => "max_completion_tokens",
            _ => "max_tokens",
        }
    }

    /// Whether to ask for the counts at the end of a streamed answer
    /// (`stream_options.include_usage`); a service that refuses fields it does not know (Mistral)
    /// sends them by itself.
    pub fn asks_for_stream_usage(self) -> bool {
        !matches!(self, Self::Mistral | Self::Anthropic)
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
            assert!(!s.label().is_empty());
        }
        // One ID each, and none an AI tool's that signs in with a subscription.
        let ids: std::collections::HashSet<_> = PaidService::ALL.iter().map(|s| s.id()).collect();
        assert_eq!(ids.len(), PaidService::ALL.len());
        for taken in [
            "claude-code",
            "codex",
            "grok",
            "kimi",
            "ollama",
            "antigravity",
            "copilot",
        ] {
            assert!(!ids.contains(taken), "{taken}");
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
