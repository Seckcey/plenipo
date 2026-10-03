//! Provider-neutral agent runtime DTOs shared with the frontend (camelCase on the wire).
//!
//! Nothing here names a vendor: runtimes and providers are identified by data values
//! (`runtimeId`, `provider`) that only the adapters define.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dto::{PromptSize, TokenUsage};

/// Whether a runtime's CLI was found and runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallState {
    /// Detection has not finished yet.
    Checking,
    Installed,
    NotInstalled,
    /// Found, but in a form Plenipo will not run (e.g. a Windows npm `.cmd` shim).
    Unsupported,
    /// Found, but it failed to report its version.
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Installation {
    pub state: InstallState,
    pub executable: Option<String>,
    pub version: Option<String>,
    /// Explanation when not installed/unsupported/broken.
    pub detail: Option<String>,
}

/// Sign-in state as reported by the runtime's own status command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuthState {
    /// Not checked yet.
    Checking,
    /// Signed in with a subscription account (the only billing Plenipo uses by default).
    Subscription,
    /// Signed in, but the billing method was not recognized.
    Unverified,
    /// Signed in with an API key: usage would be billed to the API. Refused.
    ApiKey,
    /// Routed through a third-party cloud provider. Refused.
    ThirdPartyCloud,
    /// A paid AI tool with the owner's saved key, while paid keys are switched on (Phase 16
    /// Wave 3, ADR-085): pay per use, within the spending caps.
    PaidKey,
    SignedOut,
    /// The status command failed or is not supported by this version.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthStatus {
    pub state: AuthState,
    /// Sign-in method as a short label (never an account identifier).
    pub method: Option<String>,
    pub detail: Option<String>,
}

/// What a runtime supports through its adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeCapabilities {
    /// Text arrives incrementally while the agent writes it.
    pub streaming_text: bool,
    pub resume: bool,
    pub cancel: bool,
    /// The runtime reports a structured final result (not just text on stdout).
    pub structured_results: bool,
    /// Checks its reported credential source during every turn.
    pub billing_checked_per_turn: bool,
    /// Human-readable description of what the agent may do in this phase.
    pub tool_posture: String,
    /// Effort levels the runtime accepts, lowest first; empty when it has no effort setting.
    pub effort_levels: Vec<Effort>,
    /// Models the CLI itself offers (its own aliases or model picker), most capable first;
    /// offered as choices, never assumed to be in the owner's list.
    pub known_models: Vec<KnownModel>,
    /// Who made the model the AI tool runs when none is named (ADR-081 §2). Absent: the AI
    /// tool's own company for one that runs only its own company's models, and not known for
    /// one that runs other companies' models too (Antigravity, ADR-082).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default_maker: Option<Maker>,
    /// It runs other companies' models too (Ollama), so a model it does not list was made by
    /// someone Plenipo does not know. Otherwise every model it runs is its own company's.
    #[serde(default)]
    pub runs_other_makers: bool,
}

impl RuntimeCapabilities {
    /// The effort levels `model` accepts: a known model's own, otherwise the runtime's.
    pub fn effort_levels_for(&self, model: Option<&str>) -> &[Effort] {
        model
            .and_then(|m| self.known_models.iter().find(|k| k.name == m))
            .map_or(&self.effort_levels, |k| &k.effort_levels)
    }
}

/// An AI company that made a model (ADR-081 §1): an ID and its name. A company that also makes
/// an AI tool has the same ID as that tool's company (`openai`), so it is one company
/// everywhere.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Maker {
    pub id: String,
    pub label: String,
}

impl Maker {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Work a worker would review, for cross-company review (ADR-081 §3): the AI tool that did it
/// and the model it ran (`None`: the AI tool's default).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkDoneBy {
    pub runtime_id: String,
    pub model: Option<String>,
    /// Plenipo could not read which model did it (its records could not be read), so who made
    /// it is not known on an AI tool that runs other companies' models.
    pub model_unread: bool,
}

