//! An organization's window and the panels it pops out (Phase 21, ADR-092).
//!
//! A popped-out panel is the same panel, not a copy: the organization's window opens the pop-out
//! with `window.open` and draws into it itself. Plenipo allows that only right after the page
//! asked for that panel ([`PopOuts::request`]); every other new window is refused. A pop-out has
//! no commands of its own (`capabilities/popout.json`). Plenipo keeps each pop-out's place on
//! screen in `windows.json` in its data folder, and puts it back there next time.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use plenipo_core::{PanelId, PopOutNotice, WindowPlace};
use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowFeatures, NewWindowResponse};
use tauri::{
    AppHandle, Emitter as _, LogicalPosition, LogicalSize, Manager as _, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

/// The first organization's window.
pub const MAIN: &str = "main";
/// Another organization's window starts with this (ADR-094).
pub const ORG_PREFIX: &str = "org-";
/// A pop-out window starts with this: `popout-terminal--main`.
pub const POPOUT_PREFIX: &str = "popout-";
/// Tells an organization's window what happened to its pop-outs.
pub const WINDOWS_EVENT: &str = "plenipo://windows";
/// The file of pop-out places, in the data folder.
pub const PLACES_FILE: &str = "windows.json";
/// A request lasts this long: the page opens the window right after asking.
const REQUEST_LIFETIME: Duration = Duration::from_secs(10);
/// A pop-out's first size, and the smallest it may be.
const FIRST_SIZE: (f64, f64) = (720.0, 420.0);
const SMALLEST: (f64, f64) = (320.0, 200.0);

/// An organization's window: `main`, or `org-…`.
pub fn is_org_window(label: &str) -> bool {
    label == MAIN || label.starts_with(ORG_PREFIX)
}

/// The label of the window a panel pops out into.
pub fn popout_label(parent: &str, panel: PanelId) -> String {
    format!("{POPOUT_PREFIX}{}--{parent}", panel.key())
}

/// A pop-out's panel and the organization's window that owns it.
pub fn parse_popout(label: &str) -> Option<(PanelId, &str)> {
    let rest = label.strip_prefix(POPOUT_PREFIX)?;
    let (key, parent) = rest.split_once("--")?;
    let panel = PanelId::from_key(key)?;
    is_org_window(parent).then_some((panel, parent))
}

struct Request {
    panel: PanelId,
    place: Option<WindowPlace>,
    at: Instant,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct PlacesFile {
    #[serde(default)]
    places: BTreeMap<String, WindowPlace>,
}

/// The pop-outs Plenipo was asked for, and where each pop-out was last.
pub struct PopOuts {
    requests: Mutex<HashMap<String, Request>>,
    places: Mutex<PlacesFile>,
    file: Option<PathBuf>,
}

impl PopOuts {
    /// Pop-out places kept in `file` (`None`: kept only while Plenipo runs, as in tests).
    pub fn new(file: Option<PathBuf>) -> Self {
        let places = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice::<PlacesFile>(&bytes).ok())
            .map(|mut p| {
                p.places.retain(|label, place| {
                    parse_popout(label).is_some() && place.is_sane()
                });
                p
            })
            .unwrap_or_default();
        Self {
            requests: Mutex::new(HashMap::new()),
            places: Mutex::new(places),
            file,
        }
    }

    /// The page of `parent` is about to open `panel` in its own window (at `place`, where the
    /// panel was dropped, or where it was last).
    pub fn request(&self, parent: &str, panel: PanelId, place: Option<WindowPlace>) {
        let place = place.filter(WindowPlace::is_sane);
        lock(&self.requests).insert(
            parent.to_owned(),
            Request {
                panel,
                place,
                at: Instant::now(),
            },
        );
    }

    /// The window `parent`'s page opens now: the panel it asked for, if it asked lately. A
    /// request is used once.
    fn take(&self, parent: &str) -> Option<(PanelId, Option<WindowPlace>)> {
        let request = lock(&self.requests).remove(parent)?;
        (request.at.elapsed() <= REQUEST_LIFETIME).then_some((request.panel, request.place))
    }

    /// Where the pop-out was last.
    pub fn place(&self, label: &str) -> Option<WindowPlace> {
        lock(&self.places).places.get(label).copied()
    }

    /// Keep where a pop-out is now.
    pub fn remember(&self, label: &str, place: WindowPlace) {
        if parse_popout(label).is_none() || !place.is_sane() {
            return;
        }
        let mut places = lock(&self.places);
        if places.places.get(label) == Some(&place) {
            return;
        }
        places.places.insert(label.to_owned(), place);
        self.save(&places);
    }

    /// Reset layout: forget every pop-out place of this organization's window.
    pub fn forget(&self, parent: &str) {
        let mut places = lock(&self.places);
        let before = places.places.len();
        places
            .places
            .retain(|label, _| parse_popout(label).is_none_or(|(_, p)| p != parent));
        if places.places.len() != before {
            self.save(&places);
        }
    }

    fn save(&self, places: &PlacesFile) {
        let Some(file) = &self.file else { return };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let temp = file.with_extension("json.tmp");
            std::fs::write(&temp, serde_json::to_vec_pretty(places)?)?;
            std::fs::rename(&temp, file)
        };
        if let Err(e) = write() {
            log::warn!("could not keep where the pop-out windows are: {e}");
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What an organization's window does when its page asks for a new window: open the pop-out it
/// asked for, or refuse.
pub fn on_new_window<R: Runtime>(
    app: &AppHandle<R>,
    parent: &str,
    url: &tauri::Url,
    features: NewWindowFeatures,
) -> NewWindowResponse<R> {
    // A pop-out starts empty: the page draws into it.
    if url.as_str() != "about:blank" {
        log::info!("refused a new window from {parent}'s page");
        return NewWindowResponse::Deny;
    }
    let Some(popouts) = app.try_state::<PopOuts>() else {
        return NewWindowResponse::Deny;
    };
    let Some((panel, dropped)) = popouts.take(parent) else {
        log::info!("refused a new window {parent}'s page did not ask Plenipo for");
        return NewWindowResponse::Deny;
    };
    let label = popout_label(parent, panel);
    // One window per panel: an old one (its page reloaded) goes first.
    if let Some(old) = app.get_webview_window(&label) {
        let _ = old.destroy();
    }
    let place = dropped
        .or_else(|| popouts.place(&label))
        .or_else(|| beside(app, parent));
    let mut builder = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::External("about:blank".parse().expect("a fixed address")),
    )
    .window_features(features)
    .title(format!("Plenipo · {}", panel.title()))
    .min_inner_size(SMALLEST.0, SMALLEST.1)
    .inner_size(FIRST_SIZE.0, FIRST_SIZE.1);
    if let Some(place) = place {
        builder = builder
            .inner_size(place.width.max(SMALLEST.0), place.height.max(SMALLEST.1))
            .position(place.x, place.y);
    }
    match builder.build() {
        Ok(window) => {
            if let Some(place) = place.filter(|p| !on_a_screen(&window, p)) {
                // Its screen is gone: next to the organization's window instead.
                log::info!("a pop-out's screen is not connected; it opens beside Plenipo");
                if let Some(near) = beside(app, parent) {
                    let _ = window.set_position(LogicalPosition::new(near.x, near.y));
                    let _ = window.set_size(LogicalSize::new(place.width, place.height));
                }
            }
            NewWindowResponse::Create { window }
        }
        Err(e) => {
            log::warn!("the {} window could not open: {e}", panel.title());
            NewWindowResponse::Deny
        }
    }
}

/// A place beside the organization's window, a little down and to the right.
fn beside<R: Runtime>(app: &AppHandle<R>, parent: &str) -> Option<WindowPlace> {
    let window = app.get_webview_window(parent)?;
    let scale = window.scale_factor().ok()?;
    let at = window.outer_position().ok()?.to_logical::<f64>(scale);
    Some(WindowPlace {
        x: at.x + 64.0,
        y: at.y + 64.0,
        width: FIRST_SIZE.0,
        height: FIRST_SIZE.1,
    })
}

/// Some part of `place`'s top edge is on a connected screen.
fn on_a_screen<R: Runtime>(window: &WebviewWindow<R>, place: &WindowPlace) -> bool {
    let Ok(monitors) = window.available_monitors() else {
        return true;
    };
    if monitors.is_empty() {
        return true;
    }
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let at = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        let (left, right) = (place.x, place.x + place.width);
        place.y >= at.y - 8.0
            && place.y < at.y + size.height - 32.0
            && right > at.x + 32.0
            && left < at.x + size.width - 32.0
    })
}

