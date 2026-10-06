//! A stand-in for GitHub's short-code sign-in and the few reading addresses of its web interface
//! the GitHub connection uses (Phase 25, ADR-204), inside the stand-in services
//! (`support/microsoft.rs` routes `github.com/…` and `api.github.com/…` here). Like GitHub, it
//! answers its sign-in addresses with 200 and an `error` while it waits, needs no secret for a
//! short code or a renewal, rotates the long-lived sign-in on each renewal, and lists only the
//! repositories of the accounts the app was added to. It records whether a token ever reached
//! `github.com`, which it never should.

#![allow(dead_code)]

use std::collections::HashMap;

use serde_json::{json, Value};

use super::microsoft::{form, ok, Req, Resp, World};

/// 8 West's GitHub App in the tests (not a real one).
pub const CLIENT_ID: &str = "Iv1.0123456789abcdef";
pub const APP_SLUG: &str = "plenipo-test-app";
/// Who signs in.
pub const LOGIN: &str = "frankieg";
pub const ORG: &str = "8west";
/// The code the owner types.
pub const USER_CODE: &str = "WDJB-MJHT";

/// Everything the stand-in GitHub keeps.
#[derive(Debug, Default)]
pub struct Github {
    /// How many times a short code is asked about before the owner has typed it.
    pub pending_polls: u32,
    /// The owner says no on GitHub's page.
    pub deny: bool,
    /// The code runs out.
    pub expire: bool,
    /// Ask once more slowly.
    pub slow_down_once: bool,
    /// Send the owner to another page with the code (never shown).
    pub elsewhere: bool,
    /// Sign-ins that don't expire (no refresh token).
    pub no_expiry: bool,
    /// The app shows more permissions on Client Co than it asks for.
    pub extra_permissions: bool,
    /// GitHub is busy (503) for this many more asks about a short code.
    pub fail_polls: u32,
    /// Repositories in the owner's own account, and in the organization.
    pub own_repositories: u32,
    pub org_repositories: u32,
    /// Device code → how many times it was asked about.
    pub codes: HashMap<String, u32>,
    /// Access tokens, and refresh tokens, now valid.
    pub access: Vec<String>,
    pub refresh: Vec<String>,
    /// A token ever reached `github.com` (never should).
    pub token_on_github_com: bool,
    /// A request to `github.com` ever asked for anything but JSON.
    pub not_json: bool,
    next: u64,
}

impl Github {
    pub fn seeded() -> Self {
        Self {
            pending_polls: 1,
            own_repositories: 3,
            org_repositories: 2,
            ..Self::default()
        }
    }

    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{:016}", self.next)
    }

    /// The tests' knobs (`POST /_control/knobs` with `{"github": {...}}`).
    pub fn knobs(&mut self, v: &Value) {
        let flag = |k: &str| v[k].as_bool();
        if let Some(b) = flag("deny") {
            self.deny = b;
        }
        if let Some(b) = flag("expire") {
            self.expire = b;
        }
        if let Some(b) = flag("slowDownOnce") {
            self.slow_down_once = b;
        }
        if let Some(b) = flag("elsewhere") {
            self.elsewhere = b;
        }
        if let Some(b) = flag("noExpiry") {
            self.no_expiry = b;
        }
        if let Some(b) = flag("extraPermissions") {
            self.extra_permissions = b;
        }
        if let Some(n) = v["pendingPolls"].as_u64() {
            self.pending_polls = n as u32;
        }
        if let Some(n) = v["failPolls"].as_u64() {
            self.fail_polls = n as u32;
        }
        if let Some(n) = v["ownRepositories"].as_u64() {
            self.own_repositories = n as u32;
        }
        if let Some(n) = v["orgRepositories"].as_u64() {
            self.org_repositories = n as u32;
        }
    }

    fn bearer<'a>(&self, req: &'a Req) -> Option<&'a str> {
        req.headers
            .get("authorization")
            .and_then(|a| a.strip_prefix("Bearer "))
    }

    fn signed_in(&self, req: &Req) -> bool {
        self.bearer(req)
            .is_some_and(|t| self.access.iter().any(|a| a == t))
    }

    fn tokens(&mut self) -> Value {
        let access = self.id("ghu_");
        self.access.push(access.clone());
        if self.no_expiry {
            return json!({ "access_token": access, "token_type": "bearer", "scope": "" });
        }
        let refresh = self.id("ghr_");
        self.refresh.push(refresh.clone());
        json!({
            "access_token": access, "expires_in": 28800,
            "refresh_token": refresh, "refresh_token_expires_in": 15_811_200,
            "token_type": "bearer", "scope": "",
        })
    }

    fn repositories(owner: &str, n: u32, private_every: u32) -> Vec<Value> {
        (1..=n)
            .map(|i| {
                json!({
                    "id": i, "name": format!("{owner}-repo-{i:03}"),
                    "full_name": format!("{owner}/{owner}-repo-{i:03}"),
                    "owner": { "login": owner }, "private": i % private_every == 0,
                    "description": format!("Repository {i} of {owner}"),
                    "updated_at": "2026-10-05T12:00:00Z",
                })
            })
            .collect()
    }
}

