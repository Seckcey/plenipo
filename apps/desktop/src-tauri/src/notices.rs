//! Pop-up notices (Phase 12). Each committed Ledger event that may concern the owner goes to a
//! background thread, which asks the Ledger what it means ([`Ledger::notice_for`]), keeps the
//! kinds the owner wants, gathers those that arrive together, and shows one notice, unless
//! Plenipo's window is in front and the owner wants notices only while it is not. Windows shows
//! them (through the notification plugin); the IPC tests keep them instead.
//!
//! Nothing here is a tool: workers cannot send notices, and the web page cannot either (the
//! plugin's own commands are not granted to any window).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use plenipo_ledger::notices::may_notify;
use plenipo_ledger::{Ledger, LedgerEvent, Notice, NoticeGate, NoticeKind};
use plenipo_runtime::agent::AgentRuntime;
use tauri::{AppHandle, Manager as _, Runtime};

/// Events that arrive within this long of the first become one notice.
pub const GATHER: Duration = Duration::from_millis(1500);
/// How long "Send a test notice" waits for the system to answer.
const TEST_WAIT: Duration = Duration::from_secs(5);

type Show = Arc<dyn Fn(&Notice) -> Result<(), String> + Send + Sync>;
type InFront = Arc<dyn Fn() -> bool + Send + Sync>;

/// Where notices go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The system's notices (Windows' pop-ups), through the notification plugin.
    System,
    /// Kept in memory, for tests.
    Kept,
}

/// Plenipo's notices (app state): the test notice, and what tests kept.
pub struct Notices {
    show: Show,
    kept: Arc<Mutex<Vec<Notice>>>,
}

impl Notices {
    /// Show `notice` now, whatever the owner's choices (Settings → Notifications → Send a test
    /// notice). Waits a few seconds at most for the system to answer.
    pub fn show_now(&self, notice: Notice) -> Result<(), String> {
        let show = Arc::clone(&self.show);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(show(&notice));
        });
        rx.recv_timeout(TEST_WAIT)
            .unwrap_or_else(|_| Err("the system did not answer in time".into()))
    }

    /// The notices shown so far, when they are kept (tests).
    pub fn kept(&self) -> Vec<Notice> {
        self.kept.lock().map(|k| k.clone()).unwrap_or_default()
    }
}

/// The test notice.
pub fn test_notice() -> Notice {
    Notice {
        kind: NoticeKind::Approvals,
        title: "Plenipo's notices are on".into(),
        body: "This is how Plenipo tells you when something needs you. Choose which notices you \
               get in Settings → Notifications."
            .into(),
    }
}

