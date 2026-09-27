//! The owner's website lists (Phase 10, ADR-020): the websites workers may open in Plenipo's
//! browser without asking, the ones they may never open, and what happens with every other
//! website (ask the owner, or blocked). Addresses on this computer or the local network open
//! only when a list allows them by name.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most entries in each list, and the longest entry.
pub const MAX_SITES: usize = 300;
pub const MAX_SITE_CHARS: usize = 260;

/// What happens with a website on neither list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OtherSites {
    /// Ask the owner the first time a worker opens it in a step.
    #[default]
    Ask,
    /// Never open it.
    Block,
}

/// The owner's website lists. An entry is a host name (`example.com`, which also covers its
/// subdomains such as `www.example.com`), an IP address, or `localhost`, with an optional port
/// (`127.0.0.1:8080`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct WebsiteRules {
    /// Opened without asking (when the worker may visit websites).
    pub allowed: Vec<String>,
    /// Never opened.
    pub blocked: Vec<String>,
    /// Every other website.
    pub others: OtherSites,
}

/// A web address a worker wants to use, read into what the lists match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// `https://www.example.com:8443/path?q` as given (normalized).
    pub url: String,
    /// `www.example.com` (lower case; IPv6 in brackets).
    pub host: String,
    /// The port, when the address names one other than the scheme's default.
    pub port: Option<u16>,
    /// `https`, `http`, or `about` (for `about:blank`).
    pub scheme: String,
}

impl Site {
    /// Read an address. Only `http`, `https`, and `about:blank` can be opened; everything else
    /// (files, `javascript:`, the browser's own pages, …) is refused with the reason.
    pub fn parse(address: &str) -> Result<Self, String> {
        let address = address.trim();
        if address.eq_ignore_ascii_case("about:blank") {
            return Ok(Self {
                url: "about:blank".into(),
                host: String::new(),
                port: None,
                scheme: "about".into(),
            });
        }
        let with_scheme = if address.contains("://") {
            address.to_owned()
        } else {
            format!("https://{address}")
        };
        let url = url::Url::parse(&with_scheme)
            .map_err(|e| format!("{address:?} is not a web address ({e})"))?;
        let scheme = url.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(format!(
                "{address:?} is not a website: only http and https addresses can be opened"
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(
                "web addresses with a user name or password in them are never opened".into(),
            );
        }
        let host = match url.host() {
            Some(url::Host::Domain(d)) => d.trim_end_matches('.').to_ascii_lowercase(),
            Some(url::Host::Ipv4(ip)) => ip.to_string(),
            Some(url::Host::Ipv6(ip)) => format!("[{ip}]"),
            None => return Err(format!("{address:?} names no website")),
        };
        if host.is_empty() {
            return Err(format!("{address:?} names no website"));
        }
        Ok(Self {
            url: url.to_string(),
            host,
            port: url.port(),
            scheme,
        })
    }

    /// `host` or `host:port`, as shown to the owner.
    pub fn shown(&self) -> String {
        match self.port {
            Some(p) => format!("{}:{p}", self.host),
            None => self.host.clone(),
        }
    }

    /// An address on this computer or the local network (such as a router's page).
    pub fn is_local(&self) -> bool {
        let h = self.host.as_str();
        if h == "localhost" || h.ends_with(".localhost") {
            return true;
        }
        for suffix in [".local", ".lan", ".internal", ".home.arpa", ".intranet"] {
            if h.ends_with(suffix) {
                return true;
            }
        }
        let bare = h.trim_start_matches('[').trim_end_matches(']');
        match bare.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => {
                let o = ip.octets();
                ip.is_loopback()
                    || ip.is_private()
                    || ip.is_link_local()
                    || ip.is_unspecified()
                    || ip.is_broadcast()
                    // Shared address space (carrier-grade NAT), 100.64.0.0/10.
                    || (o[0] == 100 && (64..128).contains(&o[1]))
            }
            Ok(IpAddr::V6(ip)) => {
                let first = ip.segments()[0];
                ip.is_loopback()
                    || ip.is_unspecified()
                    // Unique local (fc00::/7) and link-local (fe80::/10).
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80
                    || ip.to_ipv4_mapped().is_some_and(|v4| {
                        v4.is_loopback() || v4.is_private() || v4.is_link_local()
                    })
            }
            Err(_) => false,
        }
    }
}

