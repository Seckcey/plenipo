//! The AI tools page (Phase 19, ADR-058 to ADR-060): everything about each AI tool in one place —
//! signing in and out, its usage, how it is paid for, its version and updates, and its models.
//!
//! - **Sign in, Reconnect, Sign out** (ADR-058): a terminal tab runs the tool's own sign-in or
//!   sign-out program, from its adapter's fixed list, directly (no shell) and with only the
//!   environment its tasks get. The owner signs in; Plenipo never sees, keeps, or passes it, and
//!   never types into the tab. Guard decides first; new tasks on the tool wait while the program
//!   runs; when it ends, Plenipo checks the tool again by itself.
//! - **Updates** (ADR-059): once a day Plenipo looks for a new version — the tool's own check
//!   where it has one, otherwise its maker's published release list, read-only, through Guard's
//!   gate for Plenipo's own requests. It runs the tool's own update command only when no task is
//!   using it (it waits), then checks the version, the sign-in, that the tool still answers the
//!   way Plenipo reads it, and its models. A failed update leaves the old version working, or
//!   puts it back with the tool's own command; failing that, the tool gets no tasks and the card
//!   says why. With automatic updates off (the default), Plenipo only tells the owner.
//! - **Usage** (ADR-060): added up from the token counts saved with each step; "plan left" only
//!   as the tool reports it (Claude Code's task stream, Codex's app server).
//! - **Models** (ADR-060): the tool's own list, where it has one; a model it reports that
//!   Plenipo has not checked shows as new and can be chosen.
//!
//! What a sign-in tab shows, and what the owner types there, is never kept, logged, or read. An
//! update's output stays in memory, in Runs; only a failure's first line, with secrets hidden,
//! is recorded.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use plenipo_guard::{AiToolAction, AiToolBusy, AiToolRequest, OutboundRules, Purpose};
use plenipo_ledger::NewEvent;
use plenipo_runtime::agent::adapter::{NewestVersion, PublishedList, StatusCheck};
use plenipo_runtime::agent::{
    AccountAction, AgentRuntime, AgentRuntimeInfo, AuthState, HoldFor, InstallState, NotFree,
    PlanReport, ReportedModels, RuntimeAdapter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::broker::Broker;
use crate::broker::TerminalSink;
use crate::dto::{TerminalInfo, TerminalPlace};
use crate::error::{BrokerError, Result};
use crate::programs::{self, Run};
use crate::terminal::{Ending, ShellProgram};
use crate::updates;

/// The Ledger setting that keeps the owner's choice (automatic updates) and what Plenipo last
/// learned about each AI tool.
pub const SETTING: &str = "ai_tools";
/// Who the owner is in the record.
const OWNER: &str = "owner";
/// Plenipo itself, for what it does on its own (the daily look, automatic updates).
const PLENIPO: &str = "plenipo";
/// An update's time limit.
const UPDATE_TIME_LIMIT: Duration = Duration::from_secs(10 * 60);
/// How long an update waits for the tasks using its tool, at most.
const WAIT_AT_MOST_MS: u64 = 24 * 3_600_000;
/// How often a waiting update looks again.
const WAIT_STEP: Duration = Duration::from_secs(2);
/// The daily look for new versions: at most once this often.
const LOOK_EVERY_MS: u64 = 24 * 3_600_000 - 15 * 60_000;
/// A plan check after a task, at most this often per tool (ADR-060 §3).
const PLAN_CHECK_EVERY_MS: u64 = 5 * 60_000;
/// Most days of usage asked for at once.
pub const MAX_USAGE_DAYS: usize = 60;
/// The most a release list's answer may be.
const MAX_LIST_BYTES: usize = 512 * 1024;

// ---- On screen ------------------------------------------------------------------------------------

/// Where an AI tool's newest version comes from (ADR-059 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NewestFrom {
    /// The tool's own check (Grok).
    Own,
    /// Its maker's published release list (Claude Code, Codex, Ollama).
    Published,
    /// Nothing matches its program: Update checks and installs in one step (Kimi).
    UpdateChecks,
}

/// Where an update is (ADR-059 §3–§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AiToolUpdateState {
    /// Nothing happening.
    Idle,
    /// Waiting for the tasks using the tool to finish.
    Waiting,
    /// The tool's update command is running.
    Updating,
    /// Checking the tool after its update.
    Checking,
    Updated,
    /// The tool said it is up to date.
    UpToDate,
    /// The tool cannot update itself (installed another way): the owner types one command.
    ByHand,
    /// It did not finish: `old_still_works` says whether the old version still works.
    Failed,
}

/// An AI tool's update, as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AiToolUpdate {
    pub state: AiToolUpdateState,
    pub from: Option<String>,
    pub to: Option<String>,
    /// How many tasks are using the tool, while the update waits.
    pub tasks_using: u32,
    /// What happened, in plain words, when there is more to say.
    pub message: Option<String>,
    /// After a failure: the old version still works.
    pub old_still_works: Option<bool>,
    /// Plenipo started it by itself (automatic updates).
    pub automatic: bool,
    #[ts(type = "number | null")]
    pub at: Option<u64>,
}

impl AiToolUpdate {
    fn idle() -> Self {
        Self {
            state: AiToolUpdateState::Idle,
            from: None,
            to: None,
            tasks_using: 0,
            message: None,
            old_still_works: None,
            automatic: false,
            at: None,
        }
    }

    fn busy(&self) -> bool {
        matches!(
            self.state,
            AiToolUpdateState::Waiting | AiToolUpdateState::Updating | AiToolUpdateState::Checking
        )
    }
}

/// How an AI tool is paid for (ADR-060 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PaymentMethod {
    /// The owner's subscription.
    Subscription,
    /// A paid AI key, pay per use, within the spending caps (Phase 16 Wave 3, ADR-085): the way a
    /// paid AI tool (OpenRouter) is paid for.
    PaidKey,
}

