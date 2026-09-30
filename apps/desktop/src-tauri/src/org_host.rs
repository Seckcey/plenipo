//! Opening one organization (Phase 21, ADR-094 §3): its Ledger, and around it the full set of
//! services every organization has — the Supervisor that runs its programs, its AI tools'
//! sessions, Liaison, the Router, Guard and its tool server, its notices, and its Workforce
//! service. The first organization is opened this way when Plenipo starts; every other one
//! too, and when it is made or brought back.

use std::sync::Arc;
use std::time::Duration;

use plenipo_capabilities::control::ControlCenter;
use plenipo_ledger::Ledger;
use plenipo_liaison::{Liaison, LiaisonConfig};
use plenipo_router::Router;
use plenipo_workforce::Workforce;
use tauri::{AppHandle, Runtime};

use crate::orgs::{OrgPlace, OrgStack};
use crate::runtime_host::Persistence;
use crate::{agent_host, backup_host, guard_host, ledger_host, notices, recovery, runtime_host};

/// Your choices for the PC kept in each Ledger (notices, the terminal's shell, closing the
/// window): the first organization's, copied to every other one (ADR-094 §6).
pub const PREFERENCES: &str = "preferences";

/// How every organization is opened in this run: the app's own options (app state).
#[derive(Debug, Clone, Copy)]
pub struct Defaults {
    pub persistence: Persistence,
    pub notices: notices::Output,
    pub gather: Duration,
    /// Run each organization's work (not in the IPC tests).
    pub run: bool,
}

/// How to open an organization.
#[derive(Clone)]
pub struct Opening<'a> {
    pub persistence: Persistence,
    pub notices: notices::Output,
    pub gather: Duration,
    /// This version of Plenipo (a new one backs the Ledger up first).
    pub version: &'a str,
    /// How the last run ended (what it left unfinished is recorded as recovered).
    pub previous: &'a recovery::PreviousEnd,
    /// Run the organization's work: its tool server, Liaison's loop, the AI tools' detection,
    /// and its daily backup (not in the IPC tests).
    pub run: bool,
    /// Who uses the browser, the screen, and servers: the PC's one record.
    pub control: ControlCenter,
    /// The first organization, once open: your Workforce, your tile, and your choices for the PC.
    pub first: Option<&'a OrgStack>,
}

/// What opening the Ledger found, for the first organization's own start (its restore note
/// and its layout change are the run note's).
pub struct LedgerOpened {
    pub ledger: Arc<Ledger>,
    pub restored: Option<plenipo_ledger::RestoreOutcome>,
    pub existed: bool,
    pub changed_layout: bool,
}

/// Open organization `place`'s Ledger: a restore chosen in Diagnostics first, then the Ledger
/// (a layout change backs it up itself), then the backup a new version makes.
pub fn open_ledger<R: Runtime>(
    app: &AppHandle<R>,
    place: &OrgPlace,
    persistence: Persistence,
    version: &str,
    before_open: impl FnOnce(bool),
) -> LedgerOpened {
    let db = match persistence {
        Persistence::AppData => place.folder.as_deref().map(ledger_host::ledger_file),
        Persistence::InMemory => None,
    };
    let restored = db.as_deref().and_then(ledger_host::apply_pending_restore);
    let existed = db.as_ref().is_some_and(|p| p.exists());
    let changed_layout = existed && db.as_deref().is_some_and(ledger_host::needs_layout_change);
    before_open(changed_layout);
    let ledger = ledger_host::open(app, place, persistence);
    backup_host::before_upgrade(&ledger, existed, changed_layout, version);
    if let Some(restored) = &restored {
        ledger_host::record_restore(&ledger, restored);
    }
    LedgerOpened {
        ledger,
        restored,
        existed,
        changed_layout,
    }
}

