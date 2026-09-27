//! One tab of Plenipo's browser, for one worker's step (Phase 10, ADR-020).
//!
//! The tab keeps what it learns from the browser's events (its address, loads, crashes, the
//! helper's world), and checks every page request itself (the network gate, ADR-020 section 4,
//! ADR-035):
//! - a page opened in the tab must pass the owner's website lists (blocked websites never
//!   open; a website on neither list opens only once the owner approved it for this step);
//! - data a page sends right after a worker's click or key press (a form, a message, a beacon:
//!   any `Document`, `XHR`, `Fetch`, `Ping`, or `Other` request that is not a plain GET, HEAD,
//!   or OPTIONS read) is held until Plenipo decides: the broker asks the owner, then lets it
//!   go or stops it;
//! - data a page tries to send by itself, with no worker action (a form it submits on its own,
//!   a script's POST on a timer, a beacon), is stopped, and the worker is told so with its
//!   next result. It is stopped rather than held because nobody can decide it: an approval
//!   belongs to a worker's tool call, and a request held between calls would wait for an action
//!   it had nothing to do with. The worker can act on the page (click or press) and the owner
//!   is asked then;
//! - WebSocket frames cannot be seen or held, so the tab only notes that the page has a live
//!   connection (`Network.webSocketCreated`, forgotten on the next page), and the broker asks
//!   the owner before a click, Enter, or Space on such a page ([`Tab::has_websocket`]);
//! - a file a page tries to save (a download) is refused by the browser itself, for every tab,
//!   from its start (ADR-037, [`super::Browser`]); the browser's event names only the frame that
//!   started it, so the tab keeps the frames of its page, and [`Tabs`] finds the tab to tell;
//! - a page never gets a second tab (ADR-036). The browser attaches to every new tab paused,
//!   before any of it runs ([`super::Browser`] asks for that), and [`Tabs`] closes one that a
//!   worker's page opened (a link with `target="_blank"`, `window.open`, a form aimed at a new
//!   window). When the worker's action opened it and its address passes the website check, the
//!   worker's own tab goes there instead; either way the worker is told with its next result.
//!
//! When the owner takes control or stops it, the tab stops checking (the owner browses freely)
//! and its sign says so.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use base64::Engine as _;
use plenipo_guard::websites::{self, SiteVerdict};
use plenipo_guard::{Site, WebsiteRules};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};
use tokio::time::Instant;

use super::cdp::{Cdp, Event};
use super::classify::ElementFacts;

/// Plenipo's helper script, run in every page's isolated world.
pub const PAGE_JS: &str = include_str!("page.js");
/// The isolated world's name (the binding exists only there).
const WORLD: &str = "plenipo";
const BINDING: &str = "plenipoControl";
/// How long a tab waits for the address of a new tab its page asked for (`Page.windowOpen`, on
/// the tab's own session) once the browser attached to that tab (ADR-036): the two arrive a
/// moment apart, on different channels.
const OPENING_WAIT: Duration = Duration::from_millis(500);
/// How long an ask for a new tab that never became one is remembered.
const OPENING_KEPT: Duration = Duration::from_secs(5);
/// A new tab the page's script can reach is closed once it has made no request for this long
/// after it was let run (ADR-036, [`close_new_tab`]), and at the latest after `NEW_TAB_LONGEST`.
const NEW_TAB_QUIET: Duration = Duration::from_millis(150);
const NEW_TAB_LONGEST: Duration = Duration::from_secs(2);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Time limits for a tab.
#[derive(Debug, Clone, Copy)]
pub struct TabLimits {
    /// Longest a page may take to open.
    pub navigation: Duration,
    /// How long Plenipo watches the page after a worker's action (for data it sends).
    pub settle: Duration,
    /// Longest one command to the browser may take.
    pub command: Duration,
}

impl Default for TabLimits {
    fn default() -> Self {
        Self {
            navigation: Duration::from_secs(30),
            settle: Duration::from_millis(1200),
            command: Duration::from_secs(20),
        }
    }
}

/// Who has the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// A worker uses it; every request is checked.
    #[default]
    Worker,
    /// The owner took control; the worker stopped.
    Owner,
    /// The owner stopped all control.
    Stopped,
    /// The owner is solving a check that a person is using the site (a CAPTCHA, ADR-023); the
    /// worker waits and gets the tab back when the owner says it is done.
    Handed,
}

impl Mode {
    fn sign(self) -> &'static str {
        match self {
            Self::Worker => "active",
            Self::Owner => "owner",
            Self::Stopped => "stopped",
            Self::Handed => "handed",
        }
    }
}

/// Something the page asked the owner's side to do (from the sign or the owner's own input).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// The owner pressed "Take over" on the page.
    TakeOver,
    /// The owner clicked or typed in the page while the worker had it.
    OwnerInput,
}

/// Where a tab reports signals: its target ID and the signal.
pub type Signals = Arc<dyn Fn(&str, Signal) + Send + Sync>;

/// Data a page tried to send, held until Plenipo decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub request_id: String,
    pub method: String,
    pub url: String,
    /// The website it goes to (`host` or `host:port`).
    pub site: String,
    /// `Document` (a form), `XHR`/`Fetch` (a page script), `Ping` (a beacon), or `Other`.
    pub kind: String,
}

/// The websites a tab may open: the owner's lists, and those the owner approved for this step.
#[derive(Debug, Clone, Default)]
pub struct SitePolicy {
    pub rules: WebsiteRules,
    pub approved: HashSet<String>,
}

/// How many times a worker may try a CAPTCHA before Plenipo hands it to the owner (ADR-029).
pub const CAPTCHA_TRIES: u32 = 3;
/// How long Plenipo watches a CAPTCHA after a worker's try, for it to pass, open a puzzle, or go
/// away (ADR-032).
pub const CAPTCHA_VERDICT_WAIT: Duration = Duration::from_secs(4);

/// The page's CAPTCHA in one look (the helper's `captchaState`, ADR-032).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CaptchaState {
    /// A check shows on the page.
    pub present: bool,
    /// Who makes it: "reCAPTCHA", "hCaptcha", "Cloudflare Turnstile", "Arkose", or "CAPTCHA".
    pub provider: String,
    /// It is passed: its provider wrote the answer into the page.
    pub solved: bool,
    /// It opened a puzzle (pictures to pick) in its own frame.
    pub challenge: bool,
    /// Only a badge that works by itself (invisible reCAPTCHA): nothing to click.
    pub invisible: bool,
    /// The reference of its checkbox, when it has one a worker can click.
    pub checkbox: Option<String>,
}

#[derive(Default)]
struct State {
    main_frame: String,
    /// The page's frames now (the main frame and those inside it), by the browser's frame ID:
    /// a download the browser refused names only the frame that started it (ADR-037).
    frames: HashSet<String>,
    /// The helper's execution context in the main frame's document.
    world: Option<i64>,
    url: String,
    loads: u64,
    navigations: u64,
    loading: bool,
    crashed: bool,
    gone: bool,
    acting: bool,
    held: Vec<Held>,
    /// The page's live connections (WebSockets) open now, by the browser's request ID. The gate
    /// cannot see what goes through them, so the broker asks before acting on such a page
    /// (ADR-035).
    sockets: HashSet<String>,
    /// Tries a worker has made at this page's CAPTCHA (ADR-029); cleared when the page no longer
    /// shows one, or shows it passed (ADR-032).
    captcha_attempts: u32,
    /// Where Plenipo last left the mouse pointer in the page (the next click glides from there).
    pointer: Option<(f64, f64)>,
    /// What the worker should hear with its next result (a page Plenipo stopped, …).
    notes: Vec<String>,
    /// New tabs the page asked the browser for (`Page.windowOpen`), oldest first, until the
    /// browser attaches to each (ADR-036): the paused new tab has no address of its own yet.
    opening: Vec<Opening>,
    policy: SitePolicy,
    mode: Mode,
    worker: String,
    /// The registered script that draws the sign on each new page.
    sign_script: Option<String>,
}

impl State {
    /// The page has a live connection (a WebSocket) open now.
    fn has_websocket(&self) -> bool {
        !self.sockets.is_empty()
    }

    /// The main frame moved to a new page: what belonged to the old one is forgotten (the new
    /// page's frames attach after it).
    fn page_changed(&mut self) {
        self.world = None;
        self.sockets.clear();
        self.frames.clear();
        self.frames.insert(self.main_frame.clone());
    }
}

