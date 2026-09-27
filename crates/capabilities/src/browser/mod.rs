//! Plenipo's browser (Phase 10, ADR-020): a copy of Microsoft Edge or Google Chrome that Plenipo
//! starts and controls, with its own profile in Plenipo's data folder, separate from the
//! owner's own browser, sign-ins, and passwords.
//!
//! - It runs through the supervisor like every program Plenipo starts (its own process tree,
//!   stopped when Plenipo quits), and is always visible: never hidden, with the browser's own
//!   "controlled by automated test software" bar and Plenipo's sign on every page a worker uses.
//! - Plenipo talks to it over the DevTools protocol on a loopback port ([`cdp`]); each worker's
//!   step gets its own tab ([`tab`]).
//! - The profile never saves passwords or card details.
//! - If it crashes or is closed, the next browser tool call starts it again.

pub mod cdp;
pub mod classify;
pub mod tab;

use std::path::{Path, PathBuf};
use std::time::Duration;

use plenipo_runtime::{LaunchSpec, Supervisor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;
use ts_rs::TS;

use self::cdp::Cdp;
use self::tab::{Signals, SitePolicy, Tab, TabLimits};

/// Variables the browser gets besides the supervisor's baseline (a display on Linux).
const BROWSER_ENV: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// The browser to use (`PLENIPO_BROWSER`, or tests); otherwise Edge or Chrome is found.
    pub executable: Option<PathBuf>,
    /// Plenipo's own profile folder.
    pub profile_dir: PathBuf,
    /// Tests only: no window. The app always shows the browser.
    pub headless: bool,
    /// Tests only: more command-line options.
    pub extra_args: Vec<String>,
    pub launch_timeout: Duration,
    pub limits: TabLimits,
}

impl BrowserConfig {
    pub fn new(profile_dir: PathBuf) -> Self {
        Self {
            executable: std::env::var_os("PLENIPO_BROWSER")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute()),
            profile_dir,
            headless: false,
            extra_args: Vec::new(),
            launch_timeout: Duration::from_secs(30),
            limits: TabLimits::default(),
        }
    }
}

/// Plenipo's browser as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowserStatus {
    /// The browser Plenipo uses ("Microsoft Edge"), when one is installed.
    #[ts(optional)]
    pub name: Option<String>,
    #[ts(optional)]
    pub path: Option<String>,
    pub running: bool,
    /// Its own profile folder (sign-ins made there stay there).
    pub profile: String,
    #[ts(optional)]
    pub problem: Option<String>,
}

/// A browser's plain name from its program's name.
fn name_of(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem.contains("edge") {
        "Microsoft Edge".into()
    } else if stem.contains("chromium") || path.to_string_lossy().contains("chromium") {
        "Chromium".into()
    } else if stem.contains("chrome") {
        "Google Chrome".into()
    } else {
        stem
    }
}

/// Edge or Chrome on this computer: the configured one, then the usual places.
pub fn find_browser(configured: Option<&Path>) -> Option<(PathBuf, String)> {
    if let Some(p) = configured {
        return p.is_file().then(|| (p.to_path_buf(), name_of(p)));
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
                candidates.push(base.join("Microsoft/Edge/Application/msedge.exe"));
                candidates.push(base.join("Google/Chrome/Application/chrome.exe"));
            }
        }
    } else {
        for name in [
            "microsoft-edge",
            "microsoft-edge-stable",
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ] {
            if let Some(p) = crate::programs::find_on_path(name) {
                candidates.push(p);
            }
        }
        candidates.push(PathBuf::from(
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ));
        candidates.push(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
    }
    candidates.into_iter().find(|p| p.is_file()).map(|p| {
        let name = name_of(&p);
        (p, name)
    })
}

