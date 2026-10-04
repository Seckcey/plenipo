//! The weekly license check's one request (Phase 11A, ADR-022, ADR-115, ADR-116): a Pro copy
//! sends its key ID and app version to 8 West, and reads the signed answer.
//!
//! Guard checks the address first (the license check's one address, or a test stand-in on this
//! computer in a copy built for the tests). No redirect is followed, no cookie is kept, and the
//! answer is read up to a small limit. What the answer means is decided by `plenipo-licensing`;
//! this only carries it. A Free copy never calls this.

use std::time::Duration;

use plenipo_guard::{Guard, OutboundRules, Purpose};

/// The largest answer read.
pub const MAX_ANSWER_BYTES: usize = plenipo_licensing::answer::MAX_ANSWER_BYTES;
/// How long a check may take.
pub const TIMEOUT: Duration = Duration::from_secs(30);

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Plenipo could not prepare the check ({e})"))
}

fn words(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "8 West's service didn't answer in time".into()
    } else if e.is_connect() {
        "Plenipo couldn't reach 8 West (no internet, or it's blocked)".into()
    } else {
        "the check didn't go through".into()
    }
}

/// Send `body` (exactly the key ID and the app version) to `address`, and return the answer's
/// body when 8 West answered `200`. Anything else is a failed check, in plain words.
pub async fn post(
    guard: &Guard,
    rules: &OutboundRules,
    address: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>, String> {
    guard.check_outbound(rules, Purpose::License, address)?;
    let response = client()?
        .post(address)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| words(&e))?;
    let status = response.status();
    if status != reqwest::StatusCode::OK {
        return Err(format!("8 West's service answered {}", status.as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_ANSWER_BYTES as u64)
    {
        return Err("8 West's answer was too long".into());
    }
    let mut answer = Vec::new();
    let mut response = response;
    while let Some(chunk) = response.chunk().await.map_err(|e| words(&e))? {
        answer.extend_from_slice(&chunk);
        if answer.len() > MAX_ANSWER_BYTES {
            return Err("8 West's answer was too long".into());
        }
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    /// A stand-in on this computer: records each request, and answers `reply`. Each connection
    /// is served on its own, so a slow one never holds up the next.
    async fn stand_in(reply: &'static str) -> (u16, Arc<Mutex<Vec<Vec<u8>>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let got = Arc::new(Mutex::new(Vec::new()));
        let seen = got.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    // Read the head, then the body its Content-Length gives.
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
                    seen.lock().unwrap().push(buf);
                    let _ = s.write_all(reply.as_bytes()).await;
                });
            }
        });
        (port, got)
    }

    /// The requests a stand-in got whose first line is `line` (`POST /v1/check HTTP/1.1`). A
    /// request is found by its first line, never taken to be the first to arrive: on a busy
    /// computer another program can reach a test's stand-in too (a port it freed and then
    /// connected to can be handed to the stand-in in between).
    fn with_first_line(got: &Mutex<Vec<Vec<u8>>>, line: &str) -> Vec<Vec<u8>> {
        let want = format!("{line}\r\n");
        got.lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with(want.as_bytes()))
            .cloned()
            .collect()
    }

    fn guard() -> Guard {
        Guard::new(Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap()))
    }

    #[tokio::test]
    async fn the_check_sends_exactly_the_key_id_and_the_app_version() {
        let (port, got) = stand_in(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\
             Connection: close\r\n\r\n{}",
        )
        .await;
        let base = format!("http://127.0.0.1:{port}");
        let rules = OutboundRules::default().with_license_stand_in(Some(&base));
        let body =
            plenipo_licensing::answer::request_body("lk_01J9XW3T5B8K2M4N6P7Q8R9S0T", "1.18.0");
        let answer = post(&guard(), &rules, &format!("{base}/v1/check"), body.clone())
            .await
            .unwrap();
        assert_eq!(answer, b"{}");
        let checks = with_first_line(&got, "POST /v1/check HTTP/1.1");
        assert_eq!(checks.len(), 1, "exactly one check was sent");
        let text = String::from_utf8(checks[0].clone()).unwrap();
        let (head, sent) = text.split_once("\r\n\r\n").unwrap();
        // Byte for byte: the key ID and the app version, and nothing else.
        assert_eq!(
            sent.as_bytes(),
            br#"{"key_id":"lk_01J9XW3T5B8K2M4N6P7Q8R9S0T","app_version":"1.18.0"}"#
        );
        assert_eq!(sent.as_bytes(), body.as_slice());
        // Only the headers HTTP itself needs, the content type, and Plenipo's name.
        let mut names: Vec<String> = head
            .lines()
            .skip(1)
            .filter_map(|l| {
                l.split_once(':')
                    .map(|(n, _)| n.trim().to_ascii_lowercase())
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "accept",
                "content-length",
                "content-type",
                "host",
                "user-agent"
            ]
        );
        assert!(head.starts_with("POST /v1/check HTTP/1.1"));
        assert!(head.contains(concat!("Plenipo/", env!("CARGO_PKG_VERSION"))));
        assert!(!head.to_ascii_lowercase().contains("cookie"));
        assert!(!head.to_ascii_lowercase().contains("authorization"));
    }

    #[tokio::test]
    async fn another_status_or_a_redirect_is_a_failed_check() {
        for (reply, why) in [
            (
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "answered 500",
            ),
            (
                "HTTP/1.1 302 Found\r\nLocation: https://evil.example/\r\nContent-Length: 0\r\n\
                 Connection: close\r\n\r\n",
                "answered 302",
            ),
        ] {
            let (port, got) = stand_in(reply).await;
            let base = format!("http://127.0.0.1:{port}");
            let rules = OutboundRules::default().with_license_stand_in(Some(&base));
            let err = post(&guard(), &rules, &format!("{base}/v1/check"), b"{}".to_vec())
                .await
                .unwrap_err();
            assert!(err.contains(why), "{err}");
            // Nothing else was asked for (no redirect followed).
            assert_eq!(with_first_line(&got, "POST /v1/check HTTP/1.1").len(), 1);
        }
    }

    #[tokio::test]
    async fn a_long_answer_is_refused() {
        let reply: &'static str = Box::leak(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n{}",
                "x".repeat(MAX_ANSWER_BYTES + 1),
                n = MAX_ANSWER_BYTES + 1
            )
            .into_boxed_str(),
        );
        let (port, _) = stand_in(reply).await;
        let base = format!("http://127.0.0.1:{port}");
        let rules = OutboundRules::default().with_license_stand_in(Some(&base));
        let err = post(
            &guard(),
            &rules,
            &format!("{base}/v1/check"),
            b"{}".to_vec(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    #[tokio::test]
    async fn guard_refuses_any_other_address_before_anything_is_sent() {
        let (port, got) = stand_in("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await;
        // No stand-in in the rules: a released copy.
        let err = post(
            &guard(),
            &OutboundRules::default(),
            &format!("http://127.0.0.1:{port}/v1/check"),
            b"{}".to_vec(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("the weekly license check"), "{err}");
        assert!(got.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_internet_is_a_failed_check_in_plain_words() {
        // Port 0: no program can ever listen there, and the system never hands it out, so the
        // connection fails at once on every system without trying the network. A port the test
        // freed for this would not do: on a busy computer the system can hand it to another
        // test's stand-in before the check connects, and the check then reaches that stand-in
        // (it did on 2026-10-04).
        let base = "http://127.0.0.1:0".to_owned();
        let rules = OutboundRules::default().with_license_stand_in(Some(&base));
        let err = post(
            &guard(),
            &rules,
            &format!("{base}/v1/check"),
            b"{}".to_vec(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("couldn't reach 8 West"), "{err}");
    }
}
