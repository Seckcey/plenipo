//! Plenipo's helper for paid AI services (Phase 16 Wave 3, ADR-086 and ADR-087): the program a
//! paid task runs, like the Ollama helper (ADR-017).
//!
//! Plenipo runs itself in this mode (`plenipo-desktop --plenipo-paid <service> …`) as the
//! task's supervised process, so a task keeps its time limit, cancel, and records. The helper
//! reaches only its service's fixed addresses, each checked by Guard's rules for Plenipo's own
//! requests before it connects; redirects are never followed. It writes one JSON object per line
//! on stdout, which the paid adapter's parser reads.
//!
//! **The key** comes on the first line of standard input (`{"key":"…"}`), from the Vault, and
//! nowhere else: never an argument, never an environment variable, never written to a file or
//! printed. What follows it on standard input is the task's words.
//!
//! - `check`: the key works, and the service's models with their prices →
//!   `{"signedIn":true,"models":[…]}`, `{"signedIn":false,"reason":"…"}`, or `{"error":"…"}`.
//! - `chat --model M --session ID [--resume] --max-input-bytes N --max-output-tokens N
//!   [--effort LEVEL]`: one request, streamed → `session`, `thinking`, `text`, `answer`, `done`
//!   (token counts and the service's own bill when it sends one), `notice`, or `error` lines.
//!
//! The helper keeps each conversation in the session's own folder
//! (`.plenipo-paid-<service>-<ID>.json`) and sends at most `--max-input-bytes` of it, so the
//! most a task can cost is known before it starts (the Ledger sets that aside, ADR-085). A task
//! that does not finish leaves the conversation unchanged.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use plenipo_guard::outbound::{OutboundRules, Purpose};
use plenipo_guard::PaidService;
use plenipo_runtime::agent::paid::{
    conversation_file, fitting, load_conversation, save_conversation, valid_conversation_id,
    HELPER_ARG, MAX_INPUT_BYTES, MAX_OUTPUT_TOKENS,
};
use serde_json::{json, Value};

/// The argument that selects this mode (first after the program name).
pub const ARG: &str = HELPER_ARG;
/// Longest first line (the key) read from stdin.
const MAX_KEY_LINE: usize = 4 * 1024;
/// Longest task text read from stdin.
const MAX_PROMPT_BYTES: usize = 4 * 1024 * 1024;
/// Longest answer read from a check.
const MAX_CHECK_BYTES: usize = 8 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// At most this many models are listed from a check.
const MAX_MODELS: usize = 1_000;

/// Run this mode when the arguments ask for it; otherwise `None` (start Plenipo normally).
pub fn maybe_run_from_args(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    if args.next().as_deref() != Some(ARG) {
        return None;
    }
    let rest: Vec<String> = args.collect();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let rules = OutboundRules::default();
    Some(run(&rest, &rules, &mut std::io::stdin().lock(), &mut out))
}

