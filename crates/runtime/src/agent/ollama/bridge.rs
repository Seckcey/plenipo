//! Plenipo's Ollama bridge (ADR-017): the program a task runs for Ollama.
//!
//! Plenipo runs itself in this mode (`plenipo-desktop --plenipo-ollama …`) as the task's
//! supervised process, so a task keeps its time limit, cancel, and records. The bridge talks
//! only to the Ollama service on this PC (`127.0.0.1:11434`; `OLLAMA_HOST` is ignored) and
//! writes one JSON object per line on stdout, which the Ollama adapter's parser reads:
//!
//! - `auth`: `POST /api/me` → `{"signedIn":true,"plan":"free"}`, `{"signedIn":false}`, or
//!   `{"error":"…"}`. Never the account's email or name.
//! - `chat --model M --session ID [--resume] [--think LEVEL]`, the prompt on stdin:
//!   `POST /api/chat` (streamed) → `session`, `thinking`, `text` (pieces of the answer),
//!   `answer` (the whole answer), `done` (token counts), `notice` (with `leftOut` when earlier
//!   messages were left out), or `error` lines.
//!
//! Ollama keeps no conversations, so the bridge keeps each one in the session's own folder
//! (`.plenipo-ollama-<ID>.json`: the objectives and answers so far) and sends it with every
//! task. A task that does not finish leaves it unchanged.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The argument that selects bridge mode (first after the program name).
pub const ARG: &str = "--plenipo-ollama";
/// The Ollama service's port on this PC.
pub const DEFAULT_PORT: u16 = 11434;
/// Conversation text sent with a task, at most (about 100,000 tokens); older messages beyond it
/// are left out and the model is told so.
const MAX_HISTORY_CHARS: usize = 400_000;
/// Longest prompt read from stdin.
const MAX_PROMPT_BYTES: u64 = 4 * 1024 * 1024;
/// Longest line read from the service.
const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Run bridge mode when the arguments ask for it; otherwise `None` (start Plenipo normally).
pub fn maybe_run_from_args(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    if args.next().as_deref() != Some(ARG) {
        return None;
    }
    let rest: Vec<String> = args.collect();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    Some(run(&rest, &mut std::io::stdin().lock(), &mut out))
}

