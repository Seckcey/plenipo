//! Plenipo's browser and this computer's screen, mouse, and keyboard, through the broker
//! (Phase 10, ADR-020). Every call goes the Phase 7 way: read, checked by Guard (now also
//! against the owner's website lists), sent to the owner for approval when Guard says so,
//! carried out by Plenipo, recorded — with a screenshot — and answered with secrets hidden.
//!
//! On top of that:
//! - **Control sessions.** A worker using the browser or the mouse and keyboard is a session the
//!   owner sees everywhere ([`crate::control`]). The owner can take one over (that worker stops)
//!   or stop them all at once; a stop holds until the owner allows control again.
//! - **Sensitive actions.** Submitting a form, buying, signing in, and sending always wait for
//!   the owner's approval: from the control clicked or the key pressed (Enter in any text box
//!   sends what it holds, in a form or not), from data a page sends right after a worker's
//!   action (held in the tab until the owner decides; data it sends on its own between actions
//!   is stopped), and, on a page with a live connection (a WebSocket, which the tab cannot see
//!   into), from any click, Enter, or Space (asked before the action; ADR-035).
//! - **Never:** typing into password, one-time-code, or card fields; typing a secret; trying a
//!   CAPTCHA more than 3 times (then it goes to the owner, ADR-029; the worker sees the
//!   check's checkbox and hears how each try went, ADR-032); the Windows key.
//! - **Computer use is the last resort:** a worker must ask to take control, with its reason,
//!   and the owner is asked each time. The owner moving the mouse takes control back.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use plenipo_guard::engine::Scope;
use plenipo_guard::{
    Capability, Decision, Layer, Risk, SensitiveKind, SensitiveRule, Site, SiteVerdict, Verdict,
    Workspace,
};
use plenipo_ledger::{ApprovalState, NewEvent};
use serde_json::{json, Value};

use super::{cap, lock, Broker, Image, Inner, Prepared, Refused, Work, GUARD};
use crate::browser::classify::{self, ElementFacts};
use crate::browser::tab::{
    Held, Mode, Signal, SitePolicy, Tab, CAPTCHA_TRIES, CAPTCHA_VERDICT_WAIT,
};
use crate::browser::Start;
use crate::control::{session_id, ControlKind, ControlState, ControlStatus};
use crate::desktop::{parse_keys, Button, KeyPart};
use crate::error::{BrokerError, Result};
use crate::fence;
use crate::screens;
use crate::tools::{Action, ToolDef};

/// Most controls listed when a page is read.
const MAX_CONTROLS: usize = 150;
/// How far the mouse may drift before Plenipo takes it as the owner's hand (screen pixels).
const OWNER_MOVE_PIXELS: i32 = 6;

/// The browser or screen work of one call, once allowed.
pub(super) enum ControlWork {
    Open {
        url: String,
        timeout: Option<u64>,
    },
    Read {
        max_chars: usize,
    },
    Screenshot,
    Scroll {
        dy: i64,
    },
    Back,
    /// Hand a check that a person is using the site (a CAPTCHA) to the owner (ADR-023).
    PersonCheck,
    Click {
        facts: Box<ElementFacts>,
        what: String,
    },
    Type {
        reference: String,
        text: String,
        enter: bool,
        what: String,
    },
    Press {
        key: String,
        /// The key submits an answer to the page's CAPTCHA: one try (ADR-029).
        captcha: bool,
    },
    Select {
        reference: String,
        option: String,
    },
    ScreenView,
    TakeControl {
        reason: String,
    },
    ScreenClick {
        x: i32,
        y: i32,
        button: Button,
        count: u8,
    },
    ScreenType {
        text: String,
    },
    ScreenKeys {
        keys: Vec<KeyPart>,
    },
    ScreenScroll {
        at: Option<(i32, i32)>,
        lines: i32,
    },
    Release,
}

/// A worker's use of the screen in one step.
#[derive(Default)]
pub(super) struct DesktopUse {
    /// From the last screen_view: screen pixels per picture pixel, and the picture's size.
    pub(super) view: Option<(f64, u32, u32)>,
    /// While it has the mouse and keyboard.
    pub(super) control: Option<Arc<Watch>>,
}

/// A worker's hold on the mouse and keyboard, watched for the owner's hand.
pub(super) struct Watch {
    /// Where Plenipo last left the pointer.
    last: Mutex<Option<(i32, i32)>>,
    /// Plenipo is moving it now.
    acting: AtomicBool,
    /// The hold ended.
    ended: AtomicBool,
}

/// What a call needs to ask the owner about something that comes up while it is carried out.
pub(super) struct CallContext<'a> {
    pub(super) grant_id: &'a str,
    pub(super) task_id: &'a str,
    pub(super) runtime_id: &'a str,
    pub(super) worker: &'a str,
    pub(super) scope: &'a Scope,
    pub(super) workspace: Option<&'a Workspace>,
    pub(super) tool: &'a ToolDef,
    /// The owner already approved this action (so data it sends needs no second approval).
    pub(super) approved: bool,
}

/// How a browser or screen call went.
pub(super) struct ControlDone {
    pub(super) result: std::result::Result<String, String>,
    pub(super) images: Vec<Image>,
    pub(super) screenshot: Option<String>,
    pub(super) url: Option<String>,
}

impl Default for ControlDone {
    fn default() -> Self {
        Self {
            result: Ok(String::new()),
            images: Vec::new(),
            screenshot: None,
            url: None,
        }
    }
}

fn refuse(layer: Layer, reason: impl Into<String>, summary: impl Into<String>) -> Refused {
    Refused {
        layer,
        reason: reason.into(),
        summary: summary.into(),
    }
}

/// "button \"Send message\"", "the field \"Your name\"", "a link".
fn describe(f: &ElementFacts) -> String {
    let kind = match (f.tag.as_str(), f.r#type.as_str()) {
        ("a", _) => "link",
        ("select", _) => "list",
        ("textarea", _) => "text box",
        ("input", "checkbox") => "checkbox",
        ("input", "radio") => "option",
        ("input", "submit" | "button" | "image" | "reset") | ("button", _) => "button",
        ("input", _) => "field",
        _ if f.role == "button" => "button",
        _ if !f.role.is_empty() => f.role.as_str(),
        _ => "control",
    };
    if f.name.is_empty() {
        format!("a {kind}")
    } else {
        format!("the {kind} \"{}\"", cap(&f.name, 80))
    }
}

/// Why a click or key press on a page with a live connection is sensitive (ADR-035): the
/// network gate cannot see what the page sends through a WebSocket, so the owner is asked
/// before the action, whatever the control looks like.
fn live_connection() -> (SensitiveKind, String) {
    (
        SensitiveKind::Outbound,
        "this page has a live connection (a WebSocket) that sends as you type or click".into(),
    )
}

/// A web address's website, as the lists name it.
fn host_of(url: &str) -> String {
    Site::parse(url).map_or_else(
        |_| url.to_owned(),
        |s| {
            if s.scheme == "about" {
                "an empty page".into()
            } else {
                s.shown()
            }
        },
    )
}

/// One control, as a worker reads it.
fn control_line(e: &Value) -> String {
    let s = |k: &str| e[k].as_str().unwrap_or_default();
    let (tag, kind) = (s("tag"), s("type"));
    let what = match (tag, kind) {
        ("a", _) => "link",
        ("button", _) | ("input", "submit" | "button" | "image" | "reset") => "button",
        ("select", _) => "list",
        ("textarea", _) => "text box",
        ("input", "checkbox") => "checkbox",
        ("input", "radio") => "option",
        ("input", "password") => "password field",
        ("input", _) => "field",
        _ if !s("role").is_empty() => s("role"),
        _ => "control",
    };
    let mut line = format!("- {}: {what}", s("ref"));
    if !s("name").is_empty() {
        line.push_str(&format!(" \"{}\"", cap(s("name"), 100)));
    }
    if e["secret"] == true {
        line.push_str(" (never type here: signing in and paying are the owner's)");
    } else if let Some(v) = e["value"].as_str().filter(|v| !v.is_empty()) {
        line.push_str(&format!(" (holds \"{}\")", cap(v, 80)));
    }
    if let Some(c) = e["checked"].as_bool() {
        line.push_str(if c { " (checked)" } else { " (not checked)" });
    }
    if tag == "a" && !s("href").is_empty() {
        line.push_str(&format!(" → {}", cap(s("href"), 160)));
    }
    if e["captcha"] == true {
        line.push_str(" (the CAPTCHA's own checkbox: clicking it is one try)");
    }
    if e["disabled"] == true {
        line.push_str(" (turned off)");
    }
    line
}

