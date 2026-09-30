//! A crashed or frozen window is brought back (Phase 13, ADR-037 item 3).
//!
//! The window's page runs in WebView2's own programs, so a crash there never stops the work;
//! but nothing would bring the window back. While the window is open, its page tells Plenipo
//! every few seconds that it is alive ([`WindowWatch::alive`]). When it goes quiet while it is
//! showing, Plenipo reloads the page; if that does not bring it back, it closes the window and
//! opens a new one. It records what it did and the window says so.
//!
//! "Showing" is careful on purpose: a hidden or minimized page may be slowed down by WebView2,
//! so it is watched only while the window is visible, not minimized, and either in front or
//! last reported by the page as visible.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use plenipo_ledger::{Ledger, NewEvent};
use serde_json::json;
use tauri::{AppHandle, Manager as _, Runtime};

use crate::recovery::RecoveryState;

/// Ledger event: the window's page stopped and was brought back.
pub const WINDOW_RECOVERED: &str = "plenipo.window_recovered";

/// How long the page may be quiet while showing before Plenipo steps in.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub quiet: Duration,
    pub every: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            quiet: Duration::from_secs(30),
            every: Duration::from_secs(5),
        }
    }
}

const WATCHING: u8 = 0;
const RELOADED: u8 = 1;
const REOPENED: u8 = 2;
const GAVE_UP: u8 = 3;

/// What the watch should do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Nothing,
    Reload,
    Reopen,
    GiveUp,
}

/// The watch over the main window's page.
#[derive(Default)]
pub struct WindowWatch {
    /// When the page last said it was alive (ms), 0 before the first time.
    last_alive: AtomicU64,
    /// The page last said it was visible.
    page_visible: AtomicBool,
    stage: AtomicU8,
    /// The window was showing at the last look (a window coming back into view gets a grace
    /// period, since its page was maybe slowed down while hidden).
    was_showing: AtomicBool,
    /// When the window was last closed to be opened again (ms): closing the only window must
    /// not quit Plenipo then.
    reopened_at: AtomicU64,
    /// When the watch last looked (ms). A long gap means the computer slept (or its clock
    /// jumped): the page gets its time again instead of being reloaded.
    last_step: AtomicU64,
}

/// How long after closing a window to reopen it a "last window closed" is not a quit.
const REOPEN_GRACE_MS: u64 = 15_000;

impl WindowWatch {
    /// The page is alive. Returns what was recovered, when this is the first sign of life after
    /// a reload (`Some(false)`) or a new window (`Some(true)`).
    pub fn alive(&self, visible: bool, now: u64) -> Option<bool> {
        self.last_alive.store(now, Ordering::SeqCst);
        self.page_visible.store(visible, Ordering::SeqCst);
        match self.stage.swap(WATCHING, Ordering::SeqCst) {
            RELOADED => Some(false),
            REOPENED => Some(true),
            _ => None,
        }
    }

    /// When the page last said it was alive (ms; 0: never).
    #[cfg(test)]
    pub fn last_alive(&self) -> u64 {
        self.last_alive.load(Ordering::SeqCst)
    }

    /// The window is being closed to be opened again (at `now`).
    pub fn reopening(&self, now: u64) {
        self.reopened_at.store(now, Ordering::SeqCst);
    }

    /// Plenipo is reopening its window now: the last window closing is not a quit.
    pub fn is_reopening(&self, now: u64) -> bool {
        let at = self.reopened_at.load(Ordering::SeqCst);
        at != 0 && now.saturating_sub(at) < REOPEN_GRACE_MS
    }

