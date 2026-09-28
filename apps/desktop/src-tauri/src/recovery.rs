//! Recovery (Phase 13, ADR-037 items 6–8): knowing how the last run ended, and telling the
//! owner in plain words what stopped and what they can do.
//!
//! While Plenipo runs it keeps a small note, `run\plenipo-running.json`, in its own folder: the
//! version, when it started, a heartbeat every 30 seconds, and whether it was changing the
//! Ledger's layout. Never task text or secrets. A clean exit removes the note, so a note found at
//! the next start means the last run did not end cleanly:
//!
//! - it was changing the Ledger's layout → an interrupted migration;
//! - Windows started after its last heartbeat → a Windows restart (or a power cut);
//! - otherwise → a crash (or it was ended from Task Manager);
//! - the note is damaged → the cause is unknown.
//!
//! Then Plenipo records `plenipo.recovered` in the Ledger with the tasks that were stopped, and
//! the window shows them with **Run again** and **Leave stopped**. Nothing runs again by itself.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plenipo_core::{Recovery, RecoveryCause, RecoveryStatus, StoppedTask, WindowRecovery};
use plenipo_ledger::{Ledger, LedgerError, NewEvent, Task, TaskState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The note's file name, in the `run` folder.
pub const RUN_NOTE: &str = "plenipo-running.json";
/// How often the note's heartbeat is written.
pub const HEARTBEAT: Duration = Duration::from_secs(30);
/// Ledger event recording a recovery.
pub const RECOVERED: &str = "plenipo.recovered";
/// Ledger event recording a Run again.
pub const RUN_AGAIN: &str = "plenipo.run_again";
/// The Ledger setting for Plenipo's own state (the last version that ran, what was dismissed).
pub const APP_SETTING: &str = "app";
const PLENIPO: &str = "plenipo";
/// At most this many stopped tasks are listed (the rest are counted).
const MAX_LISTED: usize = 20;

/// What Plenipo was doing, as the note says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Opening the Ledger.
    Opening,
    /// Changing the Ledger's layout (a migration).
    ChangingLayout,
    Running,
    /// Windows is ending the session (a restart, a shutdown, or signing out) and is closing
    /// Plenipo, as it closes every program.
    EndedByWindows,
}

/// The note Plenipo keeps while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunNote {
    pub version: String,
    pub pid: u32,
    pub started_at: u64,
    pub heartbeat_at: u64,
    pub phase: Phase,
}

/// How the previous run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousEnd {
    /// It ended cleanly, or this is the first run.
    Clean,
    Unclean {
        cause: RecoveryCause,
        last_seen_at: Option<u64>,
        previous_version: Option<String>,
    },
}

impl PreviousEnd {
    pub fn is_unclean(&self) -> bool {
        matches!(self, Self::Unclean { .. })
    }
}

/// Read what the note says about the previous run. `boot_ms`: when Windows last started.
pub fn previous_end(note: &Path, boot_ms: Option<u64>) -> PreviousEnd {
    let Ok(bytes) = std::fs::read(note) else {
        return PreviousEnd::Clean;
    };
    let Ok(note) = serde_json::from_slice::<RunNote>(&bytes) else {
        return PreviousEnd::Unclean {
            cause: RecoveryCause::Unknown,
            last_seen_at: None,
            previous_version: None,
        };
    };
    let cause = if note.phase == Phase::ChangingLayout {
        RecoveryCause::LayoutChange
    } else if note.phase == Phase::EndedByWindows
        || boot_ms.is_some_and(|boot| boot > note.heartbeat_at)
    {
        RecoveryCause::WindowsRestart
    } else {
        RecoveryCause::Crash
    };
    PreviousEnd::Unclean {
        cause,
        last_seen_at: Some(note.heartbeat_at),
        previous_version: Some(note.version),
    }
}

/// When Windows (the computer) last started, in milliseconds since 1970.
pub fn boot_ms() -> Option<u64> {
    let secs = sysinfo::System::boot_time();
    (secs > 0).then(|| secs.saturating_mul(1000))
}

fn write_atomic(path: &Path, note: &RunNote) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(note).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, path)
}

/// Keeps the note for this run: its phase, a heartbeat, and removing it at a clean exit.
pub struct RunNoteKeeper {
    path: PathBuf,
    note: Mutex<RunNote>,
    finished: AtomicBool,
}

