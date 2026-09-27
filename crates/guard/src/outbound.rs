//! Plenipo's own requests to the internet (Phase 13, ADR-037, updates). Workers never use this:
//! their websites go through Plenipo's browser and the owner's website lists.
//!
//! Each request Plenipo makes for itself has a **purpose**, and a purpose allows only its own
//! addresses, only over `https`, with no user name or password and no other port. Every hop of a
//! redirect is checked again, so a download cannot be sent somewhere else. The only exception is
//! an update test server on this computer (`http://127.0.0.1:<port>`), and only when this copy
//! of Plenipo was built to use one (never a setting, never an environment variable).

use crate::websites::Site;

/// Why Plenipo reaches the internet itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Checking for, and downloading, a new version of Plenipo (GitHub Releases only).
    Updates,
}

impl Purpose {
    pub fn label(self) -> &'static str {
        match self {
            Self::Updates => "checking for updates",
        }
    }
}

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
        }
    }

    /// Check one address for `purpose`. The error says why, in plain words.
    pub fn check(&self, purpose: Purpose, address: &str) -> Result<Site, String> {
        let site = Site::parse(address)?;
        let refuse = |why: &str| {
            Err(format!(
                "Plenipo refused to reach {} for {}: {why}.",
                site.shown(),
                purpose.label()
            ))
        };
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
}
