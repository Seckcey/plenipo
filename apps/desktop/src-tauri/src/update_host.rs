//! Updates in the desktop app (Phase 13, ADR-038): checking once a day (always on, for Free and
//! Pro alike), telling the owner, and installing only when the owner says so.
//!
//! Installing goes in order and stops at the first problem, leaving this version installed:
//! download and check the installer ([`plenipo_capabilities::updates`]), back up the Ledger,
//! stop the work the normal way, start the installer ("progress bar only", as an update, and
//! reopen Plenipo after), and quit. On Linux an AppImage puts the new version in place of itself
//! and opens it; a copy the system's installer put there (a `.deb`) is updated by hand (Phase 23,
//! ADR-152).
//!
//! Where updates come from, and the updater key's public half, are built into each copy by the
//! Release workflow (`PLENIPO_UPDATE_ENDPOINT`, `PLENIPO_UPDATER_PUBLIC_KEY` at build time).
//! They are never read from a setting or from the environment at run time.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plenipo_capabilities::updates::{
    self, Release, UpdateSource, RELEASES_ENDPOINT, RELEASES_PAGE,
};
use plenipo_core::{AvailableUpdate, InstallWay, UpdateState, UpdateStatus};
use plenipo_guard::Guard;
use plenipo_ledger::{BackupKind, Ledger, NewEvent};
use serde_json::json;

/// The first automatic check waits this long after Plenipo starts.
pub const FIRST_CHECK: Duration = Duration::from_secs(3 * 60);
/// Then Plenipo checks this often.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// The folder (in Plenipo's own folder) an installer waits in while it runs.
pub const FOLDER: &str = "updates";
/// Ledger events.
pub const AVAILABLE: &str = "plenipo.update_available";
pub const INSTALLING: &str = "plenipo.update_installing";
pub const FAILED: &str = "plenipo.update_failed";

/// Where this copy of Plenipo gets updates from, as built.
pub fn built_source() -> UpdateSource {
    UpdateSource {
        endpoint: option_env!("PLENIPO_UPDATE_ENDPOINT")
            .filter(|e| !e.trim().is_empty())
            .unwrap_or(RELEASES_ENDPOINT)
            .to_owned(),
        public_key: option_env!("PLENIPO_UPDATER_PUBLIC_KEY")
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_owned),
    }
}

/// The NSIS installer's arguments for an update: a progress bar only (`/P`), as an update
/// (`/UPDATE`: no "uninstall first" question, shortcuts kept), and reopen Plenipo after (`/R`).
pub const INSTALLER_ARGS: [&str; 3] = ["/P", "/UPDATE", "/R"];

/// The switch an AppImage's new version starts with: wait until the version it replaced has
/// closed, so that one Plenipo at a time holds (Phase 23).
pub const WAIT_FOR: &str = "--plenipo-wait-for=";
/// The longest the new version waits for the old one.
const WAIT_AT_MOST: Duration = Duration::from_secs(60);

/// How this copy takes a new version: Windows' installer; on Linux, an AppImage replaces itself
/// (the AppImage's starter says where it is, `APPIMAGE`); otherwise by hand.
pub fn install_way() -> InstallWay {
    install_way_from(cfg!(windows), own_appimage().is_some())
}

fn install_way_from(windows: bool, appimage: bool) -> InstallWay {
    if windows {
        InstallWay::Installer
    } else if appimage {
        InstallWay::ReplacesItself
    } else {
        InstallWay::ByHand
    }
}

/// The AppImage this copy runs from, on Linux: a file, named by its full path.
pub fn own_appimage() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
}

struct Inner {
    state: UpdateState,
    last_checked_at: Option<u64>,
    available: Option<Release>,
    message: Option<String>,
}

/// Updates for this run of Plenipo.
pub struct Updates {
    source: UpdateSource,
    version: String,
    how: InstallWay,
    inner: Mutex<Inner>,
}

impl Updates {
    pub fn new(version: &str, source: UpdateSource) -> Arc<Self> {
        Self::new_with(version, source, install_way())
    }

