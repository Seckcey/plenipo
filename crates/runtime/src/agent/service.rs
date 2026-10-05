//! The agent session service: detects runtimes, runs turns as supervised executions,
//! normalizes their output, and records sessions and turns through a [`SessionStore`].
//!
//! Flow of one turn (ADR-007):
//!
//! 1. validate input; reserve the session (one active turn per session, a global cap);
//! 2. preflight: locate the CLI, allow it, check sign-in (refuse if not usable);
//! 3. record the turn's task; launch the adapter-built spec with the prompt on stdin;
//! 4. parse every output line into normalized events → live updates + durable activity;
//! 5. when the process ends, normalize the result. A [`TurnHook`] may keep the turn open —
//!    waiting, for example for handoff replies (ADR-008); otherwise the result is recorded and
//!    the session released.
//!
//! A waiting turn keeps its session reserved until it continues with a new step
//! ([`AgentRuntime::continue_turn`], same provider session) or is cancelled.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::agent::adapter::{
    cap, first_line, talk_answer, FileRequest, NewestVersion, ProcessEnd, ProviderSession,
    RuntimeAdapter, StatusCheck, TurnParser, TurnRequest, MAX_EVENT_TEXT,
};
use crate::agent::brief::{
    note_in_full, text_hash, BriefInput, Conversation, Delivery, Standing, StepMessage,
    NOTE_REMINDER,
};
use crate::agent::discovery::{
    locate, run_probe, run_probe_with, run_talk, runtime_env, HostEnv, Located,
    MAX_PAID_CHECK_OUTPUT, MAX_PROBE_OUTPUT,
};
use crate::agent::dto::*;
use crate::agent::live_text::{ends_blocks, LiveKind, LiveText};
use crate::agent::paid::{PaidBill, PaidCharge, PaidGate, PaidKey, PaidLimits};
use crate::agent::tools::{with_note, FileAnswer, StepInfo, StepTools, TextFilter, ToolProvider};
use crate::dto::{AgentAttribution, BriefKind, NoteKind, OutputLine, OutputStream, PromptSize};
use crate::error::RuntimeError;
use crate::pricing::Price;
use crate::profile::{LaunchSpec, StdinFeed};
use crate::supervisor::Supervisor;

/// Longest objective accepted (characters).
pub const MAX_OBJECTIVE_CHARS: usize = 10_000;
/// Longest prompt sent to a runtime (bytes): an objective plus instructions added by Core.
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;
/// Activity numbering per step: step `n` numbers its activity from `(n - 1) * STEP_SEQ + 1`.
pub const STEP_SEQ: u64 = 1_000_000;

/// How many of a conversation's latest turns get their kept activity back when Plenipo no
/// longer holds their live pieces (ADR-203 §10).
pub const RECALLED_TURNS: usize = 10;

/// What was kept of `turns` (each finished), as live activity: numbered by step the way live
/// pieces are, step `n` from `(n - 1) * STEP_SEQ + 1`, and in order. A turn whose record cannot be
/// read is left out; the chat still shows what was asked and the answer.
fn recall(store: &dyn SessionStore, session_id: &str, turns: &[AgentTurn]) -> Vec<AgentActivity> {
    let mut out = Vec::new();
    for turn in turns {
        let Ok(kept) = store.kept_activity(turn) else {
            continue;
        };
        let mut counts: HashMap<u32, u64> = HashMap::new();
        for piece in kept {
            let step = piece.step.max(1);
            let count = counts.entry(step).or_insert(0);
            *count += 1;
            out.push(AgentActivity {
                session_id: session_id.to_owned(),
                task_id: turn.task_id.clone(),
                seq: u64::from(step - 1) * STEP_SEQ + *count,
                ts: piece.ts,
                event: piece.event,
            });
        }
    }
    out
}
/// Actor recorded for turns the person using the app asked for.
pub const OWNER: &str = "owner";
/// How long a task that talks (ADR-015) gets to stop itself when cancelled.
const CANCEL_GRACE: Duration = Duration::from_secs(5);
/// How long a stopped turn gets to end by itself once its process was ended (the supervisor has
/// already waited for the process and its output); after that the runtime ends it
/// ([`AgentRuntime::force_end`]).
const HARD_END_WAIT: Duration = Duration::from_secs(5);
/// How long [`AgentRuntime::force_end`] waits, at most, for the hook to hear of the step's end:
/// a hook stuck as the step's own finisher was never holds up the stop.
const FORCED_HOOK_WAIT: Duration = Duration::from_secs(3);
/// How long an AI tool may say nothing before the owner is told ([`AgentConfig::stall_note`]).
pub const STALL_NOTE: Duration = Duration::from_secs(3 * 60);
/// How long an AI tool may say nothing, with no tool call or file of its own open, before
/// Plenipo stops the step as no longer answering ([`AgentConfig::stall_end`]).
pub const STALL_END: Duration = Duration::from_secs(10 * 60);
/// The event kinds a stalled or force-ended step's diagnostics list, newest first.
const DIAGNOSTIC_EVENTS: usize = 10;
/// How long a task waits, at most, while its AI tool is held for a sign-in or an update
/// (ADR-058 §5, ADR-059 §4); after that it goes on.
pub const HOLD_WAIT: Duration = Duration::from_secs(10 * 60);
/// How long a new turn waits, at most, for the session's last turn to be let go once it has
/// ended and is only being recorded ([`AgentConfig::release_wait`]).
pub const RELEASE_WAIT: Duration = Duration::from_secs(5);

/// Where sessions and turns are recorded. The desktop app backs this with the Ledger.
/// Calls may block (they run on a blocking thread).
pub trait SessionStore: Send + Sync + 'static {
    /// Sessions, newest first.
    fn sessions(&self, limit: usize) -> Result<Vec<AgentSession>, String>;
    fn session(&self, id: &str) -> Result<Option<AgentSession>, String>;
    /// Record a new session (`session.opened`).
    fn open_session(&self, session: &AgentSession) -> Result<(), String>;
    /// Persist changed session fields.
    fn save_session(&self, session: &AgentSession, change: SessionChange) -> Result<(), String>;
    /// Record the turn's task as running — a new task, or an adopted one — and return its ID.
    fn begin_turn(
        &self,
        session: &AgentSession,
        number: u32,
        input: &TurnInput,
    ) -> Result<String, String>;
    /// A waiting turn continues with its next step: record its task as running again.
    fn begin_step(&self, turn: &TurnRef<'_>, note: &StepNote) -> Result<(), String>;
    /// Record durable activity. `actor` identifies the runtime (e.g. `agent:claude-code`).
    fn record_activity(&self, turn: &TurnRef<'_>, event: &AgentEvent) -> Result<(), String>;
    /// Record the normalized result and finish the task.
    fn finish_turn(&self, turn: &TurnRef<'_>, result: &TurnResult) -> Result<(), String>;
    /// A session's turns, oldest first, with their finished steps.
    fn turns(&self, session_id: &str) -> Result<Vec<AgentTurn>, String>;
    /// Turns not finished — running, waiting, or never started (left behind when Plenipo
    /// stopped).
    fn unfinished_turns(&self) -> Result<Vec<AgentTurn>, String>;
    /// A turn's activity as it was kept, oldest first (ADR-203 §10): its messages, tool calls,
    /// results, and notes, each with its step and time. What a chat shows of a turn whose live
    /// pieces Plenipo no longer holds, after a restart. None for a store that keeps none.
    fn kept_activity(&self, _turn: &AgentTurn) -> Result<Vec<KeptActivity>, String> {
        Ok(Vec::new())
    }
}

/// One piece of a turn's activity as it was kept (ADR-203 §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptActivity {
    /// The step it came in (1 for the objective).
    pub step: u32,
    /// When, in milliseconds since 1970.
    pub ts: u64,
    pub event: AgentEvent,
}

/// Identifies a turn when recording it.
#[derive(Debug, Clone, Copy)]
pub struct TurnRef<'a> {
    pub session_id: &'a str,
    pub task_id: &'a str,
    pub execution_id: Option<&'a str>,
    /// The step concerned; `None` for a result that ends the turn without a step of its own
    /// (cancelled while waiting, interrupted, could not continue).
    pub step: Option<u32>,
    /// Who is recording: `agent:<runtime>` for live turns, `plenipo` for recovery.
    pub actor: &'a str,
}

/// What changed in a saved session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionChange {
    /// The provider confirmed (or changed) its session ID or model.
    Bound,
    Closed,
    /// Timestamps/counters only.
    Touched,
}

/// What a turn works on.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnTask {
    /// Record a new task. `metadata` (a JSON object, or null) is added to the turn's own;
    /// `project_id` makes the task part of a project (an organization's work, ADR-009).
    New {
        requested_by: String,
        metadata: serde_json::Value,
        project_id: Option<String>,
    },
    /// Run a task recorded earlier (for example a handoff's child task) as this turn.
    Existing { task_id: String },
}

/// One turn's input.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnInput {
    /// Recorded as the turn's objective and shown to the owner.
    pub objective: String,
    /// Sent to the runtime on stdin; the objective itself when `None`. Lets Core add
    /// instructions without changing what is recorded. Counted as Plenipo's own text.
    pub prompt: Option<String>,
    /// Liaison's message (ADR-044): its full and short forms, of which the runtime sends one.
    /// Takes the place of `prompt`.
    pub brief: Option<BriefInput>,
    pub task: TurnTask,
}

impl TurnInput {
    /// An objective the owner gives directly.
    pub fn owner(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            prompt: None,
            brief: None,
            task: TurnTask::New {
                requested_by: OWNER.into(),
                metadata: serde_json::Value::Null,
                project_id: None,
            },
        }
    }
}

/// Settings for a new session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionStart {
    /// Plenipo session ID chosen by the caller (a UUID); generated when `None`.
    pub id: Option<String>,
    pub runtime_id: String,
    pub model: Option<String>,
    /// One of the runtime's effort levels; `None` for its default.
    pub effort: Option<Effort>,
    /// Defaults to the objective's first line.
    pub title: Option<String>,
    /// Stored with the session (a JSON object, or null); opaque to the runtime.
    pub metadata: serde_json::Value,
}

/// Why a waiting turn continues; recorded with its next step.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepNote {
    /// Reason recorded with the task's return to `running`.
    pub reason: String,
    /// Extra data for the store (opaque to the runtime).
    pub data: serde_json::Value,
    /// Bytes of the step's prompt Plenipo only passes along (the replies' text), for the step's
    /// recorded size (ADR-044).
    pub passed_bytes: usize,
}

/// A turn step's process ended (passed to the [`TurnHook`]).
#[derive(Debug, Clone)]
pub struct TurnEnd {
    pub session: AgentSession,
    pub task_id: String,
    pub step: u32,
    pub execution_id: Option<String>,
    pub result: TurnResult,
    /// The conversation's mark ([`AgentRuntime::memory_mark`]) when its AI tool kept all of it
    /// through this step — so what the step was sent is still in it (ADR-044); `None` when the
    /// AI tool shortened its memory meanwhile, or the conversation changed.
    pub memory_mark: Option<u64>,
}

/// What happens to a turn whose step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnDisposition {
    /// Record the result and finish the turn (the default).
    Finish,
    /// The hook durably recorded the step's result and moved the task to a waiting state; the
    /// session stays reserved for it until [`AgentRuntime::continue_turn`] or a cancel.
    Suspended { reason: String },
}

/// Decides what happens when a turn step ends (Phase 4: Plenipo Liaison, ADR-008).
pub trait TurnHook: Send + Sync + 'static {
    /// Called on a blocking thread before anything about the ended step is recorded.
    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition;
    /// A turn finished or was suspended, or a waiting turn was cancelled: a session or worker
    /// slot may have become free.
    fn released(&self) {}
}

/// Receives updates for the UI. Must not block for long.
pub trait AgentSink: Send + Sync + 'static {
    fn emit(&self, update: AgentUpdate);
}

/// The longest a paid AI tool's key check may take: the helper's two requests (the key's
/// details and the price list) of up to 30 seconds each, and starting it (ADR-086).
const PAID_CHECK_TIMEOUT: Duration = Duration::from_secs(75);

#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Parent of each session's working directory.
    pub workspace_root: PathBuf,
    pub turn_timeout: Duration,
    pub probe_timeout: Duration,
    pub max_active_turns: usize,
    /// Longest stdout line parsed (provider events can be large).
    pub max_line_bytes: usize,
    /// Live activity kept in memory per turn (to rebuild the UI after a reload).
    pub activity_per_turn: usize,
    /// Turns whose live activity is kept in memory.
    pub activity_turns: usize,
    /// Activity events recorded in the Ledger per step.
    pub stored_activity_per_turn: u32,
    /// Extra fixed variables for every runtime process (tests and diagnostics only; never
    /// credentials).
    pub extra_env: Vec<(String, String)>,
    /// Plenipo's bridge for AI tools reached through a service on this PC (ADR-017): the
    /// program and its first arguments. Without it, such AI tools are shown as not usable.
    pub bridge: Option<Bridge>,
    /// Where AI tools with a home folder of their own keep it, one folder per tool
    /// ([`RuntimeAdapter::own_home`], ADR-082). Next to the workspaces by default.
    pub tool_homes: PathBuf,
    /// How long a task waits, at most, while its AI tool is held ([`HOLD_WAIT`]).
    pub hold_wait: Duration,
    /// How long a new turn waits, at most, for the session's ended turn to be let go
    /// ([`RELEASE_WAIT`]).
    pub release_wait: Duration,
    /// How long a running step may say nothing before the owner is told ([`STALL_NOTE`]). With
    /// a tool call or a file request open, [`AgentConfig::stall_end`] instead.
    pub stall_note: Duration,
    /// How long a running step may say nothing, with no tool call or file request open, before
    /// it is stopped as no longer answering ([`STALL_END`]). With one open, only the turn's own
    /// time limit ends it. An AI tool that never closes a tool call it opened (a `ToolUse` that
    /// gets no result) keeps one open, so there too only the turn's limit ends the stall.
    pub stall_end: Duration,
}

/// A program Plenipo runs in place of an AI tool's own (for the sign-in check and tasks),
/// usually Plenipo itself in a helper mode, e.g. `plenipo-desktop --plenipo-ollama`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bridge {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

impl AgentConfig {
    pub fn new(workspace_root: PathBuf) -> Self {
        let tool_homes = workspace_root.with_file_name("ai-tool-homes");
        Self {
            workspace_root,
            tool_homes,
            turn_timeout: Duration::from_secs(30 * 60),
            probe_timeout: Duration::from_secs(20),
            max_active_turns: 4,
            max_line_bytes: 4 * 1024 * 1024,
            activity_per_turn: 500,
            activity_turns: 50,
            stored_activity_per_turn: 500,
            extra_env: Vec::new(),
            bridge: None,
            hold_wait: HOLD_WAIT,
            release_wait: RELEASE_WAIT,
            stall_note: STALL_NOTE,
            stall_end: STALL_END,
        }
    }
}

/// Why a session's slot is claimed. Claims are exclusive, so a close, a new turn, and a
/// continuation can never interleave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    /// A step is starting or running.
    Turn,
    /// A turn is waiting to continue; no process runs.
    Wait,
    /// `close_session` holds the slot.
    Close,
}

struct Active {
    claim: Claim,
    /// The AI tool a step runs on, once it passed its check ([`AgentRuntime::tasks_using`]).
    runtime_id: Option<String>,
    task_id: Option<String>,
    execution_id: Option<String>,
    /// The step running (or, while waiting, the last one run).
    step: u32,
    step_started_at: u64,
    done: watch::Receiver<bool>,
    /// Asks a task that talks (ADR-015) to stop itself before its process is ended.
    interrupt: Option<mpsc::UnboundedSender<()>>,
    /// The step waits for its AI tool, held by an update or a sign-in tab ([`HoldFor`]).
    waiting_for_hold: bool,
    /// The owner stopped it while it waited or started: it ends as cancelled, before it runs.
    stop_waiting: bool,
    /// The turn has ended (or a stop ended its wait) and is only being recorded: nothing runs
    /// under it any more, and it is let go once recorded. A new turn waits for that
    /// ([`AgentRuntime::reserve_turn`]); a continuation is still refused meanwhile.
    releasing: bool,
    /// The running step's grant of Plenipo's tools, closed by [`AgentRuntime::force_end`].
    grant: Option<String>,
    /// The running step's conversation mark at launch and what it sent (ADR-044), recorded as
    /// not delivered by [`AgentRuntime::force_end`] when it ends the step.
    sent: Option<(u64, Delivery)>,
    /// Set when [`AgentRuntime::force_end`] ended the running step: its own finisher, if it ever
    /// comes, then records and announces nothing.
    forced: Option<Arc<AtomicBool>>,
    /// The running step's watch, for the diagnostics of a step the runtime ends itself.
    watch: Option<Arc<Mutex<StepWatch>>>,
}

/// What Stop all did ([`AgentRuntime::stop_all_turns_report`]): how many turns it stopped, and
/// the sessions it could not stop, with why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopAllReport {
    pub stopped: usize,
    pub not_stopped: Vec<(String, String)>,
}

/// Why [`AgentRuntime::try_reserve`] could not claim a session: the error, and whether the
/// claim in the way is only being recorded and let go ([`Active::releasing`]), read under the
/// same lock as the refusal.
struct Blocked {
    error: RuntimeError,
    releasing: bool,
}

#[derive(Default)]
struct State {
    runtimes: Vec<AgentRuntimeInfo>,
    /// Keyed by session ID: turns starting, running, or waiting.
    active: HashMap<String, Active>,
    activity: HashMap<String, VecDeque<AgentActivity>>,
    /// Task IDs with buffered activity, oldest first.
    activity_order: VecDeque<String>,
    notices: Vec<String>,
    consumers: Vec<JoinHandle<()>>,
    shutting_down: bool,
    /// What Plenipo sent each conversation and what its AI tool reported, by session ID
    /// (ADR-044 §5). In memory only: after Plenipo starts again, every conversation starts over.
    conversations: HashMap<String, Conversation>,
    /// The last conversation mark given out ([`Conversation::mark`]).
    marks: u64,
    /// AI tools no new task may start on for now, and how many holds of each kind each has
    /// (ADR-058 §5, ADR-059 §4).
    holds: HashMap<String, Holds>,
    /// AI tools Plenipo gives no tasks, and why: an update left one that does not answer the way
    /// Plenipo reads it (ADR-059 §6).
    out_of_service: HashMap<String, String>,
    /// The owner pressed Stop all work (Phase 25, item 3.4): no new turn starts on any AI tool
    /// until Allow again. In memory only: after a restart nothing resumes anyway.
    work_held: bool,
}

impl State {
    fn next_mark(&mut self) -> u64 {
        self.marks += 1;
        self.marks
    }
}