/// Start deciding and showing notices from `ledger`'s events.
pub fn start<R: Runtime>(
    app: &AppHandle<R>,
    ledger: Arc<Ledger>,
    agents: Option<AgentRuntime>,
    output: Output,
    gather: Duration,
) -> Notices {
    let kept: Arc<Mutex<Vec<Notice>>> = Arc::default();
    let show: Show = match output {
        Output::System => {
            let handle = app.clone();
            let failed_once = AtomicBool::new(false);
            Arc::new(move |n: &Notice| {
                use tauri_plugin_notification::NotificationExt as _;
                let shown = handle
                    .notification()
                    .builder()
                    .title(&n.title)
                    .body(&n.body)
                    .show()
                    .map_err(|e| e.to_string());
                if let Err(e) = &shown {
                    // Said once: a computer without notices would fill the log otherwise.
                    if !failed_once.swap(true, Ordering::Relaxed) {
                        eprintln!("[plenipo] the system did not show a notice: {e}");
                    }
                }
                shown
            })
        }
        Output::Kept => {
            let kept = Arc::clone(&kept);
            Arc::new(move |n: &Notice| {
                kept.lock().map_err(|e| e.to_string())?.push(n.clone());
                Ok(())
            })
        }
    };
    let handle = app.clone();
    let in_front: InFront = Arc::new(move || {
        handle.get_webview_window("main").is_some_and(|w| {
            w.is_visible().unwrap_or(false)
                && w.is_focused().unwrap_or(false)
                && !w.is_minimized().unwrap_or(false)
        })
    });

    let (tx, rx) = mpsc::channel::<LedgerEvent>();
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if may_notify(&event.event_type) {
            let _ = tx.send(event.clone());
        }
    }));
    let label = move |id: &str| -> String {
        agents
            .as_ref()
            .and_then(|a| a.runtimes().into_iter().find(|r| r.id == id))
            .map_or_else(|| id.to_owned(), |r| r.label)
    };
    let show_loop = Arc::clone(&show);
    let spawned = std::thread::Builder::new()
        .name("plenipo-notices".into())
        .spawn(move || {
            let mut gate = NoticeGate::default();
            // Ends when the Ledger (and its listener) is gone.
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                let until = Instant::now() + gather;
                loop {
                    let left = until.saturating_duration_since(Instant::now());
                    match rx.recv_timeout(left) {
                        Ok(e) => batch.push(e),
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                let settings = ledger.notice_settings().unwrap_or_default();
                let notices: Vec<Notice> = batch
                    .iter()
                    .filter_map(|e| match ledger.notice_for(e, &label) {
                        Ok(n) => n,
                        Err(err) => {
                            eprintln!("[plenipo] could not read what an event means: {err}");
                            None
                        }
                    })
                    .filter(|n| settings.wants(n.kind))
                    .collect();
                if notices.is_empty() || (settings.only_when_away && in_front()) {
                    continue;
                }
                if let Some(notice) = gate.pass(notices, plenipo_ledger::now_ms()) {
                    let _ = show_loop(&notice);
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("[plenipo] notices are unavailable: {e}");
    }
    Notices { show, kept }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::{NewEvent, NewTask, NoticeSettings, TaskState};
    use serde_json::json;

    fn kept_after(notices: &Notices, count: usize) -> Vec<Notice> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let kept = notices.kept();
            if kept.len() >= count || Instant::now() > deadline {
                return kept;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn events_that_arrive_together_become_one_notice_of_the_kinds_the_owner_wants() {
        let app = tauri::test::mock_app();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let notices = start(
            app.handle(),
            Arc::clone(&ledger),
            None,
            Output::Kept,
            Duration::from_millis(200),
        );
        let task = ledger
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "Ship the release".into(),
                    priority: 2,
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        ledger
            .transition_task(&task.id, TaskState::Running, "worker", None)
            .unwrap();
        for summary in ["git push origin", "npm publish"] {
            ledger
                .request_action_approval(
                    &task.id,
                    "git.write",
                    &json!({ "worker": "Backend Developer", "summary": summary }),
                    u64::MAX,
                    "agent:codex",
                )
                .unwrap();
        }
        let kept = kept_after(&notices, 1);
        assert_eq!(kept.len(), 1, "{kept:#?}");
        assert_eq!(kept[0].title, "2 requests are waiting for your OK");
        assert_eq!(kept[0].body, "Git push origin\nNpm publish");

        // A kind the owner turned off makes no notice; the others still do.
        ledger
            .set_notice_settings(
                &NoticeSettings {
                    problems: false,
                    ..NoticeSettings::default()
                },
                "owner",
            )
            .unwrap();
        ledger
            .append_event(NewEvent {
                source: "guard".into(),
                event_type: "ssh.host_key_changed".into(),
                payload: json!({ "server": "Shop", "serverId": "s1" }),
                ..NewEvent::default()
            })
            .unwrap();
        ledger
            .append_event(NewEvent {
                task_id: Some(task.id.clone()),
                source: "agent:codex".into(),
                event_type: "lesson.added".into(),
                payload: json!({ "worker": "Backend Developer", "text": "Pull first.", "state": "waiting" }),
                ..NewEvent::default()
            })
            .unwrap();
        let kept = kept_after(&notices, 2);
        assert_eq!(kept.len(), 2, "{kept:#?}");
        assert_eq!(kept[1].title, "Backend Developer learned something");
        // Nothing else is on its way.
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(notices.kept().len(), 2);
    }

    #[test]
    fn the_test_notice_is_shown_whatever_the_choices() {
        let app = tauri::test::mock_app();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let notices = start(app.handle(), ledger, None, Output::Kept, GATHER);
        notices.show_now(test_notice()).unwrap();
        assert_eq!(notices.kept(), vec![test_notice()]);
    }
}