/// The page as a worker reads it: its address and title, warnings, the text (marked as the
/// website's, not instructions), and its controls.
fn page_text(page: &Value, controls: bool, captcha_tries: u32, tries_allowed: bool) -> String {
    let url = page["url"].as_str().unwrap_or_default();
    let host = host_of(url);
    let mut out = format!(
        "Page: \"{}\"\nAddress: {url}\n",
        page["title"].as_str().unwrap_or_default()
    );
    let check = &page["captchaInfo"];
    let provider = check["provider"]
        .as_str()
        .filter(|p| !p.is_empty())
        .unwrap_or("CAPTCHA");
    if page["captcha"] == true {
        if check["solved"] == true {
            out.push_str(&format!(
                "This page's CAPTCHA ({provider}, a check that a person is using the site) is \
                 passed already: the website has its answer. Do not click it; go on with the \
                 page.\n"
            ));
        } else if tries_allowed {
            out.push_str(&format!(
                "This page shows a CAPTCHA ({provider}, a check that a person is using the \
                 site). "
            ));
            if check["challenge"] == true {
                out.push_str(
                    "It shows a puzzle (pictures to pick) inside its own frame, which Plenipo \
                     cannot list as controls: take a screenshot to see it, or hand it to the \
                     owner now with browser_person_check. ",
                );
            } else if let Some(r) = check["checkbox"].as_str() {
                out.push_str(&format!(
                    "Its checkbox is {r}: click it once, and the result says whether the check \
                     passed. "
                ));
            }
            out.push_str(&format!(
                "You may try to answer it yourself: each answer you submit counts as one try, \
                 and Plenipo lets you try {CAPTCHA_TRIES} times (you have used {captcha_tries}). \
                 When the tries are used up, call browser_person_check to hand it to the owner, \
                 who solves it, then continue; if that is refused, stop here and say in your \
                 answer that the owner should take over.\n",
            ));
        } else {
            out.push_str(&format!(
                "This page shows a CAPTCHA ({provider}, a check that a person is using the \
                 site). The owner has not switched on handing these checks to them, so do not \
                 try to answer it: stop here, and say in your answer that the owner should take \
                 over.\n",
            ));
        }
    } else if check["invisible"] == true {
        out.push_str(&format!(
            "This page has an invisible check that a person is using the site ({provider}). It \
             runs by itself when you use the page; there is nothing to click.\n"
        ));
    }
    if page["passwordFields"].as_u64().unwrap_or(0) > 0 {
        out.push_str(
            "This page asks for a password. Signing in is the owner's: never type in it; if you \
             need to be signed in, stop and say that the owner should take over and sign in.\n",
        );
    }
    // The website's words, fenced (crate::fence): information, never instructions.
    let mut words = format!("{}\n", page["text"].as_str().unwrap_or_default().trim());
    if page["truncated"] == true {
        words.push_str("(… more text: read with a larger maxChars, or scroll)\n");
    }
    out.push_str(&fence::fenced(&fence::Source::Page(host), &words));
    if controls {
        let items = page["elements"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            out.push_str("No links or controls are visible.\n");
        } else {
            out.push_str(
                "Links and controls (give the reference to browser_click, browser_type, or \
                 browser_select):\n",
            );
            for e in &items {
                out.push_str(&control_line(e));
                out.push('\n');
            }
            if items.len() >= MAX_CONTROLS {
                out.push_str("(… more controls further down: scroll, then read again)\n");
            }
        }
    }
    out
}

impl Broker {
    fn grant_tab(&self, grant_id: &str) -> Option<Arc<Tab>> {
        self.state()
            .grants
            .get(grant_id)
            .and_then(|g| g.tab.clone())
    }

    /// Why a browser or screen call cannot go ahead now: the owner stopped control or took it
    /// over.
    pub(super) fn control_refusal(&self, grant_id: &str, tool: &ToolDef) -> Option<String> {
        let control = &self.inner.control;
        if control.stopped() {
            return Some(
                "The owner stopped all browser, desktop, and server work. Do not try again: say in \
                 your answer what you were doing and what is left."
                    .into(),
            );
        }
        let kind = match tool.capability {
            Capability::BrowserNavigate | Capability::BrowserAutomate => ControlKind::Browser,
            Capability::SshConnect => ControlKind::Server,
            _ => ControlKind::Desktop,
        };
        // A feature the owner switched off (ADR-023).
        if let Some(off) = self
            .inner
            .guard
            .config()
            .ok()
            .and_then(|c| plenipo_guard::engine::switched_off(&c, tool.capability))
        {
            let mut off = off.to_owned();
            if let Some(first) = off.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            return Some(format!(
                "{off}. Do not try another way: say in your answer what you were doing and what \
                 is left."
            ));
        }
        match control
            .session(&session_id(kind, grant_id))
            .map(|s| s.state)
        {
            Some(ControlState::TakenOver) => Some(match kind {
                ControlKind::Browser => {
                    "The owner took over the browser, so your part in it has \
                    ended: do not use it again. Say in your answer where you were and what is left."
                        .into()
                }
                ControlKind::Desktop => "The owner took back the mouse and keyboard: do not use \
                    them again. Say in your answer where you were and what is left."
                    .into(),
                ControlKind::Server => "The owner disconnected you from the servers: do not use \
                    them again. Say in your answer what you ran, where, and what is left."
                    .into(),
            }),
            Some(ControlState::Stopped) => Some(
                "The owner stopped your use of the browser, desktop, and servers. Say in your \
                 answer what you were doing and what is left."
                    .into(),
            ),
            _ => None,
        }
    }

    /// The website the grant's tab shows now, if any.
    fn tab_site(tab: &Tab) -> Option<Site> {
        Site::parse(&tab.url()).ok()
    }

    /// A browser call needs the tab its worker already opened.
    fn open_tab(&self, grant_id: &str, summary: &str) -> std::result::Result<Arc<Tab>, Refused> {
        let tab = self.grant_tab(grant_id).ok_or_else(|| {
            refuse(
                Layer::Target,
                "no page is open yet: open one with browser_open.",
                summary,
            )
        })?;
        if let Some(why) = tab.broken() {
            return Err(refuse(
                Layer::Target,
                format!("your tab is gone ({why}): open the page again with browser_open."),
                summary,
            ));
        }
        Ok(tab)
    }

    /// A worker may try a CAPTCHA a few times only when the owner takes these checks over
    /// afterwards (the hand-off switch, ADR-023/ADR-029); with the switch off, a CAPTCHA stops
    /// the worker at once, as before.
    fn captcha_tries_allowed(&self) -> bool {
        self.inner
            .guard
            .config()
            .is_ok_and(|c| c.switches.captcha_to_owner)
    }

    /// Facts about a control, refusing what no worker may use.
    async fn control_facts(
        &self,
        tab: &Tab,
        reference: &str,
        summary: &str,
    ) -> std::result::Result<ElementFacts, Refused> {
        let facts = tab.facts(reference).await.map_err(|e| {
            refuse(
                Layer::Target,
                format!("the page could not be read ({e})."),
                summary,
            )
        })?;
        if !facts.found {
            return Err(refuse(
                Layer::Target,
                format!(
                    "{reference} is not on the page now (the page changed): read it again with \
                     browser_read."
                ),
                summary,
            ));
        }
        if facts.captcha {
            if facts.solved {
                // Nothing to answer any more: a click there only wastes a try (ADR-032).
                tab.clear_captcha_attempts();
                return Err(refuse(
                    Layer::Target,
                    format!(
                        "{reference} is part of a CAPTCHA (a check that a person is using the \
                         site) that is passed already: the website has its answer, so there is \
                         nothing to click there. Go on with the page."
                    ),
                    summary,
                ));
            }
            if !self.captcha_tries_allowed() {
                return Err(refuse(
                    Layer::Rule,
                    format!(
                        "{reference} is part of a CAPTCHA (a check that a person is using the \
                         site), and the owner has not switched on handing these checks to them. \
                         Do not try to answer it: stop here, and say in your answer that the \
                         owner should take over."
                    ),
                    summary,
                ));
            }
            let tries = tab.captcha_attempts();
            if tries >= CAPTCHA_TRIES {
                return Err(refuse(
                    Layer::Rule,
                    format!(
                        "{reference} is part of a CAPTCHA (a check that a person is using the \
                         site), and you have tried it {tries} times: that is the limit. Stop \
                         trying: call browser_person_check to hand it to the owner, or stop and \
                         say that the owner should take over."
                    ),
                    summary,
                ));
            }
        }
        Ok(facts)
    }

