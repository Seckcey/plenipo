//! Launch smoke-test support for CI.
//!
//! With `PLENIPO_SMOKE_TEST=1` the app launches normally, waits for the UI to
//! call `frontend_ready`, and exits 0. If the UI never reports ready (render
//! failure, runtime error, IPC failure) a watchdog exits with code 1.
//!
//! Phase 13's Windows installer tests add, only in smoke mode:
//! - `PLENIPO_SMOKE_REPORT=<file>`: when ready, write what the app found (version, how the last
//!   run ended, backups, updates) to that file as JSON, for the test to check;
//! - `PLENIPO_SMOKE_SCENARIO`: `stay` (report and keep running, idle), `start-work` (start a
//!   long program and a task, report, and keep running until the test ends Plenipo),
//!   `window-crash` (keep running until the window is brought back after the test ends its
//!   WebView2 programs), or `update` (check for an update and install it).

use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Runtime};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Recorded outcome before the frontend has reported anything.
const PENDING: i32 = -1;
pub const EXIT_READY: i32 = 0;
pub const EXIT_TIMEOUT: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeMode {
    Disabled,
    Enabled { timeout: Duration },
}

impl SmokeMode {
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("PLENIPO_SMOKE_TEST").ok().as_deref(),
            std::env::var("PLENIPO_SMOKE_TIMEOUT_SECS").ok().as_deref(),
        )
    }

    /// Pure parser so behavior is testable without touching process env.
    pub fn parse(flag: Option<&str>, timeout_secs: Option<&str>) -> Self {
        match flag.map(str::trim) {
            Some("1") | Some("true") => {
                let timeout = timeout_secs
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .filter(|&s| (1..=600).contains(&s))
                    .map(Duration::from_secs)
                    .unwrap_or(DEFAULT_TIMEOUT);
                Self::Enabled { timeout }
            }
            _ => Self::Disabled,
        }
    }
}

/// What a smoke run does once the window is ready (Phase 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Report and exit 0 (the launch smoke test).
    Ready,
    /// Report and keep running with nothing to do (the test ends Plenipo: idle).
    Stay,
    /// Start a long program and a task, report, and keep running (the test ends Plenipo).
    StartWork,
    /// Keep running until the window is brought back, then report and exit 0.
    WindowCrash,
    /// Check for an update and install it.
    Update,
}

impl Scenario {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("stay") => Self::Stay,
            Some("start-work") => Self::StartWork,
            Some("window-crash") => Self::WindowCrash,
            Some("update") => Self::Update,
            _ => Self::Ready,
        }
    }
}

/// Smoke-test mode plus the recorded outcome. Cheap to clone; clones share state.
#[derive(Debug, Clone)]
pub struct SmokeTest {
    mode: SmokeMode,
    outcome: Arc<AtomicI32>,
    scenario: Scenario,
    report: Option<PathBuf>,
    readies: Arc<AtomicU32>,
}

impl SmokeTest {
    pub fn new(mode: SmokeMode) -> Self {
        Self::with_scenario(mode, Scenario::Ready, None)
    }

