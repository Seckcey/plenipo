//! More than one organization (Phase 21, ADR-094): the list of organizations, each open
//! organization's own set of services, which window shows which organization, and the
//! [`Org`] command argument that gives a command its own window's organization.
//!
//! The first organization stays where it always was (Plenipo's data folder); every other one
//! has a folder of its own, `organizations/<its ID>/`, laid out the same way.

use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use plenipo_capabilities::control::ControlCenter;
use plenipo_capabilities::Broker;
use plenipo_guard::Guard;
use plenipo_ledger::Ledger;
use plenipo_liaison::Liaison;
use plenipo_router::Router;
use plenipo_runtime::agent::AgentRuntime;
use plenipo_runtime::Supervisor;
use plenipo_workforce::Workforce;
use serde::{Deserialize, Serialize};
use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};

use crate::guard_host::WatchSubscribers;
use crate::workspace_windows::{parse_popout, MAIN, ORG_PREFIX};

/// The first organization's ID: its folder is Plenipo's data folder itself.
pub const FIRST: &str = "first";
/// The list of organizations, in the data folder.
pub const LIST_FILE: &str = "organizations.json";
/// Where every other organization's folder is.
pub const FOLDER: &str = "organizations";
/// Told to every organization's window when the list changes (the page reads it again).
pub const ORGANIZATIONS_EVENT: &str = "plenipo://organizations";
/// The longest name an organization may have (the Workforce's rule for its name).
pub const MAX_NAME: usize = 80;

/// An organization in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrgEntry {
    pub id: String,
    /// A copy of the name its Ledger keeps, to show without opening it.
    pub name: String,
    pub created_at: u64,
    #[serde(default)]
    pub archived_at: Option<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ListFile {
    #[serde(default)]
    organizations: Vec<OrgEntry>,
    /// Which organization the first window shows.
    #[serde(default)]
    main_shows: Option<String>,
    /// Folders of organizations deleted for good that could not all be removed at once (a file
    /// still open): removed at the next start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pending_deletes: Vec<String>,
}

