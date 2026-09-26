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
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::agent::adapter::{
    cap, first_line, ProcessEnd, ProviderSession, RuntimeAdapter, TurnParser, TurnRequest,
    MAX_EVENT_TEXT,
};
use crate::agent::discovery::{locate, run_probe, runtime_env, HostEnv, Located};
use crate::agent::dto::*;
use crate::dto::{AgentAttribution, OutputLine, OutputStream};
use crate::error::RuntimeError;
use crate::profile::LaunchSpec;
use crate::supervisor::Supervisor;

/// Longest objective accepted (characters).
pub const MAX_OBJECTIVE_CHARS: usize = 10_000;
/// Longest prompt sent to a runtime (bytes): an objective plus instructions added by Core.
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;
/// Activity numbering per step: step `n` numbers its activity from `(n - 1) * STEP_SEQ + 1`.
pub const STEP_SEQ: u64 = 1_000_000;
/// Actor recorded for turns the person using the app asked for.
pub const OWNER: &str = "owner";

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
    /// Record a new task. `metadata` (a JSON object, or null) is added to the turn's own.
    New {
        requested_by: String,
        metadata: serde_json::Value,
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
    /// instructions (e.g. Liaison's protocol) without changing what is recorded.
    pub prompt: Option<String>,
    pub task: TurnTask,
}

