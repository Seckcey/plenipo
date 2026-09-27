//! Who is using Plenipo's browser or this computer's mouse and keyboard right now, and the
//! owner's controls over them (Phase 10, ADR-020): **Take over** (the owner takes control and
//! that worker stops) and the emergency **Stop** (all control halts at once, and stays stopped
//! until the owner allows it again). The desktop app shows this state on every page, in the
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

    fn changed(&self) {
        let status = self.status();
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(l) = listener {
            l(&status);
        }
    }

    pub fn status(&self) -> ControlStatus {
        let i = self.inner();
        ControlStatus {
            stopped: i.stopped,
            sessions: i.sessions.values().cloned().collect(),
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
        };
        self.inner().sessions.insert(id, session.clone());
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
            s.clone()
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
            s.clone()
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

    /// The owner allows control again after a stop.
    pub fn allow(&self) {
        self.inner().stopped = false;
        self.changed();
    }

    /// A session ends with its worker's step.
    pub fn end(&self, id: &str) -> Option<ControlSession> {
        let ended = self.inner().sessions.remove(id);
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
}