/// Whether `id` can name an organization: the first one's, or 32 lowercase hex digits.
pub fn is_org_id(id: &str) -> bool {
    id == FIRST
        || (id.len() == 32
            && id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
}

/// A new organization's ID.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// The organization's own folder (`data` itself for the first one).
pub fn folder_of(data: &Path, id: &str) -> PathBuf {
    if id == FIRST {
        data.to_path_buf()
    } else {
        data.join(FOLDER).join(id)
    }
}

/// The name its secrets are kept under in the Vault (ADR-094 §9): the first organization keeps
/// Plenipo's own; another adds its ID.
pub fn vault_name(identifier: &str, id: &str) -> String {
    if id == FIRST {
        identifier.to_owned()
    } else {
        format!("{identifier}.org-{id}")
    }
}

/// The label of an organization's own window (never the first window, `main`).
pub fn window_label(id: &str) -> String {
    format!("{ORG_PREFIX}{id}")
}

/// Where one organization lives, for the services built for it.
#[derive(Debug, Clone)]
pub struct OrgPlace {
    pub id: String,
    /// Its folder (`None`: nothing is kept, the tests).
    pub folder: Option<PathBuf>,
    /// Plenipo's data folder (the PC's things: the AI tools' settings folder).
    pub data: Option<PathBuf>,
    /// The name its secrets are kept under.
    pub vault: String,
}

impl OrgPlace {
    pub fn is_first(&self) -> bool {
        self.id == FIRST
    }
}

/// One open organization's full set of services (ADR-094 §3).
pub struct OrgStack {
    pub place: OrgPlace,
    pub ledger: Arc<Ledger>,
    pub supervisor: Supervisor,
    pub agents: AgentRuntime,
    pub liaison: Liaison,
    pub router: Router,
    pub guard: Guard,
    pub broker: Broker,
    pub workforce: Workforce,
    pub notices: Arc<crate::notices::Notices>,
    pub watchers: WatchSubscribers,
}

impl OrgStack {
    pub fn id(&self) -> &str {
        &self.place.id
    }

    /// Programs running for it now (its AI tools' turns among them).
    pub fn working(&self) -> usize {
        self.supervisor.active_count()
    }

    /// Whether it has work going: programs, the owner's terminals, or tasks not finished.
    pub fn busy(&self) -> bool {
        self.working() > 0
            || !self.broker.open_terminals().is_empty()
            || self.ledger.unfinished_tasks().is_ok_and(|t| !t.is_empty())
    }
}

/// Your organizations: the list, the open ones, and which window shows which.
pub struct Orgs {
    file: Option<PathBuf>,
    list: Mutex<ListFile>,
    stacks: RwLock<BTreeMap<String, Arc<OrgStack>>>,
    /// Archived organizations' services, stopped, kept to read them or save their workers before
    /// they are deleted for good.
    parked: Mutex<HashMap<String, Arc<OrgStack>>>,
    /// Window label → the organization it shows (`main` follows the list's choice).
    windows: Mutex<HashMap<String, String>>,
    /// Who uses the browser, the screen, and servers: one record for the whole PC.
    control: ControlCenter,
    /// AI tools' sign-in terminals (the first organization's broker runs them for every
    /// window): terminal ID → the window that shows it.
    ai_terminals: Mutex<HashMap<String, String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Orgs {
    /// Read the list in `data` (none: the tests, nothing kept). The first organization is
    /// always in it.
    pub fn load(data: Option<&Path>) -> Self {
        let file = data.map(|d| d.join(LIST_FILE));
        let mut list: ListFile = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        list.organizations.retain(|o| is_org_id(&o.id));
        let mut seen = std::collections::HashSet::new();
        list.organizations.retain(|o| seen.insert(o.id.clone()));
        if !list.organizations.iter().any(|o| o.id == FIRST) {
            list.organizations.insert(
                0,
                OrgEntry {
                    id: FIRST.into(),
                    name: String::new(),
                    created_at: 0,
                    archived_at: None,
                },
            );
        }
        // The first organization is never archived (ADR-094, Limits).
        for o in &mut list.organizations {
            if o.id == FIRST {
                o.archived_at = None;
            }
        }
        // What an earlier deletion left behind goes now, before anything opens it.
        if let Some(data) = data {
            let before = list.pending_deletes.len();
            list.pending_deletes
                .retain(|id| is_org_id(id) && id != FIRST && !remove_folder(&folder_of(data, id)));
            if list.pending_deletes.len() != before {
                if let Some(file) = &file {
                    let _ =
                        std::fs::write(file, serde_json::to_vec_pretty(&list).unwrap_or_default());
                }
            }
        }
        Self {
            file,
            list: Mutex::new(list),
            stacks: RwLock::new(BTreeMap::new()),
            parked: Mutex::new(HashMap::new()),
            windows: Mutex::new(HashMap::new()),
            control: ControlCenter::default(),
            ai_terminals: Mutex::new(HashMap::new()),
        }
    }

    /// The record of who uses the browser, the screen, and servers, shared by every
    /// organization's broker.
    pub fn control(&self) -> ControlCenter {
        self.control.clone()
    }

    fn save(&self, list: &ListFile) {
        let Some(file) = &self.file else { return };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let temp = file.with_extension("json.tmp");
            std::fs::write(&temp, serde_json::to_vec_pretty(list)?)?;
            std::fs::rename(&temp, file)
        };
        if let Err(e) = write() {
            log::warn!("could not keep the list of organizations: {e}");
        }
    }

    /// Every organization in the list, the first one first.
    pub fn entries(&self) -> Vec<OrgEntry> {
        lock(&self.list).organizations.clone()
    }

    pub fn entry(&self, id: &str) -> Option<OrgEntry> {
        lock(&self.list)
            .organizations
            .iter()
            .find(|o| o.id == id)
            .cloned()
    }

    /// Add a new organization to the list.
    pub fn add(&self, entry: OrgEntry) {
        let mut list = lock(&self.list);
        list.organizations.retain(|o| o.id != entry.id);
        list.organizations.push(entry);
        self.save(&list);
    }

    /// Keep an organization's name (a copy of what its Ledger says).
    pub fn set_name(&self, id: &str, name: &str) {
        let mut list = lock(&self.list);
        let Some(o) = list.organizations.iter_mut().find(|o| o.id == id) else {
            return;
        };
        if o.name != name {
            name.clone_into(&mut o.name);
            self.save(&list);
        }
    }

    /// Archive (`Some`) or bring back (`None`) an organization.
    pub fn set_archived(&self, id: &str, at: Option<u64>) {
        let mut list = lock(&self.list);
        if let Some(o) = list
            .organizations
            .iter_mut()
            .find(|o| o.id == id && o.id != FIRST)
        {
            o.archived_at = at;
            if at.is_some() && list.main_shows.as_deref() == Some(id) {
                list.main_shows = None;
            }
            self.save(&list);
        }
    }

    /// Take a deleted organization out of the list; `left`: its folder could not all be removed
    /// now, so it goes at the next start.
    pub fn remove(&self, id: &str, left: bool) {
        if id == FIRST {
            return;
        }
        let mut list = lock(&self.list);
        list.organizations.retain(|o| o.id != id);
        if left && !list.pending_deletes.iter().any(|p| p == id) {
            list.pending_deletes.push(id.to_owned());
        }
        if list.main_shows.as_deref() == Some(id) {
            list.main_shows = None;
        }
        self.save(&list);
    }

    /// The organization the first window shows: the one chosen last, while it is open.
    pub fn main_shows(&self) -> String {
        let list = lock(&self.list);
        list.main_shows
            .as_deref()
            .filter(|id| {
                list.organizations
                    .iter()
                    .any(|o| o.id == *id && o.archived_at.is_none())
            })
            .unwrap_or(FIRST)
            .to_owned()
    }

    fn set_main_shows(&self, id: &str) {
        let mut list = lock(&self.list);
        let next = (id != FIRST).then(|| id.to_owned());
        if list.main_shows != next {
            list.main_shows = next;
            self.save(&list);
        }
    }

    // ---- The open organizations --------------------------------------------------------------

    pub fn insert(&self, stack: Arc<OrgStack>) {
        self.stacks
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(stack.id().to_owned(), stack);
    }

    /// Close an organization (archived or deleted): its services are no longer found.
    pub fn take(&self, id: &str) -> Option<Arc<OrgStack>> {
        if id == FIRST {
            return None;
        }
        self.stacks
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id)
    }

    /// Keep an archived organization's stopped services.
    pub fn park(&self, stack: Arc<OrgStack>) {
        lock(&self.parked).insert(stack.id().to_owned(), stack);
    }

    /// An archived organization's stopped services, if kept.
    pub fn parked(&self, id: &str) -> Option<Arc<OrgStack>> {
        lock(&self.parked).get(id).cloned()
    }

    /// Stop keeping an archived organization's services (it opens again, or goes for good).
    pub fn unpark(&self, id: &str) -> Option<Arc<OrgStack>> {
        lock(&self.parked).remove(id)
    }

    pub fn stack(&self, id: &str) -> Option<Arc<OrgStack>> {
        self.stacks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    /// Every open organization, the first one first.
    pub fn stacks(&self) -> Vec<Arc<OrgStack>> {
        let stacks = self
            .stacks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut all: Vec<Arc<OrgStack>> = stacks.values().cloned().collect();
        all.sort_by_key(|s| s.id() != FIRST);
        all
    }

    /// The first organization (it is always open once Plenipo has started).
    pub fn first(&self) -> Option<Arc<OrgStack>> {
        self.stack(FIRST)
    }

    // ---- Windows -----------------------------------------------------------------------------

    /// The organization a window shows: a pop-out's is its window's; `main`'s is the one chosen
    /// last; `org-…` shows the one it was opened for. `None`: a window that shows none.
    pub fn org_of_window(&self, label: &str) -> Option<String> {
        let label = parse_popout(label).map_or(label, |(_, parent)| parent);
        if let Some(id) = lock(&self.windows).get(label) {
            return Some(id.clone());
        }
        (label == MAIN).then(|| self.main_shows())
    }

    /// The open organization a window shows.
    pub fn stack_for_window(&self, label: &str) -> Option<Arc<OrgStack>> {
        self.stack(&self.org_of_window(label)?)
    }

    /// The window showing organization `id`, if one does.
    pub fn window_of(&self, id: &str) -> Option<String> {
        let windows = lock(&self.windows);
        if let Some((label, _)) = windows.iter().find(|(_, org)| org.as_str() == id) {
            return Some(label.clone());
        }
        drop(windows);
        (self.org_of_window(MAIN).as_deref() == Some(id)).then(|| MAIN.to_owned())
    }

    /// Window `label` shows organization `id` from now on.
    pub fn bind(&self, label: &str, id: &str) {
        lock(&self.windows).insert(label.to_owned(), id.to_owned());
        if label == MAIN {
            self.set_main_shows(id);
        }
    }

    /// An AI tool's sign-in terminal, shown in window `label`.
    pub fn own_ai_terminal(&self, terminal: &str, label: &str) {
        let mut all = lock(&self.ai_terminals);
        // Kept small: a terminal that ended without Plenipo hearing of it is forgotten first.
        if all.len() > 64 {
            all.clear();
        }
        all.insert(terminal.to_owned(), label.to_owned());
    }

    /// The window showing AI tool sign-in terminal `terminal`, if it is one.
    pub fn ai_terminal_window(&self, terminal: &str) -> Option<String> {
        lock(&self.ai_terminals).get(terminal).cloned()
    }

    pub fn forget_ai_terminal(&self, terminal: &str) {
        lock(&self.ai_terminals).remove(terminal);
    }

    /// The AI tool sign-in terminals window `label` shows.
    pub fn ai_terminals_of(&self, label: &str) -> Vec<String> {
        lock(&self.ai_terminals)
            .iter()
            .filter(|(_, l)| l.as_str() == label)
            .map(|(t, _)| t.clone())
            .collect()
    }

    /// Window `label` closed.
    pub fn unbind(&self, label: &str) {
        if label != MAIN {
            lock(&self.windows).remove(label);
        }
    }
}

/// Remove an organization's folder; whether it is gone.
pub fn remove_folder(folder: &Path) -> bool {
    match std::fs::remove_dir_all(folder) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => {
            log::warn!("an organization's folder could not be removed yet: {e}");
            false
        }
    }
}

