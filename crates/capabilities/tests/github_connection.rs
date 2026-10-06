//! The read-only GitHub connection (Phase 25, ADR-204), against the stand-in GitHub: signing in
//! with a short code, the list of repositories, renewing without a secret, and every refusal the
//! reviewer asked for (G2: no redirects, the token only to api.github.com; G4: one sign-in at a
//! time, Metadata: read only).

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use plenipo_capabilities::connections::{Connections, ConnectionsConfig, Opener};
use plenipo_capabilities::{MemorySecretStore, SecretStore};
use plenipo_guard::{AccountKind, ConnectionState, Guard};
use plenipo_ledger::Ledger;
use support::github;
use support::microsoft::StandIn;

const ID: &str = "github";
const VAULT_ID: &str = "connection-github-token";
const WAIT: Duration = Duration::from_secs(30);

/// GitHub's short-code sign-in is one at a time on a PC: these tests take turns.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct NoBrowser;

impl Opener for NoBrowser {
    fn open(
        &self,
        _: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

struct H {
    ledger: Arc<Ledger>,
    guard: Guard,
    store: Arc<MemorySecretStore>,
    conns: Arc<Connections>,
    ms: StandIn,
}

async fn harness_with(client_id: Option<&str>) -> H {
    let ms = StandIn::start().await;
    let ledger = Arc::new(Ledger::open_in_memory().unwrap());
    let guard = Guard::new(ledger.clone());
    let store = Arc::new(MemorySecretStore::default());
    let config = ConnectionsConfig {
        stand_in: Some(ms.base()),
        github_client_id: client_id.map(str::to_owned),
        github_app_slug: Some(github::APP_SLUG.into()),
        ..ConnectionsConfig::default()
    };
    let conns = Arc::new(Connections::new(
        guard.clone(),
        store.clone() as Arc<dyn SecretStore>,
        config,
        Arc::new(NoBrowser),
    ));
    H {
        ledger,
        guard,
        store,
        conns,
        ms,
    }
}

async fn harness() -> H {
    harness_with(Some(github::CLIENT_ID)).await
}

impl H {
    fn state(&self) -> ConnectionState {
        self.guard.connection(ID).unwrap().state
    }

    fn card(&self) -> plenipo_capabilities::connections::ConnectionCard {
        self.conns
            .page(true)
            .unwrap()
            .services
            .into_iter()
            .find(|s| s.service == plenipo_guard::Service::Github)
            .unwrap()
            .connections
            .remove(0)
    }

    async fn until(&self, what: &str, done: impl Fn(&H) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(self) {
            assert!(Instant::now() < deadline, "{what} never happened");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn sign_in(&self) {
        self.conns
            .start_sign_in(ID, AccountKind::Work)
            .await
            .unwrap();
        self.until("the sign-in", |h| h.state() == ConnectionState::Connected)
            .await;
    }

    fn vault(&self) -> Option<String> {
        plenipo_capabilities::vault::read(self.store.as_ref(), VAULT_ID).unwrap()
    }

    fn requests(&self, part: &str) -> usize {
        self.ms
            .world()
            .requests
            .iter()
            .filter(|r| r.contains(part))
            .count()
    }

    /// No token or code Plenipo was given ever reached the record.
    fn record_has_no_token(&self) {
        let events = self.ledger.recent_events(1000).unwrap();
        let text = serde_json::to_string(&events).unwrap();
        for prefix in ["ghu_", "ghr_", "dc_"] {
            assert!(!text.contains(prefix), "{prefix} in the record: {text}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signing_in_with_a_short_code_keeps_only_the_long_lived_sign_in() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    assert_eq!(h.state(), ConnectionState::NotConnected);
    h.ms.world().github.pending_polls = 2;
    h.conns.start_sign_in(ID, AccountKind::Work).await.unwrap();
    // The code, with GitHub's own page and the words, on the card only.
    let card = h.card();
    let code = card.code.clone().expect("the code is on the card");
    assert_eq!(code.code, github::USER_CODE);
    assert_eq!(code.page, "https://github.com/login/device");
    assert_eq!(code.words, "Only type a code Plenipo just showed you here.");
    assert!(card.owner_only && card.parts.is_empty());
    assert_eq!(
        card.install_page.as_deref(),
        Some("https://github.com/apps/plenipo-test-app/installations/new")
    );
    // Pressing Sign in again shows the same code: no second one is asked for.
    h.conns.start_sign_in(ID, AccountKind::Work).await.unwrap();
    assert_eq!(h.requests("POST /github.com/login/device/code"), 1);
    h.until("the sign-in", |h| h.state() == ConnectionState::Connected)
        .await;
    let card = h.card();
    assert!(card.code.is_none());
    let account = card.connection.account.unwrap();
    assert_eq!(account.address, github::LOGIN);
    assert_eq!(card.connection.granted, vec!["metadata:read".to_owned()]);
    assert!(h.vault().unwrap().starts_with("ghr_"));
    // G2: no token ever reached github.com, and every sign-in ask wanted JSON.
    let w = h.ms.world();
    assert!(!w.github.token_on_github_com && !w.github.not_json);
    drop(w);
    h.record_has_no_token();
    // The page the owner types the code on is the only one opened, and it is GitHub's.
    assert!(h.conns.secrets().iter().any(|(v, _)| v.starts_with("ghr_")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_list_is_every_allowed_accounts_repositories_and_kept_ten_minutes() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    {
        let mut w = h.ms.world();
        w.github.own_repositories = 250;
        w.github.extra_permissions = true;
    }
    h.sign_in().await;
    let list = h.conns.github_repositories(false).await.unwrap();
    let logins: Vec<&str> = list.accounts.iter().map(|a| a.login.as_str()).collect();
    assert_eq!(logins, vec![github::LOGIN, github::ORG, "client-co"]);
    // G4: an account showing more than Metadata: read is refused, and never asked about.
    let client = list
        .accounts
        .iter()
        .find(|a| a.login == "client-co")
        .unwrap();
    assert!(client.refused.is_some());
    assert_eq!(h.requests("/user/installations/33/"), 0);
    // Every page of the owner's own, then the organization's.
    assert_eq!(list.repositories.len(), 252);
    assert!(list.repositories[..250]
        .iter()
        .all(|r| r.owner == github::LOGIN));
    assert_eq!(list.repositories[250].owner, github::ORG);
    assert!(list.repositories.iter().any(|r| r.private));
    assert_eq!(
        list.repositories[0].address(),
        "https://github.com/frankieg/frankieg-repo-001"
    );
    assert!(!list.more);
    // Kept for ten minutes: asking again asks GitHub nothing; Refresh asks again.
    let asked = h.requests("GET /api.github.com/user/installations");
    h.conns.github_repositories(false).await.unwrap();
    assert_eq!(h.requests("GET /api.github.com/user/installations"), asked);
    h.conns.github_repositories(true).await.unwrap();
    assert!(h.requests("GET /api.github.com/user/installations") > asked);
    h.record_has_no_token();
}

/// A sign-in renews with no secret (ADR-204), and a sign-in GitHub no longer takes is forgotten:
/// the owner signs in again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn renewing_needs_no_secret_and_a_refused_one_asks_for_a_new_sign_in() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    h.sign_in().await;
    let first = h.vault().unwrap();
    // GitHub stops taking the access token: Plenipo renews (the stand-in refuses any secret).
    h.ms.world().github.access.clear();
    h.conns.github_repositories(true).await.unwrap();
    let renewed = h.vault().unwrap();
    assert!(renewed.starts_with("ghr_") && renewed != first, "rotated");
    // GitHub stops taking the sign-in too: forgotten, and the owner signs in again.
    {
        let mut w = h.ms.world();
        w.github.access.clear();
        w.github.refresh.clear();
    }
    let why = h.conns.github_repositories(true).await.unwrap_err();
    assert!(why.contains("sign in again"), "{why}");
    assert_eq!(h.state(), ConnectionState::NeedsSignIn);
    assert!(h.vault().is_none());
}

/// GitHub's sign-ins that don't expire are kept and used as they are.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sign_in_that_does_not_expire_is_used_as_it_is() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    h.ms.world().github.no_expiry = true;
    h.sign_in().await;
    assert!(h.vault().unwrap().starts_with("ghu_"));
    assert_eq!(
        h.conns
            .github_repositories(false)
            .await
            .unwrap()
            .repositories
            .len(),
        5
    );
}

/// The reviewer's G2: GitHub's connection follows no redirect; it is refused and recorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redirect_from_github_is_refused_and_recorded() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    h.sign_in().await;
    h.ms.world().redirect_answers_to = Some(("GET /api.github.com/user/installations".into(), 1));
    let why = h.conns.github_repositories(true).await.unwrap_err();
    assert!(why.contains("another page"), "{why}");
    assert_eq!(h.requests("/.well-known/check/"), 0, "never followed");
    let refused = h
        .ledger
        .recent_events(100)
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "guard.request_refused")
        .expect("recorded");
    assert_eq!(refused.payload["purpose"], "the GitHub connection");
}

/// A code ends when the owner says no or it runs out, and a code for another page is never
/// shown; nothing is kept, and the next sign-in can start.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_or_expired_code_keeps_nothing() {
    let _turn = ONE_AT_A_TIME.lock().await;
    for knob in ["deny", "expire"] {
        let h = harness().await;
        h.ms.world()
            .github
            .knobs(&serde_json::json!({ knob: true }));
        h.conns.start_sign_in(ID, AccountKind::Work).await.unwrap();
        h.until("the card's reason", |h| h.card().problem.is_some())
            .await;
        assert_eq!(h.state(), ConnectionState::NotConnected, "{knob}");
        assert!(h.vault().is_none() && h.card().code.is_none(), "{knob}");
    }
    let h = harness().await;
    h.ms.world().github.elsewhere = true;
    let why = h
        .conns
        .start_sign_in(ID, AccountKind::Work)
        .await
        .unwrap_err();
    assert!(why.contains("another page"), "{why}");
    assert!(h.card().code.is_none());
    // Nothing left waiting: a sign-in starts again.
    h.ms.world().github.elsewhere = false;
    h.sign_in().await;
}

/// GitHub asks Plenipo to ask more slowly: it waits longer, and still signs in. A code cancelled
/// while it waits keeps nothing, and GitHub is asked about it no more.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn slowing_down_still_signs_in_and_a_cancelled_code_keeps_nothing() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    {
        let mut w = h.ms.world();
        w.github.slow_down_once = true;
        w.github.pending_polls = 0;
    }
    let started = Instant::now();
    h.sign_in().await;
    // Asked once at GitHub's pace (1 second), told to slow down, then 5 seconds more.
    assert!(
        started.elapsed() >= Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(h.requests("POST /github.com/login/oauth/access_token"), 2);

    let h = harness().await;
    h.ms.world().github.pending_polls = 1000;
    h.conns.start_sign_in(ID, AccountKind::Work).await.unwrap();
    assert!(h.card().code.is_some());
    assert!(h.conns.cancel(ID));
    h.until("the code to go", |h| h.card().code.is_none()).await;
    let asked = h.requests("POST /github.com/login/oauth/access_token");
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(
        h.requests("POST /github.com/login/oauth/access_token"),
        asked
    );
    assert_eq!(h.state(), ConnectionState::NotConnected);
    assert!(h.vault().is_none());
    assert!(!plenipo_capabilities::connections::github_code_waiting());
    h.record_has_no_token();
}

/// The reviewer's N2 on #227: GitHub busy for a moment while the owner types the code doesn't
/// end the sign-in; busy every time, a few times in a row, does, in plain words.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moment_without_github_doesnt_end_the_sign_in() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    {
        let mut w = h.ms.world();
        w.github.pending_polls = 0;
        w.github.fail_polls = 2;
    }
    h.sign_in().await;
    assert_eq!(h.requests("POST /github.com/login/oauth/access_token"), 3);

    let h = harness().await;
    h.ms.world().github.fail_polls = 1000;
    h.conns.start_sign_in(ID, AccountKind::Work).await.unwrap();
    h.until("the card's reason", |h| h.card().problem.is_some())
        .await;
    let why = h.card().problem.unwrap();
    assert!(why.contains("GitHub answered 503"), "{why}");
    assert_eq!(
        h.requests("POST /github.com/login/oauth/access_token"),
        github_missed_polls()
    );
    assert_eq!(h.state(), ConnectionState::NotConnected);
    assert!(h.vault().is_none() && h.card().code.is_none());
}

fn github_missed_polls() -> usize {
    plenipo_capabilities::connections::github::MAX_MISSED_POLLS as usize
}

/// The reviewer's G4: one GitHub sign-in at a time on this PC, whichever organization's window
/// starts it; Cancel ends it and frees the next.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_sign_in_at_a_time_on_this_pc() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let first = harness().await;
    let other = harness().await;
    first.ms.world().github.pending_polls = 1000;
    first
        .conns
        .start_sign_in(ID, AccountKind::Work)
        .await
        .unwrap();
    let why = other
        .conns
        .start_sign_in(ID, AccountKind::Work)
        .await
        .unwrap_err();
    assert!(why.contains("Another GitHub sign-in is waiting"), "{why}");
    assert!(first.conns.cancel(ID));
    other
        .until("the first sign-in to end", |_| {
            !plenipo_capabilities::connections::github_code_waiting()
        })
        .await;
    other.sign_in().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disconnect_erases_the_sign_in_and_says_where_to_finish_at_github() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let h = harness().await;
    h.sign_in().await;
    h.conns.github_repositories(false).await.unwrap();
    h.conns.disconnect(ID).await.unwrap();
    assert_eq!(h.state(), ConnectionState::NotConnected);
    assert!(h.vault().is_none());
    let note = h.card().problem.unwrap();
    assert!(
        note.contains("GitHub, then Settings, then Applications"),
        "{note}"
    );
    let why = h.conns.github_repositories(false).await.unwrap_err();
    assert!(why.contains("isn't connected"), "{why}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_copy_without_8_wests_github_app_cannot_sign_in() {
    let h = harness_with(None).await;
    let why = h
        .conns
        .start_sign_in(ID, AccountKind::Work)
        .await
        .unwrap_err();
    assert!(why.contains("no app ID for GitHub"), "{why}");
    assert!(!h.card().has_app);
}
