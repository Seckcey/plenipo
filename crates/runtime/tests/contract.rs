//! The contract every AI tool's adapter meets (ADR-014; walk-through in
//! `docs/development/adding-an-ai-tool.md`). Every test runs for each adapter in
//! `builtin_adapters()`, so registering a new AI tool there puts it under this suite. The last
//! tests start the tool's persona of `plenipo-fake-agent`. No network, no accounts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_runtime::agent::adapter::{find_version, ProcessEnd};
use plenipo_runtime::agent::service::validate_model;
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentEvent, AgentRuntime, AgentSink, AgentTurn, AgentUpdate,
    AuthState, Bridge, Effort, HoldFor, HostEnv, InstallState, MemorySessionStore, NotFree,
    ProviderSession, RuntimeAdapter, TurnOutcome, TurnRequest,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ExecutionState, MetadataStore, ProfileRegistry, RuntimeEvent,
    Supervisor, SupervisorConfig,
};

const SESSION_ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

// ---- Identity ---------------------------------------------------------------------------------

#[test]
fn every_ai_tool_is_named_and_distinct() {
    let adapters = builtin_adapters();
    assert!(!adapters.is_empty());
    let mut companies = BTreeMap::new();
    for a in &adapters {
        let id = a.id();
        for (what, value) in [
            ("label", a.label()),
            ("company ID", a.provider()),
            ("company name", a.provider_label()),
            ("executable name", a.executable_name()),
            ("install hint", a.install_hint()),
            ("sign-in hint", a.login_hint()),
        ] {
            assert!(!value.trim().is_empty(), "{id}: empty {what}");
        }
        // The ID is also a handoff address (`codex`, `role:…`) and part of a program's name.
        assert!(
            id.starts_with(|c: char| c.is_ascii_lowercase())
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{id:?}: use lowercase letters, digits, and dashes"
        );
        // One name per AI company.
        let name = companies.entry(a.provider()).or_insert(a.provider_label());
        assert_eq!(*name, a.provider_label(), "{id}: company {}", a.provider());
    }
    for (what, values) in [
        ("ID", adapters.iter().map(|a| a.id()).collect::<Vec<_>>()),
        ("label", adapters.iter().map(|a| a.label()).collect()),
        (
            "executable",
            adapters.iter().map(|a| a.executable_name()).collect(),
        ),
        ("company name", companies.values().copied().collect()),
    ] {
        let distinct: BTreeSet<_> = values.iter().collect();
        assert_eq!(distinct.len(), values.len(), "repeated {what}: {values:?}");
    }
}

// ---- Models and effort (the model capability discovery contract) ----------------------------

/// Lowest first, no repeats.
fn ascending(levels: &[Effort]) -> bool {
    levels.windows(2).all(|w| w[0] < w[1])
}

#[test]
fn known_models_are_valid_distinct_and_within_the_tools_effort_levels() {
    for a in builtin_adapters() {
        let id = a.id();
        let caps = a.capabilities();
        let checked = a.checked_version();
        assert_eq!(
            find_version(checked).as_deref(),
            Some(checked),
            "{id}: checked_version must be the CLI version, e.g. 2.1.283"
        );
        assert!(
            ascending(&caps.effort_levels),
            "{id}: effort levels lowest first, no repeats"
        );
        let mut names = BTreeSet::new();
        for m in &caps.known_models {
            assert_eq!(validate_model(&m.name).ok().as_ref(), Some(&m.name), "{id}");
            assert!(names.insert(&m.name), "{id}: {} listed twice", m.name);
            assert!(!m.label.trim().is_empty(), "{id}: {} has no label", m.name);
            assert!(ascending(&m.effort_levels), "{id}: {}", m.name);
            for e in &m.effort_levels {
                assert!(
                    caps.effort_levels.contains(e),
                    "{id}: {} takes {e:?}, which the AI tool does not list",
                    m.name
                );
            }
        }
    }
}