impl WorkDoneBy {
    pub fn new(runtime_id: &str, model: Option<&str>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            model: model.map(str::to_owned),
            model_unread: false,
        }
    }

    /// Work on `runtime_id` whose model Plenipo could not read.
    pub fn unread(runtime_id: &str) -> Self {
        Self {
            model_unread: true,
            ..Self::new(runtime_id, None)
        }
    }
}

/// The AI companies whose models the built-in AI tools list, as `(ID, name)`.
pub mod makers {
    pub const ANTHROPIC: (&str, &str) = ("anthropic", "Anthropic");
    pub const OPENAI: (&str, &str) = ("openai", "OpenAI");
    pub const XAI: (&str, &str) = ("xai", "xAI");
    pub const MOONSHOT: (&str, &str) = ("moonshot", "Moonshot AI");
    pub const GOOGLE: (&str, &str) = ("google", "Google");
    pub const DEEPSEEK: (&str, &str) = ("deepseek", "DeepSeek");
    pub const ZAI: (&str, &str) = ("zai", "Z.ai");
    pub const MINIMAX: (&str, &str) = ("minimax", "MiniMax");
    pub const NVIDIA: (&str, &str) = ("nvidia", "NVIDIA");
    pub const GITHUB: (&str, &str) = ("github", "GitHub");
    /// Qwen's maker (Phase 16 Wave 3, through OpenRouter).
    pub const ALIBABA: (&str, &str) = ("alibaba", "Alibaba (Qwen)");
    pub const MISTRAL: (&str, &str) = ("mistral", "Mistral");
    pub const META: (&str, &str) = ("meta", "Meta");
}

/// A model a CLI itself offers, as of the CLI version its adapter was checked against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnownModel {
    /// The name the CLI's model option takes, e.g. `sonnet` or `gpt-6-sol`.
    pub name: String,
    /// How the CLI names it, e.g. "Sonnet" or "GPT-6-Sol".
    pub label: String,
    /// Effort levels it accepts (empty: it has no effort setting).
    pub effort_levels: Vec<Effort>,
    /// Who made it (ADR-081 §1): always set for the models an adapter lists; absent for a model
    /// an AI tool reported that Plenipo does not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub maker: Option<Maker>,
    /// For a name that follows the newest model (Claude Code's `opus`), the exact model it
    /// points to now, as of the version the adapter was checked against (ADR-081 §8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub points_to: Option<String>,
    /// Its price per million tokens, for a model a paid AI tool reported with one (Phase 16
    /// Wave 3, ADR-085). None: not priced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub price: Option<crate::pricing::Price>,
    /// The same model on other AI tools (ADR-036 §4): one short name for it everywhere
    /// (`kimi-k3` for Kimi K3 on Kimi Code, on Ollama, and on OpenRouter), so the owner sees one
    /// model with more than one way to reach it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub same: Option<String>,
}

impl KnownModel {
    pub fn new(name: &str, label: &str, effort_levels: &[Effort]) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            effort_levels: effort_levels.to_vec(),
            maker: None,
            points_to: None,
            price: None,
            same: None,
        }
    }

    /// The same model as `same` on other AI tools (ADR-036 §4).
    pub fn same(mut self, same: &str) -> Self {
        self.same = Some(same.into());
        self
    }

    /// Made by `maker`, an `(ID, name)` pair from [`makers`].
    pub fn by(mut self, maker: (&str, &str)) -> Self {
        self.maker = Some(Maker::new(maker.0, maker.1));
        self
    }

    /// A name that points to `exact` now (ADR-081 §8).
    pub fn now(mut self, exact: &str) -> Self {
        self.points_to = Some(exact.into());
        self
    }
}

/// How much reasoning a model spends on a turn. Each runtime accepts some of these levels
/// ([`RuntimeCapabilities::effort_levels`]); `None` elsewhere means the runtime's own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Effort {
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
    Ultra,
}

