//! Plenipo on your phone (Phase 14, ADR-140 to ADR-147): the app's side of phone access.
//!
//! [`plenipo_remote::Remote`] holds pairing, the sealed meetings, sign-in, and Guard's decision on
//! every request. This module gives it the app: the license (phone access is Pro), Guard's check of
//! the relay's address, the Vault (the first organization's, like the license key), the Ledger
//! (each request recorded in the organization it touches), and each organization's own services,
//! which carry out a request the same way the main window does — never through the window's
//! commands (the plan's Architecture).
//!
//! Where the relay and the phone's page are is built into each copy: Plenipo's relay name and
//! `remote.getplenipo.com`, or, in copies built for the tests only, stand-ins on this computer
//! (`PLENIPO_REMOTE_STAND_IN`, `PLENIPO_REMOTE_PAGE` at build time). A release connects to the
//! relay only when it was built with `PLENIPO_RELAY_LIVE` (the relay change is live, ADR-140 §4);
//! until then the switch says "Coming soon". Never a setting, never an environment variable at
//! run time.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use plenipo_capabilities::vault::{self, SecretStore};
use plenipo_guard::outbound::{Purpose, RELAY_ADDRESS};
use plenipo_guard::remote::ApprovalFacts;
use plenipo_guard::{OutboundRules, OWNER, PLENIPO};
use plenipo_ledger::{ActivityScope, Ledger, LedgerEvent, NewEvent};
use plenipo_licensing::{Limit, SignedAnswer};
use plenipo_remote::devices::{ConfigFile, Kept, KeyStore};
use plenipo_remote::link::LinkHost;
use plenipo_remote::protocol::{Ask, Changed, SignedOutWhy};
use plenipo_remote::service::{RemoteSettings, RemoteView};
use plenipo_remote::{Change, Host, Phone, Remote, Settings, SystemClock};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::license_host::LicenseHost;
use crate::orgs::{OrgStack, Orgs};

/// Told to every window when phone access changes (Settings → Devices reads it again).
pub const REMOTE_EVENT: &str = "plenipo://remote";
/// The switch and the approvals kept on the PC, in Plenipo's data folder (not secret).
pub const CONFIG_FILE: &str = "remote.json";
/// How often the app looks at whether the relay link should run.
const LOOK_EVERY: Duration = Duration::from_secs(2);
/// Changes that arrive within this long of each other are told to the phones once.
const GATHER: Duration = Duration::from_millis(300);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where this copy's phone access goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The relay's address for PCs (Guard checks it each time).
    pub relay: String,
    /// The rules Guard checks it against.
    pub rules: OutboundRules,
    /// The phone's page (the pairing link, and the passkeys' page).
    pub origin: String,
    /// The passkeys' site.
    pub rp_id: String,
    /// The relay is live for this copy: otherwise the switch says "Coming soon".
    pub live: bool,
}

/// This copy's phone access, as built.
pub fn built() -> Built {
    built_for(
        option_env!("PLENIPO_REMOTE_STAND_IN"),
        option_env!("PLENIPO_REMOTE_PAGE"),
        option_env!("PLENIPO_RELAY_LIVE"),
        plenipo_licensing::trust::built_for_tests(),
    )
}