/// Make the profile Plenipo's own: it never offers to save passwords or card details.
fn prepare_profile(dir: &Path) -> std::io::Result<()> {
    let default = dir.join("Default");
    std::fs::create_dir_all(&default)?;
    let prefs_file = default.join("Preferences");
    let mut prefs: Value = std::fs::read_to_string(&prefs_file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    prefs["credentials_enable_service"] = json!(false);
    prefs["credentials_enable_autosignin"] = json!(false);
    if !prefs["profile"].is_object() {
        prefs["profile"] = json!({});
    }
    prefs["profile"]["password_manager_enabled"] = json!(false);
    if !prefs["autofill"].is_object() {
        prefs["autofill"] = json!({});
    }
    prefs["autofill"]["profile_enabled"] = json!(false);
    prefs["autofill"]["credit_card_enabled"] = json!(false);
    std::fs::write(&prefs_file, prefs.to_string())?;
    let _ = std::fs::remove_file(dir.join("DevToolsActivePort"));
    Ok(())
}

/// Running as root on Linux (a container), where Chrome needs `--no-sandbox`.
fn root_on_linux() -> bool {
    cfg!(target_os = "linux")
        && std::fs::read_to_string("/proc/self/status").is_ok_and(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(2))
                == Some("0")
        })
}

/// The command-line options Plenipo starts the browser with.
fn arguments(config: &BrowserConfig) -> Vec<String> {
    let mut args: Vec<String> = [
        format!("--user-data-dir={}", config.profile_dir.display()),
        "--remote-debugging-port=0".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        // Shows the browser's own "controlled by automated test software" bar.
        "--enable-automation".into(),
        "--disable-sync".into(),
        "--disable-extensions".into(),
        "--disable-features=PasswordManagerOnboarding,AutofillServerCommunication,Translate".into(),
        "--window-size=1280,900".into(),
    ]
    .into();
    if cfg!(target_os = "linux") {
        // Never touch the desktop's keyring.
        args.push("--password-store=basic".into());
    }
    if root_on_linux() {
        args.push("--no-sandbox".into());
    }
    if config.headless {
        args.push("--headless=new".into());
    }
    args.extend(config.extra_args.iter().cloned());
    args.push("about:blank".into());
    args
}

struct Running {
    cdp: Cdp,
    execution_id: String,
}

/// Why one start gave no browser to connect to.
enum LaunchError {
    /// It started but did not get ready in time, and has been stopped. Worth one more start.
    NotReady(String),
    /// Anything else: not found, could not start, closed at once, could not connect.
    Failed(String),
}

impl From<String> for LaunchError {
    fn from(e: String) -> Self {
        Self::Failed(e)
    }
}

impl From<&str> for LaunchError {
    fn from(e: &str) -> Self {
        Self::Failed(e.to_owned())
    }
}

/// How the browser was when a tool call needed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// It was running.
    Running,
    /// It was started now.
    Started,
    /// It had stopped (crashed or closed) and was started again.
    Restarted,
}

/// Plenipo's browser. Cheap to clone; clones share it.
#[derive(Clone)]
pub struct Browser {
    inner: std::sync::Arc<Inner>,
}

struct Inner {
    config: BrowserConfig,
    supervisor: Supervisor,
    running: AsyncMutex<Option<Running>>,
    problem: std::sync::Mutex<Option<String>>,
}

impl Browser {
    pub fn new(config: BrowserConfig, supervisor: Supervisor) -> Self {
        Self {
            inner: std::sync::Arc::new(Inner {
                config,
                supervisor,
                running: AsyncMutex::new(None),
                problem: std::sync::Mutex::new(None),
            }),
        }
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.inner.config
    }

    pub fn find(&self) -> Option<(PathBuf, String)> {
        find_browser(self.inner.config.executable.as_deref())
    }

