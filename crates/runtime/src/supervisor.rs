//! The runtime supervisor: launches approved profiles and owns their process trees.

use std::collections::{HashMap, VecDeque};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::dto::{
    ExecutionOutput, ExecutionRecord, ExecutionState, LifecycleEvent, OutputBatch, OutputLine,
    OutputStream, RuntimeEvent, RuntimeOverview,
};
use crate::error::RuntimeError;
use crate::output::{read_lines, OutputBuffer, RawLine};
use crate::policy::{build_child_env, check_working_dir, ExecutablePolicy};
use crate::profile::{LaunchProfile, ProfileRegistry};
use crate::store::{MetadataStore, MAX_HISTORY};

/// Receives runtime events. Implementations must not block for long.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: RuntimeEvent);
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Lines kept in memory per execution.
    pub output_buffer_lines: usize,
    /// Longer lines are cut and flagged `truncated`.
    pub max_line_bytes: usize,
    /// Finished executions whose output stays viewable.
    pub retained_outputs: usize,
    /// How long to wait for a killed process tree to be reaped.
    pub kill_grace: Duration,
    /// How long to wait for output pipes to close after the main process exits.
    pub drain_timeout: Duration,
    /// Output is emitted at most this often per execution...
    pub batch_interval: Duration,
    /// ...or as soon as this many lines are pending.
    pub batch_max_lines: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            output_buffer_lines: 1000,
            max_line_bytes: 8 * 1024,
            retained_outputs: 50,
            kill_grace: Duration::from_secs(5),
            drain_timeout: Duration::from_secs(2),
            batch_interval: Duration::from_millis(50),
            batch_max_lines: 200,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelReason {
    User,
    Shutdown,
}

impl CancelReason {
    fn describe(self) -> &'static str {
        match self {
            Self::User => "Cancelled by user",
            Self::Shutdown => "Cancelled because Plenipo was shutting down",
        }
    }
}

struct Live {
    cancel: Option<oneshot::Sender<CancelReason>>,
    done: watch::Receiver<bool>,
}

#[derive(Default)]
struct State {
    /// Oldest first.
    records: Vec<ExecutionRecord>,
    live: HashMap<String, Live>,
    outputs: HashMap<String, OutputBuffer>,
    /// Finished executions with retained output, oldest first.
    finished_outputs: VecDeque<String>,
    notices: Vec<String>,
    shutting_down: bool,
}

struct Inner {
    config: SupervisorConfig,
    policy: ExecutablePolicy,
    profiles: ProfileRegistry,
    store: MetadataStore,
    sink: Arc<dyn EventSink>,
    state: Mutex<State>,
}

/// Cheap to clone; all clones share the same executions.
#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Inner>,
}

impl Supervisor {
    /// Create a supervisor, recovering history from `store`.
    ///
    /// Records left `starting`/`running` by a previous session are marked `interrupted`.
    pub fn new(
        config: SupervisorConfig,
        policy: ExecutablePolicy,
        profiles: ProfileRegistry,
        store: MetadataStore,
        sink: Arc<dyn EventSink>,
        mut notices: Vec<String>,
    ) -> Self {
        let loaded = store.load();
        notices.extend(loaded.notices);
        let mut records = loaded.records;
        let mut recovered = 0;
        for record in records.iter_mut().filter(|r| !r.state.is_terminal()) {
            record.state = ExecutionState::Interrupted;
            record.detail =
                Some("Plenipo stopped while this process was running; outcome unknown".into());
            recovered += 1;
        }
        if recovered > 0 {
            notices.push(format!(
                "{recovered} execution(s) from a previous session were marked interrupted."
            ));
            if let Err(e) = store.save(&records) {
                notices.push(format!("Could not save execution history: {e}"));
            }
        }
        Self {
            inner: Arc::new(Inner {
                config,
                policy,
                profiles,
                store,
                sink,
                state: Mutex::new(State {
                    records,
                    notices,
                    ..State::default()
                }),
            }),
        }
    }

    pub fn overview(&self) -> RuntimeOverview {
        let state = self.inner.lock();
        RuntimeOverview {
            profiles: self.inner.profiles.infos(),
            executions: state.records.iter().rev().cloned().collect(),
            active_count: u32::try_from(state.live.len()).unwrap_or(u32::MAX),
            notices: state.notices.clone(),
        }
    }

    pub fn active_count(&self) -> usize {
        self.inner.lock().live.len()
    }

    pub fn record(&self, id: &str) -> Option<ExecutionRecord> {
        self.inner
            .lock()
            .records
            .iter()
            .find(|r| r.id == id)
            .cloned()
    }

