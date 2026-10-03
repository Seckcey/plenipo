//! Plenipo's own updates (Phase 13, ADR-038), on this computer only: a small test server plays
//! GitHub, and a throwaway key plays 8 West's updater key. No internet.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;
use std::sync::Arc;

use base64::Engine as _;
use plenipo_capabilities::updates::{self, Release, UpdateError, UpdateSource};
use plenipo_guard::Guard;
use plenipo_ledger::Ledger;

/// One answer of the test server: status line, extra headers, body.
type Route = (&'static str, Vec<(String, String)>, Vec<u8>);

/// Serve the routes `routes(port)` gives on 127.0.0.1 until the test ends. Returns the port.
fn serve(routes: impl FnOnce(u16) -> HashMap<String, Route>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let routes = Arc::new(routes(port));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let routes = Arc::clone(&routes);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap_or_default();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let (status, headers, body) =
                    routes
                        .get(&path)
                        .cloned()
                        .unwrap_or(("404 Not Found", vec![], b"no".to_vec()));
                let mut head = format!(
                    "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n",
                    body.len()
                );
                for (k, v) in headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            });
        }
    });
    port
}

fn b64(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

/// A throwaway updater key: (public key as Tauri stores it, signer).
fn key() -> (String, minisign::KeyPair) {
    let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let public = b64(&pair.pk.to_box().unwrap().to_string());
    (public, pair)
}

/// Sign `data` as Tauri's signing tool does, saying `version` inside the signature.
fn sign(pair: &minisign::KeyPair, data: &[u8], version: Option<&str>) -> String {
    let trusted = match version {
        Some(v) => format!("timestamp:1790000000\tfile:Plenipo_{v}_x64-setup.exe\tversion:{v}"),
        None => "timestamp:1790000000\tfile:Plenipo_x64-setup.exe".to_owned(),
    };
    let sig = minisign::sign(Some(&pair.pk), &pair.sk, data, Some(&trusted), None).unwrap();
    b64(&sig.to_string())
}

/// A `latest.json` with a download for the system the tests run on (Phase 23: each system looks
/// for its own).
fn manifest(version: &str, url: &str, signature: &str) -> Vec<u8> {
    let mut platforms = serde_json::Map::new();
    platforms.insert(
        updates::PLATFORM.to_owned(),
        serde_json::json!({ "signature": signature, "url": url }),
    );
    serde_json::to_vec(&serde_json::json!({
        "version": version,
        "notes": "Fixes and safety.",
        "pub_date": "2026-10-01T12:00:00Z",
        "platforms": platforms,
    }))
    .unwrap()
}

fn guard() -> (Arc<Ledger>, Guard) {
    let ledger = Arc::new(Ledger::open_in_memory().unwrap());
    let guard = Guard::new(ledger.clone());
    (ledger, guard)
}

const INSTALLER: &[u8] = b"MZ pretend installer for version 9.9.9";

fn source(port: u16, public_key: Option<String>) -> UpdateSource {
    UpdateSource {
        endpoint: format!("http://127.0.0.1:{port}/latest.json"),
        public_key,
    }
}

#[tokio::test]
async fn a_signed_newer_release_is_found_downloaded_and_checked() {
    let (public, pair) = key();
    let routes = |port: u16| {
        let sig = sign(&pair, INSTALLER, Some("9.9.9"));
        HashMap::from([
            (
                "/latest.json".to_owned(),
                (
                    "200 OK",
                    vec![],
                    manifest("9.9.9", &format!("http://127.0.0.1:{port}/redirect"), &sig),
                ),
            ),
            // Like GitHub, the download is sent on to another address.
            (
                "/redirect".to_owned(),
                (
                    "302 Found",
                    vec![("location".into(), "/Plenipo_9.9.9_x64-setup.exe".into())],
                    vec![],
                ),
            ),
            (
                "/Plenipo_9.9.9_x64-setup.exe".to_owned(),
                ("200 OK", vec![], INSTALLER.to_vec()),
            ),
        ])
    };
    let port = serve(routes);
    let (_ledger, guard) = guard();
    let source = source(port, Some(public));
    let release = updates::check(&guard, &source, "1.9.0")
        .await
        .unwrap()
        .expect("9.9.9 is newer");
    assert_eq!(release.version, "9.9.9");
    assert_eq!(release.notes, "Fixes and safety.");
    let bytes = updates::download(&guard, &source, &release).await.unwrap();
    assert_eq!(bytes, INSTALLER);
    // The same version is not offered again.
    assert!(updates::check(&guard, &source, "9.9.9")
        .await
        .unwrap()
        .is_none());
}

fn release(url: String, signature: String, version: &str) -> Release {
    Release {
        version: version.into(),
        notes: String::new(),
        pub_date: None,
        url,
        signature,
    }
}

#[tokio::test]
async fn a_download_that_is_not_ours_or_not_the_version_it_claims_is_refused() {
    let (public, pair) = key();
    let (_, other) = key();
    let tampered = b"MZ something else entirely".to_vec();
    let port = serve(|_| {
        HashMap::from([
            (
                "/installer.exe".to_owned(),
                ("200 OK", vec![], INSTALLER.to_vec()),
            ),
            ("/tampered.exe".to_owned(), ("200 OK", vec![], tampered)),
        ])
    });
    let (_ledger, guard) = guard();
    let source = source(port, Some(public));
    let url = |name: &str| format!("http://127.0.0.1:{port}/{name}");
    let refused = |r: Result<Vec<u8>, UpdateError>| match r {
        Err(UpdateError::Signature(why)) => why,
        other => panic!("expected a refused signature, got {other:?}"),
    };
    // Changed after signing.
    let why = refused(
        updates::download(
            &guard,
            &source,
            &release(
                url("tampered.exe"),
                sign(&pair, INSTALLER, Some("9.9.9")),
                "9.9.9",
            ),
        )
        .await,
    );
    assert!(
        why.contains("not signed with 8 West's updater key"),
        "{why}"
    );
    // Signed with another key.
    let why = refused(
        updates::download(
            &guard,
            &source,
            &release(
                url("installer.exe"),
                sign(&other, INSTALLER, Some("9.9.9")),
                "9.9.9",
            ),
        )
        .await,
    );
    assert!(
        why.contains("not signed with 8 West's updater key"),
        "{why}"
    );
    // A genuine older release presented as a new version.
    let why = refused(
        updates::download(
            &guard,
            &source,
            &release(
                url("installer.exe"),
                sign(&pair, INSTALLER, Some("1.8.0")),
                "9.9.9",
            ),
        )
        .await,
    );
    assert!(why.contains("signed as version 1.8.0"), "{why}");
    // A signature that does not say its version.
    let why = refused(
        updates::download(
            &guard,
            &source,
            &release(url("installer.exe"), sign(&pair, INSTALLER, None), "9.9.9"),
        )
        .await,
    );
    assert!(why.contains("does not say which version"), "{why}");
    // A copy of Plenipo with no updater key installs nothing.
    let unkeyed = UpdateSource {
        public_key: None,
        ..source.clone()
    };
    let why = refused(
        updates::download(
            &guard,
            &unkeyed,
            &release(
                url("installer.exe"),
                sign(&pair, INSTALLER, Some("9.9.9")),
                "9.9.9",
            ),
        )
        .await,
    );
    assert!(why.contains("no updater key"), "{why}");
}

#[tokio::test]
async fn guard_refuses_any_address_but_the_releases_and_records_it() {
    let (public, pair) = key();
    let sig = sign(&pair, INSTALLER, Some("9.9.9"));
    let port = serve(|_| {
        HashMap::from([
            (
                "/latest.json".to_owned(),
                (
                    "200 OK",
                    vec![],
                    manifest("9.9.9", "https://evil.example/Plenipo.exe", &sig),
                ),
            ),
            (
                "/away".to_owned(),
                (
                    "302 Found",
                    vec![("location".into(), "https://evil.example/x".into())],
                    vec![],
                ),
            ),
        ])
    });
    let (ledger, guard) = guard();
    let source = source(port, Some(public));
    // The release description names an installer somewhere else: refused before any request.
    let release = updates::check(&guard, &source, "1.9.0")
        .await
        .unwrap()
        .unwrap();
    let err = updates::download(&guard, &source, &release)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Refused(_)), "{err:?}");
    assert!(err.to_string().contains("evil.example"));
    // A redirect elsewhere is refused too.
    let away = self::release(format!("http://127.0.0.1:{port}/away"), sig, "9.9.9");
    let err = updates::download(&guard, &source, &away).await.unwrap_err();
    assert!(matches!(err, UpdateError::Refused(_)), "{err:?}");
    // Both were recorded.
    let refused = ledger
        .events_of_types(&["guard.request_refused"], 10)
        .unwrap();
    assert_eq!(refused.len(), 2);
    // Another port on this computer is not the test server either.
    let other = UpdateSource {
        endpoint: format!("http://127.0.0.1:{port}/latest.json"),
        public_key: None,
    };
    let rules = other.rules();
    assert!(rules
        .check(
            plenipo_guard::Purpose::Updates,
            &format!("http://127.0.0.1:{}/latest.json", port.wrapping_add(1))
        )
        .is_err());
}

