//! Plenipo's own requests to the internet (Phase 13, ADR-038, updates; Phase 19, ADR-059, the AI
//! tools' newest versions). Workers never use this: their websites go through Plenipo's browser
//! and the owner's website lists.
//!
//! Each request Plenipo makes for itself has a **purpose**, and a purpose allows only its own
//! addresses, only over `https`, with no user name or password and no other port. Every hop of a
//! redirect is checked again, so a download cannot be sent somewhere else. The only exception is
//! an update test server on this computer (`http://127.0.0.1:<port>`), and only when this copy
//! of Plenipo was built to use one (never a setting, never an environment variable).

use crate::connections::Service;
use crate::paid::PaidService;
use crate::websites::Site;

/// Why Plenipo reaches the internet itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Checking for, and downloading, a new version of Plenipo (GitHub Releases only).
    Updates,
    /// Reading the newest version of an AI tool from its maker's published release list
    /// (ADR-059 §2): only the addresses in [`AI_TOOL_RELEASE_LISTS`].
    AiToolVersions,
    /// Signing in to, and using, one of the owner's connections (Phase 20, ADR-062 §8): only
    /// that service's own addresses ([`connection_hosts`]).
    Connection(Service),
    /// A paid task, or its key's check, on a paid AI service (Phase 16 Wave 3, ADR-085): only
    /// that service's own addresses ([`PaidService::hosts`]).
    PaidAi(PaidService),
    /// A Pro copy's weekly license check (Phase 11A, ADR-022, ADR-115): only
    /// [`LICENSE_CHECK_ADDRESS`], exactly. A Free copy never makes it.
    License,
    /// Using Plenipo from a phone (Phase 14, ADR-143 §12): only Plenipo's relay,
    /// [`RELAY_ADDRESS`], exactly, on Pro with the switch on. A Free copy never makes it.
    PhoneAccess,
    /// A sealed notice to one of the owner's phones (Phase 14, ADR-144 §1): only the phones'
    /// notice services ([`NOTICE_SERVICE_HOSTS`]).
    PhoneNotices,
    /// Plenipo's own requests to the account service for Community (Phase 24, ADR-162 §7): only
    /// [`COMMUNITY_ADDRESS`]'s host and its `/v1/community/` paths, while Community is on. A Free
    /// copy that never signs in never makes them.
    Community,
}

impl Purpose {
    pub fn label(self) -> &'static str {
        match self {
            Self::Updates => "checking for updates",
            Self::AiToolVersions => "checking the AI tools for new versions",
            Self::Connection(s) => match s {
                Service::Microsoft365 => "the Microsoft 365 connection",
                Service::Slack => "the Slack connection",
                Service::Google => "the Google connection",
                Service::Hubspot => "the HubSpot connection",
                Service::Stripe => "the Stripe connection",
                Service::Wordpress => "the WordPress connection",
                Service::Github => "the GitHub connection",
            },
            Self::PaidAi(s) => match s {
                PaidService::OpenRouter => "the OpenRouter paid key",
                PaidService::Anthropic => "the Anthropic paid key",
                PaidService::OpenAi => "the OpenAI paid key",
                PaidService::Xai => "the xAI paid key",
                PaidService::Moonshot => "the Moonshot AI paid key",
                PaidService::Google => "the Google paid key",
                PaidService::DeepSeek => "the DeepSeek paid key",
                PaidService::Zai => "the Z.ai paid key",
                PaidService::MiniMax => "the MiniMax paid key",
                PaidService::Mistral => "the Mistral paid key",
                PaidService::Alibaba => "the Alibaba Cloud paid key",
            },
            Self::License => "the weekly license check",
            Self::PhoneAccess => "using Plenipo from your phone",
            Self::PhoneNotices => "a notice to your phone",
            Self::Community => "Community",
        }
    }
}

/// The only hosts a connection reaches: its sign-in and its service (ADR-065 §1, ADR-064 §3–§7).
/// The website has none fixed: it reaches only the address saved on its card (ADR-071 §4).
pub fn connection_hosts(service: Service) -> &'static [&'static str] {
    match service {
        Service::Microsoft365 => &["login.microsoftonline.com", "graph.microsoft.com"],
        // Slack's sign-in, and its Web API, are both on slack.com.
        Service::Slack => &["slack.com"],
        // Google's sign-in and its token and cancel addresses; Gmail; Calendar and Drive.
        Service::Google => &[
            "accounts.google.com",
            "oauth2.googleapis.com",
            "gmail.googleapis.com",
            "www.googleapis.com",
        ],
        // HubSpot's web interface (its dated CRM addresses, ADR-071 §6.1).
        Service::Hubspot => &["api.hubapi.com"],
        Service::Stripe => &["api.stripe.com"],
        Service::Wordpress => &[],
        // GitHub's sign-in (the short code) and its web interface (ADR-204).
        Service::Github => &["github.com", "api.github.com"],
    }
}

/// The only paths the GitHub connection reaches (ADR-204, the reviewer's G2): on `github.com`,
/// the short-code sign-in's own three (the page the owner opens, and the two Plenipo asks), and
/// the pages to choose accounts and to remove Plenipo; on `api.github.com`, who signed in, which
/// accounts Plenipo may list, and their repositories' names. Nothing that reads code or changes
/// anything.
pub fn github_path_allowed(host: &str, path: &str) -> bool {
    let path = path.trim_end_matches('/');
    match host {
        "github.com" => {
            matches!(
                path,
                "/login/device"
                    | "/login/device/code"
                    | "/login/oauth/access_token"
                    | "/settings/installations"
                    | "/settings/apps/authorizations"
            ) || path
                .strip_prefix("/apps/")
                .and_then(|rest| rest.strip_suffix("/installations/new"))
                .is_some_and(|slug| {
                    !slug.is_empty()
                        && slug.len() <= 100
                        && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                })
        }
        "api.github.com" => {
            matches!(path, "/user" | "/user/installations" | "/user/repos")
                || path
                    .strip_prefix("/user/installations/")
                    .and_then(|rest| rest.strip_suffix("/repositories"))
                    .is_some_and(|id| {
                        !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit())
                    })
        }
        _ => false,
    }
}

/// Where a connection's service sends a file's download (a short-lived address that needs no
/// sign-in): Microsoft sends OneDrive and SharePoint files to its own storage, on these
/// domains' subdomains only (personal accounts' files, to `my.microsoftpersonalcontent.com`).
pub fn connection_download_domains(service: Service) -> &'static [&'static str] {
    match service {
        Service::Microsoft365 => &[
            "sharepoint.com",
            "files.1drv.com",
            "microsoftpersonalcontent.com",
        ],
        _ => &[],
    }
}

/// Whether `host` is one of `service`'s hosts, or a subdomain of one of its download domains —
/// or, for the website, exactly the host of the address saved on its card (`site`).
fn connection_host(service: Service, host: &str, site: Option<&str>) -> bool {
    if service == Service::Wordpress {
        return site.is_some_and(|s| !s.is_empty() && s == host);
    }
    connection_hosts(service).contains(&host)
        || connection_download_domains(service).iter().any(|d| {
            host.strip_suffix(d)
                .is_some_and(|rest| rest.len() > 1 && rest.ends_with('.'))
        })
}

/// The only addresses Plenipo reads the AI tools' newest versions from (ADR-059 §2): Anthropic's,
/// OpenAI's, and GitHub's (Copilot, ADR-083) packages on npm, and Ollama's releases on GitHub.
/// Each answers with a version number; nothing about the owner is sent.
pub const AI_TOOL_RELEASE_LISTS: [&str; 4] = [
    "https://registry.npmjs.org/@anthropic-ai/claude-code/latest",
    "https://registry.npmjs.org/@openai/codex/latest",
    "https://registry.npmjs.org/@github/copilot/latest",
    "https://api.github.com/repos/ollama/ollama/releases/latest",
];

