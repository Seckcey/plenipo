//! Paid AI tools (Phase 16 Wave 3, ADR-085, paid AI keys with spending caps; ADR-086, OpenRouter
//! through a Plenipo helper).
//!
//! A paid AI tool has no program of its own on this PC: its program is Plenipo's own helper
//! (`plenipo-desktop --plenipo-paid <service>`), which reaches the service with the owner's key.
//! Everything that costs money passes a [`PaidGate`], which the desktop app provides:
//!
//! - **the key**, from the Vault, only while "Let workers use paid AI keys" is on, handed to the
//!   helper on the first line of its standard input and nowhere else;
//! - **the spending caps**: before a task's request is sent, the most it could cost is set aside
//!   (and the task does not start if it could pass a cap); when it ends, what it cost is
//!   recorded.
//!
//! The most a task could cost is known before it starts because the helper sends at most
//! `--max-input-bytes` of conversation text (a token is at least one byte of text) and asks for
//! at most `--max-output-tokens`. This module keeps the conversation files the helper and the
//! adapter share, so both count the same bytes.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent::adapter::{
    cap, model_name, Parsed, ProbeOutput, ProcessEnd, ProviderSession, RuntimeAdapter, Stop,
    TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AgentEvent, AuthState, AuthStatus, Effort, KeyLimit, KnownModel, Maker, NoticeLevel,
    PlanReport, RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::dto::TokenUsage;
use crate::pricing::{dollars_to_micros, Price};

/// The argument that selects the helper (first after the program name).
pub const HELPER_ARG: &str = "--plenipo-paid";
/// The most conversation text a task sends (about 100,000 tokens).
pub const MAX_INPUT_BYTES: u64 = 400_000;
/// The longest answer a task may ask for.
pub const MAX_OUTPUT_TOKENS: u64 = 32_000;
/// The longest answer a task asks for, unless the model's list says less.
pub const DEFAULT_OUTPUT_TOKENS: u64 = 16_000;
/// Room for the service's own wrapping of each message (roles, separators), in tokens, beside
/// the text itself.
const PER_MESSAGE_TOKENS: u64 = 8;
const FIXED_TOKENS: u64 = 256;
/// The largest conversation file read (the helper keeps each well under it).
const MAX_CONVERSATION_FILE: u64 = 16 * 1024 * 1024;
/// The conversation text kept on file: twice what a task sends, so the newest part is always
/// there to send.
const KEPT_TEXT: usize = 2 * MAX_INPUT_BYTES as usize;
/// The most messages kept on file: their wrapping (8 tokens each) stays well inside the room left
/// under a price step (ADR-087 §3).
const KEPT_MESSAGES: usize = 500;

/// The least any step on a paid AI tool can set aside at `price`: its shortest words, its fixed
/// wrapping, and the answer it asks for. A route with less left under the caps cannot run
/// (the Router skips it, ADR-085 §6).
pub fn smallest_step_cost(price: &Price) -> u64 {
    price.most(
        1 + 2 * PER_MESSAGE_TOKENS + FIXED_TOKENS,
        DEFAULT_OUTPUT_TOKENS,
    )
}

/// The helper's arguments for one step: its limits, and the price set aside for, so the service
/// sends the request nowhere dearer.
pub fn step_args(limits: &PaidLimits, price: &Price) -> Vec<String> {
    let mut args = limits.args();
    args.extend([
        "--price-input".into(),
        price.input.to_string(),
        "--price-output".into(),
        price.output.to_string(),
    ]);
    args
}

// ---- The gate the desktop app provides ------------------------------------------------------

/// A saved paid key: its Vault reference, the owner's name for it, and the key itself (never
/// printed: `Debug` hides it).
#[derive(Clone)]
pub struct PaidKey {
    pub id: String,
    pub name: String,
    value: String,
}

impl PaidKey {
    pub fn new(id: impl Into<String>, name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: value.into(),
        }
    }

    /// The first line of the helper's standard input: `{"key":"…"}`.
    pub fn stdin_line(&self) -> String {
        format!("{}\n", json!({ "key": self.value }))
    }

    /// The key itself, for the secret filter.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for PaidKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaidKey")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("value", &"[hidden]")
            .finish()
    }
}

/// A paid task about to send its request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaidCharge {
    pub task_id: String,
    pub runtime_id: String,
    pub model: String,
    pub key_id: String,
    pub key_name: String,
    /// The most the request could cost, in millionths of a dollar.
    pub most_micros: u64,
}

/// What a finished paid task cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaidBill {
    /// Its cost: the service's own bill (`by_service`), or its token counts priced.
    Spent { micros: u64, by_service: bool },
    /// The bill could not be read; why, in plain words.
    NotPriced(String),
    /// The request was never sent.
    NotSent,
}

/// What the desktop app provides for paid AI tools: the key, and the spending caps.
pub trait PaidGate: Send + Sync {
    /// The key saved for `runtime_id`, while paid keys are switched on; otherwise why not, in
    /// plain words.
    fn key(&self, runtime_id: &str) -> Result<PaidKey, String>;
    /// Set aside the most `charge` could cost under the owner's caps: a ticket to settle, or the
    /// reason it may not start.
    fn set_aside(&self, charge: &PaidCharge) -> Result<String, String>;
    /// The task ended: what it cost. Returns the caps the month's spending now passes (the
    /// task's work stops, and the owner is told).
    fn settle(&self, ticket: &str, bill: &PaidBill) -> Vec<String>;
}

