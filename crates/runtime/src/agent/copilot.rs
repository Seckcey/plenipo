//! GitHub Copilot adapter (GitHub's Copilot CLI, `copilot`), ADR-083 (GitHub Copilot as an AI
//! tool, checked before every task).
//!
//! Copilot failed its first try (`docs/phases/ai-tools-copilot-finding.md`): its one-task mode
//! has no sign-in check, and nothing in it stops GitHub from charging for extra requests once
//! the monthly allowance is used. Its second try answers both through `copilot --headless
//! --stdio`, the JSON-RPC link GitHub's own Copilot SDK uses: before every task Plenipo asks
//! `auth.getStatus` (signed in, and how) and `account.getQuota` (each allowance, and whether
//! GitHub may charge once it runs out), and waits for both answers ([`RuntimeAdapter::auth_talk`],
//! messages framed by `Content-Length` headers).
//!
//! The task itself is one process in its one-task mode: the words on standard input, one JSON
//! event per line out (`--output-format json`), its own tools off (`--available-tools` naming a
//! tool that does not exist), its GitHub add-on server off, and none of the folder's instruction
//! files. A new conversation gets an ID Plenipo chooses (`--session-id`); a later task continues
//! it (`--resume`).
//!
//! Least privilege (ADR-083 §4): Copilot gets a settings folder of its own (`COPILOT_HOME`,
//! [`RuntimeAdapter::home_variable`]), so none of the owner's own Copilot settings, hooks,
//! add-ons, or agents apply; the home folder stays the owner's, so the GitHub CLI's sign-in still
//! works (the owner's choice: either sign-in counts). Workers on it are conversation only; if one
//! of its own tools runs anyway, the task is stopped.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent::adapter::{
    cap, find_version, first_line, model_name, plain_name, talk_answer, tool_summary, Framing,
    NewestVersion, Parsed, ProbeOutput, ProcessEnd, ProviderSession, PublishedList, RuntimeAdapter,
    StatusCheck, Stop, Talk, TurnParser, TurnRequest, TurnState, MAX_EVENT_TEXT, MAX_SUMMARY,
    NETWORK_ENV,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AccountAction, AgentEvent, AuthState, AuthStatus, KnownModel, NoticeLevel, PlanReport,
    PlanWindow, RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::dto::TokenUsage;

pub const ID: &str = "copilot";
const LABEL: &str = "GitHub Copilot";

/// The variable that names Copilot's settings folder (its `help environment`).
pub const HOME_VARIABLE: &str = "COPILOT_HOME";

/// A tool name that matches none of Copilot's own tools: with it as the only tool allowed, the
/// model is offered no tools at all (1.0.88 and 1.0.89: "Unknown tool name in the tool
/// allowlist"; a tool it asks for anyway "does not exist").
pub const NO_TOOLS: &str = "plenipo_no_tools";

/// The requests of the check before each task, numbered as [`parse_auth`] reads the answers.
const CONNECT: u64 = 1;
const AUTH_STATUS: u64 = 2;
const QUOTA: u64 = 3;
const MODELS: u64 = 4;

#[derive(Debug, Default, Clone, Copy)]
pub struct Copilot;

/// `copilot --headless --stdio`: the link GitHub's Copilot SDK starts. It never updates itself
/// here and writes no log.
fn headless_args() -> Vec<String> {
    [
        "--headless",
        "--stdio",
        "--no-auto-update",
        "--log-level",
        "none",
    ]
    .map(String::from)
    .to_vec()
}

fn request(id: u64, method: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": {} }).to_string()
}

/// The npm package's real program for this computer, next to or inside `@github/copilot`
/// (GitHub's npm package keeps one package per platform, `@github/copilot-<os>-<arch>`).
fn native_in(package: &Path) -> Option<PathBuf> {
    let os = match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let name = if cfg!(windows) {
        "copilot.exe"
    } else {
        "copilot"
    };
    let platform = format!("copilot-{os}-{arch}");
    [
        package.join("node_modules").join("@github").join(&platform),
        package.parent()?.join(&platform),
    ]
    .into_iter()
    .map(|dir| dir.join(name))
    .find(|p| p.is_file())
}

