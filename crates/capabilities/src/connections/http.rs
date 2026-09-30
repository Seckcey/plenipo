//! Plenipo's own web requests for a connection (Phase 20, ADR-062 §8): every hop checked by
//! Guard's gate for Plenipo's own requests first — only that service's addresses, only `https`
//! (or, in a copy built for the tests, the stand-in on this computer) — with no redirect followed
//! on its own. The access token is added here, and only to the service's own API host: never to
//! a sign-in page, a download address, or anywhere a redirect points.

use std::time::Duration;

use plenipo_guard::{Guard, OutboundRules, Purpose, Service};
use serde_json::Value;

/// Redirects followed (each checked by Guard).
const MAX_REDIRECTS: usize = 5;
/// The longest a service may ask Plenipo to wait before trying once more ("too many requests").
pub const MAX_RETRY_WAIT: Duration = Duration::from_secs(30);

/// The hosts that get the access token or key: the service's API, nothing else. (The website's
/// is the address saved on its card, ADR-071 §4.)
pub(crate) fn api_hosts(service: Service) -> &'static [&'static str] {
    match service {
        Service::Microsoft365 => &["graph.microsoft.com"],
        Service::Slack => &["slack.com"],
        Service::Google => &["gmail.googleapis.com", "www.googleapis.com"],
        Service::Hubspot => &["api.hubapi.com"],
        Service::Stripe => &["api.stripe.com"],
        Service::Wordpress => &[],
    }
}

/// How a request proves who it is for: an access token or a key (`Authorization: Bearer`), or
/// a user name and password (`Authorization: Basic`: a WordPress Application Password, or a
/// WooCommerce key and its secret).
#[derive(Clone, Copy)]
pub(crate) enum Auth<'a> {
    Bearer(&'a str),
    Basic { user: &'a str, password: &'a str },
}

/// What a request carries.
#[derive(Clone)]
pub(crate) enum Body {
    None,
    /// No body at all, sent as `Content-Length: 0` (Microsoft asks for it on some actions).
    Empty,
    Json(Value),
    /// A form (sign-in token requests).
    Form(Vec<(String, String)>),
    /// A file's bytes.
    Bytes {
        content_type: &'static str,
        data: Vec<u8>,
    },
}

/// A service's answer.
#[derive(Debug)]
pub(crate) struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The answer as JSON (`Null` when it is not).
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

/// Why a request got no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HttpError {
    /// Guard's gate refused an address (the reason, in plain words).
    Refused(String),
    /// The network or the service failed before the request reached it (plain words).
    Network(String),
    /// The request may have reached the service, but no whole answer came back (plain words):
    /// a change may have been made.
    NoAnswer(String),
    /// The answer was bigger than allowed.
    TooBig,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(why) | Self::Network(why) | Self::NoAnswer(why) => f.write_str(why),
            Self::TooBig => f.write_str("the answer was too big"),
        }
    }
}

