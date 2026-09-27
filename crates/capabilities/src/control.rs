//! Who is using Plenipo's browser, this computer's mouse and keyboard, or one of the owner's
//! servers right now, and the owner's controls over them (Phase 10, ADR-020; servers: Phase 11,
//! ADR-025): **Take over** (the owner takes control and that worker stops; for a server,
//! **Disconnect**) and the emergency **Stop** (all control halts at once, and stays stopped until
//! the owner allows it again). The desktop app shows this state on every page, in the
//! system tray, and — while a worker uses the mouse and keyboard — in a window above all others.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a worker is using.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ControlKind {
    /// Plenipo's browser.
    Browser,
    /// This computer's mouse and keyboard (and screen).
    Desktop,
    /// One or more of the owner's servers, over SSH (Phase 11).
    Server,
}

/// Where a use of the browser or the desktop stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ControlState {
    /// The worker is using it.
    Active,
    /// The owner took control; the worker stopped.
    TakenOver,
    /// The owner's emergency stop.
    Stopped,
}

/// One worker's use of the browser or the desktop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ControlSession {
    /// `<kind>:<grant ID>`.
    pub id: String,
    pub grant_id: String,
    pub task_id: String,
    pub worker: String,
    pub kind: ControlKind,
    pub state: ControlState,
    /// The browser tab's page, or the reason the worker gave for using the desktop.
    #[ts(optional)]
    pub detail: Option<String>,
    /// What it did last ("clicked \"Send message\"").
    #[ts(optional)]
    pub last_action: Option<String>,
    #[ts(type = "number")]
    pub since: u64,
    /// A server session includes a production server (Phase 11): shown in red.
    #[serde(default)]
    pub production: bool,
}

/// Everything the owner sees about control, on every page and in the tray.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ControlStatus {
    /// The owner pressed Stop: no worker may use the browser or the desktop until the owner
    /// allows it again.
    pub stopped: bool,
    pub sessions: Vec<ControlSession>,
    /// Counts every change, so a screen shows the newest status even when two updates arrive
    /// out of order.
    #[ts(type = "number")]
    pub revision: u64,
}

impl ControlStatus {
    /// A worker is using the browser or the desktop now.
    pub fn active(&self) -> bool {
        self.sessions
            .iter()
            .any(|s| s.state == ControlState::Active)
    }

    /// A worker is using the mouse and keyboard now.
    pub fn desktop_active(&self) -> bool {
        self.sessions
            .iter()
            .any(|s| s.kind == ControlKind::Desktop && s.state == ControlState::Active)
    }
}

/// Told about every change (the desktop app: its banner, tray, and indicator window).
pub type ControlListener = Arc<dyn Fn(&ControlStatus) + Send + Sync>;

#[derive(Default)]
struct Inner {
    stopped: bool,
    sessions: BTreeMap<String, ControlSession>,
    revision: u64,
}

/// The state of control, shared by the broker and the app. Cheap to clone.
#[derive(Clone, Default)]
pub struct ControlCenter {
    inner: Arc<Mutex<Inner>>,
    listener: Arc<Mutex<Option<ControlListener>>>,
}

pub fn session_id(kind: ControlKind, grant_id: &str) -> String {
    match kind {
        ControlKind::Browser => format!("browser:{grant_id}"),
        ControlKind::Desktop => format!("desktop:{grant_id}"),
        ControlKind::Server => format!("server:{grant_id}"),
    }
}

impl ControlCenter {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn set_listener(&self, listener: ControlListener) {
        *self.listener.lock().unwrap_or_else(|p| p.into_inner()) = Some(listener);
        self.changed();
    }

    /// Tell the listener. The status is read and told under the listener's lock, so the listener
    /// hears changes in the order they happened (a Stop is never followed by an older status).
    fn changed(&self) {
        let listener = self.listener.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(l) = listener.as_ref() {
            l(&self.status());
        }
    }