    pub fn output(&self, id: &str) -> Result<ExecutionOutput, RuntimeError> {
        let state = self.inner.lock();
        if !state.records.iter().any(|r| r.id == id) {
            return Err(RuntimeError::UnknownExecution(id.to_owned()));
        }
        Ok(match state.outputs.get(id) {
            Some(buf) => {
                let (lines, dropped) = buf.snapshot();
                ExecutionOutput {
                    execution_id: id.to_owned(),
                    lines,
                    dropped,
                    available: true,
                }
            }
            None => ExecutionOutput {
                execution_id: id.to_owned(),
                lines: vec![],
                dropped: 0,
                available: false,
            },
        })
    }

    /// Launch the profile `profile_id`. Returns the new record.
    ///
    /// Policy and input problems are errors. A process that is allowed but fails to start
    /// is not an error: it produces a `failed` execution with an explanation.
    pub async fn start(&self, profile_id: &str) -> Result<ExecutionRecord, RuntimeError> {
        let inner = &self.inner;
        let profile = inner
            .profiles
            .get(profile_id)
            .ok_or_else(|| RuntimeError::UnknownProfile(profile_id.to_owned()))?
            .clone();
        if inner.lock().shutting_down {
            return Err(RuntimeError::ShuttingDown);
        }
        // Re-check at spawn time: the file may have changed since registration.
        let executable = inner.policy.check(&profile.executable)?;
        let working_dir = check_working_dir(&profile.working_dir)?;

        let id = uuid::Uuid::new_v4().to_string();
        let record = ExecutionRecord {
            id: id.clone(),
            profile_id: profile.id.clone(),
            label: profile.label.clone(),
            executable: executable.display().to_string(),
            args: profile.args.clone(),
            working_dir: working_dir.display().to_string(),
            pid: None,
            state: ExecutionState::Starting,
            exit_code: None,
            detail: None,
            started_at: crate::now_ms(),
            ended_at: None,
        };
        {
            let mut state = inner.lock();
            state.records.push(record.clone());
            state.outputs.insert(
                id.clone(),
                OutputBuffer::new(inner.config.output_buffer_lines),
            );
            inner.persist(&mut state);
        }
        inner.emit_lifecycle(record);

        let mut command = build_command(&profile, &executable, &working_dir);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let record = inner.finish(
                    &id,
                    ExecutionState::Failed,
                    None,
                    Some(format!("Failed to start: {e}")),
                );
                return Ok(record);
            }
        };

        let stdout = child.stdout().take();
        let stderr = child.stderr().take();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (done_tx, done_rx) = watch::channel(false);
        let record = {
            let mut state = inner.lock();
            state.live.insert(
                id.clone(),
                Live {
                    cancel: Some(cancel_tx),
                    done: done_rx,
                },
            );
            let record = inner.transition(&mut state, &id, ExecutionState::Running, |r| {
                r.pid = child.id();
            });
            inner.persist(&mut state);
            record
        };
        inner.emit_lifecycle(record.clone());

        tokio::spawn(supervise(
            Arc::clone(inner),
            id,
            child,
            stdout,
            stderr,
            cancel_rx,
            done_tx,
            profile.max_runtime,
        ));
        Ok(record)
    }

    /// Terminate an execution's process tree and wait for its final record.
    /// Cancelling an execution that already finished returns its record unchanged.
    pub async fn cancel(&self, id: &str) -> Result<ExecutionRecord, RuntimeError> {
        let mut done = {
            let mut state = self.inner.lock();
            let record = state
                .records
                .iter()
                .find(|r| r.id == id)
                .cloned()
                .ok_or_else(|| RuntimeError::UnknownExecution(id.to_owned()))?;
            let Some(live) = state.live.get_mut(id) else {
                return Ok(record);
            };
            if let Some(tx) = live.cancel.take() {
                let _ = tx.send(CancelReason::User);
            }
            live.done.clone()
        };
        let limit = self.inner.config.kill_grace + self.inner.config.drain_timeout * 2;
        let _ = tokio::time::timeout(limit, done.wait_for(|d| *d)).await;
        self.record(id)
            .ok_or_else(|| RuntimeError::UnknownExecution(id.to_owned()))
    }

    /// Refuse new launches, cancel everything running, and wait up to `grace`.
    /// Returns the number of executions that were still active.
    pub async fn shutdown(&self, grace: Duration) -> usize {
        let mut waiting = Vec::new();
        {
            let mut state = self.inner.lock();
            state.shutting_down = true;
            for live in state.live.values_mut() {
                if let Some(tx) = live.cancel.take() {
                    let _ = tx.send(CancelReason::Shutdown);
                }
                waiting.push(live.done.clone());
            }
        }
        let count = waiting.len();
        let all = async {
            for mut done in waiting {
                let _ = done.wait_for(|d| *d).await;
            }
        };
        let _ = tokio::time::timeout(grace, all).await;
        count
    }
}

