//! Plenipo's browser (Phase 10, ADR-020): a copy of Microsoft Edge or Google Chrome that Plenipo
//! starts and controls, with its own profile in Plenipo's data folder, separate from the
//! owner's own browser, sign-ins, and passwords.
//!
//! - It runs through the supervisor like every program Plenipo starts (its own process tree,
//!   stopped when Plenipo quits), and is always visible: never hidden, with the browser's own
//!   "controlled by automated test software" bar and Plenipo's sign on every page a worker uses.
//! - Plenipo talks to it over the DevTools protocol on two private pipes the browser inherits
//!   from Plenipo ([`cdp`]) — never a network port, so no other program on the computer can
//!   connect to the browser and drive it; each worker's step gets its own tab ([`tab`]).
//! - The profile never saves passwords or card details.
//! - It never saves files (ADR-037): every download is refused before a tab exists, and the
//!   worker whose page tried it is told with its next result.
//! - A page never gets a second tab (ADR-036): the browser attaches to every new tab paused,
//!   before any of it runs, and a tab a worker's page opened is closed, its address opened in
//!   the worker's own tab when the worker's action opened it; the worker is told.
//! - If it crashes or is closed, the next browser tool call starts it again.
//! - The owner chooses which browser (ADR-028): Automatic (Edge, then Chrome), Edge, or Chrome.
//!   Each keeps its own profile folder; a new choice is used from the browser's next start.

pub mod cdp;
pub mod classify;
pub mod tab;

use std::path::{Path, PathBuf};
use std::time::Duration;

use plenipo_guard::BrowserChoice;
use plenipo_runtime::{ExtraPipes, LaunchSpec, Supervisor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;
use ts_rs::TS;

use self::cdp::Cdp;
use self::tab::{Signals, SitePolicy, Tab, TabLimits, Tabs};

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
    /// The browser to use (`PLENIPO_BROWSER`, or tests); otherwise the owner's choice is found.
    pub executable: Option<PathBuf>,
    /// Plenipo's own profile folder (Microsoft Edge's, and a browser named by `executable`);
    /// Google Chrome's is next to it, with `-chrome` after its name.
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
            launch_timeout: start_timeout(
                std::env::var("PLENIPO_BROWSER_START_SECONDS")
                    .ok()
                    .as_deref(),
            ),
            limits: TabLimits::default(),
        }
    }
}

/// How long the browser may take to start: 30 seconds, or `PLENIPO_BROWSER_START_SECONDS`
/// (5 to 300) on a slow computer, such as a test runner.
fn start_timeout(setting: Option<&str>) -> Duration {
    let seconds = setting
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|s| (5..=300).contains(s))
        .unwrap_or(30);
    Duration::from_secs(seconds)
}

/// Plenipo's browser as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowserStatus {
    /// The browser Plenipo uses ("Microsoft Edge"), when one is installed: the open one, or
    /// the one it will start.
    #[ts(optional)]
    pub name: Option<String>,
    #[ts(optional)]
    pub path: Option<String>,
    pub running: bool,
    /// Its own profile folder (sign-ins made there stay there).
    pub profile: String,
    #[ts(optional)]
    pub problem: Option<String>,
    /// The owner's choice in Settings (ADR-028).
    pub choice: BrowserChoice,
    /// Microsoft Edge and Google Chrome, and whether each is on this computer.
    pub options: Vec<BrowserOption>,
    /// `PLENIPO_BROWSER` names the browser, so the owner's choice does not apply.
    pub fixed: bool,
    /// The browser Plenipo starts next time, when the one open now is another.
    #[ts(optional)]
    pub next: Option<String>,
}

/// A browser the owner can choose (ADR-028).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowserOption {
    pub choice: BrowserChoice,
    /// "Microsoft Edge", "Google Chrome", or "Chromium" (Chrome's open-source build, on Linux).
    pub name: String,
    pub installed: bool,
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

