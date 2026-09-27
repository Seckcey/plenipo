//! A synthetic website for the Phase 10 browser tests: a tiny web server on 127.0.0.1 that the
//! test browser reaches under made-up names (`--host-resolver-rules` maps `*.test` to it), so
//! no test ever touches the internet. It records every form and message it receives, so tests
//! can check that nothing was sent without the owner's approval.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

/// What the site received: method, path, and body.
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

    /// Forms and messages received so far (POST and other sending requests).
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
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n",
        html.len()
    );
    if let Some(l) = location {
        response.push_str(&format!("Location: {l}\r\n"));
    }
    response.push_str("\r\n");
    response.push_str(&html);
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

fn route(
    method: &str,
    path: &str,
    body: &str,
    port: u16,
) -> (&'static str, Option<String>, String) {
    let ok = |html: String| ("200 OK", None, html);
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
        _ => (
            "404 Not Found",
            None,
            page("Not found", "<p>No such page.</p>"),
        ),
    }
}
