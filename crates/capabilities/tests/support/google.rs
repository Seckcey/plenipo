//! A stand-in for Google's sign-in and the Gmail, Google Calendar, and Google Drive APIs (Phase 20
//! part 20B), inside the stand-in services (`support/microsoft.rs` routes Google's hosts here). It
//! checks the owner's app (its client ID and secret), PKCE, and the loopback address like Google
//! does, can leave out one permission as when the owner unticks it on Google's page, and refuses
//! every address a token lacks the permission for (403). It keeps a small mailbox (with the
//! planted "forward all mail" email), a calendar, and a drive. Everything sent is recorded in the
//! world's `sent`.

#![allow(dead_code)]

use std::collections::HashMap;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use super::microsoft::{
    b64, error, form, json_resp, ok, redirect, today_at, Req, Resp, World, ATTACKER, CLIENT,
    PLANTED, USER,
};

/// The owner's own Google app in the tests: its client ID, and its secret (typed into the card,
/// kept only in the Vault).
pub const CLIENT_ID: &str = "123456789012-plenipotest.apps.googleusercontent.com";
pub const SECRET: &str = "GOCSPX-stand-in-secret-5c1b3e7f9a";
const BASE: &str = "https://www.googleapis.com/auth/";

/// A code waiting to be traded.
#[derive(Debug, Clone)]
pub struct Code {
    pub challenge: String,
    pub redirect: String,
    pub scopes: Vec<String>,
    /// Asked for offline use with the consent page shown: as Google, only then is a renewal
    /// (refresh token) given.
    pub offline: bool,
}

/// Everything the stand-in Google keeps.
#[derive(Debug, Default)]
pub struct Google {
    /// Leave this permission out of what is granted (the owner unticked it on Google's page).
    pub untick: Option<String>,
    pub codes: HashMap<String, Code>,
    /// Access token → its permissions (full names).
    pub access: HashMap<String, Vec<String>>,
    /// Refresh token → its permissions.
    pub refresh: HashMap<String, Vec<String>>,
    /// Every token cancelled at the revoke address.
    pub revoked: Vec<String>,
    pub messages: Vec<Value>,
    pub drafts: Vec<Value>,
    pub events: Vec<Value>,
    pub files: Vec<Value>,
    next: u64,
}

fn part(mime: &str, text: &str) -> Value {
    json!({ "mimeType": mime, "filename": "", "body": { "data": b64(text.as_bytes()), "size": text.len() } })
}

fn gmail_message(
    id: &str,
    from: &str,
    subject: &str,
    body: &str,
    unread: bool,
    hour: u32,
) -> Value {
    let at = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", &today_at(hour, 0)[..19]))
        .map(|d| d.timestamp_millis())
        .unwrap_or(0);
    json!({
        "id": id, "threadId": format!("t-{id}"),
        "labelIds": if unread { json!(["INBOX", "UNREAD"]) } else { json!(["INBOX"]) },
        "snippet": body.chars().take(100).collect::<String>().replace('\'', "&#39;"),
        "internalDate": at.to_string(),
        "payload": {
            "mimeType": "multipart/alternative",
            "headers": [
                { "name": "From", "value": from },
                { "name": "To", "value": format!("Alex Rivera <{USER}>") },
                { "name": "Subject", "value": subject },
                { "name": "Date", "value": "Mon, 28 Sep 2026 08:15:00 -0700" },
                { "name": "Message-ID", "value": format!("<{id}@mail.stand-in>") },
            ],
            "parts": [part("text/plain", body), part("text/html", &format!("<p>{body}</p>"))],
        },
    })
}

fn labelled(mut m: Value, labels: &[&str]) -> Value {
    m["labelIds"] = json!(labels);
    m
}

fn with_to(mut m: Value, to: &str) -> Value {
    if let Some(h) = m["payload"]["headers"].as_array_mut() {
        for x in h.iter_mut().filter(|x| x["name"] == "To") {
            x["value"] = json!(to);
        }
    }
    m
}