impl TurnInput {
    /// An objective the owner gives directly.
    pub fn owner(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            prompt: None,
            task: TurnTask::New {
                requested_by: OWNER.into(),
                metadata: serde_json::Value::Null,
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
}

/// A turn step's process ended (passed to the [`TurnHook`]).
#[derive(Debug, Clone)]
pub struct TurnEnd {
    pub session: AgentSession,
    pub task_id: String,
    pub step: u32,
    pub execution_id: Option<String>,
    pub result: TurnResult,
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
}

impl AgentConfig {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            turn_timeout: Duration::from_secs(30 * 60),
            probe_timeout: Duration::from_secs(20),
            max_active_turns: 4,
            max_line_bytes: 4 * 1024 * 1024,
            activity_per_turn: 500,
            activity_turns: 50,
            stored_activity_per_turn: 500,
            extra_env: Vec::new(),
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
    task_id: Option<String>,
    execution_id: Option<String>,
    /// The step running (or, while waiting, the last one run).
    step: u32,
    step_started_at: u64,
    done: watch::Receiver<bool>,
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
}

struct Inner {
    config: AgentConfig,
    adapters: Vec<Arc<dyn RuntimeAdapter>>,
    supervisor: Supervisor,
    store: Arc<dyn SessionStore>,
    sink: Arc<dyn AgentSink>,
    hook: RwLock<Option<Arc<dyn TurnHook>>>,
    host: HostEnv,
    state: Mutex<State>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct AgentRuntime {
    inner: Arc<Inner>,
}

/// A usable runtime, checked just before a turn.
struct Ready {
    executable: PathBuf,
    env: Vec<(String, String)>,
    /// The sign-in check confirmed a subscription.
    billing_confirmed: bool,
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
                host,
                state: Mutex::new(State {
                    runtimes,
                    ..State::default()
                }),
            }),
        };
        this.recover();
        this
    }

    /// Install the hook consulted when a turn step ends (at most one; replaces any earlier).
    pub fn set_hook(&self, hook: Arc<dyn TurnHook>) {
        *self.inner.hook.write().unwrap_or_else(|p| p.into_inner()) = Some(hook);
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
    fn with_active(&self, mut session: AgentSession) -> AgentSession {
        let state = self.lock();
        let active = state.active.get(&session.id);
        session.active_task_id = active
            .filter(|a| a.claim == Claim::Turn)
            .and_then(|a| a.task_id.clone());
        session.waiting_task_id = active
            .filter(|a| a.claim == Claim::Wait)
            .and_then(|a| a.task_id.clone());
        session
    }

    /// `turn` with the live state of its running step or wait, when it holds its session.
    fn decorate(&self, mut turn: AgentTurn) -> AgentTurn {
        let state = self.lock();
        let Some(active) = state
            .active
            .get(&turn.session_id)
            .filter(|a| a.task_id.as_deref() == Some(turn.task_id.as_str()))
        else {
            return turn;
        };
        match active.claim {
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
        let (s, t) = (session_id.to_owned(), task_id.to_owned());
        match self
            .with_store(move |store| Ok(store.turns(&s)?.into_iter().find(|x| x.task_id == t)))
            .await
        {
            Ok(Some(turn)) => {
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
        self.lock().runtimes.clone()
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

    fn store_info(&self, info: AgentRuntimeInfo) {
        let mut state = self.lock();
        if let Some(slot) = state.runtimes.iter_mut().find(|r| r.id == info.id) {
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
            detail: Some("Not checked: the runtime is not installed.".into()),
        };
        let executable = match locate(adapter, host) {
            Located::Found(path) => path,
            Located::NotFound => {
                info.installation = Installation {
                    state: InstallState::NotInstalled,
                    executable: None,
                    version: None,
                    detail: Some(format!(
                        "No `{}` executable was found on PATH or in the usual install locations.",
                        adapter.executable_name()
                    )),
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
        let mut env = runtime_env(adapter, host);
        env.extend(self.inner.config.extra_env.iter().cloned());
        let workdir = self.probe_dir();
        let timeout = self.inner.config.probe_timeout;

        let known_version = previous
            .filter(|p| p.installation.executable.as_deref() == Some(shown.as_str()))
            .and_then(|p| p.installation.version);
        let version = match (version, known_version) {
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
        let out = run_probe(&executable, &adapter.auth_args(), &env, &workdir, timeout).await;
        info.auth = adapter.parse_auth(&out);
        info.ready = auth_allowed(adapter, info.auth.state);
        let billing_confirmed = info.auth.state == AuthState::Subscription;
        let ready = info.ready.then_some(Ready {
            executable,
            env,
            billing_confirmed,
        });
        (info, ready)
    }

    fn probe_dir(&self) -> PathBuf {
        let dir = &self.inner.config.workspace_root;
        if std::fs::create_dir_all(dir).is_ok() {
            dir.clone()
        } else {
            std::env::temp_dir()
        }
    }

    /// Fresh check right before a turn. Refuses with an explanation when not usable.
    async fn preflight(&self, adapter: &dyn RuntimeAdapter) -> Result<Ready, NotReady> {
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

    // ---- Sessions -----------------------------------------------------------------------

    pub async fn overview(&self) -> Result<AgentOverview, RuntimeError> {
        let sessions = self.with_store(|s| s.sessions(200)).await?;
        let sessions = sessions.into_iter().map(|s| self.with_active(s)).collect();
        let state = self.lock();
        Ok(AgentOverview {
            runtimes: state.runtimes.clone(),
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
        let session = self.with_active(session);
        let turns: Vec<AgentTurn> = turns.into_iter().map(|t| self.decorate(t)).collect();
        let state = self.lock();
        let activity = turns
            .iter()
            .filter_map(|t| state.activity.get(&t.task_id))
            .flat_map(|buf| buf.iter().cloned())
            .collect();
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
            .preflight(adapter.as_ref())
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
        // or cannot start until this turn ends.
        let reservation = self.reserve(session_id, Claim::Turn)?;
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
                "The runtime {:?} is not available in this build.",
                session.runtime_id
            ))
        })?;
        let ready = self
            .preflight(adapter.as_ref())
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
                Claim::Close => return Err(not_waiting()),
            }
            if running >= self.inner.config.max_active_turns {
                return Err(RuntimeError::Busy(format!(
                    "{running} agent turns are already running; wait for one to finish."
                )));
            }
            let (tx, rx) = watch::channel(false);
            active.claim = Claim::Turn;
            active.execution_id = None;
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
                "The runtime {:?} is not available in this build.",
                session.runtime_id
            ));
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
        let ready = match self.preflight(adapter.as_ref()).await {
            Ok(ready) => ready,
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
        let request = turn_request(&session, adapter.as_ref(), ready.billing_confirmed);
        self.launch_step(
            session,
            adapter,
            ready,
            request,
            StepLaunch {
                task_id: task_id.to_owned(),
                number,
                step,
                prompt,
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
        enum Target {
            Running(String, watch::Receiver<bool>),
            Waiting(String),
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
            match active.claim {
                Claim::Wait => {
                    let task = active.task_id.clone().unwrap_or_default();
                    // Claim it for the cancel, so a continuation cannot start meanwhile.
                    if let Some(active) = state.active.get_mut(session_id) {
                        active.claim = Claim::Close;
                    }
                    Target::Waiting(task)
                }
                _ => {
                    let execution = active.execution_id.clone().ok_or_else(|| {
                        RuntimeError::NotReady(
                            "The turn is still starting; try again in a moment.".into(),
                        )
                    })?;
                    Target::Running(execution, active.done.clone())
                }
            }
        };
        match target {
            Target::Running(execution, mut done) => {
                self.inner.supervisor.cancel(&execution).await?;
                let _ = tokio::time::timeout(Duration::from_secs(15), done.wait_for(|d| *d)).await;
            }
            Target::Waiting(task_id) => {
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
        }
        self.session(session_id).await
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
        let mut state = self.lock();
        if state.shutting_down {
            return Err(RuntimeError::ShuttingDown);
        }
        if let Some(existing) = state.active.get(session_id) {
            return Err(RuntimeError::NotReady(match (existing.claim, claim) {
                (Claim::Close, _) => "This session is being closed.".into(),
                (Claim::Wait, _) => "This session's turn is waiting to continue (for example for \
                                     handoff replies). Cancel the turn to stop waiting."
                    .into(),
                (Claim::Turn, Claim::Close) => {
                    "A turn is running in this session. Cancel it first.".into()
                }
                (Claim::Turn, _) => {
                    "A turn is already running in this session. Wait for it or cancel it.".into()
                }
            }));
        }
        let turns = state
            .active
            .values()
            .filter(|a| a.claim == Claim::Turn)
            .count();
        if claim == Claim::Turn && turns >= self.inner.config.max_active_turns {
            return Err(RuntimeError::Busy(format!(
                "{turns} agent turns are already running; wait for one to finish."
            )));
        }
        let (done_tx, done_rx) = watch::channel(false);
        state.active.insert(
            session_id.to_owned(),
            Active {
                claim,
                task_id: None,
                execution_id: None,
                step: 1,
                step_started_at: crate::now_ms(),
                done: done_rx,
            },
        );
        Ok(Reservation {
            runtime: self.clone(),
            session_id: session_id.to_owned(),
            done: Some(done_tx),
        })
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
        let request = turn_request(&session, adapter.as_ref(), ready.billing_confirmed);
        let (s, recorded) = (session.clone(), input.clone());
        let task_id = self
            .with_store(move |store| store.begin_turn(&s, number, &recorded))
            .await?;
        session.turn_count = number;
        session.updated_at = crate::now_ms();
        if let Some(active) = self.lock().active.get_mut(&session.id) {
            active.task_id = Some(task_id.clone());
        }
        let prompt = input.prompt.unwrap_or(input.objective);
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
                prompt,
            },
            done,
        )
        .await
    }

    /// Launch one step of a turn whose session is claimed; the step's consumer releases or
    /// converts the claim when it ends.
    async fn launch_step(
        &self,
        session: AgentSession,
        adapter: Arc<dyn RuntimeAdapter>,
        ready: Ready,
        request: TurnRequest,
        launch: StepLaunch,
        done: Option<watch::Sender<bool>>,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let StepLaunch {
            task_id,
            number,
            step,
            prompt,
        } = launch;
        let session_id = session.id.clone();
        let (tx, rx) = mpsc::unbounded_channel::<OutputLine>();
        let provider_session = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { preassigned } => preassigned.clone(),
        };
        let label = if step == 1 {
            format!("{} · turn {number}", adapter.label())
        } else {
            format!("{} · turn {number} · step {step}", adapter.label())
        };
        let spec = LaunchSpec {
            profile_id: format!("agent.{}", adapter.id()),
            label,
            executable: ready.executable,
            args: adapter.turn_args(&request),
            env: ready.env,
            working_dir: PathBuf::from(&session.working_dir),
            max_runtime: self.inner.config.turn_timeout,
            stdin: Some(prompt.into_bytes()),
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
            })),
        };
        let parser = adapter.parser(&request);
        let ctx = TurnContext {
            runtime: self.clone(),
            session: session.clone(),
            task_id: task_id.clone(),
            step,
            execution_id: None,
            seq: u64::from(step.saturating_sub(1)) * STEP_SEQ,
            stored: 0,
        };
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
                };
                ctx.complete(result, done).await;
                return self.session(&session_id).await;
            }
        };
        if let Some(active) = self.lock().active.get_mut(&session.id) {
            active.execution_id = Some(execution_id.clone());
        }
        self.emit_turn(&session_id, &task_id).await;
        self.emit_session(session);
        let ctx = TurnContext {
            execution_id: Some(execution_id),
            ..ctx
        };
        let handle = tokio::spawn(ctx.consume(parser, rx, done));
        {
            let mut state = self.lock();
            state.consumers.retain(|h| !h.is_finished());
            state.consumers.push(handle);
        }
        self.session(&session_id).await
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
        // Coalesce streamed text so a reload shows it without keeping every fragment.
        if let (
            AgentEvent::TextDelta { text },
            Some(AgentActivity {
                event: AgentEvent::TextDelta { text: last },
                seq,
                ..
            }),
        ) = (&activity.event, buf.back_mut())
        {
            if last.len() + text.len() <= 64 * 1024 {
                last.push_str(text);
                *seq = activity.seq;
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
    billing_confirmed: bool,
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
        billing_confirmed,
    }
}

/// What to launch for one step.
struct StepLaunch {
    task_id: String,
    number: u32,
    step: u32,
    prompt: String,
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
}

impl TurnContext {
    fn actor(&self) -> String {
        format!("agent:{}", self.session.runtime_id)
    }

    async fn consume(
        mut self,
        mut parser: Box<dyn TurnParser>,
        mut rx: mpsc::UnboundedReceiver<OutputLine>,
        done: Option<watch::Sender<bool>>,
    ) {
        let mut stopping = false;
        while let Some(line) = rx.recv().await {
            if stopping {
                continue; // stopped by policy: drain, but record nothing more from this turn
            }
            if line.stream == OutputStream::Stderr {
                parser.stderr(&line.text);
                continue;
            }
            let parsed = parser.line(&line.text, line.truncated);
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
        let result = parser.finish(&end);
        self.complete(result, done).await;
    }

    async fn event(&mut self, event: AgentEvent) {
        let runtime = self.runtime.clone();
        self.seq += 1;
        let activity = AgentActivity {
            session_id: self.session.id.clone(),
            task_id: self.task_id.clone(),
            seq: self.seq,
            ts: crate::now_ms(),
            event: event.clone(),
        };
        runtime.buffer(&activity);
        runtime.inner.sink.emit(AgentUpdate::Activity(activity));

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
        // The hook (if any) decides whether the turn finishes or waits to continue.
        let disposition = match runtime.hook() {
            Some(hook) => {
                let end = TurnEnd {
                    session: self.session.clone(),
                    task_id: self.task_id.clone(),
                    step: self.step,
                    execution_id: self.execution_id.clone(),
                    result: result.clone(),
                };
                tokio::task::spawn_blocking(move || hook.turn_ended(&end))
                    .await
                    .unwrap_or(TurnDisposition::Finish)
            }
            None => TurnDisposition::Finish,
        };
        if disposition == TurnDisposition::Finish {
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
            }
        }
        self.session.updated_at = crate::now_ms();
        let saved = self.session.clone();
        let _ = runtime
            .with_store(move |s| s.save_session(&saved, SessionChange::Touched))
            .await;

        {
            let mut state = runtime.lock();
            match &disposition {
                TurnDisposition::Finish => {
                    state.active.remove(&self.session.id);
                }
                TurnDisposition::Suspended { .. } => {
                    if let Some(active) = state.active.get_mut(&self.session.id) {
                        active.claim = Claim::Wait;
                        active.execution_id = None;
                        active.done = finished();
                    }
                }
            }
        }
        if let Some(done) = done {
            let _ = done.send(true);
        }
        runtime.emit_turn(&self.session.id, &self.task_id).await;
        runtime.emit_session(self.session);
        runtime.released();
    }
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
    }
}

