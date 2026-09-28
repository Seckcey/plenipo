//! Plenipo's own updates (Phase 13, ADR-038): the one place Plenipo reaches the internet for
//! itself. Every request is Plenipo's own purpose ("checking for updates") and goes through
//! Guard first ([`plenipo_guard::outbound`]): only GitHub's release addresses, only over
//! `https`, and every redirect checked again. A download is used only when:
//!
//! 1. its signature matches the updater key's public half built into this copy of Plenipo, and
//! 2. the version written **inside** the signature equals the version the release announced,
//!    so an old release cannot be dressed up as a new one.
//!
//! The release's `latest.json` has the layout of Tauri's updater, so the Release workflow uses
//! Tauri's own signing tool.

use std::time::Duration;

use base64::Engine as _;
use plenipo_guard::{Guard, OutboundRules, Purpose};
use serde::Deserialize;

/// Where released copies of Plenipo look for a new version.
pub const RELEASES_ENDPOINT: &str =
    "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json";
/// Where the owner downloads versions by hand.
pub const RELEASES_PAGE: &str = "https://github.com/Seckcey/plenipo/releases";
/// The platform this copy installs for, as `latest.json` names it.
pub const PLATFORM: &str = "windows-x86_64";
/// The most a release description may hold.
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
/// The most an installer may be.
const MAX_INSTALLER_BYTES: usize = 400 * 1024 * 1024;
/// Redirects followed (each checked by Guard).
const MAX_REDIRECTS: usize = 5;

/// Where this copy of Plenipo gets updates from, and the key it trusts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSource {
    /// The address of `latest.json`.
    pub endpoint: String,
    /// The updater key's public half (Tauri's form: the key file's text, in base64). `None`:
    /// this copy was not built by the Release workflow and cannot install updates.
    pub public_key: Option<String>,
}

impl UpdateSource {
    pub fn rules(&self) -> OutboundRules {
        OutboundRules::for_endpoint(&self.endpoint)
    }
}

/// A newer release, as `latest.json` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub pub_date: Option<String>,
    /// Where its installer is.
    pub url: String,
    /// Its updater signature (base64, as Tauri writes it).
    pub signature: String,
}

/// What went wrong, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// Guard refused an address.
    #[error("{0}")]
    Refused(String),
    /// The internet, GitHub, or the file did not answer as expected.
    #[error("{0}")]
    Network(String),
    /// The release description could not be read.
    #[error("the release description could not be read: {0}")]
    Manifest(String),
    /// The download is not one 8 West signed, or not the version it claims.
    #[error("the download did not pass its check: {0}")]
    Signature(String),
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    signature: String,
    url: String,
}

/// Read a `latest.json` for this platform.
pub fn parse_manifest(bytes: &[u8]) -> Result<Release, UpdateError> {
    let m: Manifest =
        serde_json::from_slice(bytes).map_err(|e| UpdateError::Manifest(e.to_string()))?;
    let version = m.version.trim().trim_start_matches('v').to_owned();
    semver::Version::parse(&version).map_err(|e| {
        UpdateError::Manifest(format!("its version {version:?} is not valid ({e})"))
    })?;
    let p = m
        .platforms
        .get(PLATFORM)
        .ok_or_else(|| UpdateError::Manifest(format!("it has no installer for {PLATFORM}")))?;
    Ok(Release {
        version,
        notes: m.notes.unwrap_or_default(),
        pub_date: m.pub_date,
        url: p.url.clone(),
        signature: p.signature.clone(),
    })
}

/// `candidate` is newer than `current` (both SemVer). Pre-releases are never offered.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (
        semver::Version::parse(current),
        semver::Version::parse(candidate),
    ) {
        (Ok(c), Ok(n)) => n.pre.is_empty() && n > c,
        _ => false,
    }
}

fn b64_text(what: &str, b64: &str) -> Result<String, UpdateError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| UpdateError::Signature(format!("the {what} is not readable ({e})")))?;
    String::from_utf8(bytes)
        .map_err(|_| UpdateError::Signature(format!("the {what} is not readable")))
}

