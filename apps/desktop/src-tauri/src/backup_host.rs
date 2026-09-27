//! Ledger backups while Plenipo runs (Phase 13): one a day, one when a new version of Plenipo
//! first starts (before it writes anything), and the restore chosen in Diagnostics, carried out
//! at the next start (`plenipo_ledger::backups`).

use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::Duration;

use plenipo_ledger::{BackupInfo, BackupKind, Ledger, LedgerBackup, NewEvent};
use serde_json::json;

use crate::recovery::APP_SETTING;

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// The first daily check waits this long after start (starting stays quick).
const FIRST_CHECK: Duration = Duration::from_secs(10 * 60);
/// Then it looks again this often.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// A daily backup waits for work to stop, but not for more than this.
const WAIT_FOR_IDLE_MS: u64 = 2 * DAY_MS;
/// Ledger event recording a backup Plenipo made.
pub const BACKED_UP: &str = "ledger.backed_up";

/// A daily backup is due: none of that kind in the last day.
pub fn daily_due(backups: &[LedgerBackup], now: u64) -> bool {
    backups
        .iter()
        .filter(|b| b.kind == BackupKind::Daily)
        .map(|b| b.created_at)
        .max()
        .is_none_or(|last| now.saturating_sub(last) >= DAY_MS)
}

fn last_daily(backups: &[LedgerBackup]) -> Option<u64> {
    backups
        .iter()
        .filter(|b| b.kind == BackupKind::Daily)
        .map(|b| b.created_at)
        .max()
}