impl Effort {
    /// The value the CLIs accept (`--effort high`, `model_reasoning_effort=high`).
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
            Effort::Ultra => "ultra",
        }
    }

    /// Plain words for the screen.
    pub fn label(self) -> &'static str {
        match self {
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "extra high",
            Effort::Max => "max",
            Effort::Ultra => "ultra",
        }
    }

    pub fn parse(value: &str) -> Option<Effort> {
        serde_json::from_value(serde_json::Value::String(value.into())).ok()
    }
}

/// Provider diagnostics for one runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentRuntimeInfo {
    /// Stable adapter ID, e.g. `claude-code`.
    pub id: String,
    pub label: String,
    pub provider: String,
    pub provider_label: String,
    pub installation: Installation,
    pub auth: AuthStatus,
    pub capabilities: RuntimeCapabilities,
    pub install_hint: String,
    pub login_hint: String,
    /// Installed and signed in with an allowed method: tasks can start.
    pub ready: bool,
    #[ts(type = "number | null")]
    pub checked_at: Option<u64>,
    /// The version of the AI tool this Plenipo was checked against (ADR-014 §6), shown beside
    /// the installed one (ADR-059 §1).
    pub checked_version: String,
    /// The AI tool's own sign-in and sign-out commands (ADR-058).
    pub account: AccountCommands,
    /// The models the AI tool last reported itself, when it has a list (ADR-060 §5).
    pub reported_models: Option<ReportedModels>,
    /// Why new tasks on it wait for now: its sign-in tab is open, or it is being updated.
    pub held: Option<HoldFor>,
    /// It can use Plenipo's tools, so an agent on it can save files and run programs. `false`:
    /// it only answers in words, whatever the agent's permissions (ADR-200, ADR-131).
    #[serde(default)]
    pub uses_tools: bool,
}

// ---- The AI tools page (Phase 19, ADR-058 to ADR-060) ------------------------------------

/// Why an AI tool is held — no new task starts on it for now — which decides how long a task
/// that would start waits for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HoldFor {
    /// The owner's sign-in or sign-out tab (ADR-058 §5): a task waits a while at most (ten
    /// minutes), so a tab left open does not stop the work for good.
    SignIn,
    /// An update and the checks after it (ADR-059 §4): a task waits until they are done, and
    /// starts on the new version, or on the old one if the update failed.
    Update,
    /// The owner pressed Stop all work (Phase 25, item 3.4; ADR-199): every AI tool is held, and
    /// a task waits until the owner presses Allow again.
    StopAll,
}

/// Signing in to, or out of, an AI tool, in a terminal tab that runs the tool's own command
/// (ADR-058). Reconnect is signing in again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AccountAction {
    SignIn,
    SignOut,
}

impl AccountAction {
    /// The word on the wire and in the record: `signIn`, `signOut`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignIn => "signIn",
            Self::SignOut => "signOut",
        }
    }

    /// "Sign in" or "Sign out", as a tab's title starts.
    pub fn title(self) -> &'static str {
        match self {
            Self::SignIn => "Sign in",
            Self::SignOut => "Sign out",
        }
    }

    /// "sign-in" or "sign-out", for sentences ("Codex's sign-in").
    pub fn noun(self) -> &'static str {
        match self {
            Self::SignIn => "sign-in",
            Self::SignOut => "sign-out",
        }
    }
}

/// An AI tool's own sign-in and sign-out commands, as the owner would type them (`codex
/// login`), or `None` where the tool has none (Kimi has no sign-out command).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountCommands {
    pub sign_in: Option<String>,
    pub sign_out: Option<String>,
}

/// The models an AI tool reported itself (ADR-060 §5), and when it was asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportedModels {
    pub models: Vec<KnownModel>,
    /// The list is every model the tool offers. False where it lists only some (Ollama lists
    /// the models downloaded to this PC), so a checked model missing from it is not "no longer
    /// offered".
    pub complete: bool,
    #[ts(type = "number")]
    pub checked_at: u64,
}