/// The weekly license check's address (ADR-105): built into every copy, and never changed.
pub const LICENSE_CHECK_ADDRESS: &str = plenipo_licensing::CHECK_ADDRESS;

/// Where Community's requests go (ADR-162 §7): 8 West's account service, built into every copy
/// and never changed. A request adds one or more names after it (`/open`, `/sign-in/start`,
/// `/directory?q=…`).
pub const COMMUNITY_ADDRESS: &str = "https://account.getplenipo.com/v1/community";
/// The start of every Community path, on the account service and on its stand-in in the tests.
const COMMUNITY_PATH: &str = "/v1/community/";
/// The only words a Community address's query may use, each once (contract §4, §6, §12, §14).
const COMMUNITY_QUERY_WORDS: [&str; 7] =
    ["q", "kind", "region", "cursor", "wait", "period", "offset"];

/// Where a PC reaches 8 West's relay for phone access (ADR-143, ADR-146): Plenipo's own name
/// for the relay Milepost uses. The relay's real address is never in this repository.
pub const RELAY_ADDRESS: &str = "https://relay.getplenipo.com/plenipo/v1/pc";
/// The path a PC uses on the relay (and on its stand-in in the tests).
pub const RELAY_PATH: &str = "/plenipo/v1/pc";

/// The phones' notice services (ADR-144): Google's (Chrome, Android), Apple's (Safari,
/// iPhone), and Mozilla's (Firefox). Microsoft's (Edge on Windows) has many hosts under
/// [`NOTICE_SERVICE_SUFFIX`].
pub const NOTICE_SERVICE_HOSTS: [&str; 3] = [
    "fcm.googleapis.com",
    "web.push.apple.com",
    "updates.push.services.mozilla.com",
];
/// Microsoft's notice service: `<name>.notify.windows.com`.
pub const NOTICE_SERVICE_SUFFIX: &str = ".notify.windows.com";