/// Edge and Chrome on this computer, Edge first: each program found, and which choice it is.
fn installed() -> Vec<(BrowserChoice, PathBuf)> {
    let mut candidates: Vec<(BrowserChoice, PathBuf)> = Vec::new();
    if cfg!(windows) {
        let bases: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
            .into_iter()
            .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
            .collect();
        for (choice, program) in [
            (BrowserChoice::Edge, "Microsoft/Edge/Application/msedge.exe"),
            (
                BrowserChoice::Chrome,
                "Google/Chrome/Application/chrome.exe",
            ),
        ] {
            candidates.extend(bases.iter().map(|base| (choice, base.join(program))));
        }
    } else {
        for (choice, name) in [
            (BrowserChoice::Edge, "microsoft-edge"),
            (BrowserChoice::Edge, "microsoft-edge-stable"),
            (BrowserChoice::Chrome, "google-chrome"),
            (BrowserChoice::Chrome, "google-chrome-stable"),
            (BrowserChoice::Chrome, "chromium"),
            (BrowserChoice::Chrome, "chromium-browser"),
        ] {
            if let Some(p) = crate::programs::find_on_path(name) {
                candidates.push((choice, p));
            }
        }
        candidates.push((
            BrowserChoice::Edge,
            PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        ));
        candidates.push((
            BrowserChoice::Chrome,
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        ));
    }
    candidates.retain(|(_, p)| p.is_file());
    candidates
}

/// Edge or Chrome on this computer: the configured one, then the usual places, Edge first.
pub fn find_browser(configured: Option<&Path>) -> Option<(PathBuf, String)> {
    find_chosen(configured, BrowserChoice::Automatic).map(|(p, name, _)| (p, name))
}

/// The browser for the owner's choice (a configured one wins), its name, and which it is
/// (`Automatic` for a configured browser, which keeps the usual profile folder).
fn find_chosen(
    configured: Option<&Path>,
    choice: BrowserChoice,
) -> Option<(PathBuf, String, BrowserChoice)> {
    if let Some(p) = configured {
        return p
            .is_file()
            .then(|| (p.to_path_buf(), name_of(p), BrowserChoice::Automatic));
    }
    installed()
        .into_iter()
        .find(|(kind, _)| choice == BrowserChoice::Automatic || *kind == choice)
        .map(|(kind, p)| {
            let name = name_of(&p);
            (p, name, kind)
        })
}

/// A browser's own profile folder: Chrome's is next to Plenipo's usual one (Edge's), because
/// two browsers must never share one profile.
fn profile_for(usual: &Path, kind: BrowserChoice) -> PathBuf {
    match kind {
        BrowserChoice::Chrome => {
            let mut name = usual.file_name().unwrap_or_default().to_os_string();
            name.push("-chrome");
            usual.with_file_name(name)
        }
        BrowserChoice::Automatic | BrowserChoice::Edge => usual.to_path_buf(),
    }
}

/// What Settings says when the chosen browser is not on this computer.
fn not_found(choice: BrowserChoice) -> &'static str {
    match choice {
        BrowserChoice::Automatic => {
            "No Microsoft Edge or Google Chrome was found on this computer, so workers cannot use \
             websites. Windows 11 includes Edge; if it was removed, install Edge or Chrome."
        }
        BrowserChoice::Edge => {
            "Microsoft Edge was not found on this computer, so workers cannot use websites. \
             Choose Automatic or Google Chrome, or install Edge."
        }
        BrowserChoice::Chrome => {
            "Google Chrome was not found on this computer, so workers cannot use websites. \
             Choose Automatic or Microsoft Edge, or install Chrome."
        }
    }
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
    // Plenipo's browser has no DevTools port; a file from an older version saying otherwise goes.
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