/// One usage window an AI tool reported (ADR-060 §3): for example five hours, or a week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanWindow {
    /// How long the window is, in minutes, when the tool says (300: five hours).
    #[ts(type = "number | null")]
    pub minutes: Option<u64>,
    /// How much of the window is used, 0 to 100, when the tool says. Plenipo never works it
    /// out itself.
    pub used_percent: Option<u8>,
    /// When the window starts again (milliseconds since 1970), when the tool says.
    #[ts(type = "number | null")]
    pub resets_at: Option<u64>,
    /// The models it counts, when it counts only some: Claude Code's weekly limits for Opus
    /// and for Sonnet ("Opus"). Phase 25, item 4.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub models: Option<String>,
}

/// A paid key's own spending limit, where the service reports it (OpenRouter's key check; Phase
/// 25, item 4.3), in US cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeyLimit {
    /// The most the key may spend; `None`: the key has no limit.
    #[ts(type = "number | null")]
    pub limit_cents: Option<u64>,
    /// What the key has spent.
    #[ts(type = "number")]
    pub used_cents: u64,
    /// The service's free tier.
    pub free_tier: bool,
}

/// How much of the owner's plan an AI tool reported used, through an official command or
/// protocol only (ADR-060 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanReport {
    pub windows: Vec<PlanWindow>,
    /// The tool said its limit is reached.
    pub limited: bool,
    /// The tool warned that the limit is near.
    pub warning: bool,
    /// The plan's name, when the tool says ("plus"). Never an account name or email.
    pub plan: Option<String>,
    #[ts(type = "number")]
    pub reported_at: u64,
    /// A paid key's own spending limit, where the service reports it (OpenRouter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub key_limit: Option<KeyLimit>,
}

/// The longest a reported reset is believed: a month (no plan window is longer).
const MAX_RESET_MS: u64 = 30 * 24 * 60 * 60 * 1000;

impl PlanReport {
    /// This report, with the windows `earlier` reported that this one does not cover and that
    /// have not started again by `now`. Claude Code reports one window at a time (its five hours,
    /// then its week), so each report adds to the last (Phase 25, item 4.3).
    #[must_use]
    pub fn merged_with(mut self, earlier: Option<&PlanReport>, now: u64) -> Self {
        let Some(earlier) = earlier else {
            return self;
        };
        let kept: Vec<PlanWindow> = earlier
            .windows
            .iter()
            .filter(|w| w.resets_at.is_some_and(|r| r > now))
            .filter(|w| {
                !self
                    .windows
                    .iter()
                    .any(|n| n.minutes == w.minutes && n.models == w.models)
            })
            .cloned()
            .collect();
        self.windows.extend(kept);
        // The shortest window first, as each tool lists them.
        self.windows
            .sort_by_key(|w| (w.minutes.unwrap_or(u64::MAX), w.models.is_some()));
        if self.plan.is_none() {
            self.plan.clone_from(&earlier.plan);
        }
        self
    }

    /// When the window at its limit starts again, for a usage limit reached at `at`: the latest
    /// reset of the windows all used up, or of every window when the tool said only that it is
    /// limited. `None` when the report gives none that is after `at` and within a month (Phase
    /// 25, item 4.3: the hold uses the reset time the tool reported).
    pub fn reset_after(&self, at: u64) -> Option<u64> {
        let full: Vec<&PlanWindow> = self
            .windows
            .iter()
            .filter(|w| w.used_percent.is_some_and(|u| u >= 100))
            .collect();
        let windows: Vec<&PlanWindow> = if !full.is_empty() {
            full
        } else if self.limited {
            self.windows.iter().collect()
        } else {
            return None;
        };
        windows
            .iter()
            .filter_map(|w| w.resets_at)
            .filter(|r| *r > at && *r <= at + MAX_RESET_MS)
            .max()
    }
}

