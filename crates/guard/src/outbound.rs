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

/// The GitHub repository whose releases Plenipo updates from.
pub const RELEASES_PATH: &str = "/Seckcey/plenipo/releases/";
/// The hosts GitHub sends release downloads to.
pub const DOWNLOAD_HOSTS: [&str; 2] = [
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

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
            | Purpose::License => {
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
}
