//! The door: one plain HTTP request head per connection. `GET /healthz` answers in plain text;
//! a WebSocket upgrade on the contract's two paths becomes a PC or a phone connection; everything
//! else gets a short answer and the door closes. The limits are checked here, before a WebSocket
//! exists.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use futures_util::{SinkExt as _, StreamExt as _};
use plenipo_relay_contract::wire::MAX_RELAY_MESSAGE;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Role, WebSocketConfig};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::hub::{Hub, Link, Turned};
use crate::limits::AddressKey;
use crate::line::Line;
use crate::ClientAddress;

/// The most a request head may be.
const MAX_HEAD: usize = 8 * 1024;
/// How long a finished connection waits for the peer's own close (a round trip, even on a
/// slow phone network) before the socket closes anyway.
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
pub(crate) const PC_PATH: &str = "/plenipo/v1/pc";
pub(crate) const PHONE_PATH: &str = "/plenipo/v1/phone";

enum Role_ {
    Pc,
    Phone,
}

pub(crate) async fn serve(hub: Arc<Hub>, mut stream: TcpStream, peer: SocketAddr) {
    let Some(head) = read_head(&mut stream, hub.config.limits.first_message).await else {
        return;
    };
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut request = httparse::Request::new(&mut headers);
    let head_len = match request.parse(&head) {
        Ok(httparse::Status::Complete(n)) => n,
        _ => {
            answer(&mut stream, 400, "Bad Request", "bad request\n").await;
            return;
        }
    };
    let method = request.method.unwrap_or("");
    let path = request.path.unwrap_or("");
    let address = client_address(hub.config.client_address, request.headers, peer.ip());
    let upgrade = is_websocket_upgrade(request.headers);

    if method == "GET" && path == "/healthz" && !upgrade {
        if hub.is_off() {
            answer(&mut stream, 503, "Service Unavailable", "off\n").await;
        } else {
            answer(&mut stream, 200, "OK", "ok\n").await;
        }
        return;
    }
    let role = match (upgrade, method, path) {
        (true, "GET", PC_PATH) => Role_::Pc,
        (true, "GET", PHONE_PATH) => Role_::Phone,
        _ => {
            answer(&mut stream, 404, "Not Found", "not found\n").await;
            return;
        }
    };
    let Some(key) = header(request.headers, "sec-websocket-key") else {
        answer(&mut stream, 400, "Bad Request", "bad request\n").await;
        return;
    };
    let accept = derive_accept_key(key.as_bytes());
    if let Err(turned) = hub.enter(address) {
        log::info!("turned away ({turned:?}, {address})");
        match turned {
            Turned::Off => answer(&mut stream, 503, "Service Unavailable", "off\n").await,
            Turned::Full => answer(&mut stream, 503, "Service Unavailable", "full\n").await,
            Turned::TooMany => {
                answer(&mut stream, 429, "Too Many Requests", "too many; wait\n").await
            }
        }
        return;
    }
    let leftover = head[head_len..].to_vec();
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    if stream.write_all(response.as_bytes()).await.is_err() {
        hub.leave(address);
        return;
    }
    let config = WebSocketConfig::default()
        // Each message goes out at once: no buffer sits around per connection.
        .write_buffer_size(0)
        .max_write_buffer_size(4 * MAX_RELAY_MESSAGE)
        .max_message_size(Some(MAX_RELAY_MESSAGE))
        .max_frame_size(Some(MAX_RELAY_MESSAGE));
    let ws =
        WebSocketStream::from_partially_read(stream, leftover, Role::Server, Some(config)).await;
    let (mut sink, reader) = ws.split();
    let (out, mut outgoing) =
        tokio::sync::mpsc::channel::<Message>(hub.config.limits.outgoing_queue);
    let kill = Arc::new(Notify::new());
    let writer_kill = kill.clone();
    let writer = tokio::spawn(async move {
        while let Some(m) = outgoing.recv().await {
            let close = matches!(m, Message::Close(_));
            if sink.send(m).await.is_err() || close {
                break;
            }
        }
        let _ = sink.close().await;
        // The reader may be waiting on a peer that never says more: it stops too.
        writer_kill.notify_one();
    });
    let link = Link { out, kill };
    let mut line = Line::new(reader, link, &hub.config.limits, hub.config.clock.clone());
    match role {
        Role_::Pc => crate::pc::serve(&hub, &mut line, address).await,
        Role_::Phone => crate::phone::serve(&hub, &mut line, address).await,
    }
    // Over: the queue closes and the writer sends what is left, its close last; meanwhile the
    // line reads what the peer still sends, until its close comes back (`Line::finish`), so the
    // socket never closes on a peer that is still sending.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(line.finish(CLOSE_WAIT), writer)
    })
    .await;
    hub.leave(address);
}