/// One command: its exit code. Output goes to `out`, one JSON object per line.
pub fn run(
    args: &[String],
    rules: &OutboundRules,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> i32 {
    let Some(service) = args.first().and_then(|s| PaidService::from_id(s)) else {
        emit(
            out,
            &json!({ "type": "error", "message": "Unknown paid AI service" }),
        );
        return 2;
    };
    let base = flag(args, "--base").unwrap_or_else(|| service.base_url().to_owned());
    let key = match read_key(input) {
        Ok(key) => key,
        Err(message) => {
            emit(out, &json!({ "type": "error", "message": message }));
            return 2;
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            emit(
                out,
                &json!({ "type": "error", "message": format!("The helper could not start: {e}") }),
            );
            return 1;
        }
    };
    let client = Client {
        service,
        base,
        key,
        rules: rules.clone(),
    };
    match args.get(1).map(String::as_str) {
        Some("check") => runtime.block_on(check(&client, out)),
        Some("chat") => runtime.block_on(chat(&client, args, input, out)),
        _ => {
            emit(
                out,
                &json!({ "type": "error", "message": "Unknown paid helper command" }),
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

/// The key: the first line of stdin, `{"key":"…"}`. Read byte by byte, so nothing after it is
/// taken from the task's words.
fn read_key(input: &mut dyn Read) -> Result<String, &'static str> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => {
                line.push(byte[0]);
                if line.len() > MAX_KEY_LINE {
                    return Err("The key was too long");
                }
            }
            Err(_) => return Err("The key could not be read"),
        }
    }
    let key = serde_json::from_slice::<Value>(&line)
        .ok()
        .and_then(|v| v.get("key").and_then(Value::as_str).map(str::to_owned))
        .filter(|k| !k.trim().is_empty() && !k.chars().any(char::is_control))
        .ok_or("No key was given")?;
    Ok(key.trim().to_owned())
}

// ---- Talking to the service ------------------------------------------------------------------

struct Client {
    service: PaidService,
    base: String,
    key: String,
    rules: OutboundRules,
}

impl Client {
    /// `path` on the service's base address, checked by Guard's rules first.
    fn address(&self, path: &str) -> Result<String, String> {
        let address = format!("{}{path}", self.base.trim_end_matches('/'));
        self.rules
            .check(Purpose::PaidAi(self.service), &address)
            .map(|site| site.url)
    }

    fn http(&self, timeout: Option<Duration>) -> Result<reqwest::Client, String> {
        let mut b = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")));
        if let Some(t) = timeout {
            b = b.timeout(t);
        }
        b.build()
            .map_err(|e| format!("The helper could not set up a connection: {e}"))
    }

    /// The service's own sign-in headers for the key.
    fn signed(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.service.protocol() {
            plenipo_guard::PaidProtocol::Anthropic => request
                .header("x-api-key", &self.key)
                .header("anthropic-version", "2023-06-01"),
            plenipo_guard::PaidProtocol::OpenAiChat => request.bearer_auth(&self.key),
        }
    }
}

/// A sentence for an error from the service, without anything the service echoed back that
/// could hold the key.
fn error_text(service: PaidService, status: u16, body: &str, key: &str) -> (String, &'static str) {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("message"))
                .or_else(|| v.get("error"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| body.chars().take(300).collect());
    let message = hide(&message, key);
    let label = service.label();
    match status {
        401 | 403 => (
            format!("{label} refused the key ({status}): it needs a new key. {message}"),
            "key",
        ),
        402 => (
            format!("{label} says the account is out of credit (402). {message}"),
            "credit",
        ),
        429 => (format!("{label} usage limit (429): {message}"), "limit"),
        _ => (format!("{label} answered {status}: {message}"), "service"),
    }
}

/// `text` with the key, and anything that starts like it, hidden.
fn hide(text: &str, key: &str) -> String {
    let mut text = text.to_owned();
    if key.len() >= 6 {
        text = text.replace(key, "[hidden by Plenipo]");
        // A service may echo part of it ("sk-or-v1-1234…").
        let head: String = key.chars().take(12).collect();
        text = text.replace(&head, "[hidden by Plenipo]");
    }
    text.chars().take(500).collect()
}

// ---- The check --------------------------------------------------------------------------------

async fn check(client: &Client, out: &mut dyn Write) -> i32 {
    let fail = |out: &mut dyn Write, e: String| {
        emit(out, &json!({ "error": e }));
        1
    };
    let http = match client.http(Some(CHECK_TIMEOUT)) {
        Ok(h) => h,
        Err(e) => return fail(out, e),
    };
    let key_path = client.service.key_check_path();
    let address = match client.address(key_path) {
        Ok(a) => a,
        Err(e) => return fail(out, e),
    };
    let answer = client.signed(http.get(&address)).send().await;
    let response = match answer {
        Ok(r) => r,
        Err(e) => {
            return fail(
                out,
                format!("{} could not be reached: {e}", client.service.label()),
            )
        }
    };
    let status = response.status().as_u16();
    let body = read_capped(response, MAX_CHECK_BYTES)
        .await
        .unwrap_or_default();
    if status == 401 || status == 403 {
        let (reason, _) = error_text(client.service, status, &body, &client.key);
        emit(out, &json!({ "signedIn": false, "reason": reason }));
        return 0;
    }
    if status != 200 {
        let (message, _) = error_text(client.service, status, &body, &client.key);
        return fail(out, message);
    }
    // The key's own limits, where the service reports them (OpenRouter's `/key`).
    let limit = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("data").cloned())
        .map(|d| {
            json!({
                "limit": d.get("limit").cloned().unwrap_or(Value::Null),
                "usage": d.get("usage").cloned().unwrap_or(Value::Null),
                "freeTier": d.get("is_free_tier").cloned().unwrap_or(Value::Null),
            })
        });
    // The models with their prices (OpenRouter lists them without a key).
    let models = match client.service.models_path() {
        Some(path) if path == key_path => models_from(client.service, &body),
        Some(path) => {
            let listed = match client.address(path) {
                Ok(a) => client.signed(http.get(&a)).send().await,
                Err(e) => return fail(out, e),
            };
            match listed {
                Ok(r) if r.status().as_u16() == 200 => {
                    let body = read_capped(r, MAX_CHECK_BYTES).await.unwrap_or_default();
                    models_from(client.service, &body)
                }
                Ok(r) => {
                    let status = r.status().as_u16();
                    let body = read_capped(r, 64 * 1024).await.unwrap_or_default();
                    let (message, _) = error_text(client.service, status, &body, &client.key);
                    return fail(out, message);
                }
                Err(e) => {
                    return fail(
                        out,
                        format!(
                            "{}'s model list could not be read: {e}",
                            client.service.label()
                        ),
                    )
                }
            }
        }
        None => Vec::new(),
    };
    emit(
        out,
        &json!({ "signedIn": true, "limit": limit, "models": models }),
    );
    0
}

/// The models a list names, with their prices per million tokens in micros when it gives them
/// (OpenRouter's `pricing` per token, as exact decimal text).
fn models_from(service: PaidService, body: &str) -> Vec<Value> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(list) = v.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    let per_million = |p: &Value, k: &str| -> Option<u64> {
        let text = match p.get(k)? {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        };
        plenipo_runtime::pricing::per_token_to_per_million(&text)
    };
    list.iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?;
            if id.is_empty() || id.len() > 200 || id.chars().any(char::is_control) {
                return None;
            }
            let name = m
                .get("name")
                .or_else(|| m.get("display_name"))
                .and_then(Value::as_str)
                .map(|n| n.chars().take(120).collect::<String>());
            let price = m.get("pricing").and_then(|p| {
                Some(json!({
                    "input": per_million(p, "prompt")?,
                    "cachedInput": per_million(p, "input_cache_read"),
                    "output": per_million(p, "completion")?,
                }))
            });
            let _ = service;
            Some(json!({
                "id": id,
                "name": name,
                "contextTokens": m.get("context_length").and_then(Value::as_u64),
                "price": price,
            }))
        })
        .take(MAX_MODELS)
        .collect()
}