/// A gate kept in memory, for tests: one key for the paid AI tools it names (or every one, or
/// none), a refusal to give when asked, and every charge and bill it saw.
#[derive(Debug, Default)]
pub struct MemoryPaidGate {
    key: std::sync::Mutex<Option<PaidKey>>,
    /// The paid AI tools that have the key; none: every one.
    only: std::sync::Mutex<Option<Vec<String>>>,
    refusal: std::sync::Mutex<Option<String>>,
    charges: std::sync::Mutex<Vec<PaidCharge>>,
    bills: std::sync::Mutex<Vec<(String, PaidBill)>>,
}

impl MemoryPaidGate {
    /// A gate with a test key saved.
    pub fn with_key() -> Self {
        let gate = Self::default();
        gate.set_key(Some(PaidKey::new(
            "paid-key-test",
            "Test key",
            "sk-or-v1-test-key-not-real-0123456789",
        )));
        gate
    }

    /// A gate with a test key saved for `runtime_ids` only.
    pub fn with_key_for(runtime_ids: &[&str]) -> Self {
        let gate = Self::with_key();
        *gate.only.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(runtime_ids.iter().map(|id| (*id).to_owned()).collect());
        gate
    }

    /// The key for every paid AI tool from now on.
    pub fn key_for_every_tool(&self) {
        *self.only.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    pub fn set_key(&self, key: Option<PaidKey>) {
        *self.key.lock().unwrap_or_else(|p| p.into_inner()) = key;
    }

    /// Refuse the next charges with `why` (the spending caps), or stop refusing.
    pub fn refuse(&self, why: Option<&str>) {
        *self.refusal.lock().unwrap_or_else(|p| p.into_inner()) = why.map(str::to_owned);
    }

    pub fn charges(&self) -> Vec<PaidCharge> {
        self.charges
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn bills(&self) -> Vec<(String, PaidBill)> {
        self.bills.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl PaidGate for MemoryPaidGate {
    fn key(&self, runtime_id: &str) -> Result<PaidKey, String> {
        let named = self
            .only
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_none_or(|ids| ids.iter().any(|id| id == runtime_id));
        if !named {
            return Err("No paid key is saved for this AI tool.".to_owned());
        }
        self.key
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| "No paid key is saved for this AI tool.".to_owned())
    }

    fn set_aside(&self, charge: &PaidCharge) -> Result<String, String> {
        if let Some(why) = self
            .refusal
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            return Err(why);
        }
        let mut charges = self.charges.lock().unwrap_or_else(|p| p.into_inner());
        charges.push(charge.clone());
        Ok(format!("ticket-{}", charges.len()))
    }

    fn settle(&self, ticket: &str, bill: &PaidBill) -> Vec<String> {
        self.bills
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((ticket.to_owned(), bill.clone()));
        Vec::new()
    }
}

// ---- Conversations, kept in the session's folder (shared with the helper) -------------------

/// Conversation IDs Plenipo gives: letters, digits, and dashes, starting with a letter or digit
/// (so one is never taken for an option).
pub fn valid_conversation_id(id: &str) -> bool {
    id.len() <= 64
        && id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The conversation's file, in the session's own folder.
pub fn conversation_file(dir: &Path, service: &str, id: &str) -> PathBuf {
    dir.join(format!(".plenipo-paid-{service}-{id}.json"))
}

/// A conversation's messages, as the helper keeps them: each a role and its words, nothing
/// else (anything else in the file is left out, so what is sent is always what is counted).
pub fn load_conversation(file: &Path) -> Option<Vec<Value>> {
    if std::fs::metadata(file).ok()?.len() > MAX_CONVERSATION_FILE {
        return None;
    }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    Some(
        v.get("messages")?
            .as_array()?
            .iter()
            .filter_map(|m| {
                let role = m.get("role").and_then(Value::as_str)?;
                let content = m.get("content").and_then(Value::as_str)?;
                matches!(role, "user" | "assistant" | "system")
                    .then(|| json!({ "role": role, "content": content }))
            })
            .collect(),
    )
}

/// Keep the conversation for its next task: its newest whole exchanges, at most twice what a task
/// sends (older ones could never be sent again).
pub fn save_conversation(file: &Path, model: &str, messages: &[Value]) -> std::io::Result<()> {
    let mut start = 0;
    let mut text: usize = messages.iter().map(content_len).sum();
    while (text > KEPT_TEXT || messages.len() - start > KEPT_MESSAGES) && start + 2 < messages.len()
    {
        text -= content_len(&messages[start]) + content_len(&messages[start + 1]);
        start += 2;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        json!({ "model": model, "messages": &messages[start..] }).to_string(),
    )?;
    std::fs::rename(&tmp, file)
}

fn content_len(m: &Value) -> usize {
    m.get("content").and_then(Value::as_str).map_or(0, str::len)
}

/// The note the model gets when earlier messages were left out.
pub const LEFT_OUT_NOTE: &str =
    "Earlier messages of this conversation were left out because it is long.";

/// The messages to send: the newest whole exchanges of `history` that fit with the task's words
/// in `max_input` bytes of text, then the words; and how many earlier messages were left out.
/// None when the words alone do not fit.
pub fn fitting(history: &[Value], prompt: &str, max_input: u64) -> Option<(Vec<Value>, usize)> {
    let max = usize::try_from(max_input).unwrap_or(usize::MAX);
    let mut budget = max.checked_sub(prompt.len())?;
    let mut keep = history.len();
    while keep >= 2 {
        let pair = content_len(&history[keep - 2]) + content_len(&history[keep - 1]);
        if pair + LEFT_OUT_NOTE.len() > budget {
            break;
        }
        budget -= pair;
        keep -= 2;
    }
    let mut messages = Vec::new();
    if keep > 0 {
        messages.push(json!({ "role": "system", "content": LEFT_OUT_NOTE }));
    }
    messages.extend(history[keep..].iter().cloned());
    messages.push(json!({ "role": "user", "content": prompt }));
    Some((messages, keep))
}

/// What a task of conversation `id` would send, with `prompt_len` bytes of words, at most
/// `max_input`: (the text in bytes, and how many messages carry it). The helper is told to send
/// no more than those bytes.
pub fn sent_text(
    dir: &Path,
    service: &str,
    id: Option<&str>,
    prompt_len: usize,
    max_input: u64,
) -> (u64, u64) {
    let history = id
        .filter(|id| valid_conversation_id(id))
        .and_then(|id| load_conversation(&conversation_file(dir, service, id)))
        .unwrap_or_default();
    let text: usize = history.iter().map(content_len).sum::<usize>() + prompt_len;
    let bytes = u64::try_from(text)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(LEFT_OUT_NOTE.len()).unwrap_or(0))
        .min(max_input);
    let messages = u64::try_from(history.len())
        .unwrap_or(u64::MAX)
        .saturating_add(2);
    (bytes, messages)
}

/// The most input tokens a task sending `bytes` of text in `messages` messages could be billed
/// for: a token is at least one byte of text, plus the service's wrapping of each message.
pub fn most_input_tokens(bytes: u64, messages: u64) -> u64 {
    bytes
        .saturating_add(messages.saturating_mul(PER_MESSAGE_TOKENS))
        .saturating_add(FIXED_TOKENS)
}

// ---- OpenRouter (ADR-086) --------------------------------------------------------------------

pub const OPENROUTER: &str = "openrouter";
const OPENROUTER_LABEL: &str = "OpenRouter";
/// The model a worker on OpenRouter uses when it names none: fast and cheap.
pub const OPENROUTER_DEFAULT_MODEL: &str = "qwen/qwen3.8-flash";
/// Effort levels OpenRouter passes on as `reasoning.effort` to models that think.
const OPENROUTER_EFFORT: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High];

#[derive(Debug, Default, Clone, Copy)]
pub struct OpenRouter;

impl OpenRouter {
    /// The short checked list (the owner's choice 7): the newest general and coding models from
    /// Qwen, Mistral, and Meta's Llama, and Kimi K3, as OpenRouter listed them on 2026-09-30.
    /// Any other OpenRouter model can be named exactly; its price comes from OpenRouter's list.
    fn listed() -> Vec<KnownModel> {
        vec![
            KnownModel::new(
                "qwen/qwen3.8-max-prime",
                "Qwen3.8 Max Prime",
                OPENROUTER_EFFORT,
            )
            .by(makers::ALIBABA),
            KnownModel::new(OPENROUTER_DEFAULT_MODEL, "Qwen3.8 Flash", OPENROUTER_EFFORT)
                .by(makers::ALIBABA)
                .same("qwen3.8-flash"),
            KnownModel::new("mistralai/mistral-medium-3-5", "Mistral Medium 3.5", &[])
                .by(makers::MISTRAL)
                .same("mistral-medium-3.5"),
            KnownModel::new("mistralai/devstral-2512", "Devstral 2 (coding)", &[])
                .by(makers::MISTRAL),
            KnownModel::new("meta-llama/llama-4-maverick", "Llama 4 Maverick", &[])
                .by(makers::META),
            KnownModel::new("meta-llama/llama-4-scout", "Llama 4 Scout", &[]).by(makers::META),
            KnownModel::new("moonshotai/kimi-k3", "Kimi K3", OPENROUTER_EFFORT)
                .by(makers::MOONSHOT)
                .same("kimi-k3"),
        ]
    }
}

/// Who made an OpenRouter model, from the company part of its name ("qwen/…").
fn openrouter_maker(name: &str) -> Option<Maker> {
    let (company, _) = name.split_once('/')?;
    let (id, label) = match company {
        "qwen" => makers::ALIBABA,
        "mistralai" => makers::MISTRAL,
        "meta-llama" => makers::META,
        "moonshotai" => makers::MOONSHOT,
        "openai" => makers::OPENAI,
        "anthropic" => makers::ANTHROPIC,
        "google" => makers::GOOGLE,
        "x-ai" => makers::XAI,
        "deepseek" => makers::DEEPSEEK,
        "z-ai" => makers::ZAI,
        "minimax" => makers::MINIMAX,
        "nvidia" => makers::NVIDIA,
        _ => return None,
    };
    Some(Maker::new(id, label))
}

impl RuntimeAdapter for OpenRouter {
    fn id(&self) -> &'static str {
        OPENROUTER
    }