impl RuntimeAdapter for Copilot {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        makers::GITHUB.0
    }

    fn provider_label(&self) -> &'static str {
        makers::GITHUB.1
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            // Its task stream never names the sign-in in use; the check before each task must
            // show one Plenipo accepts, with paid extra use off.
            billing_checked_per_turn: false,
            tool_posture: "Conversation only: GitHub Copilot's own tools are off. It cannot run \
                           programs, read or change files, open web pages, or use add-ons, and \
                           none of your own Copilot settings apply. Before each task Plenipo \
                           checks that GitHub will not charge for extra use. If Copilot uses one \
                           of its own tools anyway, Plenipo stops the task."
                .into(),
            // Only Auto was offered on the owner's plan (1.0.89, 2026-09-30), and Auto takes no
            // effort level Plenipo checked.
            effort_levels: Vec::new(),
            // No model is listed: the owner's plan offers only Auto, which is Copilot's default
            // ("Its default"). Auto picks the model itself, so who made it is not known.
            known_models: Vec::new(),
            default_maker: None,
            runs_other_makers: true,
        }
    }

    fn checked_version(&self) -> &'static str {
        "1.0.89"
    }

    fn install_hint(&self) -> &'static str {
        "Install GitHub Copilot's command-line tool. Windows: winget install GitHub.Copilot — or, \
         with Node.js: npm install -g @github/copilot. Then choose Check again."
    }

    fn login_hint(&self) -> &'static str {
        "Choose Sign in: Copilot's own sign-in opens in a terminal tab and then in your browser. \
         Sign in with the GitHub account that has your Copilot plan. (If the GitHub CLI is \
         signed in, Copilot can use that sign-in too.) Plenipo never asks for your password or \
         a token. Then choose Check again."
    }

    fn executable_name(&self) -> &'static str {
        "copilot"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if cfg!(windows) {
            // WinGet's links folder (`winget install GitHub.Copilot`).
            if let Some(local) = host.var("LOCALAPPDATA") {
                out.push(
                    PathBuf::from(local)
                        .join("Microsoft")
                        .join("WinGet")
                        .join("Links")
                        .join("copilot.exe"),
                );
            }
            // npm's global packages: the real program inside the package, never its shim.
            if let Some(appdata) = &host.appdata {
                let package = appdata
                    .join("npm")
                    .join("node_modules")
                    .join("@github")
                    .join("copilot");
                out.extend(native_in(&package));
            }
        } else {
            if let Some(home) = &host.home {
                out.push(home.join(".local").join("bin").join("copilot"));
            }
            out.extend(host.system_dirs().iter().map(|d| d.join("copilot")));
        }
        out
    }

    fn resolve(&self, found: &Path) -> Option<PathBuf> {
        let ext = found
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        if cfg!(windows) {
            return match ext.as_deref() {
                Some("exe") => Some(found.to_path_buf()),
                // npm's shim: run the real program it would start, never the shim.
                Some("cmd" | "ps1" | "bat") => {
                    let package = found
                        .parent()?
                        .join("node_modules")
                        .join("@github")
                        .join("copilot");
                    native_in(&package)
                }
                _ => None,
            };
        }
        // npm links `copilot` to the package's `npm-loader.js`; prefer the real program.
        let canonical = dunce::canonicalize(found).ok()?;
        if canonical
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("js"))
        {
            return canonical.parent().and_then(native_in);
        }
        Some(found.to_path_buf())
    }

    /// `GitHub Copilot CLI 1.0.89.` — its sentence ends with a full stop.
    fn parse_version(&self, out: &ProbeOutput) -> Option<String> {
        find_version(&out.combined()).map(|v| v.trim_end_matches('.').to_owned())
    }

    /// Only a talk tells how Copilot is signed in; this command is never run.
    fn auth_args(&self) -> Vec<String> {
        Vec::new()
    }

    fn auth_talk(&self) -> Option<Talk> {
        Some(Talk {
            args: headless_args(),
            lines: vec![
                request(CONNECT, "connect"),
                request(AUTH_STATUS, "auth.getStatus"),
                request(QUOTA, "account.getQuota"),
            ],
            answers: vec![CONNECT, AUTH_STATUS, QUOTA],
            framing: Framing::Headers,
        })
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        NETWORK_ENV.to_vec()
    }

    fn fixed_env(&self) -> Vec<(String, String)> {
        // Plenipo updates Copilot between tasks (ADR-059); it never replaces itself mid-task.
        vec![("COPILOT_AUTO_UPDATE".into(), "false".into())]
    }

    fn home_variable(&self) -> Option<&'static str> {
        Some(HOME_VARIABLE)
    }

    fn preassigns_session_id(&self) -> bool {
        true
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let mut args: Vec<String> = [
            "--output-format",
            "json",
            "--no-auto-update",
            // Its own tools off: only a tool that does not exist is allowed.
            &format!("--available-tools={NO_TOOLS}"),
            // GitHub's own add-on server (reads and writes on github.com) off.
            "--disable-builtin-mcps",
            "--no-ask-user",
            // The folder's AGENTS.md and similar files are not read as instructions.
            "--no-custom-instructions",
        ]
        .map(String::from)
        .to_vec();
        if let Some(model) = &request.model {
            args.push(format!("--model={model}"));
        }
        match &request.session {
            ProviderSession::New {
                preassigned: Some(id),
            } => args.push(format!("--session-id={id}")),
            ProviderSession::New { preassigned: None } => {}
            ProviderSession::Resume { id } => args.push(format!("--resume={id}")),
        }
        args
    }

    fn accepts_tools(&self) -> bool {
        false
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        let expected = match &request.session {
            ProviderSession::Resume { id } => Some(id.clone()),
            ProviderSession::New { preassigned } => preassigned.clone(),
        };
        Box::new(Parser {
            state: TurnState::new(LABEL),
            expected,
            model: request.model.clone(),
            started: false,
            exit_code: None,
        })
    }

    // ---- The AI tools page (Phase 19) ----------------------------------------------------

    /// `copilot login` (its browser sign-in). Copilot has no sign-out command; signing out is
    /// done inside it (`/logout`).
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        match action {
            AccountAction::SignIn => Some(vec!["login".into()]),
            AccountAction::SignOut => None,
        }
    }

    /// GitHub's npm package for the Copilot CLI.
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::Published(PublishedList::Npm("@github/copilot"))
    }

    fn update_command(&self) -> Option<Vec<String>> {
        Some(vec!["update".into()])
    }

    fn update_by_hand(&self) -> Option<&'static str> {
        Some(
            "Copilot was installed with npm and cannot update itself. Open a terminal and run: \
             npm install -g @github/copilot",
        )
    }

    /// The same link as the check before each task, asking for its models and allowances; no
    /// conversation, no prompt.
    fn status_check(&self, _dir: &Path) -> StatusCheck {
        StatusCheck::Talk {
            args: headless_args(),
            lines: vec![
                request(CONNECT, "connect"),
                request(QUOTA, "account.getQuota"),
                request(MODELS, "models.list"),
            ],
            answers: vec![CONNECT, QUOTA, MODELS],
            framing: Framing::Headers,
        }
    }

    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        let models = talk_answer(out, MODELS)?;
        let listed: Vec<KnownModel> = models
            .get("models")?
            .as_array()?
            .iter()
            .filter_map(|m| {
                let name = model_name(m.get("id")?.as_str()?)?;
                let label = m
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|l| plain_name(l, 64))
                    .unwrap_or_else(|| name.clone());
                Some(KnownModel::new(&name, &label, &[]))
            })
            .fold(Vec::new(), |mut all, m| {
                // Copilot 1.0.89 listed Auto twice.
                if !all.iter().any(|k: &KnownModel| k.name == m.name) {
                    all.push(m);
                }
                all
            });
        (!listed.is_empty()).then_some(listed)
    }

    fn parse_plan(&self, out: &ProbeOutput) -> Option<PlanReport> {
        parse_plan(out, crate::now_ms())
    }

    fn reports_plan_left(&self) -> bool {
        true
    }
}

