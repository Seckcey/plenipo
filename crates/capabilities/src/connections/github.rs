//! GitHub (Phase 25, ADR-204): a read-only list of the owner's GitHub repositories, for project
//! setup. 8 West's GitHub App asks for **Metadata: read** and nothing else, ever: the names and
//! descriptions of repositories, their branch and tag names, and who collaborates on them, never
//! their code. It signs in with a short code the owner types on GitHub's own page (the device
//! flow), which needs no secret, so this copy of Plenipo keeps none; the long-lived sign-in is
//! renewed without one too. Nothing here is ever given to a worker: GitHub has no parts and no
//! tools, and its requests start only from the owner's own screens.
//!
//! Workers keep their own GitHub sign-in, through GitHub's `gh` program and git (ADR-016 §6),
//! unchanged. This connection can't read code, push, or change anything.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Where a short code is asked for.
pub const DEVICE_CODE: &str = "https://github.com/login/device/code";
/// Where the code is traded for the sign-in, and the sign-in renewed.
pub const TOKEN: &str = "https://github.com/login/oauth/access_token";
/// The page where the owner types the code, the only one Plenipo ever shows for it.
pub const DEVICE_PAGE: &str = "https://github.com/login/device";
/// GitHub's web interface.
pub const API: &str = "https://api.github.com";
/// Where the owner removes Plenipo's sign-in at GitHub.
pub const AUTHORIZATIONS_PAGE: &str = "https://github.com/settings/apps/authorizations";
/// Where the owner sees, and removes, the accounts Plenipo may list.
pub const INSTALLATIONS_PAGE: &str = "https://github.com/settings/installations";
/// What the sign-in is allowed (ADR-204, the reviewer's G4): this, forever.
pub const GRANTED: &str = "metadata:read";
/// GitHub's own grant type for a short code.
pub const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// The headers every request to GitHub's web interface carries.
pub const API_HEADERS: [(&str, &str); 2] = [
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];
/// GitHub answers its sign-in addresses in a form unless asked for JSON.
pub const ACCEPT_JSON: [(&str, &str); 1] = [("Accept", "application/json")];
/// The most repositories listed (100 a page).
pub const MAX_REPOSITORIES: usize = 1_000;
/// The most accounts listed.
pub const MAX_ACCOUNTS: usize = 100;
/// The words the card shows with the code (ADR-204, the reviewer's G4), exactly.
pub const ONLY_THIS_CODE: &str = "Only type a code Plenipo just showed you here.";

/// A GitHub code is waiting in some organization's window: one at a time on this PC (ADR-204,
/// the reviewer's G4).
pub(crate) static WAITING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// A GitHub App's client ID, as GitHub writes it (`Iv1.0123456789abcdef`, or the newer
/// `Iv23li…`): letters, digits, and dots, at most 40.
pub fn is_client_id(s: &str) -> bool {
    (10..=40).contains(&s.len())
        && s.starts_with("Iv")
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// A GitHub App's short name in its addresses (`plenipo-by-8-west`).
pub fn is_app_slug(s: &str) -> bool {
    (1..=100).contains(&s.len())
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The page where the owner chooses the accounts Plenipo may list.
pub fn install_page(slug: &str) -> String {
    format!("https://github.com/apps/{slug}/installations/new")
}

/// A sign-in that renews: GitHub's refresh tokens start `ghr_`. One that doesn't is a sign-in
/// that doesn't expire (an app with expiring sign-ins switched off), kept and used as it is.
pub fn renews(long_lived: &str) -> bool {
    long_lived.starts_with("ghr_")
}

/// A short code waiting for the owner, as GitHub gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    /// What Plenipo trades, never shown.
    pub device_code: String,
    /// What the owner types (`WDJB-MJHT`).
    pub user_code: String,
    /// Seconds it lasts.
    pub expires_in: u64,
    /// Seconds between Plenipo's asks.
    pub interval: u64,
}

/// GitHub's answer to "a short code, please": refused unless it sends the owner to GitHub's own
/// code page (a code is never shown with any other page), and the code is a plain short code.
pub fn read_code(answer: &Value) -> Result<Code, String> {
    if let Some(error) = answer["error"].as_str() {
        return Err(refusal_words(error));
    }
    let text = |k: &str| answer[k].as_str().filter(|s| !s.is_empty());
    let device_code = text("device_code").ok_or("GitHub sent no code to wait for.")?;
    let user_code = text("user_code").ok_or("GitHub sent no code for you to type.")?;
    let page = text("verification_uri").unwrap_or_default();
    if page.trim_end_matches('/') != DEVICE_PAGE {
        return Err("GitHub sent Plenipo a code for another page, so it isn't shown.".into());
    }
    let plain = (4..=20).contains(&user_code.len())
        && user_code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-');
    if !plain || device_code.len() > 200 {
        return Err("GitHub sent a code Plenipo doesn't recognise, so it isn't shown.".into());
    }
    Ok(Code {
        device_code: device_code.to_owned(),
        user_code: user_code.to_owned(),
        expires_in: answer["expires_in"].as_u64().unwrap_or(900).clamp(60, 900),
        interval: answer["interval"].as_u64().unwrap_or(5).clamp(1, 60),
    })
}

/// Where one ask for the sign-in stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    /// The owner hasn't typed the code yet.
    Pending,
    /// Ask less often: GitHub adds five seconds.
    SlowDown,
    /// Signed in.
    Done {
        access: String,
        /// `None`: a sign-in that doesn't expire.
        refresh: Option<String>,
        expires_in: Option<u64>,
    },
    /// It ended, and why, in plain words.
    Ended(String),
}