/// A new tab the page asked for, which the browser has not attached to yet (ADR-036).
#[derive(Debug, Clone)]
struct Opening {
    url: String,
    at: Instant,
}

struct Shared {
    /// The tab's target and session in the browser, so a new tab its page opens is traced back
    /// to it, and its own page can be sent where that tab was going (ADR-036).
    target: String,
    session: String,
    limits: TabLimits,
    state: Mutex<State>,
    changed: Notify,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    fn note(&self, note: String) {
        let mut s = self.state();
        if !s.notes.contains(&note) {
            s.notes.push(note);
        }
    }

    /// The page asked the browser for a new tab (`Page.windowOpen`): keep its address for when
    /// the browser attaches to that tab (ADR-036).
    fn opening(&self, url: &str) {
        self.state().opening.push(Opening {
            url: url.to_owned(),
            at: Instant::now(),
        });
    }

    /// The address of the oldest new tab the page asked for and the browser has not attached to
    /// yet, waiting up to `wait` for the ask to arrive. Asks that never became a tab (the
    /// browser's own pop-up rules stopped one, say) are forgotten after a while.
    async fn take_opening(&self, wait: Duration) -> Option<String> {
        let deadline = Instant::now() + wait;
        loop {
            let notified = self.changed.notified();
            {
                let mut s = self.state();
                s.opening.retain(|o| o.at.elapsed() < OPENING_KEPT);
                if !s.opening.is_empty() {
                    return Some(s.opening.remove(0).url);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            let _ = tokio::time::timeout_at(deadline, notified).await;
        }
    }
}

/// What a worker's action left: data held for a decision, and notes.
#[derive(Debug, Clone, Default)]
pub struct Settled {
    pub held: Vec<Held>,
    pub navigated: bool,
    pub timed_out: bool,
}

/// How opening a page went.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    pub url: String,
    pub title: String,
    pub timed_out: bool,
}

/// One browser tab under Plenipo's control.
pub struct Tab {
    cdp: Cdp,
    pub target_id: String,
    session: String,
    shared: Arc<Shared>,
    limits: TabLimits,
    /// One action at a time.
    busy: tokio::sync::Mutex<()>,
}

impl Tab {
    /// Open a new tab for `worker`, checked against `policy`.
    pub async fn open(
        cdp: Cdp,
        limits: TabLimits,
        worker: &str,
        policy: SitePolicy,
        signals: Signals,
    ) -> Result<Self, String> {
        let t = limits.command;
        let target = cdp
            .call(
                None,
                "Target.createTarget",
                json!({ "url": "about:blank" }),
                t,
            )
            .await?;
        let target_id = target["targetId"]
            .as_str()
            .ok_or("the browser opened no tab")?
            .to_owned();
        let attached = cdp
            .call(
                None,
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                t,
            )
            .await?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or("the browser gave no session for the tab")?
            .to_owned();
        let events = cdp.listen(&session);
        let shared = Arc::new(Shared {
            target: target_id.clone(),
            session: session.clone(),
            limits,
            state: Mutex::new(State {
                policy,
                worker: worker.to_owned(),
                ..State::default()
            }),
            changed: Notify::new(),
        });
        tokio::spawn(event_loop(
            cdp.clone(),
            session.clone(),
            target_id.clone(),
            Arc::clone(&shared),
            events,
            signals,
        ));
        let tab = Self {
            cdp,
            target_id,
            session,
            shared,
            limits,
            busy: tokio::sync::Mutex::new(()),
        };
        tab.set_up().await.inspect_err(|_| {
            let cdp = tab.cdp.clone();
            let id = tab.target_id.clone();
            tokio::spawn(async move {
                let _ = cdp
                    .call(
                        None,
                        "Target.closeTarget",
                        json!({ "targetId": id }),
                        Duration::from_secs(5),
                    )
                    .await;
            });
        })?;
        Ok(tab)
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.cdp
            .call(Some(&self.session), method, params, self.limits.command)
            .await
    }

    async fn set_up(&self) -> Result<(), String> {
        self.call("Page.enable", json!({})).await?;
        self.call("Runtime.enable", json!({})).await?;
        // Network events tell the tab when the page opens a live connection (ADR-035).
        self.call("Network.enable", json!({})).await?;
        self.call(
            "Runtime.addBinding",
            json!({ "name": BINDING, "executionContextName": WORLD }),
        )
        .await?;
        self.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": PAGE_JS, "worldName": WORLD }),
        )
        .await?;
        self.watch_requests().await?;
        let tree = self.call("Page.getFrameTree", json!({})).await?;
        let frame = tree["frameTree"]["frame"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        {
            let mut s = self.shared.state();
            s.frames.insert(frame.clone());
            s.main_frame = frame;
        }
        self.show_sign().await
    }

    /// Hold every page request for Plenipo's checks (while a worker has the tab): pages and
    /// forms (`Document`), scripts' requests (`XHR`, `Fetch`), beacons (`Ping`, which is how
    /// `navigator.sendBeacon` shows), and whatever else (`Other`). The names are the DevTools
    /// protocol's `Network.ResourceType` values, as `Fetch.enable` takes them; the browser's
    /// filter accepts only some of them (checked on Chrome 141: `EventSource` and `WebSocket`
    /// are refused, and `Fetch.enable` fails outright, so the tab does not open). An
    /// `EventSource` only receives (a GET stream), and WebSockets are covered by
    /// [`Tab::has_websocket`] (ADR-035).
    async fn watch_requests(&self) -> Result<(), String> {
        let patterns: Vec<Value> = ["Document", "XHR", "Fetch", "Ping", "Other"]
            .iter()
            .map(|t| json!({ "urlPattern": "*", "resourceType": t, "requestStage": "Request" }))
            .collect();
        self.call("Fetch.enable", json!({ "patterns": patterns }))
            .await
            .map(|_| ())
    }

    /// The owner solved the check (or refused to): the worker has the tab again, with every
    /// request checked. Nothing changes when the owner took over or stopped in the meantime.
    pub async fn take_back(&self) {
        {
            let mut s = self.state();
            if s.mode != Mode::Handed {
                return;
            }
            s.mode = Mode::Worker;
        }
        self.shared.changed.notify_waiters();
        let _ = self.watch_requests().await;
        let _ = self.show_sign().await;
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state()
    }

    /// The tab is gone (closed, or its page or the browser crashed).
    pub fn broken(&self) -> Option<&'static str> {
        let s = self.state();
        if s.crashed {
            Some("the page crashed")
        } else if s.gone || self.cdp.is_closed() {
            Some("the tab was closed")
        } else {
            None
        }
    }

    pub fn mode(&self) -> Mode {
        self.state().mode
    }

    pub fn url(&self) -> String {
        self.state().url.clone()
    }

    /// The page has a live connection (a WebSocket) open now. The network gate cannot see what
    /// the page sends through it, so the broker asks the owner before a click or key press that
    /// may send (ADR-035).
    pub fn has_websocket(&self) -> bool {
        self.state().has_websocket()
    }

    /// Replace the website lists (the owner's settings as they are now).
    pub fn set_rules(&self, rules: WebsiteRules) {
        self.state().policy.rules = rules;
    }

    /// The owner approved opening `site` (`host` or `host:port`) for this step.
    pub fn approve_site(&self, site: &str) {
        self.state().policy.approved.insert(site.to_owned());
    }

    pub fn site_approved(&self, site: &str) -> bool {
        self.state().policy.approved.contains(site)
    }