/// Every allowance `account.getQuota` reported, by name; `None` when it did not answer with
/// them.
fn allowances(out: &ProbeOutput) -> Option<Vec<(String, Value)>> {
    let snapshots = talk_answer(out, QUOTA)?;
    let snapshots = snapshots.get("quotaSnapshots")?.as_object()?;
    Some(
        snapshots
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(k, v)| (cap(k, 64), v.clone()))
            .collect(),
    )
}

/// The check before each task (ADR-083 §2): `auth.getStatus` says whether and how Copilot is
/// signed in; `account.getQuota` says, for each allowance, whether GitHub may charge once it is
/// used up. A task runs only on Copilot's own sign-in or the GitHub CLI's, with paid extra use
/// off on every allowance. The account name is never kept.
fn parse_auth(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<&str>, detail: Option<String>| AuthStatus {
        state,
        method: method.map(str::to_owned),
        detail,
    };
    if let Some(e) = &out.spawn_error {
        return status(
            AuthState::Unknown,
            None,
            Some(format!("The sign-in check could not start: {e}")),
        );
    }
    let Some(auth) = talk_answer(out, AUTH_STATUS) else {
        let why = if out.timed_out {
            "Copilot did not answer the sign-in check in time.".to_owned()
        } else {
            let said = first_line(&out.stderr, 120);
            if said.is_empty() {
                "Copilot did not say how it is signed in.".to_owned()
            } else {
                format!("Copilot did not say how it is signed in: {said}")
            }
        };
        return status(AuthState::Unknown, None, Some(why));
    };
    if auth.get("isAuthenticated").and_then(Value::as_bool) != Some(true) {
        return status(AuthState::SignedOut, None, None);
    }
    let (method, accepted) =
        match auth.get("authType").and_then(Value::as_str) {
            // Its own stored sign-in (`copilot login`), or the GitHub CLI's (`gh auth login`): the
            // owner's choice, both the owner's own Copilot plan (ADR-083 §2).
            Some("user") => ("Copilot sign-in", true),
            Some("gh-cli") => ("GitHub CLI sign-in", true),
            // A token in a variable (Plenipo never passes one), a key, or an app's credential.
            Some("env" | "token") => return status(
                AuthState::ApiKey,
                Some("GitHub token"),
                Some(
                    "Copilot is using a token instead of your sign-in. Plenipo never uses one; \
                     sign in with Copilot or the GitHub CLI instead."
                        .into(),
                ),
            ),
            Some("api-key" | "hmac") => {
                return status(
                    AuthState::ApiKey,
                    Some("key"),
                    Some("Copilot is using a key instead of your sign-in.".into()),
                )
            }
            _ => ("", false),
        };
    if !accepted {
        return status(
            AuthState::Unverified,
            None,
            Some(
                "Copilot is signed in a way Plenipo does not recognize, so it cannot tell whether \
                 your Copilot plan pays for it."
                    .into(),
            ),
        );
    }
    let Some(allowances) = allowances(out) else {
        return status(
            AuthState::Unverified,
            Some(method),
            Some(
                "Copilot did not say whether GitHub may charge for extra use once your \
                 allowance runs out, so no task runs. Choose Check again to ask again."
                    .into(),
            ),
        );
    };
    // Paid extra use must be off on every allowance: then GitHub refuses a request past the
    // allowance instead of billing it. A missing answer counts as on.
    let paid: Vec<&str> = allowances
        .iter()
        .filter(|(_, v)| {
            v.get("overageAllowedWithExhaustedQuota")
                .and_then(Value::as_bool)
                != Some(false)
        })
        .map(|(k, _)| k.as_str())
        .collect();
    if allowances.is_empty() || !paid.is_empty() {
        return status(
            AuthState::Unverified,
            Some(method),
            Some(format!(
                "GitHub may charge for extra use once your Copilot allowance runs out{}. Plenipo \
                 never lets a task cost money, so no task runs. On github.com, open Settings → \
                 Billing and licensing → Budgets and alerts, set the budget for AI Credits to $0 \
                 with Stop usage on, then choose Check again.",
                if paid.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", paid.join(", "))
                }
            )),
        );
    }
    status(AuthState::Subscription, Some(method), None)
}

