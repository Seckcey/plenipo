//! A small client for the Chrome DevTools Protocol, which Chrome and Microsoft Edge speak
//! (Phase 10, ADR-020): commands with an ID and their answers, and events, each routed to the
//! tab (session) it belongs to. Only what Plenipo's browser tools use.
//!
//! The app talks over the two pipes the browser inherits from Plenipo ([`Cdp::over_pipe`]):
//! JSON texts, each ended by a NUL byte, on a connection no other program can find or join,
//! because there is no network port. The loopback WebSocket ([`Cdp::connect`]) is for the
//! tests, which stand in for the owner's own hand on the browser; the app never opens one.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use plenipo_runtime::PipeEnds;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// Largest message accepted from the browser (a screenshot is the largest). A larger one ends
/// the connection, over the pipe as over a WebSocket.
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
    /// A connection with nothing behind it yet: the client, the queue of texts to send, and
    /// the channel for the browser's own events.
    fn parts() -> (
        Self,
        mpsc::UnboundedReceiver<String>,
        mpsc::UnboundedReceiver<Event>,
    ) {
        let shared = Arc::new(Shared::default());
        let (browser_tx, browser_rx) = mpsc::unbounded_channel();
        *lock(&shared.browser) = Some(browser_tx);
        let (out, out_rx) = mpsc::unbounded_channel::<String>();
        (
            Self {
                out,
                shared,
                next: Arc::new(AtomicU64::new(1)),
            },
            out_rx,
            browser_rx,
        )
    }

    /// Talk to the browser over the pipes it inherited from Plenipo (`--remote-debugging-pipe`:
    /// it reads commands on its descriptor 3 and writes answers and events on 4, each a JSON
    /// text ended by a NUL byte). The browser's own events arrive on the returned channel. The
    /// connection ends when the browser closes its end; the browser closes when Plenipo does.
    pub fn over_pipe(ends: PipeEnds) -> Result<(Self, mpsc::UnboundedReceiver<Event>), String> {
        let (cdp, mut out_rx, browser_rx) = Self::parts();
        let PipeEnds { mut writer, reader } = ends;
        let start = |name: &str, work: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new()
                .name(format!("plenipo-browser-pipe-{name}"))
                .spawn(work)
                .map(drop)
                .map_err(|e| format!("could not start talking to Plenipo's browser: {e}"))
        };
        let writer_shared = Arc::clone(&cdp.shared);
        start(
            "out",
            Box::new(move || {
                while let Some(text) = out_rx.blocking_recv() {
                    if write_message(&mut writer, &text).is_err() {
                        break;
                    }
                }
                // Dropping the writer ends the pipe; the browser closes when it sees that.
                writer_shared.close();
            }),
        )?;
        let reader_shared = Arc::clone(&cdp.shared);
        start(
            "in",
            Box::new(move || {
                read_messages(reader, |text| reader_shared.dispatch(text));
                reader_shared.close();
            }),
        )?;
        Ok((cdp, browser_rx))
    }

    /// Tests only: connect to a browser's DevTools address on this computer
    /// (`ws://127.0.0.1:<port>/devtools/browser/…`), the way the owner's own hand would. The
    /// browser's own events arrive on the returned channel.
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
        let (cdp, mut out_rx, browser_rx) = Self::parts();
        let writer_shared = Arc::clone(&cdp.shared);
        tokio::spawn(async move {
            while let Some(text) = out_rx.recv().await {
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
            writer_shared.close();
        });
        let reader_shared = Arc::clone(&cdp.shared);
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
        Ok((cdp, browser_rx))
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

/// One message on the pipe: the JSON text (serde_json never writes a raw NUL), then a NUL.
fn write_message(writer: &mut impl Write, text: &str) -> io::Result<()> {
    writer.write_all(text.as_bytes())?;
    writer.write_all(&[0])?;
    writer.flush()
}

/// Hand `each` every NUL-ended message from `reader`, until the pipe ends, a read fails, or a
/// message grows past the cap.
fn read_messages(mut reader: impl Read, mut each: impl FnMut(&str)) {
    let mut message: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let mut rest = &chunk[..n];
        while let Some(end) = rest.iter().position(|&b| b == 0) {
            message.extend_from_slice(&rest[..end]);
            rest = &rest[end + 1..];
            if let Ok(text) = std::str::from_utf8(&message) {
                each(text);
            }
            message.clear();
        }
        message.extend_from_slice(rest);
        if message.len() > MAX_MESSAGE_BYTES {
            return;
        }
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

    /// A reader that gives its pieces one read at a time, however the pipe happened to split
    /// the bytes.
    struct Pieces(std::collections::VecDeque<Vec<u8>>);

    impl Read for Pieces {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let Some(mut piece) = self.0.pop_front() else {
                return Ok(0);
            };
            if piece.len() > buf.len() {
                let rest = piece.split_off(buf.len());
                self.0.push_front(rest);
            }
            buf[..piece.len()].copy_from_slice(&piece);
            Ok(piece.len())
        }
    }

    fn pieces(parts: &[&[u8]]) -> Pieces {
        Pieces(parts.iter().map(|p| p.to_vec()).collect())
    }

    #[test]
    fn pipe_messages_end_at_nul_bytes_however_they_arrive() {
        let mut seen = Vec::new();
        read_messages(
            pieces(&[
                b"{\"id\":1}\0{\"me",
                b"thod\":\"A\"}\0{\"id\":2}",
                b"\0\0{\"id\":3}",
            ]),
            |text| seen.push(text.to_owned()),
        );
        // Two in one read, one split across reads, an empty message, and a last one the pipe
        // ended before finishing (never delivered: it is not whole).
        assert_eq!(
            seen,
            [r#"{"id":1}"#, r#"{"method":"A"}"#, r#"{"id":2}"#, ""]
        );
    }

    #[test]
    fn a_message_past_the_cap_ends_the_reading() {
        let huge = io::repeat(b'x').take(MAX_MESSAGE_BYTES as u64 + 1);
        let mut seen = 0;
        read_messages(huge.chain(&b"\0{\"id\":1}\0"[..]), |_| seen += 1);
        assert_eq!(seen, 0, "nothing after an oversized message is read");
    }

    #[test]
    fn a_message_is_written_with_its_nul() {
        let mut out = Vec::new();
        write_message(&mut out, r#"{"id":1}"#).unwrap();
        assert_eq!(out, b"{\"id\":1}\0");
    }

    #[tokio::test]
    async fn over_a_pipe_a_call_is_answered_and_the_end_of_the_pipe_closes_it() {
        // The far end: a stand-in browser on plain pipes, in a thread. It passes on each command
        // it reads, and the test answers only once the command came: an answer sent before its
        // call would find no one waiting for it.
        let (to_browser_read, to_browser_write) = io::pipe().unwrap();
        let (from_browser_read, from_browser_write) = io::pipe().unwrap();
        let ends = PipeEnds {
            writer: to_browser_write,
            reader: from_browser_read,
        };
        let (got, mut commands) = mpsc::unbounded_channel();
        let browser = std::thread::spawn(move || {
            read_messages(to_browser_read, |text| {
                let _ = got.send(text.to_owned());
            });
        });
        let (cdp, _events) = Cdp::over_pipe(ends).unwrap();
        let mut from_browser_write = from_browser_write;
        let answer_once_asked = async {
            let request = commands.recv().await.expect("the browser got the command");
            write_message(
                &mut from_browser_write,
                r#"{"id":1,"result":{"product":"Fake"}}"#,
            )
            .unwrap();
            request
        };
        let (answer, request) = tokio::join!(
            cdp.call(
                None,
                "Browser.getVersion",
                json!({}),
                Duration::from_secs(5),
            ),
            answer_once_asked
        );
        assert_eq!(answer.unwrap()["product"], "Fake");
        assert!(!cdp.is_closed());
        // The browser goes away: its end of the pipe closes, and so does the connection.
        drop(from_browser_write);
        tokio::time::timeout(Duration::from_secs(5), cdp.closed())
            .await
            .expect("closed once the pipe ended");
        assert!(cdp.is_closed());
        // The browser saw the command as one NUL-ended JSON text on its descriptor 3's pipe.
        drop(cdp);
        browser.join().unwrap();
        let request: Value = serde_json::from_str(&request).unwrap();
        assert_eq!(request["method"], "Browser.getVersion");
        assert_eq!(request["id"], 1);
    }
}
