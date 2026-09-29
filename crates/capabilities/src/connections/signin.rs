//! Signing in to a connection in the owner's own browser (Phase 20, ADR-063): the authorization
//! code flow with PKCE (RFC 7636) and a one-time loopback listener (RFC 8252).
//!
//! - A fresh PKCE secret and a fresh `state` for every sign-in (32 random bytes each).
//! - A listener on `127.0.0.1` and `[::1]` at one port Windows picks (a port another program
//!   holds on either is never used; `[::1]` is left out only where this computer has no IPv6).
//!   The owner's browser is opened at its start page, `/start/<random>`, which sends the browser
//!   on to the service's sign-in page **once** and is "not found" after — so the address Plenipo
//!   hands to Windows holds nothing but this computer's port and a key used up by the owner's
//!   browser, and no other program on this computer can learn the sign-in's `state` or challenge
//!   from it. The listener's answer is **one** request: `GET /?code=…&state=…` whose `state`
//!   matches. Anything else gets "not found" and is ignored. The page it shows never repeats
//!   anything from the request. It stops after [`WAIT`], on Cancel, or once answered.
//! - Nothing here is recorded or logged: not the code, the state, or the secret.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use base64::Engine as _;
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

/// The longest a sign-in waits for the owner's browser.
pub const WAIT: Duration = Duration::from_secs(10 * 60);
/// The longest request line and headers the listener reads.
const MAX_REQUEST: usize = 16 * 1024;

/// What the page in the owner's browser says once Plenipo has the answer.
const DONE_PAGE: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>Plenipo</title></head><body style=\"font-family: system-ui, sans-serif; margin: 3rem\">\
<h1>Signed in.</h1><p>You can close this tab and go back to Plenipo.</p></body></html>";
const REFUSED_PAGE: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>Plenipo</title></head><body style=\"font-family: system-ui, sans-serif; margin: 3rem\">\
<h1>Not signed in.</h1><p>You can close this tab. Plenipo says what happened on the \
connection's card.</p></body></html>";

fn random_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    out[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    out
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// A fresh PKCE secret, its challenge, and a fresh `state`.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = b64(&random_bytes());
        let challenge = b64(&Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
            state: b64(&random_bytes()),
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// What came back to the listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    /// The service's code, to trade for tokens.
    Code(String),
    /// The service said no: its error code and description.
    Refused { error: String, description: String },
}

/// How many ports are tried for one that is free on both `127.0.0.1` and `[::1]`.
const PORT_TRIES: usize = 20;

/// A listener waiting for one answer.
pub struct Listener {
    pub port: u16,
    v4: TcpListener,
    v6: Option<TcpListener>,
    /// The start page's one-time key.
    start: String,
}

impl Listener {
    /// Listen on `127.0.0.1` and `[::1]` at one port the system picks. A browser opening
    /// `localhost` may try either, so a port another program holds on `[::1]` is never used;
    /// only a computer without IPv6 gets `127.0.0.1` alone.
    pub async fn open() -> std::io::Result<Self> {
        let mut last = None;
        for _ in 0..PORT_TRIES {
            let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
            let port = v4.local_addr()?.port();
            match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))).await {
                Ok(v6) => return Ok(Self::new(port, v4, Some(v6))),
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last = Some(e),
                // No IPv6 on this computer: `localhost` means `127.0.0.1` alone.
                Err(_) => return Ok(Self::new(port, v4, None)),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("no free port")))
    }

    /// Listen on the first of `ports` free on both `127.0.0.1` and `[::1]` (Slack comes back
    /// only to addresses written into its app, port included; ADR-070 §5.2). A port of `0` means
    /// any port, as [`Listener::open`].
    pub async fn open_on(ports: &[u16]) -> std::io::Result<Self> {
        let mut last = None;
        for port in ports {
            if *port == 0 {
                return Self::open().await;
            }
            let v4 = match TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, *port))).await {
                Ok(v4) => v4,
                Err(e) => {
                    last = Some(e);
                    continue;
                }
            };
            match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, *port))).await {
                Ok(v6) => return Ok(Self::new(*port, v4, Some(v6))),
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last = Some(e),
                // No IPv6 on this computer: `localhost` means `127.0.0.1` alone.
                Err(_) => return Ok(Self::new(*port, v4, None)),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("no free port")))
    }

    fn new(port: u16, v4: TcpListener, v6: Option<TcpListener>) -> Self {
        Self {
            port,
            v4,
            v6,
            start: b64(&random_bytes()),
        }
    }

    /// The address the owner's browser is opened at: this computer's one-time start page.
    pub fn start_address(&self) -> String {
        format!("http://localhost:{}/start/{}", self.port, self.start)
    }

    /// Wait for the one request whose `state` matches, until `cancel` fires or [`WAIT`] passes;
    /// meanwhile `/start` sends the browser on to `sign_in` (the service's sign-in address).
    /// `None`: cancelled or timed out.
    pub async fn wait(
        self,
        sign_in: &str,
        state: &str,
        cancel: oneshot::Receiver<()>,
    ) -> Option<Callback> {
        let serve = async {
            let mut start = Some(self.start.as_str());
            loop {
                let stream = match &self.v6 {
                    Some(v6) => tokio::select! {
                        r = self.v4.accept() => r.map(|(s, _)| s),
                        r = v6.accept() => r.map(|(s, _)| s),
                    },
                    None => self.v4.accept().await.map(|(s, _)| s),
                };
                let Ok(stream) = stream else { continue };
                match answer(stream, sign_in, state, start).await {
                    Some(Asked::Answer(answer)) => return answer,
                    // The start page is used up once the owner's browser has it.
                    Some(Asked::Start) => start = None,
                    None => {}
                }
            }
        };
        tokio::select! {
            answer = serve => Some(answer),
            _ = cancel => None,
            _ = tokio::time::sleep(WAIT) => None,
        }
    }
}