/// The organization's name, as its Ledger keeps it.
pub fn name_in(ledger: &Ledger) -> String {
    ledger
        .setting("organization")
        .ok()
        .flatten()
        .and_then(|v| v["name"].as_str().map(str::to_owned))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Organization".to_owned())
}

/// The log files hide every open organization's secrets (each broker's filter knows its own).
pub fn filter_logs(orgs: &Orgs) {
    let Some(logs) = crate::logs::installed() else {
        return;
    };
    let filters: Vec<crate::logs::Filter> = orgs
        .stacks()
        .iter()
        .map(|s| s.broker.text_filter())
        .collect();
    logs.set_filter(Arc::new(move |line: &str| {
        filters.iter().fold(line.to_owned(), |text, f| f(&text))
    }));
}

// ---- Telling an organization's window ------------------------------------------------------

/// Send `payload` to the window showing organization `org` only (ADR-094 §12). Before the list
/// is in place (Plenipo starting), the first organization's is the first window.
pub fn emit_to_org<R: Runtime, S: Serialize + Clone>(
    app: &AppHandle<R>,
    org: &str,
    event: &str,
    payload: &S,
) {
    let label = match app.try_state::<Arc<Orgs>>() {
        Some(orgs) => orgs.window_of(org),
        None => (org == FIRST).then(|| MAIN.to_owned()),
    };
    let Some(label) = label else { return };
    if let Err(e) = app.emit_to(label.as_str(), event, payload) {
        log::warn!("failed to emit {event}: {e}");
    }
}