/// What `built` makes of the build's settings (the tests try each).
pub(crate) fn built_for(
    stand_in: Option<&str>,
    page: Option<&str>,
    live: Option<&str>,
    for_tests: bool,
) -> Built {
    // Stand-ins are only for copies built for the tests, and only on this computer.
    let stand_in = stand_in
        .filter(|_| for_tests)
        .map(str::trim)
        .filter(|b| b.starts_with("http://127.0.0.1:"))
        .map(|b| b.trim_end_matches('/').to_owned());
    let page = page
        .filter(|_| for_tests)
        .map(str::trim)
        .filter(|p| p.starts_with("http://localhost:") || p.starts_with("http://127.0.0.1:"))
        .map(|p| p.trim_end_matches('/').to_owned());
    let rules = OutboundRules::default().with_relay_stand_in(stand_in.as_deref());
    let (relay, live) = match (&stand_in, rules.relay_test_port) {
        (Some(base), Some(_)) => (
            format!("{base}{}", plenipo_guard::outbound::RELAY_PATH),
            true,
        ),
        _ => (
            RELAY_ADDRESS.to_owned(),
            live.is_some_and(|l| matches!(l.trim(), "1" | "true" | "yes")),
        ),
    };
    let (origin, rp_id) = match page {
        Some(p) => {
            let host = url_host(&p).unwrap_or_else(|| "localhost".into());
            (p, host)
        }
        None => (
            plenipo_remote::PAGE_ORIGIN.to_owned(),
            plenipo_remote::RP_ID.to_owned(),
        ),
    };
    Built {
        relay,
        rules,
        origin,
        rp_id,
        live,
    }
}

fn url_host(address: &str) -> Option<String> {
    let rest = address.split_once("://")?.1;
    let host = rest.split(['/', ':']).next()?;
    (!host.is_empty()).then(|| host.to_owned())
}

/// The Vault, as phone access keeps its keys and phones in it.
struct VaultKeys(Arc<dyn SecretStore>);

impl KeyStore for VaultKeys {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        vault::read(self.0.as_ref(), id).map_err(|e| e.to_string())
    }

    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        vault::put(self.0.as_ref(), id, value).map_err(|e| e.to_string())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        vault::erase(self.0.as_ref(), id).map_err(|e| e.to_string())
    }
}

/// `remote.json` in Plenipo's data folder (or memory, when nothing is kept).
struct ConfigInFolder {
    file: Option<PathBuf>,
    memory: Mutex<Option<String>>,
}

impl ConfigFile for ConfigInFolder {
    fn read(&self) -> Option<String> {
        match &self.file {
            Some(f) => std::fs::read_to_string(f).ok(),
            None => lock(&self.memory).clone(),
        }
    }

    fn write(&self, text: &str) -> Result<(), String> {
        match &self.file {
            Some(f) => {
                let temp = f.with_extension("json.tmp");
                std::fs::write(&temp, text)
                    .and_then(|()| std::fs::rename(&temp, f))
                    .map_err(|e| e.to_string())
            }
            None => {
                *lock(&self.memory) = Some(text.to_owned());
                Ok(())
            }
        }
    }
}

/// Phone access, as the app holds it (app state).
pub struct RemoteState {
    pub remote: Arc<Remote>,
    pub built: Built,
    /// The app, as phone access sees it (the tests call it directly).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) host: Arc<dyn Host>,
    /// The relay link, while it runs.
    link: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Whether the PC was on Pro at the last look (to pause phone access once when it ends).
    was_pro: Mutex<bool>,
    /// Changes in each organization, for the phones.
    changes: Mutex<Option<mpsc::Sender<(String, Changed)>>>,
    /// The link runs only for a copy that keeps its data (the IPC tests keep none).
    pub may_connect: bool,
    /// Plenipo is quitting: the link is closed, and the look no longer starts it.
    quitting: AtomicBool,
}

/// The app, as phone access sees it.
struct AppSide<R: Runtime> {
    app: AppHandle<R>,
    license: Arc<LicenseHost>,
    built: Built,
}

impl<R: Runtime> AppSide<R> {
    fn orgs(&self) -> Result<Arc<Orgs>, String> {
        self.app
            .try_state::<Arc<Orgs>>()
            .map(|o| o.inner().clone())
            .ok_or_else(|| "Plenipo is still starting.".to_owned())
    }

    fn stack(&self, org: &str) -> Result<Arc<OrgStack>, String> {
        self.orgs()?
            .stack(org)
            .ok_or_else(|| "That organization is not open on your PC.".to_owned())
    }

    fn first(&self) -> Result<Arc<OrgStack>, String> {
        self.orgs()?
            .first()
            .ok_or_else(|| "Plenipo is still starting.".to_owned())
    }
}