/// What a request to the listener is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// `/start`: send the browser on to the sign-in page.
    Start,
    /// The answer waited for.
    Answer(Callback),
}

/// Read one request and answer it; what it was, when it was the start page or the answer.
async fn answer(
    mut stream: TcpStream,
    sign_in: &str,
    state: &str,
    start: Option<&str>,
) -> Option<Asked> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    let read = tokio::time::timeout(Duration::from_secs(10), async {
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    })
    .await;
    let asked = read.ok().and_then(|()| parse(&buf, state, start));
    let (status, page, location) = match &asked {
        Some(Asked::Start) => ("302 Found", "", Some(sign_in)),
        Some(Asked::Answer(Callback::Code(_))) => ("200 OK", DONE_PAGE, None),
        Some(Asked::Answer(Callback::Refused { .. })) => ("200 OK", REFUSED_PAGE, None),
        None => ("404 Not Found", "", None),
    };
    let location = location.map_or_else(String::new, |l| format!("Location: {l}\r\n"));
    let response = format!(
        "HTTP/1.1 {status}\r\n{location}Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
    asked
}

/// What a request is: `GET /start/<key>` while the start page is not used up (`start`), or
/// `GET /?…` with the right `state`.
pub fn parse(request: &[u8], state: &str, start: Option<&str>) -> Option<Asked> {
    let text = std::str::from_utf8(request).ok()?;
    let line = text.lines().next()?;
    let mut words = line.split(' ');
    let (method, target) = (words.next()?, words.next()?);
    if method != "GET" {
        return None;
    }
    if let Some(key) = target.strip_prefix("/start/") {
        return (start == Some(key)).then_some(Asked::Start);
    }
    if !target.starts_with("/?") {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://localhost{target}")).ok()?;
    let get = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };
    // Constant-time enough: both are random strings of the same length.
    if get("state").as_deref() != Some(state) {
        return None;
    }
    if let Some(code) = get("code").filter(|c| !c.is_empty() && c.len() <= 4096) {
        return Some(Asked::Answer(Callback::Code(code)));
    }
    get("error").map(|error| {
        Asked::Answer(Callback::Refused {
            error: error.chars().take(100).collect(),
            description: get("error_description")
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_sign_in_gets_fresh_secrets_and_the_challenge_is_their_sha256() {
        let a = Pkce::new();
        let b = Pkce::new();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
        assert_eq!(a.verifier.len(), 43);
        assert_eq!(a.challenge, b64(&Sha256::digest(a.verifier.as_bytes())));
    }

    #[test]
    fn only_a_get_with_the_right_state_is_the_answer() {
        let ok = b"GET /?code=abc&state=s1 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let k = Some("k1");
        assert_eq!(
            parse(ok, "s1", k),
            Some(Asked::Answer(Callback::Code("abc".into())))
        );
        assert_eq!(
            parse(b"GET /start/k1 HTTP/1.1\r\n\r\n", "s1", k),
            Some(Asked::Start)
        );
        // The start page only with its key, and only until it is used.
        assert_eq!(parse(b"GET /start HTTP/1.1\r\n\r\n", "s1", k), None);
        assert_eq!(parse(b"GET /start/k2 HTTP/1.1\r\n\r\n", "s1", k), None);
        assert_eq!(parse(b"GET /start/k1 HTTP/1.1\r\n\r\n", "s1", None), None);
        assert_eq!(parse(ok, "s2", k), None, "a wrong state is ignored");
        assert_eq!(
            parse(b"POST /?code=abc&state=s1 HTTP/1.1\r\n\r\n", "s1", k),
            None
        );
        assert_eq!(
            parse(b"GET /other?code=abc&state=s1 HTTP/1.1\r\n\r\n", "s1", k),
            None
        );
        assert_eq!(parse(b"GET /?code=abc HTTP/1.1\r\n\r\n", "s1", k), None);
        let refused =
            b"GET /?error=access_denied&error_description=AADSTS65001%3A+no&state=s1 HTTP/1.1\r\n\r\n";
        assert_eq!(
            parse(refused, "s1", k),
            Some(Asked::Answer(Callback::Refused {
                error: "access_denied".into(),
                description: "AADSTS65001: no".into()
            }))
        );
    }

    #[tokio::test]
    async fn the_listener_answers_once_and_ignores_the_rest() {
        let listener = Listener::open().await.unwrap();
        let port = listener.port;
        let start = listener.start_address();
        let start = start
            .strip_prefix(&format!("http://localhost:{port}"))
            .unwrap()
            .to_owned();
        assert!(start.starts_with("/start/") && start.len() > 40, "{start}");
        let (_cancel, rx) = oneshot::channel();
        let waiting =
            tokio::spawn(
                async move { listener.wait("https://sign.in/x?state=s1", "s1", rx).await },
            );
        let send = |path: &'static str| async move {
            let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
                .await
                .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            out
        };
        // The start page sends the browser on to the sign-in page, once; then it is gone, and
        // another program on this computer learns nothing from it.
        let path: &'static str = Box::leak(start.into_boxed_str());
        let first = send(path).await;
        assert!(first.starts_with("HTTP/1.1 302"));
        assert!(first.contains("Location: https://sign.in/x?state=s1"));
        let again = send(path).await;
        assert!(again.starts_with("HTTP/1.1 404"), "{again}");
        assert!(!again.contains("state=s1"));
        assert!(send("/start").await.starts_with("HTTP/1.1 404"));
        // A stranger's request: "not found", and the listener keeps waiting.
        let stranger = send("/?code=evil&state=guess").await;
        assert!(stranger.starts_with("HTTP/1.1 404"));
        let page = send("/?code=abc&state=s1").await;
        assert!(page.contains("Signed in."));
        assert!(!page.contains("abc"), "the page never repeats the code");
        assert_eq!(waiting.await.unwrap(), Some(Callback::Code("abc".into())));
    }

    #[tokio::test]
    async fn a_fixed_port_is_used_when_free_and_the_next_one_when_not() {
        // A port another program holds is passed over for the next one.
        let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let busy = taken.local_addr().unwrap().port();
        let free = {
            let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            l.local_addr().unwrap().port()
        };
        let listener = Listener::open_on(&[busy, free]).await.unwrap();
        assert_eq!(listener.port, free);
        assert!(listener
            .start_address()
            .starts_with(&format!("http://localhost:{free}/start/")));
        drop(listener);
        // All of them taken: a plain error, never another port.
        assert!(Listener::open_on(&[busy]).await.is_err());
    }

    #[tokio::test]
    async fn cancel_ends_the_wait() {
        let listener = Listener::open().await.unwrap();
        let (cancel, rx) = oneshot::channel();
        let waiting = tokio::spawn(async move { listener.wait("https://x.y/", "s1", rx).await });
        cancel.send(()).unwrap();
        assert_eq!(waiting.await.unwrap(), None);
    }
}