/// The latest plan each AI tool reported, for every organization on this PC (Phase 25, item
/// 4.3): an AI tool's plan is the owner's account, the same in each. The Router reads it for the
/// reset time of a limit whose message gave none.
#[derive(Debug, Default)]
pub struct PlanBook(std::sync::Mutex<std::collections::HashMap<String, PlanReport>>);

impl PlanBook {
    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, PlanReport>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Keep `report` for `runtime_id`, added to what it reported before; returns what is kept.
    pub fn keep(&self, runtime_id: &str, report: PlanReport) -> PlanReport {
        let mut book = self.lock();
        let merged = report.merged_with(book.get(runtime_id), crate::now_ms());
        book.insert(runtime_id.to_owned(), merged.clone());
        merged
    }

    pub fn latest(&self, runtime_id: &str) -> Option<PlanReport> {
        self.lock().get(runtime_id).cloned()
    }
}

/// An AI tool reported how much of the plan is used (ADR-060 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanUpdate {
    pub runtime_id: String,
    pub report: PlanReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeLevel {
    Info,
    Warning,
}

/// One normalized piece of agent activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AgentEvent {
    /// The provider confirmed its session and (when reported) the model.
    SessionStarted {
        provider_session_id: Option<String>,
        model: Option<String>,
    },
    /// Incremental text (live view only; not stored).
    TextDelta {
        text: String,
    },
    /// A complete assistant message.
    Message {
        text: String,
    },
    /// Reasoning summary, when the provider shares one.
    Reasoning {
        text: String,
    },
    ToolUse {
        tool: String,
        summary: String,
        /// The AI tool's own ID for the call, when it gives one, so its result can be matched
        /// to it (ADR-200).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        id: Option<String>,
    },
    ToolResult {
        tool: Option<String>,
        is_error: bool,
        summary: String,
        /// The ID of the call this answers, when the AI tool says (ADR-200).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        id: Option<String>,
    },
    Notice {
        level: NoticeLevel,
        text: String,
    },
    Usage {
        usage: TokenUsage,
    },
    /// The AI tool shortened its memory of the conversation (it compacted it, or left earlier
    /// messages out), so the next task gets the full instructions again (ADR-044 §2.5).
    /// `detail` says so in plain words.
    MemoryShortened {
        detail: String,
    },
    /// The worker's plan, all of it each time (Phase 25, item 3.1): Grok's and Kimi's plans,
    /// Codex's to-do list, Claude Code's to-dos. The screen shows "step 3 of 7" from it. Live
    /// view only.
    Plan {
        steps: Vec<PlanStep>,
    },
    /// What the AI tool is doing or waiting for between words: it asked again because its AI
    /// company was busy, it sent the request and waits for the first words, or it is thinking.
    /// Shown live in a conversation until the next words, never stored (ADR-200).
    Status {
        phase: StatusPhase,
        /// In plain words, for the owner.
        text: String,
    },
}

/// One step of a worker's plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanStep {
    pub text: String,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanStatus {
    Pending,
    InProgress,
    Done,
}

impl PlanStatus {
    /// From the words the AI tools use: "completed", "in_progress", "pending", and the like.
    pub fn from_words(status: &str) -> Self {
        match status {
            "completed" | "done" | "complete" => Self::Done,
            "in_progress" | "inProgress" | "active" | "running" => Self::InProgress,
            _ => Self::Pending,
        }
    }
}