fn build_command(
    profile: &LaunchProfile,
    executable: &std::path::Path,
    working_dir: &std::path::Path,
) -> CommandWrap {
    let env = build_child_env(&profile.env);
    let mut command = CommandWrap::with_new(executable, |c| {
        c.args(&profile.args)
            .current_dir(working_dir)
            .env_clear()
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    });
    #[cfg(windows)]
    {
        use process_wrap::tokio::{CreationFlags, JobObject};
        use windows::Win32::System::Threading::CREATE_NO_WINDOW;
        // Job Object + KillOnDrop => JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: if Plenipo dies,
        // Windows terminates the whole tree.
        command
            .wrap(CreationFlags(CREATE_NO_WINDOW))
            .wrap(JobObject);
    }
    #[cfg(unix)]
    {
        use process_wrap::tokio::ProcessGroup;
        command.wrap(ProcessGroup::leader());
    }
    command.wrap(KillOnDrop);
    command
}

enum Outcome {
    Exited(std::io::Result<ExitStatus>),
    Cancelled(CancelReason),
    TimedOut,
}

#[allow(clippy::too_many_arguments)]
async fn supervise(
    inner: Arc<Inner>,
    id: String,
    mut child: Box<dyn ChildWrapper>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    cancel_rx: oneshot::Receiver<CancelReason>,
    done_tx: watch::Sender<bool>,
    max_runtime: Duration,
) {
    let config = inner.config.clone();
    let (tx, rx) = mpsc::channel::<RawLine>(1024);
    let mut readers: Vec<JoinHandle<()>> = Vec::new();
    if let Some(out) = stdout {
        readers.push(tokio::spawn(read_lines(
            out,
            OutputStream::Stdout,
            config.max_line_bytes,
            tx.clone(),
        )));
    }
    if let Some(err) = stderr {
        readers.push(tokio::spawn(read_lines(
            err,
            OutputStream::Stderr,
            config.max_line_bytes,
            tx.clone(),
        )));
    }
    drop(tx);
    let aggregator = tokio::spawn(aggregate(Arc::clone(&inner), id.clone(), rx));

    let cancelled = async {
        match cancel_rx.await {
            Ok(reason) => reason,
            // Sender dropped without cancelling: never resolve.
            Err(_) => std::future::pending().await,
        }
    };
    let outcome = tokio::select! {
        status = child.wait() => Outcome::Exited(status),
        reason = cancelled => Outcome::Cancelled(reason),
        () = tokio::time::sleep(max_runtime) => Outcome::TimedOut,
    };

    let (state, exit_code, mut detail) = match outcome {
        Outcome::Exited(Ok(status)) => classify(status),
        Outcome::Exited(Err(e)) => (
            ExecutionState::Failed,
            None,
            Some(format!("Lost track of the process: {e}")),
        ),
        Outcome::Cancelled(reason) => {
            kill_tree(child.as_mut(), config.kill_grace).await;
            (
                ExecutionState::Cancelled,
                None,
                Some(reason.describe().to_owned()),
            )
        }
        Outcome::TimedOut => {
            kill_tree(child.as_mut(), config.kill_grace).await;
            (
                ExecutionState::TimedOut,
                None,
                Some(format!(
                    "Exceeded the maximum runtime of {}s and was terminated",
                    max_runtime.as_secs()
                )),
            )
        }
    };

    // Output pipes close when every process holding them has exited. If descendants of a
    // finished process keep them open, terminate the rest of the tree: Plenipo does not
    // leave owned processes running.
    let drained = tokio::time::timeout(config.drain_timeout, async {
        for reader in readers.iter_mut() {
            let _ = reader.await;
        }
    })
    .await
    .is_ok();
    if !drained {
        kill_tree(child.as_mut(), config.kill_grace).await;
        let drained_after_kill = tokio::time::timeout(config.kill_grace, async {
            for reader in readers.iter_mut() {
                let _ = reader.await;
            }
        })
        .await
        .is_ok();
        if !drained_after_kill {
            readers.iter().for_each(JoinHandle::abort);
        }
        let note = "Remaining descendant processes were terminated";
        detail = Some(match detail {
            Some(d) => format!("{d}. {note}"),
            None => note.to_owned(),
        });
    }
    let _ = aggregator.await;

    inner.finish(&id, state, exit_code, detail);
    let _ = done_tx.send(true);
}

