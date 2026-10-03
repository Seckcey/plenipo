//! Community's requests to 8 West's account service (Phase 24, ADR-162 §7, ADR-170): the carrier
//! for `plenipo_community::client`.
//!
//! Guard checks every address first (Community's purpose: only `account.getplenipo.com`'s
//! `/v1/community/` paths, or a test stand-in on this computer in a copy built for the tests). No
//! redirect is followed, no cookie is kept, and every answer is read up to the limit its request
//! names. The headers are exactly the contract's (§1): `Accept`, `User-Agent`, `Content-Type`
//! with a body, and `Authorization` with the pass. Nothing here logs a request, a pass, or an
//! answer. Plenipo calls this only while its owner has Community on (ADR-115, ADR-162 §5).

use std::time::Duration;

use plenipo_community::client::{Answer, Method, Request, Transport, BASE_PATH};
use plenipo_guard::{Guard, OutboundRules, Purpose};

/// How long Plenipo waits for an answer, beyond the time a pick-up asks the service to wait.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The carrier: Guard, its rules, and where the account service is.
#[derive(Clone)]
pub struct CommunityHttp {
    guard: Guard,
    rules: OutboundRules,
    /// `https://account.getplenipo.com`, or a stand-in's `http://127.0.0.1:<port>`.
    origin: String,
    client: reqwest::Client,
}

impl CommunityHttp {
    pub fn new(guard: Guard, rules: OutboundRules, origin: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Plenipo could not prepare Community's connection ({e})"))?;
        Ok(Self {
            guard,
            rules,
            origin: origin.trim_end_matches('/').to_owned(),
            client,
        })
    }

    /// The full address of a request.
    pub fn address(&self, request: &Request) -> String {
        format!("{}{BASE_PATH}{}", self.origin, request.path)
    }
}

fn words(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "8 West's service didn't answer in time".into()
    } else if e.is_connect() {
        "Plenipo couldn't reach 8 West (no internet, or it's blocked)".into()
    } else {
        "the request didn't go through".into()
    }
}