    /// Decide the next step. `showing`: the window is visible and not minimized; `focused`: it
    /// is in front.
    pub fn step(&self, showing: bool, focused: bool, now: u64, timing: Timing) -> Step {
        let quiet_ms = u64::try_from(timing.quiet.as_millis()).unwrap_or(u64::MAX);
        let previous_step = self.last_step.swap(now, Ordering::SeqCst);
        let last = self.last_alive.load(Ordering::SeqCst);
        let was_showing = self.was_showing.swap(showing, Ordering::SeqCst);
        if last == 0 {
            return Step::Nothing; // the page has not started yet
        }
        if previous_step != 0
            && (now < previous_step || now.saturating_sub(previous_step) >= quiet_ms)
        {
            // The watch itself was paused (the computer slept) or the clock jumped.
            self.last_alive.store(now, Ordering::SeqCst);
            return Step::Nothing;
        }
        if !showing {
            return Step::Nothing;
        }
        if !was_showing {
            // Just shown again: give the page its time.
            self.last_alive.store(now, Ordering::SeqCst);
            return Step::Nothing;
        }
        if !(focused || self.page_visible.load(Ordering::SeqCst)) {
            return Step::Nothing;
        }
        if now.saturating_sub(last) < quiet_ms {
            return Step::Nothing;
        }
        // Each step gets its own quiet period.
        self.last_alive.store(now, Ordering::SeqCst);
        match self.stage.load(Ordering::SeqCst) {
            WATCHING => {
                self.stage.store(RELOADED, Ordering::SeqCst);
                Step::Reload
            }
            RELOADED => {
                self.stage.store(REOPENED, Ordering::SeqCst);
                Step::Reopen
            }
            REOPENED => {
                self.stage.store(GAVE_UP, Ordering::SeqCst);
                Step::GiveUp
            }
            _ => Step::Nothing,
        }
    }
}

/// Record that the window was brought back, for the Activity trail and the window.
pub fn record(ledger: &Ledger, state: &RecoveryState, reopened: bool) {
    state.window_recovered(reopened);
    log::warn!(
        "the window stopped responding and was {}",
        if reopened { "opened again" } else { "reloaded" }
    );
    let event = NewEvent {
        source: "plenipo".into(),
        event_type: WINDOW_RECOVERED.into(),
        payload: json!({ "reopened": reopened }),
        ..NewEvent::default()
    };
    if let Err(e) = ledger.append_event(event) {
        log::warn!("the window's recovery could not be recorded: {e}");
    }
}

