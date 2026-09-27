//! A small client for the Chrome DevTools Protocol, which Chrome and Microsoft Edge speak on a
//! loopback WebSocket (Phase 10, ADR-020): commands with an ID and their answers, and events,
//! each routed to the tab (session) it belongs to. Only what Plenipo's browser tools use.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// Largest message accepted from the browser (a screenshot is the largest).
const MAX_MESSAGE_BYTES: usize = 128 * 1024 * 1024;

/// An event from the browser: its method, its parameters, and the tab session it concerns.
#[derive(Debug, Clone)]
pub struct Event {
    pub session: Option<String>,
    pub method: String,
    pub params: Value,
}

type Answer = oneshot::Sender<Result<Value, String>>;

#[derive(Default)]
struct Shared {
    pending: Mutex<HashMap<u64, Answer>>,
    /// Session ID → where its events go.
    sessions: Mutex<HashMap<String, mpsc::UnboundedSender<Event>>>,
    /// Events that belong to no session (the browser's own).
    browser: Mutex<Option<mpsc::UnboundedSender<Event>>>,
    closed: AtomicBool,
    closed_notify: Notify,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        for (_, tx) in lock(&self.pending).drain() {
            let _ = tx.send(Err("Plenipo's browser closed its connection".into()));
        }
        // Dropping the senders ends every tab's event loop.
        lock(&self.sessions).clear();
        lock(&self.browser).take();
        self.closed_notify.notify_waiters();
    }

    fn dispatch(&self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return;
        };
        if let Some(id) = message.get("id").and_then(Value::as_u64) {
            if let Some(tx) = lock(&self.pending).remove(&id) {
                let answer = match message.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or("error").to_owned()),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = tx.send(answer);
            }
            return;
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        let session = message
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let event = Event {
            session: session.clone(),
            method: method.to_owned(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        };
        match session {
            Some(s) => {
                if let Some(tx) = lock(&self.sessions).get(&s) {
                    let _ = tx.send(event);
                }
            }
            None => {
                if let Some(tx) = lock(&self.browser).as_ref() {
                    let _ = tx.send(event);
                }
            }
        }
    }
}

/// A connection to the browser. Cheap to clone; clones share it.
#[derive(Clone)]
pub struct Cdp {
    out: mpsc::UnboundedSender<String>,
    shared: Arc<Shared>,
    next: Arc<AtomicU64>,
}

impl Cdp {
    /// Connect to the browser's DevTools address (`ws://127.0.0.1:<port>/devtools/browser/…`).
    /// The browser's own events arrive on the returned channel.
    pub async fn connect(address: &str) -> Result<(Self, mpsc::UnboundedReceiver<Event>), String> {
        if !address.starts_with("ws://127.0.0.1:") && !address.starts_with("ws://[::1]:") {
            return Err(format!(
                "the browser's DevTools address {address} is not on this computer"
            ));
        }
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(MAX_MESSAGE_BYTES);
        config.max_frame_size = Some(MAX_MESSAGE_BYTES);
        let (socket, _) =
            tokio_tungstenite::connect_async_with_config(address, Some(config), false)
                .await
                .map_err(|e| format!("could not connect to Plenipo's browser: {e}"))?;
        let (mut sink, mut stream) = socket.split();
        let shared = Arc::new(Shared::default());
        let (browser_tx, browser_rx) = mpsc::unbounded_channel();
        *lock(&shared.browser) = Some(browser_tx);
        let (out, mut out_rx) = mpsc::unbounded_channel::<String>();
        let writer_shared = Arc::clone(&shared);
        tokio::spawn(async move {
            while let Some(text) = out_rx.recv().await {
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
            writer_shared.close();
        });
        let reader_shared = Arc::clone(&shared);
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(text)) => reader_shared.dispatch(text.as_str()),
                    Ok(Message::Binary(bytes)) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            reader_shared.dispatch(text);
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            reader_shared.close();
        });
        Ok((
            Self {
                out,
                shared,
                next: Arc::new(AtomicU64::new(1)),
            },
            browser_rx,
        ))
    }

    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }

    /// Wait until the connection closes.
    pub async fn closed(&self) {
        loop {
            let notified = self.shared.closed_notify.notified();
            if self.is_closed() {
                return;
            }
            notified.await;
        }
    }

    /// Send a command (to a tab's `session`, or to the browser) and wait for its answer.
    pub async fn call(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        if self.is_closed() {
            return Err("Plenipo's browser is not running".into());
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            message["sessionId"] = json!(s);
        }
        let (tx, rx) = oneshot::channel();
        lock(&self.shared.pending).insert(id, tx);
        if self.out.send(message.to_string()).is_err() {
            lock(&self.shared.pending).remove(&id);
            return Err("Plenipo's browser is not running".into());
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(answer)) => answer.map_err(|e| format!("{method}: {e}")),
            Ok(Err(_)) => Err("Plenipo's browser closed its connection".into()),
            Err(_) => {
                lock(&self.shared.pending).remove(&id);
                Err(format!(
                    "the browser did not answer within {} seconds ({method})",
                    timeout.as_secs()
                ))
            }
        }
    }

    /// Events of a tab session from now on.
    pub fn listen(&self, session: &str) -> mpsc::UnboundedReceiver<Event> {
        let (tx, rx) = mpsc::unbounded_channel();
        lock(&self.shared.sessions).insert(session.to_owned(), tx);
        rx
    }

    /// Stop routing a session's events (its tab is gone).
    pub fn forget(&self, session: &str) {
        lock(&self.shared.sessions).remove(session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_and_events_are_routed() {
        let shared = Shared::default();
        let (tx, mut rx) = oneshot::channel();
        lock(&shared.pending).insert(7, tx);
        let (stx, mut srx) = mpsc::unbounded_channel();
        lock(&shared.sessions).insert("S1".into(), stx);
        let (btx, mut brx) = mpsc::unbounded_channel();
        *lock(&shared.browser) = Some(btx);
        shared.dispatch(r#"{"id":7,"result":{"frameId":"F"}}"#);
        assert_eq!(rx.try_recv().unwrap().unwrap()["frameId"], "F");
        shared.dispatch(r#"{"method":"Page.loadEventFired","sessionId":"S1","params":{}}"#);
        assert_eq!(srx.try_recv().unwrap().method, "Page.loadEventFired");
        shared.dispatch(r#"{"method":"Target.targetDestroyed","params":{"targetId":"T"}}"#);
        assert_eq!(brx.try_recv().unwrap().params["targetId"], "T");
        // Unknown sessions, answers nobody waits for, and garbage are ignored.
        shared.dispatch(r#"{"method":"X","sessionId":"nobody","params":{}}"#);
        shared.dispatch(r#"{"id":99,"result":{}}"#);
        shared.dispatch("not json");
        let (tx, mut rx) = oneshot::channel();
        lock(&shared.pending).insert(8, tx);
        shared.dispatch(r#"{"id":8,"error":{"code":-32000,"message":"No node"}}"#);
        assert_eq!(rx.try_recv().unwrap().unwrap_err(), "No node");
        // Closing fails what still waits and ends the session channels.
        let (tx, mut rx) = oneshot::channel();
        lock(&shared.pending).insert(9, tx);
        shared.close();
        assert!(rx.try_recv().unwrap().is_err());
        assert!(srx.try_recv().is_err());
        assert!(shared.closed.load(Ordering::SeqCst));
    }
}