async fn read_capped(mut response: reqwest::Response, max: usize) -> Result<String, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() > max {
            return Err("the answer was too long".into());
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

// ---- A task -----------------------------------------------------------------------------------

async fn chat(client: &Client, args: &[String], input: &mut dyn Read, out: &mut dyn Write) -> i32 {
    let fail = |out: &mut dyn Write, message: &str, kind: &str| {
        emit(
            out,
            &json!({ "type": "error", "message": message, "kind": kind }),
        );
        1
    };
    let Some(model) = flag(args, "--model").filter(|m| !m.is_empty() && m.len() <= 200) else {
        return fail(out, "No model was given", "input");
    };
    let Some(id) = flag(args, "--session").filter(|id| valid_conversation_id(id)) else {
        return fail(out, "No valid conversation ID was given", "input");
    };
    let resume = args.iter().any(|a| a == "--resume");
    let max_input = flag(args, "--max-input-bytes")
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(MAX_INPUT_BYTES)
        .min(MAX_INPUT_BYTES);
    let max_output = flag(args, "--max-output-tokens")
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(MAX_OUTPUT_TOKENS)
        .clamp(1, MAX_OUTPUT_TOKENS);
    let effort = flag(args, "--effort");

    let mut prompt = String::new();
    if input
        .take(MAX_PROMPT_BYTES as u64)
        .read_to_string(&mut prompt)
        .is_err()
        || prompt.trim().is_empty()
    {
        return fail(out, "No task was given", "input");
    }
    let dir = flag(args, "--dir").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let file = conversation_file(&dir, client.service.id(), &id);
    let mut history = if resume {
        match load_conversation(&file) {
            Some(h) => h,
            None => {
                return fail(
                    out,
                    "This conversation's history was not found; start a new conversation",
                    "input",
                )
            }
        }
    } else {
        Vec::new()
    };
    emit(out, &json!({ "type": "session", "id": id, "model": model }));
    let Some((sent, left_out)) = fitting(&history, &prompt, max_input) else {
        return fail(
            out,
            "This task's words are longer than Plenipo sends to a paid AI service at once",
            "input",
        );
    };
    if left_out > 0 {
        emit(
            out,
            &json!({ "type": "notice", "leftOut": left_out, "text": format!(
                "{left_out} earlier message(s) were left out: the conversation is longer than Plenipo sends at once."
            ) }),
        );
    }
    let (path, body) = request_body(client.service, &model, &sent, max_output, effort.as_deref());
    let address = match client.address(path) {
        Ok(a) => a,
        Err(e) => return fail(out, &e, "guard"),
    };
    let http = match client.http(None) {
        Ok(h) => h,
        Err(e) => return fail(out, &e, "service"),
    };
    let started = Instant::now();
    let response = match client
        .signed(http.post(&address))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(r) => r,
        // Never connected: nothing was sent, so nothing was billed.
        Err(e) if e.is_connect() => {
            return fail(
                out,
                &format!("{} could not be reached: {e}", client.service.label()),
                "unreached",
            )
        }
        // It may have reached the service: counted at the most it could have cost.
        Err(e) => {
            return fail(
                out,
                &format!("{} did not answer: {e}", client.service.label()),
                "service",
            )
        }
    };
    let status = response.status().as_u16();
    if status != 200 {
        let text = read_capped(response, 64 * 1024).await.unwrap_or_default();
        let (message, kind) = error_text(client.service, status, &text, &client.key);
        return fail(out, &message, kind);
    }
    let mut stream = Stream::new(client.service);
    let mut response = response;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                for event in stream.feed(&chunk) {
                    match event {
                        Event::Emit(v) => emit(out, &v),
                        Event::Failed(message) => {
                            let message = hide(&message, &client.key);
                            return fail(out, &message, "service");
                        }
                    }
                }
                if stream.finished {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                return fail(
                    out,
                    &format!("Reading {}'s answer failed: {e}", client.service.label()),
                    "service",
                )
            }
        }
    }
    for event in stream.end() {
        if let Event::Emit(v) = event {
            emit(out, &v);
        }
    }
    let Some(usage) = stream.usage.take() else {
        return fail(
            out,
            &format!(
                "{}'s answer ended before it was complete",
                client.service.label()
            ),
            "service",
        );
    };
    emit(out, &json!({ "type": "answer", "text": stream.answer }));
    let mut done = usage;
    done["type"] = json!("done");
    done["reason"] = json!(stream.stop.as_deref().unwrap_or("stop"));
    done["durationMs"] = json!(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    emit(out, &done);
    history.push(json!({ "role": "user", "content": prompt }));
    history.push(json!({ "role": "assistant", "content": stream.answer }));
    if let Err(e) = save_conversation(&file, &model, &history) {
        emit(
            out,
            &json!({ "type": "notice", "text": format!("The conversation could not be saved for the next task: {e}") }),
        );
    }
    0
}

