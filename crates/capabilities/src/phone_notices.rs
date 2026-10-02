//! A notice to your phone (Phase 14 part 14C, ADR-144): one sealed notice, sent to the phone's own
//! notice service (Apple's, Google's, Mozilla's, or Microsoft's).
//!
//! Guard checks the address first, for its purpose **phone notices**: only those services, or a
//! stand-in on this computer in a copy built for the tests. The notice is already sealed for the
//! phone and signed with the PC's notice key (`plenipo-remote`); this only carries it. No redirect
//! is followed, no cookie is kept, and the service's answer is read only for its status.

use std::time::Duration;

use plenipo_guard::{Guard, OutboundRules, Purpose};

/// How long one notice may take.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// What the notice service said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It took the notice.
    Sent,
    /// The phone's notice address is gone (the phone turned notices off, or its browser forgot).
    Gone,
    /// It did not go, in plain words.
    Failed(String),
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Plenipo could not prepare the notice ({e})"))
}

/// Send the sealed notice `body` to `endpoint`, with its `headers` (sealing, signature, how long to
/// keep it).
pub async fn post(
    guard: &Guard,
    rules: &OutboundRules,
    endpoint: &str,
    headers: &[(&str, String)],
    body: Vec<u8>,
) -> Outcome {
    if let Err(why) = guard.check_outbound(rules, Purpose::PhoneNotices, endpoint) {
        return Outcome::Failed(why);
    }
    let client = match client() {
        Ok(c) => c,
        Err(why) => return Outcome::Failed(why),
    };
    let mut request = client.post(endpoint).body(body);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    match request.send().await {
        Ok(response) => match response.status().as_u16() {
            200..=299 => Outcome::Sent,
            404 | 410 => Outcome::Gone,
            413 => Outcome::Failed("the notice was too long for the notice service".into()),
            429 => Outcome::Failed("the notice service asked Plenipo to slow down".into()),
            status => Outcome::Failed(format!("the notice service answered {status}")),
        },
        Err(e) if e.is_timeout() => {
            Outcome::Failed("the notice service didn't answer in time".into())
        }
        Err(e) if e.is_connect() => {
            Outcome::Failed("Plenipo couldn't reach the notice service".into())
        }
        Err(_) => Outcome::Failed("the notice didn't go through".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    /// A stand-in notice service on this computer: records each request, and answers `reply`.
    async fn stand_in(reply: &'static str) -> (u16, Arc<Mutex<Vec<Vec<u8>>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
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
                    let head_end = buf.windows(4).position(|w| w == b"\r\n\r\n");
                    if let Some(end) = head_end {
                        let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                seen.lock().unwrap().push(buf);
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });
        (port, got)
    }

    fn guard() -> Guard {
        Guard::new(Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap()))
    }

    #[tokio::test]
    async fn a_notice_goes_sealed_with_its_headers_and_a_gone_address_says_so() {
        let (port, got) = stand_in("HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n").await;
        let base = format!("http://127.0.0.1:{port}");
        let rules = OutboundRules::default().with_notices_stand_in(Some(&base));
        let headers = [
            ("Content-Encoding", "aes128gcm".to_owned()),
            ("TTL", "600".to_owned()),
            ("Urgency", "high".to_owned()),
            ("Authorization", "vapid t=a.b.c, k=key".to_owned()),
        ];
        let sealed = vec![0xAB; 120];
        let outcome = post(
            &guard(),
            &rules,
            &format!("{base}/push/phone-1"),
            &headers,
            sealed.clone(),
        )
        .await;
        assert_eq!(outcome, Outcome::Sent);
        let request = got.lock().unwrap()[0].clone();
        let end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&request[..end]).to_string();
        assert!(head.starts_with("POST /push/phone-1 HTTP/1.1"), "{head}");
        for (name, value) in &headers {
            assert!(
                head.to_ascii_lowercase().contains(&format!(
                    "{}: {}",
                    name.to_ascii_lowercase(),
                    value.to_ascii_lowercase()
                )),
                "{name} in {head}"
            );
        }
        assert_eq!(
            &request[end + 4..],
            sealed.as_slice(),
            "the sealed notice as it is"
        );
        assert!(!head.to_ascii_lowercase().contains("cookie"));

        // The notice service says the phone's address is gone.
        let (port, _) = stand_in("HTTP/1.1 410 Gone\r\nContent-Length: 0\r\n\r\n").await;
        let base = format!("http://127.0.0.1:{port}");
        let rules = OutboundRules::default().with_notices_stand_in(Some(&base));
        let outcome = post(&guard(), &rules, &format!("{base}/push/x"), &[], vec![1]).await;
        assert_eq!(outcome, Outcome::Gone);
    }

    #[tokio::test]
    async fn only_a_phones_notice_service_is_reached() {
        let rules = OutboundRules::default();
        for address in [
            "https://example.com/push",
            "http://fcm.googleapis.com/fcm/send/x",
            "https://fcm.googleapis.com:8443/fcm/send/x",
            "http://127.0.0.1:9/push",
            "https://user:pw@web.push.apple.com/x",
        ] {
            let outcome = post(&guard(), &rules, address, &[], vec![1]).await;
            assert!(
                matches!(outcome, Outcome::Failed(_)),
                "{address}: {outcome:?}"
            );
        }
    }
}
