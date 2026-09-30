//! Grok adapter (xAI's Grok Build CLI) over ACP (ADR-015).
//!
//! Grok's one-task mode (`grok -p`) cannot read the prompt from stdin, so a task runs
//! `grok agent --no-leader … stdio` and talks ACP through the shared driver ([`super::acp`]):
//! the prompt, like everything else, goes in on stdin. Each task is its own process (never
//! Grok's shared background "leader"). Grok gets none of its own tools, subagents, memory, or
//! web access, and none of the owner's Claude Code or Cursor settings; a worker with
//! permissions gets Plenipo's tool server (Phase 7), and Plenipo refuses every other tool
//! request. A request is Plenipo's tool server's only by the tool name Grok itself sets
//! (`mcp__plenipo__…` in `_meta.toolName` or `toolName`), never by what the model wrote as the
//! call's arguments, and each approval covers one action.
//!
//! Billing (ADR-007 §4): `GROK_DISABLE_API_KEY_AUTH=1` makes Grok refuse API keys, including a
//! key set on a model in its own settings; the sign-in check (`grok models`, whose first line
//! names the credential) must show a Grok sign-in before a task runs. Grok does not report its
//! credential during an ACP task, so an unrecognized sign-in is refused.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent::acp::{AcpTask, AcpTurn};
use crate::agent::adapter::{
    find_version, first_line, model_name, plain_name, talk_answer, Framing, NewestVersion,
    ProbeOutput, RuntimeAdapter, StatusCheck, TurnParser, TurnRequest, NETWORK_ENV,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    makers, AccountAction, AuthState, AuthStatus, Effort, KnownModel, RuntimeCapabilities,
};

pub const ID: &str = "grok";
const LABEL: &str = "Grok";

/// Effort levels `grok agent --reasoning-effort` takes for Grok 4.7, 4.7 Fast, and 4.6
/// (lowest first).
const FULL_EFFORT: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High, Effort::XHigh];
/// Grok 4.5's levels: no Extra high.
const GROK_4_5_EFFORT: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High];

/// Grok's own tools (1.0.41, and the names its docs use), all removed from Plenipo's tasks.
/// `search_tool` and `use_tool` reach tool servers; they stay only when Plenipo's tool server
/// is given, and Plenipo still refuses calls to any other server.
const OWN_TOOLS: &[&str] = &[
    "run_terminal_command",
    "run_terminal_cmd",
    "read_file",
    "write",
    "search_replace",
    "list_dir",
    "grep",
    "lsp",
    "kill_command_or_subagent",
    "get_command_or_subagent_output",
    "todo_write",
    "spawn_subagent",
    "task",
    "Agent",
    "scheduler_create",
    "scheduler_delete",
    "scheduler_list",
    "monitor",
    "workflow",
    "enter_plan_mode",
    "exit_plan_mode",
    "ask_user_question",
    "send_feedback",
    "web_search",
    "web_fetch",
    "image_gen",
    "image_edit",
    "image_to_video",
    "reference_to_video",
];
const TOOL_SERVER_TOOLS: &[&str] = &["search_tool", "use_tool"];

#[derive(Debug, Default, Clone, Copy)]
pub struct Grok;