/// Is `host` one of the phones' notice services?
pub fn notice_service(host: &str) -> bool {
    NOTICE_SERVICE_HOSTS.contains(&host)
        || host
            .strip_suffix(NOTICE_SERVICE_SUFFIX)
            .is_some_and(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
}

/// Whether `path` and `query` (as written, after the `?`) are a Community request's: a path of
/// `/v1/community/` and then short names with a `/` between, and no query, or only
/// `word=value` pairs with each of [`COMMUNITY_QUERY_WORDS`] at most once. The error says why, in
/// plain words (and never repeats any of the address).
fn community_request(path: &str, query: Option<&str>) -> Result<(), &'static str> {
    let Some(names) = path.strip_prefix(COMMUNITY_PATH) else {
        return Err("Community reaches only addresses that start with /v1/community/");
    };
    let short_name = |name: &str| {
        (1..=64).contains(&name.len())
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    if !names.split('/').all(short_name) {
        return Err(
            "a Community address's path is short names (letters, numbers, - and _) with a / \
             between them",
        );
    }
    let Some(query) = query else {
        return Ok(());
    };
    let mut seen = Vec::new();
    for pair in query.split('&') {
        let Some((word, value)) = pair.split_once('=') else {
            return Err("a Community address's query is only word=value pairs joined by &");
        };
        if !COMMUNITY_QUERY_WORDS.contains(&word) {
            return Err(
                "a Community address's query uses only q, kind, region, cursor, wait, period, \
                 and offset",
            );
        }
        if seen.contains(&word) {
            return Err("a Community address's query uses each word only once");
        }
        seen.push(word);
        if value.len() > 200 || !percent_encoded(value) {
            return Err(
                "a Community address's query values are up to 200 characters and percent-encoded",
            );
        }
    }
    Ok(())
}

/// Whether `value` is only letters, numbers, `- _ . ~ +`, and `%` with two hex digits after it.
fn percent_encoded(value: &str) -> bool {
    let mut bytes = value.bytes();
    while let Some(b) = bytes.next() {
        match b {
            b'%' => {
                let hex = |b: Option<u8>| b.is_some_and(|b| b.is_ascii_hexdigit());
                if !(hex(bytes.next()) && hex(bytes.next())) {
                    return false;
                }
            }
            b'-' | b'_' | b'.' | b'~' | b'+' => {}
            _ if b.is_ascii_alphanumeric() => {}
            _ => return false,
        }
    }
    true
}

/// The GitHub repository whose releases Plenipo updates from.
pub const RELEASES_PATH: &str = "/Seckcey/plenipo/releases/";
/// The hosts GitHub sends release downloads to.
pub const DOWNLOAD_HOSTS: [&str; 2] = [
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// The port of a stand-in on this computer (`http://127.0.0.1:<port>`), or `None`.
fn local_port(base: Option<&str>) -> Option<u16> {
    base.and_then(|b| {
        Site::parse(b).ok().and_then(|s| {
            (s.scheme == "http" && s.host == "127.0.0.1")
                .then_some(s.port)
                .flatten()
        })
    })
}

/// The addresses a purpose may use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutboundRules {
    /// An update test server on this computer (`127.0.0.1:<port>`), for copies of Plenipo
    /// built for the Windows installer tests only.
    pub test_server_port: Option<u16>,
    /// A stand-in for the AI tools' release lists on this computer (`127.0.0.1:<port>`, the
    /// list's host and path after it), for copies of Plenipo built for the end-to-end tests
    /// only (never a setting, never an environment variable).
    pub ai_tool_test_port: Option<u16>,
    /// A stand-in for the connections' services on this computer (`127.0.0.1:<port>`, the
    /// service's host and path after it), for copies of Plenipo built for the end-to-end tests
    /// only (never a setting, never an environment variable).
    pub connections_test_port: Option<u16>,
    /// A stand-in for the paid AI services on this computer (`127.0.0.1:<port>`, the service's
    /// host and path after it), for tests only (never a setting, never an environment variable).
    pub paid_test_port: Option<u16>,
    /// A stand-in for 8 West's license check on this computer (`127.0.0.1:<port>/v1/check`), for
    /// copies of Plenipo built for the tests only (never a setting, never an environment
    /// variable).
    pub license_test_port: Option<u16>,
    /// A stand-in for 8 West's relay on this computer (`127.0.0.1:<port>` and [`RELAY_PATH`]),
    /// for copies of Plenipo built for the tests only (never a setting, never an environment
    /// variable).
    pub relay_test_port: Option<u16>,
    /// A stand-in for the phones' notice services on this computer (`127.0.0.1:<port>`), for
    /// copies of Plenipo built for the tests only.
    pub notices_test_port: Option<u16>,
    /// A stand-in for 8 West's account service for Community on this computer
    /// (`127.0.0.1:<port>` and the `/v1/community/` paths), for copies of Plenipo built for the
    /// tests only (never a setting, never an environment variable).
    pub community_test_port: Option<u16>,
}

impl OutboundRules {
    /// The rules for a copy of Plenipo whose update address is `endpoint`: a test server on
    /// this computer is allowed only when the address built into this copy names one.
    pub fn for_endpoint(endpoint: &str) -> Self {
        let port = Site::parse(endpoint).ok().and_then(|s| {
            (s.scheme == "http" && s.host == "127.0.0.1")
                .then_some(s.port)
                .flatten()
        });
        Self {
            test_server_port: port,
            ai_tool_test_port: None,
            connections_test_port: None,
            paid_test_port: None,
            license_test_port: None,
            relay_test_port: None,
            notices_test_port: None,
            community_test_port: None,
        }
    }

    /// The same rules, with a stand-in for 8 West's license check at `base`
    /// (`http://127.0.0.1:<port>`), when this copy of Plenipo was built to use one.
    pub fn with_license_stand_in(mut self, base: Option<&str>) -> Self {
        self.license_test_port = base.and_then(|b| {
            Site::parse(b).ok().and_then(|s| {
                (s.scheme == "http" && s.host == "127.0.0.1")
                    .then_some(s.port)
                    .flatten()
            })
        });
        self
    }

    /// The same rules, with a stand-in for 8 West's relay at `base` (`http://127.0.0.1:<port>`),
    /// when this copy of Plenipo was built to use one.
    pub fn with_relay_stand_in(mut self, base: Option<&str>) -> Self {
        self.relay_test_port = local_port(base);
        self
    }

    /// The same rules, with a stand-in for the phones' notice services at `base`
    /// (`http://127.0.0.1:<port>`), when this copy of Plenipo was built to use one.
    pub fn with_notices_stand_in(mut self, base: Option<&str>) -> Self {
        self.notices_test_port = local_port(base);
        self
    }

    /// The same rules, with a stand-in for 8 West's account service for Community at `base`
    /// (`http://127.0.0.1:<port>`), when this copy of Plenipo was built to use one.
    pub fn with_community_stand_in(mut self, base: Option<&str>) -> Self {
        self.community_test_port = local_port(base);
        self
    }

    /// The same rules, with a stand-in for the connections' services at `base`
    /// (`http://127.0.0.1:<port>`), when this copy of Plenipo was built to use one.
    pub fn with_connections_stand_in(mut self, base: Option<&str>) -> Self {
        self.connections_test_port = base.and_then(|b| {
            Site::parse(b).ok().and_then(|s| {
                (s.scheme == "http" && s.host == "127.0.0.1")
                    .then_some(s.port)
                    .flatten()
            })
        });
        self
    }

    /// The same rules, with a stand-in for the AI tools' release lists at `base`
    /// (`http://127.0.0.1:<port>`), when this copy of Plenipo was built to use one.
    pub fn with_ai_tool_releases(mut self, base: Option<&str>) -> Self {
        self.ai_tool_test_port = base.and_then(|b| {
            Site::parse(b).ok().and_then(|s| {
                (s.scheme == "http" && s.host == "127.0.0.1")
                    .then_some(s.port)
                    .flatten()
            })
        });
        self
    }

    /// Check one address for `purpose`. The error says why, in plain words.
    pub fn check(&self, purpose: Purpose, address: &str) -> Result<Site, String> {
        self.check_for(purpose, address, None)
    }

    /// Check one address for `purpose`; for the website connection, `site_host` is the host of
    /// the address saved on its card (ADR-071 §4), the only one it may reach.
    pub fn check_for(
        &self,
        purpose: Purpose,
        address: &str,
        site_host: Option<&str>,
    ) -> Result<Site, String> {
        let site = Site::parse(address)?;
        let refuse = |why: &str| {
            Err(format!(
                "Plenipo refused to reach {} for {}: {why}.",
                site.shown(),
                purpose.label()
            ))
        };
        if purpose == Purpose::AiToolVersions {
            return self.check_release_list(&site, refuse);
        }
        if let Purpose::Connection(service) = purpose {
            return self.check_connection(service, &site, site_host, refuse);
        }
        if let Purpose::PaidAi(service) = purpose {
            return self.check_paid(service, &site, refuse);
        }
        if purpose == Purpose::License {
            return self.check_license(&site, refuse);
        }
        if purpose == Purpose::PhoneAccess {
            return self.check_relay(&site, refuse);
        }
        if purpose == Purpose::PhoneNotices {
            return self.check_notice_service(&site, refuse);
        }
        if purpose == Purpose::Community {
            return self.check_community(&site, address, refuse);
        }
        if let Some(port) = self.test_server_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                return Ok(site);
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        match purpose {
            Purpose::Updates => {
                let path = url::Url::parse(&site.url)
                    .map(|u| u.path().to_owned())
                    .unwrap_or_default();
                let github = site.host == "github.com" && path.starts_with(RELEASES_PATH);
                let download = DOWNLOAD_HOSTS.contains(&site.host.as_str());
                if github || download {
                    Ok(site)
                } else {
                    refuse("updates come only from Plenipo's releases on GitHub")
                }
            }
            Purpose::AiToolVersions
            | Purpose::Connection(_)
            | Purpose::PaidAi(_)
            | Purpose::License
            | Purpose::PhoneAccess
            | Purpose::PhoneNotices
            | Purpose::Community => {
                unreachable!("checked above")
            }
        }
    }

    /// The license check's one address, exactly, or its stand-in on this computer in a copy
    /// built for the tests.
    fn check_license(
        &self,
        site: &Site,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        let plain = url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none();
        let path = url.path();
        if let Some(port) = self.license_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                if plain && path == "/v1/check" {
                    return Ok(site.clone());
                }
                return refuse("the test stand-in serves only the license check");
            }
        }
        let Ok(check) = url::Url::parse(LICENSE_CHECK_ADDRESS) else {
            return refuse("the license check's address is not a web address");
        };
        let exact = site.scheme == "https"
            && site.port.is_none()
            && url.host_str() == check.host_str()
            && path == check.path();
        if plain && exact {
            Ok(site.clone())
        } else {
            refuse("the license check reaches only 8 West's license check address")
        }
    }

    /// Plenipo's relay, exactly ([`RELAY_ADDRESS`]), or its stand-in on this computer in a copy
    /// built for the tests.
    fn check_relay(
        &self,
        site: &Site,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        let plain = url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none();
        if let Some(port) = self.relay_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                if plain && url.path() == RELAY_PATH {
                    return Ok(site.clone());
                }
                return refuse("the test stand-in serves only the relay");
            }
        }
        let Ok(relay) = url::Url::parse(RELAY_ADDRESS) else {
            return refuse("the relay's address is not a web address");
        };
        let exact = site.scheme == "https"
            && site.port.is_none()
            && url.host_str() == relay.host_str()
            && url.path() == relay.path();
        if plain && exact {
            Ok(site.clone())
        } else {
            refuse("phone access reaches only Plenipo's relay")
        }
    }

    /// One of the phones' notice services, over `https` on its usual port, or its stand-in on
    /// this computer in a copy built for the tests.
    fn check_notice_service(
        &self,
        site: &Site,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        if !url.username().is_empty() || url.password().is_some() {
            return refuse("a user name or password in the address is never used");
        }
        if url.fragment().is_some() {
            return refuse("a notice address has no part after #");
        }
        if let Some(port) = self.notices_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                return Ok(site.clone());
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        if notice_service(&site.host) {
            Ok(site.clone())
        } else {
            refuse("a notice goes only to a phone's own notice service (Apple's, Google's, Mozilla's, or Microsoft's)")
        }
    }

    /// Community's addresses: `https://account.getplenipo.com/v1/community/…` with a path and
    /// query [`community_request`] allows, or its stand-in on this computer in a copy built for
    /// the tests. `address` is as it was given, so a query that needed tidying (a raw space, a
    /// quote mark) is refused, never quietly fixed.
    fn check_community(
        &self,
        site: &Site,
        address: &str,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        if !url.username().is_empty() || url.password().is_some() {
            return refuse("a user name or password in the address is never used");
        }
        if url.fragment().is_some() {
            return refuse("a Community address has no part after #");
        }
        let request = community_request(
            url.path(),
            address.trim().split_once('?').map(|(_, query)| query),
        );
        if let Some(port) = self.community_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                return match request {
                    Ok(()) => Ok(site.clone()),
                    Err(why) => refuse(why),
                };
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        let Ok(account) = url::Url::parse(COMMUNITY_ADDRESS) else {
            return refuse("Community's address is not a web address");
        };
        if url.host_str() != account.host_str() {
            return refuse("Community reaches only 8 West's account service");
        }
        match request {
            Ok(()) => Ok(site.clone()),
            Err(why) => refuse(why),
        }
    }

    /// One of the paid service's own hosts, over `https` on its usual port with no user name, or
    /// its stand-in on this computer in the tests.
    fn check_paid(
        &self,
        service: PaidService,
        site: &Site,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        if !url.username().is_empty() || url.password().is_some() {
            return refuse("a user name or password in the address is never used");
        }
        if let Some(port) = self.paid_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                // `http://127.0.0.1:<port>/<host><path>`.
                let rest = url.path().trim_start_matches('/');
                let host = &rest[..rest.find('/').unwrap_or(rest.len())];
                if service.hosts().contains(&host) {
                    return Ok(site.clone());
                }
                return refuse("the test stand-in serves only the paid service's own addresses");
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        if service.hosts().contains(&site.host.as_str()) {
            Ok(site.clone())
        } else {
            refuse("a paid AI key reaches only its own service's addresses")
        }
    }

    /// One of the connection's own hosts, over `https` on its usual port with no user name, or
    /// its stand-in on this computer in a copy built for the tests.
    fn check_connection(
        &self,
        service: Service,
        site: &Site,
        site_host: Option<&str>,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        if !url.username().is_empty() || url.password().is_some() {
            return refuse("a user name or password in the address is never used");
        }
        if let Some(port) = self.connections_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                // `http://127.0.0.1:<port>/<host><path>`.
                let rest = url.path().trim_start_matches('/');
                let host = &rest[..rest.find('/').unwrap_or(rest.len())];
                let path = &rest[host.len()..];
                if service == Service::Github && !github_path_allowed(host, path) {
                    return refuse("the GitHub connection reaches only its own few addresses");
                }
                if connection_host(service, host, site_host) {
                    return Ok(site.clone());
                }
                return refuse("the test stand-in serves only the connection's own addresses");
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        if service == Service::Github && !github_path_allowed(&site.host, url.path()) {
            return refuse("the GitHub connection reaches only its own few addresses");
        }
        if connection_host(service, &site.host, site_host) {
            Ok(site.clone())
        } else if service == Service::Wordpress {
            refuse("the website connection reaches only the address saved on its card")
        } else {
            refuse("a connection reaches only its own service's addresses")
        }
    }

    /// One of the AI tools' release lists, exactly (ADR-059 §2), or its stand-in on this
    /// computer in a copy built for the tests.
    fn check_release_list(
        &self,
        site: &Site,
        refuse: impl Fn(&str) -> Result<Site, String>,
    ) -> Result<Site, String> {
        let Ok(url) = url::Url::parse(&site.url) else {
            return refuse("that is not a web address");
        };
        let exact = |host: &str, path: &str| {
            AI_TOOL_RELEASE_LISTS.iter().any(|list| {
                url::Url::parse(list).is_ok_and(|l| l.host_str() == Some(host) && l.path() == path)
            })
        };
        let plain = url.query().is_none() && url.fragment().is_none() && url.username().is_empty();
        if let Some(port) = self.ai_tool_test_port {
            if site.scheme == "http" && site.host == "127.0.0.1" && site.port == Some(port) {
                // `http://127.0.0.1:<port>/<host><path>`.
                let rest = url.path().trim_start_matches('/');
                let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
                if plain && exact(host, path) {
                    return Ok(site.clone());
                }
                return refuse("the test stand-in serves only the AI tools' release lists");
            }
        }
        if site.scheme != "https" {
            return refuse("only https is allowed");
        }
        if site.port.is_some() {
            return refuse("only the usual https port is allowed");
        }
        if plain && exact(&site.host, url.path()) {
            Ok(site.clone())
        } else {
            refuse(
                "only the AI tools' own release lists are read (Anthropic's and OpenAI's on npm, \
                 Ollama's on GitHub)",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_access_reaches_only_plenipos_relay() {
        let rules = OutboundRules::default();
        assert!(rules.check(Purpose::PhoneAccess, RELAY_ADDRESS).is_ok());
        for bad in [
            "http://relay.getplenipo.com/plenipo/v1/pc",
            "https://relay.getplenipo.com:8443/plenipo/v1/pc",
            "https://relay.getplenipo.com/plenipo/v1/phone",
            "https://relay.getplenipo.com/plenipo/v1/pc?x=1",
            "https://relay.getplenipo.com/plenipo/v1/pc#x",
            "https://relay.evil.example/plenipo/v1/pc",
            "https://account.getplenipo.com/plenipo/v1/pc",
            "http://127.0.0.1:8769/plenipo/v1/pc",
        ] {
            let err = rules.check(Purpose::PhoneAccess, bad).unwrap_err();
            assert!(
                err.contains("using Plenipo from your phone"),
                "{bad}: {err}"
            );
        }
        // A user name or password is never sent anywhere.
        assert!(rules
            .check(
                Purpose::PhoneAccess,
                "https://user:pw@relay.getplenipo.com/plenipo/v1/pc"
            )
            .is_err());
        // The relay's own purpose is the only one that reaches it.
        assert!(rules.check(Purpose::License, RELAY_ADDRESS).is_err());
        assert!(rules.check(Purpose::Updates, RELAY_ADDRESS).is_err());
    }

    #[test]
    fn the_relay_stand_in_is_only_for_copies_built_for_the_tests() {
        let test = OutboundRules::default().with_relay_stand_in(Some("http://127.0.0.1:8769"));
        assert!(test
            .check(Purpose::PhoneAccess, "http://127.0.0.1:8769/plenipo/v1/pc")
            .is_ok());
        for bad in [
            "http://127.0.0.1:8769/plenipo/v1/phone",
            "http://127.0.0.1:8769/v1/check",
            "http://127.0.0.1:9999/plenipo/v1/pc",
            "http://192.168.1.5:8769/plenipo/v1/pc",
        ] {
            assert!(test.check(Purpose::PhoneAccess, bad).is_err(), "{bad}");
        }
        assert!(test.check(Purpose::PhoneAccess, RELAY_ADDRESS).is_ok());
        // Only this computer can be a stand-in.
        let not_local = OutboundRules::default().with_relay_stand_in(Some("http://10.0.0.2:8769"));
        assert_eq!(not_local.relay_test_port, None);
    }

    #[test]
    fn a_notice_goes_only_to_a_phones_notice_service() {
        let rules = OutboundRules::default();
        for ok in [
            "https://fcm.googleapis.com/fcm/send/abc:def",
            "https://web.push.apple.com/QGx0c2y",
            "https://updates.push.services.mozilla.com/wpush/v2/gAAA",
            "https://wns2-par02p.notify.windows.com/w/?token=BQYAAA",
        ] {
            assert!(rules.check(Purpose::PhoneNotices, ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://fcm.googleapis.com/fcm/send/abc",
            "https://fcm.googleapis.com:444/fcm/send/abc",
            "https://evil.example/fcm/send/abc",
            "https://notify.windows.com/w/",
            "https://x.y.notify.windows.com/w/",
            "https://relay.getplenipo.com/plenipo/v1/pc",
            "https://web.push.apple.com/a#b",
        ] {
            let err = rules.check(Purpose::PhoneNotices, bad).unwrap_err();
            assert!(err.contains("a notice to your phone"), "{bad}: {err}");
        }
        let test = OutboundRules::default().with_notices_stand_in(Some("http://127.0.0.1:8770"));
        assert!(test
            .check(Purpose::PhoneNotices, "http://127.0.0.1:8770/push/abc")
            .is_ok());
        assert!(test
            .check(Purpose::PhoneNotices, "http://127.0.0.1:8771/push/abc")
            .is_err());
    }

    #[test]
    fn updates_come_only_from_plenipos_releases_on_github() {
        let rules = OutboundRules::default();
        for ok in [
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            "https://github.com/Seckcey/plenipo/releases/download/v1.10.0/Plenipo_1.10.0_x64-setup.exe",
            "https://objects.githubusercontent.com/github-production-release-asset/1/2?x=y",
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/2",
        ] {
            assert!(rules.check(Purpose::Updates, ok).is_ok(), "{ok}");
        }
        for bad in [
            // Not https.
            "http://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            // Another repository, or not its releases.
            "https://github.com/someone/plenipo/releases/latest/download/latest.json",
            "https://github.com/Seckcey/plenipo/archive/main.zip",
            "https://github.com/Seckcey/plenipo-evil/releases/x",
            // Another host, or a look-alike.
            "https://evil.example/latest.json",
            "https://github.com.evil.example/Seckcey/plenipo/releases/x",
            "https://objects.githubusercontent.com.evil.example/x",
            // Another port, a user name, this computer.
            "https://github.com:8443/Seckcey/plenipo/releases/x",
            "https://user:pw@github.com/Seckcey/plenipo/releases/x",
            "http://127.0.0.1:8765/latest.json",
            "file:///C:/Windows/evil.exe",
        ] {
            let err = rules.check(Purpose::Updates, bad).unwrap_err();
            assert!(!err.is_empty(), "{bad}");
        }
        let err = rules
            .check(Purpose::Updates, "https://evil.example/latest.json")
            .unwrap_err();
        assert_eq!(
            err,
            "Plenipo refused to reach evil.example for checking for updates: updates come only \
             from Plenipo's releases on GitHub."
        );
    }

    #[test]
    fn a_connection_reaches_only_its_own_services_addresses() {
        let m = Purpose::Connection(Service::Microsoft365);
        let rules = OutboundRules::default();
        for ok in [
            "https://login.microsoftonline.com/organizations/oauth2/v2.0/token",
            "https://graph.microsoft.com/v1.0/me/messages?$top=25",
            "https://contoso-my.sharepoint.com/personal/x/_layouts/15/download.aspx?tempauth=y",
            "https://public.am.files.1drv.com/y4m",
            "https://my.microsoftpersonalcontent.com/personal/abc/_layouts/15/download.aspx",
        ] {
            assert!(rules.check(m, ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://graph.microsoft.com/v1.0/me",
            "https://graph.microsoft.com:8443/v1.0/me",
            "https://user:pw@graph.microsoft.com/v1.0/me",
            "https://graph.microsoft.com.evil.example/v1.0/me",
            "https://evil.example/graph.microsoft.com/v1.0/me",
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            "http://127.0.0.1:8767/graph.microsoft.com/v1.0/me",
            "https://sharepoint.com/x",
            "https://evilsharepoint.com/x",
            "https://sharepoint.com.evil.example/x",
            "https://microsoftpersonalcontent.com/x",
            "https://evilmicrosoftpersonalcontent.com/x",
        ] {
            assert!(rules.check(m, bad).is_err(), "{bad}");
        }
        // Each service reaches only its own addresses.
        let hubspot = Purpose::Connection(Service::Hubspot);
        assert!(rules
            .check(
                hubspot,
                "https://api.hubapi.com/crm/objects/2026-09/contacts/search"
            )
            .is_ok());
        for bad in [
            "https://app.hubspot.com/contacts/1",
            "https://api.hubapi.com.evil.example/crm",
            "http://api.hubapi.com/crm",
            "https://api.stripe.com/v1/balance",
        ] {
            assert!(rules.check(hubspot, bad).is_err(), "{bad}");
        }
        let stripe = Purpose::Connection(Service::Stripe);
        assert!(rules
            .check(stripe, "https://api.stripe.com/v1/refunds")
            .is_ok());
        for bad in [
            "https://files.stripe.com/v1/files/x",
            "https://dashboard.stripe.com/test/payments",
            "https://api.hubapi.com/crm",
        ] {
            assert!(rules.check(stripe, bad).is_err(), "{bad}");
        }
        let slack = Purpose::Connection(Service::Slack);
        for ok in [
            "https://slack.com/oauth/v2/authorize?client_id=1.2",
            "https://slack.com/api/conversations.history?channel=C0100000001",
        ] {
            assert!(rules.check(slack, ok).is_ok(), "{ok}");
        }
        for bad in [
            "https://graph.microsoft.com/v1.0/me",
            "https://files.slack.com/files-pri/T1-F1/x.txt",
            "https://8westit.slack.com/api/auth.test",
            "https://slack.com.evil.example/api/auth.test",
            "http://slack.com/api/auth.test",
        ] {
            assert!(rules.check(slack, bad).is_err(), "{bad}");
        }
        let google = Purpose::Connection(Service::Google);
        for ok in [
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=x",
            "https://oauth2.googleapis.com/token",
            "https://oauth2.googleapis.com/revoke",
            "https://gmail.googleapis.com/gmail/v1/users/me/messages",
            "https://www.googleapis.com/calendar/v3/calendars/primary/events",
            "https://www.googleapis.com/drive/v3/files",
        ] {
            assert!(rules.check(google, ok).is_ok(), "{ok}");
        }
        for bad in [
            "https://slack.com/api/auth.test",
            "https://graph.microsoft.com/v1.0/me",
            "https://drive.google.com/uc?id=1",
            "https://doc-0s-4k-docs.googleusercontent.com/docs/x",
            "https://googleapis.com/x",
            "https://evil.googleapis.com/x",
        ] {
            assert!(rules.check(google, bad).is_err(), "{bad}");
        }
        // A copy built for the tests: the stand-in, for the service's own hosts only.
        let test =
            OutboundRules::default().with_connections_stand_in(Some("http://127.0.0.1:8767"));
        assert!(test
            .check(m, "http://127.0.0.1:8767/graph.microsoft.com/v1.0/me")
            .is_ok());
        assert!(test
            .check(
                m,
                "http://127.0.0.1:8767/login.microsoftonline.com/organizations/oauth2/v2.0/token"
            )
            .is_ok());
        assert!(test
            .check(m, "http://127.0.0.1:8767/evil.example/x")
            .is_err());
        assert!(test
            .check(m, "http://127.0.0.1:9999/graph.microsoft.com/v1.0/me")
            .is_err());
        // Never another computer's stand-in.
        assert_eq!(
            OutboundRules::default()
                .with_connections_stand_in(Some("http://evil.example:8767"))
                .connections_test_port,
            None
        );
    }

    #[test]
    fn the_website_reaches_only_the_address_saved_on_its_card() {
        let wp = Purpose::Connection(Service::Wordpress);
        let rules = OutboundRules::default();
        // Nothing saved: nothing reached.
        assert!(rules
            .check(wp, "https://shop.example.com/wp-json/wp/v2/users/me")
            .is_err());
        let site = Some("shop.example.com");
        for ok in [
            "https://shop.example.com/wp-json/wp/v2/users/me",
            "https://shop.example.com/store/wp-json/wc/v3/orders?per_page=1",
        ] {
            assert!(rules.check_for(wp, ok, site).is_ok(), "{ok}");
        }
        for bad in [
            // Not the saved host, even its www. twin or a look-alike.
            "https://www.shop.example.com/wp-json/wp/v2/users/me",
            "https://example.com/wp-json/wp/v2/users/me",
            "https://shop.example.com.evil.example/wp-json/",
            "https://evil.example/shop.example.com/wp-json/",
            // Not https, another port, a user name.
            "http://shop.example.com/wp-json/",
            "https://shop.example.com:8443/wp-json/",
            "https://u:p@shop.example.com/wp-json/",
        ] {
            let err = rules.check_for(wp, bad, site).unwrap_err();
            assert!(!err.is_empty(), "{bad}");
        }
        assert!(rules
            .check_for(wp, "https://www.shop.example.com/", site)
            .unwrap_err()
            .contains("saved on its card"));
        // The saved host means nothing for the other services.
        assert!(rules
            .check_for(
                Purpose::Connection(Service::Stripe),
                "https://shop.example.com/x",
                site
            )
            .is_err());
        // A copy built for the tests: the stand-in, for the saved host only.
        let test =
            OutboundRules::default().with_connections_stand_in(Some("http://127.0.0.1:8767"));
        assert!(test
            .check_for(
                wp,
                "http://127.0.0.1:8767/shop.example.com/wp-json/wp/v2/users/me",
                site
            )
            .is_ok());
        assert!(test
            .check_for(wp, "http://127.0.0.1:8767/evil.example/wp-json/", site)
            .is_err());
        assert!(test
            .check_for(wp, "http://127.0.0.1:8767/shop.example.com/wp-json/", None)
            .is_err());
    }

    #[test]
    fn a_test_server_is_allowed_only_for_a_copy_built_to_use_it() {
        // The address built into a released copy: GitHub, and no test server.
        let released = OutboundRules::for_endpoint(
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
        );
        assert_eq!(released.test_server_port, None);
        assert!(released
            .check(Purpose::Updates, "http://127.0.0.1:8765/latest.json")
            .is_err());
        // A copy built for the installer tests: that exact server on this computer only.
        let test = OutboundRules::for_endpoint("http://127.0.0.1:8765/latest.json");
        assert_eq!(test.test_server_port, Some(8765));
        assert!(test
            .check(
                Purpose::Updates,
                "http://127.0.0.1:8765/Plenipo_1.9.1_x64-setup.exe"
            )
            .is_ok());
        assert!(test
            .check(Purpose::Updates, "http://127.0.0.1:9999/latest.json")
            .is_err());
        assert!(test
            .check(Purpose::Updates, "http://192.168.1.5:8765/latest.json")
            .is_err());
        // Anything but 127.0.0.1 over http never makes a test server.
        assert_eq!(
            OutboundRules::for_endpoint("http://evil.example:8765/latest.json").test_server_port,
            None
        );
    }

    #[test]
    fn the_ai_tools_newest_versions_come_only_from_their_own_release_lists() {
        let rules = OutboundRules::default();
        for ok in AI_TOOL_RELEASE_LISTS {
            assert!(rules.check(Purpose::AiToolVersions, ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://registry.npmjs.org/@openai/codex/latest",
            "https://registry.npmjs.org/@openai/codex",
            "https://registry.npmjs.org/@openai/codex/latest?x=1",
            "https://registry.npmjs.org/evil-package/latest",
            "https://registry.npmjs.org:444/@openai/codex/latest",
            "https://api.github.com/repos/ollama/ollama/releases",
            "https://api.github.com/repos/someone/ollama/releases/latest",
            "https://chatgpt.com/backend-api/wham/usage",
            "https://api.anthropic.com/api/oauth/usage",
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            "http://127.0.0.1:8766/registry.npmjs.org/@openai/codex/latest",
        ] {
            assert!(rules.check(Purpose::AiToolVersions, bad).is_err(), "{bad}");
        }
        // Plenipo's own update addresses are not AI tool lists, and the other way round.
        assert!(rules
            .check(Purpose::Updates, AI_TOOL_RELEASE_LISTS[0])
            .is_err());
        // A copy built for the tests: that stand-in on this computer, for the same lists only.
        let test = OutboundRules::default().with_ai_tool_releases(Some("http://127.0.0.1:8766"));
        assert!(test
            .check(
                Purpose::AiToolVersions,
                "http://127.0.0.1:8766/registry.npmjs.org/@openai/codex/latest"
            )
            .is_ok());
        for bad in [
            "http://127.0.0.1:8766/evil.example/x",
            "http://127.0.0.1:8767/registry.npmjs.org/@openai/codex/latest",
            "http://127.0.0.1:8766/registry.npmjs.org/@openai/codex/latest?q=1",
        ] {
            assert!(test.check(Purpose::AiToolVersions, bad).is_err(), "{bad}");
        }
        assert_eq!(
            OutboundRules::default()
                .with_ai_tool_releases(Some("http://evil.example:8766"))
                .ai_tool_test_port,
            None
        );
    }

    #[test]
    fn a_refusal_is_recorded_in_the_ledger_with_the_host_only() {
        let ledger = std::sync::Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap());
        let guard = crate::Guard::new(ledger.clone());
        let rules = OutboundRules::default();
        assert!(guard
            .check_outbound(
                &rules,
                Purpose::Updates,
                "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            )
            .is_ok());
        assert!(ledger
            .events_of_types(&["guard.request_refused"], 5)
            .unwrap()
            .is_empty());
        assert!(guard
            .check_outbound(&rules, Purpose::Updates, "https://evil.example/x?token=abc")
            .is_err());
        let refused = ledger
            .events_of_types(&["guard.request_refused"], 5)
            .unwrap();
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].payload["host"], "evil.example");
        assert!(!refused[0].payload.to_string().contains("token=abc"));
    }

    #[test]
    fn the_license_check_reaches_only_its_one_address() {
        let rules = OutboundRules::default();
        assert!(rules
            .check(Purpose::License, "https://account.getplenipo.com/v1/check")
            .is_ok());
        for bad in [
            "http://account.getplenipo.com/v1/check",
            "https://account.getplenipo.com:8443/v1/check",
            "https://account.getplenipo.com/v1/check?key=x",
            "https://account.getplenipo.com/v1/check#x",
            "https://account.getplenipo.com/v1/checks",
            "https://account.getplenipo.com/account",
            "https://user:pw@account.getplenipo.com/v1/check",
            "https://getplenipo.com/v1/check",
            "https://account.getplenipo.com.evil.example/v1/check",
            "https://evil.example/account.getplenipo.com/v1/check",
            "http://127.0.0.1:8768/v1/check",
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
        ] {
            let err = rules.check(Purpose::License, bad).expect_err(bad);
            // An address with a user name in it is refused before any purpose is looked at.
            if !bad.contains("user:pw") {
                assert!(err.contains("the weekly license check"), "{err}");
            }
        }
        // The license check's address is for nothing else.
        assert!(rules
            .check(Purpose::Updates, "https://account.getplenipo.com/v1/check")
            .is_err());
        // A copy built for the tests: that stand-in, for the check's path only.
        let test = OutboundRules::default().with_license_stand_in(Some("http://127.0.0.1:8768"));
        assert!(test
            .check(Purpose::License, "http://127.0.0.1:8768/v1/check")
            .is_ok());
        for bad in [
            "http://127.0.0.1:8768/v1/other",
            "http://127.0.0.1:8769/v1/check",
            "http://127.0.0.1:8768/v1/check?x=1",
        ] {
            assert!(test.check(Purpose::License, bad).is_err(), "{bad}");
        }
        assert_eq!(
            OutboundRules::default()
                .with_license_stand_in(Some("http://evil.example:8768"))
                .license_test_port,
            None
        );
    }

    #[test]
    fn community_reaches_only_the_account_services_community_paths() {
        let rules = OutboundRules::default();
        for ok in [
            "https://account.getplenipo.com/v1/community/open",
            "https://account.getplenipo.com/v1/community/sign-in/start",
            "https://account.getplenipo.com/v1/community/me/profile",
            "https://account.getplenipo.com/v1/community/people/cm_01JB7Q8R9S0T1V2W3X4Y5Z6A7B/devices",
            "https://account.getplenipo.com/v1/community/people/by-name/pat-lee",
            "https://account.getplenipo.com/v1/community/directory?q=pat&region=US-CA&cursor=abc",
            "https://account.getplenipo.com/v1/community/directory?q=pat%20lee&kind=bakery",
            "https://account.getplenipo.com/v1/community/directory?q=&kind=",
            "https://account.getplenipo.com/v1/community/items?wait=25",
            "https://account.getplenipo.com/v1/community/gifs?q=well+done&offset=20",
            "https://account.getplenipo.com/v1/community/leaderboard?period=week",
        ] {
            assert!(rules.check(Purpose::Community, ok).is_ok(), "{ok}");
        }
        // The longest name and the longest value that are allowed.
        let name = "a".repeat(64);
        let value = "a".repeat(200);
        for ok in [
            format!("{COMMUNITY_ADDRESS}/{name}"),
            format!("{COMMUNITY_ADDRESS}/{name}/{name}?q={value}"),
        ] {
            assert!(rules.check(Purpose::Community, &ok).is_ok(), "{ok}");
        }
        for bad in [
            // Not https, another port, a fragment.
            "http://account.getplenipo.com/v1/community/open",
            "https://account.getplenipo.com:8443/v1/community/open",
            "https://account.getplenipo.com/v1/community/open#x",
            // Not that host, or a look-alike.
            "https://account.getplenipo.com.evil.example/v1/community/open",
            "https://evil.getplenipo.com/v1/community/open",
            "https://sub.account.getplenipo.com/v1/community/open",
            "https://getplenipo.com/v1/community/open",
            "https://relay.getplenipo.com/v1/community/open",
            "https://evil.example/account.getplenipo.com/v1/community/open",
            "http://127.0.0.1:8771/v1/community/open",
            // Not under /v1/community/, and the address alone is not a request.
            "https://account.getplenipo.com/v1/check",
            "https://account.getplenipo.com/v1/communityx/open",
            "https://account.getplenipo.com/v1/community/../check",
            "https://account.getplenipo.com/v1/community/%2e%2e/check",
            COMMUNITY_ADDRESS,
            "https://account.getplenipo.com/v1/community/",
            // Names that are not short, plain ones.
            "https://account.getplenipo.com/v1/community//open",
            "https://account.getplenipo.com/v1/community/open/",
            "https://account.getplenipo.com/v1/community/a%2Fb",
            "https://account.getplenipo.com/v1/community/a%20b",
            "https://account.getplenipo.com/v1/community/a.b",
            "https://account.getplenipo.com/v1/community/a;b",
            // A query with a word that is not Community's, a word twice, or a pair missing a part.
            "https://account.getplenipo.com/v1/community/open?key=x",
            "https://account.getplenipo.com/v1/community/open?Q=a",
            "https://account.getplenipo.com/v1/community/open?q=a&q=b",
            "https://account.getplenipo.com/v1/community/open?q",
            "https://account.getplenipo.com/v1/community/open?",
            "https://account.getplenipo.com/v1/community/open?q=a&",
            "https://account.getplenipo.com/v1/community/open?=a",
            // A value that is not percent-encoded.
            "https://account.getplenipo.com/v1/community/directory?q=a b",
            "https://account.getplenipo.com/v1/community/directory?q=a\"b",
            "https://account.getplenipo.com/v1/community/directory?q=<b>",
            "https://account.getplenipo.com/v1/community/directory?q=a=b",
            "https://account.getplenipo.com/v1/community/directory?q=%zz",
            "https://account.getplenipo.com/v1/community/directory?q=%4",
            "https://account.getplenipo.com/v1/community/directory?q=caf\u{e9}",
        ] {
            let err = rules.check(Purpose::Community, bad).expect_err(bad);
            assert!(err.contains("for Community:"), "{bad}: {err}");
        }
        // Too long: a name over 64 characters, a value over 200.
        for bad in [
            format!("{COMMUNITY_ADDRESS}/{}", "a".repeat(65)),
            format!("{COMMUNITY_ADDRESS}/open?q={}", "a".repeat(201)),
        ] {
            assert!(rules.check(Purpose::Community, &bad).is_err(), "{bad}");
        }
        // A user name or password is refused before any purpose is looked at.
        assert!(rules
            .check(
                Purpose::Community,
                "https://user:pw@account.getplenipo.com/v1/community/open"
            )
            .is_err());
        assert_eq!(
            rules
                .check(Purpose::Community, "https://evil.example/v1/community/open")
                .unwrap_err(),
            "Plenipo refused to reach evil.example for Community: Community reaches only 8 \
             West's account service."
        );
    }

    #[test]
    fn community_and_the_other_purposes_keep_to_their_own_addresses() {
        let rules = OutboundRules::default();
        // Community's address is for nothing else.
        let community = format!("{COMMUNITY_ADDRESS}/open");
        assert!(rules.check(Purpose::Community, &community).is_ok());
        for other in [
            Purpose::Updates,
            Purpose::AiToolVersions,
            Purpose::Connection(Service::Microsoft365),
            Purpose::Connection(Service::Wordpress),
            Purpose::PaidAi(PaidService::OpenRouter),
            Purpose::License,
            Purpose::PhoneAccess,
            Purpose::PhoneNotices,
        ] {
            assert!(rules.check(other, &community).is_err(), "{other:?}");
        }
        // And nothing else reaches Community's addresses, not even the same host.
        for bad in [
            LICENSE_CHECK_ADDRESS,
            RELAY_ADDRESS,
            AI_TOOL_RELEASE_LISTS[0],
            "https://github.com/Seckcey/plenipo/releases/latest/download/latest.json",
            "https://openrouter.ai/api/v1/key",
            "https://graph.microsoft.com/v1.0/me",
            "https://fcm.googleapis.com/fcm/send/abc",
        ] {
            let err = rules.check(Purpose::Community, bad).expect_err(bad);
            assert!(err.contains("for Community:"), "{bad}: {err}");
        }
        assert_eq!(Purpose::Community.label(), "Community");
    }

    #[test]
    fn the_community_stand_in_is_only_for_copies_built_for_the_tests() {
        let test = OutboundRules::default().with_community_stand_in(Some("http://127.0.0.1:8771"));
        assert_eq!(test.community_test_port, Some(8771));
        for ok in [
            "http://127.0.0.1:8771/v1/community/open",
            "http://127.0.0.1:8771/v1/community/sign-in/start",
            "http://127.0.0.1:8771/v1/community/directory?q=pat&region=US-CA",
            "http://127.0.0.1:8771/v1/community/items?wait=25",
        ] {
            assert!(test.check(Purpose::Community, ok).is_ok(), "{ok}");
        }
        // The same path and query rules apply on the stand-in.
        for bad in [
            "http://127.0.0.1:8771/v1/check",
            "http://127.0.0.1:8771/v1/communityx/open",
            "http://127.0.0.1:8771/v1/community/",
            "http://127.0.0.1:8771/v1/community//open",
            "http://127.0.0.1:8771/v1/community/../check",
            "http://127.0.0.1:8771/v1/community/open?key=x",
            "http://127.0.0.1:8771/v1/community/open?q=a&q=b",
            "http://127.0.0.1:8771/v1/community/directory?q=a b",
            "http://127.0.0.1:8771/v1/community/open#x",
            // Another port, another computer, not http.
            "http://127.0.0.1:9999/v1/community/open",
            "http://192.168.1.5:8771/v1/community/open",
            "https://127.0.0.1:8771/v1/community/open",
        ] {
            assert!(test.check(Purpose::Community, bad).is_err(), "{bad}");
        }
        // The real address still works, and the stand-in opens nothing for any other purpose.
        assert!(test
            .check(
                Purpose::Community,
                "https://account.getplenipo.com/v1/community/open"
            )
            .is_ok());
        for other in [Purpose::License, Purpose::PhoneAccess, Purpose::Updates] {
            assert!(
                test.check(other, "http://127.0.0.1:8771/v1/community/open")
                    .is_err(),
                "{other:?}"
            );
        }
        // Without the stand-in, this computer is never reached, and only this computer can be one.
        assert!(OutboundRules::default()
            .check(
                Purpose::Community,
                "http://127.0.0.1:8771/v1/community/open"
            )
            .is_err());
        assert_eq!(
            OutboundRules::default()
                .with_community_stand_in(Some("http://evil.example:8771"))
                .community_test_port,
            None
        );
        assert_eq!(
            OutboundRules::for_endpoint("http://127.0.0.1:8765/latest.json").community_test_port,
            None
        );
    }

    #[test]
    fn a_community_refusal_is_recorded_in_the_ledger_with_the_host_only() {
        let ledger = std::sync::Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap());
        let guard = crate::Guard::new(ledger.clone());
        let rules = OutboundRules::default();
        assert!(guard
            .check_outbound(
                &rules,
                Purpose::Community,
                "https://account.getplenipo.com/v1/community/directory?q=pat&region=US-CA",
            )
            .is_ok());
        assert!(ledger
            .events_of_types(&["guard.request_refused"], 5)
            .unwrap()
            .is_empty());
        // A query word that is not Community's, and a host that is not 8 West's.
        for bad in [
            "https://account.getplenipo.com/v1/community/directory?q=my-secret&token=abc",
            "https://evil.example/v1/community/directory?q=my-secret",
        ] {
            assert!(guard
                .check_outbound(&rules, Purpose::Community, bad)
                .is_err());
        }
        let refused = ledger
            .events_of_types(&["guard.request_refused"], 5)
            .unwrap();
        assert_eq!(refused.len(), 2);
        let mut hosts: Vec<&str> = refused
            .iter()
            .map(|e| e.payload["host"].as_str().unwrap())
            .collect();
        hosts.sort_unstable();
        assert_eq!(hosts, ["account.getplenipo.com", "evil.example"]);
        for event in &refused {
            assert_eq!(event.payload["purpose"], "Community");
            let recorded = event.payload.to_string();
            assert!(!recorded.contains("my-secret"), "{recorded}");
            assert!(!recorded.contains("token=abc"), "{recorded}");
        }
    }

    #[test]
    fn each_paid_service_reaches_its_own_addresses_and_no_other_services() {
        let rules = OutboundRules::default();
        for s in PaidService::ALL {
            let paid = Purpose::PaidAi(s);
            for path in [s.key_check_path(), s.chat_path()] {
                let address = format!("{}{path}", s.base_url());
                assert!(rules.check(paid, &address).is_ok(), "{address}");
            }
            for other in PaidService::ALL.into_iter().filter(|o| *o != s) {
                let address = format!("{}{}", other.base_url(), other.chat_path());
                let err = rules.check(paid, &address).expect_err(&address);
                assert!(err.contains(paid.label()), "{err}");
            }
        }
    }

    #[test]
    fn a_paid_key_reaches_only_its_own_services_https_addresses() {
        let rules = OutboundRules::default();
        let paid = Purpose::PaidAi(PaidService::OpenRouter);
        for ok in [
            "https://openrouter.ai/api/v1/key",
            "https://openrouter.ai/api/v1/models",
            "https://openrouter.ai/api/v1/chat/completions",
        ] {
            assert!(rules.check(paid, ok).is_ok(), "{ok}");
        }
        for (bad, why) in [
            ("http://openrouter.ai/api/v1/key", "only https"),
            ("https://openrouter.ai:8443/api/v1/key", "usual https port"),
            (
                "https://user:pass@openrouter.ai/api/v1/key",
                "user name or password",
            ),
            (
                "https://api.openai.com/v1/models",
                "its own service's addresses",
            ),
            (
                "https://openrouter.ai.evil.example/api/v1/key",
                "its own service's addresses",
            ),
            (
                "https://evil.example/openrouter.ai/api/v1/key",
                "its own service's addresses",
            ),
            // A stand-in on this computer only in the tests.
            (
                "http://127.0.0.1:9911/openrouter.ai/api/v1/key",
                "only https",
            ),
        ] {
            let err = rules.check(paid, bad).expect_err(bad);
            assert!(err.contains(why), "{bad}: {err}");
            // An address with a user name in it is refused before any purpose is looked at.
            if why != "user name or password" {
                assert!(err.contains("the OpenRouter paid key"), "{err}");
            }
        }
        // The tests' stand-in serves the service's own addresses, and nothing else.
        let tests = OutboundRules {
            paid_test_port: Some(9911),
            ..OutboundRules::default()
        };
        assert!(tests
            .check(paid, "http://127.0.0.1:9911/openrouter.ai/api/v1/key")
            .is_ok());
        assert!(tests
            .check(paid, "http://127.0.0.1:9911/evil.example/api/v1/key")
            .is_err());
        assert!(tests
            .check(paid, "http://127.0.0.1:9912/openrouter.ai/api/v1/key")
            .is_err());
        // Another purpose never reaches a paid service's addresses.
        assert!(rules
            .check(Purpose::Updates, "https://openrouter.ai/api/v1/key")
            .is_err());
    }

    /// ADR-204, the reviewer's G2: the GitHub connection reaches only github.com's short-code
    /// sign-in and account pages, and api.github.com's few reading addresses; never code, never a
    /// change, never another host, never plain http or another port.
    #[test]
    fn the_github_connection_reaches_only_its_few_addresses() {
        let rules = OutboundRules::default();
        let github = Purpose::Connection(Service::Github);
        for ok in [
            "https://github.com/login/device",
            "https://github.com/login/device/code",
            "https://github.com/login/oauth/access_token",
            "https://github.com/apps/plenipo-by-8-west/installations/new",
            "https://github.com/settings/installations",
            "https://github.com/settings/apps/authorizations",
            "https://api.github.com/user",
            "https://api.github.com/user/installations?per_page=100&page=2",
            "https://api.github.com/user/installations/12345/repositories?per_page=100",
            "https://api.github.com/user/repos?per_page=100",
        ] {
            assert!(rules.check(github, ok).is_ok(), "{ok}");
        }
        for refused in [
            "https://api.github.com/repos/acme/web/contents/README.md",
            "https://api.github.com/repos/acme/web",
            "https://api.github.com/user/emails",
            "https://api.github.com/user/installations/12x/repositories",
            "https://api.github.com/user/installations/1/repositories/2",
            "https://github.com/acme/web",
            "https://github.com/login/oauth/authorize",
            "https://github.com/apps/a/b/installations/new",
            "https://raw.githubusercontent.com/acme/web/main/README.md",
            "https://uploads.github.com/x",
            "http://api.github.com/user",
            "https://api.github.com:8443/user",
            "https://user:pass@api.github.com/user",
        ] {
            assert!(rules.check(github, refused).is_err(), "{refused}");
        }
        // The stand-in in a copy built for the tests serves the same few addresses only.
        let test =
            OutboundRules::default().with_connections_stand_in(Some("http://127.0.0.1:8767"));
        assert!(test
            .check(github, "http://127.0.0.1:8767/api.github.com/user")
            .is_ok());
        assert!(test
            .check(
                github,
                "http://127.0.0.1:8767/api.github.com/repos/a/b/contents"
            )
            .is_err());
        assert!(test
            .check(github, "http://127.0.0.1:8767/graph.microsoft.com/v1.0/me")
            .is_err());
        // Another connection never reaches GitHub.
        assert!(rules
            .check(
                Purpose::Connection(Service::Slack),
                "https://api.github.com/user"
            )
            .is_err());
    }
}