    /// Notes gathered since the last result, taken.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut self.state().notes)
    }

    /// How many tries a worker has made at this page's CAPTCHA (ADR-029).
    pub fn captcha_attempts(&self) -> u32 {
        self.state().captcha_attempts
    }

    /// Count one try at the page's CAPTCHA (a submitted answer, ADR-029): the tries used so far.
    pub fn count_captcha_try(&self) -> u32 {
        let mut s = self.state();
        s.captcha_attempts += 1;
        s.captcha_attempts
    }

    /// The CAPTCHA is gone or passed (the page moved on): tries start over (ADR-029, ADR-032).
    pub fn clear_captcha_attempts(&self) {
        self.state().captcha_attempts = 0;
    }

    /// The page's CAPTCHA in one look (ADR-032).
    pub async fn captcha_state(&self) -> Result<CaptchaState, String> {
        let v = self.helper("__plenipo.captchaState()").await?;
        serde_json::from_value(v).map_err(|e| format!("the page's helper answered oddly: {e}"))
    }

    /// After a worker's try at the CAPTCHA: watch the check for up to `wait` until it is passed,
    /// opens a puzzle, or is gone, and say how it stands (ADR-032). Passed or gone, the tries
    /// start over. `None` when the page cannot be read (it is changing).
    pub async fn captcha_verdict(&self, wait: Duration) -> Option<CaptchaState> {
        let deadline = Instant::now() + wait;
        loop {
            let state = self.captcha_state().await.ok()?;
            let decided = state.solved || state.challenge || !state.present;
            if decided || Instant::now() >= deadline || self.mode() != Mode::Worker {
                if state.solved || !state.present {
                    self.clear_captcha_attempts();
                }
                return Some(state);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    // ---- The helper's world ----------------------------------------------------------------

    async fn world(&self) -> Result<i64, String> {
        if let Some(id) = self.state().world {
            return Ok(id);
        }
        let frame = self.state().main_frame.clone();
        let made = self
            .call(
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": WORLD }),
            )
            .await?;
        let id = made["executionContextId"]
            .as_i64()
            .ok_or("the browser made no helper world")?;
        self.evaluate_in(id, PAGE_JS).await?;
        let (mode, worker) = {
            let mut s = self.state();
            s.world = Some(id);
            (s.mode, s.worker.clone())
        };
        self.evaluate_in(id, &sign_call(mode, &worker)).await?;
        Ok(id)
    }

    async fn evaluate_in(&self, context: i64, expression: &str) -> Result<Value, String> {
        self.evaluate_within(context, expression, self.limits.command)
            .await
    }

    async fn evaluate_within(
        &self,
        context: i64,
        expression: &str,
        timeout: Duration,
    ) -> Result<Value, String> {
        let r = self
            .cdp
            .call(
                Some(&self.session),
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "contextId": context,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                timeout,
            )
            .await?;
        if let Some(e) = r.get("exceptionDetails") {
            let text = e["exception"]["description"]
                .as_str()
                .or_else(|| e["text"].as_str())
                .unwrap_or("the page's helper failed");
            return Err(text.lines().next().unwrap_or(text).to_owned());
        }
        Ok(r["result"]["value"].clone())
    }

    /// Run the helper's `expression` in the current page (once more after a page change).
    async fn helper(&self, expression: &str) -> Result<Value, String> {
        let id = self.world().await?;
        match self.evaluate_in(id, expression).await {
            Err(e) if e.contains("context") || e.contains("__plenipo") => {
                {
                    let mut s = self.state();
                    if s.world == Some(id) {
                        s.world = None;
                    }
                }
                let id = self.world().await?;
                self.evaluate_in(id, expression).await
            }
            other => other,
        }
    }

    /// A small helper call that may be skipped: while a page's request is held, the browser
    /// does not run scripts in it, so this gives up quickly.
    async fn quick(&self, expression: &str) {
        let world = self.state().world;
        if let Some(id) = world {
            let _ = self
                .evaluate_within(id, expression, Duration::from_millis(1500))
                .await;
        }
    }

    // ---- The sign ----------------------------------------------------------------------------

    async fn show_sign(&self) -> Result<(), String> {
        let (mode, worker, old) = {
            let s = self.state();
            (s.mode, s.worker.clone(), s.sign_script.clone())
        };
        if let Some(old) = old {
            let _ = self
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    json!({ "identifier": old }),
                )
                .await;
        }
        let added = self
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": sign_call(mode, &worker), "worldName": WORLD }),
            )
            .await?;
        self.state().sign_script = added["identifier"].as_str().map(str::to_owned);
        if self.state().world.is_some() {
            // A page may be busy (a request held, a page loading): do not wait on it.
            self.quick(&sign_call(mode, &worker)).await;
            Ok(())
        } else {
            self.helper(&sign_call(mode, &worker)).await.map(|_| ())
        }
    }

    /// Hand the tab to the owner (`Owner`, or `Handed` to solve a check) or stop it (`Stopped`):
    /// data held is stopped, pages are no longer checked, and the sign says so. The tab stays
    /// open.
    pub async fn release_to(&self, mode: Mode) {
        let held = {
            let mut s = self.state();
            if s.mode != Mode::Worker {
                s.mode = mode;
                Vec::new()
            } else {
                s.mode = mode;
                s.acting = false;
                std::mem::take(&mut s.held)
            }
        };
        self.shared.changed.notify_waiters();
        for h in held {
            let _ = self
                .call(
                    "Fetch.failRequest",
                    json!({ "requestId": h.request_id, "errorReason": "BlockedByClient" }),
                )
                .await;
        }
        let _ = self.call("Fetch.disable", json!({})).await;
        let _ = self.show_sign().await;
        if matches!(mode, Mode::Owner | Mode::Handed) {
            let _ = self.call("Page.bringToFront", json!({})).await;
        }
    }

    // ---- Reading ---------------------------------------------------------------------------

    /// The page in words: address, title, text, numbered controls, CAPTCHA (and how it stands).
    pub async fn read(&self, max_chars: usize, max_controls: usize) -> Result<Value, String> {
        let _busy = self.busy.lock().await;
        let page = self
            .helper(&format!("__plenipo.read({max_chars}, {max_controls})"))
            .await?;
        if page["captcha"] != true || page["captchaInfo"]["solved"] == true {
            self.clear_captcha_attempts();
        }
        Ok(page)
    }

    /// What is at a numbered control now (scrolled into view).
    pub async fn facts(&self, reference: &str) -> Result<ElementFacts, String> {
        let v = self
            .helper(&format!("__plenipo.facts({})", json!(reference)))
            .await?;
        serde_json::from_value(v).map_err(|e| format!("the page's helper answered oddly: {e}"))
    }

    /// The control that has the keyboard focus.
    pub async fn focused(&self) -> Result<ElementFacts, String> {
        let v = self.helper("__plenipo.focused()").await?;
        serde_json::from_value(v).map_err(|e| format!("the page's helper answered oddly: {e}"))
    }

    /// The page as a JPEG image (without the sign).
    pub async fn screenshot(&self) -> Result<Vec<u8>, String> {
        self.quick("__plenipo.signHidden(true)").await;
        let shot = self
            .call(
                "Page.captureScreenshot",
                json!({ "format": "jpeg", "quality": 70, "fromSurface": true }),
            )
            .await;
        self.quick("__plenipo.signHidden(false)").await;
        let data = shot?["data"].as_str().unwrap_or_default().to_owned();
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("the screenshot could not be read: {e}"))
    }

    /// The page's address and title.
    pub async fn where_now(&self) -> (String, String) {
        match self
            .helper("({ url: location.href, title: document.title })")
            .await
        {
            Ok(v) => (
                v["url"].as_str().unwrap_or_default().to_owned(),
                v["title"].as_str().unwrap_or_default().to_owned(),
            ),
            Err(_) => (self.url(), String::new()),
        }
    }

    // ---- Opening pages ---------------------------------------------------------------------

    async fn wait(&self, deadline: Instant, done: impl Fn(&State) -> bool) -> bool {
        loop {
            let notified = self.shared.changed.notified();
            {
                let s = self.state();
                if done(&s) || s.crashed || s.gone {
                    return done(&s);
                }
            }
            if Instant::now() >= deadline {
                return false;
            }
            let _ = tokio::time::timeout_at(deadline, notified).await;
        }
    }

    /// Open `url` in the tab (already allowed), waiting for it to load.
    pub async fn navigate(&self, url: &str, timeout: Duration) -> Result<Opened, String> {
        let _busy = self.busy.lock().await;
        let before = self.state().loads;
        let deadline = Instant::now() + timeout;
        let answer = self
            .cdp
            .call(
                Some(&self.session),
                "Page.navigate",
                json!({ "url": url }),
                timeout,
            )
            .await;
        let answer = match answer {
            Ok(a) => a,
            Err(e) if e.contains("did not answer") => {
                let _ = self.call("Page.stopLoading", json!({})).await;
                return Ok(Opened {
                    url: url.to_owned(),
                    timed_out: true,
                    ..Opened::default()
                });
            }
            Err(e) => return Err(e),
        };
        if let Some(error) = answer["errorText"].as_str().filter(|e| !e.is_empty()) {
            let notes = self.take_notes();
            return Err(if notes.is_empty() {
                format!("the page did not open ({error})")
            } else {
                notes.join(" ")
            });
        }
        let timed_out =
            answer.get("loaderId").is_some() && !self.wait(deadline, |s| s.loads > before).await;
        if timed_out {
            let _ = self.call("Page.stopLoading", json!({})).await;
        }
        let (url, title) = self.where_now().await;
        Ok(Opened {
            url,
            title,
            timed_out,
        })
    }

    /// Go back one page.
    pub async fn back(&self, timeout: Duration) -> Result<Opened, String> {
        let history = self.call("Page.getNavigationHistory", json!({})).await?;
        let current = history["currentIndex"].as_i64().unwrap_or(0);
        if current <= 0 {
            return Err("there is no earlier page in this tab".into());
        }
        let entry = history["entries"][(current - 1) as usize]["id"].clone();
        let _busy = self.busy.lock().await;
        let before = self.state().navigations;
        self.call("Page.navigateToHistoryEntry", json!({ "entryId": entry }))
            .await?;
        let deadline = Instant::now() + timeout;
        let timed_out = !self
            .wait(deadline, |s| s.navigations > before && !s.loading)
            .await;
        let (url, title) = self.where_now().await;
        Ok(Opened {
            url,
            title,
            timed_out,
        })
    }

    // ---- Acting ----------------------------------------------------------------------------

    /// Run a worker's action with the network check on, then watch the page: data it sends is
    /// held; a page it opens is waited for.
    async fn acting<F>(&self, action: F) -> Result<Settled, String>
    where
        F: std::future::Future<Output = Result<(), String>>,
    {
        let before = {
            let mut s = self.state();
            s.acting = true;
            (s.navigations, s.loads)
        };
        // The page helper ignores input while Plenipo acts (it is not the owner's).
        let _ = self.helper("__plenipo.acting(true)").await;
        let result = action.await;
        let settled = self.settle(before).await;
        self.state().acting = false;
        if settled.held.is_empty() {
            self.quick("__plenipo.acting(false)").await;
        }
        result.map(|()| settled)
    }

    /// Watch the page after an action: stop at data held for a decision; wait for a page the
    /// action opened to load; otherwise stop once the page has been quiet for a moment.
    async fn settle(&self, (navigations, loads): (u64, u64)) -> Settled {
        let start = Instant::now();
        tokio::time::sleep(Duration::from_millis(250)).await;
        let quiet = start + self.limits.settle;
        let long = start + self.limits.navigation;
        loop {
            let notified = self.shared.changed.notified();
            let (held, navigating, done) = {
                let s = self.state();
                // A page opened and not loaded yet (or one still loading).
                let navigating = s.loading || (s.navigations > navigations && s.loads == loads);
                (
                    !s.held.is_empty(),
                    navigating,
                    s.crashed || s.gone || s.mode != Mode::Worker,
                )
            };
            let now = Instant::now();
            if held || done || (!navigating && now >= quiet) || now >= long {
                break;
            }
            let _ = tokio::time::timeout(Duration::from_millis(100), notified).await;
        }
        let mut s = self.state();
        Settled {
            navigated: s.navigations > navigations,
            timed_out: s.loading && Instant::now() >= long,
            held: std::mem::take(&mut s.held),
        }
    }

    async fn mouse(&self, kind: &str, x: f64, y: f64, count: u32) -> Result<(), String> {
        let mut params = json!({ "type": kind, "x": x, "y": y });
        if kind != "mouseMoved" {
            params["button"] = json!("left");
            params["clickCount"] = json!(count);
        }
        self.call("Input.dispatchMouseEvent", params)
            .await
            .map(|_| ())
    }

    /// Click a control where its facts say (its middle; a CAPTCHA widget's checkbox). The
    /// pointer glides there first and the button is held a moment, as in a person's click
    /// (ADR-032).
    pub async fn click(&self, facts: &ElementFacts) -> Result<Settled, String> {
        let _busy = self.busy.lock().await;
        let (x, y) = (facts.x, facts.y);
        let from = self.state().pointer;
        let settled = self
            .acting(async {
                self.glide(from, (x, y)).await?;
                self.mouse("mousePressed", x, y, 1).await?;
                tokio::time::sleep(Duration::from_millis(70)).await;
                self.mouse("mouseReleased", x, y, 1).await
            })
            .await;
        self.state().pointer = Some((x, y));
        settled
    }

    /// Move the pointer to `to` in a dozen steps along a gently bowed path, slow at both ends,
    /// from where it last was (or from a little below and left of the target).
    async fn glide(&self, from: Option<(f64, f64)>, to: (f64, f64)) -> Result<(), String> {
        let (x1, y1) = to;
        let (x0, y0) = from.unwrap_or(((x1 - 160.0).max(0.0), y1 + 90.0));
        let steps: u8 = 12;
        for i in 1..=steps {
            let t = f64::from(i) / f64::from(steps);
            let eased = t * t * (3.0 - 2.0 * t);
            let bow = (std::f64::consts::PI * t).sin() * 6.0;
            let x = x0 + (x1 - x0) * eased + bow;
            let y = y0 + (y1 - y0) * eased - bow;
            self.mouse("mouseMoved", x, y, 0).await?;
            tokio::time::sleep(Duration::from_millis(14)).await;
        }
        self.mouse("mouseMoved", x1, y1, 0).await
    }

    /// Type `text` into a control, replacing what it holds; then press Enter when `enter`.
    pub async fn type_text(
        &self,
        reference: &str,
        text: &str,
        enter: bool,
    ) -> Result<Settled, String> {
        let _busy = self.busy.lock().await;
        let focused = self
            .helper(&format!("__plenipo.prepareTyping({})", json!(reference)))
            .await?;
        if focused != json!(true) {
            return Err("that control cannot take typing (it did not get the focus)".into());
        }
        self.acting(async {
            self.call("Input.insertText", json!({ "text": text }))
                .await?;
            if enter {
                self.key("Enter").await?;
            }
            Ok(())
        })
        .await
    }

    async fn key(&self, key: &str) -> Result<(), String> {
        let (code, vk, text) = key_codes(key).ok_or_else(|| format!("unknown key {key}"))?;
        let mut down = json!({ "type": if text.is_empty() { "rawKeyDown" } else { "keyDown" },
            "key": key, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk });
        if !text.is_empty() {
            down["text"] = json!(text);
            down["unmodifiedText"] = json!(text);
        }
        self.call("Input.dispatchKeyEvent", down).await?;
        self.call(
            "Input.dispatchKeyEvent",
            json!({ "type": "keyUp", "key": key, "code": code,
                    "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk }),
        )
        .await
        .map(|_| ())
    }

    /// Press one key.
    pub async fn press(&self, key: &str) -> Result<Settled, String> {
        let _busy = self.busy.lock().await;
        self.acting(self.key(key)).await
    }

    /// Choose an option in a list.
    pub async fn choose(&self, reference: &str, option: &str) -> Result<(Value, Settled), String> {
        let _busy = self.busy.lock().await;
        let mut answer = Value::Null;
        let settled = self
            .acting(async {
                answer = self
                    .helper(&format!(
                        "__plenipo.choose({}, {})",
                        json!(reference),
                        json!(option)
                    ))
                    .await?;
                Ok(())
            })
            .await?;
        Ok((answer, settled))
    }

    /// Scroll the page by `dy` CSS pixels.
    pub async fn scroll(&self, dy: i64) -> Result<Value, String> {
        let _busy = self.busy.lock().await;
        self.helper(&format!("__plenipo.scroll({dy})")).await
    }

    /// Let data held after an action go (`true`) or stop it; then watch the page again.
    pub async fn release(&self, held: &[Held], go: bool) -> Settled {
        let before = {
            let s = self.state();
            (s.navigations, s.loads)
        };
        for h in held {
            let _ = if go {
                self.call(
                    "Fetch.continueRequest",
                    json!({ "requestId": h.request_id }),
                )
                .await
            } else {
                self.call(
                    "Fetch.failRequest",
                    json!({ "requestId": h.request_id, "errorReason": "BlockedByClient" }),
                )
                .await
            };
        }
        if !go {
            return Settled::default();
        }
        // What was let go may open a page (a form's answer): watch like an action.
        self.state().acting = true;
        let settled = self.settle(before).await;
        self.state().acting = false;
        settled
    }

    /// Close the tab.
    pub async fn close(&self) {
        let _ = self
            .cdp
            .call(
                None,
                "Target.closeTarget",
                json!({ "targetId": self.target_id }),
                Duration::from_secs(5),
            )
            .await;
        self.cdp.forget(&self.session);
    }
}

/// The workers' tabs open now, so the browser's own events reach the worker they concern. A
/// download the browser refused (`Browser.downloadWillBegin` under "deny", ADR-037) names only
/// the frame that started it; this finds the tab that frame is in. Cheap to clone; clones share
/// it.
#[derive(Clone, Default)]
pub struct Tabs {
    tabs: Arc<Mutex<Vec<Weak<Shared>>>>,
}

impl Tabs {
    /// Route the browser's own events to `tab` from now on (until it is gone).
    pub fn watch(&self, tab: &Tab) {
        self.add(&tab.shared);
    }

    fn add(&self, shared: &Arc<Shared>) {
        let mut tabs = lock(&self.tabs);
        tabs.retain(|t| t.strong_count() > 0);
        tabs.push(Arc::downgrade(shared));
    }

    /// A page tried to save a file and the browser refused (ADR-037): the worker whose tab holds
    /// the frame that started it hears so with its next result. `false` when no worker's tab
    /// has that frame (a tab the owner opened, say).
    pub fn download_refused(&self, params: &Value) -> bool {
        let frame = params["frameId"].as_str().unwrap_or_default();
        let filename = params["suggestedFilename"].as_str().unwrap_or_default();
        let found = lock(&self.tabs)
            .iter()
            .filter_map(Weak::upgrade)
            .find(|t| t.state().frames.contains(frame));
        let Some(tab) = found else {
            return false;
        };
        tab.note(download_note(filename));
        true
    }

    /// The worker's tab whose target is `target`, if any.
    fn find(&self, target: &str) -> Option<Arc<Shared>> {
        lock(&self.tabs)
            .iter()
            .filter_map(Weak::upgrade)
            .find(|t| t.target == target)
    }

    /// The browser attached to a new target, paused before any of it runs
    /// (`Target.attachedToTarget`, ADR-036). A new tab a worker's page opened never runs as a
    /// tab: it is closed, the worker is told, and when the worker's action opened it and its
    /// address passes the website check, the worker's own tab goes there instead. Everything
    /// else is let run: Plenipo's own new tabs (no opener), tabs of the owner's own, and new
    /// tabs of a tab the owner has (taken over, stopped, or handed a CAPTCHA).
    pub async fn target_attached(&self, cdp: &Cdp, params: &Value) {
        let info = &params["targetInfo"];
        let session = params["sessionId"].as_str().unwrap_or_default();
        let target = info["targetId"].as_str().unwrap_or_default();
        let opener = info["openerId"].as_str().filter(|o| !o.is_empty());
        let tab = match (info["type"].as_str(), opener) {
            (Some("page"), Some(opener)) => self.find(opener),
            _ => None,
        };
        let Some(tab) = tab.filter(|t| t.state().mode == Mode::Worker) else {
            release(cdp, session).await;
            return;
        };
        // The paused new tab has no address of its own yet; the page's session told it a moment
        // earlier (`Page.windowOpen`).
        let url = tab.take_opening(OPENING_WAIT).await;
        match new_tab_decision(&tab, url.as_deref()) {
            NewTab::Run => release(cdp, session).await,
            NewTab::Close => close_new_tab(cdp, target, session, reaches_opener(info)).await,
            NewTab::OpenHere(url) => {
                close_new_tab(cdp, target, session, reaches_opener(info)).await;
                // The same way a page the worker opens goes: through the tab's network gate,
                // and watched by the action that is running.
                let _ = cdp
                    .call(
                        Some(&tab.session),
                        "Page.navigate",
                        json!({ "url": url }),
                        tab.limits.navigation,
                    )
                    .await;
            }
        }
    }
}

/// What the worker hears when its page tried to save a file (ADR-037).
fn download_note(filename: &str) -> String {
    if filename.is_empty() {
        "The page tried to save a file. Plenipo's browser does not save files.".into()
    } else {
        format!(
            "The page tried to save a file ({filename}). Plenipo's browser does not save files."
        )
    }
}

fn sign_call(mode: Mode, worker: &str) -> String {
    format!(
        "globalThis.__plenipo && __plenipo.sign({}, {})",
        json!(mode.sign()),
        json!(worker)
    )
}

/// The keys a worker may press, with their DOM code, virtual key code, and text.
pub fn key_codes(key: &str) -> Option<(&'static str, u32, &'static str)> {
    Some(match key {
        "Enter" => ("Enter", 13, "\r"),
        "Tab" => ("Tab", 9, ""),
        "Escape" => ("Escape", 27, ""),
        "Backspace" => ("Backspace", 8, ""),
        "Delete" => ("Delete", 46, ""),
        "ArrowUp" => ("ArrowUp", 38, ""),
        "ArrowDown" => ("ArrowDown", 40, ""),
        "ArrowLeft" => ("ArrowLeft", 37, ""),
        "ArrowRight" => ("ArrowRight", 39, ""),
        "Home" => ("Home", 36, ""),
        "End" => ("End", 35, ""),
        "PageUp" => ("PageUp", 33, ""),
        "PageDown" => ("PageDown", 34, ""),
        " " | "Space" => ("Space", 32, " "),
        _ => return None,
    })
}

/// The browser's events for one tab: what it learns, and the check of every page request.
async fn event_loop(
    cdp: Cdp,
    session: String,
    target: String,
    shared: Arc<Shared>,
    mut events: mpsc::UnboundedReceiver<Event>,
    signals: Signals,
) {
    while let Some(e) = events.recv().await {
        let p = &e.params;
        match e.method.as_str() {
            "Page.frameNavigated" if p["frame"].get("parentId").is_none() => {
                let mut s = shared.state();
                s.url = p["frame"]["url"].as_str().unwrap_or_default().to_owned();
                s.main_frame = p["frame"]["id"]
                    .as_str()
                    .map_or_else(|| s.main_frame.clone(), str::to_owned);
                s.navigations += 1;
                s.page_changed();
            }
            "Network.webSocketCreated" | "Network.webSocketClosed" => {
                socket_event(&shared, &e.method, p);
            }
            "Page.frameAttached" | "Page.frameDetached" => frame_event(&shared, &e.method, p),
            // The page asked for a new tab; the browser's own event (with the paused tab) follows
            // on another channel, and `Tabs::target_attached` asks for this address (ADR-036).
            "Page.windowOpen" => shared.opening(p["url"].as_str().unwrap_or_default()),
            "Page.navigatedWithinDocument" => {
                let mut s = shared.state();
                if p["frameId"].as_str() == Some(s.main_frame.as_str()) {
                    s.url = p["url"].as_str().unwrap_or_default().to_owned();
                }
            }
            "Page.frameStartedLoading" => {
                let mut s = shared.state();
                if p["frameId"].as_str() == Some(s.main_frame.as_str()) {
                    s.loading = true;
                }
            }
            "Page.frameStoppedLoading" => {
                let mut s = shared.state();
                if p["frameId"].as_str() == Some(s.main_frame.as_str()) {
                    s.loading = false;
                }
            }
            "Page.loadEventFired" => {
                let mut s = shared.state();
                s.loads += 1;
                s.loading = false;
            }
            "Runtime.executionContextCreated" => {
                let c = &p["context"];
                let mut s = shared.state();
                if c["name"] == WORLD && c["auxData"]["frameId"].as_str() == Some(&s.main_frame) {
                    s.world = c["id"].as_i64();
                }
            }
            "Runtime.executionContextDestroyed" => {
                let mut s = shared.state();
                if s.world == p["executionContextId"].as_i64() {
                    s.world = None;
                }
            }
            "Runtime.executionContextsCleared" => shared.state().world = None,
            "Runtime.bindingCalled" if p["name"] == BINDING => {
                let kind = serde_json::from_str::<Value>(p["payload"].as_str().unwrap_or("{}"))
                    .ok()
                    .and_then(|v| v["kind"].as_str().map(str::to_owned));
                let signal = match kind.as_deref() {
                    Some("takeOver") => Some(Signal::TakeOver),
                    Some("input") => Some(Signal::OwnerInput),
                    _ => None,
                };
                // While Plenipo acts, a click or key in the page is its own, wherever it lands
                // (the helper in a frame inside the page cannot tell, ADR-032).
                let counts = |signal: &Signal| {
                    let s = shared.state();
                    s.mode == Mode::Worker && !(s.acting && *signal == Signal::OwnerInput)
                };
                if let Some(signal) = signal.filter(counts) {
                    signals(&target, signal);
                }
            }
            "Fetch.requestPaused" => {
                if let Some(answer) = check_request(&shared, p) {
                    let (method, params) = match answer {
                        Answer::Go => ("Fetch.continueRequest", json!({})),
                        Answer::Stop => (
                            "Fetch.failRequest",
                            json!({ "errorReason": "BlockedByClient" }),
                        ),
                    };
                    let mut params = params;
                    params["requestId"] = p["requestId"].clone();
                    let _ = cdp
                        .call(Some(&session), method, params, Duration::from_secs(10))
                        .await;
                }
            }
            "Inspector.targetCrashed" => shared.state().crashed = true,
            "Inspector.detached" => shared.state().gone = true,
            _ => {}
        }
        shared.changed.notify_waiters();
    }
    shared.state().gone = true;
    shared.changed.notify_waiters();
}

enum Answer {
    Go,
    Stop,
}

/// The page opened or closed a live connection (a WebSocket, ADR-035).
fn socket_event(shared: &Shared, method: &str, p: &Value) {
    let Some(id) = p["requestId"].as_str() else {
        return;
    };
    let mut s = shared.state();
    if method == "Network.webSocketCreated" {
        s.sockets.insert(id.to_owned());
    } else {
        s.sockets.remove(id);
    }
}

/// A frame inside the page came or went (ADR-037: a download the browser refused names the
/// frame that started it).
fn frame_event(shared: &Shared, method: &str, p: &Value) {
    let Some(id) = p["frameId"].as_str() else {
        return;
    };
    let mut s = shared.state();
    if method == "Page.frameAttached" {
        s.frames.insert(id.to_owned());
    } else {
        s.frames.remove(id);
    }
}

/// Plenipo's check of one page request (`None`: held for a decision).
fn check_request(shared: &Shared, p: &Value) -> Option<Answer> {
    let method = p["request"]["method"]
        .as_str()
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let url = p["request"]["url"].as_str().unwrap_or_default();
    let kind = p["resourceType"].as_str().unwrap_or_default();
    let mut s = shared.state();
    if s.mode != Mode::Worker {
        return Some(Answer::Go);
    }
    let main = kind == "Document" && p["frameId"].as_str() == Some(s.main_frame.as_str());
    let site = Site::parse(url).ok();
    if kind == "Document" {
        let Some(site) = &site else {
            if main {
                let note = format!(
                    "Plenipo stopped the page from opening {url}: only websites (http and https) \
                     open in its browser."
                );
                drop(s);
                shared.note(note);
                return Some(Answer::Stop);
            }
            return Some(Answer::Go);
        };
        if let Some(note) = site_refused(&s.policy, site, main) {
            drop(s);
            shared.note(note);
            return Some(Answer::Stop);
        }
    }
    let sends = !matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS");
    if sends {
        if s.acting {
            s.held.push(Held {
                request_id: p["requestId"].as_str().unwrap_or_default().to_owned(),
                method,
                url: url.to_owned(),
                site: site.map(|x| x.shown()).unwrap_or_default(),
                kind: kind.to_owned(),
            });
            return None;
        }
        // Sent with no worker action (a form the page submits on its own, a script's POST on a
        // timer, a beacon): stopped, and the worker is told (ADR-035). See the module notes for
        // why it is stopped rather than held.
        let shown = site.map_or_else(|| url.to_owned(), |x| x.shown());
        let note = if kind == "Document" {
            format!(
                "The page tried to send a form to {shown} by itself, without any action of yours; \
                 Plenipo stopped it."
            )
        } else {
            format!(
                "The page tried to send data to {shown} on its own, outside your action; Plenipo \
                 stopped it. To send it, act on the page (click or press a key): the owner is \
                 asked then."
            )
        };
        drop(s);
        shared.note(note);
        return Some(Answer::Stop);
    }
    Some(Answer::Go)
}

/// Plenipo's website check of a page about to open in the tab (`main`: as the tab's own page;
/// else in a frame inside it, where only blocked and local websites are refused): the note for
/// the worker when it may not open. The same check for a page the tab loads (the network gate)
/// and for a new tab the page opens (ADR-036).
fn site_refused(policy: &SitePolicy, site: &Site, main: bool) -> Option<String> {
    let shown = site.shown();
    match websites::check(&policy.rules, site) {
        SiteVerdict::Blocked(rule) => Some(format!(
            "Plenipo stopped {shown} from opening: it is on the owner's blocked websites list \
             (\"{rule}\")."
        )),
        SiteVerdict::Local => Some(format!(
            "Plenipo stopped {shown} from opening: it is an address on this computer or the local \
             network, which opens only when the owner's allowed websites list names it."
        )),
        SiteVerdict::Other if main => Some(format!(
            "Plenipo stopped {shown} from opening: it is not on the owner's allowed websites \
             list, and other websites are blocked."
        )),
        SiteVerdict::Ask if main && !policy.approved.contains(&shown) => Some(format!(
            "The page tried to open {shown}, which is not on the owner's allowed websites list, \
             so Plenipo stopped it. To go there, open it with browser_open (the owner is asked)."
        )),
        _ => None,
    }
}

/// What Plenipo does with a new tab a worker's page opened (ADR-036).
#[derive(Debug, Clone, PartialEq, Eq)]
enum NewTab {
    /// The owner has the tab: the new tab runs.
    Run,
    /// Close it; the worker is told.
    Close,
    /// Close it, and open this address in the worker's own tab: the worker's action opened it,
    /// and the address passes the tab's website check.
    OpenHere(String),
}

/// Decide about a new tab a worker's page opened, whose address is `url` when known, and note
/// what the worker should hear with its next result (ADR-036).
fn new_tab_decision(shared: &Shared, url: Option<&str>) -> NewTab {
    let s = shared.state();
    if s.mode != Mode::Worker {
        return NewTab::Run;
    }
    if !s.acting {
        drop(s);
        shared.note("The page tried to open a new tab on its own; Plenipo closed it.".into());
        return NewTab::Close;
    }
    // A website address: not about:blank, not a script or data address, not unknown.
    let website = url
        .and_then(|u| Site::parse(u).ok().map(|site| (u, site)))
        .filter(|(_, site)| site.scheme != "about");
    let Some((url, site)) = website else {
        drop(s);
        shared.note("The link opened a new tab; Plenipo closed it.".into());
        return NewTab::Close;
    };
    let refused = site_refused(&s.policy, &site, true);
    drop(s);
    match refused {
        Some(refusal) => {
            shared.note(format!("The link opened a new tab. {refusal}"));
            NewTab::Close
        }
        None => {
            shared.note("The link opened a new tab; Plenipo opened it here instead.".into());
            NewTab::OpenHere(url.to_owned())
        }
    }
}

/// The new tab's page can reach the page that opened it (`window.open` keeps a handle; a link
/// or a form does not). Taken as so when the browser does not say.
fn reaches_opener(info: &Value) -> bool {
    info["canAccessOpener"].as_bool().unwrap_or(true)
}

/// Let a target the browser attached to paused run: Plenipo's own new tab, or one the owner has.
async fn release(cdp: &Cdp, session: &str) {
    if !session.is_empty() {
        let _ = cdp
            .call(
                Some(session),
                "Runtime.runIfWaitingForDebugger",
                json!({}),
                Duration::from_secs(10),
            )
            .await;
    }
}

/// Close a new tab a worker's page opened, before it loads anything (ADR-036). One the page
/// cannot reach (a link, a form) is closed while still paused. One the page's script can reach
/// (`window.open`) is different: the browser holds that script until the new tab runs or is
/// closed, and closing it while paused leaves the page unable to take clicks. So the new tab is
/// first told to hold every request, then let run, and closed once it has been quiet for a
/// moment, with each request it tried refused meanwhile (a request still held when a tab closes
/// would be let go; checked on Chromium 141).
async fn close_new_tab(cdp: &Cdp, target: &str, session: &str, reaches_opener: bool) {
    let t = Duration::from_secs(10);
    if reaches_opener && !session.is_empty() {
        let mut events = cdp.listen(session);
        let armed = cdp
            .call(
                Some(session),
                "Fetch.enable",
                json!({ "patterns": [{ "urlPattern": "*", "requestStage": "Request" }] }),
                t,
            )
            .await
            .is_ok();
        if armed {
            let _ = cdp
                .call(
                    Some(session),
                    "Runtime.runIfWaitingForDebugger",
                    json!({}),
                    Duration::from_secs(2),
                )
                .await;
            let longest = Instant::now() + NEW_TAB_LONGEST;
            loop {
                tokio::select! {
                    event = events.recv() => match event {
                        Some(e) if e.method == "Fetch.requestPaused" => {
                            let _ = cdp
                                .call(
                                    Some(session),
                                    "Fetch.failRequest",
                                    json!({ "requestId": e.params["requestId"],
                                            "errorReason": "BlockedByClient" }),
                                    t,
                                )
                                .await;
                        }
                        Some(_) => {}
                        None => break,
                    },
                    () = tokio::time::sleep(NEW_TAB_QUIET) => break,
                }
                if Instant::now() >= longest {
                    break;
                }
            }
        }
        cdp.forget(session);
    }
    let _ = cdp
        .call(None, "Target.closeTarget", json!({ "targetId": target }), t)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(rules: WebsiteRules) -> Shared {
        shared_for("TAB", rules)
    }

    /// A tab whose browser target is `target`.
    fn shared_for(target: &str, rules: WebsiteRules) -> Shared {
        Shared {
            target: target.into(),
            session: format!("S-{target}"),
            limits: TabLimits::default(),
            state: Mutex::new(State {
                main_frame: "MAIN".into(),
                frames: ["MAIN".to_owned()].into(),
                policy: SitePolicy {
                    rules,
                    approved: HashSet::new(),
                },
                ..State::default()
            }),
            changed: Notify::new(),
        }
    }

    fn notes(s: &Shared) -> Vec<String> {
        std::mem::take(&mut s.state().notes)
    }

    /// ADR-036: a new tab a worker's page opened never runs as one. During the worker's action
    /// it is closed and the worker's own tab goes to its address when the website check allows
    /// it (with the gate's own words when not); on its own it is just closed. The worker hears
    /// why either way, and a tab the owner has is left alone.
    #[test]
    fn a_new_tab_a_page_opens_is_closed_and_the_worker_told() {
        let s = shared(WebsiteRules {
            allowed: vec!["shop.test".into()],
            blocked: vec!["blocked.test".into()],
            ..WebsiteRules::default()
        });
        // No action running: closed, whatever the address.
        assert_eq!(
            new_tab_decision(&s, Some("http://shop.test/second")),
            NewTab::Close
        );
        assert_eq!(
            notes(&s),
            ["The page tried to open a new tab on its own; Plenipo closed it."]
        );
        // During an action: the worker's tab goes there when the address passes the lists.
        s.state().acting = true;
        assert_eq!(
            new_tab_decision(&s, Some("http://shop.test/second")),
            NewTab::OpenHere("http://shop.test/second".into())
        );
        assert_eq!(
            notes(&s),
            ["The link opened a new tab; Plenipo opened it here instead."]
        );
        // A blocked website never loads, and the worker hears the gate's words.
        assert_eq!(
            new_tab_decision(&s, Some("https://blocked.test/x")),
            NewTab::Close
        );
        let blocked = notes(&s);
        assert_eq!(blocked.len(), 1, "{blocked:?}");
        assert!(
            blocked[0].starts_with(
                "The link opened a new tab. Plenipo stopped blocked.test from opening: it is on \
                 the owner's blocked websites list (\"blocked.test\")."
            ),
            "{blocked:?}"
        );
        // A website on neither list: stopped, and pointed at browser_open (the owner is asked
        // there); approved for this step, it goes ahead.
        assert_eq!(
            new_tab_decision(&s, Some("https://other.test/")),
            NewTab::Close
        );
        assert!(notes(&s)[0].contains("browser_open"));
        s.state().policy.approved.insert("other.test".into());
        assert_eq!(
            new_tab_decision(&s, Some("https://other.test/")),
            NewTab::OpenHere("https://other.test/".into())
        );
        notes(&s);
        // No address, an empty tab, or a script address: closed (said once).
        for odd in [
            None,
            Some("about:blank"),
            Some("javascript:void 0"),
            Some(""),
        ] {
            assert_eq!(new_tab_decision(&s, odd), NewTab::Close, "{odd:?}");
        }
        assert_eq!(notes(&s), ["The link opened a new tab; Plenipo closed it."]);
        // The owner has the tab: the new tab runs, and nobody is told.
        for mode in [Mode::Owner, Mode::Stopped, Mode::Handed] {
            s.state().mode = mode;
            assert_eq!(
                new_tab_decision(&s, Some("http://shop.test/")),
                NewTab::Run,
                "{mode:?}"
            );
        }
        assert!(s.state().notes.is_empty());
        // The browser says whether the page can reach the new tab; taken as so when it does not.
        assert!(!reaches_opener(&json!({ "canAccessOpener": false })));
        assert!(reaches_opener(&json!({ "canAccessOpener": true })));
        assert!(reaches_opener(&json!({})));
    }

    /// ADR-036: the address of a new tab comes from the page's own ask (`Page.windowOpen`), kept
    /// until the browser attaches to the tab, oldest first; an ask that never became a tab is
    /// forgotten after a while.
    #[tokio::test]
    async fn the_address_of_a_new_tab_comes_from_the_pages_ask() {
        let s = Arc::new(shared(WebsiteRules::default()));
        assert_eq!(s.take_opening(Duration::from_millis(20)).await, None);
        s.opening("http://shop.test/a");
        s.opening("http://shop.test/b");
        assert_eq!(
            s.take_opening(Duration::ZERO).await.as_deref(),
            Some("http://shop.test/a")
        );
        assert_eq!(
            s.take_opening(Duration::ZERO).await.as_deref(),
            Some("http://shop.test/b")
        );
        // An ask that arrives while waiting is taken.
        let waiter = {
            let s = Arc::clone(&s);
            tokio::spawn(async move { s.take_opening(Duration::from_secs(5)).await })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        s.opening("http://shop.test/c");
        s.changed.notify_waiters();
        assert_eq!(waiter.await.unwrap().as_deref(), Some("http://shop.test/c"));
        if let Some(at) = Instant::now().checked_sub(OPENING_KEPT + Duration::from_secs(1)) {
            s.state().opening.push(Opening {
                url: "http://shop.test/old".into(),
                at,
            });
            assert_eq!(s.take_opening(Duration::ZERO).await, None, "stale asks go");
        }
    }

    /// ADR-036: a new tab is traced to the worker's tab that opened it by the browser's target
    /// ID; a tab no worker has (the owner's), or one that is gone, is nobody's.
    #[test]
    fn a_new_tab_is_traced_to_the_tab_that_opened_it() {
        let tabs = Tabs::default();
        let a = Arc::new(shared_for("A", WebsiteRules::default()));
        let b = Arc::new(shared_for("B", WebsiteRules::default()));
        tabs.add(&a);
        tabs.add(&b);
        assert!(tabs
            .find("B")
            .is_some_and(|t| t.target == "B" && t.session == "S-B"));
        assert!(tabs.find("A").is_some_and(|t| t.target == "A"));
        assert!(tabs.find("OWNER").is_none());
        drop(b);
        assert!(tabs.find("B").is_none());
    }

    fn paused(method: &str, url: &str, kind: &str, frame: &str) -> Value {
        json!({ "requestId": "R1", "resourceType": kind, "frameId": frame,
                "request": { "method": method, "url": url } })
    }

    fn go(a: Option<Answer>) -> bool {
        matches!(a, Some(Answer::Go))
    }

    #[test]
    fn pages_pass_the_website_lists() {
        let s = shared(WebsiteRules {
            allowed: vec!["127.0.0.1:8080".into()],
            blocked: vec!["blocked.test".into()],
            ..WebsiteRules::default()
        });
        assert!(go(check_request(
            &s,
            &paused("GET", "http://127.0.0.1:8080/", "Document", "MAIN")
        )));
        assert!(!go(check_request(
            &s,
            &paused("GET", "https://blocked.test/", "Document", "MAIN")
        )));
        assert!(
            !go(check_request(
                &s,
                &paused("GET", "https://blocked.test/ad", "Document", "SUB")
            )),
            "blocked websites stay out of frames too"
        );
        // A website on neither list: stopped until the owner approves it for this step.
        assert!(!go(check_request(
            &s,
            &paused("GET", "https://other.test/", "Document", "MAIN")
        )));
        assert!(s.state().notes.iter().any(|n| n.contains("browser_open")));
        s.state().policy.approved.insert("other.test".into());
        assert!(go(check_request(
            &s,
            &paused("GET", "https://other.test/", "Document", "MAIN")
        )));
        // Frames of unlisted websites (embeds) and page scripts' reads go ahead.
        assert!(go(check_request(
            &s,
            &paused("GET", "https://cdn.test/x", "Document", "SUB")
        )));
        assert!(go(check_request(
            &s,
            &paused("GET", "https://api.test/x", "XHR", "MAIN")
        )));
        assert!(!go(check_request(
            &s,
            &paused("GET", "file:///etc/passwd", "Document", "MAIN")
        )));
    }

    #[test]
    fn data_sent_after_an_action_is_held_and_by_itself_is_stopped() {
        let s = shared(WebsiteRules {
            allowed: vec!["127.0.0.1:8080".into()],
            ..WebsiteRules::default()
        });
        let form = paused("POST", "http://127.0.0.1:8080/send", "Document", "MAIN");
        // No action: a form the page sends by itself is stopped, and so is a script's POST or a
        // beacon (ADR-035); the worker is told in both cases. Plain reads go ahead.
        assert!(!go(check_request(&s, &form)));
        assert!(s.state().notes.iter().any(|n| n.contains("by itself")));
        assert!(!go(check_request(
            &s,
            &paused("POST", "http://127.0.0.1:8080/api", "Fetch", "MAIN")
        )));
        assert!(!go(check_request(
            &s,
            &paused("POST", "http://127.0.0.1:8080/beacon", "Ping", "MAIN")
        )));
        assert!(
            s.state()
                .notes
                .iter()
                .any(|n| n.contains("on its own") && n.contains("127.0.0.1:8080")),
            "{:?}",
            s.state().notes
        );
        assert!(go(check_request(
            &s,
            &paused("GET", "http://127.0.0.1:8080/favicon.ico", "Other", "MAIN")
        )));
        assert!(go(check_request(
            &s,
            &paused("OPTIONS", "http://127.0.0.1:8080/api", "XHR", "MAIN")
        )));
        assert!(s.state().held.is_empty());
        // During a worker's action, sending is held for a decision: forms, scripts' requests,
        // beacons, and whatever else the page sends.
        s.state().acting = true;
        assert!(check_request(&s, &form).is_none());
        assert!(check_request(
            &s,
            &paused("PUT", "http://127.0.0.1:8080/api", "XHR", "MAIN")
        )
        .is_none());
        assert!(check_request(
            &s,
            &paused("POST", "http://127.0.0.1:8080/beacon", "Ping", "MAIN")
        )
        .is_none());
        assert!(check_request(
            &s,
            &paused("POST", "http://127.0.0.1:8080/misc", "Other", "MAIN")
        )
        .is_none());
        let held = std::mem::take(&mut s.state().held);
        assert_eq!(held.len(), 4);
        assert_eq!(held[0].site, "127.0.0.1:8080");
        assert_eq!(held[0].method, "POST");
        assert_eq!(held[2].kind, "Ping");
        // Once the owner has control, nothing is checked.
        s.state().mode = Mode::Owner;
        assert!(go(check_request(
            &s,
            &paused("POST", "https://blocked.test/", "Document", "MAIN")
        )));
    }

    #[test]
    fn a_live_connection_is_noticed_until_the_page_changes() {
        let s = shared(WebsiteRules::default());
        assert!(!s.state().has_websocket());
        socket_event(
            &s,
            "Network.webSocketCreated",
            &json!({ "requestId": "W1", "url": "ws://chat.test/ws" }),
        );
        socket_event(
            &s,
            "Network.webSocketCreated",
            &json!({ "requestId": "W2", "url": "ws://chat.test/ws" }),
        );
        assert!(s.state().has_websocket());
        socket_event(
            &s,
            "Network.webSocketClosed",
            &json!({ "requestId": "W1", "timestamp": 1.0 }),
        );
        assert!(s.state().has_websocket(), "one socket is still open");
        socket_event(
            &s,
            "Network.webSocketClosed",
            &json!({ "requestId": "W2", "timestamp": 2.0 }),
        );
        assert!(!s.state().has_websocket());
        // A new page starts with no live connection, whatever the old one had.
        socket_event(
            &s,
            "Network.webSocketCreated",
            &json!({ "requestId": "W3", "url": "ws://chat.test/ws" }),
        );
        s.state().page_changed();
        assert!(!s.state().has_websocket());
    }

    /// ADR-037: the tab keeps the frames of its page, so a download the browser refused (which
    /// names only the frame that started it) reaches the right worker.
    #[test]
    fn the_frames_of_the_page_are_kept_until_it_changes() {
        let s = shared(WebsiteRules::default());
        frame_event(
            &s,
            "Page.frameAttached",
            &json!({ "frameId": "AD", "parentFrameId": "MAIN" }),
        );
        frame_event(
            &s,
            "Page.frameAttached",
            &json!({ "frameId": "MAP", "parentFrameId": "MAIN" }),
        );
        assert!(s.state().frames.contains("MAIN"));
        assert!(s.state().frames.contains("AD") && s.state().frames.contains("MAP"));
        frame_event(
            &s,
            "Page.frameDetached",
            &json!({ "frameId": "AD", "reason": "remove" }),
        );
        assert!(!s.state().frames.contains("AD"));
        assert!(s.state().frames.contains("MAP"));
        frame_event(&s, "Page.frameAttached", &json!({ "reason": "odd" }));
        // A new page (whose main frame may be another) starts with its main frame only.
        s.state().main_frame = "MAIN2".into();
        s.state().page_changed();
        assert_eq!(s.state().frames, ["MAIN2".to_owned()].into());
    }

    /// ADR-037: a refused download is noted for the worker whose page (or a frame in it) tried
    /// it, in plain words; nobody is told of one from a tab no worker has.
    #[test]
    fn a_refused_download_is_noted_on_the_tab_whose_frame_started_it() {
        let tabs = Tabs::default();
        let a = Arc::new(shared(WebsiteRules::default()));
        let b = Arc::new(shared(WebsiteRules::default()));
        b.state().main_frame = "B".into();
        b.state().page_changed();
        tabs.add(&a);
        tabs.add(&b);
        frame_event(
            &b,
            "Page.frameAttached",
            &json!({ "frameId": "B-AD", "parentFrameId": "B" }),
        );
        assert!(tabs.download_refused(&json!({
            "frameId": "B-AD", "guid": "g1", "url": "http://ad.test/x.exe",
            "suggestedFilename": "x.exe"
        })));
        assert!(a.state().notes.is_empty());
        assert_eq!(
            b.state().notes,
            ["The page tried to save a file (x.exe). Plenipo's browser does not save files."]
        );
        assert!(tabs.download_refused(&json!({ "frameId": "MAIN", "guid": "g2" })));
        assert_eq!(
            a.state().notes,
            ["The page tried to save a file. Plenipo's browser does not save files."]
        );
        // The same file tried twice is one note; a tab no worker has (the owner's) tells nobody.
        assert!(tabs.download_refused(&json!({ "frameId": "MAIN", "guid": "g3" })));
        assert_eq!(a.state().notes.len(), 1);
        assert!(!tabs.download_refused(&json!({ "frameId": "OWNER", "suggestedFilename": "y" })));
        // A tab that is gone is forgotten.
        drop(b);
        assert!(!tabs.download_refused(&json!({ "frameId": "B-AD", "suggestedFilename": "x.exe" })));
        tabs.add(&a);
        assert_eq!(
            lock(&tabs.tabs).len(),
            2,
            "the gone tab was dropped from the list"
        );
    }

    #[test]
    fn a_captcha_state_reads_from_the_helper() {
        let state: CaptchaState = serde_json::from_value(json!({
            "present": true, "provider": "reCAPTCHA", "solved": false, "challenge": false,
            "invisible": false, "checkbox": "e3"
        }))
        .unwrap();
        assert_eq!(state.checkbox.as_deref(), Some("e3"));
        assert_eq!(state.provider, "reCAPTCHA");
        let bare: CaptchaState = serde_json::from_value(json!({})).unwrap();
        assert_eq!(bare, CaptchaState::default(), "missing fields read as off");
    }

    #[test]
    fn keys_have_codes() {
        assert_eq!(key_codes("Enter"), Some(("Enter", 13, "\r")));
        assert!(key_codes("F12").is_none());
        assert!(
            key_codes("Control").is_none(),
            "no modifier keys in the browser"
        );
    }
}