/// Tell every organization's window that the list changed.
pub fn list_changed<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.emit(ORGANIZATIONS_EVENT, ()) {
        log::warn!("failed to emit {ORGANIZATIONS_EVENT}: {e}");
    }
}

// ---- A command's own organization ----------------------------------------------------------

/// A service of the organization the calling window shows (ADR-094 §12): found by the window
/// that called, never by anything the page sends. Used as a command argument in place of
/// `State` for each organization's own services.
pub struct Org<'r, T: 'static> {
    value: T,
    _state: PhantomData<&'r ()>,
}

impl<T> Org<'_, T> {
    pub fn inner(&self) -> &T {
        &self.value
    }
}

impl<T> std::ops::Deref for Org<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

/// A service each organization has its own of.
pub trait OrgService: Sized {
    fn of(stack: &OrgStack) -> Self;
}

macro_rules! org_service {
    ($($ty:ty => $field:ident),* $(,)?) => {
        $(impl OrgService for $ty {
            fn of(stack: &OrgStack) -> Self {
                stack.$field.clone()
            }
        })*
    };
}

org_service! {
    Arc<Ledger> => ledger,
    Supervisor => supervisor,
    AgentRuntime => agents,
    Liaison => liaison,
    Router => router,
    Guard => guard,
    Broker => broker,
    Workforce => workforce,
    Arc<crate::notices::Notices> => notices,
    WatchSubscribers => watchers,
}