/// Check that `bytes` carry a signature by `public_key` for exactly `version`.
pub fn verify(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<(), UpdateError> {
    let key = minisign_verify::PublicKey::decode(&b64_text("updater key", public_key)?)
        .map_err(|e| UpdateError::Signature(format!("the updater key is not valid ({e})")))?;
    let sig = minisign_verify::Signature::decode(&b64_text("signature", signature)?)
        .map_err(|e| UpdateError::Signature(format!("the signature is not valid ({e})")))?;
    key.verify(bytes, &sig, false).map_err(|e| {
        UpdateError::Signature(format!("it was not signed with 8 West's updater key ({e})"))
    })?;
    // Only now is the trusted comment known to be 8 West's: the signature covers it.
    let signed = sig
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"))
        .map(str::trim);
    match signed {
        Some(v) if v.trim_start_matches('v') == version => Ok(()),
        Some(v) => Err(UpdateError::Signature(format!(
            "it was signed as version {v}, but the release says {version}"
        ))),
        None => Err(UpdateError::Signature(
            "its signature does not say which version it is".into(),
        )),
    }
}

/// Plenipo's own client: no redirects of its own (each hop goes back through Guard), and
/// Plenipo's name as its only identification.
fn client(timeout: Duration) -> Result<reqwest::Client, UpdateError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(timeout)
        .build()
        .map_err(|e| UpdateError::Network(format!("Plenipo could not prepare the request ({e})")))
}

/// GET `address`, following redirects only where Guard allows, reading at most `limit` bytes.
async fn get(
    guard: &Guard,
    rules: &OutboundRules,
    address: &str,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, UpdateError> {
    let client = client(timeout)?;
    let mut address = address.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        guard
            .check_outbound(rules, Purpose::Updates, &address)
            .map_err(UpdateError::Refused)?;
        let response = client
            .get(&address)
            .send()
            .await
            .map_err(|e| UpdateError::Network(network_words(&e)))?;
        let status = response.status();
        if status.is_redirection() {
            let next = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|l| l.to_str().ok())
                .ok_or_else(|| UpdateError::Network("GitHub sent Plenipo nowhere".into()))?;
            let base =
                reqwest::Url::parse(&address).map_err(|e| UpdateError::Network(e.to_string()))?;
            address = base
                .join(next)
                .map_err(|e| UpdateError::Network(e.to_string()))?
                .to_string();
            continue;
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(UpdateError::Network(
                "no release has been published there yet".into(),
            ));
        }
        if !status.is_success() {
            return Err(UpdateError::Network(format!("GitHub answered {status}")));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(UpdateError::Network("the file is far too big".into()));
        }
        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| UpdateError::Network(network_words(&e)))?
        {
            body.extend_from_slice(&chunk);
            if body.len() > limit {
                return Err(UpdateError::Network("the file is far too big".into()));
            }
        }
        return Ok(body);
    }
    Err(UpdateError::Network(
        "GitHub sent Plenipo round in circles".into(),
    ))
}

fn network_words(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "GitHub did not answer in time".into()
    } else if e.is_connect() {
        "Plenipo could not reach GitHub (no internet, or it is blocked)".into()
    } else {
        format!("the request did not work ({e})")
    }
}

/// Ask for the newest release. `Ok(None)`: this version is the newest.
pub async fn check(
    guard: &Guard,
    source: &UpdateSource,
    current: &str,
) -> Result<Option<Release>, UpdateError> {
    let bytes = get(
        guard,
        &source.rules(),
        &source.endpoint,
        Duration::from_secs(60),
        MAX_MANIFEST_BYTES,
    )
    .await?;
    let release = parse_manifest(&bytes)?;
    Ok(is_newer(current, &release.version).then_some(release))
}

/// Download `release`'s installer and check it (both conditions above). Nothing is kept when
/// the check fails.
pub async fn download(
    guard: &Guard,
    source: &UpdateSource,
    release: &Release,
) -> Result<Vec<u8>, UpdateError> {
    let key = source.public_key.as_deref().ok_or_else(|| {
        UpdateError::Signature(
            "this copy of Plenipo has no updater key, so it cannot check a download".into(),
        )
    })?;
    let bytes = get(
        guard,
        &source.rules(),
        &release.url,
        Duration::from_secs(20 * 60),
        MAX_INSTALLER_BYTES,
    )
    .await?;
    verify(&bytes, &release.signature, key, &release.version)?;
    Ok(bytes)
}
