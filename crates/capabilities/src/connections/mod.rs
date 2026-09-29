//! Connections (Phase 20; ADR-062, ADR-063, ADR-065): the business's own accounts that Plenipo
//! signs in to for the owner, and the calls workers make through them.
//!
//! - **Signing in** happens in the owner's own browser ([`signin`]): Plenipo never sees a
//!   password. The long-lived sign-in is kept only in the Vault ([`vault_id`]); the short-lived
//!   access token only in memory. Neither is ever recorded, shown, or given to a worker or a
//!   program.
//! - **Calls** go through Plenipo's own client ([`http`]), each hop checked by Guard's gate for
//!   Plenipo's own requests; the access token is added there, for the service's API only.
//! - **Guard decides** every worker call before it runs (the broker asks it; see
//!   `broker/connecting.rs`).

pub(crate) mod http;
pub mod microsoft365;
pub mod signin;
pub(crate) mod text;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use plenipo_guard::{AccountKind, Connection, ConnectionState, Guard, Part, PartLevel, Service};
use plenipo_runtime::Supervisor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use ts_rs::TS;

use self::http::{Body, Http, HttpError, Reply};
use self::signin::{Callback, Listener, Pkce};
use crate::vault::{self, SecretStore};

/// The Vault ID of a connection's long-lived sign-in.
pub fn vault_id(connection_id: &str) -> String {
    format!("connection-{connection_id}-token")
}

/// Refresh the access token when it has less than this left.
const REFRESH_EARLY: Duration = Duration::from_secs(5 * 60);
/// The most a sign-in or token answer may be.
const MAX_TOKEN_ANSWER: usize = 64 * 1024;
/// The most a Graph answer may be (a file's download has its own limit).
const MAX_ANSWER: usize = 8 * 1024 * 1024;

/// How this copy of Plenipo connects: the app IDs it was built with, and, in a copy built for
/// the end-to-end tests only, the stand-in for the services.
#[derive(Debug, Clone, Default)]
pub struct ConnectionsConfig {
    /// 8 West's Microsoft app ID ("Application (client) ID"). Public, not a secret.
    pub microsoft_app_id: Option<String>,
    /// `http://127.0.0.1:<port>` in copies built for the tests; never in a release.
    pub stand_in: Option<String>,
}

/// Opens a sign-in address in the owner's own browser.
pub trait Opener: Send + Sync + 'static {
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// The owner's default browser, through Windows' own handler for web addresses (`xdg-open`
/// elsewhere, for development), started through the supervisor with no shell.
pub struct SystemOpener {
    supervisor: Supervisor,
}

impl SystemOpener {
    pub fn new(supervisor: Supervisor) -> Self {
        Self { supervisor }
    }
}