/// How much of each allowance is used, from `account.getQuota` (ADR-060 §3). Allowances with
/// nothing included (`entitlementRequests` 0) and unlimited ones are left out.
fn parse_plan(out: &ProbeOutput, now: u64) -> Option<PlanReport> {
    let allowances = allowances(out)?;
    let mut limited = false;
    let mut warning = false;
    let windows: Vec<PlanWindow> = allowances
        .iter()
        .filter(|(_, v)| {
            v.get("isUnlimitedEntitlement").and_then(Value::as_bool) != Some(true)
                && v.get("entitlementRequests")
                    .and_then(Value::as_f64)
                    .is_some_and(|n| n > 0.0)
        })
        .map(|(_, v)| {
            let left = v
                .get("remainingPercentage")
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite());
            let used = left.map(|p| (100.0 - p).clamp(0.0, 100.0).round() as u8);
            limited |= used == Some(100);
            warning |= used.is_some_and(|u| u >= 80);
            PlanWindow {
                minutes: None,
                used_percent: used,
                resets_at: v.get("resetDate").and_then(Value::as_str).and_then(iso_ms),
            }
        })
        .collect();
    if windows.is_empty() {
        return None;
    }
    Some(PlanReport {
        windows,
        limited,
        warning,
        plan: None,
        reported_at: now,
    })
}

/// Milliseconds since 1970 for an ISO 8601 time in UTC (`2026-10-01T00:00:00.000Z`).
fn iso_ms(text: &str) -> Option<u64> {
    let text = text.trim().strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.split('.').next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || h > 23 || min > 59 || s > 60 {
        return None;
    }
    // Days from 1970-01-01 to the date (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + h * 3_600 + min * 60 + s;
    u64::try_from(seconds).ok().map(|s| s * 1000)
}

/// Its own tools refused, as Copilot words it: one that does not exist (all of them, with
/// Plenipo's flags), or one it had no permission for. Anything else may have run.
fn refused(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("does not exist") || m.contains("permission denied")
}

/// One task's JSON lines (Copilot 1.0.89, recorded on the owner's PC).
struct Parser {
    state: TurnState,
    /// The conversation Plenipo chose or asked to continue.
    expected: Option<String>,
    model: Option<String>,
    started: bool,
    /// `result.exitCode`.
    exit_code: Option<i64>,
}

impl Parser {
    fn stop(&mut self, outcome: TurnOutcome, reason: String) -> Parsed {
        let stop = Stop { outcome, reason };
        self.state.stop = Some(stop.clone());
        Parsed {
            stop: Some(stop),
            ..Parsed::none()
        }
    }

    /// The conversation begins: its ID is the one Plenipo chose or continues.
    fn start(&mut self, parsed: &mut Parsed) {
        if self.started {
            return;
        }
        self.started = true;
        if self.expected.is_some() {
            self.state.provider_session_id = self.expected.clone();
        }
        parsed.events.insert(
            0,
            AgentEvent::SessionStarted {
                provider_session_id: self.expected.clone(),
                model: self.model.clone(),
            },
        );
    }