    /// What Settings shows.
    pub async fn status(&self) -> BrowserStatus {
        let found = self.find();
        let running = self
            .inner
            .running
            .lock()
            .await
            .as_ref()
            .is_some_and(|r| !r.cdp.is_closed());
        BrowserStatus {
            name: found.as_ref().map(|(_, n)| n.clone()),
            path: found.as_ref().map(|(p, _)| p.display().to_string()),
            running,
            profile: self.inner.config.profile_dir.display().to_string(),
            problem: if found.is_none() {
                Some(
                    "No Microsoft Edge or Google Chrome was found on this computer, so workers \
                     cannot use websites. Windows 11 includes Edge; if it was removed, install \
                     Edge or Chrome."
                        .into(),
                )
            } else {
                self.inner
                    .problem
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone()
            },
        }
    }

    /// The connection to the running browser, starting it when needed.
    pub async fn connection(&self) -> Result<(Cdp, Start), String> {
        let mut running = self.inner.running.lock().await;
        let start = match running.as_ref() {
            Some(r) if !r.cdp.is_closed() => return Ok((r.cdp.clone(), Start::Running)),
            Some(r) => {
                // It stopped: make sure its process is gone before starting again.
                let _ = self.inner.supervisor.cancel(&r.execution_id).await;
                Start::Restarted
            }
            None => Start::Started,
        };
        *running = None;
        match self.launch().await {
            Ok(r) => {
                let cdp = r.cdp.clone();
                *running = Some(r);
                *self.inner.problem.lock().unwrap_or_else(|p| p.into_inner()) = None;
                Ok((cdp, start))
            }
            Err(e) => {
                *self.inner.problem.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Start the browser. A first start can be slow — a cold disk, a busy computer, a new
    /// profile — so a browser that does not get ready in time is stopped and started once more,
    /// and the second start finds all of that warm. Anything else that goes wrong is not tried
    /// again: a second start would fail the same way.
    async fn launch(&self) -> Result<Running, String> {
        let first = match self.launch_once().await {
            Ok(running) => return Ok(running),
            Err(LaunchError::Failed(e)) => return Err(e),
            Err(LaunchError::NotReady(e)) => e,
        };
        match self.launch_once().await {
            Ok(running) => Ok(running),
            Err(LaunchError::NotReady(_)) => Err(format!("{first}, and again on a second try")),
            Err(LaunchError::Failed(e)) => Err(e),
        }
    }

    async fn launch_once(&self) -> Result<Running, LaunchError> {
        let config = &self.inner.config;
        let (executable, name) = self.find().ok_or(
            "no Microsoft Edge or Google Chrome was found on this computer, so Plenipo's browser \
             cannot start",
        )?;
        prepare_profile(&config.profile_dir)
            .map_err(|e| format!("could not prepare the browser's profile: {e}"))?;
        let sup = &self.inner.supervisor;
        let executable = sup
            .allow_executable(&executable)
            .map_err(|e| format!("{} cannot be started: {e}", executable.display()))?;
        let env: Vec<(String, String)> = BROWSER_ENV
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_owned(), v)))
            .collect();
        let record = sup
            .launch(LaunchSpec {
                profile_id: "capability.browser".into(),
                label: format!("Plenipo's browser ({name})"),
                executable,
                args: arguments(config),
                env,
                working_dir: config.profile_dir.clone(),
                max_runtime: plenipo_runtime::profile::MAX_RUNTIME_LIMIT,
                stdin: None,
                stdin_feed: None,
                max_line_bytes: None,
                observer: None,
                agent: None,
            })
            .await
            .map_err(|e| format!("{name} could not be started: {e}"))?;
        let deadline = tokio::time::Instant::now() + config.launch_timeout;
        let port_file = config.profile_dir.join("DevToolsActivePort");
        let address = loop {
            if let Ok(text) = std::fs::read_to_string(&port_file) {
                let mut lines = text.lines();
                if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
                    if port.trim().parse::<u16>().is_ok() && path.starts_with("/devtools/") {
                        break format!("ws://127.0.0.1:{}{}", port.trim(), path.trim());
                    }
                }
            }
            let ended = sup
                .overview()
                .executions
                .iter()
                .find(|e| e.id == record.id)
                .is_none_or(|e| e.state.is_terminal());
            if ended {
                return Err(LaunchError::Failed(format!(
                    "{name} closed right after it started (is Plenipo's browser already open \
                     from an earlier start? Close it and try again)"
                )));
            }
            if tokio::time::Instant::now() >= deadline {
                let _ = sup.cancel(&record.id).await;
                return Err(LaunchError::NotReady(format!(
                    "{name} did not get ready within {} seconds",
                    config.launch_timeout.as_secs()
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let (cdp, mut browser_events) = match Cdp::connect(&address).await {
            Ok(c) => c,
            Err(e) => {
                let _ = sup.cancel(&record.id).await;
                return Err(LaunchError::Failed(e));
            }
        };
        // The browser's own events are not needed; drain them.
        tokio::spawn(async move { while browser_events.recv().await.is_some() {} });
        Ok(Running {
            cdp,
            execution_id: record.id,
        })
    }

    /// A new tab for `worker`, and how the browser was.
    pub async fn new_tab(
        &self,
        worker: &str,
        policy: SitePolicy,
        signals: Signals,
    ) -> Result<(Tab, Start), String> {
        let (cdp, start) = self.connection().await?;
        let tab = Tab::open(cdp, self.inner.config.limits, worker, policy, signals).await?;
        Ok((tab, start))
    }

    /// Open a page in a new tab for the owner (to sign in to a website, say): not checked, no
    /// worker. The tab is left to the owner.
    pub async fn open_for_owner(&self, url: &str) -> Result<(), String> {
        let (cdp, _) = self.connection().await?;
        let t = Duration::from_secs(10);
        let target = cdp
            .call(None, "Target.createTarget", json!({ "url": url }), t)
            .await?;
        let id = target["targetId"].as_str().unwrap_or_default().to_owned();
        let _ = cdp
            .call(None, "Target.activateTarget", json!({ "targetId": id }), t)
            .await;
        Ok(())
    }

    /// The running browser's program run, if any (so tests can stop it like a crash).
    pub async fn execution_id(&self) -> Option<String> {
        self.inner
            .running
            .lock()
            .await
            .as_ref()
            .filter(|r| !r.cdp.is_closed())
            .map(|r| r.execution_id.clone())
    }

    /// Close the browser (Plenipo quitting, or tests).
    pub async fn close(&self) {
        if let Some(r) = self.inner.running.lock().await.take() {
            let _ = r
                .cdp
                .call(None, "Browser.close", json!({}), Duration::from_secs(5))
                .await;
            let _ = self.inner.supervisor.cancel(&r.execution_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_options() {
        assert_eq!(
            name_of(Path::new("C:/x/Microsoft/Edge/Application/msedge.exe")),
            "Microsoft Edge"
        );
        assert_eq!(
            name_of(Path::new("/usr/bin/google-chrome")),
            "Google Chrome"
        );
        assert_eq!(
            name_of(Path::new(
                "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"
            )),
            "Chromium"
        );
        let dir = tempfile::tempdir().unwrap();
        let config = BrowserConfig {
            headless: true,
            ..BrowserConfig::new(dir.path().to_path_buf())
        };
        let args = arguments(&config);
        assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
        assert!(args.contains(&"--remote-debugging-port=0".to_owned()));
        assert!(args.contains(&"--enable-automation".to_owned()));
        assert!(args.contains(&"--headless=new".to_owned()));
        assert_eq!(args.last().map(String::as_str), Some("about:blank"));
        assert!(!arguments(&BrowserConfig::new(dir.path().to_path_buf()))
            .contains(&"--headless=new".to_owned()));
    }

    #[test]
    fn the_profile_never_saves_passwords() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = dir.path().join("Default").join("Preferences");
        std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
        std::fs::write(
            &prefs,
            r#"{"profile":{"name":"x","password_manager_enabled":true}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("DevToolsActivePort"), "1\n/x").unwrap();
        prepare_profile(dir.path()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&prefs).unwrap()).unwrap();
        assert_eq!(v["credentials_enable_service"], false);
        assert_eq!(v["profile"]["password_manager_enabled"], false);
        assert_eq!(v["profile"]["name"], "x", "other settings stay");
        assert_eq!(v["autofill"]["credit_card_enabled"], false);
        assert!(!dir.path().join("DevToolsActivePort").exists());
    }

    #[cfg(unix)]
    struct Silent;

    #[cfg(unix)]
    impl plenipo_runtime::EventSink for Silent {
        fn emit(&self, _: plenipo_runtime::RuntimeEvent) {}
    }

    /// A stand-in for Edge: a shell script that notes each start in the `starts` file, then runs
    /// `then` with `$STARTS` and `$PROFILE` (the browser's profile folder) set.
    #[cfg(unix)]
    fn fake_browser(
        then: &str,
        launch_timeout: Duration,
    ) -> (tempfile::TempDir, Browser, Supervisor) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let script = dir.path().join("fake-browser");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nSTARTS='{}'\nPROFILE='{}'\necho start >> \"$STARTS\"\n{then}\n",
                dir.path().join("starts").display(),
                profile.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let supervisor = Supervisor::new(
            plenipo_runtime::SupervisorConfig {
                kill_grace: Duration::from_millis(500),
                drain_timeout: Duration::from_millis(200),
                ..Default::default()
            },
            plenipo_runtime::ExecutablePolicy::default(),
            plenipo_runtime::ProfileRegistry::default(),
            std::sync::Arc::new(plenipo_runtime::MetadataStore::in_memory()),
            std::sync::Arc::new(Silent),
            vec![],
        );
        let config = BrowserConfig {
            executable: Some(script),
            launch_timeout,
            ..BrowserConfig::new(profile)
        };
        (dir, Browser::new(config, supervisor.clone()), supervisor)
    }

    #[cfg(unix)]
    fn starts(dir: &tempfile::TempDir) -> usize {
        std::fs::read_to_string(dir.path().join("starts")).map_or(0, |s| s.lines().count())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_browser_slow_to_start_is_stopped_and_started_once_more() {
        // It never gets ready: no DevTools address, ever.
        let (dir, browser, supervisor) = fake_browser("exec sleep 60", Duration::from_secs(1));
        let error = browser
            .connection()
            .await
            .err()
            .expect("it never gets ready");
        assert!(
            error.ends_with("did not get ready within 1 seconds, and again on a second try"),
            "{error}"
        );
        assert_eq!(starts(&dir), 2, "started once more, and only once");
        assert!(
            supervisor
                .overview()
                .executions
                .iter()
                .all(|e| e.state.is_terminal()),
            "neither start is left running"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_second_start_is_connected_to_once_it_gets_ready() {
        // Slow the first time. The second time it gets ready the way a real browser does, but
        // at an address where nothing answers: connecting is what fails, which shows the second
        // start was waited for and used.
        let (dir, browser, _supervisor) = fake_browser(
            "if [ -e \"$STARTS.once\" ]; then\n  \
             printf '1\\n/devtools/browser/x\\n' > \"$PROFILE/DevToolsActivePort\"\n\
             else touch \"$STARTS.once\"; fi\nexec sleep 60",
            Duration::from_secs(1),
        );
        let error = browser.connection().await.err().expect("nothing answers");
        assert!(
            error.starts_with("could not connect to Plenipo's browser"),
            "{error}"
        );
        assert_eq!(starts(&dir), 2);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_browser_that_closes_at_once_is_not_started_again() {
        // Not slowness: a second start would close the same way.
        let (dir, browser, _supervisor) = fake_browser("exit 0", Duration::from_secs(10));
        let error = browser.connection().await.err().expect("it closes");
        assert!(error.contains("closed right after it started"), "{error}");
        assert_eq!(starts(&dir), 1);
    }
}