/// One list entry, read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    host: String,
    port: Option<u16>,
}

impl Entry {
    fn parse(entry: &str) -> Option<Self> {
        let e = entry.trim().trim_start_matches("*.");
        let site = Site::parse(e).ok()?;
        (site.scheme != "about").then_some(Self {
            host: site.host,
            port: site.port,
        })
    }

    /// The site is this entry's host or a subdomain of it, on its port when it names one.
    fn matches(&self, site: &Site) -> bool {
        let host_ok = site.host == self.host
            || (!self.host.starts_with('[')
                && self.host.parse::<IpAddr>().is_err()
                && site.host.ends_with(&format!(".{}", self.host)));
        host_ok && self.port.is_none_or(|p| site.port == Some(p))
    }
}

/// An entry as the owner typed it, cleaned: `https://www.Example.com/path` becomes
/// `www.example.com`. `None` with the reason when it is not a website.
pub fn clean_entry(entry: &str) -> Result<String, String> {
    let raw = entry.trim();
    if raw.is_empty() || raw.chars().count() > MAX_SITE_CHARS || raw.chars().any(char::is_control) {
        return Err(format!(
            "{raw:?} is not a website: give a name like example.com (at most {MAX_SITE_CHARS} \
             characters)"
        ));
    }
    let e = Entry::parse(raw).ok_or_else(|| {
        format!("{raw:?} is not a website: give a name like example.com or 127.0.0.1:8080")
    })?;
    Ok(match e.port {
        Some(p) => format!("{}:{p}", e.host),
        None => e.host,
    })
}

/// What the lists say about a website.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteVerdict {
    /// On the allowed list (the entry that matched).
    Allowed(String),
    /// On the blocked list (the entry that matched).
    Blocked(String),
    /// On neither list, and other websites ask.
    Ask,
    /// On neither list, and other websites are blocked.
    Other,
    /// An address on this computer or the local network that no allowed entry names.
    Local,
    /// `about:blank`: an empty page, always fine.
    Blank,
}

/// The lists' verdict about `site`: blocked entries first, then allowed ones.
pub fn check(rules: &WebsiteRules, site: &Site) -> SiteVerdict {
    if site.scheme == "about" {
        return SiteVerdict::Blank;
    }
    let find = |list: &[String]| {
        list.iter()
            .find(|e| Entry::parse(e).is_some_and(|x| x.matches(site)))
            .cloned()
    };
    if let Some(e) = find(&rules.blocked) {
        return SiteVerdict::Blocked(e);
    }
    if let Some(e) = find(&rules.allowed) {
        return SiteVerdict::Allowed(e);
    }
    if site.is_local() {
        return SiteVerdict::Local;
    }
    match rules.others {
        OtherSites::Ask => SiteVerdict::Ask,
        OtherSites::Block => SiteVerdict::Other,
    }
}