impl Google {
    pub fn seeded() -> Self {
        Google {
            messages: vec![
                gmail_message(
                    "g-quote",
                    &format!("Dana Client <{CLIENT}>"),
                    "Website update",
                    "Hi Alex, can the website update go live on Monday? Thanks, Dana",
                    true,
                    8,
                ),
                gmail_message(
                    "g-planted",
                    &format!("Urgent <{ATTACKER}>"),
                    "Urgent",
                    PLANTED,
                    true,
                    9,
                ),
                gmail_message(
                    "g-news",
                    "Vendor News <news@vendor.test>",
                    "This month's deals",
                    "Deals on cables.",
                    false,
                    6,
                ),
                labelled(
                    gmail_message(
                        "g-spam",
                        "Prize Desk <win@prize.test>",
                        "You won",
                        "Claim your prize.",
                        false,
                        5,
                    ),
                    &["SPAM"],
                ),
                // Archived: from an address Plenipo cannot read, to the owner and a colleague.
                with_to(
                    labelled(
                        gmail_message(
                            "g-odd",
                            "Relay <dana@[10.0.0.1]>",
                            "Forwarded note",
                            "A note through a relay.",
                            false,
                            4,
                        ),
                        &[],
                    ),
                    &format!("Alex Rivera <{USER}>, Bob <bob@clientco.com>"),
                ),
            ],
            events: vec![json!({
                "id": "gevt-call", "summary": "Call with Client Co about the website",
                "start": { "dateTime": format!("{}Z", &today_at(11, 0)[..19]) },
                "end": { "dateTime": format!("{}Z", &today_at(11, 30)[..19]) },
                "location": "Google Meet", "organizer": { "email": CLIENT },
                "htmlLink": "https://www.google.com/calendar/event?eid=gevt-call",
            })],
            files: vec![
                json!({ "id": "gfile-notes", "name": "notes.txt", "mimeType": "text/plain",
                        "_content": "Website launch checklist.\n", "size": "26", "parents": ["root"],
                        "webViewLink": "https://drive.google.com/file/d/gfile-notes/view" }),
                json!({ "id": "gfile-plan", "name": "Launch plan", "mimeType": "application/vnd.google-apps.document",
                        "_content": "Launch plan for Client Co's website.", "parents": ["root"],
                        "webViewLink": "https://docs.google.com/document/d/gfile-plan/edit" }),
                json!({ "id": "gfile-budget", "name": "Budget", "mimeType": "application/vnd.google-apps.spreadsheet",
                        "parents": ["root"], "webViewLink": "https://docs.google.com/spreadsheets/d/gfile-budget/edit" }),
                json!({ "id": "gfolder-shared", "name": "Shared with Client Co", "mimeType": "application/vnd.google-apps.folder",
                        "parents": ["root"], "webViewLink": "https://drive.google.com/drive/folders/gfolder-shared" }),
            ],
            ..Google::default()
        }
    }
}

fn google_error(status: &'static str, code: u16, reason: &str) -> Resp {
    json_resp(
        status,
        json!({ "error": { "code": code, "message": reason, "errors": [{ "reason": reason }] } }),
    )
}