/// A pop-out moved or changed size: keep where it is now.
pub fn remember_place<R: Runtime>(window: &tauri::Window<R>) {
    let label = window.label();
    if parse_popout(label).is_none() {
        return;
    }
    let (Ok(scale), Ok(at), Ok(size)) = (
        window.scale_factor(),
        window.outer_position(),
        window.inner_size(),
    ) else {
        return;
    };
    // A minimized window reports a place far off screen: not kept.
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let at = at.to_logical::<f64>(scale);
    let size = size.to_logical::<f64>(scale);
    if let Some(popouts) = window.try_state::<PopOuts>() {
        popouts.remember(
            label,
            WindowPlace {
                x: at.x,
                y: at.y,
                width: size.width,
                height: size.height,
            },
        );
    }
}

/// A pop-out's window closed: its organization's window puts the panel back in a dock.
pub fn closed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some((panel, parent)) = parse_popout(label) else {
        return;
    };
    if let Err(e) = app.emit_to(parent, WINDOWS_EVENT, &PopOutNotice::Closed { panel }) {
        log::warn!("could not tell the window its {} panel came back: {e}", panel.key());
    }
}

/// The pop-out windows an organization's window owns.
pub fn popouts_of<R: Runtime>(app: &AppHandle<R>, parent: &str) -> Vec<WebviewWindow<R>> {
    app.webview_windows()
        .into_iter()
        .filter(|(label, _)| parse_popout(label).is_some_and(|(_, p)| p == parent))
        .map(|(_, w)| w)
        .collect()
}