fn value<T: Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn plain<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl<R: Runtime> Host for AppSide<R> {
    fn pro(&self) -> bool {
        self.license
            .entitlements()
            .check(Limit::PhoneAccess)
            .is_allowed()
    }

    fn approval(&self, org: &str, approval: &str) -> Option<ApprovalFacts> {
        let stack = self.stack(org).ok()?;
        let queue = stack.broker.approvals().ok()?;
        queue
            .pending
            .iter()
            .find(|a| a.id == approval)
            .map(|a| ApprovalFacts {
                sensitive: a.sensitive,
                environment: a.environment,
            })
    }

    fn carry_out(&self, phone: &Phone, ask: &Ask) -> Result<Value, String> {
        carry_out(self, phone, ask)
    }

    fn record(&self, org: Option<&str>, event: &str, payload: Value) {
        let ledger = org
            .and_then(|o| self.stack(o).ok())
            .or_else(|| self.first().ok())
            .map(|s| s.ledger.clone());
        let Some(ledger) = ledger else {
            return;
        };
        // What the owner did, from a phone or at the PC, is the owner's; the rest is Plenipo's.
        let owners = [
            "remote.request",
            "remote.refused",
            "remote.signed_in",
            "remote.device_added",
            "remote.device_renamed",
            "remote.device_removed",
            "remote.device_unpaused",
            "remote.switched_on",
            "remote.switched_off",
            "remote.kept_on_pc_changed",
        ];
        let source = if owners.contains(&event) {
            OWNER
        } else {
            PLENIPO
        };
        if let Err(e) = ledger.append_event(NewEvent {
            source: source.into(),
            event_type: event.into(),
            payload,
            ..NewEvent::default()
        }) {
            log::warn!("could not record {event}: {e}");
        }
    }

    fn changed(&self, what: Change) {
        let word = match what {
            Change::Devices => "devices",
            Change::Pairing => "pairing",
            Change::Switch => "switch",
            Change::Attempts => "attempts",
        };
        let _ = self.app.emit(REMOTE_EVENT, word);
    }
}

impl<R: Runtime> LinkHost for AppSide<R> {
    fn check_address(&self, address: &str) -> Result<(), String> {
        let first = self.first()?;
        first
            .guard
            .check_outbound(&self.built.rules, Purpose::PhoneAccess, address)
            .map(drop)
    }

    fn weekly_answer(&self) -> Option<SignedAnswer> {
        self.license.signed_answer()
    }
}