/// Every model an AI tool lists says who made it (ADR-081 §1): one name per company across all
/// AI tools, the same as an AI tool's company name when the IDs match; a tool that runs only its
/// own company's models lists only models its company made; and a name that points to the
/// newest model points to one that is listed too.
#[test]
fn every_listed_model_says_who_made_it() {
    let adapters = builtin_adapters();
    let mut names: BTreeMap<String, String> = adapters
        .iter()
        .map(|a| (a.provider().to_owned(), a.provider_label().to_owned()))
        .collect();
    for a in &adapters {
        let id = a.id();
        let caps = a.capabilities();
        for m in &caps.known_models {
            let maker = m
                .maker
                .as_ref()
                .unwrap_or_else(|| panic!("{id}: {} says nobody made it", m.name));
            assert!(
                !maker.id.trim().is_empty() && !maker.label.trim().is_empty(),
                "{id}: {} has an empty maker",
                m.name
            );
            let name = names
                .entry(maker.id.clone())
                .or_insert_with(|| maker.label.clone());
            assert_eq!(
                *name, maker.label,
                "{id}: company {} has two names",
                maker.id
            );
            if !caps.runs_other_makers {
                assert_eq!(
                    maker.id,
                    a.provider(),
                    "{id} runs only its own company's models, but {} is by {}",
                    m.name,
                    maker.label
                );
            }
            if let Some(exact) = &m.points_to {
                assert!(
                    caps.known_models.iter().any(|k| &k.name == exact),
                    "{id}: {} points to {exact}, which is not listed",
                    m.name
                );
            }
        }
        if caps.runs_other_makers {
            assert!(
                caps.default_maker.is_some(),
                "{id} runs other companies' models, so it must say who made its default"
            );
        }
    }
}

// ---- Launch: arguments and environment --------------------------------------------------------

/// Every shape of turn: new (with and without an ID Plenipo chose) and resumed, with the
/// default model and each known model, at the default effort and each level it takes.
fn requests(a: &dyn RuntimeAdapter) -> Vec<TurnRequest> {
    let caps = a.capabilities();
    let sessions = [
        ProviderSession::New { preassigned: None },
        ProviderSession::New {
            preassigned: a.preassigns_session_id().then(|| SESSION_ID.to_owned()),
        },
        ProviderSession::Resume {
            id: SESSION_ID.into(),
        },
    ];
    let models =
        std::iter::once(None).chain(caps.known_models.iter().map(|m| Some(m.name.clone())));
    let mut out = Vec::new();
    for model in models {
        let efforts = caps.effort_levels_for(model.as_deref()).iter().copied();
        for effort in std::iter::once(None).chain(efforts.map(Some)) {
            for session in &sessions {
                out.push(TurnRequest {
                    session: session.clone(),
                    model: model.clone(),
                    effort,
                    ..TurnRequest::default()
                });
            }
        }
    }
    out
}

/// `value` is an argument of its own, or an option's value (`--flag=value`, `key=value`).
fn passes(args: &[String], value: &str) -> bool {
    args.iter().any(|a| {
        a == value
            || a.strip_suffix(value)
                .is_some_and(|flag| flag.ends_with('='))
    })
}

/// An argument that would carry a credential.
fn secret_bearing(arg: &str) -> bool {
    let a = arg.to_ascii_lowercase().replace('_', "-");
    let flag = a.split('=').next().unwrap_or("");
    [
        "api-key",
        "apikey",
        "auth-token",
        "access-token",
        "oauth-token",
        "password",
        "secret",
        "credential",
        "bearer",
    ]
    .iter()
    .any(|w| a.contains(w))
        || ["--token", "--key"].contains(&flag)
}

/// The messages a task that talks (ADR-015) sends when the AI tool answers every request
/// the way a willing tool would; empty for a one-way task.
fn messages(a: &dyn RuntimeAdapter, request: &TurnRequest) -> Vec<String> {
    let mut parser = a.parser(request);
    let Some(mut pending) = parser.open("Contract prompt") else {
        return Vec::new();
    };
    let mut sent = Vec::new();
    for _ in 0..8 {
        let mut next = Vec::new();
        for line in pending.drain(..) {
            sent.push(line.clone());
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let (Some(id), Some(method)) = (message.get("id"), message["method"].as_str()) else {
                continue;
            };
            let result = match method {
                "initialize" => serde_json::json!({ "protocolVersion": 1, "agentCapabilities": {
                    "loadSession": true, "sessionCapabilities": { "resume": {} } } }),
                "session/new" => serde_json::json!({ "sessionId": "contract-session" }),
                _ => serde_json::json!({}),
            };
            let answer = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
            next.extend(parser.line(&answer.to_string(), false).send);
        }
        pending = next;
    }
    sent
}