/// The request for one task: its path and body, in the service's own way of talking.
fn request_body(
    service: PaidService,
    model: &str,
    messages: &[Value],
    max_output: u64,
    effort: Option<&str>,
) -> (&'static str, Value) {
    match service.protocol() {
        plenipo_guard::PaidProtocol::OpenAiChat => {
            let mut body = json!({
                "model": model,
                "messages": messages,
                "stream": true,
                "max_tokens": max_output,
                "stream_options": { "include_usage": true },
            });
            if service == PaidService::OpenRouter {
                // OpenRouter's own bill for the request, in its final line.
                body["usage"] = json!({ "include": true });
                if let Some(level) = effort {
                    body["reasoning"] = json!({ "effort": level });
                }
            } else if let Some(level) = effort {
                body["reasoning_effort"] = json!(level);
            }
            (service.chat_path(), body)
        }
        plenipo_guard::PaidProtocol::Anthropic => {
            let system: Vec<&str> = messages
                .iter()
                .filter(|m| m.get("role").and_then(Value::as_str) == Some("system"))
                .filter_map(|m| m.get("content").and_then(Value::as_str))
                .collect();
            let turns: Vec<Value> = messages
                .iter()
                .filter(|m| m.get("role").and_then(Value::as_str) != Some("system"))
                .cloned()
                .collect();
            let mut body = json!({
                "model": model,
                "messages": turns,
                "stream": true,
                "max_tokens": max_output,
            });
            if !system.is_empty() {
                body["system"] = json!(system.join("\n\n"));
            }
            (service.chat_path(), body)
        }
    }
}

