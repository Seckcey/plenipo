//! Whether a link named in a worker's answer really exists (Phase 25, item 4.8; ADR-257, catch
//! made-up answers, step 2). Plenipo asks only where it can trust the answer:
//!
//! - **A pull request on GitHub:** GitHub's own `gh` answers, signed in as the owner, the way
//!   Plenipo's GitHub tools reach GitHub (ADR-016). Without `gh`, or when it cannot tell, the
//!   link is not checked.
//! - **Any other link:** only a website on the owner's allowed list, through Guard
//!   ([`plenipo_guard::Guard::link_to_check`]): one request, no redirect followed, nothing sent but
//!   the address. "Not found" (404 or 410) means it doesn't exist; any other answer means it does,
//!   or that Plenipo cannot tell.
//!
//! Nothing a link returns is read or kept.

use std::time::Duration;

use super::Broker;
use crate::programs::{self, Run};
use crate::vault;

/// The longest one check may take.
const TIMEOUT: Duration = Duration::from_secs(10);

/// What Plenipo found about a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkVerdict {
    Exists,
    Missing,
    /// Plenipo did not, or could not, look (why).
    NotChecked(String),
}

impl Broker {
    /// Whether `address`, named in a worker's answer, exists.
    pub async fn check_link(&self, address: &str) -> LinkVerdict {
        if crate::github::pull_request_link(address).is_some() {
            return self.check_pull_request(address).await;
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
            Ok(r) => match r.status().as_u16() {
                404 | 410 => LinkVerdict::Missing,
                200..=399 | 401 | 403 => LinkVerdict::Exists,
                other => LinkVerdict::NotChecked(format!("the website answered {other}")),
            },
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