/// Carry out a request Guard allowed, with each organization's own services (the same core
/// functions as the main window's commands).
fn carry_out<R: Runtime>(side: &AppSide<R>, phone: &Phone, ask: &Ask) -> Result<Value, String> {
    use plenipo_ledger::WorkOf;
    let app = &side.app;
    match ask {
        Ask::ReadOrganizations => {
            let orgs = side.orgs()?;
            let open: Vec<Value> = orgs
                .entries()
                .into_iter()
                .filter(|e| e.archived_at.is_none() && orgs.stack(&e.id).is_some())
                .map(|e| json!({ "id": e.id, "name": e.name }))
                .collect();
            Ok(json!({
                "pcName": pc_name(),
                "organizations": open,
                "version": app.package_info().version.to_string(),
            }))
        }
        Ask::ReadHome { org } => value(side.stack(org)?.workforce.home().map_err(plain)?),
        Ask::ReadApprovals { org } => {
            let orgs = side.orgs()?;
            let kept = app
                .try_state::<Arc<RemoteState>>()
                .map(|s| s.remote.kept_on_pc())
                .unwrap_or_default();
            let wanted: Vec<Arc<OrgStack>> = match org {
                Some(o) => vec![side.stack(o)?],
                None => orgs.stacks(),
            };
            let mut all = Vec::new();
            for stack in wanted {
                let queue = stack.broker.approvals().map_err(plain)?;
                let on_pc: Vec<&str> = queue
                    .pending
                    .iter()
                    .filter(|a| {
                        kept.keeps(ApprovalFacts {
                            sensitive: a.sensitive,
                            environment: a.environment,
                        })
                    })
                    .map(|a| a.id.as_str())
                    .collect();
                all.push(json!({
                    "org": stack.id(),
                    "name": orgs.entry(stack.id()).map(|e| e.name).unwrap_or_default(),
                    "queue": queue,
                    "keptOnPc": on_pc,
                }));
            }
            Ok(Value::Array(all))
        }
        Ask::ReadOrganization { org } => {
            value(side.stack(org)?.workforce.snapshot().map_err(plain)?)
        }
        Ask::ReadProjects { org, project } => {
            let stack = side.stack(org)?;
            match project {
                Some(id) => value(
                    stack
                        .ledger
                        .work_record(WorkOf::Project(id), 100)
                        .map_err(plain)?,
                ),
                None => value(stack.workforce.snapshot().map_err(plain)?),
            }
        }
        Ask::ReadWorkers { org, position } => {
            let stack = side.stack(org)?;
            match position {
                Some(id) => value(stack.workforce.work(Some(id)).map_err(plain)?),
                None => value(stack.workforce.snapshot().map_err(plain)?),
            }
        }
        Ask::ReadTasks {
            org,
            task,
            conversation,
        } => {
            let stack = side.stack(org)?;
            match (task, conversation) {
                (Some(t), _) => value(stack.broker.task_record(t).map_err(plain)?),
                (None, Some(c)) => {
                    value(tauri::async_runtime::block_on(stack.agents.session(c)).map_err(plain)?)
                }
                (None, None) => value(stack.ledger.list_tasks(200).map_err(plain)?),
            }
        }
        Ask::ReadActivity { org, before } => {
            let stack = side.stack(org)?;
            let before = before.and_then(|b| u64::try_from(b).ok());
            value(
                stack
                    .ledger
                    .scope_events(&ActivityScope::All, before, 50)
                    .map_err(plain)?,
            )
        }
        Ask::ReadAiTools => {
            let tools = app
                .try_state::<plenipo_capabilities::ai_tools::AiTools>()
                .ok_or("Plenipo is still starting.")?;
            value(tools.page())
        }
        Ask::ReadDiagnostics => {
            let first = side.first()?;
            Ok(json!({
                "ledger": value(first.ledger.status().map_err(plain)?)?,
                "startAndClose": value(crate::start_close::settings(app, &first.ledger))?,
                "version": app.package_info().version.to_string(),
            }))
        }
        Ask::ReadControl => {
            let first = side.first()?;
            Ok(json!({
                "control": value(first.broker.control_status())?,
                "recovery": value(crate::upkeep_commands::recovery_status(app, None).map_err(|e| e.message)?)?,
            }))
        }
        Ask::ReadLessons { org } => value(side.stack(org)?.workforce.learning().map_err(plain)?),
        Ask::Approve { org, approval } | Ask::Refuse { org, approval } => {
            let approve = matches!(ask, Ask::Approve { .. });
            let view = side
                .stack(org)?
                .broker
                .resolve_approval_via(approval, approve, OWNER, Some(&phone.name))
                .map_err(plain)?;
            value(view)
        }
        Ask::StopAll => {
            let first = side.first()?;
            let status = tauri::async_runtime::block_on(crate::commands::stop_control_everywhere(
                app,
                first.broker.clone(),
            ))
            .map_err(plain)?;
            value(status)
        }
        Ask::AllowAgain => {
            let orgs = side.orgs()?;
            for s in orgs.stacks() {
                s.broker.allow_control(OWNER).map_err(plain)?;
            }
            value(side.first()?.broker.control_status())
        }
        Ask::StopTask { org, conversation } => {
            crate::commands::validate_session_id(conversation).map_err(|e| e.message)?;
            let stack = side.stack(org)?;
            value(
                tauri::async_runtime::block_on(stack.agents.cancel_turn(conversation))
                    .map_err(plain)?,
            )
        }
        Ask::RunAgain { org, task } => {
            let stack = side.stack(org)?;
            tauri::async_runtime::block_on(crate::upkeep_commands::run_again_core(
                &stack.ledger,
                &stack.workforce,
                &stack.liaison,
                task,
            ))
            .map_err(|e| e.message)?;
            value(crate::upkeep_commands::recovery_status(app, None).map_err(|e| e.message)?)
        }
        Ask::LeaveStopped { notice } => {
            let first = side.first()?;
            crate::recovery::dismiss(&first.ledger, notice).map_err(plain)?;
            value(crate::upkeep_commands::recovery_status(app, None).map_err(|e| e.message)?)
        }
        Ask::KeepLesson { org, lesson } | Ask::DiscardLesson { org, lesson } => {
            let keep = matches!(ask, Ask::KeepLesson { .. });
            value(
                side.stack(org)?
                    .workforce
                    .decide_lesson(lesson, keep, None)
                    .map_err(plain)?,
            )
        }
        Ask::SendObjective {
            org,
            position,
            project,
            text,
        } => {
            crate::commands::validate_objective(text).map_err(|e| e.message)?;
            let stack = side.stack(org)?;
            let detail = tauri::async_runtime::block_on(stack.workforce.give_objective(
                position,
                text,
                project.as_deref(),
            ))
            .map_err(plain)?;
            Ok(json!({
                "conversation": detail.session.id,
                "task": detail.turns.last().map(|t| t.task_id.clone()),
            }))
        }
        // Handled by phone access itself, before Guard's decision reaches here.
        Ask::SignIn { .. }
        | Ask::Outcome { .. }
        | Ask::SignOut
        | Ask::RemoveThisPhone
        | Ask::NoticesOn { .. }
        | Ask::NoticesOff => Err("That is handled by phone access itself.".into()),
    }
}