/// The organization's window hid (to the tray): its pop-outs hide with it.
pub fn hide_popouts<R: Runtime>(app: &AppHandle<R>, parent: &str) {
    for w in popouts_of(app, parent) {
        let _ = w.hide();
    }
}

/// The organization's window is shown again: so are its pop-outs.
pub fn show_popouts<R: Runtime>(app: &AppHandle<R>, parent: &str) {
    for w in popouts_of(app, parent) {
        let _ = w.show();
    }
}

/// The organization's page is loading again: its pop-outs close (the page opens them again).
pub fn close_popouts<R: Runtime>(app: &AppHandle<R>, parent: &str) {
    for w in popouts_of(app, parent) {
        let _ = w.destroy();
    }
}

/// Build an organization's window from the app's configuration of `main`, under `label`.
pub fn build_org_window<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    visible: bool,
) -> tauri::Result<WebviewWindow<R>> {
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .ok_or_else(|| tauri::Error::WindowNotFound)?;
    config.label = label.to_owned();
    config.visible = visible;
    config.create = true;
    let handle = app.clone();
    let parent = label.to_owned();
    let window = WebviewWindowBuilder::from_config(app, &config)?
        .on_new_window(move |url, features| on_new_window(&handle, &parent, &url, features))
        .build()?;
    allow_pop_outs(&window);
    Ok(window)
}

/// Let the page open the windows Plenipo allows (WebKitGTK asks for this; WebView2 does not).
#[cfg(target_os = "linux")]
fn allow_pop_outs<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.with_webview(|webview| {
        use webkit2gtk::{SettingsExt as _, WebViewExt as _};
        if let Some(settings) = webview.inner().settings() {
            settings.set_javascript_can_open_windows_automatically(true);
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn allow_pop_outs<R: Runtime>(_window: &WebviewWindow<R>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_name_the_panel_and_its_window() {
        assert_eq!(popout_label(MAIN, PanelId::Terminal), "popout-terminal--main");
        assert_eq!(
            parse_popout("popout-files--org-ab12"),
            Some((PanelId::Files, "org-ab12"))
        );
        assert_eq!(parse_popout("popout-details--main"), None);
        assert_eq!(parse_popout("popout-files--control-indicator"), None);
        assert_eq!(parse_popout("main"), None);
        assert!(is_org_window("main") && is_org_window("org-ab12"));
        assert!(!is_org_window("popout-files--main") && !is_org_window("control-indicator"));
        // A pop-out's label can never pass for an organization's window, whose permission file
        // matches `main` and `org-*`.
        assert!(!popout_label("org-ab12", PanelId::Files).starts_with(ORG_PREFIX));
    }

    #[test]
    fn a_window_opens_only_right_after_its_page_asked_and_once() {
        let popouts = PopOuts::new(None);
        assert!(popouts.take(MAIN).is_none(), "nothing was asked");
        popouts.request(MAIN, PanelId::Terminal, None);
        assert!(popouts.take("org-ab12").is_none(), "another window's page did not ask");
        assert_eq!(popouts.take(MAIN), Some((PanelId::Terminal, None)));
        assert!(popouts.take(MAIN).is_none(), "a request is used once");
        // A stale request is refused.
        popouts.request(MAIN, PanelId::Files, None);
        lock(&popouts.requests).get_mut(MAIN).expect("asked").at -= REQUEST_LIFETIME * 2;
        assert!(popouts.take(MAIN).is_none());
    }

    #[test]
    fn places_are_kept_forgotten_and_checked() {
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join(PLACES_FILE);
        let place = WindowPlace {
            x: 1930.0,
            y: 20.0,
            width: 800.0,
            height: 500.0,
        };
        let popouts = PopOuts::new(Some(file.clone()));
        popouts.remember("popout-terminal--main", place);
        popouts.remember("popout-files--org-ab12", place);
        popouts.remember("not-a-popout", place);
        popouts.remember(
            "popout-files--main",
            WindowPlace {
                width: f64::NAN,
                ..place
            },
        );
        // Kept across a restart.
        let again = PopOuts::new(Some(file.clone()));
        assert_eq!(again.place("popout-terminal--main"), Some(place));
        assert_eq!(again.place("popout-files--org-ab12"), Some(place));
        assert_eq!(again.place("not-a-popout"), None);
        assert_eq!(again.place("popout-files--main"), None);
        // Reset layout forgets one window's places only.
        again.forget(MAIN);
        let after = PopOuts::new(Some(file.clone()));
        assert_eq!(after.place("popout-terminal--main"), None);
        assert_eq!(after.place("popout-files--org-ab12"), Some(place));
        // A damaged file is ignored.
        std::fs::write(&file, b"{ not json").expect("written");
        assert_eq!(PopOuts::new(Some(file)).place("popout-files--org-ab12"), None);
    }

    #[test]
    fn a_drop_place_that_is_not_sane_is_not_used() {
        let popouts = PopOuts::new(None);
        popouts.request(
            MAIN,
            PanelId::Files,
            Some(WindowPlace {
                x: f64::INFINITY,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
        );
        assert_eq!(popouts.take(MAIN), Some((PanelId::Files, None)));
    }
}
