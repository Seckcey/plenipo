//! Router DTOs shared with the frontend (camelCase on the wire). AI tools (runtimes) and AI
//! companies (providers) are data values; no vendor appears in a type.

use std::collections::BTreeMap;

use plenipo_runtime::agent::{AuthState, Effort, KnownModel};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Something a model can do beyond reading and writing text and code, as the owner recorded
/// it. Plenipo cannot ask the AI tools, so an unmarked model is treated as unable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ModelFeature {
    /// Takes images as input.
    Vision,
    /// Creates images.
    ImageGeneration,
    /// Operates a computer's screen, mouse, and keyboard.
    ComputerUse,
}

impl ModelFeature {
    /// "sees images", "makes images", "uses a computer".
    pub fn words(self) -> &'static str {
        match self {
            Self::Vision => "see images",
            Self::ImageGeneration => "make images",
            Self::ComputerUse => "use a computer",
        }
    }
}

/// How much a model costs to use, relative to the others (the owner's judgment).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CostClass {
    Economical,
    #[default]
    Standard,
    Premium,
}

/// Which models a role tries first when it has no ordered list of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CostPreference {
    /// The order of the owner's model list.
    #[default]
    Any,
    Economical,
    Premium,
}

/// Whether a role's workers should come from a different AI company than the work they
/// review (plan: cross-provider review).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CrossCompany {
    #[default]
    Off,
    /// Try models from other AI companies first, then the rest.
    Prefer,
    /// Only models from other AI companies.
    Require,
}

/// What happens when an AI tool reaches its usage limit (plan §6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LimitBehavior {
    /// Never move work to another AI company because of a usage limit: wait for it to reset.
    #[default]
    Wait,
    /// Use the role's next choice, even from another AI company.
    NextChoice,
}

/// One entry of the model registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    pub id: String,
    /// The AI tool (runtime) that runs it.
    pub runtime_id: String,
    /// The name the AI tool accepts for it; `None`: whatever model the AI tool uses by default.
    pub name: Option<String>,
    /// The owner's name for it (kept apart from the provider's name).
    pub label: String,
    #[serde(default)]
    pub features: Vec<ModelFeature>,
    /// How much text it takes in at once, in tokens, when the owner recorded it.
    #[serde(default)]
    pub context_tokens: Option<u32>,
    #[serde(default)]
    pub cost: CostClass,
    /// The effort level it runs at unless a role says otherwise (`None`: the AI tool's default).
    #[serde(default)]
    pub effort: Option<Effort>,
    /// One per AI tool, added by Plenipo; it can be edited but not removed.
    #[serde(default)]
    pub built_in: bool,
}

/// Add a model (`id` absent) or change one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ModelInput {
    #[ts(optional)]
    pub id: Option<String>,
    pub runtime_id: String,
    /// Empty or absent: the AI tool's default model.
    #[ts(optional)]
    pub name: Option<String>,
    pub label: String,
    pub features: Vec<ModelFeature>,
    #[ts(optional)]
    pub context_tokens: Option<u32>,
    pub cost: CostClass,
    /// Absent: the AI tool's default effort.
    #[ts(optional)]
    pub effort: Option<Effort>,
}

/// A role's model policy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct RolePolicy {
    /// Model IDs in order: the first choice, then the fallback order. Empty: any model in the
    /// registry, ordered by `cost`.
    pub models: Vec<String>,
    /// What the model must be able to do.
    pub needs: Vec<ModelFeature>,
    /// The smallest context the model must take, in tokens.
    pub min_context_tokens: Option<u32>,
    /// AI companies (provider IDs) this role never uses.
    pub never_companies: Vec<String>,
    pub cost: CostPreference,
    pub cross_company: CrossCompany,
    /// The effort level this role runs a model at, by model ID, when it differs from the
    /// model's own setting.
    pub efforts: BTreeMap<String, Effort>,
}

/// Choices that apply to every role.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct RoutingOptions {
    pub on_usage_limit: LimitBehavior,
}

/// An AI tool reached its usage limit and is not given new work until `until`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageLimit {
    /// The model it was running, when known.
    pub model: Option<String>,
    #[ts(type = "number")]
    pub since: u64,
    /// The reset time the AI tool reported, if it did.
    #[ts(type = "number | null")]
    pub resets_at: Option<u64>,
    /// When Plenipo tries it again: the reported reset, or an hour after the limit.
    #[ts(type = "number")]
    pub until: u64,
    /// What the AI tool said.
    pub detail: String,
}

/// One AI tool in the provider registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolInfo {
    pub runtime_id: String,
    pub label: String,
    /// The AI company (provider ID) and its name.
    pub company: String,
    pub company_label: String,
    /// Installed and signed in with a subscription.
    pub ready: bool,
    pub auth: AuthState,
    /// Plain words: ready, or why not.
    pub status: String,
    pub usage_limit: Option<UsageLimit>,
    /// Can take new work now (ready and not at a usage limit).
    pub available: bool,
    /// Effort levels the AI tool accepts, lowest first (empty: none).
    pub effort_levels: Vec<Effort>,
    /// Models the AI tool itself offers (offered as choices, never assumed to be in the owner's
    /// list).
    pub known_models: Vec<KnownModel>,
}

/// What happened to one model the router considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CandidateVerdict {
    Chosen,
    Skipped,
    /// A model before it in the order was chosen.
    NotNeeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CandidateNote {
    pub model_id: String,
    /// How the model reads, e.g. "Opus (Claude Code)".
    pub label: String,
    pub verdict: CandidateVerdict,
    /// Why it was skipped; empty otherwise.
    pub note: String,
}

/// The AI tool and model chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RouteChoice {
    pub model_id: String,
    pub runtime_id: String,
    pub runtime_label: String,
    pub company: String,
    /// The model name passed to the AI tool (`None`: its default).
    pub model: Option<String>,
    /// The effort level passed to the AI tool (`None`: its default).
    pub effort: Option<Effort>,
    /// e.g. "Opus (Claude Code)".
    pub label: String,
}

/// The router's decision and its reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RouteDecision {
    pub choice: Option<RouteChoice>,
    /// One or two plain sentences: why this model, or why none.
    pub reason: String,
    /// The chosen model's place in the role's list (1 = first choice); `None` without a list.
    pub rank: Option<u32>,
    /// Every model considered, in the order tried.
    pub candidates: Vec<CandidateNote>,
    /// The owner fixed the AI tool and model on the position (the policy was not consulted).
    pub fixed: bool,
}

/// A role's policy with the model its next worker would get.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RolePolicyView {
    pub role_id: String,
    pub role_name: String,
    /// Full-time roles keep their model for a whole conversation.
    pub full_time: bool,
    pub policy: RolePolicy,
    /// Where its next worker (or new agent) would go now, outside any project.
    pub next: RouteDecision,
}

/// A model an AI tool reported running (or was asked to run).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelSeen {
    pub runtime_id: String,
    pub runtime_label: String,
    pub name: String,
    pub runs: u32,
    #[ts(type = "number")]
    pub last_used: u64,
    /// Already in the registry.
    pub listed: bool,
}

/// Everything the Settings page shows about models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutingSnapshot {
    pub models: Vec<ModelInfo>,
    pub tools: Vec<ToolInfo>,
    pub roles: Vec<RolePolicyView>,
    pub seen: Vec<ModelSeen>,
    pub options: RoutingOptions,
    /// Pay-per-use API billing (always off in this version; ADR-007).
    pub api_billing: bool,
    pub notices: Vec<String>,
    #[ts(type = "number")]
    pub generated_at: u64,
}