impl RuntimeAdapter for Grok {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        "xai"
    }

    fn provider_label(&self) -> &'static str {
        "xAI"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            // Grok does not say which credential an ACP task uses; the check before each
            // task must show a Grok sign-in.
            billing_checked_per_turn: false,
            tool_posture: "None of Grok's own tools, helpers, memory, or web access, and none \
                           of your Claude Code or Cursor settings. A worker with permissions \
                           gets Plenipo's file, program, and git tools, each checked by \
                           Plenipo Guard."
                .into(),
            // `grok agent --reasoning-effort <level>`: the levels its models list.
            effort_levels: FULL_EFFORT.to_vec(),
            // The models Grok 1.0.41 offers when signed in (`grok models` and the ACP
            // handshake on the owner's machine, 2026-09-26), in its order, with the effort
            // levels each one's menu offers. Grok 4.7 is its default.
            known_models: vec![
                KnownModel::new("grok-4.7", "Grok 4.7", FULL_EFFORT),
                KnownModel::new("grok-4.7-build-fast", "Grok 4.7 Fast", FULL_EFFORT),
                KnownModel::new("grok-4.6", "Grok 4.6", FULL_EFFORT),
                KnownModel::new("grok-4.5", "Grok 4.5", GROK_4_5_EFFORT),
            ]
            .into_iter()
            .map(|m| m.by(makers::XAI))
            .collect(),
            default_maker: None,
            runs_other_makers: false,
        }
    }

    fn checked_version(&self) -> &'static str {
        "1.0.41"
    }

    fn install_hint(&self) -> &'static str {
        "Install Grok Build, xAI's command-line tool. Windows (PowerShell): irm https://x.ai/cli/install.ps1 | iex — \
         macOS/Linux: curl -fsSL https://x.ai/cli/install.sh | bash. Then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Open a terminal, run: grok login — and sign in with the X account that has your \
         SuperGrok or X Premium Plus subscription. Plenipo never asks for your password or an \
         API key. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "grok"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(home) = &host.home {
            // xAI's installers put it in ~/.grok/bin (a real grok.exe on Windows).
            if cfg!(windows) {
                out.push(home.join(".grok").join("bin").join("grok.exe"));
            } else {
                out.push(home.join(".grok").join("bin").join("grok"));
                out.push(home.join(".local").join("bin").join("grok"));
            }
        }
        out.extend(host.system_dirs().iter().map(|d| d.join("grok")));
        out
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["models".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        std::iter::once("GROK_HOME")
            .chain(NETWORK_ENV.iter().copied())
            .collect()
    }

    fn fixed_env(&self) -> Vec<(String, String)> {
        [
            // Refuse API-key sign-in, including a key set on a model (ADR-015 §6).
            ("GROK_DISABLE_API_KEY_AUTH", "1"),
            // Do not let the CLI replace itself in the middle of a Plenipo task.
            ("GROK_DISABLE_AUTOUPDATER", "1"),
            // Least privilege until Guard grants permissions (ADR-015 §5).
            ("GROK_SUBAGENTS", "0"),
            ("GROK_MEMORY", "0"),
            ("GROK_WEB_FETCH", "0"),
            // xAI's own web and X search run on xAI's side, out of the agent profile's reach.
            ("GROK_BACKEND_SEARCH", "0"),
            ("GROK_CLAUDE_SKILLS_ENABLED", "0"),
            ("GROK_CLAUDE_HOOKS_ENABLED", "0"),
            ("GROK_CLAUDE_MCPS_ENABLED", "0"),
            ("GROK_CLAUDE_AGENTS_ENABLED", "0"),
            ("GROK_CLAUDE_RULES_ENABLED", "0"),
            ("GROK_CURSOR_SKILLS_ENABLED", "0"),
            ("GROK_CURSOR_HOOKS_ENABLED", "0"),
            ("GROK_CURSOR_MCPS_ENABLED", "0"),
            ("GROK_CURSOR_AGENTS_ENABLED", "0"),
            ("GROK_CURSOR_RULES_ENABLED", "0"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    /// `grok login` and `grok logout` (Grok 1.0.41's own help, recorded).
    fn account_command(&self, action: AccountAction) -> Option<Vec<String>> {
        Some(match action {
            AccountAction::SignIn => vec!["login".into()],
            AccountAction::SignOut => vec!["logout".into()],
        })
    }

    /// Grok's own check (recorded): `grok update --check --json`.
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::Command(vec!["update".into(), "--check".into(), "--json".into()])
    }

    fn parse_newest(&self, out: &ProbeOutput) -> Option<String> {
        let answer: Value = serde_json::from_str(out.stdout.trim()).ok()?;
        answer
            .get("latestVersion")
            .and_then(Value::as_str)
            .and_then(find_version)
    }

    fn update_command(&self) -> Option<Vec<String>> {
        Some(vec!["update".into()])
    }

    /// `grok update --version <version>` installs that version.
    fn put_back_command(&self, version: &str) -> Option<Vec<String>> {
        Some(vec!["update".into(), "--version".into(), version.into()])
    }

    /// Grok's ACP `initialize` answer lists its models with each one's effort levels
    /// (recorded), with no conversation: the same exchange also shows it answers the way
    /// Plenipo reads it.
    fn status_check(&self, _dir: &Path) -> StatusCheck {
        let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": 1,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false },
                                    "terminal": false } } });
        StatusCheck::Talk {
            args: vec!["agent".into(), "--no-leader".into(), "stdio".into()],
            lines: vec![initialize.to_string()],
            answers: vec![1],
            framing: Framing::Lines,
        }
    }

    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        let answer = talk_answer(out, 1)?;
        let models = answer
            .pointer("/_meta/modelState/availableModels")?
            .as_array()?;
        Some(
            models
                .iter()
                .filter_map(|m| {
                    let name = m
                        .get("modelId")
                        .and_then(Value::as_str)
                        .and_then(model_name)?;
                    let label = m
                        .get("name")
                        .and_then(Value::as_str)
                        .and_then(|l| plain_name(l, 64))
                        .unwrap_or_else(|| name.clone());
                    let mut efforts: Vec<Effort> = m
                        .pointer("/_meta/reasoningEfforts")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(|e| e.get("value").and_then(Value::as_str))
                                .filter_map(Effort::parse)
                                .collect()
                        })
                        .unwrap_or_default();
                    efforts.sort();
                    efforts.dedup();
                    Some(KnownModel {
                        name,
                        label,
                        effort_levels: efforts,
                        maker: None,
                        points_to: None,
                        price: None,
                        same: None,
                    })
                })
                .collect(),
        )
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        // Agent options go between `agent` and the mode (`stdio`).
        let mut args: Vec<String> = vec!["agent".into(), "--no-leader".into()];
        if let Some(model) = &request.model {
            args.extend(["-m".into(), model.clone()]);
        }
        if let Some(effort) = request.effort {
            args.extend(["--reasoning-effort".into(), effort.as_str().into()]);
        }
        args.push("stdio".into());
        args
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        Box::new(AcpTurn::new(AcpTask {
            runtime_label: LABEL,
            session: request.session.clone(),
            working_dir: request.working_dir.clone(),
            tools: request.tools.clone(),
            model: request.model.clone(),
            session_meta: Some(json!({ "agentProfile": profile(request.tools.is_some()) })),
            ..AcpTask::default()
        }))
    }
}