    fn label(&self) -> &'static str {
        OPENROUTER_LABEL
    }

    fn provider(&self) -> &'static str {
        OPENROUTER
    }

    fn provider_label(&self) -> &'static str {
        OPENROUTER_LABEL
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            billing_checked_per_turn: false,
            tool_posture: "Conversation only: an OpenRouter worker can answer, write, and review \
                           text, but cannot read files or run programs yet."
                .into(),
            effort_levels: OPENROUTER_EFFORT.to_vec(),
            known_models: Self::listed(),
            default_maker: Some(Maker::new(makers::ALIBABA.0, makers::ALIBABA.1)),
            runs_other_makers: true,
        }
    }

    fn checked_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn install_hint(&self) -> &'static str {
        "OpenRouter is built into Plenipo: nothing to install."
    }

    fn login_hint(&self) -> &'static str {
        "Turn on Settings → Switches → Let workers use paid AI keys, set the business's spending \
         cap, then add your OpenRouter key on its card. Plenipo keeps it in Windows Credential \
         Manager and never shows it again."
    }

    fn executable_name(&self) -> &'static str {
        "plenipo-openrouter"
    }

    fn known_locations(&self, _host: &HostEnv) -> Vec<PathBuf> {
        Vec::new()
    }

    fn built_in(&self) -> bool {
        true
    }

    fn bridged(&self) -> bool {
        true
    }

    fn bridge_args(&self) -> Option<Vec<String>> {
        Some(vec![HELPER_ARG.into(), OPENROUTER.into()])
    }

    fn paid(&self) -> bool {
        true
    }

    fn paid_note(&self, key_works: bool) -> Option<String> {
        (!key_works).then(|| {
            "Plenipo has not checked OpenRouter with a real key yet. Its models and prices come \
             from OpenRouter's own list before each task; make a key at openrouter.ai → Keys."
                .into()
        })
    }

    fn default_model(&self) -> Option<&'static str> {
        Some(OPENROUTER_DEFAULT_MODEL)
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["check".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_check(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        // The helper reaches only OpenRouter, and its key comes on its standard input.
        Vec::new()
    }

    fn preassigns_session_id(&self) -> bool {
        true
    }

    fn accepts_tools(&self) -> bool {
        false
    }

    fn reports_memory_shortened(&self) -> bool {
        true
    }

    fn leaves_out(&self, request: &TurnRequest, prompt_bytes: usize) -> bool {
        let ProviderSession::Resume { id } = &request.session else {
            return false;
        };
        if !valid_conversation_id(id) {
            return false;
        }
        let file = conversation_file(&request.working_dir, OPENROUTER, id);
        load_conversation(&file).is_some_and(|history| {
            let prompt = " ".repeat(prompt_bytes.min(MAX_INPUT_BYTES as usize + 1));
            fitting(&history, &prompt, MAX_INPUT_BYTES).is_none_or(|(_, left)| left > 0)
        })
    }

    /// The key's own limit, from the check's answer (OpenRouter's `/key`, in US dollars): what
    /// the key may spend and has spent (Phase 25, item 4.3). OpenRouter says nothing about a plan
    /// window, so there is none.
    fn parse_plan(&self, out: &ProbeOutput) -> Option<PlanReport> {
        let answer = last_json(out)?;
        let limit = answer.get("limit").filter(|l| l.is_object())?;
        let cents = |v: &Value| {
            v.as_f64()
                .filter(|d| d.is_finite() && *d >= 0.0 && *d < 1e9)
                .map(|d| (d * 100.0).round() as u64)
        };
        let used_cents = limit.get("usage").and_then(cents)?;
        let limit_cents = limit.get("limit").and_then(cents);
        Some(PlanReport {
            windows: Vec::new(),
            limited: limit_cents.is_some_and(|l| used_cents >= l),
            warning: false,
            plan: None,
            reported_at: crate::now_ms(),
            key_limit: Some(KeyLimit {
                limit_cents,
                used_cents,
                free_tier: limit
                    .get("freeTier")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
        })
    }

    fn reports_plan_left(&self) -> bool {
        true
    }

    /// The models, from the check's answer (OpenRouter's own list, with prices).
    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        let answer = last_json(out)?;
        let list = answer.get("models")?.as_array()?;
        let listed = Self::listed();
        Some(
            list.iter()
                .filter_map(|m| {
                    let name = m.get("id").and_then(Value::as_str).and_then(model_name)?;
                    let price = m.get("price").and_then(price_of);
                    let known = listed.iter().find(|k| k.name == name);
                    let label = known.map_or_else(
                        || {
                            m.get("name")
                                .and_then(Value::as_str)
                                .map_or_else(|| name.clone(), |n| cap(n, 120))
                        },
                        |k| k.label.clone(),
                    );
                    Some(KnownModel {
                        maker: known
                            .and_then(|k| k.maker.clone())
                            .or_else(|| openrouter_maker(&name)),
                        effort_levels: known.map(|k| k.effort_levels.clone()).unwrap_or_default(),
                        label,
                        name,
                        points_to: None,
                        price,
                        same: known.and_then(|k| k.same.clone()),
                    })
                })
                .collect(),
        )
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| OPENROUTER_DEFAULT_MODEL.to_owned());
        let mut args = vec!["chat".into(), "--model".into(), model];
        match &request.session {
            ProviderSession::New { preassigned } => {
                let id = preassigned
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                args.extend(["--session".into(), id]);
            }
            ProviderSession::Resume { id } => {
                args.extend(["--session".into(), id.clone(), "--resume".into()]);
            }
        }
        if let Some(effort) = request.effort {
            args.extend(["--effort".into(), effort.as_str().into()]);
        }
        // The step's limits ([`PaidLimits::args`]) follow, added by the runtime once it has
        // set aside what the step could cost.
        args
    }

    fn price_of(&self, model: &str, reported: Option<&[KnownModel]>) -> Option<Price> {
        reported?
            .iter()
            .find(|m| m.name == model)
            .and_then(|m| m.price)
            .filter(Price::is_sane)
    }

    fn paid_limits(&self, request: &TurnRequest, prompt_bytes: usize) -> PaidLimits {
        let id = match &request.session {
            ProviderSession::Resume { id } => Some(id.as_str()),
            ProviderSession::New { .. } => None,
        };
        let (bytes, messages) = sent_text(
            &request.working_dir,
            OPENROUTER,
            id,
            prompt_bytes,
            MAX_INPUT_BYTES,
        );
        PaidLimits {
            input_bytes: bytes,
            input_tokens: most_input_tokens(bytes, messages),
            output_tokens: DEFAULT_OUTPUT_TOKENS,
        }
    }

    fn parser(&self, _request: &TurnRequest) -> Box<dyn TurnParser> {
        Box::new(PaidParser::new(OPENROUTER_LABEL))
    }
}