/// A plan's steps from a list of objects: `text_keys` name where each step's words are, and
/// its status is read from `status`, or a `completed: true` flag. At most 50 steps.
pub fn plan_steps(items: &serde_json::Value, text_keys: &[&str]) -> Vec<PlanStep> {
    items
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = text_keys
                .iter()
                .find_map(|k| item.get(*k).and_then(serde_json::Value::as_str))?
                .trim();
            if text.is_empty() {
                return None;
            }
            let status = match item.get("status").and_then(serde_json::Value::as_str) {
                Some(s) => PlanStatus::from_words(s),
                None if item.get("completed").and_then(serde_json::Value::as_bool)
                    == Some(true) =>
                {
                    PlanStatus::Done
                }
                None => PlanStatus::Pending,
            };
            Some(PlanStep {
                text: crate::agent::adapter::first_line(text, 200),
                status,
            })
        })
        .take(50)
        .collect()
}

/// What a [`AgentEvent::Status`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StatusPhase {
    /// Waiting for the AI company: the request was sent, or it is being asked again.
    Waiting,
    /// The model is thinking, before it writes.
    Thinking,
}

impl AgentEvent {
    /// Ledger event type for activity worth keeping, `None` for live-only activity.
    pub fn ledger_type(&self) -> Option<&'static str> {
        match self {
            Self::SessionStarted { .. } => Some("agent.session_bound"),
            Self::Message { .. } => Some("agent.message"),
            Self::ToolUse { .. } => Some("agent.tool_use"),
            Self::ToolResult { .. } => Some("agent.tool_result"),
            Self::Notice { .. } => Some("agent.notice"),
            Self::MemoryShortened { .. } => Some("agent.memory_shortened"),
            Self::TextDelta { .. }
            | Self::Reasoning { .. }
            | Self::Usage { .. }
            | Self::Plan { .. }
            | Self::Status { .. } => None,
        }
    }
}

/// How a turn ended, normalized across providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TurnOutcome {
    Completed,
    /// The provider reported an error not covered below.
    Failed,
    Cancelled,
    TimedOut,
    /// A subscription usage or rate limit was reached. Plenipo never switches provider.
    UsageLimited,
    /// Not signed in, or the sign-in expired.
    AuthRequired,
    /// The runtime would have billed an API key or third-party cloud.
    BillingNotAllowed,
    /// The CLI is missing, could not start, or the provider was unreachable.
    ProviderUnavailable,
    /// The output could not be understood.
    MalformedOutput,
    /// The process ended without reporting a result.
    Crashed,
    /// Plenipo stopped while the turn was running.
    Interrupted,
}

impl TurnOutcome {
    pub fn is_success(self) -> bool {
        self == Self::Completed
    }
}

/// Normalized result of one turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnResult {
    pub outcome: TurnOutcome,
    /// One human-readable line.
    pub summary: String,
    /// The agent's final answer (capped).
    pub text: Option<String>,
    /// Provider or process error detail (capped).
    pub error: Option<String>,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    /// Output lines that were not understood (malformed or unknown event types).
    pub ignored_lines: u32,
    /// The size of what Plenipo sent with the step (ADR-044); none for a result recorded
    /// without a step of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<PromptSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SessionState {
    Open,
    Closed,
}

/// A conversation with one runtime, which can span several turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSession {
    /// Plenipo's session ID (UUID v4).
    pub id: String,
    pub runtime_id: String,
    pub provider: String,
    /// The provider's own session/thread ID.
    pub provider_session_id: Option<String>,
    /// The provider has reported `provider_session_id`, so it can be resumed.
    pub provider_session_confirmed: bool,
    pub model: Option<String>,
    /// Effort level every turn runs at (`None`: the runtime's default).
    pub effort: Option<Effort>,
    pub title: String,
    pub state: SessionState,
    pub working_dir: String,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    pub turn_count: u32,
    /// Task ID of the turn currently running, if any.
    pub active_task_id: Option<String>,
    /// Task ID of a turn waiting to be continued (for example for handoff replies), if any.
    /// The session takes no other turn meanwhile.
    pub waiting_task_id: Option<String>,
    /// Settings stored with the session by the component that started it (for example
    /// Plenipo Liaison). Opaque to the runtime.
    #[ts(type = "Record<string, unknown>")]
    pub metadata: serde_json::Value,
}

