//! Connections (Phase 20; ADR-062 to ADR-065, ADR-070, ADR-071): the business's own accounts
//! that Plenipo signs in to for the owner — Microsoft 365, Slack, Google, HubSpot, Stripe, and
//! the website (WordPress and WooCommerce) — and the calls workers make through them.
//!
//! - **Signing in** happens in the owner's own browser ([`signin`]): Plenipo never sees a
//!   password. HubSpot, Stripe, and the website connect with a key the owner types into the card
//!   instead ([`keyed`]), checked once and kept only in the Vault. The long-lived sign-in is kept only in the Vault ([`vault_id`]); the short-lived
//!   access token only in memory. Neither is ever recorded, shown, or given to a worker or a
//!   program. A Google app's secret is kept only in the Vault too ([`app_secret_id`]).
//! - **Calls** go through Plenipo's own client ([`http`]), each hop checked by Guard's gate for
//!   Plenipo's own requests; the access token is added there, for the service's API only.
//! - **Guard decides** every worker call before it runs (the broker asks it; see
//!   `broker/connecting.rs`).

pub mod google;
pub(crate) mod http;
pub mod hubspot;
pub mod keyed;
pub mod microsoft365;
pub mod signin;
pub mod slack;
pub mod stripe;
pub(crate) mod text;
pub mod wordpress;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use plenipo_guard::{
    Account, AccountKind, Connection, ConnectionState, Guard, Part, PartLevel, Service, ToolKind,
};
use plenipo_runtime::Supervisor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use ts_rs::TS;

use self::http::{Body, Http, HttpError, Reply};
use self::signin::{Callback, Listener, Pkce};
use crate::tools::ToolDef;
use crate::vault::{self, SecretStore};

/// The Vault ID of a connection's long-lived sign-in.
pub fn vault_id(connection_id: &str) -> String {
    format!("connection-{connection_id}-token")
}

/// The Vault ID of the secret of the owner's own app for a connection (Google, ADR-070 §4).
pub fn app_secret_id(connection_id: &str) -> String {
    format!("connection-{connection_id}-app-secret")
}

/// Every Vault ID connections may keep for these connection IDs (a sign-in or key, an app's
/// secret, and the website's WooCommerce key each), and each service's own, in case a value was
/// kept before its connection was.
pub fn vault_ids_of<'a>(connection_ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let services = Service::ALL.iter().map(|s| s.id());
    for c in connection_ids.chain(services) {
        for id in [vault_id(c), app_secret_id(c), keyed::store_key_id(c)] {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// Refresh the access token when it has less than this left.
const REFRESH_EARLY: Duration = Duration::from_secs(5 * 60);
/// How long a sign-in that does not expire (a Slack app without token rotation) is kept in
/// memory before it is read again from the Vault.
const LASTING: Duration = Duration::from_secs(24 * 60 * 60);
/// The most a sign-in or token answer may be.
const MAX_TOKEN_ANSWER: usize = 64 * 1024;
/// The most a service's answer may be (a file's download has its own limit).
pub(crate) const MAX_ANSWER: usize = 8 * 1024 * 1024;
/// The longest Disconnect waits for the service to cancel a sign-in.
const CANCEL_WAIT: Duration = Duration::from_secs(15);

/// How this copy of Plenipo connects: the app IDs it was built with, and, in a copy built for
/// the end-to-end tests only, the stand-in for the services.
#[derive(Debug, Clone, Default)]
pub struct ConnectionsConfig {
    /// 8 West's Microsoft app ID ("Application (client) ID"). Public, not a secret.
    pub microsoft_app_id: Option<String>,
    /// 8 West's Slack app's client ID (ADR-070 §3). Public, not a secret.
    pub slack_client_id: Option<String>,
    /// The ports Slack's sign-in comes back to (`None`: [`slack::REDIRECT_PORTS`]); the Rust
    /// tests use `Some(vec![0])` (any port), so sign-ins in parallel tests never meet.
    pub slack_ports: Option<Vec<u16>>,
    /// `http://127.0.0.1:<port>` in copies built for the tests; never in a release.
    pub stand_in: Option<String>,
}

/// Opens a sign-in address in the owner's own browser.
pub trait Opener: Send + Sync + 'static {
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// The owner's default browser, through Windows' own handler for web addresses, started through
/// the supervisor with no shell. On a Mac and Linux, the desktop's own opener, for the owner
/// (`open`, `xdg-open`; Phase 23): with the owner's own session, so the browser can appear, and
/// never as a worker's program, so it stays open.
pub struct SystemOpener {
    #[cfg_attr(unix, allow(dead_code))]
    supervisor: Supervisor,
}

impl SystemOpener {
    pub fn new(supervisor: Supervisor) -> Self {
        Self { supervisor }
    }
}

impl Opener for SystemOpener {
    #[cfg(unix)]
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(crate::programs::open_for_owner(vec![address.into()]))
    }

    #[cfg(not(unix))]
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let supervisor = self.supervisor.clone();
        Box::pin(async move {
            let root = std::env::var_os("SystemRoot")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "C:\\Windows".into());
            let executable = root.join("System32").join("rundll32.exe");
            let args = vec!["url.dll,FileProtocolHandler".to_owned(), address];
            let dir = std::env::temp_dir();
            crate::programs::run(
                &supervisor,
                crate::programs::Run {
                    label: "Open the sign-in page in your browser".into(),
                    executable,
                    args,
                    working_dir: &dir,
                    env: Vec::new(),
                    stdin: None,
                    timeout: Duration::from_secs(30),
                },
                |_| {},
            )
            .await
            .map(|_| ())
        })
    }
}

/// A stand-in browser for copies built for the tests: it follows the sign-in address the way a
/// browser would, only ever to this computer.
pub struct FollowOpener;

impl Opener for FollowOpener {
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async move {
            let local = reqwest::redirect::Policy::custom(|attempt| {
                let here = matches!(
                    attempt.url().host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]")
                );
                if attempt.previous().len() > 10 || !here {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            });
            let client = reqwest::Client::builder()
                .redirect(local)
                .build()
                .map_err(|e| e.to_string())?;
            client
                .get(&address)
                .send()
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
    }
}

// ---- The tools, and what every service's calls share ---------------------------------------

/// A connection tool: its definition, and the part it belongs to.
pub struct Tool {
    pub def: ToolDef,
    pub part: Part,
}

/// Every connection tool of `service`, in its fixed table.
pub fn tools_of(service: Service) -> &'static [Tool] {
    match service {
        Service::Microsoft365 => &microsoft365::TOOLS,
        Service::Slack => &slack::TOOLS,
        Service::Google => &google::TOOLS,
        Service::Hubspot => &hubspot::TOOLS,
        Service::Stripe => &stripe::TOOLS,
        Service::Wordpress => &wordpress::TOOLS,
    }
}

/// The connection tool called `name`, and its service.
pub fn tool(name: &str) -> Option<(Service, &'static Tool)> {
    Service::ALL.into_iter().find_map(|s| {
        tools_of(s)
            .iter()
            .find(|t| t.def.name == name)
            .map(|t| (s, t))
    })
}

/// The start of every tool name of `service`: "m365_", "slack_", "google_", "hubspot_",
/// "stripe_", "wp_".
pub fn tool_prefix(service: Service) -> &'static str {
    match service {
        Service::Microsoft365 => "m365_",
        Service::Slack => "slack_",
        Service::Google => "google_",
        Service::Hubspot => "hubspot_",
        Service::Stripe => "stripe_",
        Service::Wordpress => "wp_",
    }
}

/// A call, worked out before Guard decides: its part and kind, words for the owner, and for a
/// send, everyone it reaches (as the service has them, never as the worker said).
#[derive(Debug, Clone)]
pub struct Plan<C> {
    pub part: Part,
    pub kind: ToolKind,
    /// "send the email \"Invoice\" to 2 people".
    pub summary: String,
    /// What the approval card and the record show (for a send: its recipients, subject, and the
    /// worker's own words). A part made with [`card_only`] is shown on the card, not recorded.
    pub detail: String,
    pub recipients: Vec<String>,
    pub call: C,
    /// A send as it was approved (its recipients and subject), checked again just before it is
    /// sent.
    pub approved_as: Option<(Vec<String>, String)>,
}

/// How a keyed connection's error starts when a change may have been made although no answer
/// came back: the worker is told to check before asking again, never "Not done" (ADR-071 §6.5).
pub(crate) const MAYBE_DONE: &str = "Maybe done: ";

/// Mark the start and the end of a part of a card's detail that is [`card_only`].
const CARD_ONLY: [char; 2] = ['\u{1e}', '\u{1f}'];

/// The service's own words on an approval card (a payment's description, an invoice's lines, a
/// post's words): the owner sees them to decide, and `capability.used` keeps "(not kept)" in
/// their place, never a copy of a record (ADR-062 §7).
pub(crate) fn card_only(text: &str) -> String {
    let text: String = text.chars().filter(|c| !CARD_ONLY.contains(c)).collect();
    format!("{}{text}{}", CARD_ONLY[0], CARD_ONLY[1])
}

/// A detail as its card shows it.
pub(crate) fn as_shown(detail: &str) -> String {
    detail.chars().filter(|c| !CARD_ONLY.contains(c)).collect()
}