struct Inner {
    config: AgentConfig,
    adapters: Vec<Arc<dyn RuntimeAdapter>>,
    supervisor: Supervisor,
    store: Arc<dyn SessionStore>,
    sink: Arc<dyn AgentSink>,
    hook: RwLock<Option<Arc<dyn TurnHook>>>,
    /// Plenipo's tools for turn steps (Phase 7).
    tools: RwLock<Option<Arc<dyn ToolProvider>>>,
    /// Hides secrets in activity and results (Phase 7).
    filter: RwLock<Option<TextFilter>>,
    /// Keys and spending caps for paid AI tools (Phase 16 Wave 3, ADR-085).
    paid: RwLock<Option<Arc<dyn PaidGate>>>,
    host: HostEnv,
    state: Mutex<State>,
    /// Woken when a hold on an AI tool ends.
    holds_changed: tokio::sync::Notify,
    /// Woken when a session's claim is let go ([`AgentRuntime::reserve_turn`]).
    turn_released: tokio::sync::Notify,
    /// The latest plan each AI tool reported, shared by every organization on the PC (Phase 25,
    /// item 4.3).
    plans: RwLock<Arc<PlanBook>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct AgentRuntime {
    inner: Arc<Inner>,
}

/// A usable runtime, checked just before a turn.
struct Ready {
    executable: PathBuf,
    /// Arguments before the adapter's own (the bridge's, for bridged AI tools).
    args_prefix: Vec<String>,
    env: Vec<(String, String)>,
    /// The sign-in check confirmed a subscription.
    billing_confirmed: bool,
    /// A paid AI tool's key (ADR-085), for the helper's first line of input.
    paid_key: Option<PaidKey>,
    /// The version the check read (ADR-200).
    cli_version: Option<String>,
}

/// A runtime that cannot take work, with the outcome to record when a waiting turn cannot
/// continue because of it.
struct NotReady {
    error: RuntimeError,
    outcome: TurnOutcome,
}

/// A receiver that reports "done" at once (for claims no step is running under).
fn finished() -> watch::Receiver<bool> {
    watch::channel(true).1
}

impl AgentRuntime {
    /// Create the service. Turns left running by a previous Plenipo session are recorded as
    /// interrupted. Runtimes start in the `checking` state; call [`AgentRuntime::refresh`].
    pub fn new(
        config: AgentConfig,
        adapters: Vec<Arc<dyn RuntimeAdapter>>,
        supervisor: Supervisor,
        store: Arc<dyn SessionStore>,
        sink: Arc<dyn AgentSink>,
        host: HostEnv,
    ) -> Self {
        let runtimes = adapters.iter().map(|a| checking(a.as_ref())).collect();
        let this = Self {
            inner: Arc::new(Inner {
                config,
                adapters,
                supervisor,
                store,
                sink,
                hook: RwLock::new(None),
                tools: RwLock::new(None),
                filter: RwLock::new(None),
                paid: RwLock::new(None),
                host,
                state: Mutex::new(State {
                    runtimes,
                    ..State::default()
                }),
                holds_changed: tokio::sync::Notify::new(),
                turn_released: tokio::sync::Notify::new(),
                plans: RwLock::new(Arc::default()),
            }),
        };
        this.recover();
        this
    }

    /// Install the hook consulted when a turn step ends (at most one; replaces any earlier).
    pub fn set_hook(&self, hook: Arc<dyn TurnHook>) {
        *self.inner.hook.write().unwrap_or_else(|p| p.into_inner()) = Some(hook);
    }

    /// Install the provider of Plenipo's tools for turn steps (at most one).
    pub fn set_tools(&self, provider: Arc<dyn ToolProvider>) {
        *self.inner.tools.write().unwrap_or_else(|p| p.into_inner()) = Some(provider);
    }

    /// Install the filter that hides secrets in activity and results before they are shown,
    /// recorded, or passed on.
    pub fn set_filter(&self, filter: TextFilter) {
        *self.inner.filter.write().unwrap_or_else(|p| p.into_inner()) = Some(filter);
    }

    /// Install the gate for paid AI tools: their keys, and the spending caps (ADR-085). Without
    /// it, no paid AI tool is ready.
    pub fn set_paid_gate(&self, gate: Arc<dyn PaidGate>) {
        *self.inner.paid.write().unwrap_or_else(|p| p.into_inner()) = Some(gate);
    }

