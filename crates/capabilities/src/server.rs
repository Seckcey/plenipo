//! The tool server: listens on the loopback address for relays, admits a connection only with
//! the ticket of an open grant and only from the process tree of that grant's AI tool
//! (ADR-034), then speaks MCP (JSON-RPC, one message per line) for it. Every request is
//! handled on its own task, so a call waiting for the owner's approval never holds up the
//! others; at most [`MAX_CALLS_AT_ONCE`] of a connection's requests are worked on at a time,
//! and the rest wait their turn in the order they arrived (B6).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Semaphore};

use crate::broker::Broker;
use crate::mcp;
use crate::relay::Hello;

/// Longest message accepted from a relay (a file written in one call is at most 1 MiB).
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
/// How long a new connection has to present its ticket.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// How many of one connection's requests are worked on at once (B6). The rest wait their turn
/// in the order they arrived, so a flood of calls cannot swamp Plenipo, the Ledger, or the
/// owner's Approvals page; none is refused for waiting.
pub const MAX_CALLS_AT_ONCE: usize = 4;

/// Bind to a free port on the loopback address and serve `broker`'s grants. Returns the port.
pub async fn start(broker: Broker) -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let local = listener.local_addr()?;
    let port = local.port();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    // Loopback only (the listener is bound to it; checked again for clarity).
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let broker = broker.clone();
                    tokio::spawn(connection(broker, stream, peer, local));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    });
    Ok(port)
}

/// Read one line of at most `max` bytes (`None` at the end of the stream). A longer line is
/// an error.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok((!line.is_empty()).then_some(line));
        }
        let (chunk, done) = match available.iter().position(|b| *b == b'\n') {
            Some(i) => (&available[..i], Some(i + 1)),
            None => (available, None),
        };
        if line.len() + chunk.len() > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "message too long",
            ));
        }
        line.extend_from_slice(chunk);
        let used = done.unwrap_or(available.len());
        reader.consume(used);
        if done.is_some() {
            return Ok(Some(line));
        }
    }
}

async fn connection(broker: Broker, stream: TcpStream, peer: SocketAddr, local: SocketAddr) {
    let _ = stream.set_nodelay(true);
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, read_line(&mut reader, 4096)).await {
        Ok(Ok(Some(line))) => serde_json::from_slice::<Hello>(&line).ok(),
        _ => None,
    };
    let Some(grant_id) = hello.and_then(|h| broker.grant_for_ticket(&h.ticket)) else {
        return; // Unknown or ended ticket: close without a word.
    };
    // The ticket is honored only from the AI tool's own process tree (ADR-034): any other
    // program that read it is closed the same way, and the refusal is recorded.
    if !broker.admit(&grant_id, peer, local).await {
        return;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if write.write_all(line.as_bytes()).await.is_err()
                || write.write_all(b"\n").await.is_err()
                || write.flush().await.is_err()
            {
                break;
            }
        }
    });
    serve(&mut reader, &tx, move |message| {
        let (broker, grant) = (broker.clone(), grant_id.clone());
        async move { mcp::handle(&broker, &grant, message).await }
    })
    .await;
    drop(tx);
    let _ = writer.await;
}

/// Answer an admitted connection's messages until its stream ends or a line is unreadable:
/// each on its own task, at most [`MAX_CALLS_AT_ONCE`] at a time and in the order they arrived.
/// `answer` answers one message (`None` for a notification); answers go to `tx` as they are
/// ready, so a call waiting for the owner never holds up the others.
async fn serve<R, A, F>(reader: &mut R, tx: &mpsc::UnboundedSender<String>, answer: A)
where
    R: tokio::io::AsyncBufRead + Unpin,
    A: Fn(Value) -> F + Clone + Send + 'static,
    F: std::future::Future<Output = Option<Value>> + Send + 'static,
{
    let places = Arc::new(Semaphore::new(MAX_CALLS_AT_ONCE));
    loop {
        let line = match read_line(reader, MAX_MESSAGE_BYTES).await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(_) => {
                let _ = tx.send(
                    mcp::error(Value::Null, -32600, "Message too long or unreadable").to_string(),
                );
                break;
            }
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let message: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                let _ = tx.send(mcp::error(Value::Null, -32700, "Parse error").to_string());
                continue;
            }
        };
        // A tool call takes a place before its task starts, so calls are worked on in the
        // order they arrived: the next one waits here until one of those running is done. The
        // rest (a ping, the tools list, a notification) takes no place and is answered at once,
        // so a call waiting for the owner never makes the connection look hung.
        let place = if is_call(&message) {
            match Arc::clone(&places).acquire_owned().await {
                Ok(place) => Some(place),
                Err(_) => break,
            }
        } else {
            None
        };
        let (answer, tx) = (answer.clone(), tx.clone());
        tokio::spawn(async move {
            let _place = place;
            let response = match message {
                Value::Array(batch) => {
                    let mut out = Vec::new();
                    for m in batch {
                        if let Some(r) = answer(m).await {
                            out.push(r);
                        }
                    }
                    (!out.is_empty()).then(|| json!(out))
                }
                m => answer(m).await,
            };
            if let Some(r) = response {
                let _ = tx.send(r.to_string());
            }
        });
    }
}