/// A detail as the record keeps it: each [`card_only`] part becomes "(not kept)".
pub(crate) fn as_kept(detail: &str) -> String {
    let mut out = String::with_capacity(detail.len());
    let mut inside = false;
    for c in detail.chars() {
        match c {
            c if c == CARD_ONLY[0] && !inside => {
                inside = true;
                out.push_str("(not kept)");
            }
            c if c == CARD_ONLY[1] => inside = false,
            c if inside || c == CARD_ONLY[0] => {}
            c => out.push(c),
        }
    }
    out
}

/// A call carried out.
pub struct Done {
    /// For the worker: fenced wherever it holds other people's words.
    pub text: String,
    /// Plenipo's own short summary, kept in the record.
    pub summary: String,
    /// IDs, links, counts, and (for what a worker writes or sends) recipients and the subject.
    pub record: Value,
    /// What the worker read ("email", "chat messages"), for the approval cards that follow.
    pub read: Option<&'static str>,
}

/// The permissions `part` of `service` needs at `level`, in the service's words.
pub fn permissions(service: Service, part: Part, level: PartLevel) -> &'static [&'static str] {
    match service {
        Service::Microsoft365 => microsoft365::permissions(part, level),
        Service::Slack => slack::permissions(part, level),
        Service::Google => google::permissions(part, level),
        _ => &[],
    }
}

/// A part's level as far as the service allowed it at the last sign-in: a part turned on, or up
/// to Full access, since then works at what the service granted until the owner reconnects. A
/// key's permissions were chosen in the service, so a keyed connection's part works as set (the
/// service refuses what the key may not do, ADR-071 §6.8).
pub fn allowed_level(conn: &Connection, part: Part) -> PartLevel {
    if conn.service.uses_key() {
        return conn.part(part);
    }
    let granted = |level: PartLevel| {
        let need = permissions(conn.service, part, level);
        !need.is_empty()
            && need
                .iter()
                .all(|p| conn.granted.iter().any(|g| g.eq_ignore_ascii_case(p)))
    };
    match conn.part(part) {
        PartLevel::Off => PartLevel::Off,
        PartLevel::FullAccess if granted(PartLevel::FullAccess) => PartLevel::FullAccess,
        _ if granted(PartLevel::ReadOnly) || granted(PartLevel::FullAccess) => PartLevel::ReadOnly,
        _ => PartLevel::Off,
    }
}

/// The parts that need a new sign-in to work as set: turned on, or up to Full access, since the
/// last one (a part turned down works with what the service already allowed).
pub fn parts_to_reconnect(conn: &Connection) -> Vec<Part> {
    conn.service
        .parts()
        .iter()
        .copied()
        .filter(|p| conn.part(*p) != PartLevel::Off)
        .filter(|p| allowed_level(conn, *p) < conn.part(*p))
        .collect()
}

fn part_words(service: Service, part: Part) -> (&'static str, &'static str) {
    match service {
        Service::Microsoft365 => microsoft365::part_words(part),
        Service::Slack => slack::part_words(part),
        Service::Google => google::part_words(part),
        Service::Hubspot => hubspot::part_words(part),
        Service::Stripe => stripe::part_words(part),
        Service::Wordpress => wordpress::part_words(part),
    }
}

fn permission_words(service: Service, name: &str) -> String {
    match service {
        Service::Microsoft365 => microsoft365::permission_words(name).into(),
        Service::Slack => slack::permission_words(name).into(),
        Service::Google => google::permission_words(name).into(),
        Service::Hubspot => hubspot::permission_words(name).into(),
        Service::Stripe => stripe::permission_words(name).into(),
        Service::Wordpress => wordpress::permission_words(name),
    }
}

/// The permissions a keyed connection's key needs for its parts as set, in the service's words,
/// for the card (the owner chooses them when making the key; ADR-071 §6.2–§6.3).
pub fn key_permissions(conn: &Connection) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in conn.service.parts() {
        let level = conn.part(*part);
        if level == PartLevel::Off {
            continue;
        }
        let full = level == PartLevel::FullAccess;
        let need = match conn.service {
            Service::Hubspot => hubspot::key_permissions(*part, full),
            Service::Stripe => stripe::key_permissions(*part, full),
            _ => Vec::new(),
        };
        for p in need {
            if !out.iter().any(|o| o == p) {
                out.push(p.to_owned());
            }
        }
    }
    out
}

// ---- What Settings shows -----------------------------------------------------------------------

/// Settings → Connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionsPage {
    pub services: Vec<ServiceCard>,
    /// Roles and agents, for **Who may use it**.
    pub people: Vec<PersonOption>,
    /// Whether the switch "Sending forms and messages (without asking)" is on.
    pub send_switch_on: bool,
    /// The Vault (where sign-ins are kept) can be used on this computer.
    pub vault_available: bool,
    /// "Windows Credential Manager".
    pub vault_label: String,
    /// The app description (Slack's "manifest") to paste when a workspace makes its own Slack
    /// app for Plenipo (ADR-070 §3). Holds no secret.
    pub slack_manifest: String,
    /// The owner's add-on programs (ADR-066): never a secret's value.
    pub add_ons: Vec<plenipo_guard::AddOn>,
    /// The stored secrets an add-on may be given, by name (only ones with a variable name).
    pub secret_names: Vec<String>,
}

/// A service and its connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceCard {
    pub service: Service,
    pub label: String,
    /// Built into this copy (the rest come in a later update).
    pub built: bool,
    /// The owner may add more than one (Slack's workspaces).
    pub many: bool,
    pub connections: Vec<ConnectionCard>,
}

/// One connection's card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionCard {
    pub connection: Connection,
    /// This copy has an app to sign in with (8 West's, or the owner's own).
    pub has_app: bool,
    /// This copy has 8 West's app for this service built in.
    pub built_in_app: bool,
    /// A sign-in waits in the owner's browser.
    pub signing_in: bool,
    /// When the service said an admin must approve Plenipo first: the link to send them.
    #[ts(optional)]
    pub admin_link: Option<String>,
    /// What went wrong with the last sign-in, or with cancelling one, in plain words.
    #[ts(optional)]
    pub problem: Option<String>,
    pub parts: Vec<PartCard>,
    /// Parts turned on, or raised to Full access, since the last sign-in: Reconnect to allow
    /// them.
    pub reconnect_for: Vec<String>,
    /// What the service granted, in its words and in plain words.
    pub granted: Vec<PermissionWords>,
    /// A keyed connection (HubSpot, Stripe, the website): it connects with a key typed into the
    /// card, not a sign-in in the browser.
    pub uses_key: bool,
    /// For a keyed connection: the permissions its key needs for the parts as set, in the
    /// service's words (the owner chooses them when making the key).
    pub key_needs: Vec<String>,
    /// For the website: a WooCommerce key is kept.
    pub store_key_kept: bool,
}

/// A part on a connection's card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartCard {
    pub part: Part,
    pub label: String,
    pub level: PartLevel,
    /// The signed-in account has it (personal Microsoft accounts have no Teams or SharePoint).
    pub available: bool,
    /// It has a Full access level (Slack's Search only reads).
    pub full_access: bool,
    /// What workers can read, and change, in plain words.
    pub reads: String,
    pub changes: String,
    /// It always needs the organization's admin.
    pub needs_admin: bool,
}

/// A permission in the service's words, and in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PermissionWords {
    pub name: String,
    pub words: String,
}

/// A role or an agent the owner can add to **Who may use it**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PersonOption {
    /// `role` or `agent`.
    pub kind: String,
    pub id: String,
    /// "Documentation Writer", or an agent's title.
    pub name: String,
    /// For an agent: its role.
    #[ts(optional)]
    pub role: Option<String>,
    /// An agent that is archived: shown by name on a list, never offered to add.
    pub archived: bool,
}

/// The owner's own app for a connection, as typed into its card (ADR-070 §4): Slack's client ID,
/// or Google's client ID and secret. The secret goes straight to the Vault.
#[derive(Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AppInput {
    pub client_id: String,
    /// Google only; never kept anywhere but the Vault, never shown again.
    #[ts(optional)]
    pub secret: Option<String>,
}

impl std::fmt::Debug for AppInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppInput")
            .field("client_id", &self.client_id)
            .field("secret", &self.secret.as_ref().map(|_| "(hidden)"))
            .finish()
    }
}

/// The longest app secret Plenipo keeps.
pub const MAX_APP_SECRET: usize = 200;

// ---- The service ---------------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// Connection ID → its access token and when it expires (memory only).
    tokens: HashMap<String, (String, Instant)>,
    /// Connection ID → the sign-in waiting in the owner's browser: its turn, and how to stop it.
    waiting: HashMap<String, (u64, oneshot::Sender<()>)>,
    /// Connection ID → the link for the organization's admin.
    admin: HashMap<String, String>,
    /// Connection ID → what went wrong with the last sign-in, or with cancelling one.
    problem: HashMap<String, String>,
    /// Connection ID → the turn of its latest sign-in, Cancel, or Disconnect. A sign-in or a
    /// renewal that finishes after a newer one of these keeps nothing.
    turns: HashMap<String, u64>,
    next_turn: u64,
    /// Keys the owner typed into a card this session, kept or not (memory only): hidden in any
    /// text, so a key refused by its service is never written anywhere either.
    typed: Vec<String>,
}