async fn kill_tree(child: &mut dyn ChildWrapper, grace: Duration) {
    // Windows: terminates the Job Object. Unix: SIGKILL to the process group.
    let _ = child.start_kill();
    let _ = tokio::time::timeout(grace, child.wait()).await;
}

fn classify(status: ExitStatus) -> (ExecutionState, Option<i32>, Option<String>) {
    match status.code() {
        Some(0) => (ExecutionState::Succeeded, Some(0), None),
        Some(code) => (
            ExecutionState::Failed,
            Some(code),
            Some(format!("Exited with code {code}")),
        ),
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt as _;
                if let Some(signal) = status.signal() {
                    return (
                        ExecutionState::Failed,
                        None,
                        Some(format!("Terminated by signal {signal}")),
                    );
                }
            }
            (
                ExecutionState::Failed,
                None,
                Some("Exited without a status code".into()),
            )
        }
    }
}

/// Sequence output lines into the buffer and emit them in batches.
async fn aggregate(inner: Arc<Inner>, id: String, mut rx: mpsc::Receiver<RawLine>) {
    let config = &inner.config;
    let mut batch: Vec<OutputLine> = Vec::new();
    let mut tick = tokio::time::interval(config.batch_interval);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            raw = rx.recv() => match raw {
                Some(raw) => {
                    if let Some(line) = inner.push_output(&id, raw) {
                        batch.push(line);
                    }
                    if batch.len() >= config.batch_max_lines {
                        inner.emit_output(&id, std::mem::take(&mut batch));
                    }
                }
                None => break,
            },
            _ = tick.tick() => {
                if !batch.is_empty() {
                    inner.emit_output(&id, std::mem::take(&mut batch));
                }
            }
        }
    }
    if !batch.is_empty() {
        inner.emit_output(&id, batch);
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock must not take the whole runtime down.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn persist(&self, state: &mut State) {
        // Keep history bounded: drop the oldest finished records first.
        while state.records.len() > MAX_HISTORY {
            match state.records.iter().position(|r| r.state.is_terminal()) {
                Some(i) => {
                    let removed = state.records.remove(i);
                    state.outputs.remove(&removed.id);
                    state.finished_outputs.retain(|id| id != &removed.id);
                }
                None => break,
            }
        }
        if let Err(e) = self.store.save(&state.records) {
            let notice = format!("Could not save execution history: {e}");
            if !state.notices.contains(&notice) {
                state.notices.push(notice);
            }
        }
    }

    /// Apply a validated state transition and return the updated record.
    fn transition(
        &self,
        state: &mut State,
        id: &str,
        next: ExecutionState,
        update: impl FnOnce(&mut ExecutionRecord),
    ) -> ExecutionRecord {
        let record = state
            .records
            .iter_mut()
            .find(|r| r.id == id)
            .expect("transition on a known execution");
        assert!(
            record.state.can_transition_to(next),
            "illegal execution transition {:?} -> {next:?}",
            record.state
        );
        record.state = next;
        update(record);
        record.clone()
    }

    fn finish(
        &self,
        id: &str,
        next: ExecutionState,
        exit_code: Option<i32>,
        detail: Option<String>,
    ) -> ExecutionRecord {
        let record = {
            let mut state = self.lock();
            let record = self.transition(&mut state, id, next, |r| {
                r.exit_code = exit_code;
                r.detail = detail;
                r.ended_at = Some(crate::now_ms());
            });
            state.live.remove(id);
            state.finished_outputs.push_back(id.to_owned());
            while state.finished_outputs.len() > self.config.retained_outputs {
                if let Some(old) = state.finished_outputs.pop_front() {
                    state.outputs.remove(&old);
                }
            }
            self.persist(&mut state);
            record
        };
        self.emit_lifecycle(record.clone());
        record
    }

    fn push_output(&self, id: &str, raw: RawLine) -> Option<OutputLine> {
        let mut state = self.lock();
        state
            .outputs
            .get_mut(id)
            .map(|buf| buf.push(raw, crate::now_ms()))
    }

    fn emit_output(&self, id: &str, lines: Vec<OutputLine>) {
        self.sink.emit(RuntimeEvent::Output(OutputBatch {
            execution_id: id.to_owned(),
            lines,
        }));
    }

    fn emit_lifecycle(&self, record: ExecutionRecord) {
        self.sink
            .emit(RuntimeEvent::Lifecycle(LifecycleEvent { record }));
    }
}
