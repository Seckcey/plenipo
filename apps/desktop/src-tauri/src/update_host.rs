//! Updates in the desktop app (Phase 13, ADR-038): checking once a day (always on, for Free and
//! Pro alike), telling the owner, and installing only when the owner says so.
//!
//! Installing goes in order and stops at the first problem, leaving this version installed:
//! download and check the installer ([`plenipo_capabilities::updates`]), back up the Ledger,
//! stop the work the normal way, start the installer ("progress bar only", as an update, and
//! reopen Plenipo after), and quit.
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
use plenipo_core::{AvailableUpdate, UpdateState, UpdateStatus};
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
    inner: Mutex<Inner>,
}

impl Updates {
    pub fn new(version: &str, source: UpdateSource) -> Arc<Self> {
        Arc::new(Self {
            source,
            version: version.to_owned(),
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
    std::fs::create_dir_all(dir).map_err(|e| {
        format!("The update was not installed: its folder could not be made ({e}).")
    })?;
    let clean: String = release
        .version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .collect();
    let path = dir.join(format!("Plenipo_{clean}_x64-setup.exe"));
    std::fs::write(&path, installer)
        .map_err(|e| format!("The update was not installed: it could not be saved ({e})."))?;
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
        let path = prepare(&l, &updates, "1.9.0", &release("1.10.0"), b"MZ installer").unwrap();
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