/// Whether a message is a tool call (or a batch with one in it): what takes a place.
fn is_call(message: &Value) -> bool {
    match message {
        Value::Array(batch) => batch.iter().any(is_call),
        m => m["method"] == "tools/call",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use super::*;

    /// Give the other tasks their turns until `holds` does (they share this thread).
    async fn until(what: &str, holds: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !holds() {
            assert!(std::time::Instant::now() < deadline, "never: {what}");
            tokio::task::yield_now().await;
        }
    }

    /// B6: eight tool calls sent at once are all answered, at most four are worked on at a
    /// time, and the fifth starts only when one of the first four is done, in the order they
    /// arrived. A ping among them takes no place: it is answered while the four run.
    #[tokio::test]
    async fn at_most_four_requests_are_worked_on_at_once_and_all_are_answered() {
        let started = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let order = Arc::new(Mutex::new(Vec::new()));
        // Every request waits at this gate until the test lets one through, like a call
        // waiting for the owner's answer.
        let gate = Arc::new(Semaphore::new(0));
        let answer = {
            let (started, running, most, order, gate) = (
                Arc::clone(&started),
                Arc::clone(&running),
                Arc::clone(&most),
                Arc::clone(&order),
                Arc::clone(&gate),
            );
            move |m: Value| {
                let (started, running, most, order, gate) = (
                    Arc::clone(&started),
                    Arc::clone(&running),
                    Arc::clone(&most),
                    Arc::clone(&order),
                    Arc::clone(&gate),
                );
                async move {
                    if m["method"] == "ping" {
                        return Some(json!({ "id": m["id"] }));
                    }
                    started.fetch_add(1, Ordering::SeqCst);
                    order.lock().unwrap().push(m["id"].as_u64().unwrap());
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    gate.acquire().await.unwrap().forget();
                    running.fetch_sub(1, Ordering::SeqCst);
                    Some(json!({ "id": m["id"] }))
                }
            }
        };
        let call = |i: u64| format!("{{\"id\":{i},\"method\":\"tools/call\"}}\n");
        let input: String = (1..=4)
            .map(call)
            .chain(["{\"id\":9,\"method\":\"ping\"}\n".to_owned()])
            .chain((5..=8).map(call))
            .collect();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(input.as_bytes());
            serve(&mut reader, &tx, answer).await;
        });
        // Four are worked on; the ping is answered meanwhile; the fifth waits for a place, so
        // nothing more starts and no call is answered yet.
        until("four started", || started.load(Ordering::SeqCst) == 4).await;
        let ping = rx.recv().await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&ping).unwrap()["id"],
            9,
            "{ping}"
        );
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        assert_eq!(started.load(Ordering::SeqCst), 4);
        assert!(rx.try_recv().is_err());
        // One is done: its answer arrives, and only then does the fifth start.
        gate.add_permits(1);
        let first = rx.recv().await.unwrap();
        until("the fifth started", || started.load(Ordering::SeqCst) == 5).await;
        assert_eq!(order.lock().unwrap()[4], 5);
        assert!(rx.try_recv().is_err());
        // The rest are let through: every request is answered, and the stream ends.
        gate.add_permits(7);
        let mut answers = vec![ping, first];
        for _ in 0..7 {
            answers.push(rx.recv().await.unwrap());
        }
        server.await.unwrap();
        assert!(rx.recv().await.is_none());
        let mut ids: Vec<u64> = answers
            .iter()
            .map(|a| {
                serde_json::from_str::<Value>(a).unwrap()["id"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        // Only a tool call, or a batch with one in it, takes a place.
        assert!(is_call(&json!({ "id": 1, "method": "tools/call" })));
        assert!(!is_call(&json!({ "id": 2, "method": "tools/list" })));
        assert!(!is_call(&json!({ "method": "notifications/initialized" })));
        assert!(is_call(
            &json!([{ "method": "ping" }, { "method": "tools/call" }])
        ));
        assert!(!is_call(&json!([{ "method": "ping" }])));
        let mut starts = order.lock().unwrap().clone();
        assert_eq!(starts[..4].iter().copied().max(), Some(4), "{starts:?}");
        starts.sort_unstable();
        assert_eq!(starts, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(most.load(Ordering::SeqCst), MAX_CALLS_AT_ONCE);
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn lines_are_limited() {
        let data: &[u8] = b"short\nthis line is far too long\nlast";
        let mut r = BufReader::new(data);
        assert_eq!(read_line(&mut r, 10).await.unwrap().unwrap(), b"short");
        assert!(read_line(&mut r, 10).await.is_err());
        let mut r = BufReader::new(&b"a\nb"[..]);
        assert_eq!(read_line(&mut r, 10).await.unwrap().unwrap(), b"a");
        assert_eq!(read_line(&mut r, 10).await.unwrap().unwrap(), b"b");
        assert_eq!(read_line(&mut r, 10).await.unwrap(), None);
    }
}