/// One bridge command: its exit code. Output goes to `out`, one JSON object per line.
pub fn run(args: &[String], input: &mut dyn Read, out: &mut dyn Write) -> i32 {
    let port = flag(args, "--port")
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    match args.first().map(String::as_str) {
        Some("auth") => auth(port, out),
        Some("chat") => chat(port, args, input, out),
        _ => {
            emit(
                out,
                &json!({ "type": "error", "message": "Unknown Ollama bridge command" }),
            );
            2
        }
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn emit(out: &mut dyn Write, v: &Value) {
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

// ---- The sign-in check ------------------------------------------------------------------------

fn auth(port: u16, out: &mut dyn Write) -> i32 {
    match request(port, "POST", "/api/me", b"", Some(PROBE_TIMEOUT)) {
        Ok(mut response) if response.status == 200 => {
            let body = response.body_text();
            let plan = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("plan").and_then(Value::as_str).map(plan_word))
                .unwrap_or_default();
            emit(out, &json!({ "signedIn": true, "plan": plan }));
            0
        }
        Ok(response) if response.status == 401 => {
            emit(out, &json!({ "signedIn": false }));
            0
        }
        Ok(mut response) => {
            let detail = error_text(response.status, &response.body_text());
            emit(out, &json!({ "error": detail }));
            1
        }
        Err(e) => {
            emit(out, &json!({ "error": e }));
            1
        }
    }
}

/// A plan name, kept to a short word (never an account detail).
fn plan_word(plan: &str) -> String {
    plan.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
        .take(32)
        .collect()
}

// ---- A task -----------------------------------------------------------------------------------

fn chat(port: u16, args: &[String], input: &mut dyn Read, out: &mut dyn Write) -> i32 {
    let fail = |out: &mut dyn Write, message: &str| {
        emit(out, &json!({ "type": "error", "message": message }));
        1
    };
    let Some(model) = flag(args, "--model").filter(|m| !m.is_empty()) else {
        return fail(out, "No model was given");
    };
    let Some(id) = flag(args, "--session").filter(|id| valid_id(id)) else {
        return fail(out, "No valid conversation ID was given");
    };
    let resume = args.iter().any(|a| a == "--resume");
    let think = flag(args, "--think");

    let mut prompt = String::new();
    if input
        .take(MAX_PROMPT_BYTES)
        .read_to_string(&mut prompt)
        .is_err()
        || prompt.trim().is_empty()
    {
        return fail(out, "No prompt was given");
    }
    // The session's folder is the task's working directory; tests name another.
    let dir = flag(args, "--dir").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let file = conversation_file(&dir, &id);
    let mut history = if resume {
        match load(&file) {
            Some(h) => h,
            None => {
                return fail(
                    out,
                    "This conversation's history was not found; start a new conversation",
                )
            }
        }
    } else {
        Vec::new()
    };
    emit(out, &json!({ "type": "session", "id": id, "model": model }));

    let (sent, left_out) = fitting(&history, &prompt);
    if left_out > 0 {
        // The model no longer sees the start of the conversation: `leftOut` tells Plenipo so
        // (ADR-044 §2.5), and the text says it in plain words.
        emit(
            out,
            &json!({ "type": "notice", "leftOut": left_out, "text": format!(
                "{left_out} earlier message(s) were left out: the conversation is longer than Plenipo sends at once."
            ) }),
        );
    }
    let mut body = json!({ "model": model, "stream": true, "messages": sent });
    if let Some(level) = &think {
        body["think"] = json!(level);
    }

    let started = Instant::now();
    let mut response = match request(port, "POST", "/api/chat", body.to_string().as_bytes(), None) {
        Ok(r) => r,
        Err(e) => return fail(out, &e),
    };
    if response.status != 200 {
        let text = response.body_text();
        return fail(out, &error_text(response.status, &text));
    }

    let (mut thinking, mut answer) = (String::new(), String::new());
    let mut done: Option<Value> = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        match read_line_capped(&mut response.reader, &mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => return fail(out, &format!("Reading Ollama's answer failed: {e}")),
        }
        let Ok(v) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if let Some(e) = v.get("error").and_then(Value::as_str) {
            return fail(out, &format!("Ollama reported an error: {e}"));
        }
        let message = v.get("message").cloned().unwrap_or(Value::Null);
        if let Some(t) = message.get("thinking").and_then(Value::as_str) {
            thinking.push_str(t);
        }
        if let Some(t) = message.get("content").and_then(Value::as_str) {
            if !t.is_empty() {
                if !thinking.is_empty() {
                    emit(
                        out,
                        &json!({ "type": "thinking", "text": std::mem::take(&mut thinking) }),
                    );
                }
                answer.push_str(t);
                emit(out, &json!({ "type": "text", "text": t }));
            }
        }
        if v.get("done").and_then(Value::as_bool) == Some(true) {
            done = Some(v);
            break;
        }
    }
    if !thinking.is_empty() {
        emit(out, &json!({ "type": "thinking", "text": thinking }));
    }
    let Some(done) = done else {
        return fail(out, "Ollama's answer ended before it was complete");
    };
    emit(out, &json!({ "type": "answer", "text": answer }));
    let n = |k: &str| done.get(k).and_then(Value::as_u64).unwrap_or(0);
    emit(
        out,
        &json!({
            "type": "done",
            "reason": done.get("done_reason").and_then(Value::as_str).unwrap_or("stop"),
            "inputTokens": n("prompt_eval_count"),
            "cachedTokens": n("prompt_eval_cached_count"),
            "outputTokens": n("eval_count"),
            "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        }),
    );
    history.push(json!({ "role": "user", "content": prompt }));
    history.push(json!({ "role": "assistant", "content": answer }));
    if let Err(e) = save(&file, &model, &history) {
        emit(
            out,
            &json!({ "type": "notice", "text": format!("The conversation could not be saved for the next task: {e}") }),
        );
    }
    0
}

/// Conversation IDs Plenipo gives: letters, digits, and dashes.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The conversation's file, in the session's own folder.
fn conversation_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!(".plenipo-ollama-{id}.json"))
}

fn load(file: &Path) -> Option<Vec<Value>> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    v.get("messages")?.as_array().cloned()
}

