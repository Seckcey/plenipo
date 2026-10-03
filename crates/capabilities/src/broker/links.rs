//! Whether a link named in a worker's answer really exists (Phase 25, item 4.8; ADR-257, catch
//! made-up answers, step 2). Plenipo asks only where it can trust the answer, and only where the
//! worker that wrote it could have looked itself, without asking (the security review of #156):
//! the worker wrote the link, so a look must never reach further than the worker could.
//!
//! - **A pull request on GitHub:** GitHub's own `gh` answers, signed in as the owner, the way
//!   Plenipo's GitHub tools reach GitHub (ADR-016), and only in the one repository the worker's
//!   GitHub tools act on, when its permissions let it read GitHub. Without `gh`, or when it
//!   cannot tell, the link is not checked.
//! - **Any other link:** only when the worker's permissions let it open websites without asking,
//!   and only a website on the owner's allowed list, through Guard
//!   ([`plenipo_guard::Guard::link_to_check`]: https, no query, Plenipo's browser on): one
//!   request, no redirect followed, nothing sent but the address. "Not found" (404 or 410) means
//!   it doesn't exist; any other answer means it does, or that Plenipo cannot tell.
//!
//! Nothing a link returns is read or kept.

use std::time::Duration;

use super::Broker;
use crate::programs::{self, Run};
use crate::vault;

/// The longest one look may take (the app also stops waiting for all of an answer's looks
/// together after a few seconds).
const TIMEOUT: Duration = Duration::from_secs(5);

/// What Plenipo found about a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkVerdict {
    Exists,
    Missing,
    /// Plenipo did not, or could not, look (why).
    NotChecked(String),
}

/// Where the worker that wrote an answer could look itself, without asking: from the grant of
/// its last step, as the Ledger recorded it (`guard.grant_opened`).
#[derive(Debug, Default)]
struct Reach {
    /// It could open websites (on the owner's lists).
    websites: bool,
    /// The repository its GitHub tools act on, when it could read GitHub.
    github: Option<String>,
}

impl Broker {
    /// Whether `address`, named in the answer of the task `task_id`, exists.
    pub async fn check_link(&self, task_id: &str, address: &str) -> LinkVerdict {
        let reach = match self.ledger().last_task_event(task_id, "guard.grant_opened") {
            Ok(Some(grant)) => reach_from(&grant.payload),
            _ => Reach::default(),
        };
        if let Some((link, _)) = crate::github::pull_request_link(address) {
            if !reach.reaches_pull_request(&link) {
                return LinkVerdict::NotChecked(
                    "its worker's GitHub tools don't reach that repository".into(),
                );
            }
            return self.check_pull_request(&link).await;
        }
        if !reach.websites {
            return LinkVerdict::NotChecked("its worker can't open websites".into());
        }
        let site = match self.guard().link_to_check(address) {
            Ok(site) => site,
            Err(why) => return LinkVerdict::NotChecked(why),
        };
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Plenipo/", env!("CARGO_PKG_VERSION")))
            .timeout(TIMEOUT)
            .build()
        {
            Ok(c) => c,
            Err(e) => return LinkVerdict::NotChecked(format!("no connection ({e})")),
        };
        let mut answer = client.head(&site.url).send().await;
        // Some websites refuse HEAD; ask once more for the page itself (its body is never read).
        if matches!(&answer, Ok(r) if matches!(r.status().as_u16(), 405 | 501)) {
            answer = client.get(&site.url).send().await;
        }
        match answer {
            Ok(r) => verdict_for(r.status().as_u16()),
            Err(e) => LinkVerdict::NotChecked(format!("the website did not answer ({e})")),
        }
    }

    /// A pull request link, through GitHub's `gh` signed in as the owner.
    async fn check_pull_request(&self, address: &str) -> LinkVerdict {
        let Some(gh) = self.find_program("gh") else {
            return LinkVerdict::NotChecked("GitHub's gh is not installed".into());
        };
        let mut env = programs::dev_env();
        env.extend(programs::gh_env());
        // The owner's GitHub secret for gh, as Plenipo's GitHub tools get it.
        if let Ok(config) = self.guard().config() {
            let given = super::secrets_for("gh", super::Origin::Path, &config.secrets);
            for g in &given.give {
                if let Some(value) = vault::read(self.inner.store.as_ref(), &g.id).ok().flatten() {
                    env.push((g.var.clone(), value));
                }
            }
        }
        let dir = std::env::temp_dir();
        let ran = programs::run(
            self.supervisor(),
            Run {
                label: "Plenipo · check a pull request".into(),
                executable: gh,
                args: vec![
                    "pr".into(),
                    "view".into(),
                    address.into(),
                    "--json".into(),
                    "number".into(),
                ],
                working_dir: &dir,
                env,
                stdin: None,
                timeout: TIMEOUT,
            },
            |_| {},
        )
        .await;
        match ran {
            Ok(ran) if ran.succeeded() => LinkVerdict::Exists,
            Ok(ran) => {
                let out = ran.output.to_lowercase();
                if out.contains("could not resolve to a pullrequest")
                    || out.contains("no pull requests found")
                {
                    LinkVerdict::Missing
                } else {
                    LinkVerdict::NotChecked("GitHub's gh could not tell".into())
                }
            }
            Err(why) => LinkVerdict::NotChecked(why),
        }
    }
}