    fn paid_gate(&self) -> Option<Arc<dyn PaidGate>> {
        self.inner
            .paid
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn filter(&self) -> Option<TextFilter> {
        self.inner
            .filter
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn tool_provider(&self) -> Option<Arc<dyn ToolProvider>> {
        self.inner
            .tools
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The step's tools, if the provider gives it any.
    async fn open_tools(
        &self,
        session: &AgentSession,
        task_id: &str,
        step: u32,
        ai_tool: &'static str,
    ) -> Option<StepTools> {
        let provider = self.tool_provider()?;
        let (session, task_id) = (session.clone(), task_id.to_owned());
        tokio::task::spawn_blocking(move || {
            provider.open(&StepInfo {
                session: &session,
                task_id: &task_id,
                step,
                ai_tool,
                takes_tools: true,
            })
        })
        .await
        .ok()
        .flatten()
    }

    /// What a step without tools should know about that, if anything.
    async fn note_without_tools(
        &self,
        session: &AgentSession,
        task_id: &str,
        step: u32,
        ai_tool: &'static str,
        takes_tools: bool,
    ) -> Option<String> {
        let provider = self.tool_provider()?;
        let (session, task_id) = (session.clone(), task_id.to_owned());
        tokio::task::spawn_blocking(move || {
            provider.note_without_tools(&StepInfo {
                session: &session,
                task_id: &task_id,
                step,
                ai_tool,
                takes_tools,
            })
        })
        .await
        .ok()
        .flatten()
    }

    /// End a step's grant (before its result is recorded).
    async fn close_tools(&self, grant_id: String) {
        if let Some(provider) = self.tool_provider() {
            let _ = tokio::task::spawn_blocking(move || provider.close(&grant_id)).await;
        }
    }

    fn hook(&self) -> Option<Arc<dyn TurnHook>> {
        self.inner
            .hook
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn released(&self) {
        if let Some(hook) = self.hook() {
            hook.released();
        }
    }

    fn recover(&self) {
        let store = &self.inner.store;
        let unfinished = match store.unfinished_turns() {
            Ok(turns) => turns,
            Err(e) => {
                self.notice(format!("Could not check for interrupted agent turns: {e}"));
                return;
            }
        };
        let count = unfinished.len();
        for turn in unfinished {
            let result = administrative(
                TurnOutcome::Interrupted,
                "Plenipo stopped while this turn was running",
                None,
            );
            let turn_ref = TurnRef {
                session_id: &turn.session_id,
                task_id: &turn.task_id,
                execution_id: turn.execution_id.as_deref(),
                step: None,
                actor: "plenipo",
            };
            if let Err(e) = store.finish_turn(&turn_ref, &result) {
                self.notice(format!("Could not record an interrupted agent turn: {e}"));
            }
        }
        if count > 0 {
            self.notice(format!(
                "{count} agent turn(s) from a previous session were marked interrupted."
            ));
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// `session` with the task IDs of its running or waiting turn (if any).
    fn with_active(&self, session: AgentSession) -> AgentSession {
        let state = self.lock();
        let active = state.active.get(&session.id);
        Self::with_claim(session, active)
    }

    /// `session` with the task its claim (`active`) runs or waits on. With
    /// [`Self::decorated`] under one look at the claim, the summary and the turns agree.
    fn with_claim(mut session: AgentSession, active: Option<&Active>) -> AgentSession {
        session.active_task_id = active
            .filter(|a| a.claim == Claim::Turn)
            .and_then(|a| a.task_id.clone());
        session.waiting_task_id = active
            .filter(|a| a.claim == Claim::Wait)
            .and_then(|a| a.task_id.clone());
        session
    }

    /// `turn` with the live state of its running step or wait, when it holds its session.
    fn decorate(&self, turn: AgentTurn) -> AgentTurn {
        let state = self.lock();
        let active = state.active.get(&turn.session_id);
        Self::decorated(turn, active)
    }

    /// [`Self::decorate`], with the session's claim as one look read it (`active`).
    fn decorated(mut turn: AgentTurn, active: Option<&Active>) -> AgentTurn {
        let Some(active) = active.filter(|a| a.task_id.as_deref() == Some(turn.task_id.as_str()))
        else {
            return turn;
        };
        match active.claim {
            // Its step's result is recorded but the slot not yet released (the turn is finishing
            // or starting to wait): the recorded state is already the current one.
            Claim::Turn
                if turn
                    .steps
                    .iter()
                    .any(|s| s.number == active.step && s.result.is_some()) => {}
            Claim::Turn => {
                turn.running = true;
                turn.waiting = false;
                if let Some(id) = &active.execution_id {
                    turn.execution_id = Some(id.clone());
                }
                match turn.steps.iter_mut().find(|s| s.number == active.step) {
                    Some(step) => {
                        step.running = step.result.is_none();
                        step.execution_id = step
                            .execution_id
                            .take()
                            .or_else(|| active.execution_id.clone());
                    }
                    None => turn.steps.push(TurnStep {
                        number: active.step,
                        execution_id: active.execution_id.clone(),
                        running: true,
                        result: None,
                        started_at: Some(active.step_started_at),
                        ended_at: None,
                    }),
                }
            }
            Claim::Wait => {
                turn.running = false;
                turn.waiting = true;
            }
            Claim::Close => {}
        }
        turn
    }

    fn notice(&self, notice: String) {
        let mut state = self.lock();
        if !state.notices.contains(&notice) {
            state.notices.push(notice);
        }
    }

    fn adapter(&self, runtime_id: &str) -> Option<Arc<dyn RuntimeAdapter>> {
        self.inner
            .adapters
            .iter()
            .find(|a| a.id() == runtime_id)
            .cloned()
    }

    async fn with_store<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn SessionStore) -> Result<T, String> + Send + 'static,
    ) -> Result<T, RuntimeError> {
        let store = Arc::clone(&self.inner.store);
        tokio::task::spawn_blocking(move || f(store.as_ref()))
            .await
            .map_err(|e| RuntimeError::Store(e.to_string()))?
            .map_err(RuntimeError::Store)
    }

    /// Emit the turn as recorded, with its live state.
    async fn emit_turn(&self, session_id: &str, task_id: &str) {
        self.emit_turn_with(session_id, task_id, None).await;
    }

    /// [`AgentRuntime::emit_turn`], for a turn that has ended with `ended`: shown with that result
    /// even when the Ledger could not record it, so the screens never show a turn that ended as
    /// still running ("Writing" forever, the owner's report, 2026-10-05).
    async fn emit_turn_with(&self, session_id: &str, task_id: &str, ended: Option<&TurnResult>) {
        let (s, t) = (session_id.to_owned(), task_id.to_owned());
        match self
            .with_store(move |store| Ok(store.turns(&s)?.into_iter().find(|x| x.task_id == t)))
            .await
        {
            Ok(Some(mut turn)) => {
                if let Some(result) = ended.filter(|_| turn.result.is_none()) {
                    turn.result = Some(result.clone());
                    turn.running = false;
                    turn.waiting = false;
                }
                let turn = self.decorate(turn);
                self.inner.sink.emit(AgentUpdate::Turn(turn));
            }
            Ok(None) => {}
            Err(e) => self.notice(e.to_string()),
        }
    }

    fn emit_session(&self, session: AgentSession) {
        let current = self.with_active(session);
        self.inner.sink.emit(AgentUpdate::Session(current));
    }

    // ---- Detection ----------------------------------------------------------------------

    pub fn runtimes(&self) -> Vec<AgentRuntimeInfo> {
        let state = self.lock();
        state
            .runtimes
            .iter()
            .map(|r| AgentRuntimeInfo {
                held: if state.work_held {
                    Some(HoldFor::StopAll)
                } else {
                    state.holds.get(&r.id).map(|h| h.reason())
                },
                ..r.clone()
            })
            .collect()
    }

    /// Detect installation, version, and sign-in for every runtime (in parallel).
    pub async fn refresh(&self) -> Vec<AgentRuntimeInfo> {
        let handles: Vec<_> = self
            .inner
            .adapters
            .iter()
            .map(|adapter| {
                let this = self.clone();
                let adapter = Arc::clone(adapter);
                tokio::spawn(async move { this.detect(adapter.as_ref(), true).await.0 })
            })
            .collect();
        for handle in handles {
            if let Ok(info) = handle.await {
                self.store_info(info);
            }
        }
        let runtimes = self.runtimes();
        self.inner.sink.emit(AgentUpdate::Runtimes(RuntimesUpdate {
            runtimes: runtimes.clone(),
        }));
        runtimes
    }

    fn store_info(&self, mut info: AgentRuntimeInfo) {
        let mut state = self.lock();
        if let Some(slot) = state.runtimes.iter_mut().find(|r| r.id == info.id) {
            // What the AI tool last reported about its models outlives each check (ADR-060 §5).
            if info.reported_models.is_none() {
                info.reported_models = slot.reported_models.take();
            }
            *slot = info;
        }
    }

    /// Locate, allow, and check one runtime. `version` runs the version probe (otherwise the
    /// last known version is kept when the executable is unchanged).
    async fn detect(
        &self,
        adapter: &dyn RuntimeAdapter,
        version: bool,
    ) -> (AgentRuntimeInfo, Option<Ready>) {
        let mut info = checking(adapter);
        info.checked_at = Some(crate::now_ms());
        let previous = self
            .lock()
            .runtimes
            .iter()
            .find(|r| r.id == adapter.id())
            .cloned();
        let host = &self.inner.host;
        let not_checked = AuthStatus {
            state: AuthState::Unknown,
            method: None,
            detail: Some("Not checked: the AI tool is not installed.".into()),
        };
        // A paid AI tool's program is Plenipo's own helper (ADR-085): nothing to find.
        let located = if adapter.built_in() {
            match self.inner.config.bridge.as_ref() {
                Some(bridge) if bridge.executable.is_file() => {
                    Located::Found(bridge.executable.clone())
                }
                _ => Located::NotFound,
            }
        } else {
            locate(adapter, host)
        };
        let executable = match located {
            Located::Found(path) => path,
            Located::NotFound => {
                info.installation = Installation {
                    state: InstallState::NotInstalled,
                    executable: None,
                    version: None,
                    detail: Some(if adapter.built_in() {
                        format!(
                            "This version of Plenipo cannot reach {} (its helper is not set up).",
                            adapter.label()
                        )
                    } else {
                        format!(
                            "No `{}` executable was found on PATH or in the usual install locations.",
                            adapter.executable_name()
                        )
                    }),
                };
                info.auth = not_checked;
                return (info, None);
            }
            Located::Unsupported(paths) => {
                let shown = paths
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                info.installation = Installation {
                    state: InstallState::Unsupported,
                    executable: Some(shown.clone()),
                    version: None,
                    detail: Some(format!(
                        "Found {shown}, but Plenipo runs only native executables here (not npm \
                         launcher scripts, which pass input through a command shell, or files \
                         without execute permission). Install the native build."
                    )),
                };
                info.auth = not_checked;
                return (info, None);
            }
        };
        let executable = match self.inner.supervisor.allow_executable(&executable) {
            Ok(canonical) => canonical,
            Err(e) => {
                info.installation = Installation {
                    state: InstallState::Broken,
                    executable: Some(executable.display().to_string()),
                    version: None,
                    detail: Some(e.to_string()),
                };
                info.auth = not_checked;
                return (info, None);
            }
        };
        let shown = executable.display().to_string();
        let env = match self.tool_env(adapter) {
            Ok(env) => env,
            Err(detail) => {
                info.installation = Installation {
                    state: InstallState::Broken,
                    executable: Some(shown),
                    version: None,
                    detail: Some(detail),
                };
                info.auth = not_checked;
                return (info, None);
            }
        };
        let workdir = self.probe_dir();
        let timeout = self.inner.config.probe_timeout;

        let known_version = previous
            .filter(|p| p.installation.executable.as_deref() == Some(shown.as_str()))
            .and_then(|p| p.installation.version);
        let version = match (version, known_version) {
            _ if adapter.built_in() => Ok(adapter.checked_version().to_owned()),
            (false, Some(v)) => Ok(v),
            _ => {
                let out = run_probe(
                    &executable,
                    &adapter.version_args(),
                    &env,
                    &workdir,
                    timeout,
                )
                .await;
                match adapter.parse_version(&out) {
                    Some(v) if out.succeeded() => Ok(v),
                    _ => Err(probe_failure("version check", &out)),
                }
            }
        };
        match version {
            Ok(v) => {
                info.installation = Installation {
                    state: InstallState::Installed,
                    executable: Some(shown),
                    version: Some(v),
                    detail: None,
                }
            }
            Err(detail) => {
                info.installation = Installation {
                    state: InstallState::Broken,
                    executable: Some(shown),
                    version: None,
                    detail: Some(detail),
                };
                info.auth = not_checked;
                return (info, None);
            }
        }
        // A bridged AI tool is checked and run through Plenipo's bridge (ADR-017).
        let (executable, args_prefix) = if adapter.bridged() {
            let bridge = self
                .inner
                .config
                .bridge
                .as_ref()
                .ok_or_else(|| {
                    format!(
                        "This version of Plenipo cannot reach {} (its helper is not set up).",
                        adapter.label()
                    )
                })
                .and_then(|b| {
                    self.inner
                        .supervisor
                        .allow_executable(&b.executable)
                        .map(|exe| (exe, adapter.bridge_args().unwrap_or_else(|| b.args.clone())))
                        .map_err(|e| e.to_string())
                });
            match bridge {
                Ok(bridge) => bridge,
                Err(detail) => {
                    info.installation.state = InstallState::Broken;
                    info.installation.detail = Some(detail);
                    info.auth = not_checked;
                    return (info, None);
                }
            }
        } else {
            (executable, Vec::new())
        };
        // A paid AI tool is ready only with the owner's key, while paid keys are switched on
        // (ADR-085); its check gets the key on its first line of input.
        let paid_key = if adapter.paid() {
            let key = match self.paid_gate() {
                Some(gate) => {
                    let id = adapter.id().to_owned();
                    tokio::task::spawn_blocking(move || gate.key(&id))
                        .await
                        .unwrap_or_else(|e| Err(format!("The key could not be read: {e}")))
                }
                None => Err("This version of Plenipo cannot use paid AI keys.".to_owned()),
            };
            match key {
                Ok(key) => Some(key),
                Err(why) => {
                    info.auth = AuthStatus {
                        state: AuthState::SignedOut,
                        method: None,
                        detail: Some(why),
                    };
                    info.ready = false;
                    return (info, None);
                }
            }
        } else {
            None
        };
        // Most AI tools answer a status command; Copilot answers a short talk (ADR-083).
        let out = match adapter.auth_talk() {
            Some(talk) => {
                let args = [args_prefix.clone(), talk.args].concat();
                run_talk(
                    &executable,
                    &args,
                    &env,
                    &workdir,
                    &talk.lines,
                    &talk.answers,
                    talk.framing,
                    timeout,
                )
                .await
            }
            None => {
                let auth_args = [args_prefix.clone(), adapter.auth_args()].concat();
                let input = paid_key.as_ref().map(PaidKey::stdin_line);
                // A paid AI tool's check reads the key's details and the whole price list (two
                // requests the helper gives up to 30 seconds each, ADR-086).
                let (max_output, timeout) = if adapter.paid() {
                    (MAX_PAID_CHECK_OUTPUT, timeout.max(PAID_CHECK_TIMEOUT))
                } else {
                    (MAX_PROBE_OUTPUT, timeout)
                };
                run_probe_with(
                    &executable,
                    &auth_args,
                    &env,
                    &workdir,
                    input.as_deref().map(str::as_bytes),
                    max_output,
                    timeout,
                )
                .await
            }
        };
        info.auth = adapter.parse_auth(&out);
        // A paid AI tool's check lists its models with today's prices (ADR-085, ADR-086), and
        // OpenRouter's its key's own limit (Phase 25, item 4.3).
        if adapter.paid() {
            if let Some(models) = adapter.parse_models(&out) {
                info.reported_models = Some(ReportedModels {
                    models,
                    complete: true,
                    checked_at: crate::now_ms(),
                });
            }
            if let Some(plan) = adapter.parse_plan(&out) {
                self.plan_reported(adapter.id(), plan);
            }
        }
        info.ready = auth_allowed(adapter, info.auth.state);
        if let Some(why) = self.lock().out_of_service.get(adapter.id()) {
            info.ready = false;
            info.installation.detail = Some(why.clone());
        }
        let billing_confirmed = info.auth.state == AuthState::Subscription;
        let cli_version = info.installation.version.clone();
        let ready = info.ready.then_some(Ready {
            executable,
            args_prefix,
            env,
            billing_confirmed,
            paid_key,
            cli_version,
        });
        (info, ready)
    }

    /// Every process of an AI tool gets this environment: its own variables ([`runtime_env`]),
    /// its home folder when it has one of its own ([`Self::own_home`]), then the configured
    /// extras.
    fn tool_env(&self, adapter: &dyn RuntimeAdapter) -> Result<Vec<(String, String)>, String> {
        let mut env = runtime_env(adapter, &self.inner.host);
        // The tool's own variable for its settings folder (ADR-083), or its home folder.
        let var =
            adapter
                .home_variable()
                .unwrap_or(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
        let extra = &self.inner.config.extra_env;
        // A home folder the extras choose (tests) is where the tool's own settings go too.
        let chosen = extra
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(var))
            .map(|(_, v)| PathBuf::from(v));
        if let Some(home) = self.own_home(adapter, chosen)? {
            env.retain(|(k, _)| !k.eq_ignore_ascii_case(var));
            env.push((var.to_owned(), home.display().to_string()));
        }
        env.extend(extra.iter().cloned());
        Ok(env)
    }

    /// The AI tool's own home folder, with its settings files written afresh
    /// ([`RuntimeAdapter::own_home`], ADR-082): `chosen`, or its folder under
    /// [`AgentConfig::tool_homes`]. `None` for a tool that uses the owner's home folder. Each
    /// file is written whole and then moved into place, so a run never reads half of one.
    fn own_home(
        &self,
        adapter: &dyn RuntimeAdapter,
        chosen: Option<PathBuf>,
    ) -> Result<Option<PathBuf>, String> {
        let files = adapter.own_home();
        if files.is_empty() && adapter.home_variable().is_none() {
            return Ok(None);
        }
        let home = chosen.unwrap_or_else(|| self.inner.config.tool_homes.join(adapter.id()));
        let failed = |e: std::io::Error| {
            format!(
                "Plenipo could not prepare {}'s own settings folder ({}): {e}",
                adapter.label(),
                home.display()
            )
        };
        static DRAFTS: AtomicU64 = AtomicU64::new(0);
        // A folder named by the tool's own variable exists even with no file of Plenipo's in it.
        if adapter.home_variable().is_some() {
            std::fs::create_dir_all(&home).map_err(failed)?;
        }
        for (place, contents) in files {
            let file = home.join(place);
            // Already so (the usual case): nothing to replace, so a run that has the file open
            // is never in the way.
            if std::fs::read(&file).is_ok_and(|now| now == contents.as_bytes()) {
                continue;
            }
            let dir = file.parent().unwrap_or(&home);
            std::fs::create_dir_all(dir).map_err(failed)?;
            let draft = dir.join(format!(
                ".plenipo-{}-{}.tmp",
                std::process::id(),
                DRAFTS.fetch_add(1, Ordering::Relaxed)
            ));
            let written = std::fs::write(&draft, contents.as_bytes()).and_then(|()| {
                // On Windows a program reading the file at that moment can keep it from being
                // replaced; it lets go within moments.
                let mut tries = 0;
                loop {
                    match std::fs::rename(&draft, &file) {
                        Err(_) if tries < 4 => {
                            tries += 1;
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        done => break done,
                    }
                }
            });
            if let Err(e) = written {
                let _ = std::fs::remove_file(&draft);
                // Another run put the same settings there first: as good.
                if std::fs::read(&file).is_ok_and(|now| now == contents.as_bytes()) {
                    continue;
                }
                return Err(failed(e));
            }
        }
        Ok(Some(home))
    }

    fn probe_dir(&self) -> PathBuf {
        let dir = &self.inner.config.workspace_root;
        if std::fs::create_dir_all(dir).is_ok() {
            dir.clone()
        } else {
            std::env::temp_dir()
        }
    }

    /// Fresh check right before a turn. Refuses with an explanation when not usable. While the
    /// AI tool is held (a sign-in or an update), the turn waits first ([`Self::hold_if_free`]).
    async fn preflight(
        &self,
        adapter: &dyn RuntimeAdapter,
        session_id: &str,
    ) -> Result<Ready, NotReady> {
        if !self.wait_for_hold(adapter.id(), session_id).await {
            return Err(NotReady {
                error: RuntimeError::NotReady(format!(
                    "Stopped while it waited for {} to be free.",
                    adapter.label()
                )),
                outcome: TurnOutcome::Cancelled,
            });
        }
        let (info, ready) = self.detect(adapter, false).await;
        let changed = self
            .lock()
            .runtimes
            .iter()
            .find(|r| r.id == info.id)
            .is_none_or(|r| r.installation != info.installation || r.auth != info.auth);
        let explanation = not_ready_reason(adapter, &info);
        let outcome = unavailable_outcome(&info);
        self.store_info(info);
        if changed {
            self.inner.sink.emit(AgentUpdate::Runtimes(RuntimesUpdate {
                runtimes: self.runtimes(),
            }));
        }
        ready.ok_or(NotReady {
            error: RuntimeError::NotReady(explanation),
            outcome,
        })
    }

    // ---- The AI tools page (Phase 19, ADR-058 to ADR-060) --------------------------------

    /// The adapter for `runtime_id`, when this build has it.
    pub fn adapter_for(&self, runtime_id: &str) -> Option<Arc<dyn RuntimeAdapter>> {
        self.adapter(runtime_id)
    }

    /// The tasks using `runtime_id` now: a step passed its check and runs, or is about to
    /// (ADR-058 §5, ADR-059 §3). A task waiting between steps does not count.
    pub fn tasks_using(&self, runtime_id: &str) -> Vec<String> {
        using(&self.lock(), runtime_id)
    }

    /// Hold `runtime_id` — no new task starts on it while the hold is kept — if it is free: no
    /// task is using it, and it is not held already (one sign-in tab or update at a time, since
    /// both run the tool's own program). A task that would start meanwhile waits: for a sign-in,
    /// [`AgentConfig::hold_wait`] at most, then it goes on; for an update, until the hold is let
    /// go ([`HoldFor`]).
    pub fn hold_if_free(&self, runtime_id: &str, reason: HoldFor) -> Result<RuntimeHold, NotFree> {
        let mut state = self.lock();
        let tasks = using(&state, runtime_id);
        if !tasks.is_empty() {
            return Err(NotFree::Tasks(tasks));
        }
        if let Some(held) = state.holds.get(runtime_id) {
            return Err(NotFree::Held(held.reason()));
        }
        let holds = state.holds.entry(runtime_id.to_owned()).or_default();
        *holds.count(reason) += 1;
        drop(state);
        self.holds_shown();
        Ok(RuntimeHold {
            runtime: self.clone(),
            runtime_id: runtime_id.to_owned(),
            reason,
        })
    }

    /// Tell the screen which AI tools are held now: a new task on one says it waits.
    fn holds_shown(&self) {
        self.inner.sink.emit(AgentUpdate::Runtimes(RuntimesUpdate {
            runtimes: self.runtimes(),
        }));
    }

    /// Whether `runtime_id` is held now.
    pub fn held(&self, runtime_id: &str) -> bool {
        let state = self.lock();
        state.work_held || state.holds.contains_key(runtime_id)
    }

    // ---- Stop all work (Phase 25, item 3.4; ADR-199) ------------------------------------

    /// Hold all work: no new turn starts on any AI tool — each waits, and can still be stopped —
    /// until [`Self::allow_work`]. Turns already running go on; [`Self::stop_all_turns`] stops
    /// them.
    pub fn hold_all_work(&self) {
        self.lock().work_held = true;
        self.holds_shown();
    }

    /// Allow again after Stop all work: the turns that waited start.
    pub fn allow_work(&self) {
        self.lock().work_held = false;
        self.inner.holds_changed.notify_waiters();
        self.holds_shown();
    }

    /// Whether Stop all work holds the work now.
    pub fn work_held(&self) -> bool {
        self.lock().work_held
    }

    /// Stop every turn running or waiting now, as Stop does for one. Returns how many stopped.
    pub async fn stop_all_turns(&self) -> usize {
        self.stop_all_turns_report().await.stopped
    }

    /// [`Self::stop_all_turns`], saying also which sessions could not be stopped and why. Every
    /// turn is stopped at the same time, so one that is slow to stop never holds up the rest
    /// (the owner's report, 2026-10-05: Stop all seemed to do nothing). A turn that ended by
    /// itself meanwhile is neither.
    pub async fn stop_all_turns_report(&self) -> StopAllReport {
        let sessions: Vec<String> = {
            let state = self.lock();
            state
                .active
                .iter()
                .filter(|(_, a)| a.claim != Claim::Close)
                .map(|(id, _)| id.clone())
                .collect()
        };
        // Each stop's session, by its task: a stop that fails even to finish is still named.
        let mut stopping = tokio::task::JoinSet::new();
        let mut named = HashMap::new();
        for id in sessions {
            let this = self.clone();
            let session = id.clone();
            let stop = stopping.spawn(async move { this.cancel(&session, None).await });
            named.insert(stop.id(), id);
        }
        let mut report = StopAllReport::default();
        while let Some(joined) = stopping.join_next_with_id().await {
            let (stop, why) = match joined {
                Ok((_, Ok(_))) => {
                    report.stopped += 1;
                    continue;
                }
                Ok((_, Err(RuntimeError::NotReady(why))))
                    if why.starts_with("No turn is running") =>
                {
                    continue;
                }
                Ok((stop, Err(e))) => (stop, e.to_string()),
                Err(e) => (e.id(), e.to_string()),
            };
            let session = named.remove(&stop).unwrap_or_default();
            report.not_stopped.push((session, why));
        }
        report
    }

    fn release_hold(&self, runtime_id: &str, reason: HoldFor) {
        {
            let mut state = self.lock();
            if let Some(holds) = state.holds.get_mut(runtime_id) {
                let count = holds.count(reason);
                *count = count.saturating_sub(1);
                if *holds == Holds::default() {
                    state.holds.remove(runtime_id);
                }
            }
        }
        self.inner.holds_changed.notify_waiters();
        self.holds_shown();
    }

    /// Wait while `runtime_id` is held — while it updates, until the update and its checks are
    /// done; while only a sign-in tab holds it, [`AgentConfig::hold_wait`] at most — then mark
    /// the session's step as using it. `false`: the owner stopped the task while it waited.
    async fn wait_for_hold(&self, runtime_id: &str, session_id: &str) -> bool {
        let deadline = tokio::time::Instant::now() + self.inner.config.hold_wait;
        loop {
            let notified = self.inner.holds_changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let updating = {
                let mut state = self.lock();
                let holds = state.holds.get(runtime_id).copied().unwrap_or_default();
                let held_by_owner = state.work_held;
                let past = tokio::time::Instant::now() >= deadline;
                let Some(active) = state.active.get_mut(session_id) else {
                    return true;
                };
                if active.stop_waiting {
                    active.waiting_for_hold = false;
                    return false;
                }
                if !held_by_owner && holds.update == 0 && (holds.sign_in == 0 || past) {
                    active.runtime_id = Some(runtime_id.to_owned());
                    active.waiting_for_hold = false;
                    return true;
                }
                active.waiting_for_hold = true;
                // An update, or Stop all work: until it is let go.
                holds.update > 0 || held_by_owner
            };
            if updating {
                // Each step of an update has its own time limit, so the hold is let go.
                notified.await;
            } else {
                let _ = tokio::time::timeout_at(deadline, notified).await;
            }
        }
    }

    /// Check one AI tool again — installation, version, and sign-in — as after its sign-in tab
    /// closes (ADR-058 §4) or an update (ADR-059 §5). Returns the sign-in before, and the tool
    /// now.
    pub async fn recheck(&self, runtime_id: &str) -> Option<(AuthStatus, AgentRuntimeInfo)> {
        let adapter = self.adapter(runtime_id)?;
        let before = self
            .runtimes()
            .into_iter()
            .find(|r| r.id == runtime_id)
            .map(|r| r.auth)?;
        let (info, _) = self.detect(adapter.as_ref(), true).await;
        self.store_info(info);
        let runtimes = self.runtimes();
        let now = runtimes.iter().find(|r| r.id == runtime_id).cloned()?;
        self.inner
            .sink
            .emit(AgentUpdate::Runtimes(RuntimesUpdate { runtimes }));
        Some((before, now))
    }

    /// The AI tool's own program — found by its rules and allowed by the supervisor (ADR-005) —
    /// with the environment its tasks get, for its sign-in tab, its update, and its checks. For
    /// a bridged AI tool (Ollama), its real program, not Plenipo's bridge.
    pub fn tool_program(&self, runtime_id: &str) -> Result<ToolProgram, String> {
        let adapter = self
            .adapter(runtime_id)
            .ok_or_else(|| format!("Plenipo has no AI tool called {runtime_id:?}"))?;
        let host = &self.inner.host;
        // A paid AI tool's program is Plenipo's own helper (ADR-085).
        let located = if adapter.built_in() {
            match self.inner.config.bridge.as_ref() {
                Some(bridge) if bridge.executable.is_file() => {
                    Located::Found(bridge.executable.clone())
                }
                _ => Located::NotFound,
            }
        } else {
            locate(adapter.as_ref(), host)
        };
        let executable = match located {
            Located::Found(path) => path,
            Located::NotFound => {
                return Err(format!(
                    "{} is not installed on {}.",
                    adapter.label(),
                    plenipo_core::WORDS.this_computer
                ))
            }
            Located::Unsupported(_) => {
                return Err(format!(
                    "{} is installed in a form Plenipo does not run. Install the native build.",
                    adapter.label()
                ))
            }
        };
        let executable = self
            .inner
            .supervisor
            .allow_executable(&executable)
            .map_err(|e| e.to_string())?;
        let env = self.tool_env(adapter.as_ref())?;
        Ok(ToolProgram {
            label: adapter.label().to_owned(),
            executable,
            env,
            dir: self.probe_dir(),
        })
    }

    /// The newest version, from the AI tool's own check ([`NewestVersion::Command`]); `None`
    /// when it has none. Other sources are read by the caller (ADR-059 §2).
    pub async fn newest_by_command(&self, runtime_id: &str) -> Result<Option<String>, String> {
        let adapter = self
            .adapter(runtime_id)
            .ok_or_else(|| format!("Plenipo has no AI tool called {runtime_id:?}"))?;
        let NewestVersion::Command(args) = adapter.newest_version() else {
            return Ok(None);
        };
        let program = self.tool_program(runtime_id)?;
        let out = run_probe(
            &program.executable,
            &args,
            &program.env,
            &program.dir,
            self.inner.config.probe_timeout,
        )
        .await;
        match adapter.parse_newest(&out) {
            Some(version) => Ok(Some(version)),
            None => Err(probe_failure("check for a new version", &out)),
        }
    }

    /// The AI tool's short, task-free check (ADR-060): its own list of models, and for some
    /// tools how much of the plan is used. A list it reported is kept with the tool
    /// ([`AgentRuntimeInfo::reported_models`]).
    pub async fn status_check(&self, runtime_id: &str) -> Result<StatusAnswer, String> {
        let adapter = self
            .adapter(runtime_id)
            .ok_or_else(|| format!("Plenipo has no AI tool called {runtime_id:?}"))?;
        let program = self.tool_program(runtime_id)?;
        let timeout = self.inner.config.probe_timeout;
        let check = adapter.status_check(&program.dir);
        // A tool that talks answers its first request (`initialize`) the way Plenipo reads it
        // even when it then says it needs a sign-in, or is slow to list its models.
        let greeting = match &check {
            StatusCheck::Talk { answers, .. } => answers.first().copied(),
            _ => None,
        };
        let out = match check {
            StatusCheck::None => return Ok(StatusAnswer::default()),
            StatusCheck::Command(args) => {
                run_probe(
                    &program.executable,
                    &args,
                    &program.env,
                    &program.dir,
                    timeout,
                )
                .await
            }
            StatusCheck::Bridge(args) => {
                let bridge = self.inner.config.bridge.as_ref().ok_or_else(|| {
                    format!(
                        "This version of Plenipo cannot reach {} (its helper is not set up).",
                        adapter.label()
                    )
                })?;
                let executable = self
                    .inner
                    .supervisor
                    .allow_executable(&bridge.executable)
                    .map_err(|e| e.to_string())?;
                let args = [bridge.args.clone(), args].concat();
                run_probe(&executable, &args, &program.env, &program.dir, timeout).await
            }
            StatusCheck::Talk {
                args,
                lines,
                answers,
                framing,
            } => {
                run_talk(
                    &program.executable,
                    &args,
                    &program.env,
                    &program.dir,
                    &lines,
                    &answers,
                    framing,
                    timeout,
                )
                .await
            }
        };
        let models = adapter.parse_models(&out);
        let plan = adapter
            .parse_plan(&out)
            .map(|p| self.plans().keep(runtime_id, p));
        if let Some(plan) = &plan {
            self.inner.sink.emit(AgentUpdate::Plan(PlanUpdate {
                runtime_id: runtime_id.to_owned(),
                report: plan.clone(),
            }));
        }
        let greeted = greeting.is_some_and(|id| talk_answer(&out, id).is_some());
        if models.is_none() && plan.is_none() && !greeted {
            return Err(probe_failure("check", &out));
        }
        if let Some(models) = &models {
            self.set_reported_models(
                runtime_id,
                ReportedModels {
                    models: models.clone(),
                    complete: adapter.reports_every_model(),
                    checked_at: crate::now_ms(),
                },
            );
        }
        Ok(StatusAnswer {
            checked: true,
            models,
            plan,
        })
    }

    /// Keep the models `runtime_id` reported (a check, or the list saved in the Ledger when
    /// Plenipo starts), and tell the screen.
    pub fn set_reported_models(&self, runtime_id: &str, models: ReportedModels) {
        {
            let mut state = self.lock();
            let Some(slot) = state.runtimes.iter_mut().find(|r| r.id == runtime_id) else {
                return;
            };
            slot.reported_models = Some(models);
        }
        self.inner.sink.emit(AgentUpdate::Runtimes(RuntimesUpdate {
            runtimes: self.runtimes(),
        }));
    }

    /// Give `runtime_id` no tasks, and say why (`Some`), or give it tasks again (`None`): an
    /// update left it not answering the way Plenipo reads it (ADR-059 §6). Takes effect at its
    /// next check.
    pub fn set_out_of_service(&self, runtime_id: &str, why: Option<String>) {
        let mut state = self.lock();
        match why {
            Some(why) => state.out_of_service.insert(runtime_id.to_owned(), why),
            None => state.out_of_service.remove(runtime_id),
        };
    }

    /// Why `runtime_id` is given no tasks, if it is not ([`Self::set_out_of_service`]).
    pub fn out_of_service(&self, runtime_id: &str) -> Option<String> {
        self.lock().out_of_service.get(runtime_id).cloned()
    }

    /// An AI tool reported how much of the plan is used, during a task or in its check (ADR-060
    /// §3): kept in the PC's plan book, added to what it reported before, and told to the screen
    /// (Phase 25, item 4.3: a check in the background updates the open page too).
    fn plan_reported(&self, runtime_id: &str, report: PlanReport) {
        let report = self.plans().keep(runtime_id, report);
        self.inner.sink.emit(AgentUpdate::Plan(PlanUpdate {
            runtime_id: runtime_id.to_owned(),
            report,
        }));
    }

    /// Share one plan book with the PC's other organizations (Phase 25, item 4.3).
    pub fn share_plans(&self, book: Arc<PlanBook>) {
        *self
            .inner
            .plans
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = book;
    }

    /// The latest plan each AI tool reported, on this PC.
    pub fn plans(&self) -> Arc<PlanBook> {
        Arc::clone(
            &self
                .inner
                .plans
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    // ---- Sessions -----------------------------------------------------------------------

    pub async fn overview(&self) -> Result<AgentOverview, RuntimeError> {
        let sessions = self.with_store(|s| s.sessions(200)).await?;
        let sessions = sessions.into_iter().map(|s| self.with_active(s)).collect();
        let runtimes = self.runtimes();
        let state = self.lock();
        Ok(AgentOverview {
            runtimes,
            sessions,
            notices: state.notices.clone(),
        })
    }

    pub async fn session(&self, session_id: &str) -> Result<AgentSessionDetail, RuntimeError> {
        let id = session_id.to_owned();
        let (session, turns) = self
            .with_store(move |s| Ok((s.session(&id)?, s.turns(&id)?)))
            .await?;
        let session = session.ok_or_else(|| RuntimeError::UnknownSession(session_id.to_owned()))?;
        // One look at the session's claim for the summary and every turn, so the answer never
        // says the session is free while one of its turns reads as running, or the other way.
        let (session, turns) = {
            let state = self.lock();
            let active = state.active.get(session_id);
            let session = Self::with_claim(session, active);
            let turns: Vec<AgentTurn> = turns
                .into_iter()
                .map(|t| Self::decorated(t, active))
                .collect();
            (session, turns)
        };
        // The recent finished turns whose live pieces Plenipo no longer holds (it started again
        // since, or they are older than what it keeps in memory): what was kept of them instead,
        // from the record (ADR-203 §10). A turn not finished is never filled in, so its own live
        // pieces, numbered the same way, are never taken for ones already shown.
        let forgotten: Vec<AgentTurn> = {
            let state = self.lock();
            turns
                .iter()
                .rev()
                .take(RECALLED_TURNS)
                .filter(|t| t.result.is_some() && !state.activity.contains_key(&t.task_id))
                .rev()
                .cloned()
                .collect()
        };
        let recalled = if forgotten.is_empty() {
            Vec::new()
        } else {
            let id = session.id.clone();
            self.with_store(move |s| Ok(recall(s, &id, &forgotten)))
                .await
                .unwrap_or_default()
        };
        let state = self.lock();
        let mut activity: Vec<AgentActivity> = turns
            .iter()
            .filter_map(|t| state.activity.get(&t.task_id))
            .flat_map(|buf| buf.iter().cloned())
            .collect();
        // Filled-in pieces come after the held ones, so `activity` is not in turn order: its
        // readers group it by task and sort each task's pieces by `seq`.
        activity.extend(recalled);
        Ok(AgentSessionDetail {
            session,
            turns,
            activity,
        })
    }

    /// startSession + submitTask, for an objective from the owner.
    pub async fn start_session(
        &self,
        runtime_id: &str,
        objective: &str,
        model: Option<&str>,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        self.start_session_with(
            SessionStart {
                runtime_id: runtime_id.into(),
                model: model.map(str::to_owned),
                ..SessionStart::default()
            },
            TurnInput::owner(objective),
        )
        .await
    }

    /// startSession + submitTask with full control over the session and turn (Core only).
    pub async fn start_session_with(
        &self,
        start: SessionStart,
        input: TurnInput,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let input = validate_input(input)?;
        let model = start
            .model
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(validate_model)
            .transpose()?;
        let adapter = self.adapter(&start.runtime_id).ok_or_else(|| {
            RuntimeError::InvalidInput(format!("unknown runtime: {}", start.runtime_id))
        })?;
        if let Some(effort) = start.effort {
            if !adapter.capabilities().effort_levels.contains(&effort) {
                return Err(RuntimeError::InvalidInput(format!(
                    "{} has no {} effort level",
                    adapter.label(),
                    effort.label()
                )));
            }
        }
        let metadata = object_or_empty("session metadata", start.metadata)?;
        let session_id = match start.id {
            Some(id) => validate_session_id(&id)?,
            None => uuid::Uuid::new_v4().to_string(),
        };
        let title = start
            .title
            .map(|t| first_line(&t, 80))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| first_line(&input.objective, 80));
        let reservation = self.reserve(&session_id, Claim::Turn)?;
        let ready = self
            .preflight(adapter.as_ref(), &session_id)
            .await
            .map_err(|n| n.error)?;

        let working_dir = self.inner.config.workspace_root.join(&session_id);
        std::fs::create_dir_all(&working_dir).map_err(|e| {
            RuntimeError::NotReady(format!("Could not create the session workspace: {e}"))
        })?;
        let now = crate::now_ms();
        let session = AgentSession {
            id: session_id.clone(),
            runtime_id: adapter.id().into(),
            provider: adapter.provider().into(),
            provider_session_id: None,
            provider_session_confirmed: false,
            model,
            effort: start.effort,
            title,
            state: SessionState::Open,
            working_dir: working_dir.display().to_string(),
            created_at: now,
            updated_at: now,
            turn_count: 0,
            active_task_id: None,
            waiting_task_id: None,
            metadata,
        };
        let opened = session.clone();
        self.with_store(move |s| s.open_session(&opened)).await?;
        self.inner.sink.emit(AgentUpdate::Session(session.clone()));
        self.run_turn(session, adapter, ready, input, reservation)
            .await
    }

    /// resumeSession + submitTask, for an objective from the owner.
    pub async fn resume_session(
        &self,
        session_id: &str,
        objective: &str,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        self.resume_session_with(session_id, TurnInput::owner(objective))
            .await
    }

    /// resumeSession + submitTask with full control over the turn (Core only).
    pub async fn resume_session_with(
        &self,
        session_id: &str,
        input: TurnInput,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let input = validate_input(input)?;
        // Claim first, then read: a concurrent close either finished before (we see it closed)
        // or cannot start until this turn ends. A last turn that has ended and is only being
        // recorded is waited for, briefly.
        let reservation = self.reserve_turn(session_id).await?;
        let id = session_id.to_owned();
        let session = self
            .with_store(move |s| s.session(&id))
            .await?
            .ok_or_else(|| RuntimeError::UnknownSession(session_id.to_owned()))?;
        if session.state == SessionState::Closed {
            return Err(RuntimeError::NotReady(
                "This session is closed. Start a new task instead.".into(),
            ));
        }
        let adapter = self.adapter(&session.runtime_id).ok_or_else(|| {
            RuntimeError::NotReady(format!(
                "The AI tool {:?} is not available in this version of Plenipo.",
                session.runtime_id
            ))
        })?;
        let ready = self
            .preflight(adapter.as_ref(), session_id)
            .await
            .map_err(|n| n.error)?;
        self.run_turn(session, adapter, ready, input, reservation)
            .await
    }

    /// Continue a waiting turn with its next step: `prompt` goes to the same provider session.
    /// `Busy` means no worker slot is free (try again later); a runtime that cannot take work
    /// ends the turn as failed with the reason. The session stays reserved throughout.
    pub async fn continue_turn(
        &self,
        session_id: &str,
        task_id: &str,
        prompt: &str,
        note: StepNote,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        self.continue_with(session_id, task_id, prompt, note, false)
            .await
    }

    /// A short extra step of a waiting turn that must never end it (ADR-259, a lead's check-in
    /// on its team): like [`Self::continue_turn`], but when its AI tool can't take work now
    /// (not available, not signed in, …) the turn waits again, unchanged, instead of ending as
    /// failed. The step's own end is still the hook's to decide.
    pub async fn check_in_turn(
        &self,
        session_id: &str,
        task_id: &str,
        prompt: &str,
        note: StepNote,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        self.continue_with(session_id, task_id, prompt, note, true)
            .await
    }

    /// [`Self::continue_turn`]; `keep_waiting`: an AI tool that can't take work leaves the turn
    /// waiting ([`Self::check_in_turn`]).
    async fn continue_with(
        &self,
        session_id: &str,
        task_id: &str,
        prompt: &str,
        note: StepNote,
        keep_waiting: bool,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let prompt = validate_prompt(prompt)?;
        let (step, done) = {
            let mut state = self.lock();
            if state.shutting_down {
                return Err(RuntimeError::ShuttingDown);
            }
            let running = state
                .active
                .values()
                .filter(|a| a.claim == Claim::Turn)
                .count();
            let not_waiting = || {
                RuntimeError::NotWaiting(format!(
                    "Session {session_id} is not waiting to continue task {task_id}."
                ))
            };
            let active = state.active.get_mut(session_id).ok_or_else(not_waiting)?;
            if active.task_id.as_deref() != Some(task_id) {
                return Err(not_waiting());
            }
            match active.claim {
                Claim::Wait => {}
                Claim::Turn => {
                    return Err(RuntimeError::Busy(
                        "The turn's last step is still being recorded; try again in a moment."
                            .into(),
                    ))
                }
                // Being cancelled: the caller sees the turn end on its next look.
                Claim::Close => {
                    return Err(RuntimeError::Busy(
                        "The turn is being cancelled; it will not continue.".into(),
                    ))
                }
            }
            if running >= self.inner.config.max_active_turns {
                return Err(RuntimeError::Busy(format!(
                    "{running} agent turns are already running; wait for one to finish."
                )));
            }
            let (tx, rx) = watch::channel(false);
            active.claim = Claim::Turn;
            active.execution_id = None;
            // Not using its AI tool until its check passes (it may wait for a hold first).
            active.runtime_id = None;
            active.waiting_for_hold = false;
            active.stop_waiting = false;
            active.step += 1;
            active.step_started_at = crate::now_ms();
            active.done = rx;
            (active.step, tx)
        };
        let id = session_id.to_owned();
        let session = match self.with_store(move |s| s.session(&id)).await {
            Ok(Some(session)) => session,
            Ok(None) => {
                let e = RuntimeError::UnknownSession(session_id.to_owned());
                self.end_waiting(session_id, task_id, None, TurnOutcome::Failed, &e, done)
                    .await;
                return Err(e);
            }
            Err(e) => {
                self.wait_again(session_id, done);
                return Err(e);
            }
        };
        let Some(adapter) = self.adapter(&session.runtime_id) else {
            let e = RuntimeError::NotReady(format!(
                "The AI tool {:?} is not available in this version of Plenipo.",
                session.runtime_id
            ));
            if keep_waiting {
                self.wait_again(session_id, done);
                return Err(e);
            }
            self.end_waiting(
                session_id,
                task_id,
                Some(&session),
                TurnOutcome::ProviderUnavailable,
                &e,
                done,
            )
            .await;
            return Err(e);
        };
        let ready = match self.preflight(adapter.as_ref(), session_id).await {
            Ok(ready) => ready,
            Err(n) if keep_waiting => {
                self.wait_again(session_id, done);
                return Err(n.error);
            }
            Err(n) => {
                self.end_waiting(
                    session_id,
                    task_id,
                    Some(&session),
                    n.outcome,
                    &n.error,
                    done,
                )
                .await;
                return Err(n.error);
            }
        };
        let actor = format!("agent:{}", session.runtime_id);
        let (sid, tid) = (session_id.to_owned(), task_id.to_owned());
        let passed = note.passed_bytes;
        if let Err(e) = self
            .with_store(move |s| {
                let turn = TurnRef {
                    session_id: &sid,
                    task_id: &tid,
                    execution_id: None,
                    step: Some(step),
                    actor: &actor,
                };
                s.begin_step(&turn, &note)
            })
            .await
        {
            self.wait_again(session_id, done);
            return Err(e);
        }
        let number = self
            .with_store({
                let (s, t) = (session_id.to_owned(), task_id.to_owned());
                move |store| Ok(store.turns(&s)?.into_iter().find(|x| x.task_id == t))
            })
            .await
            .ok()
            .flatten()
            .map_or(session.turn_count, |t| t.number);
        let request = turn_request(&session, adapter.as_ref(), &ready);
        self.launch_step(
            session,
            adapter,
            ready,
            request,
            StepLaunch {
                task_id: task_id.to_owned(),
                number,
                step,
                // A continuation never carries Liaison's instructions: only the replies.
                message: StepMessage::Replies {
                    text: prompt,
                    passed,
                },
            },
            Some(done),
        )
        .await
    }

    /// A continuation could not start for a passing reason: the turn waits again. (Nothing
    /// was freed, so the hook is not told; its caller retries later.)
    fn wait_again(&self, session_id: &str, done: watch::Sender<bool>) {
        if let Some(active) = self.lock().active.get_mut(session_id) {
            active.claim = Claim::Wait;
            active.step = active.step.saturating_sub(1).max(1);
            active.done = finished();
        }
        let _ = done.send(true);
    }

    /// A waiting turn ends without another step (it could not continue, or was cancelled).
    async fn end_waiting(
        &self,
        session_id: &str,
        task_id: &str,
        session: Option<&AgentSession>,
        outcome: TurnOutcome,
        error: &RuntimeError,
        done: watch::Sender<bool>,
    ) {
        let (summary, detail) = match outcome {
            TurnOutcome::Cancelled => ("Cancelled while waiting to continue".to_owned(), None),
            _ => (
                format!(
                    "Could not continue: {}",
                    first_line(&error.to_string(), 240)
                ),
                Some(error.to_string()),
            ),
        };
        let result = administrative(outcome, &summary, detail);
        let actor = session.map_or_else(
            || "plenipo".to_owned(),
            |s| format!("agent:{}", s.runtime_id),
        );
        let (sid, tid) = (session_id.to_owned(), task_id.to_owned());
        if let Err(e) = self
            .with_store(move |s| {
                let turn = TurnRef {
                    session_id: &sid,
                    task_id: &tid,
                    execution_id: None,
                    step: None,
                    actor: &actor,
                };
                s.finish_turn(&turn, &result)
            })
            .await
        {
            self.notice(e.to_string());
        }
        self.lock().active.remove(session_id);
        self.inner.turn_released.notify_waiters();
        let _ = done.send(true);
        self.emit_turn(session_id, task_id).await;
        if let Some(session) = session {
            self.emit_session(session.clone());
        }
        self.released();
    }

    /// cancelExecution: stop the session's running turn — or end its wait — and return once it
    /// is recorded.
    pub async fn cancel_turn(&self, session_id: &str) -> Result<AgentSessionDetail, RuntimeError> {
        self.cancel(session_id, None).await
    }

    /// Stop `task_id` in `session_id` — only if that is the turn the session is working on now
    /// (running or waiting). A member of the organization keeps one conversation for all its
    /// work, so stopping one of its tasks must never stop another (Phase 8, ADR-016).
    pub async fn cancel_task(
        &self,
        session_id: &str,
        task_id: &str,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        self.cancel(session_id, Some(task_id)).await
    }

    async fn cancel(
        &self,
        session_id: &str,
        expected: Option<&str>,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        enum Target {
            Running(
                String,
                Option<String>,
                watch::Receiver<bool>,
                Option<mpsc::UnboundedSender<()>>,
            ),
            Waiting(String),
            /// Waiting for its AI tool (an update or a sign-in tab), or still starting (its
            /// check, its tools): marked, it ends as stopped before it runs.
            Starting(watch::Receiver<bool>),
        }
        // It has ended and is only being recorded (#198): nothing is left to stop. It is let go in
        // a moment; the answer waits for that, so it shows the turn ended.
        let releasing = self
            .lock()
            .active
            .get(session_id)
            .filter(|a| a.releasing)
            .map(|a| a.task_id.clone());
        if let Some(task) = releasing {
            let _ = tokio::time::timeout(
                self.inner.config.release_wait,
                self.released_now(session_id, task.as_deref()),
            )
            .await;
            return self.session(session_id).await;
        }
        let target = {
            let mut state = self.lock();
            let active = state
                .active
                .get(session_id)
                .filter(|a| a.claim != Claim::Close)
                .ok_or_else(|| {
                    RuntimeError::NotReady("No turn is running in this session.".into())
                })?;
            if let Some(expected) = expected {
                match active.task_id.as_deref() {
                    Some(t) if t == expected => {}
                    // Its task is recorded a moment after the turn is reserved; retry then.
                    None => {
                        return Err(RuntimeError::NotReady(
                            "The turn is still starting; try again in a moment.".into(),
                        ))
                    }
                    Some(_) => {
                        return Err(RuntimeError::NotReady(format!(
                            "Task {expected} is not the turn this session is working on."
                        )))
                    }
                }
            }
            match active.claim {
                Claim::Wait => {
                    let task = active.task_id.clone().unwrap_or_default();
                    // Claim it for the cancel, so a continuation cannot start meanwhile. Nothing
                    // runs under it any more: a new turn waits for it to be let go.
                    if let Some(active) = state.active.get_mut(session_id) {
                        active.claim = Claim::Close;
                        active.releasing = true;
                    }
                    Target::Waiting(task)
                }
                _ => match active.execution_id.clone() {
                    // Waiting for its AI tool, or still starting: no process yet. Marked, the
                    // step ends as stopped before it runs (`wait_for_hold`, `launch_step`).
                    None => {
                        let done = active.done.clone();
                        if let Some(active) = state.active.get_mut(session_id) {
                            active.stop_waiting = true;
                        }
                        Target::Starting(done)
                    }
                    Some(execution) => Target::Running(
                        execution,
                        active.task_id.clone(),
                        active.done.clone(),
                        active.interrupt.clone(),
                    ),
                },
            }
        };
        let waiting = match target {
            Target::Running(execution, task, mut done, interrupt) => {
                // A task that talks is asked to stop itself first (ADR-015 §7).
                let asked = interrupt.is_some_and(|i| i.send(()).is_ok());
                let stopped = asked
                    && tokio::time::timeout(CANCEL_GRACE, done.wait_for(|d| *d))
                        .await
                        .is_ok();
                if !stopped {
                    // Its program and everything it started are ended; the supervisor waits for
                    // them and their output.
                    let _ = self.inner.supervisor.cancel(&execution).await;
                    // A step that still has not ended by itself is ended here: recorded as
                    // stopped, its session let go (the owner's report, 2026-10-05).
                    if tokio::time::timeout(HARD_END_WAIT, done.wait_for(|d| *d))
                        .await
                        .is_err()
                    {
                        if let Some(task) = &task {
                            self.force_end(session_id, task, &execution).await;
                        }
                    }
                }
                // The step may have ended just before the cancel, with the turn going on to wait
                // for handoff replies (it already reads as waiting): end that wait as well. It is
                // claimed for the cancel in the same look, so a continuation cannot start first.
                let mut state = self.lock();
                match state.active.get_mut(session_id) {
                    Some(active) if active.claim == Claim::Wait && active.task_id == task => {
                        active.claim = Claim::Close;
                        active.releasing = true;
                        task
                    }
                    _ => None,
                }
            }
            Target::Waiting(task_id) => Some(task_id),
            Target::Starting(mut done) => {
                // A step waiting for its AI tool wakes to the mark; a step still starting sees it
                // before it runs anything. The answer waits a moment for that.
                self.inner.holds_changed.notify_waiters();
                let _ =
                    tokio::time::timeout(CANCEL_GRACE + HARD_END_WAIT, done.wait_for(|d| *d)).await;
                None
            }
        };
        if let Some(task_id) = waiting {
            let id = session_id.to_owned();
            let session = self
                .with_store(move |s| s.session(&id))
                .await
                .ok()
                .flatten();
            let (done, _) = watch::channel(false);
            self.end_waiting(
                session_id,
                &task_id,
                session.as_ref(),
                TurnOutcome::Cancelled,
                &RuntimeError::NotReady("cancelled".into()),
                done,
            )
            .await;
        }
        self.session(session_id).await
    }

    /// Wait until `session_id` no longer holds the turn of `task_id` (it was let go).
    async fn released_now(&self, session_id: &str, task_id: Option<&str>) {
        loop {
            let released = self.inner.turn_released.notified();
            tokio::pin!(released);
            released.as_mut().enable();
            let held = self
                .lock()
                .active
                .get(session_id)
                .is_some_and(|a| a.task_id.as_deref() == task_id);
            if !held {
                return;
            }
            released.await;
        }
    }

    /// End a stopped step that has not ended by itself: its AI tool did not answer the stop, its
    /// program was ended, and the step's own finisher still has not come (the owner's report,
    /// 2026-10-05). The turn is recorded as stopped, with one row of diagnostics and never the
    /// AI tool's words; the hook hears of it, its tools are closed, its session is let go, and
    /// the screens are told. The step's finisher, if it ever comes, records and announces
    /// nothing more.
    async fn force_end(&self, session_id: &str, task_id: &str, execution_id: &str) {
        let (grant, step, runtime_id, watch) = {
            let mut state = self.lock();
            let Some(active) = state.active.get_mut(session_id).filter(|a| {
                a.task_id.as_deref() == Some(task_id)
                    && a.execution_id.as_deref() == Some(execution_id)
                    && !a.releasing
            }) else {
                return;
            };
            // Under the same lock as the finisher's own looks (`complete`): one of the two ends
            // the step, never both.
            match &active.forced {
                Some(forced) if !forced.swap(true, Ordering::SeqCst) => {}
                _ => return,
            }
            // Only being recorded now: a new turn of the session waits for that (#198) instead
            // of being refused as busy.
            active.releasing = true;
            let ended = (
                active.grant.clone(),
                active.step,
                active.runtime_id.clone().unwrap_or_default(),
                active.watch.clone(),
            );
            // What it sent may or may not have reached the AI tool: it goes out in full again.
            if let Some((mark, sent)) = active.sent.take() {
                Self::step_finished(&mut state, session_id, mark, sent, false, None);
            }
            ended
        };
        if let Some(grant) = grant {
            self.close_tools(grant).await;
        }
        let process = self
            .inner
            .supervisor
            .record(execution_id)
            .map(|r| r.state == crate::dto::ExecutionState::Running);
        let (label, diagnostics) = match &watch {
            Some(w) => {
                let w = w.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let why = "its AI tool did not answer the stop, and its step did not end";
                (w.label, Some(w.diagnostics(why, process)))
            }
            None => ("Its AI tool", None),
        };
        let result = administrative(
            TurnOutcome::Cancelled,
            &format!("Stopped. {label} didn't answer the stop, so Plenipo ended it."),
            diagnostics.clone(),
        );
        // The hook hears of the end as of any other, before it is recorded: a check-in step is
        // closed, and what the hook kept for the step is let go. A step that did not complete
        // always finishes, so what the hook decides is not needed; and it waits a few seconds
        // at most, since the finisher may be stuck in the same hook. Whether the AI tool kept
        // the conversation can't be told (`memory_mark: None`).
        if let Some(hook) = self.hook() {
            let id = session_id.to_owned();
            if let Ok(Some(session)) = self.with_store(move |s| s.session(&id)).await {
                let end = TurnEnd {
                    session,
                    task_id: task_id.to_owned(),
                    step,
                    execution_id: Some(execution_id.to_owned()),
                    result: result.clone(),
                    memory_mark: None,
                };
                let _ = tokio::time::timeout(
                    FORCED_HOOK_WAIT,
                    tokio::task::spawn_blocking(move || hook.turn_ended(&end)),
                )
                .await;
            }
        }
        let (sid, tid, eid) = (
            session_id.to_owned(),
            task_id.to_owned(),
            execution_id.to_owned(),
        );
        let stored = result.clone();
        let actor = format!("agent:{runtime_id}");
        if let Err(e) = self
            .with_store(move |s| {
                let turn = TurnRef {
                    session_id: &sid,
                    task_id: &tid,
                    execution_id: Some(&eid),
                    step: Some(step),
                    actor: &actor,
                };
                if let Some(text) = diagnostics {
                    let note = AgentEvent::Notice {
                        level: NoticeLevel::Warning,
                        text,
                    };
                    s.record_activity(&turn, &note)?;
                }
                s.finish_turn(&turn, &stored)
            })
            .await
        {
            self.notice(e.to_string());
        }
        {
            let mut state = self.lock();
            if state
                .active
                .get(session_id)
                .is_some_and(|a| a.task_id.as_deref() == Some(task_id))
            {
                state.active.remove(session_id);
            }
        }
        self.inner.turn_released.notify_waiters();
        self.emit_turn_with(session_id, task_id, Some(&result))
            .await;
        if let Ok(Some(session)) = {
            let id = session_id.to_owned();
            self.with_store(move |s| s.session(&id)).await
        } {
            self.emit_session(session);
        }
        self.released();
    }

    /// closeSession: no further turns. The provider keeps its own transcript.
    pub async fn close_session(&self, session_id: &str) -> Result<AgentSession, RuntimeError> {
        let _claim = self.reserve(session_id, Claim::Close)?;
        let id = session_id.to_owned();
        let mut session = self
            .with_store(move |s| s.session(&id))
            .await?
            .ok_or_else(|| RuntimeError::UnknownSession(session_id.to_owned()))?;
        if session.state == SessionState::Closed {
            return Ok(session);
        }
        session.state = SessionState::Closed;
        session.updated_at = crate::now_ms();
        let saved = session.clone();
        self.with_store(move |s| s.save_session(&saved, SessionChange::Closed))
            .await?;
        self.lock().conversations.remove(session_id);
        self.inner.sink.emit(AgentUpdate::Session(session.clone()));
        Ok(session)
    }

    /// Refuse new turns, stop running ones (through the supervisor), and wait up to `grace`
    /// for their results to be recorded. Waiting turns stay as recorded (a restart marks them
    /// interrupted).
    pub async fn shutdown(&self, grace: Duration) -> usize {
        let consumers = {
            let mut state = self.lock();
            state.shutting_down = true;
            std::mem::take(&mut state.consumers)
        };
        let stopped = self.inner.supervisor.shutdown(grace).await;
        let _ = tokio::time::timeout(grace, async {
            for c in consumers {
                let _ = c.await;
            }
        })
        .await;
        stopped
    }

    // ---- Turns --------------------------------------------------------------------------

    /// Claim `session_id` for one turn (or a close). Released when the guard drops, unless a
    /// turn was launched (then the turn's consumer releases it).
    fn reserve(&self, session_id: &str, claim: Claim) -> Result<Reservation, RuntimeError> {
        self.try_reserve(session_id, claim).map_err(|b| b.error)
    }

    /// Claim `session_id` for a new turn. A turn of the session that has ended and is only being
    /// recorded ([`Active::releasing`]) is waited for, [`AgentConfig::release_wait`] at most:
    /// after a stop, or a finished step, the next objective starts instead of being told a turn
    /// is running. A turn that is really running, or waiting to continue, is refused at once,
    /// as is everything else [`AgentRuntime::reserve`] refuses; after the wait, the last refusal
    /// is returned unchanged.
    ///
    /// Each look's wake-up is pinned and enabled before reserve, so a claim let go between the
    /// refusal and the wait still wakes it ([`tokio::sync::Notify::notify_waiters`] wakes only
    /// waiters already registered).
    async fn reserve_turn(&self, session_id: &str) -> Result<Reservation, RuntimeError> {
        let deadline = tokio::time::Instant::now() + self.inner.config.release_wait;
        loop {
            let released = self.inner.turn_released.notified();
            tokio::pin!(released);
            released.as_mut().enable();
            match self.try_reserve(session_id, Claim::Turn) {
                Ok(reservation) => return Ok(reservation),
                Err(Blocked {
                    error,
                    releasing: true,
                }) => {
                    if tokio::time::timeout_at(deadline, released).await.is_err() {
                        return Err(error);
                    }
                }
                Err(Blocked { error, .. }) => return Err(error),
            }
        }
    }

    /// [`AgentRuntime::reserve`], saying also whether the claim in the way is only being recorded
    /// and let go: read under the same lock as the refusal, so the decision is one look.
    fn try_reserve(&self, session_id: &str, claim: Claim) -> Result<Reservation, Blocked> {
        let mut state = self.lock();
        if state.shutting_down {
            return Err(Blocked {
                error: RuntimeError::ShuttingDown,
                releasing: false,
            });
        }
        if let Some(existing) = state.active.get(session_id) {
            let releasing = existing.releasing;
            let error = match (existing.claim, claim) {
                (Claim::Close, _) => RuntimeError::NotReady("This session is being closed.".into()),
                (Claim::Wait, Claim::Turn) => RuntimeError::SessionBusy(
                    "This session's turn is waiting to continue (for example for handoff \
                     replies). Cancel the turn to stop waiting."
                        .into(),
                ),
                (Claim::Wait, _) => RuntimeError::NotReady(
                    "This session's turn is waiting to continue (for example for handoff \
                     replies). Cancel the turn to stop waiting."
                        .into(),
                ),
                (Claim::Turn, Claim::Close) => RuntimeError::NotReady(
                    "A turn is running in this session. Cancel it first.".into(),
                ),
                (Claim::Turn, _) => RuntimeError::SessionBusy(
                    "A turn is already running in this session. Wait for it or cancel it.".into(),
                ),
            };
            return Err(Blocked { error, releasing });
        }
        let turns = state
            .active
            .values()
            .filter(|a| a.claim == Claim::Turn)
            .count();
        if claim == Claim::Turn && turns >= self.inner.config.max_active_turns {
            return Err(Blocked {
                error: RuntimeError::Busy(format!(
                    "{turns} agent turns are already running; wait for one to finish."
                )),
                releasing: false,
            });
        }
        let (done_tx, done_rx) = watch::channel(false);
        state.active.insert(
            session_id.to_owned(),
            Active {
                claim,
                runtime_id: None,
                task_id: None,
                execution_id: None,
                step: 1,
                step_started_at: crate::now_ms(),
                done: done_rx,
                interrupt: None,
                waiting_for_hold: false,
                stop_waiting: false,
                releasing: false,
                grant: None,
                sent: None,
                forced: None,
                watch: None,
            },
        );
        Ok(Reservation {
            runtime: self.clone(),
            session_id: session_id.to_owned(),
            done: Some(done_tx),
        })
    }

    /// Whether the owner stopped `task_id`'s step in `session_id` while it was starting (no
    /// process yet): then nothing is run.
    fn stopped_while_starting(&self, session_id: &str, task_id: &str) -> bool {
        self.lock()
            .active
            .get(session_id)
            .is_some_and(|a| a.task_id.as_deref() == Some(task_id) && a.stop_waiting)
    }

    async fn run_turn(
        &self,
        mut session: AgentSession,
        adapter: Arc<dyn RuntimeAdapter>,
        ready: Ready,
        input: TurnInput,
        mut reservation: Reservation,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let number = session.turn_count + 1;
        let request = turn_request(&session, adapter.as_ref(), &ready);
        let (s, recorded) = (session.clone(), input.clone());
        let task_id = self
            .with_store(move |store| store.begin_turn(&s, number, &recorded))
            .await?;
        session.turn_count = number;
        session.updated_at = crate::now_ms();
        if let Some(active) = self.lock().active.get_mut(&session.id) {
            active.task_id = Some(task_id.clone());
        }
        let message = first_message(input);
        let done = reservation.done.take();
        self.launch_step(
            session,
            adapter,
            ready,
            request,
            StepLaunch {
                task_id,
                number,
                step: 1,
                message,
            },
            done,
        )
        .await
    }

    /// A paid step's charge (ADR-085): the key, the model's price from the tool's own list, the
    /// step's limits, and the most it could cost set aside under the owner's spending caps.
    /// Refused (with the reason) without a key, without a price, or when it does not fit.
    async fn paid_step(
        &self,
        adapter: &dyn RuntimeAdapter,
        key: Option<&PaidKey>,
        request: &TurnRequest,
        task_id: &str,
        prompt_bytes: usize,
    ) -> Result<PaidStep, String> {
        let label = adapter.label();
        let gate = self.paid_gate().ok_or_else(|| {
            format!("This version of Plenipo cannot use paid AI keys for {label}.")
        })?;
        let key = key
            .cloned()
            .ok_or_else(|| format!("{label} has no paid key saved."))?;
        let model = request
            .model
            .clone()
            .or_else(|| adapter.default_model().map(str::to_owned))
            .ok_or_else(|| format!("No model was named for {label}."))?;
        let reported = self
            .lock()
            .runtimes
            .iter()
            .find(|r| r.id == adapter.id())
            .and_then(|r| r.reported_models.clone());
        let price = adapter
            .price_of(&model, reported.as_ref().map(|r| r.models.as_slice()))
            .ok_or_else(|| {
                format!(
                    "{model} on {label} is not priced yet: Plenipo does not know what it costs, \
                     so it will not use a paid key for it."
                )
            })?;
        let limits = adapter.paid_limits(request, prompt_bytes);
        let charge = PaidCharge {
            task_id: task_id.to_owned(),
            runtime_id: adapter.id().to_owned(),
            model,
            key_id: key.id.clone(),
            key_name: key.name.clone(),
            // Where the answer-length field leaves the thinking out, the most the model can write.
            most_micros: price.most(
                limits.input_tokens,
                adapter
                    .most_output_tokens(request)
                    .map_or(limits.output_tokens, |most| most.max(limits.output_tokens)),
            ),
        };
        let g = gate.clone();
        let ticket = tokio::task::spawn_blocking(move || g.set_aside(&charge))
            .await
            .map_err(|e| format!("Plenipo could not check the spending caps: {e}"))??;
        Ok(PaidStep {
            gate,
            ticket,
            price,
            limits,
            key_line: key.stdin_line(),
        })
    }

    /// Launch one step of a turn whose session is claimed; the step's consumer releases or
    /// converts the claim when it ends.
    async fn launch_step(
        &self,
        session: AgentSession,
        adapter: Arc<dyn RuntimeAdapter>,
        ready: Ready,
        mut request: TurnRequest,
        launch: StepLaunch,
        done: Option<watch::Sender<bool>>,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let StepLaunch {
            task_id,
            number,
            step,
            message,
        } = launch;
        // A conversation the AI tool would shorten to take this step loses its start, and the
        // full instructions with it: the step goes out as after a shortened memory (ADR-044
        // §2.5), with the full instructions and note, and saved records pasted.
        if message
            .shortest_brief_len()
            .is_some_and(|len| adapter.leaves_out(&request, len))
        {
            self.memory_shortened(&session.id);
        }
        // What goes out depends on what the conversation already has (ADR-044).
        let (known, mark) =
            self.conversation_at_launch(&session.id, &request, adapter.reports_memory_shortened());
        let standing = match (&request.session, &known) {
            (ProviderSession::New { .. }, _) => Standing::New,
            (_, None) => Standing::Unknown,
            (_, Some(c)) => Standing::Known(c),
        };
        let out = message.outgoing(standing);
        // Plenipo's tools for this step (Phase 7), when the worker has permissions and its AI
        // tool can use them.
        let takes_tools = adapter.accepts_tools();
        let tools = if takes_tools {
            self.open_tools(&session, &task_id, step, adapter.label())
                .await
        } else {
            None
        };
        let grant = tools.as_ref().map(|t| t.grant_id.clone());
        let note = match &tools {
            Some(t) => {
                request.tools = Some(t.server.clone());
                Some(t.note.clone())
            }
            None => {
                self.note_without_tools(&session, &task_id, step, adapter.label(), takes_tools)
                    .await
            }
        };
        // The permissions note in full, or a short one that keeps the safety rules in view, and
        // what goes out and its size (ADR-044): sizes only, never the text.
        let note_hash = note.as_deref().map(text_hash);
        let full_note = note
            .as_deref()
            .is_some_and(|n| note_in_full(standing, out.kind == BriefKind::Full, n));
        let (prompt, size) = match &note {
            Some(note) if full_note => (
                with_note(note, &out.text),
                out.size(Some(note), NoteKind::Full, Some(note)),
            ),
            Some(note) => (
                with_note(NOTE_REMINDER, &out.text),
                out.size(Some(NOTE_REMINDER), NoteKind::Reminder, Some(note)),
            ),
            None => (out.text.clone(), out.size(None, NoteKind::None, None)),
        };
        let delivery = out.delivery(note_hash.filter(|_| full_note));
        // A paid AI tool (ADR-085): the most this step could cost is set aside under the owner's
        // spending caps before anything is sent, or the step does not start.
        let paid = if adapter.paid() {
            Some(
                self.paid_step(
                    adapter.as_ref(),
                    ready.paid_key.as_ref(),
                    &request,
                    &task_id,
                    prompt.len(),
                )
                .await,
            )
        } else {
            None
        };
        let mut env = ready.env;
        env.extend(adapter.turn_env(&request));
        let session_id = session.id.clone();
        let (tx, rx) = mpsc::unbounded_channel::<OutputLine>();
        let provider_session = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { preassigned } => preassigned.clone(),
        };
        let label = if step == 1 {
            format!("{} · task {number}", adapter.label())
        } else {
            format!("{} · task {number} · step {step}", adapter.label())
        };
        let mut parser = adapter.parser(&request);
        // What the AI tool last said about its context, to tell when it shrinks (ADR-044).
        parser.set_context_used(known.as_ref().and_then(|c| c.context_used));
        // A task that talks (ADR-015) opens with the parser's own lines and keeps stdin open;
        // otherwise the prompt is the whole of stdin.
        let (stdin, input) = match parser.open(&prompt) {
            None => (Some(parser.input(prompt).into_bytes()), None),
            Some(opening) => {
                let (tx, feed) = StdinFeed::new();
                for line in opening {
                    let _ = tx.send(input_line(line));
                }
                (None, Some((tx, feed)))
            }
        };
        // The paid helper's key goes first on its input, never on its command line; its limits
        // go on the command line.
        let stdin = match (&paid, stdin) {
            (Some(Ok(step)), Some(bytes)) => Some([step.key_line.as_bytes(), &bytes].concat()),
            (_, stdin) => stdin,
        };
        let paid_args = match &paid {
            Some(Ok(step)) => crate::agent::paid::step_args(&step.limits, &step.price),
            _ => Vec::new(),
        };
        let (interrupt_tx, interrupt_rx) = match input {
            Some(_) => {
                let (tx, rx) = mpsc::unbounded_channel();
                (Some(tx), Some(rx))
            }
            None => (None, None),
        };
        let spec = LaunchSpec {
            profile_id: format!("agent.{}", adapter.id()),
            label,
            executable: ready.executable,
            args: [ready.args_prefix, adapter.turn_args(&request), paid_args].concat(),
            env,
            working_dir: PathBuf::from(&session.working_dir),
            max_runtime: self.inner.config.turn_timeout,
            stdin,
            stdin_feed: input.as_ref().map(|(_, feed)| feed.clone()),
            max_line_bytes: Some(self.inner.config.max_line_bytes),
            observer: Some(tx),
            agent: Some(Box::new(AgentAttribution {
                runtime_id: adapter.id().into(),
                provider: adapter.provider().into(),
                session_id: session.id.clone(),
                task_id: task_id.clone(),
                model: session.model.clone(),
                provider_session_id: provider_session,
                usage: None,
                prompt: Some(size),
            })),
            extra_pipes: None,
        };
        let ctx = TurnContext {
            runtime: self.clone(),
            session: session.clone(),
            task_id: task_id.clone(),
            step,
            execution_id: None,
            seq: u64::from(step.saturating_sub(1)) * STEP_SEQ,
            stored: 0,
            grant,
            input: input.map(|(tx, _)| tx),
            size,
            mark,
            delivery,
            context_used: None,
            paid: None,
            paid_bill: None,
            live: LiveText::default(),
            forced: Arc::new(AtomicBool::new(false)),
            watch: Arc::new(Mutex::new(StepWatch::new(
                adapter.label(),
                ready.cli_version.clone(),
            ))),
        };
        let ctx = match paid {
            Some(Ok(step)) => TurnContext {
                paid: Some(step),
                ..ctx
            },
            // Refused by the spending caps, not priced yet, or no key: nothing is sent.
            Some(Err(why)) => {
                let result = TurnResult {
                    outcome: TurnOutcome::BillingNotAllowed,
                    summary: first_line(&why, 300),
                    text: None,
                    error: Some(cap(&why, MAX_EVENT_TEXT)),
                    provider_session_id: None,
                    model: None,
                    usage: None,
                    duration_ms: None,
                    ignored_lines: 0,
                    prompt: None,
                };
                ctx.complete(result, done).await;
                return self.session(&session_id).await;
            }
            None => ctx,
        };
        // Stopped while the step was starting: nothing is run.
        if self.stopped_while_starting(&session.id, &task_id) {
            let result = administrative(
                TurnOutcome::Cancelled,
                &format!("Stopped before {} started.", adapter.label()),
                None,
            );
            ctx.complete(result, done).await;
            return self.session(&session_id).await;
        }
        let execution_id = match self.inner.supervisor.launch(spec).await {
            Ok(record) => record.id,
            Err(e) => {
                // Nothing ran: record the step as failed right away.
                let result = TurnResult {
                    outcome: TurnOutcome::ProviderUnavailable,
                    summary: format!("{} could not be started", adapter.label()),
                    text: None,
                    error: Some(cap(&e.to_string(), MAX_EVENT_TEXT)),
                    provider_session_id: None,
                    model: None,
                    usage: None,
                    duration_ms: None,
                    ignored_lines: 0,
                    prompt: None,
                };
                ctx.complete(result, done).await;
                return self.session(&session_id).await;
            }
        };
        // A stop that came while the step was starting, after the look above, ends it now.
        let stopped = {
            let mut state = self.lock();
            match state.active.get_mut(&session.id) {
                Some(active) if active.task_id.as_deref() == Some(task_id.as_str()) => {
                    active.execution_id = Some(execution_id.clone());
                    active.interrupt = interrupt_tx;
                    active.grant.clone_from(&ctx.grant);
                    active.sent = Some((ctx.mark, ctx.delivery));
                    active.forced = Some(Arc::clone(&ctx.forced));
                    active.watch = Some(Arc::clone(&ctx.watch));
                    active.stop_waiting
                }
                _ => false,
            }
        };
        if stopped {
            let supervisor = self.inner.supervisor.clone();
            let id = execution_id.clone();
            tokio::spawn(async move {
                let _ = supervisor.cancel(&id).await;
            });
        }
        self.emit_turn(&session_id, &task_id).await;
        self.emit_session(session);
        let ctx = TurnContext {
            execution_id: Some(execution_id),
            ..ctx
        };
        let handle = tokio::spawn(ctx.consume(parser, rx, interrupt_rx, done));
        {
            let mut state = self.lock();
            state.consumers.retain(|h| !h.is_finished());
            state.consumers.push(handle);
        }
        self.session(&session_id).await
    }

    // ---- What each conversation has (ADR-044) ----------------------------------------------

    /// A number that stays the same while the AI tool of the session's conversation keeps all
    /// of it, and changes when it shortens its memory or a new conversation starts (ADR-044).
    /// Callers compare it to tell whether what they sent earlier is still in the conversation.
    /// `None` when that cannot be told: nothing was sent to it since Plenipo started, or its AI
    /// tool does not say when it shortens its memory.
    pub fn memory_mark(&self, session_id: &str) -> Option<u64> {
        self.lock()
            .conversations
            .get(session_id)
            .filter(|c| c.heard())
            .map(|c| c.mark)
    }

    /// What is known of the conversation a step is about to run in: nothing for a new provider
    /// conversation, or one the runtime has sent nothing to since Plenipo started. Keeps track
    /// of it from now on (`watched`: its AI tool says when it shortens its memory); returns the
    /// conversation's mark as the step launches.
    fn conversation_at_launch(
        &self,
        session_id: &str,
        request: &TurnRequest,
        watched: bool,
    ) -> (Option<Conversation>, u64) {
        let resumed = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { .. } => None,
        };
        let mut state = self.lock();
        let known = state
            .conversations
            .get(session_id)
            .filter(|c| resumed.is_some() && c.provider == resumed)
            .cloned();
        match known {
            Some(c) => {
                let mark = c.mark;
                (Some(c), mark)
            }
            None => {
                let mark = state.next_mark();
                state.conversations.insert(
                    session_id.to_owned(),
                    Conversation {
                        provider: resumed,
                        mark,
                        watched,
                        ..Conversation::default()
                    },
                );
                (None, mark)
            }
        }
    }

    /// The AI tool confirmed its conversation: another one than before starts over.
    fn conversation_bound(&self, session_id: &str, provider: &str) {
        let mut state = self.lock();
        let (another, watched) = match state.conversations.get_mut(session_id) {
            None => return,
            Some(c) => match c.provider.as_deref() {
                None => {
                    c.provider = Some(provider.to_owned());
                    (false, c.watched)
                }
                Some(p) => (p != provider, c.watched),
            },
        };
        if another {
            let mark = state.next_mark();
            state.conversations.insert(
                session_id.to_owned(),
                Conversation {
                    provider: Some(provider.to_owned()),
                    mark,
                    watched,
                    ..Conversation::default()
                },
            );
        }
    }

    /// The AI tool shortened its memory of the conversation: its next step gets the full
    /// instructions and the full permissions note (ADR-044 §2.5).
    fn memory_shortened(&self, session_id: &str) {
        let mut state = self.lock();
        let mark = state.next_mark();
        if let Some(c) = state.conversations.get_mut(session_id) {
            c.shortened = true;
            c.note = None;
            c.mark = mark;
        }
    }

    /// A step that launched at `mark` ended, having sent `sent`: the conversation has it when
    /// the step `finished`; otherwise what it sent in full is in doubt. Under the caller's lock,
    /// where it decides who ends the step (`complete` or [`Self::force_end`]).
    fn step_finished(
        state: &mut State,
        session_id: &str,
        mark: u64,
        sent: Delivery,
        finished: bool,
        context_used: Option<u64>,
    ) {
        let Some(c) = state.conversations.get_mut(session_id) else {
            return;
        };
        if context_used.is_some() {
            c.context_used = context_used;
        }
        if finished {
            c.delivered(mark, &sent);
        } else {
            c.not_delivered(mark, &sent);
        }
    }

    fn buffer(&self, activity: &AgentActivity) {
        let config = &self.inner.config;
        let mut state = self.lock();
        let state = &mut *state;
        if !state.activity.contains_key(&activity.task_id) {
            state.activity_order.push_back(activity.task_id.clone());
            while state.activity_order.len() > config.activity_turns {
                if let Some(old) = state.activity_order.pop_front() {
                    state.activity.remove(&old);
                }
            }
        }
        let buf = state.activity.entry(activity.task_id.clone()).or_default();
        // Coalesce streamed text, and streamed thinking, so a reload shows each as it was said
        // without keeping every fragment, and a long thought never pushes earlier steps out.
        // The joined piece takes the newest fragment's number and keeps its first one's time: a
        // chat rebuilt after a reload reads it as when the thinking (or the words) began, and the
        // next piece's time as when they ended ("Thought for 6 s").
        if let Some(last) = buf.back_mut() {
            let joined = match (&mut last.event, &activity.event) {
                (AgentEvent::TextDelta { text: so_far }, AgentEvent::TextDelta { text })
                | (AgentEvent::Reasoning { text: so_far }, AgentEvent::Reasoning { text })
                    if so_far.len() + text.len() <= 64 * 1024 =>
                {
                    so_far.push_str(text);
                    true
                }
                _ => false,
            };
            if joined {
                last.seq = activity.seq;
                return;
            }
        }
        if buf.len() == config.activity_per_turn {
            buf.pop_front();
        }
        buf.push_back(activity.clone());
    }
}

/// The provider session a turn runs in: the confirmed one, or a new one.
fn turn_request(
    session: &AgentSession,
    adapter: &dyn RuntimeAdapter,
    ready: &Ready,
) -> TurnRequest {
    TurnRequest {
        session: match (
            &session.provider_session_id,
            session.provider_session_confirmed,
        ) {
            (Some(id), true) => ProviderSession::Resume { id: id.clone() },
            // Unconfirmed: start the provider session (again). A fresh ID avoids reusing one a
            // failed attempt may have claimed.
            _ => ProviderSession::New {
                preassigned: adapter
                    .preassigns_session_id()
                    .then(|| uuid::Uuid::new_v4().to_string()),
            },
        },
        model: session.model.clone(),
        effort: session.effort,
        billing_confirmed: ready.billing_confirmed,
        tools: None,
        working_dir: PathBuf::from(&session.working_dir),
        cli_version: ready.cli_version.clone(),
    }
}

/// One line for a process's stdin.
fn input_line(mut line: String) -> Vec<u8> {
    line.push('\n');
    line.into_bytes()
}

/// What to launch for one step.
struct StepLaunch {
    task_id: String,
    number: u32,
    step: u32,
    /// The message, before Plenipo's tools note.
    message: StepMessage,
}

/// The first step's message: Liaison's brief, a prompt Core wrote (counted as Plenipo's own
/// text), or the objective alone.
fn first_message(input: TurnInput) -> StepMessage {
    match (input.brief, input.prompt) {
        (Some(brief), _) => StepMessage::Brief(brief),
        (None, Some(text)) => StepMessage::Plain { text, passed: 0 },
        (None, None) => StepMessage::Plain {
            passed: input.objective.len(),
            text: input.objective,
        },
    }
}

/// Why an AI tool could not be held ([`AgentRuntime::hold_if_free`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotFree {
    /// These tasks are using it.
    Tasks(Vec<String>),
    /// It is held already: its sign-in tab is open, or it is updating.
    Held(HoldFor),
}

/// How many holds of each kind one AI tool has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Holds {
    sign_in: u32,
    update: u32,
}

impl Holds {
    /// Why the tool is held: an update, when there is one.
    fn reason(self) -> HoldFor {
        if self.update > 0 {
            HoldFor::Update
        } else {
            HoldFor::SignIn
        }
    }

    fn count(&mut self, reason: HoldFor) -> &mut u32 {
        match reason {
            HoldFor::SignIn => &mut self.sign_in,
            // Stop all work holds every AI tool at once (`State::work_held`); counted with
            // updates if a single tool is ever held for it.
            HoldFor::Update | HoldFor::StopAll => &mut self.update,
        }
    }
}

/// While kept, no new task starts on one AI tool (ADR-058 §5, ADR-059 §4); dropping it lets
/// waiting tasks go.
pub struct RuntimeHold {
    runtime: AgentRuntime,
    runtime_id: String,
    reason: HoldFor,
}

impl RuntimeHold {
    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }
}

impl std::fmt::Debug for RuntimeHold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeHold")
            .field("runtime_id", &self.runtime_id)
            .field("reason", &self.reason)
            .finish()
    }
}