/// One turn: an objective given to the session, recorded as a Ledger task. A turn normally
/// runs one step; a turn that waited (for example for handoff replies, ADR-008) continues with
/// further steps in the same provider session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentTurn {
    pub task_id: String,
    pub session_id: String,
    pub number: u32,
    pub objective: String,
    /// The latest step's execution.
    pub execution_id: Option<String>,
    /// A step is running.
    pub running: bool,
    /// Waiting to be continued; no step is running.
    pub waiting: bool,
    /// The final result, once the turn has finished.
    pub result: Option<TurnResult>,
    /// Steps in order: finished ones with their results, then the running one.
    pub steps: Vec<TurnStep>,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub ended_at: Option<u64>,
}

/// One run of a turn's runtime process: the objective, or a continuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnStep {
    /// 1 for the objective, 2+ for continuations.
    pub number: u32,
    pub execution_id: Option<String>,
    pub running: bool,
    pub result: Option<TurnResult>,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
    #[ts(type = "number | null")]
    pub ended_at: Option<u64>,
}

/// Live activity for one turn. `seq` increases per turn: step `n` numbers its activity from
/// `(n - 1) * STEP_SEQ + 1` ([`crate::agent::STEP_SEQ`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentActivity {
    pub session_id: String,
    pub task_id: String,
    #[ts(type = "number")]
    pub seq: u64,
    #[ts(type = "number")]
    pub ts: u64,
    pub event: AgentEvent,
}

/// Snapshot of the agent runtimes and sessions (rebuilds the UI after a reload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentOverview {
    pub runtimes: Vec<AgentRuntimeInfo>,
    /// Newest first.
    pub sessions: Vec<AgentSession>,
    pub notices: Vec<String>,
}

/// One session with its turns (oldest first) and buffered live activity of recent turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSessionDetail {
    pub session: AgentSession,
    pub turns: Vec<AgentTurn>,
    pub activity: Vec<AgentActivity>,
}