/// Watch the main window until Plenipo quits.
pub fn start<R: Runtime>(app: &AppHandle<R>, watch: Arc<WindowWatch>, timing: Timing) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("plenipo-window-watch".into())
        .spawn(move || loop {
            std::thread::sleep(timing.every);
            let window = app.get_webview_window("main");
            let (showing, focused) = window.as_ref().map_or((false, false), |w| {
                let visible = w.is_visible().unwrap_or(false);
                let minimized = w.is_minimized().unwrap_or(false);
                (visible && !minimized, w.is_focused().unwrap_or(false))
            });
            match watch.step(showing, focused, plenipo_ledger::now_ms(), timing) {
                Step::Nothing => {}
                Step::Reload => {
                    log::warn!("the window has not answered for a while; reloading it");
                    if let Some(w) = window {
                        if let Err(e) = w.reload() {
                            log::warn!("the window could not be reloaded: {e}");
                        }
                    }
                }
                Step::Reopen => {
                    log::warn!("the window did not come back; opening it again");
                    watch.reopening(plenipo_ledger::now_ms());
                    if let Some(w) = window {
                        let _ = w.destroy();
                    }
                    // The old window must be gone before the new one takes its name.
                    for _ in 0..100 {
                        if app.get_webview_window("main").is_none() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    crate::start_close::show_main_window(&app);
                    watch.reopening(plenipo_ledger::now_ms());
                }
                Step::GiveUp => {
                    log::error!(
                        "the window could not be brought back; quit Plenipo from the tray and \
                         open it again"
                    );
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Timing = Timing {
        quiet: Duration::from_secs(30),
        every: Duration::from_secs(5),
    };

    /// Look every 5 seconds, as the watch does, from `from` to `to`: the steps taken, with when.
    fn watch(w: &WindowWatch, from: u64, to: u64, focused: bool) -> Vec<(u64, Step)> {
        (from..=to)
            .step_by(5_000)
            .map(|now| (now, w.step(true, focused, now, T)))
            .filter(|(_, s)| *s != Step::Nothing)
            .collect()
    }

    #[test]
    fn a_quiet_page_is_reloaded_then_reopened_then_left() {
        let w = WindowWatch::default();
        // Nothing before the page has started.
        assert_eq!(w.step(true, true, 100_000, T), Step::Nothing);
        w.alive(true, 100_000);
        // Quiet for 30 seconds: reloaded; each step gets its own quiet period; then left.
        assert_eq!(
            watch(&w, 105_000, 400_000, true),
            [
                (130_000, Step::Reload),
                (160_000, Step::Reopen),
                (190_000, Step::GiveUp)
            ]
        );
    }

    #[test]
    fn the_first_sign_of_life_after_a_step_says_what_brought_it_back() {
        let w = WindowWatch::default();
        w.alive(true, 1_000);
        assert_eq!(watch(&w, 5_000, 35_000, true), [(35_000, Step::Reload)]);
        assert_eq!(w.alive(true, 36_000), Some(false), "reloaded");
        assert_eq!(w.alive(true, 37_000), None, "said once");
        assert_eq!(watch(&w, 40_000, 70_000, true), [(70_000, Step::Reload)]);
        assert_eq!(watch(&w, 75_000, 100_000, true), [(100_000, Step::Reopen)]);
        assert_eq!(w.alive(true, 101_000), Some(true), "reopened");
    }

    #[test]
    fn a_hidden_or_minimized_window_is_not_watched_and_gets_time_when_shown_again() {
        let w = WindowWatch::default();
        w.alive(true, 1_000);
        w.step(true, true, 2_000, T);
        // Hidden to the tray for an hour: nothing.
        assert_eq!(w.step(false, false, 3_600_000, T), Step::Nothing);
        // Shown again: a grace period first.
        assert_eq!(w.step(true, true, 3_600_500, T), Step::Nothing);
        assert_eq!(w.step(true, true, 3_620_000, T), Step::Nothing);
        assert_eq!(w.step(true, true, 3_631_000, T), Step::Reload);
    }

    #[test]
    fn waking_from_sleep_gives_the_page_its_time_again() {
        let w = WindowWatch::default();
        w.alive(true, 1_000);
        w.step(true, true, 2_000, T);
        w.step(true, true, 7_000, T);
        // The computer slept for an hour: the first look after it is not a reason to reload.
        assert_eq!(w.step(true, true, 3_607_000, T), Step::Nothing);
        assert_eq!(w.step(true, true, 3_612_000, T), Step::Nothing);
        // A page that still does not answer is reloaded after its quiet time.
        let mut now = 3_612_000;
        let mut step = Step::Nothing;
        while step == Step::Nothing && now < 3_700_000 {
            now += 5_000;
            step = w.step(true, true, now, T);
        }
        assert_eq!(step, Step::Reload);
        assert!(now >= 3_637_000, "{now}");
        // A clock set back is not a reason either.
        let w = WindowWatch::default();
        w.alive(true, 50_000);
        w.step(true, true, 51_000, T);
        assert_eq!(w.step(true, true, 10_000, T), Step::Nothing);
    }

    #[test]
    fn closing_the_window_to_reopen_it_is_not_a_quit_for_a_while() {
        let w = WindowWatch::default();
        assert!(!w.is_reopening(1_000));
        w.reopening(1_000);
        assert!(w.is_reopening(1_500));
        assert!(w.is_reopening(1_000 + REOPEN_GRACE_MS - 1));
        assert!(!w.is_reopening(1_000 + REOPEN_GRACE_MS));
    }

    #[test]
    fn a_page_that_said_it_is_hidden_is_watched_only_when_in_front() {
        let w = WindowWatch::default();
        w.alive(false, 1_000); // covered by other windows: WebView2 may slow its timers
        assert!(watch(&w, 2_000, 100_000, false).is_empty());
        // In front, it must answer.
        assert_eq!(w.step(true, true, 102_000, T), Step::Reload);
    }
}