/// How much a paid task may send and ask for, set before it starts, so the most it could cost
/// is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaidLimits {
    /// Conversation text the helper may send, in bytes.
    pub input_bytes: u64,
    /// The most input tokens that text could be billed as.
    pub input_tokens: u64,
    /// The longest answer it asks for, in tokens.
    pub output_tokens: u64,
}

impl PaidLimits {
    /// The helper's arguments that hold the step to these limits.
    pub fn args(&self) -> Vec<String> {
        vec![
            "--max-input-bytes".into(),
            self.input_bytes.to_string(),
            "--max-output-tokens".into(),
            self.output_tokens.to_string(),
        ]
    }
}

fn price_of(v: &Value) -> Option<Price> {
    Some(Price {
        input: v.get("input")?.as_u64()?,
        cached_input: v.get("cachedInput").and_then(Value::as_u64),
        output: v.get("output")?.as_u64()?,
        // Storing input for reuse, where it costs more: Anthropic's models (Phase 25, ADR-202).
        cache_write: v.get("cacheWrite").and_then(Value::as_u64),
    })
}

pub(crate) fn last_json(out: &ProbeOutput) -> Option<Value> {
    out.stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
}

/// The helper's `check`: the key works, or it needs a new one.
pub fn parse_check(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<String>, detail: Option<String>| AuthStatus {
        state,
        method,
        detail,
    };
    if let Some(e) = &out.spawn_error {
        return status(
            AuthState::Unknown,
            None,
            Some(format!("The key check could not start: {e}")),
        );
    }
    if out.timed_out {
        return status(
            AuthState::Unknown,
            None,
            Some("The key check timed out.".into()),
        );
    }
    let Some(v) = last_json(out) else {
        return status(
            AuthState::Unknown,
            None,
            Some("The key check gave no answer.".into()),
        );
    };
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return status(AuthState::Unknown, None, Some(cap(e, 300)));
    }
    match v.get("signedIn").and_then(Value::as_bool) {
        Some(true) => status(
            AuthState::PaidKey,
            Some("Paid key (pay per use)".into()),
            None,
        ),
        Some(false) => status(
            AuthState::SignedOut,
            None,
            Some(cap(
                v.get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("The key was refused: it needs a new key."),
                300,
            )),
        ),
        None => status(
            AuthState::Unknown,
            None,
            Some("The key check gave no answer.".into()),
        ),
    }
}