/// Clean both lists (no duplicates, a website on both lists is refused).
pub fn clean(rules: &WebsiteRules) -> Result<WebsiteRules, String> {
    let list = |what: &str, items: &[String]| -> Result<Vec<String>, String> {
        let mut out: Vec<String> = Vec::new();
        for item in items.iter().filter(|i| !i.trim().is_empty()) {
            let e = clean_entry(item)?;
            if !out.contains(&e) {
                out.push(e);
            }
        }
        if out.len() > MAX_SITES {
            return Err(format!("at most {MAX_SITES} {what}"));
        }
        Ok(out)
    };
    let allowed = list("allowed websites", &rules.allowed)?;
    let blocked = list("blocked websites", &rules.blocked)?;
    if let Some(both) = allowed.iter().find(|a| blocked.contains(a)) {
        return Err(format!(
            "{both} is on both lists; keep it on one (a blocked website is never opened)"
        ));
    }
    Ok(WebsiteRules {
        allowed,
        blocked,
        others: rules.others,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(allowed: &[&str], blocked: &[&str], others: OtherSites) -> WebsiteRules {
        WebsiteRules {
            allowed: allowed.iter().map(|s| (*s).to_owned()).collect(),
            blocked: blocked.iter().map(|s| (*s).to_owned()).collect(),
            others,
        }
    }

    fn site(a: &str) -> Site {
        Site::parse(a).unwrap()
    }

    #[test]
    fn addresses_are_read_and_non_websites_refused() {
        let s = site("HTTPS://Shop.Example.COM:8443/cart?x=1");
        assert_eq!(
            (s.host.as_str(), s.port, s.scheme.as_str()),
            ("shop.example.com", Some(8443), "https")
        );
        assert_eq!(site("example.com").url, "https://example.com/");
        assert_eq!(site("about:blank").scheme, "about");
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "chrome://settings",
            "edge://settings",
            "data:text/html,hi",
            "ftp://example.com",
            "https://user:pw@example.com/",
            "view-source:https://example.com",
            "",
        ] {
            assert!(Site::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn lists_match_hosts_subdomains_and_ports() {
        let r = rules(
            &["example.com", "127.0.0.1:8080"],
            &["ads.example.com", "linkedin.com"],
            OtherSites::Ask,
        );
        assert_eq!(
            check(&r, &site("https://www.example.com/a")),
            SiteVerdict::Allowed("example.com".into())
        );
        assert_eq!(
            check(&r, &site("https://ads.example.com/")),
            SiteVerdict::Blocked("ads.example.com".into()),
            "blocked wins"
        );
        assert_eq!(
            check(&r, &site("https://notexample.com/")),
            SiteVerdict::Ask,
            "a suffix is not a subdomain"
        );
        assert_eq!(
            check(&r, &site("https://www.linkedin.com/in/x")),
            SiteVerdict::Blocked("linkedin.com".into())
        );
        assert_eq!(
            check(&r, &site("http://127.0.0.1:8080/form")),
            SiteVerdict::Allowed("127.0.0.1:8080".into())
        );
        assert_eq!(
            check(&r, &site("http://127.0.0.1:9090/")),
            SiteVerdict::Local,
            "another port of a local address is not allowed"
        );
        let strict = rules(&[], &[], OtherSites::Block);
        assert_eq!(check(&strict, &site("example.org")), SiteVerdict::Other);
        assert_eq!(check(&strict, &site("about:blank")), SiteVerdict::Blank);
    }

    #[test]
    fn local_addresses_need_an_allowed_entry() {
        let r = rules(&[], &[], OtherSites::Ask);
        for local in [
            "http://localhost:3000/",
            "http://app.localhost/",
            "http://192.168.1.1/",
            "http://10.0.0.5/",
            "http://172.20.1.1/",
            "http://169.254.1.1/",
            "http://100.100.1.1/",
            "http://[::1]/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://router.local/",
            "http://0.0.0.0/",
        ] {
            assert_eq!(check(&r, &site(local)), SiteVerdict::Local, "{local}");
        }
        assert_eq!(check(&r, &site("http://8.8.8.8/")), SiteVerdict::Ask);
        let r = rules(&["localhost"], &[], OtherSites::Ask);
        assert_eq!(
            check(&r, &site("http://localhost:3000/")),
            SiteVerdict::Allowed("localhost".into())
        );
    }

    #[test]
    fn entries_are_cleaned_and_validated() {
        assert_eq!(
            clean_entry("https://WWW.Example.com/path").unwrap(),
            "www.example.com"
        );
        assert_eq!(clean_entry("*.example.com").unwrap(), "example.com");
        assert_eq!(clean_entry("127.0.0.1:8080").unwrap(), "127.0.0.1:8080");
        for bad in ["", "file:///x", "not a site", "javascript:x"] {
            assert!(clean_entry(bad).is_err(), "{bad}");
        }
        let c = clean(&rules(
            &["Example.com", "example.com", " "],
            &["x.com"],
            OtherSites::Block,
        ))
        .unwrap();
        assert_eq!(c.allowed, ["example.com"]);
        assert_eq!(c.others, OtherSites::Block);
        assert!(clean(&rules(&["x.com"], &["X.com"], OtherSites::Ask)).is_err());
    }
}