fn save(file: &Path, model: &str, messages: &[Value]) -> std::io::Result<()> {
    let tmp = file.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        json!({ "model": model, "messages": messages }).to_string(),
    )?;
    std::fs::rename(&tmp, file)
}

/// How many of the oldest messages of `history` are left out to fit a prompt of `prompt_len`
/// bytes: whole exchanges (the user's message and the answer) are kept, newest first.
fn left_out(history: &[Value], prompt_len: usize) -> usize {
    let size = |m: &Value| m.get("content").and_then(Value::as_str).map_or(0, str::len);
    let mut budget = MAX_HISTORY_CHARS.saturating_sub(prompt_len);
    let mut keep = history.len();
    while keep >= 2 {
        let pair = size(&history[keep - 2]) + size(&history[keep - 1]);
        if pair > budget {
            break;
        }
        budget -= pair;
        keep -= 2;
    }
    keep
}

/// Whether the next task of conversation `id`, whose history is kept in `dir` (the session's
/// folder), would leave earlier messages out with a prompt of `prompt_len` bytes. Plenipo asks
/// before it sends the task: its start, and the full instructions with it, would be lost, so the
/// task goes out as after a shortened memory (ADR-044 §2.5).
pub fn would_leave_out(dir: &Path, id: &str, prompt_len: usize) -> bool {
    valid_id(id) && load(&conversation_file(dir, id)).is_some_and(|h| left_out(&h, prompt_len) > 0)
}

/// The messages to send: the newest history that fits with the prompt, then the prompt; and how
/// many earlier messages were left out.
fn fitting(history: &[Value], prompt: &str) -> (Vec<Value>, usize) {
    let left_out = left_out(history, prompt.len());
    let mut messages = Vec::new();
    if left_out > 0 {
        messages.push(json!({
            "role": "system",
            "content": "Earlier messages of this conversation were left out because it is long.",
        }));
    }
    messages.extend(history[left_out..].iter().cloned());
    messages.push(json!({ "role": "user", "content": prompt }));
    (messages, left_out)
}

/// A sentence for an HTTP error from the service.
fn error_text(status: u16, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status {
        401 => format!("401 unauthorized: sign in to Ollama (ollama signin). {message}"),
        // Seen on the owner's free plan for most cloud models (2026-09-27), with no body.
        402 => format!(
            "This model needs a paid Ollama plan (402 payment required). Choose a model your \
             plan includes, or change your plan at ollama.com. {message}"
        )
        .trim_end()
        .to_owned(),
        429 => format!("429 usage limit: {message}"),
        _ => format!("Ollama answered {status}: {message}"),
    }
}

// ---- A minimal HTTP/1.1 client for 127.0.0.1 --------------------------------------------------

struct Response {
    status: u16,
    reader: Box<dyn BufRead>,
}

impl Response {
    fn body_text(&mut self) -> String {
        let mut body = Vec::new();
        let _ = (&mut self.reader).take(1024 * 1024).read_to_end(&mut body);
        String::from_utf8_lossy(&body).into_owned()
    }
}

/// Send one request to the Ollama service on this PC. `timeout` limits each read (none while a
/// model thinks: the task's own time limit applies).
fn request(
    port: u16,
    method: &str,
    path: &str,
    body: &[u8],
    timeout: Option<Duration>,
) -> Result<Response, String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| {
        format!("Ollama is not running on this PC (connection refused: {e}). Start Ollama.")
    })?;
    stream
        .set_read_timeout(timeout)
        .map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUser-Agent: plenipo\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    writer
        .write_all(head.as_bytes())
        .and_then(|()| writer.write_all(body))
        .and_then(|()| writer.flush())
        .map_err(|e| format!("Sending to Ollama failed: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|e| format!("Reading Ollama's answer failed: {e}"))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| "Ollama sent an answer Plenipo could not read".to_owned())?;
    let mut chunked = false;
    let mut length: Option<u64> = None;
    loop {
        let mut header = String::new();
        let n = reader
            .read_line(&mut header)
            .map_err(|e| format!("Reading Ollama's answer failed: {e}"))?;
        let header = header.trim_end();
        if n == 0 || header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
            if name == "transfer-encoding" && value.to_ascii_lowercase().contains("chunked") {
                chunked = true;
            } else if name == "content-length" {
                length = value.parse().ok();
            }
        }
    }
    let reader: Box<dyn BufRead> = if chunked {
        Box::new(BufReader::new(Chunked {
            inner: reader,
            left: 0,
            done: false,
        }))
    } else if let Some(n) = length {
        Box::new(BufReader::new(reader.take(n)))
    } else {
        Box::new(reader)
    };
    Ok(Response { status, reader })
}