/// Reads the helper's lines (the same for every paid service).
pub struct PaidParser {
    state: TurnState,
    /// The service's own bill, when it sent one.
    cost: Option<u64>,
    /// The request was refused, or never reached the service: nothing was billed.
    refused: bool,
    /// The service gave both token counts (without them the price list cannot price the step).
    counted: bool,
}

impl PaidParser {
    pub fn new(label: &'static str) -> Self {
        Self {
            state: TurnState::new(label),
            cost: None,
            refused: false,
            counted: false,
        }
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

impl TurnParser for PaidParser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let parsed = match v.get("type").and_then(Value::as_str) {
            Some("session") => {
                let id = Some(cap(str_of(&v, "id"), 128)).filter(|s| !s.is_empty());
                let model = Some(cap(str_of(&v, "model"), 200)).filter(|s| !s.is_empty());
                self.state.provider_session_id.clone_from(&id);
                self.state.model.clone_from(&model);
                Parsed::one(AgentEvent::SessionStarted {
                    provider_session_id: id,
                    model,
                })
            }
            Some("thinking") => Parsed::one(AgentEvent::Reasoning {
                text: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
            }),
            Some("text") => Parsed::one(AgentEvent::TextDelta {
                text: str_of(&v, "text").to_owned(),
            }),
            Some("answer") => {
                let text = str_of(&v, "text");
                if text.trim().is_empty() {
                    Parsed::none()
                } else {
                    self.state.last_message = Some(text.to_owned());
                    Parsed::one(AgentEvent::Message {
                        text: text.to_owned(),
                    })
                }
            }
            Some("done") => {
                self.state.completed = true;
                self.counted = ["inputTokens", "outputTokens"]
                    .iter()
                    .all(|k| v.get(*k).and_then(Value::as_u64).is_some());
                let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
                self.state.provider_duration_ms = v.get("durationMs").and_then(Value::as_u64);
                let usage = TokenUsage {
                    input_tokens: n("inputTokens"),
                    cached_input_tokens: n("cachedTokens"),
                    output_tokens: n("outputTokens"),
                };
                self.state.usage = Some(usage);
                // The service's own bill, as exact decimal text (OpenRouter's `usage.cost`).
                self.cost = match v.get("costDollars") {
                    Some(Value::String(s)) => dollars_to_micros(s),
                    Some(Value::Number(n)) => dollars_to_micros(&n.to_string()),
                    _ => None,
                };
                let mut parsed = Parsed::one(AgentEvent::Usage { usage });
                if str_of(&v, "reason") == "length" {
                    parsed.events.push(AgentEvent::Notice {
                        level: NoticeLevel::Warning,
                        text: "The answer was cut off at its length limit.".into(),
                    });
                }
                parsed
            }
            Some("notice") if v.get("leftOut").and_then(Value::as_u64).unwrap_or(0) > 0 => {
                Parsed::one(AgentEvent::MemoryShortened {
                    detail: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
                })
            }
            Some("notice") => Parsed::one(AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: cap(str_of(&v, "text"), MAX_EVENT_TEXT),
            }),
            Some("error") => {
                let message = cap(str_of(&v, "message"), MAX_EVENT_TEXT);
                let kind = str_of(&v, "kind");
                // Refused by the service before any answer, stopped by Guard, or never sent:
                // nothing was billed.
                self.refused = matches!(
                    kind,
                    "key"
                        | "credit"
                        | "limit"
                        | "refused"
                        | "guard"
                        | "input"
                        | "unreached"
                        | "busy"
                );
                let outcome = match kind {
                    "key" => Some(TurnOutcome::AuthRequired),
                    // Out of credit on the service, or its own usage limit: this way to the
                    // model is held back, as a usage limit is, and a backup can run.
                    "credit" | "limit" => Some(TurnOutcome::UsageLimited),
                    "guard" | "service" | "unreached" | "busy" => {
                        Some(TurnOutcome::ProviderUnavailable)
                    }
                    _ => None,
                };
                match outcome {
                    Some(outcome) => {
                        self.state.stop = Some(Stop {
                            outcome,
                            reason: message,
                        });
                    }
                    None => self.state.error = Some(message),
                }
                Parsed::none()
            }
            _ => {
                self.state.unknown += 1;
                return Parsed::none();
            }
        };
        self.state.understood += 1;
        parsed
    }

    fn stderr(&mut self, text: &str) {
        self.state.stderr(text);
    }

    fn finish(&mut self, end: &ProcessEnd) -> TurnResult {
        self.state.finish(end)
    }

    fn paid_bill(&self, price: &Price, started: bool) -> Option<PaidBill> {
        Some(if !started || self.refused {
            PaidBill::NotSent
        } else if let Some(micros) = self.cost {
            PaidBill::Spent {
                micros,
                by_service: true,
            }
        } else if let (true, true, Some(usage)) =
            (self.state.completed, self.counted, self.state.usage)
        {
            PaidBill::Spent {
                micros: price.bill(&usage),
                by_service: false,
            }
        } else {
            PaidBill::NotPriced(
                "The answer stopped before its bill arrived, so it counts at the most it could \
                 have cost."
                    .into(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ExecutionState;

    fn probe(stdout: &str) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
            stdout: stdout.into(),
            ..ProbeOutput::default()
        }
    }

    #[test]
    fn a_key_is_never_printed_and_goes_in_as_the_first_line() {
        let key = PaidKey::new("paid-key-1", "Office key", "sk-or-v1-secret");
        assert!(!format!("{key:?}").contains("sk-or-v1-secret"));
        assert_eq!(key.stdin_line(), "{\"key\":\"sk-or-v1-secret\"}\n");
        assert_eq!(key.value(), "sk-or-v1-secret");
    }

    #[test]
    fn the_check_says_whether_the_key_works() {
        let ok = parse_check(&probe("{\"signedIn\":true,\"models\":[]}\n"));
        assert_eq!(ok.state, AuthState::PaidKey);
        let refused = parse_check(&probe(
            "{\"signedIn\":false,\"reason\":\"OpenRouter refused the key (401): it needs a new key.\"}",
        ));
        assert_eq!(refused.state, AuthState::SignedOut);
        assert!(refused.detail.unwrap().contains("needs a new key"));
        assert_eq!(
            parse_check(&probe("{\"error\":\"down\"}")).state,
            AuthState::Unknown
        );
        assert_eq!(parse_check(&probe("")).state, AuthState::Unknown);
    }

    /// Phase 25, item 4.3: OpenRouter's key limit is shown, not thrown away.
    #[test]
    fn the_check_reads_the_keys_own_limit() {
        let out = probe(
            &json!({ "signedIn": true, "models": [],
                     "limit": { "limit": 10.0, "usage": 3.204, "freeTier": false } })
            .to_string(),
        );
        let plan = OpenRouter.parse_plan(&out).unwrap();
        assert_eq!(
            plan.key_limit,
            Some(KeyLimit {
                limit_cents: Some(1000),
                used_cents: 320,
                free_tier: false,
            })
        );
        assert!(plan.windows.is_empty() && !plan.limited);
        assert!(OpenRouter.reports_plan_left());
        // No limit on the key, and a key used up.
        let none = probe(r#"{"signedIn":true,"limit":{"limit":null,"usage":2,"freeTier":true}}"#);
        let plan = OpenRouter.parse_plan(&none).unwrap();
        assert_eq!(plan.key_limit.unwrap().limit_cents, None);
        let spent = probe(r#"{"signedIn":true,"limit":{"limit":5,"usage":5}}"#);
        assert!(OpenRouter.parse_plan(&spent).unwrap().limited);
        // Nothing said: nothing worked out.
        assert!(OpenRouter
            .parse_plan(&probe(r#"{"signedIn":true,"limit":null}"#))
            .is_none());
    }

    #[test]
    fn the_models_come_with_their_makers_and_prices() {
        let out = probe(
            &json!({ "signedIn": true, "models": [
                { "id": "moonshotai/kimi-k3", "name": "MoonshotAI: Kimi K3",
                  "price": { "input": 3_000_000, "cachedInput": null, "output": 15_000_000 } },
                { "id": "someco/new-model", "name": "Someco: New", "price": null },
                { "id": "deepseek/deepseek-v4", "price": { "input": 1, "output": 2 } },
            ] })
            .to_string(),
        );
        let models = OpenRouter.parse_models(&out).unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].label, "Kimi K3", "the checked list's name");
        assert_eq!(models[0].maker.as_ref().unwrap().id, "moonshot");
        assert_eq!(models[0].price.unwrap().output, 15_000_000);
        assert!(models[1].maker.is_none(), "a company Plenipo does not know");
        assert!(models[1].price.is_none());
        assert_eq!(models[2].maker.as_ref().unwrap().id, "deepseek");
        assert_eq!(
            OpenRouter.price_of("moonshotai/kimi-k3", Some(&models)),
            models[0].price
        );
        assert_eq!(OpenRouter.price_of("someco/new-model", Some(&models)), None);
        assert_eq!(OpenRouter.price_of("moonshotai/kimi-k3", None), None);
    }

    #[test]
    fn every_listed_model_says_who_made_it() {
        for m in OpenRouter.capabilities().known_models {
            assert!(m.maker.is_some(), "{}", m.name);
            assert_eq!(openrouter_maker(&m.name), m.maker, "{}", m.name);
        }
    }

    #[test]
    fn a_task_names_its_limits_and_never_its_key() {
        let request = TurnRequest {
            session: ProviderSession::New {
                preassigned: Some("c-1".into()),
            },
            model: Some("moonshotai/kimi-k3".into()),
            effort: Some(Effort::High),
            billing_confirmed: false,
            tools: None,
            working_dir: PathBuf::from("."),
        };
        let args = OpenRouter.turn_args(&request);
        assert_eq!(
            args,
            [
                "chat",
                "--model",
                "moonshotai/kimi-k3",
                "--session",
                "c-1",
                "--effort",
                "high"
            ]
        );
        let limits = PaidLimits {
            input_bytes: 1_000,
            input_tokens: 1_300,
            output_tokens: 16_000,
        };
        assert_eq!(
            limits.args(),
            ["--max-input-bytes", "1000", "--max-output-tokens", "16000"]
        );
        assert!(
            args.iter().all(|a| !a.contains("sk-")),
            "no key on the command line"
        );
        assert_eq!(
            OpenRouter.bridge_args().unwrap(),
            [HELPER_ARG.to_owned(), OPENROUTER.to_owned()]
        );
    }

    #[test]
    fn the_most_a_task_sends_counts_its_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let (bytes, messages) = sent_text(dir.path(), OPENROUTER, Some("c-1"), 10, MAX_INPUT_BYTES);
        assert_eq!(messages, 2);
        assert_eq!(bytes, 10 + LEFT_OUT_NOTE.len() as u64);
        let file = conversation_file(dir.path(), OPENROUTER, "c-1");
        save_conversation(
            &file,
            "m",
            &[
                json!({ "role": "user", "content": "x".repeat(500) }),
                json!({ "role": "assistant", "content": "y".repeat(500) }),
            ],
        )
        .unwrap();
        let (bytes, messages) = sent_text(dir.path(), OPENROUTER, Some("c-1"), 10, MAX_INPUT_BYTES);
        assert_eq!((bytes, messages), (1_010 + LEFT_OUT_NOTE.len() as u64, 4));
        assert_eq!(
            sent_text(dir.path(), OPENROUTER, Some("c-1"), 10, 600).0,
            600
        );
        assert_eq!(most_input_tokens(1_000, 4), 1_000 + 32 + 256);
        // A new conversation's history is not read.
        assert_eq!(
            sent_text(dir.path(), OPENROUTER, None, 10, MAX_INPUT_BYTES).1,
            2
        );
    }

    #[test]
    fn a_conversation_sends_at_most_what_was_set_aside_for() {
        let history: Vec<Value> = (0..6)
            .map(|i| {
                json!({ "role": if i % 2 == 0 { "user" } else { "assistant" }, "content": "x".repeat(100) })
            })
            .collect();
        let (sent, left) = fitting(&history, "hi", 10_000).unwrap();
        assert_eq!((sent.len(), left), (7, 0));
        let room = 2 + 200 + LEFT_OUT_NOTE.len() as u64;
        let (sent, left) = fitting(&history, "hi", room).unwrap();
        assert_eq!(left, 4);
        assert_eq!(sent.len(), 1 + 2 + 1);
        let text: usize = sent.iter().map(content_len).sum();
        assert!(text as u64 <= room, "{text}");
        assert!(fitting(&history, &"y".repeat(50), 10).is_none());
    }

    #[test]
    fn a_task_reads_its_answer_counts_and_bill() {
        let mut p = PaidParser::new("OpenRouter");
        for line in [
            r#"{"type":"session","id":"c-1","model":"moonshotai/kimi-k3"}"#,
            r#"{"type":"thinking","text":"Hm"}"#,
            r#"{"type":"text","text":"OK"}"#,
            r#"{"type":"answer","text":"OK"}"#,
            r#"{"type":"done","reason":"stop","inputTokens":12,"cachedTokens":4,"outputTokens":2,"costDollars":"0.0000570","durationMs":900}"#,
        ] {
            p.line(line, false);
        }
        let r = p.finish(&ProcessEnd {
            state: ExecutionState::Succeeded,
            exit_code: Some(0),
            started: true,
            detail: None,
            duration_ms: None,
        });
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("OK"));
        assert_eq!(r.usage.unwrap().input_tokens, 12);
        let price = Price::per_million_dollars(3, 15);
        assert_eq!(
            p.paid_bill(&price, true),
            Some(PaidBill::Spent {
                micros: 57,
                by_service: true
            })
        );
        // Without the service's bill, its token counts are priced.
        let mut q = PaidParser::new("OpenRouter");
        q.line(
            r#"{"type":"done","inputTokens":1000000,"cachedTokens":0,"outputTokens":0}"#,
            false,
        );
        assert_eq!(
            q.paid_bill(&price, true),
            Some(PaidBill::Spent {
                micros: 3_000_000,
                by_service: false
            })
        );
        // An answer cut short before its bill: not priced yet.
        let mut cut = PaidParser::new("OpenRouter");
        cut.line(r#"{"type":"text","text":"O"}"#, false);
        assert!(matches!(
            cut.paid_bill(&price, true),
            Some(PaidBill::NotPriced(_))
        ));
        // Never started: nothing sent.
        assert_eq!(cut.paid_bill(&price, false), Some(PaidBill::NotSent));
    }

    #[test]
    fn a_refused_key_or_no_credit_says_so_and_holds_the_route_back() {
        for (kind, outcome) in [
            ("key", TurnOutcome::AuthRequired),
            ("credit", TurnOutcome::UsageLimited),
            ("limit", TurnOutcome::UsageLimited),
            ("service", TurnOutcome::ProviderUnavailable),
        ] {
            let mut p = PaidParser::new("OpenRouter");
            p.line(
                &json!({ "type": "error", "message": "no", "kind": kind }).to_string(),
                false,
            );
            let r = p.finish(&ProcessEnd {
                state: ExecutionState::Failed,
                exit_code: Some(1),
                started: true,
                detail: None,
                duration_ms: None,
            });
            assert_eq!(r.outcome, outcome, "{kind}");
            let bill = p.paid_bill(&Price::per_million_dollars(3, 15), true);
            if kind == "service" {
                // Failed after the service may have started answering: counted at the most.
                assert!(matches!(bill, Some(PaidBill::NotPriced(_))), "{kind}");
            } else {
                assert_eq!(
                    bill,
                    Some(PaidBill::NotSent),
                    "{kind}: refused, so not billed"
                );
            }
        }
    }

    #[test]
    fn kimi_k3_is_one_model_with_four_ways_to_reach_it() {
        // ADR-036 §4: Kimi Code's subscription, Ollama's paid plan, an OpenRouter key, and a Moonshot key.
        let ways: Vec<String> = crate::agent::builtin_adapters()
            .iter()
            .filter(|a| {
                a.capabilities()
                    .known_models
                    .iter()
                    .any(|m| m.same.as_deref() == Some("kimi-k3"))
            })
            .map(|a| a.id().to_owned())
            .collect();
        assert_eq!(ways, ["kimi", "ollama", "openrouter", "moonshot-key"]);
    }

    #[test]
    fn a_conversation_id_is_never_taken_for_an_option() {
        assert!(valid_conversation_id(
            "0f8fad5b-d9cb-469f-a165-70867728950e"
        ));
        assert!(!valid_conversation_id("--max-output-tokens"));
        assert!(!valid_conversation_id("-x"));
        assert!(!valid_conversation_id(""));
        assert!(!valid_conversation_id("../x"));
    }

    #[test]
    fn a_conversation_file_holds_only_words_and_its_newest_part() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("c.json");
        // Anything but a role and its words is left out, so what is sent is what is counted.
        std::fs::write(
            &file,
            json!({ "messages": [
                { "role": "user", "content": "hello" },
                { "role": "assistant", "content": [{ "type": "text", "text": "hidden" }] },
                { "role": "tool", "content": "x" },
                { "role": "assistant", "content": "hi", "extra": "dropped" },
            ] })
            .to_string(),
        )
        .unwrap();
        let history = load_conversation(&file).unwrap();
        assert_eq!(
            history,
            vec![
                json!({ "role": "user", "content": "hello" }),
                json!({ "role": "assistant", "content": "hi" }),
            ]
        );
        // Saved with more than twice what a task sends: the oldest exchanges go.
        let long = "x".repeat(300_000);
        let messages: Vec<Value> = (0..6)
            .map(|i| {
                json!({ "role": if i % 2 == 0 { "user" } else { "assistant" }, "content": long })
            })
            .collect();
        save_conversation(&file, "m", &messages).unwrap();
        let kept = load_conversation(&file).unwrap();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().map(content_len).sum::<usize>() <= KEPT_TEXT);
    }

    #[test]
    fn a_bill_without_counts_is_not_priced_and_a_refused_request_is_not_billed() {
        let price = Price::per_million_dollars(3, 15);
        let mut p = PaidParser::new("OpenRouter");
        p.line(
            r#"{"type":"done","inputTokens":12,"outputTokens":null,"costDollars":null}"#,
            false,
        );
        assert!(matches!(
            p.paid_bill(&price, true),
            Some(PaidBill::NotPriced(_))
        ));
        let mut p = PaidParser::new("OpenRouter");
        p.line(r#"{"type":"error","message":"OpenRouter refused this request (403).","kind":"refused"}"#, false);
        assert!(matches!(p.paid_bill(&price, true), Some(PaidBill::NotSent)));
    }

    #[test]
    fn the_smallest_step_sets_aside_its_answer_at_least() {
        let price = Price::per_million_dollars(3, 15);
        // 16,000 answer tokens at $15 a million is $0.24, and a little input.
        let least = smallest_step_cost(&price);
        assert!((240_000..250_000).contains(&least), "{least}");
        let args = step_args(
            &PaidLimits {
                input_bytes: 10,
                input_tokens: 300,
                output_tokens: 16_000,
            },
            &price,
        );
        assert!(
            args.windows(2).any(|w| w == ["--price-output", "15000000"]),
            "{args:?}"
        );
    }

    #[test]
    fn a_conversation_keeps_at_most_five_hundred_messages() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("c.json");
        let messages: Vec<Value> = (0..1_200)
            .map(|i| json!({ "role": if i % 2 == 0 { "user" } else { "assistant" }, "content": "hi" }))
            .collect();
        save_conversation(&file, "m", &messages).unwrap();
        let kept = load_conversation(&file).unwrap();
        assert!(kept.len() <= KEPT_MESSAGES, "{}", kept.len());
        // Whole exchanges: it still starts with the owner's words.
        assert_eq!(kept[0]["role"], "user");
    }

    #[test]
    fn a_busy_service_bills_nothing_and_is_unavailable_for_now() {
        let price = Price::per_million_dollars(3, 15);
        let mut p = PaidParser::new("Anthropic");
        p.line(
            r#"{"type":"error","message":"Anthropic is too busy right now (529).","kind":"busy"}"#,
            false,
        );
        assert!(matches!(p.paid_bill(&price, true), Some(PaidBill::NotSent)));
        let end = ProcessEnd {
            state: ExecutionState::Failed,
            exit_code: Some(1),
            started: true,
            detail: None,
            duration_ms: None,
        };
        assert_eq!(p.finish(&end).outcome, TurnOutcome::ProviderUnavailable);
    }
}