    fn event(&mut self, kind: &str, data: &Value) -> Parsed {
        // A model billed per use (a custom provider, BYOK) is never Plenipo's: stop at once.
        if data.get("isByok").and_then(Value::as_bool) == Some(true) {
            return self.stop(
                TurnOutcome::BillingNotAllowed,
                "GitHub Copilot used a model provider billed per use instead of your Copilot \
                 plan, so Plenipo stopped the task."
                    .into(),
            );
        }
        match kind {
            "session.auto_mode_resolved" => {
                if let Some(model) = data
                    .get("chosenModel")
                    .and_then(Value::as_str)
                    .and_then(model_name)
                {
                    self.state.model = Some(model);
                }
                Parsed::none()
            }
            "assistant.message_delta" => match data.get("deltaContent").and_then(Value::as_str) {
                Some(delta) if !delta.is_empty() => Parsed::one(AgentEvent::TextDelta {
                    text: delta.to_owned(),
                }),
                _ => Parsed::none(),
            },
            "assistant.message" => {
                if self.state.model.is_none() {
                    self.state.model = data
                        .get("model")
                        .and_then(Value::as_str)
                        .and_then(model_name);
                }
                match data.get("content").and_then(Value::as_str) {
                    Some(text) if !text.trim().is_empty() => {
                        self.state.last_message = Some(text.to_owned());
                        Parsed::one(AgentEvent::Message {
                            text: text.to_owned(),
                        })
                    }
                    _ => Parsed::none(),
                }
            }
            "assistant.usage" => {
                let n = |k: &str| data.get(k).and_then(Value::as_u64).unwrap_or(0);
                let add = TokenUsage {
                    input_tokens: n("inputTokens"),
                    cached_input_tokens: n("cacheReadTokens"),
                    output_tokens: n("outputTokens"),
                };
                let usage = match self.state.usage {
                    Some(u) => TokenUsage {
                        input_tokens: u.input_tokens + add.input_tokens,
                        cached_input_tokens: u.cached_input_tokens + add.cached_input_tokens,
                        output_tokens: u.output_tokens + add.output_tokens,
                    },
                    None => add,
                };
                self.state.usage = Some(usage);
                Parsed::one(AgentEvent::Usage { usage })
            }
            "tool.execution_start" => {
                let tool = tool_name(data);
                let summary = data.get("arguments").map(tool_summary).unwrap_or_default();
                Parsed::one(AgentEvent::ToolUse {
                    tool,
                    summary: if summary.is_empty() {
                        "asked to use one of its own tools".into()
                    } else {
                        summary
                    },
                })
            }
            "tool.execution_complete" => {
                let tool = tool_name(data);
                let message = data
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let succeeded = data.get("success").and_then(Value::as_bool) == Some(true);
                if succeeded || !refused(message) {
                    return self.stop(
                        TurnOutcome::Failed,
                        format!(
                            "GitHub Copilot used its own tool {tool}, which Plenipo does not \
                             allow, so Plenipo stopped the task."
                        ),
                    );
                }
                Parsed::one(AgentEvent::ToolResult {
                    tool: Some(tool),
                    is_error: true,
                    summary: first_line(message, MAX_SUMMARY),
                })
            }
            "session.error" => {
                let message = data
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Copilot reported an error");
                // Sorted by its kind, not its wording (its own event schema).
                let error = match data.get("errorType").and_then(Value::as_str) {
                    Some("quota" | "rate_limit") => format!("Usage limit reached: {message}"),
                    Some("authentication") => format!("Authentication failed: {message}"),
                    _ => message.to_owned(),
                };
                self.state.error = Some(cap(&error, MAX_EVENT_TEXT));
                Parsed::none()
            }
            "session.warning" => match data.get("message").and_then(Value::as_str) {
                Some(text) => {
                    self.state.last_warning = Some(cap(text, MAX_EVENT_TEXT));
                    Parsed::none()
                }
                None => Parsed::none(),
            },
            _ => Parsed::none(),
        }
    }

    fn result(&mut self, v: &Value) -> Parsed {
        let mut parsed = Parsed::none();
        if let Some(id) = v
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            let id = cap(id, 128);
            if let Some(expected) = &self.expected {
                if *expected != id {
                    parsed.events.push(AgentEvent::Notice {
                        level: NoticeLevel::Warning,
                        text: format!(
                            "GitHub Copilot reported conversation {id} instead of {expected}; \
                             Plenipo will continue {id}."
                        ),
                    });
                }
            }
            self.state.provider_session_id = Some(id);
        }
        if let Some(ms) = v
            .pointer("/usage/totalApiDurationMs")
            .and_then(Value::as_u64)
        {
            self.state.provider_duration_ms = Some(ms);
        }
        self.exit_code = v.get("exitCode").and_then(Value::as_i64);
        if self.exit_code == Some(0) && self.state.error.is_none() {
            self.state.completed = true;
        } else if self.state.error.is_none() {
            self.state.error = Some("GitHub Copilot reported that the task failed".into());
        }
        parsed
    }
}

fn tool_name(data: &Value) -> String {
    data.get("toolName")
        .and_then(Value::as_str)
        .map_or_else(|| "a tool".to_owned(), |n| cap(n, 80))
}