/// GitHub's answer to one ask (it answers 200 with an `error` while it waits).
pub fn read_poll(answer: &Value) -> Poll {
    match answer["error"].as_str() {
        Some("authorization_pending") => Poll::Pending,
        Some("slow_down") => Poll::SlowDown,
        Some(other) => Poll::Ended(refusal_words(other)),
        None => match answer["access_token"].as_str().filter(|t| !t.is_empty()) {
            Some(access) => Poll::Done {
                access: access.to_owned(),
                refresh: answer["refresh_token"]
                    .as_str()
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned),
                expires_in: answer["expires_in"].as_u64(),
            },
            None => Poll::Ended("GitHub sent no sign-in.".into()),
        },
    }
}

/// GitHub's sign-in errors, in plain words.
pub fn refusal_words(error: &str) -> String {
    match error {
        "expired_token" => {
            "The code ran out before it was typed. Press Sign in with GitHub to get a new one."
                .into()
        }
        "access_denied" => "You said no on GitHub's page, so nothing was kept.".into(),
        "device_flow_disabled" => {
            "8 West's GitHub app doesn't allow signing in with a code. Nothing was kept.".into()
        }
        "incorrect_client_credentials" | "unsupported_grant_type" | "incorrect_device_code" => {
            "GitHub didn't accept Plenipo's sign-in. Nothing was kept.".into()
        }
        other => format!(
            "GitHub answered {}. Nothing was kept.",
            other
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .take(40)
                .collect::<String>()
        ),
    }
}

/// A renewal GitHub refused because the sign-in is gone: the owner signs in again.
pub fn sign_in_gone(error: &str) -> bool {
    matches!(
        error,
        "bad_refresh_token" | "incorrect_client_credentials" | "unauthorized_client"
    )
}

/// Whether an installation's permissions are exactly what Plenipo's GitHub App asks for:
/// **Metadata: read** and nothing more (ADR-204, the reviewer's G4). An installation that shows
/// more is refused: the app was changed, and that is a new decision, never a setting.
pub fn only_metadata(permissions: &Value) -> bool {
    match permissions.as_object() {
        Some(p) => p
            .iter()
            .all(|(k, v)| k == "metadata" && v.as_str() == Some("read")),
        None => false,
    }
}

/// An account (the owner's own, or an organization) Plenipo may list the repositories of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubAccount {
    /// `frankieg`, `8west`.
    pub login: String,
    /// An organization (not a person's own account).
    pub organization: bool,
    /// Every repository, or only the ones the owner picked on GitHub.
    pub all_repositories: bool,
    /// Why Plenipo doesn't list it, in plain words (its permissions are more than it asks for).
    #[ts(optional)]
    pub refused: Option<String>,
}

/// One repository, as the picker shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubRepository {
    /// Its account: `8west`.
    pub owner: String,
    /// `plenipo`.
    pub name: String,
    pub private: bool,
    /// Its short description, at most 200 characters.
    #[ts(optional)]
    pub description: Option<String>,
    /// When it last changed (GitHub's `updated_at`, RFC 3339).
    #[ts(optional)]
    pub updated_at: Option<String>,
}

impl GithubRepository {
    /// The address Plenipo keeps for it, always built from its checked owner and name, never
    /// copied from GitHub's text (ADR-204, the reviewer's G1).
    pub fn address(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.name)
    }
}

/// What the owner may pick from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubRepositories {
    pub accounts: Vec<GithubAccount>,
    /// Sorted by account (the owner's own first), then by name.
    pub repositories: Vec<GithubRepository>,
    /// More than [`MAX_REPOSITORIES`]: the rest aren't listed.
    pub more: bool,
    /// The page where the owner chooses accounts (`None`: this copy has no app name for it).
    #[ts(optional)]
    pub install_page: Option<String>,
}

/// A GitHub account's or repository's name: letters, digits, `-`, `_`, and `.`, never `.` or `..`
/// alone, at most 100 characters.
pub fn plain_name(s: &str) -> bool {
    (1..=100).contains(&s.len())
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// One repository from GitHub's answer, kept only with a plain owner and name.
pub fn read_repository(v: &Value) -> Option<GithubRepository> {
    let owner = v["owner"]["login"].as_str().filter(|s| plain_name(s))?;
    let name = v["name"].as_str().filter(|s| plain_name(s))?;
    let description = v["description"]
        .as_str()
        .map(|d| {
            d.chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect::<String>()
        })
        .filter(|d| !d.trim().is_empty());
    let updated_at = v["updated_at"]
        .as_str()
        .filter(|t| {
            t.len() <= 40
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || ":-.+".contains(c))
        })
        .map(str::to_owned);
    Some(GithubRepository {
        owner: owner.to_owned(),
        name: name.to_owned(),
        private: v["private"].as_bool().unwrap_or(true),
        description,
        updated_at,
    })
}