impl Drop for RuntimeHold {
    fn drop(&mut self) {
        self.runtime.release_hold(&self.runtime_id, self.reason);
    }
}

/// An AI tool's own program, found and allowed, with the environment its tasks get and
/// Plenipo's empty check folder ([`AgentRuntime::tool_program`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProgram {
    /// "Codex".
    pub label: String,
    pub executable: PathBuf,
    pub env: Vec<(String, String)>,
    pub dir: PathBuf,
}

/// What an AI tool's short check said ([`AgentRuntime::status_check`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusAnswer {
    /// The tool has a check, and it ran.
    pub checked: bool,
    pub models: Option<Vec<KnownModel>>,
    pub plan: Option<PlanReport>,
}

/// The tasks whose step runs on `runtime_id` (a task not yet numbered shows as "").
fn using(state: &State, runtime_id: &str) -> Vec<String> {
    state
        .active
        .values()
        .filter(|a| a.claim == Claim::Turn && a.runtime_id.as_deref() == Some(runtime_id))
        .map(|a| a.task_id.clone().unwrap_or_default())
        .collect()
}

/// Holds a session's slot until the turn is launched (or the attempt fails).
struct Reservation {
    runtime: AgentRuntime,
    session_id: String,
    /// Taken when a consumer takes over the slot.
    done: Option<watch::Sender<bool>>,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if let Some(done) = self.done.take() {
            self.runtime.lock().active.remove(&self.session_id);
            self.runtime.inner.turn_released.notify_waiters();
            let _ = done.send(true);
        }
    }
}