impl TurnParser for Parser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return self.state.malformed_line(truncated);
        };
        let Some(kind) = v.get("type").and_then(Value::as_str) else {
            self.state.unknown += 1;
            return Parsed::none();
        };
        let mut parsed = match kind {
            "result" => self.result(&v),
            k if k.starts_with("session.")
                || k.starts_with("assistant.")
                || k.starts_with("tool.")
                || k.starts_with("model.")
                || k.starts_with("user.") =>
            {
                let data = v.get("data").cloned().unwrap_or(Value::Null);
                self.event(k, &data)
            }
            _ => {
                self.state.unknown += 1;
                return Parsed::none();
            }
        };
        self.state.understood += 1;
        if parsed.stop.is_none() {
            self.start(&mut parsed);
        }
        parsed
    }

    fn stderr(&mut self, text: &str) {
        self.state.stderr(text);
    }

    fn finish(&mut self, end: &ProcessEnd) -> TurnResult {
        self.state.finish(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ExecutionState;

    fn talk(answers: &[(u64, Value)]) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
            stdout: answers
                .iter()
                .map(|(id, result)| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                .map(|v| format!("{v}\n"))
                .collect(),
            ..ProbeOutput::default()
        }
    }

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(10),
        }
    }

    fn request(session: ProviderSession, model: Option<&str>) -> TurnRequest {
        TurnRequest {
            session,
            model: model.map(str::to_owned),
            effort: None,
            billing_confirmed: true,
            tools: None,
            working_dir: PathBuf::new(),
        }
    }

    /// `auth.getStatus` on the owner's PC (1.0.89), the account name hidden.
    fn signed_in(kind: &str) -> Value {
        json!({ "isAuthenticated": true, "authType": kind, "host": "https://github.com",
                "statusMessage": "(hidden)", "login": "(hidden)" })
    }

    /// One allowance as `account.getQuota` reported it on the owner's PC (1.0.89).
    fn allowance(entitled: u64, paid: bool) -> Value {
        json!({ "isUnlimitedEntitlement": false, "entitlementRequests": entitled,
                "usedRequests": 0, "usageAllowedWithExhaustedQuota": false, "overage": 0,
                "overageAllowedWithExhaustedQuota": paid, "remainingPercentage": 100,
                "resetDate": "2026-10-01T00:00:00.000Z", "hasQuota": entitled > 0,
                "tokenBasedBilling": true })
    }

    fn quota(paid_chat: bool) -> Value {
        json!({ "quotaSnapshots": { "chat": allowance(200, paid_chat),
                                    "completions": allowance(2000, false),
                                    "premium_interactions": allowance(0, false) } })
    }

    fn connected() -> (u64, Value) {
        (
            CONNECT,
            json!({ "ok": true, "protocolVersion": 3, "version": "1.0.89" }),
        )
    }

    #[test]
    fn its_own_sign_in_or_the_github_clis_with_paid_extra_use_off_is_ready() {
        for (kind, method) in [
            ("user", "Copilot sign-in"),
            ("gh-cli", "GitHub CLI sign-in"),
        ] {
            let s = parse_auth(&talk(&[
                connected(),
                (AUTH_STATUS, signed_in(kind)),
                (QUOTA, quota(false)),
            ]));
            assert_eq!(s.state, AuthState::Subscription, "{kind}");
            assert_eq!(s.method.as_deref(), Some(method));
            assert!(s.detail.is_none());
            // The account name is never kept.
            assert!(!format!("{s:?}").contains("hidden"), "{s:?}");
        }
    }

    #[test]
    fn paid_extra_use_on_any_allowance_is_never_ready() {
        let s = parse_auth(&talk(&[
            connected(),
            (AUTH_STATUS, signed_in("user")),
            (QUOTA, quota(true)),
        ]));
        assert_eq!(s.state, AuthState::Unverified);
        let detail = s.detail.unwrap();
        assert!(
            detail.contains("(chat)") && detail.contains("$0"),
            "{detail}"
        );
        // An allowance that does not say counts as paid extra use on.
        let mut unsure = quota(false);
        unsure["quotaSnapshots"]["chat"]
            .as_object_mut()
            .unwrap()
            .remove("overageAllowedWithExhaustedQuota");
        let s = parse_auth(&talk(&[
            connected(),
            (AUTH_STATUS, signed_in("user")),
            (QUOTA, unsure),
        ]));
        assert_eq!(s.state, AuthState::Unverified);
        // No allowances at all, or no answer: never ready.
        for answers in [
            vec![
                connected(),
                (AUTH_STATUS, signed_in("user")),
                (QUOTA, json!({ "quotaSnapshots": {} })),
            ],
            vec![connected(), (AUTH_STATUS, signed_in("user"))],
        ] {
            assert_eq!(parse_auth(&talk(&answers)).state, AuthState::Unverified);
        }
    }

    #[test]
    fn tokens_keys_and_unknown_sign_ins_are_never_used() {
        for (kind, state) in [
            ("env", AuthState::ApiKey),
            ("token", AuthState::ApiKey),
            ("api-key", AuthState::ApiKey),
            ("hmac", AuthState::ApiKey),
            ("something-new", AuthState::Unverified),
        ] {
            let s = parse_auth(&talk(&[
                connected(),
                (AUTH_STATUS, signed_in(kind)),
                (QUOTA, quota(false)),
            ]));
            assert_eq!(s.state, state, "{kind}");
        }
    }

    #[test]
    fn signed_out_and_no_answer_are_told_apart() {
        // Recorded signed out on the build machine (1.0.89).
        let out = talk(&[
            connected(),
            (
                AUTH_STATUS,
                json!({ "isAuthenticated": false, "statusMessage": "Not authenticated" }),
            ),
        ]);
        assert_eq!(parse_auth(&out).state, AuthState::SignedOut);
        let slow = ProbeOutput {
            timed_out: true,
            ..ProbeOutput::default()
        };
        let s = parse_auth(&slow);
        assert_eq!(s.state, AuthState::Unknown);
        assert!(s.detail.unwrap().contains("in time"));
        let broken = ProbeOutput {
            spawn_error: Some("not found".into()),
            ..ProbeOutput::default()
        };
        assert_eq!(parse_auth(&broken).state, AuthState::Unknown);
        let error = ProbeOutput {
            stdout: json!({ "jsonrpc": "2.0", "id": AUTH_STATUS,
                            "error": { "code": -32603, "message": "boom" } })
            .to_string(),
            stderr: "boom happened\n".into(),
            ..ProbeOutput::default()
        };
        let s = parse_auth(&error);
        assert_eq!(s.state, AuthState::Unknown);
        assert!(s.detail.unwrap().contains("boom happened"));
    }

    #[test]
    fn a_task_is_one_process_with_its_own_tools_off() {
        let args = Copilot.turn_args(&request(
            ProviderSession::New {
                preassigned: Some("0f8fad5b-d9cb-469f-a165-70867728950e".into()),
            },
            None,
        ));
        for flag in [
            "--output-format",
            "json",
            "--no-auto-update",
            "--available-tools=plenipo_no_tools",
            "--disable-builtin-mcps",
            "--no-ask-user",
            "--no-custom-instructions",
            "--session-id=0f8fad5b-d9cb-469f-a165-70867728950e",
        ] {
            assert!(args.iter().any(|a| a == flag), "{flag}: {args:?}");
        }
        // Never a way round its permissions, and never the words of the task.
        assert!(!args
            .iter()
            .any(|a| a.starts_with("--allow") || a == "--yolo" || a == "-p"));
        let args = Copilot.turn_args(&request(
            ProviderSession::Resume { id: "abc".into() },
            Some("auto"),
        ));
        assert!(args.contains(&"--resume=abc".to_owned()), "{args:?}");
        assert!(args.contains(&"--model=auto".to_owned()), "{args:?}");
    }

    #[test]
    fn the_check_asks_for_the_sign_in_and_the_allowance_framed_by_headers() {
        let t = Copilot.auth_talk().unwrap();
        assert_eq!(t.framing, Framing::Headers);
        assert_eq!(
            t.args,
            [
                "--headless",
                "--stdio",
                "--no-auto-update",
                "--log-level",
                "none"
            ]
        );
        let methods: Vec<String> = t
            .lines
            .iter()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["method"].to_string())
            .collect();
        assert_eq!(
            methods,
            ["\"connect\"", "\"auth.getStatus\"", "\"account.getQuota\""]
        );
        assert_eq!(t.answers, [CONNECT, AUTH_STATUS, QUOTA]);
    }

    /// Lines of `evidence/phase-16-wave-2/owner-check/copilot/copilot-task.jsonl`, trimmed.
    const TASK: &str = r#"{"type":"session.info","data":{"infoType":"configuration","message":"Unknown tool name in the tool allowlist: \"plenipo_no_tools\""},"ephemeral":true}
{"type":"session.auto_mode_resolved","data":{"chosenModel":"mai-code-1.1-flash","candidateModels":["mai-code-1.1-flash"],"fallback":false}}
{"type":"session.tools_updated","data":{"model":"mai-code-1.1-flash"},"ephemeral":true}
{"type":"user.message","data":{"content":"Remember the word heron. Reply with just OK.\r\n"}}
{"type":"assistant.turn_start","data":{"turnId":"0"}}
{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"OK"},"ephemeral":true}
{"type":"assistant.message","data":{"messageId":"m1","model":"mai-code-1.1-flash","content":"OK","toolRequests":[],"phase":"final_answer"}}
{"type":"assistant.turn_end","data":{"turnId":"0"}}
{"type":"session.usage_checkpoint","data":{"totalPremiumRequests":1}}
{"type":"result","timestamp":"2026-09-30T07:53:38.054Z","sessionId":"7ce402b1-bf2d-43bf-888b-55d08dd82bc4","exitCode":0,"usage":{"premiumRequests":1,"totalApiDurationMs":694,"sessionDurationMs":1794}}"#;

    fn run(lines: &str, request: &TurnRequest) -> (Vec<Parsed>, TurnResult) {
        let mut p = Copilot.parser(request);
        let parsed: Vec<Parsed> = lines.lines().map(|l| p.line(l, false)).collect();
        let r = p.finish(&end(ExecutionState::Succeeded, Some(0)));
        (parsed, r)
    }

    #[test]
    fn a_recorded_task_gives_its_answer_session_and_the_model_auto_chose() {
        let id = "7ce402b1-bf2d-43bf-888b-55d08dd82bc4";
        let (parsed, r) = run(
            TASK,
            &request(
                ProviderSession::New {
                    preassigned: Some(id.into()),
                },
                None,
            ),
        );
        assert_eq!(r.outcome, TurnOutcome::Completed, "{r:?}");
        assert_eq!(r.text.as_deref(), Some("OK"));
        assert_eq!(r.provider_session_id.as_deref(), Some(id));
        assert_eq!(r.model.as_deref(), Some("mai-code-1.1-flash"));
        assert_eq!(r.duration_ms, Some(694));
        assert_eq!(r.ignored_lines, 0);
        let events: Vec<&AgentEvent> = parsed.iter().flat_map(|p| &p.events).collect();
        assert!(matches!(
            events[0],
            AgentEvent::SessionStarted { provider_session_id: Some(s), .. } if s == id
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextDelta { text } if text == "OK")));
        assert!(!events
            .iter()
            .any(|e| matches!(e, AgentEvent::Notice { .. })));
    }

    #[test]
    fn a_continued_conversation_says_so_when_copilot_reports_another() {
        let (parsed, r) = run(
            TASK,
            &request(ProviderSession::Resume { id: "other".into() }, None),
        );
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(
            r.provider_session_id.as_deref(),
            Some("7ce402b1-bf2d-43bf-888b-55d08dd82bc4")
        );
        assert!(parsed.iter().flat_map(|p| &p.events).any(|e| matches!(
            e,
            AgentEvent::Notice {
                level: NoticeLevel::Warning,
                ..
            }
        )));
    }

    #[test]
    fn its_own_tool_refused_goes_on_and_one_that_ran_stops_the_task() {
        // `evidence/ai-tools-copilot/stand-in/tool-tools-hidden.jsonl` (1.0.88).
        let refused = r#"{"type":"tool.execution_start","data":{"toolCallId":"call_1","toolName":"bash","arguments":{"command":"touch /work/x"}}}
{"type":"tool.execution_complete","data":{"toolCallId":"call_1","success":false,"error":{"message":"Tool 'bash' does not exist.","code":"failure"}}}
{"type":"assistant.message","data":{"content":"pong","toolRequests":[]}}
{"type":"result","sessionId":"s1","exitCode":0}"#;
        let (parsed, r) = run(refused, &TurnRequest::default());
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert!(parsed.iter().all(|p| p.stop.is_none()));
        let ran = r#"{"type":"tool.execution_start","data":{"toolName":"view","arguments":{"path":"a.txt"}}}
{"type":"tool.execution_complete","data":{"toolName":"view","success":true}}"#;
        let (parsed, r) = run(ran, &TurnRequest::default());
        assert!(parsed[1].stop.is_some());
        assert_eq!(r.outcome, TurnOutcome::Failed);
        assert!(r.summary.contains("own tool"), "{}", r.summary);
        // Failed for another reason: it may have run, so the task stops too.
        let odd = r#"{"type":"tool.execution_complete","data":{"toolName":"bash","success":false,"error":{"message":"exit code 1"}}}"#;
        let (_, r) = run(odd, &TurnRequest::default());
        assert_eq!(r.outcome, TurnOutcome::Failed);
    }

    #[test]
    fn a_model_billed_per_use_stops_the_task() {
        // `evidence/ai-tools-copilot/stand-in/error-401.jsonl`: a custom provider says `isByok`.
        let byok = r#"{"type":"model.call_failure","data":{"model":"gpt-5.4","statusCode":401,"isByok":true}}"#;
        let (parsed, r) = run(byok, &TurnRequest::default());
        assert!(parsed[0].stop.is_some());
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);
    }

    #[test]
    fn errors_are_sorted_by_their_kind() {
        for (kind, outcome) in [
            ("quota", TurnOutcome::UsageLimited),
            ("rate_limit", TurnOutcome::UsageLimited),
            ("authentication", TurnOutcome::AuthRequired),
            ("query", TurnOutcome::Failed),
        ] {
            let lines = format!(
                "{}\n{}",
                json!({ "type": "session.error",
                        "data": { "errorType": kind, "message": "402 something happened" } }),
                json!({ "type": "result", "sessionId": "s", "exitCode": 1 })
            );
            let (_, r) = run(&lines, &TurnRequest::default());
            assert_eq!(r.outcome, outcome, "{kind}: {r:?}");
        }
        // Signed out: its words on stderr, no JSON (recorded, 1.0.89).
        let mut p = Copilot.parser(&TurnRequest::default());
        p.stderr("Error: No authentication information found.");
        p.stderr("Copilot can be authenticated with GitHub using an OAuth Token or a Fine-Grained Personal Access Token.");
        let r = p.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::AuthRequired, "{r:?}");
    }

    #[test]
    fn its_models_and_plan_come_from_its_own_link() {
        let out = talk(&[
            connected(),
            (QUOTA, quota(false)),
            (
                MODELS,
                json!({ "models": [ { "id": "auto", "name": "Auto" }, { "id": "auto", "name": "Auto" },
                                    { "id": "not a name", "name": "x" } ] }),
            ),
        ]);
        let models = Copilot.parse_models(&out).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(
            (models[0].name.as_str(), models[0].label.as_str()),
            ("auto", "Auto")
        );
        assert!(
            models[0].maker.is_none(),
            "who made it is never read from the tool"
        );
        let plan = parse_plan(&out, 5).unwrap();
        // Chat and completions; nothing is included in premium requests, so it is left out.
        assert_eq!(plan.windows.len(), 2);
        assert_eq!(plan.windows[0].used_percent, Some(0));
        assert_eq!(plan.windows[0].resets_at, Some(1_790_812_800_000));
        assert!(!plan.limited && !plan.warning);
        assert_eq!(plan.reported_at, 5);
    }

    #[test]
    fn its_version_sentence_gives_the_version() {
        let out = ProbeOutput {
            exit_code: Some(0),
            stdout: "GitHub Copilot CLI 1.0.89.\nRun 'copilot update' to check for updates.\n"
                .into(),
            ..ProbeOutput::default()
        };
        assert_eq!(Copilot.parse_version(&out).as_deref(), Some("1.0.89"));
    }

    #[test]
    fn iso_times() {
        assert_eq!(iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_ms("2026-10-01T00:00:00.000Z"), Some(1_790_812_800_000));
        assert_eq!(iso_ms("2024-02-29T12:30:05Z"), Some(1_709_209_805_000));
        assert_eq!(iso_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(iso_ms("2026-10-01T00:00:00+02:00"), None);
        assert_eq!(iso_ms("soon"), None);
    }

    #[test]
    fn the_npm_shim_leads_to_the_real_program() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir
            .path()
            .join("node_modules")
            .join("@github")
            .join("copilot");
        let os = match std::env::consts::OS {
            "windows" => "win32",
            "macos" => "darwin",
            o => o,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            a => a,
        };
        let name = if cfg!(windows) {
            "copilot.exe"
        } else {
            "copilot"
        };
        // Hoisted next to the package (as npm put it on the build machine).
        let hoisted = package
            .parent()
            .unwrap()
            .join(format!("copilot-{os}-{arch}"));
        std::fs::create_dir_all(&hoisted).unwrap();
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(hoisted.join(name), "").unwrap();
        assert_eq!(native_in(&package), Some(hoisted.join(name)));
        // Nested inside it (as on the owner's first check) is looked for first.
        let nested = package
            .join("node_modules")
            .join("@github")
            .join(format!("copilot-{os}-{arch}"));
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join(name), "").unwrap();
        assert_eq!(native_in(&package), Some(nested.join(name)));
    }
}