/// What the phone calls this PC: its computer name.
pub fn pc_name() -> String {
    sysinfo::System::host_name()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Your PC".to_owned())
}

/// Make phone access, with what was kept: the switch in `remote.json`, the PC's keys and the
/// phones in the first organization's Vault (`store`). `data`: Plenipo's data folder (`None`:
/// nothing is kept, and the link never runs — the IPC tests).
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    license: Arc<LicenseHost>,
    store: Arc<dyn SecretStore>,
    data: Option<PathBuf>,
    version: &str,
) -> Arc<RemoteState> {
    create_with(app, license, store, data, version, built())
}

/// [`create`], with where phone access goes given (the tests' stand-ins).
pub(crate) fn create_with<R: Runtime>(
    app: &AppHandle<R>,
    license: Arc<LicenseHost>,
    store: Arc<dyn SecretStore>,
    data: Option<PathBuf>,
    version: &str,
    built: Built,
) -> Arc<RemoteState> {
    let side = Arc::new(AppSide {
        app: app.clone(),
        license,
        built: built.clone(),
    });
    let remote = Remote::new(
        Settings {
            origin: built.origin.clone(),
            rp_id: built.rp_id.clone(),
            pc_name: pc_name(),
            version: version.to_owned(),
        },
        Arc::new(VaultKeys(store)),
        Arc::new(ConfigInFolder {
            file: data.as_ref().map(|d| d.join(CONFIG_FILE)),
            memory: Mutex::new(None),
        }),
        side.clone(),
        Arc::new(SystemClock),
    );
    let state = Arc::new(RemoteState {
        remote,
        built,
        host: side.clone(),
        link: Mutex::new(None),
        was_pro: Mutex::new(side.pro()),
        changes: Mutex::new(None),
        may_connect: data.is_some(),
        quitting: AtomicBool::new(false),
    });
    state.start_telling_phones();
    state
}