/// Build the organization's services around its open Ledger.
pub fn build<R: Runtime>(
    app: &AppHandle<R>,
    place: OrgPlace,
    ledger: Arc<Ledger>,
    how: &Opening<'_>,
) -> Arc<OrgStack> {
    // What was in progress when the last run ended, before the services mark it stopped.
    let before = if how.previous.is_unclean() {
        recovery::in_progress(&ledger)
    } else {
        recovery::InProgress::default()
    };
    if let Some(first) = how.first {
        copy_preferences(&first.ledger, &ledger);
    }
    // When this organization's services began: spending set aside before it belongs to its last
    // run.
    let started = plenipo_ledger::now_ms();
    let supervisor = runtime_host::create_supervisor(app, &place, how.persistence, ledger.clone());
    let agents = agent_host::create(
        app,
        &place,
        how.persistence,
        ledger.clone(),
        supervisor.clone(),
    );
    if how.run && how.persistence == Persistence::AppData {
        agent_host::detect_in_background(&agents);
    }
    // Liaison (Phase 4): handoffs between workers, reconciled from the Ledger.
    let liaison = Liaison::new(ledger.clone(), agents.clone(), LiaisonConfig::default());
    if how.run {
        tauri::async_runtime::spawn(liaison.clone().run());
    }
    // Router (Phase 6): model registry and role model policies.
    let router = Router::new(ledger.clone(), agents.clone());
    // Guard and the capability broker (Phase 7): permissions, Plenipo's tools for workers,
    // approvals, and the Vault, under this organization's own name.
    let (guard, broker, watchers) = guard_host::create(
        app,
        &place,
        how.persistence,
        ledger.clone(),
        supervisor.clone(),
        &agents,
        how.control.clone(),
    );
    if how.run {
        guard_host::start(&broker);
    }
    // Pop-up notices (Phase 12): what needs the owner, from each committed event.
    let notices = Arc::new(notices::start(
        app,
        ledger.clone(),
        Some(agents.clone()),
        how.notices,
        how.gather,
        notices::NoticeOrg {
            id: place.id.clone(),
            home: how.first.map(|f| f.ledger.clone()),
        },
    ));
    // Money still set aside for paid tasks from before belongs to tasks that stopped with the
    // last run (Phase 16 Wave 3, ADR-085): each counts at the most it could have cost, so a
    // spending cap is never passed unseen. After the notices start, so a cap this reaches is
    // told; a task this run started is left alone.
    if how.run {
        match ledger.recover_spending(started, plenipo_ledger::now_ms()) {
            Ok(0) => {}
            Ok(n) => log::info!(
                "{n} paid task(s) from the last run count at the most they could have cost"
            ),
            Err(e) => log::warn!("could not settle spending left from the last run: {e}"),
        }
    }
    // Workforce (Phase 5): the organization, and Liaison's directory for its members; your
    // Workforce and tile are the first organization's (ADR-094 §5).
    let workforce = Workforce::new(
        ledger.clone(),
        agents.clone(),
        liaison.clone(),
        router.clone(),
    );
    if let Some(first) = how.first {
        workforce.share_with(first.ledger.clone());
    }
    // Built-in roles get their starting permission sets once (after they are seeded).
    if let Err(e) = guard.seed_template_roles() {
        log::warn!("could not give the built-in roles their permissions: {e}");
    }
    // What the last run left unfinished is recorded as recovered (Phase 13).
    recovery::record(&ledger, how.previous, &before, how.version);
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if how.run {
        let busy = supervisor.clone();
        backup_host::start_daily(&ledger, stopped.clone(), move || busy.active_count() > 0);
    }
    Arc::new(OrgStack {
        stopped,
        place,
        ledger,
        supervisor,
        agents,
        liaison,
        router,
        guard,
        broker,
        workforce,
        notices,
        watchers,
    })
}

/// Copy your choices for the PC from the first organization's Ledger to `to` (no event: they
/// are the first organization's, recorded there).
pub fn copy_preferences(from: &Ledger, to: &Ledger) {
    let Ok(Some(prefs)) = from.setting(PREFERENCES) else {
        return;
    };
    if let Err(e) = to.change_setting(PREFERENCES, |_| Ok(prefs.clone())) {
        log::warn!("could not copy your choices to an organization: {e}");
    }
}

/// Your choices for the PC changed in the first organization: every other one follows.
pub fn preferences_changed(stacks: &[Arc<OrgStack>]) {
    let Some(first) = stacks.iter().find(|s| s.place.is_first()) else {
        return;
    };
    for s in stacks.iter().filter(|s| !s.place.is_first()) {
        copy_preferences(&first.ledger, &s.ledger);
    }
}

/// Stop an organization's work the normal way (Quit, archiving it, deleting it): Liaison stops
/// handing out work, its terminals end, then its AI tool turns and programs stop. Returns how
/// many programs were stopped.
pub async fn stop(stack: &OrgStack, why: &str, grace: Duration) -> usize {
    stack
        .stopped
        .store(true, std::sync::atomic::Ordering::SeqCst);
    stack.liaison.shutdown();
    stack.broker.stop_server();
    stack.broker.close_all_terminals(why);
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while !stack.broker.open_terminals().is_empty() && std::time::Instant::now() < deadline {
        tokio_sleep(Duration::from_millis(25)).await;
    }
    stack.agents.shutdown(grace).await + stack.supervisor.shutdown(grace).await
}

async fn tokio_sleep(d: Duration) {
    tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d))
        .await
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orgs::{self, OrgPlace, FIRST};

    fn place(data: &std::path::Path, id: &str) -> OrgPlace {
        OrgPlace {
            id: id.to_owned(),
            folder: Some(orgs::folder_of(data, id)),
            data: Some(data.to_path_buf()),
            vault: orgs::vault_name("test", id),
        }
    }

    /// Phase 21's test: each organization's backups are its own, beside its own Ledger, and a
    /// restore asked for in one never touches another (ADR-094 §10).
    #[test]
    fn each_organizations_backups_are_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let app = tauri::test::mock_app();
        let other = orgs::new_id();
        let open = |id: &str| {
            open_ledger(
                app.handle(),
                &place(dir.path(), id),
                Persistence::AppData,
                "1.15.0",
                |_| {},
            )
            .ledger
        };
        let (first, client) = (open(FIRST), open(&other));
        let (first_file, client_file) = (
            first.path().unwrap().to_owned(),
            client.path().unwrap().to_owned(),
        );
        assert_ne!(first_file, client_file);
        assert!(client_file.starts_with(orgs::folder_of(dir.path(), &other)));
        first.backup(None).unwrap();
        client.backup(None).unwrap();
        client.backup(None).unwrap();
        let mine = plenipo_ledger::backups::overview_at(&first_file).unwrap();
        let theirs = plenipo_ledger::backups::overview_at(&client_file).unwrap();
        assert_eq!(mine.backups.len(), 1);
        assert_eq!(theirs.backups.len(), 2);
        assert_ne!(mine.folder, theirs.folder);
        // A restore asked for in the client's organization waits beside its Ledger only.
        let name = theirs.backups[0].name.clone();
        plenipo_ledger::backups::request_restore(&client_file, &name).unwrap();
        assert!(plenipo_ledger::backups::overview_at(&first_file)
            .unwrap()
            .pending_restore
            .is_none());
        assert!(plenipo_ledger::backups::overview_at(&client_file)
            .unwrap()
            .pending_restore
            .is_some());
    }
}