impl Opener for SystemOpener {
    fn open(&self, address: String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let supervisor = self.supervisor.clone();
        Box::pin(async move {
            let (executable, args) = if cfg!(windows) {
                let root = std::env::var_os("SystemRoot")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| "C:\\Windows".into());
                (
                    root.join("System32").join("rundll32.exe"),
                    vec!["url.dll,FileProtocolHandler".to_owned(), address],
                )
            } else {
                let program = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                (
                    crate::programs::find_on_path(program)
                        .ok_or_else(|| format!("{program} is not installed"))?,
                    vec![address],
                )
            };
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
    pub connections: Vec<ConnectionCard>,
}

/// One connection's card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionCard {
    pub connection: Connection,
    /// This copy has an app to sign in with (8 West's, or the organization's own).
    pub has_app: bool,
    /// A sign-in waits in the owner's browser.
    pub signing_in: bool,
    /// When the service said an admin must approve Plenipo first: the link to send them.
    #[ts(optional)]
    pub admin_link: Option<String>,
    /// What went wrong with the last sign-in, in plain words.
    #[ts(optional)]
    pub problem: Option<String>,
    pub parts: Vec<PartCard>,
    /// Parts turned on, or raised to Full access, since the last sign-in: Reconnect to allow
    /// them.
    pub reconnect_for: Vec<String>,
    /// What the service granted, in its words and in plain words.
    pub granted: Vec<PermissionWords>,
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

// ---- The service ---------------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// Connection ID → its access token and when it expires (memory only).
    tokens: HashMap<String, (String, Instant)>,
    /// Connection ID → the sign-in waiting in the owner's browser: its turn, and how to stop it.
    waiting: HashMap<String, (u64, oneshot::Sender<()>)>,
    /// Connection ID → the link for the organization's admin.
    admin: HashMap<String, String>,
    /// Connection ID → what went wrong with the last sign-in.
    problem: HashMap<String, String>,
    /// Connection ID → the turn of its latest sign-in, Cancel, or Disconnect. A sign-in or a
    /// renewal that finishes after a newer one of these keeps nothing.
    turns: HashMap<String, u64>,
    next_turn: u64,
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
    authority: &'a str,
    redirect: &'a str,
    /// The permissions asked for on the sign-in page, as sent.
    scopes: &'a str,
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

    /// The app a connection signs in with: the organization's own, or 8 West's.
    pub fn app_id(&self, conn: &Connection) -> Option<String> {
        conn.own_app
            .as_ref()
            .map(|a| a.app_id.clone())
            .or_else(|| {
                self.config
                    .microsoft_app_id
                    .clone()
                    .filter(|id| plenipo_guard::connections::is_guid(id.trim()))
            })
            .filter(|_| conn.service == Service::Microsoft365)
    }

    pub fn signing_in(&self, id: &str) -> bool {
        lock(&self.state).waiting.contains_key(id)
    }

    /// Every sign-in value to hide in text: the long-lived ones in the Vault (for every kept
    /// connection, connected or not, so a value left behind is hidden too), and the access tokens
    /// in memory. (value, name)
    pub fn secrets(&self) -> Vec<(String, String)> {
        let conns = self.guard().connections().unwrap_or_default();
        let mut out: Vec<(String, String)> = conns
            .iter()
            .filter_map(|c| {
                vault::read(self.store.as_ref(), &vault_id(&c.id))
                    .ok()
                    .flatten()
                    .map(|v| (v, format!("{} sign-in", c.label())))
            })
            .collect();
        out.extend(
            lock(&self.state)
                .tokens
                .iter()
                .map(|(id, (t, _))| (t.clone(), format!("{} sign-in", name_of(id)))),
        );
        out
    }

    /// The IDs of every Vault value connections keep (for uninstalling with "delete my data"):
    /// each kept connection's, and each service's own, in case a sign-in was kept before its
    /// connection was.
    pub fn vault_ids(config: &plenipo_guard::GuardConfig) -> Vec<String> {
        let mut ids: Vec<String> = config.connections.iter().map(|c| vault_id(&c.id)).collect();
        for s in Service::ALL {
            let id = vault_id(s.id());
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    // ---- Signing in ----------------------------------------------------------------------------

    /// Start signing in to `id` with an account of `kind`: open the service's sign-in page in the
    /// owner's browser, and wait for the answer in the background. The caller has asked Guard.
    pub async fn start_sign_in(
        self: &Arc<Self>,
        id: &str,
        kind: AccountKind,
    ) -> Result<(), String> {
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        let app_id = self.app_id(&conn).ok_or_else(|| {
            format!(
                "This copy of Plenipo has no app ID for {} yet.",
                conn.label()
            )
        })?;
        let kind = if conn.own_app.is_some() {
            AccountKind::Work
        } else {
            kind
        };
        let listener = Listener::open()
            .await
            .map_err(|e| format!("Plenipo could not wait for the sign-in on this PC ({e})"))?;
        let pkce = Pkce::new();
        let redirect = format!("http://localhost:{}", listener.port);
        let authority = microsoft365::authority(kind, conn.own_app.as_ref());
        // The permissions asked for now are the ones the code is traded for, whatever the owner
        // changes while signing in.
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
        let reachable = self.http.address(&address);
        self.http
            .check(conn.service, &reachable)
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
        let scopes = scopes.join(" ");
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
                        authority: &authority,
                        redirect: &redirect,
                        scopes: &scopes,
                    };
                    this.finish_sign_in(&sign_in, &code, &pkce.verifier).await
                }
                Ok(Some(Callback::Refused { error, description })) => {
                    if microsoft365::admin_may_help(&error, &description) {
                        lock(&this.state)
                            .admin
                            .insert(id.clone(), microsoft365::admin_link(&app_id, None));
                    }
                    Err(microsoft365::refusal_words(&error, &description))
                }
            };
            let current = {
                let mut s = lock(&this.state);
                if s.waiting.get(&id).is_some_and(|(t, _)| *t == turn) {
                    s.waiting.remove(&id);
                }
                s.turn(&id) == turn
            };
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
                    if current {
                        lock(&this.state).problem.insert(id.clone(), why.clone());
                    }
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
        let form = vec![
            ("client_id".to_owned(), s.app_id.to_owned()),
            ("grant_type".to_owned(), "authorization_code".to_owned()),
            ("code".to_owned(), code.to_owned()),
            ("redirect_uri".to_owned(), s.redirect.to_owned()),
            ("code_verifier".to_owned(), verifier.to_owned()),
            ("scope".to_owned(), s.scopes.to_owned()),
        ];
        let reply = self
            .http
            .send(
                conn.service,
                reqwest::Method::POST,
                &microsoft365::token_address(s.authority),
                None,
                &[],
                Body::Form(form),
                MAX_TOKEN_ANSWER,
            )
            .await
            .map_err(|e| e.to_string())?;
        let answer = reply.json();
        if !reply.ok() {
            let error = answer["error"].as_str().unwrap_or_default();
            let description = answer["error_description"].as_str().unwrap_or_default();
            if microsoft365::admin_may_help(error, description) {
                lock(&self.state)
                    .admin
                    .insert(id.to_owned(), microsoft365::admin_link(s.app_id, None));
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
        let granted = microsoft365::granted(answer["scope"].as_str().unwrap_or_default());
        {
            let _one = lock(&self.commit);
            if lock(&self.state).turn(id) != s.turn {
                return Ok(Signed::Stopped);
            }
            let previous = vault::read(self.store.as_ref(), &vault_id(id))
                .ok()
                .flatten();
            self.keep(id, refresh, previous.as_deref())?;
            if let Some(access) = answer["access_token"].as_str() {
                let expires = answer["expires_in"].as_u64().unwrap_or(3600);
                lock(&self.state).tokens.insert(
                    id.to_owned(),
                    (
                        access.to_owned(),
                        Instant::now() + Duration::from_secs(expires),
                    ),
                );
            }
            if let Err(e) = self
                .guard()
                .connection_connected(id, Some(kind), account, &granted)
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

    /// Keep a long-lived sign-in in the Vault, and check it reads back the same. If that fails,
    /// the `previous` one goes back (Microsoft does not cancel it when it is used), or, with none,
    /// nothing is left — never a mix of the two.
    fn keep(&self, id: &str, value: &str, previous: Option<&str>) -> Result<(), String> {
        let key = vault_id(id);
        let label = self.store.label();
        let kept = vault::put(self.store.as_ref(), &key, value)
            .map_err(|e| format!("Plenipo could not keep the sign-in in {label} ({e})."))
            .and_then(|()| match vault::read(self.store.as_ref(), &key) {
                Ok(Some(v)) if v == value => Ok(()),
                _ => Err(format!(
                    "Plenipo could not keep the sign-in in {label}: it did not read back the same."
                )),
            });
        if kept.is_err() {
            let _ = match previous {
                Some(p) => vault::put(self.store.as_ref(), &key, p),
                None => vault::erase(self.store.as_ref(), &key),
            };
        }
        kept
    }

    /// Stop a sign-in waiting in the owner's browser (even one Microsoft already answered: its
    /// sign-in is not kept).
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
    /// any sign-in under way are dropped — then its sign-in leaves the Vault (with its pieces).
    /// Microsoft has no way to cancel one sign-in, so nothing is sent.
    pub fn disconnect(&self, id: &str) -> Result<(), String> {
        let marked;
        let erased;
        {
            let _one = lock(&self.commit);
            {
                let mut s = lock(&self.state);
                s.next(id);
                if let Some((_, tx)) = s.waiting.remove(id) {
                    let _ = tx.send(());
                }
                s.tokens.remove(id);
                s.admin.remove(id);
                s.problem.remove(id);
            }
            marked = self
                .guard()
                .connection_disconnected(id)
                .map(|_| ())
                .map_err(|e| e.to_string());
            erased = vault::erase(self.store.as_ref(), &vault_id(id)).map_err(|e| {
                format!(
                    "Plenipo could not remove the sign-in from {} ({e}).",
                    self.store.label()
                )
            });
        }
        self.changed();
        marked.and(erased)
    }

    // ---- Fresh access tokens --------------------------------------------------------------------

    /// A fresh access token for `id`: the one in memory while it has time left, else a new one
    /// from the long-lived sign-in (which Microsoft replaces each time; the new one replaces the
    /// old in the Vault). When Microsoft refuses, the connection needs the owner to sign in again.
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
        let kind = conn.account_kind.unwrap_or(AccountKind::Work);
        let authority = match (&conn.own_app, kind) {
            (Some(app), _) => app.tenant.clone(),
            (None, AccountKind::Personal) => "consumers".into(),
            (None, AccountKind::Work) => conn
                .account
                .as_ref()
                .and_then(|a| a.tenant.clone())
                .unwrap_or_else(|| "organizations".into()),
        };
        // Only what Microsoft already granted: asking for more fails the renewal.
        let form = vec![
            ("client_id".to_owned(), app_id),
            ("grant_type".to_owned(), "refresh_token".to_owned()),
            ("refresh_token".to_owned(), refresh.clone()),
            (
                "scope".to_owned(),
                microsoft365::refresh_scopes(&conn, kind).join(" "),
            ),
        ];
        let reply = self
            .http
            .send(
                conn.service,
                reqwest::Method::POST,
                &microsoft365::token_address(&authority),
                None,
                &[],
                Body::Form(form),
                MAX_TOKEN_ANSWER,
            )
            .await
            .map_err(|e| format!("{name}: {e}"))?;
        let answer = reply.json();
        let result = {
            let _commit = lock(&self.commit);
            // Disconnected, cancelled, or signed in again meanwhile: this renewal keeps nothing.
            if lock(&self.state).turn(id) != turn {
                return Err(format!(
                    "{name} was disconnected or signed in again meanwhile. Try again."
                ));
            }
            if reply.ok() {
                self.renewed(id, &answer, &refresh)
            } else {
                Err(self.renewal_refused(id, &answer, reply.status))
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

    /// Microsoft refused a renewal: when it no longer accepts the sign-in, forget it, and say the
    /// owner must sign in again. What the worker is told.
    fn renewal_refused(&self, id: &str, answer: &Value, status: u16) -> String {
        let name = name_of(id);
        let error = answer["error"].as_str().unwrap_or_default();
        if !matches!(
            error,
            "invalid_grant" | "interaction_required" | "consent_required"
        ) {
            return format!("{name} did not renew the sign-in ({status}).");
        }
        let reason = if error == "invalid_grant" {
            format!("{name} no longer accepts the sign-in (it expired, was removed, or the password changed).")
        } else {
            format!("{name} asks for the sign-in to be done again.")
        };
        let _ = vault::erase(self.store.as_ref(), &vault_id(id));
        lock(&self.state).tokens.remove(id);
        let _ = self.guard().connection_needs_sign_in(id, &reason);
        format!("{name} needs the owner to sign in again (Settings → Connections).")
    }

    /// One Graph call for connection `id`, with a fresh access token (tried once more with a new
    /// one when Microsoft says the token is no good).
    pub(crate) async fn graph_call(
        &self,
        id: &str,
        method: reqwest::Method,
        url: &str,
        headers: &[(&'static str, &str)],
        body: Body,
        limit: usize,
    ) -> Result<Reply, String> {
        let token = self.access_token(id).await?;
        let reply = self
            .http
            .send(
                Service::Microsoft365,
                method.clone(),
                url,
                Some(&token),
                headers,
                body.clone(),
                limit,
            )
            .await
            .map_err(|e| match e {
                HttpError::TooBig => "Microsoft 365's answer was too big to use here.".to_owned(),
                other => other.to_string(),
            })?;
        if reply.status != 401 {
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
        self.http
            .send(
                Service::Microsoft365,
                method,
                url,
                Some(&token),
                headers,
                body,
                limit,
            )
            .await
            .map_err(|e| e.to_string())
    }

    // ---- Settings -------------------------------------------------------------------------------

    /// What Settings → Connections shows.
    pub fn page(&self, vault_available: bool) -> Result<ConnectionsPage, String> {
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
        let state = lock(&self.state);
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
                    connections: conns
                        .into_iter()
                        .map(|c| {
                            let parts = c
                                .service
                                .parts()
                                .iter()
                                .map(|p| {
                                    let (reads, changes) = microsoft365::part_words(*p);
                                    let level = c.parts.get(p).copied().unwrap_or_default();
                                    PartCard {
                                        part: *p,
                                        label: p.label().into(),
                                        level,
                                        available: c.service.has_part(*p, c.account_kind),
                                        reads: reads.into(),
                                        changes: changes.into(),
                                        needs_admin: microsoft365::needs_admin_always(
                                            *p,
                                            PartLevel::ReadOnly,
                                        ),
                                    }
                                })
                                .collect();
                            let reconnect_for = if c.state == ConnectionState::Connected {
                                microsoft365::parts_to_reconnect(&c)
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
                                    words: microsoft365::permission_words(g).into(),
                                })
                                .collect();
                            ConnectionCard {
                                has_app: self.app_id(&c).is_some(),
                                signing_in: state.waiting.contains_key(&c.id),
                                admin_link: state.admin.get(&c.id).cloned(),
                                problem: state.problem.get(&c.id).cloned(),
                                parts,
                                reconnect_for,
                                granted,
                                connection: c,
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        Ok(ConnectionsPage {
            services,
            people,
            send_switch_on: config.switches.send_without_asking,
            vault_available,
            vault_label: self.store.label().to_owned(),
        })
    }
}

// ---- Graph, for one connection's tools ----------------------------------------------------------

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
            .graph_call(&self.id, method, url, &headers, body, limit)
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