/// Sign-in states a turn may run with. A runtime that re-checks billing during every turn
/// may also run when the method could not be confirmed up front (ADR-007 §4).
fn auth_allowed(adapter: &dyn RuntimeAdapter, state: AuthState) -> bool {
    match state {
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
        AuthState::Checking | AuthState::Subscription => TurnOutcome::ProviderUnavailable,
    }
}

fn not_ready_reason(adapter: &dyn RuntimeAdapter, info: &AgentRuntimeInfo) -> String {
    let label = adapter.label();
    match info.installation.state {
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
        AuthState::SignedOut => format!("{label} is not signed in. {}", adapter.login_hint()),
        AuthState::ApiKey => format!(
            "{label} is signed in with an API key. Plenipo does not use API billing; sign in \
             with your subscription instead. {}",
            adapter.login_hint()
        ),
        AuthState::ThirdPartyCloud => format!(
            "{label} is configured for a third-party cloud provider, which Plenipo does not use. {}",
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

fn validate_input(input: TurnInput) -> Result<TurnInput, RuntimeError> {
    let objective = validate_objective(&input.objective)?;
    let prompt = input.prompt.as_deref().map(validate_prompt).transpose()?;
    let task = match input.task {
        TurnTask::New {
            requested_by,
            metadata,
        } => {
            let requested_by = requested_by.trim().to_owned();
            if requested_by.is_empty() || requested_by.len() > 200 {
                return Err(RuntimeError::InvalidInput(
                    "requested_by must be 1–200 characters".into(),
                ));
            }
            TurnTask::New {
                requested_by,
                metadata: object_or_empty("turn metadata", metadata)?,
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

/// `[A-Za-z0-9][A-Za-z0-9._:\[\]-]{0,63}` — a model name, never a flag or a path.
pub fn validate_model(model: &str) -> Result<String, RuntimeError> {
    let mut chars = model.chars();
    let ok = (1..=64).contains(&model.len())
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '[' | ']' | '-'));
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
                metadata: serde_json::json!({})
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
                },
                ..TurnInput::owner("x")
            },
            TurnInput {
                task: TurnTask::New {
                    requested_by: "owner".into(),
                    metadata: serde_json::json!([1]),
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
        ] {
            assert!(validate_model(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "--dangerously-skip-permissions",
            "-m",
            "a b",
            "../x",
            "a/b",
            "m;rm",
            &"a".repeat(65),
        ] {
            assert!(validate_model(bad).is_err(), "{bad}");
        }
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