    /// As [`Updates::new`], taking a new version the given way.
    pub fn new_with(version: &str, source: UpdateSource, how: InstallWay) -> Arc<Self> {
        Arc::new(Self {
            source,
            version: version.to_owned(),
            how,
            inner: Mutex::new(Inner {
                state: UpdateState::NotChecked,
                last_checked_at: None,
                available: None,
                message: None,
            }),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn source(&self) -> &UpdateSource {
        &self.source
    }

    /// How this copy takes a new version.
    pub fn how(&self) -> InstallWay {
        self.how
    }

    /// This copy can install updates (it has the updater key's public half).
    pub fn can_install(&self) -> bool {
        self.source.public_key.is_some()
    }

    /// Checks happen by themselves (in copies that can install updates: released ones, and the
    /// Windows installer tests' copy). Copies built elsewhere check only when asked.
    pub fn checks_by_itself(&self) -> bool {
        self.can_install()
    }

    pub fn status(&self) -> UpdateStatus {
        self.status_of(&self.lock())
    }

    fn status_of(&self, inner: &Inner) -> UpdateStatus {
        let message = inner.message.clone().or_else(|| {
            (!self.can_install()).then(|| {
                "This copy of Plenipo was not built by 8 West's Release workflow, so it cannot \
                 install updates. Get new versions from GitHub Releases."
                    .to_owned()
            })
        });
        UpdateStatus {
            version: self.version.clone(),
            state: inner.state,
            can_install: self.can_install(),
            last_checked_at: inner.last_checked_at,
            available: inner.available.as_ref().map(|r| AvailableUpdate {
                version: r.version.clone(),
                notes: r.notes.clone(),
                published: r.pub_date.clone(),
            }),
            message,
            releases_page: RELEASES_PAGE.to_owned(),
            how: self.how,
        }
    }

    /// The newer release found, if any.
    pub fn available(&self) -> Option<Release> {
        self.lock().available.clone()
    }

    fn set(&self, state: UpdateState, message: Option<String>) {
        let mut inner = self.lock();
        inner.state = state;
        inner.message = message;
    }

    /// Ask GitHub (through Guard) whether a newer version exists. A newer one is recorded in
    /// the Ledger once per version, which shows the owner a notice.
    pub async fn check_now(&self, guard: &Guard, ledger: &Ledger) -> UpdateStatus {
        {
            let mut inner = self.lock();
            if inner.state == UpdateState::Installing {
                return self.status_of(&inner);
            }
            inner.state = UpdateState::Checking;
            inner.message = None;
        }
        let result = updates::check(guard, &self.source, &self.version).await;
        let now = plenipo_ledger::now_ms();
        {
            let mut inner = self.lock();
            inner.last_checked_at = Some(now);
            if inner.state == UpdateState::Installing {
                // Install now started while this check was on its way: it keeps its state (only
                // one install at a time), and what it installs was checked by its own download.
                return self.status_of(&inner);
            }
            match &result {
                Ok(Some(release)) => {
                    inner.state = UpdateState::Available;
                    inner.available = Some(release.clone());
                    inner.message = None;
                }
                Ok(None) => {
                    inner.state = UpdateState::UpToDate;
                    inner.available = None;
                    inner.message = None;
                }
                Err(e) => {
                    inner.state = UpdateState::Failed;
                    inner.message = Some(format!(
                        "Plenipo could not check for updates: {e}. It will try again later."
                    ));
                }
            }
        }
        match result {
            Ok(Some(release)) => {
                log::info!("Plenipo {} is available", release.version);
                announce(ledger, &release.version);
            }
            Ok(None) => log::info!("Plenipo is up to date"),
            Err(e) => log::warn!("the update check did not work: {e}"),
        }
        self.status()
    }

    /// Download the newer release and check it. Fails, leaving everything as it was, when
    /// there is none, this copy cannot install, or the check fails.
    pub async fn download(&self, guard: &Guard) -> Result<(Release, Vec<u8>), String> {
        let release = self
            .available()
            .ok_or("There is no newer version to install. Check for updates first.")?;
        if !self.can_install() {
            return Err(self.status().message.unwrap_or_default());
        }
        if self.how == InstallWay::ByHand {
            return Err(BY_HAND.into());
        }
        {
            // One install at a time: the check and the change are one step.
            let mut inner = self.lock();
            if inner.state == UpdateState::Installing {
                return Err("The update is already being installed.".into());
            }
            inner.state = UpdateState::Installing;
            inner.message = None;
        }
        match updates::download(guard, &self.source, &release).await {
            Ok(bytes) => Ok((release, bytes)),
            Err(e) => {
                let message = format!(
                    "The update was not installed: {e}. Plenipo {} is still installed.",
                    self.version
                );
                self.set(UpdateState::Failed, Some(message.clone()));
                Err(message)
            }
        }
    }

    /// The install did not go ahead: say why, and keep offering the update.
    pub fn install_failed(&self, ledger: &Ledger, message: String) {
        log::error!("{message}");
        self.set(UpdateState::Failed, Some(message.clone()));
        let _ = ledger.append_event(NewEvent {
            source: "plenipo".into(),
            event_type: FAILED.into(),
            payload: json!({ "message": message }),
            ..NewEvent::default()
        });
    }
}

/// Why a copy the system's installer put there never installs a new version itself.
pub const BY_HAND: &str =
    "This copy of Plenipo was installed by your computer's installer, so it is \
                           updated the same way: choose Download the new version, then install it.";

/// Record a newer version once, which also shows a notice.
fn announce(ledger: &Ledger, version: &str) {
    let already = ledger
        .events_of_types(&[AVAILABLE], 50)
        .unwrap_or_default()
        .iter()
        .any(|e| e.payload["version"].as_str() == Some(version));
    if already {
        return;
    }
    let _ = ledger.append_event(NewEvent {
        source: "plenipo".into(),
        event_type: AVAILABLE.into(),
        payload: json!({ "version": version }),
        ..NewEvent::default()
    });
}

/// Before stopping the work: back up the Ledger ("before updating to …"), write the checked
/// installer next to Plenipo's other files, and record what is about to happen. Returns the
/// installer's path.
pub fn prepare(
    ledger: &Ledger,
    dir: &Path,
    from: &str,
    release: &Release,
    installer: &[u8],
    appimage: Option<&Path>,
) -> Result<PathBuf, String> {
    let backup = if ledger.path().is_some() {
        let info = ledger
            .backup_of_kind(BackupKind::BeforeUpdate, Some(&release.version))
            .map_err(|e| {
                format!(
                    "The update was not installed: the Ledger could not be backed up first \
                     ({e}). Plenipo {from} is still installed."
                )
            })?;
        crate::backup_host::record(ledger, BackupKind::BeforeUpdate, &info, "plenipo");
        Path::new(&info.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    } else {
        None
    };
    let clean: String = release
        .version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .collect();
    let path = match appimage {
        // Next to the AppImage it replaces, on the same disk, so the swap is one step; hidden
        // until then.
        Some(appimage) => {
            let name = appimage
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            appimage.with_file_name(format!(".{name}.{clean}.new"))
        }
        None => {
            std::fs::create_dir_all(dir).map_err(|e| {
                format!("The update was not installed: its folder could not be made ({e}).")
            })?;
            dir.join(format!("Plenipo_{clean}_x64-setup.exe"))
        }
    };
    std::fs::write(&path, installer).map_err(|e| {
        let folder = path
            .parent()
            .map_or_else(String::new, |p| p.display().to_string());
        format!("The update was not installed: it could not be saved in {folder} ({e}).")
    })?;
    #[cfg(unix)]
    if appimage.is_some() {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(|e| {
            let _ = std::fs::remove_file(&path);
            format!("The update was not installed: it could not be made runnable ({e}).")
        })?;
    }
    let _ = ledger.append_event(NewEvent {
        source: "plenipo".into(),
        event_type: INSTALLING.into(),
        payload: json!({ "from": from, "to": release.version, "backup": backup }),
        ..NewEvent::default()
    });
    Ok(path)
}

/// Remove installers left from an earlier update (they have done their job).
pub fn clean_up(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Put the checked new AppImage in place of the running one (Linux). The running copy keeps
/// working from the old file until it quits.
pub fn replace_itself(new: &Path, appimage: &Path) -> std::io::Result<()> {
    std::fs::rename(new, appimage).inspect_err(|_| {
        let _ = std::fs::remove_file(new);
    })
}

/// Open the new AppImage on its own, told to wait until this one has closed (it outlives
/// Plenipo, which quits right after), with the owner's own session.
#[cfg(unix)]
pub fn start_again(appimage: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt as _;
    std::process::Command::new(appimage)
        .arg(format!("{WAIT_FOR}{}", std::process::id()))
        .env_clear()
        .envs(plenipo_runtime::policy::owner_session_env())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

/// Opening an AppImage again is for Linux alone.
#[cfg(not(unix))]
pub fn start_again(_appimage: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("not an AppImage"))
}

/// The new version of an AppImage waits here, before anything else starts, until the version it
/// replaced has closed (at most a minute), so that one Plenipo at a time holds.
pub fn wait_for_previous(args: impl IntoIterator<Item = String>) {
    let Some(pid) = args
        .into_iter()
        .find_map(|a| a.strip_prefix(WAIT_FOR).and_then(|p| p.parse::<u32>().ok()))
        .filter(|&p| p > 1 && p != std::process::id())
    else {
        return;
    };
    let started = std::time::Instant::now();
    while running(pid) && started.elapsed() < WAIT_AT_MOST {
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn running(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(pid).is_some()
}

/// Start the installer on its own (it outlives Plenipo, which quits right after).
pub fn start_installer(path: &Path) -> std::io::Result<()> {
    let mut command = std::process::Command::new(path);
    command.args(INSTALLER_ARGS);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // Not in Plenipo's program group, which ends with Plenipo; no console window.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB);
        if command.spawn().is_ok() {
            return Ok(());
        }
        command.creation_flags(DETACHED_PROCESS);
    }
    command.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            notes: "Fixes.".into(),
            pub_date: None,
            url: "https://github.com/Seckcey/plenipo/releases/download/x".into(),
            signature: "s".into(),
        }
    }

    #[test]
    fn a_check_that_ends_while_installing_leaves_the_install_alone() {
        let u = Updates::new(
            "1.9.0",
            UpdateSource {
                endpoint: RELEASES_ENDPOINT.into(),
                public_key: Some("key".into()),
            },
        );
        u.lock().available = Some(release("1.10.0"));
        u.set(UpdateState::Installing, None);
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let guard = Guard::new(Arc::clone(&ledger));
        let s = tauri::async_runtime::block_on(u.check_now(&guard, &ledger));
        assert_eq!(
            s.state,
            UpdateState::Installing,
            "a check never starts mid-install"
        );
        let again = tauri::async_runtime::block_on(u.download(&guard));
        assert!(again.unwrap_err().contains("already being installed"));
        assert_eq!(u.status().state, UpdateState::Installing);
    }

    #[test]
    fn a_copy_without_the_updater_key_says_it_cannot_install() {
        let u = Updates::new(
            "1.9.0",
            UpdateSource {
                endpoint: RELEASES_ENDPOINT.into(),
                public_key: None,
            },
        );
        let s = u.status();
        assert!(!s.can_install);
        assert!(!u.checks_by_itself());
        assert_eq!(s.state, UpdateState::NotChecked);
        assert!(s.message.unwrap().contains("cannot install updates"));
        assert_eq!(s.releases_page, RELEASES_PAGE);
        let keyed = Updates::new(
            "1.9.0",
            UpdateSource {
                endpoint: RELEASES_ENDPOINT.into(),
                public_key: Some("key".into()),
            },
        );
        assert!(keyed.checks_by_itself());
        assert!(keyed.status().message.is_none());
    }

    #[test]
    fn a_released_copy_looks_only_at_plenipos_releases() {
        // Tests are built without the Release workflow's settings (a build for the Windows
        // installer tests has its own address, checked by those tests).
        if option_env!("PLENIPO_UPDATE_ENDPOINT").is_some() {
            return;
        }
        let source = built_source();
        assert_eq!(source.endpoint, RELEASES_ENDPOINT);
        assert_eq!(source.rules(), plenipo_guard::OutboundRules::default());
    }

    #[test]
    fn preparing_backs_up_the_ledger_and_saves_the_checked_installer() {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::open(&dir.path().join("ledger").join("plenipo.db")).unwrap();
        let updates = dir.path().join(FOLDER);
        let path = prepare(
            &l,
            &updates,
            "1.9.0",
            &release("1.10.0"),
            b"MZ installer",
            None,
        )
        .unwrap();
        assert_eq!(path.file_name().unwrap(), "Plenipo_1.10.0_x64-setup.exe");
        assert_eq!(std::fs::read(&path).unwrap(), b"MZ installer");
        let backups = l.backups().unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].kind, BackupKind::BeforeUpdate);
        assert_eq!(backups[0].version.as_deref(), Some("1.10.0"));
        let installing = l.events_of_types(&[INSTALLING], 5).unwrap();
        assert_eq!(installing[0].payload["from"], "1.9.0");
        assert_eq!(installing[0].payload["to"], "1.10.0");
        // The next start clears the folder.
        clean_up(&updates);
        assert!(!path.exists());
    }

    /// Phase 23: an AppImage's new version waits next to it, runnable, and then takes its place.
    #[test]
    fn an_appimage_is_replaced_by_its_new_version() {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::open(&dir.path().join("ledger").join("plenipo.db")).unwrap();
        let apps = dir.path().join("My Apps");
        std::fs::create_dir_all(&apps).unwrap();
        let appimage = apps.join("Plenipo.AppImage");
        std::fs::write(&appimage, b"old").unwrap();
        let new = prepare(
            &l,
            &dir.path().join(FOLDER),
            "1.9.0",
            &release("1.10.0"),
            b"new",
            Some(&appimage),
        )
        .unwrap();
        assert_eq!(new.parent(), Some(apps.as_path()));
        assert_eq!(new.file_name().unwrap(), ".Plenipo.AppImage.1.10.0.new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&new).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "runnable");
        }
        assert_eq!(
            l.backups().unwrap().len(),
            1,
            "the Ledger is backed up first"
        );
        replace_itself(&new, &appimage).unwrap();
        assert_eq!(std::fs::read(&appimage).unwrap(), b"new");
        assert!(!new.exists());
    }

    #[test]
    fn each_copy_takes_a_new_version_its_own_way() {
        assert_eq!(install_way_from(true, false), InstallWay::Installer);
        assert_eq!(install_way_from(false, true), InstallWay::ReplacesItself);
        assert_eq!(install_way_from(false, false), InstallWay::ByHand);
        // A copy updated by hand never downloads the update itself.
        let u = Updates::new_with(
            "1.9.0",
            UpdateSource {
                endpoint: RELEASES_ENDPOINT.into(),
                public_key: Some("key".into()),
            },
            InstallWay::ByHand,
        );
        u.lock().available = Some(release("1.10.0"));
        assert_eq!(u.status().how, InstallWay::ByHand);
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let guard = Guard::new(Arc::clone(&ledger));
        let refused = tauri::async_runtime::block_on(u.download(&guard)).unwrap_err();
        assert_eq!(refused, BY_HAND);
        assert_eq!(u.status().state, UpdateState::NotChecked);
    }

    #[test]
    fn the_new_version_waits_only_for_a_real_earlier_one() {
        let started = std::time::Instant::now();
        // No switch, a bad one, and this program itself: no wait.
        wait_for_previous(["plenipo".to_owned()]);
        wait_for_previous(["plenipo".to_owned(), format!("{WAIT_FOR}x")]);
        wait_for_previous([format!("{WAIT_FOR}{}", std::process::id())]);
        // One that has already closed.
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let gone = child.id();
        child.wait().unwrap();
        wait_for_previous([format!("{WAIT_FOR}{gone}")]);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_newer_version_is_announced_once() {
        let l = Ledger::open_in_memory().unwrap();
        announce(&l, "1.10.0");
        announce(&l, "1.10.0");
        announce(&l, "1.11.0");
        assert_eq!(l.events_of_types(&[AVAILABLE], 10).unwrap().len(), 2);
    }

    #[test]
    fn a_failed_install_keeps_the_offer_and_says_why() {
        let l = Ledger::open_in_memory().unwrap();
        let u = Updates::new(
            "1.9.0",
            UpdateSource {
                endpoint: RELEASES_ENDPOINT.into(),
                public_key: Some("key".into()),
            },
        );
        u.lock().available = Some(release("1.10.0"));
        u.install_failed(&l, "The installer could not be started.".into());
        let s = u.status();
        assert_eq!(s.state, UpdateState::Failed);
        assert_eq!(s.available.unwrap().version, "1.10.0");
        assert_eq!(
            s.message.as_deref(),
            Some("The installer could not be started.")
        );
        assert_eq!(l.events_of_types(&[FAILED], 5).unwrap().len(), 1);
    }
}