    pub fn status(&self) -> ControlStatus {
        let i = self.inner();
        ControlStatus {
            stopped: i.stopped,
            sessions: i.sessions.values().cloned().collect(),
            revision: i.revision,
        }
    }

    pub fn stopped(&self) -> bool {
        self.inner().stopped
    }

    pub fn session(&self, id: &str) -> Option<ControlSession> {
        self.inner().sessions.get(id).cloned()
    }

    /// A worker starts using the browser or the desktop.
    pub fn begin(
        &self,
        kind: ControlKind,
        grant_id: &str,
        task_id: &str,
        worker: &str,
        detail: Option<String>,
    ) -> ControlSession {
        let id = session_id(kind, grant_id);
        let session = ControlSession {
            id: id.clone(),
            grant_id: grant_id.to_owned(),
            task_id: task_id.to_owned(),
            worker: worker.to_owned(),
            kind,
            state: ControlState::Active,
            detail,
            last_action: None,
            since: plenipo_ledger::now_ms(),
            production: false,
        };
        {
            let mut i = self.inner();
            i.sessions.insert(id, session.clone());
            i.revision += 1;
        }
        self.changed();
        session
    }

    /// What a worker did last, and where.
    pub fn note(&self, id: &str, detail: Option<String>, last_action: Option<String>) {
        {
            let mut i = self.inner();
            let Some(s) = i.sessions.get_mut(id) else {
                return;
            };
            if detail.is_some() {
                s.detail = detail;
            }
            if last_action.is_some() {
                s.last_action = last_action;
            }
            i.revision += 1;
        }
        self.changed();
    }

    /// Where a server session is connected now, and whether that includes a production server.
    pub fn servers(&self, id: &str, detail: String, production: bool) {
        {
            let mut i = self.inner();
            let Some(s) = i.sessions.get_mut(id) else {
                return;
            };
            s.detail = Some(detail);
            s.production = production;
            i.revision += 1;
        }
        self.changed();
    }

    /// The owner took control of a session. `false` when it was not active.
    pub fn take_over(&self, id: &str) -> Option<ControlSession> {
        let taken = {
            let mut i = self.inner();
            let s = i.sessions.get_mut(id)?;
            if s.state != ControlState::Active {
                return None;
            }
            s.state = ControlState::TakenOver;
            let taken = s.clone();
            i.revision += 1;
            taken
        };
        self.changed();
        Some(taken)
    }

    /// Stop one session (its worker's permissions were revoked).
    pub fn stop(&self, id: &str) -> Option<ControlSession> {
        let stopped = {
            let mut i = self.inner();
            let s = i.sessions.get_mut(id)?;
            if s.state != ControlState::Active {
                return None;
            }
            s.state = ControlState::Stopped;
            let stopped = s.clone();
            i.revision += 1;
            stopped
        };
        self.changed();
        Some(stopped)
    }

    /// The emergency stop: every active session stops, and no new one may start until
    /// [`ControlCenter::allow`]. Returns the sessions it stopped.
    pub fn stop_all(&self) -> Vec<ControlSession> {
        let stopped: Vec<ControlSession> = {
            let mut i = self.inner();
            i.stopped = true;
            i.revision += 1;
            i.sessions
                .values_mut()
                .filter(|s| s.state == ControlState::Active)
                .map(|s| {
                    s.state = ControlState::Stopped;
                    s.clone()
                })
                .collect()
        };
        self.changed();
        stopped
    }

    /// Stop every active session of one kind (the owner switched that feature off, ADR-023).
    /// Unlike [`ControlCenter::stop_all`], it sets no emergency stop. Returns the sessions stopped.
    pub fn stop_kind(&self, kind: ControlKind) -> Vec<ControlSession> {
        let stopped: Vec<ControlSession> = {
            let mut i = self.inner();
            let stopped: Vec<ControlSession> = i
                .sessions
                .values_mut()
                .filter(|s| s.kind == kind && s.state == ControlState::Active)
                .map(|s| {
                    s.state = ControlState::Stopped;
                    s.clone()
                })
                .collect();
            if !stopped.is_empty() {
                i.revision += 1;
            }
            stopped
        };
        if !stopped.is_empty() {
            self.changed();
        }
        stopped
    }