impl RemoteState {
    /// Settings → Devices.
    pub fn view(&self) -> RemoteView {
        self.remote.view()
    }

    /// Settings → Devices, with what the screen needs around it.
    pub fn settings(&self, pro: bool) -> RemoteSettings {
        RemoteSettings {
            remote: self.remote.view(),
            pro,
            coming_soon: !self.built.live,
            page: self.built.origin.clone(),
            pc_name: pc_name(),
            sensitive: plenipo_remote::service::SensitiveChoice::all(),
        }
    }

    /// Changes in each organization go to the signed-in phones, gathered.
    fn start_telling_phones(self: &Arc<Self>) {
        let (tx, rx) = mpsc::channel::<(String, Changed)>();
        *lock(&self.changes) = Some(tx);
        let remote = Arc::downgrade(&self.remote);
        let _ = std::thread::Builder::new()
            .name("plenipo-remote-changes".into())
            .spawn(move || loop {
                let Ok(first) = rx.recv() else {
                    return;
                };
                let mut gathered = vec![first];
                loop {
                    match rx.recv_timeout(GATHER) {
                        Ok(more) => {
                            if !gathered.contains(&more) {
                                gathered.push(more);
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                let Some(remote) = remote.upgrade() else {
                    return;
                };
                for (org, what) in gathered {
                    remote.notify(Some(&org), what);
                }
            });
    }

    /// Tell the phones what an organization's committed events changed (called after commit;
    /// quick).
    pub fn ledger_event(&self, org: &str, event: &LedgerEvent) {
        let Some(what) = changed_by(&event.event_type) else {
            return;
        };
        if let Some(tx) = lock(&self.changes).as_ref() {
            let _ = tx.send((org.to_owned(), what));
        }
    }

    /// Start or stop the relay link as the switch, Pro, and the build say; pause phone access
    /// when Pro ends.
    fn look<R: Runtime>(self: &Arc<Self>, app: &AppHandle<R>, license: &LicenseHost) {
        if self.quitting.load(Ordering::SeqCst) {
            return;
        }
        let pro = license
            .entitlements()
            .check(Limit::PhoneAccess)
            .is_allowed();
        {
            let mut was = lock(&self.was_pro);
            if *was && !pro {
                self.remote.pause(SignedOutWhy::ProEnded);
            }
            *was = pro;
        }
        let want = self.may_connect && self.built.live && pro && self.remote.switched_on();
        let mut link = lock(&self.link);
        match (want, link.is_some()) {
            (true, false) => {
                let host = link_host(app, &self.built);
                if let Some(host) = host {
                    let remote = self.remote.clone();
                    let address = self.built.relay.clone();
                    *link = Some(tauri::async_runtime::spawn(async move {
                        plenipo_remote::link::run(remote, address, host).await;
                    }));
                }
            }
            (false, true) => {
                if let Some(running) = link.take() {
                    running.abort();
                }
                drop(link);
                self.remote.relay_down(None);
            }
            _ => {}
        }
    }

    /// Quit: the relay link closes now, and is not started again. The phones are told the PC
    /// went away by the relay, as when it is turned off.
    pub fn stop(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        let running = lock(&self.link).take();
        if let Some(running) = running {
            running.abort();
            self.remote.relay_down(None);
        }
    }
}

fn link_host<R: Runtime>(app: &AppHandle<R>, built: &Built) -> Option<Arc<dyn LinkHost>> {
    let license = app.try_state::<Arc<LicenseHost>>()?.inner().clone();
    Some(Arc::new(AppSide {
        app: app.clone(),
        license,
        built: built.clone(),
    }))
}

/// What a Ledger event changed, for the phones' pages.
fn changed_by(event_type: &str) -> Option<Changed> {
    Some(match event_type {
        "approval.requested" | "approval.resolved" | "approval.expired" => Changed::Approvals,
        "control.stopped" | "control.allowed" | "plenipo.recovered" | "plenipo.run_again" => {
            Changed::Control
        }
        "lesson.added" | "lesson.kept" | "lesson.discarded" | "lesson.removed" => Changed::Lessons,
        "task.created" | "task.state_changed" | "agent.result" => Changed::Tasks,
        "ai_tool.updated"
        | "ai_tool.update_failed"
        | "ai_tool.signed_in"
        | "ai_tool.signed_out" => Changed::AiTools,
        "organization.renamed" => Changed::Organizations,
        _ => return None,
    })
}

/// Look now and then, for as long as Plenipo runs (the relay link starts and stops here).
pub fn start<R: Runtime>(app: &AppHandle<R>, state: Arc<RemoteState>) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("plenipo-remote".into())
        .spawn(move || {
            while !state.quitting.load(Ordering::SeqCst) {
                if let Some(license) = app.try_state::<Arc<LicenseHost>>() {
                    state.look(&app, &license);
                }
                std::thread::sleep(LOOK_EVERY);
            }
        });
}

/// An organization's committed events reach the phones (called when the organization opens).
pub fn watch<R: Runtime>(app: &AppHandle<R>, org: &str, ledger: &Ledger) {
    let Some(state) = app.try_state::<Arc<RemoteState>>() else {
        return;
    };
    let state = Arc::downgrade(state.inner());
    let org = org.to_owned();
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if let Some(s) = state.upgrade() {
            s.ledger_event(&org, event);
        }
    }));
}