/// What the AI tools page shows about one AI tool, beyond its check ([`AgentRuntimeInfo`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AiToolState {
    pub runtime_id: String,
    /// It comes with Plenipo (a paid AI tool's helper, ADR-085): it is updated with Plenipo,
    /// never on its own.
    pub built_in: bool,
    pub newest_from: NewestFrom,
    /// The newest version, when Plenipo has looked.
    pub newest: Option<String>,
    #[ts(type = "number | null")]
    pub newest_checked_at: Option<u64>,
    /// Why the last look for its newest version did not work.
    pub newest_problem: Option<String>,
    /// The tool has its own update command (Ollama updates from its tray app).
    pub can_update: bool,
    pub update: AiToolUpdate,
    /// The official command the owner types when the tool cannot update itself.
    pub update_by_hand: Option<String>,
    /// Plenipo gives the tool no tasks, and why (a failed update, ADR-059 §6).
    pub out_of_service: Option<String>,
    /// The tool officially reports how much of the plan is used (ADR-060 §3).
    pub reports_plan_left: bool,
    /// What it last reported.
    pub plan: Option<PlanReport>,
    pub payment: PaymentMethod,
    /// A paid AI tool's saved key, by name (never the key; ADR-085).
    pub paid_key: Option<plenipo_guard::PaidKeyInfo>,
    /// Why a paid AI tool's key cannot be saved or used now (paid keys switched off), in plain
    /// words. A spending cap is never needed.
    pub paid_blocked: Option<String>,
    /// Where a paid AI tool's key is kept, as the screen names it ("Windows Credential
    /// Manager"); none for a subscription AI tool.
    pub key_kept_in: Option<String>,
    /// A paid AI tool's words about what is not checked yet, and where to make a key (ADR-087).
    pub paid_note: Option<String>,
    /// The tool has its own list of models (Claude Code's come with Plenipo's updates).
    pub has_model_list: bool,
    /// Asking for its models leaves an empty conversation in its history (Kimi), so Plenipo
    /// asks only after an update and when the owner presses Check again.
    pub models_check_leaves_a_trace: bool,
    /// A check of this tool is running now.
    pub checking: bool,
}

/// The AI tools page's own part: each tool, and the switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AiToolsPage {
    pub tools: Vec<AiToolState>,
    /// Update AI tools by themselves (off by default: Plenipo asks first).
    pub auto_update: bool,
    #[ts(type = "number | null")]
    pub last_looked_at: Option<u64>,
    /// Plenipo is looking for new versions now.
    pub looking: bool,
}

/// One model's usage on one day (ADR-060 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageModel {
    /// `None`: the AI tool's default model.
    pub model: Option<String>,
    pub tasks: u32,
    /// Steps run, and how many of them the tool reported token counts for.
    pub steps: u32,
    pub counted_steps: u32,
    #[ts(type = "number")]
    pub read: u64,
    #[ts(type = "number")]
    pub reused: u64,
    #[ts(type = "number")]
    pub written: u64,
}

/// One day's usage, by model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageDay {
    /// The day's start (milliseconds since 1970), as the window gave it.
    #[ts(type = "number")]
    pub start: u64,
    pub models: Vec<UsageModel>,
}

/// An AI tool's usage, day by day (ADR-060 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AiToolUsage {
    pub runtime_id: String,
    pub days: Vec<UsageDay>,
}

/// Who started an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateBy {
    Owner,
    Automatic,
}

impl UpdateBy {
    fn word(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Automatic => "automatic",
        }
    }
}

// ---- Kept in the Ledger -------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    auto_update: bool,
    last_looked_at: Option<u64>,
    tools: BTreeMap<String, StoredTool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredTool {
    newest: Option<String>,
    newest_checked_at: Option<u64>,
    newest_problem: Option<String>,
    /// The newest version the owner was told about (once per version).
    announced: Option<String>,
    models: Option<ReportedModels>,
    plan: Option<PlanReport>,
    out_of_service: Option<String>,
    last_update: Option<AiToolUpdate>,
    /// The newest version the tool could not update to by itself (installed another way): not
    /// tried again by itself until a newer one comes (ADR-059 §10).
    by_hand_for: Option<String>,
}

// ---- The service ----------------------------------------------------------------------------------

#[derive(Default)]
struct Live {
    updates: HashMap<String, AiToolUpdate>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    looking: bool,
    checking: HashSet<String>,
    /// When each tool's plan was last asked for after a task.
    plan_checked: HashMap<String, u64>,
    /// Plenipo's own checks running now, by tool: an update waits for them (ADR-059 §3).
    checks: HashMap<String, u32>,
}

/// One of Plenipo's own checks of a tool, running: while it is kept, an update of that tool
/// waits ([`AiTools::start_check`]).
struct CheckRunning {
    tools: AiTools,
    runtime_id: String,
}

impl Drop for CheckRunning {
    fn drop(&mut self) {
        let mut live = lock(&self.tools.inner.live);
        if let Some(n) = live.checks.get_mut(&self.runtime_id) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                live.checks.remove(&self.runtime_id);
            }
        }
    }
}

struct Inner {
    agents: AgentRuntime,
    broker: Broker,
    /// Guard's gate rules for Plenipo's own requests, as this copy of Plenipo was built.
    rules: OutboundRules,
    /// A stand-in for the release lists on this computer (copies built for the tests only).
    release_base: Option<String>,
    live: Mutex<Live>,
    /// Held while the setting is read, changed, and written.
    keeping: Mutex<()>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct AiTools {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> u64 {
    plenipo_ledger::now_ms()
}

/// `candidate` is newer than `current`: numbers compared part by part; a pre-release (`-alpha`)
/// is never offered.
pub fn newer_version(current: &str, candidate: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        v.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u64>().ok())
            .collect()
    }
    if candidate.contains('-') || candidate.contains('+') {
        return false;
    }
    let current = current.split(['-', '+']).next().unwrap_or(current);
    match (parts(current), parts(candidate)) {
        (Some(c), Some(n)) => n > c,
        _ => false,
    }
}