/// What the sign-ins are doing, read before the saved connections (see [`Connections::page`]).
struct SignIns {
    waiting: HashSet<String>,
    admin: HashMap<String, String>,
    problem: HashMap<String, String>,
}

impl State {
    /// A new turn for `id`: whatever was under way for it before keeps nothing.
    fn next(&mut self, id: &str) -> u64 {
        self.next_turn += 1;
        self.turns.insert(id.to_owned(), self.next_turn);
        self.next_turn
    }

    fn turn(&self, id: &str) -> u64 {
        self.turns.get(id).copied().unwrap_or(0)
    }
}

/// The connections service: sign-ins, fresh access tokens, and the calls.
pub struct Connections {
    http: Http,
    config: ConnectionsConfig,
    store: Arc<dyn SecretStore>,
    opener: RwLock<Arc<dyn Opener>>,
    state: Mutex<State>,
    /// One renewal at a time.
    refreshing: tokio::sync::Mutex<()>,
    /// One change to a kept sign-in at a time — keeping, erasing, and recording it, after a
    /// sign-in, a renewal, or Disconnect. Never held across a wait.
    commit: Mutex<()>,
    /// Called when a sign-in or access token changes (the broker hides them again).
    changed: RwLock<Option<Arc<dyn Fn() + Send + Sync>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// A connection's name, from its ID.
fn name_of(id: &str) -> &'static str {
    plenipo_guard::connections::service_of(id).map_or("That connection", Service::label)
}

/// How a sign-in in the owner's browser ended.
enum Signed {
    /// Connected.
    In,
    /// Cancelled, ten minutes passed, or a newer sign-in, Cancel, or Disconnect came meanwhile:
    /// nothing kept.
    Stopped,
}

/// What one sign-in started with.
struct SignIn<'a> {
    id: &'a str,
    turn: u64,
    kind: AccountKind,
    app_id: &'a str,
    /// A Google app's secret (from the Vault), for the token request only.
    secret: Option<&'a str>,
    /// Microsoft's authority (`organizations`, `consumers`, or an organization).
    authority: &'a str,
    redirect: &'a str,
    /// The permissions asked for on the sign-in page, as Microsoft's token request repeats them.
    scopes: &'a str,
}

/// What a traded code gave.
struct Traded {
    /// The long-lived sign-in, for the Vault.
    long_lived: String,
    /// The access token, and how long it lasts.
    access: Option<(String, Duration)>,
    kind: Option<AccountKind>,
    account: Account,
    granted: Vec<String>,
}

impl Connections {
    pub fn new(
        guard: Guard,
        store: Arc<dyn SecretStore>,
        config: ConnectionsConfig,
        opener: Arc<dyn Opener>,
    ) -> Self {
        Self {
            http: Http::new(guard, config.stand_in.clone()),
            config,
            store,
            opener: RwLock::new(opener),
            state: Mutex::new(State::default()),
            refreshing: tokio::sync::Mutex::new(()),
            commit: Mutex::new(()),
            changed: RwLock::new(None),
        }
    }

    fn guard(&self) -> &Guard {
        self.http.guard()
    }

    pub fn set_opener(&self, opener: Arc<dyn Opener>) {
        *self.opener.write().unwrap_or_else(|p| p.into_inner()) = opener;
    }

    pub(crate) fn set_changed(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        *self.changed.write().unwrap_or_else(|p| p.into_inner()) = Some(hook);
    }

    fn changed(&self) {
        let hook = self
            .changed
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    /// 8 West's app for `service`, as built into this copy (none for Google: the owner makes
    /// their own, ADR-070 §4).
    pub fn built_in_app(&self, service: Service) -> Option<String> {
        match service {
            Service::Microsoft365 => self
                .config
                .microsoft_app_id
                .clone()
                .filter(|id| plenipo_guard::connections::is_guid(id.trim())),
            Service::Slack => self
                .config
                .slack_client_id
                .as_deref()
                .map(str::trim)
                .filter(|id| plenipo_guard::connections::is_slack_client_id(id))
                .map(str::to_owned),
            _ => None,
        }
    }

    /// The app a connection signs in with: the owner's own, or 8 West's. A Google app counts
    /// only once its secret is kept.
    pub fn app_id(&self, conn: &Connection) -> Option<String> {
        if !conn.service.built() {
            return None;
        }
        match &conn.own_app {
            Some(app) if conn.service != Service::Google || app.secret_kept => {
                Some(app.app_id.clone())
            }
            Some(_) => None,
            None => self.built_in_app(conn.service),
        }
    }

    /// The secret of the owner's own app for `id` (Google), from the Vault.
    fn app_secret(&self, id: &str) -> Result<String, String> {
        vault::read(self.store.as_ref(), &app_secret_id(id))
            .map_err(|e| format!("Plenipo could not read your app's secret ({e})."))?
            .ok_or_else(|| {
                "Your Google app's secret is missing from the Vault. Save your Google app again \
                 (Settings → Connections → Google)."
                    .to_owned()
            })
    }

    /// The ports Slack's sign-in may come back to.
    fn slack_ports(&self) -> Vec<u16> {
        self.config
            .slack_ports
            .clone()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| slack::REDIRECT_PORTS.to_vec())
    }

    pub fn signing_in(&self, id: &str) -> bool {
        lock(&self.state).waiting.contains_key(id)
    }

    /// Every sign-in value to hide in text: the long-lived ones in the Vault (for every kept
    /// connection, connected or not, so a value left behind is hidden too), the owner's own
    /// apps' secrets, and the access tokens in memory. (value, name)
    pub fn secrets(&self) -> Vec<(String, String)> {
        let conns = self.guard().connections().unwrap_or_default();
        let mut out: Vec<(String, String)> = Vec::new();
        for c in &conns {
            if let Some(v) = vault::read(self.store.as_ref(), &vault_id(&c.id))
                .ok()
                .flatten()
            {
                if c.service.uses_key() {
                    // A key, and every form it may take on its way (ADR-071 §1).
                    for form in keyed::key_forms(&v) {
                        out.push((form, format!("{} key", c.label())));
                    }
                } else {
                    out.push((v, format!("{} sign-in", c.label())));
                }
            }
            if let Some(v) = vault::read(self.store.as_ref(), &keyed::store_key_id(&c.id))
                .ok()
                .flatten()
            {
                for form in keyed::key_forms(&v) {
                    out.push((form, "WooCommerce key".into()));
                }
            }
            if c.own_app.as_ref().is_some_and(|a| a.secret_kept) {
                if let Some(v) = vault::read(self.store.as_ref(), &app_secret_id(&c.id))
                    .ok()
                    .flatten()
                {
                    out.push((v, format!("{} app secret", c.label())));
                }
            }
        }
        let s = lock(&self.state);
        out.extend(
            s.tokens
                .iter()
                .map(|(id, (t, _))| (t.clone(), format!("{} sign-in", name_of(id)))),
        );
        out.extend(
            s.typed
                .iter()
                .map(|t| (t.clone(), "a typed key".to_owned())),
        );
        out
    }

    /// The IDs of every Vault value connections keep (for uninstalling with "delete my data"):
    /// each kept connection's sign-in and app secret, and each service's own, in case one was
    /// kept before its connection was.
    pub fn vault_ids(config: &plenipo_guard::GuardConfig) -> Vec<String> {
        vault_ids_of(config.connections.iter().map(|c| c.id.as_str()))
    }

    // ---- The owner's own app ---------------------------------------------------------------------

    /// Keep the owner's own Slack or Google app for `id` (ADR-070 §3–§4), or remove it (`None`).
    /// A Google app's secret goes straight to the Vault and is read back; it is never returned.
    /// The caller has asked Guard, and checked no sign-in waits.
    pub fn save_app(&self, id: &str, app: Option<&AppInput>) -> Result<(), String> {
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        let key = app_secret_id(id);
        let _one = lock(&self.commit);
        match app {
            None => {
                self.guard()
                    .set_connection_client_app(id, None, false)
                    .map_err(|e| e.to_string())?;
                let erased = vault::erase(self.store.as_ref(), &key).map_err(|e| {
                    format!(
                        "Plenipo could not remove your app's secret from {} ({e}).",
                        self.store.label()
                    )
                });
                drop(_one);
                self.changed();
                erased
            }
            Some(app) => {
                let secret = app
                    .secret
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                match (conn.service, secret) {
                    (Service::Google, None) => {
                        return Err("Type your Google app's client secret too.".into())
                    }
                    (Service::Slack, Some(_)) => {
                        return Err(
                            "A Slack app signs in with no secret: leave the secret out.".into()
                        )
                    }
                    _ => {}
                }
                if let Some(s) = secret {
                    if s.len() > MAX_APP_SECRET
                        || s.chars().any(|c| c.is_control() || c.is_whitespace())
                    {
                        return Err("That does not look like an app's secret.".into());
                    }
                    // Checked before anything is kept.
                    if conn.state != ConnectionState::NotConnected {
                        return Err(format!(
                            "Disconnect {} first: its sign-in belongs to the app it was made \
                             with.",
                            conn.label()
                        ));
                    }
                    let previous = vault::read(self.store.as_ref(), &key).ok().flatten();
                    self.keep_at(&key, s, previous.as_deref(), "your app's secret")?;
                    if let Err(e) =
                        self.guard()
                            .set_connection_client_app(id, Some(&app.client_id), true)
                    {
                        let _ = match previous {
                            Some(p) => vault::put(self.store.as_ref(), &key, &p),
                            None => vault::erase(self.store.as_ref(), &key),
                        };
                        return Err(e.to_string());
                    }
                } else {
                    self.guard()
                        .set_connection_client_app(id, Some(&app.client_id), false)
                        .map_err(|e| e.to_string())?;
                }
                drop(_one);
                self.changed();
                Ok(())
            }
        }
    }