#[test]
fn turn_arguments_carry_the_choices_and_no_secrets() {
    for a in builtin_adapters() {
        let id = a.id();
        for request in requests(a.as_ref()) {
            let args = a.turn_args(&request);
            assert!(
                !args.iter().any(|arg| secret_bearing(arg)),
                "{id}: {args:?}"
            );
            let session = match &request.session {
                ProviderSession::Resume { id } => Some(id),
                ProviderSession::New { preassigned } => preassigned.as_ref(),
            };
            // Each choice is in the arguments, or in the messages of a task that talks
            // (ADR-015; Kimi's model and thinking level are session settings, ADR-027).
            let sent = messages(a.as_ref(), &request);
            let carried = |value: &str| {
                let quoted = format!("\"{value}\"");
                passes(&args, value) || sent.iter().any(|m| m.contains(&quoted))
            };
            if let Some(session) = session {
                assert!(carried(session), "{id}: session ID missing: {args:?}");
            }
            if let Some(model) = &request.model {
                assert!(carried(model), "{id}: model missing: {args:?} {sent:?}");
            }
            if let Some(effort) = request.effort {
                assert!(
                    carried(effort.as_str()),
                    "{id}: effort missing: {args:?} {sent:?}"
                );
            }
        }
    }
}

#[test]
fn no_api_key_or_cloud_variables_are_passed() {
    for a in builtin_adapters() {
        let fixed = a.fixed_env();
        // A fixed switch Plenipo sets to turn key sign-in off (`GROK_DISABLE_API_KEY_AUTH=1`,
        // ADR-015 §6) carries no credential; everything else is checked by name.
        let switch_off = |(k, v): &&(String, String)| {
            k.contains("DISABLE") && ["1", "true"].contains(&v.as_str())
        };
        let names = a.passthrough_env().into_iter().chain(
            fixed
                .iter()
                .filter(|kv| !switch_off(kv))
                .map(|(k, _)| k.as_str()),
        );
        for name in names {
            let upper = name.to_ascii_uppercase();
            assert!(
                ![
                    "KEY",
                    "TOKEN",
                    "SECRET",
                    "PASSWORD",
                    "CREDENTIAL",
                    "BEDROCK",
                    "VERTEX",
                    "FOUNDRY",
                    "BASE_URL",
                ]
                .iter()
                .any(|w| upper.contains(w)),
                "{}: {name} could carry a credential or switch billing",
                a.id()
            );
        }
    }
}

// ---- Output: the parser and the normalized result ---------------------------------------------

fn end(state: ExecutionState, exit_code: Option<i32>, started: bool) -> ProcessEnd {
    ProcessEnd {
        state,
        exit_code,
        started,
        detail: None,
        duration_ms: Some(1),
    }
}

#[test]
fn parsers_ignore_malformed_lines_and_finish_with_one_result() {
    use ExecutionState::*;
    let ends = [
        (end(Succeeded, Some(0), true), None),
        (end(Failed, Some(1), true), None),
        (
            end(Failed, None, false),
            Some(TurnOutcome::ProviderUnavailable),
        ),
        (end(Cancelled, None, true), Some(TurnOutcome::Cancelled)),
        (end(TimedOut, None, true), Some(TurnOutcome::TimedOut)),
        (end(Interrupted, None, true), Some(TurnOutcome::Interrupted)),
    ];
    let junk = [
        "<html>502 Bad Gateway</html>",
        "{not json",
        "[1, 2]",
        r#"{"type":"plenipo.contract.unknown"}"#,
    ];
    for a in builtin_adapters() {
        let id = a.id();
        for fed in [&junk[..0], &junk[..]] {
            for (process, want) in &ends {
                let request = TurnRequest {
                    billing_confirmed: true,
                    ..TurnRequest::default()
                };
                let mut parser = a.parser(&request);
                for line in fed {
                    let parsed = parser.line(line, false);
                    assert!(parsed.stop.is_none(), "{id}: {line:?} stopped the turn");
                    assert!(
                        parsed
                            .events
                            .iter()
                            .all(|e| matches!(e, AgentEvent::Notice { .. })),
                        "{id}: {line:?} → {:?}",
                        parsed.events
                    );
                }
                parser.line(&"x".repeat(64), true);
                let result = parser.finish(process);
                let at = format!("{id}, {:?} after {} lines", process.state, fed.len());
                assert_ne!(result.outcome, TurnOutcome::Completed, "{at}");
                assert!(!result.summary.trim().is_empty(), "{at}");
                assert_eq!(result.ignored_lines as usize, fed.len() + 1, "{at}");
                if let Some(want) = want {
                    assert_eq!(result.outcome, *want, "{at}");
                }
            }
        }
    }
}

