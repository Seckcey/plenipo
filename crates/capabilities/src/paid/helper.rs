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
/// The longest a request may go without a byte from the service.
const READ_TIMEOUT: Duration = Duration::from_secs(300);
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
            &json!({ "type": "error", "message": "Unknown paid AI service", "kind": "input" }),
        );
        return 2;
    };
    let base = flag(args, "--base").unwrap_or_else(|| service.base_url().to_owned());
    let key = match read_key(input) {
        Ok(key) => key,
        Err(message) => {
            emit(
                out,
                &json!({ "type": "error", "message": message, "kind": "input" }),
            );
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
                &json!({ "type": "error", "message": format!("The helper could not start: {e}"), "kind": "input" }),
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
                &json!({ "type": "error", "message": "Unknown paid helper command", "kind": "input" }),
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
            // A stalled answer ends instead of waiting for the task's time limit.
            .read_timeout(READ_TIMEOUT)
            .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")));
        if let Some(t) = timeout {
            b = b.timeout(t);
        }
        b.build()
            .map_err(|e| format!("The helper could not set up a connection: {e}"))
    }

    /// A task's request with the key: as `signed`, except Google's OpenAI-style chat, which
    /// takes the key as `Authorization: Bearer` (its own list takes `x-goog-api-key`).
    fn signed_chat(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.service.auth() {
            plenipo_guard::paid::PaidAuth::GoogleKey => request.bearer_auth(&self.key),
            _ => self.signed(request),
        }
    }

    /// The service's own sign-in headers for the key.
    fn signed(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.service.auth() {
            plenipo_guard::paid::PaidAuth::AnthropicKey => request
                .header("x-api-key", &self.key)
                .header("anthropic-version", "2023-06-01"),
            plenipo_guard::paid::PaidAuth::GoogleKey => request.header("x-goog-api-key", &self.key),
            plenipo_guard::paid::PaidAuth::Bearer => request.bearer_auth(&self.key),
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
        .unwrap_or_else(|| body.to_owned());
    // Hidden before it is cut short, so no part of an echoed key is left past the cut.
    let message: String = hide(&message, key).chars().take(300).collect();
    let label = service.label();
    // Google answers a key it does not know, or one that has expired, with 400.
    if status == 400 && google_bad_key(body) {
        return (
            format!("{label} refused the key ({status}): it needs a new key. {message}"),
            "key",
        );
    }
    // Anthropic answers a spending limit set in its console with 400.
    if status == 400 && message.starts_with("You have reached your specified") {
        return (
            format!("{label} usage limit ({status}): {message}"),
            "limit",
        );
    }
    match status {
        401 => (
            format!("{label} refused the key ({status}): it needs a new key. {message}"),
            "key",
        ),
        // Not the key: this request (a guardrail or moderation, as OpenRouter says).
        403 => (
            format!("{label} refused this request ({status}). {message}"),
            "refused",
        ),
        402 => (
            format!("{label} says the account is out of credit (402). {message}"),
            "credit",
        ),
        429 => (format!("{label} usage limit (429): {message}"), "limit"),
        // The request never reached a model (a model it does not have, or too long): nothing
        // was billed.
        400 | 404 | 413 | 422 => (
            format!("{label} did not take this request ({status}): {message}"),
            "input",
        ),
        // Too busy to take the request (Anthropic's 529): nothing was billed.
        503 | 529 => (
            format!("{label} is too busy right now ({status}). {message}"),
            "busy",
        ),
        _ => (format!("{label} answered {status}: {message}"), "service"),
    }
}

/// When a usage limit resets, in milliseconds since 1970, from the service's headers:
/// `X-RateLimit-Reset` (OpenRouter's, in milliseconds or seconds) or `Retry-After` (seconds).
fn reset_at(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()?;
    let number = |name: &str| headers.get(name)?.to_str().ok()?.trim().parse::<u64>().ok();
    if let Some(n) = number("x-ratelimit-reset") {
        let ms = if n < 100_000_000_000 {
            n.checked_mul(1000)?
        } else {
            n
        };
        return (ms > now).then_some(ms);
    }
    number("retry-after").and_then(|secs| now.checked_add(secs.checked_mul(1000)?))
}

/// Micros per million tokens as dollars per million tokens (3_000_000 → 3.0).
fn per_million_dollars(micros: u64) -> f64 {
    // Exact for every price a service lists (at most six decimals).
    #[allow(clippy::cast_precision_loss)]
    let dollars = micros as f64 / 1_000_000.0;
    dollars
}

/// The request's whole bill, as exact decimal dollars: OpenRouter's own charge, plus what the
/// AI company billed the owner's own key on OpenRouter, when the account uses one
/// (`cost_details.upstream_inference_cost`). None when the service gave neither.
fn service_bill(usage: &Value) -> Option<String> {
    let micros = |v: Option<&Value>| -> Option<u64> {
        let text = match v? {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        };
        plenipo_runtime::pricing::dollars_to_micros(&text)
    };
    // xAI's own bill: ten billion "ticks" to the dollar, so ten thousand to the micro.
    let ticks = usage
        .get("cost_in_usd_ticks")
        .and_then(Value::as_u64)
        .map(|t| t.div_ceil(10_000));
    let own = micros(usage.get("cost")).or(ticks);
    let upstream = micros(usage.pointer("/cost_details/upstream_inference_cost"));
    let total = match (own, upstream) {
        (None, None) => return None,
        (a, b) => a.unwrap_or(0).checked_add(b.unwrap_or(0))?,
    };
    Some(format!("{}.{:06}", total / 1_000_000, total % 1_000_000))
}

/// Google's answer to a key it does not know, or one that has expired.
fn google_bad_key(body: &str) -> bool {
    body.contains("API_KEY_INVALID")
        || body.contains("API_KEY_EXPIRED")
        || body.to_lowercase().contains("api key not valid")
        || body.contains("Please pass a valid API key")
}

/// A refusal MiniMax sends as a 200 with a JSON body (`base_resp`) instead of a stream: its
/// words and kind, or None when the body is not one. Nothing was billed.
fn minimax_refusal(body: &str) -> Option<(String, &'static str)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let code = v
        .pointer("/base_resp/status_code")
        .and_then(Value::as_i64)?;
    if code == 0 {
        return None;
    }
    let said = v
        .pointer("/base_resp/status_msg")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    Some(match code {
        1008 => (
            format!("MiniMax says the account is out of credit ({code}). {said}"),
            "credit",
        ),
        1004 | 2049 => (
            format!("MiniMax refused the key ({code}): it needs a new key. {said}"),
            "key",
        ),
        1002 => (format!("MiniMax usage limit ({code}): {said}"), "limit"),
        _ => (
            format!("MiniMax did not take this request ({code}): {said}"),
            "input",
        ),
    })
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
    // A key that may chat but not list models (OpenAI's restricted keys without "Models:
    // Read"): it works; the models and prices come from Plenipo's own row.
    if status == 403 && body.contains("api.model.read") {
        emit(
            out,
            &json!({ "signedIn": true, "limit": Value::Null, "models": [] }),
        );
        return 0;
    }
    // Google answers a key it does not know, or an expired one, with 400 and says so.
    let bad_key = status == 400 && google_bad_key(&body);
    if status == 401 || status == 403 || bad_key {
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
    // Google lists `models`, each named "models/<id>".
    if let Some(list) = v.get("models").and_then(Value::as_array) {
        return list
            .iter()
            .filter_map(|m| {
                let id = m.get("name").and_then(Value::as_str)?;
                let id = id.strip_prefix("models/").unwrap_or(id);
                if id.is_empty() || id.len() > 200 || id.chars().any(char::is_control) {
                    return None;
                }
                Some(json!({
                    "id": id,
                    "name": m.get("displayName").and_then(Value::as_str)
                        .map(|n| n.chars().take(120).collect::<String>()),
                    "contextTokens": m.get("inputTokenLimit").and_then(Value::as_u64),
                    "price": Value::Null,
                }))
            })
            .take(MAX_MODELS)
            .collect();
    }
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
            let price = m.get("pricing").filter(|p| !extra_fees(p)).and_then(|p| {
                let output = per_million(p, "completion")?;
                // Thinking priced above the answer would not be covered either.
                if per_million(p, "internal_reasoning").is_some_and(|r| r > output) {
                    return None;
                }
                Some(json!({
                    "input": per_million(p, "prompt")?,
                    "cachedInput": per_million(p, "input_cache_read"),
                    "output": output,
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

/// A fee a model's price list names beyond its token prices: a charge per request or per web
/// search, or other prices for some requests (`overrides`). Plenipo's most-a-request-can-cost
/// covers tokens only, so such a model is not priced (ADR-085 §3.4).
fn extra_fees(pricing: &Value) -> bool {
    let nonzero = |k: &str| {
        pricing.get(k).is_some_and(|v| {
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Null => return false,
                _ => return true,
            };
            text.chars().any(|c| c.is_ascii_digit() && c != '0')
        })
    };
    let overrides = pricing.get("overrides").is_some_and(|o| match o {
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        _ => true,
    });
    nonzero("request") || nonzero("web_search") || overrides
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
    // The price set aside for (micros per million tokens): OpenRouter may not route the request
    // anywhere dearer.
    let price = match (
        flag(args, "--price-input").and_then(|n| n.parse::<u64>().ok()),
        flag(args, "--price-output").and_then(|n| n.parse::<u64>().ok()),
    ) {
        (Some(input), Some(output)) => Some((input, output)),
        _ => None,
    };

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
    let (path, body) = request_body(
        client.service,
        &model,
        &sent,
        max_output,
        effort.as_deref(),
        price,
    );
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
        .signed_chat(http.post(&address))
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
        let reset = (status == 429)
            .then(|| reset_at(response.headers()))
            .flatten();
        let text = read_capped(response, 64 * 1024).await.unwrap_or_default();
        let (message, kind) = error_text(client.service, status, &text, &client.key);
        // The reset time, in the form the Router reads ("…|<ms>").
        let message = match reset {
            Some(ms) => format!("{message}|{ms}"),
            None => message,
        };
        return fail(out, &message, kind);
    }
    // A 200 that is not a stream: MiniMax sends its refusals so.
    let streamed = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|t| t.contains("event-stream"));
    if !streamed {
        let text = read_capped(response, 64 * 1024).await.unwrap_or_default();
        if let Some((message, kind)) = minimax_refusal(&text) {
            return fail(out, &hide(&message, &client.key), kind);
        }
        return fail(
            out,
            &format!(
                "{} answered with something other than a stream",
                client.service.label()
            ),
            "service",
        );
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
        match event {
            Event::Emit(v) => emit(out, &v),
            Event::Failed(message) => {
                let message = hide(&message, &client.key);
                return fail(out, &message, "service");
            }
        }
    }
    // Complete only when the service said it was done: `[DONE]`, the end message, or a reason
    // it stopped. A cut-off answer is not a finished one.
    if !stream.finished && stream.stop.is_none() {
        return fail(
            out,
            &format!(
                "{}'s answer ended before it was complete",
                client.service.label()
            ),
            "service",
        );
    }
    if stream.stop.as_deref() == Some("error") {
        return fail(
            out,
            &format!(
                "{} stopped the answer with an error",
                client.service.label()
            ),
            "service",
        );
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
    price: Option<(u64, u64)>,
) -> (&'static str, Value) {
    match service.protocol() {
        plenipo_guard::PaidProtocol::OpenAiChat => {
            let mut body = json!({
                "model": model,
                "messages": messages,
                "stream": true,
            });
            body[service.max_tokens_field()] = json!(max_output);
            if service.asks_for_stream_usage() {
                body["stream_options"] = json!({ "include_usage": true });
            }
            // MiniMax puts the thinking apart from the answer only when asked.
            if service == PaidService::MiniMax {
                body["reasoning_split"] = json!(true);
            }
            // OpenAI's standard prices: without this, a project set to its dearer Fast mode
            // would be billed more than the row says.
            if service == PaidService::OpenAi {
                body["service_tier"] = json!("default");
            }
            if service == PaidService::OpenRouter {
                // OpenRouter's own bill for the request, in its final line.
                body["usage"] = json!({ "include": true });
                if let Some(level) = effort {
                    body["reasoning"] = json!({ "effort": level });
                }
                // Only where it costs no more than the price set aside for, and no fee for the
                // request itself (dollars per million tokens, ADR-085 §2.5).
                if let Some((input, output)) = price {
                    body["provider"] = json!({ "max_price": {
                        "prompt": per_million_dollars(input),
                        "completion": per_million_dollars(output),
                        "request": 0,
                    } });
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
            if let Some(level) = effort {
                body["output_config"] = json!({ "effort": level });
            }
            // Processed anywhere, at the row's price: a workspace may default to the United
            // States only, at 1.1 times (not offered for Haiku, which refuses the field).
            if !model.contains("haiku") {
                body["inference_geo"] = json!("global");
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
    input_tokens: Option<u64>,
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
            input_tokens: None,
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
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
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
                    // Counts as the service gave them: a count it did not give stays unknown
                    // (never taken as zero).
                    let n = |p: &str| u.pointer(p).and_then(Value::as_u64);
                    let input = n("/prompt_tokens");
                    // The answer's tokens, thinking included: where a service counts the thinking
                    // only in the total, the total less the input (never fewer than it says).
                    let output = match (n("/completion_tokens"), n("/total_tokens"), input) {
                        (Some(c), Some(t), Some(i)) => Some(c.max(t.saturating_sub(i))),
                        (c, _, _) => c,
                    };
                    let cached = n("/prompt_tokens_details/cached_tokens")
                        .or_else(|| n("/prompt_cache_hit_tokens"))
                        .or_else(|| n("/cached_tokens"))
                        .unwrap_or(0);
                    self.usage = Some(json!({
                        "inputTokens": input,
                        "cachedTokens": cached,
                        "outputTokens": output,
                        "costDollars": service_bill(u),
                    }));
                }
            }
            plenipo_guard::PaidProtocol::Anthropic => match v.get("type").and_then(Value::as_str) {
                Some("message_start") => {
                    let n = |p: &str| v.pointer(p).and_then(Value::as_u64).unwrap_or(0);
                    self.cached_tokens = n("/message/usage/cache_read_input_tokens");
                    self.input_tokens = v
                        .pointer("/message/usage/input_tokens")
                        .and_then(Value::as_u64)
                        .map(|fresh| {
                            fresh
                                + self.cached_tokens
                                + n("/message/usage/cache_creation_input_tokens")
                        });
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
                    let out = v.pointer("/usage/output_tokens").and_then(Value::as_u64);
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
        // A last line without a line break.
        let mut events = if self.pending.is_empty() || self.finished {
            Vec::new()
        } else {
            self.feed(b"\n")
        };
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
        // A page that is not JSON, echoing the key where the words are cut short: hidden first,
        // so no part of it is left past the cut.
        let page = format!("{}Authorization: Bearer {KEY}", "x".repeat(268));
        let (text, _) = error_text(PaidService::OpenRouter, 500, &page, KEY);
        assert!(!text.contains(&KEY[12..20]), "{text}");
        assert!(!text.contains("sk-or"), "{text}");
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

    #[test]
    fn each_answer_says_what_it_means_and_what_was_billed() {
        let kind = |status| error_text(PaidService::OpenRouter, status, "{}", KEY).1;
        assert_eq!(kind(401), "key");
        // Not the key: a guardrail or moderation refused this request.
        assert_eq!(kind(403), "refused");
        assert!(error_text(PaidService::OpenRouter, 403, "{}", KEY)
            .0
            .contains("refused this request"));
        // Never reached a model: nothing billed.
        for status in [400, 404, 413, 422] {
            assert_eq!(kind(status), "input", "{status}");
        }
        // It may have reached a model: counted at the most it could have cost.
        assert_eq!(kind(500), "service");
    }

    #[test]
    fn a_usage_limit_says_when_it_resets() {
        use reqwest::header::{HeaderMap, HeaderValue};
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let mut h = HeaderMap::new();
        let at = now + 60_000;
        h.insert(
            "x-ratelimit-reset",
            HeaderValue::from_str(&at.to_string()).unwrap(),
        );
        assert_eq!(reset_at(&h), Some(at));
        let mut h = HeaderMap::new();
        h.insert("retry-after", HeaderValue::from_static("30"));
        let got = reset_at(&h).unwrap();
        assert!(got >= now + 30_000 && got < now + 40_000, "{got}");
        assert_eq!(reset_at(&HeaderMap::new()), None);
    }

    #[test]
    fn a_request_goes_nowhere_dearer_than_the_price_set_aside_for() {
        let (_, body) = request_body(
            PaidService::OpenRouter,
            "moonshotai/kimi-k3",
            &[],
            16_000,
            None,
            Some((3_000_000, 15_000_000)),
        );
        assert_eq!(
            body["provider"]["max_price"],
            json!({ "prompt": 3.0, "completion": 15.0, "request": 0 })
        );
        assert_eq!(body["max_tokens"], 16_000);
    }

    #[test]
    fn a_model_with_a_fee_beyond_its_token_prices_is_not_priced() {
        let body = json!({ "data": [
            { "id": "plain/model", "pricing": { "prompt": "0.000003", "completion": "0.000015",
              "request": "0", "web_search": "0", "internal_reasoning": "0.000015" } },
            { "id": "per/request", "pricing": { "prompt": "0.000003", "completion": "0.000015",
              "request": "0.005" } },
            { "id": "web/search", "pricing": { "prompt": "0.000003", "completion": "0.000015",
              "web_search": "0.01" } },
            { "id": "dear/thinking", "pricing": { "prompt": "0.000003", "completion": "0.000015",
              "internal_reasoning": "0.00006" } },
            { "id": "long/prompts", "pricing": { "prompt": "0.000003", "completion": "0.000015",
              "overrides": [{ "prompt": "0.000006" }] } },
        ] })
        .to_string();
        let models = models_from(PaidService::OpenRouter, &body);
        assert_eq!(models[0]["price"]["output"], 15_000_000);
        for m in &models[1..] {
            assert!(m["price"].is_null(), "{m}");
        }
    }

    #[test]
    fn the_bill_adds_what_the_owners_own_key_was_billed() {
        let usage =
            json!({ "cost": "0.001", "cost_details": { "upstream_inference_cost": 0.019 } });
        assert_eq!(service_bill(&usage).as_deref(), Some("0.020000"));
        assert_eq!(
            service_bill(&json!({ "cost": 0.5 })).as_deref(),
            Some("0.500000")
        );
        assert_eq!(service_bill(&json!({})), None);
    }

    #[test]
    fn the_last_line_needs_no_line_break_and_a_null_error_is_no_error() {
        let mut s = Stream::new(PaidService::OpenRouter);
        let events =
            s.feed(b"data: {\"error\":null,\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n");
        assert!(events.iter().all(|e| matches!(e, Event::Emit(_))));
        s.feed(b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":1}}");
        assert_eq!(s.stop, None, "not read until the stream ends");
        s.end();
        assert_eq!(s.stop.as_deref(), Some("stop"));
        assert_eq!(s.answer, "Hi");
        // Counts the service did not give stay unknown, never zero.
        let mut s = Stream::new(PaidService::OpenRouter);
        s.feed(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3}}\n");
        assert!(s.usage.as_ref().unwrap()["outputTokens"].is_null());
    }

    #[test]
    fn each_company_is_asked_in_its_own_words() {
        let messages = [json!({ "role": "user", "content": "Hi" })];
        let body = |s: PaidService, effort: Option<&str>| {
            request_body(s, "m", &messages, 16_000, effort, None)
        };
        // Each company's own field for the answer's length, as its reference names it.
        let (path, openai) = body(PaidService::OpenAi, Some("high"));
        assert_eq!(path, "/chat/completions");
        assert_eq!(openai["max_completion_tokens"], 16_000);
        assert!(openai.get("max_tokens").is_none());
        assert_eq!(openai["reasoning_effort"], "high");
        assert_eq!(openai["stream_options"]["include_usage"], true);
        assert!(openai.get("provider").is_none(), "only OpenRouter routes");
        // OpenAI's standard prices, whatever the project's default.
        assert_eq!(openai["service_tier"], "default");
        for s in [
            PaidService::Moonshot,
            PaidService::Alibaba,
            PaidService::Xai,
        ] {
            let b = body(s, None).1;
            assert_eq!(b["max_completion_tokens"], 16_000, "{s:?}");
            assert!(b.get("max_tokens").is_none(), "{s:?}");
            assert!(b.get("service_tier").is_none(), "{s:?}");
        }
        // Alibaba's max_tokens would leave the thinking out; MiniMax's counts it.
        for s in [
            PaidService::MiniMax,
            PaidService::DeepSeek,
            PaidService::Zai,
            PaidService::Google,
        ] {
            assert_eq!(body(s, None).1["max_tokens"], 16_000, "{s:?}");
        }
        // Mistral refuses fields it does not know, and sends the counts by itself.
        assert!(body(PaidService::Mistral, None)
            .1
            .get("stream_options")
            .is_none());
        // MiniMax keeps its thinking apart when asked.
        assert_eq!(body(PaidService::MiniMax, None).1["reasoning_split"], true);
        // Google's OpenAI-style chat lives under its own base.
        assert_eq!(
            body(PaidService::Google, None).0,
            "/openai/chat/completions"
        );
        // Anthropic's own messages, with its effort.
        let (path, anthropic) = body(PaidService::Anthropic, Some("max"));
        assert_eq!(path, "/messages");
        assert_eq!(anthropic["max_tokens"], 16_000);
        assert_eq!(anthropic["output_config"]["effort"], "max");
        assert!(anthropic.get("stream_options").is_none());
        // Processed anywhere, at the row's price; Haiku refuses the field.
        assert_eq!(anthropic["inference_geo"], "global");
        let (_, haiku) = request_body(
            PaidService::Anthropic,
            "claude-haiku-4-5",
            &messages,
            16_000,
            None,
            None,
        );
        assert!(haiku.get("inference_geo").is_none());
    }

    #[test]
    fn googles_own_list_of_models_is_read() {
        let body = json!({ "models": [
            { "name": "models/gemini-3.8-flash", "displayName": "Gemini 3.8 Flash",
              "inputTokenLimit": 1_048_576 },
            { "name": "models/gemini-3.5-flash-lite" },
        ] })
        .to_string();
        let models = models_from(PaidService::Google, &body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "gemini-3.8-flash");
        assert_eq!(models[0]["contextTokens"], 1_048_576);
        assert!(models[0]["price"].is_null());
        assert_eq!(models[1]["id"], "gemini-3.5-flash-lite");
    }

    #[test]
    fn a_busy_service_took_nothing() {
        for status in [503, 529] {
            let (text, kind) = error_text(PaidService::Anthropic, status, "{}", KEY);
            assert_eq!(kind, "busy");
            assert!(text.contains("too busy"), "{text}");
        }
    }

    #[test]
    fn counts_come_as_each_company_gives_them() {
        // DeepSeek names its cached tokens its own way.
        let mut s = Stream::new(PaidService::DeepSeek);
        s.feed(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"prompt_cache_hit_tokens\":40,\"completion_tokens\":10}}\n");
        assert_eq!(s.usage.as_ref().unwrap()["cachedTokens"], 40);
        // A service that counts the thinking only in the total: the total less the input.
        let mut s = Stream::new(PaidService::Google);
        s.feed(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":10,\"total_tokens\":150}}\n");
        assert_eq!(s.usage.as_ref().unwrap()["outputTokens"], 50);
    }

    #[test]
    fn a_bad_key_or_a_spending_limit_behind_a_400_is_read_as_what_it_is() {
        // Google's answers to a key it does not know, or an expired one.
        for body in [
            r#"{"error":{"code":400,"message":"Please pass a valid API key","status":"INVALID_ARGUMENT"}}"#,
            r#"{"error":{"code":400,"message":"API key expired.","details":[{"reason":"API_KEY_EXPIRED"}]}}"#,
            r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#,
        ] {
            assert_eq!(
                error_text(PaidService::Google, 400, body, KEY).1,
                "key",
                "{body}"
            );
        }
        // Anthropic's spending limit set in its console.
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"You have reached your specified API usage limits. You will regain access on 2026-11-01 at 00:00 UTC."}}"#;
        assert_eq!(
            error_text(PaidService::Anthropic, 400, body, KEY).1,
            "limit"
        );
        // Any other 400 never reached a model.
        assert_eq!(
            error_text(
                PaidService::Anthropic,
                400,
                r#"{"error":{"message":"bad field"}}"#,
                KEY
            )
            .1,
            "input"
        );
    }

    #[test]
    fn minimax_refusals_inside_a_200_are_not_billed() {
        let refused = |code: i64| {
            minimax_refusal(
                &json!({ "base_resp": { "status_code": code, "status_msg": "no" } }).to_string(),
            )
            .map(|(_, kind)| kind)
        };
        assert_eq!(refused(1008), Some("credit"));
        assert_eq!(refused(1004), Some("key"));
        assert_eq!(refused(2049), Some("key"));
        assert_eq!(refused(1002), Some("limit"));
        assert_eq!(refused(1026), Some("input"));
        assert_eq!(refused(0), None);
        assert_eq!(minimax_refusal("not json"), None);
    }

    #[test]
    fn xais_own_bill_is_read_and_a_missing_count_stays_unknown() {
        // Ten billion ticks to the dollar: 1,234,567,890 ticks is $0.123457 (rounded up).
        assert_eq!(
            service_bill(&json!({ "cost_in_usd_ticks": 1_234_567_890u64 })).as_deref(),
            Some("0.123457")
        );
        // Anthropic without its counts: unknown, never zero.
        let mut s = Stream::new(PaidService::Anthropic);
        s.feed(b"data: {\"type\":\"message_start\",\"message\":{\"usage\":{}}}\n");
        s.feed(b"data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{}}\n");
        let usage = s.usage.as_ref().unwrap();
        assert!(usage["inputTokens"].is_null() && usage["outputTokens"].is_null());
    }
}