/// The agent profile for a Plenipo task: none of Grok's own tools (ADR-015 §5). An empty
/// `tools` list is no limit to Grok, so each of its own tools is also named in
/// `disallowedTools` (see `evidence/ai-tools-grok/acp-tools.txt`).
fn profile(tool_server: bool) -> Value {
    let allowed: &[&str] = if tool_server { TOOL_SERVER_TOOLS } else { &[] };
    let removed: Vec<&str> = OWN_TOOLS
        .iter()
        .chain(if tool_server {
            &[][..]
        } else {
            TOOL_SERVER_TOOLS
        })
        .copied()
        .collect();
    json!({
        "name": "plenipo-worker",
        "description": "A Plenipo worker: only Plenipo's own tools, checked by Plenipo Guard.",
        "tools": allowed,
        "disallowedTools": removed,
    })
}

/// `grok models` starts with a line naming the credential in use:
/// "You are logged in with grok.com." (signed in), "You are not authenticated.",
/// "You are using XAI_API_KEY.", "Model 'grok-4.6' is using its own API key.", or
/// "You are authenticated via deployment key."
fn parse_auth(out: &ProbeOutput) -> AuthStatus {
    let status = |state, method: Option<&str>, detail: Option<&str>| AuthStatus {
        state,
        method: method.map(str::to_owned),
        detail: detail.map(str::to_owned),
    };
    if let Some(e) = &out.spawn_error {
        return status(
            AuthState::Unknown,
            None,
            Some(&format!("The sign-in check could not start: {e}")),
        );
    }
    if out.timed_out {
        return status(
            AuthState::Unknown,
            None,
            Some("The sign-in check timed out."),
        );
    }
    let line = out
        .stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let l = line.to_ascii_lowercase();
    if l.contains("not authenticated") || l.contains("not signed in") || l.contains("not logged in")
    {
        return status(AuthState::SignedOut, None, None);
    }
    if l.contains("api key") || l.contains("api_key") || l.contains("deployment key") {
        let method = if l.contains("deployment") {
            "Deployment key"
        } else {
            "API key"
        };
        return status(
            AuthState::ApiKey,
            Some(method),
            Some("Grok is using an API key: usage would be billed to the xAI API."),
        );
    }
    if out.exit_code == Some(0) && (l.contains("logged in") || l.contains("signed in")) {
        // "You are logged in with grok.com." (Grok 1.0, a subscription sign-in).
        let method = if l.contains("grok.com") {
            "grok.com sign-in"
        } else {
            "Grok sign-in"
        };
        return status(AuthState::Subscription, Some(method), None);
    }
    if !line.is_empty() {
        return status(
            AuthState::Unverified,
            None,
            Some(&format!(
                "Grok reported a sign-in Plenipo does not recognize: {}",
                first_line(line, 120)
            )),
        );
    }
    status(
        AuthState::Unknown,
        None,
        Some("This Grok version did not report its sign-in status."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::adapter::ProviderSession;
    use crate::agent::tools::ToolServer;

    fn probe(stdout: &str) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
            stdout: stdout.into(),
            ..ProbeOutput::default()
        }
    }

    #[test]
    fn sign_in_check_reads_the_first_line_of_grok_models() {
        // Real `grok models` output, Grok 1.0.41.
        let signed_out =
            include_str!("../../../../docs/phases/evidence/ai-tools-grok/models-signed-out.txt");
        let api_key =
            include_str!("../../../../docs/phases/evidence/ai-tools-grok/models-api-key.txt");
        let model_key =
            include_str!("../../../../docs/phases/evidence/ai-tools-grok/models-per-model-key.txt");
        assert_eq!(
            Grok.parse_auth(&probe(signed_out)).state,
            AuthState::SignedOut
        );
        for text in [
            api_key,
            model_key,
            "You are authenticated via deployment key.\n",
        ] {
            let s = Grok.parse_auth(&probe(text));
            assert_eq!(s.state, AuthState::ApiKey, "{text}");
            assert!(s.detail.unwrap().contains("billed"));
        }
        // Signed in: real output from the owner's machine.
        let signed_in =
            include_str!("../../../../docs/phases/evidence/ai-tools-grok/models-signed-in.txt");
        let s = Grok.parse_auth(&probe(signed_in));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("grok.com sign-in"));
        // Another sign-in wording still counts; never an account name in the method.
        let s = Grok.parse_auth(&probe("You are signed in as someone@example.com.\n"));
        assert_eq!(s.state, AuthState::Subscription);
        assert!(!s.method.unwrap().contains('@'));

        // Anything else is not trusted (Grok cannot confirm billing during a task).
        let s = Grok.parse_auth(&probe(
            "You are using a deprecated authentication method (WebLogin).\n",
        ));
        assert_eq!(s.state, AuthState::Unverified);
        let failed = ProbeOutput {
            exit_code: Some(1),
            ..probe(signed_in)
        };
        assert_eq!(Grok.parse_auth(&failed).state, AuthState::Unverified);
        assert_eq!(Grok.parse_auth(&probe("")).state, AuthState::Unknown);
        let timed_out = ProbeOutput {
            timed_out: true,
            ..ProbeOutput::default()
        };
        assert_eq!(Grok.parse_auth(&timed_out).state, AuthState::Unknown);
    }

    #[test]
    fn version_comes_from_grok_version() {
        let real = include_str!("../../../../docs/phases/evidence/ai-tools-grok/version.txt");
        assert_eq!(Grok.parse_version(&probe(real)).as_deref(), Some("1.0.41"));
        assert_eq!(Grok.checked_version(), "1.0.41");
    }

    #[test]
    fn a_task_is_one_acp_process_with_the_model_and_effort() {
        let request = TurnRequest {
            model: Some("grok-4.5".into()),
            effort: Some(Effort::Low),
            ..TurnRequest::default()
        };
        assert_eq!(
            Grok.turn_args(&request),
            [
                "agent",
                "--no-leader",
                "-m",
                "grok-4.5",
                "--reasoning-effort",
                "low",
                "stdio"
            ]
        );
        assert_eq!(
            Grok.turn_args(&TurnRequest::default()),
            ["agent", "--no-leader", "stdio"]
        );
        // The prompt never goes in the arguments; the ACP driver sends it on stdin.
        let mut parser = Grok.parser(&TurnRequest {
            working_dir: "/work/c1".into(),
            ..TurnRequest::default()
        });
        let opening = parser.open("secret plan 42").unwrap();
        assert!(opening.iter().all(|l| !l.contains("secret plan 42")));
        assert!(opening[0].contains("\"initialize\""));
    }

    #[test]
    fn the_profile_removes_grok_s_own_tools() {
        let alone = profile(false);
        assert_eq!(alone["tools"], json!([]));
        let removed = alone["disallowedTools"].as_array().unwrap();
        for tool in [
            "run_terminal_command",
            "write",
            "web_search",
            "ask_user_question",
            "use_tool",
        ] {
            assert!(removed.contains(&json!(tool)), "{tool}");
        }
        // With Plenipo's tool server: only the two tools that reach tool servers.
        let granted = profile(true);
        assert_eq!(granted["tools"], json!(["search_tool", "use_tool"]));
        assert!(!granted["disallowedTools"]
            .as_array()
            .unwrap()
            .contains(&json!("use_tool")));

        // The profile goes with the session.
        let request = TurnRequest {
            session: ProviderSession::New { preassigned: None },
            working_dir: "/work/c1".into(),
            tools: Some(ToolServer {
                name: "plenipo".into(),
                command: "/app/plenipo".into(),
                args: vec!["--plenipo-tools=t".into()],
                config_file: "/app/t.json".into(),
                call_timeout: std::time::Duration::from_secs(9),
                tools: Vec::new(),
            }),
            ..TurnRequest::default()
        };
        let mut parser = Grok.parser(&request);
        parser.open("Hi");
        let init = include_str!(
            "../../../../docs/phases/evidence/ai-tools-grok/acp-signed-out.agent.jsonl"
        )
        .lines()
        .next()
        .unwrap();
        let open: Value = serde_json::from_str(&parser.line(init, false).send[0]).unwrap();
        assert_eq!(open["method"], "session/new");
        assert_eq!(open["params"]["cwd"], "/work/c1");
        assert_eq!(open["params"]["mcpServers"][0]["name"], "plenipo");
        assert_eq!(open["params"]["mcpServers"][0]["command"], "/app/plenipo");
        assert_eq!(
            open["params"]["_meta"]["agentProfile"]["tools"],
            json!(["search_tool", "use_tool"])
        );
    }

    #[test]
    fn grok_gets_the_off_switches_and_no_credentials() {
        let env = Grok.fixed_env();
        let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("GROK_DISABLE_API_KEY_AUTH"), Some("1"));
        assert_eq!(get("GROK_DISABLE_AUTOUPDATER"), Some("1"));
        assert_eq!(get("GROK_BACKEND_SEARCH"), Some("0"));
        assert_eq!(get("GROK_CLAUDE_SKILLS_ENABLED"), Some("0"));
        assert_eq!(get("GROK_CURSOR_MCPS_ENABLED"), Some("0"));
        let passed = Grok.passthrough_env();
        assert!(passed.contains(&"GROK_HOME"));
        for secret in [
            "XAI_API_KEY",
            "GROK_CODE_XAI_API_KEY",
            "GROK_DEPLOYMENT_KEY",
        ] {
            assert!(!passed.contains(&secret));
            assert!(get(secret).is_none());
        }
    }

    #[test]
    fn known_models_come_with_their_own_effort_levels() {
        // Real output from the owner's machine: each model and the effort levels it offers.
        let listed = include_str!(
            "../../../../docs/phases/evidence/ai-tools-grok/models-effort-signed-in.txt"
        );
        let caps = Grok.capabilities();
        let names: Vec<&str> = caps.known_models.iter().map(|m| m.name.as_str()).collect();
        let lines: Vec<&str> = listed.lines().collect();
        assert_eq!(names.len(), lines.len());
        for (model, line) in caps.known_models.iter().zip(lines) {
            let (head, levels) = line.split_once(": ").unwrap();
            assert_eq!(head, format!("{} ({})", model.name, model.label));
            let mut wanted: Vec<&str> = levels.split(", ").collect();
            wanted.reverse(); // Grok lists them highest first; Plenipo lowest first.
            let ours: Vec<&str> = model.effort_levels.iter().map(|e| e.as_str()).collect();
            assert_eq!(ours, wanted, "{}", model.name);
        }
        assert!(!caps.billing_checked_per_turn);
    }
}