/// Per-step state owned by the task that consumes the step's output.
struct TurnContext {
    runtime: AgentRuntime,
    session: AgentSession,
    task_id: String,
    step: u32,
    execution_id: Option<String>,
    seq: u64,
    stored: u32,
    /// The step's grant of Plenipo's tools, closed when the step ends.
    grant: Option<String>,
    /// Writes to the process's stdin while a task that talks runs (ADR-015); dropped to
    /// close it.
    input: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// The size of what the step was sent (ADR-044), recorded with its result.
    size: PromptSize,
    /// The conversation's mark when the step launched: if it changed, the AI tool may have lost
    /// part of what the step was sent.
    mark: u64,
    /// What the conversation keeps once the step is done (ADR-044).
    delivery: Delivery,
    /// How much of its context the AI tool reported in use by the end of the step.
    context_used: Option<u64>,
    /// A paid step's charge (ADR-085): settled when the step ends.
    paid: Option<PaidStep>,
    /// The paid step's bill, once its program ended.
    paid_bill: Option<PaidBill>,
    /// The step's streamed words and thinking, held back so secrets are redacted as whole words
    /// (ADR-216).
    live: LiveText,
    /// Set when the runtime ended this step itself ([`AgentRuntime::force_end`]): its finisher
    /// then records and announces nothing.
    forced: Arc<AtomicBool>,
    /// What the runtime knows of the running step, for a stall and for its diagnostics; shared
    /// with [`Active`] so a stop can read it while the step is stuck.
    watch: Arc<Mutex<StepWatch>>,
}

