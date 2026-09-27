//! A synthetic website for the Phase 10 browser tests: a tiny web server on 127.0.0.1 that the
//! test browser reaches under made-up names (`--host-resolver-rules` maps `*.test` to it), so
//! no test ever touches the internet. It records every form and message it receives (over HTTP
//! and over a WebSocket, at `/ws`), so tests can check that nothing was sent without the owner's
//! approval.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

/// What the site received: method, path, and body (`WS` for a text frame over a WebSocket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub body: String,
}

#[derive(Clone)]
pub struct Site {
    pub port: u16,
    received: Arc<Mutex<Vec<Received>>>,
    task: Arc<tokio::task::JoinHandle<()>>,
}

impl Drop for Site {
    fn drop(&mut self) {
        if Arc::strong_count(&self.task) == 1 {
            self.task.abort();
        }
    }
}

const STYLE: &str = "<style>body{font:16px system-ui;margin:40px;background:#fff}\
    label{display:block;margin:8px 0}input,textarea{font:inherit;width:320px}\
    button{font:inherit;padding:6px 14px}</style>";

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=utf-8><title>{title}</title>{STYLE}</head>\
         <body><h1>{title}</h1>{body}</body></html>"
    )
}

impl Site {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let log = Arc::clone(&log);
                tokio::spawn(async move {
                    let _ = serve(stream, port, log).await;
                });
            }
        });
        Self {
            port,
            received,
            task: Arc::new(task),
        }
    }

    /// `http://<host>.test:<port><path>`.
    pub fn url(&self, host: &str, path: &str) -> String {
        format!("http://{host}.test:{}{path}", self.port)
    }

    /// Forms and messages received so far (POST and other sending requests, and WebSocket
    /// frames).
    pub fn sent(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method != "GET")
            .cloned()
            .collect()
    }
}

async fn serve(
    mut stream: TcpStream,
    port: u16,
    log: Arc<Mutex<Vec<Received>>>,
) -> std::io::Result<()> {
    if let Some(path) = websocket_upgrade(&stream).await? {
        return serve_socket(stream, path, log).await;
    }
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let (head, body_start) = loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&data[..i]).into_owned(), i + 4);
        }
    };
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("GET").to_owned();
    let path = first.next().unwrap_or("/").to_owned();
    let length: usize = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    while data.len() < body_start + length {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    let body = String::from_utf8_lossy(&data[body_start..]).into_owned();
    log.lock().unwrap().push(Received {
        method: method.clone(),
        path: path.clone(),
        body: body.clone(),
    });
    let (status, location, html) = route(&method, &path, &body, port);
    if path == "/slow" {
        // Never answers: the browser's time limit must end it.
        tokio::time::sleep(Duration::from_secs(600)).await;
        return Ok(());
    }
    let text_file = path.ends_with(".txt");
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n",
        if text_file { "text/plain" } else { "text/html" },
        html.len()
    );
    if let Some(l) = location {
        response.push_str(&format!("Location: {l}\r\n"));
    }
    if path == "/report.txt" {
        // A file the website says to save rather than show (ADR-037).
        response.push_str("Content-Disposition: attachment; filename=\"report.txt\"\r\n");
    }
    response.push_str("\r\n");
    response.push_str(&html);
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