/// Where a worker could look without asking, from its step's grant as the Ledger recorded it
/// (`guard.grant_opened`): only permissions it had at "allowed" count, since a look on "ask"
/// would have needed the owner's yes.
fn reach_from(grant: &serde_json::Value) -> Reach {
    let allowed = |ids: &[&str]| {
        ids.iter()
            .any(|id| grant["permissions"][*id].as_str() == Some("allowed"))
    };
    Reach {
        websites: allowed(&["browser.navigate", "browser.automate"]),
        github: allowed(&["github.read", "github.write"])
            .then(|| grant["github"].as_str().map(str::to_owned))
            .flatten(),
    }
}

impl Reach {
    /// The pull request `link` is in the one repository its GitHub tools act on.
    fn reaches_pull_request(&self, link: &str) -> bool {
        match (self.github.as_deref(), repository_of(link)) {
            (Some(its), Some(named)) => its.eq_ignore_ascii_case(&named),
            _ => false,
        }
    }
}

/// What a website's answer says about a page: "not found" (404 or 410) means it doesn't exist.
fn verdict_for(status: u16) -> LinkVerdict {
    match status {
        404 | 410 => LinkVerdict::Missing,
        200..=399 | 401 | 403 => LinkVerdict::Exists,
        other => LinkVerdict::NotChecked(format!("the website answered {other}")),
    }
}

/// `owner/name` of a GitHub pull request link (`https://github.com/owner/name/pull/7`).
fn repository_of(link: &str) -> Option<String> {
    let rest = link.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    crate::github::repo_of(&format!(
        "https://github.com/{}/{}",
        parts.next()?,
        parts.next()?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_not_found_means_a_page_doesnt_exist() {
        assert_eq!(verdict_for(404), LinkVerdict::Missing);
        assert_eq!(verdict_for(410), LinkVerdict::Missing);
        for exists in [200, 204, 301, 302, 401, 403] {
            assert_eq!(verdict_for(exists), LinkVerdict::Exists, "{exists}");
        }
        for unsure in [400, 429, 500, 503] {
            assert!(
                matches!(verdict_for(unsure), LinkVerdict::NotChecked(_)),
                "{unsure}"
            );
        }
    }

    #[test]
    fn a_pull_requests_repository_is_read_from_its_link() {
        assert_eq!(
            repository_of("https://github.com/Seckcey/plenipo/pull/156").as_deref(),
            Some("Seckcey/plenipo")
        );
        assert_eq!(repository_of("https://example.com/o/r/pull/1"), None);
    }

    /// After the security review of #156: a pull request is looked at only in the repository the
    /// worker's GitHub tools act on, and only when it could read GitHub without asking; a website
    /// only when it could open websites without asking.
    #[test]
    fn a_look_reaches_only_where_the_worker_could_look_without_asking() {
        let grant = |permissions: serde_json::Value, github: Option<&str>| {
            reach_from(&serde_json::json!({ "permissions": permissions, "github": github }))
        };
        let pr = "https://github.com/acme/website/pull/7";
        let reader = grant(
            serde_json::json!({ "github.read": "allowed" }),
            Some("Acme/Website"),
        );
        assert!(
            reader.reaches_pull_request(pr),
            "the same repository, any case"
        );
        assert!(!reader.reaches_pull_request("https://github.com/acme/other/pull/7"));
        assert!(!reader.websites);
        // Asking first, or no repository: no.
        assert!(!grant(
            serde_json::json!({ "github.read": "ask" }),
            Some("acme/website")
        )
        .reaches_pull_request(pr));
        assert!(
            !grant(serde_json::json!({ "github.read": "allowed" }), None).reaches_pull_request(pr)
        );
        // Websites: only "allowed" counts.
        assert!(grant(serde_json::json!({ "browser.navigate": "allowed" }), None).websites);
        assert!(grant(serde_json::json!({ "browser.automate": "allowed" }), None).websites);
        assert!(!grant(serde_json::json!({ "browser.navigate": "ask" }), None).websites);
        assert!(!grant(serde_json::json!({ "filesystem.read": "allowed" }), None).websites);
        // No grant on record: nowhere.
        let none = Reach::default();
        assert!(!none.websites && !none.reaches_pull_request(pr));
    }
}
