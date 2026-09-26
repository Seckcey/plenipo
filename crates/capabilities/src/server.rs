//! The tool server: listens on the loopback address for relays, admits a connection only with
//! the ticket of an open grant, then speaks MCP (JSON-RPC, one message per line) for it. Every
//! request is handled on its own task, so a call waiting for the owner's approval never holds
//! up the others.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::broker::Broker;
use crate::mcp;
use crate::relay::Hello;

/// Longest message accepted from a relay (a file written in one call is at most 1 MiB).
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
/// How long a new connection has to present its ticket.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// Bind to a free port on the loopback address and serve `broker`'s grants. Returns the port.
pub async fn start(broker: Broker) -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    // Loopback only (the listener is bound to it; checked again for clarity).
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let broker = broker.clone();
                    tokio::spawn(connection(broker, stream));
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

async fn connection(broker: Broker, stream: TcpStream) {
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
    loop {
        let line = match read_line(&mut reader, MAX_MESSAGE_BYTES).await {
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
        let (broker, grant, tx) = (broker.clone(), grant_id.clone(), tx.clone());
        tokio::spawn(async move {
            let response = match message {
                Value::Array(batch) => {
                    let mut out = Vec::new();
                    for m in batch {
                        if let Some(r) = mcp::handle(&broker, &grant, m).await {
                            out.push(r);
                        }
                    }
                    (!out.is_empty()).then(|| json!(out))
                }
                m => mcp::handle(&broker, &grant, m).await,
            };
            if let Some(r) = response {
                let _ = tx.send(r.to_string());
            }
        });
    }
    drop(tx);
    let _ = writer.await;
}

#[cfg(test)]
mod tests {
    use super::*;

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