impl Transport for CommunityHttp {
    async fn send(&self, request: &Request, pass: Option<&str>) -> Result<Answer, String> {
        let address = self.address(request);
        self.guard
            .check_outbound(&self.rules, Purpose::Community, &address)?;
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self
            .client
            .request(method, &address)
            .timeout(TIMEOUT + Duration::from_secs(u64::from(request.held_secs)))
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = &request.body {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
        }
        if let (true, Some(pass)) = (request.with_pass, pass) {
            builder = builder.bearer_auth(pass);
        }
        let mut response = builder.send().await.map_err(|e| words(&e))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        if response
            .content_length()
            .is_some_and(|n| n > request.most_answer as u64)
        {
            return Err("8 West's answer was too long".into());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| words(&e))? {
            body.extend_from_slice(&chunk);
            if body.len() > request.most_answer {
                return Err("8 West's answer was too long".into());
            }
        }
        Ok(Answer {
            status,
            body,
            retry_after,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_community::client;
    use plenipo_community::session::{self, Opening};
    use plenipo_community::stand_in::StandIn;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    fn guard() -> Guard {
        Guard::new(Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap()))
    }

    fn http(guard: &Guard, origin: &str) -> CommunityHttp {
        let rules = OutboundRules::default().with_community_stand_in(Some(origin));
        CommunityHttp::new(guard.clone(), rules, origin).unwrap()
    }

    /// A server on this computer that records each request as it arrived and answers `reply`.
    async fn raw(reply: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let got = Arc::new(Mutex::new(Vec::new()));
        let seen = got.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = s.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                seen.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf).to_string());
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });
        (origin, got)
    }

    fn header_names(request: &str) -> Vec<String> {
        let (head, _) = request.split_once("\r\n\r\n").unwrap();
        let mut names: Vec<String> = head
            .lines()
            .skip(1)
            .filter_map(|l| {
                l.split_once(':')
                    .map(|(n, _)| n.trim().to_ascii_lowercase())
            })
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn is_it_open_sends_only_the_headers_http_needs_and_plenipos_name() {
        let (origin, got) = raw(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 37\r\n\
             Connection: close\r\n\r\n{\"links\":false,\"collaborators\":false}",
        )
        .await;
        let opening = session::check_open(&http(&guard(), &origin)).await;
        assert_eq!(
            opening,
            Opening::Open(plenipo_community::wire::Open {
                links: false,
                collaborators: false
            })
        );
        let request = got.lock().unwrap()[0].clone();
        assert!(request.starts_with("GET /v1/community/open HTTP/1.1"));
        assert_eq!(header_names(&request), ["accept", "host", "user-agent"]);
        assert!(request.contains("accept: application/json"));
        assert!(request.contains(concat!("Plenipo/", env!("CARGO_PKG_VERSION"))));
    }

    #[tokio::test]
    async fn a_pass_goes_only_in_authorization_and_a_body_says_its_type() {
        let (origin, got) =
            raw("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
        let pass = "p".repeat(43);
        let request = client::set_presence(&plenipo_community::wire::Presence {
            appear_offline: true,
        });
        let answer = http(&guard(), &origin)
            .send(&request, Some(&pass))
            .await
            .unwrap();
        assert_eq!(answer.status, 204);
        let sent = got.lock().unwrap()[0].clone();
        assert!(sent.starts_with("PUT /v1/community/me/presence HTTP/1.1"));
        assert_eq!(
            header_names(&sent),
            [
                "accept",
                "authorization",
                "content-length",
                "content-type",
                "host",
                "user-agent"
            ]
        );
        assert!(sent.contains(&format!("authorization: Bearer {pass}")));
        assert!(sent.ends_with(r#"{"appear_offline":true}"#));
        assert_eq!(
            sent.matches(&pass).count(),
            1,
            "the pass is in Authorization only"
        );
    }

    #[tokio::test]
    async fn no_pass_is_sent_with_a_request_that_carries_none() {
        let (origin, got) = raw(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\n\
             Content-Length: 58\r\nConnection: close\r\n\r\n\
             {\"error\":\"not_open\",\"message\":\"Community isn't open yet.\"}",
        )
        .await;
        let answer = client::send(&http(&guard(), &origin), &client::open(), Some("p")).await;
        let failure = client::read::<plenipo_community::wire::Open>(answer.unwrap(), 200);
        assert_eq!(
            failure.unwrap_err().code(),
            Some(&client::ErrorCode::NotOpen)
        );
        assert!(!got.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization"));
    }

    #[tokio::test]
    async fn a_redirect_is_never_followed() {
        let (origin, got) = raw(
            "HTTP/1.1 302 Found\r\nLocation: https://evil.example/\r\nContent-Length: 0\r\n\
             Connection: close\r\n\r\n",
        )
        .await;
        let opening = session::check_open(&http(&guard(), &origin)).await;
        assert_eq!(opening, Opening::Unreachable);
        assert_eq!(got.lock().unwrap().len(), 1, "nothing else was asked for");
    }

    #[tokio::test]
    async fn an_answer_longer_than_its_limit_is_refused() {
        let (origin, _) = raw(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 300000\r\n\
             Connection: close\r\n\r\n",
        )
        .await;
        let err = http(&guard(), &origin)
            .send(&client::me(), Some("p"))
            .await
            .unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    #[tokio::test]
    async fn guard_refuses_any_other_address_and_records_only_its_host() {
        let ledger = Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap());
        let guard = Guard::new(ledger.clone());
        // The rules know no stand-in, so only 8 West's own address would be allowed.
        let http =
            CommunityHttp::new(guard, OutboundRules::default(), "https://evil.example").unwrap();
        let err = http
            .send(&client::me(), Some("secret-pass"))
            .await
            .unwrap_err();
        assert!(err.contains("Community"), "{err}");
        let events = ledger.recent_events(10).unwrap();
        let refused: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "guard.request_refused")
            .collect();
        assert_eq!(refused.len(), 1);
        let payload = refused[0].payload.to_string();
        assert!(payload.contains("evil.example"));
        assert!(!payload.contains("secret-pass"), "never the pass");
    }

    #[tokio::test]
    async fn signing_in_through_the_stand_in_over_https_carrier() {
        let service = StandIn::new();
        let account = service.add_account("Frank Gonzalez", "frank@example.com", true);
        service.auto_allow(Some(account));
        let origin = service.serve().await;
        let http = http(&guard(), &origin);
        assert!(matches!(session::check_open(&http).await, Opening::Open(_)));
        let signing_in = session::start(&http, "FRANKIE-DESKTOP", "1.20.0", 1_790_000_000)
            .await
            .unwrap();
        let (finish, _) = session::finish(&http, signing_in).await.unwrap();
        let session::Finish::SignedIn(pc, me) = finish else {
            panic!("allowed at once");
        };
        assert_eq!(me.account.name, "Frank Gonzalez");
        let again = session::me(&http, &pc).await.unwrap();
        assert_eq!(again.device_id, pc.device_id());
        session::sign_out(&http, &pc).await.unwrap();
        // Over the wire, the pass was only ever in Authorization.
        for seen in service.seen() {
            assert!(!seen.path.contains(pc.pass()));
            assert!(!String::from_utf8_lossy(&seen.body).contains(pc.pass()));
        }
    }
}