/// A plain JSON web token with these claims (the stand-in does not sign it; Plenipo reads it
/// only from Google's own token answer).
fn id_token(claims: Value) -> String {
    format!(
        "{}.{}.stand-in",
        b64(br#"{"alg":"none"}"#),
        b64(claims.to_string().as_bytes())
    )
}

fn header_of(m: &Value, name: &str) -> String {
    m["payload"]["headers"]
        .as_array()
        .and_then(|h| {
            h.iter().find(|x| {
                x["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
        })
        .and_then(|x| x["value"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// A raw message's headers and text.
fn parse_raw(raw: &str) -> (Vec<(String, String)>, String) {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw.trim_end_matches('='))
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    // A line starting with a space goes on the header before it (folding).
    let head = head.replace("\r\n ", " ").replace("\r\n\t", " ");
    let headers: Vec<(String, String)> = head
        .split("\r\n")
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let body = base64::engine::general_purpose::STANDARD
        .decode(body.replace("\r\n", ""))
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    (headers, body)
}

fn addresses(header: &str) -> Vec<String> {
    header
        .split(',')
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
        .collect()
}

/// A Google host's path (`gmail.googleapis.com/…`, …).
pub fn route(req: &Req, path: &str, w: &mut World) -> Resp {
    let q = |k: &str| req.query.get(k).cloned().unwrap_or_default();
    if path == "accounts.google.com/o/oauth2/v2/auth" && req.method == "GET" {
        let redirect_uri = q("redirect_uri");
        if !redirect_uri.starts_with("http://127.0.0.1:") {
            return error("400 Bad Request", "redirect_uri_mismatch");
        }
        let fail = |e: &str| redirect(&format!("{redirect_uri}?error={e}&state={}", q("state")));
        if q("client_id") != CLIENT_ID {
            return fail("invalid_client");
        }
        if q("code_challenge_method") != "S256" || q("response_type") != "code" {
            return fail("invalid_request");
        }
        let asked: Vec<String> = q("scope").split(' ').map(str::to_owned).collect();
        w.asked.push(json!({
            "service": "google", "scope": q("scope"), "redirect": redirect_uri,
            "accessType": q("access_type"), "method": q("code_challenge_method"),
            "prompt": q("prompt"),
        }));
        if w.decline_consent {
            return fail("access_denied");
        }
        let untick = w.google.untick.clone();
        let scopes: Vec<String> = asked
            .into_iter()
            .filter(|s| untick.as_deref().is_none_or(|u| !s.ends_with(u)))
            .collect();
        let code = format!("google-code-{}", w.id("c"));
        w.google.codes.insert(
            code.clone(),
            Code {
                challenge: q("code_challenge"),
                redirect: redirect_uri.clone(),
                scopes,
                offline: q("access_type") == "offline"
                    && q("prompt").split(' ').any(|p| p == "consent"),
            },
        );
        return redirect(&format!("{redirect_uri}?state={}&code={code}", q("state")));
    }
    if path == "oauth2.googleapis.com/token" && req.method == "POST" {
        return token(req, w);
    }
    if path == "oauth2.googleapis.com/revoke" && req.method == "POST" {
        let t = form(&req.body).get("token").cloned().unwrap_or_default();
        if let Some(scopes) = w.google.refresh.remove(&t) {
            w.google.access.retain(|_, s| *s != scopes);
            w.google.revoked.push(t);
            return ok(json!({}));
        }
        if w.google.access.remove(&t).is_some() {
            w.google.revoked.push(t);
            return ok(json!({}));
        }
        return json_resp("400 Bad Request", json!({ "error": "invalid_token" }));
    }
    let token = req
        .headers
        .get("authorization")
        .and_then(|a| a.strip_prefix("Bearer "))
        .unwrap_or_default();
    let Some(scopes) = w.google.access.get(token).cloned() else {
        return google_error("401 Unauthorized", 401, "authError");
    };
    let has = |s: &str| scopes.iter().any(|g| g == &format!("{BASE}{s}"));
    let forbidden = || google_error("403 Forbidden", 403, "insufficientPermissions");
    if let Some(rest) = path.strip_prefix("gmail.googleapis.com/gmail/v1/users/me/") {
        return gmail(req, rest, w, &has, forbidden);
    }
    if let Some(rest) = path.strip_prefix("www.googleapis.com/calendar/v3/calendars/primary/") {
        return calendar(req, rest, w, &has, forbidden);
    }
    if let Some(rest) = path.strip_prefix("www.googleapis.com/upload/drive/v3/") {
        if rest != "files" || req.method != "POST" {
            return google_error("404 Not Found", 404, "notFound");
        }
        if !has("drive.file") {
            return forbidden();
        }
        return upload(req, w);
    }
    if let Some(rest) = path.strip_prefix("www.googleapis.com/drive/v3/") {
        if !has("drive.readonly") {
            return forbidden();
        }
        return drive(req, rest, w);
    }
    google_error("404 Not Found", 404, "notFound")
}

fn token(req: &Req, w: &mut World) -> Resp {
    let f = form(&req.body);
    let get = |k: &str| f.get(k).cloned().unwrap_or_default();
    // A desktop app's secret: Google checks it, though it is "not treated as a secret".
    if get("client_id") != CLIENT_ID || get("client_secret") != SECRET {
        return json_resp("401 Unauthorized", json!({ "error": "invalid_client" }));
    }
    match get("grant_type").as_str() {
        "authorization_code" => {
            let Some(code) = w.google.codes.remove(&get("code")) else {
                return json_resp("400 Bad Request", json!({ "error": "invalid_grant" }));
            };
            if b64(&Sha256::digest(get("code_verifier").as_bytes())) != code.challenge
                || get("redirect_uri") != code.redirect
            {
                return json_resp("400 Bad Request", json!({ "error": "invalid_grant" }));
            }
            let access = w.token("ya29.a0");
            w.google.access.insert(access.clone(), code.scopes.clone());
            let mut answer = json!({
                "access_token": access, "expires_in": 3599,
                "scope": code.scopes.join(" "), "token_type": "Bearer",
                "id_token": id_token(json!({
                    "email": USER, "name": "Alex Rivera", "hd": "8westit.com",
                    "email_verified": true,
                })),
            });
            if code.offline {
                let refresh = w.token("1//0g");
                w.google
                    .refresh
                    .insert(refresh.clone(), code.scopes.clone());
                answer["refresh_token"] = json!(refresh);
            }
            ok(answer)
        }
        "refresh_token" => {
            if w.refuse_refresh {
                return json_resp("400 Bad Request", json!({ "error": "invalid_grant" }));
            }
            let Some(scopes) = w.google.refresh.get(&get("refresh_token")).cloned() else {
                return json_resp("400 Bad Request", json!({ "error": "invalid_grant" }));
            };
            let access = w.token("ya29.a0");
            w.google.access.insert(access.clone(), scopes.clone());
            ok(json!({
                "access_token": access, "expires_in": 3599, "scope": scopes.join(" "),
                "token_type": "Bearer",
            }))
        }
        _ => json_resp(
            "400 Bad Request",
            json!({ "error": "unsupported_grant_type" }),
        ),
    }
}

fn gmail(
    req: &Req,
    rest: &str,
    w: &mut World,
    has: &dyn Fn(&str) -> bool,
    forbidden: impl Fn() -> Resp,
) -> Resp {
    let q = |k: &str| req.query.get(k).cloned().unwrap_or_default();
    let reads = has("gmail.readonly");
    let compose = has("gmail.compose");
    match (req.method.as_str(), rest) {
        ("GET", "messages") => {
            if !reads {
                return forbidden();
            }
            let query = q("q");
            let words: Vec<&str> = query.split(' ').collect();
            let found: Vec<Value> = w
                .google
                .messages
                .iter()
                .filter(|m| {
                    let labelled = |l: &str| {
                        m["labelIds"]
                            .as_array()
                            .is_some_and(|ls| ls.iter().any(|x| x == l))
                    };
                    // As Gmail: spam and the bin only when asked for, and then only there.
                    let spam_or_bin = labelled("SPAM") || labelled("TRASH");
                    if spam_or_bin && q("includeSpamTrash") != "true" {
                        return false;
                    }
                    words.iter().all(|term| match term.split_once(':') {
                        Some(("in", "inbox")) => labelled("INBOX"),
                        Some(("in", "spam")) => labelled("SPAM"),
                        Some(("in", "trash")) => labelled("TRASH"),
                        Some(("is", "unread")) => m["labelIds"]
                            .as_array()
                            .is_some_and(|l| l.iter().any(|x| x == "UNREAD")),
                        Some(("from", who)) => header_of(m, "From").to_lowercase().contains(who),
                        Some(("after", _)) => true,
                        _ if term.is_empty() => true,
                        _ => {
                            let t = term.trim_matches('"').to_lowercase();
                            header_of(m, "Subject").to_lowercase().contains(&t)
                                || m["snippet"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_lowercase()
                                    .contains(&t)
                        }
                    })
                })
                .map(|m| json!({ "id": m["id"], "threadId": m["threadId"] }))
                .collect();
            ok(json!({ "messages": found, "resultSizeEstimate": found.len() }))
        }
        ("POST", "drafts") => {
            if !compose {
                return forbidden();
            }
            let v: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let (headers, text) = parse_raw(v["message"]["raw"].as_str().unwrap_or_default());
            let id = format!("r-{}", w.id("draft"));
            let mid = format!("m-{}", w.id("msg"));
            let draft = json!({
                "id": id,
                "message": {
                    "id": mid, "threadId": v["message"]["threadId"].as_str().unwrap_or(&mid),
                    "labelIds": ["DRAFT"],
                    "payload": {
                        "mimeType": "text/plain",
                        "headers": headers.iter().map(|(k, v)| json!({ "name": k, "value": v })).collect::<Vec<_>>(),
                        "body": { "data": b64(text.as_bytes()) },
                    },
                },
            });
            w.google.drafts.push(draft.clone());
            ok(json!({ "id": id, "message": { "id": mid, "labelIds": ["DRAFT"] } }))
        }
        ("POST", "drafts/send") => {
            if !compose {
                return forbidden();
            }
            let v: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let id = v["id"].as_str().unwrap_or_default();
            let Some(i) = w.google.drafts.iter().position(|d| d["id"] == id) else {
                return google_error("404 Not Found", 404, "notFound");
            };
            let d = w.google.drafts.remove(i);
            let m = &d["message"];
            let text = m["payload"]["body"]["data"]
                .as_str()
                .map(|x| {
                    String::from_utf8_lossy(
                        &base64::engine::general_purpose::URL_SAFE_NO_PAD
                            .decode(x)
                            .unwrap_or_default(),
                    )
                    .into_owned()
                })
                .unwrap_or_default();
            w.sent.push(json!({
                "service": "google", "kind": "mail",
                "to": addresses(&header_of(m, "To")), "cc": addresses(&header_of(m, "Cc")),
                "bcc": addresses(&header_of(m, "Bcc")),
                "subject": header_of(m, "Subject"), "inReplyTo": header_of(m, "In-Reply-To"),
                "text": text,
            }));
            ok(json!({ "id": m["id"], "threadId": m["threadId"], "labelIds": ["SENT"] }))
        }
        ("GET", r) if r.starts_with("drafts/") => {
            if !(compose || reads) {
                return forbidden();
            }
            let id = &r["drafts/".len()..];
            match w.google.drafts.iter().find(|d| d["id"] == id) {
                Some(d) => ok(d.clone()),
                None => google_error("404 Not Found", 404, "notFound"),
            }
        }
        ("GET", r) if r.starts_with("messages/") => {
            if !reads {
                return forbidden();
            }
            let id = &r["messages/".len()..];
            let Some(m) = w.google.messages.iter().find(|m| m["id"] == id) else {
                return google_error("404 Not Found", 404, "notFound");
            };
            let mut m = m.clone();
            if q("format") == "metadata" {
                m["payload"].as_object_mut().map(|p| p.remove("parts"));
            }
            ok(m)
        }
        _ => google_error("404 Not Found", 404, "notFound"),
    }
}

fn calendar(
    req: &Req,
    rest: &str,
    w: &mut World,
    has: &dyn Fn(&str) -> bool,
    forbidden: impl Fn() -> Resp,
) -> Resp {
    if rest != "events" {
        return google_error("404 Not Found", 404, "notFound");
    }
    let writes = has("calendar.events");
    let reads = writes || has("calendar.events.readonly");
    match req.method.as_str() {
        "GET" => {
            if !reads {
                return forbidden();
            }
            ok(json!({ "items": w.google.events }))
        }
        "POST" => {
            if !writes {
                return forbidden();
            }
            let v: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let id = format!("gevt-{}", w.id("e"));
            let mut e = v.clone();
            e["id"] = json!(id);
            e["htmlLink"] = json!(format!("https://www.google.com/calendar/event?eid={id}"));
            let guests: Vec<Value> = v["attendees"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|a| a["email"].clone())
                .collect();
            if !guests.is_empty() && req.query.get("sendUpdates").map(String::as_str) == Some("all")
            {
                w.sent.push(json!({
                    "service": "google", "kind": "invitation", "to": guests, "subject": v["summary"],
                }));
            }
            w.google.events.push(e.clone());
            ok(e)
        }
        _ => google_error("405 Method Not Allowed", 405, "badRequest"),
    }
}

fn drive(req: &Req, rest: &str, w: &mut World) -> Resp {
    let q = |k: &str| req.query.get(k).cloned().unwrap_or_default();
    let clean = |f: &Value| {
        let mut f = f.clone();
        if let Some(o) = f.as_object_mut() {
            o.retain(|k, _| !k.starts_with('_'));
        }
        f
    };
    if rest == "files" && req.method == "GET" {
        let query = q("q");
        let found: Vec<Value> = w
            .google
            .files
            .iter()
            .filter(|f| {
                if let Some(words) = query
                    .strip_prefix("fullText contains '")
                    .and_then(|r| r.split_once("' and"))
                    .map(|(w, _)| w.replace("\\'", "'").to_lowercase())
                {
                    f["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&words)
                        || f["_content"]
                            .as_str()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&words)
                } else if let Some(parent) = query
                    .strip_prefix('\'')
                    .and_then(|r| r.split_once("' in parents"))
                    .map(|(p, _)| p)
                {
                    f["parents"]
                        .as_array()
                        .is_some_and(|ps| ps.iter().any(|p| p == parent))
                } else {
                    true
                }
            })
            .map(clean)
            .collect();
        return ok(json!({ "files": found }));
    }
    let Some(r) = rest.strip_prefix("files/") else {
        return google_error("404 Not Found", 404, "notFound");
    };
    let (id, export) = match r.split_once('/') {
        Some((id, "export")) => (id, true),
        Some(_) => return google_error("404 Not Found", 404, "notFound"),
        None => (r, false),
    };
    let Some(f) = w.google.files.iter().find(|f| f["id"] == id) else {
        return google_error("404 Not Found", 404, "notFound");
    };
    let content = || Resp {
        status: "200 OK",
        headers: vec![("Content-Type".into(), "text/plain".into())],
        body: f["_content"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec(),
    };
    if export {
        if f["mimeType"] != "application/vnd.google-apps.document" || q("mimeType") != "text/plain"
        {
            return google_error("400 Bad Request", 400, "badRequest");
        }
        return content();
    }
    if q("alt") == "media" {
        if f["mimeType"]
            .as_str()
            .unwrap_or_default()
            .starts_with("application/vnd.google-apps.")
        {
            return google_error("403 Forbidden", 403, "fileNotDownloadable");
        }
        return content();
    }
    ok(clean(f))
}

fn upload(req: &Req, w: &mut World) -> Resp {
    let content_type = req.headers.get("content-type").cloned().unwrap_or_default();
    let Some(boundary) = content_type.split("boundary=").nth(1) else {
        return google_error("400 Bad Request", 400, "badContent");
    };
    let body = String::from_utf8_lossy(&req.body).into_owned();
    let parts: Vec<&str> = body.split(&format!("--{boundary}")).collect();
    // ["", meta, content, "--\r\n"]
    let (Some(meta), Some(content)) = (parts.get(1), parts.get(2)) else {
        return google_error("400 Bad Request", 400, "badContent");
    };
    let meta: Value = meta
        .split_once("\r\n\r\n")
        .and_then(|(_, j)| serde_json::from_str(j.trim()).ok())
        .unwrap_or(Value::Null);
    let text = content
        .split_once("\r\n\r\n")
        .map(|(_, t)| t.strip_suffix("\r\n").unwrap_or(t).to_owned())
        .unwrap_or_default();
    // With drive.file, only folders the app made (none here) take new files.
    if meta["parents"]
        .as_array()
        .is_some_and(|p| p.iter().any(|x| x != "root"))
    {
        return google_error("404 Not Found", 404, "notFound");
    }
    let id = format!("gfile-{}", w.id("f"));
    let f = json!({
        "id": id, "name": meta["name"], "mimeType": "text/plain", "_content": text,
        "size": text.len().to_string(), "parents": ["root"],
        "webViewLink": format!("https://drive.google.com/file/d/{id}/view"),
    });
    w.google.files.push(f);
    ok(
        json!({ "id": id, "name": meta["name"], "webViewLink": format!("https://drive.google.com/file/d/{id}/view") }),
    )
}