#[tokio::test]
async fn no_internet_or_a_broken_description_changes_nothing() {
    let port = serve(|_| {
        HashMap::from([(
            "/latest.json".to_owned(),
            ("200 OK", vec![], b"{ this is not json".to_vec()),
        )])
    });
    let (_ledger, guard) = guard();
    let err = updates::check(&guard, &source(port, None), "1.9.0")
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Manifest(_)), "{err:?}");
    // Nothing listening: a plain network failure.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = closed.local_addr().unwrap().port();
    drop(closed);
    let err = updates::check(&guard, &source(dead, None), "1.9.0")
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Network(_)), "{err:?}");
    // No release yet.
    let empty = serve(|_| HashMap::new());
    let err = updates::check(&guard, &source(empty, None), "1.9.0")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no release"), "{err}");
}

#[test]
fn versions_compare_and_pre_releases_are_never_offered() {
    assert!(updates::is_newer("1.9.0", "1.10.0"));
    assert!(updates::is_newer("1.9.0", "1.9.1"));
    assert!(!updates::is_newer("1.9.0", "1.9.0"));
    assert!(!updates::is_newer("1.10.0", "1.9.0"));
    assert!(!updates::is_newer("1.9.0", "2.0.0-beta.1"));
    assert!(!updates::is_newer("1.9.0", "not a version"));
    let r = updates::parse_manifest(&manifest("v1.10.0", "https://x/y", "s"))
        .unwrap()
        .expect("a download for this system");
    assert_eq!(r.version, "1.10.0");
    // A release with no download for this system is not offered (Phase 23): no error.
    let none = updates::parse_manifest(br#"{"version":"1.10.0","platforms":{}}"#).unwrap();
    assert!(none.is_none());
    let elsewhere =
        br#"{"version":"1.10.0","platforms":{"other-system":{"signature":"s","url":"u"}}}"#;
    assert!(updates::parse_manifest(elsewhere).unwrap().is_none());
}
