//! The agent session service: detects runtimes, runs turns as supervised executions,
//! normalizes their output, and records sessions and turns through a [`SessionStore`].
//!
//! Flow of one turn (ADR-007):
//!
//! 1. validate input; reserve the session (one active turn per session, a global cap);
//! 2. preflight: locate the CLI, allow it, check sign-in (refuse if not usable);
//! 3. record the turn's task; launch the adapter-built spec with the objective on stdin;
//! 4. parse every output line into normalized events → live updates + durable activity;
//! 5. when the process ends, normalize the result, record it, and release the session.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
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
    /// Record the turn's task (running) and return its ID.
    fn begin_turn(
        &self,
        session: &AgentSession,
        number: u32,
        objective: &str,
    ) -> Result<String, String>;
    fn record_activity(
        &self,
        session_id: &str,
        task_id: &str,
        execution_id: Option<&str>,
        event: &AgentEvent,
    ) -> Result<(), String>;
    /// Record the normalized result and finish the task.
    fn finish_turn(
        &self,
        session_id: &str,
        task_id: &str,
        execution_id: Option<&str>,
        result: &TurnResult,
    ) -> Result<(), String>;
    /// A session's turns, oldest first.
    fn turns(&self, session_id: &str) -> Result<Vec<AgentTurn>, String>;
    /// Turns still marked running (left behind when Plenipo stopped).
    fn unfinished_turns(&self) -> Result<Vec<AgentTurn>, String>;
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
    /// Activity events recorded in the Ledger per turn.
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

struct Active {
    task_id: Option<String>,
    execution_id: Option<String>,
    done: watch::Receiver<bool>,
}