/// What the runtime knows of a running step, for a stall and its diagnostics: when the AI tool
/// last said anything, what is open, and the kinds of its latest events. Never its words.
struct StepWatch {
    label: &'static str,
    /// The version the AI tool's check read.
    cli_version: Option<String>,
    /// The kinds of the latest events, newest last, with when they came ([`DIAGNOSTIC_EVENTS`]).
    recent: VecDeque<(&'static str, Instant)>,
    /// When the AI tool last wrote a line, on either stream (the step's start before that).
    last_heard: Instant,
    /// Its tool calls not answered yet: by the AI tool's IDs, and a count of calls without one.
    open_tools: std::collections::HashSet<String>,
    open_unnamed: u32,
    /// Its file requests Plenipo has not answered yet (ADR-027; one may wait for the owner).
    files_pending: usize,
    /// What its connection waits on, in the parser's words ([`TurnParser::waiting_on`]).
    waiting_on: Vec<String>,
    /// Lines read from the AI tool but not handled yet, at the last look.
    queued_lines: usize,
}

impl StepWatch {
    fn new(label: &'static str, cli_version: Option<String>) -> Self {
        Self {
            label,
            cli_version,
            recent: VecDeque::new(),
            last_heard: Instant::now(),
            open_tools: std::collections::HashSet::new(),
            open_unnamed: 0,
            files_pending: 0,
            waiting_on: Vec::new(),
            queued_lines: 0,
        }
    }

    /// A tool call or a file request of the AI tool is open: a long silence is then the tool's
    /// work, or the owner's decision, not the AI tool stalling.
    fn busy(&self) -> bool {
        !self.open_tools.is_empty() || self.open_unnamed > 0 || self.files_pending > 0
    }