/// A form's fields, encoded (`application/x-www-form-urlencoded`).
fn form(fields: &[(String, String)]) -> String {
    let enc = |v: &str| {
        let mut out = String::with_capacity(v.len());
        for b in v.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else if b == b' ' {
                out.push('+');
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    };
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// The client for one copy of Plenipo: the real services, or the stand-in it was built with.
#[derive(Clone)]
pub(crate) struct Http {
    guard: Guard,
    rules: OutboundRules,
    /// `http://127.0.0.1:<port>` in a copy built for the tests (never a setting).
    stand_in: Option<String>,
    client: reqwest::Client,
}

impl Http {
    pub fn new(guard: Guard, stand_in: Option<String>) -> Self {
        let rules = OutboundRules::default().with_connections_stand_in(stand_in.as_deref());
        // A stand-in that is not on this computer is never used.
        let stand_in = stand_in.filter(|_| rules.connections_test_port.is_some());
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self {
            guard,
            rules,
            stand_in,
            client,
        }
    }

    pub fn guard(&self) -> &Guard {
        &self.guard
    }

    /// The address as this copy reaches it: the real one, or the stand-in's
    /// (`http://127.0.0.1:<port>/<host><path>`).
    pub fn address(&self, url: &str) -> String {
        match (&self.stand_in, url.strip_prefix("https://")) {
            (Some(base), Some(rest)) => format!("{base}/{rest}"),
            _ => url.to_owned(),
        }
    }

    /// The real host an address (maybe the stand-in's) is for.
    pub fn real_host(&self, address: &str) -> Option<String> {
        if let Some(rest) = self
            .stand_in
            .as_deref()
            .and_then(|base| address.strip_prefix(base))
        {
            let rest = rest.trim_start_matches('/');
            return Some(rest[..rest.find('/').unwrap_or(rest.len())].to_owned());
        }
        reqwest::Url::parse(address)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
    }

    /// Check an address with Guard's gate for `service` (a refusal is recorded by Guard).
    pub fn check(&self, service: Service, address: &str) -> Result<(), HttpError> {
        self.guard
            .check_outbound(&self.rules, Purpose::Connection(service), address)
            .map(|_| ())
            .map_err(HttpError::Refused)
    }

    /// Send one request for `service` to `url` (a real address; the stand-in is used when this
    /// copy was built with one), reading at most `limit` bytes of the answer. `bearer`: the
    /// access token, added only for the service's API host. A redirect is followed with `GET`
    /// and without the token, each hop checked. A service that answers "too many requests" (or
    /// "unavailable") with a wait of at most [`MAX_RETRY_WAIT`] is tried once more after it.
    #[allow(clippy::too_many_arguments)]
    pub async fn send(
        &self,
        service: Service,
        method: reqwest::Method,
        url: &str,
        bearer: Option<&str>,
        headers: &[(&'static str, &str)],
        body: Body,
        limit: usize,
    ) -> Result<Reply, HttpError> {
        self.send_with(
            service,
            method,
            url,
            bearer.map(Auth::Bearer),
            headers,
            body,
            limit,
            true,
        )
        .await
    }

    /// As [`Http::send`], with `auth` of either kind, and `retry: false` for a request that must
    /// never be sent twice (a store refund, which has no way to tell a repeat, ADR-071 §6.5).
    #[allow(clippy::too_many_arguments)]
    pub async fn send_with(
        &self,
        service: Service,
        method: reqwest::Method,
        url: &str,
        auth: Option<Auth<'_>>,
        headers: &[(&'static str, &str)],
        body: Body,
        limit: usize,
        retry: bool,
    ) -> Result<Reply, HttpError> {
        let reply = self
            .send_once(service, method.clone(), url, auth, headers, &body, limit)
            .await?;
        if retry && matches!(reply.0.status, 429 | 503) {
            if let Some(wait) = reply.1.filter(|w| *w <= MAX_RETRY_WAIT) {
                tokio::time::sleep(wait).await;
                return Ok(self
                    .send_once(service, method, url, auth, headers, &body, limit)
                    .await?
                    .0);
            }
        }
        Ok(reply.0)
    }

    /// Whether `host` is the service's API host, the only one that gets its token or key: for
    /// the website, the host of the address saved on its card, read afresh.
    fn api_host(&self, service: Service, host: &str) -> bool {
        match service {
            Service::Wordpress => self
                .guard
                .config()
                .ok()
                .and_then(|c| c.connection("wordpress").and_then(|w| w.site_host()))
                .is_some_and(|h| h == host),
            _ => api_hosts(service).contains(&host),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_once(
        &self,
        service: Service,
        method: reqwest::Method,
        url: &str,
        auth: Option<Auth<'_>>,
        headers: &[(&'static str, &str)],
        body: &Body,
        limit: usize,
    ) -> Result<(Reply, Option<Duration>), HttpError> {
        let mut address = self.address(url);
        let mut method = method;
        let mut first = true;
        for _ in 0..=MAX_REDIRECTS {
            self.check(service, &address)?;
            let host = self.real_host(&address).unwrap_or_default();
            let mut request = self.client.request(method.clone(), &address);
            // Only for the service's own API host (the website: only its saved host).
            if let Some(auth) = auth.filter(|_| self.api_host(service, &host)) {
                request = match auth {
                    Auth::Bearer(token) => request.bearer_auth(token),
                    Auth::Basic { user, password } => request.basic_auth(user, Some(password)),
                };
            }
            if first {
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }
                request = match body {
                    Body::None => request,
                    Body::Empty => request
                        .header(reqwest::header::CONTENT_LENGTH, "0")
                        .body(Vec::<u8>::new()),
                    Body::Json(v) => request
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .body(serde_json::to_vec(v).unwrap_or_default()),
                    Body::Form(fields) => request
                        .header(
                            reqwest::header::CONTENT_TYPE,
                            "application/x-www-form-urlencoded",
                        )
                        .body(form(fields)),
                    Body::Bytes { content_type, data } => request
                        .header(reqwest::header::CONTENT_TYPE, *content_type)
                        .body(data.clone()),
                };
            }
            let response = request.send().await.map_err(|e| {
                if e.is_connect() {
                    HttpError::Network(format!("Plenipo could not reach {}", service.label()))
                } else if e.is_timeout() {
                    HttpError::NoAnswer(format!("{} took too long to answer", service.label()))
                } else {
                    HttpError::NoAnswer(format!("{}'s answer was lost on the way", service.label()))
                }
            })?;
            let status = response.status();
            if status.is_redirection() {
                let next = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|l| l.to_str().ok())
                    .ok_or_else(|| {
                        HttpError::Network(format!("{} sent Plenipo nowhere", service.label()))
                    })?;
                let base =
                    reqwest::Url::parse(&address).map_err(|e| HttpError::Network(e.to_string()))?;
                let joined = base
                    .join(next)
                    .map_err(|e| HttpError::Network(e.to_string()))?
                    .to_string();
                address = if joined.starts_with("https://") {
                    self.address(&joined)
                } else {
                    joined
                };
                method = reqwest::Method::GET;
                first = false;
                continue;
            }
            let retry = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            if response.content_length().is_some_and(|n| n > limit as u64) {
                return Err(HttpError::TooBig);
            }
            let mut data = Vec::new();
            let mut response = response;
            while let Some(chunk) = response.chunk().await.map_err(|_| {
                HttpError::NoAnswer(format!("{}'s answer was cut off", service.label()))
            })? {
                data.extend_from_slice(&chunk);
                if data.len() > limit {
                    return Err(HttpError::TooBig);
                }
            }
            return Ok((
                Reply {
                    status: status.as_u16(),
                    body: data,
                },
                retry,
            ));
        }
        Err(HttpError::Network(format!(
            "{} sent Plenipo round in circles",
            service.label()
        )))
    }
}