impl RunNoteKeeper {
    /// Start this run's note (replacing the previous run's, which must be read first).
    pub fn start(path: PathBuf, version: &str) -> Arc<Self> {
        let now = plenipo_ledger::now_ms();
        let keeper = Arc::new(Self {
            path,
            note: Mutex::new(RunNote {
                version: version.to_owned(),
                pid: std::process::id(),
                started_at: now,
                heartbeat_at: now,
                phase: Phase::Opening,
            }),
            finished: AtomicBool::new(false),
        });
        keeper.update(|_| {});
        keeper
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Change the note and write it. The lock is held while writing, and [`Self::finish`] takes
    /// it too, so a heartbeat can never write the note again after a clean exit removed it.
    fn update(&self, change: impl FnOnce(&mut RunNote)) {
        let mut note = self.note.lock().unwrap_or_else(|p| p.into_inner());
        if self.finished.load(Ordering::SeqCst) {
            return;
        }
        change(&mut note);
        if let Err(e) = write_atomic(&self.path, &note) {
            log::warn!("the note that Plenipo is running could not be written: {e}");
        }
    }

    pub fn set_phase(&self, phase: Phase) {
        self.update(|note| {
            note.phase = phase;
            note.heartbeat_at = plenipo_ledger::now_ms();
        });
    }

    pub fn beat(&self) {
        self.update(|note| note.heartbeat_at = plenipo_ledger::now_ms());
    }

    /// The run has not ended cleanly (yet).
    pub fn is_running(&self) -> bool {
        !self.finished.load(Ordering::SeqCst)
    }

    /// Write the heartbeat every [`HEARTBEAT`] until the run ends.
    pub fn keep_beating(self: &Arc<Self>) {
        let keeper = Arc::downgrade(self);
        let _ = std::thread::Builder::new()
            .name("plenipo-heartbeat".into())
            .spawn(move || loop {
                std::thread::sleep(HEARTBEAT);
                match keeper.upgrade() {
                    Some(k) if !k.finished.load(Ordering::SeqCst) => k.beat(),
                    _ => return,
                }
            });
    }

    /// A clean exit: remove the note.
    pub fn finish(&self) {
        let _note = self.note.lock().unwrap_or_else(|p| p.into_inner());
        if !self.finished.swap(true, Ordering::SeqCst) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// What the start noticed, kept for the window.
#[derive(Default)]
pub struct RecoveryState {
    window: Mutex<Option<WindowRecovery>>,
}

impl RecoveryState {
    pub fn window_recovered(&self, reopened: bool) {
        *self.window.lock().unwrap_or_else(|p| p.into_inner()) = Some(WindowRecovery {
            at: plenipo_ledger::now_ms(),
            reopened,
        });
    }

    pub fn window(&self) -> Option<WindowRecovery> {
        self.window
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn dismiss_window(&self) {
        *self.window.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}

/// The tasks and programs that were in progress when the previous run ended (read before the
/// services mark them stopped).
#[derive(Debug, Default, Clone)]
pub struct InProgress {
    pub tasks: Vec<Task>,
    pub programs: u32,
}

/// Read what was in progress, before anything marks it stopped.
pub fn in_progress(ledger: &Ledger) -> InProgress {
    // Diagnostics' synthetic tasks are the owner's hand-made test tasks, not work: they are left
    // exactly as they were (Phase 2: their trail survives a hard stop unchanged).
    let tasks = ledger
        .unfinished_tasks()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !flag(t, "synthetic"))
        .collect();
    let programs = ledger
        .recent_executions(1000)
        .map(|rows| {
            rows.iter()
                .filter(|r| matches!(r.state.as_str(), "starting" | "running"))
                .count()
        })
        .unwrap_or(0);
    InProgress {
        tasks,
        programs: u32::try_from(programs).unwrap_or(u32::MAX),
    }
}

fn flag(task: &Task, key: &str) -> bool {
    task.metadata.get(key) == Some(&Value::Bool(true))
}

/// The launch test's task (`PLENIPO_SMOKE_SCENARIO=start-work`) has no conversation to mark it
/// stopped: mark it here, as the services do for real work.
fn stop_smoke_tasks(ledger: &Ledger, tasks: &[Task]) {
    for t in tasks {
        if flag(t, "smoke") && !t.state.is_terminal() {
            let _ = ledger.transition_task(
                &t.id,
                TaskState::Failed,
                PLENIPO,
                Some("Plenipo stopped while this task was running"),
            );
        }
    }
}

/// After the services have marked unfinished work stopped: record the recovery (once).
pub fn record(
    ledger: &Ledger,
    end: &PreviousEnd,
    before: &InProgress,
    this_version: &str,
) -> Option<String> {
    let PreviousEnd::Unclean {
        cause,
        last_seen_at,
        previous_version,
    } = end
    else {
        return None;
    };
    stop_smoke_tasks(ledger, &before.tasks);
    // The objectives that stopped: the tasks with no parent (their children stopped with them).
    let roots: Vec<&String> = before
        .tasks
        .iter()
        .filter(|t| t.parent_task_id.is_none())
        .map(|t| &t.id)
        .collect();
    let event = NewEvent {
        source: PLENIPO.into(),
        event_type: RECOVERED.into(),
        payload: json!({
            "cause": cause,
            "lastSeenAt": last_seen_at,
            "previousVersion": previous_version,
            "version": this_version,
            "stoppedTasks": roots,
            "stoppedTaskCount": before.tasks.len(),
            "stoppedPrograms": before.programs,
        }),
        ..NewEvent::default()
    };
    match ledger.append_event(event) {
        Ok(e) => {
            log::warn!(
                "{} {} task(s) and {} program(s) were stopped.",
                sentence(*cause),
                before.tasks.len(),
                before.programs
            );
            Some(e.id)
        }
        Err(e) => {
            log::error!("the recovery could not be recorded: {e}");
            None
        }
    }
}

/// What happened, in a plain sentence (times are shown by the window, in local time).
pub fn sentence(cause: RecoveryCause) -> &'static str {
    match cause {
        RecoveryCause::Crash => "Plenipo closed unexpectedly.",
        RecoveryCause::WindowsRestart => {
            "Windows closed Plenipo (a restart, a shutdown, or signing out) while it was running."
        }
        RecoveryCause::LayoutChange => {
            "Plenipo was stopped while it was updating the Ledger's layout; the unfinished step \
             was undone and done again."
        }
        RecoveryCause::Unknown => "Plenipo did not close normally last time.",
    }
}

fn dismissed(ledger: &Ledger) -> Option<String> {
    ledger
        .setting(APP_SETTING)
        .ok()
        .flatten()
        .and_then(|v| v["dismissedRecovery"].as_str().map(str::to_owned))
}

/// Leave the stopped tasks stopped: the notice goes away.
pub fn dismiss(ledger: &Ledger, id: &str) -> plenipo_ledger::Result<()> {
    ledger.merge_setting(APP_SETTING, &json!({ "dismissedRecovery": id }), "owner")?;
    Ok(())
}

/// The first line of a task's objective.
fn first_line(s: &str) -> String {
    let line = s.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() > 160 {
        let cut: String = line.chars().take(159).collect();
        format!("{cut}…")
    } else {
        line.to_owned()
    }
}

/// Who was doing a task: a position's title, or the AI tool of a conversation.
fn who(ledger: &Ledger, task: &Task, tool: &dyn Fn(&str) -> String) -> Option<String> {
    if let Some(position) = task.metadata["workforce"]["positionId"].as_str() {
        if let Ok(records) = ledger.org_records() {
            if let Some(p) = records.positions.iter().find(|p| p.id == position) {
                return Some(p.title.clone());
            }
        }
    }
    task.metadata["runtimeId"].as_str().map(tool)
}

/// How a stopped task can be run again, if it can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAgain {
    /// Give the objective to the same position again.
    Position {
        position_id: String,
        project_id: Option<String>,
        objective: String,
    },
    /// Continue the same conversation (Workers) with the same objective.
    Conversation {
        session_id: String,
        objective: String,
    },
}

/// The objective as the owner gave it: without the project line Plenipo adds for the agent.
fn owners_objective(objective: &str) -> String {
    match objective.rsplit_once("\n\nProject: ") {
        Some((before, project)) if !project.contains('\n') => before.trim().to_owned(),
        _ => objective.trim().to_owned(),
    }
}

/// Whether `task` can be run again, and how. Only an objective that ended without success
/// (failed, or stopped) and was given by the owner can.
pub fn run_again_plan(task: &Task) -> Result<RunAgain, String> {
    if task.parent_task_id.is_some() {
        return Err(
            "Only a whole objective can be run again; run again the one this task belongs to."
                .into(),
        );
    }
    if !matches!(task.state, TaskState::Failed | TaskState::Cancelled) {
        return Err("Only a task that stopped or failed can be run again.".into());
    }
    let objective = owners_objective(&task.objective);
    if objective.is_empty() {
        return Err("This task has nothing to run again.".into());
    }
    let workforce = &task.metadata["workforce"];
    if let Some(position_id) = workforce["positionId"].as_str() {
        if task.metadata["liaison"]["origin"].as_str() == Some("handoff") {
            return Err(
                "This task was handed over by another worker; run its objective again.".into(),
            );
        }
        return Ok(RunAgain::Position {
            position_id: position_id.to_owned(),
            project_id: task.project_id.clone(),
            objective,
        });
    }
    if let Some(session_id) = task.metadata["sessionId"].as_str() {
        return Ok(RunAgain::Conversation {
            session_id: session_id.to_owned(),
            objective,
        });
    }
    Err("Plenipo does not know who to give this task to; give it again yourself.".into())
}

/// Record that `from` was run again as `to`.
pub fn record_run_again(ledger: &Ledger, from: &str, to: Option<&str>) {
    let event = NewEvent {
        task_id: Some(from.to_owned()),
        source: "owner".into(),
        event_type: RUN_AGAIN.into(),
        payload: json!({ "runAgainAs": to }),
        ..NewEvent::default()
    };
    if let Err(e) = ledger.append_event(event) {
        log::warn!("Run again could not be recorded: {e}");
    }
}

/// Everything recovery has to tell the window now.
pub fn status(
    ledger: &Ledger,
    state: &RecoveryState,
    settings_problems: Vec<plenipo_core::SettingsProblem>,
    tool: &dyn Fn(&str) -> String,
) -> Result<RecoveryStatus, LedgerError> {
    let latest = ledger.events_of_types(&[RECOVERED], 1)?.into_iter().next();
    let recovery = match latest {
        Some(event) if dismissed(ledger).as_deref() != Some(event.id.as_str()) => {
            let p = &event.payload;
            let cause =
                serde_json::from_value(p["cause"].clone()).unwrap_or(RecoveryCause::Unknown);
            let again: std::collections::HashMap<String, Option<String>> = ledger
                .events_of_types(&[RUN_AGAIN], 200)?
                .into_iter()
                .filter(|e| e.created_at >= event.created_at)
                .filter_map(|e| {
                    e.task_id
                        .map(|t| (t, e.payload["runAgainAs"].as_str().map(str::to_owned)))
                })
                .collect();
            let mut stopped_tasks = Vec::new();
            for id in p["stoppedTasks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .take(MAX_LISTED)
            {
                let Some(task) = ledger.task(id)? else {
                    continue;
                };
                stopped_tasks.push(StoppedTask {
                    task_id: task.id.clone(),
                    objective: first_line(&owners_objective(&task.objective)),
                    who: who(ledger, &task, tool),
                    can_run_again: run_again_plan(&task).is_ok(),
                    run_again_as: again.get(&task.id).map(|to| to.clone().unwrap_or_default()),
                });
            }
            Some(Recovery {
                id: event.id.clone(),
                cause,
                last_seen_at: p["lastSeenAt"].as_u64(),
                found_at: event.created_at,
                previous_version: p["previousVersion"].as_str().map(str::to_owned),
                stopped_tasks,
                stopped_programs: p["stoppedPrograms"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0),
            })
        }
        _ => None,
    };
    Ok(RecoveryStatus {
        recovery,
        window: state.window(),
        settings_problems,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::NewTask;

    fn note(dir: &Path, heartbeat_at: u64, phase: Phase) -> PathBuf {
        let path = dir.join(RUN_NOTE);
        write_atomic(
            &path,
            &RunNote {
                version: "1.9.0".into(),
                pid: 42,
                started_at: heartbeat_at - 1000,
                heartbeat_at,
                phase,
            },
        )
        .unwrap();
        path
    }

    #[test]
    fn no_note_means_the_last_run_ended_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            previous_end(&dir.path().join(RUN_NOTE), Some(1)),
            PreviousEnd::Clean
        );
    }

    #[test]
    fn a_note_left_behind_tells_a_crash_from_a_windows_restart() {
        let dir = tempfile::tempdir().unwrap();
        // Windows started long before the last heartbeat: Plenipo itself stopped.
        let path = note(dir.path(), 2_000_000, Phase::Running);
        assert_eq!(
            previous_end(&path, Some(1_000_000)),
            PreviousEnd::Unclean {
                cause: RecoveryCause::Crash,
                last_seen_at: Some(2_000_000),
                previous_version: Some("1.9.0".into()),
            }
        );
        // Windows started after the last heartbeat: a restart (or a power cut).
        match previous_end(&path, Some(3_000_000)) {
            PreviousEnd::Unclean { cause, .. } => assert_eq!(cause, RecoveryCause::WindowsRestart),
            other => panic!("{other:?}"),
        }
        // Unknown boot time: a crash is the safe reading.
        match previous_end(&path, None) {
            PreviousEnd::Unclean { cause, .. } => assert_eq!(cause, RecoveryCause::Crash),
            other => panic!("{other:?}"),
        }
        // Windows said it was ending the session (a sign-out, with no restart after it).
        let path = note(dir.path(), 2_000_000, Phase::EndedByWindows);
        match previous_end(&path, Some(1_000_000)) {
            PreviousEnd::Unclean { cause, .. } => assert_eq!(cause, RecoveryCause::WindowsRestart),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_note_left_while_changing_the_layout_is_an_interrupted_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = note(dir.path(), 2_000_000, Phase::ChangingLayout);
        match previous_end(&path, Some(3_000_000)) {
            PreviousEnd::Unclean { cause, .. } => assert_eq!(cause, RecoveryCause::LayoutChange),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_damaged_note_is_an_unclean_end_of_unknown_cause() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RUN_NOTE);
        std::fs::write(&path, b"\x00\x01 not json").unwrap();
        assert_eq!(
            previous_end(&path, Some(1)),
            PreviousEnd::Unclean {
                cause: RecoveryCause::Unknown,
                last_seen_at: None,
                previous_version: None,
            }
        );
    }

    #[test]
    fn the_keeper_writes_its_phase_and_heartbeat_and_a_clean_exit_removes_the_note() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join(RUN_NOTE);
        let keeper = RunNoteKeeper::start(path.clone(), "1.9.0");
        let read = || serde_json::from_slice::<RunNote>(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(read().phase, Phase::Opening);
        assert_eq!(read().pid, std::process::id());
        keeper.set_phase(Phase::ChangingLayout);
        assert_eq!(read().phase, Phase::ChangingLayout);
        keeper.set_phase(Phase::Running);
        let first = read().heartbeat_at;
        std::thread::sleep(Duration::from_millis(5));
        keeper.beat();
        assert!(read().heartbeat_at > first);
        // Nothing about the work is in it.
        let text = std::fs::read_to_string(&path).unwrap();
        for key in ["objective", "task", "secret"] {
            assert!(!text.contains(key), "{text}");
        }
        keeper.finish();
        assert!(!path.exists());
        keeper.beat(); // after the end: writes nothing
        assert!(!path.exists());
        assert_eq!(previous_end(&path, Some(1)), PreviousEnd::Clean);
    }

    fn task(l: &Ledger, objective: &str, metadata: Value) -> Task {
        l.create_task(
            NewTask {
                requested_by: "owner".into(),
                objective: objective.into(),
                metadata,
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
    }

    #[test]
    fn a_recovery_is_recorded_with_the_objectives_that_stopped_and_can_be_left_stopped() {
        let l = Ledger::open_in_memory().unwrap();
        let conversation = task(
            &l,
            "Summarize the sales call",
            json!({ "sessionId": "s-1", "runtimeId": "claude-code" }),
        );
        l.transition_task(&conversation.id, TaskState::Running, "x", None)
            .unwrap();
        let synthetic = task(
            &l,
            "Synthetic diagnostic task #1",
            json!({ "synthetic": true }),
        );
        l.transition_task(&synthetic.id, TaskState::Running, "x", None)
            .unwrap();
        let smoke = task(
            &l,
            "Smoke test: a task left running",
            json!({ "smoke": true }),
        );
        l.transition_task(&smoke.id, TaskState::Running, "x", None)
            .unwrap();
        let before = in_progress(&l);
        assert_eq!(
            before.tasks.len(),
            2,
            "Diagnostics' test tasks are not work"
        );
        // What the services do at start: the conversation's turn is marked interrupted.
        l.transition_task(&conversation.id, TaskState::Failed, "plenipo", None)
            .unwrap();
        let end = PreviousEnd::Unclean {
            cause: RecoveryCause::Crash,
            last_seen_at: Some(123),
            previous_version: Some("1.9.0".into()),
        };
        let id = record(&l, &end, &before, "1.9.0").unwrap();
        // The launch test's task had nobody to stop it: recovery did. Diagnostics' test task is
        // left as it was.
        assert_eq!(l.task(&smoke.id).unwrap().unwrap().state, TaskState::Failed);
        assert_eq!(
            l.task(&synthetic.id).unwrap().unwrap().state,
            TaskState::Running
        );
        let tool = |id: &str| format!("tool {id}");
        let s = status(&l, &RecoveryState::default(), vec![], &tool).unwrap();
        let r = s.recovery.unwrap();
        assert_eq!(r.id, id);
        assert_eq!(r.cause, RecoveryCause::Crash);
        assert_eq!(r.last_seen_at, Some(123));
        assert_eq!(r.stopped_tasks.len(), 2);
        let c = r
            .stopped_tasks
            .iter()
            .find(|t| t.task_id == conversation.id)
            .unwrap();
        assert_eq!(c.objective, "Summarize the sales call");
        assert_eq!(c.who.as_deref(), Some("tool claude-code"));
        assert!(c.can_run_again);
        assert!(r.stopped_tasks.iter().all(|t| t.task_id != synthetic.id));
        let s = r
            .stopped_tasks
            .iter()
            .find(|t| t.task_id == smoke.id)
            .unwrap();
        assert!(
            !s.can_run_again,
            "the launch test's task has nobody to give it to"
        );
        // Run again is shown once used.
        record_run_again(&l, &conversation.id, Some("new-task"));
        let r = status(&l, &RecoveryState::default(), vec![], &tool)
            .unwrap()
            .recovery
            .unwrap();
        let c = r
            .stopped_tasks
            .iter()
            .find(|t| t.task_id == conversation.id)
            .unwrap();
        assert_eq!(c.run_again_as.as_deref(), Some("new-task"));
        // Leave stopped: the notice goes away.
        dismiss(&l, &id).unwrap();
        assert!(status(&l, &RecoveryState::default(), vec![], &tool)
            .unwrap()
            .recovery
            .is_none());
    }

    #[test]
    fn a_clean_end_records_nothing() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(record(&l, &PreviousEnd::Clean, &InProgress::default(), "1.9.0").is_none());
        let tool = |id: &str| id.to_owned();
        assert!(status(&l, &RecoveryState::default(), vec![], &tool)
            .unwrap()
            .recovery
            .is_none());
    }

    #[test]
    fn run_again_gives_the_owners_objective_to_the_same_worker() {
        let l = Ledger::open_in_memory().unwrap();
        let member = task(
            &l,
            "Fix the login page\n\nProject: Website (its supervisor: role:Web Supervisor).",
            json!({ "sessionId": "s-2", "workforce": { "positionId": "p-1" } }),
        );
        l.transition_task(&member.id, TaskState::Running, "x", None)
            .unwrap();
        // Still running: not yet.
        let running = l.task(&member.id).unwrap().unwrap();
        assert!(run_again_plan(&running).is_err());
        // A task it handed over.
        let child = l
            .create_task(
                NewTask {
                    parent_task_id: Some(member.id.clone()),
                    requested_by: "worker".into(),
                    objective: "Review it".into(),
                    ..NewTask::default()
                },
                "worker",
            )
            .unwrap();
        l.transition_task(&child.id, TaskState::Failed, "x", None)
            .unwrap();
        l.transition_task(&member.id, TaskState::Failed, "plenipo", None)
            .unwrap();
        let stopped = l.task(&member.id).unwrap().unwrap();
        assert_eq!(
            run_again_plan(&stopped).unwrap(),
            RunAgain::Position {
                position_id: "p-1".into(),
                project_id: None,
                objective: "Fix the login page".into(),
            }
        );
        let conversation = task(&l, "Say hi", json!({ "sessionId": "s-3" }));
        l.transition_task(&conversation.id, TaskState::Cancelled, "x", None)
            .unwrap();
        assert_eq!(
            run_again_plan(&l.task(&conversation.id).unwrap().unwrap()).unwrap(),
            RunAgain::Conversation {
                session_id: "s-3".into(),
                objective: "Say hi".into(),
            }
        );
        // A handed-over child cannot be run again on its own.
        assert!(run_again_plan(&l.task(&child.id).unwrap().unwrap()).is_err());
        // A finished objective is not run again from here.
        let done = task(&l, "Done already", json!({ "sessionId": "s-4" }));
        l.transition_task(&done.id, TaskState::Running, "x", None)
            .unwrap();
        l.transition_task(&done.id, TaskState::Succeeded, "x", None)
            .unwrap();
        assert!(run_again_plan(&l.task(&done.id).unwrap().unwrap()).is_err());
    }
}