impl<'de, 'r, R: Runtime, T: OrgService + 'static> CommandArg<'de, R> for Org<'r, T> {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let webview = command.message.webview_ref();
        let stack = webview
            .try_state::<Arc<Orgs>>()
            .and_then(|orgs| orgs.stack_for_window(webview.label()))
            .ok_or_else(|| InvokeError::from("This window shows no organization."))?;
        Ok(Org {
            value: T::of(&stack),
            _state: PhantomData,
        })
    }
}

/// The open organization the window `label` shows.
pub fn stack_of<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<Arc<OrgStack>> {
    app.try_state::<Arc<Orgs>>()?.stack_for_window(label)
}

/// Every open organization.
pub fn all_stacks<R: Runtime>(app: &AppHandle<R>) -> Vec<Arc<OrgStack>> {
    app.try_state::<Arc<Orgs>>()
        .map(|orgs| orgs.stacks())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_folders_and_vault_names() {
        assert!(is_org_id(FIRST));
        let id = new_id();
        assert!(is_org_id(&id), "{id}");
        assert!(!is_org_id("../x") && !is_org_id("") && !is_org_id(&id.to_uppercase()));
        let data = Path::new("/data");
        assert_eq!(folder_of(data, FIRST), PathBuf::from("/data"));
        assert_eq!(folder_of(data, &id), data.join("organizations").join(&id));
        assert_eq!(
            vault_name("com.eightwest.plenipo", FIRST),
            "com.eightwest.plenipo"
        );
        assert_eq!(
            vault_name("com.eightwest.plenipo", &id),
            format!("com.eightwest.plenipo.org-{id}")
        );
        assert!(crate::workspace_windows::is_org_window(&window_label(&id)));
    }

    #[test]
    fn the_list_keeps_the_first_organization_and_the_first_windows_choice() {
        let dir = tempfile::tempdir().unwrap();
        let orgs = Orgs::load(Some(dir.path()));
        assert_eq!(orgs.entries().len(), 1);
        assert_eq!(orgs.main_shows(), FIRST);
        let id = new_id();
        orgs.add(OrgEntry {
            id: id.clone(),
            name: "Client Co".into(),
            created_at: 1,
            archived_at: None,
        });
        orgs.bind(MAIN, &id);
        assert_eq!(orgs.org_of_window(MAIN).as_deref(), Some(id.as_str()));
        assert_eq!(
            orgs.org_of_window("popout-terminal--main--3").as_deref(),
            Some(id.as_str())
        );
        assert_eq!(orgs.window_of(&id).as_deref(), Some(MAIN));
        // Kept for the next start.
        let again = Orgs::load(Some(dir.path()));
        assert_eq!(again.main_shows(), id);
        assert_eq!(again.entry(&id).unwrap().name, "Client Co");
        // Archived: the first window shows the first organization again.
        again.set_archived(&id, Some(5));
        assert_eq!(again.main_shows(), FIRST);
        // The first one is never archived or removed.
        again.set_archived(FIRST, Some(5));
        again.remove(FIRST, false);
        assert!(again.entry(FIRST).unwrap().archived_at.is_none());
        // Another organization's window, and a window that shows none.
        let label = window_label(&id);
        again.bind(&label, &id);
        assert_eq!(again.org_of_window(&label).as_deref(), Some(id.as_str()));
        assert_eq!(again.org_of_window("control-indicator"), None);
        again.unbind(&label);
        assert_eq!(again.org_of_window(&label), None);
    }

    #[test]
    fn a_damaged_list_starts_again_with_the_first_organization() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LIST_FILE), b"{ not json").unwrap();
        let orgs = Orgs::load(Some(dir.path()));
        assert_eq!(orgs.entries().len(), 1);
        std::fs::write(
            dir.path().join(LIST_FILE),
            br#"{"organizations":[{"id":"../../x","name":"Bad","createdAt":1}]}"#,
        )
        .unwrap();
        let orgs = Orgs::load(Some(dir.path()));
        assert!(orgs.entries().iter().all(|o| o.id != "../../x"));
    }
}