/// `github.com/…` (the short-code sign-in) and `api.github.com/…` (the reading addresses).
pub fn route(req: &Req, path: &str, w: &mut World) -> Resp {
    let g = &mut w.github;
    if let Some(rest) = path.strip_prefix("github.com/") {
        if req.headers.contains_key("authorization") {
            g.token_on_github_com = true;
        }
        if req.headers.get("accept").map(String::as_str) != Some("application/json")
            && req.method == "POST"
        {
            g.not_json = true;
        }
        let f = form(&req.body);
        let field = |k: &str| f.get(k).cloned().unwrap_or_default();
        return match (req.method.as_str(), rest) {
            // The page the owner opens: the stand-in browser just reads it.
            ("GET", "login/device") => Resp {
                status: "200 OK",
                headers: vec![("Content-Type".into(), "text/html".into())],
                body: b"<h1>Device activation</h1>".to_vec(),
            },
            ("POST", "login/device/code") => {
                if field("client_id") != CLIENT_ID {
                    return ok(json!({ "error": "incorrect_client_credentials" }));
                }
                let device = g.id("dc_");
                g.codes.insert(device.clone(), 0);
                let page = if g.elsewhere {
                    "https://evil.example/login/device"
                } else {
                    "https://github.com/login/device"
                };
                ok(json!({
                    "device_code": device, "user_code": USER_CODE,
                    "verification_uri": page, "expires_in": 900, "interval": 1,
                }))
            }
            ("POST", "login/oauth/access_token") => {
                if field("client_id") != CLIENT_ID || f.contains_key("client_secret") {
                    return ok(json!({ "error": "incorrect_client_credentials" }));
                }
                match field("grant_type").as_str() {
                    "urn:ietf:params:oauth:grant-type:device_code" => {
                        if g.fail_polls > 0 {
                            g.fail_polls -= 1;
                            return Resp {
                                status: "503 Service Unavailable",
                                headers: vec![],
                                body: Vec::new(),
                            };
                        }
                        let device = field("device_code");
                        let Some(asked) = g.codes.get_mut(&device) else {
                            return ok(json!({ "error": "incorrect_device_code" }));
                        };
                        *asked += 1;
                        let asked = *asked;
                        if g.expire {
                            return ok(json!({ "error": "expired_token" }));
                        }
                        if g.slow_down_once {
                            g.slow_down_once = false;
                            return ok(json!({ "error": "slow_down", "interval": 2 }));
                        }
                        if asked <= g.pending_polls {
                            return ok(json!({ "error": "authorization_pending" }));
                        }
                        if g.deny {
                            return ok(json!({ "error": "access_denied" }));
                        }
                        g.codes.remove(&device);
                        ok(g.tokens())
                    }
                    "refresh_token" => {
                        let refresh = field("refresh_token");
                        match g.refresh.iter().position(|r| *r == refresh) {
                            Some(i) => {
                                g.refresh.remove(i);
                                ok(g.tokens())
                            }
                            None => ok(json!({ "error": "bad_refresh_token" })),
                        }
                    }
                    _ => ok(json!({ "error": "unsupported_grant_type" })),
                }
            }
            _ => Resp {
                status: "404 Not Found",
                headers: vec![],
                body: Vec::new(),
            },
        };
    }
    let Some(rest) = path.strip_prefix("api.github.com/") else {
        return Resp {
            status: "404 Not Found",
            headers: vec![],
            body: Vec::new(),
        };
    };
    if !g.signed_in(req) {
        return Resp {
            status: "401 Unauthorized",
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: br#"{"message":"Bad credentials"}"#.to_vec(),
        };
    }
    let page = |k: &str, d: usize| {
        req.query
            .get(k)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(d)
    };
    let (per_page, page_no) = (page("per_page", 30).clamp(1, 100), page("page", 1).max(1));
    let slice = |all: Vec<Value>| -> Vec<Value> {
        all.into_iter()
            .skip((page_no - 1) * per_page)
            .take(per_page)
            .collect()
    };
    match rest {
        "user" => ok(json!({ "login": LOGIN, "id": 1001, "name": "Frankie G" })),
        "user/installations" => {
            let mut all = vec![
                json!({ "id": 11, "account": { "login": LOGIN, "type": "User" },
                        "repository_selection": "all", "permissions": { "metadata": "read" } }),
                json!({ "id": 22, "account": { "login": ORG, "type": "Organization" },
                        "repository_selection": "selected", "permissions": { "metadata": "read" } }),
            ];
            if g.extra_permissions {
                all.push(
                    json!({ "id": 33, "account": { "login": "client-co", "type": "Organization" },
                                 "repository_selection": "all",
                                 "permissions": { "metadata": "read", "contents": "write" } }),
                );
            }
            let total = all.len();
            ok(json!({ "total_count": total, "installations": slice(all) }))
        }
        other => {
            let Some(id) = other
                .strip_prefix("user/installations/")
                .and_then(|r| r.strip_suffix("/repositories"))
            else {
                return Resp {
                    status: "404 Not Found",
                    headers: vec![],
                    body: Vec::new(),
                };
            };
            let all = match id {
                "11" => Github::repositories(LOGIN, g.own_repositories, 2),
                "22" => Github::repositories(ORG, g.org_repositories, 1),
                // Never asked: its permissions are more than the app asks for.
                "33" => Github::repositories("client-co", 1, 1),
                _ => Vec::new(),
            };
            let total = all.len();
            ok(json!({ "total_count": total, "repositories": slice(all) }))
        }
    }
}