// ---- Against the fake CLI --------------------------------------------------------------------

const WAIT: Duration = Duration::from_secs(30);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plenipo-fake-agent"))
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// Every AI tool the fake CLI stands in for (`plenipo-fake-agent --personas`).
fn personas() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = std::process::Command::new(fake_exe())
            .arg("--personas")
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .leak()
            .lines()
            .collect()
    })
}

/// One copy of each persona per test process, ready to execute; harnesses get hard links
/// (see `agents.rs` for why).
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("contract-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(fake_exe(), &path).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while let Err(e) = std::process::Command::new(&path).arg("--version").output() {
                assert!(Instant::now() < deadline, "fake CLI never runnable: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        dir
    })
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct NoUpdates;

impl AgentSink for NoUpdates {
    fn emit(&self, _: AgentUpdate) {}
}

/// Every AI tool, played by its persona, signed in as `auth` says.
struct Fakes {
    rt: AgentRuntime,
    dir: tempfile::TempDir,
}

impl Fakes {
    fn new(auth: &str) -> Self {
        Self::with(auth, |_| {})
    }

    /// As [`Fakes::new`], with `tweak` applied to the configuration.
    fn with(auth: &str, tweak: impl FnOnce(&mut AgentConfig)) -> Self {
        let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        let bin = dir.path().join("bin");
        let home = dir.path().join("home");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(home.join(".plenipo-fake-agent")).unwrap();
        std::fs::write(home.join(".plenipo-fake-agent").join("auth"), auth).unwrap();
        for stem in personas() {
            let (source, target) = (fake_clis().join(exe_name(stem)), bin.join(exe_name(stem)));
            if std::fs::hard_link(&source, &target).is_err() {
                std::fs::copy(&source, &target).unwrap();
            }
        }
        let sup = Supervisor::new(
            SupervisorConfig::default(),
            ExecutablePolicy::default(),
            ProfileRegistry::default(),
            Arc::new(MetadataStore::in_memory()),
            Arc::new(NoOutput),
            vec![],
        );
        let mut config = AgentConfig::new(dir.path().join("workspaces"));
        config.extra_env = vec![(HOME_VAR.into(), home.display().to_string())];
        // The `ollama` persona also plays Plenipo's Ollama bridge (ADR-017).
        config.bridge = Some(Bridge {
            executable: bin.join(exe_name("ollama")),
            args: vec!["--plenipo-ollama".into()],
        });
        tweak(&mut config);
        let rt = AgentRuntime::new(
            config,
            builtin_adapters(),
            sup,
            Arc::new(MemorySessionStore::default()),
            Arc::new(NoUpdates),
            HostEnv::new(Some(bin.into_os_string()), Some(home), None),
        );
        Self { rt, dir }
    }