// ---- Reading the streamed answer (server-sent events) --------------------------------------------

enum Event {
    Emit(Value),
    Failed(String),
}

/// A streamed answer, read line by line: `data: {…}` events, comments, and `[DONE]`.
struct Stream {
    service: PaidService,
    pending: Vec<u8>,
    thinking: String,
    answer: String,
    /// The final counts (and the service's bill), once they arrive.
    usage: Option<Value>,
    stop: Option<String>,
    finished: bool,
    /// Anthropic's counts arrive in two parts.
    input_tokens: u64,
    cached_tokens: u64,
}

impl Stream {
    fn new(service: PaidService) -> Self {
        Self {
            service,
            pending: Vec::new(),
            thinking: String::new(),
            answer: String::new(),
            usage: None,
            stop: None,
            finished: false,
            input_tokens: 0,
            cached_tokens: 0,
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data == "[DONE]" {
                    self.finished = true;
                    break;
                }
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    events.extend(self.event(&v));
                }
            }
        }
        if self.pending.len() > 8 * 1024 * 1024 {
            events.push(Event::Failed("A line of the answer was too long".into()));
            self.pending.clear();
        }
        events
    }

    /// Thinking said so far, before the answer goes on.
    fn flush_thinking(&mut self, events: &mut Vec<Event>) {
        if !self.thinking.is_empty() {
            events.push(Event::Emit(
                json!({ "type": "thinking", "text": std::mem::take(&mut self.thinking) }),
            ));
        }
    }

    fn event(&mut self, v: &Value) -> Vec<Event> {
        let mut events = Vec::new();
        if let Some(e) = v.get("error") {
            let message = e
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| e.to_string(), str::to_owned);
            events.push(Event::Failed(format!(
                "{} reported an error: {message}",
                self.service.label()
            )));
            return events;
        }
        match self.service.protocol() {
            plenipo_guard::PaidProtocol::OpenAiChat => {
                if let Some(choice) = v.pointer("/choices/0") {
                    let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
                    for key in ["reasoning", "reasoning_content"] {
                        if let Some(t) = delta.get(key).and_then(Value::as_str) {
                            self.thinking.push_str(t);
                        }
                    }
                    if let Some(t) = delta.get("content").and_then(Value::as_str) {
                        if !t.is_empty() {
                            self.flush_thinking(&mut events);
                            self.answer.push_str(t);
                            events.push(Event::Emit(json!({ "type": "text", "text": t })));
                        }
                    }
                    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                        self.stop = Some(reason.to_owned());
                    }
                }
                if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
                    let n = |p: &str| u.pointer(p).and_then(Value::as_u64).unwrap_or(0);
                    let cost = u.get("cost").map(|c| match c {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    });
                    self.usage = Some(json!({
                        "inputTokens": n("/prompt_tokens"),
                        "cachedTokens": n("/prompt_tokens_details/cached_tokens"),
                        "outputTokens": n("/completion_tokens"),
                        "costDollars": cost,
                    }));
                }
            }
            plenipo_guard::PaidProtocol::Anthropic => match v.get("type").and_then(Value::as_str) {
                Some("message_start") => {
                    let n = |p: &str| v.pointer(p).and_then(Value::as_u64).unwrap_or(0);
                    self.cached_tokens = n("/message/usage/cache_read_input_tokens");
                    self.input_tokens = n("/message/usage/input_tokens")
                        + self.cached_tokens
                        + n("/message/usage/cache_creation_input_tokens");
                }
                Some("content_block_delta") => {
                    let delta = v.get("delta").cloned().unwrap_or(Value::Null);
                    if let Some(t) = delta.get("thinking").and_then(Value::as_str) {
                        self.thinking.push_str(t);
                    }
                    if let Some(t) = delta.get("text").and_then(Value::as_str) {
                        if !t.is_empty() {
                            self.flush_thinking(&mut events);
                            self.answer.push_str(t);
                            events.push(Event::Emit(json!({ "type": "text", "text": t })));
                        }
                    }
                }
                Some("message_delta") => {
                    if let Some(reason) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                        self.stop = Some(reason.to_owned());
                    }
                    let out = v
                        .pointer("/usage/output_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    self.usage = Some(json!({
                        "inputTokens": self.input_tokens,
                        "cachedTokens": self.cached_tokens,
                        "outputTokens": out,
                        "costDollars": Value::Null,
                    }));
                }
                Some("message_stop") => self.finished = true,
                _ => {}
            },
        }
        events
    }

    fn end(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        self.flush_thinking(&mut events);
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-or-v1-0123456789abcdef0123456789abcdef";

    fn key_line() -> String {
        format!("{}\n", json!({ "key": KEY }))
    }

    fn lines(out: &[u8]) -> Vec<Value> {
        String::from_utf8_lossy(out)
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    #[test]
    fn the_key_is_the_first_line_and_nothing_else_is_taken() {
        let mut input = std::io::Cursor::new(format!("{}Summarize this.", key_line()).into_bytes());
        assert_eq!(read_key(&mut input).unwrap(), KEY);
        let mut rest = String::new();
        input.read_to_string(&mut rest).unwrap();
        assert_eq!(rest, "Summarize this.");
        for bad in [
            "",
            "\n",
            "{}\n",
            "{\"key\":\"\"}\n",
            "not json\n",
            "{\"key\":\"a\\u0000b\"}\n",
        ] {
            let mut input = std::io::Cursor::new(bad.as_bytes().to_vec());
            assert!(read_key(&mut input).is_err(), "{bad:?}");
        }
        let long = format!("{}\n", json!({ "key": "k".repeat(MAX_KEY_LINE + 1) }));
        assert!(read_key(&mut std::io::Cursor::new(long.into_bytes())).is_err());
    }

    #[test]
    fn an_unknown_service_or_command_is_refused_and_no_key_is_printed() {
        let rules = OutboundRules::default();
        let mut out = Vec::new();
        assert_eq!(
            run(&["nope".into()], &rules, &mut std::io::empty(), &mut out),
            2
        );
        let mut out = Vec::new();
        let mut input = std::io::Cursor::new(key_line().into_bytes());
        assert_eq!(
            run(
                &["openrouter".into(), "fly".into()],
                &rules,
                &mut input,
                &mut out
            ),
            2
        );
        assert!(!String::from_utf8_lossy(&out).contains(KEY));
    }

    #[test]
    fn only_the_services_own_https_address_is_reached() {
        let rules = OutboundRules::default();
        let mut out = Vec::new();
        let mut input = std::io::Cursor::new(key_line().into_bytes());
        // A base address that is not OpenRouter's is refused by Guard's rules before anything
        // connects.
        let code = run(
            &[
                "openrouter".into(),
                "check".into(),
                "--base".into(),
                "https://example.com/api/v1".into(),
            ],
            &rules,
            &mut input,
            &mut out,
        );
        assert_eq!(code, 1);
        let said = lines(&out);
        assert!(
            said[0]["error"]
                .as_str()
                .unwrap()
                .starts_with("Plenipo refused to reach"),
            "{said:?}"
        );
        assert!(!String::from_utf8_lossy(&out).contains(KEY));
    }

    #[test]
    fn errors_never_carry_the_key() {
        let body = json!({ "error": { "message": format!("Invalid key {KEY}") } }).to_string();
        let (text, kind) = error_text(PaidService::OpenRouter, 401, &body, KEY);
        assert_eq!(kind, "key");
        assert!(text.contains("needs a new key"), "{text}");
        assert!(
            !text.contains(KEY) && !text.contains("0123456789abcdef"),
            "{text}"
        );
        let partly =
            json!({ "error": { "message": "key sk-or-v1-0123… is disabled" } }).to_string();
        let (text, _) = error_text(PaidService::OpenRouter, 403, &partly, KEY);
        assert!(!text.contains("sk-or-v1-012"), "{text}");
        assert_eq!(
            error_text(PaidService::OpenRouter, 402, "{}", KEY).1,
            "credit"
        );
        assert_eq!(
            error_text(PaidService::OpenRouter, 429, "{}", KEY).1,
            "limit"
        );
    }

    #[test]
    fn a_model_list_gives_prices_per_million_tokens() {
        let body = json!({ "data": [
            { "id": "moonshotai/kimi-k3", "name": "MoonshotAI: Kimi K3", "context_length": 1_048_576,
              "pricing": { "prompt": "0.000003", "completion": "0.000015", "input_cache_read": "0.0000003" } },
            { "id": "openrouter/auto", "pricing": { "prompt": "-1", "completion": "-1" } },
            { "id": "", "pricing": {} },
        ] })
        .to_string();
        let models = models_from(PaidService::OpenRouter, &body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "moonshotai/kimi-k3");
        assert_eq!(models[0]["price"]["input"], 3_000_000);
        assert_eq!(models[0]["price"]["cachedInput"], 300_000);
        assert_eq!(models[0]["price"]["output"], 15_000_000);
        assert_eq!(models[0]["contextTokens"], 1_048_576);
        // A price known only after the request is no price.
        assert!(models[1]["price"].is_null());
    }

    #[test]
    fn a_streamed_answer_gives_text_thinking_counts_and_the_bill() {
        let mut s = Stream::new(PaidService::OpenRouter);
        let mut events = s.feed(b": OPENROUTER PROCESSING\n\ndata: {\"choices\":[{\"delta\":{\"reasoning\":\"Hm\"}}]}\n");
        events.extend(s.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"O\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"K\"},\"finish_reason\":\"stop\"}]}\n"));
        events.extend(s.feed(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":4},\"cost\":0.0000570}}\n\ndata: [DONE]\n"));
        let emitted: Vec<Value> = events
            .into_iter()
            .filter_map(|e| match e {
                Event::Emit(v) => Some(v),
                Event::Failed(_) => None,
            })
            .collect();
        assert_eq!(emitted[0], json!({ "type": "thinking", "text": "Hm" }));
        assert_eq!(emitted[1], json!({ "type": "text", "text": "O" }));
        assert_eq!(s.answer, "OK");
        assert!(s.finished);
        assert_eq!(s.stop.as_deref(), Some("stop"));
        let usage = s.usage.unwrap();
        assert_eq!(usage["inputTokens"], 12);
        assert_eq!(usage["cachedTokens"], 4);
        assert_eq!(usage["outputTokens"], 2);
        assert_eq!(usage["costDollars"], "0.000057");
    }

    #[test]
    fn an_error_in_the_stream_fails_the_task() {
        let mut s = Stream::new(PaidService::OpenRouter);
        let events = s.feed(b"data: {\"error\":{\"message\":\"Provider down\"}}\n");
        assert!(matches!(&events[0], Event::Failed(m) if m.contains("Provider down")));
    }
}