/// Reads an HTTP/1.1 chunked body.
struct Chunked<R> {
    inner: R,
    left: u64,
    done: bool,
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let mut size = String::new();
            if self.inner.read_line(&mut size)? == 0 {
                self.done = true;
                return Ok(0);
            }
            let size = size.trim();
            if size.is_empty() {
                // The line break after the previous chunk.
                size_line(&mut self.inner, &mut self.left, &mut self.done)?;
            } else {
                self.left = parse_size(size)?;
                if self.left == 0 {
                    self.done = true;
                }
            }
            if self.done {
                return Ok(0);
            }
        }
        let want = usize::try_from(self.left)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let n = self.inner.read(&mut buf[..want])?;
        if n == 0 {
            self.done = true;
        }
        self.left -= n as u64;
        Ok(n)
    }
}

fn size_line<R: BufRead>(inner: &mut R, left: &mut u64, done: &mut bool) -> std::io::Result<()> {
    let mut size = String::new();
    if inner.read_line(&mut size)? == 0 {
        *done = true;
        return Ok(());
    }
    *left = parse_size(size.trim())?;
    if *left == 0 {
        *done = true;
    }
    Ok(())
}

fn parse_size(line: &str) -> std::io::Result<u64> {
    let hex = line.split(';').next().unwrap_or("").trim();
    u64::from_str_radix(hex, 16).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad chunk size in Ollama's answer",
        )
    })
}