    /// Run one task on `runtime` to its end; the task and the arguments the CLI received.
    async fn run(&self, runtime: &str, objective: &str) -> (AgentTurn, Vec<String>) {
        let started = self.rt.start_session(runtime, objective, None).await;
        let id = started.unwrap().session.id;
        let deadline = Instant::now() + WAIT;
        loop {
            let detail = self.rt.session(&id).await.unwrap();
            if detail.turns.iter().all(|t| t.result.is_some())
                && detail.session.active_task_id.is_none()
            {
                let state = self.dir.path().join("home").join(".plenipo-fake-agent");
                let args = std::fs::read_to_string(state.join("last-args.json")).unwrap();
                return (
                    detail.turns[0].clone(),
                    serde_json::from_str(&args).unwrap(),
                );
            }
            assert!(Instant::now() < deadline, "{runtime}: {detail:#?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[tokio::test]
async fn sign_in_check_tells_a_subscription_from_an_api_key() {
    for a in builtin_adapters() {
        assert!(
            personas().contains(&a.executable_name()),
            "{}: add a `{}` persona to plenipo-fake-agent",
            a.id(),
            a.executable_name()
        );
    }
    for (auth, state, ready) in [
        ("subscription", AuthState::Subscription, true),
        ("api-key", AuthState::ApiKey, false),
        ("signed-out", AuthState::SignedOut, false),
    ] {
        let fakes = Fakes::new(auth);
        for info in fakes.rt.refresh().await {
            let at = format!("{} ({auth})", info.id);
            let bridged = builtin_adapters()
                .iter()
                .any(|a| a.id() == info.id && a.bridged());
            if bridged && auth == "api-key" {
                // Reached through Plenipo's bridge, signed in with `ollama signin` only: there
                // is no API-key sign-in to tell apart (ADR-017).
                continue;
            }
            assert_eq!(info.installation.state, InstallState::Installed, "{at}");
            assert!(info.installation.version.is_some(), "{at}");
            assert_eq!((info.auth.state, info.ready), (state, ready), "{at}");
        }
    }
}

#[tokio::test]
async fn prompts_go_on_stdin_and_limits_and_sign_in_errors_are_normalized() {
    let fakes = Fakes::new("subscription");
    fakes.rt.refresh().await;
    for a in builtin_adapters() {
        let id = a.id();
        let (turn, args) = fakes.run(id, "Contract check 7c1e").await;
        let result = turn.result.unwrap();
        assert_eq!(result.outcome, TurnOutcome::Completed, "{id}: {result:#?}");
        assert!(
            result
                .text
                .unwrap_or_default()
                .contains("Contract check 7c1e"),
            "{id}: the prompt reached the CLI on stdin"
        );
        assert!(!args.iter().any(|a| a.contains("7c1e")), "{id}: {args:?}");
        for (marker, outcome) in [
            ("[usage-limit]", TurnOutcome::UsageLimited),
            ("[auth-expired]", TurnOutcome::AuthRequired),
        ] {
            let (turn, _) = fakes.run(id, marker).await;
            assert_eq!(turn.result.unwrap().outcome, outcome, "{id}: {marker}");
        }
    }
}

// ---- The AI tools page (Phase 19, ADR-058 to ADR-060) ------------------------------------------

/// Flags that would sign in, or update, for pay-per-use billing or with a secret.
fn billing_flag(arg: &str) -> bool {
    let a = arg.to_ascii_lowercase();
    a == "--console" || a.contains("api-key") || a.contains("apikey") || a.contains("token")
}

#[test]
fn every_ai_tool_signs_in_with_its_own_command_and_never_for_api_billing() {
    use plenipo_runtime::agent::AccountAction;
    for a in builtin_adapters() {
        let id = a.id();
        let sign_in = a
            .account_command(AccountAction::SignIn)
            .unwrap_or_else(|| panic!("{id}: every AI tool signs in with its own command"));
        assert!(!sign_in.is_empty(), "{id}");
        for action in [AccountAction::SignIn, AccountAction::SignOut] {
            for arg in a.account_command(action).unwrap_or_default() {
                assert!(!billing_flag(&arg), "{id}: {arg:?} changes what is billed");
                assert!(!secret_bearing(&arg), "{id}: {arg:?}");
                assert!(
                    !arg.contains(' ') && !arg.is_empty(),
                    "{id}: one word per argument"
                );
            }
        }
    }
}

#[test]
fn every_update_is_the_tools_own_command_and_asks_nothing() {
    use plenipo_runtime::agent::adapter::{NewestVersion, PublishedList};
    for a in builtin_adapters() {
        let id = a.id();
        for arg in a
            .update_command()
            .unwrap_or_default()
            .iter()
            .chain(a.put_back_command("1.2.3").unwrap_or_default().iter())
        {
            assert!(!billing_flag(arg) && !secret_bearing(arg), "{id}: {arg:?}");
            assert!(
                !arg.contains("://") && !arg.contains(['|', ';', '&']),
                "{id}: {arg:?}"
            );
        }
        if let Some(put_back) = a.put_back_command("1.2.3") {
            assert!(put_back.contains(&"1.2.3".to_owned()), "{id}");
        }
        match a.newest_version() {
            NewestVersion::Published(list) => {
                let address = list.address();
                assert!(address.starts_with("https://"), "{id}: {address}");
                assert!(
                    matches!(list, PublishedList::Npm(_) | PublishedList::GitHub(_)),
                    "{id}"
                );
            }
            NewestVersion::Command(args) => {
                assert!(args.iter().all(|a| !a.contains("://")), "{id}");
            }
            NewestVersion::None => {}
        }
        // A tool with no update command of its own has its newest version shown, never
        // installed by Plenipo (Ollama, ADR-059 §7).
        if a.update_command().is_none() {
            assert!(a.put_back_command("1.2.3").is_none(), "{id}");
        }
    }
}

#[tokio::test]
async fn each_tools_own_check_lists_its_models_with_no_task() {
    let fakes = Fakes::new("subscription");
    let state = fakes.dir.path().join("home").join(".plenipo-fake-agent");
    fakes.rt.refresh().await;
    for a in builtin_adapters() {
        let id = a.id();
        let name = format!("{}-new-model", a.executable_name());
        let reported = if id == "kimi" {
            format!("kimi-code/{name}")
        } else {
            name
        };
        std::fs::write(
            state.join(format!("models-{}", a.executable_name())),
            &reported,
        )
        .unwrap();
        let answer = fakes.rt.status_check(id).await.unwrap();
        if matches!(
            a.status_check(Path::new("/")),
            plenipo_runtime::agent::adapter::StatusCheck::None
        ) {
            assert!(!answer.checked && answer.models.is_none(), "{id}");
            continue;
        }
        let models = answer.models.unwrap_or_else(|| panic!("{id}: no models"));
        assert!(
            models.iter().any(|m| m.name == reported),
            "{id}: {models:?}"
        );
        let info = fakes
            .rt
            .runtimes()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap();
        let kept = info.reported_models.unwrap();
        assert!(kept.models.iter().any(|m| m.name == reported), "{id}");
        assert_eq!(kept.complete, a.reports_every_model(), "{id}");
        if id == "codex" {
            let plan = answer.plan.expect("Codex reports its plan");
            assert_eq!(plan.plan.as_deref(), Some("plus"));
            assert_eq!(plan.windows.len(), 2);
        }
    }
    // No task ran: no conversation, no prompt.
    assert!(fakes.rt.overview().await.unwrap().sessions.is_empty());
    let acp = std::fs::read_to_string(state.join("last-acp.json")).unwrap_or_default();
    assert!(!acp.contains("session/prompt"), "{acp}");
}

#[tokio::test]
async fn a_tool_that_answers_its_check_counts_as_answering_even_signed_out_or_slow() {
    // Signed out, Kimi answers `initialize`, then says it needs a sign-in: it still answers
    // the way Plenipo reads it (it is not broken), and reports no models.
    let fakes = Fakes::new("signed-out");
    fakes.rt.refresh().await;
    let kimi = fakes.rt.status_check("kimi").await.expect("Kimi answers");
    assert!(kimi.checked);
    assert!(kimi.models.is_none());

    // Codex answers its plan, then is slow to list its models: what it answered is kept.
    let fakes = Fakes::with("subscription", |c| c.probe_timeout = Duration::from_secs(2));
    let state = fakes.dir.path().join("home").join(".plenipo-fake-agent");
    std::fs::write(state.join("slow-model-list"), "yes").unwrap();
    fakes.rt.refresh().await;
    let codex = fakes.rt.status_check("codex").await.expect("Codex answers");
    assert_eq!(codex.plan.expect("its plan was kept").windows.len(), 2);
    assert!(codex.models.is_none());
}

#[tokio::test]
async fn a_task_waits_while_its_ai_tool_is_held_and_a_busy_tool_is_not_held() {
    let fakes = Fakes::new("subscription");
    fakes.rt.refresh().await;
    // Held (a sign-in or an update): a task that would start waits, then runs.
    let hold = fakes.rt.hold_if_free("codex", HoldFor::Update).unwrap();
    assert!(fakes.rt.held("codex"));
    // The screen is told why new tasks on Codex wait.
    let held = |id: &str| {
        fakes
            .rt
            .runtimes()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
            .held
    };
    assert_eq!(held("codex"), Some(HoldFor::Update));
    assert_eq!(held("grok"), None);
    // One at a time: no sign-in tab while it updates, and no second update.
    assert_eq!(
        fakes.rt.hold_if_free("codex", HoldFor::SignIn).unwrap_err(),
        NotFree::Held(HoldFor::Update)
    );
    assert_eq!(
        fakes.rt.hold_if_free("codex", HoldFor::Update).unwrap_err(),
        NotFree::Held(HoldFor::Update)
    );
    let rt = fakes.rt.clone();
    let started = tokio::spawn(async move { rt.start_session("codex", "hello", None).await });
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!started.is_finished(), "the task waits while Codex is held");
    assert!(fakes.rt.tasks_using("codex").is_empty());
    drop(hold);
    let detail = tokio::time::timeout(WAIT, started)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(detail.session.runtime_id, "codex");
    assert!(!fakes.rt.held("codex"));
    assert_eq!(held("codex"), None);

    // A task using a tool: the tool is not held, and the task is named.
    let rt = fakes.rt.clone();
    let slow = tokio::spawn(async move { rt.start_session("claude-code", "[slow]", None).await });
    let deadline = Instant::now() + WAIT;
    while fakes.rt.tasks_using("claude-code").is_empty() {
        assert!(Instant::now() < deadline, "the task never started");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let busy = fakes
        .rt
        .hold_if_free("claude-code", HoldFor::SignIn)
        .unwrap_err();
    assert!(
        matches!(&busy, NotFree::Tasks(tasks) if tasks.len() == 1),
        "{busy:?}"
    );
    assert!(!fakes.rt.held("claude-code"));
    // Another AI tool is free.
    assert!(fakes.rt.hold_if_free("grok", HoldFor::Update).is_ok());
    let id = slow.await.unwrap().unwrap().session.id;
    fakes.rt.cancel_turn(&id).await.ok();
}

#[tokio::test]
async fn a_sign_in_tab_left_open_holds_tasks_a_while_and_an_update_until_it_is_done() {
    let fakes = Fakes::with("subscription", |c| c.hold_wait = Duration::from_millis(300));
    fakes.rt.refresh().await;
    // A sign-in tab left open: a task waits a while, then goes ahead.
    let tab = fakes.rt.hold_if_free("codex", HoldFor::SignIn).unwrap();
    let detail = tokio::time::timeout(WAIT, fakes.rt.start_session("codex", "hello", None))
        .await
        .expect("the task went ahead after the wait")
        .unwrap();
    assert_eq!(detail.session.runtime_id, "codex");
    drop(tab);

    // An update: the task waits until the update and its checks are done, however long.
    let update = fakes.rt.hold_if_free("grok", HoldFor::Update).unwrap();
    let rt = fakes.rt.clone();
    let started = tokio::spawn(async move { rt.start_session("grok", "hello", None).await });
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(!started.is_finished(), "the task waits for the update");
    assert!(fakes.rt.tasks_using("grok").is_empty());
    // No sign-in tab opens while it updates.
    assert_eq!(
        fakes.rt.hold_if_free("grok", HoldFor::SignIn).unwrap_err(),
        NotFree::Held(HoldFor::Update)
    );
    drop(update);
    let detail = tokio::time::timeout(WAIT, started)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(detail.session.runtime_id, "grok");
    assert!(!fakes.rt.held("grok"));
}

#[tokio::test]
async fn a_tool_is_checked_again_on_its_own_after_a_sign_in() {
    let fakes = Fakes::new("signed-out");
    let state = fakes.dir.path().join("home").join(".plenipo-fake-agent");
    fakes.rt.refresh().await;
    std::fs::write(state.join("auth"), "subscription").unwrap();
    let (before, now) = fakes.rt.recheck("grok").await.unwrap();
    assert_eq!(before.state, AuthState::SignedOut);
    assert_eq!(now.auth.state, AuthState::Subscription);
    assert!(now.ready);
    // Only that tool was checked again.
    let codex = fakes
        .rt
        .runtimes()
        .into_iter()
        .find(|r| r.id == "codex")
        .unwrap();
    assert_eq!(codex.auth.state, AuthState::SignedOut);
    // The tool's own program, with its tasks' environment, for its sign-in tab.
    let program = fakes.rt.tool_program("ollama").unwrap();
    assert_eq!(
        program.executable.file_stem().unwrap().to_string_lossy(),
        "ollama",
        "the real program, not Plenipo's bridge"
    );
    assert!(fakes.rt.tool_program("nope").is_err());
}

#[tokio::test]
async fn grok_says_its_newest_version_itself() {
    let fakes = Fakes::new("subscription");
    let state = fakes.dir.path().join("home").join(".plenipo-fake-agent");
    fakes.rt.refresh().await;
    std::fs::write(state.join("newest-grok"), "1.0.100").unwrap();
    assert_eq!(
        fakes.rt.newest_by_command("grok").await.unwrap().as_deref(),
        Some("1.0.100")
    );
    assert_eq!(fakes.rt.newest_by_command("codex").await.unwrap(), None);
}