    /// The owner allows control again after a stop.
    pub fn allow(&self) {
        {
            let mut i = self.inner();
            i.stopped = false;
            i.revision += 1;
        }
        self.changed();
    }

    /// A session ends with its worker's step.
    pub fn end(&self, id: &str) -> Option<ControlSession> {
        let ended = {
            let mut i = self.inner();
            let ended = i.sessions.remove(id);
            if ended.is_some() {
                i.revision += 1;
            }
            ended
        };
        if ended.is_some() {
            self.changed();
        }
        ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_over_stop_and_allow() {
        let c = ControlCenter::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        c.set_listener(Arc::new(move |s: &ControlStatus| {
            log.lock()
                .unwrap()
                .push((s.stopped, s.sessions.len(), s.active()));
        }));
        let b = c.begin(ControlKind::Browser, "g1", "t1", "Web Assistant", None);
        let d = c.begin(
            ControlKind::Desktop,
            "g2",
            "t2",
            "Operator",
            Some("no API".into()),
        );
        assert!(c.status().desktop_active());
        c.note(
            &b.id,
            Some("https://shop.test/".into()),
            Some("opened it".into()),
        );
        assert_eq!(
            c.session(&b.id).unwrap().last_action.as_deref(),
            Some("opened it")
        );
        assert_eq!(c.take_over(&b.id).unwrap().state, ControlState::TakenOver);
        assert!(c.take_over(&b.id).is_none(), "only once");
        let stopped = c.stop_all();
        assert_eq!(stopped.len(), 1, "only the active one");
        assert_eq!(stopped[0].id, d.id);
        assert!(c.stopped() && !c.status().active());
        c.allow();
        assert!(!c.stopped());
        assert!(c.end(&b.id).is_some() && c.end(&b.id).is_none());
        let seen = seen.lock().unwrap();
        assert_eq!(seen.first(), Some(&(false, 0, false)));
        assert_eq!(seen.last(), Some(&(false, 1, false)));
    }

    #[test]
    fn switching_a_feature_off_stops_only_its_sessions() {
        let c = ControlCenter::default();
        let b = c.begin(ControlKind::Browser, "g1", "t1", "Web Assistant", None);
        let d = c.begin(ControlKind::Desktop, "g2", "t2", "Operator", None);
        let stopped = c.stop_kind(ControlKind::Browser);
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].id, b.id);
        assert_eq!(c.session(&d.id).unwrap().state, ControlState::Active);
        assert!(!c.stopped(), "no emergency stop");
        assert!(c.stop_kind(ControlKind::Browser).is_empty());
    }

    /// Changes from many threads reach the listener in the order they happened: no status it
    /// hears is older than the one before (two changes may both report the newest), and the last
    /// one is the current state.
    #[test]
    fn the_listener_hears_changes_in_order() {
        let c = ControlCenter::default();
        let heard = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&heard);
        c.set_listener(Arc::new(move |s: &ControlStatus| {
            log.lock().unwrap().push(s.revision);
        }));
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let c = c.clone();
                std::thread::spawn(move || {
                    let s = c.begin(ControlKind::Browser, &format!("g{n}"), "t", "W", None);
                    for k in 0..50 {
                        c.note(&s.id, None, Some(format!("step {k}")));
                    }
                    if n == 3 {
                        c.stop_all();
                    }
                    c.end(&s.id);
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let heard = heard.lock().unwrap();
        assert!(
            heard.windows(2).all(|w| w[0] <= w[1]),
            "never an older status"
        );
        assert_eq!(*heard.last().unwrap(), c.status().revision);
        assert!(c.status().stopped && c.status().sessions.is_empty());
    }
}