/// A version Plenipo passes to a tool's own command (`claude install <version>`): exactly a
/// version number — two to four numbers with dots between, nothing else.
fn plain_version(version: &str) -> Option<&str> {
    let parts: Vec<&str> = version.split('.').collect();
    let plain = version.len() <= 32
        && (2..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    plain.then_some(version)
}

impl AiTools {
    /// `rules` are Guard's gate rules for Plenipo's own requests; `release_base` a stand-in for
    /// the release lists on this computer, only in copies built for the tests.
    pub fn new(
        agents: AgentRuntime,
        broker: Broker,
        rules: OutboundRules,
        release_base: Option<String>,
    ) -> Self {
        let rules = rules.with_ai_tool_releases(release_base.as_deref());
        Self {
            inner: Arc::new(Inner {
                agents,
                broker,
                rules,
                release_base,
                live: Mutex::new(Live::default()),
                keeping: Mutex::new(()),
            }),
        }
    }

    fn agents(&self) -> &AgentRuntime {
        &self.inner.agents
    }

    fn ledger(&self) -> &Arc<plenipo_ledger::Ledger> {
        self.inner.broker.guard().ledger()
    }

    fn adapter(&self, runtime_id: &str) -> Result<Arc<dyn RuntimeAdapter>> {
        self.agents().adapter_for(runtime_id).ok_or_else(|| {
            BrokerError::Invalid(format!("Plenipo has no AI tool called {runtime_id:?}"))
        })
    }

    fn stored(&self) -> Stored {
        self.ledger()
            .setting(SETTING)
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    /// Change what Plenipo keeps, without an event (meaningful changes record their own). Read
    /// and written in one transaction; when what is kept cannot be read, nothing is written, so
    /// the owner's switch and the tools given no tasks are never lost.
    fn keep(&self, change: impl FnOnce(&mut Stored)) {
        let _held = lock(&self.inner.keeping);
        let kept = self.ledger().change_setting(SETTING, |current| {
            let mut stored: Stored = if current.is_null() {
                Stored::default()
            } else {
                serde_json::from_value(current)
                    .map_err(|e| plenipo_ledger::LedgerError::InvalidInput(e.to_string()))?
            };
            change(&mut stored);
            serde_json::to_value(&stored)
                .map_err(|e| plenipo_ledger::LedgerError::InvalidInput(e.to_string()))
        });
        if let Err(e) = kept {
            log::warn!("the AI tools page could not keep what it learned: {e}");
        }
    }

    fn keep_tool(&self, runtime_id: &str, change: impl FnOnce(&mut StoredTool)) {
        self.keep(|s| change(s.tools.entry(runtime_id.to_owned()).or_default()));
    }

    fn record(&self, actor: &str, event_type: &str, payload: Value) {
        let event = NewEvent {
            source: actor.into(),
            event_type: event_type.into(),
            payload,
            ..NewEvent::default()
        };
        if let Err(e) = self.ledger().append_event(event) {
            log::warn!("an AI tool event could not be recorded: {e}");
        }
    }

    fn info(&self, runtime_id: &str) -> Option<AgentRuntimeInfo> {
        self.agents()
            .runtimes()
            .into_iter()
            .find(|r| r.id == runtime_id)
    }

    /// Start one of Plenipo's own checks of `runtime_id` (its version, sign-in, models, or
    /// plan), which runs the tool's program: never while the tool is held — its sign-in tab is
    /// open, or it is updating — so `None` then, and the check is skipped. While the check runs,
    /// an update of the tool waits.
    fn start_check(&self, runtime_id: &str) -> Option<CheckRunning> {
        *lock(&self.inner.live)
            .checks
            .entry(runtime_id.to_owned())
            .or_default() += 1;
        let running = CheckRunning {
            tools: self.clone(),
            runtime_id: runtime_id.to_owned(),
        };
        // Counted first, then the hold looked at: an update takes its hold first, then looks at
        // the checks, so one of the two always sees the other.
        (!self.agents().held(runtime_id)).then_some(running)
    }

    /// Whether one of Plenipo's own checks of `runtime_id` is running.
    fn checking_now(&self, runtime_id: &str) -> bool {
        lock(&self.inner.live).checks.contains_key(runtime_id)
    }

    /// When Plenipo starts: the models each tool reported earlier, and the tools it gives no
    /// tasks.
    pub fn restore(&self) {
        for (id, tool) in self.stored().tools {
            if let Some(models) = tool.models {
                self.agents().set_reported_models(&id, models);
            }
            if tool.out_of_service.is_some() {
                self.agents().set_out_of_service(&id, tool.out_of_service);
            }
        }
    }

    // ---- The page ---------------------------------------------------------------------------------

    pub fn page(&self) -> AiToolsPage {
        let stored = self.stored();
        let guard_config = self.inner.broker.guard().config().ok();
        let paid_blocked = crate::paid::not_allowed(&self.inner.broker);
        let kept_in = self.inner.broker.secret_store().label().to_owned();
        // The paid AI tools whose key passed its check.
        let key_works: std::collections::HashSet<String> = self
            .agents()
            .runtimes()
            .into_iter()
            .filter(|r| r.ready && r.auth.state == plenipo_runtime::agent::AuthState::PaidKey)
            .map(|r| r.id)
            .collect();
        let live = lock(&self.inner.live);
        let tools = plenipo_runtime::agent::builtin_adapters()
            .into_iter()
            .map(|a| {
                let id = a.id();
                let kept = stored.tools.get(id).cloned().unwrap_or_default();
                let update = live
                    .updates
                    .get(id)
                    .cloned()
                    .or(kept.last_update.clone())
                    .unwrap_or_else(AiToolUpdate::idle);
                AiToolState {
                    runtime_id: id.to_owned(),
                    built_in: a.built_in(),
                    newest_from: match a.newest_version() {
                        NewestVersion::Command(_) => NewestFrom::Own,
                        NewestVersion::Published(_) => NewestFrom::Published,
                        NewestVersion::None => NewestFrom::UpdateChecks,
                    },
                    newest: kept.newest,
                    newest_checked_at: kept.newest_checked_at,
                    newest_problem: kept.newest_problem,
                    can_update: a.update_command().is_some(),
                    update,
                    update_by_hand: a.update_by_hand().map(str::to_owned),
                    out_of_service: self.agents().out_of_service(id),
                    reports_plan_left: a.reports_plan_left(),
                    plan: kept.plan,
                    payment: if a.paid() {
                        PaymentMethod::PaidKey
                    } else {
                        PaymentMethod::Subscription
                    },
                    paid_key: guard_config
                        .as_ref()
                        .filter(|_| a.paid())
                        .and_then(|c| c.paid_key(id).cloned()),
                    paid_blocked: paid_blocked.clone().filter(|_| a.paid()),
                    key_kept_in: a.paid().then(|| kept_in.clone()),
                    paid_note: a.paid_note(key_works.contains(id)),
                    has_model_list: !matches!(
                        a.status_check(std::path::Path::new(".")),
                        StatusCheck::None
                    ),
                    models_check_leaves_a_trace: a.status_check_leaves_a_trace(),
                    checking: live.checking.contains(id),
                }
            })
            .collect();
        AiToolsPage {
            tools,
            auto_update: stored.auto_update,
            last_looked_at: stored.last_looked_at,
            looking: live.looking,
        }
    }

    /// Updates running or waiting now (they count as work going on when Plenipo closes).
    pub fn updates_going(&self) -> usize {
        lock(&self.inner.live)
            .updates
            .values()
            .filter(|u| u.busy())
            .count()
    }

    // ---- Sign in, Reconnect, Sign out (ADR-058) ----------------------------------------------

    /// Open a terminal tab that runs `runtime_id`'s own sign-in or sign-out program. Refused
    /// while a task is using the tool (the card waits and asks again). While the program runs,
    /// new tasks on the tool wait; when it ends, the tool is checked again by itself.
    pub async fn open_account(
        &self,
        runtime_id: &str,
        action: AccountAction,
        cols: u16,
        rows: u16,
        sink: TerminalSink,
    ) -> Result<TerminalInfo> {
        let adapter = self.adapter(runtime_id)?;
        let label = adapter.label();
        let command = adapter.account_command(action);
        let guard_action = match action {
            AccountAction::SignIn => AiToolAction::SignIn,
            AccountAction::SignOut => AiToolAction::SignOut,
        };
        let (hold, tasks_using, busy) = match command {
            Some(_) => match self.agents().hold_if_free(runtime_id, HoldFor::SignIn) {
                Ok(hold) => (Some(hold), 0, None),
                Err(NotFree::Tasks(tasks)) => (None, tasks.len(), None),
                Err(NotFree::Held(HoldFor::Update)) => (None, 0, Some(AiToolBusy::Updating)),
                Err(NotFree::Held(HoldFor::SignIn)) => (None, 0, Some(AiToolBusy::SignInOpen)),
            },
            None => (None, 0, None),
        };
        self.inner
            .broker
            .guard()
            .check_ai_tool_action(&AiToolRequest {
                runtime_id,
                label: Some(label),
                action: guard_action,
                has_command: command.is_some(),
                tasks_using,
                busy,
            })
            .map_err(BrokerError::Invalid)?;
        let (Some(args), Some(hold)) = (command, hold) else {
            return Err(BrokerError::Invalid(format!(
                "{label}'s {} could not start",
                action.noun()
            )));
        };
        let program = self
            .agents()
            .tool_program(runtime_id)
            .map_err(BrokerError::Invalid)?;
        let words = [adapter.executable_name().to_owned()]
            .into_iter()
            .chain(args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        let mut env = plenipo_runtime::policy::build_child_env(&program.env);
        for (name, value) in [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")] {
            env.push((name.into(), value.into()));
        }
        // On Linux, what lets the AI tool open the owner's browser for the sign-in.
        if cfg!(all(unix, not(target_os = "macos"))) {
            for name in [
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "XDG_RUNTIME_DIR",
                "DBUS_SESSION_BUS_ADDRESS",
            ] {
                if let Some(value) = std::env::var_os(name) {
                    env.push((name.into(), value));
                }
            }
        }
        let shell = ShellProgram {
            label: format!("{label}'s {}", action.noun()),
            program: program.executable,
            args,
            env: Some(env),
        };
        let place = TerminalPlace::AiTool {
            runtime_id: runtime_id.to_owned(),
            action,
        };
        let title = format!("{} · {label}", action.title());
        let handle = tokio::runtime::Handle::current();
        let this = self.clone();
        let id = runtime_id.to_owned();
        let after_end = Box::new(move |_ending: &Ending| {
            handle.spawn(async move {
                this.checked_after_sign_in(&id).await;
                // New tasks on the tool go on once it is checked again.
                drop(hold);
            });
        });
        self.inner
            .broker
            .open_program_terminal(place, title, words, &shell, cols, rows, sink, after_end)
    }

    /// The tool is checked again after its sign-in tab's program ended; a changed sign-in is
    /// recorded (its kind only, never an account).
    async fn checked_after_sign_in(&self, runtime_id: &str) {
        lock(&self.inner.live)
            .checking
            .insert(runtime_id.to_owned());
        if let Some((before, now)) = self.agents().recheck(runtime_id).await {
            if before.state != now.auth.state || before.method != now.auth.method {
                self.record(
                    OWNER,
                    "ai_tool.sign_in_changed",
                    json!({
                        "runtime": runtime_id,
                        "from": before.state,
                        "to": now.auth.state,
                        "method": now.auth.method,
                    }),
                );
            }
        }
        lock(&self.inner.live).checking.remove(runtime_id);
    }

    // ---- Check again ---------------------------------------------------------------------------

    /// Check one AI tool again, as the owner asked: its version, its sign-in, its models, and —
    /// where it reports it — its plan. A tool given no tasks after an update gets them again when
    /// it answers.
    pub async fn check(&self, runtime_id: &str) -> Result<AiToolsPage> {
        let adapter = self.adapter(runtime_id)?;
        // While it updates or its sign-in tab is open, those check it when they end.
        let Some(_running) = self.start_check(runtime_id) else {
            return Ok(self.page());
        };
        lock(&self.inner.live)
            .checking
            .insert(runtime_id.to_owned());
        let answers = self.answers(adapter.as_ref()).await;
        if answers.ok && self.agents().out_of_service(runtime_id).is_some() {
            self.agents().set_out_of_service(runtime_id, None);
            self.keep_tool(runtime_id, |t| t.out_of_service = None);
            let _ = self.agents().recheck(runtime_id).await;
        }
        lock(&self.inner.live).checking.remove(runtime_id);
        Ok(self.page())
    }

    /// Check the tool again and ask it for its models (and plan): whether it answers the way
    /// Plenipo reads it (ADR-059 §5), and its version.
    async fn answers(&self, adapter: &dyn RuntimeAdapter) -> Answers {
        let id = adapter.id();
        let Some((_, info)) = self.agents().recheck(id).await else {
            return Answers::default();
        };
        let version = info.installation.version.clone();
        let installed = info.installation.state == InstallState::Installed && version.is_some();
        // Its sign-in check was understood (a signed-out tool still answers).
        let understood = info.auth.state != AuthState::Unknown;
        let listed = match adapter.status_check(std::path::Path::new(".")) {
            StatusCheck::None => true,
            _ => self.status_check(id).await,
        };
        Answers {
            ok: installed && understood && listed,
            version,
        }
    }

    /// The tool's short check: its models (kept, and a change recorded) and its plan.
    async fn status_check(&self, runtime_id: &str) -> bool {
        match self.agents().status_check(runtime_id).await {
            Ok(answer) => {
                if let Some(models) = answer.models {
                    self.models_reported(runtime_id, models);
                }
                if let Some(plan) = answer.plan {
                    self.keep_tool(runtime_id, |t| t.plan = Some(plan));
                }
                true
            }
            Err(e) => {
                log::info!("{runtime_id}'s check did not answer: {e}");
                false
            }
        }
    }

    fn models_reported(&self, runtime_id: &str, models: Vec<plenipo_runtime::agent::KnownModel>) {
        let Some(reported) = self.info(runtime_id).and_then(|i| i.reported_models) else {
            return;
        };
        let names = |list: &[plenipo_runtime::agent::KnownModel]| {
            list.iter().map(|m| m.name.clone()).collect::<Vec<_>>()
        };
        let before = self
            .stored()
            .tools
            .get(runtime_id)
            .and_then(|t| t.models.clone());
        if let Some(before) = &before {
            let (old, new) = (names(&before.models), names(&models));
            let added: Vec<&String> = new.iter().filter(|n| !old.contains(n)).collect();
            let removed: Vec<&String> = old.iter().filter(|n| !new.contains(n)).collect();
            if !added.is_empty() || !removed.is_empty() {
                self.record(
                    PLENIPO,
                    "ai_tool.models_changed",
                    json!({ "runtime": runtime_id, "added": added, "removed": removed }),
                );
            }
        }
        self.keep_tool(runtime_id, |t| t.models = Some(reported));
    }

    /// An AI tool reported how much of the plan is used during a task (Claude Code's
    /// `rate_limit_event`, ADR-060 §3).
    pub fn plan_reported(&self, runtime_id: &str, report: PlanReport) {
        if self.agents().adapter_for(runtime_id).is_none() {
            return;
        }
        self.keep_tool(runtime_id, |t| t.plan = Some(report));
    }

    /// A task on `runtime_id` ended: a tool that reports its plan through its check (Codex's
    /// app server) is asked again, at most every five minutes (ADR-060 §3).
    pub fn task_ended(&self, runtime_id: &str) {
        let Some(adapter) = self.agents().adapter_for(runtime_id) else {
            return;
        };
        let checks_plan = adapter.reports_plan_left()
            && !adapter.status_check_leaves_a_trace()
            && !matches!(
                adapter.status_check(std::path::Path::new(".")),
                StatusCheck::None
            );
        if !checks_plan {
            return;
        }
        {
            let mut live = lock(&self.inner.live);
            let last = live.plan_checked.get(runtime_id).copied().unwrap_or(0);
            if now().saturating_sub(last) < PLAN_CHECK_EVERY_MS {
                return;
            }
            live.plan_checked.insert(runtime_id.to_owned(), now());
        }
        let Some(running) = self.start_check(runtime_id) else {
            return;
        };
        let this = self.clone();
        let id = runtime_id.to_owned();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                this.status_check(&id).await;
                drop(running);
            });
        }
    }

    // ---- Usage (ADR-060 §1) ------------------------------------------------------------------------

    /// `runtime_id`'s usage for the days whose starts are `day_starts` (the last value ends the
    /// last day): tokens read, reused, and written, and tasks, by model, added up from the
    /// counts saved with each step.
    pub fn usage(&self, runtime_id: &str, day_starts: &[u64]) -> Result<AiToolUsage> {
        self.adapter(runtime_id)?;
        if day_starts.len() < 2 || day_starts.len() > MAX_USAGE_DAYS + 1 {
            return Err(BrokerError::Invalid(format!(
                "Usage is shown for 1 to {MAX_USAGE_DAYS} days at a time"
            )));
        }
        if day_starts.windows(2).any(|w| w[0] >= w[1]) {
            return Err(BrokerError::Invalid(
                "The days must follow one another".into(),
            ));
        }
        // A day is at most 25 hours (the day a clock goes back).
        if day_starts.windows(2).any(|w| w[1] - w[0] > 25 * 3_600_000) {
            return Err(BrokerError::Invalid(
                "Each day is at most 25 hours long".into(),
            ));
        }
        let (first, last) = (day_starts[0], day_starts[day_starts.len() - 1]);
        let steps = self.ledger().token_steps(runtime_id, first, last)?;
        let mut days: Vec<UsageDay> = day_starts
            .windows(2)
            .map(|w| UsageDay {
                start: w[0],
                models: Vec::new(),
            })
            .collect();
        // Each task is counted once: on the day, and under the model, of its first step here, so
        // the days and the models add up to the tasks (a task that ran past midnight, or on two
        // models, is not counted twice).
        let mut tasks: HashSet<String> = HashSet::new();
        for step in steps {
            let Some(day) = day_starts
                .windows(2)
                .position(|w| step.started_at >= w[0] && step.started_at < w[1])
            else {
                continue;
            };
            let models = &mut days[day].models;
            let at = match models.iter().position(|m| m.model == step.model) {
                Some(at) => at,
                None => {
                    models.push(UsageModel {
                        model: step.model.clone(),
                        tasks: 0,
                        steps: 0,
                        counted_steps: 0,
                        read: 0,
                        reused: 0,
                        written: 0,
                    });
                    models.len() - 1
                }
            };
            let entry = &mut models[at];
            entry.steps += 1;
            if let Some(task) = &step.task_id {
                if tasks.insert(task.clone()) {
                    entry.tasks += 1;
                }
            } else {
                entry.tasks += 1;
            }
            if step.read.is_some() || step.written.is_some() {
                entry.counted_steps += 1;
                entry.read += step.read.unwrap_or(0);
                entry.reused += step.reused.unwrap_or(0);
                entry.written += step.written.unwrap_or(0);
            }
        }
        Ok(AiToolUsage {
            runtime_id: runtime_id.to_owned(),
            days,
        })
    }

    // ---- How it is paid for (ADR-060 §4) -----------------------------------------------------------

    /// How an AI tool is paid for is fixed by the tool (ADR-085): an AI tool that signs in with
    /// a subscription always uses it, and a paid AI tool always uses its key. Pay-per-use for a
    /// model comes from a paid AI tool (a separate way to reach it), never by switching a
    /// subscription AI tool to a key.
    pub fn set_payment(&self, runtime_id: &str, method: PaymentMethod) -> Result<AiToolsPage> {
        let adapter = self.adapter(runtime_id)?;
        let label = adapter.label();
        match (adapter.paid(), method) {
            (false, PaymentMethod::Subscription) | (true, PaymentMethod::PaidKey) => {
                Ok(self.page())
            }
            (false, PaymentMethod::PaidKey) => Err(BrokerError::Invalid(format!(
                "{label} always uses your subscription. To pay per use, add a key to a paid AI \
                 tool (OpenRouter, or the AI company's own; Settings → AI tools), within your \
                 spending caps."
            ))),
            (true, PaymentMethod::Subscription) => Err(BrokerError::Invalid(format!(
                "{label} is paid per use with your key; it has no subscription."
            ))),
        }
    }

    /// Save the paid key for a paid AI tool (ADR-085): typed only into Plenipo's own screen,
    /// checked with one read call, then kept only in the Vault.
    pub async fn save_paid_key(
        &self,
        runtime_id: &str,
        name: &str,
        key: &str,
    ) -> Result<AiToolsPage> {
        self.adapter(runtime_id)?;
        crate::paid::save_key(&self.inner.broker, self.agents(), runtime_id, name, key)
            .await
            .map_err(BrokerError::Invalid)?;
        Ok(self.page())
    }

    /// Remove a paid AI tool's key: its reference, and the key in the Vault.
    pub async fn remove_paid_key(&self, runtime_id: &str) -> Result<AiToolsPage> {
        self.adapter(runtime_id)?;
        crate::paid::remove_key(&self.inner.broker, self.agents(), runtime_id)
            .await
            .map_err(BrokerError::Invalid)?;
        Ok(self.page())
    }

    // ---- New versions (ADR-059 §2) -----------------------------------------------------------------

    /// The owner's switch: update AI tools by themselves, or ask first (the default).
    pub fn set_auto_update(&self, on: bool) -> Result<AiToolsPage> {
        let _held = lock(&self.inner.keeping);
        self.ledger().update_setting(
            SETTING,
            "ai_tools.auto_update_switched",
            OWNER,
            |current| {
                // What is kept but cannot be read is never written over.
                let mut stored: Stored = if current.is_null() {
                    Stored::default()
                } else {
                    serde_json::from_value(current)
                        .map_err(|e| plenipo_ledger::LedgerError::InvalidInput(e.to_string()))?
                };
                stored.auto_update = on;
                let value = serde_json::to_value(&stored)
                    .map_err(|e| plenipo_ledger::LedgerError::InvalidInput(e.to_string()))?;
                Ok((value, json!({ "on": on })))
            },
        )?;
        drop(_held);
        Ok(self.page())
    }

    /// Look for each AI tool's newest version now; with automatic updates on, update the tools
    /// that have one (each waits until no task uses it).
    pub async fn look_for_new_versions(&self, by: UpdateBy) -> AiToolsPage {
        {
            let mut live = lock(&self.inner.live);
            if live.looking {
                drop(live);
                return self.page();
            }
            live.looking = true;
        }
        let auto = self.stored().auto_update;
        for adapter in plenipo_runtime::agent::builtin_adapters() {
            let id = adapter.id();
            let installed = self.info(id).and_then(|i| {
                i.installation
                    .version
                    .filter(|_| i.installation.state == InstallState::Installed)
            });
            let Some(installed) = installed else {
                continue;
            };
            let newest = match adapter.newest_version() {
                NewestVersion::None => {
                    // Kimi: with automatic updates on, its own `upgrade` checks once a day.
                    if auto && adapter.update_command().is_some() {
                        let _ = self.update(id, UpdateBy::Automatic);
                    }
                    continue;
                }
                NewestVersion::Command(_) => match self.start_check(id) {
                    Some(_running) => self
                        .agents()
                        .newest_by_command(id)
                        .await
                        .and_then(|v| v.ok_or_else(|| "it did not say".to_owned())),
                    // Updating, or its sign-in tab is open: it is asked next time.
                    None => continue,
                },
                NewestVersion::Published(list) => self.published(list).await,
            };
            let checked_at = now();
            let (newest, problem) = match newest {
                Ok(v) => (Some(v), None),
                // Kept and shown: in plain words, with secrets hidden, and short.
                Err(e) => (None, Some(self.last_line(&e, "it did not say"))),
            };
            let mut announce = None;
            let mut by_hand_for = None;
            self.keep_tool(id, |t| {
                by_hand_for = t.by_hand_for.clone();
                t.newest_checked_at = Some(checked_at);
                t.newest_problem = problem.clone();
                if let Some(v) = &newest {
                    t.newest = Some(v.clone());
                    if newer_version(&installed, v) && t.announced.as_deref() != Some(v) {
                        t.announced = Some(v.clone());
                        announce = Some(v.clone());
                    }
                }
            });
            if let Some(v) = &announce {
                self.record(
                    PLENIPO,
                    "ai_tool.update_available",
                    // With the switch on, the update itself says what happened — for a tool
                    // Plenipo can update (Ollama updates from its own tray app).
                    json!({ "runtime": id, "installed": installed, "newest": v,
                            "automatic": auto && adapter.update_command().is_some() }),
                );
            }
            let available = newest
                .as_deref()
                .is_some_and(|v| newer_version(&installed, v));
            // Not again for a version the tool could not update to by itself.
            let update_now = auto
                && available
                && adapter.update_command().is_some()
                && by_hand_for.is_none_or(|v| newest.as_deref() != Some(v.as_str()));
            // Models and plans, once a day with the look (never Kimi's: it leaves a trace). A tool
            // about to update is checked after its update instead.
            if by == UpdateBy::Automatic && !update_now && !adapter.status_check_leaves_a_trace() {
                if let Some(_running) = self.start_check(id) {
                    self.status_check(id).await;
                }
            }
            if update_now {
                let _ = self.update(id, UpdateBy::Automatic);
            }
        }
        self.keep(|s| s.last_looked_at = Some(now()));
        lock(&self.inner.live).looking = false;
        self.page()
    }

    /// The version on a published release list, through Guard's gate (ADR-059 §2).
    async fn published(&self, list: PublishedList) -> std::result::Result<String, String> {
        let address = match &self.inner.release_base {
            // A copy built for the tests: the stand-in serves `/<host><path>`.
            Some(base) => {
                let real = reqwest::Url::parse(&list.address()).map_err(|e| e.to_string())?;
                format!(
                    "{}/{}{}",
                    base.trim_end_matches('/'),
                    real.host_str().unwrap_or_default(),
                    real.path()
                )
            }
            None => list.address(),
        };
        let who = match list {
            PublishedList::Npm(_) => "npm",
            PublishedList::GitHub(_) => "GitHub",
        };
        let body = updates::get_for(
            self.inner.broker.guard(),
            &self.inner.rules,
            Purpose::AiToolVersions,
            who,
            &address,
            Duration::from_secs(30),
            MAX_LIST_BYTES,
        )
        .await
        .map_err(|e| e.to_string())?;
        list.parse(&String::from_utf8_lossy(&body))
            .ok_or_else(|| format!("{who}'s answer did not name a version"))
    }

    /// The daily job (every hour, the look at most once a day): look for new versions, refresh
    /// models and plans, and — with the switch on — update. Whether it looked.
    pub async fn daily(&self) -> bool {
        let last = self.stored().last_looked_at.unwrap_or(0);
        if now().saturating_sub(last) >= LOOK_EVERY_MS {
            self.look_for_new_versions(UpdateBy::Automatic).await;
            return true;
        }
        false
    }

    /// Ask each installed AI tool for its models (and plan), as when Plenipo starts (ADR-060 §5).
    /// Never a tool whose check leaves a trace in its own history (Kimi).
    pub async fn check_models(&self) {
        for adapter in plenipo_runtime::agent::builtin_adapters() {
            let id = adapter.id();
            let installed = self
                .info(id)
                .is_some_and(|i| i.installation.state == InstallState::Installed);
            if installed && !adapter.status_check_leaves_a_trace() {
                if let Some(_running) = self.start_check(id) {
                    self.status_check(id).await;
                }
            }
        }
    }

    // ---- Updating (ADR-059 §3–§6) ------------------------------------------------------------------

    fn set_update(&self, runtime_id: &str, update: AiToolUpdate) {
        let settled = !update.busy();
        lock(&self.inner.live)
            .updates
            .insert(runtime_id.to_owned(), update.clone());
        if settled {
            self.keep_tool(runtime_id, |t| t.last_update = Some(update));
        }
    }

    /// Update `runtime_id` with its own update command, in the background: it waits while a task
    /// uses the tool. Returns the page with the update waiting or running.
    pub fn update(&self, runtime_id: &str, by: UpdateBy) -> Result<AiToolsPage> {
        let adapter = self.adapter(runtime_id)?;
        if adapter.update_command().is_none() {
            return Err(BrokerError::Invalid(format!(
                "{} updates itself: use its own icon in {}.",
                adapter.label(),
                plenipo_core::WORDS.waits_in
            )));
        }
        let installed = self
            .info(runtime_id)
            .is_some_and(|i| i.installation.state == InstallState::Installed);
        if !installed {
            return Err(BrokerError::Invalid(format!(
                "{} is not installed on {}, so there is nothing to update.",
                adapter.label(),
                plenipo_core::WORDS.this_computer
            )));
        }
        let cancel = {
            let mut live = lock(&self.inner.live);
            if live.updates.get(runtime_id).is_some_and(AiToolUpdate::busy) {
                drop(live);
                return Ok(self.page());
            }
            let cancel = Arc::new(AtomicBool::new(false));
            live.cancels
                .insert(runtime_id.to_owned(), Arc::clone(&cancel));
            live.updates.insert(
                runtime_id.to_owned(),
                AiToolUpdate {
                    state: AiToolUpdateState::Waiting,
                    automatic: by == UpdateBy::Automatic,
                    at: Some(now()),
                    ..AiToolUpdate::idle()
                },
            );
            cancel
        };
        let this = self.clone();
        let id = runtime_id.to_owned();
        tokio::spawn(async move {
            this.run_update(&id, by, cancel).await;
        });
        Ok(self.page())
    }

    /// Stop an update that is still waiting for its tool to be free (a running one finishes).
    pub fn cancel_update(&self, runtime_id: &str) -> Result<AiToolsPage> {
        self.adapter(runtime_id)?;
        let mut live = lock(&self.inner.live);
        let waiting = live
            .updates
            .get(runtime_id)
            .is_some_and(|u| u.state == AiToolUpdateState::Waiting);
        if !waiting {
            return Err(BrokerError::Invalid(
                "Only an update that is still waiting can be cancelled".into(),
            ));
        }
        // Stopped at once: its entry goes now, so Update can be pressed again right away.
        if let Some(cancel) = live.cancels.remove(runtime_id) {
            cancel.store(true, Ordering::SeqCst);
        }
        live.updates.remove(runtime_id);
        drop(live);
        Ok(self.page())
    }

    async fn run_update(&self, runtime_id: &str, by: UpdateBy, cancel: Arc<AtomicBool>) {
        let Ok(adapter) = self.adapter(runtime_id) else {
            return;
        };
        let label = adapter.label();
        let automatic = by == UpdateBy::Automatic;
        let base = AiToolUpdate {
            automatic,
            ..AiToolUpdate::idle()
        };
        // Wait until no task is using the tool; then hold it, so none starts meanwhile. Whether
        // it was cancelled is looked at under the same lock that says it runs, so a Cancel is
        // either in time (nothing runs) or refused (it already runs).
        let started = now();
        let hold = loop {
            let free = match self.agents().hold_if_free(runtime_id, HoldFor::Update) {
                // One of Plenipo's own checks is running the tool: let it finish first.
                Ok(hold) if self.checking_now(runtime_id) => {
                    drop(hold);
                    Err(Vec::new())
                }
                Ok(hold) => Ok(hold),
                Err(NotFree::Tasks(tasks)) => Err(tasks),
                // Its sign-in tab is open: wait until it closes.
                Err(NotFree::Held(_)) => Err(Vec::new()),
            };
            let busy_all_day = now().saturating_sub(started) > WAIT_AT_MOST_MS;
            {
                let mut live = lock(&self.inner.live);
                let mine = live
                    .cancels
                    .get(runtime_id)
                    .is_some_and(|c| Arc::ptr_eq(c, &cancel));
                if cancel.load(Ordering::SeqCst) || !mine {
                    // Cancelled: its entry is gone already; a hold just taken is let go.
                    return;
                }
                match &free {
                    Ok(_) => {
                        live.updates.insert(
                            runtime_id.to_owned(),
                            AiToolUpdate {
                                state: AiToolUpdateState::Updating,
                                at: Some(now()),
                                ..base.clone()
                            },
                        );
                    }
                    Err(tasks) if !busy_all_day => {
                        live.updates.insert(
                            runtime_id.to_owned(),
                            AiToolUpdate {
                                state: AiToolUpdateState::Waiting,
                                tasks_using: u32::try_from(tasks.len()).unwrap_or(u32::MAX),
                                at: Some(now()),
                                ..base.clone()
                            },
                        );
                    }
                    Err(_) => {}
                }
            }
            match free {
                Ok(hold) => break hold,
                Err(_) if busy_all_day => {
                    self.set_update(
                        runtime_id,
                        AiToolUpdate {
                            message: Some(format!(
                                "{label} was busy all day, so Plenipo will try again."
                            )),
                            at: Some(now()),
                            ..base.clone()
                        },
                    );
                    return;
                }
                Err(_) => tokio::time::sleep(WAIT_STEP).await,
            }
        };
        if let Err(why) = self
            .inner
            .broker
            .guard()
            .check_ai_tool_action(&AiToolRequest {
                runtime_id,
                label: Some(label),
                action: AiToolAction::Update,
                has_command: true,
                tasks_using: 0,
                busy: None,
            })
        {
            self.set_update(
                runtime_id,
                AiToolUpdate {
                    state: AiToolUpdateState::Failed,
                    message: Some(why),
                    old_still_works: Some(true),
                    at: Some(now()),
                    ..base
                },
            );
            return;
        }
        let from = self.info(runtime_id).and_then(|i| i.installation.version);
        self.set_update(
            runtime_id,
            AiToolUpdate {
                state: AiToolUpdateState::Updating,
                from: from.clone(),
                at: Some(now()),
                ..base.clone()
            },
        );
        let args = adapter.update_command().unwrap_or_default();
        // The maker's own updater runs, and Plenipo cannot check what it downloads: the trail
        // keeps the exact command, so a bad update can be traced (P-DESK-3).
        let command = std::iter::once(adapter.executable_name())
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        self.record(
            if automatic { PLENIPO } else { OWNER },
            "ai_tool.update_started",
            json!({ "runtime": runtime_id, "from": from, "by": by.word(), "command": command }),
        );
        let ran = self
            .run_own(adapter.as_ref(), &format!("Update {label}"), args, true)
            .await;
        // It never started (the program is gone, or not allowed): nothing changed.
        if let Err(why) = &ran {
            self.set_update(
                runtime_id,
                AiToolUpdate {
                    state: AiToolUpdateState::Failed,
                    from: from.clone(),
                    message: Some(why.clone()),
                    old_still_works: None,
                    at: Some(now()),
                    ..base
                },
            );
            drop(hold);
            return;
        }
        let (command_ok, failure) = match &ran {
            Ok(r) if r.succeeded() => (true, None),
            Ok(r) => (
                false,
                Some(self.last_line(&r.output, &r.ending(UPDATE_TIME_LIMIT))),
            ),
            Err(e) => (false, Some(e.clone())),
        };
        self.set_update(
            runtime_id,
            AiToolUpdate {
                state: AiToolUpdateState::Checking,
                from: from.clone(),
                at: Some(now()),
                ..base.clone()
            },
        );
        let answers = self.answers(adapter.as_ref()).await;
        let to = answers.version.clone();
        let done =
            |state: AiToolUpdateState, message: Option<String>, works: Option<bool>| AiToolUpdate {
                state,
                from: from.clone(),
                to: to.clone(),
                tasks_using: 0,
                message,
                old_still_works: works,
                automatic,
                at: Some(now()),
            };
        if answers.ok {
            if self.agents().out_of_service(runtime_id).is_some() {
                self.agents().set_out_of_service(runtime_id, None);
                self.keep_tool(runtime_id, |t| t.out_of_service = None);
                let _ = self.agents().recheck(runtime_id).await;
            }
            if to != from {
                self.record(
                    if automatic { PLENIPO } else { OWNER },
                    "ai_tool.updated",
                    json!({ "runtime": runtime_id, "from": from, "to": to }),
                );
                self.set_update(runtime_id, done(AiToolUpdateState::Updated, None, None));
            } else if !command_ok {
                let reason = failure.unwrap_or_default();
                self.record(
                    PLENIPO,
                    "ai_tool.update_failed",
                    json!({ "runtime": runtime_id, "from": from, "reason": reason,
                            "oldStillWorks": true }),
                );
                self.set_update(
                    runtime_id,
                    done(AiToolUpdateState::Failed, Some(reason), Some(true)),
                );
            } else {
                let newest = self
                    .stored()
                    .tools
                    .get(runtime_id)
                    .and_then(|t| t.newest.clone());
                let behind = match (&to, &newest) {
                    (Some(to), Some(newest)) => newer_version(to, newest),
                    _ => false,
                };
                if behind {
                    let message = adapter.update_by_hand().map(str::to_owned);
                    self.record(
                        if automatic { PLENIPO } else { OWNER },
                        "ai_tool.update_by_hand",
                        json!({ "runtime": runtime_id, "installed": to, "newest": newest,
                                "message": message, "automatic": automatic }),
                    );
                    self.keep_tool(runtime_id, |t| t.by_hand_for = newest.clone());
                    self.set_update(runtime_id, done(AiToolUpdateState::ByHand, message, None));
                } else {
                    self.set_update(runtime_id, done(AiToolUpdateState::UpToDate, None, None));
                }
            }
            drop(hold);
            return;
        }
        // It does not answer the way Plenipo reads it: put the old version back with the tool's
        // own command, where it has one.
        if let Some(old) = from.as_deref().and_then(plain_version) {
            if let Some(args) = adapter.put_back_command(old) {
                let allowed = self
                    .inner
                    .broker
                    .guard()
                    .check_ai_tool_action(&AiToolRequest {
                        runtime_id,
                        label: Some(label),
                        action: AiToolAction::PutBack,
                        has_command: true,
                        tasks_using: 0,
                        busy: None,
                    })
                    .is_ok();
                if allowed
                    && self
                        .run_own(
                            adapter.as_ref(),
                            &format!("Put back {label} {old}"),
                            args,
                            false,
                        )
                        .await
                        .is_ok_and(|r| r.succeeded())
                {
                    let again = self.answers(adapter.as_ref()).await;
                    if again.ok {
                        let reason = format!(
                            "The new version did not answer the way Plenipo reads it, so \
                             Plenipo put back {old}."
                        );
                        self.record(
                            PLENIPO,
                            "ai_tool.put_back",
                            json!({ "runtime": runtime_id, "version": again.version }),
                        );
                        self.record(
                            PLENIPO,
                            "ai_tool.update_failed",
                            json!({ "runtime": runtime_id, "from": from, "reason": reason,
                                    "oldStillWorks": true }),
                        );
                        self.set_update(
                            runtime_id,
                            AiToolUpdate {
                                to: again.version,
                                ..done(AiToolUpdateState::Failed, Some(reason), Some(true))
                            },
                        );
                        drop(hold);
                        return;
                    }
                }
            }
        }
        // No way back: the tool gets no tasks until it answers again (ADR-059 §6).
        let reason = format!(
            "After its update, {label} does not answer the way Plenipo reads it. Install it \
             again: {}",
            adapter.install_hint()
        );
        self.agents()
            .set_out_of_service(runtime_id, Some(reason.clone()));
        self.keep_tool(runtime_id, |t| t.out_of_service = Some(reason.clone()));
        let _ = self.agents().recheck(runtime_id).await;
        self.record(
            PLENIPO,
            "ai_tool.update_failed",
            json!({ "runtime": runtime_id, "from": from, "reason": reason,
                    "oldStillWorks": false }),
        );
        self.set_update(
            runtime_id,
            done(AiToolUpdateState::Failed, Some(reason), Some(false)),
        );
        drop(hold);
    }

    /// Run one of the tool's own commands as an approved program, through the supervisor
    /// (ADR-005): no shell, standard input closed, its tasks' environment. Shown in Runs.
    async fn run_own(
        &self,
        adapter: &dyn RuntimeAdapter,
        label: &str,
        args: Vec<String>,
        update: bool,
    ) -> std::result::Result<programs::Ran, String> {
        let program = self.agents().tool_program(adapter.id())?;
        let mut env = program.env.clone();
        if update {
            env.extend(adapter.update_env());
        }
        programs::run(
            self.inner.broker.supervisor(),
            Run {
                label: label.to_owned(),
                executable: program.executable,
                args,
                working_dir: &program.dir,
                env,
                stdin: None,
                timeout: UPDATE_TIME_LIMIT,
            },
            |_| {},
        )
        .await
    }

    /// A program's last words — its last line that says anything, usually the error — with
    /// secrets hidden and at most 200 characters (what is kept of its output).
    fn last_line(&self, output: &str, otherwise: &str) -> String {
        let line = output
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or(otherwise);
        let filtered = (self.inner.broker.text_filter())(line);
        filtered.chars().take(200).collect()
    }

    /// Wait until `runtime_id`'s update settles (tests).
    pub async fn settled(&self, runtime_id: &str, within: Duration) -> Option<AiToolUpdate> {
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let update = lock(&self.inner.live).updates.get(runtime_id).cloned();
            if let Some(u) = &update {
                if !u.busy() {
                    return update;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// Whether a tool answered the way Plenipo reads it, and its version.
#[derive(Debug, Default)]
struct Answers {
    ok: bool,
    version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_by_their_numbers_and_pre_releases_are_never_offered() {
        assert!(newer_version("1.0.41", "1.0.43"));
        assert!(newer_version("0.34.4", "0.34.10"));
        assert!(newer_version("2.1.283", "2.2.0"));
        assert!(!newer_version("1.0.43", "1.0.43"));
        assert!(!newer_version("1.0.43", "1.0.41"));
        assert!(!newer_version("1.0.41", "1.0.42-alpha"));
        assert!(!newer_version("1.0.41", "latest"));
        assert!(newer_version("v0.34.4", "0.35.0"));
        assert_eq!(plain_version("2.1.283"), Some("2.1.283"));
        assert_eq!(plain_version("0.34"), Some("0.34"));
        assert_eq!(plain_version("2.1.283; rm -rf /"), None);
        assert_eq!(plain_version("--help"), None);
        // Exactly numbers and dots: nothing that merely starts like a version.
        for odd in [
            "2.1.283;x",
            "1.0=--y",
            "1.2.3/../..",
            "1.2--help",
            "1",
            "1..2",
            "v1.2.3",
        ] {
            assert_eq!(plain_version(odd), None, "{odd}");
        }
    }
}
