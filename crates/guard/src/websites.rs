//! The owner's website lists (Phase 10, ADR-020): the websites workers may open in Plenipo's
//! browser without asking, the ones they may never open, and what happens with every other
//! website (ask the owner, or blocked). Addresses on this computer or the local network open
//! only when a list allows them by name.
//!
//! It also says how a web address is recorded ([`safe_address`]): its website and page, with
//! what follows `?` or `#` left out (only the names of a query's fields are kept).

use std::borrow::Cow;
use std::net::IpAddr;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most entries in each list, and the longest entry.
pub const MAX_SITES: usize = 300;
pub const MAX_SITE_CHARS: usize = 260;
/// Most characters of a recorded address ([`safe_address`]).
pub const MAX_ADDRESS_CHARS: usize = 200;
/// Longest query field name kept in a recorded address; a longer or odd one shows as `…`.
const MAX_FIELD_NAME_CHARS: usize = 24;
/// Most digits in a kept field name: a name full of digits is more likely a key than a name.
const MAX_FIELD_NAME_DIGITS: usize = 4;

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

/// A web address as Plenipo records it (the Ledger, approval cards, the control center,
/// screenshot records) and shows it to the owner: its website and page. Search terms, sign-in
/// tokens, session keys, and the like belong to the page and the moment, not to a record kept
/// for good, so what follows `?` keeps only its fields' names (`?to=…&amount=…`: the owner still
/// sees what kind of data the address carries), what follows `#` shows as `#…`, a page path's
/// own settings (`;jsessionid=…`) keep only their name, and a user name or password is
/// dropped. The address is cut as written, never rebuilt, so the secrets filter still finds a
/// stored secret in what is left. Cut short at [`MAX_ADDRESS_CHARS`].
pub fn safe_address(address: &str) -> String {
    let address = address.trim();
    // Only what comes before `?` or `#` can name the scheme; an address with none (as a worker
    // may type one) starts with its website, which may carry a user name and password too.
    let before_query = &address[..address.find(['?', '#']).unwrap_or(address.len())];
    let scheme = before_query.find("://").filter(|&i| {
        let s = &address[..i];
        s.starts_with(|c: char| c.is_ascii_alphabetic())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    let (head, after) = match scheme {
        Some(i) => (&address[..i + 3], &address[i + 3..]),
        None if before_query.contains('@') => ("", address),
        None => ("", ""),
    };
    let (mut kept, rest) = if head.is_empty() && after.is_empty() {
        (String::new(), address)
    } else {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..end];
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        (format!("{head}{host}"), &after[end..])
    };
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path: Vec<String> = path
        .split('/')
        .map(|part| match part.split_once(';') {
            Some((name, _)) => format!("{name};…"),
            None => part.to_owned(),
        })
        .collect();
    kept.push_str(&path.join("/"));
    let fields: Vec<String> = query
        .split('&')
        .filter(|field| !field.is_empty())
        .map(|field| {
            // A name is kept only for a field with a value: a bare token (`?dG9r…=`) is not one.
            let (name, value) = field.split_once('=').unwrap_or(("", ""));
            let plain = !name.is_empty()
                && !value.is_empty()
                && name.chars().count() <= MAX_FIELD_NAME_CHARS
                && name.chars().filter(char::is_ascii_digit).count() <= MAX_FIELD_NAME_DIGITS
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-.[]".contains(c));
            if plain {
                format!("{name}=…")
            } else {
                "…".to_owned()
            }
        })
        .collect();
    if !fields.is_empty() {
        kept.push('?');
        kept.push_str(&fields.join("&"));
    }
    if !fragment.is_empty() {
        kept.push_str("#…");
    }
    if kept.chars().count() <= MAX_ADDRESS_CHARS {
        return kept;
    }
    let mut cut: String = kept.chars().take(MAX_ADDRESS_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// `text` with every web address in it (any `scheme://…`, up to a space or a quote) as
/// [`safe_address`] records it. Sentence marks right after an address, and a closing bracket
/// the address did not open, stay in the text.
pub fn safe_addresses(text: &str) -> Cow<'_, str> {
    static ADDRESS: OnceLock<Regex> = OnceLock::new();
    let re = ADDRESS.get_or_init(|| {
        Regex::new(r#"(?i)\b[a-z][a-z0-9+.-]*://[^\s"'<>`]+"#).expect("valid address pattern")
    });
    if !re.is_match(text) {
        return Cow::Borrowed(text);
    }
    re.replace_all(text, |caps: &regex::Captures<'_>| {
        let found = &caps[0];
        let mut address = found;
        loop {
            let unopened = |open: char, close: char| {
                address.ends_with(close)
                    && address.matches(open).count() < address.matches(close).count()
            };
            if address.ends_with(['.', ',', ';', ':', '!', '?'])
                || unopened('(', ')')
                || unopened('[', ']')
                || unopened('{', '}')
            {
                address = &address[..address.len() - 1];
            } else {
                break;
            }
        }
        format!("{}{}", safe_address(address), &found[address.len()..])
    })
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

    /// A recorded address keeps the website and its page: of what follows `?`, only its fields'
    /// names; of what follows `#`, only a mark; never a user name or password. Addresses inside
    /// a sentence are cleaned the same way, whatever characters their query holds.
    #[test]
    fn a_recorded_address_keeps_the_page_and_drops_the_rest() {
        for (address, recorded) in [
            (
                "https://shop.test/cart?session=abc123&q=mug#top",
                "https://shop.test/cart?session=…&q=…#…",
            ),
            (
                "http://mail.test:8080/inbox/?token=xyz",
                "http://mail.test:8080/inbox/?token=…",
            ),
            ("https://shop.test", "https://shop.test"),
            ("https://shop.test/a?", "https://shop.test/a"),
            (
                "https://user:pa55word@shop.test/private?x=1",
                "https://shop.test/private?x=…",
            ),
            (
                "https://app.test/items?page[size]=10&access_token=abc",
                "https://app.test/items?page[size]=…&access_token=…",
            ),
            (
                "https://app.test/cb?eyJhbGciOiJIUzI1NiJ9",
                "https://app.test/cb?…",
            ),
            (
                "https://shop.test/cart;jsessionid=ABC123/view?x=1",
                "https://shop.test/cart;…/view?x=…",
            ),
            (
                "http://[::1]:3000/cb?code=q1w2",
                "http://[::1]:3000/cb?code=…",
            ),
            ("about:blank", "about:blank"),
            (
                "javascript:alert(document.cookie)#x",
                "javascript:alert(document.cookie)#…",
            ),
            ("not a url?secret=1", "not a url?secret=…"),
            // As a worker may type it: no scheme, a user name and password, a query.
            ("admin:hunter2@192.168.1.1/", "192.168.1.1/"),
            (
                "login.test/cb?next=https://app.test/&code=S3CR3T",
                "login.test/cb?next=…&code=…",
            ),
            // A field name that looks like a key is not kept.
            (
                "https://x.test/v?dG9rZW4xMjM0NTY3ODk=",
                "https://x.test/v?…",
            ),
            (
                "https://x.test/v?5f2b8c90d1e3a4b6c7d8e9f0a1b2c3d4=1",
                "https://x.test/v?…",
            ),
            (
                "https://x.test/v?utm_campaign=fall&page2=3",
                "https://x.test/v?utm_campaign=…&page2=…",
            ),
        ] {
            assert_eq!(safe_address(address), recorded, "{address}");
        }
        let long = format!("https://shop.test/{}", "a".repeat(400));
        let cut = safe_address(&long);
        assert_eq!(cut.chars().count(), MAX_ADDRESS_CHARS);
        assert!(cut.ends_with('…'));
        assert_eq!(
            safe_addresses(
                "Opened \"Cart\" (https://shop.test/cart?session=abc123). Then \
                 http://mail.test/in?token=xyz, https://shop.test/plain, \
                 https://app.test/items?page[size]=10&access_token=abc, \
                 ftp://files.test/get?key=k3y and https://wiki.test/Foo_(bar)?id=9."
            ),
            "Opened \"Cart\" (https://shop.test/cart?session=…). Then \
             http://mail.test/in?token=…, https://shop.test/plain, \
             https://app.test/items?page[size]=…&access_token=…, \
             ftp://files.test/get?key=… and https://wiki.test/Foo_(bar)?id=…."
        );
        assert!(matches!(
            safe_addresses("no address here"),
            Cow::Borrowed(_)
        ));
    }

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