    /// Read a browser or screen call into what Guard checks and what Plenipo then does.
    pub(super) async fn prepare_control(
        &self,
        grant_id: &str,
        tool: &ToolDef,
        action: Action,
        worker: &str,
    ) -> std::result::Result<Prepared, Refused> {
        let base = |capability, summary: String, detail: String, work| Prepared {
            capability,
            risk: tool.risk,
            summary,
            detail,
            files: Vec::new(),
            writes_git_dir: false,
            command: None,
            script: None,
            inherent: None,
            inherent_owned: None,
            site: None,
            screenshot: None,
            server: None,
            harmless: false,
            work: Work::Control(work),
        };
        let nav = Capability::BrowserNavigate;
        let auto = Capability::BrowserAutomate;
        Ok(match action {
            Action::BrowserOpen { url, timeout } => {
                let summary = format!("open {}", cap(&url, 200));
                let site = Site::parse(&url)
                    .map_err(|e| refuse(Layer::Target, format!("{e}."), summary.clone()))?;
                let mut p = base(
                    nav,
                    format!("open {}", cap(&site.url, 200)),
                    site.url.clone(),
                    ControlWork::Open {
                        url: site.url.clone(),
                        timeout,
                    },
                );
                p.site = Some(site);
                p
            }
            Action::BrowserRead { max_chars } => {
                let tab = self.open_tab(grant_id, "read the page")?;
                let site = Self::tab_site(&tab);
                let mut p = base(
                    nav,
                    format!("read the page on {}", host_of(&tab.url())),
                    tab.url(),
                    ControlWork::Read { max_chars },
                );
                p.site = site;
                p
            }
            Action::BrowserScreenshot => {
                let tab = self.open_tab(grant_id, "take a screenshot of the page")?;
                let mut p = base(
                    nav,
                    format!("take a screenshot of the page on {}", host_of(&tab.url())),
                    tab.url(),
                    ControlWork::Screenshot,
                );
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserScroll { down, pages } => {
                let tab = self.open_tab(grant_id, "scroll the page")?;
                let dy = i64::from(pages) * 700 * if down { 1 } else { -1 };
                let mut p = base(
                    nav,
                    format!(
                        "scroll {} on {}",
                        if down { "down" } else { "up" },
                        host_of(&tab.url())
                    ),
                    tab.url(),
                    ControlWork::Scroll { dy },
                );
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserBack => {
                let tab = self.open_tab(grant_id, "go back")?;
                let mut p = base(
                    nav,
                    format!("go back from {}", host_of(&tab.url())),
                    tab.url(),
                    ControlWork::Back,
                );
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserPersonCheck => {
                let s = "hand a person check to the owner".to_owned();
                let tab = self.open_tab(grant_id, &s)?;
                let page = tab.read(200, 0).await.unwrap_or_default();
                if page["captcha"] == true && page["captchaInfo"]["solved"] == true {
                    return Err(refuse(
                        Layer::Target,
                        "this page's check that a person is using the site is passed already, \
                         so there is nothing to hand to the owner: go on with the page.",
                        s,
                    ));
                }
                if page["captcha"] != true {
                    return Err(refuse(
                        Layer::Target,
                        "this page shows no check that a person is using the site, so there is \
                         nothing to hand to the owner.",
                        s,
                    ));
                }
                let to_owner = self
                    .inner
                    .guard
                    .config()
                    .is_ok_and(|c| c.switches.captcha_to_owner);
                if !to_owner {
                    return Err(refuse(
                        Layer::Rule,
                        "the owner has not switched on handing these checks to them. Stop here, \
                         and say in your answer that the owner should take over.",
                        s,
                    ));
                }
                let mut p = base(
                    nav,
                    format!(
                        "hand you a check that a person is using {} (a CAPTCHA)",
                        host_of(&tab.url())
                    ),
                    tab.url(),
                    ControlWork::PersonCheck,
                );
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserClick { reference } => {
                let s = format!("click {reference}");
                let tab = self.open_tab(grant_id, &s)?;
                let facts = self.control_facts(&tab, &reference, &s).await?;
                if !facts.visible || facts.disabled {
                    return Err(refuse(
                        Layer::Target,
                        format!(
                            "{} is {} on the page.",
                            describe(&facts),
                            if facts.disabled {
                                "turned off"
                            } else {
                                "not visible"
                            }
                        ),
                        s,
                    ));
                }
                if !facts.clear {
                    return Err(refuse(
                        Layer::Target,
                        format!(
                            "something on the page covers {} (a banner or a dialog): close it \
                             first.",
                            describe(&facts)
                        ),
                        s,
                    ));
                }
                let what = describe(&facts);
                let mut p = base(
                    auto,
                    format!("click {what} on {}", host_of(&tab.url())),
                    format!("{what} ({reference}) on {}", tab.url()),
                    ControlWork::Click {
                        what: what.clone(),
                        facts: Box::new(facts.clone()),
                    },
                );
                p.inherent_owned = classify::click(&facts);
                if p.inherent_owned.is_none() && tab.has_websocket() {
                    p.inherent_owned = Some(live_connection());
                }
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserType {
                reference,
                text,
                submit,
            } => {
                let s = format!("type into {reference}");
                let tab = self.open_tab(grant_id, &s)?;
                let facts = self.control_facts(&tab, &reference, &s).await?;
                if facts.secret {
                    return Err(refuse(
                        Layer::Rule,
                        format!(
                            "{} is a password, one-time code, or card field. Plenipo never types \
                             into those: signing in and paying are the owner's. Stop, and say \
                             that the owner should take over.",
                            describe(&facts)
                        ),
                        s,
                    ));
                }
                if !facts.editable {
                    return Err(refuse(
                        Layer::Target,
                        format!("{} is not a field you can type in.", describe(&facts)),
                        s,
                    ));
                }
                if self.redact(&text) != text {
                    return Err(refuse(
                        Layer::Rule,
                        "that text contains a secret (Plenipo hides it). Plenipo never types \
                         secrets; if a website needs one, the owner enters it.",
                        s,
                    ));
                }
                let what = describe(&facts);
                let mut p = base(
                    auto,
                    format!(
                        "type \"{}\" into {what} on {}{}",
                        cap(&text, 60),
                        host_of(&tab.url()),
                        if submit { ", then send the form" } else { "" }
                    ),
                    format!("{what} ({reference}) on {}: {}", tab.url(), cap(&text, 500)),
                    ControlWork::Type {
                        reference,
                        text,
                        enter: submit,
                        what,
                    },
                );
                p.inherent_owned = submit.then(|| classify::submit(&facts));
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserPress { key } => {
                let s = format!("press {key}");
                let tab = self.open_tab(grant_id, &s)?;
                let focused = tab.focused().await.unwrap_or_default();
                if focused.captcha {
                    if focused.solved {
                        return Err(refuse(
                            Layer::Target,
                            "the keyboard is in a CAPTCHA (a check that a person is using the \
                             site) that is passed already: there is nothing to answer there. \
                             Go on with the page.",
                            s,
                        ));
                    }
                    if !self.captcha_tries_allowed() {
                        return Err(refuse(
                            Layer::Rule,
                            "the keyboard is in a CAPTCHA (a check that a person is using the \
                             site), and the owner has not switched on handing these checks to \
                             them. Do not try to answer it: stop here, and say in your answer \
                             that the owner should take over.",
                            s,
                        ));
                    }
                    let tries = tab.captcha_attempts();
                    if tries >= CAPTCHA_TRIES {
                        return Err(refuse(
                            Layer::Rule,
                            format!(
                                "the keyboard is in a CAPTCHA (a check that a person is using the \
                                 site), and you have tried it {tries} times: that is the limit. \
                                 Stop trying: call browser_person_check to hand it to the owner, \
                                 or stop and say that the owner should take over."
                            ),
                            s,
                        ));
                    }
                }
                // Enter or Space on the CAPTCHA submits an answer: that is one try, counted
                // when it happens (ADR-029). Other keys only move around inside it.
                let captcha = focused.captcha && matches!(key.as_str(), "Enter" | "Space");
                let mut p = base(
                    auto,
                    format!("press {key} on {}", host_of(&tab.url())),
                    format!("{key} in {} on {}", describe(&focused), tab.url()),
                    ControlWork::Press {
                        key: key.clone(),
                        captcha,
                    },
                );
                // Enter sends from any text box; Space presses a focused button. On a page with
                // a live connection, either may send something the gate cannot see (ADR-035).
                let acts = match key.as_str() {
                    "Enter" => true,
                    "Space" => focused.found && !focused.editable,
                    _ => false,
                };
                p.inherent_owned = match key.as_str() {
                    "Enter" => classify::enter(&focused),
                    "Space" if acts => classify::click(&focused),
                    _ => None,
                };
                if acts && p.inherent_owned.is_none() && tab.has_websocket() {
                    p.inherent_owned = Some(live_connection());
                }
                p.site = Self::tab_site(&tab);
                p
            }
            Action::BrowserSelect { reference, option } => {
                let s = format!("choose in {reference}");
                let tab = self.open_tab(grant_id, &s)?;
                let facts = self.control_facts(&tab, &reference, &s).await?;
                if facts.tag != "select" {
                    return Err(refuse(
                        Layer::Target,
                        format!("{} is not a list to choose from.", describe(&facts)),
                        s,
                    ));
                }
                let mut p = base(
                    auto,
                    format!(
                        "choose \"{}\" in {} on {}",
                        cap(&option, 60),
                        describe(&facts),
                        host_of(&tab.url())
                    ),
                    format!("{option} ({reference}) on {}", tab.url()),
                    ControlWork::Select { reference, option },
                );
                p.site = Self::tab_site(&tab);
                p
            }
            Action::ScreenView => base(
                Capability::ComputerObserve,
                "see the screen".into(),
                self.inner_desktop().name(),
                ControlWork::ScreenView,
            ),
            Action::ScreenTakeControl { reason } => {
                let mut p = base(
                    Capability::ComputerControl,
                    "take control of the mouse and keyboard".into(),
                    format!("{worker}'s reason: {reason}"),
                    ControlWork::TakeControl {
                        reason: reason.clone(),
                    },
                );
                if !self.has_control(grant_id) {
                    p.inherent_owned = Some((
                        SensitiveKind::DesktopControl,
                        format!(
                            "it lets {worker} use your mouse and keyboard ({})",
                            cap(&reason, 200)
                        ),
                    ));
                }
                p
            }
            Action::ScreenClick {
                x,
                y,
                button,
                double,
                purpose,
            } => {
                let s = format!("click at ({x}, {y}): {purpose}");
                let (sx, sy) = self.screen_point(grant_id, x, y, &s)?;
                let b = match button.as_str() {
                    "right" => Button::Right,
                    "middle" => Button::Middle,
                    _ => Button::Left,
                };
                let mut p = base(
                    Capability::ComputerControl,
                    format!(
                        "{}click at ({x}, {y}) on the screen: {}",
                        if double { "double-" } else { "" },
                        cap(&purpose, 120)
                    ),
                    format!("screen point ({sx}, {sy}); purpose: {purpose}"),
                    ControlWork::ScreenClick {
                        x: sx,
                        y: sy,
                        button: b,
                        count: if double { 2 } else { 1 },
                    },
                );
                p.inherent_owned = classify::purpose(&purpose);
                p
            }
            Action::ScreenType { text, purpose } => {
                let s = format!("type on the screen: {purpose}");
                self.need_control(grant_id, &s)?;
                if self.redact(&text) != text {
                    return Err(refuse(
                        Layer::Rule,
                        "that text contains a secret (Plenipo hides it). Plenipo never types \
                         secrets; the owner enters them.",
                        s,
                    ));
                }
                let mut p = base(
                    Capability::ComputerControl,
                    format!(
                        "type \"{}\" on the screen: {}",
                        cap(&text, 60),
                        cap(&purpose, 120)
                    ),
                    format!("{}\npurpose: {purpose}", cap(&text, 500)),
                    ControlWork::ScreenType { text: text.clone() },
                );
                p.inherent_owned = if text.contains(['\n', '\r']) {
                    Some((
                        SensitiveKind::Outbound,
                        "a new line presses Enter, which can send or submit something".into(),
                    ))
                } else {
                    classify::purpose(&purpose)
                };
                p
            }
            Action::ScreenKeys { keys, purpose } => {
                let s = format!("press {keys}: {purpose}");
                self.need_control(grant_id, &s)?;
                let parts = parse_keys(&keys).map_err(|e| refuse(Layer::Target, e, s.clone()))?;
                let mut p = base(
                    Capability::ComputerControl,
                    format!("press {keys} on the screen: {}", cap(&purpose, 120)),
                    format!("{keys}; purpose: {purpose}"),
                    ControlWork::ScreenKeys {
                        keys: parts.clone(),
                    },
                );
                // Ctrl+M and Ctrl+J are Enter in a terminal.
                let enter = parts.contains(&KeyPart::Enter)
                    || (parts.contains(&KeyPart::Ctrl)
                        && matches!(parts.last(), Some(KeyPart::Char('m' | 'j'))));
                p.inherent_owned = if enter {
                    Some((
                        SensitiveKind::Outbound,
                        "pressing Enter can send or submit something".into(),
                    ))
                } else {
                    classify::purpose(&purpose)
                };
                p
            }
            Action::ScreenScroll {
                at,
                amount,
                purpose,
            } => {
                let s = format!("scroll the screen: {purpose}");
                let at = match at {
                    Some((x, y)) => Some(self.screen_point(grant_id, x, y, &s)?),
                    None => {
                        self.need_control(grant_id, &s)?;
                        None
                    }
                };
                base(
                    Capability::ComputerControl,
                    format!("scroll the screen by {amount}: {}", cap(&purpose, 120)),
                    format!("{amount} lines; purpose: {purpose}"),
                    ControlWork::ScreenScroll { at, lines: amount },
                )
            }
            Action::ScreenRelease => base(
                Capability::ComputerControl,
                "give the mouse and keyboard back".into(),
                String::new(),
                ControlWork::Release,
            ),
            _ => unreachable!("only browser and screen actions come here"),
        })
    }

    fn has_control(&self, grant_id: &str) -> bool {
        self.state()
            .grants
            .get(grant_id)
            .is_some_and(|g| g.desktop.control.is_some())
    }

    fn need_control(&self, grant_id: &str, summary: &str) -> std::result::Result<(), Refused> {
        if self.has_control(grant_id) {
            Ok(())
        } else {
            Err(refuse(
                Layer::Target,
                "take control first with screen_take_control, giving your reason (the owner is \
                 asked).",
                summary,
            ))
        }
    }

    /// A point of screen_view's picture, in screen pixels.
    fn screen_point(
        &self,
        grant_id: &str,
        x: u32,
        y: u32,
        summary: &str,
    ) -> std::result::Result<(i32, i32), Refused> {
        self.need_control(grant_id, summary)?;
        let view = self
            .state()
            .grants
            .get(grant_id)
            .and_then(|g| g.desktop.view);
        let Some((scale, w, h)) = view else {
            return Err(refuse(
                Layer::Target,
                "look at the screen first with screen_view, then give a point in its picture.",
                summary,
            ));
        };
        if x >= w || y >= h {
            return Err(refuse(
                Layer::Target,
                format!("({x}, {y}) is outside the screen's picture ({w}×{h})."),
                summary,
            ));
        }
        Ok((
            (f64::from(x) * scale).round() as i32,
            (f64::from(y) * scale).round() as i32,
        ))
    }

    fn inner_desktop(&self) -> Arc<dyn crate::desktop::Desktop> {
        self.inner
            .desktop
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// A screenshot of the grant's page for an approval card, kept as evidence.
    pub(super) async fn approval_shot(
        &self,
        grant_id: &str,
        task_id: &str,
        worker: &str,
    ) -> Option<String> {
        let tab = self.grant_tab(grant_id)?;
        tab.broken().is_none().then_some(())?;
        self.keep_page(&tab, task_id, worker, "waiting for your approval")
            .await
    }

    /// Keep a screenshot of the tab's page as evidence; its artifact ID.
    async fn keep_page(
        &self,
        tab: &Tab,
        task_id: &str,
        worker: &str,
        what: &str,
    ) -> Option<String> {
        let bytes = tab.screenshot().await.ok()?;
        self.keep(
            task_id,
            &bytes,
            json!({
                "kind": "browser",
                "worker": worker,
                "action": what,
                "url": tab.url(),
            }),
        )
    }

    /// Keep a picture as evidence, unless it is a step's picture and the owner turned
    /// screenshots off (ADR-023): approval cards always keep theirs.
    fn keep(&self, task_id: &str, bytes: &[u8], metadata: Value) -> Option<String> {
        let for_approval = metadata["action"] == "waiting for your approval";
        if !for_approval && !self.keeps_step_pictures() {
            return None;
        }
        match self
            .inner
            .evidence
            .save(self.ledger(), task_id, bytes, &metadata)
        {
            Ok(a) => Some(a.id),
            Err(e) => {
                self.notice(format!("A screenshot could not be kept: {e}"));
                None
            }
        }
    }

    /// The owner keeps a screenshot of every significant step (Settings → Switches).
    fn keeps_step_pictures(&self) -> bool {
        self.inner
            .guard
            .config()
            .map_or(true, |c| c.switches.screenshots)
    }

    /// The grant's tab, opened (and the browser started) when needed; and a note for the
    /// worker when its earlier tab or the browser had stopped.
    async fn ensure_tab(
        &self,
        ctx: &CallContext<'_>,
    ) -> std::result::Result<(Arc<Tab>, Option<String>), String> {
        let mut note = None;
        if let Some(tab) = self.grant_tab(ctx.grant_id) {
            match tab.broken() {
                None => return Ok((tab, None)),
                Some(why) => {
                    note = Some(format!(
                        "Your earlier tab is gone ({why}), so a new one was opened; the page you \
                         were on is not in it."
                    ));
                    let _ = self.ledger().append_event(NewEvent {
                        task_id: Some(ctx.task_id.into()),
                        source: GUARD.into(),
                        event_type: "browser.tab_lost".into(),
                        payload: json!({ "grantId": ctx.grant_id, "worker": ctx.worker, "why": why }),
                        ..NewEvent::default()
                    });
                }
            }
        }
        let config = self.inner.guard.config().map_err(|e| e.to_string())?;
        self.inner.browser.set_choice(config.browser_choice);
        let rules = config.websites;
        let approved: HashSet<String> = self
            .state()
            .grants
            .get(ctx.grant_id)
            .map(|g| g.approved_sites.clone())
            .unwrap_or_default();
        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        let grant = ctx.grant_id.to_owned();
        let signals = Arc::new(move |_target: &str, signal: Signal| {
            let Some(inner) = weak.upgrade() else { return };
            let broker = Broker { inner };
            let grant = grant.clone();
            tokio::spawn(async move {
                let why = match signal {
                    Signal::TakeOver => "you pressed Take over on the page",
                    Signal::OwnerInput => "you clicked or typed in the page",
                };
                let _ = broker
                    .take_over(&session_id(ControlKind::Browser, &grant), why)
                    .await;
            });
        });
        let (tab, start) = self
            .inner
            .browser
            .new_tab(ctx.worker, SitePolicy { rules, approved }, signals)
            .await?;
        let tab = Arc::new(tab);
        if start != Start::Running {
            let status = self.inner.browser.status().await;
            let _ = self.ledger().append_event(NewEvent {
                task_id: Some(ctx.task_id.into()),
                source: GUARD.into(),
                event_type: "browser.started".into(),
                payload: json!({
                    "browser": status.name,
                    "restarted": start == Start::Restarted,
                    "profile": status.profile,
                }),
                ..NewEvent::default()
            });
            if start == Start::Restarted {
                note = Some(
                    "Plenipo's browser had stopped (it crashed or was closed) and was started \
                     again, so the page you were on is gone."
                        .into(),
                );
            }
        }
        let keep = {
            let mut s = self.state();
            match s.grants.get_mut(ctx.grant_id) {
                Some(g) => {
                    g.tab = Some(Arc::clone(&tab));
                    true
                }
                None => false,
            }
        };
        if !keep {
            tab.close().await;
            return Err("this task step has ended; its tools are closed".into());
        }
        let id = session_id(ControlKind::Browser, ctx.grant_id);
        if self.inner.control.session(&id).is_none() {
            self.inner.control.begin(
                ControlKind::Browser,
                ctx.grant_id,
                ctx.task_id,
                ctx.worker,
                None,
            );
            let _ = self.ledger().append_event(NewEvent {
                task_id: Some(ctx.task_id.into()),
                source: GUARD.into(),
                event_type: "control.started".into(),
                payload: json!({ "kind": "browser", "grantId": ctx.grant_id, "worker": ctx.worker }),
                ..NewEvent::default()
            });
        }
        Ok((tab, note))
    }

    /// A worker's try at the page's CAPTCHA just happened (ADR-029): count it, watch the check
    /// for its verdict, and put both in words (ADR-032).
    async fn captcha_tried(&self, tab: &Tab) -> Vec<String> {
        let used = tab.count_captcha_try();
        let left = CAPTCHA_TRIES.saturating_sub(used);
        let next = if left == 0 {
            "That was your last try: if the check is still there, hand it to the owner with \
             browser_person_check."
                .to_owned()
        } else {
            format!(
                "You may try {left} more time{}; after that, hand it to the owner with \
                 browser_person_check.",
                if left == 1 { "" } else { "s" }
            )
        };
        let mut lines = vec![format!(
            "That was try {used} of {CAPTCHA_TRIES} on the CAPTCHA (a check that a person is \
             using the site)."
        )];
        lines.push(match tab.captcha_verdict(CAPTCHA_VERDICT_WAIT).await {
            Some(s) if s.solved => "The check is passed: the website has its answer, so do not \
                                    click it again. Go on with the page (send the form, or read \
                                    it again with browser_read)."
                .to_owned(),
            Some(s) if s.challenge => format!(
                "The check now shows a puzzle (pictures to pick) inside its own frame, which \
                 Plenipo cannot list as controls. Take a screenshot (browser_screenshot) to see \
                 it; if you cannot answer it there, hand it to the owner with \
                 browser_person_check rather than using up your tries. {next}"
            ),
            Some(s) if !s.present => {
                "The check is gone from the page: read the page again with browser_read.".to_owned()
            }
            Some(_) => format!(
                "The check is still there and not passed yet. Read the page again with \
                 browser_read before another try. {next}"
            ),
            None => format!(
                "The page is changing: read it again with browser_read to see whether the check \
                 passed. {next}"
            ),
        });
        lines
    }

    /// Data a page sent after a worker's action: let it go when the action was approved, ask
    /// the owner otherwise. What to tell the worker.
    /// Hand a check that a person is using the site (a CAPTCHA) to the owner (ADR-023): the page
    /// comes to the front with a sign asking the owner to solve it, and the worker waits for the
    /// owner's answer. The worker has already tried the check its few allowed times (ADR-029);
    /// the owner's own clicks there do not count as taking over.
    async fn person_check(&self, ctx: &CallContext<'_>) -> ControlDone {
        let Some(tab) = self.grant_tab(ctx.grant_id) else {
            return ControlDone {
                result: Err("Your tab is gone; open the page again.".into()),
                ..ControlDone::default()
            };
        };
        let tried = tab.captcha_attempts();
        let times = if tried == 1 { "time" } else { "times" };
        tab.release_to(Mode::Handed).await;
        let url = tab.url();
        let host = host_of(&url);
        self.inner.control.note(
            &session_id(ControlKind::Browser, ctx.grant_id),
            Some(url.clone()),
            Some("waiting for you to solve a person check".into()),
        );
        let prepared = Prepared {
            capability: Capability::BrowserNavigate,
            risk: Risk::Web,
            summary: format!("hand you a check that a person is using {host} (a CAPTCHA)"),
            detail: format!(
                "Plenipo's browser shows the page in front of other windows: {url}\nSolve the \
                 check yourself, then press Approve. {}",
                if tried > 0 {
                    format!("The worker tried it {tried} {times} and could not get past it.")
                } else {
                    "The worker did not try it.".to_owned()
                }
            ),
            files: Vec::new(),
            writes_git_dir: false,
            command: None,
            script: None,
            inherent: None,
            inherent_owned: None,
            site: Site::parse(&url).ok(),
            screenshot: self
                .keep_page(&tab, ctx.task_id, ctx.worker, "waiting for your approval")
                .await,
            server: None,
            harmless: false,
            work: Work::Missing(String::new()),
        };
        let decision = Decision {
            verdict: Verdict::Ask,
            reason: format!(
                "{host} asks whether a person is using it. {}: solve it yourself in Plenipo's \
                 browser (it is in front now), then press Approve and {} continues; Deny stops \
                 it.",
                if tried > 0 {
                    format!("The worker tried the check {tried} {times} and could not get past it")
                } else {
                    "The worker did not try the check".to_owned()
                },
                ctx.worker
            ),
            layer: Layer::Rule,
            risk: Risk::Web,
            sensitive: None,
            checks: Vec::new(),
        };
        let detail = self.redact(&prepared.detail);
        let minutes = self
            .inner
            .guard
            .config()
            .map(|c| c.options.approval_minutes)
            .unwrap_or(10);
        let answer = self
            .ask(
                ctx.grant_id,
                ctx.task_id,
                ctx.runtime_id,
                ctx.worker,
                ctx.scope,
                ctx.workspace,
                ctx.tool,
                &prepared,
                &detail,
                &decision,
                minutes,
            )
            .await;
        let solved = matches!(answer, Ok((_, ApprovalState::Approved)));
        // Back to the worker, unless the owner took over or stopped it meanwhile. The owner was
        // just in the loop: the worker's tries start over (ADR-029).
        tab.take_back().await;
        tab.clear_captcha_attempts();
        let result = if solved && tab.mode() == Mode::Worker {
            Ok(format!(
                "The owner solved the check on {host}. Read the page again with browser_read and \
                 continue."
            ))
        } else {
            Err(format!(
                "The owner did not solve the check on {host}. Stop here, and say in your answer \
                 that this page needs a person."
            ))
        };
        let url_now = tab.url();
        ControlDone {
            result,
            images: Vec::new(),
            screenshot: None,
            url: Some(url_now),
        }
    }

    async fn decide_held(
        &self,
        ctx: &CallContext<'_>,
        tab: &Tab,
        held: Vec<Held>,
        what: &str,
    ) -> Option<String> {
        if held.is_empty() {
            return None;
        }
        let sites: Vec<String> = {
            let mut s: Vec<String> = held.iter().map(|h| h.site.clone()).collect();
            s.dedup();
            s
        };
        let listed: Vec<String> = held
            .iter()
            .map(|h| {
                format!(
                    "{} {} ({})",
                    h.method,
                    cap(&h.url, 200),
                    if h.kind == "Document" {
                        "a form"
                    } else {
                        "sent by the page's script"
                    }
                )
            })
            .collect();
        if ctx.approved {
            tab.release(&held, true).await;
            return Some(format!(
                "The page sent it to {} (you had the owner's approval).",
                sites.join(", ")
            ));
        }
        // The owner's rules for sending (ADR-023): blocked never sends; the "send without asking"
        // switch lets it go when every address is on the allowed websites list.
        if let Ok(config) = self.inner.guard.config() {
            if config.sensitive_rule(SensitiveKind::Outbound) == SensitiveRule::Block {
                tab.release(&held, false).await;
                return Some(format!(
                    "Not sent: the page tried to send data to {}, and the owner set \"{}\" to \
                     blocked. Do not try another way; say in your answer what you needed to send.",
                    sites.join(", "),
                    SensitiveKind::Outbound.label()
                ));
            }
            let all_allowed = held.iter().all(|h| {
                Site::parse(&h.url).is_ok_and(|s| {
                    matches!(
                        plenipo_guard::websites::check(&config.websites, &s),
                        SiteVerdict::Allowed(_)
                    )
                })
            });
            if config.switches.send_without_asking && all_allowed {
                tab.release(&held, true).await;
                return Some(format!(
                    "The page sent it to {} (the owner lets workers send on allowed websites \
                     without asking).",
                    sites.join(", ")
                ));
            }
        }
        let summary = format!(
            "let the page send data to {} after {what}",
            sites.join(", ")
        );
        let prepared = Prepared {
            capability: Capability::BrowserAutomate,
            risk: Risk::Web,
            summary: summary.clone(),
            detail: listed.join("\n"),
            files: Vec::new(),
            writes_git_dir: false,
            command: None,
            script: None,
            inherent: None,
            inherent_owned: None,
            site: Site::parse(&tab.url()).ok(),
            screenshot: self
                .keep_page(tab, ctx.task_id, ctx.worker, "waiting for your approval")
                .await,
            server: None,
            harmless: false,
            work: Work::Missing(String::new()),
        };
        let decision = Decision {
            verdict: Verdict::Ask,
            reason: format!(
                "Sending data to a website needs your approval: after {what}, the page is sending \
                 data to {} ({}).",
                sites.join(", "),
                SensitiveKind::Outbound.label()
            ),
            layer: Layer::Risk,
            risk: Risk::Web,
            sensitive: Some(SensitiveKind::Outbound),
            checks: Vec::new(),
        };
        let detail = self.redact(&prepared.detail);
        let minutes = self
            .inner
            .guard
            .config()
            .map(|c| c.options.approval_minutes)
            .unwrap_or(10);
        let answer = self
            .ask(
                ctx.grant_id,
                ctx.task_id,
                ctx.runtime_id,
                ctx.worker,
                ctx.scope,
                ctx.workspace,
                ctx.tool,
                &prepared,
                &detail,
                &decision,
                minutes,
            )
            .await;
        match answer {
            Ok((_, ApprovalState::Approved)) if tab.mode() == Mode::Worker => {
                tab.release(&held, true).await;
                Some(format!(
                    "The owner approved: the page sent it to {}.",
                    sites.join(", ")
                ))
            }
            _ => {
                tab.release(&held, false).await;
                Some(format!(
                    "Not sent: the owner did not approve the page sending data to {}. Do not try \
                     another way; say in your answer what you needed to send and why.",
                    sites.join(", ")
                ))
            }
        }
    }

    /// Carry out a browser or screen call Guard allowed (or the owner approved).
    pub(super) async fn carry_out_control(
        &self,
        ctx: &CallContext<'_>,
        work: ControlWork,
    ) -> ControlDone {
        match work {
            ControlWork::Open { url, timeout } => self.open_page(ctx, &url, timeout).await,
            ControlWork::Release => {
                self.release_control(ctx.grant_id, "the worker gave it back")
                    .await
            }
            ControlWork::ScreenView => self.screen_view(ctx).await,
            ControlWork::TakeControl { reason } => self.take_control(ctx, &reason),
            ControlWork::PersonCheck => self.person_check(ctx).await,
            w @ (ControlWork::ScreenClick { .. }
            | ControlWork::ScreenType { .. }
            | ControlWork::ScreenKeys { .. }
            | ControlWork::ScreenScroll { .. }) => self.screen_act(ctx, w).await,
            browser => self.page_act(ctx, browser).await,
        }
    }

    async fn open_page(
        &self,
        ctx: &CallContext<'_>,
        url: &str,
        timeout: Option<u64>,
    ) -> ControlDone {
        let (tab, note) = match self.ensure_tab(ctx).await {
            Ok(t) => t,
            Err(e) => {
                return ControlDone {
                    result: Err(format!("Plenipo's browser could not open the page: {e}")),
                    ..ControlDone::default()
                }
            }
        };
        if let Ok(c) = self.inner.guard.config() {
            tab.set_rules(c.websites);
        }
        let limit = timeout.map_or(
            self.inner.browser.config().limits.navigation,
            Duration::from_secs,
        );
        let opened = tab.navigate(url, limit).await;
        let mut lines: Vec<String> = note.into_iter().collect();
        let result = match opened {
            Err(e) => Err(format!("The page did not open: {e}")),
            Ok(o) => {
                if o.timed_out {
                    lines.insert(
                        0,
                        format!(
                            "The page did not finish loading within {} seconds; Plenipo stopped \
                             it. What loaded so far is below.",
                            limit.as_secs()
                        ),
                    );
                } else {
                    lines.insert(0, format!("Opened \"{}\" ({}).", o.title, o.url));
                }
                lines.extend(tab.take_notes());
                match tab.read(2000, 40).await {
                    Ok(page) => {
                        lines.push(page_text(
                            &page,
                            true,
                            tab.captcha_attempts(),
                            self.captcha_tries_allowed(),
                        ));
                        if o.timed_out {
                            Err(lines.join("\n"))
                        } else {
                            Ok(lines.join("\n"))
                        }
                    }
                    Err(e) => Err(format!("The page opened but could not be read: {e}")),
                }
            }
        };
        let url_now = tab.url();
        self.inner.control.note(
            &session_id(ControlKind::Browser, ctx.grant_id),
            Some(url_now.clone()),
            Some(format!("opened {}", host_of(&url_now))),
        );
        let screenshot = self
            .keep_page(&tab, ctx.task_id, ctx.worker, "opened the page")
            .await;
        ControlDone {
            result,
            images: Vec::new(),
            screenshot,
            url: Some(url_now),
        }
    }

    async fn page_act(&self, ctx: &CallContext<'_>, work: ControlWork) -> ControlDone {
        let Some(tab) = self.grant_tab(ctx.grant_id) else {
            return ControlDone {
                result: Err("No page is open: open one with browser_open.".into()),
                ..ControlDone::default()
            };
        };
        if let Ok(c) = self.inner.guard.config() {
            tab.set_rules(c.websites);
        }
        let mut images = Vec::new();
        let (result, last, keep): (std::result::Result<String, String>, String, bool) = match work {
            ControlWork::Read { max_chars } => (
                tab.read(max_chars, MAX_CONTROLS).await.map(|p| {
                    let mut text = page_text(
                        &p,
                        true,
                        tab.captcha_attempts(),
                        self.captcha_tries_allowed(),
                    );
                    // What Plenipo stopped since the last result (a send the page tried on its
                    // own), after the page's own words (ADR-035).
                    for note in tab.take_notes() {
                        text.push_str(&note);
                        text.push('\n');
                    }
                    text
                }),
                format!("read {}", host_of(&tab.url())),
                false,
            ),
            ControlWork::Screenshot => match tab.screenshot().await {
                Ok(bytes) => {
                    let (url, title) = tab.where_now().await;
                    let words = tab
                        .read(600, 0)
                        .await
                        .map(|p| {
                            page_text(
                                &p,
                                false,
                                tab.captcha_attempts(),
                                self.captcha_tries_allowed(),
                            )
                        })
                        .unwrap_or_default();
                    images.push(Image {
                        mime: screens::mime_of(&bytes).into(),
                        data: bytes.clone(),
                    });
                    let id = self.keep(ctx.task_id, &bytes, json!({ "kind": "browser", "worker": ctx.worker, "action": "took a screenshot", "url": url }));
                    return ControlDone {
                        result: Ok(format!(
                            "A screenshot of \"{title}\" ({url}) is attached. In words:\n{words}"
                        )),
                        images,
                        screenshot: id,
                        url: Some(url),
                    };
                }
                Err(e) => (Err(format!("No screenshot: {e}")), String::new(), false),
            },
            ControlWork::Scroll { dy } => (
                tab.scroll(dy).await.map(|v| {
                    format!(
                        "Scrolled to {} of {} pixels.",
                        v["y"].as_i64().unwrap_or(0),
                        v["height"].as_i64().unwrap_or(0)
                    )
                }),
                "scrolled".into(),
                false,
            ),
            ControlWork::Back => {
                let limit = self.inner.browser.config().limits.navigation;
                (
                    tab.back(limit)
                        .await
                        .map(|o| format!("Back on \"{}\" ({}).", o.title, o.url)),
                    "went back".into(),
                    true,
                )
            }
            ControlWork::Click { facts, what } => {
                let r = match tab.click(&facts).await {
                    Ok(settled) => {
                        let held = self
                            .decide_held(ctx, &tab, settled.held, &format!("clicking {what}"))
                            .await;
                        let (url, title) = tab.where_now().await;
                        let mut t = vec![format!("Clicked {what}. Now on \"{title}\" ({url}).")];
                        t.extend(held);
                        if facts.captcha {
                            t.extend(self.captcha_tried(&tab).await);
                        }
                        t.extend(tab.take_notes());
                        Ok(t.join("\n"))
                    }
                    Err(e) => Err(format!("The click did not happen: {e}")),
                };
                (r, format!("clicked {what}"), true)
            }
            ControlWork::Type {
                reference,
                text,
                enter,
                what,
            } => {
                let r = match tab.type_text(&reference, &text, enter).await {
                    Ok(settled) => {
                        let held = self
                            .decide_held(ctx, &tab, settled.held, &format!("typing into {what}"))
                            .await;
                        let (url, title) = tab.where_now().await;
                        let mut t = vec![format!(
                            "Typed into {what}{}. Now on \"{title}\" ({url}).",
                            if enter { " and pressed Enter" } else { "" }
                        )];
                        t.extend(held);
                        t.extend(tab.take_notes());
                        Ok(t.join("\n"))
                    }
                    Err(e) => Err(format!("Nothing was typed: {e}")),
                };
                (r, format!("typed into {what}"), true)
            }
            ControlWork::Press { key, captcha } => {
                let r = match tab.press(&key).await {
                    Ok(settled) => {
                        let held = self
                            .decide_held(ctx, &tab, settled.held, &format!("pressing {key}"))
                            .await;
                        let (url, title) = tab.where_now().await;
                        let mut t = vec![format!("Pressed {key}. Now on \"{title}\" ({url}).")];
                        t.extend(held);
                        if captcha {
                            t.extend(self.captcha_tried(&tab).await);
                        }
                        t.extend(tab.take_notes());
                        Ok(t.join("\n"))
                    }
                    Err(e) => Err(format!("The key was not pressed: {e}")),
                };
                (r, format!("pressed {key}"), true)
            }
            ControlWork::Select { reference, option } => {
                let r = match tab.choose(&reference, &option).await {
                    Ok((answer, settled)) => {
                        let held = self
                            .decide_held(ctx, &tab, settled.held, &format!("choosing \"{option}\""))
                            .await;
                        if answer["ok"] == true {
                            let mut t = vec![format!(
                                "Chose \"{}\".",
                                answer["chosen"].as_str().unwrap_or(&option)
                            )];
                            t.extend(held);
                            Ok(t.join("\n"))
                        } else {
                            Err(format!(
                                "\"{option}\" is not an option there. Options: {}",
                                answer["options"]
                                    .as_array()
                                    .map(|o| o
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(", "))
                                    .unwrap_or_default()
                            ))
                        }
                    }
                    Err(e) => Err(format!("Nothing was chosen: {e}")),
                };
                (r, format!("chose \"{option}\""), true)
            }
            _ => unreachable!("screen work is handled elsewhere"),
        };
        let url = tab.url();
        self.inner.control.note(
            &session_id(ControlKind::Browser, ctx.grant_id),
            Some(url.clone()),
            (!last.is_empty()).then_some(last.clone()),
        );
        let screenshot = if keep && tab.broken().is_none() {
            self.keep_page(&tab, ctx.task_id, ctx.worker, &last).await
        } else {
            None
        };
        ControlDone {
            result,
            images,
            screenshot,
            url: Some(url),
        }
    }

    // ---- The screen ------------------------------------------------------------------------

    async fn capture_screen(
        &self,
    ) -> std::result::Result<(crate::desktop::Frame, crate::desktop::Frame), String> {
        let desktop = self.inner_desktop();
        tokio::task::spawn_blocking(move || {
            let full = desktop.capture()?;
            let small = screens::downscale(&full, screens::MAX_WIDTH);
            Ok((full, small))
        })
        .await
        .unwrap_or_else(|e| Err(format!("the screen could not be captured: {e}")))
    }

    async fn screen_view(&self, ctx: &CallContext<'_>) -> ControlDone {
        let (full, small) = match self.capture_screen().await {
            Ok(f) => f,
            Err(e) => {
                return ControlDone {
                    result: Err(e),
                    ..ControlDone::default()
                }
            }
        };
        let bytes = match screens::png(&small) {
            Ok(b) => b,
            Err(e) => {
                return ControlDone {
                    result: Err(e),
                    ..ControlDone::default()
                }
            }
        };
        let scale = f64::from(full.width) / f64::from(small.width.max(1));
        if let Some(g) = self.state().grants.get_mut(ctx.grant_id) {
            g.desktop.view = Some((scale, small.width, small.height));
        }
        let id = self.keep(
            ctx.task_id,
            &bytes,
            json!({ "kind": "desktop", "worker": ctx.worker, "action": "looked at the screen" }),
        );
        ControlDone {
            result: Ok(format!(
                "A picture of the screen is attached: {}×{} pixels (the screen is {}×{}). Give \
                 points for screen_click and screen_scroll in the picture's pixels.",
                small.width, small.height, full.width, full.height
            )),
            images: vec![Image {
                mime: "image/png".into(),
                data: bytes,
            }],
            screenshot: id,
            url: None,
        }
    }

    fn take_control(&self, ctx: &CallContext<'_>, reason: &str) -> ControlDone {
        if self.has_control(ctx.grant_id) {
            return ControlDone {
                result: Ok("You already have the mouse and keyboard.".into()),
                ..ControlDone::default()
            };
        }
        let desktop = self.inner_desktop();
        let watch = Arc::new(Watch {
            last: Mutex::new(desktop.cursor().ok()),
            acting: AtomicBool::new(false),
            ended: AtomicBool::new(false),
        });
        let taken = match self.state().grants.get_mut(ctx.grant_id) {
            Some(g) => {
                g.desktop.control = Some(Arc::clone(&watch));
                true
            }
            None => false,
        };
        if !taken {
            return ControlDone {
                result: Err("This task step has ended.".into()),
                ..ControlDone::default()
            };
        }
        let id = session_id(ControlKind::Desktop, ctx.grant_id);
        self.inner.control.begin(
            ControlKind::Desktop,
            ctx.grant_id,
            ctx.task_id,
            ctx.worker,
            Some(cap(reason, 300)),
        );
        let _ = self.ledger().append_event(NewEvent {
            task_id: Some(ctx.task_id.into()),
            source: GUARD.into(),
            event_type: "control.started".into(),
            payload: json!({ "kind": "desktop", "grantId": ctx.grant_id, "worker": ctx.worker, "reason": self.redact(reason) }),
            ..NewEvent::default()
        });
        // Watch for the owner's hand on the mouse: it takes control back.
        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let Some(inner) = weak.upgrade() else { return };
                if watch.ended.load(Ordering::SeqCst) {
                    return;
                }
                if watch.acting.load(Ordering::SeqCst) {
                    continue;
                }
                let broker = Broker { inner };
                let desktop = broker.inner_desktop();
                let now = tokio::task::spawn_blocking(move || desktop.cursor())
                    .await
                    .ok()
                    .and_then(Result::ok);
                let last = *lock(&watch.last);
                let moved = match (last, now) {
                    (Some((a, b)), Some((c, d))) => {
                        (a - c).abs() > OWNER_MOVE_PIXELS || (b - d).abs() > OWNER_MOVE_PIXELS
                    }
                    _ => false,
                };
                if moved && !watch.acting.load(Ordering::SeqCst) {
                    let _ = broker.take_over(&id, "you moved the mouse").await;
                    return;
                }
            }
        });
        ControlDone {
            result: Ok(
                "You have the mouse and keyboard. The owner sees a sign while you do, and moving \
                 the mouse takes it back. Give it back with screen_release_control when you are \
                 done."
                    .into(),
            ),
            ..ControlDone::default()
        }
    }

    async fn screen_act(&self, ctx: &CallContext<'_>, work: ControlWork) -> ControlDone {
        let Some(watch) = self
            .state()
            .grants
            .get(ctx.grant_id)
            .and_then(|g| g.desktop.control.clone())
        else {
            return ControlDone {
                result: Err("You do not have the mouse and keyboard: take control first.".into()),
                ..ControlDone::default()
            };
        };
        // The owner's hand on the mouse since the last action takes control back.
        let desktop = self.inner_desktop();
        let d = Arc::clone(&desktop);
        let now = tokio::task::spawn_blocking(move || d.cursor())
            .await
            .ok()
            .and_then(Result::ok);
        let last = *lock(&watch.last);
        if let (Some((a, b)), Some((c, e))) = (last, now) {
            if (a - c).abs() > OWNER_MOVE_PIXELS || (b - e).abs() > OWNER_MOVE_PIXELS {
                let _ = self
                    .take_over(
                        &session_id(ControlKind::Desktop, ctx.grant_id),
                        "you moved the mouse",
                    )
                    .await;
                return ControlDone {
                    result: Err("The owner moved the mouse, which takes control back: do not use the mouse and keyboard again. Say in your answer where you were and what is left.".into()),
                    ..ControlDone::default()
                };
            }
        }
        watch.acting.store(true, Ordering::SeqCst);
        let id = session_id(ControlKind::Desktop, ctx.grant_id);
        let control = self.inner.control.clone();
        let (d, w) = (Arc::clone(&desktop), Arc::clone(&watch));
        let (done, last): (std::result::Result<String, String>, String) = tokio::task::spawn_blocking(move || {
            let still = || control.session(&id).is_some_and(|s| s.state == ControlState::Active) && !w.ended.load(Ordering::SeqCst);
            match work {
                ControlWork::ScreenClick { x, y, button, count } => {
                    let r = d.move_to(x, y).and_then(|()| d.click(button, count));
                    (r.map(|()| format!("Clicked at ({x}, {y}) on the screen.")), "clicked on the screen".to_owned())
                }
                ControlWork::ScreenType { text } => {
                    let chars: Vec<char> = text.chars().collect();
                    let mut typed = 0;
                    let mut r = Ok(());
                    for piece in chars.chunks(16) {
                        if !still() {
                            r = Err(format!("Stopped after {typed} characters: the owner took control back or stopped it."));
                            break;
                        }
                        let s: String = piece.iter().collect();
                        if let Err(e) = d.type_text(&s) {
                            r = Err(e);
                            break;
                        }
                        typed += piece.len();
                    }
                    (r.map(|()| format!("Typed {typed} characters.")), "typed on the screen".to_owned())
                }
                ControlWork::ScreenKeys { keys } => (
                    d.keys(&keys).map(|()| "Pressed the keys.".to_owned()),
                    "pressed keys".to_owned(),
                ),
                ControlWork::ScreenScroll { at, lines } => {
                    let r = at.map_or(Ok(()), |(x, y)| d.move_to(x, y)).and_then(|()| d.scroll(lines));
                    (r.map(|()| format!("Scrolled {lines}.")), "scrolled the screen".to_owned())
                }
                _ => (Err("not a screen action".into()), String::new()),
            }
        })
        .await
        .unwrap_or_else(|e| (Err(format!("the action stopped unexpectedly: {e}")), String::new()));
        let d = Arc::clone(&desktop);
        let after = tokio::task::spawn_blocking(move || d.cursor())
            .await
            .ok()
            .and_then(Result::ok);
        *lock(&watch.last) = after;
        watch.acting.store(false, Ordering::SeqCst);
        self.inner.control.note(
            &session_id(ControlKind::Desktop, ctx.grant_id),
            None,
            Some(last.clone()),
        );
        // Evidence: the screen after the action.
        let screenshot = match self.capture_screen().await {
            Ok((_, small)) => screens::png(&small).ok().and_then(|b| {
                self.keep(
                    ctx.task_id,
                    &b,
                    json!({ "kind": "desktop", "worker": ctx.worker, "action": last }),
                )
            }),
            Err(_) => None,
        };
        ControlDone {
            result: done,
            images: Vec::new(),
            screenshot,
            url: None,
        }
    }

    /// End a grant's hold on the mouse and keyboard.
    async fn release_control(&self, grant_id: &str, why: &str) -> ControlDone {
        let watch = self
            .state()
            .grants
            .get_mut(grant_id)
            .and_then(|g| g.desktop.control.take());
        let Some(watch) = watch else {
            return ControlDone {
                result: Ok("You did not have the mouse and keyboard.".into()),
                ..ControlDone::default()
            };
        };
        watch.ended.store(true, Ordering::SeqCst);
        let desktop = self.inner_desktop();
        let _ = tokio::task::spawn_blocking(move || desktop.release_all()).await;
        if let Some(s) = self
            .inner
            .control
            .end(&session_id(ControlKind::Desktop, grant_id))
        {
            let _ = self.ledger().append_event(NewEvent {
                task_id: Some(s.task_id),
                source: GUARD.into(),
                event_type: "control.ended".into(),
                payload: json!({ "kind": "desktop", "grantId": grant_id, "worker": s.worker, "why": why }),
                ..NewEvent::default()
            });
        }
        ControlDone {
            result: Ok("The mouse and keyboard are the owner's again.".into()),
            ..ControlDone::default()
        }
    }

    // ---- The owner's controls ----------------------------------------------------------------

    pub fn control_status(&self) -> ControlStatus {
        self.inner.control.status()
    }

    /// Be told about every change of control (the app's banner, tray, and indicator window).
    pub fn set_control_listener(&self, listener: crate::control::ControlListener) {
        self.inner.control.set_listener(listener);
    }

    /// The owner takes over a worker's use of the browser or the desktop: that worker stops.
    pub async fn take_over(&self, session: &str, why: &str) -> Result<ControlStatus> {
        let s = self
            .inner
            .control
            .take_over(session)
            .ok_or_else(|| BrokerError::Invalid("that worker is not using it now".into()))?;
        // The worker stops: what it is waiting for your approval on is refused.
        let pending: Vec<String> = self
            .state()
            .grants
            .get(&s.grant_id)
            .map(|g| g.pending.iter().cloned().collect())
            .unwrap_or_default();
        for approval in &pending {
            self.settle(
                approval,
                ApprovalState::Rejected,
                "owner",
                "You took over, so the worker stopped.",
            );
        }
        match s.kind {
            ControlKind::Browser => {
                if let Some(tab) = self.grant_tab(&s.grant_id) {
                    tab.release_to(Mode::Owner).await;
                }
            }
            ControlKind::Desktop => {
                let watch = self
                    .state()
                    .grants
                    .get_mut(&s.grant_id)
                    .and_then(|g| g.desktop.control.take());
                if let Some(w) = watch {
                    w.ended.store(true, Ordering::SeqCst);
                }
                let desktop = self.inner_desktop();
                let _ = tokio::task::spawn_blocking(move || desktop.release_all()).await;
            }
            ControlKind::Server => self.stop_servers(&s.grant_id, "you disconnected the worker"),
        }
        self.ledger().append_event(NewEvent {
            task_id: Some(s.task_id.clone()),
            source: "owner".into(),
            event_type: "control.taken_over".into(),
            payload: json!({
                "kind": s.kind,
                "grantId": s.grant_id,
                "worker": s.worker,
                "why": why,
            }),
            ..NewEvent::default()
        })?;
        Ok(self.inner.control.status())
    }

    /// The owner switched a feature off (ADR-023): the workers using it stop now, and what they
    /// wait for is refused. Guard refuses their later calls; unlike the emergency stop, nothing
    /// else changes.
    pub async fn switch_off_control(&self, kind: ControlKind) -> Result<ControlStatus> {
        if kind == ControlKind::Server {
            // The owner's server terminals work only while the switch is on (ADR-031).
            self.close_server_terminals("you switched Remote computers (SSH) off");
        }
        let stopped = self.inner.control.stop_kind(kind);
        for s in &stopped {
            let pending: Vec<String> = self
                .state()
                .grants
                .get(&s.grant_id)
                .map(|g| g.pending.iter().cloned().collect())
                .unwrap_or_default();
            for approval in &pending {
                self.settle(
                    approval,
                    ApprovalState::Rejected,
                    "owner",
                    "You switched this off in Settings, so the worker stopped.",
                );
            }
            match s.kind {
                ControlKind::Browser => {
                    if let Some(tab) = self.grant_tab(&s.grant_id) {
                        tab.release_to(Mode::Stopped).await;
                    }
                }
                ControlKind::Desktop => {
                    let watch = self
                        .state()
                        .grants
                        .get_mut(&s.grant_id)
                        .and_then(|g| g.desktop.control.take());
                    if let Some(w) = watch {
                        w.ended.store(true, Ordering::SeqCst);
                    }
                    let desktop = self.inner_desktop();
                    let _ = tokio::task::spawn_blocking(move || desktop.release_all()).await;
                }
                ControlKind::Server => {
                    self.stop_servers(&s.grant_id, "you switched remote computers (SSH) off");
                }
            }
        }
        if !stopped.is_empty() {
            self.ledger().append_event(NewEvent {
                source: "owner".into(),
                event_type: "control.switched_off".into(),
                payload: json!({
                    "kind": kind,
                    "sessions": stopped.iter().map(|s| json!({ "kind": s.kind, "worker": s.worker, "taskId": s.task_id })).collect::<Vec<_>>(),
                }),
                ..NewEvent::default()
            })?;
        }
        Ok(self.inner.control.status())
    }

    /// The emergency stop: all browser and desktop control halts at once, the workers using them
    /// lose their permissions for the rest of their step, and no worker may take control again
    /// until the owner allows it.
    pub async fn stop_all_control(&self, actor: &str) -> Result<ControlStatus> {
        let stopped = self.inner.control.stop_all();
        let desktop = self.inner_desktop();
        let _ = tokio::task::spawn_blocking(move || desktop.release_all()).await;
        let mut grants: Vec<String> = Vec::new();
        for s in &stopped {
            match s.kind {
                ControlKind::Browser => {
                    if let Some(tab) = self.grant_tab(&s.grant_id) {
                        tab.release_to(Mode::Stopped).await;
                    }
                }
                ControlKind::Desktop => {
                    let watch = self
                        .state()
                        .grants
                        .get_mut(&s.grant_id)
                        .and_then(|g| g.desktop.control.take());
                    if let Some(w) = watch {
                        w.ended.store(true, Ordering::SeqCst);
                    }
                }
                ControlKind::Server => self.stop_servers(&s.grant_id, "you pressed Stop all"),
            }
            if !grants.contains(&s.grant_id) {
                grants.push(s.grant_id.clone());
            }
        }
        for g in &grants {
            let _ = self.revoke(g, actor);
        }
        self.ledger().append_event(NewEvent {
            source: actor.into(),
            event_type: "control.stopped".into(),
            payload: json!({
                "sessions": stopped.iter().map(|s| json!({ "kind": s.kind, "worker": s.worker, "taskId": s.task_id })).collect::<Vec<_>>(),
            }),
            ..NewEvent::default()
        })?;
        Ok(self.inner.control.status())
    }

    /// Allow control again after a stop.
    pub fn allow_control(&self, actor: &str) -> Result<ControlStatus> {
        self.inner.control.allow();
        self.ledger().append_event(NewEvent {
            source: actor.into(),
            event_type: "control.allowed".into(),
            payload: json!({}),
            ..NewEvent::default()
        })?;
        Ok(self.inner.control.status())
    }

    /// Plenipo's browser as Settings shows it.
    pub async fn browser_status(&self) -> crate::browser::BrowserStatus {
        self.sync_browser_choice();
        self.inner.browser.status().await
    }

    /// Keep Plenipo's browser on the owner's choice in Guard's settings (ADR-028).
    fn sync_browser_choice(&self) {
        if let Ok(config) = self.inner.guard.config() {
            self.inner.browser.set_choice(config.browser_choice);
        }
    }

    /// Open Plenipo's browser for the owner (to sign in to a website before workers use it).
    pub async fn open_browser_for_owner(&self, address: Option<&str>) -> Result<()> {
        let url = match address.map(str::trim).filter(|a| !a.is_empty()) {
            Some(a) => {
                let site = Site::parse(a).map_err(BrokerError::Invalid)?;
                site.url
            }
            None => "about:blank".into(),
        };
        self.sync_browser_choice();
        self.inner
            .browser
            .open_for_owner(&url)
            .await
            .map_err(BrokerError::Invalid)?;
        self.ledger().append_event(NewEvent {
            source: "owner".into(),
            event_type: "browser.opened_by_owner".into(),
            payload: json!({ "url": url }),
            ..NewEvent::default()
        })?;
        Ok(())
    }

    /// A kept screenshot's picture, by its artifact ID.
    pub fn screenshot(&self, artifact_id: &str) -> Result<(Vec<u8>, String)> {
        self.inner
            .evidence
            .read(self.ledger(), artifact_id)
            .map_err(BrokerError::Invalid)
    }

    /// A kept screenshot as the app shows it.
    pub fn screenshot_view(&self, artifact_id: &str) -> Result<crate::dto::Screenshot> {
        use base64::Engine as _;
        let (bytes, mime) = self.screenshot(artifact_id)?;
        Ok(crate::dto::Screenshot {
            data_url: format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
            mime,
        })
    }

    /// Plenipo's browser, for tests (to stop it like a crash).
    pub fn browser(&self) -> &crate::browser::Browser {
        &self.inner.browser
    }

    /// Use this screen, mouse, and keyboard (tests use a stand-in).
    pub fn set_desktop(&self, desktop: Arc<dyn crate::desktop::Desktop>) {
        *self
            .inner
            .desktop
            .write()
            .unwrap_or_else(|p| p.into_inner()) = desktop;
    }

    /// A grant's permissions were revoked: its tab says so and stops, and its hold on the mouse
    /// and keyboard ends.
    pub(super) fn stop_grant_control(&self, grant_id: &str) {
        let (tab, watch) = {
            let mut s = self.state();
            match s.grants.get_mut(grant_id) {
                Some(g) => (g.tab.clone(), g.desktop.control.take()),
                None => (None, None),
            }
        };
        for kind in [
            ControlKind::Browser,
            ControlKind::Desktop,
            ControlKind::Server,
        ] {
            self.inner.control.stop(&session_id(kind, grant_id));
        }
        self.stop_servers(grant_id, "the worker's permissions were revoked");
        if let Some(tab) = tab {
            self.spawn(async move { tab.release_to(Mode::Stopped).await });
        }
        if let Some(w) = watch {
            w.ended.store(true, Ordering::SeqCst);
            let d = self.inner_desktop();
            self.spawn(async move {
                let _ = tokio::task::spawn_blocking(move || d.release_all()).await;
            });
        }
    }

    /// A grant ends: its tab closes (unless the owner has it), its hold on the mouse and
    /// keyboard ends, and its control sessions end.
    pub(super) fn end_control(
        &self,
        grant_id: &str,
        task_id: &str,
        worker: &str,
        tab: Option<Arc<Tab>>,
        desktop: Option<Arc<Watch>>,
    ) {
        if let Some(w) = desktop {
            w.ended.store(true, Ordering::SeqCst);
            let d = self.inner_desktop();
            self.spawn(async move {
                let _ = tokio::task::spawn_blocking(move || d.release_all()).await;
            });
        }
        if let Some(tab) = tab {
            if tab.mode() == Mode::Worker {
                self.spawn(async move { tab.close().await });
            }
        }
        for kind in [
            ControlKind::Browser,
            ControlKind::Desktop,
            ControlKind::Server,
        ] {
            if let Some(s) = self.inner.control.end(&session_id(kind, grant_id)) {
                let _ = self.ledger().append_event(NewEvent {
                    task_id: Some(task_id.into()),
                    source: GUARD.into(),
                    event_type: "control.ended".into(),
                    payload: json!({ "kind": kind, "grantId": grant_id, "worker": worker, "state": s.state }),
                    ..NewEvent::default()
                });
            }
        }
    }
}