    pub fn with_scenario(mode: SmokeMode, scenario: Scenario, report: Option<PathBuf>) -> Self {
        Self {
            mode,
            outcome: Arc::new(AtomicI32::new(PENDING)),
            scenario,
            report,
            readies: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn from_env() -> Self {
        let mode = SmokeMode::from_env();
        if mode == SmokeMode::Disabled {
            return Self::new(mode);
        }
        Self::with_scenario(
            mode,
            Scenario::parse(std::env::var("PLENIPO_SMOKE_SCENARIO").ok().as_deref()),
            std::env::var_os("PLENIPO_SMOKE_REPORT")
                .map(PathBuf::from)
                .filter(|p| !p.as_os_str().is_empty()),
        )
    }

    pub fn scenario(&self) -> Scenario {
        self.scenario
    }

    /// Where to write the report, if the test asked for one.
    pub fn report_path(&self) -> Option<&PathBuf> {
        self.report.as_ref()
    }

    /// Count a "ready" from the window; returns how many there have been, this one included.
    pub fn ready_count(&self) -> u32 {
        self.readies.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn mode(&self) -> SmokeMode {
        self.mode
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self.mode, SmokeMode::Enabled { .. })
    }

    /// Record an outcome. The first outcome wins, so a late watchdog cannot
    /// override "ready" and a late "ready" cannot rescue a timeout.
    pub fn record(&self, code: i32) -> bool {
        self.outcome
            .compare_exchange(PENDING, code, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Final process exit code: a non-zero runtime code always wins; otherwise
    /// the recorded smoke outcome (0 if smoke mode never recorded anything).
    pub fn resolve_exit_code(&self, runtime_code: i32) -> i32 {
        if runtime_code != 0 {
            return runtime_code;
        }
        match self.outcome.load(Ordering::SeqCst) {
            PENDING => 0,
            code => code,
        }
    }

    /// Exit with failure if the frontend has not reported ready within `timeout`.
    pub fn arm_watchdog<R: Runtime>(&self, app: AppHandle<R>, timeout: Duration) {
        log::info!("smoke test mode: waiting up to {timeout:?} for frontend");
        let this = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            if !this.record(EXIT_TIMEOUT) {
                return; // frontend already reported ready
            }
            log::info!("smoke test FAILED: frontend did not report ready in {timeout:?}");
            app.exit(EXIT_TIMEOUT);
            // `exit` asks the event loop to stop; force it if the loop is wedged.
            std::thread::sleep(Duration::from_secs(5));
            std::process::exit(EXIT_TIMEOUT);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default() {
        assert_eq!(SmokeMode::parse(None, None), SmokeMode::Disabled);
        assert_eq!(SmokeMode::parse(Some("0"), None), SmokeMode::Disabled);
        assert_eq!(SmokeMode::parse(Some("yes"), None), SmokeMode::Disabled);
    }

    #[test]
    fn enabled_with_default_timeout() {
        assert_eq!(
            SmokeMode::parse(Some("1"), None),
            SmokeMode::Enabled {
                timeout: DEFAULT_TIMEOUT
            }
        );
        assert!(SmokeTest::new(SmokeMode::parse(Some("true"), None)).is_enabled());
    }

    #[test]
    fn custom_timeout_is_bounded() {
        assert_eq!(
            SmokeMode::parse(Some("1"), Some("30")),
            SmokeMode::Enabled {
                timeout: Duration::from_secs(30)
            }
        );
        for bad in ["0", "601", "-5", "abc"] {
            assert_eq!(
                SmokeMode::parse(Some("1"), Some(bad)),
                SmokeMode::Enabled {
                    timeout: DEFAULT_TIMEOUT
                },
                "timeout {bad:?} should fall back to default"
            );
        }
    }

    #[test]
    fn exit_code_is_zero_when_nothing_recorded() {
        let smoke = SmokeTest::new(SmokeMode::Disabled);
        assert_eq!(smoke.resolve_exit_code(0), 0);
    }

    #[test]
    fn recorded_timeout_overrides_zero_runtime_code() {
        let smoke = SmokeTest::new(SmokeMode::parse(Some("1"), None));
        assert!(smoke.clone().record(EXIT_TIMEOUT));
        assert_eq!(smoke.resolve_exit_code(0), EXIT_TIMEOUT);
    }

    #[test]
    fn first_outcome_wins() {
        let smoke = SmokeTest::new(SmokeMode::parse(Some("1"), None));
        assert!(smoke.record(EXIT_READY));
        assert!(!smoke.record(EXIT_TIMEOUT));
        assert_eq!(smoke.resolve_exit_code(0), EXIT_READY);

        let late_ready = SmokeTest::new(SmokeMode::parse(Some("1"), None));
        assert!(late_ready.record(EXIT_TIMEOUT));
        assert!(!late_ready.record(EXIT_READY));
        assert_eq!(late_ready.resolve_exit_code(0), EXIT_TIMEOUT);
    }

    #[test]
    fn scenarios_are_read_from_their_names() {
        assert_eq!(Scenario::parse(None), Scenario::Ready);
        assert_eq!(Scenario::parse(Some("stay")), Scenario::Stay);
        assert_eq!(Scenario::parse(Some("start-work")), Scenario::StartWork);
        assert_eq!(Scenario::parse(Some("window-crash")), Scenario::WindowCrash);
        assert_eq!(Scenario::parse(Some(" update ")), Scenario::Update);
        assert_eq!(Scenario::parse(Some("anything")), Scenario::Ready);
        let smoke = SmokeTest::new(SmokeMode::parse(Some("1"), None));
        assert_eq!(smoke.ready_count(), 1);
        assert_eq!(smoke.clone().ready_count(), 2, "clones share the count");
    }

    #[test]
    fn nonzero_runtime_code_always_wins() {
        let smoke = SmokeTest::new(SmokeMode::parse(Some("1"), None));
        smoke.record(EXIT_READY);
        assert_eq!(smoke.resolve_exit_code(3), 3);
    }
}