    /// Forget a card that is not connected (a Slack workspace): its settings, and anything the
    /// Vault still holds for it. The caller has asked Guard.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _one = lock(&self.commit);
        self.guard()
            .remove_connection(id)
            .map_err(|e| e.to_string())?;
        {
            let mut s = lock(&self.state);
            s.next(id);
            s.tokens.remove(id);
            s.admin.remove(id);
            s.problem.remove(id);
        }
        let erased = [vault_id(id), app_secret_id(id)]
            .iter()
            .try_for_each(|k| vault::erase(self.store.as_ref(), k))
            .map_err(|e| {
                format!(
                    "Plenipo could not remove what {} kept for it ({e}).",
                    self.store.label()
                )
            });
        drop(_one);
        self.changed();
        erased
    }

    // ---- Signing in ----------------------------------------------------------------------------

    /// Start signing in to `id` with an account of `kind` (Microsoft 365; the others have one
    /// kind): open the service's sign-in page in the owner's browser, and wait for the answer in
    /// the background. The caller has asked Guard.
    pub async fn start_sign_in(
        self: &Arc<Self>,
        id: &str,
        kind: AccountKind,
    ) -> Result<(), String> {
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        let service = conn.service;
        if service.uses_key() {
            return Err(format!(
                "{} connects with a key: type it into its card, then press Save and check.",
                conn.label()
            ));
        }
        let app_id = self.app_id(&conn).ok_or_else(|| {
            format!(
                "This copy of Plenipo has no app ID for {} yet.",
                conn.label()
            )
        })?;
        let secret = match service {
            Service::Google => Some(self.app_secret(id)?),
            _ => None,
        };
        let kind = if service != Service::Microsoft365 || conn.own_app.is_some() {
            AccountKind::Work
        } else {
            kind
        };
        let listener = match service {
            Service::Slack => Listener::open_on(&self.slack_ports()).await.map_err(|e| {
                format!(
                    "Plenipo could not wait for Slack's sign-in on this PC: another program holds \
                     the ports Slack comes back to ({}). Close it, or try again ({e}).",
                    self.slack_ports()
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
            _ => Listener::open().await.map_err(|e| {
                format!(
                    "Plenipo could not wait for the sign-in on {} ({e})",
                    plenipo_core::WORDS.this_computer
                )
            })?,
        };
        let pkce = Pkce::new();
        let redirect = match service {
            // Google's desktop apps come back to the loopback address itself.
            Service::Google => format!("http://127.0.0.1:{}", listener.port),
            _ => format!("http://localhost:{}", listener.port),
        };
        let authority = match service {
            Service::Microsoft365 => microsoft365::authority(kind, conn.own_app.as_ref()),
            _ => String::new(),
        };
        // The permissions asked for now are the ones the code is traded for, whatever the owner
        // changes while signing in.
        let (scopes, address) = match service {
            Service::Microsoft365 => {
                let scopes = microsoft365::scopes(&conn, kind);
                let address = microsoft365::authorize_address(
                    &app_id,
                    &authority,
                    &redirect,
                    &scopes,
                    &pkce.challenge,
                    &pkce.state,
                    kind,
                );
                (scopes.join(" "), address)
            }
            Service::Slack => (
                String::new(),
                slack::authorize_address(
                    &app_id,
                    &redirect,
                    &slack::scopes(&conn),
                    &pkce.challenge,
                    &pkce.state,
                ),
            ),
            _ => (
                String::new(),
                google::authorize_address(
                    &app_id,
                    &redirect,
                    &google::scopes(&conn),
                    &pkce.challenge,
                    &pkce.state,
                ),
            ),
        };
        let reachable = self.http.address(&address);
        self.http
            .check(service, &reachable)
            .map_err(|e| e.to_string())?;
        let (cancel, cancelled) = oneshot::channel();
        let turn = {
            let mut s = lock(&self.state);
            if s.waiting.contains_key(id) {
                return Err(format!(
                    "A sign-in to {} is already waiting in your browser.",
                    conn.label()
                ));
            }
            let turn = s.next(id);
            s.waiting.insert(id.to_owned(), (turn, cancel));
            s.admin.remove(id);
            s.problem.remove(id);
            turn
        };
        let opener = self
            .opener
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let start = listener.start_address();
        let this = Arc::clone(self);
        let id = id.to_owned();
        tokio::spawn(async move {
            // Listen while the browser opens: a browser may come back before its opener
            // returns (the tests' stand-in browser follows the whole sign-in inside it).
            let open = opener.open(start);
            let wait = listener.wait(&reachable, &pkce.state, cancelled);
            tokio::pin!(open);
            tokio::pin!(wait);
            let answer = tokio::select! {
                opened = &mut open => match opened {
                    Err(why) => Err(format!("Plenipo could not open your browser ({why}).")),
                    Ok(()) => Ok(wait.await),
                },
                answer = &mut wait => Ok(answer),
            };
            let outcome = match answer {
                Err(why) => Err(why),
                Ok(None) => Ok(Signed::Stopped),
                Ok(Some(Callback::Code(code))) => {
                    let sign_in = SignIn {
                        id: &id,
                        turn,
                        kind,
                        app_id: &app_id,
                        secret: secret.as_deref(),
                        authority: &authority,
                        redirect: &redirect,
                        scopes: &scopes,
                    };
                    this.finish_sign_in(&sign_in, &code, &pkce.verifier).await
                }
                Ok(Some(Callback::Refused { error, description })) => Err(match service {
                    Service::Microsoft365 => {
                        if microsoft365::admin_may_help(&error, &description) {
                            lock(&this.state)
                                .admin
                                .insert(id.clone(), microsoft365::admin_link(&app_id, None));
                        }
                        microsoft365::refusal_words(&error, &description)
                    }
                    Service::Slack => slack::refusal_words(&error),
                    _ => google::refusal_words(&error),
                }),
            };
            // What went wrong is kept in the same step that ends the sign-in, so nothing reading
            // the card sees it ended with neither a connection nor a problem.
            {
                let mut s = lock(&this.state);
                if s.waiting.get(&id).is_some_and(|(t, _)| *t == turn) {
                    s.waiting.remove(&id);
                }
                if let (true, Err(why)) = (s.turn(&id) == turn, &outcome) {
                    s.problem.insert(id.clone(), why.clone());
                }
            }
            match outcome {
                Ok(Signed::In) => {}
                Ok(Signed::Stopped) => {
                    // Cancelled, ten minutes passed, or something newer: nothing kept.
                    let _ = this
                        .guard()
                        .ledger()
                        .append_event(plenipo_ledger::NewEvent {
                            source: "plenipo".into(),
                            event_type: "connection.sign_in_stopped".into(),
                            payload: json!({ "connectionId": id, "service": name_of(&id) }),
                            ..plenipo_ledger::NewEvent::default()
                        });
                }
                Err(why) => {
                    let _ = this.guard().ledger().append_event(plenipo_ledger::NewEvent {
                        source: "plenipo".into(),
                        event_type: "connection.sign_in_failed".into(),
                        payload: json!({ "connectionId": id, "service": name_of(&id), "reason": why }),
                        ..plenipo_ledger::NewEvent::default()
                    });
                }
            }
        });
        Ok(())
    }

    /// Trade the code for the sign-in, and keep it — unless a Cancel, Disconnect, or newer
    /// sign-in came meanwhile.
    async fn finish_sign_in(
        &self,
        s: &SignIn<'_>,
        code: &str,
        verifier: &str,
    ) -> Result<Signed, String> {
        let id = s.id;
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        // The sign-in belongs to the app it started with.
        if self.app_id(&conn).as_deref() != Some(s.app_id) {
            return Err(format!(
                "{}'s app changed while you signed in. Connect again.",
                conn.label()
            ));
        }
        let traded = match conn.service {
            Service::Microsoft365 => self.trade_microsoft(s, code, verifier).await?,
            Service::Slack => self.trade_slack(s, code, verifier).await?,
            _ => self.trade_google(s, code, verifier).await?,
        };
        {
            let _one = lock(&self.commit);
            if lock(&self.state).turn(id) != s.turn {
                return Ok(Signed::Stopped);
            }
            // The account this card is for, checked where nothing can change it meanwhile.
            let a = &traded.account;
            let (who, shown) = match conn.service {
                Service::Slack => (
                    a.tenant.as_deref().unwrap_or_default(),
                    a.organization.as_deref().unwrap_or("a Slack workspace"),
                ),
                _ => (a.address.as_str(), a.address.as_str()),
            };
            self.account_fits(id, conn.service, who, shown)?;
            let previous = vault::read(self.store.as_ref(), &vault_id(id))
                .ok()
                .flatten();
            self.keep(id, &traded.long_lived, previous.as_deref())?;
            if let Some((access, lasts)) = &traded.access {
                lock(&self.state)
                    .tokens
                    .insert(id.to_owned(), (access.clone(), Instant::now() + *lasts));
            }
            if let Err(e) =
                self.guard()
                    .connection_connected(id, traded.kind, traded.account, &traded.granted)
            {
                // Nothing kept that the owner cannot see or disconnect.
                let _ = vault::erase(self.store.as_ref(), &vault_id(id));
                lock(&self.state).tokens.remove(id);
                return Err(e.to_string());
            }
        }
        self.changed();
        Ok(Signed::In)
    }

    /// Whether a sign-in to `who` (a Slack workspace's ID, else the account's address) may be
    /// kept on card `id`: a card that is connected stays with its own workspace or account (its
    /// lists were made for it), and a Slack workspace is on one card only.
    fn account_fits(
        &self,
        id: &str,
        service: Service,
        who: &str,
        shown: &str,
    ) -> Result<(), String> {
        let conns = self.guard().connections().unwrap_or_default();
        let key = |a: &Account| -> String {
            match service {
                Service::Slack => a.tenant.clone().unwrap_or_default(),
                _ => a.address.to_lowercase(),
            }
        };
        let who = if service == Service::Slack {
            who.to_owned()
        } else {
            who.to_lowercase()
        };
        if let Some(now) = conns
            .iter()
            .find(|c| c.id == id && c.state != ConnectionState::NotConnected)
            .and_then(|c| c.account.as_ref())
        {
            let was = key(now);
            if !was.is_empty() && !who.is_empty() && was != who {
                let (place, kind) = match service {
                    Service::Slack => (
                        now.organization
                            .clone()
                            .unwrap_or_else(|| "another workspace".into()),
                        "workspace",
                    ),
                    _ => (now.address.clone(), "account"),
                };
                return Err(format!(
                    "You signed in to {shown}, but this card is for {place}. To switch it to \
                     another {kind}, press Disconnect first, then Connect. Nothing was kept."
                ));
            }
        }
        if service == Service::Slack && !who.is_empty() {
            let elsewhere = conns.iter().any(|c| {
                c.id != id
                    && c.service == Service::Slack
                    && c.state != ConnectionState::NotConnected
                    && c.account.as_ref().and_then(|a| a.tenant.as_deref()) == Some(who.as_str())
            });
            if elsewhere {
                return Err(format!(
                    "{shown}'s Slack is already connected on another card. Use that card, or \
                     disconnect it first."
                ));
            }
        }
        Ok(())
    }

    async fn post_form(
        &self,
        service: Service,
        url: &str,
        form: Vec<(String, String)>,
    ) -> Result<Reply, String> {
        self.http
            .send(
                service,
                reqwest::Method::POST,
                url,
                None,
                &[],
                Body::Form(form),
                MAX_TOKEN_ANSWER,
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn trade_microsoft(
        &self,
        s: &SignIn<'_>,
        code: &str,
        verifier: &str,
    ) -> Result<Traded, String> {
        let form = vec![
            ("client_id".to_owned(), s.app_id.to_owned()),
            ("grant_type".to_owned(), "authorization_code".to_owned()),
            ("code".to_owned(), code.to_owned()),
            ("redirect_uri".to_owned(), s.redirect.to_owned()),
            ("code_verifier".to_owned(), verifier.to_owned()),
            ("scope".to_owned(), s.scopes.to_owned()),
        ];
        let reply = self
            .post_form(
                Service::Microsoft365,
                &microsoft365::token_address(s.authority),
                form,
            )
            .await?;
        let answer = reply.json();
        if !reply.ok() {
            let error = answer["error"].as_str().unwrap_or_default();
            let description = answer["error_description"].as_str().unwrap_or_default();
            if microsoft365::admin_may_help(error, description) {
                lock(&self.state)
                    .admin
                    .insert(s.id.to_owned(), microsoft365::admin_link(s.app_id, None));
            }
            return Err(microsoft365::refusal_words(error, description));
        }
        let refresh = answer["refresh_token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "Microsoft did not let Plenipo stay signed in.".to_owned())?;
        let (mut account, tenant) =
            microsoft365::account_from_id_token(answer["id_token"].as_str().unwrap_or_default());
        let kind = if tenant.as_deref() == Some(microsoft365::PERSONAL_TENANT) {
            AccountKind::Personal
        } else {
            s.kind
        };
        account.tenant = tenant;
        Ok(Traded {
            long_lived: refresh.to_owned(),
            access: answer["access_token"].as_str().map(|a| {
                (
                    a.to_owned(),
                    Duration::from_secs(answer["expires_in"].as_u64().unwrap_or(3600)),
                )
            }),
            kind: Some(kind),
            account,
            granted: microsoft365::granted(answer["scope"].as_str().unwrap_or_default()),
        })
    }

    async fn trade_slack(
        &self,
        s: &SignIn<'_>,
        code: &str,
        verifier: &str,
    ) -> Result<Traded, String> {
        // PKCE: no client secret (ADR-070 §5.1).
        let form = vec![
            ("client_id".to_owned(), s.app_id.to_owned()),
            ("grant_type".to_owned(), "authorization_code".to_owned()),
            ("code".to_owned(), code.to_owned()),
            ("code_verifier".to_owned(), verifier.to_owned()),
            ("redirect_uri".to_owned(), s.redirect.to_owned()),
        ];
        let reply = self.post_form(Service::Slack, slack::ACCESS, form).await?;
        let answer = reply.json();
        if !reply.ok() || answer["ok"].as_bool() != Some(true) {
            return Err(slack::refusal_words(
                answer["error"].as_str().unwrap_or("no_answer"),
            ));
        }
        let user = slack::user_tokens(&answer);
        let access = user["access_token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "Slack did not sign Plenipo in as you.".to_owned())?
            .to_owned();
        // With token rotation, the refresh token lasts (30 days from its last use); without it,
        // the access token itself does not expire.
        let (long_lived, lasts) = match user["refresh_token"].as_str().filter(|t| !t.is_empty()) {
            Some(r) => (
                r.to_owned(),
                Duration::from_secs(user["expires_in"].as_u64().unwrap_or(43_200)),
            ),
            None => (access.clone(), LASTING),
        };
        let team_id = answer["team"]["id"].as_str().unwrap_or_default().to_owned();
        let team_name = answer["team"]["name"]
            .as_str()
            .unwrap_or("a Slack workspace")
            .to_owned();
        // One workspace per card: said early here, and checked again as it is kept.
        self.account_fits(s.id, Service::Slack, &team_id, &team_name)?;
        // Who signed in: their name and, when Slack allows it, their email address.
        let user_id = user["id"].as_str().unwrap_or_default().to_owned();
        let who = self
            .http
            .send(
                Service::Slack,
                reqwest::Method::GET,
                &slack::method_address("users.info", &[("user", user_id.clone())]),
                Some(&access),
                &[],
                Body::None,
                MAX_TOKEN_ANSWER,
            )
            .await
            .map(|r| r.json())
            .unwrap_or(Value::Null);
        let profile = &who["user"]["profile"];
        let name = [
            profile["real_name"].as_str(),
            who["user"]["real_name"].as_str(),
            who["user"]["name"].as_str(),
        ]
        .into_iter()
        .flatten()
        .find(|n| !n.trim().is_empty())
        .unwrap_or(user_id.as_str())
        .to_owned();
        let address = profile["email"]
            .as_str()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| plenipo_guard::connections::is_address(e))
            .unwrap_or_else(|| name.clone());
        Ok(Traded {
            long_lived,
            access: Some((access, lasts)),
            kind: None,
            account: Account {
                name,
                address,
                organization: Some(team_name),
                tenant: Some(team_id).filter(|t| !t.is_empty()),
            },
            granted: slack::granted(user["scope"].as_str().unwrap_or_default()),
        })
    }

    async fn trade_google(
        &self,
        s: &SignIn<'_>,
        code: &str,
        verifier: &str,
    ) -> Result<Traded, String> {
        let form = vec![
            ("client_id".to_owned(), s.app_id.to_owned()),
            (
                "client_secret".to_owned(),
                s.secret.unwrap_or_default().to_owned(),
            ),
            ("grant_type".to_owned(), "authorization_code".to_owned()),
            ("code".to_owned(), code.to_owned()),
            ("code_verifier".to_owned(), verifier.to_owned()),
            ("redirect_uri".to_owned(), s.redirect.to_owned()),
        ];
        let reply = self.post_form(Service::Google, google::TOKEN, form).await?;
        let answer = reply.json();
        if !reply.ok() {
            return Err(google::refusal_words(
                answer["error"].as_str().unwrap_or("no_answer"),
            ));
        }
        let refresh = answer["refresh_token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                "Google did not let Plenipo stay signed in. Connect again.".to_owned()
            })?;
        let account =
            google::account_from_id_token(answer["id_token"].as_str().unwrap_or_default());
        Ok(Traded {
            long_lived: refresh.to_owned(),
            access: answer["access_token"].as_str().map(|a| {
                (
                    a.to_owned(),
                    Duration::from_secs(answer["expires_in"].as_u64().unwrap_or(3600)),
                )
            }),
            kind: None,
            account,
            granted: google::granted(answer["scope"].as_str().unwrap_or_default()),
        })
    }

    /// Keep a long-lived sign-in in the Vault, and check it reads back the same. If that fails,
    /// the `previous` one goes back (a service does not always cancel it when it is used), or,
    /// with none, nothing is left — never a mix of the two.
    fn keep(&self, id: &str, value: &str, previous: Option<&str>) -> Result<(), String> {
        self.keep_at(&vault_id(id), value, previous, "the sign-in")
    }

    fn keep_at(
        &self,
        key: &str,
        value: &str,
        previous: Option<&str>,
        what: &str,
    ) -> Result<(), String> {
        let label = self.store.label();
        let kept = vault::put(self.store.as_ref(), key, value)
            .map_err(|e| format!("Plenipo could not keep {what} in {label} ({e})."))
            .and_then(|()| match vault::read(self.store.as_ref(), key) {
                Ok(Some(v)) if v == value => Ok(()),
                _ => Err(format!(
                    "Plenipo could not keep {what} in {label}: it did not read back the same."
                )),
            });
        if kept.is_err() {
            let _ = match previous {
                Some(p) => vault::put(self.store.as_ref(), key, p),
                None => vault::erase(self.store.as_ref(), key),
            };
        }
        kept
    }

    /// Stop a sign-in waiting in the owner's browser (even one the service already answered:
    /// its sign-in is not kept).
    pub fn cancel(&self, id: &str) -> bool {
        let mut s = lock(&self.state);
        match s.waiting.remove(id) {
            Some((_, tx)) => {
                s.next(id);
                let _ = tx.send(());
                true
            }
            None => false,
        }
    }

    /// Disconnect: its tools stop at once — it is marked not connected, and its access token and
    /// any sign-in under way are dropped — then its sign-in leaves the Vault (with its pieces),
    /// and then, where the service can, the sign-in is cancelled there (Slack, Google; ADR-070
    /// §5.8). Microsoft has no way to cancel one sign-in, so nothing is sent. A service that
    /// cannot be reached leaves a note on the card; the sign-in is gone from this computer
    /// either way.
    pub async fn disconnect(&self, id: &str) -> Result<(), String> {
        let conn = self.guard().connection(id).ok();
        let marked;
        let erased;
        let kept;
        let access;
        {
            let _one = lock(&self.commit);
            {
                let mut s = lock(&self.state);
                s.next(id);
                if let Some((_, tx)) = s.waiting.remove(id) {
                    let _ = tx.send(());
                }
                // Only one that has not run out is worth cancelling too.
                access = s
                    .tokens
                    .remove(id)
                    .filter(|(_, until)| *until > Instant::now())
                    .map(|(t, _)| t);
                s.admin.remove(id);
                s.problem.remove(id);
            }
            marked = self
                .guard()
                .connection_disconnected(id)
                .map(|_| ())
                .map_err(|e| e.to_string());
            kept = vault::read(self.store.as_ref(), &vault_id(id))
                .ok()
                .flatten();
            erased = vault::erase(self.store.as_ref(), &vault_id(id))
                .and_then(|()| vault::erase(self.store.as_ref(), &keyed::store_key_id(id)))
                .map_err(|e| {
                    format!(
                        "Plenipo could not remove the sign-in from {} ({e}).",
                        self.store.label()
                    )
                });
        }
        self.changed();
        if let (Some(conn), Some(kept)) = (conn, kept) {
            let cancelled = tokio::time::timeout(
                CANCEL_WAIT,
                self.cancel_at_service(
                    conn.service,
                    &kept,
                    access.as_deref(),
                    conn.site.as_deref(),
                ),
            )
            .await
            .unwrap_or_else(|_| Err(format!("{} took too long to answer", conn.label())));
            if let Err(why) = cancelled {
                let words = match conn.service {
                    Service::Wordpress => format!(
                        "Plenipo removed the password from this computer, but could not revoke it \
                         at your site ({why}). To be sure, revoke it in WordPress: Users, then \
                         Profile, then Application Passwords, then Revoke."
                    ),
                    Service::Slack => format!(
                        "Plenipo removed the sign-in from this computer, but could not cancel it \
                         at Slack ({why}). To be sure, remove Plenipo in Slack: your workspace's \
                         Apps page, then Plenipo, then Remove."
                    ),
                    _ => format!(
                        "Plenipo removed the sign-in from this computer, but could not cancel it \
                         at Google ({why}). To be sure, remove your app from your Google account: \
                         myaccount.google.com, then Security, then Your connections to \
                         third-party apps."
                    ),
                };
                lock(&self.state).problem.insert(id.to_owned(), words);
            }
        }
        marked.and(erased)
    }

    /// Cancel a sign-in at the service. Slack: `auth.revoke` for the long-lived sign-in (with
    /// token rotation Slack cancels only the one it is given, so the renewal itself), then for
    /// the short-lived one still in memory. Google: its revoke address. The website: the
    /// Application Password Plenipo used, which WordPress names for it. HubSpot's and Stripe's
    /// keys cannot be cancelled from outside (the card says where to delete them). Nothing is
    /// kept from it.
    async fn cancel_at_service(
        &self,
        service: Service,
        kept: &str,
        access: Option<&str>,
        site: Option<&str>,
    ) -> Result<(), String> {
        match service {
            Service::Slack => {
                // Already gone at Slack counts as cancelled.
                let gone = |e: &str| {
                    matches!(
                        e,
                        "invalid_auth" | "token_revoked" | "account_inactive" | "token_expired"
                    )
                };
                let mut first = Ok(());
                for (n, token) in std::iter::once(kept).chain(access).enumerate() {
                    let reply = self
                        .http
                        .send(
                            service,
                            reqwest::Method::POST,
                            &slack::method_address("auth.revoke", &[]),
                            Some(token),
                            &[],
                            Body::Empty,
                            MAX_TOKEN_ANSWER,
                        )
                        .await
                        .map_err(|e| e.to_string());
                    let done = reply.and_then(|r| {
                        let answer = r.json();
                        let error = answer["error"].as_str().unwrap_or("nothing");
                        if answer["ok"].as_bool() == Some(true) || gone(error) {
                            Ok(())
                        } else {
                            Err(format!("Slack answered {}", slack::clean_code(error)))
                        }
                    });
                    // The long-lived sign-in is what must be cancelled.
                    if n == 0 {
                        first = done;
                    }
                }
                first
            }
            Service::Google => {
                let reply = self
                    .post_form(
                        service,
                        google::REVOKE,
                        vec![("token".to_owned(), kept.to_owned())],
                    )
                    .await?;
                // A sign-in Google no longer knows is cancelled already.
                if reply.ok() || reply.json()["error"].as_str() == Some("invalid_token") {
                    Ok(())
                } else {
                    Err(format!("Google answered {}", reply.status))
                }
            }
            Service::Wordpress => {
                // The address the password was kept for, read before Disconnect: never one
                // saved since.
                let site = site.ok_or("the site's address is gone")?;
                let (user, password) = kept.rsplit_once(':').ok_or("the password was not whole")?;
                let auth = http::Auth::Basic { user, password };
                let send = |method: reqwest::Method, url: String| async move {
                    self.http
                        .send_with(
                            service,
                            method,
                            &url,
                            Some(auth),
                            &[],
                            Body::None,
                            MAX_TOKEN_ANSWER,
                            true,
                        )
                        .await
                        .map_err(|e| e.to_string())
                };
                let base = format!("{site}/wp-json/wp/v2/users/me/application-passwords");
                let me = send(reqwest::Method::GET, format!("{base}/introspect")).await?;
                // A password the site no longer takes is revoked already (not a site that drops
                // the sign-in on its way, which answers 401 too).
                let refused = |r: &http::Reply| {
                    r.status == 401
                        && matches!(
                            keyed::wordpress_code(r).as_str(),
                            "incorrect_password" | "invalid_username" | "invalid_email"
                        )
                        || keyed::wordpress_code(r).starts_with("application_passwords_disabled")
                };
                if refused(&me) {
                    return Ok(());
                }
                let uuid = me.json()["uuid"]
                    .as_str()
                    .filter(|u| {
                        u.len() <= 40 && u.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                    })
                    .map(str::to_owned)
                    .ok_or_else(|| format!("your site answered {}", me.status))?;
                let gone = send(reqwest::Method::DELETE, format!("{base}/{uuid}")).await?;
                if gone.ok() || refused(&gone) {
                    Ok(())
                } else {
                    Err(format!("your site answered {}", gone.status))
                }
            }
            _ => Ok(()),
        }
    }

    // ---- Fresh access tokens --------------------------------------------------------------------

    /// A fresh access token for `id`: the one in memory while it has time left, else a new one
    /// from the long-lived sign-in (which Microsoft and Slack replace each time; the new one
    /// replaces the old in the Vault). When the service refuses, the connection needs the owner
    /// to sign in again.
    async fn access_token(&self, id: &str) -> Result<String, String> {
        let fresh = |s: &State| {
            s.tokens
                .get(id)
                .filter(|(_, exp)| *exp > Instant::now() + REFRESH_EARLY)
                .map(|(t, _)| t.clone())
        };
        if let Some(t) = fresh(&lock(&self.state)) {
            return Ok(t);
        }
        let _one = self.refreshing.lock().await;
        let turn = {
            let s = lock(&self.state);
            if let Some(t) = fresh(&s) {
                return Ok(t);
            }
            s.turn(id)
        };
        let name = name_of(id);
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        if conn.service.uses_key() {
            return Err(format!("{name} uses a key, not a sign-in."));
        }
        match conn.state {
            ConnectionState::Connected => {}
            ConnectionState::NeedsSignIn => {
                return Err(format!(
                    "{name} needs the owner to sign in again (Settings → Connections)."
                ))
            }
            ConnectionState::NotConnected => return Err(format!("{name} is not connected.")),
        }
        let app_id = self
            .app_id(&conn)
            .ok_or_else(|| format!("This copy of Plenipo has no app ID for {name}."))?;
        let refresh = vault::read(self.store.as_ref(), &vault_id(id))
            .map_err(|e| format!("Plenipo could not read {name}'s sign-in ({e})."))?
            .ok_or_else(|| format!("{name}'s sign-in is missing from the Vault."))?;
        let (url, form) = match conn.service {
            Service::Microsoft365 => {
                let kind = conn.account_kind.unwrap_or(AccountKind::Work);
                let authority = match (&conn.own_app, kind) {
                    (Some(app), _) => app.tenant.clone().unwrap_or_else(|| "organizations".into()),
                    (None, AccountKind::Personal) => "consumers".into(),
                    (None, AccountKind::Work) => conn
                        .account
                        .as_ref()
                        .and_then(|a| a.tenant.clone())
                        .unwrap_or_else(|| "organizations".into()),
                };
                // Only what Microsoft already granted: asking for more fails the renewal.
                (
                    microsoft365::token_address(&authority),
                    vec![
                        ("client_id".to_owned(), app_id),
                        ("grant_type".to_owned(), "refresh_token".to_owned()),
                        ("refresh_token".to_owned(), refresh.clone()),
                        (
                            "scope".to_owned(),
                            microsoft365::refresh_scopes(&conn, kind).join(" "),
                        ),
                    ],
                )
            }
            Service::Slack => {
                if slack::lasting(&refresh) {
                    // A workspace app without token rotation: the sign-in is the access token.
                    lock(&self.state)
                        .tokens
                        .insert(id.to_owned(), (refresh.clone(), Instant::now() + LASTING));
                    return Ok(refresh);
                }
                (
                    slack::ACCESS.to_owned(),
                    vec![
                        ("client_id".to_owned(), app_id),
                        ("grant_type".to_owned(), "refresh_token".to_owned()),
                        ("refresh_token".to_owned(), refresh.clone()),
                    ],
                )
            }
            _ => (
                google::TOKEN.to_owned(),
                vec![
                    ("client_id".to_owned(), app_id),
                    ("client_secret".to_owned(), self.app_secret(id)?),
                    ("grant_type".to_owned(), "refresh_token".to_owned()),
                    ("refresh_token".to_owned(), refresh.clone()),
                ],
            ),
        };
        let reply = self
            .http
            .send(
                conn.service,
                reqwest::Method::POST,
                &url,
                None,
                &[],
                Body::Form(form),
                MAX_TOKEN_ANSWER,
            )
            .await
            .map_err(|e| format!("{name}: {e}"))?;
        let mut answer = reply.json();
        let ok = match conn.service {
            Service::Slack => {
                let ok = reply.ok() && answer["ok"].as_bool() == Some(true);
                if ok {
                    answer = slack::user_tokens(&answer);
                }
                ok
            }
            _ => reply.ok(),
        };
        let result = {
            let _commit = lock(&self.commit);
            // Disconnected, cancelled, or signed in again meanwhile: this renewal keeps nothing.
            if lock(&self.state).turn(id) != turn {
                return Err(format!(
                    "{name} was disconnected or signed in again meanwhile. Try again."
                ));
            }
            if ok {
                self.renewed(id, &answer, &refresh)
            } else {
                Err(self.renewal_refused(id, conn.service, &answer, reply.status))
            }
        };
        self.changed();
        result
    }

    /// Keep a renewal: the new long-lived sign-in (if any) and the access token.
    fn renewed(&self, id: &str, answer: &Value, previous: &str) -> Result<String, String> {
        let name = name_of(id);
        let access = answer["access_token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| format!("{name} sent no access token."))?
            .to_owned();
        if let Some(new) = answer["refresh_token"].as_str().filter(|t| !t.is_empty()) {
            self.keep(id, new, Some(previous))?;
        }
        let expires = answer["expires_in"].as_u64().unwrap_or(3600);
        lock(&self.state).tokens.insert(
            id.to_owned(),
            (
                access.clone(),
                Instant::now() + Duration::from_secs(expires),
            ),
        );
        Ok(access)
    }

    /// The service refused a renewal: when it no longer accepts the sign-in, forget it, and say
    /// the owner must sign in again. What the worker is told.
    fn renewal_refused(&self, id: &str, service: Service, answer: &Value, status: u16) -> String {
        let name = name_of(id);
        let error = answer["error"].as_str().unwrap_or_default();
        let gone = match service {
            Service::Microsoft365 => matches!(
                error,
                "invalid_grant" | "interaction_required" | "consent_required"
            ),
            // `invalid_client_id`: the workspace's own Slack app was deleted, or changed.
            Service::Slack => {
                slack::sign_in_gone(error)
                    || matches!(error, "invalid_refresh_token" | "invalid_client_id")
            }
            _ => matches!(
                error,
                "invalid_grant" | "invalid_client" | "unauthorized_client"
            ),
        };
        if !gone {
            return format!("{name} did not renew the sign-in ({status}).");
        }
        let reason = match (service, error) {
            (Service::Slack, "invalid_client_id") => format!(
                "{name} no longer knows the Slack app Plenipo signed in with (it was deleted, or \
                 its client ID changed)."
            ),
            (Service::Google, "invalid_client" | "unauthorized_client") => format!(
                "{name} no longer accepts your Google app (its client ID or secret changed, or it \
                 was deleted)."
            ),
            (_, "interaction_required" | "consent_required") => {
                format!("{name} asks for the sign-in to be done again.")
            }
            _ => format!(
                "{name} no longer accepts the sign-in (it expired, was removed, or the password \
                 changed)."
            ),
        };
        self.forget_sign_in(id, &reason);
        format!("{name} needs the owner to sign in again (Settings → Connections).")
    }

    /// The service no longer accepts the sign-in: erase it, drop the access token, and mark the
    /// connection as needing the owner to sign in again.
    fn forget_sign_in(&self, id: &str, reason: &str) {
        let _ = vault::erase(self.store.as_ref(), &vault_id(id));
        lock(&self.state).tokens.remove(id);
        let _ = self.guard().connection_needs_sign_in(id, reason);
    }

    /// One call to the service's API for connection `id`, with a fresh access token (tried once
    /// more with a new one when the service says the token is no good: Microsoft's and Google's
    /// 401, Slack's `token_expired`). A Slack sign-in that Slack no longer accepts is forgotten,
    /// and the owner must sign in again.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn api_call(
        &self,
        id: &str,
        service: Service,
        method: reqwest::Method,
        url: &str,
        headers: &[(&'static str, &str)],
        body: Body,
        limit: usize,
    ) -> Result<Reply, String> {
        let name = name_of(id);
        let token = self.access_token(id).await?;
        let send = |token: String, method: reqwest::Method, body: Body| async move {
            self.http
                .send(service, method, url, Some(&token), headers, body, limit)
                .await
                .map_err(|e| match e {
                    HttpError::TooBig => format!("{name}'s answer was too big to use here."),
                    other => other.to_string(),
                })
        };
        let reply = send(token.clone(), method.clone(), body.clone()).await?;
        // A Slack sign-in that does not expire, and that Slack no longer accepts, cannot be
        // renewed: the owner signs in again.
        let slack_gone = |reply: &Reply, token: &str| {
            service == Service::Slack
                && slack::lasting(token)
                && slack::error_of(reply).is_some_and(|e| slack::sign_in_gone(&e))
        };
        let expired = match service {
            Service::Slack => slack::error_of(&reply).is_some_and(|e| {
                e == "token_expired" || (slack::sign_in_gone(&e) && !slack::lasting(&token))
            }),
            _ => reply.status == 401,
        };
        if slack_gone(&reply, &token) {
            return Err(self.slack_refused(id));
        }
        if !expired {
            return Ok(reply);
        }
        {
            let mut s = lock(&self.state);
            // Only the token that was refused: a newer one stays.
            if s.tokens.get(id).is_some_and(|(t, _)| *t == token) {
                s.tokens.remove(id);
            }
        }
        let token = self.access_token(id).await?;
        let reply = send(token.clone(), method, body).await?;
        if service == Service::Slack
            && slack::error_of(&reply).is_some_and(|e| slack::sign_in_gone(&e))
        {
            return Err(self.slack_refused(id));
        }
        Ok(reply)
    }

    /// Slack no longer accepts a sign-in: forget it; the owner signs in again.
    fn slack_refused(&self, id: &str) -> String {
        let name = name_of(id);
        {
            let _commit = lock(&self.commit);
            self.forget_sign_in(
                id,
                &format!(
                    "{name} no longer accepts the sign-in (it was removed, or the account was \
                     turned off)."
                ),
            );
        }
        self.changed();
        format!("{name} needs the owner to sign in again (Settings → Connections).")
    }

    // ---- Settings -------------------------------------------------------------------------------

    /// What Settings → Connections shows.
    pub fn page(&self, vault_available: bool) -> Result<ConnectionsPage, String> {
        // Sign-ins first, then the saved connections: a sign-in saves its connection before it
        // stops waiting, so a page never shows one done signing in but not yet connected.
        let seen = {
            let s = lock(&self.state);
            SignIns {
                waiting: s.waiting.keys().cloned().collect(),
                admin: s.admin.clone(),
                problem: s.problem.clone(),
            }
        };
        let config = self.guard().config().map_err(|e| e.to_string())?;
        let records = self
            .guard()
            .ledger()
            .org_records()
            .map_err(|e| e.to_string())?;
        let mut people: Vec<PersonOption> = records
            .roles
            .iter()
            .map(|r| PersonOption {
                kind: "role".into(),
                id: r.id.clone(),
                name: r.name.clone(),
                role: None,
                archived: false,
            })
            .collect();
        people.extend(
            records
                .positions
                .iter()
                .filter(|p| !p.is_deleted())
                .map(|p| PersonOption {
                    kind: "agent".into(),
                    id: p.id.clone(),
                    name: p.title.clone(),
                    role: records
                        .roles
                        .iter()
                        .find(|r| r.id == p.role_id)
                        .map(|r| r.name.clone()),
                    archived: p.archived_at.is_some(),
                }),
        );
        let services = Service::ALL
            .iter()
            .map(|service| {
                let mut conns: Vec<Connection> = config
                    .connections
                    .iter()
                    .filter(|c| c.service == *service)
                    .cloned()
                    .collect();
                if conns.is_empty() && service.built() {
                    conns.push(Connection::new(service.id(), *service));
                }
                ServiceCard {
                    service: *service,
                    label: service.label().into(),
                    built: service.built(),
                    many: service.many(),
                    connections: conns.into_iter().map(|c| self.card(c, &seen)).collect(),
                }
            })
            .collect();
        Ok(ConnectionsPage {
            services,
            people,
            send_switch_on: config.switches.send_without_asking,
            vault_available,
            vault_label: self.store.label().to_owned(),
            slack_manifest: slack::manifest(),
            secret_names: config
                .secrets
                .iter()
                .filter(|s| s.env_var.is_some())
                .map(|s| s.name.clone())
                .collect(),
            add_ons: config.add_ons,
        })
    }

    fn card(&self, c: Connection, seen: &SignIns) -> ConnectionCard {
        let service = c.service;
        let parts = service
            .parts()
            .iter()
            .map(|p| {
                let (reads, changes) = part_words(service, *p);
                PartCard {
                    part: *p,
                    label: p.label().into(),
                    level: c.parts.get(p).copied().unwrap_or_default(),
                    available: service.has_part(*p, c.account_kind),
                    full_access: p.has_full_access(),
                    reads: reads.into(),
                    changes: changes.into(),
                    needs_admin: service == Service::Microsoft365
                        && microsoft365::needs_admin_always(*p, PartLevel::ReadOnly),
                }
            })
            .collect();
        let reconnect_for = if c.state == ConnectionState::Connected {
            parts_to_reconnect(&c)
                .iter()
                .map(|p| p.label().to_owned())
                .collect()
        } else {
            Vec::new()
        };
        let granted = c
            .granted
            .iter()
            .map(|g| PermissionWords {
                name: g.clone(),
                words: permission_words(service, g),
            })
            .collect();
        let store_key_kept = service == Service::Wordpress
            && c.state != ConnectionState::NotConnected
            && c.granted.iter().any(|g| g == "woocommerce:key");
        ConnectionCard {
            uses_key: service.uses_key(),
            key_needs: key_permissions(&c),
            store_key_kept,
            has_app: service.uses_key() || self.app_id(&c).is_some(),
            built_in_app: self.built_in_app(service).is_some(),
            signing_in: seen.waiting.contains(&c.id),
            admin_link: seen.admin.get(&c.id).cloned(),
            problem: seen.problem.get(&c.id).cloned(),
            parts,
            reconnect_for,
            granted,
            connection: c,
        }
    }
}

// ---- Graph, for one Microsoft 365 connection's tools --------------------------------------------

/// Microsoft Graph for one connection: authorized calls, and plain words for Microsoft's errors.
pub(crate) struct Graph<'a> {
    pub conns: &'a Connections,
    pub id: String,
    /// The signed-in account's address, lower case.
    pub me: String,
}

impl Graph<'_> {
    pub fn me(&self) -> &str {
        &self.me
    }

    /// The fence's name for this account: "Microsoft 365 (alex@8westit.com)".
    pub fn account_label(&self) -> String {
        if self.me.is_empty() {
            "Microsoft 365".into()
        } else {
            format!("Microsoft 365 ({})", self.me)
        }
    }

    fn words(reply: &Reply) -> String {
        let code = reply.json()["error"]["code"]
            .as_str()
            .map(|c| {
                c.chars()
                    .filter(|x| x.is_ascii_alphanumeric())
                    .take(40)
                    .collect::<String>()
            })
            .unwrap_or_default();
        let code = if code.is_empty() {
            String::new()
        } else {
            format!(" ({code})")
        };
        match reply.status {
            401 | 403 => format!(
                "Microsoft 365 did not allow this{code}. The part may need Full access (then \
                 Reconnect), or the organization's admin must approve it."
            ),
            404 => format!("Microsoft 365 found nothing there{code}. Check the ID."),
            409 => format!("Microsoft 365 says it is already there{code}."),
            429 => "Microsoft 365 asks Plenipo to slow down. Try again in a minute.".into(),
            s => format!("Microsoft 365 answered {s}{code}."),
        }
    }

    async fn call(
        &self,
        method: reqwest::Method,
        url: &str,
        prefer: Option<&str>,
        body: Body,
        limit: usize,
    ) -> Result<Reply, String> {
        let headers: Vec<(&'static str, &str)> =
            prefer.map(|p| ("Prefer", p)).into_iter().collect();
        let reply = self
            .conns
            .api_call(
                &self.id,
                Service::Microsoft365,
                method,
                url,
                &headers,
                body,
                limit,
            )
            .await?;
        if reply.ok() {
            Ok(reply)
        } else {
            Err(Self::words(&reply))
        }
    }

    pub async fn get_json(&self, url: &str, prefer: Option<&str>) -> Result<Value, String> {
        Ok(self
            .call(reqwest::Method::GET, url, prefer, Body::None, MAX_ANSWER)
            .await?
            .json())
    }

    pub async fn send_json(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Value,
    ) -> Result<Value, String> {
        Ok(self
            .call(method, url, None, Body::Json(body), MAX_ANSWER)
            .await?
            .json())
    }

    pub async fn send_empty(&self, method: reqwest::Method, url: &str) -> Result<(), String> {
        self.call(method, url, None, Body::Empty, MAX_ANSWER)
            .await
            .map(|_| ())
    }

    pub async fn get_bytes(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        Ok(self
            .call(reqwest::Method::GET, url, None, Body::None, limit)
            .await?
            .body)
    }

    pub async fn put_bytes(&self, url: &str, data: Vec<u8>) -> Result<Value, String> {
        Ok(self
            .call(
                reqwest::Method::PUT,
                url,
                None,
                Body::Bytes {
                    content_type: "text/plain; charset=utf-8",
                    data,
                },
                MAX_ANSWER,
            )
            .await?
            .json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_services_own_words_are_shown_on_the_card_and_not_kept() {
        let detail = format!(
            "Refund: USD 25.00\nPayment: pi_1 · {}\nMode: Test mode",
            card_only("\"Tune-up\u{1f} for Alex\u{1e}\" (the payment's own description)")
        );
        assert_eq!(
            as_shown(&detail),
            "Refund: USD 25.00\nPayment: pi_1 · \"Tune-up for Alex\" (the payment's own \
             description)\nMode: Test mode"
        );
        // A marker inside the service's words cannot end the part early.
        assert_eq!(
            as_kept(&detail),
            "Refund: USD 25.00\nPayment: pi_1 · (not kept)\nMode: Test mode"
        );
        // A detail with none is kept as it is.
        assert_eq!(as_kept("To: dana@clientco.com"), "To: dana@clientco.com");
        assert_eq!(as_kept(&card_only("a")), "(not kept)");
    }
}