/// The command-line options Plenipo starts the browser with, on its own `profile` folder.
fn arguments(config: &BrowserConfig, profile: &Path) -> Vec<String> {
    let mut args: Vec<String> = [
        format!("--user-data-dir={}", profile.display()),
        // The DevTools protocol over the two pipes the browser inherits (descriptors 3 and 4),
        // never `--remote-debugging-port`: a port has no lock on its door, and any program
        // running as the owner could open it.
        "--remote-debugging-pipe".into(),
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
    /// Which browser it is, from which program, on which profile folder.
    name: String,
    path: PathBuf,
    profile: PathBuf,
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
    /// The owner's choice (ADR-028), kept in step with Guard's settings by the broker.
    choice: std::sync::Mutex<BrowserChoice>,
    /// The workers' tabs open now, for the browser's own events (a download it refused,
    /// ADR-037).
    tabs: Tabs,
}

impl Browser {
    pub fn new(config: BrowserConfig, supervisor: Supervisor) -> Self {
        Self {
            inner: std::sync::Arc::new(Inner {
                config,
                supervisor,
                running: AsyncMutex::new(None),
                problem: std::sync::Mutex::new(None),
                choice: std::sync::Mutex::new(BrowserChoice::Automatic),
                tabs: Tabs::default(),
            }),
        }
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.inner.config
    }

    /// The owner's choice (ADR-028). A browser that is open stays open; the choice is used
    /// from its next start.
    pub fn set_choice(&self, choice: BrowserChoice) {
        *self.inner.choice.lock().unwrap_or_else(|p| p.into_inner()) = choice;
    }

    pub fn choice(&self) -> BrowserChoice {
        *self.inner.choice.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The browser Plenipo starts next: its program and name.
    pub fn find(&self) -> Option<(PathBuf, String)> {
        self.find_chosen().map(|(p, name, _)| (p, name))
    }

    fn find_chosen(&self) -> Option<(PathBuf, String, BrowserChoice)> {
        find_chosen(self.inner.config.executable.as_deref(), self.choice())
    }

    /// What Settings shows.
    pub async fn status(&self) -> BrowserStatus {
        let choice = self.choice();
        let fixed = self.inner.config.executable.is_some();
        let next = self.find_chosen();
        let open = self
            .inner
            .running
            .lock()
            .await
            .as_ref()
            .filter(|r| !r.cdp.is_closed())
            .map(|r| (r.name.clone(), r.path.clone(), r.profile.clone()));
        let found = installed();
        let options = [BrowserChoice::Edge, BrowserChoice::Chrome]
            .into_iter()
            .map(|c| {
                let program = found.iter().find(|(kind, _)| *kind == c);
                BrowserOption {
                    choice: c,
                    name: match (c, program) {
                        (_, Some((_, p))) => name_of(p),
                        (BrowserChoice::Edge, None) => "Microsoft Edge".into(),
                        (_, None) => "Google Chrome".into(),
                    },
                    installed: program.is_some(),
                }
            })
            .collect();
        let usual = &self.inner.config.profile_dir;
        let upcoming = next
            .as_ref()
            .map(|(p, name, kind)| (name.clone(), p.clone(), profile_for(usual, *kind)));
        let shown = open.clone().or_else(|| upcoming.clone());
        BrowserStatus {
            name: shown.as_ref().map(|(n, _, _)| n.clone()),
            path: shown.as_ref().map(|(_, p, _)| p.display().to_string()),
            running: open.is_some(),
            profile: shown
                .as_ref()
                .map_or_else(|| usual.clone(), |(_, _, profile)| profile.clone())
                .display()
                .to_string(),
            problem: if next.is_none() {
                Some(not_found(choice).into())
            } else {
                self.inner
                    .problem
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone()
            },
            choice,
            options,
            fixed,
            next: match (&open, &upcoming) {
                (Some((_, open_path, _)), Some((name, path, _))) if open_path != path => {
                    Some(name.clone())
                }
                _ => None,
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
        let (path, name, kind) = self
            .find_chosen()
            .ok_or_else(|| not_found(self.choice()).to_owned())?;
        let profile = profile_for(&config.profile_dir, kind);
        prepare_profile(&profile)
            .map_err(|e| format!("could not prepare the browser's profile: {e}"))?;
        let sup = &self.inner.supervisor;
        let executable = sup
            .allow_executable(&path)
            .map_err(|e| format!("{} cannot be started: {e}", path.display()))?;
        let env: Vec<(String, String)> = BROWSER_ENV
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_owned(), v)))
            .collect();
        // The browser inherits its end of two pipes (its descriptors 3 and 4) and speaks the
        // DevTools protocol over them; Plenipo keeps the other ends. No network port exists.
        let (pipes, ends) = ExtraPipes::create()
            .map_err(|e| format!("could not prepare the pipes to Plenipo's browser: {e}"))?;
        let record = sup
            .launch(LaunchSpec {
                profile_id: "capability.browser".into(),
                label: format!("Plenipo's browser ({name})"),
                executable,
                args: arguments(config, &profile),
                env,
                working_dir: profile.clone(),
                max_runtime: plenipo_runtime::profile::MAX_RUNTIME_LIMIT,
                stdin: None,
                stdin_feed: None,
                max_line_bytes: None,
                observer: None,
                agent: None,
                extra_pipes: Some(pipes),
            })
            .await
            .map_err(|e| format!("{name} could not be started: {e}"))?;
        let (cdp, mut browser_events) = match Cdp::over_pipe(ends) {
            Ok(c) => c,
            Err(e) => {
                let _ = sup.cancel(&record.id).await;
                return Err(LaunchError::Failed(e));
            }
        };
        // Its first answer shows the browser is up. A browser that ends instead closes its end
        // of the pipe, which closes the connection at once. (Stopping the browser closes the
        // connection too, so look before stopping it.)
        if cdp
            .call(None, "Browser.getVersion", json!({}), config.launch_timeout)
            .await
            .is_err()
        {
            let closed = cdp.is_closed();
            let _ = sup.cancel(&record.id).await;
            return Err(if closed {
                LaunchError::Failed(format!(
                    "{name} closed right after it started (is Plenipo's browser already open \
                     from an earlier start? Close it and try again)"
                ))
            } else {
                LaunchError::NotReady(format!(
                    "{name} did not get ready within {} seconds",
                    config.launch_timeout.as_secs()
                ))
            });
        }
        // Plenipo's browser never saves files (ADR-037). Before any tab exists, the browser is
        // told to refuse every download, and to report each one it refused so the worker whose
        // page tried it can be told. A browser that cannot be told so is not used: there is no
        // falling back to saving files.
        if let Err(e) = cdp
            .call(
                None,
                "Browser.setDownloadBehavior",
                json!({ "behavior": "deny", "eventsEnabled": true }),
                config.launch_timeout,
            )
            .await
        {
            let _ = sup.cancel(&record.id).await;
            return Err(LaunchError::Failed(format!(
                "{name} could not be set to never save files ({e}), so Plenipo did not use it"
            )));
        }
        // A page never gets a second tab (ADR-036): the browser is told to attach to every new
        // tab paused, before any of it runs, so one a page opens can be closed and the worker
        // told (`Tabs::target_attached`). Told to the browser itself, which is where new windows
        // arrive (a tab's own session only hears of its frames and workers). Plenipo's own new
        // tabs arrive the same way and are let run. A browser that cannot be told so is not used.
        if let Err(e) = cdp
            .call(
                None,
                "Target.setAutoAttach",
                json!({
                    "autoAttach": true,
                    "waitForDebuggerOnStart": true,
                    "flatten": true,
                    "filter": [{ "type": "page" }],
                }),
                config.launch_timeout,
            )
            .await
        {
            let _ = sup.cancel(&record.id).await;
            return Err(LaunchError::Failed(format!(
                "{name} could not be set to keep pages from opening new tabs ({e}), so Plenipo \
                 did not use it"
            )));
        }
        // Of the browser's own events, two matter: a download it refused, which the tab whose
        // page tried it notes for its worker; and a new tab it attached to, paused, which the
        // tabs decide about (on its own, so a slow one holds up no other event). The rest are
        // dropped.
        let tabs = self.inner.tabs.clone();
        let events_cdp = cdp.clone();
        tokio::spawn(async move {
            while let Some(event) = browser_events.recv().await {
                match event.method.as_str() {
                    "Browser.downloadWillBegin" => {
                        tabs.download_refused(&event.params);
                    }
                    "Target.attachedToTarget" => {
                        let (tabs, cdp) = (tabs.clone(), events_cdp.clone());
                        tokio::spawn(async move {
                            tabs.target_attached(&cdp, &event.params).await;
                        });
                    }
                    _ => {}
                }
            }
        });
        Ok(Running {
            cdp,
            execution_id: record.id,
            name,
            path,
            profile,
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
        self.inner.tabs.watch(&tab);
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
        let args = arguments(&config, dir.path());
        assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
        assert!(args.contains(&"--enable-automation".to_owned()));
        assert!(args.contains(&"--headless=new".to_owned()));
        assert_eq!(args.last().map(String::as_str), Some("about:blank"));
        assert!(
            !arguments(&BrowserConfig::new(dir.path().to_path_buf()), dir.path())
                .contains(&"--headless=new".to_owned())
        );
    }

    /// The app's browser is driven over the pipes it inherits, never a DevTools port: a port
    /// is open to every program running as the owner.
    #[test]
    fn the_browser_is_controlled_over_a_pipe_never_a_port() {
        let dir = tempfile::tempdir().unwrap();
        let args = arguments(&BrowserConfig::new(dir.path().to_path_buf()), dir.path());
        assert!(
            args.contains(&"--remote-debugging-pipe".to_owned()),
            "{args:?}"
        );
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with("--remote-debugging-port")),
            "{args:?}"
        );
    }

    #[test]
    fn each_browser_has_its_own_profile() {
        let usual = Path::new("/data/browser-profile");
        assert_eq!(profile_for(usual, BrowserChoice::Edge), usual);
        assert_eq!(profile_for(usual, BrowserChoice::Automatic), usual);
        assert_eq!(
            profile_for(usual, BrowserChoice::Chrome),
            Path::new("/data/browser-profile-chrome")
        );
        let chrome = profile_for(usual, BrowserChoice::Chrome);
        let args = arguments(&BrowserConfig::new(usual.to_path_buf()), &chrome);
        // As the system writes the path (`\` on Windows).
        assert!(args.contains(&format!("--user-data-dir={}", chrome.display())));
    }

    #[test]
    fn a_configured_browser_wins_over_the_choice() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("my-browser");
        std::fs::write(&program, "").unwrap();
        for choice in [BrowserChoice::Edge, BrowserChoice::Chrome] {
            let (path, name, kind) = find_chosen(Some(&program), choice).unwrap();
            assert_eq!((path, name.as_str()), (program.clone(), "my-browser"));
            assert_eq!(kind, BrowserChoice::Automatic, "the usual profile folder");
        }
        assert!(find_chosen(Some(&dir.path().join("missing")), BrowserChoice::Chrome).is_none());
    }

    #[test]
    fn the_start_time_can_grow_on_slow_computers() {
        assert_eq!(start_timeout(None), Duration::from_secs(30));
        assert_eq!(start_timeout(Some(" 90 ")), Duration::from_secs(90));
        for odd in ["0", "4", "301", "ninety", ""] {
            assert_eq!(start_timeout(Some(odd)), Duration::from_secs(30), "{odd}");
        }
    }

    #[test]
    fn missing_browsers_are_named_in_plain_words() {
        assert!(not_found(BrowserChoice::Edge).starts_with("Microsoft Edge was not found"));
        assert!(not_found(BrowserChoice::Chrome).contains("Choose Automatic or Microsoft Edge"));
        assert!(not_found(BrowserChoice::Automatic).contains("Windows 11 includes Edge"));
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
    /// `then` with `$STARTS` and `$PROFILE` (the browser's profile folder) set. Like the real
    /// browser, it gets the DevTools pipes as its descriptors 3 (commands) and 4 (answers), and
    /// `then` may call `answer_commands` to answer each command the way the real browser does
    /// (see the script), keeping a copy of every command in the `starts.commands` file.
    #[cfg(unix)]
    fn fake_browser(
        then: &str,
        launch_timeout: Duration,
    ) -> (tempfile::TempDir, Browser, Supervisor) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let script = dir.path().join("fake-browser");
        // `answer_commands [ID]`: read each command from descriptor 3 (a JSON text ended by a
        // NUL byte; a NUL cannot live in a shell variable, so an empty byte read ends one),
        // note it, and answer it on descriptor 4 with a result, or with an error for command ID.
        const ANSWER: &str = "answer_commands() {\n  \
            while :; do\n    \
              cmd=''\n    \
              while b=$(dd bs=1 count=1 2>/dev/null <&3) && [ -n \"$b\" ]; do cmd=\"$cmd$b\"; done\n    \
              [ -n \"$cmd\" ] || return\n    \
              printf '%s\\n' \"$cmd\" >> \"$STARTS.commands\"\n    \
              id=${cmd#*\\\"id\\\":}\n    \
              id=${id%%,*}\n    \
              if [ \"$id\" = \"${1:-}\" ]; then\n      \
                printf '{\"id\":%s,\"error\":{\"code\":-32601,\"message\":\"not supported\"}}\\0' \"$id\" >&4\n    \
              else\n      \
                printf '{\"id\":%s,\"result\":{\"product\":\"Fake/1\"}}\\0' \"$id\" >&4\n    \
              fi\n  \
            done\n\
            }\n";
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nSTARTS='{}'\nPROFILE='{}'\necho start >> \"$STARTS\"\n{ANSWER}{then}\n",
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

    /// The commands the fake browser was sent, in order.
    #[cfg(unix)]
    fn commands(dir: &tempfile::TempDir) -> Vec<Value> {
        std::fs::read_to_string(dir.path().join("starts.commands"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_browser_slow_to_start_is_stopped_and_started_once_more() {
        // It never gets ready: it never answers on the pipe.
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
        // Slow the first time. The second time it gets ready the way the real browser does: it
        // answers each command Plenipo sends on its descriptor 3 on its descriptor 4, as one
        // JSON text ended by a NUL byte. It never wrote a `DevToolsActivePort` file: it has no
        // port.
        let (dir, browser, supervisor) = fake_browser(
            "if [ -e \"$STARTS.once\" ]; then answer_commands; else touch \"$STARTS.once\"; fi\n\
             exec sleep 60",
            Duration::from_secs(1),
        );
        let (cdp, start) = browser
            .connection()
            .await
            .expect("the second start answers");
        assert_eq!(start, Start::Started);
        assert!(!cdp.is_closed());
        assert_eq!(starts(&dir), 2);
        assert!(!dir
            .path()
            .join("profile")
            .join("DevToolsActivePort")
            .exists());
        let run = browser.execution_id().await.expect("its run");
        supervisor.cancel(&run).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), cdp.closed())
            .await
            .expect("the connection ends with the browser");
    }

    /// ADR-037 and ADR-036: the browser is told never to save files as the very first thing
    /// after it shows it is up, before any tab exists, with the setting that refuses every
    /// download and reports each one refused; and then to attach to every new tab paused, before
    /// any of it runs, so a tab a page opens can be closed.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_browser_is_told_never_to_save_files_before_any_tab_opens() {
        let (dir, browser, supervisor) =
            fake_browser("answer_commands\nexec sleep 60", Duration::from_secs(5));
        let (cdp, start) = browser.connection().await.expect("the browser answers");
        assert_eq!(start, Start::Started);
        let sent = commands(&dir);
        let methods: Vec<&str> = sent.iter().filter_map(|c| c["method"].as_str()).collect();
        assert_eq!(
            methods,
            [
                "Browser.getVersion",
                "Browser.setDownloadBehavior",
                "Target.setAutoAttach"
            ],
            "{sent:?}"
        );
        assert_eq!(sent[1]["params"]["behavior"], "deny", "{sent:?}");
        assert_eq!(sent[1]["params"]["eventsEnabled"], true, "{sent:?}");
        assert!(
            sent[1]["params"].get("downloadPath").is_none(),
            "nowhere to save to: {sent:?}"
        );
        assert!(
            sent[1].get("sessionId").is_none(),
            "told to the browser itself, for every tab: {sent:?}"
        );
        // New tabs arrive paused, on the browser's own session (where new windows come), flat.
        assert_eq!(sent[2]["params"]["autoAttach"], true, "{sent:?}");
        assert_eq!(
            sent[2]["params"]["waitForDebuggerOnStart"], true,
            "{sent:?}"
        );
        assert_eq!(sent[2]["params"]["flatten"], true, "{sent:?}");
        assert_eq!(
            sent[2]["params"]["filter"],
            json!([{ "type": "page" }]),
            "{sent:?}"
        );
        assert!(sent[2].get("sessionId").is_none(), "{sent:?}");
        let run = browser.execution_id().await.expect("its run");
        supervisor.cancel(&run).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), cdp.closed())
            .await
            .expect("the connection ends with the browser");
    }

    /// ADR-037: a browser that does not take the setting is not used, the owner is told in
    /// plain words, and it is not started again (a second start would refuse the same way).
    #[cfg(unix)]
    #[tokio::test]
    async fn a_browser_that_may_save_files_is_not_used() {
        // It refuses Plenipo's second command, the one that keeps it from saving files.
        let (dir, browser, supervisor) =
            fake_browser("answer_commands 2\nexec sleep 60", Duration::from_secs(5));
        let error = browser.connection().await.err().expect("it is not used");
        assert!(
            error.contains("could not be set to never save files"),
            "{error}"
        );
        assert!(error.contains("not supported"), "{error}");
        assert!(error.ends_with("so Plenipo did not use it"), "{error}");
        assert_eq!(starts(&dir), 1);
        assert!(
            supervisor
                .overview()
                .executions
                .iter()
                .all(|e| e.state.is_terminal()),
            "the browser is stopped"
        );
        assert!(browser.execution_id().await.is_none());
        assert_eq!(browser.status().await.problem, Some(error));
    }

    /// ADR-036: a browser that will not hand over new tabs paused is not used either, and the
    /// owner is told in plain words.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_browser_that_lets_pages_open_new_tabs_is_not_used() {
        // It refuses Plenipo's third command, the one about new tabs.
        let (dir, browser, supervisor) =
            fake_browser("answer_commands 3\nexec sleep 60", Duration::from_secs(5));
        let error = browser.connection().await.err().expect("it is not used");
        assert!(
            error.contains("could not be set to keep pages from opening new tabs"),
            "{error}"
        );
        assert!(error.contains("not supported"), "{error}");
        assert!(error.ends_with("so Plenipo did not use it"), "{error}");
        assert_eq!(starts(&dir), 1);
        assert!(
            supervisor
                .overview()
                .executions
                .iter()
                .all(|e| e.state.is_terminal()),
            "the browser is stopped"
        );
        assert!(browser.execution_id().await.is_none());
        assert_eq!(browser.status().await.problem, Some(error));
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
