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
/// At most this many notices wait during Do not disturb (the oldest go first); they come as one.
const MAX_HELD: usize = 200;
/// Recorded when the owner's status changes: ending Do not disturb shows what waited.
const OWNER_CHANGED: &str = "owner.profile_changed";

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

/// Whose notices these are (Phase 21, ADR-094): one organization's.
#[derive(Clone)]
pub struct NoticeOrg {
    /// Its ID. Another organization's notices start with its name.
    pub id: String,
    /// Where your tile is kept (the first organization's Ledger), when it is not this one's.
    pub home: Option<Arc<Ledger>>,
}

impl NoticeOrg {
    /// The first organization's.
    pub fn first() -> Self {
        Self {
            id: crate::orgs::FIRST.into(),
            home: None,
        }
    }
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
    org: NoticeOrg,
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
                        log::warn!("the system did not show a notice: {e}");
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
    // In front: the window showing this organization (Phase 21).
    let handle = app.clone();
    let id = org.id.clone();
    let in_front: InFront = Arc::new(move || {
        let label = handle
            .try_state::<Arc<crate::orgs::Orgs>>()
            .map_or_else(|| Some("main".to_owned()), |orgs| orgs.window_of(&id));
        label
            .and_then(|l| handle.get_webview_window(&l))
            .is_some_and(|w| {
                w.is_visible().unwrap_or(false)
                    && w.is_focused().unwrap_or(false)
                    && !w.is_minimized().unwrap_or(false)
            })
    });
    // Another organization's notices say which organization they are from.
    let handle = app.clone();
    let id = org.id.clone();
    let named = move |mut n: Notice| -> Notice {
        if id == crate::orgs::FIRST {
            return n;
        }
        if let Some(entry) = handle
            .try_state::<Arc<crate::orgs::Orgs>>()
            .and_then(|orgs| orgs.entry(&id))
            .filter(|e| !e.name.is_empty())
        {
            n.title = format!("{}: {}", entry.name, n.title);
        }
        n
    };

    let (tx, rx) = mpsc::channel::<LedgerEvent>();
    // Your status (Do not disturb) is kept with your tile: in the first organization's Ledger.
    let home = org.home.clone();
    if let Some(home) = &home {
        let tx = tx.clone();
        home.add_listener(Arc::new(move |event: &LedgerEvent| {
            if event.event_type == OWNER_CHANGED {
                let _ = tx.send(event.clone());
            }
        }));
    }
    let home = home.map(|h| Arc::downgrade(&h));
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if may_notify(&event.event_type) || event.event_type == OWNER_CHANGED {
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
    // The thread holds the Ledger only while it reads a batch: the Ledger holds the listener,
    // and the listener holds the sender, so when the Ledger is gone the channel closes and the
    // thread ends.
    let ledger = Arc::downgrade(&ledger);
    let spawned = std::thread::Builder::new()
        .name("plenipo-notices".into())
        .spawn(move || {
            let mut gate = NoticeGate::default();
            // What waited while the owner was not to be disturbed.
            let mut held: Vec<Notice> = Vec::new();
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
                let Some(ledger) = ledger.upgrade() else {
                    break;
                };
                let settings = ledger.notice_settings().unwrap_or_default();
                let mut notices: Vec<Notice> = batch
                    .iter()
                    .filter_map(|e| match ledger.notice_for(e, &label) {
                        Ok(n) => n,
                        Err(err) => {
                            log::warn!("could not read what an event means: {err}");
                            None
                        }
                    })
                    .filter(|n| settings.wants(n.kind))
                    .collect();
                // Do not disturb (ADR-056): Windows' pop-ups wait, and come as one when it ends;
                // the bell still counts them.
                let tile = home.as_ref().and_then(std::sync::Weak::upgrade);
                let quiet = plenipo_workforce::owner::profile(tile.as_deref().unwrap_or(&ledger))
                    .is_ok_and(|p| p.status == plenipo_workforce::OwnerStatus::DoNotDisturb);
                if quiet {
                    held.append(&mut notices);
                    let over = held.len().saturating_sub(MAX_HELD);
                    held.drain(..over);
                    continue;
                }
                if !held.is_empty() {
                    held.append(&mut notices);
                    notices = std::mem::take(&mut held);
                }
                if notices.is_empty() || (settings.only_when_away && in_front()) {
                    continue;
                }
                if let Some(notice) = gate.pass(notices, plenipo_ledger::now_ms()) {
                    let _ = show_loop(&named(notice));
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("notices are unavailable: {e}");
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
            NoticeOrg::first(),
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
    fn do_not_disturb_holds_the_pop_ups() {
        use plenipo_workforce::{OwnerProfileInput, OwnerStatus, PictureChange};
        let app = tauri::test::mock_app();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let notices = start(
            app.handle(),
            Arc::clone(&ledger),
            None,
            Output::Kept,
            Duration::from_millis(100),
            NoticeOrg::first(),
        );
        let status = |status| {
            plenipo_workforce::owner::set_profile(
                &ledger,
                &OwnerProfileInput {
                    status,
                    mood: None,
                    message: String::new(),
                    picture: PictureChange::Keep,
                },
            )
            .unwrap();
        };
        let learned = |text: &str| {
            ledger
                .append_event(NewEvent {
                    source: "agent:codex".into(),
                    event_type: "lesson.added".into(),
                    payload: json!({ "worker": "Backend Developer", "text": text, "state": "waiting" }),
                    ..NewEvent::default()
                })
                .unwrap();
        };
        let asked = |summary: &str| {
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
            ledger
                .request_action_approval(
                    &task.id,
                    "git.write",
                    &json!({ "worker": "Backend Developer", "summary": summary }),
                    u64::MAX,
                    "agent:codex",
                )
                .unwrap();
        };
        status(OwnerStatus::DoNotDisturb);
        learned("Pull first.");
        std::thread::sleep(Duration::from_millis(300));
        asked("git push origin");
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            notices.kept().is_empty(),
            "held while you are not to be disturbed"
        );
        // Ending it shows what waited, as one notice, and nothing is lost.
        status(OwnerStatus::Busy);
        let kept = kept_after(&notices, 1);
        assert_eq!(kept.len(), 1, "{kept:#?}");
        assert_eq!(kept[0].title, "2 things need you");
        assert!(kept[0].body.contains("Backend Developer learned something"));
        assert!(
            kept[0].body.contains("waiting for your OK"),
            "{}",
            kept[0].body
        );
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(notices.kept().len(), 1, "shown once");
        // Busy still shows them as they come.
        learned("Run the tests.");
        assert_eq!(kept_after(&notices, 2).len(), 2, "busy still shows them");
    }

    #[test]
    fn the_notices_thread_does_not_keep_the_ledger() {
        let app = tauri::test::mock_app();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let _notices = start(
            app.handle(),
            Arc::clone(&ledger),
            None,
            Output::Kept,
            GATHER,
            NoticeOrg::first(),
        );
        // Only this test holds the Ledger: when it goes, so do its listener and the channel,
        // and the thread ends.
        assert_eq!(Arc::strong_count(&ledger), 1);
        let gone = Arc::downgrade(&ledger);
        drop(ledger);
        assert!(gone.upgrade().is_none());
    }

    #[test]
    fn the_test_notice_is_shown_whatever_the_choices() {
        let app = tauri::test::mock_app();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let notices = start(
            app.handle(),
            ledger,
            None,
            Output::Kept,
            GATHER,
            NoticeOrg::first(),
        );
        notices.show_now(test_notice()).unwrap();
        assert_eq!(notices.kept(), vec![test_notice()]);
    }
}