    /// Note one of the step's events: its kind, and the tool calls it opens or answers.
    fn saw(&mut self, event: &AgentEvent) {
        if self.recent.len() == DIAGNOSTIC_EVENTS {
            self.recent.pop_front();
        }
        self.recent.push_back((event_kind(event), Instant::now()));
        match event {
            AgentEvent::ToolUse { id: Some(id), .. } => {
                self.open_tools.insert(id.clone());
            }
            AgentEvent::ToolUse { id: None, .. } => self.open_unnamed += 1,
            AgentEvent::ToolResult { id: Some(id), .. } => {
                self.open_tools.remove(id);
            }
            AgentEvent::ToolResult { id: None, .. } => {
                self.open_unnamed = self.open_unnamed.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// One line for Plenipo's makers on why the step was stopped or seems stuck (`why`): the AI
    /// tool and its version, whether its program still ran, how long it has been silent, what
    /// was open and what its connection waited on, the lines not yet read, and the kinds and
    /// ages of its latest events. Never anything the AI tool wrote.
    fn diagnostics(&self, why: &str, process: Option<bool>) -> String {
        let now = Instant::now();
        let age = |at: Instant| {
            let ms = now.saturating_duration_since(at).as_millis();
            if ms < 1000 {
                format!("{ms} ms")
            } else {
                format!("{} s", ms / 1000)
            }
        };
        let version = self.cli_version.as_deref().unwrap_or("version unknown");
        let process = match process {
            Some(true) => "still running",
            Some(false) => "ended",
            None => "not known",
        };
        let waiting = if self.waiting_on.is_empty() {
            "nothing it said".to_owned()
        } else {
            self.waiting_on.join("; ")
        };
        let recent = if self.recent.is_empty() {
            "none".to_owned()
        } else {
            self.recent
                .iter()
                .rev()
                .map(|(kind, at)| format!("{kind} {}", age(*at)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "Diagnostics for Plenipo's makers (no words from the worker): {why}. {} {version}; \
             its program {process}; last heard {} ago; open tool calls {}; file requests \
             waiting {}; it waited on: {waiting}; lines not yet read {}; latest events, newest \
             first: {recent}.",
            self.label,
            age(self.last_heard),
            self.open_tools.len() + self.open_unnamed as usize,
            self.files_pending,
            self.queued_lines,
        )
    }
}

/// A stretch of time in plain words: "10 minutes", "1 minute", "45 seconds", "a moment".
fn spoken(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0 => "a moment".into(),
        1 => "1 second".into(),
        2..=59 => format!("{s} seconds"),
        60..=119 => "1 minute".into(),
        _ => format!("{} minutes", s / 60),
    }
}

/// An event's kind as the Ledger and the screens name it (`textDelta`, `toolUse`, …): never
/// what it says.
fn event_kind(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::SessionStarted { .. } => "sessionStarted",
        AgentEvent::TextDelta { .. } => "textDelta",
        AgentEvent::Message { .. } => "message",
        AgentEvent::Reasoning { .. } => "reasoning",
        AgentEvent::ToolUse { .. } => "toolUse",
        AgentEvent::ToolResult { .. } => "toolResult",
        AgentEvent::Notice { .. } => "notice",
        AgentEvent::Usage { .. } => "usage",
        AgentEvent::MemoryShortened { .. } => "memoryShortened",
        AgentEvent::Plan { .. } => "plan",
        AgentEvent::Status { .. } => "status",
    }
}

/// A paid step's charge (ADR-085): what was set aside, and what the step may send.
struct PaidStep {
    gate: Arc<dyn PaidGate>,
    ticket: String,
    price: Price,
    limits: PaidLimits,
    /// The helper's first line of input: the key.
    key_line: String,
}

impl TurnContext {
    fn actor(&self) -> String {
        format!("agent:{}", self.session.runtime_id)
    }

    async fn consume(
        mut self,
        mut parser: Box<dyn TurnParser>,
        mut rx: mpsc::UnboundedReceiver<OutputLine>,
        mut interrupt: Option<mpsc::UnboundedReceiver<()>>,
        done: Option<watch::Sender<bool>>,
    ) {
        let mut stopping = false;
        // Answers to the files the AI tool asked Plenipo for (ADR-027), as they come.
        let (answers_tx, mut answers) = mpsc::unbounded_channel::<(u64, FileAnswer)>();
        // A long silence (the owner's report, 2026-10-05): the owner is told once per silence,
        // then the step is stopped as no longer answering. With a tool call or a file request
        // open, the silence is that work's (or the owner's decision): told later, and only the
        // turn's own time limit ends it.
        let (note_after, end_after) = (
            self.runtime.inner.config.stall_note,
            self.runtime.inner.config.stall_end,
        );
        let mut noted = false;
        let mut stalled: Option<String> = None;
        loop {
            let (heard, busy) = {
                let w = self.watch();
                (w.last_heard, w.busy())
            };
            let wake = match (noted, busy) {
                (false, false) => Some(heard + note_after),
                (false, true) => Some(heard + end_after),
                (true, false) => Some(heard + end_after),
                (true, true) => None,
            }
            .filter(|_| !stopping)
            .map(tokio::time::Instant::from_std);
            let parsed = tokio::select! {
                line = rx.recv() => {
                    let Some(line) = line else { break };
                    if stopping {
                        continue; // stopped by policy: drain, but record nothing more from this turn
                    }
                    if line.stream == OutputStream::Stderr {
                        parser.stderr(&line.text);
                        continue;
                    }
                    {
                        let mut w = self.watch();
                        w.last_heard = Instant::now();
                        w.queued_lines = rx.len();
                    }
                    noted = false;
                    parser.line(&line.text, line.truncated)
                }
                Some((id, answer)) = answers.recv() => {
                    {
                        let mut w = self.watch();
                        w.files_pending = w.files_pending.saturating_sub(1);
                    }
                    if stopping {
                        continue;
                    }
                    parser.file_answered(id, answer)
                }
                asked = async {
                    match interrupt.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if asked.is_some() {
                        // Cancel (ADR-015 §7): the task is asked to stop itself first.
                        let lines = parser.cancel();
                        self.write(lines);
                    }
                    interrupt = None;
                    continue;
                }
                () = async {
                    match wake {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.watch().waiting_on = parser.waiting_on();
                    if noted {
                        // Silent far too long with nothing open: stopped as no longer
                        // answering, with one row of diagnostics. Drained like a policy stop.
                        let diagnostics = self.stall_diagnostics(end_after);
                        self.diagnostic_row(diagnostics.clone()).await;
                        stalled = Some(diagnostics);
                        if let Some(id) = &self.execution_id {
                            stopping = true;
                            let supervisor = self.runtime.inner.supervisor.clone();
                            let id = id.clone();
                            tokio::spawn(async move {
                                let _ = supervisor.terminate(&id).await;
                            });
                        }
                    } else {
                        noted = true;
                        let silent = if busy { end_after } else { note_after };
                        self.stall_note(silent, end_after, busy).await;
                    }
                    continue;
                }
            };
            self.watch().waiting_on = parser.waiting_on();
            self.write(parsed.send);
            if parsed.close_input {
                self.input = None;
            }
            for request in parsed.files {
                self.watch().files_pending += 1;
                self.file_request(request, &answers_tx);
            }
            // Changes being written go to the tool provider, which checks them (ADR-055).
            if let (Some(grant), Some(provider)) = (&self.grant, self.runtime.tool_provider()) {
                for preview in parsed.previews {
                    provider.preview_write(grant, preview);
                }
            }
            if let Some(plan) = parsed.plan {
                self.runtime.plan_reported(&self.session.runtime_id, plan);
            }
            for event in parsed.events {
                self.event(event).await;
            }
            if let (Some(_), false, Some(id)) = (&parsed.stop, stopping, &self.execution_id) {
                stopping = true;
                // Keep draining output while the process tree is terminated.
                let supervisor = self.runtime.inner.supervisor.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    let _ = supervisor.terminate(&id).await;
                });
            }
        }
        let supervisor = &self.runtime.inner.supervisor;
        let end = match &self.execution_id {
            Some(id) => match supervisor.wait(id).await {
                Ok(record) => ProcessEnd {
                    state: record.state,
                    exit_code: record.exit_code,
                    started: record.pid.is_some(),
                    detail: record.detail.clone(),
                    duration_ms: record.ended_at.map(|e| e.saturating_sub(record.started_at)),
                },
                Err(e) => ProcessEnd {
                    state: crate::dto::ExecutionState::Failed,
                    exit_code: None,
                    started: false,
                    detail: Some(e.to_string()),
                    duration_ms: None,
                },
            },
            None => ProcessEnd {
                state: crate::dto::ExecutionState::Failed,
                exit_code: None,
                started: false,
                detail: None,
                duration_ms: None,
            },
        };
        self.input = None;
        let mut result = parser.finish(&end);
        // Stopped as no longer answering: said so in plain words, with its diagnostics.
        if let Some(diagnostics) = stalled {
            let label = self.watch().label;
            result.outcome = TurnOutcome::TimedOut;
            result.summary = format!(
                "Stopped: {label} stopped answering (no word for {}). Try again.",
                spoken(end_after)
            );
            result.error = Some(cap(&diagnostics, MAX_EVENT_TEXT));
        }
        self.context_used = parser.context_used();
        if let Some(step) = &self.paid {
            self.paid_bill = parser.paid_bill(&step.price, end.started);
        }
        self.complete(result, done).await;
    }

    /// The running step's watch ([`StepWatch`]).
    fn watch(&self) -> MutexGuard<'_, StepWatch> {
        self.watch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The diagnostics of a step stopped as no longer answering after `silence`.
    fn stall_diagnostics(&self, silence: Duration) -> String {
        let process = self
            .execution_id
            .as_ref()
            .and_then(|id| self.runtime.inner.supervisor.record(id))
            .map(|r| r.state == crate::dto::ExecutionState::Running);
        self.watch()
            .diagnostics(&format!("it said nothing for {}", spoken(silence)), process)
    }

    /// Tell the owner the AI tool has said nothing for `silence`, instead of "Writing" forever,
    /// and keep one row of diagnostics. `busy`: a tool call or a file request of its own is open.
    async fn stall_note(&mut self, silence: Duration, end_after: Duration, busy: bool) {
        let label = self.watch().label;
        let text = if busy {
            format!(
                "No word from {label} for {} while its tool call or file request is open. \
                 Press Stop to end it, or wait.",
                spoken(silence)
            )
        } else {
            format!(
                "No word from {label} for {}. Plenipo stops it after {} of silence; press Stop to \
                 end it now, or wait.",
                spoken(silence),
                spoken(end_after)
            )
        };
        self.event(AgentEvent::Status {
            phase: StatusPhase::Waiting,
            text,
        })
        .await;
        let diagnostics = self.stall_diagnostics(silence);
        self.diagnostic_row(diagnostics).await;
    }

    /// One row of diagnostics on the turn, shown live and kept in the Ledger whatever the step's
    /// activity cap: kinds, ages, and states only, never the AI tool's words.
    async fn diagnostic_row(&mut self, text: String) {
        let note = AgentEvent::Notice {
            level: NoticeLevel::Warning,
            text,
        };
        // Live text still held back goes on first, as before any notice (ADR-216).
        let filter = self.runtime.filter();
        self.flush_live(filter.as_ref());
        self.emit(note.clone());
        let (session_id, task_id, execution_id, actor, step) = (
            self.session.id.clone(),
            self.task_id.clone(),
            self.execution_id.clone(),
            self.actor(),
            self.step,
        );
        if let Err(e) = self
            .runtime
            .with_store(move |s| {
                let turn = TurnRef {
                    session_id: &session_id,
                    task_id: &task_id,
                    execution_id: execution_id.as_deref(),
                    step: Some(step),
                    actor: &actor,
                };
                s.record_activity(&turn, &note)
            })
            .await
        {
            self.runtime.notice(e.to_string());
        }
    }

    /// Carry out a file the AI tool asked for (ADR-027) through the step's grant, and send the
    /// answer back when it is ready: Guard may make it wait for the owner, and the task goes on
    /// meanwhile. A step without a grant has every file request refused.
    fn file_request(
        &self,
        request: FileRequest,
        answers: &mpsc::UnboundedSender<(u64, FileAnswer)>,
    ) {
        let FileRequest { id, access } = request;
        let answers = answers.clone();
        match (&self.grant, self.runtime.tool_provider()) {
            (Some(grant), Some(provider)) => {
                let pending = provider.file_access(grant, access);
                tokio::spawn(async move {
                    let _ = answers.send((id, pending.await));
                });
            }
            _ => {
                let _ = answers.send((
                    id,
                    Err("Not done: this worker has no permission to use files.".into()),
                ));
            }
        }
    }

    /// Write lines to the process's stdin (a task that talks, ADR-015).
    fn write(&self, lines: Vec<String>) {
        if let Some(input) = &self.input {
            for line in lines {
                let _ = input.send(input_line(line));
            }
        }
    }

    /// Send one event to the live screens, and keep it for a reload (the runtime's buffer).
    fn emit(&mut self, event: AgentEvent) {
        self.seq += 1;
        let activity = AgentActivity {
            session_id: self.session.id.clone(),
            task_id: self.task_id.clone(),
            seq: self.seq,
            ts: crate::now_ms(),
            event,
        };
        self.runtime.buffer(&activity);
        self.runtime
            .inner
            .sink
            .emit(AgentUpdate::Activity(activity));
    }

    /// Send on the live text still held back (ADR-216): before an event of another kind, and
    /// when the turn ends.
    fn flush_live(&mut self, filter: Option<&TextFilter>) {
        for kind in [LiveKind::Words, LiveKind::Thinking] {
            self.flush_kind(kind, filter);
        }
    }

    /// Send on one kind's held text.
    fn flush_kind(&mut self, kind: LiveKind, filter: Option<&TextFilter>) {
        let text = self.live.flush(kind, filter);
        if !text.is_empty() {
            self.emit(kind.event(text));
        }
    }

    async fn event(&mut self, event: AgentEvent) {
        self.watch().saw(&event);
        let runtime = self.runtime.clone();
        let filter = runtime.filter();
        // Live text is redacted as whole words (ADR-216): a streamed piece goes on only up to
        // its newest whitespace, with the word being written held back until the redactor can
        // see it whole; an event of any other kind sends the held text on first, so the order
        // is kept.
        let event = match event {
            AgentEvent::TextDelta { text } => {
                // Words after thinking: the held thinking goes on first, so the screen keeps
                // the order; and the other way round below.
                self.flush_kind(LiveKind::Thinking, filter.as_ref());
                let text = self.live.push(LiveKind::Words, &text, filter.as_ref());
                if text.is_empty() {
                    return;
                }
                AgentEvent::TextDelta { text }
            }
            AgentEvent::Reasoning { text } => {
                self.flush_kind(LiveKind::Words, filter.as_ref());
                let text = self.live.push(LiveKind::Thinking, &text, filter.as_ref());
                if text.is_empty() {
                    return;
                }
                AgentEvent::Reasoning { text }
            }
            other => {
                // A message, a tool call, a sign, and the like end the blocks of text: what is
                // held goes on first, so the screen keeps the order (`ends_blocks`). A wait for
                // the AI company, counts and plans pass by without ending one.
                if ends_blocks(&other) {
                    self.flush_live(filter.as_ref());
                }
                match &filter {
                    Some(f) => filtered(f, other),
                    None => other,
                }
            }
        };
        self.emit(event.clone());

        match &event {
            AgentEvent::SessionStarted {
                provider_session_id,
                model,
            } => {
                let mut changed = false;
                if let Some(id) = provider_session_id {
                    changed |= self.session.provider_session_id.as_ref() != Some(id)
                        || !self.session.provider_session_confirmed;
                    self.session.provider_session_id = Some(id.clone());
                    self.session.provider_session_confirmed = true;
                    runtime.conversation_bound(&self.session.id, id);
                }
                if let Some(m) = model {
                    changed |= self.session.model.as_ref() != Some(m);
                    self.session.model = Some(m.clone());
                }
                if let Some(id) = &self.execution_id {
                    let (pid, m) = (provider_session_id.clone(), model.clone());
                    let _ = runtime.inner.supervisor.annotate(id, |a| {
                        if pid.is_some() {
                            a.provider_session_id = pid;
                        }
                        if m.is_some() {
                            a.model = m;
                        }
                    });
                }
                if changed {
                    let saved = self.session.clone();
                    if let Err(e) = runtime
                        .with_store(move |s| s.save_session(&saved, SessionChange::Bound))
                        .await
                    {
                        runtime.notice(e.to_string());
                    }
                    runtime.emit_session(self.session.clone());
                }
            }
            AgentEvent::Usage { usage } => {
                if let Some(id) = &self.execution_id {
                    let usage = *usage;
                    let _ = runtime
                        .inner
                        .supervisor
                        .annotate(id, |a| a.usage = Some(usage));
                }
            }
            // The next step sends everything in full again (ADR-044 §2.5).
            AgentEvent::MemoryShortened { .. } => runtime.memory_shortened(&self.session.id),
            _ => {}
        }

        if event.ledger_type().is_none() {
            return;
        }
        let limit = runtime.inner.config.stored_activity_per_turn;
        let event = match self.stored.cmp(&limit) {
            std::cmp::Ordering::Less => capped(event),
            std::cmp::Ordering::Equal => AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: format!("Further activity for this turn is shown live but not recorded (limit {limit})."),
            },
            std::cmp::Ordering::Greater => return,
        };
        self.stored += 1;
        let (session_id, task_id, execution_id, actor, step) = (
            self.session.id.clone(),
            self.task_id.clone(),
            self.execution_id.clone(),
            self.actor(),
            self.step,
        );
        if let Err(e) = runtime
            .with_store(move |s| {
                let turn = TurnRef {
                    session_id: &session_id,
                    task_id: &task_id,
                    execution_id: execution_id.as_deref(),
                    step: Some(step),
                    actor: &actor,
                };
                s.record_activity(&turn, &event)
            })
            .await
        {
            runtime.notice(e.to_string());
        }
    }

    async fn complete(mut self, result: TurnResult, done: Option<watch::Sender<bool>>) {
        let runtime = self.runtime.clone();
        // Live text still held back goes on before the turn is recorded (ADR-216).
        let filter = runtime.filter();
        self.flush_live(filter.as_ref());
        // The step's program has ended: its grant ends before anything else is recorded (a
        // pending approval would otherwise hold its task).
        if let Some(grant) = self.grant.take() {
            runtime.close_tools(grant).await;
        }
        // A paid step's spending is settled next (ADR-085): what it cost, not priced yet, or
        // not sent. A bill that passes a cap is recorded, and the owner is told.
        if let Some(step) = self.paid.take() {
            let bill = self.paid_bill.take().unwrap_or(PaidBill::NotSent);
            let _passed =
                tokio::task::spawn_blocking(move || step.gate.settle(&step.ticket, &bill))
                    .await
                    .unwrap_or_default();
        }
        let mut result = match runtime.filter() {
            Some(f) => filtered_result(&f, result),
            None => result,
        };
        result.prompt = Some(self.size);
        // Only a finished step counts as delivered: a failed one may or may not have reached
        // the AI tool, so what it sent in full goes out in full again. Unless the runtime ended
        // the step meanwhile (`force_end`), which recorded it as not delivered: looked at under
        // the same lock, so one of the two records it, never both.
        {
            let mut state = runtime.lock();
            if !self.forced.load(Ordering::SeqCst) {
                AgentRuntime::step_finished(
                    &mut state,
                    &self.session.id,
                    self.mark,
                    self.delivery,
                    result.outcome == TurnOutcome::Completed,
                    self.context_used,
                );
            }
        }
        if let Some(id) = &self.execution_id {
            let (pid, model, usage) = (
                result.provider_session_id.clone(),
                result.model.clone(),
                result.usage,
            );
            let _ = runtime.inner.supervisor.annotate(id, |a| {
                if pid.is_some() {
                    a.provider_session_id = pid;
                }
                if model.is_some() {
                    a.model = model;
                }
                if usage.is_some() {
                    a.usage = usage;
                }
            });
        }
        // Ended by the runtime meanwhile (`force_end`): it recorded the turn and let the session
        // go. What is above (tools closed, spending settled) still had to happen; nothing more.
        if self.forced.load(Ordering::SeqCst) {
            if let Some(done) = done {
                let _ = done.send(true);
            }
            return;
        }
        // The hook (if any) decides whether the turn finishes or waits to continue.
        let disposition = match runtime.hook() {
            Some(hook) => {
                let memory_mark = runtime
                    .memory_mark(&self.session.id)
                    .filter(|m| *m == self.mark);
                let end = TurnEnd {
                    session: self.session.clone(),
                    task_id: self.task_id.clone(),
                    step: self.step,
                    execution_id: self.execution_id.clone(),
                    result: result.clone(),
                    memory_mark,
                };
                tokio::task::spawn_blocking(move || hook.turn_ended(&end))
                    .await
                    .unwrap_or(TurnDisposition::Finish)
            }
            None => TurnDisposition::Finish,
        };
        // The result as the screens get it, even if the Ledger refuses it (`emit_turn_with`).
        let ended = result.clone();
        let mut recorded = true;
        if disposition == TurnDisposition::Finish {
            // Nothing runs under the turn any more: it is recorded, then let go. A new turn of
            // the session waits for that instead of being refused ([`AgentRuntime::reserve_turn`]);
            // a continuation is still refused, since the claim stays until it is let go. Looked
            // at under the same lock as `force_end`: one of the two ends the step, never both.
            {
                let mut state = runtime.lock();
                if self.forced.load(Ordering::SeqCst) {
                    drop(state);
                    if let Some(done) = done {
                        let _ = done.send(true);
                    }
                    return;
                }
                if let Some(active) = state.active.get_mut(&self.session.id) {
                    active.releasing = true;
                }
            }
            let (session_id, task_id, execution_id, actor, step, stored) = (
                self.session.id.clone(),
                self.task_id.clone(),
                self.execution_id.clone(),
                self.actor(),
                self.step,
                result,
            );
            if let Err(e) = runtime
                .with_store(move |s| {
                    let turn = TurnRef {
                        session_id: &session_id,
                        task_id: &task_id,
                        execution_id: execution_id.as_deref(),
                        step: Some(step),
                        actor: &actor,
                    };
                    s.finish_turn(&turn, &stored)
                })
                .await
            {
                runtime.notice(e.to_string());
                recorded = false;
            }
        }
        self.session.updated_at = crate::now_ms();
        let saved = self.session.clone();
        let _ = runtime
            .with_store(move |s| s.save_session(&saved, SessionChange::Touched))
            .await;

        {
            let mut state = runtime.lock();
            // Ended by the runtime while the hook decided (`force_end`): the session is no longer
            // this step's to change.
            if self.forced.load(Ordering::SeqCst) {
                drop(state);
                if let Some(done) = done {
                    let _ = done.send(true);
                }
                return;
            }
            let ours = state
                .active
                .get(&self.session.id)
                .is_some_and(|a| a.task_id.as_deref() == Some(self.task_id.as_str()));
            match &disposition {
                TurnDisposition::Finish if ours => {
                    state.active.remove(&self.session.id);
                }
                TurnDisposition::Suspended { .. } if ours => {
                    if let Some(active) = state.active.get_mut(&self.session.id) {
                        active.claim = Claim::Wait;
                        active.execution_id = None;
                        active.done = finished();
                        active.grant = None;
                        active.sent = None;
                        active.forced = None;
                        active.watch = None;
                    }
                }
                _ => {}
            }
        }
        if disposition == TurnDisposition::Finish {
            runtime.inner.turn_released.notify_waiters();
        }
        if let Some(done) = done {
            let _ = done.send(true);
        }
        let shown = (!recorded).then_some(&ended);
        runtime
            .emit_turn_with(&self.session.id, &self.task_id, shown)
            .await;
        runtime.emit_session(self.session);
        runtime.released();
    }
}

/// `event` with secrets hidden in its text. Streamed words and thinking (`TextDelta`,
/// `Reasoning`) are not here: they are redacted as whole words on their way in (`LiveText`,
/// ADR-216), since a pattern needs a whole token and a piece may hold half of one.
fn filtered(f: &TextFilter, event: AgentEvent) -> AgentEvent {
    match event {
        AgentEvent::Message { text } => AgentEvent::Message {
            text: f.redact(&text),
        },
        AgentEvent::ToolUse { tool, summary, id } => AgentEvent::ToolUse {
            tool,
            summary: f.redact(&summary),
            id,
        },
        AgentEvent::ToolResult {
            tool,
            is_error,
            summary,
            id,
        } => AgentEvent::ToolResult {
            tool,
            is_error,
            summary: f.redact(&summary),
            id,
        },
        AgentEvent::Status { phase, text } => AgentEvent::Status {
            phase,
            text: f.redact(&text),
        },
        AgentEvent::Notice { level, text } => AgentEvent::Notice {
            level,
            text: f.redact(&text),
        },
        AgentEvent::MemoryShortened { detail } => AgentEvent::MemoryShortened {
            detail: f.redact(&detail),
        },
        other => other,
    }
}

/// `result` with secrets hidden in its text.
fn filtered_result(f: &TextFilter, mut result: TurnResult) -> TurnResult {
    result.summary = f.redact(&result.summary);
    result.text = result.text.map(|t| f.redact(&t));
    result.error = result.error.map(|e| f.redact(&e));
    result
}

/// A result recorded without a step of its own.
fn administrative(outcome: TurnOutcome, summary: &str, error: Option<String>) -> TurnResult {
    TurnResult {
        outcome,
        summary: summary.into(),
        text: None,
        error: error.map(|e| cap(&e, MAX_EVENT_TEXT)),
        provider_session_id: None,
        model: None,
        usage: None,
        duration_ms: None,
        ignored_lines: 0,
        prompt: None,
    }
}

/// Stored activity is capped; live activity is not.
fn capped(event: AgentEvent) -> AgentEvent {
    match event {
        AgentEvent::Message { text } => AgentEvent::Message {
            text: cap(&text, MAX_EVENT_TEXT),
        },
        AgentEvent::Notice { level, text } => AgentEvent::Notice {
            level,
            text: cap(&text, MAX_EVENT_TEXT),
        },
        AgentEvent::MemoryShortened { detail } => AgentEvent::MemoryShortened {
            detail: cap(&detail, MAX_EVENT_TEXT),
        },
        other => other,
    }
}

fn checking(adapter: &dyn RuntimeAdapter) -> AgentRuntimeInfo {
    AgentRuntimeInfo {
        id: adapter.id().into(),
        label: adapter.label().into(),
        provider: adapter.provider().into(),
        provider_label: adapter.provider_label().into(),
        installation: Installation {
            state: InstallState::Checking,
            executable: None,
            version: None,
            detail: None,
        },
        auth: AuthStatus {
            state: AuthState::Checking,
            method: None,
            detail: None,
        },
        capabilities: adapter.capabilities(),
        install_hint: adapter.install_hint().into(),
        login_hint: adapter.login_hint().into(),
        ready: false,
        checked_at: None,
        checked_version: adapter.checked_version().into(),
        account: AccountCommands {
            sign_in: account_words(adapter, AccountAction::SignIn),
            sign_out: account_words(adapter, AccountAction::SignOut),
        },
        reported_models: None,
        held: None,
        uses_tools: adapter.accepts_tools(),
    }
}

/// The AI tool's own sign-in or sign-out command as the owner would type it (`codex login`).
fn account_words(adapter: &dyn RuntimeAdapter, action: AccountAction) -> Option<String> {
    adapter.account_command(action).map(|args| {
        [vec![adapter.executable_name().to_owned()], args]
            .concat()
            .join(" ")
    })
}

/// Sign-in states a turn may run with. A runtime that re-checks billing during every turn
/// may also run when the method could not be confirmed up front (ADR-007 §4).
fn auth_allowed(adapter: &dyn RuntimeAdapter, state: AuthState) -> bool {
    // A paid AI tool runs only with its key, within the spending caps (ADR-085), whatever its
    // check says: never as a subscription, so the Router never takes it for one.
    match state {
        AuthState::PaidKey => adapter.paid(),
        _ if adapter.paid() => false,
        AuthState::Subscription => true,
        AuthState::Unverified | AuthState::Unknown => {
            adapter.capabilities().billing_checked_per_turn
        }
        _ => false,
    }
}

/// The outcome to record for work refused because a runtime is not usable.
pub fn unavailable_outcome(info: &AgentRuntimeInfo) -> TurnOutcome {
    if info.installation.state != InstallState::Installed {
        return TurnOutcome::ProviderUnavailable;
    }
    match info.auth.state {
        AuthState::SignedOut | AuthState::Unverified | AuthState::Unknown => {
            TurnOutcome::AuthRequired
        }
        AuthState::ApiKey | AuthState::ThirdPartyCloud => TurnOutcome::BillingNotAllowed,
        AuthState::Checking | AuthState::Subscription | AuthState::PaidKey => {
            TurnOutcome::ProviderUnavailable
        }
    }
}

fn not_ready_reason(adapter: &dyn RuntimeAdapter, info: &AgentRuntimeInfo) -> String {
    let label = adapter.label();
    match info.installation.state {
        // Installed, but given no tasks (ADR-059 §6): the only case with a detail.
        InstallState::Installed if info.installation.detail.is_some() => {
            let detail = info.installation.detail.clone().unwrap_or_default();
            return format!("Plenipo is not giving {label} tasks for now: {detail}");
        }
        InstallState::Installed => {}
        InstallState::Checking => return format!("{label} is still being checked."),
        _ => {
            let detail = info.installation.detail.clone().unwrap_or_default();
            return format!(
                "{label} is not available. {detail} {}",
                adapter.install_hint()
            )
            .trim()
            .to_owned();
        }
    }
    match info.auth.state {
        // A paid AI tool: paid keys are off, no key is saved, or the key was refused (ADR-085).
        AuthState::SignedOut if adapter.paid() => format!(
            "{label} cannot take work now. {} {}",
            info.auth.detail.clone().unwrap_or_default(),
            adapter.login_hint()
        ),
        AuthState::SignedOut => format!("{label} is not signed in. {}", adapter.login_hint()),
        AuthState::ApiKey => format!(
            "{label} is signed in with its own API key, which would skip your spending caps, \
             so Plenipo does not use it. Sign in with your subscription instead; to pay per use, \
             save the key in the key box on {label}'s card (AI tools). {}",
            adapter.login_hint()
        ),
        AuthState::ThirdPartyCloud => format!(
            "{label} is configured for a third-party cloud provider, which Plenipo does not use. {}",
            adapter.login_hint()
        ),
        // The check said why (GitHub Copilot's paid extra use, ADR-083): its reason first, then
        // how to sign in, for the tools whose fix that is.
        AuthState::Unverified if info.auth.detail.is_some() => format!(
            "{label} cannot take work now. {} {}",
            info.auth.detail.clone().unwrap_or_default(),
            adapter.login_hint()
        ),
        _ => format!(
            "Plenipo could not confirm that {label} is signed in with a subscription. {}",
            adapter.login_hint()
        ),
    }
}

fn probe_failure(what: &str, out: &crate::agent::adapter::ProbeOutput) -> String {
    if let Some(e) = &out.spawn_error {
        return format!("The {what} could not start: {e}");
    }
    if out.timed_out {
        return format!("The {what} did not finish in time.");
    }
    let why = first_line(&out.combined(), 200);
    match out.exit_code {
        Some(code) if why.is_empty() => format!("The {what} exited with code {code}."),
        Some(code) => format!("The {what} exited with code {code}: {why}"),
        None => format!("The {what} ended abnormally."),
    }
}

pub fn validate_objective(objective: &str) -> Result<String, RuntimeError> {
    let trimmed = objective.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidInput(
            "the objective must not be empty".into(),
        ));
    }
    if trimmed.chars().count() > MAX_OBJECTIVE_CHARS {
        return Err(RuntimeError::InvalidInput(format!(
            "the objective is longer than {MAX_OBJECTIVE_CHARS} characters"
        )));
    }
    if trimmed.contains('\0') {
        return Err(RuntimeError::InvalidInput(
            "the objective contains a NUL character".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// A prompt Core sends on stdin: non-empty, at most [`MAX_PROMPT_BYTES`], no NUL.
pub fn validate_prompt(prompt: &str) -> Result<String, RuntimeError> {
    if prompt.trim().is_empty() {
        return Err(RuntimeError::InvalidInput(
            "the prompt must not be empty".into(),
        ));
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(RuntimeError::InvalidInput(format!(
            "the prompt is longer than {MAX_PROMPT_BYTES} bytes"
        )));
    }
    if prompt.contains('\0') {
        return Err(RuntimeError::InvalidInput(
            "the prompt contains a NUL character".into(),
        ));
    }
    Ok(prompt.to_owned())
}

/// A brief's messages are prompts, and what they pass along is part of them.
fn validate_brief(brief: BriefInput) -> Result<BriefInput, RuntimeError> {
    validate_prompt(&brief.full)?;
    let over = |passed: usize, text: &str| passed > text.len();
    let mut too_much = over(brief.passed_bytes, &brief.full);
    if let Some(reminder) = &brief.reminder {
        validate_prompt(reminder)?;
        too_much |= over(brief.reminder_passed_bytes, reminder);
    }
    if too_much {
        return Err(RuntimeError::InvalidInput(
            "a brief cannot pass along more than it holds".into(),
        ));
    }
    Ok(brief)
}

fn validate_input(input: TurnInput) -> Result<TurnInput, RuntimeError> {
    let objective = validate_objective(&input.objective)?;
    let prompt = input.prompt.as_deref().map(validate_prompt).transpose()?;
    let brief = input.brief.map(validate_brief).transpose()?;
    let task = match input.task {
        TurnTask::New {
            requested_by,
            metadata,
            project_id,
        } => {
            let requested_by = requested_by.trim().to_owned();
            if requested_by.is_empty() || requested_by.len() > 200 {
                return Err(RuntimeError::InvalidInput(
                    "requested_by must be 1–200 characters".into(),
                ));
            }
            let project_ok = |id: &String| {
                (1..=64).contains(&id.len())
                    && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            };
            if project_id.as_ref().is_some_and(|id| !project_ok(id)) {
                return Err(RuntimeError::InvalidInput("invalid project id".into()));
            }
            TurnTask::New {
                requested_by,
                metadata: object_or_empty("turn metadata", metadata)?,
                project_id,
            }
        }
        TurnTask::Existing { task_id } => {
            if task_id.is_empty() || task_id.len() > 64 {
                return Err(RuntimeError::InvalidInput("invalid task id".into()));
            }
            TurnTask::Existing { task_id }
        }
    };
    Ok(TurnInput {
        objective,
        prompt,
        brief,
        task,
    })
}

/// A JSON object (null counts as empty).
fn object_or_empty(
    what: &str,
    value: serde_json::Value,
) -> Result<serde_json::Value, RuntimeError> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(RuntimeError::InvalidInput(format!(
            "{what} must be a JSON object"
        ))),
    }
}

/// A Plenipo session ID chosen by Core: a lowercase UUID.
fn validate_session_id(id: &str) -> Result<String, RuntimeError> {
    let ok = id.len() == 36
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c) || c == '-');
    if ok {
        Ok(id.to_owned())
    } else {
        Err(RuntimeError::InvalidInput(format!(
            "invalid session id: {id:?}"
        )))
    }
}

