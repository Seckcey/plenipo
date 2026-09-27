//! One tab of Plenipo's browser, for one worker's step (Phase 10, ADR-020).
//!
//! The tab keeps what it learns from the browser's events (its address, loads, crashes, the
//! helper's world), and checks every page request itself:
//! - a page opened in the tab must pass the owner's website lists (blocked websites never
//!   open; a website on neither list opens only once the owner approved it for this step);
//! - data a page sends right after a worker's click or key press (a form, a message) is held
//!   until Plenipo decides: the broker asks the owner, then lets it go or stops it;
//! - a form a page tries to send by itself, with no worker action, is stopped.
//!
//! When the owner takes control or stops it, the tab stops checking (the owner browses freely)
//! and its sign says so.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::Engine as _;
use plenipo_guard::websites::{self, SiteVerdict};
use plenipo_guard::{Site, WebsiteRules};
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
    /// The owner is solving a check that a person is using the site (a CAPTCHA, ADR-021); the
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
    /// `Document` (a form) or `XHR`/`Fetch` (a page script).
    pub kind: String,
}

/// The websites a tab may open: the owner's lists, and those the owner approved for this step.
#[derive(Debug, Clone, Default)]
pub struct SitePolicy {
    pub rules: WebsiteRules,
    pub approved: HashSet<String>,
}

#[derive(Default)]
struct State {
    main_frame: String,
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
    /// What the worker should hear with its next result (a page Plenipo stopped, …).
    notes: Vec<String>,
    policy: SitePolicy,
    mode: Mode,
    worker: String,
    /// The registered script that draws the sign on each new page.
    sign_script: Option<String>,
}

struct Shared {
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
        self.shared.state().main_frame = frame;
        self.show_sign().await
    }

    /// Hold every page request for Plenipo's checks (while a worker has the tab).
    async fn watch_requests(&self) -> Result<(), String> {
        let patterns: Vec<Value> = ["Document", "XHR", "Fetch"]
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

    /// The page in words: address, title, text, numbered controls, CAPTCHA.
    pub async fn read(&self, max_chars: usize, max_controls: usize) -> Result<Value, String> {
        let _busy = self.busy.lock().await;
        self.helper(&format!("__plenipo.read({max_chars}, {max_controls})"))
            .await
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

    /// Click the center of a control (its facts were just read).
    pub async fn click(&self, facts: &ElementFacts) -> Result<Settled, String> {
        let _busy = self.busy.lock().await;
        let (x, y) = (facts.x, facts.y);
        self.acting(async {
            self.mouse("mouseMoved", x, y, 0).await?;
            self.mouse("mousePressed", x, y, 1).await?;
            self.mouse("mouseReleased", x, y, 1).await
        })
        .await
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
                s.world = None;
            }
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
                if let Some(signal) = signal.filter(|_| shared.state().mode == Mode::Worker) {
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
        let shown = site.shown();
        let refused = match websites::check(&s.policy.rules, site) {
            SiteVerdict::Blocked(rule) => Some(format!(
                "Plenipo stopped {shown} from opening: it is on the owner's blocked websites list \
                 (\"{rule}\")."
            )),
            SiteVerdict::Local => Some(format!(
                "Plenipo stopped {shown} from opening: it is an address on this computer or the \
                 local network, which opens only when the owner's allowed websites list names it."
            )),
            SiteVerdict::Other if main => Some(format!(
                "Plenipo stopped {shown} from opening: it is not on the owner's allowed websites \
                 list, and other websites are blocked."
            )),
            SiteVerdict::Ask if main && !s.policy.approved.contains(&shown) => Some(format!(
                "The page tried to open {shown}, which is not on the owner's allowed websites \
                 list, so Plenipo stopped it. To go there, open it with browser_open (the owner \
                 is asked)."
            )),
            _ => None,
        };
        if let Some(note) = refused {
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
        if kind == "Document" {
            let note = format!(
                "The page tried to send a form to {} by itself, without any action of yours; \
                 Plenipo stopped it.",
                site.map_or_else(|| url.to_owned(), |x| x.shown())
            );
            drop(s);
            shared.note(note);
            return Some(Answer::Stop);
        }
    }
    Some(Answer::Go)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(rules: WebsiteRules) -> Shared {
        Shared {
            state: Mutex::new(State {
                main_frame: "MAIN".into(),
                policy: SitePolicy {
                    rules,
                    approved: HashSet::new(),
                },
                ..State::default()
            }),
            changed: Notify::new(),
        }
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
        // No action: a form the page sends by itself is stopped; a script's POST goes ahead.
        assert!(!go(check_request(&s, &form)));
        assert!(s.state().notes.iter().any(|n| n.contains("by itself")));
        assert!(go(check_request(
            &s,
            &paused("POST", "http://127.0.0.1:8080/beacon", "Fetch", "MAIN")
        )));
        // During a worker's action, sending is held for a decision.
        s.state().acting = true;
        assert!(check_request(&s, &form).is_none());
        assert!(check_request(
            &s,
            &paused("PUT", "http://127.0.0.1:8080/api", "XHR", "MAIN")
        )
        .is_none());
        let held = std::mem::take(&mut s.state().held);
        assert_eq!(held.len(), 2);
        assert_eq!(held[0].site, "127.0.0.1:8080");
        assert_eq!(held[0].method, "POST");
        // Once the owner has control, nothing is checked.
        s.state().mode = Mode::Owner;
        assert!(go(check_request(
            &s,
            &paused("POST", "https://blocked.test/", "Document", "MAIN")
        )));
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