/// Update streamed from the agent runtime to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AgentUpdate {
    Activity(AgentActivity),
    Turn(AgentTurn),
    Session(AgentSession),
    Runtimes(RuntimesUpdate),
    /// An AI tool reported how much of the plan is used, during a task (ADR-060 §3).
    Plan(PlanUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimesUpdate {
    pub runtimes: Vec<AgentRuntimeInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_event_wire_format() {
        let event = AgentEvent::SessionStarted {
            provider_session_id: Some("p1".into()),
            model: None,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "sessionStarted", "providerSessionId": "p1", "model": null })
        );
        let event = AgentEvent::ToolResult {
            tool: None,
            is_error: true,
            summary: "x".into(),
            id: None,
        };
        // Without an ID the field is left out, so events stored before it still match.
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "toolResult", "tool": null, "isError": true, "summary": "x" })
        );
        let event = AgentEvent::ToolUse {
            tool: "write_file".into(),
            summary: "a.txt".into(),
            id: Some("toolu_1".into()),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "toolUse", "tool": "write_file", "summary": "a.txt", "id": "toolu_1" })
        );
        let old: AgentEvent = serde_json::from_value(
            json!({ "type": "toolUse", "tool": "write_file", "summary": "a.txt" }),
        )
        .unwrap();
        assert!(matches!(old, AgentEvent::ToolUse { id: None, .. }));
        // What the AI tool is waiting for is shown live and never stored.
        let event = AgentEvent::Status {
            phase: StatusPhase::Waiting,
            text: "Anthropic's servers are busy.".into(),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "status", "phase": "waiting", "text": "Anthropic's servers are busy." })
        );
        assert_eq!(event.ledger_type(), None);
        assert_eq!(
            serde_json::to_value(TurnOutcome::BillingNotAllowed).unwrap(),
            json!("billingNotAllowed")
        );
        let event = AgentEvent::MemoryShortened {
            detail: "Claude Code shortened its memory of this conversation.".into(),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "memoryShortened",
                    "detail": "Claude Code shortened its memory of this conversation." })
        );
        assert_eq!(event.ledger_type(), Some("agent.memory_shortened"));
    }

    #[test]
    fn only_durable_activity_has_a_ledger_type() {
        let text = |t: &str| t.to_owned();
        assert_eq!(
            AgentEvent::TextDelta { text: text("a") }.ledger_type(),
            None
        );
        assert_eq!(
            AgentEvent::Message { text: text("a") }.ledger_type(),
            Some("agent.message")
        );
        assert_eq!(
            AgentEvent::Usage {
                usage: TokenUsage::default()
            }
            .ledger_type(),
            None
        );
    }

    fn window(minutes: u64, used: u8, resets_at: u64, models: Option<&str>) -> PlanWindow {
        PlanWindow {
            minutes: Some(minutes),
            used_percent: Some(used),
            resets_at: Some(resets_at),
            models: models.map(str::to_owned),
        }
    }

    fn report(windows: Vec<PlanWindow>, limited: bool) -> PlanReport {
        PlanReport {
            windows,
            limited,
            warning: false,
            plan: None,
            reported_at: 1,
            key_limit: None,
        }
    }

    /// Phase 25, item 4.3: Claude Code reports one window at a time; each report adds to the
    /// last, and a window that has started again is dropped.
    #[test]
    fn plan_reports_add_up_and_say_when_a_full_window_resets() {
        const H: u64 = 3_600_000;
        let week = report(vec![window(10_080, 40, 100 * H, None)], false);
        let five = report(vec![window(300, 62, 5 * H, None)], false);
        let both = five.clone().merged_with(Some(&week), H);
        assert_eq!(
            both.windows.iter().map(|w| w.minutes).collect::<Vec<_>>(),
            [Some(300), Some(10_080)]
        );
        // The same window again replaces it; one past its reset is dropped.
        let later = report(vec![window(300, 70, 5 * H, None)], false).merged_with(Some(&both), H);
        assert_eq!(later.windows[0].used_percent, Some(70));
        assert_eq!(later.windows.len(), 2);
        let after = report(vec![window(10_080, 41, 100 * H, Some("Opus"))], false)
            .merged_with(Some(&later), 6 * H);
        assert_eq!(
            after
                .windows
                .iter()
                .map(|w| (w.minutes, w.models.as_deref()))
                .collect::<Vec<_>>(),
            [(Some(10_080), None), (Some(10_080), Some("Opus"))]
        );

        // The full window's reset; a report that says only "limited" gives its latest.
        let full = report(
            vec![
                window(300, 100, 5 * H, None),
                window(10_080, 40, 100 * H, None),
            ],
            true,
        );
        assert_eq!(full.reset_after(H), Some(5 * H));
        let limited = report(
            vec![
                window(300, 90, 5 * H, None),
                window(10_080, 95, 100 * H, None),
            ],
            true,
        );
        assert_eq!(limited.reset_after(H), Some(100 * H));
        // Not at the limit, past, or too far: none.
        assert_eq!(week.reset_after(H), None);
        assert_eq!(full.reset_after(6 * H), None);
        let far = report(vec![window(300, 100, 40 * 24 * H, None)], true);
        assert_eq!(far.reset_after(H), None);

        let book = PlanBook::default();
        book.keep(
            "claude-code",
            report(vec![window(300, 10, u64::MAX / 2, None)], false),
        );
        let kept = book.keep(
            "claude-code",
            report(vec![window(10_080, 20, u64::MAX / 2, None)], false),
        );
        assert_eq!(kept.windows.len(), 2);
        assert_eq!(book.latest("claude-code"), Some(kept));
        assert_eq!(book.latest("codex"), None);
    }
}