/// `[A-Za-z0-9][A-Za-z0-9._:\[\]-]*`, optionally after a provider `[A-Za-z0-9][A-Za-z0-9._-]*`
/// and one `/` (Kimi's `kimi-code/k3`, ADR-027), at most 64 characters — a model name, never a
/// flag or a path.
pub fn validate_model(model: &str) -> Result<String, RuntimeError> {
    let part = |p: &str, more: &[char]| {
        let mut chars = p.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && chars.all(|c| c.is_ascii_alphanumeric() || "._-".contains(c) || more.contains(&c))
    };
    let name = |p: &str| part(p, &[':', '[', ']']);
    let ok = (1..=64).contains(&model.len())
        && match model.split_once('/') {
            None => name(model),
            // The provider: letters, digits, `.`, `_`, `-` only (never a drive such as `C:`).
            Some((provider, rest)) => part(provider, &[]) && name(rest),
        };
    if ok {
        Ok(model.to_owned())
    } else {
        Err(RuntimeError::InvalidInput(format!(
            "invalid model name: {model:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `session` reads the session's claim once for its summary and every turn: whatever that
    /// claim is, the summary never says the session is free while one of its turns reads as
    /// running (or waiting while it reads otherwise).
    #[test]
    fn the_summary_and_the_turns_agree_on_one_look_at_the_claim() {
        let session = AgentSession {
            id: "s".into(),
            runtime_id: "codex".into(),
            provider: "openai".into(),
            provider_session_id: None,
            provider_session_confirmed: false,
            model: None,
            effort: None,
            title: "t".into(),
            state: SessionState::Open,
            working_dir: ".".into(),
            created_at: 1,
            updated_at: 1,
            turn_count: 1,
            active_task_id: None,
            waiting_task_id: None,
            metadata: serde_json::Value::Null,
        };
        let turn = AgentTurn {
            task_id: "t1".into(),
            session_id: "s".into(),
            number: 1,
            objective: "o".into(),
            execution_id: None,
            running: false,
            waiting: false,
            result: None,
            steps: Vec::new(),
            started_at: 1,
            ended_at: None,
        };
        let claim = |claim: Claim, task: Option<&str>, execution: Option<&str>| Active {
            claim,
            runtime_id: None,
            task_id: task.map(str::to_owned),
            execution_id: execution.map(str::to_owned),
            step: 1,
            step_started_at: 1,
            done: finished(),
            interrupt: None,
            waiting_for_hold: false,
            stop_waiting: false,
            releasing: false,
            grant: None,
            sent: None,
            forced: None,
            watch: None,
        };
        let claims = [
            None,
            Some(claim(Claim::Turn, None, None)),
            Some(claim(Claim::Turn, Some("t1"), None)),
            Some(claim(Claim::Turn, Some("t1"), Some("e1"))),
            Some(claim(Claim::Turn, Some("t2"), Some("e2"))),
            Some(claim(Claim::Wait, Some("t1"), None)),
            Some(claim(Claim::Close, Some("t1"), None)),
        ];
        let (mut running, mut waiting) = (0, 0);
        for active in &claims {
            let s = AgentRuntime::with_claim(session.clone(), active.as_ref());
            let t = AgentRuntime::decorated(turn.clone(), active.as_ref());
            let what = active.as_ref().map(|a| (a.claim, a.task_id.clone()));
            if t.running {
                running += 1;
                assert_eq!(s.active_task_id.as_deref(), Some("t1"), "{what:?}");
            }
            if t.waiting {
                waiting += 1;
                assert_eq!(s.waiting_task_id.as_deref(), Some("t1"), "{what:?}");
            }
            if s.active_task_id.as_deref() == Some("t1") {
                assert!(!t.waiting, "{what:?}");
            }
        }
        assert!(running >= 2 && waiting == 1, "the claims cover each state");
    }

    #[test]
    fn objectives() {
        assert_eq!(validate_objective("  hi \n").unwrap(), "hi");
        assert!(validate_objective(" \n ").is_err());
        assert!(validate_objective("a\0b").is_err());
        assert!(validate_objective(&"x".repeat(MAX_OBJECTIVE_CHARS)).is_ok());
        assert!(validate_objective(&"x".repeat(MAX_OBJECTIVE_CHARS + 1)).is_err());
    }

    #[test]
    fn prompts() {
        assert_eq!(
            validate_prompt("  keep\n spacing ").unwrap(),
            "  keep\n spacing "
        );
        assert!(validate_prompt(" \n").is_err());
        assert!(validate_prompt("a\0b").is_err());
        assert!(validate_prompt(&"x".repeat(MAX_PROMPT_BYTES)).is_ok());
        assert!(validate_prompt(&"x".repeat(MAX_PROMPT_BYTES + 1)).is_err());
    }

    #[test]
    fn turn_inputs() {
        let ok = validate_input(TurnInput::owner(" do it ")).unwrap();
        assert_eq!(ok.objective, "do it");
        assert_eq!(
            ok.task,
            TurnTask::New {
                requested_by: OWNER.into(),
                metadata: serde_json::json!({}),
                project_id: None,
            }
        );
        for bad in [
            TurnInput {
                prompt: Some(String::new()),
                ..TurnInput::owner("x")
            },
            TurnInput {
                task: TurnTask::New {
                    requested_by: " ".into(),
                    metadata: serde_json::Value::Null,
                    project_id: None,
                },
                ..TurnInput::owner("x")
            },
            TurnInput {
                task: TurnTask::New {
                    requested_by: "owner".into(),
                    metadata: serde_json::json!([1]),
                    project_id: None,
                },
                ..TurnInput::owner("x")
            },
            TurnInput {
                task: TurnTask::New {
                    requested_by: "owner".into(),
                    metadata: serde_json::Value::Null,
                    project_id: Some("../etc".into()),
                },
                ..TurnInput::owner("x")
            },
            TurnInput {
                task: TurnTask::Existing {
                    task_id: String::new(),
                },
                ..TurnInput::owner("x")
            },
        ] {
            assert!(validate_input(bad.clone()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn session_ids() {
        assert!(validate_session_id("0f8fad5b-d9cb-469f-a165-70867728950e").is_ok());
        for bad in [
            "",
            "0F8FAD5B-D9CB-469F-A165-70867728950E",
            "../../../../etc/passwd/xxxxxxxxxxxxx",
            "0f8fad5b-d9cb-469f-a165-70867728950",
        ] {
            assert!(validate_session_id(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn models() {
        for good in [
            "sonnet",
            "opus[1m]",
            "gpt-5.1-codex",
            "claude-fable-5",
            "org:model_1",
            "kimi-code/k3",
            "kimi-code/kimi-for-coding-highspeed",
        ] {
            assert!(validate_model(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "--dangerously-skip-permissions",
            "-m",
            "a b",
            "../x",
            "./x",
            "/x",
            "a/",
            "a//b",
            "a/b/c",
            "a/../b",
            "a/-m",
            "a\\b",
            "C:/x",
            "m;rm",
            &"a".repeat(65),
        ] {
            assert!(validate_model(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_refusal_says_the_checks_own_reason_when_it_gave_one() {
        let mut info = checking(&crate::agent::copilot::Copilot);
        info.installation.state = InstallState::Installed;
        info.auth.state = AuthState::Unverified;
        info.auth.detail = Some("GitHub may charge for extra use.".into());
        let why = not_ready_reason(&crate::agent::copilot::Copilot, &info);
        assert!(
            why.starts_with(
                "GitHub Copilot cannot take work now. GitHub may charge for extra use."
            ),
            "{why}"
        );
        // The way to sign in follows, for a tool whose fix that is.
        let mut codex = checking(&crate::agent::codex::Codex);
        codex.installation.state = InstallState::Installed;
        codex.auth.state = AuthState::Unverified;
        codex.auth.detail = Some("Signed in, but the billing method was not recognized.".into());
        let why = not_ready_reason(&crate::agent::codex::Codex, &codex);
        assert!(
            why.ends_with(crate::agent::codex::Codex.login_hint()),
            "{why}"
        );
        info.auth.detail = None;
        let why = not_ready_reason(&crate::agent::copilot::Copilot, &info);
        assert!(why.starts_with("Plenipo could not confirm"), "{why}");
    }

    #[test]
    fn unavailable_outcomes() {
        let mut info = checking(&crate::agent::codex::Codex);
        info.installation.state = InstallState::NotInstalled;
        assert_eq!(unavailable_outcome(&info), TurnOutcome::ProviderUnavailable);
        info.installation.state = InstallState::Installed;
        for (auth, want) in [
            (AuthState::SignedOut, TurnOutcome::AuthRequired),
            (AuthState::ApiKey, TurnOutcome::BillingNotAllowed),
            (AuthState::ThirdPartyCloud, TurnOutcome::BillingNotAllowed),
            (AuthState::Unknown, TurnOutcome::AuthRequired),
        ] {
            info.auth.state = auth;
            assert_eq!(unavailable_outcome(&info), want, "{auth:?}");
        }
    }
}