/// Record a backup Plenipo made in the Ledger (so the Activity trail and Diagnostics show it).
pub fn record(ledger: &Ledger, kind: BackupKind, info: &BackupInfo, actor: &str) {
    let name = Path::new(&info.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let event = NewEvent {
        source: actor.into(),
        event_type: BACKED_UP.into(),
        payload: json!({ "kind": kind, "name": name, "sizeBytes": info.size_bytes }),
        ..NewEvent::default()
    };
    if let Err(e) = ledger.append_event(event) {
        log::warn!("a backup could not be recorded in the Ledger: {e}");
    }
}

/// The Plenipo version that last used this Ledger (`None` before 1.9.0 wrote it).
pub fn last_version(ledger: &Ledger) -> Option<String> {
    ledger
        .setting(APP_SETTING)
        .ok()
        .flatten()
        .and_then(|v| v["lastVersion"].as_str().map(str::to_owned))
}

/// Right after the Ledger opens, before anything writes to it: when a different version of
/// Plenipo used this Ledger last, back it up first ("before every upgrade") and record the
/// change. `existed`: the Ledger's file was there before this start (not a fresh install).
/// Returns the backup's name when one was made.
pub fn before_upgrade(ledger: &Ledger, existed: bool, version: &str) -> Option<String> {
    ledger.path()?;
    let last = last_version(ledger);
    if last.as_deref() == Some(version) {
        return None;
    }
    let mut made = None;
    if existed {
        // Installing an update already made one, from the old version, just before.
        let now = plenipo_ledger::now_ms();
        let recent_update = ledger.backups().unwrap_or_default().into_iter().any(|b| {
            b.kind == BackupKind::BeforeUpdate
                && b.version.as_deref() == Some(version)
                && now.saturating_sub(b.created_at) < DAY_MS
        });
        if recent_update {
            log::info!("a backup was made just before this update; no second one is needed");
        } else {
            let from = last.as_deref().unwrap_or("earlier");
            match ledger.backup_of_kind(BackupKind::BeforeUpgrade, Some(from)) {
                Ok(info) => {
                    log::info!("the Ledger was backed up before its first use by {version}");
                    record(ledger, BackupKind::BeforeUpgrade, &info, "plenipo");
                    made = Path::new(&info.path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned());
                }
                Err(e) => {
                    log::error!("the Ledger could not be backed up before the upgrade: {e}");
                    ledger.add_notice(format!(
                        "Plenipo could not back up the Ledger after being updated ({e}). Your \
                         Ledger is unchanged; make a backup in Diagnostics."
                    ));
                }
            }
        }
        let event = NewEvent {
            source: "plenipo".into(),
            event_type: "plenipo.version_changed".into(),
            payload: json!({ "from": last, "to": version, "backup": made }),
            ..NewEvent::default()
        };
        if let Err(e) = ledger.append_event(event) {
            log::warn!("the new version could not be recorded: {e}");
        }
    }
    if let Err(e) = ledger.merge_setting(APP_SETTING, &json!({ "lastVersion": version }), "plenipo")
    {
        log::warn!("the version could not be kept in the Ledger: {e}");
    }
    made
}

/// Make today's backup if it is due. `busy`: work is running (a backup waits for it, for up
/// to two days). Returns what was made.
pub fn daily_now(ledger: &Ledger, busy: bool) -> Option<BackupInfo> {
    ledger.path()?;
    let backups = ledger.backups().ok()?;
    let now = plenipo_ledger::now_ms();
    if !daily_due(&backups, now) {
        return None;
    }
    let overdue =
        last_daily(&backups).is_none_or(|last| now.saturating_sub(last) >= WAIT_FOR_IDLE_MS);
    if busy && !overdue {
        return None;
    }
    match ledger.backup_of_kind(BackupKind::Daily, None) {
        Ok(info) => {
            log::info!("the daily backup of the Ledger was made");
            record(ledger, BackupKind::Daily, &info, "plenipo");
            Some(info)
        }
        Err(e) => {
            log::error!("the daily backup of the Ledger could not be made: {e}");
            None
        }
    }
}

/// Back up the Ledger once a day while Plenipo runs.
pub fn start_daily(ledger: &Arc<Ledger>, busy: impl Fn() -> bool + Send + 'static) {
    if ledger.path().is_none() {
        return;
    }
    let weak: Weak<Ledger> = Arc::downgrade(ledger);
    let _ = std::thread::Builder::new()
        .name("plenipo-daily-backup".into())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK);
            loop {
                let Some(ledger) = weak.upgrade() else {
                    return;
                };
                daily_now(&ledger, busy());
                drop(ledger);
                std::thread::sleep(CHECK_EVERY);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup(kind: BackupKind, created_at: u64) -> LedgerBackup {
        LedgerBackup {
            name: format!("x-{created_at}.db"),
            kind,
            created_at,
            size_bytes: 1,
            version: None,
            schema_version: Some(1),
            restorable: true,
            problem: None,
        }
    }

    #[test]
    fn a_daily_backup_is_due_once_a_day() {
        let now = 10 * DAY_MS;
        assert!(daily_due(&[], now));
        assert!(!daily_due(&[backup(BackupKind::Daily, now - 1000)], now));
        assert!(daily_due(&[backup(BackupKind::Daily, now - DAY_MS)], now));
        // Other kinds do not count as today's.
        assert!(daily_due(&[backup(BackupKind::Manual, now - 1000)], now));
    }

    fn ledger() -> (tempfile::TempDir, Ledger) {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::open(&dir.path().join("ledger").join("plenipo.db")).unwrap();
        (dir, l)
    }

    #[test]
    fn the_daily_backup_waits_for_work_to_stop_and_is_made_once() {
        let (_dir, l) = ledger();
        assert!(
            daily_now(&l, false).is_some(),
            "the first one is not waited for"
        );
        assert!(daily_now(&l, false).is_none(), "one a day");
        let kinds: Vec<_> = l.backups().unwrap().into_iter().map(|b| b.kind).collect();
        assert_eq!(kinds, [BackupKind::Daily]);
        let recorded = l.events_of_types(&[BACKED_UP], 10).unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].payload["kind"], "daily");
        // A temporary Ledger has nothing to back up.
        assert!(daily_now(&Ledger::open_in_memory().unwrap(), false).is_none());
    }

    #[test]
    fn a_new_version_backs_up_the_ledger_before_its_first_use() {
        let (_dir, l) = ledger();
        // A fresh install: nothing to back up, the version is kept.
        assert_eq!(before_upgrade(&l, false, "1.9.0"), None);
        assert_eq!(last_version(&l).as_deref(), Some("1.9.0"));
        // The same version again: nothing.
        assert_eq!(before_upgrade(&l, true, "1.9.0"), None);
        // A new version: a backup named after the version before.
        let made = before_upgrade(&l, true, "1.10.0").unwrap();
        assert!(made.starts_with("pre-upgrade-1.9.0-"), "{made}");
        assert_eq!(last_version(&l).as_deref(), Some("1.10.0"));
        let changed = l.events_of_types(&["plenipo.version_changed"], 5).unwrap();
        assert_eq!(changed[0].payload["from"], "1.9.0");
        assert_eq!(changed[0].payload["to"], "1.10.0");
    }

    #[test]
    fn a_ledger_from_before_1_9_is_backed_up_as_an_earlier_version() {
        let (_dir, l) = ledger();
        let made = before_upgrade(&l, true, "1.9.0").unwrap();
        assert!(made.starts_with("pre-upgrade-earlier-"), "{made}");
    }

    #[test]
    fn an_update_that_already_made_a_backup_is_not_backed_up_twice() {
        let (_dir, l) = ledger();
        before_upgrade(&l, false, "1.9.0");
        l.backup_of_kind(BackupKind::BeforeUpdate, Some("1.10.0"))
            .unwrap();
        assert_eq!(before_upgrade(&l, true, "1.10.0"), None);
        assert_eq!(last_version(&l).as_deref(), Some("1.10.0"));
    }
}