/// One line (without the newline) into `line`; 0 at the end. Lines longer than
/// [`MAX_LINE_BYTES`] are cut.
fn read_line_capped(reader: &mut dyn BufRead, line: &mut Vec<u8>) -> std::io::Result<usize> {
    let mut read = 0;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(read);
        }
        let (take, found) = match available.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (available.len(), false),
        };
        let room = MAX_LINE_BYTES.saturating_sub(line.len());
        line.extend_from_slice(&available[..take.min(room)]);
        reader.consume(take);
        read += take;
        if found {
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            return Ok(read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// A one-request stand-in for the Ollama service: answers `status` with `body` (chunked
    /// when `chunked`), and hands back the request it received.
    fn service(status: u16, body: &str, chunked: bool) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = body.to_owned();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut request_body = vec![0; length];
            reader.read_exact(&mut request_body).unwrap();
            tx.send(format!("{head}{}", String::from_utf8_lossy(&request_body)))
                .unwrap();
            let reason = if status == 200 { "OK" } else { "Error" };
            if chunked {
                let mut out = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\n\r\n"
                );
                for line in body.lines() {
                    let piece = format!("{line}\n");
                    out.push_str(&format!("{:x}\r\n{piece}\r\n", piece.len()));
                }
                out.push_str("0\r\n\r\n");
                stream.write_all(out.as_bytes()).unwrap();
            } else {
                let out = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(out.as_bytes()).unwrap();
            }
        });
        (port, rx)
    }

    fn lines(out: &[u8]) -> Vec<Value> {
        String::from_utf8_lossy(out)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn args(list: &[&str], port: u16) -> Vec<String> {
        let mut a: Vec<String> = list.iter().map(|s| (*s).to_owned()).collect();
        a.extend(["--port".into(), port.to_string()]);
        a
    }

    #[test]
    fn only_the_bridge_argument_selects_bridge_mode() {
        let run = |v: &[&str]| maybe_run_from_args(v.iter().map(|s| (*s).to_owned())).is_some();
        assert!(!run(&["plenipo"]));
        assert!(!run(&["plenipo", "--plenipo-tools=x"]));
    }

    #[test]
    fn the_sign_in_check_reports_the_plan_and_never_the_account() {
        let (port, rx) = service(
            200,
            r#"{"id":"1125","email":"owner@example.com","name":"owner","plan":"free"}"#,
            false,
        );
        let mut out = Vec::new();
        assert_eq!(run(&args(&["auth"], port), &mut &b""[..], &mut out), 0);
        assert_eq!(lines(&out), [json!({ "signedIn": true, "plan": "free" })]);
        assert!(!String::from_utf8_lossy(&out).contains("example.com"));
        assert!(rx.recv().unwrap().starts_with("POST /api/me HTTP/1.1\r\n"));

        let (port, _rx) = service(
            401,
            r#"{"error":"unauthorized","signin_url":"https://x"}"#,
            false,
        );
        let mut out = Vec::new();
        assert_eq!(run(&args(&["auth"], port), &mut &b""[..], &mut out), 0);
        assert_eq!(lines(&out), [json!({ "signedIn": false })]);
    }

    #[test]
    fn a_service_that_is_not_running_is_explained() {
        let port = {
            let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            l.local_addr().unwrap().port()
        };
        let mut out = Vec::new();
        assert_eq!(run(&args(&["auth"], port), &mut &b""[..], &mut out), 1);
        let v = &lines(&out)[0];
        assert!(
            v["error"]
                .as_str()
                .unwrap()
                .contains("Ollama is not running"),
            "{v}"
        );
    }

    const STREAM: &str = concat!(
        r#"{"model":"gpt-oss:120b","message":{"role":"assistant","content":"","thinking":"Say "},"done":false}"#,
        "\n",
        r#"{"model":"gpt-oss:120b","message":{"role":"assistant","content":"","thinking":"hello."},"done":false}"#,
        "\n",
        r#"{"model":"gpt-oss:120b","message":{"role":"assistant","content":"hello "},"done":false}"#,
        "\n",
        r#"{"model":"gpt-oss:120b","message":{"role":"assistant","content":"there"},"done":false}"#,
        "\n",
        r#"{"model":"gpt-oss:120b","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","prompt_eval_count":75,"prompt_eval_cached_count":48,"eval_count":55}"#,
        "\n",
    );

    #[test]
    fn a_task_streams_the_answer_and_the_conversation_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_str().unwrap();

        let (port, rx) = service(200, STREAM, true);
        let mut out = Vec::new();
        let chat = [
            "chat",
            "--model",
            "gpt-oss:120b-cloud",
            "--session",
            "abc-1",
            "--think",
            "low",
            "--dir",
            d,
        ];
        assert_eq!(run(&args(&chat, port), &mut &b"Say hello"[..], &mut out), 0);
        let got = lines(&out);
        assert_eq!(
            got[0],
            json!({ "type": "session", "id": "abc-1", "model": "gpt-oss:120b-cloud" })
        );
        assert_eq!(got[1], json!({ "type": "thinking", "text": "Say hello." }));
        assert_eq!(got[2], json!({ "type": "text", "text": "hello " }));
        assert_eq!(got[3], json!({ "type": "text", "text": "there" }));
        assert_eq!(got[4], json!({ "type": "answer", "text": "hello there" }));
        assert_eq!(got[5]["type"], "done");
        assert_eq!(
            (
                got[5]["inputTokens"].as_u64(),
                got[5]["outputTokens"].as_u64()
            ),
            (Some(75), Some(55))
        );
        let sent = rx.recv().unwrap();
        assert!(sent.starts_with("POST /api/chat HTTP/1.1\r\n"), "{sent}");
        let body: Value = serde_json::from_str(sent.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["think"], "low");
        assert_eq!(
            body["messages"],
            json!([{ "role": "user", "content": "Say hello" }])
        );

        // The next task sends the conversation so far.
        let (port, rx) = service(200, STREAM, true);
        let mut out = Vec::new();
        let resume = [
            "chat",
            "--model",
            "gpt-oss:120b-cloud",
            "--session",
            "abc-1",
            "--resume",
            "--dir",
            d,
        ];
        assert_eq!(
            run(&args(&resume, port), &mut &b"And again?"[..], &mut out),
            0
        );
        let sent = rx.recv().unwrap();
        let body: Value = serde_json::from_str(sent.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body["messages"],
            json!([
                { "role": "user", "content": "Say hello" },
                { "role": "assistant", "content": "hello there" },
                { "role": "user", "content": "And again?" }
            ])
        );
        assert!(body.get("think").is_none());
    }

    #[test]
    fn errors_and_a_missing_conversation_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_str().unwrap();
        let chat = ["chat", "--model", "m", "--session", "s1", "--dir", d];

        let (port, _rx) = service(
            429,
            r#"{"error":"you have reached your hourly usage limit"}"#,
            false,
        );
        let mut out = Vec::new();
        assert_eq!(run(&args(&chat, port), &mut &b"hi"[..], &mut out), 1);
        let last = lines(&out).pop().unwrap();
        assert_eq!(last["type"], "error");
        assert!(
            last["message"]
                .as_str()
                .unwrap()
                .starts_with("429 usage limit"),
            "{last}"
        );

        let (port, _rx) = service(402, "", false);
        let mut out = Vec::new();
        assert_eq!(run(&args(&chat, port), &mut &b"hi"[..], &mut out), 1);
        assert!(lines(&out).pop().unwrap()["message"]
            .as_str()
            .unwrap()
            .starts_with("This model needs a paid Ollama plan"));

        let (port, _rx) = service(401, r#"{"error":"unauthorized"}"#, false);
        let mut out = Vec::new();
        assert_eq!(run(&args(&chat, port), &mut &b"hi"[..], &mut out), 1);
        assert!(lines(&out).pop().unwrap()["message"]
            .as_str()
            .unwrap()
            .starts_with("401 unauthorized"));

        let resume = [
            "chat",
            "--model",
            "m",
            "--session",
            "gone",
            "--resume",
            "--dir",
            d,
            "--port",
            "1",
        ];
        let mut out = Vec::new();
        assert_eq!(run(&resume.map(String::from), &mut &b"hi"[..], &mut out), 1);
        assert!(lines(&out)[0]["message"]
            .as_str()
            .unwrap()
            .contains("history was not found"));

        let bad = ["chat", "--model", "m", "--session", "../x", "--port", "1"];
        let mut out = Vec::new();
        assert_eq!(run(&bad.map(String::from), &mut &b"hi"[..], &mut out), 1);
    }

    #[test]
    fn long_conversations_leave_out_the_oldest_exchanges() {
        let big = "x".repeat(MAX_HISTORY_CHARS / 3);
        let history: Vec<Value> = (0..4)
            .flat_map(|i| {
                [
                    json!({ "role": "user", "content": format!("{i}{big}") }),
                    json!({ "role": "assistant", "content": "ok" }),
                ]
            })
            .collect();
        let (sent, left_out) = fitting(&history, "now");
        assert_eq!(left_out, 4);
        assert_eq!(sent[0]["role"], "system");
        assert!(sent[1]["content"].as_str().unwrap().starts_with('2'));
        assert_eq!(sent.last().unwrap()["content"], "now");
        assert_eq!(fitting(&history[..2], "now").1, 0);
        // Plenipo can ask before it sends a task (ADR-044 §2.5).
        let saved = tempfile::tempdir().unwrap();
        save(&conversation_file(saved.path(), "long-0"), "m", &history).unwrap();
        assert!(would_leave_out(saved.path(), "long-0", 3));
        save(
            &conversation_file(saved.path(), "short-0"),
            "m",
            &history[..2],
        )
        .unwrap();
        assert!(!would_leave_out(saved.path(), "short-0", 3));
        assert!(would_leave_out(saved.path(), "short-0", MAX_HISTORY_CHARS));
        assert!(!would_leave_out(saved.path(), "missing", 3));
        assert!(!would_leave_out(saved.path(), "../long-0", 3));

        // A task says so, in plain words and with the count Plenipo reads (ADR-044 §2.5).
        let dir = tempfile::tempdir().unwrap();
        save(&conversation_file(dir.path(), "long-1"), "m", &history).unwrap();
        let (port, _rx) = service(200, STREAM, true);
        let resume = [
            "chat",
            "--model",
            "m",
            "--session",
            "long-1",
            "--resume",
            "--dir",
            dir.path().to_str().unwrap(),
        ];
        let mut out = Vec::new();
        assert_eq!(run(&args(&resume, port), &mut &b"now"[..], &mut out), 0);
        let notice = lines(&out)
            .into_iter()
            .find(|l| l["type"] == "notice")
            .unwrap();
        assert_eq!(notice["leftOut"], 4);
        assert!(notice["text"]
            .as_str()
            .unwrap()
            .starts_with("4 earlier message(s) were left out"));
    }
}