/// Phase 19: the AI tools page (ADR-058 to ADR-060).
#[cfg(test)]
mod ai_tools_page_tests {
    use super::*;

    #[test]
    fn signs_in_and_out_updates_and_goes_back_with_its_own_commands() {
        assert_eq!(
            Grok.account_command(AccountAction::SignIn),
            Some(vec!["login".to_owned()])
        );
        assert_eq!(
            Grok.account_command(AccountAction::SignOut),
            Some(vec!["logout".to_owned()])
        );
        assert_eq!(Grok.update_command(), Some(vec!["update".to_owned()]));
        assert_eq!(
            Grok.put_back_command("1.0.41"),
            Some(vec![
                "update".to_owned(),
                "--version".to_owned(),
                "1.0.41".to_owned()
            ])
        );
    }

    #[test]
    fn its_own_check_says_the_newest_version() {
        assert_eq!(
            Grok.newest_version(),
            NewestVersion::Command(vec!["update".into(), "--check".into(), "--json".into()])
        );
        let recorded = ProbeOutput {
            exit_code: Some(0),
            stdout: include_str!(
                "../../../../docs/phases/evidence/ai-tools-grok/update-check.jsonl"
            )
            .into(),
            ..ProbeOutput::default()
        };
        assert_eq!(Grok.parse_newest(&recorded).as_deref(), Some("1.0.41"));
        assert_eq!(Grok.parse_newest(&ProbeOutput::default()), None);
    }

    #[test]
    fn its_models_and_their_effort_come_from_its_recorded_initialize_answer() {
        let recorded = ProbeOutput {
            exit_code: Some(0),
            stdout: include_str!(
                "../../../../docs/phases/evidence/ai-tools-grok/acp-signed-out.agent.jsonl"
            )
            .into(),
            ..ProbeOutput::default()
        };
        let models = Grok.parse_models(&recorded).unwrap();
        assert_eq!(
            models,
            vec![
                KnownModel::new(
                    "grok-4.6",
                    "Grok 4.6",
                    &[Effort::Low, Effort::Medium, Effort::High, Effort::XHigh]
                ),
                KnownModel::new(
                    "grok-4.5",
                    "Grok 4.5",
                    &[Effort::Low, Effort::Medium, Effort::High]
                ),
            ]
        );
        let StatusCheck::Talk { args, lines, .. } = Grok.status_check(Path::new("/checks")) else {
            panic!("Grok talks ACP");
        };
        assert_eq!(args, ["agent", "--no-leader", "stdio"]);
        assert_eq!(lines.len(), 1, "initialize only: no conversation");
        assert!(lines[0].contains("\"initialize\""));
    }
}