/// Every Vault ID phone access uses, in the first organization's Vault (for uninstalling with
/// "delete my data").
pub fn vault_ids(store: Arc<dyn SecretStore>) -> Vec<String> {
    let keys = VaultKeys(store);
    Kept { store: &keys }.all_ids()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_reaches_only_plenipos_relay_and_page() {
        let b = built_for(None, None, None, false);
        assert_eq!(b.relay, RELAY_ADDRESS);
        assert_eq!(b.origin, "https://remote.getplenipo.com");
        assert_eq!(b.rp_id, "remote.getplenipo.com");
        assert!(!b.live, "coming soon until the relay is live");
        assert!(built_for(None, None, Some("1"), false).live);
        // A stand-in is never used by a copy not built for the tests.
        let b = built_for(
            Some("http://127.0.0.1:8769"),
            Some("http://localhost:8771"),
            None,
            false,
        );
        assert_eq!(b.relay, RELAY_ADDRESS);
        assert_eq!(b.origin, "https://remote.getplenipo.com");
        assert_eq!(b.rules.relay_test_port, None);
    }

    #[test]
    fn a_copy_built_for_the_tests_uses_stand_ins_on_this_computer() {
        let b = built_for(
            Some("http://127.0.0.1:8769"),
            Some("http://localhost:8771"),
            None,
            true,
        );
        assert_eq!(b.relay, "http://127.0.0.1:8769/plenipo/v1/pc");
        assert!(b.live);
        assert_eq!(b.origin, "http://localhost:8771");
        assert_eq!(b.rp_id, "localhost");
        assert!(b.rules.check(Purpose::PhoneAccess, &b.relay).is_ok());
        // Not this computer: not a stand-in.
        let b = built_for(
            Some("http://10.0.0.5:8769"),
            Some("http://evil.example"),
            None,
            true,
        );
        assert_eq!(b.relay, RELAY_ADDRESS);
        assert_eq!(b.origin, "https://remote.getplenipo.com");
    }

    #[test]
    fn approvals_lessons_and_stops_reach_the_phones() {
        assert_eq!(changed_by("approval.requested"), Some(Changed::Approvals));
        assert_eq!(changed_by("control.stopped"), Some(Changed::Control));
        assert_eq!(changed_by("lesson.added"), Some(Changed::Lessons));
        assert_eq!(changed_by("task.state_changed"), Some(Changed::Tasks));
        assert_eq!(changed_by("guard.request_refused"), None);
        assert_eq!(
            changed_by("remote.request"),
            None,
            "the phone's own requests do not echo"
        );
    }
}