/// One installation from GitHub's answer: the account, and whether Plenipo may list it.
pub fn read_installation(v: &Value) -> Option<(u64, GithubAccount)> {
    let id = v["id"].as_u64()?;
    let login = v["account"]["login"].as_str().filter(|s| plain_name(s))?;
    let refused = (!only_metadata(&v["permissions"])).then(|| {
        "Plenipo's GitHub app shows more permissions here than it asks for, so Plenipo doesn't \
         list this account. Remove Plenipo from it on GitHub, and tell 8 West Ventures."
            .to_owned()
    });
    Some((
        id,
        GithubAccount {
            login: login.to_owned(),
            organization: v["account"]["type"].as_str() == Some("Organization"),
            all_repositories: v["repository_selection"].as_str() == Some("all"),
            refused,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn client_ids_and_app_names_are_github_s_own_kinds() {
        assert!(is_client_id("Iv1.0123456789abcdef"));
        assert!(is_client_id("Iv23liAbCdEfGhIjKlMn"));
        for bad in ["", "abc", "Iv1.", "Iv1.0123/456789", "Ov1.0123456789abcdef"] {
            assert!(!is_client_id(bad), "{bad}");
        }
        assert!(is_app_slug("plenipo-by-8-west"));
        for bad in ["", "-x", "Plenipo", "a/b", "a b"] {
            assert!(!is_app_slug(bad), "{bad}");
        }
        assert_eq!(
            install_page("plenipo"),
            "https://github.com/apps/plenipo/installations/new"
        );
    }

    /// A code is shown only with GitHub's own code page (ADR-204, G4).
    #[test]
    fn a_code_is_shown_only_for_githubs_own_page() {
        let good = json!({
            "device_code": "dc", "user_code": "WDJB-MJHT",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900, "interval": 5,
        });
        let code = read_code(&good).unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        assert_eq!((code.expires_in, code.interval), (900, 5));
        let mut elsewhere = good.clone();
        elsewhere["verification_uri"] = json!("https://evil.example/login/device");
        assert!(read_code(&elsewhere).is_err());
        let mut odd = good.clone();
        odd["user_code"] = json!("<script>");
        assert!(read_code(&odd).is_err());
        assert!(read_code(&json!({ "error": "device_flow_disabled" })).is_err());
    }

    #[test]
    fn each_ask_is_read_as_github_answers_it() {
        assert_eq!(
            read_poll(&json!({ "error": "authorization_pending" })),
            Poll::Pending
        );
        assert_eq!(read_poll(&json!({ "error": "slow_down" })), Poll::SlowDown);
        assert!(
            matches!(read_poll(&json!({ "error": "expired_token" })), Poll::Ended(w) if w.contains("ran out"))
        );
        assert!(matches!(
            read_poll(&json!({ "error": "access_denied" })),
            Poll::Ended(_)
        ));
        assert_eq!(
            read_poll(
                &json!({ "access_token": "ghu_a", "refresh_token": "ghr_b", "expires_in": 28800 })
            ),
            Poll::Done {
                access: "ghu_a".into(),
                refresh: Some("ghr_b".into()),
                expires_in: Some(28800)
            }
        );
        assert!(renews("ghr_b") && !renews("ghu_a"));
    }

    /// The reviewer's G4: Metadata: read, and nothing more.
    #[test]
    fn only_metadata_read_is_accepted() {
        assert!(only_metadata(&json!({ "metadata": "read" })));
        assert!(!only_metadata(
            &json!({ "metadata": "read", "contents": "read" })
        ));
        assert!(!only_metadata(&json!({ "metadata": "write" })));
        assert!(!only_metadata(&json!({ "emails": "read" })));
        assert!(!only_metadata(&json!(null)));
        let (_, refused) = read_installation(&json!({
            "id": 7, "account": { "login": "8west", "type": "Organization" },
            "repository_selection": "all",
            "permissions": { "metadata": "read", "contents": "write" },
        }))
        .unwrap();
        assert!(refused.refused.is_some() && refused.organization && refused.all_repositories);
    }

    /// The reviewer's G1: a repository's address is built from a plain owner and name.
    #[test]
    fn repositories_are_kept_only_with_plain_names() {
        let r = read_repository(&json!({
            "name": "plenipo", "owner": { "login": "8west" }, "private": true,
            "description": "The app\u{0007}", "updated_at": "2026-10-05T12:00:00Z",
        }))
        .unwrap();
        assert_eq!(r.address(), "https://github.com/8west/plenipo");
        assert_eq!(r.description.as_deref(), Some("The app"));
        for (owner, name) in [("8west", ".."), ("a/b", "x"), ("8west", "x?y"), ("", "x")] {
            assert!(
                read_repository(&json!({ "name": name, "owner": { "login": owner } })).is_none(),
                "{owner}/{name}"
            );
        }
    }
}