/// Read one request head (up to the blank line), or nothing if it does not come whole in time.
async fn read_head(stream: &mut TcpStream, wait: std::time::Duration) -> Option<Vec<u8>> {
    let read = async {
        let mut head = Vec::with_capacity(1024);
        let mut chunk = [0u8; 1024];
        loop {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            head.extend_from_slice(&chunk[..n]);
            if head.windows(4).any(|w| w == b"\r\n\r\n") {
                return Some(head);
            }
            if head.len() > MAX_HEAD {
                return None;
            }
        }
    };
    tokio::time::timeout(wait, read).await.ok().flatten()
}

/// A short plain-text answer, then the door closes.
async fn answer(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn header<'a>(headers: &'a [httparse::Header<'a>], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .rev()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .map(str::trim)
}

fn is_websocket_upgrade(headers: &[httparse::Header<'_>]) -> bool {
    let upgrade = header(headers, "upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let connection = header(headers, "connection").is_some_and(|v| {
        v.split(',')
            .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
    });
    let version = header(headers, "sec-websocket-version").is_some_and(|v| v == "13");
    upgrade && connection && version
}

/// The connection's internet address, read where the settings say.
fn client_address(
    mode: ClientAddress,
    headers: &[httparse::Header<'_>],
    peer: IpAddr,
) -> AddressKey {
    let parse = |text: &str| -> Option<IpAddr> {
        let text = text.trim().trim_start_matches('[');
        // "[v6]:port" or "v4:port" from some proxies: the address alone.
        let text = text
            .split_once("]:")
            .map(|(a, _)| a)
            .unwrap_or(text)
            .trim_end_matches(']');
        text.parse::<IpAddr>().ok().or_else(|| {
            text.rsplit_once(':')
                .filter(|(a, _)| a.matches('.').count() == 3)
                .and_then(|(a, _)| a.parse::<IpAddr>().ok())
        })
    };
    let forwarded = || {
        header(headers, "x-forwarded-for")
            .and_then(|v| v.rsplit(',').next())
            .and_then(parse)
            .or_else(|| header(headers, "x-real-ip").and_then(parse))
    };
    let ip = match mode {
        ClientAddress::Peer => None,
        ClientAddress::Proxy => forwarded(),
        ClientAddress::Cloudflare => header(headers, "cf-connecting-ip")
            .and_then(parse)
            .or_else(forwarded),
    };
    AddressKey::from(ip.unwrap_or(peer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers<'a>(list: &'a [(&'a str, &'a str)]) -> Vec<httparse::Header<'a>> {
        list.iter()
            .map(|(name, value)| httparse::Header {
                name,
                value: value.as_bytes(),
            })
            .collect()
    }

    #[test]
    fn the_address_comes_from_where_the_settings_say() {
        let peer: IpAddr = "127.0.0.1".parse().unwrap();
        let h = headers(&[
            ("X-Forwarded-For", "198.51.100.7, 203.0.113.9"),
            ("X-Real-IP", "203.0.113.9"),
            ("CF-Connecting-IP", "2001:db8:1:2::77"),
        ]);
        assert_eq!(
            client_address(ClientAddress::Peer, &h, peer).to_string(),
            "127.0.0.1"
        );
        // The proxy's own last entry, never one the client wrote first.
        assert_eq!(
            client_address(ClientAddress::Proxy, &h, peer).to_string(),
            "203.0.113.9"
        );
        assert_eq!(
            client_address(ClientAddress::Cloudflare, &h, peer).to_string(),
            "2001:db8:1:2::/64"
        );
        let only_real = headers(&[("X-Real-IP", "[2001:db8::1]:4433")]);
        assert_eq!(
            client_address(ClientAddress::Proxy, &only_real, peer).to_string(),
            "2001:db8:0:0::/64"
        );
        let nonsense = headers(&[("X-Forwarded-For", "not an address")]);
        assert_eq!(
            client_address(ClientAddress::Cloudflare, &nonsense, peer).to_string(),
            "127.0.0.1"
        );
    }

    #[test]
    fn only_a_real_upgrade_counts() {
        let good = headers(&[
            ("Connection", "keep-alive, Upgrade"),
            ("Upgrade", "WebSocket"),
            ("Sec-WebSocket-Version", "13"),
        ]);
        assert!(is_websocket_upgrade(&good));
        let no_version = headers(&[("Connection", "Upgrade"), ("Upgrade", "websocket")]);
        assert!(!is_websocket_upgrade(&no_version));
        assert!(!is_websocket_upgrade(&headers(&[])));
    }
}