#[derive(Default)]
struct State {
    runtimes: Vec<AgentRuntimeInfo>,
    /// Keyed by session ID: turns starting or running.
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
            let result = TurnResult {
                outcome: TurnOutcome::Interrupted,
                summary: "Plenipo stopped while this turn was running".into(),
                text: None,
                error: None,
                provider_session_id: None,
                model: None,
                usage: None,
                duration_ms: None,
                ignored_lines: 0,
            };
            if let Err(e) = store.finish_turn(
                &turn.session_id,
                &turn.task_id,
                turn.execution_id.as_deref(),
                &result,
            ) {
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

    /// `session` with the task ID of its running turn (if any).
    fn with_active(&self, mut session: AgentSession) -> AgentSession {
        session.active_task_id = self
            .lock()
            .active
            .get(&session.id)
            .and_then(|a| a.task_id.clone());
        session
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
        let ready = info.ready.then_some(Ready { executable, env });
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
    async fn preflight(&self, adapter: &dyn RuntimeAdapter) -> Result<Ready, RuntimeError> {
        let (info, ready) = self.detect(adapter, false).await;
        let changed = self
            .lock()
            .runtimes
            .iter()
            .find(|r| r.id == info.id)
            .is_none_or(|r| r.installation != info.installation || r.auth != info.auth);
        let explanation = not_ready_reason(adapter, &info);
        self.store_info(info);
        if changed {
            self.inner.sink.emit(AgentUpdate::Runtimes(RuntimesUpdate {
                runtimes: self.runtimes(),
            }));
        }
        ready.ok_or(RuntimeError::NotReady(explanation))
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
        let state = self.lock();
        let active = state.active.get(session_id);
        let turns = turns
            .into_iter()
            .map(|mut t| {
                if let Some(a) = active.filter(|a| a.task_id.as_deref() == Some(t.task_id.as_str()))
                {
                    t.running = true;
                    t.execution_id = t.execution_id.or_else(|| a.execution_id.clone());
                }
                t
            })
            .collect::<Vec<AgentTurn>>();
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

    /// startSession + submitTask.
    pub async fn start_session(
        &self,
        runtime_id: &str,
        objective: &str,
        model: Option<&str>,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let objective = validate_objective(objective)?;
        let model = model
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(validate_model)
            .transpose()?;
        let adapter = self
            .adapter(runtime_id)
            .ok_or_else(|| RuntimeError::InvalidInput(format!("unknown runtime: {runtime_id}")))?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let reservation = self.reserve(&session_id)?;
        let ready = self.preflight(adapter.as_ref()).await?;

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
            title: first_line(&objective, 80),
            state: SessionState::Open,
            working_dir: working_dir.display().to_string(),
            created_at: now,
            updated_at: now,
            turn_count: 0,
            active_task_id: None,
        };
        let opened = session.clone();
        self.with_store(move |s| s.open_session(&opened)).await?;
        self.inner.sink.emit(AgentUpdate::Session(session.clone()));
        self.run_turn(session, adapter, ready, objective, reservation)
            .await
    }

    /// resumeSession + submitTask.
    pub async fn resume_session(
        &self,
        session_id: &str,
        objective: &str,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let objective = validate_objective(objective)?;
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
        let reservation = self.reserve(session_id)?;
        let ready = self.preflight(adapter.as_ref()).await?;
        self.run_turn(session, adapter, ready, objective, reservation)
            .await
    }

    /// cancelExecution: stop the session's running turn and wait until it is recorded.
    pub async fn cancel_turn(&self, session_id: &str) -> Result<AgentSessionDetail, RuntimeError> {
        let (execution, mut done) = {
            let state = self.lock();
            let active = state.active.get(session_id).ok_or_else(|| {
                RuntimeError::NotReady("No turn is running in this session.".into())
            })?;
            let execution = active.execution_id.clone().ok_or_else(|| {
                RuntimeError::NotReady("The turn is still starting; try again in a moment.".into())
            })?;
            (execution, active.done.clone())
        };
        self.inner.supervisor.cancel(&execution).await?;
        let _ = tokio::time::timeout(Duration::from_secs(15), done.wait_for(|d| *d)).await;
        self.session(session_id).await
    }

    /// closeSession: no further turns. The provider keeps its own transcript.
    pub async fn close_session(&self, session_id: &str) -> Result<AgentSession, RuntimeError> {
        if self.lock().active.contains_key(session_id) {
            return Err(RuntimeError::NotReady(
                "A turn is running in this session. Cancel it first.".into(),
            ));
        }
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
    /// for their results to be recorded.
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

    /// Claim `session_id` for one turn. Released when the guard drops, unless the turn was
    /// launched (then the turn's consumer releases it).
    fn reserve(&self, session_id: &str) -> Result<Reservation, RuntimeError> {
        let mut state = self.lock();
        if state.shutting_down {
            return Err(RuntimeError::ShuttingDown);
        }
        if state.active.contains_key(session_id) {
            return Err(RuntimeError::NotReady(
                "A turn is already running in this session. Wait for it or cancel it.".into(),
            ));
        }
        if state.active.len() >= self.inner.config.max_active_turns {
            return Err(RuntimeError::NotReady(format!(
                "{} agent turns are already running; wait for one to finish.",
                state.active.len()
            )));
        }
        let (done_tx, done_rx) = watch::channel(false);
        state.active.insert(
            session_id.to_owned(),
            Active {
                task_id: None,
                execution_id: None,
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
        objective: String,
        mut reservation: Reservation,
    ) -> Result<AgentSessionDetail, RuntimeError> {
        let number = session.turn_count + 1;
        let request = TurnRequest {
            session: match (
                &session.provider_session_id,
                session.provider_session_confirmed,
            ) {
                (Some(id), true) => ProviderSession::Resume { id: id.clone() },
                // Unconfirmed: start the provider session (again). A fresh ID avoids reusing
                // one a failed attempt may have claimed.
                _ => ProviderSession::New {
                    preassigned: adapter
                        .preassigns_session_id()
                        .then(|| uuid::Uuid::new_v4().to_string()),
                },
            },
            model: session.model.clone(),
        };
        let (s, text) = (session.clone(), objective.clone());
        let task_id = self
            .with_store(move |store| store.begin_turn(&s, number, &text))
            .await?;
        session.turn_count = number;
        session.updated_at = crate::now_ms();
        if let Some(active) = self.lock().active.get_mut(&session.id) {
            active.task_id = Some(task_id.clone());
        }

        let (tx, rx) = mpsc::channel::<OutputLine>(256);
        let provider_session = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { preassigned } => preassigned.clone(),
        };
        let spec = LaunchSpec {
            profile_id: format!("agent.{}", adapter.id()),
            label: format!("{} · turn {number}", adapter.label()),
            executable: ready.executable,
            args: adapter.turn_args(&request),
            env: ready.env,
            working_dir: PathBuf::from(&session.working_dir),
            max_runtime: self.inner.config.turn_timeout,
            stdin: Some(objective.clone().into_bytes()),
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
        let turn = AgentTurn {
            task_id: task_id.clone(),
            session_id: session.id.clone(),
            number,
            objective,
            execution_id: None,
            running: true,
            result: None,
            started_at: session.updated_at,
            ended_at: None,
        };
        let parser = adapter.parser(&request);
        let execution_id = match self.inner.supervisor.launch(spec).await {
            Ok(record) => Some(record.id),
            Err(e) => {
                // Nothing ran: record the turn as failed right away.
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
                let ctx = TurnContext {
                    runtime: self.clone(),
                    session,
                    turn,
                    execution_id: None,
                    seq: 0,
                    stored: 0,
                };
                ctx.complete(result, reservation.done.take()).await;
                return self.session(&reservation.session_id).await;
            }
        };
        if let Some(active) = self.lock().active.get_mut(&session.id) {
            active.execution_id.clone_from(&execution_id);
        }
        let turn = AgentTurn {
            execution_id: execution_id.clone(),
            ..turn
        };
        self.inner.sink.emit(AgentUpdate::Turn(turn.clone()));
        let current = self.with_active(session.clone());
        self.inner.sink.emit(AgentUpdate::Session(current));
        let ctx = TurnContext {
            runtime: self.clone(),
            session,
            turn,
            execution_id,
            seq: 0,
            stored: 0,
        };
        let done = reservation.done.take();
        let session_id = reservation.session_id.clone();
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

/// Per-turn state owned by the task that consumes the turn's output.
struct TurnContext {
    runtime: AgentRuntime,
    session: AgentSession,
    turn: AgentTurn,
    execution_id: Option<String>,
    seq: u64,
    stored: u32,
}

impl TurnContext {
    async fn consume(
        mut self,
        mut parser: Box<dyn TurnParser>,
        mut rx: mpsc::Receiver<OutputLine>,
        done: Option<watch::Sender<bool>>,
    ) {
        let mut stopping = false;
        while let Some(line) = rx.recv().await {
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
            task_id: self.turn.task_id.clone(),
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
                    let current = runtime.with_active(self.session.clone());
                    runtime.inner.sink.emit(AgentUpdate::Session(current));
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
        let (session_id, task_id, execution_id) = (
            self.session.id.clone(),
            self.turn.task_id.clone(),
            self.execution_id.clone(),
        );
        if let Err(e) = runtime
            .with_store(move |s| {
                s.record_activity(&session_id, &task_id, execution_id.as_deref(), &event)
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
        let (session_id, task_id, execution_id, stored) = (
            self.session.id.clone(),
            self.turn.task_id.clone(),
            self.execution_id.clone(),
            result.clone(),
        );
        if let Err(e) = runtime
            .with_store(move |s| {
                s.finish_turn(&session_id, &task_id, execution_id.as_deref(), &stored)
            })
            .await
        {
            runtime.notice(e.to_string());
        }
        self.session.updated_at = crate::now_ms();
        let saved = self.session.clone();
        let _ = runtime
            .with_store(move |s| s.save_session(&saved, SessionChange::Touched))
            .await;

        runtime.lock().active.remove(&self.session.id);
        if let Some(done) = done {
            let _ = done.send(true);
        }
        self.turn.running = false;
        self.turn.ended_at = Some(crate::now_ms());
        self.turn.result = Some(result);
        runtime.inner.sink.emit(AgentUpdate::Turn(self.turn));
        let current = runtime.with_active(self.session);
        runtime.inner.sink.emit(AgentUpdate::Session(current));
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
}