/// The request is a WebSocket handshake: its path. Peeks, so the stream still holds the whole
/// request for the handshake itself.
async fn websocket_upgrade(stream: &TcpStream) -> std::io::Result<Option<String>> {
    let mut buf = [0u8; 8192];
    for _ in 0..500 {
        let n = stream.peek(&mut buf).await?;
        if n == 0 {
            return Ok(None);
        }
        let head = String::from_utf8_lossy(&buf[..n]);
        if head.contains("\r\n\r\n") || n == buf.len() {
            let upgrade = head.lines().any(|l| {
                let l = l.to_ascii_lowercase();
                l.starts_with("upgrade:") && l.contains("websocket")
            });
            let path = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_owned();
            return Ok(upgrade.then_some(path));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(None)
}

/// A live connection: complete the handshake, then record every text frame the page sends,
/// until the page closes it.
async fn serve_socket(
    stream: TcpStream,
    path: String,
    log: Arc<Mutex<Vec<Received>>>,
) -> std::io::Result<()> {
    let mut socket = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(std::io::Error::other)?;
    while let Some(Ok(message)) = socket.next().await {
        if let Message::Text(text) = message {
            log.lock().unwrap().push(Received {
                method: "WS".into(),
                path: path.clone(),
                body: text.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn route(
    method: &str,
    path: &str,
    body: &str,
    port: u16,
) -> (&'static str, Option<String>, String) {
    let ok = |html: String| ("200 OK", None, html);
    // The framed check's variant that opens a puzzle instead of passing.
    let puzzle = path.contains("puzzle");
    match (method, path.split('?').next().unwrap_or("/")) {
        ("GET", "/") => ok(page(
            "Synthetic Shop",
            "<p>Welcome to the test shop.</p><ul>\
             <li><a href=\"/form\">Contact us</a></li>\
             <li><a href=\"/shop\">Shop</a></li>\
             <li><a href=\"/login\">Sign in</a></li></ul>",
        )),
        ("GET", "/form") => ok(page(
            "Contact us",
            "<form method=post action=\"/send\">\
             <label>Your name <input name=name></label>\
             <label>Email <input name=email type=email></label>\
             <label>Message <textarea name=message></textarea></label>\
             <button type=submit>Send message</button></form>",
        )),
        ("POST", "/send") => {
            let name = body
                .split('&')
                .find_map(|kv| kv.strip_prefix("name="))
                .unwrap_or("")
                .replace('+', " ");
            ok(page(
                "Thank you",
                &format!("<p>Thanks, {name}. We got your message.</p>"),
            ))
        }
        ("GET", "/login") => ok(page(
            "Sign in",
            "<form method=post action=\"/session\">\
             <label>Email <input name=email></label>\
             <label>Password <input name=password type=password></label>\
             <button type=submit>Sign in</button></form>",
        )),
        ("GET", "/shop") => ok(page(
            "Shop",
            "<p>Blue mug — $12</p><form method=post action=\"/checkout\">\
             <input type=hidden name=item value=mug>\
             <button type=submit>Buy now</button></form>",
        )),
        ("POST", "/checkout") => ok(page("Order placed", "<p>Your order is placed.</p>")),
        ("GET", "/captcha") => ok(page(
            "Check",
            "<p>Please confirm you are a person.</p><div class=\"g-recaptcha\" \
             data-sitekey=\"test\"><button type=button>I'm not a robot</button>\
             <input name=answer></div>",
        )),
        // A check like a real one (ADR-032): its checkbox is inside its own frame, and passing
        // it writes the answer into the page, where Plenipo reads it. With `?puzzle`, the
        // check opens a puzzle frame instead.
        ("GET", "/captcha-frame") => ok(page(
            "Framed check",
            &format!(
                "<p>Please confirm you are a person.</p>\
                 <div class=\"g-recaptcha\" data-sitekey=\"test\">\
                 <iframe src=\"/recaptcha/anchor{}\" title=\"reCAPTCHA\" width=304 height=78 \
                 style=\"border:0\"></iframe>\
                 <textarea name=\"g-recaptcha-response\" style=\"display:none\"></textarea></div>\
                 <div id=puzzle style=\"visibility:hidden;position:absolute;top:160px;left:40px\">\
                 <iframe src=\"/recaptcha/bframe\" title=\"recaptcha challenge expires in two \
                 minutes\" width=400 height=580 style=\"border:0\"></iframe></div>\
                 <form method=post action=\"/send\"><input type=hidden name=name value=checked>\
                 <button type=submit>Send</button></form>\
                 <script>addEventListener('message', e => {{ \
                 if (e.data === 'plenipo-test:passed') \
                 document.querySelector('[name=g-recaptcha-response]').value = 'token-' + Date.now(); \
                 if (e.data === 'plenipo-test:puzzle') \
                 document.getElementById('puzzle').style.visibility = 'visible'; }})</script>",
                if puzzle { "?puzzle" } else { "" }
            ),
        )),
        // The check's widget, laid out like reCAPTCHA's: a 28-pixel checkbox at the left of a
        // 304 × 78 row, then the words. Only the checkbox itself answers.
        ("GET", "/recaptcha/anchor") => ok(format!(
            "<!doctype html><html><head><meta charset=utf-8><style>body{{margin:0;font:14px \
             system-ui}}.rc{{display:flex;align-items:center;height:78px;width:304px;\
             box-sizing:border-box;border:2px solid #d3d3d3;background:#f9f9f9}}\
             #box{{width:28px;height:28px;margin:0 12px;border:2px solid #c1c1c1;\
             border-radius:2px;background:#fff;box-sizing:border-box}}#box.on{{border-color:\
             #1a73e8}}</style></head><body><div class=rc><div id=box role=checkbox \
             aria-checked=false></div><span>I'm not a robot</span></div>\
             <script>document.getElementById('box').onclick = () => {{ \
             document.getElementById('box').className = 'on'; \
             parent.postMessage({}, '*') }}</script></body></html>",
            if puzzle {
                "'plenipo-test:puzzle'"
            } else {
                "'plenipo-test:passed'"
            }
        )),
        ("GET", "/recaptcha/bframe") => ok(page(
            "Puzzle",
            "<p>Select all images with a bus.</p>",
        )),
        ("GET", "/auto") => ok(page(
            "Auto",
            "<form id=f method=post action=\"/send\"><input name=name value=auto></form>\
             <script>document.getElementById('f').submit()</script>",
        )),
        ("GET", "/script-send") => ok(page(
            "Chat",
            "<p id=out>Ready</p><button type=button id=go>Go</button>\
             <script>document.getElementById('go').onclick = () => \
             fetch('/api/messages', {method: 'POST', body: 'hello'}).then(r => \
             document.getElementById('out').textContent = 'Sent: ' + r.status)</script>",
        )),
        ("POST", "/api/messages") => ok("{\"ok\":true}".into()),
        // A chat: its composer is a contenteditable outside any <form>, and Enter (or "Go")
        // sends what it holds over a live connection (a WebSocket) the network gate cannot see
        // into (ADR-035).
        ("GET", "/chat") => ok(page(
            "Chat",
            "<p id=out>Connecting</p>\
             <div id=composer contenteditable=true role=textbox aria-label=\"Message\" \
             style=\"border:1px solid #888;min-height:24px;width:320px;padding:4px\"></div>\
             <button type=button id=go>Go</button>\
             <script>const ws = new WebSocket('ws://' + location.host + '/ws'); \
             ws.onopen = () => document.getElementById('out').textContent = 'Connected'; \
             const send = () => { const c = document.getElementById('composer'); \
             ws.send(c.textContent); c.textContent = ''; \
             document.getElementById('out').textContent = 'Sent'; }; \
             document.getElementById('composer').addEventListener('keydown', e => { \
             if (e.key === 'Enter') { e.preventDefault(); send(); } }); \
             document.getElementById('go').onclick = send</script>",
        )),
        // A page whose harmless-looking button sends data on its own, 2.5 seconds later: long
        // after Plenipo stops watching the click (ADR-035).
        ("GET", "/late-send") => ok(page(
            "Late send",
            "<p id=out>Ready</p><button type=button id=go>Go</button> \
             <button type=button id=buy>Buy now</button>\
             <script>document.getElementById('go').onclick = () => { \
             document.getElementById('out').textContent = 'Saving soon'; \
             setTimeout(() => fetch('/api/messages', {method: 'POST', body: 'late'}).then(r => \
             document.getElementById('out').textContent = 'Sent: ' + r.status, () => \
             document.getElementById('out').textContent = 'Stopped'), 2500) }</script>",
        )),
        ("GET", "/to-blocked") => (
            "302 Found",
            Some(format!("http://blocked.test:{port}/")),
            String::new(),
        ),
        ("GET", "/secret-field") => ok(page(
            "Account",
            "<form method=post action=\"/save\"><label>API key \
             <input name=key></label><button type=submit>Save</button></form>",
        )),
        // Two links that save a file rather than open a page (ADR-037): the first because the
        // website answers with `Content-Disposition: attachment`, the second because the link
        // itself says so (its `download` attribute).
        ("GET", "/download") => ok(page(
            "Files to save",
            "<p>Take the files.</p><ul>\
             <li><a href=\"/report.txt\">Download the report</a></li>\
             <li><a href=\"/notes.txt\" download=\"notes.txt\">Save the notes</a></li></ul>",
        )),
        ("GET", "/report.txt") => ok("The quarterly report.\n".into()),
        ("GET", "/notes.txt") => ok("Notes for the worker.\n".into()),
        _ => (
            "404 Not Found",
            None,
            page("Not found", "<p>No such page.</p>"),
        ),
    }
}
